use crate::commands::settingview;
use encoding_rs::GBK;
use lopdf::Document as LoDocument;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::Html;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Instant;
use tauri::{Emitter, Window};
use tracing::{info, warn};
use zip::ZipArchive;

const DEFAULT_MODEL: &str = "utils/Models/tts/kokoro_multi_lang/model.onnx";
const DEFAULT_TOKENS: &str = "utils/Models/tts/kokoro_multi_lang/tokens.txt";
/// 默认跟着硬件走：交给 speech::config 的探测决定用 CUDA 还是 CPU。
const DEFAULT_EXECUTION_PROVIDER: &str = "auto";
const TTS_BOOK_STATUS_QUEUED: &str = "queued";
const TTS_BOOK_STATUS_RUNNING: &str = "running";
const TTS_BOOK_STATUS_PAUSED: &str = "paused";
const TTS_BOOK_STATUS_STOPPED: &str = "stopped";
const TTS_BOOK_STATUS_SUCCESS: &str = "success";
const TTS_BOOK_INDEX_FILE: &str = "tts_book_resume_index.json";
const TTS_BOOK_PAUSE_SIGNAL: &str = "__TTS_BOOK_PAUSED__";
const TTS_BOOK_STOP_SIGNAL: &str = "__TTS_BOOK_STOPPED__";

#[derive(Debug, Clone, Default)]
struct TtsBookTaskControl {
    pause_requested: bool,
    stop_requested: bool,
}

static TTS_BOOK_TASK_CONTROLS: Lazy<Mutex<HashMap<String, TtsBookTaskControl>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
/// 第三方解析库（pdf-extract / anydoc / ebook-rs）都可能被畸形输入触发 panic。
/// panic hook 是进程级全局状态，并发替换会互相覆盖，所以本文件所有解析器共用这一把锁。
static PARSER_PANIC_HOOK_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Deserialize)]
pub struct TtsBookConvertRequest {
    pub file_path: String,
    pub output_dir: String,
    #[serde(default)]
    pub output_name: String,
    #[serde(default)]
    pub tts_model: String,
    #[serde(default)]
    pub tts_tokens: String,
    #[serde(default)]
    pub tts_mode_hint: String,
    #[serde(default)]
    pub speaker_id: String,
    #[serde(default)]
    pub speed: Option<f32>,
    #[serde(default)]
    pub voice_zh: String,
    #[serde(default)]
    pub voice_en: String,
    #[serde(default)]
    pub split_by_toc: bool,
    pub max_chunk_chars: Option<usize>,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub execution_provider: String,
}

#[derive(Debug, Serialize)]
pub struct TtsBookConvertResponse {
    pub task_id: String,
    pub output_path: String,
    pub segment_json_path: String,
    pub title: String,
    pub total_chars: usize,
    pub total_chunks: usize,
    pub zh_chunks: usize,
    pub en_chunks: usize,
    pub chapter_count: usize,
    pub chapter_output_paths: Vec<String>,
    pub toc: Vec<String>,
}

#[derive(Debug)]
struct ExtractedDoc {
    title: String,
    text: String,
    toc: Vec<String>,
}

