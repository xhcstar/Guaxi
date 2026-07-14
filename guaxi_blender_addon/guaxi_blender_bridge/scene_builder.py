"""Story IR 到 Blender 5.2 LTS 低模预演场景的确定性构建器。

这里刻意不包含大模型调用，也不接受 Python 字符串。所有输入都是普通 JSON 数据，
从而让相同 Story IR 在同一个 Blender 补丁版本上得到可复现的预演工程。
"""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any, Callable

import bpy
from mathutils import Vector


ProgressCallback = Callable[[int, str, str], None]


def _safe_number(value: Any, fallback: float, minimum: float, maximum: float) -> float:
    """把模型输出的宽松数字限制在生产安全范围内。"""
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return fallback
    return max(minimum, min(maximum, parsed))


def _slug(value: Any, fallback: str) -> str:
    """生成稳定的 Blender 数据块名称，保留中文但移除路径/控制字符。"""
    text = str(value or fallback).strip()
    cleaned = "".join(ch for ch in text if ch not in '<>:"/\\|?*' and ch.isprintable())
    return (cleaned or fallback)[:80]


def _look_at(obj: bpy.types.Object, target: tuple[float, float, float]) -> None:
    direction = Vector(target) - obj.location
    obj.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def _material(name: str, color: tuple[float, float, float, float]) -> bpy.types.Material:
    material = bpy.data.materials.new(name=name)
    material.diffuse_color = color
    material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF")
    if principled is not None:
        principled.inputs["Base Color"].default_value = color
        principled.inputs["Roughness"].default_value = 0.65
    return material


def _clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for data_collection in (bpy.data.materials, bpy.data.cameras, bpy.data.lights):
        for block in list(data_collection):
            if block.users == 0:
                data_collection.remove(block)


def _keyframe_visibility(obj: bpy.types.Object, start_frame: int, end_frame: int) -> None:
    """用 hide_render 关键帧隔离场景，避免为每个分镜建立独立 .blend。"""
    if start_frame > 1:
        obj.hide_render = True
        obj.keyframe_insert(data_path="hide_render", frame=start_frame - 1)
    obj.hide_render = False
    obj.keyframe_insert(data_path="hide_render", frame=start_frame)
    obj.hide_render = False
    obj.keyframe_insert(data_path="hide_render", frame=end_frame)
    obj.hide_render = True
    obj.keyframe_insert(data_path="hide_render", frame=end_frame + 1)


def _create_actor_placeholder(
    actor_id: str,
    actor_index: int,
    scene_index: int,
    start_frame: int,
    end_frame: int,
) -> bpy.types.Object:
    x = (actor_index - 1) * 2.0
    bpy.ops.mesh.primitive_cube_add(location=(x, 0.2, 1.05), scale=(0.55, 0.45, 1.05))
    actor = bpy.context.object
    actor.name = f"GUAXI_ACTOR_{_slug(actor_id, str(actor_index))}_{scene_index + 1:03d}"
    palette = [
        (0.24, 0.55, 0.95, 1.0),
        (0.94, 0.42, 0.34, 1.0),
        (0.34, 0.78, 0.52, 1.0),
        (0.76, 0.48, 0.92, 1.0),
    ]
    actor.data.materials.append(_material(f"MAT_{actor.name}", palette[actor_index % len(palette)]))
    _keyframe_visibility(actor, start_frame, end_frame)

    # 占位动作不是最终角色动画，只用于让预演能观察人物运动方向和镜头节奏。
    actor.location.x = x - 0.35
    actor.keyframe_insert(data_path="location", frame=start_frame)
    actor.location.x = x + 0.35
    actor.keyframe_insert(data_path="location", frame=end_frame)
    return actor


