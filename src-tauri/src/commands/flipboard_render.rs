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
use std::path::{Path, PathBuf};
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
    /// 命中登录墙 / 验证码 / 风控页（提示调用方需要登录或人工验证）。
    pub blocked: bool,
}

/// 同时最多打开的 Chrome 标签页数（Chrome 每页吃内存，保守限制）。
static RENDER_SEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(3));

/// 全局共享的 Chrome 实例（懒启动）。`None` 表示尚未启动。
static BROWSER: OnceCell<Arc<Mutex<Option<BrowserHandle>>>> = OnceCell::const_new();

/// Chrome 熔断器：仅在浏览器**确实**连续启动失败时短暂暂停，避免无意义的反复崩溃；
/// 冷却后自动恢复（绝不永久降级）。正常环境下因启动稳定，几乎不会触发。
static HEADLESS_FAIL_COUNT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
static HEADLESS_DISABLED_UNTIL: Lazy<std::sync::Mutex<Option<std::time::Instant>>> =
    Lazy::new(|| std::sync::Mutex::new(None));
/// 连续失败达到该阈值才进入冷却（提高阈值，避免“动不动就降级”）。
const HEADLESS_FAIL_THRESHOLD: u32 = 6;
/// 冷却时长：到点自动恢复并清零计数。
const HEADLESS_COOLDOWN: Duration = Duration::from_secs(90);

fn headless_disabled() -> bool {
    let mut guard = HEADLESS_DISABLED_UNTIL.lock().unwrap();
    match *guard {
        Some(until) if std::time::Instant::now() < until => true,
        Some(_) => {
            // 冷却结束：自动恢复。
            *guard = None;
            HEADLESS_FAIL_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
            false
        }
        None => false,
    }
}

fn note_headless_failure() {
    let n = HEADLESS_FAIL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if n >= HEADLESS_FAIL_THRESHOLD {
        *HEADLESS_DISABLED_UNTIL.lock().unwrap() =
            Some(std::time::Instant::now() + HEADLESS_COOLDOWN);
        warn!(
            "flipboard_render: headless Chrome paused for {}s after {} failures (will auto-recover)",
            HEADLESS_COOLDOWN.as_secs(),
            n
        );
    }
}

fn note_headless_success() {
    HEADLESS_FAIL_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    *HEADLESS_DISABLED_UNTIL.lock().unwrap() = None;
}

struct BrowserHandle {
    browser: Browser,
    /// 驱动 CDP 事件循环的后台任务句柄。
    _handler_task: tokio::task::JoinHandle<()>,
    profile: String,
}