#[derive(Debug, Clone)]
struct TextChunk {
    index: usize,
    text: String,
    lang: ChunkLang,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkLang {
    Zh,
    En,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
struct TtsBookProgress {
    task_id: String,
    file_path: String,
    file_name: String,
    stage: String,
    current: usize,
    total: usize,
    progress: u8,
    message: String,
    resumed: bool,
    failed: bool,
    output_path: String,
    segment_json_path: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct TtsBookResumeState {
    version: u32,
    task_id: String,
    file_name: String,
    title: String,
    output_dir: String,
    task_output_dir: String,
    output_name: String,
    tts_model: String,
    tts_tokens: String,
    tts_mode_hint: String,
    speaker_id: String,
    speed: Option<f32>,
    execution_provider: String,
    status: String,
    last_stage: String,
    last_message: String,
    progress: u8,
    source_file: String,
    source_size: u64,
    source_modified_ms: u64,
    cleaned_text_hash: u64,
    max_chunk_chars: usize,
    split_by_toc: bool,
    total_chunks: usize,
    completed_chunks: Vec<usize>,
    completed_chapters: Vec<usize>,
    output_path: String,
    segment_json_path: String,
    chapter_output_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct TtsBookResumeIndex {
    tasks: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TtsBookSavedTask {
    pub task_id: String,
    pub file_path: String,
    pub file_name: String,
    pub title: String,
    pub output_dir: String,
    pub output_name: String,
    pub tts_model: String,
    pub tts_tokens: String,
    pub tts_mode_hint: String,
    pub speaker_id: String,
    pub speed: Option<f32>,
    pub split_by_toc: bool,
    pub max_chunk_chars: usize,
    pub execution_provider: String,
    pub status: String,
    pub message: String,
    pub progress: u8,
    pub output_path: String,
    pub segment_json_path: String,
    pub total_chunks: usize,
    pub completed_chunks: usize,
    pub chapter_count: usize,
    pub completed_chapters: usize,
    pub chapter_output_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SegmentManifest {
    title: String,
    source_file: String,
    total_chars: usize,
    total_chunks: usize,
    toc: Vec<String>,
    chunks: Vec<SegmentEntry>,
    chapters: Vec<ChapterSegmentEntry>,
}

#[derive(Debug, Serialize)]
struct SegmentEntry {
    index: usize,
    lang: &'static str,
    kind: &'static str,
    speaker_hint: &'static str,
    pause_after_ms: u32,
    audio_duration_ms: u64,
    start_ms: u64,
    end_ms: u64,
    char_count: usize,
    text: String,
}

#[derive(Debug, Serialize)]
struct ChapterSegmentEntry {
    index: usize,
    title: String,
    char_count: usize,
    chunk_count: usize,
    text: String,
}

#[tauri::command]
pub async fn tts_book_convert(
    request: TtsBookConvertRequest,
    window: Window,
) -> Result<TtsBookConvertResponse, String> {
    let total_start = Instant::now();
    let tts_cfg = settingview::get_tts_config();
    let file_path = PathBuf::from(request.file_path.trim());
    let file_name = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let task_id = if request.task_id.trim().is_empty() {
        format!("{}-{}", file_name, unix_now_ms())
    } else {
        request.task_id.trim().to_string()
    };
    reset_task_control(&task_id);
    info!(
        file_path = file_path.to_string_lossy().as_ref(),
        output_dir = request.output_dir.as_str(),
        split_by_toc = request.split_by_toc,
        max_chunk_chars = request.max_chunk_chars.unwrap_or(180),
        task_id = task_id.as_str(),
        "tts_book_convert start"
    );
    if !file_path.exists() || !file_path.is_file() {
        return Err("输入文件不存在或不是文件".to_string());
    }

    let output_dir = if request.output_dir.trim().is_empty() {
        PathBuf::from(tts_cfg.output_dir.trim())
    } else {
        PathBuf::from(request.output_dir.trim())
    };
    if output_dir.as_os_str().is_empty() {
        return Err("output_dir 不能为空".to_string());
    }
    fs::create_dir_all(&output_dir).map_err(|e| format!("创建输出目录失败: {}", e))?;

    emit_tts_book_progress(
        &window,
        TtsBookProgress {
            task_id: task_id.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            file_name: file_name.clone(),
            stage: "prepare".to_string(),
            current: 0,
            total: 1,
            progress: 0,
            message: "开始解析文档".to_string(),
            resumed: false,
            failed: false,
            output_path: String::new(),
            segment_json_path: String::new(),
        },
    )?;

    let extracted = extract_document(&file_path).await?;
    let cleaned = clean_text_for_tts_pro(&extracted.text);
    if cleaned.trim().is_empty() {
        return Err("未提取到可朗读文本".to_string());
    }

    let max_chunk_chars = request.max_chunk_chars.unwrap_or(180).clamp(80, 420);

    // 先切章节、再在章节内部切分。这样整本音频就是所有分段的顺序拼接，
    // 章节音频只是取其中一段切片——每段只合成一次。
    // 旧实现先把整本逐段合成一遍、再逐章重新合成一遍，默认配置下 TTS 白跑一倍。
    let chapters = if request.split_by_toc {
        split_text_into_chapters(&cleaned, &extracted.toc)
    } else {
        Vec::new()
    };
    info!(chapter_count = chapters.len(), "tts_book_convert chapter split done");

    // 未做章节拆分时把整份文本当作唯一一章来切分，后面的装配逻辑就不必再分叉。
    let chunk_sources: Vec<&str> = if chapters.is_empty() {
        vec![cleaned.as_str()]
    } else {
        chapters.iter().map(|(_, text)| text.as_str()).collect()
    };

    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut chapter_chunk_ranges: Vec<std::ops::Range<usize>> = Vec::with_capacity(chunk_sources.len());
    for source in &chunk_sources {
        let start = chunks.len();
        chunks.extend(split_text_chunks_pro(source, max_chunk_chars));
        chapter_chunk_ranges.push(start..chunks.len());
    }
    // split_text_chunks_pro 的 index 是各章从 1 重新数的，这里统一改成全局序号。
    for (position, chunk) in chunks.iter_mut().enumerate() {
        chunk.index = position + 1;
    }

    info!(
        cleaned_chars = cleaned.chars().count(),
        chunk_count = chunks.len(),
        max_chunk_chars,
        "tts_book_convert chunked"
    );
    if chunks.is_empty() {
        return Err("文本分段失败".to_string());
    }

    let stem = sanitize_file_stem(if request.output_name.trim().is_empty() {
        extracted.title.as_str()
    } else {
        request.output_name.trim()
    });
    let task_output_dir = output_dir.join(&stem);
    fs::create_dir_all(&task_output_dir).map_err(|e| format!("创建任务输出目录失败: {}", e))?;
    let output_path = task_output_dir.join(format!("{}_audiobook.wav", stem));
    let segment_json_path = task_output_dir.join(format!("{}_segments.json", stem));
    let cleaned_text_hash = hash_text(&cleaned);
    let work_dir = task_output_dir.join(format!(
        "{}_tts_work_{:016x}_{}_{}",
        stem,
        cleaned_text_hash,
        max_chunk_chars,
        // -v2：分段方案改成「先分章再分段」，段号语义随之变化，
        // 旧缓存目录必须作废，否则会复用错位的分段音频。
        if request.split_by_toc { "chap-v2" } else { "full-v2" }
    ));
    let chunk_dir = work_dir.join("chunks");
    fs::create_dir_all(&chunk_dir).map_err(|e| format!("创建分段缓存目录失败: {}", e))?;
    let resume_state_path = work_dir.join("resume_state.json");

    let (source_size, source_modified_ms) = file_signature(&file_path)?;
    let mut resume_state = load_resume_state(&resume_state_path).unwrap_or_default();
    let resume_matches = resume_state.version == 1
        && resume_state.source_file == file_path.to_string_lossy()
        && resume_state.source_size == source_size
        && resume_state.source_modified_ms == source_modified_ms
        && resume_state.cleaned_text_hash == cleaned_text_hash
        && resume_state.max_chunk_chars == max_chunk_chars
        && resume_state.split_by_toc == request.split_by_toc
        && resume_state.total_chunks == chunks.len();
    if !resume_matches {
        resume_state = TtsBookResumeState {
            version: 1,
            task_id: task_id.clone(),
            file_name: file_name.clone(),
            title: extracted.title.clone(),
            output_dir: output_dir.to_string_lossy().to_string(),
            task_output_dir: task_output_dir.to_string_lossy().to_string(),
            output_name: request.output_name.trim().to_string(),
            tts_model: String::new(),
            tts_tokens: String::new(),
            tts_mode_hint: String::new(),
            speaker_id: String::new(),
            speed: None,
            execution_provider: DEFAULT_EXECUTION_PROVIDER.to_string(),
            status: TTS_BOOK_STATUS_QUEUED.to_string(),
            last_stage: "prepare".to_string(),
            last_message: "等待处理".to_string(),
            progress: 0,
            source_file: file_path.to_string_lossy().to_string(),
            source_size,
            source_modified_ms,
            cleaned_text_hash,
            max_chunk_chars,
            split_by_toc: request.split_by_toc,
            total_chunks: chunks.len(),
            completed_chunks: Vec::new(),
            completed_chapters: Vec::new(),
            output_path: output_path.to_string_lossy().to_string(),
            segment_json_path: segment_json_path.to_string_lossy().to_string(),
            chapter_output_paths: Vec::new(),
        };
        save_resume_state(&resume_state_path, &resume_state)?;
    }

    let tts_exe = tts_cfg.exe.trim().to_string();
    let tts_model = if request.tts_model.trim().is_empty() {
        let model = tts_cfg.model.trim();
        if model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            model.to_string()
        }
    } else {
        request.tts_model.trim().to_string()
    };
    let tts_tokens = if request.tts_tokens.trim().is_empty() {
        let tokens = tts_cfg.tokens.trim();
        if tokens.is_empty() {
            DEFAULT_TOKENS.to_string()
        } else {
            tokens.to_string()
        }
    } else {
        request.tts_tokens.trim().to_string()
    };
    let tts_mode_hint = if request.tts_mode_hint.trim().is_empty() {
        infer_tts_mode_hint(&tts_model)
    } else {
        request.tts_mode_hint.trim().to_string()
    };
    let speaker_id = if request.speaker_id.trim().is_empty() {
        if !request.voice_zh.trim().is_empty() {
            request.voice_zh.trim().to_string()
        } else if !request.voice_en.trim().is_empty() {
            request.voice_en.trim().to_string()
        } else {
            "0".to_string()
        }
    } else {
        request.speaker_id.trim().to_string()
    };
    let speed = request.speed.map(|value| value.clamp(0.2, 3.0));
    let execution_provider = normalize_execution_provider(&request.execution_provider);
    resume_state.task_id = task_id.clone();
    resume_state.file_name = file_name.clone();
    resume_state.title = extracted.title.clone();
    resume_state.output_dir = output_dir.to_string_lossy().to_string();
    resume_state.task_output_dir = task_output_dir.to_string_lossy().to_string();
    resume_state.output_name = request.output_name.trim().to_string();
    resume_state.tts_model = tts_model.clone();
    resume_state.tts_tokens = tts_tokens.clone();
    resume_state.tts_mode_hint = tts_mode_hint.clone();
    resume_state.speaker_id = speaker_id.clone();
    resume_state.speed = speed;
    resume_state.execution_provider = execution_provider.clone();
    resume_state.status = TTS_BOOK_STATUS_RUNNING.to_string();
    resume_state.last_stage = "prepare".to_string();
    resume_state.last_message = "开始解析文档".to_string();
    resume_state.progress = 0;
    register_resume_state(&resume_state_path)?;
    save_resume_state(&resume_state_path, &resume_state)?;

    info!(
        tts_exe = tts_exe.as_str(),
        tts_model = tts_model.as_str(),
        tts_tokens = tts_tokens.as_str(),
        tts_mode_hint = tts_mode_hint.as_str(),
        speaker_id = speaker_id.as_str(),
        speed = speed.unwrap_or(1.0),
        execution_provider = execution_provider.as_str(),
        "tts_book_convert tts config resolved"
    );

    let total_steps = chunks.len() + chapters.len() + 2;
    let mut completed_steps = 0usize;
    let mut resumed_any = false;

    write_segment_manifest(
        &segment_json_path,
        &SegmentManifest {
            title: extracted.title.clone(),
            source_file: file_path.to_string_lossy().to_string(),
            total_chars: cleaned.chars().count(),
            total_chunks: chunks.len(),
            toc: extracted.toc.clone(),
            chunks: build_segment_entries(&chunks, &vec![0; chunks.len()]),
            chapters: chapters
                .iter()
                .enumerate()
                .map(|(index, (chapter_title, chapter_text))| ChapterSegmentEntry {
                    index: index + 1,
                    title: chapter_title.clone(),
                    char_count: chapter_text.chars().count(),
                    chunk_count: chapter_chunk_ranges.get(index).map(|range| range.len()).unwrap_or(0),
                    text: chapter_text.clone(),
                })
                .collect(),
        },
    )?;
    resume_state.segment_json_path = segment_json_path.to_string_lossy().to_string();
    update_resume_state_progress(
        &mut resume_state,
        "segment_json",
        "已输出分段 JSON",
        completed_steps,
        total_steps,
        TTS_BOOK_STATUS_RUNNING,
    );
    save_resume_state(&resume_state_path, &resume_state)?;
    completed_steps += 1;
    emit_tts_book_progress(
        &window,
        make_progress(
            &task_id,
            &file_path,
            &file_name,
            "segment_json",
            completed_steps,
            total_steps,
            "已输出分段 JSON".to_string(),
            false,
            false,
            &output_path,
            &segment_json_path,
        ),
    )?;

    let synth_start = Instant::now();
    let mut zh_chunks = 0usize;
    let mut en_chunks = 0usize;
    let mut chunk_paths = Vec::with_capacity(chunks.len());
    let mut chunk_durations_ms = vec![0u64; chunks.len()];
    let mut last_resume_save_ms = unix_now_ms();
    // 引擎参数整轮解析一次即可；放进循环会每段重复做路径与资源探测。
    let tts_synth_config = crate::guaxiservices::speech::resolve_tts_engine_config(
        &tts_model,
        &tts_tokens,
        &tts_mode_hint,
        &execution_provider,
        None,
    )?;
    let tts_speaker = speaker_id.trim().parse::<i32>().unwrap_or(0);
    let tts_speed = speed.unwrap_or(1.0);

    // 界面上显式选了 GPU、探测却不通过时会被静默降级到 CPU（见 speech::config::cuda_available）。
    // 不说明的话，任务卡上只会看到"正在合成"，用户会以为 GPU 生效了。
    // "auto" 不提示——那是"跟着硬件走"的正常结果，不算降级。
    let provider_fallback_note = (execution_provider == "cuda" && tts_synth_config.provider != "cuda")
        .then(|| "请求的 CUDA 不可用，本次已改用 CPU 合成".to_string());
    if let Some(note) = provider_fallback_note.as_deref() {
        emit_tts_book_progress(
            &window,
            make_progress(
                &task_id,
                &file_path,
                &file_name,
                "synth_chunk",
                completed_steps,
                total_steps,
                note.to_string(),
                false,
                false,
                &output_path,
                &segment_json_path,
            ),
        )?;
    }

    // 引擎在进程内常驻，单实例内部已用 num_threads 吃满多核；再叠多路并发只会
    // 在同一把锁上排队、白占内存，所以这里串行。批结构保留——它同时承担
    // 「按顺序落盘、上报进度、更新续转状态」的职责。
    let batch_size = 1usize;
    let mut batch_start = 0usize;
    while batch_start < chunks.len() {
        let batch_end = (batch_start + batch_size).min(chunks.len());
        let batch = &chunks[batch_start..batch_end];
        check_task_control(
            &task_id,
            &mut resume_state,
            &resume_state_path,
            completed_steps,
            total_steps,
        )?;

        // 已在缓存里的分段不必再合成，只把缺的挑出来并发跑。
        let missing = batch
            .iter()
            .enumerate()
            .filter(|(_, chunk)| !chunk_dir.join(format!("{:05}.wav", chunk.index)).exists())
            .map(|(offset, _)| offset)
            .collect::<Vec<_>>();

        let synthesized = futures_util::future::join_all(missing.iter().map(|offset| {
            let chunk = &batch[*offset];
            let config = &tts_synth_config;
            async move { synthesize_chunk_audio(chunk, config, tts_speaker, tts_speed).await }
        }))
        .await;

        let mut results: Vec<Option<Result<Vec<u8>, String>>> =
            (0..batch.len()).map(|_| None).collect();
        for (offset, result) in missing.iter().zip(synthesized) {
            results[*offset] = Some(result);
        }

        for (offset, chunk) in batch.iter().enumerate() {
            match chunk.lang {
                ChunkLang::Zh => zh_chunks += 1,
                ChunkLang::En => en_chunks += 1,
            }
            let chunk_path = chunk_dir.join(format!("{:05}.wav", chunk.index));
            let pending = results[offset].take();
            let resumed = pending.is_none();
            // 新合成的分段内存里已经有字节了，不必再读一次盘；只有复用旧分段才需要读。
            let chunk_bytes = match pending {
                Some(result) => {
                    let bytes = result?;
                    fs::write(&chunk_path, &bytes)
                        .map_err(|e| format!("写入分段音频失败: {}", e))?;
                    bytes
                }
                None => {
                    resumed_any = true;
                    fs::read(&chunk_path).map_err(|e| format!("读取分段音频失败: {}", e))?
                }
            };
            // `completed_chunks` 只被用来取长度（见 saved_task_from_state），顺序无意义，
            // 因此不必每段都排一次序。
            if !resume_state.completed_chunks.contains(&chunk.index) {
                resume_state.completed_chunks.push(chunk.index);
            }
            if let Some(duration) = chunk_durations_ms.get_mut(chunk.index.saturating_sub(1)) {
                *duration = wav_duration_ms(&chunk_bytes)?;
            }
            chunk_paths.push(chunk_path);
            completed_steps += 1;
            let progress_message = if resumed {
                format!("复用已完成分段 {}/{}", chunk.index, chunks.len())
            } else {
                format!("正在合成分段 {}/{}", chunk.index, chunks.len())
            };
            update_resume_state_progress(
                &mut resume_state,
                "synth_chunk",
                progress_message.as_str(),
                completed_steps,
                total_steps,
                TTS_BOOK_STATUS_RUNNING,
            );
            save_resume_state_throttled(&mut last_resume_save_ms, &resume_state_path, &resume_state)?;
            emit_tts_book_progress(
                &window,
                make_progress(
                    &task_id,
                    &file_path,
                    &file_name,
                    "synth_chunk",
                    completed_steps,
                    total_steps,
                    progress_message,
                    resumed,
                    false,
                    &output_path,
                    &segment_json_path,
                ),
            )?;
        }

        batch_start = batch_end;
    }
    // 分段循环里是节流落盘的，这里强制写一次，确保进入合并阶段前状态是完整的。
    save_resume_state(&resume_state_path, &resume_state)?;
    info!(
        total_chunks = chunks.len(),
        zh_chunks,
        en_chunks,
        cost_ms = synth_start.elapsed().as_millis() as u64,
        "tts_book_convert full audio synthesized"
    );

    write_segment_manifest(
        &segment_json_path,
        &SegmentManifest {
            title: extracted.title.clone(),
            source_file: file_path.to_string_lossy().to_string(),
            total_chars: cleaned.chars().count(),
            total_chunks: chunks.len(),
            toc: extracted.toc.clone(),
            chunks: build_segment_entries(&chunks, &chunk_durations_ms),
            chapters: chapters
                .iter()
                .enumerate()
                .map(|(index, (chapter_title, chapter_text))| ChapterSegmentEntry {
                    index: index + 1,
                    title: chapter_title.clone(),
                    char_count: chapter_text.chars().count(),
                    chunk_count: chapter_chunk_ranges.get(index).map(|range| range.len()).unwrap_or(0),
                    text: chapter_text.clone(),
                })
                .collect(),
        },
    )?;

    merge_wav_files_to_path(&chunk_paths, &chunks, &output_path)?;
    resume_state.output_path = output_path.to_string_lossy().to_string();
    update_resume_state_progress(
        &mut resume_state,
        "merge_audio",
        "已合并完整音频",
        completed_steps + 1,
        total_steps,
        TTS_BOOK_STATUS_RUNNING,
    );
    save_resume_state(&resume_state_path, &resume_state)?;
    completed_steps += 1;
    emit_tts_book_progress(
        &window,
        make_progress(
            &task_id,
            &file_path,
            &file_name,
            "merge_audio",
            completed_steps,
            total_steps,
            "已合并完整音频".to_string(),
            resumed_any,
            false,
            &output_path,
            &segment_json_path,
        ),
    )?;

    let mut chapter_output_paths = Vec::new();
    if request.split_by_toc {
        let chapter_dir = task_output_dir.join(format!("{}_chapters", stem));
        fs::create_dir_all(&chapter_dir).map_err(|e| format!("创建章节目录失败: {}", e))?;

        for (index, (chapter_title, chapter_text)) in chapters.iter().enumerate() {
            check_task_control(
                &task_id,
                &mut resume_state,
                &resume_state_path,
                completed_steps,
                total_steps,
            )?;
            let chapter_start = Instant::now();
            let chapter_stem = sanitize_file_stem(&truncate_title(chapter_title, 36));
            let chapter_name = format!("{:03}_{}.wav", index + 1, chapter_stem);
            let chapter_path = chapter_dir.join(chapter_name);
            let range = chapter_chunk_ranges[index].clone();
            if range.is_empty() {
                completed_steps += 1;
                continue;
            }
            let resumed = chapter_path.exists();
            if !resumed {
                info!(
                    chapter_index = index + 1,
                    chapter_title = chapter_title.as_str(),
                    chapter_chars = chapter_text.chars().count(),
                    chunk_count = range.len(),
                    "tts_book_convert chapter merge start"
                );
                // 直接取该章对应的分段音频拼装，不再重新合成一遍。
                merge_wav_files_to_path(&chunk_paths[range.clone()], &chunks[range], &chapter_path)?;
                info!(
                    chapter_index = index + 1,
                    chapter_path = chapter_path.to_string_lossy().as_ref(),
                    cost_ms = chapter_start.elapsed().as_millis() as u64,
                    "tts_book_convert chapter merge done"
                );
            } else {
                resumed_any = true;
            }
            let chapter_path_str = chapter_path.to_string_lossy().to_string();
            chapter_output_paths.push(chapter_path_str.clone());
            if !resume_state.completed_chapters.contains(&(index + 1)) {
                resume_state.completed_chapters.push(index + 1);
            }
            if !resume_state.chapter_output_paths.contains(&chapter_path_str) {
                resume_state.chapter_output_paths.push(chapter_path_str);
            }
            completed_steps += 1;
            let progress_message = if resumed {
                format!("复用已完成章节 {}/{}", index + 1, chapters.len())
            } else {
                format!("正在生成章节 {}/{}", index + 1, chapters.len())
            };
            update_resume_state_progress(
                &mut resume_state,
                "chapter_audio",
                progress_message.as_str(),
                completed_steps,
                total_steps,
                TTS_BOOK_STATUS_RUNNING,
            );
            save_resume_state_throttled(&mut last_resume_save_ms, &resume_state_path, &resume_state)?;
            emit_tts_book_progress(
                &window,
                make_progress(
                    &task_id,
                    &file_path,
                    &file_name,
                    "chapter_audio",
                    completed_steps,
                    total_steps,
                    if resumed {
                        format!("复用已完成章节 {}/{}", index + 1, chapters.len())
                    } else {
                        format!("正在生成章节 {}/{}", index + 1, chapters.len())
                    },
                    resumed,
                    false,
                    &output_path,
                    &segment_json_path,
                ),
            )?;
        }
    }

    info!(
        file_path = file_path.to_string_lossy().as_ref(),
        output_path = output_path.to_string_lossy().as_ref(),
        chapter_count = chapter_output_paths.len(),
        total_cost_ms = total_start.elapsed().as_millis() as u64,
        resumed_any,
        "tts_book_convert finished"
    );

    // 降级说明挂在收尾消息上：分段进度是滚动覆盖的，只有这条会一直留在任务卡上。
    let completed_message = {
        let base = if resumed_any {
            "转换完成，已从中断位置继续"
        } else {
            "转换完成"
        };
        match &provider_fallback_note {
            Some(note) => format!("{}（{}）", base, note),
            None => base.to_string(),
        }
    };

    emit_tts_book_progress(
        &window,
        make_progress(
            &task_id,
            &file_path,
            &file_name,
            "completed",
            total_steps,
            total_steps,
            completed_message.clone(),
            resumed_any,
            false,
            &output_path,
            &segment_json_path,
        ),
    )?;
    resume_state.status = TTS_BOOK_STATUS_SUCCESS.to_string();
    resume_state.last_stage = "completed".to_string();
    resume_state.last_message = completed_message;
    resume_state.progress = 100;
    resume_state.chapter_output_paths = chapter_output_paths.clone();
    save_resume_state(&resume_state_path, &resume_state)?;
    unregister_resume_state(&resume_state_path)?;
    clear_task_control(&task_id);

    Ok(TtsBookConvertResponse {
        task_id,
        output_path: output_path.to_string_lossy().to_string(),
        segment_json_path: segment_json_path.to_string_lossy().to_string(),
        title: extracted.title,
        total_chars: cleaned.chars().count(),
        total_chunks: chunks.len(),
        zh_chunks,
        en_chunks,
        chapter_count: chapter_output_paths.len(),
        chapter_output_paths,
        toc: extracted.toc,
    })
}

#[tauri::command]
pub fn tts_book_pause_task(task_id: String) -> Result<(), String> {
    set_task_control(&task_id, true, false)
}

#[tauri::command]
pub fn tts_book_stop_task(task_id: String) -> Result<(), String> {
    set_task_control(&task_id, false, true)
}

#[tauri::command]
pub fn tts_book_pause_all_tasks(task_ids: Vec<String>) -> Result<(), String> {
    for task_id in task_ids {
        set_task_control(&task_id, true, false)?;
    }
    Ok(())
}

#[tauri::command]
pub fn tts_book_stop_all_tasks(task_ids: Vec<String>) -> Result<(), String> {
    for task_id in task_ids {
        set_task_control(&task_id, false, true)?;
    }
    Ok(())
}

#[tauri::command]
pub fn tts_book_list_saved_tasks() -> Result<Vec<TtsBookSavedTask>, String> {
    let mut out = Vec::new();
    let index = load_resume_index()?;
    let mut dirty = false;
    let mut valid_paths = Vec::new();

    for raw_path in index.tasks {
        let path = PathBuf::from(raw_path);
        if !path.exists() {
            dirty = true;
            continue;
        }
        let mut state = match load_resume_state(&path) {
            Ok(state) => state,
            Err(_) => {
                dirty = true;
                continue;
            }
        };
        if matches!(state.status.as_str(), TTS_BOOK_STATUS_SUCCESS | TTS_BOOK_STATUS_STOPPED) {
            dirty = true;
            continue;
        }
        if state.status == TTS_BOOK_STATUS_RUNNING {
            state.status = TTS_BOOK_STATUS_PAUSED.to_string();
            if state.last_message.trim().is_empty() || state.last_message.contains("正在") {
                state.last_message = "检测到上次退出时任务未完成，可继续转换".to_string();
            }
            save_resume_state(&path, &state)?;
        }
        valid_paths.push(path.to_string_lossy().to_string());
        out.push(saved_task_from_state(&state));
    }

    if dirty {
        save_resume_index(&TtsBookResumeIndex { tasks: valid_paths })?;
    }

    Ok(out)
}

fn infer_tts_mode_hint(model: &str) -> String {
    let model = model.trim().to_lowercase();
    if model.contains("kokoro") {
        "kokoro".to_string()
    } else if model.contains("vits") {
        "vits".to_string()
    } else {
        String::new()
    }
}

fn saved_task_from_state(state: &TtsBookResumeState) -> TtsBookSavedTask {
    TtsBookSavedTask {
        task_id: state.task_id.clone(),
        file_path: state.source_file.clone(),
        file_name: state.file_name.clone(),
        title: state.title.clone(),
        output_dir: state.output_dir.clone(),
        output_name: state.output_name.clone(),
        tts_model: state.tts_model.clone(),
        tts_tokens: state.tts_tokens.clone(),
        tts_mode_hint: state.tts_mode_hint.clone(),
        speaker_id: state.speaker_id.clone(),
        speed: state.speed,
        split_by_toc: state.split_by_toc,
        max_chunk_chars: state.max_chunk_chars,
        execution_provider: normalize_execution_provider(&state.execution_provider),
        status: if state.status.trim().is_empty() {
            TTS_BOOK_STATUS_PAUSED.to_string()
        } else {
            state.status.clone()
        },
        message: state.last_message.clone(),
        progress: state.progress,
        output_path: state.output_path.clone(),
        segment_json_path: state.segment_json_path.clone(),
        total_chunks: state.total_chunks,
        completed_chunks: state.completed_chunks.len(),
        chapter_count: state.chapter_output_paths.len().max(state.completed_chapters.len()),
        completed_chapters: state.completed_chapters.len(),
        chapter_output_paths: state.chapter_output_paths.clone(),
    }
}

/// "auto" 原样传下去，由 speech::config 按 CUDA 可用性决定；"cpu" 是用户显式选择，必须尊重。
fn normalize_execution_provider(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "cuda" | "gpu" => "cuda".to_string(),
        "coreml" => "coreml".to_string(),
        "cpu" => "cpu".to_string(),
        _ => DEFAULT_EXECUTION_PROVIDER.to_string(),
    }
}

fn resume_index_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取程序目录失败: {}", e))?;
    let base = exe
        .parent()
        .ok_or_else(|| "无法获取程序目录".to_string())?
        .join("data")
        .join("tts_book");
    fs::create_dir_all(&base).map_err(|e| format!("创建 TTSBook 状态目录失败: {}", e))?;
    Ok(base.join(TTS_BOOK_INDEX_FILE))
}

fn load_resume_index() -> Result<TtsBookResumeIndex, String> {
    let path = resume_index_path()?;
    if !path.exists() {
        return Ok(TtsBookResumeIndex::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取续转索引失败: {}", e))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析续转索引失败: {}", e))
}

fn save_resume_index(index: &TtsBookResumeIndex) -> Result<(), String> {
    let path = resume_index_path()?;
    let raw = serde_json::to_string_pretty(index).map_err(|e| format!("序列化续转索引失败: {}", e))?;
    fs::write(path, raw).map_err(|e| format!("写入续转索引失败: {}", e))
}

fn register_resume_state(path: &Path) -> Result<(), String> {
    let mut index = load_resume_index()?;
    let path_str = path.to_string_lossy().to_string();
    if !index.tasks.contains(&path_str) {
        index.tasks.push(path_str);
        index.tasks.sort();
        index.tasks.dedup();
        save_resume_index(&index)?;
    }
    Ok(())
}

fn unregister_resume_state(path: &Path) -> Result<(), String> {
    let mut index = load_resume_index()?;
    let path_str = path.to_string_lossy().to_string();
    index.tasks.retain(|item| item != &path_str);
    save_resume_index(&index)
}

fn set_task_control(task_id: &str, pause: bool, stop: bool) -> Result<(), String> {
    let normalized = task_id.trim();
    if normalized.is_empty() {
        return Err("task_id 不能为空".to_string());
    }
    let mut guard = TTS_BOOK_TASK_CONTROLS
        .lock()
        .map_err(|_| "任务控制锁已损坏".to_string())?;
    let entry = guard.entry(normalized.to_string()).or_default();
    if stop {
        entry.stop_requested = true;
        entry.pause_requested = false;
    } else if pause {
        entry.pause_requested = true;
    }
    Ok(())
}

fn reset_task_control(task_id: &str) {
    if let Ok(mut guard) = TTS_BOOK_TASK_CONTROLS.lock() {
        guard.insert(task_id.to_string(), TtsBookTaskControl::default());
    }
}

fn clear_task_control(task_id: &str) {
    if let Ok(mut guard) = TTS_BOOK_TASK_CONTROLS.lock() {
        guard.remove(task_id);
    }
}

fn check_task_control(
    task_id: &str,
    state: &mut TtsBookResumeState,
    state_path: &Path,
    current: usize,
    total: usize,
) -> Result<(), String> {
    let control = TTS_BOOK_TASK_CONTROLS
        .lock()
        .map_err(|_| "任务控制锁已损坏".to_string())?
        .get(task_id)
        .cloned()
        .unwrap_or_default();
    if control.stop_requested {
        update_resume_state_progress(
            state,
            "stopped",
            "任务已终止",
            current,
            total,
            TTS_BOOK_STATUS_STOPPED,
        );
        save_resume_state(state_path, state)?;
        let _ = unregister_resume_state(state_path);
        clear_task_control(task_id);
        return Err(TTS_BOOK_STOP_SIGNAL.to_string());
    }
    if control.pause_requested {
        update_resume_state_progress(
            state,
            "paused",
            "任务已暂停，可稍后继续",
            current,
            total,
            TTS_BOOK_STATUS_PAUSED,
        );
        save_resume_state(state_path, state)?;
        clear_task_control(task_id);
        return Err(TTS_BOOK_PAUSE_SIGNAL.to_string());
    }
    Ok(())
}

fn update_resume_state_progress(
    state: &mut TtsBookResumeState,
    stage: &str,
    message: &str,
    current: usize,
    total: usize,
    status: &str,
) {
    let safe_total = total.max(1);
    let safe_current = current.min(safe_total);
    state.last_stage = stage.to_string();
    state.last_message = message.to_string();
    state.status = status.to_string();
    state.progress = ((safe_current as f64 / safe_total as f64) * 100.0).round() as u8;
}

async fn synthesize_chunk_audio(
    chunk: &TextChunk,
    config: &crate::guaxiservices::speech::TtsEngineConfig,
    speaker: i32,
    speed: f32,
) -> Result<Vec<u8>, String> {
    let chunk_start = Instant::now();
    let prepared_text = prepare_chunk_text_for_tts(&chunk.text, chunk.lang);

    info!(
        index = chunk.index,
        lang = match chunk.lang { ChunkLang::Zh => "zh", ChunkLang::En => "en" },
        chars = prepared_text.chars().count(),
        "tts_book_convert: synth one chunk"
    );

    // 直接调进程内合成，音频以字节返回——省掉旧路径的临时文件与 base64 编解码往返。
    let bytes = crate::guaxiservices::speech::synthesize_wav(
        config.clone(),
        prepared_text,
        speaker,
        speed,
    )
    .await?;

    info!(
        index = chunk.index,
        audio_bytes = bytes.len(),
        cost_ms = chunk_start.elapsed().as_millis() as u64,
        "tts_book_convert: synth one chunk done"
    );
    Ok(bytes)
}

fn emit_tts_book_progress(window: &Window, payload: TtsBookProgress) -> Result<(), String> {
    window
        .emit("tts_book_progress", payload)
        .map_err(|e| e.to_string())
}

fn make_progress(
    task_id: &str,
    file_path: &Path,
    file_name: &str,
    stage: &str,
    current: usize,
    total: usize,
    message: String,
    resumed: bool,
    failed: bool,
    output_path: &Path,
    segment_json_path: &Path,
) -> TtsBookProgress {
    let safe_total = total.max(1);
    let safe_current = current.min(safe_total);
    let progress = ((safe_current as f64 / safe_total as f64) * 100.0).round() as u8;
    TtsBookProgress {
        task_id: task_id.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        file_name: file_name.to_string(),
        stage: stage.to_string(),
        current: safe_current,
        total: safe_total,
        progress,
        message,
        resumed,
        failed,
        output_path: output_path.to_string_lossy().to_string(),
        segment_json_path: segment_json_path.to_string_lossy().to_string(),
    }
}

fn load_resume_state(path: &Path) -> Result<TtsBookResumeState, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("读取续转状态失败: {}", e))?;
    serde_json::from_str(&raw).map_err(|e| format!("解析续转状态失败: {}", e))
}

/// 续转状态是内部检查点、不给人看，用紧凑 JSON 即可（pretty 会显著放大写入量）。
fn save_resume_state(path: &Path, state: &TtsBookResumeState) -> Result<(), String> {
    let raw = serde_json::to_string(state).map_err(|e| format!("序列化续转状态失败: {}", e))?;
    fs::write(path, raw).map_err(|e| format!("写入续转状态失败: {}", e))
}

/// 每段都落盘会让总写入量随段数平方增长（`completed_chunks` 越来越长），
/// 所以按时间节流。安全性由分段 WAV 文件本身兜底——续转时是用 `chunk_path.exists()`
/// 判断复用，而不是 `completed_chunks`；暂停/终止等关键点由 `check_task_control` 强制落盘。
const RESUME_SAVE_INTERVAL_MS: u128 = 5000;

fn save_resume_state_throttled(
    last_saved_ms: &mut u128,
    path: &Path,
    state: &TtsBookResumeState,
) -> Result<(), String> {
    let now = unix_now_ms();
    if now.saturating_sub(*last_saved_ms) < RESUME_SAVE_INTERVAL_MS {
        return Ok(());
    }
    save_resume_state(path, state)?;
    *last_saved_ms = now;
    Ok(())
}

fn write_segment_manifest(path: &Path, manifest: &SegmentManifest) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(manifest).map_err(|e| format!("序列化分段 JSON 失败: {}", e))?;
    fs::write(path, raw).map_err(|e| format!("写入分段 JSON 失败: {}", e))
}

