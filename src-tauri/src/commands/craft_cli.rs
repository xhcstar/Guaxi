//! Office 文档的 CLI 能力接入（WordCraft / GridCraft / DeckCraft）。
//!
//! 与 `craft_app`（负责「启动外部编辑器的 GUI」）互补：这里是**不打开界面**的用法——
//! 用各家的 CLI 读出结构信息、导出 PDF、抽取文本。两件事的收益不同：
//!
//! - 读结构：Guaxi 里能显示「这份文档有多少页/多少工作表/多少张幻灯片」，
//!   用户不必先打开一个重量级 GUI 才能知道。
//! - 导出 PDF：把 Office 文档变成可批注、可搜索的 PDF，落进同一个阅读器里。
//!
//! 设计取舍：
//! - **不猜 CLI 参数**：每个子命令都按各自 USAGE 的写法拼，参数名写死在常量里，
//!   上游改名时这里会立刻失败而不是静默给出错误结果。
//! - **超时**：CLI 解析畸形文件可能长时间不返回。统一在后台线程里跑并设上限，
//!   超时就杀掉，避免卡住一个阅读器窗口。
//! - **失败不外抛致命错误**：CLI 不可用是常态（用户可能只装了 GUI 没装 CLI），
//!   所以所有命令都返回结构化的 `{ ok, ... }`，由前端决定怎么呈现。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// CLI 子进程的超时上限。
///
/// 取 60s：正常文件都在几秒内返回；超过这个值基本是畸形文件或死循环，
/// 与其让用户对着转圈等，不如明确报「解析超时」。
const CLI_TIMEOUT: Duration = Duration::from_secs(60);

/// 支持的 CLI 工具。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CraftCli {
    Word,
    Grid,
    Deck,
}

impl CraftCli {
    fn from_kind(kind: &str) -> Option<Self> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "word" | "docx" | "doc" => Some(Self::Word),
            "grid" | "xlsx" | "xls" | "csv" | "ods" => Some(Self::Grid),
            "deck" | "pptx" | "ppt" | "odp" => Some(Self::Deck),
            _ => None,
        }
    }

    /// CLI 的可执行文件名（与 GUI 的 `<stem>.exe` 不同，CLI 带 `-cli` 后缀）。
    fn cli_stem(self) -> &'static str {
        match self {
            Self::Word => "wordcraft-cli",
            Self::Grid => "gridcraft-cli",
            Self::Deck => "deckcraft-cli",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Word => "WordCraft",
            Self::Grid => "GridCraft",
            Self::Deck => "DeckCraft",
        }
    }
}

/// CLI 调用的统一结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliResult {
    /// CLI 是否成功执行（退出码 0 且没有超时）
    pub ok: bool,
    /// stdout（成功时是命令输出，失败时可能为空）
    pub stdout: String,
    /// stderr
    pub stderr: String,
    /// 退出码（超时为 None）
    pub code: Option<i32>,
    /// 人类可读的说明：超时、找不到 CLI、非零退出等
    pub message: String,
}

impl CliResult {
    fn failure(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            stdout: String::new(),
            stderr: String::new(),
            code: None,
            message: message.into(),
        }
    }
}

