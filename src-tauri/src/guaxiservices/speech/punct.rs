//! 标点恢复：给识别结果补上标点与断句。
//!
//! sherpa 的识别模型（SenseVoice、Paraformer 等）输出的是**不带标点**的文本流，
//! 实时听写攒出来的就是一整段没有标点的字。这里在识别之后用官方标点模型
//! （CT-Transformer，中英混排）补标点，与识别器共用同一套「常驻实例 + 配置键缓存」。

use std::hash::{Hash, Hasher};
use std::path::Path;

use once_cell::sync::Lazy;
use sherpa_onnx::{OfflinePunctuation, OfflinePunctuationConfig, OfflinePunctuationModelConfig};
use tracing::warn;

use super::cache::EngineCache;

static PUNCT_CACHE: Lazy<EngineCache<OfflinePunctuation>> = Lazy::new(EngineCache::new);

fn cache_key(model: &Path, provider: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    model.to_string_lossy().hash(&mut hasher);
    provider.hash(&mut hasher);
    hasher.finish()
}

fn build_engine(model: &Path, provider: &str) -> Result<OfflinePunctuation, String> {
    let config = OfflinePunctuationConfig {
        model: OfflinePunctuationModelConfig {
            ct_transformer: Some(model.to_string_lossy().to_string()),
            num_threads: 1,
            debug: false,
            provider: Some(provider.to_string()),
        },
    };
    OfflinePunctuation::create(&config)
        .ok_or_else(|| format!("创建标点模型失败：{}", model.display()))
}

/// 给一段识别文本补标点。
///
/// 刻意做成「失败就原样返回」：标点是锦上添花的东西，
/// 不该因为它让本来已经成功的识别结果变成报错。
///
/// 阻塞调用，调用方须放在 `spawn_blocking` 里。
pub fn add_punctuation(model: &Path, provider: &str, text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    let entry = match PUNCT_CACHE.get_or_create(cache_key(model, provider), || {
        build_engine(model, provider)
    }) {
        Ok(entry) => entry,
        Err(error) => {
            warn!(error = error.as_str(), "标点模型不可用，跳过标点恢复");
            return text.to_string();
        }
    };

    let engine = entry.lock();
    engine.add_punctuation(text).unwrap_or_else(|| text.to_string())
}
