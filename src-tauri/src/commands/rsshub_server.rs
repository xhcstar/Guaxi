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
const STARTUP_TIMEOUT_SECS: u64 = 90;

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

/// 前端命令：重启 RSSHub 以读取最新登录态 cookie（用户登录知乎/小红书后调用）。
/// RSSHub 未构建时返回明确错误，供前端提示。
#[tauri::command]
pub async fn rsshub_refresh_cookies() -> Result<(), String> {
    if !is_available() {
        return Err("RSSHub 未构建，无法刷新登录态（其余抓取不受影响）".to_string());
    }
    restart().await
}

/// 前端命令：RSSHub 本地服务基址（供前端拼接路由 URL、展示提示）。
#[tauri::command]
pub fn rsshub_base_url() -> String {
    RSSHUB_BASE_URL.to_string()
}

/// 前端命令：Radar 探测——给定网页 URL，返回该域名下 RSSHub 能生成的路由规则。
/// 这正是 RSSHub 浏览器插件"把任意网页做成订阅"的能力：代理到本地 sidecar 的
/// `/api/radar/rules/{domain}`，避免前端直连 loopback 端口受 CSP 限制。
///
/// 返回 RSSHub 原始 JSON（按子域名分组的规则数组），前端负责匹配路径与展示。
/// RSSHub 未启动/未构建时返回明确错误。
#[tauri::command]
pub async fn rsshub_radar_detect(page_url: String) -> Result<serde_json::Value, String> {
    if !is_available() {
        return Err("RSSHub 未构建（其余抓取不受影响）".to_string());
    }
    // 确保 sidecar 在跑。
    ensure_started().await?;

    // 从网页 URL 解析主域名（radar 规则按 registrable domain 分组）。
    let host = url::Url::parse(page_url.trim())
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .ok_or_else(|| "无法从该网址解析出域名".to_string())?;
    // 取主域名（去掉 www. 等常见子域前缀；radar key 是 registrable domain）。
    let domain = registrable_domain(&host);

    let client = Client::new();
    let url = format!("{}/api/radar/rules/{}", RSSHUB_BASE_URL, domain);
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("radar 请求失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("radar 返回错误: {}", resp.status()));
    }
    let value: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("radar 响应解析失败: {}", e))?;
    Ok(value)
}

/// 取可注册主域名（简化：保留最后两段，如 www.bilibili.com → bilibili.com）。
/// 对多数站点够用；少数二级后缀(.com.cn 等)可能不精确，但 radar 侧对完整域名也能命中，
/// 这里主要为了对齐 radar 规则的分组 key。
fn registrable_domain(host: &str) -> String {
    let parts: Vec<&str> = host.trim_end_matches('.').split('.').collect();
    let n = parts.len();
    if n <= 2 {
        return host.to_string();
    }
    // 常见二级后缀：com.cn / net.cn / org.cn / gov.cn / co.uk / com.hk ...
    let last2 = format!("{}.{}", parts[n - 2], parts[n - 1]);
    const SECOND_LEVEL: [&str; 8] = [
        "com.cn", "net.cn", "org.cn", "gov.cn", "edu.cn", "co.uk", "com.hk", "com.tw",
    ];
    if SECOND_LEVEL.contains(&last2.as_str()) && n >= 3 {
        format!("{}.{}.{}", parts[n - 3], parts[n - 2], parts[n - 1])
    } else {
        last2
    }
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

    // 登录墙站点 cookie：从用户已登录的持久化 Chrome profile 导出，作为环境变量传给 RSSHub。
    // RSSHub 在启动时读取这些 env（config.ts），故登录态变化需重启 sidecar 才生效（见 restart）。
    let login_cookies = collect_login_cookies().await;

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
    for (key, val) in &login_cookies {
        cmd.env(key, val);
    }
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

/// 收集登录墙站点的 cookie，映射到 RSSHub 识别的环境变量名。
///
/// 复用阶段 B 的 `flipboard_render::cookie_header_for_url`：从用户手动登录后落盘的
/// 持久化 Chrome profile（按 host 区分）读出 cookie。用户没登录过的站返回空，跳过。
/// env 名对应 RSSHub config.ts：ZHIHU_COOKIES / XIAOHONGSHU_COOKIE / WEIBO_COOKIES。
async fn collect_login_cookies() -> Vec<(String, String)> {
    // (代表性 URL, profile=host, RSSHub 环境变量名)
    const SITES: [(&str, &str, &str); 3] = [
        ("https://www.zhihu.com", "www.zhihu.com", "ZHIHU_COOKIES"),
        ("https://www.xiaohongshu.com", "www.xiaohongshu.com", "XIAOHONGSHU_COOKIE"),
        ("https://weibo.com", "weibo.com", "WEIBO_COOKIES"),
    ];
    let mut out = Vec::new();
    for (url, profile, env_key) in SITES {
        if let Some(cookie) =
            crate::commands::flipboard_render::cookie_header_for_url(url, profile).await
        {
            if !cookie.trim().is_empty() {
                info!("[rsshub] injecting login cookie for {}", env_key);
                out.push((env_key.to_string(), cookie));
            }
        }
    }
    out
}

/// 重启 RSSHub sidecar：停掉现有实例后重新启动，以读取最新登录态 cookie。
/// 用户在阅读页对知乎/小红书完成登录后调用，让带 cookie 的路由立即生效。
pub async fn restart() -> Result<(), String> {
    stop();
    // 等端口释放，避免新实例 bind 冲突。
    sleep(Duration::from_secs(1)).await;
    ensure_started().await
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
