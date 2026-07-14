"""合规与内容凭证生成（§22）。

从冻结的 Build Plan 的 provenance 汇总，写出发布所需的合规工件：
- compliance/license_ledger.json   资产许可账本（来源、作者、许可证、用途）
- compliance/model_usage.json      模型使用记录（模型、seed、prompt 哈希、输入哈希）
- compliance/aigc_label.json       AIGC 合规标识与说明（中国平台/法规要求显著标识）

设计原则：
- 只汇总已存在的 provenance，不臆造许可信息；缺失项显式标记为 "unknown"，供人工补全。
- 纯 Python，无 bpy 依赖，可离线运行，可在渲染前后独立调用。
- 不写入任何密钥；只记录可公开的来源与许可事实。
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


# 各解析级别对应的“是否属于 AI 生成/合成内容”标注，用于 AIGC 判定。
_AI_GENERATED_SOURCES = {"generated"}
_EXTERNAL_SOURCES = {"polyhaven", "sketchfab", "generated"}


def _asset_rows(build_plan: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for asset in build_plan.get("assets", []) or []:
        if not isinstance(asset, dict):
            continue
        provenance = asset.get("provenance") if isinstance(asset.get("provenance"), dict) else {}
        rows.append(
            {
                "assetId": str(asset.get("id") or ""),
                "kind": str(asset.get("kind") or ""),
                "source": str(asset.get("source") or ""),
                "author": str(provenance.get("author") or "unknown"),
                "license": str(provenance.get("license") or "unknown"),
                "url": str(provenance.get("url") or ""),
                "retrievedAt": str(provenance.get("retrievedAt") or ""),
                "model": str(provenance.get("model") or ""),
                "seed": provenance.get("seed"),
            }
        )
    return rows


def build_license_ledger(build_plan: dict[str, Any]) -> dict[str, Any]:
    rows = _asset_rows(build_plan)
    external = [r for r in rows if r["source"] in _EXTERNAL_SOURCES]
    missing = [r["assetId"] for r in external if r["license"] in ("", "unknown")]
    return {
        "schemaVersion": "1.0",
        "title": str(build_plan.get("metadata", {}).get("title") or ""),
        "planHash": str(build_plan.get("metadata", {}).get("planHash") or ""),
        "assets": rows,
        "externalAssetCount": len(external),
        "missingLicenseAssetIds": missing,
        # 发布闸门：外部/生成资产存在未知许可时，不应进入正式发布。
        "readyForPublish": len(missing) == 0,
    }


def build_model_usage(build_plan: dict[str, Any], compile_context: dict[str, Any] | None = None) -> dict[str, Any]:
    metadata = build_plan.get("metadata", {}) if isinstance(build_plan.get("metadata"), dict) else {}
    generative = [
        {
            "assetId": str(a.get("id") or ""),
            "model": str((a.get("provenance") or {}).get("model") or ""),
            "seed": (a.get("provenance") or {}).get("seed"),
        }
        for a in build_plan.get("assets", []) or []
        if isinstance(a, dict) and str(a.get("source") or "") in _AI_GENERATED_SOURCES
    ]
    record = {
        "schemaVersion": "1.0",
        "storyIrHash": str(metadata.get("storyIrHash") or ""),
        "planHash": str(metadata.get("planHash") or ""),
        "generator": str(metadata.get("generator") or ""),
        "generativeAssets": generative,
    }
    if compile_context:
        # 允许上层传入 Story IR 编译所用的模型 id、prompt 哈希等（不含密钥）。
        record["storyCompilation"] = {
            "modelId": compile_context.get("modelId"),
            "promptHash": compile_context.get("promptHash"),
        }
    return record


def build_aigc_label(build_plan: dict[str, Any]) -> dict[str, Any]:
    """生成 AIGC 合规标识。

    判定：本工具的画面由 AI 编排/合成产生（3D 编排 + 可选生成式媒体），因此默认标注为
    “包含人工智能生成合成内容”。这满足中国主流平台与深度合成/生成式 AI 相关规定对显著
    标识的要求；具体展示文案仍需按目标平台规范在发布环节落地。
    """
    metadata = build_plan.get("metadata", {}) if isinstance(build_plan.get("metadata"), dict) else {}
    has_generative = any(
        isinstance(a, dict) and str(a.get("source") or "") in _AI_GENERATED_SOURCES
        for a in build_plan.get("assets", []) or []
    )
    return {
        "schemaVersion": "1.0",
        "isAIGC": True,
        "containsGenerativeMedia": has_generative,
        "label": "本内容包含由人工智能生成或合成的画面",
        "labelShort": "AI 生成",
        "method": "3D 编排 + 生成式媒体" if has_generative else "AI 辅助 3D 编排",
        "planHash": str(metadata.get("planHash") or ""),
        "notes": [
            "上传视频平台时，请在标题/简介或平台提供的“AI 生成”声明入口进行显著标注。",
            "如使用真实人物肖像或声音，需另行取得授权并在 compliance/consent 留存。",
        ],
    }


def write_compliance_artifacts(
    build_plan: dict[str, Any],
    project_dir: str | Path,
    compile_context: dict[str, Any] | None = None,
) -> dict[str, str]:
    """写出全部合规工件，返回各文件路径。渲染前后均可调用；幂等覆盖。"""
    compliance_dir = Path(project_dir).expanduser().resolve() / "compliance"
    compliance_dir.mkdir(parents=True, exist_ok=True)

    outputs: dict[str, dict[str, Any]] = {
        "license_ledger.json": build_license_ledger(build_plan),
        "model_usage.json": build_model_usage(build_plan, compile_context),
        "aigc_label.json": build_aigc_label(build_plan),
    }
    paths: dict[str, str] = {}
    for name, payload in outputs.items():
        target = compliance_dir / name
        with target.open("w", encoding="utf-8") as stream:
            json.dump(payload, stream, ensure_ascii=False, indent=2)
        paths[name] = str(target)
    return paths
