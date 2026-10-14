//! 转换引擎的运行时自动装配（无需用户手动构建）。
//!
//! 设计目标：软件启动后自动检查并装配转换所需依赖，用户开箱即用；装配失败时自己换源、
//! 重试、降级，而不是把原始错误直接抛给用户。
//!
//! 分层策略（按“开箱即用”的重要性与安装成本）：
//! - **极速引擎（anydoc，纯 Rust）**：编译进二进制，无装配、无 Python、无模型，恒就绪。
//! - **快速引擎 + 探针（必备，轻量）**：复用应用已有的“托管 Python”机制
//!   （[`code_runtime::ensure_managed_python_with_packages`]）。装 `pymupdf4llm` /
//!   `markitdown` / `pymupdf` 即可，纯 Python 轮子、无需 conda/GPU。启动时后台自动完成。
//! - **Marker（强引擎，中等）**：跑在**独立**的 uv 托管 venv（`utils/MarkdownConvertor/.venv`）里。
//!   与 Python cell 共用的托管 venv 隔离，避免 marker/surya 的精确版本约束（opencv / pillow /
//!   transformers）和用户日常装包互相污染。有 NVIDIA GPU 时自动换 CUDA 版 torch。
//! - **MinerU（强引擎，重）**：独立 venv + 数 GB 模型 + 可选 GPU。作为可选增强，缺失时自动回退 marker。
//!
//! 关键点：快速引擎与探针脚本随二进制内嵌（`include_str!`），运行时物化到托管 runtime 目录，
//! 因此不依赖 `Convertor/` 源码是否随包分发，真正做到“装好就能用”。
//!
//! 网络策略见 [`super::net`]：模型与依赖的下载一律“配了系统代理就走代理，没配就直连”。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::commands::code_runtime::{self, PythonLaunchSpec};
use crate::guaxiservices::bundled_resources;

use super::net;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// MinerU 是否已确认可用 CUDA（用于运行时设置 MINERU_DEVICE_MODE 与 UI 展示）。
static MINERU_CUDA: AtomicBool = AtomicBool::new(false);
/// Marker 是否已确认可用 CUDA（用于 UI 展示“GPU 加速”）。
static MARKER_CUDA: AtomicBool = AtomicBool::new(false);
/// Marker 是否处于“降级模式”：源码不可用，装的是 PyPI 版 `marker-pdf`。
static MARKER_DEGRADED: AtomicBool = AtomicBool::new(false);

pub fn mineru_uses_cuda() -> bool {
    MINERU_CUDA.load(Ordering::Relaxed)
}

pub fn marker_uses_cuda() -> bool {
    MARKER_CUDA.load(Ordering::Relaxed)
}

pub fn marker_uses_degraded_pypi() -> bool {
    MARKER_DEGRADED.load(Ordering::Relaxed)
}

fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let _ = cmd;
}

/// 机器是否存在可用的 NVIDIA GPU（通过 nvidia-smi 探测，成功即视为有 GPU）。
pub fn nvidia_gpu_present() -> bool {
    let mut cmd = Command::new("nvidia-smi");
    cmd.arg("-L").stdout(Stdio::piped()).stderr(Stdio::null());
    no_window(&mut cmd);
    match cmd.output() {
        Ok(o) => o.status.success() && !o.stdout.is_empty(),
        Err(_) => false,
    }
}

/// 运行 `python -c code`，返回 stdout（trim）。失败返回 Err。
fn python_eval(program: &str, base_args: &[String], code: &str) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(base_args)
        .arg("-c")
        .arg(code)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    no_window(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn torch_cuda_available(program: &str, base_args: &[String]) -> bool {
    python_eval(
        program,
        base_args,
        "import torch;print('1' if torch.cuda.is_available() else '0')",
    )
    .map(|s| s.trim() == "1")
    .unwrap_or(false)
}

/// 已装 torch 的基础版本（去掉 +cpu/+cuXXX 本地标签），如 "2.12.1"。
fn torch_base_version(program: &str, base_args: &[String]) -> Option<String> {
    python_eval(
        program,
        base_args,
        "import torch;print(torch.__version__.split('+')[0])",
    )
    .ok()
    .filter(|s| !s.is_empty())
}

/// CUDA 版本标签候选链，按“与已装 torch 版本匹配”推导。
///
/// 为什么不能写死一个：官方索引的 CUDA 变体是**跟着 torch 版本走**的。
/// 实测（2026-10）`cu124` 最高只有 torch 2.6.0，而 marker 2.x 要求 `torch>=2.7`，
/// 写死 cu124 会永远装不上。torch 2.7+ 的 Windows 轮子分布在 cu126 / cu128 / cu129。
///
/// `GUAXI_TORCH_CUDA` 可覆盖（如 cu128）；`GUAXI_TORCH_CUDA_INDEX` 可完全接管索引列表。
fn cuda_variants(base_torch_version: Option<&str>) -> Vec<String> {
    if let Ok(v) = std::env::var("GUAXI_TORCH_CUDA") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return vec![v];
        }
    }

    let minor: Option<u32> = base_torch_version
        .and_then(|v| v.split('.').nth(1))
        .and_then(|m| m.parse().ok());

    match minor {
        // torch 2.7+：cu124 已停更，优先 cu126（覆盖面最广），再退 cu128 / cu129。
        Some(m) if m >= 7 => vec!["cu126".into(), "cu128".into(), "cu129".into()],
        // 老版本 torch（MinerU 现状）沿用 cu124 优先，保持既有行为不变。
        _ => vec!["cu124".into(), "cu126".into()],
    }
}

