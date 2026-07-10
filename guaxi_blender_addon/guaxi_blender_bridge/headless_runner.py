"""Guaxi 通过 Blender 后台模式启动的受控入口。"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import traceback
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from scene_builder import build_story_preview, load_story_ir  # noqa: E402


def emit_progress(progress: int, stage: str, message: str) -> None:
    payload = {"progress": int(progress), "stage": stage, "message": message}
    print(f"GUAXI_PROGRESS:{json.dumps(payload, ensure_ascii=False)}", flush=True)


def parse_arguments() -> argparse.Namespace:
    # Blender 会消费 `--` 前的参数，Guaxi 参数全部放在分隔符之后。
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser(description="Build a Guaxi Story IR preview")
    parser.add_argument("--project-dir", required=True)
    parser.add_argument("--story-ir", required=True)
    parser.add_argument("--mode", choices=("project", "preview"), default="project")
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


def main() -> int:
    args = parse_arguments()
    emit_progress(2, "load-story-ir", "正在读取 UTF-8 Story IR")
    story_ir = load_story_ir(args.story_ir)
    outputs = build_story_preview(
        story_ir,
        args.project_dir,
        render_animation=args.mode == "preview",
        progress=emit_progress,
    )
    if args.mode == "preview":
        outputs["previewEncoded"] = encode_preview(outputs)
        if outputs["previewEncoded"]:
            emit_progress(100, "completed", "Story IR 预演与 MP4 编码完成")
        else:
            emit_progress(100, "completed", "Story IR 预演完成，MP4 等待 FFmpeg 编码")
    else:
        emit_progress(100, "completed", "Story IR Blender 工程构建完成")
    print(f"GUAXI_RESULT:{json.dumps(outputs, ensure_ascii=False)}", flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # Blender 通过 --python-exit-code 把异常转换为非零退出码。
        emit_progress(0, "failed", f"Blender 执行器失败：{exc}")
        traceback.print_exc()
        raise
