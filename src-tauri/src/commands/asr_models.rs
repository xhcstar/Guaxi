//! 语言学习的多语种 ASR 模型管理。
//!
//! 模型放在 `repos/guaxi_language_learning/models/asr/<id>/`，与安装目录里的
//! `utils/Models/whisper_onnx` 分开：下载来的模型不污染随包资源，卸载时也只删用户目录。
//!
//! 路由规则：
//! - 英语：优先内置 `*.en` Whisper，其次用户安装的多语种 Whisper
//! - 中/日/韩/粤：优先 SenseVoice，其次多语种 Whisper
//! - 法/德/西/俄：多语种 Whisper
//!
//! 下载的是 sherpa-onnx 官方 tar.bz2；解压后把含 tokens.txt 的目录拍平到 `<id>/`。

use bzip2::read::BzDecoder;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;

const LANG_LEARN_DIR: &str = "guaxi_language_learning";
const MODEL_DIR: &str = "models/asr";
const JOBS: Lazy<Mutex<HashMap<String, AsrJob>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AsrJob {
    pub id: String,
    pub stage: String,
    pub progress: f64,
    pub message: String,
    pub error: String,
    pub started_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AsrModelInfo {
    pub id: String,
    pub name: String,
    pub languages: String,
    pub size_bytes: u64,
    pub builtin: bool,
    pub installed: bool,
    pub path: String,
    pub description: String,
}

struct AsrSpec {
    id: &'static str,
    name: &'static str,
    languages: &'static str,
    size_bytes: u64,
    description: &'static str,
    url: &'static str,
}

const CATALOG: &[AsrSpec] = &[
    AsrSpec {
        id: "whisper-tiny",
        name: "Whisper Tiny 多语种",
        languages: "99 种语言",
        size_bytes: 116_204_861,
        description: "体积最小，适合低带宽和核显；识别精度一般。",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2",
    },
    AsrSpec {
        id: "whisper-base",
        name: "Whisper Base 多语种",
        languages: "99 种语言",
        size_bytes: 207_557_382,
        description: "多语种默认推荐；比 tiny 稳，CPU 也能跑。",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
    },
    AsrSpec {
        id: "sense-voice",
        name: "SenseVoice 多语种",
        languages: "中 / 英 / 日 / 韩 / 粤",
        size_bytes: 1_047_870_769,
        description: "中文识别质量最好，体积约 1GB；只覆盖中英日韩粤。",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2",
    },
];

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn learning_root() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let root = exe
        .parent()
        .ok_or_else(|| "无法确定可执行文件目录".to_string())?
        .join("repos")
        .join(LANG_LEARN_DIR);
    fs::create_dir_all(&root).map_err(|e| format!("创建语言学习目录失败: {}", e))?;
    Ok(root)
}

fn models_root() -> Result<PathBuf, String> {
    let root = learning_root()?.join(MODEL_DIR);
    fs::create_dir_all(&root).map_err(|e| format!("创建语音模型目录失败: {}", e))?;
    Ok(root)
}

fn set_job(id: &str, stage: &str, progress: f64, message: &str, error: &str) {
    if id.trim().is_empty() {
        return;
    }
    let now = now_iso();
    if let Ok(mut jobs) = JOBS.lock() {
        let job = jobs.entry(id.to_string()).or_insert_with(|| AsrJob {
            id: id.to_string(),
            started_at: now.clone(),
            ..Default::default()
        });
        job.stage = stage.to_string();
        job.progress = progress.clamp(0.0, 100.0);
        job.message = message.to_string();
        job.error = error.to_string();
        job.updated_at = now;
    }
}

#[tauri::command]
pub fn language_learning_asr_jobs() -> Vec<AsrJob> {
    JOBS.lock()
        .map(|jobs| jobs.values().cloned().collect())
        .unwrap_or_default()
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if meta.is_dir() {
                total += dir_size(&entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

fn has_file_suffix(dir: &Path, suffix: &str) -> bool {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().ends_with(suffix))
        })
        .unwrap_or(false)
}