def _configure_render(scene: bpy.types.Scene, story_ir: dict[str, Any], frame_prefix: Path) -> None:
    production = story_ir.get("production") if isinstance(story_ir.get("production"), dict) else {}
    resolution = production.get("resolution") if isinstance(production.get("resolution"), dict) else {}
    # Blender 5.2 将 Eevee 枚举统一为 BLENDER_EEVEE；部分早期版本使用
    # BLENDER_EEVEE_NEXT。运行时读取 RNA 枚举可以让预发布补丁变化产生明确降级日志，
    # 而不是在设置渲染器时直接终止整个项目。
    engine_items = scene.render.bl_rna.properties["engine"].enum_items
    available_engines = {item.identifier for item in engine_items}
    if "BLENDER_EEVEE" in available_engines:
        scene.render.engine = "BLENDER_EEVEE"
    elif "BLENDER_EEVEE_NEXT" in available_engines:
        scene.render.engine = "BLENDER_EEVEE_NEXT"
    elif "BLENDER_WORKBENCH" in available_engines:
        scene.render.engine = "BLENDER_WORKBENCH"
    else:
        raise RuntimeError(f"没有可用的 Eevee/Workbench 渲染引擎：{sorted(available_engines)}")
    scene.render.resolution_x = int(_safe_number(resolution.get("width"), 1280, 320, 3840))
    scene.render.resolution_y = int(_safe_number(resolution.get("height"), 720, 240, 2160))
    # 预演强制限制到 50%，最终渲染阶段再使用完整分辨率。
    scene.render.resolution_percentage = int(_safe_number(resolution.get("percentage"), 50, 10, 50))
    # 生产流程始终先渲染图片序列。这样 Blender/编码器在任意帧失败时都能断点续作，
    # 也规避 Blender 5.2 RC 中 FFMPEG 静态枚举可见但动态赋值被拒绝的问题。
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.image_settings.color_depth = "8"
    scene.render.use_file_extension = True
    scene.render.filepath = str(frame_prefix)
    try:
        scene.view_settings.look = "AgX - Medium High Contrast"
    except (TypeError, ValueError):
        # 预发布版本可能调整 Look 枚举；保持 AgX 默认配置即可继续生成预演。
        pass


def _create_master_timeline(
    source_scene: bpy.types.Scene,
    story_ir: dict[str, Any],
    frame_prefix: Path,
    fps: int,
) -> bpy.types.Scene:
    """创建生产用 VSE Master Scene。

    3D 预演作为 Scene Strip 放在频道 1；后续 Pallaidium/ComfyUI 视频、对白、音乐、
    字幕可以写入更高频道，不会破坏原始 3D 场景和关键帧。
    """
    existing = bpy.data.scenes.get("GUAXI_MASTER_EDIT")
    if existing is not None:
        bpy.data.scenes.remove(existing)
    master = bpy.data.scenes.new("GUAXI_MASTER_EDIT")
    master.frame_start = source_scene.frame_start
    master.frame_end = source_scene.frame_end
    master.render.fps = fps
    _configure_render(master, story_ir, frame_prefix)
    master.render.use_sequencer = True

    editor = master.sequence_editor_create()
    strip = editor.strips.new_scene(
        "GUAXI_3D_PREVIEW",
        source_scene,
        channel=1,
        frame_start=source_scene.frame_start,
    )
    strip["guaxi_media_role"] = "3d-preview"
    strip["guaxi_story_ir_schema"] = str(story_ir.get("schemaVersion", "1.0"))
    strip["guaxi_revision"] = 1
    metadata = story_ir.get("metadata") if isinstance(story_ir.get("metadata"), dict) else {}
    strip["guaxi_story_title"] = str(metadata.get("title", "未命名影像项目"))

    # Master 时间线复制场景/镜头标记，剪辑时无需切回 3D Source Scene。
    for marker in source_scene.timeline_markers:
        master.timeline_markers.new(marker.name, frame=marker.frame)
    master["guaxi_master_timeline"] = True
    master["guaxi_channel_1"] = "3D scene strips"
    master["guaxi_channels_10_19"] = "generated/reference video"
    master["guaxi_channels_20_29"] = "dialogue/narration"
    master["guaxi_channels_30_39"] = "music/sfx"
    master["guaxi_channels_40_49"] = "subtitle/title/map overlay"
    return master


