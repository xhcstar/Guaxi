//! 邮箱客户端（应用页「邮件」卡片）。
//!
//! 结构：
//! - `model`    数据结构（IPC 与库表共用的扁平形态）
//! - `store`    SQLite（账户 / 文件夹 / 邮件 / 正文 / 规则 / outbox / 设置）
//! - `net`      代理感知的 TCP + rustls（IMAP 与 SMTP 共用）
//! - `imap`     同步、标记、移动、追加、诊断
//! - `smtp`     手写 SMTP 会话（复用 `net`，代理对发信同样生效）
//! - `mime`     来信解析 / 去信组装 / 回复头
//! - `sanitize` 来信 HTML 清洗（安全边界）
//! - `rules`    收信规则判定 + 撤回语义
//! - `theme`    两套内置明暗主题 + 自定义 token 表
//!
//! 口令只进系统 keychain（service = `guaxi-mail`，account = 账户 id），
//! 库里连密文都不存——这样备份 / 同步数据库不会把口令带出去。

pub mod contacts;
/// 本地静态加密：正文与附件清单在落盘前加密，密钥存系统 keychain。
pub mod crypto;
pub mod export;
/// 本地 Exchange Server（EWS）通道。
pub mod ews;
/// 原生 Exchange Online / Microsoft 365（Microsoft Graph）通道。
pub mod graph;
pub mod imap;
pub mod mime;
pub mod model;
pub mod net;
/// 系统通知、诊断导出、危险附件判定。
pub mod notify;
/// 离线写队列：断网时排队，恢复后重放。
pub mod offline;
/// OAuth2（XOAUTH2）：Gmail / Outlook 现代账户的唯一可用认证方式。
pub mod oauth;
pub mod pgp;
pub mod pool;
/// POP3 会话（RFC 1939）：手写在 `net` 之上，与 SMTP 同一套代理 / TLS。
pub mod pop3;
/// POP3 账户的同步与本地文件夹模型。
pub mod pop3_sync;
/// 批量写入：一次同步只开一个连接、一个事务。
pub mod batch;
pub mod rules;
pub mod sanitize;
/// 服务器端规则（Sieve，RFC 5228 / RFC 5804）。
pub mod sieve;
pub mod smtp;
pub mod smime;
pub mod smime_crypto;
pub mod store;
/// 同步互斥 + 离线写队列。
pub mod sync;
pub mod theme;

use model::{
    Account, FolderRole, MailAddress, MailFolder, MailRule, MailSettings, MessageDetail,
    MessageSummary, OutboxItem, ProxyConfig, SearchQuery, SendRequest,
};
use std::collections::HashMap;
use std::sync::OnceLock;
use tauri::{command, AppHandle, Emitter};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const KEYRING_SERVICE: &str = "guaxi-mail";
/// 同步状态广播：前端监听它刷新未读数与列表。
pub const MAIL_SYNC_EVENT: &str = "mail-sync-progress";
/// 新邮件到达。
pub const MAIL_NEW_EVENT: &str = "mail-new-messages";

static INITED: OnceLock<()> = OnceLock::new();
/// 启动时前端会并发调用一批命令（设置、主题、账户、未读…），它们都会走 `ensure_init`。
/// 建库 / 迁移 / 补表必须串行：两个线程同时跑迁移时，其中一个会拿到 SQLITE_BUSY
/// 直接把整个命令打回。主题列表就这么被吞过一次——设置页的「主题」整块空白，
/// 而其它命令看起来都正常。
static INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn ensure_init(app: &AppHandle) -> Result<(), String> {
    if INITED.get().is_some() {
        return Ok(());
    }
    let _guard = INIT_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // 等锁期间可能已经有别的命令初始化完成。
    if INITED.get().is_some() {
        return Ok(());
    }
    store::init(app)?;
    contacts::store::init()?;
    offline::init_table()?;
    if INITED.set(()).is_ok() {
        // 后台补一次「收件人」通讯录：老版本只收进了发件人，邮件组地址一直缺。
        // 放到后台跑，不和首屏抢时间；它全是同步 SQL，别占 async 工作线程。
        tauri::async_runtime::spawn_blocking(|| {
            match contacts::backfill_recipients_once() {
                Ok(0) => {}
                Ok(n) => info!(rows = n, "mail: 已补齐通讯录里的收件人"),
                Err(e) => warn!(error = %e, "mail: 补齐通讯录收件人失败"),
            }
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 凭据
// ---------------------------------------------------------------------------

fn keyring_entry(account_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, account_id).map_err(|e| format!("访问系统凭据失败: {e}"))
}

pub fn read_secret(account_id: &str) -> Option<String> {
    keyring_entry(account_id)
        .ok()
        .and_then(|entry| entry.get_password().ok())
}

fn write_secret(account_id: &str, secret: &str) -> Result<(), String> {
    keyring_entry(account_id)?
        .set_password(secret)
        .map_err(|e| format!("写入系统凭据失败: {e}"))
}

fn delete_secret(account_id: &str) {
    if let Ok(entry) = keyring_entry(account_id) {
        let _ = entry.delete_credential();
    }
}

/// 账户级代理覆盖全局设置。`None` 表示直连（账户显式关掉了代理）。
pub fn effective_proxy(account: &Account) -> Option<&ProxyConfig> {
    account.proxy.as_ref()
}

/// 取「可用于认证的秘密」。
///
/// 密码账户是 keychain 里的口令 / 授权码；OAuth2 账户是访问令牌（过期自动刷新）。
/// 所有调用点都走这里，认证方式的差异就被收敛到一个函数里——
/// 否则每加一处「连服务器」的代码都要记得判断 `auth_kind`。
fn account_password(account: &Account) -> Result<String, String> {
    if account.auth_kind.eq_ignore_ascii_case("oauth2") {
        return oauth::access_token(account);
    }
    read_secret(&account.id).ok_or_else(|| {
        format!(
            "账户 {} 尚未保存密码或授权码，请在账户设置里填写",
            account.email
        )
    })
}

fn load_account(account_id: &str) -> Result<Account, String> {
    store::get_account(account_id)?.ok_or_else(|| "账户不存在".to_string())
}

/// 按语义角色取文件夹的服务器名（Graph 是 folder id，EWS 是 FolderId）。
fn folder_name_by_role(account_id: &str, role: FolderRole) -> Result<String, String> {
    folder_by_role(account_id, role)
        .map(|folder| folder.name)
        .ok_or_else(|| format!("账户缺少「{}」文件夹，请先刷新文件夹", role.as_str()))
}

/// 同一角色可能有多个候选：QQ 的「其他文件夹/Deleted Items」模板壳也会被标成
/// Trash，真正的「Deleted Messages」在它旁边。删除 / 垃圾邮件必须选真正可用的
/// 那个，否则邮件会被搬进一个根本不展示、也不同步的壳里。
fn folder_by_role(account_id: &str, role: FolderRole) -> Option<MailFolder> {
    let mut candidates: Vec<MailFolder> = store::list_folders(account_id)
        .ok()?
        .into_iter()
        .filter(|folder| folder.role == role && folder.selectable)
        .collect();
    candidates.sort_by_key(|folder| {
        let label = format!("{} {}", folder.display_name, folder.name).to_ascii_lowercase();
        let shell = label.contains("其他文件夹") || label.contains("other folders");
        (
            shell as u8,
            (folder.total == 0) as u8,
            folder.display_name.chars().count(),
        )
    });
    candidates.into_iter().next()
}

/// 这封邮件的原始报文。POP3 账户没有按 uid 取单封的接口，走 `pop3_sync` 重新下载。
///
/// 所有「先拿原文再解析」的路径（打开正文、存 .eml、导出附件、规则转发）都走这里，
/// 协议差异就只收敛在这一个地方。
///
/// 先查本地原文缓存：打开过一封邮件之后，附件、另存 .eml、导出都直接读盘。
/// 以前每次都重新 `BODY.PEEK[]` 拉整封（QQ 上 800 KB 的往来邮件 + TLS 往返
/// 经常十秒起步），用户点开附件时看到的是一段没有任何反馈的等待。
async fn fetch_raw_bytes(
    account: &Account,
    password: &str,
    folder: &str,
    uid: u32,
) -> Result<Vec<u8>, String> {
    let (account_id, folder_owned) = (account.id.clone(), folder.to_string());
    let cached = tauri::async_runtime::spawn_blocking(move || {
        store::cached_raw_body(&account_id, &folder_owned, uid)
    })
    .await
    .ok()
    .flatten();
    if let Some(raw) = cached {
        return Ok(raw);
    }

    if graph::is_graph(account) {
        let remote_id = store::remote_id_for(&account.id, folder, uid)
            .ok_or_else(|| "这封 Graph 邮件还没有远端 ID，请先同步该文件夹".to_string())?;
        let acc = account.clone();
        let raw = tauri::async_runtime::spawn_blocking(move || graph::fetch_mime(&acc, &remote_id))
            .await
            .map_err(|e| format!("取信任务失败: {e}"))??;
        let (account_id, folder_owned, bytes) =
            (account.id.clone(), folder.to_string(), raw.clone());
        if let Err(error) = tauri::async_runtime::spawn_blocking(move || {
            store::cache_raw_body(&account_id, &folder_owned, uid, &bytes)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
        {
            warn!(error = %error, "mail: 缓存邮件原文失败");
        }
        return Ok(raw);
    }

    if ews::is_ews(account) {
        let remote_id = store::remote_id_for(&account.id, folder, uid)
            .ok_or_else(|| "这封 EWS 邮件还没有远端 ID，请先同步该文件夹".to_string())?;
        let acc = account.clone();
        let raw = tauri::async_runtime::spawn_blocking(move || ews::fetch_mime(&acc, &remote_id))
            .await
            .map_err(|e| format!("取信任务失败: {e}"))??;
        let (account_id, folder_owned, bytes) =
            (account.id.clone(), folder.to_string(), raw.clone());
        if let Err(error) = tauri::async_runtime::spawn_blocking(move || {
            store::cache_raw_body(&account_id, &folder_owned, uid, &bytes)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
        {
            warn!(error = %error, "mail: 缓存邮件原文失败");
        }
        return Ok(raw);
    }

    let raw = if pop3_sync::is_pop3(account) {
        let acc = account.clone();
        let parsed =
            tauri::async_runtime::spawn_blocking(move || pop3_sync::fetch_message(&acc, uid))
                .await
                .map_err(|e| format!("取信任务失败: {e}"))??;
        parsed.raw.ok_or_else(|| "POP3 邮件原文不可用".to_string())?
    } else {
        let (acc, f, pw) = (account.clone(), folder.to_string(), password.to_string());
        tauri::async_runtime::spawn_blocking(move || imap::fetch_raw(&acc, &pw, &f, uid))
            .await
            .map_err(|e| format!("取信任务失败: {e}"))??
    };
    // 缓存失败不影响本次读取（最多下次再拉一遍），所以这里只记日志。
    let (account_id, folder_owned, bytes) = (account.id.clone(), folder.to_string(), raw.clone());
    if let Err(error) =
        tauri::async_runtime::spawn_blocking(move || {
            store::cache_raw_body(&account_id, &folder_owned, uid, &bytes)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()))
    {
        warn!(error = %error, "mail: 缓存邮件原文失败");
    }
    Ok(raw)
}

/// 执行一次「会写到服务器」的操作；断网失败时按配置入离线队列。
///
/// 关键点：**本地状态先改，服务器同步失败不算失败**。用户点了已读就应当立刻变已读，
/// 网络问题留给队列去补——这正是「离线可用」的全部含义。
/// 但认证失败、文件夹不存在这类永久性错误不能排队（重放一百次也不会成功），
/// 由 `offline::is_offline_error` 判定。
fn write_with_fallback(
    account: &Account,
    op: offline::OfflineOp,
    action: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let settings = store::load_settings();
    match action() {
        Ok(()) => Ok(()),
        Err(e) => {
            if settings.offline_queue && offline::is_offline_error(&e) {
                let mut op = op;
                op.account_id = account.id.clone();
                if op.created_at == 0 {
                    op.created_at = chrono::Utc::now().timestamp();
                }
                if let Err(queue_error) = offline::enqueue(&op) {
                    return Err(format!("{e}（离线排队也失败: {queue_error}）"));
                }
                info!(kind = %op.kind, folder = %op.folder, "mail: 网络问题，写操作已排队待重放");
                return Ok(());
            }
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// 账户
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_list_accounts(app: AppHandle) -> Result<Vec<Account>, String> {
    ensure_init(&app)?;
    store::list_accounts()
}

/// 保存账户。`secret` 非空时同时写 keychain；为空表示「沿用原密码」。
#[command]
pub async fn mail_save_account(
    app: AppHandle,
    account: Account,
    secret: Option<String>,
) -> Result<Account, String> {
    ensure_init(&app)?;
    let mut account = account;
    if account.id.trim().is_empty() {
        account.id = Uuid::new_v4().to_string();
    }
    if account.created_at == 0 {
        account.created_at = chrono::Utc::now().timestamp();
    }
    if account.accent.trim().is_empty() {
        account.accent = "#5b7cfa".to_string();
    }
    // Graph / EWS 是原生 Exchange 通道：不要被折叠回 imap，否则保存后
    // 账户会悄悄去连一个不存在的 IMAP 服务器。EWS 尚未实现，先不接受该值。
    let protocol = account.protocol.trim().to_ascii_lowercase();
    account.protocol = if protocol == "graph" || protocol == "ews" {
        protocol
    } else if pop3_sync::is_pop3(&account) {
        "pop3".to_string()
    } else {
        "imap".to_string()
    };
    if account.protocol == "pop3" {
        // POP3 账户不填 IMAP 地址（界面上也没有那一栏）：存一个空值，
        // 避免将来切回 IMAP 时悄悄连到一个过期的旧服务器。
        account.imap_host = String::new();
        account.pop3_host = account.pop3_host.trim().to_string();
    } else {
        account.pop3_host = String::new();
    }
    if account.protocol == "graph" {
        // Graph 账户不经过 IMAP/POP3/SMTP：清掉表单里可能残留的旧地址，
        // 避免设置页显示一组「填了但没用」的服务器。
        account.imap_host = String::new();
        account.pop3_host = String::new();
        account.smtp_host = String::new();
        account.auth_kind = "oauth2".to_string();
        if account.oauth_provider.trim().is_empty() {
            account.oauth_provider = "graph".to_string();
        }
    }
    if account.protocol == "ews" {
        // EWS 只使用 EwsUrl；本地 Exchange 通常用 Basic，混合部署可用 OAuth。
        account.imap_host = String::new();
        account.pop3_host = String::new();
        account.smtp_host = String::new();
        account.ews_url = account.ews_url.trim().to_string();
    }
    if let Some(secret) = secret
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        write_secret(&account.id, secret)?;
    }
    store::upsert_account(&account)?;
    // 新账户一接进来就有「存档 / 发件箱」两个固定入口：否则用户第一次点归档时
    // 侧栏里还找不到存档，要等下次同步才补出来。
    ensure_standard_folders(&account);
    // 重建账户时把既有规则引用的文件夹补回来（删账户会连本地文件夹一起清掉）。
    // 放到后台：建文件夹要连服务器，不能让「保存账户」卡在这上面；
    // 建完会发事件，侧栏收到后自己刷新。
    let rules_app = app.clone();
    let rules_account = account.clone();
    tauri::async_runtime::spawn(async move {
        ensure_rule_folders(&rules_app, &rules_account).await;
    });
    Ok(account)
}

/// 补齐账户的标准文件夹占位（存档 / 发件箱）。失败只记日志——文件夹会在下次
/// 同步时再补，不该让「保存账户」因为它失败。
fn ensure_standard_folders(account: &Account) {
    let Ok(existing) = store::list_folders(&account.id) else {
        return;
    };
    for (role, name, display) in [
        (FolderRole::Archive, "Archive", "存档"),
        (FolderRole::Outbox, "Outbox", "发件箱"),
    ] {
        // Graph 的「存档」是服务器上的 wellKnownName；本地不要先造一个同名占位，
        // 否则第一次文件夹刷新前侧栏会出现一个点进去为空的假存档。
        if matches!(account.protocol.as_str(), "graph" | "ews") && role == FolderRole::Archive {
            continue;
        }
        if existing.iter().any(|f| f.role == role) {
            continue;
        }
        let folder = MailFolder {
            account_id: account.id.clone(),
            name: name.to_string(),
            display_name: display.to_string(),
            delimiter: None,
            role,
            unread: 0,
            total: 0,
            uid_validity: 0,
            uid_next: 0,
            selectable: true,
            local_only: role.is_local_only(),
            user_created: false,
        };
        if let Err(error) = store::insert_folder_row(&folder) {
            warn!(error = %error, "mail: 补标准文件夹失败");
        }
    }
}

/// 删除账户。
///
/// `delete_local_mail` 由用户在确认框里选：true = 连本机缓存的邮件一起删干净
/// （正文、检索索引、附件缓存、该账户的本地草稿文件），false = 只移除账户。
#[command]
pub async fn mail_delete_account(
    app: AppHandle,
    account_id: String,
    delete_local_mail: Option<bool>,
) -> Result<(), String> {
    ensure_init(&app)?;
    let wipe = delete_local_mail.unwrap_or(true);
    store::delete_account(&account_id, wipe)?;
    delete_secret(&account_id);
    // 本地加密密钥也要一起删：留着会在系统凭据库里堆孤儿条目，
    // 而且下次用同一个账户 id 恢复数据时会因密钥不匹配而解不开。
    crypto::delete_key(&account_id);
    if wipe {
        purge_local_artifacts(&app, &account_id);
    }
    Ok(())
}

/// 删掉账户在磁盘上的残余：附件缓存目录与本地暂存的草稿文件。
///
/// 数据库里的邮件行已经删了，但附件缓存（`attachments/<account_id>/`）和
/// 编辑器暂存的 JSON 草稿还在——「删干净」得把这两处也一起清掉。
fn purge_local_artifacts(app: &AppHandle, account_id: &str) {
    let Ok(root) = store::data_root(app) else {
        return;
    };
    let safe_id = sanitize_filename(account_id);
    let attach_dir = root.join("attachments").join(&safe_id);
    if attach_dir.exists() {
        if let Err(error) = std::fs::remove_dir_all(&attach_dir) {
            warn!(error = %error, "mail: 清理附件缓存失败");
        }
    }
    let drafts_dir = root.join("drafts");
    let Ok(entries) = std::fs::read_dir(&drafts_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let owner = value
            .get("draft")
            .and_then(|d| d.get("accountId"))
            .and_then(|v| v.as_str());
        if owner == Some(account_id) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[command]
pub async fn mail_has_secret(app: AppHandle, account_id: String) -> Result<bool, String> {
    ensure_init(&app)?;
    Ok(read_secret(&account_id).is_some())
}

/// 删除 keychain 里的口令（连接诊断用的临时凭据靠它清理）。
#[command]
pub async fn mail_delete_secret(app: AppHandle, account_id: String) -> Result<(), String> {
    ensure_init(&app)?;
    delete_secret(&account_id);
    Ok(())
}

/// 连接诊断：同时探测 IMAP 与 SMTP，返回两个阶段的结果。
#[command]
pub async fn mail_test_account(
    app: AppHandle,
    account: Account,
    secret: Option<String>,
) -> Result<Vec<model::ConnectionProbe>, String> {
    ensure_init(&app)?;
    if graph::is_graph(&account) {
        let acc = account.clone();
        let result = tauri::async_runtime::spawn_blocking(move || graph::test_account(&acc))
            .await
            .map_err(|e| format!("诊断任务失败: {e}"))?;
        return Ok(vec![model::ConnectionProbe {
            ok: result.is_ok(),
            stage: "graph".into(),
            detail: match result {
                Ok(()) => "Microsoft Graph 连接正常".into(),
                Err(error) => error,
            },
            capabilities: vec!["Microsoft Graph".into()],
        }]);
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let secret_for_test = secret.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            ews::test_account_with_secret(&acc, secret_for_test.as_deref())
        })
        .await
        .map_err(|e| format!("诊断任务失败: {e}"))?;
        return Ok(vec![model::ConnectionProbe {
            ok: result.is_ok(),
            stage: "ews".into(),
            detail: match result {
                Ok(()) => "Exchange EWS 连接正常".into(),
                Err(error) => error,
            },
            capabilities: vec!["Exchange EWS".into()],
        }]);
    }
    // 新建账户时表单里的口令还没落库，优先用传进来的那份。
    // OAuth2 账户的表单里没有口令字段：直接取（可能还要刷新的）访问令牌。
    let password = secret
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if account.auth_kind.eq_ignore_ascii_case("oauth2") {
                oauth::access_token(&account).ok()
            } else {
                read_secret(&account.id)
            }
        })
        .unwrap_or_default();

    let receive_probe = tauri::async_runtime::spawn_blocking({
        let account = account.clone();
        let password = password.clone();
        move || {
            if pop3_sync::is_pop3(&account) {
                pop3_sync::probe(&account, &password)
            } else {
                imap::probe(&account, &password)
            }
        }
    })
    .await
    .map_err(|e| format!("诊断任务失败: {e}"))?;

    let smtp_probe = tauri::async_runtime::spawn_blocking(move || smtp_probe(&account, &password))
        .await
        .map_err(|e| format!("诊断任务失败: {e}"))?;

    Ok(vec![receive_probe, smtp_probe])
}

fn smtp_probe(account: &Account, password: &str) -> model::ConnectionProbe {
    let mut probe = model::ConnectionProbe {
        ok: false,
        stage: "connect".into(),
        detail: String::new(),
        capabilities: Vec::new(),
    };
    match smtp::SmtpSession::connect(
        &account.smtp_host,
        account.smtp_port,
        account.smtp_security,
        effective_proxy(account),
        "localhost",
    ) {
        Ok(mut session) => {
            probe.capabilities = session.capabilities().to_vec();
            probe.stage = "auth".into();
            match session.authenticate_kind(
                &account.login_name(),
                password,
                account.auth_kind.eq_ignore_ascii_case("oauth2"),
            ) {
                Ok(()) => {
                    probe.ok = true;
                    probe.stage = "done".into();
                    probe.detail = "SMTP 连接与认证正常".into();
                }
                Err(e) => probe.detail = e,
            }
            session.quit();
        }
        Err(e) => probe.detail = e,
    }
    probe
}

// ---------------------------------------------------------------------------
// 文件夹
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_list_folders(app: AppHandle, account_id: String) -> Result<Vec<MailFolder>, String> {
    ensure_init(&app)?;
    store::list_folders(&account_id)
}

/// 从服务器重新发现文件夹（新增 / 删除的文件夹、未读数）。
#[command]
pub async fn mail_refresh_folders(
    app: AppHandle,
    account_id: String,
) -> Result<Vec<MailFolder>, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let folder_list = refresh_folders_inner(&account).await?;
    Ok(folder_list)
}

/// 未读汇总（左侧账户条）。
#[command]
pub async fn mail_unread_counts(app: AppHandle) -> Result<Vec<(String, u32)>, String> {
    ensure_init(&app)?;
    store::unread_counts()
}

#[command]
pub async fn mail_create_folder(
    app: AppHandle,
    account_id: String,
    name: String,
) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("文件夹名不能为空".to_string());
    }

    create_folder_for_account(&account, &name).await
}

/// 真正创建一个文件夹：POP3 建在本地，其余在服务器上建完再回写本地表。
///
/// 用户手动「新建文件夹」和「规则目标文件夹自动补齐」共用这一条路径，
/// 免得两处对「服务器上已存在」的判断不一致。
async fn create_folder_for_account(account: &Account, name: &str) -> Result<(), String> {
    // POP3 只有一个服务器邮箱，但用户仍然需要在自己这边分门别类：
    // 建一个纯本地的文件夹即可，不碰服务器。
    if pop3_sync::is_pop3(account) {
        note_local_folder(&account.id, name)?;
        return Ok(());
    }
    if graph::is_graph(account) {
        let acc = account.clone();
        let folder_name = name.to_string();
        tauri::async_runtime::spawn_blocking(move || graph::create_folder(&acc, &folder_name))
            .await
            .map_err(|e| format!("创建文件夹任务失败: {e}"))??;
        // 重新发现一次，拿到 Graph 分配的 folder id；前端随后会刷新侧栏。
        let _ = refresh_folders_inner(account).await;
        return Ok(());
    }
    if ews::is_ews(account) {
        let acc = account.clone();
        let folder_name = name.to_string();
        tauri::async_runtime::spawn_blocking(move || ews::create_folder(&acc, &folder_name))
            .await
            .map_err(|e| format!("创建文件夹任务失败: {e}"))??;
        let _ = refresh_folders_inner(account).await;
        return Ok(());
    }
    let password = account_password(account)?;
    let (acc, folder_name) = (account.clone(), name.to_string());
    let created = tauri::async_runtime::spawn_blocking(move || {
        imap::create_folder(&acc, &password, &folder_name)
    })
    .await
    .map_err(|e| format!("创建文件夹任务失败: {e}"))?;
    if let Err(error) = created {
        // 服务器上已经有了、只是本地表还没刷新：重新发现一次，有就当成功，
        // 而不是在「文件夹已存在」上把规则的移动一起卡死。
        let list = refresh_folders_inner(account).await?;
        if list.iter().any(|folder| folder.name == name) {
            note_local_folder(&account.id, name)?;
            return Ok(());
        }
        return Err(error);
    }
    // 服务器上建好了，本地表也要立刻能看到；并标记成「用户自己建的」，
    // 否则下一次文件夹刷新时它会被当成服务商遗留目录从侧栏里藏起来。
    note_local_folder(&account.id, name)?;
    Ok(())
}

/// 规则目标文件夹可能在账户被删 / 重建后不存在了（POP3 的文件夹是纯本地的，
/// 删账户时一并清空；IMAP 上也可能被别处删掉）。执行规则前先把它补出来，
/// 否则用户只会看到「规则命中了，但邮件没进那个文件夹」。
async fn ensure_rule_folder(account: &Account, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(());
    }
    if store::list_folders(&account.id)?
        .iter()
        .any(|folder| folder.name == name)
    {
        // 规则明确要用它：即使这是服务商带下来的目录，也要在侧栏里保留，
        // 否则「规则把它当目标」和「侧栏看不到它」会同时成立。
        store::set_folder_user_created(&account.id, name, true)?;
        return Ok(());
    }
    create_folder_for_account(account, name).await
}

/// 保存账户时，把它名下规则引用的目标文件夹补齐。
///
/// 删除账户再重新添加时，邮件和文件夹都会被清掉，但规则还在；没有这一步，
/// 用户要等下一封新邮件命中规则才发现「文件夹没回来」。
async fn ensure_rule_folders(app: &AppHandle, account: &Account) {
    // 设备码授权还没完成（保存账户的那一刻）：没有令牌，连服务器只会得到
    // 一串「未授权」，还会把保存动作拖慢好几十秒。等授权成功后由同步补建。
    if account.auth_kind.eq_ignore_ascii_case("oauth2") && !oauth::has_tokens(&account.id) {
        return;
    }
    let Ok(rules) = store::list_rules() else {
        return;
    };
    let mut targets: Vec<String> = rules
        .iter()
        .filter(|rule| rule.enabled)
        .filter(|rule| {
            rule.condition.account_ids.is_empty()
                || rule
                    .condition
                    .account_ids
                    .iter()
                    .any(|id| id == &account.id)
        })
        .flat_map(|rule| [rule.action.move_to.clone(), rule.action.copy_to.clone()])
        .flatten()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    targets.sort();
    targets.dedup();
    let existing = store::list_folders(&account.id).unwrap_or_default();
    let mut created = false;
    for target in &targets {
        let already = existing.iter().any(|folder| folder.name == *target);
        if let Err(error) = ensure_rule_folder(account, target).await {
            warn!(
                account = %account.email,
                folder = %target,
                error = %error,
                "mail: 规则目标文件夹补建失败"
            );
        } else if !already {
            created = true;
        }
    }
    if created {
        // 侧栏是按本地文件夹表渲染的：建完要让它刷新，否则用户得等下一次同步
        // 才看见文件夹「回来了」。
        let _ = app.emit(
            MAIL_SYNC_EVENT,
            serde_json::json!({ "accountId": account.id, "foldersChanged": true }),
        );
    }
}

/// 本地新建文件夹（POP3 直接建在本地；IMAP 在服务器创建成功后补一条本地记录）。
fn note_local_folder(account_id: &str, name: &str) -> Result<(), String> {
    if store::list_folders(account_id)?
        .iter()
        .any(|f| f.name == name)
    {
        // 服务器早就有这个名字（同步带下来过、或者用户刚在别的客户端建的）：
        // 光「已存在」不够，要把它标成用户的，否则用户在这边建完侧栏里看不到。
        store::set_folder_user_created(account_id, name, true)?;
        return Ok(());
    }
    store::insert_folder_row(&MailFolder {
        account_id: account_id.to_string(),
        name: name.to_string(),
        display_name: name.to_string(),
        delimiter: None,
        role: FolderRole::Custom,
        unread: 0,
        total: 0,
        uid_validity: 0,
        uid_next: 0,
        selectable: true,
        local_only: pop3_sync::is_pop3(&load_account(account_id)?),
        user_created: true,
    })
}

/// 把文件夹标记为「用户自己建的 / 保留在侧栏」（或取消）。
///
/// 服务器同步会把 QQ / 163 自带的模板目录一起带下来；用户想留着某个的话，
/// 右键点一下就能让它常驻侧栏，而不必去服务器上改结构。
#[command]
pub async fn mail_set_folder_pinned(
    app: AppHandle,
    account_id: String,
    name: String,
    pinned: bool,
) -> Result<(), String> {
    ensure_init(&app)?;
    store::set_folder_user_created(&account_id, &name, pinned)
}

/// 删除自建文件夹。角色文件夹（收件箱/已发送/草稿/垃圾箱/归档）一律拒绝——
/// 删掉它们通常意味着连同邮件一起消失，不该由一个右键菜单触发。
#[command]
pub async fn mail_delete_folder(
    app: AppHandle,
    account_id: String,
    name: String,
) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let folders = store::list_folders(&account_id)?;
    let folder = folders
        .iter()
        .find(|f| f.name == name)
        .ok_or_else(|| "文件夹不存在".to_string())?;
    if folder.role != FolderRole::Custom {
        return Err(format!(
            "「{}」是系统文件夹，不能删除",
            folder.display_name
        ));
    }
    if pop3_sync::is_pop3(&account) {
        // POP3 只有本地文件夹：删掉它以及里面的邮件，不碰服务器。
        store::delete_folder_row(&account_id, &name)?;
        return Ok(());
    }
    if graph::is_graph(&account) {
        let acc = account.clone();
        let folder_id = name.clone();
        tauri::async_runtime::spawn_blocking(move || graph::delete_folder(&acc, &folder_id))
            .await
            .map_err(|e| format!("删除文件夹任务失败: {e}"))??;
        store::delete_folder_row(&account_id, &name)?;
        return Ok(());
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let folder_id = name.clone();
        tauri::async_runtime::spawn_blocking(move || ews::delete_folder(&acc, &folder_id))
            .await
            .map_err(|e| format!("删除文件夹任务失败: {e}"))??;
        store::delete_folder_row(&account_id, &name)?;
        return Ok(());
    }
    let password = account_password(&account)?;
    let (acc, folder_name) = (account.clone(), name.clone());
    tauri::async_runtime::spawn_blocking(move || {
        imap::delete_folder(&acc, &password, &folder_name)
    })
    .await
    .map_err(|e| format!("删除文件夹任务失败: {e}"))??;
    store::delete_folder_row(&account_id, &name)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 同步
// ---------------------------------------------------------------------------

/// 同步一个文件夹。返回本次新增的邮件（前端据此更新列表）。
///
/// 整段在账户级互斥锁内执行：手动刷新、定时轮询、规则动作、发信后的草稿同步
/// 四个触发源都会走到这里，没有锁时同一批 UID 会被并行拉取两遍。
#[command]
pub async fn mail_sync_folder(
    app: AppHandle,
    account_id: String,
    folder: String,
) -> Result<Vec<MessageSummary>, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let items = sync_folder_guarded(&app, &account, &folder).await?;
    let _ = app.emit(
        MAIL_SYNC_EVENT,
        serde_json::json!({
            "accountId": account_id,
            "folder": folder,
            "added": items.len(),
        }),
    );
    emit_new_mail(&app, &account, &folder, &items);
    Ok(items)
}

/// 列表滚到底部时，向更早的历史补一页。
///
/// 增量同步只向前走（`UID > 本地最大`），第一次同步也只取最新的一批；
/// 没有这个入口，比那批更早的邮件永远到不了本机。前端在「本地已全部显示」
/// 之后调用它，并用 `hasMore` 决定是否还能继续往前翻。
#[command]
pub async fn mail_backfill_folder(
    app: AppHandle,
    account_id: String,
    folder: String,
    page_size: Option<u32>,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    // Graph / EWS / POP3 的拉取范围由各自协议决定：POP3 本来整箱下载，
    // Graph/EWS 另有分页；这里只处理 IMAP 的 UID 回溯。
    if !account.protocol.eq_ignore_ascii_case("imap") {
        return Ok(serde_json::json!({
            "added": 0,
            "hasMore": false,
            "supported": false,
        }));
    }
    let page = page_size.unwrap_or(120).clamp(20, 300);
    let snippet_len = store::load_settings().snippet_len as usize;
    let acc = account.clone();
    let account_key = account_id.clone();
    let folder_name = folder.clone();

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<imap::BackfillResult, String> {
        let _guard = sync::lock_account(&account_key);
        let before_uid = store::min_uid(&account_key, &folder_name)?;
        if before_uid <= 1 {
            return Ok(imap::BackfillResult::default());
        }
        imap::backfill_folder_pooled(
            &acc,
            &folder_name,
            before_uid,
            page,
            snippet_len,
        )
    })
    .await
    .map_err(|e| format!("回溯同步任务失败: {e}"))??;

    let _ = app.emit(
        MAIL_SYNC_EVENT,
        serde_json::json!({
            "accountId": account_id,
            "folder": folder,
            "added": result.added,
            "backfill": true,
        }),
    );
    Ok(serde_json::json!({
        "added": result.added,
        "hasMore": result.has_more,
        "supported": true,
    }))
}

/// 同步单个文件夹的完整流程（加锁、增量、标记比对、规则、通知）。
async fn sync_folder_guarded(
    app: &AppHandle,
    account: &Account,
    folder: &str,
) -> Result<Vec<MessageSummary>, String> {
    let settings = store::load_settings();
    let account_id = account.id.clone();
    let folder_name = folder.to_string();
    let acc = account.clone();
    // 规则在每批邮件落库后立刻执行。
    //
    // 以前要等整箱下载完才跑规则：POP3 首次同步几千封时，用户会看到邮件在收件箱里
    // 停几分钟才慢慢消失。现在同步函数把每批摘要通过进度回调交回来，这里边收边跑；
    // Graph / EWS 没有分批接口，仍然在收尾时统一处理。
    let rules = if settings.rules_enabled {
        store::list_rules().unwrap_or_default()
    } else {
        Vec::new()
    };
    let batched = !graph::is_graph(account) && !ews::is_ews(account);
    let (batch_tx, mut batch_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<MessageSummary>>();
    // 每批落库后交给下面「边收边跑规则」的循环；进度事件也放到规则跑完之后
    // 再发，否则列表会先显示这封邮件、过一会儿又被规则移走，看起来像卡住。
    let on_progress = move |batch: &[MessageSummary]| {
        let _ = batch_tx.send(batch.to_vec());
    };

    let sync_task = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<MessageSummary>, String> {
        let _guard = sync::lock_account(&account_id);
        // Microsoft Graph：文件夹、消息、标记都在 Graph JSON 里。
        if graph::is_graph(&acc) {
            return graph::sync_folder(&acc, &folder_name, settings.snippet_len as usize);
        }
        if ews::is_ews(&acc) {
            return ews::sync_folder(&acc, &folder_name, settings.snippet_len as usize);
        }
        // POP3 没有文件夹与服务器端标记：整轮同步就是「把收件箱里没见过的取下来」。
        // 已发送 / 草稿 / 删除都是本地文件夹，没有可同步的东西，直接返回空列表
        // （否则在「已发送」里点刷新会收到收件箱的行，计数与列表对不上）。
        if pop3_sync::is_pop3(&acc) {
            if folder_name != pop3_sync::INBOX {
                return Ok(Vec::new());
            }
            return pop3_sync::sync_inbox(
                &acc,
                settings.snippet_len as usize,
                Some(&on_progress),
            );
        }
        // 纯本地文件夹（发件箱）在服务器上不存在：去 SELECT 只会每轮报错。
        if store::list_folders(&account_id)
            .ok()
            .and_then(|list| list.into_iter().find(|f| f.name == folder_name))
            .map(|f| f.local_only)
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }
        let (uid_validity, _) = store::folder_uid_state(&account_id, &folder_name)?;
        let last_uid = store::last_uid(&account_id, &folder_name)?;
        let items = imap::sync_folder_pooled(
            &acc,
            &folder_name,
            uid_validity,
            last_uid,
            settings.snippet_len as usize,
            Some(&on_progress),
        )?;
        // 增量之后按间隔做一次标记全量比对：增量只管「新信」，
        // 而「已读 / 星标 / 被别处删掉」是就地变化，UID 不变。
        //
        // 间隔由 `flag_sync_minutes` 控制（默认 10 分钟）：全量比对要一次
        // `UID FETCH (FLAGS)`，几万封也要几百 KB，每 90 秒跑一次没必要。
        let interval_secs = (settings.flag_sync_minutes.max(1) as i64) * 60;
        let last_flags = store::folder_flags_synced_at(&account_id, &folder_name);
        let now = chrono::Utc::now().timestamp();
        let due = last_flags == 0 || now - last_flags >= interval_secs;
        if due {
            match imap::sync_flags_pooled(&acc, &folder_name) {
                Ok(result) => {
                    if result.updated > 0 || result.removed > 0 {
                        info!(
                            folder = %folder_name,
                            updated = result.updated,
                            removed = result.removed,
                            "mail: 多设备标记已同步"
                        );
                    }
                    store::touch_folder_flags_synced(&account_id, &folder_name, now);
                }
                Err(e) => warn!(error = %e, "mail: 标记比对失败（不影响增量结果）"),
            }
        }
        Ok(items)
    });

    // 边下载边处理已落库的批次：规则移动邮件与继续下载互不等待。
    if batched {
        let rule_account = account.clone();
        while let Some(batch) = batch_rx.recv().await {
            if !batch.is_empty() && !rules.is_empty() {
                apply_rules_to_new(&rule_account, &batch, &rules).await;
            }
            // 规则跑完再刷新列表：这一批里该移走的已经不在了，不会先闪一下再消失。
            let _ = app.emit(
                MAIL_SYNC_EVENT,
                serde_json::json!({
                    "accountId": account.id,
                    "folder": folder,
                    "added": batch.len(),
                    "partial": true,
                }),
            );
        }
    }

    let items = sync_task
        .await
        .map_err(|e| format!("同步任务失败: {e}"))??;

    store::touch_account_sync(&account.id, chrono::Utc::now().timestamp())?;
    collect_contacts(account, &items);

    // Graph / EWS 没有分批回调：整批同步完再跑一次规则。
    if !batched && !rules.is_empty() && !items.is_empty() {
        apply_rules_to_new(account, &items, &rules).await;
    }

    // 兜底补分类：同步中断 / 某批移动失败时，收件箱里会留下命中规则的邮件。
    // 放在同步收尾时做，此时数据库写入已经结束，不会再和下载批次抢写锁。
    let inbox_synced = store::list_folders(&account.id)
        .ok()
        .and_then(|list| list.into_iter().find(|meta| meta.name == folder))
        .map(|meta| meta.role == FolderRole::Inbox)
        .unwrap_or(false);
    if inbox_synced && settings.rules_enabled {
        reclassify_inbox_moves(account).await;
    }
    Ok(items)
}

