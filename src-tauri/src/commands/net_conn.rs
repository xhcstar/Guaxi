//! 网络工具箱 —— 客户端/服务端模拟 + DNS + TLS 探测（批 N2）。
//!
//! 套接字级(无需特权)：TCP/UDP 真实连接、监听、收发任意字节；DNS 用 hickory
//! 构造/解析并向指定 DNS 服务器查询；TLS 用 rustls 完成握手并报告版本/密码套件/证书链。
//! 服务端把每次收到的数据经事件推前端。原始套接字(Npcap)抓包/注入见 net_capture 模块。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use uuid::Uuid;

fn to_bytes(data: &str, is_hex: bool) -> Result<Vec<u8>, String> {
    if is_hex {
        let compact: String = data.chars().filter(|c| !c.is_whitespace()).collect();
        hex::decode(&compact).map_err(|e| format!("hex 解码失败: {e}"))
    } else {
        Ok(data.as_bytes().to_vec())
    }
}

#[derive(Serialize)]
pub struct SendResult {
    pub sent: usize,
    pub recv_len: usize,
    pub recv_hex: String,
    pub recv_text: String,
}

/// TCP 客户端：连接 → 发送 → 读取响应（读到超时或对端关闭为止）。
#[command]
pub async fn net_tcp_send(
    host: String,
    port: u16,
    data: String,
    is_hex: bool,
    timeout_ms: Option<u64>,
) -> Result<SendResult, String> {
    let payload = to_bytes(&data, is_hex)?;
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(3000));
    let mut stream = tokio::time::timeout(timeout, TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| "连接超时".to_string())?
        .map_err(|e| format!("连接失败: {e}"))?;
    stream.write_all(&payload).await.map_err(|e| format!("发送失败: {e}"))?;
    let mut buf = vec![0u8; 65536];
    let mut recv: Vec<u8> = Vec::new();
    loop {
        match tokio::time::timeout(timeout, stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                recv.extend_from_slice(&buf[..n]);
                if recv.len() > 1_048_576 {
                    break;
                }
            }
            Ok(Err(e)) => return Err(format!("读取失败: {e}")),
            Err(_) => break, // 超时即视为响应结束
        }
    }
    Ok(SendResult {
        sent: payload.len(),
        recv_len: recv.len(),
        recv_hex: hex::encode(&recv),
        recv_text: String::from_utf8_lossy(&recv).into_owned(),
    })
}

/// UDP 客户端：发送一个数据报，等待一个响应（超时返回空）。
#[command]
pub async fn net_udp_send(
    host: String,
    port: u16,
    data: String,
    is_hex: bool,
    timeout_ms: Option<u64>,
) -> Result<SendResult, String> {
    let payload = to_bytes(&data, is_hex)?;
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(3000));
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| format!("绑定失败: {e}"))?;
    sock.connect((host.as_str(), port)).await.map_err(|e| format!("连接失败: {e}"))?;
    let sent = sock.send(&payload).await.map_err(|e| format!("发送失败: {e}"))?;
    let mut buf = vec![0u8; 65536];
    let recv = match tokio::time::timeout(timeout, sock.recv(&mut buf)).await {
        Ok(Ok(n)) => buf[..n].to_vec(),
        _ => Vec::new(),
    };
    Ok(SendResult {
        sent,
        recv_len: recv.len(),
        recv_hex: hex::encode(&recv),
        recv_text: String::from_utf8_lossy(&recv).into_owned(),
    })
}

// ============================ DNS ============================

#[derive(Serialize)]
pub struct DnsRecord {
    pub name: String,
    pub rtype: String,
    pub ttl: u32,
    pub data: String,
}

#[derive(Serialize)]
pub struct DnsResult {
    pub query_hex: String,
    pub response_hex: String,
    pub answers: Vec<DnsRecord>,
}

fn dns_record_type(s: &str) -> hickory_proto::rr::RecordType {
    use hickory_proto::rr::RecordType as RT;
    match s.to_ascii_uppercase().as_str() {
        "A" => RT::A,
        "AAAA" => RT::AAAA,
        "CNAME" => RT::CNAME,
        "MX" => RT::MX,
        "TXT" => RT::TXT,
        "NS" => RT::NS,
        "SOA" => RT::SOA,
        "PTR" => RT::PTR,
        "SRV" => RT::SRV,
        "CAA" => RT::CAA,
        _ => RT::A,
    }
}