fn build_segment_entries(chunks: &[TextChunk], durations_ms: &[u64]) -> Vec<SegmentEntry> {
    let mut cursor_ms = 0u64;
    let mut entries = Vec::with_capacity(chunks.len());

    for (idx, chunk) in chunks.iter().enumerate() {
        let pause_after_ms = derive_pause_after_ms(&chunk.text);
        let audio_duration_ms = durations_ms.get(idx).copied().unwrap_or(0);
        let trailing_pause_ms = if idx + 1 < chunks.len() {
            pause_after_ms as u64
        } else {
            0
        };
        let start_ms = cursor_ms;
        let end_ms = start_ms
            .saturating_add(audio_duration_ms)
            .saturating_add(trailing_pause_ms);

        entries.push(SegmentEntry {
            index: chunk.index,
            lang: match chunk.lang {
                ChunkLang::Zh => "zh",
                ChunkLang::En => "en",
            },
            kind: detect_segment_kind(&chunk.text),
            speaker_hint: detect_speaker_hint(&chunk.text),
            pause_after_ms,
            audio_duration_ms,
            start_ms,
            end_ms,
            char_count: chunk.text.chars().count(),
            text: chunk.text.clone(),
        });

        cursor_ms = end_ms;
    }

    entries
}

fn file_signature(path: &Path) -> Result<(u64, u64), String> {
    let metadata = fs::metadata(path).map_err(|e| format!("读取文件元数据失败: {}", e))?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Ok((metadata.len(), modified))
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn prepare_chunk_text_for_tts(text: &str, lang: ChunkLang) -> String {
    let text = match lang {
        ChunkLang::Zh => normalize_zh_tts_text(text),
        ChunkLang::En => normalize_inline_spaces(text),
    };
    text.replace('\n', " ").trim().to_string()
}

/// 每个分段都要调 `normalize_zh_tts_text`，正则不能在里面现编。
static ZH_FRACTION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+)\s*/\s*(\d+)").expect("valid regex"));

