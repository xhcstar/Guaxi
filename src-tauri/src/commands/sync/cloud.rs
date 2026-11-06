use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use md5::{Digest, Md5};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::info;

use super::{RemoteEntry, SyncBackend, SyncConnection, normalize_key};

const BAIDU_API: &str = "https://pan.baidu.com/rest/2.0/xpan";
const BAIDU_PCS: &str = "https://d.pcs.baidu.com/rest/2.0/pcs/superfile2";
const BAIDU_UA: &str = "pan.baidu.com";
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";
const ALIYUN_API: &str = "https://openapi.alipan.com";
const QUARK_API: &str = "https://drive-pc.quark.cn/1/clouddrive";
const BAIDU_BLOCK: usize = 4 * 1024 * 1024;
const ALIYUN_PART: usize = 10 * 1024 * 1024;

/// 百度 / 阿里云盘 / 夸克。百度与阿里走开放平台 token；夸克官方 API 未公开，
/// 先支持 Cookie 模式的浏览与下载，上传会在同步时给出明确错误。
pub struct CloudBackend {
    client: Client,
    provider: String,
    access_token: Mutex<String>,
    refresh_token: String,
    client_id: String,
    client_secret: String,
    drive_id: Mutex<String>,
    app_name: String,
    cookie: String,
    root: String,
    cache: Mutex<HashMap<String, String>>,
}

