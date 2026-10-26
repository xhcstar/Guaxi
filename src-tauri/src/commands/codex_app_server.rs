//! Codex App-Server 引擎 — 以常驻 JSON-RPC 子进程驱动 harness。
//!
//! 与 `codex_agent.rs` 的关系
//!
//! 两者驱动同一份 `codex.exe`、共用 `CODEX_HOME` 与生成的 `config.toml`，区别只在协议：
//!
//! - `codex_agent.rs` 走 `codex exec --json`：一轮起一个进程，发一句、收事件、进程退出。
//! - 本模块走 `codex app-server --stdio`：一个常驻进程，`thread/*` 管会话、`turn/*` 管轮次，
//!   审批是服务端发过来的请求，diff / 计划 / 用量是通知。
//!
//! 三条硬约束
//!
//! 1. **协议适配只在这里**。app-server 仍是 `[experimental]`，字段会随版本变；
//!    把它收敛到一个文件，前端只认 `agent://*` 事件，升级时只需改这一层。
//! 2. **不与用户自己的 codex 打架**。沿用 `codex_agent` 的二进制与私有 `CODEX_HOME` 解析。
//! 3. **失败要能降级**。进程起不来 / 握手失败，返回 Err，由调用方退到 `exec`，再退到 Python。
//!    已经进入事件流之后的错误不当 Err 抛，而是作为 error 通知发出去。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::commands::codex_agent;

/// 单向下行事件：item 开始/完成、正文增量、计划、diff、用量、错误。
pub const EVENT_NOTIFICATION: &str = "agent://notification";
/// 需要前端回 decision 的审批。
pub const EVENT_APPROVAL: &str = "agent://approval";

/// 审批等待上限。超时按拒绝处理——stdio 那头的 agent 会一直挂着等，
/// 不能让它无限期卡住一整轮。
const APPROVAL_TIMEOUT_SECS: u64 = 300;

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window(_cmd: &mut Command) {}

// ───────────────────────────── 进程状态

/// 已连接进程的写端。读端在独立的 tokio 任务里跑，不在这里持有。
struct ServerHandle {
    tx: mpsc::UnboundedSender<String>,
    pid: u32,
    user_agent: String,
    /// 必须持有 Child 本体。
    ///
    /// `Command::kill_on_drop(true)` 的语义是「这个 Child 值被释放时杀掉进程」。
    /// 之前只把 pid 存进全局状态、把 Child 留成局部变量，于是 ensure_started 一返回
    /// 就 drop 掉了它——进程立刻被杀，握手明明成功，下一次调用却报「app-server 未运行」。
    /// 现在句柄跟着全局状态活，进程才真正常驻。
    ///
    child: tokio::process::Child,
}

static SERVER: LazyLock<Mutex<Option<ServerHandle>>> = LazyLock::new(|| Mutex::new(None));

/// 启动串行化。
///
/// 工作台一次挂载就会并发触发 `agent_engine_status` / `agent_list_models` /
/// `agent_start_thread`，三者都走 `ensure_started`。没有这把锁时它们会各自
/// spawn 一个 app-server，后两次把前一次的句柄覆盖掉，被覆盖的那个进程随即
/// 因为 kill_on_drop 死掉——表现同样是「连上了但马上又未运行」。
static START_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 等待响应的 JSON-RPC 请求：id → 结果发送端。
/// 携带 `Result`，让错误响应用 `Err` 表达，调用方不会把 `{"__error":..}` 当成正常结果。
static PENDING_RPC: LazyLock<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 等待前端裁决的审批：request_id → 结果发送端。
static PENDING_APPROVALS: LazyLock<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

fn take_pending_rpc(id: u64) -> Option<oneshot::Sender<Result<Value, String>>> {
    PENDING_RPC.lock().ok()?.remove(&id)
}

fn sender_line() -> Result<mpsc::UnboundedSender<String>, String> {
    SERVER
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .map(|h| h.tx.clone())
        .ok_or_else(|| "codex app-server 未运行".to_string())
}

// ───────────────────────────── 请求 / 响应类型

#[derive(Deserialize, Debug, Default)]
pub struct ThreadStartRequest {
    /// 工作目录；空则用 codex-home/workspace。
    #[serde(default)]
    pub cwd: String,
    /// read-only | workspace-write | danger-full-access
    #[serde(default)]
    pub sandbox: String,
    /// untrusted | on-request | never
    #[serde(default)]
    pub approval_policy: String,
    #[serde(default)]
    pub model: String,
    /// 工作模式：explore | code | research | computer
    #[serde(default)]
    pub mode: String,
    /// 归入哪个会话分区（thread section）。
    #[serde(default)]
    pub section_id: String,
    /// 不落盘的临时线程（用于「快速问一句」）。
    #[serde(default)]
    pub ephemeral: bool,
}

