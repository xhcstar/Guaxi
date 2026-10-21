//! 解析层：对“只有详情页、没有直链”的条目尽力解析出可下载直链。
//! 复用 sniffer 的页面抓取 + 直链/磁力提取；影子库(Anna's) 的 md5 改走 LibGen 页解析
//! （Anna's 匿名搜索/下载被 DDoS-Guard 挑战拦截，但同一 md5 在 LibGen 上无验证码）。
use super::super::model::{ResourceItem, ResourceKind};
use super::sniffer;
use futures_util::future::join_all;

/// 单条待解析项的抓取目标：影子库详情页改写为更易解析的镜像页。
fn resolve_target(it: &ResourceItem) -> String {
    if it.source == "AnnasArchive" {
        if let Some(tail) = it.source_url.split("/md5/").nth(1) {
            let hash: String = tail.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if hash.len() >= 16 {
                return format!("https://libgen.li/ads.php?md5={hash}");
            }
        }
    }
    it.source_url.clone()
}

/// 对详情页-only 条目（无直链/磁力/非 yt-dlp）就地解析直链。限量并发，慢源不拖垮整体。
pub async fn resolve_items(items: Vec<ResourceItem>) -> Vec<ResourceItem> {
    // 若干站点不是「扒页面找链接」那么简单，各有专用解析：
    // - 苦瓜书盘：详情页的下载按钮要带着 Referer 跟 302；
    // - Standard Ebooks：只有详情页里才有 `.epub` 直链；
    // - 网盘分享：匿名只能读清单，顺便判定它到底是不是书。
    // 这一步会剔除「不是书」的网盘条目，所以返回新的 Vec 而不是就地改切片。
    let mut items = resolve_special(items).await;

    // 先摘出待解析项的owned数据（下标 + 抓取目标 + 标题 + 类型），释放对 items 的借用，
    // 再并发抓取，最后回填——避免借用冲突。
    let jobs: Vec<(usize, String, String, ResourceKind)> = items
        .iter()
        .enumerate()
        .filter(|(_, it)| {
            it.download_url.is_none()
                && it.magnet.is_none()
                && !it.needs_ytdlp
                && !it.needs_pan
                && !it.source_url.is_empty()
        })
        .take(16)
        .map(|(i, it)| (i, resolve_target(it), it.title.clone(), it.kind))
        .collect();
    if jobs.is_empty() {
        return items;
    }
    let futs = jobs.into_iter().map(|(i, target, title, kind)| async move {
        let got = super::timed(9, sniffer::sniff(&target, &title, &[kind])).await;
        let hit = got
            .into_iter()
            .find(|r| r.download_url.is_some() || r.magnet.is_some() || r.needs_ytdlp);
        (i, hit)
    });
    for (i, hit) in join_all(futs).await {
        if let Some(r) = hit {
            items[i].download_url = r.download_url;
            items[i].magnet = r.magnet;
            if r.needs_ytdlp {
                items[i].needs_ytdlp = true;
            }
            if items[i].format.is_none() {
                items[i].format = r.format;
            }
        }
    }
    items
}

/// 走各自专用解析器的源。解析失败/判定不是书的条目直接丢弃。
async fn resolve_special(items: Vec<ResourceItem>) -> Vec<ResourceItem> {
    let mut keep = Vec::with_capacity(items.len());
    for mut it in items {
        match it.source.as_str() {
            "苦瓜书盘" if it.download_url.is_none() => {
                match super::timed(14, super::book_sources::kgbook_detail(&it.source_url)).await {
                    Some(full) => {
                        // 详情页才是权威元数据：标题/作者/大小/格式以它为准。
                        keep.push(full);
                    }
                    // 详情页打不开就退回搜索页给的那条（至少还能点开看）。
                    None => keep.push(it),
                }
            }
            "StandardEbooks" if it.download_url.is_none() => {
                match super::timed(14, super::book_sources::standard_ebooks_detail(&it.source_url)).await {
                    Some(dl) => {
                        it.download_url = Some(dl);
                        keep.push(it);
                    }
                    None => keep.push(it),
                }
            }
            _ if it.needs_pan => {
                // timed 超时返回 T::default()；用 Option 包一层把「超时」与「判定为否」区分开：
                // 超时/网络故障保守保留（宁可多列，也不要静默吞掉整源结果）。
                let verdict = super::timed(14, async {
                    Some(super::pan_sources::enrich_share(&mut it).await)
                })
                .await;
                if verdict.unwrap_or(true) {
                    keep.push(it);
                }
            }
            _ => keep.push(it),
        }
    }
    keep
}
