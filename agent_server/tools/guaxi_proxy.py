"""把 Guaxi 认知层的工具面代理给内置 Python Agent。

为什么是代理而不是再实现一套
------------------------------
Rust 侧 `cognition/tools.rs` 已经是唯一的能力边界：分级授权、不可信闸、路径白名单、
审计、逐动作确认都在那里，且 codex 与外部 MCP 客户端共用同一份。Python 引擎如果
自己再实现一遍知识库召回/抓取/电脑操作，就会出现「同一个问题在两个引擎里答案不同」
以及「改了一处忘了另一处」。

所以这里只做一件事：启动时向 `127.0.0.1:8888/mcp` 要一份**已授权**的工具清单，
把它们注册成 `guaxi__<name>`；调用时原样转发。授权变化在下一次 agent 会话启动时生效。

内置的 `filesystem` / `web` / `code_exec` 保留不动：它们是 Python 引擎的兜底能力
（没有主进程时也能读文件），与本模块不重叠。
"""

import json
import os
import urllib.request
from typing import Any

ENDPOINT = os.environ.get("GUAXI_MCP_ENDPOINT", "http://127.0.0.1:8888/mcp")
PREFIX = "guaxi__"

# 不把 Python 引擎本身就是兜底的那几类能力代理过来，避免模型同时看到
# `web_search` 与 `guaxi__guaxi_web_search` 两个几乎一样的选择。
_PROXY_TIERS = {"read", "write", "network", "exec", "gui"}


def _post(payload: dict[str, Any], timeout: float = 180.0) -> dict[str, Any]:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(
        ENDPOINT, data=body, headers={"content-type": "application/json"}
    )
    # 目标是回环地址，绝不能走系统代理（公司代理会让它绕一圈再回来）。
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def _list_tools() -> list[dict[str, Any]]:
    try:
        resp = _post({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
    except Exception:
        # 主进程没起时静默跳过：Python 引擎仍能用自带的文件工具工作。
        return []
    tools = resp.get("result", {}).get("tools", [])
    return [t for t in tools if isinstance(t, dict) and t.get("name")]


def _call_tool(name: str, args: dict[str, Any]) -> str:
    try:
        resp = _post(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {"name": name, "arguments": args},
            }
        )
    except Exception as e:
        return json.dumps({"error": f"无法连接 Guaxi 主程序: {e}"}, ensure_ascii=False)

    if "error" in resp:
        return json.dumps({"error": resp["error"].get("message", "未知错误")}, ensure_ascii=False)
    content = resp.get("result", {}).get("content", [])
    texts = [c.get("text", "") for c in content if isinstance(c, dict)]
    return "\n".join(t for t in texts if t)


def register_guaxi_tools() -> int:
    """把已授权的 Guaxi 工具注册进 Python 注册表。返回注册数量。"""
    # 延迟导入，避免与 tools/__init__ 的 register 定义产生循环。
    from . import register

    count = 0
    for tool in _list_tools():
        raw_name = tool["name"]
        description = str(tool.get("description") or raw_name)
        tier = ""
        if description.startswith("[") and "]" in description:
            tier = description[1 : description.index("]")]
        if tier not in _PROXY_TIERS:
            continue
        schema = tool.get("inputSchema") or {"type": "object", "properties": {}}
        proxied = f"{PREFIX}{raw_name}"

        def _handler(args, work_dir="", _name=raw_name):
            return _call_tool(_name, args or {})

        # 写/执行/操控类在 Rust 侧本来就会要求确认；Python 侧不再设 needs_confirm，
        # 否则同一次调用会弹两次卡。
        register(
            name=proxied,
            description=f"{description}（Guaxi 工具面代理）",
            parameters=schema,
            require_confirm=False,
        )(_handler)
        count += 1
    return count
