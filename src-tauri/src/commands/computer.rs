//! 电脑操控：截屏、窗口枚举、鼠标键盘、剪贴板。
//!
//! 为什么单独成模块
//!
//! 这些能力不是「文件工具的一种」，它们动的是用户正在用的那台机器——鼠标会真的移走、
//! 键盘会真的输入。所以它们不挂在文件系统工具下，而是自成一组，并统一受 `Tier::Gui` 管制。
//!
//! **默认关闭**。开启后每一次 GUI 动作都要经过确认，这一点由调用方（工具分发层）保证；
//! 本模块只负责「动作本身能不能安全执行」，不做授权判断——那里已经有一层，重复判断只会让
//! 两处的规则慢慢分叉。
//!
//! 为什么不用 enigo
//!
//! 项目已经直接依赖 `windows-sys`（keyring 用），截屏与输入注入所需的 API 都在
//! `Win32_UI_WindowsAndMessaging` / `Input_KeyboardAndMouse` / `Graphics_Gdi` 里。
//! 再引一个跨平台输入库，换来的是三个平台的适配与一份新的构建风险（Windows 构建机没 NASM，
//! 之前已经为加密库付过一次代价），而实际只需要一份 Windows 实现。真要做跨平台时再换，
//! 接口已经按平台无关的形状设计好了。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// 一张截图的结果。`path` 是落盘位置，`width/height` 是逻辑像素（模型看到的那张图的坐标），
/// `scale` 是物理/逻辑比例——模型给的是逻辑坐标，注入时也要用逻辑坐标，两者一致，
/// 但把 `scale` 一并回给前端，审批卡里能显示真实分辨率。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CaptureResult {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub origin_x: i32,
    pub origin_y: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WindowInfo {
    pub handle: i64,
    pub title: String,
    pub class_name: String,
    pub process_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub visible: bool,
    pub minimized: bool,
}

/// 屏幕逻辑尺寸（未乘 DPI 缩放，与 `SetCursorPos` 的坐标系一致）。
///
/// 这里返回的是**逻辑**像素（`GetSystemMetrics` 在 DPI 感知进程里就是逻辑值），
/// 而 `capture_screen` 出的图是**物理**像素。两者由 `dpi_scale()` 换算。
pub fn screen_size() -> Result<(i32, i32), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        // SAFETY: GetSystemMetrics 无参数副作用，返回整数值。
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if w <= 0 || h <= 0 {
            return Err("无法获取屏幕尺寸".to_string());
        }
        return Ok((w, h));
    }
    #[cfg(not(windows))]
    {
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

/// 物理像素 / 逻辑像素的比例（125% 缩放返回 1.25）。
///
/// 用 `GetDeviceCaps(LOGPIXELSX)` 而不是读窗口 DPI：这里没有具体窗口，
/// 要的是整块主屏的缩放比例。失败时返回 1.0——宁可当 100% 处理，
/// 也不要在拿不到信息时把坐标算歪。
pub fn dpi_scale() -> f32 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, LOGPIXELSX, ReleaseDC};
        // SAFETY: GetDC/ReleaseDC 成对调用；只读取设备能力，不写任何状态。
        unsafe {
            let dc = GetDC(std::ptr::null_mut() as HWND);
            if dc.is_null() {
                return 1.0;
            }
            let dpi = GetDeviceCaps(dc, LOGPIXELSX as i32);
            ReleaseDC(std::ptr::null_mut() as HWND, dc);
            if dpi <= 0 {
                1.0
            } else {
                dpi as f32 / 96.0
            }
        }
    }
    #[cfg(not(windows))]
    {
        1.0
    }
}