fn normalize_zh_tts_text(text: &str) -> String {
    ZH_FRACTION_RE
        .replace_all(text, |caps: &regex::Captures| format!("{}分之{}", &caps[2], &caps[1]))
        .to_string()
}

fn detect_segment_kind(text: &str) -> &'static str {
    let trimmed = text.trim();
    if trimmed.contains('“') || trimmed.contains('”') || trimmed.contains('"') {
        "dialogue"
    } else {
        "narration"
    }
}

fn detect_speaker_hint(text: &str) -> &'static str {
    if text.contains("她说")
        || text.contains("她问")
        || text.contains("她回答")
        || text.contains("姑娘")
        || text.contains("女士")
    {
        "female"
    } else if text.contains("他说")
        || text.contains("他问")
        || text.contains("他回答")
        || text.contains("男人")
        || text.contains("先生")
    {
        "male"
    } else {
        "narration"
    }
}

pub(crate) fn derive_pause_after_ms(text: &str) -> u32 {
    let trimmed = text.trim_end();
    if trimmed.ends_with("……") || trimmed.ends_with('…') {
        520
    } else if trimmed.ends_with('。') || trimmed.ends_with('！') || trimmed.ends_with('？') {
        380
    } else if trimmed.ends_with('；') || trimmed.ends_with('：') {
        280
    } else if trimmed.ends_with('，') || trimmed.ends_with('、') || trimmed.ends_with(',') {
        150
    } else {
        110
    }
}

fn split_text_into_chapters(text: &str, toc: &[String]) -> Vec<(String, String)> {
    let lines = text.lines().map(|l| l.trim()).collect::<Vec<_>>();
    if lines.is_empty() {
        return vec![];
    }

    let mut marks: Vec<(usize, String)> = Vec::new();

    if !toc.is_empty() {
        let mut cursor = 0usize;
        for heading in toc {
            let h = heading.trim();
            if h.is_empty() {
                continue;
            }
            let mut found: Option<usize> = None;
            for idx in cursor..lines.len() {
                if heading_matches_line(h, lines[idx]) {
                    found = Some(idx);
                    break;
                }
            }
            if let Some(idx) = found {
                marks.push((idx, h.to_string()));
                cursor = idx.saturating_add(1);
            }
        }
    }

    if marks.len() < 2 {
        marks = find_heading_marks(&lines);
    }

    if marks.is_empty() {
        return vec![("全文".to_string(), text.to_string())];
    }

    let mut out: Vec<(String, String)> = Vec::new();
    if let Some((first_idx, _)) = marks.first() {
        if *first_idx > 0 {
            let preface = lines[..*first_idx].join("\n");
            if preface.chars().count() >= 60 {
                out.push(("序章".to_string(), preface));
            }
        }
    }

    for i in 0..marks.len() {
        let (start, title) = &marks[i];
        let end = if i + 1 < marks.len() {
            marks[i + 1].0
        } else {
            lines.len()
        };
        if *start >= end || end > lines.len() {
            continue;
        }
        let section = lines[*start..end].join("\n");
        if section.chars().count() >= 40 {
            out.push((title.clone(), section));
        }
    }

    if out.is_empty() {
        vec![("全文".to_string(), text.to_string())]
    } else {
        out
    }
}

fn find_heading_marks(lines: &[&str]) -> Vec<(usize, String)> {
    let mut marks = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.chars().count() > 100 {
            continue;
        }
        if looks_like_heading(t) {
            marks.push((idx, t.to_string()));
        }
        if marks.len() >= 200 {
            break;
        }
    }
    marks
}

fn heading_matches_line(heading: &str, line: &str) -> bool {
    let h = heading.trim();
    let l = line.trim();
    if h.is_empty() || l.is_empty() {
        return false;
    }
    h == l || l.contains(h) || h.contains(l)
}

fn truncate_title(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in s.chars().take(max_chars) {
        out.push(ch);
    }
    if out.trim().is_empty() {
        "chapter".to_string()
    } else {
        out
    }
}

fn sanitize_file_stem(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "audiobook".to_string()
    } else {
        trimmed.to_string()
    }
}

async fn extract_document(path: &Path) -> Result<ExtractedDoc, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();

    match ext.as_str() {
        "txt" => {
            let text = read_text_with_fallback(path)?;
            let toc = extract_outline_lines(&text);
            Ok(ExtractedDoc { title, text, toc })
        }
        "md" | "markdown" => {
            let raw = read_text_with_fallback(path)?;
            let toc = extract_markdown_headings(&raw);
            let text = markdown_to_plain_text(&raw);
            Ok(ExtractedDoc { title, text, toc })
        }
        "pdf" => {
            let text = extract_pdf_text(path).await?;
            let toc = extract_outline_lines(&text);
            Ok(ExtractedDoc { title, text, toc })
        }
        "docx" => {
            let text = extract_docx_text(path)?;
            let toc = extract_outline_lines(&text);
            Ok(ExtractedDoc { title, text, toc })
        }
        "epub" => extract_epub_text(path, title),
        "mobi" | "azw3" => {
            let text = extract_ebook_text(path).await?;
            let toc = extract_outline_lines(&text);
            Ok(ExtractedDoc { title, text, toc })
        }
        "doc" => {
            let text = extract_anydoc_text(path).await?;
            let toc = extract_outline_lines(&text);
            Ok(ExtractedDoc { title, text, toc })
        }
        _ => {
            // 未知后缀不再按白名单直接拒绝，改看内容：能当文本读的就按纯文本处理。
            // 否则「任意纯文本文件」只能靠无限堆后缀来覆盖。
            // 先只读文件头再决定是否读全文：前端放开了「所有文件」，
            // 误选大体积二进制（视频、镜像）时不能把它整个读进内存。
            let head = read_file_head(path, PLAIN_TEXT_SNIFF_BYTES)?;
            if !looks_like_plain_text(&head) {
                let label = if ext.is_empty() {
                    "该文件".to_string()
                } else {
                    format!(".{} 文件", ext)
                };
                return Err(format!(
                    "{}的内容无法识别为文本，请确认它是纯文本或受支持的电子书格式",
                    label
                ));
            }
            let bytes = fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;
            let text = decode_text_bytes(&bytes);
            let toc = extract_outline_lines(&text);
            Ok(ExtractedDoc { title, text, toc })
        }
    }
}

fn read_text_with_fallback(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;
    Ok(decode_text_bytes(&bytes))
}

/// 先按 UTF-8 解码，失败再退到 GBK（大量中文老书是 GBK/GB18030）。
fn decode_text_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => normalize_line_endings(s),
        Err(_) => {
            let (decoded, _, _) = GBK.decode(bytes);
            normalize_line_endings(&decoded)
        }
    }
}

/// 判定「是不是纯文本」时最多读这么多字节，避免把巨型二进制整个读进内存。
const PLAIN_TEXT_SNIFF_BYTES: usize = 8192;

/// 读文件开头至多 `limit` 字节。用于在决定是否读全文之前先嗅探内容类型。
fn read_file_head(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("读取文件失败: {}", e))?;
    let mut head = vec![0u8; limit];
    let mut filled = 0usize;
    while filled < limit {
        match file.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) => return Err(format!("读取文件失败: {}", e)),
        }
    }
    head.truncate(filled);
    Ok(head)
}

/// 判断一段字节能否当纯文本读：含 NUL 直接判为二进制，控制字符占比过高也判为二进制。
/// 中文的 UTF-8 / GBK 多字节都在 0x80 以上，不会被误杀。
/// 调用方负责限制传入长度（见 `PLAIN_TEXT_SNIFF_BYTES`）。
fn looks_like_plain_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    if bytes.contains(&0) {
        return false;
    }
    let control = bytes
        .iter()
        .filter(|b| matches!(**b, 0x01..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f))
        .count();
    control * 100 <= bytes.len() * 5
}

/// 用 `catch_unwind` 把第三方解析库的 panic 限制在本次调用内，绝不向调用方展开。
/// 前提：release 必须保持 `panic = "unwind"`（见 Cargo.toml 的 profile.release 说明）。
fn with_parser_panic_guard<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    let _guard = PARSER_PANIC_HOOK_GUARD
        .lock()
        .map_err(|_| "解析器 panic hook 锁获取失败".to_string())?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(previous_hook);
    result.map_err(|_| "解析器内部错误（已隔离，不影响程序运行）".to_string())
}

/// 老版 Word 二进制（.doc）以及 anydoc 覆盖的其他文档：转 Markdown 后再压成纯文本。
async fn extract_anydoc_text(path: &Path) -> Result<String, String> {
    let path_buf = path.to_path_buf();
    let parsed = tauri::async_runtime::spawn_blocking(move || {
        let markdown = with_parser_panic_guard(|| anydoc::to_markdown(&path_buf))?;
        markdown.map_err(|e| crate::commands::convert::engine::anydoc_error_message(&e))
    })
    .await
    .map_err(|e| format!("文档解析任务失败: {}", e))?;

    let text = markdown_to_plain_text(&parsed?);
    if text.trim().is_empty() {
        return Err("文档解析结果为空".to_string());
    }
    Ok(text)
}

/// MOBI / AZW3：anydoc 不覆盖这两类，走 ebook-rs 取每节的纯文本。
async fn extract_ebook_text(path: &Path) -> Result<String, String> {
    let path_buf = path.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let book = with_parser_panic_guard(|| ebook_rs::Book::from_file(&path_buf))?;
        let book = book.map_err(|e| ebook_error_message(&e))?;
        let blocks = book
            .sections()
            .iter()
            .map(|section| section.plain_text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        if blocks.is_empty() {
            return Err("未能从该电子书中提取到正文".to_string());
        }
        Ok(blocks.join("\n\n"))
    })
    .await
    .map_err(|e| format!("电子书解析任务失败: {}", e))?
}

fn ebook_error_message(e: &ebook_rs::EbookError) -> String {
    use ebook_rs::EbookError as E;
    match e {
        E::DrmProtected(_) => "该电子书受 DRM 保护，无法解析，请先去 DRM 后再转换".to_string(),
        E::InvalidFormat(detail) => format!(
            "无法解析该电子书格式：{}。AZW3 若是 KF8 专有或 Huff/CDIC 压缩，可能不被支持",
            detail
        ),
        E::CorruptedData(detail) => format!("该电子书数据损坏：{}", detail),
        E::NotFound(detail) => format!("该电子书缺少必需内容：{}", detail),
        E::Io(detail) => format!("读取电子书失败：{}", detail),
        E::Xml(detail) => format!("电子书结构解析失败：{}", detail),
        E::Zip(detail) => format!("解压电子书失败：{}", detail),
        _ => format!("电子书解析失败：{}", e),
    }
}

async fn extract_pdf_text(path: &Path) -> Result<String, String> {
    let path_buf = path.to_path_buf();
    let primary = tauri::async_runtime::spawn_blocking(move || {
        let _guard = PARSER_PANIC_HOOK_GUARD
            .lock()
            .map_err(|_| "PDF 解析 panic hook 锁获取失败".to_string())?;
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = catch_unwind(AssertUnwindSafe(|| pdf_extract::extract_text(&path_buf)));
        std::panic::set_hook(previous_hook);
        result
            .map_err(|_| "PDF 解析器发生 panic，可能是该文件中的字体编码不被 pdf-extract 当前版本支持".to_string())?
            .map_err(|e| format!("PDF 解析失败: {}", e))
    })
    .await
    .map_err(|e| format!("PDF 解析任务失败: {}", e))?;
    let raw = match primary {
        Ok(text) => text,
        Err(err) => {
            warn!(
                path = path.to_string_lossy().as_ref(),
                error = err.as_str(),
                "pdf-extract failed, fallback to lopdf"
            );
            match extract_pdf_text_with_lopdf(path) {
                Ok(text) => text,
                Err(lopdf_err) => {
                    warn!(
                        path = path.to_string_lossy().as_ref(),
                        error = lopdf_err.as_str(),
                        "lopdf failed, fallback to pdfium"
                    );
                    extract_pdf_text_with_pdfium(path)?
                }
            }
        }
    };
    let text = normalize_pdf_text(&raw);
    if text.trim().is_empty() {
        return Err("PDF 解析结果为空".to_string());
    }
    Ok(text)
}

