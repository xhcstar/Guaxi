use crate::guaxiservices::bundled_resources;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{command, AppHandle, Emitter};
use tracing::{info, warn};
use uuid::Uuid;
use zip::ZipArchive;

#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;

static PY_WORKER_STATES: OnceLock<Mutex<HashMap<String, Arc<Mutex<PythonWorkerState>>>>> =
    OnceLock::new();

/// 当前常驻的 Python 执行会话数（设置页「服务器」用；每个会话是一个常驻子进程）。
pub fn active_python_worker_count() -> usize {
    PY_WORKER_STATES
        .get()
        .and_then(|store| store.lock().ok().map(|guard| guard.len()))
        .unwrap_or(0)
}
static PY_WORKER_ACTIVE_REQUESTS: OnceLock<Mutex<HashMap<String, PythonActiveRequest>>> =
    OnceLock::new();
// 当前前端发起的操作的进度回传通道。uv 对同一 venv 有文件锁、操作实际串行，
// 因此用单个全局 sink 就够；命令开始时设置、结束时（含出错）由守卫清理。
static ACTIVE_PROGRESS_SINK: OnceLock<Mutex<Option<ProgressSink>>> = OnceLock::new();

const PY_WORKER_FILE_NAME: &str = "guaxi_python_worker.py";
const PY_WORKER_SOURCE: &str = include_str!("../../scripts/guaxi_python_worker.py");
// 数学块「命令式工作区」引擎：作为真实 Python 模块随 worker 一起物化到同一目录，
// worker 启动时把该目录加入 sys.path，从而 `import guaximath_engine` 一次常驻会话。
const GUAXIMATH_ENGINE_FILE_NAME: &str = "guaximath_engine.py";
const GUAXIMATH_ENGINE_SOURCE: &str = include_str!("../../scripts/guaximath_engine.py");
const PY_WORKER_REQUEST_TIMEOUT_SECS: u64 = 45;
const PY_WORKER_EXEC_TIMEOUT_SECS: u64 = 300;
const PY_WORKER_TABLE_IO_TIMEOUT_SECS: u64 = 180;
const PY_WORKER_DEFAULT_KEY: &str = "__guaxi_default_worker__";

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_python_process_no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_python_process_no_window(_cmd: &mut Command) {}

fn python_worker_timeout_secs_for_cmd(worker_cmd: &str) -> u64 {
    match worker_cmd {
        "exec" => PY_WORKER_EXEC_TIMEOUT_SECS,
        "table_slice" | "table_export" | "import_table" => PY_WORKER_TABLE_IO_TIMEOUT_SECS,
        "extract_article" => PY_WORKER_TABLE_IO_TIMEOUT_SECS,
        _ => PY_WORKER_REQUEST_TIMEOUT_SECS,
    }
}

#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_VERSION: &str = "3.13.13";
#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_BASE_URL: &str = "https://www.python.org/ftp/python";
#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_USER_AGENT: &str = "guaxi-python-runtime-bootstrap/1.0";
#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_WINDOWS_PAGE_URL: &str = "https://www.python.org/downloads/windows/";
const GET_PIP_BOOTSTRAP_URL: &str = "https://bootstrap.pypa.io/get-pip.py";
const PYTHON_PIP_INDEX_URL: &str = "https://pypi.tuna.tsinghua.edu.cn/simple";
const PYTHON_PIP_TRUSTED_HOST: &str = "pypi.tuna.tsinghua.edu.cn";

// uv 供给：uv 二进制不进安装包，首次初始化时联网下载（与 python.org 下载对称）。
// 用 GitHub release 的 `latest/download/<asset>` 直链：asset 名跨版本稳定，且始终存在，
// 避免锁死一个将来可能失效的具体版本号导致整条供给链断掉。
#[cfg(windows)]
const UV_DOWNLOAD_LATEST_BASE_URL: &str = "https://github.com/astral-sh/uv/releases/latest/download";
// uv 创建托管 `.venv` 时默认使用的 Python 版本；只给到 minor，uv 会自动取该系列最新补丁版。
const UV_DEFAULT_PYTHON_VERSION: &str = "3.13";
// 用户主要在中国大陆：uv 二进制与 uv 托管的 python-build-standalone 都在 GitHub，直连很慢。
// 默认套一层 GitHub 加速代理前缀；可用环境变量 GUAXI_GITHUB_MIRROR 覆盖，置空表示直连。
// 下载均为“先镜像后直连”兜底，代理临时挂掉也不至于完全装不上。
const DEFAULT_GITHUB_MIRROR_PREFIX: &str = "https://ghfast.top/";
// python-build-standalone 的默认下载前缀；uv 用 UV_PYTHON_INSTALL_MIRROR 覆盖它做镜像。
const PYTHON_BUILD_STANDALONE_RELEASE_PREFIX: &str =
    "https://github.com/astral-sh/python-build-standalone/releases/download";
const PYTHON_RUNTIME_RECOMMENDED_PACKAGES: &[&str] = &[
    "pandas",
    "matplotlib",
    // Matplotlib 动画在没有系统 ffmpeg 时会回退到 PillowWriter；
    // imageio-ffmpeg 则让 notebook 动画优先稳定导出为 mp4，避免依赖用户机器环境。
    "Pillow",
    "imageio-ffmpeg",
    // Python cell 的动画捕获链路会直接识别 moviepy.VideoClip；
    // manim 则通过生成的媒体文件路径回传，默认安装后用户可直接在 cell 内创作动画。
    "moviepy",
    "manim",
    "openpyxl",
    "ipython",
    "plotly",
    "sympy",
    // 版本必须为 4.11.*：sympy 1.14 的 parse_latex 强制要求该主版本，
    // 装更新的 4.13 会导致 LaTeX 解析报 ImportError（数学块据此判定后端是否可用）。
    "antlr4-python3-runtime==4.11.1",
    "scipy",
    // 数据实验室「分析」工作台的关系/规律发现依赖：scikit-learn 提供降维(PCA/t-SNE)、
    // 聚类(KMeans/DBSCAN)、互信息、特征重要性；statsmodels 提供 Granger 因果、自相关/偏自相关。
    // 缺失时分析方法会给中文提示并降级，但默认安装让开箱即用。
    "scikit-learn",
    "statsmodels",
    "seaborn",
    // pandas 的 parquet / feather 导入导出需要 pyarrow 或 fastparquet。
    // 默认选择 pyarrow，覆盖 notebook 数据实验室和 Python cell 的表格工作流。
    "pyarrow",
    "duckdb",
    "polars",
    // worker 与前端的 JSON 协议序列化加速:装了走 orjson,没装 worker 自动回退标准库 json(行为不变)。
    "orjson",
    // Flipboard 正文抽取金标准引擎：默认安装，让"收集完整图文"开箱即用；
    // 未装时 flipboard_extract 也会后台自动 pip 安装并临时降级到 Rust scraper。
    "trafilatura",
    // ====== Guaxi Agent Server 依赖 ======
    // Agent 服务核心：FastAPI + SSE 流式通信
    "fastapi",
    "uvicorn[standard]",
    // LLM 路由：通过 openai 库统一调用所有支持 OpenAI 兼容 API 的模型
    "openai",
    // 网络搜索与抓取
    // Agent 服务会跟随系统/环境代理；当代理是 socks5:// 时，httpx 需要 socksio。
    "httpx[socks]",
    "socksio",
    "duckduckgo_search",
    "beautifulsoup4",
    "lxml",
    "markdownify",
];
const PYTHON_RUNTIME_BOOTSTRAP_PACKAGE_NAMES: &[&str] =
    &["pip", "setuptools", "wheel", "virtualenv"];

