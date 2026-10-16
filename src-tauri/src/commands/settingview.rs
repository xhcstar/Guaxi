use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::guaxiservices::bundled_resources;
use crate::guaxiservices::lsp_extensions;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose, Engine as _};
use keyring::Entry;
use rand::rngs::OsRng;
use rand::TryRngCore;
use tracing::{debug, info, warn};
use uuid::Uuid;
use zip::ZipArchive;

use git2::{Repository, Signature};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn apply_hidden_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

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
    // 相册地名反查用的高德 Web 服务 key。留空则回退 Nominatim(OSM)。
    #[serde(default)]
    life_album_amap_key: String,
    #[serde(default)]
    template_repo_path: String,
    #[serde(default)]
    slidev_repo_root: String,
    #[serde(default = "default_chat_record_root")]
    chat_record_root: String,
    /// 文档转换（marker）的 OCR 策略：`auto` | `full` | `text_only`。
    /// 语义见 `convert::engine::apply_marker_ocr_policy`；设置页可切换。
    #[serde(default = "default_marker_ocr_mode")]
    marker_ocr_mode: String,
    /// 文档转换（marker）的阅读顺序策略：`layout` | `pdftext`。
    /// 语义见 `convert::engine::apply_marker_reading_order`；设置页可切换。
    #[serde(default = "default_marker_reading_order")]
    marker_reading_order: String,
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
    /// 被用户在「设置 → 服务器」停用的后端服务 id；重启应用后不再自动拉起。
    /// 供 `commands::service_control` 读取/写回。
    #[serde(default)]
    pub disabled_services: Vec<String>,
    #[serde(default)]
    lsp_syntax_highlight_enabled: bool,
    // 代码配色主题（编辑器语法色与 Markdown 代码块配色共用同一套）。取值是前端 codeThemes.js 里的主题 id。
    // 这里只当字符串存、不校验取值：主题清单属于前端资产，后端不认识具体 id 时由前端回退到默认主题。
    #[serde(default = "default_code_theme")]
    code_theme: String,
    // 探索者笔记本的「注释/自然语言→生成 Python 代码」AI 辅助开关。
    // 默认关闭:未配置云端模型时会回退本地大模型,低配机器上提示很慢,由用户显式开启。
    #[serde(default)]
    python_codegen_assist_enabled: bool,
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
    #[serde(default)]
    terminal_config: TerminalConfig,
    #[serde(default)]
    film_studio_config: FilmStudioConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    host: String,
    port: u16,
}

fn default_cc_switch_base_url() -> String {
    "http://127.0.0.1:15721/v1".to_string()
}

fn default_cc_switch_wire_api() -> String {
    "chat".to_string()
}

fn default_codex_sandbox() -> String {
    "workspace-write".to_string()
}

fn default_agent_engine() -> String {
    "codex".to_string()
}

#[derive(Serialize, Deserialize, Debug)]
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
    // 自定义 OpenAI 兼容供应商：base_url + api_key + model
    #[serde(default)]
    pub custom_base_url: String,
    #[serde(default)]
    pub custom_api_key: String,
    #[serde(default)]
    pub custom_model: String,
    // cc-switch 本地代理：供应商切换交给 cc-switch，Guaxi 只认这一个入口地址。
    // 这样在 cc-switch 里换供应商后，Guaxi 的普通对话与 codex Agent 会同时跟随，无需改配置。
    #[serde(default)]
    pub cc_switch_enabled: bool,
    #[serde(default = "default_cc_switch_base_url")]
    pub cc_switch_base_url: String,
    #[serde(default)]
    pub cc_switch_api_key: String,
    #[serde(default)]
    pub cc_switch_model: String,
    /// cc-switch 代理暴露的协议：`chat`（/chat/completions）或 `responses`（/responses）。
    #[serde(default = "default_cc_switch_wire_api")]
    pub cc_switch_wire_api: String,
    // Codex Agent 引擎
    #[serde(default)]
    pub codex_model: String,
    #[serde(default = "default_codex_sandbox")]
    pub codex_sandbox: String,
    /// Agent 引擎：`codex`（沙箱 codex exec）或 `builtin`（Python agent_server 回退）。
    #[serde(default = "default_agent_engine")]
    pub agent_engine: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            chatgpt_api_key: String::new(),
            deepseek_api_key: String::new(),
            qwen_api_key: String::new(),
            kimi_api_key: String::new(),
            doubao_api_key: String::new(),
            siliconflow_api_key: String::new(),
            custom_base_url: String::new(),
            custom_api_key: String::new(),
            custom_model: String::new(),
            cc_switch_enabled: false,
            cc_switch_base_url: default_cc_switch_base_url(),
            cc_switch_api_key: String::new(),
            cc_switch_model: String::new(),
            cc_switch_wire_api: default_cc_switch_wire_api(),
            codex_model: String::new(),
            codex_sandbox: default_codex_sandbox(),
            agent_engine: default_agent_engine(),
        }
    }
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
    /// 默认音色（Speaker ID）。请求里没显式给 voice 时用它，
    /// 这样各个朗读入口不必各自硬编码一个编号。
    #[serde(default = "default_tts_speaker_id")]
    pub speaker_id: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider: default_tts_provider(),
            model: default_tts_model(),
            tokens: default_tts_tokens(),
            exe: default_tts_exe(),
            output_dir: default_tts_output_dir(),
            speaker_id: default_tts_speaker_id(),
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

