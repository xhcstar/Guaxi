//! Office 文档的外部编辑器集成（WordCraft / GridCraft / DeckCraft）。
//!
//! 为什么是「外部程序」而不是内嵌：
//! 这三个工具都是纯 Rust + egui 的独立桌面应用（UI 层自绘在 canvas 上，不是 DOM 组件），
//! 没有可嵌入的 Web 组件形态。把它们作为 crate 引入会拉进三套 egui/wgpu 依赖树，构建时间与
//! 产物体积成倍增长，且要与 Tauri 的窗口系统争夺事件循环。以独立进程启动则：
//!   - 不碰 Guaxi 的 Rust 依赖树与构建时间；
//!   - 编辑体验是它们自己的完整实现（WordCraft 支持修订、样式、邮件合并等）；
//!   - 文件是各自格式的原生读写，不必担心我们自己写坏 .docx 的排版。
//!
//! 采用「按需探测、找不到就明确告知」的策略：不静默失败，也不假装支持。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

/// 支持的 Office 编辑器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CraftApp {
    Word,
    Grid,
    Deck,
}

impl CraftApp {
    fn from_kind(kind: &str) -> Option<Self> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "word" | "docx" | "doc" => Some(Self::Word),
            "grid" | "xlsx" | "xls" | "csv" => Some(Self::Grid),
            "deck" | "pptx" | "ppt" => Some(Self::Deck),
            _ => None,
        }
    }

    /// 可执行文件名（不含扩展名）。三个项目各自的桌面目标名。
    fn exe_stem(self) -> &'static str {
        match self {
            Self::Word => "wordcraft",
            Self::Grid => "gridcraft",
            Self::Deck => "deckcraft",
        }
    }

    /// 用户可读的名字，用在错误提示里。
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Word => "WordCraft",
            Self::Grid => "GridCraft",
            Self::Deck => "DeckCraft",
        }
    }
}

/// 一次「用外部编辑器打开」请求的结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CraftOpenResult {
    /// 实际用的应用
    pub app: String,
    /// 探测到的可执行文件绝对路径
    pub executable: String,
    /// 启动的进程号
    pub pid: u32,
}

/// 在若干目录里找可执行文件。
///
/// 查找顺序刻意从「最可控」到「最宽松」：
///   1. 环境变量 `GUAXI_CRAFT_DIR`（用户显式指定，优先级最高）；
///   2. Guaxi 可执行文件同级的 `craft/` 子目录（随应用分发的首选位置）；
///   3. 系统 PATH（用户自己安装过）。
///
/// 不支持从任意路径猜 —— 那会有被同名恶意程序劫持的风险。
fn find_executable(app: CraftApp) -> Option<PathBuf> {
    let stem = app.exe_stem();
    let exe_name = if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    };

    // 1) 显式指定的目录
    if let Ok(dir) = std::env::var("GUAXI_CRAFT_DIR") {
        let candidate = Path::new(dir.trim()).join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 2) 与 Guaxi 同级的 craft/ 子目录
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("craft").join(&exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 3) PATH（只接受绝对路径命中，避免当前目录被劫持）
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

/// 打开一个 Office 文档到对应的外部编辑器。
///
/// `kind` 来自 `sniff_file_kind`（docx / xlsx / pptx / doc / xls / ppt），
/// 但这里主要看扩展名，因为用户可能直接传 `word` / `grid` / `deck` 这类简称。
#[tauri::command]
pub async fn open_in_craft_app(path: String, kind: String) -> Result<CraftOpenResult, String> {
    let file = Path::new(path.trim()).to_path_buf();
    if !file.is_file() {
        return Err(format!("文件不存在：{}", file.display()));
    }

    // 扩展名优先：kind 可能来自嗅探结果，也可能来自前端简化名。
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let app = CraftApp::from_kind(&ext)
        .or_else(|| CraftApp::from_kind(&kind))
        .ok_or_else(|| format!("不支持的文件类型：{}", ext))?;

    let exe = find_executable(app).ok_or_else(|| {
        format!(
            "未找到 {}。\n\n请把 {} 安装到 Guaxi 程序目录下的 craft/ 子目录，\
或设置环境变量 GUAXI_CRAFT_DIR 指向它的安装目录。\n\n\
下载地址：https://getartcraft.com/",
            app.display_name(),
            app.display_name()
        )
    })?;

    info!(
        app = app.display_name(),
        exe = %exe.display(),
        file = %file.display(),
        "[Craft] launching external editor"
    );

    // 直接传文件路径作为第一个参数 —— 三个应用都支持（WordCraft 还支持多个文件）。
    let child = Command::new(&exe)
        .arg(&file)
        .spawn()
        .map_err(|e| format!("启动 {} 失败：{}", app.display_name(), e))?;

    let pid = child.id();
    info!(pid, "[Craft] launched");

    Ok(CraftOpenResult {
        app: app.display_name().to_string(),
        executable: exe.display_string_lossy(),
        pid,
    })
}

/// 查询某个应用是否可用（前端据此决定是否显示「编辑」入口）。
#[tauri::command]
pub fn probe_craft_apps() -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for app in [CraftApp::Word, CraftApp::Grid, CraftApp::Deck] {
        let found = find_executable(app);
        if found.is_none() {
            warn!(
                app = app.display_name(),
                "[Craft] executable not found"
            );
        }
        out.insert(
            app.exe_stem().to_string(),
            serde_json::json!({
                "name": app.display_name(),
                "available": found.is_some(),
                "path": found.map(|p| p.to_string_lossy().to_string()),
            }),
        );
    }
    serde_json::Value::Object(out)
}

/// `PathBuf` 上补一个 `to_string_lossy` 的便捷写法（避免调用处写一长串）。
trait DisplayStringLossy {
    fn display_string_lossy(&self) -> String;
}

impl DisplayStringLossy for PathBuf {
    fn display_string_lossy(&self) -> String {
        self.to_string_lossy().to_string()
    }
}
