use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::info;
use uuid::Uuid;

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
    template_repo_path: String,
    #[serde(default)]
    knowledge_bases_root: String,
    #[serde(default)]
    device_id: String,
    #[serde(default = "default_gc_cycle_seconds")]
    pub gc_cycle_seconds: u64,
    #[serde(default)]
    lsp_syntax_highlight_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    host: String,
    port: u16,
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
            template_repo_path: "".to_string(),
            knowledge_bases_root: "".to_string(),
            device_id: Uuid::new_v4().to_string(),
            gc_cycle_seconds: default_gc_cycle_seconds(),
            lsp_syntax_highlight_enabled: false,
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
pub fn get_knowledge_bases_root() -> String {
    if let Err(e) = init_knowledge_bases_root() {
        info!("init_knowledge_bases_root failed: {}", e);
    }
    let config = load_config();
    config.knowledge_bases_root
}

#[tauri::command]
pub fn set_knowledge_bases_root(path: String) -> Result<(), String> {
    let root_path = PathBuf::from(&path);
    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    let meta_path = root_path.join(".meta");
    if !meta_path.exists() {
        fs::create_dir_all(&meta_path).map_err(|e| e.to_string())?;
    }

    let mut config = load_config();
    config.knowledge_bases_root = path;
    save_config(&config);
    Ok(())
}

pub fn init_knowledge_bases_root() -> Result<(), String> {
    let mut config = load_config();
    let default_root = default_knowledge_bases_root();

    let configured = config.knowledge_bases_root.trim().to_string();
    let mut effective = if configured.is_empty() {
        default_root.clone()
    } else {
        configured
    };

    let effective_path = PathBuf::from(&effective);
    if !effective_path.exists() || !effective_path.is_dir() {
        effective = default_root.clone();
        config.knowledge_bases_root = effective.clone();
        save_config(&config);
    } else if config.knowledge_bases_root.trim().is_empty() {
        config.knowledge_bases_root = effective.clone();
        save_config(&config);
    }

    let root_path = PathBuf::from(&effective);
    if !root_path.exists() {
        fs::create_dir_all(&root_path).map_err(|e| e.to_string())?;
    }
    let meta_path = root_path.join(".meta");
    if !meta_path.exists() {
        fs::create_dir_all(&meta_path).map_err(|e| e.to_string())?;
    }

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

pub fn get_device_id() -> String {
    let mut config = load_config();
    if config.device_id.is_empty() {
        config.device_id = Uuid::new_v4().to_string();
        save_config(&config);
    }
    config.device_id.clone()
}