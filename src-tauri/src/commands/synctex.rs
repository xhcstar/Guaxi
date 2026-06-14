//! SyncTeX 双向定位（仅 LaTeX / tectonic）。
//!
//! tectonic 编译预览时加 `--synctex`，在输出目录生成 `<stem>.synctex.gz`。
//! 本模块把它解压、解析成"盒子"列表（每个盒子记录所属源文件行号 + 在页面上的矩形），
//! 然后提供：
//!   - `synctex_forward`：源码行 -> PDF 位置（page + 矩形），用于编辑器光标驱动预览滚动定位。
//!   - `synctex_inverse`：PDF 点击点(page,x,y) -> 源码行，用于在预览里点一下跳回编辑器对应行。
//!
//! 坐标统一为"左上原点、单位 pt"，与 pdf_reader / 前端一致。
//!
//! 设计取舍：
//! - 命令只收 `pdf_path`，synctex 路径由它派生（同目录同名换扩展名），保持无状态。
//! - 解析结果按 (路径, mtime) 缓存，避免每次光标移动都重新解压解析。
//! - SyncTeX 单位换算：值(sp) * unit * mag/1000 / 65536 得到 TeX pt，再乘 72/72.27 → PDF pt。
//!   偏移/放大在 tectonic 输出里通常是 0/1000，这里仍按字段读取以防万一。

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use serde::Serialize;
use tracing::info;

/// 一个带几何信息的 SyncTeX 记录（盒子或锚点）。坐标为 PDF pt、左上原点。
#[derive(Debug, Clone)]
struct SyncBox {
    page: u32,
    tag: u32,
    line: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// 一份解析好的 synctex 数据。
#[derive(Debug, Clone, Default)]
struct SyncData {
    boxes: Vec<SyncBox>,
    /// tag -> 源文件路径（来自 Input: 行）。
    inputs: HashMap<u32, String>,
}

/// 解析缓存：synctex 路径 -> (mtime, 数据)。
static CACHE: Lazy<Mutex<HashMap<PathBuf, (SystemTime, SyncData)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize)]
pub struct ForwardResult {
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Serialize)]
pub struct InverseResult {
    pub line: u32,
    pub tag: u32,
    /// 命中的源文件路径（synctex Input 记录里的原始路径）。
    pub file: String,
}

/// 由 pdf 路径派生 synctex 路径：优先 `<stem>.synctex.gz`，其次未压缩 `<stem>.synctex`。
fn synctex_path_for(pdf_path: &str) -> Option<PathBuf> {
    let pdf = PathBuf::from(pdf_path);
    let gz = pdf.with_extension("synctex.gz");
    if gz.is_file() {
        return Some(gz);
    }
    let plain = pdf.with_extension("synctex");
    if plain.is_file() {
        return Some(plain);
    }
    None
}

/// 读取 synctex 文件内容（自动判断是否 gzip）。
fn read_synctex(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("打开 synctex 失败: {e}"))?;
    let is_gz = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("gz"))
        .unwrap_or(false);
    let mut out = String::new();
    if is_gz {
        let mut dec = GzDecoder::new(file);
        dec.read_to_string(&mut out)
            .map_err(|e| format!("解压 synctex 失败: {e}"))?;
    } else {
        file.read_to_string(&mut out)
            .map_err(|e| format!("读取 synctex 失败: {e}"))?;
    }
    Ok(out)
}

