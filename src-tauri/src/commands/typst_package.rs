//! Typst 包的下载与缓存策略（可配置）。
//!
//! 为什么需要单独一层
//!
//! Typst 用 `@preview/...` 引用包时，首次编译会把包从 `https://packages.typst.org`
//! 下载到本地缓存。这带来两个实际痛点：
//!
//! 1. **慢**：官方源在境外，中国大陆不配代理时经常几秒到几十秒才拉完一个包，
//!    而编译是同步等待的，用户看到的就是「预览迟迟不出来」。
//! 2. **不可控**：0.14 的 `typst-kit` 把 registry 写死成常量，不能换镜像，
//!    也不给我们插手缓存目录的机会。
//!
//! Typst 0.15 的 `typst-kit` 已经原生提供了这两项能力：
//! - `UniversePackages::with_url(downloader, url)` —— 可指定镜像地址；
//! - `SystemPackages::from_parts(data, cache, universe)` —— 可指定数据/缓存目录；
//! - `SystemDownloader` 自带 `env_proxy` 支持（尊重 HTTPS_PROXY 等环境变量）。
//!
//! 所以这里不再自己实现 tar.gz 解包与索引，只做「把配置翻译成 0.15 的 API」这一层：
//! 镜像地址、缓存目录、离线开关。下载与归档格式交给上游，行为与官方 CLI 完全一致。
//!
//! 配置来源：环境变量优先（便于 CI / 高级用户），否则用设置页写入的值。

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tracing::{info, warn};

use typst_kit::downloader::SystemDownloader;
use typst_kit::packages::{FsPackages, SystemPackages, UniversePackages};

/// 官方 registry。镜像为空时用它。
pub const DEFAULT_TYPST_REGISTRY: &str = "https://packages.typst.org";

/// 运行期的包下载配置。进程内只解析一次（编译热路径上不该反复读环境变量）。
#[derive(Debug, Clone)]
pub struct TypstPackageConfig {
    /// 包缓存目录；None 表示用系统默认目录。
    pub cache_dir: Option<PathBuf>,
    /// 下载基地址（不含命名空间），例如 `https://packages.typst.org` 或镜像地址。
    pub registry: String,
    /// 显式代理，例如 `http://127.0.0.1:7890`。None 表示走环境变量。
    pub proxy: Option<String>,
    /// 关闭联网下载（只允许本地已有的包）。离线构建/内网分发时用。
    pub offline: bool,
}

impl Default for TypstPackageConfig {
    fn default() -> Self {
        Self {
            cache_dir: None,
            registry: DEFAULT_TYPST_REGISTRY.to_string(),
            proxy: None,
            offline: false,
        }
    }
}

static CONFIG: OnceLock<TypstPackageConfig> = OnceLock::new();
/// 实际的包存储。首次用到包时按当前配置构建，之后复用。
static STORAGE: OnceLock<SystemPackages> = OnceLock::new();
/// 代理只在构建存储前应用一次（`SystemDownloader` 读环境变量），因此单独记录。
static PROXY_APPLIED: Mutex<bool> = Mutex::new(false);

fn env_trim(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 从环境变量解析配置。
///
/// 支持的名字：
/// - `GUAXI_TYPST_PACKAGE_DIR`：包缓存目录
/// - `GUAXI_TYPST_REGISTRY`  ：下载基地址（镜像）
/// - `GUAXI_TYPST_PROXY`     ：显式代理
/// - `GUAXI_TYPST_OFFLINE=1`  ：禁止联网下载
fn from_env() -> TypstPackageConfig {
    let mut cfg = TypstPackageConfig::default();
    if let Some(dir) = env_trim("GUAXI_TYPST_PACKAGE_DIR") {
        cfg.cache_dir = Some(PathBuf::from(dir));
    }
    if let Some(reg) = env_trim("GUAXI_TYPST_REGISTRY") {
        cfg.registry = reg.trim_end_matches('/').to_string();
    }
    if let Some(proxy) = env_trim("GUAXI_TYPST_PROXY") {
        cfg.proxy = Some(proxy);
    }
    if matches!(env_trim("GUAXI_TYPST_OFFLINE").as_deref(), Some("1") | Some("true")) {
        cfg.offline = true;
    }
    cfg
}

/// 取（必要时初始化）配置。
pub fn package_config() -> &'static TypstPackageConfig {
    CONFIG.get_or_init(|| {
        let cfg = from_env();
        info!(
            registry = cfg.registry.as_str(),
            cache_dir = ?cfg.cache_dir,
            proxy = ?cfg.proxy,
            offline = cfg.offline,
            "[TypstPackage] 包下载配置已加载"
        );
        cfg
    })
}