fn default_terminal_default_profile() -> String {
    if cfg!(windows) {
        "powershell".to_string()
    } else {
        "bash".to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TerminalConfig {
    #[serde(default = "default_terminal_default_profile")]
    pub default_profile: String,
    #[serde(default)]
    pub git_bash_path: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            default_profile: default_terminal_default_profile(),
            git_bash_path: String::new(),
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

/// 用户自定义的同步目录。两台设备需配置相同的 `id` 才能配对该目录。
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CustomSyncDir {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct P2PSyncConfig {
    #[serde(default)]
    pub device_name: String,
    #[serde(default = "default_p2p_ticket_validity_mode")]
    pub ticket_validity_mode: String,
    #[serde(default = "default_p2p_ticket_ttl_minutes")]
    pub ticket_ttl_minutes: u64,
    #[serde(default)]
    pub auto_sync_resource_repo: bool,
    #[serde(default = "default_p2p_sync_interval_minutes")]
    pub sync_interval_minutes: u64,
    #[serde(default = "default_p2p_transfer_concurrency")]
    pub transfer_concurrency: u32,
    #[serde(default = "default_p2p_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_p2p_enable_block_delta")]
    pub enable_block_delta: bool,
    #[serde(default = "default_p2p_auto_propagate_trust")]
    pub auto_propagate_trust: bool,
    #[serde(default)]
    pub custom_sync_dirs: Vec<CustomSyncDir>,
    /// 中继模式："default"(n0 公共中继) | "custom"(自部署中继) | "lan_only"(禁用中继，仅局域网直连)。
    #[serde(default = "default_p2p_relay_mode")]
    pub relay_mode: String,
    /// 自定义中继地址，多个以换行或逗号分隔。仅 relay_mode == "custom" 时生效。
    #[serde(default)]
    pub custom_relay_urls: String,
}

fn default_p2p_relay_mode() -> String {
    "default".to_string()
}

impl Default for P2PSyncConfig {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            ticket_validity_mode: default_p2p_ticket_validity_mode(),
            ticket_ttl_minutes: default_p2p_ticket_ttl_minutes(),
            auto_sync_resource_repo: false,
            sync_interval_minutes: default_p2p_sync_interval_minutes(),
            transfer_concurrency: default_p2p_transfer_concurrency(),
            idle_timeout_secs: default_p2p_idle_timeout_secs(),
            enable_block_delta: default_p2p_enable_block_delta(),
            auto_propagate_trust: default_p2p_auto_propagate_trust(),
            custom_sync_dirs: Vec::new(),
            relay_mode: default_p2p_relay_mode(),
            custom_relay_urls: String::new(),
        }
    }
}

fn default_p2p_ticket_validity_mode() -> String {
    "expires".to_string()
}

fn default_p2p_ticket_ttl_minutes() -> u64 {
    10
}

fn default_p2p_sync_interval_minutes() -> u64 {
    720
}

fn default_p2p_transfer_concurrency() -> u32 {
    6
}

fn default_p2p_idle_timeout_secs() -> u64 {
    30
}

fn default_p2p_enable_block_delta() -> bool {
    true
}

fn default_p2p_auto_propagate_trust() -> bool {
    true
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
    #[serde(default)]
    pub p2p: P2PSyncConfig,
}

impl Default for ResourceRepoSyncConfig {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            prefer_pull: false,
            webdav: WebDavSyncConfig::default(),
            syncthing: SyncthingSyncConfig::default(),
            p2p: P2PSyncConfig::default(),
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

/// 语音合成已改为进程内调用 sherpa-onnx 库，不再有外部可执行文件。
/// `exe` 字段保留只为兼容既有的 guaxi_config.json，不再参与任何逻辑。
fn default_tts_exe() -> String {
    String::new()
}

/// 默认音色。Kokoro 名单里 0 是 af_maple（美式女声），中英文都可用。
fn default_tts_speaker_id() -> String {
    "0".to_string()
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
    // 默认指向 SenseVoice 多语模型目录（zh/en/ja/ko/yue）。
    // 通过 tools/fetch_sherpa_onnx_libs.mjs 下载到该目录后即开箱可用。
    "utils/Models/asr/sense-voice".to_string()
}

/// 同 [`default_tts_exe`]：识别也已进程内化，这里只为兼容旧配置。
fn default_asr_exe() -> String {
    String::new()
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

/// 代码配色主题的默认值：Atom One（暗色即 One Dark，亮色即 One Light）。
/// 与前端 `src/monaco/codeThemes.js` 的 DEFAULT_CODE_THEME_ID 保持一致。
fn default_code_theme() -> String {
    "atom-one".to_string()
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

/// AI 影像工坊的持久化配置：Blender / FFmpeg 可执行文件路径与本地资产库根目录。
/// 空字符串表示“未配置”，前端会回退到自动探测。这些不是机密，明文保存即可。
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FilmStudioConfig {
    #[serde(default)]
    pub blender_path: String,
    #[serde(default)]
    pub ffmpeg_path: String,
    #[serde(default)]
    pub asset_library_root: String,
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
const LOCAL_QWEN_MODEL_FILENAME: &str = "Qwen3.6-35B-A3B-UD-Q4_K_M.gguf";

/// 清理魔搭下载缓存目录。对外开放给其它模块（本地翻译小模型）复用。
pub(crate) fn cleanup_modelscope_download_dir(local_dir: &Path) {
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

/// 用已集成的魔搭（ModelScope）下载工具拉取单个模型文件。
/// 对外开放给其它模块（本地翻译小模型）复用：国内直连魔搭比 HuggingFace 快得多。
pub(crate) fn run_modelscope_download(local_dir: &Path, model_id: &str, filename: &str) -> Result<(), String> {
    let _ = bundled_resources::ensure_relative_dir("utils/python_libs")?;

    // 解释器来源统一交给文档转换模块解析：旧 conda 分发版 → marker 独立 venv → 托管运行时。
    // 改造前这里写死“必须存在 utils/MarkdownConvertor/marker-1.10.1”，一旦不再随包分发 conda 环境，
    // 本地 DeepSeek/Qwen 的模型下载就会直接断掉。
    let python_path = crate::commands::convert::provision::resolve_aux_python().ok_or_else(|| {
        "找不到可用的 Python 解释器：Marker 运行环境与托管运行时都不可用，\
         请先在设置页确认运行环境已就绪"
            .to_string()
    })?;

    let script_path = bundled_resources::ensure_relative_dir("scripts/modelscope_download.py")?;
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

    let mut command = Command::new(&python_path);
    apply_hidden_window(&mut command);
    let output = command
        .arg(script_path)
        .arg("--model-id")
        .arg(model_id)
        .arg("--filename")
        .arg(filename)
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
            life_album_amap_key: "".to_string(),
            template_repo_path: "".to_string(),
            slidev_repo_root: "".to_string(),
            chat_record_root: default_chat_record_root(),
            marker_ocr_mode: default_marker_ocr_mode(),
            marker_reading_order: default_marker_reading_order(),
            knowledge_bases_root: "".to_string(),
            knowledge_bases_remote_url: "".to_string(),
            knowledge_bases_remote_username: "".to_string(),
            knowledge_bases_remote_token: "".to_string(),
            knowledge_bases_git_username: "".to_string(),
            knowledge_bases_git_email: "".to_string(),
            device_id: Uuid::new_v4().to_string(),
            gc_cycle_seconds: default_gc_cycle_seconds(),
            lsp_syntax_highlight_enabled: false,
            code_theme: default_code_theme(),
            python_codegen_assist_enabled: false,
            lsp_github_token: "".to_string(),
            webrtc_signal_url: "".to_string(),
            llm_config: LlmConfig::default(),
            tts_config: TtsConfig::default(),
            asr_config: AsrConfig::default(),
            terminal_config: TerminalConfig::default(),
            film_studio_config: FilmStudioConfig::default(),
            disabled_services: Vec::new(),
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
    debug!(
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

    // 这里绝不能 panic：读配置的地方非常多（每次朗读、每个分段都要读一遍），
    // 而保存配置是「先截断再写」，撞上写的中间态就会读到空内容或半截内容。
    // 与其把整个应用打挂，不如退回默认配置——真损坏的原文件会先备份，设置不会凭空消失。
    let raw = fs::read_to_string(&config_path).unwrap_or_default();
    let mut v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                error = %error,
                path = config_path.to_string_lossy().as_ref(),
                "配置文件无法解析，暂用默认配置"
            );
            // 空文件多半是「写了一半」的瞬间，动它反而会踩到写侧；有内容却解析不了才是真损坏。
            if !raw.trim().is_empty() {
                stash_corrupt_config(&config_path);
            }
            serde_json::json!({})
        }
    };

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
        "/llm_config/custom_api_key",
        "/llm_config/cc_switch_api_key",
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
    try_decrypt("/llm_config/custom_api_key");
    try_decrypt("/llm_config/cc_switch_api_key");
    try_decrypt("/knowledge_bases_remote_token");
    try_decrypt("/lsp_github_token");

    let mut config: AppConfig = match serde_json::from_value(v) {
        Ok(config) => config,
        Err(error) => {
            // 与上面同理：字段对不上也退回默认配置，并触发一次修复性重写。
            warn!(error = %error, "配置字段无法映射，改用默认配置");
            needs_save = true;
            AppConfig::default()
        }
    };

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

/// 把损坏的配置文件挪到一边留作取证。
///
/// 不这么做的话，紧接着的修复性 `save_config` 会用默认配置直接覆盖掉它，用户的设置就真没了。
/// 文件名带时间戳，反复损坏也不会互相覆盖。
fn stash_corrupt_config(config_path: &Path) {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let backup = config_path.with_file_name(format!("guaxi_config.corrupt-{}.json", stamp));
    match fs::rename(config_path, &backup) {
        Ok(()) => warn!(
            backup = backup.to_string_lossy().as_ref(),
            "已把损坏的配置文件备份到一旁"
        ),
        Err(error) => warn!(error = %error, "备份损坏的配置文件失败"),
    }
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
    encrypt_field(&mut val, "/llm_config/custom_api_key");
    encrypt_field(&mut val, "/llm_config/cc_switch_api_key");
    encrypt_field(&mut val, "/knowledge_bases_remote_token");
    encrypt_field(&mut val, "/lsp_github_token");

    let json = serde_json::to_string_pretty(&val).unwrap();
    // 原子落盘：`fs::write` 是先截断再写，中间那一瞬间文件是空的，
    // 而读配置的地方很多（每次朗读、每个分段都要读一遍），撞上就会解析失败。
    // 先写临时文件、再改名过去——同卷改名是原子的，读侧永远看不到中间态。
    let temp_path = config_path.with_extension("json.tmp");
    let write_result = fs::write(&temp_path, &json).and_then(|()| fs::rename(&temp_path, &config_path));
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        // 写失败不再 panic：这里只是保存设置，不该把应用打挂。
        warn!(
            error = %error,
            path = config_path.to_string_lossy().as_ref(),
            "写入配置文件失败"
        );
        return;
    }

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
pub fn get_code_theme() -> String {
    let config = load_config();
    config.code_theme
}

#[tauri::command]
pub fn set_code_theme(code_theme: String) {
    let normalized = code_theme.trim().to_lowercase();
    let next = if normalized.is_empty() {
        default_code_theme()
    } else {
        normalized
    };

    info!(code_theme = next.as_str(), "set_code_theme");

    let mut config = load_config();
    config.code_theme = next;
    save_config(&config);
}

#[tauri::command]
pub fn get_python_codegen_assist_enabled() -> bool {
    let config = load_config();
    config.python_codegen_assist_enabled
}

#[tauri::command]
pub fn set_python_codegen_assist_enabled(enabled: bool) {
    let mut config = load_config();
    config.python_codegen_assist_enabled = enabled;
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
pub fn list_lsp_extensions() -> Result<Vec<lsp_extensions::LspExtensionState>, String> {
    lsp_extensions::list_lsp_extensions()
}

#[tauri::command]
pub async fn search_open_vsx_extensions(
    query: String,
) -> Result<Vec<lsp_extensions::OpenVsxSearchResult>, String> {
    lsp_extensions::search_open_vsx_extensions(&query).await
}

#[tauri::command]
pub async fn search_vscode_marketplace_extensions(
    query: String,
) -> Result<Vec<lsp_extensions::VscodeMarketplaceSearchResult>, String> {
    lsp_extensions::search_vscode_marketplace_extensions(&query).await
}

#[tauri::command]
pub async fn install_open_vsx_extension(
    extension_id: String,
    version: Option<String>,
    download_url: Option<String>,
) -> Result<lsp_extensions::LspExtensionState, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id 涓嶈兘涓虹┖".to_string());
    }
    lsp_extensions::install_open_vsx_extension(&id, version.as_deref(), download_url.as_deref()).await
}

#[tauri::command]
pub async fn install_vscode_marketplace_extension(
    extension_id: String,
    version: Option<String>,
    download_url: Option<String>,
) -> Result<lsp_extensions::LspExtensionState, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id 不能为空".to_string());
    }
    lsp_extensions::install_vscode_marketplace_extension(&id, version.as_deref(), download_url.as_deref()).await
}

#[tauri::command]
pub async fn install_catalog_lsp_extension(
    extension_id: String,
) -> Result<lsp_extensions::LspExtensionState, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id 不能为空".to_string());
    }
    lsp_extensions::install_catalog_lsp_extension(&id).await
}

#[tauri::command]
pub async fn import_lsp_extension_vsix(
    vsix_path: String,
) -> Result<lsp_extensions::LspExtensionState, String> {
    let path = vsix_path.trim().to_string();
    if path.is_empty() {
        return Err("vsix_path 涓嶈兘涓虹┖".to_string());
    }
    lsp_extensions::import_vsix_extension(&path).await
}

#[tauri::command]
pub async fn import_lsp_extension_directory(
    dir_path: String,
) -> Result<lsp_extensions::LspExtensionState, String> {
    let path = dir_path.trim().to_string();
    if path.is_empty() {
        return Err("dir_path 涓嶈兘涓虹┖".to_string());
    }
    lsp_extensions::import_extension_directory(&path).await
}

#[tauri::command]
pub async fn remove_lsp_extension(extension_id: String) -> Result<(), String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id 涓嶈兘涓虹┖".to_string());
    }
    lsp_extensions::remove_lsp_extension(&id).await
}

