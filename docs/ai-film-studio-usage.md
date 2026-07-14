# Guaxi AI 影像工坊使用手册

本文档说明如何安装 Blender 5.2、安装 Guaxi Blender Bridge 插件，并从一篇 Markdown 文稿开始生成 Story IR、Blender 预演工程和 MP4 视频。

> 当前版本定位：已经可以完成“Markdown → Story IR → 低模场景预演 → `.blend`/MP4”。
>
> **重要说明（为什么现在渲染出的是立方体）：** 当前 M0 预演器 `scene_builder.py` 只生成彩色立方体占位符与背景板，不会导入任何真实模型，也没有接入媒体生成。因此现在的产物是**动态分镜（animatic）**，用来验证故事结构、节奏与机位，**不是电影级成片**。
>
> 要让画面变成文稿描述的真实场景与模型，需要新增的 **Asset Resolver（资产解析）+ AI Scene Director（AI 自主布景）+ ComfyUI/云端图像视频生成** 三层能力，属于 M1.5/M2 里程碑。详细升级方案见 [全流程架构与实施计划](./ai-film-studio-plan.md) 的 §0、§3.1、§7.1、§7.2、§12。

---

## 1. 当前可以完成什么

### 1.1 Guaxi 影像工坊

- 从知识库默认目录选择 Markdown。
- 从磁盘任意位置选择 Markdown。
- 在工作台内临时编辑文稿。
- 选择 Guaxi 已配置的云端、本地或自定义 OpenAI 兼容模型。
- 将文稿编译为结构化 Story IR。
- 查看场景、镜头、资产需求和研究缺口。
- 手工编辑 Story IR JSON。
- 创建标准影像项目目录。
- 检测并调用 Blender 5.2.x。
- 后台构建 `.blend`，或生成简单 MP4 预演。
- 取消正在执行的 Blender 后台任务。
- 通过本地 Bridge 把 Story IR 发送到已经打开的 Blender。

### 1.2 Blender 插件

- 在 Blender 中启动仅监听 `127.0.0.1` 的本地 Bridge。
- 使用 Bearer Token 验证 Guaxi 请求。
- 限制读写范围为用户指定的 Guaxi 项目工作区。
- 将 Story IR 构建为低模预演场景。
- 切换帧、选择对象、修改对象位置/旋转/缩放。
- 修改世界背景颜色。
- 保存 `.blend` 工程。
- 在 Blender 主线程执行场景操作，避免后台 HTTP 线程直接操作 `bpy`。

### 1.3 当前预演的画面特点

当前预演器主要验证（Blender 5.2 使用 `BLENDER_EEVEE` 引擎标识）：

- 场景顺序是否正确。
- 每个场景的持续时间是否合理。
- 镜头时间线标记是否正确。
- 人物在场关系和大致运动方向。
- Blender、渲染编码和项目落盘流程是否正常。

默认执行器现在会先经 **Asset Resolver** 尝试用真实模型替换立方体：命中本地资产库的角色/环境/道具以真实模型出现，未命中项才降级为彩色立方体占位，并记入 Build Plan 的 `assetGaps`。因此"是否还是立方体"取决于你是否配置了资产库（见 §1.4）。它仍可在投入完整渲染成本前验证故事结构。

> 升级路线：Asset Resolver 逐级降级（本地资产库 → PolyHaven/Sketchfab → 文/图生 3D → 程序化 → 立方体兜底）；后续由 **AI Scene Director** 自动布景、打光、定机位；写实画面再由 **ComfyUI/云端图像视频 API** 以 3D 场景为约束生成。见设计文档 §3.1、§7.1、§7.2、§7.3。

### 1.4 让画面出现真实模型（配置本地资产库）

默认执行器走 `assets` 模式：解析真实资产 → 冻结确定性 Build Plan（`story/build_plan.json`）→ 渲染。要让立方体变成真实模型，配置一个本地资产库即可：

1. 准备一个资产根目录，例如 `D:\GuaxiAssets`，放入 `.glb/.gltf/.usd/.fbx/.blend/.obj` 模型。
2. 在根目录建 `guaxi_asset_index.json`，为每个模型登记标签、类型与许可：

```json
{
  "assets": [
    {
      "id": "env_han_tent_01",
      "kind": "environment",
      "path": "environments/han_tent_01.glb",
      "tags": ["汉代", "营帐", "tent"],
      "unitScale": 1.0,
      "license": "CC-BY-4.0",
      "author": "作者",
      "url": "来源链接"
    },
    {
      "id": "char_liubang",
      "kind": "character",
      "path": "characters/liubang.glb",
      "tags": ["刘邦", "汉", "男性", "帝王"],
      "license": "自有"
    }
  ]
}
```

3. 设置环境变量 `GUAXI_ASSET_LIBRARY=D:\GuaxiAssets`（不设置时默认用项目内 `assets/` 目录）。
4. 重新点击"生成 MP4 预演"。标签命中 Story IR 的 `assetHints/name/description` 的资产会以真实模型出现；其余仍是立方体并列入 `assetGaps`。

确定性说明：同一 `build_plan.json` 重复渲染逐帧一致。想跳过资产解析直接重渲染，用 `replay` 模式（重放已冻结的 Build Plan）；想回到纯立方体对照，用 `greybox` 模式。

---

## 2. 系统要求

### 2.1 Guaxi

