//! Typst 编译支持：把 .typ 源码编译成 PDF（纯 Rust，无外部依赖）。
//!
//! 设计说明：
//! 1. 与 LaTeX 预览（tectonic）对齐，提供两个命令：
//!    - `build_typ_pdf_preview(source_path, content)`：编译编辑器当前内容到临时 PDF，返回路径，供实时预览。
//!    - `export_typ_pdf(input_path, output_path)`：从磁盘源文件编译并写到目标路径，供导出。
//! 2. 字体来自系统（`include_system_fonts(true)`）；不内嵌、不联网下载包。中文需用户系统装有中文字体。
//! 3. 字体扫描较慢，全局缓存一次（`FONTS`），后续预览复用。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, Local, Timelike};
use once_cell::sync::Lazy;
use serde::Serialize;
use tracing::info;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::layout::{Abs, PagedDocument, Point};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_ide::{jump_from_click, jump_from_cursor, IdeWorld, Jump};
use typst_kit::download::{Downloader, ProgressSink};
use typst_kit::fonts::{FontSearcher, FontSlot};
use typst_kit::package::PackageStorage;

/// 全局标准库（所有编译共享，构建一次）。
static LIBRARY: Lazy<LazyHash<Library>> = Lazy::new(|| LazyHash::new(Library::default()));

/// 全局包存储：解析 `@preview` 等包，首次用到时联网下载到本地缓存。
static PACKAGES: Lazy<PackageStorage> = Lazy::new(|| {
    let downloader = Downloader::new(concat!("guaxi-typst/", env!("CARGO_PKG_VERSION")));
    PackageStorage::new(None, None, downloader)
});

/// 全局字体（扫描系统字体一次后缓存）。
struct GlobalFonts {
    book: LazyHash<FontBook>,
    fonts: Vec<FontSlot>,
}

static FONTS: Lazy<GlobalFonts> = Lazy::new(|| {
    // include_embedded_fonts：内嵌 Typst 默认字体（正文 LibertinusSerif / 数学 NewCMMath / 代码 DejaVuSansMono），
    // 不依赖系统是否装有这些字体，修复数学公式等渲染异常。系统字体仍加载，供中文等使用。
    let fonts = FontSearcher::new()
        .include_system_fonts(true)
        .include_embedded_fonts(true)
        .search();
    info!(count = fonts.fonts.len(), "[Typst] fonts loaded (system + embedded)");
    GlobalFonts {
        book: LazyHash::new(fonts.book),
        fonts: fonts.fonts,
    }
});

/// 一次编译所用的 World：主源码在内存，其余文件（import / 图片）按相对路径从磁盘读。
struct GuaxiTypstWorld {
    root: PathBuf,
    main_id: FileId,
    main_source: Source,
    now: Datetime,
}

impl GuaxiTypstWorld {
    fn new(root: PathBuf, content: String) -> Self {
        // 主文件在 World 内用一个固定的虚拟路径，真实相对依赖以 `root` 为根解析。
        let main_id = FileId::new(None, VirtualPath::new("__guaxi_main__.typ"));
        let main_source = Source::new(main_id, content);
        let now = local_now_datetime();
        Self {
            root,
            main_id,
            main_source,
            now,
        }
    }

    /// 把 FileId 解析为磁盘真实路径。包引用先经 PackageStorage 下载/定位，再在包目录内解析虚拟路径。
    fn id_to_path(&self, id: FileId) -> FileResult<PathBuf> {
        let root = if let Some(spec) = id.package() {
            PACKAGES
                .prepare_package(spec, &mut ProgressSink)
                .map_err(FileError::Package)?
        } else {
            self.root.clone()
        };
        id.vpath()
            .resolve(&root)
            .ok_or_else(|| FileError::NotFound(id.vpath().as_rootless_path().to_path_buf()))
    }
}

impl World for GuaxiTypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            return Ok(self.main_source.clone());
        }
        let path = self.id_to_path(id)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|err| FileError::from_io(err, &path))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.id_to_path(id)?;
        let data = std::fs::read(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Bytes::new(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Some(self.now)
    }
}

impl IdeWorld for GuaxiTypstWorld {
    fn upcast(&self) -> &dyn World {
        self
    }
}

