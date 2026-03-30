use crate::guaxiservices::bundled_resources;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::command;
use tracing::{info, warn};
use uuid::Uuid;
use zip::ZipArchive;

static PY_WORKER_STATE: OnceLock<Mutex<Option<PythonWorkerState>>> = OnceLock::new();

const PY_WORKER_FILE_NAME: &str = "guaxi_python_worker.py";
const PY_WORKER_SOURCE: &str = include_str!("../../scripts/guaxi_python_worker.py");

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
}

#[derive(Debug, Deserialize)]
pub struct PythonInstallPackagesRequest {
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
    let output = command
        .output()
        .map_err(|e| format!("spawn process failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("process exited with status {}", output.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn resolve_python_base_launch_spec() -> Result<PythonLaunchSpec, String> {
    let runtime_dir = python_runtime_dir()?;

    for candidate in bundled_python_candidates(&runtime_dir) {
        if candidate.is_file() {
            return Ok(PythonLaunchSpec {
                program: candidate.to_string_lossy().to_string(),
                base_args: Vec::new(),
                source: "bundled".to_string(),
                runtime_dir,
            });
        }
    }

    if let Ok(custom) = std::env::var("GUAXI_PYTHON") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Ok(PythonLaunchSpec {
                program: trimmed.to_string(),
                base_args: Vec::new(),
                source: "env".to_string(),
                runtime_dir,
            });
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
        let mut command = Command::new(&program);
        command.args(&base_args).arg("--version");
        if command.output().is_ok() {
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
    let base_spec = resolve_python_base_launch_spec()?;
    let runtime_dir = base_spec.runtime_dir.clone();
    let venv_dir = managed_venv_dir(&runtime_dir);
    let has_managed_python = managed_python_candidates(&runtime_dir)
        .into_iter()
        .any(|candidate| candidate.is_file());

    if !has_managed_python {
        fs::create_dir_all(&runtime_dir).map_err(|e| format!("create python runtime dir failed: {e}"))?;

        let mut command = Command::new(&base_spec.program);
        command
            .args(&base_spec.base_args)
            .args(["-m", "venv"])
            .arg(&venv_dir)
            .current_dir(&runtime_dir);
        let _ = command_output_trimmed(command)
            .map_err(|e| format!("create Guaxi python virtualenv failed: {e}"))?;
    }

    ensure_managed_pip_mirror(&runtime_dir)?;
    let managed_spec = managed_python_launch_spec(&runtime_dir, base_spec.source.clone())?;
    let _ = ensure_pip_available(&managed_spec)?;
    Ok(managed_spec)
}

fn resolve_python_launch_spec() -> Result<PythonLaunchSpec, String> {
    ensure_managed_python_launch_spec()
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(&spec.runtime_dir);

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

    let payload = json!({
        "cmd": "exec",
        "code": request.code,
        "persist": request.persist,
        "expect_json": request.expect_json,
        "vars": request.vars.unwrap_or_default(),
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
pub async fn python_runtime_get_status() -> Result<PythonRuntimeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let base_spec = resolve_python_base_launch_spec()?;
        let runtime_dir = base_spec.runtime_dir.clone();
        let runtime_marker = runtime_marker_path(&runtime_dir);
        let bundled_runtime_present = bundled_python_candidates(&runtime_dir)
            .into_iter()
            .any(|candidate| candidate.is_file());
        let initialized = runtime_is_initialized(&runtime_dir)
            && managed_python_candidates(&runtime_dir)
                .into_iter()
                .any(|candidate| candidate.is_file());
        let managed_spec = if initialized {
            managed_python_launch_spec(&runtime_dir, base_spec.source.clone()).ok()
        } else {
            None
        };

        let (python_path, version, pip_version, packages) = if let Some(spec) = managed_spec.as_ref() {
            let mut version_cmd = Command::new(&spec.program);
            version_cmd.args(&spec.base_args).arg("--version");
            let version = command_output_trimmed(version_cmd)?;
            let pip_version = ensure_pip_available(spec).unwrap_or_default();
            let packages = list_python_packages(spec);
            (spec.program.clone(), version, pip_version, packages)
        } else {
            let mut version_cmd = Command::new(&base_spec.program);
            version_cmd.args(&base_spec.base_args).arg("--version");
            let version = command_output_trimmed(version_cmd)?;
            (
                managed_venv_dir(&runtime_dir).to_string_lossy().to_string(),
                version,
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
            ready: true,
            source: base_spec.source,
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
        })
    })
    .await
    .map_err(|e| format!("python_runtime_get_status join failed: {e}"))?
}

#[command]
pub async fn python_runtime_initialize() -> Result<PythonRuntimeInitResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let spec = ensure_managed_python_launch_spec()?;
        let pip_version = ensure_pip_available(&spec)?;
        let _ = ensure_worker_script(&spec.runtime_dir)?;
        clear_python_worker()?;
        let _ = with_python_worker(|_| Ok(()))?;

        let marker_path = runtime_marker_path(&spec.runtime_dir);
        let marker = json!({
            "python_path": spec.program,
            "source": spec.source,
            "pip_version": pip_version,
            "venv_dir": managed_venv_dir(&spec.runtime_dir).to_string_lossy().to_string(),
            "pip_config_path": managed_pip_config_path(&spec.runtime_dir).to_string_lossy().to_string(),
            "pip_index_url": "https://pypi.tuna.tsinghua.edu.cn/simple",
        });
        fs::write(&marker_path, serde_json::to_vec_pretty(&marker).map_err(|e| e.to_string())?)
            .map_err(|e| format!("write python runtime marker failed: {e}"))?;

        info!(
            runtime_dir = %spec.runtime_dir.to_string_lossy(),
            source = spec.source.as_str(),
            "[PythonRuntime] runtime initialized"
        );

        Ok(PythonRuntimeInitResponse {
            runtime_dir: spec.runtime_dir.to_string_lossy().to_string(),
            python_path: spec.program,
            pip_version,
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
