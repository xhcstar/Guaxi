//! 终端调试工坊 —— 交互式 GDB/MI 调试（批5，K 模块）。
//!
//! 通过已建好的 SSH 连接（复用 ssh_terminal 的连接/认证）开一个 exec 通道，
//! 以 `gdb -i=mi` 启动远端 gdb，拿结构化的 MI 输出。MI 记录被解析成 JSON：
//! - 结果记录 `token^done,...`      → 按 token 路由回对应命令的应答
//! - 异步记录 `*stopped` / `=...`   → 作为事件推前端（触发线程/watch 刷新）
//! - 流记录 `~"..."` / `&"..."`     → 控制台/日志文本，推前端并落盘录制（K2）
//! 前端在此之上实现结构体树(K3/K4)、watch(K5)、命令板(K6)、宏(K7)、断点(K8)、
//! 线程(K9)、内存(K10)、源码(K11)——它们本质都是发特定 MI 命令 + 渲染其 JSON 应答。

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{command, AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Notify};
use uuid::Uuid;

use crate::commands::ssh_terminal::connect_authenticated;

const GDB_OUTPUT_EVENT: &str = "gdb-mi-output";
const GDB_ASYNC_EVENT: &str = "gdb-mi-async";
const GDB_EXIT_EVENT: &str = "gdb-mi-exit";

/// gdb 未在这么多秒内就绪（su 认证 + gdb 启动）就判定为失败，把排队的命令以明确错误回掉。
const READY_TIMEOUT_SECS: u64 = 45;

enum GdbCmd {
    /// 发送一条带 token 的 MI 命令。应答不在这里回传，而是由会话任务存进 `done`，
    /// 前端用 `gdb_mi_await` 取——这样**前端超时/刷新都不会丢应答**。
    Mi { token: u64, line: String },
    /// 发送一行原始文本（交互输入 / 目标进程 stdin）。
    Raw(String),
    Close,
}

struct GdbSession {
    cmd_tx: mpsc::UnboundedSender<GdbCmd>,
    token: AtomicU64,
    /// 已完成的应答（token → 应答体）。迟到的结果留在这里等前端来取，不丢。
    done: Arc<Mutex<HashMap<u64, Value>>>,
    /// 有新应答时唤醒所有正在 `await` 的等待者。
    notify: Arc<Notify>,
    /// 会话是否仍存活；置 false 时所有等待者应立即收到"已断开"而不是干等到超时。
    alive: Arc<AtomicBool>,
    /// gdb 的 MI 是否已就绪（su 认证 / gdb 启动完成）。
    ready: Arc<AtomicBool>,
}

type SessionMap = HashMap<String, Arc<GdbSession>>;
static GDB_SESSIONS: OnceLock<Mutex<SessionMap>> = OnceLock::new();

/// `done` 最多保留的应答条数；超出丢最旧的，防止前端长期不取导致无界增长。
const DONE_CAP: usize = 512;

fn session_store() -> &'static Mutex<SessionMap> {
    GDB_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 当前活跃调试会话数（设置页「服务器」用）。
pub fn active_session_count() -> usize {
    session_store().lock().map(|guard| guard.len()).unwrap_or(0)
}

/// 按 id 取会话句柄。
fn get_session(session_id: &str) -> Result<Arc<GdbSession>, String> {
    session_store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .get(session_id)
        .cloned()
        .ok_or_else(|| "gdb 会话不存在（可能已结束）".to_string())
}

/// 结束一个会话：标记不存活并唤醒等待者，让在途的 await 立刻返回而不是等满超时。
fn finish_session(s: &GdbSession) {
    s.alive.store(false, Ordering::SeqCst);
    s.notify.notify_waiters();
}

pub fn shutdown_all_sessions() {
    if let Ok(mut g) = session_store().lock() {
        for (_id, s) in g.drain() {
            finish_session(&s);
            let _ = s.cmd_tx.send(GdbCmd::Close);
        }
    }
}

// ============================ MI 解析 ============================

struct MiParser {
    chars: Vec<char>,
    i: usize,
}

