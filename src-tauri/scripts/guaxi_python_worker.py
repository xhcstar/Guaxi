import ast
import io
import json
import os
import shutil
import subprocess
import sys
import traceback

SESSIONS = {}
RESULT_HISTORY = {}
SESSION_DIRS = {}


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
        "preview": _safe_repr(value),
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
    except Exception:
        pass

    return summary


def _snapshot_session_vars(globals_dict):
    items = []
    hidden_names = {
        "__builtins__",
        "__guaxi_last_value",
        "__guaxi_plotly_display__",
        "_",
        "__",
        "___",
        "_result",
        "_guaxi_session",
        "_guaxi_inline_matplotlib",
        "__guaxi_pandas_verbose_repr_configured__",
        "__guaxi_numpy_verbose_repr_configured__",
        "_guaxi_cwd",
        "_guaxi_sandbox_dir",
        "_doc_path",
        "_py_cell_id",
    }

    for name in sorted(globals_dict.keys()):
        if name in hidden_names or str(name).startswith("__"):
            continue
        try:
            items.append({"name": str(name), **_summarize_value(globals_dict.get(name))})
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


def _handle_magic(code, globals_dict, session_id):
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
            "session_vars": _snapshot_session_vars(globals_dict),
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
        "session_vars": _snapshot_session_vars(globals_dict),
    }


def _handle_exec(payload):
    code = _normalize_exec_code(payload.get("code"))
    persist = bool(payload.get("persist"))
    expect_json = bool(payload.get("expect_json"))
    vars_map = payload.get("vars") or {}
    session_id = str(vars_map.get("_guaxi_session") or "").strip()

    globals_dict, resolved_session_id = _get_session_globals(session_id, persist)
    globals_dict.update(vars_map)
    sandbox_dir = _prepare_session_dir(resolved_session_id, globals_dict)

    magic_res = _handle_magic(code, globals_dict, resolved_session_id)
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

    try:
        if sandbox_dir:
            old_cwd = os.getcwd()
            os.chdir(sandbox_dir)

        if "\0" in code:
            raise ValueError("Python code contains NUL byte (\\0), cannot execute")

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
                try:
                    result_repr = repr(result)
                except Exception:
                    result_repr = None

                if expect_json:
                    result_json = _json_basic(result)

                dataframe_payload = _build_dataframe_payload(result)
                if dataframe_payload:
                    if isinstance(result_json, dict):
                        result_json.setdefault("__guaxi_dataframe__", dataframe_payload)
                    elif result_json is None:
                        result_json = {"__guaxi_dataframe__": dataframe_payload}
                    else:
                        result_json = {
                            "__guaxi_dataframe__": dataframe_payload,
                            "__guaxi_result__": result_json,
                        }

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

        png_b64 = _capture_matplotlib_png_base64()
        if png_b64:
            display = {
                "type": "image",
                "mime": "image/png",
                "data_base64": png_b64,
            }
            if isinstance(result_json, dict):
                result_json.setdefault("__guaxi_display__", display)
            elif result_json is None:
                result_json = {"__guaxi_display__": display}
            else:
                result_json = {
                    "__guaxi_display__": display,
                    "__guaxi_result__": result_json,
                }

        if persist:
            _shift_last_results(globals_dict, resolved_session_id)

    except Exception as exc:
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

    return {
        "ok": ok,
        "stdout": stdout,
        "stderr": stderr,
        "result_repr": result_repr,
        "result_json": result_json,
        "session_id": resolved_session_id,
        "session_vars": _snapshot_session_vars(globals_dict),
    }


def _handle_reset(_payload):
    SESSIONS.clear()
    RESULT_HISTORY.clear()
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

    return {
        "ok": True,
        "stdout": "",
        "stderr": "",
        "result_repr": None,
        "result_json": {"sandbox_dir": sandbox_dir},
        "session_id": session_id,
        "session_vars": [],
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
    return {"ok": False, "error": f"Unknown worker command: {cmd}"}


def main():
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
        except Exception as exc:
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
