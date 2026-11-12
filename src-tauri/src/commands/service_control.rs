//! 后端服务的登记表与启停控制（「设置 → 系统与服务 → 服务器」页面的后端）。
//!
//! 为什么要这个：应用会拉起一堆长期运行的后端服务（本地 HTTP 后端、局域网接收、Agent、
//! RSSHub、本地大模型/向量化/翻译、图床分享、以及各种会话池）。此前它们**既看不到也管不了**
//! —— 启动失败只打一行日志，用户碰上端口冲突后除了重启应用没有别的办法；严格网络管控环境
//! 下想关掉某个端口也没有入口。
//!
//! 设计要点：
//! - 一张**静态描述表**：每项给出状态探测、启动、停止三个口子。已有 start/stop 的直接指向
//!   既有函数，不改它们的内部逻辑；没有"启动"的按需服务留空即可（`can_start` 由它推导）。
//! - 状态探测**优先用 TCP connect**：比"句柄还在不在"更真实 —— 能发现"进程活着但没在监听"
//!   （端口被别的程序抢走时就是这个表现，正是本页要暴露的那类问题）。没有固定端口的
//!   （P2P 走 QUIC、局域网文件接收端口由用户指定）交给各自的模块自己判断。
//!   例外是 8888 / 14219 这两个**由本进程托管**的 actix 服务：它们的状态看本进程是否
//!   真的持有服务句柄（见 `server::local_backend_owned`），否则另一个 Guaxi 实例占着端口
//!   时会被误报成"运行中"，而实际接走请求的是别人。
//! - 会话池（终端/SSH/调试/…）没有"启停"一说，只有"当前几个 + 一键全关"，所以它们用
//!   `count` 提供计数，状态由"有没有活跃会话"推导。
//! - 用户的「停用」写进 `AppConfig.disabled_services` **持久化**：严格管控环境里关掉的端口，
//!   重启应用后不该自己又开起来。
//! - 重启统一走「停 → 等端口真正释放 → 起 → 确认已在监听」，沿用 `rsshub_server::restart`
//!   的既有范式；否则会和自己刚停掉的服务抢同一个端口，反倒制造 `os error 10048`。

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
    /// `http` | `child` | `session`，仅用于前端换个图标/文案，不参与逻辑。
    pub kind: String,
    /// 分组标题：常驻服务 / 按需服务 / 会话与临时任务。
    pub group: String,
    /// 形如 `127.0.0.1:8888` / `0.0.0.0:14219`；无端口的为 None。
    pub address: Option<String>,
    pub port: Option<u16>,
    pub status: ServiceStatus,
    pub can_start: bool,
    pub can_stop: bool,
    /// 用户是否允许它随应用自启（即未在 `disabled_services` 里）。
    pub autostart: bool,
    /// 补充说明，例如「3 个会话」「端口由用户指定」。
    pub detail: Option<String>,
}

type Action = fn() -> BoxFuture<'static, Result<(), String>>;
/// 没有固定端口时的自定义存活探测。
type Probe = fn() -> BoxFuture<'static, bool>;

struct ServiceDef {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    kind: &'static str,
    group: &'static str,
    /// 有端口就按 TCP connect 探测，不必再写 probe。
    port: Option<u16>,
    address: Option<&'static str>,
    /// 没有端口的服务（P2P / 文件接收）用它判断是否在跑。
    probe: Option<Probe>,
    /// 会话池：返回当前活跃数量。给了它就按"数量 > 0"判定在跑。
    count: Option<fn() -> usize>,
    /// 是否由本模块负责"随应用启动"。agent-server / rsshub 的自启仍在 main.rs 的 setup 里
    /// （那边带着"未打包则跳过"之类的判断），这里只登记它们的启停能力，不重复启动。
    boot_autostart: bool,
    /// 静态补充说明（会话池的说明由 count 动态生成，不用这个）。
    detail: Option<&'static str>,
    start: Option<Action>,
    stop: Option<Action>,
}

const GROUP_BOOT: &str = "常驻服务";
const GROUP_ONDEMAND: &str = "按需服务";
const GROUP_SESSION: &str = "会话与临时任务";

/// 重启时等端口释放 / 等它开始监听的上限。
const PORT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
/// 重启成功后等端口开始监听的上限（RSSHub / uvicorn 这类要起进程，比释放慢）。
const START_LISTEN_TIMEOUT: Duration = Duration::from_secs(60);