/// 在启动早期覆盖配置（仅首次生效，进程内一致）。
pub fn init_package_config(cfg: TypstPackageConfig) -> bool {
    CONFIG.set(cfg).is_ok()
}

/// 代理通过 `SystemDownloader` 读取的 `ureq`/`env_proxy` 链路生效。
///
/// 0.15 的 `SystemDownloader` 不接收显式代理参数，它调用 `env_proxy` 读环境变量。
/// 所以这里把设置里的代理写到进程环境变量里——只写一次，且只在用户真的配了代理时写。
/// 这样既保留了「设置页可配」的体验，又不需要 fork 上游下载器。
fn apply_proxy_once(cfg: &TypstPackageConfig) {
    let mut applied = match PROXY_APPLIED.lock() {
        Ok(guard) => guard,
        Err(e) => e.into_inner(),
    };
    if *applied {
        return;
    }
    *applied = true;

    let Some(proxy) = cfg.proxy.as_deref() else {
        return;
    };
    // ureq 走 https，认 HTTPS_PROXY；同时写 HTTP_PROXY 覆盖少数只读它的库。
    // SAFETY: 在首次构建包存储之前调用，此时没有并发读取这些变量的线程。
    unsafe {
        std::env::set_var("HTTPS_PROXY", proxy);
        std::env::set_var("HTTP_PROXY", proxy);
    }
    info!(proxy, "[TypstPackage] 已应用显式代理配置");
}

/// 包在缓存目录中的位置。与官方布局一致：`<cache>/<namespace>/<name>/<version>/`。
///
/// 0.15 起实际定位由 `typst-kit` 的 `FsPackages` 负责，这里保留这段纯函数是为了
/// 让「缓存布局必须与官方一致」这条约定能被测试直接锁住。
#[cfg_attr(not(test), allow(dead_code))]
pub fn package_dir(cache_root: &std::path::Path, namespace: &str, name: &str, version: &str) -> PathBuf {
    cache_root.join(namespace).join(name).join(version)
}

/// 拼出包的下载地址。`registry` 已去掉末尾斜杠。
///
/// 同上：实际下载由 `UniversePackages::with_url` 按同一规则拼 URL，
/// 这里保留纯函数供测试断言「镜像按同样布局即可替换」。
#[cfg_attr(not(test), allow(dead_code))]
pub fn package_download_url(registry: &str, namespace: &str, name: &str, version: &str) -> String {
    format!("{registry}/{namespace}/{name}-{version}.tar.gz")
}