impl CloudBackend {
    pub fn new(connection: &SyncConnection, remote_root: &str) -> Result<Self, String> {
        let provider = connection.cloud.provider.trim().to_lowercase();
        if !matches!(provider.as_str(), "baidu" | "aliyun" | "quark") {
            return Err(format!("不支持的网盘类型：{}", provider));
        }
        if provider == "quark" && connection.cloud.cookie.trim().is_empty() {
            return Err("夸克 Cookie 不能为空".to_string());
        }
        if provider != "quark"
            && connection.cloud.access_token.trim().is_empty()
            && (connection.cloud.refresh_token.trim().is_empty()
                || connection.cloud.client_id.trim().is_empty()
                || connection.cloud.client_secret.is_empty())
        {
            return Err("请填写 Access Token，或同时填写 Refresh Token / Client ID / Client Secret".to_string());
        }
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(BROWSER_UA));
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| format!("创建网盘客户端失败：{}", e))?;

        let root = if provider == "baidu" {
            let raw = remote_root.trim();
            if raw.is_empty() {
                let app = connection.cloud.app_name.trim();
                if app.is_empty() {
                    "/".to_string()
                } else {
                    format!("/apps/{}", app)
                }
            } else if raw.starts_with('/') {
                raw.to_string()
            } else {
                format!("/{}", raw)
            }
        } else {
            let raw = remote_root.trim();
            if raw.is_empty() {
                "root".to_string()
            } else {
                raw.to_string()
            }
        };

        Ok(Self {
            client,
            provider,
            access_token: Mutex::new(connection.cloud.access_token.trim().to_string()),
            refresh_token: connection.cloud.refresh_token.trim().to_string(),
            client_id: connection.cloud.client_id.trim().to_string(),
            client_secret: connection.cloud.client_secret.clone(),
            drive_id: Mutex::new(connection.cloud.drive_id.trim().to_string()),
            app_name: connection.cloud.app_name.trim().to_string(),
            cookie: connection.cloud.cookie.trim().to_string(),
            root,
            cache: Mutex::new(HashMap::new()),
        })
    }

    async fn token(&self) -> Result<String, String> {
        let current = self.access_token.lock().await.clone();
        if !current.is_empty() {
            return Ok(current);
        }
        if self.refresh_token.is_empty() {
            return Err("Access Token 已失效，请重新授权".to_string());
        }
        let (access, refresh) = match self.provider.as_str() {
            "baidu" => self.refresh_baidu().await?,
            "aliyun" => self.refresh_aliyun().await?,
            _ => return Err("夸克请使用 Cookie".to_string()),
        };
        let mut guard = self.access_token.lock().await;
        *guard = access.clone();
        if !refresh.is_empty() {
            // 只在内存中更新；下次启动会重新用 refresh_token 换。
            let _ = refresh;
        }
        Ok(access)
    }

    async fn refresh_baidu(&self) -> Result<(String, String), String> {
        let resp = self
            .client
            .get("https://openapi.baidu.com/oauth/2.0/token")
            .query(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", self.refresh_token.as_str()),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("百度 token 刷新失败：{}", e))?;
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        let access = value
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("百度 token 刷新失败：{}", value))?;
        let refresh = value
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok((access.to_string(), refresh))
    }

    async fn refresh_aliyun(&self) -> Result<(String, String), String> {
        let resp = self
            .client
            .post(format!("{}/oauth/access_token", ALIYUN_API))
            .json(&json!({
                "grant_type": "refresh_token",
                "refresh_token": self.refresh_token,
                "client_id": self.client_id,
                "client_secret": self.client_secret,
            }))
            .send()
            .await
            .map_err(|e| format!("阿里云盘 token 刷新失败：{}", e))?;
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        let access = value
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("阿里云盘 token 刷新失败：{}", value))?;
        let refresh = value
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok((access.to_string(), refresh))
    }

    async fn ensure_drive_id(&self) -> Result<String, String> {
        let current = self.drive_id.lock().await.clone();
        if !current.is_empty() {
            return Ok(current);
        }
        let token = self.token().await?;
        let resp = self
            .client
            .get(format!("{}/adrive/v1.0/user/getDriveInfo", ALIYUN_API))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("获取阿里云盘 drive_id 失败：{}", e))?;
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        let id = value
            .get("resource_drive_id")
            .or_else(|| value.get("backup_drive_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("无法获取阿里云盘 drive_id：{}", value))?
            .to_string();
        *self.drive_id.lock().await = id.clone();
        Ok(id)
    }

    fn baidu_abs(&self, key: &str) -> String {
        let key = normalize_key(key);
        if key.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{}", self.root.trim_end_matches('/'), key)
        }
    }

    async fn baidu_list_raw(&self, dir: &str) -> Result<Vec<Value>, String> {
        let token = self.token().await?;
        let mut start = 0u32;
        let mut out = Vec::new();
        loop {
            let resp = self
                .client
                .get(format!("{}/file", BAIDU_API))
                .header(USER_AGENT, BAIDU_UA)
                .query(&[
                    ("method", "list"),
                    ("access_token", token.as_str()),
                    ("dir", dir),
                    ("start", &start.to_string()),
                    ("limit", "1000"),
                    ("order", "name"),
                ])
                .send()
                .await
                .map_err(|e| format!("百度网盘列目录失败：{}", e))?;
            let value: Value = resp.json().await.map_err(|e| e.to_string())?;
            let errno = value.get("errno").and_then(|v| v.as_i64()).unwrap_or(0);
            if errno != 0 {
                return Err(format!("百度网盘列目录失败：{}", value));
            }
            let list = value
                .get("list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let count = list.len();
            out.extend(list);
            if count < 1000 {
                break;
            }
            start += count as u32;
        }
        Ok(out)
    }

    async fn baidu_walk(&self) -> Result<Vec<(String, u64, i64, bool, String)>, String> {
        let mut out = Vec::new();
        let mut queue = vec![self.root.clone()];
        let mut seen = std::collections::HashSet::new();
        while let Some(dir) = queue.pop() {
            if !seen.insert(dir.clone()) {
                continue;
            }
            for item in self.baidu_list_raw(&dir).await? {
                let path = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let name = item
                    .get("server_filename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if path.is_empty() || name.is_empty() {
                    continue;
                }
                let is_dir = item.get("isdir").and_then(|v| v.as_i64()).unwrap_or(0) == 1;
                let relative = path
                    .strip_prefix(self.root.trim_end_matches('/'))
                    .unwrap_or(path)
                    .trim_start_matches('/')
                    .to_string();
                if relative.is_empty() {
                    continue;
                }
                let fs_id = item.get("fs_id").and_then(|v| v.as_i64()).unwrap_or(0).to_string();
                if !fs_id.is_empty() {
                    self.cache.lock().await.insert(relative.clone(), fs_id.clone());
                }
                if is_dir {
                    queue.push(path.to_string());
                } else {
                    let size = item.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let mtime = item
                        .get("server_mtime")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                        * 1000;
                    out.push((relative, size, mtime, false, fs_id));
                }
            }
        }
        Ok(out)
    }

    async fn baidu_resolve(&self, key: &str) -> Result<String, String> {
        let key = normalize_key(key);
        if let Some(id) = self.cache.lock().await.get(&key) {
            return Ok(id.clone());
        }
        self.baidu_walk().await?;
        self.cache
            .lock()
            .await
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("百度网盘中不存在：{}", key))
    }

    async fn aliyun_list_raw(&self, parent: &str) -> Result<Vec<Value>, String> {
        let token = self.token().await?;
        let drive = self.ensure_drive_id().await?;
        let mut marker = String::new();
        let mut out = Vec::new();
        loop {
            let mut body = json!({
                "drive_id": drive,
                "parent_file_id": parent,
                "limit": 100,
                "order_by": "name",
                "order_direction": "ASC",
            });
            if !marker.is_empty() {
                body["marker"] = json!(marker);
            }
            let resp = self
                .client
                .post(format!("{}/adrive/v1.0/openFile/list", ALIYUN_API))
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("阿里云盘列目录失败：{}", e))?;
            let ok = resp.status().is_success();
            let value: Value = resp.json().await.map_err(|e| e.to_string())?;
            if !ok {
                return Err(format!("阿里云盘列目录失败：{}", value));
            }
            let items = value
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            out.extend(items);
            marker = value
                .get("next_marker")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if marker.is_empty() {
                break;
            }
        }
        Ok(out)
    }

    async fn aliyun_walk(&self) -> Result<Vec<(String, u64, i64, bool, String)>, String> {
        let mut out = Vec::new();
        let mut queue = vec![(self.root.clone(), String::new())];
        let mut seen = std::collections::HashSet::new();
        while let Some((parent, prefix)) = queue.pop() {
            if !seen.insert(parent.clone()) {
                continue;
            }
            for item in self.aliyun_list_raw(&parent).await? {
                let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let id = item.get("file_id").and_then(|v| v.as_str()).unwrap_or("");
                if name.is_empty() || id.is_empty() {
                    continue;
                }
                let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("file");
                let relative = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", prefix, name)
                };
                self.cache.lock().await.insert(relative.clone(), id.to_string());
                if kind == "folder" {
                    queue.push((id.to_string(), relative));
                } else {
                    let size = item.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let mtime = item
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                        .map(|v| v.timestamp_millis())
                        .unwrap_or(0);
                    out.push((relative, size, mtime, false, id.to_string()));
                }
            }
        }
        Ok(out)
    }

    async fn aliyun_resolve(&self, key: &str) -> Result<String, String> {
        let key = normalize_key(key);
        if let Some(id) = self.cache.lock().await.get(&key) {
            return Ok(id.clone());
        }
        self.aliyun_walk().await?;
        self.cache
            .lock()
            .await
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("阿里云盘中不存在：{}", key))
    }

    async fn quark_list_raw(&self, parent: &str) -> Result<Vec<Value>, String> {
        let mut page = 1u32;
        let mut out = Vec::new();
        loop {
            let resp = self
                .client
                .get(format!("{}/file/sort", QUARK_API))
                .query(&[
                    ("pr", "ucpro"),
                    ("fr", "pc"),
                    ("pdir_fid", parent),
                    ("_page", &page.to_string()),
                    ("_size", "100"),
                    ("_sort", "file_type:asc,updated_at:desc"),
                ])
                .header("cookie", self.cookie.as_str())
                .header("referer", "https://pan.quark.cn/")
                .send()
                .await
                .map_err(|e| format!("夸克列目录失败：{}", e))?;
            let value: Value = resp.json().await.map_err(|e| e.to_string())?;
            if value.get("status").and_then(|v| v.as_i64()).unwrap_or(0) != 200 {
                return Err(format!("夸克列目录失败：{}", value));
            }
            let list = value
                .pointer("/data/list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let count = list.len();
            out.extend(list);
            if count < 100 {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    async fn quark_walk(&self) -> Result<Vec<(String, u64, i64, bool, String)>, String> {
        let mut out = Vec::new();
        let mut queue = vec![("0".to_string(), String::new())];
        let mut seen = std::collections::HashSet::new();
        while let Some((parent, prefix)) = queue.pop() {
            if !seen.insert(parent.clone()) {
                continue;
            }
            for item in self.quark_list_raw(&parent).await? {
                let name = item.get("file_name").and_then(|v| v.as_str()).unwrap_or("");
                let id = item.get("fid").and_then(|v| v.as_str()).unwrap_or("");
                if name.is_empty() || id.is_empty() {
                    continue;
                }
                let is_dir = item.get("dir").and_then(|v| v.as_bool()).unwrap_or(false);
                let relative = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", prefix, name)
                };
                self.cache.lock().await.insert(relative.clone(), id.to_string());
                if is_dir {
                    queue.push((id.to_string(), relative));
                } else {
                    let size = item.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let mtime = item
                        .get("updated_at")
                        .and_then(|v| v.as_i64())
                        .map(|v| if v < 10_000_000_000 { v * 1000 } else { v })
                        .unwrap_or(0);
                    out.push((relative, size, mtime, false, id.to_string()));
                }
            }
        }
        Ok(out)
    }

    async fn quark_resolve(&self, key: &str) -> Result<String, String> {
        let key = normalize_key(key);
        if let Some(id) = self.cache.lock().await.get(&key) {
            return Ok(id.clone());
        }
        self.quark_walk().await?;
        self.cache
            .lock()
            .await
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("夸克网盘中不存在：{}", key))
    }
}

#[async_trait]
impl SyncBackend for CloudBackend {
    async fn test(&self) -> Result<String, String> {
        match self.provider.as_str() {
            "baidu" => {
                self.baidu_list_raw(&self.root).await?;
                Ok("百度网盘连接正常".to_string())
            }
            "aliyun" => {
                self.ensure_drive_id().await?;
                self.aliyun_list_raw(&self.root).await?;
                Ok("阿里云盘连接正常".to_string())
            }
            "quark" => {
                self.quark_list_raw("0").await?;
                Ok("夸克网盘连接正常（Cookie 模式）".to_string())
            }
            _ => Err("未知的网盘类型".to_string()),
        }
    }

    async fn list(&self) -> Result<Vec<RemoteEntry>, String> {
        let items = match self.provider.as_str() {
            "baidu" => self.baidu_walk().await?,
            "aliyun" => self.aliyun_walk().await?,
            "quark" => self.quark_walk().await?,
            _ => return Err("未知的网盘类型".to_string()),
        };
        Ok(items
            .into_iter()
            .map(|(key, size, mtime_ms, _is_dir, _id)| RemoteEntry {
                key,
                size,
                mtime_ms,
            })
            .collect())
    }

    async fn stat(&self, key: &str) -> Result<Option<RemoteEntry>, String> {
        let key = normalize_key(key);
        let cached = self.cache.lock().await.get(&key).cloned();
        match (self.provider.as_str(), cached) {
            ("baidu", Some(fs_id)) => {
                let token = self.token().await?;
                let resp = self
                    .client
                    .get(format!("{}/multimedia", BAIDU_API))
                    .header(USER_AGENT, BAIDU_UA)
                    .query(&[
                        ("method", "filemetas"),
                        ("access_token", token.as_str()),
                        ("fsids", &format!("[{}]", fs_id)),
                    ])
                    .send()
                    .await
                    .map_err(|e| format!("百度读取文件属性失败：{}", e))?;
                let value: Value = resp.json().await.map_err(|e| e.to_string())?;
                Ok(value.pointer("/list/0").map(|item| RemoteEntry {
                    key: key.clone(),
                    size: item.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                    mtime_ms: item
                        .get("server_mtime")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                        * 1000,
                }))
            }
            ("aliyun", Some(file_id)) => {
                let token = self.token().await?;
                let drive = self.ensure_drive_id().await?;
                let resp = self
                    .client
                    .post(format!("{}/adrive/v1.0/openFile/get", ALIYUN_API))
                    .bearer_auth(&token)
                    .json(&json!({"drive_id": drive, "file_id": file_id}))
                    .send()
                    .await
                    .map_err(|e| format!("阿里云盘读取文件属性失败：{}", e))?;
                if !resp.status().is_success() {
                    return Ok(None);
                }
                let value: Value = resp.json().await.map_err(|e| e.to_string())?;
                Ok(Some(RemoteEntry {
                    key: key.clone(),
                    size: value.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                    mtime_ms: value
                        .get("updated_at")
                        .and_then(|v| v.as_str())
                        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                        .map(|v| v.timestamp_millis())
                        .unwrap_or(0),
                }))
            }
            _ => {
                let items = match self.provider.as_str() {
                    "baidu" => self.baidu_walk().await?,
                    "aliyun" => self.aliyun_walk().await?,
                    "quark" => self.quark_walk().await?,
                    _ => return Err("未知的网盘类型".to_string()),
                };
                Ok(items
                    .into_iter()
                    .find(|(item_key, _, _, _, _)| *item_key == key)
                    .map(|(key, size, mtime_ms, _, _)| RemoteEntry {
                        key,
                        size,
                        mtime_ms,
                    }))
            }
        }
    }

    async fn get_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let url = match self.provider.as_str() {
            "baidu" => {
                let token = self.token().await?;
                let fs_id = self.baidu_resolve(key).await?;
                let resp = self
                    .client
                    .get(format!("{}/multimedia", BAIDU_API))
                    .header(USER_AGENT, BAIDU_UA)
                    .query(&[
                        ("method", "filemetas"),
                        ("access_token", token.as_str()),
                        ("fsids", &format!("[{}]", fs_id)),
                        ("dlink", "1"),
                    ])
                    .send()
                    .await
                    .map_err(|e| format!("百度获取下载地址失败：{}", e))?;
                let value: Value = resp.json().await.map_err(|e| e.to_string())?;
                let link = value
                    .pointer("/list/0/dlink")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("百度未返回下载地址：{}", value))?;
                format!("{}&access_token={}", link, token)
            }
            "aliyun" => {
                let token = self.token().await?;
                let drive = self.ensure_drive_id().await?;
                let file_id = self.aliyun_resolve(key).await?;
                let resp = self
                    .client
                    .post(format!("{}/adrive/v1.0/openFile/getDownloadUrl", ALIYUN_API))
                    .bearer_auth(&token)
                    .json(&json!({"drive_id": drive, "file_id": file_id, "expire_sec": 14400}))
                    .send()
                    .await
                    .map_err(|e| format!("阿里云盘获取下载地址失败：{}", e))?;
                let value: Value = resp.json().await.map_err(|e| e.to_string())?;
                value
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("阿里云盘未返回下载地址：{}", value))?
                    .to_string()
            }
            "quark" => {
                let fid = self.quark_resolve(key).await?;
                let resp = self
                    .client
                    .post(format!("{}/file/download", QUARK_API))
                    .query(&[("pr", "ucpro"), ("fr", "pc")])
                    .header("cookie", self.cookie.as_str())
                    .json(&json!({"fids": [fid]}))
                    .send()
                    .await
                    .map_err(|e| format!("夸克获取下载地址失败：{}", e))?;
                let value: Value = resp.json().await.map_err(|e| e.to_string())?;
                value
                    .pointer("/data/0/download_url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("夸克未返回下载地址：{}", value))?
                    .to_string()
            }
            _ => return Err("未知的网盘类型".to_string()),
        };

        let mut request = self.client.get(&url);
        if self.provider == "baidu" {
            request = request.header(USER_AGENT, BAIDU_UA);
        } else if self.provider == "quark" {
            request = request
                .header("cookie", self.cookie.as_str())
                .header("referer", "https://pan.quark.cn/");
        }
        let resp = request
            .send()
            .await
            .map_err(|e| format!("网盘下载失败：{}", e))?;
        if !resp.status().is_success() {
            return Err(format!("网盘下载失败，HTTP {}", resp.status().as_u16()));
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
            let chunk = chunk.map_err(|e| format!("读取网盘响应失败：{}", e))?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn put_file(&self, key: &str, local: &Path) -> Result<(), String> {
        match self.provider.as_str() {
            "baidu" => self.put_baidu(key, local).await,
            "aliyun" => self.put_aliyun(key, local).await,
            "quark" => Err("夸克官方上传 API 尚未公开，请改用 WebDAV / 同步盘目录".to_string()),
            _ => Err("未知的网盘类型".to_string()),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        match self.provider.as_str() {
            "baidu" => {
                let token = self.token().await?;
                let path = self.baidu_abs(key);
                let resp = self
                    .client
                    .post(format!("{}/file?method=delete&access_token={}", BAIDU_API, token))
                    .header(USER_AGENT, BAIDU_UA)
                    .form(&[("async", "1"), ("filelist", &format!("[\"{}\"]", path))])
                    .send()
                    .await
                    .map_err(|e| format!("百度删除失败：{}", e))?;
                if resp.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("百度删除失败，HTTP {}", resp.status().as_u16()))
                }
            }
            "aliyun" => {
                let token = self.token().await?;
                let drive = self.ensure_drive_id().await?;
                let file_id = self.aliyun_resolve(key).await?;
                let resp = self
                    .client
                    .post(format!("{}/adrive/v1.0/openFile/recyclebin/trash", ALIYUN_API))
                    .bearer_auth(&token)
                    .json(&json!({"drive_id": drive, "file_id": file_id}))
                    .send()
                    .await
                    .map_err(|e| format!("阿里云盘删除失败：{}", e))?;
                if resp.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("阿里云盘删除失败，HTTP {}", resp.status().as_u16()))
                }
            }
            "quark" => {
                let fid = self.quark_resolve(key).await?;
                let resp = self
                    .client
                    .post(format!("{}/file/delete", QUARK_API))
                    .query(&[("pr", "ucpro"), ("fr", "pc")])
                    .header("cookie", self.cookie.as_str())
                    .json(&json!({"fids": [fid]}))
                    .send()
                    .await
                    .map_err(|e| format!("夸克删除失败：{}", e))?;
                if resp.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("夸克删除失败，HTTP {}", resp.status().as_u16()))
                }
            }
            _ => Err("未知的网盘类型".to_string()),
        }
    }

    async fn mkdir(&self, key: &str) -> Result<(), String> {
        let key = normalize_key(key);
        if key.is_empty() {
            return Ok(());
        }
        if self.provider == "aliyun" {
            return self.aliyun_mkdir(&key).await;
        }
        if self.provider == "quark" {
            return self.quark_mkdir(&key).await;
        }
        let token = self.token().await?;
        let mut acc = String::new();
        for segment in key.split('/').filter(|s| !s.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(segment);
            let path = self.baidu_abs(&acc);
            let _ = self
                .client
                .post(format!("{}/file?method=create&access_token={}", BAIDU_API, token))
                .header(USER_AGENT, BAIDU_UA)
                .form(&[
                    ("path", path.as_str()),
                    ("size", "0"),
                    ("isdir", "1"),
                    ("rtype", "3"),
                ])
                .send()
                .await;
        }
        Ok(())
    }

    fn concurrency(&self) -> usize {
        4
    }
}