- 能正常启动当前 Guaxi Vue/Tauri 应用。
- 至少配置一个可用大模型。
- Markdown 使用 UTF-8 编码。

支持的模型类型：

- ChatGPT、DeepSeek、Qwen、Kimi、豆包等已接入 API。
- Guaxi 自定义 OpenAI 兼容模型。
- Local DeepSeek、Local Qwen 等 Guaxi 本地模型。

网页聊天模型不参与 Story IR 编译，因为网页会话不能提供稳定的结构化后端接口。

### 2.2 Blender

- 目标版本：Blender 5.2 LTS。
- 当前开发期可以使用 Blender 5.2 Release Candidate。
- 正式项目建议固定同一个 Blender 5.2.x 补丁版本。
- 不建议同一个项目混用 Blender 4.x、5.1 和 5.2。

开发期可以从 Blender 官方 Daily Builds/Release Candidate 页面取得 5.2；正式发布后应改用正式稳定包。

### 2.3 硬件建议

当前低模预演要求不高：

- 16 GB 内存。
- 支持 Blender 5.2 的显卡和驱动。
- 预留数 GB 项目空间。

后续使用本地视频生成模型时，硬件要求会显著提高，通常需要 NVIDIA GPU、较大显存和数十 GB 模型空间。也可以把媒体生成任务交给 ComfyUI 服务器或云端服务。

---

## 3. Blender Bridge 插件目录

插件位于 Guaxi 工程根目录：

```text
guaxi_blender_addon/
  README.md
  guaxi_blender_bridge/
    __init__.py
    blender_manifest.toml
    protocol.schema.json
    build_plan.schema.json
    asset_resolver.py
    scene_director.py
    build_executor.py
    scene_builder.py
    headless_runner.py
```

各文件作用：

| 文件 | 作用 |
|---|---|
| `blender_manifest.toml` | Blender 5.2 扩展名称、版本、最低 Blender 版本和权限声明 |
| `__init__.py` | 插件面板、本地 HTTP Bridge、鉴权、命令队列和 Blender 主线程执行 |
| `protocol.schema.json` | Guaxi 与 Blender 的请求协议和允许操作 |
| `build_plan.schema.json` | 确定性 Build Plan 数据契约（§7.3） |
| `asset_resolver.py` | 资产解析层：把资产需求解析为真实模型，逐级降级到灰盒兜底 |
| `scene_director.py` | 将 Story IR 编译为确定性 Build Plan 并冻结 |
| `build_executor.py` | 重放 Build Plan，导入真实资产，仅在兜底时用彩色占位 |
| `scene_builder.py` | 预演编排（资产化 / 旧立方体 / Build Plan 重放）与渲染、主时间线设置 |
| `headless_runner.py` | 后台模式入口，由 Tauri 使用 `blender --background` 调用 |

后台模式不要求插件已经在 Blender 设置中启用，因为 Guaxi 会直接调用仓库中的 `headless_runner.py`。只有“连接当前打开的 Blender”时，才需要安装并启用插件。

---

## 4. 打包 Blender 插件

Blender 从磁盘安装扩展时，ZIP 根目录需要直接包含：

```text
blender_manifest.toml
__init__.py
scene_builder.py
headless_runner.py
protocol.schema.json
```

不能让 ZIP 根目录多套一层无关文件夹。

### 4.1 Windows PowerShell 打包

在 Guaxi 工程根目录执行：

```powershell
.\guaxi_blender_addon\package_plugin.ps1
```

脚本会检查清单与入口文件、输出关键日志，并在工程根目录生成：

```text
GuaxiBlenderBridge-0.1.0.zip
```

也可以手工执行等效命令：

```powershell
Compress-Archive `
  -Path .\guaxi_blender_addon\guaxi_blender_bridge\* `
  -DestinationPath .\GuaxiBlenderBridge-0.1.0.zip `
  -Force
