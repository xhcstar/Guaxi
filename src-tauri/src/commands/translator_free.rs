//! 划词/批量翻译的 Tauri 命令层。
//!
//! 真正的接口调度、竞速、熔断、限速全部下沉到 [`crate::commands::translate_pool`]，
//! 这里只负责：租户密钥优先级、文本预处理、以及给前端的返回结构。

use serde::Serialize;
use std::collections::HashMap;
use std::env;
use tracing::{info, warn};

use crate::commands::translate_pool::{self, ProviderSpec};
use crate::commands::translator;

#[derive(Debug, Serialize, Clone)]
pub struct TranslationResult {
    pub success: bool,
    pub translated_texts: Vec<String>,
    pub error: Option<String>,
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

fn normalize_lang_lower(lang: &str) -> String {
    lang.trim().to_ascii_lowercase()
}

/// 富文本片段里的图片占位与尖括号会让部分接口直接报错，先做无害化。
fn preprocess_text(text: &str) -> String {
    if text.trim().starts_with("<img") {
        return "[Image]".to_string();
    }
    text.replace('<', "&lt;").replace('>', "&gt;")
}

fn load_vendor_secret_pair() -> Option<(String, String)> {
    let candidates = [
        ("TENCENT_SECRET_ID", "TENCENT_SECRET_KEY"),
        ("TMT_SECRET_ID", "TMT_SECRET_KEY"),
        ("GUAXI_TENCENT_SECRET_ID", "GUAXI_TENCENT_SECRET_KEY"),
    ];
    for (id_key, key_key) in candidates {
        let id = env::var(id_key).unwrap_or_default().trim().to_string();
        let key = env::var(key_key).unwrap_or_default().trim().to_string();
        if !id.is_empty() && !key.is_empty() {
            return Some((id, key));
        }
    }
    None
}

/// 批量翻译。配置了腾讯云密钥时优先走官方接口，否则整批交给免费接口池。
#[tauri::command]
pub async fn translate_text_free(
    texts: Vec<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<TranslationResult, String> {
    if texts.is_empty() {
        return Ok(TranslationResult { success: true, translated_texts: Vec::new(), error: None });
    }
    let from = normalize_lang_lower(&from.unwrap_or_else(|| "auto".into()));
    let to = normalize_lang_lower(&to.unwrap_or_else(|| "zh".into()));
    let clean: Vec<String> = texts.iter().map(|t| preprocess_text(t)).collect();

    if let Some((secret_id, secret_key)) = load_vendor_secret_pair() {
        match translator::translate_text(clean.clone(), secret_id, secret_key).await {
            Ok(resp) if resp.success && resp.translated_texts.len() == clean.len() => {
                for (src, dst) in clean.iter().zip(resp.translated_texts.iter()) {
                    translate_pool::cache_put(&from, &to, src, dst);
                }
                return Ok(TranslationResult {
                    success: true,
                    translated_texts: resp.translated_texts,
                    error: None,
                });
            }
            Ok(_) => warn!("[Translate] 腾讯云返回结果不完整，回退到免费接口池"),
            Err(e) => warn!("[Translate] 腾讯云调用失败，回退到免费接口池: {}", e),
        }
    }

    let outcome = translate_pool::translate_texts(&clean, &from, &to).await;
    info!(
        "[Translate] {} 条 (from={}, to={}) 由 {} 完成",
        clean.len(),
        from,
        to,
        if outcome.engine.is_empty() { "-" } else { outcome.engine.as_str() }
    );
    Ok(TranslationResult {
        success: outcome.error.is_none(),
        translated_texts: outcome.texts,
        error: outcome.error,
    })
}

fn env_u64(key: &str, default_value: u64, lo: u64, hi: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default_value)
        .clamp(lo, hi)
}

/// 划词翻译：同时问健康度最高的几家接口，第一家成功后再给其余几百毫秒的宽限期收结果。
/// 这样面板通常能并排展示 2~3 家译文，而总耗时由最快的那家决定，不会被最慢的拖住。
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
    let from_norm = normalize_lang_lower(&from);
    let to_norm = normalize_lang_lower(&to);

    // 同一段文字重复划词（很常见）直接命中缓存，零网络耗时。
    if let Some(hit) = translate_pool::cache_get(&from_norm, &to_norm, &clean) {
        return Ok(SelectionTranslateMultiResult {
            success: true,
            items: vec![SelectionTranslateEngineItem {
                engine: "Cache".into(),
                success: true,
                text: hit,
                error: None,
            }],
            error: None,
        });
    }

