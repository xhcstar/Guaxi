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
pub mod imap;
pub mod mime;
pub mod model;
pub mod net;
pub mod rules;
pub mod sanitize;
pub mod smtp;
pub mod store;
pub mod theme;

use model::{
    Account, FolderRole, MailAddress, MailFolder, MailRule, MailSettings, MessageDetail,
    MessageSummary, OutboxItem, ProxyConfig, SearchQuery, SendRequest,
};
use std::sync::OnceLock;
use tauri::{command, AppHandle, Emitter};
use tracing::{error, info, warn};
use uuid::Uuid;

const KEYRING_SERVICE: &str = "guaxi-mail";
/// 同步状态广播：前端监听它刷新未读数与列表。
pub const MAIL_SYNC_EVENT: &str = "mail-sync-progress";
/// 新邮件到达。
pub const MAIL_NEW_EVENT: &str = "mail-new-messages";

static INITED: OnceLock<()> = OnceLock::new();

fn ensure_init(app: &AppHandle) -> Result<(), String> {
    if INITED.get().is_some() {
        return Ok(());
    }
    store::init(app)?;
    contacts::store::init()?;
    let _ = INITED.set(());
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

fn account_password(account: &Account) -> Result<String, String> {
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
    if let Some(secret) = secret
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        write_secret(&account.id, secret)?;
    }
    store::upsert_account(&account)?;
    Ok(account)
}

#[command]
pub async fn mail_delete_account(app: AppHandle, account_id: String) -> Result<(), String> {
    ensure_init(&app)?;
    store::delete_account(&account_id)?;
    delete_secret(&account_id);
    Ok(())
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
    // 新建账户时表单里的口令还没落库，优先用传进来的那份。
    let password = secret
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| read_secret(&account.id))
        .unwrap_or_default();

    let imap_probe = tauri::async_runtime::spawn_blocking({
        let account = account.clone();
        let password = password.clone();
        move || imap::probe(&account, &password)
    })
    .await
    .map_err(|e| format!("诊断任务失败: {e}"))?;

    let smtp_probe = tauri::async_runtime::spawn_blocking(move || smtp_probe(&account, &password))
        .await
        .map_err(|e| format!("诊断任务失败: {e}"))?;

    Ok(vec![imap_probe, smtp_probe])
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
            match session.authenticate(&account.login_name(), password) {
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
    let password = account_password(&account)?;
    tauri::async_runtime::spawn_blocking(move || imap::create_folder(&account, &password, &name))
        .await
        .map_err(|e| format!("创建文件夹任务失败: {e}"))??;
    Ok(())
}

// ---------------------------------------------------------------------------
// 同步
// ---------------------------------------------------------------------------

/// 同步一个文件夹。返回本次新增的邮件（前端据此更新列表）。
#[command]
pub async fn mail_sync_folder(
    app: AppHandle,
    account_id: String,
    folder: String,
) -> Result<Vec<MessageSummary>, String> {
    ensure_init(&app)?;
    let account = load_account(&account_id)?;
    let password = account_password(&account)?;
    let settings = store::load_settings();
    let (uid_validity, _) = store::folder_uid_state(&account_id, &folder)?;
    let last_uid = store::last_uid(&account_id, &folder)?;
    let snippet_len = settings.snippet_len as usize;

    let items = tauri::async_runtime::spawn_blocking({
        let account = account.clone();
        let folder = folder.clone();
        let password = password.clone();
        move || imap::sync_folder(&account, &password, &folder, uid_validity, last_uid, snippet_len)
    })
    .await
    .map_err(|e| format!("同步任务失败: {e}"))??;

    store::touch_account_sync(&account_id, chrono::Utc::now().timestamp())?;
    collect_contacts(&account, &items);

    if settings.rules_enabled && !items.is_empty() {
        let rules = store::list_rules()?;
        if !rules.is_empty() {
            apply_rules_to_new(&account, &password, &folder, &items, &rules).await;
        }
    }

    let _ = app.emit(
        MAIL_SYNC_EVENT,
        serde_json::json!({
            "accountId": account_id,
            "folder": folder,
            "added": items.len(),
        }),
    );
    if !items.is_empty() {
        let _ = app.emit(MAIL_NEW_EVENT, &items);
    }
    Ok(items)
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
    for message in items {
        if message.outgoing {
            continue;
        }
        let to: Vec<contacts::model::LabeledValue> = serde_json::from_str(&message.to_json).unwrap_or_default();
        let cc: Vec<contacts::model::LabeledValue> = serde_json::from_str(&message.cc_json).unwrap_or_default();
        if let Err(e) = contacts::store::touch_from_message(
            &own,
            &message.from_name,
            &message.from_addr,
            &to,
            &cc,
            message.date,
        ) {
            warn!(error = %e, uid = message.uid, "mail: 自动收录联系人失败");
            return;
        }
    }
}

