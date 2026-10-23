//! IMAP 同步：文件夹发现、增量拉取、正文按需获取、标记、移动与追加。
//!
//! 连接一律建在 `net::Stream` 上（`imap::Client::new`），代理与 TLS 全由本模块控制。
//!
//! 两个绕不开的坑：
//! 1. `imap` crate 的 `connect_starttls` / `secure()` 只对 `TcpStream` 实现，一旦流是
//!    「经代理拿到的」就没法再升级。这里自己发 STARTTLS 命令、再把加密封装的流交给
//!    `Client::new`；因为 `Client::new` 要求先读问候语，而 STARTTLS 之后服务器不会再发问候，
//!    所以用 `ReplayStream` 把一行伪问候语垫在流前面。
//! 2. 服务器上的非 ASCII 文件夹名是 IMAP modified UTF-7（`&XfJT0ZAB-`），
//!    crate 原样发送会乱码，必须自己编解码（`utf7` 子模块）。

use super::model::{Account, FolderRole, MailFolder, MessageSummary};
use super::net::{open_stream, Stream};
use super::{mime, sanitize, store};
use imap::types::{Flag, NameAttribute};
use imap::{Client, Session};
use std::io::{Read, Write};
use tracing::{debug, info, warn};

/// 单次同步最多拉取的新邮件数。首次同步几千封的收件箱会拖到几十秒，
/// 这里分批推进，每批都能在前端看到进展。
pub const MAX_FETCH_PER_SYNC: u32 = 200;

// ---------------------------------------------------------------------------
// 流垫片：把一行文本垫在真实流前面
// ---------------------------------------------------------------------------

/// 在第一次读取时先吐出 `prefix`，之后透明转发给底层流。
struct ReplayStream {
    prefix: Vec<u8>,
    inner: Stream,
}

impl ReplayStream {
    fn new(inner: Stream, prefix: &[u8]) -> Self {
        let mut prefix = prefix.to_vec();
        if !prefix.is_empty() {
            prefix.extend_from_slice(b"\r\n");
        }
        Self { prefix, inner }
    }
}

impl Read for ReplayStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.prefix.is_empty() {
            let n = self.prefix.len().min(buf.len());
            buf[..n].copy_from_slice(&self.prefix[..n]);
            self.prefix.drain(..n);
            return Ok(n);
        }
        self.inner.read(buf)
    }
}

impl Write for ReplayStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

// ---------------------------------------------------------------------------
// modified UTF-7（RFC 3501 §5.1.3）
// ---------------------------------------------------------------------------

pub mod utf7 {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,";

    /// 编码：ASCII 原样，`&` 转义为 `&-`，非 ASCII 段用 modified base64。
    pub fn encode(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut pending: Vec<u16> = Vec::new();
        for ch in input.chars() {
            if ('\u{20}'..='\u{7e}').contains(&ch) {
                if !pending.is_empty() {
                    flush(&mut pending, &mut out);
                }
                if ch == '&' {
                    out.push_str("&-");
                } else {
                    out.push(ch);
                }
            } else {
                let mut buf = [0u16; 2];
                pending.extend_from_slice(ch.encode_utf16(&mut buf).as_ref());
            }
        }
        if !pending.is_empty() {
            flush(&mut pending, &mut out);
        }
        out
    }

    fn flush(pending: &mut Vec<u16>, out: &mut String) {
        let bytes: Vec<u8> = pending.iter().flat_map(|u| u.to_be_bytes()).collect();
        out.push('&');
        out.push_str(&b64(&bytes));
        out.push('-');
        pending.clear();
    }

