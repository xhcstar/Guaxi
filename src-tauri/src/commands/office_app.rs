//! 本机 Office 套件的探测与启动（Microsoft Office / WPS）。
//!
//! 为什么要探测而不是直接 `start file.docx`：Windows 的「默认关联」在用户机器上并不可靠
//! —— 有人把 .docx 关联给了记事本、有人关联给了浏览器插件。打开 Office 文档的正确语义是
//! 「找到本机真正的 Office 套件，用它打开」，而不是「让 shell 猜」。
//!
//! 探测顺序（`HKCU` 优先于 `HKLM`：用户装了新版 Office 时，个人配置比机器级更准）：
//!   1. 注册表 `App Paths\WINWORD.EXE` 等 —— Office 安装后会写这一项，且指向真实 exe；
//!   2. WPS 的 `App Paths\wps.exe` / `et.exe` / `wpp.exe`；
//!   3. 已知安装目录兜底（64/32 位 Program Files 各一份）。
//!
//! 三个 doc kind 分别对应：word → winword/wps，grid → excel/et，deck → powerpnt/wpp。
//! 全部找不到时返回 None，由调用方退回 WordCraft / GridCraft / DeckCraft。

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

/// 提供该类型文档的办公套件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OfficeSuite {
    MicrosoftOffice,
    Wps,
}

impl OfficeSuite {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::MicrosoftOffice => "Microsoft Office",
            Self::Wps => "WPS Office",
        }
    }
}

/// 探测结果：哪个套件、用哪个可执行文件打开这类文档。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeProbe {
    /// `microsoftoffice` / `wps` / `none`
    pub suite: String,
    /// 套件的展示名（`none` 时为空）。
    pub display_name: String,
    /// 实际可执行文件的绝对路径（`none` 时为空）。
    pub executable: String,
}

/// 文档大类的内部枚举。和 `CraftApp` 保持同一套 kind 词汇。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeKind {
    Word,
    Grid,
    Deck,
}

impl OfficeKind {
    pub fn from_kind(kind: &str) -> Option<Self> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "word" | "docx" | "doc" | "rtf" | "odt" => Some(Self::Word),
            "grid" | "xlsx" | "xls" | "csv" | "ods" => Some(Self::Grid),
            "deck" | "pptx" | "ppt" | "odp" => Some(Self::Deck),
            _ => None,
        }
    }

    /// Microsoft Office 的可执行名。
    fn ms_exe(self) -> &'static str {
        match self {
            Self::Word => "WINWORD.EXE",
            Self::Grid => "EXCEL.EXE",
            Self::Deck => "POWERPNT.EXE",
        }
    }

    /// WPS 的可执行名：文字 / 表格 / 演示。
    fn wps_exe(self) -> &'static str {
        match self {
            Self::Word => "wps.exe",
            Self::Grid => "et.exe",
            Self::Deck => "wpp.exe",
        }
    }
}

