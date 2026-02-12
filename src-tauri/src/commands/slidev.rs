use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

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
    workdir_node_modules_created: bool,
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
    let cli = root.join("node_modules").join(".bin").join("slidev.cmd");

    #[cfg(not(windows))]
    let cli = root.join("node_modules").join(".bin").join("slidev");

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

    let theme_default_pkg = root
        .join("node_modules")
        .join("@slidev")
        .join("theme-default")
        .join("package.json");
    if !theme_default_pkg.is_file() {
        return Err(format!(
            "内置 SlidevEnv 缺少默认主题依赖：@slidev/theme-default。\n\n当前 Slidev CLI 会在缺少主题时尝试交互式安装，但桌面应用场景无法弹窗确认，进程将直接退出。\n\n请在下列目录执行安装后重试：\n  {}\n\n示例命令（需要本机可用的 npm 与网络）：\n  cd /d {}\n  npm i @slidev/theme-default@^0.25.0\n\n或在构建阶段将该依赖预先打包进 SlidevEnv/node_modules。",
            status.expected_root,
            status.expected_root,
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

fn ensure_workdir_node_modules(workdir: &Path, env_root: &Path) -> Result<bool, String> {
    let dst = workdir.join("node_modules");
    if dst.exists() {
        if dst.is_dir() {
            return Ok(false);
        }
        return Err(format!(
            "SlideRepo/node_modules 已存在但不是目录：{}",
            dst.to_string_lossy()
        ));
    }

    let src = env_root.join("node_modules");
    if !src.is_dir() {
        return Err(format!(
            "内置 SlidevEnv/node_modules 不存在：{}",
            src.to_string_lossy()
        ));
    }

    #[cfg(windows)]
    {
        let dst_s = dst.to_string_lossy().replace('/', "\\");
        let src_s = src.to_string_lossy().replace('/', "\\");

        info!(
            "[Slidev] creating node_modules junction: dst={} src={} (powershell)",
            dst_s, src_s
        );

        let dst_ps = dst_s.replace('"', "`\"");
        let src_ps = src_s.replace('"', "`\"");
        let script = format!(
            "[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding; New-Item -ItemType Junction -Path \"{}\" -Target \"{}\" -ErrorAction Stop | Out-Null",
            dst_ps, src_ps
        );

        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .output()
            .map_err(|e| format!("创建 node_modules 联接失败: {e}"))?;

        if !out.status.success() {
            let so = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let se = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(format!(
                "创建 node_modules 联接失败: status={} stdout={} stderr={}",
                out.status, so, se
            ));
        }

        info!("[Slidev] node_modules junction created: {} -> {}", dst_s, src_s);
        return Ok(true);
    }

    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(&src, &dst)
            .map_err(|e| format!("创建 node_modules symlink 失败: {e}"))?;
        info!(
            "[Slidev] node_modules symlink created: {} -> {}",
            dst.to_string_lossy(),
            src.to_string_lossy()
        );
        Ok(true)
    }
}

fn cleanup_workdir_node_modules_if_created(entry_path: &str, created_by_app: bool) {
    if !created_by_app {
        return;
    }

    let entry_fs = normalize_path_for_fs(entry_path);
    let Ok(workdir) = entry_workdir(&entry_fs) else {
        warn!("[Slidev] cleanup node_modules skipped: cannot resolve workdir, entry={}", entry_path);
        return;
    };

    let dst = workdir.join("node_modules");
    if !dst.exists() {
        info!(
            "[Slidev] cleanup node_modules skipped: already gone, path={}",
            dst.to_string_lossy()
        );
        return;
    }

    #[cfg(windows)]
    {
        let dst_s = dst.to_string_lossy().replace('/', "\\");
        info!("[Slidev] cleanup node_modules junction: {}", dst_s);

        let cmdline = format!("rmdir /S /Q \"{}\"", dst_s);
        let out = Command::new("cmd").args(["/C", &cmdline]).output();

        match out {
            Ok(out) if out.status.success() => {
                info!("[Slidev] cleanup node_modules done: {}", dst_s);
            }
            Ok(out) => {
                let so = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let se = String::from_utf8_lossy(&out.stderr).trim().to_string();
                warn!(
                    "[Slidev] cleanup node_modules failed: status={} stdout={} stderr={}",
                    out.status, so, se
                );
            }
            Err(e) => {
                warn!("[Slidev] cleanup node_modules failed to spawn cmd: {}", e);
            }
        }

        return;
    }

    #[cfg(not(windows))]
    {
        info!(
            "[Slidev] cleanup node_modules symlink/dir: {}",
            dst.to_string_lossy()
        );
        if let Err(e) = std::fs::remove_dir_all(&dst) {
            warn!("[Slidev] cleanup node_modules failed: {}", e);
        } else {
            info!("[Slidev] cleanup node_modules done: {}", dst.to_string_lossy());
        }
    }
}

fn spawn_slidev_pipe_logger(reader: impl Read + Send + 'static, port: u16, kind: &'static str) {
    thread::spawn(move || {
        let mut br = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match br.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let s = line.trim_end_matches(['\r', '\n']);
                    if !s.is_empty() {
                        info!("[Slidev][dev:{}][{}] {}", port, kind, s);
                    }
                }
                Err(e) => {
                    warn!("[Slidev][dev:{}][{}] read failed: {}", port, kind, e);
                    break;
                }
            }
        }
        info!("[Slidev][dev:{}][{}] pipe closed", port, kind);
    });
}

