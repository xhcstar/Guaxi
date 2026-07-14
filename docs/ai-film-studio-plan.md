# Guaxi AI 影像工坊：全流程架构与实施计划

实际安装与逐步操作请阅读：[AI 影像工坊使用手册](./ai-film-studio-usage.md)。

## 实现状态板（诚实基线，2026-07）

> 用于区分“已落地可用”“已搭骨架待接后端”“仍属规划”。避免把占位当成成品。

| 能力 | 状态 | 说明 |
|---|---|---|
| Markdown → Story IR 编译 | ✅ 已落地 | 复用现有 modelid |
| Story IR/Build Plan 校验 | ✅ 已落地 | Rust + Python 双侧，坏计划执行前拦截 |
| 确定性 Build Plan 冻结 + 重放 | ✅ 已落地 | `scene_director` + `build_executor`，含 planHash |
| Asset Resolver L1 本地库 + L4 可编辑程序化模型 | ✅ 已落地 | 未命中时生成独立 `.blend`，不再只留场景内立方体 |
| 模型编辑 → 哈希检测 → 重新启用 | ✅ 已落地 | `assets/asset_manifest.json` + “启用已修改模型” |
| 真实资产导入（glTF/USD/FBX/OBJ/blend） | ✅ 已落地 | `.blend` 资产已通过 Blender 5.2 RC 实机构建与渲染 |
| 合规工件（许可账本/模型使用/AIGC 标识） | ✅ 已落地 | 发布前必需 |
| 每任务 JSONL 日志 | ✅ 已落地 | `logs/<job-id>.jsonl` |
| 平台发布预设 | ✅ 数据已备 | 编码/导出层待接线 |
| 路径持久化 / 窗口尺寸 / 代理 502 / 打包脚本 | ✅ 已修复 | 见使用手册 |
| Asset Resolver L2 在线库（PolyHaven/Sketchfab） | 🚧 扩展点 | 接口已留，需接 API + 下载缓存 |
| Asset Resolver L3 文/图生 3D | 🚧 扩展点 | 接 Hunyuan3D 等服务；输出仍缓存为可编辑模型文件 |
| 媒体生成后端（ComfyUI/云图像视频） | 🚧 规划 | 需 GPU/API 与队列实现 |
| AI Scene Director 截图自检循环 | 🚧 规划 | 需多模态回读 + 迭代 |
| 声音/配音/口型、后期调色、多画幅重构图 | 🚧 规划 | 见 §17–§21 |

> 说明：可编辑 `.blend` 生成、再次导入、48 帧 Eevee 渲染、分镜相机切换、资产修改哈希失效与重新启用，已在 Blender 5.2.0 LTS RC 实机验证。其他导入格式仍需用实际资产分别回归。标 🚧 的部分依赖 GPU、外部 API 或大量新代码，属后续里程碑。

## 0. 修订说明（2026-07 架构升级）

### 0.1 为什么当前只渲染出"会动的立方体"

用户反馈：文稿再详细，渲染结果都是一个个彩色立方体在灰色地面上平移，而不是文稿描述的场景与模型。经排查，这不是渲染 bug，而是当前 M0 架构的必然结果，根因有四层：

1. **构建器只会造几何图元。** `scene_builder.py` 里 `_create_actor_placeholder()` 对每个角色调用 `primitive_cube_add`（彩色立方体），环境只是背景板 `primitive_cube_add` + 地面 `primitive_plane_add`，材质只有纯色 `diffuse_color`。无论 Story IR 描述多丰富，这段代码的输出上限就是"彩色盒子 + 灰地面"。
2. **有资产需求数据，却没有资产解析层。** LLM 编译出的 `entities.characters[].assetHints`、`assetPlan`、`shot.assetRequirements`、`searchTags` 全部被 `scene_builder.py` 忽略，没有任何代码去查询资产库或导入真实模型。
3. **没有任何媒体生成后端落地。** Provider 栈、ComfyUI/Pallaidium 适配、文生图、图生视频全是"未来里程碑"，当前为空。VSE 主时间线里只有一条灰盒 3D 场景 strip。
4. **产品定位错配。** 工具默认且唯一能跑通的"生成 MP4 预演"渲染的就是灰盒 animatic，但对外表述为"电影级视频"，用户合理地认为工具坏了。

结论：**当前造的是"故事结构验证器（animatic）"，却被当成"成片生成器"在用。**

### 0.2 修订后的目标与主路线

按用户诉求（一人工作室、把建模布景等体力活交给 AI、云端图像/视频 API + 本地 ComfyUI），架构升级为：

