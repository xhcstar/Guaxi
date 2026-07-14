"""Asset Resolver（资产解析层，§7.1）。

把 Story IR 的资产需求（entities / assetPlan / assetHints / searchTags）解析为可导入
Blender 的真实资产句柄，逐级降级：

    L1 本地资产库  ->  L2 在线库(PolyHaven/Sketchfab)  ->  L3 文/图生3D
    ->  L4 程序化  ->  L5 灰盒兜底

本模块刻意与 bpy 解耦：它只做"查找与决策"，产出纯 JSON 资产引用（assetRef），
由 build_executor 负责真正导入 Blender。这样解析层可离线、可单测、可缓存、可复现。

当前实现完整落地 L1（本地库，确定性、离线）与 L5（灰盒兜底）。L2/L3/L4 暴露清晰的
扩展点：默认关闭（needs online / needs gpu），命中时返回 None 让流程继续降级，绝不
静默宣称完成未实现的能力。
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Iterable


# 本地资产库索引文件名。放在资产库根目录，声明每个资产的标签、类型、导入器与许可。
LOCAL_INDEX_NAME = "guaxi_asset_index.json"

# 支持的网格容器与对应导入器。
_IMPORTER_BY_SUFFIX = {
    ".glb": "gltf",
    ".gltf": "gltf",
    ".usd": "usd",
    ".usda": "usd",
    ".usdc": "usd",
    ".usdz": "usd",
    ".fbx": "fbx",
    ".blend": "blend",
    ".obj": "obj",
}

# 灰盒占位调色板，与旧 scene_builder 保持一致，保证降级观感稳定。
_GREYBOX_PALETTE = [
    [0.24, 0.55, 0.95, 1.0],
    [0.94, 0.42, 0.34, 1.0],
    [0.34, 0.78, 0.52, 1.0],
    [0.76, 0.48, 0.92, 1.0],
]


def _normalize_tag(value: Any) -> str:
    text = str(value or "").strip().lower()
    # 统一分隔符，让 "汉代 营帐" / "han-tent" / "han_tent" 可比较。
    return re.sub(r"[\s_\-]+", " ", text)


def _tag_set(values: Iterable[Any]) -> set[str]:
    result: set[str] = set()
    for value in values:
        normalized = _normalize_tag(value)
        if normalized:
            result.add(normalized)
            # 额外拆词，提升中英混合命中率。
            for token in normalized.split(" "):
                if len(token) >= 2:
                    result.add(token)
    return result


class AssetLibrary:
    """L1 本地资产库。读取索引一次，按标签做确定性匹配。

    索引结构（guaxi_asset_index.json）：
        {
          "assets": [
            {
              "id": "env_han_tent_01",
              "kind": "environment",
              "path": "environments/han_tent_01.glb",
              "tags": ["汉代", "营帐", "tent", "military"],
              "unitScale": 1.0,
              "license": "CC-BY-4.0",
              "author": "...",
              "url": "..."
            }
          ]
        }

    没有索引文件时，库为空，解析直接进入下一级（不报错）。
    """

    def __init__(self, root: str | Path | None) -> None:
        self.root: Path | None = None
        self._entries: list[dict[str, Any]] = []
        if not root:
            return
        candidate = Path(str(root)).expanduser()
        if not candidate.exists():
            return
        self.root = candidate.resolve()
        self._load_index()

    def _load_index(self) -> None:
        assert self.root is not None
        index_path = self.root / LOCAL_INDEX_NAME
        if not index_path.is_file():
            return
        try:
            with index_path.open("r", encoding="utf-8-sig") as stream:
                data = json.load(stream)
        except (OSError, ValueError):
            return
        raw_assets = data.get("assets") if isinstance(data, dict) else None
        if not isinstance(raw_assets, list):
            return
        for raw in raw_assets:
            if not isinstance(raw, dict):
                continue
            path_value = str(raw.get("path", "")).strip()
            if not path_value:
                continue
            entry_path = (self.root / path_value).resolve()
            # 索引条目必须落在库内且文件存在，避免路径穿越与悬空引用。
            try:
                entry_path.relative_to(self.root)
            except ValueError:
                continue
            if not entry_path.is_file():
                continue
            self._entries.append(
                {
                    "id": str(raw.get("id") or entry_path.stem),
                    "kind": str(raw.get("kind") or "prop"),
                    "path": str(entry_path),
                    "tags": _tag_set(raw.get("tags", []) if isinstance(raw.get("tags"), list) else []),
                    "unitScale": float(raw.get("unitScale") or 1.0),
                    "license": str(raw.get("license") or ""),
                    "author": str(raw.get("author") or ""),
                    "url": str(raw.get("url") or ""),
                }
            )

    @property
    def size(self) -> int:
        return len(self._entries)

    def best_match(self, kind: str, query_tags: set[str]) -> dict[str, Any] | None:
        """按 (类型匹配, 标签重合度) 打分选最优；打分并列时用 id 排序保证确定性。"""
        if not self._entries or not query_tags:
            return None
        scored: list[tuple[int, int, str, dict[str, Any]]] = []
        for entry in self._entries:
            overlap = len(entry["tags"] & query_tags)
            if overlap == 0:
                continue
            kind_bonus = 1 if entry["kind"] == kind else 0
            # 负号让 sorted 升序时把高分排前；id 作为最终确定性 tiebreaker。
            scored.append((-(overlap + kind_bonus * 2), -overlap, entry["id"], entry))
        if not scored:
            return None
        scored.sort()
        return scored[0][3]


def _importer_for(path: str) -> str:
    return _IMPORTER_BY_SUFFIX.get(Path(path).suffix.lower(), "none")


def _collect_query_tags(*sources: Any) -> set[str]:
    tags: list[Any] = []
    for source in sources:
        if source is None:
            continue
        if isinstance(source, (list, tuple, set)):
            tags.extend(source)
        else:
            tags.append(source)
    return _tag_set(tags)


def _greybox_ref(asset_id: str, kind: str, color_index: int) -> dict[str, Any]:
    color = _GREYBOX_PALETTE[color_index % len(_GREYBOX_PALETTE)]
    return {
        "id": asset_id,
        "source": "greybox",
        "kind": kind,
        "path": "",
        "importer": "none",
        "unitScale": 1.0,
        "displayColor": color,
        "provenance": {"license": "n/a"},
    }


class AssetResolver:
    """逐级降级解析。目前 L1 + L5 完整；L2/L3/L4 为可插拔扩展点，默认返回 None。"""

    def __init__(
        self,
        library_root: str | Path | None = None,
        *,
        allow_online: bool = False,
        allow_generative: bool = False,
    ) -> None:
        self.library = AssetLibrary(library_root)
        self.allow_online = allow_online
        self.allow_generative = allow_generative
        # 记录降级情况，供上层写入 assetGaps 与日志。
        self.gaps: list[dict[str, str]] = []

    # -- 可插拔扩展点（默认未启用）--------------------------------------

    def _resolve_online(self, asset_id: str, kind: str, query_tags: set[str]) -> dict[str, Any] | None:
        """L2 PolyHaven/Sketchfab。需要网络与下载缓存；默认关闭。

        实现时应：搜索 -> 下载到工作区 assets/ -> 返回带 license/author/url 的 assetRef。
        """
        return None

    def _resolve_generative(self, asset_id: str, kind: str, description: str) -> dict[str, Any] | None:
        """L3 文/图生3D（Hyper3D/Hunyuan3D/Tripo）。需要 GPU 或云 API；默认关闭。"""
        return None

    def _resolve_procedural(self, asset_id: str, kind: str, query_tags: set[str]) -> dict[str, Any] | None:
        """L4 Geometry Nodes 模板。地形/建筑/路线等规则化几何；默认未接入。"""
        return None

    # -- 主入口 --------------------------------------------------------

    def resolve(
        self,
        asset_id: str,
        kind: str,
        *,
        hints: Any = None,
        description: str = "",
        color_index: int = 0,
    ) -> dict[str, Any]:
        """解析单个资产需求，必定返回一个 assetRef（最差是 greybox），绝不抛出。"""
        query_tags = _collect_query_tags(hints, kind, asset_id, description)

        # L1 本地库
        match = self.library.best_match(kind, query_tags)
        if match is not None:
            return {
                "id": asset_id,
                "source": "local",
                "kind": kind,
                "path": match["path"],
                "importer": _importer_for(match["path"]),
                "unitScale": match["unitScale"],
                "provenance": {
                    "author": match["author"],
                    "license": match["license"],
                    "url": match["url"],
                    "matchedAssetId": match["id"],
                },
            }

        # L2 在线库
        if self.allow_online:
            online = self._resolve_online(asset_id, kind, query_tags)
            if online is not None:
                return online

        # L3 生成式
        if self.allow_generative and description:
            generated = self._resolve_generative(asset_id, kind, description)
            if generated is not None:
                return generated

        # L4 程序化
        procedural = self._resolve_procedural(asset_id, kind, query_tags)
        if procedural is not None:
            return procedural

        # L5 灰盒兜底
        self.gaps.append({"assetId": asset_id, "kind": kind, "reason": "未在可用资产源命中"})
        return _greybox_ref(asset_id, kind, color_index)


def load_resolver_from_env(project_dir: str | Path) -> AssetResolver:
    """便捷构造：优先读环境变量 GUAXI_ASSET_LIBRARY，否则用项目内 assets/ 作库根。

    放在 asset_resolver 中而非 runner，方便交互式 Bridge 与后台 runner 共用一致规则。
    """
    import os

    configured = os.environ.get("GUAXI_ASSET_LIBRARY", "").strip()
    root = configured or str(Path(project_dir).expanduser().resolve() / "assets")
    allow_online = os.environ.get("GUAXI_ALLOW_ONLINE_ASSETS", "").strip() == "1"
    allow_generative = os.environ.get("GUAXI_ALLOW_GENERATIVE_ASSETS", "").strip() == "1"
    return AssetResolver(root, allow_online=allow_online, allow_generative=allow_generative)
