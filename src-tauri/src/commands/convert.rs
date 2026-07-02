//! 文档转换引擎（多引擎路由 + 持久化任务队列）。
//!
//! 设计目标：
//! - 简单文档走快速引擎（pymupdf4llm / markitdown），复杂理工科文档走强引擎（MinerU），
//!   MinerU 不可用时自动回退到 marker。
//! - 任务持久化到 SQLite `conversion_tasks`，支持进度检查、断点续跑、失败重试、取消。
//! - 完全向后兼容：保留旧的 `conversion_progress` 事件与 `output_dir/{stem}/{stem}.md` 输出契约。
//!
//! 子模块：
//! - [`task_store`]：SQLite 任务表的 CRUD / 查询封装（复用 repoview::init_db）。
//! - [`engine`]：各引擎 venv/脚本路径解析、探针、子进程构建（marker 路径保持不变）。
//! - [`queue`]：后台队列 worker（放入 Tauri app state），并发控制 + 进度落库 + 事件广播。
//! - [`commands`]：暴露给前端的 Tauri 命令。

pub mod task_store;
pub mod engine;
pub mod provision;
pub mod queue;
pub mod commands;

pub use queue::ConversionQueue;