fn local_now_datetime() -> Datetime {
    let now = Local::now();
    Datetime::from_ymd_hms(
        now.year(),
        now.month() as u8,
        now.day() as u8,
        now.hour() as u8,
        now.minute() as u8,
        now.second() as u8,
    )
    .unwrap_or_else(|| Datetime::from_ymd(2026, 1, 1).expect("valid fallback date"))
}

/// 把诊断信息格式化成可读字符串（带行号），供前端预览面板展示。
fn format_diagnostics(world: &GuaxiTypstWorld, diags: &[SourceDiagnostic]) -> String {
    let mut out = String::new();
    for diag in diags {
        let line_info = diag
            .span
            .id()
            .and_then(|id| world.source(id).ok())
            .and_then(|src| {
                let range = src.range(diag.span)?;
                let line = src.lines().byte_to_line(range.start)? + 1;
                let col = src.lines().byte_to_column(range.start)? + 1;
                Some(format!("{}:{}: ", line, col))
            })
            .unwrap_or_default();
        out.push_str(&format!("error: {}{}\n", line_info, diag.message));
        for hint in &diag.hints {
            out.push_str(&format!("  hint: {}\n", hint));
        }
    }
    if out.is_empty() {
        out.push_str("Typst 编译失败（无详细诊断）");
    }
    out
}

/// 把内容编译为 PDF 字节。
fn compile_typ_to_pdf(root: PathBuf, content: String) -> Result<Vec<u8>, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<PagedDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    Ok(pdf)
}

/// 编译并同时缓存 World + 文档，供双向跳转（typst-ide）复用。返回 PDF 字节。
///
/// 为什么缓存：jump_from_cursor / jump_from_click 需要"刚编译出来的那份"World 与 PagedDocument，
/// 而预览编译已经把它们算出来了，丢掉再重算既慢又可能因内容已改而对不上。
/// 以 source_path 为 key 缓存最近一次预览编译的产物，跳转时直接取用。
fn compile_typ_and_cache(
    source_path: &str,
    root: PathBuf,
    content: String,
) -> Result<Vec<u8>, String> {
    let world = GuaxiTypstWorld::new(root, content);
    let result = typst::compile::<PagedDocument>(&world);
    let document = result
        .output
        .map_err(|diags| format_diagnostics(&world, &diags))?;
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|diags| format_diagnostics(&world, &diags))?;

    let entry = TypstJumpEntry {
        world: Arc::new(world),
        document: Arc::new(document),
    };
    JUMP_CACHE
        .lock()
        .unwrap()
        .insert(source_path.to_string(), entry);

    Ok(pdf)
}

/// 跳转缓存条目：最近一次成功预览编译的 World + 文档。
#[derive(Clone)]
struct TypstJumpEntry {
    world: Arc<GuaxiTypstWorld>,
    document: Arc<PagedDocument>,
}

/// source_path -> 最近一次编译产物。
static JUMP_CACHE: Lazy<Mutex<HashMap<String, TypstJumpEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize)]
pub struct TypstForwardResult {
    pub page: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize)]
pub struct TypstInverseResult {
    pub line: u32,
    pub column: u32,
}

/// 正向：源码 (line, column) -> PDF 位置（page + 左上原点坐标 pt）。
/// 行列均 1-based（与编辑器一致），内部转 0-based。
#[tauri::command]
pub async fn typst_forward_search(
    source_path: String,
    line: u32,
    column: u32,
) -> Result<Option<TypstForwardResult>, String> {
    let entry = {
        let cache = JUMP_CACHE.lock().unwrap();
        cache.get(&source_path).cloned()
    };
    let entry = match entry {
        Some(e) => e,
        None => return Ok(None), // 还没编译过：前端会先触发一次预览构建
    };

    let source = entry.world.main_source.clone();
    let line0 = line.saturating_sub(1) as usize;
    let col0 = column.saturating_sub(1) as usize;
    let cursor = match source.lines().line_column_to_byte(line0, col0) {
        Some(b) => b,
        None => source
            .lines()
            .line_to_byte(line0)
            .unwrap_or(0),
    };

    let positions = jump_from_cursor(&entry.document, &source, cursor);
    Ok(positions.first().map(|pos| TypstForwardResult {
        page: pos.page.get() as u32,
        x: pos.point.x.to_pt() as f32,
        y: pos.point.y.to_pt() as f32,
    }))
}