    fn b64(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = *chunk.get(1).unwrap_or(&0) as u32;
            let b2 = *chunk.get(2).unwrap_or(&0) as u32;
            let n = (b0 << 16) | (b1 << 8) | b2;
            out.push(ALPHABET[(n >> 18) as usize & 63] as char);
            out.push(ALPHABET[(n >> 12) as usize & 63] as char);
            if chunk.len() > 1 {
                out.push(ALPHABET[(n >> 6) as usize & 63] as char);
            }
            if chunk.len() > 2 {
                out.push(ALPHABET[n as usize & 63] as char);
            }
        }
        out
    }

    /// 解码：`&-` 变回 `&`，`&...-` 段按 modified base64 还原。非法段整体丢弃。
    pub fn decode(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'&' {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'-' {
                    j += 1;
                }
                if j == i + 1 {
                    out.push('&');
                } else {
                    let payload = &input[i + 1..j.min(bytes.len())];
                    if let Some(decoded) = decode_segment(payload) {
                        out.push_str(&decoded);
                    }
                }
                i = j.saturating_add(1);
            } else {
                // ASCII 直接拷贝（按字符推进，避免切断多字节）。
                let ch = input[i..].chars().next().unwrap_or('\u{fffd}');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
        out
    }

    fn decode_segment(payload: &str) -> Option<String> {
        fn value(c: u8) -> Option<u8> {
            match c {
                b'A'..=b'Z' => Some(c - b'A'),
                b'a'..=b'z' => Some(c - b'a' + 26),
                b'0'..=b'9' => Some(c - b'0' + 52),
                b'+' => Some(62),
                b',' => Some(63),
                _ => None,
            }
        }
        let mut bits: u32 = 0;
        let mut nbits = 0u32;
        let mut raw: Vec<u8> = Vec::new();
        for &c in payload.as_bytes() {
            let v = value(c)?;
            bits = (bits << 6) | v as u32;
            nbits += 6;
            if nbits >= 8 {
                nbits -= 8;
                raw.push(((bits >> nbits) & 0xFF) as u8);
            }
        }
        if raw.len() % 2 != 0 {
            return None;
        }
        let units: Vec<u16> = raw
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&units).ok()
    }
}

// ---------------------------------------------------------------------------
// 连接
// ---------------------------------------------------------------------------

/// 读一行（到 LF），用于 STARTTLS 前后手工收发。
fn read_line(stream: &mut Stream) -> Result<String, String> {
    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = stream
            .read(&mut byte)
            .map_err(|e| format!("读取服务器响应失败: {e}"))?;
        if n == 0 {
            return Err("IMAP 服务器关闭了连接".to_string());
        }
        if byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            buf.push(byte[0]);
        }
        if buf.len() > 64 * 1024 {
            return Err("IMAP 响应行过长".to_string());
        }
    }
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// 建连并登录。
///
/// 两种安全模式最终都返回同一种会话类型：把底层流统一包进 `ReplayStream`
/// （SSL 分支前缀为空，等价于透明转发），省掉一个枚举与两套调用点的重复。
fn login(account: &Account, password: &str) -> Result<Session<ReplayStream>, String> {
    let proxy = super::effective_proxy(account);
    let stream = open_stream(
        &account.imap_host,
        account.imap_port,
        account.imap_security,
        proxy,
    )?;

    let stream = match account.imap_security {
        super::model::Security::Starttls => {
            let mut plain = stream;
            // 问候语必须先读掉，否则会被当成 STARTTLS 的响应。
            let greeting = read_line(&mut plain)?;
            if !greeting.starts_with("* OK") && !greeting.starts_with("* PREAUTH") {
                return Err(format!("IMAP 服务器问候异常: {greeting}"));
            }
            plain
                .write_all(b"a0 STARTTLS\r\n")
                .map_err(|e| format!("发送 STARTTLS 失败: {e}"))?;
            plain.flush().ok();
            loop {
                let line = read_line(&mut plain)?;
                if line.starts_with("a0 ") {
                    if !line[3..].trim_start().to_ascii_uppercase().starts_with("OK") {
                        return Err(format!("服务器拒绝 STARTTLS: {line}"));
                    }
                    break;
                }
            }
            let tls = plain.upgrade_tls(&account.imap_host)?;
            // STARTTLS 后服务器不再发问候语，但 `Client::new` 要求先读一次；
            // 垫一行合法的伪问候语把它喂过去。
            ReplayStream::new(tls, b"* OK [CAPABILITY IMAP4rev1] ready")
        }
        _ => ReplayStream::new(stream, b""),
    };

    let mut client = Client::new(stream);
    client
        .read_greeting()
        .map_err(|e| format!("读取 IMAP 问候失败: {e}"))?;
    client
        .login(account.login_name(), password)
        .map_err(|(e, _)| format!("IMAP 登录失败（{}）: {e}", account.login_name()))
}

// ---------------------------------------------------------------------------
// 文件夹
// ---------------------------------------------------------------------------

