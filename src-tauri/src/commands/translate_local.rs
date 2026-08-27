//! 本地翻译小模型服务（复用已内置的 llama.cpp）。
//!
//! 定位：**只做翻译**（单词 / 句子 / 段落 / 文档全文），与大模型助手里的 AI 翻译完全independent，
//! 不共用会话、不共用模型、不走同一个端口。它是免费翻译接口池的**兜底**：
//! 只有当池子里所有免费接口都失效 / 被风控封禁 / 额度用完时才启用。
//!
//! 设计取舍（针对「无独显、性能较弱的 PC 也要快」）：
//! - 默认 Qwen3-0.6B Q4_K_M（约 400MB），弱 CPU 上约 20~40 tok/s，段落级翻译 1~3 秒；
//!   注册表里同时给出 1.7B 与 TranslateGemma-4B，用户可在设置里换成质量更好的。
//! - 单独一个 llama-server 实例（端口 18082），`--parallel 1`，避免与 embedding / 聊天服务抢内存。
//! - Qwen3 默认会输出 <think> 推理块，这里显式关掉思考模式并在解析时再兜底剥一层。
//!
//! 运行时验证点：utils/llama/llama-server 可执行 + utils/Models/translate/ 下的 GGUF。

use crate::guaxiservices::bundled_resources;
use once_cell::sync::Lazy;
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

const LOCAL_HOST: &str = "127.0.0.1";
const LOCAL_PORT: u16 = 18082;
const LOCAL_BASE_URL: &str = "http://127.0.0.1:18082";
const LOCAL_CHAT_ENDPOINT: &str = "http://127.0.0.1:18082/v1/chat/completions";

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window_in_release(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window_in_release(_cmd: &mut Command) {}

/// 本地翻译模型注册项。
#[derive(Clone, Debug)]
pub struct LocalTranslateModel {
    pub key: &'static str,
    pub filename: &'static str,
    /// ModelScope（魔搭）仓库 id。国内下载首选，走已集成的 modelscope 下载工具。
    pub ms_model_id: &'static str,
    /// HuggingFace 仓库与文件名，作为魔搭失败后的兜底。
    pub repo: &'static str,
    pub remote_file: &'static str,
    /// 上下文长度。翻译是短输入短输出，给小一点省内存、装载更快。
    pub ctx_size: u32,
    /// 是否需要关闭「思考模式」（Qwen3 系列默认会先输出 <think> 段）。
    pub disable_thinking: bool,
    /// 粗略的模型体积下限（字节），用于校验下载文件是否完整。
    pub min_bytes: u64,
    pub note: &'static str,
}

/// HuggingFace 兜底镜像（顺序尝试）。主路径是魔搭，这里只在魔搭不可用时启用。
const MIRROR_HOSTS: [&str; 2] = ["https://hf-mirror.com", "https://huggingface.co"];

/// 默认模型：体积最小、弱 CPU 上最快，满足「无独显也能快速翻译」。
pub const MODEL_QWEN3_06B: LocalTranslateModel = LocalTranslateModel {
    key: "qwen3-0.6b",
    filename: "Qwen3-0.6B-Q4_K_M.gguf",
    ms_model_id: "unsloth/Qwen3-0.6B-GGUF",
    repo: "unsloth/Qwen3-0.6B-GGUF",
    remote_file: "Qwen3-0.6B-Q4_K_M.gguf",
    ctx_size: 4096,
    disable_thinking: true,
    min_bytes: 200_000_000,
    note: "约 378MB，弱 CPU 也能秒级出译文",
};

/// 质量更好一档，体积与速度都翻倍。
pub const MODEL_QWEN3_17B: LocalTranslateModel = LocalTranslateModel {
    key: "qwen3-1.7b",
    filename: "Qwen3-1.7B-Q4_K_M.gguf",
    ms_model_id: "unsloth/Qwen3-1.7B-GGUF",
    repo: "unsloth/Qwen3-1.7B-GGUF",
    remote_file: "Qwen3-1.7B-Q4_K_M.gguf",
    ctx_size: 4096,
    disable_thinking: true,
    min_bytes: 700_000_000,
    note: "约 1.0GB，译文质量更好，长文档偏慢",
};

/// Google 专用翻译模型（55 语种），质量最好但对弱机偏重。
pub const MODEL_TRANSLATEGEMMA_4B: LocalTranslateModel = LocalTranslateModel {
    key: "translategemma-4b",
    filename: "translategemma-4b-it-Q4_K_M.gguf",
    ms_model_id: "bullerwins/translategemma-4b-it-GGUF",
    repo: "bullerwins/translategemma-4b-it-GGUF",
    remote_file: "translategemma-4b-it-Q4_K_M.gguf",
    ctx_size: 4096,
    disable_thinking: false,
    min_bytes: 1_500_000_000,
    note: "约 2.3GB，专用翻译模型，需 8GB 以上内存",
};

pub fn all_models() -> [LocalTranslateModel; 3] {
    [MODEL_QWEN3_06B, MODEL_QWEN3_17B, MODEL_TRANSLATEGEMMA_4B]
}

pub fn model_by_key(key: &str) -> Option<LocalTranslateModel> {
    all_models().into_iter().find(|m| m.key == key)
}

/// 当前激活模型：读取设置 local_translate_model，缺省 Qwen3-0.6B。
pub fn active_model() -> LocalTranslateModel {
    let key = crate::commands::kgraph::get_kg_setting("local_translate_model").unwrap_or_default();
    model_by_key(key.trim()).unwrap_or(MODEL_QWEN3_06B)
}

/// 本地兜底翻译是否启用。默认开；设置 local_translate_enabled=0 或
/// 环境变量 GUAXI_LOCAL_TRANSLATE=0 可以整块关掉（例如磁盘紧张不想下模型）。
pub fn local_translate_enabled() -> bool {
    let env_flag = std::env::var("GUAXI_LOCAL_TRANSLATE").unwrap_or_default();
    let env_flag = env_flag.trim().to_ascii_lowercase();
    if env_flag == "0" || env_flag == "false" || env_flag == "no" {
        return false;
    }
    let setting = crate::commands::kgraph::get_kg_setting("local_translate_enabled")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    !(setting == "0" || setting == "false" || setting == "no")
}

fn model_dir() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/Models/translate")
}

