//! 工业级渲染层（chromiumoxide / 无头 Chrome）。
//!
//! 职责：对 JS 重度渲染或登录墙站点，用真无头 Chrome 拿到 JS 执行后的完整 DOM。
//! - HTTP 优先：`reqwest` 抓到的 body 足够厚就直接用，省去启动 Chrome 的开销。
//! - CDP 升级：body 过薄/空壳/已知 JS 站 → 启动无头 Chrome，等网络空闲 + 自动滚动
//!   触发懒加载，再取 `page.content()`。
//! - Chrome 自举：首次使用时 `BrowserFetcher` 自动下载匹配版 Chromium 到应用目录。
//! - 反检测：每页注入 stealth 脚本（隐藏 navigator.webdriver 等）。
//! - 会话持久化：用固定 user_data_dir（按 profile 区分），登录一次后复用 cookie。
//!
//! 并发：用 `Semaphore` 限制同时打开的标签页数量（WebView2/Chrome 每页都吃内存）。

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell, Semaphore};
use tracing::{info, warn};

/// 渲染结果。`mode`: "http" | "chrome"。
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub html: String,
    /// 跟随重定向后的最终 URL（供 P7 重定向规范化使用）。
    #[allow(dead_code)]
    pub final_url: String,
    pub mode: &'static str,
}

/// 同时最多打开的 Chrome 标签页数（Chrome 每页吃内存，保守限制）。
static RENDER_SEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(3));

/// 全局共享的 Chrome 实例（懒启动）。`None` 表示尚未启动。
static BROWSER: OnceCell<Arc<Mutex<Option<BrowserHandle>>>> = OnceCell::const_new();

/// Chrome 熔断器：无头渲染连续启动失败若干次后，本进程内停用 Chrome 升级，
/// 后续抓取干净降级到 HTTP，避免反复崩溃 + 刷屏 WS 错误日志拖慢 UI。
static HEADLESS_DISABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static HEADLESS_FAIL_COUNT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
/// 连续失败达到该阈值即熔断。
const HEADLESS_FAIL_THRESHOLD: u32 = 2;

fn headless_disabled() -> bool {
    HEADLESS_DISABLED.load(std::sync::atomic::Ordering::Relaxed)
}

fn note_headless_failure() {
    let n = HEADLESS_FAIL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if n >= HEADLESS_FAIL_THRESHOLD {
        HEADLESS_DISABLED.store(true, std::sync::atomic::Ordering::Relaxed);
        warn!(
            "flipboard_render: headless Chrome disabled after {} failures; falling back to HTTP-only for this session",
            n
        );
    }
}

fn note_headless_success() {
    HEADLESS_FAIL_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
}

struct BrowserHandle {
    browser: Browser,
    /// 驱动 CDP 事件循环的后台任务句柄。
    _handler_task: tokio::task::JoinHandle<()>,
    profile: String,
}

/// stealth 脚本：抹掉常见的自动化指纹。业界 puppeteer-extra-stealth 的精简版。
const STEALTH_SCRIPT: &str = r#"
Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
Object.defineProperty(navigator, 'languages', { get: () => ['zh-CN', 'zh', 'en'] });
Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
window.chrome = { runtime: {} };
const originalQuery = window.navigator.permissions && window.navigator.permissions.query;
if (originalQuery) {
  window.navigator.permissions.query = (parameters) => (
    parameters.name === 'notifications'
      ? Promise.resolve({ state: Notification.permission })
      : originalQuery(parameters)
  );
}
"#;

/// 已知 JS 重度渲染 / 登录墙站点的 host 关键字。
fn host_needs_render(url: &str) -> bool {
    let u = url.to_lowercase();
    u.contains("zhihu.com")
        || u.contains("xiaohongshu.com")
        || u.contains("bilibili.com")
        || u.contains("mp.weixin.qq.com")
        || u.contains("weibo.com")
        || u.contains("twitter.com")
        || u.contains("x.com")
}

/// 判断 HTTP 抓到的 body 是否"太薄"（疑似 JS 空壳），需要升级渲染。
fn body_is_thin(html: &str) -> bool {
    // 去标签后可见文本长度。
    let text_len = html
        .split('<')
        .filter_map(|seg| seg.split_once('>').map(|(_, rest)| rest))
        .map(|t| t.trim().chars().filter(|c| !c.is_whitespace()).count())
        .sum::<usize>();
    if text_len < 400 {
        return true;
    }
    // 常见 SPA 空壳标记。
    let lower = html.to_lowercase();
    (lower.contains("id=\"root\"") || lower.contains("id=\"app\"") || lower.contains("id=\"__next\""))
        && text_len < 1200
        || lower.contains("请开启javascript")
        || lower.contains("enable javascript")
}