- **主路线改为“AI 辅助 + Blender 原生 3D 成片”。** AI 负责理解文稿、提出资产需求和布景决策；模型文件、场景、动画、灯光、相机与最终像素均由 Blender 管理和渲染。ComfyUI/云视频只作为可选增强，不是默认终点。
- **新增 AI 场景导演（AI Scene Director）层。** 在 Story IR 与 Blender 之间插入一个 agent 循环，逐镜头自主驱动 Blender：导入真实资产、布景、打光、定机位、加动画，并读取 viewport 截图自检迭代。参考 [ahujasid/blender-mcp](https://github.com/ahujasid/blender-mcp) 的双向 socket + 资产集成模式。
- **新增 Asset Resolver（资产解析层）。** 真正消费 `assetHints/assetPlan`，逐级降级获取真实模型。
- **新增可编辑资产底线。** 外部库/AI 未命中时也生成独立、分件的 `.blend`；用户可在 Blender 或 Python 中继续修改，再显式启用到新的 Build Plan。
- **接入媒体生成后端。** 本地 ComfyUI + 云端图像/视频 API，用 3D 场景导出的 depth/pose 作 ControlNet 约束生成电影级画面。
- **修订安全模型。** 原"禁止执行任何 AI 生成 Python"与"AI 自主建模"冲突，改为分层执行模型（见 §10）。

修订后完整数据流见 §3.1；分层执行安全模型见 §10.1；里程碑见 §12。

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
            │                     │   本地 ComfyUI / 云图像视频 API / /v1
            ▼                     ▼
   AI Scene Director（agent 循环）← 图片/视频/语音/音乐
            │
            ├── Asset Resolver（本地库→PolyHaven→Sketchfab→文/图生3D→程序化→灰盒兜底）
            ├── Blockout 兜底构建器（旧 scene_builder，仅在无资产时占位）
            ├── 交互式 Bridge / MCP（双向 socket，可读 viewport 截图自检）
            └── VSE Master Timeline
                       │
                       ▼
             渲染 / 合成 / 审核 / 发布
```

### 3.1 修订后的核心数据流（M2+ 目标）

灰盒不再是终点，而是 AI 场景导演的起始 blockout。逐镜头循环：

```text
for each shot in Story IR:
  1. Asset Resolver 解析 shot 需要的 characters/environment/props：
     本地资产库命中 → 直接导入
     未命中 → PolyHaven / Sketchfab 搜索下载
     仍未命中 → 文/图生 3D（Hyper3D Rodin / Hunyuan3D / Tripo）
     仍不可行 → 程序化生成独立可编辑 .blend（人物/环境/道具/地图）
     文件物化失败 → 最后才灰盒占位并写入 assetGaps
  2. AI Scene Director（agent）布景：摆放资产、定相机、打光、加动画、
     设置世界/雾/景深，参照 shot.description / cameraMovement / visualStyle。
  3. Director 渲染 viewport 截图 → 多模态自检"画面是否符合镜头描述" →
     不符合则调整并重试（有限次数，超限降级并记录）。
  4.（可选，媒体生成路线）导出该镜头的 depth/normal/pose pass →
     作为 ControlNet 约束送入 ComfyUI / 云端图生图/图生视频 →
     生成电影级画面 → 落入 VSE 对应频道 strip。
  5. 每步写入 provenance：使用的资产 ID、来源、许可证、prompt、seed、模型版本。
```

关键点：

- **AI 只做资产/布景决策，不接管最终像素。** 模型来自资产库、程序化配方或可选文生 3D；AI 负责“需要什么、用哪个、放哪、怎么打光、机位怎么走”，最后由 Blender 渲染。
- **确定性兜底始终存在。** 任一环节失败都能退回灰盒，保证"至少能出 animatic"，不会整片失败。
- **两条出画路线可切换。** 纯 3D 渲染（Eevee/Cycles）或 3D 约束 + AI 生成，由制作模式决定。

### 3.2 AI Scene Director 与 Blender 的通道

参考 [ahujasid/blender-mcp](https://github.com/ahujasid/blender-mcp)：Blender 内插件开 socket，外部 agent 通过 MCP/HTTP 下发指令并回读场景状态与 viewport 截图，形成"下发→执行→截图→自检→再下发"闭环。

Guaxi 在其基础上强化：

1. **稳定实体 ID 贯通。** 每个导入对象绑定 Story IR 的 `char_/loc_/prop_` ID，跨镜头一致，支持局部替换而不重排全片。
2. **分层执行安全（§10.1）。** 默认只走允许列表 + 参数化模板；AI 直接执行 Python 属 Tier2，需显式授权、限定工作区、diff 确认与审计日志。
3. **provider 无关。** 驱动 Director 的 agent 可以是 Guaxi 已配置的任意 modelid，也可以是外部 MCP 客户端。

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

> 当前边界：默认 assets 路径已消费人物、地点、`assetRequirements`、`assetPlan` 道具，并为未命中项生成独立可编辑 `.blend`；镜头相机按 Timeline Marker 切换。程序化模型属于可继续精修的基础模型，不等同于任意物体的电影级精模。L2 在线资产和 L3 文/图生 3D 仍待接入，升级路线见 §7.1、§7.2。

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
- `characterBible`：角色外观锁定、服装状态集、资产/声线绑定、一致性锚（§16.1）。
- `continuity`：服装、伤痕、道具、时间、天气和人物状态连续性（§16.2）。
- `cinematography`：镜头语法、覆盖率、镜头/景深、色彩脚本、节奏（§18）。
- `audioPlan`（扩展）：多角色声线、口型、音乐、音效、环境声、混音与响度目标（§17）。
- `provenance`：每个镜头使用的事实、资产、模型和人工修改记录。
- `compliance`：许可账本、肖像/声音授权、AIGC 标识、C2PA 凭证、模型使用记录（§22）。
- `distribution`：平台预设、多画幅、封面/标题/标签、本地化、发布排期（§21）。
- `variants`：横屏、竖屏、短视频和语言版本的镜头裁切策略。

Story IR 表达"叙事意图"，是随机的 LLM 产物；由它派生的 **Build Plan（`story/build_plan.json`，见 §7.3）** 表达"确定性执行"，一经冻结即作为渲染的唯一依据。两者分离，是"文本 LLM 驱动 + 每次渲染确定"的关键。

## 6. 标准项目目录

> 面向完整功能栈（§14）扩展后的目录。M0 阶段只创建其中一部分，其余随里程碑逐步启用；未用到的子目录可缺省。

```text
项目名-xxxxxxxx/
  guaxi-film-project.json      # 项目清单：层级、版本、Build Plan 哈希、成本汇总
  source/
    source.md                  # 源文稿副本
    imports/                   # 原始导入（pdf/epub/geojson/dem/csv…）
    citations.json             # 事实—来源锚点—置信度
    knowledge_graph.json       # 实体、关系、时空坐标（§15.2）
    moodboard/                 # 参考图板 + 来源/许可标注
  story/
    story_ir.json              # 叙事意图（随机 LLM 产物）
    build_plan.json            # 冻结的确定性搭建计划（§7.3），渲染据此重放
    screenplay.md
    storyboard.json
    continuity.json            # 连续性状态机（§16.2）
  characters/                  # 角色圣经（§16.1）：每角色一册
    <char_id>/bible.json + refs/ + wardrobe/
  assets/
    characters/ environments/ props/ maps/ archive/
    licenses/                  # 资产许可记录
  audio/
    dialogue/ narration/ music/ sfx/ ambience/
    mix/                       # 混音与响度归一产物
  blender/
    preview.blend  master.blend  shots/
  renders/
    preview/ final/ frames/
  review/
    qc_report.json             # 自动 QC（§20）
    continuity_report.json
    fact_review.json           # 事实审核清单（史书/地理）
    notes/                     # 导演批注往返
  exports/
    subtitles/                 # 多语言 SRT/ASS
    localization/              # 多语言字幕/配音/画面文字
    thumbnails/                # 封面（可 A/B）
    metadata/                  # 标题、简介、标签、章节草案
    platform/                  # 按平台预设打包的成片（横/竖屏…）
  compliance/
    license_ledger.json        # 许可账本（§22）
    consent/                   # 肖像/声音授权
    model_usage.json           # 模型/版本/seed/prompt/输入哈希
    aigc_label.json            # AIGC 合规标识与说明
    content_credentials/       # C2PA 内容凭证
  analytics/                   # 发布后数据回收（§24）
  cache/
  logs/                        # 每个 job 的 logs/<job-id>.jsonl
```

规则：

- 源文稿复制而不是移动。
- 最终渲染优先图片序列，MP4 是可重建交付物。
- 每个镜头可以单独 `.blend` 和渲染，Master 只链接资产和合成。
- 缓存可删除，Story IR、Build Plan、资产来源和人工修改不可丢失。
- Build Plan 一旦冻结即为确定性重放依据；重新调用 LLM 规划前应保留旧版本以便 diff 与回滚。

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

### 7.1 Asset Resolver（资产解析层，新增核心）

这是解决"只有立方体"的第一块拼图。它是 Story IR `entities/assetPlan` 与 Blender 场景之间唯一的资产入口，把"资产需求"变成"场景里的真实模型"。

解析顺序（逐级降级，每级失败进入下一级）：

| 级别 | 来源 | 说明 |
|---|---|---|
| L1 本地资产库 | Blender Asset Browser、项目 `assets/`、用户资产根 | 语义标签匹配 `assetHints/searchTags`；优先，可离线、可复用、许可清晰 |
| L2 在线资产库 | PolyHaven（HDRI/贴图/模型）、Sketchfab（CC 模型） | 按标签搜索、下载、缓存到项目 `assets/`；记录作者与许可证 |
| L3 文/图生 3D | Hyper3D Rodin、Tencent Hunyuan3D、Tripo 等 | 用 `description/appearance` 生成网格；适合库中没有的特定角色/道具 |
| L4 程序化 | Blender 受控配方 / 后续 Geometry Nodes 模板库 | 已能生成并保存分件式人物、环境和常见道具；后续扩展地形、建筑、道路等模板 |
| L5 灰盒兜底 | 现有 `scene_builder` 占位 | 前四级都失败时占位，并写入 `assetGaps`，绝不让整片失败 |

Resolver 契约：

- 输入：一个 `entity`（character/location/prop）或 `assetRequirement` + `visualStyle` + 目标引擎。
- 输出：可导入 Blender 的资产句柄（glTF/USD/FBX/blend 路径或库引用）+ 归一化单位/朝向 + provenance（来源、作者、许可证、下载时间、生成模型与 seed）。
- 缓存：同一 `assetHints` 命中的资产写入 `assets/` 并在项目内建立"实体 ID → 资产"映射，跨镜头稳定复用，避免同一角色每个镜头长得不一样。
- 许可闸门：L2/L3 结果必须带许可证记录才能进入发布交付；无许可只能用于内部预演。
- 可编辑闭环：项目生成模型写入 `assets/generated/*.blend`，旁边保留 `.asset.json` 配方，汇总清单写入 `assets/asset_manifest.json`。
- 修改启用：用户可直接编辑生成模型，或用 `assets/asset_overrides.json`/`assets/overrides/` 指定替换文件；点击“启用已修改模型”后重新计算 SHA-256 并冻结新计划。

### 7.2 AI Scene Director（AI 场景导演，新增核心）

第二块拼图。它是把"资产 + 镜头描述"变成"电影级构图"的 agent 层，替用户干布景、打光、机位、动画这些体力活（对应用户"一人工作室、体力活交给 AI"的诉求）。

职责边界：

- **AI 负责决策，不负责发明像素。** 它决定"用哪个资产、放在哪、相机在哪、怎么运动、几盏灯什么色温、加什么动画、世界与雾/景深怎么设"。
- **真实模型来自 Asset Resolver，真实画面来自渲染或媒体 provider。**

工作循环（逐镜头）：

```text
plan → act → render viewport 截图 → 多模态自检 → 修正 → 收敛或降级
```

1. `plan`：读 shot 的 `shotType/cameraMovement/description/action/characters/visualStyle`，规划布景与镜头语言。
2. `act`：通过 Bridge/MCP 下发确定性操作（导入资产、set transform、加相机与约束、加灯、加动画曲线、设世界）。
3. 截图自检：渲染 viewport → 让多模态模型判断"画面是否符合镜头描述与风格"，输出可执行的调整项。
4. 修正并重试，有限次数（默认 3 次）；超限则保留当前最佳结果并写入 `directorNotes`，不无限烧 token。
5. 每个动作写 provenance，支持镜头级锁定：锁定镜头不被全局重规划覆盖。

> 与确定性的关系：这个 plan→act→自检 循环本身是随机的（LLM 驱动），因此它属于"规划阶段"。循环收敛后，把最终的操作序列**固化为 Build Plan（§7.3）**；此后渲染只重放 Build Plan，不再进入循环，从而保证渲染确定。换言之：**AI 自检循环只在"重新规划"时运行一次，日常渲染是确定性重放。**

与 blender-mcp 的关系：借鉴其"socket 双向 + viewport 截图 + 资产集成"，但强化稳定实体 ID、分层执行安全、许可与事实审核（Guaxi 差异化能力）。

媒体生成路线（可选，出电影级写实画面）：

```text
Director 搭好 3D 场景 → 导出该镜头 depth / normal / pose pass
        → 作为 ControlNet/IPAdapter 约束
        → 本地 ComfyUI 或云端图生图/图生视频
        → 生成写实画面 → 落入 VSE 视频频道 strip（保留 3D 约束的构图与一致性）
```

这样"3D 保证空间与一致性、AI 保证质感与写实"，是纯 3D 与纯 AI 生成之间可控性最好的折中。

### 7.3 确定性构建：Build Plan 冻结（默认路线，文本 LLM 驱动，不依赖生成式随机）

用户诉求：用在线文本模型（如 DeepSeek）指导 MCP 在本地真正搭建 3D 场景/模型再渲染，让**每次渲染结果确定**，而不是扩散模型每帧随机生成。这条路可行，并被定为**默认确定性路线**；但"确定性"必须靠工程实现，不是自动成立。

#### 7.3.1 确定性的真正来源：分离"搭建"与"渲染"

链条里有两段性质相反：

| 环节 | 性质 | 说明 |
|---|---|---|
| 文本 LLM → 搭建计划（build plan） | **随机** | 同一文稿两次调用会给出不同方案；即使 temperature=0 也不保证跨时间/跨版本一致 |
| 3D 场景 → 渲染视频 | **完全确定** | 固定 `.blend` 逐帧可复现，无 seed 随机；这正是相对扩散模型的优势 |

因此确定性来自**把 LLM 的产物冻结成工件、之后只重放渲染**：

```text
文稿 → LLM（DeepSeek 等）生成 Build Plan —— 仅一次
     → 冻结为 story/build_plan.json（+ 可选 scene_build.py），算哈希、纳入版本管理
     → 渲染 = 重放 Build Plan 构建同一 .blend → 逐帧完全一致
     → 仅当用户显式"重新规划"才再次调用 LLM
```

副产物优势：可复现、可审计、可手改、可 diff、可镜头级锁定，与 Guaxi"Story IR 可审计"哲学一致，也天然支持断点续作与局部重做。

#### 7.3.2 Build Plan 是什么

Build Plan 是介于 Story IR（叙事意图）与 Blender（执行）之间的**确定性场景指令集**：

- 内容：每个镜头用到的资产引用（Asset Resolver 解析出的稳定句柄）、对象 transform、相机与约束、灯光、动画关键帧、世界/雾/景深、渲染设置。
- 形式优先级：**结构化操作（Tier 1 参数化，安全可校验）> 冻结的模板化 Python（Tier 1）> 裸 Python（Tier 2，仅在必要时，冻结 + diff 审阅）**。
- 幂等：同一 Build Plan + 同一 Blender 补丁版 + 同一资产缓存 → 同一 `.blend` → 同一视频。
- 归属：写入 `story/build_plan.json`，与 `story_ir.json` 并列；哈希记入项目清单。

#### 7.3.3 真实模型来源（画质边界，需诚实设定预期）

文本 LLM **不生成几何**，只做"选择 + 装配 + 布景决策"。真实模型来自 Asset Resolver（§7.1）：

- **资产库（PolyHaven/Sketchfab）**：下载轻量、真实模型、缓存后确定 —— "省资源 + 真实 + 确定"的最佳落点。
- **程序化（Geometry Nodes）**：地形、河流、建筑体块、路线 —— 地理/历史环境主力，确定。
- **LLM 图元拼装**：便宜但只能到风格化/低多边形。

画质天花板：环境、地图、低多边形/风格化、纪录片式 3D 很能打；**写实人物特写是纯文本+3D 到不了的上限**，此类镜头才保留 §7.2 的生成式媒体路线作为可选升级。

#### 7.3.4 资源与定位

- 文本 token 成本 ≪ 扩散 GPU；渲染重放不再调 LLM，重渲染几乎零 AI 成本。
- 定位：**默认走确定性原生 3D（本节）；生成式媒体（§7.2 末）降为个别写实镜头的可选升级**，由制作模式与镜头级设置决定，不是全片默认。

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

### 10.1 分层执行安全模型（修订：兼容 AI 自主建模）

原则"禁止执行任何 AI 生成的 Python"与"让 AI 自主驱动 Blender 建模"直接冲突。修订为**分层执行模型**，在保留安全边界的同时允许受控的 AI 建模：

| 层级 | 能力 | 默认 | 约束 |
|---|---|---|---|
| Tier 0 允许列表 | set_frame / select / set_transform / set_world_color / import_asset / add_light / add_camera 等固定操作 | 开启 | 参数范围校验，不执行任意代码；AI Scene Director 首选此层 |
| Tier 1 参数化模板 | 仓库签入、经审计的 Geometry Nodes / Python 模板，只接受参数 | 开启 | 只能填参数，不能改模板代码；模板文件哈希校验 |
| Tier 2 AI 代码执行 | 执行 AI 生成的 Blender Python（blender-mcp 式 execute_code） | **默认关闭** | 显式逐会话授权；限定项目工作区；执行前向用户展示 diff/摘要确认；全量审计到 `logs/`；建议独立 Blender 进程隔离；可随时熔断 |

规则：

- 默认体验（Tier 0/1）完全不执行 AI 任意代码，安全等价于旧设计。
- 复杂建模确需 Tier 2 时，必须用户显式开启，且每次执行留痕、可回滚、可中断。
- Tier 2 代码不得触碰工作区外文件、网络与系统命令；由受控 runner 预处理拦截明显危险调用。
- 任何层级都不把 API Key、Bridge Token 暴露给被执行代码。

### 10.2 其它安全与可靠性约束

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

> 修订说明：为解决"只有立方体"，把原本排在 M3/M4 的"资产解析"与"AI 自主建模"提前，成为紧接 M0 的最高优先级（新 M1.5 / M2）。原分镜工作台（旧 M1）与之并行推进。

### M0：基础骨架（已完成）

- 独立窗口、任意 Markdown、任意后端模型。
- Story IR、标准项目、Blender 5.2 检测。
- 低模预演、后台任务、交互式插件和安全协议。

验收：一篇 Markdown 能生成 Story IR、`.blend` 和简单 MP4；任务可取消。

现状即"动态分镜验证器"，产物为灰盒 animatic。以下里程碑负责把它升级为真实场景与电影级画面。

### M1.5：确定性原生 3D（资产解析 + AI 场景导演 + Build Plan 冻结，新增，最高优先级）

直接消灭"只有立方体"，且保证渲染确定性（对应用户"文本 LLM 驱动 MCP 本地真实搭建、每次结果确定"的诉求）：

- 实现 Asset Resolver L1（本地库）+ L2（PolyHaven/Sketchfab），真正消费 `assetHints/assetPlan`。
- 打通 blender-mcp 式双向 socket / MCP：AI 可下发操作并回读 viewport 截图。
- AI Scene Director 最小闭环：导入资产 → 布景/机位/灯光 → 截图自检 → 修正。
- **Build Plan 冻结（§7.3）**：Scene Director 收敛后把操作序列固化为 `story/build_plan.json`（Tier 1 结构化操作优先），算哈希；渲染改为**重放 Build Plan**，不再调用 LLM。
- 分层执行安全 Tier 0/1 落地；Tier 2 作为默认关闭的高级开关。
- 灰盒构建器降级为兜底，仅在资产解析失败时占位并写 `assetGaps`。

验收：
1. 同一篇"鸿门宴"文稿，场景里出现的是营帐/人物/道具类真实资产而非立方体；缺失资产才用灰盒并明确标记。
2. **同一 `build_plan.json` 重复渲染两次，输出逐帧完全一致；只有显式"重新规划"才会再调用 LLM 并生成新版本 Build Plan。**

### M1：真正的分镜工作台

- Story IR JSON Schema 独立文件和前后端完整验证。
- 镜头卡拖拽、时长调整、锁定、局部重生成。
- 分镜图片、contact sheet、旁白草稿和 SRT。
- VSE Master Timeline，Strip 写入完整 Guaxi 元数据。

验收：修改单镜头不会重做整片，重新打开项目能恢复全部状态。

### M2：媒体生成后端（ComfyUI + 云端 API）与队列

对应用户选定的后端：本地 ComfyUI/GPU + 云端图像/视频 API。

- Provider registry、能力发现和模型列表。
- 本地 ComfyUI workflow 适配器 + 云端图生图/图生视频 API 适配器 + Mock。
- Asset Resolver L3（文/图生 3D：Hyper3D Rodin / Hunyuan3D / Tripo）。
- 3D 场景导出 depth/normal/pose pass → ControlNet 约束 → 生成写实画面。
- submit/poll/cancel/collect、并发限制、显存调度和失败重试。
- 图片/视频/音频自动放入对应 VSE 频道，保留 prompt/seed/模型/来源元数据。

验收：同一镜头可在"纯 3D 渲染"与"3D 约束 + AI 生成写实画面"之间切换；同一角色跨镜头保持一致；元数据和 Redo 均可用。

### M3：采集、资产、GIS 与历史制作（层 A、D、部分 E）

- 多源导入：PDF/EPUB/DOCX、网页、CBDB/维基数据、GeoJSON/SHP/GPX/KML、DEM/GeoTIFF、CSV（§15.1）。
- 实体链接、消歧、事实置信度、知识图谱与参考图板（§15.2）。
- Blender Asset Browser 索引与语义标签；glTF/USD/FBX/Blend 解析和单位统一。
- GeoJSON/DEM/路线动画与地图标注；历史年代、服饰、建筑规则包；引用与艺术想象标记。

验收：地理文稿能生成带比例尺的路线/地形镜头；历史项目能从多源史料输出带来源锚点的事实审核表。

### M4：角色一致性、声音与导演智能（层 C、G、E）

- 角色圣经与连续性状态机（§16）：外观锁定、服装状态、跨镜头/跨集一致、镜头级锁定。
- 声音栈（§17）：多角色 TTS 声线、口型、对齐、音乐、音效、环境声、混音与响度规范。
- 摄影与导演智能（§18）：镜头语法、覆盖率、镜头/景深、色彩脚本、节奏引擎。
- 镜头级人工精修与导演批注往返（§20）。

验收：3–5 分钟多角色短片可断点制作，同一角色跨镜头一致，局部修改不破坏相邻镜头，声画对齐且响度达标。

### M5：后期、QC 与渲染农场（层 H、I、L）

- VSE 装配、调色、字幕、标题、动态图形、地图标注（§19）。
- 自动 QC + 连续性检查 + 事实/敏感审核（§20）。
- 本机多 GPU/远端节点调度、资产打包、检查点恢复、成本追踪、模板库（§23）。

验收：整片能通过自动 QC 与连续性报告；渲染可断点续作并计量成本。

### M6：多版本、发布与合规运营（层 J、K、M）

面向"上传视频平台"的最后一公里：

- 智能多画幅重构图（16:9 / 9:16 / 1:1）与平台预设编码（§21.1、§21.2）。
- 封面/标题/标签/章节草案、多语言字幕与本地化、发布打包与排期（§21.3–§21.5）。
- 许可账本、肖像/声音授权、**AIGC 合规标识**、C2PA 内容凭证、模型使用记录（§22）。
- 发布后数据回收与逆向到剧本的迭代闭环（§24）。

验收：同一项目可靠导出横屏与竖屏版本，附事实/版权/模型使用清单与 AIGC 标识；能生成平台发布包并支持局部迭代再制作。

### 里程碑 ↔ 功能栈映射

| 里程碑 | 覆盖层 | 一句话目标 |
|---|---|---|
| M1.5 | B、C(基础)、D、E、F(可选) | 消灭立方体，确定性原生 3D |
| M1 | B | 分镜工作台与状态恢复 |
| M2 | F | 生成式媒体后端与队列 |
| M3 | A、D | 多源采集、资产、GIS、历史制作 |
| M4 | C、G、E | 角色一致性、声音、导演智能 |
| M5 | H、I、L | 后期、QC、渲染农场 |
| M6 | J、K、M | 发布、合规、运营闭环 |

## 13. 下一步优先级（修订：先消灭立方体）

聚焦"让画面变成真实场景"，按依赖顺序：

1. **定义 Build Plan schema（§7.3）。** 先确定确定性搭建计划的结构（资产引用、transform、相机、灯光、关键帧、渲染设置），它是"文本 LLM 驱动 + 确定性渲染"的地基。
2. **Blender 侧扩展 Tier 0 操作集。** 在现有允许列表上增加 `import_asset`、`add_camera`、`add_light`、`set_animation` 等确定性操作——它们既是 Build Plan 的执行原语，也供 AI Scene Director 调用。
3. **Asset Resolver L1/L2 落地。** 本地库匹配 + PolyHaven/Sketchfab 下载缓存，产出可导入句柄与 provenance，缓存后确定可复现。这是脱离立方体的第一步。
4. **打通双向通道。** 参考 blender-mcp，让 Bridge/MCP 支持回读场景状态与 viewport 截图，形成 agent 自检闭环。
5. **AI Scene Director + Build Plan 冻结。** 用 DeepSeek 等文本 modelid 逐镜头 plan→act→截图自检→修正；收敛后固化为 `build_plan.json`。**渲染改为重放 Build Plan，验证两次渲染逐帧一致。**
6. **接入 ComfyUI + 云端图像/视频 API（可选写实路线）。** 先 Mock 打通队列与落盘，再接真实 provider；实现 depth/pose 约束的图生图/图生视频，仅用于个别写实镜头。
7. **分层执行安全 Tier 0/1/2。** Tier 2 默认关闭、显式授权、diff 确认、工作区限定、全量审计。
8. 并行：Story IR 与 Build Plan 共享 JSON Schema 双端验证；TTS 接入对白/旁白轨道与 SRT。

> 与旧计划的差异：
> - 原第 5 条"评估 Pallaidium 适配器"降级为可选参考。
> - **主攻方向改为"确定性原生 3D：文本 LLM → Build Plan 冻结 → 确定性渲染"**；Asset Resolver + AI Scene Director 是其组成部分。
> - 生成式媒体（ComfyUI/云 API）从"默认成片手段"降为"个别写实镜头的可选升级"，直接对应用户"每次渲染确定、不靠 AI 随机生成"的目标。

---

# 第二部分：完整功能栈（能力目录）

> 面向用户的真实使用场景：**史书 / 地理数据叙事 / 小说故事电影 / 个人微电影 → 上传视频平台**。
> 本部分是"从文稿到发布再到复盘"的全生命周期能力目录；上面的里程碑（§12）是它的交付节奏。每一层都遵循前文原则：Guaxi 决策、Blender 执行、媒体模型补质感、稳定 ID 贯通、确定性优先、来源与许可可审计。

## 14. 全流程功能栈总览

```text
┌─ A 采集与研究 ── 多源导入、实体链接、事实置信度、参考图板
├─ B 故事与结构 ── Story IR、剧本、分镜、节拍、时长预算
├─ C 角色与一致性 ── 角色圣经、外观锁定、跨镜头/跨集连续性、声线绑定
├─ D 世界与资产 ── Asset Resolver、GIS、历史考据规则包、程序化
├─ E 摄影与导演 ── 镜头语法、覆盖率、镜头/景深、色彩脚本、Build Plan
├─ F 媒体生成 ── ComfyUI/云 API、3D 约束的图/视频、修复超分
├─ G 声音与配音 ── 多角色 TTS、口型、音乐、音效、环境声、混音响度
├─ H 剪辑与后期 ── VSE 装配、调色、字幕、标题、字幕、动态图形、地图标注
├─ I 质量与审核 ── 自动 QC、连续性、事实审核、导演批注往返
├─ J 多版本与发布 ── 智能重构图、平台预设、封面/标题/标签、编码打包
├─ K 版权与合规 ── 许可账本、肖像/声音授权、AIGC 标识、C2PA 内容凭证
├─ L 生产管理 ── 系列/分集、版本、渲染农场、成本、模板库
└─ M 发布后闭环 ── 平台数据回收、逆向到剧本、迭代再制作
```

三条纵向主线（跨层）：**稳定 ID 谱系** · **来源/许可/凭证 provenance** · **确定性可复现**。

## 15. A 采集与研究层

当前只支持 Markdown。史书与地理项目需要更宽的入口与更强的事实纪律。

### 15.1 多源导入

| 来源 | 用途 | 说明 |
|---|---|---|
| Markdown / TXT | 小说、讲稿 | 已支持 |
| PDF / EPUB / DOCX | 史书、论文、书稿 | 抽取正文 + 结构 + 脚注 |
| 网页 / 百科 | 背景资料 | 记录 URL、抓取时间、可信度 |
| 结构化史料库 | 人物、年代、职官 | 如 CBDB 中国历代人物传记数据库、维基数据 |
| GeoJSON / SHP / GPX / KML | 地理边界、路线 | 明确 CRS |
| DEM / GeoTIFF | 高程地形 | 生成三维地形 |
| CSV / 表格 | 年表、统计、路线点 | 时间轴与数据可视化 |
| 图片 / 参考包 | 服饰、建筑、肖像参考 | 进入角色/场景 moodboard，不直接作史实 |

### 15.2 研究纪律

- **实体链接与消歧**：同名人物、古今地名、异称官职归一到稳定 ID；跨章节、跨作品一致。
- **事实置信度**：每条 fact 记 `sourceAnchor` + `confidence`；不确定进入 `researchGaps`。
- **知识图谱**：人物关系、事件因果、时空坐标，供后续分镜与一致性检查引用。
- **参考图板（moodboard）**：为角色、建筑、器物收集参考图并标注来源与许可；仅作视觉参考，画面标识区分"考据/推断/想象"。
- **敏感与偏见审查入口**：战争、死亡、族群、宗教、真实在世人物的呈现在此登记，贯穿到发布前审核。

## 16. C 角色与一致性系统（小说/史书成片的核心）

跨镜头"同一个人长得不一样"是 AI 影像最大的翻车点。本系统保证一致性。

### 16.1 角色圣经（Character Bible）

每个角色一张锁定卡：

- 外观：正面 / 侧面 / 表情 / 全身，色板，身高体型。
- 服装状态集：不同场景/时段的服装及其**连续性状态**（干净/沾血/淋湿/破损/年龄变化）。
- 3D 资产绑定：Asset Resolver 解析出的稳定网格/材质句柄。
- AI 一致性锚：参考图集、seed、可选角色 LoRA / IP-Adapter 参考，用于生成式镜头。
- 声线绑定：TTS 音色 ID、语速、情绪基线（连到 §18）。
- 关系图与弧线：与其他角色的关系、情绪与立场变化。

### 16.2 连续性状态机（Continuity）

- 维度：服装、道具、伤痕、妆发、时间（昼夜）、天气、光线方向、人物站位与朝向。
- 规则：镜头切换时校验状态是否连贯；违反则告警（如上一镜白天下一镜深夜却无转场）。
- 跨集：系列作品中角色年龄、伤痕、剧情道具随集数推进，写入 `continuity` 域。

### 16.3 镜头级锁定

锁定的镜头与角色状态不被"全局重规划"覆盖；只有显式解锁才可变。与 Build Plan 冻结（§7.3）协同：一致性即"冻结 + 复用"。

## 17. G 声音与配音栈

当前音频仅一行占位。微电影的成败一半在声音。

| 子系统 | 能力 |
|---|---|
| 多角色配音 | 复用 Guaxi TTS，按角色分配音色、情绪、语速；旁白独立音色 |
| 对白时序与口型 | 对白对齐到镜头时间；3D 角色做基础口型，写实镜头交生成式口型 |
| 音乐 | 生成或授权曲库，按情绪弧线铺底；记录版权 |
| 音效 / 拟音 | 环境音、动作音、点睛音；分层到独立 VSE 频道 |
| 环境声场 | 场景氛围（营帐、雨夜、戈壁风），随场景切换 |
| 混音 | 对白优先、音乐闪避（ducking）、声像与动态处理 |
| 响度规范 | 按平台做响度归一（如 -14 LUFS 一类目标）与峰值限制，避免过响被平台压制 |
| 授权 | 声音克隆、真实人声需授权字段；配乐版权登记 |

字幕与配音同源：对白/旁白同时产出 SRT/ASS，供后期与本地化使用（连 §19、§21）。

## 18. E 摄影与导演智能

让 AI Scene Director（§7.2）"会拍"，而不仅"会摆"。

- **镜头语法规则包**：180° 轴线、越轴检查、正反打、视线匹配、动接动、景别递进。
- **覆盖率（coverage）**：一个叙事节拍生成多角度候选（大全/中/特/插入），供剪辑选择。
- **镜头与光学**：镜头焦段模拟、景深、运动模糊、机位运动曲线（推拉摇移升降环绕）。
- **色彩脚本**：按场景/情绪定义色调与对比，驱动灯光与后期调色一致。
- **节奏引擎**：按类型（史书克制、悬疑紧张、地理舒缓）规划镜头时长与切换密度。
- **分镜→预演→成片**三级递进，均可回到 Story IR 修改并重放。

## 19. H 剪辑与后期

- **VSE 装配**：所有媒体成为带元数据的 Strip（§8）；转场、J/L cut、匹配剪辑。
- **调色**：OCIO / AgX 色彩管理、LUT、逐镜头一级/二级调色，贴合色彩脚本。
- **字幕**：多语言字幕生成、烧录或软字幕、安全区校验、中文授权字体。
- **标题与图形**：片头、章节卡、下沿字幕条、演职员表、知识讲解的动态图形。
- **地图标注（地理专用）**：指北针、比例尺、时间滑块、地名标签避让、路线高亮。
- **可逆**：任何 Strip 可 Redo / 替换 / 反向重建剧本（连 §M）。

## 20. I 质量控制与审核

发布前的自动 + 人工双闸。

- **自动 QC**：黑帧、掉帧、异常闪烁、音频削波、响度越界、画幅不符、字幕越安全区、时长超限。
- **连续性检查**：调用 §16.2 状态机做整片扫描并出报告。
- **事实审核（史书/地理）**：导出"事实—来源锚点—置信度—是否标注"清单；地图坐标、投影、比例尺核对。
- **敏感内容复核**：war/death/族群/宗教/真人呈现按 §15.2 登记项逐条确认。
- **导演批注往返**：镜头级批注 → 局部重规划 → 重放，不动已锁定镜头。

## 21. J/多版本、本地化与发布分发

"上传视频平台"落到实处的一层。

### 21.1 智能多画幅

- 16:9 / 9:16 / 1:1 自动重构图（跟随主体的再构图，而非硬裁）。
- 每个画幅独立校验构图、字幕位置与安全区。

### 21.2 平台预设

| 平台 | 主画幅 | 关注项 |
|---|---|---|
| 哔哩哔哩 / YouTube / 西瓜 | 横屏 16:9（1080p/4K） | 封面、章节、标签、时长 |
| 抖音 / 快手 / 视频号 / 小红书 | 竖屏 9:16（1080×1920） | 前 3 秒抓人、字幕大字、时长上限 |

每个预设内置：分辨率、码率、编码（H.264/H.265/AV1）、容器、faststart、封面规格、字幕格式、时长与文件大小限制。

### 21.3 元数据与素材生成

- 封面/缩略图生成（可出 A/B 版本）。
- 标题、简介、标签、章节、话题的草案（SEO 友好，人工确认）。
- 平台字幕/CC 上传格式。

### 21.4 本地化

- 多语言字幕翻译、可选配音、画面文字替换；文化适配提示。

### 21.5 发布打包

- 一键导出"横屏 + 竖屏 + 字幕 + 封面 + 元数据 + 制作清单"的平台包；支持批量与排期。

## 22. K 版权、合规与内容凭证

- **许可账本**：模型、资产、贴图、字体、音乐、地图数据的来源/作者/许可证/允许用途/下载时间。
- **肖像与声音**：真实人物形象、声音克隆需授权字段，缺失则禁止进入发布交付。
- **AIGC 合规标识（重要）**：中国主流平台与法规（生成式 AI/深度合成相关规定）要求对 AI 生成/合成内容进行显著标识；发布包自动附带"AI 生成"标注与说明，纳入发布前检查项。
- **内容凭证 C2PA**：为成片写入内容来源与制作链凭证（可选），增强可信度与溯源。
- **模型使用记录**：模型、版本、seed、prompt、输入素材哈希，保证可复现与审计。

## 23. L 项目、系列与生产管理

支撑"一人工作室"长期、批量、可维护地产出。

- **层级**：作品 → 卷/季 → 章/集 → 场景 → 镜头 → 任务；跨集连续性与资产复用。
- **版本与检查点**：Story IR / Build Plan / `.blend` / 渲染的版本管理、分支（做横竖屏/多语言变体）、回滚。
- **渲染农场与队列**：本机多 GPU、后续远端节点；单镜头任务、幂等 job ID、断点续作。
- **成本追踪**：LLM token、云 API 调用、GPU 时长按项目/镜头计量，预算与告警。
- **模板与预设库**：按类型（史书/地理/小说/知识）沉淀镜头、色彩、字幕、发布预设，一键复用。
- **资产库管理**：跨项目资产标签、检索、去重与许可归档。

## 24. M 发布后闭环

- **数据回收**：平台播放、完播率、留存等指标回流（手动或接口）。
- **逆向工作流**：从成片/字幕反向重建剧本与分镜，便于修订与二创。
- **迭代再制作**：基于反馈局部重规划特定镜头/片段，重放渲染，不重做整片。
