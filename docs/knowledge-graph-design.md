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
| 索引核心 | `src-tauri/src/commands/kgraph.rs` | schema、关系抽取、索引构建、petgraph 内存图、PageRank、向量库、Louvain 社区、布局存取、全部 `kg_*` 查询命令 |
| 向量服务 | `src-tauri/src/commands/embedding.rs` | 复用 llama-server 起 embedding 实例、双模型注册、缺模型自动下载、`embed_texts` |
| LLM 抽取 | `src-tauri/src/commands/kg_llm.rs` | 概念/关系抽取命令,经 `chatview::kg_llm_complete` 调模型 |
| 模型后端 | `src-tauri/src/commands/chatview.rs` | `kg_llm_complete`(本地优先 + 外部 API 预留 + 失败回退) |
| 增量钩子 | `src-tauri/src/commands/kbmeta.rs` | 写/建/删/移文件时 `mark_kb_dirty` / `drop_kb_index` |
| 命令注册 | `src-tauri/src/main.rs` | `generate_handler!` 注册所有命令 |
| 前端数据层 | `src/stores/knowledgeBaseStore.js` | `kg*` 动作封装 + `fetchGraphData` 优先走索引 |
| 前端布局 | `src/components/BrainViewComponent/kgLayout.js` | d3-force-3d + 四视角布点 + 坐标固化 |
| 前端渲染 | `src/components/BrainViewComponent/GuaxiKnowledgeGraph3D.vue` | InstancedMesh 分层、社区着色、导航工具、多视角、小地图、时间轴、亮/暗色 |
| 视图宿主 | `GuaxiCenterBrainView.vue` / `GuaxiHeaderBrainView.vue` | Header 切换经典/工业图谱(默认工业),宿主挂载 |

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

- **双模型可切换**:默认 `multilingual-e5-small`(384 维,中英文,集成显卡可跑),可选 `bge-m3`(1024 维,术语质量更强)。`kg_set_embedding_model` 写入 `kg_settings`。
- **缺模型自动下载**:`ensure_model_present` 在模型文件不存在时自动从 HuggingFace 下载到 `utils/Models/embedding/`,原子落盘,无需用户手动安装。
- **前缀**:e5 系列对 query/passage 加前缀(`embed_texts(is_query)`),bge-m3 不需要。

> 运行时依赖:需要机器上有 `llama-server` 二进制(项目已打包)并能启动 embedding 实例。首次会下载约 120MB(e5-small)模型。

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
| `kg_extract_concepts` | kbName,relPath,modelid,maxFiles | {files_processed,concepts,relations,failures} |
| `kg_save_layout`/`kg_load_layout` | kbName,view[,points] | — / [坐标] |
| `kg_set_embedding_model` | modelKey | — |
| `kg_ensure_embedding_service`/`kg_embedding_status` | — | 服务状态 |
| `kg_download_embedding_model` | modelKey? | 下载结果 |

---

## 9. 前端渲染(`GuaxiKnowledgeGraph3D.vue`)

- **InstancedMesh 按类型分组**:每种节点类型一个实例网格(各自材质/颜色),`instanceColor` 用社区色或类型色 —— 个位数 draw call 撑大规模。
- **分层 LOD**:`kg_query_graph` 用 `root_id + max_depth + limit` 控制返回规模;双击目录下钻(面包屑回退);"折叠"按社区聚合为超级节点,双击超级节点展开该社区。
- **多视角**:relation(力导向)/ tag(按标签或社区聚簇)/ time(按修改时间沿轴)/ importance(PageRank 向心)。切换走 `kgLayout.resolveLayout(force)` 重算坐标 + GSAP 平滑 morph + 相机重取景。
- **缓存坐标**:布局结果存 `kg_layout`,下次秒载、稳定可复现。
- **导航工具**:搜索飞行定位(语义检索优先,回退名称)、点击 ego 聚光、两点最短路高亮、类型过滤、小地图(可点击定位)、时间轴回放。
- **亮/暗色**:读取 `--guaxi-content-bg` 亮度判定主题,设置场景/边色,并对覆盖层加 `.kg3d-light` 类切换配色;`MutationObserver` 监听根节点变化实时跟随。

---

## 10. 已知取舍与后续

- **ANN**:当前用 rayon 暴力余弦(单查询足够快);全库 all-pairs 推荐若成瓶颈再上 HNSW。
- **标签渲染**:用 CSS2D + LOD(限可见+重要≤60 个),非 SDF sprite —— 已能约束 DOM 数量;超大规模可再换 GPU sprite 图集。
- **社区折叠**:当前为单层折叠;多层社区(分级折叠)可在 `kg_communities` 的 `level` 字段上扩展。
