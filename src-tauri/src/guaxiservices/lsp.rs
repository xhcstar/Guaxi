use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct LspCommandConfig {
    program: String,
    args: Vec<String>,
}

fn resolve_lsp_binary(name: &str) -> String {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let base_dir = exe_path
        .parent()
        .unwrap_or(&exe_path)
        .to_path_buf();

    #[cfg(target_os = "windows")]
    let file_name = if std::path::Path::new(name).extension().is_some() {
        name.to_string()
    } else {
        format!("{}.exe", name)
    };
    #[cfg(not(target_os = "windows"))]
    let file_name = name.to_string();

    let mut candidates = vec![
        base_dir.join("lsp").join(&file_name),
        base_dir.join("utils").join("lsp").join(&file_name),
    ];

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        candidates.push(
            std::path::PathBuf::from(manifest_dir)
                .join("utils")
                .join("lsp")
                .join(&file_name),
        );
    }

    for p in candidates {
        if p.exists() {
            return p.to_string_lossy().into_owned();
        }
    }

    file_name
}

fn lsp_command_for_language(language: &str) -> Option<LspCommandConfig> {
    match language {
        "cpp" | "c" => Some(LspCommandConfig {
            program: resolve_lsp_binary("clangd"),
            args: vec!["--enable-config".to_string()],
        }),
        "rust" => Some(LspCommandConfig {
            program: resolve_lsp_binary("rust-analyzer"),
            args: vec![],
        }),
        "python" => Some(LspCommandConfig {
            program: resolve_lsp_binary("pyright-ls"),
            args: vec!["--stdio".to_string()],
        }),
        "shell" => Some(LspCommandConfig {
            program: resolve_lsp_binary("bash-ls"),
            args: vec!["start".to_string()],
        }),
        "lua" => Some(LspCommandConfig {
            program: resolve_lsp_binary("lua-language-server"),
            args: vec![],
        }),
        "java" => Some(LspCommandConfig {
            program: resolve_lsp_binary("jdtls"),
            args: vec![],
        }),
        "javascript" | "typescript" => Some(LspCommandConfig {
            program: resolve_lsp_binary("typescript-ls"),
            args: vec!["--stdio".to_string()],
        }),
        "html" => Some(LspCommandConfig {
            program: resolve_lsp_binary("html-ls"),
            args: vec!["--stdio".to_string()],
        }),
        "markdown" => Some(LspCommandConfig {
            program: resolve_lsp_binary("markdown-ls"),
            args: vec!["--stdio".to_string()],
        }),
        "latex" => Some(LspCommandConfig {
            program: resolve_lsp_binary("texlab"),
            args: vec![],
        }),
        _ => None,
    }
}

pub struct LspWsSession {
    language: String,
    child: Option<Child>,
    child_stdin: Option<Arc<Mutex<ChildStdin>>>,
}

impl LspWsSession {
    pub fn new(language: String) -> Self {
        Self {
            language,
            child: None,
            child_stdin: None,
        }
    }
}

impl Actor for LspWsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        if let Some(cfg) = lsp_command_for_language(&self.language) {
            let addr = ctx.address();
            actix::spawn(async move {
                let program_for_log = cfg.program.clone();
                let mut cmd = Command::new(cfg.program);
                cmd.args(cfg.args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());

                match cmd.spawn() {
                    Ok(mut child) => {
                        let stdout = child.stdout.take();
                        let stdin = child.stdin.take();

                        let stdin_arc = stdin.map(|s| Arc::new(Mutex::new(s)));

                        if let Some(mut stdout) = stdout {
                            let addr_clone = addr.clone();
                            actix::spawn(async move {
                                let mut buf = vec![0u8; 8192];
                                let mut pending = Vec::<u8>::new();
                                loop {
                                    match stdout.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            pending.extend_from_slice(&buf[..n]);

                                            loop {
                                                let Some((header_end, content_len)) =
                                                    parse_lsp_frame_header(&pending)
                                                else {
                                                    break;
                                                };

                                                let frame_end = header_end + content_len;
                                                if pending.len() < frame_end {
                                                    break;
                                                }

                                                let payload =
                                                    pending[header_end..frame_end].to_vec();
                                                pending.drain(0..frame_end);

                                                if let Ok(data) = String::from_utf8(payload) {
                                                    addr_clone.do_send(LspStdoutMessage { data });
                                                }
                                            }

                                            if pending.len() > 1024 * 1024 {
                                                pending.clear();
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }

                        addr.do_send(LspChildStarted { child, stdin: stdin_arc });
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to start LSP process ({}): {:?}",
                            program_for_log, e
                        );
                        let _ = addr.try_send(LspProcessFailed);
                    }
                }
            });
        } else {
            eprintln!("Unsupported language for LSP: {}", self.language);
            ctx.close(None);
        }

        ctx.run_interval(Duration::from_secs(30), |_act, ctx| {
            ctx.ping(b"ping");
        });
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(mut child) = self.child.take() {
            actix::spawn(async move {
                let _ = child.kill().await;
            });
        }
    }
}

struct LspChildStarted {
    child: Child,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
}

impl Message for LspChildStarted {
    type Result = ();
}

impl Handler<LspChildStarted> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: LspChildStarted, _ctx: &mut Self::Context) {
        self.child = Some(msg.child);
        self.child_stdin = msg.stdin;
    }
}

struct LspStdoutMessage {
    data: String,
}

impl Message for LspStdoutMessage {
    type Result = ();
}

impl Handler<LspStdoutMessage> for LspWsSession {
    type Result = ();

    fn handle(&mut self, msg: LspStdoutMessage, ctx: &mut Self::Context) {
        ctx.text(msg.data);
    }
}

struct LspProcessFailed;

impl Message for LspProcessFailed {
    type Result = ();
}

impl Handler<LspProcessFailed> for LspWsSession {
    type Result = ();

    fn handle(&mut self, _msg: LspProcessFailed, ctx: &mut Self::Context) {
        ctx.close(None);
        ctx.stop();
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for LspWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                if let Some(stdin_arc) = &self.child_stdin {
                    let payload = text.into_bytes();
                    let stdin_clone = stdin_arc.clone();
                    actix::spawn(async move {
                        let mut guard = stdin_clone.lock().await;
                        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
                        let _ = guard.write_all(header.as_bytes()).await;
                        let _ = guard.write_all(&payload).await;
                        let _ = guard.flush().await;
                    });
                }
            }
            Ok(ws::Message::Binary(_bin)) => {}
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Pong(_)) => {}
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

pub async fn lsp_ws_route(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, Error> {
    let language = path.into_inner().0;
    let session = LspWsSession::new(language);
    ws::start(session, &req, stream)
}

fn parse_lsp_frame_header(bytes: &[u8]) -> Option<(usize, usize)> {
    let sep = bytes.windows(4).position(|w| w == b"\r\n\r\n")?;
    let header_end = sep + 4;
    let header = std::str::from_utf8(&bytes[..sep]).ok()?;

    let mut content_len = None;
    for line in header.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            if let Ok(v) = rest.trim().parse::<usize>() {
                content_len = Some(v);
                break;
            }
        }
    }

    content_len.map(|len| (header_end, len))
}
