use crate::commands::translator_free;
use crate::guaxiservices::bundled_resources;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use rss::Channel;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::{info, warn};
use urlencoding::encode;
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

const LANG_LEARN_DIR_NAME: &str = "guaxi_language_learning";
const LANG_LEARN_DB_NAME: &str = "language_learning.sqlite3";
const LISTENING_ITEMS_DIR_NAME: &str = "guaxi_listening_items";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
static LISTENING_SYNC_LOCK: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));
static LISTENING_SYNC_PENDING_IDS: Lazy<std::sync::Mutex<HashSet<String>>> =
    Lazy::new(|| std::sync::Mutex::new(HashSet::new()));

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WordMetaPayload {
    pub word: String,
    pub phonetic: Option<String>,
    pub phonetic_uk: Option<String>,
    pub phonetic_us: Option<String>,
    pub part_of_speech: Option<String>,
    #[serde(default)]
    pub meanings: Vec<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub antonyms: Vec<String>,
    #[serde(default)]
    pub associates: Vec<String>,
    #[serde(default)]
    pub synonyms_translations: Vec<String>,
    #[serde(default)]
    pub antonyms_translations: Vec<String>,
    #[serde(default)]
    pub associates_translations: Vec<String>,
    #[serde(default)]
    pub phrases: Vec<String>,
    #[serde(default)]
    pub phrase_translations: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub example_translations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationRecordInput {
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    pub engine: Option<String>,
    pub record_type: Option<String>,
    pub word_meta: Option<WordMetaPayload>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationRecord {
    pub id: String,
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine: String,
    pub record_type: String,
    pub is_word: bool,
    pub word: String,
    pub phonetic: String,
    pub phonetic_uk: String,
    pub phonetic_us: String,
    pub part_of_speech: String,
    pub meanings: Vec<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub associates: Vec<String>,
    pub synonyms_translations: Vec<String>,
    pub antonyms_translations: Vec<String>,
    pub associates_translations: Vec<String>,
    pub phrases: Vec<String>,
    pub phrase_translations: Vec<String>,
    pub examples: Vec<String>,
    pub example_translations: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioResourceInput {
    pub id: Option<String>,
    pub title: String,
    pub source_kind: String,
    pub source_uri: String,
    pub provider: Option<String>,
    pub topic: Option<String>,
    pub language_tag: Option<String>,
    pub transcript: Option<String>,
    pub sync_json_uri: Option<String>,
    pub notes: Option<String>,
    pub duration_sec: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioResource {
    pub id: String,
    pub title: String,
    pub source_kind: String,
    pub source_uri: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub language_tag: String,
    pub transcript: String,
    #[serde(default)]
    pub sync_json_uri: String,
    pub notes: String,
    pub duration_sec: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhisperSyncSegment {
    pub index: usize,
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
    #[serde(default)]
    pub translated_text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhisperSyncPayload {
    pub version: String,
    pub generated_at: String,
    pub model_path: String,
    pub audio_path: String,
    pub transcript: String,
    pub segments: Vec<WhisperSyncSegment>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WordEnrichment {
    pub word: String,
    pub phonetic: String,
    pub phonetic_uk: String,
    pub phonetic_us: String,
    pub part_of_speech: String,
    pub meanings: Vec<String>,
    pub translation: String,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub associates: Vec<String>,
    pub synonyms_translations: Vec<String>,
    pub antonyms_translations: Vec<String>,
    pub associates_translations: Vec<String>,
    pub phrases: Vec<String>,
    pub phrase_translations: Vec<String>,
    pub examples: Vec<String>,
    pub example_translations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateTranslationRecordInput {
    pub id: String,
    pub source_text: String,
    pub translated_text: String,
    pub record_type: Option<String>,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    pub engine: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningDataBundle {
    pub learning_lang: String,
    pub exported_at: String,
    pub records: Vec<TranslationRecord>,
    pub audio_resources: Vec<AudioResource>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LearningAudioAssetRef {
    pub audio_id: String,
    #[serde(default)]
    pub media_path: String,
    #[serde(default)]
    pub sync_json_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningExportPackage {
    pub learning_lang: String,
    pub scope: String,
    pub exported_at: String,
    pub bundle: LearningDataBundle,
    #[serde(default)]
    pub assets: Vec<LearningAudioAssetRef>,
    #[serde(default)]
    pub mastery: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningCounts {
    pub records: i64,
    pub audio_resources: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListeningTopicCollectInput {
    pub topic: String,
    pub learning_lang: Option<String>,
    pub limit: Option<i64>,
    pub sources: Option<Vec<ListeningSourceInput>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListeningSourceInput {
    pub provider: String,
    pub name: Option<String>,
    pub feed_url: String,
    pub language_tag: Option<String>,
    pub max_duration_sec: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListeningTranscriptFetchInput {
    pub source_uri: String,
    pub provider: Option<String>,
    pub topic: Option<String>,
    pub language_tag: Option<String>,
    pub notes: Option<String>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn resolve_db_path() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    let repos_dir = exe_dir.join("repos");
    if !repos_dir.exists() {
        fs::create_dir_all(&repos_dir).map_err(|e| format!("创建 repos 目录失败: {}", e))?;
    }

    let db_dir = repos_dir.join(LANG_LEARN_DIR_NAME);
    if !db_dir.exists() {
        fs::create_dir_all(&db_dir).map_err(|e| format!("创建语言学习目录失败: {}", e))?;
    }

    Ok(db_dir.join(LANG_LEARN_DB_NAME))
}

fn open_conn(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("打开语言学习数据库失败: {}", e))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("设置 foreign_keys 失败: {}", e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {}", e))?;
    Ok(conn)
}

fn setup_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS translation_records (
            id TEXT PRIMARY KEY,
            source_text TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            source_lang TEXT NOT NULL DEFAULT 'auto',
            target_lang TEXT NOT NULL DEFAULT 'zh',
            engine TEXT NOT NULL DEFAULT '',
            record_type TEXT NOT NULL DEFAULT 'sentence',
            is_word INTEGER NOT NULL DEFAULT 0,
            word TEXT NOT NULL DEFAULT '',
            phonetic TEXT NOT NULL DEFAULT '',
            phonetic_uk TEXT NOT NULL DEFAULT '',
            phonetic_us TEXT NOT NULL DEFAULT '',
            part_of_speech TEXT NOT NULL DEFAULT '',
            meanings_json TEXT NOT NULL DEFAULT '[]',
            synonyms_json TEXT NOT NULL DEFAULT '[]',
            antonyms_json TEXT NOT NULL DEFAULT '[]',
            associates_json TEXT NOT NULL DEFAULT '[]',
            synonyms_translations_json TEXT NOT NULL DEFAULT '[]',
            antonyms_translations_json TEXT NOT NULL DEFAULT '[]',
            associates_translations_json TEXT NOT NULL DEFAULT '[]',
            phrases_json TEXT NOT NULL DEFAULT '[]',
            phrase_translations_json TEXT NOT NULL DEFAULT '[]',
            examples_json TEXT NOT NULL DEFAULT '[]',
            example_translations_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_translation_records_created_at ON translation_records(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_translation_records_record_type ON translation_records(record_type);
        CREATE INDEX IF NOT EXISTS idx_translation_records_word ON translation_records(word);
        CREATE INDEX IF NOT EXISTS idx_translation_records_record_word ON translation_records(record_type, word);
        CREATE INDEX IF NOT EXISTS idx_translation_records_record_source ON translation_records(record_type, source_text);
        CREATE INDEX IF NOT EXISTS idx_translation_records_source_text ON translation_records(source_text);

        CREATE TABLE IF NOT EXISTS audio_resources (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            source_kind TEXT NOT NULL DEFAULT 'remote',
            source_uri TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT '',
            topic TEXT NOT NULL DEFAULT '',
            language_tag TEXT NOT NULL DEFAULT '',
            transcript TEXT NOT NULL DEFAULT '',
            sync_json_uri TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            duration_sec REAL NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_audio_resources_created_at ON audio_resources(created_at DESC);
        "#,
    )
    .map_err(|e| e.to_string())?;

    let migrations = [
        "ALTER TABLE translation_records ADD COLUMN associates_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE translation_records ADD COLUMN synonyms_translations_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE translation_records ADD COLUMN antonyms_translations_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE translation_records ADD COLUMN associates_translations_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE translation_records ADD COLUMN phonetic_uk TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE translation_records ADD COLUMN phonetic_us TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE translation_records ADD COLUMN phrases_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE translation_records ADD COLUMN phrase_translations_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE translation_records ADD COLUMN example_translations_json TEXT NOT NULL DEFAULT '[]'",
    ];
    for sql in migrations {
        let _ = conn.execute(sql, []);
    }
    for sql in [
        "CREATE INDEX IF NOT EXISTS idx_translation_records_record_word ON translation_records(record_type, word)",
        "CREATE INDEX IF NOT EXISTS idx_translation_records_record_source ON translation_records(record_type, source_text)",
        "CREATE INDEX IF NOT EXISTS idx_translation_records_source_text ON translation_records(source_text)",
        "ALTER TABLE audio_resources ADD COLUMN provider TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE audio_resources ADD COLUMN topic TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE audio_resources ADD COLUMN language_tag TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE audio_resources ADD COLUMN sync_json_uri TEXT NOT NULL DEFAULT ''",
    ] {
        let _ = conn.execute(sql, []);
    }

    Ok(())
}

fn parse_json_vec(text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(text).unwrap_or_default()
}

fn load_record_by_id(conn: &Connection, table: &str, id: &str) -> Result<Option<TranslationRecord>, String> {
    let sql = format!(
        r#"
        SELECT
          id, source_text, translated_text, source_lang, target_lang, engine,
          record_type, is_word, word, phonetic, phonetic_uk, phonetic_us, part_of_speech,
          meanings_json, synonyms_json, antonyms_json, associates_json,
          synonyms_translations_json, antonyms_translations_json, associates_translations_json,
          phrases_json, phrase_translations_json,
          examples_json, example_translations_json,
          created_at, updated_at
        FROM {table}
        WHERE id = ?1
        LIMIT 1
        "#,
        table = table
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("准备查询已有记录失败: {}", e))?;
    let row = stmt
        .query_row(params![id], |r| {
            let meanings_json: String = r.get(13)?;
            let synonyms_json: String = r.get(14)?;
            let antonyms_json: String = r.get(15)?;
            let associates_json: String = r.get(16)?;
            let synonyms_translations_json: String = r.get(17)?;
            let antonyms_translations_json: String = r.get(18)?;
            let associates_translations_json: String = r.get(19)?;
            let phrases_json: String = r.get(20)?;
            let phrase_translations_json: String = r.get(21)?;
            let examples_json: String = r.get(22)?;
            let example_translations_json: String = r.get(23)?;
            Ok(TranslationRecord {
                id: r.get(0)?,
                source_text: r.get(1)?,
                translated_text: r.get(2)?,
                source_lang: r.get(3)?,
                target_lang: r.get(4)?,
                engine: r.get(5)?,
                record_type: r.get(6)?,
                is_word: r.get::<_, i64>(7)? == 1,
                word: r.get(8)?,
                phonetic: r.get(9)?,
                phonetic_uk: r.get(10)?,
                phonetic_us: r.get(11)?,
                part_of_speech: r.get(12)?,
                meanings: parse_json_vec(&meanings_json),
                synonyms: parse_json_vec(&synonyms_json),
                antonyms: parse_json_vec(&antonyms_json),
                associates: parse_json_vec(&associates_json),
                synonyms_translations: parse_json_vec(&synonyms_translations_json),
                antonyms_translations: parse_json_vec(&antonyms_translations_json),
                associates_translations: parse_json_vec(&associates_translations_json),
                phrases: parse_json_vec(&phrases_json),
                phrase_translations: parse_json_vec(&phrase_translations_json),
                examples: parse_json_vec(&examples_json),
                example_translations: parse_json_vec(&example_translations_json),
                created_at: r.get(24)?,
                updated_at: r.get(25)?,
            })
        })
        .optional()
        .map_err(|e| format!("读取已有记录失败: {}", e))?;
    Ok(row)
}

fn normalize_word(word: &str) -> String {
    word.trim()
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .to_lowercase()
}

fn classify_text(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return "sentence".into();
    }
    let is_word = t.chars().all(|c| c.is_alphabetic() || c == '\'' || c == '-')
        && !t.contains(char::is_whitespace)
        && t.chars().count() <= 48;
    if is_word {
        "word".into()
    } else if t.contains('\n') || t.chars().count() > 120 {
        "paragraph".into()
    } else {
        "sentence".into()
    }
}

fn normalize_learning_lang(lang: Option<String>) -> String {
    let raw = lang.unwrap_or_else(|| "zh".to_string());
    let mut out = String::new();
    for ch in raw.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        }
    }
    if out.is_empty() {
        return "zh".to_string();
    }
    out
}

fn normalize_learning_scope(scope: Option<String>) -> String {
    let raw = scope.unwrap_or_else(|| "all".to_string());
    let key = raw.trim().to_lowercase();
    match key.as_str() {
        "all" | "my_vocab" | "my_sentences" | "my_paragraphs" | "my_listening" => key,
        _ => "all".to_string(),
    }
}

fn record_type_for_scope(scope: &str) -> Option<&'static str> {
    match scope {
        "my_vocab" => Some("word"),
        "my_sentences" => Some("sentence"),
        "my_paragraphs" => Some("paragraph"),
        _ => None,
    }
}

fn scope_requires_assets(scope: &str) -> bool {
    scope == "all" || scope == "my_listening"
}

fn normalize_topic_once(raw: &str) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return String::new();
    }
    let tokens = compact.split(' ').collect::<Vec<_>>();
    if tokens.len() >= 4 && tokens.len() % 2 == 0 {
        let half = tokens.len() / 2;
        let a = tokens[..half].join(" ");
        let b = tokens[half..].join(" ");
        if a == b {
            return a;
        }
    }
    compact
}

fn sanitize_asset_fragment(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let cleaned = out.trim_matches('_').to_string();
    if cleaned.is_empty() {
        "item".to_string()
    } else {
        cleaned
    }
}

fn resolve_listening_translate_target_lang() -> String {
    "zh".to_string()
}

fn infer_learning_lang(source_lang: &str, source_text: &str) -> String {
    let normalized = normalize_learning_lang(Some(source_lang.to_string()));
    if normalized != "auto" {
        return normalized;
    }
    let text = source_text.trim();
    if text.is_empty() {
        return "zh".to_string();
    }
    if text.chars().any(|c| c >= '\u{3040}' && c <= '\u{30ff}') {
        return "ja".to_string();
    }
    if text.chars().any(|c| c >= '\u{ac00}' && c <= '\u{d7af}') {
        return "ko".to_string();
    }
    if text.chars().any(|c| c >= '\u{0400}' && c <= '\u{04ff}') {
        return "ru".to_string();
    }
    if text
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c.is_ascii_whitespace() || c == '\'' || c == '-' || c == ',' || c == '.')
    {
        return "en".to_string();
    }
    if text.chars().any(|c| c >= '\u{4e00}' && c <= '\u{9fff}') {
        return "zh".to_string();
    }
    "zh".to_string()
}

fn records_table_for(lang: &str) -> String {
    let tag = normalize_learning_lang(Some(lang.to_string())).replace('-', "_");
    format!("translation_records_{}", tag)
}

fn audio_table_for(lang: &str) -> String {
    let tag = normalize_learning_lang(Some(lang.to_string())).replace('-', "_");
    format!("audio_resources_{}", tag)
}

fn ensure_lang_tables(conn: &Connection, lang: &str) -> Result<(), String> {
    let records_table = records_table_for(lang);
    let audio_table = audio_table_for(lang);

    let create_records_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table} (
            id TEXT PRIMARY KEY,
            source_text TEXT NOT NULL,
            translated_text TEXT NOT NULL,
            source_lang TEXT NOT NULL DEFAULT 'auto',
            target_lang TEXT NOT NULL DEFAULT 'zh',
            engine TEXT NOT NULL DEFAULT '',
            record_type TEXT NOT NULL DEFAULT 'sentence',
            is_word INTEGER NOT NULL DEFAULT 0,
            word TEXT NOT NULL DEFAULT '',
            phonetic TEXT NOT NULL DEFAULT '',
            phonetic_uk TEXT NOT NULL DEFAULT '',
            phonetic_us TEXT NOT NULL DEFAULT '',
            part_of_speech TEXT NOT NULL DEFAULT '',
            meanings_json TEXT NOT NULL DEFAULT '[]',
            synonyms_json TEXT NOT NULL DEFAULT '[]',
            antonyms_json TEXT NOT NULL DEFAULT '[]',
            associates_json TEXT NOT NULL DEFAULT '[]',
            synonyms_translations_json TEXT NOT NULL DEFAULT '[]',
            antonyms_translations_json TEXT NOT NULL DEFAULT '[]',
            associates_translations_json TEXT NOT NULL DEFAULT '[]',
            phrases_json TEXT NOT NULL DEFAULT '[]',
            phrase_translations_json TEXT NOT NULL DEFAULT '[]',
            examples_json TEXT NOT NULL DEFAULT '[]',
            example_translations_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_{table}_created_at ON {table}(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_{table}_record_type ON {table}(record_type);
        CREATE INDEX IF NOT EXISTS idx_{table}_word ON {table}(word);
        CREATE INDEX IF NOT EXISTS idx_{table}_record_word ON {table}(record_type, word);
        CREATE INDEX IF NOT EXISTS idx_{table}_record_source ON {table}(record_type, source_text);
        CREATE INDEX IF NOT EXISTS idx_{table}_source_text ON {table}(source_text);
        "#,
        table = records_table
    );
    conn.execute_batch(&create_records_sql)
        .map_err(|e| format!("创建语言词条表失败: {}", e))?;
    for sql in [
        format!(
            "ALTER TABLE {} ADD COLUMN synonyms_translations_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN antonyms_translations_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN associates_translations_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN phonetic_uk TEXT NOT NULL DEFAULT ''",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN phonetic_us TEXT NOT NULL DEFAULT ''",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN phrases_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN phrase_translations_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN examples_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN example_translations_json TEXT NOT NULL DEFAULT '[]'",
            records_table
        ),
    ] {
        let _ = conn.execute(&sql, []);
    }
    for sql in [
        format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_record_word ON {}(record_type, word)",
            records_table, records_table
        ),
        format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_record_source ON {}(record_type, source_text)",
            records_table, records_table
        ),
        format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_source_text ON {}(source_text)",
            records_table, records_table
        ),
    ] {
        let _ = conn.execute(&sql, []);
    }

    let create_audio_sql = format!(
        r#"
        CREATE TABLE IF NOT EXISTS {table} (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            source_kind TEXT NOT NULL DEFAULT 'remote',
            source_uri TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT '',
            topic TEXT NOT NULL DEFAULT '',
            language_tag TEXT NOT NULL DEFAULT '',
            transcript TEXT NOT NULL DEFAULT '',
            sync_json_uri TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            duration_sec REAL NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_{table}_created_at ON {table}(created_at DESC);
        "#,
        table = audio_table
    );
    conn.execute_batch(&create_audio_sql)
        .map_err(|e| format!("创建语言听力表失败: {}", e))?;
    for sql in [
        format!(
            "ALTER TABLE {} ADD COLUMN provider TEXT NOT NULL DEFAULT ''",
            audio_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN topic TEXT NOT NULL DEFAULT ''",
            audio_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN language_tag TEXT NOT NULL DEFAULT ''",
            audio_table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN sync_json_uri TEXT NOT NULL DEFAULT ''",
            audio_table
        ),
    ] {
        let _ = conn.execute(&sql, []);
    }

    Ok(())
}

fn with_db<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let _ = app;
    let db_path = resolve_db_path()?;
    let conn = open_conn(&db_path)?;
    setup_schema(&conn)?;
    ensure_lang_tables(&conn, "zh")?;
    f(&conn)
}

#[tauri::command]
pub async fn create_language_learning_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    theme: Option<String>,
) -> Result<(), String> {
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/language-learning?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/language-learning".into())
    };

    info!(
        "[LanguageLearning] create window label={} title={} url={:?}",
        label, title, url
    );

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1360.0, 900.0)
        .min_inner_size(1080.0, 720.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();
    Ok(())
}

#[tauri::command]
pub fn language_learning_record_translation(
    app: tauri::AppHandle,
    input: TranslationRecordInput,
) -> Result<TranslationRecord, String> {
    with_db(&app, |conn| {
        let now = now_iso();
        let record_id = Uuid::new_v4().to_string();
        let source_text = input.source_text.trim().to_string();
        let translated_text = input.translated_text.trim().to_string();
        if source_text.is_empty() || translated_text.is_empty() {
            return Err("原文或译文为空，无法记录".into());
        }

        let source_lang = input
            .source_lang
            .unwrap_or_else(|| "auto".into())
            .trim()
            .to_string();
        let target_lang = input
            .target_lang
            .unwrap_or_else(|| "zh".into())
            .trim()
            .to_string();
        let learning_lang = infer_learning_lang(&source_lang, &source_text);
        ensure_lang_tables(conn, &learning_lang)?;
        let records_table = records_table_for(&learning_lang);
        let engine = input.engine.unwrap_or_default().trim().to_string();

        let fallback_type = classify_text(&source_text);
        let record_type = input
            .record_type
            .unwrap_or(fallback_type)
            .trim()
            .to_lowercase();
        let is_word = record_type == "word";

        let meta = input.word_meta.unwrap_or_default();
        let word = normalize_word(if meta.word.trim().is_empty() {
            &source_text
        } else {
            &meta.word
        });
        let phonetic = meta.phonetic.unwrap_or_default();
        let phonetic_uk = meta.phonetic_uk.unwrap_or_default();
        let phonetic_us = meta.phonetic_us.unwrap_or_default();
        let part_of_speech = meta.part_of_speech.unwrap_or_default();
        let meanings_json =
            serde_json::to_string(&meta.meanings).map_err(|e| format!("meanings 序列化失败: {}", e))?;
        let synonyms_json =
            serde_json::to_string(&meta.synonyms).map_err(|e| format!("synonyms 序列化失败: {}", e))?;
        let antonyms_json =
            serde_json::to_string(&meta.antonyms).map_err(|e| format!("antonyms 序列化失败: {}", e))?;
        let associates_json =
            serde_json::to_string(&meta.associates).map_err(|e| format!("associates 序列化失败: {}", e))?;
        let synonyms_translations_json = serde_json::to_string(&meta.synonyms_translations)
            .map_err(|e| format!("synonyms_translations 序列化失败: {}", e))?;
        let antonyms_translations_json = serde_json::to_string(&meta.antonyms_translations)
            .map_err(|e| format!("antonyms_translations 序列化失败: {}", e))?;
        let associates_translations_json = serde_json::to_string(&meta.associates_translations)
            .map_err(|e| format!("associates_translations 序列化失败: {}", e))?;
        let phrases_json =
            serde_json::to_string(&meta.phrases).map_err(|e| format!("phrases 序列化失败: {}", e))?;
        let phrase_translations_json = serde_json::to_string(&meta.phrase_translations)
            .map_err(|e| format!("phrase_translations 序列化失败: {}", e))?;
        let examples_json =
            serde_json::to_string(&meta.examples).map_err(|e| format!("examples 序列化失败: {}", e))?;
        let example_translations_json = serde_json::to_string(&meta.example_translations)
            .map_err(|e| format!("example_translations 序列化失败: {}", e))?;

        let duplicate_id = if is_word {
            let sql = format!(
                "SELECT id FROM {} WHERE record_type = 'word' AND word = ?1 LIMIT 1",
                records_table
            );
            conn.query_row(&sql, params![&word], |r| r.get::<_, String>(0))
                .optional()
                .map_err(|e| format!("查询重复词条失败: {}", e))?
        } else {
            let sql = format!(
                "SELECT id FROM {} WHERE record_type = ?1 AND source_text = ?2 LIMIT 1",
                records_table
            );
            conn.query_row(&sql, params![&record_type, &source_text], |r| r.get::<_, String>(0))
                .optional()
                .map_err(|e| format!("查询重复记录失败: {}", e))?
        };
        if let Some(existing_id) = duplicate_id {
            let update_sql = format!(
                r#"
                UPDATE {table}
                SET
                  translated_text = ?1,
                  source_lang = ?2,
                  target_lang = ?3,
                  engine = ?4,
                  phonetic = ?5,
                  phonetic_uk = ?6,
                  phonetic_us = ?7,
                  part_of_speech = ?8,
                  meanings_json = ?9,
                  synonyms_json = ?10,
                  antonyms_json = ?11,
                  associates_json = ?12,
                  synonyms_translations_json = ?13,
                  antonyms_translations_json = ?14,
                  associates_translations_json = ?15,
                  phrases_json = ?16,
                  phrase_translations_json = ?17,
                  examples_json = ?18,
                  example_translations_json = ?19,
                  updated_at = ?20
                WHERE id = ?21
                "#,
                table = records_table
            );
            conn.execute(
                &update_sql,
                params![
                    translated_text,
                    source_lang,
                    target_lang,
                    engine,
                    phonetic,
                    phonetic_uk,
                    phonetic_us,
                    part_of_speech,
                    meanings_json,
                    synonyms_json,
                    antonyms_json,
                    associates_json,
                    synonyms_translations_json,
                    antonyms_translations_json,
                    associates_translations_json,
                    phrases_json,
                    phrase_translations_json,
                    examples_json,
                    example_translations_json,
                    now,
                    existing_id
                ],
            )
            .map_err(|e| format!("更新重复记录失败: {}", e))?;
            return load_record_by_id(conn, &records_table, &existing_id)?
                .ok_or_else(|| "读取重复记录失败".to_string());
        }

        let insert_sql = format!(
            r#"
            INSERT INTO {table}(
              id, source_text, translated_text, source_lang, target_lang, engine,
              record_type, is_word, word, phonetic, phonetic_uk, phonetic_us, part_of_speech,
              meanings_json, synonyms_json, antonyms_json, associates_json,
              synonyms_translations_json, antonyms_translations_json, associates_translations_json,
              phrases_json, phrase_translations_json,
              examples_json, example_translations_json,
              created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
            "#,
            table = records_table
        );
        conn.execute(
            &insert_sql,
            params![
                record_id,
                source_text,
                translated_text,
                source_lang,
                target_lang,
                engine,
                record_type,
                if is_word { 1 } else { 0 },
                word,
                phonetic,
                phonetic_uk,
                phonetic_us,
                part_of_speech,
                meanings_json,
                synonyms_json,
                antonyms_json,
                associates_json,
                synonyms_translations_json,
                antonyms_translations_json,
                associates_translations_json,
                phrases_json,
                phrase_translations_json,
                examples_json,
                example_translations_json,
                now,
                now
            ],
        )
        .map_err(|e| format!("写入翻译记录失败: {}", e))?;

        let record = TranslationRecord {
            id: record_id,
            source_text: source_text.clone(),
            translated_text: translated_text.clone(),
            source_lang,
            target_lang,
            engine,
            record_type,
            is_word,
            word,
            phonetic,
            phonetic_uk,
            phonetic_us,
            part_of_speech,
            meanings: meta.meanings,
            synonyms: meta.synonyms,
            antonyms: meta.antonyms,
            associates: meta.associates,
            synonyms_translations: meta.synonyms_translations,
            antonyms_translations: meta.antonyms_translations,
            associates_translations: meta.associates_translations,
            phrases: meta.phrases,
            phrase_translations: meta.phrase_translations,
            examples: meta.examples,
            example_translations: meta.example_translations,
            created_at: now.clone(),
            updated_at: now,
        };
        Ok(record)
    })
}

#[tauri::command]
pub fn language_learning_list_records(
    app: tauri::AppHandle,
    keyword: Option<String>,
    kind: Option<String>,
    learning_lang: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<TranslationRecord>, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let records_table = records_table_for(&learning_lang);
        let q = keyword.unwrap_or_default().trim().to_string();
        let kind = kind.unwrap_or_else(|| "all".into()).trim().to_lowercase();
        let limit = limit.unwrap_or(200).clamp(1, 1000);
        let offset = offset.unwrap_or(0).max(0);

        let like = format!("%{}%", q);
        let list_sql = format!(
            r#"
                SELECT
                  id, source_text, translated_text, source_lang, target_lang, engine,
                  record_type, is_word, word, phonetic, phonetic_uk, phonetic_us, part_of_speech,
                  meanings_json, synonyms_json, antonyms_json, associates_json,
                  synonyms_translations_json, antonyms_translations_json, associates_translations_json,
                  phrases_json, phrase_translations_json,
                  examples_json, example_translations_json,
                  created_at, updated_at
                FROM {table}
                WHERE (?1 = '' OR source_text LIKE ?2 OR translated_text LIKE ?2 OR word LIKE ?2)
                  AND (?3 = 'all' OR record_type = ?3)
                ORDER BY datetime(created_at) DESC
                LIMIT ?4 OFFSET ?5
                "#,
            table = records_table
        );
        let mut stmt = conn
            .prepare(&list_sql)
            .map_err(|e| format!("准备查询失败: {}", e))?;

        let rows = stmt
            .query_map(params![q, like, kind, limit, offset], |r| {
                let meanings_json: String = r.get(13)?;
                let synonyms_json: String = r.get(14)?;
                let antonyms_json: String = r.get(15)?;
                let associates_json: String = r.get(16)?;
                let synonyms_translations_json: String = r.get(17)?;
                let antonyms_translations_json: String = r.get(18)?;
                let associates_translations_json: String = r.get(19)?;
                let phrases_json: String = r.get(20)?;
                let phrase_translations_json: String = r.get(21)?;
                let examples_json: String = r.get(22)?;
                let example_translations_json: String = r.get(23)?;
                Ok(TranslationRecord {
                    id: r.get(0)?,
                    source_text: r.get(1)?,
                    translated_text: r.get(2)?,
                    source_lang: r.get(3)?,
                    target_lang: r.get(4)?,
                    engine: r.get(5)?,
                    record_type: r.get(6)?,
                    is_word: r.get::<_, i64>(7)? == 1,
                    word: r.get(8)?,
                    phonetic: r.get(9)?,
                    phonetic_uk: r.get(10)?,
                    phonetic_us: r.get(11)?,
                    part_of_speech: r.get(12)?,
                    meanings: parse_json_vec(&meanings_json),
                    synonyms: parse_json_vec(&synonyms_json),
                    antonyms: parse_json_vec(&antonyms_json),
                    associates: parse_json_vec(&associates_json),
                    synonyms_translations: parse_json_vec(&synonyms_translations_json),
                    antonyms_translations: parse_json_vec(&antonyms_translations_json),
                    associates_translations: parse_json_vec(&associates_translations_json),
                    phrases: parse_json_vec(&phrases_json),
                    phrase_translations: parse_json_vec(&phrase_translations_json),
                    examples: parse_json_vec(&examples_json),
                    example_translations: parse_json_vec(&example_translations_json),
                    created_at: r.get(24)?,
                    updated_at: r.get(25)?,
                })
            })
            .map_err(|e| format!("执行查询失败: {}", e))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("读取查询结果失败: {}", e))?);
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn language_learning_get_counts(
    app: tauri::AppHandle,
    keyword: Option<String>,
    kind: Option<String>,
    learning_lang: Option<String>,
) -> Result<LearningCounts, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let records_table = records_table_for(&learning_lang);
        let audio_table = audio_table_for(&learning_lang);
        let q = keyword.unwrap_or_default().trim().to_string();
        let kind = kind.unwrap_or_else(|| "all".into()).trim().to_lowercase();
        let like = format!("%{}%", q);
        let records_sql = format!(
            r#"
            SELECT COUNT(1) FROM {table}
            WHERE (?1 = '' OR source_text LIKE ?2 OR translated_text LIKE ?2 OR word LIKE ?2)
              AND (?3 = 'all' OR record_type = ?3)
            "#,
            table = records_table
        );
        let records: i64 = conn
            .query_row(&records_sql, params![q, like, kind], |r| r.get(0))
            .map_err(|e| format!("统计词条数量失败: {}", e))?;
        let audio_sql = format!(
            r#"
            SELECT COUNT(1) FROM {table}
            WHERE (?1 = '' OR title LIKE ?2 OR transcript LIKE ?2 OR notes LIKE ?2 OR provider LIKE ?2 OR topic LIKE ?2)
            "#,
            table = audio_table
        );
        let audio_resources: i64 = conn
            .query_row(&audio_sql, params![q, like], |r| r.get(0))
            .map_err(|e| format!("统计听力数量失败: {}", e))?;
        Ok(LearningCounts {
            records,
            audio_resources,
        })
    })
}

#[tauri::command]
pub fn language_learning_update_record(
    app: tauri::AppHandle,
    input: UpdateTranslationRecordInput,
    learning_lang: Option<String>,
) -> Result<TranslationRecord, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let records_table = records_table_for(&learning_lang);
        let id = input.id.trim().to_string();
        if id.is_empty() {
            return Err("记录ID不能为空".into());
        }
        let source_text = input.source_text.trim().to_string();
        let translated_text = input.translated_text.trim().to_string();
        if source_text.is_empty() || translated_text.is_empty() {
            return Err("原文和译文不能为空".into());
        }
        let record_type = input
            .record_type
            .unwrap_or_else(|| classify_text(&source_text))
            .trim()
            .to_lowercase();
        let is_word = record_type == "word";
        let word = if is_word {
            normalize_word(&source_text)
        } else {
            String::new()
        };
        let source_lang = normalize_learning_lang(input.source_lang);
        let target_lang = normalize_learning_lang(input.target_lang);
        let engine = input.engine.unwrap_or_default().trim().to_string();
        let now = now_iso();
        let sql = format!(
            r#"
            UPDATE {table}
            SET source_text = ?1,
                translated_text = ?2,
                source_lang = ?3,
                target_lang = ?4,
                engine = ?5,
                record_type = ?6,
                is_word = ?7,
                word = ?8,
                updated_at = ?9
            WHERE id = ?10
            "#,
            table = records_table
        );
        let changes = conn
            .execute(
                &sql,
                params![
                    source_text,
                    translated_text,
                    source_lang,
                    target_lang,
                    engine,
                    record_type,
                    if is_word { 1 } else { 0 },
                    word,
                    now,
                    id
                ],
            )
            .map_err(|e| format!("更新记录失败: {}", e))?;
        if changes <= 0 {
            return Err("未找到要更新的记录".into());
        }
        load_record_by_id(conn, &records_table, &id)?
            .ok_or_else(|| "读取更新后的记录失败".to_string())
    })
}

