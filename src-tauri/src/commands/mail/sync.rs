//! 同步互斥：同一账户的同步 / 服务器写操作串行化。
//!
//! 为什么必须在后端做：前端那个 `this.syncing` 只是「本窗口正在同步」，
//! 而同步至少有四个触发源——手动刷新、定时轮询、规则动作、发信后的草稿箱同步。
//! 两个 `sync_folder` 撞在一起时，两边都从同一个 `last_uid` 起点拉取，
//! 结果是同一批邮件被解析两遍、`upsert_summaries` 互相覆盖，
//! 规则还可能被执行两次（一封邮件被移动两次，第二次会失败并留下脏日志）。
//! IMAP 侧的 `SELECT` / `EXPUNGE` 并发同样会互相干扰。
//!
//! 按「账户」粒度加锁而不是全局锁：两个账户的同步互不相干，一个慢账户
//! 不该把另一个账户的同步也堵住。用 `parking_lot::Mutex` 的 owning guard，
//! 守卫可以直接跨线程持有，不需要任何 unsafe。

use parking_lot::{ArcMutexGuard, Mutex};
use std::collections::HashMap;
use std::sync::Arc;

type AccountMutex = Arc<Mutex<()>>;

static LOCKS: once_cell::sync::Lazy<Mutex<HashMap<String, AccountMutex>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

fn mutex_for(account_id: &str) -> AccountMutex {
    let mut map = LOCKS.lock();
    map.entry(account_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// 账户锁的持有者，Drop 时释放。
pub struct AccountLock {
    // `ArcMutexGuard` 内部持有一份 Arc，能安全地跨线程移动与释放。
    _guard: ArcMutexGuard<parking_lot::RawMutex, ()>,
}

/// 取得账户级同步锁（阻塞）。同步本来就该排队，不该让用户看到「正忙」的失败提示。
pub fn lock_account(account_id: &str) -> AccountLock {
    let mutex = mutex_for(account_id);
    AccountLock {
        _guard: Mutex::lock_arc(&mutex),
    }
}


/// 当前正被占用的账户（供诊断面板展示「哪个账户在同步」）。
pub fn busy_accounts() -> Vec<String> {
    let map = LOCKS.lock();
    map.iter()
        .filter(|(_, m)| m.try_lock().is_none())
        .map(|(id, _)| id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn same_account_is_serialised() {
        let counter = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let counter = counter.clone();
            let peak = peak.clone();
            handles.push(std::thread::spawn(move || {
                let _g = lock_account("acc");
                let now = counter.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(30));
                counter.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1, "同一账户不允许并发进入临界区");
    }

    #[test]
    fn different_accounts_do_not_block_each_other() {
        let a = lock_account("a");
        // 另一个账户的锁必须立刻可用，否则一个慢账户会拖住所有账户。
        let b = lock_account("b");
        drop(a);
        drop(b);
    }
}
