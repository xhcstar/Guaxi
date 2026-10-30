//! Codex Agent 引擎 — 以 sidecar 方式驱动 openai/codex
//!
//! 设计说明
//!
//! 1. **为什么是子进程而不是库**：crates.io 上没有官方的 codex 库（`codex-core` 未发布，
//!    `codex-protocol` 是第三方 fork 且版本落后），`codex-rs` 也没有稳定的嵌入 API。
//!    所以把 codex 可执行文件作为 Tauri sidecar 打进安装包，用 `codex exec --json` 驱动。
//! 2. **与用户机器上的 codex 完全隔离**：只认打包进来的二进制（不查 PATH、不查
//!    `%LOCALAPPDATA%\OpenAI\Codex`），并把 `CODEX_HOME` 指向 Guaxi 自己的配置目录
//!    `<exe_dir>/config/codex-home`，再加 `--ignore-user-config` 双保险。
//!    用户的 `~/.codex/{config.toml,auth.json,sessions}` 全程只读都不读。
//! 3. **密钥不落盘**：供应商配置写进私有 `config.toml`，但 API Key 用 `env_key` 指向环境变量，
//!    只在派生子进程时传入，不写 `auth.json`。
//! 4. **事件复用**：把 codex 的 JSONL 事件翻译成智能助手已有的
//!    `modle-streaming-respond-output` / `agent-step` 事件，前端的工具卡片逻辑不用改。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{info, warn};

use crate::commands::settingview;

/// 传给 codex 的环境变量名，与生成的 config.toml 里 `env_key` 保持一致。
pub const CODEX_API_KEY_ENV: &str = "GUAXI_CODEX_API_KEY";
/// 生成的 config.toml 里的供应商标识，固定名字便于每次幂等重写。
const CODEX_PROVIDER_ID: &str = "guaxi";
/// 供应商未配置密钥时的占位值：cc-switch 代理会用上游真实密钥改写，这里只需非空。
const CODEX_API_KEY_PLACEHOLDER: &str = "guaxi-managed";

/// 正在运行的 codex 子进程。同一时刻只允许一个 Agent 会话，与前端"pending 时不能再发"一致。
static CODEX_CHILD: Mutex<Option<(u32, String)>> = Mutex::new(None);

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window(_cmd: &mut Command) {}

#[derive(Deserialize, Debug, Default)]
pub struct CodexAgentRequest {
    pub question: String,
    #[serde(default)]
    pub sessionid: String,
    /// 活动知识库根路径，作为 codex 的工作目录（沙箱可写根）。
    #[serde(default)]
    pub repo_path: String,
    /// 其余已配置知识库，通过 `--add-dir` 一起加入可写范围，支持跨库引用。
    #[serde(default)]
    pub repo_roots: Vec<String>,
    /// 上一轮的 codex thread id；有值时走 `codex exec resume` 续接上下文。
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub web_search_enabled: bool,
}

#[derive(Serialize, Debug, Default)]
pub struct CodexAgentStatus {
    pub binary_path: String,
    pub binary_exists: bool,
    pub version: String,
    pub codex_home: String,
    pub provider_base_url: String,
    pub provider_model: String,
    pub provider_ready: bool,
    pub sandbox: String,
    pub hint: String,
}

fn codex_file_name() -> &'static str {
    if cfg!(windows) {
        "codex.exe"
    } else {
        "codex"
    }
}

/// 定位随 Guaxi 一起分发的 codex 可执行文件。
///
/// 走 `utils/codex/`，与 `utils/llama`、`utils/sherpa-onnx` 同一套分发与解析机制
/// （`bundled_resources::ensure_relative_dir` 负责 dev/release 路径差异与外部分包解压）。
/// 刻意不回落到 PATH，也不去找 `%LOCALAPPDATA%\OpenAI\Codex`：
/// 用户机器上已有的 codex 与其配置必须完全不受影响。
pub fn codex_binary_path() -> Result<PathBuf, String> {
    let name = codex_file_name();
    let dir = crate::guaxiservices::bundled_resources::ensure_relative_dir("utils/codex")?;

    let direct = dir.join(name);
    if direct.is_file() {
        return Ok(direct);
    }
    // 压缩包解出来可能多一层目录，浅层递归找一下。
    if let Some(found) = find_file_by_name(&dir, name, 3) {
        return Ok(found);
    }

    Err(format!(
        "未找到随 Guaxi 分发的 codex 可执行文件（{}）。预期位置: {}。可执行 `npm run codex:fetch` 获取。",
        name,
        dir.join(name).to_string_lossy()
    ))
}