pub fn model_path(model: &LocalTranslateModel) -> Result<PathBuf, String> {
    Ok(model_dir()?.join(model.filename))
}

/// 候选下载地址：用户自定义直链 → 各镜像。
fn model_download_urls(model: &LocalTranslateModel) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if let Some(custom) =
        crate::commands::kgraph::get_kg_setting(&format!("local_translate_url_{}", model.key))
    {
        let c = custom.trim();
        if !c.is_empty() {
            urls.push(c.to_string());
        }
    }
    for host in MIRROR_HOSTS {
        urls.push(format!("{host}/{}/resolve/main/{}", model.repo, model.remote_file));
    }
    urls
}

fn resolve_llama_server_path() -> Result<PathBuf, String> {
    let llama_dir = bundled_resources::ensure_relative_dir("utils/llama")?;
    if !llama_dir.exists() {
        return Err(format!("llama 目录不存在: {}", llama_dir.to_string_lossy()));
    }
    let candidates = if cfg!(windows) {
        vec!["llama-server.exe", "llama_server.exe"]
    } else {
        vec!["llama-server", "llama_server"]
    };
    for name in candidates {
        let direct = llama_dir.join(name);
        if direct.is_file() {
            return Ok(direct);
        }
        if let Some(found) = find_file_by_name(&llama_dir, name, 6) {
            return Ok(found);
        }
    }
    Err(format!(
        "未找到 llama-server 可执行文件: {}",
        llama_dir.to_string_lossy()
    ))
}

fn find_file_by_name(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }
    for entry in fs::read_dir(root).ok()?.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case(name))
                .unwrap_or(false)
        {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(found) = find_file_by_name(&p, name, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

// ---------------------------------------------------------------- 模型文件就位

fn file_is_complete(path: &Path, min_bytes: u64) -> bool {
    fs::metadata(path).map(|m| m.len() >= min_bytes).unwrap_or(false)
}

/// 同一时刻只允许一个下载任务：启动自动下载和设置页手动下载可能撞在一起，
/// 后到的那个等锁，拿到锁时文件已经在了就直接返回，不会重复拉几百 MB。
static DOWNLOAD_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));
static DOWNLOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 当前是否正在下载模型（设置页据此显示 Downloading 状态）。
pub fn is_downloading() -> bool {
    DOWNLOADING.load(std::sync::atomic::Ordering::Relaxed)
}

