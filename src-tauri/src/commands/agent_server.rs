//! Guaxi Agent Server — Python 进程管理
//!
//! 管理 agent_server/ FastAPI 服务的生命周期：
//! - 启动 uvicorn 进程（127.0.0.1:9876）
//! - 健康检查
//! - 转发 SSE 流到 Tauri 前端

use crate::commands::settingview;
use reqwest::Client;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;
use tracing::{info, warn};

const AGENT_SERVER_PORT: u16 = 9876;
const AGENT_SERVER_URL: &str = "http://127.0.0.1:9876";
const AGENT_SERVER_STARTUP_TIMEOUT_SECS: u64 = 30;

#[cfg(all(windows, not(debug_assertions)))]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(all(windows, not(debug_assertions)))]
fn apply_no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn apply_no_window(_cmd: &mut Command) {}

/// 全局 agent server 进程
static AGENT_SERVER_PROCESS: std::sync::LazyLock<Mutex<Option<Child>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// agent server 日志文件路径（stdout/stderr 重定向目标）。
///
/// 设计说明：旧版把子进程 stdout/stderr 丢进 Stdio::null()，Python 启动失败（缺依赖、
/// 导入报错、端口占用）时前端只会看到一句没有信息量的“30 秒未就绪”。这里把输出落到固定
/// 日志文件，启动失败时读取尾部回传，便于排查。
fn agent_server_log_path() -> PathBuf {
    std::env::temp_dir().join("guaxi-agent-server.log")
}

/// 读取日志文件尾部若干行，用于把子进程的真实报错带回错误信息。
fn read_log_tail(path: &Path, max_lines: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(max_lines);
            lines[start..].join("\n")
        }
        Err(_) => String::new(),
    }
}

/// 解析 agent_server/requirements.txt 为待安装的依赖列表。
///
/// 过滤空行与 `#` 注释行；其余行（含 `uvicorn[standard]>=0.23.0` 这类带 extras/版本约束的
/// 写法）原样返回，由 code_runtime 的 pip 安装逻辑按规范化包名做幂等去重。
fn read_agent_requirements(source_dir: &Path) -> Vec<String> {
    let req_path = source_dir.join("requirements.txt");
    match std::fs::read_to_string(&req_path) {
        Ok(content) => content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect(),
        Err(err) => {
            warn!(
                path = %req_path.to_string_lossy(),
                error = %err,
                "[agent_server] read requirements.txt failed"
            );
            Vec::new()
        }
    }
}

