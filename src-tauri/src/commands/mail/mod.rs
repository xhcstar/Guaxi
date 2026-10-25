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
    offline::init_table()?;
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
    // 本地加密密钥也要一起删：留着会在系统凭据库里堆孤儿条目，
    // 而且下次用同一个账户 id 恢复数据时会因密钥不匹配而解不开。
    crypto::delete_key(&account_id);
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
    let password = account_password(&account)?;
    tauri::async_runtime::spawn_blocking(move || imap::create_folder(&account, &password, &name))
        .await
        .map_err(|e| format!("创建文件夹任务失败: {e}"))??;
    Ok(())
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
    let items = sync_folder_guarded(&account, &folder).await?;
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

/// 同步单个文件夹的完整流程（加锁、增量、标记比对、规则、通知）。
async fn sync_folder_guarded(
    account: &Account,
    folder: &str,
) -> Result<Vec<MessageSummary>, String> {
    let settings = store::load_settings();
    let account_id = account.id.clone();
    let folder_name = folder.to_string();
    let acc = account.clone();

    let items = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<MessageSummary>, String> {
        let _guard = sync::lock_account(&account_id);
        let (uid_validity, _) = store::folder_uid_state(&account_id, &folder_name)?;
        let last_uid = store::last_uid(&account_id, &folder_name)?;
        let items = imap::sync_folder_pooled(
            &acc,
            &folder_name,
            uid_validity,
            last_uid,
            settings.snippet_len as usize,
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
    })
    .await
    .map_err(|e| format!("同步任务失败: {e}"))??;

    store::touch_account_sync(&account.id, chrono::Utc::now().timestamp())?;
    collect_contacts(account, &items);

    if settings.rules_enabled && !items.is_empty() {
        let rules = store::list_rules()?;
        if !rules.is_empty() {
            apply_rules_to_new(account, &items, &rules).await;
        }
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
    messages: &[MessageSummary],
    all_rules: &[MailRule],
) {
    let folders = store::list_folders(&account.id).unwrap_or_default();
    let trash = folders
        .iter()
        .find(|f| f.role == FolderRole::Trash)
        .map(|f| f.name.clone());
    // 收/发件人名单比规则优先：命中的发件人直接按名单处理，不再跑规则。
    let sender_rules = store::list_sender_rules().unwrap_or_default();

    for msg in messages {
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
                        apply_rule_move(account, msg, trash).await;
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
            let (acc, folder_name) = (account.clone(), msg.folder.clone());
            let uids = vec![msg.uid];
            let seen = if merged.mark_read { Some(true) } else { None };
            let flagged = if merged.star { Some(true) } else { None };
            let _ = tauri::async_runtime::spawn_blocking(move || {
                imap::set_flags(&acc, "", &folder_name, &uids, seen, flagged)
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
            apply_rule_move(account, msg, &target).await;
        }

        if !merged.forward_to.is_empty() {
            forward_by_rule(&account.clone(), msg, &merged.forward_to).await;
        }
    }
}

/// 规则 / 名单触发的移动：搬完再删本地行，失败只记日志（不阻塞同步）。
async fn apply_rule_move(account: &Account, msg: &MessageSummary, target: &str) {
    let (acc, folder_name, target_owned) =
        (account.clone(), msg.folder.clone(), target.to_string());
    let uids = vec![msg.uid];
    let moved = tauri::async_runtime::spawn_blocking(move || {
        imap::move_messages(&acc, "", &folder_name, &uids, &target_owned)
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

async fn forward_by_rule(account: &Account, msg: &MessageSummary, recipients: &[String]) {
    let (acc, folder, uid) = (account.clone(), msg.folder.clone(), msg.uid);
    let recipients = recipients.to_vec();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let raw = imap::fetch_raw(&acc, "", &folder, uid)?;
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
        let secret = imap::auth_secret(&acc)?;
        smtp::send_raw(&acc, &secret, effective_proxy(&acc), &acc.email, &recipients, &bytes)
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
        // 后台轮询只同步收件箱：已发送 / 草稿等文件夹的变化频率低，
        // 每 90 秒把所有文件夹都拉一遍会让服务器负担与移动端耗电都变得不合理。
        // 用户正在浏览的文件夹由前端单独调 `mail_sync_folder` 补。
        for folder in folders
            .iter()
            .filter(|f| f.role == FolderRole::Inbox && f.selectable)
        {
            match sync_one(&account, &folder.name).await {
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
    let password = account_password(account)?;
    let acc = account.clone();
    let folders = tauri::async_runtime::spawn_blocking(move || imap::list_folders(&acc, &password))
        .await
        .map_err(|e| format!("刷新文件夹任务失败: {e}"))??;
    store::replace_folders(&account.id, &folders)?;
    Ok(folders)
}

async fn sync_one(account: &Account, folder: &str) -> Result<usize, String> {
    let items = sync_folder_guarded(account, folder).await?;
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
    let mut summary = store::get_summary(&account_id, &folder, uid)?
        .ok_or_else(|| "本地没有这封邮件的记录".to_string())?;

    let cached = if force.unwrap_or(false) {
        None
    } else {
        store::get_body(&account_id, &folder, uid)?
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
    if fetched && !cached_needs_inline && (!cached_text.is_empty() || cached_html.is_some()) {
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
        let password = account_password(&account)?;
        let settings = store::load_settings();
        let (f, u) = (folder.clone(), uid);
        let acc = account.clone();
        let raw =
            tauri::async_runtime::spawn_blocking(move || imap::fetch_raw(&acc, &password, &f, u))
                .await
                .map_err(|e| format!("取信任务失败: {e}"))??;
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
    let acc = account.clone();
    let server = tauri::async_runtime::spawn_blocking(move || {
        imap::move_messages(&acc, "", &f, &u, &t)
    })
    .await
    .map_err(|e| format!("移动任务失败: {e}"))?;
    for uid in &uids {
        let _ = store::delete_message(&account_id, &folder, *uid);
    }
    match server {
        Ok(()) => Ok(()),
        Err(e) => write_with_fallback(&account, op, || Err(e)),
    }
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
    let dir = store::data_root(&app)?
        .join("attachments")
        .join(sanitize_filename(&account_id))
        .join(uid.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建缓存目录失败: {e}"))?;
    let path = dir.join(safe_name);
    std::fs::write(&path, &bytes).map_err(|e| format!("写入附件失败: {e}"))?;
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|e| format!("调用系统默认程序失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
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
    let (f, u) = (folder.clone(), uid);
    let raw =
        tauri::async_runtime::spawn_blocking(move || imap::fetch_raw(&account, &password, &f, u))
            .await
            .map_err(|e| format!("取信任务失败: {e}"))??;
    std::fs::write(&target_path, &raw).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(target_path)
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
    let inbox = store::list_folders(&account_id)?
        .into_iter()
        .find(|f| f.role == FolderRole::Inbox)
        .map(|f| f.name)
        .ok_or_else(|| "账户没有收件箱".to_string())?;
    move_batch(&app, &account_id, &folder, &uids, &inbox).await
}

fn junk_folder_name(account_id: &str) -> Result<String, String> {
    let folders = store::list_folders(account_id)?;
    folders
        .iter()
        .find(|f| f.role == FolderRole::Junk)
        .map(|f| f.name.clone())
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
    let password = account_password(&account)?;
    let (acc, f, t) = (account.clone(), folder.to_string(), target.to_string());
    let batch = uids.to_vec();
    tauri::async_runtime::spawn_blocking(move || imap::move_messages(&acc, &password, &f, &batch, &t))
        .await
        .map_err(|e| format!("移动任务失败: {e}"))??;
    for uid in uids {
        let _ = store::delete_message(account_id, folder, *uid);
    }
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
    let folders = store::list_folders(&account_id)?;
    let current = folders
        .iter()
        .find(|f| f.name == folder)
        .map(|f| f.role)
        .unwrap_or(FolderRole::Custom);
    let trash = folders
        .iter()
        .find(|f| f.role == FolderRole::Trash)
        .map(|f| f.name.clone());

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
    for uid in &uids {
        let _ = store::delete_message(&account_id, &folder, *uid);
    }
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
        to: display_recipients(&request.to),
        due_at: now + delay.max(1),
        created_at: now,
        attempts: 0,
        next_attempt_at: 0,
        last_error: String::new(),
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
            Ok(request) => match send_inner(&app, &request).await {
                Ok(()) => {
                    let _ = store::mark_outbox_sent(&id);
                    sent += 1;
                }
                // 发送失败保留在队列里，等用户处理（避免静默丢信）。
                // 同时把认领时间归零，下一次轮询会立刻重试；否则要等 5 分钟认领窗口过期，
                // 用户会以为「点了立即发送没反应」。
                Err(e) => {
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
#[command]
pub async fn mail_save_draft(app: AppHandle, request: SendRequest) -> Result<(), String> {
    ensure_init(&app)?;
    let account = load_account(&request.account_id)?;
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
    let raw = mime::build(&request, &from, &account.signature)?;
    let raw = insert_draft_recipients_header(raw, &original_recipients);
    let drafts = store::list_folders(&account.id)?
        .into_iter()
        .find(|f| f.role == FolderRole::Drafts)
        .map(|f| f.name)
        .ok_or_else(|| "账户没有草稿箱".to_string())?;
    let acc = account.clone();
    let drafts_for_append = drafts.clone();
    let password_for_append = password.clone();
    tauri::async_runtime::spawn_blocking(move || {
        imap::append_to_folder(&acc, &password_for_append, &drafts_for_append, &raw, &["draft", "seen"])
    })
    .await
    .map_err(|e| format!("保存草稿任务失败: {e}"))??;

    // 立刻同步一次草稿箱，让它在侧栏里马上可见。
    // 只按 UID 增量拉，不重拉整个文件夹。
    let (uid_validity, _) = store::folder_uid_state(&account.id, &drafts)?;
    let last_uid = store::last_uid(&account.id, &drafts)?;
    let settings = store::load_settings();
    let acc = account.clone();
    let drafts_for_sync = drafts.clone();
    let snippet_len = settings.snippet_len as usize;
    let _ = tauri::async_runtime::spawn_blocking(move || {
        imap::sync_folder(
            &acc,
            &password,
            &drafts_for_sync,
            uid_validity,
            last_uid,
            snippet_len,
        )
    })
    .await;
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
            let (f, u) = (folder.to_string(), uid);
            let raw = tauri::async_runtime::spawn_blocking(move || {
                imap::fetch_raw(&account, &password, &f, u)
            })
            .await
            .map_err(|e| format!("取信任务失败: {e}"))??;
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
