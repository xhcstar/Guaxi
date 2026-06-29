"""
Guaxi Agent Server - FastAPI 服务入口
运行方式: python -m agent_server.main
           uvicorn agent_server.main:app --host 127.0.0.1 --port 9876
"""
import json
import asyncio
from contextlib import asynccontextmanager
from fastapi import FastAPI, Request
from fastapi.responses import StreamingResponse, JSONResponse
from pydantic import BaseModel

from .config import load_config_from_env, DEFAULT_WORK_DIR
from .agent import run_agent, set_cancel, clear_cancel, submit_confirm_result, is_cancelled
from .tools import get_tool_specs as list_tools, dispatch as tool_dispatch


# ========== 请求/响应模型 ==========

class ChatRequest(BaseModel):
    chat_id: str = "default"
    model_id: int = 9
    question: str = ""
    history: list[dict] = []
    work_dir: str = ""


class ConfirmRequest(BaseModel):
    call_id: str = ""
    approved: bool = True
    edited_args: dict | None = None


class CancelRequest(BaseModel):
    chat_id: str = "default"


class ToolExecRequest(BaseModel):
    name: str = ""
    args: dict = {}
    work_dir: str = ""


# ========== 应用初始化 ==========

@asynccontextmanager
async def lifespan(app: FastAPI):
    load_config_from_env()
    print(f"[agent_server] Ready on http://127.0.0.1:9876, work_dir={DEFAULT_WORK_DIR}")
    yield

app = FastAPI(title="Guaxi Agent Server", lifespan=lifespan)


# ========== 端点 ==========

@app.get("/health")
async def health():
    return {"status": "ok", "work_dir": DEFAULT_WORK_DIR}


@app.post("/agent/chat")
async def agent_chat(req: ChatRequest):
    """Agent 对话端点（SSE 流式返回）"""
    async def event_stream():
        try:
            async for event in run_agent(
                chat_id=req.chat_id,
                model_id=req.model_id,
                question=req.question,
                history=req.history,
                work_dir=req.work_dir or DEFAULT_WORK_DIR,
            ):
                data = json.dumps(event, ensure_ascii=False)
                yield f"data: {data}\n\n"
        except Exception as e:
            yield f"data: {json.dumps({'type': 'error', 'message': str(e)}, ensure_ascii=False)}\n\n"
    
    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
        }
    )


@app.post("/agent/confirm")
async def agent_confirm(req: ConfirmRequest):
    """用户确认/拒绝危险操作"""
    submit_confirm_result(req.call_id, req.approved, req.edited_args)
    return {"ok": True}


@app.post("/agent/cancel")
async def agent_cancel(req: CancelRequest):
    """取消正在运行的 Agent 任务"""
    set_cancel(req.chat_id)
    return {"ok": True}


@app.get("/tools/list")
async def tools_list():
    """列出所有可用工具"""
    specs = list_tools()
    return {"tools": specs, "count": len(specs)}


@app.post("/tools/exec")
async def tools_exec(req: ToolExecRequest):
    """直接执行工具（用于调试/测试）"""
    wd = req.work_dir or DEFAULT_WORK_DIR
    result = await tool_dispatch(req.name, req.args, wd)
    return {"result": result}


# ========== 入口 ==========

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=9876, log_level="info")
