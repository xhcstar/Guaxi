//! 本地 embedding 服务(复用 llama.cpp)
//!
//! 在 deepseek 聊天服务之外,再起一个 llama-server 实例(端口 18081,`--embedding` 模式),
//! 加载一个小型多语言 embedding GGUF 模型,通过 OpenAI 兼容的 `/v1/embeddings` 出向量。
//! 复用现有的 llama-server 二进制解析、进程管理、GPU offload(有独显)/CPU 回退逻辑。
//!
//! 模型可切换:默认 multilingual-e5-small(轻、集成显卡可跑),bge-m3 可选(中英+术语质量更强)。
//! 运行时验证点:需要用户机器上 utils/Models/embedding/ 存在对应 GGUF 且能成功起服务。

use crate::guaxiservices::bundled_resources;
use once_cell::sync::Lazy;
use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
#[cfg(all(windows, not(debug_assertions)))]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

const EMBED_SERVER_HOST: &str = "127.0.0.1";
const EMBED_SERVER_PORT: u16 = 18081;
const EMBED_SERVER_URL: &str = "http://127.0.0.1:18081";
const EMBED_ENDPOINT: &str = "http://127.0.0.1:18081/v1/embeddings";

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window_in_release(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window_in_release(_cmd: &mut Command) {}

/// embedding 模型注册项。
#[derive(Clone, Debug)]
pub struct EmbedModel {
    pub key: &'static str,
    pub filename: &'static str,
    pub dim: usize,
    /// e5 系列需要 "query: " / "passage: " 前缀;bge-m3 不需要。
    pub query_prefix: &'static str,
    pub passage_prefix: &'static str,
    /// HuggingFace 仓库与文件名。下载时按多镜像拼装 URL。
    pub repo: &'static str,
    pub remote_file: &'static str,
}

/// 下载镜像主机(顺序尝试)。国内优先 hf-mirror,失败回退官方站。
const MIRROR_HOSTS: [&str; 2] = ["https://hf-mirror.com", "https://huggingface.co"];

pub const MODEL_E5_SMALL: EmbedModel = EmbedModel {
    key: "multilingual-e5-small",
    filename: "multilingual-e5-small.Q8_0.gguf",
    dim: 384,
    query_prefix: "query: ",
    passage_prefix: "passage: ",
    repo: "cstr/multilingual-e5-small-GGUF",
    remote_file: "multilingual-e5-small-q8_0.gguf",
};

pub const MODEL_BGE_M3: EmbedModel = EmbedModel {
    key: "bge-m3",
    filename: "bge-m3-Q8_0.gguf",
    dim: 1024,
    query_prefix: "",
    passage_prefix: "",
    repo: "gpustack/bge-m3-GGUF",
    remote_file: "bge-m3-Q8_0.gguf",
};

/// 构造该模型的候选下载 URL 列表:用户自定义直链(kg_settings) → 各镜像。
fn model_download_urls(model: &EmbedModel) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if let Some(custom) =
        crate::commands::kgraph::get_kg_setting(&format!("embedding_url_{}", model.key))
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

pub fn all_models() -> [EmbedModel; 2] {
    [MODEL_E5_SMALL, MODEL_BGE_M3]
}

pub fn model_by_key(key: &str) -> Option<EmbedModel> {
    all_models().into_iter().find(|m| m.key == key)
}

/// 当前激活模型:读取 kg_settings.embedding_model,缺省 bge-m3(中英混合+术语质量更强)。
pub fn active_model() -> EmbedModel {
    let key = crate::commands::kgraph::get_kg_setting("embedding_model").unwrap_or_default();
    model_by_key(key.trim()).unwrap_or(MODEL_BGE_M3)
}

fn embedding_model_dir() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir("utils/Models/embedding")
}

