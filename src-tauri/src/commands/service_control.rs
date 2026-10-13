//! 后端服务的登记表与启停控制（「设置 → 系统与服务 → 服务器」页面的后端）。
//!
//! 为什么要这个：应用会拉起一堆长期运行的后端服务（本地 HTTP 后端、局域网接收、Agent、
//! RSSHub、本地大模型/向量化/翻译、图床分享…）。此前它们**既看不到也管不了** —— 启动失败
//! 只打一行日志，用户碰上端口冲突后除了重启应用没有别的办法；严格网络管控环境下想关掉某个
//! 端口也没有入口。
//!
//! 设计要点：
//! - 一张**静态描述表**：每项给出状态探测、启动、停止三个口子。已有 start/stop 的直接指向
//!   既有函数，不改它们的内部逻辑；没有"启动"的按需服务留空即可（`can_start` 由它推导）。
//! - 状态探测**优先用 TCP connect**：比"句柄还在不在"更真实 —— 能发现"进程活着但没在监听"
//!   （端口被别的程序抢走时就是这个表现，正是本页要暴露的那类问题）。
//! - 用户的「停用」写进 `AppConfig.disabled_services` **持久化**：严格管控环境里关掉的端口，
//!   重启应用后不该自己又开起来。
//! - 重启统一走「停 → 等 1 秒放端口 → 起」，沿用 `rsshub_server::restart` 的既有范式；
//!   否则会和自己刚停掉的服务抢同一个端口，反倒制造 `os error 10048`。

use futures_util::future::BoxFuture;
use serde::Serialize;
use std::net::{SocketAddr, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::state::SharedState;

/// 服务当前状态。`Disabled` 表示用户在设置里停用了它（与"停着但允许自启"的 `Stopped` 区分）。
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Running,
    Stopped,
    Disabled,
}

/// 给前端看的一条服务信息。
#[derive(Serialize, Clone, Debug)]
pub struct ServiceInfo {
    pub id: String,
    pub label: String,
    pub description: String,
    /// `http` | `child`，仅用于前端换个图标/文案，不参与逻辑。
    pub kind: String,
    /// 形如 `127.0.0.1:8888` / `0.0.0.0:14219`；无端口的服务为 None。
    pub address: Option<String>,
    pub port: Option<u16>,
    pub status: ServiceStatus,
    pub can_start: bool,
    pub can_stop: bool,
    /// 用户是否允许它随应用自启（即未在 `disabled_services` 里）。
    pub autostart: bool,
}

type Action = fn() -> BoxFuture<'static, Result<(), String>>;

struct ServiceDef {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    kind: &'static str,
    port: Option<u16>,
    address: Option<&'static str>,
    /// 是否由本模块负责"随应用启动"。agent-server / rsshub 的自启仍在 main.rs 的 setup 里
    /// （那边还带着"未打包则跳过"之类的判断），这里只登记它们的启停能力，不重复启动。
    boot_autostart: bool,
    start: Option<Action>,
    stop: Option<Action>,
}

/// 服务登记表。加服务 = 加一行（阶段 3 会把本地大模型/向量化/翻译等逐个补进来）。
static SERVICE_DEFS: &[ServiceDef] = &[
    ServiceDef {
        id: "local-backend",
        label: "本地后端服务",
        description: "Markdown 图片路径改写与本地图片/图床服务、知识库与浏览器预览、MCP、LSP。挂掉会导致文档里的本地图片渲染不出来。",
        kind: "http",
        port: Some(8888),
        address: Some("127.0.0.1:8888"),
        boot_autostart: true,
        start: Some(start_local_backend),
        stop: Some(stop_local_backend),
    },
    ServiceDef {
        id: "capture-import",
        label: "局域网导入接收",
        description: "供手机等设备把采集内容写进文件库的窄接口。监听 0.0.0.0，不需要时可单独关闭以减少暴露面。",
        kind: "http",
        port: Some(14219),
        address: Some("0.0.0.0:14219"),
        boot_autostart: true,
        start: Some(start_capture_import),
        stop: Some(stop_capture_import),
    },
    ServiceDef {
        id: "agent-server",
        label: "Agent 服务",
        description: "代理/工具调用用的本地 Python 服务（uvicorn）。",
        kind: "child",
        port: Some(9876),
        address: Some("127.0.0.1:9876"),
        boot_autostart: false,
        start: Some(start_agent_server),
        stop: Some(stop_agent_server),
    },
    ServiceDef {
        id: "rsshub",
        label: "RSSHub 订阅服务",
        description: "把网站转成 RSS 的 sidecar 服务，供订阅类内容抓取。",
        kind: "child",
        port: Some(1200),
        address: Some("127.0.0.1:1200"),
        boot_autostart: false,
        start: Some(start_rsshub),
        stop: Some(stop_rsshub),
    },
    ServiceDef {
        id: "library-share",
        label: "知识库对外分享",
        description: "带令牌把知识库暴露到局域网的独立端口（微信/QQ/浏览器访问）。默认不开。",
        kind: "http",
        port: Some(8899),
        address: Some("0.0.0.0:8899"),
        boot_autostart: false,
        start: Some(start_library_share),
        stop: Some(stop_library_share),
    },
];

