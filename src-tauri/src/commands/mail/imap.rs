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
//!
//! 连接复用与 IDLE 见文件末尾的 `conn` 子模块：短连接模式在 `imap.rs` 里保留为
//! 「拿不到长连接时的兜底」，而 UI 交互路径一律先问连接池。

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
pub struct ReplayStream {
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

impl ::imap::extensions::idle::SetReadTimeout for ReplayStream {
    fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> ::imap::Result<()> {
        // 底层 `Stream::set_read_timeout` 返回的是本模块的 String 错误，
        // 这里要换成 imap crate 的错误类型（`SetReadTimeout` 的签名要求）。
        self.inner
            .set_read_timeout(timeout)
            .map_err(|e| ::imap::Error::Io(std::io::Error::other(e)))
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
///
/// `secret` 的含义取决于 `account.auth_kind`：`password` 时是口令 / 授权码，
/// `oauth2` 时是访问令牌（由调用方用刷新令牌换好）。
pub fn login(account: &Account, password: &str) -> Result<ImapSession, String> {
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
    if account.auth_kind.eq_ignore_ascii_case("oauth2") {
        let auth = super::oauth::XOAuth2::new(account.login_name(), password);
        return client
            .authenticate("XOAUTH2", &auth)
            .map_err(|(e, _)| format!("IMAP OAuth2 认证失败（{}）: {e}", account.login_name()));
    }
    client
        .login(account.login_name(), password)
        .map_err(|(e, _)| format!("IMAP 登录失败（{}）: {e}", account.login_name()))
}

/// 取一个「可直接用于认证」的秘密。
///
/// 密码账户直接用 keychain 里的口令；OAuth2 账户则先换访问令牌
/// （过期时自动刷新），这样所有调用点都不需要关心认证方式的差异。
pub fn auth_secret(account: &Account) -> Result<String, String> {
    if account.auth_kind.eq_ignore_ascii_case("oauth2") {
        super::oauth::access_token(account)
    } else {
        super::account_password(account)
    }
}

/// 认证后的 IMAP 会话类型（连接池与短连接共用）。
pub type ImapSession = Session<ReplayStream>;

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
    // 文件夹发现是一次性重活，走短连接：它自己会 LOGOUT，
    // 不必占用请求连接（否则同步中途发现文件夹会打断正在进行的 FETCH）。
    let mut session = login(account, password)?;
    // 先把 LIST 结果拷成自有数据：后面 STATUS 出错要重连，不能再借着 `names` 借用旧会话。
    let mut listed: Vec<(String, Option<String>, bool, FolderRole)> = Vec::new();
    {
        let names = session
            .list(None, Some("*"))
            .map_err(|e| format!("列出文件夹失败: {e}"))?;
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
            listed.push((
                raw_name,
                name.delimiter().map(|d| d.to_string()),
                selectable,
                role,
            ));
        }
    }

