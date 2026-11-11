//! 语言学习的离线词典包。
//!
//! 每个词典包是一个目录：`repos/guaxi_language_learning/dicts/<id>/`
//! - `manifest.json`：语言、来源、许可、条目数、安装时间
//! - `dict.sqlite3`：统一 schema，导入时把不同来源（ECDICT / CC-CEDICT / 用户 CSV）归一化
//!
//! 查询顺序：精确匹配 → 词形变化（lemmas）→ 前缀；只读打开，不锁住正在使用的包。
//! 目录下载放后台任务，前端轮询 `language_learning_dict_jobs`，避免 66MB 的 ECDICT
//! 卡住界面。

use futures_util::StreamExt;
use once_cell::sync::Lazy;
use reqwest::Client;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::io::AsyncWriteExt;
use tracing::info;
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

const LANG_LEARN_DIR: &str = "guaxi_language_learning";
const DICT_DIR: &str = "dicts";
const DICT_DB: &str = "dict.sqlite3";
const MANIFEST: &str = "manifest.json";

const ECDICT_URL: &str = "https://raw.githubusercontent.com/skywind3000/ECDICT/master/ecdict.csv";
const CEDICT_URL: &str = "https://www.mdbg.net/chinese/export/cedict/cedict_1_0_ts_utf-8_mdbg.zip";

static DICT_JOBS: Lazy<Mutex<HashMap<String, DictJob>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DictJob {
    pub id: String,
    pub stage: String,
    pub progress: f64,
    pub message: String,
    pub error: String,
    pub started_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictManifest {
    pub id: String,
    pub name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub version: String,
    pub license: String,
    pub description: String,
    pub entry_count: i64,
    pub installed_at: String,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictCatalogItem {
    pub id: String,
    pub name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub description: String,
    pub license: String,
    pub size_bytes: u64,
    pub version: String,
    pub installed: bool,
    pub installed_entry_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictEntry {
    pub term: String,
    pub phonetic: String,
    pub pos: String,
    pub meanings: Vec<String>,
    pub examples: Vec<String>,
    pub synonyms: Vec<String>,
    pub source_lang: String,
    pub target_lang: String,
    pub pack_id: String,
    pub pack_name: String,
}

struct CatalogSpec {
    id: &'static str,
    name: &'static str,
    source_lang: &'static str,
    target_lang: &'static str,
    description: &'static str,
    license: &'static str,
    size_bytes: u64,
    version: &'static str,
    format: &'static str,
    url: &'static str,
}

const CATALOG: &[CatalogSpec] = &[
    CatalogSpec {
        id: "ecdict",
        name: "ECDICT 英汉词典",
        source_lang: "en",
        target_lang: "zh",
        description: "76 万词条的英汉双解词典，含音标、词性、词形变化与考试标签。MIT 许可。",
        license: "MIT",
        size_bytes: 66_009_548,
        version: "1.0.28",
        format: "ecdict_csv",
        url: ECDICT_URL,
    },
    CatalogSpec {
        id: "cc-cedict",
        name: "CC-CEDICT 汉英词典",
        source_lang: "zh",
        target_lang: "en",
        description: "12 万条简繁中文词条，含拼音与英文释义。CC BY-SA 3.0。",
        license: "CC BY-SA 3.0",
        size_bytes: 3_983_590,
        version: "2025-11-30",
        format: "cedict_zip",
        url: CEDICT_URL,
    },
];

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn learning_root() -> Result<PathBuf, String> {
    let dir = crate::paths::repos_dir().join(LANG_LEARN_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建语言学习目录失败: {}", e))?;
    Ok(dir)
}

fn dict_root() -> Result<PathBuf, String> {
    let dir = learning_root()?.join(DICT_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建词典目录失败: {}", e))?;
    Ok(dir)
}

fn sanitize_id(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    let cleaned = out.trim_matches('_').to_string();
    if cleaned.is_empty() {
        format!("pack_{}", Uuid::new_v4())
    } else {
        cleaned
    }
}

fn normalize_lang(raw: &str, fallback: &str) -> String {
    let value = raw.trim().to_lowercase();
    if value.is_empty() || value == "auto" {
        fallback.to_string()
    } else {
        value
    }
}

fn normalize_term(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-' && c != ' ')
        .to_lowercase()
}

fn detect_lang(text: &str) -> &'static str {
    if text.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c)) {
        return "ja";
    }
    if text.chars().any(|c| ('\u{ac00}'..='\u{d7af}').contains(&c)) {
        return "ko";
    }
    if text.chars().any(|c| ('\u{0400}'..='\u{04ff}').contains(&c)) {
        return "ru";
    }
    if text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
        return "zh";
    }
    "en"
}

fn set_job(id: &str, stage: &str, progress: f64, message: &str, error: &str) {
    let key = id.trim().to_string();
    if key.is_empty() {
        return;
    }
    let now = now_iso();
    if let Ok(mut jobs) = DICT_JOBS.lock() {
        let job = jobs.entry(key.clone()).or_insert_with(|| DictJob {
            id: key,
            started_at: now.clone(),
            ..Default::default()
        });
        job.stage = stage.to_string();
        job.progress = progress.clamp(0.0, 100.0);
        job.message = message.to_string();
        job.error = error.to_string();
        job.updated_at = now;
    }
}

#[tauri::command]
pub fn language_learning_dict_jobs() -> Vec<DictJob> {
    DICT_JOBS
        .lock()
        .map(|jobs| jobs.values().cloned().collect())
        .unwrap_or_default()
}

fn read_manifest(dir: &Path) -> Option<DictManifest> {
    let text = fs::read_to_string(dir.join(MANIFEST)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_manifest(dir: &Path, manifest: &DictManifest) -> Result<(), String> {
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("序列化词典清单失败: {}", e))?;
    fs::write(dir.join(MANIFEST), text).map_err(|e| format!("写入词典清单失败: {}", e))
}

fn installed_packs() -> Vec<(DictManifest, PathBuf, u64)> {
    let root = match dict_root() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if let Some(manifest) = read_manifest(&dir) {
                let db_size = fs::metadata(dir.join(DICT_DB)).map(|m| m.len()).unwrap_or(0);
                out.push((manifest, dir, db_size));
            }
        }
    }
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    out
}

