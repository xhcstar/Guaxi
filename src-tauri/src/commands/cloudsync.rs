use crate::commands::settingview::{get_image_bed_root, get_resource_repo_sync_config};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Method};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use url::Url;
use walkdir::WalkDir;

fn normalize_url_base(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

fn truncate_for_error(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}...", &s[..max])
}

fn truncate_bytes_for_log(bytes: &[u8], max: usize) -> Vec<u8> {
    if bytes.len() <= max {
        return bytes.to_vec();
    }
    bytes[..max].to_vec()
}

fn build_webdav_client() -> Result<Client, String> {
    let http_proxy_present = std::env::var("HTTP_PROXY").is_ok() || std::env::var("http_proxy").is_ok();
    let https_proxy_present = std::env::var("HTTPS_PROXY").is_ok() || std::env::var("https_proxy").is_ok();
    let all_proxy_present = std::env::var("ALL_PROXY").is_ok() || std::env::var("all_proxy").is_ok();

    info!(
        "[WebDAV] build client no_proxy=true proxy_env_present http={} https={} all={} ",
        http_proxy_present, https_proxy_present, all_proxy_present
    );

    Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| format!("创建 WebDAV HTTP 客户端失败: {}", e))
}

fn join_webdav_url(base: &str, remote_base_path: &str) -> Result<String, String> {
    let base_trimmed = base.trim();
    let remote_trimmed = remote_base_path.trim();

    // 允许用户从文档/Markdown 里复制 `https://...` 这种带反引号的内容
    let base_trimmed = base_trimmed.trim_matches('`').trim_matches('"').trim_matches('\'');
    let remote_trimmed = remote_trimmed.trim_matches('`').trim_matches('"').trim_matches('\'');

    info!(
        "[WebDAV] join_webdav_url input base={:?} remote_base_path={:?} base_len={} remote_len={} base_bytes_head={:?} remote_bytes_head={:?}",
        base_trimmed,
        remote_trimmed,
        base_trimmed.len(),
        remote_trimmed.len(),
        truncate_bytes_for_log(base_trimmed.as_bytes(), 120),
        truncate_bytes_for_log(remote_trimmed.as_bytes(), 120)
    );

    let mut u = Url::parse(base_trimmed).map_err(|e| format!("WebDAV URL 非法: {}", e))?;
    u.set_query(None);
    u.set_fragment(None);

    // 基础 URL 的 path 段（例如 https://host/dav -> ["dav"]）
    let base_path_segments: Vec<String> = u
        .path()
        .split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    info!(
        "[WebDAV] join_webdav_url base_path={:?} base_path_segments={:?}",
        u.path(),
        base_path_segments
    );

    // 远端目录段（允许 \\，并过滤空段，避免生成 //）
    let remote_norm = remote_trimmed.replace('\\', "/");
    info!("[WebDAV] join_webdav_url remote_norm={:?}", remote_norm);
    let mut remote_segments: Vec<String> = remote_norm
        .split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    info!("[WebDAV] join_webdav_url remote_segments={:?}", remote_segments);

    // 坚果云：强制确保 path 以 /dav/ 开头，并容错去掉重复 dav 段
    if u.host_str() == Some("dav.jianguoyun.com") {
        let base_has_dav = base_path_segments.first().map(|s| s == "dav").unwrap_or(false);
        let remote_has_dav = remote_segments.first().map(|s| s == "dav").unwrap_or(false);

        if base_has_dav && remote_has_dav {
            remote_segments.remove(0);
            warn!("[WebDAV] join_webdav_url jianguoyun remote starts with dav, dedup it");
        } else if !base_has_dav && !remote_has_dav {
            remote_segments.insert(0, "dav".to_string());
            warn!(
                "[WebDAV] join_webdav_url jianguoyun missing /dav/, auto prepend dav segment"
            );
        }
    }

    let mut all_segments: Vec<String> = Vec::new();
    all_segments.extend(base_path_segments);
    all_segments.extend(remote_segments);
    info!("[WebDAV] join_webdav_url all_segments={:?}", all_segments);

    let mut normalized_path = if all_segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", all_segments.join("/"))
    };
    if !normalized_path.ends_with('/') {
        normalized_path.push('/');
    }

    info!("[WebDAV] join_webdav_url normalized_path={:?}", normalized_path);
    u.set_path(&normalized_path);

    let final_url = u.to_string().trim_end_matches('/').to_string();
    let parsed = Url::parse(&final_url).map_err(|e| format!("WebDAV URL 拼接后非法: {}", e))?;

    info!(
        "[WebDAV] join_webdav_url result url={:?} path={:?}",
        final_url,
        parsed.path()
    );

    if parsed.path().contains("//") {
        warn!(
            "[WebDAV] invalid webdav path contains // path={:?} url={:?}",
            parsed.path(),
            final_url
        );
        return Err(format!("WebDAV URL 路径包含非法双斜杠: {}", parsed.path()));
    }

    if parsed.host_str() == Some("dav.jianguoyun.com") {
        let p = parsed.path();
        if !p.starts_with("/dav/") {
            warn!("[WebDAV] 坚果云URL路径异常 path={:?} url={:?}", p, final_url);
            return Err(
                "坚果云 WebDAV URL 路径不正确，应形如: https://dav.jianguoyun.com/dav/你的路径"
                    .to_string(),
            );
        }

        let path_part = p.trim_start_matches("/dav/");
        let path_depth = path_part.split('/').filter(|s| !s.is_empty()).count();
        info!("[WebDAV] 坚果云路径深度: {}", path_depth);
        if path_depth == 0 {
            return Err("坚果云 WebDAV 路径不能为空".to_string());
        }

        for seg in path_part.split('/').filter(|s| !s.is_empty()) {
            if seg.contains(' ') || seg.contains('\\') {
                return Err(format!("坚果云路径段包含非法字符: '{}'", seg));
            }
        }
    }

    Ok(final_url)
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn encode_rel_path(rel: &str) -> String {
    rel.split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Debug, Clone)]
