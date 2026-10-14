//! 文档转换引擎（多引擎路由 + 持久化任务队列）。
//!
//! 设计目标：
//! - 简单文档走快速引擎（pymupdf4llm / markitdown），复杂理工科文档走强引擎（MinerU），
//!   MinerU 不可用时自动回退到 marker。
//! - 另有「极速引擎」（anydoc，纯 Rust）：零装配、进程内直调，只适合纯文本型文档、不做 OCR。
//!   既可以由用户在下拉里显式选择，也会被探针路由到——旧版 Office / RTF / OpenDocument
//!   （.doc/.ppt/.rtf/.odt/.ods/.odp）只有它能读（快速引擎的 markitdown 没有这些转换器）。
//! - 任务持久化到 SQLite `conversion_tasks`，支持进度检查、断点续跑、失败重试、取消。
//! - 完全向后兼容：保留旧的 `conversion_progress` 事件与 `output_dir/{stem}/{stem}.md` 输出契约。
//!
//! 子模块：
//! - [`task_store`]：SQLite 任务表的 CRUD / 查询封装（复用 repoview::init_db）。
//! - [`engine`]：各引擎路径解析、探针、子进程构建，以及极速引擎的进程内转换。
//! - [`net`]：转换链路的网络环境（跟随系统代理 / 直连）与带代理的下载。
//! - [`provision`]：各引擎运行时的自动装配（uv 建 venv、依赖与模型下载）。
//! - [`queue`]：后台队列 worker（放入 Tauri app state），并发控制 + 进度落库 + 事件广播。
//! - [`commands`]：暴露给前端的 Tauri 命令。

pub mod task_store;
pub mod engine;
pub mod net;
pub mod provision;
pub mod queue;
pub mod commands;

pub use queue::ConversionQueue;