fn extract_pdf_text_with_lopdf(path: &Path) -> Result<String, String> {
    let document = LoDocument::load(path).map_err(|e| format!("lopdf 加载 PDF 失败: {}", e))?;
    let pages = document.get_pages();
    if pages.is_empty() {
        return Err("lopdf 未发现任何 PDF 页面".to_string());
    }

    let mut page_numbers = pages.keys().copied().collect::<Vec<_>>();
    page_numbers.sort_unstable();
    let mut out = String::new();
    let mut extracted_pages = 0usize;
    let mut page_errors = Vec::new();

    for page_number in page_numbers {
        match document.extract_text(&[page_number]) {
            Ok(text) if !text.trim().is_empty() => {
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
                out.push_str(&text);
                extracted_pages += 1;
            }
            Ok(_) => {
                if let Some(&page_id) = pages.get(&page_number) {
                    let fallback = extract_page_text_from_lopdf_stream(&document, page_id);
                    if !fallback.trim().is_empty() {
                        if !out.is_empty() {
                            out.push_str("\n\n");
                        }
                        out.push_str(&fallback);
                        extracted_pages += 1;
                    }
                }
            }
            Err(err) => {
                if let Some(&page_id) = pages.get(&page_number) {
                    let fallback = extract_page_text_from_lopdf_stream(&document, page_id);
                    if !fallback.trim().is_empty() {
                        if !out.is_empty() {
                            out.push_str("\n\n");
                        }
                        out.push_str(&fallback);
                        extracted_pages += 1;
                        continue;
                    }
                }
                page_errors.push(format!("page {}: {}", page_number, err));
            }
        }
    }

    if !out.trim().is_empty() {
        info!(
            extracted_pages,
            total_pages = pages.len(),
            "lopdf text extraction completed with per-page fallback"
        );
        return Ok(out);
    }

    let detail = if page_errors.is_empty() {
        "无可用页面文本".to_string()
    } else {
        page_errors.join(" | ")
    };
    Err(format!("lopdf 提取文本失败: {}", detail))
}

fn extract_page_text_from_lopdf_stream(document: &LoDocument, page_id: (u32, u16)) -> String {
    let Ok(content) = document.get_page_content(page_id) else {
        return String::new();
    };

    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut hex = String::new();
    let mut in_literal = false;
    let mut in_hex = false;
    let mut escape = false;
    let mut literal_depth = 0usize;

    for ch in String::from_utf8_lossy(&content).chars() {
        if in_literal {
            if escape {
                let mapped = match ch {
                    'n' => '\n',
                    'r' => '\n',
                    't' => '\t',
                    'b' => ' ',
                    'f' => ' ',
                    other => other,
                };
                literal.push(mapped);
                escape = false;
                continue;
            }
            match ch {
                '\\' => {
                    escape = true;
                }
                '(' => {
                    literal_depth += 1;
                    literal.push(ch);
                }
                ')' => {
                    if literal_depth == 0 {
                        if !literal.trim().is_empty() {
                            pieces.push(literal.trim().to_string());
                        }
                        literal.clear();
                        in_literal = false;
                    } else {
                        literal_depth -= 1;
                        literal.push(ch);
                    }
                }
                _ => literal.push(ch),
            }
            continue;
        }

        if in_hex {
            match ch {
                '>' => {
                    let decoded = decode_pdf_hex_text(&hex);
                    if !decoded.trim().is_empty() {
                        pieces.push(decoded);
                    }
                    hex.clear();
                    in_hex = false;
                }
                c if c.is_ascii_hexdigit() => hex.push(c),
                _ => {}
            }
            continue;
        }

        match ch {
            '(' => {
                in_literal = true;
                literal.clear();
                literal_depth = 0;
            }
            '<' => {
                in_hex = true;
                hex.clear();
            }
            _ => {}
        }
    }

    pieces.join("\n")
}

fn decode_pdf_hex_text(hex: &str) -> String {
    let compact = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>();
    if compact.is_empty() {
        return String::new();
    }

    let padded = if compact.len() % 2 == 0 {
        compact
    } else {
        format!("{}0", compact)
    };

    let mut bytes = Vec::with_capacity(padded.len() / 2);
    let mut chars = padded.chars();
    while let (Some(high), Some(low)) = (chars.next(), chars.next()) {
        let pair = [high, low].iter().collect::<String>();
        if let Ok(byte) = u8::from_str_radix(&pair, 16) {
            bytes.push(byte);
        }
    }

    if bytes.starts_with(&[0xFE, 0xFF]) {
        let utf16 = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&utf16);
    }

    String::from_utf8_lossy(&bytes).to_string()
}

fn extract_pdf_text_with_pdfium(path: &Path) -> Result<String, String> {
    // pdfium 全进程唯一、非线程安全：不再自行 bind，统一投递到 pdf_reader 的工作线程。
    // 本函数在 spawn_blocking 上下文调用，用阻塞版把任务交给 pdfium 工作线程。
    let path_str = path.to_string_lossy().to_string();
    crate::commands::pdf_reader::run_pdf_blocking(move |state| {
        state.with_doc(&path_str, |document| {
            let mut out = String::new();
            let total_pages = document.pages().len();
            for index in 0..total_pages {
                let page = document
                    .pages()
                    .get(index)
                    .map_err(|_| format!("PDFium 获取第 {} 页失败", index + 1))?;
                let page_text = match page.text() {
                    Ok(text_page) => text_page.all(),
                    Err(_) => String::new(),
                };
                if !page_text.trim().is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(&page_text);
                }
            }

            if out.trim().is_empty() {
                return Err("PDFium 解析结果为空".to_string());
            }
            Ok(out)
        })
    })
}

fn normalize_pdf_text(raw: &str) -> String {
    let normalized = normalize_line_endings(raw);
    let mut freq: HashMap<String, usize> = HashMap::new();
    let lines = normalized.lines().map(|line| line.trim()).collect::<Vec<_>>();
    for line in &lines {
        if is_probable_running_header(line) {
            *freq.entry((*line).to_string()).or_insert(0) += 1;
        }
    }

    let mut cleaned_lines = Vec::new();
    for line in lines {
        if line.is_empty() {
            cleaned_lines.push(String::new());
            continue;
        }
        if is_probable_page_noise(line) {
            continue;
        }
        if freq.get(line).copied().unwrap_or(0) >= 3 && !looks_like_heading(line) {
            continue;
        }
        cleaned_lines.push(line.to_string());
    }

    join_wrapped_lines(&cleaned_lines)
}

fn is_probable_page_noise(line: &str) -> bool {
    let digits_only = line.chars().all(|c| c.is_ascii_digit());
    if digits_only && line.len() <= 4 {
        return true;
    }
    let compact = line.to_ascii_lowercase().replace(' ', "");
    compact.starts_with("page") && compact[4..].chars().all(|c| c.is_ascii_digit())
}
fn extract_docx_text(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("打开 docx 失败: {}", e))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("读取 docx 压缩包失败: {}", e))?;

    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|e| format!("读取 word/document.xml 失败: {}", e))?
        .read_to_string(&mut xml)
        .map_err(|e| format!("读取 document.xml 内容失败: {}", e))?;

    Ok(docx_xml_to_text(&xml))
}

fn docx_xml_to_text(xml: &str) -> String {
    let mut s = xml.to_string();
    s = s.replace("</w:p>", "\n");
    s = s.replace("</w:tr>", "\n");
    s = s.replace("</w:tc>", " ");
    s = s.replace("<w:br/>", "\n");
    s = s.replace("<w:tab/>", " ");

    let tag_re = Regex::new(r"<[^>]+>").expect("valid regex");
    s = tag_re.replace_all(&s, "").to_string();

    join_wrapped_lines(
        &normalize_line_endings(&decode_xml_entities(&s))
            .lines()
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>(),
    )
}

fn extract_epub_text(path: &Path, title: String) -> Result<ExtractedDoc, String> {
    let file = fs::File::open(path).map_err(|e| format!("打开 epub 失败: {}", e))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("读取 epub 压缩包失败: {}", e))?;

    let (opf_path, opf_dir) = resolve_epub_opf_path(&mut zip)?;
    let mut opf_content = String::new();
    zip.by_name(&opf_path)
        .map_err(|e| format!("读取 OPF 失败: {}", e))?
        .read_to_string(&mut opf_content)
        .map_err(|e| format!("读取 OPF 内容失败: {}", e))?;

    let toc = extract_epub_toc(&mut zip, &opf_content, &opf_dir);
    let spine_files = resolve_epub_spine_files(&opf_content, &opf_dir);

    let mut text_blocks: Vec<String> = Vec::new();
    for rel in &spine_files {
        if let Ok(mut f) = zip.by_name(rel) {
            let mut html = String::new();
            if f.read_to_string(&mut html).is_ok() {
                let block = html_to_text(&html);
                if !block.trim().is_empty() {
                    text_blocks.push(block);
                }
            }
        }
    }

    if text_blocks.is_empty() {
        for i in 0..zip.len() {
            let Ok(mut f) = zip.by_index(i) else {
                continue;
            };
            let name = f.name().to_lowercase();
            if !(name.ends_with(".xhtml") || name.ends_with(".html") || name.ends_with(".htm")) {
                continue;
            }
            let mut html = String::new();
            if f.read_to_string(&mut html).is_ok() {
                let block = html_to_text(&html);
                if !block.trim().is_empty() {
                    text_blocks.push(block);
                }
            }
        }
    }

    let text = text_blocks.join("\n\n");
    let toc = if toc.is_empty() {
        extract_outline_lines(&text)
    } else {
        toc
    };

    Ok(ExtractedDoc { title, text, toc })
}

fn resolve_epub_opf_path(zip: &mut ZipArchive<fs::File>) -> Result<(String, String), String> {
    let mut container_xml = String::new();
    zip.by_name("META-INF/container.xml")
        .map_err(|e| format!("读取 container.xml 失败: {}", e))?
        .read_to_string(&mut container_xml)
        .map_err(|e| format!("读取 container.xml 内容失败: {}", e))?;

    let re = Regex::new(r#"full-path\s*=\s*"([^"]+)""#).expect("valid regex");
    let opf_path = re
        .captures(&container_xml)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| "未找到 OPF 路径".to_string())?;

    let opf_dir = opf_path
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();

    Ok((opf_path, opf_dir))
}

// OPF 是 XML，属性顺序由书写者决定。旧实现用「id 必须在 href 之前」的单条正则去匹配
// `<item>`，遇到 href 在前、或中间夹着 media-type 的书整条 manifest 就匹配为空，
// spine 随之落空，最后退化成按 zip 条目顺序扫 HTML——阅读顺序就乱了。
// 这里改成先切出每个标签、再逐个属性取值，与顺序无关。
static OPF_ITEM_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<item\s[^>]*>").expect("valid regex"));
static OPF_ITEMREF_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?is)<itemref\s[^>]*>").expect("valid regex"));
static OPF_ATTR_ID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)\bid\s*=\s*["']([^"']*)["']"#).expect("valid regex"));
static OPF_ATTR_IDREF_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bidref\s*=\s*["']([^"']*)["']"#).expect("valid regex"));
static OPF_ATTR_HREF_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bhref\s*=\s*["']([^"']*)["']"#).expect("valid regex"));
static OPF_ATTR_PROPERTIES_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bproperties\s*=\s*["']([^"']*)["']"#).expect("valid regex"));
static OPF_ATTR_MEDIATYPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\bmedia-type\s*=\s*["']([^"']*)["']"#).expect("valid regex"));

fn opf_attr(tag: &str, re: &Regex) -> Option<String> {
    re.captures(tag).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

fn resolve_epub_spine_files(opf_content: &str, opf_dir: &str) -> Vec<String> {
    let mut manifest: HashMap<String, String> = HashMap::new();
    for tag in OPF_ITEM_TAG_RE.find_iter(opf_content).map(|m| m.as_str()) {
        let Some(id) = opf_attr(tag, &OPF_ATTR_ID_RE) else { continue };
        let Some(href) = opf_attr(tag, &OPF_ATTR_HREF_RE) else { continue };
        if !id.is_empty() && !href.is_empty() {
            manifest.insert(id, join_zip_path(opf_dir, &href));
        }
    }

    let mut out = Vec::new();
    for tag in OPF_ITEMREF_TAG_RE.find_iter(opf_content).map(|m| m.as_str()) {
        if let Some(idref) = opf_attr(tag, &OPF_ATTR_IDREF_RE) {
            if let Some(path) = manifest.get(&idref) {
                out.push(path.clone());
            }
        }
    }
    out
}

/// 在 OPF 的 `<item>` 列表里按谓词找一个条目的 href。
fn find_opf_item_href(opf_content: &str, opf_dir: &str, matches: impl Fn(&str) -> bool) -> Option<String> {
    OPF_ITEM_TAG_RE
        .find_iter(opf_content)
        .map(|m| m.as_str())
        .find(|tag| matches(tag))
        .and_then(|tag| opf_attr(tag, &OPF_ATTR_HREF_RE))
        .map(|href| join_zip_path(opf_dir, &href))
}

/// EPUB3 用 nav 文档、EPUB2 用 NCX，两者都取不到才返回空，交给正文标题启发式兜底。
fn extract_epub_toc(zip: &mut ZipArchive<fs::File>, opf_content: &str, opf_dir: &str) -> Vec<String> {
    let nav = extract_epub_toc_from_nav(zip, opf_content, opf_dir);
    if !nav.is_empty() {
        return nav;
    }
    extract_epub_toc_from_ncx(zip, opf_content, opf_dir)
}

fn extract_epub_toc_from_nav(
    zip: &mut ZipArchive<fs::File>,
    opf_content: &str,
    opf_dir: &str,
) -> Vec<String> {
    // 用 token 全等而不是 `[^"]*nav[^"]*`：后者会把 `navigation`、`navy` 也算成 nav。
    let Some(nav_path) = find_opf_item_href(opf_content, opf_dir, |tag| {
        opf_attr(tag, &OPF_ATTR_PROPERTIES_RE)
            .map(|props| props.split_whitespace().any(|token| token.eq_ignore_ascii_case("nav")))
            .unwrap_or(false)
    }) else {
        return vec![];
    };

    let Ok(mut file) = zip.by_name(&nav_path) else {
        return vec![];
    };
    let mut html = String::new();
    if file.read_to_string(&mut html).is_err() {
        return vec![];
    }

    let doc = Html::parse_document(&html);
    let selector = scraper::Selector::parse("nav li a, nav a").expect("valid selector");
    let mut out = Vec::new();
    for el in doc.select(&selector).take(200) {
        let text = el.text().collect::<Vec<_>>().join(" ");
        let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
        if out.len() >= 40 {
            break;
        }
    }
    out
}

/// EPUB2 的目录在 NCX（media-type application/x-dtbncx+xml）里。
/// 只认 EPUB3 的 nav 会让老书的目录为空，章节拆分退化成正文标题启发式。
fn extract_epub_toc_from_ncx(
    zip: &mut ZipArchive<fs::File>,
    opf_content: &str,
    opf_dir: &str,
) -> Vec<String> {
    let Some(ncx_path) = find_opf_item_href(opf_content, opf_dir, |tag| {
        opf_attr(tag, &OPF_ATTR_MEDIATYPE_RE)
            .map(|v| v.to_ascii_lowercase().contains("dtbncx"))
            .unwrap_or(false)
    }) else {
        return vec![];
    };

    let Ok(mut file) = zip.by_name(&ncx_path) else {
        return vec![];
    };
    let mut xml = String::new();
    if file.read_to_string(&mut xml).is_err() {
        return vec![];
    }

    let doc = Html::parse_document(&xml);
    let selector = scraper::Selector::parse("navlabel text").expect("valid selector");
    let mut out = Vec::new();
    for el in doc.select(&selector).take(200) {
        let text = el.text().collect::<Vec<_>>().join(" ");
        let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
        if out.len() >= 40 {
            break;
        }
    }
    out
}

fn join_zip_path(base_dir: &str, rel: &str) -> String {
    let rel = rel.trim_start_matches('/');
    if base_dir.is_empty() {
        rel.to_string()
    } else {
        format!("{}/{}", base_dir.trim_end_matches('/'), rel)
    }
}

/// 每个 EPUB 章节文件都要过一遍 `html_to_text`，正则提到静态避免逐文件编译。
static HTML_NO_SCRIPT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<(script|style|svg|noscript)[^>]*>.*?</(script|style|svg|noscript)>").expect("valid regex"));
static HTML_BLOCK_BREAK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)</?(p|div|section|article|li|ul|ol|blockquote|h[1-6]|tr|table|br)[^>]*>").expect("valid regex"));

