# 知识图谱(kgraph)子系统 · 设计文档

> 适用版本:分支 `knowgraph`。本文档与代码一一对应,所有命令、表、字段均可在源码中检索到。

## 1. 背景与目标

原有知识库的 3D 图谱直接由文件目录树驱动、每次现场遍历磁盘 + 重解析 Markdown 生成,存在四个问题:

1. **看不出关系** —— 布局只反映目录层级,真正的知识关联(双链、引用、语义相近)没有进入图。
2. **布局混乱、性能低** —— 3D 力导向每次重算,坐标不稳定,节点一多就糊。
3. **无法切换层次** —— 库/目录/文件/章节/段落一锅端。
4. **效率低** —— 关系靠现场解析或人工,无法扩展到十万级节点。

kgraph 子系统重建了一套分层架构来解决这些问题:

```
.md 文件(真相源)
  └─ .meta JSON(可移植元数据,原有机制保留)
       └─ repos/kg_index.db(SQLite 固化索引)  ← 本子系统核心
            ├─ petgraph 内存图(中心度 / 最短路 / 社区)
            ├─ 向量库(语义检索 / 关系推荐 / 零样本打标签)
            └─ kg_layout 缓存坐标(秒开 + 稳定转场)
                 └─ 前端 InstancedMesh 3D + 导航工具
```

设计原则:**真相源与派生索引分离**。`.md` 与 `.meta` 是可移植真相;`kg_index.db` 是可随时重建的高速派生层。文件变更只把索引标脏,查询时惰性重建(materialized-view 模式),保存零阻塞。

---

## 2. 代码地图

| 层 | 文件 | 职责 |
|---|---|---|
| 索引核心 | `src-tauri/src/commands/kgraph.rs` | schema、关系抽取、索引构建、petgraph 内存图、PageRank、向量库、Louvain 社区、布局存取与**后端力导向布局**、标签/关系增删、全部 `kg_*` 查询命令 |
| 向量服务 | `src-tauri/src/commands/embedding.rs` | 复用 llama-server 起 embedding 实例、双模型注册、缺模型自动下载、`embed_texts`(物理批=ctx,超长输入截断) |
| LLM 抽取 | `src-tauri/src/commands/kg_llm.rs` | 概念/关系抽取命令,经 `chatview::kg_llm_complete` 调模型 |
| 模型后端 | `src-tauri/src/commands/chatview.rs` | `kg_llm_complete`(本地优先 + 外部 API 预留 + 失败回退) |
| 增量钩子 | `src-tauri/src/commands/kbmeta.rs` | 写/建/删/移文件时 `mark_kb_dirty` / `drop_kb_index`;`scan_markdown_structure` 解析知识点结构时**剥离 `<!--py/math/pyout-->` 哨兵块** |
| 命令注册 | `src-tauri/src/main.rs` | `generate_handler!` 注册所有命令 |
| 前端数据层 | `src/stores/knowledgeBaseStore.js` | `kg*` 动作封装 + `fetchGraphData` 优先走索引 + kg3d 通信总线(选中/动作请求/历史/书签) |
| 前端布局 | `src/components/BrainViewComponent/kgLayout.js` | 四视角布点 + 坐标固化;relation 视角**三级回退**:Rust 后端 → Web Worker → 主线程同步 |
| 布局 Worker | `src/workers/kgForceLayoutWorker.js` | d3-force-3d 力导向移出主线程,失败自动回退 |
| 前端渲染 | `src/components/BrainViewComponent/GuaxiKnowledgeGraph3D.vue` | InstancedMesh 分层、社区着色、导航工具、多视角、小地图、时间轴、亮/暗色、hover/发光高亮、飞行模式、**按需渲染 + 隐藏暂停** |
| kg3d 右栏 | `src/components/BrainViewComponent/GuaxiKg3dOpsPanel.vue` | 工业图谱模式下的右侧操作面板(节点信息/标签增删/关系增删/视角动作) |
| kg3d 左栏 | `src/components/BrainViewComponent/GuaxiKg3dLeftPanel.vue` | 概览统计 / 社区分类导航 / 历史与书签 |
| 右栏宿主 | `src/components/BrainViewComponent/GuaxiRightContentBrainView.vue` | 按 `graphViewMode` 分发:经典→原面板,工业→kg3d 操作面板 |
| 左栏宿主 | `src/components/BrainViewComponent/GuaxiSidbarBrainContent.vue` / `GuaxiBrainKnowledgeBaseList.vue` | 知识库列表 + 文件树(过滤 `.meta/.cellmeta`,点击定位图中节点)+ kg3d 左栏 |
| 视图宿主 | `GuaxiCenterBrainView.vue` / `GuaxiHeaderBrainView.vue` | Header 切换经典/工业图谱(默认工业),宿主挂载 |
| 知识库管理器 | `src/components/HomeViewComponent/GuaxiKnowledgeBaseManager.vue` / `GuaxiKnowledgeBaseGraphEditor.vue` | 元数据面板 + 图形编辑模式(标签/关联可视编辑,与图谱属性对应) |

