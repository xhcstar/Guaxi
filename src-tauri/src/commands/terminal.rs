use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter};
use tracing::{info, warn};
use uuid::Uuid;

use crate::commands::settingview::{get_terminal_config, TerminalConfig};
use crate::guaxiservices::bundled_resources;

const TERMINAL_OUTPUT_EVENT: &str = "terminal-session-output";
const TERMINAL_EXIT_EVENT: &str = "terminal-session-exit";

type SessionMap = HashMap<String, Arc<TerminalSessionHandle>>;

static TERMINAL_SESSIONS: OnceLock<Mutex<SessionMap>> = OnceLock::new();

fn session_store() -> &'static Mutex<SessionMap> {
    TERMINAL_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalProfile {
    pub id: String,
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub available: bool,
    pub source: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct TerminalProfilesResponse {
    pub default_profile: String,
    pub profiles: Vec<TerminalProfile>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalCreateRequest {
    pub profile_id: String,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TerminalWriteRequest {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
pub struct TerminalResizeRequest {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Deserialize)]
pub struct TerminalCloseRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TerminalSessionSnapshot {
    pub session_id: String,
    pub profile_id: String,
    pub label: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize)]
struct TerminalOutputEvent {
    session_id: String,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
struct TerminalExitEvent {
    session_id: String,
    exit_code: Option<i32>,
}

struct TerminalSessionHandle {
    snapshot: TerminalSessionSnapshot,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn portable_pty::Child + Send>>,
}

#[derive(Debug, Clone)]
struct LaunchTarget {
    program: String,
    args: Vec<String>,
    source: String,
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| format!("resolve current exe failed: {e}"))?;
    exe_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法获取当前程序目录".to_string())
}

fn python_runtime_dir() -> Result<PathBuf, String> {
    let dir = bundled_resources::ensure_relative_dir("utils/python_runtime")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("create python runtime dir failed: {e}"))?;
    Ok(dir)
}

fn split_env_paths(raw: &str) -> Vec<PathBuf> {
    env::split_paths(raw).collect()
}

fn path_ext_candidates() -> Vec<String> {
    if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    } else {
        Vec::new()
    }
}

fn find_executable(candidates: &[&str]) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let roots = split_env_paths(&path_var.to_string_lossy());
    let exts = path_ext_candidates();

    for root in roots {
        for candidate in candidates {
            let candidate_path = root.join(candidate);
            if candidate_path.is_file() {
                return Some(candidate_path);
            }
            if cfg!(windows) && Path::new(candidate).extension().is_none() {
                for ext in &exts {
                    let with_ext = root.join(format!("{candidate}{ext}"));
                    if with_ext.is_file() {
                        return Some(with_ext);
                    }
                }
            }
        }
    }

    None
}

fn split_command_line(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    out.push(current.clone());
                    current.clear();
                }
            }
            '\\' if in_quotes => {
                if let Some('"') = chars.peek().copied() {
                    current.push('"');
                    let _ = chars.next();
                } else {
                    current.push(ch);
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn parse_configured_command(raw: &str) -> Option<(PathBuf, Vec<String>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let direct = PathBuf::from(trimmed);
    if direct.is_file() {
        return Some((direct, Vec::new()));
    }

    let parts = split_command_line(trimmed);
    if parts.is_empty() {
        return None;
    }

    for split_index in (1..=parts.len()).rev() {
        let executable_candidate = parts[..split_index].join(" ");
        let executable_path = PathBuf::from(executable_candidate.trim());
        if executable_path.is_file() {
            let args = parts[split_index..]
                .iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>();
            return Some((executable_path, args));
        }
    }

    None
}

fn dev_source_utils_root() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }

    let exe_dir = exe_dir().ok()?;
    for ancestor in exe_dir.ancestors() {
        let cargo_toml = ancestor.join("Cargo.toml");
        let utils_dir = ancestor.join("utils");
        if cargo_toml.is_file() && utils_dir.is_dir() {
            return Some(utils_dir);
        }
    }

    None
}