struct WebDavItem {
    rel_path: String,
    is_dir: bool,
}

async fn webdav_propfind(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    depth: &str,
) -> Result<String, String> {
    let method = Method::from_bytes(b"PROPFIND").map_err(|e| e.to_string())?;

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
                        <d:propfind xmlns:d="DAV:">
                          <d:prop>
                            <d:resourcetype />
                            <d:getcontentlength />
                            <d:getlastmodified />
                          </d:prop>
                        </d:propfind>"#;

    let resp = client
        .request(method, url)
        .basic_auth(username, Some(password))
        .header("Depth", depth)
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("WebDAV 请求失败: {}", e))?;

    let code = resp.status().as_u16();
    if code == 200 || code == 207 {
        resp.text().await.map_err(|e| e.to_string())
    } else {
        let text = resp.text().await.unwrap_or_default();
        warn!(
            "[WebDAV] PROPFIND failed http={} url={} resp_len={}",
            code,
            url,
            text.len()
        );
        if !text.is_empty() {
            warn!("[WebDAV] PROPFIND resp_body={} ", text);
        }
        Err(format!(
            "WebDAV 请求失败，HTTP {} url={} resp={}",
            code,
            url,
            truncate_for_error(&text, 500)
        ))
    }
}

fn rel_from_href(base_dir_url: &str, href_raw: &str) -> Option<String> {
    let href = xml_unescape(href_raw).trim().to_string();
    let href_path = if href.starts_with("http://") || href.starts_with("https://") {
        Url::parse(&href).ok()?.path().to_string()
    } else if href.starts_with('/') {
        href
    } else {
        format!("/{}", href)
    };

    let href_path = urlencoding::decode(&href_path).ok()?.into_owned();

    let mut base_path = Url::parse(base_dir_url).ok()?.path().to_string();
    if !base_path.ends_with('/') {
        base_path.push('/');
    }

    if !href_path.starts_with(&base_path) {
        return None;
    }

    let rel = href_path[base_path.len()..].trim_start_matches('/');
    Some(rel.trim_end_matches('/').to_string())
}