// ─────────────────────────── 全局状态 ───────────────────────────

static STATE: OnceLock<SharedState> = OnceLock::new();

/// 由 main.rs 的 setup 注入。两个 actix 服务与 Agent/分享服务都需要它。
pub fn init_state(state: SharedState) {
    let _ = STATE.set(state);
}

fn state() -> Result<SharedState, String> {
    STATE
        .get()
        .cloned()
        .ok_or_else(|| "服务控制未初始化（缺少 SharedState）".to_string())
}

// ─────────────────────────── 各族服务的启停口子 ───────────────────────────

fn start_local_backend() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        let state = state()?;
        crate::guaxiservices::server::run_local_backend(state).await
    })
}

fn stop_local_backend() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::guaxiservices::server::stop_local_backend().await;
        Ok(())
    })
}

fn start_capture_import() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        let state = state()?;
        crate::guaxiservices::server::run_capture_import(state).await
    })
}

fn stop_capture_import() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::guaxiservices::server::stop_capture_import().await;
        Ok(())
    })
}

fn start_agent_server() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        let app = state()?
            .get_app_handle()
            .ok_or_else(|| "AppHandle 未就绪".to_string())?;
        crate::commands::agent_server::ensure_agent_server_started(&app).await
    })
}

fn stop_agent_server() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::agent_server::stop_agent_server();
        Ok(())
    })
}

fn start_rsshub() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(crate::commands::rsshub_server::ensure_started())
}

fn stop_rsshub() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::rsshub_server::stop();
        Ok(())
    })
}

fn start_library_share() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        let app = state()?
            .get_app_handle()
            .ok_or_else(|| "AppHandle 未就绪".to_string())?;
        crate::commands::library_site::library_site_start_share(app).await.map(|_| ())
    })
}

fn stop_library_share() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::library_site::library_site_stop_share().await.map(|_| ())
    })
}

// ─────────────────────────── 探测 ───────────────────────────

/// 端口上是否真的有人在监听。
/// 用 TCP connect 而不是"进程句柄还在不在"：端口被别的程序抢走时，进程活着但没在监听，
/// 只看句柄会误报成"运行中"。
fn tcp_listening(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

fn is_running(def: &ServiceDef) -> bool {
    match def.port {
        Some(port) => tcp_listening(port),
        // 没有端口的服务暂时不参与状态展示（阶段 3 接入时再补各自的探测）。
        None => false,
    }
}

fn find_def(id: &str) -> Result<&'static ServiceDef, String> {
    SERVICE_DEFS
        .iter()
        .find(|def| def.id == id)
        .ok_or_else(|| format!("未知的服务: {id}"))
}

// ─────────────────────────── 停用偏好（持久化） ───────────────────────────

fn disabled_services() -> Vec<String> {
    crate::commands::settingview::load_config().disabled_services
}

/// 该服务是否允许随应用自启。
pub fn autostart_enabled(id: &str) -> bool {
    !disabled_services().iter().any(|item| item == id)
}

fn set_autostart_flag(id: &str, enabled: bool) -> Result<(), String> {
    let mut config = crate::commands::settingview::load_config();
    let was_enabled = !config.disabled_services.iter().any(|item| item == id);
    if was_enabled == enabled {
        return Ok(());
    }
    config.disabled_services.retain(|item| item != id);
    if !enabled {
        config.disabled_services.push(id.to_string());
    }
    crate::commands::settingview::save_config(&config);
    info!(service = id, enabled, "[ServiceControl] 自启偏好已更新");
    Ok(())
}

// ─────────────────────────── 启动（含退避重试） ───────────────────────────

/// 随应用启动的服务：按登记表拉起，并尊重用户的停用偏好。
pub fn autostart_services(state: SharedState) {
    init_state(state);
    for def in SERVICE_DEFS.iter().filter(|def| def.boot_autostart) {
        if def.start.is_none() {
            continue;
        }
        if !autostart_enabled(def.id) {
            info!(service = def.id, "[ServiceControl] 已被用户停用，跳过自启");
            continue;
        }
        spawn_start_with_retry(def);
    }
}