/// 语言码只比较主语言：`en-US` / `en_GB` 都应命中 `en` 词典，
/// 否则调用方带上区域码时会静默查不到任何包。
fn lang_key(raw: &str) -> String {
    raw.trim()
        .to_ascii_lowercase()
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_string()
}

fn lang_matches(a: &str, b: &str) -> bool {
    let a = lang_key(a);
    !a.is_empty() && a == lang_key(b)
}

fn pack_matches(manifest: &DictManifest, source: &str, target: &str) -> bool {
    let src_ok = source.is_empty() || source == "auto" || lang_matches(&manifest.source_lang, source);
    let dst_ok = target.is_empty() || target == "auto" || lang_matches(&manifest.target_lang, target);
    src_ok && dst_ok
}

fn open_readonly(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("打开离线词典失败: {}", e))
}

/// 查询列顺序：term, phonetic, pos, meanings_json, examples_json, synonyms_json。
fn row_to_entry(row: &rusqlite::Row<'_>, manifest: &DictManifest) -> rusqlite::Result<DictEntry> {
    let meanings_json: String = row.get(3)?;
    let examples_json: String = row.get(4)?;
    let synonyms_json: String = row.get(5)?;
    Ok(DictEntry {
        term: row.get(0)?,
        phonetic: row.get(1)?,
        pos: row.get(2)?,
        meanings: serde_json::from_str(&meanings_json).unwrap_or_default(),
        examples: serde_json::from_str(&examples_json).unwrap_or_default(),
        synonyms: serde_json::from_str(&synonyms_json).unwrap_or_default(),
        source_lang: manifest.source_lang.clone(),
        target_lang: manifest.target_lang.clone(),
        pack_id: manifest.id.clone(),
        pack_name: manifest.name.clone(),
    })
}

