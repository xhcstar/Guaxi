# 代码块整理:对选区内每个 ``` 围栏块,去行尾空白、Tab 转 4 空格;json 块美化缩进。
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re
import json

_FENCE = re.compile(r"(^|\n)(```|~~~)([^\n`~]*)\n(.*?)\n(\2)(?=\n|$)", re.DOTALL)


def _tidy_block(lang, code):
    code = code.replace("\t", "    ")
    code = "\n".join(line.rstrip() for line in code.split("\n"))
    l = (lang or "").strip().lower()
    if l in ("json", "jsonc"):
        try:
            code = json.dumps(json.loads(code), ensure_ascii=False, indent=2)
        except Exception:
            pass
    return code


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""

    def repl(m):
        return m.group(1) + m.group(2) + m.group(3) + "\n" + _tidy_block(m.group(3), m.group(4)) + "\n" + m.group(5)

    return {"text": _FENCE.sub(repl, text)}
