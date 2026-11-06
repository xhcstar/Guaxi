use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{CONTENT_LENGTH, HeaderValue};
use reqwest::{Client, Method};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use tracing::info;
use url::Url;

use super::{RemoteEntry, SyncBackend, SyncConnection, encode_key, mtime_ms, normalize_key};

pub struct WebDavBackend {
    client: Client,
    base: String,
    base_path: String,
    username: String,
    password: String,
}

impl WebDavBackend {
    pub fn new(connection: &SyncConnection, remote_root: &str) -> Result<Self, String> {
        let raw = connection.webdav.url.trim();
        if raw.is_empty() {
            return Err("WebDAV 地址不能为空".to_string());
        }
        let mut url = Url::parse(raw.trim_matches('`').trim_matches('"'))
            .map_err(|e| format!("WebDAV 地址非法：{}", e))?;
        url.set_query(None);
        url.set_fragment(None);

        let mut base = url.to_string().trim_end_matches('/').to_string();
        let root = normalize_key(remote_root);
        if !root.is_empty() {
            base = format!("{}/{}", base, encode_key(&root));
        }
        let raw_base_path = Url::parse(&base)
            .map_err(|e| e.to_string())?
            .path()
            .to_string();
        // PROPFIND 的 href 会被解码后再比较；base 也必须用解码后的形式，中文目录才匹配得上。
        let base_path = urlencoding::decode(&raw_base_path)
            .map(|value| value.into_owned())
            .unwrap_or(raw_base_path);

        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(600))
                .build()
                .map_err(|e| format!("创建 WebDAV 客户端失败：{}", e))?,
            base,
            base_path,
            username: connection.webdav.username.trim().to_string(),
            password: connection.webdav.password.clone(),
        })
    }

    fn url_for(&self, key: &str) -> String {
        if key.is_empty() {
            self.base.clone()
        } else {
            format!("{}/{}", self.base, encode_key(key))
        }
    }

    fn url_for_dir(&self, key: &str) -> String {
        format!("{}/", self.url_for(key))
    }

    fn with_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.username.is_empty() && self.password.is_empty() {
            request
        } else {
            request.basic_auth(&self.username, Some(&self.password))
        }
    }

    async fn propfind(&self, url: &str, depth: &str) -> Result<String, String> {
        let method = Method::from_bytes(b"PROPFIND").map_err(|e| e.to_string())?;
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop>
<d:resourcetype/><d:getcontentlength/><d:getlastmodified/>
</d:prop></d:propfind>"#;
        let resp = self
            .with_auth(self.client.request(method, url))
            .header("Depth", depth)
            .header("Content-Type", "text/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("WebDAV 请求失败：{}", e))?;
        let code = resp.status().as_u16();
        if code == 200 || code == 207 {
            resp.text().await.map_err(|e| e.to_string())
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(format!("WebDAV HTTP {}：{}", code, truncate(&text, 200)))
        }
    }

    async fn ensure_root(&self) {
        let root = format!("{}/", self.base.trim_end_matches('/'));
        if self.propfind(&root, "0").await.is_err() {
            let _ = self.mkcol(&root).await;
        }
    }

    async fn mkcol(&self, url: &str) -> Result<(), String> {
        let method = Method::from_bytes(b"MKCOL").map_err(|e| e.to_string())?;
        let resp = self
            .with_auth(self.client.request(method, url))
            .send()
            .await
            .map_err(|e| format!("WebDAV MKCOL 失败：{}", e))?;
        let code = resp.status().as_u16();
        if matches!(code, 200 | 201 | 405) {
            Ok(())
        } else {
            Err(format!("WebDAV 创建目录失败，HTTP {}", code))
        }
    }
}

#[async_trait]
impl SyncBackend for WebDavBackend {
    async fn test(&self) -> Result<String, String> {
        self.ensure_root().await;
        let xml = self.propfind(&format!("{}/", self.base.trim_end_matches('/')), "0").await?;
        let _ = xml;
        Ok("WebDAV 连接正常".to_string())
    }

    async fn list(&self) -> Result<Vec<RemoteEntry>, String> {
        self.ensure_root().await;
        use futures_util::StreamExt;

        let mut out = Vec::new();
        let mut level = vec![String::new()];
        let mut seen = std::collections::HashSet::new();
        seen.insert(String::new());

        // 按层并发：目录多时不再一层一层串行等 RTT。
        while !level.is_empty() {
            if seen.len() > 5000 {
                return Err("WebDAV 目录过多，请检查远端路径".to_string());
            }
            let results = futures_util::stream::iter(level.into_iter().map(|dir| async move {
                let url = self.url_for_dir(&dir);
                (dir, self.propfind(&url, "1").await)
            }))
            .buffer_unordered(6)
            .collect::<Vec<_>>()
            .await;

            let mut next = Vec::new();
            for (dir, xml) in results {
                let xml = xml.map_err(|e| format!("列目录 {} 失败：{}", dir, e))?;
                for item in parse_propfind(&self.base_path, &xml) {
                    if item.is_dir {
                        if seen.insert(item.key.clone()) {
                            next.push(item.key);
                        }
                    } else if !item.key.is_empty() {
                        out.push(RemoteEntry {
                            key: item.key,
                            size: item.size,
                            mtime_ms: item.mtime_ms,
                        });
                    }
                }
            }
            level = next;
        }
        info!("[Sync][WebDAV] listed {} files", out.len());
        Ok(out)
    }