/// 找 CLI 可执行文件。
///
/// 查找顺序与 `craft_app::find_executable` 完全一致（环境变量 → 应用目录 craft/ → PATH），
/// 但文件名带 `-cli` 后缀。这样用户只需要把三个工具解压到同一个 craft/ 目录，
/// GUI 与 CLI 就都能被找到。
fn find_cli(app: CraftCli) -> Option<PathBuf> {
    let stem = app.cli_stem();
    let exe_name = if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    };

    if let Ok(dir) = std::env::var("GUAXI_CRAFT_DIR") {
        let candidate = Path::new(dir.trim()).join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("craft").join(&exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 执行一次 CLI 调用（带超时）。
///
/// 超时实现：把子进程放到独立线程里 `wait_with_output`，主线程轮询加截止时间。
/// 超时后 `kill()` —— 三个 CLI 都是纯计算，没有需要优雅关闭的外部资源。
fn run_cli(exe: &Path, args: &[String]) -> CliResult {
    info!(
        exe = %exe.display(),
        args = ?args,
        "[CraftCli] running"
    );

    let mut child = match Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return CliResult::failure(format!("启动 {} 失败：{}", exe.display(), e));
        }
    };

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程已退出：读出全部输出。
                let output = child.wait_with_output();
                return match output {
                    Ok(out) => {
                        let code = status.code();
                        let ok = status.success();
                        CliResult {
                            ok,
                            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                            code,
                            message: if ok {
                                String::new()
                            } else {
                                format!("{} 退出码 {}", exe.display(), code.unwrap_or(-1))
                            },
                        }
                    }
                    Err(e) => CliResult::failure(format!("读取 CLI 输出失败：{}", e)),
                };
            }
            Ok(None) => {
                if started.elapsed() > CLI_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    warn!(exe = %exe.display(), "[CraftCli] timeout");
                    return CliResult::failure(format!(
                        "{} 解析超时（超过 {} 秒）",
                        exe.display(),
                        CLI_TIMEOUT.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(e) => {
                let _ = child.kill();
                return CliResult::failure(format!("等待 CLI 失败：{}", e));
            }
        }
    }
}

/// 解析 `kind` 与文件路径，解析失败时给出可直接展示给用户的中文说明。
fn resolve(app_kind: &str, path: &str) -> Result<(CraftCli, PathBuf, PathBuf), String> {
    let file = Path::new(path.trim()).to_path_buf();
    if !file.is_file() {
        return Err(format!("文件不存在：{}", file.display()));
    }
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let app = CraftCli::from_kind(&ext)
        .or_else(|| CraftCli::from_kind(app_kind))
        .ok_or_else(|| format!("不支持的文件类型：{}", ext))?;
    let exe = find_cli(app).ok_or_else(|| {
        format!(
            "未找到 {} CLI（{}）。\n\n请把 {} 解压到 Guaxi 程序目录下的 craft/ 子目录，\
或设置环境变量 GUAXI_CRAFT_DIR。",
            app.display_name(),
            app.cli_stem(),
            app.display_name()
        )
    })?;
    Ok((app, file, exe))
}

/// 读取文档结构信息（各家的 `info` 子命令，输出 JSON）。
///
/// 返回的是**原始 JSON 字符串**而不是解析后的结构：三家格式不同、且还在演进，
/// 把它反序列化成我们自己的结构等于给上游每个字段变化都写一遍兼容代码。
/// 前端只需要展示摘要，直接读 JSON 里的几个字段即可。
#[tauri::command]
pub async fn craft_cli_info(path: String, kind: String) -> Result<CliResult, String> {
    let (app, file, exe) = match resolve(&kind, &path) {
        Ok(v) => v,
        Err(msg) => return Ok(CliResult::failure(msg)),
    };

    let mut args: Vec<String> = vec!["info".into(), file.to_string_lossy().to_string()];
    // GridCraft 的 info 需要 --json 才是机器可读输出；另外两家默认就是 JSON。
    if app == CraftCli::Grid {
        args.push("--json".into());
    }

    let mut result = tauri::async_runtime::spawn_blocking(move || run_cli(&exe, &args))
        .await
        .map_err(|e| format!("CLI 任务失败：{}", e))?;

    if result.ok && result.stdout.trim().is_empty() {
        result.ok = false;
        result.message = "CLI 没有返回内容".to_string();
    }
    Ok(result)
}

/// 把文档导出成 PDF（三家都支持 `convert IN OUT`）。
///
/// 输出路径由调用方给定。导出成功后 Guaxi 可以直接用现成的 PDF 阅读器打开它 ——
/// 搜索、大纲、批注这些能力随之全部可用，比在阅读器里重写一遍 Office 渲染划算得多。
#[tauri::command]
pub async fn craft_cli_export_pdf(
    path: String,
    kind: String,
    output: String,
) -> Result<CliResult, String> {
    let (_, file, exe) = match resolve(&kind, &path) {
        Ok(v) => v,
        Err(msg) => return Ok(CliResult::failure(msg)),
    };

    let out = PathBuf::from(output.trim());
    if out.as_os_str().is_empty() {
        return Ok(CliResult::failure("输出路径为空"));
    }
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Ok(CliResult::failure(format!("创建输出目录失败：{}", e)));
            }
        }
    }

    let args: Vec<String> = vec![
        "convert".into(),
        file.to_string_lossy().to_string(),
        out.to_string_lossy().to_string(),
    ];

    let result = tauri::async_runtime::spawn_blocking(move || run_cli(&exe, &args))
        .await
        .map_err(|e| format!("CLI 任务失败：{}", e))?;

    if result.ok && !out.is_file() {
        // 退出码 0 但没有产物：说明上游改了行为，明确报出来而不是让前端去读一个不存在的文件。
        return Ok(CliResult {
            ok: false,
            message: format!("CLI 报告成功但未生成 {}", out.display()),
            ..result
        });
    }
    if result.ok {
        info!(output = %out.display(), "[CraftCli] exported pdf");
    }
    Ok(result)
}