/// 解析 synctex 文本为 SyncData。
fn parse_synctex(text: &str) -> SyncData {
    let mut data = SyncData::default();

    // 头部参数（带默认值）。
    let mut unit: f64 = 1.0;
    let mut mag: f64 = 1000.0;
    let mut x_offset: f64 = 0.0;
    let mut y_offset: f64 = 0.0;

    let mut in_content = false;
    let mut cur_page: u32 = 0;

    for raw in text.lines() {
        let line = raw;
        if !in_content {
            if let Some(rest) = line.strip_prefix("Input:") {
                // 形如 Input:tag:/path/to/file.tex
                if let Some((tag_s, fname)) = rest.split_once(':') {
                    if let Ok(tag) = tag_s.trim().parse::<u32>() {
                        data.inputs.insert(tag, fname.trim().to_string());
                    }
                }
                continue;
            }
            if let Some(v) = line.strip_prefix("Unit:") {
                if let Ok(n) = v.trim().parse::<f64>() {
                    unit = n;
                }
                continue;
            }
            if let Some(v) = line.strip_prefix("Magnification:") {
                if let Ok(n) = v.trim().parse::<f64>() {
                    mag = n;
                }
                continue;
            }
            if let Some(v) = line.strip_prefix("X Offset:") {
                if let Ok(n) = v.trim().parse::<f64>() {
                    x_offset = n;
                }
                continue;
            }
            if let Some(v) = line.strip_prefix("Y Offset:") {
                if let Ok(n) = v.trim().parse::<f64>() {
                    y_offset = n;
                }
                continue;
            }
            if line.starts_with("Content:") {
                in_content = true;
            }
            continue;
        }

        // ── Content 区 ──
        // Input 记录也可能出现在内容区。
        if let Some(rest) = line.strip_prefix("Input:") {
            if let Some((tag_s, fname)) = rest.split_once(':') {
                if let Ok(tag) = tag_s.trim().parse::<u32>() {
                    data.inputs.insert(tag, fname.trim().to_string());
                }
            }
            continue;
        }

        let mut chars = line.char_indices();
        let Some((_, first)) = chars.next() else {
            continue;
        };

        match first {
            '{' | '<' => {
                // 开始一页（{ 是 sheet，< 是新版 form/page）。页码紧跟其后。
                if let Ok(p) = line[1..].trim().parse::<u32>() {
                    cur_page = p;
                }
            }
            '}' | '>' => { /* 结束一页，无需处理 */ }
            // 带完整几何的记录：[ ( h v r  → tag,line:left,top:width,height,depth
            '[' | '(' | 'h' | 'v' | 'r' => {
                if let Some(b) = parse_geom_record(
                    &line[1..],
                    cur_page,
                    unit,
                    mag,
                    x_offset,
                    y_offset,
                    true,
                ) {
                    if b.page > 0 {
                        data.boxes.push(b);
                    }
                }
            }
            // 仅有锚点的记录：g x k $  → tag,line:left,top
            'g' | 'x' | 'k' | '$' => {
                if let Some(b) = parse_geom_record(
                    &line[1..],
                    cur_page,
                    unit,
                    mag,
                    x_offset,
                    y_offset,
                    false,
                ) {
                    if b.page > 0 {
                        data.boxes.push(b);
                    }
                }
            }
            _ => { /* ] ) 等关闭符与未知行忽略 */ }
        }
    }

    data
}

/// 把一条记录体（去掉首字母后）解析为 SyncBox。
/// `with_size=true`：`tag,line:left,top:width,height,depth`
/// `with_size=false`：`tag,line:left,top`
fn parse_geom_record(
    body: &str,
    page: u32,
    unit: f64,
    mag: f64,
    x_offset: f64,
    y_offset: f64,
    with_size: bool,
) -> Option<SyncBox> {
    // body 例：8,1:4736286,52379128:32830699,2228243,535511
    // 先按 ':' 切成 [tag,line] [left,top] [width,height,depth?]
    let mut parts = body.split(':');
    let id_part = parts.next()?;
    let pos_part = parts.next()?;

    let (tag_s, line_s) = id_part.split_once(',')?;
    let tag: u32 = tag_s.trim().parse().ok()?;
    let line: u32 = line_s.trim().parse().ok()?;

    let (h_s, v_s) = pos_part.split_once(',')?;
    let h: i64 = h_s.trim().parse().ok()?;
    let v: i64 = v_s.trim().parse().ok()?;

    let conv = |val: i64, offset: f64| -> f32 {
        (((val as f64) * unit * (mag / 1000.0) + offset) / 65536.0 * (72.0 / 72.27)) as f32
    };

    let x = conv(h, x_offset);
    let v_baseline = conv(v, y_offset);

    let (w, ht, dp) = if with_size {
        if let Some(size_part) = parts.next() {
            let mut s = size_part.split(',');
            let w_raw: i64 = s.next().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            let h_raw: i64 = s.next().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            let d_raw: i64 = s.next().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            (
                conv(w_raw, 0.0).abs(),
                conv(h_raw, 0.0).abs(),
                conv(d_raw, 0.0).abs(),
            )
        } else {
            (0.0, 0.0, 0.0)
        }
    } else {
        (0.0, 0.0, 0.0)
    };

    // 盒子顶边 = 基线 - 高度；纵向跨度 = 高 + 深。
    let y_top = v_baseline - ht;
    let box_h = ht + dp;

    Some(SyncBox {
        page,
        tag,
        line,
        x,
        y: y_top,
        w,
        h: box_h,
    })
}