/// 解析 Chromium 可执行路径：环境变量 > 自动下载缓存。首次会触发下载。
async fn ensure_chromium_executable() -> Result<PathBuf, String> {
    static CHROME_PATH: OnceCell<PathBuf> = OnceCell::const_new();
    CHROME_PATH
        .get_or_try_init(|| async {
            // 1. 环境变量覆盖（用户已装 Chrome/Edge）。
            for var in ["GUAXI_CHROME_PATH", "CHROME_PATH"] {
                if let Ok(p) = std::env::var(var) {
                    let pb = PathBuf::from(p);
                    if pb.exists() {
                        return Ok(pb);
                    }
                }
            }
            // 2. 系统已安装的 Chrome / Edge（比自动下载的 snapshot 与 CDP 协议更稳，
            //    可避免 "WS Invalid message" 协议不匹配与启动崩溃）。
            if let Some(pb) = find_system_chrome() {
                info!("flipboard_render: using system browser at {:?}", pb);
                return Ok(pb);
            }
            // 3. 自动下载到应用数据目录。
            let download_dir = chromium_download_dir()?;
            tokio::fs::create_dir_all(&download_dir)
                .await
                .map_err(|e| format!("create chromium dir failed: {e}"))?;
            info!("flipboard_render: fetching headless Chromium into {:?}", download_dir);
            let fetcher = BrowserFetcher::new(
                BrowserFetcherOptions::builder()
                    .with_path(&download_dir)
                    .build()
                    .map_err(|e| format!("fetcher options: {e}"))?,
            );
            let info = fetcher
                .fetch()
                .await
                .map_err(|e| format!("download chromium failed: {e}"))?;
            info!("flipboard_render: chromium ready at {:?}", info.executable_path);
            Ok(info.executable_path)
        })
        .await
        .cloned()
}

/// 探测系统已安装的 Chrome / Edge（Windows 常见安装路径）。
fn find_system_chrome() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let envs = [
        std::env::var("PROGRAMFILES").ok(),
        std::env::var("PROGRAMFILES(X86)").ok(),
        std::env::var("LOCALAPPDATA").ok(),
    ];
    let rel = [
        r"Google\Chrome\Application\chrome.exe",
        r"Microsoft\Edge\Application\msedge.exe",
    ];
    for base in envs.iter().flatten() {
        for r in rel.iter() {
            candidates.push(PathBuf::from(base).join(r));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

fn chromium_download_dir() -> Result<PathBuf, String> {
    // 放在可执行文件同级的 chromium/ 下，便于卸载清理。
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "exe has no parent".to_string())?
        .join("chromium");
    Ok(dir)
}

fn chrome_profile_dir(profile: &str) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let safe: String = profile
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe = if safe.is_empty() { "default".to_string() } else { safe };
    Ok(exe
        .parent()
        .ok_or_else(|| "exe has no parent".to_string())?
        .join("chrome_profiles")
        .join(safe))
}

/// 启动（或复用）共享 Chrome 实例。`headless=false` 用于登录会话。
async fn launch_browser(profile: &str, headless: bool) -> Result<Browser, String> {
    let exec = ensure_chromium_executable().await?;
    let user_data = chrome_profile_dir(profile)?;
    tokio::fs::create_dir_all(&user_data)
        .await
        .map_err(|e| format!("create profile dir failed: {e}"))?;

    let mut builder = BrowserConfig::builder()
        .chrome_executable(exec)
        .user_data_dir(user_data)
        .window_size(1366, 900)
        .request_timeout(Duration::from_secs(30))
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-dev-shm-usage")
        .arg("--lang=zh-CN")
        .arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");

    if headless {
        builder = builder.new_headless_mode();
    } else {
        builder = builder.with_head();
    }

    let config = builder.no_sandbox().build().map_err(|e| format!("browser config: {e}"))?;
    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("launch chrome failed: {e}"))?;

    // 驱动 CDP 事件循环。
    tokio::spawn(async move {
        while let Some(ev) = handler.next().await {
            if ev.is_err() {
                break;
            }
        }
    });

    Ok(browser)
}

/// 获取一个可用的共享浏览器（headless 抓取用，profile 固定）。
async fn shared_browser(profile: &str) -> Result<Arc<Mutex<Option<BrowserHandle>>>, String> {
    let cell = BROWSER
        .get_or_init(|| async { Arc::new(Mutex::new(None)) })
        .await
        .clone();

    {
        let mut guard = cell.lock().await;
        let needs_relaunch = match guard.as_ref() {
            None => true,
            Some(h) => h.profile != profile,
        };
        if needs_relaunch {
            // 关闭旧实例（profile 切换或首次）。
            if let Some(old) = guard.take() {
                let mut b = old.browser;
                let _ = b.close().await;
            }
            let browser = launch_browser(profile, true).await?;
            let handler_task = tokio::spawn(async {});
            *guard = Some(BrowserHandle {
                browser,
                _handler_task: handler_task,
                profile: profile.to_string(),
            });
        }
    }
    Ok(cell)
}

