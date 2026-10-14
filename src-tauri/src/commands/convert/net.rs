//! 文档转换链路的网络环境：把「跟随系统代理 / 直连」收敛成一处。
//!
//! 需求：模型与依赖的下载，**配了系统代理就默认走代理，没配就直连**。
//! 这条规则要同时覆盖三类下载：
//! - Rust 侧：GitHub 源码包（zipball）、uv、Python 解释器；
//! - Python 侧：s3 模型（`urllib`）、HF 权重（`huggingface_hub` → `httpx`）。
//!
//! 为什么必须显式注入环境变量：`requests` 在 Windows 上会自己读注册表代理，但
//! `huggingface_hub` 底层的 `httpx` **只认环境变量**，reqwest 也不读 Windows 注册表。
//! 因此这里把「系统代理」统一翻译成子进程的 `HTTP(S)_PROXY` / `ALL_PROXY`；
//! 没有代理时**显式清空**这几个变量，避免环境里残留的值把境内下载（清华镜像、
//! models.datalab.to）也绕出去。
//!
//! 另有一条容易被忽略的坑：一旦设了代理，本地推理服务的 `127.0.0.1` 调用也会被代理拦掉
//! （`story_studio.rs` 里已经踩过一次）。所以注入代理时**必须同时设 `NO_PROXY`**。

use std::path::Path;

use std::process::Command as StdCommand;
use tokio::process::Command as TokioCommand;

/// 需要写入 / 清理的代理环境变量（大小写各一份：不同库读的键不一样）。
pub const PROXY_ENV_KEYS: [&str; 6] = [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
];

/// 强制直连的地址：本地推理服务（llama-server / surya 各 server）都走回环地址。
pub const NO_PROXY_VALUE: &str = "127.0.0.1,localhost,::1";

/// 当前生效的代理 URL（None 表示直连）。
///
/// 默认跟随系统代理（环境变量优先，其次 Windows 注册表）；
/// `GUAXI_MARKER_PROXY` 可覆盖：给了值就用它，显式置空则强制直连。
pub fn effective_proxy() -> Option<String> {
    match std::env::var("GUAXI_MARKER_PROXY") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else if trimmed.contains("://") {
                Some(trimmed.to_string())
            } else {
                Some(format!("http://{trimmed}"))
            }
        }
        Err(_) => crate::commands::resource_hunter::proxy::system_proxy_url(),
    }
}

/// 一次「代理环境」应用的完整结果：要设的键值对 + 要清掉的键。
#[derive(Debug, Clone)]
pub struct ProxyEnv {
    sets: Vec<(String, String)>,
    removes: Vec<&'static str>,
}

/// 计算当前该给子进程注入的代理环境。
pub fn proxy_env() -> ProxyEnv {
    match effective_proxy() {
        Some(proxy) => ProxyEnv {
            sets: PROXY_ENV_KEYS
                .iter()
                .map(|key| ((*key).to_string(), proxy.clone()))
                .chain(
                    ["NO_PROXY", "no_proxy"]
                        .iter()
                        .map(|key| ((*key).to_string(), NO_PROXY_VALUE.to_string())),
                )
                .collect(),
            removes: Vec::new(),
        },
        None => ProxyEnv {
            sets: Vec::new(),
            // 没配代理就直连：显式清掉，避免残留值把境内源也绕出去。
            removes: PROXY_ENV_KEYS.to_vec(),
        },
    }
}

/// 抽象 `std::process::Command` / `tokio::process::Command` 的环境变量写入，
/// 让同一套代理规则只写一次。
pub trait EnvSink {
    fn env_set(&mut self, key: &str, value: &str);
    fn env_unset(&mut self, key: &str);
}

impl EnvSink for StdCommand {
    fn env_set(&mut self, key: &str, value: &str) {
        self.env(key, value);
    }
    fn env_unset(&mut self, key: &str) {
        self.env_remove(key);
    }
}

impl EnvSink for TokioCommand {
    fn env_set(&mut self, key: &str, value: &str) {
        self.env(key, value);
    }
    fn env_unset(&mut self, key: &str) {
        self.env_remove(key);
    }
}

/// 把代理规则应用到任意子进程命令上。
pub fn apply_proxy_env(cmd: &mut impl EnvSink) {
    let env = proxy_env();
    for (key, value) in &env.sets {
        cmd.env_set(key, value);
    }
    for key in &env.removes {
        cmd.env_unset(key);
    }
}

/// 构造一个带（或不带）代理的阻塞式 HTTP 客户端。
///
/// 显式指定代理，而不是依赖 reqwest 的隐式行为——Windows 上 reqwest 不读注册表设置。
pub fn http_client(proxy: Option<&str>) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(30 * 60))
        .user_agent("guaxi-doc-convert/1.0");
    builder = match proxy {
        Some(url) => {
            let proxy = reqwest::Proxy::all(url).map_err(|e| format!("代理地址无效: {e}"))?;
            builder.proxy(proxy)
        }
        // 走系统代理时也把回环地址排除掉，避免本地服务被拦。
        None => builder.no_proxy(),
    };
    builder.build().map_err(|e| format!("构造 HTTP 客户端失败: {e}"))
}

/// 下载一个 URL 到目标文件（先写 `.part` 再原子替换，失败不留半成品）。
pub fn download_to_file(url: &str, dest: &Path) -> Result<(), String> {
    let proxy = effective_proxy();
    let client = http_client(proxy.as_deref())?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建下载目录失败: {e}"))?;
    }
    let partial = dest.with_extension(format!(
        "{}part",
        dest.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("{e}."))
            .unwrap_or_default()
    ));
    let _ = std::fs::remove_file(&partial);

    let mut response = client
        .get(url)
        .send()
        .map_err(|e| format!("请求失败 {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("下载失败 {url}: HTTP {}", response.status()));
    }

    let mut file = std::fs::File::create(&partial).map_err(|e| format!("创建文件失败: {e}"))?;
    std::io::copy(&mut response, &mut file).map_err(|e| format!("写入失败: {e}"))?;
    drop(file);

    std::fs::rename(&partial, dest).map_err(|e| format!("发布下载文件失败: {e}"))?;
    tracing::info!(
        url = url,
        dest = dest.to_string_lossy().as_ref(),
        via_proxy = proxy.is_some(),
        "[convert] 下载完成"
    );
    Ok(())
}

/// 先镜像后直连地下载一个 GitHub 直链。
/// 大陆网络下 GitHub 直连经常极慢或失败，用现成的加速前缀兜一层。
pub fn download_github_url_with_mirror(github_url: &str, dest: &Path) -> Result<(), String> {
    let mirror = github_mirror_prefix();
    if !mirror.is_empty() {
        let mirrored = if mirror.ends_with('/') {
            format!("{mirror}{github_url}")
        } else {
            format!("{mirror}/{github_url}")
        };
        match download_to_file(&mirrored, dest) {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::warn!(
                    mirror = mirrored.as_str(),
                    error = error.as_str(),
                    "[convert] GitHub 镜像下载失败，回退直连"
                );
            }
        }
    }
    download_to_file(github_url, dest)
}

/// 当前 GitHub 加速前缀：环境变量优先（置空即直连），否则用默认代理前缀。
/// 与 `code_runtime` 里 uv / Python 下载用的是同一个开关。
pub fn github_mirror_prefix() -> String {
    match std::env::var("GUAXI_GITHUB_MIRROR") {
        Ok(value) => value.trim().to_string(),
        Err(_) => "https://ghfast.top/".to_string(),
    }
}