/// 确保模型文件在本地：优先魔搭（国内快），失败再走 HuggingFace 镜像。
/// 首次调用会下载几百 MB，调用方需要有进度提示或放到后台。
pub async fn ensure_model_present(model: &LocalTranslateModel) -> Result<PathBuf, String> {
    let dest = model_path(model)?;
    if dest.is_file() && file_is_complete(&dest, model.min_bytes) {
        return Ok(dest);
    }

    let _lock = DOWNLOAD_LOCK.lock().await;
    // 等锁期间可能已经被另一个任务下完了。
    if dest.is_file() && file_is_complete(&dest, model.min_bytes) {
        return Ok(dest);
    }
    DOWNLOADING.store(true, std::sync::atomic::Ordering::Relaxed);
    let result = download_model(model, &dest).await;
    DOWNLOADING.store(false, std::sync::atomic::Ordering::Relaxed);
    result
}

async fn download_model(model: &LocalTranslateModel, dest: &Path) -> Result<PathBuf, String> {
    let dest = dest.to_path_buf();
    if dest.is_file() {
        warn!(
            "[LocalTranslate] 模型文件不完整，删除后重下: {}",
            dest.to_string_lossy()
        );
        let _ = fs::remove_file(&dest);
    }
    let parent = dest
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法定位模型目录".to_string())?;
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;

    // 1) 魔搭：项目里已经集成了下载脚本，国内速度最好。
    let ms_dir = parent.clone();
    let ms_id = model.ms_model_id.to_string();
    let ms_file = model.remote_file.to_string();
    let ms_result = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::settingview::run_modelscope_download(&ms_dir, &ms_id, &ms_file)
    })
    .await;

    match ms_result {
        Ok(Ok(())) => {
            // 魔搭脚本可能把文件放在 remote_file 名下，与注册的 filename 未必一致。
            let downloaded = parent.join(model.remote_file);
            if downloaded != dest && downloaded.is_file() && !dest.is_file() {
                let _ = fs::rename(&downloaded, &dest);
            }
            if file_is_complete(&dest, model.min_bytes) {
                crate::commands::settingview::cleanup_modelscope_download_dir(&parent);
                info!("[LocalTranslate] 魔搭下载完成: {}", model.key);
                return Ok(dest);
            }
            warn!("[LocalTranslate] 魔搭下载后文件校验不通过，回退 HuggingFace 镜像");
        }
        Ok(Err(e)) => warn!("[LocalTranslate] 魔搭下载失败，回退 HuggingFace 镜像: {}", e),
        Err(e) => warn!("[LocalTranslate] 魔搭下载任务异常，回退 HuggingFace 镜像: {}", e),
    }

    // 2) HuggingFace 镜像兜底。
    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(7200))
        .build()
        .map_err(|e| e.to_string())?;
    let mut last_err = String::new();
    for url in model_download_urls(model) {
        warn!("[LocalTranslate] 尝试下载 {} ← {}", model.key, url);
        match download_to(&client, &url, &dest, model.min_bytes).await {
            Ok(n) => {
                info!("[LocalTranslate] 下载完成 {} ({} bytes)", model.key, n);
                return Ok(dest);
            }
            Err(e) => {
                warn!("[LocalTranslate] 下载失败（换下一个源）: {} :: {}", url, e);
                last_err = e;
            }
        }
    }
    Err(format!(
        "本地翻译模型 {} 下载失败（魔搭与 HuggingFace 均不可用），最后错误：{}。可手动把 {} 放到 {}",
        model.key,
        last_err,
        model.remote_file,
        dest.to_string_lossy()
    ))
}

async fn download_to(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    min_bytes: u64,
) -> Result<u64, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取失败: {e}"))?;
    if (bytes.len() as u64) < min_bytes {
        return Err(format!("内容异常（仅 {} 字节），疑似错误页", bytes.len()));
    }
    let tmp = dest.with_extension("part");
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok(bytes.len() as u64)
}

// ---------------------------------------------------------------- llama-server 进程

#[derive(Default)]
struct ServerState {
    child: Option<Child>,
    model_key: Option<String>,
}

static SERVER: Lazy<Mutex<ServerState>> = Lazy::new(|| Mutex::new(ServerState::default()));