fn candidate_runtime_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(runtime_dir) = python_runtime_dir() {
        roots.push(runtime_dir);
    }

    if let Ok(dir) = exe_dir() {
        roots.push(dir.join("utils").join("python_runtime"));
        roots.push(dir.join("python_runtime"));
    }

    if let Some(dev_utils) = dev_source_utils_root() {
        roots.push(dev_utils.join("python_runtime"));
    }

    roots
}

fn managed_python_candidates(runtime_dir: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            runtime_dir.join(".venv").join("Scripts").join("python.exe"),
            runtime_dir.join(".venv").join("python.exe"),
            runtime_dir.join("python.exe"),
            runtime_dir.join("Python").join("python.exe"),
            runtime_dir.join("embedded").join("python.exe"),
        ]
    } else {
        vec![
            runtime_dir.join(".venv").join("bin").join("python"),
            runtime_dir.join(".venv").join("bin").join("python3"),
            runtime_dir.join("bin").join("python3"),
            runtime_dir.join("bin").join("python"),
            runtime_dir.join("python"),
        ]
    }
}

fn ipython_executable_candidates(runtime_dir: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            runtime_dir.join(".venv").join("Scripts").join("ipython.exe"),
            runtime_dir.join(".venv").join("Scripts").join("ipython"),
        ]
    } else {
        vec![runtime_dir.join(".venv").join("bin").join("ipython")]
    }
}

