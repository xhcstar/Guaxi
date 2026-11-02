//! IMAP 长连接池 + IDLE 推送。
//!
//! 之前每次操作都 `login()` 建一条新连接：拖拽移动 20 封邮件 = 20 次 TCP + TLS +
//! AUTHENTICATE 往返；读一封邮件要等一次完整握手。Outlook / Thunderbird 都是
//! 维持一条长连接。
//!
//! 结构：
//! - 每个账户**一条**「请求-应答」连接（`Pooled`），所有同步 / 标记 / 移动都复用它；
//! - 另有一条**独立的 IDLE 连接**（`idle` 子模块），只做「等服务器说有变化」，
//!   收到变化就唤醒请求连接做一次增量同步。IDLE 会独占它那条连接，
//!   所以绝不能和请求连接共用——否则同步期间收不到推送，推送期间发不出命令。
//! - 连接失效（服务器超时断开 / 网络切换）时透明重连一次，重连失败才把错误抛给调用方。
//!
//! 连接池按账户 id 保存。账户设置改变（服务器 / 用户名 / 认证方式）时用
//! `invalidate` 丢弃旧连接，下次使用自动重建。

use super::imap;
use super::model::Account;
use parking_lot::{ArcMutexGuard, Mutex, RawMutex};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub type ImapSession = imap::ImapSession;

/// 一条可复用的 IMAP 会话。
pub struct Pooled {
    session: Option<ImapSession>,
    /// 这条连接当前 SELECT 的文件夹；切换文件夹时如果相同就跳过 SELECT。
    mailbox: Option<String>,
    /// 服务器能力（登录后取一次）。MOVE / UIDPLUS / IDLE 的判定都靠它。
    pub capabilities: Vec<String>,
    /// 连接建立时账户的关键参数；任何一项变了就必须重连。
    fingerprint: String,
}

impl Pooled {
    pub fn session(&mut self) -> &mut ImapSession {
        self.session
            .as_mut()
            .expect("连接已建立后才能借用会话")
    }

    /// 确保当前 SELECT 的是目标文件夹。
    pub fn select(&mut self, folder: &str) -> Result<(), String> {
        if self.mailbox.as_deref() == Some(folder) {
            return Ok(());
        }
        self.session()
            .select(folder)
            .map_err(|e| format!("选择文件夹失败（{folder}）: {e}"))?;
        self.mailbox = Some(folder.to_string());
        Ok(())
    }

    /// 服务器是否支持某项能力。
    pub fn supports(&self, name: &str) -> bool {
        self.capabilities
            .iter()
            .any(|c| c.eq_ignore_ascii_case(name))
    }

    /// 主动检查连接是否还活着（NOOP 是最廉价的探测）。
    fn alive(&mut self) -> bool {
        match self.session.as_mut() {
            Some(session) => session.noop().is_ok(),
            None => false,
        }
    }

    /// 丢弃底层会话（连接坏了、或者账户配置变了）。
    fn disconnect(&mut self) {
        if let Some(mut session) = self.session.take() {
            let _ = session.logout();
        }
        self.mailbox = None;
        self.capabilities.clear();
        self.fingerprint.clear();
    }
}

/// 账户指纹：用于判断「连接还能不能代表当前账户配置」。
fn fingerprint(account: &Account) -> String {
    format!(
        "{}:{}:{:?}|{}|{}",
        account.imap_host,
        account.imap_port,
        account.imap_security,
        account.login_name(),
        account.auth_kind,
    )
}

type Entry = Arc<Mutex<Pooled>>;

static POOL: once_cell::sync::Lazy<Mutex<HashMap<String, Entry>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

fn entry_for(account: &Account) -> Entry {
    let mut map = POOL.lock();
    map.entry(account.id.clone())
        .or_insert_with(|| {
            Arc::new(Mutex::new(Pooled {
                session: None,
                mailbox: None,
                capabilities: Vec::new(),
                fingerprint: String::new(),
            }))
        })
        .clone()
}

/// 借用到的连接守卫。Drop 时归还连接池（`parking_lot` 的 owning guard
/// 会自己解锁，即使调用方 panic 也不会把账户永久锁死）。
pub struct Lease {
    guard: ArcMutexGuard<RawMutex, Pooled>,
    account_id: String,
}

