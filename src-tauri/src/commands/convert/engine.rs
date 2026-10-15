//! 引擎解析、探针与转换执行。
//!
//! 各引擎共用同一套“路径解析 + 环境变量 + JSON 进度协议”约定：
//! - fast  : 托管 Python 中的 `guaxi_fast_convert.py`（pymupdf4llm / markitdown）
//! - marker: 独立的 uv 托管 venv 中的 `guaxi_marker_convert.py`（源码取自 Guaxi fork）
//! - mineru: 独立 venv `utils/MinerU/.venv` 中的 `guaxi_mineru_convert.py`
//!
//! 第四个引擎 anydoc 是纯 Rust 库，不上子进程、无需装配，由 [`convert_with_anydoc`]
//! 在本进程内完成转换（queue.rs 里单独分支处理，不走 [`build_plan`]）。
//!
//! marker 相关解析逻辑与 `docprocess.rs` 中的私有实现保持一致（此处独立一份，避免改动旧文件签名）。

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio::process::Command as TokioCommand;
use tracing::{info, warn};

use crate::guaxiservices::bundled_resources;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engine {
    Fast,
    Marker,
    Mineru,
    /// 纯 Rust 的 anydoc 库，进程内执行、无需装配（仅显式选择时使用，auto 不会路由到它）。
    Anydoc,
}

impl Engine {
    pub fn as_str(&self) -> &'static str {
        match self {
            Engine::Fast => "fast",
            Engine::Marker => "marker",
            Engine::Mineru => "mineru",
            Engine::Anydoc => "anydoc",
        }
    }

    pub fn from_str(s: &str) -> Option<Engine> {
        match s.to_ascii_lowercase().as_str() {
            "fast" => Some(Engine::Fast),
            "marker" => Some(Engine::Marker),
            "mineru" => Some(Engine::Mineru),
            "anydoc" => Some(Engine::Anydoc),
            _ => None,
        }
    }
}

// ───────────────────────── marker 路径解析（与 docprocess 保持一致） ─────────────────────────

pub fn resolve_marker_source_dir() -> Result<PathBuf, String> {
    let p = bundled_resources::ensure_relative_dir("utils/MarkdownConvertor/marker-1.10.1")?;
    if p.exists() {
        return Ok(p);
    }
    if cfg!(debug_assertions) {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("Convertor")
            .join("marker-1.10.1");
        if p.exists() {
            return Ok(p);
        }
        // 兼容另一种工作区布局
        let p2 = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("marker-1.10.1");
        if p2.exists() {
            return Ok(p2);
        }
    }
    Err("找不到 marker-1.10.1 源码目录".to_string())
}

fn venv_python(venv_dir: &Path) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if cfg!(windows) {
        vec![
            venv_dir.join("Scripts").join("python.exe"),
            venv_dir.join("python.exe"),
        ]
    } else {
        vec![venv_dir.join("bin").join("python")]
    };
    candidates.into_iter().find(|p| p.exists())
}

pub fn resolve_marker_python(marker_src_dir: &Path) -> Result<PathBuf, String> {
    venv_python(&marker_src_dir.join(".venv"))
        .ok_or_else(|| format!("找不到 marker venv：{}", marker_src_dir.join(".venv").display()))
}

/// Marker 强引擎是否可用（源码目录 + venv + convert_single.py 均在）。
pub fn marker_available() -> bool {
    match resolve_marker_source_dir() {
        Ok(dir) => resolve_marker_python(&dir).is_ok() && dir.join("convert_single.py").exists(),
        Err(_) => false,
    }
}

// ───────────────────────── MinerU 路径解析（可选，缺失时回退 marker） ─────────────────────────

