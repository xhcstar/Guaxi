# Guaxi Blender Bridge

Guaxi AI 影像工坊面向 Blender 5.2 LTS 的本地桥接插件与无界面执行器。

完整的打包、安装、配置、功能说明与“从 Markdown 生成视频”示例请阅读：

- [`docs/ai-film-studio-usage.md`](../docs/ai-film-studio-usage.md)
- [`docs/ai-film-studio-plan.md`](../docs/ai-film-studio-plan.md)

## 目录职责

- `guaxi_blender_bridge/__init__.py`：Blender 侧面板、本地 HTTP 服务和受控命令执行队列。
- `guaxi_blender_bridge/asset_resolver.py`：资产解析层（§7.1）。把资产需求解析为真实模型句柄，逐级降级到灰盒兜底。纯 JSON、可离线。
- `guaxi_blender_bridge/scene_director.py`：Story IR → 确定性 Build Plan 编译器（§7.2/§7.3）。产出并冻结 `story/build_plan.json`。
- `guaxi_blender_bridge/build_executor.py`：Build Plan 执行器。逐帧确定性重放，导入真实资产；仅在兜底时用彩色占位。
- `guaxi_blender_bridge/scene_builder.py`：预演编排（新版资产化路径 + 旧版立方体路径 + Build Plan 重放）与渲染/主时间线设置。
- `guaxi_blender_bridge/build_plan.schema.json`：Build Plan 数据契约。
- `guaxi_blender_bridge/headless_runner.py`：由 Tauri 通过 `blender --background` 调用的批处理入口。
- `guaxi_blender_bridge/protocol.schema.json`：Guaxi 与 Blender 之间的消息契约。
- `guaxi_blender_bridge/blender_manifest.toml`：Blender 5.2 扩展清单。

## 确定性资产化管线（消灭立方体）

默认路径已从"纯立方体预演"升级为"真实资产 + 冻结 Build Plan"：

```text
Story IR --scene_director--> build_plan.json（冻结，含 planHash）
        --build_executor--> 导入真实资产/兜底灰盒 --> .blend / PNG 序列
```

后台执行器 `--builder` 三种模式：

| 模式 | 行为 |
|---|---|
| `assets`（默认） | Asset Resolver 解析真实资产 + 冻结 `story/build_plan.json` + 重放渲染 |
| `greybox` | 旧行为，纯彩色立方体预演，用于结构验证与对照 |
| `replay` | 直接重放已冻结的 `story/build_plan.json`，确定性重渲染，不再解析或调用 LLM |

### 本地资产库索引

在资产库根目录放 `guaxi_asset_index.json`，Asset Resolver 据此按标签匹配真实模型：

```json
{
  "assets": [
    {
      "id": "env_han_tent_01",
      "kind": "environment",
      "path": "environments/han_tent_01.glb",
      "tags": ["汉代", "营帐", "tent", "military"],
      "unitScale": 1.0,
      "license": "CC-BY-4.0",
      "author": "作者",
      "url": "来源链接"
    }
  ]
}
```

- 库根目录由环境变量 `GUAXI_ASSET_LIBRARY` 指定；未设置时使用项目内 `assets/` 目录。
- `path` 必须位于库根内且文件存在（glb/gltf/usd/fbx/blend/obj）。
- `tags` 与 Story IR 的 `assetHints/searchTags/name/description` 做确定性重合度匹配。
- 未命中时逐级降级：在线库 → 文/图生3D → 程序化 → 灰盒；降级项写入 Build Plan 的 `metadata.assetGaps`。
- 在线库与生成式默认关闭，需要显式开启 `GUAXI_ALLOW_ONLINE_ASSETS=1` / `GUAXI_ALLOW_GENERATIVE_ASSETS=1`（当前为扩展点占位）。

## 安装

开发阶段先在工程根目录运行 `.\guaxi_blender_addon\package_plugin.ps1`，再在 Blender 的 Extensions/扩展设置中从磁盘安装生成的 ZIP。启用插件后：

1. 打开 `3D Viewport → Sidebar → Guaxi`。
2. 设置 Guaxi 项目工作区根目录。
3. 生成或填写访问令牌。
4. 启动本地桥接服务。

服务只监听 `127.0.0.1`，并且只接受协议允许列表中的结构化操作。它不会执行请求中的 Python、Shell 或任意表达式。

## 开发环境变量

- `GUAXI_BLENDER_PATH`：Guaxi 后端使用的 Blender 5.2 可执行文件。
- `GUAXI_BLENDER_ADDON_DIR`：本目录的绝对路径，用于打包后定位 `headless_runner.py`。

## 开发环境变量（资产管线新增）

- `GUAXI_ASSET_LIBRARY`：本地资产库根目录（含 `guaxi_asset_index.json`）。
- `GUAXI_ALLOW_ONLINE_ASSETS=1`：允许 L2 在线资产库（PolyHaven/Sketchfab，扩展点）。
- `GUAXI_ALLOW_GENERATIVE_ASSETS=1`：允许 L3 文/图生 3D（扩展点）。

## 当前里程碑

当前执行器会根据 Story IR 解析真实资产、冻结 Build Plan 并创建可播放的预演工程和 MP4。命中本地资产库的角色/环境/道具以真实模型出现；未命中项降级为灰盒并记入 `assetGaps`，用于验证场景顺序、时长、走位与镜头节奏。GIS 地形、在线/生成式资产、动作重定向、口型、配音与最终剪辑在项目计划（§12 M2–M6）中作为后续可替换阶段接入。
