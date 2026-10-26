//! POP3 账户的同步与文件夹模型。
//!
//! 现有客户端是按 IMAP 的模型写起来的：多个文件夹、服务器端标记、按 uid 增量。
//! POP3 只有「一个收件箱 + 一叠邮件」，所以这里把它接进同一套模型时做三件事：
//! 1. **文件夹**：只建一条 `INBOX`。已发送 / 草稿 / 垃圾箱都做成**本地**文件夹，
//!    写入进 store，服务器上没有对应物（POP3 不提供任何服务器端文件夹）。
//! 2. **标识**：用服务器给的 `UIDL` 换一个稳定的本地 uid（见 `store::pop3_uid_for`），
//!    这样重复同步不会产生重复邮件，已读 / 星标也不会丢。
//! 3. **同步**：逐个下载未见过的 UIDL。`TOP n 0` 可用的服务器先取头部做列表摘要，
//!    正文留到打开时再取——POP3 没有「只取正文」的接口，打开时的第二次 RETR
//!    在本机缓存里也只发生一次。
//!
//! 删除语义：`pop3_leave_on_server = true`（默认）时本地删除不动服务器；
//! 关闭它时，本地删除会在下一次同步时 `DELE` 对应序号。

use super::model::{Account, FolderRole, MailFolder, MessageSummary};
use super::{imap, mime, pop3, sanitize, store};
use tracing::{debug, info, warn};

/// POP3 账户唯一可用的服务器文件夹名（与 IMAP 的 INBOX 同名，前端逻辑可以共用）。
pub const INBOX: &str = "INBOX";

/// POP3 没有服务器文件夹：客户端内置的那几个语义文件夹都是纯本地的。
/// 返回 (服务器名, 显示名, 角色)。
fn local_folders() -> Vec<(String, String, FolderRole)> {
    vec![
        (INBOX.to_string(), "收件箱".to_string(), FolderRole::Inbox),
        ("Drafts".to_string(), "草稿".to_string(), FolderRole::Drafts),
        // 发件箱：多次投递失败后暂存在这里等用户处置；纯本地。
        ("Outbox".to_string(), "发件箱".to_string(), FolderRole::Outbox),
        // 发信由 SMTP 负责，「已发送」只是把副本落在本地库里。
        ("Sent".to_string(), "已发送".to_string(), FolderRole::Sent),
        ("Archive".to_string(), "存档".to_string(), FolderRole::Archive),
        ("Trash".to_string(), "已删除".to_string(), FolderRole::Trash),
        ("Junk".to_string(), "垃圾邮件".to_string(), FolderRole::Junk),
    ]
}

/// 这个账户是不是 POP3。
pub fn is_pop3(account: &Account) -> bool {
    account.protocol.eq_ignore_ascii_case("pop3")
}

/// 建立 / 修正 POP3 账户的本地文件夹表。
///
/// 不做「发现」：POP3 的文件夹是固定的，每次同步直接以 `local_folders` 为准
/// 刷新一下（保留已有未读数，避免侧栏数字闪一下又回来）。
pub fn ensure_folders(account: &Account) -> Result<Vec<MailFolder>, String> {
    let existing = store::list_folders(&account.id)?;
    let mut folders = Vec::new();
    for (name, display, role) in local_folders() {
        let previous = existing.iter().find(|f| f.name == name);
        folders.push(MailFolder {
            account_id: account.id.clone(),
            name,
            display_name: display,
            delimiter: None,
            role,
            unread: previous.map(|f| f.unread).unwrap_or(0),
            total: previous.map(|f| f.total).unwrap_or(0),
            uid_validity: 0,
            uid_next: previous.map(|f| f.uid_next).unwrap_or(0),
            selectable: true,
            // POP3 的文件夹全部是本地语义：没有一个是服务器上真实存在的。
            local_only: true,
            user_created: true,
        });
    }
    store::replace_folders(&account.id, &folders)?;
    Ok(folders)
}

