//! 关键词多语翻译：把用户输入译成多种主流语言，扩展跨语种召回。
//! 一本书/一篇论文的英文/日文/德文版根本不会匹配中文关键词，译后并行检索能显著提升命中。
//! 用 Google 翻译公开 gtx 端点（keyless、auto 源语言检测），走境外代理路由；失败静默返回空。
use super::super::proxy;

/// 多语检索开启时追加的目标语言（中英已由 cross_cn_en 强制覆盖，这里是“可选的其它语种”）。
pub const EXTRA_TARGETS: [&str; 5] = ["ja", "de", "fr", "ru", "es"];

fn client() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), true).ok()
}

/// 是否含 CJK（中日韩统一表意文字 / 假名 / 谚文）字符，用于判断中英互译方向。
fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3040}'..='\u{30ff}').contains(&c)
            || ('\u{ac00}'..='\u{d7af}').contains(&c)
    })
}

/// 中英互译（始终执行，保证“中英双语召回必含”）：含 CJK 则译英，否则译中。
pub async fn cross_cn_en(keyword: &str) -> Vec<String> {
    let target = if has_cjk(keyword) { "en" } else { "zh-CN" };
    translate_multi(keyword, &[target]).await
}

/// 把 keyword 译成单一目标语言。gtx 返回形如 [[[译文,原文,...],...],...]，拼接首层各段译文。
async fn translate_one(client: &reqwest::Client, keyword: &str, target: &str) -> Option<String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&q={}",
        target,
        urlencoding::encode(keyword)
    );
    let json: serde_json::Value = client.get(&url).send().await.ok()?.json().await.ok()?;
    let segs = json.get(0)?.as_array()?;
    let mut out = String::new();
    for s in segs {
        if let Some(t) = s.get(0).and_then(|v| v.as_str()) {
            out.push_str(t);
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() { None } else { Some(out) }
}

/// 把 keyword 译成多种语言，返回去重后的译文集合（不含与原词相同者）。
/// 并发发起、单个失败不影响其它；整体尽力而为。
pub async fn translate_multi(keyword: &str, targets: &[&str]) -> Vec<String> {
    let kw = keyword.trim();
    if kw.is_empty() {
        return vec![];
    }
    let client = match client() {
        Some(c) => c,
        None => return vec![],
    };
    let futs = targets.iter().map(|t| {
        let c = client.clone();
        async move { translate_one(&c, kw, t).await }
    });
    let results = futures_util::future::join_all(futs).await;
    let kw_lc = kw.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for r in results.into_iter().flatten() {
        let lc = r.to_lowercase();
        if lc == kw_lc || !seen.insert(lc) {
            continue;
        }
        out.push(r);
    }
    out
}
