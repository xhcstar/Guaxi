use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::guaxiservices::bundled_resources;
use crate::guaxiservices::lsp;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose, Engine as _};
use keyring::Entry;
use rand::rngs::OsRng;
use rand::TryRngCore;
use tracing::{info, warn};
use uuid::Uuid;
use zip::ZipArchive;

use git2::{Repository, Signature};

#[derive(Serialize, Deserialize, Debug)]
pub struct Library {
    library_id: i32,
    library_path: String,
    library_name: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LibraryConfig {
    pub libraries_repo_path: String,
    pub library: Vec<Library>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    #[serde(default)]
    appearance: AppearanceConfig,
    libraries_repo_config: LibraryConfig,
    active_repo_path: Option<String>,
    repopathlist: Vec<String>,
    libraries_root: String,
    #[serde(default = "default_image_bed_root")]
    image_bed_root: String,
    #[serde(default)]
    resource_repo_sync: ResourceRepoSyncConfig,
    #[serde(default)]
    life_album_album_dir: String,
    #[serde(default)]
    template_repo_path: String,
    #[serde(default)]
    slidev_repo_root: String,
    #[serde(default = "default_chat_record_root")]
    chat_record_root: String,
    #[serde(default)]
    knowledge_bases_root: String,
    #[serde(default)]
    knowledge_bases_remote_url: String,
    #[serde(default)]
    knowledge_bases_remote_username: String,
    #[serde(default)]
    knowledge_bases_remote_token: String,
    #[serde(default)]
    knowledge_bases_git_username: String,
    #[serde(default)]
    knowledge_bases_git_email: String,
    #[serde(default)]
    device_id: String,
    #[serde(default = "default_gc_cycle_seconds")]
    pub gc_cycle_seconds: u64,
    #[serde(default)]
    lsp_syntax_highlight_enabled: bool,
    #[serde(default)]
    lsp_github_token: String,
    #[serde(default)]
    webrtc_signal_url: String,
    #[serde(default)]
    llm_config: LlmConfig,
    #[serde(default)]
    tts_config: TtsConfig,
    #[serde(default)]
    asr_config: AsrConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct LlmConfig {
    #[serde(default)]
    pub chatgpt_api_key: String,
    #[serde(default)]
    pub deepseek_api_key: String,
    #[serde(default)]
    pub qwen_api_key: String,
    #[serde(default)]
    pub kimi_api_key: String,
    #[serde(default)]
    pub doubao_api_key: String,
    #[serde(default)]
    pub siliconflow_api_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TtsConfig {
    #[serde(default = "default_tts_provider")]
    pub provider: String,
    #[serde(default = "default_tts_model")]
    pub model: String,
    #[serde(default = "default_tts_tokens")]
    pub tokens: String,
    #[serde(default = "default_tts_exe")]
    pub exe: String,
    #[serde(default = "default_tts_output_dir")]
    pub output_dir: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider: default_tts_provider(),
            model: default_tts_model(),
            tokens: default_tts_tokens(),
            exe: default_tts_exe(),
            output_dir: default_tts_output_dir(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AsrConfig {
    #[serde(default = "default_asr_provider")]
    pub provider: String,
    #[serde(default = "default_asr_model")]
    pub model: String,
    #[serde(default = "default_asr_exe")]
    pub exe: String,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            provider: default_asr_provider(),
            model: default_asr_model(),
            exe: default_asr_exe(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WebDavSyncConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub remote_base_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SyncthingSyncConfig {
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub folder_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceRepoSyncConfig {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub prefer_pull: bool,
    #[serde(default)]
    pub webdav: WebDavSyncConfig,
    #[serde(default)]
    pub syncthing: SyncthingSyncConfig,
}

impl Default for ResourceRepoSyncConfig {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            prefer_pull: false,
            webdav: WebDavSyncConfig::default(),
            syncthing: SyncthingSyncConfig::default(),
        }
    }
}

fn default_gc_cycle_seconds() -> u64 {
    3600
}

fn default_image_bed_root() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir.join("ResourceRepo").to_string_lossy().to_string()
}

fn default_template_repo_path() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir.join("Template").to_string_lossy().to_string()
}

fn default_slidev_repo_root() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir.join("SlideRepo").to_string_lossy().to_string()
}

fn default_chat_record_root() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir.join("ChatRecord").to_string_lossy().to_string()
}

fn default_tts_provider() -> String {
    "sherpa-onnx".to_string()
}

fn default_tts_model() -> String {
    "utils/Models/tts/kokoro_multi_lang/model.onnx".to_string()
}

fn default_tts_tokens() -> String {
    "utils/Models/tts/kokoro_multi_lang/tokens.txt".to_string()
}

fn default_tts_exe() -> String {
    if cfg!(target_os = "windows") {
        "utils/sherpa-onnx/sherpa-onnx-offline-tts.exe".to_string()
    } else {
        "utils/sherpa-onnx/sherpa-onnx-offline-tts".to_string()
    }
}

fn default_tts_output_dir() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir.join("tts_output").to_string_lossy().to_string()
}

fn default_asr_provider() -> String {
    "sherpa-onnx".to_string()
}

fn default_asr_model() -> String {
    "".to_string()
}

fn default_asr_exe() -> String {
    if cfg!(target_os = "windows") {
        "utils/sherpa-onnx/sherpa-onnx-offline.exe".to_string()
    } else {
        "utils/sherpa-onnx/sherpa-onnx-offline".to_string()
    }
}

fn default_knowledge_bases_root() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir
        .join("KnowledgeBases")
        .to_string_lossy()
        .to_string()
}

const KNOWLEDGE_BASES_REMOTE_URL: &str = "https://gitee.com/xhcstar/knowledge-bases.git";

fn normalize_knowledge_bases_root(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }
    let p = PathBuf::from(trimmed);
    match p.file_name().and_then(|n| n.to_str()) {
        Some("KnowledgeBases") => p,
        _ => p.join("KnowledgeBases"),
    }
}

fn normalize_remote_url(raw: &str) -> String {
    let mut s = raw
        .trim()
        .trim_matches(|c: char| matches!(c, '`' | '"' | '\''))
        .to_string();
    if s.is_empty() {
        return s;
    }

    if let Some((left, _)) = s.split_once('#') {
        s = left.to_string();
    }
    if let Some((left, _)) = s.split_once('?') {
        s = left.to_string();
    }

    while s.ends_with('/') {
        s.pop();
    }

    if (s.starts_with("http://") || s.starts_with("https://")) && !s.ends_with(".git") {
        s.push_str(".git");
    }

    s
}

fn schedule_knowledge_bases_git_init(root: PathBuf) {
    if root.as_os_str().is_empty() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let root_for_job = root.clone();
        let result = tauri::async_runtime::spawn_blocking(move || ensure_knowledge_bases_git_repo(&root_for_job)).await;

        match result {
            Ok(Ok(_)) => {
                info!(
                    root = root.to_string_lossy().as_ref(),
                    "KnowledgeBases git repo ready"
                );
            }
            Ok(Err(e)) => {
                warn!(
                    root = root.to_string_lossy().as_ref(),
                    error = e.as_str(),
                    "KnowledgeBases git repo init failed"
                );
            }
            Err(e) => {
                warn!(
                    root = root.to_string_lossy().as_ref(),
                    error = e.to_string().as_str(),
                    "KnowledgeBases git repo init task failed"
                );
            }
        }
    });
}

