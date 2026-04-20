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
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::command;
use tracing::{info, warn};
use uuid::Uuid;
use zip::ZipArchive;

#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;

static PY_WORKER_STATE: OnceLock<Mutex<Option<PythonWorkerState>>> = OnceLock::new();

const PY_WORKER_FILE_NAME: &str = "guaxi_python_worker.py";
const PY_WORKER_SOURCE: &str = include_str!("../../scripts/guaxi_python_worker.py");

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_python_process_no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_python_process_no_window(_cmd: &mut Command) {}

#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_VERSION: &str = "3.13.12";
#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_BASE_URL: &str = "https://www.python.org/ftp/python";
#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_USER_AGENT: &str = "guaxi-python-runtime-bootstrap/1.0";
#[cfg(windows)]
const AUTO_DOWNLOAD_PYTHON_WINDOWS_PAGE_URL: &str = "https://www.python.org/downloads/windows/";
const GET_PIP_BOOTSTRAP_URL: &str = "https://bootstrap.pypa.io/get-pip.py";
const PYTHON_PIP_INDEX_URL: &str = "https://pypi.tuna.tsinghua.edu.cn/simple";
const PYTHON_PIP_TRUSTED_HOST: &str = "pypi.tuna.tsinghua.edu.cn";
const PYTHON_RUNTIME_RECOMMENDED_PACKAGES: &[&str] = &["pandas", "matplotlib", "openpyxl", "ipython"];
const PYTHON_RUNTIME_BOOTSTRAP_PACKAGE_NAMES: &[&str] =
    &["pip", "setuptools", "wheel", "virtualenv"];

#[derive(Debug, Deserialize)]
pub struct PyExecRequest {
    pub code: String,
    pub persist: bool,
    pub expect_json: bool,
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
}

fn python_worker_state_cell() -> &'static Mutex<Option<PythonWorkerState>> {
    PY_WORKER_STATE.get_or_init(|| Mutex::new(None))
}

fn python_runtime_dir() -> Result<PathBuf, String> {
    let dir = bundled_resources::ensure_relative_dir("utils/python_runtime")?;
    fs::create_dir_all(&dir).map_err(|e| format!("create python runtime dir failed: {e}"))?;
    Ok(dir)
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

        if file_name_str.to_ascii_lowercase().ends_with(".dll") {
            let scripts_target_path = scripts_dir.join(&file_name);
            if scripts_target_path.exists() {
                let _ = fs::remove_file(&scripts_target_path);
            }
            fs::copy(&source_path, &scripts_target_path)
                .map_err(|e| format!("copy runtime dll to managed Scripts failed: {e}"))?;
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
    package.trim().to_ascii_lowercase().replace('_', "-")
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
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.base_args)
        .args(["-m", "pip", "list", "--format=json"]);
    match command_output_trimmed(command) {
        Ok(output) => serde_json::from_str::<Vec<Value>>(&output)
            .ok()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
            .map(|name| normalize_python_package_name(&name))
            .filter(|name| !name.is_empty())
            .collect(),
        Err(error) => {
            warn!(
                source = spec.source.as_str(),
                error = error.as_str(),
                "[PythonRuntime] list installed python package names failed"
            );
            HashSet::new()
        }
    }
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

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.base_args)
        .args(["-m", "pip", "install"])
        .args(&missing_packages)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_python_process_no_window(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("install managed runtime packages failed: {e}"))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "install managed runtime packages failed during {reason}: status={}, stdout={}, stderr={}",
            output.status, stdout, stderr
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