/// 一条附加输入。与 `UserInput` 的几种类型一一对应：
/// text / mention / skill / localImage / image / localAudio / audio。
#[derive(Deserialize, Debug, Default, Clone)]
pub struct UserInputAttachment {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct TurnStartRequest {
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub model: String,
    /// low | medium | high | xhigh | max | ultra
    #[serde(default)]
    pub effort: String,
    /// auto | concise | detailed | none
    #[serde(default)]
    pub summary: String,
    /// 本轮覆盖的工作目录。
    #[serde(default)]
    pub cwd: String,
    /// 本轮覆盖的审批策略。
    #[serde(default)]
    pub approval_policy: String,
    /// 本轮覆盖的沙箱：read-only | workspace-write | danger-full-access
    #[serde(default)]
    pub sandbox: String,
    /// 本地图片路径（「操作电脑」模式把截图交给模型）。
    #[serde(default)]
    pub images: Vec<String>,
    /// 附件 / 文件引用 / 技能，按顺序插在正文之后。
    #[serde(default)]
    pub attachments: Vec<UserInputAttachment>,
    /// 客户端生成的消息 id，用于回流去重。
    #[serde(default)]
    pub client_user_message_id: String,
}

#[derive(Serialize, Debug, Default)]
pub struct EngineStatus {
    pub running: bool,
    pub pid: u32,
    pub user_agent: String,
    pub codex_home: String,
    pub binary_path: String,
    pub binary_exists: bool,
    pub binary_version: String,
    pub hint: String,
}

// ───────────────────────────── 底层读写

/// 发一条 JSON-RPC 请求并等响应。
async fn rpc_call(method: &str, params: Value) -> Result<Value, String> {
    rpc_call_with_timeout(method, params, 30).await
}

async fn rpc_call_with_timeout(
    method: &str,
    params: Value,
    timeout_secs: u64,
) -> Result<Value, String> {
    let id = next_id();
    let (tx, rx) = oneshot::channel();
    PENDING_RPC
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id, tx);

    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    sender_line()?.send(line.to_string()).map_err(|e| e.to_string())?;

    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => {
            take_pending_rpc(id);
            Err(format!("{method}: 连接已关闭"))
        }
        Err(_) => {
            take_pending_rpc(id);
            Err(format!("{method}: 等待响应超时"))
        }
    }
}

/// 发一条通知（不需要响应）。
fn rpc_notify(method: &str, params: Value) -> Result<(), String> {
    let line = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    sender_line()?.send(line.to_string()).map_err(|e| e.to_string())
}

/// 回复一个服务端发来的请求（审批、客户端工具调用）。
fn rpc_reply(id: u64, result: Value) {
    if let Ok(tx) = sender_line() {
        let line = json!({ "jsonrpc": "2.0", "id": id, "result": result });
        let _ = tx.send(line.to_string());
    }
}

fn rpc_reply_error(id: u64, message: String) {
    if let Ok(tx) = sender_line() {
        let line = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32000, "message": message }
        });
        let _ = tx.send(line.to_string());
    }
}

// ───────────────────────────── 进程启动 / 收尾

/// 启动并握手。已经启动且进程还活着时直接复用。
pub async fn ensure_started(app: &AppHandle) -> Result<String, String> {
    // 快路径：已经连上就不排队。
    {
        let mut guard = SERVER.lock().map_err(|e| e.to_string())?;
        if let Some(h) = guard.as_mut() {
            if sender_alive(h) && process_alive(h) && !h.user_agent.is_empty() {
                return Ok(h.user_agent.clone());
            }
        }
    }

    // 慢路径串行化：拿到锁后必须再查一次，排队期间别人可能已经连好了。
    let _start_guard = START_LOCK.lock().await;
    {
        let mut guard = SERVER.lock().map_err(|e| e.to_string())?;
        if let Some(h) = guard.as_mut() {
            if sender_alive(h) && process_alive(h) && !h.user_agent.is_empty() {
                return Ok(h.user_agent.clone());
            }
        }
        // 走到这里说明旧句柄已经不可用。先摘掉再重启，否则会留下一个
        // 指向死进程的条目；被覆盖的 Child 也会连带触发一次 kill。
        if let Some(stale) = guard.take() {
            warn!(
                pid = stale.pid,
                "[codex_app_server] 旧进程已退出，重新拉起"
            );
            crate::guaxiservices::process_shutdown::unregister_managed_pid(stale.pid);
        }
    }

    let binary = codex_agent::codex_binary_path()?;
    let home = codex_agent::codex_home_dir()?;
    if !home.exists() {
        std::fs::create_dir_all(&home).map_err(|e| format!("创建 codex-home 失败: {}", e))?;
    }

    // 供应商与沙箱配置复用 exec 链路那一份。两条链路读同一份 cc-switch / 自定义配置，
    // 在 cc-switch 里换供应商时不需要为 app-server 单独再改一次。
    let provider = codex_agent::resolve_provider_for_engine()?;
    let sandbox = codex_agent::resolve_sandbox_mode_for_engine();
    codex_agent::write_codex_config_for_engine(&home, &provider, &sandbox)?;

    let mut cmd = Command::new(&binary);
    cmd.args(["app-server", "--stdio"])
        .env("CODEX_HOME", &home)
        .env(codex_agent::CODEX_API_KEY_ENV, &provider.api_key)
        .current_dir(&home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    apply_no_window(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动 codex app-server 失败: {}", e))?;
    let pid = child.id().unwrap_or(0);
    crate::guaxiservices::process_shutdown::register_managed_pid("codex_app_server", pid);

    let stdin = child.stdin.take().ok_or("app-server stdin 不可用")?;
    let stdout = child.stdout.take().ok_or("app-server stdout 不可用")?;
    let stderr = child.stderr.take().ok_or("app-server stderr 不可用")?;

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    // 写任务：独占 stdin，避免多处写交错。
    tokio::spawn(async move {
        let mut stdin = stdin;
        while let Some(line) = rx.recv().await {
            if stdin.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if stdin.write_all(b"\n").await.is_err() {
                break;
            }
            let _ = stdin.flush().await;
        }
    });

    spawn_stderr_reader(stderr, pid);
    spawn_stdout_reader(app.clone(), stdout, pid);

    {
        let mut guard = SERVER.lock().map_err(|e| e.to_string())?;
        *guard = Some(ServerHandle {
            tx,
            pid,
            user_agent: String::new(),
            // 把句柄交给全局状态：这是进程能活过本函数返回的关键。
            child,
        });
    }

    // 握手。experimentalApi 是 dynamicTools 的前置条件（实测：不开会直接报
    // `thread/start.dynamicTools requires experimentalApi capability`），
    // 工作台要把截图交给模型就必须声明它。
    let init = rpc_call(
        "initialize",
        json!({
            "clientInfo": { "name": "guaxi", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": true }
        }),
    )
    .await;

    let init = match init {
        Ok(v) => v,
        Err(e) => {
            stop();
            return Err(e);
        }
    };
    let user_agent = init
        .get("userAgent")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    if let Err(e) = rpc_notify("initialized", json!({})) {
        stop();
        return Err(e);
    }

    if let Ok(mut guard) = SERVER.lock() {
        if let Some(h) = guard.as_mut() {
            h.user_agent = user_agent.clone();
        }
    }

    info!(
        pid,
        ua = %user_agent,
        home = %home.to_string_lossy(),
        "[codex_app_server] 已连接"
    );
    Ok(user_agent)
}

fn sender_alive(h: &ServerHandle) -> bool {
    // 两条都看：写端可能还挂着，但进程已经退出（崩溃、被外部杀掉）。
    // 只看 tx 会误判成「已连接」，于是后续请求全部写进黑洞。
    !h.tx.is_closed()
}

/// 检查句柄对应的进程是否还活着。
///
/// `try_wait` 在进程已退出时返回 `Some(status)`。这里不改状态，
/// 真正的清理交给 `stdout` 读端（它退出时会按 pid 摘掉句柄）。
fn process_alive(h: &mut ServerHandle) -> bool {
    matches!(h.child.try_wait(), Ok(None))
}

/// 收掉进程（软件退出 / 手动重置）。
pub fn stop() {
    let handle = SERVER.lock().ok().and_then(|mut g| g.take());
    // 没拿到 lock 时也要尽力清理：进程残留比状态不一致更麻烦。
    if let Some(h) = handle {
        info!(pid = h.pid, "[codex_app_server] 停止");
        crate::guaxiservices::process_shutdown::terminate_process_tree(
            h.pid,
            "codex_app_server",
        );
        crate::guaxiservices::process_shutdown::unregister_managed_pid(h.pid);
    }
    // 挂着的请求一并失败，避免调用方一直等。
    if let Ok(mut m) = PENDING_RPC.lock() {
        m.clear();
    }
    if let Ok(mut m) = PENDING_APPROVALS.lock() {
        m.clear();
    }
}

fn spawn_stderr_reader(stderr: tokio::process::ChildStderr, pid: u32) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let text = line.trim();
            if text.is_empty() {
                continue;
            }
            debug!(pid, line = %text, "[codex_app_server] stderr");
        }
    });
}

