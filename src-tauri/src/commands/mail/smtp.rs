//! SMTP 客户端。
//!
//! 不用 lettre 的 SMTP 传输：它在内部自己建 TCP 连接，代理只能靠环境变量，
//! 而且不支持 SOCKS5。这里手写一份极小的 SMTP 会话（EHLO → STARTTLS? → AUTH → MAIL/RCPT/DATA），
//! 复用 `net::Stream`，因此 IMAP 与 SMTP 走同一套代理逻辑。lettre 仍然只用它的
//! 报文组装能力（`mime::build`），那部分与网络无关。

use super::model::{Account, ProxyConfig, Security};
use super::net::{open_stream, Stream};
use std::io::{BufRead, BufReader, Write};
use tracing::debug;

pub struct SmtpSession {
    stream: BufReader<Stream>,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub code: u16,
    pub lines: Vec<String>,
}

impl Reply {
    fn text(&self) -> String {
        self.lines.join(" | ")
    }
}

fn io_err(context: &str, e: std::io::Error) -> String {
    format!("{context}: {e}")
}

impl SmtpSession {
    /// 建连并完成 EHLO。`security` 为 starttls 时自动发 STARTTLS 再 EHLO 一次。
    pub fn connect(
        host: &str,
        port: u16,
        security: Security,
        proxy: Option<&ProxyConfig>,
        helo_name: &str,
    ) -> Result<Self, String> {
        let stream = open_stream(host, port, security, proxy)?;
        let mut session = SmtpSession {
            stream: BufReader::new(stream),
            capabilities: Vec::new(),
        };

        let greeting = session.read_reply()?;
        if greeting.code != 220 {
            return Err(format!(
                "SMTP 服务器拒绝连接（{}）: {}",
                greeting.code,
                greeting.text()
            ));
        }

        session.ehlo(helo_name)?;

        if security == Security::Starttls {
            if !session.supports("STARTTLS") {
                return Err("服务器不支持 STARTTLS，但账户配置要求加密传输".to_string());
            }
            let reply = session.command("STARTTLS")?;
            if reply.code != 220 {
                return Err(format!("STARTTLS 失败（{}）: {}", reply.code, reply.text()));
            }
            let plain = session.stream.into_inner();
            session.stream = BufReader::new(plain.upgrade_tls(host)?);
            session.ehlo(helo_name)?;
        }

        Ok(session)
    }

    fn ehlo(&mut self, helo_name: &str) -> Result<(), String> {
        let reply = self.command(&format!("EHLO {helo_name}"))?;
        if reply.code == 250 {
            self.capabilities = reply
                .lines
                .iter()
                .skip(1)
                .filter_map(|line| line.trim().split_whitespace().next())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_ascii_uppercase())
                .collect();
            return Ok(());
        }
        // 老服务器只认 HELO。
        let reply = self.command(&format!("HELO {helo_name}"))?;
        if reply.code != 250 {
            return Err(format!("EHLO 失败（{}）: {}", reply.code, reply.text()));
        }
        Ok(())
    }

    pub fn supports(&self, capability: &str) -> bool {
        let cap = capability.to_ascii_uppercase();
        self.capabilities.iter().any(|c| *c == cap)
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    /// AUTH LOGIN / PLAIN。优先 LOGIN——国内企业邮对 PLAIN 的支持参差不齐。
    pub fn authenticate(&mut self, username: &str, password: &str) -> Result<(), String> {
        let lower: Vec<String> = self
            .capabilities
            .iter()
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let has_login = lower
            .iter()
            .any(|c| c == "auth" || c.starts_with("auth=login") || c == "auth=login");
        let has_plain = lower.iter().any(|c| c.starts_with("auth=plain"));

        if has_login {
            let reply = self.command("AUTH LOGIN")?;
            if reply.code != 334 {
                return Err(format!(
                    "AUTH LOGIN 被拒绝（{}）: {}",
                    reply.code,
                    reply.text()
                ));
            }
            let reply = self.command(&base64(username.as_bytes()))?;
            if reply.code != 334 {
                return Err(format!("用户名被拒绝（{}）: {}", reply.code, reply.text()));
            }
            let reply = self.command(&base64(password.as_bytes()))?;
            if reply.code != 235 {
                return Err(format!("认证失败（{}）: {}", reply.code, reply.text()));
            }
            return Ok(());
        }

        if has_plain {
            let token = base64(format!("\0{username}\0{password}").as_bytes());
            let reply = self.command(&format!("AUTH PLAIN {token}"))?;
            if reply.code != 235 {
                return Err(format!("认证失败（{}）: {}", reply.code, reply.text()));
            }
            return Ok(());
        }

        // 服务器没声明 AUTH（少数内网中继直接放行），交给 MAIL FROM 决定成败。
        debug!("mail: SMTP 服务器未声明 AUTH 能力，跳过认证");
        Ok(())
    }

    /// 发送一封已经组装好的报文。`envelope_from` 是返回路径，
    /// `recipients` 是信封收件人（含 Bcc，与头部可见的收件人不同）。
    pub fn send(&mut self, envelope_from: &str, recipients: &[String], raw: &[u8]) -> Result<(), String> {
        if recipients.is_empty() {
            return Err("收件人为空".to_string());
        }

        let reply = self.command(&format!("MAIL FROM:<{}>", envelope_from.trim()))?;
        if reply.code != 250 {
            return Err(format!(
                "MAIL FROM 被拒绝（{}）: {}",
                reply.code,
                reply.text()
            ));
        }

        for rcpt in recipients {
            let reply = self.command(&format!("RCPT TO:<{}>", rcpt.trim()))?;
            // 251 = 用户不在本地但会转发，与 250 一样算成功。
            if reply.code != 250 && reply.code != 251 {
                return Err(format!(
                    "收件人 {rcpt} 被拒绝（{}）: {}",
                    reply.code,
                    reply.text()
                ));
            }
        }

        let reply = self.command("DATA")?;
        if reply.code != 354 {
            return Err(format!("DATA 被拒绝（{}）: {}", reply.code, reply.text()));
        }

        let payload = data_payload(raw);
        self.stream
            .get_mut()
            .write_all(&payload)
            .map_err(|e| io_err("写入报文失败", e))?;
        self.stream
            .get_mut()
            .flush()
            .map_err(|e| io_err("刷新报文失败", e))?;

        let reply = self.read_reply()?;
        if reply.code != 250 {
            return Err(format!("服务器拒收（{}）: {}", reply.code, reply.text()));
        }
        Ok(())
    }

    pub fn quit(mut self) {
        let _ = self.command("QUIT");
    }

    fn command(&mut self, line: &str) -> Result<Reply, String> {
        debug!("mail: SMTP C: {}", redact(line));
        let out = self.stream.get_mut();
        out.write_all(line.as_bytes())
            .map_err(|e| io_err("写入命令失败", e))?;
        out.write_all(b"\r\n")
            .map_err(|e| io_err("写入命令失败", e))?;
        out.flush().map_err(|e| io_err("刷新命令失败", e))?;
        self.read_reply()
    }

    fn read_reply(&mut self) -> Result<Reply, String> {
        let mut lines = Vec::new();
        let mut code = 0u16;
        loop {
            let mut line = String::new();
            let n = self
                .stream
                .read_line(&mut line)
                .map_err(|e| io_err("读取服务器响应失败", e))?;
            if n == 0 {
                return Err("SMTP 服务器关闭了连接".to_string());
            }
            let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
            debug!("mail: SMTP S: {}", trimmed);
            if trimmed.len() >= 3 {
                if let Ok(parsed) = trimmed[..3].parse::<u16>() {
                    code = parsed;
                }
            }
            // 多行响应的续行格式是 `250-...`，最后一行是 `250 ...`。
            let continuation = trimmed.as_bytes().get(3) == Some(&b'-');
            lines.push(trimmed);
            if !continuation {
                break;
            }
            if lines.len() > 200 {
                return Err("SMTP 响应行数异常".to_string());
            }
        }
        Ok(Reply { code, lines })
    }
}

