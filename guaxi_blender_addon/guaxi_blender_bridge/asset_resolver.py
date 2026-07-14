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

import hashlib
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

_EDITABLE_SUFFIX_PRIORITY = (".blend", ".glb", ".gltf", ".usd", ".usda", ".usdc", ".fbx", ".obj")


def asset_file_hash(path: str | Path) -> str:
    """计算资产内容哈希，供 Build Plan 检测模型被人工修改。

    哈希不依赖文件时间戳；用户保存过但内容未变化时不会让冻结计划失效。
    文件由 Blender/Python 修改后，重新执行 assets 构建即可把新哈希写入计划。
    """
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def editable_asset_filename(asset_id: str, suffix: str = ".blend") -> str:
    """把任意（包括中文）资产 ID 转为稳定、安全的文件名。"""
    readable = re.sub(r"[^0-9A-Za-z._-]+", "_", str(asset_id)).strip("._-")
    readable = (readable or "asset")[:72]
    fingerprint = hashlib.sha256(str(asset_id).encode("utf-8")).hexdigest()[:10]
    normalized_suffix = suffix if suffix.startswith(".") else f".{suffix}"
    return f"{readable}-{fingerprint}{normalized_suffix.lower()}"

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
    """逐级降级解析，并管理项目内可编辑资产。

    项目资产有两种稳定入口：
    - ``assets/overrides/``：用户放入的替换模型，优先级最高；
    - ``assets/generated/``：Guaxi 首次构建生成的独立 ``.blend`` 模型。

    两者都会在重新执行 ``assets`` 构建时重新计算内容哈希并冻结到 Build Plan，
    因而既允许 Blender/Python 后期编辑，又不会破坏 ``replay`` 的确定性语义。
    """

    def __init__(
        self,
        library_root: str | Path | None = None,
        *,
        project_dir: str | Path | None = None,
        allow_online: bool = False,
        allow_generative: bool = False,
    ) -> None:
        self.library = AssetLibrary(library_root)
        self.project_dir = Path(project_dir).expanduser().resolve() if project_dir else None
        self.project_assets_root = self.project_dir / "assets" if self.project_dir else None
        self.override_root = self.project_assets_root / "overrides" if self.project_assets_root else None
        self.generated_root = self.project_assets_root / "generated" if self.project_assets_root else None
        self.allow_online = allow_online
        self.allow_generative = allow_generative
        # 记录降级情况，供上层写入 assetGaps 与日志。
        self.gaps: list[dict[str, str]] = []

    def _override_map_path(self) -> Path | None:
        return self.project_assets_root / "asset_overrides.json" if self.project_assets_root else None

    def _configured_override(self, asset_id: str) -> Path | None:
        """读取用户显式覆盖表；路径必须留在项目 assets 内，阻断路径穿越。"""
        map_path = self._override_map_path()
        if map_path is None or not map_path.is_file() or self.project_assets_root is None:
            return None
        try:
            with map_path.open("r", encoding="utf-8-sig") as stream:
                raw = json.load(stream)
        except (OSError, ValueError) as exc:
            print(f"[GuaxiAssetResolver][WARN] 读取资产覆盖表失败：{map_path} ({exc})", flush=True)
            return None
        entries = raw.get("assets") if isinstance(raw, dict) else None
        value = entries.get(asset_id) if isinstance(entries, dict) else None
        if isinstance(value, dict):
            if value.get("enabled", True) is False:
                return None
            value = value.get("path")
        if not isinstance(value, str) or not value.strip():
            return None
        candidate = Path(value.strip()).expanduser()
        if not candidate.is_absolute():
            candidate = self.project_assets_root / candidate
        try:
            resolved = candidate.resolve()
            resolved.relative_to(self.project_assets_root)
        except (OSError, ValueError):
            print(
                f"[GuaxiAssetResolver][WARN] 忽略超出项目 assets 的覆盖路径：{candidate}",
                flush=True,
            )
            return None
        return resolved if resolved.is_file() and _importer_for(str(resolved)) != "none" else None

    def _project_editable_asset(self, asset_id: str, kind: str) -> dict[str, Any] | None:
        """优先发现人工覆盖，其次发现上次生成并可能已编辑的模型文件。"""
        configured = self._configured_override(asset_id)
        candidates: list[tuple[str, Path]] = []
        if configured is not None:
            candidates.append(("override-map", configured))
        if self.override_root is not None:
            base_name = editable_asset_filename(asset_id, ".blend").rsplit(".", 1)[0]
            candidates.extend(("override-file", self.override_root / f"{base_name}{suffix}") for suffix in _EDITABLE_SUFFIX_PRIORITY)
        if self.generated_root is not None:
            candidates.append(("generated-editable", self.generated_root / editable_asset_filename(asset_id)))

        seen: set[Path] = set()
        for workflow, candidate in candidates:
            try:
                resolved = candidate.resolve()
            except OSError:
                continue
            if resolved in seen or not resolved.is_file():
                continue
            seen.add(resolved)
            importer = _importer_for(str(resolved))
            if importer == "none":
                continue
            print(
                f"[GuaxiAssetResolver][INFO] 启用项目可编辑资产：{asset_id} -> {resolved}",
                flush=True,
            )
            return {
                "id": asset_id,
                "source": "local",
                "kind": kind,
                "path": str(resolved),
                "importer": importer,
                "unitScale": 1.0,
                "contentHash": asset_file_hash(resolved),
                "editable": True,
                "provenance": {
                    "license": "user-supplied" if workflow != "generated-editable" else "project-generated",
                    "workflow": workflow,
                },
            }
        return None

    # -- 可插拔扩展点（默认未启用）--------------------------------------

    def _resolve_online(self, asset_id: str, kind: str, query_tags: set[str]) -> dict[str, Any] | None:
        """L2 PolyHaven/Sketchfab。需要网络与下载缓存；默认关闭。

        实现时应：搜索 -> 下载到工作区 assets/ -> 返回带 license/author/url 的 assetRef。
        """
        return None

    def _resolve_generative(self, asset_id: str, kind: str, description: str) -> dict[str, Any] | None:
        """L3 文/图生3D（Hyper3D/Hunyuan3D/Tripo）。需要 GPU 或云 API；默认关闭。"""
        return None

    def _resolve_procedural(
        self,
        asset_id: str,
        kind: str,
        query_tags: set[str],
        description: str,
        color_index: int,
    ) -> dict[str, Any] | None:
        """L4 可编辑程序化模型。

        这里仅描述生成配方，不调用 bpy。真正网格由 Blender 侧
        ``procedural_asset_builder`` 创建并保存为独立 ``.blend`` 文件。这样即使
        规则模型不够准确，用户也始终有一个可继续雕刻、替换材质或用 Python 修改的文件。
        """
        if self.generated_root is None or kind not in {"character", "environment", "prop", "map"}:
            return None
        path = self.generated_root / editable_asset_filename(asset_id)
        return {
            "id": asset_id,
            "source": "procedural",
            "kind": kind,
            "path": str(path),
            "importer": "blend",
            "unitScale": 1.0,
            "displayColor": _GREYBOX_PALETTE[color_index % len(_GREYBOX_PALETTE)],
            "editable": True,
            "generation": {
                "provider": "blender-procedural",
                "prompt": description.strip() or " ".join(sorted(query_tags)),
                "tags": sorted(query_tags),
                "recipeVersion": "1.0",
            },
            "provenance": {
                "license": "project-generated",
                "model": "guaxi-blender-procedural-1.0",
            },
        }

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

        # 项目中人工替换/修改过的资产永远优先。用户修改后重新执行 assets 构建，
        # 新内容哈希会进入 Build Plan；旧 replay 则由执行器拒绝静默漂移。
        editable = self._project_editable_asset(asset_id, kind)
        if editable is not None:
            return editable

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
                "contentHash": asset_file_hash(match["path"]),
                "editable": False,
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
        procedural = self._resolve_procedural(asset_id, kind, query_tags, description, color_index)
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
    return AssetResolver(
        root,
        project_dir=project_dir,
        allow_online=allow_online,
        allow_generative=allow_generative,
    )