/// 带退避重试地启动一个服务。
///
/// 为什么要重试：端口绑定失败绝大多数是**暂时**的 —— 上一个实例刚退出、端口还在 TIME_WAIT，
/// 或子进程仍在收尾。隔几秒再试通常就好了。真被别的进程长期占用时重试到底也起不来，
/// 所以有上限；放弃后用户可以在「设置 → 服务器」手动启动。
fn spawn_start_with_retry(def: &'static ServiceDef) {
    let Some(start) = def.start else { return };
    let id = def.id;
    tauri::async_runtime::spawn(async move {
        const MAX_ATTEMPTS: u32 = 6;
        for attempt in 1..=MAX_ATTEMPTS {
            match start().await {
                // 正常结束（被停掉，或进程在退出）—— 不需要重试。
                Ok(()) => return,
                Err(error) if attempt == MAX_ATTEMPTS => {
                    error!(
                        service = id,
                        attempts = MAX_ATTEMPTS,
                        %error,
                        "[ServiceControl] 连续启动失败，已放弃自动重试；处理掉端口占用后可在 设置 → 服务器 手动启动"
                    );
                    return;
                }
                Err(error) => {
                    let wait = Duration::from_secs(1u64 << (attempt - 1).min(4));
                    warn!(
                        service = id,
                        attempt,
                        max = MAX_ATTEMPTS,
                        wait_secs = wait.as_secs(),
                        %error,
                        "[ServiceControl] 启动失败，退避后重试"
                    );
                    tokio::time::sleep(wait).await;
                }
            }
        }
    });
}

// ─────────────────────────── 对外命令 ───────────────────────────

#[tauri::command]
pub fn service_list() -> Vec<ServiceInfo> {
    let disabled = disabled_services();
    SERVICE_DEFS
        .iter()
        .map(|def| {
            let running = is_running(def);
            let autostart = !disabled.iter().any(|item| item == def.id);
            let status = if running {
                ServiceStatus::Running
            } else if autostart {
                ServiceStatus::Stopped
            } else {
                ServiceStatus::Disabled
            };
            ServiceInfo {
                id: def.id.to_string(),
                label: def.label.to_string(),
                description: def.description.to_string(),
                kind: def.kind.to_string(),
                address: def.address.map(|value| value.to_string()),
                port: def.port,
                status,
                can_start: def.start.is_some(),
                can_stop: def.stop.is_some(),
                autostart,
            }
        })
        .collect()
}

/// 手动启动。会**清掉停用标记** —— 用户点了启动就是要它起来，不该下次还被偏好挡住。
#[tauri::command]
pub async fn service_start(id: String) -> Result<(), String> {
    let def = find_def(&id)?;
    let start = def.start.ok_or_else(|| format!("{} 不支持手动启动", def.label))?;
    set_autostart_flag(&id, true)?;
    info!(service = id, "[ServiceControl] 手动启动");
    start().await
}

#[tauri::command]
pub async fn service_stop(id: String) -> Result<(), String> {
    let def = find_def(&id)?;
    let stop = def.stop.ok_or_else(|| format!("{} 不支持停止", def.label))?;
    info!(service = id, "[ServiceControl] 停止");
    stop().await
}

/// 重启 = 停 → 等端口释放 → 起。中间那一秒是必须的：actix/子进程释放监听端口不是瞬时的，
/// 立刻重绑会撞上 `os error 10048`（这正是用户最初报的那个错）。
#[tauri::command]
pub async fn service_restart(id: String) -> Result<(), String> {
    let def = find_def(&id)?;
    let stop = def.stop.ok_or_else(|| format!("{} 不支持停止", def.label))?;
    let start = def.start.ok_or_else(|| format!("{} 不支持启动", def.label))?;

    info!(service = id, "[ServiceControl] 重启：先停");
    stop().await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    info!(service = id, "[ServiceControl] 重启：再起");
    let result = start().await;
    if result.is_ok() {
        // 重启成功说明这个服务现在是用户要的，顺手清掉停用标记。
        let _ = set_autostart_flag(&id, true);
    }
    result
}

/// 设置"是否随应用自启"。停用是持久化的：重启应用后不会自己再起来。
#[tauri::command]
pub fn service_set_autostart(id: String, enabled: bool) -> Result<(), String> {
    find_def(&id)?;
    set_autostart_flag(&id, enabled)
}