#[tauri::command]
pub fn language_learning_delete_records(
    app: tauri::AppHandle,
    ids: Vec<String>,
    learning_lang: Option<String>,
) -> Result<i64, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let records_table = records_table_for(&learning_lang);
        let valid_ids: Vec<String> = ids
            .into_iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
        if valid_ids.is_empty() {
            return Ok(0);
        }
        let placeholders = (0..valid_ids.len())
            .map(|_| "?".to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM {} WHERE id IN ({})",
            records_table, placeholders
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("准备删除记录失败: {}", e))?;
        let affected = stmt
            .execute(rusqlite::params_from_iter(valid_ids.iter()))
            .map_err(|e| format!("删除记录失败: {}", e))?;
        Ok(affected as i64)
    })
}

fn resolve_bundle_target_lang(bundle: &LearningDataBundle, learning_lang: Option<String>) -> String {
    normalize_learning_lang(Some(if let Some(v) = learning_lang {
        v
    } else if bundle.learning_lang.trim().is_empty() {
        "zh".to_string()
    } else {
        bundle.learning_lang.clone()
    }))
}

fn build_learning_bundle_for_scope(
    app: tauri::AppHandle,
    learning_lang: &str,
    scope: &str,
) -> Result<LearningDataBundle, String> {
    let records = if scope == "my_listening" {
        Vec::new()
    } else if let Some(kind) = record_type_for_scope(scope) {
        language_learning_list_records(
            app.clone(),
            None,
            Some(kind.to_string()),
            Some(learning_lang.to_string()),
            Some(100000),
            Some(0),
        )?
    } else {
        language_learning_list_records(
            app.clone(),
            None,
            Some("all".to_string()),
            Some(learning_lang.to_string()),
            Some(100000),
            Some(0),
        )?
    };
    let audio_resources = if scope == "my_vocab" || scope == "my_sentences" || scope == "my_paragraphs" {
        Vec::new()
    } else {
        language_learning_list_audio_resources(app, None, Some(learning_lang.to_string()))?
    };
    Ok(LearningDataBundle {
        learning_lang: learning_lang.to_string(),
        exported_at: now_iso(),
        records,
        audio_resources,
    })
}

