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

    let punctuated = {
        let engine = entry.lock();
        engine.add_punctuation(text)
    };
    let Some(punctuated) = punctuated else {
        return text.to_string();
    };

    // 这个模型是中英混排的，但标点表里**只有全角符号**，英文句子同样会被加上「，。」。
    // 整段不含中日韩文字时按英文习惯换成半角，中文文本保持全角。
    if contains_cjk(&punctuated) {
        punctuated
    } else {
        to_half_width(&punctuated)
    }
}

/// 是否含中日韩表意文字。用来判断该用全角还是半角标点。
fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(ch,
            '\u{3400}'..='\u{4DBF}'   // 扩展 A
            | '\u{4E00}'..='\u{9FFF}' // 统一表意
            | '\u{F900}'..='\u{FAFF}' // 兼容表意
            | '\u{3040}'..='\u{30FF}' // 平假名 / 片假名
        )
    })
}

/// 全角标点转半角，并按英文习惯在标点后补一个空格。
fn to_half_width(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        let mapped = match ch {
            '，' | '、' => ',',
            '。' => '.',
            '？' => '?',
            '！' => '!',
            '：' => ':',
            '；' => ';',
            '（' => '(',
            '）' => ')',
            '《' => '<',
            '》' => '>',
            '“' | '”' => '"',
            '‘' | '’' => '\'',
            '—' => '-',
            other => other,
        };
        out.push(mapped);

        // 模型是直接把标点贴在词后面的，英文里需要补一个分隔空格。
        // 这里只认 ASCII 字母数字：`is_alphanumeric()` 对中日韩字符同样返回 true，
        // 用它会往中文里插入多余空格。
        if matches!(mapped, ',' | '.' | '?' | '!' | ':' | ';') {
            if chars.peek().is_some_and(|next| next.is_ascii_alphanumeric()) {
                out.push(' ');
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{contains_cjk, to_half_width};

    #[test]
    fn detects_cjk_for_punctuation_style() {
        assert!(contains_cjk("今天天气不错"));
        assert!(contains_cjk("mixed 中文 text"));
        assert!(!contains_cjk("hello world, this is a test."));
        assert!(!contains_cjk("12345"));
    }

    /// 全角转半角不只是换字符：还要按英文习惯补空格，
    /// 否则模型给出的「hello world，this」会变成「hello world,this」。
    #[test]
    fn converts_full_width_punctuation_to_half_width() {
        assert_eq!(to_half_width("hello world，this is a test。"), "hello world, this is a test.");
        assert_eq!(to_half_width("真的吗？是的！"), "真的吗?是的!");
        // 标点后已经有空格时不重复补。
        assert_eq!(to_half_width("a。 b"), "a. b");
        // 句末没有后继字符，也不补尾随空格。
        assert_eq!(to_half_width("done。"), "done.");
    }
}