/// CUDA torch 轮子索引候选（PEP503，供 pip/uv 的 `--index-url` 使用）。
///
/// 现实情况（已实测）：国内并没有可用的 pytorch-wheels 镜像能提供 torch>=2.7 的 CUDA 轮子
/// ——阿里云 `pytorch-wheels` 是扁平目录（不是 simple index）且最高只到 torch 2.6.0，
/// 上交镜像当前不可达，清华/bfsu/ustc/nju 都没有这个镜像。因此这里以**官方索引**为准，
/// 靠 [`super::net`] 的“跟随系统代理”来提速；同时保留环境变量让用户接入自己的镜像。
fn cuda_torch_indexes(variant: &str) -> Vec<String> {
    if let Ok(v) = std::env::var("GUAXI_TORCH_CUDA_INDEX") {
        let custom: Vec<String> = v
            .split(';')
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !custom.is_empty() {
            return custom;
        }
    }

    vec![
        format!("https://download.pytorch.org/whl/{variant}"),
        format!("https://mirror.sjtu.edu.cn/pytorch-wheels/{variant}"),
    ]
}

/// 在存在 NVIDIA GPU 时，把 venv 里的 CPU 版 torch 换成 CUDA 版。
/// 幂等：若 torch.cuda 已可用则直接返回。best-effort：失败仅记录，保留 CPU 版继续可用。
///
/// 关键点：
/// - 按已装 torch 的**基础版本**锁定 `torch==<base>`，避免换到不兼容的大版本；
/// - 已装 CPU 版与 CUDA 版公有版本相同，pip/uv 会判定“已满足”，故必须强制重装；
/// - `--no-deps` 避免误动其它依赖；`--index-url` 只指向 CUDA 源，确保拿到的是 GPU 轮子；
/// - 每个候选源装完都**实际校验** `torch.cuda.is_available()`，不通过就换下一个源；
/// - 全部失败 → 保留 CPU 版（引擎仍可用，只是慢），并记录原因供 UI 展示。
fn ensure_cuda_torch(spec: &PythonLaunchSpec, engine_label: &str) -> Result<bool, String> {
    let program = spec.program.as_str();
    let base_args = spec.base_args.as_slice();

    if torch_cuda_available(program, base_args) {
        info!(engine = engine_label, "[convert] 已检测到可用 CUDA torch");
        return Ok(true);
    }
    if !nvidia_gpu_present() {
        info!(engine = engine_label, "[convert] 未检测到 NVIDIA GPU，使用 CPU");
        return Ok(false);
    }

    let base = torch_base_version(program, base_args);
    let torch_spec = match &base {
        Some(v) => format!("torch=={}", v),
        None => "torch".to_string(),
    };
    let variants = cuda_variants(base.as_deref());

    let mut last_error = String::new();
    for variant in &variants {
        for index in cuda_torch_indexes(variant) {
            info!(
                engine = engine_label,
                variant = variant.as_str(),
                index = index.as_str(),
                spec = torch_spec.as_str(),
                "[convert] 尝试从该 CUDA 源安装 torch"
            );

            let attempt = install_torch_from_index(spec, &index, &torch_spec);
            match attempt {
                Ok(true) => {
                    if torch_cuda_available(program, base_args) {
                        info!(
                            engine = engine_label,
                            index = index.as_str(),
                            "[convert] CUDA torch 安装成功，将使用 GPU"
                        );
                        return Ok(true);
                    }
                    last_error = format!("{index} 安装完成但 torch.cuda 仍不可用");
                    warn!(engine = engine_label, error = last_error.as_str(), "[convert] 校验未通过，尝试下一个源");
                }
                Ok(false) => {
                    last_error = format!("{index} 安装命令失败");
                    warn!(engine = engine_label, error = last_error.as_str(), "[convert] 尝试下一个源");
                }
                Err(e) => {
                    last_error = e;
                    warn!(engine = engine_label, error = last_error.as_str(), "[convert] 尝试下一个源");
                }
            }
        }
    }

    warn!(
        engine = engine_label,
        error = last_error.as_str(),
        "[convert] 所有 CUDA 源均未成功，保留 CPU 版 torch（仍可用，仅速度较慢）"
    );
    Ok(false)
}

/// 从指定索引安装 torch（优先 uv，缺失时回退 `python -m pip`）。
/// 返回 Ok(true) 表示命令成功；Ok(false) 表示命令失败（细节已记日志）。
fn install_torch_from_index(
    spec: &PythonLaunchSpec,
    index: &str,
    torch_spec: &str,
) -> Result<bool, String> {
    let args: Vec<String> = vec![
        "--reinstall".to_string(),
        "--no-deps".to_string(),
        "--index-url".to_string(),
        index.to_string(),
        torch_spec.to_string(),
    ];
    let (ok, _stdout, stderr) = code_runtime::install_pip_requirements(spec, &args)?;
    if !ok {
        let tail: String = stderr
            .lines()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        warn!(error = tail.as_str(), "[convert] torch 安装命令失败");
    }
    Ok(ok)
}

// ───────────────────────── 内嵌脚本 ─────────────────────────

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

/// MinerU 强引擎的 pip 依赖（重，含 torch 与自有模型）。首次装配耗时长；
/// 仅在“需要 mineru 且未装配”时按需触发。模型首次运行按需下载。
const MINERU_ENGINE_PACKAGES: &[&str] = &["mineru[core]"];

// ───────────────────────── Marker 独立运行环境 ─────────────────────────

/// Marker 独立 venv 的根目录（相对 `utils/`）。venv 落在其下的 `.venv`。
/// 刻意选在 `MarkdownConvertor` 下：与新 venv 不冲突（旧 conda 版在 `marker-1.10.1/.venv`），
/// 且与设置页展示的路径语义一致。
pub const MARKER_RUNTIME_REL: &str = "utils/MarkdownConvertor";

/// marker / surya 源码在 runtime 下的缓存目录名。
const MARKER_SRC_DIRNAME: &str = "src";

/// Guaxi 定制分支所在的仓库（源码从这个分支拉取，保证补丁生效）。
const MARKER_REPO_URL: &str = "https://github.com/xhcstar/marker.git";
const SURYA_REPO_URL: &str = "https://github.com/xhcstar/surya.git";
const GUAXI_BRANCH: &str = "guaxi-mods";

