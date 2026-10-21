//! 网盘分享源：中文电子书的另一条主流通路。
//! 网盘分享**匿名可读**（能拿到分享标题与文件清单，据此判断是不是一本书），
//! 但**匿名下载不可行**：夸克要转存到自己的空间、阿里云盘要登录后才给下载地址。
//! 所以这里只做「发现 + 打开分享」，条目标记 `needs_pan`，前端据此提示走浏览器转存。
use super::super::model::{ResourceItem, ResourceKind};
use super::super::proxy;
use super::make_id;
use regex::Regex;

fn client() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), true).ok()
}

/// 网盘分享链接的域名 → 网盘名。用于识别分享页、以及从 PanSearch 的 HTML 里抽链接。
const PAN_HOSTS: &[(&str, &str)] = &[
    ("pan.quark.cn", "夸克网盘"),
    ("quark.cn", "夸克网盘"),
    ("aliyundrive.com", "阿里云盘"),
    ("alipan.com", "阿里云盘"),
    ("pan.baidu.com", "百度网盘"),
    ("pan.xunlei.com", "迅雷云盘"),
    ("cloud.189.cn", "天翼云盘"),
    ("123pan.com", "123云盘"),
    ("123684.com", "123云盘"),
    ("pan.123684.com", "123云盘"),
    ("115.com", "115网盘"),
    ("caiyun.139.com", "移动云盘"),
    ("mypikpak.com", "PikPak"),
];

/// 从 URL 识别所属网盘；返回 (网盘名, 分享 id)。
fn parse_share(url: &str) -> Option<(&'static str, String)> {
    let u = url::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let pan = PAN_HOSTS
        .iter()
        .find(|(h, _)| host == *h || host.ends_with(&format!(".{h}")))
        .map(|(_, n)| *n)?;
    // 分享 id 通常在 /s/<id> 路径段后；夸克/阿里/百度都是这个形态。
    let segs: Vec<&str> = u.path_segments()?.filter(|s| !s.is_empty()).collect();
    let id = segs
        .windows(2)
        .find(|w| w[0] == "s")
        .map(|w| w[1].to_string())
        .or_else(|| segs.last().map(|s| s.to_string()))?;
    if id.is_empty() {
        return None;
    }
    Some((pan, id))
}

/// 书籍文件名判定：必须带 epub/pdf/azw3 等书籍文件扩展名。
/// 不看「电子书」这类字样——有声书、课程资料也爱这么写，会把音频合集误判成书。
fn bookish_name(name: &str) -> bool {
    let lc = name.to_ascii_lowercase();
    const MEDIA: [&str; 14] = [
        "mp4", "mkv", "avi", "mov", "flv", "wmv", "ts", "rmvb",
        "mp3", "flac", "ape", "wav", "iso", "exe",
    ];
    if MEDIA.iter().any(|e| lc.ends_with(&format!(".{e}"))) {
        return false;
    }
    ["epub", "pdf", "mobi", "azw3", "djvu", "txt"]
        .iter()
        .any(|e| lc.ends_with(&format!(".{e}")))
}