impl MiParser {
    fn new(s: &str) -> Self {
        MiParser { chars: s.chars().collect(), i: 0 }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.i).copied();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn parse_cstring(&mut self) -> String {
        let mut out = String::new();
        // 已确保当前是 '"'
        self.bump();
        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(e) = self.bump() {
                        out.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            other => other,
                        });
                    }
                }
                other => out.push(other),
            }
        }
        out
    }
    fn parse_ident(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                out.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        out
    }
    fn parse_value(&mut self) -> Value {
        match self.peek() {
            Some('"') => Value::String(self.parse_cstring()),
            Some('{') => self.parse_tuple(),
            Some('[') => self.parse_list(),
            _ => Value::String(self.parse_ident()),
        }
    }
    fn parse_tuple(&mut self) -> Value {
        self.bump(); // {
        let mut obj = serde_json::Map::new();
        loop {
            match self.peek() {
                Some('}') => {
                    self.bump();
                    break;
                }
                None => break,
                Some(',') => {
                    self.bump();
                }
                _ => {
                    let name = self.parse_ident();
                    if self.peek() == Some('=') {
                        self.bump();
                        obj.insert(name, self.parse_value());
                    } else {
                        // 没有 name= 的裸值，跳过一个 value 防死循环
                        let _ = self.parse_value();
                    }
                }
            }
        }
        Value::Object(obj)
    }
    fn parse_list(&mut self) -> Value {
        self.bump(); // [
        let mut arr = Vec::new();
        loop {
            match self.peek() {
                Some(']') => {
                    self.bump();
                    break;
                }
                None => break,
                Some(',') => {
                    self.bump();
                }
                Some('"') | Some('{') | Some('[') => arr.push(self.parse_value()),
                _ => {
                    // 可能是 name=value 形式（如 frame={...}）；丢名保值，得到同构数组
                    let _name = self.parse_ident();
                    if self.peek() == Some('=') {
                        self.bump();
                        arr.push(self.parse_value());
                    } else if !_name.is_empty() {
                        arr.push(Value::String(_name));
                    } else {
                        self.bump();
                    }
                }
            }
        }
        Value::Array(arr)
    }
    fn parse_results(&mut self) -> Value {
        let mut obj = serde_json::Map::new();
        while self.peek().is_some() {
            if self.peek() == Some(',') {
                self.bump();
                continue;
            }
            let name = self.parse_ident();
            if name.is_empty() {
                break;
            }
            if self.peek() == Some('=') {
                self.bump();
                obj.insert(name, self.parse_value());
            }
        }
        Value::Object(obj)
    }
}

// ============================ 记录分类 ============================