fn probe_http_root(port: u16) -> Result<String, String> {
    let addrs: Vec<SocketAddr> = ("localhost", port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve localhost failed: {e}"))?
        .collect();

    if addrs.is_empty() {
        return Err("resolve localhost returned empty".to_string());
    }

    let mut last_err = String::new();

    // Slidev(Vite) 在 Windows 上可能绑定到 IPv6 ::1；仅探测 127.0.0.1 会误判“未启动”。
    for addr in addrs {
        match probe_http_root_addr(addr, port) {
            Ok(line) => return Ok(line),
            Err(e) => {
                last_err = format!("addr={addr} {e}");
            }
        }
    }

    Err(last_err)
}

fn probe_http_root_addr(addr: SocketAddr, port: u16) -> Result<String, String> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(450))
        .map_err(|e| format!("connect_timeout failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_millis(450)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_millis(450)))
        .ok();

    // Host 头用 localhost，避免某些中间件对 IP Host 做限制。
    let req = format!("GET / HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;

    let mut buf = [0u8; 512];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    if n == 0 {
        return Err("empty response".to_string());
    }

    let head = String::from_utf8_lossy(&buf[..n]).to_string();
    let first_line = head.lines().next().unwrap_or("").trim().to_string();
    if first_line.is_empty() {
        return Err(format!("invalid response head: {} bytes", n));
    }
    Ok(first_line)
}

fn wait_slidev_ready(port: u16, child: &mut Child, timeout: Duration) -> Result<String, String> {
    let start = Instant::now();
    let mut attempts: u32 = 0;
    let mut last_log_at = Instant::now();
    let mut last_err = String::new();

    loop {
        attempts += 1;

        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(format!("slidev dev exited early: status={status}"));
            }
            Ok(None) => {}
            Err(e) => {
                last_err.clear();
                last_err.push_str(&format!("try_wait failed: {e}"));
            }
        }

        match probe_http_root(port) {
            Ok(status_line) => return Ok(status_line),
            Err(e) => {
                last_err.clear();
                last_err.push_str(&e);
            }
        }

        if start.elapsed() >= timeout {
            return Err(format!(
                "wait ready timeout: port={port} attempts={attempts} last_err={}",
                if last_err.is_empty() {
                    "unknown"
                } else {
                    last_err.as_str()
                }
            ));
        }

        if last_log_at.elapsed() >= Duration::from_secs(1) {
            last_log_at = Instant::now();
            info!(
                "[Slidev] waiting dev ready... port={} attempts={} elapsedMs={} lastErr={}",
                port,
                attempts,
                start.elapsed().as_millis(),
                last_err
            );
        }

        thread::sleep(Duration::from_millis(120));
    }
}

