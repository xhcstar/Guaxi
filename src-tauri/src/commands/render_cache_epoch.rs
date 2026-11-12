//! 渲染缓存「图片授权代次」的查询命令。
//!
//! 背景：`/markdown/api/images/<绝对路径>` 只放行三类文件 —— 图床/库/知识库根目录下的，
//! 以及**渲染预处理阶段登记过**的（笔记放在这三个根之外时全靠这条）。那份登记表在内存里
//! （见 `markdown_image_process::ResolvedImageRegistry`），遇到两种事会丢掉已有登记：
//! 达到上限整体清空、进程重启。
//!
//! 而前端有若干层渲染缓存（编辑器预览的分段/全文缓存、Notebook 的 cell 缓存）把图片 URL
//! 直接存进了 HTML。表一旦清空，缓存仍会命中，浏览器于是拿旧 URL 去请求一个不再被授权的
//! 文件 —— 图片集体 403，且只有改一次文档或手动点「重新加载」才会恢复。
//!
//! 解法：把登记表代次并进渲染缓存的 key。这里只提供查询入口；具体何时拼 key 由前端决定。

use crate::guaxiservices::markdown_image_process;

/// 本进程的启动标识。
///
/// 为什么还需要它：进程重启后登记表代次会重新从 0 开始，光看代次数字是分不清
/// 「重启前清过 2 次」和「刚重启没清过」的。把启动标识一并给出去，前端拼进 key 之后
/// 只要进程换了，缓存自然全部失效。
fn instance_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("{millis}")
    })
}

/// 缓存 key 用的完整代次串：`<进程启动毫秒>-<登记表代次>`。
#[tauri::command]
pub fn render_cache_epoch() -> String {
    format!("{}-{}", instance_id(), markdown_image_process::registry_epoch())
}
