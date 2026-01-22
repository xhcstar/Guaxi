use chrono::Utc;
use reqwest::{header, Client};
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
        match process_batch(chunk, "auto", "zh").await {
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

#[derive(Debug, Serialize, Clone)]
pub struct SelectionTranslateEngineItem {
    pub engine: String,
    pub success: bool,
    pub text: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SelectionTranslateMultiResult {
    pub success: bool,
    pub items: Vec<SelectionTranslateEngineItem>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn selection_translate_free_multi(
    text: String,
    from: String,
    to: String,
) -> Result<SelectionTranslateMultiResult, String> {
    let raw = text.trim().to_string();
    if raw.is_empty() {
        return Err("Empty text".into());
    }

    let clean = preprocess_text(&raw);
    let texts = vec![clean];

    info!(
        "SelectionTranslate multi: len={}, from={}, to={}",
        texts[0].len(),
        from,
        to
    );

    let services: [(&str, TranslateFn); 3] = [
        ("Bing", bing_translate_service),
        ("Google", google_translate_service),
        ("Tencent", tencent_translate_service),
    ];

    let mut items = Vec::with_capacity(services.len());

    for (name, service) in services.iter() {
        match service(&texts, &from, &to).await {
            Ok((results, _engine)) => {
                let t = results.get(0).cloned().unwrap_or_default();
                if t.trim().is_empty() {
                    items.push(SelectionTranslateEngineItem {
                        engine: name.to_string(),
                        success: false,
                        text: String::new(),
                        error: Some("Empty result".into()),
                    });
                } else {
                    items.push(SelectionTranslateEngineItem {
                        engine: name.to_string(),
                        success: true,
                        text: t,
                        error: None,
                    });
                }
            }
            Err(e) => {
                warn!("SelectionTranslate engine {} failed: {}", name, e);
                items.push(SelectionTranslateEngineItem {
                    engine: name.to_string(),
                    success: false,
                    text: String::new(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let success = items.iter().any(|it| it.success);
    let error = if success {
        None
    } else {
        Some("All translation services failed".into())
    };

    Ok(SelectionTranslateMultiResult {
        success,
        items,
        error,
    })
}

// 3. 定义翻译函数类型
type TranslateFn = for<'a> fn(
    &'a [String],
    &'a str,
    &'a str,
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>>;

// 4. 处理每批翻译
async fn process_batch(
    texts: &[String],
    from: &str,
    to: &str,
) -> Result<(Vec<String>, String), TranslationError> {
    let clean_texts: Vec<String> = texts.iter().map(|t| preprocess_text(t)).collect();

    // 定义所有翻译服务（按优先级排序）
    // Tencent 的 free 接口近期容易返回 405/非 JSON（反爬/接口变更），放到最后兜底。
    let services: [TranslateFn; 3] = [
        bing_translate_service,
        google_translate_service,
        tencent_translate_service,
    ];

    let mut last_error = None;

    for service in services.iter() {
        match service(&clean_texts, from, to).await {
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

fn normalize_lang(lang: &str) -> String {
    lang.trim().to_string()
}

fn normalize_lang_lower(lang: &str) -> String {
    lang.trim().to_ascii_lowercase()
}

fn map_google_lang(lang: &str) -> String {
    let l = normalize_lang_lower(lang);
    match l.as_str() {
        "auto" => "auto".into(),
        "zh" | "zh-cn" | "zh-hans" => "zh-CN".into(),
        "zh-tw" | "zh-hant" => "zh-TW".into(),
        _ => l,
    }
}

fn map_bing_lang(lang: &str) -> String {
    let l = normalize_lang_lower(lang);
    match l.as_str() {
        "auto" => "auto".into(),
        "zh" | "zh-cn" | "zh-hans" => "zh-Hans".into(),
        "zh-tw" | "zh-hant" => "zh-Hant".into(),
        _ => normalize_lang(lang),
    }
}

fn map_tencent_lang(lang: &str) -> String {
    let l = normalize_lang_lower(lang);
    match l.as_str() {
        "auto" => "auto".into(),
        "zh" | "zh-cn" | "zh-hans" | "zh-hant" | "zh-tw" => "zh".into(),
        "ja" => "jp".into(),
        _ => l,
    }
}

fn summarize_body(body: &str) -> String {
    let trimmed = body.trim();
    let mut s: String = trimmed.chars().take(240).collect();
    s = s.replace('\n', " ").replace('\r', " ");
    if trimmed.chars().count() > 240 {
        s.push('…');
    }
    s
}

fn response_is_probably_html(content_type: Option<&str>, body: &str) -> bool {
    if let Some(ct) = content_type {
        let ct_lower = ct.to_ascii_lowercase();
        if ct_lower.contains("text/html") {
            return true;
        }
    }

    let b = body.trim_start().to_ascii_lowercase();
    b.starts_with("<!doctype html") || b.starts_with("<html")
}

// 6. 腾讯翻译实现
fn tencent_translate_service<'a>(
    texts: &'a [String],
    from: &'a str,
    to: &'a str,
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        let client = Client::builder()
            .user_agent("Mozilla/5.0")
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(TranslationError::from)?;

        let mut translated_texts = Vec::with_capacity(texts.len());

        let source = map_tencent_lang(from);
        let target = map_tencent_lang(to);

        for text in texts {
            let response = client
                .post("https://fanyi.qq.com/api/translate")
                .header("Content-Type", "application/x-www-form-urlencoded")
                .header("Origin", "https://fanyi.qq.com")
                .header("Referer", "https://fanyi.qq.com/")
                .form(&[
                    ("sourceText", text.as_str()),
                    ("source", source.as_str()),
                    ("target", target.as_str()),
                    ("qtk", &generate_qtk()),
                    ("qtv", &generate_qtv()),
                ])
                .send()
                .await
                .map_err(TranslationError::from)?;

            let status = response.status();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let response_text = response.text().await.map_err(TranslationError::from)?;

            if !status.is_success() {
                return Err(TranslationError(format!(
                    "Tencent HTTP {}: {}",
                    status.as_u16(),
                    summarize_body(&response_text)
                )));
            }
            if response_text.trim().is_empty() {
                return Err(TranslationError("Tencent empty response".into()));
            }
            if response_is_probably_html(content_type.as_deref(), &response_text) {
                return Err(TranslationError(format!(
                    "Tencent non-JSON response: {}",
                    summarize_body(&response_text)
                )));
            }

            let result: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
                TranslationError(format!(
                    "Tencent JSON error: {} ({})",
                    e,
                    summarize_body(&response_text)
                ))
            })?;

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
    from: &'a str,
    to: &'a str,
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        let client = Client::builder()
            .user_agent("Mozilla/5.0")
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(TranslationError::from)?;

        let mut translated_texts = Vec::with_capacity(texts.len());

        let sl = map_google_lang(from);
        let tl = map_google_lang(to);

        for text in texts {
            let response = client
                .get("https://translate.googleapis.com/translate_a/single")
                .query(&[
                    ("client", "gtx"),
                    ("sl", sl.as_str()),
                    ("tl", tl.as_str()),
                    ("dt", "t"),
                    ("q", text.as_str()),
                ])
                .send()
                .await
                .map_err(TranslationError::from)?;

            let status = response.status();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let response_text = response.text().await.map_err(TranslationError::from)?;

            if !status.is_success() {
                return Err(TranslationError(format!(
                    "Google HTTP {}: {}",
                    status.as_u16(),
                    summarize_body(&response_text)
                )));
            }
            if response_text.trim().is_empty() {
                return Err(TranslationError("Google empty response".into()));
            }
            if response_is_probably_html(content_type.as_deref(), &response_text) {
                return Err(TranslationError(format!(
                    "Google non-JSON response: {}",
                    summarize_body(&response_text)
                )));
            }

            let result: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
                TranslationError(format!(
                    "Google JSON error: {} ({})",
                    e,
                    summarize_body(&response_text)
                ))
            })?;

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
    from: &'a str,
    to: &'a str,
) -> Pin<Box<dyn Future<Output = Result<(Vec<String>, String), TranslationError>> + Send + 'a>> {
    Box::pin(async move {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/113.0.0.0 Safari/537.36 Edg/113.0.1774.42")
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(12))
            .build()
            .map_err(TranslationError::from)?;

        let token_response = client
            .get("https://edge.microsoft.com/translate/auth")
            .send()
            .await
            .map_err(TranslationError::from)?;

        let token_status = token_response.status();
        let token_content_type = token_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let token_body = token_response.text().await.map_err(TranslationError::from)?;

        if !token_status.is_success() {
            return Err(TranslationError(format!(
                "Bing token HTTP {}: {}",
                token_status.as_u16(),
                summarize_body(&token_body)
            )));
        }
        if token_body.trim().is_empty() {
            return Err(TranslationError("Bing token empty response".into()));
        }
        if response_is_probably_html(token_content_type.as_deref(), &token_body) {
            return Err(TranslationError(format!(
                "Bing token non-plain response: {}",
                summarize_body(&token_body)
            )));
        }

        let token = token_body.trim().to_string();

        let mut translated_texts = Vec::with_capacity(texts.len());

        let from_norm = normalize_lang_lower(from);
        let mut query: Vec<(&str, String)> = vec![("api-version", "3.0".into()), ("to", map_bing_lang(to))];
        if from_norm != "auto" {
            query.push(("from", map_bing_lang(from)));
        }

        for text in texts {
            let response = client
                .post("https://api-edge.cognitive.microsofttranslator.com/translate")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .query(&query)
                .json(&[serde_json::json!({"Text": text})])
                .send()
                .await
                .map_err(TranslationError::from)?;

            let status = response.status();
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let response_text = response.text().await.map_err(TranslationError::from)?;

            if !status.is_success() {
                return Err(TranslationError(format!(
                    "Bing HTTP {}: {}",
                    status.as_u16(),
                    summarize_body(&response_text)
                )));
            }
            if response_text.trim().is_empty() {
                return Err(TranslationError("Bing empty response".into()));
            }
            if response_is_probably_html(content_type.as_deref(), &response_text) {
                return Err(TranslationError(format!(
                    "Bing non-JSON response: {}",
                    summarize_body(&response_text)
                )));
            }

            let result: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
                TranslationError(format!(
                    "Bing JSON error: {} ({})",
                    e,
                    summarize_body(&response_text)
                ))
            })?;

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