/// 角色识别：按服务器返回的 SPECIAL-USE 属性优先，其次按常见命名猜。
/// 国内邮箱（QQ/163/阿里/腾讯企业邮）与 Exchange/Gmail 的命名各不相同，这份表是必需的。
fn detect_role(name: &str, attributes: &[NameAttribute<'_>]) -> FolderRole {
    for attr in attributes {
        let text = format!("{attr:?}").to_ascii_lowercase();
        if text.contains("archive") || text.contains("allmail") {
            return FolderRole::Archive;
        }
        if text.contains("drafts") {
            return FolderRole::Drafts;
        }
        if text.contains("flagged") {
            continue;
        }
        if text.contains("junk") || text.contains("spam") {
            return FolderRole::Junk;
        }
        if text.contains("sent") {
            return FolderRole::Sent;
        }
        if text.contains("trash") {
            return FolderRole::Trash;
        }
    }
    let lower = name.to_ascii_lowercase();
    if lower == "inbox" {
        return FolderRole::Inbox;
    }
    let display = utf7::decode(name).to_ascii_lowercase();
    let hay = format!("{lower} {display}");
    let has = |needles: &[&str]| needles.iter().any(|n| hay.contains(n));
    if has(&["sent", "已发送", "已寄出", "outbox"]) {
        return FolderRole::Sent;
    }
    if has(&["draft", "草稿"]) {
        return FolderRole::Drafts;
    }
    if has(&["trash", "deleted", "已删除", "垃圾箱", "废纸篓", "回收站"]) {
        return FolderRole::Trash;
    }
    if has(&["junk", "spam", "垃圾邮件", "广告", "bulk"]) {
        return FolderRole::Junk;
    }
    if has(&["archive", "存档", "归档", "all mail"]) {
        return FolderRole::Archive;
    }
    FolderRole::Custom
}

/// 列出文件夹并带回未读数。`LIST "" "*"` 一次拿全，`STATUS` 逐个查未读。
pub fn list_folders(account: &Account, password: &str) -> Result<Vec<MailFolder>, String> {
    let mut session = login(account, password)?;
    let names = session
        .list(None, Some("*"))
        .map_err(|e| format!("列出文件夹失败: {e}"))?;

    let mut folders: Vec<MailFolder> = Vec::new();
    for name in names.iter() {
        let selectable = !name
            .attributes()
            .iter()
            .any(|a| matches!(a, NameAttribute::NoSelect));
        let raw_name = name.name().to_string();
        let mut role = detect_role(&raw_name, name.attributes());
        // 用户在账户里手工映射过的角色优先（键是服务器名，值是角色名）。
        if let Some(role_name) = account.folder_map.get(&raw_name) {
            role = FolderRole::parse(role_name.trim());
        }

        let (unread, total) = if selectable {
            match session.status(&raw_name, "(MESSAGES UNSEEN)") {
                Ok(status) => (status.unseen.unwrap_or(0), status.exists),
                Err(e) => {
                    debug!(folder = %raw_name, error = %e, "mail: STATUS 失败，按 0 处理");
                    (0, 0)
                }
            }
        } else {
            (0, 0)
        };

        folders.push(MailFolder {
            account_id: account.id.clone(),
            display_name: utf7::decode(&raw_name),
            name: raw_name,
            delimiter: name.delimiter().map(|d| d.to_string()),
            role,
            unread,
            total,
            uid_validity: 0,
            uid_next: 0,
            selectable,
        });
    }

    // 补齐缺失的标准角色。
    //
    // 有的服务器没有「草稿箱」或「已发送」（新账户尤其常见），但用户仍需要一个
    // 固定位置——Mailspring / Outlook 都会把这类入口一直显示出来。
    // 这里补成占位项；第一次往里写（保存草稿 / 发信存档）时再由调用方 CREATE。
    //
    // 只补 草稿 / 已发送 / 已删除 这三个「必须有」的：
    // 存档在部分服务商那里是 Gmail 的 All Mail（语义不同），不强行造一个。
    for (role, display, default_name) in [
        (FolderRole::Drafts, "草稿", "Drafts"),
        (FolderRole::Sent, "已发送", "Sent"),
        (FolderRole::Trash, "已删除", "Trash"),
    ] {
        if folders.iter().any(|f| f.role == role) {
            continue;
        }
        folders.push(MailFolder {
            account_id: account.id.clone(),
            name: default_name.to_string(),
            display_name: display.to_string(),
            delimiter: None,
            role,
            unread: 0,
            total: 0,
            uid_validity: 0,
            uid_next: 0,
            selectable: true,
        });
    }

    folders.sort_by(|a, b| {
        role_order(a.role)
            .cmp(&role_order(b.role))
            .then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    let _ = session.logout();
    Ok(folders)
}

fn role_order(role: FolderRole) -> u8 {
    match role {
        FolderRole::Inbox => 0,
        FolderRole::Drafts => 1,
        FolderRole::Sent => 2,
        FolderRole::Archive => 3,
        FolderRole::Junk => 4,
        FolderRole::Trash => 5,
        FolderRole::Custom => 6,
    }
}

/// 服务器上的「已发送」文件夹名。优先用户映射，其次按已同步的文件夹角色找。
pub fn sent_folder_name(account: &Account) -> Option<String> {
    if let Some(mapped) = account
        .folder_map
        .iter()
        .find(|(_, role)| role.trim() == "sent")
        .map(|(name, _)| name.clone())
    {
        return Some(mapped);
    }
    store::list_folders(&account.id)
        .ok()?
        .into_iter()
        .find(|f| f.role == FolderRole::Sent)
        .map(|f| f.name)
}

/// 在服务器上新建文件夹（`CREATE`）。名字按 modified UTF-7 编码。
pub fn create_folder(account: &Account, password: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("文件夹名不能为空".to_string());
    }
    let mut session = login(account, password)?;
    let encoded = utf7::encode(name);
    session
        .create(&encoded)
        .map_err(|e| format!("创建文件夹失败: {e}"))?;
    let _ = session.subscribe(&encoded);
    let _ = session.logout();
    Ok(())
}

/// 删除服务器上的文件夹（`DELETE`）。
///
/// 只允许删「用户自建」的文件夹：收件箱 / 已发送 / 草稿 / 垃圾箱这些角色文件夹
/// 一旦删掉，服务端往往是整个文件夹连同里面的邮件一起没，属于不可逆操作。
/// 调用方（命令层）已经做过角色校验，这里再挡一道。
pub fn delete_folder(account: &Account, password: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("文件夹名为空".to_string());
    }
    if name.eq_ignore_ascii_case("INBOX") {
        return Err("收件箱不能删除".to_string());
    }
    let mut session = login(account, password)?;
    let encoded = utf7::encode(name);
    session
        .delete(&encoded)
        .map_err(|e| format!("删除文件夹失败: {e}"))?;
    let _ = session.logout();
    Ok(())
}

// ---------------------------------------------------------------------------
// 同步
// ---------------------------------------------------------------------------

/// 同步一个文件夹里的新邮件。
///
/// 策略：`UID SEARCH UID <last+1>:*` 找出比本地更大的 UID，取最近 `MAX_FETCH_PER_SYNC`
/// 封的完整报文（列表摘要需要正文前缀，全文索引也需要它），解析后入库。
/// `BODY.PEEK` 不会把邮件标成已读——后台同步不能替用户读邮件。
pub fn sync_folder(
    account: &Account,
    password: &str,
    folder_name: &str,
    uid_validity_hint: u32,
    last_uid: u32,
    snippet_len: usize,
) -> Result<Vec<MessageSummary>, String> {
    let mut session = login(account, password)?;
    let mailbox = session
        .select(folder_name)
        .map_err(|e| format!("选择文件夹失败（{folder_name}）: {e}"))?;

    let uid_validity = mailbox.uid_validity.unwrap_or(0);
    // UIDVALIDITY 变了说明服务器重建了 UID 空间，本地缓存必须整段作废。
    if uid_validity != 0 && uid_validity_hint != 0 && uid_validity != uid_validity_hint {
        warn!(
            folder = folder_name,
            old = uid_validity_hint,
            new = uid_validity,
            "mail: UIDVALIDITY 变化，清空该文件夹缓存"
        );
        store::clear_folder_messages(&account.id, folder_name)?;
    }

    let uids = session
        .uid_search(format!("UID {}:*", last_uid + 1))
        .map_err(|e| format!("检索新邮件失败: {e}"))?;
    let mut uids: Vec<u32> = uids.into_iter().filter(|uid| *uid > last_uid).collect();
    uids.sort_unstable();
    // 超出一批的留到下次；先处理最新的。
    if uids.len() > MAX_FETCH_PER_SYNC as usize {
        uids = uids.split_off(uids.len() - MAX_FETCH_PER_SYNC as usize);
    }

    if uids.is_empty() {
        store::update_folder_counts(
            &account.id,
            folder_name,
            mailbox.unseen.unwrap_or(0),
            mailbox.exists,
            uid_validity,
            mailbox.uid_next.unwrap_or(0),
        )?;
        let _ = session.logout();
        return Ok(Vec::new());
    }

    let set = compact_uid_set(&uids);
    let fetches = session
        .uid_fetch(&set, "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])")
        .map_err(|e| format!("拉取邮件失败: {e}"))?;

    let role = store::list_folders(&account.id)
        .ok()
        .and_then(|list| list.into_iter().find(|f| f.name == folder_name))
        .map(|f| f.role)
        .unwrap_or(FolderRole::Custom);
    let outgoing_folder = matches!(role, FolderRole::Sent | FolderRole::Drafts);

    let mut summaries: Vec<MessageSummary> = Vec::new();
    // (uid, html, text, 附件清单 JSON)：附件清单要一起入库，否则「列表里看到回形针、
    // 点开却没有附件」——正文走缓存时就没有第二次解析的机会了。
    let mut bodies: Vec<(u32, Option<String>, String, String)> = Vec::new();

    for fetch in fetches.iter() {
        let Some(uid) = fetch.uid else { continue };
        let Some(raw) = fetch.body() else { continue };
        let Some(parsed) = mime::parse(raw, true) else {
            debug!(uid, "mail: 报文解析失败，跳过");
            continue;
        };

        let (mut seen, mut flagged, mut answered, mut draft, mut deleted) =
            (false, false, false, false, false);
        for flag in fetch.flags() {
            match flag {
                Flag::Seen => seen = true,
                Flag::Flagged => flagged = true,
                Flag::Answered => answered = true,
                Flag::Draft => draft = true,
                Flag::Deleted => deleted = true,
                _ => {}
            }
        }

        let date = if parsed.date > 0 {
            parsed.date
        } else {
            fetch
                .internal_date()
                .map(|d| d.timestamp())
                .unwrap_or_else(|| chrono::Utc::now().timestamp())
        };

        summaries.push(MessageSummary {
            account_id: account.id.clone(),
            folder: folder_name.to_string(),
            uid,
            message_id: parsed.message_id.clone(),
            subject: parsed.subject.clone(),
            from_name: parsed.from_name.clone(),
            from_addr: parsed.from_addr.clone(),
            to_json: serde_json::to_string(&parsed.to).unwrap_or_else(|_| "[]".into()),
            cc_json: serde_json::to_string(&parsed.cc).unwrap_or_else(|_| "[]".into()),
            date,
            size: fetch.size.unwrap_or(raw.len() as u32),
            snippet: sanitize::truncate_chars(&parsed.snippet_source, snippet_len),
            seen,
            flagged,
            answered,
            draft,
            deleted,
            has_attachments: parsed.has_attachments,
            thread_key: mime::thread_key(&parsed.subject),
            labels: String::new(),
            outgoing: outgoing_folder,
        });

        bodies.push((
            uid,
            parsed.html.clone(),
            parsed.text.clone(),
            serde_json::to_string(&parsed.attachments).unwrap_or_else(|_| "[]".into()),
        ));
    }

    store::upsert_summaries(&summaries)?;
    // 本批邮件的正文已经为列表摘要解析过了，全部入库。这样全文索引从一开始就是完整的，
    // 「搜正文搜不到刚同步的邮件」这类问题不会出现；一次最多 MAX_FETCH_PER_SYNC 封，
    // 写入开销与摘要本身同量级。
    for (uid, html, text, attachments) in &bodies {
        store::upsert_body(&account.id, folder_name, *uid, html.as_deref(), text, attachments)?;
    }
    store::update_folder_counts(
        &account.id,
        folder_name,
        mailbox.unseen.unwrap_or(0),
        mailbox.exists,
        uid_validity,
        mailbox.uid_next.unwrap_or(0),
    )?;

    info!(
        account = %account.email,
        folder = folder_name,
        fetched = summaries.len(),
        "mail: 同步完成"
    );
    let _ = session.logout();
    Ok(summaries)
}

/// UID 集合压缩成区间串（`1:5,8,10:12`），减少协议体积。
fn compact_uid_set(uids: &[u32]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut iter = uids.iter().copied().peekable();
    while let Some(start) = iter.next() {
        let mut end = start;
        while let Some(next) = iter.peek() {
            if *next == end + 1 {
                end = *next;
                iter.next();
            } else {
                break;
            }
        }
        if end > start {
            parts.push(format!("{start}:{end}"));
        } else {
            parts.push(start.to_string());
        }
    }
    parts.join(",")
}

/// 取单封邮件的完整原文（正文缺失时按需补拉；也用于附件下载）。
pub fn fetch_raw(account: &Account, password: &str, folder: &str, uid: u32) -> Result<Vec<u8>, String> {
    let mut session = login(account, password)?;
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let fetches = session
        .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
        .map_err(|e| format!("拉取邮件失败: {e}"))?;
    let raw = fetches
        .iter()
        .find_map(|f| f.body().map(|b| b.to_vec()))
        .ok_or_else(|| "服务器没有返回邮件内容".to_string())?;
    let _ = session.logout();
    Ok(raw)
}

// ---------------------------------------------------------------------------
// 写操作
// ---------------------------------------------------------------------------

/// 回传标记：`\Seen` / `\Flagged`。先改服务器，再由调用方改本地库——
/// 顺序反过来会出现「本地已读、服务器失败」的不一致。
pub fn set_flags(
    account: &Account,
    password: &str,
    folder: &str,
    uids: &[u32],
    seen: Option<bool>,
    flagged: Option<bool>,
) -> Result<(), String> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut session = login(account, password)?;
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let set = compact_uid_set(uids);
    let mut ops: Vec<String> = Vec::new();
    if let Some(v) = seen {
        ops.push(format!(
            "{}FLAGS.SILENT (\\Seen)",
            if v { "+" } else { "-" }
        ));
    }
    if let Some(v) = flagged {
        ops.push(format!(
            "{}FLAGS.SILENT (\\Flagged)",
            if v { "+" } else { "-" }
        ));
    }
    for op in ops {
        session
            .uid_store(&set, &op)
            .map_err(|e| format!("更新标记失败: {e}"))?;
    }
    let _ = session.logout();
    Ok(())
}