def build_story_preview(
    story_ir: dict[str, Any],
    project_dir: str | Path,
    render_animation: bool = False,
    progress: ProgressCallback | None = None,
) -> dict[str, str]:
    """构建低模预演并返回输出路径。

    当前阶段用几何占位符验证叙事结构。后续资产解析器可以按稳定对象 ID 替换人物和场景，
    不需要让大模型重新生成 Blender Python。
    """

    def report(value: int, stage: str, message: str) -> None:
        if progress is not None:
            progress(value, stage, message)

    report(5, "scene-reset", "正在清理 Blender 默认场景")
    _clear_scene()
    project_path = Path(project_dir).expanduser().resolve()
    blend_path = project_path / "blender" / "preview.blend"
    preview_path = project_path / "renders" / "preview" / "preview.mp4"
    frame_directory = project_path / "renders" / "preview" / "frames"
    # 显式使用 6 位帧号，避免 Blender 默认 4 位编号与 FFmpeg 输入模板不一致。
    frame_prefix = frame_directory / "frame_######"
    blend_path.parent.mkdir(parents=True, exist_ok=True)
    preview_path.parent.mkdir(parents=True, exist_ok=True)
    frame_directory.mkdir(parents=True, exist_ok=True)

    scene = bpy.context.scene
    scene.name = "GUAXI_3D_SOURCE"
    metadata = story_ir.get("metadata") if isinstance(story_ir.get("metadata"), dict) else {}
    fps = int(_safe_number(metadata.get("fps"), 24, 12, 60))
    scene.render.fps = fps
    scene.frame_start = 1
    scene.world.color = (0.015, 0.022, 0.04)

    # 摄像机、主光和地面属于整个预演工程的公共舞台。
    bpy.ops.object.camera_add(location=(0.0, -13.5, 4.2))
    camera = bpy.context.object
    camera.name = "GUAXI_CAMERA_PREVIEW"
    camera.data.lens = 48
    _look_at(camera, (0.0, 0.3, 1.5))
    scene.camera = camera

    bpy.ops.object.light_add(type="AREA", location=(2.5, -4.0, 7.0))
    key_light = bpy.context.object
    key_light.name = "GUAXI_LIGHT_KEY"
    key_light.data.energy = 1100
    key_light.data.shape = "DISK"
    key_light.data.size = 5.0
    _look_at(key_light, (0.0, 0.0, 1.0))

    bpy.ops.mesh.primitive_plane_add(size=30, location=(0.0, 0.0, 0.0))
    ground = bpy.context.object
    ground.name = "GUAXI_STAGE_GROUND"
    ground.data.materials.append(_material("MAT_GUAXI_GROUND", (0.035, 0.045, 0.07, 1.0)))

    scenes = story_ir.get("scenes") if isinstance(story_ir.get("scenes"), list) else []
    if not scenes:
        raise ValueError("Story IR 没有可构建的 scenes")

    current_frame = 1
    max_preview_frames = fps * 180
    for scene_index, scene_spec in enumerate(scenes[:500]):
        if current_frame >= max_preview_frames:
            report(70, "timeline", "预演达到 180 秒上限，剩余场景保留在 Story IR 中")
            break
        scene_spec = scene_spec if isinstance(scene_spec, dict) else {}
        duration_seconds = _safe_number(scene_spec.get("durationSeconds"), 5.0, 1.0, 120.0)
        duration_frames = max(1, round(duration_seconds * fps))
        start_frame = current_frame
        end_frame = min(max_preview_frames, current_frame + duration_frames - 1)
        scene_id = _slug(scene_spec.get("id"), f"scene_{scene_index + 1:03d}")
        title = _slug(scene_spec.get("title"), f"场景 {scene_index + 1}")
        scene.timeline_markers.new(f"{scene_id} · {title}", frame=start_frame)

        # 背景板颜色让相邻场景在预演中有清晰切换，即使尚未匹配正式环境资产。
        bpy.ops.mesh.primitive_cube_add(location=(0.0, 2.4, 2.2), scale=(5.8, 0.25, 2.8))
        backdrop = bpy.context.object
        backdrop.name = f"GUAXI_ENV_{scene_id}"
        hue_shift = (scene_index % 5) * 0.025
        backdrop.data.materials.append(
            _material(
                f"MAT_{backdrop.name}",
                (0.055 + hue_shift, 0.08 + hue_shift, 0.14 + hue_shift, 1.0),
            )
        )
        _keyframe_visibility(backdrop, start_frame, end_frame)

        shots = scene_spec.get("shots") if isinstance(scene_spec.get("shots"), list) else []
        actor_ids: list[str] = []
        for shot in shots:
            if not isinstance(shot, dict):
                continue
            for actor_id in shot.get("characters", []):
                actor_text = str(actor_id).strip()
                if actor_text and actor_text not in actor_ids:
                    actor_ids.append(actor_text)
        if not actor_ids:
            actor_ids = ["narrator-placeholder"]
        for actor_index, actor_id in enumerate(actor_ids[:4]):
            _create_actor_placeholder(actor_id, actor_index, scene_index, start_frame, end_frame)

        # 镜头节点先转成时间线标记，后续相机系统可以在同一帧位置生成真实镜头。
        shot_frame = start_frame
        for shot_index, shot in enumerate(shots):
            if not isinstance(shot, dict):
                continue
            shot_id = _slug(shot.get("id"), f"shot_{shot_index + 1:03d}")
            scene.timeline_markers.new(shot_id, frame=min(shot_frame, end_frame))
            shot_seconds = _safe_number(shot.get("durationSeconds"), 2.0, 0.25, 60.0)
            shot_frame += max(1, round(shot_seconds * fps))

        current_frame = end_frame + 1
        report(
            min(65, 10 + round((scene_index + 1) / max(1, len(scenes)) * 55)),
            "story-layout",
            f"已构建场景 {scene_index + 1}/{len(scenes)}：{title}",
        )

    scene.frame_end = max(1, current_frame - 1)
    _configure_render(scene, story_ir, frame_prefix)
    scene["guaxi_story_ir_schema"] = str(story_ir.get("schemaVersion", "1.0"))
    scene["guaxi_project_dir"] = str(project_path)
    scene["guaxi_story_title"] = str(metadata.get("title", "未命名影像项目"))

    report(69, "master-timeline", "正在创建 Blender VSE 主时间线")
    master_scene = _create_master_timeline(scene, story_ir, frame_prefix, fps)
    # 后台模式同样拥有 Window 上下文；切到 Master 后保存/渲染，打开 .blend 即进入主剪辑场景。
    if bpy.context.window is not None:
        bpy.context.window.scene = master_scene

    report(72, "save-project", "正在保存 Blender 预演工程")
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)
    if render_animation:
        report(76, "render-preview", "正在渲染可断点恢复的 PNG 图片序列")
        bpy.ops.render.render(animation=True, scene=master_scene.name)
        report(98, "save-project", "正在写入最终预演工程状态")
        bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)

    report(96, "frames-completed", "Blender 工程与图片序列构建完成")
    return {
        "blendPath": str(blend_path),
        "previewPath": str(preview_path),
        "frameDirectory": str(frame_directory),
        "fps": str(fps),
    }


