//! LLM 查询扩展：把检索词扩成同义词 / 中英互译 / 相关术语，提升召回与精度。
//! 复用智能助手的模型通道；无 key / 调用失败时静默降级返回空。
use crate::commands::chatview;

pub async fn expand_query(keyword: &str) -> Vec<String> {
    let prompt = format!(
        "你是搜索助手。为下面的检索词生成用于全网寻找资源(书籍/论文/视频/音频/图片/代码)的扩展关键词，\
         覆盖中英文同义词、相关术语、常见作者或组织名。严格只输出一个 JSON 字符串数组(最多6个)，\
         不要任何解释、不要 markdown 代码围栏。\n检索词：{keyword}"
    );
    match chatview::get_answer_from_siliconflow(&prompt, &[], "").await {
        Ok(resp) => parse_array(&resp.answer),
        Err(_) => vec![],
    }
}

/// 从模型回复里容错抽出 JSON 字符串数组。
fn parse_array(text: &str) -> Vec<String> {
    let start = text.find('[');
    let end = text.rfind(']');
    if let (Some(s), Some(e)) = (start, end) {
        if e > s {
            if let Ok(v) = serde_json::from_str::<Vec<String>>(&text[s..=e]) {
                return v
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    vec![]
}
