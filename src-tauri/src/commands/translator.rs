use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use rand::Rng;
use std::error::Error;
use std::fmt;
use tracing::{warn, info, error, debug};
use std::pin::Pin;
use std::future::Future;

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

// 有道翻译响应结构
#[derive(Debug, Deserialize)]
struct YoudaoResponse {
    translation: Option<Vec<String>>,
    errorCode: Option<String>,
}

// 必应翻译响应结构
#[derive(Debug, Deserialize)]
struct BingResponse {
    translations: Vec<BingTranslation>,
}

#[derive(Debug, Deserialize, Clone)]
struct BingResult {
    translations: Vec<BingTranslation>,
}

#[derive(Debug, Deserialize, Clone)]
struct BingTranslation {
    text: String,
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
) -> Result<TranslationResponse, String> {
    info!("开始翻译文本: {}", text);
    let clean_text = preprocess_text(&text);
    info!("预处理后的文本: {}", clean_text);
    
    type TranslateFn = for<'a> fn(&'a str) -> Pin<Box<dyn Future<Output = Result<TranslationResponse, TranslationError>> + Send + 'a>>;

    let services: [TranslateFn; 3] = [
        |clean_text| Box::pin(try_bing_translate(clean_text)) as _,
        |clean_text| Box::pin(try_baidu_translate(clean_text)) as _,
        |clean_text| Box::pin(try_youdao_translate(clean_text)) as _,
    ];

    for (i, service) in services.iter().enumerate() {
        let service_name = match i {
            0 => "Baidu",
            1 => "Youdao",
            2 => "Bing",
            _ => "Unknown",
        };
        info!("尝试使用 {} 翻译服务", service_name);
        
        match service(&text).await {
            Ok(result) => {
                info!("{} 翻译成功: {}", service_name, result.text);
                return Ok(result);
            },
            Err(e) => {
                warn!("{} 翻译服务失败: {}", service_name, e);
                debug!("详细错误信息: {:?}", e);
            }
        }
    }
    
    error!("所有翻译服务均不可用");
    Err("所有翻译服务均不可用".to_string())
}

async fn try_baidu_translate(text: &str) -> Result<TranslationResponse, TranslationError> {
    debug!("尝试百度翻译: {}", text);
    
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| {
            error!("创建百度翻译客户端失败: {}", e);
            TranslationError::from(e)
        })?;

    let salt = rand::thread_rng().gen::<u32>().to_string();
    debug!("百度翻译 salt: {}", salt);
    
    let mut hasher = Sha256::new();
    hasher.update(format!("fanyideskweb{}{}Ygy_4c=r#ew#MC#N6K", text, salt));
    let sign = format!("{:x}", hasher.finalize());
    debug!("百度翻译 sign: {}", sign);

    let params = [
        ("from", "auto"),
        ("to", "zh"),
        ("query", text),
        ("sign", &sign),
        ("token", "a16531d6e6a1aed4e0d2d5c5b5f4e6d"),
    ];

    let response = client
        .post("https://fanyi.baidu.com/v2transapi")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            error!("百度翻译请求失败: {}", e);
            TranslationError::from(e)
        })?;

    debug!("百度翻译响应状态: {}", response.status());
    
    let json: serde_json::Value = response.json().await.map_err(|e| {
        error!("百度翻译响应解析失败: {}", e);
        TranslationError::from(e)
    })?;
    
    debug!("百度翻译原始响应: {:?}", json);

    let translation = json["trans_result"]["data"][0]["dst"]
        .as_str()
        .ok_or_else(|| {
            error!("无法解析百度翻译结果，原始响应: {:?}", json);
            TranslationError("无法解析百度翻译结果".into())
        })?
        .to_string();

    Ok(TranslationResponse {
        text: translation,
        confidence: 0.9,
        engine: "Baidu".into(),
    })
}

async fn try_youdao_translate(text: &str) -> Result<TranslationResponse, TranslationError> {
    debug!("尝试有道翻译: {}", text);
    
    let client = Client::new();

    let salt = rand::thread_rng().gen::<u32>().to_string();
    debug!("有道翻译 salt: {}", salt);
    
    let mut hasher = Sha256::new();
    hasher.update(format!("yd_secret_key{}{}", truncate_q(text), salt));
    let sign = format!("{:x}", hasher.finalize());
    debug!("有道翻译 sign: {}", sign);

    let params = [
        ("q", text),
        ("from", "auto"),
        ("to", "zh-CHS"),
        ("salt", &salt),
        ("sign", &sign),
    ];

    let response = client
        .post("https://openapi.youdao.com/api")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            error!("有道翻译请求失败: {}", e);
            TranslationError::from(e)
        })?;

    debug!("有道翻译响应状态: {}", response.status());
    
    let result: YoudaoResponse = response.json().await.map_err(|e| {
        error!("有道翻译响应解析失败: {}", e);
        TranslationError::from(e)
    })?;
    
    debug!("有道翻译原始响应: {:?}", result);

    result.translation
        .and_then(|v| v.into_iter().next())
        .map(|text| TranslationResponse {
            text,
            confidence: 0.85,
            engine: "Youdao".into(),
        })
        .ok_or_else(|| {
            let err_msg = result.errorCode.unwrap_or_else(|| "有道翻译失败".into());
            error!("有道翻译业务错误: {}", err_msg);
            TranslationError(err_msg)
        })
}

async fn try_bing_translate(text: &str) -> Result<TranslationResponse, TranslationError> {
    debug!("尝试必应翻译: {}", text);
    
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(|e| {
            error!("创建必应翻译客户端失败: {}", e);
            TranslationError::from(e)
        })?;

    let response = client
        .post("https://cn.bing.com/ttranslatev3")
        .form(&[("text", text), ("to", "zh-Hans")])
        .send()
        .await
        .map_err(|e| {
            error!("必应翻译请求失败: {}", e);
            TranslationError::from(e)
        })?;

    debug!("必应翻译响应状态: {}", response.status());
    
    let result: Vec<BingResult> = response.json().await.map_err(|e| {
        error!("必应翻译响应解析失败: {}", e);
        TranslationError::from(e)
    })?;
    
    debug!("必应翻译原始响应: {:?}", result);

    // 先保存错误信息需要的原始响应
    let original_response = result.clone();
    
    match result.into_iter()
        .flat_map(|r| r.translations.into_iter().next())
        .map(|t| TranslationResponse {
            text: t.text,
            confidence: 0.88,
            engine: "Bing".into(),
        })
        .next() {
            Some(translation) => Ok(translation),
            None => {
                error!("必应翻译解析失败，原始响应: {:?}", original_response);
                Err(TranslationError("必应翻译解析失败".into()))
            }
        }
}

fn truncate_q(q: &str) -> String {
    if q.len() <= 20 { q.into() }
    else { format!("{}{}{}", &q[..10], &q[q.len()-10..], q.len()) }
}