impl CloudBackend {
    async fn aliyun_mkdir(&self, key: &str) -> Result<(), String> {
        let token = self.token().await?;
        let drive = self.ensure_drive_id().await?;
        let mut parent = self.root.clone();
        let mut acc = String::new();
        for segment in key.split('/').filter(|s| !s.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(segment);
            if let Some(id) = self.cache.lock().await.get(&acc).cloned() {
                parent = id;
                continue;
            }
            let items = self.aliyun_list_raw(&parent).await?;
            if let Some(item) = items.iter().find(|item| {
                item.get("name").and_then(|v| v.as_str()) == Some(segment)
                    && item.get("type").and_then(|v| v.as_str()) == Some("folder")
            }) {
                if let Some(id) = item.get("file_id").and_then(|v| v.as_str()) {
                    self.cache.lock().await.insert(acc.clone(), id.to_string());
                    parent = id.to_string();
                    continue;
                }
            }
            let resp = self
                .client
                .post(format!("{}/adrive/v1.0/openFile/create", ALIYUN_API))
                .bearer_auth(&token)
                .json(&json!({
                    "drive_id": drive,
                    "parent_file_id": parent,
                    "name": segment,
                    "type": "folder",
                    "check_name_mode": "refuse",
                }))
                .send()
                .await
                .map_err(|e| format!("阿里云盘创建目录失败：{}", e))?;
            let value: Value = resp.json().await.map_err(|e| e.to_string())?;
            let created = value
                .get("file_id")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            if let Some(id) = created {
                self.cache.lock().await.insert(acc.clone(), id.clone());
                parent = id;
            } else {
                // 目录已存在时 create 会报错；回列表里取一次 id。
                let items = self.aliyun_list_raw(&parent).await?;
                if let Some(id) = items.iter().find_map(|item| {
                    (item.get("name").and_then(|v| v.as_str()) == Some(segment)
                        && item.get("type").and_then(|v| v.as_str()) == Some("folder"))
                    .then(|| item.get("file_id").and_then(|v| v.as_str()))
                    .flatten()
                }) {
                    self.cache.lock().await.insert(acc.clone(), id.to_string());
                    parent = id.to_string();
                } else {
                    return Err(format!("阿里云盘创建目录失败：{}", value));
                }
            }
        }
        Ok(())
    }

