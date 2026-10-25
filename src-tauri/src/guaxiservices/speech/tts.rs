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
    /// 实例标签，参与缓存键，用来给不同**用途**分配互不阻塞的引擎实例。
    ///
    /// 为什么需要：`synthesize` 在整段生成期间一直持有实例锁，所以共用实例的调用方
    /// 会互相排队——逐句朗读这种长任务会把划词朗读、有声书堵在门外。换一个标签就拿到
    /// 独立实例。默认空串，即与改动前"大家共用同一个实例"的行为完全一致。
    pub instance_tag: String,
}

impl TtsEngineConfig {
    /// 派生一份"用途隔离 + 线程预算"的副本：只换实例标签与线程数，模型等参数不动。
    pub fn with_instance(mut self, tag: &str, num_threads: i32) -> Self {
        self.instance_tag = tag.to_string();
        self.num_threads = num_threads.max(1);
        self
    }

    /// 缓存键：任一参数变化都必须换实例，否则会拿旧模型合成。
    fn cache_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!(
            "{:?}|{}|{}|{}|{}",
            self.model, self.rule_fsts.is_some(), self.provider, self.num_threads, self.instance_tag
        )
        .hash(&mut hasher);
        if let Some(rule_fsts) = &self.rule_fsts {
            rule_fsts.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// 卸载该配置对应的引擎实例，回收其占用的内存（会话结束时用，见 `EngineCache::release`）。
/// 返回是否真的卸载了。
pub fn release_engine(config: &TtsEngineConfig) -> bool {
    TTS_CACHE.release(config.cache_key())
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

#[cfg(test)]
mod tmp_gpu_smoke {
    use super::*;

    /// 临时冒烟：在 gpu 构建里直接以 cuda provider 建引擎并合成，
    /// 用来验证「原始 LoadLibrary 探测 1114」是否是误报。
    #[test]
    fn tmp_gpu_tts_smoke() {
        if !cfg!(feature = "gpu") {
            return;
        }
        let base =
            r"D:\MyProject\Guaxi\Guaxi\guaxi\src-tauri\utils\Models\tts\kokoro_multi_lang";
        let cfg = TtsEngineConfig {
            model: TtsModel::Kokoro {
                model: format!("{base}\model.onnx"),
                tokens: format!("{base}\tokens.txt"),
                voices: Some(format!("{base}\voices.bin")),
                data_dir: Some(format!("{base}\espeak-ng-data")),
                dict_dir: None,
                lexicon: Some(format!("{base}\lexicon-us-en.txt")),
            },
            rule_fsts: None,
            provider: "cuda".to_string(),
            num_threads: 2,
            instance_tag: "tmp-gpu-smoke".to_string(),
        };
        let audio = synthesize(&cfg, "Hello world", 0, 1.0).expect("cuda 合成应成功");
        println!("samples={} rate={}", audio.samples.len(), audio.sample_rate);
        assert!(!audio.samples.is_empty());
        release_engine(&cfg);
    }
}
