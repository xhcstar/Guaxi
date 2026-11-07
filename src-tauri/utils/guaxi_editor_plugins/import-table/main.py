# 导入表格:读取 CSV / TSV / XLSX / JSON,在光标处生成 Markdown 表格。
# 文件路径可写在插件参数里;留空时尝试使用剪贴板中的路径。
import csv
import io
import json
import os


def _cell(v):
    return str(v).replace("|", "\\|").replace("\n", "<br>")


def _md(header, rows):
    out = ["| " + " | ".join(_cell(h) for h in header) + " |",
           "| " + " | ".join("---" for _ in header) + " |"]
    out.extend("| " + " | ".join(_cell(c) for c in r) + " |" for r in rows)
    return "\n".join(out)


def _rows_from_json(data):
    if isinstance(data, list):
        if data and all(isinstance(x, dict) for x in data):
            header = []
            for item in data:
                for k in item.keys():
                    if k not in header:
                        header.append(k)
            return header, [[item.get(k, "") for k in header] for item in data]
        if data and all(isinstance(x, (list, tuple)) for x in data):
            return list(data[0]), [list(x) for x in data[1:]]
    if isinstance(data, dict):
        if data and all(isinstance(v, list) for v in data.values()):
            header = list(data.keys())
            n = max(len(v) for v in data.values())
            return header, [[data[k][i] if i < len(data[k]) else "" for k in header] for i in range(n)]
        return list(data.keys()), [list(data.values())]
    return ["值"], [[data]]


def run(ctx):
    args = ctx.get("args") or {}
    path = str(args.get("path") or "").strip().strip('"')
    if not path:
        clip = str(ctx.get("clipboard") or "").strip().strip('"')
        if clip and os.path.isfile(clip):
            path = clip
    if not path:
        return {"error": "请先在插件参数里填写文件路径,或把文件路径复制到剪贴板"}
    if not os.path.isfile(path):
        return {"error": "文件不存在:%s" % path}
    try:
        max_rows = max(1, min(int(float(args.get("max_rows") or 200)), 5000))
    except Exception:
        max_rows = 200
    ext = os.path.splitext(path)[1].lower()
    try:
        if ext in (".xlsx", ".xlsm", ".xls"):
            import pandas as pd
            sheet = str(args.get("sheet") or "").strip()
            df = pd.read_excel(path, sheet_name=(sheet or 0))
            header = [str(c) for c in df.columns]
            rows = df.astype(str).values.tolist()
        elif ext == ".json":
            with open(path, "r", encoding="utf-8-sig") as fh:
                header, rows = _rows_from_json(json.load(fh))
        elif ext == ".tsv":
            with open(path, "r", encoding="utf-8-sig", errors="ignore") as fh:
                rows = [line.split("\t") for line in fh.read().splitlines() if line.strip()]
            header, rows = rows[0], rows[1:]
        else:
            with open(path, "r", encoding="utf-8-sig", errors="ignore") as fh:
                raw = fh.read()
            try:
                dialect = csv.Sniffer().sniff(raw[:4096], delimiters=",;\t")
                rows = list(csv.reader(io.StringIO(raw), dialect))
            except Exception:
                rows = [line.split(",") for line in raw.splitlines() if line.strip()]
            header, rows = rows[0], rows[1:]
    except Exception as e:
        return {"error": "读取失败:%s" % e}
    if not header:
        return {"error": "文件里没有表头"}
    truncated = len(rows) > max_rows
    rows = rows[:max_rows]
    width = len(header)
    rows = [(list(r) + [""] * width)[:width] for r in rows]
    note = "\n\n> 已截断,只显示前 %d 行" % max_rows if truncated else ""
    return {"text": _md(header, rows) + "\n" + note}