enum Record {
    Stream { kind: &'static str, text: String },
    Result { token: Option<u64>, class: String, payload: Value },
    Async { class: String, payload: Value },
    Ignore,
}

/// 是否是 PTY 回显行——即"我们自己写进去的命令"被 tty 回显出来的副本。
///
/// MI 记录的形态是 `<token><记录符>...`，记录符必为 `^ * + = ~ @ &` 之一；
/// 回显则是 `<token><原始命令文本>`（如 `25-stack-list-frames`）或裸命令行。
/// 两者之别就在 token 后的那个字符，据此判定。
fn is_echo_line(t: &str) -> bool {
    if t.is_empty() {
        return false;
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    match t[digits.len()..].chars().next() {
        Some(c) if "^*+=~@&".contains(c) => false,
        _ => true,
    }
}

fn classify(line: &str) -> Record {
    let t = line.trim_end();
    if t.is_empty() || t == "(gdb)" || t == "(gdb) " {
        return Record::Ignore;
    }
    let mut p = MiParser::new(t);
    // 前导 token（数字）
    let mut token_str = String::new();
    while let Some(c) = p.peek() {
        if c.is_ascii_digit() {
            token_str.push(c);
            p.i += 1;
        } else {
            break;
        }
    }
    let token = token_str.parse::<u64>().ok();
    match p.bump() {
        Some('~') => Record::Stream { kind: "console", text: read_stream(&mut p) },
        Some('@') => Record::Stream { kind: "target", text: read_stream(&mut p) },
        Some('&') => Record::Stream { kind: "log", text: read_stream(&mut p) },
        Some('^') => {
            let class = p.parse_ident();
            let payload = if p.peek() == Some(',') {
                p.bump();
                p.parse_results()
            } else {
                Value::Object(serde_json::Map::new())
            };
            Record::Result { token, class, payload }
        }
        Some('*') | Some('+') | Some('=') => {
            let class = p.parse_ident();
            let payload = if p.peek() == Some(',') {
                p.bump();
                p.parse_results()
            } else {
                Value::Object(serde_json::Map::new())
            };
            Record::Async { class, payload }
        }
        _ => Record::Ignore,
    }
}

fn read_stream(p: &mut MiParser) -> String {
    if p.peek() == Some('"') {
        p.parse_cstring()
    } else {
        String::new()
    }
}

fn gdb_rec_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?
        .join("remote_terminal")
        .join("gdb_recordings");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 gdb 录制目录失败: {e}"))?;
    Ok(dir)
}

// ============================ 会话驱动（K1/K2） ============================

/// GDB 的传输方式。
///
/// SSH 是常态；串口/Telnet 用于调试网络相关程序——那些机器只有串口控制台
/// （或只能 telnet 到串口服务器），此时没有 SSH 可用。
enum GdbTransport {
    Ssh {
        channel: russh::Channel<russh::client::Msg>,
        /// su 提权密码（仅 PTY 场景）。
        pending_password: Option<String>,
    },
    Serial { rx: mpsc::UnboundedReceiver<Vec<u8>>, session_id: String },
    Telnet { rx: mpsc::UnboundedReceiver<Vec<u8>>, session_id: String },
}

impl GdbTransport {
    async fn send(&mut self, text: &str) -> Result<(), String> {
        match self {
            GdbTransport::Ssh { channel, .. } => channel
                .data(text.as_bytes())
                .await
                .map_err(|e| format!("写 SSH 通道失败: {e}")),
            GdbTransport::Serial { session_id, .. } => {
                crate::commands::serial_telnet::serial_write_bytes(session_id, text.as_bytes())
            }
            GdbTransport::Telnet { session_id, .. } => {
                crate::commands::serial_telnet::telnet_write_bytes(session_id, text.as_bytes())
            }
        }
    }
}

/// 从当前传输读一段原始字节；连接结束返回 None。
async fn recv_bytes(t: &mut GdbTransport) -> Option<Vec<u8>> {
    match t {
        GdbTransport::Ssh { channel, .. } => loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Data { data }) => return Some(data.to_vec()),
                Some(russh::ChannelMsg::ExtendedData { data, .. }) => return Some(data.to_vec()),
                Some(russh::ChannelMsg::ExitStatus { .. }) | None => return None,
                Some(_) => continue,
            }
        },
        GdbTransport::Serial { rx, .. } | GdbTransport::Telnet { rx, .. } => rx.recv().await,
    }
}

