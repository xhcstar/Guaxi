"""
Guaxi Agent Server - 代码执行工具
"""
import json
import subprocess
import tempfile
from pathlib import Path
from . import register


@register(
    name="exec_python",
    description="在沙箱中执行 Python 代码并返回结果。用于数据分析、计算、文件处理等。代码运行在受限环境中。",
    parameters={
        "type": "object",
        "properties": {
            "code": {
                "type": "string",
                "description": "Python 代码"
            },
            "timeout_secs": {
                "type": "integer",
                "description": "超时秒数，默认 30"
            }
        },
        "required": ["code"]
    }
)
async def exec_python(args: dict, work_dir: str) -> str:
    code = args["code"]
    timeout = args.get("timeout_secs", 30)
    
    # 受限的全局命名空间
    safe_globals = {
        "__builtins__": {
            "print": print,
            "len": len,
            "range": range,
            "int": int,
            "float": float,
            "str": str,
            "list": list,
            "dict": dict,
            "set": set,
            "tuple": tuple,
            "bool": bool,
            "sum": sum,
            "min": min,
            "max": max,
            "sorted": sorted,
            "enumerate": enumerate,
            "zip": zip,
            "map": map,
            "filter": filter,
            "abs": abs,
            "round": round,
            "isinstance": isinstance,
            "type": type,
            "open": open,
            "Path": Path,
            "json": json,
        }
    }
    
    try:
        import io
        import sys
        import signal
        from contextlib import redirect_stdout, redirect_stderr
        
        stdout_buf = io.StringIO()
        stderr_buf = io.StringIO()
        
        # 切换到工作目录
        import os
        original_cwd = os.getcwd()
        os.chdir(work_dir)
        
        try:
            with redirect_stdout(stdout_buf), redirect_stderr(stderr_buf):
                exec(code, safe_globals, {})
        finally:
            os.chdir(original_cwd)
        
        stdout = stdout_buf.getvalue()
        stderr = stderr_buf.getvalue()
        
        return json.dumps({
            "ok": True,
            "stdout": stdout.strip(),
            "stderr": stderr.strip(),
        }, ensure_ascii=False)
    except Exception as e:
        return json.dumps({
            "ok": False,
            "error": str(e),
        }, ensure_ascii=False)


@register(
    name="exec_shell",
    description="执行系统命令并返回结果。危险的写操作需要用户确认。",
    parameters={
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "description": "要执行的命令"
            },
            "timeout_secs": {
                "type": "integer",
                "description": "超时秒数，默认 60"
            }
        },
        "required": ["command"]
    },
    require_confirm=True
)
async def exec_shell(args: dict, work_dir: str) -> str:
    command = args["command"]
    timeout = args.get("timeout_secs", 60)
    
    try:
        result = subprocess.run(
            command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=work_dir,
        )
        return json.dumps({
            "ok": result.returncode == 0,
            "returncode": result.returncode,
            "stdout": result.stdout.strip()[-5000:],  # 最多 5000 字符
            "stderr": result.stderr.strip()[-2000:],
        }, ensure_ascii=False)
    except subprocess.TimeoutExpired:
        return json.dumps({"error": f"命令超时（{timeout} 秒）"}, ensure_ascii=False)
    except Exception as e:
        return json.dumps({"error": f"执行失败: {e}"}, ensure_ascii=False)
