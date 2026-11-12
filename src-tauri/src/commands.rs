pub mod agent_server;
/// RSSHub sidecar 进程管理（进程内集成，复用 SlidevEnv 的 node）。
pub mod rsshub_server;
/// cc-switch（第三方供应商切换工具）的只读探测，用于把 base_url 指向它的本地代理。
pub mod cc_switch;
pub mod chatview;
/// 大模型供应商的统一清单与解析：对话、Codex Agent、Python Agent 共用一份。
pub mod llm_provider;
pub mod plugin_llm;
/// 旅行计划：高德/OSM 双通道数据、路线、天气、LLM 包装、本地计划存储与局域网分享。
pub mod travel;
/// 智能助手联网检索：多引擎并发 + 正文摘录，给模型提供可读的联网资料。
pub mod web_search;
/// Codex Agent 引擎：以打包进安装包的 codex sidecar 驱动智能助手的 Agent 模式。
pub mod codex_agent;
/// Codex App-Server 引擎：常驻 JSON-RPC 进程，提供 steer / 审批 / 计划 / diff / 用量，
/// 供 Agent 工作台使用。与 `codex_agent` 共用同一个二进制与配置，失败可降级到它。
pub mod codex_app_server;
/// 电脑操控：截屏、窗口、鼠标键盘、剪贴板。全部受 `Tier::Gui` 管制，默认关闭。
pub mod computer;
pub mod docprocess;
/// 文档阅读器后端支撑：文件类型嗅探、电子书/旧版 Office 阅读载荷、表格读取、导入用便携转换。
/// 全部是新增的独立命令，不改动任何既有命令的行为。
pub mod doc_reader;
pub mod doc_reader_window;
pub mod convert;
pub mod craft_app;
pub mod craft_cli;
pub mod office_app;
pub mod reader_state;
pub mod import_suggest;
pub mod file_watcher;
pub mod file_index;
pub mod fulltext;
pub mod fileprocess;
pub mod life_album;
pub mod life_album_viewer_window;
pub mod guaxireader;
pub mod readaloud;
pub mod repoview;
pub mod serverview;
pub mod settingview;
pub mod templateview;
pub mod resource_repo_window;
/// 资源库管理器的只读支撑：一次扫描拿到全库条目、按需生成缩略图、反查资源被哪些文档引用。
pub mod resource_repo;
pub mod slidev;
pub mod cloudsync;
pub mod p2psync;
pub mod statisticview;
pub mod guaxiscreencap;
pub mod ocr;
pub mod translator;
/// 免费翻译接口池：多家免费接口竞速 + 健康度熔断 + 限速防封。
pub mod translate_pool;
/// 本地翻译小模型（llama.cpp）：免费接口全挂/被封/额度用完时的兜底翻译。
pub mod translate_local;
/// 翻译结果的持久化缓存（SQLite），全文翻译重复触发时零网络请求。
pub mod translate_cache;
pub mod translator_free;
pub mod searcher;
/// 通用差异对比引擎：任意两个文件 / 目录，和 git 无关。
pub mod diffcompare;
/// 承载差异对比界面的独立窗口，以及主窗口交接过来的对比请求。
pub mod diff_compare_window;
/// 独立的写信窗口：回复 / 转发不再挡住整个应用页。
pub mod mail_compose_window;
pub mod knowledgebase;
pub mod kbmeta;
pub mod kgraph;
pub mod embedding;
pub mod kg_llm;
pub mod excerptsearch;
pub mod git;
pub mod markdown_reader_doc_process;
pub mod word_reader_doc_process;
pub mod epub_reader_doc_process;
pub mod text_reader_doc_process;
pub mod backup;
pub mod markdowncheck;
pub mod flipboard;
pub mod markdown_manual;
pub mod notebook;
pub mod todo;
/// 简历工坊：多份简历、版本快照与自定义模板的本地 SQLite 存储。
pub mod resume;
pub mod music;
pub mod filetransfer;
pub mod library_site;
pub mod code_runtime;
pub mod editor_plugin;
pub mod code_cell_runtime;
pub mod terminal;
pub mod remote_terminal;
pub mod ssh_terminal;
pub mod gdb_mi;
pub mod remote_terminal_tools;
// 远程工具台：在 SSH 档案对应的远程 Linux 上执行用户自备工具并传输文件。
pub mod remote_tool;
pub mod serial_telnet;
pub mod net_toolbox;
pub mod net_conn;
pub mod net_capture;
pub mod sftp_fs;
pub mod sync;
pub mod crypto_lab;
// 远程 Tongsuo/OpenSSL 的 SM2/PQC 证书工作流（不要求本机安装 Tongsuo）。
pub mod crypto_remote;
// 命令下发（配置驱动的设备 CLI 批量执行）。
pub mod cmd_dispatch;
// 命令下发用的命令目录与场景模板（数据来自产品 CLI 手册）。
pub mod cmd_catalog;
// 源码定位（把调试信息里的路径映射到本机/远端源码，支持 chroot/docker 构建布局）。
pub mod source_locate;
// 国密 TLCP/SM2/PQC 引擎：仅在 `tlcp` 特性下编译（绑定 Tongsuo）。默认构建不包含。
#[cfg(feature = "tlcp")]
pub mod crypto_engine;
pub mod frontend_logging;
pub mod notebook_lab;
pub mod flipboard_pipeline;
pub mod flipboard_admit;
pub mod flipboard_items;
pub mod flipboard_clean;
pub mod flipboard_pagekind;
pub mod flipboard_quality;
pub mod flipboard_dedup;
pub mod flipboard_feeds;
pub mod flipboard_extract;
pub mod flipboard_render;
pub mod flipboard_llm;
pub mod site_category_discovery;
pub mod language_learning;
/// 语言学习离线词典包：ECDICT / CC-CEDICT 目录下载、导入导出与本地查询。
pub mod dict_formats;
pub mod offline_dict;
/// 语言学习多语种 ASR 模型：按语言路由、按需下载与卸载。
pub mod asr_models;
/// 网页 / URL 导入：公开页面正文、音频、字幕 -> 听力或阅读条目。
pub mod web_import;
pub mod tts_book;
pub mod syntax_highlight;
pub mod typst_compile;
pub mod typst_package;
pub mod pdf_reader;
pub mod preview_render_cache;
/// 后端图片登记表的代次：渲染缓存要跟着它一起失效。
pub mod render_cache_epoch;
// 后端服务的登记表与启停控制（「设置 → 服务器」页面的后端）。
pub mod service_control;
pub mod synctex;
pub mod task_block;
pub mod taskmanager;
pub mod task_manager_window;
pub mod neovim;
/// AI 影像工坊与 Blender 5.2 LTS 的本地桥接命令。
/// 独立成模块，避免把进程管理、工程目录和 Blender 版本探测继续堆入 chatview。
pub mod story_studio;
/// Story IR 共享校验器，供模型编译、项目保存和项目重开共同使用。
pub mod story_ir;
/// 资源嗅探下载引擎：全网搜索 → 嗅探文件直链 → 多线程/续传/yt-dlp 下载 → 资源库管理。
pub mod resource_hunter;
/// 邮箱客户端：IMAP/SMTP + 代理 + 本地检索 + 规则。
pub mod mail;
