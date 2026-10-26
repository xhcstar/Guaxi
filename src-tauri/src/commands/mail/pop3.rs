//! POP3 客户端（RFC 1939 / 2595 / 2449）。
//!
//! 为什么要自己写：市面上现成的 Rust POP3 库（`pop3` crate）把 OpenSSL 写死在依赖里，
//! 而这台 Windows 构建机没有 NASM、也不该依赖用户机器上的 OpenSSL。项目里的 SMTP 同样
//! 是手写在 `net::Stream` 上的——POP3 沿用同一套思路，代理、TLS 根证书、超时全都复用，
//! 三种加密方式（SSL / STARTTLS / 明文）也就天然一致。
//!
//! POP3 的能力比 IMAP 弱得多，这一点直接影响数据模型：
//! - 只有**一个**邮箱（INBOX），没有文件夹，也没有服务器端的「已读 / 星标」；
//! - 邮件的身份是「本次会话里的序号」，换个会话就可能变，所以本地必须用服务器返回的
//!   `UIDL` 做稳定主键；
//! - 不声明 `UIDL` 的服务器只能退化成按序号比对（少见，但 Dovecot / Exchange 早期版本有）。
//!
//! 因此这里把 POP3 账户当作「只有收件箱 + 发件靠 SMTP」的账户接进现有模型：
//! 文件夹表里仍然是一条 `INBOX`，`uid` 用 `UIDL` 的稳定哈希，标记类操作全部走本地。

use super::model::{Account, ProxyConfig, Security};
use super::net::{open_stream, Stream};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use tracing::debug;

/// POP3 下载整封邮件时可能很慢（老服务器逐封串行），给一个宽松的读超时。
const READ_TIMEOUT: Duration = Duration::from_secs(120);

pub struct Pop3Session {
    /// `Option` 只为 STARTTLS 服务：升级要拿走流的所有权，升级完再放回。
    stream: Option<BufReader<Stream>>,
}

#[derive(Debug, Clone)]
pub struct Pop3Reply {
    /// `+OK` 为 true，`-ERR` 为 false。
    pub ok: bool,
    pub text: String,
}

/// POP3 邮件索引：`number` 是本次会话里的序号，`uidl` 是跨会话稳定的标识。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pop3Entry {
    pub number: u32,
    pub uidl: String,
    pub size: u32,
}

fn io_err(context: &str, e: std::io::Error) -> String {
    format!("{context}: {e}")
}