fn spawn_stdout_reader(app: AppHandle, stdout: tokio::process::ChildStdout, pid: u32) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let text = line.trim();
            if text.is_empty() {
                continue;
            }
            let msg: Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(_) => {
                    debug!(line = %text, "[codex_app_server] 非 JSON 输出");
                    continue;
                }
            };
            dispatch_incoming(app.clone(), msg).await;
        }
        info!(pid, "[codex_app_server] stdout 关闭");
        // 读端结束意味着进程没了：清掉句柄，下一次 ensure_started 会重新拉起。
        if let Ok(mut guard) = SERVER.lock() {
            if guard.as_ref().map(|h| h.pid) == Some(pid) {
                *guard = None;
                crate::guaxiservices::process_shutdown::unregister_managed_pid(pid);
            }
        }
    });
}

/// 一条入站消息的三条去路：响应、服务端请求、通知。
async fn dispatch_incoming(app: AppHandle, msg: Value) {
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = msg.get("id").and_then(|v| v.as_u64());

    // 1) 对我们请求的响应
    if method.is_empty() {
        if let Some(id) = id {
            if let Some(tx) = take_pending_rpc(id) {
                if let Some(err) = msg.get("error") {
                    let text = err
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("未知错误")
                        .to_string();
                    warn!(id, error = %text, "[codex_app_server] 请求失败");
                    let _ = tx.send(Err(text));
                } else {
                    let _ = tx.send(Ok(msg.get("result").cloned().unwrap_or(Value::Null)));
                }
            }
        }
        return;
    }

    // 2) 服务端发来的请求（审批、客户端工具调用）。必须回包，否则那头的 agent 会挂住。
    if let Some(id) = id {
        handle_server_request(app, id, method, msg.get("params").cloned().unwrap_or(Value::Null))
            .await;
        return;
    }

    // 3) 通知：补齐 threadId/turnId 之外，原样透传，同时给出一个稳定的 kind 供前端分流。
    //    把 threadId 提到顶层很关键：工作台同时开着多个会话，事件必须能直接归账，
    //    否则前端要在每个 payload 里按类型去猜字段。
    let payload = msg.get("params").cloned().unwrap_or(Value::Null);
    let thread_id = notification_thread_id(&payload);
    let turn_id = payload
        .get("turnId")
        .and_then(|v| v.as_str())
        .or_else(|| payload.pointer("/turn/id").and_then(|v| v.as_str()))
        .unwrap_or("");
    let _ = app.emit(
        EVENT_NOTIFICATION,
        json!({
            "kind": method,
            "threadId": thread_id,
            "turnId": turn_id,
            "payload": payload,
            "at": chrono::Utc::now().timestamp_millis(),
        }),
    );
}

