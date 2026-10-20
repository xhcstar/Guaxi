//! 终端调试工坊 —— 串口(A2) 与 Telnet(A3) 连接。
//!
//! 两者都把接收到的字节实时推前端（xterm 渲染），输入经命令下发。串口用
//! serialport crate（阻塞读放在独立线程）；Telnet 是裸 TCP（tokio），输入经
//! mpsc 通道下发到读写事件循环。

use std::collections::HashMap;
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{command, AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone, Serialize)]
struct OutputEvent {
    session_id: String,
    data: String,
}

// ============================ 串口 A2 ============================

struct SerialSession {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    stop: Arc<AtomicBool>,
    /// 会话描述（路径 @ 波特率），GDB 等消费者在列表里展示用。
    label: String,
    /// 额外订阅者（GDB 等需要原始字节流的消费者）。终端渲染走事件，不受影响。
    subs: Arc<Mutex<Vec<mpsc::UnboundedSender<Vec<u8>>>>>,
}

#[derive(Serialize)]
pub struct ConsoleSessionInfo {
    pub id: String,
    pub kind: String,
    pub label: String,
}

#[command]
pub fn console_list_sessions() -> Vec<ConsoleSessionInfo> {
    let mut out = Vec::new();
    if let Ok(g) = serial_store().lock() {
        for (id, s) in g.iter() {
            out.push(ConsoleSessionInfo { id: id.clone(), kind: "serial".into(), label: s.label.clone() });
        }
    }
    if let Ok(g) = telnet_store().lock() {
        for (id, s) in g.iter() {
            out.push(ConsoleSessionInfo { id: id.clone(), kind: "telnet".into(), label: s.label.clone() });
        }
    }
    out
}

type SerialMap = HashMap<String, SerialSession>;
static SERIAL_SESSIONS: OnceLock<Mutex<SerialMap>> = OnceLock::new();
fn serial_store() -> &'static Mutex<SerialMap> {
    SERIAL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 当前打开的串口会话数（设置页「服务器」用）。
pub fn serial_active_count() -> usize {
    serial_store().lock().map(|guard| guard.len()).unwrap_or(0)
}

#[derive(Serialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub kind: String,
}

#[command]
pub fn serial_list_ports() -> Result<Vec<SerialPortInfo>, String> {
    let ports = serialport::available_ports().map_err(|e| format!("枚举串口失败: {e}"))?;
    Ok(ports
        .into_iter()
        .map(|p| {
            let kind = match p.port_type {
                serialport::SerialPortType::UsbPort(_) => "USB",
                serialport::SerialPortType::BluetoothPort => "Bluetooth",
                serialport::SerialPortType::PciPort => "PCI",
                _ => "Unknown",
            }
            .to_string();
            SerialPortInfo { name: p.port_name, kind }
        })
        .collect())
}

#[command]
pub fn serial_open(app: AppHandle, path: String, baud: u32) -> Result<String, String> {
    let port = serialport::new(&path, baud)
        .timeout(Duration::from_millis(200))
        .open()
        .map_err(|e| format!("打开串口失败: {e}"))?;
    let port = Arc::new(Mutex::new(port));
    let stop = Arc::new(AtomicBool::new(false));
    let subs: Arc<Mutex<Vec<mpsc::UnboundedSender<Vec<u8>>>>> = Arc::new(Mutex::new(Vec::new()));
    let session_id = Uuid::new_v4().to_string();

    let reader_port = port.clone();
    let reader_stop = stop.clone();
    let reader_subs = subs.clone();
    let sid = session_id.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while !reader_stop.load(Ordering::SeqCst) {
            let n = {
                let mut p = match reader_port.lock() {
                    Ok(p) => p,
                    Err(_) => break,
                };
                match p.read(&mut buf) {
                    Ok(n) => n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => 0,
                    Err(_) => break,
                }
            };
            if n > 0 {
                let bytes = buf[..n].to_vec();
                if let Ok(mut g) = reader_subs.lock() {
                    g.retain(|tx| tx.send(bytes.clone()).is_ok());
                }
                let data = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = app.emit("serial-output", OutputEvent { session_id: sid.clone(), data });
            } else {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let _ = app.emit("serial-exit", OutputEvent { session_id: sid.clone(), data: String::new() });
    });

    serial_store()
        .lock()
        .map_err(|_| "串口锁失败".to_string())?
        .insert(session_id.clone(), SerialSession { port, stop, subs, label: format!("{path} @ {baud}") });
    Ok(session_id)
}

/// 订阅串口原始字节流（GDB/MI 之类的消费者用）。
pub(crate) fn serial_subscribe(
    session_id: &str,
) -> Result<mpsc::UnboundedReceiver<Vec<u8>>, String> {
    let store = serial_store().lock().map_err(|_| "串口锁失败".to_string())?;
    let s = store.get(session_id).ok_or_else(|| "串口会话不存在".to_string())?;
    let (tx, rx) = mpsc::unbounded_channel();
    s.subs.lock().map_err(|_| "串口锁失败".to_string())?.push(tx);
    Ok(rx)
}

/// 往串口写原始字节（订阅者调用）。
pub(crate) fn serial_write_bytes(session_id: &str, data: &[u8]) -> Result<(), String> {
    let store = serial_store().lock().map_err(|_| "串口锁失败".to_string())?;
    let s = store.get(session_id).ok_or_else(|| "串口会话不存在".to_string())?;
    let mut p = s.port.lock().map_err(|_| "串口锁失败".to_string())?;
    p.write_all(data).map_err(|e| format!("写串口失败: {e}"))
}

