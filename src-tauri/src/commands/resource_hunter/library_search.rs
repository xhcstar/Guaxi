//! 资源库全文检索：tantivy + jieba 中文分词。RAM 索引，按 store 版本号惰性重建
//! （资源库规模有限，重建代价只是一次扫库）。查询语法出错时回退子串过滤。
use super::store;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::Mutex;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING};
use tantivy::tokenizer::{LowerCaser, TextAnalyzer};
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument};

const TOKENIZER: &str = "rh_jieba";

struct State {
    index: Index,
    reader: IndexReader,
    id_field: Field,
    title_field: Field,
    tags_field: Field,
    version: u64,
    /// id -> 完整资源 JSON（返回给前端，避免在索引里存全部字段）。
    rows: HashMap<String, serde_json::Value>,
    order: Vec<String>,
}

fn state_cell() -> &'static Mutex<Option<State>> {
    static S: OnceCell<Mutex<Option<State>>> = OnceCell::new();
    S.get_or_init(|| Mutex::new(None))
}

fn analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(tantivy_jieba::JiebaTokenizer::new())
        .filter(LowerCaser)
        .build()
}

fn build_schema() -> (Schema, Field, Field, Field) {
    let indexing = TextFieldIndexing::default()
        .set_tokenizer(TOKENIZER)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let analyzed = TextOptions::default().set_indexing_options(indexing);
    let mut b = Schema::builder();
    let id = b.add_text_field("id", STRING | STORED);
    let title = b.add_text_field("title", analyzed.clone());
    let tags = b.add_text_field("tags", analyzed);
    (b.build(), id, title, tags)
}

// PLACEHOLDER_LIB_SEARCH

fn rebuild() -> Result<(), String> {
    let version = store::library_version();
    let all = store::list_resources(None)?; // 最多 500 条，重建很快
    let (schema, id_f, title_f, tags_f) = build_schema();
    let index = Index::create_in_ram(schema);
    index.tokenizers().register(TOKENIZER, analyzer());
    let mut writer = index
        .writer(15_000_000)
        .map_err(|e| format!("创建索引写入器失败: {e}"))?;
    let mut rows = HashMap::new();
    let mut order = Vec::new();
    for row in &all {
        let id = row.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let title = row.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let tags = row.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let mut doc = TantivyDocument::default();
        doc.add_text(id_f, &id);
        doc.add_text(title_f, title);
        doc.add_text(tags_f, tags);
        writer.add_document(doc).map_err(|e| e.to_string())?;
        rows.insert(id.clone(), row.clone());
        order.push(id);
    }
    writer.commit().map_err(|e| e.to_string())?;
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|e| format!("创建索引读取器失败: {e}"))?;
    *state_cell().lock().unwrap() = Some(State {
        index,
        reader,
        id_field: id_f,
        title_field: title_f,
        tags_field: tags_f,
        version,
        rows,
        order,
    });
    Ok(())
}

fn ensure_fresh() -> Result<(), String> {
    let need = {
        let guard = state_cell().lock().unwrap();
        match guard.as_ref() {
            Some(s) => s.version != store::library_version(),
            None => true,
        }
    };
    if need {
        rebuild()?;
    }
    Ok(())
}

/// 资源库全文检索。空查询返回全部（按入库时间倒序）；否则 jieba 分词 + BM25。
pub fn search(query: &str, limit: usize) -> Result<Vec<serde_json::Value>, String> {
    ensure_fresh()?;
    let guard = state_cell().lock().unwrap();
    let st = guard.as_ref().ok_or("索引未就绪")?;
    let q = query.trim();
    if q.is_empty() {
        return Ok(st.order.iter().filter_map(|id| st.rows.get(id).cloned()).collect());
    }
    let searcher = st.reader.searcher();
    let mut parser = QueryParser::for_index(&st.index, vec![st.title_field, st.tags_field]);
    parser.set_conjunction_by_default();
    let parsed = match parser.parse_query(q) {
        Ok(p) => p,
        Err(_) => {
            // 语法出错：回退子串过滤。
            let ql = q.to_lowercase();
            return Ok(st
                .order
                .iter()
                .filter_map(|id| st.rows.get(id))
                .filter(|r| {
                    r.get("title").and_then(|v| v.as_str()).unwrap_or("").to_lowercase().contains(&ql)
                })
                .cloned()
                .collect());
        }
    };
    let top = searcher
        .search(&parsed, &TopDocs::with_limit(limit.max(20)).order_by_score())
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (_score, addr) in top {
        let doc: TantivyDocument = match searcher.doc(addr) {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Some(id) = doc.get_first(st.id_field).and_then(|v| v.as_str()) {
            if let Some(row) = st.rows.get(id) {
                out.push(row.clone());
            }
        }
    }
    Ok(out)
}

