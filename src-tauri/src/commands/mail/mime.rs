//! MIME 编解码：来信解析（mail-parser）与去信组装（lettre）。
//!
//! 解析侧只做「原文 → 模型」，组装侧只做「模型 → 原始字节」，都不碰网络。
//! 两条链路因此都能单测，网络问题与编码问题不会混在一起。

use super::model::{AttachmentMeta, MailAddress, SendRequest};
use super::sanitize;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use lettre::message::header::{ContentType as LettreContentType, InReplyTo, References};
use lettre::message::{Attachment, Mailbox, MultiPart};
use lettre::Message;
use mail_parser::{MessageParser, MimeHeaders};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::collections::HashMap;

/// 解析后的来信。正文 HTML 已经清洗过。
pub struct ParsedMessage {
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub bcc: Vec<MailAddress>,
    pub reply_to: Vec<MailAddress>,
    pub date: i64,
    pub message_id: Option<String>,
    pub in_reply_to: String,
    pub references: String,
    pub html: Option<String>,
    pub text: String,
    pub attachments: Vec<AttachmentMeta>,
    pub headers: Vec<(String, String)>,
    pub has_attachments: bool,
    /// 列表预览用：从正文抽出的纯文本。
    pub snippet_source: String,
    /// 原始报文。POP3 打开单封时没有「按 uid 取正文」的接口，`pop3_sync` 必须把
    /// 刚下载的这一份带出去；IMAP 路径不解码它，一律 `None`，避免多留一份正文副本。
    pub raw: Option<Vec<u8>>,
}

/// 线程归并键：去掉回复/转发前缀、折叠空白、转小写后的主题。
/// 只用主题是刻意的——`References` 头在邮件客户端之间经常断链，
/// 而「同一主题的来回」在绝大多数场景下就是同一会话。
pub fn thread_key(subject: &str) -> String {
    const PREFIXES: [&str; 15] = [
        "re:", "re：", "回复:", "回复：", "答复:", "答复：", "aw:", "sv:", "antw:", "fw:",
        "fwd:", "转发:", "转发：", "回覆:", "回覆：",
    ];
    let mut current = subject.trim();
    loop {
        let lower = current.to_lowercase();
        let Some(prefix) = PREFIXES.iter().find(|p| lower.starts_with(**p)) else {
            break;
        };
        // 全角/半角前缀的字节长度各不相同，按原文切片而不是按小写串。
        let rest = current[prefix.len()..].trim();
        if rest.is_empty() || rest.len() == current.len() {
            break;
        }
        current = rest;
    }
    current
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// 规范化 `Message-Id`：mail-parser 会去掉尖括号，但 `In-Reply-To` / `References`
/// 的取值按 RFC 5322 必须带尖括号，否则收信端对不上会话。
fn normalize_msg_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('<') && trimmed.ends_with('>') {
        return Some(trimmed.to_string());
    }
    Some(format!("<{trimmed}>"))
}

fn parse_one(addr: &mail_parser::Addr<'_>) -> MailAddress {
    MailAddress {
        name: addr.name().unwrap_or("").trim().to_string(),
        address: addr.address().unwrap_or("").trim().to_string(),
    }
}

fn parse_addrs(addr: Option<&mail_parser::Address<'_>>) -> Vec<MailAddress> {
    addr.map(|a| a.iter().map(parse_one).collect())
        .unwrap_or_default()
}

/// mail-parser 的 DateTime → Unix 秒。时区按头部声明的偏移换算。
fn to_unix(dt: &mail_parser::DateTime) -> i64 {
    let offset_minutes = (dt.tz_hour as i32) * 60 + (dt.tz_minute as i32);
    let sign = if dt.tz_before_gmt { -1 } else { 1 };
    let naive = chrono::NaiveDate::from_ymd_opt(dt.year as i32, dt.month as u32, dt.day as u32)
        .and_then(|d| d.and_hms_opt(dt.hour as u32, dt.minute as u32, dt.second as u32));
    match naive {
        Some(n) => {
            let utc = n - chrono::Duration::minutes((sign * offset_minutes) as i64);
            utc.and_utc().timestamp()
        }
        None => 0,
    }
}

