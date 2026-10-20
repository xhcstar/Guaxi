pub mod agent_server;
/// RSSHub sidecar 进程管理（进程内集成，复用 SlidevEnv 的 node）。
pub mod rsshub_server;
/// cc-switch（第三方供应商切换工具）的只读探测，用于把 base_url 指向它的本地代理。
pub mod cc_switch;
pub mod chatview;
/// Codex Agent 引擎：以打包进安装包的 codex sidecar 驱动智能助手的 Agent 模式。
pub mod codex_agent;
pub mod docprocess;
/// 文档阅读器后端支撑：文件类型嗅探、电子书/旧版 Office 阅读载荷、表格读取、导入用便携转换。
/// 全部是新增的独立命令，不改动任何既有命令的行为。
pub mod doc_reader;
pub mod convert;
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
pub mod flipboard_extract;
pub mod flipboard_render;
pub mod flipboard_llm;
pub mod site_category_discovery;
pub mod language_learning;
pub mod tts_book;
pub mod syntax_highlight;
pub mod typst_compile;
pub mod pdf_reader;
pub mod preview_render_cache;
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