impl Pop3Session {
    /// 建连、按需 STARTTLS、登录。
    ///
    /// `security` 的语义与 IMAP / SMTP 完全一致：`ssl` 建连即 TLS（995），
    /// `starttls` 先明文再升级（110 + STLS），`none` 明文。
    pub fn connect(
        account: &Account,
        password: &str,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Self, String> {
        let host = account.pop3_host.trim();
        if host.is_empty() {
            return Err("这个账户还没有填写 POP3 服务器".to_string());
        }
        let mut stream = open_stream(host, account.pop3_port, account.pop3_security, proxy)?;
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .map_err(|_| "无法设置 POP3 读超时".to_string())
            .ok();
        let mut session = Pop3Session {
            stream: Some(BufReader::new(stream)),
        };

        let greeting = session.read_reply()?;
        if !greeting.ok {
            return Err(format!("POP3 服务器拒绝连接: {}", greeting.text));
        }
        // 问候语里的 `<...>` 是 APOP 的时间戳；我们不用 APOP（多数服务器已不支持，
        // 且需要明文存储的摘要），只把它留在日志里便于排障。
        debug!(greeting = %greeting.text, "mail: POP3 已连接");

        if account.pop3_security == Security::Starttls {
            session.starttls(host)?;
        }

        session.authenticate(account, password)?;
        Ok(session)
    }

    fn starttls(&mut self, host: &str) -> Result<(), String> {
        let reply = self.command("STLS")?;
        if !reply.ok {
            return Err(format!(
                "服务器不支持 STARTTLS（{}）。请在账户设置里改用「SSL/TLS」或「不加密」",
                reply.text
            ));
        }
        // 升级要拿走流的所有权。BufReader 必须丢掉：它可能已经把 TLS 握手字节读进缓冲区。
        let reader = self
            .stream
            .take()
            .ok_or_else(|| "POP3 连接已关闭".to_string())?;
        let tls = reader.into_inner().upgrade_tls(host)?;
        self.stream = Some(BufReader::new(tls));
        Ok(())
    }

    fn io(&mut self) -> Result<&mut BufReader<Stream>, String> {
        self.stream
            .as_mut()
            .ok_or_else(|| "POP3 连接已关闭".to_string())
    }

    fn authenticate(&mut self, account: &Account, password: &str) -> Result<(), String> {
        let user = account.login_name();
        // USER/PASS 是唯一所有 POP3 服务器都支持的方式；APOP / SASL 只在少数服务器上有，
        // 且 Gmail / QQ / 163 的「应用专用密码」同样走 USER/PASS。OAuth2 账户不适用 POP3。
        let reply = self.command(&format!("USER {user}"))?;
        if !reply.ok {
            return Err(format!("POP3 用户名被拒绝（{user}）: {}", reply.text));
        }
        let reply = self.command(&format!("PASS {password}"))?;
        if !reply.ok {
            return Err(format!(
                "POP3 登录失败（{user}）: {}。多数国内邮箱需要先在网页端开启 POP3 并使用授权码，而不是登录密码",
                reply.text
            ));
        }
        Ok(())
    }

    /// `STAT`：返回邮件总数与总字节数。
    pub fn stat(&mut self) -> Result<(u32, u64), String> {
        let reply = self.command("STAT")?;
        if !reply.ok {
            return Err(format!("读取邮箱状态失败: {}", reply.text));
        }
        let mut parts = reply.text.split_whitespace();
        let count = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let size = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        Ok((count, size))
    }

    /// `UIDL`：一次拿全部邮件的稳定标识。不支持 `UIDL` 的服务器返回空列表，
    /// 调用方据此退回按序号比对。
    pub fn uidl(&mut self) -> Result<Vec<Pop3Entry>, String> {
        // 这是**多行响应**：`+OK` 之后每封一行，直到单独的 `.`。用单行读取只会拿到
        // 状态行，后面的正文会被当成下一条命令的响应，整条会话随之错位。
        let reply = self.command_multiline("UIDL")?;
        if !reply.ok {
            debug!(reply = %reply.text, "mail: 服务器不支持 UIDL");
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for line in reply.lines {
            let mut parts = line.split_whitespace();
            let number: u32 = match parts.next().and_then(|s| s.parse().ok()) {
                Some(n) => n,
                None => continue,
            };
            let uidl = parts.next().unwrap_or("").trim().to_string();
            if uidl.is_empty() {
                continue;
            }
            entries.push(Pop3Entry {
                number,
                uidl,
                size: 0,
            });
        }
        Ok(entries)
    }

    /// `LIST`：拿到每封邮件的大小（用于列表展示与「别一次拉太大的信」的取舍）。
    /// 与 `UIDL` 一样是多行响应。
    pub fn list(&mut self) -> Result<Vec<Pop3Entry>, String> {
        let reply = self.command_multiline("LIST")?;
        if !reply.ok {
            return Err(format!("读取邮件列表失败: {}", reply.text));
        }
        let mut entries = Vec::new();
        for line in reply.lines {
            let mut parts = line.split_whitespace();
            let number: u32 = match parts.next().and_then(|s| s.parse().ok()) {
                Some(n) => n,
                None => continue,
            };
            let size: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            entries.push(Pop3Entry {
                number,
                uidl: String::new(),
                size,
            });
        }
        Ok(entries)
    }

    /// `RETR`：取整封原始报文（含邮件头与全部附件）。
    pub fn retr(&mut self, number: u32) -> Result<Vec<u8>, String> {
        let reply = self.command_multiline(&format!("RETR {number}"))?;
        if !reply.ok {
            return Err(format!("下载第 {number} 封邮件失败: {}", reply.text));
        }
        // 点对点搬运：正文按字节保存，不能走 String，否则二进制附件会被破坏。
        Ok(reply.bytes)
    }

    /// `DELE`：在本次会话里标记删除，`QUIT` 时才真正落盘。
    pub fn delete(&mut self, number: u32) -> Result<(), String> {
        let reply = self.command(&format!("DELE {number}"))?;
        if !reply.ok {
            return Err(format!("删除第 {number} 封邮件失败: {}", reply.text));
        }
        Ok(())
    }

    pub fn quit(mut self) {
        // QUIT 才会提交 DELE；连接已断时忽略错误。
        let _ = self.command("QUIT");
    }

    /// 不提删除地断开：抓信中途出错时用 `RSET` 取消已标记的删除，再退出。
    pub fn abort(mut self) {
        let _ = self.command("RSET");
        let _ = self.command("QUIT");
    }

    fn command(&mut self, line: &str) -> Result<Pop3Reply, String> {
        debug!("mail: POP3 C: {}", redact(line));
        {
            let out = self.io()?.get_mut();
            out.write_all(line.as_bytes())
                .map_err(|e| io_err("写入 POP3 命令失败", e))?;
            out.write_all(b"\r\n")
                .map_err(|e| io_err("写入 POP3 命令失败", e))?;
            out.flush()
                .map_err(|e| io_err("刷新 POP3 命令失败", e))?;
        }
        let reply = self.read_reply()?;
        debug!(ok = reply.ok, text = %reply.text, "mail: POP3 S");
        Ok(reply)
    }

    fn read_reply(&mut self) -> Result<Pop3Reply, String> {
        let mut line = String::new();
        let read = self
            .io()?
            .read_line(&mut line)
            .map_err(|e| io_err("读取 POP3 响应失败", e))?;
        if read == 0 {
            return Err("POP3 服务器关闭了连接".to_string());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
        if let Some(rest) = trimmed.strip_prefix("+OK") {
            return Ok(Pop3Reply {
                ok: true,
                text: rest.trim().to_string(),
            });
        }
        if let Some(rest) = trimmed.strip_prefix("-ERR") {
            return Ok(Pop3Reply {
                ok: false,
                text: rest.trim().to_string(),
            });
        }
        // 个别服务器会用 + 或 - 开头的非标准状态行；按失败处理并原样带回。
        Ok(Pop3Reply {
            ok: trimmed.starts_with('+'),
            text: trimmed,
        })
    }

    /// 多行响应：读到单独一行的 `.` 为止。行首的 `..` 是转义，要还原成 `.`。
    fn command_multiline(&mut self, line: &str) -> Result<MultilineReply, String> {
        debug!("mail: POP3 C: {}", redact(line));
        {
            let out = self.io()?.get_mut();
            out.write_all(line.as_bytes())
                .map_err(|e| io_err("写入 POP3 命令失败", e))?;
            out.write_all(b"\r\n")
                .map_err(|e| io_err("写入 POP3 命令失败", e))?;
            out.flush().map_err(|e| io_err("刷新 POP3 命令失败", e))?;
        }
        let reply = self.read_reply()?;
        if !reply.ok {
            return Ok(MultilineReply {
                ok: false,
                text: reply.text,
                lines: Vec::new(),
                bytes: Vec::new(),
            });
        }
        let mut lines: Vec<String> = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        loop {
            let mut raw = Vec::new();
            let read = self
                .io()?
                .read_until(b'\n', &mut raw)
                .map_err(|e| io_err("读取 POP3 正文失败", e))?;
            if read == 0 {
                return Err("POP3 连接在正文传输中中断".to_string());
            }
            // 去掉行尾 CRLF，保留行内所有字节。
            while matches!(raw.last(), Some(b'\n') | Some(b'\r')) {
                raw.pop();
            }
            if raw == b"." {
                break;
            }
            // 点填充（dot-stuffing）还原：行首 ``..`` 变回 ``.``。
            if raw.first() == Some(&b'.') {
                raw.remove(0);
            }
            lines.push(String::from_utf8_lossy(&raw).to_string());
            if !bytes.is_empty() {
                bytes.extend_from_slice(b"\r\n");
            }
            bytes.extend_from_slice(&raw);
        }
        Ok(MultilineReply {
            ok: true,
            text: reply.text,
            lines,
            bytes,
        })
    }
}

struct MultilineReply {
    ok: bool,
    text: String,
    lines: Vec<String>,
    bytes: Vec<u8>,
}

/// 日志里不出现口令。
fn redact(line: &str) -> String {
    if line.len() > 5 && line[..4].eq_ignore_ascii_case("PASS") {
        return format!("{} ******", &line[..4]);
    }
    line.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::mail::model::{Account, Security};
    use std::io::{BufRead, BufReader, Write as IoWrite};
    use std::net::TcpListener;

    fn account(port: u16) -> Account {
        Account {
            id: "pop-probe".into(),
            email: "me@example.com".into(),
            display_name: String::new(),
            protocol: "pop3".into(),
            imap_host: String::new(),
            imap_port: 0,
            imap_security: Security::None,
            pop3_host: "127.0.0.1".into(),
            pop3_port: port,
            pop3_security: Security::None,
            pop3_leave_on_server: true,
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
        }
    }

    /// 一个够用的假 POP3 服务器：USER/PASS、STAT、UIDL、LIST、RETR、DELE、QUIT。
    fn serve_once(listener: TcpListener, raw: Vec<u8>) {
        let (socket, _) = listener.accept().expect("accept");
        let mut out = socket.try_clone().expect("clone");
        let mut reader = BufReader::new(socket);
        out.write_all(b"+OK fake pop3 ready\r\n").ok();
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let upper = line.trim().to_ascii_uppercase();
            if upper.starts_with("USER") || upper.starts_with("PASS") || upper == "RSET" {
                out.write_all(b"+OK\r\n").ok();
            } else if upper == "STAT" {
                out.write_all(b"+OK 2 900\r\n").ok();
            } else if upper == "UIDL" {
                out.write_all(b"+OK unique-id listing follows\r\n1 abc\r\n2 def\r\n.\r\n").ok();
            } else if upper == "LIST" {
                out.write_all(b"+OK 2 messages\r\n1 400\r\n2 500\r\n.\r\n").ok();
            } else if upper.starts_with("RETR") {
                // 正文里有以 `.` 开头的行，验证点填充还原。
                out.write_all(b"+OK 500 octets\r\n").ok();
                out.write_all(&raw).ok();
                out.write_all(b"\r\n.\r\n").ok();
            } else if upper.starts_with("DELE") {
                out.write_all(b"+OK marked\r\n").ok();
            } else if upper == "QUIT" {
                out.write_all(b"+OK bye\r\n").ok();
                break;
            } else {
                out.write_all(b"-ERR unknown\r\n").ok();
            }
        }
    }

    #[test]
    fn uidl_and_list_are_multiline() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || serve_once(listener, b"hello".to_vec()));

        let mut session = Pop3Session::connect(&account(port), "pw", None).expect("connect");
        let uids = session.uidl().expect("uidl");
        assert_eq!(uids.len(), 2);
        assert_eq!(uids[0].uidl, "abc");
        assert_eq!(uids[1].number, 2);

        let sizes = session.list().expect("list");
        assert_eq!(sizes[0].size, 400);
        assert_eq!(sizes[1].size, 500);
        session.quit();
        let _ = server.join();
    }