fn parse_propfind_items(base_dir_url: &str, xml: &str) -> Vec<WebDavItem> {
    let re_resp = Regex::new(r"(?s)<[^:>]*:response[^>]*>(.*?)</[^:>]*:response>").unwrap();
    let re_href = Regex::new(r"(?s)<[^:>]*:href[^>]*>(.*?)</[^:>]*:href>").unwrap();
    // match both <d:collection/> and <collection/>
    let re_collection = Regex::new(r"<[^:>]*:collection\s*/?>").unwrap();

    let mut out = Vec::new();
    for cap in re_resp.captures_iter(xml) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let href = match re_href.captures(block).and_then(|c| c.get(1)) {
            Some(v) => v.as_str(),
            None => continue,
        };

        let rel = match rel_from_href(base_dir_url, href) {
            Some(v) => v,
            None => continue,
        };
        if rel.is_empty() {
            continue;
        }

        let is_dir = re_collection.is_match(block) || href.trim().ends_with('/');
        out.push(WebDavItem { rel_path: rel, is_dir });
    }
    out
}

async fn webdav_list_recursive_depth1(
    client: &Client,
    base_dir_url: &str,
    username: &str,
    password: &str,
) -> Result<Vec<WebDavItem>, String> {
    let base = base_dir_url.trim_end_matches('/').to_string();
    let base_for_propfind = format!("{}/", base);

    let mut seen_dirs: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut files: HashSet<String> = HashSet::new();
    let mut dirs: HashSet<String> = HashSet::new();

    seen_dirs.insert("".to_string());
    queue.push_back("".to_string());

    while let Some(dir_rel) = queue.pop_front() {
        if seen_dirs.len() > 2000 {
            return Err(format!(
                "WebDAV 目录过多（{}），停止递归扫描，请检查远端路径是否配置正确",
                seen_dirs.len()
            ));
        }

        let dir_url = if dir_rel.is_empty() {
            base_for_propfind.clone()
        } else {
            format!("{}{}/", base_for_propfind, encode_rel_path(&dir_rel))
        };

        info!(
            "[WebDAV] list_recursive_depth1 PROPFIND url={:?} rel={:?} seen_dirs={} queue_len={}",
            dir_url,
            dir_rel,
            seen_dirs.len(),
            queue.len()
        );

        let xml = webdav_propfind(client, &dir_url, username, password, "1").await?;
        let items = parse_propfind_items(&base_for_propfind, &xml);

        for it in items {
            if it.is_dir {
                if dirs.insert(it.rel_path.clone()) && seen_dirs.insert(it.rel_path.clone()) {
                    queue.push_back(it.rel_path);
                }
            } else {
                files.insert(it.rel_path);
            }
        }
    }

    let mut out: Vec<WebDavItem> = Vec::with_capacity(dirs.len() + files.len());
    for d in dirs {
        out.push(WebDavItem {
            rel_path: d,
            is_dir: true,
        });
    }
    for f in files {
        out.push(WebDavItem {
            rel_path: f,
            is_dir: false,
        });
    }
    Ok(out)
}

