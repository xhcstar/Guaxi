use crate::commands::chatview::{audio_text_to_speech, AudioCommonRequest, TextToSpeechRequest};
use crate::commands::settingview;
use base64::{engine::general_purpose, Engine as _};
use encoding_rs::GBK;
use once_cell::sync::Lazy;
use regex::Regex;
use scraper::Html;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Instant;
use tauri::{Emitter, Window};
use tracing::{info, warn};
use zip::ZipArchive;

const SHERPA_PROVIDER: &str = "sherpa-onnx";
const DEFAULT_MODEL: &str = "utils/Models/tts/kokoro_multi_lang/model.onnx";
const DEFAULT_TOKENS: &str = "utils/Models/tts/kokoro_multi_lang/tokens.txt";
const DEFAULT_EXECUTION_PROVIDER: &str = "cpu";
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

    let output_dir = PathBuf::from(request.output_dir.trim());
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
    let chunks = split_text_chunks_pro(&cleaned, max_chunk_chars);
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
    let output_path = output_dir.join(format!("{}_audiobook.wav", stem));
    let segment_json_path = output_dir.join(format!("{}_segments.json", stem));
    let cleaned_text_hash = hash_text(&cleaned);
    let work_dir = output_dir.join(format!(
        "{}_tts_work_{:016x}_{}_{}",
        stem,
        cleaned_text_hash,
        max_chunk_chars,
        if request.split_by_toc { "chap" } else { "full" }
    ));
    let chunk_dir = work_dir.join("chunks");
    fs::create_dir_all(&chunk_dir).map_err(|e| format!("创建分段缓存目录失败: {}", e))?;
    let resume_state_path = work_dir.join("resume_state.json");

    let chapters = if request.split_by_toc {
        split_text_into_chapters(&cleaned, &extracted.toc)
    } else {
        Vec::new()
    };
    info!(chapter_count = chapters.len(), "tts_book_convert chapter split done");

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

    let tts_cfg = settingview::get_tts_config();
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
                    chunk_count: split_text_chunks_pro(chapter_text, max_chunk_chars).len(),
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
    for chunk in &chunks {
        check_task_control(
            &task_id,
            &mut resume_state,
            &resume_state_path,
            completed_steps,
            total_steps,
        )?;
        match chunk.lang {
            ChunkLang::Zh => zh_chunks += 1,
            ChunkLang::En => en_chunks += 1,
        }
        let chunk_path = chunk_dir.join(format!("{:05}.wav", chunk.index));
        let resumed = chunk_path.exists();
        if !resumed {
            let bytes = synthesize_chunk_audio(
                chunk,
                &tts_exe,
                &tts_model,
                &tts_tokens,
                &tts_mode_hint,
                &speaker_id,
                speed,
                &execution_provider,
            )
            .await?;
            fs::write(&chunk_path, bytes).map_err(|e| format!("写入分段音频失败: {}", e))?;
        } else {
            resumed_any = true;
        }
        if !resume_state.completed_chunks.contains(&chunk.index) {
            resume_state.completed_chunks.push(chunk.index);
            resume_state.completed_chunks.sort_unstable();
            save_resume_state(&resume_state_path, &resume_state)?;
        }
        let chunk_bytes = fs::read(&chunk_path).map_err(|e| format!("读取分段音频失败: {}", e))?;
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
        save_resume_state(&resume_state_path, &resume_state)?;
        emit_tts_book_progress(
            &window,
            make_progress(
                &task_id,
                &file_path,
                &file_name,
                "synth_chunk",
                completed_steps,
                total_steps,
                if resumed {
                    format!("复用已完成分段 {}/{}", chunk.index, chunks.len())
                } else {
                    format!("正在合成分段 {}/{}", chunk.index, chunks.len())
                },
                resumed,
                false,
                &output_path,
                &segment_json_path,
            ),
        )?;
    }
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
                    chunk_count: split_text_chunks_pro(chapter_text, max_chunk_chars).len(),
                    text: chapter_text.clone(),
                })
                .collect(),
        },
    )?;

    let merged_wav = merge_wav_files_with_postprocess(&chunk_paths, &chunks)?;
    fs::write(&output_path, merged_wav).map_err(|e| format!("写入音频文件失败: {}", e))?;
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
        let chapter_dir = output_dir.join(format!("{}_chapters", stem));
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
            let resumed = chapter_path.exists();
            if !resumed {
                let chapter_chunks = split_text_chunks_pro(chapter_text, max_chunk_chars);
                if chapter_chunks.is_empty() {
                    completed_steps += 1;
                    continue;
                }
                info!(
                    chapter_index = index + 1,
                    chapter_title = chapter_title.as_str(),
                    chapter_chars = chapter_text.chars().count(),
                    "tts_book_convert chapter synth start"
                );
                let (chapter_wav, _, _) = synthesize_wav_from_chunks(
                    &chapter_chunks,
                    &tts_exe,
                    &tts_model,
                    &tts_tokens,
                    &tts_mode_hint,
                    &speaker_id,
                    speed,
                    &execution_provider,
                )
                .await?;
                fs::write(&chapter_path, chapter_wav)
                    .map_err(|e| format!("写入章节音频失败: {}", e))?;
                info!(
                    chapter_index = index + 1,
                    chapter_path = chapter_path.to_string_lossy().as_ref(),
                    cost_ms = chapter_start.elapsed().as_millis() as u64,
                    "tts_book_convert chapter synth done"
                );
            } else {
                resumed_any = true;
            }
            let chapter_path_str = chapter_path.to_string_lossy().to_string();
            chapter_output_paths.push(chapter_path_str.clone());
            if !resume_state.completed_chapters.contains(&(index + 1)) {
                resume_state.completed_chapters.push(index + 1);
                resume_state.completed_chapters.sort_unstable();
            }
            if !resume_state.chapter_output_paths.contains(&chapter_path_str) {
                resume_state.chapter_output_paths.push(chapter_path_str);
            }
            save_resume_state(&resume_state_path, &resume_state)?;
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
            save_resume_state(&resume_state_path, &resume_state)?;
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

    emit_tts_book_progress(
        &window,
        make_progress(
            &task_id,
            &file_path,
            &file_name,
            "completed",
            total_steps,
            total_steps,
            if resumed_any {
                "转换完成，已从中断位置继续".to_string()
            } else {
                "转换完成".to_string()
            },
            resumed_any,
            false,
            &output_path,
            &segment_json_path,
        ),
    )?;
    resume_state.status = TTS_BOOK_STATUS_SUCCESS.to_string();
    resume_state.last_stage = "completed".to_string();
    resume_state.last_message = if resumed_any {
        "转换完成，已从中断位置继续".to_string()
    } else {
        "转换完成".to_string()
    };
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