/// stealth 脚本：全面抹除自动化/无头指纹（参考 puppeteer-extra-stealth /
/// undetected-chromedriver / Crawl4AI 的规避集合）。纯 JS，导航前注入，
/// 对每个新 document 生效。覆盖 webdriver / chrome / plugins / WebGL /
/// userAgentData / 硬件 / 权限 / cdc_ 痕迹 / headless 窗口尺寸等指纹向量。
const STEALTH_SCRIPT: &str = r#"
(() => {
  const def = (obj, prop, get) => { try { Object.defineProperty(obj, prop, { get, configurable: true }); } catch (e) {} };

  // 1. navigator.webdriver -> false（并尝试从原型删除）
  def(Navigator.prototype, 'webdriver', () => false);
  try { delete Navigator.prototype.webdriver; } catch (e) {}

  // 2. 语言
  def(navigator, 'languages', () => ['zh-CN', 'zh', 'en-US', 'en']);

  // 3. plugins / mimeTypes（真实 PDF 阅读器集合）
  const mkPlugin = (name, desc) => ({ name, filename: 'internal-pdf-viewer', description: desc, length: 1 });
  const plugins = [
    mkPlugin('PDF Viewer', 'Portable Document Format'),
    mkPlugin('Chrome PDF Viewer', 'Portable Document Format'),
    mkPlugin('Chromium PDF Viewer', 'Portable Document Format'),
    mkPlugin('Microsoft Edge PDF Viewer', 'Portable Document Format'),
    mkPlugin('WebKit built-in PDF', 'Portable Document Format'),
  ];
  def(navigator, 'plugins', () => {
    const arr = plugins.slice();
    arr.item = (i) => arr[i];
    arr.namedItem = (n) => arr.find(p => p.name === n) || null;
    arr.refresh = () => {};
    return arr;
  });
  def(navigator, 'mimeTypes', () => {
    const arr = [{ type: 'application/pdf', suffixes: 'pdf', description: 'Portable Document Format' }];
    arr.item = (i) => arr[i];
    arr.namedItem = (n) => arr.find(m => m.type === n) || null;
    return arr;
  });

  // 4. window.chrome 完整对象
  if (!window.chrome) window.chrome = {};
  window.chrome.runtime = window.chrome.runtime || {};
  window.chrome.app = window.chrome.app || {
    isInstalled: false,
    InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
    RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
  };
  window.chrome.csi = window.chrome.csi || function () { return {}; };
  window.chrome.loadTimes = window.chrome.loadTimes || function () { return {}; };

  // 5. permissions.query
  try {
    const orig = window.navigator.permissions && window.navigator.permissions.query;
    if (orig) {
      window.navigator.permissions.query = (p) => (
        p && p.name === 'notifications'
          ? Promise.resolve({ state: Notification.permission })
          : orig(p)
      );
    }
  } catch (e) {}

  // 6. 硬件 / 平台指纹
  def(navigator, 'hardwareConcurrency', () => 8);
  def(navigator, 'deviceMemory', () => 8);
  def(navigator, 'platform', () => 'Win32');
  def(navigator, 'maxTouchPoints', () => 0);
  def(navigator, 'vendor', () => 'Google Inc.');

  // 7. userAgentData（Client Hints，与 UA 一致）
  try {
    const uaData = {
      brands: [
        { brand: 'Chromium', version: '124' },
        { brand: 'Google Chrome', version: '124' },
        { brand: 'Not-A.Brand', version: '99' },
      ],
      mobile: false,
      platform: 'Windows',
      getHighEntropyValues: () => Promise.resolve({
        architecture: 'x86', bitness: '64', model: '', platform: 'Windows',
        platformVersion: '15.0.0', uaFullVersion: '124.0.0.0',
        fullVersionList: [
          { brand: 'Chromium', version: '124.0.0.0' },
          { brand: 'Google Chrome', version: '124.0.0.0' },
          { brand: 'Not-A.Brand', version: '99.0.0.0' },
        ],
      }),
      toJSON: () => ({}),
    };
    def(navigator, 'userAgentData', () => uaData);
  } catch (e) {}

  // 8. WebGL vendor/renderer 伪装
  const spoofWebGL = (proto) => {
    if (!proto) return;
    const getParam = proto.getParameter;
    proto.getParameter = function (p) {
      if (p === 37445) return 'Google Inc. (Intel)';
      if (p === 37446) return 'ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)';
      return getParam.apply(this, [p]);
    };
  };
  try { spoofWebGL(WebGLRenderingContext.prototype); } catch (e) {}
  try { spoofWebGL(window.WebGL2RenderingContext && WebGL2RenderingContext.prototype); } catch (e) {}

  // 9. 移除 cdc_ 自动化痕迹（ChromeDriver 注入物）
  try {
    for (const k of Object.keys(window)) {
      if (/^[$_]?cdc_/.test(k) || k.includes('cdc_adoQpoasnfa76pfcZLmcfl')) {
        try { delete window[k]; } catch (e) {}
      }
    }
  } catch (e) {}

  // 10. headless 下 outerWidth/Height 常为 0
  try {
    if (window.outerWidth === 0) def(window, 'outerWidth', () => window.innerWidth);
    if (window.outerHeight === 0) def(window, 'outerHeight', () => window.innerHeight + 85);
  } catch (e) {}

  // 11. navigator.connection（部分检测会查）
  try {
    def(navigator, 'connection', () => ({ effectiveType: '4g', rtt: 50, downlink: 10, saveData: false }));
  } catch (e) {}
})();
"#;

