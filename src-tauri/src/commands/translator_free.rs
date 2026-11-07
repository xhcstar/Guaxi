//! 划词/批量翻译的 Tauri 命令层。
//!
//! 真正的接口调度、竞速、熔断、限速全部下沉到 [`crate::commands::translate_pool`]，
//! 这里只负责：租户密钥优先级、文本预处理、以及给前端的返回结构。

use serde::Serialize;
use std::collections::HashMap;
use std::env;
use tracing::{info, warn};

use crate::commands::offline_dict;
use crate::commands::translate_local;
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

/// 离线词典只收录词条，碰到整句就交给在线接口：
/// 这样单个词/短语在断网时也能出结果，又不会把句子硬塞进词典匹配。
fn offline_dict_translation(text: &str, from: &str, to: &str) -> Option<String> {
    let term = text.trim();
    if term.is_empty() || term.chars().count() > 64 {
        return None;
    }
    if term.split_whitespace().count() > 4 {
        return None;
    }
    if term.chars().any(|c| {
        matches!(
            c,
            '.' | ',' | ';' | ':' | '!' | '?' | '\n' | '。' | '，' | '；' | '：' | '！' | '？'
        )
    }) {
        return None;
    }
    let entry = offline_dict::lookup_word(term, from, to)?;
    let joined = entry
        .meanings
        .iter()
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .collect::<Vec<_>>()
        .join("；");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
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

    // 词条先查离线词典：命中就不必等网络，剩下的句子再交给在线接口。
    let mut translated: Vec<Option<String>> = vec![None; clean.len()];
    let mut pending: Vec<usize> = Vec::new();
    for (idx, text) in clean.iter().enumerate() {
        match offline_dict_translation(text, &from, &to) {
            Some(hit) => {
                translate_pool::cache_put_engine(&from, &to, text, &hit, "OfflineDict");
                translated[idx] = Some(hit);
            }
            None => pending.push(idx),
        }
    }
    if pending.is_empty() {
        info!("[Translate] {} 条 (from={}, to={}) 由 OfflineDict 完成", clean.len(), from, to);
        return Ok(TranslationResult {
            success: true,
            translated_texts: translated.into_iter().map(|x| x.unwrap_or_default()).collect(),
            error: None,
        });
    }
    let pending_texts: Vec<String> = pending.iter().map(|idx| clean[*idx].clone()).collect();

    if let Some((secret_id, secret_key)) = load_vendor_secret_pair() {
        match translator::translate_text(pending_texts.clone(), secret_id, secret_key).await {
            Ok(resp) if resp.success && resp.translated_texts.len() == pending_texts.len() => {
                for (pos, dst) in resp.translated_texts.iter().enumerate() {
                    let idx = pending[pos];
                    translate_pool::cache_put_engine(&from, &to, &clean[idx], dst, "Tencent");
                    translated[idx] = Some(dst.clone());
                }
                return Ok(TranslationResult {
                    success: true,
                    translated_texts: translated.into_iter().map(|x| x.unwrap_or_default()).collect(),
                    error: None,
                });
            }
            Ok(_) => warn!("[Translate] 腾讯云返回结果不完整，回退到免费接口池"),
            Err(e) => warn!("[Translate] 腾讯云调用失败，回退到免费接口池: {}", e),
        }
    }

    let outcome = translate_pool::translate_texts(&pending_texts, &from, &to).await;
    for (pos, idx) in pending.iter().enumerate() {
        if let Some(text) = outcome.texts.get(pos) {
            translated[*idx] = Some(text.clone());
        }
    }
    info!(
        "[Translate] {} 条 (from={}, to={}) 由 {} 完成",
        clean.len(),
        from,
        to,
        if outcome.engine.is_empty() { "-" } else { outcome.engine.as_str() }
    );
    Ok(TranslationResult {
        success: outcome.error.is_none(),
        translated_texts: translated.into_iter().map(|x| x.unwrap_or_default()).collect(),
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
    let offline_hit = offline_dict_translation(&clean, &from_norm, &to_norm);

    // 同一段文字重复划词（很常见）直接命中缓存：先查进程内，再查落盘库，零网络耗时。
    if let Some(hit) = translate_pool::cache_get_persistent(&from_norm, &to_norm, &clean).await {
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
        // 没有可用在线接口时，离线词典命中就足以给出结果。
        if let Some(hit) = offline_hit {
            return Ok(SelectionTranslateMultiResult {
                success: true,
                items: vec![SelectionTranslateEngineItem {
                    engine: "离线词典".into(),
                    success: true,
                    text: hit,
                    error: None,
                }],
                error: None,
            });
        }
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
    if offline_hit.is_some() {
        items.insert(
            0,
            SelectionTranslateEngineItem {
                engine: "离线词典".into(),
                success: true,
                text: offline_hit.clone().unwrap_or_default(),
                error: None,
            },
        );
    }
    items.sort_by_key(|it| !it.success);

    if let Some(primary) = items.iter().find(|it| it.success && !it.text.trim().is_empty()) {
        translate_pool::cache_put_engine(&from_norm, &to_norm, &clean, &primary.text, &primary.engine);
    }
    let mut success = items.iter().any(|it| it.success);
    if !success {
        warn!(
            "[SelectionTranslate] 全部在线接口失败: {}",
            items
                .iter()
                .map(|it| format!("{}={}", it.engine, it.error.clone().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("；")
        );
        // 在线接口全挂（失效/被封/额度用完）时，用本地小模型兜底，保证划词一定有译文。
        match translate_local::translate_texts_local(
            &[clean.clone()],
            &from_norm,
            &to_norm,
        )
        .await
        {
            Ok(results) => {
                let text = results.into_iter().next().unwrap_or_default();
                if !text.trim().is_empty() {
                    translate_pool::cache_put_engine(&from_norm, &to_norm, &clean, &text, "LocalModel");
                    items.insert(
                        0,
                        SelectionTranslateEngineItem {
                            engine: "本地小模型".into(),
                            success: true,
                            text,
                            error: None,
                        },
                    );
                    success = true;
                }
            }
            Err(e) => warn!("[SelectionTranslate] 本地小模型兜底失败: {}", e),
        }
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

