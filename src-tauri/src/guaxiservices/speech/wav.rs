//! 语音服务共用的音频缓冲与 WAV 编解码。
//!
//! 进程内调用拿到的和要写出的都是裸采样，不再经过临时文件，所以这里只保留
//! 编解码本身：PCM16 / f32 的 WAV 读写，以及线性重采样。

/// 一段交错存储的浮点音频。
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: i32,
    pub channels: u16,
}

impl AudioBuffer {
    pub fn new(samples: Vec<f32>, sample_rate: i32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            channels: channels.max(1),
        }
    }

    /// 编码成 16-bit PCM WAV（单声道/多声道按 `channels` 交错写出）。
    pub fn to_pcm16_wav(&self) -> Vec<u8> {
        let channels = self.channels.max(1);
        let sample_rate = self.sample_rate.max(1) as u32;
        let bits_per_sample: u16 = 16;
        let block_align = channels * bits_per_sample / 8;
        let byte_rate = sample_rate * block_align as u32;
        let data_size = (self.samples.len() * 2) as u32;

        let mut out = Vec::with_capacity(44 + data_size as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_size).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits_per_sample.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_size.to_le_bytes());
        for sample in &self.samples {
            let clamped = sample.clamp(-1.0, 1.0);
            out.extend_from_slice(&((clamped * i16::MAX as f32) as i16).to_le_bytes());
        }
        out
    }

    /// 解码 WAV 字节并重采样到 `target_rate`，输出始终为单声道——
    /// 语音识别与 whisper 都只吃单声道。
    pub fn decode_to_mono(bytes: &[u8], target_rate: i32) -> Result<Self, String> {
        let (fmt, data) = parse_wav_chunks(bytes)?;
        let audio_format = u16::from_le_bytes([fmt[0], fmt[1]]);
        let channels = u16::from_le_bytes([fmt[2], fmt[3]]).max(1);
        let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]) as i32;
        let bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
        if sample_rate <= 0 {
            return Err("WAV 采样率无效".to_string());
        }

        // 先解成交错 f32。
        let interleaved: Vec<f32> = match (audio_format, bits_per_sample) {
            (1, 16) => data
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / i16::MAX as f32)
                .collect(),
            (3, 32) => data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
            _ => {
                return Err(format!(
                    "不支持的 WAV 格式：format={} bits={}（仅支持 16-bit PCM 或 32-bit float）",
                    audio_format, bits_per_sample
                ))
            }
        };

        // 多声道下混成单声道。
        let mono: Vec<f32> = if channels == 1 {
            interleaved
        } else {
            let ch = channels as usize;
            interleaved
                .chunks_exact(ch)
                .map(|frame| frame.iter().sum::<f32>() / ch as f32)
                .collect()
        };

        let samples = if sample_rate == target_rate {
            mono
        } else {
            resample_linear(&mono, sample_rate, target_rate)
        };
        Ok(AudioBuffer::new(samples, target_rate, 1))
    }
}

/// 线性插值重采样。语音识别对轻微的音质损失不敏感，不值得为此引入重采样库。
fn resample_linear(input: &[f32], from_rate: i32, to_rate: i32) -> Vec<f32> {
    if input.is_empty() || from_rate <= 0 || to_rate <= 0 || from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// 取出 WAV 的 `fmt ` 与 `data` 两个 chunk 载荷。
/// 遍历 chunk 而不是按固定偏移读，才能容忍 `LIST`/`fact` 等前置块。
fn parse_wav_chunks(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("不是有效的 WAV 文件".to_string());
    }
    let mut cursor = 12usize;
    let mut fmt: Option<Vec<u8>> = None;
    let mut data: Option<Vec<u8>> = None;

    while cursor + 8 <= bytes.len() {
        let id = &bytes[cursor..cursor + 4];
        let size = u32::from_le_bytes([
            bytes[cursor + 4],
            bytes[cursor + 5],
            bytes[cursor + 6],
            bytes[cursor + 7],
        ]) as usize;
        let body_start = cursor + 8;
        let body_end = body_start.saturating_add(size).min(bytes.len());

        if id == b"fmt " {
            fmt = Some(bytes[body_start..body_end].to_vec());
        } else if id == b"data" {
            data = Some(bytes[body_start..body_end].to_vec());
        }

        // chunk 按偶数字节对齐。
        cursor = body_start + size + (size & 1);
    }

    match (fmt, data) {
        (Some(fmt), Some(data)) => {
            if fmt.len() < 16 {
                return Err("WAV fmt chunk 不完整".to_string());
            }
            Ok((fmt, data))
        }
        (None, _) => Err("WAV 缺少 fmt chunk".to_string()),
        _ => Err("WAV 缺少 data chunk".to_string()),
    }
}
