# 数据生成:按列规格生成示例数据(Markdown / CSV / JSON),在光标处插入。
import datetime
import json
import random

_SURNAMES = "赵钱孙李周吴郑王冯陈褚卫蒋沈韩杨朱秦尤许何吕施张孔曹严华金魏陶姜"
_GIVEN = ["子涵", "雨桐", "浩然", "思远", "佳怡", "宇轩", "欣怡", "明哲", "诗涵", "俊杰", "嘉宁", "若彤"]
_DOMAINS = ["example.com", "mail.com", "test.cn"]
_STATUS = ["待处理", "进行中", "已完成"]


def _value(kind, rng, i):
    k = (kind or "").strip().lower()
    if k in ("name", "姓名", "人名"):
        return rng.choice(_SURNAMES) + rng.choice(_GIVEN)
    if k in ("email", "邮箱"):
        return "user%03d@%s" % (i + 1, rng.choice(_DOMAINS))
    if k in ("phone", "手机", "电话"):
        return "1%d%08d" % (rng.choice([3, 5, 7, 8, 9]), rng.randrange(10 ** 8))
    if k in ("date", "日期"):
        return (datetime.date(2025, 1, 1) + datetime.timedelta(days=rng.randrange(365))).isoformat()
    if k in ("amount", "金额", "price", "价格"):
        return "%.2f" % (rng.random() * 10000)
    if k in ("int", "整数", "number", "数字"):
        return str(rng.randrange(1, 1000))
    if k in ("status", "状态"):
        return rng.choice(_STATUS)
    if k in ("bool", "布尔"):
        return rng.choice(["是", "否"])
    return "%s-%d" % (kind or "value", i + 1)


def _columns(spec):
    cols = []
    for part in str(spec or "").split(","):
        part = part.strip()
        if not part:
            continue
        if ":" in part:
            label, kind = part.split(":", 1)
        elif "：" in part:
            label, kind = part.split("：", 1)
        else:
            label, kind = part, part
        cols.append((label.strip(), kind.strip()))
    return cols or [("姓名", "name"), ("邮箱", "email"), ("金额", "amount")]


def _cell(v):
    return str(v).replace("|", "\\|").replace("\n", "<br>")


def _md_table(header, rows):
    out = ["| " + " | ".join(_cell(h) for h in header) + " |",
           "| " + " | ".join("---" for _ in header) + " |"]
    out.extend("| " + " | ".join(_cell(c) for c in r) + " |" for r in rows)
    return "\n".join(out)


def run(ctx):
    args = ctx.get("args") or {}
    try:
        rows_n = int(float(args.get("rows") or 20))
    except Exception:
        rows_n = 20
    rows_n = max(1, min(rows_n, 1000))
    try:
        seed = int(float(args.get("seed") or 42))
    except Exception:
        seed = 42
    fmt = str(args.get("format") or "Markdown").strip()
    cols = _columns(args.get("columns"))
    rng = random.Random(seed)
    rows = [[_value(kind, rng, i) for _label, kind in cols] for i in range(rows_n)]
    header = [label for label, _kind in cols]
    if fmt.lower() == "csv":
        lines = [",".join(header)]
        lines.extend(",".join('"%s"' % str(c).replace('"', '""') for c in r) for r in rows)
        return {"text": "\n".join(lines) + "\n"}
    if fmt.lower() == "json":
        return {"text": json.dumps([dict(zip(header, r)) for r in rows], ensure_ascii=False, indent=2) + "\n"}
    return {"text": _md_table(header, rows) + "\n"}