fn managed_python_runtime_health(spec: &PythonLaunchSpec) -> Result<(), String> {
    let mut version_cmd = Command::new(&spec.program);
    version_cmd.args(&spec.base_args).arg("--version");
    let _ = command_output_trimmed(version_cmd)
        .map_err(|e| format!("managed python probe failed: {e}"))?;

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

    // 设计说明：
    // 1. pip 安装、worker 启动、状态读取这些运行期操作，只要托管 `.venv` 当前可用，就应该直接复用它。
    // 2. 只有在“首次创建”或“已损坏需要重建”时，才必须依赖内置的 bundled base Python。
    // 3. 上一版把 bundled base 解析提前到了函数入口，导致即使 `.venv` 已经健康可用，安装包也会被
    //    “未找到内置基础运行时”误拦截。
    if let Some(spec) = managed_spec.as_ref() {
        if !managed_runtime_broken {
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
            return Ok(spec.clone());
        }
    }

    let base_spec = resolve_bootstrap_python_base_launch_spec(download_version)?;

    if managed_runtime_broken {
        warn!(
            venv_dir = venv_dir.to_string_lossy().as_ref(),
            error = managed_runtime_health_error.as_deref().unwrap_or("unknown"),
            "[PythonRuntime] detected broken managed virtual environment, recreate it from base python"
        );
        remove_path_if_exists(&venv_dir)?;
        let marker_path = runtime_marker_path(&runtime_dir);
        let _ = remove_path_if_exists(&marker_path);
    }

    if !has_managed_python || managed_runtime_broken {
        fs::create_dir_all(&runtime_dir).map_err(|e| format!("create python runtime dir failed: {e}"))?;
        create_managed_virtualenv(&base_spec, &venv_dir)?;
        sync_managed_runtime_support_files(&base_spec, &venv_dir)?;
        for recommended_package in python_runtime_recommended_packages() {
            if !managed_package_snapshot.iter().any(|item| {
                normalize_python_package_name(item)
                    == normalize_python_package_name(&recommended_package)
            }) {
                managed_package_snapshot.push(recommended_package);
            }
        }
        info!(
            source = base_spec.source.as_str(),
            venv_dir = venv_dir.to_string_lossy().as_ref(),
            "[PythonRuntime] managed virtual environment created from managed bootstrap pipeline"
        );
    }

    ensure_managed_pip_mirror(&runtime_dir)?;
    let managed_spec = managed_python_launch_spec(&runtime_dir, base_spec.source.clone())?;
    let pip_version = ensure_pip_available(&managed_spec)?;
    ensure_python_packages_installed(
        &managed_spec,
        &managed_package_snapshot,
        if managed_runtime_broken {
            "managed-runtime-repair"
        } else if !has_managed_python {
            "managed-runtime-initialize"
        } else {
            "managed-runtime-ensure"
        },
    )?;
    let _ = write_runtime_marker(
        &managed_spec,
        &pip_version,
        download_version.unwrap_or_default(),
        did_use_download_bootstrap(&runtime_dir, &base_spec.source),
    )?;
    cleanup_bootstrap_runtime_artifacts(&runtime_dir, "managed-runtime-ready");
    Ok(managed_spec)
}

fn resolve_python_launch_spec() -> Result<PythonLaunchSpec, String> {
    ensure_managed_python_launch_spec()
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
        .stderr(Stdio::null())
        .current_dir(&spec.runtime_dir);
    apply_python_process_no_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn python worker failed: {e}"))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "python worker stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "python worker stdout unavailable".to_string())?;

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
    })
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

fn with_python_worker<R>(f: impl FnOnce(&mut PythonWorkerState) -> Result<R, String>) -> Result<R, String> {
    let mut guard = python_worker_state_cell()
        .lock()
        .map_err(|_| "lock python worker state failed".to_string())?;

    let need_spawn = match guard.as_mut() {
        Some(state) => !worker_is_alive(state),
        None => true,
    };

    if need_spawn {
        *guard = None;
        let spec = resolve_python_launch_spec()?;
        *guard = Some(spawn_python_worker(&spec)?);
    }

    let state = guard
        .as_mut()
        .ok_or_else(|| "python worker not available".to_string())?;
    f(state)
}

fn send_worker_request(payload: &Value) -> Result<Value, String> {
    with_python_worker(|state| {
        let line = serde_json::to_string(payload)
            .map_err(|e| format!("serialize worker request failed: {e}"))?;
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

        let mut response_line = String::new();
        let bytes_read = state
            .stdout
            .read_line(&mut response_line)
            .map_err(|e| format!("read worker stdout failed: {e}"))?;
        if bytes_read == 0 {
            return Err("python worker closed stdout unexpectedly".to_string());
        }

        serde_json::from_str::<Value>(response_line.trim())
            .map_err(|e| format!("parse worker response failed: {e}"))
    })
}

fn list_python_packages(spec: &PythonLaunchSpec) -> Vec<String> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.base_args)
        .args(["-m", "pip", "list", "--format=json"]);
    match command_output_trimmed(command) {
        Ok(output) => serde_json::from_str::<Vec<Value>>(&output)
            .ok()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let name = item.get("name").and_then(Value::as_str)?.trim();
                let version = item.get("version").and_then(Value::as_str).unwrap_or("").trim();
                if version.is_empty() {
                    Some(name.to_string())
                } else {
                    Some(format!("{name} {version}"))
                }
            })
            .take(18)
            .collect(),
        Err(error) => {
            warn!(error = error.as_str(), "[PythonRuntime] list pip packages failed");
            Vec::new()
        }
    }
}

fn clear_python_worker() -> Result<(), String> {
    let mut guard = python_worker_state_cell()
        .lock()
        .map_err(|_| "lock python worker state failed".to_string())?;
    if let Some(state) = guard.as_mut() {
        let _ = state.child.kill();
    }
    *guard = None;
    Ok(())
}

#[command]
pub async fn python_reset_repl() -> Result<(), String> {
    let payload = json!({ "cmd": "reset" });
    let response = tauri::async_runtime::spawn_blocking(move || send_worker_request(&payload))
        .await
        .map_err(|e| format!("python_reset_repl join failed: {e}"))??;

    if response.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        info!("[PythonRuntime] worker sessions reset");
        Ok(())
    } else {
        Err(response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("reset python worker failed")
            .to_string())
    }
}