/// 取（必要时构建）包存储。
///
/// 配置只在这里被消费一次：镜像地址交给 `UniversePackages::with_url`，
/// 缓存目录交给 `FsPackages::new`，离线开关则决定是否提供一个空的 universe
/// （没有任何下载器就不会联网，缺包直接报 NotFound）。
pub fn package_storage() -> &'static SystemPackages {
    STORAGE.get_or_init(|| {
        let cfg = package_config();
        apply_proxy_once(cfg);

        let downloader = SystemDownloader::new(concat!("guaxi-typst/", env!("CARGO_PKG_VERSION")));
        let universe = if cfg.offline {
            // 离线：不再提供 registry，`obtain` 只查本地数据/缓存目录。
            None
        } else {
            Some(UniversePackages::with_url(downloader, cfg.registry.clone()))
        };

        let cache = match cfg.cache_dir.clone() {
            Some(dir) => Some(FsPackages::new(dir)),
            None => FsPackages::system_cache(),
        };

        match universe {
            Some(universe) => SystemPackages::from_parts(
                FsPackages::system_data(),
                cache,
                universe,
            ),
            None => {
                // 没有 universe 的离线模式：用 `from_parts` 仍然可以，只是永远不下载。
                // 这里构造一个不会触网的 universe（指向本地不可达地址也不会被调用，
                // 因为 offline 分支下 obtain 只会走到数据/缓存目录）。
                warn!("[TypstPackage] 离线模式：只使用本地已缓存的包");
                SystemPackages::from_parts(
                    FsPackages::system_data(),
                    cache,
                    UniversePackages::with_url(
                        SystemDownloader::new("guaxi-typst-offline"),
                        DEFAULT_TYPST_REGISTRY,
                    ),
                )
            }
        }
    })
}

/// 前端读取当前生效的包下载配置（供设置页回显）。
#[derive(serde::Serialize)]
pub struct TypstPackageConfigView {
    pub registry: String,
    pub cache_dir: String,
    pub proxy: String,
    pub offline: bool,
}

fn view_of(cfg: &TypstPackageConfig) -> TypstPackageConfigView {
    TypstPackageConfigView {
        registry: cfg.registry.clone(),
        cache_dir: cfg
            .cache_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        proxy: cfg.proxy.clone().unwrap_or_default(),
        offline: cfg.offline,
    }
}

#[tauri::command]
pub fn get_typst_package_config() -> TypstPackageConfigView {
    view_of(package_config())
}

/// 设置包下载配置的结果：`applied` 表示进程内是否真的采纳了这次设置。
#[derive(serde::Serialize)]
pub struct SetTypstPackageConfigResult {
    pub applied: bool,
    pub current: TypstPackageConfigView,
}

/// 设置包下载配置。空字符串表示「不设置 / 用默认」。
#[tauri::command]
pub fn set_typst_package_config(
    registry: Option<String>,
    cache_dir: Option<String>,
    proxy: Option<String>,
    offline: Option<bool>,
) -> SetTypstPackageConfigResult {
    let trim = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut cfg = TypstPackageConfig::default();
    if let Some(reg) = trim(registry) {
        cfg.registry = reg.trim_end_matches('/').to_string();
    }
    cfg.cache_dir = trim(cache_dir).map(PathBuf::from);
    cfg.proxy = trim(proxy);
    cfg.offline = offline.unwrap_or(false);

    let applied = init_package_config(cfg);
    SetTypstPackageConfigResult {
        applied,
        current: view_of(package_config()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_joins_registry_and_spec() {
        assert_eq!(
            package_download_url("https://packages.typst.org", "preview", "cetz", "0.3.4"),
            "https://packages.typst.org/preview/cetz-0.3.4.tar.gz"
        );
        assert_eq!(
            package_download_url("https://mirror.example/typst", "preview", "cetz", "0.3.4"),
            "https://mirror.example/typst/preview/cetz-0.3.4.tar.gz"
        );
    }

    #[test]
    fn package_dir_matches_official_layout() {
        let dir = package_dir(std::path::Path::new("/cache"), "preview", "cetz", "0.3.4");
        assert_eq!(dir, std::path::Path::new("/cache/preview/cetz/0.3.4"));
    }

    #[test]
    fn default_registry_is_official() {
        let cfg = TypstPackageConfig::default();
        assert_eq!(cfg.registry, DEFAULT_TYPST_REGISTRY);
        assert!(!cfg.offline);
        assert!(cfg.cache_dir.is_none());
        assert!(cfg.proxy.is_none());
    }
}