fn query_pack(db_path: &Path, manifest: &DictManifest, term: &str, limit: usize) -> Vec<DictEntry> {
    let normalized = normalize_term(term);
    if normalized.is_empty() {
        return Vec::new();
    }
    let conn = match open_readonly(db_path) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let exact_sql = "SELECT term, phonetic, pos, meanings_json, examples_json, synonyms_json \
                     FROM entries WHERE normalized = ?1 LIMIT ?2";
    if let Ok(mut stmt) = conn.prepare(exact_sql) {
        if let Ok(rows) = stmt.query_map(params![normalized, limit as i64], |row| {
            row_to_entry(row, manifest)
        }) {
            out.extend(rows.flatten());
        }
    }
    if !out.is_empty() {
        return out;
    }
    let lemma_sql = "SELECT headword FROM lemmas WHERE form = ?1 LIMIT ?2";
    let mut headwords = Vec::new();
    if let Ok(mut stmt) = conn.prepare(lemma_sql) {
        if let Ok(rows) = stmt.query_map(params![normalized, limit as i64], |row| {
            row.get::<_, String>(0)
        }) {
            headwords.extend(rows.flatten());
        }
    }
    for headword in headwords {
        let head_norm = normalize_term(&headword);
        if head_norm.is_empty() {
            continue;
        }
        if let Ok(mut stmt) = conn.prepare(exact_sql) {
            if let Ok(rows) = stmt.query_map(params![head_norm, limit as i64], |row| {
                row_to_entry(row, manifest)
            }) {
                out.extend(rows.flatten());
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    let prefix_sql = "SELECT term, phonetic, pos, meanings_json, examples_json, synonyms_json \
                      FROM entries WHERE normalized LIKE ?1 ORDER BY length(normalized) ASC LIMIT ?2";
    if let Ok(mut stmt) = conn.prepare(prefix_sql) {
        if let Ok(rows) = stmt.query_map(params![format!("{}%", normalized), limit as i64], |row| {
            row_to_entry(row, manifest)
        }) {
            out.extend(rows.flatten());
        }
    }
    out
}

pub fn lookup_entries(
    raw_term: &str,
    source_lang: &str,
    target_lang: &str,
    limit: usize,
) -> Vec<DictEntry> {
    let term = raw_term.trim();
    if term.is_empty() {
        return Vec::new();
    }
    let source = if source_lang.trim().is_empty() || source_lang == "auto" {
        detect_lang(term).to_string()
    } else {
        lang_key(source_lang)
    };
    let target = lang_key(&normalize_lang(target_lang, "zh"));
    let packs = installed_packs();
    let mut out = Vec::new();
    for (manifest, dir, _) in &packs {
        if !pack_matches(manifest, &source, &target) {
            continue;
        }
        let found = query_pack(&dir.join(DICT_DB), manifest, term, limit.saturating_sub(out.len()).max(1));
        out.extend(found);
        if out.len() >= limit {
            break;
        }
    }
    info!(
        term,
        source = source.as_str(),
        target = target.as_str(),
        packs = packs.len(),
        hits = out.len(),
        "offline dict lookup"
    );
    out
}

pub fn lookup_word(raw_term: &str, source_lang: &str, target_lang: &str) -> Option<DictEntry> {
    lookup_entries(raw_term, source_lang, target_lang, 1).into_iter().next()
}

#[tauri::command]
pub fn language_learning_dict_list() -> Vec<DictManifest> {
    installed_packs().into_iter().map(|(m, _, _)| m).collect()
}

#[tauri::command]
pub fn language_learning_dict_catalog() -> Vec<DictCatalogItem> {
    let installed: HashMap<String, DictManifest> = installed_packs()
        .into_iter()
        .map(|(m, _, _)| (m.id.clone(), m))
        .collect();
    CATALOG
        .iter()
        .map(|spec| {
            let hit = installed.get(spec.id);
            DictCatalogItem {
                id: spec.id.to_string(),
                name: spec.name.to_string(),
                source_lang: spec.source_lang.to_string(),
                target_lang: spec.target_lang.to_string(),
                description: spec.description.to_string(),
                license: spec.license.to_string(),
                size_bytes: spec.size_bytes,
                version: spec.version.to_string(),
                installed: hit.is_some(),
                installed_entry_count: hit.map(|m| m.entry_count).unwrap_or(0),
            }
        })
        .collect()
}

#[tauri::command]
pub fn language_learning_dict_lookup(
    word: String,
    source_lang: Option<String>,
    target_lang: Option<String>,
    limit: Option<u32>,
) -> Vec<DictEntry> {
    lookup_entries(
        &word,
        source_lang.as_deref().unwrap_or("auto"),
        target_lang.as_deref().unwrap_or("zh"),
        limit.unwrap_or(6).clamp(1, 20) as usize,
    )
}

fn proxy_client() -> Result<Client, String> {
    let config = crate::commands::resource_hunter::store::get_setting("proxy")
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str::<crate::commands::resource_hunter::model::ProxyConfig>(&value).ok())
        .unwrap_or_default();
    let proxy_url = crate::commands::resource_hunter::proxy::effective_proxy_url(&config);
    crate::commands::resource_hunter::proxy::build_client_raw(proxy_url.as_deref())
}

async fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    job_id: &str,
    expected_total: u64,
) -> Result<(), String> {
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(900))
        .send()
        .await
        .map_err(|e| format!("下载词典失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("下载词典失败，状态码: {}", response.status()));
    }
    let total = response.content_length().unwrap_or(expected_total);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建词典临时目录失败: {}", e))?;
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("创建词典文件失败: {}", e))?;
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取词典数据失败: {}", e))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| format!("写入词典文件失败: {}", e))?;
        written += bytes.len() as u64;
        let percent = if total > 0 {
            (written as f64 / total as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let message = if total > 0 {
            format!("下载中 {:.0}%", percent)
        } else {
            format!("下载中 {:.1} MB", written as f64 / 1_048_576.0)
        };
        set_job(job_id, "download", percent, &message, "");
    }
    file.flush().await.map_err(|e| format!("写入词典文件失败: {}", e))?;
    Ok(())
}

fn create_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = OFF;
        PRAGMA synchronous = OFF;
        PRAGMA temp_store = MEMORY;
        CREATE TABLE IF NOT EXISTS entries (
            id INTEGER PRIMARY KEY,
            term TEXT NOT NULL,
            normalized TEXT NOT NULL,
            phonetic TEXT NOT NULL DEFAULT '',
            pos TEXT NOT NULL DEFAULT '',
            meanings_json TEXT NOT NULL DEFAULT '[]',
            examples_json TEXT NOT NULL DEFAULT '[]',
            synonyms_json TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_entries_normalized ON entries(normalized);
        CREATE TABLE IF NOT EXISTS lemmas (
            form TEXT NOT NULL,
            headword TEXT NOT NULL,
            PRIMARY KEY(form, headword)
        );
        CREATE INDEX IF NOT EXISTS idx_lemmas_form ON lemmas(form);
        PRAGMA user_version = 1;
        "#,
    )
    .map_err(|e| format!("创建词典索引失败: {}", e))
}

