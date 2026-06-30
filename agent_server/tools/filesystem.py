"""
Guaxi Agent Server - 文件系统工具
"""
import os
import json
from contextvars import ContextVar
from pathlib import Path
from . import register


# 额外允许访问的根目录白名单（全部已配置知识库根）。
# 由 run_agent 在每次请求开始时设置；ContextVar 在同一异步任务的 await 链上有效，
# 因此无需改动各工具 handler 的签名即可让 _safe_path 读取到。
allowed_roots_var: ContextVar[list] = ContextVar("allowed_roots", default=[])


def _is_within(resolved: Path, root: Path) -> bool:
    """resolved 是否落在 root 内（含 root 自身）。"""
    if str(root) == os.sep or str(root) == "/":
        return True
    try:
        resolved.relative_to(root)
        return True
    except ValueError:
        return False


def _safe_path(raw: str, work_dir: str) -> Path:
    """安全解析路径，限制在工作目录或任一已配置知识库根之内。"""
    p = Path(raw)
    if not p.is_absolute():
        p = Path(work_dir) / raw
    resolved = p.resolve()

    # 允许根列表：工作目录 + 所有已配置知识库根。
    roots = [Path(work_dir).resolve()]
    for r in (allowed_roots_var.get() or []):
        if not r:
            continue
        try:
            roots.append(Path(r).resolve())
        except (OSError, ValueError):
            continue

    if any(_is_within(resolved, root) for root in roots):
        return resolved
    raise PermissionError(f"拒绝访问工作目录之外的文件: {raw}")


@register(
    name="read_file",
    description="读取指定文件的文本内容。用于查看文档、笔记、代码等。",
    parameters={
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "文件路径（绝对路径或相对工作目录）"
            },
            "max_chars": {
                "type": "integer",
                "description": "最大返回字符数，默认 10000",
                "default": 10000
            }
        },
        "required": ["path"]
    }
)
async def read_file(args: dict, work_dir: str) -> str:
    path = _safe_path(args["path"], work_dir)
    max_chars = args.get("max_chars", 10000)
    
    if not path.exists():
        return json.dumps({"error": f"文件不存在: {path}"}, ensure_ascii=False)
    if not path.is_file():
        return json.dumps({"error": f"不是文件: {path}"}, ensure_ascii=False)
    
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        return json.dumps({"error": f"读取失败: {e}"}, ensure_ascii=False)
    
    total_chars = len(content)
    truncated = total_chars > max_chars
    if truncated:
        content = content[:max_chars]
    
    return json.dumps({
        "path": str(path),
        "total_chars": total_chars,
        "truncated": truncated,
        "content": content,
    }, ensure_ascii=False)


@register(
    name="list_directory",
    description="列出目录中的文件和子目录。用于浏览文件结构。",
    parameters={
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "目录路径（省略时列出工作目录根）"
            },
            "max_entries": {
                "type": "integer",
                "description": "最大返回条目数，默认 50"
            }
        },
        "required": []
    }
)
async def list_directory(args: dict, work_dir: str) -> str:
    raw = args.get("path", "")
    if not raw:
        target = Path(work_dir)
    else:
        target = _safe_path(raw, work_dir)
    
    max_entries = args.get("max_entries", 50)
    
    if not target.exists():
        return json.dumps({"error": f"目录不存在: {target}"}, ensure_ascii=False)
    if not target.is_dir():
        return json.dumps({"error": f"不是目录: {target}"}, ensure_ascii=False)
    
    entries = []
    try:
        for entry in sorted(target.iterdir()):
            if len(entries) >= max_entries:
                break
            info = {
                "name": entry.name,
                "type": "dir" if entry.is_dir() else "file",
            }
            if entry.is_file():
                try:
                    info["size"] = entry.stat().st_size
                except OSError:
                    info["size"] = 0
            entries.append(info)
    except PermissionError as e:
        return json.dumps({"error": f"权限不足: {e}"}, ensure_ascii=False)
    
    return json.dumps({
        "path": str(target),
        "entries": entries,
        "truncated": len(list(target.iterdir())) > max_entries,
    }, ensure_ascii=False, default=str)


@register(
    name="write_file",
    description="创建或覆盖文件内容。写操作需要用户确认。",
    parameters={
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "文件路径"
            },
            "content": {
                "type": "string",
                "description": "要写入的内容"
            }
        },
        "required": ["path", "content"]
    },
    require_confirm=True
)
async def write_file(args: dict, work_dir: str) -> str:
    path = _safe_path(args["path"], work_dir)
    content = args["content"]
    
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    except Exception as e:
        return json.dumps({"error": f"写入失败: {e}"}, ensure_ascii=False)
    
    return json.dumps({
        "ok": True,
        "action": "written",
        "path": str(path),
        "size": len(content),
    }, ensure_ascii=False)


@register(
    name="search_files",
    description="按文件名模式搜索文件。支持通配符匹配。",
    parameters={
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "搜索模式（如 *.md, report*）"
            },
            "directory": {
                "type": "string",
                "description": "搜索目录，默认为工作目录根"
            },
            "max_results": {
                "type": "integer",
                "description": "最大结果数，默认 30"
            }
        },
        "required": ["pattern"]
    }
)
async def search_files(args: dict, work_dir: str) -> str:
    import fnmatch
    
    pattern = args["pattern"]
    search_dir = Path(work_dir)
    if args.get("directory"):
        search_dir = _safe_path(args["directory"], work_dir)
    max_results = args.get("max_results", 30)
    
    matches = []
    try:
        for root, dirs, files in os.walk(search_dir):
            # 跳过隐藏目录
            dirs[:] = [d for d in dirs if not d.startswith(".")]
            for fname in files:
                if fnmatch.fnmatch(fname, pattern):
                    full = Path(root) / fname
                    rel = full.relative_to(search_dir)
                    matches.append({
                        "name": fname,
                        "path": str(full),
                        "relative": str(rel),
                        "size": full.stat().st_size if full.exists() else 0,
                    })
                    if len(matches) >= max_results:
                        break
            if len(matches) >= max_results:
                break
    except Exception as e:
        return json.dumps({"error": f"搜索失败: {e}"}, ensure_ascii=False)
    
    return json.dumps({
        "pattern": pattern,
        "matches": matches,
        "count": len(matches),
        "truncated": len(matches) >= max_results,
    }, ensure_ascii=False, default=str)
