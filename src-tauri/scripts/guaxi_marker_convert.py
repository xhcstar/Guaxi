"""Guaxi 的 marker 单文件转换入口（跑在 marker 独立 uv venv 里）。

与上游 `marker/scripts/convert_single.py` 的差异：

1. `--progress_json`：把进度以单行 JSON 打到 stdout，供 Rust 侧调度器解析并回显到任务面板；
2. `--output_dir` 直接作为产物目录——上游 `ConfigParser.get_output_folder()` 会在其后再追加
   一层文件名子目录，而 Guaxi 已经把 `{基目录}/{文件名}` 算好传进来，再追加会导致产物路径对不上；
3. 结束时额外输出 `conversion_result`，让上层确认产物真的写出了；
4. Windows 可嵌入 Python 的多进程修复（见下）；
5. **大文档自动分块**（见下）。

## 为什么大文档要分块

marker 的 `DocumentBuilder.build_document()` 会 `provider.get_images(page_range, lowres_dpi)`
**一次性把所有页的低清图读进内存**；上千页的 PDF 光这一项就是数 GB（96dpi 下 A4 约 2.7MB/页），
再加各页的数据结构，必然爆内存。而 surya 的 `--page_range` 用的是**原始页号**
（`page_bboxes = {i: ... for i in page_range}`），分页标记 `{N}` 也取自原始页号，
因此按页窗口分块既能限制内存，又能让各块的产物**天然可拼接**：

- 各块的 md 里 `{N}` 仍是全局页号，直接按顺序拼起来就是一份完整的 md（上游单次转换也是这样）；
- 各块图片名 `_page_N_*.jpeg` 带上原始页号，不会跨块冲突。

另外，分块是在**同一个进程内**循环完成的：模型（含 surya 的 VLM 推理服务）只加载一次，
每块只重建廉价的转换器并处理自己的页窗口，块结束后整份文档对象被释放——既省内存又不重复加载模型。

页数不超过阈值时完全走原来的单次路径，产物与行为与改造前**逐字节一致**。
"""

import os
import shutil
import sys

# --- Windows 多进程兼容修复（必须在导入 multiprocessing / torch 之前执行）---
# 托管 Python 为 embeddable 版：建 venv 后其 base python 目录被清理，
# 导致 Windows 上 multiprocessing spawn 使用不存在的 base-executable，报 WinError 3。
# 将 _base_executable 指向当前可用的 venv python 即可修复（有真实 base 时为无操作）。
if sys.platform == "win32":
    _be = getattr(sys, "_base_executable", None)
    if not _be or not os.path.exists(_be):
        sys._base_executable = sys.executable

os.environ["GRPC_VERBOSITY"] = "ERROR"
os.environ["GLOG_minloglevel"] = "2"
os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = (
    "1"  # Transformers uses .isin for a simple op, which is not supported on MPS
)

import json
import time
import traceback

import click

from marker.config.parser import ConfigParser
from marker.config.printer import CustomClickPrinter
from marker.logger import configure_logging, get_logger
from marker.models import create_model_dict
from marker.output import save_output

configure_logging()
logger = get_logger()

OUTPUT_EXT = {"markdown": "md", "html": "html", "json": "json", "chunks": "json"}

# 大文档分块的默认参数。
# - `THRESHOLD`：页数不超过它就直接单次转换，保证小文档路径与改造前完全一致；
# - `CHUNK_PAGES`：每块页数。40 页在 96dpi 下约 110MB 低清图，留足余量又不会太碎。
DEFAULT_CHUNK_THRESHOLD = 100
DEFAULT_CHUNK_PAGES = 40

# 分块过程中的临时目录名（放在最终产物目录内，结束时删除）。
PARTS_DIRNAME = ".guaxi_parts"


def _emit_progress(enabled: bool, current: int, total: int, message: str):
    if not enabled:
        return
    print(
        json.dumps(
            {
                "type": "conversion_progress",
                "current": int(current),
                "total": int(total),
                "message": str(message),
            },
            ensure_ascii=False,
        ),
        flush=True,
    )


def _env_int(name: str, default: int) -> int:
    try:
        value = int(str(os.environ.get(name, "")).strip())
        return value if value > 0 else default
    except Exception:
        return default