/// DNS 查询：向指定 DNS 服务器(UDP 53)构造并发送查询，解析应答。
#[command]
pub async fn net_dns_query(
    server: String,
    name: String,
    qtype: String,
) -> Result<DnsResult, String> {
    use hickory_proto::op::{Message, Query};
    use hickory_proto::rr::Name;

    let dname = Name::from_ascii(&name).map_err(|e| format!("域名无效: {e}"))?;
    let mut msg = Message::new();
    msg.set_id(rand::random::<u16>())
        .set_recursion_desired(true)
        .add_query(Query::query(dname, dns_record_type(&qtype)));
    let query_bytes = msg.to_vec().map_err(|e| format!("构造查询失败: {e}"))?;

    let server_addr = if server.contains(':') { server.clone() } else { format!("{server}:53") };
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| format!("绑定失败: {e}"))?;
    sock.connect(&server_addr).await.map_err(|e| format!("连接 DNS 失败: {e}"))?;
    sock.send(&query_bytes).await.map_err(|e| format!("发送失败: {e}"))?;
    let mut buf = vec![0u8; 4096];
    let n = match tokio::time::timeout(Duration::from_millis(4000), sock.recv(&mut buf)).await {
        Ok(Ok(n)) => n,
        _ => return Err("DNS 查询超时".to_string()),
    };
    let resp = &buf[..n];
    let parsed = Message::from_vec(resp).map_err(|e| format!("解析应答失败: {e}"))?;
    let answers = parsed
        .answers()
        .iter()
        .map(|r| DnsRecord {
            name: r.name().to_string(),
            rtype: format!("{:?}", r.record_type()),
            ttl: r.ttl(),
            data: r.data().map(|d| d.to_string()).unwrap_or_default(),
        })
        .collect();
    Ok(DnsResult {
        query_hex: hex::encode(&query_bytes),
        response_hex: hex::encode(resp),
        answers,
    })
}

// ============================ TCP/UDP 服务端模拟 ============================

struct ServerHandle {
    stop: Arc<AtomicBool>,
}
type ServerMap = HashMap<String, ServerHandle>;
static SERVERS: OnceLock<Mutex<ServerMap>> = OnceLock::new();
fn servers() -> &'static Mutex<ServerMap> {
    SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 当前在跑的监听服务数（设置页「服务器」用）。
pub fn active_server_count() -> usize {
    servers().lock().map(|guard| guard.len()).unwrap_or(0)
}

pub fn shutdown_all_servers() {
    if let Ok(g) = servers().lock() {
        for (_id, h) in g.iter() {
            h.stop.store(true, Ordering::SeqCst);
        }
    }
}

#[derive(Serialize, Clone)]
struct ServerEvent {
    session_id: String,
    peer: String,
    data_hex: String,
    data_text: String,
}

/// TCP 服务端：监听端口，收到数据经事件 net-server-event 推前端；echo=true 时回显。
#[command]
pub async fn net_tcp_listen(app: AppHandle, port: u16, echo: bool) -> Result<String, String> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| format!("监听失败: {e}"))?;
    let sid = Uuid::new_v4().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    servers()
        .lock()
        .map_err(|_| "锁失败".to_string())?
        .insert(sid.clone(), ServerHandle { stop: stop.clone() });

    let sid_task = sid.clone();
    tokio::spawn(async move {
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let accept = tokio::time::timeout(Duration::from_millis(500), listener.accept()).await;
            let (mut socket, peer) = match accept {
                Ok(Ok(v)) => v,
                Ok(Err(_)) => continue,
                Err(_) => continue, // 超时：回头检查 stop
            };
            let app = app.clone();
            let sid = sid_task.clone();
            let stop_conn = stop.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                loop {
                    if stop_conn.load(Ordering::SeqCst) {
                        break;
                    }
                    match tokio::time::timeout(Duration::from_millis(500), socket.read(&mut buf)).await {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => {
                            let chunk = &buf[..n];
                            let _ = app.emit(
                                "net-server-event",
                                ServerEvent {
                                    session_id: sid.clone(),
                                    peer: peer.to_string(),
                                    data_hex: hex::encode(chunk),
                                    data_text: String::from_utf8_lossy(chunk).into_owned(),
                                },
                            );
                            if echo {
                                let _ = socket.write_all(chunk).await;
                            }
                        }
                        Ok(Err(_)) => break,
                        Err(_) => continue,
                    }
                }
            });
        }
    });
    Ok(sid)
}