/// 移动邮件。服务器支持 MOVE（RFC 6851）时一步完成，否则 COPY + 标记删除 + EXPUNGE。
pub fn move_messages(
    account: &Account,
    password: &str,
    folder: &str,
    uids: &[u32],
    target: &str,
) -> Result<(), String> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut session = login(account, password)?;
    let supports_move = session
        .capabilities()
        .map(|caps| caps.has_str("MOVE"))
        .unwrap_or(false);

    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let set = compact_uid_set(uids);
    let encoded_target = utf7::encode(target);

    if supports_move {
        session
            .uid_mv(&set, &encoded_target)
            .map_err(|e| format!("移动邮件失败: {e}"))?;
    } else {
        session
            .uid_copy(&set, &encoded_target)
            .map_err(|e| format!("复制到目标文件夹失败: {e}"))?;
        session
            .uid_store(&set, "+FLAGS.SILENT (\\Deleted)")
            .map_err(|e| format!("标记删除失败: {e}"))?;
        // 有 UIDPLUS 时用 UID EXPUNGE，避免误删别的客户端标记的邮件。
        if session.uid_expunge(&set).is_err() {
            session
                .expunge()
                .map_err(|e| format!("清理已删除邮件失败: {e}"))?;
        }
    }
    let _ = session.logout();
    Ok(())
}