/// 从通知参数里取线程 id。`thread/started` 把线程放在 `thread` 里，其余大多直接在 `threadId`。
fn notification_thread_id(payload: &Value) -> String {
    payload
        .get("threadId")
        .and_then(|v| v.as_str())
        .or_else(|| payload.pointer("/thread/id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string()
}

async fn handle_server_request(app: AppHandle, id: u64, method: &str, params: Value) {
    match method {
        // 审批类：转发给前端，等裁决。
        "item/commandExecution/requestApproval"
        | "item/fileChange/requestApproval"
        | "item/permissions/requestApproval"
        | "item/tool/requestUserInput"
        | "mcpServer/elicitation/request"
        | "execCommandApproval"
        | "applyPatchApproval" => {
            let (tx, rx) = oneshot::channel();
            if let Ok(mut m) = PENDING_APPROVALS.lock() {
                m.insert(id, tx);
            }
            let kind = approval_kind(method);
            let _ = app.emit(
                EVENT_APPROVAL,
                json!({
                    "requestId": id,
                    "kind": kind,
                    "method": method,
                    "threadId": notification_thread_id(&params),
                    "turnId": params.get("turnId").and_then(|v| v.as_str()).unwrap_or(""),
                    "params": params,
                    "at": chrono::Utc::now().timestamp_millis(),
                }),
            );

            match tokio::time::timeout(
                std::time::Duration::from_secs(APPROVAL_TIMEOUT_SECS),
                rx,
            )
            .await
            {
                Ok(Ok(decision)) => rpc_reply(id, decision),
                _ => {
                    if let Ok(mut m) = PENDING_APPROVALS.lock() {
                        m.remove(&id);
                    }
                    warn!(id, method, "[codex_app_server] 审批超时，按拒绝处理");
                    rpc_reply(id, json!({ "decision": "decline" }));
                }
            }
        }
        // 客户端工具调用：模型要跑 Guaxi 侧的工具。走认知层同一套 dispatch，
        // 授权/审计/不可信闸都在那里，不在这里另开一条捷径。
        "item/tool/call" => {
            let tool = params.get("tool").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let call_id = params.get("callId").and_then(|v| v.as_str()).unwrap_or("");
            debug!(tool, call_id, "[codex_app_server] 客户端工具调用");
            let result = crate::cognition::tools::dispatch("codex", tool, &args, false).await;
            match result {
                Ok(v) => rpc_reply(
                    id,
                    json!({
                        "contentItems": [{
                            "type": "inputText",
                            "text": serde_json::to_string_pretty(&v).unwrap_or_default()
                        }],
                        "success": true
                    }),
                ),
                Err(e) => rpc_reply(
                    id,
                    json!({
                        "contentItems": [{ "type": "inputText", "text": format!("工具执行失败: {e}") }],
                        "success": false
                    }),
                ),
            }
        }
        // ChatGPT 账号态刷新、attestation：Guaxi 不用官方登录态（走 cc-switch / 自定义 Key），
        // 明确回错误而不是挂住。
        other => {
            warn!(method = other, "[codex_app_server] 未支持的服务端请求");
            rpc_reply_error(id, format!("Guaxi 不支持的服务端请求: {other}"));
        }
    }
}

/// 把 codex 的细粒度审批方法归成前端好处理的几类。
fn approval_kind(method: &str) -> &'static str {
    match method {
        "item/commandExecution/requestApproval" | "execCommandApproval" => "command",
        "item/fileChange/requestApproval" | "applyPatchApproval" => "file",
        "item/permissions/requestApproval" => "permission",
        "item/tool/requestUserInput" | "mcpServer/elicitation/request" => "input",
        _ => "other",
    }
}

// ───────────────────────────── 对外命令

/// app-server 引擎状态（设置页 / 工作台顶栏）。
#[tauri::command]
pub async fn agent_engine_status() -> EngineStatus {
    let mut status = EngineStatus::default();
    let (home, binary) = (codex_agent::codex_home_dir(), codex_agent::codex_binary_path());
    match home {
        Ok(h) => status.codex_home = h.to_string_lossy().to_string(),
        Err(e) => status.hint = e,
    }
    match binary {
        Ok(b) => {
            status.binary_exists = true;
            status.binary_path = b.to_string_lossy().to_string();
            status.binary_version = codex_agent::read_version(&b).await;
        }
        Err(e) => {
            status.binary_exists = false;
            if status.hint.is_empty() {
                status.hint = e;
            }
        }
    }
    if let Ok(mut guard) = SERVER.lock() {
        if let Some(h) = guard.as_mut() {
            // running 要按进程真实状态算，不能只看写端还在。
            // 否则进程崩了、句柄还没被读端摘掉时，界面会显示「已连接」。
            status.running = sender_alive(h) && process_alive(h);
            status.pid = h.pid;
            status.user_agent = h.user_agent.clone();
        }
    }
    status
}

/// 引擎当前是否可用：连上过、进程还活着、且没有待处理的启动错误。
/// 工作台用它决定「发送」按钮是否可点，避免点了才发现连不上。
#[tauri::command]
pub async fn agent_engine_ready() -> bool {
    let (alive, handshaked) = SERVER
        .lock()
        .ok()
        .and_then(|mut g| {
            g.as_mut()
                .map(|h| (sender_alive(h) && process_alive(h), !h.user_agent.is_empty()))
        })
        .unwrap_or((false, false));
    alive && handshaked
}

/// 可用模型清单。`model/list` 不需要供应商凭证（实测），所以即使还没配 Key 也能拉出来，
/// 设置页模型下拉可以直接用这份数据。
#[tauri::command]
pub async fn agent_list_models(app: AppHandle, include_hidden: Option<bool>) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({});
    if include_hidden.unwrap_or(false) {
        params["includeHidden"] = json!(true);
    }
    rpc_call("model/list", params).await
}