fn has_tokens(dir: &Path) -> bool {
    dir.join("tokens.txt").is_file() || has_file_suffix(dir, "-tokens.txt")
}

fn model_ready(dir: &Path) -> bool {
    if !has_tokens(dir) {
        return false;
    }
    dir.join("model.onnx").is_file()
        || dir.join("model.int8.onnx").is_file()
        || dir.join("encoder.onnx").is_file()
        || dir.join("encoder.int8.onnx").is_file()
        || has_file_suffix(dir, "-encoder.onnx")
        || has_file_suffix(dir, "-encoder.int8.onnx")
}

fn classify_dir_name(name: &str) -> (String, String, String) {
    let lower = name.to_lowercase();
    if lower.contains("sense") || lower.contains("sensevoice") {
        return (
            "中 / 英 / 日 / 韩 / 粤".to_string(),
            "SenseVoice 多语种模型".to_string(),
            "sense".to_string(),
        );
    }
    if lower.contains(".en") {
        return (
            "英语".to_string(),
            "英文专用 Whisper 模型".to_string(),
            "en".to_string(),
        );
    }
    (
        "多语种".to_string(),
        "多语种 Whisper 模型".to_string(),
        "multi".to_string(),
    )
}

fn bundled_whisper_dirs() -> Vec<PathBuf> {
    let base = match crate::guaxiservices::bundled_resources::ensure_relative_dir("utils/Models/whisper_onnx") {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir.is_dir() && model_ready(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

fn user_whisper_dirs() -> Vec<PathBuf> {
    let root = match models_root() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir.is_dir() && model_ready(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

fn all_model_dirs() -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    for dir in bundled_whisper_dirs() {
        out.push((dir, true));
    }
    for dir in user_whisper_dirs() {
        out.push((dir, false));
    }
    out
}

#[tauri::command]
pub fn language_learning_asr_models() -> Vec<AsrModelInfo> {
    let mut out = Vec::new();
    for (dir, builtin) in all_model_dirs() {
        let name = dir
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_else(|| "asr-model".to_string());
        let (languages, description, _) = classify_dir_name(&name);
        let id = if builtin {
            format!("bundled-{}", name)
        } else {
            name.clone()
        };
        out.push(AsrModelInfo {
            id,
            name,
            languages,
            size_bytes: dir_size(&dir),
            builtin,
            installed: true,
            path: dir.to_string_lossy().to_string(),
            description,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[tauri::command]
pub fn language_learning_asr_catalog() -> Vec<AsrModelInfo> {
    let installed_ids: Vec<String> = user_whisper_dirs()
        .into_iter()
        .filter_map(|dir| dir.file_name().map(|x| x.to_string_lossy().to_string()))
        .collect();
    CATALOG
        .iter()
        .map(|spec| {
            let installed = installed_ids.iter().any(|id| id == spec.id);
            AsrModelInfo {
                id: spec.id.to_string(),
                name: spec.name.to_string(),
                languages: spec.languages.to_string(),
                size_bytes: spec.size_bytes,
                builtin: false,
                installed,
                path: String::new(),
                description: spec.description.to_string(),
            }
        })
        .collect()
}

fn proxy_client() -> Result<Client, String> {
    let config = crate::commands::resource_hunter::store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|value| {
            serde_json::from_str::<crate::commands::resource_hunter::model::ProxyConfig>(&value).ok()
        })
        .unwrap_or_default();
    let proxy_url = crate::commands::resource_hunter::proxy::effective_proxy_url(&config);
    crate::commands::resource_hunter::proxy::build_client_raw(proxy_url.as_deref())
}

async fn download_archive(client: &Client, url: &str, dest: &Path, job_id: &str, size: u64) -> Result<(), String> {
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(1800))
        .send()
        .await
        .map_err(|e| format!("下载语音模型失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("下载语音模型失败，状态码: {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(size);
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("创建模型文件失败: {}", e))?;
    let mut stream = resp.bytes_stream();
    let mut written = 0u64;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取模型数据失败: {}", e))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| format!("写入模型文件失败: {}", e))?;
        written += bytes.len() as u64;
        let percent = if total > 0 {
            (written as f64 / total as f64 * 92.0).min(92.0)
        } else {
            0.0
        };
        set_job(job_id, "download", percent, &format!("下载中 {:.1} MB", written as f64 / 1_048_576.0), "");
    }
    file.flush().await.map_err(|e| format!("写入模型文件失败: {}", e))?;
    Ok(())
}

fn safe_extract_tar_bz2(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|e| format!("读取模型压缩包失败: {}", e))?;
    let decoder = BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|e| format!("解析模型压缩包失败: {}", e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("读取模型条目失败: {}", e))?;
        let path = entry.path().map_err(|e| format!("读取模型路径失败: {}", e))?.into_owned();
        if path.is_absolute() || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            continue;
        }
        entry
            .unpack_in(dest)
            .map_err(|e| format!("解压模型失败: {}", e))?;
    }
    Ok(())
}

fn find_model_dir(root: &Path) -> Option<PathBuf> {
    if model_ready(root) {
        return Some(root.to_path_buf());
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let dir = entry.path();
        if dir.is_dir() && model_ready(&dir) {
            return Some(dir);
        }
    }
    None
}

fn move_model_into(src: &Path, final_dir: &Path) -> Result<(), String> {
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(|e| format!("替换旧模型失败: {}", e))?;
    }
    fs::create_dir_all(final_dir).map_err(|e| format!("创建模型目录失败: {}", e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("读取解压目录失败: {}", e))?.flatten() {
        let from = entry.path();
        let to = final_dir.join(entry.file_name());
        fs::rename(&from, &to).map_err(|e| format!("安装模型失败: {}", e))?;
    }
    Ok(())
}

async fn install_model(spec: &'static AsrSpec, job_id: &str) -> Result<AsrModelInfo, String> {
    let root = models_root()?;
    let final_dir = root.join(spec.id);
    let temp_dir = root.join(format!(".tmp-{}", spec.id));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| format!("创建模型临时目录失败: {}", e))?;
    let archive = temp_dir.join("model.tar.bz2");
    set_job(job_id, "download", 0.0, "准备下载", "");
    let client = proxy_client()?;
    download_archive(&client, spec.url, &archive, job_id, spec.size_bytes).await?;
    set_job(job_id, "extract", 93.0, "解压模型中", "");
    let extract_dir = temp_dir.join("extract");
    fs::create_dir_all(&extract_dir).map_err(|e| format!("创建解压目录失败: {}", e))?;
    let archive_for_extract = archive.clone();
    let extract_for_extract = extract_dir.clone();
    tauri::async_runtime::spawn_blocking(move || safe_extract_tar_bz2(&archive_for_extract, &extract_for_extract))
        .await
        .map_err(|e| format!("模型解压任务异常: {}", e))??;
    let model_dir = find_model_dir(&extract_dir).ok_or_else(|| "压缩包里没有找到可用模型目录".to_string())?;
    set_job(job_id, "install", 97.0, "安装模型中", "");
    move_model_into(&model_dir, &final_dir)?;
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(AsrModelInfo {
        id: spec.id.to_string(),
        name: spec.name.to_string(),
        languages: spec.languages.to_string(),
        size_bytes: dir_size(&final_dir),
        builtin: false,
        installed: true,
        path: final_dir.to_string_lossy().to_string(),
        description: spec.description.to_string(),
    })
}

#[tauri::command]
pub fn language_learning_asr_install(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _ = app;
    let spec = CATALOG
        .iter()
        .find(|spec| spec.id == id.trim())
        .ok_or_else(|| "未知的语音模型".to_string())?;
    let job_id = spec.id.to_string();
    if let Ok(jobs) = JOBS.lock() {
        if let Some(job) = jobs.get(&job_id) {
            if !job.stage.is_empty() && job.stage != "done" && job.stage != "error" {
                return Ok(());
            }
        }
    }
    set_job(&job_id, "download", 0.0, "准备下载", "");
    tauri::async_runtime::spawn(async move {
        let spec = CATALOG.iter().find(|s| s.id == job_id.as_str());
        let Some(spec) = spec else {
            set_job(&job_id, "error", 0.0, "模型不存在", "未知的语音模型");
            return;
        };
        match install_model(spec, &job_id).await {
            Ok(info) => set_job(&job_id, "done", 100.0, &format!("已安装 {}", info.name), ""),
            Err(error) => set_job(&job_id, "error", 0.0, "安装失败", &error),
        }
    });
    Ok(())
}

#[tauri::command]
pub fn language_learning_asr_remove(id: String) -> Result<(), String> {
    let safe: String = id
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() || safe != id.trim() {
        return Err("模型 ID 不合法".to_string());
    }
    let dir = models_root()?.join(safe);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("删除模型失败: {}", e))?;
    }
    Ok(())
}

fn supports_lang(name: &str, lang: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.contains("sense") {
        return matches!(lang, "zh" | "en" | "ja" | "ko" | "yue");
    }
    if lower.contains(".en") {
        return lang == "en";
    }
    true
}

fn prefer_model(name: &str, lang: &str) -> i32 {
    let lower = name.to_lowercase();
    let mut score = 0;
    if lang == "en" && lower.contains(".en") {
        score += 40;
    }
    if lang != "en" && lower.contains(".en") {
        score -= 100;
    }
    if matches!(lang, "zh" | "ja" | "ko" | "yue") && lower.contains("sense") {
        score += 30;
    }
    if lower.contains("base") {
        score += 10;
    }
    if lower.contains("tiny") {
        score += 5;
    }
    score
}

/// 供语音转写使用：返回 (模型目录, 是否 SenseVoice)。
pub fn resolve_model_for_language(lang: &str) -> Result<(PathBuf, bool), String> {
    let target = if lang.trim().is_empty() { "en" } else { lang.trim() };
    let mut candidates: Vec<(PathBuf, bool)> = all_model_dirs()
        .into_iter()
        .filter(|(dir, _)| {
            let name = dir.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            supports_lang(&name, target)
        })
        .collect();
    if candidates.is_empty() {
        return Err(format!(
            "学习语言「{}」还没有可用的语音识别模型。请在「语音模型」里下载 Whisper 多语种{}模型",
            target,
            if matches!(target, "zh" | "ja" | "ko") { " 或 SenseVoice " } else { " " }
        ));
    }
    candidates.sort_by_key(|(dir, _)| {
        let name = dir.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        -(prefer_model(&name, target))
    });
    let (dir, _) = candidates.remove(0);
    let name = dir.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    Ok((dir, name.to_lowercase().contains("sense")))
}

pub fn build_engine_config(
    lang: &str,
    provider: &str,
) -> Result<(crate::guaxiservices::speech::AsrEngineConfig, String), String> {
    let (dir, is_sense) = resolve_model_for_language(lang)?;
    let config = if is_sense {
        crate::guaxiservices::speech::resolve_asr_engine_config(
            dir.to_string_lossy().as_ref(),
            provider,
        )?
    } else {
        crate::guaxiservices::speech::resolve_whisper_onnx_engine_config(
            &dir,
            provider,
            Some(lang),
        )?
    };
    Ok((config, dir.to_string_lossy().to_string()))
}

pub fn vad_model() -> Result<PathBuf, String> {
    let vad = crate::guaxiservices::bundled_resources::ensure_relative_dir("utils/Models/vad/silero_vad.onnx")?;
    if !vad.is_file() {
        return Err(format!("未找到 VAD 模型：{}", vad.display()));
    }
    Ok(vad)
}