    async fn quark_mkdir(&self, key: &str) -> Result<(), String> {
        let mut parent = "0".to_string();
        let mut acc = String::new();
        for segment in key.split('/').filter(|s| !s.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(segment);
            if let Some(id) = self.cache.lock().await.get(&acc).cloned() {
                parent = id;
                continue;
            }
            let items = self.quark_list_raw(&parent).await?;
            if let Some(id) = items.iter().find_map(|item| {
                (item.get("file_name").and_then(|v| v.as_str()) == Some(segment)
                    && item.get("dir").and_then(|v| v.as_bool()).unwrap_or(false))
                .then(|| item.get("fid").and_then(|v| v.as_str()))
                .flatten()
            }) {
                self.cache.lock().await.insert(acc.clone(), id.to_string());
                parent = id.to_string();
                continue;
            }
            let resp = self
                .client
                .post(format!("{}/file", QUARK_API))
                .query(&[("pr", "ucpro"), ("fr", "pc")])
                .header("cookie", self.cookie.as_str())
                .json(&json!({
                    "pdir_fid": parent,
                    "file_name": segment,
                    "dir_path": "",
                    "dir_init_lock": false,
                }))
                .send()
                .await
                .map_err(|e| format!("夸克创建目录失败：{}", e))?;
            let value: Value = resp.json().await.map_err(|e| e.to_string())?;
            let created = value
                .pointer("/data/fid")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            if let Some(id) = created {
                self.cache.lock().await.insert(acc.clone(), id.clone());
                parent = id;
            } else {
                return Err(format!("夸克创建目录失败：{}", value));
            }
        }
        Ok(())
    }