/// 新邮件通知：只在「确实有新邮件」且设置开启时发。
fn emit_new_mail(app: &AppHandle, account: &Account, folder: &str, items: &[MessageSummary]) {
    if items.is_empty() {
        return;
    }
    let _ = app.emit(MAIL_NEW_EVENT, items);
    let settings = store::load_settings();
    if !settings.notify_new_mail {
        return;
    }
    // 只对收件箱（以及没有角色信息的默认收件箱）提示：自己发出去的、
    // 后台同步草稿箱 / 已发送的，不该让系统弹「新邮件」。
    let role = store::list_folders(&account.id)
        .ok()
        .and_then(|list| list.into_iter().find(|f| f.name == folder))
        .map(|f| f.role);
    if !matches!(role, Some(FolderRole::Inbox) | None) {
        return;
    }
    let notices: Vec<model::NewMailNotice> = items
        .iter()
        .filter(|m| !m.outgoing)
        .take(5)
        .map(|m| model::NewMailNotice {
            account_id: m.account_id.clone(),
            account_email: account.email.clone(),
            folder: m.folder.clone(),
            uid: m.uid,
            from_name: m.from_name.clone(),
            from_addr: m.from_addr.clone(),
            subject: m.subject.clone(),
        })
        .collect();
    notify::notify_new_mail(app, &notices);
}


/// 同步完一批邮件后，把发件人 / 收件人自动收录进通讯录。
///
/// 放在同步返回之后、规则执行之前：规则可能把邮件移走或删掉，之后再收录就找不到它们了。
/// 收录失败不影响同步结果（通讯录是附属功能，不该让收信失败）。
fn collect_contacts(account: &Account, items: &[MessageSummary]) {
    if items.is_empty() {
        return;
    }
    let own = vec![account.email.clone(), account.login_name()];
    let mut entries: Vec<(String, String, i64)> = Vec::new();
    for message in items {
        // 自己发出去的邮件也要收「收件人」：邮件组 / 分发列表地址往往只出现在
        // 发出的邮件里。发件人那一条会被 own 过滤掉，不会把用户自己写进通讯录。
        if !message.from_addr.trim().is_empty() {
            entries.push((
                message.from_name.clone(),
                message.from_addr.clone(),
                message.date,
            ));
        }
        for item in recipients_of(&message.to_json) {
            entries.push((item.label, item.value, message.date));
        }
        for item in recipients_of(&message.cc_json) {
            entries.push((item.label, item.value, message.date));
        }
    }
    // 整批一次写库：一封一个连接会让同步在通讯录上白花一截时间。
    if let Err(e) = contacts::store::touch_addresses(&own, &entries) {
        warn!(error = %e, "mail: 自动收录联系人失败");
    }
}

/// `to_json` / `cc_json` 存的是 `[{"name","address"}]`（解析 MIME 时的 MailAddress），
/// 不是通讯录的 `{label,value}`。以前按 LabeledValue 反序列化必然失败，收件人一个都
/// 收不进来——邮件组地址永远进不了通讯录就是这么来的（真实反馈）。
fn recipients_of(json: &str) -> Vec<contacts::model::LabeledValue> {
    serde_json::from_str::<Vec<MailAddress>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !item.address.trim().is_empty())
        .map(|item| contacts::model::LabeledValue {
            label: item.name,
            value: item.address,
        })
        .collect()
}