/// 开一条新线程。返回 thread/start 的完整响应：前端需要 thread.id、模型、
/// 实际生效的沙箱/审批、以及 instructionSources（Environment 面板要显示）。
#[tauri::command]
pub async fn agent_start_thread(
    app: AppHandle,
    request: ThreadStartRequest,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let home = codex_agent::codex_home_dir()?;
    let cwd = resolve_work_dir(&request.cwd, &home)?;
    let sandbox = normalize_sandbox(&request.sandbox);
    let approval = normalize_approval(&request.approval_policy);
    let (base, developer) = mode_instructions(&request.mode);

    let mut params = json!({
        "cwd": cwd.to_string_lossy(),
        "sandbox": sandbox,
        "approvalPolicy": approval,
    });
    if let Some(v) = base {
        params["baseInstructions"] = json!(v);
    }
    if let Some(v) = developer {
        params["developerInstructions"] = json!(v);
    }
    if !request.model.trim().is_empty() {
        params["model"] = json!(request.model.trim());
    }
    if request.ephemeral {
        params["ephemeral"] = json!(true);
    }

    let resp = rpc_call("thread/start", params).await?;
    let thread_id = resp
        .pointer("/thread/id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // section 归属在 thread 创建之后单独设：thread/start 不接受 section 字段。
    if !request.section_id.trim().is_empty() && !thread_id.is_empty() {
        let _ = rpc_call(
            "thread/section/move",
            json!({ "threadId": thread_id, "sectionId": request.section_id.trim() }),
        )
        .await;
    }

    info!(
        thread_id = %thread_id,
        cwd = %cwd.to_string_lossy(),
        model = resp.get("model").and_then(|v| v.as_str()).unwrap_or(""),
        "[codex_app_server] thread/start"
    );
    Ok(resp)
}

/// 恢复既有线程（软件重启 / 切换会话后接着用）。
#[tauri::command]
pub async fn agent_resume_thread(
    app: AppHandle,
    thread_id: String,
    cwd: Option<String>,
    model: Option<String>,
    sandbox: Option<String>,
    approval_policy: Option<String>,
    mode: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "threadId": thread_id });
    if let Some(cwd) = cwd.filter(|v| !v.trim().is_empty()) {
        params["cwd"] = json!(cwd);
    }
    if let Some(model) = model.filter(|v| !v.trim().is_empty()) {
        params["model"] = json!(model);
    }
    if let Some(sandbox) = sandbox.filter(|v| !v.trim().is_empty()) {
        params["sandbox"] = json!(normalize_sandbox(&sandbox));
    }
    if let Some(approval) = approval_policy.filter(|v| !v.trim().is_empty()) {
        params["approvalPolicy"] = json!(normalize_approval(&approval));
    }
    let (base, developer) = mode_instructions(mode.as_deref().unwrap_or(""));
    if let Some(v) = base {
        params["baseInstructions"] = json!(v);
    }
    if let Some(v) = developer {
        params["developerInstructions"] = json!(v);
    }
    rpc_call_with_timeout("thread/resume", params, 60).await
}

/// 线程历史（会话恢复时补渲染用）。
#[tauri::command]
pub async fn agent_read_thread(
    app: AppHandle,
    thread_id: String,
    include_turns: Option<bool>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call_with_timeout(
        "thread/read",
        json!({ "threadId": thread_id, "includeTurns": include_turns.unwrap_or(true) }),
        60,
    )
    .await
}

/// 历史会话列表。直接吃 codex 自己的 rollout 索引，Guaxi 不另存一份会话正文。
///
/// `params` 原样透传 thread/list 的过滤条件（archived / sectionId / searchTerm /
/// cwd / cursor / sortKey …），前端只需要理解协议，不需要为每个筛选开一条命令。
#[tauri::command]
pub async fn agent_list_threads(app: AppHandle, params: Option<Value>) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut p = params.unwrap_or_else(|| json!({}));
    if !p.is_object() {
        p = json!({});
    }
    if p.get("limit").is_none() {
        p["limit"] = json!(50);
    }
    if p.get("sortKey").is_none() {
        p["sortKey"] = json!("recency_at");
    }
    rpc_call_with_timeout("thread/list", p, 60).await
}

/// 从某个节点分叉（Codex 的「从这里重新开始」）。
#[tauri::command]
pub async fn agent_fork_thread(
    app: AppHandle,
    thread_id: String,
    last_turn_id: Option<String>,
    cwd: Option<String>,
    model: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "threadId": thread_id });
    if let Some(t) = last_turn_id.filter(|v| !v.trim().is_empty()) {
        params["lastTurnId"] = json!(t);
    }
    if let Some(c) = cwd.filter(|v| !v.trim().is_empty()) {
        params["cwd"] = json!(c);
    }
    if let Some(m) = model.filter(|v| !v.trim().is_empty()) {
        params["model"] = json!(m);
    }
    rpc_call_with_timeout("thread/fork", params, 60).await
}

