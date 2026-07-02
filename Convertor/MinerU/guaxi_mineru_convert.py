"""MinerU 强引擎包装：复杂/理工科文档 -> Markdown（保留公式、表格、代码、图片）。

在 MinerU 独立 venv 中运行（与 marker venv 隔离，避免 torch/依赖冲突）。

CLI（与 marker / fast 引擎对齐，便于 Rust 统一调度）：
    guaxi_mineru_convert.py <fpath> --output_dir <dir> [--progress_json]

输出契约（与 marker / fast 一致）：Markdown 写到 <output_dir>/<stem>.md，
图片写到 <output_dir>（相对引用），stdout 输出进度 JSON：
    {"type":"conversion_progress","current":..,"total":100,"message":".."}
    {"type":"conversion_result","output_file":"<abs md path>","output_exists":true}

兼容性：MinerU 的 Python API 在 1.x / 2.x 间有差异，这里做多路探测：
  1) 优先尝试 `mineru`（2.x）的高层 pipeline API；
  2) 回退 `magic_pdf`（1.x）的 UNIPipe / 数据读取 API；
  3) 均不可用则报错（Rust 侧会在构建计划前通过 mineru_available() 拦截，
     运行期异常则由队列标记失败并按策略回退/重试）。
"""

import os
import sys

# --- Windows 多进程兼容修复（必须在导入 multiprocessing / mineru 之前执行）---
# 托管 Python 为 embeddable 版：建 venv 后其 base python 目录被清理，
# 导致 Windows 上 multiprocessing spawn 使用不存在的 base-executable，报 WinError 3。
# 将 _base_executable 指向当前可用的 venv python 即可修复（有真实 base 时为无操作）。
if sys.platform == "win32":
    _be = getattr(sys, "_base_executable", None)
    if not _be or not os.path.exists(_be):
        sys._base_executable = sys.executable

os.environ.setdefault("GRPC_VERBOSITY", "ERROR")
os.environ.setdefault("GLOG_minloglevel", "2")

import argparse
import json
import shutil
import sys
import time
import traceback


def _emit_progress(enabled, current, total, message):
    if not enabled:
        return
    print(
        json.dumps(
            {"type": "conversion_progress", "current": int(current),
             "total": int(total), "message": str(message)},
            ensure_ascii=False,
        ),
        flush=True,
    )


def _emit_result(output_file, exists):
    print(
        json.dumps(
            {"type": "conversion_result", "output_file": output_file,
             "output_exists": bool(exists)},
            ensure_ascii=False,
        ),
        flush=True,
    )


def _finalize(md_text, images_src_dir, out_dir, stem, progress):
    """写出 Markdown 与图片，返回最终 md 路径。"""
    md_path = os.path.join(out_dir, f"{stem}.md")
    os.makedirs(out_dir, exist_ok=True)

    # 迁移图片目录（MinerU 常输出到 images/ 子目录），保持 Markdown 内相对引用可用。
    if images_src_dir and os.path.isdir(images_src_dir):
        dst_images = os.path.join(out_dir, "images")
        if os.path.abspath(images_src_dir) != os.path.abspath(dst_images):
            os.makedirs(dst_images, exist_ok=True)
            for name in os.listdir(images_src_dir):
                try:
                    shutil.copy2(os.path.join(images_src_dir, name),
                                 os.path.join(dst_images, name))
                except Exception:
                    pass

    with open(md_path, "w", encoding="utf-8") as f:
        f.write(md_text or "")
    _emit_progress(progress, 95, 100, "写入 Markdown")
    return md_path


def _convert_mineru_2x(fpath, out_dir, stem, progress):
    """MinerU 2.x：使用高层 API 完成解析并拿到 markdown。"""
    # 2.x 的公开入口在不同小版本间有差异，做保守探测。
    from mineru.cli.common import do_parse  # type: ignore

    _emit_progress(progress, 30, 100, "MinerU 解析中")
    # do_parse 常见签名：(output_dir, pdf_file_names, pdf_bytes_list, p_lang_list, backend=...)
    with open(fpath, "rb") as f:
        pdf_bytes = f.read()

    work_dir = os.path.join(out_dir, "_mineru")
    os.makedirs(work_dir, exist_ok=True)

    do_parse(
        output_dir=work_dir,
        pdf_file_names=[stem],
        pdf_bytes_list=[pdf_bytes],
        p_lang_list=["ch"],
        backend="pipeline",
    )
    _emit_progress(progress, 85, 100, "整理输出")

    # 递归寻找生成的 .md 与 images 目录
    md_text, images_dir = _harvest_output(work_dir, stem)
    return _finalize(md_text, images_dir, out_dir, stem, progress)


