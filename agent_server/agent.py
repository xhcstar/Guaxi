"""
Guaxi Agent Server - ReAct Agent 核心
Agent 循环：思考 → 行动 → 观察 → 迭代
"""
import json
import asyncio
from typing import AsyncGenerator, Any

from .config import DEFAULT_WORK_DIR
from .llm import chat_complete
from .tools import get_tool_specs, needs_confirm, dispatch as tool_dispatch
from .tools.filesystem import allowed_roots_var

# 全局取消标志
_cancel_flags: dict[str, bool] = {}

# 全局待确认请求
_pending_confirms: dict[str, asyncio.Event] = {}
_pending_confirm_results: dict[str, dict[str, Any]] = {}

AGENT_SYSTEM_PROMPT = """你是 Guaxi 智能知识管理助手，嵌入在用户的本地桌面应用中。
你可以调用工具来访问用户的文件系统、搜索网络、执行代码。

核心能力：
1. **文件管理**：读取、搜索、浏览用户的文件和笔记
2. **网络搜索**：搜索最新信息，抓取网页内容
3. **代码执行**：运行 Python 代码做数据分析、计算
4. **知识整理**：帮助用户总结文档、提取关键信息

规则：
1. 涉及用户的文件/文档问题时，先搜索定位，再读取内容，基于真实内容作答
2. 需要最新信息时，使用 web_search 搜索网络
3. 不要编造文件内容或搜索结果；信息不足时如实说明
4. 工具调用应精简高效，避免重复检索
5. 最终回答使用简洁的 Markdown 格式
6. 你是用户的知识伙伴，回答应友善、专业、有帮助"""

MAX_ITERATIONS = 8


def set_cancel(chat_id: str):
    """设置取消标志"""
    _cancel_flags[chat_id] = True


def clear_cancel(chat_id: str):
    """清除取消标志"""
    _cancel_flags.pop(chat_id, None)


def is_cancelled(chat_id: str) -> bool:
    """检查是否被取消"""
    return _cancel_flags.get(chat_id, False)


def submit_confirm_result(call_id: str, approved: bool, edited_args: dict | None = None):
    """提交用户确认结果"""
    if call_id in _pending_confirms:
        _pending_confirm_results[call_id] = {
            "approved": approved,
            "edited_args": edited_args,
        }
        _pending_confirms[call_id].set()


async def run_agent(
    chat_id: str,
    model_id: int,
    question: str,
    history: list[dict[str, str]],
    work_dir: str = "",
    allowed_roots: list[str] | None = None,
) -> AsyncGenerator[dict[str, Any], None]:
    """
    运行 ReAct Agent 循环，通过 SSE yield 事件。
    
    事件类型：
    - {"type": "status", "text": "思考中..."}
    - {"type": "tool_call", "name": "xxx", "args": {...}, "status": "running"}
    - {"type": "tool_result", "name": "xxx", "result": "..."}
    - {"type": "tool_confirm", "call_id": "...", "name": "xxx", "args": {...}}
    - {"type": "text", "content": "增量文本"}
    - {"type": "done"}
    - {"type": "error", "message": "..."}
    """
    wd = work_dir or DEFAULT_WORK_DIR
    clear_cancel(chat_id)

    # 设置跨库文件访问白名单：工作目录 + 所有已配置知识库根。
    # ContextVar 在本协程及其 await 链（含 tool_dispatch → _safe_path）内有效。
    allowed_roots_var.set(list(allowed_roots or []))

    tools = get_tool_specs()
    
    # 构建消息
    messages: list[dict[str, Any]] = []
    messages.append({"role": "system", "content": AGENT_SYSTEM_PROMPT})
    
    for msg in history:
        role = msg.get("role", "")
        content = msg.get("content", "")
        if role in ("user", "assistant") and content.strip():
            messages.append({"role": role, "content": content})
    
    messages.append({"role": "user", "content": question})
    
    try:
        for iteration in range(MAX_ITERATIONS):
            if is_cancelled(chat_id):
                yield {"type": "status", "text": "已被用户取消"}
                return
            
            yield {"type": "status", "text": f"Agent 思考中...（第 {iteration + 1} 轮）"}
            
            # 最后一轮不带工具，强制总结
            with_tools = iteration < MAX_ITERATIONS - 1
            effective_tools = tools if with_tools else None
            
            try:
                response = chat_complete(
                    model_id=model_id,
                    messages=messages,
                    tools=effective_tools,
                    tool_choice="auto" if with_tools else "none",
                    stream=False,
                )
            except Exception as e:
                yield {"type": "error", "message": f"LLM 调用失败: {e}"}
                return
            
            tool_calls = response.get("tool_calls", [])
            content = response.get("content", "")
            
            # 如果有工具调用
            if with_tools and tool_calls:
                # 记录 assistant 消息（含 tool_calls）
                messages.append({
                    "role": "assistant",
                    "content": content or None,
                    "tool_calls": tool_calls,
                })
                
                for tc in tool_calls:
                    if is_cancelled(chat_id):
                        return
                    
                    fname = tc["function"]["name"]
                    try:
                        args = json.loads(tc["function"]["arguments"])
                    except json.JSONDecodeError:
                        args = {}
                    
                    call_id = tc.get("id", "")
                    
                    # 检查是否需要用户确认
                    if needs_confirm(fname):
                        confirm_event = asyncio.Event()
                        _pending_confirms[call_id] = confirm_event
                        
                        yield {
                            "type": "tool_confirm",
                            "call_id": call_id,
                            "name": fname,
                            "args": args,
                        }
                        
                        # 等待用户确认（最多 120 秒）
                        try:
                            await asyncio.wait_for(confirm_event.wait(), timeout=120.0)
                        except asyncio.TimeoutError:
                            tool_result = "用户未在 120 秒内确认，操作已取消"
                            yield {"type": "tool_result", "name": fname, "status": "timeout"}
                        else:
                            result_data = _pending_confirm_results.pop(call_id, {})
                            if result_data.get("approved"):
                                eff_args = result_data.get("edited_args") or args
                                yield {"type": "tool_call", "name": fname, "args": eff_args, "status": "running"}
                                tool_result = await tool_dispatch(fname, eff_args, wd)
                                yield {"type": "tool_result", "name": fname, "status": "done", "result": tool_result}
                            else:
                                tool_result = "用户拒绝执行此操作"
                                yield {"type": "tool_result", "name": fname, "status": "rejected"}
                        finally:
                            _pending_confirms.pop(call_id, None)
                            _pending_confirm_results.pop(call_id, None)
                    else:
                        yield {"type": "tool_call", "name": fname, "args": args, "status": "running"}
                        tool_result = await tool_dispatch(fname, args, wd)
                        yield {"type": "tool_result", "name": fname, "status": "done", "result": tool_result}
                    
                    # 添加 tool 结果到消息
                    messages.append({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": tool_result,
                    })
                
                continue  # 继续下一轮
            
            # 没有工具调用：输出最终答案
            if content:
                yield {"type": "text", "content": content}
            
            yield {"type": "done"}
            return
        
        # 超过最大迭代次数
        yield {"type": "status", "text": "已达到最大思考轮数，生成最终回答..."}
        final_msg = messages[-1]
        yield {"type": "text", "content": final_msg.get("content", "抱歉，我无法在限定的轮数内完成任务。")}
        yield {"type": "done"}
        
    except Exception as e:
        yield {"type": "error", "message": f"Agent 执行失败: {e}"}
