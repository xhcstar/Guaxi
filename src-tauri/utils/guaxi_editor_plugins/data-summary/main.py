# 数据摘要:从选区中抽取所有数字,算计数/求和/均值/中位数/极值/标准差,
# 在选区下方追加一张 Markdown 摘要表(保留原选区内容)。
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re
import math


def _nums(text):
    out = []
    for tok in re.split(r"[\s,;]+", text.strip()):
        try:
            out.append(float(tok.replace("%", "")))
        except Exception:
            pass
    return out


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    xs = _nums(text)
    if not xs:
        return {"text": text}
    n = len(xs)
    srt = sorted(xs)
    mid = n // 2
    median = srt[mid] if n % 2 else (srt[mid - 1] + srt[mid]) / 2
    mean = sum(xs) / n
    std = math.sqrt(sum((x - mean) ** 2 for x in xs) / n)

    def g(v):
        return "%g" % v

    rows = [
        ("计数", str(n)), ("求和", g(sum(xs))), ("均值", g(mean)), ("中位数", g(median)),
        ("最小", g(min(xs))), ("最大", g(max(xs))), ("标准差(总体)", g(std)),
    ]
    table = ["| 指标 | 值 |", "| --- | --- |"] + ["| %s | %s |" % (k, v) for k, v in rows]
    return {"text": text.rstrip() + "\n\n" + "\n".join(table) + "\n"}