fn html_to_text(html: &str) -> String {
    let no_script = HTML_NO_SCRIPT_RE.replace_all(html, " ");
    let with_breaks = HTML_BLOCK_BREAK_RE.replace_all(&no_script, "\n");
    let doc = Html::parse_document(&with_breaks);
    let text = doc.root_element().text().collect::<Vec<_>>().join(" ");
    join_wrapped_lines(
        &normalize_line_endings(&decode_xml_entities(&text))
            .lines()
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>(),
    )
}

fn extract_markdown_headings(md: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^\s{0,3}#{1,6}\s+(.+?)\s*$").expect("valid regex");
    re.captures_iter(md)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
        .take(80)
        .collect()
}

fn markdown_to_plain_text(md: &str) -> String {
    let mut s = md.to_string();

    s = Regex::new(r"(?ms)^---\s*\n.*?\n---\s*")
        .expect("valid regex")
        .replace(&s, " ")
        .to_string();

    s = Regex::new(r"(?s)```.*?```")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"(?s)`[^`]*`")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"!\[[^\]]*\]\([^\)]*\)")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"\[([^\]]+)\]\([^\)]*\)")
        .expect("valid regex")
        .replace_all(&s, "$1")
        .to_string();
    s = Regex::new(r"<[^>]+>")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"(?m)^\s{0,3}[-*+]\s+")
        .expect("valid regex")
        .replace_all(&s, "")
        .to_string();
    s = Regex::new(r"(?m)^\s{0,3}>\s?")
        .expect("valid regex")
        .replace_all(&s, "")
        .to_string();
    s = Regex::new(r"(?m)^\s*\|.*\|\s*$")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = s.replace('#', " ");

    join_wrapped_lines(
        &normalize_line_endings(&s)
            .lines()
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>(),
    )
}

#[allow(dead_code)]
fn clean_text_for_tts(raw: &str) -> String {
    let mut s = normalize_line_endings(raw);

    s = s.replace('\u{feff}', " ");
    s = s.replace('\u{00a0}', " ");
    s = s.replace('\t', " ");
    s = Regex::new(r"https?://\S+")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"www\.\S+")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"\[[^\]]*\]\([^\)]*\)")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"<[^>]+>")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"\[(?:\d{1,3}|[ivxlcdmIVXLCDM]{1,12})\]")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = Regex::new(r"(?m)^\s*(第\s*\d+\s*页|page\s+\d+)\s*$")
        .expect("valid regex")
        .replace_all(&s, "\n")
        .to_string();
    s = Regex::new(r"(?m)^\s*[-=]{3,}\s*$")
        .expect("valid regex")
        .replace_all(&s, "\n")
        .to_string();
    s = Regex::new(r"[ \u{2000}-\u{200f}\u{2028}\u{2029}\u{2060}]+")
        .expect("valid regex")
        .replace_all(&s, " ")
        .to_string();
    s = decode_xml_entities(&s);
    s = normalize_punctuation(&s);

    let mut lines: Vec<String> = Vec::new();
    for line in s.lines() {
        let t = normalize_inline_spaces(line);
        if t.is_empty() {
            continue;
        }
        if is_noise_line(&t) {
            continue;
        }
        lines.push(t);
    }

    join_wrapped_lines(&lines)
        .replace("\n \n", "\n\n")
        .trim()
        .to_string()
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_inline_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[allow(dead_code)]
fn normalize_punctuation(text: &str) -> String {
    text.replace(' ', " ")
        .replace('•', " ")
        .replace('·', " ")
        .replace('—', "，")
        .replace('–', "-")
        .replace('…', "……")
        .replace('“', "\"")
        .replace('”', "\"")
        .replace('‘', "'")
        .replace('’', "'")
}

fn is_noise_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    if is_probable_page_noise(t) {
        return true;
    }
    if t.starts_with("图 ") || t.starts_with("图") || t.starts_with("Figure ") {
        return true;
    }
    if t.starts_with("表 ") || t.starts_with("表") || t.starts_with("Table ") {
        return true;
    }
    if t.chars().count() <= 2 && t.chars().all(|c| c.is_ascii_punctuation() || c.is_ascii_digit()) {
        return true;
    }
    false
}

fn is_probable_running_header(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.chars().count() > 60 {
        return false;
    }
    let digit_count = t.chars().filter(|c| c.is_ascii_digit()).count();
    let alpha_count = t.chars().filter(|c| c.is_ascii_alphabetic()).count();
    let cjk_count = t.chars().filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c)).count();
    (digit_count > 0 || alpha_count > 0 || cjk_count > 0) && !t.ends_with('。') && !t.ends_with('！') && !t.ends_with('？')
}

fn join_wrapped_lines(lines: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            if matches!(out.last(), Some(last) if !last.is_empty()) {
                out.push(String::new());
            }
            continue;
        }
        if let Some(last) = out.last_mut() {
            if !last.is_empty() && should_join_lines(last, t) {
                if last.ends_with('-') && is_ascii_word_like(t) {
                    last.pop();
                    last.push_str(t);
                } else {
                    let glue = if needs_space_between(last, t) { " " } else { "" };
                    last.push_str(glue);
                    last.push_str(t);
                }
                continue;
            }
        }
        out.push(t.to_string());
    }
    out.join("\n")
        .split("\n\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn should_join_lines(prev: &str, next: &str) -> bool {
    let prev = prev.trim_end();
    let next = next.trim_start();
    if prev.is_empty() || next.is_empty() {
        return false;
    }
    if looks_like_heading(prev) || looks_like_heading(next) {
        return false;
    }
    if prev.ends_with('。')
        || prev.ends_with('！')
        || prev.ends_with('？')
        || prev.ends_with('；')
        || prev.ends_with(';')
        || prev.ends_with(':')
        || prev.ends_with('：')
    {
        return false;
    }
    if next.starts_with('第') && looks_like_heading(next) {
        return false;
    }
    true
}

fn needs_space_between(prev: &str, next: &str) -> bool {
    let prev_last = prev.chars().last().unwrap_or(' ');
    let next_first = next.chars().next().unwrap_or(' ');
    prev_last.is_ascii_alphanumeric() && next_first.is_ascii_alphanumeric()
}

fn is_ascii_word_like(text: &str) -> bool {
    text.chars().take(12).all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

/// `join_wrapped_lines` 会逐行调用 `looks_like_heading`，正则必须提到静态，
/// 否则一本几十万字的书要重复编译数万次。
static HEADING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(第[一二三四五六七八九十百千万0-9]+[章节回卷篇].*|chapter\s+[0-9ivxlcdm]+.*|[0-9]+(?:\.[0-9]+){0,3}\s+.+)$")
        .expect("valid regex")
});

fn looks_like_heading(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.chars().count() > 80 {
        return false;
    }
    HEADING_RE.is_match(t)
}