def _pdf_page_count(path: str):
    """返回 PDF 页数；非 PDF 或读取失败返回 None（→ 走单次路径）。"""
    try:
        import pypdfium2 as pdfium  # surya 的既有依赖，不额外引入

        doc = pdfium.PdfDocument(path)
        try:
            return len(doc)
        finally:
            doc.close()
    except Exception:
        return None


def _make_converter(models, kwargs, page_range: str | None):
    """按给定配置（可带页范围）构造一个转换器。

    模型（models）在调用方复用，这里只重建廉价的配置/转换器对象，
    因此分块不会重复加载模型与推理服务。
    """
    options = dict(kwargs)
    if page_range:
        options["page_range"] = page_range
    else:
        options.pop("page_range", None)
    config_parser = ConfigParser(options)
    converter_cls = config_parser.get_converter_cls()
    converter = converter_cls(
        config=config_parser.generate_config_dict(),
        artifact_dict=models,
        processor_list=config_parser.get_processors(),
        renderer=config_parser.get_renderer(),
        llm_service=config_parser.get_llm_service(),
    )
    return converter, config_parser


def _convert_once(fpath, models, kwargs, out_folder, page_range=None):
    """转换一次并写出产物，返回 (md 路径, 该次写出的其它文件列表)。"""
    os.makedirs(out_folder, exist_ok=True)
    before = set(os.listdir(out_folder)) if os.path.isdir(out_folder) else set()

    converter, config_parser = _make_converter(models, kwargs, page_range)
    rendered = converter(fpath)

    base_name = config_parser.get_base_filename(fpath)
    save_output(rendered, out_folder, base_name)

    output_format = kwargs.get("output_format", "markdown")
    md_path = os.path.join(out_folder, f"{base_name}.{OUTPUT_EXT.get(output_format, 'md')}")
    after = set(os.listdir(out_folder))
    extras = [os.path.join(out_folder, name) for name in sorted(after - before)]
    return md_path, extras


def _read_text(path: str) -> str:
    # newline="" 关掉换行翻译：各块产物是 Windows 风格 CRLF，原样读进来再原样拼出去，
    # 合并结果才能与单次转换逐字节一致（用默认的通用换行会把 CRLF 折成 LF）。
    with open(path, "r", encoding="utf-8", errors="replace", newline="") as handle:
        return handle.read()


def _merge_markdown(part_md_paths, dest_md_path: str) -> None:
    """按顺序把各块的 md 拼成一份（各块的 `{N}` 已是全局页号）。"""
    parent = os.path.dirname(dest_md_path)
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(dest_md_path, "w", encoding="utf-8", newline="") as out:
        for path in part_md_paths:
            out.write(_read_text(path))


def _merge_meta(part_meta_paths, dest_meta_path: str) -> None:
    """合并各块的 `_meta.json`。

    主要是 `page_stats` 按页拼接；结构不符合预期时退回第一块的 meta，
    保证一定产出一份可用文件（阅读器并不读它，但保持产物形态与改造前一致）。
    """
    merged = None
    try:
        stats = []
        tocs = []
        base = None
        for path in part_meta_paths:
            if not os.path.isfile(path):
                continue
            data = json.loads(_read_text(path))
            if base is None:
                base = data
            page_stats = data.get("page_stats")
            if isinstance(page_stats, list):
                stats.extend(page_stats)
            toc = data.get("table_of_contents")
            if isinstance(toc, list):
                tocs.extend(toc)
        if base is None:
            return
        merged = dict(base)
        # page_stats 与 table_of_contents 都是按页产生的、且 page_id 已是全局页号，
        # 因此按块顺序拼接即等价于单次转换的结果（不拼的话只会剩下首块的条目）。
        if stats and isinstance(base.get("page_stats"), list):
            merged["page_stats"] = stats
        if tocs and isinstance(base.get("table_of_contents"), list):
            merged["table_of_contents"] = tocs
    except Exception as exc:  # noqa: BLE001
        logger.warning(f"合并 meta.json 失败，退回首块内容: {exc}")
        for path in part_meta_paths:
            if os.path.isfile(path):
                try:
                    merged = json.loads(_read_text(path))
                except Exception:  # noqa: BLE001
                    merged = None
                break
    if merged is None:
        return
    with open(dest_meta_path, "w", encoding="utf-8") as handle:
        json.dump(merged, handle, ensure_ascii=False, indent=2)