    let want = env_u64("GUAXI_SELECTION_FANOUT", 3, 1, 6) as usize;
    let budget_ms = env_u64("GUAXI_SELECTION_BUDGET_MS", 2600, 1200, 12000);
    let grace_ms = env_u64("GUAXI_SELECTION_GRACE_MS", 450, 0, 3000);
    let stagger_ms = env_u64("GUAXI_SELECTION_STAGGER_MS", 90, 0, 1000);

    let candidates: Vec<&'static ProviderSpec> = translate_pool::ranked_for_selection(&to_norm, want);
    if candidates.is_empty() {
        return Err("翻译接口池为空".into());
    }
    info!(
        "[SelectionTranslate] len={} from={} to={} engines=[{}]",
        clean.len(),
        from_norm,
        to_norm,
        candidates.iter().map(|s| s.name).collect::<Vec<_>>().join(",")
    );

    let mut join_set = tokio::task::JoinSet::new();
    for (idx, spec) in candidates.iter().enumerate() {
        let spec: &'static ProviderSpec = spec;
        let text_c = clean.clone();
        let from_c = from_norm.clone();
        let to_c = to_norm.clone();
        let delay = stagger_ms.saturating_mul(idx as u64);
        join_set.spawn(async move {
            if delay > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
            let result = translate_pool::attempt_one(spec, &text_c, &from_c, &to_c).await;
            (spec.name, result)
        });
    }

    let start = std::time::Instant::now();
    let hard_deadline = start + std::time::Duration::from_millis(budget_ms);
    let mut collected: HashMap<&'static str, SelectionTranslateEngineItem> = HashMap::new();
    let mut first_success_at: Option<std::time::Instant> = None;

    loop {
        let now = std::time::Instant::now();
        let deadline = match first_success_at {
            Some(t) => hard_deadline.min(t + std::time::Duration::from_millis(grace_ms)),
            None => hard_deadline,
        };
        if now >= deadline {
            break;
        }
        match tokio::time::timeout(deadline - now, join_set.join_next()).await {
            Ok(Some(Ok((name, Ok(translated))))) => {
                if first_success_at.is_none() {
                    first_success_at = Some(std::time::Instant::now());
                }
                collected.insert(
                    name,
                    SelectionTranslateEngineItem {
                        engine: name.to_string(),
                        success: true,
                        text: translated,
                        error: None,
                    },
                );
            }
            Ok(Some(Ok((name, Err(e))))) => {
                collected.insert(
                    name,
                    SelectionTranslateEngineItem {
                        engine: name.to_string(),
                        success: false,
                        text: String::new(),
                        error: Some(e.message),
                    },
                );
            }
            Ok(Some(Err(e))) => warn!("[SelectionTranslate] 任务异常: {}", e),
            Ok(None) => break,
            Err(_) => break,
        }
    }
    join_set.abort_all();

    // 按健康度顺序输出，前端取第一条成功项作为主译文，所以最优接口要排在前面。
    let mut items: Vec<SelectionTranslateEngineItem> = Vec::with_capacity(candidates.len());
    for spec in &candidates {
        items.push(collected.remove(spec.name).unwrap_or_else(|| SelectionTranslateEngineItem {
            engine: spec.name.to_string(),
            success: false,
            text: String::new(),
            error: Some("未在时限内返回".into()),
        }));
    }
    items.sort_by_key(|it| !it.success);

    if let Some(primary) = items.iter().find(|it| it.success && !it.text.trim().is_empty()) {
        translate_pool::cache_put(&from_norm, &to_norm, &clean, &primary.text);
    }
    let success = items.iter().any(|it| it.success);
    if !success {
        warn!(
            "[SelectionTranslate] 全部失败: {}",
            items
                .iter()
                .map(|it| format!("{}={}", it.engine, it.error.clone().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("；")
        );
    }

    Ok(SelectionTranslateMultiResult {
        success,
        items,
        error: if success { None } else { Some("All translation services failed".into()) },
    })
}

/// 接口池实时健康状态，用于设置页/日志排查哪家接口在冷却或已失效。
#[tauri::command]
pub async fn translate_pool_status() -> Result<Vec<translate_pool::ProviderStatus>, String> {
    Ok(translate_pool::pool_status())
}