fn ensure_knowledge_bases_git_repo(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;

    let repo = if root.join(".git").exists() {
        Repository::open(root).map_err(|e| e.to_string())?
    } else {
        Repository::init(root).map_err(|e| e.to_string())?
    };

    let config = load_config();

    let git_username_cfg = config.knowledge_bases_git_username.trim().to_string();
    let git_email_cfg = config.knowledge_bases_git_email.trim().to_string();
    let git_username = if git_username_cfg.is_empty() {
        "Guaxi".to_string()
    } else {
        git_username_cfg
    };
    let git_email = if git_email_cfg.is_empty() {
        "guaxi@localhost".to_string()
    } else {
        git_email_cfg
    };

    let mut repo_cfg = repo.config().map_err(|e| e.to_string())?;
    repo_cfg
        .set_str("user.name", git_username.as_str())
        .map_err(|e| e.to_string())?;
    repo_cfg
        .set_str("user.email", git_email.as_str())
        .map_err(|e| e.to_string())?;

    if repo.head().is_err() {
        let readme_rel = Path::new("README.md");
        let readme_abs = root.join(readme_rel);
        if !readme_abs.exists() {
            fs::write(&readme_abs, "# knowledge-bases\n")
                .map_err(|e| e.to_string())?;
        }

        let mut index = repo.index().map_err(|e| e.to_string())?;
        index.add_path(readme_rel).map_err(|e| e.to_string())?;
        index.write().map_err(|e| e.to_string())?;

        let tree_id = index.write_tree().map_err(|e| e.to_string())?;
        let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

        let sig = Signature::now(git_username.as_str(), git_email.as_str())
            .map_err(|e| e.to_string())?;
        repo.commit(
            Some("refs/heads/master"),
            &sig,
            &sig,
            "first commit",
            &tree,
            &[],
        )
        .map_err(|e| e.to_string())?;
        repo.set_head("refs/heads/master").map_err(|e| e.to_string())?;
    }
    let remote_url = if config.knowledge_bases_remote_url.trim().is_empty() {
        normalize_remote_url(KNOWLEDGE_BASES_REMOTE_URL)
    } else {
        normalize_remote_url(config.knowledge_bases_remote_url.as_str())
    };

    if repo.find_remote("origin").is_err() {
        repo.remote("origin", &remote_url)
            .map_err(|e| e.to_string())?;
    } else if let Ok(origin) = repo.find_remote("origin") {
        let cur = origin.url().unwrap_or_default();
        if cur.trim() != remote_url.as_str() {
            repo.remote_set_url("origin", remote_url.as_str())
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DatabaseConfig {
    url: String,
    username: String,
    password: String,
}

fn default_appearance_theme() -> String {
    "light".to_string()
}

fn default_appearance_font_size() -> u8 {
    14
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppearanceConfig {
    #[serde(default = "default_appearance_theme")]
    theme: String,
    #[serde(default = "default_appearance_font_size")]
    font_size: u8,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        AppearanceConfig {
            theme: default_appearance_theme(),
            font_size: default_appearance_font_size(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarkdownInstallStatus {
    pub installed: bool,
    pub expected_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocalDeepseekStatus {
    pub installed: bool,
    pub expected_path: String,
    pub llama_server_exists: bool,
    pub llama_server_path: String,
}

const LOCAL_DEEPSEEK_MODEL_FILENAME: &str = "DeepSeek-R1-0528-Qwen3-8B-IQ4_XS.gguf";

fn cleanup_modelscope_download_dir(local_dir: &Path) {
    let download_root = local_dir.join("_modelscope_download");
    if !download_root.exists() {
        return;
    }
    match fs::remove_dir_all(&download_root) {
        Ok(_) => {
            info!(path = download_root.to_string_lossy().as_ref(), "modelscope cache removed");
        }
        Err(e) => {
            warn!(path = download_root.to_string_lossy().as_ref(), error = %e, "modelscope cache cleanup failed");
        }
    }
}

fn run_modelscope_download(exe_dir: &Path, local_dir: &Path) -> Result<(), String> {
    let _ = bundled_resources::ensure_relative_dir("utils/python_libs")?;
    let marker_src_dir = exe_dir
        .join("utils")
        .join("MarkdownConvertor")
        .join("marker-1.10.1");
    if !marker_src_dir.exists() {
        return Err(
            "找不到 marker-1.10.1 源码目录，请检查是否存在：<软件目录>\\utils\\MarkdownConvertor\\marker-1.10.1"
                .to_string(),
        );
    }

    let venv_dir = marker_src_dir.join(".venv");
    let python_path = if cfg!(windows) {
        let candidates = [
            venv_dir.join("Scripts").join("python.exe"),
            venv_dir.join("python.exe"),
        ];
        let mut found = None;
        for p in candidates {
            if p.exists() {
                found = Some(p);
                break;
            }
        }
        found.ok_or_else(|| {
            format!(
                "找不到 Marker 虚拟环境中的 python，请先构建运行环境：{}",
                venv_dir.to_string_lossy()
            )
        })?
    } else {
        let p = venv_dir.join("bin").join("python");
        if !p.exists() {
            return Err(format!(
                "找不到 Marker 虚拟环境中的 python，请先构建运行环境：{}",
                venv_dir.to_string_lossy()
            ));
        }
        p
    };

    let script_path = exe_dir.join("scripts").join("modelscope_download.py");
    if !script_path.is_file() {
        return Err(format!(
            "modelscope 下载脚本不存在: {}",
            script_path.to_string_lossy()
        ));
    }

    let local_dir_s = local_dir.to_string_lossy().to_string();

    info!(
        python = python_path.to_string_lossy().as_ref(),
        script = script_path.to_string_lossy().as_ref(),
        local_dir = local_dir_s.as_str(),
        "modelscope download start"
    );

    let output = Command::new(&python_path)
        .arg(script_path)
        .arg("--model-id")
        .arg("unsloth/DeepSeek-R1-0528-Qwen3-8B-GGUF")
        .arg("--filename")
        .arg(LOCAL_DEEPSEEK_MODEL_FILENAME)
        .arg("--local-dir")
        .arg(&local_dir_s)
        .output()
        .map_err(|e| format!("modelscope python launch failed: {}", e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(format!(
        "modelscope python exit={} stderr={} stdout={}",
        output.status, stderr, stdout
    ))
}

impl Default for AppConfig {
    fn default() -> Self {
        // 获取当前程序所在路径
        let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
        let exe_dir = exe_path
            .parent()
            .expect("无法获取可执行文件所在目录")
            .to_path_buf();

        // 设置 libraries_repo_path 为当前程序所在路径下的 Libraries 目录
        let libraries_repo_path = exe_dir.join("Libraries").to_string_lossy().to_string();

        AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            database: DatabaseConfig {
                url: "http://localhost".to_string(),
                username: "admin".to_string(),
                password: "password".to_string(),
            },
            appearance: AppearanceConfig {
                theme: "light".to_string(),
                font_size: 14,
            },
            libraries_repo_config: LibraryConfig {
                libraries_repo_path: libraries_repo_path,
                library: vec![],
            },
            active_repo_path: None,
            repopathlist: Vec::new(),
            libraries_root: "".to_string(),
            image_bed_root: default_image_bed_root(),
            resource_repo_sync: ResourceRepoSyncConfig::default(),
            life_album_album_dir: "".to_string(),
            template_repo_path: "".to_string(),
            slidev_repo_root: "".to_string(),
            chat_record_root: default_chat_record_root(),
            knowledge_bases_root: "".to_string(),
            knowledge_bases_remote_url: "".to_string(),
            knowledge_bases_remote_username: "".to_string(),
            knowledge_bases_remote_token: "".to_string(),
            knowledge_bases_git_username: "".to_string(),
            knowledge_bases_git_email: "".to_string(),
            device_id: Uuid::new_v4().to_string(),
            gc_cycle_seconds: default_gc_cycle_seconds(),
            lsp_syntax_highlight_enabled: false,
            lsp_github_token: "".to_string(),
            webrtc_signal_url: "".to_string(),
            llm_config: LlmConfig::default(),
            tts_config: TtsConfig::default(),
            asr_config: AsrConfig::default(),
        }
    }
}

pub fn init_library_config() -> Result<(), String> {
    // 获取当前程序所在路径
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();

    let libraries_dir = exe_dir.join("Libraries");
    if !libraries_dir.exists() {
        fs::create_dir_all(&libraries_dir).expect("无法创建 Libraries 目录");
    }
    // info!("init libraries successfully libraries_patch: {:?}", libraries_dir);
    Ok(())
}

pub fn get_config_path() -> PathBuf {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();

    let config_dir = exe_dir.join("config");
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).expect("无法创建 config 目录");
    }

    config_dir.join("guaxi_config.json")
}

pub fn load_config() -> AppConfig {
    const ENC_PREFIX: &str = "enc:v1:";
    const KEYRING_SERVICE: &str = "com.guaxi.guaxi";
    const KEYRING_USER_PREFIX: &str = "guaxi_config_master_key:";

    let config_path = get_config_path();
    info!(
        config_path = config_path.to_string_lossy().as_ref(),
        "loading guaxi_config.json"
    );

    let mut needs_save = false;
    let mut needs_migrate = false;

    if !config_path.exists() {
        let mut default_config = AppConfig::default();
        if default_config.device_id.trim().is_empty() {
            default_config.device_id = Uuid::new_v4().to_string();
        }

        save_config(&default_config);

        info!(
            config_path = config_path.to_string_lossy().as_ref(),
            "guaxi_config.json created with default config"
        );
        return default_config;
    }

    let config_data = fs::read_to_string(&config_path).expect("无法读取配置文件");
    let mut v: serde_json::Value = serde_json::from_str(&config_data).expect("无法解析配置文件");

    let device_id = v
        .pointer("/device_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let device_id = if device_id.is_empty() {
        let new_id = Uuid::new_v4().to_string();
        v["device_id"] = serde_json::Value::String(new_id.clone());
        needs_save = true;
        warn!("device_id is empty in config, regenerated a new one");
        new_id
    } else {
        device_id
    };

    let mut cached_key: Option<[u8; 32]> = None;
    let mut get_master_key = || -> Result<[u8; 32], String> {
        if let Some(k) = cached_key {
            return Ok(k);
        }

        let user = format!("{}{}", KEYRING_USER_PREFIX, device_id);
        let entry = Entry::new(KEYRING_SERVICE, &user).map_err(|e| e.to_string())?;

        let key_b64 = match entry.get_password() {
            Ok(s) => s,
            Err(_) => {
                let mut k = [0u8; 32];
                let mut rng = OsRng;
                if let Err(e) = rng.try_fill_bytes(&mut k) {
                    warn!(error = %e, "generate master key failed");
                    return Err(format!("generate master key failed: {}", e));
                }

                let b64 = general_purpose::STANDARD.encode(k);
                entry
                    .set_password(&b64)
                    .map_err(|e| format!("store master key failed: {}", e))?;
                info!("master key generated and stored in system keyring");
                b64
            }
        };

        let raw = general_purpose::STANDARD
            .decode(key_b64.trim())
            .map_err(|e| format!("invalid master key in keyring: {}", e))?;
        if raw.len() != 32 {
            return Err(format!("invalid master key length: {}", raw.len()));
        }

        let mut out = [0u8; 32];
        out.copy_from_slice(&raw);
        cached_key = Some(out);
        Ok(out)
    };

    // 标记明文字段（用于首次升级自动迁移）
    for ptr in [
        "/database/password",
        "/resource_repo_sync/webdav/password",
        "/resource_repo_sync/syncthing/api_key",
        "/llm_config/chatgpt_api_key",
        "/llm_config/deepseek_api_key",
        "/llm_config/qwen_api_key",
        "/llm_config/kimi_api_key",
        "/llm_config/doubao_api_key",
        "/llm_config/siliconflow_api_key",
        "/knowledge_bases_remote_token",
        "/lsp_github_token",
    ] {
        if let Some(s) = v.pointer(ptr).and_then(|x| x.as_str()) {
            let t = s.trim();
            if !t.is_empty() && !t.starts_with(ENC_PREFIX) {
                needs_migrate = true;
            }
        }
    }

    let mut try_decrypt = |ptr: &str| {
        let Some(slot) = v.pointer_mut(ptr) else {
            return;
        };
        let Some(raw) = slot.as_str().map(|s| s.to_string()) else {
            return;
        };

        let t = raw.trim();
        if t.is_empty() || !t.starts_with(ENC_PREFIX) {
            return;
        }

        let enc = t.trim_start_matches(ENC_PREFIX);
        let blob = match general_purpose::STANDARD.decode(enc) {
            Ok(b) => b,
            Err(e) => {
                warn!(ptr = ptr, error = %e, "decode encrypted field failed");
                return;
            }
        };
        if blob.len() <= 12 {
            warn!(ptr = ptr, blob_len = blob.len(), "encrypted blob too short");
            return;
        }

        let key = match get_master_key() {
            Ok(k) => k,
            Err(e) => {
                warn!(ptr = ptr, error = e.as_str(), "load master key failed");
                return;
            }
        };

        // enc:v1:BASE64( nonce(12B) || ciphertext+tag )
        let cipher = match Aes256Gcm::new_from_slice(&key) {
            Ok(c) => c,
            Err(e) => {
                warn!(ptr = ptr, error = %e, "init aes-256-gcm cipher failed");
                return;
            }
        };
        let nonce = Nonce::from_slice(&blob[..12]);
        let ciphertext = &blob[12..];

        let plain = match cipher.decrypt(nonce, ciphertext) {
            Ok(p) => p,
            Err(_) => {
                warn!(ptr = ptr, "decrypt encrypted field failed");
                return;
            }
        };
        let plain = match String::from_utf8(plain) {
            Ok(s) => s,
            Err(e) => {
                warn!(ptr = ptr, error = %e, "decrypt output is not utf-8");
                return;
            }
        };

        *slot = serde_json::Value::String(plain);
    };

    // 解密密文字段（内存中使用明文；落盘仍由 save_config 统一加密）
    try_decrypt("/database/password");
    try_decrypt("/resource_repo_sync/webdav/password");
    try_decrypt("/resource_repo_sync/syncthing/api_key");
    try_decrypt("/llm_config/chatgpt_api_key");
    try_decrypt("/llm_config/deepseek_api_key");
    try_decrypt("/llm_config/qwen_api_key");
    try_decrypt("/llm_config/kimi_api_key");
    try_decrypt("/llm_config/doubao_api_key");
    try_decrypt("/llm_config/siliconflow_api_key");
    try_decrypt("/knowledge_bases_remote_token");
    try_decrypt("/lsp_github_token");

    let mut config: AppConfig = serde_json::from_value(v).expect("无法解析配置文件");

    if config.device_id.trim().is_empty() {
        config.device_id = Uuid::new_v4().to_string();
        needs_save = true;
    }

    if needs_save || needs_migrate {
        info!(needs_save, needs_migrate, "migrating config sensitive fields to encrypted form");
        save_config(&config);
    }

    config
}

pub fn save_config(config: &AppConfig) {
    const ENC_PREFIX: &str = "enc:v1:";
    const KEYRING_SERVICE: &str = "com.guaxi.guaxi";
    const KEYRING_USER_PREFIX: &str = "guaxi_config_master_key:";

    let config_path = get_config_path();

    let device_id = config.device_id.trim();
    let device_id = if device_id.is_empty() { "default" } else { device_id };

    let mut cached_key: Option<[u8; 32]> = None;
    let mut get_master_key = || -> Result<[u8; 32], String> {
        if let Some(k) = cached_key {
            return Ok(k);
        }

        let user = format!("{}{}", KEYRING_USER_PREFIX, device_id);
        let entry = Entry::new(KEYRING_SERVICE, &user).map_err(|e| e.to_string())?;

        let key_b64 = match entry.get_password() {
            Ok(s) => s,
            Err(_) => {
                let mut k = [0u8; 32];
                let mut rng = OsRng;
                if let Err(e) = rng.try_fill_bytes(&mut k) {
                    warn!(error = %e, "generate master key failed");
                    return Err(format!("generate master key failed: {}", e));
                }

                let b64 = general_purpose::STANDARD.encode(k);
                entry
                    .set_password(&b64)
                    .map_err(|e| format!("store master key failed: {}", e))?;
                info!("master key generated and stored in system keyring");
                b64
            }
        };

        let raw = general_purpose::STANDARD
            .decode(key_b64.trim())
            .map_err(|e| format!("invalid master key in keyring: {}", e))?;
        if raw.len() != 32 {
            return Err(format!("invalid master key length: {}", raw.len()));
        }

        let mut out = [0u8; 32];
        out.copy_from_slice(&raw);
        cached_key = Some(out);
        Ok(out)
    };

    let mut encrypt_string = |plain: &str| -> Result<String, String> {
        let t = plain.trim();
        if t.is_empty() {
            return Ok("".to_string());
        }
        if t.starts_with(ENC_PREFIX) {
            return Ok(t.to_string());
        }

        let key = get_master_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("init aes-256-gcm cipher failed: {}", e))?;

        let mut nonce_bytes = [0u8; 12];
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut nonce_bytes)
            .map_err(|e| format!("generate nonce failed: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        // enc:v1:BASE64( nonce(12B) || ciphertext+tag )
        let ciphertext = cipher
            .encrypt(nonce, t.as_bytes())
            .map_err(|_| "encrypt failed".to_string())?;

        let mut blob: Vec<u8> = Vec::with_capacity(12 + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);

        Ok(format!(
            "{}{}",
            ENC_PREFIX,
            general_purpose::STANDARD.encode(blob)
        ))
    };

    let mut encrypted_fields = 0u32;
    let mut encrypt_field = |val: &mut serde_json::Value, ptr: &str| {
        let Some(slot) = val.pointer_mut(ptr) else {
            return;
        };
        let Some(cur) = slot.as_str().map(|s| s.to_string()) else {
            return;
        };

        match encrypt_string(&cur) {
            Ok(out) => {
                if !out.is_empty() && out != cur {
                    encrypted_fields += 1;
                }
                *slot = serde_json::Value::String(out);
            }
            Err(e) => {
                warn!(ptr = ptr, error = e.as_str(), "encrypt config field failed; keep plaintext");
            }
        }
    };

    let mut val = serde_json::to_value(config).unwrap();
    encrypt_field(&mut val, "/database/password");
    encrypt_field(&mut val, "/resource_repo_sync/webdav/password");
    encrypt_field(&mut val, "/resource_repo_sync/syncthing/api_key");
    encrypt_field(&mut val, "/llm_config/chatgpt_api_key");
    encrypt_field(&mut val, "/llm_config/deepseek_api_key");
    encrypt_field(&mut val, "/llm_config/qwen_api_key");
    encrypt_field(&mut val, "/llm_config/kimi_api_key");
    encrypt_field(&mut val, "/llm_config/doubao_api_key");
    encrypt_field(&mut val, "/llm_config/siliconflow_api_key");
    encrypt_field(&mut val, "/knowledge_bases_remote_token");
    encrypt_field(&mut val, "/lsp_github_token");

    let json = serde_json::to_string_pretty(&val).unwrap();
    fs::write(&config_path, json).expect("无法写入配置文件");

    info!(
        config_path = config_path.to_string_lossy().as_ref(),
        encrypted_fields,
        "guaxi_config.json saved"
    );
}

#[tauri::command]
pub fn get_gc_cycle_seconds() -> u64 {
    let config = load_config();
    config.gc_cycle_seconds
}

#[tauri::command]
pub fn set_gc_cycle_seconds(seconds: u64) {
    let mut config = load_config();
    config.gc_cycle_seconds = seconds;
    save_config(&config);
}

#[tauri::command]
pub fn get_lsp_syntax_highlight_enabled() -> bool {
    let config = load_config();
    config.lsp_syntax_highlight_enabled
}

#[tauri::command]
pub fn set_lsp_syntax_highlight_enabled(enabled: bool) {
    let mut config = load_config();
    config.lsp_syntax_highlight_enabled = enabled;
    save_config(&config);
}

#[tauri::command]
pub fn get_lsp_github_token() -> String {
    let config = load_config();
    config.lsp_github_token
}

#[tauri::command]
pub fn set_lsp_github_token(token: String) {
    let mut config = load_config();
    config.lsp_github_token = token.trim().to_string();
    save_config(&config);
}

#[tauri::command]
pub async fn get_lsp_server_statuses() -> Result<Vec<lsp::LspServerStatus>, String> {
    Ok(lsp::get_lsp_server_statuses().await)
}

#[tauri::command]
pub async fn install_lsp_server(language: String) -> Result<lsp::LspServerStatus, String> {
    let lang = language.trim().to_string();
    if lang.is_empty() {
        return Err("language 不能为空".to_string());
    }
    let config = load_config();
    let token = config.lsp_github_token.trim().to_string();
    let token = if token.is_empty() { None } else { Some(token) };
    lsp::install_lsp_server(&lang, token).await
}

#[tauri::command]
pub fn get_webrtc_signal_url() -> String {
    let config = load_config();
    config.webrtc_signal_url
}

#[tauri::command]
pub fn set_webrtc_signal_url(url: String) {
    let next = url.trim().to_string();
    info!(url = next.as_str(), "set_webrtc_signal_url");

    let mut config = load_config();
    config.webrtc_signal_url = next;
    save_config(&config);
}

#[tauri::command]
pub fn get_llm_config() -> LlmConfig {
    let config = load_config();
    config.llm_config
}

#[tauri::command]
pub fn set_llm_config(llm_config: LlmConfig) {
    let mut config = load_config();
    config.llm_config = llm_config;
    save_config(&config);
}

#[tauri::command]
pub fn get_tts_config() -> TtsConfig {
    let config = load_config();
    config.tts_config
}

#[tauri::command]
pub fn set_tts_config(tts_config: TtsConfig) {
    let mut config = load_config();
    config.tts_config = TtsConfig {
        provider: tts_config.provider.trim().to_string(),
        model: tts_config.model.trim().to_string(),
        tokens: tts_config.tokens.trim().to_string(),
        exe: tts_config.exe.trim().to_string(),
        output_dir: tts_config.output_dir.trim().to_string(),
    };
    if config.tts_config.provider.is_empty() {
        config.tts_config.provider = default_tts_provider();
    }
    if config.tts_config.model.is_empty() {
        config.tts_config.model = default_tts_model();
    }
    if config.tts_config.tokens.is_empty() {
        config.tts_config.tokens = default_tts_tokens();
    }
    if config.tts_config.exe.is_empty() {
        config.tts_config.exe = default_tts_exe();
    }
    if config.tts_config.output_dir.is_empty() {
        config.tts_config.output_dir = default_tts_output_dir();
    }
    let output_dir = PathBuf::from(&config.tts_config.output_dir);
    if !output_dir.exists() {
        let _ = fs::create_dir_all(&output_dir);
    }
    save_config(&config);
}

#[tauri::command]
pub fn get_asr_config() -> AsrConfig {
    let config = load_config();
    config.asr_config
}

#[tauri::command]
pub fn set_asr_config(asr_config: AsrConfig) {
    let mut config = load_config();
    config.asr_config = AsrConfig {
        provider: asr_config.provider.trim().to_string(),
        model: asr_config.model.trim().to_string(),
        exe: asr_config.exe.trim().to_string(),
    };
    if config.asr_config.provider.is_empty() {
        config.asr_config.provider = default_asr_provider();
    }
    if config.asr_config.model.is_empty() {
        config.asr_config.model = default_asr_model();
    }
    if config.asr_config.exe.is_empty() {
        config.asr_config.exe = default_asr_exe();
    }
    save_config(&config);
}

#[tauri::command]
pub fn get_chat_record_root() -> String {
    if let Err(e) = init_chat_record_root() {
        warn!(error = e.as_str(), "init_chat_record_root failed");
    }
    let config = load_config();
    config.chat_record_root
}

#[tauri::command]
pub fn set_chat_record_root(path: String) -> Result<(), String> {
    let mut root_path = PathBuf::from(path);
    if root_path.as_os_str().is_empty() {
        root_path = PathBuf::from(default_chat_record_root());
    }

    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }

    let mut config = load_config();
    config.chat_record_root = root_path.to_string_lossy().to_string();
    save_config(&config);
    Ok(())
}

fn exe_dir() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())
}

/*
fn build_tts_cuda_diagnostic() -> Result<TtsCudaDiagnostic, String> {
    let config = get_tts_config();
    let exe_root = exe_dir()?;
    let sherpa_exe_raw = if config.exe.trim().is_empty() {
        default_tts_exe()
    } else {
        config.exe.trim().to_string()
    };
    let sherpa_exe = resolve_path_from_exe_dir(&exe_root, sherpa_exe_raw.as_str());
    let sherpa_dir = sherpa_exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| exe_root.join("utils").join("sherpa-onnx"));
    let provider_dll = sherpa_dir.join("onnxruntime_providers_cuda.dll");

    let dependency_names = detect_cuda_dependency_names(&provider_dll);

    let search_roots = vec![sherpa_dir.clone(), exe_root.clone()];
    let mut dependencies = Vec::new();
    let mut missing_dependencies = Vec::new();
    for name in &dependency_names {
        let found = find_binary_in_search_paths(name.as_str(), &search_roots);
        let status = BinaryDependencyStatus {
            name: name.clone(),
            found: found.is_some(),
            path: found
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        };
        if !status.found {
            missing_dependencies.push(status.name.clone());
        }
        dependencies.push(status);
    }

    let provider_dll_exists = provider_dll.is_file();
    let (provider_dll_loadable, provider_dll_load_error) = if provider_dll_exists {
        try_load_dynamic_library(&provider_dll)
    } else {
        (false, "未找到 onnxruntime_providers_cuda.dll".to_string())
    };

    let can_use_cuda = provider_dll_exists && provider_dll_loadable && missing_dependencies.is_empty();
    let summary = if can_use_cuda {
        "CUDA 运行库自检通过，sherpa-onnx 应可尝试使用 GPU / CUDA。".to_string()
    } else if !provider_dll_exists {
        format!("未找到 CUDA provider DLL: {}", provider_dll.to_string_lossy())
    } else if !missing_dependencies.is_empty() {
        format!("CUDA 运行库缺失: {}", missing_dependencies.join(", "))
    } else if !provider_dll_load_error.is_empty() {
        format!("CUDA provider DLL 无法加载: {}", provider_dll_load_error)
    } else {
        "CUDA provider 当前不可用。".to_string()
    };
    let recommendation = if can_use_cuda {
        "可以保留 CUDA；如果实际合成仍失败，建议查看日志中的 provider 初始化信息。".to_string()
    } else if !missing_dependencies.is_empty() {
        "当前 sherpa/onnxruntime CUDA 包需要匹配版本的 CUDA 11.x 与 cuDNN 8 DLL；补齐这些 DLL 或改用与本机 CUDA 12.x 匹配的包。".to_string()
    } else {
        "建议先切换到 CPU，或更换与当前系统 CUDA 运行库匹配的 sherpa/onnxruntime CUDA 包。".to_string()
    };

    Ok(TtsCudaDiagnostic {
        checked: true,
        provider: "cuda".to_string(),
        can_use_cuda,
        summary,
        recommendation,
        sherpa_exe: sherpa_exe.to_string_lossy().to_string(),
        provider_dll: provider_dll.to_string_lossy().to_string(),
        provider_dll_exists,
        provider_dll_loadable,
        provider_dll_load_error,
        dependencies,
        missing_dependencies,
    })
}

fn resolve_path_from_exe_dir(exe_root: &Path, raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return exe_root.join(default_tts_exe());
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        path
    } else {
        exe_root.join(path)
    }
}

fn detect_cuda_dependency_names(provider_dll: &Path) -> Vec<String> {
    let detected = extract_cuda_dependency_names_from_binary(provider_dll);
    if !detected.is_empty() {
        return detected;
    }

    #[cfg(target_os = "windows")]
    {
        return vec![
            "cublas64_11.dll".to_string(),
            "cublasLt64_11.dll".to_string(),
            "cudart64_110.dll".to_string(),
            "cudnn64_8.dll".to_string(),
            "cufft64_10.dll".to_string(),
        ];
    }

    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

fn extract_cuda_dependency_names_from_binary(provider_dll: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(provider_dll) else {
        return Vec::new();
    };

    let mut current = Vec::new();
    let mut out = Vec::new();
    for byte in bytes {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
        } else {
            maybe_push_cuda_dependency(&mut out, &current);
            current.clear();
        }
    }
    maybe_push_cuda_dependency(&mut out, &current);
    out
}

fn maybe_push_cuda_dependency(out: &mut Vec<String>, raw: &[u8]) {
    if raw.len() < 8 {
        return;
    }

    let Ok(text) = std::str::from_utf8(raw) else {
        return;
    };
    let trimmed = text.trim_matches(char::from(0)).trim();
    if trimmed.is_empty() || !trimmed.to_ascii_lowercase().ends_with(".dll") {
        return;
    }

    let lower = trimmed.to_ascii_lowercase();
    let is_cuda_dep = lower.starts_with("cublas64_")
        || lower.starts_with("cublaslt64_")
        || lower.starts_with("cudart64_")
        || lower.starts_with("cudnn64_")
        || lower.starts_with("cufft64_")
        || lower.starts_with("curand64_");
    if !is_cuda_dep {
        return;
    }

    if !out.iter().any(|existing| existing.eq_ignore_ascii_case(trimmed)) {
        out.push(trimmed.to_string());
    }
}

fn find_binary_in_search_paths(name: &str, search_roots: &[PathBuf]) -> Option<PathBuf> {
    for root in search_roots {
        let candidate = root.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Some(path_env) = env::var_os("PATH") {
        for entry in env::split_paths(&path_env) {
            let candidate = entry.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn try_load_dynamic_library(path: &Path) -> (bool, String) {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> *mut std::ffi::c_void;
        fn FreeLibrary(h_lib_module: *mut std::ffi::c_void) -> i32;
        fn GetLastError() -> u32;
    }

    let wide = OsStr::new(path.as_os_str())
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<u16>>();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        let err = unsafe { GetLastError() };
        return (false, format!("Windows LoadLibrary 失败，错误码={}", err));
    }
    unsafe {
        let _ = FreeLibrary(handle);
    }
    (true, String::new())
}

#[cfg(not(target_os = "windows"))]
fn try_load_dynamic_library(path: &Path) -> (bool, String) {
    if path.is_file() {
        (true, String::new())
    } else {
        (false, "动态库不存在".to_string())
    }
}

*/
fn remove_path_if_exists(p: &Path) -> Result<(), String> {
    if !p.exists() {
        return Ok(());
    }
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(f).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let out_path = target_dir.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }

        if let Some(p) = out_path.parent() {
            if !p.exists() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
        }

        let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;

    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ty.is_file() {
            if let Some(p) = dst_path.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
            }
            if dst_path.exists() {
                let _ = fs::remove_file(&dst_path);
            }
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn find_dir_by_name(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }

    if root
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s == name)
        .unwrap_or(false)
    {
        return Some(root.to_path_buf());
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }

        if p.file_name()
            .and_then(|s| s.to_str())
            .map(|s| s == name)
            .unwrap_or(false)
        {
            return Some(p);
        }

        if let Some(found) = find_dir_by_name(&p, name, max_depth - 1) {
            return Some(found);
        }
    }

    None
}

fn find_file_by_name(root: &Path, name: &str, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }

    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
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

fn local_deepseek_model_path(exe: &Path) -> PathBuf {
    exe.join("utils")
        .join("Models")
        .join("chat")
        .join(LOCAL_DEEPSEEK_MODEL_FILENAME)
}

fn resolve_llama_server_path(exe: &Path) -> Option<PathBuf> {
    let _ = bundled_resources::ensure_relative_dir("utils/llama");
    let llama_dir = exe.join("utils").join("llama");
    if !llama_dir.exists() {
        return None;
    }

    let candidates = if cfg!(windows) {
        vec!["llama-server.exe", "llama_server.exe"]
    } else {
        vec!["llama-server", "llama_server"]
    };

    for name in candidates {
        let direct = llama_dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }

        if let Some(found) = find_file_by_name(&llama_dir, name, 6) {
            return Some(found);
        }
    }

    None
}

