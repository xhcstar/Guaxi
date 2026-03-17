use crate::commands::chatview::{audio_text_to_speech, AudioCommonRequest, TextToSpeechRequest};
use crate::commands::settingview;
use base64::{engine::general_purpose, Engine as _};
use encoding_rs::GBK;
use pdfium_render::prelude::*;
use regex::Regex;
use scraper::Html;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command as TokioCommand;
use tokio::time::{timeout, Duration};
use tracing::{info, warn};
use walkdir::WalkDir;
use zip::ZipArchive;

const PDFIUM_LIB_PATH: &str = if cfg!(debug_assertions) {
    "../../libs"
} else {
    "./libs"
};

const SHERPA_PROVIDER: &str = "sherpa-onnx";
const EN_FALLBACK_MODEL: &str = "utils/Models/tts/kokoro_multi_lang/model.onnx";
const EN_FALLBACK_TOKENS: &str = "utils/Models/tts/kokoro_multi_lang/tokens.txt";
const ZH_MODEL_DIR: &str = "utils/Models/tts/vits-melo-tts-zh-en";
const ZH_TOKENS: &str = "utils/Models/tts/vits-melo-tts-zh-en/tokens.txt";

#[derive(Debug, Deserialize)]
pub struct TtsBookConvertRequest {
    pub file_path: String,
    pub output_dir: String,
    #[serde(default)]
    pub output_name: String,
    #[serde(default)]
    pub voice_zh: String,
    #[serde(default)]
    pub voice_en: String,
    #[serde(default)]
    pub split_by_toc: bool,
    pub max_chunk_chars: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct TtsBookConvertResponse {
    pub output_path: String,
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
    text: String,
    lang: ChunkLang,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkLang {
    Zh,
    En,
}

#[tauri::command]
pub async fn tts_book_convert(request: TtsBookConvertRequest) -> Result<TtsBookConvertResponse, String> {
    let total_start = Instant::now();
    let file_path = PathBuf::from(request.file_path.trim());
    info!(
        file_path = file_path.to_string_lossy().as_ref(),
        output_dir = request.output_dir.as_str(),
        split_by_toc = request.split_by_toc,
        max_chunk_chars = request.max_chunk_chars.unwrap_or(180),
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

    let extracted = extract_document(&file_path).await?;
    let cleaned = clean_text_for_tts(&extracted.text);
    if cleaned.trim().is_empty() {
        return Err("未提取到可朗读文本".to_string());
    }

    let max_chunk_chars = request.max_chunk_chars.unwrap_or(180).clamp(80, 420);
    let chunks = split_text_chunks(&cleaned, max_chunk_chars);
    info!(
        cleaned_chars = cleaned.chars().count(),
        chunk_count = chunks.len(),
        max_chunk_chars,
        "tts_book_convert chunked"
    );
    if chunks.is_empty() {
        return Err("文本分段失败".to_string());
    }

    let tts_cfg = settingview::get_tts_config();
    let tts_exe = if tts_cfg.exe.trim().is_empty() {
        String::new()
    } else {
        tts_cfg.exe.trim().to_string()
    };

    let english_model = {
        let m = tts_cfg.model.trim();
        if m.is_empty() || m.contains("vits-melo-tts-zh-en") {
            EN_FALLBACK_MODEL.to_string()
        } else {
            m.to_string()
        }
    };
    let english_tokens = {
        let t = tts_cfg.tokens.trim();
        if t.is_empty() || t.contains("vits-melo-tts-zh-en") {
            EN_FALLBACK_TOKENS.to_string()
        } else {
            t.to_string()
        }
    };

    let voice_zh = if request.voice_zh.trim().is_empty() {
        "0".to_string()
    } else {
        request.voice_zh.trim().to_string()
    };
    let voice_en = if request.voice_en.trim().is_empty() {
        "0".to_string()
    } else {
        request.voice_en.trim().to_string()
    };

    let synth_start = Instant::now();
    let (merged_wav, zh_chunks, en_chunks) = synthesize_wav_from_chunks(
        &chunks,
        &tts_exe,
        &english_model,
        &english_tokens,
        &voice_zh,
        &voice_en,
    )
    .await?;
    info!(
        total_chunks = chunks.len(),
        zh_chunks,
        en_chunks,
        cost_ms = synth_start.elapsed().as_millis() as u64,
        "tts_book_convert full audio synthesized"
    );

    let stem = sanitize_file_stem(if request.output_name.trim().is_empty() {
        extracted.title.as_str()
    } else {
        request.output_name.trim()
    });
    let output_path = output_dir.join(format!("{}_audiobook.wav", stem));
    fs::write(&output_path, merged_wav).map_err(|e| format!("写入音频文件失败: {}", e))?;

    let mut chapter_output_paths = Vec::new();
    if request.split_by_toc {
        let chapter_dir = output_dir.join(format!("{}_chapters", stem));
        fs::create_dir_all(&chapter_dir).map_err(|e| format!("创建章节目录失败: {}", e))?;

        let chapters = split_text_into_chapters(&cleaned, &extracted.toc);
        info!(chapter_count = chapters.len(), "tts_book_convert chapter split done");
        for (index, (chapter_title, chapter_text)) in chapters.iter().enumerate() {
            let chapter_start = Instant::now();
            info!(
                chapter_index = index + 1,
                chapter_title = chapter_title.as_str(),
                chapter_chars = chapter_text.chars().count(),
                "tts_book_convert chapter synth start"
            );
            let chapter_chunks = split_text_chunks(chapter_text, max_chunk_chars);
            if chapter_chunks.is_empty() {
                continue;
            }
            let (chapter_wav, _, _) = synthesize_wav_from_chunks(
                &chapter_chunks,
                &tts_exe,
                &english_model,
                &english_tokens,
                &voice_zh,
                &voice_en,
            )
            .await?;

            let chapter_stem = sanitize_file_stem(&truncate_title(chapter_title, 36));
            let chapter_name = format!("{:03}_{}.wav", index + 1, chapter_stem);
            let chapter_path = chapter_dir.join(chapter_name);
            fs::write(&chapter_path, chapter_wav)
                .map_err(|e| format!("写入章节音频失败: {}", e))?;
            chapter_output_paths.push(chapter_path.to_string_lossy().to_string());
            info!(
                chapter_index = index + 1,
                chapter_path = chapter_path.to_string_lossy().as_ref(),
                cost_ms = chapter_start.elapsed().as_millis() as u64,
                "tts_book_convert chapter synth done"
            );
        }
    }

    info!(
        file_path = file_path.to_string_lossy().as_ref(),
        output_path = output_path.to_string_lossy().as_ref(),
        chapter_count = chapter_output_paths.len(),
        total_cost_ms = total_start.elapsed().as_millis() as u64,
        "tts_book_convert finished"
    );

    Ok(TtsBookConvertResponse {
        output_path: output_path.to_string_lossy().to_string(),
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

async fn synthesize_wav_from_chunks(
    chunks: &[TextChunk],
    tts_exe: &str,
    english_model: &str,
    english_tokens: &str,
    voice_zh: &str,
    voice_en: &str,
) -> Result<(Vec<u8>, usize, usize), String> {
    let mut wav_parts: Vec<Vec<u8>> = Vec::with_capacity(chunks.len());
    let mut zh_chunks = 0usize;
    let mut en_chunks = 0usize;

    for (idx, chunk) in chunks.iter().enumerate() {
        let chunk_start = Instant::now();
        let (model, tokens, voice) = match chunk.lang {
            ChunkLang::Zh => {
                zh_chunks += 1;
                (ZH_MODEL_DIR.to_string(), ZH_TOKENS.to_string(), voice_zh.to_string())
            }
            ChunkLang::En => {
                en_chunks += 1;
                (
                    english_model.to_string(),
                    english_tokens.to_string(),
                    voice_en.to_string(),
                )
            }
        };

        info!(
            index = idx + 1,
            total = chunks.len(),
            lang = match chunk.lang { ChunkLang::Zh => "zh", ChunkLang::En => "en" },
            chars = chunk.text.chars().count(),
            "tts_book_convert: synth chunk"
        );

        let rsp = audio_text_to_speech(TextToSpeechRequest {
            common: AudioCommonRequest {
                provider: SHERPA_PROVIDER.to_string(),
                model,
                api_key: String::new(),
                endpoint: tts_exe.to_string(),
                tokens,
            },
            input: chunk.text.clone(),
            voice,
            response_format: "wav".to_string(),
            sample_rate: None,
            stream: Some(false),
            speed: None,
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

    let merged_wav = merge_wav_parts(&wav_parts)?;
    Ok((merged_wav, zh_chunks, en_chunks))
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
    let chapter_re = Regex::new(
        r"(?i)^(第[一二三四五六七八九十百千万0-9]+[章节回卷篇].*|chapter\s+[0-9ivxlcdm]+.*|[0-9]+(?:\.[0-9]+){0,3}\s+.+)$",
    )
    .expect("valid regex");

    let mut marks = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.chars().count() > 100 {
            continue;
        }
        if chapter_re.is_match(t) {
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
        Ok(s) => Ok(s),
        Err(_) => {
            let bytes = fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;
            let (decoded, _, _) = GBK.decode(&bytes);
            Ok(decoded.into_owned())
        }
    }
}

fn init_pdfium() -> Result<Pdfium, String> {
    let mut tried: Vec<String> = Vec::new();

    match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(PDFIUM_LIB_PATH)) {
        Ok(bindings) => return Ok(Pdfium::new(bindings)),
        Err(e) => tried.push(format!("{} => {}", PDFIUM_LIB_PATH, e)),
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let candidates = vec![
                exe_dir.join("libs"),
                exe_dir.join("resources").join("libs"),
                exe_dir.join("..").join("libs"),
                exe_dir.join("..").join("..").join("libs"),
            ];

            for candidate in candidates {
                let path = candidate.to_string_lossy().to_string();
                match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&candidate)) {
                    Ok(bindings) => return Ok(Pdfium::new(bindings)),
                    Err(e) => tried.push(format!("{} => {}", path, e)),
                }
            }
        }
    }

    if cfg!(debug_assertions) {
        let workspace_libs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("libs");
        let path = workspace_libs.to_string_lossy().to_string();
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&workspace_libs)) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(e) => tried.push(format!("{} => {}", path, e)),
        }
    }

    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(e) => {
            tried.push(format!("system => {}", e));
            Err(format!("PDFium 初始化失败，已尝试路径: {}", tried.join(" | ")))
        }
    }
}

fn resolve_marker_source_dir_for_tts(exe_dir: &Path) -> Result<PathBuf, String> {
    let p = exe_dir
        .join("utils")
        .join("MarkdownConvertor")
        .join("marker-1.10.1");
    if p.exists() {
        return Ok(p);
    }

    if cfg!(debug_assertions) {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("marker-1.10.1");
        if p.exists() {
            return Ok(p);
        }
    }

    Err("找不到 marker-1.10.1 源码目录".to_string())
}

fn resolve_marker_python_path_for_tts(marker_src_dir: &Path) -> Result<PathBuf, String> {
    let venv_dir = marker_src_dir.join(".venv");
    if cfg!(windows) {
        let candidates = [
            venv_dir.join("Scripts").join("python.exe"),
            venv_dir.join("python.exe"),
        ];
        for p in candidates {
            if p.exists() {
                return Ok(p);
            }
        }
    } else {
        let p = venv_dir.join("bin").join("python");
        if p.exists() {
            return Ok(p);
        }
    }
    Err("找不到 Marker 虚拟环境 Python，请先准备 marker 运行环境".to_string())
}

async fn extract_pdf_text_via_marker(path: &Path) -> Result<String, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("获取可执行路径失败: {}", e))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "获取可执行目录失败".to_string())?;

    let marker_src_dir = resolve_marker_source_dir_for_tts(exe_dir)?;
    let python_path = resolve_marker_python_path_for_tts(&marker_src_dir)?;

    let script_path = marker_src_dir.join("convert_single.py");
    if !script_path.exists() {
        return Err(format!("找不到 convert_single.py: {}", script_path.to_string_lossy()));
    }

    let model_dir = exe_dir.join("utils").join("Models").join("surya_models");
    let _ = fs::create_dir_all(&model_dir);

    let temp_dir = tempfile::tempdir().map_err(|e| format!("创建临时目录失败: {}", e))?;
    let input_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    let expected_output_dir = temp_dir.path().join(input_stem);
    fs::create_dir_all(&expected_output_dir).map_err(|e| format!("创建临时输出目录失败: {}", e))?;

    let existing_pythonpath = std::env::var("PYTHONPATH").unwrap_or_default();
    let marker_pythonpath = marker_src_dir.to_string_lossy().to_string();
    let path_sep = if cfg!(windows) { ";" } else { ":" };
    let combined_pythonpath = if existing_pythonpath.trim().is_empty() {
        marker_pythonpath
    } else {
        format!("{}{}{}", marker_pythonpath, path_sep, existing_pythonpath)
    };

    info!(
        pdf_path = path.to_string_lossy().as_ref(),
        python = python_path.to_string_lossy().as_ref(),
        marker_src_dir = marker_src_dir.to_string_lossy().as_ref(),
        output_dir = expected_output_dir.to_string_lossy().as_ref(),
        "tts_book_convert marker fallback start"
    );

    let marker_start = Instant::now();
    let mut cmd = TokioCommand::new(&python_path);
    cmd.arg("-u")
        .arg(script_path.to_string_lossy().to_string())
        .arg(path.to_string_lossy().to_string())
        .arg("--output_format")
        .arg("markdown")
        .arg("--paginate_output")
        .arg("--output_dir")
        .arg(expected_output_dir.to_string_lossy().to_string())
        .arg("--progress_json")
        .current_dir(&marker_src_dir)
        .env("PYTHONPATH", combined_pythonpath)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("HF_HOME", model_dir.to_string_lossy().to_string())
        .env("HUGGINGFACE_HUB_CACHE", model_dir.to_string_lossy().to_string())
        .env("TRANSFORMERS_CACHE", model_dir.to_string_lossy().to_string())
        .env("TORCH_HOME", model_dir.to_string_lossy().to_string())
        .env("XDG_CACHE_HOME", model_dir.to_string_lossy().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = timeout(Duration::from_secs(900), cmd.output())
        .await
        .map_err(|_| "Marker 转换超时（15分钟），请先在文档工具中单独转换 PDF 预热模型后再试".to_string())?
        .map_err(|e| format!("启动 Marker 转换失败: {}", e))?;

    info!(
        status = output.status.code().unwrap_or(-1),
        stdout_len = output.stdout.len(),
        stderr_len = output.stderr.len(),
        cost_ms = marker_start.elapsed().as_millis() as u64,
        "tts_book_convert marker fallback done"
    );

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("Marker 转换失败: {}", stderr));
    }

    let expected_md = expected_output_dir.join(format!("{}.md", input_stem));
    let md_path = if expected_md.exists() {
        expected_md
    } else {
        WalkDir::new(&expected_output_dir)
            .into_iter()
            .filter_map(Result::ok)
            .find(|e| e.file_type().is_file() && e.path().extension().and_then(|x| x.to_str()) == Some("md"))
            .map(|e| e.into_path())
            .ok_or_else(|| "Marker 转换成功但未找到 Markdown 输出".to_string())?
    };

    let md = fs::read_to_string(&md_path).map_err(|e| format!("读取 Marker 输出失败: {}", e))?;
    let text = markdown_to_plain_text(&md);
    info!(
        md_path = md_path.to_string_lossy().as_ref(),
        markdown_chars = md.chars().count(),
        text_chars = text.chars().count(),
        "tts_book_convert marker text extracted"
    );
    if text.trim().is_empty() {
        return Err("Marker 输出文本为空".to_string());
    }
    Ok(text)
}

