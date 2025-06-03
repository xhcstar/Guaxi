use chrono::Utc;
use hex;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

// 批量翻译结果结构
#[derive(Debug, Serialize)]
pub struct TranslationResult {
    pub success: bool,
    pub translated_texts: Vec<String>,
    pub error: Option<String>,
}

// 单条翻译结果结构（兼容旧版）
#[derive(Debug, Serialize)]
pub struct TranslationResponse {
    pub text: String,
    pub confidence: f32,
    pub engine: String,
}

// 自定义错误类型
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

// 腾讯批量翻译响应结构
#[derive(Debug, Deserialize)]
struct TencentBatchResponse {
    Response: TencentBatchResponseInner,
    #[serde(default)]
    Error: Option<TencentError>,
}

#[derive(Debug, Deserialize)]
struct TencentBatchResponseInner {
    #[serde(rename = "TargetTextList", default)]
    target_text_list: Vec<String>,
    #[serde(rename = "RequestId", default)]
    request_id: String,
}

#[derive(Debug, Deserialize)]
struct TencentError {
    #[serde(default)]
    Code: String,
    #[serde(default)]
    Message: String,
}

// 文本预处理
fn preprocess_text(text: &str) -> String {
    if text.trim().starts_with("<img") {
        return "[图片]".to_string();
    }
    text.replace("<", "&lt;").replace(">", "&gt;")
}

// 主翻译接口（批量版）
#[tauri::command]
pub async fn translate_text(
    texts: Vec<String>,
    secret_id: String,
    secret_key: String,
) -> Result<TranslationResult, String> {
    info!("开始批量翻译 {} 条文本", texts.len());
    
    let mut translated_texts = Vec::with_capacity(texts.len());
    let mut last_error = None;
    
    // 分批处理，每批5条
    for chunk in texts.chunks(5) {
        match process_batch(chunk, &secret_id, &secret_key).await {
            Ok(results) => translated_texts.extend(results),
            Err(e) => {
                last_error = Some(e.to_string());
                translated_texts.extend(vec!["[翻译失败]".to_string(); chunk.len()]);
                warn!("批量翻译部分失败: {}", e);
            }
        }
        
        // 控制请求频率 (200ms间隔)
        sleep(Duration::from_millis(200)).await;
    }
    
    Ok(TranslationResult {
        success: last_error.is_none(),
        translated_texts,
        error: last_error,
    })
}

// 定义翻译函数类型
type TranslateFn = for<'a> fn(
    &'a [String],
    &'a str,
    &'a str,
) -> Pin<Box<dyn Future<Output = Result<Vec<String>, TranslationError>> + Send + 'a>>;

// 处理每批翻译
async fn process_batch(
    texts: &[String],
    secret_id: &str,
    secret_key: &str,
) -> Result<Vec<String>, TranslationError> {
    let clean_texts: Vec<String> = texts.iter().map(|t| preprocess_text(t)).collect();
    
    let services: [TranslateFn; 1] = [tencent_translate_service];

    for service in services.iter() {
        match service(&clean_texts, secret_id, secret_key).await {
            Ok(results) => return Ok(results),
            Err(e) => warn!("翻译服务失败: {}", e),
        }
    }
    
    Err(TranslationError("所有翻译服务均不可用".into()))
}

// 腾讯翻译服务包装函数
fn tencent_translate_service<'a>(
    texts: &'a [String],
    secret_id: &'a str,
    secret_key: &'a str,
) -> Pin<Box<dyn Future<Output = Result<Vec<String>, TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        try_tencent_batch_translate(texts, secret_id, secret_key).await
    })
}

// 腾讯批量翻译实现
async fn try_tencent_batch_translate(
    texts: &[String],
    secret_id: &str,
    secret_key: &str,
) -> Result<Vec<String>, TranslationError> {
    debug!("尝试腾讯批量翻译 {} 条文本", texts.len());

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(TranslationError::from)?;

    // 准备请求参数
    let service = "tmt";
    let host = "tmt.tencentcloudapi.com";
    let region = "ap-chengdu";
    let version = "2018-03-21";
    let action = "TextTranslateBatch";
    let algorithm = "TC3-HMAC-SHA256";
    let timestamp = Utc::now().timestamp();
    let date = Utc::now().format("%Y-%m-%d").to_string();

    // 构建批量请求体
    let payload = serde_json::json!({
        "SourceTextList": texts,
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
    let response_text = response.text().await.map_err(|e| {
        error!("获取响应文本失败: {}", e);
        TranslationError::from(e)
    })?;

    debug!("腾讯翻译原始响应: {}", response_text);

    let result: TencentBatchResponse = serde_json::from_str(&response_text).map_err(|e| {
        error!("腾讯翻译响应解析失败: {}", e);
        TranslationError::from(e)
    })?;

    // 处理API错误
    if let Some(err) = result.Error {
        error!("腾讯翻译API错误: {} - {}", err.Code, err.Message);
        return Err(TranslationError(format!("API错误: {}", err.Message)));
    }

    if result.Response.target_text_list.len() == texts.len() {
        info!("返回内容: {:?} ", result.Response.target_text_list);
        Ok(result.Response.target_text_list)
    } else if result.Response.target_text_list.is_empty() {
        error!("腾讯翻译返回空结果");
        Err(TranslationError("翻译服务返回空结果".into()))
    } else {
        error!(
            "返回结果数量不匹配，期望 {} 得到 {}",
            texts.len(),
            result.Response.target_text_list.len()
        );
        Err(TranslationError("返回结果数量不匹配".into()))
    }
}

// 兼容旧版的单条翻译接口
#[tauri::command]
pub async fn translate_single_text(
    text: String,
    secret_id: String,
    secret_key: String,
) -> Result<TranslationResponse, String> {
    let result = translate_text(vec![text], secret_id, secret_key).await?;
    
    match result.translated_texts.first() {
        Some(text) => Ok(TranslationResponse {
            text: text.clone(),
            confidence: 0.9,
            engine: "Tencent".into(),
        }),
        None => Err("翻译结果为空".into()),
    }
}