```

生成文件：

```text
GuaxiBlenderBridge-0.1.0.zip
```

如果修改过插件代码，应重新生成 ZIP，再在 Blender 中更新安装。

### 4.2 验证 ZIP 结构

打开 ZIP 后，第一层应该立即看到 `blender_manifest.toml` 和 `__init__.py`。如果第一层只看到 `guaxi_blender_bridge` 文件夹，需要重新打包。

---

## 5. 在 Blender 5.2 中安装插件

Blender 5.2 的具体文字可能随 RC/正式版略有变化，大致步骤如下：

1. 启动 Blender 5.2。
2. 打开 `Edit → Preferences`。
3. 进入 `Get Extensions` 或 `Add-ons/Extensions` 页面。
4. 点击右上角下拉菜单。
5. 选择 `Install from Disk...`。
6. 选择 `GuaxiBlenderBridge-0.1.0.zip`。
7. 确认安装并启用 `Guaxi Blender Bridge`。
8. 如果 Blender 提示扩展需要网络/文件权限，允许其访问本地网络和用户配置的项目目录。

插件只会监听本机回环地址 `127.0.0.1`，不会主动监听局域网网卡，也不会接受 Guaxi 指定任意远端主机。

### 5.1 更新插件

开发期间更新插件：

1. 停止 Blender 中的 Bridge。
2. 关闭旧 Blender 实例，确保 Python 模块不再被占用。
3. 重新打包 ZIP。
4. 在 Blender 扩展页面选择从磁盘重新安装/更新。
5. 重新启动 Blender。

如果 Blender 仍加载旧代码，可先卸载旧版，再重新安装。

### 5.2 卸载插件

1. 打开 Blender Preferences。
2. 找到 `Guaxi Blender Bridge`。
3. 先禁用插件。
4. 选择卸载。

卸载插件不会删除已创建的 Guaxi 影像项目。

---

## 6. 配置 Blender Bridge

安装完成后：

1. 进入 Blender 的 `3D Viewport`。
2. 按 `N` 打开右侧 Sidebar。
3. 选择 `Guaxi` 标签。
4. 面板标题为 `Guaxi AI 影像工坊`。

详细设置位于：

```text
Edit → Preferences → Add-ons/Extensions → Guaxi Blender Bridge
```

### 6.1 Guaxi 项目工作区

这是插件唯一允许写入的根目录。

例如：

```text
D:\FilmProjects
```

Guaxi 影像工坊中的“项目输出目录”也应选择：

```text
D:\FilmProjects
```

Guaxi 随后创建：

```text
D:\FilmProjects\鸿门宴-12ab34cd
```

因为这个项目在 Blender 配置的工作区内，插件可以构建和保存它。

如果 Blender 配置的是：

```text
D:\OtherProjects
```

但 Guaxi 项目在：

```text
D:\FilmProjects\鸿门宴-12ab34cd
```

插件会拒绝请求，并提示路径超出 Guaxi 工作区。这是安全设计，不是路径解析故障。

### 6.2 端口

默认端口：

```text
19152
```

通常不需要修改。如果端口被其他程序占用，可以改成大于等于 1024 的其他端口，例如：

```text
19153
```

Guaxi 界面中的 Bridge 端口必须和 Blender 插件一致。

### 6.3 访问令牌

在 Blender Guaxi 面板中点击：

```text
生成访问令牌
```

令牌用于保护本地命令接口。将它复制到 Guaxi 影像工坊的“插件访问令牌”输入框。

安全注意事项：

- 不要把令牌写入 Markdown。
- 不要把令牌写入 Story IR。
- 不要提交到 Git。
- 不要截图分享到公开场所。
- 怀疑泄露时，重新生成令牌。

### 6.4 启动 Bridge

点击：

```text
启动 Guaxi Bridge
```

面板应显示：

```text
Bridge 运行中
协议 1.0 · Blender 5.2.x
127.0.0.1:19152
```

停止时点击：

```text
停止 Guaxi Bridge
```

---

## 7. 打开 Guaxi AI 影像工坊

1. 启动 Guaxi。
2. 打开左侧主导航中的 `应用`。
3. 默认进入 `创作类`。
4. 点击 `AI 影像工坊`。
5. Guaxi 会打开一个专用大窗口。

窗口由三部分组成：

- 左侧：文稿、模型、项目和 Blender 控制。
- 中间：文稿、分镜、Story IR、全流程和日志。
- 右侧：制作原则、Pallaidium 对照策略和当前项目概况。

---

## 8. 左侧功能逐项说明

## 8.1 第 01 步：文稿与项目

### 从知识库选择

打开 Markdown 文件选择器，并将 Guaxi 知识库根目录作为默认位置。

适用场景：

- 文稿已经存放在知识库中。
- 需要快速从当前研究资料开始制作。

### 任意位置

打开普通 Markdown 文件选择器，不限制在知识库内。

适用场景：

- 小说文稿存放在其他写作目录。
- 从 Git 仓库、移动硬盘或临时目录加载脚本。

### 项目名称

默认使用 Markdown 文件名并移除扩展名。

例如：

```text
鸿门宴.md → 鸿门宴
```

项目名称用于输出目录名。Windows 不允许的字符会被自动删除。

### 项目输出目录

所有项目文件的父目录。

推荐：

```text
D:\FilmProjects
```

不要直接选择某个现有项目目录；Guaxi 会在输出目录下创建带唯一后缀的新项目。

---

## 8.2 第 02 步：故事编译模型

### 模型

模型列表来自 Guaxi 现有模型配置。

选择建议：

- 长篇史书：选择上下文较长、JSON 遵循能力强的模型。
- 本地隐私文稿：选择 Local Qwen 或 Local DeepSeek。
- 自定义服务：在 Guaxi 设置中配置 OpenAI 兼容 Base URL、API Key 和模型名，然后选择“自定义”。

模型只负责将文稿编译为 Story IR，不会直接执行 Blender Python。

### 内容类型

可选类型：

- `史书 / 历史`
- `地理 / 地图叙事`
- `小说 / 故事电影`
- `纪录片`
- `知识讲解`
- `其他`

选择会改变模型规划重点。

例如选择“史书 / 历史”时，模型需要保留：

- 年代和事件顺序。
- 人物关系。
- 原文锚点。
- 研究缺口。
- 艺术想象和史实的区别。

### 目标时长

单位是秒，范围 5–10800。

建议第一次使用：

```text
30–60 秒
```

先跑通短预演，再扩展为完整章节。长篇作品应拆成卷、章或场景，而不是一次生成数小时 Story IR。

### 视觉风格

预设包括：

- 电影写实。
- 历史纪录片。
- 水墨与三维融合。
- 风格化低多边形。
- 地图信息可视化。
- 二维动画。

当前低模预演不会直接达到这些最终风格，但视觉风格会写入 Story IR，为后续资产、媒体生成和渲染阶段使用。

### 制作模式

| 模式 | 用途 | 确定性 |
|---|---|---|
| Blender 原生 3D | **推荐默认（确定性路线）**：文本 LLM（如 DeepSeek）驱动 MCP 在本地导入真实资产、布景、打光、定机位，冻结为 Build Plan 后渲染；每次渲染逐帧一致 | 确定 |
| 混合制作：3D + AI 媒体 | 3D 保证空间/一致性，个别写实镜头再用 AI 生成质感；用 3D 的 depth/pose 约束以尽量可控 | 部分确定 |
| AI 镜头 + VSE 剪辑 | 主要通过 ComfyUI/云端图像视频 API 生成镜头，画质高但每次生成有随机性 | 随机 |
| 分镜预演 | 只关心故事、构图和时长，不追求最终画质（灰盒 animatic） | 确定 |

> 想要"每次渲染结果都一样、不靠 AI 随机生成"，选 **Blender 原生 3D**：文本 LLM 只在"规划"时调用一次生成 Build Plan，之后渲染是确定性重放。原理见设计文档 §7.3。

### 导演补充要求

这里写项目级约束。

历史示例：

```text
人物服饰遵循西汉早期考据。无法确定的建筑细节必须加入 researchGaps，
不能作为确定史实呈现。冲突场面克制，不出现写实伤口。
```

地理示例：

```text
所有地图镜头必须有指北针和比例尺。路线使用暖黄色，河流使用蓝色，
地名首次出现时显示现代名称和历史名称。
```

小说示例：

```text
整体为冷色悬疑风格。女主外套在全部夜景中保持深蓝色。
内心活动改为旁白，不要让角色直接说出心理描写。
```

### 编译 Story IR 与分镜

调用所选模型生成 Story IR。

完成后自动进入“分镜与场景”标签页。

如果失败：

- 检查模型服务是否启动。
- 检查 API Key 和 Base URL。
- 检查本地模型是否加载完成。
- 尝试缩短文稿。
- 查看“任务日志”。

---

## 8.3 第 03 步：Blender 5.2

### Blender 可执行文件

Guaxi 会尝试自动检测：

```text
C:\Program Files\Blender Foundation\Blender 5.2\blender.exe
```

如果使用 Blender 便携版或 RC 解压版，点击“选择”并指定：

```text
D:\Tools\blender-5.2.0\blender.exe
```

也可以设置环境变量：

```text
GUAXI_BLENDER_PATH=D:\Tools\blender-5.2.0\blender.exe
```

> 本机配置示例（对应常见解压安装）：
> - Blender：选择 `D:\blender-5.2.0\blender.exe`。
> - FFmpeg：选择 `D:\ffmpeg\bin`（可直接选目录，程序会自动定位其中的 `ffmpeg.exe`）或 `D:\ffmpeg\bin\ffmpeg.exe`。
> - 资产库：选择放置真实模型与 `guaxi_asset_index.json` 的目录（见 §1.4）。
>
> 这三项现在会持久化保存到应用配置（`config/guaxi_config.json`），**重启后自动恢复，不需要每次重新选择**。

### 检测环境

点击窗口右上角“检测环境”。

可能状态：

- `Blender 5.2.x`：版本匹配。
- `预发布版本警告`：检测到 5.2 RC，可以开发验证。
- `版本不匹配`：检测到 4.x、5.1 或 5.3，当前执行器不会启动。
- `未找到受控执行器`：Guaxi 没找到根目录插件文件。

打包后的 Guaxi 如果不能定位插件目录，可以设置：

```text
GUAXI_BLENDER_ADDON_DIR=D:\Guaxi\guaxi_blender_addon
```

### 交互式 Bridge

填写：

- Blender 插件显示的端口。
- Blender 插件生成的访问令牌。

点击“连接”。成功后显示“已连接”。

创建标准项目后，可以点击：

```text
发送 Story IR 到当前 Blender
```

适合：

- Blender 界面已经打开。
- 希望构建后立即检查和修改对象。
- 希望后续通过自然语言逐镜头调整。

### 创建标准制作项目

该按钮在以下条件满足后启用：

- 已加载 Markdown。
- 已生成 Story IR。
- 已指定项目名称和输出目录。

它会创建源文稿副本、Story IR、资产目录、音频目录、Blender 目录和渲染目录。

### 生成 MP4 预演

后台启动 Blender：

```text
blender --background --factory-startup --python-exit-code 17 --python headless_runner.py
```

生成的生产主产物：

```text
blender/preview.blend
renders/preview/frames/frame_000001.png
renders/preview/frames/frame_000002.png
...
```

检测到独立 FFmpeg 时还会生成：

```text
renders/preview/preview.mp4
```

可以通过 `GUAXI_FFMPEG_PATH` 指定 `ffmpeg.exe`。先渲染图片序列再编码是有意的生产设计：渲染中断时可以从缺失帧恢复，不会因为一个视频容器损坏而丢失全部结果。

任务运行期间会显示进度，可以取消。

### 仅构建 `.blend`

创建场景并保存 `.blend`，不渲染 MP4。

适合：

- 快速验证 Blender 构建是否正常。
- 想在 Blender 中人工检查后再渲染。
- 文稿较长，不希望立即生成全部预演帧。

---

## 9. 中间工作区功能

### 9.1 文稿工作台

显示当前 Markdown 源文稿。

可以直接修改。修改只影响当前工作台会话；创建项目时，修改后的内容会作为 UTF-8 `source/source.md` 保存。

如果重新选择文件，尚未保存到项目的临时修改会被替换。

### 9.2 分镜与场景

显示：

- 场景数。
- 镜头数。
- 目标时长。
- 资产需求数。
- 每个场景的标题、摘要和持续时间。
- 每个镜头的景别、相机运动、描述、旁白和时长。

检查重点：

1. 是否遗漏重要事件。
2. 场景顺序是否符合原文。
3. 镜头是否真的可视化。
4. 旁白是否过多。
5. 总时长是否合理。
6. 历史事实和艺术想象是否区分。

### 9.3 Story IR

显示完整 JSON。

可以手工修改后点击“应用 JSON 修改”。

常见修改示例：

将场景时长从 8 秒改成 12 秒：

```json
{
  "id": "scene_001",
  "durationSeconds": 12
}
```

将静态镜头改成推进：

```json
{
  "id": "shot_001",
  "cameraMovement": "dolly"
}
```

修改后会使已经创建的项目状态失效，需要重新点击“创建标准制作项目”，以保证磁盘中的 `story_ir.json` 与界面一致。

### 9.4 全流程

显示未来完整制作链（对应设计文档 §14 全流程功能栈 A–M）：

1. 采集与研究：多源导入（Markdown/PDF/EPUB/GeoJSON/DEM/CSV…）、实体链接、事实置信度、参考图板。
2. 故事与结构：Story IR、剧本、分镜、节拍、时长预算。
3. 角色与一致性：角色圣经、外观锁定、跨镜头/跨集连续性、声线绑定。
4. 世界与资产：Asset Resolver、GIS、历史考据规则包、程序化建模。
5. 摄影与导演：镜头语法、覆盖率、色彩脚本、Build Plan 冻结。
6. 媒体生成：ComfyUI/云 API、3D 约束的图/视频、修复超分（可选写实路线）。
7. 声音与配音：多角色 TTS、口型、音乐、音效、环境声、混音响度。
8. 剪辑与后期：VSE 装配、调色、字幕、标题、动态图形、地图标注。
9. 质量与审核：自动 QC、连续性、事实审核、导演批注往返。
10. 多版本与发布：智能重构图、平台预设、封面/标题/标签、编码打包。
11. 版权与合规：许可账本、肖像/声音授权、AIGC 标识、C2PA 内容凭证。
12. 生产管理与发布后闭环：系列/分集、版本、渲染农场、成本、数据回收、逆向到剧本。

这里是路线图说明，不代表所有阶段都已自动完成；当前已落地的是第 1–2 步与第 5 步的分镜预演（灰盒 animatic），其余随里程碑推进。

### 9.5 任务日志

记录：

- 文稿加载。
- Story IR 编译。
- Blender 检测。
- 项目创建。
- Bridge 连接与调用。
- Blender 后台进度和错误。

日志不会记录 Bridge Token 或 API Key。

---

## 10. 从零生成一个历史短视频

下面用“鸿门宴 45 秒预演”作为完整例子。

### 10.1 准备 Markdown

新建 UTF-8 文件：

```text
鸿门宴.md
```

内容：

```markdown
# 鸿门宴：宴席中的转折