/// 在目录下浅层查找同名文件，容忍压缩包多出来的一层目录。
fn find_file_by_name(root: &PathBuf, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file_by_name(&path, name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

/// Guaxi 私有的 CODEX_HOME，放在软件配置目录下，与用户的 ~/.codex 互不影响。
pub fn codex_home_dir() -> Result<PathBuf, String> {
    let config_path = settingview::get_config_path();
    let config_dir = config_path
        .parent()
        .ok_or_else(|| "无法获取 Guaxi 配置目录".to_string())?;
    let home = config_dir.join("codex-home");
    if !home.exists() {
        std::fs::create_dir_all(&home)
            .map_err(|e| format!("创建 codex-home 目录失败: {}", e))?;
    }
    Ok(home)
}

/// codex 的模型供应商配置来源。
struct CodexProvider {
    base_url: String,
    api_key: String,
    model: String,
    source: &'static str,
}

/// 给 app-server 引擎看的一份供应商快照。
///
/// `codex exec` 与 `codex app-server` 是同一个二进制、同一份 `config.toml`，
/// 所以两条链路必须用完全相同的供应商解析与配置生成。这里把私有类型包一层，
/// 是为了让新引擎能复用而不用把 `CodexProvider` 的字段全部公开。
pub struct EngineProvider {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub source: &'static str,
}

pub fn resolve_provider_for_engine() -> Result<EngineProvider, String> {
    resolve_provider().map(|p| EngineProvider {
        base_url: p.base_url,
        api_key: p.api_key,
        model: p.model,
        source: p.source,
    })
}

pub fn resolve_sandbox_mode_for_engine() -> String {
    resolve_sandbox_mode()
}

pub fn write_codex_config_for_engine(
    home: &PathBuf,
    provider: &EngineProvider,
    sandbox: &str,
) -> Result<(), String> {
    let inner = CodexProvider {
        base_url: provider.base_url.clone(),
        api_key: provider.api_key.clone(),
        model: provider.model.clone(),
        source: provider.source,
    };
    write_codex_config(home, &inner, sandbox, "on-request")
}

/// 读取 codex 版本号（设置页状态面板）。
pub async fn read_version(binary: &PathBuf) -> String {
    read_codex_version(binary).await
}


/// 解析要交给 codex 的供应商：cc-switch 本地代理优先，其次自定义 OpenAI 兼容配置。
///
/// 具体规则与普通对话共用 `llm_provider::resolve_agent`，避免设置页看到的东西和
/// Agent 实际使用的东西对不上。wire_api 固定 Responses（codex 0.149 起不再支持 chat），
/// 设置页里的 `cc_switch_wire_api` 只影响 Guaxi 自己的普通对话链路。
fn resolve_provider() -> Result<CodexProvider, String> {
    let provider = crate::commands::llm_provider::resolve_agent()?;
    let api_key = if provider.api_key.is_empty() {
        CODEX_API_KEY_PLACEHOLDER.to_string()
    } else {
        provider.api_key
    };
    // 只用于写进 config.toml 的供应商展示名（`Guaxi (cc-switch)`），保持静态字符串。
    let source = match provider.source.as_str() {
        "cc-switch" => "cc-switch",
        "provider" => "provider",
        _ => "custom",
    };
    Ok(CodexProvider {
        base_url: provider.base_url,
        api_key,
        model: provider.model,
        source,
    })
}

/// 最小化的 TOML 字符串转义：配置值来自用户输入，必须转义反斜杠与引号。
fn toml_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

fn resolve_sandbox_mode() -> String {
    let raw = settingview::get_llm_config().codex_sandbox;
    match raw.trim() {
        "read-only" => "read-only".to_string(),
        "danger-full-access" => "danger-full-access".to_string(),
        // 未知值一律收敛到 workspace-write：既能让 codex 自主改知识库文件，
        // 又不会把整台机器暴露给模型。
        _ => "workspace-write".to_string(),
    }
}

/// 幂等重写 Guaxi 私有的 `codex-home/config.toml`。
///
/// 每次发起 Agent 之前都重写一遍，保证设置页改完立刻生效，也不需要做增量合并。
/// API Key 不写进文件，只通过 `env_key` 指向环境变量。
///
/// `wire_api` 固定 `responses`：codex 0.149 起明确移除了 `wire_api = "chat"`
/// （启动时直接报 "no longer supported" 并拒绝加载配置）。
///
/// `approval_policy` 决定 MCP 工具的审批行为（见 `mcp_server_toml` 的说明），
/// 所以这里必须把它一并传下去，而不是在段里写死一个值。
fn write_codex_config(
    home: &PathBuf,
    provider: &CodexProvider,
    sandbox: &str,
    approval_policy: &str,
) -> Result<(), String> {
    let content = format!(
        concat!(
            "# 本文件由 Guaxi 自动生成，每次启动 Agent 都会被覆盖，请不要手动编辑。\n",
            "# 它只作用于 Guaxi 内置的 codex，与用户自己的 ~/.codex 完全无关。\n",
            "model = \"{model}\"\n",
            "model_provider = \"{provider_id}\"\n",
            "approval_policy = \"{approval_policy}\"\n",
            "sandbox_mode = \"{sandbox}\"\n",
            "disable_response_storage = true\n",
            // codex 原生的联网搜索。cached 只吃索引、live 会真的打开网页。
            // 它的开关必须跟随 Guaxi 的「出网」授权：否则设置页关着出网，
            // codex 自己却仍在联网，授权就形同虚设。
            "web_search = \"{web_search}\"\n",
            // 不给推理摘要的话，工作台中间栏就只剩工具卡片，看不见模型在想什么。
            "model_reasoning_summary = \"auto\"\n",
            "\n",
            "[model_providers.{provider_id}]\n",
            "name = \"Guaxi ({source})\"\n",
            "base_url = \"{base_url}\"\n",
            "wire_api = \"responses\"\n",
            "env_key = \"{env_key}\"\n",
        ),
        model = toml_escape(&provider.model),
        provider_id = CODEX_PROVIDER_ID,
        approval_policy = approval_policy,
        sandbox = sandbox,
        web_search = if crate::cognition::tools::tier_granted(crate::cognition::tools::Tier::Network) {
            "live"
        } else {
            "disabled"
        },
        source = provider.source,
        base_url = toml_escape(&provider.base_url),
        env_key = CODEX_API_KEY_ENV,
    );

    // Guaxi 自己的 MCP 服务端：把知识库召回、图谱、感知收件箱、信念、任务
    // 这些应用能力交给 codex。没有它，codex 只能看见文件系统，
    // 于是"大脑"永远是文件形状的，问不了"我知识库里关于 X 的结论是什么"。
    //
    // 走 stdio 桥接进程而不是直连 HTTP：codex 的 mcp_servers 只支持派生子进程。
    // 桥接缺失时**不写**这一段——写了 codex 每轮都会尝试拉起一个不存在的进程并报错。
    let content = match mcp_server_toml(approval_policy) {
        Some(extra) => format!("{content}{extra}"),
        None => content,
    };

    let path = home.join("config.toml");
    std::fs::write(&path, content).map_err(|e| format!("写入 codex config.toml 失败: {}", e))?;
    info!(
        path = %path.to_string_lossy(),
        provider = provider.source,
        sandbox = sandbox,
        "[codex_agent] codex config.toml 已生成"
    );
    Ok(())
}

/// 生成 `[mcp_servers.guaxi]` 配置片段。
///
/// 三件事必须同时成立，工具才真的能调：
///
/// 1. **挂得上**。优先用独立桥接二进制；找不到时退到主程序自己的
///    `--mcp-server` 模式（开发构建里通常没有旁挂的 `guaxi_mcp.exe`）。
/// 2. **只列已授权的工具**。`enabled_tools` 直接把未授权的等级挡在模型视野外——
///    让模型看见它调不动的东西，只会换来一串失败重试。这里与 `tools::dispatch`
///    的服务端强拦是两层，不是重复：一层省 token 和误判，一层是真正的边界。
/// 3. **审批模式与 approval_policy 配套**。codex 0.149 对所有 MCP 调用都要求审批，
///    唯一例外是工具带 `readOnlyHint`（已由服务端 `tools/list` 提供）。
///    - `never`（exec 链路，没有 UI 接审批）→ `auto`：只读直接跑，写/GUI 被 codex 拦下。
///    - 有审批 UI 的 app-server → `prompt`：每次需要确认的调用都弹到前端。
///    不能写 `approve`：那等于把所有写权限都交给模型。
fn mcp_server_toml(approval_policy: &str) -> Option<String> {
    let bridge = crate::cognition::commands::bridge_binary_path();
    let (command, args) = if bridge.is_file() {
        (bridge.to_string_lossy().to_string(), "args = []")
    } else {
        let exe = std::env::current_exe().ok()?;
        (exe.to_string_lossy().to_string(), "args = [\"--mcp-server\"]")
    };

    let approval_mode = if approval_policy == "never" {
        "auto"
    } else {
        "prompt"
    };
    let enabled = enabled_tool_names();

    let mut extra = format!(
        concat!(
            "\n",
            "# Guaxi 认知层工具面（召回 / 图谱 / 感知 / 信念 / 任务）。\n",
            "# 由 stdio 桥接转发到主进程 127.0.0.1:8888/mcp；工具的读写授权在 Guaxi 设置里控制。\n",
            "[mcp_servers.guaxi]\n",
            "command = \"{cmd}\"\n",
            "{args}\n",
            // 首次拉起桥接要等主进程 8888 就绪；默认 30 秒在冷启动（同时起 embedding、
            // Python agent_server）时不够，会被判成握手超时。
            "startup_timeout_sec = 60\n",
            "default_tools_approval_mode = \"{approval_mode}\"\n",
        ),
        cmd = toml_escape(&command),
        args = args,
        approval_mode = approval_mode,
    );
    if !enabled.is_empty() {
        let list = enabled
            .iter()
            .map(|n| format!("\"{}\"", toml_escape(n)))
            .collect::<Vec<_>>()
            .join(", ");
        extra.push_str(&format!("enabled_tools = [{list}]\n"));
    }
    Some(extra)
}

/// 当前已授权的工具名，供 codex 的 `enabled_tools` 白名单使用。
fn enabled_tool_names() -> Vec<String> {
    crate::cognition::tools::list_tools()
        .into_iter()
        .filter(|t| t.enabled)
        .map(|t| t.name)
        .collect()
}

/// 写完配置后做一次可用性自检：真的拉起 codex 问它认不认这个 MCP 服务器。
///
/// 没有这一步时，"桥接没被分发"或"配置写错"与"工具层坏了"是同一个症状——
/// 模型只是不用工具。这里把原因变成一条可读日志，而不是让用户猜。
pub async fn verify_mcp_bridge(binary: &PathBuf, home: &PathBuf) -> Result<usize, String> {
    let output = Command::new(binary)
        .arg("mcp")
        .arg("list")
        .env("CODEX_HOME", home)
        .output()
        .await
        .map_err(|e| format!("运行 codex mcp list 失败: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|l| l.trim_start().starts_with("guaxi"));
    match line {
        Some(l) if l.contains("enabled") => Ok(enabled_tool_names().len()),
        Some(l) => Err(format!("codex 认到了 guaxi，但状态不是 enabled: {}", l.trim())),
        None => Err(format!(
            "codex 没有注册 guaxi MCP 服务器。原始输出: {}",
            text.trim()
        )),
    }
}

/// codex item 的 `item_type` → 前端工具卡片用的工具名。
///
/// 返回 None 表示这类 item 不做成卡片（例如纯正文的 agent_message）。
fn tool_name_for_item(item_type: &str) -> Option<&'static str> {
    match item_type {
        "command_execution" => Some("command_execution"),
        "file_change" | "patch_apply" => Some("file_change"),
        "mcp_tool_call" => Some("mcp_tool_call"),
        "web_search" => Some("web_search"),
        "todo_list" => Some("todo_list"),
        "reasoning" => Some("reasoning"),
        _ => None,
    }
}

/// 从 item 里取可展示的文本。字段名随 codex 版本略有差异，这里按优先级兜一圈。
fn item_text(item: &Value) -> String {
    for key in ["text", "content", "message", "summary"] {
        if let Some(s) = item.get(key).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    String::new()
}

/// 工具卡片"结果"区展示的内容：命令输出优先，其次退回文本。
fn item_result_text(item: &Value) -> String {
    if let Some(out) = item.get("aggregated_output").and_then(|v| v.as_str()) {
        let exit = item.get("exit_code").and_then(|v| v.as_i64());
        return match exit {
            Some(code) => format!("exit_code: {}\n{}", code, out),
            None => out.to_string(),
        };
    }
    let text = item_text(item);
    if !text.is_empty() {
        return text;
    }
    // 兜底：整个 item 原样给出，便于排查未覆盖的新类型。
    serde_json::to_string_pretty(item).unwrap_or_default()
}

fn item_id(item: &Value) -> String {
    for key in ["id", "item_id"] {
        if let Some(s) = item.get(key).and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    String::new()
}

fn item_type_of(item: &Value) -> String {
    item.get("item_type")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("type").and_then(|v| v.as_str()))
        .unwrap_or_default()
        .to_string()
}

/// 工具卡片"参数"区展示的字段。只挑已知的小字段，避免把整段命令输出塞进参数区。
fn card_args(item: &Value) -> Value {
    let mut map = serde_json::Map::new();
    for key in [
        "command", "cwd", "query", "server", "tool", "name", "path", "changes", "items",
    ] {
        if let Some(value) = item.get(key) {
            map.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(map)
}

/// 流式解析过程中的可变状态。
#[derive(Default)]
struct StreamState {
    /// agent_message 已经推给前端的字符数，按 item id 记，用来只发增量。
    emitted: std::collections::HashMap<String, usize>,
    thread_id: String,
    error: Option<String>,
    emitted_any_text: bool,
}

/// 把 codex 的一条 JSONL 事件翻译成前端已有的事件。
fn handle_codex_event(app: &AppHandle, event: &Value, state: &mut StreamState) {
    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match event_type {
        "thread.started" => {
            let thread_id = event
                .get("thread_id")
                .and_then(|v| v.as_str())
                .or_else(|| event.pointer("/thread/id").and_then(|v| v.as_str()))
                .unwrap_or_default()
                .to_string();
            if !thread_id.is_empty() {
                state.thread_id = thread_id.clone();
                // 前端把它存到当前会话上，下一轮提问就能 resume 同一个 codex 线程。
                let _ = app.emit("codex-thread", json!({ "thread_id": thread_id }));
            }
        }
        "item.started" | "item.updated" | "item.completed" => {
            let Some(item) = event.get("item") else { return };
            let item_type = item_type_of(item);

            if item_type == "agent_message" {
                emit_agent_message_delta(app, item, state);
                return;
            }

            // item 级别的错误（模型元数据缺失、单步执行失败等）也要让用户看见，
            // 否则只有正文说"失败了"而没有任何线索。
            if item_type == "error" {
                if event_type == "item.completed" {
                    let message = item_text(item);
                    if !message.is_empty() {
                        warn!(message = %message, "[codex_agent] codex item error");
                        let _ = app.emit(
                            "agent-step",
                            json!({
                                "type": "error",
                                "name": "agent_stream",
                                "status": "error",
                                "message": message,
                            }),
                        );
                    }
                }
                return;
            }

            let Some(tool_name) = tool_name_for_item(&item_type) else { return };
            match event_type {
                "item.started" => {
                    let _ = app.emit(
                        "agent-step",
                        json!({
                            "type": "tool_call",
                            "name": tool_name,
                            "status": "running",
                            "args": card_args(item),
                        }),
                    );
                }
                "item.completed" => {
                    // codex 只在失败时给非零 exit_code / status=failed，这里映射成卡片的错误态。
                    let failed = item
                        .get("exit_code")
                        .and_then(|v| v.as_i64())
                        .map(|code| code != 0)
                        .unwrap_or(false)
                        || item
                            .get("status")
                            .and_then(|v| v.as_str())
                            .map(|s| s == "failed" || s == "error")
                            .unwrap_or(false);
                    let _ = app.emit(
                        "agent-step",
                        json!({
                            "type": "tool_result",
                            "name": tool_name,
                            "status": if failed { "error" } else { "done" },
                            "result": item_result_text(item),
                        }),
                    );
                }
                _ => {}
            }
        }
        "turn.failed" | "error" => {
            let message = event
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .or_else(|| event.get("message").and_then(|v| v.as_str()))
                .unwrap_or("codex 执行失败")
                .to_string();

            // codex 会把每次重试都作为 error 事件上报（"Reconnecting... 1/5"）。
            // 这些是过程噪声，最终仍会有一条真正的失败事件；只记日志，不刷满工具卡片。
            if message.starts_with("Reconnecting") {
                warn!(message = %message, "[codex_agent] codex 正在重连");
                return;
            }

            warn!(message = %message, "[codex_agent] codex 报告错误");
            state.error = Some(message.clone());
            let _ = app.emit(
                "agent-step",
                json!({
                    "type": "error",
                    "name": "agent_stream",
                    "status": "error",
                    "message": message,
                }),
            );
        }
        _ => {}
    }
}

/// agent_message 是最终正文。codex 会多次上报同一个 item 的完整文本，
/// 因此按已发送字符数切出增量，避免前端重复拼接。
fn emit_agent_message_delta(app: &AppHandle, item: &Value, state: &mut StreamState) {
    let text = item_text(item);
    if text.is_empty() {
        return;
    }
    let key = item_id(item);
    let already = state.emitted.get(&key).copied().unwrap_or(0);
    let total = text.chars().count();
    if total <= already {
        return;
    }
    let delta: String = text.chars().skip(already).collect();
    state.emitted.insert(key, total);
    state.emitted_any_text = true;
    let _ = app.emit("modle-streaming-respond-output", delta);
}

/// 组装 `codex exec` 的参数。
///
/// 注意：`codex exec resume` 不支持 `-C` / `-s` / `--add-dir`（工作目录与沙箱沿用原会话），
/// 所以续接时只传通用参数；新会话才带上工作目录与沙箱。
fn build_args(request: &CodexAgentRequest, work_dir: &PathBuf, sandbox: &str, model: &str) -> Vec<String> {
    let mut args: Vec<String> = vec!["exec".to_string()];
    let thread_id = request.thread_id.trim();
    let resuming = !thread_id.is_empty();
    if resuming {
        args.push("resume".to_string());
        args.push(thread_id.to_string());
    }

    args.push("--json".to_string());
    // 知识库通常不是 git 仓库，不跳过检查 codex 会直接拒绝启动。
    args.push("--skip-git-repo-check".to_string());

    if !resuming {
        args.push("-C".to_string());
        args.push(work_dir.to_string_lossy().to_string());
        // 跨库引用：其余知识库也加入可写范围，去掉与主工作目录重复的项。
        let primary = work_dir.to_string_lossy().to_string();
        for root in &request.repo_roots {
            let root = root.trim();
            if root.is_empty() || root == primary {
                continue;
            }
            args.push("--add-dir".to_string());
            args.push(root.to_string());
        }
        args.push("-s".to_string());
        args.push(sandbox.to_string());
    }

    args.push("-m".to_string());
    args.push(model.to_string());

    if request.web_search_enabled {
        // 联网工具的开关名在不同 codex 版本里有出入；传不认识的 -c 只会被忽略，不影响启动。
        args.push("-c".to_string());
        args.push("tools.web_search_request=true".to_string());
    }

    // 提示词走 stdin，避免命令行长度上限与 Windows 下的引号转义问题。
    args.push("-".to_string());
    args
}

/// 记录/清理当前运行的 codex 子进程，供停止按钮与软件退出时使用。
fn remember_child(pid: u32, label: &str) {
    if let Ok(mut guard) = CODEX_CHILD.lock() {
        *guard = Some((pid, label.to_string()));
    }
    crate::guaxiservices::process_shutdown::register_managed_pid(label, pid);
}

fn forget_child(pid: u32) {
    if let Ok(mut guard) = CODEX_CHILD.lock() {
        if guard.as_ref().map(|(id, _)| *id) == Some(pid) {
            *guard = None;
        }
    }
    crate::guaxiservices::process_shutdown::unregister_managed_pid(pid);
}

/// 结束仍在运行的 codex 子进程（软件退出流程调用）。
pub fn stop_codex_agent() {
    let current = CODEX_CHILD.lock().ok().and_then(|mut g| g.take());
    if let Some((pid, label)) = current {
        info!(pid, "[codex_agent] stopping codex agent");
        crate::guaxiservices::process_shutdown::terminate_process_tree(pid, &label);
        crate::guaxiservices::process_shutdown::unregister_managed_pid(pid);
    }
}

/// 决定 codex 的工作目录（沙箱可写根）。没有打开知识库时用私有 workspace 兜底，
/// 避免让模型直接把 Guaxi 的配置目录当工作区。
fn resolve_work_dir(request: &CodexAgentRequest, home: &PathBuf) -> Result<PathBuf, String> {
    let repo = request.repo_path.trim();
    if !repo.is_empty() {
        return Ok(PathBuf::from(repo));
    }
    let scratch = home.join("workspace");
    if !scratch.exists() {
        std::fs::create_dir_all(&scratch)
            .map_err(|e| format!("创建 codex 临时工作目录失败: {}", e))?;
    }
    Ok(scratch)
}

/// 驱动一轮 codex Agent 对话，把事件流转发给前端。
///
/// 返回 Err 表示这一轮没能跑起来（二进制缺失、供应商没配、进程启动失败），
/// 调用方据此决定是否回退到内置 Python Agent。已经进入事件流后的错误不再返回 Err，
/// 而是通过 `agent-step` 的 error 事件展示，保持与内置引擎一致的表现。
pub async fn run_codex_agent(app: &AppHandle, request: &CodexAgentRequest) -> Result<(), String> {
    let binary = codex_binary_path()?;
    let home = codex_home_dir()?;
    let provider = resolve_provider()?;
    let sandbox = resolve_sandbox_mode();
    // exec 链路没有 UI 可以接审批，保持 never；审批只由 app-server 那条路承担。
    write_codex_config(&home, &provider, &sandbox, "never")?;

    let work_dir = resolve_work_dir(request, &home)?;

    // 自检：确认 codex 真的注册了 guaxi 的 MCP 服务器。失败不阻断这一轮
    // （文件工具仍然可用），但必须留下可读原因——否则"工具没接上"和
    // "模型不想用工具"在用户眼里是同一件事。
    match verify_mcp_bridge(&binary, &home).await {
        Ok(n) => info!(tools = n, "[codex_agent] guaxi MCP 已就绪"),
        Err(e) => warn!("[codex_agent] guaxi MCP 未接上，本轮只能用 codex 内置工具：{e}"),
    }

    let args = build_args(request, &work_dir, &sandbox, &provider.model);

    info!(
        binary = %binary.to_string_lossy(),
        work_dir = %work_dir.to_string_lossy(),
        sandbox = %sandbox,
        model = %provider.model,
        provider = provider.source,
        session_id = %request.sessionid,
        resuming = !request.thread_id.trim().is_empty(),
        "[codex_agent] launching codex exec"
    );

    let mut cmd = Command::new(&binary);
    cmd.args(&args)
        // CODEX_HOME 指向 Guaxi 私有目录，codex 的配置、会话、缓存都落在这里，
        // 用户自己的 ~/.codex 全程不读不写。
        .env("CODEX_HOME", &home)
        .env(CODEX_API_KEY_ENV, &provider.api_key)
        .current_dir(&work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    apply_no_window(&mut cmd);

    let mut child: Child = cmd
        .spawn()
        .map_err(|e| format!("启动 codex 失败: {}", e))?;
    let pid = child.id().unwrap_or(0);
    remember_child(pid, "codex_agent(codex exec)");

    // 提示词写进 stdin 后立刻关闭，codex 读到 EOF 才会开始执行。
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        if let Err(err) = stdin.write_all(request.question.as_bytes()).await {
            warn!(error = %err, "[codex_agent] 写入 codex stdin 失败");
        }
        let _ = stdin.flush().await;
        drop(stdin);
    }

    // stderr 只用于诊断：codex 把进度写 stderr，正式事件都在 stdout。
    // 单独收尾部内容，启动即失败时把它带回错误信息。
    let stderr_tail = std::sync::Arc::new(Mutex::new(String::new()));
    if let Some(stderr) = child.stderr.take() {
        let sink = stderr_tail.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut buf) = sink.lock() {
                    buf.push_str(&line);
                    buf.push('\n');
                    // 只保留尾部，避免长时间任务把内存吃掉。
                    if buf.len() > 8192 {
                        let cut = buf.len() - 4096;
                        *buf = buf[cut..].to_string();
                    }
                }
            }
        });
    }

    let mut state = StreamState::default();
    let mut canceled = false;

    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            tokio::select! {
                line = lines.next_line() => {
                    match line {
                        Ok(Some(line)) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() { continue; }
                            match serde_json::from_str::<Value>(trimmed) {
                                Ok(event) => handle_codex_event(app, &event, &mut state),
                                // 非 JSON 行是 codex 偶发写到 stdout 的提示文本，记日志即可。
                                Err(_) => info!(line = %trimmed, "[codex_agent] 非 JSON 输出"),
                            }
                        }
                        Ok(None) => break,
                        Err(err) => {
                            warn!(error = %err, "[codex_agent] 读取 codex stdout 失败");
                            break;
                        }
                    }
                }
                _ = ticker.tick() => {
                    // 前端的停止按钮走 cancel_chat_stream，两个引擎共用同一个标志位。
                    if crate::commands::chatview::stream_cancel_requested() {
                        canceled = true;
                        break;
                    }
                }
            }
        }
    }

    if canceled {
        info!(pid, "[codex_agent] canceled by user");
        crate::guaxiservices::process_shutdown::terminate_process_tree(pid, "codex_agent(codex exec)");
        let _ = child.kill().await;
        forget_child(pid);
        return Ok(());
    }

    let status = child.wait().await.ok();
    forget_child(pid);

    let tail = stderr_tail.lock().map(|b| b.clone()).unwrap_or_default();
    let exit_ok = status.map(|s| s.success()).unwrap_or(false);

    if !exit_ok && !state.emitted_any_text && state.error.is_none() {
        // 完全没产出：这一轮等于没跑起来，把 stderr 尾部带回去让调用方决定是否回退。
        return Err(format!(
            "codex 执行失败（{}）。诊断输出:\n{}",
            status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "未知退出状态".to_string()),
            tail.trim()
        ));
    }

    if !exit_ok {
        warn!(tail = %tail.trim(), "[codex_agent] codex 非正常退出，但已有部分输出");
    }

    info!(
        thread_id = %state.thread_id,
        emitted_text = state.emitted_any_text,
        "[codex_agent] codex exec finished"
    );
    Ok(())
}

