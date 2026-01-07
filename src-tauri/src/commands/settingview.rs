use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use uuid::Uuid;

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
    appearance: AppearanceConfig,
    libraries_repo_config: LibraryConfig,
    active_repo_path: Option<String>,
    repopathlist: Vec<String>,
    libraries_root: String,
    #[serde(default = "default_image_bed_root")]
    image_bed_root: String,
    #[serde(default)]
    life_album_album_dir: String,
    #[serde(default)]
    template_repo_path: String,
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
    llm_config: LlmConfig,
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

fn default_chat_record_root() -> String {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();
    exe_dir.join("ChatRecord").to_string_lossy().to_string()
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
    let mut s = raw.trim().to_string();
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

#[derive(Serialize, Deserialize, Debug)]
pub struct AppearanceConfig {
    theme: String,
    font_size: u8,
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
            life_album_album_dir: "".to_string(),
            template_repo_path: "".to_string(),
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
            llm_config: LlmConfig::default(),
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
    let config_path = get_config_path();

    if !config_path.exists() {
        // 如果配置文件不存在，创建默认配置
        let mut default_config = AppConfig::default();
        if default_config.device_id.is_empty() {
            default_config.device_id = Uuid::new_v4().to_string();
        }
        fs::write(
            &config_path,
            serde_json::to_string(&default_config).unwrap(),
        )
        .expect("无法写入默认配置文件");
        return default_config;
    }

    // 读取配置文件
    let config_data = fs::read_to_string(&config_path).expect("无法读取配置文件");
    let mut config: AppConfig = serde_json::from_str(&config_data).expect("无法解析配置文件");
    if config.device_id.is_empty() {
        config.device_id = Uuid::new_v4().to_string();
        save_config(&config);
    }
    config
}

pub fn save_config(config: &AppConfig) {
    let config_path = get_config_path();
    fs::write(&config_path, serde_json::to_string(config).unwrap()).expect("无法写入配置文件");
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
    let config = load_config();
    KnowledgeBasesRemoteConfig {
        remote_url: config.knowledge_bases_remote_url,
        username: config.knowledge_bases_remote_username,
        has_token: !config.knowledge_bases_remote_token.trim().is_empty(),
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
    config.knowledge_bases_remote_url = normalize_remote_url(remote_url.as_str());
    config.knowledge_bases_remote_username = username.trim().to_string();
    if !token.trim().is_empty() {
        config.knowledge_bases_remote_token = token.trim().to_string();
    }
    config.knowledge_bases_git_username = git_username.trim().to_string();
    config.knowledge_bases_git_email = git_email.trim().to_string();
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

pub fn get_knowledge_bases_remote_username_token() -> (String, String) {
    let config = load_config();
    (
        config.knowledge_bases_remote_username.trim().to_string(),
        config.knowledge_bases_remote_token.trim().to_string(),
    )
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