#[derive(Debug, Deserialize)]
pub struct PyExecRequest {
    pub code: String,
    pub persist: bool,
    pub expect_json: bool,
    pub request_id: Option<String>,
    pub include_session_vars: Option<bool>,
    pub session_var_names: Option<Vec<String>>,
    pub vars: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PyExecResponse {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub result_repr: Option<String>,
    pub result_json: Option<Value>,
    pub session_id: Option<String>,
    pub session_vars: Option<Vec<PythonSessionVar>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PythonSessionVar {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    pub preview: Option<String>,
    pub shape: Option<Vec<Value>>,
    pub length: Option<i64>,
    pub rows: Option<i64>,
    pub columns: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PythonRuntimeStatus {
    pub ready: bool,
    pub source: String,
    pub python_path: String,
    pub runtime_dir: String,
    pub version: String,
    pub pip_version: String,
    pub worker_alive: bool,
    pub packages: Vec<String>,
    pub bundled_runtime_present: bool,
    pub initialized: bool,
    pub worker_script_ready: bool,
    pub runtime_marker_path: String,
    pub bootstrap_download_available: bool,
    pub bootstrap_download_version: String,
    // 当前已就绪的 uv 版本（供工作台显示"包管理器: uv x.y.z"）；uv 未下载时为空。
    pub uv_version: String,
}

#[derive(Debug, Deserialize)]
pub struct PythonInstallPackagesRequest {
    pub packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PythonManagePackagesRequest {
    pub action: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PythonInstallPackagesResponse {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct PythonRuntimeInitResponse {
    pub runtime_dir: String,
    pub python_path: String,
    pub pip_version: String,
    pub download_version: String,
    pub used_download_bootstrap: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PythonRuntimeInitializeRequest {
    pub download_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PythonRuntimeDownloadVersion {
    pub version: String,
    pub series: String,
    pub release_date: String,
    pub archive_url: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
pub struct PythonRuntimeDownloadCatalog {
    pub versions: Vec<PythonRuntimeDownloadVersion>,
    pub default_version: String,
    pub download_source: String,
    pub bootstrap_download_available: bool,
}

#[derive(Debug, Serialize)]
pub struct PythonRuntimeArchiveInspection {
    pub archive_path: String,
    pub packaged_runtime_dir: String,
    pub python_entry_found: bool,
    pub candidate_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PythonImportTableRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct PythonSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct PythonCancelRequest {
    pub request_id: String,
    pub session_id: Option<String>,
    pub clear_session: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PythonSessionInfo {
    pub session_id: String,
    pub sandbox_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PythonImportTableResponse {
    pub ok: bool,
    pub error: Option<String>,
    pub table: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonTableRowsRequest {
    pub handle: String,
    pub session_id: Option<String>,
    pub table_key: Option<String>,
    pub manifest_path: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonTableRowsResponse {
    pub ok: bool,
    pub error: Option<String>,
    pub table_key: Option<String>,
    pub table_name: Option<String>,
    pub table_kind: Option<String>,
    pub columns: Option<Vec<String>>,
    pub rows: Option<Vec<Vec<Value>>>,
    pub row_count: Option<usize>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonTableExportRequest {
    pub handle: String,
    pub session_id: Option<String>,
    pub table_key: Option<String>,
    pub manifest_path: Option<String>,
    pub output_path: String,
    pub format: String,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonTableExportResponse {
    pub ok: bool,
    pub error: Option<String>,
    pub output_path: Option<String>,
    pub format: Option<String>,
    pub scope: Option<String>,
    pub table_count: Option<usize>,
}

#[derive(Debug, Clone)]
struct PythonLaunchSpec {
    program: String,
    base_args: Vec<String>,
    source: String,
    runtime_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PythonRuntimeMarker {
    python_path: String,
    source: String,
    pip_version: String,
    download_version: String,
    venv_dir: String,
    pip_config_path: String,
    pip_index_url: String,
    used_download_bootstrap: bool,
}

struct PythonWorkerState {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr_tail: Arc<Mutex<String>>,
}

#[derive(Debug, Clone)]
struct PythonActiveRequest {
    worker_key: String,
    session_id: String,
    cell_id: String,
    pid: u32,
}

fn python_worker_states_cell() -> &'static Mutex<HashMap<String, Arc<Mutex<PythonWorkerState>>>> {
    PY_WORKER_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn python_worker_active_requests_cell() -> &'static Mutex<HashMap<String, PythonActiveRequest>> {
    PY_WORKER_ACTIVE_REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn python_worker_key_for_session(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        PY_WORKER_DEFAULT_KEY.to_string()
    } else {
        format!("session::{trimmed}")
    }
}

fn python_worker_key_from_payload(payload: &Value) -> String {
    let session_id_from_vars = payload
        .get("vars")
        .and_then(Value::as_object)
        .and_then(|vars| vars.get("_guaxi_session"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let session_id = if session_id_from_vars.trim().is_empty() {
        payload
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
    } else {
        session_id_from_vars
    };
    python_worker_key_for_session(session_id)
}

fn python_runtime_dir() -> Result<PathBuf, String> {
    let dir = bundled_resources::ensure_relative_dir("utils/python_runtime")?;
    fs::create_dir_all(&dir).map_err(|e| format!("create python runtime dir failed: {e}"))?;
    Ok(dir)
}

/// 托管 Python runtime 目录（供其它模块把辅助脚本物化到与托管 venv 同一位置）。
pub fn managed_runtime_dir() -> Result<PathBuf, String> {
    python_runtime_dir()
}

#[cfg(windows)]
fn auto_download_python_available() -> bool {
    true
}

#[cfg(not(windows))]
fn auto_download_python_available() -> bool {
    false
}

#[cfg(windows)]
fn auto_download_python_version_label() -> &'static str {
    AUTO_DOWNLOAD_PYTHON_VERSION
}

#[cfg(not(windows))]
fn auto_download_python_version_label() -> &'static str {
    ""
}

#[cfg(windows)]
fn auto_download_python_archive_file_name_for_version(version: &str) -> String {
    // 设计说明：
    // 1. 这里需要下载的是 Windows embeddable package，而不是常规安装器同名资源。
    // 2. 旧文件名缺少 `-embed-`，会把下载结果指向错误资源，随后触发 ZIP 校验失败。
    // 3. 改成 embeddable ZIP 后，后续解压和创建 `.venv` 的流程才能正常继续。
    match std::env::consts::ARCH {
        "x86_64" => format!("python-{version}-embed-amd64.zip"),
        "aarch64" => format!("python-{version}-embed-arm64.zip"),
        _ => format!("python-{version}-embed-win32.zip"),
    }
}

#[cfg(windows)]
fn auto_download_python_archive_url_for_version(version: &str) -> String {
    format!(
        "{}/{}/{}",
        AUTO_DOWNLOAD_PYTHON_BASE_URL,
        version,
        auto_download_python_archive_file_name_for_version(version)
    )
}

#[cfg(windows)]
fn auto_download_python_install_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("Python")
}

#[cfg(windows)]
fn python_download_http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(30 * 60))
        .user_agent(AUTO_DOWNLOAD_PYTHON_USER_AGENT)
        .build()
        .map_err(|e| format!("create python download client failed: {e}"))
}

#[cfg(windows)]
fn download_file_to_path(url: &str, destination: &Path) -> Result<(), String> {
    let client = python_download_http_client()?;

    info!(
        url = url,
        destination = destination.to_string_lossy().as_ref(),
        "[PythonRuntime] downloading official python archive"
    );

    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("download python archive failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "download python archive failed with http status {}",
            response.status()
        ));
    }
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    // reqwest 在响应体被完整读取后，`content_length()` 可能反映为剩余可读字节数。
    // 这里必须在 copy_to 之前缓存服务端声明的长度，否则会把成功下载的文件误判成
    // “期望 0 字节，实际 N 字节”。
    let expected_len = response.content_length();
    info!(
        url = url,
        final_url = final_url.as_str(),
        content_type = content_type.as_str(),
        expected_len = expected_len.unwrap_or_default(),
        "[PythonRuntime] received python archive response headers"
    );

    let mut file = fs::File::create(destination)
        .map_err(|e| format!("create archive file failed: {e}"))?;
    response
        .copy_to(&mut file)
        .map_err(|e| format!("write archive file failed: {e}"))?;

    if let Some(expected_len) = expected_len {
        let actual_len = file
            .metadata()
            .map_err(|e| format!("stat downloaded archive failed: {e}"))?
            .len();
        if actual_len != expected_len {
            let _ = remove_path_if_exists(destination);
            return Err(format!(
                "downloaded archive size mismatch: expected {expected_len} bytes, got {actual_len} bytes"
            ));
        }
        info!(
            url = url,
            expected_len,
            actual_len,
            "[PythonRuntime] downloaded python archive size verified"
        );
    }
    Ok(())
}

#[cfg(windows)]
fn normalize_python_download_version(version: Option<&str>) -> Result<String, String> {
    let default_version = AUTO_DOWNLOAD_PYTHON_VERSION.to_string();
    let raw = version.map(|value| value.trim()).unwrap_or_default();
    if raw.is_empty() {
        return Ok(default_version);
    }
    let version_regex = Regex::new(r"^\d+\.\d+\.\d+$").map_err(|e| e.to_string())?;
    if !version_regex.is_match(raw) {
        return Err(format!("不合法的 Python 版本号：{raw}"));
    }
    Ok(raw.to_string())
}

#[cfg(windows)]
fn latest_compatible_python_download_version_from_windows_page() -> Result<String, String> {
    let client = python_download_http_client()?;
    let html = client
        .get(AUTO_DOWNLOAD_PYTHON_WINDOWS_PAGE_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("fetch python windows page failed: {e}"))?
        .text()
        .map_err(|e| format!("read python windows page failed: {e}"))?;
    let release_regex =
        Regex::new(r"Python\s+(3\.(?:13|12)\.\d+)\s+-\s+([A-Za-z]+\.*\s+\d{1,2},\s+\d{4})")
            .map_err(|e| e.to_string())?;
    for capture in release_regex.captures_iter(&html) {
        if let Some(version) = capture.get(1).map(|value| value.as_str().trim()) {
            return Ok(version.to_string());
        }
    }
    Err("无法从 Python Windows 下载页解析近期稳定版".to_string())
}

#[cfg(windows)]
fn resolve_requested_python_download_version(version: Option<&str>) -> Result<String, String> {
    let normalized = normalize_python_download_version(version)?;
    if version.is_some() {
        return Ok(normalized);
    }
    match latest_compatible_python_download_version_from_windows_page() {
        Ok(latest) => Ok(latest),
        Err(error) => {
            warn!(
                fallback = AUTO_DOWNLOAD_PYTHON_VERSION,
                error = error.as_str(),
                "[PythonRuntime] fallback to built-in default python download version"
            );
            Ok(normalized)
        }
    }
}

#[cfg(windows)]
fn python_download_cache_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".downloads")
}

#[cfg(windows)]
fn cached_python_archive_path(runtime_dir: &Path, version: &str) -> PathBuf {
    python_download_cache_dir(runtime_dir).join(auto_download_python_archive_file_name_for_version(version))
}

#[cfg(windows)]
fn cached_python_archive_is_valid(archive_path: &Path) -> bool {
    let Ok(file) = fs::File::open(archive_path) else {
        return false;
    };
    ZipArchive::new(file).is_ok()
}

#[cfg(windows)]
fn python_download_versions_fallback() -> PythonRuntimeDownloadCatalog {
    let default_version = AUTO_DOWNLOAD_PYTHON_VERSION.to_string();
    PythonRuntimeDownloadCatalog {
        versions: vec![PythonRuntimeDownloadVersion {
            version: default_version.clone(),
            series: "3.13".to_string(),
            release_date: "".to_string(),
            archive_url: auto_download_python_archive_url_for_version(&default_version),
            label: format!("Python {default_version}"),
            is_default: true,
        }],
        default_version,
        download_source: "fallback".to_string(),
        bootstrap_download_available: true,
    }
}

#[cfg(not(windows))]
fn python_download_versions_fallback() -> PythonRuntimeDownloadCatalog {
    PythonRuntimeDownloadCatalog {
        versions: Vec::new(),
        default_version: String::new(),
        download_source: "unsupported".to_string(),
        bootstrap_download_available: false,
    }
}

// 用 uv 自带的 python-build-standalone 版本清单构建可下载版本目录（跨平台，离线可用）。
// 只在 uv 已就绪时尝试，缺失则由调用方回退到旧的 python.org 目录。
fn fetch_python_download_versions_catalog_via_uv() -> Result<PythonRuntimeDownloadCatalog, String> {
    let runtime_dir = python_runtime_dir()?;
    let uv_path = existing_uv_binary(&runtime_dir).ok_or_else(|| "uv 尚未就绪".to_string())?;
    let mut command = uv_command(&uv_path, &runtime_dir);
    command.args(["python", "list", "--all-versions"]);
    let output = command_output_trimmed(command)?;

    // 解析所有 cpython-X.Y.Z-，按 minor 系列（3.9 / 3.10 / …）各取最新补丁版，
    // 供用户以“大版本”维度选择切换，而不是把同一系列几十个补丁版全铺出来。
    let release_regex = Regex::new(r"cpython-(\d+)\.(\d+)\.(\d+)-").map_err(|e| e.to_string())?;
    // series("3.13") -> 该系列见过的最高 (patch, "3.13.15")
    let mut latest_per_series: HashMap<String, (u64, String)> = HashMap::new();
    for capture in release_regex.captures_iter(&output) {
        let major: u64 = capture[1].parse().unwrap_or(0);
        let minor: u64 = capture[2].parse().unwrap_or(0);
        let patch: u64 = capture[3].parse().unwrap_or(0);
        // 只保留 3.9+ 系列：更老的版本 python-build-standalone 支持零散，且实际无人再用。
        if major != 3 || minor < 9 {
            continue;
        }
        let series = format!("{major}.{minor}");
        let full = format!("{major}.{minor}.{patch}");
        latest_per_series
            .entry(series)
            .and_modify(|slot| {
                if patch > slot.0 {
                    *slot = (patch, full.clone());
                }
            })
            .or_insert((patch, full));
    }
    if latest_per_series.is_empty() {
        return Err("uv python list 未解析到可用版本".to_string());
    }

    // 按 minor 降序排列：3.13、3.12、3.11…，最新的排在最前。
    let mut series_entries: Vec<(u64, String)> = latest_per_series
        .into_iter()
        .filter_map(|(series, (_, full))| {
            series
                .split('.')
                .nth(1)
                .and_then(|minor| minor.parse::<u64>().ok())
                .map(|minor| (minor, full))
        })
        .collect();
    series_entries.sort_by(|a, b| b.0.cmp(&a.0));

    // 只保留“默认系列 + 相邻各一个”，最多三个：默认 3.13 → 列出 3.14(若有且受支持) / 3.13 / 3.12。
    // 不把 uv 支持的所有系列全铺出来，避免选择项过多。
    let default_minor: u64 = UV_DEFAULT_PYTHON_VERSION
        .split('.')
        .nth(1)
        .and_then(|minor| minor.parse().ok())
        .unwrap_or(13);
    // 默认系列不在可用列表里（极端情况）时，以最新可用系列为中心。
    let center_minor = if series_entries.iter().any(|(minor, _)| *minor == default_minor) {
        default_minor
    } else {
        series_entries
            .first()
            .map(|(minor, _)| *minor)
            .unwrap_or(default_minor)
    };
    series_entries.retain(|(minor, _)| *minor + 1 >= center_minor && *minor <= center_minor + 1);

    let default_version = series_entries
        .iter()
        .find(|(minor, _)| *minor == center_minor)
        .or_else(|| series_entries.first())
        .map(|(_, full)| full.clone())
        .unwrap_or_default();

    let versions = series_entries
        .into_iter()
        .map(|(minor, version)| {
            let series = format!("3.{minor}");
            PythonRuntimeDownloadVersion {
                is_default: version == default_version,
                label: format!("Python {series} 系列（{version}）"),
                // uv 自行下载解释器，无需具体 archive URL。
                archive_url: String::new(),
                release_date: String::new(),
                series,
                version,
            }
        })
        .collect();

    Ok(PythonRuntimeDownloadCatalog {
        versions,
        default_version,
        download_source: "uv".to_string(),
        bootstrap_download_available: true,
    })
}

#[cfg(windows)]
fn fetch_python_download_versions_catalog() -> Result<PythonRuntimeDownloadCatalog, String> {
    let client = python_download_http_client()?;
    let html = client
        .get(AUTO_DOWNLOAD_PYTHON_WINDOWS_PAGE_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("fetch python windows page failed: {e}"))?
        .text()
        .map_err(|e| format!("read python windows page failed: {e}"))?;
    let release_regex =
        Regex::new(r"Python\s+(3\.(?:13|12)\.\d+)\s+-\s+([A-Za-z]+\.*\s+\d{1,2},\s+\d{4})")
            .map_err(|e| e.to_string())?;

    let mut seen = HashSet::new();
    let mut parsed_versions: Vec<(String, String)> = Vec::new();
    for capture in release_regex.captures_iter(&html) {
        let Some(version) = capture.get(1).map(|value| value.as_str().trim().to_string()) else {
            continue;
        };
        let Some(release_date) = capture.get(2).map(|value| value.as_str().trim().to_string()) else {
            continue;
        };
        if !seen.insert(version.clone()) {
            continue;
        }
        parsed_versions.push((version, release_date));
        if parsed_versions.len() >= 10 {
            break;
        }
    }

    if parsed_versions.is_empty() {
        return Err("未获取到可用的 Python 版本列表".to_string());
    }

    let default_version = parsed_versions
        .iter()
        .find(|(version, _)| version.starts_with("3.13."))
        .map(|(version, _)| version.clone())
        .unwrap_or_else(|| parsed_versions[0].0.clone());

    let versions = parsed_versions
        .into_iter()
        .map(|(version, release_date)| {
            let mut parts = version.split('.');
            let major = parts.next().unwrap_or("3");
            let minor = parts.next().unwrap_or("13");
            let series = format!("{major}.{minor}");
            PythonRuntimeDownloadVersion {
                archive_url: auto_download_python_archive_url_for_version(&version),
                label: format!("Python {version} · {release_date}"),
                is_default: version == default_version,
                version,
                series,
                release_date,
            }
        })
        .collect();

    Ok(PythonRuntimeDownloadCatalog {
        versions,
        default_version,
        download_source: AUTO_DOWNLOAD_PYTHON_WINDOWS_PAGE_URL.to_string(),
        bootstrap_download_available: true,
    })
}

#[cfg(windows)]
fn stage_downloaded_python_install(
    runtime_dir: &Path,
    install_dir: &Path,
    version: &str,
) -> Result<(), String> {
    let cache_dir = python_download_cache_dir(runtime_dir);
    let cached_archive = cached_python_archive_path(runtime_dir, version);
    let partial_archive = cache_dir.join(format!(
        "{}.partial",
        auto_download_python_archive_file_name_for_version(version)
    ));
    let stage_dir = runtime_dir.join(format!("Python.__downloading_{}", Uuid::new_v4()));
    let backup_dir = runtime_dir.join(format!("Python.__backup_{}", Uuid::new_v4()));

    fs::create_dir_all(&cache_dir).map_err(|e| format!("create python archive cache dir failed: {e}"))?;
    let _ = remove_path_if_exists(&partial_archive);
    let _ = remove_path_if_exists(&stage_dir);
    let _ = remove_path_if_exists(&backup_dir);

    if !cached_python_archive_is_valid(&cached_archive) {
        let _ = remove_path_if_exists(&cached_archive);
        let archive_url = auto_download_python_archive_url_for_version(version);
        info!(
            archive_url = archive_url.as_str(),
            archive_path = partial_archive.to_string_lossy().as_ref(),
            version = version,
            "[PythonRuntime] downloading fresh python archive because cache is missing or invalid"
        );
        download_file_to_path(&archive_url, &partial_archive)?;
        if !cached_python_archive_is_valid(&partial_archive) {
            warn!(
                archive_url = archive_url.as_str(),
                archive_path = partial_archive.to_string_lossy().as_ref(),
                version = version,
                "[PythonRuntime] downloaded python archive failed ZIP validation"
            );
            let _ = remove_path_if_exists(&partial_archive);
            return Err("下载得到的 Python ZIP 校验失败，文件不是有效压缩包".to_string());
        }
        fs::rename(&partial_archive, &cached_archive)
            .map_err(|e| format!("publish cached python archive failed: {e}"))?;
    } else {
        info!(
            archive = cached_archive.to_string_lossy().as_ref(),
            version = version,
            "[PythonRuntime] reuse cached official python archive"
        );
    }

    fs::create_dir_all(&stage_dir).map_err(|e| format!("create python stage dir failed: {e}"))?;
    extract_zip_to_dir(&cached_archive, &stage_dir)
        .map_err(|e| format!("extract downloaded python archive failed: {e}"))?;

    if !bundled_python_candidates(&stage_dir)
        .into_iter()
        .any(|candidate| candidate.is_file())
    {
        let _ = remove_path_if_exists(&stage_dir);
        return Err(format!(
            "解压后的 Python 目录校验失败，未找到解释器入口：{}",
            stage_dir.to_string_lossy()
        ));
    }

    let publish_result: Result<(), String> = (|| {
        if install_dir.exists() {
            fs::rename(install_dir, &backup_dir)
                .map_err(|e| format!("backup old python install failed: {e}"))?;
        }
        fs::rename(&stage_dir, install_dir)
            .map_err(|e| format!("publish downloaded python dir failed: {e}"))?;
        Ok(())
    })();

    match publish_result {
        Ok(_) => {
            let _ = remove_path_if_exists(&backup_dir);
            Ok(())
        }
        Err(error) => {
            let _ = remove_path_if_exists(&stage_dir);
            if backup_dir.exists() {
                let _ = fs::rename(&backup_dir, install_dir);
            }
            Err(error)
        }
    }
}

#[cfg(windows)]
fn ensure_downloaded_bundled_python(runtime_dir: &Path, version: Option<&str>) -> Result<String, String> {
    let resolved_version = resolve_requested_python_download_version(version)?;
    for candidate in bundled_python_candidates(runtime_dir) {
        if !candidate.is_file() {
            continue;
        }
        let candidate_str = candidate.to_string_lossy().to_string();
        match probe_python_program(&candidate_str, &[]) {
            Ok(version) => {
                info!(
                    program = candidate_str.as_str(),
                    version = version.as_str(),
                    "[PythonRuntime] reuse downloaded bundled python"
                );
                return Ok(resolved_version);
            }
            Err(error) => {
                warn!(
                    program = candidate_str.as_str(),
                    error = error.as_str(),
                    "[PythonRuntime] bundled python exists but probe failed, will reinstall"
                );
            }
        }
    }

    let install_dir = auto_download_python_install_dir(runtime_dir);
    // 设计说明：
    // 1. release 安装包不再内置整个 python_runtime 目录，避免显著放大安装体积。
    // 2. 当首次初始化时发现基础解释器缺失，这里直接从 python.org 拉取官方 ZIP 包，
    //    解压到 `utils/python_runtime/Python`，随后再在同目录下创建 `.venv`。
    // 3. 下载与发布都带有校验/回滚：先缓存 ZIP，再解压到临时目录，最后再覆盖正式目录，
    //    避免失败时把用户已有运行时目录破坏成半成品。
    info!(
        target_dir = install_dir.to_string_lossy().as_ref(),
        version = resolved_version.as_str(),
        archive_url = auto_download_python_archive_url_for_version(&resolved_version),
        "[PythonRuntime] prepare official python archive bootstrap"
    );

    stage_downloaded_python_install(runtime_dir, &install_dir, &resolved_version)?;

    let bundled_spec = resolve_bundled_python_base_launch_spec()?;
    info!(
        program = bundled_spec.program.as_str(),
        source = bundled_spec.source.as_str(),
        version = resolved_version.as_str(),
        "[PythonRuntime] official python archive downloaded and extracted successfully"
    );
    Ok(resolved_version)
}

// 当前生效的 GitHub 加速前缀：环境变量优先（置空即直连），否则用默认代理。
fn github_mirror_prefix() -> String {
    match std::env::var("GUAXI_GITHUB_MIRROR") {
        Ok(value) => value.trim().to_string(),
        Err(_) => DEFAULT_GITHUB_MIRROR_PREFIX.to_string(),
    }
}

// 给一个 GitHub 直链套上加速前缀；前缀为空则原样返回（直连）。
fn github_url_with_mirror(github_url: &str) -> String {
    let prefix = github_mirror_prefix();
    if prefix.is_empty() {
        github_url.to_string()
    } else if prefix.ends_with('/') {
        format!("{prefix}{github_url}")
    } else {
        format!("{prefix}/{github_url}")
    }
}

fn uv_install_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("uv")
}

fn uv_binary_path(runtime_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        uv_install_dir(runtime_dir).join("uv.exe")
    } else {
        uv_install_dir(runtime_dir).join("uv")
    }
}

// uv 托管的 Python 解释器与缓存都落在 python_runtime 目录内，
// 让整套运行时自成一体，不污染用户机器上的全局 uv 目录。
fn uv_python_install_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("uv-python")
}

fn uv_cache_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".uv-cache")
}

fn probe_uv_binary(uv_path: &Path) -> Result<String, String> {
    let mut command = Command::new(uv_path);
    command.arg("--version");
    command_output_trimmed(command)
}

/// 构造一个已配置好托管目录与无窗口标志的 uv 命令。
/// 调用方只需再 `.args([...])` 追加子命令即可。
fn uv_command(uv_path: &Path, runtime_dir: &Path) -> Command {
    let mut command = Command::new(uv_path);
    command
        .env("UV_PYTHON_INSTALL_DIR", uv_python_install_dir(runtime_dir))
        .env("UV_CACHE_DIR", uv_cache_dir(runtime_dir))
        // 允许 uv 在缺失时自动下载 python-build-standalone（联网供给，与用户选择一致）。
        .env("UV_PYTHON_DOWNLOADS", "automatic");
    // 中国大陆：让 uv 下载 Python 时也走 GitHub 加速代理（前缀为空则不设，走直连）。
    let mirror_prefix = github_mirror_prefix();
    if !mirror_prefix.is_empty() {
        command.env(
            "UV_PYTHON_INSTALL_MIRROR",
            github_url_with_mirror(PYTHON_BUILD_STANDALONE_RELEASE_PREFIX),
        );
    }
    apply_python_process_no_window(&mut command);
    command
}

// 与 uv_command 相同，但强制清掉 Python 下载镜像，用于镜像失败后直连 GitHub 兜底。
fn uv_command_without_python_mirror(uv_path: &Path, runtime_dir: &Path) -> Command {
    let mut command = uv_command(uv_path, runtime_dir);
    command.env_remove("UV_PYTHON_INSTALL_MIRROR");
    command
}

#[cfg(windows)]
fn uv_archive_file_name() -> &'static str {
    // GitHub release 里 Windows 资产名跨版本稳定。
    match std::env::consts::ARCH {
        "x86_64" => "uv-x86_64-pc-windows-msvc.zip",
        "aarch64" => "uv-aarch64-pc-windows-msvc.zip",
        _ => "uv-i686-pc-windows-msvc.zip",
    }
}

#[cfg(windows)]
fn download_uv_binary(runtime_dir: &Path) -> Result<PathBuf, String> {
    let install_dir = uv_install_dir(runtime_dir);
    let cache_dir = python_download_cache_dir(runtime_dir);
    fs::create_dir_all(&cache_dir).map_err(|e| format!("create uv download cache dir failed: {e}"))?;

    let archive_name = uv_archive_file_name();
    let direct_url = format!("{UV_DOWNLOAD_LATEST_BASE_URL}/{archive_name}");
    let mirror_url = github_url_with_mirror(&direct_url);
    let partial_archive = cache_dir.join(format!("{archive_name}.partial"));
    let stage_dir = runtime_dir.join(format!("uv.__downloading_{}", Uuid::new_v4()));
    let _ = remove_path_if_exists(&partial_archive);
    let _ = remove_path_if_exists(&stage_dir);

    // 中国大陆：先走 GitHub 加速代理，失败或校验不过再直连 GitHub 兜底。
    let mirror_ok = mirror_url != direct_url && {
        info!(
            url = mirror_url.as_str(),
            archive_path = partial_archive.to_string_lossy().as_ref(),
            "[PythonRuntime] downloading uv binary archive via github mirror"
        );
        download_file_to_path(&mirror_url, &partial_archive).is_ok()
            && cached_python_archive_is_valid(&partial_archive)
    };
    if !mirror_ok {
        let _ = remove_path_if_exists(&partial_archive);
        info!(
            url = direct_url.as_str(),
            archive_path = partial_archive.to_string_lossy().as_ref(),
            "[PythonRuntime] downloading uv binary archive directly from github"
        );
        download_file_to_path(&direct_url, &partial_archive)?;
        if !cached_python_archive_is_valid(&partial_archive) {
            let _ = remove_path_if_exists(&partial_archive);
            return Err("下载得到的 uv 压缩包校验失败，文件不是有效压缩包".to_string());
        }
    }

    fs::create_dir_all(&stage_dir).map_err(|e| format!("create uv stage dir failed: {e}"))?;
    extract_zip_to_dir(&partial_archive, &stage_dir)
        .map_err(|e| format!("extract uv archive failed: {e}"))?;

    // Windows msvc 压缩包把 uv.exe / uvx.exe 平铺在根目录。
    let staged_uv = stage_dir.join("uv.exe");
    if !staged_uv.is_file() {
        let _ = remove_path_if_exists(&stage_dir);
        let _ = remove_path_if_exists(&partial_archive);
        return Err(format!(
            "解压后的 uv 目录校验失败，未找到 uv.exe：{}",
            stage_dir.to_string_lossy()
        ));
    }

    let publish_result: Result<(), String> = (|| {
        remove_path_if_exists(&install_dir)?;
        fs::create_dir_all(&install_dir)
            .map_err(|e| format!("create uv install dir failed: {e}"))?;
        for entry in fs::read_dir(&stage_dir).map_err(|e| format!("read uv stage dir failed: {e}"))? {
            let entry = entry.map_err(|e| format!("read uv stage entry failed: {e}"))?;
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let target = install_dir.join(entry.file_name());
            fs::copy(entry.path(), &target)
                .map_err(|e| format!("publish uv binary failed: {e}"))?;
        }
        Ok(())
    })();

    let _ = remove_path_if_exists(&stage_dir);
    let _ = remove_path_if_exists(&partial_archive);
    publish_result?;

    let uv_path = uv_binary_path(runtime_dir);
    let version = probe_uv_binary(&uv_path)?;
    info!(
        uv_path = uv_path.to_string_lossy().as_ref(),
        version = version.as_str(),
        "[PythonRuntime] uv binary downloaded and verified"
    );
    Ok(uv_path)
}

/// 解析可用的 uv 二进制：本地已下载且可探测则复用，否则联网下载（仅 Windows）。
/// 非 Windows 尝试系统 PATH 上的 uv，缺失则明确报错。
fn resolve_uv_binary() -> Result<PathBuf, String> {
    let runtime_dir = python_runtime_dir()?;
    let uv_path = uv_binary_path(&runtime_dir);
    if uv_path.is_file() {
        match probe_uv_binary(&uv_path) {
            Ok(version) => {
                info!(
                    uv_path = uv_path.to_string_lossy().as_ref(),
                    version = version.as_str(),
                    "[PythonRuntime] reuse downloaded uv binary"
                );
                return Ok(uv_path);
            }
            Err(error) => warn!(
                uv_path = uv_path.to_string_lossy().as_ref(),
                error = error.as_str(),
                "[PythonRuntime] downloaded uv binary exists but probe failed, will re-download"
            ),
        }
    }

    #[cfg(windows)]
    {
        download_uv_binary(&runtime_dir)
    }
    #[cfg(not(windows))]
    {
        if can_probe_python_program("uv", &[]) {
            return Ok(PathBuf::from("uv"));
        }
        Err("未找到可用的 uv 二进制；当前平台不支持自动下载，请先手动安装 uv。".to_string())
    }
}

/// 只在 uv 已就绪时返回它，绝不触发下载。
/// 供包安装/列表等运行期读路径使用：这些操作发生在初始化之后，uv 通常已存在；
/// 缺失时应静默回退到 venv 自带 pip，而不是在读路径上突然联网下载。
fn existing_uv_binary(runtime_dir: &Path) -> Option<PathBuf> {
    let uv_path = uv_binary_path(runtime_dir);
    if uv_path.is_file() && probe_uv_binary(&uv_path).is_ok() {
        Some(uv_path)
    } else {
        None
    }
}

const PY_OP_LOG_EVENT: &str = "python-runtime-op-log";

// 一次前端操作的进度回传：把 uv/pip 子进程的输出按 opId 打回给前端。
struct ProgressSink {
    app: AppHandle,
    op_id: String,
}

fn active_progress_sink_cell() -> &'static Mutex<Option<ProgressSink>> {
    ACTIVE_PROGRESS_SINK.get_or_init(|| Mutex::new(None))
}

fn set_active_progress_sink(sink: Option<ProgressSink>) {
    if let Ok(mut guard) = active_progress_sink_cell().lock() {
        *guard = sink;
    }
}

// 作用域守卫：命令结束（正常或出错）时自动清空全局 sink，避免残留污染后续操作。
struct ProgressSinkGuard;
impl Drop for ProgressSinkGuard {
    fn drop(&mut self) {
        set_active_progress_sink(None);
    }
}

// 在闭包内设置 sink 并返回守卫；op_id 为空则不启用（后台静默操作不回传）。
fn begin_progress_scope(app: &AppHandle, op_id: Option<String>) -> Option<ProgressSinkGuard> {
    let op_id = op_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    set_active_progress_sink(Some(ProgressSink {
        app: app.clone(),
        op_id,
    }));
    Some(ProgressSinkGuard)
}

fn emit_progress_line(stream: &str, line: &str) {
    if let Ok(guard) = active_progress_sink_cell().lock() {
        if let Some(sink) = guard.as_ref() {
            let _ = sink.app.emit(
                PY_OP_LOG_EVENT,
                json!({ "opId": sink.op_id, "stream": stream, "line": line }),
            );
        }
    }
}

/// 运行命令并把 stdout/stderr 逐行回传给前端（若当前有活跃 sink），同时累积完整输出返回。
/// 无活跃 sink 时只累积、不回传，因此后台静默调用也能安全复用。
/// 返回 (成功, stdout, stderr)。调用方无需再设置 piped——本函数统一处理。
fn run_command_streamed(mut command: Command) -> Result<(bool, String, String), String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn streamed command failed: {e}"))?;

    let stdout_acc = Arc::new(Mutex::new(String::new()));
    let stderr_acc = Arc::new(Mutex::new(String::new()));

    let stdout_handle = child.stdout.take().map(|out| {
        let acc = Arc::clone(&stdout_acc);
        thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                emit_progress_line("stdout", &line);
                if let Ok(mut guard) = acc.lock() {
                    guard.push_str(&line);
                    guard.push('\n');
                }
            }
        })
    });
    let stderr_handle = child.stderr.take().map(|err| {
        let acc = Arc::clone(&stderr_acc);
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                emit_progress_line("stderr", &line);
                if let Ok(mut guard) = acc.lock() {
                    guard.push_str(&line);
                    guard.push('\n');
                }
            }
        })
    });

    let status = child
        .wait()
        .map_err(|e| format!("wait streamed command failed: {e}"))?;
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    let stdout_text = stdout_acc
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let stderr_text = stderr_acc
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    Ok((status.success(), stdout_text, stderr_text))
}