pub fn model_path(model: &EmbedModel) -> Result<PathBuf, String> {
    Ok(embedding_model_dir()?.join(model.filename))
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

fn find_file_by_name(root: &std::path::Path, name: &str, max_depth: usize) -> Option<PathBuf> {
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

#[derive(Default)]
struct EmbedProcessState {
    child: Option<Child>,
    model_key: Option<String>,
}

static EMBED_PROCESS: Lazy<Mutex<EmbedProcessState>> =
    Lazy::new(|| Mutex::new(EmbedProcessState::default()));

/// 关闭 embedding llama-server。
///
/// 设计说明：知识图谱/语义检索一旦用过就会常驻这个 llama-server，主程序退出时必须收掉，
/// 否则会残留一个占用显存与 8080 端口的后台进程，下次启动还会因端口被占而报错。
pub fn shutdown_embedding_server() {
    match EMBED_PROCESS.lock() {
        Ok(mut guard) => {
            if let Some(mut child) = guard.child.take() {
                crate::guaxiservices::process_shutdown::terminate_child(
                    "llama-server(embedding)",
                    &mut child,
                );
            }
            guard.model_key = None;
        }
        Err(_) => {
            warn!("[embedding] 进程锁损坏，退出时无法回收 embedding 服务");
        }
    }
}

fn exe_dir() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法定位可执行目录".to_string())
}

async fn is_embed_server_ready() -> bool {
    let Ok(client) = ClientBuilder::new().no_proxy().build() else {
        return false;
    };
    for path in ["/health", "/v1/models"] {
        if let Ok(resp) = client.get(format!("{EMBED_SERVER_URL}{path}")).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
    }
    false
}

fn refresh_process_state() {
    if let Ok(mut guard) = EMBED_PROCESS.lock() {
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

#[derive(Serialize, Debug, Clone)]
pub struct EmbeddingServiceStatus {
    pub ready: bool,
    pub endpoint: String,
    pub model_key: String,
    pub model_path: String,
    pub dim: usize,
    pub model_present: bool,
}

/// 确保模型文件就位:不存在则自动下载(多镜像逐个尝试,原子落盘)。返回模型文件路径。
async fn ensure_model_present(model: &EmbedModel) -> Result<PathBuf, String> {
    let dest = model_path(model)?;
    if dest.is_file() {
        // 校验已存在文件大小,过小视为上次下载残缺,删除重下
        let ok = fs::metadata(&dest).map(|m| m.len() >= 8_000_000).unwrap_or(false);
        if ok {
            return Ok(dest);
        }
        warn!("embedding 模型文件异常(过小),删除后重新下载: {}", dest.to_string_lossy());
        let _ = fs::remove_file(&dest);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let urls = model_download_urls(model);
    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(7200))
        .build()
        .map_err(|e| e.to_string())?;

    let mut last_err = String::new();
    for url in &urls {
        warn!("embedding 模型缺失,尝试下载: {} ← {}", model.key, url);
        match download_to(&client, url, &dest).await {
            Ok(n) => {
                warn!("embedding 模型下载完成: {} ({} bytes) via {}", model.key, n, url);
                return Ok(dest);
            }
            Err(e) => {
                warn!("下载失败(将尝试下一个镜像): {} :: {}", url, e);
                last_err = e;
            }
        }
    }
    Err(format!(
        "embedding 模型 {} 所有镜像下载均失败。最后错误:{}。可在设置写入 kg_settings.embedding_url_{} 指定可达直链,或手动放入 {}",
        model.key, last_err, model.key, dest.to_string_lossy()
    ))
}

/// 从单个 URL 下载到目标(.part 原子落盘 + 体积健全性校验)。返回字节数。
async fn download_to(
    client: &reqwest::Client,
    url: &str,
    dest: &std::path::Path,
) -> Result<u64, String> {
    let resp = client.get(url).send().await.map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取失败: {e}"))?;
    if bytes.len() < 8_000_000 {
        return Err(format!("内容异常(仅 {} 字节),疑似错误页/重定向占位", bytes.len()));
    }
    let tmp = dest.with_extension("part");
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
    Ok(bytes.len() as u64)
}

/// 确保 embedding 服务就绪(模型缺失会自动下载,然后按需启动 llama-server 的 embedding 实例)。
async fn ensure_embed_server_started() -> Result<EmbedModel, String> {
    let model = active_model();
    // 模型缺失 → 自动下载(不再要求用户手动安装)
    let mpath = ensure_model_present(&model).await?;

    // 已就绪且模型未变 → 直接复用
    if is_embed_server_ready().await {
        let same = EMBED_PROCESS
            .lock()
            .ok()
            .and_then(|g| g.model_key.clone())
            .map(|k| k == model.key)
            .unwrap_or(true);
        if same {
            return Ok(model);
        }
        // 模型切换:杀掉旧进程重启
        if let Ok(mut guard) = EMBED_PROCESS.lock() {
            if let Some(mut c) = guard.child.take() {
                let _ = c.kill();
            }
            guard.model_key = None;
        }
        sleep(Duration::from_millis(300)).await;
    }

    refresh_process_state();

    {
        let mut guard = EMBED_PROCESS.lock().map_err(|_| "embedding 进程锁损坏".to_string())?;
        if guard.child.is_none() {
            let server_path = resolve_llama_server_path()?;
            let log_dir = exe_dir()?.join("logs");
            fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
            let log_path = log_dir.join("embedding_server.log");
            let log_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .map_err(|e| format!("打开 embedding 日志失败: {e}"))?;
            let log_err = log_file.try_clone().map_err(|e| e.to_string())?;

            let mut command = Command::new(&server_path);
            command
                .arg("--model")
                .arg(&mpath)
                .arg("--embedding")
                .arg("--pooling")
                .arg("mean")
                .arg("--host")
                .arg(EMBED_SERVER_HOST)
                .arg("--port")
                .arg(EMBED_SERVER_PORT.to_string())
                .arg("--threads")
                .arg("-1")
                .arg("--n-gpu-layers")
                .arg("99")
                .arg("--ctx-size")
                .arg("8192")
                // 逻辑批与物理批都提到 ctx 大小:embedding 模型为非因果注意力,
                // 整条输入序列必须落在单个物理批(ubatch)内,默认 512 会导致
                // 超过 512 token 的段落报 500 "input is too large ... increase the physical batch size"。
                .arg("--batch-size")
                .arg("8192")
                .arg("--ubatch-size")
                .arg("8192")
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_err))
                .stdin(Stdio::null());
            apply_no_window_in_release(&mut command);
            if let Some(parent) = server_path.parent() {
                command.current_dir(parent);
            }
            let child = command.spawn().map_err(|e| {
                format!("启动 embedding llama-server 失败: {e},log={}", log_path.to_string_lossy())
            })?;
            guard.child = Some(child);
            guard.model_key = Some(model.key.to_string());
        }
    }

    for _ in 0..160 {
        if is_embed_server_ready().await {
            return Ok(model);
        }
        if let Ok(mut guard) = EMBED_PROCESS.lock() {
            if let Some(child) = guard.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    guard.child = None;
                    guard.model_key = None;
                    return Err(format!("embedding llama-server 启动后异常退出: {status}"));
                }
            }
        }
        sleep(Duration::from_millis(250)).await;
    }

    if let Ok(mut guard) = EMBED_PROCESS.lock() {
        if let Some(mut c) = guard.child.take() {
            let _ = c.kill();
        }
        guard.model_key = None;
    }
    Err("embedding 服务启动超时".to_string())
}

