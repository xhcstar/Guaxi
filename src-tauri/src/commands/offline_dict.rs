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
use std::env;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
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
    /// 来源格式：mdx / stardict / dsl / wiktionary / csv / json / ecdict_csv / cedict_zip
    #[serde(default)]
    pub format: String,
    /// 查词时是否参与；老清单没有这个字段，默认开启。
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 越大越靠前；同优先级按名称排序。
    #[serde(default)]
    pub priority: i64,
    /// 是否带 MDD 解包出来的图片/样式资源。
    #[serde(default)]
    pub has_resources: bool,
    #[serde(default)]
    pub resource_bytes: u64,
}

fn default_true() -> bool {
    true
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
    /// 富文本释义（MDict / StarDict / DSL 才有），已把资源引用替换成 data URI。
    #[serde(default)]
    pub definition_html: String,
    #[serde(default)]
    pub format: String,
}

/// 按词典分组的查词结果：词典应用要像欧路那样一本一本列出来。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictPackHits {
    pub pack_id: String,
    pub pack_name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub format: String,
    pub entry_count: i64,
    pub entries: Vec<DictEntry>,
}

/// 词条浏览 / 模糊搜索的轻量行。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DictBrowserRow {
    pub term: String,
    pub phonetic: String,
    pub pos: String,
    pub preview: String,
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
    CatalogSpec {
        id: "zhwiktionary-ja",
        name: "维基日汉词典",
        source_lang: "ja",
        target_lang: "zh",
        description: "取自中文维基词典的日语词条，简体中文释义，含词性、假名注音、同义词与例句。CC BY-SA。",
        license: "CC BY-SA 3.0",
        size_bytes: 90_547_237,
        version: "zhwiktionary",
        format: "wiktionary_jsonl",
        url: "https://kaikki.org/zhwiktionary/%E6%97%A5%E8%AA%9E/kaikki.org-dictionary-%E6%97%A5%E8%AA%9E.jsonl",
    },
    CatalogSpec {
        id: "zhwiktionary-ko",
        name: "维基韩汉词典",
        source_lang: "ko",
        target_lang: "zh",
        description: "取自中文维基词典的朝鲜语词条，简体中文释义，含词性与例句。CC BY-SA。",
        license: "CC BY-SA 3.0",
        size_bytes: 31_578_800,
        version: "zhwiktionary",
        format: "wiktionary_jsonl",
        url: "https://kaikki.org/zhwiktionary/%E6%9C%9D%E9%AE%AE%E8%AA%9E/kaikki.org-dictionary-%E6%9C%9D%E9%AE%AE%E8%AA%9E.jsonl",
    },
    CatalogSpec {
        id: "zhwiktionary-fr",
        name: "维基法汉词典",
        source_lang: "fr",
        target_lang: "zh",
        description: "取自中文维基词典的法语词条，简体中文释义，含词性、音标与例句。CC BY-SA。",
        license: "CC BY-SA 3.0",
        size_bytes: 79_830_943,
        version: "zhwiktionary",
        format: "wiktionary_jsonl",
        url: "https://kaikki.org/zhwiktionary/%E6%B3%95%E8%AA%9E/kaikki.org-dictionary-%E6%B3%95%E8%AA%9E.jsonl",
    },
    CatalogSpec {
        id: "zhwiktionary-de",
        name: "维基德汉词典",
        source_lang: "de",
        target_lang: "zh",
        description: "取自中文维基词典的德语词条，简体中文释义，含词性、音标与例句。CC BY-SA。",
        license: "CC BY-SA 3.0",
        size_bytes: 116_514_382,
        version: "zhwiktionary",
        format: "wiktionary_jsonl",
        url: "https://kaikki.org/zhwiktionary/%E5%BE%B7%E8%AA%9E/kaikki.org-dictionary-%E5%BE%B7%E8%AA%9E.jsonl",
    },
    CatalogSpec {
        id: "zhwiktionary-es",
        name: "维基西汉词典",
        source_lang: "es",
        target_lang: "zh",
        description: "取自中文维基词典的西班牙语词条，简体中文释义，含词性、音标与例句。CC BY-SA。",
        license: "CC BY-SA 3.0",
        size_bytes: 119_562_856,
        version: "zhwiktionary",
        format: "wiktionary_jsonl",
        url: "https://kaikki.org/zhwiktionary/%E8%A5%BF%E7%8F%AD%E7%89%99%E8%AA%9E/kaikki.org-dictionary-%E8%A5%BF%E7%8F%AD%E7%89%99%E8%AA%9E.jsonl",
    },
    CatalogSpec {
        id: "zhwiktionary-ru",
        name: "维基俄汉词典",
        source_lang: "ru",
        target_lang: "zh",
        description: "取自中文维基词典的俄语词条，简体中文释义，含词性、音标与例句。CC BY-SA。",
        license: "CC BY-SA 3.0",
        size_bytes: 97_303_085,
        version: "zhwiktionary",
        format: "wiktionary_jsonl",
        url: "https://kaikki.org/zhwiktionary/%E4%BF%84%E8%AA%9E/kaikki.org-dictionary-%E4%BF%84%E8%AA%9E.jsonl",
    },
];

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn learning_root() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "无法确定可执行文件目录".to_string())?
        .join("repos")
        .join(LANG_LEARN_DIR);
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
    // 先按用户排的优先级，再按名称稳定排序。
    out.sort_by(|a, b| {
        b.0.priority
            .cmp(&a.0.priority)
            .then_with(|| a.0.name.cmp(&b.0.name))
    });
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
        definition_html: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
        format: manifest.format.clone(),
    })
}

