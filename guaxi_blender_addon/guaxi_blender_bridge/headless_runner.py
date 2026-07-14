"""Guaxi 通过 Blender 后台模式启动的受控入口。"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
import traceback
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from scene_builder import (  # noqa: E402
    build_story_asset_preview,
    build_story_preview,
    load_json,
    load_story_ir,
    render_build_plan,
)


# 每个任务的 JSONL 日志句柄；在 main() 中按 --job-id / --project-dir 打开（§11）。
_JOB_LOG_PATH: Path | None = None


def _open_job_log(project_dir: str, job_id: str) -> None:
    """在项目 logs/ 下为本任务建立 <job-id>.jsonl。失败不影响主流程。"""
    global _JOB_LOG_PATH
    try:
        safe_id = "".join(ch for ch in (job_id or "job") if ch.isalnum() or ch in "-_")[:64] or "job"
        log_dir = Path(project_dir).expanduser().resolve() / "logs"
        log_dir.mkdir(parents=True, exist_ok=True)
        _JOB_LOG_PATH = log_dir / f"{safe_id}.jsonl"
    except OSError:
        _JOB_LOG_PATH = None


def _append_job_log(record: dict) -> None:
    if _JOB_LOG_PATH is None:
        return
    try:
        with _JOB_LOG_PATH.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(record, ensure_ascii=False) + "\n")
    except OSError:
        pass  # 日志尽力而为，不能因磁盘问题中断渲染。


def emit_progress(progress: int, stage: str, message: str) -> None:
    payload = {"progress": int(progress), "stage": stage, "message": message}
    print(f"GUAXI_PROGRESS:{json.dumps(payload, ensure_ascii=False)}", flush=True)
    _append_job_log({"ts": time.time(), "type": "progress", **payload})


def parse_arguments() -> argparse.Namespace:
    # Blender 会消费 `--` 前的参数，Guaxi 参数全部放在分隔符之后。
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser(description="Build a Guaxi Story IR preview")
    parser.add_argument("--project-dir", required=True)
    parser.add_argument("--story-ir", help="Story IR JSON 路径（builder 模式需要）")
    parser.add_argument("--build-plan", help="已冻结的 Build Plan JSON 路径（replay 模式需要）")
    parser.add_argument("--mode", choices=("project", "preview"), default="project")
    # builder 选择：
    #   assets  = 默认，Asset Resolver 解析真实资产 + 冻结 Build Plan（§7.3）
    #   greybox = 旧行为，纯彩色立方体预演（兼容与对照）
    #   replay  = 直接重放 --build-plan 指定的冻结计划，确定性重渲染
    parser.add_argument("--builder", choices=("assets", "greybox", "replay"), default="assets")
    parser.add_argument("--job-id", default="", help="任务 ID，用于 logs/<job-id>.jsonl")
    return parser.parse_args(argv)


def resolve_ffmpeg() -> str | None:
    """解析独立 FFmpeg；令牌/网络配置不会传给编码器。"""
    configured = os.environ.get("GUAXI_FFMPEG_PATH", "").strip()
    if configured and Path(configured).is_file():
        return configured
    return shutil.which("ffmpeg")


def encode_preview(outputs: dict[str, str]) -> bool:
    """把已完成的 PNG 序列编码为 H.264；原始帧始终保留用于恢复和最终合成。"""
    ffmpeg = resolve_ffmpeg()
    if not ffmpeg:
        emit_progress(
            99,
            "awaiting-encoder",
            "PNG 图片序列已完成；未检测到 FFmpeg，设置 GUAXI_FFMPEG_PATH 后可生成 MP4",
        )
        return False
    frame_pattern = str(Path(outputs["frameDirectory"]) / "frame_%06d.png")
    preview_path = Path(outputs["previewPath"])
    preview_path.parent.mkdir(parents=True, exist_ok=True)
    command = [
        ffmpeg,
        "-hide_banner",
        "-loglevel",
        "warning",
        "-y",
        "-framerate",
        str(outputs.get("fps", "24")),
        "-start_number",
        "1",
        "-i",
        frame_pattern,
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-movflags",
        "+faststart",
        str(preview_path),
    ]
    emit_progress(98, "encode-preview", "正在使用 FFmpeg 编码 H.264 MP4")
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        reason = (completed.stderr or completed.stdout or "未知 FFmpeg 错误").strip()
        raise RuntimeError(f"FFmpeg 编码失败：{reason[-2000:]}")
    return preview_path.is_file() and preview_path.stat().st_size > 0


def build_outputs(args: argparse.Namespace) -> dict:
    render_animation = args.mode == "preview"
    if args.builder == "replay":
        if not args.build_plan:
            raise ValueError("replay 模式需要 --build-plan 指定已冻结的 Build Plan")
        emit_progress(2, "load-build-plan", "正在读取已冻结的 Build Plan")
        build_plan = load_json(args.build_plan)
        return render_build_plan(build_plan, args.project_dir, render_animation, emit_progress)

    if not args.story_ir:
        raise ValueError("builder 模式需要 --story-ir 指定 Story IR")
    emit_progress(2, "load-story-ir", "正在读取 UTF-8 Story IR")
    story_ir = load_story_ir(args.story_ir)
    if args.builder == "greybox":
        # 兼容旧路径：纯立方体预演，只验证结构。
        return build_story_preview(story_ir, args.project_dir, render_animation, emit_progress)
    # 默认：真实资产解析 + 冻结 Build Plan。
    return build_story_asset_preview(story_ir, args.project_dir, render_animation, emit_progress)


def main() -> int:
    args = parse_arguments()
    _open_job_log(args.project_dir, args.job_id)
    _append_job_log(
        {
            "ts": time.time(),
            "type": "start",
            "mode": args.mode,
            "builder": args.builder,
            "projectDir": args.project_dir,
        }
    )
    outputs = build_outputs(args)
    if args.mode == "preview":
        outputs["previewEncoded"] = encode_preview(outputs)
        if outputs["previewEncoded"]:
            emit_progress(100, "completed", "Story IR 预演与 MP4 编码完成")
        else:
            emit_progress(100, "completed", "Story IR 预演完成，MP4 等待 FFmpeg 编码")
    else:
        emit_progress(100, "completed", "Story IR Blender 工程构建完成")
    print(f"GUAXI_RESULT:{json.dumps(outputs, ensure_ascii=False)}", flush=True)
    _append_job_log({"ts": time.time(), "type": "result", "outputs": outputs})
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # Blender 通过 --python-exit-code 把异常转换为非零退出码。
        emit_progress(0, "failed", f"Blender 执行器失败：{exc}")
        _append_job_log({"ts": time.time(), "type": "failed", "error": str(exc)})
        traceback.print_exc()
        raise