## 背景

秦末，项羽军驻鸿门，刘邦军驻霸上。范增认为刘邦有争夺天下之志，劝项羽尽早除掉刘邦。

## 场景一：刘邦赴宴

清晨，刘邦带少数随从来到鸿门。营地戒备森严，士兵在帐外巡视。刘邦进入大帐，向项羽解释自己入关后的行动并无称王之意。

旁白：这场宴席表面上是解释误会，实际上决定着双方此后的命运。

## 场景二：项庄舞剑

范增多次向项羽示意，项羽没有回应。项庄提出舞剑助兴，实际准备寻找机会刺杀刘邦。项伯也拔剑起舞，用身体不断遮挡刘邦。

## 场景三：樊哙闯帐

樊哙持盾进入营帐，直视项羽。项羽被他的气势吸引，命人赐酒。紧张局面暂时改变。

## 场景四：刘邦离席

刘邦借口如厕离开营帐，随后从小路返回霸上。张良留下献礼并向项羽辞谢。

## 说明

视频需要把可确认的史实与影视化想象区分开。营帐细节、服装颜色和天气属于艺术设计，不应表述为确定史实。
```

### 10.2 在 Guaxi 中加载

1. 打开 `应用 → 创作类 → AI 影像工坊`。
2. 点击“任意位置”。
3. 选择 `鸿门宴.md`。
4. 确认项目名称为“鸿门宴”。
5. 输出目录选择 `D:\FilmProjects`。

### 10.3 选择编译参数

```text
模型：Local-Qwen 或可用的云端模型
内容类型：史书 / 历史
目标时长：45
视觉风格：历史纪录片
制作模式：混合制作：3D + AI 媒体
```

导演补充要求：

```text
使用克制的历史纪录片语气。项庄舞剑保持紧张但不出现血腥画面。
营帐和服饰无法从原文确认的部分加入 researchGaps。
每个场景保留原文章节作为 sourceAnchors。
```

### 10.4 编译 Story IR

点击：

```text
编译 Story IR 与分镜
```

预期得到约 4 个场景、若干镜头和资产需求。

检查：

- 场景一到场景四顺序正确。
- 没有虚构新的死亡或战斗结果。
- “营帐外观”“具体天气”等进入研究缺口或艺术设计。
- 总时长接近 45 秒。

### 10.5 手工修正

如果“项庄舞剑”只有 3 秒，可以在 Story IR 中把场景时长改成 10 秒，并调整其镜头时长。

修改后点击：

```text
应用 JSON 修改
```

### 10.6 创建项目

点击：

```text
创建标准制作项目
```

生成类似目录：

```text
D:\FilmProjects\鸿门宴-12ab34cd
```

### 10.7 选择工作方式

#### 方法 A：后台直接生成 MP4

1. 检测 Blender 5.2。
2. 点击“生成 MP4 预演”。
3. 切换到“任务日志”。
4. 等待进度达到 100%。
5. 点击“打开项目目录”。
6. 查看：

```text
renders\preview\preview.mp4
```

#### 方法 B：先生成 Blender 工程

1. 点击“仅构建 .blend”。
2. 打开：

```text
blender\preview.blend
```

3. 在 Blender 时间线上查看场景标记和占位人物。
4. 手工调整摄影机、灯光和对象。

#### 方法 C：发送到当前 Blender

1. Blender 插件工作区设置为 `D:\FilmProjects`。
2. 生成令牌并启动 Bridge。
3. 在 Guaxi 填写端口和令牌。
4. 点击“连接”。
5. 点击“发送 Story IR 到当前 Blender”。
6. 当前打开的 Blender 会构建预演场景。

### 10.8 当前视频之后怎么继续

当前得到的是结构预演。正式制作可以继续：

1. 用正式营帐和人物资产替换占位方块。
2. 为项庄、项伯、樊哙匹配动作。
3. 用 Guaxi TTS 生成旁白和对白。
4. 在 VSE 中加入声音、字幕和转场。
5. 通过 Pallaidium/ComfyUI 生成难以建模的过场镜头。
6. 使用 Eevee 做快速成片，关键镜头按需使用 Cycles。
7. 完成史实、素材许可证和平台规格检查。

---

## 11. 地理视频示例

Markdown：

```markdown
# 河西走廊路线可视化