/// 发一轮指令。
#[tauri::command]
pub async fn agent_start_turn(app: AppHandle, request: TurnStartRequest) -> Result<Value, String> {
    ensure_started(&app).await?;
    if request.thread_id.trim().is_empty() {
        return Err("缺少 thread_id".to_string());
    }

    let input = build_turn_input(&request);
    if input.is_empty() {
        return Err("输入为空".to_string());
    }

    let mut params = json!({
        "threadId": request.thread_id,
        "input": input,
    });
    if !request.model.trim().is_empty() {
        params["model"] = json!(request.model.trim());
    }
    if !request.effort.trim().is_empty() {
        params["effort"] = json!(request.effort.trim());
    }
    if !request.summary.trim().is_empty() {
        params["summary"] = json!(request.summary.trim());
    }
    if !request.cwd.trim().is_empty() {
        params["cwd"] = json!(request.cwd.trim());
    }
    if !request.approval_policy.trim().is_empty() {
        params["approvalPolicy"] = json!(normalize_approval(&request.approval_policy));
    }
    if !request.sandbox.trim().is_empty() {
        if let Some(policy) = sandbox_policy_value(&request.sandbox, &request.cwd) {
            params["sandboxPolicy"] = policy;
        }
    }
    if !request.client_user_message_id.trim().is_empty() {
        params["clientUserMessageId"] = json!(request.client_user_message_id.trim());
    }

    let resp = rpc_call("turn/start", params).await?;
    info!(
        thread_id = %request.thread_id,
        turn = %resp.pointer("/turn/id").and_then(|v| v.as_str()).unwrap_or(""),
        "[codex_app_server] turn/start"
    );
    Ok(resp)
}

/// 把正文、附件、文件引用、技能拼成 `UserInput[]`。
///
/// 顺序很重要：正文在前，引用在后。模型读到的第一条永远是用户真正说的话，
/// 而不是一串路径。
fn build_turn_input(request: &TurnStartRequest) -> Vec<Value> {
    let mut input: Vec<Value> = Vec::new();
    if !request.text.trim().is_empty() {
        input.push(json!({ "type": "text", "text": request.text, "text_elements": [] }));
    }

    for attachment in &request.attachments {
        let kind = attachment.kind.trim();
        let value = match kind {
            "mention" => {
                if attachment.path.trim().is_empty() {
                    continue;
                }
                json!({ "type": "mention", "name": attachment.name, "path": attachment.path })
            }
            "skill" => {
                if attachment.path.trim().is_empty() {
                    continue;
                }
                json!({ "type": "skill", "name": attachment.name, "path": attachment.path })
            }
            "image" => {
                if attachment.url.trim().is_empty() {
                    continue;
                }
                let mut v = json!({ "type": "image", "url": attachment.url });
                if !attachment.detail.trim().is_empty() {
                    v["detail"] = json!(attachment.detail.trim());
                }
                v
            }
            "localImage" => {
                if attachment.path.trim().is_empty() {
                    continue;
                }
                let mut v = json!({ "type": "localImage", "path": attachment.path });
                if !attachment.detail.trim().is_empty() {
                    v["detail"] = json!(attachment.detail.trim());
                }
                v
            }
            "localAudio" => {
                if attachment.path.trim().is_empty() {
                    continue;
                }
                json!({ "type": "localAudio", "path": attachment.path })
            }
            "audio" => {
                if attachment.url.trim().is_empty() {
                    continue;
                }
                json!({ "type": "audio", "url": attachment.url })
            }
            _ => {
                if attachment.text.trim().is_empty() {
                    continue;
                }
                json!({ "type": "text", "text": attachment.text, "text_elements": [] })
            }
        };
        input.push(value);
    }

    for path in &request.images {
        if path.trim().is_empty() {
            continue;
        }
        input.push(json!({ "type": "localImage", "path": path }));
    }
    input
}

/// 中途改指令（不打断当前回合）。
#[tauri::command]
pub async fn agent_steer_turn(
    app: AppHandle,
    thread_id: String,
    turn_id: String,
    text: String,
    attachments: Option<Vec<UserInputAttachment>>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut input: Vec<Value> = Vec::new();
    if !text.trim().is_empty() {
        input.push(json!({ "type": "text", "text": text, "text_elements": [] }));
    }
    for attachment in attachments.unwrap_or_default() {
        let request = TurnStartRequest {
            attachments: vec![attachment],
            ..Default::default()
        };
        input.extend(build_turn_input(&request));
    }
    if input.is_empty() {
        return Err("输入为空".to_string());
    }
    rpc_call(
        "turn/steer",
        json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "input": input
        }),
    )
    .await
}

/// 中断当前回合。
#[tauri::command]
pub async fn agent_interrupt_turn(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("turn/interrupt", json!({ "threadId": thread_id })).await
}

/// 回一条审批裁决。前端把 decision 原样送回来（结构随 kind 不同），
/// 这里只负责把它转给正在等待的那个请求。
#[tauri::command]
pub fn agent_resolve_approval(request_id: u64, decision: Value) -> Result<(), String> {
    let tx = PENDING_APPROVALS
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&request_id);
    match tx {
        Some(tx) => tx.send(decision).map_err(|_| "审批请求已关闭".to_string()),
        None => Err(format!("没有等待中的审批请求: {request_id}")),
    }
}

/// 重命名线程（会话栏用）。
#[tauri::command]
pub async fn agent_rename_thread(
    app: AppHandle,
    thread_id: String,
    name: String,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call(
        "thread/name/set",
        json!({ "threadId": thread_id, "name": name }),
    )
    .await
}

/// 归档线程。
#[tauri::command]
pub async fn agent_archive_thread(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("thread/archive", json!({ "threadId": thread_id })).await
}

/// 取消归档。
#[tauri::command]
pub async fn agent_unarchive_thread(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("thread/unarchive", json!({ "threadId": thread_id })).await
}

/// 永久删除线程。调用方（会话栏）必须先做二次确认。
#[tauri::command]
pub async fn agent_delete_thread(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("thread/delete", json!({ "threadId": thread_id })).await
}

/// 压缩会话上下文（Codex 的 compact）。
#[tauri::command]
pub async fn agent_compact_thread(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call_with_timeout(
        "thread/compact/start",
        json!({ "threadId": thread_id }),
        120,
    )
    .await
}

