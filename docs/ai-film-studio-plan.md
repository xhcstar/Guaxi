# Guaxi AI 影像工坊：全流程架构与实施计划

实际安装与逐步操作请阅读：[AI 影像工坊使用手册](./ai-film-studio-usage.md)。

## 1. 产品定位

AI 影像工坊是 App 区域中的独立创作工作台，不与知识库强绑定：

- 可以从任意文件位置加载 UTF-8 Markdown。
- 文件选择器默认从 Guaxi 知识库根目录打开。
- 可以使用 Guaxi 已配置的任意 API 模型、自定义 OpenAI 兼容模型和本地模型。
- 可以在后台启动 Blender 构建/渲染，也可以连接已打开的 Blender 进行交互修改。
- 面向史书、地理、小说、纪录片、知识视频和个人微电影，不只是一次性“文生视频”。

核心分工：

```text
Guaxi：研究、证据、长文理解、导演决策、Story IR、任务编排、审核
Blender：资产、空间、相机、动画、灯光、VSE、合成、渲染
媒体模型：图片、视频、语音、音乐、字幕、修复、超分和风格化
```

## 2. 对 Pallaidium 的参考与边界

参考项目：[tin2tin/Pallaidium](https://github.com/tin2tin/Pallaidium)。Pallaidium 已经验证了“Blender VSE 内的生成式电影工作室”方向，具有文本/图片/视频/音频生成、可插拔模型、异步队列、远端后端、VSE Strip 元数据和 MCP 等能力。

Guaxi 应借鉴：

1. 生成媒体统一进入 VSE 时间线，而不是散落为不可追踪的文件。
2. 模型以 provider/plugin 发现，不在 UI 中硬编码每一家服务。
3. 本地与远端模型使用同一任务队列，具备排队、进度、取消、失败重试。
4. 每个 Strip 保存 prompt、negative prompt、seed、模型、版本、输入引用和生成时间，支持 Redo。
5. 支持“从成片回到字幕/剧本/分镜”的逆向工作流。

Guaxi 的差异化能力：

1. Markdown 长文与知识库上下文，而不只是一段 prompt。
2. 史料事实、地理坐标、年代、人物关系、出处与置信度。
3. 跨章节、跨镜头的一致性和稳定实体 ID。
4. Story IR 是 Guaxi、Blender、媒体生成服务之间的可审计协议。
5. 项目级资产谱系、许可记录、事实审核和发布检查。

集成路径有三种，按优先级排序：

- 路径 A：Guaxi 插件把 Story IR 编译为 VSE Strip，再调用 Pallaidium 已有生成操作。开发成本最低。
- 路径 B：Guaxi 实现同类媒体后端协议，Pallaidium/ComfyUI/自研服务作为 provider。
- 路径 C：只参考架构，Guaxi 完全独立实现生成队列。适合需要严格控制发布和依赖的后期阶段。

任何直接复用代码前必须检查 GPL-3.0 许可证对分发方式的影响。本次代码只参考公开架构，没有复制 Pallaidium 实现。

## 3. 系统架构

```text
任意 Markdown / 知识库 / GIS / 资产库
                  │
                  ▼
         Research & Source Layer
     事实、出处、地名、年代、人物、引用
                  │
                  ▼
       LLM Story Compiler（任意 modelid）
                  │
                  ▼
              Story IR 1.0
  metadata / research / entities / scenes / shots
       assetPlan / audioPlan / publishing
            │                     │
            │                     ├── 媒体生成队列
            │                     │   本地 / ComfyUI / /v1 / 云 API
            ▼                     ▼
      Blender Scene Builder ← 图片/视频/语音/音乐
            │
            ├── 低模 Animatic
            ├── 正式 3D 场景
            ├── 交互式 Bridge
            └── VSE Master Timeline
                       │
                       ▼
             渲染 / 合成 / 审核 / 发布
```

## 4. 当前已实现的基础

### 4.1 Guaxi 前端

- App 卡片和独立大窗口。
- 知识库默认选择与任意路径 Markdown 加载。
- 文稿编辑、模型选择、内容类型、视觉风格和目标时长。
- Story IR 编译、场景/镜头预览、JSON 人工校正。
- Blender 5.2 状态检测、可执行文件选择、项目创建、后台任务和取消。
- Blender Bridge 端口、令牌、探测和发送 Story IR。
- 结构化任务日志和完整流程面板。

### 4.2 Tauri 后端

- UTF-8 Markdown/BOM 安全读取和 16 MiB 上限。
- 复用 `chatview.rs` 现有 `modelid` 路由。
- Story IR JSON 解析、基础校验和版本归一化。
- 标准项目目录创建。
- Blender 5.2.x 严格版本探测，RC 显示开发警告。
- `--background --factory-startup --python-exit-code` 受控启动。
- stdout 进度协议、Tauri 事件、取消标记和退出状态日志。
- 固定 loopback 的交互式 Bridge 客户端；不允许前端指定任意远端 URL。

### 4.3 Blender 插件

- Blender 5.2 扩展清单。
- `127.0.0.1` HTTP Bridge、Bearer Token 和 1 MiB 请求上限。
- HTTP 线程只排队，`bpy.app.timers` 在 Blender 主线程执行。
- 写入路径被限制在用户配置的 Guaxi 工作区。
- 允许列表操作，不执行 Python/Shell/表达式。
- Story IR 低模预演：场景时长、时间线标记、人物占位、简单动作、相机、灯光、Eevee 和 MP4。

## 5. Story IR 设计

Story IR 必须版本化，所有跨阶段对象使用稳定 ID。

主要域：

- `metadata`：标题、类型、风格、语言、帧率、目标时长、Blender 版本。
- `research`：事实、原文锚点、置信度、研究缺口、敏感呈现。
- `entities`：人物、地点、道具；后续扩展组织、事件、朝代、路线。
- `scenes`：场景、地点、时间、原文锚点、时长。
- `shots`：景别、机位运动、动作、对白、旁白、画面文字、资产要求。
- `assetPlan`：资产类型、搜索标签、许可要求、来源和替换策略。
- `audioPlan`：旁白、对白、音乐、音效、声场。
- `production`：引擎、分辨率、色彩、交付物。
- `publishing`：画幅、平台、事实审核、版权审核。

后续 1.1 建议增加：

- `timeline`：绝对时间、历史年代与剧情时间的映射。
- `geography`：CRS、经纬度、海拔、范围、地图投影、比例尺。
- `continuity`：服装、伤痕、道具、时间、天气和人物状态连续性。
- `provenance`：每个镜头使用的事实、资产、模型和人工修改记录。
- `variants`：横屏、竖屏、短视频和语言版本的镜头裁切策略。

## 6. 标准项目目录

```text
项目名-xxxxxxxx/
  guaxi-film-project.json
  source/
    source.md
    citations.json
  story/
    story_ir.json
    screenplay.md
    storyboard.json
    continuity.json
  assets/
    characters/
    environments/
    props/
    maps/
    archive/
    licenses/
  audio/
    dialogue/
    narration/
    music/
    sfx/
  blender/
    preview.blend
    master.blend
    shots/
  renders/
    preview/
    final/
    frames/
  exports/
    subtitles/
    thumbnails/
    platform/
  cache/
  logs/
```

规则：

- 源文稿复制而不是移动。
- 最终渲染优先图片序列，MP4 是可重建交付物。
- 每个镜头可以单独 `.blend` 和渲染，Master 只链接资产和合成。
- 缓存可删除，Story IR、资产来源和人工修改不可丢失。

## 7. 媒体生成 Provider 栈

统一能力矩阵：

| 输入 | 输出 | 典型用途 |
|---|---|---|
| 文本 | 图片 | 概念图、角色设定、背景、封面 |
| 图片 | 图片 | 一致性修复、重绘、风格化、超分 |
| 文本/图片 | 视频 | 过场、历史想象镜头、无法建模的景象 |
| 视频/姿态 | 视频 | 动作控制、角色一致性、render-to-real |
| 文本/参考音频 | 语音 | 旁白、角色对白、声音克隆 |
| 文本 | 音乐/音效 | 配乐、环境音、拟音草案 |
| 音视频 | 文本 | 字幕、镜头描述、逆向剧本 |

Provider 接口至少声明：

- `id`、`displayName`、`mediaType`、`version`。
- 输入能力：文本、负面词、单图、多图、视频、音频、ControlNet、LoRA。
- 参数 Schema：尺寸、帧数、fps、steps、guidance、seed。
- 运行位置：Blender 本地、Guaxi 本地进程、远端 HTTP、云 API。
- 资源要求：VRAM、磁盘、依赖、许可证。
- `submit / poll / cancel / collect` 生命周期。

第一批适配：

1. Pallaidium 操作适配器。
2. ComfyUI API workflow 适配器。
3. OpenAI `/v1/models`、`/v1/files`、图片/音频风格接口。
4. Guaxi 已有 TTS 作为对白/旁白 provider。
5. Mock provider，用纯色图片、静音音频和短测试视频验证流水线。

## 8. VSE Master Timeline

每个生成结果必须成为 VSE Strip，并写入自定义属性：

```text
guaxi_project_id
guaxi_scene_id
guaxi_shot_id
guaxi_asset_id
guaxi_provider_id
guaxi_model_id
guaxi_model_version
guaxi_prompt_hash
guaxi_seed
guaxi_source_anchors
guaxi_license_record
guaxi_generation_job_id
guaxi_revision
```

VSE 频道建议：

- 1–9：正式视频/3D 场景。
- 10–19：替代镜头与参考。
- 20–29：对白、旁白。
- 30–39：环境音、音效、音乐。
- 40–49：字幕、标题、地图标注。
- 50+：注释、审核和临时控制 Strip。

## 9. 垂直领域能力

### 9.1 史书与历史

- 年代表和事件因果图。
- 人物同名消歧、官职和势力变化。
- 史料原句锚点与可靠度。
- 服饰、建筑、交通、兵器的时代约束。
- “史实”“学术推断”“艺术想象”画面标识。
- 战争、死亡、族群和宗教内容的敏感呈现审核。

### 9.2 地理与地图叙事

- GeoJSON、GPX、KML、CSV 坐标和 DEM 高程导入。
- 明确 CRS，禁止把经纬度直接当平面米制坐标。
- 地形、河流、行政区、迁徙/行军/旅行路线动画。
- 地图比例尺、指北针、时间滑块、地名标签避让。
- 卫星图、地图瓦片和 DEM 的许可与缓存记录。

### 9.3 小说和故事电影

- 角色设定卡、服装状态、关系图和跨镜头连续性。
- 场景节拍、冲突、情绪弧线和对白节奏。
- 角色外观参考包：正面/侧面/表情/服装/色板。
- 镜头级人工锁定：锁定的镜头不会被全局重新规划覆盖。
- 动作库、面捕/口型、配音声线与角色 ID 绑定。

## 10. 安全、版权与可靠性

- 禁止执行模型生成的 Python、Shell 或 Blender 表达式。
- 后台只运行仓库签入的 runner；正式发布应增加文件哈希校验。
- Bridge 只监听 loopback、使用强随机令牌、写入限制在项目工作区。
- API Key 不进入 Story IR、日志、Strip 元数据和项目清单。
- 所有外部资产记录来源、作者、许可证、允许用途和下载时间。
- 人脸、声音克隆和真实人物呈现需要授权字段。
- 史实镜头保存引用锚点；不确定内容进入 researchGaps。
- 固定 Blender 5.2.x 补丁版本；升级必须复制项目并运行回归测试。
- 单镜头渲染、断点续作和幂等 job ID，避免长片失败后全部重来。

## 11. 日志与可观测性

关键日志：

- 文稿路径、UTF-8 字节数、模型 ID，不记录 API Key。
- Story IR 的场景数、镜头数、警告数和 Schema 版本。
- Blender 路径、版本、是否预发布、启动参数类别，不记录令牌。
- job ID、stage、progress、退出状态、输出路径。
- provider、模型版本、seed、输入哈希、耗时、显存错误和重试次数。
- 人工修改人/时间/字段和是否锁定。

后续应为每个任务写 `logs/<job-id>.jsonl`，前端日志只是实时视图。

## 12. 分阶段实施

### M0：基础骨架（本次）

- 独立窗口、任意 Markdown、任意后端模型。
- Story IR、标准项目、Blender 5.2 检测。
- 低模预演、后台任务、交互式插件和安全协议。

验收：一篇 Markdown 能生成 Story IR、`.blend` 和简单 MP4；任务可取消。

### M1：真正的分镜工作台

- Story IR JSON Schema 独立文件和前后端完整验证。
- 镜头卡拖拽、时长调整、锁定、局部重生成。
- 分镜图片、contact sheet、旁白草稿和 SRT。
- VSE Master Timeline，Strip 写入完整 Guaxi 元数据。

验收：修改单镜头不会重做整片，重新打开项目能恢复全部状态。

### M2：Pallaidium/ComfyUI 与媒体队列

- Provider registry、能力发现和模型列表。
- Mock、Pallaidium、ComfyUI、OpenAI 风格远端适配器。
- submit/poll/cancel/collect、并发限制、显存调度和失败重试。
- 图片/视频/音频自动放入对应 VSE 频道。

验收：同一镜头可切换本地或远端 provider，元数据和 Redo 均可用。

### M3：资产、GIS 与历史制作

- Blender Asset Browser 索引与语义标签。
- glTF/USD/FBX/Blend 资产解析和单位统一。
- GeoJSON/DEM/路线动画与地图标注。
- 历史年代、服饰和建筑规则包；引用与艺术想象标记。

验收：地理文稿能生成带比例尺的路线/地形镜头；历史项目能输出事实审核表。

### M4：角色电影栈

- 角色设定一致性、动作重定向、口型和面部动画。
- Guaxi TTS 多角色声线、音频对齐、音乐和拟音。
- 连续性检查：服装、道具、方向、时间、光线和人物状态。
- 镜头级人工精修和导演批注往返。

验收：3–5 分钟多角色短片可断点制作，局部修改不破坏相邻镜头。

### M5：渲染农场与发布

- 本机多 GPU/远端节点调度、资产打包、检查点恢复。
- OCIO/AgX、响度、字幕安全区、闪烁和黑帧 QC。
- 16:9、9:16、1:1 自动重构图，不只是裁切。
- 封面、标题、章节、字幕、制作清单和平台发布包。

验收：同一项目可靠导出横屏与竖屏版本，并附事实/版权/模型使用清单。

## 13. 下一步优先级

1. 用 Blender 5.2 RC 实机跑通当前 headless runner，修正最终 API 差异。
2. 将 Story IR Schema 提取为共享 JSON Schema，并加入 Rust/Python 双端验证。
3. 为预演生成 VSE Master Scene 和带元数据的 Scene Strip。
4. 实现 Mock provider 与队列，先验证端到端媒体落盘和取消。
5. 评估 Pallaidium 操作适配器，决定直接互操作还是兼容其 remote backend contract。
6. 将 Guaxi 现有 TTS 接入对白/旁白轨道，输出 SRT 与 VSE Sound Strip。