/// 对新到邮件跑规则。只有「标读 / 星标 / 打标签」是本地即时生效的；
/// 移动、删除、转发需要服务器往返，失败时只记录日志，不阻塞同步。
async fn apply_rules_to_new(
    account: &Account,
    password: &str,
    folder: &str,
    messages: &[MessageSummary],
    all_rules: &[MailRule],
) {
    let folders = store::list_folders(&account.id).unwrap_or_default();
    let folder_role = folders
        .iter()
        .find(|f| f.name == folder)
        .map(|f| f.role.as_str().to_string())
        .unwrap_or_else(|| "custom".to_string());
    let trash = folders
        .iter()
        .find(|f| f.role == FolderRole::Trash)
        .map(|f| f.name.clone());

    for msg in messages {
        if msg.outgoing {
            continue;
        }
        let body = store::get_body(&msg.account_id, &msg.folder, msg.uid)
            .ok()
            .flatten()
            .map(|(_, text, _, _)| text)
            .unwrap_or_default();
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
        if let Err(e) = store::set_local_flags(
            &msg.account_id,
            &msg.folder,
            &[msg.uid],
            if merged.mark_read { Some(true) } else { None },
            if merged.star { Some(true) } else { None },
            Some(&labels),
        ) {
            warn!(error = %e, "mail: 写入规则结果失败");
        }

        if merged.mark_read || merged.star {
            let (acc, pw, folder_name) = (account.clone(), password.to_string(), folder.to_string());
            let uids = vec![msg.uid];
            let seen = if merged.mark_read { Some(true) } else { None };
            let flagged = if merged.star { Some(true) } else { None };
            let _ = tauri::async_runtime::spawn_blocking(move || {
                imap::set_flags(&acc, &pw, &folder_name, &uids, seen, flagged)
            })
            .await;
        }

        // 移动 / 删除：本地先删，再在后台把服务器操作做完。
        let move_target = merged.move_to.clone().or_else(|| {
            if merged.delete {
                trash.clone()
            } else {
                None
            }
        });
        if let Some(target) = move_target {
            let (acc, pw, folder_name) = (account.clone(), password.to_string(), folder.to_string());
            let uids = vec![msg.uid];
            let moved = tauri::async_runtime::spawn_blocking(move || {
                imap::move_messages(&acc, &pw, &folder_name, &uids, &target)
            })
            .await;
            match moved {
                Ok(Ok(())) => {
                    let _ = store::delete_message(&msg.account_id, &msg.folder, msg.uid);
                }
                Ok(Err(e)) => error!(error = %e, uid = msg.uid, "mail: 规则移动邮件失败"),
                Err(e) => error!(error = %e, uid = msg.uid, "mail: 规则移动任务失败"),
            }
        }

        if !merged.forward_to.is_empty() {
            forward_by_rule(&account.clone(), password, msg, &merged.forward_to).await;
        }
    }
}

async fn forward_by_rule(
    account: &Account,
    password: &str,
    msg: &MessageSummary,
    recipients: &[String],
) {
    let (acc, pw) = (account.clone(), password.to_string());
    let (folder, uid) = (msg.folder.clone(), msg.uid);
    let recipients = recipients.to_vec();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let raw = imap::fetch_raw(&acc, &pw, &folder, uid)?;
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
        };
        let from = MailAddress {
            name: acc.display_name.clone(),
            address: acc.email.clone(),
        };
        let bytes = mime::build(&request, &from, "")?;
        smtp::send_raw(&acc, &pw, effective_proxy(&acc), &acc.email, &recipients, &bytes)
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
            Ok(f) if !f.is_empty() => f,
            _ => match refresh_folders_inner(&account).await {
                Ok(f) => f,
                Err(e) => {
                    errors.push(format!("{}: {e}", account.email));
                    continue;
                }
            },
        };
        for folder in folders
            .iter()
            .filter(|f| f.role == FolderRole::Inbox && f.selectable)
        {
            match sync_one(&account, &folder.name).await {
                Ok(n) => total += n,
                Err(e) => errors.push(format!("{} / {}: {e}", account.email, folder.display_name)),
            }
        }
    }
    Ok(serde_json::json!({ "added": total, "errors": errors }))
}

async fn refresh_folders_inner(account: &Account) -> Result<Vec<MailFolder>, String> {
    let password = account_password(account)?;
    let acc = account.clone();
    let folders = tauri::async_runtime::spawn_blocking(move || imap::list_folders(&acc, &password))
        .await
        .map_err(|e| format!("刷新文件夹任务失败: {e}"))??;
    store::replace_folders(&account.id, &folders)?;
    Ok(folders)
}