#[command]
#[allow(clippy::too_many_arguments)]
pub async fn gdb_mi_start(
    app: AppHandle,
    profile_id: Option<String>,
    program: Option<String>,
    args: Option<Vec<String>>,
    attach_pid: Option<u32>,
    use_sudo: Option<bool>,
    escalate: Option<String>,
    escalate_password: Option<String>,
    transport: Option<String>,
    serial_session_id: Option<String>,
    telnet_session_id: Option<String>,
) -> Result<String, String> {
    let kind = transport.unwrap_or_else(|| "ssh".to_string());
    let esc = escalate
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| if use_sudo.unwrap_or(false) { "sudo".into() } else { "none".into() });

    // 先拼 gdb 核心命令：
    //  - 附加进程：gdb -q -nx -i=mi [program]（不在命令行用 `-p PID`，由前端 -target-attach 发起）。
    //  - 启动程序：gdb -q -nx -i=mi --args program args...
    let mut core = String::from("gdb -q -nx -i=mi");
    if attach_pid.is_some() {
        if let Some(prog) = program.as_ref().filter(|s| !s.trim().is_empty()) {
            core.push(' ');
            core.push_str(&shell_quote(prog));
        }
    } else if let Some(prog) = program.as_ref().filter(|s| !s.trim().is_empty()) {
        core.push_str(" --args ");
        core.push_str(&shell_quote(prog));
        if let Some(a) = args {
            for arg in a {
                core.push(' ');
                core.push_str(&shell_quote(&arg));
            }
        }
    }

    // SSH 时必须把连接 handle 保持到会话结束；串口/Telnet 没有 handle。
    let mut ssh_handle: Option<russh::client::Handle<crate::commands::ssh_terminal::ClientHandler>> = None;
    let (mut transport, need_pty, esc) = if kind == "serial" || kind == "telnet" {
        // 串口/Telnet 上登录的是一次性 shell；没有 sudo/su 的概念（本就是直接控制台）。
        let sid = if kind == "serial" {
            serial_session_id.ok_or("串口调试需要先连接一个串口会话")?
        } else {
            telnet_session_id.ok_or("Telnet 调试需要先连接一个 Telnet 会话")?
        };
        let rx = if kind == "serial" {
            crate::commands::serial_telnet::serial_subscribe(&sid)?
        } else {
            crate::commands::serial_telnet::telnet_subscribe(&sid)?
        };
        let t = if kind == "serial" {
            GdbTransport::Serial { rx, session_id: sid }
        } else {
            GdbTransport::Telnet { rx, session_id: sid }
        };
        (t, false, "none".to_string())
    } else {
        let pid = profile_id
            .filter(|s| !s.trim().is_empty())
            .ok_or("SSH 调试需要选择连接档案")?;
        let handle = connect_authenticated(&app, &pid).await?;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开 gdb 通道失败: {e}"))?;
        ssh_handle = Some(handle);
        let need = esc == "sudo" || esc == "su";
        let pw = if esc == "su" {
            escalate_password.filter(|s| !s.is_empty())
        } else {
            None
        };
        (
            GdbTransport::Ssh { channel, pending_password: pw },
            need,
            esc.clone(),
        )
    };

    // 按提权方式包裹（仅 SSH）：sudo -n 免密，su 需要 PTY 喂密码。
    let need_pty = need_pty;
    let core = if need_pty {
        format!("stty -echo 2>/dev/null; exec {core}")
    } else {
        core
    };
    let cmd = match esc.as_str() {
        "sudo" => format!("sudo -n sh -c {}", shell_quote(&core)),
        "su" => format!("su -c {}", shell_quote(&core)),
        _ => core,
    };
    let mut pw_to_send: Option<String> = match &mut transport {
        GdbTransport::Ssh { pending_password, .. } => pending_password.take(),
        _ => None,
    };
    if let GdbTransport::Ssh { channel, .. } = &mut transport {
        if need_pty {
            let _ = channel.request_pty(false, "xterm", 200, 50, 0, 0, &[]).await;
        }
        channel
            .exec(true, cmd.as_bytes())
            .await
            .map_err(|e| format!("启动 gdb 失败: {e}"))?;
    } else {
        // 串口/Telnet：先发一条空行确认控制台在用，再发 gdb 命令。
        // 串口设备通常已有登录 shell（root），直接执行即可。
        let _ = transport.send("\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        transport
            .send(&format!("{cmd}\n"))
            .await
            .map_err(|e| format!("在远端控制台启动 gdb 失败: {e}"))?;
    }
    // 录制文件（K2）
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let rec_path = gdb_rec_dir(&app)?.join(format!("gdb-{ts}.log"));
    let mut recorder: Option<BufWriter<File>> = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rec_path)
        .ok()
        .map(BufWriter::new);

    let session_id = Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<GdbCmd>();
    let done: Arc<Mutex<HashMap<u64, Value>>> = Arc::new(Mutex::new(HashMap::new()));
    let notify = Arc::new(Notify::new());
    let alive = Arc::new(AtomicBool::new(true));
    let ready_flag = Arc::new(AtomicBool::new(false));
    session_store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .insert(
            session_id.clone(),
            Arc::new(GdbSession {
                cmd_tx: tx,
                token: AtomicU64::new(1),
                done: done.clone(),
                notify: notify.clone(),
                alive: alive.clone(),
                ready: ready_flag.clone(),
            }),
        );

    let sid = session_id.clone();
    tokio::spawn(async move {
        // 尚未就绪（su 认证 / gdb 启动中）时先存这里，就绪后补发。
        let mut queued: Vec<(u64, String)> = Vec::new();
        let mut linebuf = String::new();
        // 直到 gdb 的 MI 真正就绪（打印出 `(gdb)` 提示或任一 MI 记录）才下发命令。
        // 否则在 su 认证/gdb 启动期间过早写入的 MI 命令会和密码输入抢同一个 PTY，导致 attach 丢失/超时。
        let mut ready = false;
        // 就绪超时：su 密码错 / 远端没有 gdb / 登录脚本卡住时，不能一直静默排队。
        // 触发一次后置 None（不再重触发）。
        let mut ready_deadline: Option<Instant> = Some(Instant::now() + Duration::from_secs(READY_TIMEOUT_SECS));
        loop {
            // 就绪哨兵：只在尚未就绪时计时，避免已经就绪后还在跑定时器。
            let deadline_snapshot = ready_deadline;
            let ready_timer = async move {
                match deadline_snapshot {
                    Some(d) => tokio::time::sleep_until(tokio::time::Instant::from_std(d)).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                _ = ready_timer => {
                    ready_deadline = None;
                    if !ready {
                        // 把排队的命令全部用明确错误回掉，别让前端无限等待。
                        let msg = format!(
                            "gdb 在 {READY_TIMEOUT_SECS}s 内未就绪：可能是 su/sudo 密码错误、远端未安装 gdb、或登录脚本卡住。请看下方输出排查。"
                        );
                        let mut d = done.lock().unwrap();
                        for (token, _) in queued.drain(..) {
                            d.insert(token, json!({ "class": "error", "payload": { "msg": msg } }));
                        }
                        drop(d);
                        notify.notify_waiters();
                        let _ = app.emit(
                            GDB_ASYNC_EVENT,
                            json!({ "session_id": sid, "class": "notready", "payload": { "msg": msg } }),
                        );
                    }
                }
                data = recv_bytes(&mut transport) => {
                    match data {
                        Some(data) => {
                            if let Some(w) = recorder.as_mut() {
                                let _ = w.write_all(&data);
                                let _ = w.flush();
                            }
                            linebuf.push_str(&String::from_utf8_lossy(&data));
                            while let Some(pos) = linebuf.find('\n') {
                                let line: String = linebuf.drain(..=pos).collect();
                                let t = line.trim();
                                // 丢掉 PTY 回显行：`<token>命令文本`。真 MI 记录的 token 后必然紧跟
                                // 记录符 ^ * + = ~ @ &，否则就是 tty 把我们写进去的命令回显了一遍。
                                if is_echo_line(t) {
                                    continue;
                                }
                                // gdb MI 就绪探测：`(gdb)` 提示或 token 后首字符是 MI 记录符。
                                if !ready {
                                    let body = t.trim_start_matches(|c: char| c.is_ascii_digit());
                                    if t == "(gdb)"
                                        || body.starts_with(|c: char| "^*+=~@&".contains(c))
                                    {
                                        ready = true;
                                        ready_flag.store(true, Ordering::SeqCst);
                                        ready_deadline = None;
                                        let _ = app.emit(
                                            GDB_ASYNC_EVENT,
                                            json!({ "session_id": sid, "class": "ready", "payload": {} }),
                                        );
                                    }
                                }
                                handle_line(line.trim_end(), &done, &notify, &app, &sid);
                            }
                            // su 的密码提示不带换行，会留在 linebuf 尾部；见到提示就喂一次密码。
                            if let Some(pw) = pw_to_send.take() {
                                let low = linebuf.to_lowercase();
                                if low.contains("password") || linebuf.contains("口令") || linebuf.contains("密码") {
                                    let _ = transport.send(&format!("{pw}\n")).await;
                                    linebuf.clear();
                                } else {
                                    pw_to_send = Some(pw);
                                }
                            }
                            // gdb 就绪后，把排队的命令按顺序补发。
                            if ready && !queued.is_empty() {
                                for (token, line) in queued.drain(..) {
                                    let _ = transport.send(&format!("{token}{line}\n")).await;
                                }
                            }
                        }
                        None => break,
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(GdbCmd::Mi { token, line }) => {
                            if ready {
                                let _ = transport.send(&format!("{token}{line}\n")).await;
                            } else {
                                // gdb 尚未就绪（可能在 su 认证中）：先排队，就绪后再发。
                                queued.push((token, line));
                            }
                        }
                        Some(GdbCmd::Raw(text)) => {
                            let _ = transport.send(&text).await;
                        }
                        Some(GdbCmd::Close) | None => break,
                    }
                }
            }
        }
        if let Ok(mut sessions) = session_store().lock() {
            sessions.remove(&sid);
        }
        // 会话收尾：标记不存活 → 唤醒所有等待者（它们会立刻收到"已断开"而不是干等到超时），
        // 并把还没被取走的应答清掉。
        alive.store(false, Ordering::SeqCst);
        notify.notify_waiters();
        if let Ok(mut d) = done.lock() {
            d.clear();
        }
        let _ = app.emit(GDB_EXIT_EVENT, json!({ "session_id": sid }));
        // SSH 通道要显式 eof；串口/Telnet 不关闭（那是用户的控制台会话，留着继续用）。
        if let GdbTransport::Ssh { channel, .. } = &mut transport {
            let _ = channel.eof().await;
        }
        // 保持 SSH handle 存活至会话结束——否则连接会随 gdb_mi_start 返回而关闭。
        drop(ssh_handle);
    });

    Ok(session_id)
}