/// Marker 独立 venv 使用的 Python 版本。默认与托管运行时一致（3.13，uv 已下载过解释器，
/// 省一次下载；已核实 opencv `cp37-abi3`、pypdfium2 `py3`、pdftext `py3` 轮子在 3.13 上可用）。
/// 可用 `GUAXI_MARKER_PY` 覆盖（真机遇到缺轮子时可退回 3.12）。
const MARKER_PYTHON_VERSION: &str = "3.13";

/// 装配状态持久化文件（相对 `utils/`）。跨重启保留，用于“上次没装完、下次接着装”。
const PROVISION_STATE_REL: &str = "utils/convert_provision.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedProvisionState {
    /// marker 运行环境（venv + 依赖）是否已确认可用。
    marker_env_ready: bool,
    /// 最近一次 marker 运行环境装配的错误（成功时清空）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    marker_env_error: Option<String>,
    /// marker/surya 源码目录是否已就绪。
    marker_src_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    marker_src_error: Option<String>,
    /// marker 模型是否已确认齐全。
    marker_models_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    marker_models_error: Option<String>,
    /// marker 是否走 GPU。
    marker_cuda: bool,
    /// marker 是否处于降级模式（源码不可用，用 PyPI 版 marker-pdf）。
    #[serde(default)]
    marker_degraded: bool,
    /// 最近更新时间（Unix 秒）。
    updated_at: u64,
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn provision_state_path() -> Option<PathBuf> {
    bundled_resources::ensure_relative_dir(PROVISION_STATE_REL).ok()
}

fn load_persisted_state() -> PersistedProvisionState {
    let Some(path) = provision_state_path() else {
        return PersistedProvisionState::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => PersistedProvisionState::default(),
    }
}

fn save_persisted_state(state: &PersistedProvisionState) {
    let Some(path) = provision_state_path() else {
        return;
    };
    let mut snapshot = state.clone();
    snapshot.updated_at = now_epoch_secs();
    match serde_json::to_vec_pretty(&snapshot) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&path, bytes) {
                warn!(error = %e, "[convert] 写入装配状态失败");
            }
        }
        Err(e) => warn!(error = %e, "[convert] 序列化装配状态失败"),
    }
}

/// 更新持久化状态并落盘。
fn update_persisted(f: impl FnOnce(&mut PersistedProvisionState)) {
    let mut state = load_persisted_state();
    f(&mut state);
    save_persisted_state(&state);
}

/// 已解析的引擎运行时（Python 解释器 + 脚本目录 [+ 源码目录]）。
#[derive(Debug, Clone)]
pub struct EngineRuntime {
    pub program: String,
    pub base_args: Vec<String>,
    pub script_dir: PathBuf,
    /// marker 专用：marker / surya 源码目录（供 PYTHONPATH 与 uv 安装使用）。
    pub marker_src: Option<PathBuf>,
    pub surya_src: Option<PathBuf>,
}

static FAST_RUNTIME: OnceLock<Mutex<Option<EngineRuntime>>> = OnceLock::new();
static MARKER_RUNTIME: OnceLock<Mutex<Option<EngineRuntime>>> = OnceLock::new();
static MINERU_RUNTIME: OnceLock<Mutex<Option<EngineRuntime>>> = OnceLock::new();
static PROVISION_STATE: OnceLock<Mutex<ProvisionStatus>> = OnceLock::new();
static PROVISION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn fast_runtime_cell() -> &'static Mutex<Option<EngineRuntime>> {
    FAST_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn marker_runtime_cell() -> &'static Mutex<Option<EngineRuntime>> {
    MARKER_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn mineru_runtime_cell() -> &'static Mutex<Option<EngineRuntime>> {
    MINERU_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn status_cell() -> &'static Mutex<ProvisionStatus> {
    PROVISION_STATE.get_or_init(|| Mutex::new(ProvisionStatus::default()))
}

/// 串行化装配：启动期后台装配与用户手动触发的按需装配可能并发，
/// uv 对同一目录有文件锁，串行能避免互相踩踏与重复下载。
///
/// 注意：`std::sync::Mutex` **不可重入**。任何已持锁的函数都不得再调用另一个会加锁的函数
/// （历史上 `ensure_marker_models_blocking` 就曾因此自锁），需要嵌套时必须在取锁前完成。
fn provision_lock() -> &'static Mutex<()> {
    PROVISION_LOCK.get_or_init(|| Mutex::new(()))
}

/// 取装配锁；即使此前有线程 panic 导致锁中毒，也恢复继续用（装配是幂等操作）。
fn lock_provision() -> std::sync::MutexGuard<'static, ()> {
    provision_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 转换引擎装配状态（供前端展示“转换引擎是否就绪”）。
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
    /// 极速引擎（anydoc，纯 Rust 进程内）是否就绪。编译进二进制、无装配过程，恒为 true。
    pub anydoc_ready: bool,
    /// Marker 强引擎是否可用（独立 uv venv 或旧的 conda 分发版任一就绪）。
    pub marker_ready: bool,
    /// Marker 强引擎装配是否正在进行。
    pub marker_provisioning: bool,
    /// 最近一次 marker 装配错误（若有）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_error: Option<String>,
    /// Marker 是否使用 GPU（CUDA）加速。
    pub marker_cuda: bool,
    /// Marker 是否处于降级模式（源码不可用，改用 PyPI 版 marker-pdf）。
    pub marker_degraded: bool,
    /// Marker 模型是否已齐全。
    pub marker_models_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_models_error: Option<String>,
    /// 模型目录（供 UI 显示）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_models_dir: Option<String>,
    /// 运行环境目录（供 UI 显示）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_env_dir: Option<String>,
    /// MinerU 强引擎是否可用。
    pub mineru_ready: bool,
    /// MinerU 是否使用 GPU（CUDA）加速。
    pub mineru_cuda: bool,
}