struct EntryIn<'a> {
    term: &'a str,
    phonetic: &'a str,
    pos: &'a str,
    meanings: &'a [String],
    examples: &'a [String],
    synonyms: &'a [String],
}

fn insert_entry(tx: &rusqlite::Transaction<'_>, entry: &EntryIn<'_>) -> Result<(), String> {
    let normalized = normalize_term(entry.term);
    if normalized.is_empty() {
        return Ok(());
    }
    let meanings = serde_json::to_string(entry.meanings).unwrap_or_else(|_| "[]".to_string());
    let examples = serde_json::to_string(entry.examples).unwrap_or_else(|_| "[]".to_string());
    let synonyms = serde_json::to_string(entry.synonyms).unwrap_or_else(|_| "[]".to_string());
    tx.execute(
        "INSERT INTO entries(term, normalized, phonetic, pos, meanings_json, examples_json, synonyms_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.term.trim(),
            normalized,
            entry.phonetic.trim(),
            entry.pos.trim(),
            meanings,
            examples,
            synonyms
        ],
    )
    .map_err(|e| format!("写入词典条目失败: {}", e))?;
    Ok(())
}

fn add_lemma(tx: &rusqlite::Transaction<'_>, form: &str, headword: &str) {
    let form = normalize_term(form);
    let headword = headword.trim();
    if form.is_empty() || headword.is_empty() || form == normalize_term(headword) {
        return;
    }
    let _ = tx.execute(
        "INSERT OR IGNORE INTO lemmas(form, headword) VALUES (?1, ?2)",
        params![form, headword],
    );
}

fn split_meanings(raw: &str) -> Vec<String> {
    let expanded = raw.replace("\\n", "\n");
    expanded
        .split(|c| c == '\n' || c == '；' || c == ';' || c == '|')
        .map(|x| x.trim().trim_start_matches("[网络]").trim().to_string())
        .filter(|x| !x.is_empty() && x.chars().count() <= 180)
        .take(8)
        .collect()
}

fn parse_exchange(tx: &rusqlite::Transaction<'_>, word: &str, exchange: &str) {
    for part in exchange.split('/') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((kind, form)) = part.split_once(':') {
            let form = form.trim();
            if form.is_empty() {
                continue;
            }
            match kind {
                "0" | "1" => {
                    // 0/1 是原形/词根；把当前词挂到它下面。
                    add_lemma(tx, word, form);
                }
                "p" | "d" | "i" | "3" | "s" | "r" | "t" => {
                    add_lemma(tx, form, word);
                }
                _ => {}
            }
        }
    }
}

fn known_pos(token: &str) -> bool {
    let known = [
        "n.", "v.", "vt.", "vi.", "adj.", "adv.", "prep.", "pron.", "conj.", "num.",
        "art.", "int.", "aux.", "abbr.",
    ];
    known.iter().any(|p| token.eq_ignore_ascii_case(p))
}

fn infer_pos(text: &str) -> String {
    let first = text.split_whitespace().next().unwrap_or_default();
    if known_pos(first) {
        first.to_string()
    } else {
        String::new()
    }
}

