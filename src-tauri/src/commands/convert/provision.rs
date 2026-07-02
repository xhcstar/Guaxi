//! 转换引擎的运行时自动装配（无需用户手动构建）。
//!
//! 设计目标：软件启动后自动检查并装配转换所需依赖，用户开箱即用。
//!
//! 分层策略（按“开箱即用”的重要性与安装成本）：
//! - **快速引擎 + 探针（必备，轻量）**：复用应用已有的“托管 Python”机制
//!   （[`code_runtime::ensure_managed_python_with_packages`]，会自动下载 Python、
//!   建 venv、配好清华镜像并幂等 pip 安装）。装 `pymupdf4llm` / `markitdown` / `pymupdf` 即可，
//!   纯 Python 轮子、无需 conda/GPU。启动时后台自动完成。
//! - **Marker（强引擎，中等）**：沿用 conda venv（体积大、含 surya 模型）。若已随安装目录分发则直接用；
//!   缺失时不阻塞——路由层自动降级到快速引擎。
//! - **MinerU（强引擎，重）**：独立 venv + 数 GB 模型 + 可选 GPU。作为可选增强，缺失时自动回退 marker。
//!
//! 关键点：快速引擎与探针脚本随二进制内嵌（`include_str!`），运行时物化到托管 runtime 目录，
//! 因此不依赖 `Convertor/` 源码是否随包分发，真正做到“装好就能用”。

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tracing::{info, warn};

/// 内嵌的引擎脚本（与 Convertor/marker-1.10.1/ 下同名文件保持同步）。
const FAST_CONVERT_SOURCE: &str = include_str!("../../../scripts/guaxi_fast_convert.py");
const PROBE_SOURCE: &str = include_str!("../../../scripts/guaxi_probe.py");
const MARKER_CONVERT_SOURCE: &str = include_str!("../../../scripts/guaxi_marker_convert.py");
const MINERU_CONVERT_SOURCE: &str = include_str!("../../../scripts/guaxi_mineru_convert.py");

pub const FAST_CONVERT_FILE: &str = "guaxi_fast_convert.py";
pub const PROBE_FILE: &str = "guaxi_probe.py";
pub const MARKER_CONVERT_FILE: &str = "guaxi_marker_convert.py";
pub const MINERU_CONVERT_FILE: &str = "guaxi_mineru_convert.py";

/// 快速引擎所需的纯 Python 依赖（在托管 venv 中幂等安装）。
/// pymupdf4llm 会带入 PyMuPDF(fitz)，探针也依赖它做版面/公式分析。
const FAST_ENGINE_PACKAGES: &[&str] = &["pymupdf4llm", "markitdown", "pymupdf"];

/// Marker 强引擎的 pip 依赖。marker-pdf 会带入 surya-ocr 与 torch（CPU 轮子），
/// surya 模型在首次转换时按需自动下载到模型目录（受 HF_HOME 等环境变量控制）。
const MARKER_ENGINE_PACKAGES: &[&str] = &["marker-pdf"];

/// MinerU 强引擎的 pip 依赖（重，含 torch 与自有模型）。首次装配耗时长；
/// 仅在“需要 mineru 且未装配”时按需触发。模型首次运行按需下载。
const MINERU_ENGINE_PACKAGES: &[&str] = &["mineru[core]"];

/// 装配状态：供前端展示“转换引擎是否就绪”。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionStatus {
    /// 快速引擎（含探针）是否就绪。
    pub fast_ready: bool,
    /// 托管 Python 解释器路径（就绪时）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_python: Option<String>,
    /// 快速引擎装配是否正在进行。
    pub fast_provisioning: bool,
    /// 最近一次装配错误（若有）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_error: Option<String>,
    /// Marker 强引擎是否可用（conda 分发版 或 托管 pip 版任一就绪）。
    pub marker_ready: bool,
    /// Marker 强引擎装配是否正在进行。
    pub marker_provisioning: bool,
    /// 最近一次 marker 装配错误（若有）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_error: Option<String>,
    /// MinerU 强引擎是否可用。
    pub mineru_ready: bool,
}

/// 已解析的快速引擎运行时（托管 Python + 脚本目录）。
#[derive(Debug, Clone)]
pub struct FastRuntime {
    pub program: String,
    pub base_args: Vec<String>,
    pub script_dir: PathBuf,
}