---

## 3. 数据模型(`kg_index.db`)

`init_kg_schema()`(kgraph.rs)建立以下表,均以 `kb_name` 区分多知识库。

### kg_nodes —— 节点
| 字段 | 说明 |
|---|---|
| `node_id` | 主键。`kb_root` / `dir_<sha>` / `file_<sha>` / `section_<stableId>` / `para_<stableId>` / `concept_<sha(name)>` |
| `node_type` | `knowledge-base` / `directory` / `file` / `section` / `paragraph` / `concept` |
| `name`,`rel_path`,`abs_path`,`file_path` | 展示名与路径 |
| `stable_id` | 章节/段落跨会话稳定标识(SHA256,与 kbmeta 一致) |
| `parent_id`,`depth` | 层级 |
| `start_line`,`end_line`,`level` | 章节/段落定位 |
| `content_hash` | 内容指纹,embedding 失效判定 |
| `degree`,`pagerank`,`community_id` | 派生指标 |
| `created_at`,`modified_at` | 时间(供时间视角) |

### kg_edges —— 有类型有方向的边
| 字段 | 说明 |
|---|---|
| `from_id`,`to_id`,`edge_type` | `contains`/`next`/`link`/`mention`/`related`/`prerequisite`/`elaborates`/`contradicts`/`part-of`/`similar` |
| `strength` | 权重(影响布局与社区) |
| `source` | `structural`/`wikilink`/`mdlink`/`tag`/`semantic`/`llm`/`manual` |
| `src_file` | 产生该边的源文件(增量清理用) |

### 其余表
- **kg_node_tags**`(node_id,tag,source,score)` —— 标签维度,source ∈ `manual`/`inline`/`zeroshot`/`llm`。
- **kg_embeddings**`(node_id,content_hash,dim,vec BLOB)` —— f32 小端向量。
- **kg_layout**`(node_id,view,x,y,z)` —— 各视角缓存坐标。
- **kg_communities**`(community_id,level,label,size,centroid_node,color)` —— Louvain 结果。
- **kg_build_state**`(kb_name,schema_version,built_at,node/edge/embedded_count,embedding_model/dim,layout_version,community_level,dirty)` —— 构建状态。
- **kg_settings**`(key,value)` —— 全局设置(如 `embedding_model`)。

**重建保留策略**(`build_into_conn`):全量重建会清空结构性数据,但**保留** concept 节点、`source∈(semantic/llm/manual)` 的边、`source∈(zeroshot/llm)` 的标签 —— 仅当端点仍存在时回填,避免悬挂。这样 AI 产出与用户确认不会因重扫文件丢失。

---

## 4. 关系生产管线

四个来源,从确定性到智能,在 `build_kb_index` / `kg_embed_kb` / `kg_extract_concepts` 中分别产生:

