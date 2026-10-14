//! 进程内语音服务：TTS 与 ASR（含 whisper 在内的各识别模型族）。
//!
//! 这些链路原先都靠启动外部可执行文件完成，每次调用都要重新加载模型。
//! 这里把引擎常驻在进程内，并统一提供 `async` 入口——底层的合成与识别是阻塞的，
//! 全部包在 `spawn_blocking` 里，调用方不需要关心。

pub mod asr;
pub mod cache;
pub mod config;
pub mod punct;
pub mod tts;
pub mod wav;

pub use asr::{AsrEngineConfig, AsrSegment};
pub use config::{
    normalize_provider, resolve_asr_engine_config, resolve_punctuation_model,
    resolve_tts_engine_config, resolve_whisper_onnx_engine_config,
};
pub use tts::TtsEngineConfig;
pub use wav::AudioBuffer;

/// 合成语音，返回 PCM16 WAV 字节。
pub async fn synthesize_wav(
    config: TtsEngineConfig,
    text: String,
    speaker: i32,
    speed: f32,
) -> Result<Vec<u8>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = tts::synthesize(&config, &text, speaker, speed)?;
        Ok(audio.to_pcm16_wav())
    })
    .await
    .map_err(|e| format!("语音合成任务失败：{}", e))?
}

/// 识别 PCM16 WAV 字节里的语音。内部会重采样到识别器要求的 16kHz 单声道。
///
/// 识别结果会尽量补上标点：sherpa 的识别模型本身不输出标点，
/// 实时听写攒出来就是一整段没有断句的字。标点模型缺失时原样返回，不影响识别。
pub async fn transcribe_wav(config: AsrEngineConfig, wav_bytes: Vec<u8>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = AudioBuffer::decode_to_mono(&wav_bytes, asr::ASR_SAMPLE_RATE)?;
        let text = asr::transcribe(&config, &audio.samples)?;
        Ok(match resolve_punctuation_model() {
            Some(model) => punct::add_punctuation(&model, &config.provider, &text),
            None => text,
        })
    })
    .await
    .map_err(|e| format!("语音识别任务失败：{}", e))?
}

/// 用 VAD 切段后逐段识别，返回带时间轴的分段（语言学习用）。
pub async fn transcribe_segments_wav(
    config: AsrEngineConfig,
    vad_model: std::path::PathBuf,
    wav_bytes: Vec<u8>,
) -> Result<Vec<AsrSegment>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let audio = AudioBuffer::decode_to_mono(&wav_bytes, asr::ASR_SAMPLE_RATE)?;
        asr::transcribe_segments_with_vad(&vad_model, &config, &audio.samples)
    })
    .await
    .map_err(|e| format!("语音转写任务失败：{}", e))?
}