/// 读取 codex 版本号。只在设置页刷新时调用，结果由前端缓存。
async fn read_codex_version(binary: &PathBuf) -> String {
    let mut cmd = Command::new(binary);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    apply_no_window(&mut cmd);
    match cmd.output().await {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(err) => {
            warn!(error = %err, "[codex_agent] 读取 codex 版本失败");
            String::new()
        }
    }
}

/// 设置页状态面板用的 Codex 引擎状态。
#[tauri::command]
pub async fn codex_agent_status() -> CodexAgentStatus {
    let mut status = CodexAgentStatus {
        sandbox: resolve_sandbox_mode(),
        ..Default::default()
    };

    match codex_home_dir() {
        Ok(home) => status.codex_home = home.to_string_lossy().to_string(),
        Err(err) => status.hint = err,
    }

    match codex_binary_path() {
        Ok(binary) => {
            status.binary_exists = true;
            status.binary_path = binary.to_string_lossy().to_string();
            status.version = read_codex_version(&binary).await;
        }
        Err(err) => {
            status.binary_exists = false;
            status.hint = err;
        }
    }

    match resolve_provider() {
        Ok(provider) => {
            status.provider_ready = true;
            status.provider_base_url = provider.base_url;
            status.provider_model = provider.model;
        }
        Err(err) => {
            status.provider_ready = false;
            if status.hint.is_empty() {
                status.hint = err;
            }
        }
    }

    status
}