/// UDP 服务端：监听端口，收到数据报经事件推前端；echo=true 时原样回发来源。
#[command]
pub async fn net_udp_listen(app: AppHandle, port: u16, echo: bool) -> Result<String, String> {
    let sock = UdpSocket::bind(("0.0.0.0", port))
        .await
        .map_err(|e| format!("监听失败: {e}"))?;
    let sid = Uuid::new_v4().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    servers()
        .lock()
        .map_err(|_| "锁失败".to_string())?
        .insert(sid.clone(), ServerHandle { stop: stop.clone() });

    let sid_task = sid.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            match tokio::time::timeout(Duration::from_millis(500), sock.recv_from(&mut buf)).await {
                Ok(Ok((n, peer))) => {
                    let chunk = &buf[..n];
                    let _ = app.emit(
                        "net-server-event",
                        ServerEvent {
                            session_id: sid_task.clone(),
                            peer: peer.to_string(),
                            data_hex: hex::encode(chunk),
                            data_text: String::from_utf8_lossy(chunk).into_owned(),
                        },
                    );
                    if echo {
                        let _ = sock.send_to(chunk, peer).await;
                    }
                }
                Ok(Err(_)) => {}
                Err(_) => {}
            }
        }
    });
    Ok(sid)
}

#[command]
pub fn net_server_stop(session_id: String) -> Result<(), String> {
    if let Some(h) = servers().lock().map_err(|_| "锁失败".to_string())?.remove(&session_id) {
        h.stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

// ============================ TLS 握手探测 ============================

#[derive(Serialize)]
pub struct TlsCert {
    pub subject: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
}

#[derive(Serialize)]
pub struct TlsReport {
    pub version: String,
    pub cipher: String,
    pub alpn: String,
    pub certs: Vec<TlsCert>,
}

/// TLS 探测：与 host:port 完成握手，报告协议版本 / 密码套件 / 证书链（用 rustls + ring）。
#[command]
pub async fn net_tls_inspect(host: String, port: u16, sni: Option<String>) -> Result<TlsReport, String> {
    use std::sync::Arc as StdArc;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    use tokio_rustls::TlsConnector;
    use rustls_pki_types::ServerName;

    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let provider = StdArc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("TLS 配置失败: {e}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(StdArc::new(config));

    let server_name_str = sni.unwrap_or_else(|| host.clone());
    let server_name = ServerName::try_from(server_name_str)
        .map_err(|e| format!("SNI 无效: {e}"))?;

    let tcp = tokio::time::timeout(Duration::from_millis(6000), TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| "连接超时".to_string())?
        .map_err(|e| format!("连接失败: {e}"))?;
    let tls = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| format!("TLS 握手失败: {e}"))?;

    let (_, conn) = tls.get_ref();
    let version = conn.protocol_version().map(|v| format!("{v:?}")).unwrap_or_default();
    let cipher = conn.negotiated_cipher_suite().map(|c| format!("{:?}", c.suite())).unwrap_or_default();
    let alpn = conn.alpn_protocol().map(|p| String::from_utf8_lossy(p).into_owned()).unwrap_or_default();

    let mut certs: Vec<TlsCert> = Vec::new();
    if let Some(chain) = conn.peer_certificates() {
        for c in chain {
            match x509_parser::parse_x509_certificate(c.as_ref()) {
                Ok((_, cert)) => certs.push(TlsCert {
                    subject: cert.subject().to_string(),
                    issuer: cert.issuer().to_string(),
                    not_before: cert.validity().not_before.to_string(),
                    not_after: cert.validity().not_after.to_string(),
                }),
                Err(_) => certs.push(TlsCert {
                    subject: "(解析失败)".into(),
                    issuer: String::new(),
                    not_before: String::new(),
                    not_after: String::new(),
                }),
            }
        }
    }

    Ok(TlsReport { version, cipher, alpn, certs })
}

// ============================ HTTP 客户端 ============================

#[derive(Deserialize)]
pub struct HttpReq {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct HttpResp {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub body_len: usize,
    pub elapsed_ms: u128,
}

/// HTTP/HTTPS 客户端：自定义方法/头/体发起请求，返回状态/头/体。为便于测试接受自签名证书。
#[command]
pub async fn net_http_request(req: HttpReq) -> Result<HttpResp, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_millis(req.timeout_ms.unwrap_or(10000)))
        .build()
        .map_err(|e| format!("构造客户端失败: {e}"))?;
    let method = reqwest::Method::from_bytes(req.method.trim().to_uppercase().as_bytes())
        .map_err(|_| format!("非法方法: {}", req.method))?;
    let mut rb = client.request(method, req.url.trim());
    for (k, v) in &req.headers {
        if !k.trim().is_empty() {
            rb = rb.header(k.as_str(), v.as_str());
        }
    }
    if let Some(b) = req.body.filter(|s| !s.is_empty()) {
        rb = rb.body(b);
    }
    let start = Instant::now();
    let resp = rb.send().await.map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = resp.text().await.unwrap_or_default();
    Ok(HttpResp {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        body_len: body.len(),
        body,
        headers,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

// ============================ TLS 客户端裸发 ============================

/// TLS 客户端：完成握手后，把 data 明文写入加密通道并读取响应（如手写 HTTPS 请求）。
#[command]
pub async fn net_tls_send(
    host: String,
    port: u16,
    sni: Option<String>,
    data: String,
    is_hex: bool,
    timeout_ms: Option<u64>,
) -> Result<SendResult, String> {
    use std::sync::Arc as StdArc;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    use tokio_rustls::TlsConnector;
    use rustls_pki_types::ServerName;

    let payload = to_bytes(&data, is_hex)?;
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(5000));

    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = StdArc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("TLS 配置失败: {e}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(StdArc::new(config));
    let server_name = ServerName::try_from(sni.unwrap_or_else(|| host.clone()))
        .map_err(|e| format!("SNI 无效: {e}"))?;

    let tcp = tokio::time::timeout(timeout, TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| "连接超时".to_string())?
        .map_err(|e| format!("连接失败: {e}"))?;
    let mut tls = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| format!("TLS 握手失败: {e}"))?;
    tls.write_all(&payload).await.map_err(|e| format!("发送失败: {e}"))?;
    let mut buf = vec![0u8; 65536];
    let mut recv: Vec<u8> = Vec::new();
    loop {
        match tokio::time::timeout(timeout, tls.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                recv.extend_from_slice(&buf[..n]);
                if recv.len() > 1_048_576 {
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => break,
        }
    }
    Ok(SendResult {
        sent: payload.len(),
        recv_len: recv.len(),
        recv_hex: hex::encode(&recv),
        recv_text: String::from_utf8_lossy(&recv).into_owned(),
    })
}

// ============================ HTTP 模拟服务端 ============================

/// HTTP 服务端：监听端口,收到请求经事件推前端,并回一个可配置的 HTTP 响应。
#[command]
pub async fn net_http_listen(
    app: AppHandle,
    port: u16,
    status: u16,
    body: String,
) -> Result<String, String> {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| format!("监听失败: {e}"))?;
    let sid = Uuid::new_v4().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    servers()
        .lock()
        .map_err(|_| "锁失败".to_string())?
        .insert(sid.clone(), ServerHandle { stop: stop.clone() });

    let sid_task = sid.clone();
    let reason = http_reason(status);
    tokio::spawn(async move {
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let accept = tokio::time::timeout(Duration::from_millis(500), listener.accept()).await;
            let (mut socket, peer) = match accept {
                Ok(Ok(v)) => v,
                _ => continue,
            };
            let app = app.clone();
            let sid = sid_task.clone();
            let body = body.clone();
            let reason = reason.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                if let Ok(Ok(n)) = tokio::time::timeout(Duration::from_millis(2000), socket.read(&mut buf)).await {
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let _ = app.emit(
                        "net-server-event",
                        ServerEvent {
                            session_id: sid.clone(),
                            peer: peer.to_string(),
                            data_hex: hex::encode(&buf[..n]),
                            data_text: req,
                        },
                    );
                }
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
                    body.as_bytes().len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    Ok(sid)
}

fn http_reason(status: u16) -> String {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
    .to_string()
}