/// 对新到邮件跑规则。只有「标读 / 星标 / 打标签」是本地即时生效的；
/// 移动、删除、转发需要服务器往返，失败时只记录日志，不阻塞同步。
async fn apply_rules_to_new(
    account: &Account,
    messages: &[MessageSummary],
    all_rules: &[MailRule],
) {
    let folders = store::list_folders(&account.id).unwrap_or_default();
    let trash = folder_by_role(&account.id, FolderRole::Trash).map(|f| f.name);
    // 收/发件人名单比规则优先：命中的发件人直接按名单处理，不再跑规则。
    let sender_rules = store::list_sender_rules().unwrap_or_default();
    // 没有一条规则要看正文时，整个循环都不读正文：正文是密文，读一次就解一次，
    // 1000 封新邮件 = 1000 次 AES 解密，而绝大多数规则只看发件人 / 主题。
    let body_needed = rules::needs_body(all_rules);

    // 先把每封邮件要做的动作算出来，再按「源夹 → 目标夹」合并成批量操作。
    // 以前每封命中都单独连一次服务器、单独重算两次文件夹计数：几百封就要跑很久，
    // 界面上看到的就是「正在对已有邮件执行…」一直不动（真实反馈）。
    let mut moves: HashMap<(String, String), Vec<u32>> = HashMap::new();
    let mut flag_batches: HashMap<(String, bool, bool), Vec<u32>> = HashMap::new();
    let mut forwards: Vec<(usize, Vec<String>)> = Vec::new();
    // 本地标记改动累积到循环末尾一次写：逐封写就是逐封开一次 4.9GB 的库，
    // 实测每封 639ms（几千封＝十几分钟），同一个事务里写整批只要零点几秒。
    let mut flag_changes: Vec<store::FlagChange> = Vec::new();

    for (index, msg) in messages.iter().enumerate() {
        if msg.outgoing {
            continue;
        }
        let folder_role = msg
            .folder
            .as_str()
            .is_empty()
            .then(|| "custom".to_string())
            .unwrap_or_else(|| {
                folders
                    .iter()
                    .find(|f| f.name == msg.folder)
                    .map(|f| f.role.as_str().to_string())
                    .unwrap_or_else(|| "custom".to_string())
            });
        if let Some(verdict) = rules::sender_list_verdict(&sender_rules, &msg.from_addr, &msg.from_name) {
            match verdict {
                rules::SenderVerdict::Blocked => {
                    if let Some(trash) = trash.as_ref() {
                        moves
                            .entry((msg.folder.clone(), trash.clone()))
                            .or_default()
                            .push(msg.uid);
                    } else {
                        let _ = store::delete_message(&msg.account_id, &msg.folder, msg.uid);
                    }
                    continue;
                }
                rules::SenderVerdict::Allowed => {
                    // 白名单：跳过后续规则，但仍要标为已读（用户明确在意的人）。
                    continue;
                }
            }
        }
        // 先用不需要正文的条件过一遍：命不中的邮件就不必解密正文
        // （「对已有邮件执行」动辄几千封，这一步能把解密量降一个数量级）。
        let cheap_ctx = rules::RuleContext {
            message: msg,
            body_preview: "",
            account,
            folder_role: &folder_role,
        };
        let body = if body_needed && rules::body_relevant(all_rules, &cheap_ctx) {
            // 规则只看纯文本：只读、只解密这一列，别把 HTML 与附件清单也拖进来。
            store::get_body_text(&msg.account_id, &msg.folder, msg.uid).unwrap_or_default()
        } else {
            String::new()
        };
        let ctx = rules::RuleContext {
            message: msg,
            body_preview: &body,
            account,
            folder_role: &folder_role,
        };
        let hits = rules::evaluate(all_rules, &ctx);
        if hits.is_empty() {
            continue;
        }
        let actions: Vec<_> = hits.iter().map(|(_, a)| a.clone()).collect();
        let merged = rules::merge_actions(&actions);

        let labels =
            rules::apply_label_delta(&msg.labels, &merged.add_labels, &merged.remove_labels);
        flag_changes.push(store::FlagChange {
            account_id: msg.account_id.clone(),
            folder: msg.folder.clone(),
            uid: msg.uid,
            seen: if merged.mark_read { Some(true) } else { None },
            flagged: if merged.star { Some(true) } else { None },
            labels: Some(labels),
        });

        if merged.mark_read || merged.star {
            flag_batches
                .entry((msg.folder.clone(), merged.mark_read, merged.star))
                .or_default()
                .push(msg.uid);
        }

        // 移动 / 删除：先记下来，循环结束后按文件夹一次性搬。
        let move_target = merged.move_to.clone().or_else(|| {
            if merged.delete {
                trash.clone()
            } else {
                None
            }
        });
        if let Some(target) = move_target {
            moves
                .entry((msg.folder.clone(), target))
                .or_default()
                .push(msg.uid);
        }

        if !merged.forward_to.is_empty() {
            forwards.push((index, merged.forward_to));
        }
    }

    // 本地标记（已读 / 星标 / 标签）整批一次写完：逐封写就是逐封开一次库。
    if let Err(e) = store::apply_flag_changes(&flag_changes) {
        warn!(error = %e, count = flag_changes.len(), "mail: 写入规则结果失败");
    }

    // 转发要在搬运之前：它读的是原文件夹里的原文，邮件被搬走之后就取不到了。
    for (index, recipients) in forwards {
        forward_by_rule(&account.clone(), &messages[index], &recipients).await;
    }
    for ((folder, target), uids) in moves {
        apply_rule_move_batch(account, &folder, &uids, &target).await;
    }
    // 服务器上的已读 / 星标同样按文件夹合并：一次连接改完。
    if graph::is_graph(account) || ews::is_ews(account) {
        let native_ews = ews::is_ews(account);
        for ((folder, seen, flagged), uids) in flag_batches {
            let acc = account.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if native_ews {
                    ews::set_flags(
                        &acc,
                        &folder,
                        &uids,
                        if seen { Some(true) } else { None },
                        if flagged { Some(true) } else { None },
                    )
                } else {
                    graph::set_flags(
                        &acc,
                        &folder,
                        &uids,
                        if seen { Some(true) } else { None },
                        if flagged { Some(true) } else { None },
                    )
                }
            })
            .await;
        }
    } else if !pop3_sync::is_pop3(account) {
        for ((folder, seen, flagged), uids) in flag_batches {
            let acc = account.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                imap::set_flags(
                    &acc,
                    "",
                    &folder,
                    &uids,
                    if seen { Some(true) } else { None },
                    if flagged { Some(true) } else { None },
                )
            })
            .await;
        }
    }
}

/// 规则 / 名单触发的移动：整批搬完再删本地行，失败只记日志（不阻塞同步）。
async fn apply_rule_move_batch(account: &Account, folder: &str, uids: &[u32], target: &str) {
    if uids.is_empty() || folder == target {
        return;
    }
    // 目标文件夹可能已经被删掉（或账户重建后从没建回来）：先补建，再搬。
    if let Err(error) = ensure_rule_folder(account, target).await {
        error!(
            error = %error,
            folder = %target,
            count = uids.len(),
            "mail: 规则目标文件夹不存在且无法创建，跳过本批移动"
        );
        return;
    }
    // POP3 的文件夹全在本机：走本地搬行，别去连 IMAP（会直接失败，
    // 表现是「规则命中但邮件没动」）。
    if pop3_sync::is_pop3(account) {
        if let Err(e) = move_locally(&account.id, folder, uids, target) {
            error!(
                error = %e,
                folder = %folder,
                target = %target,
                count = uids.len(),
                "mail: 规则移动邮件失败（本地）"
            );
        }
        return;
    }
    if graph::is_graph(account) {
        let acc = account.clone();
        let (f, u, t) = (folder.to_string(), uids.to_vec(), target.to_string());
        match tauri::async_runtime::spawn_blocking(move || graph::move_messages(&acc, &f, &u, &t))
            .await
        {
            Ok(Ok(())) => {
                let _ = store::delete_messages(&account.id, folder, uids);
            }
            Ok(Err(e)) => error!(error = %e, "mail: 规则移动邮件失败（Graph）"),
            Err(e) => error!(error = %e, "mail: 规则移动任务失败（Graph）"),
        }
        return;
    }
    if ews::is_ews(account) {
        let acc = account.clone();
        let (f, u, t) = (folder.to_string(), uids.to_vec(), target.to_string());
        match tauri::async_runtime::spawn_blocking(move || ews::move_messages(&acc, &f, &u, &t))
            .await
        {
            Ok(Ok(())) => {
                let _ = store::delete_messages(&account.id, folder, uids);
            }
            Ok(Err(e)) => error!(error = %e, "mail: 规则移动邮件失败（EWS）"),
            Err(e) => error!(error = %e, "mail: 规则移动任务失败（EWS）"),
        }
        return;
    }
    let acc = account.clone();
    let folder_owned = folder.to_string();
    let target_owned = target.to_string();
    let uids_owned = uids.to_vec();
    let moved = tauri::async_runtime::spawn_blocking(move || {
        imap::move_messages(&acc, "", &folder_owned, &uids_owned, &target_owned)
    })
    .await;
    match moved {
        Ok(Ok(())) => {
            let _ = store::delete_messages(&account.id, folder, uids);
        }
        Ok(Err(e)) => error!(
            error = %e,
            folder = %folder,
            target = %target,
            count = uids.len(),
            "mail: 规则移动邮件失败"
        ),
        Err(e) => error!(error = %e, "mail: 规则移动任务失败"),
    }
}

/// 收件箱补分类：只处理「命中移动规则却还留在收件箱」的邮件。
///
/// 正常路径上规则在每批邮件落库后立刻执行；但同步中途被关掉、网络断开、
/// 或某一批移动撞上数据库锁时，这些邮件会留在收件箱里再也没有机会被处理。
/// 这里在每次收件箱同步结束后扫一遍最近的邮件，只执行「移动 / 移入垃圾箱」，
/// 不重复转发、不重复打标记——避免把用户手动放回收件箱的邮件反复折腾。
async fn reclassify_inbox_moves(account: &Account) {
    let Ok(folders) = store::list_folders(&account.id) else {
        return;
    };
    let Some(inbox) = folders.iter().find(|folder| folder.role == FolderRole::Inbox) else {
        return;
    };
    let rules: Vec<MailRule> = store::list_rules()
        .unwrap_or_default()
        .into_iter()
        .filter(|rule| rule.enabled)
        .collect();
    if rules.is_empty() {
        return;
    }
    let trash = folder_by_role(&account.id, FolderRole::Trash).map(|folder| folder.name);
    let body_needed = rules::needs_body(&rules);
    let rows = store::list_recent(&account.id, &inbox.name, 300, 0).unwrap_or_default();

    let mut moves: HashMap<String, Vec<u32>> = HashMap::new();
    for msg in rows.iter().filter(|msg| !msg.outgoing) {
        let cheap_ctx = rules::RuleContext {
            message: msg,
            body_preview: "",
            account,
            folder_role: "inbox",
        };
        let body = if body_needed && rules::body_relevant(&rules, &cheap_ctx) {
            store::get_body_text(&msg.account_id, &msg.folder, msg.uid).unwrap_or_default()
        } else {
            String::new()
        };
        let ctx = rules::RuleContext {
            message: msg,
            body_preview: &body,
            account,
            folder_role: "inbox",
        };
        let hits = rules::evaluate(&rules, &ctx);
        if hits.is_empty() {
            continue;
        }
        let actions: Vec<_> = hits.iter().map(|(_, action)| action.clone()).collect();
        let merged = rules::merge_actions(&actions);
        let target = merged
            .move_to
            .clone()
            .or_else(|| merged.delete.then(|| trash.clone()).flatten());
        if let Some(target) = target {
            moves.entry(target).or_default().push(msg.uid);
        }
    }

    let mut moved = 0usize;
    for (target, uids) in moves {
        if uids.is_empty() {
            continue;
        }
        apply_rule_move_batch(account, &inbox.name, &uids, &target).await;
        // 移动失败时会留在收件箱，下一轮同步再补；这里只统计真正消失的。
        let alive: std::collections::HashSet<u32> =
            store::list_recent(&account.id, &inbox.name, 300, 0)
                .unwrap_or_default()
                .into_iter()
                .map(|msg| msg.uid)
                .collect();
        moved += uids.iter().filter(|uid| !alive.contains(uid)).count();
    }
    if moved > 0 {
        info!(
            account = %account.email,
            moved,
            "mail: 收件箱补分类完成"
        );
    }
}

async fn forward_by_rule(account: &Account, msg: &MessageSummary, recipients: &[String]) {
    let (acc, folder, uid) = (account.clone(), msg.folder.clone(), msg.uid);
    let recipients = recipients.to_vec();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let graph_mode = graph::is_graph(&acc);
        let ews_mode = ews::is_ews(&acc);
        let raw = if graph_mode || ews_mode {
            let remote_id = store::remote_id_for(&acc.id, &folder, uid)
                .ok_or_else(|| "规则转发：这封邮件没有远端 ID".to_string())?;
            if ews_mode {
                ews::fetch_mime(&acc, &remote_id)?
            } else {
                graph::fetch_mime(&acc, &remote_id)?
            }
        } else {
            imap::fetch_raw(&acc, "", &folder, uid)?
        };
        let parsed = mime::parse(&raw, false).ok_or("无法解析原邮件")?;
        let request = SendRequest {
            account_id: acc.id.clone(),
            to: recipients.clone(),
            cc: vec![],
            bcc: vec![],
            subject: mime::forward_subject(&parsed.subject),
            text: parsed.text.clone(),
            html: parsed.html.clone(),
            in_reply_to: None,
            references: None,
            attachments: vec![],
            save_to_sent: Some(false),
            inline_images: Vec::new(),
            crypto_mode: String::new(),
        };
        let from = MailAddress {
            name: acc.display_name.clone(),
            address: acc.email.clone(),
        };
        let bytes = mime::build(&request, &from, "")?;
        if graph_mode {
            graph::send_mime(&acc, &bytes)
        } else if ews_mode {
            let drafts = folder_name_by_role(&acc.id, FolderRole::Drafts)?;
            let sent = folder_name_by_role(&acc.id, FolderRole::Sent)?;
            ews::send_mime(&acc, &drafts, &sent, &bytes)
        } else {
            let secret = imap::auth_secret(&acc)?;
            smtp::send_raw(&acc, &secret, effective_proxy(&acc), &acc.email, &recipients, &bytes)
        }
    })
    .await;
    match result {
        Ok(Err(e)) => error!(error = %e, "mail: 规则转发失败"),
        Err(e) => error!(error = %e, "mail: 规则转发任务失败"),
        _ => {}
    }
}

/// 同步所有账户的收件箱（定时轮询 / 手动刷新）。
#[command]
pub async fn mail_sync_all(app: AppHandle) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let settings = store::load_settings();
    if settings.retention_days > 0 {
        // 本地缓存过期清理：只删本地行，服务器上的邮件不动；星标邮件保留。
        let cutoff = chrono::Utc::now().timestamp() - (settings.retention_days as i64) * 86_400;
        if let Err(e) = store::purge_older_than(cutoff) {
            warn!(error = %e, "mail: 清理过期缓存失败");
        }
    }
    let accounts = store::list_accounts()?;
    let mut total = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for account in accounts {
        let folders = match store::list_folders(&account.id) {
            // Graph 账户保存时会先补一个本地「发件箱」占位，因此「表非空」
            // 不代表文件夹已经发现；没有收件箱就重新从 Graph 拉一次。
            Ok(f)
                if !f.is_empty()
                    && !((graph::is_graph(&account) || ews::is_ews(&account))
                        && !f.iter().any(|folder| folder.role == FolderRole::Inbox)) =>
            {
                f
            }
            _ => match refresh_folders_inner(&account).await {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!("{}: {e}", account.email));
                    continue;
                }
            },
        };
        // 后台轮询只同步收件箱：已发送 / 草稿等文件夹的变化频率低，
        // 每 90 秒把所有文件夹都拉一遍会让服务器负担与移动端耗电都变得不合理。
        // 用户正在浏览的文件夹由前端单独调 `mail_sync_folder` 补。
        for folder in folders
            .iter()
            .filter(|f| f.role == FolderRole::Inbox && f.selectable)
        {
            match sync_one(&app, &account, &folder.name).await {
                Ok(n) => total += n,
                Err(e) => {
                    // 网络类错误在轮询里很常见（笔记本合盖、切 Wi-Fi），
                    // 不该把用户从「安静」模式里吵醒；只在日志留痕。
                    warn!(
                        account = %account.email,
                        folder = %folder.display_name,
                        error = %e,
                        "mail: 定时同步失败"
                    );
                    errors.push(format!("{} / {}: {e}", account.email, folder.display_name));
                }
            }
        }
    }
    Ok(serde_json::json!({ "added": total, "errors": errors }))
}

async fn refresh_folders_inner(account: &Account) -> Result<Vec<MailFolder>, String> {
    // POP3 的文件夹是固定的本地列表，不需要也不会去服务器「发现」。
    if pop3_sync::is_pop3(account) {
        let acc = account.clone();
        return tauri::async_runtime::spawn_blocking(move || pop3_sync::ensure_folders(&acc))
            .await
            .map_err(|e| format!("刷新文件夹任务失败: {e}"))?;
    }
    if graph::is_graph(account) {
        let acc = account.clone();
        let folders = tauri::async_runtime::spawn_blocking(move || graph::list_folders(&acc))
            .await
            .map_err(|e| format!("刷新文件夹任务失败: {e}"))??;
        store::replace_folders(&account.id, &folders)?;
        return store::list_folders(&account.id);
    }
    if ews::is_ews(account) {
        let acc = account.clone();
        let folders = tauri::async_runtime::spawn_blocking(move || ews::list_folders(&acc))
            .await
            .map_err(|e| format!("刷新文件夹任务失败: {e}"))??;
        store::replace_folders(&account.id, &folders)?;
        return store::list_folders(&account.id);
    }
    let password = account_password(account)?;
    let acc = account.clone();
    let folders = tauri::async_runtime::spawn_blocking(move || imap::list_folders(&acc, &password))
        .await
        .map_err(|e| format!("刷新文件夹任务失败: {e}"))??;
    store::replace_folders(&account.id, &folders)?;
    // 返回落库后的完整表，而不是服务器这一次的 LIST 结果：`replace_folders` 会保留
    // 服务器没返回的自建文件夹，只回服务器那份的话，前端每次刷新都会把它们抹掉。
    store::list_folders(&account.id)
}

async fn sync_one(app: &AppHandle, account: &Account, folder: &str) -> Result<usize, String> {
    let items = sync_folder_guarded(app, account, folder).await?;
    Ok(items.len())
}