/// 同步收件箱：拉回新邮件，返回新增的列表行。
///
/// `snippet_len` 与 IMAP 同步一致，由调用方从设置里取。
pub fn sync_inbox(account: &Account, snippet_len: usize) -> Result<Vec<MessageSummary>, String> {
    let secret = imap::auth_secret(account)?;
    let proxy = super::effective_proxy(account);
    let mut session = pop3::Pop3Session::connect(account, &secret, proxy)?;

    // 先把文件夹表补齐：前端第一次打开时侧栏就有东西可点。
    ensure_folders(account).ok();

    let result = (|| -> Result<Vec<MessageSummary>, String> {
        let entries = match session.uidl() {
            Ok(list) if !list.is_empty() => list,
            _ => {
                // 没有 UIDL 的服务器只能按序号走：用 `<number>:<size>` 当伪 UIDL。
                // 序号会在删除后重排，所以这种账户在服务器删过信之后可能重复收信。
                // 这是少数老服务器上的已知代价；有 UIDL 时按 UIDL 去重是精确的。
                warn!(
                    account = %account.email,
                    "mail: POP3 服务器不支持 UIDL，退回按序号比对"
                );
                let list = session.list()?;
                list.into_iter()
                    .map(|entry| pop3::Pop3Entry {
                        uidl: format!("seq:{}:{}", entry.number, entry.size),
                        ..entry
                    })
                    .collect()
            }
        };

        // 列表里的大小信息：UIDL 命令不带大小，单独取一次 LIST 补上（一次往返）。
        let sizes: std::collections::HashMap<u32, u32> = session
            .list()
            .unwrap_or_default()
            .into_iter()
            .map(|entry| (entry.number, entry.size))
            .collect();

        let known = store::pop3_known_uids(&account.id)?;
        let settings = store::load_settings();
        let mut refreshed: Vec<MessageSummary> = Vec::new();
        let mut downloaded = 0usize;

        for entry in &entries {
            if known.contains(&entry.uidl) {
                continue;
            }
            // 单封超过上限的邮件先跳过：POP3 是整封下载，一封 80 MB 的附件会让
            // 整个同步卡住好几分钟。用户点开时再强制下载（`fetch_message`）。
            let size = sizes.get(&entry.number).copied().unwrap_or(0);
            if size as u64 > MAX_AUTO_FETCH_BYTES {
                debug!(
                    uid = entry.number,
                    size,
                    "mail: 邮件过大，跳过自动下载，打开时再取"
                );
                continue;
            }
            match fetch_one(account, &mut session, entry, snippet_len, &settings) {
                Ok(summary) => {
                    downloaded += 1;
                    refreshed.push(summary);
                }
                Err(error) => {
                    // 单封失败（畸形报文、服务器超时）不该让整个收件失败。
                    warn!(
                        account = %account.email,
                        number = entry.number,
                        error = %error,
                        "mail: 下载单封 POP3 邮件失败，跳过"
                    );
                }
            }
        }

        // 删信语义：只有明确关闭「保留在服务器」时才 DELE。
        // 无论哪种模式，都要顺手丢掉「服务器上已不存在」的 UIDL 映射，
        // 否则本地删除过的邮件会一直被当成「待删除」，下一轮还会去撞一个不存在的序号。
        apply_server_deletions(account, &mut session, &entries, account.pop3_leave_on_server)?;

        // 未读数 / 总数按本地库重算后写回文件夹行。
        refresh_inbox_counts(account)?;

        info!(
            account = %account.email,
            server = entries.len(),
            downloaded,
            "mail: POP3 同步完成"
        );
        Ok(refreshed)
    })();

    match &result {
        Ok(_) => session.quit(),
        Err(_) => session.abort(),
    }
    result
}

/// 处理「本地已经删掉」的邮件。
///
/// 「本地删了」的判定是：UIDL 映射里还有这一条，但本地消息表里已经没有对应行。
/// `keep_on_server` 为 true 时只清理映射（服务器上那份按用户意愿保留）；
/// 为 false 时先在服务器上 `DELE` 再清理映射。
fn apply_server_deletions(
    account: &Account,
    session: &mut pop3::Pop3Session,
    entries: &[pop3::Pop3Entry],
    keep_on_server: bool,
) -> Result<(), String> {
    let by_uidl: std::collections::HashMap<&str, u32> = entries
        .iter()
        .map(|entry| (entry.uidl.as_str(), entry.number))
        .collect();
    let mut pending = Vec::new();
    for uidl in store::pop3_known_uids(&account.id)? {
        let Some(number) = by_uidl.get(uidl.as_str()).copied() else {
            // 服务器上已经没有了（例如在别的客户端删掉了）：本地映射也该清掉。
            let _ = store::pop3_forget_uidl(&account.id, &uidl);
            continue;
        };
        let uid = store::pop3_uid_for(&account.id, &uidl).ok();
        let still_local = uid
            .and_then(|uid| store::get_summary(&account.id, INBOX, uid).ok().flatten())
            .is_some();
        if !still_local {
            pending.push((uidl, number));
        }
    }
    for (uidl, number) in pending {
        if keep_on_server {
            // 用户选择「在服务器上保留副本」：本地删除只影响本机。
            let _ = store::pop3_forget_uidl(&account.id, &uidl);
            continue;
        }
        if let Err(error) = session.delete(number) {
            warn!(
                account = %account.email,
                number,
                error = %error,
                "mail: 在服务器上删除邮件失败"
            );
            continue;
        }
        let _ = store::pop3_forget_uidl(&account.id, &uidl);
    }
    Ok(())
}