/// 夸克网盘匿名接口：先换 stoken，再列分享内文件。
/// detail 要 GET（POST 会 405），列表主要看文件名判断是不是书。
async fn quark_share(client: &reqwest::Client, share_id: &str) -> Option<(String, Vec<String>)> {
    // 分享链接可能带 .?pwd=xxxx 提取码，调用方已剥离成 path id；这里试空提取码。
    let token_url = "https://drive-pc.quark.cn/1/clouddrive/share/sharepage/token?pr=ucpro&fr=pc";
    let body = serde_json::json!({ "pwd_id": share_id, "passcode": "" });
    let resp = client
        .post(token_url)
        .json(&body)
        .header(reqwest::header::ORIGIN, "https://pan.quark.cn")
        .header(reqwest::header::REFERER, "https://pan.quark.cn/")
        .send()
        .await
        .ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    let data = json.get("data")?;
    let stoken = data.get("stoken")?.as_str()?.to_string();
    let title = data
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut files = Vec::new();
    // 根目录列一次；若根下是单个目录，再进一层（书常常打包在文件夹里）。
    let mut pdir = "0".to_string();
    for _ in 0..2 {
        let url = format!(
            "https://drive-pc.quark.cn/1/clouddrive/share/sharepage/detail?pr=ucpro&fr=pc&pwd_id={}&stoken={}&pdir_fid={}&force=0&_page=1&_size=50",
            urlencoding::encode(share_id),
            urlencoding::encode(&stoken),
            urlencoding::encode(&pdir),
        );
        let resp = client
            .get(&url)
            .header(reqwest::header::ORIGIN, "https://pan.quark.cn")
            .header(reqwest::header::REFERER, "https://pan.quark.cn/")
            .send()
            .await
            .ok()?;
        let json: serde_json::Value = resp.json().await.ok()?;
        let list = json.get("data").and_then(|d| d.get("list")).and_then(|v| v.as_array())?;
        let mut first_dir: Option<String> = None;
        for f in list {
            let name = f.get("file_name").and_then(|v| v.as_str()).unwrap_or("");
            let is_dir = f.get("file_type").and_then(|v| v.as_u64()).unwrap_or(0) == 0;
            if is_dir {
                if first_dir.is_none() {
                    first_dir = f.get("fid").and_then(|v| v.as_str()).map(|s| s.to_string());
                }
                if !name.is_empty() {
                    files.push(name.to_string());
                }
            } else if !name.is_empty() {
                files.push(name.to_string());
            }
        }
        match first_dir {
            Some(d) if pdir == "0" => pdir = d,
            _ => break,
        }
    }
    Some((title, files))
}

