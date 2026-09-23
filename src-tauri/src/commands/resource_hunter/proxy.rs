//! 代理层：把 ProxyConfig 变成带代理的 reqwest::Client，并按目标域分流。
//! 三条通道共用——通用搜索、HTTP 下载各自 build_client；yt-dlp 走 proxy_arg。
use super::model::{ProxyConfig, ProxyMode};
use reqwest::Client;
use std::time::Duration;
use url::Url;

/// 解析系统代理（跟随系统模式）。sysproxy 读 Windows 注册表 / 各平台设置。
fn system_proxy_url() -> Option<String> {
    // 优先环境变量（部分终端/代理软件只设 env）。
    for key in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(normalize_proxy(v));
            }
        }
    }
    match sysproxy::Sysproxy::get_system_proxy() {
        Ok(sp) if sp.enable && !sp.host.is_empty() => {
            Some(format!("http://{}:{}", sp.host, sp.port))
        }
        _ => None,
    }
}

fn normalize_proxy(raw: &str) -> String {
    if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{}", raw)
    }
}

/// 当前生效的代理 URL（None 表示直连）。
pub fn effective_proxy_url(cfg: &ProxyConfig) -> Option<String> {
    match cfg.mode {
        ProxyMode::None => None,
        ProxyMode::System => system_proxy_url(),
        ProxyMode::Custom => cfg.url.as_ref().map(|u| normalize_proxy(u.trim())),
    }
}

/// 目标 URL 是否命中直连白名单。
fn is_bypassed(cfg: &ProxyConfig, target: &str) -> bool {
    let host = match Url::parse(target) {
        Ok(u) => u.host_str().unwrap_or("").to_ascii_lowercase(),
        Err(_) => return false,
    };
    cfg.bypass.iter().any(|b| {
        let b = b.trim().to_ascii_lowercase();
        !b.is_empty() && (host == b || host.ends_with(&b))
    })
}

/// 目标是否像境外域名（智能分流轻量启发式）：不以常见境内后缀结尾。
fn looks_foreign(target: &str) -> bool {
    let host = match Url::parse(target) {
        Ok(u) => u.host_str().unwrap_or("").to_ascii_lowercase(),
        Err(_) => return false,
    };
    const CN_HINTS: [&str; 6] = [".cn", "baidu.", "bilibili.", "qq.com", "163.com", "sogou."];
    !CN_HINTS.iter().any(|h| host.contains(h))
}

/// 为某个目标 URL 决定是否套代理，构造对应的 Client。
pub fn build_client_for(cfg: &ProxyConfig, target: &str) -> Result<Client, String> {
    let mut want_proxy = effective_proxy_url(cfg);
    if want_proxy.is_some() {
        if is_bypassed(cfg, target) {
            want_proxy = None;
        } else if cfg.proxy_foreign_only && !looks_foreign(target) {
            want_proxy = None;
        }
    }
    build_client_raw(want_proxy.as_deref())
}

/// 不分流、直接指定代理（或直连）构造 Client。
pub fn build_client_raw(proxy_url: Option<&str>) -> Result<Client, String> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/124.0 Safari/537.36",
        );
    if let Some(url) = proxy_url {
        let proxy = reqwest::Proxy::all(url).map_err(|e| format!("代理地址无效: {e}"))?;
        builder = builder.proxy(proxy);
    } else {
        builder = builder.no_proxy();
    }
    builder.build().map_err(|e| format!("构造 HTTP 客户端失败: {e}"))
}

/// 给 yt-dlp 传的 --proxy 参数（None 表示不传）。
pub fn proxy_arg(cfg: &ProxyConfig) -> Option<String> {
    effective_proxy_url(cfg)
}
