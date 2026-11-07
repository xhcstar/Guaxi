# 参考文献整理:逐行去掉原有项目符号/编号、折叠多余空白,统一重新编号为 [1][2]...
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    items = []
    for raw in text.splitlines():
        s = raw.strip()
        if not s:
            continue
        s = re.sub(r"^\s*(?:[-*+]|\[\d+\]|\(?\d+[.)\]]|\d+、)\s*", "", s)
        s = re.sub(r"\s+", " ", s).strip()
        if s:
            items.append(s)
    if not items:
        return {"text": text}
    return {"text": "\n".join("[%d] %s" % (i + 1, it) for i, it in enumerate(items))}
