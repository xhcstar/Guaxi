//! sherpa-onnx 离线 TTS 的进程内封装。
//!
//! 与旧的 CLI 路径相比，模型的加载只发生一次：实测 kokoro 加载约 3.5s，
//! 而每次合成本身只要约 2s——旧路径每段都要把 3.5s 的加载重付一遍。

use std::hash::{Hash, Hasher};

use once_cell::sync::Lazy;
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};

use super::cache::EngineCache;
use super::wav::AudioBuffer;

static TTS_CACHE: Lazy<EngineCache<OfflineTts>> = Lazy::new(EngineCache::new);

/// 一次合成任务的完整引擎参数，由 `config` 子模块解析得到。
/// 字段刻意与 sherpa-onnx 的配置对齐，避免中间再套一层映射。
#[derive(Debug, Clone, PartialEq)]
pub enum TtsModel {
    Kokoro {
        model: String,
        tokens: String,
        voices: Option<String>,
        data_dir: Option<String>,
        dict_dir: Option<String>,
        lexicon: Option<String>,
    },
    Vits {
        model: String,
        tokens: String,
        data_dir: Option<String>,
        lexicon: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TtsEngineConfig {
    pub model: TtsModel,
    /// 逗号分隔的 fst 文件列表（数字/日期文本归一化）。
    pub rule_fsts: Option<String>,
    /// "cpu" | "cuda" | "coreml" | "directml" | "trt" | ...
    pub provider: String,
    pub num_threads: i32,
}

impl TtsEngineConfig {
    /// 缓存键：任一参数变化都必须换实例，否则会拿旧模型合成。
    fn cache_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}|{}|{}|{}", self.model, self.rule_fsts.is_some(), self.provider, self.num_threads)
            .hash(&mut hasher);
        if let Some(rule_fsts) = &self.rule_fsts {
            rule_fsts.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// 合成一段文本。模型已加载过则直接复用，不再重新读盘。
///
/// 阻塞调用，调用方必须放在 `spawn_blocking` 里。
pub fn synthesize(
    config: &TtsEngineConfig,
    text: &str,
    speaker: i32,
    speed: f32,
) -> Result<AudioBuffer, String> {
    let cached = TTS_CACHE.get_or_create(config.cache_key(), || build_engine(config))?;
    let engine = cached.lock();

    let generation = GenerationConfig {
        sid: speaker,
        speed,
        ..Default::default()
    };
    let audio = engine
        .generate_with_config(text, &generation, None::<fn(&[f32], f32) -> bool>)
        .ok_or_else(|| "语音合成失败".to_string())?;

    Ok(AudioBuffer::new(
        audio.samples().to_vec(),
        audio.sample_rate(),
        1,
    ))
}

fn build_engine(config: &TtsEngineConfig) -> Result<OfflineTts, String> {
    let tts_config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            kokoro: kokoro_config(&config.model),
            vits: vits_config(&config.model),
            num_threads: config.num_threads,
            provider: Some(config.provider.clone()),
            debug: false,
            ..Default::default()
        },
        rule_fsts: config.rule_fsts.clone(),
        rule_fars: None,
        max_num_sentences: 1,
        silence_scale: 0.2,
    };

    // create 对配置错误只返回 None，拿不到具体原因，所以这里把关键路径带上便于排查。
    OfflineTts::create(&tts_config).ok_or_else(|| {
        format!(
            "创建 sherpa-onnx TTS 失败，请检查模型文件是否存在：{}",
            describe_model(&config.model)
        )
    })
}

fn kokoro_config(model: &TtsModel) -> OfflineTtsKokoroModelConfig {
    match model {
        TtsModel::Kokoro {
            model,
            tokens,
            voices,
            data_dir,
            dict_dir,
            lexicon,
        } => OfflineTtsKokoroModelConfig {
            model: Some(model.clone()),
            tokens: Some(tokens.clone()),
            voices: voices.clone(),
            data_dir: data_dir.clone(),
            dict_dir: dict_dir.clone(),
            lexicon: lexicon.clone(),
            length_scale: 1.0,
            lang: None,
        },
        _ => OfflineTtsKokoroModelConfig::default(),
    }
}

fn vits_config(model: &TtsModel) -> OfflineTtsVitsModelConfig {
    match model {
        TtsModel::Vits {
            model,
            tokens,
            data_dir,
            lexicon,
        } => OfflineTtsVitsModelConfig {
            model: Some(model.clone()),
            tokens: Some(tokens.clone()),
            data_dir: data_dir.clone(),
            lexicon: lexicon.clone(),
            ..Default::default()
        },
        _ => OfflineTtsVitsModelConfig::default(),
    }
}

fn describe_model(model: &TtsModel) -> &str {
    match model {
        TtsModel::Kokoro { model, .. } | TtsModel::Vits { model, .. } => model,
    }
}
