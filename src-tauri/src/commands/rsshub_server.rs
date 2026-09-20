//! RSSHub sidecar 进程管理（进程内集成，不用 Docker）。
//!
//! RSSHub 是 Node 应用，构建期由 `tools/prepare_rsshub_env.mjs` 装进
//! `utils/SlidevEnv/node_modules/rsshub`，**复用 SlidevEnv 的 node.exe** 启动，
//! 不额外打包 Node 运行时。本模块负责：
//! - 用 SlidevEnv/node.exe 启动 `node_modules/rsshub/dist/index.mjs`（监听 127.0.0.1:1200）
//! - 健康检查（轮询根路由）
//! - 进程随应用退出被杀（register_managed_pid + terminate_child）
//!
//! RSSHub 缺失（未构建进包）时优雅降级：`ensure_started` 返回 Err，调用方据此
//! 不启用 RSSHub 源，其余抓取不受影响。

use reqwest::Client;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

use crate::guaxiservices::bundled_resources;

/// RSSHub 默认端口（其自身默认 1200）。仅监听 loopback、无鉴权——与 agent_server(9876) 同等风险面。
pub const RSSHUB_PORT: u16 = 1200;
pub const RSSHUB_BASE_URL: &str = "http://127.0.0.1:1200";
const STARTUP_TIMEOUT_SECS: u64 = 40;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
}
#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window(_cmd: &mut Command) {}

static RSSHUB_PROCESS: std::sync::LazyLock<Mutex<Option<Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn slidev_env_root() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/SlidevEnv")
}

/// SlidevEnv 里共用的 node 可执行文件。
fn node_binary() -> Result<PathBuf, String> {
    let root = slidev_env_root()?;
    #[cfg(windows)]
    let node = root.join("node.exe");
    #[cfg(not(windows))]
    let node = root.join("node");
    if node.is_file() {
        Ok(node)
    } else {
        Err(format!("SlidevEnv node 运行时缺失: {}", node.to_string_lossy()))
    }
}

/// RSSHub 源码/构建目录（独立 utils 顶层项，由 prepare_rsshub_env.mjs 克隆+构建）。
fn rsshub_dir() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/rsshub")
}

/// RSSHub 现代构建产物入口（git 源码 `pnpm build` 产出 `dist/index.mjs`，
/// start = `node dist/index.mjs`）。找不到则视为未构建（降级）。
fn rsshub_entry() -> Result<PathBuf, String> {
    let entry = rsshub_dir()?.join("dist").join("index.mjs");
    if entry.is_file() {
        Ok(entry)
    } else {
        Err(format!(
            "RSSHub 未构建（未找到 dist/index.mjs）: {}。请先运行 `npm run rsshub:prepare`。聚合功能将不启用 RSSHub 源。",
            entry.to_string_lossy()
        ))
    }
}

fn rsshub_log_path() -> PathBuf {
    std::env::temp_dir().join("guaxi-rsshub.log")
}

/// RSSHub 是否已就绪（是否可用）。缺文件即视为不可用（降级）。
pub fn is_available() -> bool {
    rsshub_entry().is_ok() && node_binary().is_ok()
}

/// 启动（或复用）RSSHub sidecar。已在运行则直接返回。
pub async fn ensure_started() -> Result<(), String> {
    let node = node_binary()?;
    let entry = rsshub_entry()?;
    let work_dir = rsshub_dir()?;

    if check_health().await.is_ok() {
        info!("[rsshub] already running on {}", RSSHUB_BASE_URL);
        return Ok(());
    }

    info!(
        node = %node.to_string_lossy(),
        entry = %entry.to_string_lossy(),
        "[rsshub] starting RSSHub sidecar"
    );

    let log_path = rsshub_log_path();
    let log_stdout = std::fs::File::create(&log_path)
        .map_err(|e| format!("创建 RSSHub 日志文件失败: {}", e))?;
    let log_stderr = log_stdout
        .try_clone()
        .map_err(|e| format!("克隆 RSSHub 日志句柄失败: {}", e))?;

    // NODE_PATH 指向 RSSHub 自己的 node_modules，确保其运行时依赖能被解析。
    // cwd 也设为 rsshub 目录（其构建产物按此定位内部资源）。
    let node_modules = work_dir.join("node_modules");

    let mut cmd = Command::new(&node);
    cmd.arg(&entry)
        .env("NODE_ENV", "production")
        .env("PORT", RSSHUB_PORT.to_string())
        .env("NODE_PATH", &node_modules)
        // RSSHub 官方 start 脚本用的 http header 上限，沿用。
        .env("NODE_OPTIONS", "--max-http-header-size=32768")
        .current_dir(&work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_stdout))
        .stderr(Stdio::from(log_stderr));
    apply_no_window(&mut cmd);

    let child = cmd.spawn().map_err(|e| format!("启动 RSSHub 失败: {}", e))?;

    crate::guaxiservices::process_shutdown::register_managed_pid("rsshub(node)", child.id());

    {
        let mut guard = RSSHUB_PROCESS.lock().map_err(|_| "lock rsshub process failed".to_string())?;
        if let Some(mut old) = guard.take() {
            let _ = old.kill();
        }
        *guard = Some(child);
    }

    let client = Client::new();
    for _ in 0..STARTUP_TIMEOUT_SECS {
        // 子进程若已退出，读日志尾部回传报错，不空等满超时。
        if let Ok(mut guard) = RSSHUB_PROCESS.lock() {
            if let Some(child) = guard.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let tail = read_log_tail(&log_path, 40);
                    return Err(format!(
                        "RSSHub 进程已退出（{}）。日志: {}\n{}",
                        status,
                        log_path.to_string_lossy(),
                        tail
                    ));
                }
            }
        }
        if let Ok(resp) = client
            .get(RSSHUB_BASE_URL)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() || resp.status().is_redirection() {
                info!("[rsshub] ready on {}", RSSHUB_BASE_URL);
                return Ok(());
            }
        }
        sleep(Duration::from_secs(1)).await;
    }

    let tail = read_log_tail(&log_path, 40);
    Err(format!(
        "RSSHub 在 {} 秒内未就绪。日志: {}\n{}",
        STARTUP_TIMEOUT_SECS,
        log_path.to_string_lossy(),
        tail
    ))
}

/// 停止 RSSHub sidecar（进程树终止，避免残留 node 子进程）。
pub fn stop() {
    if let Ok(mut guard) = RSSHUB_PROCESS.lock() {
        if let Some(mut child) = guard.take() {
            info!("[rsshub] stopping RSSHub sidecar");
            crate::guaxiservices::process_shutdown::terminate_child("rsshub(node)", &mut child);
        }
    }
}

async fn check_health() -> Result<(), String> {
    let client = Client::new();
    let resp = client
        .get(RSSHUB_BASE_URL)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map_err(|e| format!("rsshub 不可达: {}", e))?;
    if resp.status().is_success() || resp.status().is_redirection() {
        Ok(())
    } else {
        Err(format!("rsshub 返回错误: {}", resp.status()))
    }
}

fn read_log_tail(path: &std::path::Path, max_lines: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(max_lines);
            lines[start..].join("\n")
        }
        Err(_) => String::new(),
    }
}