/// 主程序退出时收掉本地翻译服务，否则会残留占用 18082 端口的后台进程。
pub fn shutdown_local_translate_server() {
    match SERVER.lock() {
        Ok(mut guard) => {
            if let Some(mut child) = guard.child.take() {
                crate::guaxiservices::process_shutdown::terminate_child(
                    "llama-server(translate)",
                    &mut child,
                );
            }
            guard.model_key = None;
        }
        Err(_) => warn!("[LocalTranslate] 进程锁损坏，退出时无法回收本地翻译服务"),
    }
}

fn exe_dir() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法定位可执行目录".to_string())
}

async fn is_ready() -> bool {
    let Ok(client) = ClientBuilder::new().no_proxy().build() else {
        return false;
    };
    for path in ["/health", "/v1/models"] {
        if let Ok(resp) = client
            .get(format!("{LOCAL_BASE_URL}{path}"))
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            if resp.status().is_success() {
                return true;
            }
        }
    }
    false
}

fn refresh_process_state() {
    if let Ok(mut guard) = SERVER.lock() {
        if let Some(child) = guard.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) | Err(_) => {
                    guard.child = None;
                    guard.model_key = None;
                }
                Ok(None) => {}
            }
        }
    }
}

/// 确保本地翻译服务就绪（模型缺失会先下载，然后按需拉起 llama-server）。
async fn ensure_server_started() -> Result<LocalTranslateModel, String> {
    let model = active_model();
    let mpath = ensure_model_present(&model).await?;

    if is_ready().await {
        let same = SERVER
            .lock()
            .ok()
            .and_then(|g| g.model_key.clone())
            .map(|k| k == model.key)
            .unwrap_or(true);
        if same {
            return Ok(model);
        }
        // 用户换了模型：杀掉旧进程重启。
        if let Ok(mut guard) = SERVER.lock() {
            if let Some(mut c) = guard.child.take() {
                let _ = c.kill();
            }
            guard.model_key = None;
        }
        sleep(Duration::from_millis(300)).await;
    }

    refresh_process_state();

    {
        let mut guard = SERVER.lock().map_err(|_| "本地翻译进程锁损坏".to_string())?;
        if guard.child.is_none() {
            let server_path = resolve_llama_server_path()?;
            let log_dir = exe_dir()?.join("logs");
            fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
            let log_path = log_dir.join("translate_server.log");
            let log_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .map_err(|e| format!("打开本地翻译日志失败: {e}"))?;
            let log_err = log_file.try_clone().map_err(|e| e.to_string())?;

            let mut command = Command::new(&server_path);
            command
                .arg("--model")
                .arg(&mpath)
                .arg("--alias")
                .arg(model.key)
                .arg("--host")
                .arg(LOCAL_HOST)
                .arg("--port")
                .arg(LOCAL_PORT.to_string())
                // 弱机优化：线程交给 llama.cpp 自适应；只开单槽位，避免多请求互相拖慢；
                // 上下文压到 4096，装载更快、内存占用更小（翻译都是短输入短输出）。
                .arg("--threads")
                .arg("-1")
                .arg("--parallel")
                .arg("1")
                .arg("--ctx-size")
                .arg(model.ctx_size.to_string())
                .arg("--batch-size")
                .arg("512")
                // 有独显就 offload，纯 CPU 构建会自动忽略这个参数。
                .arg("--n-gpu-layers")
                .arg("99")
                .arg("--no-warmup")
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_err))
                .stdin(Stdio::null());
            apply_no_window_in_release(&mut command);
            if let Some(parent) = server_path.parent() {
                command.current_dir(parent);
            }
            let child = command.spawn().map_err(|e| {
                format!(
                    "启动本地翻译 llama-server 失败: {e}, log={}",
                    log_path.to_string_lossy()
                )
            })?;
            guard.child = Some(child);
            guard.model_key = Some(model.key.to_string());
            info!("[LocalTranslate] llama-server 已启动: {}", model.key);
        }
    }

    for _ in 0..200 {
        if is_ready().await {
            return Ok(model);
        }
        if let Ok(mut guard) = SERVER.lock() {
            if let Some(child) = guard.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    guard.child = None;
                    guard.model_key = None;
                    return Err(format!("本地翻译 llama-server 启动后异常退出: {status}"));
                }
            }
        }
        sleep(Duration::from_millis(250)).await;
    }

    if let Ok(mut guard) = SERVER.lock() {
        if let Some(mut c) = guard.child.take() {
            let _ = c.kill();
        }
        guard.model_key = None;
    }
    Err("本地翻译服务启动超时".to_string())
}