def _convert_chunked(fpath, models, kwargs, out_folder, progress_json, total_pages, chunk_pages):
    """按页窗口分块转换，并把各块产物拼成与单次转换同形的结果。"""
    base_name = ConfigParser(kwargs).get_base_filename(fpath)

    parts_dir = os.path.join(out_folder, PARTS_DIRNAME)
    shutil.rmtree(parts_dir, ignore_errors=True)
    os.makedirs(parts_dir, exist_ok=True)

    starts = list(range(0, total_pages, chunk_pages))
    total_chunks = len(starts)
    part_md_paths = []
    part_meta_paths = []
    moved_images = []

    try:
        for index, start in enumerate(starts):
            end = min(start + chunk_pages, total_pages) - 1
            page_range = f"{start}-{end}"
            done = index
            pct = 5 + int(80 * done / total_chunks)
            _emit_progress(
                progress_json,
                pct,
                100,
                f"正在转换（第 {index + 1}/{total_chunks} 块，共 {total_pages} 页，第 {start + 1}-{end + 1} 页）",
            )

            chunk_dir = os.path.join(parts_dir, f"chunk_{index:05d}")
            md_path, extras = _convert_once(
                fpath, models, kwargs, chunk_dir, page_range=page_range
            )
            part_md_paths.append(md_path)
            for extra in extras:
                name = os.path.basename(extra)
                if name.endswith("_meta.json"):
                    part_meta_paths.append(extra)

            # 每块结束后立刻把图片搬到最终目录，块目录随即可被清理，避免峰值占用翻倍。
            for extra in extras:
                name = os.path.basename(extra)
                if extra == md_path or name.endswith("_meta.json"):
                    continue
                target = os.path.join(out_folder, name)
                if os.path.exists(target):
                    os.remove(target)
                shutil.move(extra, target)
                moved_images.append(target)

        _emit_progress(progress_json, 88, 100, "正在合并分段结果")

        dest_md_path = os.path.join(out_folder, f"{base_name}.{OUTPUT_EXT.get(kwargs.get('output_format', 'markdown'), 'md')}")
        _merge_markdown(part_md_paths, dest_md_path)
        _merge_meta(part_meta_paths, os.path.join(out_folder, f"{base_name}_meta.json"))

        logger.info(
            f"Chunked conversion done: pages={total_pages} chunks={total_chunks} "
            f"images={len(moved_images)} md={dest_md_path}"
        )
        return dest_md_path
    finally:
        shutil.rmtree(parts_dir, ignore_errors=True)