// ---------------------------------------------------------------------------
// 读取
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_list_messages(
    app: AppHandle,
    account_id: String,
    folder: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<MessageSummary>, String> {
    ensure_init(&app)?;
    store::list_recent(
        &account_id,
        &folder,
        limit.unwrap_or(100),
        offset.unwrap_or(0),
    )
}

#[command]
pub async fn mail_search(app: AppHandle, query: SearchQuery) -> Result<Vec<MessageSummary>, String> {
    ensure_init(&app)?;
    store::search(&query)
}

#[command]
pub async fn mail_suggest(app: AppHandle, prefix: String) -> Result<Vec<String>, String> {
    ensure_init(&app)?;
    store::suggest(&prefix, 12)
}

#[command]
pub async fn mail_labels(app: AppHandle) -> Result<Vec<(String, u32)>, String> {
    ensure_init(&app)?;
    store::labels()
}

#[command]
pub async fn mail_thread(app: AppHandle, thread_key: String) -> Result<Vec<MessageSummary>, String> {
    ensure_init(&app)?;
    store::thread(&thread_key)
}

/// 打开一封邮件：本地有正文就直接返回；没有则去服务器补拉并入库。
#[command]
pub async fn mail_get_message(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    // 跳过正文缓存、重新取原始报文。续写草稿时要读自定义头（组芯片），
    // 而缓存路径不带完整头部。
    force: Option<bool>,
) -> Result<MessageDetail, String> {
    ensure_init(&app)?;
    // 摘要 / 正文 / 渲染版本一次连接读完：打开 4.9GB 的库约 50ms，而点查本身是
    // 微秒级。分成四五次 open 的话，一封邮件光在打开库上就要两百毫秒。
    let records = store::open_records(&account_id, &folder, uid)?;
    let mut summary = records
        .summary
        .ok_or_else(|| "本地没有这封邮件的记录".to_string())?;
    let render_version = records.render_version;

    let cached = if force.unwrap_or(false) {
        None
    } else {
        records.body
    };
    let (cached_html, cached_text, cached_attachments, fetched) =
        cached.unwrap_or((None, String::new(), "[]".into(), false));

    let mut detail = store::base_detail(&summary);
    // 老缓存里可能还留着 `cid:` 引用（旧版没有把内联图转成 data URI）：
    // 重新取一次原始报文并解析，顺带把缓存刷新成可显示的内联图。
    let cached_needs_inline = cached_html
        .as_deref()
        .map(|html| html.to_ascii_lowercase().contains("cid:"))
        .unwrap_or(false);
    // 表格排版的清洗规则变过：旧的缓存正文里 border / cellpadding / bgcolor
    // 已经被剥掉，重新打开时要按原文重新解析一次才能恢复。只对含 <table> 的
    // 旧缓存触发，避免所有邮件都重新下载一遍。
    let stale_table_render = render_version < store::TABLE_RENDER_VERSION
        && cached_html
            .as_deref()
            .map(|html| html.to_ascii_lowercase().contains("<table"))
            .unwrap_or(false);
    // 附件的判定也改过：旧缓存会把正文里以 `cid:` 引用的内联图列成附件。
    // 这类缓存按版本号重新解析一次；重解析后版本号更新，只会发生一次。
    let stale_inline_attachments = render_version < store::RENDER_VERSION
        && cached_html.is_some()
        && serde_json::from_str::<Vec<model::AttachmentMeta>>(&cached_attachments)
            .map(|list| {
                list.iter().any(|a| {
                    a.content_id
                        .as_deref()
                        .is_some_and(|cid| !cid.trim().is_empty())
                        && a.content_type.to_ascii_lowercase().starts_with("image/")
                })
            })
            .unwrap_or(false);
    if fetched
        && !cached_needs_inline
        && !stale_table_render
        && !stale_inline_attachments
        && (!cached_text.is_empty() || cached_html.is_some())
    {
        detail.html = cached_html.map(|html| {
            // 旧缓存里可能残留邮件自带的 <style>（深色 media query 会让正文变黑）。
            // 只对这类正文重洗一次，干净的缓存原样返回，避免每次打开都多解析一遍。
            if html.to_ascii_lowercase().contains("<style") {
                sanitize::resanitize_cached(&html)
            } else {
                html
            }
        });
        detail.text = cached_text;
        detail.attachments = serde_json::from_str(&cached_attachments).unwrap_or_default();
    } else {
        let account = load_account(&account_id)?;
        let password = if graph::is_graph(&account) {
            String::new()
        } else {
            account_password(&account)?
        };
        let settings = store::load_settings();
        let raw = match fetch_raw_bytes(&account, &password, &folder, uid).await {
            Ok(raw) => raw,
            Err(error) => {
                // 离线 / 原邮件已被服务器删除时，旧缓存仍然是可读的：
                // 宁可继续显示旧版渲染，也不要把「查看邮件」直接变成错误。
                if fetched && (!cached_text.is_empty() || cached_html.is_some()) {
                    detail.html = cached_html;
                    detail.text = cached_text;
                    detail.attachments =
                        serde_json::from_str(&cached_attachments).unwrap_or_default();
                    if !summary.seen && mark_seen_inner(&account_id, &folder, &[uid], true).await.is_ok()
                    {
                        detail.seen = true;
                    }
                    warn!(error = %error, "mail: 重新解析正文失败，回退旧缓存");
                    return Ok(detail);
                }
                return Err(error);
            }
        };
        // 端到端加密 / 签名：在解析 MIME 之前先解开。
        //
        // 这里只做「能解就解、解不开就说清楚」：拿不到私钥时把原始提示
        // 放进正文，而不是让用户对着一个 `application/pkcs7-mime` 附件发呆。
        let mut raw = raw;
        let mut crypto_note: Option<String> = None;
        if let Some(content_type) = mime::top_level_content_type(&raw) {
            let lower = content_type.to_ascii_lowercase();
            if lower.contains("application/pkcs7-mime") || lower.contains("application/x-pkcs7-mime") {
                let acc = account.clone();
                let body = String::from_utf8_lossy(&raw).to_string();
                match tauri::async_runtime::spawn_blocking(move || smime::decrypt(&acc.id, &body))
                    .await
                    .map_err(|e| format!("解密任务失败: {e}"))?
                {
                    Ok(plain) => raw = plain,
                    Err(e) => crypto_note = Some(e),
                }
            } else if lower.contains("application/pgp-encrypted") {
                let dir = pgp::key_dir(&app)?;
                let bytes = raw.clone();
                match tauri::async_runtime::spawn_blocking(move || pgp::decrypt(&dir, &bytes, ""))
                    .await
                    .map_err(|e| format!("PGP 解密任务失败: {e}"))?
                {
                    Ok(plain) => raw = plain,
                    Err(e) => crypto_note = Some(e),
                }
            } else if lower.contains("multipart/signed") && lower.contains("pgp") {
                let dir = pgp::key_dir(&app)?;
                let bytes = raw.clone();
                if let Ok(result) = tauri::async_runtime::spawn_blocking(move || {
                    pgp::verify(&dir, &bytes)
                })
                .await
                .unwrap_or(Ok(pgp::PgpVerifyResult {
                    signed: false,
                    valid: false,
                    key_id: String::new(),
                    detail: String::new(),
                    body: Vec::new(),
                }))
                {
                    if !result.body.is_empty() {
                        raw = result.body;
                    }
                    crypto_note = Some(if result.valid {
                        format!("✓ PGP {}", result.detail)
                    } else {
                        format!("⚠ {}", result.detail)
                    });
                }
            }
        }
        let parsed = mime::parse(&raw, !settings.inline_images).ok_or("无法解析邮件内容")?;
        let attachments_json =
            serde_json::to_string(&parsed.attachments).unwrap_or_else(|_| "[]".into());
        store::upsert_body(
            &account_id,
            &folder,
            uid,
            parsed.html.as_deref(),
            &parsed.text,
            &attachments_json,
        )?;
        summary.message_id = summary.message_id.or(parsed.message_id.clone());
        // 这一份是完整报文解析的结果：直接覆盖，而不是只在为真时保留。
        // 旧库把正文里的内联图记成了附件，`||` 会让回形针永远留在那一行上。
        summary.has_attachments = parsed.has_attachments;
        // 顺手把引用链补进摘要：老库里的邮件是这套字段上线前入库的，
        // 只要用户打开过一封，它就能凭 References 找回自己的会话，
        // 历史上被主题拆开的几段会话也会在这一刻并回去。
        summary.in_reply_to = parsed.in_reply_to.clone();
        summary.references = parsed.references.clone();
        store::upsert_summaries(&[summary.clone()])?;

        detail.html = parsed.html;
        detail.text = parsed.text;
        detail.to = parsed.to;
        detail.cc = parsed.cc;
        detail.bcc = parsed.bcc;
        detail.reply_to = parsed.reply_to;
        detail.in_reply_to = parsed.in_reply_to;
        detail.references = parsed.references;
        detail.attachments = parsed.attachments;
        detail.headers = parsed.headers;
        if detail.date == 0 {
            detail.date = parsed.date;
        }
        if let Some(note) = crypto_note {
            // 加密状态作为一条可见提示追加到正文顶部：
            // 用户必须知道自己看到的是一封「验签通过 / 未通过」的邮件。
            let banner = format!(
                "<div class=\"guaxi-crypto-note\" style=\"padding:6px 10px;margin-bottom:10px;border:1px solid #b8791a;border-radius:6px;font-size:12px;color:#b8791a\">{}</div>",
                escape_html(&note)
            );
            detail.html = Some(format!("{banner}{}", detail.html.unwrap_or_default()));
            if detail.text.is_empty() {
                detail.text = note;
            }
        }
    }

    // 带附件的邮件打开后，后台把原始报文缓存下来。
    //
    // POP3 只能整封 RETR、IMAP 也要整封 FETCH，点附件时再下载会等很久；
    // 趁用户在读正文时先取回，真正点附件时只做本地解包。附件太大（>32MB，
    // 与原文缓存上限一致）就不预取，避免为了点一个附件把整封几十 MB 都拉下来。
    let attachment_bytes: u64 = detail.attachments.iter().map(|att| att.size as u64).sum();
    let has_attachments = summary.has_attachments || !detail.attachments.is_empty();
    if has_attachments
        // 尺寸未知（解析器给 0）时也预取：至少上限交给原文缓存那一层判断。
        && (attachment_bytes == 0 || attachment_bytes <= store::RAW_CACHE_LIMIT as u64)
        && store::cached_raw_body(&account_id, &folder, uid).is_none()
    {
        let acc_id = account_id.clone();
        let folder_name = folder.clone();
        tauri::async_runtime::spawn(async move {
            let Ok(account) = load_account(&acc_id) else {
                return;
            };
            let password = if graph::is_graph(&account) {
                String::new()
            } else {
                match account_password(&account) {
                    Ok(value) => value,
                    Err(_) => return,
                }
            };
            if let Err(error) = fetch_raw_bytes(&account, &password, &folder_name, uid).await {
                debug!(error = %error, "mail: 预取附件原文失败");
            }
        });
    }

    // 打开即已读（与主流客户端一致）。失败不影响阅读。
    if !summary.seen && mark_seen_inner(&account_id, &folder, &[uid], true).await.is_ok() {
        detail.seen = true;
    }
    Ok(detail)
}

/// 一条会话里所有「本地正文已就绪」的邮件：一次连接读完。
///
/// 会话视图逐封调用 `mail_get_message` 时，每封都要重新打开一次 4.9GB 的库；
/// 十四封的会话光打开库就要几秒（用户反馈「读信做不到秒开」）。这里只开一次连接，
/// 能直接用缓存的直接返回详情，需要回服务器补正文的返回 `null` —— 前端再按原路
/// 逐封补拉。渲染结果与单封路径完全一致（同一套渲染版本判定）。
#[command]
pub async fn mail_thread_details(
    app: AppHandle,
    keys: Vec<model::MessageKey>,
) -> Result<Vec<Option<MessageDetail>>, String> {
    ensure_init(&app)?;
    let keys: Vec<(String, String, u32)> = keys
        .into_iter()
        .map(|k| (k.account_id, k.folder, k.uid))
        .collect();
    let records = store::open_thread_records(&keys)?;
    let mut out = Vec::with_capacity(records.len());
    let mut unseen_folders: Vec<String> = Vec::new();
    for record in records {
        let Some(summary) = record.summary else {
            out.push(None);
            continue;
        };
        let Some((cached_html, cached_text, cached_attachments, fetched)) = record.body else {
            out.push(None);
            continue;
        };
        let fresh = fetched
            && !cached_needs_refetch(cached_html.as_deref())
            && !cached_html
                .as_deref()
                .is_some_and(|html| html.to_ascii_lowercase().contains("<style"))
            && record.render_version >= store::RENDER_VERSION
            && record.render_version >= store::TABLE_RENDER_VERSION;
        if !fresh {
            out.push(None);
            continue;
        }
        let mut detail = store::base_detail(&summary);
        detail.html = cached_html;
        detail.text = cached_text;
        detail.attachments = serde_json::from_str(&cached_attachments).unwrap_or_default();
        detail.seen = summary.seen;
        // 打开即已读：单封路径本来就由后端落库，批量路径不能把这件事漏掉，
        // 否则会话读完后列表里仍是粗体。按文件夹合并成一次写入。
        if !summary.seen {
            unseen_folders.push(summary.folder.clone());
            detail.seen = true;
        }
        out.push(Some(detail));
    }
    // 逐封写已读就是逐封开库；按文件夹整批写一次（同一账户的会话）。
    unseen_folders.sort();
    unseen_folders.dedup();
    if let Some((account_id, _, _)) = keys.first() {
        for folder in unseen_folders {
            let uids: Vec<u32> = keys
                .iter()
                .filter(|(acc, f, _)| acc == account_id && *f == folder)
                .map(|(_, _, uid)| *uid)
                .collect();
            // 服务器端的标记仍走单封路径同一条通道（离线可重放），
            // 本地行由它内部一并写，不重复提交。
            let _ = mark_seen_inner(account_id, &folder, &uids, true).await;
        }
    }
    Ok(out)
}

/// 缓存正文是否需要回服务器重新解析（`cid:` 内联图 / 旧渲染版本）。
fn cached_needs_refetch(cached_html: Option<&str>) -> bool {
    cached_html
        .map(|html| html.to_ascii_lowercase().contains("cid:"))
        .unwrap_or(false)
}

#[command]
pub async fn mail_set_seen(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    seen: bool,
) -> Result<(), String> {
    ensure_init(&app)?;
    mark_seen_inner(&account_id, &folder, &uids, seen).await
}
async fn mark_seen_inner(
    account_id: &str,
    folder: &str,
    uids: &[u32],
    seen: bool,
) -> Result<(), String> {
    let account = load_account(account_id)?;
    // POP3 没有服务器端标记：已读只写本地，不需要服务器往返，也不该进重放队列。
    if pop3_sync::is_pop3(&account) {
        store::set_local_flags(account_id, folder, uids, Some(seen), None, None)?;
        if folder == pop3_sync::INBOX {
            let _ = pop3_sync::sync_counts(&account);
        }
        return Ok(());
    }
    if graph::is_graph(&account) {
        let acc = account.clone();
        let (f, u) = (folder.to_string(), uids.to_vec());
        let server = tauri::async_runtime::spawn_blocking(move || {
            graph::set_flags(&acc, &f, &u, Some(seen), None)
        })
        .await
        .map_err(|e| format!("标记任务失败: {e}"))?;
        store::set_local_flags(account_id, folder, uids, Some(seen), None, None)?;
        return server;
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let (f, u) = (folder.to_string(), uids.to_vec());
        let server = tauri::async_runtime::spawn_blocking(move || {
            ews::set_flags(&acc, &f, &u, Some(seen), None)
        })
        .await
        .map_err(|e| format!("标记任务失败: {e}"))?;
        store::set_local_flags(account_id, folder, uids, Some(seen), None, None)?;
        return server;
    }
    let (f, u) = (folder.to_string(), uids.to_vec());
    let op = offline::OfflineOp {
        id: Uuid::new_v4().to_string(),
        account_id: account_id.to_string(),
        kind: "seen".into(),
        folder: folder.to_string(),
        uids: uids.to_vec(),
        value: seen,
        payload: String::new(),
        created_at: chrono::Utc::now().timestamp(),
        attempts: 0,
        last_error: String::new(),
    };
    let acc = account.clone();
    let server = tauri::async_runtime::spawn_blocking(move || {
        imap::set_flags(&acc, "", &f, &u, Some(seen), None)
    })
    .await
    .map_err(|e| format!("标记任务失败: {e}"))?;
    store::set_local_flags(account_id, folder, uids, Some(seen), None, None)?;
    // 服务器那一路失败：断网就排队，其余报错。
    match server {
        Ok(()) => Ok(()),
        Err(e) => write_with_fallback(&account, op, || Err(e)),
    }
}

#[command]
pub async fn mail_set_flagged(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    flagged: bool,
) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    if pop3_sync::is_pop3(&account) {
        store::set_local_flags(&account_id, &folder, &uids, None, Some(flagged), None)?;
        return Ok(());
    }
    if graph::is_graph(&account) {
        let acc = account.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let server = tauri::async_runtime::spawn_blocking(move || {
            graph::set_flags(&acc, &f, &u, None, Some(flagged))
        })
        .await
        .map_err(|e| format!("标记任务失败: {e}"))?;
        store::set_local_flags(&account_id, &folder, &uids, None, Some(flagged), None)?;
        return server;
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let server = tauri::async_runtime::spawn_blocking(move || {
            ews::set_flags(&acc, &f, &u, None, Some(flagged))
        })
        .await
        .map_err(|e| format!("标记任务失败: {e}"))?;
        store::set_local_flags(&account_id, &folder, &uids, None, Some(flagged), None)?;
        return server;
    }
    let (f, u) = (folder.clone(), uids.clone());
    let op = offline::OfflineOp {
        id: Uuid::new_v4().to_string(),
        account_id: account_id.clone(),
        kind: "flagged".into(),
        folder: folder.clone(),
        uids: uids.clone(),
        value: flagged,
        payload: String::new(),
        created_at: chrono::Utc::now().timestamp(),
        attempts: 0,
        last_error: String::new(),
    };
    let acc = account.clone();
    let server = tauri::async_runtime::spawn_blocking(move || {
        imap::set_flags(&acc, "", &f, &u, None, Some(flagged))
    })
    .await
    .map_err(|e| format!("标记任务失败: {e}"))?;
    store::set_local_flags(&account_id, &folder, &uids, None, Some(flagged), None)?;
    match server {
        Ok(()) => Ok(()),
        Err(e) => write_with_fallback(&account, op, || Err(e)),
    }
}

#[command]
pub async fn mail_set_labels(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    labels: String,
) -> Result<(), String> {
    ensure_init(&app)?;
    store::set_local_flags(&account_id, &folder, &[uid], None, None, Some(&labels))
}

/// 移动邮件（归档 / 移入垃圾箱 / 自定义文件夹）。
#[command]
pub async fn mail_move_messages(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    target: String,
) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let (f, u, t) = (folder.clone(), uids.clone(), target.clone());
    let op = offline::OfflineOp {
        id: Uuid::new_v4().to_string(),
        account_id: account_id.clone(),
        kind: "move".into(),
        folder: folder.clone(),
        uids: uids.clone(),
        value: false,
        payload: target.clone(),
        created_at: chrono::Utc::now().timestamp(),
        attempts: 0,
        last_error: String::new(),
    };
    if pop3_sync::is_pop3(&account) {
        // POP3 的文件夹都在本机：移动就是「删掉旧行、在新文件夹写一份」。
        return move_locally(&account_id, &folder, &uids, &target);
    }
    if graph::is_graph(&account) {
        let acc = account.clone();
        let (f, u, t) = (folder.clone(), uids.clone(), target.clone());
        tauri::async_runtime::spawn_blocking(move || graph::move_messages(&acc, &f, &u, &t))
            .await
            .map_err(|e| format!("移动任务失败: {e}"))??;
        let _ = store::delete_messages(&account_id, &folder, &uids);
        return Ok(());
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let (f, u, t) = (folder.clone(), uids.clone(), target.clone());
        tauri::async_runtime::spawn_blocking(move || ews::move_messages(&acc, &f, &u, &t))
            .await
            .map_err(|e| format!("移动任务失败: {e}"))??;
        let _ = store::delete_messages(&account_id, &folder, &uids);
        return Ok(());
    }
    let acc = account.clone();
    let server = tauri::async_runtime::spawn_blocking(move || {
        imap::move_messages(&acc, "", &f, &u, &t)
    })
    .await
    .map_err(|e| format!("移动任务失败: {e}"))?;
    let _ = store::delete_messages(&account_id, &folder, &uids);
    match server {
        Ok(()) => Ok(()),
        Err(e) => write_with_fallback(&account, op, || Err(e)),
    }
}

/// POP3 的删除：有本地「已删除」就移过去，已经在「已删除」里才真正删掉。
///
/// 以前 POP3 直接删本地行，侧栏里的「已删除」永远是空的——用户以为删掉的邮件
/// 去了回收站，实际已经永久消失。POP3 没有服务器端 Trash，这里用本地文件夹模拟。
fn pop3_delete_to_trash(account: &Account, folder: &str, uids: &[u32]) -> Result<(), String> {
    let folders = store::list_folders(&account.id)?;
    let current = folders
        .iter()
        .find(|f| f.name == folder)
        .map(|f| f.role)
        .unwrap_or(FolderRole::Custom);
    let trash = folder_by_role(&account.id, FolderRole::Trash).map(|folder| folder.name);
    if current != FolderRole::Trash {
        if let Some(trash) = trash {
            return move_locally(&account.id, folder, uids, &trash);
        }
    }
    let _ = store::delete_messages(&account.id, folder, uids);
    if folder == pop3_sync::INBOX {
        let _ = pop3_sync::sync_counts(account);
    }
    refresh_folder_totals(&account.id, folder);
    Ok(())
}

/// POP3 账户的本地移动：把（摘要 + 正文）复制到目标文件夹，再删掉原行。
///
/// uid 尽量原样带走：POP3 的 UIDL→uid 映射记的就是这个号，换号之后同步时查不到
/// 「这封还在不在本地」，就被当成用户删掉了 → 忘记 UIDL → 下一轮重新下载一份。
/// 规则自动归类的邮件每封都会走一次这里，所以不保留原号就会让每个自定义文件夹里的
/// 同一封邮件按同步轮次越攒越多（真实反馈：SSL 里 1400 行只有 200 封不同的邮件）。
/// 只有目标文件夹确实已经占着这个 uid 时才换新号，并把映射一起挪过去。
fn move_locally(account_id: &str, folder: &str, uids: &[u32], target: &str) -> Result<(), String> {
    // 原地移动没有意义（同一行改两次反而要多写一次索引）。
    if folder == target {
        return Ok(());
    }
    // 整批在一个事务里只改 folder 列：正文 / 原文都是大字段，逐封复制会让
    // 规则分类卡在加解密上（真实反馈：几百封要等几分钟）。
    let moved = store::move_messages_to_folder(account_id, folder, uids, target)?;
    if moved == 0 {
        return Ok(());
    }
    // POP3 的邮件全在本机：两个文件夹的数量都能就地重算，侧栏角标才是准的
    // （只在内存里改的话，重启后又会读回旧数字）。
    refresh_folder_totals(account_id, folder);
    refresh_folder_totals(account_id, target);
    Ok(())
}

/// 删除：默认移入垃圾箱；已经在垃圾箱里则永久删除。
#[command]
pub async fn mail_delete_messages(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    // POP3 的删除只碰本地：服务器上的那一份由「收信后是否保留副本」决定。
    if pop3_sync::is_pop3(&account) {
        return pop3_delete_to_trash(&account, &folder, &uids);
    }
    let password = account_password(&account)?;
    let folders = store::list_folders(&account_id)?;
    let current_role = folders
        .iter()
        .find(|f| f.name == folder)
        .map(|f| f.role)
        .unwrap_or(FolderRole::Custom);
    let trash = folder_by_role(&account_id, FolderRole::Trash).map(|folder| folder.name);

    if graph::is_graph(&account) {
        let acc = account.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let permanent = matches!(current_role, FolderRole::Trash) || trash.is_none();
        let trash_target = trash.clone();
        tauri::async_runtime::spawn_blocking(move || {
            graph::delete_messages(&acc, &f, &u, permanent, trash_target.as_deref())
        })
        .await
        .map_err(|e| format!("删除任务失败: {e}"))??;
        let _ = store::delete_messages(&account_id, &folder, &uids);
        return Ok(());
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let permanent = matches!(current_role, FolderRole::Trash);
        tauri::async_runtime::spawn_blocking(move || {
            ews::delete_messages(&acc, &f, &u, permanent)
        })
        .await
        .map_err(|e| format!("删除任务失败: {e}"))??;
        let _ = store::delete_messages(&account_id, &folder, &uids);
        return Ok(());
    }

    match (current_role, trash) {
        (FolderRole::Trash, _) | (_, None) => {
            let (f, u) = (folder.clone(), uids.clone());
            let acc = account.clone();
            tauri::async_runtime::spawn_blocking(move || {
                imap::delete_permanently(&acc, &password, &f, &u)
            })
            .await
            .map_err(|e| format!("删除任务失败: {e}"))??;
        }
        (_, Some(trash)) => {
            let (f, u) = (folder.clone(), uids.clone());
            let acc = account.clone();
            tauri::async_runtime::spawn_blocking(move || {
                imap::move_messages(&acc, &password, &f, &u, &trash)
            })
            .await
            .map_err(|e| format!("删除任务失败: {e}"))??;
        }
    }
    let _ = store::delete_messages(&account_id, &folder, &uids);
    Ok(())
}

