use crate::guaxiservices::bundled_resources;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::command;
use tracing::{info, warn};

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

fn resolve_python_launch_spec() -> Result<PythonLaunchSpec, String> {
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
        let spec = resolve_python_launch_spec()?;

        let mut version_cmd = Command::new(&spec.program);
        version_cmd.args(&spec.base_args).arg("--version");
        let version = command_output_trimmed(version_cmd)?;

        let pip_version = ensure_pip_available(&spec).unwrap_or_default();
        let packages = list_python_packages(&spec);
        let worker_alive = {
            let mut guard = python_worker_state_cell()
                .lock()
                .map_err(|_| "lock python worker state failed".to_string())?;
            match guard.as_mut() {
                Some(state) => worker_is_alive(state),
                None => false,
            }
        };

        Ok(PythonRuntimeStatus {
            ready: true,
            source: spec.source,
            python_path: spec.program,
            runtime_dir: spec.runtime_dir.to_string_lossy().to_string(),
            version,
            pip_version,
            worker_alive,
            packages,
        })
    })
    .await
    .map_err(|e| format!("python_runtime_get_status join failed: {e}"))?
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