    let mut statuses = StatusReader::new(session, account, password);
    let mut folders: Vec<MailFolder> = Vec::new();
    for (raw_name, delimiter, selectable, role) in listed {
        let (unread, total) = if selectable {
            statuses.counts(&raw_name)
        } else {
            (0, 0)
        };

        folders.push(MailFolder {
            account_id: account.id.clone(),
            display_name: utf7::decode(&raw_name),
            name: raw_name,
            delimiter,
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
    let _ = statuses.logout();
    Ok(folders)
}

/// STATUS 的容错读取器。
///
/// 为什么不直接用 `Session::status`：crate 要求响应行完全按 RFC 文本解析。
/// QQ 邮箱会用 GBK 回显中文文件夹名（如 `其他文件夹`），解析失败时它已经读掉了
/// 那行响应，**却没有读掉这条命令的 tagged 完成行**；下一条 STATUS 会把上一条的
/// 完成行当自己的，撞上 crate 内部的 `assert_eq!(tag, match_tag)` 并 panic
/// （前端看到的就是 `task ... panicked with message "assertion left == right failed"`）。
///
/// 这里的做法：
/// 1. 自己发 STATUS，只用宽松的数字扫描取 MESSAGES / UNSEEN；
/// 2. 任何可能留下残留响应的错误都立刻重连，绝不把未读完的响应带到下一条命令。
struct StatusReader<'a> {
    account: &'a Account,
    password: &'a str,
    session: Option<ImapSession>,
}

impl<'a> StatusReader<'a> {
    fn new(session: ImapSession, account: &'a Account, password: &'a str) -> Self {
        Self {
            account,
            password,
            session: Some(session),
        }
    }

    /// 返回 `(未读数, 总数)`；拿不到时按 0 处理——未读角标不该阻断文件夹列表。
    fn counts(&mut self, mailbox: &str) -> (u32, u32) {
        let Some(session) = self.session.as_mut() else {
            return (0, 0);
        };
        let Some(quoted) = quote_mailbox(mailbox) else {
            return (0, 0);
        };
        let command = format!("STATUS {quoted} (MESSAGES UNSEEN)");
        match session.run_command_and_read_response(&command) {
            Ok(raw) => parse_status_counts(&raw),
            Err(error) => {
                let parsed = match &error {
                    imap::Error::Parse(imap::error::ParseError::Invalid(bytes)) => {
                        // 原始行可能含非 UTF-8（GBK）字节，lossy 只用于日志。
                        debug!(
                            folder = %mailbox,
                            response = %String::from_utf8_lossy(bytes),
                            "mail: STATUS 响应不是合法 IMAP 文本，改用宽松解析"
                        );
                        Some(parse_status_counts(bytes))
                    }
                    _ => None,
                };
                // NO/BAD 已经是一条完整应答；VALIDATE 压根没发出去。
                // 其余错误（解析失败、IO、连接断开）都可能残留半条响应，必须换连接。
                let poisoned = !matches!(
                    &error,
                    imap::Error::No(_) | imap::Error::Bad(_) | imap::Error::Validate(_)
                );
                if poisoned {
                    debug!(folder = %mailbox, error = %error, "mail: STATUS 通道已不可靠，重连");
                    self.session = match login(self.account, self.password) {
                        Ok(session) => Some(session),
                        Err(relogin) => {
                            debug!(error = %relogin, "mail: STATUS 重连失败，后续文件夹按 0 处理");
                            None
                        }
                    };
                }
                parsed.unwrap_or((0, 0))
            }
        }
    }

    fn logout(&mut self) {
        if let Some(mut session) = self.session.take() {
            let _ = session.logout();
        }
    }
}

/// `STATUS` 的邮箱名一律加引号并转义，避免名字里的空格 / 反斜杠 / 引号把命令切坏。
fn quote_mailbox(name: &str) -> Option<String> {
    if name.contains('\r') || name.contains('\n') {
        return None;
    }
    let mut out = String::with_capacity(name.len() + 2);
    out.push('"');
    for ch in name.chars() {
        if ch == '\\' || ch == '"' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    Some(out)
}

/// 从 STATUS 响应里取 `UNSEEN` / `MESSAGES`。不依赖响应能整体解析成 UTF-8，
/// 所以非 ASCII 回显（GBK）也照样能读出数字。
fn parse_status_counts(bytes: &[u8]) -> (u32, u32) {
    (
        number_after(bytes, b"UNSEEN").unwrap_or(0),
        number_after(bytes, b"MESSAGES").unwrap_or(0),
    )
}

fn number_after(haystack: &[u8], key: &[u8]) -> Option<u32> {
    if key.is_empty() || haystack.len() < key.len() {
        return None;
    }
    let mut found = None;
    for start in 0..=haystack.len() - key.len() {
        if !haystack[start..start + key.len()].eq_ignore_ascii_case(key) {
            continue;
        }
        let mut i = start + key.len();
        while i < haystack.len() && haystack[i].is_ascii_whitespace() {
            i += 1;
        }
        let digits = i;
        while i < haystack.len() && haystack[i].is_ascii_digit() {
            i += 1;
        }
        if i > digits {
            if let Ok(text) = std::str::from_utf8(&haystack[digits..i]) {
                if let Ok(value) = text.parse::<u32>() {
                    // 取最后一次出现：文件夹名本身含同名子串时也不会误取。
                    found = Some(value);
                }
            }
        }
    }
    found
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
    let summaries = sync_folder_with(account, &mut session, folder_name, uid_validity_hint, last_uid, snippet_len)?;
    let _ = session.logout();
    Ok(summaries)
}

/// 同步的核心：在一条已经认证、尚未 SELECT 的会话上完成增量拉取。
///
/// 拆出来是为了让连接池复用它——池里的连接已经 SELECT 过别的文件夹，
/// 所以这里自己负责 SELECT；短连接路径直接复用同一份逻辑。
pub fn sync_folder_with(
    account: &Account,
    session: &mut ImapSession,
    folder_name: &str,
    uid_validity_hint: u32,
    last_uid: u32,
    snippet_len: usize,
) -> Result<Vec<MessageSummary>, String> {
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
    Ok(summaries)
}

/// 多设备一致性：把服务器上的 FLAGS 与存在性全量比对回本地。
///
/// 为什么需要它：增量同步只拉「UID 比本地大」的新邮件，而「已读 / 星标 / 回复过」
/// 是**就地变化**的——手机上读了、别的客户端加了星，UID 不变，本地永远显示旧的。
/// 别的客户端把邮件移走 / 删掉，本地也还留着。
///
/// 实现上没有走 CONDSTORE/QRESYNC：两者都需要服务器声明支持，而国内邮箱
/// （QQ/163/阿里）普遍不支持，骨干路径不能押在可选扩展上。这里改用
/// `UID FETCH (FLAGS)` 全量比对——只取 FLAGS 不取正文，十万封也就是几百 KB，
/// 由 `flag_sync_minutes` 控制频率（默认 10 分钟），并且只比对本地已有的 UID。
pub fn sync_flags(
    session: &mut ImapSession,
    account: &Account,
    folder: &str,
    local_uids: &[u32],
) -> Result<FlagSyncResult, String> {
    if local_uids.is_empty() {
        return Ok(FlagSyncResult::default());
    }
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败（{folder}）: {e}"))?;

    let mut server: std::collections::HashMap<u32, (bool, bool, bool, bool)> =
        std::collections::HashMap::new();
    // 分块 FETCH：一次几千个 UID 的命令行会被服务器拒绝（RFC 3501 规定命令长度上限）。
    for chunk in local_uids.chunks(500) {
        let set = compact_uid_set(chunk);
        let fetches = session
            .uid_fetch(&set, "(UID FLAGS)")
            .map_err(|e| format!("比对标记失败: {e}"))?;
        for fetch in fetches.iter() {
            let Some(uid) = fetch.uid else { continue };
            let (mut seen, mut flagged, mut answered, mut deleted) = (false, false, false, false);
            for flag in fetch.flags() {
                match flag {
                    Flag::Seen => seen = true,
                    Flag::Flagged => flagged = true,
                    Flag::Answered => answered = true,
                    Flag::Deleted => deleted = true,
                    _ => {}
                }
            }
            server.insert(uid, (seen, flagged, answered, deleted));
        }
    }

    let changed: Vec<(u32, bool, bool, bool, bool)> = server
        .iter()
        .map(|(uid, (s, f, a, d))| (*uid, *s, *f, *a, *d))
        .collect();
    let updated = store::apply_server_flags(&account.id, folder, &changed)?;

    let present: std::collections::HashSet<u32> = server.keys().copied().collect();
    let removed = store::drop_missing_uids(&account.id, folder, &present)?;

    if updated > 0 || removed > 0 {
        info!(
            account = %account.email,
            folder,
            updated,
            removed,
            "mail: 多设备标记比对有变化"
        );
    }
    Ok(FlagSyncResult { updated, removed })
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagSyncResult {
    pub updated: usize,
    pub removed: usize,
}

/// 走连接池的增量同步（UI 交互路径统一用它）。连接池不可用时回退短连接。
pub fn sync_folder_pooled(
    account: &Account,
    folder_name: &str,
    uid_validity_hint: u32,
    last_uid: u32,
    snippet_len: usize,
) -> Result<Vec<MessageSummary>, String> {
    let mut lease = try_lease(account)?;
    sync_folder_with(
        account,
        lease.session(),
        folder_name,
        uid_validity_hint,
        last_uid,
        snippet_len,
    )
}

/// 走连接池的全量 FLAGS 比对。本地没有该文件夹的缓存时直接返回空结果，
/// 不必为一个空文件夹做一次完整 SELECT。
pub fn sync_flags_pooled(
    account: &Account,
    folder_name: &str,
) -> Result<FlagSyncResult, String> {
    let local = store::uids_in_folder(&account.id, folder_name)?;
    if local.is_empty() {
        return Ok(FlagSyncResult::default());
    }
    let mut lease = try_lease(account)?;
    sync_flags(lease.session(), account, folder_name, &local)
}

/// 账户在有委托 / 共享邮箱时的「邮箱命名空间前缀」。
///
/// 三家的写法完全不同，必须分别处理：
/// - Exchange / Office 365 / Dovecot：`Other Users/<邮箱>/<文件夹>`（分隔符是 `/`）；
/// - Courier / 部分国内企业邮：`共享文件夹/<邮箱>/<文件夹>`；
/// - iCloud：`Shared/<邮箱>/<文件夹>`。
///
/// 这里不做「探测」，因为探测意味着为每个账户多开一次 LIST 往返；
/// 由用户在账户设置里选一个命名空间，默认按服务商类型猜一个。
pub fn delegate_prefix(account: &Account, mailbox: &str) -> String {
    let domain = mime::domain_of(&account.email);
    if domain.ends_with("icloud.com") || domain.ends_with("me.com") {
        return format!("Shared/{mailbox}");
    }
    if domain.ends_with("qq.com") || domain.ends_with("163.com") || domain.ends_with("126.com") {
        return format!("共享文件夹/{mailbox}");
    }
    format!("Other Users/{mailbox}")
}

/// 列出一个共享邮箱下的文件夹（`LIST "<prefix>/*"`）。
///
/// 返回的文件夹名保持服务器原样（`Other Users/x@y.com/Inbox`），
/// 调用方在前面加上「共享邮箱」分组展示。
pub fn list_delegate_folders(
    account: &Account,
    password: &str,
    mailbox: &str,
) -> Result<Vec<MailFolder>, String> {
    let mut session = login(account, password)?;
    let prefix = delegate_prefix(account, mailbox);
    // 先 LIST 前缀本身：有的服务器只暴露顶层子文件夹，`prefix/*` 不会递归。
    let pattern = format!("{prefix}/*");
    let listed: Vec<(String, Option<String>)> = {
        let names = session
            .list(None, Some(&pattern))
            .map_err(|e| format!("列出共享邮箱失败（{mailbox}）: {e}"))?;
        names
            .iter()
            .filter(|name| {
                !name
                    .attributes()
                    .iter()
                    .any(|a| matches!(a, NameAttribute::NoSelect))
            })
            .map(|name| {
                (
                    name.name().to_string(),
                    name.delimiter().map(|d| d.to_string()),
                )
            })
            .collect()
    };

    let mut statuses = StatusReader::new(session, account, password);
    let mut folders = Vec::new();
    for (raw_name, delimiter) in listed {
        // 共享邮箱里的文件夹同样可能是中文名，STATUS 走同一套容错读取。
        let (unread, total) = statuses.counts(&raw_name);
        // 显示名去掉前缀，只留「邮箱/文件夹」之后的部分，读起来才不啰嗦。
        let trimmed = raw_name
            .strip_prefix(&format!("{prefix}/"))
            .unwrap_or(&raw_name)
            .to_string();
        folders.push(MailFolder {
            account_id: account.id.clone(),
            display_name: utf7::decode(&trimmed),
            name: raw_name,
            delimiter,
            role: FolderRole::Custom,
            unread,
            total,
            uid_validity: 0,
            uid_next: 0,
            selectable: true,
        });
    }
    statuses.logout();
    Ok(folders)
}

/// 把共享邮箱的收件箱等标准文件夹补成角色，方便侧栏与搜索按角色过滤。
pub fn mark_delegate_roles(folders: &mut [MailFolder], mailbox: &str) {
    let prefix = folders
        .first()
        .map(|f| f.name.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_default())
        .unwrap_or_default();
    let _ = mailbox;
    for folder in folders.iter_mut() {
        let tail = folder
            .name
            .rsplit_once('/')
            .map(|(_, t)| t.to_ascii_lowercase())
            .unwrap_or_default();
        folder.role = match tail.as_str() {
            "inbox" | "收件箱" => FolderRole::Inbox,
            "sent items" | "sent" | "已发送" => FolderRole::Sent,
            "drafts" | "草稿" => FolderRole::Drafts,
            "deleted items" | "trash" | "已删除" => FolderRole::Trash,
            "junk email" | "junk" | "垃圾邮件" => FolderRole::Junk,
            "archive" | "存档" => FolderRole::Archive,
            _ => FolderRole::Custom,
        };
    }
    let _ = prefix;
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
    // 优先复用长连接：读一封信不该付一次完整 TLS 握手的代价。
    if let Ok(mut lease) = try_lease(account) {
        lease.select(folder)?;
        return fetch_raw_with(lease.session(), uid);
    }
    let secret = if password.is_empty() { auth_secret(account)? } else { password.to_string() };
    let mut session = login(account, &secret)?;
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let raw = fetch_raw_with(&mut session, uid)?;
    let _ = session.logout();
    Ok(raw)
}

fn fetch_raw_with(session: &mut ImapSession, uid: u32) -> Result<Vec<u8>, String> {
    let fetches = session
        .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
        .map_err(|e| format!("拉取邮件失败: {e}"))?;
    fetches
        .iter()
        .find_map(|f| f.body().map(|b| b.to_vec()))
        .ok_or_else(|| "服务器没有返回邮件内容".to_string())
}

/// 尝试借用连接池里的长连接。失败（账户刚加、网络刚断）返回 Err，
/// 调用方回退到短连接——保持「连接池是优化，不是唯一路径」。
fn try_lease(account: &Account) -> Result<super::pool::Lease, String> {
    super::pool::lease(account)
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
    if let Ok(mut lease) = try_lease(account) {
        lease.select(folder)?;
        return set_flags_with(lease.session(), uids, seen, flagged);
    }
    let secret = if password.is_empty() { auth_secret(account)? } else { password.to_string() };
    let mut session = login(account, &secret)?;
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let result = set_flags_with(&mut session, uids, seen, flagged);
    let _ = session.logout();
    result
}

fn set_flags_with(
    session: &mut ImapSession,
    uids: &[u32],
    seen: Option<bool>,
    flagged: Option<bool>,
) -> Result<(), String> {
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
    if let Ok(mut lease) = try_lease(account) {
        let supports_move = lease.supports("MOVE");
        lease.select(folder)?;
        return move_with(lease.session(), supports_move, uids, target);
    }
    let secret = if password.is_empty() { auth_secret(account)? } else { password.to_string() };
    let mut session = login(account, &secret)?;
    let supports_move = session
        .capabilities()
        .map(|caps| caps.has_str("MOVE"))
        .unwrap_or(false);
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let result = move_with(&mut session, supports_move, uids, target);
    let _ = session.logout();
    result
}

fn move_with(
    session: &mut ImapSession,
    supports_move: bool,
    uids: &[u32],
    target: &str,
) -> Result<(), String> {
    let set = compact_uid_set(uids);
    let encoded_target = utf7::encode(target);
    // imap crate 的 uid_copy 不会给邮箱名加引号（uid_mv 会），
    // 像「其他文件夹/Deleted Items」这种带空格的目标会被拆成两个参数，
    // QQ 直接回 [CLIENTBUG] UID parameters。这里自己引号化。
    let quoted_target =
        quote_mailbox(&encoded_target).ok_or_else(|| "目标文件夹名包含非法字符".to_string())?;

    if supports_move {
        session
            .uid_mv(&set, &encoded_target)
            .map_err(|e| format!("移动邮件失败: {e}"))?;
    } else {
        session
            .uid_copy(&set, &quoted_target)
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
    if let Ok(mut lease) = try_lease(account) {
        lease.select(folder)?;
        return delete_permanently_with(lease.session(), uids);
    }
    let secret = if password.is_empty() { auth_secret(account)? } else { password.to_string() };
    let mut session = login(account, &secret)?;
    session
        .select(folder)
        .map_err(|e| format!("选择文件夹失败: {e}"))?;
    let result = delete_permanently_with(&mut session, uids);
    let _ = session.logout();
    result
}

fn delete_permanently_with(session: &mut ImapSession, uids: &[u32]) -> Result<(), String> {
    let set = compact_uid_set(uids);
    session
        .uid_store(&set, "+FLAGS.SILENT (\\Deleted)")
        .map_err(|e| format!("标记删除失败: {e}"))?;
    if session.uid_expunge(&set).is_err() {
        session.expunge().map_err(|e| format!("清理失败: {e}"))?;
    }
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

    #[test]
    fn status_counts_survive_gbk_mailbox_echo() {
        // QQ 邮箱的真实回显：文件夹名不是 UTF-8，整行无法按 RFC 解析，
        // 但数字仍然要能取出来。
        let line = b"* STATUS \"\xc6\xe4\xcb\xfb\xce\xc4\xbc\xfe\xbc\xd0\" (MESSAGES 173 UNSEEN 4)\r\n";
        assert_eq!(parse_status_counts(line), (4, 173));
    }

    #[test]
    fn status_counts_default_to_zero() {
        assert_eq!(parse_status_counts(b"* OK nothing here\r\n"), (0, 0));
        assert_eq!(parse_status_counts(b""), (0, 0));
    }

    #[test]
    fn mailbox_names_are_quoted_and_escaped() {
        assert_eq!(quote_mailbox("INBOX").as_deref(), Some("\"INBOX\""));
        assert_eq!(quote_mailbox("a\"b").as_deref(), Some("\"a\\\"b\""));
        assert!(quote_mailbox("a\nb").is_none());
    }

    /// 回归：QQ 用 GBK 回显文件夹名导致 STATUS 解析失败时，连接里会残留
    /// 上一条命令的 tagged 完成行。以前继续在同一个会话上发下一条 STATUS，
    /// 会撞上 imap crate 的标签断言直接 panic；现在必须换连接后继续。
    #[test]
    fn status_reader_reconnects_after_unparsable_response() {
        use crate::commands::mail::model::Security;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        fn serve(mut socket: std::net::TcpStream) {
            use std::io::{BufRead, BufReader, Write as IoWrite};
            socket.write_all(b"* OK fake IMAP ready\r\n").ok();
            let mut reader = BufReader::new(socket.try_clone().expect("clone"));
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break; // 客户端在解析失败后直接丢了这条连接
                }
                let tag = line.split_whitespace().next().unwrap_or("").to_string();
                let upper = line.to_ascii_uppercase();
                if upper.contains(" LOGIN ") {
                    socket
                        .write_all(format!("{tag} OK logged in\r\n").as_bytes())
                        .ok();
                } else if upper.contains(" STATUS ") {
                    // GBK 编码的「其他文件夹」：imap crate 解析这一行会失败。
                    socket
                        .write_all(
                            b"* STATUS \"\xc6\xe4\xcb\xfb\xce\xc4\xbc\xfe\xbc\xd0\" (MESSAGES 12 UNSEEN 3)\r\n",
                        )
                        .ok();
                    socket
                        .write_all(format!("{tag} OK status done\r\n").as_bytes())
                        .ok();
                } else if upper.contains(" LOGOUT") {
                    socket
                        .write_all(format!("* BYE\r\n{tag} OK bye\r\n").as_bytes())
                        .ok();
                    break;
                } else {
                    socket.write_all(format!("{tag} OK\r\n").as_bytes()).ok();
                }
            }
        }
        let server = std::thread::spawn(move || {
            // 两次 STATUS 各自触发一次重连，最后一条连接用于 LOGOUT。
            // 每条连接单独一个线程：重连发生在旧连接被丢弃之前，串行 accept 会卡住。
            let mut handlers = Vec::new();
            for _ in 0..3 {
                let Ok((socket, _)) = listener.accept() else {
                    break;
                };
                handlers.push(std::thread::spawn(move || serve(socket)));
            }
            for handler in handlers {
                let _ = handler.join();
            }
        });

        let account = Account {
            id: "probe".into(),
            email: "me@example.com".into(),
            display_name: String::new(),
            imap_host: "127.0.0.1".into(),
            imap_port: port,
            imap_security: Security::None,
            smtp_host: String::new(),
            smtp_port: 0,
            smtp_security: Security::None,
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

        let session = login(&account, "pw").expect("login");
        let mut statuses = StatusReader::new(session, &account, "pw");
        // `&UXZO1mWHTvZZOQ-` 是 QQ 上「其他文件夹」的 modified UTF-7 名字。
        assert_eq!(statuses.counts("&UXZO1mWHTvZZOQ-"), (3, 12));
        // 没有重连的话，这一条会读到上一条命令残留的 tagged 行并 panic。
        assert_eq!(statuses.counts("Drafts"), (3, 12));
        statuses.logout();
        let _ = server.join();
    }
}