| 级别 | 来源 | 代码 | 产物 |
|---|---|---|---|
| 1 显式 | 结构 + `[[wikilink]]` + `[](x.md)` + `#tag` | `extract_refs` / `resolve_refs_to_edges` | `contains`/`next`/`link` 边、tag |
| 2 语义 | 本地 embedding 余弦相似 | `kg_suggest_relations` | 候选 `related` 边(用户确认后固化) |
| 3 分类 | 标签原型向量零样本匹配 | `kg_suggest_tags` | 候选标签(确认后 `zeroshot`) |
| 4 抽取 | LLM 实体/关系抽取 | `kg_extract_concepts` | concept 节点 + `mention` + typed 关系边 |

**链接解析**(`resolve_refs_to_edges`):`[[名称]]` 先按相对路径(含/不含扩展名)再按文件名(stem,小写)解析到文件节点;带 `#锚点` 再解析到目标文件内同名标题的 section 节点;`from` 端优先归到引用所在 section,否则文件。

**人工增删**:用户可在图谱中手动建立关系(`kg_confirm_relation`,source=`manual`)、删除关系(`kg_remove_relation`,结构性 `contains`/`next` 边受保护)、添加标签(`kg_add_tag`,source 可为 `manual`)、删除标签(`kg_remove_tag`)。AI 产出与人工确认在全量重建时按"重建保留策略"保留。

---

## 5. 内存图与算法

`load_mem_graph(kb)`(带缓存,按 `built_at` 失效)把 SQLite 图载入 petgraph 无向图:

- **PageRank**`compute_pagerank` —— 20 轮阻尼 0.85,作为"重要度"。
- **最短路**`kg_shortest_path` —— 无权 BFS,回溯路径节点与边。
- **N 跳邻居**`kg_node_neighbors` —— BFS 诱导子图(ego 网络)。
- **社区检测**`louvain` —— 多层模块度优化,结果写 `community_id` + `kg_communities`,配黄金角散布的区分色。

**向量检索**`vec_search` —— 向量 L2 归一化后点积≈余弦,rayon 并行暴力 top-k。10 万级单查询亚毫秒,无需 ANN 索引;若未来全库 all-pairs 推荐成瓶颈可换 HNSW(向量库 `VecStore` 已为此预留结构)。

---

## 6. Embedding 服务

`embedding.rs` 复用项目已有的 `utils/llama/llama-server`,**另起一个实例**(端口 18081,`--embedding --pooling mean`),与聊天用的 deepseek 实例(18080)互不干扰。

- **双模型可切换**:默认 `bge-m3`(1024 维,中英文 + 专业术语质量最强,~634MB),可选 `multilingual-e5-small`(384 维,轻量 ~131MB,集成显卡更快)。`kg_set_embedding_model` 写入 `kg_settings`。
- **缺模型自动下载**:`ensure_model_present` 在模型文件不存在时自动从 HuggingFace 下载到 `utils/Models/embedding/`,原子落盘,无需用户手动安装。
- **前缀**:e5 系列对 query/passage 加前缀(`embed_texts(is_query)`),bge-m3 不需要。
- **物理批对齐 ctx**:embedding 模型是非因果注意力,**整条输入序列必须落在单个物理批(ubatch)内**。启动 llama-server 时 `--batch-size`/`--ubatch-size` 都设为 `8192`(与 `--ctx-size` 对齐),否则超过默认 512 token 的段落会报 `500 input is too large ... increase the physical batch size`。`embed_texts` 另对极端超长段落(>6000 字)按字符截断兜底。

> 运行时依赖:需要机器上有 `llama-server` 二进制(项目已打包)并能启动 embedding 实例。首次会自动下载模型(默认 bge-m3 约 634MB;可切换为 e5-small 约 131MB)。

---

## 7. LLM 后端抽象

`chatview::kg_llm_complete(modelid, prompt, sessionid)` 是统一的补全入口,体现"本地优先 + 外部 API 预留 + 失败回退":

- `modelid` 路由到既有 8 个后端(OpenAI/DeepSeek/Qwen/Kimi/豆包/SiliconFlow/本地 DeepSeek)。
- 远程/付费后端失败时自动回退本地 DeepSeek,保证可用性。
- `kg_extract_concepts` 默认用 `modelid=6`(本地),用户可传其它以接入外部 API。

---

## 8. 命令接口(Tauri)