fn import_learning_bundle_rows(
    app: tauri::AppHandle,
    bundle: LearningDataBundle,
    target_lang: &str,
) -> Result<(i64, i64), String> {
    let mut imported_records = 0i64;
    let mut imported_audio_resources = 0i64;
    for r in bundle.records {
        let input = TranslationRecordInput {
            source_text: r.source_text,
            translated_text: r.translated_text,
            source_lang: Some(r.source_lang),
            target_lang: Some(r.target_lang),
            engine: Some(r.engine),
            record_type: Some(r.record_type),
            word_meta: Some(WordMetaPayload {
                word: r.word,
                phonetic: Some(r.phonetic),
                phonetic_uk: Some(r.phonetic_uk),
                phonetic_us: Some(r.phonetic_us),
                part_of_speech: Some(r.part_of_speech),
                meanings: r.meanings,
                synonyms: r.synonyms,
                antonyms: r.antonyms,
                associates: r.associates,
                synonyms_translations: r.synonyms_translations,
                antonyms_translations: r.antonyms_translations,
                associates_translations: r.associates_translations,
                phrases: r.phrases,
                phrase_translations: r.phrase_translations,
                examples: r.examples,
                example_translations: r.example_translations,
            }),
        };
        let _ = language_learning_record_translation(app.clone(), input)?;
        imported_records += 1;
    }
    for a in bundle.audio_resources {
        let input = AudioResourceInput {
            id: Some(a.id),
            title: a.title,
            source_kind: a.source_kind,
            source_uri: a.source_uri,
            provider: Some(a.provider),
            topic: Some(a.topic),
            language_tag: Some(a.language_tag),
            transcript: Some(a.transcript),
            sync_json_uri: Some(a.sync_json_uri),
            notes: Some(a.notes),
            duration_sec: Some(a.duration_sec),
        };
        let _ = language_learning_upsert_audio_resource(app.clone(), input, Some(target_lang.to_string()))?;
        imported_audio_resources += 1;
    }
    Ok((imported_records, imported_audio_resources))
}

fn write_file_to_zip(zip: &mut ZipWriter<std::fs::File>, entry_name: &str, bytes: &[u8]) -> Result<(), String> {
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file(entry_name, options).map_err(|e| format!("写入压缩包文件失败: {}", e))?;
    zip.write_all(bytes).map_err(|e| format!("写入压缩包内容失败: {}", e))?;
    Ok(())
}

fn maybe_collect_local_asset_path(audio: &AudioResource) -> Option<PathBuf> {
    let source = normalize_media_uri(&audio.source_uri);
    if !source.is_empty() && !source.starts_with("http://") && !source.starts_with("https://") {
        let p = PathBuf::from(source);
        if p.exists() && p.is_file() {
            return Some(p);
        }
    }
    let entry = resolve_listening_entry_dir_by_id(&audio.id).ok().flatten()?;
    let mut candidates = fs::read_dir(entry).ok()?;
    while let Some(Ok(e)) = candidates.next() {
        let p = e.path();
        if p.is_file() {
            let name = p.file_name().and_then(|x| x.to_str()).unwrap_or_default().to_lowercase();
            if name.starts_with("media.") {
                return Some(p);
            }
        }
    }
    None
}

fn maybe_collect_sync_asset_path(audio: &AudioResource) -> Option<PathBuf> {
    let sync = normalize_media_uri(&audio.sync_json_uri);
    if !sync.is_empty() {
        let p = PathBuf::from(sync);
        if p.exists() && p.is_file() {
            return Some(p);
        }
    }
    let entry = resolve_listening_entry_dir_by_id(&audio.id).ok().flatten()?;
    let p = entry.join("media.json");
    if p.exists() && p.is_file() {
        return Some(p);
    }
    None
}

fn parse_bundle_and_mastery_from_json(raw: &str) -> Result<(LearningDataBundle, serde_json::Value), String> {
    if let Ok(pkg) = serde_json::from_str::<LearningExportPackage>(raw) {
        return Ok((pkg.bundle, pkg.mastery));
    }
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| format!("解析导入文件失败: {}", e))?;
    let bundle = LearningDataBundle {
        learning_lang: v.get("learning_lang").and_then(|x| x.as_str()).unwrap_or("zh").to_string(),
        exported_at: v
            .get("exported_at")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string(),
        records: serde_json::from_value(v.get("records").cloned().unwrap_or_else(|| serde_json::json!([])))
            .unwrap_or_default(),
        audio_resources: serde_json::from_value(v.get("audio_resources").cloned().unwrap_or_else(|| serde_json::json!([])))
            .unwrap_or_default(),
    };
    let mastery = v.get("mastery").cloned().unwrap_or_else(|| serde_json::json!({}));
    Ok((bundle, mastery))
}

#[tauri::command]
pub fn language_learning_export_bundle(
    app: tauri::AppHandle,
    learning_lang: Option<String>,
) -> Result<LearningDataBundle, String> {
    let lang = normalize_learning_lang(learning_lang);
    build_learning_bundle_for_scope(app, &lang, "all")
}

#[tauri::command]
pub fn language_learning_import_bundle(
    app: tauri::AppHandle,
    bundle: LearningDataBundle,
    learning_lang: Option<String>,
) -> Result<serde_json::Value, String> {
    let target_lang = resolve_bundle_target_lang(&bundle, learning_lang);
    let (imported_records, imported_audio_resources) =
        import_learning_bundle_rows(app, bundle, &target_lang)?;
    Ok(serde_json::json!({
        "learning_lang": target_lang,
        "imported_records": imported_records,
        "imported_audio_resources": imported_audio_resources
    }))
}

#[tauri::command]
pub fn language_learning_cleanup_duplicate_topics(
    app: tauri::AppHandle,
    learning_lang: Option<String>,
) -> Result<serde_json::Value, String> {
    with_db(&app, |conn| {
        let lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &lang)?;
        let table = audio_table_for(&lang);
        let sql = format!("SELECT id, topic FROM {}", table);
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("准备查询主题失败: {}", e))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| format!("读取主题失败: {}", e))?;
        let mut changed = 0i64;
        for row in rows {
            let (id, topic) = row.map_err(|e| format!("解析主题失败: {}", e))?;
            let normalized = normalize_topic_once(&topic);
            if normalized == topic {
                continue;
            }
            let up_sql = format!("UPDATE {} SET topic = ?1, updated_at = ?2 WHERE id = ?3", table);
            conn.execute(&up_sql, params![normalized, now_iso(), id])
                .map_err(|e| format!("更新主题失败: {}", e))?;
            changed += 1;
        }
        Ok(serde_json::json!({ "learning_lang": lang, "changed_topics": changed }))
    })
}

#[tauri::command]
pub async fn language_learning_export_scope_package(
    app: tauri::AppHandle,
    learning_lang: Option<String>,
    scope: Option<String>,
    output_path: String,
    mastery: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        let lang = normalize_learning_lang(learning_lang);
        let scope = normalize_learning_scope(scope);
        let output = output_path.trim().to_string();
        if output.is_empty() {
            return Err("导出路径不能为空".to_string());
        }
        let bundle = build_learning_bundle_for_scope(app_clone.clone(), &lang, &scope)?;
        let package = LearningExportPackage {
            learning_lang: lang.clone(),
            scope: scope.clone(),
            exported_at: now_iso(),
            bundle,
            assets: Vec::new(),
            mastery: mastery.unwrap_or_else(|| serde_json::json!({})),
        };
        if !scope_requires_assets(&scope) {
            let text = serde_json::to_string_pretty(&package).map_err(|e| format!("序列化导出数据失败: {}", e))?;
            fs::write(&output, text).map_err(|e| format!("写入导出文件失败: {}", e))?;
            return Ok(serde_json::json!({
                "learning_lang": lang,
                "scope": scope,
                "export_path": output,
                "records": package.bundle.records.len(),
                "audio_resources": package.bundle.audio_resources.len(),
                "assets": 0
            }));
        }
        let file = std::fs::File::create(&output).map_err(|e| format!("创建导出压缩包失败: {}", e))?;
        let mut zip = ZipWriter::new(file);
        let mut package = package;
        let mut assets_count = 0usize;
        for audio in &package.bundle.audio_resources {
            let id_key = sanitize_asset_fragment(&audio.id);
            let mut asset_ref = LearningAudioAssetRef {
                audio_id: audio.id.clone(),
                media_path: String::new(),
                sync_json_path: String::new(),
            };
            if let Some(media_path) = maybe_collect_local_asset_path(audio) {
                let ext = media_path
                    .extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("mp3")
                    .to_lowercase();
                let rel = format!("assets/{}/media.{}", id_key, ext);
                let bytes = fs::read(&media_path).map_err(|e| format!("读取音频资源失败: {}", e))?;
                write_file_to_zip(&mut zip, &rel, &bytes)?;
                asset_ref.media_path = rel;
                assets_count += 1;
            }
            if let Some(sync_path) = maybe_collect_sync_asset_path(audio) {
                let rel = format!("assets/{}/media.json", id_key);
                let bytes = fs::read(&sync_path).map_err(|e| format!("读取同步元数据失败: {}", e))?;
                write_file_to_zip(&mut zip, &rel, &bytes)?;
                asset_ref.sync_json_path = rel;
                assets_count += 1;
            }
            if !asset_ref.media_path.is_empty() || !asset_ref.sync_json_path.is_empty() {
                package.assets.push(asset_ref);
            }
        }
        let manifest = serde_json::to_string_pretty(&package).map_err(|e| format!("序列化导出包失败: {}", e))?;
        write_file_to_zip(&mut zip, "package.json", manifest.as_bytes())?;
        zip.finish().map_err(|e| format!("完成导出压缩包失败: {}", e))?;
        Ok(serde_json::json!({
            "learning_lang": lang,
            "scope": scope,
            "export_path": output,
            "records": package.bundle.records.len(),
            "audio_resources": package.bundle.audio_resources.len(),
            "assets": assets_count
        }))
    })
    .await
    .map_err(|e| format!("导出任务执行失败: {}", e))?
}

