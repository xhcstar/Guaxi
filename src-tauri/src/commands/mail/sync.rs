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
use std::collections::HashSet;
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


// ---------------------------------------------------------------------------
// 用户文件夹快照
// ---------------------------------------------------------------------------
//
// 「哪些文件夹是用户自己建的」只能在本地判断：服务器下发的 LIST 里，用户的
// 「工作/2026」和服务商遗留的「其他文件夹/QQ邮件订阅」长得完全一样。
// 这里缓存每个账户最近一次同步**之前**的文件夹表快照，由 `imap::list_folders`
// 在解析前读取：快照里出现过的就是用户这边的既有文件夹，新出现的才需要判断归属。
//
// 用快照而不是直接查库：`list_folders` 处在连接 / STATUS 的同步阻塞路径上，
// 不该再开一次数据库连接。

static FOLDER_SNAPSHOTS: once_cell::sync::Lazy<Mutex<HashMap<String, HashSet<String>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

/// 记下这次同步之前本地已知的文件夹名（同步结束后会被服务器结果覆盖）。
pub fn remember_folder_names(account_id: &str, names: HashSet<String>) {
    FOLDER_SNAPSHOTS.lock().insert(account_id.to_string(), names);
}

/// 取出快照；没有快照（首次同步 / 刚启动）时返回空集合。
pub fn folder_names(account_id: &str) -> HashSet<String> {
    FOLDER_SNAPSHOTS
        .lock()
        .get(account_id)
        .cloned()
        .unwrap_or_default()
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