#[tauri::command]
pub fn slidev_get_env_status() -> Result<SlidevEnvStatus, String> {
    let root = expected_slidev_env_root()?;

    #[cfg(windows)]
    let node = root.join("node.exe");
    #[cfg(not(windows))]
    let node = root.join("node");

    #[cfg(windows)]
    let cli = root.join("node_modules").join(".bin").join("slidev.cmd");

    #[cfg(not(windows))]
    let cli = root.join("node_modules").join(".bin").join("slidev");

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
    let url = format!("http://localhost:{}/", port);

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
                    "[Slidev] dev running but entry/port changed, stopping old: old_entry={} old_port={} ",
                    rt.info.entry_path, rt.info.port
                );
                kill_child(&mut rt.child);
                cleanup_workdir_node_modules_if_created(
                    &rt.info.entry_path,
                    rt.workdir_node_modules_created,
                );
                *guard = None;
            }
            Ok(Some(status)) => {
                warn!("[Slidev] dev exited status={}", status);
                *guard = None;
            }
            Err(e) => {
            warn!("[Slidev] dev try_wait failed: {}", e);
            kill_child(&mut rt.child);
            cleanup_workdir_node_modules_if_created(
                &rt.info.entry_path,
                rt.workdir_node_modules_created,
            );
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

    let node_modules_created = ensure_workdir_node_modules(&workdir, &env_root)?;

    cmd.arg(&entry_path_norm)
        .arg("--port")
        .arg(port.to_string())
        .arg("--log")
        .arg("info")
        .arg("--theme")
        .arg("@slidev/theme-default")
        .current_dir(&workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let program = cmd.get_program().to_string_lossy().to_string();
    let args: Vec<String> = cmd
        .get_args()
        .map(|v| v.to_string_lossy().to_string())
        .collect();
    let cmd_cwd = cmd
        .get_current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    info!(
        "[Slidev] spawn dev program={} cmdCwd={} entryWorkdir={} port={} entry={} args={:?}",
        program,
        cmd_cwd,
        workdir.display(),
        port,
        entry_path_norm,
        args
    );

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            error!("[Slidev] spawn failed: {}", e);
            cleanup_workdir_node_modules_if_created(&entry_path_norm, node_modules_created);
            return Err(format!("启动 Slidev 失败: {}", e));
        }
    };

    let pid = child.id();
    info!("[Slidev] dev spawned pid={} port={} url={}", pid, port, url);

    if let Some(stdout) = child.stdout.take() {
        spawn_slidev_pipe_logger(stdout, port, "stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_slidev_pipe_logger(stderr, port, "stderr");
    }

    match wait_slidev_ready(port, &mut child, Duration::from_secs(12)) {
        Ok(status_line) => {
            info!(
                "[Slidev] dev ready port={} url={} statusLine={}",
                port, url, status_line
            );
        }
        Err(e) => {
            warn!("[Slidev] dev not ready: {}", e);
            kill_child(&mut child);
            cleanup_workdir_node_modules_if_created(&entry_path_norm, node_modules_created);
            return Err(format!(
                "Slidev 已启动但未就绪：{}\n\n请查看 logs/logfile.log 获取 Slidev 输出与更多上下文。",
                e
            ));
        }
    }

    let info = SlidevDevInfo {
        entry_path: entry_path_norm,
        port,
        url,
    };

    *guard = Some(SlidevDevRuntime {
        info: info.clone(),
        child,
        workdir_node_modules_created: node_modules_created,
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
    cleanup_workdir_node_modules_if_created(&rt.info.entry_path, rt.workdir_node_modules_created);
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