#[tauri::command]
pub async fn language_learning_import_scope_package(
    app: tauri::AppHandle,
    learning_lang: Option<String>,
    scope: Option<String>,
    input_path: String,
) -> Result<serde_json::Value, String> {
    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        let scope = normalize_learning_scope(scope);
        let path = input_path.trim().to_string();
        if path.is_empty() {
            return Err("导入路径不能为空".to_string());
        }
        let mut bundle: LearningDataBundle;
        let mut mastery = serde_json::json!({});
        if path.to_lowercase().ends_with(".zip") {
            let file = std::fs::File::open(&path).map_err(|e| format!("打开导入压缩包失败: {}", e))?;
            let mut archive = ZipArchive::new(file).map_err(|e| format!("读取导入压缩包失败: {}", e))?;
            let mut manifest_text = String::new();
            archive
                .by_name("package.json")
                .map_err(|e| format!("导入包缺少 package.json: {}", e))?
                .read_to_string(&mut manifest_text)
                .map_err(|e| format!("读取 package.json 失败: {}", e))?;
            let package: LearningExportPackage =
                serde_json::from_str(&manifest_text).map_err(|e| format!("解析导入包失败: {}", e))?;
            bundle = package.bundle;
            mastery = package.mastery;
            let mut asset_map = HashMap::new();
            for asset in package.assets {
                asset_map.insert(asset.audio_id.clone(), asset);
            }
            for audio in &mut bundle.audio_resources {
                let Some(asset) = asset_map.get(&audio.id) else {
                    continue;
                };
                let entry_dir = resolve_listening_entry_dir(audio)?;
                if !asset.media_path.trim().is_empty() {
                    if let Ok(mut zf) = archive.by_name(asset.media_path.as_str()) {
                        let mut bytes = Vec::new();
                        zf.read_to_end(&mut bytes)
                            .map_err(|e| format!("读取导入音频失败: {}", e))?;
                        let ext = Path::new(asset.media_path.as_str())
                            .extension()
                            .and_then(|x| x.to_str())
                            .unwrap_or("mp3")
                            .to_lowercase();
                        let media_path = entry_dir.join(format!("media.{}", ext));
                        fs::write(&media_path, bytes).map_err(|e| format!("写入导入音频失败: {}", e))?;
                        audio.source_uri = media_path.to_string_lossy().to_string();
                    }
                }
                if !asset.sync_json_path.trim().is_empty() {
                    if let Ok(mut zf) = archive.by_name(asset.sync_json_path.as_str()) {
                        let mut bytes = Vec::new();
                        zf.read_to_end(&mut bytes)
                            .map_err(|e| format!("读取导入元数据失败: {}", e))?;
                        let sync_path = entry_dir.join("media.json");
                        fs::write(&sync_path, bytes).map_err(|e| format!("写入导入元数据失败: {}", e))?;
                        audio.sync_json_uri = sync_path.to_string_lossy().to_string();
                    }
                }
            }
        } else {
            let raw = fs::read_to_string(&path).map_err(|e| format!("读取导入文件失败: {}", e))?;
            let parsed = parse_bundle_and_mastery_from_json(&raw)?;
            bundle = parsed.0;
            mastery = parsed.1;
        }
        let target_lang = resolve_bundle_target_lang(&bundle, learning_lang);
        let import_bundle = if scope == "all" {
            bundle
        } else if scope == "my_listening" {
            LearningDataBundle {
                learning_lang: bundle.learning_lang.clone(),
                exported_at: bundle.exported_at.clone(),
                records: Vec::new(),
                audio_resources: bundle.audio_resources,
            }
        } else if let Some(kind) = record_type_for_scope(&scope) {
            LearningDataBundle {
                learning_lang: bundle.learning_lang.clone(),
                exported_at: bundle.exported_at.clone(),
                records: bundle
                    .records
                    .into_iter()
                    .filter(|x| x.record_type.trim().eq_ignore_ascii_case(kind))
                    .collect(),
                audio_resources: Vec::new(),
            }
        } else {
            bundle
        };
        let (imported_records, imported_audio_resources) =
            import_learning_bundle_rows(app_clone, import_bundle, &target_lang)?;
        Ok(serde_json::json!({
            "learning_lang": target_lang,
            "scope": scope,
            "imported_records": imported_records,
            "imported_audio_resources": imported_audio_resources,
            "mastery": mastery
        }))
    })
    .await
    .map_err(|e| format!("导入任务执行失败: {}", e))?
}

#[tauri::command]
pub fn language_learning_list_audio_resources(
    app: tauri::AppHandle,
    keyword: Option<String>,
    learning_lang: Option<String>,
) -> Result<Vec<AudioResource>, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let audio_table = audio_table_for(&learning_lang);
        let q = keyword.unwrap_or_default().trim().to_string();
        let like = format!("%{}%", q);
        let list_sql = format!(
            r#"
                SELECT id, title, source_kind, source_uri, provider, topic, language_tag, transcript, sync_json_uri, notes, duration_sec, created_at, updated_at
                FROM {table}
                WHERE (?1 = '' OR title LIKE ?2 OR transcript LIKE ?2 OR notes LIKE ?2 OR provider LIKE ?2 OR topic LIKE ?2)
                ORDER BY datetime(created_at) DESC
                LIMIT 1000
                "#,
            table = audio_table
        );
        let mut stmt = conn
            .prepare(&list_sql)
            .map_err(|e| format!("准备音频查询失败: {}", e))?;

        let rows = stmt
            .query_map(params![q, like], |r| {
                Ok(AudioResource {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    source_kind: r.get(2)?,
                    source_uri: r.get(3)?,
                    provider: r.get(4)?,
                    topic: r.get(5)?,
                    language_tag: r.get(6)?,
                    transcript: r.get(7)?,
                    sync_json_uri: r.get(8)?,
                    notes: r.get(9)?,
                    duration_sec: r.get::<_, f64>(10)?,
                    created_at: r.get(11)?,
                    updated_at: r.get(12)?,
                })
            })
            .map_err(|e| format!("执行音频查询失败: {}", e))?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| format!("读取音频结果失败: {}", e))?);
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn language_learning_list_pending_sync_audio_ids(
    app: tauri::AppHandle,
    learning_lang: Option<String>,
) -> Result<Vec<String>, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let audio_table = audio_table_for(&learning_lang);
        let sql = format!(
            r#"
                SELECT id, transcript, sync_json_uri
                FROM {table}
                ORDER BY datetime(created_at) DESC
            "#,
            table = audio_table
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("准备待同步条目查询失败: {}", e))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("执行待同步条目查询失败: {}", e))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, transcript, sync_json_uri) =
                row.map_err(|e| format!("读取待同步条目失败: {}", e))?;
            if transcript.trim().is_empty() || !sync_json_has_full_translation(&sync_json_uri) {
                out.push(id);
            }
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn language_learning_upsert_audio_resource(
    app: tauri::AppHandle,
    input: AudioResourceInput,
    learning_lang: Option<String>,
) -> Result<AudioResource, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let audio_table = audio_table_for(&learning_lang);
        let now = now_iso();
        let title = input.title.trim().to_string();
        let source_kind = input.source_kind.trim().to_string();
        let source_uri = normalize_media_uri(&input.source_uri);
        if title.is_empty() || source_uri.is_empty() {
            return Err("标题和资源地址不能为空".into());
        }
        let id = if input.id.as_ref().map(|x| x.trim().is_empty()).unwrap_or(true) {
            let check_sql = format!("SELECT id FROM {} WHERE source_uri = ?1 LIMIT 1", audio_table);
            conn.query_row(&check_sql, params![source_uri.clone()], |r| r.get::<_, String>(0))
                .optional()
                .map_err(|e| format!("检查重复音频失败: {}", e))?
                .unwrap_or_else(|| Uuid::new_v4().to_string())
        } else {
            input.id.unwrap_or_else(|| Uuid::new_v4().to_string())
        };
        let provider = input.provider.unwrap_or_default().trim().to_string();
        let topic = input.topic.unwrap_or_default().trim().to_string();
        let language_tag = normalize_learning_lang(input.language_tag);
        let transcript = input.transcript.unwrap_or_default().trim().to_string();
        let sync_json_uri = normalize_media_uri(input.sync_json_uri.unwrap_or_default().as_str());
        let notes = input.notes.unwrap_or_default().trim().to_string();
        let duration_sec = input.duration_sec.unwrap_or(0.0).max(0.0);

        let upsert_sql = format!(
            r#"
            INSERT INTO {table}(id, title, source_kind, source_uri, provider, topic, language_tag, transcript, sync_json_uri, notes, duration_sec, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
              title=excluded.title,
              source_kind=excluded.source_kind,
              source_uri=excluded.source_uri,
              provider=excluded.provider,
              topic=excluded.topic,
              language_tag=excluded.language_tag,
              transcript=excluded.transcript,
              sync_json_uri=excluded.sync_json_uri,
              notes=excluded.notes,
              duration_sec=excluded.duration_sec,
              updated_at=excluded.updated_at
            "#,
            table = audio_table
        );
        conn.execute(
            &upsert_sql,
            params![
                id,
                title,
                source_kind,
                source_uri,
                provider,
                topic,
                language_tag,
                transcript,
                sync_json_uri,
                notes,
                duration_sec,
                now,
                now
            ],
        )
        .map_err(|e| format!("保存音频资源失败: {}", e))?;

        Ok(AudioResource {
            id,
            title,
            source_kind,
            source_uri,
            provider,
            topic,
            language_tag,
            transcript,
            sync_json_uri,
            notes,
            duration_sec,
            created_at: now.clone(),
            updated_at: now,
        })
    })
}