// ---------------------------------------------------------------- 翻译

/// 语种代码 → 提示词里用的自然语言名。小模型对英文语种名最敏感。
fn lang_display(code: &str) -> String {
    let c = code.trim().to_ascii_lowercase();
    let name = match c.as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh_hans" | "zh-chs" => "Simplified Chinese",
        "zh-tw" | "zh-hant" | "zh-cht" | "zh-hk" => "Traditional Chinese",
        "en" | "en-us" | "en-gb" => "English",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "ru" => "Russian",
        "pt" => "Portuguese",
        "it" => "Italian",
        "ar" => "Arabic",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "" | "auto" => "the same language as detected",
        other => other,
    };
    name.to_string()
}

/// 小模型很容易画蛇添足，这里把常见的多余外壳剥掉。
fn clean_model_output(raw: &str) -> String {
    let mut text = raw.trim().to_string();

    // 1) 思考块：Qwen3 关不掉思考时会先吐 <think>...</think>。
    if let Some(end) = text.find("</think>") {
        text = text[end + "</think>".len()..].trim().to_string();
    }
    text = text.replace("<think>", "").replace("</think>", "");

    // 2) 常见前缀。
    for prefix in [
        "译文：", "翻译：", "翻译结果：", "Translation:", "Translated text:", "译文:",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim().to_string();
        }
    }

    // 3) 整体被引号或代码围栏包裹。
    let trimmed = text.trim();
    if trimmed.starts_with("```") {
        let inner: Vec<&str> = trimmed.lines().collect();
        if inner.len() >= 2 {
            let body = inner[1..inner.len().saturating_sub(1)].join("\n");
            if !body.trim().is_empty() {
                text = body;
            }
        }
    }
    let trimmed = text.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.chars().next().unwrap_or(' ');
        let last = trimmed.chars().last().unwrap_or(' ');
        let pairs = [('"', '"'), ('“', '”'), ('「', '」'), ('\'', '\'')];
        if pairs.iter().any(|(a, b)| first == *a && last == *b) {
            let mut chars: Vec<char> = trimmed.chars().collect();
            chars.pop();
            if !chars.is_empty() {
                chars.remove(0);
            }
            text = chars.into_iter().collect();
        }
    }

    text.trim().to_string()
}

fn build_prompt(model: &LocalTranslateModel, text: &str, from: &str, to: &str) -> (String, String) {
    let target = lang_display(to);
    let source_hint = if from.trim().is_empty() || from.trim().eq_ignore_ascii_case("auto") {
        String::new()
    } else {
        format!(" The source language is {}.", lang_display(from))
    };
    let system = format!(
        "You are a professional translation engine. Translate the user's text into {target}.{source_hint} \
Rules: output ONLY the translation; no explanation, no notes, no pinyin, no quotes around the result; \
keep numbers, URLs, file paths, code identifiers and inline markup exactly as in the source; \
preserve the original line breaks and list markers."
    );
    let mut user = text.to_string();
    if model.disable_thinking {
        // Qwen3 的软开关：即使服务端不支持 chat_template_kwargs 也能关掉思考。
        user.push_str("\n/no_think");
    }
    (system, user)
}

fn max_tokens_for(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    chars.saturating_mul(2).clamp(128, 2048)
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: String,
}
#[derive(Deserialize)]
struct ChatChoice {
    #[serde(default)]
    message: Option<ChatChoiceMessage>,
}
#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

/// llama-server 只开了一个槽位，这里用异步锁串行化，避免并发请求互相排队拖长尾。
static INFER_LOCK: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

fn request_timeout_secs() -> u64 {
    std::env::var("GUAXI_LOCAL_TRANSLATE_TIMEOUT_S")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(180)
        .clamp(20, 900)
}

async fn translate_one(
    client: &reqwest::Client,
    model: &LocalTranslateModel,
    text: &str,
    from: &str,
    to: &str,
) -> Result<String, String> {
    let (system, user) = build_prompt(model, text, from, to);
    let payload = serde_json::json!({
        "model": model.key,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": 0.2,
        "top_p": 0.9,
        "max_tokens": max_tokens_for(text),
        "stream": false,
        // 新版 llama-server 支持；旧版会忽略，此时靠 /no_think 兜底。
        "chat_template_kwargs": { "enable_thinking": false }
    });

    let resp = client
        .post(LOCAL_CHAT_ENDPOINT)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("本地翻译请求失败: {e}"))?;
    if !resp.status().is_success() {
        let code = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("本地翻译服务返回 {code}: {body}"));
    }
    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("解析本地翻译响应失败: {e}"))?;
    let raw = parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message)
        .map(|m| m.content)
        .unwrap_or_default();
    let cleaned = clean_model_output(&raw);
    if cleaned.is_empty() {
        return Err("本地翻译返回空结果".to_string());
    }
    Ok(cleaned)
}