/// 超过这个大小的邮件不在自动同步里下载（打开邮件时照常会取）。
const MAX_AUTO_FETCH_BYTES: u64 = 32 * 1024 * 1024;

/// 下载一封并入库。
fn fetch_one(
    account: &Account,
    session: &mut pop3::Pop3Session,
    entry: &pop3::Pop3Entry,
    snippet_len: usize,
    settings: &super::model::MailSettings,
) -> Result<MessageSummary, String> {
    let uid = store::pop3_uid_for(&account.id, &entry.uidl)?;
    let raw = session.retr(entry.number)?;
    let parsed = mime::parse(&raw, !settings.inline_images).ok_or("无法解析邮件内容")?;

    let summary = MessageSummary {
        account_id: account.id.clone(),
        folder: INBOX.to_string(),
        uid,
        message_id: parsed.message_id.clone(),
        subject: parsed.subject.clone(),
        from_name: parsed.from_name.clone(),
        from_addr: parsed.from_addr.clone(),
        to_json: serde_json::to_string(&parsed.to).unwrap_or_else(|_| "[]".into()),
        cc_json: serde_json::to_string(&parsed.cc).unwrap_or_else(|_| "[]".into()),
        date: parsed.date,
        size: raw.len() as u32,
        snippet: sanitize::truncate_chars(&parsed.snippet_source, snippet_len),
        seen: false,
        flagged: false,
        answered: false,
        draft: false,
        deleted: false,
        has_attachments: parsed.has_attachments,
        thread_key: mime::thread_key(&parsed.subject),
        labels: String::new(),
        outgoing: false,
    };
    let attachments_json = serde_json::to_string(&parsed.attachments).unwrap_or_else(|_| "[]".into());
    store::upsert_summaries(std::slice::from_ref(&summary))?;
    store::upsert_body(
        &account.id,
        INBOX,
        uid,
        parsed.html.as_deref(),
        &parsed.text,
        &attachments_json,
    )?;
    Ok(summary)
}

/// 按本地库里收件箱的实际行数刷新文件夹的未读 / 总数。
pub fn sync_counts(account: &Account) -> Result<(), String> {
    refresh_inbox_counts(account)
}

fn refresh_inbox_counts(account: &Account) -> Result<(), String> {
    let rows = store::list_recent(&account.id, INBOX, u32::MAX, 0)?;
    let unread = rows.iter().filter(|row| !row.seen).count() as u32;
    store::update_folder_counts(
        &account.id,
        INBOX,
        unread,
        rows.len() as u32,
        0,
        0,
    )
}

/// 保存草稿到本地 `Drafts`。POP3 没有服务器端草稿箱（也没有 APPEND）。
pub fn save_draft_copy(account: &Account, raw: &[u8]) -> Result<(), String> {
    save_copy_to(account, "Drafts", raw, false)
}

/// 把一份原始报文写成本地文件夹里的一封邮件。发件箱暂存、已发送副本、草稿共用。
pub fn save_copy_to(
    account: &Account,
    folder: &str,
    raw: &[u8],
    outgoing: bool,
) -> Result<(), String> {
    let parsed = mime::parse(raw, true).ok_or("无法解析刚保存的报文")?;
    // 文件夹可能还没入库（新账户第一次往发件箱里放东西）：先补一条再写邮件。
    if !store::list_folders(&account.id)?.iter().any(|f| f.name == folder) {
        store::insert_folder_row(&MailFolder {
            account_id: account.id.clone(),
            name: folder.to_string(),
            display_name: folder.to_string(),
            delimiter: None,
            role: FolderRole::parse(folder),
            unread: 0,
            total: 0,
            uid_validity: 0,
            uid_next: 0,
            selectable: true,
            local_only: true,
            user_created: false,
        })?;
    }
    let uid = store::last_uid(&account.id, folder)?.saturating_add(1).max(1);
    let settings = store::load_settings();
    let summary = MessageSummary {
        account_id: account.id.clone(),
        folder: folder.to_string(),
        uid,
        message_id: parsed.message_id.clone(),
        subject: parsed.subject.clone(),
        from_name: parsed.from_name.clone(),
        from_addr: parsed.from_addr.clone(),
        to_json: serde_json::to_string(&parsed.to).unwrap_or_else(|_| "[]".into()),
        cc_json: serde_json::to_string(&parsed.cc).unwrap_or_else(|_| "[]".into()),
        date: if parsed.date == 0 {
            chrono::Utc::now().timestamp()
        } else {
            parsed.date
        },
        size: raw.len() as u32,
        snippet: sanitize::truncate_chars(&parsed.snippet_source, settings.snippet_len as usize),
        seen: true,
        flagged: false,
        answered: false,
        draft: !outgoing,
        deleted: false,
        has_attachments: parsed.has_attachments,
        thread_key: mime::thread_key(&parsed.subject),
        labels: String::new(),
        outgoing,
    };
    let attachments_json = serde_json::to_string(&parsed.attachments).unwrap_or_else(|_| "[]".into());
    store::upsert_summaries(std::slice::from_ref(&summary))?;
    store::upsert_body(
        &account.id,
        folder,
        uid,
        parsed.html.as_deref(),
        &parsed.text,
        &attachments_json,
    )?;
    // 本地生成的邮件（已发送 / 草稿 / 发件箱）没有服务器可以回查，原文就在这里，
    // 顺手缓存：打开附件、另存 .eml 都不必再走网络（本地邮件本来也没有网络路径）。
    store::cache_raw_body(&account.id, folder, uid, raw).ok();
    Ok(())
}