#[tauri::command]
pub fn language_learning_delete_audio_resource(
    app: tauri::AppHandle,
    id: String,
    learning_lang: Option<String>,
) -> Result<(), String> {
    let removed = with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let audio_table = audio_table_for(&learning_lang);
        let row = load_audio_resource_by_id(conn, &audio_table, &id)?;
        let delete_sql = format!("DELETE FROM {} WHERE id = ?1", audio_table);
        conn.execute(&delete_sql, params![id])
            .map_err(|e| format!("删除音频资源失败: {}", e))?;
        Ok(row)
    })?;
    if let Some(row) = removed {
        cleanup_listening_entry_files(&row)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn language_learning_collect_audio_by_topic(
    app: tauri::AppHandle,
    input: ListeningTopicCollectInput,
) -> Result<Vec<AudioResource>, String> {
    let topic = input.topic.trim().to_string();
    if topic.is_empty() {
        return Err("主题不能为空".to_string());
    }
    let learning_lang = normalize_learning_lang(input.learning_lang);
    let limit = input.limit.unwrap_or(24).clamp(1, 80) as usize;
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {}", e))?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut selected_sources = input.sources.unwrap_or_default();
    if selected_sources.is_empty() {
        selected_sources = vec![
            ListeningSourceInput {
                provider: "archive".to_string(),
                name: Some("Archive".to_string()),
                feed_url: String::new(),
                language_tag: Some(learning_lang.clone()),
                max_duration_sec: Some(1800),
            },
            ListeningSourceInput {
                provider: "wikimedia".to_string(),
                name: Some("Wikimedia".to_string()),
                feed_url: String::new(),
                language_tag: Some(learning_lang.clone()),
                max_duration_sec: Some(1800),
            },
        ];
    }
    selected_sources = selected_sources
        .into_iter()
        .filter(|src| source_matches_language(src.language_tag.as_deref(), &learning_lang))
        .collect::<Vec<_>>();
    if selected_sources.is_empty() {
        return Err("未找到与当前学习语言匹配的听力源".to_string());
    }
    let mut empty_rounds = 0usize;
    while out.len() < limit && empty_rounds < 2 {
        let before_round = out.len();
        for src in selected_sources.clone() {
            if out.len() >= limit {
                break;
            }
            let source_limit = (limit - out.len()).min(2).max(1);
            let provider = src.provider.trim().to_lowercase();
            if provider == "archive" {
                collect_from_archive_org(
                    &client,
                    &app,
                    &topic,
                    &learning_lang,
                    source_limit,
                    &mut seen,
                    &mut out,
                )
                .await;
                continue;
            }
            if provider == "wikimedia" {
                collect_from_wikimedia_commons(
                    &client,
                    &app,
                    &topic,
                    &learning_lang,
                    source_limit,
                    &mut seen,
                    &mut out,
                )
                .await;
                continue;
            }
            let max_duration_sec = src.max_duration_sec.unwrap_or(1800).clamp(60, 1800) as f64;
            let mut feed_url = src.feed_url.trim().to_string();
            if feed_url.is_empty() {
                continue;
            }
            if feed_url.contains("{topic}") {
                feed_url = feed_url.replace("{topic}", &encode(&topic));
            }
            collect_from_rss_feed(
                &client,
                &app,
                &topic,
                &learning_lang,
                &provider,
                &feed_url,
                max_duration_sec,
                source_limit,
                &mut seen,
                &mut out,
            )
            .await;
        }
        if out.len() == before_round {
            empty_rounds += 1;
        } else {
            empty_rounds = 0;
        }
    }
    if out.is_empty() {
        return Err("未找到可用听力资源，请尝试更换主题或语言".to_string());
    }
    Ok(out)
}

async fn collect_from_archive_org(
    client: &Client,
    app: &tauri::AppHandle,
    topic: &str,
    learning_lang: &str,
    limit: usize,
    seen: &mut HashSet<String>,
    out: &mut Vec<AudioResource>,
) {
    if limit == 0 {
        return;
    }
    let lang_filter = match learning_lang {
        "en" => " AND (language:(eng OR english))",
        "zh" => " AND (language:(chi OR zho OR chinese))",
        "ja" => " AND (language:(jpn OR japanese))",
        "ko" => " AND (language:(kor OR korean))",
        "fr" => " AND (language:(fre OR fra OR french))",
        "de" => " AND (language:(ger OR deu OR german))",
        "es" => " AND (language:(spa OR spanish))",
        "ru" => " AND (language:(rus OR russian))",
        _ => "",
    };
    let variants = topic_keyword_variants(topic);
    let topic_query = if variants.is_empty() {
        topic.to_string()
    } else {
        variants
            .iter()
            .take(8)
            .map(|kw| format!("({})", kw))
            .collect::<Vec<_>>()
            .join(" OR ")
    };
    let day_seed = chrono::Utc::now().format("%Y%m%d").to_string();
    let archive_page = 1 + (stable_hash_u64(&format!("archive:{}:{}", topic, day_seed)) % 4) as usize;
    let query = format!("({}) AND mediatype:(audio){lang_filter}", topic_query);
    let url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=description&rows={}&page={}&sort[]=publicdate%20desc&output=json",
        encode(&query),
        limit.saturating_mul(5).clamp(16, 140),
        archive_page
    );
    let payload = match fetch_json(client, &url).await {
        Some(v) => v,
        None => return,
    };
    let docs = payload
        .get("response")
        .and_then(|v| v.get("docs"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut collected = 0usize;
    for doc in docs {
        if collected >= limit {
            break;
        }
        let identifier = doc
            .get("identifier")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if identifier.is_empty() {
            continue;
        }
        let title = doc
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let notes = doc
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let metadata_url = format!("https://archive.org/metadata/{}", encode(&identifier));
        let metadata = match fetch_json(client, &metadata_url).await {
            Some(v) => v,
            None => continue,
        };
        let files = metadata
            .get("files")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut source_uri = String::new();
        for f in files {
            let name = f
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let lname = name.to_lowercase();
            if !(lname.ends_with(".mp3")
                || lname.ends_with(".m4a")
                || lname.ends_with(".wav")
                || lname.ends_with(".ogg")
                || lname.ends_with(".flac"))
            {
                continue;
            }
            let url = format!(
                "https://archive.org/download/{}/{}",
                encode(&identifier),
                encode(&name)
            );
            source_uri = url;
            break;
        }
        if source_uri.is_empty() || seen.contains(&source_uri) {
            continue;
        }
        seen.insert(source_uri.clone());
        let saved = language_learning_upsert_audio_resource(
            app.clone(),
            AudioResourceInput {
                id: None,
                title: if title.is_empty() {
                    format!("{}（Archive）", topic)
                } else {
                    title
                },
                source_kind: "remote".to_string(),
                source_uri,
                provider: Some("archive".to_string()),
                topic: Some(topic.to_string()),
                language_tag: Some(learning_lang.to_string()),
                transcript: Some(String::new()),
                sync_json_uri: Some(String::new()),
                notes: Some(notes),
                duration_sec: Some(0.0),
            },
            Some(learning_lang.to_string()),
        );
        if let Ok(row) = saved {
            out.push(row);
            collected += 1;
        }
    }
}

async fn collect_from_wikimedia_commons(
    client: &Client,
    app: &tauri::AppHandle,
    topic: &str,
    learning_lang: &str,
    limit: usize,
    seen: &mut HashSet<String>,
    out: &mut Vec<AudioResource>,
) {
    if limit == 0 {
        return;
    }
    let variants = topic_keyword_variants(topic);
    let query_terms = if variants.is_empty() {
        topic.to_string()
    } else {
        variants
            .iter()
            .take(6)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let day_seed = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let sr_offset = (stable_hash_u64(&format!("wikimedia:{}:{}", topic, day_seed)) % 80) as usize;
    let search = format!("filetype:audio {}", query_terms);
    let search_url = format!(
        "https://commons.wikimedia.org/w/api.php?action=query&list=search&srsearch={}&sroffset={}&srlimit={}&format=json&utf8=1",
        encode(&search),
        sr_offset,
        limit.saturating_mul(4).clamp(12, 80)
    );
    let payload = match fetch_json(client, &search_url).await {
        Some(v) => v,
        None => return,
    };
    let rows = payload
        .get("query")
        .and_then(|v| v.get("search"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut collected = 0usize;
    for row in rows {
        if collected >= limit {
            break;
        }
        let page_id = row.get("pageid").and_then(|v| v.as_i64()).unwrap_or_default();
        if page_id <= 0 {
            continue;
        }
        let title = row
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .trim_start_matches("File:")
            .to_string();
        let detail_url = format!(
            "https://commons.wikimedia.org/w/api.php?action=query&prop=imageinfo&pageids={}&iiprop=url|mime&format=json",
            page_id
        );
        let detail = match fetch_json(client, &detail_url).await {
            Some(v) => v,
            None => continue,
        };
        let page_key = page_id.to_string();
        let image = detail
            .get("query")
            .and_then(|v| v.get("pages"))
            .and_then(|v| v.get(&page_key))
            .and_then(|v| v.get("imageinfo"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .cloned();
        let source_uri = image
            .as_ref()
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let mime = image
            .as_ref()
            .and_then(|v| v.get("mime"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_lowercase();
        if source_uri.is_empty() || !mime.starts_with("audio/") || seen.contains(&source_uri) {
            continue;
        }
        seen.insert(source_uri.clone());
        let saved = language_learning_upsert_audio_resource(
            app.clone(),
            AudioResourceInput {
                id: None,
                title: if title.is_empty() {
                    format!("{}（Wikimedia）", topic)
                } else {
                    title
                },
                source_kind: "remote".to_string(),
                source_uri,
                provider: Some("wikimedia".to_string()),
                topic: Some(topic.to_string()),
                language_tag: Some(learning_lang.to_string()),
                transcript: Some(String::new()),
                sync_json_uri: Some(String::new()),
                notes: Some("来源：Wikimedia Commons".to_string()),
                duration_sec: Some(0.0),
            },
            Some(learning_lang.to_string()),
        );
        if let Ok(row) = saved {
            out.push(row);
            collected += 1;
        }
    }
}

async fn collect_from_rss_feed(
    client: &Client,
    app: &tauri::AppHandle,
    topic: &str,
    learning_lang: &str,
    provider: &str,
    feed_url: &str,
    max_duration_sec: f64,
    limit: usize,
    seen: &mut HashSet<String>,
    out: &mut Vec<AudioResource>,
) {
    if limit == 0 {
        return;
    }
    let xml = match client.get(feed_url).send().await {
        Ok(resp) if resp.status().is_success() => match resp.text().await {
            Ok(v) => v,
            Err(_) => return,
        },
        _ => return,
    };
    let channel = match Channel::read_from(xml.as_bytes()) {
        Ok(v) => v,
        Err(_) => return,
    };
    struct RssCandidate {
        source_uri: String,
        title: String,
        notes: String,
        page_url: String,
        content_text: String,
        duration_sec: f64,
        pub_ts: Option<i64>,
        score: f64,
    }
    let bucket = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let mut candidates = Vec::new();
    for item in channel.items() {
        let enclosure = match item.enclosure() {
            Some(v) => v,
            None => continue,
        };
        let source_uri = normalize_media_uri(enclosure.url());
        if source_uri.is_empty() || seen.contains(&source_uri) {
            continue;
        }
        let title = item.title().unwrap_or_default().trim().to_string();
        let desc_raw = item.description().unwrap_or_default().trim().to_string();
        let content_raw = item.content().unwrap_or_default().trim().to_string();
        let base_notes = strip_html_tags(&desc_raw);
        let content_text = strip_html_tags(&content_raw);
        let page_url = item.link().unwrap_or_default().trim().to_string();
        let notes = if page_url.is_empty() {
            base_notes.clone()
        } else if base_notes.is_empty() {
            format!("来源页: {}", page_url)
        } else {
            format!("{}\n来源页: {}", base_notes, page_url)
        };
        let search_text = format!("{} {}", title.to_lowercase(), notes.to_lowercase());
        if !topic_matches_text(&search_text, topic) {
            continue;
        }
        let mut duration_sec =
            parse_duration_text(item.itunes_ext().and_then(|x| x.duration()).unwrap_or_default())
                .unwrap_or(0.0);
        if duration_sec <= 0.0 {
            duration_sec = 0.0;
        }
        if duration_sec > max_duration_sec {
            continue;
        }
        let pub_ts = parse_pub_datetime_to_ts(item.pub_date().unwrap_or_default());
        let recency = recency_preference_weight(pub_ts);
        let duration_pref = duration_preference_weight(duration_sec);
        let jitter = (stable_hash_u64(&format!(
            "{}|{}|{}|{}",
            source_uri, topic, provider, bucket
        )) % 1000) as f64
            / 1000.0
            * 0.24;
        candidates.push(RssCandidate {
            source_uri,
            title,
            notes,
            page_url,
            content_text,
            duration_sec,
            pub_ts,
            score: recency * duration_pref + jitter,
        });
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.pub_ts.unwrap_or(0).cmp(&a.pub_ts.unwrap_or(0)))
    });
    let mut collected = 0usize;
    for candidate in candidates.into_iter().take(limit) {
        if collected >= limit {
            break;
        }
        let mut transcript = if candidate.content_text.chars().count() >= 220 {
            split_long_text_to_lines(&candidate.content_text)
        } else {
            String::new()
        };
        if transcript.trim().is_empty() {
            transcript = fetch_transcript_from_source(
                client,
                Some(provider),
                &candidate.source_uri,
                if candidate.page_url.is_empty() {
                    None
                } else {
                    Some(candidate.page_url.as_str())
                },
                if candidate.notes.is_empty() {
                    None
                } else {
                    Some(candidate.notes.as_str())
                },
            )
            .await
            .unwrap_or_default();
        }
        if transcript.trim().is_empty() {
            transcript = if candidate.notes.is_empty() {
                String::new()
            } else {
                candidate.notes.clone()
            };
        }
        seen.insert(candidate.source_uri.clone());
        let saved = language_learning_upsert_audio_resource(
            app.clone(),
            AudioResourceInput {
                id: None,
                title: if candidate.title.is_empty() {
                    format!("{}（{}）", topic, provider)
                } else {
                    candidate.title
                },
                source_kind: "remote".to_string(),
                source_uri: candidate.source_uri,
                provider: Some(provider.to_string()),
                topic: Some(topic.to_string()),
                language_tag: Some(learning_lang.to_string()),
                transcript: Some(transcript),
                sync_json_uri: Some(String::new()),
                notes: Some(candidate.notes),
                duration_sec: Some(candidate.duration_sec),
            },
            Some(learning_lang.to_string()),
        );
        if let Ok(row) = saved {
            out.push(row);
            collected += 1;
        }
    }
}

#[tauri::command]
pub async fn language_learning_fetch_transcript_from_source(
    input: ListeningTranscriptFetchInput,
) -> Result<String, String> {
    let source_uri = normalize_media_uri(&input.source_uri);
    if source_uri.is_empty() {
        return Err("资源地址不能为空".to_string());
    }
    let provider = input.provider.unwrap_or_default();
    let notes = input.notes.unwrap_or_default();
    let page_url = extract_first_url(&notes);
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {}", e))?;
    let transcript = fetch_transcript_from_source(
        &client,
        if provider.trim().is_empty() {
            None
        } else {
            Some(provider.trim())
        },
        &source_uri,
        page_url.as_deref(),
        if notes.trim().is_empty() {
            None
        } else {
            Some(notes.as_str())
        },
    )
    .await
    .unwrap_or_default();
    if transcript.trim().is_empty() {
        return Err("未找到可用原文".to_string());
    }
    Ok(transcript)
}

#[tauri::command]
pub fn language_learning_read_local_media_base64(path: String) -> Result<serde_json::Value, String> {
    let source = normalize_media_uri(&path);
    if source.is_empty() {
        return Err("音频路径为空".to_string());
    }
    if source.starts_with("http://") || source.starts_with("https://") {
        return Err("仅支持本地音频路径".to_string());
    }
    let local_path = if source.starts_with("file://") {
        let trimmed = source.trim_start_matches("file://");
        if cfg!(target_os = "windows") {
            trimmed.trim_start_matches('/').to_string()
        } else {
            format!("/{}", trimmed.trim_start_matches('/'))
        }
    } else {
        source
    };
    let p = PathBuf::from(local_path);
    if !p.exists() {
        return Err("音频文件不存在".to_string());
    }
    let bytes = fs::read(&p).map_err(|e| format!("读取音频文件失败: {}", e))?;
    let mime = match p
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "mp3" => "audio/mpeg",
        "m4a" | "mp4" => "audio/mp4",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        _ => "application/octet-stream",
    };
    let base64 = BASE64_STANDARD.encode(bytes);
    Ok(serde_json::json!({
        "mime": mime,
        "base64": base64
    }))
}

#[tauri::command]
pub async fn language_learning_generate_whisper_sync_json(
    app: tauri::AppHandle,
    audio_id: String,
    learning_lang: Option<String>,
) -> Result<AudioResource, String> {
    let request_lang = normalize_learning_lang(learning_lang);
    let translate_target_lang = resolve_listening_translate_target_lang();
    let (resolved_lang, row) = with_db(&app, |conn| {
        ensure_lang_tables(conn, &request_lang)?;
        load_audio_resource_with_fallback_lang(conn, &audio_id, &request_lang)?
            .ok_or_else(|| "听力条目不存在".to_string())
    })?;
    let existing_sync = normalize_media_uri(&row.sync_json_uri);
    if !existing_sync.is_empty() {
        let p = PathBuf::from(&existing_sync);
        if p.exists() && p.is_file() {
            let has_full_translation = sync_json_has_full_translation(&existing_sync);
            let has_transcript = !row.transcript.trim().is_empty();
            if has_full_translation && has_transcript {
                return Ok(row);
            }
            warn!(
                audio_id = row.id.as_str(),
                has_full_translation,
                has_transcript,
                sync_json = existing_sync.as_str(),
                "listening sync exists but incomplete, regenerate whisper sync json"
            );
        }
    }
    let entry_dir = resolve_listening_entry_dir(&row)?;
    let local_audio = prepare_entry_audio_file(&row, &entry_dir).await?;
    let local_json =
        run_whisper_and_build_sync_json(&local_audio, &entry_dir, translate_target_lang.as_str()).await?;
    let json_text = fs::read_to_string(&local_json).map_err(|e| format!("读取同步 JSON 失败: {}", e))?;
    let payload: WhisperSyncPayload =
        serde_json::from_str(&json_text).map_err(|e| format!("解析同步 JSON 失败: {}", e))?;
    let transcript = payload.transcript.trim().to_string();
    let next = language_learning_upsert_audio_resource(
        app,
        AudioResourceInput {
            id: Some(row.id),
            title: row.title,
            source_kind: "local".to_string(),
            source_uri: local_audio.to_string_lossy().to_string(),
            provider: Some(if row.provider.trim().is_empty() {
                "local_whisper".to_string()
            } else {
                row.provider
            }),
            topic: Some(row.topic),
            language_tag: Some(if row.language_tag.trim().is_empty() {
                resolved_lang.clone()
            } else {
                row.language_tag
            }),
            transcript: Some(transcript),
            sync_json_uri: Some(local_json.to_string_lossy().to_string()),
            notes: Some(row.notes),
            duration_sec: Some(row.duration_sec),
        },
        Some(resolved_lang),
    )?;
    Ok(next)
}

async fn backfill_existing_sync_json_translation(
    sync_json_path: &Path,
    target_lang: &str,
) -> Result<WhisperSyncPayload, String> {
    let json_text = fs::read_to_string(sync_json_path)
        .map_err(|e| format!("读取同步 JSON 失败: {}", e))?;
    let mut payload: WhisperSyncPayload =
        serde_json::from_str(&json_text).map_err(|e| format!("解析同步 JSON 失败: {}", e))?;
    if payload.segments.is_empty() {
        return Err("同步 JSON 不包含可翻译分段".to_string());
    }
    let mut source_lines = Vec::new();
    let mut missing_indices = Vec::new();
    for (idx, seg) in payload.segments.iter().enumerate() {
        let src = seg.text.trim().to_string();
        if src.is_empty() {
            continue;
        }
        let translated = seg.translated_text.trim().to_string();
        if translated.is_empty() || translated.contains("[Translation Failed]") || translated == src {
            source_lines.push(src);
            missing_indices.push(idx);
        }
    }
    if source_lines.is_empty() {
        return Ok(payload);
    }
    let translated_lines = translate_segments_with_sentence_groups(&source_lines, target_lang).await;
    for (local_idx, next_idx) in missing_indices.iter().enumerate() {
        let Some(seg) = payload.segments.get_mut(*next_idx) else {
            continue;
        };
        let next = translated_lines
            .get(local_idx)
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .unwrap_or_default();
        if !next.is_empty() {
            seg.translated_text = next;
        }
    }
    if payload.transcript.trim().is_empty() {
        payload.transcript = source_lines.join("\n");
    }
    let formatted = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("构建同步 JSON 失败: {}", e))?;
    fs::write(sync_json_path, formatted).map_err(|e| format!("写入同步 JSON 失败: {}", e))?;
    Ok(payload)
}

#[tauri::command]
pub async fn language_learning_backfill_sync_translation(
    app: tauri::AppHandle,
    audio_id: String,
    learning_lang: Option<String>,
) -> Result<AudioResource, String> {
    let request_lang = normalize_learning_lang(learning_lang);
    let translate_target_lang = resolve_listening_translate_target_lang();
    let (resolved_lang, row) = with_db(&app, |conn| {
        ensure_lang_tables(conn, &request_lang)?;
        load_audio_resource_with_fallback_lang(conn, &audio_id, &request_lang)?
            .ok_or_else(|| "听力条目不存在".to_string())
    })?;
    let existing_sync = normalize_media_uri(&row.sync_json_uri);
    if !existing_sync.is_empty() {
        let sync_path = PathBuf::from(&existing_sync);
        if sync_path.exists() && sync_path.is_file() {
            let has_full_translation = sync_json_has_full_translation(&existing_sync);
            let has_transcript = !row.transcript.trim().is_empty();
            if has_full_translation && has_transcript {
                return Ok(row);
            }
            info!(
                audio_id = row.id.as_str(),
                learning_lang = resolved_lang.as_str(),
                has_full_translation,
                has_transcript,
                "backfill translation for existing sync json"
            );
            if let Ok(payload) = backfill_existing_sync_json_translation(&sync_path, translate_target_lang.as_str()).await {
                let transcript = if row.transcript.trim().is_empty() {
                    payload.transcript.trim().to_string()
                } else {
                    row.transcript.clone()
                };
                let next = language_learning_upsert_audio_resource(
                    app.clone(),
                    AudioResourceInput {
                        id: Some(row.id.clone()),
                        title: row.title.clone(),
                        source_kind: row.source_kind.clone(),
                        source_uri: row.source_uri.clone(),
                        provider: Some(row.provider.clone()),
                        topic: Some(row.topic.clone()),
                        language_tag: Some(if row.language_tag.trim().is_empty() {
                            resolved_lang.clone()
                        } else {
                            row.language_tag.clone()
                        }),
                        transcript: Some(transcript),
                        sync_json_uri: Some(existing_sync.clone()),
                        notes: Some(row.notes.clone()),
                        duration_sec: Some(row.duration_sec),
                    },
                    Some(resolved_lang.clone()),
                )?;
                if sync_json_has_full_translation(&existing_sync) {
                    return Ok(next);
                }
            }
        }
    }
    language_learning_generate_whisper_sync_json(app, audio_id, Some(resolved_lang)).await
}

#[tauri::command]
pub fn language_learning_generate_whisper_sync_json_async(
    app: tauri::AppHandle,
    audio_id: String,
    learning_lang: Option<String>,
) -> Result<(), String> {
    let id = audio_id.trim().to_string();
    if id.is_empty() {
        return Err("听力条目ID不能为空".to_string());
    }
    {
        let mut pending = LISTENING_SYNC_PENDING_IDS
            .lock()
            .map_err(|_| "后台同步任务状态异常".to_string())?;
        if pending.contains(&id) {
            return Ok(());
        }
        pending.insert(id.clone());
    }
    let app_handle = app.clone();
    let lang = learning_lang.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = LISTENING_SYNC_LOCK.lock().await;
        let _ = language_learning_generate_whisper_sync_json(app_handle, id.clone(), lang).await;
        if let Ok(mut pending) = LISTENING_SYNC_PENDING_IDS.lock() {
            pending.remove(&id);
        }
    });
    Ok(())
}

async fn fetch_transcript_from_source(
    client: &Client,
    provider: Option<&str>,
    source_uri: &str,
    page_url: Option<&str>,
    notes: Option<&str>,
) -> Option<String> {
    let provider_lc = provider.unwrap_or_default().trim().to_lowercase();
    if let Some(url) = page_url {
        if let Some(text) = fetch_transcript_from_page(client, url).await {
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    if !looks_like_audio_url(source_uri) {
        if let Some(text) = fetch_transcript_from_page(client, source_uri).await {
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    if (provider_lc == "ted" || provider_lc == "bbc" || provider_lc == "cnn")
        && notes.unwrap_or_default().contains("来源页:")
    {
        let page = extract_first_url(notes.unwrap_or_default());
        if let Some(url) = page {
            if let Some(text) = fetch_transcript_from_page(client, &url).await {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

async fn fetch_transcript_from_page(client: &Client, url: &str) -> Option<String> {
    let html = client.get(url).send().await.ok()?.text().await.ok()?;
    if html.trim().is_empty() {
        return None;
    }
    let block_re =
        Regex::new(r#"(?is)<(section|div)[^>]*(transcript|caption|subtitle)[^>]*>(.*?)</(section|div)>"#)
            .ok();
    if let Some(re) = block_re {
        for cap in re.captures_iter(&html).take(4) {
            let raw = cap.get(3).map(|m| m.as_str()).unwrap_or_default();
            let text = collapse_whitespace(&strip_html_tags(raw));
            if text.chars().count() >= 180 {
                return Some(split_long_text_to_lines(&text));
            }
        }
    }
    let p_re = Regex::new(r"(?is)<p[^>]*>(.*?)</p>").ok()?;
    let mut chunks = Vec::new();
    for cap in p_re.captures_iter(&html).take(420) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        let text = collapse_whitespace(&strip_html_tags(raw));
        if text.chars().count() < 24 {
            continue;
        }
        if !contains_sentence_like_content(&text) {
            continue;
        }
        chunks.push(text);
        if chunks.len() >= 180 {
            break;
        }
    }
    if chunks.len() >= 8 {
        return Some(chunks.join("\n"));
    }
    None
}

fn source_matches_language(source_lang: Option<&str>, learning_lang: &str) -> bool {
    let s = source_lang.unwrap_or_default().trim().to_lowercase();
    if s.is_empty() || s == "all" {
        return true;
    }
    s == learning_lang
}

fn stable_hash_u64(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn topic_keyword_variants(topic: &str) -> Vec<String> {
    let raw = topic.trim().to_lowercase();
    if raw.is_empty() {
        return Vec::new();
    }
    let domain_aliases: [(&str, &[&str]); 14] = [
        ("新闻", &["news", "current events", "world news"]),
        ("科技", &["tech", "technology", "ai", "digital"]),
        ("科学", &["science", "research", "discovery"]),
        ("数学", &["math", "mathematics", "algebra", "geometry"]),
        ("物理", &["physics", "quantum", "mechanics"]),
        ("历史", &["history", "ancient history", "modern history"]),
        ("地理", &["geography", "earth", "places"]),
        ("文化", &["culture", "society", "humanities"]),
        ("宗教", &["religion", "faith", "belief"]),
        ("生活", &["life", "lifestyle", "daily life"]),
        ("交通", &["transport", "traffic", "mobility"]),
        ("旅游", &["travel", "tourism", "trip"]),
        ("音乐", &["music", "song", "concert"]),
        ("电影", &["movie", "film", "cinema"]),
    ];
    let mut out = Vec::new();
    out.push(raw.clone());
    for (zh, aliases) in domain_aliases {
        if raw.contains(zh) || aliases.iter().any(|x| raw.contains(x)) {
            out.push(zh.to_string());
            for alias in aliases {
                out.push(alias.to_string());
            }
        }
    }
    let chunks = raw
        .split(|c: char| c.is_whitespace() || c == ',' || c == '，' || c == ';' || c == '；' || c == '/' || c == '|')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty());
    for chunk in chunks {
        out.push(chunk.to_string());
    }
    let mut uniq = Vec::new();
    for item in out {
        let key = item.trim().to_lowercase();
        if key.len() < 2 {
            continue;
        }
        if !uniq.iter().any(|x: &String| x == &key) {
            uniq.push(key);
        }
    }
    uniq
}

fn topic_matches_text(text_lc: &str, topic: &str) -> bool {
    let variants = topic_keyword_variants(topic);
    if variants.is_empty() {
        return true;
    }
    variants.iter().any(|kw| text_lc.contains(kw))
}

fn duration_preference_weight(duration_sec: f64) -> f64 {
    if duration_sec <= 0.0 {
        return 1.0;
    }
    if (600.0..=1200.0).contains(&duration_sec) {
        return 1.25;
    }
    if (1200.0..=1800.0).contains(&duration_sec) {
        return 0.88;
    }
    if duration_sec < 600.0 {
        return 0.82;
    }
    0.78
}

fn recency_preference_weight(pub_ts: Option<i64>) -> f64 {
    let ts = match pub_ts {
        Some(v) if v > 0 => v,
        _ => return 1.0,
    };
    let now = chrono::Utc::now().timestamp();
    let age_days = ((now - ts).max(0) as f64) / 86400.0;
    if age_days <= 2.0 {
        return 1.34;
    }
    if age_days <= 7.0 {
        return 1.2;
    }
    if age_days <= 30.0 {
        return 1.06;
    }
    0.9
}

fn parse_pub_datetime_to_ts(raw: &str) -> Option<i64> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(text) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(dt.timestamp());
    }
    None
}

fn strip_html_tags(text: &str) -> String {
    let re = Regex::new(r"(?is)<[^>]+>").ok();
    let no_tags = if let Some(re) = re {
        re.replace_all(text, " ").to_string()
    } else {
        text.to_string()
    };
    no_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_media_uri(raw: &str) -> String {
    let cleaned = raw
        .trim()
        .trim_matches(|c: char| c == '`' || c == '"' || c == '\'' || c == '<' || c == '>')
        .trim()
        .to_string();
    if cleaned.to_ascii_lowercase().starts_with("file://") {
        let mut stripped = cleaned.as_str();
        if cleaned.len() >= 7 && cleaned[..7].eq_ignore_ascii_case("file://") {
            stripped = &cleaned[7..];
        }
        let stripped = stripped.trim_start_matches('/');
        if stripped.contains(':') {
            return stripped.to_string();
        }
    }
    cleaned
}

fn sync_json_has_full_translation(raw_path: &str) -> bool {
    let path = normalize_media_uri(raw_path);
    if path.is_empty() {
        info!("sync_json_has_full_translation: empty path");
        return false;
    }
    let p = PathBuf::from(path);
    if !p.exists() || !p.is_file() {
        info!(
            raw_path = raw_path,
            normalized_path = p.to_string_lossy().as_ref(),
            "sync_json_has_full_translation: path missing"
        );
        return false;
    }
    let text = match fs::read_to_string(&p) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                normalized_path = p.to_string_lossy().as_ref(),
                error = e.to_string().as_str(),
                "sync_json_has_full_translation: read failed"
            );
            return false;
        }
    };
    let payload: WhisperSyncPayload = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warn!(
                normalized_path = p.to_string_lossy().as_ref(),
                error = e.to_string().as_str(),
                "sync_json_has_full_translation: parse failed"
            );
            return false;
        }
    };
    if payload.segments.is_empty() {
        return false;
    }
    let mut total = 0usize;
    let mut translated = 0usize;
    for seg in payload.segments {
        let source = seg.text.trim().to_string();
        if source.is_empty() {
            continue;
        }
        total += 1;
        let target = seg.translated_text.trim().to_string();
        if target.is_empty() || target.contains("[Translation Failed]") {
            continue;
        }
        if target == source {
            continue;
        }
        translated += 1;
    }
    if total == 0 {
        info!(
            normalized_path = p.to_string_lossy().as_ref(),
            "sync_json_has_full_translation: no valid source segment"
        );
        return false;
    }
    let ratio = (translated as f64) / (total as f64);
    info!(
        normalized_path = p.to_string_lossy().as_ref(),
        translated,
        total,
        ratio,
        "sync_json_has_full_translation: computed ratio"
    );
    ratio >= 0.98
}

fn load_audio_resource_by_id(
    conn: &Connection,
    table: &str,
    id: &str,
) -> Result<Option<AudioResource>, String> {
    ensure_audio_table_columns(conn, table);
    let sql = format!(
        r#"
        SELECT id, title, source_kind, source_uri, provider, topic, language_tag, transcript, sync_json_uri, notes, duration_sec, created_at, updated_at
        FROM {table}
        WHERE id = ?1
        LIMIT 1
        "#,
        table = table
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("准备读取听力条目失败: {}", e))?;
    stmt.query_row(params![id], |r| {
        Ok(AudioResource {
            id: r.get(0)?,
            title: r.get(1)?,
            source_kind: r.get(2)?,
            source_uri: r.get(3)?,
            provider: r.get(4)?,
            topic: r.get(5)?,
            language_tag: r.get(6)?,
            transcript: r.get(7)?,
            sync_json_uri: r.get(8)?,
            notes: r.get(9)?,
            duration_sec: r.get(10)?,
            created_at: r.get(11)?,
            updated_at: r.get(12)?,
        })
    })
    .optional()
    .map_err(|e| format!("读取听力条目失败: {}", e))
}

fn ensure_audio_table_columns(conn: &Connection, table: &str) {
    for sql in [
        format!(
            "ALTER TABLE {} ADD COLUMN provider TEXT NOT NULL DEFAULT ''",
            table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN topic TEXT NOT NULL DEFAULT ''",
            table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN language_tag TEXT NOT NULL DEFAULT ''",
            table
        ),
        format!(
            "ALTER TABLE {} ADD COLUMN sync_json_uri TEXT NOT NULL DEFAULT ''",
            table
        ),
    ] {
        let _ = conn.execute(&sql, []);
    }
}

fn list_audio_tables(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT name
            FROM sqlite_master
            WHERE type = 'table' AND name LIKE 'audio_resources_%'
            ORDER BY name ASC
            "#,
        )
        .map_err(|e| format!("读取音频表列表失败: {}", e))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| format!("查询音频表列表失败: {}", e))?;
    let mut tables = Vec::new();
    for row in rows {
        tables.push(row.map_err(|e| format!("读取音频表名称失败: {}", e))?);
    }
    Ok(tables)
}

fn load_audio_resource_with_fallback_lang(
    conn: &Connection,
    audio_id: &str,
    preferred_lang: &str,
) -> Result<Option<(String, AudioResource)>, String> {
    let preferred_lang = normalize_learning_lang(Some(preferred_lang.to_string()));
    let preferred_table = audio_table_for(&preferred_lang);
    if let Some(row) = load_audio_resource_by_id(conn, &preferred_table, audio_id)? {
        return Ok(Some((preferred_lang, row)));
    }
    let mut tables = list_audio_tables(conn)?;
    tables.retain(|t| t != &preferred_table);
    for table in tables {
        if let Some(row) = load_audio_resource_by_id(conn, &table, audio_id)? {
            let lang = table
                .strip_prefix("audio_resources_")
                .unwrap_or_default()
                .replace('_', "-");
            let resolved = if lang.trim().is_empty() {
                preferred_lang.clone()
            } else {
                normalize_learning_lang(Some(lang))
            };
            return Ok(Some((resolved, row)));
        }
    }
    Ok(None)
}

fn resolve_listening_store_root() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?
        .to_path_buf();
    let root = exe_dir.join(LISTENING_ITEMS_DIR_NAME);
    if !root.exists() {
        fs::create_dir_all(&root).map_err(|e| format!("创建听力存储目录失败: {}", e))?;
    }
    Ok(root)
}

fn resolve_listening_entry_dir(row: &AudioResource) -> Result<PathBuf, String> {
    if let Some(existing) = resolve_listening_entry_dir_by_id(&row.id)? {
        if existing.exists() && existing.is_dir() {
            return Ok(existing);
        }
    }
    let root = resolve_listening_store_root()?;
    let short_id: String = row.id.chars().take(8).collect();
    let entry = root.join(format!("item_{}", short_id));
    if !entry.exists() {
        fs::create_dir_all(&entry).map_err(|e| format!("创建听力条目目录失败: {}", e))?;
    }
    Ok(entry)
}

fn resolve_listening_entry_dir_by_id(id: &str) -> Result<Option<PathBuf>, String> {
    let root = resolve_listening_store_root()?;
    let short_id: String = id.chars().take(8).collect();
    if short_id.is_empty() {
        return Ok(None);
    }
    let suffix = format!("_{}", short_id);
    let entries = fs::read_dir(&root).map_err(|e| format!("读取听力存储目录失败: {}", e))?;
    for it in entries {
        let entry = match it {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == format!("item_{}", short_id) || name.ends_with(&suffix) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn cleanup_listening_entry_files(row: &AudioResource) -> Result<(), String> {
    let mut paths = Vec::new();
    if let Some(dir) = resolve_listening_entry_dir_by_id(&row.id)? {
        paths.push(dir);
    }
    let source = normalize_media_uri(&row.source_uri);
    if !source.is_empty() && !source.starts_with("http://") && !source.starts_with("https://") {
        let p = PathBuf::from(&source);
        if p.exists() {
            paths.push(p.clone());
            if let Some(parent) = p.parent() {
                if parent
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("listening_"))
                    .unwrap_or(false)
                {
                    paths.push(parent.to_path_buf());
                }
            }
        }
    }
    let sync_json = normalize_media_uri(&row.sync_json_uri);
    if !sync_json.is_empty() {
        let p = PathBuf::from(sync_json);
        if p.exists() {
            paths.push(p);
        }
    }
    paths.sort();
    paths.dedup();
    for p in paths {
        if p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        } else if p.is_file() {
            let _ = fs::remove_file(&p);
        }
    }
    Ok(())
}

fn infer_ext_from_uri(raw: &str) -> String {
    let cleaned = normalize_media_uri(raw);
    let uri = cleaned.split('?').next().unwrap_or_default();
    let p = Path::new(uri);
    let ext = p
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if ext.is_empty() {
        "mp3".to_string()
    } else {
        ext
    }
}

async fn prepare_entry_audio_file(row: &AudioResource, entry_dir: &Path) -> Result<PathBuf, String> {
    if !entry_dir.exists() {
        fs::create_dir_all(entry_dir).map_err(|e| format!("创建听力目录失败: {}", e))?;
    }
    let ext = infer_ext_from_uri(&row.source_uri);
    let audio_path = entry_dir.join(format!("media.{}", ext));
    let source = normalize_media_uri(&row.source_uri);
    if source.starts_with("http://") || source.starts_with("https://") {
        let client = Client::builder()
            .user_agent("Mozilla/5.0")
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("初始化下载客户端失败: {}", e))?;
        let resp = client
            .get(source.as_str())
            .send()
            .await
            .map_err(|e| format!("下载音频失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("下载音频失败，状态码: {}", resp.status()));
        }
        let data = resp
            .bytes()
            .await
            .map_err(|e| format!("读取音频数据失败: {}", e))?;
        fs::write(&audio_path, &data).map_err(|e| format!("保存音频文件失败: {}", e))?;
        return Ok(audio_path);
    }
    let source_path = PathBuf::from(source);
    if !source_path.exists() {
        return Err("本地音频文件不存在".to_string());
    }
    let same = source_path
        .canonicalize()
        .ok()
        .zip(audio_path.canonicalize().ok())
        .map(|(a, b)| a == b)
        .unwrap_or(false);
    if !same {
        fs::copy(&source_path, &audio_path).map_err(|e| format!("复制音频文件失败: {}", e))?;
    }
    Ok(audio_path)
}

fn parse_ts_to_sec(raw: &str) -> Option<f64> {
    let t = raw.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    let parts = t.split(':').collect::<Vec<_>>();
    if parts.len() == 3 {
        let h = parts[0].trim().parse::<f64>().ok()?;
        let m = parts[1].trim().parse::<f64>().ok()?;
        let s = parts[2].trim().parse::<f64>().ok()?;
        return Some(h * 3600.0 + m * 60.0 + s);
    }
    if parts.len() == 2 {
        let m = parts[0].trim().parse::<f64>().ok()?;
        let s = parts[1].trim().parse::<f64>().ok()?;
        return Some(m * 60.0 + s);
    }
    parts[0].trim().parse::<f64>().ok()
}

fn extract_whisper_segments(v: &serde_json::Value) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    if let Some(arr) = v.get("segments").and_then(|x| x.as_array()) {
        for item in arr {
            let start = item.get("start").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let end = item.get("end").and_then(|x| x.as_f64()).unwrap_or(start + 1.2);
            let text = item
                .get("text")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            if !text.is_empty() {
                out.push((start.max(0.0), end.max(start + 0.4), text));
            }
        }
    }
    if out.is_empty() {
        if let Some(arr) = v.get("transcription").and_then(|x| x.as_array()) {
            for item in arr {
                let mut start = item
                    .get("offsets")
                    .and_then(|x| x.get("from"))
                    .and_then(|x| x.as_f64())
                    .map(|x| x / 1000.0)
                    .unwrap_or(0.0);
                let mut end = item
                    .get("offsets")
                    .and_then(|x| x.get("to"))
                    .and_then(|x| x.as_f64())
                    .map(|x| x / 1000.0)
                    .unwrap_or(start + 1.2);
                if let Some(from) = item
                    .get("timestamps")
                    .and_then(|x| x.get("from"))
                    .and_then(|x| x.as_str())
                    .and_then(parse_ts_to_sec)
                {
                    start = from;
                }
                if let Some(to) = item
                    .get("timestamps")
                    .and_then(|x| x.get("to"))
                    .and_then(|x| x.as_str())
                    .and_then(parse_ts_to_sec)
                {
                    end = to;
                }
                let text = item
                    .get("text")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    out.push((start.max(0.0), end.max(start + 0.4), text));
                }
            }
        }
    }
    out
}

fn resolve_whisper_cli_and_model() -> Result<(PathBuf, PathBuf), String> {
    let whisper_cli = if cfg!(target_os = "windows") {
        bundled_resources::ensure_relative_dir("utils/whisper/whisper-cli.exe")?
    } else {
        bundled_resources::ensure_relative_dir("utils/whisper/whisper-cli")?
    };
    if !whisper_cli.exists() {
        return Err(format!(
            "未找到 whisper 可执行文件: {}",
            whisper_cli.to_string_lossy()
        ));
    }
    let model_en = bundled_resources::ensure_relative_dir("utils/Models/whisper_models/ggml-base.en.bin")?;
    let model_base = bundled_resources::ensure_relative_dir("utils/Models/whisper_models/ggml-base.bin")?;
    let model = if model_en.exists() { model_en } else { model_base };
    if !model.exists() {
        return Err(format!(
            "未找到 whisper 模型文件: {}",
            model.to_string_lossy()
        ));
    }
    Ok((whisper_cli, model))
}

async fn run_whisper_and_build_sync_json(local_audio: &Path, entry_dir: &Path, target_lang: &str) -> Result<PathBuf, String> {
    if !entry_dir.exists() {
        fs::create_dir_all(entry_dir).map_err(|e| format!("创建听力目录失败: {}", e))?;
    }
    let (whisper_cli, model_path) = resolve_whisper_cli_and_model()?;
    let output_base = entry_dir.join("media");
    let mut cmd = Command::new(&whisper_cli);
    cmd.arg("-m")
        .arg(model_path.to_string_lossy().to_string())
        .arg("-f")
        .arg(local_audio.to_string_lossy().to_string())
        .arg("-oj")
        .arg("-of")
        .arg(output_base.to_string_lossy().to_string())
        .arg("--language")
        .arg("en");
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("执行 whisper 失败: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("whisper 转写失败: {}", stderr));
    }
    let whisper_json = {
        let primary = entry_dir.join("media.json");
        if primary.exists() {
            primary
        } else {
            let mut fallback: Option<PathBuf> = None;
            let entries = fs::read_dir(entry_dir).map_err(|e| format!("读取听力目录失败: {}", e))?;
            for it in entries {
                let entry = match it {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let is_json = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case("json"))
                    .unwrap_or(false);
                if is_json {
                    fallback = Some(p);
                    break;
                }
            }
            fallback.ok_or_else(|| "whisper 未生成 JSON 文件".to_string())?
        }
    };
    let raw = fs::read_to_string(&whisper_json).map_err(|e| format!("读取 whisper 输出失败: {}", e))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("解析 whisper 输出失败: {}", e))?;
    let parsed = extract_whisper_segments(&v);
    if parsed.is_empty() {
        return Err("whisper 输出不包含可用分段".to_string());
    }
    let mut segments = Vec::new();
    let mut transcript_lines = Vec::new();
    let source_lines = parsed.iter().map(|(_, _, text)| text.clone()).collect::<Vec<_>>();
    let translated_lines = translate_segments_with_sentence_groups(&source_lines, target_lang).await;
    for (idx, (start, end, text)) in parsed.iter().enumerate() {
        transcript_lines.push(text.clone());
        segments.push(WhisperSyncSegment {
            index: idx,
            start_sec: *start,
            end_sec: *end,
            text: text.clone(),
            translated_text: translated_lines
                .get(idx)
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .unwrap_or_default(),
        });
    }
    let payload = WhisperSyncPayload {
        version: "1.0".to_string(),
        generated_at: now_iso(),
        model_path: model_path.to_string_lossy().to_string(),
        audio_path: local_audio.to_string_lossy().to_string(),
        transcript: transcript_lines.join("\n"),
        segments,
    };
    let formatted =
        serde_json::to_string_pretty(&payload).map_err(|e| format!("构建同步 JSON 失败: {}", e))?;
    if !entry_dir.exists() {
        fs::create_dir_all(entry_dir).map_err(|e| format!("创建听力目录失败: {}", e))?;
    }
    fs::write(&whisper_json, formatted).map_err(|e| format!("写入同步 JSON 失败: {}", e))?;
    Ok(whisper_json)
}