/// 永久删除（从垃圾箱彻底删除）。
pub fn delete_permanently(
    account: &Account,
    password: &str,
    folder: &str,
    uids: &[u32],
) -> Result<(), String> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut session = login(account, password)?;
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let set = compact_uid_set(uids);
    session
        .uid_store(&set, "+FLAGS.SILENT (\\Deleted)")
        .map_err(|e| format!("标记删除失败: {e}"))?;
    if session.uid_expunge(&set).is_err() {
        session.expunge().map_err(|e| format!("清理失败: {e}"))?;
    }
    let _ = session.logout();
    Ok(())
}

/// 把已发送 / 草稿追加到服务器文件夹。
pub fn append_to_folder(
    account: &Account,
    password: &str,
    folder: &str,
    raw: &[u8],
    flags: &[&str],
) -> Result<(), String> {
    let mut session = login(account, password)?;
    let encoded = utf7::encode(folder);
    let parsed_flags: Vec<Flag<'static>> = flags
        .iter()
        .filter_map(|f| match f.to_ascii_lowercase().as_str() {
            "seen" => Some(Flag::Seen),
            "draft" => Some(Flag::Draft),
            "answered" => Some(Flag::Answered),
            "flagged" => Some(Flag::Flagged),
            _ => None,
        })
        .collect();
    let date: Option<chrono::DateTime<chrono::FixedOffset>> =
        Some(chrono::Local::now().fixed_offset());
    // 服务器上还没有这个文件夹（新建账户常见）时先建再追加，
    // 否则「保存草稿」会直接失败——用户会觉得草稿功能坏了。
    if session
        .append_with_flags_and_date(&encoded, raw, &parsed_flags, date)
        .is_err()
    {
        session
            .create(&encoded)
            .map_err(|e| format!("创建「{folder}」失败: {e}"))?;
        let _ = session.subscribe(&encoded);
        let date: Option<chrono::DateTime<chrono::FixedOffset>> =
            Some(chrono::Local::now().fixed_offset());
        session
            .append_with_flags_and_date(&encoded, raw, &parsed_flags, date)
            .map_err(|e| format!("保存到「{folder}」失败: {e}"))?;
    }
    let _ = session.logout();
    Ok(())
}