def _pdf_looks_scanned(path: str, sample_pages: int = 8) -> bool | None:
    """判断 PDF 是否像扫描件：采样若干页，看能抽到多少文本。

    返回 True（像扫描件）/ False（像数字文档）/ None（判断不了，调用方应保守处理）。

    阈值取每页 100 个字符：数字文档每页通常几百到几千字符，扫描件（没有文字层）接近 0。
    """
    try:
        import pypdfium2 as pdfium

        doc = pdfium.PdfDocument(path)
        try:
            page_count = len(doc)
            if page_count <= 0:
                return None
            step = max(1, page_count // sample_pages)
            indices = list(range(0, page_count, step))[:sample_pages]
            chars = 0
            for index in indices:
                page = doc[index]
                textpage = page.get_textpage()
                chars += len((textpage.get_text_range() or "").strip())
            return (chars / len(indices)) < 100
        finally:
            doc.close()
    except Exception as exc:  # noqa: BLE001
        logger.warning(f"扫描件判断失败，按“不干预”处理: {exc}")
        return None


def _resolve_disable_ocr(fpath: str, ocr_mode: str, force_disable: bool, progress_json: bool) -> bool:
    """把 OCR 策略解析为“本次是否禁用 OCR”。

    策略语义（与 Rust 侧 apply_marker_ocr_policy 对应）：
    - full：不干预，完全交给 marker（质量最高）；
    - text_only：永不调用 VLM；
    - auto：**由调用方确认“没有 GPU”之后**才传进来，这里按文档类型决定——
      数字文档跳过 OCR（秒级），扫描件保留 OCR（慢但必要）。

    这样"只有 CPU 的机器"仍然能处理扫描件，只是慢；不会因为省时间而彻底废掉扫描件。
    """
    if force_disable or ocr_mode == "text_only":
        _emit_progress(progress_json, 42, 100, "仅文本层模式：不调用 OCR/VLM")
        return True

    if ocr_mode == "full":
        return False

    scanned = _pdf_looks_scanned(fpath)
    if scanned is False:
        _emit_progress(progress_json, 42, 100, "未检测到扫描页：跳过 OCR 以加速")
        return True
    if scanned is True:
        _emit_progress(progress_json, 42, 100, "检测到扫描页/图片页：保留 OCR（较慢）")
    return False


@click.command(cls=CustomClickPrinter, help="Convert a single PDF to markdown.")
@click.argument("fpath", type=str)
@click.option("--paginate_output", is_flag=True, default=False)
@click.option(
    "--disable_ocr",
    is_flag=True,
    default=False,
    help="完全不调用 VLM（等同 --ocr_mode text_only）。",
)
@click.option(
    "--ocr_mode",
    type=click.Choice(["auto", "full", "text_only"]),
    default="full",
    help="OCR 策略：full=不干预；text_only=只抽文本层；auto=数字文档跳过 OCR、扫描件保留。",
)
@click.option("--progress_json", is_flag=True, default=False)
@ConfigParser.common_options
def convert_single_cli(
    fpath: str,
    paginate_output: bool | None,
    disable_ocr: bool,
    ocr_mode: str,
    progress_json: bool,
    **kwargs,
):
    paginate_output = bool(paginate_output)
    start = time.time()
    _emit_progress(progress_json, 0, 100, "开始转换")

    try:
        _emit_progress(progress_json, 5, 100, "加载模型")
        models = create_model_dict()

        _emit_progress(progress_json, 40, 100, "初始化转换器")
        kwargs["paginate_output"] = paginate_output
        # disable_ocr 是 builder 的配置键（不在 common_options 里），显式注入即可。
        if _resolve_disable_ocr(fpath, ocr_mode, disable_ocr, progress_json):
            kwargs["disable_ocr"] = True
        config_parser = ConfigParser(kwargs)

        base_name = config_parser.get_base_filename(fpath)
        out_folder = kwargs.get("output_dir")
        if out_folder:
            os.makedirs(out_folder, exist_ok=True)
        else:
            out_folder = config_parser.get_output_folder(fpath)

        # 大文档走分块路径；小文档与改造前完全一致（单次转换）。
        page_count = _pdf_page_count(fpath)
        chunk_threshold = _env_int("GUAXI_MARKER_CHUNK_THRESHOLD", DEFAULT_CHUNK_THRESHOLD)
        chunk_pages = _env_int("GUAXI_MARKER_CHUNK_PAGES", DEFAULT_CHUNK_PAGES)

        if page_count and page_count > chunk_threshold:
            logger.info(
                f"Large document detected: pages={page_count} chunk_pages={chunk_pages} "
                f"-> chunked conversion"
            )
            _emit_progress(
                progress_json,
                45,
                100,
                f"文档较大（{page_count} 页），按每块 {chunk_pages} 页分批转换",
            )
            output_file = _convert_chunked(
                fpath, models, kwargs, out_folder, progress_json, page_count, chunk_pages
            )
        else:
            _emit_progress(
                progress_json,
                55,
                100,
                "正在转换（首次运行需启动推理服务并下载模型，可能较久）",
            )
            converter, _ = _make_converter(models, kwargs, None)
            rendered = converter(fpath)

            _emit_progress(progress_json, 90, 100, "正在写入结果")
            save_output(rendered, out_folder, base_name)
            output_format = kwargs.get("output_format", "markdown")
            output_file = os.path.join(
                out_folder, f"{base_name}.{OUTPUT_EXT.get(output_format, 'md')}"
            )

        if progress_json:
            print(
                json.dumps(
                    {
                        "type": "conversion_result",
                        "output_dir": out_folder,
                        "output_file": output_file,
                        "output_exists": os.path.exists(output_file),
                    },
                    ensure_ascii=False,
                ),
                flush=True,
            )

        _emit_progress(progress_json, 100, 100, "转换完成")
        logger.info(f"Saved markdown to {out_folder}")
        logger.info(f"Total time: {time.time() - start}")
    except Exception as e:
        _emit_progress(progress_json, 100, 100, f"转换失败: {e}")
        traceback.print_exc()
        raise


if __name__ == "__main__":
    convert_single_cli()
