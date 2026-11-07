# repo(文件库)作用面示例:ctx["resources"] 是资源元数据列表,返回 {"report": markdown}。
from collections import Counter


def run(ctx):
    res = ctx.get("resources", []) if isinstance(ctx, dict) else []
    types = Counter((r.get("type") or "未知") for r in res)
    tagged = sum(1 for r in res if r.get("tags"))
    lines = ["# 库健康报告", "", "资源总数: %d" % len(res), "已打标签: %d" % tagged, "", "## 类型分布"]
    for t, c in types.most_common():
        lines.append("- %s: %d" % (t, c))
    return {"report": "\n".join(lines)}