/// DATA 阶段的有效载荷：dot-stuffing + 保证以 CRLF 结束 + 结束标记。
fn data_payload(raw: &[u8]) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::with_capacity(raw.len() + raw.len() / 64 + 8);
    let mut at_line_start = true;
    for &byte in raw {
        // 以 `.` 开头的行要再加一个点，否则会被当成报文结束。
        if at_line_start && byte == b'.' {
            payload.push(b'.');
        }
        payload.push(byte);
        at_line_start = byte == b'\n';
    }
    if !payload.ends_with(b"\r\n") {
        if payload.ends_with(b"\n") {
            payload.pop();
            payload.push(b'\r');
        }
        payload.extend_from_slice(b"\r\n");
    }
    payload.extend_from_slice(b".\r\n");
    payload
}

/// 日志里不打印认证 token（AUTH LOGIN 的后续行是用户名/口令的 base64）。
fn redact(line: &str) -> String {
    let looks_like_token = line.len() > 40
        && line
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
    if looks_like_token {
        "<credentials>".to_string()
    } else {
        line.to_string()
    }
}

fn base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 发送一封邮件：建连 → 认证 → 投递 → 退出。
pub fn send_raw(
    account: &Account,
    password: &str,
    proxy: Option<&ProxyConfig>,
    envelope_from: &str,
    recipients: &[String],
    raw: &[u8],
) -> Result<(), String> {
    let mut session = SmtpSession::connect(
        &account.smtp_host,
        account.smtp_port,
        account.smtp_security,
        proxy,
        &helo_name(account),
    )?;
    session.authenticate(&account.login_name(), password)?;
    session.send(envelope_from, recipients, raw)?;
    session.quit();
    Ok(())
}

/// EHLO 名：用发件域，避免某些服务器把 `localhost` 判成垃圾邮件来源。
fn helo_name(account: &Account) -> String {
    let domain = super::mime::domain_of(&account.email);
    if domain.is_empty() {
        "localhost".to_string()
    } else {
        domain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_stuffing_is_applied() {
        let payload = data_payload(b".leading dot\r\nnormal\r\n");
        assert!(payload.starts_with(b"..leading dot"));
        assert!(payload.ends_with(b"\r\n.\r\n"));
    }

    #[test]
    fn payload_terminates_with_crlf_even_without_trailing_newline() {
        let payload = data_payload(b"a\r\nb");
        assert!(payload.ends_with(b"\r\n.\r\n"));
        assert!(payload.windows(4).any(|w| w == b"\r\nb\r"));
    }

    #[test]
    fn redact_hides_long_tokens() {
        assert_eq!(redact(&"A".repeat(48)), "<credentials>");
        assert_eq!(redact("MAIL FROM:<a@b.c>"), "MAIL FROM:<a@b.c>");
    }
}