fn extract_pdf_text_via_pdfium(path: &Path) -> Result<String, String> {
    let pdfium = init_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| format!("PDF 打开失败: {}", e))?;

    let mut pages: Vec<Vec<String>> = Vec::new();
    for idx in 0..document.pages().len() {
        let page = document
            .pages()
            .get(idx as u16)
            .map_err(|e| format!("PDF 页面读取失败: {}", e))?;
        let text = page
            .text()
            .map(|t| t.all())
            .unwrap_or_default();
        let lines = text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>();
        pages.push(lines);
    }

    let cleaned_pages = strip_pdf_page_margins(pages);
    Ok(cleaned_pages.join("\n\n"))
}

async fn extract_pdf_text(path: &Path) -> Result<String, String> {
    match extract_pdf_text_via_pdfium(path) {
        Ok(text) => Ok(text),
        Err(pdfium_err) => {
            warn!(err = pdfium_err.as_str(), "pdfium parse failed, fallback to marker converter");
            extract_pdf_text_via_marker(path).await.map_err(|marker_err| {
                format!(
                    "PDFium 解析失败，且 Marker 回退也失败。PDFium: {} ; Marker: {}",
                    pdfium_err, marker_err
                )
            })
        }
    }
}

fn strip_pdf_page_margins(mut pages: Vec<Vec<String>>) -> Vec<String> {
    if pages.is_empty() {
        return vec![];
    }

    let mut first_line_count: HashMap<String, usize> = HashMap::new();
    let mut last_line_count: HashMap<String, usize> = HashMap::new();

    for lines in &pages {
        if let Some(first) = lines.first() {
            *first_line_count.entry(first.clone()).or_insert(0) += 1;
        }
        if let Some(last) = lines.last() {
            *last_line_count.entry(last.clone()).or_insert(0) += 1;
        }
    }

    let threshold = (pages.len() / 3).max(3);
    let common_header = first_line_count
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .and_then(|(k, c)| if c >= threshold { Some(k) } else { None });
    let common_footer = last_line_count
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .and_then(|(k, c)| if c >= threshold { Some(k) } else { None });

    let mut out = Vec::with_capacity(pages.len());
    for lines in &mut pages {
        if let Some(h) = &common_header {
            if lines.first() == Some(h) {
                lines.remove(0);
            }
        }
        if let Some(f) = &common_footer {
            if lines.last() == Some(f) {
                lines.pop();
            }
        }
        out.push(lines.join("\n"));
    }

    out
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
    s = s.replace("<w:tab/>", " ");

    let tag_re = Regex::new(r"<[^>]+>").expect("valid regex");
    s = tag_re.replace_all(&s, "").to_string();

    decode_xml_entities(&s)
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
    let no_script = Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>")
        .expect("valid regex")
        .replace_all(html, " ");
    let doc = Html::parse_document(&no_script);
    let text = doc.root_element().text().collect::<Vec<_>>().join(" ");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    s = s.replace('#', " ");

    s
}