static FAST_RUNTIME: OnceLock<Mutex<Option<FastRuntime>>> = OnceLock::new();
static MARKER_RUNTIME: OnceLock<Mutex<Option<FastRuntime>>> = OnceLock::new();
static MINERU_RUNTIME: OnceLock<Mutex<Option<FastRuntime>>> = OnceLock::new();
static PROVISION_STATE: OnceLock<Mutex<ProvisionStatus>> = OnceLock::new();

fn fast_runtime_cell() -> &'static Mutex<Option<FastRuntime>> {
    FAST_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn marker_runtime_cell() -> &'static Mutex<Option<FastRuntime>> {
    MARKER_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn mineru_runtime_cell() -> &'static Mutex<Option<FastRuntime>> {
    MINERU_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn status_cell() -> &'static Mutex<ProvisionStatus> {
    PROVISION_STATE.get_or_init(|| Mutex::new(ProvisionStatus::default()))
}

/// 读取当前装配状态快照（合并强引擎实时探测结果）。
pub fn current_status() -> ProvisionStatus {
    let mut s = status_cell().lock().map(|g| g.clone()).unwrap_or_default();
    // marker 就绪 = conda 分发版可用 或 托管 pip 版已装配。
    s.marker_ready = super::engine::marker_available() || marker_runtime().is_some();
    s.mineru_ready = super::engine::mineru_available() || mineru_runtime().is_some();
    s
}

/// 快速引擎是否已就绪（脚本已物化 + 依赖已装）。
pub fn fast_runtime() -> Option<FastRuntime> {
    fast_runtime_cell().lock().ok().and_then(|g| g.clone())
}

/// 托管 pip 版 marker 运行时（若已装配）。
pub fn marker_runtime() -> Option<FastRuntime> {
    marker_runtime_cell().lock().ok().and_then(|g| g.clone())
}

/// 托管 pip 版 MinerU 运行时（若已装配）。
pub fn mineru_runtime() -> Option<FastRuntime> {
    mineru_runtime_cell().lock().ok().and_then(|g| g.clone())
}

fn update_status(f: impl FnOnce(&mut ProvisionStatus)) {
    if let Ok(mut g) = status_cell().lock() {
        f(&mut g);
    }
}

/// 将内嵌脚本物化到托管 runtime 目录（仅在内容变化时写盘）。
fn materialize_scripts(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建脚本目录失败: {e}"))?;
    for (name, source) in [
        (FAST_CONVERT_FILE, FAST_CONVERT_SOURCE),
        (PROBE_FILE, PROBE_SOURCE),
        (MARKER_CONVERT_FILE, MARKER_CONVERT_SOURCE),
        (MINERU_CONVERT_FILE, MINERU_CONVERT_SOURCE),
    ] {
        let path = dir.join(name);
        let needs_write = match std::fs::read_to_string(&path) {
            Ok(existing) => existing != source,
            Err(_) => true,
        };
        if needs_write {
            std::fs::write(&path, source)
                .map_err(|e| format!("写入 {name} 失败: {e}"))?;
            info!(script = %path.to_string_lossy(), "[convert] 物化快速引擎脚本");
        }
    }
    Ok(())
}

/// 同步装配快速引擎：确保托管 Python + 依赖 + 脚本就绪。阻塞，放到 spawn_blocking 中调用。
/// 幂等：已装配时开销极小（pip list 比对 + 脚本内容比对）。
pub fn ensure_fast_engine_blocking() -> Result<FastRuntime, String> {
    // 已缓存则直接返回。
    if let Some(rt) = fast_runtime() {
        return Ok(rt);
    }

    update_status(|s| {
        s.fast_provisioning = true;
        s.fast_error = None;
    });

    let packages: Vec<String> = FAST_ENGINE_PACKAGES.iter().map(|s| s.to_string()).collect();
    let result = (|| -> Result<FastRuntime, String> {
        // 复用应用既有的托管 Python 装配（自动下载 Python + venv + 镜像 + 幂等安装）。
        let (program, base_args) =
            crate::commands::code_runtime::ensure_managed_python_with_packages(
                &packages,
                "doc-convert-fast-engine",
            )?;

        // 脚本物化到托管 runtime 目录（与 program 同一 runtime，随 venv 一起可用）。
        let script_dir = crate::commands::code_runtime::managed_runtime_dir()?;
        materialize_scripts(&script_dir)?;

        Ok(FastRuntime {
            program,
            base_args,
            script_dir,
        })
    })();

    match &result {
        Ok(rt) => {
            if let Ok(mut g) = fast_runtime_cell().lock() {
                *g = Some(rt.clone());
            }
            update_status(|s| {
                s.fast_ready = true;
                s.fast_provisioning = false;
                s.fast_python = Some(rt.program.clone());
                s.fast_error = None;
            });
            info!(python = %rt.program, "[convert] 快速引擎装配完成");
        }
        Err(e) => {
            update_status(|s| {
                s.fast_ready = false;
                s.fast_provisioning = false;
                s.fast_error = Some(e.clone());
            });
            warn!(error = %e, "[convert] 快速引擎装配失败");
        }
    }

    result
}