/// 已知 JS 重度渲染 / 登录墙站点的 host 关键字（含 MediaCrawler 目标平台）。
fn host_needs_render(url: &str) -> bool {
    let u = url.to_lowercase();
    [
        "zhihu.com",
        "xiaohongshu.com",
        "xhscdn.com",
        "douyin.com",
        "kuaishou.com",
        "bilibili.com",
        "mp.weixin.qq.com",
        "weibo.com",
        "weibo.cn",
        "tieba.baidu.com",
        "twitter.com",
        "x.com",
        "instagram.com",
        "facebook.com",
        "medium.com",
        "quora.com",
    ]
    .iter()
    .any(|h| u.contains(h))
}

/// 判断渲染后的页面是否是登录墙 / 验证码 / 风控页。
fn looks_like_block_page(html: &str) -> bool {
    let l = html.to_lowercase();
    // 文本越短越可疑；但只要命中明确标记即判为受阻。
    let markers = [
        "captcha",
        "recaptcha",
        "hcaptcha",
        "cf-challenge",
        "cf-browser-verification",
        "just a moment",
        "checking your browser",
        "unusual traffic",
        "security check",
        "verify you are human",
        "滑动验证",
        "滑块验证",
        "完成安全验证",
        "请完成验证",
        "拖动滑块",
        "访问验证",
        "人机验证",
        "环境异常",
        "请登录",
        "登录后查看",
        "登录后继续",
        "扫码登录",
        "sign in to continue",
        "log in to continue",
        "please log in",
        "please sign in",
        // 知乎 / 常见风控限流话术（HTTP 直连或未登录会话常触发）。
        "您当前请求存在异常",
        "暂时限制本次访问",
        "知乎小管家",
        "系统检测到您的网络",
        "访问过于频繁",
        "操作过于频繁",
        "请求过于频繁",
        "unusual traffic from your",
    ];
    markers.iter().any(|m| l.contains(m))
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

/// 解析一个**实测可用**的浏览器：环境变量 > 系统 Chrome/Edge > 随包下载 Chromium。
/// 对每个候选执行真实启动探测（probe），只缓存能真正拉起 + 建立 CDP 连接的那个，
/// 从根本上避免“选了个起不来的浏览器然后一直降级”。
async fn ensure_chromium_executable() -> Result<PathBuf, String> {
    static CHROME_PATH: OnceCell<PathBuf> = OnceCell::const_new();
    CHROME_PATH
        .get_or_try_init(|| async {
            let mut candidates: Vec<PathBuf> = Vec::new();
            // 1. 环境变量显式指定。
            for var in ["GUAXI_CHROME_PATH", "CHROME_PATH"] {
                if let Ok(p) = std::env::var(var) {
                    let pb = PathBuf::from(p);
                    if pb.exists() && !candidates.contains(&pb) {
                        candidates.push(pb);
                    }
                }
            }
            // 2. 系统已安装的 Chrome / Edge（全部候选，Chrome 优先）。
            for pb in find_system_browsers() {
                if !candidates.contains(&pb) {
                    candidates.push(pb);
                }
            }

            // 逐个实测启动，选第一个真正能用的。
            for cand in &candidates {
                if probe_browser(cand).await {
                    info!("flipboard_render: verified working browser: {:?}", cand);
                    return Ok(cand.clone());
                }
                warn!(
                    "flipboard_render: browser candidate failed launch probe, skipping: {:?}",
                    cand
                );
            }

            // 3. 系统浏览器都不可用 → 下载随包 Chromium 并实测。
            let download_dir = chromium_download_dir()?;
            tokio::fs::create_dir_all(&download_dir)
                .await
                .map_err(|e| format!("create chromium dir failed: {e}"))?;
            info!(
                "flipboard_render: no working system browser, fetching bundled Chromium into {:?}",
                download_dir
            );
            let fetcher = BrowserFetcher::new(
                BrowserFetcherOptions::builder()
                    .with_path(&download_dir)
                    .build()
                    .map_err(|e| format!("fetcher options: {e}"))?,
            );
            let dl = fetcher
                .fetch()
                .await
                .map_err(|e| format!("download chromium failed: {e}"))?;
            let exec = dl.executable_path;
            if probe_browser(&exec).await {
                info!("flipboard_render: verified bundled Chromium: {:?}", exec);
            } else {
                warn!(
                    "flipboard_render: bundled Chromium failed probe; will still try on demand: {:?}",
                    exec
                );
            }
            Ok(exec)
        })
        .await
        .cloned()
}

/// 探测系统已安装的 Chrome / Edge（Windows 常见安装路径），返回所有存在的候选（Chrome 优先）。
fn find_system_browsers() -> Vec<PathBuf> {
    let bases: Vec<String> = [
        std::env::var("PROGRAMFILES").ok(),
        std::env::var("PROGRAMFILES(X86)").ok(),
        std::env::var("LOCALAPPDATA").ok(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let rels = [
        r"Google\Chrome\Application\chrome.exe",
        r"Microsoft\Edge\Application\msedge.exe",
    ];
    let mut out: Vec<PathBuf> = Vec::new();
    for rel in rels.iter() {
        for base in bases.iter() {
            let p = PathBuf::from(base).join(rel);
            if p.exists() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// 清理上次未正常关闭残留的“单实例锁”文件。这些残留会让新 Chrome 误以为已有实例在跑，
/// 启动即退出（典型表现：status 21 / "exited before websocket URL could be resolved"）。
fn clean_singleton_locks(dir: &Path) {
    for name in [
        "SingletonLock",
        "SingletonSocket",
        "SingletonCookie",
        "lockfile",
        "DevToolsActivePort",
    ] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

/// 统一构建启动配置。`--remote-allow-origins=*` 是 Chrome 111+ 接受 DevTools WebSocket 的必需项
/// （缺失会导致连接被拒 / "WS Invalid message" / 握手失败）。
fn build_browser_config(
    exec: &Path,
    user_data: &Path,
    headless: bool,
) -> Result<BrowserConfig, String> {
    let ua = "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
        (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

    let mut builder = BrowserConfig::builder()
        .chrome_executable(exec)
        .user_data_dir(user_data)
        .window_size(1366, 900)
        .request_timeout(Duration::from_secs(30))
        // Chrome 111+ DevTools WebSocket 必需，否则连接被拒。
        .arg("--remote-allow-origins=*")
        // ===== 反自动化检测旗标集合（参考 undetected-chromedriver / Crawl4AI）=====
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--disable-features=IsolateOrigins,site-per-process,AutomationControlled,Translate,TranslateUI,OptimizationHints")
        .arg("--disable-infobars")
        .arg("--disable-popup-blocking")
        .arg("--disable-notifications")
        .arg("--disable-background-timer-throttling")
        .arg("--disable-backgrounding-occluded-windows")
        .arg("--disable-renderer-backgrounding")
        .arg("--disable-ipc-flooding-protection")
        .arg("--no-service-autorun")
        .arg("--password-store=basic")
        .arg("--use-mock-keychain")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-gpu")
        .arg("--disable-dev-shm-usage")
        .arg("--lang=zh-CN")
        .arg("--window-size=1366,900")
        .arg(ua);

    // 可选代理（IP 轮换）：环境变量 GUAXI_RENDER_PROXY=http://host:port 或 socks5://...
    if let Some(proxy) = render_proxy() {
        builder = builder.arg(format!("--proxy-server={}", proxy));
    }

    if headless {
        builder = builder.new_headless_mode();
    } else {
        builder = builder.with_head();
    }

    builder
        .no_sandbox()
        .build()
        .map_err(|e| format!("browser config: {e}"))
}

/// 实测探测：尝试以无头模式真正启动该浏览器并打开一个空白页，验证可用性。
async fn probe_browser(exec: &Path) -> bool {
    let user_data = match chrome_profile_dir("__probe__") {
        Ok(d) => d,
        Err(_) => return false,
    };
    let _ = tokio::fs::create_dir_all(&user_data).await;
    clean_singleton_locks(&user_data);

    let config = match build_browser_config(exec, &user_data, true) {
        Ok(c) => c,
        Err(_) => return false,
    };

    match tokio::time::timeout(Duration::from_secs(20), Browser::launch(config)).await {
        Ok(Ok((mut browser, mut handler))) => {
            let h = tokio::spawn(async move {
                while let Some(ev) = handler.next().await {
                    if ev.is_err() {
                        break;
                    }
                }
            });
            // 真正建立一次 CDP 调用，确认 WebSocket 通了。
            let ok = tokio::time::timeout(Duration::from_secs(10), browser.new_page("about:blank"))
                .await
                .map(|r| r.is_ok())
                .unwrap_or(false);
            let _ = browser.close().await;
            h.abort();
            ok
        }
        _ => false,
    }
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
    // 清理上次未正常关闭残留的单实例锁，避免 Chrome 启动即退出（status 21）。
    clean_singleton_locks(&user_data);

    let config = build_browser_config(&exec, &user_data, headless)?;
    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("launch chrome failed: {e}"))?;

    // 无头 Chrome 会被复用并长期驻留（抓取调度器随时会用），主程序退出时必须一起结束，
    // 否则 chrome.exe 会连同一堆渲染子进程残留在后台。
    if let Some(pid) = browser
        .get_mut_child()
        .and_then(|child| child.as_mut_inner().id())
    {
        crate::guaxiservices::process_shutdown::register_managed_pid("chrome(render)", pid);
    }

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

/// 可选渲染代理（环境变量）。后续可扩展为按 feed 配置。
fn render_proxy() -> Option<String> {
    for var in ["GUAXI_RENDER_PROXY", "GUAXI_HTTP_PROXY"] {
        if let Ok(p) = std::env::var(var) {
            let p = p.trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    None
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

    // 人类行为模拟：随机步长滚动 + 随机鼠标移动 + 随机间隔，触发懒加载并降低风控。
    let _ = page
        .evaluate(
            r#"
            new Promise((resolve) => {
              const rand = (a, b) => a + Math.random() * (b - a);
              let total = 0;
              const maxScroll = Math.min((document.body && document.body.scrollHeight) || 4000, 12000);
              const tick = () => {
                try {
                  window.scrollBy(0, rand(280, 900));
                  total += 600;
                  const ev = new MouseEvent('mousemove', {
                    clientX: rand(40, (window.innerWidth || 1200) - 40),
                    clientY: rand(40, (window.innerHeight || 800) - 40),
                    bubbles: true,
                  });
                  document.dispatchEvent(ev);
                } catch (e) {}
                if (total >= maxScroll) { try { window.scrollTo(0, 0); } catch (e) {} resolve(true); }
                else setTimeout(tick, rand(120, 360));
              };
              setTimeout(tick, rand(200, 500));
            })
            "#,
        )
        .await;
    tokio::time::sleep(Duration::from_millis(700)).await;

    let final_url = page.url().await.ok().flatten().unwrap_or_else(|| url.to_string());
    let html = page.content().await.map_err(|e| format!("content: {e}"))?;
    let _ = page.close().await;

    note_headless_success();

    let blocked = looks_like_block_page(&html);
    if blocked {
        warn!("flipboard_render: page looks blocked (login/captcha): {}", url);
    }

    Ok(RenderedPage {
        html,
        final_url,
        mode: "chrome",
        blocked,
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
                            blocked: false,
                        });
                    }
                    // 太薄 → 升级渲染；渲染失败则退回 HTTP body。
                    match render_with_chrome(url, profile).await {
                        Ok(rendered) => return Ok(rendered),
                        Err(e) => {
                            warn!("flipboard_render: chrome render failed ({}), fallback to http body", e);
                            let blocked = looks_like_block_page(&body);
                            return Ok(RenderedPage {
                                html: body,
                                final_url,
                                mode: "http",
                                blocked,
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

/// 关闭并清空共享 Chrome 实例（登录前后调用，避免 user_data_dir 被占用，
/// 并让下次抓取以最新 cookie 重新启动）。
async fn reset_shared_browser() {
    if let Some(cell) = BROWSER.get() {
        let mut guard = cell.lock().await;
        if let Some(old) = guard.take() {
            let mut b = old.browser;
            if let Some(pid) = b.get_mut_child().and_then(|child| child.as_mut_inner().id()) {
                crate::guaxiservices::process_shutdown::unregister_managed_pid(pid);
            }
            let _ = b.close().await;
        }
    }
}

/// 以可见窗口打开登录页，供用户手动登录一次；cookie 落盘到该 profile 的 user_data_dir。
/// 返回后即关闭浏览器实例（会话已持久化在磁盘）。
///
/// 关键：登录用的可见实例与后续无头抓取实例**共用同一 user_data_dir**，Chrome 不允许
/// 两个实例同时占用同一 profile。因此登录前先关掉共享无头实例（释放占用），登录后再关掉
/// 可见实例并清空共享句柄——下次抓取会以登录后的最新 cookie 重新启动。这正是 MediaCrawler
/// “持久化登录上下文 + 复用真实页面”的做法。
pub async fn open_login_window(profile: &str, login_url: &str) -> Result<(), String> {
    // 释放共享无头实例对该 profile 的占用（Chrome 不允许两实例共用同一 user_data_dir）。
    reset_shared_browser().await;

    let exec = ensure_chromium_executable().await?;
    let user_data = chrome_profile_dir(profile)?;
    tokio::fs::create_dir_all(&user_data)
        .await
        .map_err(|e| format!("create profile dir failed: {e}"))?;
    clean_singleton_locks(&user_data);

    // 关键：用普通可见进程直接启动浏览器（不走 CDP / DevTools）。
    // 这样登录窗口完全由用户掌控——扫码 / 输密码 / 过验证码都行，且 DevTools WebSocket
    // 重置不会把窗口关掉。用户登录后 cookie 持久化到该 user_data_dir，后续无头抓取复用。
    // 这正是 MediaCrawler “持久化登录上下文 + 真实浏览器手动登录” 的核心做法。
    // 不传任何自动化旗标（--enable-automation 等），降低被风控识别的概率。
    let mut cmd = std::process::Command::new(&exec);
    cmd.arg(format!("--user-data-dir={}", user_data.display()))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--start-maximized")
        .arg("--new-window")
        .arg(login_url);
    cmd.spawn()
        .map_err(|e| format!("启动登录浏览器失败：{e}"))?;

    info!(
        "flipboard_render: launched standalone login browser for profile {} ({})",
        profile, login_url
    );
    // 立即返回：登录由用户在该窗口内自行完成，cookie 落盘后下次抓取自动复用。
    Ok(())
}

/// 主动确保 Chromium 已就绪（可被前端"准备渲染引擎"按钮触发下载）。
pub async fn ensure_chromium_ready() -> Result<String, String> {
    let path = ensure_chromium_executable().await?;
    Ok(path.to_string_lossy().to_string())
}