/// 针对托管 venv 执行一次包管理操作，优先用 uv pip（真正的解析器 + 更快），
/// uv 不在则回退到 venv 自带的 `python -m pip`。action 取
/// "install" | "upgrade" | "reinstall" | "uninstall"。返回 (成功, stdout, stderr)。
fn run_managed_package_op(
    spec: &PythonLaunchSpec,
    action: &str,
    packages: &[String],
) -> Result<(bool, String, String), String> {
    // 装/卸/升级包会改变运行时状态,使"就绪缓存"失效,下次 spawn 重新探测并刷新。
    invalidate_runtime_ready_cache();
    let command = match existing_uv_binary(&spec.runtime_dir) {
        Some(uv_path) => {
            let mut command = uv_command(&uv_path, &spec.runtime_dir);
            command.arg("pip");
            match action {
                "install" => {
                    command.arg("install");
                }
                "upgrade" => {
                    command.args(["install", "--upgrade"]);
                }
                // uv 用 `--reinstall`，没有 pip 的 `--force-reinstall`。
                "reinstall" => {
                    command.args(["install", "--reinstall"]);
                }
                "uninstall" => {
                    command.arg("uninstall");
                }
                _ => return Err(format!("不支持的 Python 包管理动作：{action}")),
            }
            command.args(packages);
            command.args(["--python", spec.program.as_str()]);
            if action != "uninstall" {
                command.args(["--index-url", PYTHON_PIP_INDEX_URL]);
            }
            command
        }
        None => {
            let mut command = Command::new(&spec.program);
            command.args(&spec.base_args).args(["-m", "pip"]);
            match action {
                "install" => {
                    command.arg("install");
                }
                "upgrade" => {
                    command.args(["install", "--upgrade"]);
                }
                "reinstall" => {
                    command.args(["install", "--upgrade", "--force-reinstall"]);
                }
                "uninstall" => {
                    command.args(["uninstall", "-y"]);
                }
                _ => return Err(format!("不支持的 Python 包管理动作：{action}")),
            }
            command.args(packages);
            apply_python_process_no_window(&mut command);
            command
        }
    };
    // 逐行流式回传输出（有活跃 sink 时），前端可实时看到安装/卸载日志。
    run_command_streamed(command)
}