/// 连接诊断：失败信息写在 `detail` 里，不抛错。
pub fn probe(account: &Account, password: &str) -> super::model::ConnectionProbe {
    use super::model::ConnectionProbe;
    let mut probe = ConnectionProbe {
        ok: false,
        stage: "connect".into(),
        detail: String::new(),
        capabilities: Vec::new(),
    };
    match login(account, password) {
        Ok(mut session) => {
            probe.stage = "capabilities".into();
            if let Ok(caps) = session.capabilities() {
                probe.capabilities = caps.iter().map(|c| format!("{c:?}")).collect();
            }
            probe.stage = "folders".into();
            match session.list(None, Some("*")) {
                Ok(names) => {
                    probe.ok = true;
                    probe.stage = "done".into();
                    probe.detail = format!("连接正常，共 {} 个文件夹", names.len());
                }
                Err(e) => probe.detail = format!("列出文件夹失败: {e}"),
            }
            let _ = session.logout();
        }
        Err(e) => probe.detail = e,
    }
    probe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf7_roundtrip_chinese() {
        for name in ["已发送", "收件箱", "Draft 草稿", "测试 文件夹"] {
            let encoded = utf7::encode(name);
            assert_eq!(utf7::decode(&encoded), name, "roundtrip failed for {name}");
        }
    }

    #[test]
    fn utf7_escapes_ampersand() {
        assert_eq!(utf7::encode("A&B"), "A&-B");
        assert_eq!(utf7::decode("A&-B"), "A&B");
    }

    #[test]
    fn utf7_matches_rfc_encoding() {
        assert_eq!(utf7::decode("&XfJT0ZAB-"), "已发送");
    }

    #[test]
    fn compact_uid_set_builds_ranges() {
        assert_eq!(compact_uid_set(&[1, 2, 3, 5, 7, 8]), "1:3,5,7:8");
        assert_eq!(compact_uid_set(&[42]), "42");
    }

    #[test]
    fn role_detection_covers_localised_names() {
        assert_eq!(detect_role("INBOX", &[]), FolderRole::Inbox);
        assert_eq!(detect_role("已发送", &[]), FolderRole::Sent);
        assert_eq!(detect_role("Drafts", &[]), FolderRole::Drafts);
        assert_eq!(detect_role("垃圾邮件", &[]), FolderRole::Junk);
        assert_eq!(detect_role("已删除", &[]), FolderRole::Trash);
        assert_eq!(detect_role("Project X", &[]), FolderRole::Custom);
    }
}
