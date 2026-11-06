use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use hmac::{Hmac, Mac};
use regex::Regex;
use reqwest::header::CONTENT_LENGTH;
use reqwest::{Client, Method};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tracing::info;
use url::Url;

use super::{RemoteEntry, SyncBackend, SyncConnection, encode_key, normalize_key};

type HmacSha256 = Hmac<Sha256>;

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const MULTIPART_THRESHOLD: u64 = 64 * 1024 * 1024;
const PART_SIZE: usize = 16 * 1024 * 1024;

pub struct S3Backend {
    client: Client,
    endpoint: Url,
    bucket: String,
    region: String,
    access_key: String,
    secret_key: String,
    path_style: bool,
    root: String,
}

impl S3Backend {
    pub fn new(connection: &SyncConnection, remote_root: &str) -> Result<Self, String> {
        let raw = connection.s3.endpoint.trim();
        if raw.is_empty() {
            return Err("S3 Endpoint 不能为空".to_string());
        }
        let endpoint_text = if raw.contains("://") {
            raw.to_string()
        } else {
            format!("https://{}", raw)
        };
        let endpoint = Url::parse(&endpoint_text)
            .map_err(|e| format!("S3 Endpoint 非法：{}", e))?;
        if connection.s3.bucket.trim().is_empty() {
            return Err("S3 Bucket 不能为空".to_string());
        }
        if connection.s3.access_key.trim().is_empty() || connection.s3.secret_key.is_empty() {
            return Err("S3 Access Key / Secret Key 不能为空".to_string());
        }
        Ok(Self {
            client: Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|e| format!("创建 S3 客户端失败：{}", e))?,
            endpoint,
            bucket: connection.s3.bucket.trim().to_string(),
            region: if connection.s3.region.trim().is_empty() {
                "auto".to_string()
            } else {
                connection.s3.region.trim().to_string()
            },
            access_key: connection.s3.access_key.trim().to_string(),
            secret_key: connection.s3.secret_key.clone(),
            path_style: connection.s3.path_style,
            root: normalize_key(remote_root),
        })
    }

    fn object_key(&self, key: &str) -> String {
        let key = normalize_key(key);
        if self.root.is_empty() {
            key
        } else if key.is_empty() {
            self.root.clone()
        } else {
            format!("{}/{}", self.root, key)
        }
    }

    fn relative_key(&self, full_key: &str) -> String {
        if self.root.is_empty() {
            full_key.to_string()
        } else {
            full_key
                .strip_prefix(&format!("{}/", self.root))
                .or_else(|| full_key.strip_prefix(&self.root))
                .unwrap_or(full_key)
                .trim_start_matches('/')
                .to_string()
        }
    }

    fn host_and_path(&self, key: &str) -> (String, String) {
        let host = self.endpoint.host_str().unwrap_or("").to_string();
        let port = self.endpoint.port().map(|p| format!(":{}", p)).unwrap_or_default();
        if self.path_style {
            let path = if key.is_empty() {
                format!("/{}", self.bucket)
            } else {
                format!("/{}/{}", self.bucket, encode_key(key))
            };
            (format!("{}{}", host, port), path)
        } else {
            let path = if key.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", encode_key(key))
            };
            (format!("{}.{}{}", self.bucket, host, port), path)
        }
    }

    fn url_for(&self, key: &str, query: &[(String, String)]) -> String {
        let (host, path) = self.host_and_path(key);
        let scheme = self.endpoint.scheme();
        let mut out = format!("{}://{}{}", scheme, host, path);
        if !query.is_empty() {
            let encoded = query
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            out.push('?');
            out.push_str(&encoded);
        }
        out
    }

    async fn request(
        &self,
        method: Method,
        key: &str,
        query: &[(String, String)],
        body: Option<Vec<u8>>,
    ) -> Result<reqwest::Response, String> {
        let (host, canonical_uri) = self.host_and_path(key);
        let mut canonical_query = query
            .iter()
            .map(|(k, v)| (urlencoding::encode(k).into_owned(), urlencoding::encode(v).into_owned()))
            .collect::<Vec<_>>();
        canonical_query.sort();

        let payload_hash = match &body {
            Some(bytes) => hex::encode(Sha256::digest(bytes)),
            None => EMPTY_SHA256.to_string(),
        };
        let now = chrono::Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();

        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            host, payload_hash, amz_date
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.as_str(),
            canonical_uri,
            canonical_query
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&"),
            canonical_headers,
            signed_headers,
            payload_hash
        );

        let scope = format!("{}/{}/s3/aws4_request", date, self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            scope,
            hex::encode(Sha256::digest(canonical_request.as_bytes()))
        );
        let signing_key = self.signing_key(&date);
        let signature = hex::encode(hmac(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key, scope, signed_headers, signature
        );

        let url = self.url_for(key, query);
        let mut request = self
            .client
            .request(method, url)
            .header("host", host)
            .header("x-amz-content-sha256", payload_hash)
            .header("x-amz-date", amz_date)
            .header("authorization", authorization);
        if let Some(bytes) = body {
            request = request.body(bytes);
        }
        request
            .send()
            .await
            .map_err(|e| format!("S3 请求失败：{}", e))
    }

    async fn put_file_multipart(&self, key: &str, local: &Path, size: u64) -> Result<(), String> {
        let object_key = self.object_key(key);
        let init = self
            .request(
                Method::POST,
                &object_key,
                &[("uploads".to_string(), String::new())],
                Some(Vec::new()),
            )
            .await?;
        if !init.status().is_success() {
            let code = init.status().as_u16();
            let text = init.text().await.unwrap_or_default();
            return Err(format!("S3 分片初始化失败，HTTP {}：{}", code, truncate(&text, 200)));
        }
        let xml = init.text().await.map_err(|e| e.to_string())?;
        let upload_id = Regex::new(r"(?s)<UploadId>(.*?)</UploadId>")
            .ok()
            .and_then(|re| re.captures(&xml))
            .and_then(|cap| cap.get(1))
            .map(|m| xml_unescape(m.as_str()))
            .ok_or_else(|| "S3 分片初始化没有返回 UploadId".to_string())?;

        let result = self
            .upload_parts(&object_key, local, size, &upload_id)
            .await;
        if let Err(error) = &result {
            // 任何一片失败都主动取消，避免远端留下一个永远不完整的 multipart。
            let _ = self
                .request(
                    Method::DELETE,
                    &object_key,
                    &[("uploadId".to_string(), upload_id.clone())],
                    None,
                )
                .await;
            return Err(error.clone());
        }
        let completed = result.unwrap();
        let mut body = String::from("<CompleteMultipartUpload>");
        for (number, etag) in completed {
            body.push_str(&format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                number, etag
            ));
        }
        body.push_str("</CompleteMultipartUpload>");

        let response = self
            .request(
                Method::POST,
                &object_key,
                &[("uploadId".to_string(), upload_id)],
                Some(body.into_bytes()),
            )
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            let code = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(format!("S3 分片完成失败，HTTP {}：{}", code, truncate(&text, 200)))
        }
    }

    async fn upload_parts(
        &self,
        object_key: &str,
        local: &Path,
        size: u64,
        upload_id: &str,
    ) -> Result<Vec<(u32, String)>, String> {
        let mut file = tokio::fs::File::open(local)
            .await
            .map_err(|e| format!("读取本地文件失败：{}", e))?;
        let mut parts = Vec::new();
        let mut part_number = 1u32;
        let mut uploaded = 0u64;
        while uploaded < size {
            let want = std::cmp::min(PART_SIZE as u64, size - uploaded) as usize;
            let mut buffer = vec![0u8; want];
            let mut filled = 0usize;
            while filled < want {
                let read = file
                    .read(&mut buffer[filled..])
                    .await
                    .map_err(|e| format!("读取本地文件失败：{}", e))?;
                if read == 0 {
                    break;
                }
                filled += read;
            }
            buffer.truncate(filled);
            if buffer.is_empty() {
                break;
            }
            let response = self
                .request(
                    Method::PUT,
                    object_key,
                    &[
                        ("partNumber".to_string(), part_number.to_string()),
                        ("uploadId".to_string(), upload_id.to_string()),
                    ],
                    Some(buffer),
                )
                .await?;
            if !response.status().is_success() {
                let code = response.status().as_u16();
                let text = response.text().await.unwrap_or_default();
                return Err(format!("S3 分片 {} 上传失败，HTTP {}：{}", part_number, code, truncate(&text, 200)));
            }
            let etag = response
                .headers()
                .get("etag")
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_string())
                .ok_or_else(|| format!("S3 分片 {} 没有返回 ETag", part_number))?;
            parts.push((part_number, etag));
            uploaded += filled as u64;
            part_number += 1;
        }
        Ok(parts)
    }

    fn signing_key(&self, date: &str) -> Vec<u8> {
        let k_date = hmac(format!("AWS4{}", self.secret_key).as_bytes(), date.as_bytes());
        let k_region = hmac(&k_date, self.region.as_bytes());
        let k_service = hmac(&k_region, b"s3");
        hmac(&k_service, b"aws4_request")
    }
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[async_trait]
impl SyncBackend for S3Backend {
    async fn test(&self) -> Result<String, String> {
        let prefix = if self.root.is_empty() {
            String::new()
        } else {
            format!("{}/", self.root)
        };
        let query = vec![
            ("list-type".to_string(), "2".to_string()),
            ("prefix".to_string(), prefix),
            ("max-keys".to_string(), "1".to_string()),
        ];
        let resp = self.request(Method::GET, "", &query, None).await?;
        if resp.status().is_success() {
            Ok("S3 连接正常".to_string())
        } else {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("S3 连接失败，HTTP {}：{}", code, truncate(&text, 200)))
        }
    }

    async fn list(&self) -> Result<Vec<RemoteEntry>, String> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        let prefix = if self.root.is_empty() {
            String::new()
        } else {
            format!("{}/", self.root)
        };
        loop {
            let mut query = vec![
                ("list-type".to_string(), "2".to_string()),
                ("prefix".to_string(), prefix.clone()),
                ("max-keys".to_string(), "1000".to_string()),
            ];
            if let Some(token) = &token {
                query.push(("continuation-token".to_string(), token.clone()));
            }
            let resp = self.request(Method::GET, "", &query, None).await?;
            if !resp.status().is_success() {
                let code = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                return Err(format!("S3 列举失败，HTTP {}：{}", code, truncate(&text, 200)));
            }
            let xml = resp.text().await.map_err(|e| e.to_string())?;
            for item in parse_list_objects(&xml) {
                let key = self.relative_key(&item.key);
                if key.is_empty() || item.key.ends_with('/') {
                    continue;
                }
                out.push(RemoteEntry {
                    key,
                    size: item.size,
                    mtime_ms: item.mtime_ms,
                });
            }
            let truncated = Regex::new(r"(?s)<IsTruncated>\s*true\s*</IsTruncated>")
                .map(|re| re.is_match(&xml))
                .unwrap_or(false);
            token = Regex::new(r"(?s)<NextContinuationToken>(.*?)</NextContinuationToken>")
                .ok()
                .and_then(|re| re.captures(&xml))
                .and_then(|c| c.get(1))
                .map(|m| xml_unescape(m.as_str()).to_string());
            if !truncated || token.is_none() {
                break;
            }
        }
        info!("[Sync][S3] listed {} files", out.len());
        Ok(out)
    }

    async fn stat(&self, key: &str) -> Result<Option<RemoteEntry>, String> {
        let resp = self
            .request(Method::HEAD, &self.object_key(key), &[], None)
            .await?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("S3 HEAD 失败，HTTP {}", resp.status().as_u16()));
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
        let resp = self
            .request(Method::GET, &self.object_key(key), &[], None)
            .await?;
        if !resp.status().is_success() {
            return Err(format!("S3 下载失败，HTTP {}", resp.status().as_u16()));
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
            let chunk = chunk.map_err(|e| format!("读取 S3 响应失败：{}", e))?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn put_file(&self, key: &str, local: &Path) -> Result<(), String> {
        let size = tokio::fs::metadata(local)
            .await
            .map_err(|e| format!("读取本地文件失败：{}", e))?
            .len();
        if size > MULTIPART_THRESHOLD {
            return self.put_file_multipart(key, local, size).await;
        }
        let bytes = tokio::fs::read(local)
            .await
            .map_err(|e| format!("读取本地文件失败：{}", e))?;
        let resp = self
            .request(Method::PUT, &self.object_key(key), &[], Some(bytes))
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let code = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("S3 上传失败，HTTP {}：{}", code, truncate(&text, 200)))
        }
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        let resp = self
            .request(Method::DELETE, &self.object_key(key), &[], None)
            .await?;
        if resp.status().is_success() || resp.status().as_u16() == 404 {
            Ok(())
        } else {
            Err(format!("S3 删除失败，HTTP {}", resp.status().as_u16()))
        }
    }

    async fn mkdir(&self, _key: &str) -> Result<(), String> {
        // 对象存储没有目录，键里的前缀会随对象自动存在。
        Ok(())
    }

    fn concurrency(&self) -> usize {
        6
    }
}

