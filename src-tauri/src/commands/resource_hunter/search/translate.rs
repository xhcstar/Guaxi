//! 关键词多语翻译：把用户输入译成多种主流语言，扩展跨语种召回。
//! 一本书/一篇论文的英文/日文/德文版根本不会匹配中文关键词，译后并行检索能显著提升命中。
//! 用 Google 翻译公开 gtx 端点（keyless、auto 源语言检测），走境外代理路由；失败静默返回空。
use super::super::proxy;

/// 默认目标语言：覆盖学术/书籍/影音内容最密集的语种。en 价值最高，其余次之。
pub const DEFAULT_TARGETS: [&str; 6] = ["en", "ja", "de", "fr", "ru", "es"];

fn client() -> Option<reqwest::Client> {
    proxy::client_for_source(&super::load_proxy(), true).ok()
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