fn strip_leading_pos(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(first) = trimmed.split_whitespace().next() {
        if known_pos(first) {
            return trimmed[first.len()..].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn import_ecdict(csv_path: &Path, db_path: &Path, job_id: &str) -> Result<i64, String> {
    let mut conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let file = fs::File::open(csv_path).map_err(|e| format!("读取 ECDICT 文件失败: {}", e))?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(BufReader::new(file));
    let headers = reader
        .headers()
        .map_err(|e| format!("读取 ECDICT 表头失败: {}", e))?
        .clone();
    let index = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let i_word = index("word").ok_or_else(|| "ECDICT 缺少 word 列".to_string())?;
    let i_phonetic = index("phonetic");
    let i_translation = index("translation");
    let i_pos = index("pos");
    let i_exchange = index("exchange");
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    let mut count = 0i64;
    for record in reader.records().flatten() {
        let word = record.get(i_word).unwrap_or_default().trim();
        if word.is_empty() {
            continue;
        }
        let phonetic = i_phonetic.and_then(|i| record.get(i)).unwrap_or_default();
        let translation = i_translation.and_then(|i| record.get(i)).unwrap_or_default();
        let pos_raw = i_pos.and_then(|i| record.get(i)).unwrap_or_default();
        let pos = if pos_raw.trim().is_empty() {
            infer_pos(translation)
        } else {
            pos_raw.trim().to_string()
        };
        let mut meanings = split_meanings(translation);
        if !pos.is_empty() {
            meanings = meanings
                .into_iter()
                .map(|meaning| strip_leading_pos(&meaning))
                .filter(|meaning| !meaning.is_empty())
                .collect();
        }
        if meanings.is_empty() {
            continue;
        }
        insert_entry(
            &tx,
            &EntryIn {
                term: word,
                phonetic,
                pos: &pos,
                meanings: &meanings,
                examples: &[],
                synonyms: &[],
            },
        )?;
        if let Some(exchange) = i_exchange.and_then(|i| record.get(i)) {
            parse_exchange(&tx, word, exchange);
        }
        count += 1;
        if count % 20_000 == 0 {
            let percent = (88.0 + (count as f64 / 800_000.0 * 11.0)).min(99.0);
            set_job(
                job_id,
                "import",
                percent,
                &format!("已导入 {} 条", count),
                "",
            );
        }
    }
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = conn.execute_batch("PRAGMA optimize;");
    Ok(count)
}

fn import_cedict(zip_path: &Path, db_path: &Path, job_id: &str) -> Result<i64, String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("读取 CC-CEDICT 压缩包失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解析 CC-CEDICT 压缩包失败: {}", e))?;
    let mut text = String::new();
    let mut found = false;
    // 先找文件名里带 cedict 的条目，找不到再退回第一个文本文件；
    // 压缩包里的 README.txt 排在前面时不能把 README 当词库。
    for want_cedict in [true, false] {
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("读取 CC-CEDICT 条目失败: {}", e))?;
            let name = entry.name().to_lowercase();
            let is_text = name.ends_with(".u8") || name.ends_with(".txt");
            if !is_text {
                continue;
            }
            if want_cedict && !name.contains("cedict") {
                continue;
            }
            entry
                .read_to_string(&mut text)
                .map_err(|e| format!("读取 CC-CEDICT 文本失败: {}", e))?;
            found = true;
            break;
        }
        if found {
            break;
        }
    }
    if !found {
        return Err("压缩包里没有找到 CC-CEDICT 文本".to_string());
    }
    let mut conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    let mut count = 0i64;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((head, rest)) = line.split_once(' ') else {
            continue;
        };
        let Some((simp, rest)) = rest.split_once(' ') else {
            continue;
        };
        let (pinyin, defs) = if let Some(start) = rest.find('[') {
            if let Some(end) = rest[start..].find(']') {
                let pinyin = rest[start + 1..start + end].trim().to_string();
                let defs = rest[start + end + 1..].trim();
                (pinyin, defs)
            } else {
                (String::new(), rest)
            }
        } else {
            (String::new(), rest)
        };
        let meanings: Vec<String> = defs
            .trim_matches('/')
            .split('/')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .take(10)
            .collect();
        if meanings.is_empty() {
            continue;
        }
        let phonetic = if pinyin.is_empty() {
            String::new()
        } else {
            format!("/{}/", pinyin)
        };
        let mut terms = vec![simp];
        if head != simp {
            terms.push(head);
        }
        for term in terms {
            if term.trim().is_empty() {
                continue;
            }
            insert_entry(
                &tx,
                &EntryIn {
                    term,
                    phonetic: &phonetic,
                    pos: "",
                    meanings: &meanings,
                    examples: &[],
                    synonyms: &[],
                },
            )?;
            count += 1;
        }
        if count % 10_000 == 0 {
            set_job(job_id, "import", 92.0, &format!("已导入 {} 条", count), "");
        }
    }
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = conn.execute_batch("PRAGMA optimize;");
    Ok(count)
}

fn import_csv(
    csv_path: &Path,
    db_path: &Path,
    source_lang: &str,
    target_lang: &str,
    job_id: &str,
) -> Result<i64, String> {
    let file = fs::File::open(csv_path).map_err(|e| format!("读取词典 CSV 失败: {}", e))?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(BufReader::new(file));
    let headers = reader
        .headers()
        .map_err(|e| format!("读取词典表头失败: {}", e))?
        .clone();
    let index = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let i_term = index("term")
        .or_else(|| index("word"))
        .ok_or_else(|| "CSV 至少需要 term 或 word 列".to_string())?;
    let i_phonetic = index("phonetic");
    let i_pos = index("pos");
    let i_meanings = index("meanings").or_else(|| index("translation"));
    let i_examples = index("examples");
    let i_synonyms = index("synonyms");
    let conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let mut conn = conn;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    let mut count = 0i64;
    for record in reader.records().flatten() {
        let term = record.get(i_term).unwrap_or_default().trim();
        if term.is_empty() {
            continue;
        }
        let meanings = i_meanings
            .and_then(|i| record.get(i))
            .map(split_meanings)
            .unwrap_or_default();
        if meanings.is_empty() {
            continue;
        }
        let examples: Vec<String> = i_examples
            .and_then(|i| record.get(i))
            .map(|x| x.split('|').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect())
            .unwrap_or_default();
        let synonyms: Vec<String> = i_synonyms
            .and_then(|i| record.get(i))
            .map(|x| x.split('|').map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect())
            .unwrap_or_default();
        insert_entry(
            &tx,
            &EntryIn {
                term,
                phonetic: i_phonetic.and_then(|i| record.get(i)).unwrap_or_default(),
                pos: i_pos.and_then(|i| record.get(i)).unwrap_or_default(),
                meanings: &meanings,
                examples: &examples,
                synonyms: &synonyms,
            },
        )?;
        count += 1;
        if count % 10_000 == 0 {
            set_job(job_id, "import", 90.0, &format!("已导入 {} 条", count), "");
        }
    }
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = (source_lang, target_lang);
    Ok(count)
}