/// 列出托管 venv 里已安装的包（pip list --format=json 的原始数组），
/// 优先 uv pip list，回退 `python -m pip list`。
fn managed_pip_list_json(spec: &PythonLaunchSpec) -> Vec<Value> {
    let command = match existing_uv_binary(&spec.runtime_dir) {
        Some(uv_path) => {
            let mut command = uv_command(&uv_path, &spec.runtime_dir);
            command.args([
                "pip",
                "list",
                "--format=json",
                "--python",
                spec.program.as_str(),
            ]);
            command
        }
        None => {
            let mut command = Command::new(&spec.program);
            command
                .args(&spec.base_args)
                .args(["-m", "pip", "list", "--format=json"]);
            apply_python_process_no_window(&mut command);
            command
        }
    };
    match command_output_trimmed(command) {
        Ok(output) => serde_json::from_str::<Vec<Value>>(&output)
            .ok()
            .unwrap_or_default(),
        Err(error) => {
            warn!(
                source = spec.source.as_str(),
                error = error.as_str(),
                "[PythonRuntime] list managed packages failed"
            );
            Vec::new()
        }
    }
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 删除托管 venv 目录，带重试：刚 kill 掉 worker 后，Windows 可能还没释放
/// `.venv/Scripts/python.exe` 等文件句柄，`remove_dir_all` 会短暂失败（os error 32）。
/// 重试几次给系统释放句柄的时间，避免留下删一半的半成品目录。
fn remove_venv_dir_with_retry(venv_dir: &Path) -> Result<(), String> {
    if !venv_dir.exists() {
        return Ok(());
    }
    let mut last_err = String::new();
    for attempt in 0..20 {
        match fs::remove_dir_all(venv_dir) {
            Ok(_) => return Ok(()),
            Err(error) => {
                last_err = error.to_string();
                if !venv_dir.exists() {
                    return Ok(());
                }
                if attempt < 19 {
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
    Err(format!(
        "清理旧虚拟环境失败（可能有进程仍占用文件）：{} ({last_err})",
        venv_dir.to_string_lossy()
    ))
}

fn extract_zip_to_dir(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let out_path = target_dir.join(entry.mangled_name());
        if entry.name().ends_with('/') {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if dst_path.exists() {
                let _ = fs::remove_file(&dst_path);
            }
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn find_dir_by_name(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }
    if root
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(name))
        .unwrap_or(false)
    {
        return Some(root.to_path_buf());
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_dir_by_name(&path, name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn bundled_python_candidates(runtime_dir: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            runtime_dir.join("python.exe"),
            runtime_dir.join("Python").join("python.exe"),
            runtime_dir.join("embedded").join("python.exe"),
        ]
    } else {
        vec![
            runtime_dir.join("bin").join("python3"),
            runtime_dir.join("bin").join("python"),
            runtime_dir.join("python"),
        ]
    }
}

fn managed_venv_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(".venv")
}

#[cfg(windows)]
fn bootstrap_runtime_artifact_paths(runtime_dir: &Path) -> Vec<PathBuf> {
    vec![
        auto_download_python_install_dir(runtime_dir),
        python_download_cache_dir(runtime_dir),
    ]
}

#[cfg(not(windows))]
fn bootstrap_runtime_artifact_paths(_runtime_dir: &Path) -> Vec<PathBuf> {
    Vec::new()
}

fn managed_python_candidates(runtime_dir: &Path) -> Vec<PathBuf> {
    let venv_dir = managed_venv_dir(runtime_dir);
    if cfg!(windows) {
        vec![
            venv_dir.join("Scripts").join("python.exe"),
            venv_dir.join("python.exe"),
        ]
    } else {
        vec![
            venv_dir.join("bin").join("python"),
            venv_dir.join("bin").join("python3"),
        ]
    }
}

fn managed_pip_config_path(runtime_dir: &Path) -> PathBuf {
    let venv_dir = managed_venv_dir(runtime_dir);
    if cfg!(windows) {
        venv_dir.join("pip.ini")
    } else {
        venv_dir.join("pip.conf")
    }
}

fn python_session_sandbox_root(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("sessions")
}

fn sanitize_session_label(session_id: &str) -> String {
    let stem = Path::new(session_id)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("session");
    let mut sanitized = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "session".to_string()
    } else {
        trimmed.to_string()
    }
}

fn python_session_hash(session_id: &str) -> String {
    let mut hasher = DefaultHasher::new();
    session_id.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn python_session_sandbox_dir(runtime_dir: &Path, session_id: &str) -> PathBuf {
    let label = sanitize_session_label(session_id);
    let hash = python_session_hash(session_id);
    python_session_sandbox_root(runtime_dir).join(format!("{label}-{hash}"))
}

fn ensure_python_session_sandbox_dir(session_id: &str) -> Result<PathBuf, String> {
    let runtime_dir = python_runtime_dir()?;
    let sandbox_root = python_session_sandbox_root(&runtime_dir);
    fs::create_dir_all(&sandbox_root)
        .map_err(|e| format!("create python session sandbox root failed: {e}"))?;
    let sandbox_dir = python_session_sandbox_dir(&runtime_dir, session_id);
    fs::create_dir_all(&sandbox_dir)
        .map_err(|e| format!("create python session sandbox dir failed: {e}"))?;
    Ok(sandbox_dir)
}

fn ensure_managed_pip_mirror(runtime_dir: &Path) -> Result<(), String> {
    let venv_dir = managed_venv_dir(runtime_dir);
    fs::create_dir_all(&venv_dir).map_err(|e| format!("create managed venv dir failed: {e}"))?;

    let config_path = managed_pip_config_path(runtime_dir);
    let desired = concat!(
        "[global]\n",
        "index-url = https://pypi.tuna.tsinghua.edu.cn/simple\n",
        "\n",
        "[install]\n",
        "trusted-host = pypi.tuna.tsinghua.edu.cn\n",
    );

    let needs_write = match fs::read_to_string(&config_path) {
        Ok(existing) => existing != desired,
        Err(_) => true,
    };

    if needs_write {
        fs::write(&config_path, desired)
            .map_err(|e| format!("write managed pip mirror config failed: {e}"))?;
        info!(
            config_path = config_path.to_string_lossy().as_ref(),
            "[PythonRuntime] refreshed managed pip mirror config"
        );
    } else {
        info!(
            config_path = config_path.to_string_lossy().as_ref(),
            "[PythonRuntime] reuse existing managed pip mirror config"
        );
    }

    Ok(())
}

fn runtime_marker_path(runtime_dir: &Path) -> PathBuf {
    // runtime.json 作为桌面端“已完成初始化”的稳定标记，方便前端和后续模块复用。
    runtime_dir.join("runtime.json")
}

fn runtime_is_initialized(runtime_dir: &Path) -> bool {
    runtime_marker_path(runtime_dir).is_file()
}

fn read_runtime_marker(runtime_dir: &Path) -> Result<Option<PythonRuntimeMarker>, String> {
    let marker_path = runtime_marker_path(runtime_dir);
    if !marker_path.is_file() {
        return Ok(None);
    }

    let marker_content = fs::read_to_string(&marker_path)
        .map_err(|e| format!("read python runtime marker failed: {e}"))?;
    let marker = serde_json::from_str::<PythonRuntimeMarker>(&marker_content)
        .map_err(|e| format!("decode python runtime marker failed: {e}"))?;
    Ok(Some(marker))
}

fn write_runtime_marker(
    spec: &PythonLaunchSpec,
    pip_version: &str,
    download_version: &str,
    used_download_bootstrap: bool,
) -> Result<PythonRuntimeMarker, String> {
    let marker = PythonRuntimeMarker {
        python_path: spec.program.clone(),
        source: spec.source.clone(),
        pip_version: pip_version.to_string(),
        download_version: download_version.to_string(),
        venv_dir: managed_venv_dir(&spec.runtime_dir)
            .to_string_lossy()
            .to_string(),
        pip_config_path: managed_pip_config_path(&spec.runtime_dir)
            .to_string_lossy()
            .to_string(),
        pip_index_url: PYTHON_PIP_INDEX_URL.to_string(),
        used_download_bootstrap,
    };

    let marker_path = runtime_marker_path(&spec.runtime_dir);
    fs::write(
        &marker_path,
        serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write python runtime marker failed: {e}"))?;

    info!(
        marker_path = marker_path.to_string_lossy().as_ref(),
        source = marker.source.as_str(),
        used_download_bootstrap = marker.used_download_bootstrap,
        "[PythonRuntime] runtime marker updated"
    );

    Ok(marker)
}

#[cfg(windows)]
fn did_use_download_bootstrap(runtime_dir: &Path, base_source: &str) -> bool {
    base_source == "bundled" && auto_download_python_install_dir(runtime_dir).is_dir()
}

#[cfg(not(windows))]
fn did_use_download_bootstrap(_runtime_dir: &Path, _base_source: &str) -> bool {
    false
}

fn worker_script_ready(runtime_dir: &Path) -> bool {
    runtime_dir.join(PY_WORKER_FILE_NAME).is_file()
}

fn inspect_runtime_archive_path(zip_path: &Path) -> Result<PythonRuntimeArchiveInspection, String> {
    // 安装前先做一次轻量检查，避免把不完整压缩包直接覆盖到 utils/python_runtime。
    let exe_root = exe_dir()?;
    let temp_dir = exe_root
        .join("tmp")
        .join(format!("python_runtime_inspect_{}", Uuid::new_v4()));

    remove_path_if_exists(&temp_dir)?;
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let result: Result<PythonRuntimeArchiveInspection, String> = (|| {
        info!(
            archive_path = zip_path.to_string_lossy().as_ref(),
            temp_dir = temp_dir.to_string_lossy().as_ref(),
            "[PythonRuntime] inspecting runtime archive"
        );

        extract_zip_to_dir(zip_path, &temp_dir)?;
        let packaged_runtime_dir = find_dir_by_name(&temp_dir, "python_runtime", 5)
            .unwrap_or_else(|| temp_dir.clone());
        let candidate_count = bundled_python_candidates(&packaged_runtime_dir)
            .into_iter()
            .filter(|candidate| candidate.is_file())
            .count();

        let mut warnings = Vec::new();
        if candidate_count == 0 {
            warnings.push("压缩包内未检测到可用的 Python 解释器入口。".to_string());
        }
        if packaged_runtime_dir == temp_dir {
            warnings.push("未在压缩包内找到 python_runtime 目录，将直接使用压缩包根目录。".to_string());
        }

        Ok(PythonRuntimeArchiveInspection {
            archive_path: zip_path.to_string_lossy().to_string(),
            packaged_runtime_dir: packaged_runtime_dir.to_string_lossy().to_string(),
            python_entry_found: candidate_count > 0,
            candidate_count,
            warnings,
        })
    })();

    let _ = remove_path_if_exists(&temp_dir);
    result
}

fn command_output_trimmed(mut command: Command) -> Result<String, String> {
    // 设计说明：
    // 1. Python 工作台初始化、状态探测、pip 探测都会频繁走 `Command::output()`。
    // 2. Windows release 下如果不显式设置 `CREATE_NO_WINDOW`，这些短命令会不断闪出控制台窗口；
    //    dev 模式通常不明显，但打包后体验会很差。
    // 3. 这里统一在公共出口加一层无窗口配置，避免后续新增探测命令时再次漏配。
    apply_python_process_no_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("spawn process failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            #[cfg(windows)]
            {
                if output.status.code() == Some(0xc0000135_u32 as i32) {
                    warn!(
                        status = output.status.code().unwrap_or_default(),
                        "[PythonRuntime] process exited with STATUS_DLL_NOT_FOUND"
                    );
                    format!(
                        "process exited with status {} (STATUS_DLL_NOT_FOUND: 内置 Python 依赖的 DLL 未找到，请检查 utils/python_runtime/Python 与 .venv 是否完整)",
                        output.status
                    )
                } else {
                    format!("process exited with status {}", output.status)
                }
            }
            #[cfg(not(windows))]
            {
                format!("process exited with status {}", output.status)
            }
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn probe_python_program(program: &str, base_args: &[String]) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(base_args).arg("--version");
    command_output_trimmed(command)
}

fn can_probe_python_program(program: &str, base_args: &[String]) -> bool {
    probe_python_program(program, base_args).is_ok()
}

fn resolve_bundled_python_base_launch_spec() -> Result<PythonLaunchSpec, String> {
    let runtime_dir = python_runtime_dir()?;

    // 设计说明：
    // 1. Python 会话运行时要求真正独立，`.venv` 必须永远从软件目录内置的基础解释器创建。
    // 2. 因此这里故意不再接受 GUAXI_PYTHON / 系统 PATH 作为“托管虚拟环境的创建来源”，
    //    避免系统 Python 重装后把会话环境重新绑回外部解释器。
    // 3. 只要 bundled runtime 缺失，就明确返回错误，提示先安装/修复内置 Python 运行时。
    for candidate in bundled_python_candidates(&runtime_dir) {
        if !candidate.is_file() {
            continue;
        }
        let candidate_str = candidate.to_string_lossy().to_string();
        match probe_python_program(&candidate_str, &[]) {
            Ok(version) => {
                info!(
                    program = candidate_str.as_str(),
                    runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                    version = version.as_str(),
                    "[PythonRuntime] resolved bundled base python"
                );
                return Ok(PythonLaunchSpec {
                    program: candidate_str,
                    base_args: Vec::new(),
                    source: "bundled".to_string(),
                    runtime_dir,
                });
            }
            Err(error) => {
                warn!(
                    program = candidate_str.as_str(),
                    runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                    error = error.as_str(),
                    "[PythonRuntime] skip bundled runtime candidate because executable probe failed"
                );
            }
        }
    }

    Err(format!(
        "未找到可用的内置 Python 基础运行时，请先安装或修复目录：{}",
        runtime_dir.to_string_lossy()
    ))
}

fn resolve_bootstrap_python_base_launch_spec(download_version: Option<&str>) -> Result<PythonLaunchSpec, String> {
    match resolve_bundled_python_base_launch_spec() {
        Ok(spec) => Ok(spec),
        Err(original_error) => {
            if auto_download_python_available() {
                #[cfg(windows)]
                {
                    let runtime_dir = python_runtime_dir()?;
                    warn!(
                        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                        error = original_error.as_str(),
                        "[PythonRuntime] bundled runtime missing, fallback to official python download bootstrap"
                    );
                    let _ = ensure_downloaded_bundled_python(&runtime_dir, download_version)?;
                    return resolve_bundled_python_base_launch_spec();
                }
            }

            Err(original_error)
        }
    }
}

fn resolve_python_base_launch_spec_internal(prefer_managed_runtime: bool) -> Result<PythonLaunchSpec, String> {
    let runtime_dir = python_runtime_dir()?;

    // 设计说明：
    // 1. Guaxi 的 Python 工作台目标是优先使用软件目录里的托管运行时，而不是系统 Python。
    // 2. 旧顺序只先看 bundled python.exe，再看 GUAXI_PYTHON / 系统 PATH；如果 runtime_dir 里只有 `.venv`，
    //    状态查询会被外部解释器“抢走”，甚至在 GUAXI_PYTHON 失效时直接报错。
    // 3. 这里把 `.venv` 提升到最高优先级，只要软件目录下已有托管虚拟环境，就始终以它为准。
    if prefer_managed_runtime {
        for candidate in managed_python_candidates(&runtime_dir) {
            if !candidate.is_file() {
                continue;
            }
            let candidate_str = candidate.to_string_lossy().to_string();
            match probe_python_program(&candidate_str, &[]) {
                Ok(version) => {
                    info!(
                        program = candidate_str.as_str(),
                        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                        version = version.as_str(),
                        "[PythonRuntime] resolved base python from managed runtime"
                    );
                    return Ok(PythonLaunchSpec {
                        program: candidate_str,
                        base_args: Vec::new(),
                        source: "managed-runtime".to_string(),
                        runtime_dir,
                    });
                }
                Err(error) => {
                    warn!(
                        program = candidate_str.as_str(),
                        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                        error = error.as_str(),
                        "[PythonRuntime] skip broken managed runtime candidate because executable probe failed"
                    );
                }
            }
        }
    }

    for candidate in bundled_python_candidates(&runtime_dir) {
        if !candidate.is_file() {
            continue;
        }
        let candidate_str = candidate.to_string_lossy().to_string();
        match probe_python_program(&candidate_str, &[]) {
            Ok(version) => {
                info!(
                    program = candidate_str.as_str(),
                    runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                    version = version.as_str(),
                    "[PythonRuntime] resolved base python from bundled runtime"
                );
                return Ok(PythonLaunchSpec {
                    program: candidate_str,
                    base_args: Vec::new(),
                    source: "bundled".to_string(),
                    runtime_dir,
                });
            }
            Err(error) => {
                warn!(
                    program = candidate_str.as_str(),
                    runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                    error = error.as_str(),
                    "[PythonRuntime] skip bundled runtime candidate because executable probe failed"
                );
            }
        }
    }

    if let Ok(custom) = std::env::var("GUAXI_PYTHON") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            if can_probe_python_program(trimmed, &[]) {
                info!(
                    program = trimmed,
                    "[PythonRuntime] resolved base python from GUAXI_PYTHON"
                );
                return Ok(PythonLaunchSpec {
                    program: trimmed.to_string(),
                    base_args: Vec::new(),
                    source: "env".to_string(),
                    runtime_dir,
                });
            }

            warn!(
                program = trimmed,
                "[PythonRuntime] ignore invalid GUAXI_PYTHON because executable probe failed"
            );
        }
    }

    let fallbacks: Vec<(String, Vec<String>, &str)> = if cfg!(windows) {
        vec![
            ("py".to_string(), vec!["-3".to_string()], "system-py-launcher"),
            ("python".to_string(), Vec::new(), "system-python"),
            ("python3".to_string(), Vec::new(), "system-python3"),
        ]
    } else {
        vec![
            ("python3".to_string(), Vec::new(), "system-python3"),
            ("python".to_string(), Vec::new(), "system-python"),
        ]
    };

    for (program, base_args, source) in fallbacks {
        if can_probe_python_program(&program, &base_args) {
            info!(
                program = program.as_str(),
                source = source,
                "[PythonRuntime] resolved base python from fallback"
            );
            return Ok(PythonLaunchSpec {
                program,
                base_args,
                source: source.to_string(),
                runtime_dir,
            });
        }
    }

    Err("未找到可用的 Python 解释器；请先提供 utils/python_runtime 或安装系统 Python。".to_string())
}

fn is_missing_venv_module_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("no module named venv") || normalized.contains("modulenotfounderror")
}

#[cfg(windows)]
fn bundled_python_pth_file(runtime_dir: &Path) -> Option<PathBuf> {
    let install_dir = auto_download_python_install_dir(runtime_dir);
    fs::read_dir(&install_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case("_pth"))
                .unwrap_or(false)
        })
}

#[cfg(not(windows))]
fn bundled_python_pth_file(_runtime_dir: &Path) -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn enable_site_for_bundled_python(runtime_dir: &Path) -> Result<(), String> {
    let Some(pth_path) = bundled_python_pth_file(runtime_dir) else {
        return Ok(());
    };
    let content = fs::read_to_string(&pth_path)
        .map_err(|e| format!("read bundled python _pth file failed: {e}"))?;
    if content.lines().any(|line| line.trim() == "import site") {
        return Ok(());
    }

    let mut changed = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "#import site" || trimmed == "# import site" {
            lines.push("import site".to_string());
            changed = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !changed {
        lines.push("import site".to_string());
    }
    let next = lines.join("\n");
    fs::write(&pth_path, next).map_err(|e| format!("write bundled python _pth file failed: {e}"))?;
    info!(
        pth_path = pth_path.to_string_lossy().as_ref(),
        "[PythonRuntime] enabled import site for bundled python"
    );
    Ok(())
}

#[cfg(not(windows))]
fn enable_site_for_bundled_python(_runtime_dir: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn bundled_get_pip_script_path(runtime_dir: &Path) -> PathBuf {
    python_download_cache_dir(runtime_dir).join("get-pip.py")
}

#[cfg(windows)]
fn ensure_bundled_bootstrap_pip(base_spec: &PythonLaunchSpec) -> Result<(), String> {
    enable_site_for_bundled_python(&base_spec.runtime_dir)?;

    let mut pip_check = Command::new(&base_spec.program);
    pip_check.args(&base_spec.base_args).args(["-m", "pip", "--version"]);
    if command_output_trimmed(pip_check).is_ok() {
        return Ok(());
    }

    let cache_dir = python_download_cache_dir(&base_spec.runtime_dir);
    fs::create_dir_all(&cache_dir).map_err(|e| format!("create bundled bootstrap cache dir failed: {e}"))?;
    let get_pip_path = bundled_get_pip_script_path(&base_spec.runtime_dir);
    if !get_pip_path.is_file() {
        info!(
            url = GET_PIP_BOOTSTRAP_URL,
            destination = get_pip_path.to_string_lossy().as_ref(),
            "[PythonRuntime] downloading get-pip bootstrap script for bundled python"
        );
        download_file_to_path(GET_PIP_BOOTSTRAP_URL, &get_pip_path)?;
    }

    let mut command = Command::new(&base_spec.program);
    command
        .args(&base_spec.base_args)
        .arg(&get_pip_path)
        .args([
            "--disable-pip-version-check",
            "--no-warn-script-location",
            "-i",
            PYTHON_PIP_INDEX_URL,
            "--trusted-host",
            PYTHON_PIP_TRUSTED_HOST,
        ])
        .current_dir(&base_spec.runtime_dir);
    let _ = command_output_trimmed(command)
        .map_err(|e| format!("bootstrap bundled python pip failed: {e}"))?;
    info!("[PythonRuntime] bootstrapped pip for bundled python");
    Ok(())
}

#[cfg(not(windows))]
fn ensure_bundled_bootstrap_pip(_base_spec: &PythonLaunchSpec) -> Result<(), String> {
    Ok(())
}

fn ensure_bundled_virtualenv(base_spec: &PythonLaunchSpec) -> Result<(), String> {
    ensure_bundled_bootstrap_pip(base_spec)?;

    let mut virtualenv_check = Command::new(&base_spec.program);
    virtualenv_check
        .args(&base_spec.base_args)
        .args(["-m", "virtualenv", "--version"]);
    if command_output_trimmed(virtualenv_check).is_ok() {
        return Ok(());
    }

    let mut command = Command::new(&base_spec.program);
    command
        .args(&base_spec.base_args)
        .args([
            "-m",
            "pip",
            "install",
            "--upgrade",
            "pip",
            "setuptools",
            "wheel",
            "virtualenv",
            "-i",
            PYTHON_PIP_INDEX_URL,
            "--trusted-host",
            PYTHON_PIP_TRUSTED_HOST,
        ])
        .current_dir(&base_spec.runtime_dir);
    let _ = command_output_trimmed(command)
        .map_err(|e| format!("install bundled python bootstrap packages failed: {e}"))?;
    info!("[PythonRuntime] installed virtualenv for bundled python");
    Ok(())
}

fn create_managed_virtualenv(base_spec: &PythonLaunchSpec, venv_dir: &Path) -> Result<(), String> {
    let mut command = Command::new(&base_spec.program);
    command
        .args(&base_spec.base_args)
        .args(["-m", "venv"])
        .arg(venv_dir)
        .current_dir(&base_spec.runtime_dir);
    match command_output_trimmed(command) {
        Ok(_) => Ok(()),
        Err(error) => {
            if base_spec.source == "bundled" && is_missing_venv_module_error(&error) {
                warn!(
                    source = base_spec.source.as_str(),
                    error = error.as_str(),
                    "[PythonRuntime] bundled python is missing venv module, bootstrap virtualenv instead"
                );
                ensure_bundled_virtualenv(base_spec)?;
                let mut fallback_command = Command::new(&base_spec.program);
                fallback_command
                    .args(&base_spec.base_args)
                    .args(["-m", "virtualenv"])
                    .arg(venv_dir)
                    .current_dir(&base_spec.runtime_dir);
                let _ = command_output_trimmed(fallback_command)
                    .map_err(|e| format!("create Guaxi python virtualenv via virtualenv failed: {e}"))?;
                return Ok(());
            }
            Err(format!("create Guaxi python virtualenv failed: {error}"))
        }
    }
}

/// 用 uv 创建托管 `.venv`：先装目标 Python（缺失时 uv 联网下载 python-build-standalone），
/// 再 `uv venv --seed`。`--seed` 让 venv 自带 pip，任何仍走 `python -m pip` 的旧路径都不会断。
/// 产出的 `.venv/Scripts/python.exe` 结构与旧 `python -m venv` 完全一致，执行层无感。
fn create_managed_virtualenv_with_uv(
    uv_path: &Path,
    runtime_dir: &Path,
    venv_dir: &Path,
    version: &str,
) -> Result<(), String> {
    // 流式执行：切换版本/首次建环境时，前端能实时看到下载解释器与建环境的进度。
    let mut install_cmd = uv_command(uv_path, runtime_dir);
    install_cmd.args(["python", "install", version]);
    let (install_ok, _, install_err) = run_command_streamed(install_cmd)?;
    if !install_ok {
        // 镜像装不上（代理临时挂了等）时直连 GitHub 再试一次。
        warn!(
            version,
            error = install_err.trim(),
            "[PythonRuntime] uv install python via mirror failed, retry direct github"
        );
        let mut direct_cmd = uv_command_without_python_mirror(uv_path, runtime_dir);
        direct_cmd.args(["python", "install", version]);
        let (direct_ok, _, direct_err) = run_command_streamed(direct_cmd)?;
        if !direct_ok {
            return Err(format!("uv install python {version} failed: {}", direct_err.trim()));
        }
    }

    // 先杀掉常驻 worker 再删目录：Windows 上正在运行的 python worker 会占用
    // `.venv/Scripts/python.exe`，导致删除只删掉一部分、留下缺 pyvenv.cfg 的半成品，
    // 随后 uv 就会报 “will not clear a directory that is not a virtual environment”。
    let _ = clear_python_worker();
    // 自己删掉目标目录再建，而不是依赖 uv 的 `--clear`：`--clear` 只肯清“它认得的 venv”，
    // 遇到缺 pyvenv.cfg 的半成品目录会直接拒绝。到这一步意味着调用方已判定要创建/重建
    // （缺失、损坏、换版本），残留目录一律该覆盖，因此主动 remove，让本步骤对任何残留状态幂等。
    remove_venv_dir_with_retry(venv_dir)?;
    let mut venv_cmd = uv_command(uv_path, runtime_dir);
    venv_cmd
        .args(["venv", "--seed", "--python", version])
        .arg(venv_dir)
        .current_dir(runtime_dir);
    let (venv_ok, _, venv_err) = run_command_streamed(venv_cmd)?;
    if !venv_ok {
        return Err(format!("uv venv (python {version}) failed: {}", venv_err.trim()));
    }
    Ok(())
}

/// 现有托管 venv 的 Python 版本是否匹配请求版本。
/// 请求 "3.13" 能匹配当前 "3.13.13"（同系列）；请求 "3.12" 对 "3.13.13" 则不匹配，触发版本切换重建。
fn managed_python_version_matches(spec: &PythonLaunchSpec, requested: &str) -> bool {
    let mut command = Command::new(&spec.program);
    command.args(&spec.base_args).arg("--version");
    match command_output_trimmed(command) {
        Ok(output) => {
            let current = output
                .trim()
                .strip_prefix("Python")
                .map(str::trim)
                .unwrap_or_else(|| output.trim());
            let req = requested.trim();
            current == req || current.starts_with(&format!("{req}."))
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn should_copy_windows_runtime_support_file(file_name: &str) -> bool {
    let normalized = file_name.trim().to_ascii_lowercase();
    (normalized.starts_with("python")
        && (normalized.ends_with(".dll") || normalized.ends_with(".zip")))
        || (normalized.starts_with("vcruntime") && normalized.ends_with(".dll"))
        || (normalized.starts_with("ucrtbase") && normalized.ends_with(".dll"))
}

#[cfg(not(windows))]
fn should_copy_windows_runtime_support_file(_file_name: &str) -> bool {
    false
}

#[cfg(windows)]
fn sync_managed_runtime_support_files(base_spec: &PythonLaunchSpec, venv_dir: &Path) -> Result<(), String> {
    if base_spec.source != "bundled" {
        return Ok(());
    }

    let base_root = Path::new(&base_spec.program)
        .parent()
        .ok_or_else(|| format!("resolve bundled python parent dir failed: {}", base_spec.program))?;
    let scripts_dir = venv_dir.join("Scripts");
    fs::create_dir_all(venv_dir).map_err(|e| format!("create managed venv root failed: {e}"))?;
    fs::create_dir_all(&scripts_dir).map_err(|e| format!("create managed venv Scripts dir failed: {e}"))?;

    let mut copied_file_count = 0usize;
    let mut copied_dir_count = 0usize;

    // 设计说明：
    // 1. Windows 下由 embeddable/bundled Python 创建出来的 `.venv`，启动时仍可能依赖基础目录中的
    //    `python3*.dll`、`vcruntime*.dll`、`python*.zip` 和 `DLLs/` 目录。
    // 2. 如果这些文件只停留在下载解压目录而没有同步进 `.venv`，状态检测或 worker 启动就可能出现
    //    `0xc0000135 (STATUS_DLL_NOT_FOUND)`。
    // 3. 因此在初始化阶段把运行时必要文件复制进托管环境，让 `.venv` 本身就是可直接运行的完整 Python 环境，
    //    后续状态查询、pip 与执行链路都不再依赖额外搜索路径补丁。
    for entry in fs::read_dir(base_root).map_err(|e| format!("read bundled python dir failed: {e}"))? {
        let entry = entry.map_err(|e| format!("read bundled python dir entry failed: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("read bundled python file type failed: {e}"))?;
        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy().to_string();
        if !should_copy_windows_runtime_support_file(&file_name_str) {
            continue;
        }

        let source_path = entry.path();
        let root_target_path = venv_dir.join(&file_name);
        if root_target_path.exists() {
            let _ = fs::remove_file(&root_target_path);
        }
        fs::copy(&source_path, &root_target_path)
            .map_err(|e| format!("copy runtime support file to managed venv failed: {e}"))?;
        copied_file_count += 1;

        let normalized_file_name = file_name_str.to_ascii_lowercase();
        if normalized_file_name.ends_with(".dll") || normalized_file_name.ends_with(".zip") {
            let scripts_target_path = scripts_dir.join(&file_name);
            if scripts_target_path.exists() {
                let _ = fs::remove_file(&scripts_target_path);
            }
            fs::copy(&source_path, &scripts_target_path)
                .map_err(|e| format!("copy runtime support file to managed Scripts failed: {e}"))?;
            copied_file_count += 1;
        }
    }

    let base_dlls_dir = base_root.join("DLLs");
    if base_dlls_dir.is_dir() {
        let managed_dlls_dir = venv_dir.join("DLLs");
        if managed_dlls_dir.exists() {
            remove_path_if_exists(&managed_dlls_dir)?;
        }
        copy_dir_recursive(&base_dlls_dir, &managed_dlls_dir)?;
        copied_dir_count += 1;
    }

    info!(
        base_root = base_root.to_string_lossy().as_ref(),
        venv_dir = venv_dir.to_string_lossy().as_ref(),
        copied_file_count,
        copied_dir_count,
        "[PythonRuntime] synchronized bundled runtime support files into managed virtual environment"
    );

    Ok(())
}

#[cfg(not(windows))]
fn sync_managed_runtime_support_files(_base_spec: &PythonLaunchSpec, _venv_dir: &Path) -> Result<(), String> {
    Ok(())
}

fn managed_runtime_marker_exists(runtime_dir: &Path) -> bool {
    runtime_marker_path(runtime_dir).is_file()
}

fn normalize_python_package_name(package: &str) -> String {
    // 去掉版本约束（==、>=、~= 等）和 extras（[...]），只保留可比较的包名，
    // 这样 "antlr4-python3-runtime==4.11.1" 能与已安装的 "antlr4-python3-runtime"
    // 正确匹配，避免每次启动都重复尝试安装。
    let name = package
        .trim()
        .split(|c| matches!(c, '=' | '<' | '>' | '!' | '~' | ' ' | '[' | ';'))
        .next()
        .unwrap_or("")
        .trim();
    name.to_ascii_lowercase().replace('_', "-")
}

fn should_skip_python_package_restore(package: &str) -> bool {
    let normalized = normalize_python_package_name(package);
    PYTHON_RUNTIME_BOOTSTRAP_PACKAGE_NAMES
        .iter()
        .any(|item| normalize_python_package_name(item) == normalized)
}

fn python_runtime_recommended_packages() -> Vec<String> {
    PYTHON_RUNTIME_RECOMMENDED_PACKAGES
        .iter()
        .map(|item| (*item).to_string())
        .collect()
}

fn list_python_package_names(spec: &PythonLaunchSpec) -> HashSet<String> {
    managed_pip_list_json(spec)
        .into_iter()
        .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
        .map(|name| normalize_python_package_name(&name))
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(windows)]
fn managed_site_packages_candidates(venv_dir: &Path) -> Vec<PathBuf> {
    vec![
        venv_dir.join("Lib").join("site-packages"),
        venv_dir.join("lib").join("site-packages"),
    ]
}

#[cfg(not(windows))]
fn managed_site_packages_candidates(venv_dir: &Path) -> Vec<PathBuf> {
    let lib_dir = venv_dir.join("lib");
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let path = entry.path().join("site-packages");
            if path.is_dir() {
                candidates.push(path);
            }
        }
    }
    if candidates.is_empty() {
        candidates.push(lib_dir.join("site-packages"));
    }
    candidates
}

fn parse_package_name_from_dist_info_dir(path: &Path) -> Option<String> {
    let metadata_path = path.join("METADATA");
    if metadata_path.is_file() {
        if let Ok(content) = fs::read_to_string(&metadata_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(value) = trimmed.strip_prefix("Name:") {
                    let normalized = normalize_python_package_name(value);
                    if !normalized.is_empty() {
                        return Some(normalized);
                    }
                }
            }
        }
    }

    let file_name = path.file_name()?.to_string_lossy();
    let base = file_name
        .strip_suffix(".dist-info")
        .or_else(|| file_name.strip_suffix(".egg-info"))
        .unwrap_or(&file_name);
    let inferred = base
        .split('-')
        .next()
        .map(normalize_python_package_name)
        .unwrap_or_default();
    if inferred.is_empty() {
        None
    } else {
        Some(inferred)
    }
}

fn snapshot_managed_runtime_packages_from_filesystem(venv_dir: &Path) -> Vec<String> {
    let mut packages = HashSet::new();

    for site_packages_dir in managed_site_packages_candidates(venv_dir) {
        if !site_packages_dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&site_packages_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !(file_name.ends_with(".dist-info") || file_name.ends_with(".egg-info")) {
                continue;
            }
            if let Some(package_name) = parse_package_name_from_dist_info_dir(&path) {
                if !should_skip_python_package_restore(&package_name) {
                    packages.insert(package_name);
                }
            }
        }
    }

    let mut sorted = packages.into_iter().collect::<Vec<_>>();
    sorted.sort();
    if !sorted.is_empty() {
        info!(
            venv_dir = venv_dir.to_string_lossy().as_ref(),
            package_count = sorted.len(),
            "[PythonRuntime] captured managed runtime package snapshot from filesystem"
        );
    }
    sorted
}

fn snapshot_managed_runtime_packages(spec: &PythonLaunchSpec) -> Vec<String> {
    let package_names = list_python_package_names(spec)
        .into_iter()
        .filter(|name| !should_skip_python_package_restore(name))
        .collect::<Vec<_>>();
    if !package_names.is_empty() {
        info!(
            runtime_dir = spec.runtime_dir.to_string_lossy().as_ref(),
            package_count = package_names.len(),
            "[PythonRuntime] captured managed runtime package snapshot from pip list"
        );
        return package_names;
    }

    snapshot_managed_runtime_packages_from_filesystem(&managed_venv_dir(&spec.runtime_dir))
}

fn ensure_python_packages_installed(
    spec: &PythonLaunchSpec,
    packages: &[String],
    reason: &str,
) -> Result<(), String> {
    let desired_packages = packages
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| !should_skip_python_package_restore(item))
        .collect::<Vec<_>>();
    if desired_packages.is_empty() {
        return Ok(());
    }

    let installed = list_python_package_names(spec);
    let missing_packages = desired_packages
        .into_iter()
        .filter(|item| !installed.contains(&normalize_python_package_name(item)))
        .collect::<Vec<_>>();
    if missing_packages.is_empty() {
        info!(
            reason = reason,
            runtime_dir = spec.runtime_dir.to_string_lossy().as_ref(),
            "[PythonRuntime] all desired python packages already installed"
        );
        return Ok(());
    }

    let (ok, stdout, stderr) = run_managed_package_op(spec, "install", &missing_packages)?;
    if !ok {
        return Err(format!(
            "install managed runtime packages failed during {reason}: stdout={}, stderr={}",
            stdout.trim(),
            stderr.trim()
        ));
    }

    info!(
        reason = reason,
        runtime_dir = spec.runtime_dir.to_string_lossy().as_ref(),
        package_count = missing_packages.len(),
        packages = missing_packages.join(", ").as_str(),
        "[PythonRuntime] installed missing managed runtime packages"
    );
    Ok(())
}

/// 合并推荐包列表与当前快照、安装缺失的包。
/// - 幂等：只 pip install 尚未在 venv 中存在的包；
/// - 无论走早期复用还是首次创建都调用，确保老运行时补上新版新依赖。
fn ensure_recommended_runtime_packages_installed(
    runtime_dir: &Path,
    managed_spec: &Option<PythonLaunchSpec>,
    managed_package_snapshot: &mut Vec<String>,
    reason: &str,
) -> Result<(), String> {
    let spec = match managed_spec {
        Some(spec) => spec,
        None => {
            info!(
                runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                reason = reason,
                "[PythonRuntime] ensure_recommended_packages skipped (no managed spec)"
            );
            return Ok(());
        }
    };

    for recommended_package in python_runtime_recommended_packages() {
        if !managed_package_snapshot.iter().any(|item| {
            normalize_python_package_name(item)
                == normalize_python_package_name(&recommended_package)
        }) {
            managed_package_snapshot.push(recommended_package);
        }
    }
    info!(
        package_count = managed_package_snapshot.len(),
        packages = managed_package_snapshot.join(", ").as_str(),
        "[PythonRuntime] prepared managed runtime package ensure list"
    );

    ensure_managed_pip_mirror(runtime_dir)?;
    ensure_pip_available(spec)?;
    ensure_python_packages_installed(spec, managed_package_snapshot, reason)
}

fn managed_python_runtime_health(spec: &PythonLaunchSpec) -> Result<(), String> {
    // 关键：托管 venv 必须有 pyvenv.cfg。缺了它，.venv/Scripts/python.exe 会以“基础解释器”身份启动，
    // sys.prefix 指向 uv-python 基础目录、看不到 .venv/Lib/site-packages 里装的包
    // （表现为 numpy 明明已装却 ModuleNotFoundError）。缺失即判定损坏，触发用 uv --clear 重建。
    let venv_dir = managed_venv_dir(&spec.runtime_dir);
    if !venv_dir.join("pyvenv.cfg").is_file() {
        return Err("managed venv missing pyvenv.cfg".to_string());
    }

    let mut version_cmd = Command::new(&spec.program);
    version_cmd.args(&spec.base_args).arg("--version");
    let _ = command_output_trimmed(version_cmd)
        .map_err(|e| format!("managed python probe failed: {e}"))?;

    let mut script_mode_cmd = Command::new(&spec.program);
    script_mode_cmd
        .args(&spec.base_args)
        .args(["-c", "import encodings, sys; print(sys.executable)"]);
    let script_mode_probe = command_output_trimmed(script_mode_cmd)
        .map_err(|e| format!("managed python script-mode probe failed: {e}"))?;
    info!(
        program = spec.program.as_str(),
        source = spec.source.as_str(),
        executable = script_mode_probe.as_str(),
        "[PythonRuntime] managed runtime script-mode probe succeeded"
    );

    let mut pip_cmd = Command::new(&spec.program);
    pip_cmd.args(&spec.base_args).args(["-m", "pip", "--version"]);
    let _ = command_output_trimmed(pip_cmd)
        .map_err(|e| format!("managed pip probe failed: {e}"))?;

    if !managed_runtime_marker_exists(&spec.runtime_dir) {
        return Err("managed runtime marker missing".to_string());
    }

    Ok(())
}

fn cleanup_bootstrap_runtime_artifacts(runtime_dir: &Path, reason: &str) {
    for target in bootstrap_runtime_artifact_paths(runtime_dir) {
        if !target.exists() {
            continue;
        }
        match remove_path_if_exists(&target) {
            Ok(_) => info!(
                reason = reason,
                target = target.to_string_lossy().as_ref(),
                "[PythonRuntime] cleaned bootstrap runtime artifact"
            ),
            Err(error) => warn!(
                reason = reason,
                target = target.to_string_lossy().as_ref(),
                error = error.as_str(),
                "[PythonRuntime] failed to clean bootstrap runtime artifact"
            ),
        }
    }
}

fn managed_python_launch_spec(runtime_dir: &Path, source: String) -> Result<PythonLaunchSpec, String> {
    for candidate in managed_python_candidates(runtime_dir) {
        if candidate.is_file() {
            return Ok(PythonLaunchSpec {
                program: candidate.to_string_lossy().to_string(),
                base_args: Vec::new(),
                source,
                runtime_dir: runtime_dir.to_path_buf(),
            });
        }
    }

    Err(format!(
        "未找到 Guaxi 管理的 Python 虚拟环境：{}",
        managed_venv_dir(runtime_dir).to_string_lossy()
    ))
}

// 会话内托管运行时"就绪"缓存。
// 原因:每次 worker spawn 都会跑 3 个 health 探针子进程 + pip list,Windows 上合计 2–5s。
// 只要 .venv 指纹(python 可执行文件 / pyvenv.cfg / runtime marker 的 mtime+size)未变且 TTL 未过期,
// 就复用上次已确认健康的 spec,跳过这些探测。装/卸包(run_managed_package_op)与重建会失效或刷新缓存。
struct RuntimeReadyCacheEntry {
    runtime_dir: PathBuf,
    fingerprint: u64,
    spec: PythonLaunchSpec,
    verified_at: Instant,
}

static RUNTIME_READY_CACHE: OnceLock<Mutex<Option<RuntimeReadyCacheEntry>>> = OnceLock::new();
const RUNTIME_READY_CACHE_TTL: Duration = Duration::from_secs(60);

fn runtime_ready_cache_cell() -> &'static Mutex<Option<RuntimeReadyCacheEntry>> {
    RUNTIME_READY_CACHE.get_or_init(|| Mutex::new(None))
}

fn runtime_ready_fingerprint(spec: &PythonLaunchSpec, runtime_dir: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    spec.program.hash(&mut hasher);
    let probe_paths = [
        PathBuf::from(&spec.program),
        managed_venv_dir(runtime_dir).join("pyvenv.cfg"),
        runtime_marker_path(runtime_dir),
    ];
    for path in probe_paths {
        match fs::metadata(&path) {
            Ok(meta) => {
                meta.len().hash(&mut hasher);
                if let Ok(modified) = meta.modified() {
                    if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                        dur.as_nanos().hash(&mut hasher);
                    }
                }
            }
            Err(_) => 0u8.hash(&mut hasher),
        }
    }
    hasher.finish()
}

fn runtime_ready_cache_lookup(spec: &PythonLaunchSpec, runtime_dir: &Path) -> Option<PythonLaunchSpec> {
    let fingerprint = runtime_ready_fingerprint(spec, runtime_dir);
    let guard = runtime_ready_cache_cell().lock().ok()?;
    let entry = guard.as_ref()?;
    if entry.runtime_dir == runtime_dir
        && entry.fingerprint == fingerprint
        && entry.verified_at.elapsed() < RUNTIME_READY_CACHE_TTL
    {
        Some(entry.spec.clone())
    } else {
        None
    }
}

fn runtime_ready_cache_store(spec: &PythonLaunchSpec, runtime_dir: &Path) {
    let fingerprint = runtime_ready_fingerprint(spec, runtime_dir);
    if let Ok(mut guard) = runtime_ready_cache_cell().lock() {
        *guard = Some(RuntimeReadyCacheEntry {
            runtime_dir: runtime_dir.to_path_buf(),
            fingerprint,
            spec: spec.clone(),
            verified_at: Instant::now(),
        });
    }
}

fn invalidate_runtime_ready_cache() {
    if let Some(cell) = RUNTIME_READY_CACHE.get() {
        if let Ok(mut guard) = cell.lock() {
            *guard = None;
        }
    }
}

fn ensure_managed_python_launch_spec() -> Result<PythonLaunchSpec, String> {
    ensure_managed_python_launch_spec_with_download_version(None)
}

fn ensure_managed_python_launch_spec_with_download_version(
    download_version: Option<&str>,
) -> Result<PythonLaunchSpec, String> {
    let runtime_dir = python_runtime_dir()?;
    let venv_dir = managed_venv_dir(&runtime_dir);
    let managed_spec = managed_python_launch_spec(&runtime_dir, "managed-runtime".to_string()).ok();
    let has_managed_python = managed_spec.is_some();

    // 会话内快速路径:未显式切换 Python 版本、且托管运行时指纹与上次确认一致时,直接复用,
    // 跳过下面的 health 探针与 pip list。首次调用(缓存空)仍走完整探测,行为不变。
    let version_override_requested = download_version
        .map(str::trim)
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if !version_override_requested {
        if let Some(spec) = managed_spec.as_ref() {
            if let Some(cached) = runtime_ready_cache_lookup(spec, &runtime_dir) {
                return Ok(cached);
            }
        }
    }

    let mut managed_package_snapshot = managed_spec
        .as_ref()
        .map(snapshot_managed_runtime_packages)
        .unwrap_or_default();
    let managed_runtime_health_error = managed_spec
        .as_ref()
        .and_then(|spec| managed_python_runtime_health(spec).err());
    let marker_missing_only =
        managed_runtime_health_error.as_deref() == Some("managed runtime marker missing");
    let managed_runtime_broken = managed_runtime_health_error.is_some() && !marker_missing_only;

    // 版本切换：显式请求了某个 Python 版本、且与当前 venv 不符时，跳过复用、走重建。
    let requested_version = download_version
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let version_mismatch = match (requested_version, managed_spec.as_ref()) {
        (Some(requested), Some(spec)) => !managed_python_version_matches(spec, requested),
        _ => false,
    };

    // 设计说明：
    // 1. pip 安装、worker 启动、状态读取这些运行期操作，只要托管 `.venv` 当前可用，就应该直接复用它。
    // 2. 只有在“首次创建”“已损坏需要重建”或“显式切换 Python 版本”时，才走重建流程。
    // 3. 上一版把 bundled base 解析提前到了函数入口，导致即使 `.venv` 已经健康可用，安装包也会被
    //    “未找到内置基础运行时”误拦截。
    if let Some(spec) = managed_spec.as_ref() {
        if !managed_runtime_broken && !version_mismatch {
            if marker_missing_only {
                // Recovery note:
                // 1. Older bootstrap flows could leave a healthy `.venv` behind without persisting runtime.json.
                // 2. We repair the marker in place instead of recreating the environment, so installed packages stay intact.
                // 3. The original bootstrap source may be unrecoverable here, so we persist the current managed-runtime facts.
                ensure_managed_pip_mirror(&runtime_dir)?;
                let pip_version = ensure_pip_available(spec)?;
                let _ = write_runtime_marker(
                    spec,
                    &pip_version,
                    download_version.unwrap_or_default(),
                    false,
                )?;
                cleanup_bootstrap_runtime_artifacts(&runtime_dir, "managed-runtime-marker-healed");
                info!(
                    program = spec.program.as_str(),
                    runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                    "[PythonRuntime] healed runtime marker for existing managed virtual environment"
                );
            }
            info!(
                program = spec.program.as_str(),
                runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                "[PythonRuntime] reuse existing managed virtual environment"
            );
            // 即使运行时已存在且健康，仍需补齐可能新增的推荐依赖；
            // 幂等操作——pip list 比对只需毫秒级，跳过已安装的包。
            {
                let mut initial = Vec::new();
                ensure_recommended_runtime_packages_installed(
                    &runtime_dir,
                    &Some(spec.clone()),
                    &mut initial,
                    "managed-runtime-reuse-ensure",
                )?;
            }
            runtime_ready_cache_store(spec, &runtime_dir);
            return Ok(spec.clone());
        }
    }

    // 到这里意味着托管 venv 需要创建或重建：缺失、损坏、或切换 Python 版本。
    if managed_runtime_broken || version_mismatch {
        warn!(
            venv_dir = venv_dir.to_string_lossy().as_ref(),
            broken = managed_runtime_broken,
            version_mismatch,
            error = managed_runtime_health_error.as_deref().unwrap_or("none"),
            "[PythonRuntime] recreate managed virtual environment"
        );
        // 先收掉常驻 worker 释放对 .venv 的文件占用，再带重试删除（Windows 句柄释放有延迟）。
        let _ = clear_python_worker();
        remove_venv_dir_with_retry(&venv_dir)?;
        let _ = remove_path_if_exists(&runtime_marker_path(&runtime_dir));
    }

    fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("create python runtime dir failed: {e}"))?;

    // uv 优先：能解析到 uv 就用它创建 venv，完全绕开 python.org 基础解释器下载与 Windows DLL 同步；
    // uv 不可用（下载失败等）时，回退到旧的 bundled base + `python -m venv` 供给链。
    let target_version = requested_version.unwrap_or(UV_DEFAULT_PYTHON_VERSION);
    let base_source = match resolve_uv_binary() {
        Ok(uv_path) => {
            create_managed_virtualenv_with_uv(&uv_path, &runtime_dir, &venv_dir, target_version)?;
            info!(
                target_version,
                venv_dir = venv_dir.to_string_lossy().as_ref(),
                "[PythonRuntime] managed virtual environment created via uv"
            );
            "uv-managed".to_string()
        }
        Err(uv_error) => {
            warn!(
                error = uv_error.as_str(),
                "[PythonRuntime] uv unavailable, fallback to bundled base python + venv"
            );
            let base_spec = resolve_bootstrap_python_base_launch_spec(download_version)?;
            create_managed_virtualenv(&base_spec, &venv_dir)?;
            sync_managed_runtime_support_files(&base_spec, &venv_dir)?;
            info!(
                source = base_spec.source.as_str(),
                venv_dir = venv_dir.to_string_lossy().as_ref(),
                "[PythonRuntime] managed virtual environment created from bundled base python"
            );
            base_spec.source
        }
    };

    let marker_spec = managed_python_launch_spec(&runtime_dir, base_source.clone())?;
    let pip_version = ensure_pip_available(&marker_spec)?;
    ensure_recommended_runtime_packages_installed(
        &runtime_dir,
        &Some(marker_spec.clone()),
        &mut managed_package_snapshot,
        if version_mismatch {
            "managed-runtime-version-switch"
        } else if managed_runtime_broken {
            "managed-runtime-repair"
        } else if !has_managed_python {
            "managed-runtime-initialize"
        } else {
            "managed-runtime-ensure"
        },
    )?;
    let _ = write_runtime_marker(
        &marker_spec,
        &pip_version,
        download_version.unwrap_or_default(),
        did_use_download_bootstrap(&runtime_dir, &base_source),
    )?;
    cleanup_bootstrap_runtime_artifacts(&runtime_dir, "managed-runtime-ready");
    runtime_ready_cache_store(&marker_spec, &runtime_dir);
    Ok(marker_spec)
}

fn resolve_python_launch_spec() -> Result<PythonLaunchSpec, String> {
    ensure_managed_python_launch_spec()
}

/// 确保 Guaxi 托管 Python 运行时就绪，并把给定依赖安装进托管 `.venv`。
///
/// 供 agent server 等外部子进程复用同一套托管 Python，避免依赖不可控的系统环境。
/// 首次调用可能触发内置 Python 下载与 pip 安装，属于阻塞 I/O，调用方应放到阻塞线程执行。
/// 返回托管 venv 的 python 可执行路径与基础参数（base_args）。
pub fn ensure_managed_python_with_packages(
    packages: &[String],
    reason: &str,
) -> Result<(String, Vec<String>), String> {
    let spec = ensure_managed_python_launch_spec()?;
    // 幂等：ensure_python_packages_installed 内部会先 `pip list` 比对，已装的包会被跳过，
    // 因此后续每次启动几乎零开销，只在缺包时才真正执行 pip install。
    ensure_python_packages_installed(&spec, packages, reason)?;
    Ok((spec.program, spec.base_args))
}

fn auto_repair_managed_runtime_if_needed(
    runtime_dir: &Path,
    managed_spec: Option<PythonLaunchSpec>,
    managed_health_error: Option<String>,
) -> (Option<PythonLaunchSpec>, Option<String>) {
    let Some(original_error) = managed_health_error else {
        return (managed_spec, None);
    };

    // 设计说明：
    // 1. 旧版本可能留下“文件还在、但 `.venv` 已损坏”的半成品环境；如果这里只返回错误，前端每次打开状态面板都会先撞到失败。
    // 2. 现在线程已经处在后端阻塞任务中，最稳妥的做法是在探测到损坏时立即走一次与初始化相同的重建流程。
    // 3. 这样用户看到的第一次状态就是“已自动修复后的结果”；只有自动修复本身失败时，才回退为待初始化/待处理状态。
    warn!(
        runtime_dir = runtime_dir.to_string_lossy().as_ref(),
        error = original_error.as_str(),
        had_managed_spec = managed_spec.is_some(),
        "[PythonRuntime] detected broken managed runtime during status probe, start automatic repair"
    );

    match ensure_managed_python_launch_spec_with_download_version(None) {
        Ok(repaired_spec) => {
            info!(
                runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                program = repaired_spec.program.as_str(),
                "[PythonRuntime] automatic managed runtime repair completed during status probe"
            );
            (Some(repaired_spec), None)
        }
        Err(repair_error) => {
            warn!(
                runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                original_error = original_error.as_str(),
                repair_error = repair_error.as_str(),
                "[PythonRuntime] automatic managed runtime repair failed during status probe"
            );
            (managed_spec, Some(repair_error))
        }
    }
}

fn ensure_worker_script(runtime_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(runtime_dir).map_err(|e| format!("create runtime dir failed: {e}"))?;
    let script_path = runtime_dir.join(PY_WORKER_FILE_NAME);
    let needs_write = match fs::read_to_string(&script_path) {
        Ok(existing) => existing != PY_WORKER_SOURCE,
        Err(_) => true,
    };
    if needs_write {
        fs::write(&script_path, PY_WORKER_SOURCE)
            .map_err(|e| format!("write python worker script failed: {e}"))?;
        info!(worker_script = %script_path.to_string_lossy(), "[PythonRuntime] worker script refreshed");
    }
    // 把数学引擎模块物化到 worker 同一目录（worker 已将该目录加入 sys.path）。
    // 仅在内容变化时重写，避免每次启动都触碰磁盘。失败不致命：bootstrap 侧有 ?raw 内联兜底。
    let engine_path = runtime_dir.join(GUAXIMATH_ENGINE_FILE_NAME);
    let engine_needs_write = match fs::read_to_string(&engine_path) {
        Ok(existing) => existing != GUAXIMATH_ENGINE_SOURCE,
        Err(_) => true,
    };
    if engine_needs_write {
        if let Err(e) = fs::write(&engine_path, GUAXIMATH_ENGINE_SOURCE) {
            warn!(engine_script = %engine_path.to_string_lossy(), error = %e,
                  "[PythonRuntime] write guaximath engine failed, will fall back to inline source");
        } else {
            info!(engine_script = %engine_path.to_string_lossy(), "[PythonRuntime] guaximath engine refreshed");
        }
    }
    Ok(script_path)
}

fn ensure_pip_available(spec: &PythonLaunchSpec) -> Result<String, String> {
    let mut version_cmd = Command::new(&spec.program);
    version_cmd.args(&spec.base_args).args(["-m", "pip", "--version"]);
    match command_output_trimmed(version_cmd) {
        Ok(version) => Ok(version),
        Err(first_error) => {
            warn!(source = spec.source.as_str(), error = first_error.as_str(), "[PythonRuntime] pip missing, try ensurepip");
            let mut ensure_cmd = Command::new(&spec.program);
            ensure_cmd.args(&spec.base_args).args(["-m", "ensurepip", "--upgrade"]);
            let _ = command_output_trimmed(ensure_cmd)?;

            let mut retry_cmd = Command::new(&spec.program);
            retry_cmd.args(&spec.base_args).args(["-m", "pip", "--version"]);
            command_output_trimmed(retry_cmd)
        }
    }
}

fn spawn_python_worker(spec: &PythonLaunchSpec) -> Result<PythonWorkerState, String> {
    let worker_script_path = ensure_worker_script(&spec.runtime_dir)?;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.base_args)
        .arg("-u")
        .arg(&worker_script_path)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&spec.runtime_dir);
    apply_python_process_no_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn python worker failed: {e}"))?;

    // 常驻 worker 登记到全局托管表：退出时即使某个 worker 正忙、拿不到它的锁，
    // 也仍然能按 PID 结束进程树，避免残留 python 进程。
    crate::guaxiservices::process_shutdown::register_managed_pid("python-worker", child.id());

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "python worker stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "python worker stdout unavailable".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "python worker stderr unavailable".to_string())?;
    let stderr_tail = Arc::new(Mutex::new(String::new()));
    let stderr_tail_reader = Arc::clone(&stderr_tail);
    let worker_script_for_log = worker_script_path.to_string_lossy().to_string();
    thread::spawn(move || {
        let mut stderr_reader = BufReader::new(stderr);
        let mut stderr_text = String::new();
        match stderr_reader.read_to_string(&mut stderr_text) {
            Ok(_) => {
                if stderr_text.trim().is_empty() {
                    return;
                }
                let tail = tail_text_by_chars(&stderr_text, 6000);
                if let Ok(mut guard) = stderr_tail_reader.lock() {
                    *guard = tail.clone();
                }
                warn!(
                    worker_script = worker_script_for_log.as_str(),
                    stderr_tail = tail.as_str(),
                    "[PythonRuntime] worker stderr captured"
                );
            }
            Err(error) => {
                warn!(
                    worker_script = worker_script_for_log.as_str(),
                    error = %error,
                    "[PythonRuntime] read worker stderr failed"
                );
            }
        }
    });

    info!(
        python = spec.program.as_str(),
        source = spec.source.as_str(),
        worker_script = %worker_script_path.to_string_lossy(),
        "[PythonRuntime] worker spawned"
    );

    Ok(PythonWorkerState {
        child,
        stdin,
        stdout: BufReader::new(stdout),
        stderr_tail,
    })
}