fn normalize_execution_provider(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "cuda" | "gpu" => "cuda".to_string(),
        "coreml" => "coreml".to_string(),
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

async fn synthesize_wav_from_chunks(
    chunks: &[TextChunk],
    tts_exe: &str,
    tts_model: &str,
    tts_tokens: &str,
    tts_mode_hint: &str,
    speaker_id: &str,
    speed: Option<f32>,
    execution_provider: &str,
) -> Result<(Vec<u8>, usize, usize), String> {
    let mut wav_parts: Vec<Vec<u8>> = Vec::with_capacity(chunks.len());
    let mut zh_chunks = 0usize;
    let mut en_chunks = 0usize;

    for (idx, chunk) in chunks.iter().enumerate() {
        let chunk_start = Instant::now();
        match chunk.lang {
            ChunkLang::Zh => zh_chunks += 1,
            ChunkLang::En => en_chunks += 1,
        }
        let prepared_text = prepare_chunk_text_for_tts(&chunk.text, chunk.lang);

        info!(
            index = idx + 1,
            total = chunks.len(),
            lang = match chunk.lang { ChunkLang::Zh => "zh", ChunkLang::En => "en" },
            chars = prepared_text.chars().count(),
            "tts_book_convert: synth chunk"
        );

        let rsp = audio_text_to_speech(TextToSpeechRequest {
            common: AudioCommonRequest {
                provider: SHERPA_PROVIDER.to_string(),
                runtime_provider: execution_provider.to_string(),
                model: tts_model.to_string(),
                api_key: String::new(),
                endpoint: tts_exe.to_string(),
                tokens: tts_tokens.to_string(),
                mode_hint: tts_mode_hint.to_string(),
            },
            input: prepared_text,
            voice: speaker_id.to_string(),
            response_format: "wav".to_string(),
            sample_rate: None,
            stream: Some(false),
            speed,
            gain: None,
            max_tokens: None,
        })
        .await?;

        let bytes = general_purpose::STANDARD
            .decode(rsp.audio_base64.as_bytes())
            .map_err(|e| format!("Base64 音频解码失败: {}", e))?;
        info!(
            index = idx + 1,
            total = chunks.len(),
            audio_bytes = bytes.len(),
            cost_ms = chunk_start.elapsed().as_millis() as u64,
            "tts_book_convert: synth chunk done"
        );
        wav_parts.push(bytes);
    }

    let merged_wav = merge_wav_parts_with_postprocess(&wav_parts, chunks)?;
    Ok((merged_wav, zh_chunks, en_chunks))
}

async fn synthesize_chunk_audio(
    chunk: &TextChunk,
    tts_exe: &str,
    tts_model: &str,
    tts_tokens: &str,
    tts_mode_hint: &str,
    speaker_id: &str,
    speed: Option<f32>,
    execution_provider: &str,
) -> Result<Vec<u8>, String> {
    let chunk_start = Instant::now();
    let prepared_text = prepare_chunk_text_for_tts(&chunk.text, chunk.lang);

    info!(
        index = chunk.index,
        lang = match chunk.lang { ChunkLang::Zh => "zh", ChunkLang::En => "en" },
        chars = prepared_text.chars().count(),
        "tts_book_convert: synth one chunk"
    );

    let rsp = audio_text_to_speech(TextToSpeechRequest {
        common: AudioCommonRequest {
            provider: SHERPA_PROVIDER.to_string(),
            runtime_provider: execution_provider.to_string(),
            model: tts_model.to_string(),
            api_key: String::new(),
            endpoint: tts_exe.to_string(),
            tokens: tts_tokens.to_string(),
            mode_hint: tts_mode_hint.to_string(),
        },
        input: prepared_text,
        voice: speaker_id.to_string(),
        response_format: "wav".to_string(),
        sample_rate: None,
        stream: Some(false),
        speed,
        gain: None,
        max_tokens: None,
    })
    .await?;

    let bytes = general_purpose::STANDARD
        .decode(rsp.audio_base64.as_bytes())
        .map_err(|e| format!("Base64 音频解码失败: {}", e))?;
    info!(
        index = chunk.index,
        audio_bytes = bytes.len(),
        cost_ms = chunk_start.elapsed().as_millis() as u64,
        "tts_book_convert: synth one chunk done"
    );
    Ok(bytes)
}

#[allow(dead_code)]
fn merge_wav_files(paths: &[PathBuf]) -> Result<Vec<u8>, String> {
    let mut parts = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(path).map_err(|e| format!("读取音频分片失败: {}", e))?;
        parts.push(bytes);
    }
    merge_wav_parts(&parts)
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

fn save_resume_state(path: &Path, state: &TtsBookResumeState) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(state).map_err(|e| format!("序列化续转状态失败: {}", e))?;
    fs::write(path, raw).map_err(|e| format!("写入续转状态失败: {}", e))
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

fn normalize_zh_tts_text(text: &str) -> String {
    let fraction_re = Regex::new(r"(\d+)\s*/\s*(\d+)").expect("valid regex");
    fraction_re
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

fn derive_pause_after_ms(text: &str) -> u32 {
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
        "doc" => Err("暂不支持 .doc，请先另存为 .docx 后再转换".to_string()),
        _ => Err("仅支持 txt/pdf/markdown/md/docx/epub 文件".to_string()),
    }
}

fn read_text_with_fallback(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(normalize_line_endings(&s)),
        Err(_) => {
            let bytes = fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;
            let (decoded, _, _) = GBK.decode(&bytes);
            Ok(normalize_line_endings(&decoded))
        }
    }
}