fn import_json(
    json_path: &Path,
    db_path: &Path,
    job_id: &str,
) -> Result<i64, String> {
    let text = fs::read_to_string(json_path).map_err(|e| format!("读取词典 JSON 失败: {}", e))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("解析词典 JSON 失败: {}", e))?;
    let rows = value.as_array().ok_or_else(|| "词典 JSON 需要是数组".to_string())?;
    let conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let mut conn = conn;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    let mut count = 0i64;
    for row in rows {
        let term = row
            .get("term")
            .or_else(|| row.get("word"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if term.trim().is_empty() {
            continue;
        }
        let mut meanings: Vec<String> = row
            .get("meanings")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|v| v.trim().to_string()).collect())
            .unwrap_or_default();
        if meanings.is_empty() {
            if let Some(text) = row.get("translation").and_then(|v| v.as_str()) {
                meanings = split_meanings(text);
            }
        }
        if meanings.is_empty() {
            continue;
        }
        let examples: Vec<String> = row
            .get("examples")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|v| v.trim().to_string()).collect())
            .unwrap_or_default();
        let synonyms: Vec<String> = row
            .get("synonyms")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|v| v.trim().to_string()).collect())
            .unwrap_or_default();
        insert_entry(
            &tx,
            &EntryIn {
                term,
                phonetic: row.get("phonetic").and_then(|v| v.as_str()).unwrap_or_default(),
                pos: row.get("pos").and_then(|v| v.as_str()).unwrap_or_default(),
                meanings: &meanings,
                examples: &examples,
                synonyms: &synonyms,
            },
        )?;
        count += 1;
        if count % 10_000 == 0 {
            set_job(job_id, "import", 90.0, &format!("已导入 {} 条", count), "");
        }
    }
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    Ok(count)
}

async fn install_catalog(spec: &'static CatalogSpec, job_id: &str) -> Result<DictManifest, String> {
    let root = dict_root()?;
    let final_dir = root.join(spec.id);
    let temp_dir = root.join(format!(".tmp-{}", spec.id));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).map_err(|e| format!("创建词典目录失败: {}", e))?;
    let download_path = temp_dir.join(if spec.format == "cedict_zip" {
        "source.zip"
    } else {
        "source.csv"
    });
    set_job(job_id, "download", 0.0, "准备下载", "");
    let client = proxy_client()?;
    download_file(
        &client,
        spec.url,
        &download_path,
        job_id,
        spec.size_bytes,
    )
    .await?;
    set_job(job_id, "import", 88.0, "正在建立词典索引", "");
    let db_path = temp_dir.join(DICT_DB);
    let format = spec.format.to_string();
    let import_download = download_path.clone();
    let import_db = db_path.clone();
    let import_job = job_id.to_string();
    let entry_count = tauri::async_runtime::spawn_blocking(move || {
        if format == "cedict_zip" {
            import_cedict(&import_download, &import_db, &import_job)
        } else {
            import_ecdict(&import_download, &import_db, &import_job)
        }
    })
    .await
    .map_err(|e| format!("词典索引导入任务异常: {}", e))??;
    let manifest = DictManifest {
        id: spec.id.to_string(),
        name: spec.name.to_string(),
        source_lang: spec.source_lang.to_string(),
        target_lang: spec.target_lang.to_string(),
        version: spec.version.to_string(),
        license: spec.license.to_string(),
        description: spec.description.to_string(),
        entry_count,
        installed_at: now_iso(),
        builtin: true,
    };
    write_manifest(&temp_dir, &manifest)?;
    let _ = fs::remove_file(&download_path);
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir).map_err(|e| format!("替换旧词典失败: {}", e))?;
    }
    fs::rename(&temp_dir, &final_dir).map_err(|e| format!("安装词典失败: {}", e))?;
    Ok(manifest)
}

#[tauri::command]
pub fn language_learning_dict_install(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _ = app;
    let spec = CATALOG
        .iter()
        .find(|spec| spec.id == id.trim())
        .ok_or_else(|| "未知的词典".to_string())?;
    let job_id = spec.id.to_string();
    {
        let jobs = DICT_JOBS.lock().map_err(|_| "词典任务状态异常".to_string())?;
        if let Some(job) = jobs.get(&job_id) {
            if !job.stage.is_empty() && job.stage != "done" && job.stage != "error" {
                return Ok(());
            }
        }
    }
    set_job(&job_id, "download", 0.0, "准备下载", "");
    tauri::async_runtime::spawn(async move {
        let spec = CATALOG.iter().find(|s| s.id == job_id.as_str());
        let Some(spec) = spec else {
            set_job(&job_id, "error", 0.0, "词典不存在", "未知的词典");
            return;
        };
        match install_catalog(spec, &job_id).await {
            Ok(manifest) => set_job(
                &job_id,
                "done",
                100.0,
                &format!("已安装 {} 条", manifest.entry_count),
                "",
            ),
            Err(error) => set_job(&job_id, "error", 0.0, "安装失败", &error),
        }
    });
    Ok(())
}