/// 阿里云盘分享：匿名接口直接回分享标题与文件清单，无需 token。
async fn alipan_share(client: &reqwest::Client, share_id: &str) -> Option<(String, Vec<String>)> {
    let url = format!(
        "https://api.alipan.com/adrive/v3/share_link/get_share_by_anonymous?share_id={}",
        urlencoding::encode(share_id)
    );
    let body = serde_json::json!({ "share_id": share_id });
    let resp = client
        .post(&url)
        .json(&body)
        .header(reqwest::header::ORIGIN, "https://www.alipan.com")
        .header(reqwest::header::REFERER, "https://www.alipan.com/")
        .send()
        .await
        .ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    let title = json
        .get("share_name")
        .or_else(|| json.get("display_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let files = json
        .get("file_infos")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.get("file_name").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    Some((title, files))
}

/// 把一条网盘分享变成资源条目。判定为书籍才返回：
/// 分享标题或文件清单里出现书籍格式/电子书字样即可；否则视为无关分享丢弃。
fn share_item(url: &str, hint_title: &str) -> Option<ResourceItem> {
    let (pan, id) = parse_share(url)?;
    Some(ResourceItem {
        id: make_id(url),
        title: if hint_title.trim().is_empty() {
            format!("{pan} 分享 · {id}")
        } else {
            hint_title.trim().to_string()
        },
        kind: ResourceKind::Book,
        source: pan.to_string(),
        // 分享链接本身既是详情页也是打开目标；标 needs_pan 让前端走浏览器、提示转存。
        source_url: url.to_string(),
        download_url: None,
        magnet: None,
        needs_ytdlp: false,
        needs_pan: true,
        format: None,
        thumbnail: None,
        author: None,
        published_at: None,
        summary: None,
        size_hint: None,
        relevance: 0.72,
        tags: vec!["网盘".into()],
    })
}

/// 补全一条网盘条目：拿分享标题与文件清单，标题更准就替换，并把文件名作为摘要与格式。
/// 返回 false 表示这条分享不是书（调用方据此丢弃）。
pub async fn enrich_share(item: &mut ResourceItem) -> bool {
    let client = match client() {
        Some(c) => c,
        None => return true, // 网络不可用时保留原样（标题由搜索侧提供）
    };
    let (pan, id) = match parse_share(&item.source_url) {
        Some(v) => v,
        None => return false,
    };
    let got = match pan {
        "夸克网盘" => quark_share(&client, &id).await,
        "阿里云盘" => alipan_share(&client, &id).await,
        // 其它网盘没有匿名文件清单接口：只能看分享标题里的文件扩展名。
        _ => return bookish_name(&item.title),
    };
    let (title, files) = match got {
        // 接口失败/限流（阿里云盘匿名接口常回 429 或空清单）不能当成「不是书」：
        // 按分享标题里的书籍扩展名判断，判不了就保留（前端有「网盘分享」标记，用户一眼能分辨）。
        None => return true,
        Some(v) => v,
    };
    if files.is_empty() {
        if !title.is_empty() {
            item.title = title;
        }
        return true;
    }

    // 判定这本书是不是分享内容：
    // - 文件清单里出现书籍扩展名（夸克能列到真实文件名）→ 收；
    // - 只列到目录名时，看分享标题/目录名本身是否是文件；
    // - 阿里云盘只能拿到一层文件名，同样是保守判断。
    let file_hit = files.iter().find(|f| bookish_name(f));
    let is_book = file_hit.is_some() || bookish_name(&title);
    if !is_book {
        return false;
    }
    if !title.is_empty() {
        item.title = title.clone();
    }
    // 标题常是「XX等 N 个文件」的聚合名，用清单补全摘要与格式。
    let ext = file_hit
        .and_then(|f| f.rsplit('.').next())
        .map(|e| e.to_ascii_lowercase())
        .or_else(|| title.rsplit('.').next().filter(|_| bookish_name(&title)).map(|e| e.to_ascii_lowercase()));
    if !files.is_empty() {
        item.summary = Some(format!("分享内文件：{}", files.join("、")));
    }
    if let Some(e) = ext {
        item.format = Some(e);
    }
    true
}

/// PanSearch 聚合：一个站覆盖夸克/阿里/百度/迅雷等多网盘，是中文书分享的主要入口。
/// 该站国内直连会被重置，必须走代理。
///
/// 站内结果按「网盘日报」聚合，影视/资料远多于书。用「书名 + 书籍格式」多组并发查，
/// 只保留能证明是书的分享（见 `enrich_share`），否则书会被影音资源淹没。
pub async fn pansearch(kw: &str, limit: usize) -> Vec<ResourceItem> {
    let client = match client() {
        Some(c) => c,
        None => return vec![],
    };
    // 带格式词的命中质量远高于泛搜（泛搜基本是影视合集）。
    let queries = [format!("{kw} epub"), format!("{kw} pdf"), format!("{kw} 电子书")];
    let futs = queries.iter().map(|q| {
        let c = client.clone();
        let url = format!(
            "https://www.pansearch.me/api/search?keyword={}&page=1",
            urlencoding::encode(q)
        );
        async move {
            let resp = c
                .get(&url)
                .header(reqwest::header::ACCEPT, "application/json")
                .header(reqwest::header::REFERER, "https://www.pansearch.me/")
                .send()
                .await
                .ok()?;
            resp.json::<serde_json::Value>().await.ok()
        }
    });
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for json in futures_util::future::join_all(futs).await.into_iter().flatten() {
        for it in parse_pansearch(&json, limit) {
            if seen.insert(it.source_url.clone()) {
                out.push(it);
            }
        }
    }
    out.truncate(limit.saturating_mul(2));
    out
}

/// PanSearch 返回的 `content` 是带 HTML 的日报式文本：
/// `1、书名: <a href="分享地址">…</a>`，一个条目里可能塞多条链接。
fn parse_pansearch(json: &serde_json::Value, limit: usize) -> Vec<ResourceItem> {
    let anchor_re = Regex::new(r#"<a[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).unwrap();
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in json.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
        let content = entry.get("content").and_then(|v| v.as_str()).unwrap_or("");
        // 标题取链接前、行首到冒号之间的那段文字（"1、樊登讲论语先进:"）。
        for c in anchor_re.captures_iter(content) {
            let url = html_unescape(c[1].trim());
            if parse_share(&url).is_none() || !seen.insert(url.clone()) {
                continue;
            }
            let label = tag_re.replace_all(&c[2], "").trim().to_string();
            // 标题取链接所在行的行首文字（"名称：红楼梦 | 链接：<a…>" 的「名称」段）。
            let raw = &content[..c.get(0).map(|m| m.start()).unwrap_or(0).min(content.len())];
            let line = raw.rsplit('\n').next().unwrap_or(raw);
            // 去掉行首序号（"1、"、"2."），再取冒号前那一段。
            // 行首文字里也可能夹着高亮标签（`鬼吹灯 <span …>三体 电子书</span>`），一并去掉。
            let head = tag_re
                .replace_all(line, "")
                .trim_start_matches(|c: char| c.is_ascii_digit() || c == '、' || c == '.')
                .split([':', '：'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            // 只写「链接」的行没有书名信息，此时用锚文本（很多条目锚文本就是书名）。
            let title = if head.len() >= 2 && !is_placeholder(&head) {
                head
            } else if label.len() >= 2 && !is_placeholder(&label) {
                label
            } else {
                // 一条既没有书名行、锚文本也只是「链接」的分享，无法判断它是什么，丢掉。
                continue;
            };
            // 站内会把命中的关键词用 span 高亮、并用方括号补格式词（"论语[epub]"），
            // 这些都不是书名的一部分。
            let title = clean_title(&title);
            if title.len() < 2 || title.starts_with("http") {
                continue;
            }
            if let Some(it) = share_item(&url, &title) {
                out.push(it);
            }
            if out.len() >= limit {
                return out;
            }
        }
    }
    out
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 各家网盘的 /s/<id> 形态都要能识别出网盘与分享 id。
    #[test]
    fn parse_share_identifies_pan_and_id() {
        assert_eq!(
            parse_share("https://pan.quark.cn/s/0f4387ab7fba"),
            Some(("夸克网盘", "0f4387ab7fba".to_string()))
        );
        assert_eq!(
            parse_share("https://www.aliyundrive.com/s/J6HSe4g5w1a"),
            Some(("阿里云盘", "J6HSe4g5w1a".to_string()))
        );
        assert_eq!(
            parse_share("https://pan.baidu.com/s/1sxGBauY0dDURq2h24DJA1Q?pwd=2wz8"),
            Some(("百度网盘", "1sxGBauY0dDURq2h24DJA1Q".to_string()))
        );
        // 普通网页不是分享链接。
        assert_eq!(parse_share("https://example.com/books/1"), None);
    }

    /// PanSearch 的日报式文本一条里塞多条链接，标题取链接前的行首文字。
    #[test]
    fn pansearch_extracts_titles() {
        let json = serde_json::json!({
            "data": [{
                "content": "1、樊登讲论语先进:<a class=\"resource-link\" href=\"https://pan.quark.cn/s/0f4387ab7fba\">x</a>;\n\n2、机器学习:<a href=\"https://www.aliyundrive.com/s/J6HSe4g5w1a\">y</a>;"
            }]
        });
        let got = parse_pansearch(&json, 10);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].title, "樊登讲论语先进");
        assert_eq!(got[0].source, "夸克网盘");
        assert!(got[0].needs_pan);
        assert_eq!(got[1].title, "机器学习");
    }

    /// 只有书籍格式才算书，视频合集等应被排除。
    #[test]
    fn bookish_name_accepts_ebook_formats() {
        assert!(bookish_name("《论语》.epub"));
        assert!(bookish_name("book.PDF"));
        assert!(bookish_name("某书.txt"));
        // 只有「电子书」字样、没有书籍文件的不算（有声书/课程资料常这么写）。
        assert!(!bookish_name("有声书电子书汇总"));
        assert!(!bookish_name("全36集 国语中字 1080P"));
        assert!(!bookish_name("课程.mp4"));
    }
}

/// 清洗 PanSearch 给的标题：去掉高亮残留的方括号格式词（`论语[epub]`）和多余空白。
fn clean_title(s: &str) -> String {
    let re = Regex::new(r"(?i)\s*[\[(（【]?\s*(epub|pdf|mobi|azw3|txt|djvu)\s*[\])）】]?\s*$").unwrap();
    let stripped = re.replace(s, "").into_owned();
    // 末尾残留的半截高亮（`论语[epub`）也一并削掉。
    stripped
        .trim()
        .trim_end_matches(['[', '(', '（', '【'])
        .trim()
        .to_string()
}

/// 站内占位式锚文本（「链接」「点击」「下载」），没有书名信息。
fn is_placeholder(s: &str) -> bool {
    matches!(s.trim(), "链接" | "点击" | "下载" | "网盘链接" | "地址" | "here" | "link")
}