struct ListItem {
    key: String,
    size: u64,
    mtime_ms: i64,
}

fn parse_list_objects(xml: &str) -> Vec<ListItem> {
    let re_content = Regex::new(r"(?s)<Contents>(.*?)</Contents>").unwrap();
    let re_key = Regex::new(r"(?s)<Key>(.*?)</Key>").unwrap();
    let re_size = Regex::new(r"(?s)<Size>(.*?)</Size>").unwrap();
    let re_time = Regex::new(r"(?s)<LastModified>(.*?)</LastModified>").unwrap();
    let mut out = Vec::new();
    for cap in re_content.captures_iter(xml) {
        let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let key = re_key
            .captures(block)
            .and_then(|c| c.get(1))
            .map(|m| xml_unescape(m.as_str()))
            .unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        let size = re_size
            .captures(block)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().trim().parse::<u64>().ok())
            .unwrap_or(0);
        let mtime_ms = re_time
            .captures(block)
            .and_then(|c| c.get(1))
            .and_then(|m| chrono::DateTime::parse_from_rfc3339(m.as_str().trim()).ok())
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);
        out.push(ListItem {
            key,
            size,
            mtime_ms,
        });
    }
    out
}

fn parse_http_date(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc2822(raw)
        .ok()
        .map(|dt| dt.timestamp_millis())
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