    async fn stat(&self, key: &str) -> Result<Option<RemoteEntry>, String> {
        let url = self.url_for(key);
        let resp = self
            .with_auth(self.client.head(&url))
            .send()
            .await
            .map_err(|e| format!("WebDAV HEAD 失败：{}", e))?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("WebDAV HEAD 失败，HTTP {}", resp.status().as_u16()));
        }
        let size = resp
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let mtime = resp
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .and_then(parse_http_date)
            .unwrap_or(0);
        Ok(Some(RemoteEntry {
            key: key.to_string(),
            size,
            mtime_ms: mtime,
        }))
    }

    async fn get_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let url = self.url_for(key);
        let resp = self
            .with_auth(self.client.get(&url))
            .send()
            .await
            .map_err(|e| format!("WebDAV 下载失败：{}", e))?;
        if !resp.status().is_success() {
            return Err(format!("WebDAV 下载失败，HTTP {}", resp.status().as_u16()));
        }
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        let mut file = tokio::fs::File::create(local)
            .await
            .map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("读取 WebDAV 响应失败：{}", e))?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn put_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let meta = tokio::fs::metadata(local)
            .await
            .map_err(|e| e.to_string())?;
        let file = tokio::fs::File::open(local)
            .await
            .map_err(|e| e.to_string())?;
        let stream = ReaderStream::new(file);
        let url = self.url_for(key);
        let request = self
            .with_auth(self.client.put(&url))
            .header(
                CONTENT_LENGTH,
                HeaderValue::from_str(&meta.len().to_string()).map_err(|e| e.to_string())?,
            )
            .body(reqwest::Body::wrap_stream(stream));
        let resp = request
            .send()
            .await
            .map_err(|e| format!("WebDAV 上传失败：{}", e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "WebDAV 上传失败，HTTP {}",
                resp.status().as_u16()
            ))
        }
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        let url = self.url_for(key);
        let resp = self
            .with_auth(self.client.delete(&url))
            .send()
            .await
            .map_err(|e| format!("WebDAV 删除失败：{}", e))?;
        if resp.status().is_success() || resp.status().as_u16() == 404 {
            Ok(())
        } else {
            Err(format!(
                "WebDAV 删除失败，HTTP {}",
                resp.status().as_u16()
            ))
        }
    }

    async fn mkdir(&self, key: &str) -> Result<(), String> {
        if key.is_empty() {
            return Ok(());
        }
        // 逐级创建；已存在的段返回 405，按成功处理。
        let mut acc = String::new();
        for seg in key.split('/').filter(|s| !s.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(seg);
            self.mkcol(&self.url_for_dir(&acc)).await?;
        }
        Ok(())
    }

    fn concurrency(&self) -> usize {
        6
    }
}

struct DavItem {
    key: String,
    is_dir: bool,
    size: u64,
    mtime_ms: i64,
}

fn parse_propfind(base_path: &str, xml: &str) -> Vec<DavItem> {
    let re_resp = match Regex::new(r"(?s)<[^:>]*:response[^>]*>(.*?)</[^:>]*:response>") {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let re_href = Regex::new(r"(?s)<[^:>]*:href[^>]*>(.*?)</[^:>]*:href>").unwrap();
    let re_collection = Regex::new(r"<[^:>]*:collection\s*/?>").unwrap();
    let re_size = Regex::new(r"(?s)<[^:>]*:getcontentlength[^>]*>(.*?)</[^:>]*:getcontentlength>").unwrap();
    let re_mtime = Regex::new(r"(?s)<[^:>]*:getlastmodified[^>]*>(.*?)</[^:>]*:getlastmodified>").unwrap();

    let mut out = Vec::new();
    for cap in re_resp.captures_iter(xml) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let href = match re_href.captures(block).and_then(|c| c.get(1)) {
            Some(v) => xml_unescape(v.as_str()).trim().to_string(),
            None => continue,
        };
        let path = if href.starts_with("http://") || href.starts_with("https://") {
            Url::parse(&href).ok().map(|u| u.path().to_string())
        } else if href.starts_with('/') {
            Some(href.clone())
        } else {
            Some(format!("/{}", href))
        };
        let Some(path) = path else { continue };
        let path = urlencoding::decode(&path)
            .map(|v| v.into_owned())
            .unwrap_or(path);
        let base = if base_path.ends_with('/') {
            base_path.to_string()
        } else {
            format!("{}/", base_path)
        };
        if !path.starts_with(&base) {
            continue;
        }
        let key = normalize_key(&path[base.len()..]);
        if key.is_empty() {
            continue;
        }
        let is_dir = re_collection.is_match(block) || href.ends_with('/');
        let size = re_size
            .captures(block)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().trim().parse::<u64>().ok())
            .unwrap_or(0);
        let mtime = re_mtime
            .captures(block)
            .and_then(|c| c.get(1))
            .and_then(|m| parse_http_date(m.as_str().trim()))
            .unwrap_or(0);
        out.push(DavItem {
            key,
            is_dir,
            size,
            mtime_ms: mtime,
        });
    }
    out
}

fn parse_http_date(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc2822(raw)
        .ok()
        .map(|dt| dt.timestamp_millis())
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(raw, "%a, %d %b %Y %H:%M:%S GMT")
                .ok()
                .map(|dt| dt.and_utc().timestamp_millis())
        })
}

fn xml_unescape(raw: &str) -> String {
    raw.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn truncate(raw: &str, max: usize) -> String {
    if raw.len() <= max {
        raw.to_string()
    } else {
        format!("{}…", &raw[..max])
    }
}

// 保留给引擎判断文件是否刚被写入。
#[allow(dead_code)]
fn local_mtime(path: &Path) -> i64 {
    std::fs::metadata(path).map(|m| mtime_ms(&m)).unwrap_or(0)
}
