//! sherpa-onnx 离线语音识别的进程内封装。
//!
//! 取代原来的 `sherpa-onnx-offline.exe` 子进程：识别器常驻，音频直接在内存里
//! 传进去，不再落临时文件、也不再靠 ffmpeg 解容器。

use std::hash::{Hash, Hasher};
use std::path::Path;

use once_cell::sync::Lazy;
use sherpa_onnx::{
    OfflineModelConfig, OfflineNemoEncDecCtcModelConfig, OfflineParaformerModelConfig,
    OfflineRecognizer, OfflineRecognizerConfig, OfflineRecognizerResult, OfflineSenseVoiceModelConfig,
    OfflineTransducerModelConfig, OfflineWhisperModelConfig, SileroVadModelConfig, VadModelConfig,
    VoiceActivityDetector,
};

use super::cache::EngineCache;

static ASR_CACHE: Lazy<EngineCache<OfflineRecognizer>> = Lazy::new(EngineCache::new);

/// 识别器只吃 16kHz。外部传来的音频必须先重采样到这个值。
pub const ASR_SAMPLE_RATE: i32 = 16_000;

/// 支持的离线识别模型族，与旧 CLI 按目录/文件探测出来的种类一一对应。
#[derive(Debug, Clone, PartialEq)]
pub enum AsrModel {
    SenseVoice {
        model: String,
        tokens: String,
        language: Option<String>,
        use_itn: bool,
    },
    Paraformer {
        model: String,
        tokens: String,
    },
    Transducer {
        encoder: String,
        decoder: String,
        joiner: String,
        tokens: String,
    },
    Whisper {
        encoder: String,
        decoder: String,
        tokens: String,
        language: Option<String>,
    },
    NemoCtc {
        model: String,
        tokens: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrEngineConfig {
    pub model: AsrModel,
    /// "cpu" | "cuda" | "coreml" | ...
    pub provider: String,
    pub num_threads: i32,
}

impl AsrEngineConfig {
    fn cache_key(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}|{}|{}", self.model, self.provider, self.num_threads).hash(&mut hasher);
        hasher.finish()
    }
}

/// 带时间轴的分段，时间单位为秒。
#[derive(Debug, Clone)]
pub struct AsrSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// 识别一段 16kHz 单声道音频，只取文本。
///
/// 阻塞调用，调用方必须放在 `spawn_blocking` 里。
pub fn transcribe(config: &AsrEngineConfig, samples: &[f32]) -> Result<String, String> {
    Ok(run(config, samples)?.text.trim().to_string())
}

/// Silero VAD 的切段参数。max_speech_duration 兜底防止一整段朗读被切成一条超长记录。
const VAD_THRESHOLD: f32 = 0.5;
const VAD_MIN_SILENCE_SECS: f32 = 0.35;
const VAD_MIN_SPEECH_SECS: f32 = 0.25;
const VAD_MAX_SPEECH_SECS: f32 = 20.0;
/// Silero 在 16kHz 下的标准窗口。
const VAD_WINDOW_SIZE: i32 = 512;

/// 先用 VAD 切出语音段，再逐段识别，从而拿到带时间轴的分段。
///
/// 为什么不走模型自带的时间戳：sherpa 的预编译 whisper 模型缺少 cross-attention 输出，
/// 无论开 token 还是 segment 时间戳都拿不到数据（官方要求另行导出模型）。
/// 改用 VAD 定边界既绕开了这个限制，也让「一段话一条记录」的粒度更贴合语言学习。
pub fn transcribe_segments_with_vad(
    vad_model: &Path,
    config: &AsrEngineConfig,
    samples: &[f32],
) -> Result<Vec<AsrSegment>, String> {
    if samples.is_empty() {
        return Err("音频为空，无法识别".to_string());
    }

    // VAD 是有状态的流式检测器，每次调用新建一个，避免上一段音频的状态串到下一次。
    let vad_config = VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(vad_model.to_string_lossy().to_string()),
            threshold: VAD_THRESHOLD,
            min_silence_duration: VAD_MIN_SILENCE_SECS,
            min_speech_duration: VAD_MIN_SPEECH_SECS,
            window_size: VAD_WINDOW_SIZE,
            max_speech_duration: VAD_MAX_SPEECH_SECS,
        },
        sample_rate: ASR_SAMPLE_RATE,
        num_threads: 1,
        provider: Some(config.provider.clone()),
        debug: false,
        ..Default::default()
    };
    let vad = VoiceActivityDetector::create(&vad_config, 30.0)
        .ok_or_else(|| format!("创建 VAD 失败，请检查模型文件：{}", vad_model.display()))?;

    for chunk in samples.chunks(VAD_WINDOW_SIZE as usize) {
        vad.accept_waveform(chunk);
    }
    // 把尾部还没闭合的语音段冲出来，否则最后一句会丢。
    vad.flush();

    let mut segments = Vec::new();
    while let Some(segment) = vad.front() {
        let start = segment.start() as f64 / ASR_SAMPLE_RATE as f64;
        let duration = segment.samples().len() as f64 / ASR_SAMPLE_RATE as f64;
        let text = match transcribe(config, segment.samples()) {
            Ok(text) => text,
            Err(err) => {
                vad.pop();
                return Err(err);
            }
        };
        if !text.is_empty() {
            segments.push(AsrSegment {
                start: start.max(0.0),
                end: (start + duration).max(start + 0.4),
                text,
            });
        }
        vad.pop();
    }

    Ok(segments)
}