#[allow(dead_code)]
fn split_into_sentences(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut current = String::new();
    for (idx, ch) in chars.iter().enumerate() {
        current.push(*ch);
        let next = chars.get(idx + 1).copied().unwrap_or('\0');
        let prev = if idx > 0 { chars[idx - 1] } else { '\0' };
        let should_break = match *ch {
            '。' | '！' | '？' | '；' | '…' => true,
            '\n' => true,
            '.' => !(prev.is_ascii_digit() && next.is_ascii_digit()),
            '!' | '?' | ';' => true,
            _ => false,
        };
        if should_break {
            let t = current.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

#[allow(dead_code)]
fn split_oversized_sentence(sentence: &str, base_limit: usize) -> Vec<String> {
    let lang = detect_chunk_lang(sentence);
    let hard_limit = if lang == ChunkLang::Zh {
        base_limit.saturating_mul(2).max(160)
    } else {
        base_limit.saturating_mul(3).max(240)
    };
    if sentence.chars().count() <= hard_limit {
        return vec![sentence.trim().to_string()];
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    for token in sentence.split_inclusive(['，', '、', '：', ',', ':']) {
        if current.chars().count() + token.chars().count() > hard_limit && !current.trim().is_empty() {
            parts.push(current.trim().to_string());
            current.clear();
        }
        current.push_str(token);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    if parts.len() <= 1 {
        return split_by_word_boundary(sentence, hard_limit);
    }
    parts
}

fn split_by_word_boundary(text: &str, limit: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.chars().count() + word.chars().count() + 1 > limit && !current.trim().is_empty() {
            parts.push(current.trim().to_string());
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    if parts.is_empty() {
        vec![text.trim().to_string()]
    } else {
        parts
    }
}

fn extract_outline_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.chars().count() > 80 {
            continue;
        }
        if looks_like_heading(t) {
            out.push(t.to_string());
        }
        if out.len() >= 80 {
            break;
        }
    }
    out
}

#[allow(dead_code)]
fn split_text_chunks(text: &str, base_limit: usize) -> Vec<TextChunk> {
    let paragraphs = text
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if paragraphs.is_empty() {
        return vec![];
    }

    let zh_limit = base_limit;
    let en_limit = (base_limit.saturating_mul(2)).clamp(120, 640);

    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut chunk_buf = String::new();
    let mut chunk_lang: Option<ChunkLang> = None;

    for paragraph in &paragraphs {
        let sentences = split_into_sentences(paragraph);
        for sentence in &sentences {
            let fragments = split_oversized_sentence(sentence, base_limit);
            for fragment in fragments {
                let lang = detect_chunk_lang(&fragment);
                let limit = if lang == ChunkLang::Zh { zh_limit } else { en_limit };

                let should_flush = match chunk_lang {
                    Some(l) if l != lang => true,
                    _ => chunk_buf.chars().count() + fragment.chars().count() + 1 > limit,
                };

                if should_flush && !chunk_buf.trim().is_empty() {
                    chunks.push(TextChunk {
                        index: chunks.len() + 1,
                        text: chunk_buf.trim().to_string(),
                        lang: chunk_lang.unwrap_or(lang),
                    });
                    chunk_buf.clear();
                }

                if !chunk_buf.is_empty() {
                    chunk_buf.push(' ');
                }
                chunk_buf.push_str(fragment.trim());
                chunk_lang = Some(lang);
            }
        }
        if !chunk_buf.trim().is_empty() {
            chunks.push(TextChunk {
                index: chunks.len() + 1,
                text: chunk_buf.trim().to_string(),
                lang: chunk_lang.unwrap_or(ChunkLang::Zh),
            });
            chunk_buf.clear();
            chunk_lang = None;
        }
    }

    if chunks.is_empty() {
        warn!("split_text_chunks produced empty chunks");
    }

    chunks
}

fn detect_chunk_lang(text: &str) -> ChunkLang {
    let mut cjk = 0usize;
    let mut latin = 0usize;

    for ch in text.chars() {
        if ('\u{4E00}'..='\u{9FFF}').contains(&ch) {
            cjk += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        }
    }

    if cjk >= latin {
        ChunkLang::Zh
    } else {
        ChunkLang::En
    }
}

/// 内存版合并（整本音频解成 f32）。生产路径已换成 `merge_wav_files_to_path` 的流式实现，
/// 这里只作为等价性测试的参照保留，避免它随 release 一起编进二进制。
#[cfg(test)]
fn merge_wav_parts(parts: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if parts.is_empty() {
        return Err("没有可拼接的音频分片".to_string());
    }

    let mut fmt_chunk: Option<Vec<u8>> = None;
    let mut all_data: Vec<u8> = Vec::new();

    for (idx, bytes) in parts.iter().enumerate() {
        let (fmt, data) = parse_wav_fmt_data(bytes).map_err(|e| format!("第 {} 段 WAV 无效: {}", idx + 1, e))?;
        if let Some(prev) = &fmt_chunk {
            if prev != &fmt {
                warn!(index = idx + 1, "WAV fmt chunk differs, continue with first fmt");
            }
        } else {
            fmt_chunk = Some(fmt);
        }
        all_data.extend_from_slice(&data);
    }

    let fmt = fmt_chunk.ok_or_else(|| "缺少 WAV fmt chunk".to_string())?;
    build_wav(&fmt, &all_data)
}

fn parse_wav_fmt_data(bytes: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if bytes.len() < 12 {
        return Err("字节长度过短".to_string());
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("不是 RIFF/WAVE 格式".to_string());
    }

    let mut offset = 12usize;
    let mut fmt: Option<Vec<u8>> = None;
    let mut data: Option<Vec<u8>> = None;

    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let start = offset + 8;
        let end = start.saturating_add(size);
        if end > bytes.len() {
            break;
        }

        if id == b"fmt " {
            fmt = Some(bytes[start..end].to_vec());
        } else if id == b"data" {
            data = Some(bytes[start..end].to_vec());
        }

        offset = end;
        if size % 2 == 1 {
            offset = offset.saturating_add(1);
        }
    }

    let fmt = fmt.ok_or_else(|| "缺少 fmt chunk".to_string())?;
    let data = data.ok_or_else(|| "缺少 data chunk".to_string())?;
    Ok((fmt, data))
}

/// 只拼 WAV 头（到 data 的长度字段为止，不含 data 载荷）。
/// 流式合并时先写占位头、写完再回填，所以头和载荷必须能分开构造。
fn build_wav_header(fmt_chunk_data: &[u8], data_size: u32) -> Vec<u8> {
    let fmt_size = fmt_chunk_data.len() as u32;
    let riff_size = 4u32
        .saturating_add(8u32.saturating_add(fmt_size))
        .saturating_add(8u32.saturating_add(data_size));

    let mut out = Vec::with_capacity(fmt_chunk_data.len() + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&fmt_size.to_le_bytes());
    out.extend_from_slice(fmt_chunk_data);
    if fmt_size % 2 == 1 {
        out.push(0);
    }

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    out
}

/// 整块构造 WAV。流式路径用的是 `build_wav_header` + 边写边补，这里只供测试构造样例。
#[cfg(test)]
fn build_wav(fmt_chunk_data: &[u8], data_chunk_data: &[u8]) -> Result<Vec<u8>, String> {
    let data_size = data_chunk_data.len() as u32;
    let mut out = build_wav_header(fmt_chunk_data, data_size);
    out.extend_from_slice(data_chunk_data);
    if data_size % 2 == 1 {
        out.push(0);
    }

    if out.len() < 44 {
        return Err("生成 WAV 失败".to_string());
    }

    Ok(out)
}

fn clean_text_for_tts_pro(raw: &str) -> String {
    let mut text = normalize_line_endings(raw)
        .replace('\u{feff}', " ")
        .replace('\u{00a0}', " ")
        .replace('\t', " ");

    for pattern in [
        r"https?://\S+",
        r"www\.\S+",
        r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b",
        r"\[(?:\d{1,3}|[ivxlcdmIVXLCDM]{1,12})\]",
        r"\(\s*\d{1,3}\s*\)",
        r"(?m)^\s*(page\s+\d+|\d+\s*/\s*\d+)\s*$",
    ] {
        text = Regex::new(pattern)
            .expect("valid regex")
            .replace_all(&text, " ")
            .to_string();
    }

    text = decode_xml_entities(&text);
    text = normalize_book_punctuation(&text);

    let mut lines = Vec::new();
    for raw_line in text.lines() {
        let line = normalize_inline_spaces(raw_line);
        if line.is_empty() || is_noise_line(&line) {
            continue;
        }
        lines.push(line);
    }

    join_wrapped_lines(&lines)
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn normalize_book_punctuation(text: &str) -> String {
    text.replace('“', "\"")
        .replace('”', "\"")
        .replace('‘', "'")
        .replace('’', "'")
        .replace('—', " - ")
        .replace('–', "-")
        .replace('•', " ")
        .replace('·', " ")
        .replace("……", "…")
}

pub(crate) fn split_into_sentences_pro(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut current = String::new();
    let closing_quotes = ['"', '\'', ')', '）', '】', '》'];

    let mut idx = 0usize;
    while idx < chars.len() {
        let ch = chars[idx];
        current.push(ch);

        let prev = if idx > 0 { chars[idx - 1] } else { '\0' };
        let next = chars.get(idx + 1).copied().unwrap_or('\0');
        let mut should_break = matches!(ch, '。' | '！' | '？' | '；' | '!' | '?' | ';' | '…');
        if ch == '.' {
            should_break = !(prev.is_ascii_digit() && next.is_ascii_digit());
        } else if ch == '\n' {
            should_break = current.trim().chars().count() >= 8;
        }

        if should_break {
            let mut lookahead = idx + 1;
            while let Some(extra) = chars.get(lookahead) {
                if closing_quotes.contains(extra) {
                    current.push(*extra);
                    lookahead += 1;
                    idx += 1;
                } else {
                    break;
                }
            }

            let segment = current.trim();
            if !segment.is_empty() {
                out.push(segment.to_string());
            }
            current.clear();
        }

        idx += 1;
    }

    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

pub(crate) fn split_oversized_sentence_pro(sentence: &str, base_limit: usize) -> Vec<String> {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let lang = detect_chunk_lang(trimmed);
    let target_limit = if lang == ChunkLang::Zh {
        base_limit.clamp(80, 260)
    } else {
        (base_limit.saturating_mul(2)).clamp(120, 420)
    };
    let hard_limit = target_limit.max(40);

    if trimmed.chars().count() <= hard_limit {
        return vec![trimmed.to_string()];
    }

    let mut pieces = Vec::new();
    let mut buf = String::new();

    for clause in trimmed.split_inclusive(['，', '、', '：', '；', ',', ':', ';']) {
        let clause = clause.trim();
        if clause.is_empty() {
            continue;
        }

        let next_len = buf.chars().count() + clause.chars().count();
        if next_len > hard_limit && buf.trim().chars().count() >= 6 {
            pieces.push(buf.trim().to_string());
            buf.clear();
        }
        if !buf.is_empty() && needs_space_between(&buf, clause) {
            buf.push(' ');
        }
        buf.push_str(clause);
    }

    if !buf.trim().is_empty() {
        pieces.push(buf.trim().to_string());
    }

    if pieces.len() <= 1 {
        split_by_word_boundary(trimmed, hard_limit)
    } else {
        pieces
    }
}

fn split_text_chunks_pro(text: &str, base_limit: usize) -> Vec<TextChunk> {
    let paragraphs = text
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if paragraphs.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut buffer_lang: Option<ChunkLang> = None;

    for paragraph in paragraphs {
        for sentence in split_into_sentences_pro(paragraph) {
            for fragment in split_oversized_sentence_pro(&sentence, base_limit) {
                let fragment = fragment.trim();
                if fragment.is_empty() {
                    continue;
                }

                let lang = detect_chunk_lang(fragment);
                let limit = if lang == ChunkLang::Zh {
                    base_limit.clamp(80, 260)
                } else {
                    (base_limit.saturating_mul(2)).clamp(120, 420)
                };

                let next_len = buffer.chars().count()
                    + fragment.chars().count()
                    + if buffer.is_empty() { 0 } else { 1 };
                let should_flush = match buffer_lang {
                    Some(existing) if existing != lang => true,
                    _ => next_len > limit,
                };

                if should_flush && !buffer.trim().is_empty() {
                    chunks.push(TextChunk {
                        index: chunks.len() + 1,
                        text: buffer.trim().to_string(),
                        lang: buffer_lang.unwrap_or(lang),
                    });
                    buffer.clear();
                }

                if !buffer.is_empty() {
                    if needs_space_between(&buffer, fragment) {
                        buffer.push(' ');
                    }
                }
                buffer.push_str(fragment);
                buffer_lang = Some(lang);
            }
        }

        if !buffer.trim().is_empty() {
            chunks.push(TextChunk {
                index: chunks.len() + 1,
                text: buffer.trim().to_string(),
                lang: buffer_lang.unwrap_or(ChunkLang::Zh),
            });
            buffer.clear();
            buffer_lang = None;
        }
    }

    chunks
}

/// 段间淡接时长（毫秒）。
const CROSSFADE_MS: u32 = 36;

/// 把 `extra` 接到逻辑音频流末尾，同时只把最后 `keep` 个样本押后不写。
///
/// 之所以要押后：跨淡叠接会改写「上一段的尾部」，而这部分可能已经写出去了。
/// 押后量只有 overlap（36ms）级别，所以既能流式写盘，又不改变原有淡接语义。
fn write_held_back(
    writer: &mut BufWriter<fs::File>,
    tail: &mut Vec<f32>,
    extra: &[f32],
    keep: usize,
    data_bytes: &mut u64,
) -> Result<(), String> {
    let total = tail.len() + extra.len();
    if total <= keep {
        tail.extend_from_slice(extra);
        return Ok(());
    }

    let from_tail = tail.len().min(total - keep);
    if from_tail > 0 {
        let encoded = encode_pcm16_samples(&tail[..from_tail]);
        writer.write_all(&encoded).map_err(|e| format!("写入音频失败: {}", e))?;
        *data_bytes += encoded.len() as u64;
        tail.drain(..from_tail);
    }

    let from_extra = total - keep - from_tail;
    if from_extra > 0 {
        let encoded = encode_pcm16_samples(&extra[..from_extra]);
        writer.write_all(&encoded).map_err(|e| format!("写入音频失败: {}", e))?;
        *data_bytes += encoded.len() as u64;
    }
    tail.extend_from_slice(&extra[from_extra..]);
    Ok(())
}

/// 把占位头回填成真实长度。`writer` 必须已经 flush 过。
fn patch_wav_header(
    writer: &mut BufWriter<fs::File>,
    fmt_chunk_data: &[u8],
    data_bytes: u64,
) -> Result<(), String> {
    writer.flush().map_err(|e| format!("写入音频失败: {}", e))?;
    let header = build_wav_header(fmt_chunk_data, data_bytes as u32);
    let file = writer.get_mut();
    file.seek(SeekFrom::Start(0)).map_err(|e| format!("写入音频失败: {}", e))?;
    file.write_all(&header).map_err(|e| format!("写入音频失败: {}", e))?;
    file.flush().map_err(|e| format!("写入音频失败: {}", e))
}

/// 流式合并音频分片到 `output_path`。
///
/// 旧实现把整本音频解成 f32 常驻内存再编码（3 小时书约 690MB、10 小时直接 OOM），
/// 而且先把所有分段文件读进内存。这里改为逐段读取、边算边写，最后回填 WAV 长度。
fn merge_wav_files_to_path(
    paths: &[PathBuf],
    chunks: &[TextChunk],
    output_path: &Path,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有可拼接的音频分片".to_string());
    }

    // 第一段决定 fmt 与采样参数。只有 16-bit PCM、且段数与文本数一一对应时
    // 才做后处理与淡接，否则退化成纯字节拼接（与旧行为一致）。
    let first = fs::read(&paths[0]).map_err(|e| format!("读取音频分片失败: {}", e))?;
    let probed = parse_wav_fmt_data(&first)
        .ok()
        .and_then(|(fmt, _)| parse_wav_format_fields(&fmt).map(|fields| (fmt, fields)));
    let Some((fmt, (audio_format, channels, sample_rate, bits_per_sample))) = probed else {
        return concat_wav_files_to_path(paths, output_path);
    };
    if audio_format != 1 || bits_per_sample != 16 || paths.len() != chunks.len() {
        return concat_wav_files_to_path(paths, output_path);
    }

    let file = fs::File::create(output_path).map_err(|e| format!("创建音频文件失败: {}", e))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&build_wav_header(&fmt, 0))
        .map_err(|e| format!("写入音频失败: {}", e))?;

    let overlap_frames = ((sample_rate as usize * CROSSFADE_MS as usize) / 1000).max(1);
    let overlap_samples = overlap_frames.saturating_mul(channels as usize);

    let mut data_bytes = 0u64;
    let mut tail: Vec<f32> = Vec::new();

    for (index, path) in paths.iter().enumerate() {
        let bytes = fs::read(path).map_err(|e| format!("读取音频分片失败: {}", e))?;
        let (_, data) = parse_wav_fmt_data(&bytes)?;
        let mut samples = decode_pcm16_samples(&data);
        postprocess_segment_samples(&mut samples, sample_rate, channels);

        if index == 0 {
            write_held_back(&mut writer, &mut tail, &samples, overlap_samples, &mut data_bytes)?;
            continue;
        }

        let pause_ms = derive_pause_after_ms(&chunks[index - 1].text);
        if pause_ms <= 90 {
            // tail 正好是已写出部分的最后一段，直接在它上面做淡出/淡入混合。
            let blend = overlap_samples.min(tail.len()).min(samples.len());
            for k in 0..blend {
                let fade_out = 1.0 - (k as f32 / blend as f32);
                let fade_in = k as f32 / blend as f32;
                tail[k] = tail[k] * fade_out + samples[k] * fade_in;
            }
            write_held_back(&mut writer, &mut tail, &samples[blend..], overlap_samples, &mut data_bytes)?;
        } else {
            write_held_back(&mut writer, &mut tail, &samples, overlap_samples, &mut data_bytes)?;
        }

        let silence = build_silence_samples(sample_rate, channels, pause_ms);
        write_held_back(&mut writer, &mut tail, &silence, overlap_samples, &mut data_bytes)?;
    }

    // 收尾：把押后的尾部补上，再回填真实长度。
    let encoded = encode_pcm16_samples(&tail);
    writer.write_all(&encoded).map_err(|e| format!("写入音频失败: {}", e))?;
    data_bytes += encoded.len() as u64;
    patch_wav_header(&mut writer, &fmt, data_bytes)
}

/// 纯字节拼接（不做后处理）。同样流式，避免把整本音频读进内存。
fn concat_wav_files_to_path(paths: &[PathBuf], output_path: &Path) -> Result<(), String> {
    let file = fs::File::create(output_path).map_err(|e| format!("创建音频文件失败: {}", e))?;
    let mut writer = BufWriter::new(file);

    let mut fmt_chunk: Option<Vec<u8>> = None;
    let mut data_bytes = 0u64;
    for (idx, path) in paths.iter().enumerate() {
        let bytes = fs::read(path).map_err(|e| format!("读取音频分片失败: {}", e))?;
        let (fmt, data) =
            parse_wav_fmt_data(&bytes).map_err(|e| format!("第 {} 段 WAV 无效: {}", idx + 1, e))?;

        match &fmt_chunk {
            None => {
                writer
                    .write_all(&build_wav_header(&fmt, 0))
                    .map_err(|e| format!("写入音频失败: {}", e))?;
                fmt_chunk = Some(fmt);
            }
            Some(prev) if prev != &fmt => {
                warn!(index = idx + 1, "WAV fmt chunk differs, continue with first fmt");
            }
            Some(_) => {}
        }

        writer.write_all(&data).map_err(|e| format!("写入音频失败: {}", e))?;
        data_bytes += data.len() as u64;
    }

    let fmt = fmt_chunk.ok_or_else(|| "缺少 WAV fmt chunk".to_string())?;
    // 奇数长度载荷要补一个填充字节，WAV 规范如此；build_wav 里也是这个行为。
    if data_bytes % 2 == 1 {
        writer.write_all(&[0u8]).map_err(|e| format!("写入音频失败: {}", e))?;
    }
    patch_wav_header(&mut writer, &fmt, data_bytes)
}

/// 内存版「后处理 + 淡接 + 静音」合并，作为 `merge_wav_files_to_path` 的等价性参照。
#[cfg(test)]
fn merge_wav_parts_with_postprocess(parts: &[Vec<u8>], chunks: &[TextChunk]) -> Result<Vec<u8>, String> {
    if parts.is_empty() {
        return Err("没有可拼接的音频分片".to_string());
    }
    if parts.len() != chunks.len() {
        return merge_wav_parts(parts);
    }

    let mut fmt_chunk: Option<Vec<u8>> = None;
    let mut sample_rate = 16_000u32;
    let mut channels = 1u16;
    let mut all_pcm: Vec<f32> = Vec::new();

    for (index, bytes) in parts.iter().enumerate() {
        let (fmt, data) = parse_wav_fmt_data(bytes)?;
        let Some((audio_format, pcm_channels, pcm_sample_rate, bits_per_sample)) = parse_wav_format_fields(&fmt) else {
            return merge_wav_parts(parts);
        };
        if audio_format != 1 || bits_per_sample != 16 {
            return merge_wav_parts(parts);
        }

        if fmt_chunk.is_none() {
            fmt_chunk = Some(fmt.clone());
            sample_rate = pcm_sample_rate;
            channels = pcm_channels;
        }

        let mut samples = decode_pcm16_samples(&data);
        postprocess_segment_samples(&mut samples, sample_rate, channels);

        if !all_pcm.is_empty() {
            let pause_ms = derive_pause_after_ms(&chunks[index - 1].text);
            if pause_ms <= 90 {
                append_with_crossfade(&mut all_pcm, &samples, sample_rate, channels, 36);
            } else {
                all_pcm.extend_from_slice(&samples);
            }
            let silence = build_silence_samples(sample_rate, channels, pause_ms);
            all_pcm.extend_from_slice(&silence);
        } else {
            all_pcm.extend_from_slice(&samples);
        }
    }

    let fmt = fmt_chunk.ok_or_else(|| "缺少 WAV fmt chunk".to_string())?;
    let data = encode_pcm16_samples(&all_pcm);
    build_wav(&fmt, &data)
}

fn parse_wav_format_fields(fmt: &[u8]) -> Option<(u16, u16, u32, u16)> {
    if fmt.len() < 16 {
        return None;
    }
    let audio_format = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
    Some((audio_format, channels, sample_rate, bits_per_sample))
}

fn wav_duration_ms(bytes: &[u8]) -> Result<u64, String> {
    let (fmt, data) = parse_wav_fmt_data(bytes)?;
    let Some((audio_format, channels, sample_rate, bits_per_sample)) = parse_wav_format_fields(&fmt) else {
        return Err("缺少 WAV 格式字段".to_string());
    };
    if audio_format != 1 || sample_rate == 0 || channels == 0 || bits_per_sample == 0 {
        return Err("不支持的 WAV 参数".to_string());
    }

    let bytes_per_sample = (bits_per_sample as u64 / 8).max(1);
    let frame_bytes = bytes_per_sample.saturating_mul(channels as u64).max(1);
    let frames = data.len() as u64 / frame_bytes;
    Ok(frames.saturating_mul(1000) / sample_rate as u64)
}

fn decode_pcm16_samples(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f32 / i16::MAX as f32
        })
        .collect()
}

fn encode_pcm16_samples(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&scaled.to_le_bytes());
    }
    out
}