## 范围

从兰州向西，经武威、张掖、酒泉、敦煌，展示河西走廊与祁连山、戈壁之间的空间关系。

## 镜头要求

1. 中国西北范围地图，标出河西走廊位置。
2. 摄影机下降到区域地形，显示祁连山和主要绿洲。
3. 路线从兰州依次延伸到敦煌。
4. 每个城市首次出现时显示名称。

## 数据说明

路线仅用于概念演示，正式版本需要使用可靠 GeoJSON 和 DEM 数据校正。
```

参数：

```text
内容类型：地理 / 地图叙事
视觉风格：地图信息可视化
目标时长：30 秒
```

导演要求：

```text
地图必须显示指北针、比例尺和数据来源。路线使用暖黄色，山地使用低饱和棕色。
如果缺少精确坐标，不得自行编造，写入 researchGaps。
```

当前版本会生成镜头规划和低模预演；GeoJSON、DEM 和地图标签系统将在 GIS 里程碑接入。

---

## 12. 小说短片示例

Markdown：

```markdown
# 雨夜末班车

## 场景：空站台

深夜，林夏独自站在被雨水打湿的站台。远处没有列车，只有信号灯时明时暗。

她收到一条没有署名的信息：不要上车。

广播：末班列车即将进站。

林夏抬起头，看见隧道深处出现两束灯光。
```

参数：

```text
内容类型：小说 / 故事电影
视觉风格：电影写实
目标时长：20 秒
```

导演要求：

```text
冷蓝色雨夜，红色信号灯作为唯一暖色。镜头从大全景逐渐推进到手机特写，
最后回到林夏面部近景。不要增加新的对白。
```

检查 Story IR 是否包含：

- 站台大全景。
- 手机信息特写。
- 广播作为声音而不是可见人物对白。
- 隧道灯光和林夏抬头动作。

---

## 13. 输出目录说明

创建项目后：

```text
项目目录/
  guaxi-film-project.json
  source/
    source.md
  story/
    story_ir.json
  assets/
    characters/
    environments/
    props/
  audio/
    dialogue/
    music/
    sfx/
  blender/
    preview.blend
  renders/
    preview/
      preview.mp4
    final/
  exports/
  cache/
  logs/