pub fn resolve_mineru_dir() -> Option<PathBuf> {
    if let Ok(p) = bundled_resources::ensure_relative_dir("utils/MinerU") {
        if p.exists() {
            return Some(p);
        }
    }
    if cfg!(debug_assertions) {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("Convertor")
            .join("MinerU");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn resolve_mineru_python() -> Option<PathBuf> {
    let dir = resolve_mineru_dir()?;
    venv_python(&dir.join(".venv"))
}

/// MinerU 是否可用（venv + 入口脚本都存在）。
pub fn mineru_available() -> bool {
    match (resolve_mineru_dir(), resolve_mineru_python()) {
        (Some(dir), Some(_py)) => dir.join("guaxi_mineru_convert.py").exists(),
        _ => false,
    }
}

fn surya_model_dir() -> Result<String, String> {
    let model_dir = bundled_resources::ensure_relative_dir("utils/Models/surya_models")?;
    let _ = std::fs::create_dir_all(&model_dir);
    Ok(model_dir.to_string_lossy().to_string())
}

fn mineru_model_dir() -> Option<String> {
    bundled_resources::ensure_relative_dir("utils/Models/mineru_models")
        .ok()
        .map(|p| {
            let _ = std::fs::create_dir_all(&p);
            p.to_string_lossy().to_string()
        })
}

fn combined_pythonpath(extra: &Path) -> String {
    let existing = std::env::var("PYTHONPATH").unwrap_or_default();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let extra = extra.to_string_lossy().to_string();
    if existing.trim().is_empty() {
        extra
    } else {
        format!("{}{}{}", extra, sep, existing)
    }
}

// ───────────────────────── 探针 ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub recommended: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub scores: serde_json::Value,
}

/// 运行探针脚本，返回建议引擎与原始 JSON。
/// 使用托管快速运行时（自动装配，含 pymupdf）。装配失败时保守返回：
/// marker 可用则 marker，否则 fast（保证仍可转换）。
pub async fn probe(input_path: &str) -> ProbeResult {
    let fallback = |reason: String| ProbeResult {
        recommended: if marker_engine_ready() { "marker" } else { "fast" }.to_string(),
        reason,
        scores: serde_json::Value::Null,
    };

    // 确保快速运行时就绪（探针依赖其中的 pymupdf）。放到阻塞线程。
    let rt = match tokio::task::spawn_blocking(super::provision::ensure_fast_engine_blocking).await {
        Ok(Ok(rt)) => rt,
        Ok(Err(e)) => return fallback(format!("快速运行时未就绪，探针跳过: {}", e)),
        Err(e) => return fallback(format!("装配任务失败，探针跳过: {}", e)),
    };
    let script = rt.script_dir.join(super::provision::PROBE_FILE);
    if !script.exists() {
        return fallback("探针脚本缺失".to_string());
    }

    let mut cmd = TokioCommand::new(&rt.program);
    cmd.args(&rt.base_args)
        .arg("-u")
        .arg(&script)
        .arg(input_path)
        .current_dir(&rt.script_dir)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = match cmd.output().await {
        Ok(o) => o,
        Err(e) => return fallback(format!("探针进程启动失败: {}", e)),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if v.get("type").and_then(|t| t.as_str()) == Some("probe_result") {
                let recommended = v
                    .get("recommended")
                    .and_then(|r| r.as_str())
                    .unwrap_or("marker")
                    .to_string();
                let reason = v
                    .get("reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                let scores = v.get("scores").cloned().unwrap_or(serde_json::Value::Null);
                info!(input = input_path, recommended = recommended.as_str(), reason = reason.as_str(), "probe result");
                return ProbeResult { recommended, reason, scores };
            }
        }
    }
    fallback("探针无有效输出".to_string())
}

/// 将“请求引擎 + 探针建议”解析为实际执行引擎，处理 MinerU 缺失时的自动回退。
pub async fn resolve_engine(requested: &str, input_path: &str) -> (Engine, Option<ProbeResult>) {
    let mut probe_result: Option<ProbeResult> = None;
    let req_lower = requested.to_ascii_lowercase();
    let explicit_mineru = req_lower == "mineru";

    let mut target = match req_lower.as_str() {
        "fast" => Engine::Fast,
        "marker" => Engine::Marker,
        "mineru" => Engine::Mineru,
        // 极速引擎（anydoc）：既可由用户显式选择，也会被探针路由到——
        // 旧版 Office / RTF / OpenDocument（.doc/.ppt/.rtf/.odt…）只有它能读。
        "anydoc" => Engine::Anydoc,
        _ => {
            // auto：探针决定
            let pr = probe(input_path).await;
            let eng = Engine::from_str(&pr.recommended).unwrap_or(Engine::Marker);
            probe_result = Some(pr);
            eng
        }
    };

    // MinerU：仅在“用户显式选择高质量”时才允许按需重装配（多 GB，避免自动路由触发意外大下载）。
    // 自动路由若建议 mineru 但尚未装配，则回退到 marker（更轻、可按需 pip 装配）。
    if target == Engine::Mineru {
        let mineru_ready = mineru_available() || super::provision::mineru_runtime().is_some();
        if !mineru_ready && !explicit_mineru {
            warn!(input = input_path, "自动路由建议 MinerU 但未装配，回退到 marker 引擎");
            target = Engine::Marker;
        }
    }

    // Marker 路由说明：
    // - 独立 uv venv 已装配则直接用；否则 build_marker_plan 会按需装配（首次较慢，
    //   任务面板显示装配中），失败再回退旧的 conda 分发版；
    // - 这里不强制降级到 fast，以保证复杂文档仍获得 marker 质量。

    (target, probe_result)
}

// ───────────────────────── 子进程构建 ─────────────────────────

/// 一次转换所需的执行计划。
pub struct ConversionPlan {
    pub command: TokioCommand,
    /// 期望产物目录（脚本以此为 --output_dir）。
    #[allow(dead_code)]
    pub expected_output_dir: PathBuf,
    /// 期望的 Markdown 文件路径（校验用）。
    pub expected_md_path: PathBuf,
    #[allow(dead_code)]
    pub engine: Engine,
    #[allow(dead_code)]
    pub cwd: PathBuf,
}

/// 由输入文件与输出基目录推导产物路径：`{output_dir}/{stem}/{stem}.md`。
/// 返回 `(expected_output_dir, expected_md_path)`。
pub fn expected_md_path_for(input_path: &str, output_dir: &str) -> (PathBuf, PathBuf) {
    let stem = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();
    let expected_output_dir = PathBuf::from(output_dir).join(&stem);
    let expected_md_path = expected_output_dir.join(format!("{}.md", stem));
    (expected_output_dir, expected_md_path)
}

/// 为指定引擎构建可直接 spawn 的命令。output_dir 为 markdown 基目录，
/// 产物落在 output_dir/{stem}/{stem}.md（与旧 marker 契约一致）。
///
/// 注意：anydoc 是进程内引擎，不在此处构建命令，由 queue.rs 单独分支处理。
pub fn build_plan(engine: Engine, input_path: &str, output_dir: &str) -> Result<ConversionPlan, String> {
    let (expected_output_dir, expected_md_path) = expected_md_path_for(input_path, output_dir);
    let _ = std::fs::create_dir_all(&expected_output_dir);

    match engine {
        Engine::Fast => build_fast_plan(input_path, expected_output_dir, expected_md_path),
        Engine::Marker => build_marker_plan(input_path, expected_output_dir, expected_md_path),
        Engine::Mineru => build_mineru_plan(input_path, expected_output_dir, expected_md_path),
        // 走到这里说明 queue.rs 的路由被改坏了——anydoc 不该有子进程命令。
        Engine::Anydoc => Err("anydoc 为进程内引擎，不应构建子进程命令".to_string()),
    }
}

fn apply_common(cmd: &mut TokioCommand, cwd: &Path, pythonpath: String, model_dir: &str) {
    cmd.current_dir(cwd)
        .env("PYTHONPATH", pythonpath)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("HF_HOME", model_dir)
        .env("HUGGINGFACE_HUB_CACHE", model_dir)
        .env("TRANSFORMERS_CACHE", model_dir)
        .env("TORCH_HOME", model_dir)
        .env("XDG_CACHE_HOME", model_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
}

fn build_fast_plan(
    input_path: &str,
    expected_output_dir: PathBuf,
    expected_md_path: PathBuf,
) -> Result<ConversionPlan, String> {
    // 快速引擎运行在“托管 Python”上（启动时自动装配 pymupdf4llm/markitdown/pymupdf）。
    let rt = super::provision::ensure_fast_engine_blocking()?;
    let script = rt.script_dir.join(super::provision::FAST_CONVERT_FILE);
    if !script.exists() {
        return Err(format!("找不到快速转换脚本: {}", script.display()));
    }

    let mut cmd = TokioCommand::new(&rt.program);
    cmd.args(&rt.base_args)
        .arg("-u")
        .arg(&script)
        .arg(input_path)
        .arg("--output_dir")
        .arg(&expected_output_dir)
        .arg("--progress_json")
        .current_dir(&rt.script_dir)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    Ok(ConversionPlan {
        command: cmd,
        expected_output_dir,
        expected_md_path,
        engine: Engine::Fast,
        cwd: rt.script_dir,
    })
}

/// 按设置页的 OCR 策略决定 marker 调不调用 VLM。
///
/// 为什么要有策略而不是一个开关：CPU 上 VLM 解码只有约 6 token/s（170ms/token），
/// 一次整页 OCR 要数分钟，上千页根本跑不完；但"干脆不用 OCR"又会让扫描件彻底没内容。
/// 所以做成三档，默认 `auto`：
/// - `auto`：**有 GPU 时按 `full`**（OCR 很快，不该为速度牺牲质量）；**没有 GPU 时**把判断
///   下推给脚本——数字文档跳过 OCR（秒级），扫描件仍照常 OCR（慢但必要）。
///   这样"只有 CPU 的机器"依然能处理扫描件，只是慢。
/// - `full`：完全交给 marker（等价于它自己的行为，质量最高）。
/// - `text_only`：永不调用 VLM，只抽文本层（最快；扫描件会几乎没有内容）。
///
/// `GUAXI_MARKER_DISABLE_OCR=1` 仍作为最高优先级的应急开关（排查用）。
fn apply_marker_ocr_policy(cmd: &mut TokioCommand) {
    let emergency_off = std::env::var("GUAXI_MARKER_DISABLE_OCR")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
    if emergency_off {
        cmd.args(["--ocr_mode", "text_only"]);
        return;
    }

    let effective = match crate::commands::settingview::marker_ocr_mode().as_str() {
        "text_only" => "text_only",
        "full" => "full",
        // auto：只有在没有 GPU 时才让脚本按文档类型判断。
        _ => {
            if super::provision::marker_uses_cuda() {
                "full"
            } else {
                "auto"
            }
        }
    };
    cmd.args(["--ocr_mode", effective]);
}

/// 按设置页的阅读顺序策略决定 marker 用什么顺序排块。
///
/// marker 2.x 的 `use_pdftext_reading_order` 默认为真：只要页面有文字层，就按 PDF 字符流
/// 的序号重排所有块，跳过版面模型的阅读顺序头——多栏页面上这样更准，所以它是上游的选择。
/// 但字符流顺序并不等于视觉顺序：浏览器「打印成 PDF」的网页会先输出正文、最后才写各节标题，
/// 按字符流排就会把标题整段挪到文末，标题与正文分家。因此 Guaxi 默认取 `layout`
/// （即传 false，等价于 marker 1.x 的版面/模型排序），需要时可在设置页切回 `pdftext`。
///
/// 只在独立 venv（marker 2.x）这条路径上传：旧的 conda 分发版是 1.10.1，没有这个选项，
/// 传过去会被 CLI 判为未知参数。
fn apply_marker_reading_order(cmd: &mut TokioCommand) {
    let value = if crate::commands::settingview::marker_reading_order() == "pdftext" {
        "true"
    } else {
        "false"
    };
    cmd.args(["--use_pdftext_reading_order", value]);
}

/// Marker 引擎是否就绪：独立的 uv venv 或旧的 conda 分发版任一可用。
pub fn marker_engine_ready() -> bool {
    super::provision::marker_runtime().is_some() || marker_available()
}

fn build_marker_plan(
    input_path: &str,
    expected_output_dir: PathBuf,
    expected_md_path: PathBuf,
) -> Result<ConversionPlan, String> {
    let model_dir = surya_model_dir()?;

    // 主路径：独立 uv venv（源码取自 Guaxi fork 的定制分支，全部补丁生效）。
    // 与 Python cell 共用的托管 venv 隔离，避免精确版本约束互相污染。
    match super::provision::ensure_marker_engine_blocking() {
        Ok(rt) => {
            let script = rt.script_dir.join(super::provision::MARKER_CONVERT_FILE);
            if !script.exists() {
                return Err(format!("找不到 marker 转换脚本: {}", script.display()));
            }
            let pythonpath = marker_pythonpath(&rt, &script);
            let mut cmd = TokioCommand::new(&rt.program);
            cmd.args(&rt.base_args)
                .arg("-u")
                .arg(&script)
                .arg(input_path)
                .arg("--output_format")
                .arg("markdown")
                .arg("--paginate_output");
            apply_marker_mode(&mut cmd);
            apply_marker_ocr_policy(&mut cmd);
            apply_marker_reading_order(&mut cmd);
            cmd.arg("--output_dir")
                .arg(&expected_output_dir)
                .arg("--progress_json");
            apply_common(&mut cmd, &rt.script_dir, pythonpath, &model_dir);
            apply_marker_env(&mut cmd, &model_dir);
            return Ok(ConversionPlan {
                command: cmd,
                expected_output_dir,
                expected_md_path,
                engine: Engine::Marker,
                cwd: rt.script_dir,
            });
        }
        Err(error) => {
            warn!(
                error = error.as_str(),
                "[convert] 独立 Marker 运行环境不可用，尝试回退旧 conda 分发版"
            );
        }
    }

    // 兜底：旧的 conda 分发版（随安装目录分发过、且尚未清理的机器）。
    // 这条路径保留是为了“升级期间不出空白”，确认新路径稳定后可移除。
    if let Ok(marker_src) = resolve_marker_source_dir() {
        if let Ok(python) = resolve_marker_python(&marker_src) {
            let script = marker_src.join("convert_single.py");
            if script.exists() {
                let mut cmd = TokioCommand::new(&python);
                cmd.arg("-u")
                    .arg(&script)
                    .arg(input_path)
                    .arg("--output_format")
                    .arg("markdown")
                    .arg("--paginate_output")
                    .arg("--output_dir")
                    .arg(&expected_output_dir)
                    .arg("--progress_json");
                apply_common(&mut cmd, &marker_src, combined_pythonpath(&marker_src), &model_dir);
                return Ok(ConversionPlan {
                    command: cmd,
                    expected_output_dir,
                    expected_md_path,
                    engine: Engine::Marker,
                    cwd: marker_src,
                });
            }
        }
    }

    Err(
        "Marker 运行环境不可用：自动装配失败，且未找到旧的 conda 分发版。\
         请在设置页查看失败原因并重试（常见原因是网络无法下载依赖或模型）。"
            .to_string(),
    )
}

/// marker 子进程的 PYTHONPATH：源码目录优先，保证定制补丁生效（editable 安装之外的兜底）。
fn marker_pythonpath(rt: &super::provision::EngineRuntime, script: &Path) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut parts: Vec<String> = Vec::new();
    if let Some(src) = &rt.marker_src {
        parts.push(src.to_string_lossy().to_string());
    }
    if let Some(src) = &rt.surya_src {
        parts.push(src.to_string_lossy().to_string());
    }
    parts.push(
        script
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let existing = std::env::var("PYTHONPATH").unwrap_or_default();
    if !existing.trim().is_empty() {
        parts.push(existing);
    }
    parts.join(sep)
}

/// 转换模式：默认交给 marker 按设备自动选（GPU→balanced，CPU→fast），
/// `GUAXI_MARKER_MODE=fast|balanced` 可强制覆盖。
fn apply_marker_mode(cmd: &mut TokioCommand) {
    if let Ok(mode) = std::env::var("GUAXI_MARKER_MODE") {
        let mode = mode.trim().to_ascii_lowercase();
        if mode == "fast" || mode == "balanced" {
            cmd.arg("--mode").arg(mode);
        }
    }
}

/// Marker（surya VLM）运行所需的专用环境变量。
///
/// marker 2.x 起布局/OCR/表格全部由 surya VLM 承担，模型跑在**独立推理服务**里：
/// - Windows 上自动探测到 NVIDIA 卡会去用 vLLM（要 Docker），桌面端不假设有 Docker，
///   因此显式走 llama.cpp；
/// - llama.cpp 复用应用已随包分发的 `utils/llama/llama-server.exe`（含多模态 mmproj 支持），
///   不必再为文档引擎单独准备一份推理二进制；
/// - 保留常驻推理服务，避免每次转换都重新加载数 GB 权重；
/// - 模型缓存统一指向 `utils/Models/surya_models`（HF 快照 + s3 模型都落这里），
///   这样才能随安装目录分发、支持离线加载。
///
/// 全部可用同名环境变量覆盖，便于排查。
fn apply_marker_env(cmd: &mut TokioCommand, model_dir: &str) {
    // llama.cpp 的 `llama-server`：应用随包分发在 utils/llama 下，Windows / Linux 通用，
    // 优先复用它（用户显式设置了 LLAMA_CPP_BINARY 时遵从用户）。
    let llama_server_ready = if std::env::var("LLAMA_CPP_BINARY").is_err() {
        match crate::commands::settingview::resolve_llama_server_path() {
            Some(path) => {
                cmd.env("LLAMA_CPP_BINARY", path.to_string_lossy().to_string());
                true
            }
            None => false,
        }
    } else {
        true
    };

    // 后端选择：
    // - Windows：必须显式指定 llamacpp。surya 会优先自动探测到 NVIDIA 卡并去用 vLLM，
    //   而 vLLM 后端是跑在 Docker 容器里的，桌面端不假设有 Docker。
    // - 其他平台：只要手上确实有 llama-server 就同样强制 llamacpp（避开 Docker 依赖）；
    //   否则交给 surya 自己的设备探测——Linux + NVIDIA 且本机已有 vLLM 时，
    //   用户能拿到比 llama.cpp 更快的后端。
    if std::env::var("SURYA_INFERENCE_BACKEND").is_err() && (cfg!(windows) || llama_server_ready) {
        cmd.env("SURYA_INFERENCE_BACKEND", "llamacpp");
    }
    if !llama_server_ready {
        warn!(
            "[convert] 未找到 utils/llama 下的 llama-server；若 surya 也自行找不到，\
             marker 的 VLM 推理服务将无法启动"
        );
    }
    // 有 GPU 时把全部层放到显存（纯 CPU 构建下是安全的 no-op）。
    if std::env::var("LLAMA_CPP_NGL").is_err() {
        cmd.env("LLAMA_CPP_NGL", "99");
    }
    // 首次启动服务要加载数 GB 权重，默认超时偏紧，这里放宽。
    if std::env::var("SURYA_INFERENCE_STARTUP_TIMEOUT").is_err() {
        cmd.env("SURYA_INFERENCE_STARTUP_TIMEOUT", "1800");
    }
    // HF 官方域名在大陆直连不可达。模型由上面的预下载负责，但转换时若仍有缺失
    // （比如用户手动删了模型目录），surya 会自己按需下载——这里必须给同一个镜像，
    // 否则转换会在“加载模型”一步长时间卡住后失败。用户已设置 HF_ENDPOINT 时尊重用户。
    if std::env::var("HF_ENDPOINT").is_err() {
        cmd.env("HF_ENDPOINT", "https://hf-mirror.com");
    }
    // 模型已在本地时进入 HuggingFace 离线模式。
    //
    // 为什么要这样：联网模式下 `snapshot_download` 每次转换都会先请求 HF API 核对 revision，
    // 即便缓存命中也得走一次网络。实测就撞到过这一次请求被重置、整次转换直接失败的
    // `httpx.ConnectError: [SSL: UNEXPECTED_EOF_WHILE_READING]`。对上千页的大任务来说，
    // 一次瞬时抖动毁掉整轮转换是不可接受的。离线模式既去掉这个单点故障，也省掉加载前的网络往返。
    if std::env::var("HF_HUB_OFFLINE").is_err() && super::provision::marker_models_ready() {
        cmd.env("HF_HUB_OFFLINE", "1");
        cmd.env("TRANSFORMERS_OFFLINE", "1");
    }
    // 刻意**不**设置 SURYA_INFERENCE_KEEP_ALIVE：开启后推理服务会在转换进程退出后继续常驻，
    // 成为应用关不掉的孤儿进程（占住数 GB 内存/显存）。默认（关闭）下服务随转换进程一起结束，
    // 与应用既有的“不留残余进程”约定一致。用户若显式设置该变量则遵从用户。
    // s3 模型（text_detection / ocr_error_detection）的缓存目录。
    cmd.env("MODEL_CACHE_DIR", model_dir);
    // 代理：本地推理服务走 127.0.0.1，必须排除；境外下载则走系统代理。
    super::net::apply_proxy_env(cmd);

    // 每次转换打一行运行参数，便于排查“到底有没有用上 GPU / 走了哪个后端”。
    info!(
        cuda_torch = super::provision::marker_uses_cuda(),
        backend = std::env::var("SURYA_INFERENCE_BACKEND")
            .unwrap_or_else(|_| "auto".to_string())
            .as_str(),
        llama_server = llama_server_ready,
        models_local = super::provision::marker_models_ready(),
        via_proxy = super::net::effective_proxy().is_some(),
        mode = std::env::var("GUAXI_MARKER_MODE").unwrap_or_else(|_| "auto".to_string()).as_str(),
        "[convert] marker 运行参数（GPU 加速需 cuda_torch=true 且 llama.cpp 为 CUDA 构建）"
    );
}

fn build_mineru_plan(
    input_path: &str,
    expected_output_dir: PathBuf,
    expected_md_path: PathBuf,
) -> Result<ConversionPlan, String> {
    let model_dir = mineru_model_dir().unwrap_or_else(|| surya_model_dir().unwrap_or_default());

    // 优先使用独立 venv 分发版（若已随包提供）。
    if let (Some(dir), Some(python)) = (resolve_mineru_dir(), resolve_mineru_python()) {
        let script = dir.join("guaxi_mineru_convert.py");
        if script.exists() {
            let mut cmd = TokioCommand::new(&python);
            cmd.arg("-u")
                .arg(&script)
                .arg(input_path)
                .arg("--output_dir")
                .arg(&expected_output_dir)
                .arg("--progress_json");
            apply_common(&mut cmd, &dir, combined_pythonpath(&dir), &model_dir);
            apply_mineru_device_env(&mut cmd);
            return Ok(ConversionPlan {
                command: cmd,
                expected_output_dir,
                expected_md_path,
                engine: Engine::Mineru,
                cwd: dir,
            });
        }
    }

    // 回退：托管 pip 版 MinerU（自动装配 mineru[core] + GPU 机器装 CUDA torch；模型首次运行按需下载）。
    let rt = super::provision::ensure_mineru_engine_blocking()?;
    let script = rt.script_dir.join(super::provision::MINERU_CONVERT_FILE);
    if !script.exists() {
        return Err(format!("找不到 MinerU 转换脚本: {}", script.display()));
    }
    let mut cmd = TokioCommand::new(&rt.program);
    cmd.args(&rt.base_args)
        .arg("-u")
        .arg(&script)
        .arg(input_path)
        .arg("--output_dir")
        .arg(&expected_output_dir)
        .arg("--progress_json");
    apply_common(&mut cmd, &rt.script_dir, combined_pythonpath(&rt.script_dir), &model_dir);
    apply_mineru_device_env(&mut cmd);

    Ok(ConversionPlan {
        command: cmd,
        expected_output_dir,
        expected_md_path,
        engine: Engine::Mineru,
        cwd: rt.script_dir,
    })
}

/// 设置 MinerU 设备相关环境变量。
/// - 已确认 CUDA torch 可用 -> 显式 MINERU_DEVICE_MODE=cuda；
/// - 否则不强制（MinerU 会按 torch.cuda.is_available() 自动选择，避免误设 cuda 而崩溃）。
/// 均可被用户通过同名环境变量覆盖。
fn apply_mineru_device_env(cmd: &mut TokioCommand) {
    if std::env::var("MINERU_DEVICE_MODE").is_err() && super::provision::mineru_uses_cuda() {
        cmd.env("MINERU_DEVICE_MODE", "cuda");
    }
    // 模型源：默认走 ModelScope（国内下载快、稳定，规避 huggingface.co 超时/断连）。
    // 可用 GUAXI_MINERU_MODEL_SOURCE 覆盖为 huggingface / local。
    let model_source = std::env::var("GUAXI_MINERU_MODEL_SOURCE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "modelscope".to_string());
    cmd.env("MINERU_MODEL_SOURCE", model_source);
}

// ───────────────────────── 极速引擎（anydoc，进程内） ─────────────────────────

/// 极速引擎的单文件体积上限。anydoc 会把整份文件读进内存，超过这个量级既有 OOM 风险，
/// 也不是「纯文本型文档高速转换」的目标场景。
const ANYDOC_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

/// 串行化 panic hook 的替换。hook 是进程级全局状态，并发调用会互相覆盖，
/// 必须和 `tts_book.rs` 里 pdf-extract 那处一样加锁。
static ANYDOC_PANIC_HOOK_GUARD: Mutex<()> = Mutex::new(());

/// 用 anydoc 在**本进程内**把文档转成 Markdown 并写入 `output_md`。
///
/// 阻塞调用，务必放在 `spawn_blocking` 里执行。
///
/// 关键点：anydoc 是第三方解析库，畸形输入理论上可能触发 panic。这里沿用
/// `tts_book.rs::extract_pdf_text` 的既有写法，用 `catch_unwind` 把 panic 限制在本次调用内，
/// 绝不向调用方展开——**前提是 release 必须 `panic = "unwind"`**（见 Cargo.toml 的 profile.release，
/// 那里写明了不能改回 abort 的原因）。
pub fn convert_with_anydoc(input_path: &Path, output_md: &Path) -> Result<(), String> {
    let size = std::fs::metadata(input_path)
        .map_err(|e| format!("读取文件信息失败: {}", e))?
        .len();
    if size > ANYDOC_MAX_INPUT_BYTES {
        return Err(format!(
            "文件过大（{:.0} MB），极速引擎不予处理，请改用其他引擎",
            size as f64 / 1024.0 / 1024.0
        ));
    }

    let markdown = {
        let _guard = ANYDOC_PANIC_HOOK_GUARD
            .lock()
            .map_err(|_| "极速引擎 panic hook 锁获取失败".to_string())?;
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = catch_unwind(AssertUnwindSafe(|| anydoc::to_markdown(input_path)));
        std::panic::set_hook(previous_hook);

        match result {
            Ok(Ok(md)) => md,
            Ok(Err(e)) => {
                let msg = anydoc_error_message(&e);
                warn!(input = %input_path.display(), error = %e, "极速引擎转换失败");
                return Err(msg);
            }
            Err(_) => {
                warn!(input = %input_path.display(), "极速引擎解析器 panic（已隔离，不影响主程序）");
                return Err("极速引擎内部错误（已隔离，不影响程序运行）".to_string());
            }
        }
    };

    if let Some(parent) = output_md.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {}", e))?;
    }
    std::fs::write(output_md, markdown).map_err(|e| format!("写入 Markdown 失败: {}", e))?;
    Ok(())
}

/// 把 anydoc 的类型化错误翻译成给用户看的中文文案。
/// `ConvertError` 标了 `#[non_exhaustive]`，兜底分支必须保留。
/// 有声书转换（`tts_book.rs`）解析 .doc 时复用同一份文案，故为 `pub(crate)`。
pub(crate) fn anydoc_error_message(e: &anydoc::ConvertError) -> String {
    use anydoc::ConvertError as E;
    match e {
        E::Encrypted => "文档已加密或受密码保护，无法转换".to_string(),
        E::NeedsOcr { pages, page_count } => format!(
            "该 PDF 为扫描件（{} / {} 页需要 OCR），极速引擎不做 OCR，请改用 Marker 或高质量强引擎",
            pages.len(),
            page_count
        ),
        E::Unsupported(what) => format!("不支持的文件格式：{}", what),
        E::Malformed { part, detail } => match part {
            Some(p) => format!("文档结构损坏（{}）：{}", p, detail),
            None => format!("文档结构损坏：{}", detail),
        },
        E::ResourceLimit { limit, detail } => format!("超出安全限制（{}）：{}", limit, detail),
        E::MissingPart { part } => format!("文档缺少必需部件：{}", part),
        E::Io(err) => format!("读取文件失败：{}", err),
        _ => format!("转换失败：{}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_string_roundtrip_includes_anydoc() {
        assert_eq!(Engine::Anydoc.as_str(), "anydoc");
        assert_eq!(Engine::from_str("anydoc"), Some(Engine::Anydoc));
        // 与其他引擎一致：大小写不敏感。
        assert_eq!(Engine::from_str("AnyDoc"), Some(Engine::Anydoc));
    }

    /// 产物路径必须与 commands.rs / 前端各自复算的 `{output_dir}/{stem}/{stem}.md` 一致。
    #[test]
    fn expected_md_path_matches_output_contract() {
        let (dir, md) = expected_md_path_for("/tmp/in/report.docx", "/tmp/out");
        assert_eq!(dir, PathBuf::from("/tmp/out").join("report"));
        assert_eq!(md, dir.join("report.md"));
    }

    /// 极速引擎必须真的能转换：用 CSV（无签名格式，走扩展名识别）跑通进程内链路。
    #[test]
    fn convert_with_anydoc_produces_markdown() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let src = tmp.path().join("sample.csv");
        std::fs::write(&src, "name,age\nAlice,30\nBob,25\n").expect("write csv");

        let out = tmp.path().join("sample").join("sample.md");
        convert_with_anydoc(&src, &out).expect("CSV 应能被极速引擎转换");

        let md = std::fs::read_to_string(&out).expect("read markdown");
        assert!(
            md.contains("Alice") && md.contains("Bob"),
            "markdown 应包含表格内容，实际: {md}"
        );
    }

    /// 新路由的关键前提：旧版 Office 类格式必须真的能被 anydoc 读出来。
    ///
    /// 探针现在会把 `.doc/.ppt/.rtf/.odt…` 路由到极速引擎（因为快速引擎的 markitdown
    /// 没有这些转换器）。这条测试用 RTF（纯文本、可手写）把该前提钉住，
    /// 避免以后 anydoc 被换掉/降级时路由悄悄失效。
    #[test]
    fn convert_with_anydoc_handles_legacy_office_formats() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let src = tmp.path().join("sample.rtf");
        std::fs::write(
            &src,
            r"{\rtf1\ansi\deff0 {\fonttbl {\f0 Times;}}\f0\fs24 Hello RTF world.\par Second paragraph.\par}",
        )
        .expect("write rtf");

        let out = tmp.path().join("sample").join("sample.md");
        convert_with_anydoc(&src, &out).expect("RTF 应能被极速引擎转换");

        let md = std::fs::read_to_string(&out).expect("read markdown");
        assert!(
            md.contains("Hello RTF world"),
            "markdown 应包含正文，实际: {md}"
        );
    }

    #[test]
    fn convert_with_anydoc_rejects_unreadable_input() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let out = tmp.path().join("x.md");
        assert!(convert_with_anydoc(&tmp.path().join("nope.docx"), &out).is_err());
        assert!(!out.exists(), "失败时不应产生输出文件");
    }

    /// 无法识别的输入要给出可操作的中文文案，而不是裸露的库错误。
    /// `ConvertError` 标了 `#[non_exhaustive]`，测试里无法直接构造它的变体，只能走真实文件。
    /// （纯文本 .txt 不在 anydoc 支持范围内，会走 Unsupported 分支。）
    #[test]
    fn convert_with_anydoc_reports_unsupported_format() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let src = tmp.path().join("note.txt");
        std::fs::write(&src, "just plain text, no recognisable container").expect("write");
        let out = tmp.path().join("note").join("note.md");

        let err = convert_with_anydoc(&src, &out).expect_err("纯文本无法识别，应失败");
        assert!(err.contains("不支持"), "实际: {err}");
        assert!(!out.exists());
    }

    /// 崩溃隔离的前提：release 必须保持 unwind。
    /// abort 会让 catch_unwind 失效，极速引擎（及 pdf-extract）的第三方解析 panic 将直接
    /// 终止整个应用。这条测试把该不变量钉住，防止被无声改回。
    #[test]
    fn release_profile_must_keep_unwind() {
        let cargo_toml = include_str!("../../../Cargo.toml");
        assert!(
            cargo_toml.contains("panic = \"unwind\""),
            "release profile 必须保持 panic = \"unwind\""
        );
        assert!(
            !cargo_toml.contains("panic = \"abort\""),
            "不能改回 panic = \"abort\"：catch_unwind 会失效，畸形文档可直接崩掉应用"
        );
    }
}
