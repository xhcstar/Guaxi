# Guaxi 示例插件:中英文之间自动加空格(pangu 风格)。
# 契约:实现 run(ctx) 并返回 {"text": 新文本}。ctx 提供:
#   ctx["selection"] 当前选中文本 / ctx["document"] 全文 / ctx["args"] 预留参数
import re

_CJK = "一-鿿぀-ヿ㐀-䶿"


def _pangu(text):
    text = re.sub("([" + _CJK + "])([A-Za-z0-9])", r"\1 \2", text)
    text = re.sub("([A-Za-z0-9])([" + _CJK + "])", r"\1 \2", text)
    return text


def run(ctx):
    selection = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    return {"text": _pangu(selection)}