/// 声明进程 DPI 感知。
///
/// 不声明时 Windows 会把整个进程的坐标「虚拟化」成 96 DPI 视角：
/// `GetSystemMetrics`、`SetCursorPos`、窗口矩形全是缩放后的值，而 GDI 的
/// `BitBlt` 抓到的却是真实像素——两套坐标混在一起，高缩放下必然错位。
/// 应当在 `main` 创建任何窗口之前调用一次。
pub fn init_dpi_awareness() {
    #[cfg(windows)]
    {
        // 优先用 PER_MONITOR_AWARE_V2（Windows 10 1703+），失败再退旧接口。
        #[link(name = "user32")]
        extern "system" {
            fn SetProcessDpiAwarenessContext(value: isize) -> i32;
        }
        // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = (DPI_AWARENESS_CONTEXT)-4
        const PER_MONITOR_AWARE_V2: isize = -4;
        // SAFETY: 只是设置本进程的 DPI 策略，无参数校验风险。
        let ok = unsafe { SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2) };
        if ok != 0 {
            return;
        }
        use windows_sys::Win32::UI::HiDpi::{
            SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE,
        };
        // SAFETY: 同上；返回 E_ACCESSDENIED 说明已经设过，忽略即可。
        unsafe {
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
        }
    }
}

/// 鼠标当前坐标。
pub fn cursor_position() -> Result<(i32, i32), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: 传入的是本栈上分配的可写 POINT。
        let ok = unsafe { GetCursorPos(&mut p) };
        if ok == 0 {
            return Err("读取鼠标位置失败".to_string());
        }
        return Ok((p.x, p.y));
    }
    #[cfg(not(windows))]
    {
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

/// 截取整屏或指定区域，落成 PNG 并返回元信息。
///
/// `region` 为 `(x, y, w, h)`；None 表示整屏。坐标是逻辑像素。
pub fn capture_screen(region: Option<(i32, i32, i32, i32)>, out_path: &std::path::Path) -> Result<CaptureResult, String> {
    #[cfg(windows)]
    {
        capture_screen_win(region, out_path)
    }
    #[cfg(not(windows))]
    {
        let _ = (region, out_path);
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

#[cfg(windows)]
fn capture_screen_win(
    region: Option<(i32, i32, i32, i32)>,
    out_path: &std::path::Path,
) -> Result<CaptureResult, String> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        SRCCOPY,
    };

    let (sx, sy, sw, sh) = match region {
        Some((x, y, w, h)) if w > 0 && h > 0 => (x, y, w, h),
        Some(_) => return Err("截图区域无效".to_string()),
        None => {
            let (w, h) = screen_size()?;
            (0, 0, w, h)
        }
    };

    // SAFETY: 这一段全是 Win32 GDI 调用，所有句柄在函数末尾按创建逆序释放；
    // 任何一步失败都走 early return 且已创建的句柄由 RAII guard 回收（见下方结构）。
    unsafe {
        // 传入的 region 是**逻辑像素**（与模型从截图上读到的坐标同一套），
        // 而 BitBlt 取的是物理像素。高缩放下两者不相等，必须换算，
        // 否则 150% 缩放的机器上截出来的区域会整体偏移、只抓到左上角一块。
        let scale = dpi_scale();
        let (px, py, pw, ph) = if (scale - 1.0).abs() < 0.01 {
            (sx, sy, sw, sh)
        } else {
            (
                (sx as f32 * scale).round() as i32,
                (sy as f32 * scale).round() as i32,
                (sw as f32 * scale).round() as i32,
                (sh as f32 * scale).round() as i32,
            )
        };

        let screen_dc = GetDC(std::ptr::null_mut() as HWND);
        if screen_dc.is_null() {
            return Err("获取屏幕 DC 失败".to_string());
        }
        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_null() {
            ReleaseDC(std::ptr::null_mut() as HWND, screen_dc);
            return Err("创建内存 DC 失败".to_string());
        }
        let bitmap = CreateCompatibleBitmap(screen_dc, pw, ph);
        if bitmap.is_null() {
            DeleteDC(mem_dc);
            ReleaseDC(std::ptr::null_mut() as HWND, screen_dc);
            return Err("创建位图失败".to_string());
        }
        let old = SelectObject(mem_dc, bitmap);
        let copied = BitBlt(mem_dc, 0, 0, pw, ph, screen_dc, px, py, SRCCOPY | CAPTUREBLT);

        let mut pixels: Vec<u8> = Vec::new();
        let mut header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: pw,
            // 负高度 = 自上而下，省掉一次翻行。
            biHeight: -ph,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        if copied != 0 {
            pixels.resize((pw as usize) * (ph as usize) * 4, 0);
            let scan_lines = GetDIBits(
                mem_dc,
                bitmap,
                0,
                ph as u32,
                pixels.as_mut_ptr() as *mut _,
                &mut header as *mut BITMAPINFOHEADER as *mut _,
                DIB_RGB_COLORS,
            );
            if scan_lines == 0 {
                pixels.clear();
            }
        }

        SelectObject(mem_dc, old);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
        ReleaseDC(std::ptr::null_mut() as HWND, screen_dc);

        if pixels.is_empty() {
            return Err("截屏失败（BitBlt/GetDIBits 未返回像素）".to_string());
        }

        // GDI 给的是 BGRA；image crate 要 RGBA。
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }

        let img = image::RgbaImage::from_raw(pw as u32, ph as u32, pixels)
            .ok_or_else(|| "构造图像缓冲失败".to_string())?;
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建截图目录失败: {e}"))?;
        }
        img.save(out_path)
            .map_err(|e| format!("保存截图失败: {e}"))?;

        return Ok(CaptureResult {
            path: out_path.to_string_lossy().to_string(),
            // 图片本身是物理像素；模型读到的坐标也是这张图上的像素，
            // 往 `mouse` 传时会按同一个 scale 换算回逻辑坐标，所以两边自洽。
            width: pw as u32,
            height: ph as u32,
            scale,
            origin_x: sx,
            origin_y: sy,
        });
    }
}