#[command]
pub fn serial_write(session_id: String, data: String) -> Result<(), String> {
    let store = serial_store().lock().map_err(|_| "串口锁失败".to_string())?;
    let s = store.get(&session_id).ok_or_else(|| "串口会话不存在".to_string())?;
    let mut p = s.port.lock().map_err(|_| "串口锁失败".to_string())?;
    p.write_all(data.as_bytes()).map_err(|e| format!("写串口失败: {e}"))
}

#[command]
pub fn serial_close(session_id: String) -> Result<(), String> {
    if let Some(s) = serial_store().lock().map_err(|_| "串口锁失败".to_string())?.remove(&session_id) {
        s.stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

pub fn shutdown_all_serial() {
    if let Ok(mut g) = serial_store().lock() {
        for (_id, s) in g.drain() {
            s.stop.store(true, Ordering::SeqCst);
        }
    }
}

// ============================ Telnet A3 ============================

enum TelnetCmd {
    Data(Vec<u8>),
    Close,
}

struct TelnetSession {
    cmd_tx: mpsc::UnboundedSender<TelnetCmd>,
    label: String,
    /// 额外订阅者（GDB 等）。
    subs: Arc<Mutex<Vec<mpsc::UnboundedSender<Vec<u8>>>>>,
}

type TelnetMap = HashMap<String, TelnetSession>;
static TELNET_SESSIONS: OnceLock<Mutex<TelnetMap>> = OnceLock::new();
fn telnet_store() -> &'static Mutex<TelnetMap> {
    TELNET_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 当前打开的 Telnet 会话数（设置页「服务器」用）。
pub fn telnet_active_count() -> usize {
    telnet_store().lock().map(|guard| guard.len()).unwrap_or(0)
}

pub fn shutdown_all_telnet() {
    if let Ok(mut g) = telnet_store().lock() {
        for (_id, s) in g.drain() {
            let _ = s.cmd_tx.send(TelnetCmd::Close);
        }
    }
}

#[command]
pub async fn telnet_open(app: AppHandle, host: String, port: u16) -> Result<String, String> {
    let stream = tokio::net::TcpStream::connect((host.as_str(), port))
        .await
        .map_err(|e| format!("Telnet 连接失败: {e}"))?;
    let (mut read_half, mut write_half) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<TelnetCmd>();
    let subs: Arc<Mutex<Vec<mpsc::UnboundedSender<Vec<u8>>>>> = Arc::new(Mutex::new(Vec::new()));
    let session_id = Uuid::new_v4().to_string();
    telnet_store()
        .lock()
        .map_err(|_| "telnet 锁失败".to_string())?
        .insert(session_id.clone(), TelnetSession { cmd_tx: tx, subs: subs.clone(), label: format!("{host}:{port}") });

    let sid = session_id.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            tokio::select! {
                r = read_half.read(&mut buf) => {
                    match r {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut g) = subs.lock() {
                                g.retain(|tx| tx.send(buf[..n].to_vec()).is_ok());
                            }
                            let data = String::from_utf8_lossy(&buf[..n]).to_string();
                            let _ = app.emit("telnet-output", OutputEvent { session_id: sid.clone(), data });
                        }
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(TelnetCmd::Data(bytes)) => { let _ = write_half.write_all(&bytes).await; }
                        Some(TelnetCmd::Close) | None => break,
                    }
                }
            }
        }
        if let Ok(mut sessions) = telnet_store().lock() {
            sessions.remove(&sid);
        }
        let _ = app.emit("telnet-exit", OutputEvent { session_id: sid.clone(), data: String::new() });
    });
    Ok(session_id)
}

#[command]
pub fn telnet_write(session_id: String, data: String) -> Result<(), String> {
    let store = telnet_store().lock().map_err(|_| "telnet 锁失败".to_string())?;
    let s = store.get(&session_id).ok_or_else(|| "telnet 会话不存在".to_string())?;
    s.cmd_tx.send(TelnetCmd::Data(data.into_bytes())).map_err(|_| "telnet 已关闭".to_string())
}

/// 订阅 Telnet 原始字节流（GDB 等消费者用）。
pub(crate) fn telnet_subscribe(
    session_id: &str,
) -> Result<mpsc::UnboundedReceiver<Vec<u8>>, String> {
    let store = telnet_store().lock().map_err(|_| "telnet 锁失败".to_string())?;
    let s = store.get(session_id).ok_or_else(|| "telnet 会话不存在".to_string())?;
    let (tx, rx) = mpsc::unbounded_channel();
    s.subs.lock().map_err(|_| "telnet 锁失败".to_string())?.push(tx);
    Ok(rx)
}

/// 往 Telnet 写原始字节（订阅者调用）。
pub(crate) fn telnet_write_bytes(session_id: &str, data: &[u8]) -> Result<(), String> {
    let store = telnet_store().lock().map_err(|_| "telnet 锁失败".to_string())?;
    let s = store.get(session_id).ok_or_else(|| "telnet 会话不存在".to_string())?;
    s.cmd_tx.send(TelnetCmd::Data(data.to_vec())).map_err(|_| "telnet 已关闭".to_string())
}

#[command]
pub fn telnet_close(session_id: String) -> Result<(), String> {
    if let Some(s) = telnet_store().lock().map_err(|_| "telnet 锁失败".to_string())?.remove(&session_id) {
        let _ = s.cmd_tx.send(TelnetCmd::Close);
    }
    Ok(())
}
