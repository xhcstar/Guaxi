use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::info;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    appearance: AppearanceConfig,
    active_repo_path: Option<String>,
    repopathlist: Vec<String>, // 新增字段
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    host: String,
    port: u16,
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
        AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            database: DatabaseConfig {
                url: "postgres://localhost:5432/mydb".to_string(),
                username: "admin".to_string(),
                password: "password".to_string(),
            },
            appearance: AppearanceConfig {
                theme: "light".to_string(),
                font_size: 14,
            },
            active_repo_path: None,
            repopathlist: Vec::new(), // 默认值为空列表
        }
    }
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
        let default_config = AppConfig::default();
        fs::write(&config_path, serde_json::to_string(&default_config).unwrap())
            .expect("无法写入默认配置文件");
        return default_config;
    }

    // 读取配置文件
    let config_data = fs::read_to_string(&config_path).expect("无法读取配置文件");
    serde_json::from_str(&config_data).expect("无法解析配置文件")
}

pub fn save_config(config: &AppConfig) {
    let config_path = get_config_path();
    fs::write(&config_path, serde_json::to_string(config).unwrap())
        .expect("无法写入配置文件");
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
    config.database = DatabaseConfig { url, username, password };
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