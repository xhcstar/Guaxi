"""Blender 内置的可编辑程序化资产生成器。

目标不是假装能在离线状态下精确生成任意物体，而是保证每个未命中外部资产的需求都能
得到一个独立、可打开、可继续编辑的 ``.blend`` 文件。生成结果由多个有语义名称的网格
部件组成；用户可以直接在 Blender 中雕刻/替换，也可以通过 Python 修改这些对象。

安全边界：本模块只执行仓库内固定的几何配方，不执行 Story IR 中携带的 Python、表达式
或脚本。Story IR 文本只用于选择允许列表内的配方和写入资产元数据。
"""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

import bpy
from mathutils import Vector


def _rgba(asset: dict[str, Any]) -> tuple[float, float, float, float]:
    value = asset.get("displayColor")
    if isinstance(value, list) and len(value) == 4:
        return tuple(max(0.0, min(1.0, float(item))) for item in value)  # type: ignore[return-value]
    return (0.42, 0.58, 0.78, 1.0)


def _material(name: str, color: tuple[float, float, float, float], metallic: float = 0.0) -> bpy.types.Material:
    material = bpy.data.materials.new(name=name)
    material.diffuse_color = color
    material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF") if material.node_tree else None
    if principled is not None:
        principled.inputs["Base Color"].default_value = color
        principled.inputs["Roughness"].default_value = 0.58
        principled.inputs["Metallic"].default_value = metallic
    return material


def _parent(obj: bpy.types.Object, root: bpy.types.Object, parts: list[bpy.types.Object]) -> bpy.types.Object:
    obj.parent = root
    parts.append(obj)
    return obj


def _add_cube(
    name: str,
    root: bpy.types.Object,
    parts: list[bpy.types.Object],
    material: bpy.types.Material,
    location: tuple[float, float, float],
    scale: tuple[float, float, float],
    *,
    bevel: float = 0.06,
) -> bpy.types.Object:
    bpy.ops.mesh.primitive_cube_add(location=location, scale=scale)
    obj = _parent(bpy.context.object, root, parts)
    obj.name = name
    obj.data.materials.append(material)
    if bevel > 0:
        modifier = obj.modifiers.new(name="Guaxi 可编辑倒角", type="BEVEL")
        modifier.width = bevel
        modifier.segments = 2
    return obj


def _add_sphere(
    name: str,
    root: bpy.types.Object,
    parts: list[bpy.types.Object],
    material: bpy.types.Material,
    location: tuple[float, float, float],
    scale: tuple[float, float, float],
) -> bpy.types.Object:
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=16, location=location)
    obj = _parent(bpy.context.object, root, parts)
    obj.name = name
    obj.scale = scale
    obj.data.materials.append(material)
    for polygon in obj.data.polygons:
        polygon.use_smooth = True
    return obj


def _add_cylinder_between(
    name: str,
    root: bpy.types.Object,
    parts: list[bpy.types.Object],
    material: bpy.types.Material,
    start: tuple[float, float, float],
    end: tuple[float, float, float],
    radius: float,
) -> bpy.types.Object:
    start_vector = Vector(start)
    end_vector = Vector(end)
    direction = end_vector - start_vector
    midpoint = (start_vector + end_vector) * 0.5
    bpy.ops.mesh.primitive_cylinder_add(vertices=20, radius=radius, depth=max(direction.length, 0.001), location=midpoint)
    obj = _parent(bpy.context.object, root, parts)
    obj.name = name
    obj.rotation_euler = direction.to_track_quat("Z", "Y").to_euler()
    obj.data.materials.append(material)
    for polygon in obj.data.polygons:
        polygon.use_smooth = True
    return obj


