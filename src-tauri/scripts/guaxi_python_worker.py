import ast
import io
import json
import os
import re
import shutil
import subprocess
import sys
import traceback
import uuid

SESSIONS = {}
RESULT_HISTORY = {}
SESSION_DIRS = {}
TABLE_HANDLES = {}


def _normalize_basic(value):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, (list, tuple)):
        return [_normalize_basic(v) for v in value]
    if isinstance(value, dict):
        return {str(key): _normalize_basic(item) for key, item in value.items()}
    if hasattr(value, "isoformat"):
        try:
            return value.isoformat()
        except Exception:
            pass
    if hasattr(value, "item"):
        try:
            return _normalize_basic(value.item())
        except Exception:
            pass
    return str(value)


def _json_basic(value):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, (list, tuple)):
        return [_json_basic(v) for v in value]
    if isinstance(value, dict):
        return {str(key): _json_basic(item) for key, item in value.items()}
    return None


def _normalize_exec_code(value):
    text = str(value or "")
    if not text:
        return text

    # Some upstream text paths can carry lone surrogate code points.
    # Python's parser rejects those during ast.parse/compile, so we
    # strip them before execution.
    if any(0xD800 <= ord(ch) <= 0xDFFF for ch in text):
        text = "".join(ch for ch in text if not (0xD800 <= ord(ch) <= 0xDFFF))

    return text


def _safe_repr(value):
    try:
        text = repr(value)
    except Exception:
        text = f"<{type(value).__name__}>"
    return str(text)


def _truncate_text(text, limit=240):
    raw = str(text or "")
    if len(raw) <= limit:
        return raw
    return raw[: max(0, limit - 3)] + "..."


def _is_small_basic_structure(value, depth=0):
    if depth > 2:
        return False
    if value is None or isinstance(value, (bool, int, float, str)):
        return True
    if isinstance(value, (list, tuple)):
        return len(value) <= 12 and all(_is_small_basic_structure(item, depth + 1) for item in value)
    if isinstance(value, dict):
        return len(value) <= 12 and all(
            isinstance(key, (bool, int, float, str)) and _is_small_basic_structure(item, depth + 1)
            for key, item in value.items()
        )
    return False


def _compact_preview(value):
    # 设计说明：
    # 1. 之前这里大量依赖 repr(value)，大 DataFrame / 多 sheet dict / ndarray 会在“执行已完成”后还继续花很久组装字符串。
    # 2. 用户看到的现象就是前端一直停在“Python 正在运行”，但真正拖慢的往往不是读表，而是结果摘要和变量快照。
    # 3. 因此这里改成“简单对象保留内容，大对象只返回结构摘要”，优先保证产品可用性和稳定时延。
    if _is_small_basic_structure(value):
        return _truncate_text(_safe_repr(value))

    if isinstance(value, str):
        return repr(_truncate_text(value))

    if isinstance(value, (bytes, bytearray)):
        return f"<{type(value).__name__} len={len(value)}>"

    pandas_module = sys.modules.get("pandas")
    if pandas_module is not None:
        try:
            if isinstance(value, pandas_module.DataFrame):
                columns = [str(col) for col in value.columns.tolist()[:8]]
                suffix = "..." if len(value.columns) > len(columns) else ""
                return (
                    f"<DataFrame rows={len(value.index)} cols={len(value.columns)} "
                    f"columns={columns}{suffix}>"
                )
            if isinstance(value, pandas_module.Series):
                return f"<Series len={len(value)} dtype={getattr(value, 'dtype', 'unknown')}>"
        except Exception:
            pass

    numpy_module = sys.modules.get("numpy")
    if numpy_module is not None:
        try:
            if isinstance(value, numpy_module.ndarray):
                return f"<ndarray shape={list(value.shape)} dtype={value.dtype}>"
        except Exception:
            pass

    if isinstance(value, dict):
        keys = [str(key) for key in list(value.keys())[:6]]
        suffix = "..." if len(value) > len(keys) else ""
        return f"<dict len={len(value)} keys={keys}{suffix}>"

    if isinstance(value, (list, tuple, set)):
        return f"<{type(value).__name__} len={len(value)}>"

    try:
        shape = getattr(value, "shape")
        if isinstance(shape, tuple):
            return f"<{type(value).__name__} shape={list(shape)}>"
    except Exception:
        pass

    try:
        if hasattr(value, "__len__"):
            return f"<{type(value).__name__} len={len(value)}>"
    except Exception:
        pass

    return f"<{type(value).__name__}>"


def _configure_verbose_object_rendering(globals_dict):
    # Design notes:
    # 1. Python Cell 每次执行都会先经过这里，哪怕只是简单的 `print("hello world")`。
    # 2. 如果为了格式化输出而在这里主动 import pandas / numpy，一旦这些大库的导入异常变慢或卡住，
    #    前端就会表现成“一直运行中”。
    # 3. 因此这里只对当前 worker 已经加载过的模块补一次显示配置，既保留完整 repr，又避免无关 cell 被重型依赖拖住。
    pandas_module = sys.modules.get("pandas")
    if pandas_module is not None and not globals_dict.get("__guaxi_pandas_verbose_repr_configured__"):
        try:
            pandas_module.set_option("display.max_rows", None)
            pandas_module.set_option("display.min_rows", None)
            pandas_module.set_option("display.max_columns", None)
            pandas_module.set_option("display.max_colwidth", None)
            pandas_module.set_option("display.width", None)
            pandas_module.set_option("display.expand_frame_repr", False)
            globals_dict["__guaxi_pandas_verbose_repr_configured__"] = True
        except Exception:
            pass

    numpy_module = sys.modules.get("numpy")
    if numpy_module is not None and not globals_dict.get("__guaxi_numpy_verbose_repr_configured__"):
        try:
            numpy_module.set_printoptions(threshold=sys.maxsize, linewidth=sys.maxsize)
            globals_dict["__guaxi_numpy_verbose_repr_configured__"] = True
        except Exception:
            pass


def _summarize_value(value):
    summary = {
        "type": type(value).__name__,
        "preview": _compact_preview(value),
    }

    try:
        if hasattr(value, "shape"):
            shape = getattr(value, "shape")
            if isinstance(shape, tuple):
                summary["shape"] = [_normalize_basic(item) for item in shape]
    except Exception:
        pass

    try:
        if hasattr(value, "__len__") and not isinstance(value, (str, bytes, bytearray)):
            summary["length"] = int(len(value))
    except Exception:
        pass

    try:
        pandas_module = sys.modules.get("pandas")
        if pandas_module is not None and isinstance(value, pandas_module.DataFrame):
            summary["rows"] = int(len(value.index))
            summary["columns"] = int(len(value.columns))
            summary["column_names"] = [str(col) for col in value.columns.tolist()]
            summary["dtypes"] = {str(k): str(v) for k, v in value.dtypes.astype(str).to_dict().items()}
        elif pandas_module is not None and isinstance(value, pandas_module.Series):
            summary["rows"] = int(len(value.index))
            summary["columns"] = 1
            summary["column_names"] = [str(value.name or "value")]
            summary["dtypes"] = {str(value.name or "value"): str(value.dtype)}
    except Exception:
        pass

    return summary


def _hidden_session_var_names():
    return {
        "__builtins__",
        "__guaxi_last_value",
        "__guaxi_plotly_display__",
        "__guaxi_pandas_verbose_repr_configured__",
        "__guaxi_numpy_verbose_repr_configured__",
        "_",
        "__",
        "___",
        "_result",
        "_guaxi_session",
        "_guaxi_inline_matplotlib",
        "_guaxi_cwd",
        "_guaxi_sandbox_dir",
        "_doc_path",
        "_py_cell_id",
    }


def _is_visible_session_value(name, value):
    if name in _hidden_session_var_names() or str(name).startswith("__"):
        return False
    module_type = type(sys)
    if isinstance(value, module_type):
        return False
    if isinstance(value, type):
        return False
    if callable(value):
        return False
    return True


def _snapshot_session_var_map(globals_dict):
    output = {}
    for name in sorted(globals_dict.keys()):
        try:
            value = globals_dict.get(name)
            if not _is_visible_session_value(name, value):
                continue
            output[str(name)] = _summarize_value(value)
        except Exception:
            output[str(name)] = {
                "type": "unknown",
                "preview": "<unavailable>",
            }
    return output


def _summary_signature(summary):
    if not isinstance(summary, dict):
        return ""
    return json.dumps({
        "type": summary.get("type"),
        "shape": summary.get("shape"),
        "length": summary.get("length"),
        "rows": summary.get("rows"),
        "columns": summary.get("columns"),
        "preview": summary.get("preview"),
    }, ensure_ascii=False, sort_keys=True, default=str)


def _analyze_code_symbol_flow(code):
    # 设计说明：
    # 1. 前端需要知道“这个 Cell 可能读/写了哪些变量”，从而给下游 Cell 标记过期和展示依赖图。
    # 2. 这里使用 Python AST 做保守静态分析：能识别 Name 的 Load/Store/Del，但不尝试解析动态 globals()/exec()。
    # 3. 真实的新增/修改/删除仍由执行前后的变量快照确认，静态分析只作为依赖提示。
    flow = {
        "used": [],
        "assigned": [],
        "deleted": [],
    }
    try:
        tree = ast.parse(code or "", mode="exec")
    except Exception:
        return flow

    used = set()
    assigned = set()
    deleted = set()
    hidden = _hidden_session_var_names()

    for node in ast.walk(tree):
        if not isinstance(node, ast.Name):
            continue
        name = str(node.id or "")
        if not name or name in hidden or name.startswith("__"):
            continue
        if isinstance(node.ctx, ast.Load):
            used.add(name)
        elif isinstance(node.ctx, (ast.Store, ast.Param)):
            assigned.add(name)
        elif isinstance(node.ctx, ast.Del):
            deleted.add(name)

    flow["used"] = sorted(used - assigned)
    flow["assigned"] = sorted(assigned)
    flow["deleted"] = sorted(deleted)
    return flow