/// 解析一封完整的 RFC822 报文。`block_remote_images` 控制远端图片是否在清洗时被挡住。
/// 收集内联图片：Content-ID → data URI。
///
/// 只收 `image/*`，单张上限 2MB、总计 8MB——内联图通常是签名 logo 或小图标，
/// 超过这个量级的多半是把整张海报塞进了正文，放进 DOM 会让滚动明显变卡。
fn collect_inline_images(msg: &mail_parser::Message<'_>) -> HashMap<String, String> {
    let mut images = HashMap::new();
    let mut total = 0usize;
    for part in msg.attachments() {
        let Some(raw_cid) = part.content_id() else {
            continue;
        };
        let cid = normalize_cid(raw_cid);
        if cid.is_empty() || images.contains_key(&cid) {
            continue;
        }
        let Some(ctype) = part.content_type() else {
            continue;
        };
        let Some(subtype) = ctype.subtype() else {
            continue;
        };
        if ctype.ctype() != "image" {
            continue;
        }
        let body: &[u8] = match &part.body {
            mail_parser::PartType::Binary(bytes)
            | mail_parser::PartType::InlineBinary(bytes) => bytes.as_ref(),
            _ => continue,
        };
        if body.is_empty()
            || body.len() > 2 * 1024 * 1024
            || total + body.len() > 8 * 1024 * 1024
        {
            continue;
        }
        total += body.len();
        images.insert(
            cid,
            format!("data:image/{subtype};base64,{}", STANDARD.encode(body)),
        );
    }
    images
}

fn normalize_cid(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('<').trim_end_matches('>');
    let decoded = urlencoding::decode(trimmed)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| trimmed.to_string());
    decoded.trim().to_ascii_lowercase()
}

