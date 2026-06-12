//! Typst 编译支持：把 .typ 源码编译成 PDF（纯 Rust，无外部依赖）。
//!
//! 设计说明：
//! 1. 与 LaTeX 预览（tectonic）对齐，提供两个命令：
//!    - `build_typ_pdf_preview(source_path, content)`：编译编辑器当前内容到临时 PDF，返回路径，供实时预览。
//!    - `export_typ_pdf(input_path, output_path)`：从磁盘源文件编译并写到目标路径，供导出。
//! 2. 字体来自系统（`include_system_fonts(true)`）；不内嵌、不联网下载包。中文需用户系统装有中文字体。
//! 3. 字体扫描较慢，全局缓存一次（`FONTS`），后续预览复用。

use std::path::PathBuf;

use chrono::{Datelike, Local, Timelike};
use once_cell::sync::Lazy;
use tracing::info;

use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
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

    let pdf_bytes = compile_typ_to_pdf(input_dir, content)?;

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