fn handle_line(
    line: &str,
    done: &Arc<Mutex<HashMap<u64, Value>>>,
    notify: &Arc<Notify>,
    app: &AppHandle,
    session_id: &str,
) {
    match classify(line) {
        Record::Stream { kind, text } => {
            let _ = app.emit(
                GDB_OUTPUT_EVENT,
                json!({ "session_id": session_id, "kind": kind, "text": text }),
            );
        }
        Record::Result { token, class, payload } => {
            // 只要带 token，就是某条命令的应答：一律存进 `done` 等前端来取。
            // 绝不能因为"当前没有人在等"就当成异步事件推给前端——那会把迟到的
            // ^done 伪装成 *stopped 之类的通知，让界面错乱。
            if let Some(tok) = token {
                if let Ok(mut d) = done.lock() {
                    if d.len() >= DONE_CAP {
                        // 简单防膨胀：清掉一半（都是前端已放弃的旧应答）。
                        let keys: Vec<u64> = d.keys().take(DONE_CAP / 2).copied().collect();
                        for k in keys {
                            d.remove(&k);
                        }
                    }
                    d.insert(tok, json!({ "class": class, "payload": payload }));
                }
                notify.notify_waiters();
                return;
            }
            // 无 token 的结果记录当作通知推前端
            let _ = app.emit(
                GDB_ASYNC_EVENT,
                json!({ "session_id": session_id, "class": class, "payload": payload }),
            );
        }
        Record::Async { class, payload } => {
            let _ = app.emit(
                GDB_ASYNC_EVENT,
                json!({ "session_id": session_id, "class": class, "payload": payload }),
            );
        }
        Record::Ignore => {
            // 非 MI 文本（sudo 报错、gdb 启动错误、权限提示等）照样推到控制台，便于诊断——
            // 否则这些关键失败信息会被静默吞掉。排除空行、gdb 提示符与我们自己回显的命令行。
            let t = line.trim();
            if !t.is_empty()
                && t != "(gdb)"
                && !t.starts_with('(')
                && !(t.starts_with('-') && !t.starts_with("-->"))
            {
                let _ = app.emit(
                    GDB_OUTPUT_EVENT,
                    json!({ "session_id": session_id, "kind": "log", "text": format!("{t}\n") }),
                );
            }
        }
    }
}