/// 只读查询一个注册表字符串值。`None` 表示键或值不存在、或读取失败。
#[cfg(windows)]
fn read_reg_string(root: isize, subkey: &str, value: &str) -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, KEY_READ, REG_SZ,
    };

    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let sub_w = wide(subkey);
    let val_w = wide(value);
    let mut key: HKEY = std::ptr::null_mut();

    // SAFETY: 传进来的指针都由本函数持有、生命周期覆盖调用；缓冲区按 REG_SZ 的
    // 「宽字符 + 结尾 NUL」语义分配，长度通过 `size` 回传给 RegQueryValueExW。
    unsafe {
        if RegOpenKeyExW(root as HKEY, sub_w.as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS {
            return None;
        }
        let mut size: u32 = 0;
        let mut ty: u32 = 0;
        let rc = RegQueryValueExW(
            key,
            val_w.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            std::ptr::null_mut(),
            &mut size,
        );
        if rc != ERROR_SUCCESS || ty != REG_SZ || size == 0 || size > 32 * 1024 {
            RegCloseKey(key);
            return None;
        }
        let mut buf = vec![0u16; (size as usize) / 2 + 1];
        let rc = RegQueryValueExW(
            key,
            val_w.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            buf.as_mut_ptr() as *mut u8,
            &mut size,
        );
        RegCloseKey(key);
        if rc != ERROR_SUCCESS {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]);
        let s = s.trim().trim_matches('"').to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

#[cfg(windows)]
const HKCU: isize = 0x8000_0001u32 as isize;
#[cfg(windows)]
const HKLM: isize = 0x8000_0002u32 as isize;

/// 在注册表的 `App Paths` 下找某个 exe 的绝对路径。
#[cfg(windows)]
fn lookup_app_path(exe: &str) -> Option<PathBuf> {
    let sub = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{}", exe);
    let sub32 = format!(
        "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{}",
        exe
    );
    for (root, key) in [(HKCU, &sub), (HKLM, &sub), (HKLM, &sub32)] {
        if let Some(p) = read_reg_string(root, key, "") {
            let path = PathBuf::from(p);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn lookup_app_path(_exe: &str) -> Option<PathBuf> {
    None
}

/// 已知安装目录兜底：注册表没写时（绿色版 WPS 常见）逐目录找一次。
fn lookup_common_dir(exe: &str) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for env in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(v) = std::env::var(env) {
            if !v.trim().is_empty() {
                roots.push(PathBuf::from(v));
            }
        }
    }
    for root in roots {
        for rel in [
            format!("Microsoft Office\\root\\Office16\\{}", exe),
            format!("Microsoft Office\\Office16\\{}", exe),
            format!("Microsoft Office\\Office15\\{}", exe),
            format!("Kingsoft\\WPS Office\\office6\\{}", exe),
            format!("WPS Office\\office6\\{}", exe),
        ] {
            let p = root.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// 探测某类文档由哪个套件负责。找不到任何套件时返回 `suite: "none"`。
pub fn probe_office(kind: OfficeKind) -> OfficeProbe {
    // Office 优先：它读写的 OOXML 最权威，能正确处理修订与复杂排版。
    if let Some(p) = lookup_app_path(kind.ms_exe()).or_else(|| lookup_common_dir(kind.ms_exe())) {
        return OfficeProbe {
            suite: "microsoftoffice".to_string(),
            display_name: OfficeSuite::MicrosoftOffice.display_name().to_string(),
            executable: p.to_string_lossy().to_string(),
        };
    }
    if let Some(p) = lookup_app_path(kind.wps_exe()).or_else(|| lookup_common_dir(kind.wps_exe())) {
        return OfficeProbe {
            suite: "wps".to_string(),
            display_name: OfficeSuite::Wps.display_name().to_string(),
            executable: p.to_string_lossy().to_string(),
        };
    }
    OfficeProbe {
        suite: "none".to_string(),
        display_name: String::new(),
        executable: String::new(),
    }
}

/// 用本机办公套件打开文档。文档不存在或套件缺失都返回明确错误。
pub fn open_with_office(path: &str, kind: &str) -> Result<OfficeProbe, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("文件不存在：{}", path));
    }
    let kind = OfficeKind::from_kind(kind)
        .ok_or_else(|| format!("不是 Office 文档类型：{}", kind))?;
    let probe = probe_office(kind);
    if probe.suite == "none" {
        return Err(format!(
            "未检测到 Microsoft Office 或 WPS，无法用本机办公套件打开（{}）",
            kind_label(kind)
        ));
    }

    let mut cmd = Command::new(&probe.executable);
    cmd.arg(p);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        // Office 自己是 GUI 进程，CREATE_NO_WINDOW 只影响我们这条启动命令的控制台窗口。
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // 不 wait：Office 启动动辄数秒，等待会把命令拖成「无响应」。
    let child = cmd
        .spawn()
        .map_err(|e| format!("启动 {} 失败：{}", probe.display_name, e))?;
    info!(
        path = path,
        suite = %probe.suite,
        pid = child.id(),
        "已用本机办公套件打开文档"
    );
    Ok(probe)
}

fn kind_label(kind: OfficeKind) -> &'static str {
    match kind {
        OfficeKind::Word => "文字文档",
        OfficeKind::Grid => "表格",
        OfficeKind::Deck => "演示文稿",
    }
}

/// 探测三类文档各自的可用套件，供前端在打开前决定走哪条路。
#[tauri::command]
pub fn probe_office_suites() -> serde_json::Value {
    let word = probe_office(OfficeKind::Word);
    let grid = probe_office(OfficeKind::Grid);
    let deck = probe_office(OfficeKind::Deck);
    serde_json::json!({ "word": word, "grid": grid, "deck": deck })
}

/// 用本机 Office/WPS 打开文档。
#[tauri::command]
pub async fn open_in_office(path: String, kind: String) -> Result<OfficeProbe, String> {
    tauri::async_runtime::spawn_blocking(move || open_with_office(&path, &kind))
        .await
        .map_err(|e| format!("启动办公套件任务失败：{}", e))?
}

/// 不做探测，直接交给 shell 的默认关联程序打开。
///
/// 给「用户明确要求用系统默认程序打开」的场景用：它不假设装了 Office，
/// 也不挑剔套件 —— 行为完全等同于在资源管理器里双击。
#[tauri::command]
pub fn open_with_system_default(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("文件不存在：{}", path));
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", ""]).arg(p);
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn().map_err(|e| format!("调用系统默认程序失败：{}", e))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("调用系统默认程序失败：{}", e))?;
        Ok(())
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        match Command::new("xdg-open").arg(p).spawn() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("调用系统默认程序失败：{}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_document_kinds() {
        assert_eq!(OfficeKind::from_kind("docx"), Some(OfficeKind::Word));
        assert_eq!(OfficeKind::from_kind("DOC"), Some(OfficeKind::Word));
        assert_eq!(OfficeKind::from_kind("xlsx"), Some(OfficeKind::Grid));
        assert_eq!(OfficeKind::from_kind("pptx"), Some(OfficeKind::Deck));
        assert_eq!(OfficeKind::from_kind("pdf"), None);
    }

    #[test]
    fn probe_never_panics_on_missing_suite() {
        // 探测必须在「什么都没装」的机器上也能给出结构化结果，而不是报错或 panic。
        let p = probe_office(OfficeKind::Word);
        assert!(p.suite == "none" || !p.executable.is_empty());
    }

    #[test]
    fn opening_a_missing_file_reports_clearly() {
        let err = open_with_office("Z:\\definitely\\not\\here.docx", "docx").unwrap_err();
        assert!(err.contains("文件不存在"), "实际: {err}");
    }
}