/// 反向：PDF 点击 (page, x, y)（pt，左上原点）-> 源码 (line, column)，均 1-based。
#[tauri::command]
pub async fn typst_inverse_search(
    source_path: String,
    page: u32,
    x: f32,
    y: f32,
) -> Result<Option<TypstInverseResult>, String> {
    let entry = {
        let cache = JUMP_CACHE.lock().unwrap();
        cache.get(&source_path).cloned()
    };
    let entry = match entry {
        Some(e) => e,
        None => return Ok(None),
    };

    let page_idx = page.saturating_sub(1) as usize;
    let pdf_page = match entry.document.pages.get(page_idx) {
        Some(p) => p,
        None => return Ok(None),
    };

    let click = Point::new(Abs::pt(x as f64), Abs::pt(y as f64));
    let jump = jump_from_click(
        entry.world.as_ref(),
        &entry.document,
        &pdf_page.frame,
        click,
    );

    let Some(Jump::File(id, offset)) = jump else {
        return Ok(None);
    };

    // 命中的应是主源文件（编辑器当前内容）。
    let source = if id == entry.world.main_id {
        entry.world.main_source.clone()
    } else {
        match entry.world.source(id) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        }
    };

    let lines = source.lines();
    let line0 = lines.byte_to_line(offset).unwrap_or(0);
    let col0 = lines.byte_to_column(offset).unwrap_or(0);

    Ok(Some(TypstInverseResult {
        line: (line0 + 1) as u32,
        column: (col0 + 1) as u32,
    }))
}

fn input_dir_of(source_path: &str) -> Result<(PathBuf, PathBuf), String> {
    let source = PathBuf::from(source_path.trim());
    let input_dir = source
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| p.is_dir())
        .ok_or_else(|| "无法定位 .typ 文件所在目录".to_string())?;
    Ok((source, input_dir))
}

/// 为 Typst 实时预览构建 PDF：把编辑器内容编译为临时 PDF，返回路径。
#[tauri::command]
pub async fn build_typ_pdf_preview(source_path: String, content: String) -> Result<String, String> {
    let (source, input_dir) = input_dir_of(&source_path)?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preview")
        .to_string();

    let pdf_bytes = compile_typ_and_cache(&source_path, input_dir, content)?;

    let tmp_base = std::env::temp_dir().join("guaxi_typst_preview");
    let _ = std::fs::create_dir_all(&tmp_base);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let outdir = tmp_base.join(format!("{}", now.as_nanos()));
    std::fs::create_dir_all(&outdir).map_err(|e| format!("创建预览输出目录失败: {e}"))?;
    let pdf_path = outdir.join(format!(".guaxi_preview_{stem}.pdf"));
    std::fs::write(&pdf_path, &pdf_bytes).map_err(|e| format!("写入预览 PDF 失败: {e}"))?;

    Ok(pdf_path.to_string_lossy().to_string())
}

/// 导出：从磁盘源文件编译并写到目标路径。
#[tauri::command]
pub async fn export_typ_pdf(input_path: String, output_path: String) -> Result<(), String> {
    let input = PathBuf::from(input_path.trim());
    if !input.is_file() {
        return Err(format!("typ file not found: {}", input.to_string_lossy()));
    }
    let input_dir = input
        .parent()
        .ok_or_else(|| "Failed to get typ parent dir".to_string())?
        .to_path_buf();
    let content = std::fs::read_to_string(&input)
        .map_err(|e| format!("读取 .typ 源文件失败: {e}"))?;

    let pdf_bytes = compile_typ_to_pdf(input_dir, content)?;

    let output = PathBuf::from(output_path.trim());
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&output, &pdf_bytes)
        .map_err(|e| format!("写入导出 PDF 失败: {e} (path={})", output.to_string_lossy()))?;
    info!(to = %output.display(), "[ExportTyp] pdf exported");
    Ok(())
}
