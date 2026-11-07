# 选中内容 → 知识库笔记:写入当前知识库,并按标签/标题重叠补 [[双链]]。
import datetime
import os
import re


def _today():
    return datetime.date.today().isoformat()


def _slug(title):
    s = re.sub(r'[\\/:*?"<>|]+', "", title).strip().strip(".")
    s = re.sub(r"\s+", "-", s)
    return s[:80] or "note"


def _front(text):
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    out = {}
    for line in text[3:end].split("\n"):
        m = re.match(r"^([A-Za-z0-9_\-]+)\s*:\s*(.*)$", line)
        if m:
            out[m.group(1).lower()] = m.group(2).strip().strip("\"'")
    return out


def _tags(raw):
    raw = str(raw or "").strip()
    if raw.startswith("[") and raw.endswith("]"):
        raw = raw[1:-1]
    return [t for t in (x.strip() for x in re.split(r"[,，;；\s]+", raw)) if t]


def _related(loc, title, tags, limit=5):
    lower = title.lower()
    wanted = {t.lower() for t in tags}
    scored = []
    for root, _dirs, files in os.walk(loc):
        for f in files:
            if not f.lower().endswith(".md"):
                continue
            path = os.path.join(root, f)
            try:
                with open(path, "r", encoding="utf-8", errors="ignore") as fh:
                    head = fh.read(2000)
            except Exception:
                continue
            fm = _front(head)
            other = (fm.get("title") or f[:-3]).strip()
            if not other or other == title:
                continue
            score = len({t.lower() for t in _tags(fm.get("tags", ""))} & wanted)
            if lower and (lower in other.lower() or other.lower() in lower):
                score += 2
            if score > 0:
                scored.append((score, f[:-3], other))
    scored.sort(key=lambda x: (-x[0], x[1]))
    return [(stem, name) for _s, stem, name in scored[:limit]]


def run(ctx):
    content = str(ctx.get("selection") or "").strip()
    if not content:
        return {"error": "请先选中要存入知识库的内容"}
    loc = str(ctx.get("kb_location") or "").strip()
    if not loc or not os.path.isdir(loc):
        return {"error": "未找到当前知识库目录:请先在左侧选中一个知识库"}
    args = ctx.get("args") or {}
    folder = str(args.get("folder") or "收集").strip() or "收集"
    tags = _tags(args.get("tags"))

    title = str(args.get("title") or "").strip()
    if not title:
        for line in content.splitlines():
            s = line.strip()
            if s.startswith("#"):
                title = s.lstrip("#").strip()
                if title:
                    break
    if not title:
        first = next((l.strip() for l in content.splitlines() if l.strip()), "")
        title = first[:40] if first else "未命名笔记"
    title = title.replace("\n", " ").strip()

    target_dir = os.path.join(loc, folder)
    os.makedirs(target_dir, exist_ok=True)
    base = _slug(title)
    path = os.path.join(target_dir, base + ".md")
    n = 2
    while os.path.exists(path):
        path = os.path.join(target_dir, "%s-%d.md" % (base, n))
        n += 1

    source = str(ctx.get("file_path") or "").replace("\\", "/")
    title_yaml = title.replace("\\", "\\\\").replace('"', '\\"')
    source_yaml = source.replace("\\", "\\\\").replace('"', '\\"')
    tag_yaml = "[" + ", ".join('"%s"' % t.replace('"', '\\"') for t in tags) + "]"
    fm = [
        "---",
        'title: "%s"' % title_yaml,
        "date: %s" % _today(),
        "tags: %s" % tag_yaml,
        'source: "%s"' % source_yaml,
        "---",
        "",
    ]
    related = _related(loc, title, tags)
    body = [content, ""]
    if related:
        body.append("## 相关")
        for stem, name in related:
            body.append("- [[%s|%s]]" % (stem, name) if name != stem else "- [[%s]]" % stem)
        body.append("")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(fm) + "\n".join(body))
    stem = os.path.splitext(os.path.basename(path))[0]
    return {"text": content + "\n\n> 已存入知识库:%s[[%s]]" % (folder + "/", stem)}
