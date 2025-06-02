use chrono::Utc;
use hex;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256}; // 添加 Digest trait 导入
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, error, info, warn};

#[derive(Debug, Serialize)]
pub struct TranslationResponse {
    pub text: String,
    pub confidence: f32,
    pub engine: String,
}

#[derive(Debug)]
struct TranslationError(String);

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for TranslationError {}

impl From<reqwest::Error> for TranslationError {
    fn from(err: reqwest::Error) -> Self {
        TranslationError(format!("网络请求失败: {}", err))
    }
}

impl From<serde_json::Error> for TranslationError {
    fn from(err: serde_json::Error) -> Self {
        TranslationError(format!("JSON解析失败: {}", err))
    }
}

impl From<TranslationError> for String {
    fn from(err: TranslationError) -> Self {
        err.0
    }
}

// 腾讯翻译响应结构
#[derive(Debug, Deserialize)]
struct TencentResponse {
    Response: TencentResponseInner,
}

#[derive(Debug, Deserialize)]
struct TencentResponseInner {
    TargetText: Vec<String>,
    RequestId: String,
}

fn preprocess_text(text: &str) -> String {
    // 如果是纯图片标签，返回空字符串或占位文本
    if text.trim().starts_with("<img") {
        return "[图片]".to_string();
    }
    text.replace("<", "&lt;").replace(">", "&gt;")
}

#[tauri::command]
pub async fn translate_text(
    text: String,
    secret_id: String,
    secret_key: String,
) -> Result<TranslationResponse, String> {
    info!("开始翻译文本: {}", text);
    let clean_text = preprocess_text(&text);
    info!("预处理后的文本: {}", clean_text);

    type TranslateFn = for<'a> fn(
        &'a str,
        &'a str,
        &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<TranslationResponse, TranslationError>> + Send + 'a>,
    >;

    let services: [TranslateFn; 1] =
        [|text, id, key| Box::pin(try_tencent_translate(text, id, key)) as _];

    for (i, service) in services.iter().enumerate() {
        let service_name = match i {
            0 => "Tencent",
            _ => "Unknown",
        };
        info!("尝试使用 {} 翻译服务", service_name);

        match service(&clean_text, &secret_id, &secret_key).await {
            Ok(result) => {
                info!("{} 翻译成功: {}", service_name, result.text);
                return Ok(result);
            }
            Err(e) => {
                warn!("{} 翻译服务失败: {}", service_name, e);
                debug!("详细错误信息: {:?}", e);
            }
        }
    }

    error!("所有翻译服务均不可用");
    Err("所有翻译服务均不可用".to_string())
}

async fn try_tencent_translate(
    text: &str,
    secret_id: &str,
    secret_key: &str,
) -> Result<TranslationResponse, TranslationError> {
    debug!("尝试腾讯翻译: {}", text);

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| {
            error!("创建腾讯翻译客户端失败: {}", e);
            TranslationError::from(e)
        })?;

    // 准备请求参数
    let service = "tmt";
    let host = "tmt.tencentcloudapi.com";
    let region = "ap-chengdu";
    let version = "2018-03-21";
    let action = "TextTranslate";
    let algorithm = "TC3-HMAC-SHA256";
    let timestamp = Utc::now().timestamp();
    let date = Utc::now().format("%Y-%m-%d").to_string();

    // 构建请求体
    let payload = serde_json::json!({
        "SourceText": text,
        "Source": "auto",
        "Target": "zh",
        "ProjectId": 0
    })
    .to_string();

    // ************* 步骤 1：拼接规范请求串 *************
    let http_request_method = "POST";
    let canonical_uri = "/";
    let canonical_querystring = "";
    let ct = "application/json; charset=utf-8";
    let canonical_headers = format!(
        "content-type:{}\nhost:{}\nx-tc-action:{}\n",
        ct,
        host,
        action.to_lowercase()
    );
    let signed_headers = "content-type;host;x-tc-action";
    let hashed_request_payload = {
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        hex::encode(hasher.finalize())
    };

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        http_request_method,
        canonical_uri,
        canonical_querystring,
        canonical_headers,
        signed_headers,
        hashed_request_payload
    );

    // ************* 步骤 2：拼接待签名字符串 *************
    let credential_scope = format!("{}/{}/{}", date, service, "tc3_request");
    let hashed_canonical_request = {
        let mut hasher = Sha256::new();
        hasher.update(canonical_request.as_bytes());
        hex::encode(hasher.finalize())
    };

    let string_to_sign = format!(
        "{}\n{}\n{}\n{}",
        algorithm, timestamp, credential_scope, hashed_canonical_request
    );

    // ************* 步骤 3：计算签名 *************
    type HmacSha256 = Hmac<Sha256>;

    let secret_date = {
        let mut mac = HmacSha256::new_from_slice(format!("TC3{}", secret_key).as_bytes())
            .map_err(|e| TranslationError(format!("HMAC初始化失败: {}", e)))?;
        mac.update(date.as_bytes());
        mac.finalize().into_bytes()
    };

    let secret_service = {
        let mut mac = HmacSha256::new_from_slice(&secret_date)
            .map_err(|e| TranslationError(format!("HMAC初始化失败: {}", e)))?;
        mac.update(service.as_bytes());
        mac.finalize().into_bytes()
    };

    let secret_signing = {
        let mut mac = HmacSha256::new_from_slice(&secret_service)
            .map_err(|e| TranslationError(format!("HMAC初始化失败: {}", e)))?;
        mac.update("tc3_request".as_bytes());
        mac.finalize().into_bytes()
    };

    let signature = {
        let mut mac = HmacSha256::new_from_slice(&secret_signing)
            .map_err(|e| TranslationError(format!("HMAC初始化失败: {}", e)))?;
        mac.update(string_to_sign.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    };

    // ************* 步骤 4：拼接 Authorization *************
    let authorization = format!(
        "{} Credential={}/{}, SignedHeaders={}, Signature={}",
        algorithm, secret_id, credential_scope, signed_headers, signature
    );

    // ************* 步骤 5：构造并发起请求 *************
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Authorization", authorization.parse().unwrap());
    headers.insert(
        "Content-Type",
        "application/json; charset=utf-8".parse().unwrap(),
    );
    headers.insert("Host", host.parse().unwrap());
    headers.insert("X-TC-Action", action.parse().unwrap());
    headers.insert("X-TC-Timestamp", timestamp.to_string().parse().unwrap());
    headers.insert("X-TC-Version", version.parse().unwrap());
    headers.insert("X-TC-Region", region.parse().unwrap());

    let response = client
        .post(format!("https://{}", host))
        .headers(headers)
        .body(payload)
        .send()
        .await
        .map_err(|e| {
            error!("腾讯翻译请求失败: {}", e);
            TranslationError::from(e)
        })?;

    debug!("腾讯翻译响应状态: {}", response.status());

    let result: TencentResponse = response.json().await.map_err(|e| {
        error!("腾讯翻译响应解析失败: {}", e);
        TranslationError::from(e)
    })?;

    debug!("腾讯翻译原始响应: {:?}", result);

    match result.Response.TargetText.iter().next() {
        Some(translated_text) => Ok(TranslationResponse {
            text: translated_text.clone(), // 克隆字符串
            confidence: 0.9,
            engine: "Tencent".into(),
        }),
        None => {
            error!("腾讯翻译解析失败，原始响应: {:?}", result);
            Err(TranslationError("腾讯翻译解析失败".into()))
        }
    }
}