def load_story_ir(path: str | Path) -> dict[str, Any]:
    with Path(path).open("r", encoding="utf-8-sig") as stream:
        value = json.load(stream)
    if not isinstance(value, dict):
        raise ValueError("Story IR 根节点必须是 JSON 对象")
    return value


def load_json(path: str | Path) -> dict[str, Any]:
    """通用 UTF-8(-sig) JSON 读取，供 Build Plan 等工件复用。"""
    with Path(path).open("r", encoding="utf-8-sig") as stream:
        value = json.load(stream)
    if not isinstance(value, dict):
        raise ValueError(f"JSON 根节点必须是对象：{path}")
    return value


def build_story_asset_preview(
    story_ir: dict[str, Any],
    project_dir: str | Path,
    render_animation: bool = False,
    progress: ProgressCallback | None = None,
) -> dict[str, str]:
    """确定性资产化预演（§7.2/§7.3）：Story IR → 冻结 Build Plan → 真实资产重放。

    与 build_story_preview 的区别：本函数经 Asset Resolver 尝试用真实模型替换立方体，
    并把冻结的 build_plan.json 写入项目 story/ 目录，供后续确定性重渲染与审计。
    资产未命中时逐级降级，最差回落到灰盒，保证不会整片失败。
    """
    # 延迟导入：scene_director / build_executor 依赖 asset_resolver，且 build_executor 依赖 bpy。
    build_executor, scene_director, load_resolver_from_env = _import_pipeline()

    def report(value: int, stage: str, message: str) -> None:
        if progress is not None:
            progress(value, stage, message)

    project_path = Path(project_dir).expanduser().resolve()
    report(4, "resolve-assets", "正在解析场景资产并编译确定性 Build Plan")
    resolver = load_resolver_from_env(project_path)
    build_plan = scene_director.compile_build_plan(story_ir, resolver)

    # 冻结 Build Plan：一经写入即为确定性重放依据（§7.3）。
    plan_path = project_path / "story" / "build_plan.json"
    plan_path.parent.mkdir(parents=True, exist_ok=True)
    with plan_path.open("w", encoding="utf-8") as stream:
        json.dump(build_plan, stream, ensure_ascii=False, indent=2)

    gap_count = len(build_plan.get("metadata", {}).get("assetGaps", []))
    local_count = sum(1 for a in build_plan.get("assets", []) if a.get("source") == "local")
    report(
        8,
        "plan-frozen",
        f"Build Plan 已冻结：真实资产 {local_count} 个，灰盒兜底 {gap_count} 个",
    )

    # 写出合规工件（许可账本 / 模型使用 / AIGC 标识），供发布前检查（§22）。
    try:
        from . import compliance as _compliance
    except ImportError:
        import compliance as _compliance  # type: ignore[no-redef]
    try:
        compliance_paths = _compliance.write_compliance_artifacts(build_plan, project_path)
        report(9, "compliance", f"已生成合规工件 {len(compliance_paths)} 项（含 AIGC 标识）")
    except OSError as exc:
        report(9, "compliance-warning", f"合规工件写入失败（不阻断预演）：{exc}")

    outputs = build_executor.build_from_plan(
        build_plan, project_path, render_animation=render_animation, progress=progress
    )
    outputs["buildPlanPath"] = str(plan_path)
    outputs["assetGapCount"] = str(gap_count)
    return outputs


def render_build_plan(
    build_plan: dict[str, Any],
    project_dir: str | Path,
    render_animation: bool = False,
    progress: ProgressCallback | None = None,
) -> dict[str, str]:
    """直接重放一个已冻结的 Build Plan（确定性渲染，不再解析或调用 LLM）。"""
    build_executor, _director, _loader = _import_pipeline()

    return build_executor.build_from_plan(
        build_plan, project_dir, render_animation=render_animation, progress=progress
    )


def _import_pipeline():
    """在插件包模式与 headless 顶层脚本模式下都能导入资产管线模块。

    - 交互式 Bridge：模块作为 `guaxi_blender_bridge` 包加载，用相对导入。
    - 后台 runner：headless_runner 把目录加入 sys.path 后以顶层名导入，无包上下文，
      relative import 会失败，因此回退到绝对导入。
    """
    try:
        from . import build_executor as be
        from . import scene_director as sd
        from .asset_resolver import load_resolver_from_env as loader
    except ImportError:
        import build_executor as be  # type: ignore[no-redef]
        import scene_director as sd  # type: ignore[no-redef]
        from asset_resolver import load_resolver_from_env as loader  # type: ignore[no-redef]
    return be, sd, loader
