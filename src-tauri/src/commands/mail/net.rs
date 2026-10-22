//! 邮箱传输层：代理感知的 TCP 连接 + rustls TLS。
//!
//! 为什么不用 `imap::connect` 和 lettre 的 SMTP 客户端：两者的连接建立都在内部写死了
//! `TcpStream::connect`，代理配置只能走 `ALL_PROXY` 环境变量（进程级、全局生效），
//! 没法按账户区分。这里把「拿到一条已连通的字节流」单独抽出来，IMAP/SMTP 都建在它上面：
//!
//! ```text
//!   resolve_proxy(account) ──> direct     ──> TcpStream::connect
//!                          ├─> http(s)   ──> CONNECT host:port → 2xx → 透传
//!                          └─> socks5    ──> SOCKS5 握手（域名交给代理解析）
//!   然后按 Security：ssl → 立刻 TLS；starttls → 交给协议层发 STARTTLS 后再包 TLS
//! ```
//!
//! TLS 用 rustls + ring（与项目其余部分一致：Windows 构建机没有 NASM，不能碰 aws-lc-rs），
//! 根证书来自系统证书库，企业内网自签的邮件网关也能直接信任。

use super::model::{ProxyConfig, ProxyMode, Security};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::{debug, info};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(60);

/// 一条已加密或明文的双向字节流。
pub enum Stream {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

impl Stream {
    /// 把明文流升级为 TLS（STARTTLS 用）。`host` 必须是要校验的服务器名。
    pub fn upgrade_tls(self, host: &str) -> Result<Stream, String> {
        match self {
            Stream::Tls(_) => Err("连接已经是加密的".to_string()),
            Stream::Plain(tcp) => {
                let conn = tls_connection(host)?;
                Ok(Stream::Tls(Box::new(StreamOwned::new(conn, tcp))))
            }
        }
    }
}

fn tls_config() -> Arc<ClientConfig> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            let result = rustls_native_certs::load_native_certs();
            let mut added = 0usize;
            for cert in result.certs {
                if roots.add(cert).is_ok() {
                    added += 1;
                }
            }
            if added == 0 {
                // 系统证书库读不出来（精简版 Windows 镜像偶尔如此）时退回内置根证书，
                // 否则所有 TLS 连接都会失败，而且报错看起来像服务器证书有问题。
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                info!("mail: 系统根证书为空，已回退到内置根证书");
            }
            for err in result.errors {
                debug!(error = %err, "mail: 读取系统根证书时出现错误（已跳过）");
            }
            let config = ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

fn tls_connection(host: &str) -> Result<ClientConnection, String> {
    let server_name = rustls_pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("服务器名无效（{host}）: {e}"))?;
    ClientConnection::new(tls_config(), server_name).map_err(|e| format!("TLS 初始化失败: {e}"))
}

/// 代理的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedProxy {
    Direct,
    /// HTTP 代理（`http://` 或 `https://`），走 CONNECT 隧道。
    Http {
        host: String,
        port: u16,
        auth: Option<(String, String)>,
    },
    /// SOCKS5，域名交给代理解析。
    Socks5 {
        host: String,
        port: u16,
        auth: Option<(String, String)>,
    },
}

/// 解析代理 URL。支持 `http://`、`https://`、`socks5://`、`socks5h://`，
/// 允许内嵌 `user:pass@`。不带协议头时按 HTTP 代理处理。
pub fn parse_proxy_url(raw: &str) -> Result<ResolvedProxy, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(ResolvedProxy::Direct);
    }
    let normalized = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    };
    let url = url::Url::parse(&normalized).map_err(|e| format!("代理地址无效（{raw}）: {e}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| format!("代理地址缺少主机名: {raw}"))?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(match url.scheme() {
        "socks5" | "socks5h" => 1080,
        "https" => 443,
        _ => 8080,
    });
    let auth = {
        let user = url.username();
        if user.is_empty() {
            None
        } else {
            Some((
                percent_decode(user),
                percent_decode(url.password().unwrap_or("")),
            ))
        }
    };
    match url.scheme() {
        "socks5" | "socks5h" => Ok(ResolvedProxy::Socks5 { host, port, auth }),
        "http" | "https" => Ok(ResolvedProxy::Http { host, port, auth }),
        other => Err(format!("不支持的代理协议: {other}")),
    }
}

fn percent_decode(input: &str) -> String {
    urlencoding::decode(input)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| input.to_string())
}