fn looks_like_audio_url(url: &str) -> bool {
    let u = url.trim().to_lowercase();
    u.ends_with(".mp3")
        || u.ends_with(".m4a")
        || u.ends_with(".wav")
        || u.ends_with(".flac")
        || u.ends_with(".ogg")
        || u.contains(".mp3?")
        || u.contains(".m4a?")
}

fn extract_first_url(text: &str) -> Option<String> {
    let re = Regex::new(r#"https?://[^\s"'<>()]+"#).ok()?;
    re.find(text)
        .map(|m| m.as_str().trim().trim_end_matches(|c: char| c == ')' || c == ',' || c == ';').to_string())
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn contains_sentence_like_content(text: &str) -> bool {
    let t = text.trim();
    if t.chars().count() < 20 {
        return false;
    }
    t.contains(' ') || t.contains('。') || t.contains('.') || t.contains('？') || t.contains('?')
}

fn split_long_text_to_lines(text: &str) -> String {
    let mut out = Vec::new();
    let mut current = String::new();
    for part in text.split(|c| c == '.' || c == '!' || c == '?' || c == '。' || c == '！' || c == '？') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if current.len() + p.len() + 1 > 180 && !current.is_empty() {
            out.push(current.trim().to_string());
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(p);
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    if out.is_empty() {
        text.to_string()
    } else {
        out.join("\n")
    }
}

fn parse_duration_text(raw: &str) -> Option<f64> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        return t.parse::<f64>().ok();
    }
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() == 2 {
        let m = parts[0].trim().parse::<f64>().ok()?;
        let s = parts[1].trim().parse::<f64>().ok()?;
        return Some(m * 60.0 + s);
    }
    if parts.len() == 3 {
        let h = parts[0].trim().parse::<f64>().ok()?;
        let m = parts[1].trim().parse::<f64>().ok()?;
        let s = parts[2].trim().parse::<f64>().ok()?;
        return Some(h * 3600.0 + m * 60.0 + s);
    }
    None
}

async fn fetch_json(client: &Client, url: &str) -> Option<serde_json::Value> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<serde_json::Value>().await.ok()
}