def _build_character(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    """生成分件式基础人物；各部件可直接替换，优于不可编辑的单一占位立方体。"""
    primary = _material(f"MAT_{root.name}_服装", _rgba(asset))
    skin = _material(f"MAT_{root.name}_皮肤", (0.72, 0.49, 0.36, 1.0))
    dark = _material(f"MAT_{root.name}_头发", (0.035, 0.025, 0.02, 1.0))
    _add_cube("躯干_Torso", root, parts, primary, (0.0, 0.0, 1.55), (0.44, 0.26, 0.62), bevel=0.16)
    _add_sphere("头部_Head", root, parts, skin, (0.0, 0.0, 2.48), (0.34, 0.31, 0.42))
    _add_sphere("头发_Hair", root, parts, dark, (0.0, 0.015, 2.69), (0.35, 0.32, 0.24))
    _add_cylinder_between("左臂_Arm.L", root, parts, primary, (-0.42, 0.0, 1.95), (-0.7, 0.0, 1.15), 0.13)
    _add_cylinder_between("右臂_Arm.R", root, parts, primary, (0.42, 0.0, 1.95), (0.7, 0.0, 1.15), 0.13)
    _add_cylinder_between("左腿_Leg.L", root, parts, primary, (-0.2, 0.0, 1.05), (-0.22, 0.0, 0.12), 0.17)
    _add_cylinder_between("右腿_Leg.R", root, parts, primary, (0.2, 0.0, 1.05), (0.22, 0.0, 0.12), 0.17)


def _build_tree(
    prefix: str,
    root: bpy.types.Object,
    parts: list[bpy.types.Object],
    location: tuple[float, float, float],
    scale: float = 1.0,
) -> None:
    trunk = _material(f"MAT_{prefix}_树干", (0.18, 0.075, 0.025, 1.0))
    leaves = _material(f"MAT_{prefix}_树冠", (0.055, 0.26, 0.08, 1.0))
    x, y, z = location
    _add_cylinder_between(f"{prefix}_Trunk", root, parts, trunk, (x, y, z), (x, y, z + 2.2 * scale), 0.18 * scale)
    _add_sphere(f"{prefix}_Crown", root, parts, leaves, (x, y, z + 2.65 * scale), (0.95 * scale, 0.95 * scale, 1.25 * scale))


def _build_environment(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    generation = asset.get("generation") if isinstance(asset.get("generation"), dict) else {}
    text = (str(generation.get("prompt") or "") + " " + " ".join(generation.get("tags") or [])).lower()
    if any(word in text for word in ("forest", "woods", "tree", "森林", "树林", "树木")):
        ground = _material(f"MAT_{root.name}_地面", (0.08, 0.16, 0.055, 1.0))
        _add_cube("森林地面_Ground", root, parts, ground, (0.0, 1.5, -0.12), (6.0, 5.0, 0.12), bevel=0)
        positions = [(-3.5, 2.5, 0.0), (-1.4, 4.0, 0.0), (1.2, 3.5, 0.0), (3.7, 2.8, 0.0), (-4.2, 5.5, 0.0), (3.0, 5.8, 0.0)]
        for index, position in enumerate(positions):
            _build_tree(f"Tree_{index + 1:02d}", root, parts, position, 0.8 + (index % 3) * 0.14)
        return

    wall = _material(f"MAT_{root.name}_墙体", _rgba(asset))
    floor = _material(f"MAT_{root.name}_地面", (0.16, 0.13, 0.1, 1.0))
    trim = _material(f"MAT_{root.name}_梁柱", (0.16, 0.055, 0.02, 1.0))
    _add_cube("场景地面_Floor", root, parts, floor, (0.0, 2.0, -0.1), (6.0, 5.0, 0.1), bevel=0)
    _add_cube("后墙_BackWall", root, parts, wall, (0.0, 6.6, 2.6), (6.0, 0.16, 2.7), bevel=0.02)
    _add_cube("左墙_LeftWall", root, parts, wall, (-5.85, 2.0, 2.6), (0.16, 4.5, 2.7), bevel=0.02)
    _add_cube("右墙_RightWall", root, parts, wall, (5.85, 2.0, 2.6), (0.16, 4.5, 2.7), bevel=0.02)
    for index, x in enumerate((-4.7, -1.6, 1.6, 4.7)):
        _add_cube(f"立柱_Pillar_{index + 1:02d}", root, parts, trim, (x, 6.15, 2.5), (0.16, 0.2, 2.5), bevel=0.03)


def _build_table(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    wood = _material(f"MAT_{root.name}_木材", _rgba(asset))
    _add_cube("桌面_TableTop", root, parts, wood, (0.0, 0.0, 1.05), (1.2, 0.68, 0.12), bevel=0.07)
    for label, x, y in (("FL", -0.95, -0.48), ("FR", 0.95, -0.48), ("BL", -0.95, 0.48), ("BR", 0.95, 0.48)):
        _add_cube(f"桌腿_Leg_{label}", root, parts, wood, (x, y, 0.5), (0.1, 0.1, 0.5), bevel=0.03)


def _build_chair(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    wood = _material(f"MAT_{root.name}_椅子", _rgba(asset))
    _add_cube("椅面_Seat", root, parts, wood, (0.0, 0.0, 0.78), (0.55, 0.55, 0.1), bevel=0.06)
    _add_cube("椅背_Back", root, parts, wood, (0.0, 0.48, 1.35), (0.55, 0.08, 0.65), bevel=0.05)
    for label, x, y in (("FL", -0.42, -0.42), ("FR", 0.42, -0.42), ("BL", -0.42, 0.42), ("BR", 0.42, 0.42)):
        _add_cube(f"椅腿_Leg_{label}", root, parts, wood, (x, y, 0.37), (0.07, 0.07, 0.37), bevel=0.02)


def _build_blade(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    metal = _material(f"MAT_{root.name}_金属", (0.42, 0.48, 0.56, 1.0), metallic=0.85)
    grip = _material(f"MAT_{root.name}_握柄", _rgba(asset))
    _add_cube("刀身_Blade", root, parts, metal, (0.0, 0.0, 1.25), (0.08, 0.025, 1.05), bevel=0.015)
    _add_cube("护手_Guard", root, parts, metal, (0.0, 0.0, 0.28), (0.32, 0.06, 0.05), bevel=0.02)
    _add_cylinder_between("握柄_Grip", root, parts, grip, (0.0, 0.0, 0.25), (0.0, 0.0, -0.35), 0.07)


def _build_vehicle(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    body = _material(f"MAT_{root.name}_车身", _rgba(asset), metallic=0.25)
    tire = _material(f"MAT_{root.name}_轮胎", (0.025, 0.025, 0.025, 1.0))
    _add_cube("车身_Body", root, parts, body, (0.0, 0.0, 0.72), (1.65, 0.78, 0.38), bevel=0.18)
    _add_cube("车舱_Cabin", root, parts, body, (0.2, 0.0, 1.2), (0.78, 0.68, 0.42), bevel=0.15)
    for label, x, y in (("FL", -1.05, -0.78), ("FR", 1.05, -0.78), ("BL", -1.05, 0.78), ("BR", 1.05, 0.78)):
        bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.38, depth=0.2, location=(x, y, 0.48), rotation=(math.pi / 2, 0.0, 0.0))
        wheel = _parent(bpy.context.object, root, parts)
        wheel.name = f"车轮_Wheel_{label}"
        wheel.data.materials.append(tire)


def _build_generic_prop(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    """未知物体的可编辑代理：多部件轮廓，而非单一立方体。"""
    primary = _material(f"MAT_{root.name}_主体", _rgba(asset))
    accent = _material(f"MAT_{root.name}_细节", (0.16, 0.2, 0.28, 1.0), metallic=0.2)
    _add_sphere("主体_MainForm", root, parts, primary, (0.0, 0.0, 0.85), (0.65, 0.5, 0.85))
    _add_cylinder_between("支撑_Support", root, parts, accent, (0.0, 0.0, 0.0), (0.0, 0.0, 0.58), 0.2)
    _add_cube("可替换细节_Detail", root, parts, accent, (0.0, -0.42, 0.9), (0.26, 0.18, 0.22), bevel=0.08)


def _build_prop(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    generation = asset.get("generation") if isinstance(asset.get("generation"), dict) else {}
    text = (str(generation.get("prompt") or "") + " " + " ".join(generation.get("tags") or [])).lower()
    if any(word in text for word in ("table", "desk", "桌", "案几")):
        _build_table(root, parts, asset)
    elif any(word in text for word in ("chair", "stool", "椅", "凳")):
        _build_chair(root, parts, asset)
    elif any(word in text for word in ("tree", "plant", "树", "植物")):
        _build_tree("Tree", root, parts, (0.0, 0.0, 0.0), 0.75)
    elif any(word in text for word in ("sword", "knife", "blade", "剑", "刀")):
        _build_blade(root, parts, asset)
    elif any(word in text for word in ("car", "vehicle", "wagon", "汽车", "车辆", "马车")):
        _build_vehicle(root, parts, asset)
    elif any(word in text for word in ("cup", "bottle", "vase", "杯", "瓶", "壶")):
        ceramic = _material(f"MAT_{root.name}_器皿", _rgba(asset), metallic=0.05)
        bpy.ops.mesh.primitive_cylinder_add(vertices=32, radius=0.36, depth=0.82, location=(0.0, 0.0, 0.41))
        vessel = _parent(bpy.context.object, root, parts)
        vessel.name = "器皿_Vessel"
        vessel.data.materials.append(ceramic)
        bevel = vessel.modifiers.new(name="Guaxi 器皿倒角", type="BEVEL")
        bevel.width = 0.06
        bevel.segments = 3
    else:
        _build_generic_prop(root, parts, asset)


def _build_map(root: bpy.types.Object, parts: list[bpy.types.Object], asset: dict[str, Any]) -> None:
    base = _material(f"MAT_{root.name}_地图", _rgba(asset))
    marker = _material(f"MAT_{root.name}_标记", (0.8, 0.08, 0.045, 1.0))
    _add_cube("地图底板_MapBase", root, parts, base, (0.0, 0.0, 0.04), (3.0, 2.0, 0.04), bevel=0.05)
    for index, (x, y) in enumerate(((-1.8, -0.6), (-0.3, 0.5), (1.4, -0.2))):
        bpy.ops.mesh.primitive_cone_add(vertices=20, radius1=0.16, radius2=0.0, depth=0.5, location=(x, y, 0.33))
        pin = _parent(bpy.context.object, root, parts)
        pin.name = f"地图标记_Marker_{index + 1:02d}"
        pin.data.materials.append(marker)


def materialize_procedural_asset(asset: dict[str, Any]) -> Path:
    """创建资产并写成独立 .blend；返回可供 Build Plan 引用的文件路径。"""
    path = Path(str(asset.get("path") or "")).expanduser().resolve()
    if path.suffix.lower() != ".blend":
        raise ValueError(f"程序化资产必须输出 .blend：{path}")
    path.parent.mkdir(parents=True, exist_ok=True)

    asset_id = str(asset.get("id") or "asset")
    root = bpy.data.objects.new(f"GUAXI_ASSET_{asset_id}", None)
    bpy.context.collection.objects.link(root)
    root["guaxi_asset_id"] = asset_id
    root["guaxi_asset_kind"] = str(asset.get("kind") or "prop")
    root["guaxi_editable"] = True
    root["guaxi_generation"] = json.dumps(asset.get("generation", {}), ensure_ascii=False)
    parts: list[bpy.types.Object] = []

    kind = str(asset.get("kind") or "prop")
    if kind == "character":
        _build_character(root, parts, asset)
    elif kind == "environment":
        _build_environment(root, parts, asset)
    elif kind == "map":
        _build_map(root, parts, asset)
    else:
        _build_prop(root, parts, asset)

    # 保存“完整可直接打开的主文件”，而不是只写对象数据块库。Blender 5.2 RC 的
    # bpy.data.libraries.write(Scene) 存在原生崩溃路径，save_as_mainfile(copy=True) 经实机
    # 验证更稳定，同时不会改变当前制作工程的目标文件名。
    bpy.context.scene["guaxi_editable_asset"] = asset_id
    bpy.ops.wm.save_as_mainfile(filepath=str(path), check_existing=False, copy=True, compress=True)

    # 每个资产必须彼此隔离；保存后删除本轮对象及其孤立数据，下一轮再生成新的主文件。
    meshes = {obj.data for obj in parts if obj.type == "MESH" and obj.data is not None}
    materials = {slot.material for obj in parts for slot in obj.material_slots if slot.material is not None}
    for obj in [root, *parts]:
        if obj.name in bpy.data.objects:
            bpy.data.objects.remove(obj, do_unlink=True)
    for mesh in meshes:
        if mesh.users == 0:
            bpy.data.meshes.remove(mesh)
    for material in materials:
        if material.users == 0:
            bpy.data.materials.remove(material)
    if "guaxi_editable_asset" in bpy.context.scene:
        del bpy.context.scene["guaxi_editable_asset"]
    sidecar = path.with_suffix(".asset.json")
    with sidecar.open("w", encoding="utf-8") as stream:
        json.dump(
            {
                "schemaVersion": "1.0",
                "assetId": asset_id,
                "kind": kind,
                "modelPath": str(path),
                "generation": asset.get("generation", {}),
                "instructions": "可在 Blender 或 Python 中修改此 .blend；保存后回到 Guaxi 点击“启用已修改模型”。",
            },
            stream,
            ensure_ascii=False,
            indent=2,
        )
    print(
        f"[GuaxiProceduralAsset][INFO] 已生成可编辑模型：{asset_id} -> {path}（{len(parts)} 个部件）",
        flush=True,
    )
    return path
