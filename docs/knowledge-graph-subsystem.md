---
name: knowledge-graph-subsystem
description: Guaxi 知识图谱(kgraph)子系统——已成为唯一知识管理体系(固化索引+语义+概念+社区+3D),含可移植人工层与多机同步
metadata: 
  node_type: memory
  type: project
  originSessionId: a8dd24e5-9ee1-486c-858d-4510e600a2cc
---

为 Guaxi 知识库新建的"知识图谱"子系统(2026-06,分支 knowgraph),解决原 3D 图"看不出关系、布局乱、性能低、无法切层次"的问题。**2026-06 已完成统一重构:kgraph 成为唯一知识管理体系,旧的 `.meta`/kbmeta 侧车体系与经典图谱全部废弃。**

**架构分层(现状)**:`.md`(内容真相,走 p2p 文件同步)→ `repos/kg_index.db`(SQLite 固化索引,本机派生,**永不同步**)→ petgraph 内存图 + 向量库(工作记忆)→ `kg_layout` 缓存坐标 → 前端 3D。kgraph **自己解析 `.md` 结构**(`parse_structure`,与旧 stable_id 公式一致),不再依赖任何 `.meta`。

**kg_index.db 分层**:① 结构层(file/dir/section/paragraph + contains/next/link/inline-tag):本机从 `.md` 重建;② 人工/LLM 层(source∈manual/llm/semantic 的边、source∈manual/zeroshot/llm 的标签、concept 概念节点):**唯一需跨机同步的知识**,重建时被 `preserved_*` 保留;③ 派生层(社区/布局/向量):本机重算,不同步。所有节点 id 跨机一致:file/dir=`<pfx>_<sha256(rel)>`、section/para=`section_/para_<stable_id内容哈希>`、concept=`concept_<sha256(小写名)>`。

**人工层可移植与多机同步(2026-06 新增)**:
- 每库一个 `.manual.kgraph.json`(点前缀→文件树隐藏+图构建跳过,但 p2p 仍同步)。`kgraph.rs`:`export_manual_bundle/import_manual_bundle`(整层替换,带删除)/`merge_manual_bundle`(行级 INSERT OR IGNORE,无墓碑)/`write_manual_snapshot`(写穿)/`read_manual_snapshot`。命令 `kg_export_manual/kg_import_manual/kg_write_manual_snapshot`。写命令(confirm_relation/add_tag/remove_tag/remove_relation/extract_concepts)编辑后自动写穿快照。
- **机制A(默认)**:`.manual.kgraph.json` 随 `p2p_sync_knowledge_base_now` 文件同步(mtime LWW,冲突生成 `.conflict` 副本),对端 `kg_import_manual`(整层替换)。前端管理器"同步"按钮已接入。
- **机制B(可选,`kg_set_manual_sync` 门控,存 kg_settings)**:`p2psync.rs` 新增 `KgManualBundle/PushKgManualBundle` 线协议 + `exchange_kg_manual_bundle`,知识库同步后按 KB 双向行级合并(无墓碑,删除不传播,靠机制A兜底)。
- 嵌入向量:本机重算不同步(`content_hash` 驱动增量,避免模型不一致+大 BLOB)。

**后端命令**:kg_build_index/query_graph/node_neighbors/shortest_path/stats/embed_kb/semantic_search/suggest_relations/suggest_tags/confirm_relation/add_tag/remove_tag/remove_relation/detect_communities/communities/save_layout/load_layout/compute_layout/extract_concepts(kg_llm.rs)/set_embedding_model/index_status/**export_manual/import_manual/write_manual_snapshot/set_manual_sync/get_manual_sync/kb_summaries**。`embedding.rs`(llama-server :18081,e5-small/bge-m3,缺模型自动下载)。文件 CRUD 钩子已从 kbmeta 迁到 `kgraph::after_{write_file,create_path,delete_path,move_path}`(fileprocess.rs/flipboard.rs/knowledgebase.rs 调用),只做 mark_kb_dirty/drop_kb_index。

**前端**:`stores/knowledgeBaseStore.js`(全部 kg* actions + manual/sync 封装);`GuaxiKnowledgeBaseManager.vue` 已**重写为 kgraph 编辑器**(节点列表/语义检索/标签增删+建议/关系增删+建议/概念抽取/重建/嵌入/社区/同步/机制B开关);`GuaxiKnowledgeGraph3D.vue` + `kgLayout.js`;`GuaxiCenterBrainView.vue` 恒用 3D(经典分支已删)。

**已废弃删除**:`.meta`/kbmeta 全套数据命令(`kbmeta.rs` 仅剩 `create_knowledge_base_manager_window`)、磁盘根级 `.meta/`、经典图谱 `GuaxiKnowledgeBaseGraph.vue`、`GuaxiKnowledgeBaseGraphEditor.vue`、统计面板改 `kg_kb_summaries`。**保留**:`.cellmeta`(Notebook 侧车,无关)、各 KB 内 `{kb}/.meta/meta.json`(knowledgebase.rs 创建元数据,另一套)。`chatview::kbmeta_agent_process` 仍注册但前端已不用。

**验证状态(2026-06)**:后端 `cargo check` 通过 + 前端 `vite build` 通过。**待真机验证**:① 管理器交互与 3D 联动;② 两机端到端 A/B 同步与冲突;③ embedding 下载/质量/帧率/10万规模。**已知取舍**:机制B无墓碑(删除靠A兜底)、KB 重命名后人工层需一次同步恢复、未做 `.meta`→DB 迁移(旧数据按用户意见废弃)。

详见 [[flipboard-industrial-refactor]] 同类工业级重构模式。
