use crate::commands::translator_free;
use keyring::Entry;
use regex::Regex;
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tracing::{info, warn};
use urlencoding::encode;
use uuid::Uuid;

const LANG_LEARN_DIR_NAME: &str = "guaxi_language_learning";
const LANG_LEARN_DB_NAME: &str = "language_learning.sqlite3";
const KEKENET_KEYRING_SERVICE: &str = "guaxi_language_learning_kekenet";

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
    pub transcript: Option<String>,
    pub notes: Option<String>,
    pub duration_sec: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioResource {
    pub id: String,
    pub title: String,
    pub source_kind: String,
    pub source_uri: String,
    pub transcript: String,
    pub notes: String,
    pub duration_sec: f64,
    pub created_at: String,
    pub updated_at: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KekenetCredentialsInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KekenetCredentialsOutput {
    pub username: String,
    pub has_password: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KekenetImportInput {
    pub page_url: String,
    pub title: Option<String>,
    pub transcript: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LearningCounts {
    pub records: i64,
    pub audio_resources: i64,
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
            transcript TEXT NOT NULL DEFAULT '',
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
            transcript TEXT NOT NULL DEFAULT '',
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
            WHERE (?1 = '' OR title LIKE ?2 OR transcript LIKE ?2 OR notes LIKE ?2)
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

#[tauri::command]
pub fn language_learning_export_bundle(
    app: tauri::AppHandle,
    learning_lang: Option<String>,
) -> Result<LearningDataBundle, String> {
    let lang = normalize_learning_lang(learning_lang);
    let records = language_learning_list_records(
        app.clone(),
        None,
        Some("all".to_string()),
        Some(lang.clone()),
        Some(100000),
        Some(0),
    )?;
    let audio_resources = language_learning_list_audio_resources(app, None, Some(lang.clone()))?;
    Ok(LearningDataBundle {
        learning_lang: lang,
        exported_at: now_iso(),
        records,
        audio_resources,
    })
}

#[tauri::command]
pub fn language_learning_import_bundle(
    app: tauri::AppHandle,
    bundle: LearningDataBundle,
    learning_lang: Option<String>,
) -> Result<serde_json::Value, String> {
    let target_lang = normalize_learning_lang(Some(if let Some(v) = learning_lang {
        v
    } else if bundle.learning_lang.trim().is_empty() {
        "zh".to_string()
    } else {
        bundle.learning_lang
    }));
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
            transcript: Some(a.transcript),
            notes: Some(a.notes),
            duration_sec: Some(a.duration_sec),
        };
        let _ = language_learning_upsert_audio_resource(app.clone(), input, Some(target_lang.clone()))?;
        imported_audio_resources += 1;
    }
    Ok(serde_json::json!({
        "learning_lang": target_lang,
        "imported_records": imported_records,
        "imported_audio_resources": imported_audio_resources
    }))
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
                SELECT id, title, source_kind, source_uri, transcript, notes, duration_sec, created_at, updated_at
                FROM {table}
                WHERE (?1 = '' OR title LIKE ?2 OR transcript LIKE ?2 OR notes LIKE ?2)
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
                    transcript: r.get(4)?,
                    notes: r.get(5)?,
                    duration_sec: r.get::<_, f64>(6)?,
                    created_at: r.get(7)?,
                    updated_at: r.get(8)?,
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
pub fn language_learning_upsert_audio_resource(
    app: tauri::AppHandle,
    input: AudioResourceInput,
    learning_lang: Option<String>,
) -> Result<AudioResource, String> {
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let audio_table = audio_table_for(&learning_lang);
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = now_iso();
        let title = input.title.trim().to_string();
        let source_kind = input.source_kind.trim().to_string();
        let source_uri = input.source_uri.trim().to_string();
        if title.is_empty() || source_uri.is_empty() {
            return Err("标题和资源地址不能为空".into());
        }
        let transcript = input.transcript.unwrap_or_default().trim().to_string();
        let notes = input.notes.unwrap_or_default().trim().to_string();
        let duration_sec = input.duration_sec.unwrap_or(0.0).max(0.0);

        let upsert_sql = format!(
            r#"
            INSERT INTO {table}(id, title, source_kind, source_uri, transcript, notes, duration_sec, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
              title=excluded.title,
              source_kind=excluded.source_kind,
              source_uri=excluded.source_uri,
              transcript=excluded.transcript,
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
                transcript,
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
            transcript,
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
    with_db(&app, |conn| {
        let learning_lang = normalize_learning_lang(learning_lang);
        ensure_lang_tables(conn, &learning_lang)?;
        let audio_table = audio_table_for(&learning_lang);
        let delete_sql = format!("DELETE FROM {} WHERE id = ?1", audio_table);
        conn.execute(&delete_sql, params![id])
            .map_err(|e| format!("删除音频资源失败: {}", e))?;
        Ok(())
    })
}

#[tauri::command]
pub fn language_learning_save_kekenet_credentials(
    input: KekenetCredentialsInput,
) -> Result<KekenetCredentialsOutput, String> {
    let username = input.username.trim().to_string();
    let password = input.password.trim().to_string();
    if username.is_empty() || password.is_empty() {
        return Err("账号和密码不能为空".into());
    }
    let entry = Entry::new(KEKENET_KEYRING_SERVICE, "default").map_err(|e| e.to_string())?;
    let payload = serde_json::json!({
        "username": username,
        "password": password
    })
    .to_string();
    entry.set_password(&payload).map_err(|e| e.to_string())?;
    Ok(KekenetCredentialsOutput {
        username,
        has_password: true,
    })
}

#[tauri::command]
pub fn language_learning_get_kekenet_credentials() -> Result<KekenetCredentialsOutput, String> {
    let entry = Entry::new(KEKENET_KEYRING_SERVICE, "default").map_err(|e| e.to_string())?;
    let raw = match entry.get_password() {
        Ok(v) => v,
        Err(_) => {
            return Ok(KekenetCredentialsOutput {
                username: String::new(),
                has_password: false,
            })
        }
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
    let username = parsed
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    let has_password = parsed
        .get("password")
        .and_then(|v| v.as_str())
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    Ok(KekenetCredentialsOutput {
        username,
        has_password,
    })
}

#[tauri::command]
pub async fn language_learning_import_kekenet_resource(
    app: tauri::AppHandle,
    input: KekenetImportInput,
    learning_lang: Option<String>,
) -> Result<AudioResource, String> {
    let page_url = input.page_url.trim().to_string();
    if page_url.is_empty() {
        return Err("页面地址不能为空".into());
    }
    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(6))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {}", e))?;
    let html = client
        .get(&page_url)
        .send()
        .await
        .map_err(|e| format!("抓取页面失败: {}", e))?
        .text()
        .await
        .map_err(|e| format!("读取页面内容失败: {}", e))?;
    let re_audio = Regex::new(r#"https?://[^"' <>\n\r]+?\.(?:mp3|m4a|wav)"#)
        .map_err(|e| format!("音频地址解析器初始化失败: {}", e))?;
    let source_uri = re_audio
        .find(&html)
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| page_url.clone());
    let title = if let Some(t) = input.title {
        let v = t.trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    } else {
        None
    }
    .or_else(|| {
        let re_title = Regex::new(r"(?is)<title>(.*?)</title>").ok()?;
        let caps = re_title.captures(&html)?;
        Some(caps.get(1)?.as_str().trim().to_string())
    })
    .unwrap_or_else(|| "可可英语导入内容".to_string());
    let transcript = {
        let input_text = input.transcript.unwrap_or_default().trim().to_string();
        if !input_text.is_empty() {
            input_text
        } else {
            let re_p = Regex::new(r"(?is)<p[^>]*>(.*?)</p>").ok();
            let re_tag = Regex::new(r"(?is)<[^>]+>").ok();
            let mut chunks = Vec::new();
            if let (Some(pat), Some(tag_re)) = (re_p, re_tag) {
                for cap in pat.captures_iter(&html).take(120) {
                    let raw = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
                    let clean = tag_re.replace_all(raw, "");
                    let text = clean
                        .replace("&nbsp;", " ")
                        .replace("&amp;", "&")
                        .trim()
                        .to_string();
                    if text.chars().count() >= 20 {
                        chunks.push(text);
                    }
                    if chunks.len() >= 24 {
                        break;
                    }
                }
            }
            chunks.join("\n")
        }
    };
    language_learning_upsert_audio_resource(
        app,
        AudioResourceInput {
            id: None,
            title,
            source_kind: "kekenet".to_string(),
            source_uri,
            transcript: Some(transcript),
            notes: Some("来自可可英语在线导入".to_string()),
            duration_sec: Some(0.0),
        },
        learning_lang,
    )
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
        Some("en".into()),
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

#[tauri::command]
pub async fn language_learning_enrich_word(
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

    let dict_base = env_or_default(
        "GUAXI_DICT_API_BASE",
        "https://api.dictionaryapi.dev/api/v2/entries/en",
    );
    let datamuse_base = env_or_default("GUAXI_DATAMUSE_API_BASE", "https://api.datamuse.com/words");
    let dict_url = format!(
        "{}/{}",
        dict_base,
        encode(&clean)
    );
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

    let mut phonetic = String::new();
    let mut phonetic_uk = String::new();
    let mut phonetic_us = String::new();
    let mut part_of_speech = String::new();
    let mut meanings = Vec::new();
    let mut examples = Vec::new();
    let mut synonyms = Vec::new();
    let mut antonyms = Vec::new();

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

    let from = source_lang.unwrap_or_else(|| "auto".into());
    let target_lang_for_defs = if to.trim().is_empty() {
        "zh".to_string()
    } else {
        to.clone()
    };
    let meanings = dedup_limit(meanings, 12);
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

    let translated = translator_free::translate_text_free(vec![clean.clone()], Some(from), Some(to))
        .await
        .ok()
        .and_then(|res| res.translated_texts.first().cloned())
        .unwrap_or_default();

    let simple_translations = split_simple_translations(&translated);
    let meanings_translated = if simple_translations.is_empty() {
        translate_aligned(&meanings, &target_lang_for_defs).await
    } else {
        Vec::new()
    };
    let (
        example_translations,
        synonyms_translations,
        antonyms_translations,
        associates_translations,
        phrase_translations,
    ) = tokio::join!(
        translate_aligned(&examples, &target_lang_for_defs),
        translate_aligned(&synonyms, &target_lang_for_defs),
        translate_aligned(&antonyms, &target_lang_for_defs),
        translate_aligned(&associates, &target_lang_for_defs),
        translate_aligned(&phrases, &target_lang_for_defs)
    );
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
        associates,
        synonyms_translations,
        antonyms_translations,
        associates_translations,
        phrases,
        phrase_translations,
        examples,
        example_translations,
    };
    Ok(enrichment)
}
