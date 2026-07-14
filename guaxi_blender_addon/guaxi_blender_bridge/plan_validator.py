"""Build Plan 校验器：在执行器接触 Blender 之前，验证计划的结构与引用完整性。

目的：把"坏计划"挡在 Blender 之外，用清晰的中文错误替代 Blender 内部的堆栈崩溃。
这是生产可靠性的关键一环——LLM 或手工编辑可能产出引用悬空、时长非法的计划。

纯 Python，无 bpy 依赖，可离线单测。返回 (errors, warnings)：
- errors 非空 => 计划不可执行，应终止并报告。
- warnings => 可执行但存在隐患（如某镜头无对象），记录但不阻断。
"""

from __future__ import annotations

from typing import Any


MAX_SCENES = 500
MAX_OBJECTS_PER_SCENE = 200


def _is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _is_vec3(value: Any) -> bool:
    return isinstance(value, list) and len(value) == 3 and all(_is_number(v) for v in value)


def validate_build_plan(plan: Any) -> tuple[list[str], list[str]]:
    """校验 Build Plan，返回 (errors, warnings)。"""
    errors: list[str] = []
    warnings: list[str] = []

    if not isinstance(plan, dict):
        return (["Build Plan 根节点必须是 JSON 对象"], warnings)
    if str(plan.get("schemaVersion")) != "1.0":
        errors.append("Build Plan schemaVersion 必须为 1.0")

    metadata = plan.get("metadata")
    if not isinstance(metadata, dict):
        errors.append("Build Plan.metadata 必须是对象")
    else:
        fps = metadata.get("fps")
        if not _is_number(fps) or not (12 <= float(fps) <= 60):
            errors.append("metadata.fps 必须是 12–60 的数字")

    # 资产表：建立 id 集合，供引用检查。
    asset_ids: set[str] = set()
    assets = plan.get("assets", [])
    if assets and not isinstance(assets, list):
        errors.append("assets 必须是数组")
        assets = []
    for index, asset in enumerate(assets or []):
        if not isinstance(asset, dict):
            errors.append(f"assets[{index}] 不是对象")
            continue
        asset_id = str(asset.get("id") or "").strip()
        if not asset_id:
            errors.append(f"assets[{index}] 缺少 id")
            continue
        if asset_id in asset_ids:
            errors.append(f"assets 存在重复 id：{asset_id}")
        asset_ids.add(asset_id)
        source = str(asset.get("source") or "")
        if source not in {"local", "polyhaven", "sketchfab", "generated", "procedural", "greybox"}:
            errors.append(f"assets[{asset_id}].source 非法：{source}")

    # 角色表：绑定的 assetId 必须存在（除非留空由执行器兜底）。
    character_ids: set[str] = set()
    for index, char in enumerate(plan.get("characters", []) or []):
        if not isinstance(char, dict):
            errors.append(f"characters[{index}] 不是对象")
            continue
        char_id = str(char.get("id") or "").strip()
        if not char_id:
            errors.append(f"characters[{index}] 缺少 id")
            continue
        character_ids.add(char_id)
        bound = str(char.get("assetId") or "").strip()
        if bound and bound not in asset_ids:
            warnings.append(f"角色 {char_id} 绑定的 assetId 不存在：{bound}（将兜底为占位）")

    # 场景与对象。
    scenes = plan.get("scenes")
    if not isinstance(scenes, list) or not scenes:
        errors.append("scenes 必须是非空数组")
        return (errors, warnings)
    if len(scenes) > MAX_SCENES:
        errors.append(f"scenes 数量超过上限 {MAX_SCENES}")

    scene_ids: set[str] = set()
    for s_index, scene in enumerate(scenes):
        label = f"scenes[{s_index}]"
        if not isinstance(scene, dict):
            errors.append(f"{label} 不是对象")
            continue
        scene_id = str(scene.get("id") or "").strip()
        if not scene_id:
            errors.append(f"{label} 缺少 id")
        elif scene_id in scene_ids:
            errors.append(f"场景 id 重复：{scene_id}")
        else:
            scene_ids.add(scene_id)

        duration = scene.get("durationSeconds")
        if not _is_number(duration) or not (0 < float(duration) <= 3600):
            errors.append(f"{label}.durationSeconds 必须在 (0, 3600] 内")

        environment = scene.get("environment")
        if environment is not None and isinstance(environment, dict):
            env_asset = str(environment.get("assetId") or "").strip()
            if env_asset and env_asset not in asset_ids:
                warnings.append(f"{label} 环境 assetId 不存在：{env_asset}（将用背景板兜底）")

        objects = scene.get("objects", [])
        if objects and not isinstance(objects, list):
            errors.append(f"{label}.objects 必须是数组")
            objects = []
        if len(objects or []) > MAX_OBJECTS_PER_SCENE:
            errors.append(f"{label}.objects 数量超过上限 {MAX_OBJECTS_PER_SCENE}")
        if not objects:
            warnings.append(f"{label} 没有对象，画面可能为空")
        for o_index, obj in enumerate(objects or []):
            olabel = f"{label}.objects[{o_index}]"
            if not isinstance(obj, dict):
                errors.append(f"{olabel} 不是对象")
                continue
            if not str(obj.get("id") or "").strip():
                errors.append(f"{olabel} 缺少 id")
            a_ref = str(obj.get("assetId") or "").strip()
            if a_ref and a_ref not in asset_ids:
                warnings.append(f"{olabel} assetId 不存在：{a_ref}（将兜底为占位）")
            c_ref = str(obj.get("characterId") or "").strip()
            if c_ref and c_ref not in character_ids:
                warnings.append(f"{olabel} characterId 不存在：{c_ref}")
            for key in ("location", "rotationEuler", "scale"):
                if key in obj and obj[key] is not None and not _is_vec3(obj[key]):
                    errors.append(f"{olabel}.{key} 必须是长度 3 的数字数组")

        # 相机：至少能渲染需要一个；无相机时执行器会补默认机位，仅告警。
        cameras = scene.get("cameras", [])
        if cameras and not isinstance(cameras, list):
            errors.append(f"{label}.cameras 必须是数组")
        elif not cameras:
            warnings.append(f"{label} 未定义相机，将使用默认机位")

    return (errors, warnings)
