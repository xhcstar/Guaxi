"""Build Plan 执行器（§7.3）：把冻结的 build_plan.json 逐帧确定性重放为 Blender 场景。

与旧 scene_builder 的关系：
- scene_builder 直接把 Story IR 造成"彩色立方体"，且忽略资产需求。
- build_executor 消费 Build Plan：对每个对象，若其资产解析为真实模型(local/…)则**导入真实网格**，
  仅当资产为 greybox 兜底时才生成彩色占位。这是"消灭立方体"的落地点。

确定性：执行器不含随机数；相同 Build Plan + 相同 Blender 补丁版 + 相同资产文件 => 相同 .blend。
渲染/主时间线相关的生产设置复用自 scene_builder，避免重复实现并保持行为一致。
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Callable

import bpy
from mathutils import Vector

# 兼容两种加载方式：作为 guaxi_blender_bridge 包（交互式 Bridge）用相对导入；
# 作为 headless_runner 加入 sys.path 后的顶层模块用绝对导入。
try:
    from . import scene_builder as sb
except ImportError:  # pragma: no cover - headless top-level import
    import scene_builder as sb  # type: ignore[no-redef]


ProgressCallback = Callable[[int, str, str], None]


def _look_at(obj: bpy.types.Object, target: Any) -> None:
    direction = Vector(target) - obj.location
    obj.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def _asset_tools():
    """兼容插件包模式和 headless 顶层模块模式的延迟导入。"""
    try:
        from .asset_resolver import asset_file_hash
        from .procedural_asset_builder import materialize_procedural_asset
    except ImportError:  # pragma: no cover - headless top-level import
        from asset_resolver import asset_file_hash  # type: ignore[no-redef]
        from procedural_asset_builder import materialize_procedural_asset  # type: ignore[no-redef]
    return asset_file_hash, materialize_procedural_asset


def materialize_assets(
    build_plan: dict[str, Any],
    project_dir: str | Path,
    progress: ProgressCallback | None = None,
) -> None:
    """把待生成配方固化为独立模型文件，并把内容哈希写回内存中的 Build Plan。

    这一步必须发生在 planHash 计算和 build_plan.json 落盘之前，否则首次生成的模型不在
    确定性边界内。已存在的生成模型不会覆盖，用户在 Blender/Python 中的修改会被保留。
    """
    asset_file_hash, materialize_procedural_asset = _asset_tools()
    assets = [item for item in build_plan.get("assets", []) if isinstance(item, dict)]
    procedural = [item for item in assets if item.get("source") == "procedural"]
    missing_procedural = [item for item in procedural if not Path(str(item.get("path") or "")).expanduser().is_file()]
    if missing_procedural:
        # save_as_mainfile(copy=True) 会保存当前 Main 中的全部数据块，所以首次物化前先清空
        # factory-startup 默认立方体/相机；每个资产保存后由生成器清理自己的数据。
        sb._clear_scene()
    total = max(1, len(procedural))
    for index, asset in enumerate(procedural):
        path = Path(str(asset.get("path") or "")).expanduser()
        if not path.is_absolute():
            path = Path(project_dir).expanduser().resolve() / path
        if path.is_file():
            print(
                f"[GuaxiBuildExecutor][INFO] 保留现有可编辑模型：{asset.get('id')} -> {path}",
                flush=True,
            )
        else:
            asset["path"] = str(path.resolve())
            try:
                materialize_procedural_asset(asset)
            except (OSError, RuntimeError, ValueError) as exc:
                # 单个模型物化失败不能拖垮整部预演。保留 generation 配方供后续修复，
                # 本轮明确降级为 greybox 并写 assetGaps，不伪装成已经生成成功。
                message = f"程序化模型写入失败：{asset.get('id')} ({exc})"
                print(f"[GuaxiBuildExecutor][ERROR] {message}", flush=True)
                asset["source"] = "greybox"
                asset["path"] = ""
                asset["importer"] = "none"
                asset["editable"] = False
                build_plan.setdefault("metadata", {}).setdefault("assetGaps", []).append(
                    {"assetId": str(asset.get("id") or ""), "kind": str(asset.get("kind") or ""), "reason": message}
                )
                continue
        asset["contentHash"] = asset_file_hash(path)
        asset["editable"] = True
        if progress is not None:
            progress(
                min(7, 4 + round((index + 1) / total * 3)),
                "materialize-assets",
                f"已准备可编辑模型 {index + 1}/{len(procedural)}：{asset.get('id')}",
            )

    # 外部/本地模型也冻结内容哈希；不存在的文件交给执行阶段输出明确降级日志。
    for asset in assets:
        path_value = str(asset.get("path") or "")
        if not path_value:
            continue
        path = Path(path_value).expanduser()
        if path.is_file() and not asset.get("contentHash"):
            asset["contentHash"] = asset_file_hash(path)


def _verify_frozen_asset_hashes(build_plan: dict[str, Any]) -> None:
    """拒绝使用内容已漂移的模型重放旧计划，提示用户显式启用修改版本。"""
    asset_file_hash, _materialize = _asset_tools()
    changed: list[str] = []
    for asset in build_plan.get("assets", []):
        if not isinstance(asset, dict):
            continue
        expected = str(asset.get("contentHash") or "")
        path_value = str(asset.get("path") or "")
        if not expected or not path_value:
            continue
        path = Path(path_value).expanduser()
        if path.is_file() and asset_file_hash(path) != expected:
            changed.append(f"{asset.get('id')} ({path})")
    if changed:
        preview = "；".join(changed[:8])
        print(f"[GuaxiBuildExecutor][WARN] 检测到模型内容已修改：{preview}", flush=True)
        raise ValueError(
            "检测到模型已被 Blender/Python 修改，旧 Build Plan 已失效："
            f"{preview}。请在 Guaxi 点击“启用已修改模型”重新冻结计划。"
        )


def _import_asset(asset: dict[str, Any]) -> bpy.types.Object | None:
    """按 importer 导入资产文件，返回一个代表对象（父空物或首个网格）。

    失败时返回 None，让调用方降级为灰盒。导入被限制为读取已解析路径的确定性操作，
    不执行任何来自计划的表达式或 Python。
    """
    importer = str(asset.get("importer") or "none")
    path = str(asset.get("path") or "")
    if importer == "none" or not path or not Path(path).is_file():
        return None

    before = set(bpy.data.objects)
    try:
        if importer == "gltf":
            bpy.ops.import_scene.gltf(filepath=path)
        elif importer == "fbx":
            bpy.ops.import_scene.fbx(filepath=path)
        elif importer == "obj":
            bpy.ops.wm.obj_import(filepath=path)
        elif importer == "usd":
            bpy.ops.wm.usd_import(filepath=path)
        elif importer == "blend":
            # 只追加 blend 中的对象数据，避免整档链接带来的副作用。
            with bpy.data.libraries.load(path, link=False) as (data_from, data_to):
                data_to.objects = list(data_from.objects)
            for obj in data_to.objects:
                if obj is not None:
                    bpy.context.collection.objects.link(obj)
        else:
            return None
    except (RuntimeError, OSError) as exc:
        sb_log = f"资产导入失败，降级为灰盒：{path} ({exc})"
        print(f"[GuaxiBuildExecutor][WARN] {sb_log}", flush=True)
        return None

    new_objects = [obj for obj in bpy.data.objects if obj not in before]
    if not new_objects:
        return None
    unit_scale = float(asset.get("unitScale") or 1.0)
    # 用一个空物作为统一句柄，把导入结果打包，便于 transform/keyframe 作用于整体。
    root = bpy.data.objects.new(f"IMPORT_{asset.get('id', 'asset')}", None)
    bpy.context.collection.objects.link(root)
    for obj in new_objects:
        if obj.parent is None:
            obj.parent = root
    if unit_scale != 1.0:
        root.scale = (unit_scale, unit_scale, unit_scale)
    return root


def _greybox_object(name: str, color: Any) -> bpy.types.Object:
    bpy.ops.mesh.primitive_cube_add(location=(0.0, 0.0, 1.05), scale=(0.55, 0.45, 1.05))
    obj = bpy.context.object
    obj.name = name
    rgba = list(color) if isinstance(color, (list, tuple)) and len(color) == 4 else [0.7, 0.7, 0.75, 1.0]
    obj.data.materials.append(sb._material(f"MAT_{name}", tuple(rgba)))
    return obj


def _apply_keyframes(obj: bpy.types.Object, keyframes: list[dict[str, Any]], base_frame: int) -> None:
    for kf in keyframes:
        if not isinstance(kf, dict):
            continue
        frame = base_frame + int(kf.get("frameOffset", 0))
        if "location" in kf and isinstance(kf["location"], list):
            obj.location = kf["location"]
            obj.keyframe_insert(data_path="location", frame=frame)
        if "rotationEuler" in kf and isinstance(kf["rotationEuler"], list):
            obj.rotation_euler = kf["rotationEuler"]
            obj.keyframe_insert(data_path="rotation_euler", frame=frame)
        if "scale" in kf and isinstance(kf["scale"], list):
            obj.scale = kf["scale"]
            obj.keyframe_insert(data_path="scale", frame=frame)


def _place_object(spec: dict[str, Any], asset: dict[str, Any] | None, base_frame: int) -> bpy.types.Object:
    obj = _import_asset(asset) if asset else None
    if obj is None:
        color = (asset or {}).get("displayColor", [0.7, 0.7, 0.75, 1.0])
        obj = _greybox_object(str(spec.get("id") or "GUAXI_OBJECT"), color)
    else:
        obj.name = str(spec.get("id") or obj.name)

    if isinstance(spec.get("location"), list):
        obj.location = spec["location"]
    if isinstance(spec.get("rotationEuler"), list):
        obj.rotation_euler = spec["rotationEuler"]
    if isinstance(spec.get("scale"), list):
        # 导入资产使用 Empty 作为统一根节点；同样必须应用实例缩放。乘上导入器设置的
        # unitScale，而不是覆盖它，避免厘米资产被错误放大为米。
        obj.scale = tuple(float(spec["scale"][i]) * float(obj.scale[i]) for i in range(3))

    keyframes = spec.get("keyframes") if isinstance(spec.get("keyframes"), list) else []
    if keyframes:
        _apply_keyframes(obj, keyframes, base_frame)
    return obj


def _add_camera(spec: dict[str, Any], scene: bpy.types.Scene, base_frame: int) -> bpy.types.Object:
    bpy.ops.object.camera_add(location=spec.get("location", [0.0, -13.5, 4.2]))
    camera = bpy.context.object
    camera.name = str(spec.get("id") or "GUAXI_CAMERA")
    camera.data.lens = float(spec.get("lens") or 48)
    if isinstance(spec.get("rotationEuler"), list):
        camera.rotation_euler = spec["rotationEuler"]
    elif isinstance(spec.get("lookAt"), list):
        _look_at(camera, spec["lookAt"])
    if spec.get("active", True) and scene.camera is None:
        scene.camera = camera
    _apply_keyframes(camera, spec.get("keyframes") if isinstance(spec.get("keyframes"), list) else [], base_frame)
    return camera


def _add_light(spec: dict[str, Any]) -> bpy.types.Object:
    light_type = str(spec.get("type") or "AREA")
    bpy.ops.object.light_add(type=light_type, location=spec.get("location", [0.0, 0.0, 6.0]))
    light = bpy.context.object
    light.name = str(spec.get("id") or "GUAXI_LIGHT")
    light.data.energy = float(spec.get("energy") or 1000)
    if isinstance(spec.get("color"), list) and len(spec["color"]) == 3:
        light.data.color = spec["color"]
    if light_type == "AREA" and spec.get("size"):
        light.data.size = float(spec["size"])
    if isinstance(spec.get("lookAt"), list):
        _look_at(light, spec["lookAt"])
    return light


def build_from_plan(
    build_plan: dict[str, Any],
    project_dir: str | Path,
    render_animation: bool = False,
    progress: ProgressCallback | None = None,
) -> dict[str, str]:
    """确定性重放 Build Plan，产出 .blend / PNG 序列，返回输出路径。"""

    def report(value: int, stage: str, message: str) -> None:
        if progress is not None:
            progress(value, stage, message)

    # 执行前校验：坏计划在此被拦截并给出清晰错误，而不是在 Blender 内部崩溃。
    try:
        from . import plan_validator
    except ImportError:  # pragma: no cover - headless top-level import
        import plan_validator  # type: ignore[no-redef]
    errors, plan_warnings = plan_validator.validate_build_plan(build_plan)
    if errors:
        raise ValueError("Build Plan 校验失败：" + "；".join(errors[:10]))
    for warning in plan_warnings[:20]:
        report(3, "plan-warning", f"Build Plan 提示：{warning}")

    _verify_frozen_asset_hashes(build_plan)

    scenes_spec = build_plan.get("scenes") if isinstance(build_plan.get("scenes"), list) else []
    if not scenes_spec:
        raise ValueError("Build Plan 没有可构建的 scenes")

    report(5, "scene-reset", "正在清理 Blender 默认场景")
    sb._clear_scene()

    project_path = Path(project_dir).expanduser().resolve()
    blend_path = project_path / "blender" / "preview.blend"
    preview_path = project_path / "renders" / "preview" / "preview.mp4"
    frame_directory = project_path / "renders" / "preview" / "frames"
    frame_prefix = frame_directory / "frame_######"
    for directory in (blend_path.parent, preview_path.parent, frame_directory):
        directory.mkdir(parents=True, exist_ok=True)

    metadata = build_plan.get("metadata") if isinstance(build_plan.get("metadata"), dict) else {}
    fps = int(sb._safe_number(metadata.get("fps"), 24, 12, 60))
    render_spec = build_plan.get("render") if isinstance(build_plan.get("render"), dict) else {}

    scene = bpy.context.scene
    scene.name = "GUAXI_3D_SOURCE"
    scene.render.fps = fps
    scene.frame_start = 1
    world_color = render_spec.get("worldColor", [0.015, 0.022, 0.04])
    if scene.world is None:
        scene.world = bpy.data.worlds.new("GUAXI_WORLD")
    scene.world.color = tuple(world_color[:3])

    # 公共地面（环境资产未覆盖地面时的兜底舞台）。
    bpy.ops.mesh.primitive_plane_add(size=30, location=(0.0, 0.0, 0.0))
    ground = bpy.context.object
    ground.name = "GUAXI_STAGE_GROUND"
    ground.data.materials.append(sb._material("MAT_GUAXI_GROUND", (0.035, 0.045, 0.07, 1.0)))

    assets_by_id = {a["id"]: a for a in build_plan.get("assets", []) if isinstance(a, dict) and a.get("id")}

    current_frame = 1
    max_preview_frames = fps * 180
    total = len(scenes_spec)
    for scene_index, scene_spec in enumerate(scenes_spec):
        if current_frame >= max_preview_frames:
            report(70, "timeline", "预演达到 180 秒上限，剩余场景保留在 Build Plan 中")
            break
        duration = sb._safe_number(scene_spec.get("durationSeconds"), 5.0, 1.0, 120.0)
        duration_frames = max(1, round(duration * fps))
        start_frame = current_frame
        end_frame = min(max_preview_frames, current_frame + duration_frames - 1)

        # 时间线标记。镜头相机绑定在相机创建后补充，普通标记先在这里创建。
        for marker in scene_spec.get("markers", []):
            if isinstance(marker, dict):
                frame = start_frame + int(marker.get("frameOffset", 0))
                scene.timeline_markers.new(str(marker.get("name") or "marker"), frame=min(frame, end_frame))

        # 环境资产（真实模型或背景板兜底）
        environment = scene_spec.get("environment") if isinstance(scene_spec.get("environment"), dict) else {}
        env_asset = assets_by_id.get(str(environment.get("assetId") or ""))
        env_object = _import_asset(env_asset) if env_asset else None
        if env_object is None:
            # 背景板兜底，保留旧预演的场景切换观感。
            hue = (scene_index % 5) * 0.025
            bpy.ops.mesh.primitive_cube_add(location=(0.0, 2.4, 2.2), scale=(5.8, 0.25, 2.8))
            env_object = bpy.context.object
            env_object.data.materials.append(
                sb._material(f"MAT_ENV_{scene_index:03d}", (0.055 + hue, 0.08 + hue, 0.14 + hue, 1.0))
            )
        env_object.name = f"GUAXI_ENV_{sb._slug(scene_spec.get('id'), str(scene_index))}"
        sb._keyframe_visibility(env_object, start_frame, end_frame)

        # 对象（角色/道具）
        for obj_spec in scene_spec.get("objects", []):
            if not isinstance(obj_spec, dict):
                continue
            asset = assets_by_id.get(str(obj_spec.get("assetId") or ""))
            placed = _place_object(obj_spec, asset, start_frame)
            sb._keyframe_visibility(placed, start_frame, end_frame)

        # 相机与灯光（每个场景独立）。TimelineMarker.camera 让 Blender 在镜头起始帧
        # 自动切换相机，避免整部视频只使用第一个场景的固定机位。
        scene_cameras: list[tuple[dict[str, Any], bpy.types.Object]] = []
        for cam_spec in scene_spec.get("cameras", []):
            if isinstance(cam_spec, dict):
                camera = _add_camera(cam_spec, scene, start_frame)
                scene_cameras.append((cam_spec, camera))
        for cam_index, (cam_spec, camera) in enumerate(scene_cameras):
            camera_frame = min(end_frame, start_frame + max(0, int(cam_spec.get("frameOffset", 0))))
            marker = scene.timeline_markers.new(
                f"CAM · {cam_spec.get('id') or cam_index + 1}",
                frame=camera_frame,
            )
            marker.camera = camera
        for light_spec in scene_spec.get("lights", []):
            if isinstance(light_spec, dict):
                light = _add_light(light_spec)
                sb._keyframe_visibility(light, start_frame, end_frame)

        current_frame = end_frame + 1
        report(
            min(65, 10 + round((scene_index + 1) / max(1, total) * 55)),
            "story-layout",
            f"已构建场景 {scene_index + 1}/{total}：{sb._slug(scene_spec.get('title'), '场景')}",
        )

    scene.frame_end = max(1, current_frame - 1)
    if scene.camera is None:
        # 保证至少有一个可渲染相机。
        _add_camera(_fallback_camera(), scene, 1)

    sb._configure_render(scene, build_plan_to_story_shim(build_plan), frame_prefix)
    scene["guaxi_plan_hash"] = str(metadata.get("planHash", ""))
    scene["guaxi_story_ir_hash"] = str(metadata.get("storyIrHash", ""))
    scene["guaxi_project_dir"] = str(project_path)
    scene["guaxi_story_title"] = str(metadata.get("title", "未命名影像项目"))

    report(69, "master-timeline", "正在创建 Blender VSE 主时间线")
    master_scene = sb._create_master_timeline(scene, build_plan_to_story_shim(build_plan), frame_prefix, fps)
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


def _fallback_camera() -> dict[str, Any]:
    return {"id": "GUAXI_CAMERA_FALLBACK", "location": [0.0, -13.5, 4.2], "lookAt": [0.0, 0.3, 1.5], "lens": 48}


def build_plan_to_story_shim(build_plan: dict[str, Any]) -> dict[str, Any]:
    """为复用 scene_builder 的渲染/时间线设置，提供其所需的最小 story_ir 形状。

    scene_builder._configure_render / _create_master_timeline 只读 production.resolution、
    metadata.title、schemaVersion，这里做一个薄适配，避免复制那部分成熟代码。
    """
    metadata = build_plan.get("metadata") if isinstance(build_plan.get("metadata"), dict) else {}
    render_spec = build_plan.get("render") if isinstance(build_plan.get("render"), dict) else {}
    resolution: dict[str, Any] = {}
    if render_spec.get("resolutionX"):
        resolution["width"] = render_spec["resolutionX"]
    if render_spec.get("resolutionY"):
        resolution["height"] = render_spec["resolutionY"]
    if render_spec.get("resolutionPercentage"):
        resolution["percentage"] = render_spec["resolutionPercentage"]
    return {
        "schemaVersion": "1.0",
        "metadata": {"title": metadata.get("title", "未命名影像项目")},
        "production": {"resolution": resolution},
    }