```

### 不要手工删除

- `guaxi-film-project.json`
- `source/source.md`
- `story/story_ir.json`

### 可以重新生成

- `cache/`
- 低清预览。
- 临时渲染帧。

### 建议备份

- 最终确认的 Story IR。
- 人工修改的 `.blend`。
- 正式资产和许可证记录。
- 配音、音乐和最终导出。

---

## 14. Bridge 功能与高级示例

普通用户通过 Guaxi 界面使用 Bridge 即可。下面的 HTTP 示例用于调试或开发插件。

假设：

```text
端口：19152
令牌：YOUR_TOKEN
```

### 14.1 健康检查

不需要令牌：

```powershell
Invoke-RestMethod http://127.0.0.1:19152/health
```

返回示例：

```json
{
  "ok": true,
  "service": "guaxi-blender-bridge",
  "protocolVersion": "1.0",
  "blenderVersion": "5.2.0"
}
```

### 14.2 Ping

```powershell
$headers = @{ Authorization = "Bearer YOUR_TOKEN" }
$body = @{
  protocolVersion = "1.0"
  requestId = "example-ping-001"
  operations = @(
    @{ type = "ping" }
  )
} | ConvertTo-Json -Depth 8

Invoke-RestMethod `
  -Method Post `
  -Uri http://127.0.0.1:19152/v1/commands `
  -Headers $headers `
  -ContentType "application/json; charset=utf-8" `
  -Body $body