def _build_dataflow_payload(code, before_vars, after_vars):
    before = before_vars if isinstance(before_vars, dict) else {}
    after = after_vars if isinstance(after_vars, dict) else {}
    static_flow = _analyze_code_symbol_flow(code)

    before_names = set(before.keys())
    after_names = set(after.keys())
    created = sorted(after_names - before_names)
    removed = sorted(before_names - after_names)
    updated = sorted(
        name
        for name in before_names.intersection(after_names)
        if _summary_signature(before.get(name)) != _summary_signature(after.get(name))
    )

    return {
        "version": 1,
        "used": static_flow.get("used") or [],
        "assigned": static_flow.get("assigned") or [],
        "deleted": static_flow.get("deleted") or [],
        "created": created,
        "updated": updated,
        "removed": removed,
        "touched": sorted(set(created + updated + removed + (static_flow.get("assigned") or []))),
        "after": after,
    }


def _normalize_session_var_names(value):
    if not isinstance(value, (list, tuple, set)):
        return None

    names = []
    seen = set()
    for item in value:
        name = str(item or "").strip()
        if not name or name in seen:
            continue
        seen.add(name)
        names.append(name)

    # 设计说明：
    # 1. `None` 表示“不过滤，返回全部会话变量”，而空列表 `[]` 在产品语义上通常不是一个有意义的调用方式；
    # 2. 历史上中间层一度把未传值错误编码成 `[]`，导致 worker 把它解释成“过滤后无任何变量”；
    # 3. 这里把归一化后的空列表回退为 None，作为防御性兼容，避免未来再次出现同类问题。
    return names or None


def _snapshot_session_vars(globals_dict, selected_names=None):
    items = []

    candidate_names = selected_names if selected_names is not None else sorted(globals_dict.keys())
    for name in candidate_names:
        try:
            if name not in globals_dict:
                continue
            value = globals_dict.get(name)
            if not _is_visible_session_value(name, value):
                continue
            items.append({"name": str(name), **_summarize_value(value)})
            if len(items) >= 128:
                break
        except Exception:
            items.append({
                "name": str(name),
                "type": "unknown",
                "preview": "<unavailable>",
            })
    return items


def _capture_matplotlib_png_base64():
    try:
        if "matplotlib" not in sys.modules:
            return None
        import base64
        from io import BytesIO
        from matplotlib._pylab_helpers import Gcf

        managers = Gcf.get_all_fig_managers()
        if not managers:
            return None

        fig = managers[-1].canvas.figure
        buf = BytesIO()
        fig.savefig(buf, format="png", bbox_inches="tight")
        png_b64 = base64.b64encode(buf.getvalue()).decode("ascii")

        try:
            import matplotlib.pyplot as plt

            for manager in managers:
                try:
                    plt.close(manager.canvas.figure)
                except Exception:
                    pass
        except Exception:
            pass

        return png_b64
    except Exception:
        return None


def _prepare_display_artifact_dir(globals_dict, display_kind):
    sandbox_dir = str(
        globals_dict.get("_guaxi_sandbox_dir")
        or globals_dict.get("_guaxi_cwd")
        or os.getcwd()
    ).strip()
    artifact_root = os.path.join(sandbox_dir, ".guaxi-display-artifacts", str(display_kind or "display"))
    artifact_dir = os.path.join(artifact_root, uuid.uuid4().hex)
    os.makedirs(artifact_dir, exist_ok=True)
    return artifact_dir


def _estimate_animation_frame_count(animation):
    for attr_name in ("_save_count", "save_count"):
        try:
            value = getattr(animation, attr_name, None)
            if isinstance(value, int) and value >= 0:
                return value
        except Exception:
            pass

    try:
        saved = getattr(animation, "_save_seq", None)
        if isinstance(saved, (list, tuple)):
            return len(saved)
    except Exception:
        pass

    return None


def _snapshot_matplotlib_animation_ids(globals_dict):
    """记录执行前已经存在的 matplotlib 动画对象 id。"""
    try:
        if "matplotlib" not in sys.modules:
            return set()
        from matplotlib.animation import Animation
    except Exception:
        return set()

    animation_ids = set()
    for value in list(globals_dict.values()):
        try:
            if isinstance(value, Animation):
                animation_ids.add(id(value))
        except Exception:
            pass
    return animation_ids


def _iter_matplotlib_animations(globals_dict, previous_animation_ids=None, explicit_result_only=False):
    """从执行后的命名空间里收集本次 matplotlib 动画对象。

    设计说明：
    1. matplotlib 的 ``FuncAnimation`` / ``ArtistAnimation`` 必须保留一个强引用，
       否则会被垃圾回收而停止播放；用户代码通常会把它赋给一个变量（如 ``ani``）。
    2. 持久 session 会保留历史变量；如果无差别遍历全局命名空间，之前 cell 里的 ``ani``
       会在之后任意 cell 运行时被重复导出。
    3. 所以默认只收集“本次执行后新出现”的动画对象；如果本次末行表达式明确返回旧动画，
       调用方会设置 explicit_result_only，只捕获 ``_result`` 指向的对象。
    """
    animations = []
    previous_animation_ids = set(previous_animation_ids or set())
    try:
        if "matplotlib" not in sys.modules:
            return animations
        from matplotlib.animation import Animation
    except Exception:
        return animations

    seen_ids = set()

    def _consider(candidate, allow_existing=False):
        candidate_id = id(candidate)
        if not isinstance(candidate, Animation) or candidate_id in seen_ids:
            return
        if not allow_existing and candidate_id in previous_animation_ids:
            return
        seen_ids.add(candidate_id)
        animations.append(candidate)

    # 约定变量名优先，保证“末行表达式”动画排在最前面；_result 可显式指向历史动画。
    result_value = globals_dict.get("_result")
    try:
        _consider(result_value, allow_existing=True)
    except Exception:
        pass

    if explicit_result_only:
        return animations

    for preferred_name in ("ani", "anim", "animation"):
        try:
            _consider(globals_dict.get(preferred_name))
        except Exception:
            pass

    for value in list(globals_dict.values()):
        try:
            _consider(value)
        except Exception:
            pass

    return animations


def _capture_matplotlib_animation_html(globals_dict, previous_animation_ids=None, explicit_result_only=False):
    """把 matplotlib 动画保存为 HTML 工件（JS 动画播放器）。

    设计说明：
    1. 旧路径使用 ``to_jshtml()``，会把每一帧 base64 内嵌到字符串，再塞进 JSON 响应；
       300 帧 3D 动画很容易把 worker 响应做成超大字符串，甚至触发 Python 进程崩溃。
    2. ``HTMLWriter(embed_frames=False)`` 会把 HTML 和帧图片写到会话沙箱的工件目录，
       前端 iframe 直接加载 HTML 文件，既保留 2D/3D 动画交互控件，又避免 stdout/JSON 传输大块帧数据。
    3. 如果已经识别出动画但导出失败，返回 Markdown 错误模块而不是静默降级为静态图；
       这样用户能明确知道动画链路哪里坏了。
    """
    animations = _iter_matplotlib_animations(
        globals_dict,
        previous_animation_ids=previous_animation_ids,
        explicit_result_only=explicit_result_only,
    )
    if not animations:
        return None

    try:
        animation = animations[0]
        display_dir = _prepare_display_artifact_dir(globals_dict, "matplotlib-animation")
        html_path = os.path.join(display_dir, "animation.html")
        frame_dir = os.path.join(display_dir, "animation_frames")

        from matplotlib.animation import HTMLWriter

        fps = None
        interval = getattr(animation, "_interval", None)
        if interval:
            try:
                fps = 1000 / float(interval)
            except Exception:
                fps = None
        default_mode = "loop" if getattr(animation, "_repeat", False) else "once"
        writer = HTMLWriter(
            fps=fps or 30,
            embed_frames=False,
            default_mode=default_mode,
        )
        animation.save(html_path, writer=writer, progress_callback=None)
        if not os.path.exists(html_path):
            raise FileNotFoundError(f"Matplotlib animation HTML was not created: {html_path}")

        try:
            import matplotlib.pyplot as plt

            plt.close("all")
        except Exception:
            pass

        return {
            "type": "html",
            "mime": "text/html",
            "html": "",
            "path": html_path,
            "source_path": html_path,
            "artifact_path": html_path,
            "artifact_dir": display_dir,
            "frame_dir": frame_dir,
            "renderer": "matplotlib-htmlwriter",
            "frame_count": _estimate_animation_frame_count(animation),
        }
    except BaseException as exc:
        return {
            "type": "markdown",
            "mime": "text/markdown",
            "markdown": "Matplotlib 动画导出失败。\n\n```text\n" + _format_traceback(exc) + "\n```",
        }


def _ffmpeg_cache_dir():
    """返回一个可写的 ffmpeg 缓存目录（按需下载的静态二进制落在这里）。

    优先放在 python 运行时 venv 根目录下，便于 ``_resolve_ffmpeg_binary`` 复用；
    不可写时回退到系统临时目录。可用 ``GUAXI_FFMPEG_DIR`` 环境变量覆盖。
    """
    override = str(os.environ.get("GUAXI_FFMPEG_DIR") or "").strip()
    candidates = []
    if override:
        candidates.append(override)
    else:
        try:
            venv_root = os.path.dirname(os.path.dirname(sys.executable))
            candidates.append(os.path.join(venv_root, ".guaxi-ffmpeg"))
        except Exception:
            pass

    import tempfile

    candidates.append(os.path.join(tempfile.gettempdir(), "guaxi-ffmpeg"))

    for path in candidates:
        try:
            os.makedirs(path, exist_ok=True)
            probe = os.path.join(path, ".write-test")
            with open(probe, "w", encoding="utf-8") as handle:
                handle.write("ok")
            os.remove(probe)
            return path
        except Exception:
            continue

    fallback = os.path.join(tempfile.gettempdir(), "guaxi-ffmpeg")
    os.makedirs(fallback, exist_ok=True)
    return fallback