/// 跟随系统代理（环境变量优先，其次 Windows 注册表 / 各平台设置）。
pub fn system_proxy_url() -> Option<String> {
    for key in [
        "ALL_PROXY",
        "all_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    match sysproxy::Sysproxy::get_system_proxy() {
        Ok(sp) if sp.enable && !sp.host.is_empty() => {
            Some(format!("http://{}:{}", sp.host, sp.port))
        }
        _ => None,
    }
}

/// 目标主机是否命中直连白名单。
fn is_bypassed(cfg: &ProxyConfig, host: &str) -> bool {
    let host_lower = host.to_ascii_lowercase();
    cfg.bypass.iter().any(|entry| {
        let entry = entry.trim().trim_start_matches('.').to_ascii_lowercase();
        !entry.is_empty() && (host_lower == entry || host_lower.ends_with(&format!(".{entry}")))
    })
}

pub fn resolve_proxy(config: Option<&ProxyConfig>, host: &str) -> Result<ResolvedProxy, String> {
    let Some(cfg) = config else {
        return Ok(ResolvedProxy::Direct);
    };
    if is_bypassed(cfg, host) {
        return Ok(ResolvedProxy::Direct);
    }
    let raw = match cfg.mode {
        ProxyMode::None => return Ok(ResolvedProxy::Direct),
        ProxyMode::System => system_proxy_url().unwrap_or_default(),
        ProxyMode::Custom => cfg.url.clone().unwrap_or_default(),
    };
    parse_proxy_url(&raw)
}

/// 建立到 `host:port` 的明文 TCP 连接（必要时经代理隧道）。
fn connect_tcp(host: &str, port: u16, proxy: &ResolvedProxy) -> Result<TcpStream, String> {
    match proxy {
        ResolvedProxy::Direct => {
            let addr = (host, port)
                .to_socket_addrs()
                .map_err(|e| format!("解析 {host}:{port} 失败: {e}"))?
                .next()
                .ok_or_else(|| format!("{host}:{port} 没有可用的地址"))?;
            let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
                .map_err(|e| format!("连接 {host}:{port} 失败: {e}"))?;
            configure(&stream)?;
            Ok(stream)
        }
        ResolvedProxy::Http {
            host: ph,
            port: pp,
            auth,
        } => {
            let addr = (ph.as_str(), *pp)
                .to_socket_addrs()
                .map_err(|e| format!("解析代理 {ph}:{pp} 失败: {e}"))?
                .next()
                .ok_or_else(|| format!("代理 {ph}:{pp} 没有可用的地址"))?;
            let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
                .map_err(|e| format!("连接代理 {ph}:{pp} 失败: {e}"))?;
            configure(&stream)?;

            let mut request = format!(
                "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\nProxy-Connection: keep-alive\r\n"
            );
            if let Some((user, pass)) = auth {
                let token = base64_encode(format!("{user}:{pass}").as_bytes());
                request.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
            }
            request.push_str("\r\n");
            stream
                .write_all(request.as_bytes())
                .map_err(|e| format!("代理握手写入失败: {e}"))?;
            stream.flush().ok();

            let (status, rest) = read_http_head(&mut stream)?;
            if !(200..300).contains(&status) {
                return Err(format!(
                    "代理拒绝建立隧道（HTTP {status}）{}",
                    if status == 407 {
                        "：请检查代理用户名/密码"
                    } else {
                        ""
                    }
                ));
            }
            if !rest.is_empty() {
                // CONNECT 成功后协议数据理论上不会先到；万一捎带了，丢掉会损坏协议流，
                // 这里直接报错比默默错位更安全。
                return Err("代理在隧道响应里捎带了额外数据，无法继续".to_string());
            }
            Ok(stream)
        }
        ResolvedProxy::Socks5 {
            host: ph,
            port: pp,
            auth,
        } => {
            let proxy_addr = (ph.as_str(), *pp)
                .to_socket_addrs()
                .map_err(|e| format!("解析代理 {ph}:{pp} 失败: {e}"))?
                .next()
                .ok_or_else(|| format!("代理 {ph}:{pp} 没有可用的地址"))?;
            // socks 0.3 的 `connect` 接受 `ToSocketAddrs` 作为目标，但它会本地解析域名；
            // 用 TargetAddr::Domain 把域名交给代理（socks5h 语义），更符合大陆环境的使用习惯。
            let target = socks::TargetAddr::Domain(host.to_string(), port);
            let stream = match auth {
                Some((user, pass)) => {
                    socks::Socks5Stream::connect_with_password(proxy_addr, target, user, pass)
                }
                None => socks::Socks5Stream::connect(proxy_addr, target),
            }
            .map_err(|e| format!("经 SOCKS5 代理连接 {host}:{port} 失败: {e}"))?;
            let tcp = stream.into_inner();
            configure(&tcp)?;
            Ok(tcp)
        }
    }
}

