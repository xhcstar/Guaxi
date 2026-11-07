# 剪贴板表格 → Markdown:从 Excel / 网页复制区域,在光标处生成 Markdown 表格。
# 支持 Tab 分隔(Excel 默认)与 CSV;纯数值列可自动右对齐。
import csv
import io


def _clean(v):
    s = str(v if v is not None else "").replace("\r\n", "\n").replace("\r", "\n").strip()
    return s.replace("|", "\\|").replace("\n", "<br>")


def _is_num(v):
    t = str(v).replace(",", "").replace("%", "").replace("￥", "").replace("$", "").strip()
    if not t:
        return False
    try:
        float(t)
        return True
    except Exception:
        return False


def _parse(text):
    text = text.replace("\ufeff", "").replace("\r\n", "\n").replace("\r", "\n")
    lines = [l for l in text.split("\n") if l.strip() != ""]
    if not lines:
        return []
    if "\t" in text:
        return [l.split("\t") for l in lines]
    try:
        dialect = csv.Sniffer().sniff(text[:4096], delimiters=",;\t")
        return list(csv.reader(io.StringIO(text), dialect))
    except Exception:
        return [[l] for l in lines]


def _flag(v, default=True):
    if isinstance(v, bool):
        return v
    if v is None:
        return default
    return str(v).strip().lower() not in ("false", "0", "no", "")


def run(ctx):
    raw = ctx.get("clipboard") or ""
    if not str(raw).strip():
        return {"error": "剪贴板为空:请先在 Excel / 网页表格里复制一块区域"}
    args = ctx.get("args") or {}
    has_header = _flag(args.get("header"), True)
    numeric_align = _flag(args.get("numeric_align"), True)
    first_label = str(args.get("first_col_label") or "列").strip() or "列"

    rows = [[_clean(c) for c in r] for r in _parse(str(raw))]
    if not rows:
        return {"error": "没有解析到表格行"}
    width = max(len(r) for r in rows)
    rows = [(r + [""] * width)[:width] for r in rows]
    if has_header:
        header = rows[0]
        body = rows[1:]
    else:
        header = ["%s%d" % (first_label, i + 1) for i in range(width)]
        body = rows

    num_cols = []
    for c in range(width):
        vals = [r[c] for r in body if str(r[c]).strip() != ""]
        num_cols.append(bool(vals) and all(_is_num(v) for v in vals))

    def line(cells):
        return "| " + " | ".join(cells) + " |"

    sep = ["---:" if numeric_align and num_cols[c] else "---" for c in range(width)]
    out = [line(header), "| " + " | ".join(sep) + " |"]
    out.extend(line(r) for r in body)
    return {"text": "\n".join(out) + "\n"}
