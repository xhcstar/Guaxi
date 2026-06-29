//! Guaxi Agent Server — Python 进程管理
//!
//! 管理 agent_server/ FastAPI 服务的生命周期：
//! - 启动 uvicorn 进程（127.0.0.1:9876）
//! - 健康检查
//! - 转发 SSE 流到 Tauri 前端

use crate::commands::settingview;
use crate::guaxiservices::bundled_resources;
use reqwest::Client;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;
use tracing::info;

const AGENT_SERVER_PORT: u16 = 9876;
const AGENT_SERVER_URL: &str = "http://127.0.0.1:9876";
const AGENT_SERVER_STARTUP_TIMEOUT_SECS: u64 = 30;

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window(_cmd: &mut Command) {}

/// 全局 agent server 进程
static AGENT_SERVER_PROCESS: std::sync::LazyLock<Mutex<Option<Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// 找到 Python 解释器路径
fn resolve_python() -> Result<String, String> {
    // 优先使用 Guaxi 托管运行时 (utils/python_runtime/.venv)
    let runtime_dir = bundled_resources::ensure_relative_dir("utils/python_runtime")?;
    let venv_dir = runtime_dir.join(".venv");
    
    let managed_python = if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    };
    
    if managed_python.is_file() {
        let path_str = managed_python.to_string_lossy().to_string();
        // 验证可用性
        if let Ok(output) = Command::new(&path_str).arg("--version").output() {
            if output.status.success() {
                info!(python = %path_str, "[agent_server] using managed python");
                return Ok(path_str);
            }
        }
    }

    // Fallback: 系统 Python
    for exe in &["python", "python3"] {
        if let Ok(output) = Command::new(exe).arg("--version").output() {
            if output.status.success() {
                return Ok(exe.to_string());
            }
        }
    }

    Err("未找到可用的 Python 解释器".to_string())
}

/// 找到 agent_server 源代码目录
fn agent_server_source_dir() -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法获取 exe 目录")?
        .to_path_buf();

    // 开发模式：agent_server/ 在项目根
    // exe: target/debug/guaxi.exe → 项目根: ../../../agent_server
    let dev_path = exe_dir
        .parent()   // target/debug → target
        .ok_or("无效 exe 路径")?
        .parent()   // target → src-tauri
        .ok_or("无效 exe 路径")?
        .parent()   // src-tauri → 项目根
        .ok_or("无效 exe 路径")?
        .join("agent_server");
    if dev_path.join("main.py").exists() {
        return Ok(dev_path);
    }

    // 打包模式：查找 bundled 资源
    // agent_server 会作为 extra resource 被打包
    let bundled = exe_dir.join("agent_server");
    if bundled.join("main.py").exists() {
        return Ok(bundled);
    }

    Err(format!(
        "找不到 agent_server 源代码目录，已查找: {:?}, {:?}",
        dev_path, bundled
    ))
}

/// 构建 LLM 配置 JSON（从 settingview 读取所有模型的 API keys）
fn build_llm_config_json() -> Value {
    let llm = settingview::get_llm_config();
    // 提取所有字段避免部分移动问题
    let chatgpt = llm.chatgpt_api_key;
    let deepseek = llm.deepseek_api_key;
    let qwen = llm.qwen_api_key;
    let kimi = llm.kimi_api_key;
    let doubao = llm.doubao_api_key;
    let sf = llm.siliconflow_api_key;
    let custom_base = llm.custom_base_url;
    let custom_key = llm.custom_api_key;
    let custom_model = llm.custom_model;

    json!([
        {"id":0,"name":"ChatGPT","base_url":"https://api.openai.com/v1","api_key":chatgpt,"model_name":"gpt-4o","supports_tools":true},
        {"id":1,"name":"DeepSeek","base_url":"https://api.deepseek.com/v1","api_key":deepseek,"model_name":"deepseek-chat","supports_tools":true},
        {"id":2,"name":"Qwen","base_url":"https://dashscope.aliyuncs.com/compatible-mode/v1","api_key":qwen,"model_name":"qwen-plus","supports_tools":true},
        {"id":3,"name":"Kimi","base_url":"https://api.moonshot.cn/v1","api_key":kimi,"model_name":"moonshot-v1-8k","supports_tools":true},
        {"id":4,"name":"Doubao","base_url":"https://ark.cn-beijing.volces.com/api/v3","api_key":doubao,"model_name":"doubao-pro-32k","supports_tools":true},
        {"id":5,"name":"SiliconFlow","base_url":"https://api.siliconflow.cn/v1","api_key":sf,"model_name":"deepseek-ai/DeepSeek-R1-0528-Qwen3-8B","supports_tools":true},
        {"id":6,"name":"LocalDeepSeek","base_url":"http://127.0.0.1:18080/v1","api_key":"not-needed","model_name":"local-deepseek","supports_tools":true},
        {"id":9,"name":"LocalQwen3","base_url":"http://127.0.0.1:18082/v1","api_key":"not-needed","model_name":"local-qwen3","supports_tools":true},
        {"id":10,"name":"Custom","base_url":custom_base,"api_key":custom_key,"model_name":custom_model,"supports_tools":true}
    ])
}