/// 起一次代码审查。
#[tauri::command]
pub async fn agent_review_start(
    app: AppHandle,
    thread_id: String,
    target: Value,
    delivery: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "threadId": thread_id, "target": target });
    if let Some(d) = delivery.filter(|v| !v.trim().is_empty()) {
        params["delivery"] = json!(d);
    }
    rpc_call_with_timeout("review/start", params, 120).await
}

// ───────────────────────────── 会话分区 / 环境 / 搜索

/// 分区（Codex 左侧的会话分组）列表。
#[tauri::command]
pub async fn agent_section_list(app: AppHandle, limit: Option<u32>) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call(
        "threadSection/list",
        json!({ "limit": limit.unwrap_or(100) }),
    )
    .await
}

#[tauri::command]
pub async fn agent_section_create(
    app: AppHandle,
    name: String,
    icon: Option<String>,
    color: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "name": name });
    let appearance = json!({ "icon": icon, "color": color });
    if appearance["icon"].is_string() || appearance["color"].is_string() {
        params["appearance"] = appearance;
    }
    rpc_call("threadSection/create", params).await
}

#[tauri::command]
pub async fn agent_section_update(
    app: AppHandle,
    section_id: String,
    name: String,
    icon: Option<String>,
    color: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "sectionId": section_id, "name": name });
    let appearance = json!({ "icon": icon, "color": color });
    if appearance["icon"].is_string() || appearance["color"].is_string() {
        params["appearance"] = appearance;
    }
    rpc_call("threadSection/update", params).await
}

#[tauri::command]
pub async fn agent_section_delete(app: AppHandle, section_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("threadSection/delete", json!({ "sectionId": section_id })).await
}

/// 把线程移入 / 移出分区，或调整分区内顺序。
#[tauri::command]
pub async fn agent_thread_move_section(
    app: AppHandle,
    thread_id: String,
    section_id: Option<String>,
    before_thread_id: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({
        "threadId": thread_id,
        "sectionId": section_id.filter(|v| !v.is_empty()),
    });
    if let Some(before) = before_thread_id.filter(|v| !v.is_empty()) {
        params["beforeThreadId"] = json!(before);
    }
    rpc_call("thread/section/move", params).await
}

/// 权限预设（read-only / workspace / full access）。
#[tauri::command]
pub async fn agent_list_permission_profiles(app: AppHandle) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("permissionProfile/list", json!({})).await
}

/// MCP 连接器状态。`detail` 传 "toolsAndAuthOnly" 时不拉完整 schema，开面板更快。
#[tauri::command]
pub async fn agent_list_mcp_servers(
    app: AppHandle,
    thread_id: Option<String>,
    detail: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "detail": detail.unwrap_or_else(|| "toolsAndAuthOnly".into()) });
    if let Some(t) = thread_id.filter(|v| !v.is_empty()) {
        params["threadId"] = json!(t);
    }
    rpc_call_with_timeout("mcpServerStatus/list", params, 60).await
}

/// 重载 MCP 配置（用户在设置里改了工具授权后点一下）。
#[tauri::command]
pub async fn agent_reload_mcp_servers(app: AppHandle) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call_with_timeout("config/mcpServer/reload", Value::Null, 60).await
}

/// 技能清单（Codex 的 skills）。工作台用来做 `/` 命令面板的补全。
#[tauri::command]
pub async fn agent_list_skills(
    app: AppHandle,
    cwds: Option<Vec<String>>,
    force_reload: Option<bool>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    let cwds = cwds.unwrap_or_default();
    rpc_call_with_timeout(
        "skills/list",
        json!({ "cwds": cwds, "forceReload": force_reload.unwrap_or(false) }),
        60,
    )
    .await
}

/// 文件提及（`@`）用的模糊搜索。
#[tauri::command]
pub async fn agent_fuzzy_file_search(
    app: AppHandle,
    query: String,
    roots: Vec<String>,
    cancellation_token: Option<String>,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call(
        "fuzzyFileSearch",
        json!({
            "query": query,
            "roots": roots,
            "cancellationToken": cancellation_token,
        }),
    )
    .await
}

/// 引擎实际生效的配置（Environment 面板展示 + 排障）。
#[tauri::command]
pub async fn agent_read_config(app: AppHandle, cwd: Option<String>) -> Result<Value, String> {
    ensure_started(&app).await?;
    let mut params = json!({ "includeLayers": false });
    if let Some(c) = cwd.filter(|v| !v.is_empty()) {
        params["cwd"] = json!(c);
    }
    rpc_call("config/read", params).await
}

/// 工作目录的 git 概览。Changes 面板据此显示分支与未提交行数。
#[tauri::command]
pub async fn agent_git_status(cwd: String) -> Result<Value, String> {
    let dir = resolve_work_dir(&cwd, &codex_agent::codex_home_dir()?)?;
    let inside = run_git(&dir, &["rev-parse", "--is-inside-work-tree"]).await;
    if !matches!(&inside, Ok(out) if out.trim() == "true") {
        return Ok(json!({ "isRepo": false }));
    }
    let branch = run_git(&dir, &["branch", "--show-current"])
        .await
        .unwrap_or_default()
        .trim()
        .to_string();
    let porcelain = run_git(&dir, &["status", "--porcelain=v1"])
        .await
        .unwrap_or_default();
    let (mut added, mut removed, mut files) = (0i64, 0i64, 0i64);
    for line in porcelain.lines().filter(|l| !l.trim().is_empty()) {
        files += 1;
        let path = line.get(3..).unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        let unstaged = run_git(&dir, &["diff", "--numstat", "--", path])
            .await
            .unwrap_or_default();
        let staged = run_git(&dir, &["diff", "--cached", "--numstat", "--", path])
            .await
            .unwrap_or_default();
        for text in [unstaged, staged] {
            for row in text.lines() {
                let mut cols = row.split('\t');
                if let (Some(a), Some(r)) = (cols.next(), cols.next()) {
                    added += a.trim().parse::<i64>().unwrap_or(0);
                    removed += r.trim().parse::<i64>().unwrap_or(0);
                }
            }
        }
    }
    Ok(json!({
        "isRepo": true,
        "branch": branch,
        "files": files,
        "added": added,
        "removed": removed,
        "dirty": files > 0,
    }))
}

