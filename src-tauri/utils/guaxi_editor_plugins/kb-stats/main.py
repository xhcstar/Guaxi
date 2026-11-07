# knowledge-base 作用面示例:ctx["kb_location"] 是知识库目录,遍历统计,返回 {"report": markdown}。
import os


def run(ctx):
    loc = ctx.get("kb_location", "") if isinstance(ctx, dict) else ""
    n_files = 0
    md = 0
    chars = 0
    if loc and os.path.isdir(loc):
        for root, _dirs, files in os.walk(loc):
            for f in files:
                n_files += 1
                if f.lower().endswith(".md"):
                    md += 1
                    try:
                        with open(os.path.join(root, f), "r", encoding="utf-8", errors="ignore") as fh:
                            chars += len(fh.read())
                    except Exception:
                        pass
    lines = ["# 知识库统计", "", "文件总数: %d" % n_files, "Markdown 笔记: %d" % md, "Markdown 总字数(约): %d" % chars]
    return {"report": "\n".join(lines)}