/// 服务登记表。加服务 = 加一行。
static SERVICE_DEFS: &[ServiceDef] = &[
    // ───────── 常驻：随应用启动 ─────────
    ServiceDef {
        id: "local-backend",
        label: "本地后端服务",
        description: "Markdown 图片路径改写与本地图片/图床服务、知识库与浏览器预览、MCP、LSP。挂掉会导致文档里的本地图片渲染不出来。",
        kind: "http",
        group: GROUP_BOOT,
        port: Some(8888),
        address: Some("127.0.0.1:8888"),
        probe: None,
        count: None,
        boot_autostart: true,
        detail: None,
        start: Some(start_local_backend),
        stop: Some(stop_local_backend),
    },
    ServiceDef {
        id: "capture-import",
        label: "局域网导入接收",
        description: "供手机等设备把采集内容写进文件库的窄接口。监听 0.0.0.0，不需要时可单独关闭以减少暴露面。",
        kind: "http",
        group: GROUP_BOOT,
        port: Some(14219),
        address: Some("0.0.0.0:14219"),
        probe: None,
        count: None,
        boot_autostart: true,
        detail: None,
        start: Some(start_capture_import),
        stop: Some(stop_capture_import),
    },
    ServiceDef {
        id: "agent-server",
        label: "Agent 服务",
        description: "代理/工具调用用的本地 Python 服务（uvicorn）。",
        kind: "child",
        group: GROUP_BOOT,
        port: Some(9876),
        address: Some("127.0.0.1:9876"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_agent_server),
        stop: Some(stop_agent_server),
    },
    ServiceDef {
        id: "rsshub",
        label: "RSSHub 订阅服务",
        description: "把网站转成 RSS 的 sidecar 服务，供订阅类内容抓取。未打包进安装包时不会启动。",
        kind: "child",
        group: GROUP_BOOT,
        port: Some(1200),
        address: Some("127.0.0.1:1200"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_rsshub),
        stop: Some(stop_rsshub),
    },
    ServiceDef {
        id: "p2p",
        label: "P2P 同步（iroh）",
        description: "设备间直连同步用的 QUIC endpoint（中继 + 打洞）。没有固定端口，关闭后会中断正在进行的同步。",
        kind: "http",
        group: GROUP_BOOT,
        port: None,
        address: Some("QUIC（中继 + 打洞，无固定端口）"),
        probe: Some(p2p_running),
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_p2p),
        stop: Some(stop_p2p),
    },
    // ───────── 按需：功能触发后常驻 ─────────
    ServiceDef {
        id: "local-deepseek",
        label: "本地大模型（DeepSeek）",
        description: "本地聊天用的 llama-server。由聊天功能按需拉起，也可以在这里关掉以释放显存/内存。",
        kind: "child",
        group: GROUP_ONDEMAND,
        port: Some(18080),
        address: Some("127.0.0.1:18080"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_local_deepseek),
        stop: Some(stop_local_deepseek),
    },
    ServiceDef {
        id: "local-qwen",
        label: "本地大模型（Qwen）",
        description: "本地聊天用的 llama-server。注意它与「本地翻译服务」默认都用 18082，同时开只会有一个绑得上。",
        kind: "child",
        group: GROUP_ONDEMAND,
        port: Some(18082),
        address: Some("127.0.0.1:18082"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_local_qwen),
        stop: Some(stop_local_qwen),
    },
    ServiceDef {
        id: "embedding",
        label: "向量化服务",
        description: "知识库/认知层做语义检索用的 embedding 服务（llama-server --embedding）。",
        kind: "child",
        group: GROUP_ONDEMAND,
        port: Some(18081),
        address: Some("127.0.0.1:18081"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_embedding),
        stop: Some(stop_embedding),
    },
    ServiceDef {
        id: "translate-local",
        label: "本地翻译服务",
        description: "免费接口全挂时的兜底翻译（llama-server）。由翻译功能按需拉起，这里只提供关闭。注意它与「本地大模型（Qwen）」默认都用 18082。",
        kind: "child",
        group: GROUP_ONDEMAND,
        port: Some(18082),
        address: Some("127.0.0.1:18082"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: None,
        stop: Some(stop_translate_local),
    },
    ServiceDef {
        id: "library-share",
        label: "知识库对外分享",
        description: "带令牌把知识库暴露到局域网的独立端口（微信/QQ/浏览器访问）。默认不开。",
        kind: "http",
        group: GROUP_ONDEMAND,
        port: Some(8899),
        address: Some("0.0.0.0:8899"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: None,
        start: Some(start_library_share),
        stop: Some(stop_library_share),
    },
    ServiceDef {
        id: "filetransfer",
        label: "局域网文件接收",
        description: "设备间互传文件时的接收服务。监听地址与保存目录由「文件互传」页面指定，所以这里不能一键启动。",
        kind: "http",
        group: GROUP_ONDEMAND,
        port: None,
        address: None,
        probe: Some(filetransfer_running),
        count: None,
        boot_autostart: false,
        detail: Some("端口与保存目录由「文件互传」页面指定"),
        start: None,
        stop: Some(stop_filetransfer),
    },
    ServiceDef {
        id: "slidev-dev",
        label: "Slidev 预览服务",
        description: "演示文稿预览用的 Node 开发服务。启动需要一个入口文件，所以这里只提供关闭。",
        kind: "child",
        group: GROUP_ONDEMAND,
        port: Some(3030),
        address: Some("127.0.0.1:3030"),
        probe: None,
        count: None,
        boot_autostart: false,
        detail: Some("启动需要指定入口文件，请在 Slidev 面板里操作"),
        start: None,
        stop: Some(stop_slidev),
    },
    // ───────── 会话池：只有"几个 + 一键全关" ─────────
    session_def("terminal", "终端会话", "内置终端里打开的 shell 会话；每个下面还挂着各自的进程树。", terminal_count, stop_terminal_sessions),
    session_def("ssh", "SSH 会话", "远程 SSH 连接。", ssh_count, stop_ssh_sessions),
    session_def("gdb", "调试会话（GDB）", "GDB/MI 调试会话。", gdb_count, stop_gdb_sessions),
    session_def("serial", "串口会话", "已打开的串口连接。", serial_count, stop_serial_sessions),
    session_def("telnet", "Telnet 会话", "已打开的 Telnet 连接。", telnet_count, stop_telnet_sessions),
    session_def("net-conn", "网络监听服务", "网络工具箱里在跑的 TCP/UDP 监听。", net_conn_count, stop_net_conn_servers),
    session_def("net-capture", "抓包会话", "正在运行的抓包任务。", net_capture_count, stop_net_captures),
    session_def("python-worker", "Python 执行会话", "笔记本里常驻的 Python 执行子进程（每个会话一个进程）。", python_worker_count, stop_python_workers),
];

/// 会话池条目的构造：都是同一套形状，用个小函数避免写八遍。
const fn session_def(
    id: &'static str,
    label: &'static str,
    description: &'static str,
    count: fn() -> usize,
    stop: Action,
) -> ServiceDef {
    ServiceDef {
        id,
        label,
        description,
        kind: "session",
        group: GROUP_SESSION,
        port: None,
        address: None,
        probe: None,
        count: Some(count),
        boot_autostart: false,
        detail: None,
        start: None,
        stop: Some(stop),
    }
}

// ─────────────────────────── 全局状态 ───────────────────────────

static STATE: OnceLock<SharedState> = OnceLock::new();

/// 由 main.rs 的 setup 注入。几个服务需要它（取 AppHandle 或起 actix 服务）。
pub fn init_state(state: SharedState) {
    let _ = STATE.set(state);
}

fn state() -> Result<SharedState, String> {
    STATE
        .get()
        .cloned()
        .ok_or_else(|| "服务控制未初始化（缺少 SharedState）".to_string())
}

fn app_handle() -> Result<tauri::AppHandle, String> {
    state()?
        .get_app_handle()
        .ok_or_else(|| "AppHandle 未就绪".to_string())
}

// ─────────────────────────── 各服务的启停口子 ───────────────────────────

fn start_local_backend() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::guaxiservices::server::run_local_backend(state()?).await })
}