async fn sync_one(account: &Account, folder: &str) -> Result<usize, String> {
    let password = account_password(account)?;
    let settings = store::load_settings();
    let (uid_validity, _) = store::folder_uid_state(&account.id, folder)?;
    let last_uid = store::last_uid(&account.id, folder)?;
    let acc = account.clone();
    let folder_name = folder.to_string();
    let snippet_len = settings.snippet_len as usize;
    let password_for_sync = password.clone();
    let items = tauri::async_runtime::spawn_blocking(move || {
        imap::sync_folder(
            &acc,
            &password_for_sync,
            &folder_name,
            uid_validity,
            last_uid,
            snippet_len,
        )
    })
    .await
    .map_err(|e| format!("同步任务失败: {e}"))??;
    store::touch_account_sync(&account.id, chrono::Utc::now().timestamp())?;
    collect_contacts(account, &items);

    if settings.rules_enabled && !items.is_empty() {
        let rules = store::list_rules()?;
        if !rules.is_empty() {
            apply_rules_to_new(account, &password, folder, &items, &rules).await;
        }
    }
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
) -> Result<MessageDetail, String> {
    ensure_init(&app)?;
    let mut summary = store::get_summary(&account_id, &folder, uid)?
        .ok_or_else(|| "本地没有这封邮件的记录".to_string())?;

    let cached = store::get_body(&account_id, &folder, uid)?;
    let (cached_html, cached_text, cached_attachments, fetched) =
        cached.unwrap_or((None, String::new(), "[]".into(), false));

    let mut detail = store::base_detail(&summary);
    if fetched && (!cached_text.is_empty() || cached_html.is_some()) {
        detail.html = cached_html;
        detail.text = cached_text;
        detail.attachments = serde_json::from_str(&cached_attachments).unwrap_or_default();
    } else {
        let account = load_account(&account_id)?;
        let password = account_password(&account)?;
        let settings = store::load_settings();
        let (f, u) = (folder.clone(), uid);
        let acc = account.clone();
        let raw =
            tauri::async_runtime::spawn_blocking(move || imap::fetch_raw(&acc, &password, &f, u))
                .await
                .map_err(|e| format!("取信任务失败: {e}"))??;
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
        summary.has_attachments = summary.has_attachments || parsed.has_attachments;
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
    }

    // 打开即已读（与主流客户端一致）。失败不影响阅读。
    if !summary.seen && mark_seen_inner(&account_id, &folder, &[uid], true).await.is_ok() {
        detail.seen = true;
    }
    Ok(detail)
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
    let password = account_password(&account)?;
    let (f, u) = (folder.to_string(), uids.to_vec());
    let acc = account.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imap::set_flags(&acc, &password, &f, &u, Some(seen), None)
    })
    .await
    .map_err(|e| format!("标记任务失败: {e}"))??;
    store::set_local_flags(account_id, folder, uids, Some(seen), None, None)
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
    let password = account_password(&account)?;
    let (f, u) = (folder.clone(), uids.clone());
    let acc = account.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imap::set_flags(&acc, &password, &f, &u, None, Some(flagged))
    })
    .await
    .map_err(|e| format!("标记任务失败: {e}"))??;
    store::set_local_flags(&account_id, &folder, &uids, None, Some(flagged), None)
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
    let password = account_password(&account)?;
    let (f, u, t) = (folder.clone(), uids.clone(), target.clone());
    let acc = account.clone();
    tauri::async_runtime::spawn_blocking(move || imap::move_messages(&acc, &password, &f, &u, &t))
        .await
        .map_err(|e| format!("移动任务失败: {e}"))??;
    for uid in &uids {
        let _ = store::delete_message(&account_id, &folder, *uid);
    }
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
    let password = account_password(&account)?;
    let folders = store::list_folders(&account_id)?;
    let current_role = folders
        .iter()
        .find(|f| f.name == folder)
        .map(|f| f.role)
        .unwrap_or(FolderRole::Custom);
    let trash = folders
        .iter()
        .find(|f| f.role == FolderRole::Trash)
        .map(|f| f.name.clone());

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
    for uid in &uids {
        let _ = store::delete_message(&account_id, &folder, *uid);
    }
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
    let (f, safe_name) = (folder.clone(), sanitize_filename(&filename));
    let raw =
        tauri::async_runtime::spawn_blocking(move || imap::fetch_raw(&account, &password, &f, uid))
            .await
            .map_err(|e| format!("取信任务失败: {e}"))??;
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