/// 抽取出纯文本（WordCraft 的 `text`；另两家用 convert 到 .txt）。
#[tauri::command]
pub async fn craft_cli_text(path: String, kind: String) -> Result<CliResult, String> {
    let (app, file, exe) = match resolve(&kind, &path) {
        Ok(v) => v,
        Err(msg) => return Ok(CliResult::failure(msg)),
    };

    if app == CraftCli::Word {
        let args: Vec<String> = vec!["text".into(), file.to_string_lossy().to_string()];
        return tauri::async_runtime::spawn_blocking(move || run_cli(&exe, &args))
            .await
            .map_err(|e| format!("CLI 任务失败：{}", e));
    }

    // Grid/Deck 没有专门的 text 子命令，转成临时 .txt 再读回来。
    let tmp = std::env::temp_dir().join(format!(
        "guaxi-craft-text-{}.txt",
        std::process::id()
    ));
    let args: Vec<String> = vec![
        "convert".into(),
        file.to_string_lossy().to_string(),
        tmp.to_string_lossy().to_string(),
    ];
    let result = tauri::async_runtime::spawn_blocking(move || {
        let r = run_cli(&exe, &args);
        let text = if r.ok {
            std::fs::read_to_string(&tmp).unwrap_or_default()
        } else {
            String::new()
        };
        let _ = std::fs::remove_file(&tmp);
        (r, text)
    })
    .await
    .map_err(|e| format!("CLI 任务失败：{}", e))?;

    let (mut r, text) = result;
    if r.ok {
        r.stdout = text;
    }
    Ok(r)
}

/// 探测三个 CLI 是否可用（前端据此决定是否显示「用 CLI 导出 PDF」等入口）。
#[tauri::command]
pub fn probe_craft_clis() -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for app in [CraftCli::Word, CraftCli::Grid, CraftCli::Deck] {
        let found = find_cli(app);
        if found.is_none() {
            warn!(cli = app.cli_stem(), "[CraftCli] not found");
        }
        out.insert(
            app.cli_stem().to_string(),
            serde_json::json!({
                "name": app.display_name(),
                "available": found.is_some(),
                "path": found.map(|p| p.to_string_lossy().to_string()),
            }),
        );
    }
    serde_json::Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::CraftCli;

    #[test]
    fn maps_extensions_to_the_right_tool() {
        assert_eq!(CraftCli::from_kind("docx"), Some(CraftCli::Word));
        assert_eq!(CraftCli::from_kind("DOC"), Some(CraftCli::Word));
        assert_eq!(CraftCli::from_kind("xlsx"), Some(CraftCli::Grid));
        assert_eq!(CraftCli::from_kind("ods"), Some(CraftCli::Grid));
        assert_eq!(CraftCli::from_kind("pptx"), Some(CraftCli::Deck));
        assert_eq!(CraftCli::from_kind("pdf"), None, "PDF 不该路由到任何 CLI");
        assert_eq!(CraftCli::from_kind(""), None);
    }

    #[test]
    fn cli_stems_have_the_cli_suffix() {
        // GUI 与 CLI 的可执行文件名不同（后者带 -cli）；写错就找不到程序。
        for app in [CraftCli::Word, CraftCli::Grid, CraftCli::Deck] {
            assert!(
                app.cli_stem().ends_with("-cli"),
                "{} 的 CLI 名应以 -cli 结尾",
                app.display_name()
            );
        }
    }
}
