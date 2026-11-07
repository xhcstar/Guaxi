# 表单填充助手:从当前笔记/选中内容(或任务查询)提取字段,生成预填好的表单链接。
# 默认只输出链接与字段预览,不会自动提交;auto_open 打开时用默认浏览器打开 GET 链接。
import json
import re
import urllib.parse
import urllib.request
import webbrowser

_MCP = "http://127.0.0.1:8888/mcp"


def _bool(v, default=False):
    if isinstance(v, bool):
        return v
    if v is None:
        return default
    return str(v).lower() not in ("false", "0", "no", "")


def _mcp_call(name, arguments):
    body = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    }, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(_MCP, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    if data.get("error"):
        raise RuntimeError(str(data["error"]))
    content = ((data.get("result") or {}).get("content") or [])
    text = content[0].get("text") if content else ""
    if not text:
        return None
    try:
        return json.loads(text)
    except Exception:
        return text


def _fields_from_text(text):
    fields = {}
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        for line in lines[1:]:
            if line.strip() == "---":
                break
            m = re.match(r"^([A-Za-z0-9_\-\u4e00-\u9fff]+)\s*:\s*(.*)$", line)
            if m:
                fields[m.group(1).strip()] = m.group(2).strip().strip("\"'")
    for line in lines:
        if not line.strip().startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 2 and cells[0] and not re.fullmatch(r":?-{2,}:?", cells[0]) and not re.fullmatch(r":?-{2,}:?", cells[1]):
            if cells[0] in ("字段", "名称", "键", "Key", "key", "Field", "field"):
                continue
            fields.setdefault(cells[0], cells[1])
    for line in lines:
        if line.strip().startswith("|"):
            continue
        m = re.match(r"^\s*[-*]?\s*([^:：=]{1,40})[:：=]\s*(.+?)\s*$", line)
        if m and m.group(1).strip() != "---":
            fields.setdefault(m.group(1).strip(), m.group(2).strip().strip("\"'"))
    return fields


def _task_fields(keyword):
    args = {"keyword": keyword} if keyword else {}
    data = _mcp_call("guaxi_task_query", args)
    if isinstance(data, str):
        return {"任务查询结果": data}
    items = data if isinstance(data, list) else (data or {}).get("tasks") or (data or {}).get("items") or []
    if not items:
        return {}
    first = items[0] if isinstance(items[0], dict) else {"内容": str(items[0])}
    fields = {str(k): str(v) for k, v in first.items() if isinstance(v, (str, int, float, bool))}
    fields["任务总数"] = str(len(items))
    return fields


def run(ctx):
    args = ctx.get("args") or {}
    template = str(args.get("url_template") or "").strip()
    if not template:
        return {"error": "请先在插件参数里填写表单链接模板,例如 https://example.com/new?title={{title}}"}
    content = str(ctx.get("selection") or "").strip() or str(ctx.get("document") or "").strip()
    if not content:
        return {"error": "当前没有可用的笔记内容"}
    fields = _fields_from_text(content)
    if "任务" in str(args.get("source") or ""):
        try:
            fields.update(_task_fields(str(args.get("task_keyword") or "").strip()))
        except Exception as e:
            return {"error": "查询任务失败:%s(请确认本地服务已启动)" % e}
    if not fields:
        return {"error": "没有解析到字段,请用 key: value 或两列表格整理内容"}
    fields.setdefault("content", content)
    fields.setdefault("selection", content)

    def repl(m):
        return urllib.parse.quote(str(fields.get(m.group(1).strip(), "")), safe="")

    url = re.sub(r"\{\{\s*([^{}]+?)\s*\}\}", repl, template)
    preview = ["已生成表单链接(发送前请自行确认):", "", "[打开表单](<%s>)" % url, "", url, "", "| 字段 | 值 |", "| --- | --- |"]
    for k, v in fields.items():
        if k in ("content", "selection"):
            continue
        preview.append("| %s | %s |" % (
            str(k).replace("|", "\\|"),
            str(v).replace("|", "\\|").replace("\n", "<br>"),
        ))
    if _bool(args.get("auto_open"), False):
        try:
            webbrowser.open(url)
            preview.insert(2, "已在默认浏览器打开。\n")
        except Exception:
            pass
    return {"text": "\n".join(preview) + "\n"}