#[tauri::command]
pub fn get_lsp_extension_task(
    task_id: String,
) -> Result<Option<lsp_extensions::LspExtensionTaskSnapshot>, String> {
    Ok(lsp_extensions::get_lsp_extension_task(task_id.trim()))
}

#[tauri::command]
pub fn list_lsp_extension_tasks() -> Result<Vec<lsp_extensions::LspExtensionTaskSnapshot>, String> {
    Ok(lsp_extensions::list_lsp_extension_tasks())
}

#[tauri::command]
pub fn start_install_open_vsx_extension_task(
    extension_id: String,
    version: Option<String>,
    download_url: Option<String>,
) -> Result<lsp_extensions::LspExtensionTaskSnapshot, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }
    lsp_extensions::start_install_open_vsx_extension_task(id, version, download_url)
}

#[tauri::command]
pub fn start_install_vscode_marketplace_extension_task(
    extension_id: String,
    version: Option<String>,
    download_url: Option<String>,
) -> Result<lsp_extensions::LspExtensionTaskSnapshot, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }
    lsp_extensions::start_install_vscode_marketplace_extension_task(id, version, download_url)
}

#[tauri::command]
pub fn start_install_catalog_lsp_extension_task(
    extension_id: String,
) -> Result<lsp_extensions::LspExtensionTaskSnapshot, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }
    lsp_extensions::start_install_catalog_lsp_extension_task(id)
}