/// 取得（缓存的）解析数据。
fn load_data(pdf_path: &str) -> Result<(SyncData, PathBuf), String> {
    let sync_path = synctex_path_for(pdf_path)
        .ok_or_else(|| "未找到 synctex 文件（确认编译时已开启 --synctex）".to_string())?;

    let mtime = std::fs::metadata(&sync_path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    {
        let cache = CACHE.lock().unwrap();
        if let Some((cached_mtime, data)) = cache.get(&sync_path) {
            if *cached_mtime == mtime {
                return Ok((data.clone(), sync_path));
            }
        }
    }

    let text = read_synctex(&sync_path)?;
    let data = parse_synctex(&text);
    info!(
        path = %sync_path.display(),
        boxes = data.boxes.len(),
        inputs = data.inputs.len(),
        "[SyncTeX] parsed"
    );

    let mut cache = CACHE.lock().unwrap();
    cache.insert(sync_path.clone(), (mtime, data.clone()));
    Ok((data, sync_path))
}

/// 判断 synctex Input 路径是否对应给定源文件。
/// tectonic 预览编译的是临时文件 `.guaxi_preview_<stem>.tex`，行号与编辑器内容一致，
/// 故按"基名相同"或"同为预览临时文件"宽松匹配；source_path 为空时匹配主输入。
fn input_matches(input_path: &str, source_path: &str) -> bool {
    if source_path.is_empty() {
        return true;
    }
    let in_base = Path::new(input_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let src_base = Path::new(source_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if in_base.is_empty() || src_base.is_empty() {
        return false;
    }
    // 预览临时文件名形如 .guaxi_preview_<stem>.tex；与真实 <stem>.tex 对应。
    let norm = |b: &str| -> String {
        b.trim_start_matches(".guaxi_preview_")
            .to_ascii_lowercase()
    };
    norm(in_base) == norm(src_base) || in_base.eq_ignore_ascii_case(src_base)
}

/// 找出与 source_path 匹配的所有 tag。匹配不到则返回全部 tag（best-effort）。
fn matching_tags(data: &SyncData, source_path: &str) -> Vec<u32> {
    let mut tags: Vec<u32> = data
        .inputs
        .iter()
        .filter(|(_, p)| input_matches(p, source_path))
        .map(|(t, _)| *t)
        .collect();
    if tags.is_empty() {
        tags = data.inputs.keys().copied().collect();
    }
    tags
}

/// 正向：源码行 -> PDF 位置。返回行号最接近、页码最小、最靠上的盒子。
#[tauri::command]
pub async fn synctex_forward(
    pdf_path: String,
    source_path: String,
    line: u32,
    #[allow(unused_variables)] column: Option<u32>,
) -> Result<Option<ForwardResult>, String> {
    let (data, _) = load_data(&pdf_path)?;
    let tags = matching_tags(&data, &source_path);

    let candidates: Vec<&SyncBox> = data
        .boxes
        .iter()
        .filter(|b| tags.contains(&b.tag))
        .collect();
    if candidates.is_empty() {
        return Ok(None);
    }

    // 选择策略：先按 |line 差| 最小，再按 page 最小，再按 y 最小。
    let best = candidates.iter().min_by(|a, b| {
        let da = (a.line as i64 - line as i64).abs();
        let db = (b.line as i64 - line as i64).abs();
        da.cmp(&db)
            .then(a.page.cmp(&b.page))
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });

    Ok(best.map(|b| ForwardResult {
        page: b.page,
        x: b.x,
        y: b.y,
        w: b.w,
        h: b.h.max(8.0),
    }))
}

/// 反向：PDF 点击点 (page, x, y)（pt，左上原点）-> 源码行。
#[tauri::command]
pub async fn synctex_inverse(
    pdf_path: String,
    page: u32,
    x: f32,
    y: f32,
) -> Result<Option<InverseResult>, String> {
    let (data, _) = load_data(&pdf_path)?;

    let on_page: Vec<&SyncBox> = data.boxes.iter().filter(|b| b.page == page).collect();
    if on_page.is_empty() {
        return Ok(None);
    }

    // 1) 优先：包含点击点的盒子里，面积最小者（最精确）。
    let mut contained: Vec<&&SyncBox> = on_page
        .iter()
        .filter(|b| {
            b.w > 0.0
                && b.h > 0.0
                && x >= b.x
                && x <= b.x + b.w
                && y >= b.y
                && y <= b.y + b.h
        })
        .collect();
    if !contained.is_empty() {
        contained.sort_by(|a, b| {
            let area_a = a.w * a.h;
            let area_b = b.w * b.h;
            area_a.partial_cmp(&area_b).unwrap_or(std::cmp::Ordering::Equal)
        });
        let hit = contained[0];
        return Ok(Some(InverseResult {
            line: hit.line,
            tag: hit.tag,
            file: data.inputs.get(&hit.tag).cloned().unwrap_or_default(),
        }));
    }

    // 2) 兜底：到盒子中心（或锚点）距离最近者。
    let nearest = on_page.iter().min_by(|a, b| {
        let dist = |bx: &SyncBox| -> f32 {
            let cx = bx.x + bx.w / 2.0;
            let cy = bx.y + bx.h / 2.0;
            // 纵向权重更大：同一行内横向漂移不应跨行。
            let dx = cx - x;
            let dy = (cy - y) * 3.0;
            dx * dx + dy * dy
        };
        dist(a).partial_cmp(&dist(b)).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(nearest.map(|b| InverseResult {
        line: b.line,
        tag: b.tag,
        file: data.inputs.get(&b.tag).cloned().unwrap_or_default(),
    }))
}
