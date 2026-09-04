pub mod agent_server;
/// cc-switch（第三方供应商切换工具）的只读探测，用于把 base_url 指向它的本地代理。
pub mod cc_switch;
pub mod chatview;
/// Codex Agent 引擎：以打包进安装包的 codex sidecar 驱动智能助手的 Agent 模式。
pub mod codex_agent;
pub mod docprocess;
pub mod convert;
pub mod import_suggest;
pub mod file_watcher;
pub mod file_index;
pub mod fulltext;
pub mod fileprocess;
pub mod life_album;
pub mod guaxireader;
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
pub mod filetransfer;
pub mod code_runtime;
pub mod code_cell_runtime;
pub mod terminal;
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