fn quick_runtime_python() -> Option<PathBuf> {
    for root in candidate_runtime_roots() {
        for candidate in managed_python_candidates(&root) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    find_executable(&["python", "python3"])
}

fn quick_runtime_ipython() -> Option<PathBuf> {
    for root in candidate_runtime_roots() {
        for candidate in ipython_executable_candidates(&root) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    find_executable(&["ipython", "ipython.exe"])
}

fn python_supports_ipython(program: &Path) -> bool {
    Command::new(program)
        .args(["-c", "import IPython"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn resolve_ipython_launch() -> Result<LaunchTarget, String> {
    for root in candidate_runtime_roots() {
        for candidate in managed_python_candidates(&root) {
            if candidate.is_file() && python_supports_ipython(&candidate) {
                info!(
                    program = %candidate.to_string_lossy(),
                    runtime_dir = %root.to_string_lossy(),
                    "[Terminal] resolved IPython from managed python runtime"
                );
                return Ok(LaunchTarget {
                    program: candidate.to_string_lossy().to_string(),
                    args: vec![
                        "-m".to_string(),
                        "IPython".to_string(),
                    ],
                    source: "guaxi-runtime".to_string(),
                });
            }
        }
    }

    if let Some(ipython) = quick_runtime_ipython() {
        info!(
            program = %ipython.to_string_lossy(),
            "[Terminal] resolved IPython from executable candidate"
        );
        return Ok(LaunchTarget {
            program: ipython.to_string_lossy().to_string(),
            args: Vec::new(),
            source: "guaxi-runtime".to_string(),
        });
    }

    if let Some(python) = quick_runtime_python().filter(|candidate| python_supports_ipython(candidate))
    {
        info!(
            program = %python.to_string_lossy(),
            "[Terminal] resolved IPython from fallback python"
        );
        return Ok(LaunchTarget {
            program: python.to_string_lossy().to_string(),
            args: vec![
                "-m".to_string(),
                "IPython".to_string(),
            ],
            source: "guaxi-runtime".to_string(),
        });
    }

    Err("未找到可用的 IPython 运行时；请先在 Guaxi Python 环境或系统环境中安装 Python/IPython。".to_string())
}

fn detect_profiles(config: &TerminalConfig) -> Vec<TerminalProfile> {
    let mut profiles = Vec::new();

    let ipython_program = quick_runtime_ipython()
        .or_else(quick_runtime_python)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let ipython_args = if ipython_program.to_lowercase().contains("ipython") {
        Vec::new()
    } else {
        vec![
            "-m".to_string(),
            "IPython".to_string(),
        ]
    };

    profiles.push(TerminalProfile {
        id: "ipython".to_string(),
        label: "IPython".to_string(),
        program: ipython_program,
        args: ipython_args,
        available: true,
        source: "guaxi-runtime".to_string(),
        detail: "使用 Guaxi 当前 Python 环境启动交互式 IPython，真正校验放到创建会话时执行。".to_string(),
    });

    if cfg!(windows) {
        let powershell = find_executable(&["pwsh.exe", "powershell.exe"]);
        profiles.push(TerminalProfile {
            id: "powershell".to_string(),
            label: "PowerShell".to_string(),
            program: powershell
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            args: Vec::new(),
            available: powershell.is_some(),
            source: "auto-detect".to_string(),
            detail: "自动检测系统中的 PowerShell / pwsh。".to_string(),
        });

        let cmd = find_executable(&["cmd.exe"]);
        profiles.push(TerminalProfile {
            id: "cmd".to_string(),
            label: "Command Prompt".to_string(),
            program: cmd
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            args: Vec::new(),
            available: cmd.is_some(),
            source: "auto-detect".to_string(),
            detail: "自动检测 Windows 命令提示符。".to_string(),
        });
    } else {
        let bash = find_executable(&["bash"]);
        profiles.push(TerminalProfile {
            id: "bash".to_string(),
            label: "Bash".to_string(),
            program: bash
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            args: vec!["-i".to_string()],
            available: bash.is_some(),
            source: "auto-detect".to_string(),
            detail: "自动检测系统中的 bash。".to_string(),
        });

        let zsh = find_executable(&["zsh"]);
        profiles.push(TerminalProfile {
            id: "zsh".to_string(),
            label: "Zsh".to_string(),
            program: zsh
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            args: vec!["-i".to_string()],
            available: zsh.is_some(),
            source: "auto-detect".to_string(),
            detail: "自动检测系统中的 zsh。".to_string(),
        });

        let powershell = find_executable(&["pwsh"]);
        profiles.push(TerminalProfile {
            id: "powershell".to_string(),
            label: "PowerShell".to_string(),
            program: powershell
                .as_ref()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            args: Vec::new(),
            available: powershell.is_some(),
            source: "auto-detect".to_string(),
            detail: "自动检测系统中的 pwsh。".to_string(),
        });
    }

    let git_bash = parse_configured_command(&config.git_bash_path);
    profiles.push(TerminalProfile {
        id: "git-bash".to_string(),
        label: "Git Bash".to_string(),
        program: git_bash
            .as_ref()
            .map(|(path, _)| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        args: git_bash
            .as_ref()
            .map(|(_, args)| {
                if args.is_empty() {
                    vec!["-i".to_string(), "-l".to_string()]
                } else {
                    args.clone()
                }
            })
            .unwrap_or_else(|| vec!["-i".to_string(), "-l".to_string()]),
        available: git_bash.is_some(),
        source: "settings".to_string(),
        detail: "使用设置页中配置的 Git Bash 启动命令，支持“可执行文件路径 + 参数”的形式。".to_string(),
    });

    info!(
        profile_count = profiles.len(),
        "[Terminal] detect_profiles finished with lightweight scan"
    );

    profiles
}

fn resolve_profile_launch(
    config: &TerminalConfig,
    profile_id: &str,
) -> Result<(TerminalProfile, LaunchTarget), String> {
    let profiles = detect_profiles(config);
    let profile = profiles
        .into_iter()
        .find(|item| item.id == profile_id)
        .ok_or_else(|| format!("未找到终端配置：{profile_id}"))?;

    let launch = match profile.id.as_str() {
        "ipython" => resolve_ipython_launch()?,
        "git-bash" => {
            let (path, args) = parse_configured_command(&config.git_bash_path).ok_or_else(|| {
                "Git Bash 尚未配置可执行命令，请在设置页填写完整路径，例如 D:\\Git\\bin\\bash.exe -i -l。".to_string()
            })?;
            LaunchTarget {
                program: path.to_string_lossy().to_string(),
                args: if args.is_empty() {
                    vec!["-i".to_string(), "-l".to_string()]
                } else {
                    args
                },
                source: "settings".to_string(),
            }
        }
        "powershell" if cfg!(windows) => {
            if !profile.available {
                return Err("当前系统未检测到可用的 PowerShell。".to_string());
            }
            let mut args = vec!["-NoLogo".to_string()];
            args.extend(profile.args.clone());
            LaunchTarget {
                program: profile.program.clone(),
                args,
                source: profile.source.clone(),
            }
        }
        _ => {
            if !profile.available {
                return Err(format!("终端配置不可用：{}", profile.label));
            }
            LaunchTarget {
                program: profile.program.clone(),
                args: profile.args.clone(),
                source: profile.source.clone(),
            }
        }
    };

    Ok((profile, launch))
}

fn choose_working_dir(requested: &Option<String>) -> Result<PathBuf, String> {
    let requested_path = requested
        .as_ref()
        .map(|value| PathBuf::from(value.trim()))
        .filter(|path| path.exists());

    if let Some(path) = requested_path {
        if path.is_dir() {
            return Ok(path);
        }
        if let Some(parent) = path.parent() {
            if parent.is_dir() {
                return Ok(parent.to_path_buf());
            }
        }
    }

    exe_dir()
}

fn spawn_terminal_reader(
    app: AppHandle,
    session_id: String,
    mut reader: Box<dyn Read + Send>,
    child_handle: Arc<TerminalSessionHandle>,
) {
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    let data = String::from_utf8_lossy(&buffer[..size]).to_string();
                    if !data.is_empty() {
                        let payload = TerminalOutputEvent {
                            session_id: session_id.clone(),
                            data,
                        };
                        if let Err(error) = app.emit(TERMINAL_OUTPUT_EVENT, payload) {
                            warn!(
                                session_id = session_id.as_str(),
                                error = %error,
                                "[Terminal] emit output failed"
                            );
                        }
                    }
                }
                Err(error) => {
                    warn!(
                        session_id = session_id.as_str(),
                        error = %error,
                        "[Terminal] read terminal output failed"
                    );
                    break;
                }
            }
        }

        let exit_code = match child_handle.child.lock() {
            Ok(mut child) => child.wait().ok().map(|status| status.exit_code() as i32),
            Err(error) => {
                warn!(
                    session_id = session_id.as_str(),
                    error = %error,
                    "[Terminal] lock child failed while waiting exit"
                );
                None
            }
        };

        if let Ok(mut sessions) = session_store().lock() {
            sessions.remove(&session_id);
        }

        let payload = TerminalExitEvent {
            session_id: session_id.clone(),
            exit_code,
        };
        if let Err(error) = app.emit(TERMINAL_EXIT_EVENT, payload) {
            warn!(
                session_id = session_id.as_str(),
                error = %error,
                "[Terminal] emit exit failed"
            );
        }

        info!(
            session_id = session_id.as_str(),
            exit_code = exit_code.unwrap_or_default(),
            "[Terminal] session exited"
        );
    });
}

fn default_terminal_profile_fallback() -> String {
    if cfg!(windows) {
        "powershell".to_string()
    } else {
        "bash".to_string()
    }
}

#[command]
pub fn terminal_detect_profiles() -> Result<TerminalProfilesResponse, String> {
    let config = get_terminal_config();
    let profiles = detect_profiles(&config);
    let has_default = profiles
        .iter()
        .any(|profile| profile.id == config.default_profile && profile.available);

    let default_profile = if has_default {
        config.default_profile
    } else {
        profiles
            .iter()
            .find(|profile| profile.available)
            .map(|profile| profile.id.clone())
            .unwrap_or_else(default_terminal_profile_fallback)
    };

    info!(
        default_profile = default_profile.as_str(),
        profile_count = profiles.len(),
        "[Terminal] detected profiles"
    );

    Ok(TerminalProfilesResponse {
        default_profile,
        profiles,
    })
}

#[command]
pub fn terminal_list_sessions() -> Result<Vec<TerminalSessionSnapshot>, String> {
    let sessions = session_store()
        .lock()
        .map_err(|e| format!("lock terminal sessions failed: {e}"))?;
    Ok(sessions.values().map(|session| session.snapshot.clone()).collect())
}

#[command]
pub fn terminal_create_session(
    app: AppHandle,
    request: TerminalCreateRequest,
) -> Result<TerminalSessionSnapshot, String> {
    let config = get_terminal_config();
    let requested_profile_id = if request.profile_id.trim().is_empty() {
        config.default_profile.clone()
    } else {
        request.profile_id.trim().to_string()
    };
    let (profile, launch) = resolve_profile_launch(&config, &requested_profile_id)?;
    let working_dir = choose_working_dir(&request.cwd)?;
    let cols = request.cols.unwrap_or(120).max(20);
    let rows = request.rows.unwrap_or(32).max(10);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("open pty failed: {e}"))?;

    let mut builder = CommandBuilder::new(launch.program.clone());
    for arg in &launch.args {
        builder.arg(arg);
    }
    // 终端统一显式声明 ANSI 颜色能力，避免 IPython 在 PTY 中退回到无高亮模式。
    builder.env("TERM", "xterm-256color");
    builder.env("COLORTERM", "truecolor");
    builder.cwd(working_dir.clone());

    info!(
        profile_id = profile.id.as_str(),
        program = launch.program.as_str(),
        args = ?launch.args,
        cwd = working_dir.to_string_lossy().as_ref(),
        source = launch.source.as_str(),
        "[Terminal] creating session"
    );

    let child = pair
        .slave
        .spawn_command(builder)
        .map_err(|e| format!("spawn terminal failed: {e}"))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone terminal reader failed: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take terminal writer failed: {e}"))?;

    let snapshot = TerminalSessionSnapshot {
        session_id: Uuid::new_v4().to_string(),
        profile_id: profile.id.clone(),
        label: profile.label.clone(),
        cwd: working_dir.to_string_lossy().to_string(),
    };

    let handle = Arc::new(TerminalSessionHandle {
        snapshot: snapshot.clone(),
        master: Mutex::new(pair.master),
        writer: Mutex::new(writer),
        child: Mutex::new(child),
    });

    session_store()
        .lock()
        .map_err(|e| format!("lock terminal sessions failed: {e}"))?
        .insert(snapshot.session_id.clone(), handle.clone());

    info!(
        session_id = snapshot.session_id.as_str(),
        profile_id = snapshot.profile_id.as_str(),
        cwd = snapshot.cwd.as_str(),
        "[Terminal] session created"
    );

    spawn_terminal_reader(app, snapshot.session_id.clone(), reader, handle);
    Ok(snapshot)
}

