//! 引擎解析、探针与子进程构建。
//!
//! 三个引擎共用同一套“路径解析 + 环境变量 + JSON 进度协议”约定：
//! - fast  : marker venv 中的 `guaxi_fast_convert.py`（pymupdf4llm / markitdown）
//! - marker: marker venv 中的 `convert_single.py`（保持与旧实现完全一致）
//! - mineru: 独立 venv `utils/MinerU/.venv` 中的 `guaxi_mineru_convert.py`
//!
//! marker 相关解析逻辑与 `docprocess.rs` 中的私有实现保持一致（此处独立一份，避免改动旧文件签名）。

use std::path::{Path, PathBuf};
use std::process::Stdio;

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
}

impl Engine {
    pub fn as_str(&self) -> &'static str {
        match self {
            Engine::Fast => "fast",
            Engine::Marker => "marker",
            Engine::Mineru => "mineru",
        }
    }

    pub fn from_str(s: &str) -> Option<Engine> {
        match s.to_ascii_lowercase().as_str() {
            "fast" => Some(Engine::Fast),
            "marker" => Some(Engine::Marker),
            "mineru" => Some(Engine::Mineru),
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
        recommended: if marker_available() { "marker" } else { "fast" }.to_string(),
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
    // - 若 conda 分发版可用则直接用；
    // - 否则 build_marker_plan 会按需装配“托管 pip 版 marker”（首次较慢，任务面板显示装配中）。
    // 这里不强制降级到 fast，以保证复杂文档仍获得 marker 质量；仅当托管 marker 已装配或
    // conda 版可用时才认为“就绪”，其余情况交由 build 阶段按需装配或报错重试。

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

/// 为指定引擎构建可直接 spawn 的命令。output_dir 为 markdown 基目录，
/// 产物落在 output_dir/{stem}/{stem}.md（与旧 marker 契约一致）。
pub fn build_plan(engine: Engine, input_path: &str, output_dir: &str) -> Result<ConversionPlan, String> {
    let stem = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string();
    let expected_output_dir = PathBuf::from(output_dir).join(&stem);
    let expected_md_path = expected_output_dir.join(format!("{}.md", stem));
    let _ = std::fs::create_dir_all(&expected_output_dir);

    match engine {
        Engine::Fast => build_fast_plan(input_path, expected_output_dir, expected_md_path),
        Engine::Marker => build_marker_plan(input_path, expected_output_dir, expected_md_path),
        Engine::Mineru => build_mineru_plan(input_path, expected_output_dir, expected_md_path),
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

fn build_marker_plan(
    input_path: &str,
    expected_output_dir: PathBuf,
    expected_md_path: PathBuf,
) -> Result<ConversionPlan, String> {
    let model_dir = surya_model_dir()?;

    // 优先使用 conda 分发版（随安装目录，含预置 surya 模型，无需联网）。
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

    // 回退：托管 pip 版 marker（自动装配 marker-pdf；surya 模型首次运行按需下载）。
    let rt = super::provision::ensure_marker_engine_blocking()?;
    let script = rt.script_dir.join(super::provision::MARKER_CONVERT_FILE);
    if !script.exists() {
        return Err(format!("找不到 marker 转换脚本: {}", script.display()));
    }
    let mut cmd = TokioCommand::new(&rt.program);
    cmd.args(&rt.base_args)
        .arg("-u")
        .arg(&script)
        .arg(input_path)
        .arg("--output_format")
        .arg("markdown")
        .arg("--paginate_output")
        .arg("--output_dir")
        .arg(&expected_output_dir)
        .arg("--progress_json");
    apply_common(&mut cmd, &rt.script_dir, combined_pythonpath(&rt.script_dir), &model_dir);

    Ok(ConversionPlan {
        command: cmd,
        expected_output_dir,
        expected_md_path,
        engine: Engine::Marker,
        cwd: rt.script_dir,
    })
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
