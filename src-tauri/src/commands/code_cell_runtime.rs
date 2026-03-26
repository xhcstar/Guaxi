use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rquickjs::{Context, Function, Runtime, Value as JsValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use tauri::command;
use tracing::{error, info, warn};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Deserialize)]
pub struct CodeCellExecRequest {
    pub kind: String,
    pub code: String,
    pub vars: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize)]
pub struct CodeCellArtifact {
    pub kind: String,
    pub name: Option<String>,
    pub mime: Option<String>,
    pub data_base64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CodeCellExecResponse {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub result_repr: Option<String>,
    pub artifacts: Option<Vec<CodeCellArtifact>>,
}

fn norm_kind(s: &str) -> String {
    s.trim().to_lowercase()
}

fn var_str(vars: &Option<HashMap<String, Value>>, key: &str) -> Option<String> {
    vars.as_ref()
        .and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn output_to_text(out: Output) -> (bool, String, String) {
    let ok = out.status.success();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (ok, stdout, stderr)
}

fn apply_hidden_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

fn exec_javascript(code: &str) -> Result<CodeCellExecResponse, String> {
    let rt = Runtime::new().map_err(|e| format!("Runtime::new failed: {e}"))?;
    let ctx = Context::full(&rt).map_err(|e| format!("Context::full failed: {e}"))?;

    let out_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let err_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

    let mut ok = true;
    let mut result_repr: Option<String> = None;

    ctx.with(|ctx| {
        let g = ctx.globals();

        let out_for_print = out_buf.clone();
        let print = Function::new(ctx.clone(), move |s: String| {
            if let Ok(mut guard) = out_for_print.lock() {
                guard.push_str(&s);
                guard.push('\n');
            }
        })
        .map_err(|e| format!("create __guaxi_print failed: {e}"))?;
        g.set("__guaxi_print", print)
            .map_err(|e| format!("set __guaxi_print failed: {e}"))?;

        ctx.eval::<(), _>(
            r#"globalThis.console = {
  log: (...v) => globalThis.__guaxi_print(v.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' ')),
  warn: (...v) => globalThis.__guaxi_print(v.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' ')),
  error: (...v) => globalThis.__guaxi_print(v.map(x => typeof x === 'string' ? x : JSON.stringify(x)).join(' '))
};

globalThis.__guaxi_to_string = (v) => {
  try {
    if (v === undefined) return '';
    return String(v);
  } catch (e) {
    try { return '[toString failed] ' + String(e); } catch (_) { return '[toString failed]'; }
  }
};"#,
        )
        .map_err(|e| format!("init console failed: {e}"))?;

        let v: Result<JsValue, _> = ctx.eval(code);
        match v {
            Ok(val) => {
                while ctx.execute_pending_job() {}
                if !val.is_undefined() {
                    match g.get::<_, Function>("__guaxi_to_string") {
                        Ok(to_str) => match to_str.call::<_, String>((val,)) {
                            Ok(s) => {
                                let t = s.trim();
                                if !t.is_empty() {
                                    result_repr = Some(t.to_string());
                                }
                            }
                            Err(e) => {
                                warn!(err = %e, "[CodeCell] js stringify result failed");
                            }
                        },
                        Err(e) => {
                            warn!(err = %e, "[CodeCell] get __guaxi_to_string failed");
                        }
                    }
                }
            }
            Err(e) => {
                ok = false;
                if let Ok(mut guard) = err_buf.lock() {
                    guard.push_str(&format!("{e}"));
                }
            }
        }

        Ok::<(), String>(())
    })?;

    let stdout = out_buf.lock().ok().map(|g| g.clone()).unwrap_or_default();
    let stderr = err_buf.lock().ok().map(|g| g.clone()).unwrap_or_default();

    Ok(CodeCellExecResponse {
        ok,
        stdout,
        stderr,
        result_repr,
        artifacts: None,
    })
}