const RES_DIR: &str = "resources";

/// 查词结果里的图片/样式引用统一换成 data URI：词典窗口没有自定义协议，
/// 也不想为了几张图标去开 asset scope。
fn query_pack(db_path: &Path, manifest: &DictManifest, term: &str, limit: usize) -> Vec<DictEntry> {
    let mut rows = query_pack_raw(db_path, manifest, term, limit);
    if manifest.has_resources && !rows.is_empty() {
        if let Some(dir) = db_path.parent() {
            let res_dir = dir.join(RES_DIR);
            for row in rows.iter_mut() {
                if !row.definition_html.is_empty() {
                    row.definition_html =
                        crate::commands::dict_formats::resolve_resources(&row.definition_html, &res_dir);
                }
            }
        }
    }
    rows
}

fn query_pack_raw(db_path: &Path, manifest: &DictManifest, term: &str, limit: usize) -> Vec<DictEntry> {
    let normalized = normalize_term(term);
    if normalized.is_empty() {
        return Vec::new();
    }
    ensure_entry_columns(db_path);
    let conn = match open_readonly(db_path) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let exact_sql = "SELECT term, phonetic, pos, meanings_json, examples_json, synonyms_json, definition_html \
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
    let prefix_sql = "SELECT term, phonetic, pos, meanings_json, examples_json, synonyms_json, definition_html \
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
        if !manifest.enabled {
            continue;
        }
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

/// 下载中的临时文件名：先写 `xxx.part`，完整收到后才改名。
/// 中途退出 / 断网留下的半截文件下次能接着传，不会从 0 重来。
fn partial_download_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    dest.with_file_name(format!("{}.part", name))
}

/// 单块读超时：只要还有数据在流动就一直传下去。
/// 不能用「整请求超时」——一本 1GB 的词典在慢速代理下几分钟都传不完，
/// 会被 reqwest 直接判超时，表现就是「下载到一半被打断」。
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

async fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    job_id: &str,
    expected_total: u64,
) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建词典临时目录失败: {}", e))?;
    }
    let part = partial_download_path(dest);
    let mut start_at = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let mut attempt = 0;

    loop {
        attempt += 1;
        let mut request = client.get(url);
        if start_at > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={}-", start_at));
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("下载词典失败: {}", e))?;
        let status = response.status();

        // 服务器把上次没下完的当成完整文件了（416），说明 .part 已经够用或已失效，
        // 直接删掉重来一次。
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && start_at > 0 {
            let _ = fs::remove_file(&part);
            start_at = 0;
            if attempt < 3 {
                continue;
            }
            return Err("下载词典失败：服务器拒绝续传".to_string());
        }
        if !status.is_success() {
            return Err(format!("下载词典失败，状态码: {}", status));
        }

        let resumed = start_at > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if start_at > 0 && !resumed {
            // 服务器忽略 Range，只能从头写。
            start_at = 0;
        }
        let body_len = response.content_length().unwrap_or(0);
        let total = if resumed { start_at + body_len } else { body_len };
        let total = if total == 0 { expected_total } else { total };

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(&part)
            .await
            .map_err(|e| format!("创建词典文件失败: {}", e))?;

        let mut written = start_at;
        let mut stream = response.bytes_stream();
        let mut last_report = std::time::Instant::now() - Duration::from_secs(1);
        loop {
            let next = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, stream.next()).await;
            let chunk = match next {
                Err(_) => {
                    // 保留 .part，下次继续。
                    return Err(format!(
                        "下载超时（{} 秒无数据），已保留已下载的 {:.1} MB，可稍后重试继续",
                        DOWNLOAD_IDLE_TIMEOUT.as_secs(),
                        written as f64 / 1_048_576.0
                    ));
                }
                Ok(None) => break,
                Ok(Some(chunk)) => chunk.map_err(|e| format!("读取词典数据失败: {}", e))?,
            };
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("写入词典文件失败: {}", e))?;
            written += chunk.len() as u64;
            // 每 300ms 才刷一次进度，几十万次 set_job 会白白抢锁。
            if last_report.elapsed() >= Duration::from_millis(300) {
                last_report = std::time::Instant::now();
                let percent = if total > 0 {
                    (written as f64 / total as f64 * 100.0).min(99.9)
                } else {
                    0.0
                };
                let message = if total > 0 {
                    format!(
                        "{} {:.0}% · {:.1}/{:.1} MB",
                        if resumed { "续传中" } else { "下载中" },
                        percent,
                        written as f64 / 1_048_576.0,
                        total as f64 / 1_048_576.0
                    )
                } else {
                    format!("下载中 {:.1} MB", written as f64 / 1_048_576.0)
                };
                set_job(job_id, "download", percent, &message, "");
            }
        }
        file.flush().await.map_err(|e| format!("写入词典文件失败: {}", e))?;

        if total > 0 && written < total {
            return Err(format!(
                "下载不完整（{:.1}/{:.1} MB），已保留临时文件，下次会自动续传",
                written as f64 / 1_048_576.0,
                total as f64 / 1_048_576.0
            ));
        }
        fs::rename(&part, dest).map_err(|e| format!("保存词典文件失败: {}", e))?;
        return Ok(());
    }
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
            synonyms_json TEXT NOT NULL DEFAULT '[]',
            definition_html TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_entries_normalized ON entries(normalized);
        CREATE TABLE IF NOT EXISTS lemmas (
            form TEXT NOT NULL,
            headword TEXT NOT NULL,
            PRIMARY KEY(form, headword)
        );
        CREATE INDEX IF NOT EXISTS idx_lemmas_form ON lemmas(form);
        PRAGMA user_version = 2;
        "#,
    )
    .map_err(|e| format!("创建词典索引失败: {}", e))
}

/// 老版本装好的词库（ECDICT / CC-CEDICT）没有 `definition_html` 列。
/// 查询前补一列，避免升级后老词典整本查不出字。
fn ensure_entry_columns(db_path: &Path) {
    static UPGRADED: Lazy<Mutex<std::collections::HashSet<PathBuf>>> =
        Lazy::new(|| Mutex::new(std::collections::HashSet::new()));
    let key = db_path.to_path_buf();
    if let Ok(set) = UPGRADED.lock() {
        if set.contains(&key) {
            return;
        }
    }
    if let Ok(conn) = Connection::open(db_path) {
        let has_column = conn
            .prepare("PRAGMA table_info(entries)")
            .ok()
            .and_then(|mut stmt| {
                let rows = stmt
                    .query_map([], |row| row.get::<_, String>(1))
                    .ok()?;
                Some(rows.flatten().any(|name| name == "definition_html"))
            })
            .unwrap_or(true);
        if !has_column {
            let _ = conn.execute_batch(
                "ALTER TABLE entries ADD COLUMN definition_html TEXT NOT NULL DEFAULT '';",
            );
        }
    }
    if let Ok(mut set) = UPGRADED.lock() {
        set.insert(key);
    }
}