fn configure(stream: &TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|e| format!("设置读超时失败: {e}"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|e| format!("设置写超时失败: {e}"))?;
    stream.set_nodelay(true).ok();
    Ok(())
}

/// 读 HTTP 响应头，返回 (状态码, 头之后的剩余字节)。剩余字节通常为空。
fn read_http_head(stream: &mut TcpStream) -> Result<(u16, Vec<u8>), String> {
    let mut buf: Vec<u8> = Vec::with_capacity(512);
    let mut scanned_from = 0usize;
    loop {
        let mut chunk = [0u8; 512];
        let n = stream
            .read(&mut chunk)
            .map_err(|e| format!("读取代理响应失败: {e}"))?;
        if n == 0 {
            return Err("代理在握手过程中关闭了连接".to_string());
        }
        let search_from = scanned_from.saturating_sub(3);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(rel) = find_subsequence(&buf[search_from..], b"\r\n\r\n") {
            let end = search_from + rel + 4;
            let head = String::from_utf8_lossy(&buf[..end]).to_string();
            let status = head
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|code| code.parse::<u16>().ok())
                .unwrap_or(0);
            return Ok((status, buf[end..].to_vec()));
        }
        scanned_from = buf.len();
        if buf.len() > 64 * 1024 {
            return Err("代理响应头过长".to_string());
        }
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// 建立到邮件服务器的字节流，按 `security` 决定是否立即做 TLS。
///
/// `starttls` 时返回明文流，由协议层发出 STARTTLS 后再 `Stream::upgrade_tls`。
pub fn open_stream(
    host: &str,
    port: u16,
    security: Security,
    proxy_config: Option<&ProxyConfig>,
) -> Result<Stream, String> {
    if host.trim().is_empty() {
        return Err("服务器地址为空".to_string());
    }
    let proxy = resolve_proxy(proxy_config, host)?;
    debug!(
        host,
        port,
        security = security.as_str(),
        proxy = %proxy_label(&proxy),
        "mail: 建立连接"
    );
    let tcp = connect_tcp(host, port, &proxy)?;
    match security {
        Security::Ssl => {
            let conn = tls_connection(host)?;
            Ok(Stream::Tls(Box::new(StreamOwned::new(conn, tcp))))
        }
        Security::Starttls | Security::None => Ok(Stream::Plain(tcp)),
    }
}

fn proxy_label(proxy: &ResolvedProxy) -> String {
    match proxy {
        ResolvedProxy::Direct => "direct".to_string(),
        ResolvedProxy::Http { host, port, .. } => format!("http://{host}:{port}"),
        ResolvedProxy::Socks5 { host, port, .. } => format!("socks5://{host}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_proxy_with_auth() {
        let p = parse_proxy_url("http://user:p%40ss@127.0.0.1:7890").unwrap();
        assert_eq!(
            p,
            ResolvedProxy::Http {
                host: "127.0.0.1".into(),
                port: 7890,
                auth: Some(("user".into(), "p@ss".into())),
            }
        );
    }

    #[test]
    fn parse_socks5_default_port() {
        let p = parse_proxy_url("socks5h://10.0.0.2").unwrap();
        assert_eq!(
            p,
            ResolvedProxy::Socks5 {
                host: "10.0.0.2".into(),
                port: 1080,
                auth: None
            }
        );
    }

    #[test]
    fn bare_host_is_http_proxy() {
        let p = parse_proxy_url("127.0.0.1:8080").unwrap();
        assert!(matches!(p, ResolvedProxy::Http { port: 8080, .. }));
    }

    #[test]
    fn empty_proxy_is_direct() {
        assert_eq!(parse_proxy_url("  ").unwrap(), ResolvedProxy::Direct);
    }

    #[test]
    fn bypass_matches_suffix_and_exact() {
        let cfg = ProxyConfig {
            mode: ProxyMode::System,
            url: None,
            bypass: vec!["corp.local".into(), "localhost".into()],
        };
        assert!(is_bypassed(&cfg, "mail.corp.local"));
        assert!(is_bypassed(&cfg, "localhost"));
        assert!(!is_bypassed(&cfg, "corp.local.evil.com"));
    }

    #[test]
    fn find_subsequence_locates_header_end() {
        let buf = b"HTTP/1.1 200 OK\r\nX: y\r\n\r\nrest";
        // 状态行 17 字节 + "X: y" 4 字节 + CRLF，空行从第 21 字节开始。
        assert_eq!(find_subsequence(buf, b"\r\n\r\n"), Some(21));
    }
}