/// 把一张截图叠上编号网格，便于模型用「第 7 格」这种稳定方式表达位置。
///
/// 网格用低对比度细线 + 每格一个编号；编号画在格子左上角，字号随格宽缩放。
/// 这是给模型看的位置索引，不是给人看的装饰，所以不加颜色、不加阴影。
pub fn draw_grid(src: &std::path::Path, dst: &std::path::Path, cols: u32, rows: u32) -> Result<(), String> {
    let cols = cols.clamp(1, 40);
    let rows = rows.clamp(1, 40);
    let mut img = image::open(src)
        .map_err(|e| format!("读取截图失败: {e}"))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let cell_w = w / cols;
    let cell_h = h / rows;
    if cell_w == 0 || cell_h == 0 {
        return Err("网格过密".to_string());
    }

    let line = image::Rgba([0, 0, 0, 90]);
    let font_scale = (cell_w.min(cell_h) / 18).max(1) as f32;

    // 横竖线
    for c in 0..=cols {
        let x = (c * cell_w).min(w - 1);
        for y in 0..h {
            img.put_pixel(x, y, blend(img.get_pixel(x, y), line));
        }
    }
    for r in 0..=rows {
        let y = (r * cell_h).min(h - 1);
        for x in 0..w {
            img.put_pixel(x, y, blend(img.get_pixel(x, y), line));
        }
    }

    // 编号：用 5x7 的点阵数字，避免为此引一个字体光栅化依赖。
    for r in 0..rows {
        for c in 0..cols {
            let index = r * cols + c + 1;
            let ox = c * cell_w + 3;
            let oy = r * cell_h + 3;
            draw_number(&mut img, ox, oy, index, font_scale);
        }
    }

    img.save(dst).map_err(|e| format!("保存网格截图失败: {e}"))?;
    Ok(())
}

fn blend(base: &image::Rgba<u8>, over: image::Rgba<u8>) -> image::Rgba<u8> {
    let a = over[3] as u32;
    let inv = 255 - a;
    let mix = |b: u8, o: u8| ((o as u32 * a + b as u32 * inv) / 255) as u8;
    image::Rgba([mix(base[0], over[0]), mix(base[1], over[1]), mix(base[2], over[2]), 255])
}

/// 5x7 点阵数字（0-9）。每位是一个 u8 行掩码的低 5 位。
const DIGITS: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
];