fn tail_text_by_chars(text: &str, max_chars: usize) -> String {
    let mut items = text.chars().rev().take(max_chars).collect::<Vec<_>>();
    items.reverse();
    items.into_iter().collect::<String>()
}

fn python_worker_stderr_tail(state: &PythonWorkerState) -> String {
    state
        .stderr_tail
        .lock()
        .ok()
        .map(|guard| guard.trim().to_string())
        .unwrap_or_default()
}

fn start_python_worker_request_watchdog(
    pid: u32,
    worker_cmd: String,
    session_id: String,
    cell_id: String,
    timeout_secs: u64,
) -> Arc<AtomicBool> {
    let done = Arc::new(AtomicBool::new(false));
    let done_flag = Arc::clone(&done);
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(timeout_secs));
        if done_flag.load(Ordering::SeqCst) {
            return;
        }

        warn!(
            pid,
            cmd = worker_cmd.as_str(),
            session_id = session_id.as_str(),
            cell_id = cell_id.as_str(),
            timeout_secs,
            "[PythonRuntime] worker request timed out, terminating python process"
        );

        let _ = terminate_process_by_pid(pid);
    });
    done
}

#[cfg(windows)]
fn terminate_process_by_pid(pid: u32) -> Result<(), String> {
    let mut command = Command::new("taskkill");
    command.args(["/PID", &pid.to_string(), "/T", "/F"]);
    apply_python_process_no_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("terminate python worker process failed: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "terminate python worker process failed: status={}, stdout={}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(not(windows))]
fn terminate_process_by_pid(pid: u32) -> Result<(), String> {
    let mut command = Command::new("kill");
    command.args(["-9", &pid.to_string()]);
    let output = command
        .output()
        .map_err(|e| format!("terminate python worker process failed: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "terminate python worker process failed: status={}, stdout={}, stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn worker_is_alive(state: &mut PythonWorkerState) -> bool {
    match state.child.try_wait() {
        Ok(Some(status)) => {
            warn!(status = %status, "[PythonRuntime] worker exited");
            false
        }
        Ok(None) => true,
        Err(error) => {
            warn!(error = %error, "[PythonRuntime] worker try_wait failed");
            false
        }
    }
}

fn terminate_python_worker_state(worker_key: &str, worker: Arc<Mutex<PythonWorkerState>>) {
    match worker.lock() {
        Ok(mut state) => {
            crate::guaxiservices::process_shutdown::unregister_managed_pid(state.child.id());
            let _ = state.child.kill();
            info!(worker_key, "[PythonRuntime] worker terminated");
        }
        Err(_) => {
            warn!(worker_key, "[PythonRuntime] lock python worker state failed during terminate");
        }
    }
}

/// 关闭所有常驻 python worker（探索者模式的 python 引擎）。
///
/// 设计说明：
/// 1. 每个 notebook 会话都会常驻一个 python worker 进程等待下一次执行，主程序退出时
///    必须显式收掉，否则会残留一堆 python 进程（用户看不到，只能去任务管理器杀）。
/// 2. worker 里跑用户代码，可能已经派生出 pip / matplotlib / 子进程池等孙进程，
///    因此走进程树终止而不是只 kill 直接子进程。
/// 3. 这里可能在“某个 cell 正在执行”时被调用，`worker.lock()` 会被执行线程占用，
///    所以先只从注册表取出 PID 直接终止进程树，再尽力拿锁回收句柄，避免退出时卡死。
pub fn shutdown_all_python_workers() {
    let workers = match python_worker_states_cell().lock() {
        Ok(mut guard) => guard.drain().collect::<Vec<_>>(),
        Err(_) => {
            warn!("[PythonRuntime] lock python worker states failed during shutdown");
            Vec::new()
        }
    };

    let total = workers.len();
    for (worker_key, worker) in workers {
        match worker.try_lock() {
            Ok(mut state) => {
                crate::guaxiservices::process_shutdown::terminate_child(
                    &format!("python-worker({worker_key})"),
                    &mut state.child,
                );
            }
            Err(_) => {
                // 正在执行 cell，拿不到锁；此时只能靠活跃请求登记表里的 PID 终止进程树。
                warn!(
                    worker_key = worker_key.as_str(),
                    "[PythonRuntime] worker busy during shutdown, fallback to pid-based terminate"
                );
            }
        }
    }

    let active = match python_worker_active_requests_cell().lock() {
        Ok(mut guard) => guard.drain().map(|(_, item)| item).collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    for item in active {
        crate::guaxiservices::process_shutdown::terminate_process_tree(
            item.pid,
            &format!("python-worker({})", item.worker_key),
        );
    }

    if total > 0 {
        info!(total, "[PythonRuntime] python workers terminated on shutdown");
    }
}

fn register_python_active_request(
    request_id: &str,
    worker_key: &str,
    session_id: &str,
    cell_id: &str,
    pid: u32,
) {
    let normalized_request_id = request_id.trim();
    if normalized_request_id.is_empty() {
        return;
    }

    match python_worker_active_requests_cell().lock() {
        Ok(mut guard) => {
            guard.insert(
                normalized_request_id.to_string(),
                PythonActiveRequest {
                    worker_key: worker_key.to_string(),
                    session_id: session_id.to_string(),
                    cell_id: cell_id.to_string(),
                    pid,
                },
            );
            info!(
                request_id = normalized_request_id,
                worker_key,
                session_id,
                cell_id,
                pid,
                "[PythonRuntime] active request registered"
            );
        }
        Err(_) => {
            warn!(
                request_id = normalized_request_id,
                worker_key,
                "[PythonRuntime] lock active request registry failed during register"
            );
        }
    }
}

fn unregister_python_active_request(request_id: &str, reason: &str) {
    let normalized_request_id = request_id.trim();
    if normalized_request_id.is_empty() {
        return;
    }

    match python_worker_active_requests_cell().lock() {
        Ok(mut guard) => {
            let removed = guard.remove(normalized_request_id);
            if let Some(active) = removed {
                info!(
                    request_id = normalized_request_id,
                    worker_key = active.worker_key.as_str(),
                    session_id = active.session_id.as_str(),
                    cell_id = active.cell_id.as_str(),
                    pid = active.pid,
                    reason,
                    "[PythonRuntime] active request unregistered"
                );
            }
        }
        Err(_) => {
            warn!(
                request_id = normalized_request_id,
                reason,
                "[PythonRuntime] lock active request registry failed during unregister"
            );
        }
    }
}

fn remove_python_worker_by_key(
    worker_key: &str,
) -> Result<Option<Arc<Mutex<PythonWorkerState>>>, String> {
    let mut guard = python_worker_states_cell()
        .lock()
        .map_err(|_| "lock python worker states failed".to_string())?;
    Ok(guard.remove(worker_key))
}

fn ensure_python_worker_for_key(worker_key: &str) -> Result<Arc<Mutex<PythonWorkerState>>, String> {
    let existing_worker = {
        let guard = python_worker_states_cell()
            .lock()
            .map_err(|_| "lock python worker states failed".to_string())?;
        guard.get(worker_key).cloned()
    };

    if let Some(worker) = existing_worker {
        let mut state = worker
            .lock()
            .map_err(|_| "lock python worker state failed".to_string())?;
        if worker_is_alive(&mut state) {
            drop(state);
            return Ok(worker);
        }
        warn!(worker_key, "[PythonRuntime] cached worker is no longer alive, respawn it");
    }

    if let Some(stale_worker) = remove_python_worker_by_key(worker_key)? {
        terminate_python_worker_state(worker_key, stale_worker);
    }

    let spec = resolve_python_launch_spec()?;
    let spawned_worker = Arc::new(Mutex::new(spawn_python_worker(&spec)?));

    let winner = {
        let mut guard = python_worker_states_cell()
            .lock()
            .map_err(|_| "lock python worker states failed".to_string())?;
        match guard.get(worker_key).cloned() {
            Some(existing_worker) => existing_worker,
            None => {
                guard.insert(worker_key.to_string(), Arc::clone(&spawned_worker));
                Arc::clone(&spawned_worker)
            }
        }
    };

    if !Arc::ptr_eq(&winner, &spawned_worker) {
        terminate_python_worker_state(worker_key, spawned_worker);
    }

    Ok(winner)
}

fn with_python_worker_for_key<R>(
    worker_key: &str,
    f: impl FnOnce(&mut PythonWorkerState) -> Result<R, String>,
) -> Result<R, String> {
    let worker = ensure_python_worker_for_key(worker_key)?;
    let worker_probe = Arc::clone(&worker);
    let active_worker = match worker_probe.try_lock() {
        Ok(state) => {
            drop(state);
            worker
        }
        Err(TryLockError::WouldBlock) => {
            warn!(
                worker_key,
                "[PythonRuntime] worker state is still locked, replace cached worker to avoid hanging requests"
            );
            let _ = remove_python_worker_by_key(worker_key)?;
            ensure_python_worker_for_key(worker_key)?
        }
        Err(TryLockError::Poisoned(_)) => {
            warn!(
                worker_key,
                "[PythonRuntime] worker state lock poisoned, replace cached worker"
            );
            let _ = remove_python_worker_by_key(worker_key)?;
            ensure_python_worker_for_key(worker_key)?
        }
    };
    let mut state = active_worker
        .lock()
        .map_err(|_| "lock python worker state failed".to_string())?;

    if !worker_is_alive(&mut state) {
        drop(state);
        if let Some(stale_worker) = remove_python_worker_by_key(worker_key)? {
            terminate_python_worker_state(worker_key, stale_worker);
        }
        let respawned_worker = ensure_python_worker_for_key(worker_key)?;
        let mut respawned_state = respawned_worker
            .lock()
            .map_err(|_| "lock python worker state failed".to_string())?;
        return f(&mut respawned_state);
    }

    f(&mut state)
}

fn send_worker_request(payload: &Value) -> Result<Value, String> {
    let worker_key = python_worker_key_from_payload(payload);
    with_python_worker_for_key(&worker_key, |state| {
        let worker_cmd = payload
            .get("cmd")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let worker_vars = payload.get("vars");
        let session_id = worker_vars
            .and_then(|vars| vars.get("_guaxi_session"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let cell_id = worker_vars
            .and_then(|vars| vars.get("_py_cell_id"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let request_id = payload
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let timeout_secs = python_worker_timeout_secs_for_cmd(worker_cmd);
        let pid = state.child.id();
        let watchdog = start_python_worker_request_watchdog(
            pid,
            worker_cmd.to_string(),
            session_id.to_string(),
            cell_id.to_string(),
            timeout_secs,
        );
        let line = serde_json::to_string(payload)
            .map_err(|e| format!("serialize worker request failed: {e}"))?;
        info!(
            worker_key = worker_key.as_str(),
            cmd = worker_cmd,
            session_id,
            cell_id,
            request_id,
            payload_len = line.len(),
            timeout_secs,
            "[PythonRuntime] sending worker request"
        );
        register_python_active_request(request_id, worker_key.as_str(), session_id, cell_id, pid);

        let write_result = (|| -> Result<(), String> {
            state
                .stdin
                .write_all(line.as_bytes())
                .map_err(|e| format!("write worker stdin failed: {e}"))?;
            state
                .stdin
                .write_all(b"\n")
                .map_err(|e| format!("write worker newline failed: {e}"))?;
            state
                .stdin
                .flush()
                .map_err(|e| format!("flush worker stdin failed: {e}"))?;
            Ok(())
        })();
        if let Err(error) = write_result {
            unregister_python_active_request(request_id, "worker-write-failed");
            return Err(error);
        }

        let mut response_line = String::new();
        let read_result = state
            .stdout
            .read_line(&mut response_line);
        watchdog.store(true, Ordering::SeqCst);
        let bytes_read = match read_result {
            Ok(value) => value,
            Err(error) => {
                unregister_python_active_request(request_id, "worker-read-failed");
                return Err(format!("read worker stdout failed: {error}"));
            }
        };
        if bytes_read == 0 {
            let exit_status = match state.child.try_wait() {
                Ok(Some(status)) => status.to_string(),
                Ok(None) => "still-running-after-stdout-eof".to_string(),
                Err(error) => format!("try-wait-failed: {error}"),
            };
            let stderr_tail = python_worker_stderr_tail(state);
            warn!(
                worker_key = worker_key.as_str(),
                cmd = worker_cmd,
                session_id,
                cell_id,
                request_id,
                exit_status = exit_status.as_str(),
                stderr_tail = stderr_tail.as_str(),
                "[PythonRuntime] worker stdout closed before response"
            );
            unregister_python_active_request(request_id, "worker-stdout-closed");
            if stderr_tail.is_empty() {
                return Err(format!(
                    "python worker closed stdout unexpectedly, status={exit_status}"
                ));
            }
            return Err(format!(
                "python worker closed stdout unexpectedly, status={exit_status}, stderr={stderr_tail}"
            ));
        }
        unregister_python_active_request(request_id, "worker-response-received");
        info!(
            worker_key = worker_key.as_str(),
            cmd = worker_cmd,
            session_id,
            cell_id,
            request_id,
            response_len = response_line.len(),
            "[PythonRuntime] received worker response"
        );

        serde_json::from_str::<Value>(response_line.trim())
            .map_err(|e| format!("parse worker response failed: {e}"))
    })
}

/// 工业级正文抽取：调用常驻 Python worker 的 trafilatura handler，把原始 HTML
/// 清洗成高质量图文。返回 worker 的 `article` 对象（serde_json::Value）。
///
/// 设计：复用既有 worker 协议（`send_worker_request`）。trafilatura 未安装时
/// worker 会返回 `{"ok": false, "error": "trafilatura unavailable..."}`，调用方
/// （flipboard 抽取层）据此回退到本地 Rust 抽取，保证链路不中断。
pub fn extract_article_via_python(html: &str, url: &str) -> Result<Value, String> {
    let payload = json!({
        "cmd": "extract_article",
        "html": html,
        "url": url,
        // 固定 worker key，避免污染用户的交互式会话 worker。
        "session_id": "__guaxi_flipboard_extract__",
    });
    let response = send_worker_request(&payload)?;
    let ok = response.get("ok").and_then(Value::as_bool).unwrap_or(false);
    if !ok {
        let err = response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown extract error")
            .to_string();
        return Err(err);
    }
    response
        .get("article")
        .cloned()
        .ok_or_else(|| "extract response missing article".to_string())
}

/// 确保 trafilatura 已安装到管理化运行时（best-effort，幂等）。
/// 已安装则快速返回；缺失则 pip 安装（清华镜像）。失败不致命——
/// 调用方会回退到本地抽取。返回是否安装成功。
#[allow(dead_code)] // 由 P3 爬虫初始化 / P7 设置面板触发
pub fn ensure_trafilatura_installed() -> Result<bool, String> {
    let spec = resolve_python_launch_spec()?;
    // 先探测是否已可导入，避免每次都跑 pip。
    let mut probe = Command::new(&spec.program);
    probe
        .args(&spec.base_args)
        .args(["-c", "import trafilatura"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_python_process_no_window(&mut probe);
    if let Ok(status) = probe.status() {
        if status.success() {
            return Ok(true);
        }
    }

    let _ = ensure_pip_available(&spec)?;
    info!("[PythonRuntime] installing trafilatura for flipboard extraction");
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.base_args)
        .args([
            "-m",
            "pip",
            "install",
            "-i",
            PYTHON_PIP_INDEX_URL,
            "--trusted-host",
            PYTHON_PIP_TRUSTED_HOST,
            "trafilatura",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_python_process_no_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("install trafilatura failed: {e}"))?;
    // 安装后 worker 进程内的解释器已缓存旧的 import 失败，重启 worker 让其重新加载。
    let _ = clear_python_worker();
    Ok(output.status.success())
}

fn send_one_shot_worker_request(payload: &Value) -> Result<Value, String> {
    let worker_cmd = payload
        .get("cmd")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let timeout_secs = python_worker_timeout_secs_for_cmd(worker_cmd);
    let spec = resolve_python_launch_spec()?;
    let worker_script_path = ensure_worker_script(&spec.runtime_dir)?;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.base_args)
        .arg("-u")
        .arg(&worker_script_path)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&spec.runtime_dir);
    apply_python_process_no_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn one-shot python worker failed: {e}"))?;
    let pid = child.id();
    let watchdog = start_python_worker_request_watchdog(
        pid,
        worker_cmd.to_string(),
        String::new(),
        String::new(),
        timeout_secs,
    );
    let line = serde_json::to_string(payload)
        .map_err(|e| format!("serialize one-shot worker request failed: {e}"))?;
    info!(
        cmd = worker_cmd,
        pid,
        payload_len = line.len(),
        timeout_secs,
        "[PythonRuntime] sending one-shot worker request"
    );

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("write one-shot worker stdin failed: {e}"))?;
        stdin
            .write_all(b"\n")
            .map_err(|e| format!("write one-shot worker newline failed: {e}"))?;
        stdin
            .flush()
            .map_err(|e| format!("flush one-shot worker stdin failed: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("wait one-shot worker output failed: {e}"))?;
    watchdog.store(true, Ordering::SeqCst);

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "one-shot python worker failed: status={}, stdout={}, stderr={}",
            output.status, stdout, stderr
        ));
    }
    if stdout.is_empty() {
        return Err(format!(
            "one-shot python worker returned empty stdout, stderr={stderr}"
        ));
    }

    let response_line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string();
    info!(
        cmd = worker_cmd,
        pid,
        response_len = response_line.len(),
        "[PythonRuntime] received one-shot worker response"
    );

    serde_json::from_str::<Value>(&response_line)
        .map_err(|e| format!("parse one-shot worker response failed: {e}, stdout={stdout}, stderr={stderr}"))
}

fn list_python_packages(spec: &PythonLaunchSpec) -> Vec<String> {
    // 返回完整列表（不再截断到 18 个）：工作台“已安装的包”要能列出全部依赖并逐个管理。
    // 名称与版本以单个空格拼接，前端按空格切分成 name / version 两列。
    let mut packages = managed_pip_list_json(spec)
        .into_iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?.trim().to_string();
            let version = item.get("version").and_then(Value::as_str).unwrap_or("").trim();
            if version.is_empty() {
                Some(name)
            } else {
                Some(format!("{name} {version}"))
            }
        })
        .collect::<Vec<_>>();
    packages.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    packages
}

fn clear_python_worker() -> Result<(), String> {
    let workers = {
        let mut guard = python_worker_states_cell()
            .lock()
            .map_err(|_| "lock python worker states failed".to_string())?;
        guard.drain().collect::<Vec<_>>()
    };
    for (worker_key, worker) in workers {
        terminate_python_worker_state(&worker_key, worker);
    }
    Ok(())
}

fn clear_python_worker_for_session(session_id: &str) -> Result<(), String> {
    let worker_key = python_worker_key_for_session(session_id);
    if let Some(worker) = remove_python_worker_by_key(&worker_key)? {
        terminate_python_worker_state(&worker_key, worker);
    }
    Ok(())
}

#[command]
pub async fn python_reset_repl() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        clear_python_worker()?;
        let runtime_dir = python_runtime_dir()?;
        let sessions_dir = runtime_dir.join("sessions");
        if sessions_dir.exists() {
            remove_path_if_exists(&sessions_dir)?;
        }
        fs::create_dir_all(&sessions_dir)
            .map_err(|e| format!("recreate python sessions dir failed: {e}"))?;
        info!("[PythonRuntime] worker sessions reset by clearing all session workers");
        Ok(())
    })
    .await
    .map_err(|e| format!("python_reset_repl join failed: {e}"))?
}

#[command]
pub async fn python_runtime_prewarm_session(session_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let worker_key = python_worker_key_for_session(&session_id);
        // 只确保 worker 进程就绪(spawn),不执行任何用户代码,语义无副作用;
        // 把会话冷启动成本移到"打开笔记本"时的后台,首条 cell 就能直接命中已就绪的 worker。
        ensure_python_worker_for_key(&worker_key)?;
        info!(
            worker_key = worker_key.as_str(),
            "[PythonRuntime] prewarmed session worker on open"
        );
        Ok(())
    })
    .await
    .map_err(|e| format!("python_runtime_prewarm_session join failed: {e}"))?
}