/// 下载附件到指定目录。`part` 是 MIME 部件的正文字节偏移。
#[command]
pub async fn mail_save_attachment(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    part: u32,
    target_dir: String,
    filename: String,
) -> Result<String, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let password = account_password(&account)?;
    let safe_name = sanitize_filename(&filename);
    let raw = fetch_raw_bytes(&account, &password, &folder, uid).await?;
    let (bytes, _) = mime::extract_part(&raw, part).ok_or("找不到该附件")?;
    let dir = std::path::PathBuf::from(target_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let path = unique_path(&dir, &safe_name);
    std::fs::write(&path, &bytes).map_err(|e| format!("写入附件失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|\r\n\t".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    }
}

/// 下载附件到本地缓存，并交给系统默认程序打开（阅读页点附件的行为）。
///
/// 和「保存到…」分开：这个路径是确定性的（同一封邮件的同一个附件始终落到同一处），
/// 重复点击不会堆出 `file (1).pdf`、`file (2).pdf`；用户想留档时再用保存按钮选目录。
#[command]
pub async fn mail_open_attachment(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    part: u32,
    filename: String,
    content_type: Option<String>,
) -> Result<String, String> {
    ensure_init(&app)?;
    let safe_name = ensure_attachment_extension(
        sanitize_filename(&filename),
        content_type.as_deref(),
    );
    let dir = store::data_root(&app)?
        .join("attachments")
        .join(sanitize_filename(&account_id))
        .join(uid.to_string())
        // 同一封邮件的不同附件可能同名（received_1.pdf / received_1.pdf），按 MIME 部件分目录。
        .join(part.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建缓存目录失败: {e}"))?;
    let path = dir.join(safe_name);
    if !path.exists() {
        // 附件内容不会变，落过一次之后重复点击直接打开缓存，不再重下整封邮件。
        let account = load_account(&account_id)?;
        let password = account_password(&account)?;
        let raw = fetch_raw_bytes(&account, &password, &folder, uid).await?;
        let (bytes, _) = mime::extract_part(&raw, part).ok_or("找不到该附件")?;
        std::fs::write(&path, &bytes).map_err(|e| format!("写入附件失败: {e}"))?;
    }
    // 只调用系统默认关联（不指定程序、不写注册表、不改默认应用）。
    // 如果 Windows 弹出「选择应用」，是因为该扩展名在系统里没有默认程序；
    // 文件名缺少扩展名时按 MIME 补一个（见 `ensure_attachment_extension`），
    // 能避免这种对话框，也就不会把用户的选择记成新的默认程序。
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|e| format!("调用系统默认程序失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 文件名没有扩展名时，按 MIME 类型补一个常见扩展名。
///
/// 没有扩展名的文件在 Windows 上会弹「选择应用」，用户选一次就可能改变该类型的
/// 默认程序；补上扩展名后系统直接走已有默认关联。
fn ensure_attachment_extension(name: String, content_type: Option<&str>) -> String {
    if std::path::Path::new(&name).extension().is_some() {
        return name;
    }
    let Some(ext) = content_type.and_then(mime_extension) else {
        return name;
    };
    format!("{name}.{ext}")
}

fn mime_extension(content_type: &str) -> Option<&'static str> {
    let base = content_type.split(';').next()?.trim().to_ascii_lowercase();
    Some(match base.as_str() {
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "text/plain" => "txt",
        "text/html" => "html",
        "text/csv" => "csv",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "application/zip" => "zip",
        "message/rfc822" => "eml",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.ms-powerpoint" => "ppt",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "audio/mpeg" => "mp3",
        "video/mp4" => "mp4",
        _ => return None,
    })
}

fn unique_path(dir: &std::path::Path, filename: &str) -> std::path::PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let path = std::path::Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("attachment");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for i in 1..1000 {
        let name = if ext.is_empty() {
            format!("{stem} ({i})")
        } else {
            format!("{stem} ({i}).{ext}")
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-{}{}", Uuid::new_v4(), ext))
}

/// 把整封邮件（含附件）另存为 `.eml`。
///
/// 用原始报文而不是重新拼一份：「另存为」的用途是归档 / 转交 / 交给别的客户端打开，
/// 原始 RFC822 才能被 Thunderbird、Outlook、邮件取证工具原样读取。
#[command]
pub async fn mail_save_as_eml(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    target_path: String,
) -> Result<String, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let password = account_password(&account)?;
    let raw = fetch_raw_bytes(&account, &password, &folder, uid).await?;
    std::fs::write(&target_path, &raw).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(target_path)
}

/// 打开本机保存的 `.eml` 查看。
///
/// 「另存为 .eml」只有出口没有入口：用户把邮件存成文件之后，想看还得换程序。
/// 这里读一个本地文件、走与收信完全相同的解析 + 清洗路径，返回一份可直接渲染的详情。
/// 不写库、不碰账户：附件的 `part` 偏移对本地文件没有意义，前端只列名字与大小。
#[command]
pub async fn mail_read_eml(app: AppHandle, path: String) -> Result<MessageDetail, String> {
    ensure_init(&app)?;
    let bytes = std::fs::read(&path).map_err(|e| format!("读取文件失败：{e}"))?;
    let parsed = mime::parse(&bytes, true).ok_or("这个文件不是一封可解析的邮件（.eml）")?;
    let file_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("邮件")
        .to_string();
    Ok(MessageDetail {
        account_id: String::new(),
        folder: file_name,
        uid: 0,
        message_id: parsed.message_id.clone(),
        subject: parsed.subject.clone(),
        from_name: parsed.from_name.clone(),
        from_addr: parsed.from_addr.clone(),
        to: parsed.to.clone(),
        cc: parsed.cc.clone(),
        bcc: parsed.bcc.clone(),
        reply_to: parsed.reply_to.clone(),
        date: parsed.date,
        received_at: parsed.date,
        in_reply_to: parsed.in_reply_to.clone(),
        references: parsed.references.clone(),
        html: parsed.html.clone(),
        text: parsed.text.clone(),
        attachments: parsed.attachments.clone(),
        headers: parsed.headers.clone(),
        labels: String::new(),
        seen: true,
        flagged: false,
    })
}

/// 把邮件正文另存为 `.html`（单文件，图片仍是远端地址）。
#[command]
pub async fn mail_save_as_html(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    target_path: String,
) -> Result<String, String> {
    ensure_init(&app)?;
    let (html, text, _, _) = store::get_body(&account_id, &folder, uid)?
        .ok_or_else(|| "本地没有这封邮件的正文".to_string())?;
    let summary = store::get_summary(&account_id, &folder, uid)?
        .ok_or_else(|| "邮件不存在".to_string())?;
    let title = if summary.subject.trim().is_empty() {
        "（无主题）".to_string()
    } else {
        summary.subject.clone()
    };
    let body = html.unwrap_or_else(|| format!("<pre>{}</pre>", escape_html(&text)));
    let doc = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{}</title>         <style>body{{font-family:system-ui,'Microsoft YaHei',sans-serif;max-width:820px;         margin:32px auto;padding:0 20px;line-height:1.7;color:#222}}         img{{max-width:100%}}</style>{}",
        escape_html(&title),
        body
    );
    std::fs::write(&target_path, doc).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(target_path)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 本地加密开关（默认开）。关闭后新正文明文落盘，已有密文仍可读。
#[command]
pub async fn mail_get_encryption_enabled(app: AppHandle) -> Result<bool, String> {
    ensure_init(&app)?;
    Ok(store::encryption_enabled())
}

#[command]
pub async fn mail_set_encryption_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    ensure_init(&app)?;
    store::set_encryption_enabled(enabled)
}

/// 标记为垃圾邮件：移动到垃圾箱（没有垃圾箱就用 Junk 角色文件夹，再退回收件箱旁的默认名）。
#[command]
pub async fn mail_junk_messages(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
) -> Result<(), String> {
    ensure_init(&app)?;
    let target = junk_folder_name(&account_id)?;
    move_batch(&app, &account_id, &folder, &uids, &target).await
}

/// 取消垃圾标记：把邮件移回收件箱。
#[command]
pub async fn mail_unmark_junk(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
) -> Result<(), String> {
    ensure_init(&app)?;
    let inbox = folder_by_role(&account_id, FolderRole::Inbox)
        .map(|folder| folder.name)
        .ok_or_else(|| "账户没有收件箱".to_string())?;
    move_batch(&app, &account_id, &folder, &uids, &inbox).await
}

fn junk_folder_name(account_id: &str) -> Result<String, String> {
    folder_by_role(account_id, FolderRole::Junk)
        .map(|folder| folder.name)
        .ok_or_else(|| "该账户没有垃圾邮件文件夹（可在文件夹列表里新建「垃圾邮件」）".to_string())
}

/// 批量移动的共用实现：一次 IMAP 往返搬完整批，而不是逐封调用。
async fn move_batch(
    app: &AppHandle,
    account_id: &str,
    folder: &str,
    uids: &[u32],
    target: &str,
) -> Result<(), String> {
    ensure_init(app)?;
    if uids.is_empty() {
        return Ok(());
    }
    let account = load_account(account_id)?;
    if pop3_sync::is_pop3(&account) {
        return move_locally(account_id, folder, uids, target);
    }
    if graph::is_graph(&account) {
        let acc = account.clone();
        let (f, t, batch) = (folder.to_string(), target.to_string(), uids.to_vec());
        tauri::async_runtime::spawn_blocking(move || graph::move_messages(&acc, &f, &batch, &t))
            .await
            .map_err(|e| format!("移动任务失败: {e}"))??;
        let _ = store::delete_messages(account_id, folder, uids);
        return Ok(());
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let (f, t, batch) = (folder.to_string(), target.to_string(), uids.to_vec());
        tauri::async_runtime::spawn_blocking(move || ews::move_messages(&acc, &f, &batch, &t))
            .await
            .map_err(|e| format!("移动任务失败: {e}"))??;
        let _ = store::delete_messages(account_id, folder, uids);
        return Ok(());
    }
    let password = account_password(&account)?;
    let (acc, f, t) = (account.clone(), folder.to_string(), target.to_string());
    let batch = uids.to_vec();
    tauri::async_runtime::spawn_blocking(move || imap::move_messages(&acc, &password, &f, &batch, &t))
        .await
        .map_err(|e| format!("移动任务失败: {e}"))??;
    let _ = store::delete_messages(account_id, folder, uids);
    Ok(())
}

/// 批量标记已读 / 未读 / 星标。一次往返，不逐封发命令。
#[command]
pub async fn mail_batch_flags(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    seen: Option<bool>,
    flagged: Option<bool>,
) -> Result<(), String> {
    ensure_init(&app)?;
    if uids.is_empty() {
        return Ok(());
    }
    let account = load_account(&account_id)?;
    if pop3_sync::is_pop3(&account) {
        let result = store::set_local_flags(&account_id, &folder, &uids, seen, flagged, None);
        if folder == pop3_sync::INBOX {
            let _ = pop3_sync::sync_counts(&account);
        }
        return result;
    }
    if graph::is_graph(&account) {
        let acc = account.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let server = tauri::async_runtime::spawn_blocking(move || {
            graph::set_flags(&acc, &f, &u, seen, flagged)
        })
        .await
        .map_err(|e| format!("批量标记失败: {e}"))?;
        store::set_local_flags(&account_id, &folder, &uids, seen, flagged, None)?;
        return server;
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let server = tauri::async_runtime::spawn_blocking(move || {
            ews::set_flags(&acc, &f, &u, seen, flagged)
        })
        .await
        .map_err(|e| format!("批量标记失败: {e}"))?;
        store::set_local_flags(&account_id, &folder, &uids, seen, flagged, None)?;
        return server;
    }
    let password = account_password(&account)?;
    let (acc, f) = (account.clone(), folder.clone());
    let batch = uids.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imap::set_flags(&acc, &password, &f, &batch, seen, flagged)
    })
    .await
    .map_err(|e| format!("批量标记失败: {e}"))??;
    store::set_local_flags(&account_id, &folder, &uids, seen, flagged, None)
}

/// 批量删除（移入垃圾箱；已在垃圾箱则永久删除）。
#[command]
pub async fn mail_batch_delete(
    app: AppHandle,
    account_id: String,
    folder: String,
    uids: Vec<u32>,
) -> Result<(), String> {
    ensure_init(&app)?;
    if uids.is_empty() {
        return Ok(());
    }
    let account_for_protocol = load_account(&account_id)?;
    if pop3_sync::is_pop3(&account_for_protocol) {
        return pop3_delete_to_trash(&account_for_protocol, &folder, &uids);
    }
    let folders = store::list_folders(&account_id)?;
    let current = folders
        .iter()
        .find(|f| f.name == folder)
        .map(|f| f.role)
        .unwrap_or(FolderRole::Custom);
    let trash = folder_by_role(&account_id, FolderRole::Trash).map(|folder| folder.name);

    if graph::is_graph(&account_for_protocol) {
        let acc = account_for_protocol.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let permanent = matches!(current, FolderRole::Trash) || trash.is_none();
        let trash_target = trash.clone();
        tauri::async_runtime::spawn_blocking(move || {
            graph::delete_messages(&acc, &f, &u, permanent, trash_target.as_deref())
        })
        .await
        .map_err(|e| format!("删除任务失败: {e}"))??;
        let _ = store::delete_messages(&account_id, &folder, &uids);
        return Ok(());
    }
    if ews::is_ews(&account_for_protocol) {
        let acc = account_for_protocol.clone();
        let (f, u) = (folder.clone(), uids.clone());
        let permanent = matches!(current, FolderRole::Trash);
        tauri::async_runtime::spawn_blocking(move || ews::delete_messages(&acc, &f, &u, permanent))
            .await
            .map_err(|e| format!("删除任务失败: {e}"))??;
        let _ = store::delete_messages(&account_id, &folder, &uids);
        return Ok(());
    }

    match (current, trash) {
        (FolderRole::Trash, _) | (_, None) => {
            let account = load_account(&account_id)?;
            let password = account_password(&account)?;
            let (acc, f) = (account.clone(), folder.clone());
            let batch = uids.clone();
            tauri::async_runtime::spawn_blocking(move || {
                imap::delete_permanently(&acc, &password, &f, &batch)
            })
            .await
            .map_err(|e| format!("删除任务失败: {e}"))??;
        }
        (_, Some(trash)) => {
            move_batch(&app, &account_id, &folder, &uids, &trash).await?;
        }
    }
    let _ = store::delete_messages(&account_id, &folder, &uids);
    Ok(())
}

// ---------------------------------------------------------------------------
// 存储路径 / 备份恢复
// ---------------------------------------------------------------------------

/// 读取当前存储配置：实际生效目录 + 自定义目录（空表示默认）。
#[command]
pub async fn mail_storage_info(app: AppHandle) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let active = store::data_root(&app)?;
    let db = store::db_path(&app)?;
    let size = std::fs::metadata(&db).map(|m| m.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "activeDir": active.to_string_lossy(),
        "customDir": store::read_storage_dir(),
        "dbPath": db.to_string_lossy(),
        "dbBytes": size,
    }))
}

/// 迁移邮件库到新目录。
///
/// 步骤刻意做成「先复制、校验、再切换」：直接 move 一旦中途失败，
/// 用户会同时失去新旧两份数据。切换成功后才删源文件。
#[command]
pub async fn mail_migrate_storage(app: AppHandle, target_dir: String) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let target = std::path::PathBuf::from(target_dir.trim());
    if target_dir.trim().is_empty() {
        return Err("目标目录为空".to_string());
    }
    std::fs::create_dir_all(&target).map_err(|e| format!("创建目录失败: {e}"))?;

    let source = store::db_path(&app)?;
    let dest = target.join("mail.db");
    if source == dest {
        return Err("目标与当前目录相同".to_string());
    }
    if dest.exists() {
        return Err(format!("目标已存在 {}，请换一个目录或先改名", dest.display()));
    }

    // 先 checkpoint，确保 WAL 里的数据也写进主库文件，否则复制出来的库会丢最近的写入。
    store::checkpoint()?;
    std::fs::copy(&source, &dest).map_err(|e| format!("复制数据库失败: {e}"))?;

    let src_len = std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0);
    let dst_len = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    if src_len != dst_len {
        let _ = std::fs::remove_file(&dest);
        return Err("复制后大小不一致，已回滚（原库未改动）".to_string());
    }

    // 切换：写配置 + 换内存里的路径。此后的连接都指向新文件。
    store::set_storage_dir(&target.to_string_lossy())?;
    store::switch_db_path(dest.clone())?;

    // 源文件保留为 `.bak`，不直接删——用户确认新位置能用之后再自己清。
    let backup = source.with_extension("db.bak");
    let _ = std::fs::rename(&source, &backup);
    // WAL/SHM 一起挪走，否则新位置的库会读到旧的 -wal。
    for suffix in ["-wal", "-shm"] {
        let from = source.with_file_name(format!("mail.db{suffix}"));
        if from.exists() {
            let _ = std::fs::rename(&from, backup.with_file_name(format!("mail.db.bak{suffix}")));
        }
    }

    pool::invalidate_all();
    Ok(serde_json::json!({
        "dbPath": dest.to_string_lossy(),
        "backupPath": backup.to_string_lossy(),
    }))
}

/// 把整个邮件库导出为备份文件（`.db`，可直接再导入）。
#[command]
pub async fn mail_backup(app: AppHandle, target_path: String) -> Result<String, String> {
    ensure_init(&app)?;
    store::checkpoint()?;
    let source = store::db_path(&app)?;
    std::fs::copy(&source, &target_path).map_err(|e| format!("写备份失败: {e}"))?;
    Ok(target_path)
}

/// 从备份文件恢复。
///
/// 覆盖前先把当前库改名留档（`.before-restore`），恢复失败能退回去——
/// 「恢复」是少数几个一旦出错就彻底丢数据的操作，必须留后路。
#[command]
pub async fn mail_restore(app: AppHandle, source_path: String) -> Result<(), String> {
    ensure_init(&app)?;
    let src = std::path::PathBuf::from(source_path.trim());
    if !src.exists() {
        return Err("备份文件不存在".to_string());
    }

    // 校验：是不是一个能打开的 SQLite，且含有我们需要的表。
    {
        let conn = rusqlite::Connection::open_with_flags(
            &src,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| format!("备份文件无法打开: {e}"))?;
        let ok: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("备份文件已损坏: {e}"))?;
        if ok == 0 {
            return Err("备份文件里没有邮件表，可能不是本应用的备份".to_string());
        }
    }

    let dest = store::db_path(&app)?;
    store::checkpoint()?;

    let safety = dest.with_extension("db.before-restore");
    if dest.exists() {
        std::fs::copy(&dest, &safety).map_err(|e| format!("备份当前库失败: {e}"))?;
    }
    std::fs::copy(&src, &dest).map_err(|e| format!("写入恢复文件失败: {e}"))?;
    // 恢复后旧的 WAL 会让新库读到过期页，必须清掉。
    for suffix in ["-wal", "-shm"] {
        let p = dest.with_file_name(format!("mail.db{suffix}"));
        let _ = std::fs::remove_file(p);
    }
    store::reset_runtime_caches();
    // 库整个换了：连接池里的会话绑的是旧库状态，IDLE 线程也要停。
    pool::invalidate_all();
    Ok(())
}

// ---------------------------------------------------------------------------
// 发送
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_send(app: AppHandle, request: SendRequest) -> Result<(), String> {
    ensure_init(&app)?;
    send_inner(&app, &request).await
}

async fn send_inner(app: &AppHandle, request: &SendRequest) -> Result<(), String> {
    // 写信页的通讯组只以 `组名 <group:ID>` 存在收件人里；真正投递前展开成成员地址。
    // 这一步必须在组装 MIME 之前：邮件头里不能出现 `group:` 这种非地址串，
    // 否则收信端解析不到收件人，回复全部也会丢人。
    let mut request = request.clone();
    let mut expanded = contacts::store::expand_recipient_lists(&[
        request.to.clone(),
        request.cc.clone(),
        request.bcc.clone(),
    ])?;
    request.bcc = expanded.pop().unwrap_or_default();
    request.cc = expanded.pop().unwrap_or_default();
    request.to = expanded.pop().unwrap_or_default();
    if request.to.is_empty() && request.cc.is_empty() && request.bcc.is_empty() {
        return Err("收件人为空".to_string());
    }

    let account = load_account(&request.account_id)?;
    let password = account_password(&account)?;
    let from = MailAddress {
        name: account.display_name.clone(),
        address: account.email.clone(),
    };
    let save_to_sent = request.save_to_sent.unwrap_or(true);

    // 附件体积检查：超过上限直接拒绝，而不是让用户在 SMTP 超时五分钟之后
    // 才看到一句「发送失败」。上限取设置里的值（默认 25 MB）。
    let settings = store::load_settings();
    let limit_bytes = (settings.attachment_limit_mb.max(1) as u64) * 1024 * 1024;
    let total = mime::attachments_size(&request.attachments);
    if total > limit_bytes {
        return Err(format!(
            "附件总大小 {:.1} MB，超过 {} MB 上限。请改用网盘链接，或在设置里调高上限。",
            total as f64 / 1048576.0,
            settings.attachment_limit_mb
        ));
    }

    let mut raw = mime::build(&request, &from, &account.signature)?;

    // 端到端加密 / 签名：在 SMTP 之前对整份 MIME 再做一层封装。
    let mode = request.crypto_mode.as_str();
    if mode.starts_with("smime") || mode.starts_with("pgp") {
        let account_for_crypto = account.clone();
        let to_list = request.to.clone();
        let mode = mode.to_string();
        let signed_only = mode.contains("sign");
        let app_for_pgp = app.clone();
        raw = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
            let key_dir = pgp::key_dir(&app_for_pgp)?;
            let canonical = smime::canonicalize(&raw);
            if mode.starts_with("pgp") {
                if mode.contains("encrypt") {
                    let recipient = to_list.first().cloned().unwrap_or_default();
                    let encrypted = pgp::encrypt(&key_dir, &recipient, &canonical)?;
                    Ok(pgp::wrap_encrypted(&encrypted))
                } else {
                    let signed = pgp::sign(&key_dir, &canonical, "")?;
                    Ok(pgp::wrap_signed(&signed))
                }
            } else if signed_only && !mode.contains("encrypt") {
                smime::sign(&account_for_crypto.id, &canonical)
            } else {
                // 加密：需要收件人证书。找不到证书时报清楚，
                // 而不是默默退化成明文发送——后者是最危险的行为。
                let cert = smime::recipient_certificate(&to_list)
                    .ok_or_else(|| "没有收件人的 S/MIME 证书，无法加密发送".to_string())?;
                if signed_only {
                    // 标准顺序：先签名，再把签名结果加密。
                    let signed = smime::sign(&account_for_crypto.id, &canonical)?;
                    let wrapped = smime::wrap_signed_entity(&signed);
                    smime::encrypt(&account_for_crypto.id, &wrapped, &cert)
                } else {
                    smime::encrypt(&account_for_crypto.id, &canonical, &cert)
                }
            }
        })
        .await
        .map_err(|e| format!("加密封装失败: {e}"))??;
    }

    // Graph：直接把这封 MIME 交给 Exchange Online。附件、内联图、签名、
    // S/MIME/PGP 都在上面的 `mime::build` / 加密封装里处理完了。
    if graph::is_graph(&account) {
        // Graph 的 MIME 草稿接口约有 4 MB 上限；MIME 里的 base64 还会再膨胀，
        // 超过时直接说清楚，避免用户等到超时才看到失败。
        if raw.len() > 4 * 1024 * 1024 {
            return Err(
                "Exchange Graph 单封邮件（含附件）不能超过约 4 MB；请改用网盘链接，或拆分附件。"
                    .to_string(),
            );
        }
        let acc = account.clone();
        let raw_for_send = raw.clone();
        tauri::async_runtime::spawn_blocking(move || graph::send_mime(&acc, &raw_for_send))
            .await
            .map_err(|e| format!("发信任务失败: {e}"))??;
        info!(account = %account.email, "mail: Graph 发送成功");
        record_sent_recipients(&account, &request);
        return Ok(());
    }

    if ews::is_ews(&account) {
        if raw.len() > 20 * 1024 * 1024 {
            return Err("Exchange EWS 单封邮件（含附件）不能超过 20 MB；请改用网盘链接或拆分附件。".to_string());
        }
        let drafts = folder_name_by_role(&account.id, FolderRole::Drafts)?;
        let sent = folder_name_by_role(&account.id, FolderRole::Sent)?;
        let acc = account.clone();
        let raw_for_send = raw.clone();
        tauri::async_runtime::spawn_blocking(move || {
            ews::send_mime(&acc, &drafts, &sent, &raw_for_send)
        })
        .await
        .map_err(|e| format!("发信任务失败: {e}"))??;
        info!(account = %account.email, "mail: EWS 发送成功");
        record_sent_recipients(&account, &request);
        return Ok(());
    }

    // 信封收件人去重：同一个人既在「收件人」又在「抄送」，或者两个通讯组都包含他时，
    // 服务器会收到两条 RCPT，部分服务器会因此重复投递。邮件头保持原样（各字段各自的收件人）。
    let mut recipients: Vec<String> = Vec::new();
    for addr in request
        .to
        .iter()
        .chain(request.cc.iter())
        .chain(request.bcc.iter())
    {
        let email = mime::parse_mailbox(addr)?.email.to_string();
        let normalized = email.trim().to_ascii_lowercase();
        if !normalized.is_empty()
            && !recipients
                .iter()
                .any(|seen: &String| seen.eq_ignore_ascii_case(&email))
        {
            recipients.push(email);
        }
    }

    let acc = account.clone();
    let envelope_from = account.email.clone();
    let raw_for_send = raw.clone();
    let pw_for_send = password.clone();
    let recipient_count = recipients.len();
    let send_result = tauri::async_runtime::spawn_blocking(move || {
        send_in_batches(&acc, &pw_for_send, &envelope_from, &raw_for_send, &recipients)
    })
    .await
    .map_err(|e| format!("发信任务失败: {e}"))?;
    if let Err(error) = send_result {
        // 大通讯组最常见的失败是服务器对「单封收件人数」设了上限（Gmail 100、
        // Exchange 默认 500），报错往往只指向某一个地址，很难看出是人数问题。
        if recipient_count > 50 && error.contains("被拒绝") {
            return Err(format!(
                "{error}\n这封邮件共 {recipient_count} 位收件人，超过了邮件服务器允许的单封收件人数。请把通讯组拆小后分批发送，或改用密送。"
            ));
        }
        return Err(error);
    }

    if save_to_sent {
        if pop3_sync::is_pop3(&account) {
            // POP3 没有服务器端的「已发送」：副本落到本地库的 Sent。
            // 不再等它写完——SMTP 已经成功，副本晚几百毫秒出现比让用户多等一秒更好。
            let acc = account.clone();
            let raw_clone = raw.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = pop3_sync::save_sent_copy(&acc, &raw_clone) {
                    warn!(account = %acc.email, error = %error, "mail: 保存 POP3 已发送副本失败");
                }
            });
        } else if let Some(sent) = imap::sent_folder_name(&account) {
            // IMAP 的已发送要再连一次服务器（TCP + TLS + 登录 + APPEND）。
            // 以前这一趟串在发送路径里等：用户看到「发送中」多转好几秒。
            let acc = account.clone();
            let pw = password.clone();
            let raw_clone = raw.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = imap::append_to_folder(&acc, &pw, &sent, &raw_clone, &["seen"]) {
                    warn!(account = %acc.email, error = %error, "mail: 追加到已发送失败");
                }
            });
        }
    }
    info!(account = %account.email, "mail: 发送成功");
    record_sent_recipients(&account, &request);
    Ok(())
}