fn draw_number(img: &mut image::RgbaImage, ox: u32, oy: u32, value: u32, scale: f32) {
    let text = value.to_string();
    let s = scale.max(1.0) as u32;
    let mut cx = ox;
    let ink = image::Rgba([0, 0, 0, 220]);
    for ch in text.chars() {
        let Some(d) = ch.to_digit(10) else { continue };
        let glyph = DIGITS[d as usize];
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..5u32 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        let x = cx + col * s + dx;
                        let y = oy + row as u32 * s + dy;
                        if x < img.width() && y < img.height() {
                            let base = *img.get_pixel(x, y);
                            img.put_pixel(x, y, blend(&base, ink));
                        }
                    }
                }
            }
        }
        cx += 6 * s;
    }
}

// ───────────────────────────── 窗口

pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    #[cfg(windows)]
    {
        list_windows_win()
    }
    #[cfg(not(windows))]
    {
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

#[cfg(windows)]
fn list_windows_win() -> Result<Vec<WindowInfo>, String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: isize) -> i32 {
        let out = &mut *(lparam as *mut Vec<WindowInfo>);
        let len = GetWindowTextLengthW(hwnd);
        // 无标题的通常是隐藏的辅助窗口，跳过——列出来只会淹没有用的项。
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            let title = OsString::from_wide(&buf[..n.max(0) as usize])
                .to_string_lossy()
                .to_string();

            let mut class_buf = vec![0u16; 256];
            let cn = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256);
            let class_name = OsString::from_wide(&class_buf[..cn.max(0) as usize])
                .to_string_lossy()
                .to_string();

            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            let _ = GetWindowRect(hwnd, &mut rect);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);

            out.push(WindowInfo {
                handle: hwnd as i64,
                title,
                class_name,
                process_id: pid,
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
                visible: IsWindowVisible(hwnd) != 0,
                minimized: IsIconic(hwnd) != 0,
            });
        }
        1
    }

    let mut list: Vec<WindowInfo> = Vec::new();
    // SAFETY: 回调把 `list` 的可写指针当 lparam 传回，生命周期覆盖整个 EnumWindows 调用。
    let ok = unsafe { EnumWindows(Some(enum_proc), &mut list as *mut _ as isize) };
    if ok == 0 {
        return Err("枚举窗口失败".to_string());
    }
    Ok(list)
}

/// 按标题子串查找窗口（不区分大小写），返回第一个可见项。
pub fn find_window(query: &str) -> Result<WindowInfo, String> {
    let q = query.trim().to_lowercase();
    let windows = list_windows()?;
    windows
        .into_iter()
        .find(|w| w.visible && !w.minimized && w.title.to_lowercase().contains(&q))
        .or_else(|| {
            list_windows()
                .ok()?
                .into_iter()
                .find(|w| w.title.to_lowercase().contains(&q))
        })
        .ok_or_else(|| format!("未找到窗口: {query}"))
}

pub fn focus_window(handle: i64) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let hwnd = handle as HWND;
        // SAFETY: handle 来自 list_windows；非法句柄时这些调用只是返回 0/false，不会崩溃。
        unsafe {
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }
            if SetForegroundWindow(hwnd) == 0 {
                return Err("激活窗口失败（可能被系统前台锁限制）".to_string());
            }
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = handle;
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

// ───────────────────────────── 输入

#[derive(Deserialize, Debug, Default)]
pub struct MouseAction {
    /// move | click | double_click | right_click | drag | scroll
    pub action: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// drag 的终点
    #[serde(default)]
    pub to_x: i32,
    #[serde(default)]
    pub to_y: i32,
    /// scroll 的滚轮格数（正数向上）
    #[serde(default)]
    pub amount: i32,
}