fn clean_word(word: &str) -> String {
    normalize_word(word)
}

fn pick_phonetics(item: &serde_json::Value) -> (String, String, String) {
    let mut any = String::new();
    let mut uk = String::new();
    let mut us = String::new();
    if let Some(arr) = item.get("phonetics").and_then(|v| v.as_array()) {
        for p in arr {
            if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                let s = t.trim();
                if !s.is_empty() {
                    if any.is_empty() {
                        any = s.to_string();
                    }
                    let audio = p
                        .get("audio")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_lowercase();
                    if uk.is_empty() && (audio.contains("uk") || audio.contains("en-gb")) {
                        uk = s.to_string();
                    }
                    if us.is_empty() && (audio.contains("us") || audio.contains("en-us")) {
                        us = s.to_string();
                    }
                }
            }
        }
    }
    if uk.is_empty() {
        uk = any.clone();
    }
    if us.is_empty() {
        us = any.clone();
    }
    (any, uk, us)
}

fn split_simple_translations(text: &str) -> Vec<String> {
    let seps = [';', '；', ',', '，', '、', '/', '|', '｜'];
    let mut out = Vec::new();
    for part in text.split(|c| seps.contains(&c)) {
        let p = part.trim().trim_matches(|c: char| c == '.' || c == '。');
        if p.is_empty() {
            continue;
        }
        if p.chars().count() > 16 {
            continue;
        }
        if !out.iter().any(|x: &String| x == p) {
            out.push(p.to_string());
        }
        if out.len() >= 5 {
            break;
        }
    }
    out
}

fn collect_phrases_from_edges(v: Option<serde_json::Value>, tail_word: &str, prefix: bool) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(arr) = v.and_then(|x| x.as_array().cloned()) {
        for item in arr.iter().take(18) {
            let w = item
                .get("word")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if w.is_empty() || w == tail_word {
                continue;
            }
            let phrase = if prefix {
                format!("{} {}", tail_word, w)
            } else {
                format!("{} {}", w, tail_word)
            };
            out.push(phrase);
        }
    }
    dedup_limit(out, 10)
}

fn collect_meanings(item: &serde_json::Value) -> (String, Vec<String>, Vec<String>, Vec<String>) {
    let mut part_of_speech = String::new();
    let mut meanings = Vec::new();
    let mut examples = Vec::new();
    let mut synonyms = Vec::new();

    if let Some(m_arr) = item.get("meanings").and_then(|v| v.as_array()) {
        for m in m_arr {
            if part_of_speech.is_empty() {
                if let Some(pos) = m.get("partOfSpeech").and_then(|v| v.as_str()) {
                    let s = pos.trim();
                    if !s.is_empty() {
                        part_of_speech = s.to_string();
                    }
                }
            }
            if let Some(defs) = m.get("definitions").and_then(|v| v.as_array()) {
                for d in defs.iter().take(6) {
                    if let Some(def) = d.get("definition").and_then(|v| v.as_str()) {
                        let s = def.trim();
                        if !s.is_empty() {
                            meanings.push(s.to_string());
                        }
                    }
                    if let Some(ex) = d.get("example").and_then(|v| v.as_str()) {
                        let s = ex.trim();
                        if !s.is_empty() {
                            examples.push(s.to_string());
                        }
                    }
                }
            }
            if let Some(syns) = m.get("synonyms").and_then(|v| v.as_array()) {
                for s in syns.iter().take(24) {
                    if let Some(x) = s.as_str() {
                        let t = x.trim();
                        if !t.is_empty() {
                            synonyms.push(t.to_string());
                        }
                    }
                }
            }
        }
    }

    (part_of_speech, meanings, examples, synonyms)
}

