//! Typst 包的下载与缓存策略（可配置）。
//!
//! 为什么需要单独一层
//!
//! Typst 用 `@preview/...` 引用包时，首次编译会把包从 `https://packages.typst.org`
//! 下载到本地缓存。这带来两个实际痛点：
//!
//! 1. **慢**：官方源在境外，中国大陆不配代理时经常几秒到几十秒才拉完一个包，
//!    而编译是同步等待的，用户看到的就是「预览迟迟不出来」。
//! 2. **不可控**：`typst-kit` 把 registry 写死成常量（`DEFAULT_REGISTRY`），
//!    既不能换镜像，也不给我们插手下载过程的机会。
//!
//! 因此这里自带一个 `PackageStorage` 的替代实现：
//! - 包缓存目录可配置（默认仍是 `typst-kit` 选的系统缓存目录，行为不变）；
//! - 下载地址可配置成镜像前缀（`https://mirror/.../{ns}/{name}-{version}.tar.gz`）；
//! - 下载走 `ureq` 并支持显式代理（比 env_proxy 更可控，用户能在设置里填）。
//!
//! 归档格式与官方一致（tar.gz，解包后是包目录），所以镜像只要按同样的路径布局
//! 镜像一份就能直接替换。
//!
//! 配置来源：环境变量优先（便于 CI / 高级用户），否则用应用设置里的值。
//! 这样即使不在设置页加 UI，用户也能通过环境变量立刻生效。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::{info, warn};

/// 官方 registry。镜像前缀为空时用它。
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
        // 去掉末尾斜杠：拼接时统一 `{registry}/{ns}/{name}-{ver}.tar.gz`。
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

/// 在启动早期（或设置变更后首次编译前）覆盖配置。
///
/// 只用 `set` 的「首次生效」语义：配置被编译热路径缓存，进程内改过之后不会影响
/// 已经解析的那一份。设置页改完提示用户重开文档才有意义，这里如实返回是否真的生效。
pub fn init_package_config(cfg: TypstPackageConfig) -> bool {
    CONFIG.set(cfg).is_ok()
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

/// 包在缓存目录中的位置。与官方布局一致：`<cache>/<namespace>/<name>/<version>/`。
pub fn package_dir(cache_root: &Path, namespace: &str, name: &str, version: &str) -> PathBuf {
    cache_root.join(namespace).join(name).join(version)
}

/// 拼出包的下载地址。`registry` 已去掉末尾斜杠。
pub fn package_download_url(registry: &str, namespace: &str, name: &str, version: &str) -> String {
    format!("{registry}/{namespace}/{name}-{version}.tar.gz")
}

/// 下载并解包一个包到 `<cache>/<ns>/<name>/<version>`。
///
/// 已经存在就直接返回，不发网络请求 —— 这是「第二次编译很快」的前提。
/// 解包先落到临时目录再 rename：中途失败不会留下半个包，让下次误以为已缓存。
pub fn ensure_package(
    spec_namespace: &str,
    name: &str,
    version: &str,
    progress: impl Fn(&str),
) -> Result<PathBuf, String> {
    let cfg = package_config();
    let cache_root = match cfg.cache_dir.clone() {
        Some(dir) => dir,
        None => typst_kit::package::default_package_cache_path()
            .ok_or_else(|| "无法确定 Typst 包缓存目录（请在设置里指定）".to_string())?,
    };
    let target = package_dir(&cache_root, spec_namespace, name, version);
    if target.is_dir() {
        return Ok(target);
    }
    if cfg.offline {
        return Err(format!(
            "离线模式下不允许下载包 @{spec_namespace}/{name}:{version}",
        ));
    }

    std::fs::create_dir_all(&cache_root)
        .map_err(|e| format!("创建包缓存目录失败: {e}"))?;

    let url = package_download_url(&cfg.registry, spec_namespace, name, version);
    progress(&url);
    info!(url = url.as_str(), "[TypstPackage] 开始下载包");

    let bytes = download_bytes(&url, cfg.proxy.as_deref())?;

    // 解包到临时目录，成功后再 rename 到最终位置。
    let staging = target.with_extension(format!("staging-{}", std::process::id()));
    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;
    if let Err(e) = unpack_tar_gz(&bytes, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // 归档内的顶层目录名由打包者决定，这里找「第一个目录」当作包根。
    let root = match first_subdir(&staging) {
        Ok(dir) => dir,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建包目录失败: {e}"))?;
    }
    std::fs::rename(&root, &target).map_err(|e| format!("安装包失败: {e}"))?;
    let _ = std::fs::remove_dir_all(&staging);

    info!(dir = %target.display(), "[TypstPackage] 包已就绪");
    Ok(target)
}

fn download_bytes(url: &str, proxy: Option<&str>) -> Result<Vec<u8>, String> {
    let mut agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(120))
        .user_agent(concat!("guaxi-typst/", env!("CARGO_PKG_VERSION")));
    // 显式代理优先；没配就交给环境变量（ureq 默认不读，这里显式补上 env_proxy）。
    let proxy_url = proxy
        .map(|p| p.to_string())
        .or_else(|| env_proxy::for_url_str(url).to_url().map(|u| u.to_string()));
    if let Some(p) = proxy_url.as_deref() {
        match ureq::Proxy::new(p) {
            Ok(proxy) => {
                info!(proxy = p, "[TypstPackage] 使用代理下载");
                agent = agent.proxy(proxy);
            }
            Err(e) => warn!(proxy = p, error = %e, "[TypstPackage] 代理地址无效，改为直连"),
        }
    }

    let response = agent
        .build()
        .get(url)
        .call()
        .map_err(|e| format!("下载包失败 ({url}): {e}"))?;
    let mut buf = Vec::new();
    use std::io::Read;
    response
        .into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("读取下载内容失败: {e}"))?;
    Ok(buf)
}

fn unpack_tar_gz(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    archive
        .unpack(dest)
        .map_err(|e| format!("解包失败: {e}"))?;
    Ok(())
}

fn first_subdir(dir: &Path) -> Result<PathBuf, String> {
    let mut entries = std::fs::read_dir(dir).map_err(|e| format!("读取解包目录失败: {e}"))?;
    let mut dirs = Vec::new();
    for entry in entries.by_ref().flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    match dirs.len() {
        0 => Err("包归档中找不到目录".to_string()),
        1 => Ok(dirs.remove(0)),
        _ => {
            // 多个顶层目录（少见）：挑包含 typst.toml 的那个。
            dirs.into_iter()
                .find(|d| d.join("typst.toml").is_file())
                .ok_or_else(|| "包归档中找不到 typst.toml".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_joins_registry_and_spec() {
        assert_eq!(
            package_download_url(
                "https://packages.typst.org",
                "preview",
                "cetz",
                "0.3.4"
            ),
            "https://packages.typst.org/preview/cetz-0.3.4.tar.gz"
        );
        // 镜像同样布局
        assert_eq!(
            package_download_url("https://mirror.example/typst", "preview", "cetz", "0.3.4"),
            "https://mirror.example/typst/preview/cetz-0.3.4.tar.gz"
        );
    }

    #[test]
    fn package_dir_matches_official_layout() {
        let dir = package_dir(Path::new("/cache"), "preview", "cetz", "0.3.4");
        assert_eq!(dir, Path::new("/cache/preview/cetz/0.3.4"));
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
