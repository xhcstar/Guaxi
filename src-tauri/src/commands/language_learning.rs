use crate::commands::translator_free;
use reqwest::Client;
use rusqlite::{params, Connection};
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
    pub part_of_speech: Option<String>,
    pub meanings: Vec<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub examples: Vec<String>,
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
    pub part_of_speech: String,
    pub meanings: Vec<String>,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub examples: Vec<String>,
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
    pub part_of_speech: String,
    pub meanings: Vec<String>,
    pub translation: String,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub examples: Vec<String>,
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
            part_of_speech TEXT NOT NULL DEFAULT '',
            meanings_json TEXT NOT NULL DEFAULT '[]',
            synonyms_json TEXT NOT NULL DEFAULT '[]',
            antonyms_json TEXT NOT NULL DEFAULT '[]',
            examples_json TEXT NOT NULL DEFAULT '[]',
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
    .map_err(|e| e.to_string())
}

fn parse_json_vec(text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(text).unwrap_or_default()
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

fn with_db<T>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let _ = app;
    let db_path = resolve_db_path()?;
    let conn = open_conn(&db_path)?;
    setup_schema(&conn)?;
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
        let part_of_speech = meta.part_of_speech.unwrap_or_default();
        let meanings_json =
            serde_json::to_string(&meta.meanings).map_err(|e| format!("meanings 序列化失败: {}", e))?;
        let synonyms_json =
            serde_json::to_string(&meta.synonyms).map_err(|e| format!("synonyms 序列化失败: {}", e))?;
        let antonyms_json =
            serde_json::to_string(&meta.antonyms).map_err(|e| format!("antonyms 序列化失败: {}", e))?;
        let examples_json =
            serde_json::to_string(&meta.examples).map_err(|e| format!("examples 序列化失败: {}", e))?;

        conn.execute(
            r#"
            INSERT INTO translation_records(
              id, source_text, translated_text, source_lang, target_lang, engine,
              record_type, is_word, word, phonetic, part_of_speech,
              meanings_json, synonyms_json, antonyms_json, examples_json,
              created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            "#,
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
                part_of_speech,
                meanings_json,
                synonyms_json,
                antonyms_json,
                examples_json,
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
            part_of_speech,
            meanings: meta.meanings,
            synonyms: meta.synonyms,
            antonyms: meta.antonyms,
            examples: meta.examples,
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
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<TranslationRecord>, String> {
    with_db(&app, |conn| {
        let q = keyword.unwrap_or_default().trim().to_string();
        let kind = kind.unwrap_or_else(|| "all".into()).trim().to_lowercase();
        let limit = limit.unwrap_or(200).clamp(1, 1000);
        let offset = offset.unwrap_or(0).max(0);

        let like = format!("%{}%", q);
        let mut stmt = conn
            .prepare(
                r#"
                SELECT
                  id, source_text, translated_text, source_lang, target_lang, engine,
                  record_type, is_word, word, phonetic, part_of_speech,
                  meanings_json, synonyms_json, antonyms_json, examples_json,
                  created_at, updated_at
                FROM translation_records
                WHERE (?1 = '' OR source_text LIKE ?2 OR translated_text LIKE ?2 OR word LIKE ?2)
                  AND (?3 = 'all' OR record_type = ?3)
                ORDER BY datetime(created_at) DESC
                LIMIT ?4 OFFSET ?5
                "#,
            )
            .map_err(|e| format!("准备查询失败: {}", e))?;

        let rows = stmt
            .query_map(params![q, like, kind, limit, offset], |r| {
                let meanings_json: String = r.get(11)?;
                let synonyms_json: String = r.get(12)?;
                let antonyms_json: String = r.get(13)?;
                let examples_json: String = r.get(14)?;
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
                    part_of_speech: r.get(10)?,
                    meanings: parse_json_vec(&meanings_json),
                    synonyms: parse_json_vec(&synonyms_json),
                    antonyms: parse_json_vec(&antonyms_json),
                    examples: parse_json_vec(&examples_json),
                    created_at: r.get(15)?,
                    updated_at: r.get(16)?,
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
) -> Result<Vec<AudioResource>, String> {
    with_db(&app, |conn| {
        let q = keyword.unwrap_or_default().trim().to_string();
        let like = format!("%{}%", q);
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, title, source_kind, source_uri, transcript, notes, duration_sec, created_at, updated_at
                FROM audio_resources
                WHERE (?1 = '' OR title LIKE ?2 OR transcript LIKE ?2 OR notes LIKE ?2)
                ORDER BY datetime(created_at) DESC
                LIMIT 1000
                "#,
            )
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
) -> Result<AudioResource, String> {
    with_db(&app, |conn| {
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

        conn.execute(
            r#"
            INSERT INTO audio_resources(id, title, source_kind, source_uri, transcript, notes, duration_sec, created_at, updated_at)
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
) -> Result<(), String> {
    with_db(&app, |conn| {
        conn.execute("DELETE FROM audio_resources WHERE id = ?1", params![id])
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

fn pick_phonetic(item: &serde_json::Value) -> String {
    if let Some(arr) = item.get("phonetics").and_then(|v| v.as_array()) {
        for p in arr {
            if let Some(t) = p.get("text").and_then(|v| v.as_str()) {
                let s = t.trim();
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
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

    let dict_json = fetch_json(&client, &dict_url).await;
    let syn_json = fetch_json(&client, &syn_url).await;
    let ant_json = fetch_json(&client, &ant_url).await;

    let mut phonetic = String::new();
    let mut part_of_speech = String::new();
    let mut meanings = Vec::new();
    let mut examples = Vec::new();
    let mut synonyms = Vec::new();
    let mut antonyms = Vec::new();

    if let Some(v) = dict_json {
        if let Some(arr) = v.as_array() {
            if let Some(first) = arr.first() {
                phonetic = pick_phonetic(first);
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
    let translated = translator_free::translate_text_free(vec![clean.clone()], Some(from), Some(to))
        .await
        .ok()
        .and_then(|res| res.translated_texts.first().cloned())
        .unwrap_or_default();

    let enrichment = WordEnrichment {
        word: clean,
        phonetic,
        part_of_speech,
        meanings: dedup_limit(meanings, 12),
        translation: translated,
        synonyms: dedup_limit(synonyms, 24),
        antonyms: dedup_limit(antonyms, 24),
        examples: dedup_limit(examples, 12),
    };
    Ok(enrichment)
}