fn stop_local_backend() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::guaxiservices::server::stop_local_backend().await;
        Ok(())
    })
}

fn start_capture_import() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::guaxiservices::server::run_capture_import(state()?).await })
}

fn stop_capture_import() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::guaxiservices::server::stop_capture_import().await;
        Ok(())
    })
}

fn start_agent_server() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::commands::agent_server::ensure_agent_server_started(&app_handle()?).await })
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

fn p2p_running() -> BoxFuture<'static, bool> {
    Box::pin(crate::commands::p2psync::p2p_endpoint_running())
}

fn start_p2p() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(crate::commands::p2psync::p2p_start_endpoint())
}

fn stop_p2p() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(crate::commands::p2psync::p2p_stop_endpoint())
}

fn start_local_deepseek() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::commands::chatview::ensure_local_deepseek_service().await.map(|_| ()) })
}

fn stop_local_deepseek() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::chatview::shutdown_local_deepseek_server();
        Ok(())
    })
}

fn start_local_qwen() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::commands::chatview::ensure_local_qwen_service().await.map(|_| ()) })
}

fn stop_local_qwen() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::chatview::shutdown_local_qwen_server();
        Ok(())
    })
}

fn start_embedding() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::commands::embedding::kg_ensure_embedding_service().await.map(|_| ()) })
}