#[command]
pub async fn python_exec(request: PyExecRequest) -> Result<PyExecResponse, String> {
    let include_session_vars = request.include_session_vars.unwrap_or(true);
    let session_var_name_count = request
        .session_var_names
        .as_ref()
        .map(|items| items.len())
        .unwrap_or_default();
    let session_var_names = request
        .session_var_names
        .as_ref()
        .filter(|items| !items.is_empty())
        .cloned();
    info!(
        persist = request.persist,
        expect_json = request.expect_json,
        include_session_vars,
        session_var_name_count,
        code_len = request.code.len(),
        "[PythonRuntime] python_exec called"
    );

    let mut vars = request.vars.unwrap_or_default();
    let session_id = vars
        .get("_guaxi_session")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    if !session_id.is_empty() {
        let sandbox_dir = ensure_python_session_sandbox_dir(&session_id)?;
        vars.insert(
            "_guaxi_sandbox_dir".to_string(),
            Value::String(sandbox_dir.to_string_lossy().to_string()),
        );
        info!(
            session_id = session_id.as_str(),
            sandbox_dir = sandbox_dir.to_string_lossy().as_ref(),
            "[PythonRuntime] session sandbox prepared"
        );
    }

    let payload = json!({
        "cmd": "exec",
        "request_id": request.request_id,
        "code": request.code,
        "persist": request.persist,
        "expect_json": request.expect_json,
        "include_session_vars": include_session_vars,
        // 设计说明：
        // 1. `session_var_names = None` 的语义是“返回当前会话里的全部变量”；
        // 2. 旧实现这里用了 `unwrap_or_default()`，会把 `None` 错误地序列化成空数组 `[]`；
        // 3. Python worker 收到 `[]` 后会把它当成“按空名单过滤”，最终稳定返回 0 个变量，
        //    于是就出现“cell 内还能 print(y1)，但右侧会话变量始终为空”的现象；
        // 4. 这里改为把 `None` 保持为 JSON null，只在调用方明确给出变量名列表时才传数组。
        "session_var_names": session_var_names,
        "vars": vars,
    });

    let response = tauri::async_runtime::spawn_blocking(move || send_worker_request(&payload))
        .await
        .map_err(|e| format!("python_exec task join failed: {e}"))??;

    let parsed: PyExecResponse = serde_json::from_value(response)
        .map_err(|e| format!("decode python worker response failed: {e}"))?;

    info!(
        ok = parsed.ok,
        stdout_len = parsed.stdout.len(),
        stderr_len = parsed.stderr.len(),
        has_result_repr = parsed.result_repr.is_some(),
        has_result_json = parsed.result_json.is_some(),
        session_var_count = parsed.session_vars.as_ref().map(|items| items.len()).unwrap_or_default(),
        "[PythonRuntime] python_exec completed"
    );

    Ok(parsed)
}