#[command]
pub async fn python_exec(request: PyExecRequest) -> Result<PyExecResponse, String> {
    info!(
        persist = request.persist,
        expect_json = request.expect_json,
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
        "code": request.code,
        "persist": request.persist,
        "expect_json": request.expect_json,
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
    let payload = json!({
        "cmd": "drop_session",
        "session_id": session_id,
    });

    let response = tauri::async_runtime::spawn_blocking(move || send_worker_request(&payload))
        .await
        .map_err(|e| format!("python_runtime_drop_session join failed: {e}"))??;

    let ok = response.get("ok").and_then(Value::as_bool).unwrap_or(false);
    if !ok {
        return Err(
            response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("drop python session failed")
                .to_string(),
        );
    }

    if sandbox_dir.exists() {
        remove_path_if_exists(&sandbox_dir)?;
    }

    info!(
        sandbox_dir = sandbox_dir.to_string_lossy().as_ref(),
        "[PythonRuntime] session dropped"
    );
    Ok(())
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
            let mut guard = python_worker_state_cell()
                .lock()
                .map_err(|_| "lock python worker state failed".to_string())?;
            match (initialized, guard.as_mut()) {
                (true, Some(state)) => worker_is_alive(state),
                _ => false,
            }
        };

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
        })
    })
    .await
    .map_err(|e| format!("python_runtime_get_status join failed: {e}"))?
}

#[command]
pub async fn python_runtime_list_downloadable_versions() -> Result<PythonRuntimeDownloadCatalog, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
    request: Option<PythonRuntimeInitializeRequest>,
) -> Result<PythonRuntimeInitResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let requested_download_version = request
            .as_ref()
            .and_then(|value| value.download_version.as_deref())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        let spec = ensure_managed_python_launch_spec_with_download_version(requested_download_version)?;
        let pip_version = ensure_pip_available(&spec)?;
        let _ = ensure_worker_script(&spec.runtime_dir)?;
        clear_python_worker()?;
        let _ = with_python_worker(|_| Ok(()))?;
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
    request: PythonInstallPackagesRequest,
) -> Result<PythonInstallPackagesResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
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

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.base_args)
            .args(["-m", "pip", "install"])
            .args(&packages)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        apply_python_process_no_window(&mut command);
        let output = command
            .output()
            .map_err(|e| format!("install python packages failed: {e}"))?;
        clear_python_worker()?;

        Ok(PythonInstallPackagesResponse {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    })
    .await
    .map_err(|e| format!("python_runtime_install_packages join failed: {e}"))?
}

#[command]
pub async fn python_runtime_manage_packages(
    request: PythonManagePackagesRequest,
) -> Result<PythonInstallPackagesResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
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

        let pip_args: Vec<String> = match action.as_str() {
            "install" => {
                let mut args = vec!["install".to_string()];
                args.extend(packages.iter().cloned());
                args
            }
            "upgrade" => {
                let mut args = vec!["install".to_string(), "--upgrade".to_string()];
                args.extend(packages.iter().cloned());
                args
            }
            "reinstall" => {
                let mut args = vec![
                    "install".to_string(),
                    "--upgrade".to_string(),
                    "--force-reinstall".to_string(),
                ];
                args.extend(packages.iter().cloned());
                args
            }
            "uninstall" => {
                let mut args = vec!["uninstall".to_string(), "-y".to_string()];
                args.extend(packages.iter().cloned());
                args
            }
            _ => {
                return Err(format!("不支持的 Python 包管理动作：{action}"));
            }
        };

        let spec = resolve_python_launch_spec()?;
        let _ = ensure_pip_available(&spec)?;

        info!(
            action = action.as_str(),
            packages = packages.join(", ").as_str(),
            source = spec.source.as_str(),
            "[PythonRuntime] manage packages"
        );

        // 统一走 pip 子命令，前端再根据 action 给出更贴近用户语义的反馈。
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.base_args)
            .args(["-m", "pip"])
            .args(&pip_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        apply_python_process_no_window(&mut command);
        let output = command
            .output()
            .map_err(|e| format!("manage python packages failed: {e}"))?;
        clear_python_worker()?;

        Ok(PythonInstallPackagesResponse {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    })
    .await
    .map_err(|e| format!("python_runtime_manage_packages join failed: {e}"))?
}

#[command]
pub async fn python_runtime_import_table_file(
    request: PythonImportTableRequest,
) -> Result<PythonImportTableResponse, String> {
    let payload = json!({
        "cmd": "import_table",
        "path": request.path,
    });

    let response = tauri::async_runtime::spawn_blocking(move || send_worker_request(&payload))
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