fn stop_embedding() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::embedding::shutdown_embedding_server();
        Ok(())
    })
}

fn stop_translate_local() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async { crate::commands::translate_local::translate_local_stop().await.map(|_| ()) })
}

fn start_library_share() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::library_site::library_site_start_share(app_handle()?)
            .await
            .map(|_| ())
    })
}

fn stop_library_share() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::library_site::library_site_stop_share()
            .await
            .map(|_| ())
    })
}

fn filetransfer_running() -> BoxFuture<'static, bool> {
    Box::pin(async {
        crate::commands::filetransfer::filetransfer_get_status()
            .await
            .map(|status| status.receiver.is_some())
            .unwrap_or(false)
    })
}

fn stop_filetransfer() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(crate::commands::filetransfer::filetransfer_stop_receiver())
}

fn stop_slidev() -> BoxFuture<'static, Result<(), String>> {
    // 注意 slidev_stop_dev 是同步的，别直接 Box::pin 它。
    Box::pin(async { crate::commands::slidev::slidev_stop_dev() })
}

// ── 会话池：计数 + 一键全关 ──

fn terminal_count() -> usize {
    crate::commands::terminal::active_session_count()
}
fn stop_terminal_sessions() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::terminal::shutdown_all_sessions();
        Ok(())
    })
}

fn ssh_count() -> usize {
    crate::commands::ssh_terminal::active_session_count()
}
fn stop_ssh_sessions() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::ssh_terminal::shutdown_all_sessions();
        Ok(())
    })
}

fn gdb_count() -> usize {
    crate::commands::gdb_mi::active_session_count()
}
fn stop_gdb_sessions() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::gdb_mi::shutdown_all_sessions();
        Ok(())
    })
}

fn serial_count() -> usize {
    crate::commands::serial_telnet::serial_active_count()
}
fn stop_serial_sessions() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::serial_telnet::shutdown_all_serial();
        Ok(())
    })
}

fn telnet_count() -> usize {
    crate::commands::serial_telnet::telnet_active_count()
}
fn stop_telnet_sessions() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::serial_telnet::shutdown_all_telnet();
        Ok(())
    })
}

fn net_conn_count() -> usize {
    crate::commands::net_conn::active_server_count()
}
fn stop_net_conn_servers() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::net_conn::shutdown_all_servers();
        Ok(())
    })
}

fn net_capture_count() -> usize {
    crate::commands::net_capture::active_capture_count()
}
fn stop_net_captures() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::net_capture::shutdown_all_captures();
        Ok(())
    })
}