#[tauri::command]
pub async fn get_markdown_models_status() -> Result<MarkdownInstallStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let exe = exe_dir()?;
        let expected = exe
            .join("utils")
            .join("Models")
            .join("surya_models");

        Ok(MarkdownInstallStatus {
            installed: expected.is_dir(),
            expected_path: expected.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_local_deepseek_status() -> Result<LocalDeepseekStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let exe = exe_dir()?;
        let expected = local_deepseek_model_path(&exe);
        let llama_server_path = resolve_llama_server_path(&exe);

        Ok(LocalDeepseekStatus {
            installed: expected.is_file(),
            expected_path: expected.to_string_lossy().to_string(),
            llama_server_exists: llama_server_path
                .as_ref()
                .map(|p| p.is_file())
                .unwrap_or(false),
            llama_server_path: llama_server_path
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn download_local_deepseek_model() -> Result<String, String> {
    let exe = exe_dir()?;
    let model_path = local_deepseek_model_path(&exe);

    if model_path.is_file() {
        if let Some(parent) = model_path.parent() {
            cleanup_modelscope_download_dir(parent);
        }
        info!(path = model_path.to_string_lossy().as_ref(), "local deepseek model exists, skip download");
        return Ok(model_path.to_string_lossy().to_string());
    }

    let parent = model_path
        .parent()
        .ok_or("invalid model path".to_string())?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;

    let exe_for_modelscope = exe.clone();
    let parent_for_modelscope = parent.clone();
    let modelscope_result = tauri::async_runtime::spawn_blocking(move || {
        run_modelscope_download(&exe_for_modelscope, &parent_for_modelscope)
    })
    .await
    .map_err(|e| e.to_string())?;

    if modelscope_result.is_ok() && model_path.is_file() {
        cleanup_modelscope_download_dir(&parent);
        return Ok(model_path.to_string_lossy().to_string());
    }

    Err(format!(
        "download local deepseek model failed. modelscope_result={:?}. You can manually import GGUF via install_local_deepseek_model_from_file(source_path). target={}",
        modelscope_result.err(),
        model_path.to_string_lossy()
    ))
}

#[tauri::command]
pub async fn install_local_deepseek_model_from_file(source_path: String) -> Result<String, String> {
    let src = PathBuf::from(source_path.trim());
    if !src.is_file() {
        return Err(format!(
            "source file not found: {}",
            src.to_string_lossy()
        ));
    }

    let ext_ok = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !ext_ok {
        return Err("source file must be a .gguf model file".to_string());
    }

    let exe = exe_dir()?;
    let model_path = local_deepseek_model_path(&exe);
    let parent = model_path
        .parent()
        .ok_or("invalid model path".to_string())?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;

    let tmp_path = model_path.with_extension("copying");
    let _ = fs::remove_file(&tmp_path);
    fs::copy(&src, &tmp_path).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, &model_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        e.to_string()
    })?;

    Ok(model_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_markdown_convertor_env_status() -> Result<MarkdownInstallStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let exe = exe_dir()?;
        let expected = exe
            .join("utils")
            .join("MarkdownConvertor")
            .join("marker-1.10.1");

        Ok(MarkdownInstallStatus {
            installed: expected.is_dir(),
            expected_path: expected.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_markdown_models_archive(archive_path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = archive_path.trim();
        if p.is_empty() {
            return Err("archive_path is empty".to_string());
        }

        let zip_path = PathBuf::from(p);
        if !zip_path.is_file() {
            return Err(format!("找不到模型压缩包: {}", zip_path.to_string_lossy()));
        }

        if !zip_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            return Err("模型压缩包仅支持 .zip".to_string());
        }

        let exe = exe_dir()?;
        let utils_dir = exe.join("utils");
        let models_dir = utils_dir.join("Models");
        fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

        let temp_dir = exe
            .join("tmp")
            .join(format!("surya_models_{}", Uuid::new_v4()));
        remove_path_if_exists(&temp_dir)?;
        fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

        let result: Result<String, String> = (|| {
            info!(
                archive_path = zip_path.to_string_lossy().as_ref(),
                temp_dir = temp_dir.to_string_lossy().as_ref(),
                "extracting markdown models archive"
            );

            extract_zip_to_dir(&zip_path, &temp_dir)?;

            let packaged_surya_models = find_dir_by_name(&temp_dir, "surya_models", 5)
                .ok_or_else(|| "压缩包内未找到 surya_models 目录".to_string())?;

            let dst_surya_models = models_dir.join("surya_models");
            remove_path_if_exists(&dst_surya_models)?;
            copy_dir_recursive(&packaged_surya_models, &dst_surya_models)?;

            if !dst_surya_models.is_dir() {
                return Err(format!(
                    "模型目录校验失败: {}",
                    dst_surya_models.to_string_lossy()
                ));
            }

            Ok(dst_surya_models.to_string_lossy().to_string())
        })();

        let _ = remove_path_if_exists(&temp_dir);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_markdown_convertor_env_archive(archive_path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = archive_path.trim();
        if p.is_empty() {
            return Err("archive_path is empty".to_string());
        }

        let zip_path = PathBuf::from(p);
        if !zip_path.is_file() {
            return Err(format!("找不到运行环境压缩包: {}", zip_path.to_string_lossy()));
        }

        if !zip_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            return Err("运行环境压缩包仅支持 .zip".to_string());
        }

        let exe = exe_dir()?;
        let utils_dir = exe.join("utils");
        fs::create_dir_all(&utils_dir).map_err(|e| e.to_string())?;

        let temp_dir = exe
            .join("tmp")
            .join(format!("markdown_convertor_env_{}", Uuid::new_v4()));
        remove_path_if_exists(&temp_dir)?;
        fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

        let result: Result<String, String> = (|| {
            info!(
                archive_path = zip_path.to_string_lossy().as_ref(),
                temp_dir = temp_dir.to_string_lossy().as_ref(),
                "extracting markdown convertor env archive"
            );

            extract_zip_to_dir(&zip_path, &temp_dir)?;

            let packaged_convertor_dir = find_dir_by_name(&temp_dir, "MarkdownConvertor", 5)
                .ok_or_else(|| "压缩包内未找到 MarkdownConvertor 目录".to_string())?;

            let dst_convertor_dir = utils_dir.join("MarkdownConvertor");
            remove_path_if_exists(&dst_convertor_dir)?;
            copy_dir_recursive(&packaged_convertor_dir, &dst_convertor_dir)?;

            let expected = dst_convertor_dir.join("marker-1.10.1");
            if !expected.is_dir() {
                return Err(format!(
                    "运行环境目录校验失败: {}",
                    expected.to_string_lossy()
                ));
            }

            Ok(expected.to_string_lossy().to_string())
        })();

        let _ = remove_path_if_exists(&temp_dir);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_active_repo_path() -> Option<String> {
    let config = load_config();
    config.active_repo_path
}

#[tauri::command]
pub fn set_active_repo_path(path: String) {
    let mut config = load_config();
    config.active_repo_path = Some(path);
    save_config(&config);
}

#[tauri::command]
pub fn get_server_config() -> ServerConfig {
    let config = load_config();
    config.server
}

#[tauri::command]
pub fn set_server_config(host: String, port: u16) {
    let mut config = load_config();
    config.server = ServerConfig { host, port };
    save_config(&config);
}

#[tauri::command]
pub fn get_database_config() -> DatabaseConfig {
    let config = load_config();
    config.database
}

#[tauri::command]
pub fn set_database_config(url: String, username: String, password: String) {
    let mut config = load_config();
    config.database = DatabaseConfig {
        url,
        username,
        password,
    };
    save_config(&config);
}

#[tauri::command]
pub fn get_appearance_config() -> AppearanceConfig {
    let config = load_config();
    config.appearance
}

#[tauri::command]
pub fn set_appearance_config(theme: String, font_size: u8) {
    let raw_theme = theme.trim().to_lowercase();
    let theme = if raw_theme == "dark" { "dark" } else { "light" }.to_string();
    let font_size = if font_size == 0 {
        default_appearance_font_size()
    } else {
        font_size
    };

    info!(
        theme = theme.as_str(),
        font_size = font_size,
        "set_appearance_config"
    );

    let mut config = load_config();
    config.appearance = AppearanceConfig { theme, font_size };
    save_config(&config);
}

#[tauri::command]
pub fn get_libraries_repo_config() -> LibraryConfig {
    let config = load_config();
    config.libraries_repo_config
}

#[tauri::command]
pub fn set_libraries_repo_config(libraries_repo_path: String, library: Vec<Library>) {
    let mut config = load_config();
    config.libraries_repo_config = LibraryConfig {
        libraries_repo_path,
        library,
    };
    save_config(&config);
}

#[tauri::command]
pub fn get_repopathlist() -> Vec<String> {
    let config = load_config();
    info!("get_repopathlist: {:?}", config.repopathlist);
    config.repopathlist
}

#[tauri::command]
pub fn set_repopathlist(paths: Vec<String>) {
    let mut config = load_config();
    config.repopathlist = paths;
    save_config(&config);
}

#[tauri::command]
pub fn get_libraries_root() -> String {
    let config = load_config();
    config.libraries_root
}

#[tauri::command]
pub fn set_libraries_root(path: String) {
    let mut config = load_config();
    config.libraries_root = path;
    save_config(&config);
}

#[tauri::command]
pub fn get_image_bed_root() -> String {
    let config = load_config();
    config.image_bed_root
}

#[tauri::command]
pub fn set_image_bed_root(path: String) {
    let mut config = load_config();
    config.image_bed_root = path;
    save_config(&config);
}

#[tauri::command]
pub fn get_resource_repo_sync_config() -> ResourceRepoSyncConfig {
    let config = load_config();
    config.resource_repo_sync
}

#[tauri::command]
pub fn set_resource_repo_sync_config(config_input: ResourceRepoSyncConfig) {
    let mut config = load_config();
    config.resource_repo_sync = config_input;
    save_config(&config);
}

#[tauri::command]
pub fn get_life_album_album_dir() -> String {
    let config = load_config();
    config.life_album_album_dir
}

#[tauri::command]
pub fn set_life_album_album_dir(path: String) -> Result<(), String> {
    let p = PathBuf::from(path.trim());
    if p.as_os_str().is_empty() {
        let mut config = load_config();
        config.life_album_album_dir = "".to_string();
        save_config(&config);
        return Ok(());
    }

    if !p.is_dir() {
        return Err("路径不是目录".to_string());
    }

    let mut config = load_config();
    config.life_album_album_dir = p.to_string_lossy().to_string();
    save_config(&config);
    Ok(())
}

#[tauri::command]
pub fn get_template_repo_path() -> String {
    let mut config = load_config();
    if config.template_repo_path.trim().is_empty() {
        config.template_repo_path = default_template_repo_path();
        save_config(&config);
    }
    config.template_repo_path
}

#[tauri::command]
pub fn set_template_repo_path(path: String) -> Result<(), String> {
    let mut config = load_config();
    config.template_repo_path = path;
    save_config(&config);
    init_template_root()
}

#[tauri::command]
pub fn get_slidev_repo_root() -> String {
    if let Err(e) = init_slidev_repo_root() {
        info!("init_slidev_repo_root failed: {}", e);
    }
    let config = load_config();
    config.slidev_repo_root
}

#[tauri::command]
pub fn set_slidev_repo_root(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    let root_path = if trimmed.is_empty() {
        PathBuf::from(default_slidev_repo_root())
    } else {
        PathBuf::from(trimmed)
    };

    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }

    let mut config = load_config();
    config.slidev_repo_root = root_path.to_string_lossy().to_string();
    save_config(&config);
    Ok(())
}

#[derive(Serialize)]
pub struct KnowledgeBasesRemoteConfig {
    pub remote_url: String,
    pub username: String,
    pub has_token: bool,
    pub git_username: String,
    pub git_email: String,
}

#[tauri::command]
pub fn get_knowledge_bases_remote_config() -> KnowledgeBasesRemoteConfig {
    let mut config = load_config();

    let normalized_remote_url = normalize_remote_url(config.knowledge_bases_remote_url.as_str());
    if normalized_remote_url != config.knowledge_bases_remote_url {
        config.knowledge_bases_remote_url = normalized_remote_url;
        save_config(&config);
    }

    info!(
        remote_url = config.knowledge_bases_remote_url.as_str(),
        git_username = config.knowledge_bases_git_username.as_str(),
        git_email = config.knowledge_bases_git_email.as_str(),
        "get_knowledge_bases_remote_config"
    );
    KnowledgeBasesRemoteConfig {
        remote_url: config.knowledge_bases_remote_url,
        username: "".to_string(),
        has_token: false,
        git_username: config.knowledge_bases_git_username,
        git_email: config.knowledge_bases_git_email,
    }
}

#[tauri::command]
pub fn set_knowledge_bases_remote_config(
    remote_url: String,
    username: String,
    token: String,
    git_username: String,
    git_email: String,
) -> Result<(), String> {
    let mut config = load_config();

    let _ = username;
    let _ = token;

    let normalized_remote_url = normalize_remote_url(remote_url.as_str());
    let normalized_git_username = git_username.trim().to_string();
    let normalized_git_email = git_email.trim().to_string();

    info!(
        remote_url = normalized_remote_url.as_str(),
        git_username = normalized_git_username.as_str(),
        git_email = normalized_git_email.as_str(),
        "set_knowledge_bases_remote_config"
    );

    config.knowledge_bases_remote_url = normalized_remote_url;
    config.knowledge_bases_remote_username = "".to_string();
    config.knowledge_bases_remote_token = "".to_string();
    config.knowledge_bases_git_username = normalized_git_username;
    config.knowledge_bases_git_email = normalized_git_email;
    save_config(&config);

    let root = get_knowledge_bases_root();
    let root_path = PathBuf::from(root);
    schedule_knowledge_bases_git_init(root_path);
    Ok(())
}

pub fn is_knowledge_bases_root_path(path: &str) -> bool {
    let kb = get_knowledge_bases_root();
    let kb_canon = PathBuf::from(kb).canonicalize().ok();
    let path_canon = PathBuf::from(path).canonicalize().ok();
    match (kb_canon, path_canon) {
        (Some(a), Some(b)) => b.starts_with(a),
        _ => false,
    }
}


#[tauri::command]
pub fn get_knowledge_bases_root() -> String {
    if let Err(e) = init_knowledge_bases_root() {
        info!("init_knowledge_bases_root failed: {}", e);
    }
    let config = load_config();
    config.knowledge_bases_root
}

#[tauri::command]
pub fn set_knowledge_bases_root(path: String) -> Result<(), String> {
    let mut root_path = normalize_knowledge_bases_root(&path);
    if root_path.as_os_str().is_empty() {
        root_path = PathBuf::from(default_knowledge_bases_root());
    }

    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    let meta_path = root_path.join(".meta");
    if !meta_path.exists() {
        fs::create_dir_all(&meta_path).map_err(|e| e.to_string())?;
    }

    let mut config = load_config();
    config.knowledge_bases_root = root_path.to_string_lossy().to_string();
    save_config(&config);

    schedule_knowledge_bases_git_init(root_path);
    Ok(())
}

pub fn init_knowledge_bases_root() -> Result<(), String> {
    let mut config = load_config();
    let default_root = PathBuf::from(default_knowledge_bases_root());

    let configured = config.knowledge_bases_root.trim().to_string();
    let mut root_path = if configured.is_empty() {
        default_root.clone()
    } else {
        let normalized = normalize_knowledge_bases_root(&configured);
        if normalized.as_os_str().is_empty() {
            default_root.clone()
        } else {
            normalized
        }
    };

    if let Err(e) = fs::create_dir_all(&root_path) {
        warn!(error = e.to_string().as_str(), "create KnowledgeBases root failed, fallback to default");
        root_path = default_root.clone();
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }

    let meta_path = root_path.join(".meta");
    if !meta_path.exists() {
        fs::create_dir_all(&meta_path).map_err(|e| e.to_string())?;
    }

    let effective = root_path.to_string_lossy().to_string();
    if config.knowledge_bases_root.trim() != effective {
        config.knowledge_bases_root = effective;
        save_config(&config);
    }

    schedule_knowledge_bases_git_init(root_path);
    Ok(())
}

pub fn init_resource_root() -> Result<(), String> {
    let mut config = load_config();
    if config.image_bed_root.trim().is_empty() {
        config.image_bed_root = default_image_bed_root();
        save_config(&config);
    }
    let root_path = PathBuf::from(&config.image_bed_root);
    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn init_template_root() -> Result<(), String> {
    let mut config = load_config();
    if config.template_repo_path.trim().is_empty() {
        config.template_repo_path = default_template_repo_path();
        save_config(&config);
    }
    let root_path = PathBuf::from(&config.template_repo_path);
    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn init_slidev_repo_root() -> Result<(), String> {
    let mut config = load_config();
    if config.slidev_repo_root.trim().is_empty() {
        config.slidev_repo_root = default_slidev_repo_root();
        save_config(&config);
    }
    let root_path = PathBuf::from(&config.slidev_repo_root);
    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn init_chat_record_root() -> Result<(), String> {
    let mut config = load_config();
    if config.chat_record_root.trim().is_empty() {
        config.chat_record_root = default_chat_record_root();
        save_config(&config);
    }
    let root_path = PathBuf::from(&config.chat_record_root);
    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn get_device_id() -> String {
    let mut config = load_config();
    if config.device_id.is_empty() {
        config.device_id = Uuid::new_v4().to_string();
        save_config(&config);
    }
    config.device_id.clone()
}