def _convert_magic_pdf_1x(fpath, out_dir, stem, progress):
    """MinerU 1.x（magic_pdf）：UNIPipe 流水线。"""
    from magic_pdf.data.data_reader_writer import FileBasedDataWriter  # type: ignore
    from magic_pdf.data.dataset import PymuDocDataset  # type: ignore
    from magic_pdf.model.doc_analyze_by_custom_model import doc_analyze  # type: ignore
    from magic_pdf.config.enums import SupportedPdfParseMethod  # type: ignore

    images_dir = os.path.join(out_dir, "images")
    os.makedirs(images_dir, exist_ok=True)
    image_writer = FileBasedDataWriter(images_dir)

    _emit_progress(progress, 30, 100, "MinerU 解析中")
    with open(fpath, "rb") as f:
        pdf_bytes = f.read()
    ds = PymuDocDataset(pdf_bytes)

    if ds.classify() == SupportedPdfParseMethod.OCR:
        infer = ds.apply(doc_analyze, ocr=True)
        pipe = infer.pipe_ocr_mode(image_writer)
    else:
        infer = ds.apply(doc_analyze, ocr=False)
        pipe = infer.pipe_txt_mode(image_writer)

    _emit_progress(progress, 85, 100, "生成 Markdown")
    # 图片以相对 images/ 引用
    md_text = pipe.get_markdown("images")
    return _finalize(md_text, images_dir, out_dir, stem, progress)


def _harvest_output(work_dir, stem):
    """在 MinerU 2.x 工作目录中定位 .md 与 images/。"""
    md_text = ""
    images_dir = None
    best_md = None
    for root, _dirs, files in os.walk(work_dir):
        for name in files:
            if name.lower().endswith(".md"):
                # 优先匹配 stem 的 md
                full = os.path.join(root, name)
                if best_md is None or stem in name:
                    best_md = full
        if os.path.basename(root) == "images":
            images_dir = root
    if best_md and os.path.isfile(best_md):
        with open(best_md, "r", encoding="utf-8", errors="replace") as f:
            md_text = f.read()
    return md_text, images_dir


def main(argv):
    parser = argparse.ArgumentParser(description="Guaxi MinerU 强引擎")
    parser.add_argument("fpath", type=str)
    parser.add_argument("--output_dir", type=str, required=True)
    parser.add_argument("--progress_json", action="store_true", default=False)
    args = parser.parse_args(argv[1:])

    progress = args.progress_json
    start = time.time()
    _emit_progress(progress, 0, 100, "开始转换")

    fpath = args.fpath
    out_dir = args.output_dir
    os.makedirs(out_dir, exist_ok=True)
    stem = os.path.splitext(os.path.basename(fpath))[0]

    _emit_progress(progress, 10, 100, "加载 MinerU 引擎")

    md_path = None
    last_err = None

    # 依次尝试 2.x -> 1.x
    for fn in (_convert_mineru_2x, _convert_magic_pdf_1x):
        try:
            md_path = fn(fpath, out_dir, stem, progress)
            if md_path and os.path.exists(md_path):
                break
        except ImportError as e:
            last_err = e
            continue
        except Exception as e:
            last_err = e
            traceback.print_exc()
            continue

    if not md_path or not os.path.exists(md_path):
        _emit_progress(progress, 100, 100, f"转换失败: {last_err}")
        sys.stderr.write(f"[guaxi_mineru] failed: {last_err}\n")
        return 1

    _emit_progress(progress, 100, 100, "转换完成")
    _emit_result(os.path.abspath(md_path), True)
    sys.stderr.write(f"[guaxi_mineru] saved {md_path} in {time.time()-start:.2f}s\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
