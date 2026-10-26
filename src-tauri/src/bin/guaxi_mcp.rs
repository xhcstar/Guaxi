//! `guaxi-mcp`：stdio ⇄ HTTP 的薄桥接。
//!
//! 真正的主体在 `src/cognition/mcp_bridge.rs`，主程序在 `--mcp-server`
//! 模式下也跑同一份实现。这里保留独立二进制，是为了兼容旧的外部客户端配置。
//!
//! 这个 bin 直接 `#[path]` 引入源文件，而不是依赖 `guaxi_lib`：本项目的
//! lib.rs 是空的（模块都挂在 main.rs 下），bin 链接不到 lib 里的模块。

#[path = "../cognition/mcp_bridge.rs"]
mod mcp_bridge;

fn main() {
    mcp_bridge::run_stdio_bridge();
}