/// 按需装配托管 pip 版 Marker 强引擎（marker-pdf + surya）。阻塞，放到 spawn_blocking 中调用。
/// 幂等：已装配直接返回。首次会 pip 安装较大依赖（torch CPU 轮子等），耗时较长，
/// 因此不在启动时自动执行，而是在“需要 marker 且 conda 版缺失”时按需触发。
pub fn ensure_marker_engine_blocking() -> Result<FastRuntime, String> {
    if let Some(rt) = marker_runtime() {
        return Ok(rt);
    }

    update_status(|s| {
        s.marker_provisioning = true;
        s.marker_error = None;
    });

    let packages: Vec<String> = MARKER_ENGINE_PACKAGES.iter().map(|s| s.to_string()).collect();
    let result = (|| -> Result<FastRuntime, String> {
        let (program, base_args) =
            crate::commands::code_runtime::ensure_managed_python_with_packages(
                &packages,
                "doc-convert-marker-engine",
            )?;
        let script_dir = crate::commands::code_runtime::managed_runtime_dir()?;
        materialize_scripts(&script_dir)?;
        Ok(FastRuntime {
            program,
            base_args,
            script_dir,
        })
    })();

    match &result {
        Ok(rt) => {
            if let Ok(mut g) = marker_runtime_cell().lock() {
                *g = Some(rt.clone());
            }
            update_status(|s| {
                s.marker_provisioning = false;
                s.marker_error = None;
            });
            info!(python = %rt.program, "[convert] 托管 Marker 引擎装配完成");
        }
        Err(e) => {
            update_status(|s| {
                s.marker_provisioning = false;
                s.marker_error = Some(e.clone());
            });
            warn!(error = %e, "[convert] 托管 Marker 引擎装配失败");
        }
    }

    result
}

/// 按需装配托管 pip 版 MinerU 强引擎。阻塞，放到 spawn_blocking 中调用。幂等。
/// 依赖很重（torch + mineru + 模型），仅在“需要 mineru 且未装配”时按需触发。
pub fn ensure_mineru_engine_blocking() -> Result<FastRuntime, String> {
    if let Some(rt) = mineru_runtime() {
        return Ok(rt);
    }

    let packages: Vec<String> = MINERU_ENGINE_PACKAGES.iter().map(|s| s.to_string()).collect();
    let result = (|| -> Result<FastRuntime, String> {
        let (program, base_args) =
            crate::commands::code_runtime::ensure_managed_python_with_packages(
                &packages,
                "doc-convert-mineru-engine",
            )?;
        let script_dir = crate::commands::code_runtime::managed_runtime_dir()?;
        materialize_scripts(&script_dir)?;
        Ok(FastRuntime {
            program,
            base_args,
            script_dir,
        })
    })();

    if let Ok(rt) = &result {
        if let Ok(mut g) = mineru_runtime_cell().lock() {
            *g = Some(rt.clone());
        }
        info!(python = %rt.program, "[convert] 托管 MinerU 引擎装配完成");
    } else if let Err(e) = &result {
        warn!(error = %e, "[convert] 托管 MinerU 引擎装配失败");
    }

    result
}

/// 启动时后台装配：不阻塞主流程，失败仅记录（简单文档可退回 marker，若 marker 也缺则报错时再暴露）。
pub fn spawn_startup_provision() {
    tauri::async_runtime::spawn(async {
        let res = tokio::task::spawn_blocking(ensure_fast_engine_blocking).await;
        match res {
            Ok(Ok(_)) => info!("[convert] 启动装配：快速引擎就绪"),
            Ok(Err(e)) => warn!(error = %e, "[convert] 启动装配：快速引擎失败（将按需重试/降级）"),
            Err(e) => warn!(error = %e, "[convert] 启动装配任务 join 失败: {e}"),
        }
    });
}