fn python_worker_count() -> usize {
    crate::commands::code_runtime::active_python_worker_count()
}
fn stop_python_workers() -> BoxFuture<'static, Result<(), String>> {
    Box::pin(async {
        crate::commands::code_runtime::shutdown_all_python_workers();
        Ok(())
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

/// 等端口空出来（`want_listening = false`）或等它开始监听（`true`）。
///
/// 每 100ms 探一次，满足即返回，所以正常情况下几乎不花时间 —— 比写死 `sleep(1s)` 既快
/// 又更可靠：慢机器上 1 秒可能不够，快机器上白等。
async fn wait_for_port(port: u16, want_listening: bool) -> bool {
    let timeout = if want_listening {
        START_LISTEN_TIMEOUT
    } else {
        PORT_WAIT_TIMEOUT
    };
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let listening = tcp_listening(port);
        if listening == want_listening {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn is_running(def: &ServiceDef) -> bool {
    // 会话池：有活跃会话就算在跑。
    if let Some(count) = def.count {
        return count() > 0;
    }

    // 这两个服务由本进程托管，状态直接看「我们自己还有没有在跑」。
    //
    // 为什么不用端口探测：端口能连上不代表监听的是**本实例**。调试版 + 安装版同时开、
    // 或上个实例没退干净时，8888 会被另一个进程占着 —— 端口探测报「运行中」，
    // 但实际接走请求的是另一份配置（另一套图床根目录），于是本地图片/图床 404。
    // 这种情况必须显示成「未启动」，用户才知道该重启的是眼前这个实例。
    if def.id == "local-backend" {
        return crate::guaxiservices::server::local_backend_owned();
    }
    if def.id == "capture-import" {
        return crate::guaxiservices::server::capture_import_owned();
    }

    if let Some(port) = def.port {
        return tcp_listening(port);
    }
    match def.probe {
        Some(check) => check().await,
        None => false,
    }
}

fn find_def(id: &str) -> Result<&'static ServiceDef, String> {
    SERVICE_DEFS
        .iter()
        .find(|def| def.id == id)
        .ok_or_else(|| format!("未知的服务: {id}"))
}

/// 实际监听的端口。局域网导入接收允许用 `GUAXI_CAPTURE_BIND` 改写地址，登记表里那个
/// 只是默认值；拿它去等端口会等错对象。
fn effective_port(def: &ServiceDef) -> Option<u16> {
    if def.id == "capture-import" {
        crate::guaxiservices::server::capture_import_port().or(def.port)
    } else {
        def.port
    }
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

/// 兜底探测：没有可用运行时的时候，只做同步端口检查（覆盖绝大多数服务）。
fn futures_lite_blocking(def: &ServiceDef) -> bool {
    if let Some(count) = def.count {
        return count() > 0;
    }
    if def.id == "local-backend" {
        return crate::guaxiservices::server::local_backend_owned();
    }
    if def.id == "capture-import" {
        return crate::guaxiservices::server::capture_import_owned();
    }
    if let Some(port) = def.port {
        return tcp_listening(port);
    }
    false
}

#[tauri::command]
pub async fn service_list() -> Vec<ServiceInfo> {
    let disabled = disabled_services();

    // 并行探测，而不是逐个 await。
    //
    // 为什么要改：每个服务最多花 150ms 做 TCP 探测（端口没人监听时要等满超时），
    // 12 个服务里 10 个是端口探测 —— 串行最坏要 1.5 秒。设置页打开时就会明显卡一下，
    // 而这些探测彼此完全独立，没有理由排队。
    // 端口探测用的是同步 TcpStream::connect_timeout，直接 await 会占住 async worker 线程。
    // 丢进阻塞线程池后，UI 线程与其它命令都不受影响。
    //
    // 注意：probe 里既有同步 TCP 探测、也有 async 检查（p2p），所以在阻塞线程里
    // 用 handle.block_on 驱动一次轻量运行时是最直接的写法。
    let probes = tokio::task::spawn_blocking(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        match rt {
            Ok(rt) => rt.block_on(futures_util::future::join_all(
                SERVICE_DEFS.iter().map(|def| is_running(def)),
            )),
            // 建不出运行时（极少数资源受限场景）就退回逐个探测，行为仍然正确。
            Err(_) => SERVICE_DEFS
                .iter()
                .map(|def| futures_lite_blocking(def))
                .collect::<Vec<_>>(),
        }
    })
    .await
    .unwrap_or_default();
    let mut running_map = std::collections::HashMap::with_capacity(SERVICE_DEFS.len());
    for (def, running) in SERVICE_DEFS.iter().zip(probes) {
        running_map.insert(def.id, running);
    }

    let mut out = Vec::with_capacity(SERVICE_DEFS.len());
    for def in SERVICE_DEFS {
        let running = running_map.get(def.id).copied().unwrap_or(false);
        let autostart = !disabled.iter().any(|item| item == def.id);
        let status = if running {
            ServiceStatus::Running
        } else if autostart {
            ServiceStatus::Stopped
        } else {
            ServiceStatus::Disabled
        };
        // 会话池的说明由当前数量生成，比静态文案有用得多。
        let detail = match (def.count, def.detail) {
            (Some(count), _) => Some(format!("{} 个会话", count())),
            (None, Some(text)) => Some(text.to_string()),
            (None, None) => None,
        };
        out.push(ServiceInfo {
            id: def.id.to_string(),
            label: def.label.to_string(),
            description: def.description.to_string(),
            kind: def.kind.to_string(),
            group: def.group.to_string(),
            address: def.address.map(|value| value.to_string()),
            port: def.port,
            status,
            can_start: def.start.is_some(),
            can_stop: def.stop.is_some(),
            autostart,
            detail,
        });
    }
    out
}

/// 手动启动。会**清掉停用标记** —— 用户点了启动就是要它起来，不该下次还被偏好挡住。
#[tauri::command]
pub async fn service_start(id: String) -> Result<(), String> {
    let def = find_def(&id)?;
    let start = def
        .start
        .ok_or_else(|| format!("{} 不支持手动启动", def.label))?;
    set_autostart_flag(&id, true)?;
    info!(service = id, "[ServiceControl] 手动启动");
    start().await?;

    // 和重启一样把关：命令返回成功 = 端口上真的有人监听。否则报告「已启动」、
    // 用户随后发现接口不工作，比直接报错难排查得多。
    if let Some(port) = effective_port(def) {
        if !wait_for_port(port, true).await {
            return Err(format!(
                "{} 启动后 {} 秒内没有在端口 {port} 上监听，可能已被别的进程占用",
                def.label,
                START_LISTEN_TIMEOUT.as_secs()
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn service_stop(id: String) -> Result<(), String> {
    let def = find_def(&id)?;
    let stop = def
        .stop
        .ok_or_else(|| format!("{} 不支持停止", def.label))?;
    info!(service = id, "[ServiceControl] 停止");
    stop().await
}

/// 重启 = 停 → 等端口释放 → 起 → 等它真的在监听。
///
/// 三步都不能省：
/// - 中间那次等待：actix/子进程释放监听端口不是瞬时的，立刻重绑会撞上 `os error 10048`。
/// - 停止本身有上限：8888 上挂着 LSP / 浏览器预览这类长连接，actix 的优雅停机默认要等
///   30 秒，前端会一直转圈。上限由 `server::stop_*` 自己兜住。
/// - 启动后确认监听：`start()` 有的服务（actix）在「绑上端口」时就返回，有的（RSSHub、
///   uvicorn）是等健康检查通过才返回。这里是最后的把关 —— 没在监听就报错，
///   而不是给前端一个「已重启」的假成功（用户随后发现图床接口 404 正是这种假成功）。
#[tauri::command]
pub async fn service_restart(id: String) -> Result<(), String> {
    let def = find_def(&id)?;
    let stop = def
        .stop
        .ok_or_else(|| format!("{} 不支持停止", def.label))?;
    let start = def
        .start
        .ok_or_else(|| format!("{} 不支持启动", def.label))?;

    info!(service = id, "[ServiceControl] 重启：先停");
    stop().await?;

    // 等端口释放：最多 10 秒，一旦空出来就立刻继续，不必傻等固定时长。
    //
    // 没空出来通常意味着端口被**别的进程**占着（另一个实例、或别的软件）。这时继续
    // start 必然 10048 失败，报错信息也难懂；直接说清楚「谁占着」，用户才知道下一步做什么。
    if let Some(port) = effective_port(def) {
        if !wait_for_port(port, false).await {
            return Err(format!(
                "端口 {port} 在 {} 秒内没有释放，可能被其他进程占用（例如另一个 Guaxi 实例）。\
                 关掉占用它的程序后再重启 {}。",
                PORT_WAIT_TIMEOUT.as_secs(),
                def.label
            ));
        }
    } else {
        tokio::time::sleep(Duration::from_millis(600)).await;
    }

    info!(service = id, "[ServiceControl] 重启：再起");
    start().await?;

    // 有端口的服务：确认它真的在监听，否则把「假成功」变成一条可读的错误。
    if let Some(port) = effective_port(def) {
        if !wait_for_port(port, true).await {
            return Err(format!(
                "{} 已启动但 {} 秒内没有在端口 {port} 上监听，可能已被别的进程占用",
                def.label,
                START_LISTEN_TIMEOUT.as_secs()
            ));
        }
    }

    // 重启成功说明这个服务现在是用户要的，顺手清掉停用标记。
    let _ = set_autostart_flag(&id, true);
    Ok(())
}

/// 设置"是否随应用自启"。停用是持久化的：重启应用后不会自己再起来。
#[tauri::command]
pub fn service_set_autostart(id: String, enabled: bool) -> Result<(), String> {
    find_def(&id)?;
    set_autostart_flag(&id, enabled)
}
