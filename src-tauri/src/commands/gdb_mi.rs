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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{command, AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::commands::ssh_terminal::connect_authenticated;

const GDB_OUTPUT_EVENT: &str = "gdb-mi-output";
const GDB_ASYNC_EVENT: &str = "gdb-mi-async";
const GDB_EXIT_EVENT: &str = "gdb-mi-exit";

enum GdbCmd {
    /// 发送一条带 token 的 MI 命令，结果记录经 oneshot 回传。
    Mi { token: u64, line: String, reply: oneshot::Sender<Value> },
    /// 发送一行原始文本（交互输入 / 目标进程 stdin）。
    Raw(String),
    Close,
}

struct GdbSession {
    cmd_tx: mpsc::UnboundedSender<GdbCmd>,
    token: AtomicU64,
}

type SessionMap = HashMap<String, Arc<GdbSession>>;
static GDB_SESSIONS: OnceLock<Mutex<SessionMap>> = OnceLock::new();

fn session_store() -> &'static Mutex<SessionMap> {
    GDB_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn shutdown_all_sessions() {
    if let Ok(mut g) = session_store().lock() {
        for (_id, s) in g.drain() {
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

#[command]
pub async fn gdb_mi_start(
    app: AppHandle,
    profile_id: String,
    program: Option<String>,
    args: Option<Vec<String>>,
) -> Result<String, String> {
    let handle = connect_authenticated(&app, &profile_id).await?;
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("打开 gdb 通道失败: {e}"))?;

    // 组装远端命令：gdb -q -nx -i=mi [--args program args...]
    let mut cmd = String::from("gdb -q -nx -i=mi");
    if let Some(prog) = program.as_ref().filter(|s| !s.trim().is_empty()) {
        cmd.push_str(" --args ");
        cmd.push_str(&shell_quote(prog));
        if let Some(a) = args {
            for arg in a {
                cmd.push(' ');
                cmd.push_str(&shell_quote(&arg));
            }
        }
    }
    channel
        .exec(true, cmd.as_bytes())
        .await
        .map_err(|e| format!("启动 gdb 失败: {e}"))?;

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
    session_store()
        .lock()
        .map_err(|_| "会话锁失败".to_string())?
        .insert(
            session_id.clone(),
            Arc::new(GdbSession { cmd_tx: tx, token: AtomicU64::new(1) }),
        );

    let sid = session_id.clone();
    tokio::spawn(async move {
        let mut pending: HashMap<u64, oneshot::Sender<Value>> = HashMap::new();
        let mut linebuf = String::new();
        loop {
            tokio::select! {
                msg = channel.wait() => {
                    match msg {
                        Some(russh::ChannelMsg::Data { ref data })
                        | Some(russh::ChannelMsg::ExtendedData { ref data, .. }) => {
                            if let Some(w) = recorder.as_mut() {
                                let _ = w.write_all(data);
                                let _ = w.flush();
                            }
                            linebuf.push_str(&String::from_utf8_lossy(data));
                            while let Some(pos) = linebuf.find('\n') {
                                let line: String = linebuf.drain(..=pos).collect();
                                handle_line(line.trim_end(), &mut pending, &app, &sid);
                            }
                        }
                        Some(russh::ChannelMsg::ExitStatus { .. }) | None => break,
                        _ => {}
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(GdbCmd::Mi { token, line, reply }) => {
                            pending.insert(token, reply);
                            let s = format!("{token}{line}\n");
                            let _ = channel.data(s.as_bytes()).await;
                        }
                        Some(GdbCmd::Raw(text)) => {
                            let _ = channel.data(text.as_bytes()).await;
                        }
                        Some(GdbCmd::Close) | None => break,
                    }
                }
            }
        }
        if let Ok(mut sessions) = session_store().lock() {
            sessions.remove(&sid);
        }
        let _ = app.emit(GDB_EXIT_EVENT, json!({ "session_id": sid }));
        let _ = channel.eof().await;
        // 保持 SSH handle 存活至会话结束——否则连接会随 gdb_mi_start 返回而关闭。
        drop(handle);
    });

    Ok(session_id)
}

fn handle_line(
    line: &str,
    pending: &mut HashMap<u64, oneshot::Sender<Value>>,
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
            if let Some(tok) = token {
                if let Some(sender) = pending.remove(&tok) {
                    let _ = sender.send(json!({ "class": class, "payload": payload }));
                    return;
                }
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
        Record::Ignore => {}
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

/// K1：下发一条命令（MI 或 CLI），返回结构化结果记录 {class, payload}。
#[command]
pub async fn gdb_mi_command(session_id: String, command: String) -> Result<Value, String> {
    let session = {
        let store = session_store().lock().map_err(|_| "会话锁失败".to_string())?;
        store.get(&session_id).cloned()
    }
    .ok_or_else(|| "gdb 会话不存在".to_string())?;

    let token = session.token.fetch_add(1, Ordering::SeqCst);
    let (reply_tx, reply_rx) = oneshot::channel();
    let line = normalize_command(&command);
    session
        .cmd_tx
        .send(GdbCmd::Mi { token, line, reply: reply_tx })
        .map_err(|_| "gdb 会话已关闭".to_string())?;

    match tokio::time::timeout(Duration::from_secs(30), reply_rx).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(_)) => Err("gdb 未返回结果".to_string()),
        Err(_) => Err("gdb 命令超时".to_string()),
    }
}

/// 发送原始文本（交互输入 / 目标进程 stdin）。
#[command]
pub fn gdb_mi_raw(session_id: String, text: String) -> Result<(), String> {
    let session = {
        let store = session_store().lock().map_err(|_| "会话锁失败".to_string())?;
        store.get(&session_id).cloned()
    }
    .ok_or_else(|| "gdb 会话不存在".to_string())?;
    session.cmd_tx.send(GdbCmd::Raw(text)).map_err(|_| "gdb 会话已关闭".to_string())
}

#[command]
pub fn gdb_mi_close(session_id: String) -> Result<(), String> {
    let session = {
        let mut store = session_store().lock().map_err(|_| "会话锁失败".to_string())?;
        store.remove(&session_id)
    };
    if let Some(s) = session {
        let _ = s.cmd_tx.send(GdbCmd::Close);
    }
    Ok(())
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
