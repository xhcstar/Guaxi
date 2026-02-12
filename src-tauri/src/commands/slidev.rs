use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

use serde::Serialize;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize)]
pub struct SlidevDevInfo {
    pub entry_path: String,
    pub port: u16,
    pub url: String,
}

#[derive(Debug)]
struct SlidevDevRuntime {
    info: SlidevDevInfo,
    child: Child,
}

static SLIDEV_DEV: OnceLock<Mutex<Option<SlidevDevRuntime>>> = OnceLock::new();

fn dev_lock() -> &'static Mutex<Option<SlidevDevRuntime>> {
    SLIDEV_DEV.get_or_init(|| Mutex::new(None))
}

fn normalize_path_for_fs(input: &str) -> PathBuf {
    let s = input.trim().replace('\\', "/");
    PathBuf::from(s)
}

#[derive(Debug, Clone, Serialize)]
pub struct SlidevEnvStatus {
    pub installed: bool,
    pub expected_root: String,
    pub node_path: Option<String>,
    pub cli_path: Option<String>,
}

fn exe_dir() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("无法获取 exe 路径: {e}"))?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "无法获取 exe 所在目录".to_string())?
        .to_path_buf();
    Ok(dir)
}

fn expected_slidev_env_root() -> Result<PathBuf, String> {
    Ok(exe_dir()?.join("utils").join("SlidevEnv"))
}

fn resolve_slidev_runtime() -> Result<(PathBuf, PathBuf, SlidevEnvStatus), String> {
    let root = expected_slidev_env_root()?;

    #[cfg(windows)]
    let node = root.join("node.exe");
    #[cfg(not(windows))]
    let node = root.join("node");

    #[cfg(windows)]
    let cli = root
        .join("node_modules")
        .join(".bin")
        .join("slidev.cmd");

    #[cfg(not(windows))]
    let cli = root
        .join("node_modules")
        .join(".bin")
        .join("slidev");

    let status = SlidevEnvStatus {
        installed: node.is_file() && cli.is_file(),
        expected_root: root.to_string_lossy().to_string(),
        node_path: if node.is_file() {
            Some(node.to_string_lossy().to_string())
        } else {
            None
        },
        cli_path: if cli.is_file() {
            Some(cli.to_string_lossy().to_string())
        } else {
            None
        },
    };

    if !status.installed {
        return Err(format!(
            "未检测到内置 Slidev 运行环境。\n\n期望目录：{}\n期望文件：{} 与 {}\n\n说明：为了避免依赖用户机器是否安装 Node/Slidev，本功能只使用软件内置的 SlidevEnv。请将对应平台的 SlidevEnv 放到上述目录后重试。",
            status.expected_root,
            node.to_string_lossy(),
            cli.to_string_lossy(),
        ));
    }

    Ok((node, cli, status))
}

fn inject_slidev_env_path(cmd: &mut Command, env_root: &Path) {
    let mut paths: Vec<PathBuf> = vec![
        env_root.to_path_buf(),
        env_root.join("node_modules").join(".bin"),
    ];

    if let Some(old) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&old));
    }

    match std::env::join_paths(paths) {
        Ok(joined) => {
            cmd.env("PATH", joined);
        }
        Err(e) => {
            warn!("[Slidev] join_paths failed: {}", e);
        }
    }
}


#[tauri::command]
pub fn slidev_get_env_status() -> Result<SlidevEnvStatus, String> {
    let root = expected_slidev_env_root()?;

    #[cfg(windows)]
    let node = root.join("node.exe");
    #[cfg(not(windows))]
    let node = root.join("node");

    let cli = root
        .join("node_modules")
        .join(".bin")
        .join("slidev");

    Ok(SlidevEnvStatus {
        installed: node.is_file() && cli.is_file(),
        expected_root: root.to_string_lossy().to_string(),
        node_path: if node.is_file() {
            Some(node.to_string_lossy().to_string())
        } else {
            None
        },
        cli_path: if cli.is_file() {
            Some(cli.to_string_lossy().to_string())
        } else {
            None
        },
    })
}

fn entry_workdir(entry: &Path) -> Result<PathBuf, String> {
    let parent = entry
        .parent()
        .ok_or_else(|| "无法解析 entry 的父目录".to_string())?;
    Ok(parent.to_path_buf())
}

fn kill_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {}
        Err(e) => {
            warn!("[Slidev] try_wait failed: {}", e);
        }
    }

    if let Err(e) = child.kill() {
        warn!("[Slidev] kill failed: {}", e);
        return;
    }

    let _ = child.wait();
}