async fn webdav_mkcol(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let parsed_path = Url::parse(url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_default();

    info!("[WebDAV] MKCOL request url={:?} path={:?}", url, parsed_path);

    let method = Method::from_bytes(b"MKCOL").map_err(|e| e.to_string())?;
    let resp = client
        .request(method, url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("WebDAV MKCOL 请求失败: {}", e))?;

    let code = resp.status().as_u16();
    if code == 201 || code == 405 {
        info!("[WebDAV] MKCOL ok http={} url={:?} path={:?}", code, url, parsed_path);
        return Ok(());
    }

    let text = resp.text().await.unwrap_or_default();
    warn!(
        "[WebDAV] MKCOL failed http={} url={:?} path={:?} resp_len={} resp_body={}",
        code,
        url,
        parsed_path,
        text.len(),
        truncate_for_error(&text, 500)
    );

    Err(format!(
        "WebDAV MKCOL 失败，HTTP {} url={} resp={}",
        code,
        url,
        truncate_for_error(&text, 500)
    ))
}

async fn webdav_head_exists(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<bool, String> {
    let resp = client
        .head(url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("WebDAV HEAD 失败: {}", e))?;

    let code = resp.status().as_u16();
    if resp.status().is_success() {
        Ok(true)
    } else if code == 404 {
        Ok(false)
    } else {
        Err(format!("WebDAV HEAD 失败，HTTP {}", code))
    }
}

async fn webdav_ensure_dir_url(
    client: &Client,
    dir_url: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let mut u = Url::parse(dir_url).map_err(|e| e.to_string())?;
    u.set_query(None);
    u.set_fragment(None);

    let is_jianguoyun = u.host_str() == Some("dav.jianguoyun.com");

    let segs: Vec<String> = u
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    for i in 0..segs.len() {
        let mut partial_path = format!("/{}", segs[..=i].join("/"));
        if !partial_path.ends_with('/') {
            partial_path.push('/');
        }
        u.set_path(&partial_path);
        let cur = u.to_string();

        info!("[WebDAV] ensure_dir_url step url={:?} path={:?}", cur, partial_path);

        if is_jianguoyun && i == 0 && segs.get(0).map(|s| s.as_str()) == Some("dav") {
            info!("[WebDAV] ensure_dir_url skip jianguoyun dav root url={:?}", cur);
            continue;
        }

        match webdav_propfind_ok(client, &cur, username, password).await {
            Ok(_) => {
                info!("[WebDAV] ensure_dir_url exists, skip MKCOL url={:?}", cur);
                continue;
            }
            Err(e) => {
                warn!(
                    "[WebDAV] ensure_dir_url probe failed, will try MKCOL url={:?} err={}",
                    cur, e
                );
            }
        }

        if let Err(e) = webdav_mkcol(client, &cur, username, password).await {
            if let Ok(_) = webdav_propfind_ok(client, &cur, username, password).await {
                warn!(
                    "[WebDAV] MKCOL ignored (exists or not allowed) url={:?} err={}",
                    cur, e
                );
                continue;
            }
            return Err(format!("ensure_dir_url MKCOL failed url={} err={}", cur, e));
        }

        info!("[WebDAV] ensure_dir_url created url={:?}", cur);
    }

    Ok(u.to_string().trim_end_matches('/').to_string())
}

async fn webdav_propfind_ok(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let method = Method::from_bytes(b"PROPFIND").map_err(|e| e.to_string())?;

    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:resourcetype />
  </d:prop>
</d:propfind>"#;

    let url_has_backtick = url.contains('`');
    if let Ok(p) = Url::parse(url) {
        info!(
            "[WebDAV] PROPFIND(test) request url={:?} path={:?} url_len={} has_backtick={} url_bytes_head={:?}",
            url,
            p.path(),
            url.len(),
            url_has_backtick,
            truncate_bytes_for_log(url.as_bytes(), 160)
        );
    } else {
        warn!(
            "[WebDAV] PROPFIND(test) request url parse failed url={:?} url_len={} has_backtick={} url_bytes_head={:?}",
            url,
            url.len(),
            url_has_backtick,
            truncate_bytes_for_log(url.as_bytes(), 160)
        );
    }

    let resp = client
        .request(method, url)
        .basic_auth(username, Some(password))
        .header("Depth", "0")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("WebDAV 请求失败: {}", e))?;

    let code = resp.status().as_u16();
    if code == 200 || code == 207 {
        Ok(())
    } else {
        let text = resp.text().await.unwrap_or_default();
        let parsed_path = Url::parse(url)
            .ok()
            .map(|u| u.path().to_string())
            .unwrap_or_else(|| "".to_string());
        warn!(
            "[WebDAV] PROPFIND(test) failed http={} url={:?} path={:?} url_len={} url_bytes_head={:?} resp_len={}",
            code,
            url,
            parsed_path,
            url.len(),
            truncate_bytes_for_log(url.as_bytes(), 160),
            text.len()
        );
        if !text.is_empty() {
            warn!("[WebDAV] PROPFIND(test) resp_body={} ", text);
        }
        Err(format!(
            "WebDAV 连接失败，HTTP {} url={} resp={}",
            code,
            url,
            truncate_for_error(&text, 500)
        ))
    }
}

#[tauri::command]
pub async fn resource_repo_test_connection() -> Result<String, String> {
    let cfg = get_resource_repo_sync_config();
    let provider = cfg.provider.trim().to_lowercase();

    info!("[ResourceRepoSync] test_connection provider={}", provider);

    if provider == "none" || provider.is_empty() {
        return Err("未启用资源库同步后端".to_string());
    }

    if provider == "webdav" {
        if cfg.webdav.url.trim().is_empty() {
            return Err("WebDAV URL 不能为空".to_string());
        }
        info!(
            "[ResourceRepoSync][WebDAV] test_connection raw_url={:?} raw_remote_base_path={:?} raw_url_len={} raw_remote_len={} raw_url_bytes_head={:?} raw_remote_bytes_head={:?}",
            cfg.webdav.url,
            cfg.webdav.remote_base_path,
            cfg.webdav.url.len(),
            cfg.webdav.remote_base_path.len(),
            truncate_bytes_for_log(cfg.webdav.url.as_bytes(), 160),
            truncate_bytes_for_log(cfg.webdav.remote_base_path.as_bytes(), 160)
        );
        let url0 = join_webdav_url(&cfg.webdav.url, &cfg.webdav.remote_base_path)?;
        info!("[ResourceRepoSync][WebDAV] test_connection joined url0={:?}", url0);
        let url = format!("{}/", url0.trim_end_matches('/'));
        let url_path = Url::parse(&url).ok().map(|u| u.path().to_string()).unwrap_or_default();
        info!("[ResourceRepoSync][WebDAV] test_connection url={:?} path={:?}", url, url_path);

        if cfg.webdav.username.trim().is_empty() {
            return Err("WebDAV 用户名不能为空".to_string());
        }
        if cfg.webdav.password.is_empty() {
            return Err("WebDAV 密码不能为空".to_string());
        }

        let client = build_webdav_client()?;
        webdav_propfind_ok(&client, &url, &cfg.webdav.username, &cfg.webdav.password).await?;
        return Ok("WebDAV 连接正常".to_string());
    }

    if provider == "syncthing" {
        let api = normalize_url_base(&cfg.syncthing.api_url);
        if api.is_empty() {
            return Err("Syncthing API URL 不能为空".to_string());
        }
        if cfg.syncthing.api_key.is_empty() {
            return Err("Syncthing API Key 不能为空".to_string());
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-API-Key",
            HeaderValue::from_str(&cfg.syncthing.api_key)
                .map_err(|_| "Syncthing API Key 非法".to_string())?,
        );

        let client = Client::new();
        let url = format!("{}/rest/system/status", api);
        let resp = client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Syncthing 请求失败: {}", e))?;

        if resp.status().is_success() {
            return Ok("Syncthing 连接正常".to_string());
        }
        Err(format!(
            "Syncthing 连接失败，HTTP {}",
            resp.status().as_u16()
        ))
    } else {
        Err("未知同步后端".to_string())
    }
}

async fn webdav_put_bytes(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let resp = client
        .put(url)
        .basic_auth(username, Some(password))
        .body(bytes)
        .send()
        .await
        .map_err(|e| format!("WebDAV PUT 失败: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("WebDAV PUT 失败，HTTP {}", resp.status().as_u16()))
    }
}

async fn webdav_get_bytes(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<Vec<u8>, String> {
    let resp = client
        .get(url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("WebDAV GET 失败: {}", e))?;

    if resp.status().is_success() {
        let b = resp.bytes().await.map_err(|e| e.to_string())?;
        Ok(b.to_vec())
    } else {
        Err(format!("WebDAV GET 失败，HTTP {}", resp.status().as_u16()))
    }
}

#[tauri::command]
pub async fn resource_repo_sync_now() -> Result<String, String> {
    let cfg = get_resource_repo_sync_config();
    let provider = cfg.provider.trim().to_lowercase();
    let prefer_pull = cfg.prefer_pull;

    info!(
        "[ResourceRepoSync] sync_now provider={} prefer_pull={}",
        provider, prefer_pull
    );

    if provider == "none" || provider.is_empty() {
        return Err("未启用资源库同步后端".to_string());
    }

    if provider == "syncthing" {
        let api = normalize_url_base(&cfg.syncthing.api_url);
        if api.is_empty() {
            return Err("Syncthing API URL 不能为空".to_string());
        }
        if cfg.syncthing.api_key.is_empty() {
            return Err("Syncthing API Key 不能为空".to_string());
        }
        let folder = cfg.syncthing.folder_id.trim().to_string();
        if folder.is_empty() {
            return Err("Syncthing Folder ID 不能为空".to_string());
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-API-Key",
            HeaderValue::from_str(&cfg.syncthing.api_key)
                .map_err(|_| "Syncthing API Key 非法".to_string())?,
        );

        let client = Client::new();
        let url = format!(
            "{}/rest/db/scan?folder={}",
            api,
            urlencoding::encode(&folder)
        );
        let resp = client
            .post(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Syncthing 触发扫描失败: {}", e))?;

        if resp.status().is_success() {
            return Ok("已触发 Syncthing 扫描（后台同步进行中）".to_string());
        }
        return Err(format!(
            "Syncthing 触发扫描失败，HTTP {}",
            resp.status().as_u16()
        ));
    }

    if provider == "webdav" {
        if cfg.webdav.url.trim().is_empty() {
            return Err("WebDAV URL 不能为空".to_string());
        }
        if cfg.webdav.username.trim().is_empty() {
            return Err("WebDAV 用户名不能为空".to_string());
        }
        if cfg.webdav.password.is_empty() {
            return Err("WebDAV 密码不能为空".to_string());
        }

        let root = PathBuf::from(get_image_bed_root());
        ensure_dir(&root)?;

        info!(
            "[ResourceRepoSync][WebDAV] sync raw_url={} raw_remote_base_path={}",
            cfg.webdav.url, cfg.webdav.remote_base_path
        );
        let base_dir_url = join_webdav_url(&cfg.webdav.url, &cfg.webdav.remote_base_path)?;
        let base_dir_path = Url::parse(&base_dir_url)
            .ok()
            .map(|u| u.path().to_string())
            .unwrap_or_default();
        let client = build_webdav_client()?;
        info!(
            "[ResourceRepoSync][WebDAV] sync base_dir_url={:?} path={:?}",
            base_dir_url,
            base_dir_path
        );

        let base_dir_url = webdav_ensure_dir_url(
            &client,
            &base_dir_url,
            &cfg.webdav.username,
            &cfg.webdav.password,
        )
        .await?;
        let base = base_dir_url.trim_end_matches('/').to_string();
        let base_for_propfind = format!("{}/", base);

        webdav_propfind_ok(
            &client,
            &base_for_propfind,
            &cfg.webdav.username,
            &cfg.webdav.password,
        )
        .await?;

        if !prefer_pull {
            let root_canon = root.canonicalize().unwrap_or_else(|_| root.clone());
            let mut dirs: Vec<String> = Vec::new();
            let mut files: Vec<String> = Vec::new();

            for entry in WalkDir::new(&root_canon).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                let rel = match p.strip_prefix(&root_canon) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if rel.as_os_str().is_empty() {
                    continue;
                }
                let rel_str = rel
                    .to_string_lossy()
                    .replace('\\', "/")
                    .trim_start_matches('/')
                    .to_string();
                if rel_str.is_empty() {
                    continue;
                }
                if p.is_dir() {
                    dirs.push(rel_str);
                } else {
                    files.push(rel_str);
                }
            }

            dirs.sort_by_key(|s| s.len());
            dirs.dedup();
            for d in dirs.iter() {
                let url = format!("{}/{}/", base, encode_rel_path(d));
                let _ =
                    webdav_mkcol(&client, &url, &cfg.webdav.username, &cfg.webdav.password).await;
            }

            let mut uploaded = 0u32;
            let mut skipped = 0u32;
            for rel in files.iter() {
                let url = format!("{}/{}", base, encode_rel_path(rel));
                let exists =
                    webdav_head_exists(&client, &url, &cfg.webdav.username, &cfg.webdav.password)
                        .await?;
                if exists {
                    skipped += 1;
                    continue;
                }

                let local_path = root_canon.join(PathBuf::from(rel));
                let bytes = fs::read(&local_path).map_err(|e| e.to_string())?;
                info!(
                    "[ResourceRepoSync][WebDAV] upload_missing rel={} bytes_len={} url={}",
                    rel,
                    bytes.len(),
                    url
                );
                webdav_put_bytes(
                    &client,
                    &url,
                    &cfg.webdav.username,
                    &cfg.webdav.password,
                    bytes,
                )
                .await?;
                uploaded += 1;
            }

            return Ok(format!(
                "WebDAV 推送完成：本地目录 {}，新增文件 {}，跳过已存在文件 {}",
                dirs.len(),
                uploaded,
                skipped
            ));
        }

        let items = match webdav_propfind(
            &client,
            &base_for_propfind,
            &cfg.webdav.username,
            &cfg.webdav.password,
            "infinity",
        )
        .await
        {
            Ok(xml) => {
                info!(
                    "[WebDAV] PROPFIND depth=infinity ok base_url={:?} resp_len={}",
                    base_for_propfind,
                    xml.len()
                );
                parse_propfind_items(&base_for_propfind, &xml)
            }
            Err(e) => {
                warn!(
                    "[WebDAV] PROPFIND depth=infinity failed, fallback to recursive depth=1 scan base_url={:?} err={}",
                    base_for_propfind,
                    e
                );
                match webdav_list_recursive_depth1(
                    &client,
                    &base_for_propfind,
                    &cfg.webdav.username,
                    &cfg.webdav.password,
                )
                .await
                {
                    Ok(items) => items,
                    Err(e2) => {
                        error!(
                            "[WebDAV] fallback recursive depth=1 scan failed base_url={:?} err={}",
                            base_for_propfind,
                            e2
                        );
                        return Err(format!(
                            "WebDAV 拉取失败：PROPFIND(infinity)={}；回退递归扫描(depth=1)={}",
                            e, e2
                        ));
                    }
                }
            }
        };
        let mut remote_dirs: HashSet<String> = HashSet::new();
        let mut remote_files: Vec<String> = Vec::new();
        for it in items.into_iter() {
            if it.is_dir {
                remote_dirs.insert(it.rel_path);
            } else {
                remote_files.push(it.rel_path);
            }
        }

        for d in remote_dirs.iter() {
            ensure_dir(&root.join(PathBuf::from(d)))?;
        }

        let mut downloaded = 0u32;
        let mut skipped = 0u32;
        for rel in remote_files.into_iter() {
            let dest = root.join(PathBuf::from(&rel));
            if dest.exists() {
                skipped += 1;
                continue;
            }
            if let Some(parent) = dest.parent() {
                ensure_dir(parent)?;
            }

            let url = format!("{}/{}", base, encode_rel_path(&rel));
            info!(
                "[ResourceRepoSync][WebDAV] download_missing rel={} url={}",
                rel, url
            );
            let bytes =
                webdav_get_bytes(&client, &url, &cfg.webdav.username, &cfg.webdav.password).await?;
            fs::write(&dest, &bytes).map_err(|e| e.to_string())?;
            downloaded += 1;
        }

        return Ok(format!(
            "WebDAV 拉取完成：远端目录 {}，新增文件 {}，跳过已存在文件 {}",
            remote_dirs.len(),
            downloaded,
            skipped
        ));
    }

    warn!("[ResourceRepoSync] unknown provider={}", provider);
    Err("未知同步后端".to_string())
}