fn fallback_examples(word: &str, phrases: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(p) = phrases.first() {
        out.push(p.trim().to_string());
    }
    if out.is_empty() && !word.trim().is_empty() {
        out.push(format!("This is a {}.", word.trim()));
    }
    if out.is_empty() {
        out.push("This is commonly used in daily communication.".to_string());
    }
    dedup_limit(out, 3)
}

fn dedup_limit(mut arr: Vec<String>, limit: usize) -> Vec<String> {
    arr.retain(|s| !s.trim().is_empty());
    let mut out = Vec::new();
    for s in arr {
        if !out.iter().any(|x: &String| x.eq_ignore_ascii_case(&s)) {
            out.push(s);
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn env_or_default(key: &str, default_value: &str) -> String {
    let raw = env::var(key).unwrap_or_default();
    let v = raw.trim();
    if v.is_empty() {
        default_value.to_string()
    } else {
        v.trim_end_matches('/').to_string()
    }
}

fn cn_accel_enabled(target_lang: &str) -> bool {
    let flag = env::var("GUAXI_CN_ACCEL").unwrap_or_default().to_ascii_lowercase();
    if flag == "1" || flag == "true" || flag == "yes" {
        return true;
    }
    let to = target_lang.trim().to_ascii_lowercase();
    to == "zh" || to == "zh-cn" || to == "zh_hans" || to == "zh-hans"
}

async fn translate_aligned(texts: &[String], target_lang: &str) -> Vec<String> {
    if texts.is_empty() {
        return Vec::new();
    }
    let mut out = vec![String::new(); texts.len()];
    let resp = translator_free::translate_text_free(
        texts.to_vec(),
        Some("auto".into()),
        Some(target_lang.to_string()),
    )
    .await
    .ok();
    if let Some(res) = resp {
        for (idx, t) in res.translated_texts.into_iter().enumerate() {
            if idx >= out.len() {
                break;
            }
            if t.trim().is_empty() || t.contains("[Translation Failed]") {
                continue;
            }
            out[idx] = t.trim().to_string();
        }
    }
    out
}

fn is_sentence_terminal_text(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    t.ends_with('.')
        || t.ends_with('!')
        || t.ends_with('?')
        || t.ends_with('。')
        || t.ends_with('！')
        || t.ends_with('？')
        || t.ends_with("…")
}

fn build_sentence_groups_for_segment_texts(texts: &[String]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_len = 0usize;
    for (idx, raw) in texts.iter().enumerate() {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        current.push(idx);
        current_len += s.chars().count();
        let should_flush = is_sentence_terminal_text(s) || current_len >= 220 || current.len() >= 6;
        if should_flush {
            groups.push(current);
            current = Vec::new();
            current_len = 0;
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

fn split_translation_by_weights(translated: &str, weights: &[usize]) -> Vec<String> {
    if weights.is_empty() {
        return Vec::new();
    }
    if weights.len() == 1 {
        return vec![translated.trim().to_string()];
    }
    let chars = translated.trim().chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return vec![String::new(); weights.len()];
    }
    let total_chars = chars.len();
    let total_weight: usize = weights.iter().copied().sum::<usize>().max(1);
    let mut out = Vec::with_capacity(weights.len());
    let mut consumed = 0usize;
    let mut used_weight = 0usize;
    for (idx, w) in weights.iter().enumerate() {
        if idx == weights.len() - 1 {
            let tail = chars[consumed..].iter().collect::<String>().trim().to_string();
            out.push(tail);
            break;
        }
        used_weight += *w;
        let mut end = ((used_weight as f64 / total_weight as f64) * total_chars as f64).round() as usize;
        if end <= consumed {
            end = (consumed + 1).min(total_chars);
        }
        let chunk = chars[consumed..end].iter().collect::<String>().trim().to_string();
        out.push(chunk);
        consumed = end;
    }
    out
}

async fn translate_segments_with_sentence_groups(texts: &[String], target_lang: &str) -> Vec<String> {
    if texts.is_empty() {
        return Vec::new();
    }
    let mut out = vec![String::new(); texts.len()];
    let groups = build_sentence_groups_for_segment_texts(texts);
    if groups.is_empty() {
        return out;
    }
    let mut sentence_sources = Vec::with_capacity(groups.len());
    let mut sentence_weights = Vec::with_capacity(groups.len());
    for group in &groups {
        let mut merged = Vec::new();
        let mut weights = Vec::new();
        for idx in group {
            let s = texts.get(*idx).map(|x| x.trim()).unwrap_or("");
            if s.is_empty() {
                continue;
            }
            merged.push(s.to_string());
            weights.push(s.chars().count().max(1));
        }
        sentence_sources.push(merged.join(" "));
        sentence_weights.push(weights);
    }
    let sentence_translated = translate_aligned(&sentence_sources, target_lang).await;
    for (gidx, group) in groups.iter().enumerate() {
        let translated_sentence = sentence_translated
            .get(gidx)
            .map(|x| x.trim().to_string())
            .unwrap_or_default();
        if translated_sentence.is_empty() {
            continue;
        }
        let weights = sentence_weights.get(gidx).cloned().unwrap_or_default();
        let split = split_translation_by_weights(&translated_sentence, &weights);
        for (local_idx, seg_idx) in group.iter().enumerate() {
            if let Some(slot) = out.get_mut(*seg_idx) {
                *slot = split.get(local_idx).cloned().unwrap_or_else(|| translated_sentence.clone());
            }
        }
    }
    out
}

#[tauri::command]
pub async fn language_learning_enrich_word_basic(
    word: String,
    source_lang: Option<String>,
    target_lang: Option<String>,
) -> Result<WordEnrichment, String> {
    let clean = clean_word(&word);
    if clean.is_empty() {
        return Err("单词为空".into());
    }
    let to = target_lang.clone().unwrap_or_else(|| "zh".into());
    let cn_accel = cn_accel_enabled(&to);
    let (connect_timeout_s, request_timeout_s) = if cn_accel { (4, 8) } else { (8, 12) };

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(connect_timeout_s))
        .timeout(std::time::Duration::from_secs(request_timeout_s))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {}", e))?;

    let dict_base = env_or_default("GUAXI_DICT_API_BASE", "https://api.dictionaryapi.dev/api/v2/entries/en");
    let dict_url = format!("{}/{}", dict_base, encode(&clean));
    let dict_json = fetch_json(&client, &dict_url).await;

    let mut phonetic = String::new();
    let mut phonetic_uk = String::new();
    let mut phonetic_us = String::new();
    let mut part_of_speech = String::new();
    let mut meanings = Vec::new();
    let mut examples = Vec::new();
    let mut synonyms = Vec::new();
    let antonyms = Vec::new();

    if let Some(v) = dict_json {
        if let Some(arr) = v.as_array() {
            for item in arr.iter().take(3) {
                let (p, p_uk, p_us) = pick_phonetics(item);
                if phonetic.is_empty() {
                    phonetic = p;
                }
                if phonetic_uk.is_empty() {
                    phonetic_uk = p_uk;
                }
                if phonetic_us.is_empty() {
                    phonetic_us = p_us;
                }
                let (pos, defs, exs, syns) = collect_meanings(item);
                if part_of_speech.is_empty() {
                    part_of_speech = pos;
                }
                meanings.extend(defs);
                examples.extend(exs);
                synonyms.extend(syns);
            }
        }
    } else {
        warn!("[LanguageLearning] dictionary fetch failed for {}", clean);
    }

    let from = source_lang.unwrap_or_else(|| "auto".into());
    let meanings = dedup_limit(meanings, 12);
    let synonyms = dedup_limit(synonyms, 12);

    let translated = translator_free::translate_text_free(vec![clean.clone()], Some(from), Some(to))
        .await
        .ok()
        .and_then(|res| res.translated_texts.first().cloned())
        .unwrap_or_default();

    let simple_translations = split_simple_translations(&translated);
    let meanings_translated = if simple_translations.is_empty() {
        dedup_limit(meanings.clone(), 5)
    } else {
        Vec::new()
    };
    let meanings_simple = if simple_translations.is_empty() {
        if meanings_translated.is_empty() {
            meanings.into_iter().take(5).collect::<Vec<_>>()
        } else {
            dedup_limit(meanings_translated, 5)
        }
    } else {
        simple_translations
    };

    let enrichment = WordEnrichment {
        word: clean,
        phonetic,
        phonetic_uk,
        phonetic_us,
        part_of_speech,
        meanings: meanings_simple,
        translation: translated,
        synonyms: synonyms.clone(),
        antonyms,
        associates: Vec::new(),
        synonyms_translations: Vec::new(),
        antonyms_translations: Vec::new(),
        associates_translations: Vec::new(),
        phrases: Vec::new(),
        phrase_translations: Vec::new(),
        examples,
        example_translations: Vec::new(),
    };
    Ok(enrichment)
}

#[tauri::command]
pub async fn language_learning_enrich_word_details(
    word: String,
    _source_lang: Option<String>,
    target_lang: Option<String>,
) -> Result<WordEnrichment, String> {
    let clean = clean_word(&word);
    if clean.is_empty() {
        return Err("单词为空".into());
    }
    let to = target_lang.clone().unwrap_or_else(|| "zh".into());
    let cn_accel = cn_accel_enabled(&to);
    let (connect_timeout_s, request_timeout_s) = if cn_accel { (4, 8) } else { (8, 12) };

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(connect_timeout_s))
        .timeout(std::time::Duration::from_secs(request_timeout_s))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {}", e))?;

    let dict_base = env_or_default("GUAXI_DICT_API_BASE", "https://api.dictionaryapi.dev/api/v2/entries/en");
    let datamuse_base = env_or_default("GUAXI_DATAMUSE_API_BASE", "https://api.datamuse.com/words");
    let dict_url = format!("{}/{}", dict_base, encode(&clean));
    let syn_url = format!("{}?ml={}&max=16", datamuse_base, encode(&clean));
    let ant_url = format!("{}?rel_ant={}&max=16", datamuse_base, encode(&clean));
    let bga_url = format!("{}?rel_bga={}&max=10", datamuse_base, encode(&clean));
    let bgb_url = format!("{}?rel_bgb={}&max=10", datamuse_base, encode(&clean));

    let (dict_json, syn_json, ant_json, bga_json, bgb_json) = tokio::join!(
        fetch_json(&client, &dict_url),
        fetch_json(&client, &syn_url),
        fetch_json(&client, &ant_url),
        fetch_json(&client, &bga_url),
        fetch_json(&client, &bgb_url)
    );

    let mut examples = Vec::new();
    let mut synonyms = Vec::new();
    let mut antonyms = Vec::new();
    if let Some(v) = dict_json {
        if let Some(arr) = v.as_array() {
            for item in arr.iter().take(3) {
                let (_, _, _, syns) = collect_meanings(item);
                let (_, _, exs, _) = collect_meanings(item);
                examples.extend(exs);
                synonyms.extend(syns);
            }
        }
    }
    if let Some(v) = syn_json {
        if let Some(arr) = v.as_array() {
            for item in arr.iter().take(24) {
                if let Some(w) = item.get("word").and_then(|x| x.as_str()) {
                    let t = w.trim();
                    if !t.is_empty() {
                        synonyms.push(t.to_string());
                    }
                }
            }
        }
    }
    if let Some(v) = ant_json {
        if let Some(arr) = v.as_array() {
            for item in arr.iter().take(24) {
                if let Some(w) = item.get("word").and_then(|x| x.as_str()) {
                    let t = w.trim();
                    if !t.is_empty() {
                        antonyms.push(t.to_string());
                    }
                }
            }
        }
    }

    let target_lang_for_defs = if to.trim().is_empty() { "zh".to_string() } else { to.clone() };
    let synonyms = dedup_limit(synonyms, 12);
    let antonyms = dedup_limit(antonyms, 12);
    let associates = {
        let mut out = Vec::new();
        for x in synonyms.iter().skip(10).take(12) {
            out.push(x.clone());
        }
        dedup_limit(out, 12)
    };
    let mut phrases = collect_phrases_from_edges(bga_json, &clean, false);
    phrases.extend(collect_phrases_from_edges(bgb_json, &clean, true));
    phrases = dedup_limit(phrases, 12);
    let mut examples = dedup_limit(examples, 12);
    if examples.is_empty() {
        examples = fallback_examples(&clean, &phrases);
    }

    let examples_c = examples.clone();
    let synonyms_c = synonyms.clone();
    let antonyms_c = antonyms.clone();
    let associates_c = associates.clone();
    let phrases_c = phrases.clone();
    let t1 = target_lang_for_defs.clone();
    let t2 = target_lang_for_defs.clone();
    let t3 = target_lang_for_defs.clone();
    let t4 = target_lang_for_defs.clone();
    let t5 = target_lang_for_defs.clone();
    let ex_task = tokio::spawn(async move { translate_aligned(&examples_c, &t1).await });
    let syn_task = tokio::spawn(async move { translate_aligned(&synonyms_c, &t2).await });
    let ant_task = tokio::spawn(async move { translate_aligned(&antonyms_c, &t3).await });
    let asc_task = tokio::spawn(async move { translate_aligned(&associates_c, &t4).await });
    let phr_task = tokio::spawn(async move { translate_aligned(&phrases_c, &t5).await });

    let example_translations = ex_task.await.unwrap_or_default();
    let synonyms_translations = syn_task.await.unwrap_or_default();
    let antonyms_translations = ant_task.await.unwrap_or_default();
    let associates_translations = asc_task.await.unwrap_or_default();
    let phrase_translations = phr_task.await.unwrap_or_default();

    Ok(WordEnrichment {
        word: clean,
        phonetic: String::new(),
        phonetic_uk: String::new(),
        phonetic_us: String::new(),
        part_of_speech: String::new(),
        meanings: Vec::new(),
        translation: String::new(),
        synonyms,
        antonyms,
        associates,
        synonyms_translations,
        antonyms_translations,
        associates_translations,
        phrases,
        phrase_translations,
        examples,
        example_translations,
    })
}

#[tauri::command]
pub async fn language_learning_enrich_word(
    word: String,
    source_lang: Option<String>,
    target_lang: Option<String>,
) -> Result<WordEnrichment, String> {
    let mut basic = language_learning_enrich_word_basic(word.clone(), source_lang.clone(), target_lang.clone()).await?;
    if let Ok(details) = language_learning_enrich_word_details(word, source_lang, target_lang).await {
        basic.synonyms = details.synonyms;
        basic.antonyms = details.antonyms;
        basic.associates = details.associates;
        basic.synonyms_translations = details.synonyms_translations;
        basic.antonyms_translations = details.antonyms_translations;
        basic.associates_translations = details.associates_translations;
        basic.phrases = details.phrases;
        basic.phrase_translations = details.phrase_translations;
        basic.examples = details.examples;
        basic.example_translations = details.example_translations;
    }
    Ok(basic)
}