前端经 `@tauri-apps/api` 调用;参数 JS 用 camelCase,Rust 收 snake_case。`knowledgeBaseStore.js` 已封装为 `kg*` 动作。

| 命令 | 入参 | 返回 |
|---|---|---|
| `kg_index_status` | kbName | KgIndexStatus(是否过期、计数等) |
| `kg_build_index` | kbName | {node_count,edge_count} |
| `kg_query_graph` | kbName, opts{root_id,max_depth,node_types,tags,edge_types,search,limit} | {nodes,edges,total_nodes,returned_nodes,truncated} |
| `kg_node_neighbors` | kbName,nodeId,hops,limit,edgeTypes | 子图 |
| `kg_shortest_path` | kbName,fromId,toId | {found,nodes,edges,hops} |
| `kg_stats` | kbName,topN | 类型/边/标签分布 + 核心节点 |
| `kg_detect_communities` | kbName,resolution | {communities,nodes} |
| `kg_communities` | kbName | [{community_id,label,size,color,...}] |
| `kg_embed_kb` | kbName | {embedded,skipped,model_key,dim} |
| `kg_semantic_search` | kbName,query,topK,nodeTypes | [节点+score] |
| `kg_suggest_relations` | kbName,nodeId,topK,minScore | [{from,to,to_name,to_type,score}] |
| `kg_suggest_tags` | kbName,nodeId,candidateTags,topK,minScore | [{tag,score}] |
| `kg_confirm_relation` | kbName,fromId,toId,relation,strength | — |
| `kg_add_tag` | kbName,nodeId,tag,source,score | — |
| `kg_remove_tag` | kbName,nodeId,tag | — |
| `kg_remove_relation` | kbName,fromId,toId,relation? | 删除条数(结构性 `contains`/`next` 边受保护不可删) |
| `kg_extract_concepts` | kbName,relPath,modelid,maxFiles | {files_processed,concepts,relations,failures} |
| `kg_save_layout`/`kg_load_layout` | kbName,view[,points] | — / [坐标] |
| `kg_compute_layout` | kbName,view,force | [坐标] —— 后端 rayon 力导向(relation 视角),写入缓存 |
| `kg_set_embedding_model` | modelKey | — |
| `kg_ensure_embedding_service`/`kg_embedding_status` | — | 服务状态 |
| `kg_download_embedding_model` | modelKey? | 下载结果 |

---

## 9. 前端渲染(`GuaxiKnowledgeGraph3D.vue`)