/// 发信成功后保存「已发送」副本。POP3 没有服务器端的已发送文件夹，
/// 这一份只能落在本地库的 `Sent` 下。
pub fn save_sent_copy(account: &Account, raw: &[u8]) -> Result<(), String> {
    save_copy_to(account, "Sent", raw, true)
}

/// 连接诊断：建连、认证、读一次邮箱状态。与 `imap::probe` 同构，
/// 前端「测试连接」不需要区分协议。
pub fn probe(account: &Account, password: &str) -> super::model::ConnectionProbe {
    let mut probe = super::model::ConnectionProbe {
        ok: false,
        stage: "connect".into(),
        detail: String::new(),
        capabilities: Vec::new(),
    };
    match pop3::Pop3Session::connect(account, password, super::effective_proxy(account)) {
        Ok(mut session) => {
            probe.stage = "auth".into();
            probe.capabilities = vec!["POP3".to_string()];
            match session.stat() {
                Ok((count, size)) => {
                    probe.ok = true;
                    probe.stage = "done".into();
                    probe.detail = format!(
                        "POP3 连接与认证正常（服务器上有 {count} 封，共 {:.1} MB）",
                        size as f64 / 1048576.0
                    );
                }
                Err(error) => probe.detail = error,
            }
            session.quit();
        }
        Err(error) => probe.detail = error,
    }
    probe
}

/// 打开一封 POP3 邮件：本地有正文就直接返回；没有则重新下载整封并入库。
///
/// 与 IMAP 的区别：POP3 取单封必须整封下载，没有「按 uid 取正文」的接口，
/// 服务器给的序号也不是跨会话稳定的，所以这里先把 UIDL 列表拉一遍、
/// 找到与本地 uid 对应的那一封再 RETR。
pub fn fetch_message(
    account: &Account,
    uid: u32,
) -> Result<mime::ParsedMessage, String> {
    let secret = imap::auth_secret(account)?;
    let mut session = pop3::Pop3Session::connect(account, &secret, super::effective_proxy(account))?;
    let result = (|| -> Result<mime::ParsedMessage, String> {
        let entries = session.uidl()?;
        // 反查：这个 uid 对应哪个 UIDL。旧缓存里如果只有按序号得到的伪 UIDL，
        // 服务器换了一轮序号就会找不到——这时报清楚，而不是下载错的那一封。
        let uidl = store::pop3_uidl_for(&account.id, uid)?
            .ok_or_else(|| "服务器上找不到这封邮件（可能已被其他客户端删除）".to_string())?;
        let entry = entries
            .iter()
            .find(|entry| entry.uidl == uidl)
            .ok_or_else(|| "服务器上找不到这封邮件（可能已被其他客户端删除）".to_string())?;
        let raw = session.retr(entry.number)?;
        let mut parsed = mime::parse(&raw, !store::load_settings().inline_images)
            .ok_or_else(|| "无法解析邮件内容".to_string())?;
        // 刚下载的原文直接带出去：附件下载 / 另存为 .eml / 导出都会用到，
        // 否则每一处都得再连一次服务器把整封重下一遍。
        parsed.raw = Some(raw);
        Ok(parsed)
    })();
    match result {
        Ok(parsed) => {
            session.quit();
            Ok(parsed)
        }
        Err(error) => {
            session.abort();
            Err(error)
        }
    }
}