def _ffmpeg_download_sources():
    """返回 ``(url, kind)`` 候选列表，kind ∈ {'exe','zip'}；可用 env 覆盖。"""
    sources = []
    override = str(os.environ.get("GUAXI_FFMPEG_DOWNLOAD_URL") or "").strip()
    if override:
        sources.append((override, "zip" if override.lower().endswith(".zip") else "exe"))

    import platform

    system = platform.system().lower()
    is_64 = sys.maxsize > 2 ** 32

    if system == "windows":
        sources.append((
            "https://github.com/imageio/imageio-binaries/raw/master/ffmpeg/ffmpeg-win64-v4.2.2.exe",
            "exe",
        ))
        sources.append((
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
            "zip",
        ))
    elif system == "darwin":
        sources.append((
            "https://github.com/imageio/imageio-binaries/raw/master/ffmpeg/ffmpeg-osx64-v4.2.2",
            "exe",
        ))
    else:
        name = "ffmpeg-linux64-v4.2.2" if is_64 else "ffmpeg-linux32-v4.2.2"
        sources.append((
            f"https://github.com/imageio/imageio-binaries/raw/master/ffmpeg/{name}",
            "exe",
        ))
    return sources


def _verify_ffmpeg(path):
    try:
        result = subprocess.run(
            [path, "-version"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=20,
        )
        return result.returncode == 0
    except Exception:
        return False


def _download_ffmpeg_on_demand():
    """缺失 ffmpeg 时按需下载一个静态二进制到缓存目录，离线/失败则返回 None。

    可用 ``GUAXI_FFMPEG_NO_DOWNLOAD=1`` 关闭下载（强制走 gif 兜底）。
    """
    if str(os.environ.get("GUAXI_FFMPEG_NO_DOWNLOAD") or "").strip().lower() in ("1", "true", "yes"):
        return None

    import urllib.request

    dest_dir = _ffmpeg_cache_dir()
    exe_name = "ffmpeg.exe" if os.name == "nt" else "ffmpeg"
    final_path = os.path.join(dest_dir, exe_name)

    if os.path.exists(final_path) and _verify_ffmpeg(final_path):
        return final_path

    for url, kind in _ffmpeg_download_sources():
        tmp_path = os.path.join(dest_dir, "ffmpeg.download.tmp")
        try:
            request = urllib.request.Request(url, headers={"User-Agent": "guaxi-ffmpeg-fetch"})
            with urllib.request.urlopen(request, timeout=120) as response, open(tmp_path, "wb") as out:
                shutil.copyfileobj(response, out)

            if kind == "zip":
                import zipfile

                with zipfile.ZipFile(tmp_path) as archive:
                    member = next(
                        (
                            name for name in archive.namelist()
                            if name.replace("\\", "/").lower().endswith("bin/" + exe_name)
                            or os.path.basename(name).lower() == exe_name
                        ),
                        "",
                    )
                    if not member:
                        raise FileNotFoundError("ffmpeg not found in archive")
                    with archive.open(member) as src, open(final_path + ".part", "wb") as dst:
                        shutil.copyfileobj(src, dst)
                os.replace(final_path + ".part", final_path)
            else:
                os.replace(tmp_path, final_path)

            if os.name != "nt":
                try:
                    os.chmod(final_path, 0o755)
                except Exception:
                    pass

            if _verify_ffmpeg(final_path):
                return final_path
            try:
                os.remove(final_path)
            except Exception:
                pass
        except Exception:
            continue
        finally:
            try:
                if os.path.exists(tmp_path):
                    os.remove(tmp_path)
            except Exception:
                pass
    return None


def _ensure_ffmpeg_binary():
    """解析可用 ffmpeg；找不到时按需下载（保证 mp4 在有网时一定可用）。"""
    cached = getattr(_ensure_ffmpeg_binary, "_cache", None)
    if cached is not None:
        return cached or None

    resolved = _resolve_ffmpeg_binary()
    if not resolved:
        try:
            resolved = _download_ffmpeg_on_demand()
        except Exception:
            resolved = None
        if resolved:
            os.environ.setdefault("IMAGEIO_FFMPEG_EXE", resolved)
            os.environ.setdefault("FFMPEG_BINARY", resolved)
            # 修正 _resolve_ffmpeg_binary 之前缓存的“未找到”结果，使后续调用命中。
            _resolve_ffmpeg_binary._cache = resolved

    _ensure_ffmpeg_binary._cache = resolved or ""
    return resolved or None


def _resolve_ffmpeg_binary():
    """解析一个可用的 ffmpeg 可执行文件路径，不依赖用户系统是否安装。

    查找顺序：
    1. 随 Python 运行时打包的 ffmpeg（与当前解释器同级的 Scripts 目录）。
    2. ``imageio-ffmpeg`` 自带的静态二进制（构建时随依赖引入即可，无需系统安装）。
    3. 系统 PATH 兜底。
    全部失败返回 None；调用方会改用 Pillow 直接导出 GIF（无需 ffmpeg），
    因此“没有 ffmpeg”不会导致动画无法显示，只是退化为 GIF。
    """
    cached = getattr(_resolve_ffmpeg_binary, "_cache", None)
    if cached is not None:
        return cached or None

    import shutil

    exe_name = "ffmpeg.exe" if os.name == "nt" else "ffmpeg"
    candidates = []
    try:
        interpreter_dir = os.path.dirname(sys.executable)
        candidates.append(os.path.join(interpreter_dir, exe_name))
        candidates.append(os.path.join(interpreter_dir, "ffmpeg", exe_name))
    except Exception:
        pass

    # 按需下载缓存目录（缺失时下载的静态 ffmpeg 落在这里，下次直接命中）。
    try:
        candidates.append(os.path.join(_ffmpeg_cache_dir(), exe_name))
    except Exception:
        pass

    try:
        import imageio_ffmpeg

        candidates.append(imageio_ffmpeg.get_ffmpeg_exe())
    except Exception:
        pass

    try:
        found = shutil.which("ffmpeg")
        if found:
            candidates.append(found)
    except Exception:
        pass

    resolved = ""
    for candidate in candidates:
        try:
            if candidate and os.path.exists(candidate):
                resolved = candidate
                break
        except Exception:
            pass

    if resolved:
        # 同步给 moviepy / imageio，使其在无系统 ffmpeg 时也能复用同一个二进制。
        os.environ.setdefault("IMAGEIO_FFMPEG_EXE", resolved)
        os.environ.setdefault("FFMPEG_BINARY", resolved)

    _resolve_ffmpeg_binary._cache = resolved
    return resolved or None


def _animation_default_fps(animation, fallback=30):
    interval = getattr(animation, "_interval", None)
    if interval:
        try:
            value = 1000.0 / float(interval)
            if value > 0:
                return value
        except Exception:
            pass
    return fallback


def _build_media_display(path, mime, renderer, display_dir=None, frame_count=None):
    """统一构造媒体输出模块（前端用原生 <video>/<img> 渲染，不依赖内联脚本）。"""
    is_video = str(mime or "").startswith("video/")
    display = {
        "type": "video" if is_video else "image",
        "mime": mime,
        "path": path,
        "source_path": path,
        "artifact_path": path,
        "renderer": renderer,
    }
    if display_dir:
        display["artifact_dir"] = display_dir
    if frame_count is not None:
        display["frame_count"] = frame_count
    return display


def _export_matplotlib_animation_media(animation, display_dir):
    """把 matplotlib 动画导出为媒体文件：优先 mp4(ffmpeg)，无 ffmpeg 时回退 gif(Pillow)。

    返回 ``(path, mime, renderer)``。GIF 路径只依赖 Pillow，保证“零 ffmpeg”也能出图。
    """
    fps = _animation_default_fps(animation)
    safe_fps = max(1, int(round(fps)))
    ffmpeg_path = _ensure_ffmpeg_binary()

    if ffmpeg_path:
        try:
            import matplotlib

            matplotlib.rcParams["animation.ffmpeg_path"] = ffmpeg_path
            from matplotlib.animation import FFMpegWriter

            mp4_path = os.path.join(display_dir, "animation.mp4")
            writer = FFMpegWriter(fps=safe_fps, bitrate=-1)
            animation.save(mp4_path, writer=writer, progress_callback=None)
            if os.path.exists(mp4_path) and os.path.getsize(mp4_path) > 0:
                return mp4_path, "video/mp4", "matplotlib-ffmpeg"
        except BaseException:
            # 任何 ffmpeg 失败都落到 GIF 兜底，保证动画仍能显示。
            pass

    from matplotlib.animation import PillowWriter

    gif_path = os.path.join(display_dir, "animation.gif")
    animation.save(gif_path, writer=PillowWriter(fps=safe_fps), progress_callback=None)
    if not (os.path.exists(gif_path) and os.path.getsize(gif_path) > 0):
        raise FileNotFoundError(f"Matplotlib animation media was not created: {gif_path}")
    return gif_path, "image/gif", "matplotlib-pillow"


def _capture_matplotlib_animation_display(globals_dict, previous_animation_ids=None, explicit_result_only=False):
    """matplotlib 动画生产者：导出为 mp4/gif 媒体文件。"""
    animations = _iter_matplotlib_animations(
        globals_dict,
        previous_animation_ids=previous_animation_ids,
        explicit_result_only=explicit_result_only,
    )
    if not animations:
        return None

    try:
        animation = animations[0]
        display_dir = _prepare_display_artifact_dir(globals_dict, "matplotlib-animation")
        path, mime, renderer = _export_matplotlib_animation_media(animation, display_dir)
        try:
            import matplotlib.pyplot as plt

            plt.close("all")
        except Exception:
            pass
        return _build_media_display(
            path,
            mime,
            renderer,
            display_dir=display_dir,
            frame_count=_estimate_animation_frame_count(animation),
        )
    except BaseException:
        # 媒体导出彻底失败时，回退到旧的 HTMLWriter 交互播放器（仅作为最后兜底）。
        return _capture_matplotlib_animation_html(
            globals_dict,
            previous_animation_ids=previous_animation_ids,
            explicit_result_only=explicit_result_only,
        )


def _capture_moviepy_animation_display(globals_dict):
    """moviepy 生产者：把 VideoClip 导出为 mp4（无 ffmpeg 时回退 gif）。"""
    if "moviepy" not in sys.modules:
        return None
    try:
        from moviepy.video.VideoClip import VideoClip
    except Exception:
        return None

    clip = None
    candidate = globals_dict.get("_result")
    if isinstance(candidate, VideoClip):
        clip = candidate
    else:
        for name in ("clip", "video", "animation", "ani", "anim"):
            value = globals_dict.get(name)
            if isinstance(value, VideoClip):
                clip = value
                break
    if clip is None:
        return None

    try:
        display_dir = _prepare_display_artifact_dir(globals_dict, "moviepy-animation")
        ffmpeg_path = _ensure_ffmpeg_binary()
        if ffmpeg_path:
            try:
                mp4_path = os.path.join(display_dir, "animation.mp4")
                clip.write_videofile(mp4_path, codec="libx264", audio=False, logger=None)
                if os.path.exists(mp4_path) and os.path.getsize(mp4_path) > 0:
                    return _build_media_display(mp4_path, "video/mp4", "moviepy-ffmpeg", display_dir=display_dir)
            except BaseException:
                pass
        gif_path = os.path.join(display_dir, "animation.gif")
        clip.write_gif(gif_path, logger=None)
        if os.path.exists(gif_path) and os.path.getsize(gif_path) > 0:
            return _build_media_display(gif_path, "image/gif", "moviepy-gif", display_dir=display_dir)
        raise FileNotFoundError("moviepy animation media was not created")
    except BaseException as exc:
        return {
            "type": "markdown",
            "mime": "text/markdown",
            "markdown": "moviepy 动画导出失败。\n\n```text\n" + _format_traceback(exc) + "\n```",
        }


_ANIMATION_MEDIA_EXTS = {
    ".mp4": "video/mp4",
    ".webm": "video/webm",
    ".mov": "video/quicktime",
    ".m4v": "video/x-m4v",
    ".ogv": "video/ogg",
    ".gif": "image/gif",
    ".apng": "image/apng",
}


def _capture_media_filepath_display(globals_dict):
    """通用媒体文件生产者：用户代码（含 manim 等）产出的 mp4/webm/gif 文件路径直接透传。

    约定：把生成的媒体文件路径作为末行表达式返回，或赋值给
    ``video_path`` / ``animation_path`` / ``output_path`` / ``media_path``。
    文件会被复制进会话沙箱工件目录，确保位于前端 asset 协议作用域内可加载。
    """
    raw_paths = []
    candidate = globals_dict.get("_result")
    if isinstance(candidate, (str, os.PathLike)):
        raw_paths.append(str(candidate))
    for name in ("video_path", "animation_path", "output_path", "media_path"):
        value = globals_dict.get(name)
        if isinstance(value, (str, os.PathLike)):
            raw_paths.append(str(value))

    for raw in raw_paths:
        try:
            path = str(raw or "").strip().strip('"').strip("'")
            if not path:
                continue
            ext = os.path.splitext(path)[1].lower()
            mime = _ANIMATION_MEDIA_EXTS.get(ext)
            if not mime or not os.path.isfile(path):
                continue
            import shutil

            display_dir = _prepare_display_artifact_dir(globals_dict, "media-file")
            dest = os.path.join(display_dir, "animation" + ext)
            shutil.copyfile(path, dest)
            renderer = "manim" if "manim" in sys.modules else "media-file"
            return _build_media_display(dest, mime, renderer, display_dir=display_dir)
        except Exception:
            continue
    return None


def _capture_cell_animation_display(globals_dict, previous_animation_ids=None, explicit_result_only=False):
    """通用动画捕获框架：按生产者顺序探测，返回首个命中的媒体输出（video/image）。

    设计说明：
    1. 统一产出“媒体文件 + 原生 <video>/<img>”而非 iframe 内联脚本播放器：
       既高性能，又不受 release CSP / iframe sandbox 限制，debug 与 release 行为一致。
    2. 生产者可插拔：matplotlib / moviepy / 已生成媒体文件路径(含 manim 产物)。
       后续接入新动画类型只需新增一个 _capture_* 生产者并登记到这里。
    3. 没有任何动画时返回 None，调用方继续走静态图等既有路径。
    """
    producers = (
        lambda: _capture_matplotlib_animation_display(
            globals_dict,
            previous_animation_ids=previous_animation_ids,
            explicit_result_only=explicit_result_only,
        ),
        lambda: _capture_moviepy_animation_display(globals_dict),
        lambda: _capture_media_filepath_display(globals_dict),
    )
    for produce in producers:
        try:
            display = produce()
        except BaseException:
            display = None
        if display:
            return display
    return None


def _maybe_prepare_inline_matplotlib():
    try:
        if "matplotlib" not in sys.modules:
            import matplotlib

            matplotlib.use("Agg")
    except Exception:
        pass


def _build_dataframe_payload(df, source_path=None):
    pandas_module = sys.modules.get("pandas")
    if pandas_module is None:
        return None

    if not isinstance(df, pandas_module.DataFrame):
        return None

    # Design notes:
    # 1. The frontend session/output panels now need the full table body so
    #    imported tables and result dataframes can be scrolled instead of being
    #    silently truncated to a short preview.
    # 2. We still normalize missing values to None so JSON transport remains
    #    stable across pandas dtypes.
    preview_df = df.astype(object).where(pandas_module.notnull(df), None)
    rows = []
    for row in preview_df.values.tolist():
        rows.append([_normalize_basic(item) for item in row])

    return {
        "columns": [str(col) for col in df.columns.tolist()],
        "rows": rows,
        "row_count": int(len(df.index)),
        "truncated": False,
        "dtypes": {str(k): str(v) for k, v in df.dtypes.astype(str).to_dict().items()},
        "source_path": source_path or "",
    }


def _normalize_table_key(value, fallback="table"):
    text = str(value or "").strip()
    if not text:
        text = fallback
    text = re.sub(r"[^0-9A-Za-z_\-.]+", "_", text).strip("._")
    return text or fallback


def _normalize_table_display_name(value, fallback="Table"):
    text = str(value or "").strip()
    return text or fallback


def _normalize_table_rows_payload(columns, rows):
    normalized_rows = []
    for row in rows:
        normalized_row = []
        if isinstance(row, dict):
            for column in columns:
                normalized_row.append(_normalize_basic(row.get(column)))
        else:
            raw_row = row if isinstance(row, (list, tuple)) else []
            for index in range(len(columns)):
                normalized_row.append(_normalize_basic(raw_row[index] if index < len(raw_row) else None))
        normalized_rows.append(normalized_row)
    return normalized_rows


def _normalize_records_table(value):
    if not isinstance(value, list):
        return None
    if value and not isinstance(value[0], dict):
        return None

    columns = []
    for item in value:
        if not isinstance(item, dict):
            return None
        for key in item.keys():
            key_text = str(key)
            if key_text not in columns:
                columns.append(key_text)

    return {
        "columns": columns,
        "rows": _normalize_table_rows_payload(columns, value),
        "row_count": len(value),
        "dtypes": {str(column): "json" for column in columns},
        "kind": "records",
    }


def _build_remote_table_payload(
    *,
    handle,
    manifest_path,
    artifact_path,
    session_id,
    table_key,
    table_name,
    columns,
    row_count,
    dtypes,
    source_path,
    table_count,
    table_kind,
):
    return {
        "columns": [str(col) for col in columns],
        "rows": [],
        "row_count": int(row_count),
        "truncated": False,
        "dtypes": {str(k): str(v) for k, v in (dtypes or {}).items()},
        "source_path": source_path or "",
        "result_handle": handle,
        "manifest_path": manifest_path,
        "artifact_path": artifact_path,
        "session_id": session_id,
        "table_key": table_key,
        "table_name": table_name,
        "table_kind": table_kind,
        "table_count": int(table_count),
        "remote": True,
    }


def _prepare_table_artifact_dir(session_id, globals_dict, handle):
    sandbox_dir = _prepare_session_dir(session_id, globals_dict)
    if not sandbox_dir:
        return ""

    artifact_dir = os.path.join(sandbox_dir, ".guaxi-table-artifacts", handle)
    try:
        os.makedirs(artifact_dir, exist_ok=True)
        return artifact_dir
    except Exception as exc:
        print(f"[GuaxiPythonWorker] prepare table artifact dir failed: {exc}", file=sys.stderr)
        return ""


def _records_for_artifact(columns, rows):
    records = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        record = {}
        for column in columns:
            record[column] = _normalize_basic(row.get(column))
        records.append(record)
    return records


def _write_table_artifact(*, artifact_dir, table_key, table_kind, value, columns):
    if not artifact_dir:
        return ""

    if table_kind == "dataframe":
        artifact_path = os.path.join(artifact_dir, f"{table_key}.pkl")
        value.to_pickle(artifact_path)
        return artifact_path

    artifact_path = os.path.join(artifact_dir, f"{table_key}.json")
    with open(artifact_path, "w", encoding="utf-8") as handle_file:
        json.dump(_records_for_artifact(columns, value if isinstance(value, list) else []), handle_file, ensure_ascii=False)
    return artifact_path


def _extract_table_bundle(result, globals_dict):
    pandas_module = sys.modules.get("pandas")
    source_path = str(globals_dict.get("__guaxi_table_path") or "").strip()
    session_id = str(globals_dict.get("_guaxi_session") or "").strip()
    table_specs = []

    if pandas_module is not None and isinstance(result, pandas_module.DataFrame):
        table_specs.append({
            "table_key": "table",
            "table_name": "Table",
            "kind": "dataframe",
            "value": result,
        })
    elif pandas_module is not None and isinstance(result, dict) and result:
        if all(isinstance(value, pandas_module.DataFrame) for value in result.values()):
            for index, (name, value) in enumerate(result.items(), start=1):
                table_specs.append({
                    "table_key": _normalize_table_key(name, f"sheet_{index}"),
                    "table_name": _normalize_table_display_name(name, f"Sheet {index}"),
                    "kind": "dataframe",
                    "value": value,
                })
        elif all(_normalize_records_table(value) is not None for value in result.values()):
            for index, (name, value) in enumerate(result.items(), start=1):
                table_specs.append({
                    "table_key": _normalize_table_key(name, f"table_{index}"),
                    "table_name": _normalize_table_display_name(name, f"Table {index}"),
                    "kind": "records",
                    "value": value,
                })
    elif pandas_module is not None and isinstance(result, (list, tuple)) and result and all(
        isinstance(value, pandas_module.DataFrame) for value in result
    ):
        for index, value in enumerate(result, start=1):
            table_specs.append({
                "table_key": f"table_{index}",
                "table_name": f"Table {index}",
                "kind": "dataframe",
                "value": value,
            })
    else:
        normalized_records = _normalize_records_table(result)
        if normalized_records is not None:
            table_specs.append({
                "table_key": "table",
                "table_name": "Table",
                "kind": "records",
                "value": result,
            })

    if not table_specs:
        return None

    handle = f"tbl_{uuid.uuid4().hex}"
    artifact_dir = _prepare_table_artifact_dir(session_id, globals_dict, handle)
    manifest_path = os.path.join(artifact_dir, "manifest.json") if artifact_dir else ""
    table_payloads = []
    registry_tables = {}
    manifest_tables = []

    for spec in table_specs:
        table_key = _normalize_table_key(spec.get("table_key"), "table")
        table_name = _normalize_table_display_name(spec.get("table_name"), "Table")
        table_kind = str(spec.get("kind") or "").strip() or "records"
        artifact_path = ""

        if table_kind == "dataframe":
            df = spec.get("value")
            columns = [str(col) for col in df.columns.tolist()]
            row_count = int(len(df.index))
            dtypes = {str(k): str(v) for k, v in df.dtypes.astype(str).to_dict().items()}
            try:
                artifact_path = _write_table_artifact(
                    artifact_dir=artifact_dir,
                    table_key=table_key,
                    table_kind=table_kind,
                    value=df,
                    columns=columns,
                )
            except Exception as exc:
                artifact_path = ""
                print(f"[GuaxiPythonWorker] persist dataframe artifact failed: {exc}", file=sys.stderr)
            registry_tables[table_key] = {
                "kind": "dataframe",
                "value": df,
                "columns": columns,
                "row_count": row_count,
                "dtypes": dtypes,
                "artifact_path": artifact_path,
                "table_name": table_name,
            }
        else:
            normalized = _normalize_records_table(spec.get("value")) or {
                "columns": [],
                "rows": [],
                "row_count": 0,
                "dtypes": {},
                "kind": "records",
            }
            columns = normalized["columns"]
            row_count = int(normalized["row_count"])
            dtypes = normalized["dtypes"]
            try:
                artifact_path = _write_table_artifact(
                    artifact_dir=artifact_dir,
                    table_key=table_key,
                    table_kind=table_kind,
                    value=spec.get("value"),
                    columns=columns,
                )
            except Exception as exc:
                artifact_path = ""
                print(f"[GuaxiPythonWorker] persist records artifact failed: {exc}", file=sys.stderr)
            registry_tables[table_key] = {
                "kind": "records",
                "value": spec.get("value"),
                "columns": columns,
                "row_count": row_count,
                "dtypes": dtypes,
                "artifact_path": artifact_path,
                "table_name": table_name,
            }

        manifest_entry = {
            "table_key": table_key,
            "table_name": table_name,
            "table_kind": table_kind,
            "artifact_path": artifact_path,
            "columns": columns,
            "row_count": row_count,
            "dtypes": dtypes,
            "source_path": source_path,
        }
        manifest_tables.append(manifest_entry)
        table_payloads.append(_build_remote_table_payload(
            handle=handle,
            manifest_path=manifest_path,
            artifact_path=artifact_path,
            session_id=session_id,
            table_key=table_key,
            table_name=table_name,
            columns=columns,
            row_count=row_count,
            dtypes=dtypes,
            source_path=source_path,
            table_count=len(table_specs),
            table_kind=table_kind,
        ))

    manifest = {
        "handle": handle,
        "manifest_path": manifest_path,
        "table_count": len(manifest_tables),
        "source_path": source_path,
        "tables": manifest_tables,
    }

    if manifest_path:
        try:
            with open(manifest_path, "w", encoding="utf-8") as handle_file:
                json.dump(manifest, handle_file, ensure_ascii=False)
        except Exception as exc:
            print(f"[GuaxiPythonWorker] persist table manifest failed: {exc}", file=sys.stderr)
            manifest_path = ""
            manifest["manifest_path"] = ""
            for payload in table_payloads:
                payload["manifest_path"] = ""

    TABLE_HANDLES[handle] = {
        "manifest": manifest,
        "manifest_path": manifest_path,
        "tables": registry_tables,
    }

    return {
        "handle": handle,
        "manifest_path": manifest_path,
        "table_count": len(manifest_tables),
        "tables": table_payloads,
        "source_path": source_path,
    }


def _load_table_bundle(handle="", manifest_path=""):
    normalized_handle = str(handle or "").strip()
    if normalized_handle and normalized_handle in TABLE_HANDLES:
        return TABLE_HANDLES[normalized_handle]

    resolved_manifest_path = str(manifest_path or "").strip()
    if not resolved_manifest_path and normalized_handle:
        cached = TABLE_HANDLES.get(normalized_handle)
        if cached:
            resolved_manifest_path = str(cached.get("manifest_path") or "").strip()

    if not resolved_manifest_path or not os.path.exists(resolved_manifest_path):
        return None

    with open(resolved_manifest_path, "r", encoding="utf-8") as handle_file:
        manifest = json.load(handle_file)

    bundle = {
        "manifest": manifest,
        "manifest_path": resolved_manifest_path,
        "tables": {},
    }
    if normalized_handle:
        TABLE_HANDLES[normalized_handle] = bundle
    return bundle


def _load_table_entry_value(entry):
    if "value" in entry:
        return entry["value"]

    artifact_path = str(entry.get("artifact_path") or "").strip()
    table_kind = str(entry.get("kind") or entry.get("table_kind") or "").strip() or "records"
    if not artifact_path or not os.path.exists(artifact_path):
        raise FileNotFoundError(f"Table artifact not found: {artifact_path}")

    if table_kind == "dataframe":
        try:
            import pandas as pd
        except Exception as exc:
            raise RuntimeError(f"Loading dataframe artifact requires pandas: {exc}") from exc
        value = pd.read_pickle(artifact_path)
    else:
        with open(artifact_path, "r", encoding="utf-8") as handle_file:
            value = json.load(handle_file)

    entry["value"] = value
    return value


def _resolve_table_entry(payload):
    handle = str(payload.get("handle") or "").strip()
    manifest_path = str(payload.get("manifest_path") or "").strip()
    table_key = _normalize_table_key(payload.get("table_key"), "table")

    bundle = _load_table_bundle(handle=handle, manifest_path=manifest_path)
    if not bundle:
        raise ValueError("Table handle or manifest is missing")

    manifest = bundle.get("manifest") or {}
    manifest_tables = manifest.get("tables") or []
    manifest_entry = next(
        (item for item in manifest_tables if _normalize_table_key(item.get("table_key"), "table") == table_key),
        None,
    )
    if manifest_entry is None and manifest_tables:
        manifest_entry = manifest_tables[0]
        table_key = _normalize_table_key(manifest_entry.get("table_key"), "table")
    if manifest_entry is None:
        raise ValueError(f"Table entry not found: {table_key}")

    table_entry = bundle.setdefault("tables", {}).get(table_key)
    if table_entry is None:
        table_entry = {
            "kind": str(manifest_entry.get("table_kind") or "records"),
            "artifact_path": str(manifest_entry.get("artifact_path") or ""),
            "table_name": str(manifest_entry.get("table_name") or table_key),
            "columns": [str(col) for col in manifest_entry.get("columns") or []],
            "row_count": int(manifest_entry.get("row_count") or 0),
            "dtypes": manifest_entry.get("dtypes") or {},
        }
        bundle["tables"][table_key] = table_entry

    return bundle, manifest, table_key, manifest_entry, table_entry


def _dataframe_rows_slice(df, offset, limit):
    if limit <= 0:
        return []
    sliced = df.iloc[offset : offset + limit]
    preview_df = sliced.astype(object).where(sliced.notnull(), None)
    return [[_normalize_basic(item) for item in row] for row in preview_df.values.tolist()]


def _records_rows_slice(records, columns, offset, limit):
    if limit <= 0:
        return []
    sliced = records[offset : offset + limit]
    return _normalize_table_rows_payload(columns, sliced)


def _table_to_records(entry):
    value = _load_table_entry_value(entry)
    if str(entry.get("kind") or "").strip() == "dataframe":
        pandas_module = sys.modules.get("pandas")
        if pandas_module is None:
            raise RuntimeError("Exporting dataframe artifacts requires pandas")
        preview_df = value.astype(object).where(pandas_module.notnull(value), None)
        columns = [str(col) for col in value.columns.tolist()]
        rows = preview_df.to_dict(orient="records")
        return columns, rows

    normalized = _normalize_records_table(value) or {
        "columns": [],
        "rows": [],
    }
    rows = []
    for raw_row in value:
        row = {}
        for index, column in enumerate(normalized["columns"]):
            row[column] = raw_row.get(column) if isinstance(raw_row, dict) else None
        rows.append(row)
    return normalized["columns"], rows


def _sheet_name_for_export(value, fallback):
    normalized = _normalize_table_display_name(value, fallback)
    normalized = re.sub(r"[\[\]:*?/\\]+", "_", normalized).strip()
    return normalized[:31] or fallback[:31] or "Sheet1"


def _capture_rich_display(result):
    if result is None:
        return None

    display_methods = (
        ("_repr_html_", "html", "text/html", "html"),
        ("_repr_markdown_", "markdown", "text/markdown", "markdown"),
        ("_repr_svg_", "svg", "image/svg+xml", "svg"),
    )

    for method_name, display_type, mime, field in display_methods:
        method = getattr(result, method_name, None)
        if not callable(method):
            continue
        try:
            rendered = method()
        except Exception:
            continue
        if isinstance(rendered, str) and rendered.strip():
            return {
                "type": display_type,
                "mime": mime,
                field: rendered,
            }

    return None


def _build_plotly_display_payload(fig):
    try:
        import plotly.io as pio
    except Exception:
        return None

    try:
        html = pio.to_html(
            fig,
            # 设计说明：
            # 探索模式里的 Plotly 输出需要在 notebook 内独立可渲染，
            # 不能依赖外部浏览器或在线 CDN，否则很容易只剩一个空白容器。
            # 这里改成把 plotly.js 内联进输出 HTML，配合前端 iframe/srcdoc 稳定渲染。
            include_plotlyjs=True,
            full_html=False,
            config={"responsive": True},
        )
    except Exception:
        return None

    if not isinstance(html, str) or not html.strip():
        return None

    return {
        "type": "html",
        "mime": "text/html",
        "html": html,
    }


def _install_plotly_notebook_hook(globals_dict):
    try:
        import plotly.io as pio
        from plotly.basedatatypes import BaseFigure
    except Exception:
        return

    if globals_dict.get("__guaxi_plotly_hook_installed__"):
        return

    original_show = getattr(pio, "show", None)
    original_base_figure_show = getattr(BaseFigure, "show", None)

    def _capture_and_store(fig):
        payload = _build_plotly_display_payload(fig)
        if payload:
            globals_dict["__guaxi_plotly_display__"] = payload
        return payload

    def _guaxi_plotly_show(fig, *args, **kwargs):
        payload = _capture_and_store(fig)
        if payload is not None:
            return None
        if callable(original_show):
            return original_show(fig, *args, **kwargs)
        return None

    def _guaxi_base_figure_show(self, *args, **kwargs):
        payload = _capture_and_store(self)
        if payload is not None:
            return None
        if callable(original_base_figure_show):
            return original_base_figure_show(self, *args, **kwargs)
        return None

    # 设计说明：
    # 探索模式里的 Plotly 应该像 notebook 一样把交互图渲染回当前 cell，
    # 而不是走默认浏览器 renderer 弹到外部页面。
    pio.show = _guaxi_plotly_show
    BaseFigure.show = _guaxi_base_figure_show
    globals_dict["__guaxi_plotly_hook_installed__"] = True


def _fresh_globals():
    return {"__builtins__": __builtins__}


def _get_session_globals(session_id, persist):
    sid = str(session_id or "").strip()
    if not persist:
        return _fresh_globals(), sid
    if not sid:
        sid = "__guaxi_global__"
    if sid not in SESSIONS:
        SESSIONS[sid] = _fresh_globals()
    return SESSIONS[sid], sid


def _prepare_session_dir(session_id, globals_dict):
    sandbox_dir = str(globals_dict.get("_guaxi_sandbox_dir") or "").strip()
    if not session_id or not sandbox_dir:
        return ""

    try:
        os.makedirs(sandbox_dir, exist_ok=True)
    except Exception:
        return ""

    SESSION_DIRS[session_id] = sandbox_dir
    globals_dict["_guaxi_sandbox_dir"] = sandbox_dir
    globals_dict["_guaxi_cwd"] = sandbox_dir
    return sandbox_dir


def _shift_last_results(globals_dict, session_id):
    history = RESULT_HISTORY.setdefault(session_id or "__guaxi_global__", [])
    value = globals_dict.get("_result")
    if value is None:
        return
    history.insert(0, value)
    del history[3:]
    if len(history) > 0:
        globals_dict["_"] = history[0]
    if len(history) > 1:
        globals_dict["__"] = history[1]
    if len(history) > 2:
        globals_dict["___"] = history[2]


def _format_traceback(exc):
    try:
        return "".join(traceback.format_exception(exc))
    except Exception:
        return str(exc)


def _run_pip(args):
    proc = subprocess.run(
        [sys.executable, "-m", "pip", *args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return {
        "ok": proc.returncode == 0,
        "stdout": proc.stdout or "",
        "stderr": proc.stderr or "",
        "result_repr": None,
        "result_json": None,
    }


def _handle_magic(code, globals_dict, session_id, include_session_vars=True, session_var_names=None):
    stripped = str(code or "").strip()
    if not stripped.startswith("%"):
        return None

    if stripped == "%reset":
        sid = session_id or "__guaxi_global__"
        SESSIONS[sid] = _fresh_globals()
        RESULT_HISTORY.pop(sid, None)
        return {
            "ok": True,
            "stdout": "Session reset.",
            "stderr": "",
            "result_repr": None,
            "result_json": None,
            "session_id": sid,
            "session_vars": [],
        }

    if stripped in ("%who", "%whos"):
        names = sorted(
            key
            for key in globals_dict.keys()
            if not str(key).startswith("__") and key not in {"_", "__", "___", "_result"}
        )
        return {
            "ok": True,
            "stdout": "\n".join(names)
            if stripped == "%who"
            else "\n".join(f"{name}: {type(globals_dict.get(name)).__name__}" for name in names),
            "stderr": "",
            "result_repr": None,
            "result_json": None,
            "session_id": session_id or "__guaxi_global__",
            "session_vars": _snapshot_session_vars(globals_dict, session_var_names) if include_session_vars else [],
        }

    if stripped.startswith("%pip "):
        return _run_pip(stripped[len("%pip ") :].split())

    return {
        "ok": False,
        "stdout": "",
        "stderr": f"Unsupported magic command: {stripped}",
        "result_repr": None,
        "result_json": None,
        "session_id": session_id or "__guaxi_global__",
        "session_vars": _snapshot_session_vars(globals_dict, session_var_names) if include_session_vars else [],
    }


def _handle_exec(payload):
    code = _normalize_exec_code(payload.get("code"))
    persist = bool(payload.get("persist"))
    expect_json = bool(payload.get("expect_json"))
    include_session_vars = bool(payload.get("include_session_vars", True))
    session_var_names = _normalize_session_var_names(payload.get("session_var_names"))
    vars_map = payload.get("vars") or {}
    session_id = str(vars_map.get("_guaxi_session") or "").strip()

    globals_dict, resolved_session_id = _get_session_globals(session_id, persist)
    globals_dict.update(vars_map)
    sandbox_dir = _prepare_session_dir(resolved_session_id, globals_dict)

    magic_res = _handle_magic(
        code,
        globals_dict,
        resolved_session_id,
        include_session_vars=include_session_vars,
        session_var_names=session_var_names,
    )
    if magic_res is not None:
        return magic_res

    if vars_map.get("_guaxi_inline_matplotlib", None) is True or "_py_cell_id" in vars_map or "_guaxi_session" in vars_map:
        _maybe_prepare_inline_matplotlib()
        _install_plotly_notebook_hook(globals_dict)

    stdout_buffer = io.StringIO()
    stderr_buffer = io.StringIO()
    old_stdout, old_stderr = sys.stdout, sys.stderr
    sys.stdout, sys.stderr = stdout_buffer, stderr_buffer

    ok = True
    err_trace = ""
    result_repr = None
    result_json = None
    old_cwd = None
    dataflow_before = _snapshot_session_var_map(globals_dict)
    matplotlib_animation_ids_before = _snapshot_matplotlib_animation_ids(globals_dict)

    try:
        if sandbox_dir:
            old_cwd = os.getcwd()
            os.chdir(sandbox_dir)

        if "\0" in code:
            raise ValueError("Python code contains NUL byte (\\0), cannot execute")

        # 设计说明：
        # 1. `_result` / `__guaxi_last_value` 是 worker 内部为“本次执行结果提取”准备的临时槽位，不应跨 cell 执行残留。
        # 2. 如果这里不提前清掉，上一次导入表格或表达式求值留下的 `_result` 会被下一次普通执行重复当成当前输出，
        #    前端就会误以为“别的 Python cell 的表格也在当前 cell 里输出了”。
        # 3. 真正需要跨 cell 复用的数据应该由用户显式命名变量保存，而不是依赖内部 `_result` 缓存。
        globals_dict.pop("_result", None)
        globals_dict.pop("__guaxi_last_value", None)

        tree = ast.parse(code, mode="exec")
        has_last_expr = bool(tree.body) and isinstance(tree.body[-1], ast.Expr)
        if has_last_expr:
            last_expr = tree.body.pop().value
            tree.body.append(
                ast.Assign(
                    targets=[ast.Name(id="__guaxi_last_value", ctx=ast.Store())],
                    value=last_expr,
                )
            )
            ast.fix_missing_locations(tree)

        exec(compile(tree, "<guaxi-worker>", "exec"), globals_dict, globals_dict)
        _configure_verbose_object_rendering(globals_dict)

        if has_last_expr and "__guaxi_last_value" in globals_dict and "_result" not in globals_dict:
            globals_dict["_result"] = globals_dict.get("__guaxi_last_value")

        globals_dict.pop("__guaxi_last_value", None)

        if "_result" in globals_dict:
            result = globals_dict.get("_result")
            if result is not None:
                result_repr = _compact_preview(result)

                if expect_json:
                    result_json = _json_basic(result)

                table_bundle = _extract_table_bundle(result, globals_dict)
                if table_bundle:
                    table_payloads = table_bundle.get("tables") or []
                    next_table_json = {
                        "__guaxi_tables__": table_payloads,
                        "__guaxi_table_bundle__": {
                            "handle": table_bundle.get("handle"),
                            "manifest_path": table_bundle.get("manifest_path"),
                            "table_count": table_bundle.get("table_count"),
                            "source_path": table_bundle.get("source_path") or "",
                        },
                    }
                    if table_payloads:
                        next_table_json["__guaxi_dataframe__"] = table_payloads[0]

                    if isinstance(result_json, dict):
                        result_json.setdefault("__guaxi_dataframe__", next_table_json.get("__guaxi_dataframe__"))
                        result_json["__guaxi_tables__"] = table_payloads
                        result_json["__guaxi_table_bundle__"] = next_table_json["__guaxi_table_bundle__"]
                    elif result_json is None:
                        result_json = next_table_json
                    else:
                        next_table_json["__guaxi_result__"] = result_json
                        result_json = next_table_json
                else:
                    rich_display = _capture_rich_display(result)
                    if rich_display:
                        if isinstance(result_json, dict):
                            result_json.setdefault("__guaxi_display__", rich_display)
                        elif result_json is None:
                            result_json = {"__guaxi_display__": rich_display}
                        else:
                            result_json = {
                                "__guaxi_display__": rich_display,
                                "__guaxi_result__": result_json,
                            }
                    elif result_json is not None and not isinstance(result_json, dict):
                        result_json = {
                            "__guaxi_result__": result_json,
                        }

        plotly_display = globals_dict.pop("__guaxi_plotly_display__", None)
        if plotly_display:
            if isinstance(result_json, dict):
                result_json.setdefault("__guaxi_display__", plotly_display)
            elif result_json is None:
                result_json = {"__guaxi_display__": plotly_display}
            else:
                result_json = {
                    "__guaxi_display__": plotly_display,
                    "__guaxi_result__": result_json,
                }

        # 设计说明：富媒体输出按“动画优先于静态图”的顺序捕获。
        # 1. 通用动画框架优先把 matplotlib / moviepy / manim 等动画导出为 mp4/gif 媒体文件，
        #    前端用原生 <video>/<img> 播放；成功后内部已 plt.close("all")，静态 PNG 捕获不会重复输出末帧。
        # 2. 没有动画时回退到既有的静态 PNG 路径，保持原有单帧绘图行为不变。
        explicit_animation_result = False
        try:
            if "matplotlib" in sys.modules and "_result" in globals_dict:
                from matplotlib.animation import Animation

                explicit_animation_result = isinstance(globals_dict.get("_result"), Animation)
        except Exception:
            explicit_animation_result = False

        mpl_display = _capture_cell_animation_display(
            globals_dict,
            previous_animation_ids=matplotlib_animation_ids_before,
            explicit_result_only=explicit_animation_result,
        )
        if mpl_display is None:
            png_b64 = _capture_matplotlib_png_base64()
            if png_b64:
                mpl_display = {
                    "type": "image",
                    "mime": "image/png",
                    "data_base64": png_b64,
                }
        if mpl_display:
            if isinstance(result_json, dict):
                result_json.setdefault("__guaxi_display__", mpl_display)
            elif result_json is None:
                result_json = {"__guaxi_display__": mpl_display}
            else:
                result_json = {
                    "__guaxi_display__": mpl_display,
                    "__guaxi_result__": result_json,
                }

        dataflow_after = _snapshot_session_var_map(globals_dict)
        dataflow_payload = _build_dataflow_payload(code, dataflow_before, dataflow_after)
        if isinstance(result_json, dict):
            result_json["__guaxi_dataflow__"] = dataflow_payload
        elif result_json is None:
            result_json = {"__guaxi_dataflow__": dataflow_payload}
        else:
            result_json = {
                "__guaxi_result__": result_json,
                "__guaxi_dataflow__": dataflow_payload,
            }

        if persist:
            _shift_last_results(globals_dict, resolved_session_id)

    except BaseException as exc:
        ok = False
        err_trace = _format_traceback(exc)
    finally:
        if old_cwd is not None:
            try:
                os.chdir(old_cwd)
            except Exception:
                pass
        sys.stdout, sys.stderr = old_stdout, old_stderr

    stdout = stdout_buffer.getvalue()
    stderr = stderr_buffer.getvalue()
    if err_trace.strip():
        if stderr.strip():
            stderr += "\n"
        stderr += err_trace

    session_vars = _snapshot_session_vars(globals_dict, session_var_names) if include_session_vars else []

    return {
        "ok": ok,
        "stdout": stdout,
        "stderr": stderr,
        "result_repr": result_repr,
        "result_json": result_json,
        "session_id": resolved_session_id,
        "session_vars": session_vars,
    }


def _handle_reset(_payload):
    SESSIONS.clear()
    RESULT_HISTORY.clear()
    TABLE_HANDLES.clear()
    for _, sandbox_dir in list(SESSION_DIRS.items()):
        try:
            shutil.rmtree(sandbox_dir, ignore_errors=True)
        except Exception:
            pass
    SESSION_DIRS.clear()
    return {
        "ok": True,
        "stdout": "",
        "stderr": "",
        "result_repr": None,
        "result_json": None,
        "session_id": "__guaxi_global__",
        "session_vars": [],
    }


def _handle_drop_session(payload):
    session_id = str(payload.get("session_id") or "").strip()
    if not session_id:
        return {"ok": False, "error": "Missing session_id"}

    SESSIONS.pop(session_id, None)
    RESULT_HISTORY.pop(session_id, None)
    sandbox_dir = SESSION_DIRS.pop(session_id, "")

    if sandbox_dir:
        try:
            shutil.rmtree(sandbox_dir, ignore_errors=True)
        except Exception:
            pass

    table_prefix = f"{session_id}-"
    for handle in list(TABLE_HANDLES.keys()):
        if session_id == "__guaxi_global__":
            break
        manifest_path = str(TABLE_HANDLES.get(handle, {}).get("manifest_path") or "")
        if sandbox_dir and manifest_path.startswith(sandbox_dir):
            TABLE_HANDLES.pop(handle, None)

    return {
        "ok": True,
        "stdout": "",
        "stderr": "",
        "result_repr": None,
        "result_json": {"sandbox_dir": sandbox_dir},
        "session_id": session_id,
        "session_vars": [],
    }


def _handle_table_slice(payload):
    try:
        _, _, table_key, manifest_entry, table_entry = _resolve_table_entry(payload)
        offset = max(0, int(payload.get("offset") or 0))
        limit = max(1, min(2000, int(payload.get("limit") or 200)))

        value = _load_table_entry_value(table_entry)
        columns = [str(col) for col in manifest_entry.get("columns") or table_entry.get("columns") or []]
        row_count = int(manifest_entry.get("row_count") or table_entry.get("row_count") or 0)
        table_name = str(manifest_entry.get("table_name") or table_entry.get("table_name") or table_key)
        table_kind = str(manifest_entry.get("table_kind") or table_entry.get("kind") or "records")

        if table_kind == "dataframe":
            rows = _dataframe_rows_slice(value, offset, limit)
        else:
            rows = _records_rows_slice(value, columns, offset, limit)

        return {
            "ok": True,
            "error": None,
            "table_key": table_key,
            "table_name": table_name,
            "table_kind": table_kind,
            "columns": columns,
            "rows": rows,
            "row_count": row_count,
            "offset": offset,
            "limit": limit,
        }
    except Exception as exc:
        return {
            "ok": False,
            "error": _format_traceback(exc),
            "rows": [],
        }


def _handle_table_export(payload):
    try:
        bundle, manifest, table_key, manifest_entry, table_entry = _resolve_table_entry(payload)
        output_path = str(payload.get("output_path") or "").strip()
        export_format = str(payload.get("format") or "").strip().lower()
        export_scope = str(payload.get("scope") or "table").strip().lower()
        if not output_path:
            return {"ok": False, "error": "Missing output_path"}
        if not export_format:
            return {"ok": False, "error": "Missing format"}

        tables_meta = manifest.get("tables") or []
        export_entries = []
        if export_scope == "workbook":
            for item in tables_meta:
                item_key = _normalize_table_key(item.get("table_key"), "table")
                entry = bundle.setdefault("tables", {}).get(item_key) or {
                    "kind": str(item.get("table_kind") or "records"),
                    "artifact_path": str(item.get("artifact_path") or ""),
                    "table_name": str(item.get("table_name") or item_key),
                    "columns": [str(col) for col in item.get("columns") or []],
                    "row_count": int(item.get("row_count") or 0),
                    "dtypes": item.get("dtypes") or {},
                }
                bundle["tables"][item_key] = entry
                export_entries.append((item_key, item, entry))
        else:
            export_entries.append((table_key, manifest_entry, table_entry))

        if export_format in ("xlsx", "xls"):
            try:
                import pandas as pd
            except Exception as exc:
                raise RuntimeError(f"Exporting Excel files requires pandas: {exc}") from exc

            with pd.ExcelWriter(output_path) as writer:
                for index, (_, item_meta, entry) in enumerate(export_entries, start=1):
                    columns, rows = _table_to_records(entry)
                    df = pd.DataFrame(rows, columns=columns)
                    sheet_name = _sheet_name_for_export(item_meta.get("table_name"), f"Sheet{index}")
                    df.to_excel(writer, sheet_name=sheet_name, index=False)
        elif export_format in ("csv", "tsv"):
            if len(export_entries) != 1:
                raise ValueError("CSV/TSV export supports a single table only")
            _, _, entry = export_entries[0]
            columns, rows = _table_to_records(entry)
            delimiter = "\t" if export_format == "tsv" else ","
            import csv

            with open(output_path, "w", encoding="utf-8-sig", newline="") as handle_file:
                writer = csv.DictWriter(handle_file, fieldnames=columns, delimiter=delimiter)
                writer.writeheader()
                for row in rows:
                    writer.writerow(row)
        elif export_format == "json":
            if len(export_entries) == 1 and export_scope != "workbook":
                _, _, entry = export_entries[0]
                _, rows = _table_to_records(entry)
                payload_json = rows
            else:
                payload_json = {}
                for index, (_, item_meta, entry) in enumerate(export_entries, start=1):
                    sheet_name = _sheet_name_for_export(item_meta.get("table_name"), f"Sheet{index}")
                    _, rows = _table_to_records(entry)
                    payload_json[sheet_name] = rows
            with open(output_path, "w", encoding="utf-8") as handle_file:
                json.dump(payload_json, handle_file, ensure_ascii=False, indent=2)
        elif export_format == "parquet":
            if len(export_entries) != 1:
                raise ValueError("Parquet export supports a single table only")
            try:
                import pandas as pd
            except Exception as exc:
                raise RuntimeError(f"Exporting parquet files requires pandas: {exc}") from exc
            _, _, entry = export_entries[0]
            columns, rows = _table_to_records(entry)
            pd.DataFrame(rows, columns=columns).to_parquet(output_path, index=False)
        elif export_format in ("sqlite", "db", "duckdb"):
            try:
                import pandas as pd
            except Exception as exc:
                raise RuntimeError(f"Exporting database files requires pandas: {exc}") from exc

            if export_format == "duckdb":
                try:
                    import duckdb
                except Exception as exc:
                    raise RuntimeError(f"Exporting DuckDB files requires duckdb: {exc}") from exc

                conn = duckdb.connect(output_path)
                try:
                    for index, (_, item_meta, entry) in enumerate(export_entries, start=1):
                        columns, rows = _table_to_records(entry)
                        table_name = _normalize_table_key(item_meta.get("table_name"), f"table_{index}")
                        df = pd.DataFrame(rows, columns=columns)
                        conn.execute(f'DROP TABLE IF EXISTS "{table_name}"')
                        conn.register("__guaxi_export_df", df)
                        conn.execute(f'CREATE TABLE "{table_name}" AS SELECT * FROM __guaxi_export_df')
                        conn.unregister("__guaxi_export_df")
                finally:
                    conn.close()
            else:
                import sqlite3

                conn = sqlite3.connect(output_path)
                try:
                    for index, (_, item_meta, entry) in enumerate(export_entries, start=1):
                        columns, rows = _table_to_records(entry)
                        table_name = _normalize_table_key(item_meta.get("table_name"), f"table_{index}")
                        pd.DataFrame(rows, columns=columns).to_sql(table_name, conn, if_exists="replace", index=False)
                finally:
                    conn.close()
        else:
            raise ValueError(f"Unsupported export format: {export_format}")

        return {
            "ok": True,
            "error": None,
            "output_path": output_path,
            "format": export_format,
            "scope": export_scope,
            "table_count": len(export_entries),
        }
    except Exception as exc:
        return {
            "ok": False,
            "error": _format_traceback(exc),
        }


def _handle_import_table(payload):
    path = str(payload.get("path") or "").strip()
    if not path:
        return {"ok": False, "error": "Missing table file path"}
    if not os.path.exists(path):
        return {"ok": False, "error": f"File not found: {path}"}

    ext = os.path.splitext(path)[1].lower()
    try:
        try:
            import pandas as pd
        except Exception:
            pd = None

        if pd is not None:
            if ext in (".csv", ".txt", ".tsv"):
                sep = "\t" if ext == ".tsv" else ","
                df = pd.read_csv(path, sep=sep)
            elif ext in (".xlsx", ".xls"):
                df = pd.read_excel(path)
            elif ext == ".json":
                df = pd.read_json(path)
            elif ext == ".parquet":
                df = pd.read_parquet(path)
            else:
                return {"ok": False, "error": f"Unsupported table format: {ext or path}"}

            return {"ok": True, "table": _build_dataframe_payload(df, source_path=path)}

        if ext in (".csv", ".txt", ".tsv"):
            import csv

            delimiter = "\t" if ext == ".tsv" else ","
            with open(path, "r", encoding="utf-8-sig", newline="") as f:
                rows = list(csv.reader(f, delimiter=delimiter))
            if not rows:
                return {
                    "ok": True,
                    "table": {
                        "columns": [],
                        "rows": [],
                        "row_count": 0,
                        "truncated": False,
                        "dtypes": {},
                        "source_path": path,
                    },
                }
            columns = [str(col) for col in rows[0]]
            body = rows[1:]
            return {
                "ok": True,
                "table": {
                    "columns": columns,
                    "rows": [[_normalize_basic(item) for item in row] for row in body],
                    "row_count": max(0, len(rows) - 1),
                    "truncated": False,
                    "dtypes": {str(col): "str" for col in columns},
                    "source_path": path,
                },
            }

        if ext == ".json":
            with open(path, "r", encoding="utf-8") as f:
                payload_json = json.load(f)
            if isinstance(payload_json, list) and payload_json and isinstance(payload_json[0], dict):
                columns = sorted({str(key) for item in payload_json for key in item.keys()})
                body = payload_json
                rows = [[_normalize_basic(item.get(col)) for col in columns] for item in body]
                return {
                    "ok": True,
                    "table": {
                        "columns": columns,
                        "rows": rows,
                        "row_count": len(payload_json),
                        "truncated": False,
                        "dtypes": {str(col): "json" for col in columns},
                        "source_path": path,
                    },
                }
            return {"ok": False, "error": "JSON table import requires a list of objects or pandas support"}

        return {"ok": False, "error": f"Unsupported table format without pandas: {ext or path}"}
    except Exception as exc:
        return {"ok": False, "error": _format_traceback(exc)}


def _handle_extract_article(payload):
    """工业级正文抽取：用 trafilatura 把原始 HTML 清洗成高质量图文。

    输入：{"html": "<原始页面 HTML>", "url": "<原文 URL，供相对链接绝对化>"}
    输出：{"ok": True, "article": {
            title, markdown, html, text, lead_image, images[], byline,
            published, lang, excerpt
        }}
    trafilatura 缺失/抽取失败时返回 {"ok": False, "error": ...}，
    由 Rust 侧回退到本地 scraper 抽取，保证链路不中断。
    """
    html = str(payload.get("html") or "")
    url = str(payload.get("url") or "").strip() or None
    if not html.strip():
        return {"ok": False, "error": "empty html"}

    try:
        import trafilatura
        from trafilatura.settings import use_config
    except Exception as exc:
        return {"ok": False, "error": f"trafilatura unavailable: {exc}"}

    try:
        cfg = use_config()
        # 不做最小长度截断，保留尽量完整的正文；关闭信号量超时（worker 已有看门狗）。
        cfg.set("DEFAULT", "MIN_EXTRACTED_SIZE", "0")
        cfg.set("DEFAULT", "MIN_OUTPUT_SIZE", "0")
        cfg.set("DEFAULT", "EXTRACTION_TIMEOUT", "0")

        # Markdown 正文（含图片/链接）。
        markdown = trafilatura.extract(
            html,
            url=url,
            output_format="markdown",
            include_images=True,
            include_links=True,
            include_formatting=True,
            include_tables=True,
            favor_precision=True,
            config=cfg,
        ) or ""

        # 结构化元数据 + HTML 正文。
        meta = trafilatura.bare_extraction(
            html,
            url=url,
            with_metadata=True,
            include_images=True,
            include_links=True,
            include_formatting=True,
            include_tables=True,
            favor_precision=True,
            config=cfg,
        )

        def _meta_get(key):
            if meta is None:
                return None
            if isinstance(meta, dict):
                return meta.get(key)
            return getattr(meta, key, None)

        title = _meta_get("title")
        byline = _meta_get("author")
        published = _meta_get("date")
        excerpt = _meta_get("description") or _meta_get("excerpt")
        lang = _meta_get("language")
        lead_image = _meta_get("image")
        body_html = _meta_get("body") or _meta_get("text") or ""
        plain_text = _meta_get("text") or ""

        # 从 markdown 里收集图片 URL（![](url) 与裸 http 图片链接）。
        images = []
        seen = set()
        for m in re.finditer(r"!\[[^\]]*\]\(([^)\s]+)", markdown):
            u = m.group(1).strip()
            if u and u not in seen:
                seen.add(u)
                images.append(u)
        if lead_image and lead_image not in seen:
            images.insert(0, lead_image)
            seen.add(lead_image)
        if not lead_image and images:
            lead_image = images[0]

        if not (markdown.strip() or str(body_html).strip()):
            return {"ok": False, "error": "trafilatura produced empty content"}

        return {
            "ok": True,
            "article": {
                "title": title or "",
                "markdown": markdown,
                "html": str(body_html or ""),
                "text": str(plain_text or ""),
                "lead_image": lead_image or "",
                "images": images,
                "byline": byline or "",
                "published": published or "",
                "lang": lang or "",
                "excerpt": excerpt or "",
            },
        }
    except Exception as exc:
        return {"ok": False, "error": _format_traceback(exc)}


def _dispatch(payload):
    cmd = str(payload.get("cmd") or "").strip()
    if cmd == "exec":
        return _handle_exec(payload)
    if cmd == "reset":
        return _handle_reset(payload)
    if cmd == "drop_session":
        return _handle_drop_session(payload)
    if cmd == "import_table":
        return _handle_import_table(payload)
    if cmd == "table_slice":
        return _handle_table_slice(payload)
    if cmd == "table_export":
        return _handle_table_export(payload)
    if cmd == "extract_article":
        return _handle_extract_article(payload)
    return {"ok": False, "error": f"Unknown worker command: {cmd}"}


def main():
    # 让 worker 自身所在目录进入 sys.path，使随它一起物化的数学引擎
    # guaximath_engine.py 可被 `import guaximath_engine` 直接加载（一次常驻会话）。
    try:
        _worker_dir = os.path.dirname(os.path.abspath(__file__))
        if _worker_dir and _worker_dir not in sys.path:
            sys.path.insert(0, _worker_dir)
    except Exception:
        pass
    try:
        sys.stdin.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    try:
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
            response = _dispatch(payload)
        except BaseException as exc:
            response = {
                "ok": False,
                "stdout": "",
                "stderr": _format_traceback(exc),
                "error": _format_traceback(exc),
                "result_repr": None,
                "result_json": None,
                "session_id": "__guaxi_global__",
                "session_vars": [],
            }

        sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
