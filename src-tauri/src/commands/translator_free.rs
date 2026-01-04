use chrono::Utc;
use reqwest::Client;
use serde::Serialize;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use uuid::Uuid;

// 1. 定义数据结构
#[derive(Debug, Serialize, Clone)]
pub struct TranslationResult {
    pub success: bool,
    pub translated_texts: Vec<String>,
    pub error: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct TranslationResponse {
    pub text: String,
    pub confidence: f32,
    pub engine: String,
}

#[derive(Debug, Clone)]
struct TranslationError(String);

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for TranslationError {}

impl From<reqwest::Error> for TranslationError {
    fn from(err: reqwest::Error) -> Self {
        TranslationError(format!("Network error: {}", err))
    }
}

impl From<serde_json::Error> for TranslationError {
    fn from(err: serde_json::Error) -> Self {
        TranslationError(format!("JSON error: {}", err))
    }
}

// 2. 主翻译接口
#[tauri::command]
pub async fn translate_text_free(
    texts: Vec<String>,
) -> Result<TranslationResult, String> {
    info!("Translating {} texts", texts.len());
    
    let mut translated_texts = Vec::with_capacity(texts.len());
    let mut last_error = None;
    
    // 分批处理，每批5条
    for chunk in texts.chunks(5) {
        match process_batch(chunk).await {
            Ok((results, _engine)) => translated_texts.extend(results),
            Err(e) => {
                let error_str = e.to_string();
                last_error = Some(error_str.clone());
                warn!("Translation service failed: {}", error_str);
                translated_texts.extend(vec!["[Translation Failed]".to_string(); chunk.len()]);
            }
        }
        
        // 控制请求频率
        sleep(Duration::from_millis(200)).await;
    }
    
    Ok(TranslationResult {
        success: last_error.is_none(),
        translated_texts,
        error: last_error,
    })
}

// 3. 定义翻译函数类型
type TranslateFn = for<'a> fn(
    &'a [String],
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>>;

// 4. 处理每批翻译
async fn process_batch(
    texts: &[String],
) -> Result<(Vec<String>, String), TranslationError> {
    let clean_texts: Vec<String> = texts.iter().map(|t| preprocess_text(t)).collect();
    
    // 定义所有翻译服务（按优先级排序）
    let services: [TranslateFn; 3] = [
        tencent_translate_service,
        google_translate_service,
        bing_translate_service,
    ];

    let mut last_error = None;
    
    for service in services.iter() {
        match service(&clean_texts).await {
            Ok((results, engine)) => return Ok((results, engine)),
            Err(e) => {
                last_error = Some(e.clone());
                warn!("Translation service failed: {}", e);
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| TranslationError("All translation services failed".into())))
}

// 5. 文本预处理
fn preprocess_text(text: &str) -> String {
    if text.trim().starts_with("<img") {
        return "[Image]".to_string();
    }
    text.replace("<", "&lt;").replace(">", "&gt;")
}

// 6. 腾讯翻译实现
fn tencent_translate_service<'a>(
    texts: &'a [String],
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        let client = Client::builder()
            .user_agent("Mozilla/5.0")
            .build()
            .map_err(TranslationError::from)?;

        let mut translated_texts = Vec::with_capacity(texts.len());
        
        for text in texts {
            let response = client
                .post("https://fanyi.qq.com/api/translate")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .form(&[
                    ("sourceText", text.as_str()),
                    ("source", "auto"),
                    ("target", "zh"),
                    ("qtk", &generate_qtk()),
                    ("qtv", &generate_qtv()),
                ])
                .send()
                .await
                .map_err(TranslationError::from)?;

            let response_text = response.text().await.map_err(TranslationError::from)?;
            let result: serde_json::Value = serde_json::from_str(&response_text)?;

            if let Some(records) = result["translate"]["records"].as_array() {
                if !records.is_empty() {
                    translated_texts.push(records[0]["targetText"].as_str().unwrap_or("").to_string());
                    continue;
                }
            }
            return Err(TranslationError("Tencent translation failed".into()));
        }
        
        Ok((translated_texts, "Tencent".into()))
    })
}

// 8. 谷歌翻译实现
fn google_translate_service<'a>(
    texts: &'a [String],
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        let client = Client::builder()
            .user_agent("Mozilla/5.0")
            .build()
            .map_err(TranslationError::from)?;

        let mut translated_texts = Vec::with_capacity(texts.len());
        
        for text in texts {
            let response = client
                .get("https://translate.googleapis.com/translate_a/single")
                .query(&[
                    ("client", "gtx"),
                    ("sl", "auto"),
                    ("tl", "zh-CN"),
                    ("dt", "t"),
                    ("q", text.as_str()),
                ])
                .send()
                .await
                .map_err(TranslationError::from)?;

            let response_text = response.text().await.map_err(TranslationError::from)?;
            let result: serde_json::Value = serde_json::from_str(&response_text)?;

            if let Some(translation) = result[0].as_array() {
                let mut translated = String::new();
                for part in translation {
                    if let Some(text) = part[0].as_str() {
                        translated.push_str(text);
                    }
                }
                translated_texts.push(translated);
            } else {
                return Err(TranslationError("Google translation failed".into()));
            }
        }
        
        Ok((translated_texts, "Google".into()))
    })
}

// 9. 必应翻译实现
fn bing_translate_service<'a>(
    texts: &'a [String],
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/113.0.0.0 Safari/537.36 Edg/113.0.1774.42")
            .build()
            .map_err(TranslationError::from)?;

        // 获取token
        let token = client
            .get("https://edge.microsoft.com/translate/auth")
            .send()
            .await
            .map_err(TranslationError::from)?
            .text()
            .await
            .map_err(TranslationError::from)?;

        let mut translated_texts = Vec::with_capacity(texts.len());
        
        for text in texts {
            let response = client
                .post("https://api-edge.cognitive.microsofttranslator.com/translate")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .query(&[
                    ("api-version", "3.0"),
                    ("from", "auto"),
                    ("to", "zh-Hans"),
                ])
                .json(&[serde_json::json!({"Text": text})])
                .send()
                .await
                .map_err(TranslationError::from)?;

            let response_text = response.text().await.map_err(TranslationError::from)?;
            let result: serde_json::Value = serde_json::from_str(&response_text)?;

            if let Some(translations) = result[0]["translations"].as_array() {
                if !translations.is_empty() {
                    translated_texts.push(translations[0]["text"].as_str().unwrap_or("").to_string());
                    continue;
                }
            }
            return Err(TranslationError("Bing translation failed".into()));
        }
        
        Ok((translated_texts, "Bing".into()))
    })
}

// 辅助函数
fn generate_qtk() -> String {
    let timestamp = Utc::now().timestamp_millis();
    let random_num = rand::random::<u32>();
    format!("{}{}", timestamp, random_num)
}

fn generate_qtv() -> String {
    Uuid::new_v4().to_string().replace("-", "")
}