```

### 14.3 切换到第 72 帧

操作：

```json
{
  "type": "set_frame",
  "frame": 72
}
```

用途：让 Guaxi 自然语言指令“跳到第 72 帧”落到确定性操作。

### 14.4 选择对象

```json
{
  "type": "select_object",
  "objectId": "GUAXI_CAMERA_PREVIEW"
}
```

### 14.5 修改对象位置

```json
{
  "type": "set_object_transform",
  "objectId": "GUAXI_CAMERA_PREVIEW",
  "location": [0.0, -10.0, 3.5],
  "rotationEuler": [1.2, 0.0, 0.0],
  "scale": [1.0, 1.0, 1.0]
}
```

位置、旋转和缩放受到范围限制，不能通过该操作执行任意表达式。

### 14.6 修改世界背景颜色

```json
{
  "type": "set_world_color",
  "color": [0.01, 0.02, 0.08]
}
```

颜色分量范围为 0–1。

### 14.7 构建 Story IR

```json
{
  "type": "build_story_ir",
  "projectDir": "D:\\FilmProjects\\鸿门宴-12ab34cd",
  "storyIr": {
    "schemaVersion": "1.0",
    "metadata": {
      "title": "鸿门宴",
      "fps": 24,
      "targetBlenderVersion": "5.2 LTS"
    },
    "scenes": [
      {
        "id": "scene_001",
        "title": "刘邦赴宴",
        "durationSeconds": 5,
        "shots": []
      }
    ]
  },
  "renderAnimation": false
}
```

`projectDir` 必须位于插件配置的工作区内。

### 14.8 保存工程

```json
{
  "type": "save_project",
  "filepath": "D:\\FilmProjects\\鸿门宴-12ab34cd\\blender\\manual.blend"
}
```

只允许 `.blend` 扩展名，且必须位于工作区内。

---

## 15. 后台模式与交互模式怎么选

| 情况 | 推荐模式 |
|---|---|
| 第一次验证环境 | 仅构建 `.blend` |
| 想快速查看节奏 | 生成 MP4 预演 |
| Blender 已打开，想立即修改 | 交互式 Bridge |
| 批量处理多个章节 | 后台模式 |
| 最终人工精修 | 交互式 Blender |
| 长时间最终渲染 | 后台队列/未来渲染节点 |

后台模式与交互模式可以共存，但不要让两个任务同时写同一个 `.blend` 文件。

---

## 16. 常见问题

### 16.1 Guaxi 找不到 Blender

处理：

1. 点击 Blender 路径旁的“选择”。
2. 指向 Blender 5.2 的实际可执行文件。
3. 或设置 `GUAXI_BLENDER_PATH`。
4. 重新点击“检测环境”。

### 16.2 提示版本不兼容

当前执行器严格要求 Blender 5.2.x。

不要用 Blender 4.5 LTS 或 5.1 强行运行。安装/选择 5.2 后重新检测。

### 16.3 检测到 RC 警告

这是开发期预期状态，可以生成测试项目。正式生产时等待并改用 Blender 5.2 正式稳定版。

### 16.4 插件无法启动 Bridge

检查：

- 是否填写工作区。
- 端口是否被占用。
- 端口是否小于 1024。
- 插件是否有本地网络权限。
- Blender 控制台中的 `[GuaxiBridge][ERROR]` 日志。

### 16.5 Guaxi 连接 Bridge 失败（含 502 / 连接超时）

检查：

1. Blender 是否仍在运行。
2. Bridge 面板是否显示运行中。
3. Guaxi 和 Blender 端口是否一致。
4. 防火墙/安全软件是否阻断本机回环连接。

**关于 502 / 连接超时（常见根因：系统代理拦截了 127.0.0.1）**

如果插件已启动但 Guaxi 连接时报 502 Bad Gateway 或超时，通常不是插件问题，而是系统/环境配置了 HTTP 代理，把发往 `127.0.0.1` 的回环请求也转发给了代理，代理无法回连本机于是返回 502。

当前版本已在 Guaxi 端为 Bridge 请求禁用代理（`no_proxy`），大多数情况下无需额外配置。若仍出现，请在系统或环境变量中把回环地址加入代理例外：

```text
NO_PROXY=127.0.0.1,localhost
no_proxy=127.0.0.1,localhost
```

设置后重启 Guaxi 再连接。

健康检查可以验证服务是否在线（PowerShell 也需绕过代理）：

```powershell
Invoke-RestMethod -Uri http://127.0.0.1:19152/health -Proxy $null
```

Linux/macOS：

```bash
curl --noproxy '*' http://127.0.0.1:19152/health
```

### 16.6 提示 Unauthorized

令牌不一致。

1. 在 Blender 中重新生成令牌。
2. 完整复制到 Guaxi。
3. 不要带前后空格。
4. 重新连接。

### 16.7 提示路径超出工作区

确保 Blender 插件的工作区是 Guaxi 输出目录或其父目录。

推荐统一：

```text
Blender 工作区：D:\FilmProjects
Guaxi 输出目录：D:\FilmProjects
```

### 16.8 Story IR 不是有效 JSON

处理：

- 换用 JSON 遵循能力更好的模型。
- 缩短文稿。
- 减少过多导演要求。
- 检查自定义模型是否返回额外 Markdown 代码块。
- 查看任务日志中的解析错误。

后端会尝试从常见代码块中提取 JSON，但仍要求最终根节点是对象并包含场景数组。

### 16.9 Story IR 内容不准确

不要立即渲染：

1. 检查 `research.facts`。
2. 检查 `sourceAnchors`。
3. 将疑点放入 `researchGaps`。
4. 在 Story IR 标签页手工修正。
5. 重新创建标准项目。

历史和地理项目必须人工审核事实。

### 16.10 `.blend` 已生成但 MP4 不存在

可能原因：

- FFmpeg/H.264 编码失败。
- 渲染任务被取消。
- Blender 在某帧异常退出。
- 输出目录没有写权限。

查看：

- Guaxi 任务日志。
- Blender stderr 日志。
- `blender/preview.blend` 是否存在。

如果 `.blend` 已存在，可以在 Blender 中打开并手工渲染动画。

### 16.11 中文显示异常

Markdown 和 Story IR 都使用 UTF-8。Blender 自带字体可能不包含全部中文字形，因此当前预演主要把中文标题保存在对象名称和时间线标记中。正式字幕和画面文字应指定包含中文的授权字体。

### 16.12 视频只是方块和背景板

**首先确认是否配置了本地资产库（§1.4）。** 默认 `assets` 模式会用真实模型替换立方体，但前提是资产库里有标签匹配的模型；没有配置库、或标签没命中时，才会降级为立方体并写入 `assetGaps`。因此"全是方块"通常意味着资产库为空或标签不匹配，而不是渲染错误。

排查：

1. 是否设置了 `GUAXI_ASSET_LIBRARY` 且目录含 `guaxi_asset_index.json`。
2. 索引里的 `path` 文件是否真实存在、扩展名是否受支持。
3. `tags` 是否与 Story IR 中该角色/地点的 `name/assetHints/description` 有重合词。
4. 查看 `story/build_plan.json` 的 `metadata.assetGaps`，确认哪些资产未命中。

要得到更完整的电影级画面，还需要以下正在规划的升级（详见设计文档 §0/§3.1/§7.1/§7.2/§7.3/§12）：

- **Asset Resolver 资产解析层：** 本地资产库 → PolyHaven/Sketchfab → 文/图生 3D（Hyper3D/Hunyuan3D/Tripo）→ 程序化 → 立方体兜底，真正把 `assetHints/assetPlan` 变成场景里的真实模型。
- **AI Scene Director：** AI agent 通过 MCP/Bridge 自主导入资产、布景、打光、定机位、加动画，并读 viewport 截图自检迭代（把体力活交给 AI，参考 blender-mcp）。
- **ComfyUI + 云端图像/视频生成：** 用 3D 场景导出的 depth/pose 作 ControlNet 约束，生成电影级写实画面并落入 VSE strip。
- 动作、口型、配音与 VSE Master Timeline。

在这些能力落地前，请把当前输出当作"动态分镜预演"使用；升级完成后，立方体将被真实资产替换，缺失资产才继续用立方体占位并明确标记。

---

## 17. 推荐的实际创作方法

不要直接把整部长篇小说一次生成电影。推荐层级：

```text
作品
  → 卷/章节
    → 场景
      → 镜头
        → 资产/动作/声音/渲染任务