/// 跑一次识别，拿原始结果（文本 + token 级时间戳）。
fn run(config: &AsrEngineConfig, samples: &[f32]) -> Result<OfflineRecognizerResult, String> {
    if samples.is_empty() {
        return Err("音频为空，无法识别".to_string());
    }

    let cached = ASR_CACHE.get_or_create(config.cache_key(), || build_recognizer(config))?;
    let recognizer = cached.lock();

    let stream = recognizer.create_stream();
    stream.accept_waveform(ASR_SAMPLE_RATE, samples);
    recognizer.decode(&stream);

    stream
        .get_result()
        .ok_or_else(|| "语音识别没有返回结果".to_string())
}

fn build_recognizer(config: &AsrEngineConfig) -> Result<OfflineRecognizer, String> {
    let mut base = OfflineRecognizerConfig::default();
    base.model_config = model_config(&config.model, &config.provider, config.num_threads);

    // create 对配置错误只返回 None，带上模型路径便于排查。
    OfflineRecognizer::create(&base)
        .ok_or_else(|| format!("创建 sherpa-onnx 识别器失败，请检查模型文件：{}", describe(&config.model)))
}

fn model_config(model: &AsrModel, provider: &str, num_threads: i32) -> OfflineModelConfig {
    let mut config = OfflineModelConfig {
        provider: Some(provider.to_string()),
        num_threads,
        debug: false,
        ..Default::default()
    };

    match model {
        AsrModel::SenseVoice {
            model,
            tokens,
            language,
            use_itn,
        } => {
            config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(model.clone()),
                language: language.clone(),
                use_itn: *use_itn,
            };
            config.tokens = Some(tokens.clone());
        }
        AsrModel::Paraformer { model, tokens } => {
            config.paraformer = OfflineParaformerModelConfig {
                model: Some(model.clone()),
            };
            config.tokens = Some(tokens.clone());
        }
        AsrModel::Transducer {
            encoder,
            decoder,
            joiner,
            tokens,
        } => {
            config.transducer = OfflineTransducerModelConfig {
                encoder: Some(encoder.clone()),
                decoder: Some(decoder.clone()),
                joiner: Some(joiner.clone()),
            };
            config.tokens = Some(tokens.clone());
        }
        AsrModel::Whisper {
            encoder,
            decoder,
            tokens,
            language,
        } => {
            config.whisper = OfflineWhisperModelConfig {
                encoder: Some(encoder.clone()),
                decoder: Some(decoder.clone()),
                language: language.clone(),
                task: Some("transcribe".to_string()),
                // 语言学习的同步 JSON 需要时间轴，只有开了这个才有逐 token 时间戳。
                enable_token_timestamps: true,
                ..Default::default()
            };
            config.tokens = Some(tokens.clone());
        }
        AsrModel::NemoCtc { model, tokens } => {
            config.nemo_ctc = OfflineNemoEncDecCtcModelConfig {
                model: Some(model.clone()),
            };
            config.tokens = Some(tokens.clone());
        }
    }

    config
}

fn describe(model: &AsrModel) -> &str {
    match model {
        AsrModel::SenseVoice { model, .. }
        | AsrModel::Paraformer { model, .. }
        | AsrModel::NemoCtc { model, .. } => model,
        AsrModel::Transducer { encoder, .. } | AsrModel::Whisper { encoder, .. } => encoder,
    }
}