/// 找到 agent_server 源代码目录
fn agent_server_source_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法获取 exe 目录")?
        .to_path_buf();

    let mut candidates: Vec<PathBuf> = Vec::new();

    // 打包模式：agent_server 会作为 Tauri resource 落在 exe 同级目录或 resources 子目录。
    // 先检查 release 候选，避免浅层安装路径（例如 D:\guaxi.exe）因开发路径 parent 不足提前失败。
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("agent_server"));
    }
    candidates.push(exe_dir.join("agent_server"));
    candidates.push(exe_dir.join("resources").join("agent_server"));

    // 开发模式：agent_server/ 在项目根。
    // exe: target/debug/guaxi.exe → 项目根: ../../../agent_server
    // 设计说明：开发路径只是候选项，不能用 ok_or 提前返回；release 的 exe 目录层级并不固定。
    if let Some(dev_path) = exe_dir
        .parent()   // target/debug → target
        .and_then(|p| p.parent())   // target → src-tauri
        .and_then(|p| p.parent())   // src-tauri → 项目根
        .map(|p| p.join("agent_server"))
    {
        candidates.push(dev_path);
    }

    for candidate in &candidates {
        if candidate.join("main.py").exists() {
            info!(
                source_dir = %candidate.to_string_lossy(),
                "[agent_server] resolved source directory"
            );
            return Ok(candidate.clone());
        }
    }

    let searched = candidates
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    warn!("[agent_server] source directory not found, searched: {}", searched);
    Err(format!(
        "找不到 agent_server 源代码目录，已查找: {}",
        searched
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
    let source_dir = agent_server_source_dir(app)?;

    // 确保 Guaxi 托管 Python 就绪，并把 agent_server 依赖安装进托管 .venv。
    //
    // 设计说明：这一步要放在健康检查之前。agent server 进程可能已经存活，但 requirements
    // 刚新增了运行时依赖（例如 httpx 的 SOCKS 支持 socksio）；如果先 health check 并直接返回，
    // 已运行服务会继续在请求阶段因缺包失败。pip list 对比是幂等的，已满足时开销很小。
    let requirements = read_agent_requirements(&source_dir);
    let (python, base_args) = tokio::task::spawn_blocking(move || {
        crate::commands::code_runtime::ensure_managed_python_with_packages(
            &requirements,
            "agent-server",
        )
    })
    .await
    .map_err(|e| format!("准备 Python 运行时任务失败: {}", e))??;

    // 依赖已确认后再检查服务是否运行；运行中的 Python 进程可在后续请求动态 import 新装包。
    if check_agent_server_health().await.is_ok() {
        info!("[agent_server] agent server already running with dependencies ensured");
        return Ok(());
    }

    info!(
        python = %python,
        source_dir = %source_dir.to_string_lossy(),
        "[agent_server] starting agent server"
    );

    // 构建 LLM 配置
    let llm_config = build_llm_config_json().to_string();
    let work_dir = settingview::get_knowledge_bases_root();

    // agent_server 目录是 agent_server/agent.py 所在目录的父目录
    // （即项目根目录），PYTHONPATH 设为项目根以支持 import。
    let pythonpath = source_dir
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| source_dir.to_string_lossy().to_string());

    // 子进程 stdout/stderr 重定向到日志文件，启动失败时可读取尾部回传真实报错。
    let log_path = agent_server_log_path();
    let log_stdout = std::fs::File::create(&log_path)
        .map_err(|e| format!("创建 agent server 日志文件失败: {}", e))?;
    let log_stderr = log_stdout
        .try_clone()
        .map_err(|e| format!("克隆 agent server 日志句柄失败: {}", e))?;

    // 经 `python -m agent_server.main` 启动：其 __main__ 会把 uvicorn 放进放大栈的子线程运行，
    // 规避 Windows 内置 embeddable Python 主线程小栈导致的 0xC0000005 崩溃。
    // host/port 通过环境变量传入，与 Rust 侧常量保持一致。
    let mut cmd = Command::new(&python);
    cmd.args(&base_args)
        .args(["-m", "agent_server.main"])
        .env("PYTHONPATH", &pythonpath)
        .env("GUAXI_LLM_CONFIG", &llm_config)
        .env("GUAXI_WORK_DIR", &work_dir)
        .env("GUAXI_AGENT_HOST", "127.0.0.1")
        .env("GUAXI_AGENT_PORT", AGENT_SERVER_PORT.to_string())
        .env("PYTHONUTF8", "1")
        .current_dir(&pythonpath)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_stdout))
        .stderr(Stdio::from(log_stderr));
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
        // 子进程若已退出（如导入失败/端口占用），立即读取日志尾部报错，不必空等满超时。
        // 注意：std Mutex 守卫在该 if 块结束即释放，不会跨下方 await 持锁。
        if let Ok(mut guard) = AGENT_SERVER_PROCESS.lock() {
            if let Some(child) = guard.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let tail = read_log_tail(&log_path, 40);
                    return Err(format!(
                        "agent server 进程已退出（{}）。日志: {}\n{}",
                        status,
                        log_path.to_string_lossy(),
                        tail
                    ));
                }
            }
        }

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

    let tail = read_log_tail(&log_path, 40);
    Err(format!(
        "agent server 在 {} 秒内未就绪。日志: {}\n{}",
        AGENT_SERVER_STARTUP_TIMEOUT_SECS,
        log_path.to_string_lossy(),
        tail
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
