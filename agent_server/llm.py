"""
Guaxi Agent Server - LLM Router
统一的 LLM 调用接口，支持所有 OpenAI 兼容模型。
"""
import json
from typing import Any
from openai import OpenAI
from .config import get_model_config, ModelConfig


def build_client(config: ModelConfig) -> OpenAI:
    """构建 OpenAI 兼容客户端"""
    return OpenAI(
        base_url=config.base_url,
        api_key=config.api_key,
        timeout=120.0,
    )


def chat_complete(
    model_id: int,
    messages: list[dict[str, Any]],
    tools: list[dict[str, Any]] | None = None,
    tool_choice: str = "auto",
    stream: bool = False,
) -> dict[str, Any]:
    """
    统一的 LLM 补全接口。
    
    Args:
        model_id: 模型ID (0-10)
        messages: 对话消息列表
        tools: 工具定义（OpenAI function calling 格式）
        tool_choice: 工具选择策略
        stream: 是否流式（Agent 循环中通常用非流式）
    
    Returns:
        OpenAI 格式的完整响应
    """
    config = get_model_config(model_id)
    if config is None:
        raise ValueError(f"Unknown model id: {model_id}")
    
    client = build_client(config)
    
    kwargs: dict[str, Any] = {
        "model": config.model_name,
        "messages": messages,
        "stream": stream,
    }
    
    if tools and config.supports_tools:
        kwargs["tools"] = tools
        kwargs["tool_choice"] = tool_choice
    
    response = client.chat.completions.create(**kwargs)
    
    if stream:
        return _collect_stream(response)
    
    return _parse_response(response)


def _parse_response(response) -> dict[str, Any]:
    """解析非流式响应为统一格式"""
    choice = response.choices[0]
    msg = choice.message
    
    result: dict[str, Any] = {
        "role": msg.role or "assistant",
        "content": msg.content or "",
    }
    
    # 解析 tool_calls
    if msg.tool_calls:
        tool_calls = []
        for tc in msg.tool_calls:
            tool_calls.append({
                "id": tc.id,
                "type": "function",
                "function": {
                    "name": tc.function.name,
                    "arguments": tc.function.arguments,
                }
            })
        result["tool_calls"] = tool_calls
    
    return result


def _collect_stream(stream) -> dict[str, Any]:
    """收集流式响应"""
    # Agent 循环中暂时不使用流式，保留接口
    collected_content = ""
    for chunk in stream:
        if chunk.choices[0].delta.content:
            collected_content += chunk.choices[0].delta.content
    return {"role": "assistant", "content": collected_content}