- **InstancedMesh 按类型分组**:每种节点类型一个实例网格(各自材质/颜色),`instanceColor` 用社区色或类型色 —— 个位数 draw call 撑大规模。
- **工具条图标化**:操作按钮用图标 + 通俗中文 tooltip(自由漫游 / 找两点之间的路 / 按主题收拢 / 自动分组上色 / AI 提炼概念 / 重新扫描),避免抽象术语。
- **分层 LOD**:`kg_query_graph` 用 `root_id + max_depth + limit` 控制返回规模;双击目录下钻(面包屑回退);"按主题收拢"按社区聚合为超级节点,双击超级节点展开该社区。**收拢/概念建议等操作若缺前置(未分组、未生成向量)会自动先跑前置步骤**,不需用户二次操作。
- **多视角**:relation(力导向)/ tag(按标签或社区聚簇)/ time(按修改时间沿轴)/ importance(PageRank 向心)。切换走 `kgLayout.resolveLayout(force)` 重算坐标 + GSAP 平滑 morph + 相机重取景。
- **缓存坐标 + 增量补点**:布局结果存 `kg_layout`,下次秒载、稳定可复现。缓存只覆盖当时存在的节点,因此 `kg_compute_layout(force=false)` 不能直接把缓存原样返回 —— 新增文件的 `file`/`section` 节点会缺坐标,渲染层回退到 `(0,0,0)`,新节点全部重叠堆在原点、被大球吞掉、连边被整条跳过,表现为"新建的文件在 3D 图里根本看不到"。正确策略:缓存全覆盖→直接返回;只差少量节点→`seed_missing_layout` 让新节点贴着已定位的邻居(父目录/所属文件)确定性落位并写回缓存,已有布局一动不动;缺口超过 `max(64, n/5)`(批量导入)→ 才全量重算。前端 `kgLayout.js` 的 `seedMissingPositions` 是同策略的跨层兜底,任何路径都不允许节点无坐标。
- **强高亮 + 发光**:选中/路径/社区/邻居/关联预览时,强调节点放大提亮、其余压暗缩小,叠加青色发光边(`highlightEdges`)与加色混合的呼吸光晕 sprite(`glowGroup`),并把相机取景拉近到强调集包围盒,远距也看得清。
- **过滤**:顶部类型勾选 + 标签下拉多选过滤;隐藏节点缩到 0 且其连边同步收起(端点不可见的边不画)。
- **导航工具**:搜索飞行定位(语义检索优先,回退名称)、点击 ego 聚光、两点最短路高亮、类型/标签过滤、小地图(可点击定位)、时间轴回放、文件树点击在图中定位高亮、收藏/历史快捷跳转。
- **飞行模式**:PointerLock 第一人称(WASD 平移 + 鼠标转向 + Q/E 升降 + Shift 冲刺,Esc 退出),与 OrbitControls 切换式共存。
- **打开节点**:文件/章节/段落/概念节点可在主编辑器打开并跳转到对应行(目录/库节点无此项)。
- **知识演化回放**:按节点 `_evoFrac`(有时间戳用时间、否则按 depth→重要度的结构顺序)逐步显现,新生节点 pop 闪光、边随端点出现,避免"一直空白到最后突然全冒出"。
- **亮/暗色**:读取 `--guaxi-content-bg` 亮度判定主题,设置场景/边色,并对覆盖层加 `.kg3d-light` 类切换配色;`MutationObserver` 监听根节点变化实时跟随。

### 9.1 左右边栏(工业图谱模式)

- **右栏** `GuaxiKg3dOpsPanel.vue`:节点信息、标签增删、关系加载/推荐/删除、点关联名预览高亮、视角与社区/向量/刷新动作。通过 store 通信总线(`kg3dSelected` / `requestKg3dAction` / `kg3dRelations` / `kg3dSuggestions`)与 3D 组件双向联动。
- **左栏** `GuaxiKg3dLeftPanel.vue`:概览统计(节点/边数、类型分布条、PageRank Top 核心节点)、社区/分类导航(点击聚焦社区、标签云)、历史与书签(最近访问 + 收藏,点击定位)。
- **经典视图**仍用原右栏面板;`GuaxiRightContentBrainView.vue` 按 `graphViewMode` 分发。

---

## 10. 性能架构(切页不卡 / 后台不阻塞 UI)

UI 流畅是硬性约束,核心策略:**重计算下沉后台、隐藏即停、按需渲染**。

- **隐藏即停渲染**:页面在 `<KeepAlive>` 下不会卸载,`onActivated/onDeactivated` + `visibilitychange` 控制 `startLoop/stopLoop`,切到别的页面或最小化时 RAF 循环、小地图、光晕脉冲全部暂停,后台零空耗。
- **按需 / 降帧渲染**:静止时跳过昂贵的 `renderer.render`,仅在交互(OrbitControls `change`)、GSAP 活跃动画(全局 ticker 探测)、飞行、光晕脉冲、时间轴播放时满帧;所有场景突变点统一 `requestRender()` 标脏。
- **力导向三级回退**(`kgLayout.js`):relation 视角最重的布局优先走 **后端 Rust `kg_compute_layout`**(rayon 并行斥力的 3D Fruchterman-Reingold,确定性初始布点、规模自适应迭代、直接写 `kg_layout` 缓存)→ 失败回退 **Web Worker**(`kgForceLayoutWorker.js`,d3-force-3d 不阻塞主线程)→ 再回退主线程同步(`computeForceLayoutSync` 兜底)。任一层失败自动降级,零功能损失。
- **挂载错峰**:`onMounted` 先 `initThree` 建轻量场景,等一帧让路由切换先出框架,再异步 `loadGraph` 拉数据 + 布局。
- **GPU 友好**:`WebGLRenderer` 设 `powerPreference:"high-performance"`、`stencil:false`,抗锯齿按 DPR 自适应(高分屏关 AA 省带宽)。
- **后端命令本就异步**:`kg_embed_kb`/`kg_extract_concepts`/`kg_compute_layout` 均 `async tauri::command`,聊天走 `get_chat_answer_stream` 流式,AI 处理期间前端可继续交互。