    #[test]
    fn retr_unstuffs_dot_lines() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        // 服务器端把行首的 `.` 写成 `..`；客户端必须还原成单个 `.`
        let server = std::thread::spawn(move || serve_once(listener, b"Subject: x\r\n\r\n..dot\r\n".to_vec()));

        let mut session = Pop3Session::connect(&account(port), "pw", None).expect("connect");
        let raw = session.retr(1).expect("retr");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains(".dot"), "点填充应被还原：{text}");
        assert!(!text.contains("..dot"), "多点号应还原成一个：{text}");
        session.quit();
        let _ = server.join();
    }

    #[test]
    fn login_failure_reports_server_message() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("accept");
            let mut out = socket.try_clone().expect("clone");
            let mut reader = BufReader::new(socket);
            out.write_all(b"+OK ready\r\n").ok();
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let upper = line.trim().to_ascii_uppercase();
                line.clear();
                if upper.starts_with("USER") {
                    out.write_all(b"+OK\r\n").ok();
                } else if upper.starts_with("PASS") {
                    out.write_all(b"-ERR bad credentials\r\n").ok();
                    break;
                }
            }
        });
        let error = match Pop3Session::connect(&account(port), "wrong", None) {
            Ok(_) => panic!("错误口令不应登录成功"),
            Err(error) => error,
        };
        assert!(error.contains("POP3 登录失败"), "{error}");
        assert!(error.contains("bad credentials"), "{error}");
        let _ = server.join();
    }

    #[test]
    fn redacts_password_in_log_lines() {
        assert_eq!(redact("PASS hunter2"), "PASS ******");
        assert_eq!(redact("USER me"), "USER me");
    }
}