    async fn put_baidu(&self, key: &str, local: &Path) -> Result<(), String> {
        let token = self.token().await?;
        let path = self.baidu_abs(key);
        let parent_key = normalize_key(key)
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_string())
            .unwrap_or_default();
        if !parent_key.is_empty() {
            self.mkdir(&parent_key).await?;
        }
        let bytes = tokio::fs::read(local)
            .await
            .map_err(|e| format!("读取本地文件失败：{}", e))?;
        if bytes.is_empty() {
            return Err("不支持上传空文件".to_string());
        }
        let mut blocks = Vec::new();
        for chunk in bytes.chunks(BAIDU_BLOCK) {
            blocks.push(hex::encode(Md5::digest(chunk)));
        }
        let block_json = serde_json::to_string(&blocks).map_err(|e| e.to_string())?;
        let resp = self
            .client
            .post(format!("{}/file?method=precreate&access_token={}", BAIDU_API, token))
            .header(USER_AGENT, BAIDU_UA)
            .form(&[
                ("path", path.as_str()),
                ("size", &bytes.len().to_string()),
                ("isdir", "0"),
                ("autoinit", "1"),
                ("block_list", block_json.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("百度预上传失败：{}", e))?;
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        let errno = value.get("errno").and_then(|v| v.as_i64()).unwrap_or(0);
        if errno != 0 {
            return Err(format!("百度预上传失败：{}", value));
        }
        let upload_id = value
            .get("uploadid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("百度预上传未返回 uploadid：{}", value))?;
        let missing: Vec<usize> = value
            .get("block_list")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_u64().map(|n| n as usize))
                    .collect()
            })
            .unwrap_or_else(|| (0..blocks.len()).collect());
        for index in missing {
            let start = index * BAIDU_BLOCK;
            let end = std::cmp::min(start + BAIDU_BLOCK, bytes.len());
            let part = bytes[start..end].to_vec();
            let form = reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(part).file_name("blob"),
            );
            let resp = self
                .client
                .post(BAIDU_PCS)
                .header(USER_AGENT, BAIDU_UA)
                .query(&[
                    ("method", "upload"),
                    ("access_token", token.as_str()),
                    ("type", "tmpfile"),
                    ("path", path.as_str()),
                    ("uploadid", upload_id),
                    ("partseq", &index.to_string()),
                ])
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("百度分片上传失败：{}", e))?;
            if !resp.status().is_success() {
                return Err(format!("百度分片上传失败，HTTP {}", resp.status().as_u16()));
            }
        }
        let resp = self
            .client
            .post(format!("{}/file?method=create&access_token={}", BAIDU_API, token))
            .header(USER_AGENT, BAIDU_UA)
            .form(&[
                ("path", path.as_str()),
                ("size", &bytes.len().to_string()),
                ("isdir", "0"),
                ("rtype", "3"),
                ("block_list", block_json.as_str()),
                ("uploadid", upload_id),
            ])
            .send()
            .await
            .map_err(|e| format!("百度创建文件失败：{}", e))?;
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        if value.get("errno").and_then(|v| v.as_i64()).unwrap_or(0) != 0 {
            return Err(format!("百度创建文件失败：{}", value));
        }
        info!(key, "[Sync][Baidu] uploaded");
        Ok(())
    }

    async fn put_aliyun(&self, key: &str, local: &Path) -> Result<(), String> {
        let token = self.token().await?;
        let drive = self.ensure_drive_id().await?;
        let key = normalize_key(key);
        let name = key.rsplit('/').next().unwrap_or("file");
        let parent_key = key.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        self.mkdir(parent_key).await?;
        let parent_id = if parent_key.is_empty() {
            self.root.clone()
        } else {
            self.aliyun_resolve(parent_key).await?
        };
        let bytes = tokio::fs::read(local)
            .await
            .map_err(|e| format!("读取本地文件失败：{}", e))?;
        let part_count = ((bytes.len() + ALIYUN_PART - 1) / ALIYUN_PART).max(1);
        let parts: Vec<Value> = (1..=part_count)
            .map(|number| json!({"part_number": number}))
            .collect();
        let resp = self
            .client
            .post(format!("{}/adrive/v1.0/openFile/create", ALIYUN_API))
            .bearer_auth(&token)
            .json(&json!({
                "drive_id": drive,
                "parent_file_id": parent_id,
                "name": name,
                "type": "file",
                "size": bytes.len(),
                "check_name_mode": "overwrite",
                "part_info_list": parts,
            }))
            .send()
            .await
            .map_err(|e| format!("阿里云盘创建上传任务失败：{}", e))?;
        let ok = resp.status().is_success();
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        if !ok {
            return Err(format!("阿里云盘创建上传任务失败：{}", value));
        }
        if value.get("rapid_upload").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(());
        }
        let upload_id = value
            .get("upload_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("阿里云盘未返回 upload_id：{}", value))?;
        let file_id = value
            .get("file_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("阿里云盘未返回 file_id：{}", value))?;
        let urls = value
            .get("part_info_list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for (index, url) in urls.iter().enumerate() {
            let upload_url = url
                .get("upload_url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "阿里云盘分片缺少 upload_url".to_string())?;
            let start = index * ALIYUN_PART;
            let end = std::cmp::min(start + ALIYUN_PART, bytes.len());
            let chunk = bytes[start..end].to_vec();
            let resp = self
                .client
                .put(upload_url)
                .header("content-length", chunk.len().to_string())
                .body(chunk)
                .send()
                .await
                .map_err(|e| format!("阿里云盘分片上传失败：{}", e))?;
            if !resp.status().is_success() {
                return Err(format!("阿里云盘分片上传失败，HTTP {}", resp.status().as_u16()));
            }
        }
        let resp = self
            .client
            .post(format!("{}/adrive/v1.0/openFile/complete", ALIYUN_API))
            .bearer_auth(&token)
            .json(&json!({"drive_id": drive, "file_id": file_id, "upload_id": upload_id}))
            .send()
            .await
            .map_err(|e| format!("阿里云盘完成上传失败：{}", e))?;
        if !resp.status().is_success() {
            return Err(format!("阿里云盘完成上传失败，HTTP {}", resp.status().as_u16()));
        }
        info!(key, "[Sync][Aliyun] uploaded");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::sync::CloudConnection;

    fn connection(provider: &str) -> SyncConnection {
        SyncConnection {
            kind: "cloud".to_string(),
            cloud: CloudConnection {
                provider: provider.to_string(),
                access_token: "token".to_string(),
                app_name: "Guaxi".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn unknown_provider_is_rejected() {
        assert!(CloudBackend::new(&connection("dropbox"), "").is_err());
    }

    #[test]
    fn baidu_default_root_is_app_directory() {
        let backend = CloudBackend::new(&connection("baidu"), "").expect("backend");
        assert_eq!(backend.root, "/apps/Guaxi");
        assert_eq!(backend.baidu_abs("images/a.png"), "/apps/Guaxi/images/a.png");
    }

    #[test]
    fn aliyun_default_root_is_drive_root() {
        let backend = CloudBackend::new(&connection("aliyun"), "").expect("backend");
        assert_eq!(backend.root, "root");
    }

    #[test]
    fn quark_requires_cookie() {
        let mut conn = connection("quark");
        conn.cloud.cookie = String::new();
        assert!(CloudBackend::new(&conn, "").is_err());
        conn.cloud.cookie = "a=1; b=2".to_string();
        assert!(CloudBackend::new(&conn, "").is_ok());
    }
}