```

第一次项目建议：

- 1 篇 Markdown。
- 3–5 个场景。
- 30–60 秒。
- 720p 的 50% 预演分辨率。
- 先生成 `.blend`，再生成 MP4。

稳定后再增加：

- 跨章节角色连续性。
- 正式资产。
- AI 视频镜头。
- 多角色配音。
- 横屏/竖屏多版本。

---

## 18. 发布前检查

历史/地理/纪录片：

- [ ] 每个重要事实有来源锚点。
- [ ] 不确定信息已标记。
- [ ] 地图坐标、投影、比例尺正确。
- [ ] 艺术想象没有冒充史实。

资产与生成媒体：

- [ ] 人物、模型、贴图、字体、音乐和地图数据有许可证记录。
- [ ] 使用真实人物肖像或声音时获得授权。
- [ ] 记录模型、版本、seed、prompt 和输入素材。

视频技术：

- [ ] 没有黑帧、掉帧或异常闪烁。
- [ ] 字幕在安全区域内。
- [ ] 对白、旁白和音乐响度合理。
- [ ] 画幅符合目标平台。
- [ ] 横屏和竖屏版本分别检查构图。

---

## 19. 相关文档

- [全流程架构与实施计划](./ai-film-studio-plan.md)
- [Blender 插件目录说明](../guaxi_blender_addon/README.md)
- [Pallaidium 项目](https://github.com/tin2tin/Pallaidium)
- [blender-mcp（AI 通过 MCP 双向驱动 Blender、集成 PolyHaven/Sketchfab/文生3D）](https://github.com/ahujasid/blender-mcp)
- [Blender Python API](https://docs.blender.org/api/current/)
- [Blender 命令行参数](https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html)
