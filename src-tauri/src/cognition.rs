//! 认知层（cognition）：Guaxi 的"主动大脑"。
//!
//! 它不是又一个功能纵向，而是横切在已有能力之上的一层：
//!
//! ```text
//! 感知 percept ──► 记忆 episode / belief / profile ──► 召回 recall ──► 主动 nudge / brief
//!                             ▲                                            │
//!                             └──────────── 进化 evolve（评测闩） ◄─────────┘
//! ```
//!
//! | 模块 | 职责 |
//! |---|---|
//! | `store`   | cognition.db 的 schema、迁移、token 账本与工具审计 |
//! | `percept` | 一切外部输入的统一入口 + 不可信内容中和 + 新颖度/显著性打分 |
//! | `episode` | append-only 行为轨迹，学习与审计的唯一底料 |
//! | `profile` | 用户模型：markdown 是真相，SQLite 是派生 |
//! | `recall`  | 四路证据融合成带引证的上下文（对话链路的接地入口） |
//! | `belief`  | 从知识库抽出可被反驳的主张，并检测新材料与它的冲突 |
//! | `nudge`   | 审议循环产出提醒 + 治理器（预算、免打扰、自适应阈值） |
//! | `brief`   | 晨间简报：绝大多数提醒的归宿，不需要打断权限 |
//! | `evolve`  | 自调参数的评测闩：不过评测不许落库，且一切可回滚 |
//! | `tools`   | 分级工具面，Agent / MCP / UI 共用 |
//! | `mcp`     | JSON-RPC 2.0 的 MCP 服务端，挂在既有 127.0.0.1:8888 上 |
//! | `runtime` | 全局 AppHandle、四条后台循环、总开关 |
//!
//! 三条贯穿全层的硬约束：
//! 1. **不可信内容永不越权**：percept 派生文本只能当资料读，且不得触发写/网络/执行类工具。
//! 2. **本地优先**：感知流的打分、新颖度、亲和度全走本地 embedding；远端模型只用于
//!    每天真正触达用户的那少数几件事，且受每日 token 预算硬约束。
//! 3. **可解释可回滚**：所有自调产物都是带版本的行，附带评测分数与一键回滚。

pub mod activity;
pub mod belief;
pub mod brief;
pub mod commands;
pub mod episode;
pub mod evolve;
pub mod fsrs;
pub mod hooks;
pub mod learning;
pub mod library_digest;
pub mod mcp;
pub mod mcp_bridge;
pub mod memory;
pub mod nudge;
pub mod percept;
pub mod profile;
pub mod ranker;
pub mod reading;
pub mod recall;
pub mod runtime;
pub mod store;
pub mod tools;
pub mod tasks;

pub use runtime::{init_app_handle, shutdown, start_background_loops};
pub use hooks::{hook_agent_start, hook_chat_failed, hook_chat_start, hook_percept};
