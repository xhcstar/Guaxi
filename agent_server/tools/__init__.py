"""
Guaxi Agent Server - 工具注册表
所有 agent 可用工具的注册、调度与管理。
"""
import json
import traceback
from typing import Any, Callable


_registry: dict[str, dict[str, Any]] = {}
_handlers: dict[str, Callable] = {}


def register(
    name: str,
    description: str,
    parameters: dict[str, Any],
    require_confirm: bool = False,
):
    """装饰器：注册一个 agent 工具"""
    def decorator(func: Callable):
        _registry[name] = {
            "name": name,
            "description": description,
            "parameters": parameters,
            "require_confirm": require_confirm,
        }
        _handlers[name] = func
        return func
    return decorator


def get_tool_specs() -> list[dict[str, Any]]:
    """获取所有工具的 OpenAI function calling 格式定义"""
    specs = []
    for name, meta in _registry.items():
        specs.append({
            "type": "function",
            "function": {
                "name": name,
                "description": meta["description"],
                "parameters": meta["parameters"],
            }
        })
    return specs


def get_tool_meta(name: str) -> dict[str, Any] | None:
    """获取工具元数据"""
    return _registry.get(name)


def needs_confirm(name: str) -> bool:
    """检查工具是否需要用户确认"""
    meta = _registry.get(name)
    return meta["require_confirm"] if meta else False


async def dispatch(name: str, args: dict[str, Any], work_dir: str = "") -> str:
    """
    调度执行工具调用。
    
    Args:
        name: 工具名称
        args: 工具参数
        work_dir: 工作目录
    
    Returns:
        工具执行结果（字符串）
    """
    handler = _handlers.get(name)
    if handler is None:
        return json.dumps({"error": f"未知工具: {name}"}, ensure_ascii=False)
    
    try:
        result = await handler(args, work_dir)
        if isinstance(result, str):
            return result
        return json.dumps(result, ensure_ascii=False, default=str)
    except Exception as e:
        return json.dumps({
            "error": str(e),
            "traceback": traceback.format_exc(),
        }, ensure_ascii=False)