/// 用本地小模型翻译一批文本。返回与输入等长的数组，单条失败留空字符串（调用方决定如何回退）。
pub async fn translate_texts_local(
    texts: &[String],
    from: &str,
    to: &str,
) -> Result<Vec<String>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    if !local_translate_enabled() {
        return Err("本地翻译小模型已关闭（local_translate_enabled=0）".to_string());
    }
    let model = ensure_server_started().await?;
    let client = ClientBuilder::new()
        .no_proxy()
        .timeout(Duration::from_secs(request_timeout_secs()))
        .build()
        .map_err(|e| e.to_string())?;

    let _guard = INFER_LOCK.lock().await;
    let mut out = Vec::with_capacity(texts.len());
    for text in texts {
        let src = text.trim();
        if src.is_empty() {
            out.push(String::new());
            continue;
        }
        match translate_one(&client, &model, src, from, to).await {
            Ok(t) => out.push(t),
            Err(e) => {
                warn!("[LocalTranslate] 单条翻译失败: {}", e);
                out.push(String::new());
            }
        }
    }
    if out.iter().all(|t| t.trim().is_empty()) {
        return Err("本地翻译全部失败".to_string());
    }
    Ok(out)
}

// ---------------------------------------------------------------- 启动预置

/// 首次启动时在后台把翻译模型下好，用户第一次用划词/全文翻译就已经有兜底能力，
/// 不用等一次几百 MB 的下载。
///
/// 刻意只下模型、不拉起 llama-server：常驻一个推理进程要占几百 MB 内存，
/// 而它只在免费接口全挂时才需要，按需启动足够。
pub fn spawn_startup_model_provision() {
    tauri::async_runtime::spawn(async {
        if !local_translate_enabled() {
            info!("[LocalTranslate] 本地兜底翻译已关闭，跳过启动预下载");
            return;
        }
        let model = active_model();
        match model_path(&model) {
            Ok(p) if file_is_complete(&p, model.min_bytes) => {
                info!("[LocalTranslate] 翻译模型已就位: {}", model.key);
                return;
            }
            Ok(_) => {}
            Err(e) => {
                warn!("[LocalTranslate] 无法定位模型目录，跳过启动预下载: {}", e);
                return;
            }
        }

        // 让位给启动阶段的其它初始化（后端服务、Agent、转换引擎装配），别一开机就抢带宽和磁盘。
        sleep(Duration::from_secs(25)).await;
        info!("[LocalTranslate] 启动预下载翻译模型: {}", model.key);
        match ensure_model_present(&model).await {
            Ok(p) => info!("[LocalTranslate] 启动预下载完成: {}", p.to_string_lossy()),
            Err(e) => warn!(
                "[LocalTranslate] 启动预下载失败（可在设置页手动下载/导入）: {}",
                e
            ),
        }
    });
}

// ---------------------------------------------------------------- Tauri 命令

#[derive(Serialize, Debug, Clone)]
pub struct LocalTranslateModelInfo {
    pub key: String,
    pub filename: String,
    pub note: String,
    pub downloaded: bool,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Debug, Clone)]
pub struct LocalTranslateStatus {
    pub enabled: bool,
    pub ready: bool,
    pub downloading: bool,
    pub endpoint: String,
    pub active_model: String,
    /// 当前激活模型是否已经下载完整。
    pub installed: bool,
    pub expected_path: String,
    pub llama_server_exists: bool,
    pub llama_server_path: String,
    pub models: Vec<LocalTranslateModelInfo>,
}

fn model_info(model: &LocalTranslateModel) -> LocalTranslateModelInfo {
    let path = model_path(model).unwrap_or_default();
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    LocalTranslateModelInfo {
        key: model.key.to_string(),
        filename: model.filename.to_string(),
        note: model.note.to_string(),
        downloaded: size >= model.min_bytes,
        path: path.to_string_lossy().to_string(),
        size_bytes: size,
    }
}