/// 命令:确保 embedding 服务就绪并返回状态。
#[tauri::command]
pub async fn kg_ensure_embedding_service() -> Result<EmbeddingServiceStatus, String> {
    let model = ensure_embed_server_started().await?;
    let mpath = model_path(&model)?;
    Ok(EmbeddingServiceStatus {
        ready: true,
        endpoint: EMBED_ENDPOINT.to_string(),
        model_key: model.key.to_string(),
        model_path: mpath.to_string_lossy().to_string(),
        dim: model.dim,
        model_present: mpath.is_file(),
    })
}

/// 命令:查询 embedding 服务/模型状态(不强制启动)。
#[tauri::command]
pub async fn kg_embedding_status() -> Result<EmbeddingServiceStatus, String> {
    let model = active_model();
    let mpath = model_path(&model)?;
    Ok(EmbeddingServiceStatus {
        ready: is_embed_server_ready().await,
        endpoint: EMBED_ENDPOINT.to_string(),
        model_key: model.key.to_string(),
        model_path: mpath.to_string_lossy().to_string(),
        dim: model.dim,
        model_present: mpath.is_file(),
    })
}

#[derive(Serialize)]
struct EmbedReq<'a> {
    model: &'a str,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbedRespItem {
    embedding: Vec<f32>,
}
#[derive(Deserialize)]
struct EmbedResp {
    data: Vec<EmbedRespItem>,
}

