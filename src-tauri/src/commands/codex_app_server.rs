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
}

static SERVER: LazyLock<Mutex<Option<ServerHandle>>> = LazyLock::new(|| Mutex::new(None));

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
}

#[derive(Deserialize, Debug, Default)]
pub struct TurnStartRequest {
    pub thread_id: String,
    pub text: String,
    #[serde(default)]
    pub model: String,
    /// low | medium | high | xhigh | max | ultra
    #[serde(default)]
    pub effort: String,
    /// 本地图片路径（「操作电脑」模式把截图交给模型）。
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Serialize, Debug, Default)]
pub struct ThreadSummary {
    pub thread_id: String,
    pub cwd: String,
    pub model: String,
    pub sandbox: String,
    pub approval_policy: String,
    pub path: String,
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
    let id = next_id();
    let (tx, rx) = oneshot::channel();
    PENDING_RPC
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id, tx);

    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
    sender_line()?.send(line.to_string()).map_err(|e| e.to_string())?;

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
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
    {
        let guard = SERVER.lock().map_err(|e| e.to_string())?;
        if let Some(h) = guard.as_ref() {
            if sender_alive(h) {
                return Ok(h.user_agent.clone());
            }
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
    !h.tx.is_closed()
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
    let payload = msg.get("params").cloned().unwrap_or(Value::Null);
    let _ = app.emit(
        EVENT_NOTIFICATION,
        json!({
            "kind": method,
            "payload": payload,
            "at": chrono::Utc::now().timestamp_millis(),
        }),
    );
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
    if let Ok(guard) = SERVER.lock() {
        if let Some(h) = guard.as_ref() {
            status.running = sender_alive(h);
            status.pid = h.pid;
            status.user_agent = h.user_agent.clone();
        }
    }
    status
}

/// 可用模型清单。`model/list` 不需要供应商凭证（实测），所以即使还没配 Key 也能拉出来，
/// 设置页模型下拉可以直接用这份数据。
#[tauri::command]
pub async fn agent_list_models(app: AppHandle) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("model/list", json!({})).await
}

/// 开一条新线程。
#[tauri::command]
pub async fn agent_start_thread(
    app: AppHandle,
    request: ThreadStartRequest,
) -> Result<ThreadSummary, String> {
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

    let resp = rpc_call("thread/start", params).await?;
    let thread = resp.get("thread").cloned().unwrap_or(Value::Null);
    let summary = ThreadSummary {
        thread_id: thread
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        cwd: thread
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        model: resp
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        sandbox: sandbox_string(&resp),
        approval_policy: resp
            .get("approvalPolicy")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        path: thread
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    };
    info!(
        thread_id = %summary.thread_id,
        cwd = %summary.cwd,
        model = %summary.model,
        "[codex_app_server] thread/start"
    );
    Ok(summary)
}

/// 恢复既有线程（软件重启后接着用）。
#[tauri::command]
pub async fn agent_resume_thread(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("thread/resume", json!({ "threadId": thread_id })).await
}

/// 线程历史（会话恢复时补渲染用）。
#[tauri::command]
pub async fn agent_read_thread(app: AppHandle, thread_id: String) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call("thread/read", json!({ "threadId": thread_id })).await
}

/// 历史会话列表。直接吃 codex 自己的 rollout 索引，Guaxi 不另存一份会话正文。
#[tauri::command]
pub async fn agent_list_threads(app: AppHandle, limit: Option<u32>) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call(
        "thread/list",
        json!({ "limit": limit.unwrap_or(50), "sortKeys": ["recency_at"] }),
    )
    .await
}

/// 发一轮指令。
#[tauri::command]
pub async fn agent_start_turn(app: AppHandle, request: TurnStartRequest) -> Result<Value, String> {
    ensure_started(&app).await?;
    if request.thread_id.trim().is_empty() {
        return Err("缺少 thread_id".to_string());
    }
    if request.text.trim().is_empty() && request.images.is_empty() {
        return Err("输入为空".to_string());
    }

    let mut input: Vec<Value> = Vec::new();
    if !request.text.trim().is_empty() {
        input.push(json!({ "type": "text", "text": request.text }));
    }
    for path in &request.images {
        if path.trim().is_empty() {
            continue;
        }
        input.push(json!({ "type": "localImage", "path": path }));
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

    let resp = rpc_call("turn/start", params).await?;
    info!(
        thread_id = %request.thread_id,
        turn = %resp.pointer("/turn/id").and_then(|v| v.as_str()).unwrap_or(""),
        "[codex_app_server] turn/start"
    );
    Ok(resp)
}

/// 中途改指令（不打断当前回合）。
#[tauri::command]
pub async fn agent_steer_turn(
    app: AppHandle,
    thread_id: String,
    turn_id: String,
    text: String,
) -> Result<Value, String> {
    ensure_started(&app).await?;
    rpc_call(
        "turn/steer",
        json!({
            "threadId": thread_id,
            "turnId": turn_id,
            "input": [{ "type": "text", "text": text }]
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

fn sandbox_string(resp: &Value) -> String {
    resp.pointer("/sandbox/type")
        .and_then(|v| v.as_str())
        .unwrap_or("workspace-write")
        .to_string()
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