/// 用无头 Chrome 渲染一个 URL，返回最终 DOM。
async fn render_with_chrome(url: &str, profile: &str) -> Result<RenderedPage, String> {
    if headless_disabled() {
        return Err("headless chrome disabled (circuit breaker)".to_string());
    }

    let _permit = RENDER_SEMAPHORE
        .acquire()
        .await
        .map_err(|e| format!("semaphore: {e}"))?;

    let cell = match shared_browser(profile).await {
        Ok(c) => c,
        Err(e) => {
            note_headless_failure();
            return Err(e);
        }
    };
    let guard = cell.lock().await;
    let handle = guard.as_ref().ok_or_else(|| "browser not available".to_string())?;

    // 打开新标签页。
    let page = handle
        .browser
        .new_page("about:blank")
        .await
        .map_err(|e| format!("new_page: {e}"))?;

    // 导航前注入 stealth。
    let _ = page.add_init_script(STEALTH_SCRIPT).await;

    // 导航 + 等待（带超时）。
    let nav = tokio::time::timeout(Duration::from_secs(25), async {
        page.goto(url).await.map_err(|e| format!("goto: {e}"))?;
        page.wait_for_navigation().await.map_err(|e| format!("wait_nav: {e}"))?;
        Ok::<(), String>(())
    })
    .await;

    if let Err(_) = nav {
        let _ = page.close().await;
        return Err("chrome navigation timeout".to_string());
    }
    if let Ok(Err(e)) = nav {
        let _ = page.close().await;
        return Err(e);
    }

    // 自动滚动触发懒加载（最多几屏），并给网络一点时间。
    let _ = page
        .evaluate(
            r#"
            new Promise((resolve) => {
              let total = 0;
              const step = 800;
              const timer = setInterval(() => {
                window.scrollBy(0, step);
                total += step;
                if (total >= document.body.scrollHeight || total > 8000) {
                  clearInterval(timer);
                  resolve(true);
                }
              }, 200);
            })
            "#,
        )
        .await;
    tokio::time::sleep(Duration::from_millis(600)).await;

    let final_url = page.url().await.ok().flatten().unwrap_or_else(|| url.to_string());
    let html = page.content().await.map_err(|e| format!("content: {e}"))?;
    let _ = page.close().await;

    note_headless_success();

    Ok(RenderedPage {
        html,
        final_url,
        mode: "chrome",
    })
}

/// 混合抓取：HTTP 优先，瘦身/空壳/已知 JS 站升级到 Chrome 渲染。
/// `http_client` 为复用的 reqwest 客户端；`force_render` 强制走 Chrome；
/// `profile` 为会话 profile（登录墙复用）。
pub async fn fetch_page(
    http_client: &reqwest::Client,
    url: &str,
    force_render: bool,
    profile: &str,
) -> Result<RenderedPage, String> {
    if !force_render {
        // HTTP 优先。
        match http_client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let final_url = resp.url().to_string();
                if let Ok(body) = resp.text().await {
                    if !host_needs_render(url) && !body_is_thin(&body) {
                        return Ok(RenderedPage {
                            html: body,
                            final_url,
                            mode: "http",
                        });
                    }
                    // 太薄 → 升级渲染；渲染失败则退回 HTTP body。
                    match render_with_chrome(url, profile).await {
                        Ok(rendered) => return Ok(rendered),
                        Err(e) => {
                            warn!("flipboard_render: chrome render failed ({}), fallback to http body", e);
                            return Ok(RenderedPage {
                                html: body,
                                final_url,
                                mode: "http",
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // 强制渲染或 HTTP 彻底失败。
    render_with_chrome(url, profile).await
}

/// 以可见窗口打开登录页，供用户手动登录一次；cookie 落盘到该 profile 的 user_data_dir。
/// 返回后即关闭浏览器实例（会话已持久化在磁盘）。
pub async fn open_login_window(profile: &str, login_url: &str) -> Result<(), String> {
    let mut browser = launch_browser(profile, false).await?;
    let page = browser
        .new_page(login_url)
        .await
        .map_err(|e| format!("new_page: {e}"))?;
    let _ = page.add_init_script(STEALTH_SCRIPT).await;
    // 给用户充足时间登录（最多 5 分钟）；窗口可见，用户登录后会话即持久化。
    // 这里不阻塞太久——前端可在用户确认后调用一次抓取来验证。
    let _ = tokio::time::timeout(Duration::from_secs(300), async {
        // 轮询：直到页面 URL 变化（登录跳转）或超时。
        let start_url = page.url().await.ok().flatten().unwrap_or_default();
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let cur = page.url().await.ok().flatten().unwrap_or_default();
            if cur != start_url && !cur.is_empty() {
                // 已发生跳转，多等一会儿让 cookie 落盘。
                tokio::time::sleep(Duration::from_secs(2)).await;
                break;
            }
        }
    })
    .await;
    let _ = browser.close().await;
    Ok(())
}

/// 主动确保 Chromium 已就绪（可被前端"准备渲染引擎"按钮触发下载）。
pub async fn ensure_chromium_ready() -> Result<String, String> {
    let path = ensure_chromium_executable().await?;
    Ok(path.to_string_lossy().to_string())
}