pub fn mouse(action: &MouseAction) -> Result<Value, String> {
    #[cfg(windows)]
    {
        // 工具层约定：模型给的是截图上的像素（物理像素），这里换算成
        // SetCursorPos 要的逻辑坐标；`ScreenShotTool` 与设置页的「试一下」
        // 传的也是同一套截图坐标。100% 缩放下 scale=1，行为与以前完全一致。
        let scale = dpi_scale();
        let to_logical = |v: i32| -> i32 {
            if (scale - 1.0).abs() < 0.01 {
                v
            } else {
                (v as f32 / scale).round() as i32
            }
        };
        let (x, y) = match action.action.as_str() {
            "move" | "click" | "double_click" | "right_click" | "drag" => {
                (to_logical(action.x), to_logical(action.y))
            }
            "scroll" => cursor_position().unwrap_or((0, 0)),
            other => return Err(format!("未知鼠标动作: {other}")),
        };
        move_to(x, y)?;
        match action.action.as_str() {
            "move" => {}
            "click" => click_at(x, y, Button::Left, 1)?,
            "double_click" => click_at(x, y, Button::Left, 2)?,
            "right_click" => click_at(x, y, Button::Right, 1)?,
            "drag" => {
                button(Button::Left, true)?;
                move_to(to_logical(action.to_x), to_logical(action.to_y))?;
                button(Button::Left, false)?;
            }
            "scroll" => scroll(action.amount)?,
            _ => {}
        }
        return Ok(json!({
            "ok": true,
            "action": action.action,
            "x": x, "y": y,
            "logicalX": x, "logicalY": y,
            "scale": scale,
            "toX": action.to_x, "toY": action.to_y,
        }));
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

#[derive(Clone, Copy)]
enum Button {
    Left,
    Right,
}

#[cfg(windows)]
fn move_to(x: i32, y: i32) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos;
    // SAFETY: 纯输入注入，参数越界时系统会钳制。
    let ok = unsafe { SetCursorPos(x, y) };
    if ok == 0 {
        return Err(format!("移动鼠标到 ({x}, {y}) 失败"));
    }
    // 给目标窗口一点时间处理 hover，连续操作时不会点错位置。
    std::thread::sleep(std::time::Duration::from_millis(60));
    Ok(())
}

#[cfg(windows)]
fn click_at(x: i32, y: i32, btn: Button, times: u32) -> Result<(), String> {
    for _ in 0..times {
        button(btn, true)?;
        std::thread::sleep(std::time::Duration::from_millis(30));
        button(btn, false)?;
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    let _ = (x, y);
    Ok(())
}

#[cfg(windows)]
fn button(btn: Button, down: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT,
    };
    let flags = match (btn, down) {
        (Button::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (Button::Left, false) => MOUSEEVENTF_LEFTUP,
        (Button::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (Button::Right, false) => MOUSEEVENTF_RIGHTUP,
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: 单个 INPUT 结构，size 与类型匹配。
    let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("发送鼠标事件失败".to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn scroll(amount: i32) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: (amount * 120) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: 同 button。
    let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("发送滚轮事件失败".to_string());
    }
    Ok(())
}

#[derive(Deserialize, Debug, Default)]
pub struct KeyboardAction {
    /// type | key | hotkey
    pub action: String,
    /// type 的文本
    #[serde(default)]
    pub text: String,
    /// key / hotkey 的按键名，热键形如 ["ctrl", "s"]
    #[serde(default)]
    pub keys: Vec<String>,
}

pub fn keyboard(action: &KeyboardAction) -> Result<Value, String> {
    #[cfg(windows)]
    {
        match action.action.as_str() {
            "type" => type_text(&action.text)?,
            "key" => {
                let k = action
                    .keys
                    .first()
                    .ok_or_else(|| "缺少按键名".to_string())?;
                let vk = vk_for(k)?;
                press_vk(vk, true)?;
                press_vk(vk, false)?;
            }
            "hotkey" => {
                let vks: Result<Vec<u16>, String> = action.keys.iter().map(|k| vk_for(k)).collect();
                let vks = vks?;
                for vk in &vks {
                    press_vk(*vk, true)?;
                }
                for vk in vks.iter().rev() {
                    press_vk(*vk, false)?;
                }
            }
            other => return Err(format!("未知键盘动作: {other}")),
        }
        return Ok(json!({ "ok": true, "action": action.action, "chars": action.text.chars().count() }));
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

#[cfg(windows)]
fn type_text(text: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    };
    for ch in text.chars() {
        let mut buf = [0u16; 2];
        let units = ch.encode_utf16(&mut buf);
        for unit in units.iter() {
            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: *unit,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: *unit,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            // SAFETY: 两个 INPUT 都在栈上，size 与类型匹配。
            let sent = unsafe {
                SendInput(1, &down, std::mem::size_of::<INPUT>() as i32);
                SendInput(1, &up, std::mem::size_of::<INPUT>() as i32)
            };
            let _ = sent;
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn press_vk(vk: u16, down: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };
    let flags = if down { 0 } else { KEYEVENTF_KEYUP };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    // SAFETY: 同 type_text。
    let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("发送键盘事件失败".to_string());
    }
    Ok(())
}

/// 按键名 → 虚拟键码。只覆盖常用键；名字按「能被模型自然写出来」来定。
#[cfg(windows)]
fn vk_for(name: &str) -> Result<u16, String> {
    let n = name.trim().to_lowercase();
    let code = match n.as_str() {
        "ctrl" | "control" => 0x11,
        "shift" => 0x10,
        "alt" => 0x12,
        "win" | "meta" | "super" => 0x5B,
        "enter" | "return" => 0x0D,
        "esc" | "escape" => 0x1B,
        "tab" => 0x09,
        "space" => 0x20,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        "capslock" => 0x14,
        _ => {
            // 单字符：字母数字走 VkKeyScan 太绕，直接映射。
            let mut chars = n.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                match c {
                    'a'..='z' => c.to_ascii_uppercase() as u16,
                    '0'..='9' => c as u16,
                    _ => return Err(format!("不支持的按键: {name}")),
                }
            } else if let Some(rest) = n.strip_prefix('f') {
                if let Ok(num) = rest.parse::<u16>() {
                    if (1..=24).contains(&num) {
                        0x70 + (num - 1)
                    } else {
                        return Err(format!("不支持的按键: {name}"));
                    }
                } else {
                    return Err(format!("不支持的按键: {name}"));
                }
            } else {
                return Err(format!("不支持的按键: {name}"));
            }
        }
    };
    Ok(code)
}

// ───────────────────────────── 剪贴板

pub fn read_clipboard_text() -> Result<String, String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HGLOBAL;
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, OpenClipboard,
        };
        use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
        const CF_UNICODETEXT: u32 = 13;

        // SAFETY: 剪贴板句柄必须成对打开/关闭；所有指针都在同一作用域内使用完即解锁。
        unsafe {
            if OpenClipboard(std::ptr::null_mut() as _) == 0 {
                return Err("打开剪贴板失败".to_string());
            }
            let handle = GetClipboardData(CF_UNICODETEXT);
            if handle.is_null() {
                CloseClipboard();
                return Ok(String::new());
            }
            let ptr = GlobalLock(handle as HGLOBAL);
            if ptr.is_null() {
                CloseClipboard();
                return Err("锁定剪贴板数据失败".to_string());
            }
            let mut len = 0usize;
            let wide = ptr as *const u16;
            while *wide.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(wide, len);
            let text = String::from_utf16_lossy(slice);
            GlobalUnlock(handle as HGLOBAL);
            CloseClipboard();
            return Ok(text);
        }
    }
    #[cfg(not(windows))]
    {
        Err("电脑操控目前只支持 Windows".to_string())
    }
}

pub fn write_clipboard_text(text: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HGLOBAL;
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Memory::{
            GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
        };
        const CF_UNICODETEXT: u32 = 13;

        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        // SAFETY: 分配→写入→交给系统；SetClipboardData 成功后所有权归系统，不能自行释放。
        unsafe {
            if OpenClipboard(std::ptr::null_mut() as _) == 0 {
                return Err("打开剪贴板失败".to_string());
            }
            EmptyClipboard();
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if handle.is_null() {
                CloseClipboard();
                return Err("分配剪贴板内存失败".to_string());
            }
            let ptr = GlobalLock(handle);
            if ptr.is_null() {
                CloseClipboard();
                return Err("锁定剪贴板内存失败".to_string());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
            GlobalUnlock(handle);
            if SetClipboardData(CF_UNICODETEXT, handle as HGLOBAL).is_null() {
                CloseClipboard();
                return Err("写入剪贴板失败".to_string());
            }
            CloseClipboard();
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        Err("电脑操控目前只支持 Windows".to_string())
    }
}
