# Guaxi Blender Bridge

Guaxi AI 影像工坊面向 Blender 5.2 LTS 的本地桥接插件与无界面执行器。

完整的打包、安装、配置、功能说明与“从 Markdown 生成视频”示例请阅读：

- [`docs/ai-film-studio-usage.md`](../docs/ai-film-studio-usage.md)
- [`docs/ai-film-studio-plan.md`](../docs/ai-film-studio-plan.md)

## 目录职责

- `guaxi_blender_bridge/__init__.py`：Blender 侧面板、本地 HTTP 服务和受控命令执行队列。
- `guaxi_blender_bridge/scene_builder.py`：Story IR 到预演场景的确定性构建器。
- `guaxi_blender_bridge/headless_runner.py`：由 Tauri 通过 `blender --background` 调用的批处理入口。
- `guaxi_blender_bridge/protocol.schema.json`：Guaxi 与 Blender 之间的消息契约。
- `guaxi_blender_bridge/blender_manifest.toml`：Blender 5.2 扩展清单。

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

## 当前里程碑

当前执行器会根据 Story IR 创建可播放的低模预演工程和 MP4，用于验证场景顺序、时长、人物占位与镜头节奏。正式角色资产、GIS 地形、动作重定向、口型、配音与最终剪辑在项目计划中作为后续可替换阶段接入。
