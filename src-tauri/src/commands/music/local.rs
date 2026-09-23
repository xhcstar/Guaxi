use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::MimeType;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{env, fs};
use tracing::{info, warn};
use walkdir::WalkDir;

const MUSIC_DIR_NAME: &str = "guaxi_music";
const MUSIC_DB_NAME: &str = "music.sqlite3";
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "ape", "alac", "aiff",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MusicLocalTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64, // 秒
    pub path: String,
    pub cover_path: String, // 空串表示无封面
    pub added_at: String,
}

fn track_id(path: &str) -> String {
    let mut h = Sha256::new();
    h.update(path.as_bytes());
    let digest = h.finalize();
    format!("local_{}", hex::encode(&digest[..8]))
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn resolve_dir() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("获取可执行文件路径失败: {}", e))?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "无法获取可执行文件目录".to_string())?
        .to_path_buf();
    let dir = exe_dir.join("repos").join(MUSIC_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建音乐数据目录失败: {}", e))?;
    }
    Ok(dir)
}

fn covers_dir() -> Result<PathBuf, String> {
    let dir = resolve_dir()?.join("covers");
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建封面目录失败: {}", e))?;
    }
    Ok(dir)
}

fn open_conn() -> Result<Connection, String> {
    let db_path = resolve_dir()?.join(MUSIC_DB_NAME);
    let conn = Connection::open(&db_path).map_err(|e| format!("打开音乐数据库失败: {}", e))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 失败: {}", e))?;
    setup_schema(&conn)?;
    Ok(conn)
}

fn setup_schema(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS music_local_tracks (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL DEFAULT '',
            artist TEXT NOT NULL DEFAULT '',
            album TEXT NOT NULL DEFAULT '',
            duration REAL NOT NULL DEFAULT 0,
            cover_path TEXT NOT NULL DEFAULT '',
            added_at TEXT NOT NULL DEFAULT ''
        )",
        [],
    )
    .map_err(|e| format!("建表失败: {}", e))?;
    Ok(())
}

// 读取标签、时长与内嵌封面，构造一条曲目记录（封面写到 covers/<id>.<ext>）。
fn build_track(path: &Path) -> Result<MusicLocalTrack, String> {
    let path_str = path.to_string_lossy().to_string();
    let id = track_id(&path_str);

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未知曲目")
        .to_string();

    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();
    let mut duration = 0.0_f64;
    let mut cover_path = String::new();

    match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged) => {
            duration = tagged.properties().duration().as_secs_f64();
            if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
                title = tag.title().map(|s| s.to_string()).unwrap_or_default();
                artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
                album = tag.album().map(|s| s.to_string()).unwrap_or_default();
                if let Some(pic) = tag.pictures().first() {
                    let ext = match pic.mime_type() {
                        Some(MimeType::Png) => "png",
                        _ => "jpg",
                    };
                    if let Ok(dir) = covers_dir() {
                        let out = dir.join(format!("{}.{}", id, ext));
                        if fs::write(&out, pic.data()).is_ok() {
                            cover_path = out.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }
        Err(e) => warn!("读取音频标签失败 {}: {}", path_str, e),
    }

    if title.trim().is_empty() {
        title = file_stem;
    }

    Ok(MusicLocalTrack {
        id,
        title,
        artist,
        album,
        duration,
        path: path_str,
        cover_path,
        added_at: chrono::Local::now().to_rfc3339(),
    })
}

fn upsert_track(conn: &Connection, t: &MusicLocalTrack) -> Result<(), String> {
    conn.execute(
        "INSERT INTO music_local_tracks (id, path, title, artist, album, duration, cover_path, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(path) DO UPDATE SET
            title=excluded.title, artist=excluded.artist, album=excluded.album,
            duration=excluded.duration, cover_path=excluded.cover_path",
        params![t.id, t.path, t.title, t.artist, t.album, t.duration, t.cover_path, t.added_at],
    )
    .map_err(|e| format!("写入曲目失败: {}", e))?;
    Ok(())
}

fn query_all(conn: &Connection) -> Result<Vec<MusicLocalTrack>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, artist, album, duration, path, cover_path, added_at
             FROM music_local_tracks ORDER BY added_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(MusicLocalTrack {
                id: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                duration: r.get(4)?,
                path: r.get(5)?,
                cover_path: r.get(6)?,
                added_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn music_import_files(paths: Vec<String>) -> Result<Vec<MusicLocalTrack>, String> {
    let conn = open_conn()?;
    let mut added = Vec::new();
    for p in paths {
        let path = Path::new(&p);
        if !path.is_file() || !is_audio(path) {
            continue;
        }
        match build_track(path) {
            Ok(t) => {
                upsert_track(&conn, &t)?;
                added.push(t);
            }
            Err(e) => warn!("导入失败 {}: {}", p, e),
        }
    }
    info!("music_import_files 导入 {} 首", added.len());
    Ok(added)
}

#[tauri::command]
pub fn music_import_dir(dir_path: String) -> Result<Vec<MusicLocalTrack>, String> {
    let conn = open_conn()?;
    let mut added = Vec::new();
    for entry in WalkDir::new(&dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && is_audio(path) {
            match build_track(path) {
                Ok(t) => {
                    upsert_track(&conn, &t)?;
                    added.push(t);
                }
                Err(e) => warn!("导入失败 {}: {}", path.display(), e),
            }
        }
    }
    info!("music_import_dir 从 {} 导入 {} 首", dir_path, added.len());
    Ok(added)
}

#[tauri::command]
pub fn music_local_list() -> Result<Vec<MusicLocalTrack>, String> {
    let conn = open_conn()?;
    query_all(&conn)
}

#[tauri::command]
pub fn music_local_remove(id: String) -> Result<(), String> {
    let conn = open_conn()?;
    conn.execute("DELETE FROM music_local_tracks WHERE id = ?1", params![id])
        .map_err(|e| format!("删除曲目失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn music_local_clear() -> Result<(), String> {
    let conn = open_conn()?;
    conn.execute("DELETE FROM music_local_tracks", [])
        .map_err(|e| format!("清空曲库失败: {}", e))?;
    Ok(())
}