#[command]
pub async fn python_runtime_get_session_info(
    request: PythonSessionRequest,
) -> Result<PythonSessionInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session_id = request.session_id.trim().to_string();
        if session_id.is_empty() {
            return Err("session_id 不能为空".to_string());
        }

        let sandbox_dir = ensure_python_session_sandbox_dir(&session_id)?;
        info!(
            session_id = session_id.as_str(),
            sandbox_dir = sandbox_dir.to_string_lossy().as_ref(),
            "[PythonRuntime] session info prepared"
        );

        Ok(PythonSessionInfo {
            session_id,
            sandbox_dir: sandbox_dir.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| format!("python_runtime_get_session_info join failed: {e}"))?
}

#[command]
pub async fn python_runtime_drop_session(request: PythonSessionRequest) -> Result<(), String> {
    let session_id = request.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err("session_id 不能为空".to_string());
    }

    let runtime_dir = python_runtime_dir()?;
    let sandbox_dir = python_session_sandbox_dir(&runtime_dir, &session_id);
    clear_python_worker_for_session(&session_id)?;

    if sandbox_dir.exists() {
        remove_path_if_exists(&sandbox_dir)?;
    }

    info!(
        session_id = session_id.as_str(),
        sandbox_dir = sandbox_dir.to_string_lossy().as_ref(),
        "[PythonRuntime] session dropped"
    );
    Ok(())
}