/// 读取当前装配状态快照（合并持久化状态与实时探测结果）。
pub fn current_status() -> ProvisionStatus {
    let mut s = status_cell().lock().map(|g| g.clone()).unwrap_or_default();
    let persisted = load_persisted_state();

    // 极速引擎编译进二进制，不存在装配步骤，恒就绪。
    s.anydoc_ready = true;

    // marker 就绪 = 独立 venv 已装配 或 旧的 conda 分发版可用。
    s.marker_ready = marker_runtime().is_some() || super::engine::marker_available();
    s.marker_cuda = marker_uses_cuda() || persisted.marker_cuda;
    s.marker_degraded = marker_uses_degraded_pypi() || persisted.marker_degraded;
    s.marker_models_ready = persisted.marker_models_ready || marker_models_ready();
    if let Ok(dir) = marker_model_dir() {
        s.marker_models_dir = Some(dir.to_string_lossy().to_string());
    }
    if let Ok(root) = marker_runtime_root() {
        s.marker_env_dir = Some(root.join(".venv").to_string_lossy().to_string());
    }
    if s.marker_error.is_none() {
        s.marker_error = persisted
            .marker_env_error
            .clone()
            .or_else(|| persisted.marker_src_error.clone());
    }
    if s.marker_models_error.is_none() {
        s.marker_models_error = persisted.marker_models_error.clone();
    }
    s.mineru_ready = super::engine::mineru_available() || mineru_runtime().is_some();
    s.mineru_cuda = mineru_uses_cuda();
    s
}

/// 快速引擎是否已就绪（脚本已物化 + 依赖已装）。
pub fn fast_runtime() -> Option<EngineRuntime> {
    fast_runtime_cell().lock().ok().and_then(|g| g.clone())
}

/// Marker 引擎运行时（独立 uv venv，若已装配）。
pub fn marker_runtime() -> Option<EngineRuntime> {
    marker_runtime_cell().lock().ok().and_then(|g| g.clone())
}

/// MinerU 运行时（若已装配）。
pub fn mineru_runtime() -> Option<EngineRuntime> {
    mineru_runtime_cell().lock().ok().and_then(|g| g.clone())
}

fn update_status(f: impl FnOnce(&mut ProvisionStatus)) {
    if let Ok(mut g) = status_cell().lock() {
        f(&mut g);
    }
}

/// 将内嵌脚本物化到指定目录（仅在内容变化时写盘）。
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
            std::fs::write(&path, source).map_err(|e| format!("写入 {name} 失败: {e}"))?;
            info!(script = %path.to_string_lossy(), "[convert] 物化引擎脚本");
        }
    }
    Ok(())
}

// ───────────────────────── 快速引擎 ─────────────────────────