/// 发送成功后把收件人记进通讯录（SMTP 与 Graph 共用）。
fn record_sent_recipients(account: &Account, request: &SendRequest) {
    let own = vec![account.email.clone(), account.login_name()];
    let mut to_labeled: Vec<contacts::model::LabeledValue> = Vec::new();
    let mut cc_labeled: Vec<contacts::model::LabeledValue> = Vec::new();
    for (list, out) in [(&request.to, &mut to_labeled), (&request.cc, &mut cc_labeled)] {
        for raw in list {
            if let Ok(addr) = mime::parse_mailbox(raw) {
                out.push(contacts::model::LabeledValue {
                    label: addr.name.unwrap_or_default(),
                    value: addr.email.to_string(),
                });
            }
        }
    }
    let when = chrono::Utc::now().timestamp();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) =
            contacts::store::touch_from_message(&own, "", "", &to_labeled, &cc_labeled, when)
        {
            warn!(error = %error, "mail: 收录发件收件人失败");
        }
    });
}

/// 服务器拒绝 RCPT 是不是因为「这一封的收件人太多」。
///
/// Gmail 返回 `452 4.5.3 Too many recipients`；Exchange 常见 `550 5.5.3` 且正文里
/// 带 recipient limit。只有明确是人数问题时才拆分发，普通地址无效（550 5.1.1）不该重发。
fn recipient_limit_hit(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("too many recipient")
        || lower.contains("recipient limit")
        || error.contains("（452）")
        || error.contains("(452)")
        || error.contains("收件人数")
        || error.contains("收件人数量")
}

/// 逐批投递：先按整份名单发，被服务器以「收件人过多」拒绝时对半拆分重试。
///
/// 拆分只改信封收件人，报文（含邮件头）保持不变；每个收件人只会落在其中一批里。
/// 所以同一封邮件不会重复投递，只是服务器会收到多次 DATA。
fn send_in_batches(
    account: &Account,
    password: &str,
    envelope_from: &str,
    raw: &[u8],
    recipients: &[String],
) -> Result<(), String> {
    fn attempt(
        account: &Account,
        password: &str,
        envelope_from: &str,
        raw: &[u8],
        batch: &[String],
        depth: usize,
    ) -> Result<(), String> {
        match smtp::send_raw(
            account,
            password,
            effective_proxy(account),
            envelope_from,
            batch,
            raw,
        ) {
            Ok(()) => Ok(()),
            Err(error)
                if depth < 4 && batch.len() > 1 && recipient_limit_hit(&error) =>
            {
                let middle = batch.len() / 2;
                warn!(
                    recipients = batch.len(),
                    "mail: 服务器限制单封收件人数，拆分后重发"
                );
                attempt(account, password, envelope_from, raw, &batch[..middle], depth + 1)?;
                attempt(account, password, envelope_from, raw, &batch[middle..], depth + 1)
            }
            Err(error) => Err(error),
        }
    }
    attempt(account, password, envelope_from, raw, recipients, 0)
}

/// 草稿里保留写信页的原始收件人清单（含 `名称 <group:ID>` 芯片）。
///
/// 草稿正文头必须写展开后的真实地址（别的客户端要能读懂），
/// 所以把用户看到的清单单独塞进 `X-Guaxi-Draft-Recipients`（base64 JSON）。
/// 本应用续写草稿时优先用它恢复组芯片；其他客户端会忽略这个头。
fn insert_draft_recipients_header(raw: Vec<u8>, recipients: &serde_json::Value) -> Vec<u8> {
    use base64::Engine as _;
    let Ok(json) = serde_json::to_vec(recipients) else {
        return raw;
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(json);
    let mut header = String::from("X-Guaxi-Draft-Recipients:");
    for (index, chunk) in encoded.as_bytes().chunks(74).enumerate() {
        header.push_str(if index == 0 { " " } else { "\r\n " });
        header.push_str(std::str::from_utf8(chunk).unwrap_or(""));
    }
    header.push_str("\r\n");
    let separator = b"\r\n\r\n";
    let Some(pos) = raw
        .windows(separator.len())
        .position(|window| window == separator.as_slice())
    else {
        return raw;
    };
    let mut out = Vec::with_capacity(raw.len() + header.len());
    out.extend_from_slice(&raw[..pos + 2]);
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(&raw[pos + 2..]);
    out
}

/// 延迟发送列表里展示的收件人：通讯组只显示组名，别把 `group:` 内部标记露给用户。
fn display_recipients(list: &[String]) -> String {
    list.iter()
        .map(|raw| {
            if let Some(open) = raw.rfind("<group:") {
                let name = raw[..open].trim();
                if name.is_empty() {
                    "通讯组".to_string()
                } else {
                    format!("{name}（通讯组）")
                }
            } else {
                raw.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// 把一条发不出去的待发邮件写进本地「发件箱」文件夹，侧栏与列表里就能看到它。
///
/// 与 `outbox` 表是两份数据：表里留着原始请求（重试 / 撤回都靠它），
/// 文件夹里是给人看的一封「邮件」（可打开、可另存、可删除）。
/// 失败只记日志——这一步是善后，不该让「发送失败」的报错被它盖住。
fn stage_outbox_into_folder(request: &SendRequest, error: &str) {
    let Ok(account) = load_account(&request.account_id) else {
        return;
    };
    let Ok(folders) = store::list_folders(&account.id) else {
        return;
    };
    let Some(outbox) = folders.iter().find(|f| f.role == FolderRole::Outbox) else {
        return;
    };
    let from = MailAddress {
        name: account.display_name.clone(),
        address: account.email.clone(),
    };
    // 正文后面附一句失败原因：用户打开这封「没发出去的邮件」时第一眼要知道为什么。
    let mut request = request.clone();
    let note = format!(
        "<p style=\"margin:12px 0 0;padding:8px 10px;border-radius:6px;background:#fff4e5;color:#8a5a00;font-size:12px\">上次发送失败：{}</p>",
        escape_html(error)
    );
    request.html = Some(format!("{}{}", request.html.unwrap_or_default(), note));
    let Ok(raw) = mime::build(&request, &from, &account.signature) else {
        return;
    };
    let _ = pop3_sync::save_copy_to(&account, &outbox.name, &raw, false);
    // 侧栏「发件箱」只在有内容时出现：写入后刷新计数。
    refresh_folder_totals(&account.id, &outbox.name);
}

/// 延迟发送：先进 outbox，倒计时结束后由 `mail_flush_outbox` 投递。
///
/// `send_at`（Unix 秒）用于「定时发送」：给了就按这个时间投递，
/// 不再叠加设置里的撤回窗口；没给则沿用原来的倒计时撤回逻辑。
#[command]
pub async fn mail_queue_send(
    app: AppHandle,
    request: SendRequest,
    send_at: Option<i64>,
) -> Result<OutboxItem, String> {
    ensure_init(&app)?;
    let settings = store::load_settings();
    let delay = settings.undo_send_seconds as i64;
    let now = chrono::Utc::now().timestamp();
    let due_at = match send_at {
        Some(at) if at > now => at,
        Some(_) => return Err("定时发送时间必须晚于当前时间".to_string()),
        None => now + delay.max(1),
    };
    let item = OutboxItem {
        id: Uuid::new_v4().to_string(),
        account_id: request.account_id.clone(),
        subject: request.subject.clone(),
        to: display_recipients(&request.to),
        due_at,
        created_at: now,
        attempts: 0,
        next_attempt_at: 0,
        last_error: String::new(),
        staged: false,
    };
    let payload = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    store::enqueue_outbox(&item, &payload)?;
    Ok(item)
}

/// 投递失败多少次之后不再自动重试，转进「发件箱」等用户处置。
///
/// 取 6 次：按退避（60s 起步、逐次翻倍、上限 1 小时）大约是「隔一夜还发不出去」。
/// 再自动重试只会在用户不知情的情况下反复撞服务器，不如把问题摆到台面上。
const OUTBOX_STAGE_AFTER: u32 = 6;

#[command]
pub async fn mail_list_outbox(app: AppHandle) -> Result<Vec<OutboxItem>, String> {
    ensure_init(&app)?;
    store::list_outbox()
}

/// 撤回一条待发邮件（倒计时窗口内，或发件箱里点「撤回」）。
#[command]
pub async fn mail_cancel_outbox(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app)?;
    store::drop_outbox(&id)
}

/// 重试一条暂存在发件箱里的邮件：清零退避后立刻尝试投递一次。
///
/// 用户在发件箱里点「重试」时往往刚修好网络或改好口令，不该再等一小时退避。
#[command]
pub async fn mail_retry_outbox(app: AppHandle, id: String) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let payload = store::outbox_payload(&id)?.ok_or_else(|| "这条待发邮件不存在".to_string())?;
    let request: SendRequest = serde_json::from_str(&payload).map_err(|e| e.to_string())?;
    store::requeue_outbox(&id)?;
    match send_inner(&app, &request).await {
        Ok(()) => {
            store::mark_outbox_sent(&id)?;
            // 成功了就从发件箱文件夹里把暂存的那一封也删掉。
            remove_staged_copy(&request);
            Ok(serde_json::json!({ "sent": true }))
        }
        Err(error) => {
            // 仍然是失败：立刻退回「暂存」状态，不要在后台又开始自动重试。
            store::stage_outbox(&id, &error).ok();
            Err(error)
        }
    }
}

/// 发件箱里暂存的那封「邮件」（与 outbox 表里的待发请求是两份数据）。
/// 发送成功后要把它从文件夹里删掉，否则用户会看到「已发送」和「发件箱」各有一份。
fn remove_staged_copy(request: &SendRequest) {
    let Ok(account) = load_account(&request.account_id) else {
        return;
    };
    let Ok(folders) = store::list_folders(&account.id) else {
        return;
    };
    let Some(outbox) = folders.iter().find(|f| f.role == FolderRole::Outbox) else {
        return;
    };
    let Ok(rows) = store::list_recent(&account.id, &outbox.name, 500, 0) else {
        return;
    };
    let subject = request.subject.trim();
    for row in rows {
        if row.subject.trim() == subject {
            let _ = store::delete_message(&account.id, &outbox.name, row.uid);
            refresh_folder_totals(&account.id, &outbox.name);
            return;
        }
    }
}

/// 按本地库的实际行数刷新某个本地文件夹的 total / unread。
///
/// 服务器文件夹的计数来自 IMAP STATUS；本地文件夹（发件箱、POP3 的已发送 / 草稿）
/// 只能自己算，否则侧栏角标会一直是 0。
fn refresh_folder_totals(account_id: &str, folder: &str) {
    // Graph 的本地只缓存最近几百封：用本地行数更新会把侧栏角标改小。
    // 计数由 Graph 同步 / 文件夹刷新负责；这里不要在异步线程里再发一次 HTTP。
    if let Ok(Some(account)) = store::get_account(account_id) {
        if graph::is_graph(&account) || ews::is_ews(&account) {
            return;
        }
    }
    // 只查两个数，不再把整个文件夹的行都读出来物化一遍：规则批量搬运时
    // 这个函数会在每封邮件上被调用，全表读取会让「对已有邮件执行」卡上几十分钟。
    let Ok((total, unread)) = store::folder_counts(account_id, folder) else {
        return;
    };
    let _ = store::update_folder_counts(account_id, folder, unread, total, 0, 0);
}

/// POP3 账户的草稿：落到本地 `Drafts` 文件夹，与「已发送」副本同一套写库路径。
fn save_draft_locally(account: &Account, mut request: SendRequest) -> Result<u32, String> {
    // 收件人里可能有通讯组芯片（`组名 <group:ID>`）：正文头要展开成真实地址，
    // 但本地草稿的列表行只关心主题与时间，展开结果不影响它。
    let mut expanded = contacts::store::expand_recipient_lists(&[
        request.to.clone(),
        request.cc.clone(),
        request.bcc.clone(),
    ])?;
    request.bcc = expanded.pop().unwrap_or_default();
    request.cc = expanded.pop().unwrap_or_default();
    request.to = expanded.pop().unwrap_or_default();
    let from = MailAddress {
        name: account.display_name.clone(),
        address: account.email.clone(),
    };
    let raw = mime::build_draft(&request, &from, &account.signature)?;
    let uid = pop3_sync::save_draft_copy(account, &raw)?;
    // 侧栏「草稿」的数量来自 folders 表：存完要重算，角标才会立刻 +1。
    refresh_folder_totals(&account.id, "Drafts");
    Ok(uid)
}

/// 投递所有到期的待发邮件。返回成功 / 失败数与失败原因。
#[command]
pub async fn mail_flush_outbox(app: AppHandle) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let now = chrono::Utc::now().timestamp();
    let due = store::take_due_outbox(now)?;
    let mut sent = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for (id, payload) in due {
        match serde_json::from_str::<SendRequest>(&payload) {
            Ok(request) => match send_inner(&app, &request).await {
                Ok(()) => {
                    let _ = store::mark_outbox_sent(&id);
                    sent += 1;
                }
                // 发送失败保留在队列里，等用户处理（避免静默丢信）。
                // 同时把认领时间归零，下一次轮询会立刻重试；否则要等 5 分钟认领窗口过期，
                // 用户会以为「点了立即发送没反应」。
                Err(e) => {
                    let attempts = store::list_outbox()
                        .unwrap_or_default()
                        .into_iter()
                        .find(|item| item.id == id)
                        .map(|item| item.attempts)
                        .unwrap_or(0);
                    if attempts >= OUTBOX_STAGE_AFTER {
                        // 反复失败：不再自动重试，转进发件箱让用户看到并处置。
                        if store::stage_outbox(&id, &e).is_ok() {
                            stage_outbox_into_folder(&request, &e);
                            errors.push(format!(
                                "「{}」多次发送失败，已放入发件箱：{e}",
                                request.subject.trim()
                            ));
                            continue;
                        }
                    }
                    // 指数退避：失败后不立刻重试，而是把下次尝试时间推后。
                    // 网络抖动时「每 90 秒猛撞一次」是最容易被服务器拉黑的形态。
                    match store::note_outbox_failure(&id, &e) {
                        Ok(delay) => errors.push(format!("{e}（{delay} 秒后重试）")),
                        Err(_) => errors.push(e),
                    }
                }
            },
            Err(e) => {
                errors.push(format!("待发数据损坏: {e}"));
                let _ = store::drop_outbox(&id);
            }
        }
    }
    Ok(serde_json::json!({ "sent": sent, "errors": errors }))
}

/// 保存草稿到服务器「草稿箱」。
///
/// 返回 `{ folder, uid }`：编辑器靠它把同一份草稿的上一版删掉再写，
/// 否则自动保存（每 15 秒一次）会在草稿箱里堆出一串同名草稿。
#[command]
pub async fn mail_save_draft(
    app: AppHandle,
    request: SendRequest,
    sync_after_append: Option<bool>,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let sync_after_append = sync_after_append.unwrap_or(true);
    let account = load_account(&request.account_id)?;
    // POP3 没有服务器端的草稿箱：草稿只落本地库的 Drafts，不发 IMAP APPEND。
    if pop3_sync::is_pop3(&account) {
        let uid = save_draft_locally(&account, request)?;
        return Ok(serde_json::json!({ "folder": "Drafts", "uid": uid }));
    }
    let password = account_password(&account)?;
    // 草稿和正式发送一样要展开通讯组：IMAP 草稿箱里的地址必须是真实邮箱。
    // 但「组芯片」本身要留着，否则续写草稿时会看到上百个地址——
    // 这正是写信页想避免的。原始收件人清单单独编码进一个自定义头。
    let original_recipients = serde_json::json!({
        "to": request.to.clone(),
        "cc": request.cc.clone(),
        "bcc": request.bcc.clone(),
    });
    let mut request = request;
    let mut expanded = contacts::store::expand_recipient_lists(&[
        request.to.clone(),
        request.cc.clone(),
        request.bcc.clone(),
    ])?;
    request.bcc = expanded.pop().unwrap_or_default();
    request.cc = expanded.pop().unwrap_or_default();
    request.to = expanded.pop().unwrap_or_default();
    let from = MailAddress {
        name: account.display_name.clone(),
        address: account.email.clone(),
    };
    let raw = mime::build_draft(&request, &from, &account.signature)?;
    let raw = insert_draft_recipients_header(raw, &original_recipients);
    let drafts = store::list_folders(&account.id)?
        .into_iter()
        .find(|f| f.role == FolderRole::Drafts)
        .map(|f| f.name);

    // Graph：MIME 草稿直接创建在 Exchange 草稿箱；续写 / 删除旧草稿走
    // `mail_discard_draft` 的 Graph 分支。同步标记对 Graph 没有意义。
    if graph::is_graph(&account) {
        let Some(drafts) = drafts else {
            return Err("Exchange 草稿箱还没有同步到本地，请刷新文件夹后重试".to_string());
        };
        if raw.len() > 4 * 1024 * 1024 {
            return Err(
                "Exchange Graph 草稿（含附件）不能超过约 4 MB；请先移除大附件再保存。".to_string(),
            );
        }
        let acc = account.clone();
        let folder = drafts.clone();
        let raw_for_draft = raw.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            graph::save_draft(&acc, &folder, &raw_for_draft)
        })
        .await
        .map_err(|e| format!("保存草稿任务失败: {e}"))?;
    }

    if ews::is_ews(&account) {
        if raw.len() > 20 * 1024 * 1024 {
            return Err("Exchange EWS 草稿（含附件）不能超过 20 MB；请先移除大附件再保存。".to_string());
        }
        let drafts = folder_name_by_role(&account.id, FolderRole::Drafts)?;
        let acc = account.clone();
        let raw_for_draft = raw.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            ews::save_draft(&acc, &drafts, &raw_for_draft)
        })
        .await
        .map_err(|e| format!("保存草稿任务失败: {e}"))?;
    }

    // 没有草稿箱、或者连不上服务器（离线 / OAuth 过期）：草稿先落到本机库的 Drafts。
    // 「只要没发出去就必须留下一份草稿」优先于「草稿必须在服务器上」——
    // 用户关掉编辑器却看到草稿箱空空如也，比草稿暂时只在本机严重得多。
    let Some(drafts) = drafts else {
        return save_draft_copy_locally(&account, &raw).await;
    };
    let acc = account.clone();
    let drafts_for_append = drafts.clone();
    let password_for_append = password.clone();
    let raw_for_append = raw.clone();
    let appended = tauri::async_runtime::spawn_blocking(move || {
        imap::append_to_folder(&acc, &password_for_append, &drafts_for_append, &raw_for_append, &["draft", "seen"])
    })
    .await
    .map_err(|e| format!("保存草稿任务失败: {e}"))?;
    if let Err(error) = appended {
        warn!(account = %account.email, error = %error, "mail: 草稿写服务器草稿箱失败，落到本机");
        return save_draft_copy_locally(&account, &raw).await;
    }

    // 关窗时的「最后一份草稿」不需要等这次同步：APPEND 成功就已经在服务器草稿箱里了，
    // 后面的增量同步只是为了拿 UID（编辑器用它在下一轮替换而不是追加）。让用户
    // 等这一两秒再关窗，表现就是「点保存并关闭很慢」。
    if !sync_after_append {
        let (acc, folder_name, secret) = (account.clone(), drafts.clone(), password.clone());
        tauri::async_runtime::spawn_blocking(move || {
            let Ok((uid_validity, _)) = store::folder_uid_state(&acc.id, &folder_name) else {
                return;
            };
            let Ok(last_uid) = store::last_uid(&acc.id, &folder_name) else {
                return;
            };
            let snippet_len = store::load_settings().snippet_len as usize;
            let _ = imap::sync_folder(
                &acc,
                &secret,
                &folder_name,
                uid_validity,
                last_uid,
                snippet_len,
            );
            refresh_folder_totals(&acc.id, &folder_name);
        });
        return Ok(serde_json::json!({ "folder": drafts, "uid": null, "deferred": true }));
    }

    // 立刻同步一次草稿箱，让它在侧栏里马上可见。
    // 只按 UID 增量拉，不重拉整个文件夹。
    let (uid_validity, _) = store::folder_uid_state(&account.id, &drafts)?;
    let last_uid = store::last_uid(&account.id, &drafts)?;
    let settings = store::load_settings();
    let acc = account.clone();
    let drafts_for_sync = drafts.clone();
    let snippet_len = settings.snippet_len as usize;
    let password_for_sync = password.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        imap::sync_folder(
            &acc,
            &password_for_sync,
            &drafts_for_sync,
            uid_validity,
            last_uid,
            snippet_len,
        )
    })
    .await;
    // 刚 append 的那封就是草稿箱里 UID 最大的那封：交给前端做「替换而不是追加」。
    // 同步没拉到（网络抖动 / 服务器延迟）时返回 null，前端会退回「下一轮再删」，
    // 宁可留一封重复草稿，也不要误删用户的正文。
    let new_uid = store::last_uid(&account.id, &drafts)?;
    let uid = if new_uid > last_uid { Some(new_uid) } else { None };
    // 侧栏的「草稿」数量读的是 folders 表：存完要重算，否则角标不动。
    refresh_folder_totals(&account.id, &drafts);
    Ok(serde_json::json!({ "folder": drafts, "uid": uid }))
}

/// 草稿落到**本机** Drafts（服务器不可达时的兜底，也是 POP3 的正常路径）。
async fn save_draft_copy_locally(
    account: &Account,
    raw: &[u8],
) -> Result<serde_json::Value, String> {
    let raw = raw.to_vec();
    let acc = account.clone();
    let uid = tauri::async_runtime::spawn_blocking(move || {
        pop3_sync::save_copy_to(&acc, "Drafts", &raw, false)
    })
    .await
    .map_err(|e| format!("保存草稿任务失败: {e}"))??;
    // 侧栏的「草稿」数量是从 folders 表读的：不重算一次，用户存完草稿看不到 +1。
    refresh_folder_totals(&account.id, "Drafts");
    Ok(serde_json::json!({ "folder": "Drafts", "uid": uid, "localOnly": true }))
}

// ---------------------------------------------------------------------------
// 规则
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_list_rules(app: AppHandle) -> Result<Vec<MailRule>, String> {
    ensure_init(&app)?;
    store::list_rules()
}

#[command]
pub async fn mail_save_rule(
    app: AppHandle,
    rule: MailRule,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let mut rule = rule;
    if rule.id.trim().is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }
    if rule.created_at == 0 {
        rule.created_at = chrono::Utc::now().timestamp();
    }
    // 更新已有规则时先记住旧版本：条件改了之后，旧目标文件夹里不再匹配的邮件
    // 要退回收件箱（用户要求的语义：文件夹内容跟随规则条件）。
    let previous = store::list_rules()?
        .into_iter()
        .find(|existing| existing.id == rule.id);
    store::upsert_rule(&rule)?;
    let restored = match previous {
        Some(old) => restore_unmatched_from_rule(&rule, &old).await,
        None => 0,
    };
    Ok(serde_json::json!({ "rule": rule, "restored": restored }))
}