fn find_tectonic_exe() -> Result<(PathBuf, bool, PathBuf), String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to get current exe path: {e}"))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to get parent directory of exe".to_string())?
        .to_path_buf();

    #[cfg(windows)]
    {
        let bundled = exe_dir.join("utils").join("tectonic.exe");
        if bundled.is_file() {
            return Ok((bundled, true, exe_dir));
        }
        return Ok((PathBuf::from("tectonic"), false, exe_dir));
    }

    #[cfg(not(windows))]
    {
        let bundled = exe_dir.join("utils").join("tectonic");
        if bundled.is_file() {
            return Ok((bundled, true, exe_dir));
        }
        return Ok((PathBuf::from("tectonic"), false, exe_dir));
    }
}

fn exec_latex(code: &str) -> Result<CodeCellExecResponse, String> {
    let (tectonic_path, is_bundled, exe_dir) = find_tectonic_exe()?;

    let dir = tempfile::tempdir().map_err(|e| format!("tempdir failed: {e}"))?;
    let base = dir.path();

    let tex_path = base.join("main.tex");
    fs::write(&tex_path, code).map_err(|e| format!("write tex failed: {e}"))?;

    let outdir = base.join("out");
    fs::create_dir_all(&outdir).map_err(|e| format!("create outdir failed: {e}"))?;

    info!(
        tectonic = %tectonic_path.display(),
        bundled = is_bundled,
        tex = %tex_path.display(),
        outdir = %outdir.display(),
        "[CodeCell] running tectonic"
    );

    let mut cmd = Command::new(&tectonic_path);
    apply_hidden_window(&mut cmd);
    cmd.current_dir(base)
        .arg("-X")
        .arg("compile")
        .arg("--outdir")
        .arg(&outdir)
        .arg("--untrusted")
        .arg(&tex_path);

    let out = cmd.output().map_err(|e| {
        format!(
            "spawn tectonic failed: {e} (path={})",
            tectonic_path.display()
        )
    })?;

    let (ok, stdout, stderr) = output_to_text(out);

    let pdf_path = outdir.join("main.pdf");
    if !ok {
        warn!(
            stdout_len = stdout.len(),
            stderr_len = stderr.len(),
            pdf_exists = pdf_path.is_file(),
            "[CodeCell] tectonic failed"
        );
        return Ok(CodeCellExecResponse {
            ok: false,
            stdout,
            stderr: if is_bundled {
                stderr
            } else {
                {
                    #[cfg(windows)]
                    {
                        format!(
                            "{stderr}\n\nHint: 未找到内置 tectonic.exe（期望位置：{}）。你可以把 Windows 版 tectonic.exe 放到 src-tauri\\utils\\tectonic.exe 后重新运行。",
                            exe_dir.join("utils").join("tectonic.exe").to_string_lossy()
                        )
                    }
                    #[cfg(not(windows))]
                    {
                        format!(
                            "{stderr}\n\nHint: 未找到内置 tectonic（期望位置：{}）。你可以把 Linux 版 tectonic 放到 src-tauri/utils/tectonic，并确保它有可执行权限（chmod +x）。",
                            exe_dir.join("utils").join("tectonic").to_string_lossy()
                        )
                    }
                }
            },
            result_repr: None,
            artifacts: None,
        });
    }

    if !pdf_path.is_file() {
        return Ok(CodeCellExecResponse {
            ok: false,
            stdout,
            stderr: format!(
                "tectonic finished but PDF not found: {}",
                pdf_path.to_string_lossy()
            ),
            result_repr: None,
            artifacts: None,
        });
    }

    let pdf_bytes = fs::read(&pdf_path).map_err(|e| format!("read pdf failed: {e}"))?;
    let b64 = STANDARD.encode(&pdf_bytes);

    info!(bytes = pdf_bytes.len(), "[CodeCell] tectonic produced pdf");

    Ok(CodeCellExecResponse {
        ok: true,
        stdout,
        stderr,
        result_repr: None,
        artifacts: Some(vec![CodeCellArtifact {
            kind: "pdf".to_string(),
            name: Some("main.pdf".to_string()),
            mime: Some("application/pdf".to_string()),
            data_base64: Some(b64),
        }]),
    })
}