#[tauri::command]
pub fn start_import_lsp_extension_vsix_task(
    vsix_path: String,
) -> Result<lsp_extensions::LspExtensionTaskSnapshot, String> {
    let path = vsix_path.trim().to_string();
    if path.is_empty() {
        return Err("vsix_path cannot be empty".to_string());
    }
    lsp_extensions::start_import_vsix_extension_task(path)
}

#[tauri::command]
pub fn start_import_lsp_extension_directory_task(
    dir_path: String,
) -> Result<lsp_extensions::LspExtensionTaskSnapshot, String> {
    let path = dir_path.trim().to_string();
    if path.is_empty() {
        return Err("dir_path cannot be empty".to_string());
    }
    lsp_extensions::start_import_extension_directory_task(path)
}

#[tauri::command]
pub fn start_remove_lsp_extension_task(
    extension_id: String,
) -> Result<lsp_extensions::LspExtensionTaskSnapshot, String> {
    let id = extension_id.trim().to_string();
    if id.is_empty() {
        return Err("extension_id cannot be empty".to_string());
    }
    lsp_extensions::start_remove_lsp_extension_task(id)
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
    let mut tts = config.tts_config;
    // 与 get_asr_config 同样的兜底：任一字段为空就回退到内置默认。
    //
    // 注意 serde 的 `#[serde(default = ...)]` 只在**字段缺失**时生效，
    // 配置里写成 `"model": ""` 时它不会兜底，语音侧就会拿到空路径。
    // `exe` 已退役（语音改为进程内调用），保持原样即可。
    if tts.provider.trim().is_empty() {
        tts.provider = default_tts_provider();
    }
    if tts.model.trim().is_empty() {
        tts.model = default_tts_model();
    }
    if tts.tokens.trim().is_empty() {
        tts.tokens = default_tts_tokens();
    }
    if tts.output_dir.trim().is_empty() {
        tts.output_dir = default_tts_output_dir();
    }
    if tts.speaker_id.trim().is_empty() {
        tts.speaker_id = default_tts_speaker_id();
    }
    tts
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
        // 空值回落默认，避免前端漏传这个字段时把用户选的音色清掉。
        speaker_id: if tts_config.speaker_id.trim().is_empty() {
            default_tts_speaker_id()
        } else {
            tts_config.speaker_id.trim().to_string()
        },
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
    let mut asr = config.asr_config;
    // 兼容旧版本/被清空的配置：任一字段为空时回退到内置默认，
    // 避免以空 model 启动 sherpa-onnx 触发 "tokens: '' does not exist"。
    if asr.provider.trim().is_empty() {
        asr.provider = default_asr_provider();
    }
    if asr.model.trim().is_empty() {
        asr.model = default_asr_model();
    }
    if asr.exe.trim().is_empty() {
        asr.exe = default_asr_exe();
    }
    asr
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
pub fn get_terminal_config() -> TerminalConfig {
    let config = load_config();
    config.terminal_config
}

#[tauri::command]
pub fn set_terminal_config(terminal_config: TerminalConfig) {
    let mut config = load_config();
    let mut next = TerminalConfig {
        default_profile: terminal_config.default_profile.trim().to_string(),
        git_bash_path: terminal_config.git_bash_path.trim().to_string(),
    };
    if next.default_profile.is_empty() {
        next.default_profile = default_terminal_default_profile();
    }
    config.terminal_config = next.clone();
    info!(
        default_profile = next.default_profile.as_str(),
        git_bash_path = next.git_bash_path.as_str(),
        "set_terminal_config"
    );
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

/// marker OCR 策略的默认值：`auto`。
///
/// 为什么不直接默认 `full`：纯 CPU 机器上 VLM 解码只有约 6 token/s，扫描件每页要几分钟，
/// 上千页基本跑不完；但也不能默认 `text_only`——那会让扫描件彻底没内容。
/// `auto` 把判断交给运行时：有 GPU 就按 `full`（不该为了速度牺牲质量），
/// 没 GPU 时才让脚本按文档类型决定（数字文档跳过 OCR，扫描件照常 OCR）。
pub fn default_marker_ocr_mode() -> String {
    "auto".to_string()
}

/// 当前 marker OCR 策略（已归一化，非法值回退默认）。
pub fn marker_ocr_mode() -> String {
    let mode = load_config().marker_ocr_mode.trim().to_ascii_lowercase();
    match mode.as_str() {
        "auto" | "full" | "text_only" => mode,
        _ => default_marker_ocr_mode(),
    }
}

#[tauri::command]
pub fn get_marker_ocr_mode() -> String {
    marker_ocr_mode()
}

#[tauri::command]
pub fn set_marker_ocr_mode(mode: String) -> Result<(), String> {
    let normalized = mode.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "auto" | "full" | "text_only") {
        return Err(format!("不支持的 marker OCR 策略: {mode}"));
    }
    let mut config = load_config();
    config.marker_ocr_mode = normalized;
    save_config(&config);
    Ok(())
}

/// marker 阅读顺序策略的默认值：`layout`。
///
/// marker 2.x 默认按 PDF 字符流排块（`use_pdftext_reading_order=true`），这在多栏页面上
/// 比模型的阅读顺序头更准；但字符流顺序与版面顺序并不总是一致——浏览器「打印成 PDF」
/// 的网页会先输出正文、最后才写各节标题，按字符流排序就会把标题整段挪到文末，
/// 与正文分家。版面顺序对这类文档稳定得多，所以默认取 `layout`。
pub fn default_marker_reading_order() -> String {
    "layout".to_string()
}

/// 当前 marker 阅读顺序策略（已归一化，非法值回退默认）。
pub fn marker_reading_order() -> String {
    let order = load_config().marker_reading_order.trim().to_ascii_lowercase();
    match order.as_str() {
        "layout" | "pdftext" => order,
        _ => default_marker_reading_order(),
    }
}

#[tauri::command]
pub fn get_marker_reading_order() -> String {
    marker_reading_order()
}

#[tauri::command]
pub fn set_marker_reading_order(order: String) -> Result<(), String> {
    let normalized = order.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "layout" | "pdftext") {
        return Err(format!("不支持的 marker 阅读顺序: {order}"));
    }
    let mut config = load_config();
    config.marker_reading_order = normalized;
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

fn local_deepseek_model_path() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir(&format!(
        "utils/Models/chat/{}",
        LOCAL_DEEPSEEK_MODEL_FILENAME
    ))
}

