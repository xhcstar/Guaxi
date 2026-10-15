//! 音频容器解码：把任意容器解成识别器要的裸采样。
//!
//! 识别器（VAD + whisper）只吃单声道 f32，而采集来的听力基本都是播客 mp3/m4a。
//! WAV 继续走 `wav.rs` 里那套零依赖解析（没有额外开销），其它容器交给 symphonia：
//! 它是纯 Rust 实现，不引入运行时外部工具，也不需要在分发时多带一个可执行文件。

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use super::wav::{resample_linear, AudioBuffer};

fn is_wav_header(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
}

fn file_is_wav(path: &Path) -> bool {
    let mut header = [0u8; 12];
    match File::open(path).and_then(|mut file| file.read_exact(&mut header)) {
        Ok(()) => is_wav_header(&header),
        Err(_) => false,
    }
}

/// 解码音频文件到 `target_rate` 单声道 f32。
///
/// 只有 WAV 会整块读进内存（它的解析要按 chunk 遍历）；其余容器直接从文件流式解，
/// 不再把整集播客读成一份等长字节。
pub fn decode_file_to_mono(path: &Path, target_rate: i32) -> Result<AudioBuffer, String> {
    if file_is_wav(path) {
        let bytes =
            fs::read(path).map_err(|e| format!("读取音频失败（{}）：{}", path.display(), e))?;
        return AudioBuffer::decode_to_mono(&bytes, target_rate);
    }
    let file =
        File::open(path).map_err(|e| format!("打开音频失败（{}）：{}", path.display(), e))?;
    decode_container(file, target_rate)
}

fn decode_container<S: MediaSource + 'static>(
    source: S,
    target_rate: i32,
) -> Result<AudioBuffer, String> {
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut reader = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("无法识别音频容器：{}", e))?;

    let (track_id, params) = {
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| "音频文件里没有可用的音轨".to_string())?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .cloned()
            .ok_or_else(|| "音轨缺少音频编码参数".to_string())?;
        (track.id, params)
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| format!("不支持的音频编码：{}", e))?;

    let mut mono: Vec<f32> = Vec::new();
    let mut interleaved: Vec<f32> = Vec::new();
    let mut sample_rate = 0i32;

    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // 偶发坏包跳过：整段播客里有一帧损坏不值得让整条转写失败。
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::ResetRequired) => break,
            Err(err) => return Err(format!("读取音频数据失败：{}", err)),
        };
        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::ResetRequired) => break,
            Err(err) => return Err(format!("解码音频失败：{}", err)),
        };
        let spec = decoded.spec();
        sample_rate = spec.rate() as i32;
        let channels = spec.channels().count().max(1);
        interleaved.clear();
        decoded.copy_to_vec_interleaved(&mut interleaved);
        if channels == 1 {
            mono.extend_from_slice(&interleaved);
        } else {
            // 多声道下混成单声道，与 wav.rs 的做法保持一致。
            mono.extend(
                interleaved
                    .chunks_exact(channels)
                    .map(|frame| frame.iter().sum::<f32>() / channels as f32),
            );
        }
    }

    if mono.is_empty() {
        return Err("音频解码后没有采样数据".to_string());
    }
    if sample_rate <= 0 {
        return Err("音频采样率无效".to_string());
    }
    let samples = if sample_rate == target_rate {
        mono
    } else {
        resample_linear(&mono, sample_rate, target_rate)
    };
    Ok(AudioBuffer::new(samples, target_rate, 1))
}