/// 对一批文本求向量。`is_query=true` 用 query 前缀,否则用 passage 前缀(对 e5 类模型有效)。
/// 自动分批,避免单请求过大。
pub async fn embed_texts(texts: &[String], is_query: bool) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let model = ensure_embed_server_started().await?;
    let prefix = if is_query { model.query_prefix } else { model.passage_prefix };

    let client = ClientBuilder::new()
        .no_proxy()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(64) {
        let input: Vec<String> = chunk
            .iter()
            .map(|t| {
                let t = truncate_for_embedding(t.trim());
                if prefix.is_empty() {
                    t
                } else {
                    format!("{prefix}{t}")
                }
            })
            .collect();
        let req = EmbedReq { model: model.key, input };
        let resp = client
            .post(EMBED_ENDPOINT)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("embedding 请求失败: {e}"))?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("embedding 服务返回 {code}: {body}"));
        }
        let parsed: EmbedResp = resp.json().await.map_err(|e| format!("解析 embedding 响应失败: {e}"))?;
        if parsed.data.len() != chunk.len() {
            warn!("embedding 返回数量不匹配: got {} expected {}", parsed.data.len(), chunk.len());
        }
        for item in parsed.data {
            out.push(l2_normalize(item.embedding));
        }
    }
    Ok(out)
}

/// 按字符粗略截断,确保单条输入不超过模型上下文(8192 token)。
/// CJK 最坏约 1 token/字,取 6000 字上限留足余量;ASCII 占 token 更少,保守不影响。
/// 仅对极端超长段落生效,正常段落原样通过。
fn truncate_for_embedding(t: &str) -> String {
    const MAX_CHARS: usize = 6000;
    if t.chars().count() <= MAX_CHARS {
        return t.to_string();
    }
    t.chars().take(MAX_CHARS).collect()
}

/// L2 归一化,便于用点积近似余弦相似度。
fn l2_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    v
}

#[derive(Serialize)]
pub struct DownloadResult {
    pub ok: bool,
    pub model_key: String,
    pub path: String,
    pub bytes: u64,
}

/// 命令:显式下载指定(或当前)embedding 模型。通常无需手动调用——服务启动时会自动下载。
#[tauri::command]
pub async fn kg_download_embedding_model(model_key: Option<String>) -> Result<DownloadResult, String> {
    let model = match model_key {
        Some(k) => model_by_key(k.trim()).ok_or_else(|| format!("未知 embedding 模型: {k}"))?,
        None => active_model(),
    };
    let dest = ensure_model_present(&model).await?;
    let bytes = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    Ok(DownloadResult {
        ok: true,
        model_key: model.key.into(),
        path: dest.to_string_lossy().into(),
        bytes,
    })
}
