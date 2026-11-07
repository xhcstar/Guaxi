# 知识库 frontmatter 体检:遍历 .md,检查缺失 title/date/tags、日期格式是否规范。
# knowledge-base 契约:ctx["kb_location"] 是知识库目录,返回 {"report": markdown}。
import os
import re

_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}")


def _front(text):
    if not text.startswith("---"):
        return None
    end = text.find("\n---", 3)
    return None if end == -1 else text[3:end]


def run(ctx):
    loc = ctx.get("kb_location", "") if isinstance(ctx, dict) else ""
    if not loc or not os.path.isdir(loc):
        return {"report": "# frontmatter 体检\n\n未找到知识库目录。"}
    total = 0
    buckets = {"缺少 frontmatter": [], "缺少 title": [], "缺少 date": [], "缺少 tags": [], "date 非 YYYY-MM-DD": []}
    for root, _d, files in os.walk(loc):
        for f in files:
            if not f.lower().endswith(".md"):
                continue
            total += 1
            rel = os.path.relpath(os.path.join(root, f), loc).replace("\\", "/")
            try:
                txt = open(os.path.join(root, f), "r", encoding="utf-8", errors="ignore").read()
            except Exception:
                continue
            fm = _front(txt)
            if fm is None:
                buckets["缺少 frontmatter"].append(rel)
                continue
            keys = {}
            for line in fm.split("\n"):
                m = re.match(r"^([A-Za-z0-9_\-]+)\s*:\s*(.*)$", line)
                if m:
                    keys[m.group(1).lower()] = m.group(2).strip()
            for k in ("title", "date", "tags"):
                if not keys.get(k):
                    buckets["缺少 %s" % k].append(rel)
            d = keys.get("date", "").strip("\"'")
            if d and not _DATE.match(d):
                buckets["date 非 YYYY-MM-DD"].append(rel)
    lines = ["# frontmatter 体检", "", "Markdown 笔记总数: %d" % total, ""]
    for title, items in buckets.items():
        lines.append("## %s (%d)" % (title, len(items)))
        lines.extend(["- %s" % it for it in items[:50]] or ["- (无)"])
        if len(items) > 50:
            lines.append("- …其余 %d 条" % (len(items) - 50))
        lines.append("")
    return {"report": "\n".join(lines)}