fn clean_text_for_tts(raw: &str) -> String {
    let mut s = raw.replace("\r\n", "\n").replace('\r', "\n");

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
    s = decode_xml_entities(&s);

    let mut lines: Vec<String> = Vec::new();
    for line in s.lines() {
        let t = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.is_empty() {
            continue;
        }
        if t.starts_with("图 ") || t.starts_with("图") || t.starts_with("Figure ") {
            continue;
        }
        lines.push(t);
    }

    lines.join("\n")
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

fn extract_outline_lines(text: &str) -> Vec<String> {
    let chapter_re = Regex::new(
        r"(?i)^(第[一二三四五六七八九十百千万0-9]+[章节回卷篇].*|chapter\s+[0-9ivxlcdm]+.*|[0-9]+(?:\.[0-9]+){0,3}\s+.+)$",
    )
    .expect("valid regex");

    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.chars().count() > 80 {
            continue;
        }
        if chapter_re.is_match(t) {
            out.push(t.to_string());
        }
        if out.len() >= 80 {
            break;
        }
    }
    out
}

fn split_text_chunks(text: &str, base_limit: usize) -> Vec<TextChunk> {
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();

    let push_current = |buf: &mut String, out: &mut Vec<String>| {
        let t = buf.trim();
        if !t.is_empty() {
            out.push(t.to_string());
        }
        buf.clear();
    };

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | ';' | '；' | '\n') {
            push_current(&mut current, &mut sentences);
        }
    }
    push_current(&mut current, &mut sentences);

    if sentences.is_empty() {
        return vec![];
    }

    let zh_limit = base_limit;
    let en_limit = (base_limit.saturating_mul(2)).clamp(120, 640);

    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut chunk_buf = String::new();
    let mut chunk_lang: Option<ChunkLang> = None;

    for sentence in &sentences {
        let lang = detect_chunk_lang(sentence);
        let limit = if lang == ChunkLang::Zh { zh_limit } else { en_limit };

        let should_flush = match chunk_lang {
            Some(l) if l != lang => true,
            _ => chunk_buf.chars().count() + sentence.chars().count() > limit,
        };

        if should_flush && !chunk_buf.trim().is_empty() {
            chunks.push(TextChunk {
                text: chunk_buf.trim().to_string(),
                lang: chunk_lang.unwrap_or(lang),
            });
            chunk_buf.clear();
        }

        if !chunk_buf.is_empty() {
            chunk_buf.push(' ');
        }
        chunk_buf.push_str(sentence.trim());
        chunk_lang = Some(lang);
    }

    if !chunk_buf.trim().is_empty() {
        chunks.push(TextChunk {
            text: chunk_buf.trim().to_string(),
            lang: chunk_lang.unwrap_or(ChunkLang::Zh),
        });
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