impl std::ops::Deref for Lease {
    type Target = Pooled;
    fn deref(&self) -> &Pooled {
        &self.guard
    }
}

impl std::ops::DerefMut for Lease {
    fn deref_mut(&mut self) -> &mut Pooled {
        &mut self.guard
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        // 命中连接失效时把它丢掉，下次借用会重建。这里只在「明显坏了」时做，
        // 不做 NOOP 探测——归还时的额外往返反而会拖慢批量操作。
        if self.guard.session.is_none() {
            debug!(account = %self.account_id, "mail: 归还的连接已不可用，下次将重建");
        }
    }
}

/// 借用一条已认证的 IMAP 连接（阻塞）。连接不可用时自动重建。
pub fn lease(account: &Account) -> Result<Lease, String> {
    let entry = entry_for(account);
    let mut guard = Mutex::lock_arc(&entry);
    let want = fingerprint(account);

    if guard.session.is_some() && guard.fingerprint != want {
        info!(account = %account.email, "mail: 账户配置已变，丢弃旧连接");
        guard.disconnect();
    }
    if guard.session.is_none() {
        connect(account, &mut guard)?;
    } else if !guard.alive() {
        debug!(account = %account.email, "mail: 长连接已失效，重连一次");
        guard.disconnect();
        connect(account, &mut guard)?;
    }

    Ok(Lease {
        guard,
        account_id: account.id.clone(),
    })
}

/// 建立并认证连接，顺便把服务器能力读出来。
fn connect(account: &Account, guard: &mut Pooled) -> Result<(), String> {
    let secret = imap::auth_secret(account)?;
    let mut session = imap::login(account, &secret)?;
    guard.capabilities = session
        .capabilities()
        .map(|caps| caps.iter().map(|c| format!("{c:?}").to_uppercase()).collect())
        .unwrap_or_default();
    guard.session = Some(session);
    guard.mailbox = None;
    guard.fingerprint = fingerprint(account);
    Ok(())
}

/// 让某个账户的下一条连接强制重建（账户配置改了、OAuth 令牌换了之后调用）。
pub fn invalidate(account_id: &str) {
    let entry = {
        let map = POOL.lock();
        map.get(account_id).cloned()
    };
    if let Some(entry) = entry {
        let mut guard = Mutex::lock_arc(&entry);
        guard.disconnect();
    }
    // IDLE 连接也要一起停：它是按账户 id 起的长驻任务，账户参数变了旧循环没意义。
    idle::stop(account_id);
}