/// 规则条件变化后，把旧目标文件夹里已经不匹配的邮件退回收件箱。
///
/// 返回实际搬回多少封。只在「规则仍然启用、且条件确实变了」时执行：
/// 改名、调整优先级、停用/启用都不会动邮件。
///
/// 没有记录「哪条规则把哪封邮件移进来的」，所以按目标文件夹的语义处理：
/// 这个文件夹是这条规则的归档区，条件改完后不满足的邮件就不该留在里面。
/// 只处理原本作用于收件箱的规则——作用在其它角色上的规则不会把邮件从收件箱
/// 搬走，把目标夹里的邮件塞回收件箱反而错位。
async fn restore_unmatched_from_rule(new_rule: &MailRule, old_rule: &MailRule) -> usize {
    if !new_rule.enabled {
        return 0;
    }
    let conditions_changed =
        serde_json::to_string(&new_rule.condition).ok() != serde_json::to_string(&old_rule.condition).ok();
    // 只有条件真的变了才回迁：改名、调优先级、停用/启用都不动邮件。
    if !conditions_changed {
        return 0;
    }
    let Some(target) = old_rule
        .action
        .move_to
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return 0;
    };
    let roles = &new_rule.condition.folder_roles;
    let inbox_scoped = roles.is_empty() || roles.iter().any(|role| role == "inbox" || role == "*");
    if !inbox_scoped {
        return 0;
    }

    let body_needed = rules::needs_body(std::slice::from_ref(new_rule));
    let mut restored = 0usize;
    for account in store::list_accounts().unwrap_or_default() {
        if !new_rule.condition.account_ids.is_empty()
            && !new_rule
                .condition
                .account_ids
                .iter()
                .any(|id| id == &account.id)
        {
            continue;
        }
        let folders = store::list_folders(&account.id).unwrap_or_default();
        let Some(target_folder) = folders.iter().find(|folder| folder.name == target) else {
            continue;
        };
        let Some(inbox) = folders.iter().find(|folder| folder.role == FolderRole::Inbox) else {
            continue;
        };
        if target_folder.role == FolderRole::Inbox {
            continue;
        }

        let rows = store::list_recent(&account.id, target, u32::MAX, 0).unwrap_or_default();
        let mut uids: Vec<u32> = Vec::new();
        for msg in rows.iter().filter(|msg| !msg.outgoing) {
            // 这些邮件当初是从收件箱被规则搬过来的，所以按 inbox 角色判断条件，
            // 不能用它现在所在的目标文件夹角色（否则文件夹范围条件会把所有邮件判成不匹配）。
            let cheap_ctx = rules::RuleContext {
                message: msg,
                body_preview: "",
                account: &account,
                folder_role: "inbox",
            };
            let body = if body_needed
                && rules::body_relevant(std::slice::from_ref(new_rule), &cheap_ctx)
            {
                store::get_body_text(&msg.account_id, &msg.folder, msg.uid).unwrap_or_default()
            } else {
                String::new()
            };
            let ctx = rules::RuleContext {
                message: msg,
                body_preview: &body,
                account: &account,
                folder_role: "inbox",
            };
            if rules::evaluate(std::slice::from_ref(new_rule), &ctx).is_empty() {
                uids.push(msg.uid);
            }
        }
        if uids.is_empty() {
            continue;
        }
        apply_rule_move_batch(&account, target, &uids, &inbox.name).await;

        // 移动失败时本地目标夹里的行还在，按「还剩哪些」数实际搬走的封数，
        // 与「对已有邮件执行规则」保持同一种如实口径。
        let alive: std::collections::HashSet<u32> =
            store::list_recent(&account.id, target, u32::MAX, 0)
                .unwrap_or_default()
                .into_iter()
                .map(|msg| msg.uid)
                .collect();
        let moved = uids.iter().filter(|uid| !alive.contains(uid)).count();
        if moved == 0 {
            continue;
        }
        restored += moved;
        // 收件箱那份在服务器上，本地要再同步一次才看得到（POP3 是本地搬行，不需要）。
        if !pop3_sync::is_pop3(&account) {
            let _ = sync_folder_once(&account, &inbox.name).await;
        }
        refresh_folder_totals(&account.id, target);
        refresh_folder_totals(&account.id, &inbox.name);
    }
    restored
}

/// 把规则**立即**应用到库里已有的邮件上。
///
/// 规则平时只在「同步到新邮件」时跑一遍，所以新建一条规则后老邮件纹丝不动，
/// 用户建完文件夹点进去是空的（真实反馈）。这里按规则的范围扫一遍本地库，
/// 复用与新邮件完全相同的执行路径（移动 / 标记 / 打标签 / 转发）。
#[command]
pub async fn mail_apply_rules_to_existing(
    app: AppHandle,
    account_id: String,
    rule_id: Option<String>,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let wanted = rule_id.filter(|id| !id.trim().is_empty());
    let picked: Vec<MailRule> = store::list_rules()?
        .into_iter()
        .filter(|r| r.enabled)
        .filter(|r| match &wanted {
            Some(id) => &r.id == id,
            None => true,
        })
        .collect();
    if picked.is_empty() {
        return Ok(serde_json::json!({ "matched": 0, "scanned": 0 }));
    }
    let folders = store::list_folders(&account.id)?;
    // 规则限定了角色就只扫那些文件夹；空 = 收件箱；`*` = 全部文件夹。
    let roles: Vec<String> = picked
        .iter()
        .flat_map(|r| r.condition.folder_roles.clone())
        .collect();
    let all_roles = roles.iter().any(|role| role == "*");
    let targets: Vec<&MailFolder> = folders
        .iter()
        // 只排除发件箱（本机待发队列）。以前用 `!local_only`：POP3 账户的文件夹
        // 全都是本地的，结果一封邮件都扫不到，报「没有命中」——用户明明有邮件。
        .filter(|f| f.role != FolderRole::Outbox)
        .filter(|f| {
            all_roles
                || roles.is_empty()
                    && f.role == FolderRole::Inbox
                    || !roles.is_empty() && roles.iter().any(|role| *role == f.role.as_str())
        })
        .collect();

    let body_needed = rules::needs_body(&picked);
    let mut scanned = 0usize;
    let mut matched = 0usize;
    let mut batch: Vec<MessageSummary> = Vec::new();
    for folder in targets {
        let Ok(rows) = store::list_recent(&account.id, &folder.name, u32::MAX, 0) else {
            continue;
        };
        for msg in rows {
            scanned += 1;
            if msg.outgoing {
                continue;
            }
            let cheap_ctx = rules::RuleContext {
                message: &msg,
                body_preview: "",
                account: &account,
                folder_role: folder.role.as_str(),
            };
            let body = if body_needed && rules::body_relevant(&picked, &cheap_ctx) {
                store::get_body_text(&msg.account_id, &msg.folder, msg.uid).unwrap_or_default()
            } else {
                String::new()
            };
            let ctx = rules::RuleContext {
                message: &msg,
                body_preview: &body,
                account: &account,
                folder_role: folder.role.as_str(),
            };
            if rules::evaluate(&picked, &ctx).is_empty() {
                continue;
            }
            matched += 1;
            batch.push(msg);
        }
    }
    let mut moved = 0usize;
    if !batch.is_empty() {
        // 源夹先记下来：移动成功的行会被删掉，事后要靠它核对到底搬走了几封。
        let mut sources: Vec<String> = batch.iter().map(|m| m.folder.clone()).collect();
        sources.sort();
        sources.dedup();
        apply_rules_to_new(&account, &batch, &picked).await;
        if picked
            .iter()
            .any(|r| r.action.move_to.is_some() || r.action.delete)
        {
            // 「命中」不等于「搬走了」：离线时 IMAP MOVE 整批失败，只报命中数会让用户
            // 以为规则坏了。按源夹重新数一遍还剩哪些，差值才是真的处理掉了。
            for folder in &sources {
                let alive: std::collections::HashSet<u32> = store::list_recent(&account.id, folder, u32::MAX, 0)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|m| m.uid)
                    .collect();
                moved += batch
                    .iter()
                    .filter(|m| &m.folder == folder && !alive.contains(&m.uid))
                    .count();
            }
        } else {
            moved = matched;
        }
        // 移动 / 标记都改了本地行：把涉及的文件夹计数校正一次，侧栏才不会停在旧数字。
        let mut touched: Vec<String> = sources;
        // 目标夹里只有服务器上的那一封，本地库还没有对应的行：不补这一次同步，
        // 用户点进刚在规则里新建的文件夹看到的仍然是空的（真实反馈）。
        if !pop3_sync::is_pop3(&account) {
            for dest in rule_target_folders(&picked) {
                if sync_folder_once(&account, &dest).await {
                    touched.push(dest);
                }
            }
        }
        touched.sort();
        touched.dedup();
        for folder in touched {
            refresh_folder_totals(&account.id, &folder);
        }
    }
    Ok(serde_json::json!({ "matched": matched, "scanned": scanned, "moved": moved }))
}

/// 这批规则会把邮件送去哪些文件夹（移动与复制的目标，去重后返回）。
fn rule_target_folders(rules: &[MailRule]) -> Vec<String> {
    let mut out: Vec<String> = rules
        .iter()
        .flat_map(|r| [r.action.move_to.clone(), r.action.copy_to.clone()])
        .flatten()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// 增量拉一个文件夹，让刚被规则移进去的邮件立刻在本地可见。
/// 离线 / 认证过期就返回 false：这次没同步成，等常规同步再补，不影响已完成的移动。
async fn sync_folder_once(account: &Account, folder: &str) -> bool {
    let snippet_len = store::load_settings().snippet_len as usize;
    if graph::is_graph(account) {
        let acc = account.clone();
        let name = folder.to_string();
        let synced = tauri::async_runtime::spawn_blocking(move || {
            graph::sync_folder(&acc, &name, snippet_len).map(|_| ())
        })
        .await
        .map(|result| result.is_ok())
        .unwrap_or(false);
        if !synced {
            warn!(account = %account.email, folder = %folder, "mail: 规则目标文件夹同步失败（Graph）");
        }
        return synced;
    }
    if ews::is_ews(account) {
        let acc = account.clone();
        let name = folder.to_string();
        let synced = tauri::async_runtime::spawn_blocking(move || {
            ews::sync_folder(&acc, &name, snippet_len).map(|_| ())
        })
        .await
        .map(|result| result.is_ok())
        .unwrap_or(false);
        if !synced {
            warn!(account = %account.email, folder = %folder, "mail: 规则目标文件夹同步失败（EWS）");
        }
        return synced;
    }
    let Ok(password) = account_password(account) else {
        return false;
    };
    let Ok((uid_validity, _)) = store::folder_uid_state(&account.id, folder) else {
        return false;
    };
    let Ok(last_uid) = store::last_uid(&account.id, folder) else {
        return false;
    };
    let acc = account.clone();
    let name = folder.to_string();
    let synced = tauri::async_runtime::spawn_blocking(move || {
        imap::sync_folder(&acc, &password, &name, uid_validity, last_uid, snippet_len)
    })
    .await
    .map(|result| result.is_ok())
    .unwrap_or(false);
    if !synced {
        warn!(account = %account.email, folder = %folder, "mail: 规则目标文件夹同步失败");
    }
    synced
}

#[command]
pub async fn mail_delete_rule(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app)?;
    store::delete_rule(&id)
}

// ---------------------------------------------------------------------------
// 设置与主题
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_get_settings(app: AppHandle) -> Result<MailSettings, String> {
    ensure_init(&app)?;
    Ok(store::load_settings())
}

#[command]
pub async fn mail_set_settings(app: AppHandle, settings: MailSettings) -> Result<(), String> {
    ensure_init(&app)?;
    store::save_settings(&settings)
}

/// 主题 token 清单：前端据此渲染「自定义主题」编辑器的字段。
#[command]
pub async fn mail_theme_tokens() -> Result<Vec<&'static str>, String> {
    Ok(theme::TOKENS.to_vec())
}

/// 内置主题（两套明暗）+ 用户自定义主题。
#[command]
pub async fn mail_builtin_themes(app: AppHandle) -> Result<Vec<model::CustomTheme>, String> {
    ensure_init(&app)?;
    Ok(theme::builtin_themes())
}

/// 清空本地邮件缓存（不动服务器）。账户与设置保留。
#[command]
pub async fn mail_clear_cache(app: AppHandle, account_id: Option<String>) -> Result<(), String> {
    ensure_init(&app)?;
    store::clear_cache(account_id.as_deref())?;
    // 缓存清空后本地 UID 起点回到 0，连接池里的会话没有状态问题，
    // 但文件夹的 `flags_synced_at` 已无意义——清掉，让下次同步立刻做全量比对。
    if let Some(id) = account_id.as_deref() {
        for folder in store::list_folders(id).unwrap_or_default() {
            store::touch_folder_flags_synced(id, &folder.name, 0);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 导出 / 笔记桥 / 任务桥 / 草稿暂存
// ---------------------------------------------------------------------------

/// 把一封邮件组装成导出用的输入。正文优先取缓存，缺失时向服务器补拉。
async fn export_input(
    account_id: &str,
    folder: &str,
    uid: u32,
) -> Result<export::ExportInput, String> {
    let summary = store::get_summary(account_id, folder, uid)?
        .ok_or_else(|| "本地没有这封邮件的记录".to_string())?;

    let cached = store::get_body(account_id, folder, uid)?;
    let (html, text) = match cached {
        Some((html, text, _, true)) if html.is_some() || !text.is_empty() => (html, text),
        _ => {
            let account = load_account(account_id)?;
            let password = account_password(&account)?;
            let raw = fetch_raw_bytes(&account, &password, folder, uid).await?;
            let parsed = mime::parse(&raw, false).ok_or("无法解析邮件内容")?;
            (parsed.html, parsed.text)
        }
    };

    let date = chrono::DateTime::from_timestamp(summary.date, 0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default();

    // 地址显示成 "Name <addr>"，无名时只留地址。
    let fmt_addr = |a: &model::MailAddress| {
        if a.name.trim().is_empty() {
            a.address.clone()
        } else {
            format!("{} <{}>", a.name, a.address)
        }
    };
    let to = summary.to_addrs().iter().map(fmt_addr).collect::<Vec<_>>().join(", ");
    let cc = summary.cc_addrs().iter().map(fmt_addr).collect::<Vec<_>>().join(", ");

    Ok(export::ExportInput {
        subject: summary.subject.clone(),
        from_name: summary.from_name.clone(),
        from_addr: summary.from_addr.clone(),
        to,
        cc,
        date,
        text,
        html,
        labels: summary.labels.clone(),
        account_id: account_id.to_string(),
        folder: folder.to_string(),
        uid,
    })
}

/// 导出单封邮件到文件。`format` = markdown | html | pdf | word | text。
#[command]
pub async fn mail_export_message(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    format: String,
    target_path: String,
) -> Result<String, String> {
    ensure_init(&app)?;
    if target_path.trim().is_empty() {
        return Err("目标路径为空".to_string());
    }
    let fmt = export::ExportFormat::parse(&format);
    let input = export_input(&account_id, &folder, uid).await?;

    match fmt {
        export::ExportFormat::Markdown => {
            let content = export::to_markdown(&input);
            std::fs::write(&target_path, content).map_err(|e| format!("写入失败: {e}"))?;
        }
        export::ExportFormat::Html => {
            let content = export::to_html(&input);
            std::fs::write(&target_path, content).map_err(|e| format!("写入失败: {e}"))?;
        }
        export::ExportFormat::Text => {
            let content = export::to_text(&input);
            std::fs::write(&target_path, content).map_err(|e| format!("写入失败: {e}"))?;
        }
        export::ExportFormat::Word => {
            let bytes = export::to_docx(&input)?;
            std::fs::write(&target_path, bytes).map_err(|e| format!("写入失败: {e}"))?;
        }
        export::ExportFormat::Pdf => {
            let md = export::to_markdown(&input);
            let typst = export::markdown_to_typst(&md);
            // 编译根目录用临时目录：邮件导出不需要 @preview 包与本地相对资源。
            let root = std::env::temp_dir();
            let bytes = tauri::async_runtime::spawn_blocking(move || {
                crate::commands::typst_compile::compile_typ_source_to_pdf(root, typst)
            })
            .await
            .map_err(|e| format!("PDF 编译任务失败: {e}"))??;
            std::fs::write(&target_path, bytes).map_err(|e| format!("写入失败: {e}"))?;
        }
    }
    Ok(target_path)
}

/// 把邮件正文保存进笔记库（Markdown 文件，带 front matter）。
///
/// 直接写文件而不是走 `update_note`：笔记库本来就以磁盘文件为事实来源，
/// 写进去之后既有的文件监听与索引会自然收录，不需要第二套入库逻辑。
#[command]
pub async fn mail_save_to_notes(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    subdir: Option<String>,
) -> Result<String, String> {
    ensure_init(&app)?;
    let root = crate::commands::settingview::get_libraries_root();
    if root.trim().is_empty() {
        return Err("还没有设置笔记库目录（设置 → 笔记库）".to_string());
    }
    let input = export_input(&account_id, &folder, uid).await?;
    let markdown = export::to_markdown(&input);

    let mut dir = std::path::PathBuf::from(root.trim());
    if let Some(sub) = subdir.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        // 只允许一层字面目录名，防止 `../` 逃出笔记库。
        if sub.contains("..") || sub.contains('/') || sub.contains('\\') {
            return Err("目录名不能包含路径分隔符".to_string());
        }
        dir = dir.join(sub);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {e}"))?;

    let stem = sanitize_filename(&format!(
        "{}-{}",
        chrono::Local::now().format("%Y%m%d"),
        if input.subject.trim().is_empty() {
            "邮件".to_string()
        } else {
            input.subject.clone()
        }
    ));
    let mut path = dir.join(format!("{stem}.md"));
    let mut index = 1;
    while path.exists() {
        path = dir.join(format!("{stem}-{index}.md"));
        index += 1;
        if index > 999 {
            return Err("同名文件过多，请换一个目录".to_string());
        }
    }
    std::fs::write(&path, markdown).map_err(|e| format!("写入笔记失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 把邮件转为任务管理器的一条任务。
///
/// 走 `task_import` 的 org 通道（任务系统唯一的事实来源是 .org 文件），
/// 不直接写它的 SQLite 索引，避免两套写入路径打架。
#[command]
pub async fn mail_to_task(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
    category: Option<String>,
    status: Option<String>,
    deadline: Option<String>,
) -> Result<u32, String> {
    ensure_init(&app)?;
    let input = export_input(&account_id, &folder, uid).await?;
    let cat = category.clone().unwrap_or_else(|| "work".into());
    let org = export::mail_to_task_org(
        &input,
        &cat,
        status.as_deref().unwrap_or("TODO"),
        deadline.as_deref().unwrap_or(""),
    );
    let import = crate::commands::taskmanager::TaskImportInput {
        category: cat,
        target_file_name: "mail-inbox.org".to_string(),
        source_kind: "org".to_string(),
        content: org,
    };
    let count = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::taskmanager::import_tasks(import)
    })
    .await
    .map_err(|e| format!("导入任务失败: {e}"))??;
    Ok(count as u32)
}

/// 从任务/其它模块跳回原邮件：把邮件窗口带到前台，并让它打开指定邮件。
///
/// 邮件跑在独立窗口（label `app-tools-mail`）里，无法跨窗口直接操作它的 store，
/// 所以这里负责「唤起窗口 + 广播定位」，具体打开由窗口自己完成。
#[command]
pub async fn mail_open_origin(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    let label = "app-tools-mail";
    let Some(window) = app.get_webview_window(label) else {
        return Err("邮件窗口没有打开，请先在应用页打开「邮件」".to_string());
    };
    window.show().map_err(|e| format!("显示窗口失败: {e}"))?;
    window.unminimize().ok();
    window.set_focus().ok();
    // 窗口已在前台但前端可能还没就绪：事件可能丢失，所以前端自己也会读一次待打开队列。
    window
        .emit(
            "mail-open-origin",
            serde_json::json!({
                "accountId": account_id,
                "folder": folder,
                "uid": uid,
            }),
        )
        .map_err(|e| format!("通知邮件窗口失败: {e}"))?;
    Ok(())
}

/// 删除一条已存草稿（续写后保存 / 发送成功时调用，避免草稿箱重复堆积）。
#[command]
pub async fn mail_discard_draft(
    app: AppHandle,
    account_id: String,
    folder: String,
    uid: u32,
) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    if graph::is_graph(&account) {
        let acc = account.clone();
        let folder_for_delete = folder.clone();
        tauri::async_runtime::spawn_blocking(move || {
            graph::discard_draft(&acc, &folder_for_delete, uid)
        })
        .await
        .map_err(|e| format!("删除草稿任务失败: {e}"))??;
        let _ = store::delete_message(&account_id, &folder, uid);
        return Ok(());
    }
    if ews::is_ews(&account) {
        let acc = account.clone();
        let folder_for_delete = folder.clone();
        tauri::async_runtime::spawn_blocking(move || {
            ews::discard_draft(&acc, &folder_for_delete, uid)
        })
        .await
        .map_err(|e| format!("删除草稿任务失败: {e}"))??;
        let _ = store::delete_message(&account_id, &folder, uid);
        return Ok(());
    }
    // POP3 的草稿只在本机：删掉本地行就是全部。
    if pop3_sync::is_pop3(&account) {
        let _ = store::delete_message(&account_id, &folder, uid);
        return Ok(());
    }
    let password = account_password(&account)?;
    let (f, u) = (folder.clone(), uid);
    let acc = account.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        imap::delete_permanently(&acc, &password, &f, &[u])
    })
    .await;
    let _ = store::delete_message(&account_id, &folder, uid);
    Ok(())
}

/// 暂存草稿到本地（不发送）。关闭编辑器 / 切账户后仍可续写。
#[command]
pub async fn mail_stash_draft(app: AppHandle, draft: serde_json::Value) -> Result<String, String> {
    ensure_init(&app)?;
    let dir = store::data_root(&app)?.join("drafts");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建草稿目录失败: {e}"))?;
    let id = draft
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let path = dir.join(format!("{id}.json"));
    let payload = serde_json::json!({
        "id": id,
        "savedAt": chrono::Utc::now().timestamp(),
        "draft": draft,
    });
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("保存草稿失败: {e}"))?;
    Ok(id)
}

/// 列出本地暂存草稿（按保存时间倒序）。
#[command]
pub async fn mail_list_stashed_drafts(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    ensure_init(&app)?;
    let dir = store::data_root(&app)?.join("drafts");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut items: Vec<serde_json::Value> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                items.push(value);
            }
        }
    }
    items.sort_by_key(|v| std::cmp::Reverse(v.get("savedAt").and_then(|x| x.as_i64()).unwrap_or(0)));
    Ok(items)
}

