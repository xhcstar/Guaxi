use crate::commands::translator_free;
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
    if text.chars().any(|c| (c >= '\u{3040}' && c <= '\u{30ff}')) {
        return "ja".to_string();
    }
    if text.chars().any(|c| (c >= '\u{ac00}' && c <= '\u{d7af}')) {
        return "ko".to_string();
    }
    if text.chars().any(|c| (c >= '\u{0400}' && c <= '\u{04ff}')) {
        return "ru".to_string();
    }
    if text
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c.is_ascii_whitespace() || c == '\'' || c == '-' || c == ',' || c == '.')
    {
        return "en".to_string();
    }
    if text.chars().any(|c| (c >= '\u{4e00}' && c <= '\u{9fff}')) {
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

    let client = Client::builder()
        .user_agent("Mozilla/5.0")
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {}", e))?;

    let dict_url = format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        encode(&clean)
    );
    let syn_url = format!("https://api.datamuse.com/words?ml={}&max=16", encode(&clean));
    let ant_url = format!("https://api.datamuse.com/words?rel_ant={}&max=16", encode(&clean));
    let bga_url = format!("https://api.datamuse.com/words?rel_bga={}&max=10", encode(&clean));
    let bgb_url = format!("https://api.datamuse.com/words?rel_bgb={}&max=10", encode(&clean));

    let dict_json = fetch_json(&client, &dict_url).await;
    let syn_json = fetch_json(&client, &syn_url).await;
    let ant_json = fetch_json(&client, &ant_url).await;
    let bga_json = fetch_json(&client, &bga_url).await;
    let bgb_json = fetch_json(&client, &bgb_url).await;

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
            if let Some(first) = arr.first() {
                let (p, p_uk, p_us) = pick_phonetics(first);
                phonetic = p;
                phonetic_uk = p_uk;
                phonetic_us = p_us;
                let (pos, defs, exs, syns) = collect_meanings(first);
                part_of_speech = pos;
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
    let to = target_lang.unwrap_or_else(|| "zh".into());
    let target_lang_for_defs = if to.trim().is_empty() {
        "zh".to_string()
    } else {
        to.clone()
    };
    let meanings = dedup_limit(meanings, 12);
    let examples = dedup_limit(examples, 12);
    let synonyms = dedup_limit(synonyms, 24);
    let antonyms = dedup_limit(antonyms, 24);
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

    let translated = translator_free::translate_text_free(vec![clean.clone()], Some(from), Some(to))
        .await
        .ok()
        .and_then(|res| res.translated_texts.first().cloned())
        .unwrap_or_default();

    let meanings_translated = translate_aligned(&meanings, &target_lang_for_defs).await;
    let example_translations = translate_aligned(&examples, &target_lang_for_defs).await;
    let synonyms_translations = translate_aligned(&synonyms, &target_lang_for_defs).await;
    let antonyms_translations = translate_aligned(&antonyms, &target_lang_for_defs).await;
    let associates_translations = translate_aligned(&associates, &target_lang_for_defs).await;
    let phrase_translations = translate_aligned(&phrases, &target_lang_for_defs).await;
    let simple_translations = split_simple_translations(&translated);
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