struct EntryIn<'a> {
    term: &'a str,
    phonetic: &'a str,
    pos: &'a str,
    meanings: &'a [String],
    examples: &'a [String],
    synonyms: &'a [String],
    definition: &'a str,
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
        "INSERT INTO entries(term, normalized, phonetic, pos, meanings_json, examples_json, synonyms_json, definition_html) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            entry.term.trim(),
            normalized,
            entry.phonetic.trim(),
            entry.pos.trim(),
            meanings,
            examples,
            synonyms,
            entry.definition
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
                definition: "",
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
                    definition: "",
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
                definition: "",
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
                definition: "",
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
    let download_path = temp_dir.join(match spec.format {
        "cedict_zip" => "source.zip",
        "wiktionary_jsonl" => "source.jsonl",
        _ => "source.csv",
    });
    // 已经下完的文件（只有完整收到才会改名成正式文件名）直接复用：
    // 上次若是「建索引」阶段失败，不必再把上百 MB 重下一遍。
    let downloaded = fs::metadata(&download_path)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false);
    // 有半截文件就留着续传，别把已经下了几百 MB 的进度抹掉。
    let partial = partial_download_path(&download_path);
    let resumable = fs::metadata(&partial)
        .map(|meta| meta.len() > 0)
        .unwrap_or(false);
    if !downloaded && !resumable {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).map_err(|e| format!("创建词典目录失败: {}", e))?;
    if downloaded {
        set_job(job_id, "import", 80.0, "已下载过，直接建立索引", "");
    } else {
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
    }
    set_job(job_id, "import", 88.0, "正在建立词典索引", "");
    let db_path = temp_dir.join(DICT_DB);
    let format = spec.format.to_string();
    let import_download = download_path.clone();
    let import_db = db_path.clone();
    let import_job = job_id.to_string();
    let entry_count = tauri::async_runtime::spawn_blocking(move || {
        if format == "cedict_zip" {
            import_cedict(&import_download, &import_db, &import_job)
        } else if format == "wiktionary_jsonl" {
            import_wiktionary_into(&import_download, &import_db, &import_job)
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
        format: spec.format.to_string(),
        enabled: true,
        priority: 0,
        has_resources: false,
        resource_bytes: 0,
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
        format: "zip".to_string(),
        enabled: true,
        priority: 0,
        has_resources: false,
        resource_bytes: 0,
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

fn format_label(format: &str) -> &'static str {
    match format {
        "mdx" => "MDict 词库",
        "stardict" => "StarDict 词库",
        "dsl" => "ABBYY DSL 词库",
        "wiktionary" => "Wiktionary 词库",
        "ecdict_csv" => "ECDICT 词库",
        "cedict_zip" => "CC-CEDICT 词库",
        "zip" => "词典包",
        "csv" => "CSV 词库",
        "json" => "JSON 词库",
        _ => "离线词库",
    }
}

fn import_mdx_into(mdx: &Path, db_path: &Path, job_id: &str) -> Result<i64, String> {
    let mut conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    let mut count = 0i64;
    let mut scanned = 0i64;
    set_job(job_id, "import", 55.0, "正在解析 MDX 词条", "");
    let result = crate::commands::dict_formats::parse_mdx(mdx, |entry| {
        scanned += 1;
        if scanned % 5000 == 0 {
            let percent = 55.0 + (scanned as f64 / 500_000.0 * 42.0).min(42.0);
            set_job(job_id, "import", percent, &format!("已解析 {} 条", scanned), "");
        }
        if let Some(target) = entry.link.as_deref() {
            add_lemma(&tx, &entry.term, target);
            return Ok(());
        }
        insert_entry(
            &tx,
            &EntryIn {
                term: &entry.term,
                phonetic: &entry.phonetic,
                pos: &entry.pos,
                meanings: &entry.meanings,
                examples: &entry.examples,
                synonyms: &entry.synonyms,
                definition: &entry.definition,
            },
        )?;
        count += 1;
        Ok(())
    });
    result?;
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = conn.execute_batch("PRAGMA optimize;");
    Ok(count)
}

fn import_stardict_into(ifo: &Path, db_path: &Path, job_id: &str) -> Result<(i64, String), String> {
    let mut conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    set_job(job_id, "import", 55.0, "正在解析 StarDict 词库", "");
    let mut count = 0i64;
    let (parsed, name, _seq) = crate::commands::dict_formats::parse_stardict(ifo, |entry| {
        insert_entry(
            &tx,
            &EntryIn {
                term: &entry.term,
                phonetic: &entry.phonetic,
                pos: &entry.pos,
                meanings: &entry.meanings,
                examples: &entry.examples,
                synonyms: &entry.synonyms,
                definition: &entry.definition,
            },
        )?;
        count += 1;
        if count % 10_000 == 0 {
            set_job(job_id, "import", 80.0, &format!("已导入 {} 条", count), "");
        }
        Ok(())
    })?;
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = conn.execute_batch("PRAGMA optimize;");
    Ok((parsed, name))
}

fn import_dsl_into(dsl: &Path, db_path: &Path, job_id: &str) -> Result<(i64, String, String, String), String> {
    let mut conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    set_job(job_id, "import", 55.0, "正在解析 DSL 词库", "");
    let mut count = 0i64;
    let (parsed, name, src, dst) = crate::commands::dict_formats::parse_dsl(dsl, |entry| {
        insert_entry(
            &tx,
            &EntryIn {
                term: &entry.term,
                phonetic: &entry.phonetic,
                pos: &entry.pos,
                meanings: &entry.meanings,
                examples: &entry.examples,
                synonyms: &entry.synonyms,
                definition: &entry.definition,
            },
        )?;
        count += 1;
        if count % 10_000 == 0 {
            set_job(job_id, "import", 80.0, &format!("已导入 {} 条", count), "");
        }
        Ok(())
    })?;
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = conn.execute_batch("PRAGMA optimize;");
    Ok((parsed, name, src, dst))
}

fn import_wiktionary_into(jsonl: &Path, db_path: &Path, job_id: &str) -> Result<i64, String> {
    let mut conn = Connection::open(db_path).map_err(|e| format!("创建词典数据库失败: {}", e))?;
    create_schema(&conn)?;
    let tx = conn.transaction().map_err(|e| format!("开始词典事务失败: {}", e))?;
    set_job(job_id, "import", 55.0, "正在解析 Wiktionary 词库", "");
    let mut count = 0i64;
    let mut scanned = 0i64;
    let result = crate::commands::dict_formats::parse_wiktionary_jsonl(jsonl, |entry| {
        scanned += 1;
        if scanned % 5000 == 0 {
            set_job(job_id, "import", 82.0, &format!("已解析 {} 条", scanned), "");
        }
        insert_entry(
            &tx,
            &EntryIn {
                term: &entry.term,
                phonetic: &entry.phonetic,
                pos: &entry.pos,
                meanings: &entry.meanings,
                examples: &entry.examples,
                synonyms: &entry.synonyms,
                definition: &entry.definition,
            },
        )?;
        count += 1;
        Ok(())
    });
    result?;
    tx.commit().map_err(|e| format!("提交词典事务失败: {}", e))?;
    let _ = conn.execute_batch("PRAGMA optimize;");
    Ok(count)
}

fn import_legacy_pack(
    input: &Path,
    db_path: &Path,
    manifest: &mut DictManifest,
    job_id: &str,
) -> Result<i64, String> {
    let ext = input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if ext == "zip" {
        manifest.format = "zip".to_string();
        let (zip_manifest, csv_member, json_member) = normalized_manifest_from_zip(
            input,
            &manifest.name,
            &manifest.source_lang,
            &manifest.target_lang,
        )?;
        manifest.name = zip_manifest.name;
        manifest.source_lang = zip_manifest.source_lang;
        manifest.target_lang = zip_manifest.target_lang;
        manifest.version = zip_manifest.version;
        manifest.license = zip_manifest.license;
        if csv_member == "__cedict__" {
            manifest.source_lang = "zh".to_string();
            manifest.target_lang = "en".to_string();
            import_cedict(input, db_path, job_id)
        } else if !csv_member.is_empty() {
            let temp_csv = extract_zip_member(input, &csv_member)?;
            let count = import_csv(
                &temp_csv,
                db_path,
                &manifest.source_lang,
                &manifest.target_lang,
                job_id,
            );
            let _ = fs::remove_file(temp_csv);
            count
        } else if !json_member.is_empty() {
            let temp_json = extract_zip_member(input, &json_member)?;
            let count = import_json(&temp_json, db_path, job_id);
            let _ = fs::remove_file(temp_json);
            count
        } else {
            Err("词典包里没有可导入的 CSV / JSON / CC-CEDICT 文本".to_string())
        }
    } else if ext == "json" {
        manifest.format = "json".to_string();
        import_json(input, db_path, job_id)
    } else if ext == "csv" {
        manifest.format = "csv".to_string();
        import_csv(
            input,
            db_path,
            &manifest.source_lang,
            &manifest.target_lang,
            job_id,
        )
    } else {
        Err("无法识别的词典格式".to_string())
    }
}

/// 把一组文件（.mdx + .mdd / .ifo 三件套 / .dsl / .jsonl / .zip / .csv / .json）
/// 安装成一本词典。同名 .mdd 会自动一起解包成图片资源。
fn build_pack(
    files: &[PathBuf],
    display_name: &str,
    src_lang: &str,
    dst_lang: &str,
    job_id: &str,
) -> Result<DictManifest, String> {
    if files.is_empty() {
        return Err("没有选择词典文件".to_string());
    }
    let primary = &files[0];
    let stem = primary
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("imported")
        .to_string();
    let name = if display_name.trim().is_empty() {
        stem
    } else {
        display_name.trim().to_string()
    };

    let root = dict_root()?;
    let temp_dir = root.join(format!(".tmp-import-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).map_err(|e| format!("创建词典目录失败: {}", e))?;
    let db_path = temp_dir.join(DICT_DB);
    let res_dir = temp_dir.join(RES_DIR);

    let mut manifest = DictManifest {
        id: format!("{}_{}", sanitize_id(&name), Uuid::new_v4()),
        name: name.clone(),
        source_lang: src_lang.to_string(),
        target_lang: dst_lang.to_string(),
        version: "imported".to_string(),
        license: "用户导入".to_string(),
        description: String::new(),
        entry_count: 0,
        installed_at: now_iso(),
        builtin: false,
        format: String::new(),
        enabled: true,
        priority: 0,
        has_resources: false,
        resource_bytes: 0,
    };
    // 扩展名规则只放在 dict_formats::detect_format 一处，这里按它的判定归组。
    let (mdx, mdd, ifo, dsl, jsonl) = {
        let mut mdx: Option<PathBuf> = None;
        let mut mdd: Option<PathBuf> = None;
        let mut ifo: Option<PathBuf> = None;
        let mut dsl: Option<PathBuf> = None;
        let mut jsonl: Option<PathBuf> = None;
        for file in files {
            match crate::commands::dict_formats::detect_format(file) {
                "mdx" if mdx.is_none() => mdx = Some(file.clone()),
                "mdd" if mdd.is_none() => mdd = Some(file.clone()),
                "stardict" if ifo.is_none() => ifo = Some(file.clone()),
                "dsl" if dsl.is_none() => dsl = Some(file.clone()),
                "wiktionary" if jsonl.is_none() => jsonl = Some(file.clone()),
                _ => {}
            }
        }
        (mdx, mdd, ifo, dsl, jsonl)
    };

    let count = if let Some(mdx_path) = mdx {
        manifest.format = "mdx".to_string();
        if manifest.source_lang.is_empty() || manifest.source_lang == "auto" {
            manifest.source_lang = "en".to_string();
        }
        let count = import_mdx_into(&mdx_path, &db_path, &job_id)?;
        if let Some(mdd_path) = mdd {
            set_job(&job_id, "import", 96.0, "正在解包 MDD 图片资源", "");
            let mut total = 0u64;
            let saved = crate::commands::dict_formats::parse_mdd(
                &mdd_path,
                &res_dir,
                512 * 1024 * 1024,
                |_key, size| {
                    total = size;
                    Ok(())
                },
            )
            .unwrap_or(0);
            manifest.has_resources = saved > 0;
            manifest.resource_bytes = total;
        }
        count
    } else if let Some(ifo_path) = ifo {
        manifest.format = "stardict".to_string();
        let (count, book) = import_stardict_into(&ifo_path, &db_path, &job_id)?;
        if display_name.trim().is_empty() && !book.trim().is_empty() {
            manifest.name = book.trim().to_string();
        }
        count
    } else if let Some(dsl_path) = dsl {
        manifest.format = "dsl".to_string();
        let (count, dsl_name, src_hint, dst_hint) = import_dsl_into(&dsl_path, &db_path, &job_id)?;
        if display_name.trim().is_empty() && !dsl_name.trim().is_empty() {
            manifest.name = dsl_name.trim().to_string();
        }
        if display_name.trim().is_empty() {
            if !src_hint.trim().is_empty() {
                manifest.source_lang = src_hint.trim().to_lowercase();
            }
            if !dst_hint.trim().is_empty() {
                manifest.target_lang = dst_hint.trim().to_lowercase();
            }
        }
        count
    } else if let Some(jsonl_path) = jsonl {
        manifest.format = "wiktionary".to_string();
        import_wiktionary_into(&jsonl_path, &db_path, &job_id)?
    } else if mdd.is_some() {
        return Err("只选了 .mdd 资源包，请同时选择同名词库 .mdx".to_string());
    } else {
        import_legacy_pack(primary, &db_path, &mut manifest, &job_id)?
    };

    if count <= 0 {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err("没有解析出任何词条，请确认文件格式与编码".to_string());
    }
    manifest.entry_count = count;
    manifest.description = format!(
        "{} · {} → {}",
        format_label(&manifest.format),
        manifest.source_lang,
        manifest.target_lang
    );
    manifest.installed_at = now_iso();
    write_manifest(&temp_dir, &manifest)?;
    let final_dir = root.join(&manifest.id);
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir).map_err(|e| format!("替换旧词典失败: {}", e))?;
    }
    fs::rename(&temp_dir, &final_dir).map_err(|e| format!("安装词典失败: {}", e))?;
    set_job(
        &job_id,
        "done",
        100.0,
        &format!("已导入 {} 条", manifest.entry_count),
        "",
    );
    Ok(manifest)
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
    let job_id = format!("import-{}", Uuid::new_v4());
    tauri::async_runtime::spawn_blocking(move || {
        build_pack(&[input], &display_name, &src, &dst, &job_id)
    })
    .await
    .map_err(|e| format!("导入词典任务异常: {}", e))?
}

/// 多文件导入：MDX 常与 MDD 成对出现，StarDict 是 .ifo/.idx/.dict 三件套，
/// 前端一次多选，这里按格式归组。
#[tauri::command]
pub async fn language_learning_dict_import_files(
    paths: Vec<String>,
    name: Option<String>,
    source_lang: Option<String>,
    target_lang: Option<String>,
    job_id: Option<String>,
) -> Result<DictManifest, String> {
    let inputs: Vec<PathBuf> = paths
        .iter()
        .map(|path| PathBuf::from(path.trim()))
        .filter(|path| path.is_file())
        .collect();
    if inputs.is_empty() {
        return Err("要导入的词典文件不存在".to_string());
    }
    // 先按扩展名过一遍，能提前给出「不支持这个格式」而不是解析到一半才失败。
    let recognized = inputs
        .iter()
        .filter(|path| !crate::commands::dict_formats::detect_format(path).is_empty())
        .count();
    if recognized == 0 {
        return Err("暂不支持这些文件格式，可导入 .mdx/.mdd/.ifo/.dsl/.jsonl/.zip/.csv/.json".to_string());
    }
    let display_name = name.unwrap_or_default();
    let src = normalize_lang(source_lang.as_deref().unwrap_or("en"), "en");
    let dst = normalize_lang(target_lang.as_deref().unwrap_or("zh"), "zh");
    let job_id = job_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("import-{}", Uuid::new_v4()));
    tauri::async_runtime::spawn_blocking(move || {
        build_pack(&inputs, &display_name, &src, &dst, &job_id)
    })
    .await
    .map_err(|e| format!("导入词典任务异常: {}", e))?
}

/// 按词典分组查词：词典应用要像欧路那样一本一本地展示命中。
#[tauri::command]
pub fn dict_lookup_grouped(
    word: String,
    source_lang: Option<String>,
    target_lang: Option<String>,
    per_pack_limit: Option<u32>,
    max_packs: Option<u32>,
) -> Vec<DictPackHits> {
    let term = word.trim();
    if term.is_empty() {
        return Vec::new();
    }
    let source = source_lang.unwrap_or_else(|| "auto".to_string());
    let source = if source.trim().is_empty() || source == "auto" {
        detect_lang(term).to_string()
    } else {
        lang_key(&source)
    };
    let mut target = lang_key(&normalize_lang(target_lang.as_deref().unwrap_or("zh"), "zh"));
    // 查中文词时用户想看的通常是英文释义（CC-CEDICT 是 中→英），
    // 与语言学习里的方向翻转保持一致。
    if source == "zh" && target == "zh" {
        target = "en".to_string();
    }
    let per_pack = per_pack_limit.unwrap_or(3).clamp(1, 10) as usize;
    let max_packs = max_packs.unwrap_or(6).clamp(1, 16) as usize;
    let mut out = Vec::new();
    for (manifest, dir, _) in installed_packs() {
        if !manifest.enabled || !pack_matches(&manifest, &source, &target) {
            continue;
        }
        let entries = query_pack(&dir.join(DICT_DB), &manifest, term, per_pack);
        if entries.is_empty() {
            continue;
        }
        out.push(DictPackHits {
            pack_id: manifest.id.clone(),
            pack_name: manifest.name.clone(),
            source_lang: manifest.source_lang.clone(),
            target_lang: manifest.target_lang.clone(),
            format: manifest.format.clone(),
            entry_count: manifest.entry_count,
            entries,
        });
        if out.len() >= max_packs {
            break;
        }
    }
    out
}

/// 词典浏览器：按词条顺序翻阅或模糊搜索，不依赖查词精准命中。
#[tauri::command]
pub fn dict_browse_entries(
    id: String,
    keyword: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Vec<DictBrowserRow> {
    let safe = sanitize_id(&id);
    let dir = match dict_root() {
        Ok(root) => root.join(safe),
        Err(_) => return Vec::new(),
    };
    let db_path = dir.join(DICT_DB);
    if !db_path.is_file() {
        return Vec::new();
    }
    ensure_entry_columns(&db_path);
    let conn = match open_readonly(&db_path) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let keyword = keyword.unwrap_or_default().trim().to_lowercase();
    let offset = offset.unwrap_or(0) as i64;
    let limit = limit.unwrap_or(60).clamp(1, 300) as i64;
    let sql = if keyword.is_empty() {
        "SELECT term, phonetic, pos, meanings_json FROM entries ORDER BY normalized ASC LIMIT ?1 OFFSET ?2"
    } else {
        "SELECT term, phonetic, pos, meanings_json FROM entries WHERE normalized LIKE ?3 ORDER BY length(normalized) ASC, normalized ASC LIMIT ?1 OFFSET ?2"
    };
    let mut stmt = match conn.prepare(sql) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<DictBrowserRow> {
        let meanings_json: String = row.get(3)?;
        let meanings: Vec<String> = serde_json::from_str(&meanings_json).unwrap_or_default();
        Ok(DictBrowserRow {
            term: row.get(0)?,
            phonetic: row.get(1)?,
            pos: row.get(2)?,
            preview: meanings.first().cloned().unwrap_or_default(),
        })
    };
    let rows = if keyword.is_empty() {
        stmt.query_map(params![limit, offset], map_row)
    } else {
        stmt.query_map(params![limit, offset, format!("%{}%", keyword)], map_row)
    };
    match rows {
        Ok(iter) => iter.flatten().collect(),
        Err(_) => Vec::new(),
    }
}

fn read_pack_dir(id: &str) -> Result<(DictManifest, PathBuf), String> {
    let safe = sanitize_id(id);
    if safe != id.trim().to_lowercase() && safe != id.trim() {
        return Err("词典 ID 不合法".to_string());
    }
    let dir = dict_root()?.join(&safe);
    let manifest = read_manifest(&dir).ok_or_else(|| "词典不存在".to_string())?;
    Ok((manifest, dir))
}

#[tauri::command]
pub fn dict_set_enabled(id: String, enabled: bool) -> Result<DictManifest, String> {
    let (mut manifest, dir) = read_pack_dir(&id)?;
    manifest.enabled = enabled;
    write_manifest(&dir, &manifest)?;
    Ok(manifest)
}

#[tauri::command]
pub fn dict_set_priority(id: String, priority: i64) -> Result<DictManifest, String> {
    let (mut manifest, dir) = read_pack_dir(&id)?;
    manifest.priority = priority;
    write_manifest(&dir, &manifest)?;
    Ok(manifest)
}

/// 按传入顺序重排优先级（第一本最优先）。
#[tauri::command]
pub fn dict_reorder(ids: Vec<String>) -> Result<Vec<DictManifest>, String> {
    let total = ids.len() as i64;
    for (index, id) in ids.iter().enumerate() {
        let (mut manifest, dir) = read_pack_dir(id)?;
        manifest.priority = (total - index as i64) * 10;
        write_manifest(&dir, &manifest)?;
    }
    Ok(installed_packs().into_iter().map(|(m, _, _)| m).collect())
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
