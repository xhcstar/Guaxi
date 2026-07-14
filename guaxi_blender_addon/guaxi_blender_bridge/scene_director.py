"""Scene Director：Story IR -> 确定性 Build Plan 编译器（§7.2 / §7.3）。

职责：把"叙事意图"（Story IR，随机 LLM 产物）编译为"确定性执行计划"（Build Plan），
过程中调用 Asset Resolver 把每个角色/环境/道具解析为真实资产引用。产出一次并冻结；
之后由 build_executor 逐帧重放，不再需要 LLM。

设计要点：
- 纯 JSON -> JSON，不触碰 bpy，可离线单测、可缓存、可 diff。
- 完全确定：相同 Story IR + 相同资产库 => 相同 Build Plan（含 planHash）。
- 这是"基线确定性编译器"。未来的 AI Scene Director（截图自检循环）产出的也是同构的
  Build Plan，冻结后走同一条重放路径，因此两者可互换、可对比。
"""

from __future__ import annotations

import hashlib
import json
from typing import Any

# 兼容包模式（交互式 Bridge）与 headless 顶层模块模式（后台 runner）。
try:
    from .asset_resolver import AssetResolver
except ImportError:  # pragma: no cover - headless top-level import
    from asset_resolver import AssetResolver  # type: ignore[no-redef]


def _safe_number(value: Any, fallback: float, minimum: float, maximum: float) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return fallback
    return max(minimum, min(maximum, parsed))


def _slug(value: Any, fallback: str) -> str:
    text = str(value or fallback).strip()
    cleaned = "".join(ch for ch in text if ch not in '<>:"/\\|?*' and ch.isprintable())
    return (cleaned or fallback)[:80]


def _stable_hash(payload: Any) -> str:
    """对归一化 JSON 做稳定哈希。排序键 + 无多余空白，保证可复现。"""
    encoded = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def _entity_index(story_ir: dict[str, Any]) -> dict[str, dict[str, dict[str, Any]]]:
    """收集 characters/locations/props，按 id 建索引，供镜头引用查 hint。"""
    entities = story_ir.get("entities") if isinstance(story_ir.get("entities"), dict) else {}
    index: dict[str, dict[str, dict[str, Any]]] = {"characters": {}, "locations": {}, "props": {}}
    for group in index:
        raw = entities.get(group)
        if not isinstance(raw, list):
            continue
        for item in raw:
            if isinstance(item, dict) and item.get("id"):
                index[group][str(item["id"])] = item
    return index


def _hints_for_character(entity: dict[str, Any] | None) -> list[str]:
    if not entity:
        return []
    hints: list[Any] = []
    hints.extend(entity.get("assetHints", []) if isinstance(entity.get("assetHints"), list) else [])
    for key in ("name", "appearance", "wardrobe", "description"):
        if entity.get(key):
            hints.append(entity[key])
    return [str(h) for h in hints if h]


def _hints_for_location(entity: dict[str, Any] | None) -> list[str]:
    if not entity:
        return []
    hints: list[Any] = []
    hints.extend(entity.get("assetHints", []) if isinstance(entity.get("assetHints"), list) else [])
    for key in ("name", "era", "description"):
        if entity.get(key):
            hints.append(entity[key])
    return [str(h) for h in hints if h]


# 与 scene_builder 一致的默认舞台布局，保证降级观感稳定、机位可用。
def _default_camera(scene_index: int) -> dict[str, Any]:
    return {
        "id": f"GUAXI_CAMERA_{scene_index + 1:03d}",
        "location": [0.0, -13.5, 4.2],
        "lookAt": [0.0, 0.3, 1.5],
        "lens": 48,
        "shotType": "wide",
        "movement": "static",
        "active": True,
    }


def _default_key_light() -> dict[str, Any]:
    return {
        "id": "GUAXI_LIGHT_KEY",
        "type": "AREA",
        "location": [2.5, -4.0, 7.0],
        "lookAt": [0.0, 0.0, 1.0],
        "energy": 1100,
        "color": [1.0, 0.96, 0.9],
        "size": 5.0,
    }