#[tauri::command]
pub fn language_learning_dict_remove(id: String) -> Result<(), String> {
    let safe = sanitize_id(&id);
    if safe != id.trim().to_lowercase() {
        return Err("词典 ID 不合法".to_string());
    }
    let dir = dict_root()?.join(safe);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("删除词典失败: {}", e))?;
    }
    Ok(())
}

fn normalized_manifest_from_zip(
    zip_path: &Path,
    name: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<(DictManifest, String, String), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("读取词典包失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解析词典包失败: {}", e))?;
    let mut manifest: Option<DictManifest> = None;
    let mut csv_name = String::new();
    let mut json_name = String::new();
    let mut cedict_text = String::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取词典包条目失败: {}", e))?;
        let entry_name = entry.name().to_string();
        let lower = entry_name.to_lowercase();
        if lower.ends_with(MANIFEST) || lower.ends_with("manifest.json") {
            let mut text = String::new();
            entry.read_to_string(&mut text).ok();
            manifest = serde_json::from_str(&text).ok();
        } else if lower.ends_with(".csv") {
            if csv_name.is_empty() {
                csv_name = entry_name;
            }
        } else if lower.ends_with(".json") {
            if json_name.is_empty() {
                json_name = entry_name;
            }
        } else if (lower.ends_with(".u8") || lower.ends_with(".txt"))
            && lower.contains("cedict")
        {
            entry.read_to_string(&mut cedict_text).ok();
        }
    }
    let id = format!("{}_{}", sanitize_id(if name.trim().is_empty() { "imported" } else { name }), Uuid::new_v4());
    let manifest = manifest.unwrap_or(DictManifest {
        id: id.clone(),
        name: if name.trim().is_empty() { "导入词典".to_string() } else { name.trim().to_string() },
        source_lang: normalize_lang(source_lang, "en"),
        target_lang: normalize_lang(target_lang, "zh"),
        version: "imported".to_string(),
        license: "用户导入".to_string(),
        description: "用户导入的离线词典包".to_string(),
        entry_count: 0,
        installed_at: now_iso(),
        builtin: false,
    });
    let mut manifest = manifest;
    manifest.id = id;
    manifest.builtin = false;
    if !name.trim().is_empty() {
        manifest.name = name.trim().to_string();
    }
    let csv_member = if csv_name.is_empty() && json_name.is_empty() && !cedict_text.is_empty() {
        "__cedict__".to_string()
    } else {
        csv_name
    };
    Ok((manifest, csv_member, json_name))
}

fn extract_zip_member(zip_path: &Path, member: &str) -> Result<PathBuf, String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("读取词典包失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解析词典包失败: {}", e))?;
    let entry = archive
        .by_name(member)
        .map_err(|e| format!("词典包里找不到 {}: {}", member, e))?;
    let temp = std::env::temp_dir().join(format!("guaxi_dict_{}", Uuid::new_v4()));
    let mut out = fs::File::create(&temp).map_err(|e| format!("解压词典失败: {}", e))?;
    let mut reader = BufReader::new(entry);
    std::io::copy(&mut reader, &mut out).map_err(|e| format!("解压词典失败: {}", e))?;
    Ok(temp)
}