/// 极简 shell 转义：用单引号包裹，内部单引号用 '\'' 序列。
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// MI 里的双引号字符串转义。
fn mi_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// MI 命令以 '-' 开头直接下发；否则视为 CLI，包进 -interpreter-exec console。
fn normalize_command(command: &str) -> String {
    let c = command.trim();
    if c.starts_with('-') {
        c.to_string()
    } else {
        format!("-interpreter-exec console {}", mi_quote(c))
    }
}

/// K1a：**异步提交**一条命令（MI 或 CLI），立刻返回 token；应答用 `gdb_mi_await` 取。
///
/// 这是本模块的关键设计：命令与应答解耦，前端的等待时长完全由自己掌握——
/// 对端慢（加载符号、链路卡）时不会被后端的固定超时掐断，迟到的应答也不会丢。
#[command]
pub fn gdb_mi_submit(session_id: String, command: String) -> Result<u64, String> {
    let session = get_session(&session_id)?;
    let token = session.token.fetch_add(1, Ordering::SeqCst);
    let line = normalize_command(&command);
    session
        .cmd_tx
        .send(GdbCmd::Mi { token, line })
        .map_err(|_| "gdb 会话已关闭".to_string())?;
    Ok(token)
}

/// K1b：等待某 token 的应答，最多等 `wait_ms`（默认 1s，上限 60s）。
///
/// - 返回 `Some(应答体)`：拿到了（取走后不再保留）。
/// - 返回 `None`：还在路上，调用方可以继续等（前端据此刷新"已等待 Ns"并允许取消）。
/// - 返回 `Err(...)`：会话已结束/已断开——调用方应立即停止等待，而不是等满超时。
#[command]
pub async fn gdb_mi_await(
    session_id: String,
    token: u64,
    wait_ms: Option<u64>,
) -> Result<Option<Value>, String> {
    let session = get_session(&session_id)?;
    let budget = Duration::from_millis(wait_ms.unwrap_or(1000).min(60_000));
    let deadline = Instant::now() + budget;
    loop {
        if let Ok(mut d) = session.done.lock() {
            if let Some(v) = d.remove(&token) {
                return Ok(Some(v));
            }
        }
        // 会话已结束：立刻告知，别让前端干等到自己的上限。
        if !session.alive.load(Ordering::SeqCst) {
            return Err("会话已结束（gdb 进程已退出或连接断开）".to_string());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(None);
        }
        // 有新应答会被唤醒；否则睡到本次预算用尽。
        let _ = tokio::time::timeout(remaining, session.notify.notified()).await;
    }
}

