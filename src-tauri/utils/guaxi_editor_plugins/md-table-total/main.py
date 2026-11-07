# Markdown 表格:规范对齐,并为纯数值列追加「合计」行。
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re


def _split_row(line):
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|"):
        s = s[:-1]
    return [c.strip() for c in s.split("|")]


def _is_sep(cells):
    return len(cells) > 0 and all(re.fullmatch(r":?-{1,}:?", c or "") for c in cells)


def _to_num(x):
    t = (x or "").replace(",", "").replace("%", "").strip()
    try:
        return float(t)
    except Exception:
        return None


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    lines = [l for l in text.splitlines() if l.strip()]
    if len(lines) < 2:
        return {"text": text}
    rows = [_split_row(l) for l in lines]
    if not _is_sep(rows[1]):
        return {"text": text}
    header = rows[0]
    ncol = len(header)
    body = [(r + [""] * ncol)[:ncol] for r in rows[2:]]
    totals = [""] * ncol
    has_total = False
    if body:
        for c in range(ncol):
            parsed = [_to_num(r[c]) for r in body]
            if parsed and all(v is not None for v in parsed):
                totals[c] = "%g" % sum(parsed)
                has_total = True
    if has_total:
        if not totals[0]:
            totals[0] = "合计"
        body = body + [totals]
    widths = [len(header[c]) for c in range(ncol)]
    for r in body:
        for c in range(ncol):
            widths[c] = max(widths[c], len(r[c]))

    def fmt(cells):
        return "| " + " | ".join(cells[c].ljust(widths[c]) for c in range(ncol)) + " |"

    out = [fmt(header), "| " + " | ".join("-" * max(3, widths[c]) for c in range(ncol)) + " |"]
    for r in body:
        out.append(fmt(r))
    return {"text": "\n".join(out)}