def write_project_asset_manifest(build_plan: dict[str, Any], project_dir: str | Path) -> Path:
    """写出面向用户的资产状态表和编辑说明。

    该文件不是冻结计划的执行输入，只用于告诉用户模型文件在哪里、是否可编辑，以及
    修改后如何启用。执行输入仍是 ``story/build_plan.json``。
    """
    project_path = Path(project_dir).expanduser().resolve()
    assets_root = project_path / "assets"
    assets_root.mkdir(parents=True, exist_ok=True)
    items: list[dict[str, Any]] = []
    for asset in build_plan.get("assets", []):
        if not isinstance(asset, dict):
            continue
        path_value = str(asset.get("path") or "")
        model_path = Path(path_value) if path_value else None
        ready = bool(model_path and model_path.is_file())
        items.append(
            {
                "id": asset.get("id"),
                "kind": asset.get("kind"),
                "source": asset.get("source"),
                "status": "ready" if ready else "pending-generation",
                "modelPath": str(model_path) if model_path else "",
                "editable": bool(asset.get("editable")),
                "contentHash": asset.get("contentHash", ""),
                "generation": asset.get("generation", {}),
                "provenance": asset.get("provenance", {}),
            }
        )
    manifest_path = assets_root / "asset_manifest.json"
    with manifest_path.open("w", encoding="utf-8") as stream:
        json.dump(
            {
                "schemaVersion": "1.0",
                "planHash": build_plan.get("metadata", {}).get("planHash", ""),
                "instructions": "修改 editable=true 的模型后，在 Guaxi 点击“启用已修改模型”以重新冻结 Build Plan。",
                "assets": items,
            },
            stream,
            ensure_ascii=False,
            indent=2,
        )
    print(
        f"[GuaxiAssetResolver][INFO] 资产清单已写入：{manifest_path}（{len(items)} 项）",
        flush=True,
    )
    return manifest_path