/// 便捷包装：提交 + 等满 timeout_ms（默认 60s）。保留给一次性调用的场景。
#[command]
pub async fn gdb_mi_command(
    session_id: String,
    command: String,
    timeout_ms: Option<u64>,
) -> Result<Value, String> {
    let token = gdb_mi_submit(session_id.clone(), command)?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.unwrap_or(60_000));
    loop {
        match gdb_mi_await(session_id.clone(), token, Some(1000)).await? {
            Some(v) => return Ok(v),
            None => {
                if Instant::now() >= deadline {
                    return Err("gdb 命令超时".to_string());
                }
            }
        }
    }
}

/// 发送原始文本（交互输入 / 目标进程 stdin）。
#[command]
pub fn gdb_mi_raw(session_id: String, text: String) -> Result<(), String> {
    let session = get_session(&session_id)?;
    session.cmd_tx.send(GdbCmd::Raw(text)).map_err(|_| "gdb 会话已关闭".to_string())
}

#[command]
pub fn gdb_mi_close(session_id: String) -> Result<(), String> {
    let session = {
        let mut store = session_store().lock().map_err(|_| "会话锁失败".to_string())?;
        store.remove(&session_id)
    };
    if let Some(s) = session {
        // 先唤醒等待者：正在 `gdb_mi_await` 的前端应立刻收到"已结束"，而不是等满自己的上限。
        finish_session(&s);
        let _ = s.cmd_tx.send(GdbCmd::Close);
    }
    Ok(())
}

/// 会话状态快照，供前端渲染"正在建立 / 等待应答 / 已结束"等提示。
#[derive(Serialize)]
pub struct GdbSessionState {
    pub alive: bool,
    pub ready: bool,
    /// 尚未被取走的应答条数（可粗略反映"对端积压"）。
    pub pending_results: usize,
}

#[command]
pub fn gdb_mi_state(session_id: String) -> Result<GdbSessionState, String> {
    let s = get_session(&session_id)?;
    Ok(GdbSessionState {
        alive: s.alive.load(Ordering::SeqCst),
        ready: s.ready.load(Ordering::SeqCst),
        pending_results: s.done.lock().map(|d| d.len()).unwrap_or(0),
    })
}

// ============================ 调试宏（K7） ============================

#[derive(Serialize, Deserialize)]
pub struct GdbMacro {
    pub name: String,
    pub commands: Vec<String>,
}

#[command]
pub fn gdb_save_macros(path: String, macros: Vec<GdbMacro>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&macros).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("保存宏失败: {e}"))
}

#[command]
pub fn gdb_load_macros(path: String) -> Result<Vec<GdbMacro>, String> {
    let data = std::fs::read_to_string(&path).map_err(|e| format!("读取宏失败: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("解析宏失败: {e}"))
}