fn postprocess_segment_samples(samples: &mut [f32], sample_rate: u32, channels: u16) {
    if samples.is_empty() {
        return;
    }

    let peak = samples
        .iter()
        .fold(0.0_f32, |acc, value| acc.max(value.abs()))
        .max(1e-6);
    let gain = (0.92 / peak).min(1.8);
    for sample in samples.iter_mut() {
        *sample *= gain;
        let abs = sample.abs();
        if abs > 0.72 {
            let compressed = 0.72 + (abs - 0.72) / 3.0;
            *sample = sample.signum() * compressed;
        }
    }

    apply_edge_fade(samples, sample_rate, channels, 8);
}

fn apply_edge_fade(samples: &mut [f32], sample_rate: u32, channels: u16, fade_ms: u32) {
    let fade_frames = ((sample_rate as usize * fade_ms as usize) / 1000).max(1);
    let fade_samples = fade_frames.saturating_mul(channels as usize).min(samples.len() / 2);
    if fade_samples == 0 {
        return;
    }

    for i in 0..fade_samples {
        let gain = i as f32 / fade_samples as f32;
        samples[i] *= gain;
        let tail = samples.len() - 1 - i;
        samples[tail] *= gain;
    }
}

fn build_silence_samples(sample_rate: u32, channels: u16, duration_ms: u32) -> Vec<f32> {
    let frames = ((sample_rate as u64 * duration_ms as u64) / 1000) as usize;
    vec![0.0; frames.saturating_mul(channels as usize)]
}

/// 直接在累积缓冲尾部做淡接。流式路径改用 `write_held_back` 的押后写，
/// 这里只作为等价性测试的参照保留。
#[cfg(test)]
fn append_with_crossfade(
    base: &mut Vec<f32>,
    next: &[f32],
    sample_rate: u32,
    channels: u16,
    crossfade_ms: u32,
) {
    if base.is_empty() || next.is_empty() {
        base.extend_from_slice(next);
        return;
    }

    let overlap_frames = ((sample_rate as usize * crossfade_ms as usize) / 1000).max(1);
    let overlap_samples = overlap_frames
        .saturating_mul(channels as usize)
        .min(base.len())
        .min(next.len());

    if overlap_samples == 0 {
        base.extend_from_slice(next);
        return;
    }

    let split_at = base.len() - overlap_samples;
    // 原地混合。旧的实现是 `base[..split_at].to_vec()` 再回写整个缓冲，
    // 而本函数每段都会调用一次，累加长度线性增长 → 总体退化成 O(n²)。
    // 重叠区只有 overlap_samples 个样本需要旧值，先取出来再截断即可。
    let tail: Vec<f32> = base[split_at..].to_vec();
    base.truncate(split_at);
    base.reserve(next.len());

    for i in 0..overlap_samples {
        let fade_out = 1.0 - (i as f32 / overlap_samples as f32);
        let fade_in = i as f32 / overlap_samples as f32;
        base.push(tail[i] * fade_out + next[i] * fade_in);
    }
    base.extend_from_slice(&next[overlap_samples..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯文本嗅探是「任意纯文本文件」这条路的唯一判据，必须钉住行为：
    /// UTF-8 / GBK 中文要被接受，二进制要被拒。
    #[test]
    fn plain_text_sniffing_accepts_text_and_rejects_binary() {
        assert!(looks_like_plain_text("第一章 起源\n他说过一句话。".as_bytes()));
        assert!(looks_like_plain_text(b""));

        // GBK 中文：字节多在 0x81 以上，不能被误判成二进制。
        let (gbk, _, _) = GBK.encode("第一章 起源。");
        assert!(looks_like_plain_text(gbk.as_ref()));

        // 含 NUL（PNG 头、可执行文件）直接判为二进制。
        assert!(!looks_like_plain_text(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"));
        // 无 NUL 但控制字符占比过高（二进制数据）同样判为二进制。
        let noisy = (1u8..=255).cycle().take(1024).collect::<Vec<u8>>();
        assert!(!looks_like_plain_text(&noisy));
    }

    fn pcm16_wav(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let bits = 16u16;
        let block_align = channels * bits / 8;
        let byte_rate = sample_rate * block_align as u32;
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&sample_rate.to_le_bytes());
        fmt.extend_from_slice(&byte_rate.to_le_bytes());
        fmt.extend_from_slice(&block_align.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());

        let mut data = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            data.extend_from_slice(&sample.to_le_bytes());
        }
        build_wav(&fmt, &data).expect("构造样例 WAV")
    }

    /// 流式合并必须与旧的内存版逐字节一致——这是替换音频输出路径的唯一凭据。
    #[test]
    fn streaming_merge_matches_in_memory_reference() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let (sample_rate, channels) = (16_000u32, 1u16);

        // 段长特意长短不一，覆盖「比 overlap 还短」这类边界。
        let specs: [(&str, usize); 5] = [
            ("第一段。", 400),
            ("第二段，", 120),
            ("第三段；", 17),
            ("第四段", 5),
            ("第五段……", 900),
        ];

        let mut paths = Vec::new();
        let mut parts = Vec::new();
        let mut chunks = Vec::new();
        for (index, (text, len)) in specs.iter().enumerate() {
            let samples = (0..*len)
                .map(|k| (((k as i32 * 37 + index as i32 * 11) % 2000) - 1000) as i16)
                .collect::<Vec<_>>();
            let bytes = pcm16_wav(&samples, sample_rate, channels);
            let path = tmp.path().join(format!("{:05}.wav", index));
            fs::write(&path, &bytes).expect("写入样例分片");
            paths.push(path);
            parts.push(bytes);
            chunks.push(TextChunk {
                index: index + 1,
                text: (*text).to_string(),
                lang: ChunkLang::Zh,
            });
        }

        let expected = merge_wav_parts_with_postprocess(&parts, &chunks).expect("内存版合并");
        let output_path = tmp.path().join("merged.wav");
        merge_wav_files_to_path(&paths, &chunks, &output_path).expect("流式合并");
        let actual = fs::read(&output_path).expect("读取合并结果");

        if actual != expected {
            let first_diff = actual.iter().zip(&expected).position(|(a, b)| a != b);
            panic!(
                "流式合并与内存版不一致：长度 {} vs {}，首个差异字节 {:?}",
                actual.len(),
                expected.len(),
                first_diff
            );
        }
    }

    /// 与旧实现逐字对应的朴素版本，用来钉住 `append_with_crossfade` 原地化后的语义。
    fn naive_crossfade(base: &[f32], next: &[f32], sample_rate: u32, channels: u16, crossfade_ms: u32) -> Vec<f32> {
        let mut out = base.to_vec();
        if base.is_empty() || next.is_empty() {
            out.extend_from_slice(next);
            return out;
        }
        let overlap_frames = ((sample_rate as usize * crossfade_ms as usize) / 1000).max(1);
        let overlap = overlap_frames
            .saturating_mul(channels as usize)
            .min(base.len())
            .min(next.len());
        if overlap == 0 {
            out.extend_from_slice(next);
            return out;
        }
        let split_at = base.len() - overlap;
        out.truncate(split_at);
        for i in 0..overlap {
            let fade_out = 1.0 - (i as f32 / overlap as f32);
            let fade_in = i as f32 / overlap as f32;
            out.push(base[split_at + i] * fade_out + next[i] * fade_in);
        }
        out.extend_from_slice(&next[overlap..]);
        out
    }

    /// `derive_pause_after_ms` 最小返回 110ms，而淡接阈值是 `<= 90`，
    /// 所以淡接分支在生产路径上其实走不到——只能直接测这个纯函数。
    /// 它仍然值得测：原地化改动一旦写错，将来阈值被调整时就会静默产出错误音频。
    #[test]
    fn in_place_crossfade_keeps_original_semantics() {
        let (sample_rate, channels, crossfade_ms) = (16_000u32, 1u16, 36u32);
        let base: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let next: Vec<f32> = (0..40).map(|i| 1.0 - i as f32 * 0.01).collect();

        let cases: Vec<(Vec<f32>, Vec<f32>)> = vec![
            (base.clone(), next.clone()),
            // next 比 overlap 还短。
            (base.clone(), vec![0.5; 5]),
            // base 比 overlap 还短。
            (vec![0.25; 3], next.clone()),
            (Vec::new(), next.clone()),
            (base.clone(), Vec::new()),
        ];

        for (index, (b, n)) in cases.iter().enumerate() {
            let mut actual = b.clone();
            append_with_crossfade(&mut actual, n, sample_rate, channels, crossfade_ms);
            let expected = naive_crossfade(b, n, sample_rate, channels, crossfade_ms);
            assert_eq!(actual, expected, "第 {} 组输入原地化后语义变了", index);
        }
    }
}