/// 把工作区改动提交到 git。调用方必须已经让用户确认过提交信息。
#[tauri::command]
pub async fn agent_commit_changes(
    cwd: String,
    message: String,
    paths: Option<Vec<String>>,
) -> Result<Value, String> {
    if message.trim().is_empty() {
        return Err("提交信息不能为空".to_string());
    }
    let dir = resolve_work_dir(&cwd, &codex_agent::codex_home_dir()?)?;
    let mut add_args: Vec<String> = vec!["add".to_string()];
    match paths {
        Some(list) if !list.is_empty() => {
            add_args.push("--".to_string());
            add_args.extend(list);
        }
        _ => add_args.push("-A".to_string()),
    }
    let add_refs: Vec<&str> = add_args.iter().map(|s| s.as_str()).collect();
    run_git(&dir, &add_refs).await?;
    let output = run_git(&dir, &["commit", "-m", message.trim()]).await?;

    let head = run_git(&dir, &["rev-parse", "--short", "HEAD"])
        .await
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(json!({ "commit": head, "output": output }))
}

/// 跑一条 git 命令并取 stdout。非零退出码转成带 stderr 的错误。
async fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("执行 git 失败: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

/// 主动重置连接（设置页「重启引擎」）。
#[tauri::command]
pub async fn agent_restart_engine(app: AppHandle) -> Result<EngineStatus, String> {
    stop();
    ensure_started(&app).await?;
    Ok(agent_engine_status().await)
}

// ───────────────────────────── 参数归一

fn resolve_work_dir(raw: &str, home: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let trimmed = raw.trim();
    if !trimmed.is_empty() {
        let p = std::path::PathBuf::from(trimmed);
        if p.is_dir() {
            return Ok(p);
        }
        return Err(format!("工作目录不存在: {}", p.to_string_lossy()));
    }
    let scratch = home.join("workspace");
    if !scratch.exists() {
        std::fs::create_dir_all(&scratch)
            .map_err(|e| format!("创建 app-server 工作目录失败: {}", e))?;
    }
    Ok(scratch)
}

fn normalize_sandbox(raw: &str) -> &'static str {
    match raw.trim() {
        "read-only" | "readOnly" => "read-only",
        "danger-full-access" | "dangerFullAccess" => "danger-full-access",
        _ => "workspace-write",
    }
}

fn normalize_approval(raw: &str) -> &'static str {
    match raw.trim() {
        "untrusted" => "untrusted",
        "never" => "never",
        _ => "on-request",
    }
}

/// 把界面的沙箱模式转成 `TurnStartParams.sandboxPolicy` 的对象形式。
///
/// turn/start 只接受完整 policy，不接受模式字符串；workspace-write 还需要显式
/// 给出可写根目录，否则 codex 会按全局配置而不是当前工作目录来算。
fn sandbox_policy_value(mode: &str, cwd: &str) -> Option<Value> {
    match normalize_sandbox(mode) {
        "read-only" => Some(json!({ "type": "readOnly", "networkAccess": false })),
        "danger-full-access" => Some(json!({ "type": "dangerFullAccess" })),
        _ => {
            let root = if cwd.trim().is_empty() {
                String::new()
            } else {
                cwd.trim().to_string()
            };
            let mut roots = Vec::new();
            if !root.is_empty() {
                roots.push(root);
            }
            Some(json!({
                "type": "workspaceWrite",
                "writableRoots": roots,
                "networkAccess": true,
                "excludeTmpdirEnvVar": false,
                "excludeSlashTmp": false,
            }))
        }
    }
}

/// 工作模式 → (baseInstructions, developerInstructions)。
///
/// 这两种指令是 app-server 的原生字段，比在提示词前面拼一段前缀可靠：
/// 它们不占用户输入的上下文，也不会被模型当成用户说的话。
fn mode_instructions(mode: &str) -> (Option<String>, Option<String>) {
    match mode.trim() {
        "explore" => (
            Some(
                "你是一个探索型助手。优先理解与检索，不修改用户文件。\
                 回答必须给出依据（文件路径、片段编号或链接）。"
                    .to_string(),
            ),
            None,
        ),
        "code" => (
            None,
            Some(
                "工作方式：先读代码再改；改动尽量小且集中；改完运行相关的测试或构建；\
                 最后用 diff 说明你改了什么、为什么。不要顺手重构无关代码。"
                    .to_string(),
            ),
        ),
        "research" => (
            Some(
                "你是一个研究型助手。先调 guaxi_recall 看用户自己的资料，再联网检索补充；\
                 区分「用户资料里说的」与「网上的说法」，并各自给出处。"
                    .to_string(),
            ),
            None,
        ),
        "computer" => (
            Some(
                "你在协助用户操作这台电脑。每次动作前先用截图确认当前屏幕状态，\
                 依据带编号的网格或 OCR 坐标决定点击位置；一步一确认，不要连续盲操作。"
                    .to_string(),
            ),
            None,
        ),
        _ => (None, None),
    }
}