#[command]
pub async fn python_runtime_cancel_request(request: PythonCancelRequest) -> Result<(), String> {
    let request_id = request.request_id.trim().to_string();
    if request_id.is_empty() {
        return Err("request_id 不能为空".to_string());
    }

    let requested_session_id = request
        .session_id
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let clear_session = request.clear_session.unwrap_or(true);

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let active = {
            let mut guard = python_worker_active_requests_cell()
                .lock()
                .map_err(|_| "lock python active request registry failed".to_string())?;
            guard.remove(&request_id)
        };

        let active = match active {
            Some(active) => active,
            None => {
                warn!(
                    request_id = request_id.as_str(),
                    requested_session_id = requested_session_id.as_str(),
                    "[PythonRuntime] cancel request ignored because active request was not found"
                );
                return Ok(());
            }
        };

        if !requested_session_id.is_empty() && requested_session_id != active.session_id {
            warn!(
                request_id = request_id.as_str(),
                requested_session_id = requested_session_id.as_str(),
                active_session_id = active.session_id.as_str(),
                "[PythonRuntime] cancel request session mismatch"
            );
            return Ok(());
        }

        let _ = remove_python_worker_by_key(&active.worker_key)?;
        if let Err(error) = terminate_process_by_pid(active.pid) {
            warn!(
                request_id = request_id.as_str(),
                pid = active.pid,
                error = error.as_str(),
                "[PythonRuntime] cancel request process termination reported an error"
            );
        }

        if clear_session && !active.session_id.trim().is_empty() {
            let runtime_dir = python_runtime_dir()?;
            let sandbox_dir = python_session_sandbox_dir(&runtime_dir, &active.session_id);
            if sandbox_dir.exists() {
                remove_path_if_exists(&sandbox_dir)?;
            }
            info!(
                request_id = request_id.as_str(),
                session_id = active.session_id.as_str(),
                sandbox_dir = sandbox_dir.to_string_lossy().as_ref(),
                "[PythonRuntime] cancelled request session sandbox removed"
            );
        }

        info!(
            request_id = request_id.as_str(),
            worker_key = active.worker_key.as_str(),
            session_id = active.session_id.as_str(),
            cell_id = active.cell_id.as_str(),
            pid = active.pid,
            clear_session,
            "[PythonRuntime] active request cancelled"
        );
        Ok(())
    })
    .await
    .map_err(|e| format!("python_runtime_cancel_request join failed: {e}"))?
}

#[command]
pub async fn python_runtime_get_status() -> Result<PythonRuntimeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let runtime_dir = python_runtime_dir()?;
        let runtime_marker = runtime_marker_path(&runtime_dir);
        let bundled_runtime_present = bundled_python_candidates(&runtime_dir)
            .into_iter()
            .any(|candidate| candidate.is_file());
        let auto_download_ready = !bundled_runtime_present && auto_download_python_available();
        let base_spec = resolve_python_base_launch_spec_internal(true).ok();
        let managed_spec = if runtime_is_initialized(&runtime_dir)
            && managed_python_candidates(&runtime_dir)
                .into_iter()
                .any(|candidate| candidate.is_file())
        {
            managed_python_launch_spec(&runtime_dir, "managed-runtime".to_string()).ok()
        } else {
            None
        };
        let managed_health_error = managed_spec
            .as_ref()
            .and_then(|spec| managed_python_runtime_health(spec).err());
        let (managed_spec, managed_health_error) = auto_repair_managed_runtime_if_needed(
            &runtime_dir,
            managed_spec,
            managed_health_error,
        );
        let initialized = managed_health_error.is_none() && managed_spec.is_some();

        if let Some(error) = managed_health_error.as_ref() {
            warn!(
                runtime_dir = runtime_dir.to_string_lossy().as_ref(),
                error = error.as_str(),
                "[PythonRuntime] managed runtime is still unhealthy after automatic repair attempt"
            );
        }

        let (ready, source, python_path, version, pip_version, packages) = if let Some(spec) = managed_spec.as_ref() {
            if managed_health_error.is_none() {
                let mut version_cmd = Command::new(&spec.program);
                version_cmd.args(&spec.base_args).arg("--version");
                let version = command_output_trimmed(version_cmd)?;
                let pip_version = ensure_pip_available(spec).unwrap_or_default();
                let packages = list_python_packages(spec);
                (
                    true,
                    "managed-runtime".to_string(),
                    spec.program.clone(),
                    version,
                    pip_version,
                    packages,
                )
            } else if let Some(spec) = base_spec.as_ref() {
                let mut version_cmd = Command::new(&spec.program);
                version_cmd.args(&spec.base_args).arg("--version");
                let version = command_output_trimmed(version_cmd)?;
                (
                    true,
                    spec.source.clone(),
                    managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                    format!("{version} (managed runtime damaged, needs rebuild)"),
                    String::new(),
                    Vec::new(),
                )
            } else if auto_download_ready {
                (
                    true,
                    "bundled".to_string(),
                    managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                    format!(
                        "Python {} (managed runtime damaged, initialization will rebuild it)",
                        auto_download_python_version_label()
                    ),
                    String::new(),
                    Vec::new(),
                )
            } else {
                (
                    false,
                    "bundled-missing".to_string(),
                    managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                    String::new(),
                    String::new(),
                    Vec::new(),
                )
            }
        } else if let Some(spec) = base_spec.as_ref() {
            let mut version_cmd = Command::new(&spec.program);
            version_cmd.args(&spec.base_args).arg("--version");
            let version = command_output_trimmed(version_cmd)?;
            (
                true,
                spec.source.clone(),
                managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                version,
                String::new(),
                Vec::new(),
            )
        } else if auto_download_ready {
            (
                true,
                "bundled".to_string(),
                managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                format!(
                    "Python {} (首次初始化时自动下载安装)",
                    auto_download_python_version_label()
                ),
                String::new(),
                Vec::new(),
            )
        } else {
            (
                false,
                "bundled-missing".to_string(),
                managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                String::new(),
                String::new(),
                Vec::new(),
            )
        };

        let worker_alive = {
            let workers = {
                let guard = python_worker_states_cell()
                    .lock()
                    .map_err(|_| "lock python worker states failed".to_string())?;
                guard.values().cloned().collect::<Vec<_>>()
            };
            if !initialized {
                false
            } else {
                workers.into_iter().any(|worker| {
                    worker
                        .lock()
                        .map(|mut state| worker_is_alive(&mut state))
                        .unwrap_or(false)
                })
            }
        };

        let uv_version = existing_uv_binary(&runtime_dir)
            .and_then(|uv_path| probe_uv_binary(&uv_path).ok())
            .map(|value| value.trim().to_string())
            .unwrap_or_default();

        Ok(PythonRuntimeStatus {
            ready,
            source,
            python_path,
            runtime_dir: runtime_dir.to_string_lossy().to_string(),
            version,
            pip_version,
            worker_alive,
            packages,
            bundled_runtime_present,
            initialized,
            worker_script_ready: worker_script_ready(&runtime_dir),
            runtime_marker_path: runtime_marker.to_string_lossy().to_string(),
            bootstrap_download_available: auto_download_python_available(),
            bootstrap_download_version: auto_download_python_version_label().to_string(),
            uv_version,
        })
    })
    .await
    .map_err(|e| format!("python_runtime_get_status join failed: {e}"))?
}

#[command]
pub async fn python_runtime_list_downloadable_versions() -> Result<PythonRuntimeDownloadCatalog, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // uv 优先：uv 自带的版本清单跨平台可用；uv 尚未就绪时再回退到旧目录。
        match fetch_python_download_versions_catalog_via_uv() {
            Ok(catalog) => {
                info!(
                    count = catalog.versions.len(),
                    default_version = catalog.default_version.as_str(),
                    "[PythonRuntime] fetched downloadable python versions via uv"
                );
                return Ok(catalog);
            }
            Err(error) => warn!(
                error = error.as_str(),
                "[PythonRuntime] uv python list unavailable, fallback to legacy catalog"
            ),
        }

        if !auto_download_python_available() {
            return Ok(python_download_versions_fallback());
        }

        #[cfg(windows)]
        {
            match fetch_python_download_versions_catalog() {
                Ok(catalog) => {
                    info!(
                        count = catalog.versions.len(),
                        default_version = catalog.default_version.as_str(),
                        "[PythonRuntime] fetched downloadable python versions catalog"
                    );
                    Ok(catalog)
                }
                Err(error) => {
                    warn!(
                        error = error.as_str(),
                        "[PythonRuntime] failed to fetch downloadable python versions catalog, fallback to built-in list"
                    );
                    Ok(python_download_versions_fallback())
                }
            }
        }

        #[cfg(not(windows))]
        {
            Ok(python_download_versions_fallback())
        }
    })
    .await
    .map_err(|e| format!("python_runtime_list_downloadable_versions join failed: {e}"))?
}

#[command]
pub async fn python_runtime_initialize(
    app: AppHandle,
    op_id: Option<String>,
    request: Option<PythonRuntimeInitializeRequest>,
) -> Result<PythonRuntimeInitResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // 切换版本/初始化的下载与建环境进度按 op_id 流式回传前端；作用域结束自动清理。
        let _sink_guard = begin_progress_scope(&app, op_id);
        let requested_download_version = request
            .as_ref()
            .and_then(|value| value.download_version.as_deref())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let spec = ensure_managed_python_launch_spec_with_download_version(requested_download_version)?;
        let pip_version = ensure_pip_available(&spec)?;
        let _ = ensure_worker_script(&spec.runtime_dir)?;
        clear_python_worker()?;
        let _ = with_python_worker_for_key(PY_WORKER_DEFAULT_KEY, |_| Ok(()))?;
        let marker = match read_runtime_marker(&spec.runtime_dir)? {
            Some(existing) => existing,
            None => write_runtime_marker(
                &spec,
                &pip_version,
                requested_download_version.unwrap_or_default(),
                false,
            )?,
        };

        info!(
            runtime_dir = %spec.runtime_dir.to_string_lossy(),
            source = spec.source.as_str(),
            "[PythonRuntime] runtime initialized"
        );

        Ok(PythonRuntimeInitResponse {
            runtime_dir: spec.runtime_dir.to_string_lossy().to_string(),
            python_path: spec.program,
            pip_version,
            download_version: marker.download_version,
            used_download_bootstrap: marker.used_download_bootstrap,
        })
    })
    .await
    .map_err(|e| format!("python_runtime_initialize join failed: {e}"))?
}

#[command]
pub async fn python_runtime_inspect_archive(
    archive_path: String,
) -> Result<PythonRuntimeArchiveInspection, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let trimmed = archive_path.trim();
        if trimmed.is_empty() {
            return Err("archive_path is empty".to_string());
        }

        let zip_path = PathBuf::from(trimmed);
        if !zip_path.is_file() {
            return Err(format!("找不到 Python 运行时压缩包: {}", zip_path.to_string_lossy()));
        }
        if !zip_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            return Err("Python 运行时压缩包仅支持 .zip".to_string());
        }

        inspect_runtime_archive_path(&zip_path)
    })
    .await
    .map_err(|e| format!("python_runtime_inspect_archive join failed: {e}"))?
}

#[command]
pub async fn python_runtime_install_archive(archive_path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let trimmed = archive_path.trim();
        if trimmed.is_empty() {
            return Err("archive_path is empty".to_string());
        }

        let zip_path = PathBuf::from(trimmed);
        if !zip_path.is_file() {
            return Err(format!("找不到 Python 运行时压缩包: {}", zip_path.to_string_lossy()));
        }
        if !zip_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            return Err("Python 运行时压缩包仅支持 .zip".to_string());
        }

        let runtime_dir = python_runtime_dir()?;
        let exe_root = exe_dir()?;
        let temp_dir = exe_root
            .join("tmp")
            .join(format!("python_runtime_install_{}", Uuid::new_v4()));

        remove_path_if_exists(&temp_dir)?;
        fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

        let result: Result<String, String> = (|| {
            info!(
                archive_path = zip_path.to_string_lossy().as_ref(),
                temp_dir = temp_dir.to_string_lossy().as_ref(),
                "[PythonRuntime] extracting runtime archive"
            );

            extract_zip_to_dir(&zip_path, &temp_dir)?;
            let packaged_runtime_dir = find_dir_by_name(&temp_dir, "python_runtime", 5)
                .unwrap_or_else(|| temp_dir.clone());

            clear_python_worker()?;
            remove_path_if_exists(&runtime_dir)?;
            copy_dir_recursive(&packaged_runtime_dir, &runtime_dir)?;

            if !bundled_python_candidates(&runtime_dir)
                .into_iter()
                .any(|candidate| candidate.is_file())
            {
                return Err(format!(
                    "安装后的 Python 运行时校验失败: {}",
                    runtime_dir.to_string_lossy()
                ));
            }

            Ok(runtime_dir.to_string_lossy().to_string())
        })();

        let _ = remove_path_if_exists(&temp_dir);
        result
    })
    .await
    .map_err(|e| format!("python_runtime_install_archive join failed: {e}"))?
}

#[command]
pub async fn python_runtime_install_packages(
    app: AppHandle,
    op_id: Option<String>,
    request: PythonInstallPackagesRequest,
) -> Result<PythonInstallPackagesResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _sink_guard = begin_progress_scope(&app, op_id);
        let packages: Vec<String> = request
            .packages
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
        if packages.is_empty() {
            return Err("未提供需要安装的 Python 包。".to_string());
        }

        let spec = resolve_python_launch_spec()?;
        let _ = ensure_pip_available(&spec)?;

        info!(packages = packages.join(", ").as_str(), source = spec.source.as_str(), "[PythonRuntime] installing packages");

        let (ok, stdout, stderr) = run_managed_package_op(&spec, "install", &packages)?;
        clear_python_worker()?;

        Ok(PythonInstallPackagesResponse { ok, stdout, stderr })
    })
    .await
    .map_err(|e| format!("python_runtime_install_packages join failed: {e}"))?
}

#[command]
pub async fn python_runtime_manage_packages(
    app: AppHandle,
    op_id: Option<String>,
    request: PythonManagePackagesRequest,
) -> Result<PythonInstallPackagesResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _sink_guard = begin_progress_scope(&app, op_id);
        let action = request.action.trim().to_lowercase();
        let packages: Vec<String> = request
            .packages
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();

        if packages.is_empty() {
            return Err("未提供需要处理的 Python 包。".to_string());
        }

        if !matches!(action.as_str(), "install" | "upgrade" | "reinstall" | "uninstall") {
            return Err(format!("不支持的 Python 包管理动作：{action}"));
        }

        let spec = resolve_python_launch_spec()?;
        let _ = ensure_pip_available(&spec)?;

        info!(
            action = action.as_str(),
            packages = packages.join(", ").as_str(),
            source = spec.source.as_str(),
            "[PythonRuntime] manage packages"
        );

        // 统一走 uv pip（缺失时回退 python -m pip），前端再根据 action 给出更贴近用户语义的反馈。
        let (ok, stdout, stderr) = run_managed_package_op(&spec, action.as_str(), &packages)?;
        clear_python_worker()?;

        Ok(PythonInstallPackagesResponse { ok, stdout, stderr })
    })
    .await
    .map_err(|e| format!("python_runtime_manage_packages join failed: {e}"))?
}

/// 升级 uv 本身。我们的 uv 是从 GitHub release 直接下载的 zip，`uv self update` 不支持
/// （它只认标准安装脚本装的 uv），所以这里改为“重新拉取最新版覆盖安装”，等价于升级。
#[command]
pub async fn python_runtime_upgrade_uv(
    app: AppHandle,
    op_id: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _sink_guard = begin_progress_scope(&app, op_id);
        let runtime_dir = python_runtime_dir()?;
        #[cfg(windows)]
        {
            emit_progress_line("stdout", "正在从 GitHub 拉取最新版 uv…");
            let uv_path = download_uv_binary(&runtime_dir)?;
            let version = probe_uv_binary(&uv_path)?;
            emit_progress_line("stdout", &format!("uv 已更新为：{version}"));
            info!(version = version.as_str(), "[PythonRuntime] uv upgraded via re-download");
            Ok(version)
        }
        #[cfg(not(windows))]
        {
            let _ = runtime_dir;
            Err("当前平台的 uv 由系统包管理器维护，请用系统方式升级（如 brew upgrade uv）。".to_string())
        }
    })
    .await
    .map_err(|e| format!("python_runtime_upgrade_uv join failed: {e}"))?
}

#[command]
pub async fn python_runtime_import_table_file(
    request: PythonImportTableRequest,
) -> Result<PythonImportTableResponse, String> {
    let payload = json!({
        "cmd": "import_table",
        "path": request.path,
    });

    let response = tauri::async_runtime::spawn_blocking(move || send_one_shot_worker_request(&payload))
        .await
        .map_err(|e| format!("python_runtime_import_table_file join failed: {e}"))??;

    Ok(PythonImportTableResponse {
        ok: response.get("ok").and_then(Value::as_bool).unwrap_or(false),
        error: response
            .get("error")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        table: response.get("table").cloned(),
    })
}

#[command]
pub async fn python_runtime_get_table_rows(
    request: PythonTableRowsRequest,
) -> Result<PythonTableRowsResponse, String> {
    let payload = json!({
        "cmd": "table_slice",
        "handle": request.handle,
        "session_id": request.session_id.unwrap_or_default(),
        "table_key": request.table_key.unwrap_or_default(),
        "manifest_path": request.manifest_path.unwrap_or_default(),
        "offset": request.offset.unwrap_or(0),
        "limit": request.limit.unwrap_or(200),
    });

    let response = tauri::async_runtime::spawn_blocking(move || send_worker_request(&payload))
        .await
        .map_err(|e| format!("python_runtime_get_table_rows join failed: {e}"))??;

    let parsed: PythonTableRowsResponse = serde_json::from_value(response)
        .map_err(|e| format!("decode python table rows response failed: {e}"))?;

    info!(
        ok = parsed.ok,
        table_key = parsed.table_key.as_deref().unwrap_or_default(),
        row_count = parsed.row_count.unwrap_or_default(),
        returned_rows = parsed.rows.as_ref().map(|rows| rows.len()).unwrap_or_default(),
        "[PythonRuntime] table rows response decoded"
    );

    Ok(parsed)
}

#[command]
pub async fn python_runtime_export_table_result(
    request: PythonTableExportRequest,
) -> Result<PythonTableExportResponse, String> {
    let payload = json!({
        "cmd": "table_export",
        "handle": request.handle,
        "session_id": request.session_id.unwrap_or_default(),
        "table_key": request.table_key.unwrap_or_default(),
        "manifest_path": request.manifest_path.unwrap_or_default(),
        "output_path": request.output_path,
        "format": request.format,
        "scope": request.scope.unwrap_or_else(|| "table".to_string()),
    });

    let response = tauri::async_runtime::spawn_blocking(move || send_worker_request(&payload))
        .await
        .map_err(|e| format!("python_runtime_export_table_result join failed: {e}"))??;

    let parsed: PythonTableExportResponse = serde_json::from_value(response)
        .map_err(|e| format!("decode python table export response failed: {e}"))?;

    info!(
        ok = parsed.ok,
        format = parsed.format.as_deref().unwrap_or_default(),
        scope = parsed.scope.as_deref().unwrap_or_default(),
        table_count = parsed.table_count.unwrap_or_default(),
        output_path = parsed.output_path.as_deref().unwrap_or_default(),
        "[PythonRuntime] table export response decoded"
    );

    Ok(parsed)
}
