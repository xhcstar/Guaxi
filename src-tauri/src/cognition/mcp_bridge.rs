//! stdio ⇄ HTTP 的 MCP 桥接主体。
//!
//! 这段逻辑原来只活在独立的 `guaxi_mcp` 二进制里。问题在于开发构建与便携目录
//! 里经常没有那个旁挂文件，`config.toml` 就写不进 `[mcp_servers.guaxi]`，
//! codex 于是只能看见文件系统。现在主程序自己也能充当桥接进程
//! （`guaxi --mcp-server`），`codex_agent` 在找不到旁挂二进制时会退到这条路径。
//!
//! ```text
//! codex ──stdio(JSONL)──► guaxi --mcp-server ──HTTP POST──► 主进程 /mcp
//! ```
//!
//! 三条设计要点：
//! - **通知不回响应**：JSON-RPC 里没有 `id` 的消息是通知，主进程回 202 无体，此处不写 stdout。
//! - **主程序没起时给结构化错误**，而不是静默退出——否则 codex 只会显示"MCP 服务器挂了"。
//! - **绝不走系统代理**：目标是 loopback，走代理必然失败。

use std::io::{BufRead, Write};

const ENDPOINT_ENV: &str = "GUAXI_MCP_ENDPOINT";
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8888/mcp";

fn endpoint() -> String {
    std::env::var(ENDPOINT_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
}

fn rpc_error(id: serde_json::Value, code: i64, message: String) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

/// 阻塞式读 stdin、逐行转发。主程序在 `--mcp-server` 模式下直接调用它，
/// 独立二进制 `guaxi_mcp` 也复用同一份实现。
pub fn run_stdio_bridge() {
    let url = endpoint();
    let client = match reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(180))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[guaxi-mcp] HTTP 客户端初始化失败: {e}");
            std::process::exit(1);
        }
    };

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[guaxi-mcp] 读取 stdin 失败: {e}");
                break;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let msg = rpc_error(serde_json::Value::Null, -32700, format!("JSON 解析失败: {e}"));
                let _ = writeln!(stdout, "{msg}");
                let _ = stdout.flush();
                continue;
            }
        };
        let id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let is_notification = parsed.get("id").is_none();

        let resp = client.post(&url).json(&parsed).send();
        match resp {
            Ok(r) => {
                let status = r.status();
                // 202 = 通知已受理，无响应体。
                if status.as_u16() == 202 {
                    continue;
                }
                let text = r.text().unwrap_or_default();
                if is_notification {
                    continue;
                }
                if text.trim().is_empty() {
                    let msg = rpc_error(id, -32603, format!("Guaxi 返回空响应（HTTP {status}）"));
                    let _ = writeln!(stdout, "{msg}");
                } else {
                    let _ = writeln!(stdout, "{}", text.trim());
                }
            }
            Err(e) => {
                if is_notification {
                    continue;
                }
                let msg = rpc_error(
                    id,
                    -32603,
                    format!(
                        "无法连接 Guaxi 主程序（{url}）：{e}。请确认 Guaxi 正在运行；\
                         端口不同可用环境变量 {ENDPOINT_ENV} 覆盖。"
                    ),
                );
                let _ = writeln!(stdout, "{msg}");
            }
        }
        let _ = stdout.flush();
    }
}
