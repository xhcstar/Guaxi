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
    if cfg!(debug_assertions) {
        return name.to_string();
    }

    let exe_path = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let base_dir = exe_path
        .parent()
        .unwrap_or(&exe_path)
        .to_path_buf();

    #[cfg(target_os = "windows")]
    let file_name = format!("{}.exe", name);
    #[cfg(not(target_os = "windows"))]
    let file_name = name.to_string();

    base_dir
        .join("lsp")
        .join(file_name)
        .to_string_lossy()
        .into_owned()
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
            program: resolve_lsp_binary("pyright-langserver"),
            args: vec!["--stdio".to_string()],
        }),
        "shell" => Some(LspCommandConfig {
            program: resolve_lsp_binary("bash-language-server"),
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
            program: resolve_lsp_binary("typescript-language-server"),
            args: vec!["--stdio".to_string()],
        }),
        "html" => Some(LspCommandConfig {
            program: resolve_lsp_binary("vscode-html-language-server"),
            args: vec!["--stdio".to_string()],
        }),
        "markdown" => Some(LspCommandConfig {
            program: resolve_lsp_binary("markdown-language-server"),
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
                                loop {
                                    match stdout.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            let data =
                                                String::from_utf8_lossy(&buf[..n]).to_string();
                                            addr_clone.do_send(LspStdoutMessage { data });
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }

                        addr.do_send(LspChildStarted { child, stdin: stdin_arc });
                    }
                    Err(_) => {
                        let _ = addr.try_send(LspStdoutMessage {
                            data: "LSP process start failed".to_string(),
                        });
                    }
                }
            });
        } else {
            ctx.text(format!("Unsupported language: {}", self.language));
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

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for LspWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                if let Some(stdin_arc) = &self.child_stdin {
                    let data = text.into_bytes();
                    let stdin_clone = stdin_arc.clone();
                    actix::spawn(async move {
                        let mut guard = stdin_clone.lock().await;
                        let _ = guard.write_all(&data).await;
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