### 10.1 索引变更广播(视图自动跟随)

索引惰性重建解决了"保存不卡",但前端各视图此前只在挂载/切库/手动刷新时查询,主界面新建文件后管理器与图谱看不到新节点。为此加一条**只通知、不计算**的广播链路:

- **发端**:`mark_kb_dirty` / `drop_kb_index`(kgraph.rs)是全部文件 CRUD 钩子的汇聚点,标脏后 `emit("kg_index_changed", { kb_name, reason })`(`reason` = `dirty` / `removed`)。全局 `AppHandle` 由 `main.rs` 的 `setup` 通过 `kgraph::init_app_handle` 注入,事件天然广播到所有窗口(含独立的知识库管理器窗口)。事件不触发任何重建,重建仍由查询侧 `build_if_needed` 惰性完成。
- **收端**:`src/utils/kgIndexEvents.js` 的 `onKgIndexChanged(handler, { debounceMs })` 按知识库名防抖(自动保存会连发脏标记),四处订阅:
  - `GuaxiKnowledgeBaseManager.vue` —— 命中当前库重查图谱(选中节点自动复位),库被删/新库出现则刷新知识库列表。
  - `GuaxiBrainKnowledgeBaseList.vue` —— 静默重拉文件树,`applyLoadedTree(..., { preserveState: true })` 保留展开态与选中项。
  - `GuaxiKnowledgeGraph3D.vue` —— 可见时按当前面包屑层级重载并恢复选中;漫游/选路径/演化回放/社区折叠/页面不可见时只记 `pendingReload`,退出该状态或 `onActivated` 时补一次。自动重载走 `loadGraph(..., { ensureIndex: false })`,跳过强制全量 `kg_build_index`,只靠 `build_if_needed` 在真脏时重建一次。
  - `GuaxiKg3dLeftPanel.vue` —— 失效概览统计与社区缓存,已展开的重新拉取。

> 另注:管理器查询必须显式传 `{ max_depth: 99, limit: 100000 }`。`kg_query_graph` 默认 `max_depth=2`,否则二级目录以下的文件节点根本不会返回。

---

## 11. 知识库管理器对齐(`GuaxiKnowledgeBaseManager.vue` / `GuaxiKnowledgeBaseGraphEditor.vue`)

- **元数据面板**与图谱属性对应:文件/段落的标签、关联节点、结构节点分类可视编辑。
- **图形编辑模式**的节点信息卡不止显示内容,也展示并可增删标签 / 关联(写回 `*_meta.json`),与知识图谱的标签/关系语义一致。
- **不解析哨兵块**:知识点结构解析(`scan_markdown_structure`)跳过 `<!--py/math/pyout-->` 这类为 Notebook 等模式提供的可执行特殊语法,知识库管理时不再把它们当作结构节点。

---

## 12. 已知取舍与后续

- **ANN**:当前用 rayon 暴力余弦(单查询足够快);全库 all-pairs 推荐若成瓶颈再上 HNSW。
- **后端布局斥力**:`kg_compute_layout` 当前为 O(n²) 斥力(rayon 并行,数千节点可接受);超大规模可换 Barnes-Hut 八叉树近似。
- **标签渲染**:用 CSS2D + LOD(限可见+重要≤60 个,强调节点常显),非 SDF sprite —— 已能约束 DOM 数量;超大规模可再换 GPU sprite 图集。
- **社区折叠**:当前为单层折叠;多层社区(分级折叠)可在 `kg_communities` 的 `level` 字段上扩展。
- **非 relation 视角布局**:radial/time/cluster 仍在前端算(O(n) 轻量);如需也可并入 `kg_compute_layout` 的 `view` 分支下沉后端。