/// 启动 agent server 进程
pub async fn ensure_agent_server_started(app: &AppHandle) -> Result<(), String> {
    // 先检查是否已在运行
    if check_agent_server_health().await.is_ok() {
        return Ok(());
    }

    let python = resolve_python()?;
    let source_dir = agent_server_source_dir()?;

    info!(
        python = %python,
        source_dir = %source_dir.to_string_lossy(),
        "[agent_server] starting agent server"
    );

    // 构建 LLM 配置
    let llm_config = build_llm_config_json().to_string();
    let work_dir = settingview::get_knowledge_bases_root();

    let mut cmd = Command::new(&python);
    cmd.args([
        "-m", "uvicorn",
        "agent_server.main:app",
        "--host", "127.0.0.1",
        "--port", &AGENT_SERVER_PORT.to_string(),
        "--log-level", "info",
    ])
    .env("PYTHONPATH", source_dir.parent().unwrap_or(&source_dir).to_string_lossy().to_string())
    .env("GUAXI_LLM_CONFIG", &llm_config)
    .env("GUAXI_WORK_DIR", &work_dir)
    .env("PYTHONUTF8", "1")
    .current_dir(source_dir.parent().unwrap_or(&source_dir))
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    apply_no_window(&mut cmd);

    let child = cmd
        .spawn()
        .map_err(|e| format!("启动 agent server 失败: {}", e))?;

    // 保存进程句柄
    {
        let mut guard = AGENT_SERVER_PROCESS
            .lock()
            .map_err(|_| "lock agent server process failed".to_string())?;
        if let Some(mut old) = guard.take() {
            let _ = old.kill();
        }
        *guard = Some(child);
    }

    // 等待服务就绪
    let client = Client::new();
    for _ in 0..AGENT_SERVER_STARTUP_TIMEOUT_SECS {
        if let Ok(resp) = client
            .get(format!("{}/health", AGENT_SERVER_URL))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() {
                info!("[agent_server] agent server is ready on {}", AGENT_SERVER_URL);
                let _ = app.emit("agent-server-ready", json!({"url": AGENT_SERVER_URL}));
                return Ok(());
            }
        }
        sleep(Duration::from_secs(1)).await;
    }

    Err(format!(
        "agent server 在 {} 秒内未就绪",
        AGENT_SERVER_STARTUP_TIMEOUT_SECS
    ))
}

/// 停止 agent server 进程
pub fn stop_agent_server() {
    if let Ok(mut guard) = AGENT_SERVER_PROCESS.lock() {
        if let Some(mut child) = guard.take() {
            info!("[agent_server] stopping agent server");
            let _ = child.kill();
        }
    }
}

/// 健康检查
pub async fn check_agent_server_health() -> Result<(), String> {
    let client = Client::new();
    let resp = client
        .get(format!("{}/health", AGENT_SERVER_URL))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map_err(|e| format!("agent server 不可达: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("agent server 返回错误: {}", resp.status()))
    }
}