// ---------------------------------------------------------------------------
// 发送
// ---------------------------------------------------------------------------

#[command]
pub async fn mail_send(app: AppHandle, request: SendRequest) -> Result<(), String> {
    ensure_init(&app)?;
    send_inner(&request).await
}

async fn send_inner(request: &SendRequest) -> Result<(), String> {
    let account = load_account(&request.account_id)?;
    let password = account_password(&account)?;
    let from = MailAddress {
        name: account.display_name.clone(),
        address: account.email.clone(),
    };
    let save_to_sent = request.save_to_sent.unwrap_or(true);
    let raw = mime::build(request, &from, &account.signature)?;

    let mut recipients: Vec<String> = Vec::new();
    for addr in request
        .to
        .iter()
        .chain(request.cc.iter())
        .chain(request.bcc.iter())
    {
        recipients.push(mime::parse_mailbox(addr)?.email.to_string());
    }

    let acc = account.clone();
    let envelope_from = account.email.clone();
    let raw_for_send = raw.clone();
    let pw_for_send = password.clone();
    tauri::async_runtime::spawn_blocking(move || {
        smtp::send_raw(
            &acc,
            &pw_for_send,
            effective_proxy(&acc),
            &envelope_from,
            &recipients,
            &raw_for_send,
        )
    })
    .await
    .map_err(|e| format!("发信任务失败: {e}"))??;

    if save_to_sent {
        if let Some(sent) = imap::sent_folder_name(&account) {
            let acc = account.clone();
            let pw = password.clone();
            let raw_clone = raw.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                imap::append_to_folder(&acc, &pw, &sent, &raw_clone, &["seen"])
            })
            .await;
        }
    }
    info!(account = %account.email, "mail: 发送成功");
    Ok(())
}

/// 延迟发送：先进 outbox，倒计时结束后由 `mail_flush_outbox` 投递。
#[command]
pub async fn mail_queue_send(app: AppHandle, request: SendRequest) -> Result<OutboxItem, String> {
    ensure_init(&app)?;
    let settings = store::load_settings();
    let delay = settings.undo_send_seconds as i64;
    let now = chrono::Utc::now().timestamp();
    let item = OutboxItem {
        id: Uuid::new_v4().to_string(),
        account_id: request.account_id.clone(),
        subject: request.subject.clone(),
        to: request.to.join(", "),
        due_at: now + delay.max(1),
        created_at: now,
    };
    let payload = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    store::enqueue_outbox(&item, &payload)?;
    Ok(item)
}

#[command]
pub async fn mail_list_outbox(app: AppHandle) -> Result<Vec<OutboxItem>, String> {
    ensure_init(&app)?;
    store::list_outbox()
}

/// 撤回一条待发邮件（只在倒计时窗口内有效）。
#[command]
pub async fn mail_cancel_outbox(app: AppHandle, id: String) -> Result<(), String> {
    ensure_init(&app)?;
    store::drop_outbox(&id)
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
            Ok(request) => match send_inner(&request).await {
                Ok(()) => {
                    let _ = store::mark_outbox_sent(&id);
                    sent += 1;
                }
                // 发送失败保留在队列里，等用户处理（避免静默丢信）。
                // 同时把认领时间归零，下一次轮询会立刻重试；否则要等 5 分钟认领窗口过期，
                // 用户会以为「点了立即发送没反应」。
                Err(e) => {
                    let _ = store::release_outbox(&id);
                    errors.push(e);
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
#[command]
pub async fn mail_save_draft(app: AppHandle, request: SendRequest) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&request.account_id)?;
    let password = account_password(&account)?;
    let from = MailAddress {
        name: account.display_name.clone(),
        address: account.email.clone(),
    };
    let raw = mime::build(&request, &from, &account.signature)?;
    let drafts = store::list_folders(&account.id)?
        .into_iter()
        .find(|f| f.role == FolderRole::Drafts)
        .map(|f| f.name)
        .ok_or_else(|| "账户没有草稿箱".to_string())?;
    let acc = account.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imap::append_to_folder(&acc, &password, &drafts, &raw, &["draft", "seen"])
    })
    .await
    .map_err(|e| format!("保存草稿任务失败: {e}"))??;
    Ok(())
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
pub async fn mail_save_rule(app: AppHandle, rule: MailRule) -> Result<MailRule, String> {
    ensure_init(&app)?;
    let mut rule = rule;
    if rule.id.trim().is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }
    if rule.created_at == 0 {
        rule.created_at = chrono::Utc::now().timestamp();
    }
    store::upsert_rule(&rule)?;
    Ok(rule)
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
    store::clear_cache(account_id.as_deref())
}