fn compile_and_run_native(kind: &str, code: &str) -> Result<CodeCellExecResponse, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir failed: {e}"))?;
    let base = dir.path();

    let (src_name, candidates, build_args): (
        &str,
        Vec<&str>,
        Box<dyn Fn(&str, &PathBuf, &PathBuf) -> Vec<String>>,
    ) = match kind {
        "c" => (
            "main.c",
            vec!["clang", "gcc", "cc"],
            Box::new(|_cc, src, out| {
                vec![
                    src.display().to_string(),
                    "-O0".into(),
                    "-g".into(),
                    "-o".into(),
                    out.display().to_string(),
                ]
            }),
        ),
        "cpp" => (
            "main.cpp",
            vec!["clang++", "g++", "c++"],
            Box::new(|_cc, src, out| {
                vec![
                    src.display().to_string(),
                    "-std=c++17".into(),
                    "-O0".into(),
                    "-g".into(),
                    "-o".into(),
                    out.display().to_string(),
                ]
            }),
        ),
        "rust" => (
            "main.rs",
            vec!["rustc"],
            Box::new(|_cc, src, out| {
                vec![
                    src.display().to_string(),
                    "-o".into(),
                    out.display().to_string(),
                ]
            }),
        ),
        _ => return Err(format!("unsupported native kind: {kind}")),
    };

    let src_path = base.join(src_name);
    fs::write(&src_path, code).map_err(|e| format!("write source failed: {e}"))?;

    #[cfg(windows)]
    let out_path = base.join("a.exe");
    #[cfg(not(windows))]
    let out_path = base.join("a");

    let mut last_compile_err = String::new();
    let mut compiled = false;

    for cc in candidates {
        let mut cmd = Command::new(cc);
        apply_hidden_window(&mut cmd);
        cmd.args(build_args(cc, &src_path, &out_path));
        match cmd.output() {
            Ok(out) => {
                let (ok, _stdout, stderr) = output_to_text(out);
                if ok {
                    compiled = true;
                    info!(kind = kind, compiler = cc, "[CodeCell] compile ok");
                    break;
                }
                last_compile_err = stderr;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(e) => {
                last_compile_err = format!("{e}");
            }
        }
    }

    if !compiled {
        if last_compile_err.trim().is_empty() {
            return Err(format!(
                "compiler not found for {kind} (need clang/gcc/g++/rustc in PATH)"
            ));
        }
        return Ok(CodeCellExecResponse {
            ok: false,
            stdout: String::new(),
            stderr: last_compile_err,
            result_repr: None,
            artifacts: None,
        });
    }

    let mut run_cmd = Command::new(&out_path);
    apply_hidden_window(&mut run_cmd);
    let run_out = run_cmd.output().map_err(|e| format!("run failed: {e}"))?;
    let (run_ok, stdout, stderr) = output_to_text(run_out);

    Ok(CodeCellExecResponse {
        ok: run_ok,
        stdout,
        stderr,
        result_repr: None,
        artifacts: None,
    })
}

#[command]
pub async fn code_cell_exec(request: CodeCellExecRequest) -> Result<CodeCellExecResponse, String> {
    let kind = norm_kind(&request.kind);
    let kind_for_log = kind.clone();
    let kind_for_task = kind.clone();

    let code_len = request.code.len();

    let doc_path = var_str(&request.vars, "_doc_path").unwrap_or_default();
    let cell_id = var_str(&request.vars, "_cell_id").unwrap_or_default();

    info!(
        kind = kind.as_str(),
        code_len = code_len,
        doc_path = doc_path.as_str(),
        cell_id = cell_id.as_str(),
        "[CodeCell] code_cell_exec called"
    );

    let task = tokio::task::spawn_blocking(move || -> Result<CodeCellExecResponse, String> {
        match kind_for_task.as_str() {
            "javascript" | "js" => exec_javascript(&request.code),
            "latex" | "tex" => exec_latex(&request.code),
            "c" => compile_and_run_native("c", &request.code),
            "cpp" | "c++" => compile_and_run_native("cpp", &request.code),
            "rust" | "rs" => compile_and_run_native("rust", &request.code),
            other => {
                warn!(kind = other, "[CodeCell] unsupported kind");
                Err(format!("unsupported kind: {other}"))
            }
        }
    });

    task.await
        .map_err(|e| format!("code_cell_exec task join failed: {e}"))?
        .map_err(|e| {
            error!(
                kind = kind_for_log.as_str(),
                err = e.as_str(),
                "[CodeCell] exec failed"
            );
            e
        })
}