#[tauri::command]
pub fn slidev_start_dev(entry_path: String, port: Option<u16>) -> Result<SlidevDevInfo, String> {
    let entry_path_norm = entry_path.trim().to_string();
    if entry_path_norm.is_empty() {
        return Err("entry_path 不能为空".to_string());
    }

    let entry_fs = normalize_path_for_fs(&entry_path_norm);
    if !entry_fs.exists() {
        return Err(format!("entry 不存在: {}", entry_path_norm));
    }
    if entry_fs.is_dir() {
        return Err("entry_path 不能是目录，请选择具体的 slides.md".to_string());
    }

    let port = port.unwrap_or(3030);
    let url = format!("http://127.0.0.1:{}/", port);

    let workdir = entry_workdir(&entry_fs)?;
    info!(
        "[Slidev] start dev entry={} port={} workdir={}",
        entry_path_norm,
        port,
        workdir.display()
    );

    let mut guard = dev_lock()
        .lock()
        .map_err(|_| "Slidev dev 锁异常".to_string())?;

    if let Some(rt) = guard.as_mut() {
        let same = rt.info.entry_path == entry_path_norm && rt.info.port == port;

        match rt.child.try_wait() {
            Ok(None) => {
                if same {
                    info!("[Slidev] dev already running url={}", rt.info.url);
                    return Ok(rt.info.clone());
                }

                info!(
                    "[Slidev] dev running but entry/port changed, stopping old: old_entry={} old_port={}",
                    rt.info.entry_path, rt.info.port
                );
                kill_child(&mut rt.child);
                *guard = None;
            }
            Ok(Some(status)) => {
                warn!("[Slidev] dev exited status={}", status);
                *guard = None;
            }
            Err(e) => {
                warn!("[Slidev] dev try_wait failed: {}", e);
                kill_child(&mut rt.child);
                *guard = None;
            }
        }
    }

    let (_node_path, cli_path, env_status) = resolve_slidev_runtime()?;
    let env_root = PathBuf::from(&env_status.expected_root);

    info!(
        "[Slidev] using bundled slidev cli: root={} cli={}",
        env_status.expected_root,
        cli_path.display()
    );

    let mut cmd = Command::new(&cli_path);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    inject_slidev_env_path(&mut cmd, &env_root);

    cmd.arg(&entry_path_norm)
        .arg("--port")
        .arg(port.to_string())
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--open")
        .arg("false")
        .current_dir(&env_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().map_err(|e| {
        error!("[Slidev] spawn failed: {}", e);
        format!("启动 Slidev 失败: {}", e)
    })?;

    let info = SlidevDevInfo {
        entry_path: entry_path_norm,
        port,
        url,
    };

    *guard = Some(SlidevDevRuntime {
        info: info.clone(),
        child,
    });

    Ok(info)
}

#[tauri::command]
pub fn slidev_stop_dev() -> Result<(), String> {
    let mut guard = dev_lock()
        .lock()
        .map_err(|_| "Slidev dev 锁异常".to_string())?;
    let Some(mut rt) = guard.take() else {
        info!("[Slidev] stop dev skipped: not running");
        return Ok(());
    };

    info!(
        "[Slidev] stop dev entry={} port={}",
        rt.info.entry_path, rt.info.port
    );
    kill_child(&mut rt.child);
    Ok(())
}

#[tauri::command]
pub fn slidev_export_pptx(entry_path: String, output_path: String) -> Result<(), String> {
    let entry_path_norm = entry_path.trim().to_string();
    if entry_path_norm.is_empty() {
        return Err("entry_path 不能为空".to_string());
    }
    let output_path_norm = output_path.trim().to_string();
    if output_path_norm.is_empty() {
        return Err("output_path 不能为空".to_string());
    }

    let entry_fs = normalize_path_for_fs(&entry_path_norm);
    if !entry_fs.exists() {
        return Err(format!("entry 不存在: {}", entry_path_norm));
    }
    if entry_fs.is_dir() {
        return Err("entry_path 不能是目录，请选择具体的 slides.md".to_string());
    }

    let workdir = entry_workdir(&entry_fs)?;
    info!(
        "[Slidev] export pptx entry={} output={} workdir={}",
        entry_path_norm,
        output_path_norm,
        workdir.display()
    );

    let (_node_path, cli_path, env_status) = resolve_slidev_runtime()?;
    let env_root = PathBuf::from(&env_status.expected_root);

    info!(
        "[Slidev] using bundled slidev cli: root={} cli={}",
        env_status.expected_root,
        cli_path.display()
    );

    let mut cmd = Command::new(&cli_path);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    inject_slidev_env_path(&mut cmd, &env_root);

    cmd.arg("export")
        .arg(&entry_path_norm)
        .arg("--format")
        .arg("pptx")
        .arg("--output")
        .arg(&output_path_norm)
        .current_dir(&env_root);

    let out = cmd
        .output()
        .map_err(|e| format!("执行 Slidev export 失败: {}", e))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let msg = format!(
            "Slidev 导出失败(status={}):\n{}\n{}",
            out.status, stdout, stderr
        );
        error!("[Slidev] export failed: {}", msg);
        return Err(msg);
    }

    info!("[Slidev] export success output={}", output_path_norm);
    Ok(())
}