fn local_qwen_model_path() -> Result<PathBuf, String> {
    bundled_resources::ensure_relative_dir(&format!(
        "utils/Models/chat/{}",
        LOCAL_QWEN_MODEL_FILENAME
    ))
}

/// 解析 `utils/llama` 下的 llama-server 可执行文件。
/// 文档转换引擎（marker 的 surya VLM 走 llama.cpp 后端）也复用它，故为 crate 可见。
pub(crate) fn resolve_llama_server_path() -> Option<PathBuf> {
    let llama_dir = bundled_resources::ensure_relative_dir("utils/llama").ok()?;
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
        let expected = bundled_resources::ensure_relative_dir("utils/Models/surya_models")?;

        // marker 2.x 的模型集合与 1.x 完全不同（VLM 的 GGUF + rf-detr 布局 + s3 的
        // ocr_error）。单看目录是否存在，会把 1.x 留下来的、已经失效的模型目录误判成
        // “已加载”，因此改为检查关键模型是否齐全。
        let installed = crate::commands::convert::provision::marker_models_ready();

        Ok(MarkdownInstallStatus {
            installed,
            expected_path: expected.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_local_deepseek_status() -> Result<LocalDeepseekStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let expected = local_deepseek_model_path()?;
        let llama_server_path = resolve_llama_server_path();

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
    let model_path = local_deepseek_model_path()?;

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

    let parent_for_modelscope = parent.clone();
    let modelscope_result = tauri::async_runtime::spawn_blocking(move || {
        run_modelscope_download(
            &parent_for_modelscope,
            "unsloth/DeepSeek-R1-0528-Qwen3-8B-GGUF",
            LOCAL_DEEPSEEK_MODEL_FILENAME,
        )
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

    let model_path = local_deepseek_model_path()?;
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
pub async fn get_local_qwen_status() -> Result<LocalDeepseekStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let expected = local_qwen_model_path()?;
        let llama_server_path = resolve_llama_server_path();

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
pub async fn download_local_qwen_model() -> Result<String, String> {
    let model_path = local_qwen_model_path()?;

    if model_path.is_file() {
        if let Some(parent) = model_path.parent() {
            cleanup_modelscope_download_dir(parent);
        }
        info!(path = model_path.to_string_lossy().as_ref(), "local qwen model exists, skip download");
        return Ok(model_path.to_string_lossy().to_string());
    }

    let parent = model_path
        .parent()
        .ok_or("invalid model path".to_string())?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;

    let parent_for_modelscope = parent.clone();
    let modelscope_result = tauri::async_runtime::spawn_blocking(move || {
        run_modelscope_download(
            &parent_for_modelscope,
            "unsloth/Qwen3.6-35B-A3B-GGUF",
            LOCAL_QWEN_MODEL_FILENAME,
        )
    })
    .await
    .map_err(|e| e.to_string())?;

    if modelscope_result.is_ok() && model_path.is_file() {
        cleanup_modelscope_download_dir(&parent);
        return Ok(model_path.to_string_lossy().to_string());
    }

    Err(format!(
        "download local qwen model failed. modelscope_result={:?}. You can manually import GGUF via install_local_qwen_model_from_file(source_path). target={}",
        modelscope_result.err(),
        model_path.to_string_lossy()
    ))
}

#[tauri::command]
pub async fn install_local_qwen_model_from_file(source_path: String) -> Result<String, String> {
    let src = PathBuf::from(source_path.trim());
    if !src.is_file() {
        return Err(format!("source file not found: {}", src.to_string_lossy()));
    }

    let ext_ok = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false);
    if !ext_ok {
        return Err("source file must be a .gguf model file".to_string());
    }

    let model_path = local_qwen_model_path()?;
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

/// 目录复制，且**跟随符号链接**并把目标内容落地成真实文件。
///
/// marker 2.x 的模型取自 HuggingFace 缓存布局：`blobs/<hash>` 存真实数据，
/// `snapshots/<rev>/<name>` 只是指向它们的符号链接。而 `DirEntry::file_type()` **不跟随链接**
/// ——链接既不是 `is_dir` 也不是 `is_file`，用通用复制会把它**静默跳过**，得到“安装提示成功、
/// 模型实际不可用”这种最难排查的结果。因此这里改用 `fs::metadata`（跟随链接）判定类型，
/// 用 `fs::copy`（同样跟随链接）复制内容；链接指向的目标不存在时直接报错，而不是留个空壳。
fn copy_dir_recursive_follow_links(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        // fs::metadata 跟随符号链接；meta 失败说明是断链，直接报出来。
        let meta = fs::metadata(&src_path)
            .map_err(|e| format!("读取失败（可能是断链）: {} ({e})", src_path.to_string_lossy()))?;

        if meta.is_dir() {
            copy_dir_recursive_follow_links(&src_path, &dst_path)?;
        } else if meta.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if dst_path.exists() {
                let _ = fs::remove_file(&dst_path);
            }
            fs::copy(&src_path, &dst_path).map_err(|e| {
                format!("复制失败: {} -> {} ({e})", src_path.to_string_lossy(), dst_path.to_string_lossy())
            })?;
        }
    }
    Ok(())
}

/// 在 HuggingFace 缓存目录里找一个 snapshot 文件（`snapshots/<rev>/<name>`）。
/// `is_file()` 跟随链接，因此断链会判为不存在——这正是我们要区分的情况。
fn find_snapshot_file(repo_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let snapshots = repo_dir.join("snapshots");
    for entry in fs::read_dir(&snapshots).ok()?.flatten() {
        let candidate = entry.path().join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 判断一个 HuggingFace 缓存仓库是否是**完整快照**。
///
/// 为什么完整性是硬要求：模型就地就绪后，转换进程会进入 `HF_HUB_OFFLINE=1` 离线模式
/// （本地有模型就不必每次转换都先去 HF API 核对 revision，大陆网络下那次请求经常抖动，
/// 实测撞到过它把整次转换打断）。而 huggingface_hub 在离线模式下会拿下载时留下的仓库清单
/// `trees/<rev>.json` 逐个核对本地文件，**缺任何一个就直接抛 IncompleteSnapshotError**，
/// 哪怕缺的只是不参与推理的 LICENSE / README.md。
///
/// 所以这里做与 hub 相同的核对：清单里的每个文件都必须在 snapshots 下真实存在（跟随符号链接），
/// 且大小一致。清单不存在时**判定为不完整**——这样老版本下载脚本留下的"只下权重"的缓存会被
/// 判为未就绪，从而触发一次补齐下载（下载脚本自身是整仓拉取），自愈成完整快照。
fn hf_cache_complete(repo_dir: &Path) -> Result<(), String> {
    let snapshots = repo_dir.join("snapshots");
    let rev_dir = fs::read_dir(&snapshots)
        .map_err(|e| format!("缺少 snapshots 目录 {}: {e}", snapshots.display()))?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .ok_or_else(|| format!("snapshots 目录为空: {}", snapshots.display()))?;

    let rev = rev_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("无法解析 revision 名: {}", rev_dir.display()))?;

    let tree_file = repo_dir.join("trees").join(format!("{rev}.json"));
    let text = fs::read_to_string(&tree_file).map_err(|e| {
        format!(
            "缺少仓库清单 {}（无法确认缓存完整，需重新下载）: {e}",
            tree_file.display()
        )
    })?;
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("仓库清单解析失败: {e}"))?;
    let files = parsed
        .get("files")
        .and_then(|value| value.as_object())
        .ok_or_else(|| "仓库清单缺少 files 字段".to_string())?;

    for (relative, meta) in files {
        let local = rev_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let actual = fs::metadata(&local).map_err(|e| {
            format!("缓存不完整：缺少 {relative}（{e}）")
        })?;
        if !actual.is_file() {
            return Err(format!("缓存不完整：{relative} 不是文件"));
        }
        if let Some(expected) = meta.get("size").and_then(|value| value.as_u64()) {
            if expected > 0 && actual.len() != expected {
                return Err(format!(
                    "缓存损坏：{relative} 大小不符（期望 {expected}，实际 {}）",
                    actual.len()
                ));
            }
        }
    }

    Ok(())
}

/// 校验离线模型包的安装结果：关键权重必须**真的能读到**，而不是断链/被跳过后的空壳。
///
/// 阈值 1 MB 只用来区分“真实权重”与“丢了链接后残留的路径文本文件”（几十字节），
/// 不是精度门槛，因此不会因为模型换量化版本而误报。
///
/// 同时被文档转换模块复用（`provision::marker_models_ready`）：那里用它判定“模型是否齐全”，
/// 而该判定又会决定是否跳过预下载、以及让转换进程进入 HF 离线模式，所以必须验到文件级，
/// 而且要验到**完整快照**（见 [`hf_cache_complete`]）。
pub(crate) fn verify_installed_markdown_models(root: &Path) -> Result<(), String> {
    const MIN_WEIGHT_BYTES: u64 = 1024 * 1024;

    let gguf_repo = root.join("models--datalab-to--surya-ocr-2-gguf");
    let gguf = find_snapshot_file(&gguf_repo, "surya-2.gguf").ok_or_else(|| {
        "离线模型包不完整：未找到 VLM 权重 surya-2.gguf".to_string()
    })?;
    let gguf_size = fs::metadata(&gguf).map(|m| m.len()).unwrap_or(0);
    if gguf_size < MIN_WEIGHT_BYTES {
        return Err(format!(
            "离线模型包不可用：surya-2.gguf 只有 {gguf_size} 字节，通常是打包时丢掉了 HuggingFace \
             缓存里的符号链接。请在能联网的机器上执行 \
             `python download_surya_models.py --dest <目录> --dir surya_models --materialize` \
             重新生成模型目录，再打包分发。"
        ));
    }
    if find_snapshot_file(&gguf_repo, "surya-2-mmproj.gguf").is_none() {
        return Err("离线模型包不完整：未找到 surya-2-mmproj.gguf（多模态投影权重）".to_string());
    }

    let layout_repo = root.join("models--datalab-to--surya_layout2");
    let rfdetr = find_snapshot_file(&layout_repo, "rfdetr_layout.pth").ok_or_else(|| {
        "离线模型包不完整：未找到布局模型 rfdetr_layout.pth（datalab-to/surya_layout2）".to_string()
    })?;
    if fs::metadata(&rfdetr).map(|m| m.len()).unwrap_or(0) < MIN_WEIGHT_BYTES {
        return Err("离线模型包不可用：rfdetr_layout.pth 内容异常（疑似打包时丢失符号链接）".to_string());
    }

    if !root
        .join("ocr_error_detection")
        .join("2025_02_18")
        .join("manifest.json")
        .is_file()
    {
        return Err(
            "离线模型包不完整：缺少 ocr_error_detection/2025_02_18/manifest.json".to_string(),
        );
    }

    // 关键权重都在之后，还要确认 HF 缓存是**完整快照**：
    // 转换进程的就绪判定会据此开启离线模式，而离线模式对不完整缓存是硬失败。
    for repo in [
        "models--datalab-to--surya-ocr-2-gguf",
        "models--datalab-to--surya_layout2",
    ] {
        hf_cache_complete(&root.join(repo)).map_err(|error| {
            format!("{error}（仓库 {repo}；重新下载模型即可补齐）")
        })?;
    }

    Ok(())
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
        let utils_dir = bundled_resources::ensure_relative_dir("utils")?;
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
            // 跟随符号链接复制：HF 缓存布局下 snapshots/ 里是指向 blobs/ 的链接，
            // 不跟随会静默丢文件（装完看着目录在、模型实际用不了）。
            copy_dir_recursive_follow_links(&packaged_surya_models, &dst_surya_models)?;

            if !dst_surya_models.is_dir() {
                return Err(format!(
                    "模型目录校验失败: {}",
                    dst_surya_models.to_string_lossy()
                ));
            }

            // 装完立刻校验关键权重是否真的落地，避免把“空壳目录”当成安装成功。
            verify_installed_markdown_models(&dst_surya_models)?;

            Ok(dst_surya_models.to_string_lossy().to_string())
        })();

        let _ = remove_path_if_exists(&temp_dir);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

// 说明：原先这里还有 `get_markdown_convertor_env_status` / `install_markdown_convertor_env_archive`
// 一对命令，用于“离线加载转换运行环境”（把预打包好的 MarkdownConvertor.zip 拷进安装目录）。
// marker 运行环境改为启动时自动装配（uv 建独立 venv + 按 GPU 选 torch）后，这套离线分发不再需要，
// 已整体移除；模型仍保留离线加载入口（见上面的 install_markdown_models_archive）。

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
pub fn get_film_studio_config() -> FilmStudioConfig {
    let config = load_config();
    config.film_studio_config
}

#[tauri::command]
pub fn set_film_studio_config(
    blender_path: String,
    ffmpeg_path: String,
    asset_library_root: String,
) {
    let mut config = load_config();
    config.film_studio_config = FilmStudioConfig {
        blender_path: blender_path.trim().to_string(),
        ffmpeg_path: ffmpeg_path.trim().to_string(),
        asset_library_root: asset_library_root.trim().to_string(),
    };
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

pub fn get_life_album_amap_key() -> String {
    load_config().life_album_amap_key
}

#[tauri::command]
pub fn get_life_album_amap_key_cmd() -> String {
    load_config().life_album_amap_key
}

#[tauri::command]
pub fn set_life_album_amap_key(key: String) {
    let mut config = load_config();
    config.life_album_amap_key = key.trim().to_string();
    save_config(&config);
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

#[allow(dead_code)]
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