#[tauri::command]
pub async fn language_learning_dict_import(
    path: String,
    name: Option<String>,
    source_lang: Option<String>,
    target_lang: Option<String>,
) -> Result<DictManifest, String> {
    let input = PathBuf::from(path.trim());
    if !input.is_file() {
        return Err("要导入的词典文件不存在".to_string());
    }
    let display_name = name.unwrap_or_default();
    let src = normalize_lang(source_lang.as_deref().unwrap_or("en"), "en");
    let dst = normalize_lang(target_lang.as_deref().unwrap_or("zh"), "zh");
    tauri::async_runtime::spawn_blocking(move || {
        let root = dict_root()?;
        let mut manifest = DictManifest {
            id: format!("{}_{}", sanitize_id(if display_name.trim().is_empty() { "imported" } else { &display_name }), Uuid::new_v4()),
            name: if display_name.trim().is_empty() { "导入词典".to_string() } else { display_name.trim().to_string() },
            source_lang: src,
            target_lang: dst,
            version: "imported".to_string(),
            license: "用户导入".to_string(),
            description: "用户导入的离线词典包".to_string(),
            entry_count: 0,
            installed_at: now_iso(),
            builtin: false,
        };
        let temp_dir = root.join(format!(".tmp-import-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).map_err(|e| format!("创建词典目录失败: {}", e))?;
        let db_path = temp_dir.join(DICT_DB);
        let ext = input
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let job_id = format!("import-{}", manifest.id);
        let count = if ext == "zip" {
            let (zip_manifest, csv_member, json_member) =
                normalized_manifest_from_zip(&input, &manifest.name, &manifest.source_lang, &manifest.target_lang)?;
            manifest = zip_manifest;
            if csv_member == "__cedict__" {
                // 直接导入官方 CC-CEDICT 压缩包时，方向固定是 中→英。
                manifest.source_lang = "zh".to_string();
                manifest.target_lang = "en".to_string();
                // 直接解析 zip 内的 CC-CEDICT
                import_cedict(&input, &db_path, &job_id)?
            } else if !csv_member.is_empty() {
                let temp_csv = extract_zip_member(&input, &csv_member)?;
                let count = import_csv(&temp_csv, &db_path, &manifest.source_lang, &manifest.target_lang, &job_id)?;
                let _ = fs::remove_file(temp_csv);
                count
            } else if !json_member.is_empty() {
                let temp_json = extract_zip_member(&input, &json_member)?;
                let count = import_json(&temp_json, &db_path, &job_id)?;
                let _ = fs::remove_file(temp_json);
                count
            } else {
                return Err("词典包里没有可导入的 CSV/JSON".to_string());
            }
        } else if ext == "json" {
            import_json(&input, &db_path, &job_id)?
        } else if ext == "csv" {
            import_csv(&input, &db_path, &manifest.source_lang, &manifest.target_lang, &job_id)?
        } else {
            return Err("仅支持 .zip / .csv / .json 词典包".to_string());
        };
        manifest.entry_count = count;
        manifest.installed_at = now_iso();
        write_manifest(&temp_dir, &manifest)?;
        let final_dir = root.join(&manifest.id);
        if final_dir.exists() {
            fs::remove_dir_all(&final_dir).map_err(|e| format!("替换旧词典失败: {}", e))?;
        }
        fs::rename(&temp_dir, &final_dir).map_err(|e| format!("安装词典失败: {}", e))?;
        Ok(manifest)
    })
    .await
    .map_err(|e| format!("导入词典任务异常: {}", e))?
}

#[tauri::command]
pub async fn language_learning_dict_export(id: String, output_path: String) -> Result<String, String> {
    let target = PathBuf::from(output_path.trim());
    tauri::async_runtime::spawn_blocking(move || {
        let safe = sanitize_id(&id);
        let dir = dict_root()?.join(&safe);
        let manifest = read_manifest(&dir).ok_or_else(|| "词典不存在".to_string())?;
        let db_path = dir.join(DICT_DB);
        let conn = open_readonly(&db_path)?;
        let file = fs::File::create(&target).map_err(|e| format!("创建导出文件失败: {}", e))?;
        let mut zip = ZipWriter::new(file);
        let options: FileOptions<'_, ()> = FileOptions::default();
        zip.start_file(MANIFEST, options)
            .map_err(|e| format!("写入词典清单失败: {}", e))?;
        zip.write_all(serde_json::to_string_pretty(&manifest).unwrap_or_default().as_bytes())
            .map_err(|e| format!("写入词典清单失败: {}", e))?;
        zip.start_file("entries.csv", options)
            .map_err(|e| format!("写入词典条目失败: {}", e))?;
        zip.write_all(b"term,phonetic,pos,meanings,examples,synonyms\n")
            .map_err(|e| format!("写入词典条目失败: {}", e))?;
        let mut stmt = conn
            .prepare("SELECT term, phonetic, pos, meanings_json, examples_json, synonyms_json FROM entries ORDER BY id")
            .map_err(|e| format!("读取词典条目失败: {}", e))?;
        let mut rows = stmt.query([]).map_err(|e| format!("读取词典条目失败: {}", e))?;
        while let Some(row) = rows.next().map_err(|e| format!("读取词典条目失败: {}", e))? {
            let term: String = row.get(0).unwrap_or_default();
            let phonetic: String = row.get(1).unwrap_or_default();
            let pos: String = row.get(2).unwrap_or_default();
            let meanings: Vec<String> = serde_json::from_str(&row.get::<_, String>(3).unwrap_or_default()).unwrap_or_default();
            let examples: Vec<String> = serde_json::from_str(&row.get::<_, String>(4).unwrap_or_default()).unwrap_or_default();
            let synonyms: Vec<String> = serde_json::from_str(&row.get::<_, String>(5).unwrap_or_default()).unwrap_or_default();
            let line = format!(
                "{},{},{},{},{},{}\n",
                csv_escape(&term),
                csv_escape(&phonetic),
                csv_escape(&pos),
                csv_escape(&meanings.join("|")),
                csv_escape(&examples.join("|")),
                csv_escape(&synonyms.join("|"))
            );
            zip.write_all(line.as_bytes())
                .map_err(|e| format!("写入词典条目失败: {}", e))?;
        }
        zip.finish().map_err(|e| format!("完成词典导出失败: {}", e))?;
        Ok(target.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("导出词典任务异常: {}", e))?
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