#[command]
pub fn terminal_write_session(request: TerminalWriteRequest) -> Result<(), String> {
    let session = {
        let sessions = session_store()
            .lock()
            .map_err(|e| format!("lock terminal sessions failed: {e}"))?;
        sessions
            .get(request.session_id.trim())
            .cloned()
            .ok_or_else(|| "终端会话不存在".to_string())?
    };

    let mut writer = session
        .writer
        .lock()
        .map_err(|e| format!("lock terminal writer failed: {e}"))?;
    writer
        .write_all(request.data.as_bytes())
        .map_err(|e| format!("write terminal input failed: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("flush terminal input failed: {e}"))?;
    Ok(())
}

#[command]
pub fn terminal_resize_session(request: TerminalResizeRequest) -> Result<(), String> {
    let session = {
        let sessions = session_store()
            .lock()
            .map_err(|e| format!("lock terminal sessions failed: {e}"))?;
        sessions
            .get(request.session_id.trim())
            .cloned()
            .ok_or_else(|| "终端会话不存在".to_string())?
    };

    let master = session
        .master
        .lock()
        .map_err(|e| format!("lock terminal master failed: {e}"))?;
    master
        .resize(PtySize {
            rows: request.rows.max(5),
            cols: request.cols.max(20),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("resize terminal failed: {e}"))?;
    Ok(())
}

#[command]
pub fn terminal_close_session(request: TerminalCloseRequest) -> Result<(), String> {
    let session = session_store()
        .lock()
        .map_err(|e| format!("lock terminal sessions failed: {e}"))?
        .remove(request.session_id.trim())
        .ok_or_else(|| "终端会话不存在".to_string())?;

    if let Ok(mut child) = session.child.lock() {
        let _ = child.kill();
    }

    info!(
        session_id = session.snapshot.session_id.as_str(),
        profile_id = session.snapshot.profile_id.as_str(),
        "[Terminal] session closed by command"
    );
    Ok(())
}
