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
    manifest_path = ""
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