def compile_build_plan(
    story_ir: dict[str, Any],
    resolver: AssetResolver,
) -> dict[str, Any]:
    """编译并返回 Build Plan（含 planHash / storyIrHash / assetGaps）。"""
    scenes_in = story_ir.get("scenes") if isinstance(story_ir.get("scenes"), list) else []
    if not scenes_in:
        raise ValueError("Story IR 没有可编译的 scenes")

    metadata_in = story_ir.get("metadata") if isinstance(story_ir.get("metadata"), dict) else {}
    fps = int(_safe_number(metadata_in.get("fps"), 24, 12, 60))
    entity_idx = _entity_index(story_ir)

    assets: dict[str, dict[str, Any]] = {}  # assetRef.id -> assetRef，去重
    characters: dict[str, dict[str, Any]] = {}
    scenes_out: list[dict[str, Any]] = []

    def register_asset(ref: dict[str, Any]) -> str:
        assets.setdefault(ref["id"], ref)
        return ref["id"]

    for scene_index, raw_scene in enumerate(scenes_in[:500]):
        raw_scene = raw_scene if isinstance(raw_scene, dict) else {}
        scene_id = _slug(raw_scene.get("id"), f"scene_{scene_index + 1:03d}")
        title = _slug(raw_scene.get("title"), f"场景 {scene_index + 1}")
        duration = _safe_number(raw_scene.get("durationSeconds"), 5.0, 1.0, 120.0)

        # -- 环境资产 --
        location_id = str(raw_scene.get("locationId") or "").strip()
        location_entity = entity_idx["locations"].get(location_id)
        env_ref = resolver.resolve(
            asset_id=f"env_{scene_id}",
            kind="environment",
            hints=_hints_for_location(location_entity),
            description=str((location_entity or {}).get("description") or raw_scene.get("summary") or ""),
            color_index=scene_index,
        )
        register_asset(env_ref)
        environment = {
            "assetId": env_ref["id"],
            "worldColor": [0.015, 0.022, 0.04],
        }

        # -- 场景内的角色 --
        shots_in = raw_scene.get("shots") if isinstance(raw_scene.get("shots"), list) else []
        scene_actor_ids: list[str] = []
        for shot in shots_in:
            if not isinstance(shot, dict):
                continue
            for actor_id in shot.get("characters", []):
                text = str(actor_id).strip()
                if text and text not in scene_actor_ids:
                    scene_actor_ids.append(text)
        if not scene_actor_ids:
            scene_actor_ids = ["narrator-placeholder"]

        objects: list[dict[str, Any]] = []
        for actor_index, actor_id in enumerate(scene_actor_ids[:8]):
            entity = entity_idx["characters"].get(actor_id)
            char_slug = _slug(actor_id, str(actor_index))
            asset_ref = resolver.resolve(
                asset_id=f"char_{char_slug}",
                kind="character",
                hints=_hints_for_character(entity),
                description=str((entity or {}).get("description") or ""),
                color_index=actor_index,
            )
            register_asset(asset_ref)
            if actor_id not in characters:
                characters[actor_id] = {
                    "id": actor_id,
                    "name": str((entity or {}).get("name") or actor_id),
                    "assetId": asset_ref["id"],
                    "displayColor": asset_ref.get("displayColor", [0.7, 0.7, 0.75, 1.0]),
                    "continuity": {
                        "wardrobe": str((entity or {}).get("wardrobe") or ""),
                        "state": "default",
                    },
                }
            x = (actor_index - 1) * 2.0
            objects.append(
                {
                    "id": f"GUAXI_ACTOR_{char_slug}_{scene_index + 1:03d}",
                    "assetId": asset_ref["id"],
                    "characterId": actor_id,
                    "location": [x, 0.2, 0.0],
                    "rotationEuler": [0.0, 0.0, 0.0],
                    "scale": [1.0, 1.0, 1.0],
                    # 占位走位：让预演能观察运动方向；命中真实资产时同样适用。
                    "keyframes": [
                        {"frameOffset": 0, "location": [x - 0.35, 0.2, 0.0]},
                        {"frameOffset": max(1, round(duration * fps)) - 1, "location": [x + 0.35, 0.2, 0.0]},
                    ],
                }
            )

        # -- 镜头标记 --
        markers = [{"name": f"{scene_id} · {title}", "frameOffset": 0}]
        shot_frame_offset = 0
        for shot_index, shot in enumerate(shots_in):
            if not isinstance(shot, dict):
                continue
            shot_id = _slug(shot.get("id"), f"shot_{shot_index + 1:03d}")
            markers.append({"name": shot_id, "frameOffset": shot_frame_offset})
            shot_seconds = _safe_number(shot.get("durationSeconds"), 2.0, 0.25, 60.0)
            shot_frame_offset += max(1, round(shot_seconds * fps))

        # -- 相机取自首个镜头语言（若有），否则默认机位 --
        camera = _default_camera(scene_index)
        first_shot = next((s for s in shots_in if isinstance(s, dict)), None)
        if first_shot is not None:
            camera["shotType"] = str(first_shot.get("shotType") or camera["shotType"])
            camera["movement"] = str(first_shot.get("cameraMovement") or camera["movement"])

        scenes_out.append(
            {
                "id": scene_id,
                "title": title,
                "durationSeconds": duration,
                "environment": environment,
                "objects": objects,
                "cameras": [camera],
                "lights": [_default_key_light()],
                "markers": markers,
            }
        )

    plan: dict[str, Any] = {
        "schemaVersion": "1.0",
        "metadata": {
            "title": str(metadata_in.get("title") or "未命名影像项目"),
            "fps": fps,
            "visualStyle": str(metadata_in.get("visualStyle") or ""),
            "generator": "deterministic-compiler",
            "targetBlenderVersion": "5.2 LTS",
            "storyIrHash": _stable_hash(story_ir),
        },
        "render": {
            "worldColor": [0.015, 0.022, 0.04],
        },
        "assets": list(assets.values()),
        "characters": list(characters.values()),
        "scenes": scenes_out,
    }
    if resolver.gaps:
        plan["metadata"]["assetGaps"] = resolver.gaps

    # planHash 最后计算，且排除自身，保证同输入同哈希。
    plan["metadata"]["planHash"] = _stable_hash(
        {k: v for k, v in plan.items() if k != "metadata"} | {"_meta": {"fps": fps}}
    )
    return plan
