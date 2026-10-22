//! MIME 编解码：来信解析（mail-parser）与去信组装（lettre）。
//!
//! 解析侧只做「原文 → 模型」，组装侧只做「模型 → 原始字节」，都不碰网络。
//! 两条链路因此都能单测，网络问题与编码问题不会混在一起。

use super::model::{AttachmentMeta, MailAddress, SendRequest};
use super::sanitize;
use lettre::message::header::{ContentType as LettreContentType, InReplyTo, References};
use lettre::message::{Attachment, Mailbox, MultiPart};
use lettre::Message;
use mail_parser::{MessageParser, MimeHeaders};

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
        sanitize::ensure_viewport(&sanitize::sanitize(&quoted, block_remote_images))
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
    })
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
pub fn build(request: &SendRequest, from: &MailAddress, signature: &str) -> Result<Vec<u8>, String> {
    if request.to.is_empty() && request.cc.is_empty() && request.bcc.is_empty() {
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
        return builder
            .body(text_body)
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

    if request.attachments.is_empty() {
        return builder
            .multipart(alternative)
            .map(|m| m.formatted())
            .map_err(|e| format!("组装邮件失败: {e}"));
    }

    let mut mixed = MultiPart::mixed().multipart(alternative);
    for path in &request.attachments {
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
        mixed = mixed.singlepart(Attachment::new(filename).body(bytes, ctype));
    }

    builder
        .multipart(mixed)
        .map(|m| m.formatted())
        .map_err(|e| format!("组装邮件失败: {e}"))
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
        };
        let from = MailAddress {
            name: String::new(),
            address: "me@example.com".into(),
        };
        assert!(build(&req, &from, "").is_err());
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

}
