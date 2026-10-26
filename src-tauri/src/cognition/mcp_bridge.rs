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
//! 四条设计要点：
//! - **按字节读写，不按文本行**：JSONL 里的中文工具描述在 Windows 默认 GBK
//!   区域设置下会被 `BufRead::lines` / `writeln!` 按平台编码处理，写坏一个字节
//!   整条 JSON 就解析不了，codex 那边表现为「握手超时」。这里全程 `Vec<u8>`。
//! - **通知不回响应**：JSON-RPC 里没有 `id` 的消息是通知，主进程回 202 无体，此处不写 stdout。
//! - **主程序没起时给结构化错误**，而不是静默退出——否则 codex 只会显示"MCP 服务器挂了"。
//! - **绝不走系统代理**：目标是 loopback，走代理必然失败。

use std::io::{BufRead, Read, Write};

const ENDPOINT_ENV: &str = "GUAXI_MCP_ENDPOINT";
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8888/mcp";
/// 单行上限。MCP 的一条消息通常只有几 KB，但 `tools/call` 的返回可能带上
/// 截图之类的 base64；给到 16MB 既覆盖得住，又不至于被畸形输入拖垮内存。
const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;

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

/// 写一行 JSON 到 stdout，并立刻 flush。
///
/// 必须按字节写：MCP 帧是 UTF-8 JSON，而 Windows 控制台的代码页通常不是。
fn emit(out: &mut impl Write, line: &str) {
    let _ = out.write_all(line.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// stdout 是否接着真正的管道（而不是控制台）。
///
/// `guaxi --mcp-server` 走的是主程序本体：Windows release 构建是 GUI 子系统，
/// 若被误双击启动，stdout 会挂到控制台而不是 MCP 客户端。此时写 JSONL 毫无意义，
/// 还会和日志输出混在一起，所以提前退出并给出可读原因。
fn stdout_is_pipe() -> bool {
    use std::io::IsTerminal;
    !std::io::stdout().is_terminal()
}

/// 阻塞式读 stdin、逐行转发。主程序在 `--mcp-server` 模式下直接调用它，
/// 独立二进制 `guaxi_mcp` 也复用同一份实现。
pub fn run_stdio_bridge() {
    if !stdout_is_pipe() {
        eprintln!(
            "[guaxi-mcp] stdout 不是管道，拒绝作为 MCP 服务器运行。\
             这个模式应由 codex 等 MCP 客户端拉起，不要直接双击运行。"
        );
        std::process::exit(2);
    }

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
    let mut reader = stdin.lock();
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    loop {
        buf.clear();
        // 按字节读到换行：绕开平台编码，也不依赖 UTF-8 恰好是 ASCII 兼容这一点之外的假设。
        let read = match (&mut reader).take(MAX_LINE_BYTES as u64).read_until(b'\n', &mut buf) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("[guaxi-mcp] 读取 stdin 失败: {e}");
                break;
            }
        };
        if read == 0 {
            break; // EOF：客户端关了管道，正常退出。
        }

        let trimmed = String::from_utf8_lossy(&buf);
        let trimmed = trimmed.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                emit(
                    &mut writer,
                    &rpc_error(serde_json::Value::Null, -32700, format!("JSON 解析失败: {e}")),
                );
                continue;
            }
        };
        let id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let is_notification = parsed.get("id").is_none();

        // 转发原始字节而不是重新序列化：主进程收到的是客户端本来的那份 JSON，
        // 不会因为两侧 serde 配置差异丢掉未知字段。
        let resp = client
            .post(&url)
            .header("content-type", "application/json")
            .body(trimmed.as_bytes().to_vec())
            .send();
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
                let text = text.trim();
                if text.is_empty() {
                    emit(
                        &mut writer,
                        &rpc_error(id, -32603, format!("Guaxi 返回空响应（HTTP {status}）")),
                    );
                } else {
                    // 保序：主进程回什么就原样转发什么，不做再序列化。
                    emit(&mut writer, text);
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
                emit(&mut writer, &msg);
            }
        }
    }
}