/// 同步装配快速引擎：确保托管 Python + 依赖 + 脚本就绪。阻塞，放到 spawn_blocking 中调用。
/// 幂等：已装配时开销极小（pip list 比对 + 脚本内容比对）。
pub fn ensure_fast_engine_blocking() -> Result<EngineRuntime, String> {
    if let Some(rt) = fast_runtime() {
        return Ok(rt);
    }

    update_status(|s| {
        s.fast_provisioning = true;
        s.fast_error = None;
    });

    let packages: Vec<String> = FAST_ENGINE_PACKAGES.iter().map(|s| s.to_string()).collect();
    let result = (|| -> Result<EngineRuntime, String> {
        // 复用应用既有的托管 Python 装配（自动下载 Python + venv + 镜像 + 幂等安装）。
        let (program, base_args) = code_runtime::ensure_managed_python_with_packages(
            &packages,
            "doc-convert-fast-engine",
        )?;
        let script_dir = code_runtime::managed_runtime_dir()?;
        materialize_scripts(&script_dir)?;
        Ok(EngineRuntime {
            program,
            base_args,
            script_dir,
            marker_src: None,
            surya_src: None,
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

// ───────────────────────── Marker：源码 + 独立 venv ─────────────────────────

/// Marker 独立运行时根目录（`utils/MarkdownConvertor`）。
pub fn marker_runtime_root() -> Result<PathBuf, String> {
    let root = bundled_resources::ensure_relative_dir(MARKER_RUNTIME_REL)?;
    std::fs::create_dir_all(&root).map_err(|e| format!("创建 marker 运行时目录失败: {e}"))?;
    Ok(root)
}

/// surya 模型目录（作为 HF_HOME / HUGGINGFACE_HUB_CACHE / MODEL_CACHE_DIR 的统一落点）。
pub fn marker_model_dir() -> Result<PathBuf, String> {
    let dir = bundled_resources::ensure_relative_dir("utils/Models/surya_models")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建模型目录失败: {e}"))?;
    Ok(dir)
}

/// marker / surya 源码目录。
#[derive(Debug, Clone)]
pub struct MarkerSources {
    pub marker: PathBuf,
    pub surya: PathBuf,
}

fn src_looks_valid(dir: &Path) -> bool {
    dir.join("pyproject.toml").is_file()
}

fn marker_sources_paths() -> Result<MarkerSources, String> {
    let root = marker_runtime_root()?;
    let src = root.join(MARKER_SRC_DIRNAME);
    Ok(MarkerSources {
        marker: src.join("marker"),
        surya: src.join("surya"),
    })
}

/// 从 Guaxi fork 拉取源码（zipball，先走 GitHub 加速镜像再直连）。
///
/// 为什么用 zipball 而不是 git clone：仓库里已有一套经过验证的 zip 解压逻辑可复用，
/// 且不必假设用户机器上有 git。
fn download_branch_zipball(repo_url: &str, dest_dir: &Path, label: &str) -> Result<(), String> {
    let repo_path = repo_url
        .trim_end_matches(".git")
        .trim_start_matches("https://github.com/");
    if repo_path.is_empty() {
        return Err(format!("无法解析仓库地址: {repo_url}"));
    }
    let zip_url = format!(
        "https://github.com/{repo_path}/archive/refs/heads/{GUAXI_BRANCH}.zip"
    );

    let root = marker_runtime_root()?;
    let tmp_zip = root.join(format!("{label}-{GUAXI_BRANCH}.zip"));
    let tmp_dir = root.join(format!(".extract-{label}"));

    net::download_github_url_with_mirror(&zip_url, &tmp_zip)?;

    if tmp_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建解压目录失败: {e}"))?;

    let extract_result = (|| -> Result<(), String> {
        code_runtime::extract_zip_to_dir(&tmp_zip, &tmp_dir)?;

        // zipball 顶层是 `<repo>-<branch>/`，需要找到那唯一一层。
        let mut top: Option<PathBuf> = None;
        for entry in std::fs::read_dir(&tmp_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if top.is_some() {
                    return Err(format!("{label} 压缩包结构异常：顶层有多个目录"));
                }
                top = Some(entry.path());
            }
        }
        let top = top.ok_or_else(|| format!("{label} 压缩包内没有目录"))?;

        if dest_dir.exists() {
            let _ = std::fs::remove_dir_all(dest_dir);
        }
        if let Some(parent) = dest_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&top, dest_dir)
            .map_err(|e| format!("发布 {label} 源码失败: {e}"))?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_file(&tmp_zip);

    extract_result?;
    if !src_looks_valid(dest_dir) {
        return Err(format!(
            "{label} 源码校验失败（缺 pyproject.toml）：{}",
            dest_dir.to_string_lossy()
        ));
    }
    info!(dest = dest_dir.to_string_lossy().as_ref(), "[convert] 源码就绪");
    Ok(())
}

/// 确保 marker / surya 源码就绪。
///
/// 来源优先级：
/// 1. `GUAXI_MARKER_SRC_DIR`：本地目录覆盖（开发 / 离线），其下应有 `marker/` 与 `surya/`；
/// 2. 已缓存的 `utils/MarkdownConvertor/src/{marker,surya}`；
/// 3. 从 Guaxi fork 的 `guaxi-mods` 分支下载 zipball（先镜像后直连）。
pub fn ensure_marker_sources_blocking() -> Result<MarkerSources, String> {
    if let Ok(override_dir) = std::env::var("GUAXI_MARKER_SRC_DIR") {
        let trimmed = override_dir.trim();
        if !trimmed.is_empty() {
            let base = PathBuf::from(trimmed);
            let sources = MarkerSources {
                marker: base.join("marker"),
                surya: base.join("surya"),
            };
            if src_looks_valid(&sources.marker) && src_looks_valid(&sources.surya) {
                info!(base = trimmed, "[convert] 使用 GUAXI_MARKER_SRC_DIR 指定的源码");
                update_persisted(|s| {
                    s.marker_src_ready = true;
                    s.marker_src_error = None;
                });
                return Ok(sources);
            }
            return Err(format!(
                "GUAXI_MARKER_SRC_DIR 下未找到 marker/ 与 surya/ 源码（缺 pyproject.toml）：{trimmed}"
            ));
        }
    }

    let sources = marker_sources_paths()?;
    if src_looks_valid(&sources.marker) && src_looks_valid(&sources.surya) {
        update_persisted(|s| {
            s.marker_src_ready = true;
            s.marker_src_error = None;
        });
        return Ok(sources);
    }

    let result = (|| -> Result<(), String> {
        download_branch_zipball(MARKER_REPO_URL, &sources.marker, "marker")?;
        download_branch_zipball(SURYA_REPO_URL, &sources.surya, "surya")?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            update_persisted(|s| {
                s.marker_src_ready = true;
                s.marker_src_error = None;
            });
            Ok(sources)
        }
        Err(error) => {
            update_persisted(|s| {
                s.marker_src_ready = false;
                s.marker_src_error = Some(error.clone());
            });
            Err(error)
        }
    }
}

/// 独立 venv 的 Python 解释器（用于其它模块复用，如 ModelScope 下载）。
pub fn marker_env_python() -> Option<PathBuf> {
    venv_python(&marker_runtime_root().ok()?.join(".venv"))
}

/// 校验过的 venv 解释器路径（Windows 优先 `Scripts/python.exe`）。
fn venv_python(venv_dir: &Path) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if cfg!(windows) {
        vec![
            venv_dir.join("Scripts").join("python.exe"),
            venv_dir.join("python.exe"),
        ]
    } else {
        vec![venv_dir.join("bin").join("python")]
    };
    candidates.into_iter().find(|p| p.is_file())
}

/// 供其它模块（ModelScope 下载等）复用的“辅助 Python 解释器”。
///
/// 顺序刻意与改造前保持一致：**优先旧 conda 分发版**，这样本来就跑得通的机器行为完全不变；
/// 之后依次回退到 marker 独立 venv 与托管运行时——后者在应用启动后总是存在，
/// 因此即使机器上完全没有 conda 环境，这条链路也不会因为“找不到解释器”直接断掉。
pub fn resolve_aux_python() -> Option<PathBuf> {
    if let Ok(legacy) = bundled_resources::ensure_relative_dir("utils/MarkdownConvertor/marker-1.10.1")
    {
        if let Some(python) = venv_python(&legacy.join(".venv")) {
            return Some(python);
        }
    }
    if let Some(python) = marker_env_python() {
        return Some(python);
    }
    let managed = code_runtime::managed_runtime_dir().ok()?;
    venv_python(&managed.join(".venv"))
}

/// 独立 venv 是否“看起来”已装配（廉价的文件系统检查，不做子进程探测）。
fn marker_env_looks_ready(root: &Path) -> bool {
    let venv = root.join(".venv");
    let has_python = if cfg!(windows) {
        venv.join("Scripts").join("python.exe").is_file()
    } else {
        venv.join("bin").join("python").is_file()
    };
    has_python && venv.join("pyvenv.cfg").is_file() && root.join("runtime.json").is_file()
}

/// 用一次子进程探测 marker/surya 是否真的能 import，返回版本串。
///
/// 刻意 import 到 `marker.models`（它会把 surya 的布局 / 推理 / OCR 客户端一并拉起来），
/// 这样 torch、transformers、httpx、opencv 等运行时依赖缺任何一个都能在装配阶段暴露，
/// 而不是等到用户真正转换时才失败。只 import 不加载权重，代价很低。
///
/// `sources` 为 None 表示“降级模式”（源码不可用、装的是 PyPI 版），此时不设 PYTHONPATH，
/// 直接让解释器从 site-packages 里找。
fn probe_marker_env(
    spec: &PythonLaunchSpec,
    sources: Option<&MarkerSources>,
) -> Result<String, String> {
    let probe_code = "import marker, surya\n\
                      from marker.models import create_model_dict\n\
                      from surya.inference import SuryaInferenceManager\n\
                      print(getattr(surya, '__version__', ''))";
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.base_args)
        .arg("-c")
        .arg(probe_code)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(sources) = sources {
        let sep = if cfg!(windows) { ";" } else { ":" };
        cmd.env(
            "PYTHONPATH",
            format!(
                "{}{sep}{}",
                sources.marker.to_string_lossy(),
                sources.surya.to_string_lossy()
            ),
        );
    }
    no_window(&mut cmd);
    let out = cmd.output().map_err(|e| format!("启动探测进程失败: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// 按需装配 Marker 独立运行环境（源码 + uv venv + 依赖 + 可选 CUDA torch）。
///
/// 幂等：源码与 venv 都已就绪且能 import 时，只做一次廉价探测便返回。
/// 阻塞，放到 spawn_blocking 中调用。
pub fn ensure_marker_engine_blocking() -> Result<EngineRuntime, String> {
    if let Some(rt) = marker_runtime() {
        return Ok(rt);
    }

    let _guard = lock_provision();

    // 双重检查：等锁期间可能已被启动装配完成。
    if let Some(rt) = marker_runtime() {
        return Ok(rt);
    }

    update_status(|s| {
        s.marker_provisioning = true;
        s.marker_error = None;
    });

    let result = (|| -> Result<EngineRuntime, String> {
        let root = marker_runtime_root()?;

        // 源码可用性单独处理：拉不到源码**不是致命错误**——可以降级为从 PyPI 装 marker-pdf。
        // 那部分定制能力在 App 侧都有等价实现（进度协议来自本仓库的 guaxi_marker_convert.py，
        // 模型目录由 MODEL_CACHE_DIR 环境变量指定），因此降级后功能仍然完整。
        let sources = match ensure_marker_sources_blocking() {
            Ok(sources) => Some(sources),
            Err(error) => {
                warn!(
                    error = error.as_str(),
                    "[convert] Guaxi 定制源码不可用，降级为 PyPI 版 marker-pdf"
                );
                None
            }
        };
        let degraded = sources.is_none();

        // 廉价快路径：环境看起来就绪时只做一次 import 探测。
        let mut spec: Option<PythonLaunchSpec> = None;
        if marker_env_looks_ready(&root) {
            if let Ok(existing) = code_runtime::isolated_runtime_spec(&root) {
                if probe_marker_env(&existing, sources.as_ref()).is_ok() {
                    spec = Some(existing);
                }
            }
        }

        let spec = match spec {
            Some(spec) => {
                info!(python = spec.program.as_str(), "[convert] 复用已就绪的 marker 独立运行环境");
                spec
            }
            None => build_marker_env(&root, sources.as_ref())?,
        };

        // GPU：有 NVIDIA 卡且当前 torch 看不到 CUDA 时，换成 CUDA 轮子。
        let cuda = ensure_cuda_torch(&spec, "marker").unwrap_or(false);
        MARKER_CUDA.store(cuda, Ordering::Relaxed);
        MARKER_DEGRADED.store(degraded, Ordering::Relaxed);

        let script_dir = code_runtime::managed_runtime_dir()?;
        materialize_scripts(&script_dir)?;

        Ok(EngineRuntime {
            program: spec.program.clone(),
            base_args: spec.base_args.clone(),
            script_dir,
            marker_src: sources.as_ref().map(|s| s.marker.clone()),
            surya_src: sources.as_ref().map(|s| s.surya.clone()),
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
                s.marker_cuda = marker_uses_cuda();
            });
            update_persisted(|s| {
                s.marker_env_ready = true;
                s.marker_env_error = None;
                s.marker_cuda = marker_uses_cuda();
                s.marker_degraded = marker_uses_degraded_pypi();
            });
            info!(python = %rt.program, "[convert] Marker 运行环境装配完成");
        }
        Err(e) => {
            update_status(|s| {
                s.marker_provisioning = false;
                s.marker_error = Some(e.clone());
            });
            update_persisted(|s| s.marker_env_error = Some(e.clone()));
            warn!(error = %e, "[convert] Marker 运行环境装配失败");
        }
    }

    result
}

/// 建/重建 marker 独立 venv 并安装 marker（与 surya）。
///
/// `sources` 为 Some 时从 Guaxi fork 源码安装（仓库里的定制补丁生效）；
/// 为 None 时降级从 PyPI 安装 `marker-pdf`——源码拉不到（例如 GitHub 不可达）时
/// 仍然保证能建出一个可用的运行环境，而不是把错误直接抛给用户。
fn build_marker_env(root: &Path, sources: Option<&MarkerSources>) -> Result<PythonLaunchSpec, String> {
    let target_version = std::env::var("GUAXI_MARKER_PY")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| MARKER_PYTHON_VERSION.to_string());

    // 第一层自愈：建 venv（uv 优先，内部已实现“镜像失败直连 GitHub 重试”）。
    let spec = code_runtime::ensure_isolated_venv_with_packages(
        root,
        Some(target_version.as_str()),
        &[],
        "doc-convert-marker-engine",
    )?;

    // 幂等：已装好且能 import 就直接用（后续启动走的就是这条）。
    if probe_marker_env(&spec, sources).is_ok() {
        return Ok(spec);
    }

    match sources {
        Some(sources) => {
            // 第二层自愈：先装 surya（marker 依赖 surya-ocr，需先满足依赖解析）。
            let surya_target = vec!["-e".to_string(), sources.surya.to_string_lossy().to_string()];
            install_with_index_retry(&spec, &surya_target, "surya")?;

            let marker_target = vec!["-e".to_string(), sources.marker.to_string_lossy().to_string()];
            install_with_index_retry(&spec, &marker_target, "marker")?;
        }
        None => {
            install_with_index_retry(&spec, &["marker-pdf".to_string()], "marker-pdf")?;
        }
    }

    match probe_marker_env(&spec, sources) {
        Ok(versions) => {
            info!(versions = versions.as_str(), "[convert] marker 依赖安装完成并探测通过");
            Ok(spec)
        }
        Err(error) => Err(format!(
            "marker 运行环境安装后仍无法导入（通常是某个依赖缺失）: {}",
            tail_lines(&error, 6)
        )),
    }
}

/// 按“先清华镜像、再官方 PyPI”的顺序安装一个目标，两次都失败才报错。
fn install_with_index_retry(
    spec: &PythonLaunchSpec,
    target_args: &[String],
    label: &str,
) -> Result<(), String> {
    let mut args = target_args.to_vec();
    args.extend([
        "--index-url".to_string(),
        code_runtime::PYTHON_PIP_INDEX_URL.to_string(),
    ]);
    let (ok, _out, err) = code_runtime::install_pip_requirements(spec, &args)?;
    if ok {
        return Ok(());
    }

    // 国内镜像偶发缺包或同步延迟：换官方 PyPI 再试一次。
    warn!(
        label = label,
        error = tail_lines(&err, 4).as_str(),
        "[convert] 国内镜像安装失败，改用官方 PyPI 重试"
    );
    let mut retry_args = target_args.to_vec();
    retry_args.extend(["--index-url".to_string(), "https://pypi.org/simple".to_string()]);
    let (retry_ok, _out2, err2) = code_runtime::install_pip_requirements(spec, &retry_args)?;
    if retry_ok {
        return Ok(());
    }
    Err(format!(
        "安装 {label} 失败（已尝试国内镜像与官方 PyPI）: {}",
        tail_lines(&err2, 6)
    ))
}

fn tail_lines(text: &str, count: usize) -> String {
    text.lines()
        .rev()
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

// ───────────────────────── Marker：模型 ─────────────────────────

/// HF 缓存里某个仓库目录是否存在（`HUGGINGFACE_HUB_CACHE` 直接指向模型目录）。
fn hf_repo_cached(model_dir: &Path, repo_id: &str) -> bool {
    let dir_name = format!("models--{}", repo_id.replace('/', "--"));
    let dir = model_dir.join(dir_name);
    if !dir.is_dir() {
        return false;
    }
    // snapshot 目录下有真实文件才算数（只有 refs/locks 的空壳不算）。
    let snapshots = dir.join("snapshots");
    let Ok(entries) = std::fs::read_dir(&snapshots) else {
        return false;
    };
    entries.flatten().any(|entry| entry.path().is_dir())
}

/// 模型是否齐全（廉价文件系统检查，供 UI 与启动装配判断）。
pub fn marker_models_ready() -> bool {
    let Ok(dir) = marker_model_dir() else {
        return false;
    };
    hf_repo_cached(&dir, "datalab-to/surya-ocr-2-gguf")
        && hf_repo_cached(&dir, "datalab-to/surya_layout2")
        && dir.join("ocr_error_detection").join("2025_02_18").join("manifest.json").is_file()
}

/// 下载 marker/surya 模型（阻塞）。交给 fork 里的 `download_surya_models.py`：
/// s3 走 models.datalab.to，HF 走 HF_ENDPOINT（默认 hf-mirror），全链路跟随系统代理。
pub fn ensure_marker_models_blocking() -> Result<(), String> {
    if marker_models_ready() {
        update_persisted(|s| {
            s.marker_models_ready = true;
            s.marker_models_error = None;
        });
        return Ok(());
    }

    // 关键：运行环境装配**必须先做且不带锁**——它内部自己会取装配锁，
    // 若先取锁再调用就会在同线程二次加锁，直接死锁。
    let rt = ensure_marker_engine_blocking()?;

    let _guard = lock_provision();
    if marker_models_ready() {
        return Ok(());
    }

    update_status(|s| s.marker_models_error = None);

    let result = (|| -> Result<(), String> {
        let Some(surya_src) = rt.surya_src.clone() else {
            // 降级模式（源码不可用、装的是 PyPI 版 marker）：没有 fork 里的下载脚本，
            // 模型只能交给 surya 在首次转换时按需下载——apply_marker_env 已把
            // HF_ENDPOINT 指向国内镜像，因此这条路仍然可用，只是时机更晚。
            return Err(
                "缺少 surya 源码，无法预下载模型；首次转换时会由 surya 自行按需下载".to_string(),
            );
        };
        let script = surya_src.join("download_surya_models.py");
        if !script.is_file() {
            return Err(format!(
                "找不到模型下载脚本（请检查 surya 源码是否完整）：{}",
                script.display()
            ));
        }

        let utils_dir = bundled_resources::ensure_relative_dir("utils")?;
        let model_dir = marker_model_dir()?;

        let mut cmd = Command::new(&rt.program);
        cmd.args(&rt.base_args)
            .arg("-u")
            .arg(&script)
            .arg("--dest")
            .arg(&utils_dir)
            .arg("--dir")
            .arg("Models/surya_models")
            .arg("--progress_json")
            .env("PYTHONUTF8", "1")
            .env("PYTHONIOENCODING", "utf-8")
            // 模型必须落到应用目录，保证可随安装目录分发 / 离线加载。
            .env("HF_HOME", model_dir.to_string_lossy().to_string())
            .env("HUGGINGFACE_HUB_CACHE", model_dir.to_string_lossy().to_string())
            .env("MODEL_CACHE_DIR", model_dir.to_string_lossy().to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // HF 官方域名在大陆直连不可达；默认指向国内镜像，用户已有设置优先。
        if std::env::var("HF_ENDPOINT").is_err() {
            cmd.env("HF_ENDPOINT", "https://hf-mirror.com");
        }
        net::apply_proxy_env(&mut cmd);
        no_window(&mut cmd);

        let output = cmd.output().map_err(|e| format!("启动模型下载失败: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "模型下载失败: {}",
                tail_lines(stderr.trim(), 8)
            ));
        }

        if !marker_models_ready() {
            return Err("模型下载命令已结束，但关键模型仍不完整".to_string());
        }
        Ok(())
    })();

    match &result {
        Ok(()) => {
            update_status(|s| {
                s.marker_models_ready = true;
                s.marker_models_error = None;
            });
            update_persisted(|s| {
                s.marker_models_ready = true;
                s.marker_models_error = None;
            });
            info!("[convert] Marker 模型就绪");
        }
        Err(e) => {
            update_status(|s| s.marker_models_error = Some(e.clone()));
            update_persisted(|s| {
                s.marker_models_ready = false;
                s.marker_models_error = Some(e.clone());
            });
            warn!(error = %e, "[convert] Marker 模型准备失败（首次转换时会按需重试）");
        }
    }

    result
}

// ───────────────────────── MinerU ─────────────────────────

/// 按需装配托管 pip 版 MinerU 强引擎。阻塞，放到 spawn_blocking 中调用。幂等。
/// 依赖很重（torch + mineru + 模型），仅在“需要 mineru 且未装配”时按需触发。
pub fn ensure_mineru_engine_blocking() -> Result<EngineRuntime, String> {
    if let Some(rt) = mineru_runtime() {
        return Ok(rt);
    }

    let _guard = lock_provision();
    if let Some(rt) = mineru_runtime() {
        return Ok(rt);
    }

    let packages: Vec<String> = MINERU_ENGINE_PACKAGES.iter().map(|s| s.to_string()).collect();
    let result = (|| -> Result<EngineRuntime, String> {
        let (program, base_args) = code_runtime::ensure_managed_python_with_packages(
            &packages,
            "doc-convert-mineru-engine",
        )?;
        let spec = code_runtime::PythonLaunchSpec {
            program: program.clone(),
            base_args: base_args.clone(),
            source: "managed-runtime".to_string(),
            runtime_dir: code_runtime::managed_runtime_dir()?,
        };
        // 有 NVIDIA GPU 时装 CUDA 版 torch，让 MinerU 自动走 GPU 加速。
        let cuda = ensure_cuda_torch(&spec, "mineru").unwrap_or(false);
        MINERU_CUDA.store(cuda, Ordering::Relaxed);
        let script_dir = code_runtime::managed_runtime_dir()?;
        materialize_scripts(&script_dir)?;
        Ok(EngineRuntime {
            program,
            base_args,
            script_dir,
            marker_src: None,
            surya_src: None,
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

// ───────────────────────── 启动装配 ─────────────────────────

/// 启动时后台装配，串行执行：快速引擎 → Marker 运行环境 → Marker 模型。
///
/// 为什么串行：三者都要联网下载（Python 解释器 / torch / 数 GB 模型），并发会互相抢带宽，
/// 也会和首屏加载抢磁盘。每步失败都不阻塞后续，也不阻塞主流程；
/// 失败原因写入持久化状态，下次启动自动继续重试。
///
/// `GUAXI_SKIP_STARTUP_PROVISION=1` 可关闭启动装配（排查问题时用）。
pub fn spawn_startup_provision() {
    if std::env::var("GUAXI_SKIP_STARTUP_PROVISION")
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
    {
        info!("[convert] 已按环境变量跳过启动装配");
        return;
    }

    tauri::async_runtime::spawn(async {
        let res = tokio::task::spawn_blocking(|| {
            // 1) 快速引擎（轻，必备）
            match ensure_fast_engine_blocking() {
                Ok(_) => info!("[convert] 启动装配：快速引擎就绪"),
                Err(e) => warn!(error = %e, "[convert] 启动装配：快速引擎失败（将按需重试/降级）"),
            }

            // 2) Marker 运行环境（重：torch 等）
            match ensure_marker_engine_blocking() {
                Ok(_) => info!("[convert] 启动装配：Marker 运行环境就绪"),
                Err(e) => warn!(error = %e, "[convert] 启动装配：Marker 运行环境失败（将按需重试）"),
            }

            // 3) Marker 模型（最重：数 GB，失败不阻塞转换——首次转换会再试）
            match ensure_marker_models_blocking() {
                Ok(_) => info!("[convert] 启动装配：Marker 模型就绪"),
                Err(e) => warn!(error = %e, "[convert] 启动装配：Marker 模型失败（首次转换时会重试）"),
            }
        })
        .await;

        if let Err(e) = res {
            warn!(error = %e, "[convert] 启动装配任务 join 失败");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_variants_prefers_new_indexes_for_torch_27_plus() {
        // torch>=2.7 时 cu124 已停更，必须选 cu126/cu128/cu129，否则永远装不上。
        assert_eq!(
            cuda_variants(Some("2.7.0")),
            vec!["cu126".to_string(), "cu128".to_string(), "cu129".to_string()]
        );
        // 老版本保持 cu124 优先（MinerU 现状不变）。
        assert_eq!(cuda_variants(Some("2.6.0")).first().map(String::as_str), Some("cu124"));
        // 版本未知时走保守分支，不 panic。
        assert!(!cuda_variants(None).is_empty());
    }

    #[test]
    fn marker_runtime_and_models_use_utils_layout() {
        // 路径契约：venv 与源码都在 utils/MarkdownConvertor 下，模型在 utils/Models/surya_models。
        let root = marker_runtime_root().expect("runtime root");
        assert!(root.to_string_lossy().replace('\\', "/").ends_with("utils/MarkdownConvertor"));
        let model_dir = marker_model_dir().expect("model dir");
        assert!(model_dir
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("utils/Models/surya_models"));
    }
}