/// 把 HTML 里的 `cid:xxx` 引用换成内联图片的 data URI。
fn replace_cid_sources(html: &str, images: &HashMap<String, String>) -> String {
    if images.is_empty() || !html.to_ascii_lowercase().contains("cid:") {
        return html.to_string();
    }
    static CID_SRC: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(?i)src\s*=\s*"cid:([^"]+)""#).unwrap());
    static CID_URL: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(?i)url\(\s*['"]?cid:([^'")]+)['"]?\s*\)"#).unwrap());
    let replaced = CID_SRC.replace_all(html, |caps: &Captures| {
        match images.get(&normalize_cid(&caps[1])) {
            Some(data) => format!("src=\"{data}\""),
            None => caps[0].to_string(),
        }
    });
    CID_URL
        .replace_all(&replaced, |caps: &Captures| {
            match images.get(&normalize_cid(&caps[1])) {
                Some(data) => format!("url(\"{data}\")"),
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

pub fn parse(raw: &[u8], block_remote_images: bool) -> Option<ParsedMessage> {
    let parser = MessageParser::default()
        .with_minimal_headers()
        .with_message_ids();
    let msg = parser.parse(raw)?;

    let subject = msg.subject().unwrap_or("").trim().to_string();
    let (from_name, from_addr) = match msg.from().and_then(|a| a.first()) {
        Some(addr) => (
            addr.name().unwrap_or("").trim().to_string(),
            addr.address().unwrap_or("").trim().to_string(),
        ),
        None => (String::new(), String::new()),
    };

    let date = msg.date().map(to_unix).unwrap_or(0);

    // 正文：优先取最后一个 text/html（multipart/alternative 的后者是富文本版本）。
    let raw_html: Option<String> = if msg.html_body_count() > 0 {
        msg.body_html(msg.html_body_count() - 1)
            .map(|c| c.into_owned())
    } else {
        None
    };
    let text: String = if msg.text_body_count() > 0 {
        msg.body_text(msg.text_body_count() - 1)
            .map(|c| c.into_owned())
            .unwrap_or_default()
    } else {
        raw_html
            .as_deref()
            .map(sanitize::html_to_text)
            .unwrap_or_default()
    };

    let html = raw_html.as_deref().map(|html| {
        let quoted = sanitize::mark_quotes(html);
        let sanitized = sanitize::ensure_viewport(&sanitize::sanitize(&quoted, block_remote_images));
        // `cid:` 是邮件协议里的内联图引用，WebView 不认识这个 scheme，必须换成
        // data URI 才能真正显示出来（否则控制台报 ERR_UNKNOWN_URL_SCHEME）。
        replace_cid_sources(&sanitized, &collect_inline_images(&msg))
    });

    let mut attachments: Vec<AttachmentMeta> = Vec::new();
    for (idx, part) in msg.attachments().enumerate() {
        let filename = part
            .attachment_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("附件-{}", idx + 1));
        let content_type = part
            .content_type()
            .map(|ct| match ct.subtype() {
                Some(sub) => format!("{}/{}", ct.ctype(), sub),
                None => ct.ctype().to_string(),
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());
        attachments.push(AttachmentMeta {
            // 用部件正文字节偏移做 id：下载附件时按它取原始字节。
            part: part.offset_body,
            filename,
            content_type,
            size: part.body.len() as u32,
            content_id: part.content_id().map(|s| s.to_string()),
        });
    }

    let has_attachments = !attachments.is_empty();

    let headers: Vec<(String, String)> = msg
        .headers()
        .iter()
        .take(40)
        .map(|h| {
            (
                h.name().to_string(),
                h.value().as_text().unwrap_or("").to_string(),
            )
        })
        .collect();

    let snippet_source = if !text.trim().is_empty() {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        html.as_deref()
            .map(sanitize::html_to_text)
            .unwrap_or_default()
    };

    Some(ParsedMessage {
        subject,
        from_name,
        from_addr,
        to: parse_addrs(msg.to()),
        cc: parse_addrs(msg.cc()),
        bcc: parse_addrs(msg.bcc()),
        reply_to: parse_addrs(msg.reply_to()),
        date,
        message_id: msg.message_id().and_then(normalize_msg_id),
        in_reply_to: msg
            .in_reply_to()
            .as_text()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        references: msg
            .references()
            .as_text()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        html,
        text,
        attachments,
        headers,
        has_attachments,
        snippet_source,
        // 默认不带原文：只有 POP3 的调用方会自己补上（见 pop3_sync::fetch_message）。
        raw: None,
    })
}

/// 报文顶层 `Content-Type`（只看头部，不做完整解析）。
///
/// 用于在解析前判断「这是不是 S/MIME / PGP 加密封装」——完整解析一个加密报文
/// 只会得到一堆 `application/octet-stream`，先认出类型才能走解密分支。
pub fn top_level_content_type(raw: &[u8]) -> Option<String> {
    let head_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n"))?;
    let head = String::from_utf8_lossy(&raw[..head_end]);
    // 头部可能折行（RFC 5322 允许续行以空白开头），先拼回去。
    let mut unfolded = String::with_capacity(head.len());
    for line in head.lines() {
        if line.starts_with([' ', '\t']) {
            unfolded.push(' ');
            unfolded.push_str(line.trim_start());
        } else {
            unfolded.push('\n');
            unfolded.push_str(line);
        }
    }
    for line in unfolded.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-type:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// 取指定部件的原始字节（附件下载）。`part_offset` 是 `AttachmentMeta.part`。
pub fn extract_part(raw: &[u8], part_offset: u32) -> Option<(Vec<u8>, String)> {
    let parser = MessageParser::default().with_minimal_headers();
    let msg = parser.parse(raw)?;
    for part in msg.parts.iter() {
        if part.offset_body == part_offset {
            let name = part.attachment_name().unwrap_or("").to_string();
            let bytes = match &part.body {
                mail_parser::PartType::Text(s) => s.as_bytes().to_vec(),
                mail_parser::PartType::Html(s) => s.as_bytes().to_vec(),
                mail_parser::PartType::Binary(b) => b.to_vec(),
                mail_parser::PartType::InlineBinary(b) => b.to_vec(),
                _ => Vec::new(),
            };
            return Some((bytes, name));
        }
    }
    None
}

/// 把 `"Name <a@b.c>"` 或 `a@b.c` 解析成 lettre 的 Mailbox。
pub fn parse_mailbox(raw: &str) -> Result<Mailbox, String> {
    raw.trim()
        .parse::<Mailbox>()
        .map_err(|e| format!("地址无效（{raw}）: {e}"))
}

/// 组装去信。返回可以同时喂给 SMTP 与「保存到已发送」的原始字节。
///
/// 结构按 RFC 2046 选最合适的一种：
/// - 没有附件、没有内联图：`multipart/alternative`（纯文本 + HTML）；
/// - 有内联图：HTML 与图片一起进 `multipart/related`（`cid:` 引用），
///   这样收信端把图片当作正文的一部分显示，而不是一堆乱七八糟的附件；
/// - 有普通附件：最外层 `multipart/mixed` 包住上面那个。
pub fn build(request: &SendRequest, from: &MailAddress, signature: &str) -> Result<Vec<u8>, String> {
    build_inner(request, from, signature, true)
}

/// 组装**草稿**。
///
/// 草稿允许一个收件人都没有：用户经常先写正文、回头再想发给谁。
/// 以前存草稿也走 `build`，于是「收件人为空」直接把自动保存打回，
/// 用户看到的现象就是「关掉编辑器，草稿箱里什么都没有」。
/// 正式发送仍然必须至少有一个收件人。
pub fn build_draft(
    request: &SendRequest,
    from: &MailAddress,
    signature: &str,
) -> Result<Vec<u8>, String> {
    build_inner(request, from, signature, false)
}

fn build_inner(
    request: &SendRequest,
    from: &MailAddress,
    signature: &str,
    require_recipients: bool,
) -> Result<Vec<u8>, String> {
    if require_recipients && request.to.is_empty() && request.cc.is_empty() && request.bcc.is_empty()
    {
        return Err("收件人为空".to_string());
    }
    let from_box = if from.name.trim().is_empty() {
        parse_mailbox(&from.address)?
    } else {
        parse_mailbox(&format!(
            "\"{}\" <{}>",
            from.name.replace('"', ""),
            from.address
        ))?
    };

    let mut builder = Message::builder().from(from_box).subject(&request.subject);
    for addr in &request.to {
        builder = builder.to(parse_mailbox(addr)?);
    }
    for addr in &request.cc {
        builder = builder.cc(parse_mailbox(addr)?);
    }
    for addr in &request.bcc {
        builder = builder.bcc(parse_mailbox(addr)?);
    }
    if let Some(id) = request.in_reply_to.as_ref().filter(|s| !s.trim().is_empty()) {
        builder = builder.header(InReplyTo::from(id.trim().to_string()));
    }
    if let Some(refs) = request.references.as_ref().filter(|s| !s.trim().is_empty()) {
        builder = builder.header(References::from(refs.trim().to_string()));
    }

    let text_body = if signature.trim().is_empty() {
        request.text.clone()
    } else {
        format!("{}\n\n-- \n{}", request.text, signature)
    };

    let Some(html) = request.html.as_deref().filter(|h| !h.trim().is_empty()) else {
        // 纯文本邮件：有附件时也要包成 multipart/mixed，否则附件无处安放。
        if request.attachments.is_empty() {
            return builder
                .body(text_body)
                .map(|m| m.formatted())
                .map_err(|e| format!("组装邮件失败: {e}"));
        }
        let mut mixed = MultiPart::mixed().singlepart(
            lettre::message::SinglePart::plain(text_body),
        );
        for attachment in build_attachments(&request.attachments)? {
            mixed = mixed.singlepart(attachment);
        }
        return builder
            .multipart(mixed)
            .map(|m| m.formatted())
            .map_err(|e| format!("组装邮件失败: {e}"));
    };

    let html_with_sig = if signature.trim().is_empty() {
        html.to_string()
    } else {
        format!(
            "{html}<br><br><div class=\"guaxi-signature\">{}</div>",
            html_escape(signature)
        )
    };
    let alternative = MultiPart::alternative_plain_html(text_body, html_with_sig);

    // 内联图：`multipart/related` 的第一个部件是 HTML，其余是按 Content-ID 引用的图片。
    // 只有真的存在内联图时才包 related —— 否则会多出一层没有必要的
    // multipart/related，部分老客户端会因此把正文显示成空的。
    let has_inline = !request.inline_images.is_empty();
    let has_attachments = !request.attachments.is_empty();
    let payload = if has_inline {
        let mut related = MultiPart::related().multipart(alternative);
        for inline in &request.inline_images {
            let ctype = mime_guess::from_path(&inline.path)
                .first_or_octet_stream()
                .to_string();
            let ctype = LettreContentType::parse(&ctype)
                .map_err(|e| format!("内联图片类型无效（{}）: {e}", inline.path))?;
            related = related.singlepart(build_inline_image(inline, ctype)?);
        }
        related
    } else {
        alternative
    };

    if !has_attachments {
        return builder
            .multipart(payload)
            .map(|m| m.formatted())
            .map_err(|e| format!("组装邮件失败: {e}"));
    }

    let mut mixed = MultiPart::mixed().multipart(payload);
    for attachment in build_attachments(&request.attachments)? {
        mixed = mixed.singlepart(attachment);
    }

    builder
        .multipart(mixed)
        .map(|m| m.formatted())
        .map_err(|e| format!("组装邮件失败: {e}"))
}

/// 构造一条普通附件。读取失败时给出「哪个文件」而不是一个裸 IO 错误。
fn build_attachments(paths: &[String]) -> Result<Vec<lettre::message::SinglePart>, String> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = std::fs::read(path).map_err(|e| format!("读取附件失败（{path}）: {e}"))?;
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("attachment")
            .to_string();
        let guessed = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        let ctype = LettreContentType::parse(&guessed)
            .map_err(|e| format!("附件类型无效（{guessed}）: {e}"))?;
        out.push(Attachment::new(filename).body(bytes, ctype));
    }
    Ok(out)
}

/// 内联图片部件：必须带 `Content-ID: <cid>` 与 `Content-Disposition: inline`，
/// 否则收信端（尤其是 Outlook）会把它折叠成附件、正文里留一个破图。
fn build_inline_image(
    inline: &super::model::InlineImage,
    ctype: LettreContentType,
) -> Result<lettre::message::SinglePart, String> {
    let bytes = std::fs::read(&inline.path)
        .map_err(|e| format!("读取内联图片失败（{}）: {e}", inline.path))?;
    let cid = inline
        .content_id
        .trim()
        .trim_matches(['<', '>'])
        .to_string();
    let filename = if inline.filename.trim().is_empty() {
        std::path::Path::new(&inline.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("image")
            .to_string()
    } else {
        inline.filename.clone()
    };
    Ok(Attachment::new_inline_with_name(cid, filename).body(bytes, ctype))
}

/// 附件体积检查：给前端在发送前提示（超限、危险类型）。
///
/// 返回的 `level` 只有三档，刻意做得粗：`dangerous` 是「双击可能执行」的类型，
/// `large` 是超过配置上限，`ok` 是其余。这里的判定只用于**提示**，
/// 真正拦截交给用户确认——企业里把 .exe 当附件发给同事是合法需求。
pub fn check_attachments(paths: &[String], limit_mb: u32) -> Vec<super::model::AttachmentCheck> {
    const DANGEROUS: &[&str] = &[
        "exe", "scr", "com", "bat", "cmd", "pif", "msi", "vbs", "vbe", "js", "jse", "wsf",
        "wsh", "ps1", "psm1", "hta", "cpl", "reg", "lnk", "jar", "application",
    ];
    let limit = (limit_mb.max(1) as u64) * 1024 * 1024;
    paths
        .iter()
        .map(|path| {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_string();
            let metadata = std::fs::metadata(path);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let missing = metadata.is_err();
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let (level, detail) = if DANGEROUS.contains(&ext.as_str()) {
                (
                    "dangerous",
                    format!(".{ext} 是可执行/脚本类型：收件人双击就可能运行"),
                )
            } else if size > limit {
                (
                    "large",
                    format!(
                        "{} MB，超过 {} MB 上限；多数邮件服务器会拒收",
                        (size as f64 / 1048576.0).round() as u64,
                        limit_mb
                    ),
                )
            } else if missing {
                // 附件路径失效（被移动 / 删除）：发送时 `mime::build` 会报错，
                // 但这里先提示，用户不用等到点「发送」才发现。
                ("large", "文件不存在或无法读取，发送时会失败".to_string())
            } else {
                ("ok", String::new())
            };
            super::model::AttachmentCheck {
                path: path.clone(),
                name,
                size,
                level: level.to_string(),
                detail,
            }
        })
        .collect()
}

/// 附件总大小（字节）。
pub fn attachments_size(paths: &[String]) -> u64 {
    paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', "<br>")
}

/// 转发的主题前缀。规则里的「转发」动作走这里拼主题。
pub fn forward_subject(subject: &str) -> String {
    let text = String::from(subject).trim().to_string();
    let lower = text.to_lowercase();
    if lower.starts_with("fw:") || lower.starts_with("fwd:") || lower.starts_with("转发") {
        return text;
    }
    format!("Fwd: {text}")
}

/// `a@b.c` → `b.c`，用于卡片展示与 EHLO 名。
pub fn domain_of(addr: &str) -> String {
    addr.rsplit_once('@')
        .map(|(_, domain)| domain.to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_key_strips_prefixes_case_insensitively() {
        assert_eq!(thread_key("Re: 季度报告"), thread_key("季度报告"));
        assert_eq!(thread_key("RE: Re:   quarterly report "), "quarterly report");
        assert_eq!(thread_key("回复：甲方确认"), "甲方确认");
    }


    #[test]
    fn parse_mailbox_accepts_bare_and_named() {
        assert!(parse_mailbox("a@b.c").is_ok());
        assert!(parse_mailbox("张三 <a@b.c>").is_ok());
        assert!(parse_mailbox("not-an-email").is_err());
    }

    #[test]
    fn domain_of_handles_missing_at() {
        assert_eq!(domain_of("x@Example.COM"), "example.com");
        assert_eq!(domain_of("nope"), "");
    }

    #[test]
    fn build_produces_rfc822_with_headers() {
        let req = SendRequest {
            account_id: "a".into(),
            to: vec!["b@c.d".into()],
            cc: vec![],
            bcc: vec![],
            subject: "你好".into(),
            text: "正文".into(),
            html: Some("<p>正文</p>".into()),
            in_reply_to: None,
            references: None,
            attachments: vec![],
            save_to_sent: None,
            inline_images: Vec::new(),
            crypto_mode: String::new(),
        };
        let from = MailAddress {
            name: "我".into(),
            address: "me@example.com".into(),
        };
        let raw = build(&req, &from, "").unwrap();
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("From:"));
        assert!(text.contains("To:"));
        assert!(text.to_lowercase().contains("multipart/alternative"));
    }

    #[test]
    fn build_rejects_empty_recipients() {
        let req = SendRequest {
            account_id: "a".into(),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: "s".into(),
            text: "t".into(),
            html: None,
            in_reply_to: None,
            references: None,
            attachments: vec![],
            save_to_sent: None,
            inline_images: Vec::new(),
            crypto_mode: String::new(),
        };
        let from = MailAddress {
            name: String::new(),
            address: "me@example.com".into(),
        };
        assert!(build(&req, &from, "").is_err());
    }

    #[test]
    fn top_level_content_type_reads_header() {
        let raw = b"From: a@b.c
Content-Type: application/pkcs7-mime;
 smime-type=enveloped-data

body";
        let ct = top_level_content_type(raw).unwrap();
        // 折行要拼回去，否则会只拿到一半的参数。
        assert!(ct.contains("application/pkcs7-mime"));
        assert!(ct.contains("smime-type=enveloped-data"));
    }

    #[test]
    fn parse_handles_simple_message_and_timezone() {
        // 2024-01-01 10:00:00 +0800 == 2024-01-01 02:00:00 UTC
        let raw = b"From: Alice <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: Hello\r\n\
Date: Mon, 1 Jan 2024 10:00:00 +0800\r\n\
Message-ID: <abc@example.com>\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Hello world\r\n";
        let parsed = parse(raw, false).unwrap();
        assert_eq!(parsed.subject, "Hello");
        assert_eq!(parsed.from_addr, "alice@example.com");
        assert_eq!(parsed.from_name, "Alice");
        assert!(parsed.text.contains("Hello world"));
        assert_eq!(parsed.date, 1704074400);
        // mail-parser 会去掉尖括号，解析层负责补回来。
        assert_eq!(parsed.message_id.as_deref(), Some("<abc@example.com>"));
    }

    #[test]
    fn cid_sources_become_data_uris() {
        let mut images = HashMap::new();
        images.insert(
            "logo@x".to_string(),
            "data:image/png;base64,AAAA".to_string(),
        );
        let html = r#"<img src="cid:LOGO@x"><img src="cid:logo%40x"><img src="cid:missing@x">"#;
        let out = replace_cid_sources(html, &images);
        assert_eq!(out.matches("data:image/png;base64,AAAA").count(), 2);
        assert!(out.contains(r#"src="cid:missing@x""#));
    }

    #[test]
    fn parse_embeds_inline_image_from_multipart_related() {
        let raw = b"From: a@example.com\r\n\
To: b@example.com\r\n\
Subject: inline\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/related; boundary=\"b1\"\r\n\
\r\n\
--b1\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-ID: <html@x>\r\n\
\r\n\
<html><body><img src=\"cid:img1@x\"></body></html>\r\n\
--b1\r\n\
Content-Type: image/png\r\n\
Content-ID: <img1@x>\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-Disposition: inline; filename=\"logo.png\"\r\n\
\r\n\
iVBORw0KGgo=\r\n\
--b1--\r\n";
        let parsed = parse(raw, false).unwrap();
        let html = parsed.html.expect("html");
        assert!(
            html.contains(r#"src="data:image/png;base64,iVBORw0KGgo=""#),
            "内联图应替换为 data URI: {html}"
        );
        assert!(!html.contains("cid:img1@x"));
    }
}