/// 查询本地翻译小模型状态（不触发下载、不拉起服务）。
#[tauri::command]
pub async fn translate_local_status() -> Result<LocalTranslateStatus, String> {
    let model = active_model();
    let expected = model_path(&model).unwrap_or_default();
    let llama = resolve_llama_server_path().ok();
    Ok(LocalTranslateStatus {
        enabled: local_translate_enabled(),
        ready: is_ready().await,
        downloading: is_downloading(),
        endpoint: LOCAL_CHAT_ENDPOINT.to_string(),
        active_model: model.key.to_string(),
        installed: file_is_complete(&expected, model.min_bytes),
        expected_path: expected.to_string_lossy().to_string(),
        llama_server_exists: llama.as_ref().map(|p| p.is_file()).unwrap_or(false),
        llama_server_path: llama
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        models: all_models().iter().map(model_info).collect(),
    })
}

/// 手动下载当前激活的翻译模型（设置页按钮）。已存在则直接返回路径。
#[tauri::command]
pub async fn translate_local_download_model(
    model_key: Option<String>,
) -> Result<String, String> {
    let model = match model_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(k) => model_by_key(k).ok_or_else(|| format!("未知的本地翻译模型: {}", k))?,
        None => active_model(),
    };
    let path = ensure_model_present(&model).await?;
    Ok(path.to_string_lossy().to_string())
}

/// 手动导入一个本地 .gguf 作为翻译模型（离线机器、或自己换量化版本）。
#[tauri::command]
pub async fn translate_local_import_model(
    source_path: String,
    model_key: Option<String>,
) -> Result<String, String> {
    let src = PathBuf::from(source_path.trim());
    if !src.is_file() {
        return Err(format!("源文件不存在: {}", src.to_string_lossy()));
    }
    let ext_ok = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !ext_ok {
        return Err("只支持 .gguf 模型文件".to_string());
    }

    let model = match model_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(k) => model_by_key(k).ok_or_else(|| format!("未知的本地翻译模型: {}", k))?,
        None => active_model(),
    };
    let dest = model_path(&model)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // 先落到临时文件再改名，避免半个文件被当成可用模型加载。
    let tmp = dest.with_extension("copying");
    let _ = fs::remove_file(&tmp);
    let src_c = src.clone();
    let tmp_c = tmp.clone();
    tauri::async_runtime::spawn_blocking(move || fs::copy(&src_c, &tmp_c).map(|_| ()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("拷贝模型失败: {}", e))?;
    fs::rename(&tmp, &dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("落盘模型失败: {}", e)
    })?;

    // 换了权重就把旧服务收掉，下次翻译重新加载。
    shutdown_local_translate_server();
    Ok(dest.to_string_lossy().to_string())
}

/// 预热：下载（若缺失）并拉起本地翻译服务。首次会拉几百 MB，建议在后台调用。
#[tauri::command]
pub async fn translate_local_prepare() -> Result<LocalTranslateStatus, String> {
    ensure_server_started().await?;
    translate_local_status().await
}

/// 切换本地翻译模型。切换后下次翻译会自动重启服务加载新模型。
#[tauri::command]
pub async fn translate_local_set_model(model_key: String) -> Result<LocalTranslateStatus, String> {
    let model = model_by_key(model_key.trim())
        .ok_or_else(|| format!("未知的本地翻译模型: {}", model_key))?;
    crate::commands::kgraph::set_kg_setting("local_translate_model", model.key)?;
    translate_local_status().await
}

/// 开关本地兜底翻译。
#[tauri::command]
pub async fn translate_local_set_enabled(enabled: bool) -> Result<LocalTranslateStatus, String> {
    crate::commands::kgraph::set_kg_setting(
        "local_translate_enabled",
        if enabled { "1" } else { "0" },
    )?;
    translate_local_status().await
}

/// 手动停止本地翻译服务，释放内存。
#[tauri::command]
pub async fn translate_local_stop() -> Result<bool, String> {
    shutdown_local_translate_server();
    Ok(true)
}

/// 直接用本地小模型翻译（供「强制本地翻译」入口使用，不经过免费接口池）。
#[tauri::command]
pub async fn translate_local_texts(
    texts: Vec<String>,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<String>, String> {
    let from = from.unwrap_or_else(|| "auto".into());
    let to = to.unwrap_or_else(|| "zh".into());
    translate_texts_local(&texts, &from, &to).await
}

