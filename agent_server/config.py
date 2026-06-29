"""
Guaxi Agent Server - 配置管理
从环境变量或 Rust 后端获取 LLM API 配置。
"""
import os
import json
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class ModelConfig:
    """单个模型的配置"""
    name: str
    base_url: str
    api_key: str
    model_name: str
    supports_tools: bool = True


# 模型ID -> ModelConfig 的映射
MODEL_REGISTRY: dict[int, ModelConfig] = {}


# 默认工作目录（知识库根目录）
DEFAULT_WORK_DIR: str = os.environ.get("GUAXI_WORK_DIR", os.path.expanduser("~"))


def load_config_from_env():
    """从环境变量加载 LLM 配置（由 Rust 后端在启动 agent 时设置）"""
    global MODEL_REGISTRY, DEFAULT_WORK_DIR
    
    # 工作目录
    DEFAULT_WORK_DIR = os.environ.get("GUAXI_WORK_DIR", os.path.expanduser("~"))
    
    # 从环境变量 GUAXI_LLM_CONFIG (JSON) 加载
    raw = os.environ.get("GUAXI_LLM_CONFIG", "")
    if raw:
        try:
            data = json.loads(raw)
            for item in data:
                mid = int(item["id"])
                MODEL_REGISTRY[mid] = ModelConfig(
                    name=item.get("name", f"model-{mid}"),
                    base_url=item["base_url"],
                    api_key=item.get("api_key", ""),
                    model_name=item.get("model_name", ""),
                    supports_tools=item.get("supports_tools", True),
                )
            print(f"[agent_server] Loaded {len(MODEL_REGISTRY)} models from env")
            return
        except Exception as e:
            print(f"[agent_server] Failed to parse GUAXI_LLM_CONFIG: {e}")
    
    # Fallback: 使用默认配置（本地模型 + 常用 API）
    _load_defaults()


def _load_defaults():
    """默认配置（当 Rust 后端未提供时）"""
    # Local Qwen3
    MODEL_REGISTRY[9] = ModelConfig(
        name="Local-Qwen3",
        base_url="http://127.0.0.1:18082/v1",
        api_key="not-needed",
        model_name="local-qwen3",
        supports_tools=True,
    )
    # SiliconFlow DeepSeek
    key = os.environ.get("SILICONFLOW_API_KEY", "")
    if key:
        MODEL_REGISTRY[5] = ModelConfig(
            name="SiliconFlow-DeepSeek",
            base_url="https://api.siliconflow.cn/v1",
            api_key=key,
            model_name="deepseek-ai/DeepSeek-R1-0528-Qwen3-8B",
            supports_tools=True,
        )
    # DeepSeek API
    key = os.environ.get("DEEPSEEK_API_KEY", "")
    if key:
        MODEL_REGISTRY[1] = ModelConfig(
            name="DeepSeek",
            base_url="https://api.deepseek.com/v1",
            api_key=key,
            model_name="deepseek-chat",
            supports_tools=True,
        )


def get_model_config(model_id: int) -> Optional[ModelConfig]:
    """获取模型配置"""
    return MODEL_REGISTRY.get(model_id)