/// 清空所有账户的连接（恢复备份 / 切换数据库后调用）。
pub fn invalidate_all() {
    let entries: Vec<(String, Entry)> = {
        let map = POOL.lock();
        map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    for (id, entry) in entries {
        let mut guard = Mutex::lock_arc(&entry);
        guard.disconnect();
        idle::stop(&id);
    }
}

/// 已经在池里的账户（诊断用）。
pub fn pooled_accounts() -> Vec<String> {
    POOL.lock().keys().cloned().collect()
}

// ---------------------------------------------------------------------------
// IDLE：服务器推送
// ---------------------------------------------------------------------------

/// IDLE 子模块。
///
/// 用法：`start(account, folder, on_change)` 起一条常驻线程；
/// 收到 `* n EXISTS` / `* n EXPUNGE` / `* n FETCH` 时回调一次。
/// 线程在账户被删除、`stop` 被调用、或者连续重连失败时退出。
pub mod idle {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    struct IdleHandle {
        stop: Arc<AtomicBool>,
    }

    static IDLE: once_cell::sync::Lazy<Mutex<HashMap<String, IdleHandle>>> =
        once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

    /// 某账户的 IDLE 是否在跑。
    pub fn is_running(account_id: &str) -> bool {
        IDLE.lock().contains_key(account_id)
    }

    /// 启动 IDLE。重复调用是幂等的（已经在跑就忽略）。
    ///
    /// `account` 会被克隆进线程：IDLE 是长驻的，不能借用调用方的栈。
    pub fn start(
        account: Account,
        folder: String,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Result<(), String> {
        let account_id = account.id.clone();
        {
            let map = IDLE.lock();
            if let Some(existing) = map.get(&account_id) {
                if !existing.stop.load(Ordering::SeqCst) {
                    return Ok(());
                }
            }
        }
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut map = IDLE.lock();
            map.insert(
                account_id.clone(),
                IdleHandle {
                    stop: stop.clone(),
                },
            );
        }
        let on_change = Arc::new(on_change);
        std::thread::Builder::new()
            .name(format!("mail-idle-{account_id}"))
            .spawn(move || {
                let mut backoff = 2u64;
                while !stop.load(Ordering::SeqCst) {
                    match idle_once(&account, &folder, &stop, &*on_change) {
                        Ok(()) => backoff = 2,
                        Err(e) => {
                            if stop.load(Ordering::SeqCst) {
                                break;
                            }
                            warn!(account = %account.email, error = %e, "mail: IDLE 连接异常，稍后重连");
                            // 退避而不是立刻重连：服务器把我们踢掉时（例如登录次数超限）
                            // 立刻重连会被当成滥用，越重连越连不上。
                            std::thread::sleep(Duration::from_secs(backoff));
                            backoff = (backoff * 2).min(120);
                        }
                    }
                }
                IDLE.lock().remove(&account_id);
                debug!(account = %account.email, "mail: IDLE 已停止");
            })
            .map_err(|e| format!("启动 IDLE 线程失败: {e}"))?;
        Ok(())
    }

    /// 一次 IDLE 会话：登录 → SELECT → 循环 wait_with_timeout。
    ///
    /// 用 `wait_with_timeout` 而不是 `wait_keepalive`：后者在超时后会重发 IDLE
    /// 但不会给我们机会检查停止标志；用前者每 25 分钟重发一次，顺便检查退出条件。
    fn idle_once(
        account: &Account,
        folder: &str,
        stop: &Arc<AtomicBool>,
        on_change: &(dyn Fn() + Send + Sync),
    ) -> Result<(), String> {
        let secret = imap::auth_secret(account)?;
        let mut session = imap::login(account, &secret)?;
        session
            .select(folder)
            .map_err(|e| format!("IDLE 选择文件夹失败: {e}"))?;
        debug!(account = %account.email, folder, "mail: IDLE 开始等待");

        loop {
            if stop.load(Ordering::SeqCst) {
                let _ = session.logout();
                return Ok(());
            }
            let handle = session
                .idle()
                .map_err(|e| format!("发起 IDLE 失败: {e}"))?;
            // 29 分钟是 RFC 2177 的上限：再长服务器会判定客户端失联。
            let outcome = handle
                .wait_with_timeout(Duration::from_secs(25 * 60))
                .map_err(|e| format!("IDLE 等待失败: {e}"))?;
            if outcome == ::imap::extensions::idle::WaitOutcome::MailboxChanged {
                debug!(account = %account.email, folder, "mail: IDLE 收到服务器变更");
                on_change();
            }
        }
    }

    /// 停止某账户的 IDLE（线程会在下一次循环检查点时退出）。
    pub fn stop(account_id: &str) {
        let map = IDLE.lock();
        if let Some(handle) = map.get(account_id) {
            handle.stop.store(true, Ordering::SeqCst);
        }
    }

    /// 停止所有 IDLE（应用退出 / 切换库）。
    #[allow(dead_code)]
    pub fn stop_all() {
        let map = IDLE.lock();
        for handle in map.values() {
            handle.stop.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_with_login_identity() {
        let mut account = crate::commands::mail::model::Account {
            id: "x".into(),
            email: "a@b.c".into(),
            display_name: String::new(),
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            imap_security: crate::commands::mail::model::Security::Ssl,
            protocol: "imap".into(),
            pop3_host: String::new(),
            pop3_port: 995,
            pop3_security: crate::commands::mail::model::Security::None,
            pop3_leave_on_server: true,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_security: crate::commands::mail::model::Security::Ssl,
            ews_url: String::new(),
            username: String::new(),
            auth_kind: "password".into(),
            oauth_provider: String::new(),
            oauth_expires: 0,
            proxy: None,
            accent: String::new(),
            signature: String::new(),
            folder_map: Default::default(),
            delegate: Vec::new(),
            sieve: None,
            created_at: 0,
            last_sync_at: 0,
        };
        let before = fingerprint(&account);
        account.username = "other".into();
        assert_ne!(before, fingerprint(&account));
        account.auth_kind = "oauth2".into();
        assert!(fingerprint(&account).contains("oauth2"));
    }
}