#[command]
pub async fn mail_delete_stashed_draft(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app)?;
    let path = store::data_root(&app)?.join("drafts").join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("删除草稿失败: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// OAuth2
// ---------------------------------------------------------------------------

/// 发起授权：返回给用户一个授权页地址。
/// - Gmail：设备码流程（短码 + 网页输入）。
/// - Outlook：授权码流程 + 本机 localhost 回调（避免设备码页面把浏览器导向
///   127.0.0.1 的错误页）。回调线程拿到令牌后直接写库，`mail_oauth_poll` 只等结果。
#[command]
pub async fn mail_oauth_start(
    app: AppHandle,
    provider: String,
    client_id: Option<String>,
    scope: Option<String>,
    account_id: Option<String>,
) -> Result<oauth::DeviceCode, String> {
    ensure_init(&app)?;
    let provider = oauth::provider_for(&provider)
        .ok_or_else(|| format!("不支持的 OAuth 服务商: {provider}"))?;
    if provider.id == "outlook" {
        let account_id = account_id.unwrap_or_default();
        return oauth::start_loopback_flow(
            &provider,
            &account_id,
            client_id.as_deref(),
            scope.as_deref(),
        );
    }
    tauri::async_runtime::spawn_blocking(move || {
        oauth::start_device_flow(&provider, client_id.as_deref(), scope.as_deref())
    })
    .await
    .map_err(|e| format!("授权任务失败: {e}"))?
}

/// 轮询设备码换取令牌。成功后写进 keychain，并把账户标记为 oauth2。
#[command]
pub async fn mail_oauth_poll(
    app: AppHandle,
    account_id: String,
    provider: String,
    device_code: String,
    interval: Option<u64>,
    client_id: Option<String>,
) -> Result<Option<String>, String> {
    ensure_init(&app)?;
    let account_id = account_id.trim().to_string();
    // 令牌必须挂在真实账户上。前端如果还没拿到保存后的 id（例如设备码流程里
    // 先轮询后落库），这里宁可直接失败，也不要把令牌写到一个空 id 的凭据条目里——
    // 那会让用户看到「账户已存在，但永远提示未授权」。
    let mut account = store::get_account(&account_id)?
        .ok_or_else(|| "账户还没有保存完成，无法写入授权令牌".to_string())?;
    let provider_info = oauth::provider_for(&provider)
        .ok_or_else(|| format!("不支持的 OAuth 服务商: {provider}"))?;
    // Outlook 走 localhost 回调：令牌已经由回调线程保存，这里只等它结束。
    if provider_info.id == "outlook" {
        let flow_id = device_code;
        tauri::async_runtime::spawn_blocking(move || oauth::wait_loopback_flow(&flow_id))
            .await
            .map_err(|e| format!("等待授权失败: {e}"))??;
        pool::invalidate(&account_id);
        // 回调线程可能已经从 id_token 里补上了邮箱：把最新的地址带回前端。
        let email = store::get_account(&account_id)?
            .map(|a| a.email.trim().to_string())
            .filter(|e| !e.is_empty());
        return Ok(email);
    }
    let (tokens, oauth_email) = tauri::async_runtime::spawn_blocking(move || {
        oauth::poll_device_token(
            &provider_info,
            client_id.as_deref(),
            &device_code,
            interval.unwrap_or(5),
        )
    })
    .await
    .map_err(|e| format!("等待授权失败: {e}"))??;

    oauth::save_tokens(&account_id, &tokens)?;
    // 账户开启 OAuth2 并把提供商标记好。
    account.auth_kind = "oauth2".to_string();
    account.oauth_provider = provider;
    account.oauth_expires = tokens.expires_at;
    // 用户没填邮箱时用授权结果里的真实地址补上。
    if (account.email.trim().is_empty() || !account.email.contains('@')) && oauth_email.is_some() {
        account.email = oauth_email.unwrap_or_default();
    }
    store::upsert_account(&account)?;
    pool::invalidate(&account_id);
    let email = account.email.trim().to_string();
    Ok((!email.is_empty()).then_some(email))
}

/// 手动完成 Outlook 授权：浏览器没能自动回到本机时，把回调地址里的 code 交给
/// 后端完成「授权码换令牌」。`flow_id` 用来找到同一次授权会话并唤醒等待方。
#[command]
pub async fn mail_oauth_complete(
    app: AppHandle,
    account_id: String,
    flow_id: String,
    provider: String,
    redirect_uri: String,
    code: String,
) -> Result<Option<String>, String> {
    ensure_init(&app)?;
    if store::get_account(&account_id)?.is_none() {
        return Err("账户还没有保存完成，无法写入授权令牌".to_string());
    }
    let provider = oauth::provider_for(&provider)
        .ok_or_else(|| format!("不支持的 OAuth 服务商: {provider}"))?;
    let flow_account = account_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        oauth::complete_loopback_flow(&flow_id, &flow_account, &provider, &redirect_uri, &code)
    })
    .await
    .map_err(|e| format!("完成授权失败: {e}"))??;
    let email = store::get_account(&account_id)?
        .map(|a| a.email.trim().to_string())
        .filter(|e| !e.is_empty());
    Ok(email)
}

/// 退出 OAuth：清掉令牌并回到密码模式。
#[command]
pub async fn mail_oauth_sign_out(app: AppHandle, account_id: String) -> Result<(), String> {    ensure_init(&app)?;
    oauth::delete_tokens(&account_id);
    if let Ok(Some(mut account)) = store::get_account(&account_id) {
        account.auth_kind = "password".to_string();
        account.oauth_provider.clear();
        account.oauth_expires = 0;
        store::upsert_account(&account)?;
    }
    pool::invalidate(&account_id);
    Ok(())
}

#[command]
pub async fn mail_oauth_status(
    app: AppHandle,
    account_id: String,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let tokens = oauth::load_tokens(&account_id);
    Ok(serde_json::json!({
        "connected": oauth::has_tokens(&account_id),
        "expiresAt": tokens.as_ref().map(|t| t.expires_at).unwrap_or(0),
        "expired": tokens.as_ref().map(|t| t.expires_at < chrono::Utc::now().timestamp()).unwrap_or(false),
    }))
}

// ---------------------------------------------------------------------------
// 离线队列 / 连接状态
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_offline_list(
    app: AppHandle,
    account_id: Option<String>,
) -> Result<Vec<offline::OfflineOp>, String> {
    ensure_init(&app)?;
    offline::list(account_id.as_deref())
}

#[command]
pub async fn mail_offline_count(app: AppHandle, account_id: String) -> Result<i64, String> {
    ensure_init(&app)?;
    offline::count(&account_id)
}

/// 重放离线队列。返回重放成功 / 失败数与剩余条数。
#[command]
pub async fn mail_offline_replay(
    app: AppHandle,
    account_id: Option<String>,
) -> Result<offline::ReplayResult, String> {
    ensure_init(&app)?;
    let accounts: Vec<Account> = match account_id.as_deref() {
        Some(id) => vec![load_account(id)?],
        None => store::list_accounts()?,
    };
    let mut combined = offline::ReplayResult::default();
    for account in accounts {
        // POP3 的写操作全是本地的，进不了队列；万一有（老队列 / 手工导入），
        // 在这里直接丢弃，避免无限重放一个永远会失败的服务器操作。
        if pop3_sync::is_pop3(&account) || graph::is_graph(&account) || ews::is_ews(&account) {
            for op in offline::list(Some(&account.id))? {
                let _ = offline::remove(&op.id);
            }
            continue;
        }
        let ops = offline::list(Some(&account.id))?;
        for op in ops {
            if op.attempts >= 5 {
                continue;
            }
            let acc = account.clone();
            let folder = op.folder.clone();
            let uids = op.uids.clone();
            let kind = op.kind.clone();
            let payload = op.payload.clone();
            let value = op.value;
            let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
                match kind.as_str() {
                    "seen" => imap::set_flags(&acc, "", &folder, &uids, Some(value), None),
                    "flagged" => imap::set_flags(&acc, "", &folder, &uids, None, Some(value)),
                    "move" => imap::move_messages(&acc, "", &folder, &uids, &payload),
                    "delete" => imap::delete_permanently(&acc, "", &folder, &uids),
                    other => Err(format!("未知的离线操作: {other}")),
                }
            })
            .await
            .map_err(|e| format!("重放任务失败: {e}"))?;
            match result {
                Ok(()) => {
                    let _ = offline::remove(&op.id);
                    combined.replayed += 1;
                    if op.kind == "move" || op.kind == "delete" {
                        for uid in &op.uids {
                            let _ = store::delete_message(&account.id, &op.folder, *uid);
                        }
                    }
                }
                Err(e) => {
                    combined.failed += 1;
                    if combined.errors.len() < 5 {
                        combined.errors.push(e);
                    }
                }
            }
        }
        combined.remaining += offline::count(&account.id).unwrap_or(0);
    }
    Ok(combined)
}

/// 连接与队列状态（顶栏的「离线 / 待发 N 条」就靠它）。
#[command]
pub async fn mail_connection_status(
    app: AppHandle,
    account_id: Option<String>,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let queued = match account_id.as_deref() {
        Some(id) => offline::count(id).unwrap_or(0),
        None => store::list_accounts()?
            .iter()
            .map(|a| offline::count(&a.id).unwrap_or(0))
            .sum(),
    };
    let pending_send = store::list_outbox().map(|v| v.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "offlineQueued": queued,
        "pendingSend": pending_send,
        "busyAccounts": sync::busy_accounts(),
        "idleActive": account_id
            .as_deref()
            .map(pool::idle::is_running)
            .unwrap_or(false),
        "pooled": pool::pooled_accounts(),
    }))
}

/// 启动某个账户的 IDLE 推送（打开邮件页时调用）。收到变更后前端会刷新。
#[command]
pub async fn mail_start_idle(
    app: AppHandle,
    account_id: String,
    folder: Option<String>,
) -> Result<bool, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    // POP3 没有 IDLE（RFC 2177 是 IMAP 扩展）：保持轮询。
    // Graph 第一版也走轮询（后续可接 webhook/subscription）。
    if pop3_sync::is_pop3(&account) || graph::is_graph(&account) || ews::is_ews(&account) {
        return Ok(false);
    }
    let settings = store::load_settings();
    if settings.fetch_mode != "idle" {
        return Ok(false);
    }
    // 服务器不支持 IDLE 时不要硬起线程：`capabilities` 里没有 IDLE 就退回轮询。
    let supports = tauri::async_runtime::spawn_blocking({
        let acc = account.clone();
        move || pool::lease(&acc).map(|l| l.supports("IDLE"))
    })
    .await
    .map_err(|e| format!("检查 IDLE 能力失败: {e}"))??;
    if !supports {
        info!(account = %account.email, "mail: 服务器不支持 IDLE，继续用轮询");
        return Ok(false);
    }
    let target = folder
        .filter(|f| !f.trim().is_empty())
        .unwrap_or_else(|| "INBOX".to_string());
    let app_handle = app.clone();
    let acc_id = account.id.clone();
    pool::idle::start(account, target, move || {
        let _ = app_handle.emit(
            "mail-idle-change",
            serde_json::json!({ "accountId": acc_id }),
        );
    })
    .map_err(|e| format!("启动 IMAP IDLE 失败: {e}"))?;
    Ok(true)
}

#[command]
pub async fn mail_stop_idle(app: AppHandle, account_id: String) -> Result<(), String> {
    ensure_init(&app)?;
    pool::idle::stop(&account_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// 共享邮箱 / 委托访问
// ---------------------------------------------------------------------------

/// 列出某个共享邮箱下的文件夹，并把它们挂到账户的文件夹表里（`role = custom`，
/// 名字保留服务器前缀）。
///
/// 共享邮箱在 IMAP 上没有「登录」这一步：用你自己的凭据登录后直接
/// `SELECT "Other Users/team@corp.com/Inbox"`。所以本地不建独立账户，
/// 而是把这些带前缀的文件夹挂在主账户下——同步、搜索、规则全部复用现有路径。
#[command]
pub async fn mail_list_delegate_folders(
    app: AppHandle,
    account_id: String,
    mailbox: String,
) -> Result<Vec<MailFolder>, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    if graph::is_graph(&account) || ews::is_ews(&account) {
        return Err("原生 Exchange 第一版暂不支持共享邮箱 / 委托访问".to_string());
    }
    if !account.delegate.iter().any(|d| d.email.eq_ignore_ascii_case(&mailbox)) {
        return Err(format!("{mailbox} 不在该账户的共享邮箱列表里"));
    }
    let secret = account_password(&account)?;
    let mut folders = tauri::async_runtime::spawn_blocking({
        let acc = account.clone();
        let box_name = mailbox.clone();
        move || imap::list_delegate_folders(&acc, &secret, &box_name)
    })
    .await
    .map_err(|e| format!("读取共享邮箱失败: {e}"))??;
    imap::mark_delegate_roles(&mut folders, &mailbox);

    // 合并到账户的文件夹表：已有的同名前缀文件夹替换，其余保留。
    let mut all = store::list_folders(&account_id)?;
    let incoming: std::collections::HashSet<String> =
        folders.iter().map(|f| f.name.clone()).collect();
    all.retain(|f| !incoming.contains(&f.name));
    all.extend(folders.iter().cloned());
    store::replace_folders(&account_id, &all)?;
    Ok(folders)
}

// ---------------------------------------------------------------------------
// 模板 / 收发件人名单 / 附件检查
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_list_templates(app: AppHandle) -> Result<Vec<model::MailTemplate>, String> {
    ensure_init(&app)?;
    store::list_templates()
}

#[command]
pub async fn mail_save_template(
    app: AppHandle,
    template: model::MailTemplate,
) -> Result<model::MailTemplate, String> {
    ensure_init(&app)?;
    let mut template = template;
    if template.id.trim().is_empty() {
        template.id = Uuid::new_v4().to_string();
    }
    if template.created_at == 0 {
        template.created_at = chrono::Utc::now().timestamp();
    }
    store::upsert_template(&template)?;
    Ok(template)
}

#[command]
pub async fn mail_delete_template(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app)?;
    store::delete_template(&id)
}

#[command]
pub async fn mail_list_sender_rules(app: AppHandle) -> Result<Vec<model::SenderRule>, String> {
    ensure_init(&app)?;
    store::list_sender_rules()
}

#[command]
pub async fn mail_save_sender_rule(
    app: AppHandle,
    rule: model::SenderRule,
) -> Result<model::SenderRule, String> {
    ensure_init(&app)?;
    let mut rule = rule;
    if rule.id.trim().is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }
    if rule.pattern.trim().is_empty() {
        return Err("名单条目不能为空".to_string());
    }
    if !matches!(rule.kind.as_str(), "allow" | "block") {
        return Err("名单类型只能是 allow 或 block".to_string());
    }
    if rule.created_at == 0 {
        rule.created_at = chrono::Utc::now().timestamp();
    }
    store::upsert_sender_rule(&rule)?;
    Ok(rule)
}

#[command]
pub async fn mail_delete_sender_rule(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app)?;
    store::delete_sender_rule(&id)
}

/// 发送前的附件检查（大小上限 + 危险类型）。
#[command]
pub async fn mail_check_attachments(
    app: AppHandle,
    paths: Vec<String>,
) -> Result<Vec<model::AttachmentCheck>, String> {
    ensure_init(&app)?;
    let settings = store::load_settings();
    Ok(mime::check_attachments(&paths, settings.attachment_limit_mb))
}

#[command]
pub async fn mail_is_dangerous_attachment(name: String) -> Result<bool, String> {
    Ok(notify::is_dangerous_filename(&name))
}

// ---------------------------------------------------------------------------
// Sieve 服务器端规则
// ---------------------------------------------------------------------------

/// 预演：生成 Sieve 脚本但不上传（设置页的「查看将要上传的脚本」）。
#[command]
pub async fn mail_sieve_preview(
    app: AppHandle,
    account_id: String,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let rules = store::list_rules()?;
    let trash = store::list_folders(&account_id)?
        .into_iter()
        .find(|f| f.role == FolderRole::Trash)
        .map(|f| f.name);
    let (script, warnings) = sieve::generate(&account, &rules, trash.as_deref());
    Ok(serde_json::json!({ "script": script, "warnings": warnings }))
}

/// 把规则镜像到服务器。
#[command]
pub async fn mail_sieve_sync(app: AppHandle, account_id: String) -> Result<Vec<String>, String> {
    ensure_init(&app)?;
    let mut account = load_account(&account_id)?;
    let config = account
        .sieve
        .clone()
        .filter(|c| c.mode != "off")
        .ok_or_else(|| "还没有启用服务器端规则（设置 → 账户 → 服务器端规则）".to_string())?;
    let rules = store::list_rules()?;
    let trash = store::list_folders(&account_id)?
        .into_iter()
        .find(|f| f.role == FolderRole::Trash)
        .map(|f| f.name);
    let secret = account_password(&account)?;
    let result = tauri::async_runtime::spawn_blocking({
        let acc = account.clone();
        let cfg = config.clone();
        move || sieve::sync_rules(&acc, &cfg, &rules, trash.as_deref(), &secret)
    })
    .await
    .map_err(|e| format!("上传服务器端规则失败: {e}"))?;

    // 成功与失败都记录：失败原因（「这台服务器不支持 ManageSieve」）必须留在
    // 账户配置里，否则用户每次点上传都只会看到一个瞬时弹窗，回来看不出上周就失败了。
    if let Some(cfg) = account.sieve.as_mut() {
        cfg.last_sync_at = chrono::Utc::now().timestamp();
        cfg.last_error = match &result {
            Ok(_) => String::new(),
            Err(e) => e.clone(),
        };
    }
    store::upsert_account(&account)?;
    result
}

// ---------------------------------------------------------------------------
// S/MIME 与 PGP
// ---------------------------------------------------------------------------

/// 当前账户的 S/MIME 身份摘要（不含私钥）。
#[command]
pub async fn mail_smime_status(
    app: AppHandle,
    account_id: String,
) -> Result<serde_json::Value, String> {
    ensure_init(&app)?;
    let Some(identity) = smime::load_identity(&account_id) else {
        return Ok(serde_json::json!({ "configured": false }));
    };
    let cert = smime::parse_certificate(&identity.cert_pem).ok();
    Ok(serde_json::json!({
        "configured": true,
        "subject": cert.as_ref().map(smime::subject_cn).unwrap_or_default(),
        "expired": cert.as_ref().map(smime::certificate_expired).unwrap_or(false),
        "hasKey": !identity.key_pem.trim().is_empty(),
    }))
}

#[command]
pub async fn mail_smime_import(
    app: AppHandle,
    account_id: String,
    cert_pem: String,
    key_pem: String,
) -> Result<(), String> {
    ensure_init(&app)?;
    smime::save_identity(
        &account_id,
        &smime::SmimeIdentity {
            cert_pem,
            key_pem,
        },
    )
}

/// 导入某个收件人的 S/MIME 证书（发加密邮件前必须有）。
#[command]
pub async fn mail_smime_save_peer(
    app: AppHandle,
    address: String,
    cert_pem: String,
) -> Result<(), String> {
    ensure_init(&app)?;
    if address.trim().is_empty() {
        return Err("收件人地址为空".to_string());
    }
    smime::save_peer_certificate(&address, &cert_pem)
}

#[command]
pub async fn mail_smime_delete_peer(app: AppHandle, address: String) -> Result<(), String> {
    ensure_init(&app)?;
    smime::delete_peer_certificate(&address);
    Ok(())
}

/// 查询某个地址是否已有 S/MIME 证书。
#[command]
pub async fn mail_smime_peer_status(app: AppHandle, address: String) -> Result<bool, String> {
    ensure_init(&app)?;
    Ok(smime::load_peer_certificate(&address).is_some())
}

#[command]
pub async fn mail_smime_delete(app: AppHandle, account_id: String) -> Result<(), String> {
    ensure_init(&app)?;
    smime::delete_identity(&account_id);
    Ok(())
}

/// PGP 钥匙串摘要。
#[command]
pub async fn mail_pgp_keys(app: AppHandle) -> Result<Vec<pgp::PgpKeyInfo>, String> {
    ensure_init(&app)?;
    Ok(pgp::public_key_summaries(&app))
}

#[command]
pub async fn mail_pgp_import_key(
    app: AppHandle,
    armored: String,
    secret: bool,
) -> Result<usize, String> {
    ensure_init(&app)?;
    let dir = pgp::key_dir(&app)?;
    if secret {
        // 私钥环落盘的那条路径要求是文件；写一份临时文件再走统一入口，
        // 保证「粘贴导入」与「文件导入」走过完全相同的校验。
        let path = std::env::temp_dir().join(format!("guaxi-pgp-{}.asc", Uuid::new_v4()));
        std::fs::write(&path, armored).map_err(|e| format!("写入临时文件失败: {e}"))?;
        let result = pgp::import_from_file(&dir, &path, true);
        let _ = std::fs::remove_file(&path);
        result
    } else {
        pgp::add_public_key(&dir, &armored)?;
        Ok(1)
    }
}

#[command]
pub async fn mail_pgp_import_file(
    app: AppHandle,
    path: String,
    secret: bool,
) -> Result<usize, String> {
    ensure_init(&app)?;
    let dir = pgp::key_dir(&app)?;
    pgp::import_from_file(&dir, std::path::Path::new(&path), secret)
}

// ---------------------------------------------------------------------------
// 通知 / 诊断
// ---------------------------------------------------------------------------

/// 发一条测试通知（设置页的「试试通知」按钮）。
#[command]
pub async fn mail_test_notification(app: AppHandle) -> Result<(), String> {
    ensure_init(&app)?;
    notify::notify_new_mail(
        &app,
        &[model::NewMailNotice {
            account_id: String::new(),
            account_email: "Guaxi 邮箱".into(),
            folder: "INBOX".into(),
            uid: 0,
            from_name: "Guaxi".into(),
            from_addr: "noreply@guaxi.local".into(),
            subject: "通知已开启，收到新邮件时会像这样提醒".into(),
        }],
    );
    Ok(())
}

/// 导出诊断报告到指定文件。
#[command]
pub async fn mail_export_diagnostics(
    app: AppHandle,
    target_path: String,
) -> Result<String, String> {
    ensure_init(&app)?;
    if target_path.trim().is_empty() {
        return Err("目标路径为空".to_string());
    }
    let report = notify::diagnostics(&app)?;
    std::fs::write(&target_path, report).map_err(|e| format!("写入诊断报告失败: {e}"))?;
    Ok(target_path)
}

// ---------------------------------------------------------------------------
// 连接管理
// ---------------------------------------------------------------------------

/// 清空连接池并重连（用户在设置里改了服务器参数之后调用）。
#[command]
pub async fn mail_reset_connections(
    app: AppHandle,
    account_id: Option<String>,
) -> Result<(), String> {
    ensure_init(&app)?;
    match account_id {
        Some(id) => pool::invalidate(&id),
        None => pool::invalidate_all(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_recipient_header_is_inserted_before_body() {
        let raw = b"From: a@b.c\r\nSubject: x\r\n\r\nbody".to_vec();
        let recipients = serde_json::json!({
            "to": ["财务组 <group:g1>"],
            "cc": [],
            "bcc": [],
        });
        let text = String::from_utf8(insert_draft_recipients_header(raw, &recipients)).unwrap();
        let header = text.find("X-Guaxi-Draft-Recipients:").expect("缺少自定义头");
        let body = text.find("\r\n\r\nbody").expect("正文分隔丢了");
        assert!(header < body);
        assert!(text.starts_with("From: a@b.c\r\n"));
    }

    #[test]
    fn recipient_limit_errors_are_detected() {
        assert!(recipient_limit_hit(
            "收件人 a@b.c 被拒绝（452）: 4.5.3 Too many recipients"
        ));
        assert!(recipient_limit_hit("RCPT failed: too many recipients"));
        assert!(!recipient_limit_hit(
            "收件人 a@b.c 被拒绝（550）: 5.1.1 unknown user"
        ));
    }
}