async fn extract_pdf_text(path: &Path) -> Result<String, String> {
    let raw = pdf_extract::extract_text(path).map_err(|e| format!("PDF 解析失败: {}", e))?;
    let text = normalize_pdf_text(&raw);
    if text.trim().is_empty() {
        return Err("PDF 解析结果为空".to_string());
    }
    Ok(text)
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

    let toc = extract_epub_toc_from_nav(&mut zip, &opf_content, &opf_dir);
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

fn resolve_epub_spine_files(opf_content: &str, opf_dir: &str) -> Vec<String> {
    let item_re = Regex::new(r#"<item\s+[^>]*id=\"([^\"]+)\"[^>]*href=\"([^\"]+)\"[^>]*/?>"#)
        .expect("valid regex");
    let spine_re = Regex::new(r#"<itemref\s+[^>]*idref=\"([^\"]+)\"[^>]*/?>"#).expect("valid regex");

    let mut manifest: HashMap<String, String> = HashMap::new();
    for cap in item_re.captures_iter(opf_content) {
        let id = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        let href = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
        if !id.is_empty() && !href.is_empty() {
            manifest.insert(id.to_string(), join_zip_path(opf_dir, href));
        }
    }

    let mut out = Vec::new();
    for cap in spine_re.captures_iter(opf_content) {
        let idref = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        if let Some(path) = manifest.get(idref) {
            out.push(path.clone());
        }
    }
    out
}

fn extract_epub_toc_from_nav(
    zip: &mut ZipArchive<fs::File>,
    opf_content: &str,
    opf_dir: &str,
) -> Vec<String> {
    let nav_item_re = Regex::new(
        r#"<item\s+[^>]*properties=\"[^\"]*nav[^\"]*\"[^>]*href=\"([^\"]+)\"[^>]*/?>"#,
    )
    .expect("valid regex");
    let Some(nav_path) = nav_item_re
        .captures(opf_content)
        .and_then(|c| c.get(1))
        .map(|m| join_zip_path(opf_dir, m.as_str()))
    else {
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

fn join_zip_path(base_dir: &str, rel: &str) -> String {
    let rel = rel.trim_start_matches('/');
    if base_dir.is_empty() {
        rel.to_string()
    } else {
        format!("{}/{}", base_dir.trim_end_matches('/'), rel)
    }
}

fn html_to_text(html: &str) -> String {
    let no_script = Regex::new(r"(?is)<(script|style|svg|noscript)[^>]*>.*?</(script|style|svg|noscript)>")
        .expect("valid regex")
        .replace_all(html, " ");
    let with_breaks = Regex::new(r"(?i)</?(p|div|section|article|li|ul|ol|blockquote|h[1-6]|tr|table|br)[^>]*>")
        .expect("valid regex")
        .replace_all(&no_script, "\n");
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

fn looks_like_heading(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.chars().count() > 80 {
        return false;
    }
    Regex::new(r"(?i)^(第[一二三四五六七八九十百千万0-9]+[章节回卷篇].*|chapter\s+[0-9ivxlcdm]+.*|[0-9]+(?:\.[0-9]+){0,3}\s+.+)$")
        .expect("valid regex")
        .is_match(t)
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

fn build_wav(fmt_chunk_data: &[u8], data_chunk_data: &[u8]) -> Result<Vec<u8>, String> {
    let fmt_size = fmt_chunk_data.len() as u32;
    let data_size = data_chunk_data.len() as u32;

    let riff_size = 4u32
        .saturating_add(8u32.saturating_add(fmt_size))
        .saturating_add(8u32.saturating_add(data_size));

    let mut out = Vec::with_capacity((riff_size as usize).saturating_add(8));
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

fn split_into_sentences_pro(text: &str) -> Vec<String> {
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

fn split_oversized_sentence_pro(sentence: &str, base_limit: usize) -> Vec<String> {
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

fn merge_wav_files_with_postprocess(paths: &[PathBuf], chunks: &[TextChunk]) -> Result<Vec<u8>, String> {
    let mut parts = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(path).map_err(|e| format!("读取音频分片失败: {}", e))?;
        parts.push(bytes);
    }
    merge_wav_parts_with_postprocess(&parts, chunks)
}

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
    let mut merged = base[..split_at].to_vec();

    for i in 0..overlap_samples {
        let fade_out = 1.0 - (i as f32 / overlap_samples as f32);
        let fade_in = i as f32 / overlap_samples as f32;
        merged.push(base[split_at + i] * fade_out + next[i] * fade_in);
    }
    merged.extend_from_slice(&next[overlap_samples..]);
    *base = merged;
}
