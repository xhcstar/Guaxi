use super::repoview::init_db;
use super::settingview::get_libraries_root;
use chrono::{DateTime, FixedOffset, Local};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Window};

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};
use tracing::{debug, error};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[allow(non_snake_case)]
#[derive(Serialize)]
pub struct BackupResult {
    pub success: bool,
    pub backupPath: String,
}

#[allow(non_snake_case)]
#[derive(Serialize)]
pub struct ExportResult {
    pub success: bool,
    pub exportPath: String,
    pub exportedCount: usize,
}

#[allow(non_snake_case)]
#[derive(Serialize)]
pub struct ImportResult {
    pub success: bool,
    pub importedCount: usize,
}

#[derive(Serialize, Deserialize)]
struct ImportedFileExport {
    id: i32,
    uuid: Option<String>,
    import_file_name: String,
    description: String,
    library: String,
    tags: Vec<String>,
    file_type: String,
    import_file_create_time: String,
    import_file_size: u64,
    knowledge_repo: String,
    hash: Option<String>,
    revision_count: Option<i32>,
    deleted: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct NoteExport {
    id: i32,
    uuid: Option<String>,
    description: String,
    note_file_name: String,
    tags: Vec<String>,
    note_file_type: String,
    from_imported_file: String,
    from_imported_file_library: String,
    note_file_create_time: String,
    note_file_update_time: String,
    knowledge_repo: String,
    hash: Option<String>,
    revision_count: Option<i32>,
    imported_file_id: Option<i32>,
    imported_file_uuid: Option<String>,
    deleted: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct DeviceExport {
    device_uuid: String,
    name: Option<String>,
    is_active: i32,
}

#[derive(Serialize, Deserialize)]
struct DeletionEventExport {
    object_type: String,
    object_uuid: String,
    device_uuid: String,
    deleted_at: String,
}

#[derive(Serialize, Deserialize)]
struct BackupManifest {
    version: u32,
    knowledge_repo: String,
    exported_at: String,
    imported_files: Vec<ImportedFileExport>,
    notes: Vec<NoteExport>,
    #[serde(default)]
    devices: Vec<DeviceExport>,
    #[serde(default)]
    deletion_events: Vec<DeletionEventExport>,
}

#[allow(non_snake_case)]
#[derive(serde::Serialize, Clone)]
struct BackupProgress {
    current: u32,
    total: u32,
    message: String,
    databaseType: String,
}

#[derive(Serialize, Deserialize)]
struct ImportedFilesTag {
    id: i32,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct ImportedFilesTagRel {
    file_id: i32,
    tag_id: i32,
}

#[derive(Serialize, Deserialize)]
struct NotesTag {
    id: i32,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct NotesTagRel {
    note_id: i32,
    tag_id: i32,
}

#[derive(Serialize, Deserialize)]
struct NoteLink {
    id: i32,
    source_note_id: i32,
    target_note_id: i32,
    context: Option<String>,
}

#[allow(non_snake_case)]
#[derive(serde::Serialize, Clone)]
struct RecoveryProgress {
    current: u32,
    total: u32,
    message: String,
    databaseType: String,
}

fn exe_dir() -> Result<PathBuf, String> {
    let p = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(p.parent().ok_or("no parent")?.to_path_buf())
}

fn start_file(zip: &mut ZipWriter<std::fs::File>, name: &str, data: &[u8]) -> Result<(), String> {
    let options: FileOptions<'_, ()> = FileOptions::default();
    zip.start_file(name, options).map_err(|e| e.to_string())?;
    zip.write_all(data).map_err(|e| e.to_string())?;
    Ok(())
}

fn zip_dir(
    zip: &mut ZipWriter<std::fs::File>,
    base: &Path,
    rel: &Path,
    prefix: &Path,
) -> Result<(), String> {
    let root = base.join(rel);
    if !root.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let path_on_disk = entry.path();
        let rel_path_in_zip = path_on_disk.strip_prefix(base).map_err(|e| e.to_string())?;
        let zip_path = prefix.join(rel_path_in_zip);
        let zip_path_str = zip_path.to_string_lossy().replace('\\', "/");
        let options: FileOptions<'_, ()> = FileOptions::default();
        if path_on_disk.is_dir() {
            let dir_name = if zip_path_str.ends_with('/') {
                zip_path_str.clone()
            } else {
                format!("{}/", zip_path_str)
            };
            zip.add_directory(&dir_name, options)
                .map_err(|e| e.to_string())?;
        } else {
            zip.start_file(&zip_path_str, options)
                .map_err(|e| e.to_string())?;
            let bytes = fs::read(path_on_disk).map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn collect_repo_data(repo: &str) -> Result<BackupManifest, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let files_sql = if repo.eq_ignore_ascii_case("ALL") {
        "SELECT id, uuid, original_filename, description, library, original_filetype,
         create_time, original_filesize, knowledge_repo, hash, revision_count, deleted
         FROM imported_files"
    } else {
        "SELECT id, uuid, original_filename, description, library, original_filetype,
         create_time, original_filesize, knowledge_repo, hash, revision_count, deleted
         FROM imported_files WHERE knowledge_repo = ?"
    };
    let mut stmt = conn.prepare(files_sql).map_err(|e| e.to_string())?;
    let mut files: Vec<ImportedFileExport> = Vec::new();
    let mapper = |row: &rusqlite::Row| {
        Ok((
            row.get::<_, i32>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, u64>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, Option<i32>>(10)?,
            row.get::<_, Option<i32>>(11)?,
        ))
    };
    let rows = if repo.eq_ignore_ascii_case("ALL") {
        stmt.query_map([], mapper)
    } else {
        stmt.query_map(params![repo], mapper)
    }
    .map_err(|e| e.to_string())?;
    for r in rows {
        let (id, uuid, name, desc, lib, ty, ctime, size, kr, hash, rev, deleted) = r.map_err(|e| e.to_string())?;
        let mut ts = conn
            .prepare(
                "SELECT ift.name FROM imported_files_tags ift
             JOIN imported_files_tags_relations iftr ON ift.id = iftr.tag_id
             WHERE iftr.file_id = ?",
            )
            .map_err(|e| e.to_string())?;
        let tags = ts
            .query_map(params![id], |row| Ok(row.get::<_, String>(0)?))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?;
        files.push(ImportedFileExport {
            id,
            uuid,
            import_file_name: name,
            description: desc,
            library: lib,
            tags,
            file_type: ty,
            import_file_create_time: ctime,
            import_file_size: size,
            knowledge_repo: kr,
            hash,
            revision_count: rev,
            deleted,
        });
    }

    let mut notes: Vec<NoteExport> = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, uuid, description, note_file_name, note_file_type,
         from_imported_file, from_imported_file_library, note_file_create_time,
         note_file_update_time, knowledge_repo, hash, revision_count, imported_file_id, imported_file_uuid, deleted
         FROM notes WHERE imported_file_id = ?",
        )
        .map_err(|e| e.to_string())?;
    for f in files.iter() {
        let rows = stmt
            .query_map(params![f.id.to_string()], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<i32>>(11)?,
                    row.get::<_, Option<i32>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<i32>>(14)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for r in rows {
            let (id, uuid, desc, nfn, nft, fif, fifl, c, u, kr, hash, rev, fifid, imported_file_uuid, deleted) =
                r.map_err(|e| e.to_string())?;
            let mut ts = conn
                .prepare(
                    "SELECT nt.name FROM notes_tags nt
                 JOIN notes_tags_relations ntr ON nt.id = ntr.tag_id
                 WHERE ntr.note_id = ?",
                )
                .map_err(|e| e.to_string())?;
            let note_tags = ts
                .query_map(params![id], |row| Ok(row.get::<_, String>(0)?))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|e| e.to_string())?;
            notes.push(NoteExport {
                id,
                uuid,
                description: desc,
                note_file_name: nfn,
                tags: note_tags,
                note_file_type: nft,
                from_imported_file: fif,
                from_imported_file_library: fifl,
                note_file_create_time: c,
                note_file_update_time: u,
                knowledge_repo: kr,
                hash,
                revision_count: rev,
                imported_file_id: fifid,
                imported_file_uuid,
                deleted,
            });
        }
    }

    let (devices, deletion_events) = load_devices_and_deletion_events(&conn)?;

    Ok(BackupManifest {
        version: 1,
        knowledge_repo: repo.to_string(),
        exported_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        imported_files: files,
        notes,
        devices,
        deletion_events,
    })
}

fn load_devices_and_deletion_events(
    conn: &rusqlite::Connection,
) -> Result<(Vec<DeviceExport>, Vec<DeletionEventExport>), String> {
    let mut devices: Vec<DeviceExport> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT device_uuid, name, is_active FROM devices")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DeviceExport {
                device_uuid: row.get(0)?,
                name: row.get(1)?,
                is_active: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    for r in rows {
        devices.push(r.map_err(|e| e.to_string())?);
    }

    let mut deletion_events: Vec<DeletionEventExport> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT object_type, object_uuid, device_uuid, deleted_at FROM deletion_events")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DeletionEventExport {
                object_type: row.get(0)?,
                object_uuid: row.get(1)?,
                device_uuid: row.get(2)?,
                deleted_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    for r in rows {
        deletion_events.push(r.map_err(|e| e.to_string())?);
    }

    Ok((devices, deletion_events))
}

fn file_stem(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_string()
}

fn infer_library_from_note_path(p: &str) -> Option<String> {
    let path = Path::new(p);
    let mut it = path.components();
    while let Some(c) = it.next() {
        if c.as_os_str().to_string_lossy() == "Libraries" {
            return it.next().and_then(|x| x.as_os_str().to_str()).map(|s| s.to_string());
        }
    }
    None
}

fn rel_path_after_libraries(p: &str) -> Option<String> {
    let s = p.replace('\\', "/");
    if let Some(pos) = s.find("Libraries/") {
        return Some(s[pos + "Libraries/".len()..].to_string());
    }
    None
}

#[allow(non_snake_case)]
#[tauri::command]
pub async fn backup_database(
    databaseType: String,
    outputDir: Option<String>,
    window: Window,
) -> Result<BackupResult, String> {
    let win = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        win.emit(
            "backup_progress",
            BackupProgress {
                current: 0,
                total: 100,
                message: "开始备份...".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        let manifest = collect_repo_data(&databaseType)?;
        win.emit(
            "backup_progress",
            BackupProgress {
                current: 10,
                total: 100,
                message: "已收集元数据".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        let target_dir = if let Some(dir) = outputDir {
            PathBuf::from(dir)
        } else {
            let exe = exe_dir()?;
            let backups_dir = exe.join("backups");
            if !backups_dir.exists() {
                fs::create_dir_all(&backups_dir).map_err(|e| e.to_string())?;
            }
            backups_dir
        };
        let fname = format!(
            "repo-{}-{}.zip",
            databaseType,
            Local::now().format("%Y%m%d_%H%M%S")
        );
        let backup_zip_path = target_dir.join(&fname);
        let f = std::fs::File::create(&backup_zip_path).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(f);

        let json_bytes = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
        start_file(&mut zip, "data/manifest.json", &json_bytes)?;
        win.emit(
            "backup_progress",
            BackupProgress {
                current: 20,
                total: 100,
                message: "已写入 manifest.json".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        let notes_bytes = serde_json::to_vec(&manifest.notes).map_err(|e| e.to_string())?;
        start_file(&mut zip, "data/notes-meta.json", &notes_bytes)?;
        win.emit(
            "backup_progress",
            BackupProgress {
                current: 30,
                total: 100,
                message: "已写入 notes-meta.json".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        // 备份剩余的数据库表
        let conn = init_db().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare("SELECT id, name FROM imported_files_tags")
            .map_err(|e| e.to_string())?;
        let itags = stmt
            .query_map([], |row| {
                Ok(ImportedFilesTag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<ImportedFilesTag>, _>>()
            .map_err(|e| e.to_string())?;
        start_file(
            &mut zip,
            "data/imported_files_tags.json",
            &serde_json::to_vec(&itags).map_err(|e| e.to_string())?,
        )?;

        let mut stmt = conn
            .prepare("SELECT file_id, tag_id FROM imported_files_tags_relations")
            .map_err(|e| e.to_string())?;
        let itrels = stmt
            .query_map([], |row| {
                Ok(ImportedFilesTagRel {
                    file_id: row.get(0)?,
                    tag_id: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<ImportedFilesTagRel>, _>>()
            .map_err(|e| e.to_string())?;
        start_file(
            &mut zip,
            "data/imported_files_tags_relations.json",
            &serde_json::to_vec(&itrels).map_err(|e| e.to_string())?,
        )?;

        let mut stmt = conn
            .prepare("SELECT id, name FROM notes_tags")
            .map_err(|e| e.to_string())?;
        let ntags = stmt
            .query_map([], |row| {
                Ok(NotesTag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<NotesTag>, _>>()
            .map_err(|e| e.to_string())?;
        start_file(
            &mut zip,
            "data/notes_tags.json",
            &serde_json::to_vec(&ntags).map_err(|e| e.to_string())?,
        )?;

        let mut stmt = conn
            .prepare("SELECT note_id, tag_id FROM notes_tags_relations")
            .map_err(|e| e.to_string())?;
        let ntrels = stmt
            .query_map([], |row| {
                Ok(NotesTagRel {
                    note_id: row.get(0)?,
                    tag_id: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<NotesTagRel>, _>>()
            .map_err(|e| e.to_string())?;
        start_file(
            &mut zip,
            "data/notes_tags_relations.json",
            &serde_json::to_vec(&ntrels).map_err(|e| e.to_string())?,
        )?;

        let mut stmt = conn
            .prepare("SELECT id, source_note_id, target_note_id, context FROM notes_links")
            .map_err(|e| e.to_string())?;
        let nlinks = stmt
            .query_map([], |row| {
                Ok(NoteLink {
                    id: row.get(0)?,
                    source_note_id: row.get(1)?,
                    target_note_id: row.get(2)?,
                    context: row.get::<_, Option<String>>(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<NoteLink>, _>>()
            .map_err(|e| e.to_string())?;
        start_file(
            &mut zip,
            "data/notes_links.json",
            &serde_json::to_vec(&nlinks).map_err(|e| e.to_string())?,
        )?;

        win.emit(
            "backup_progress",
            BackupProgress {
                current: 60,
                total: 100,
                message: "已备份数据库表".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        let libraries_root = get_libraries_root();
        let base = PathBuf::from(&libraries_root).join("Libraries");
        let prefix = PathBuf::from("repo");
        let repo_name = databaseType.trim();
        win.emit(
            "backup_progress",
            BackupProgress {
                current: 70,
                total: 100,
                message: "开始打包库文件".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        if repo_name.is_empty() || repo_name.eq_ignore_ascii_case("ALL") {
            zip_dir(&mut zip, &base, &PathBuf::new(), &prefix)?;
        } else {
            let rel = PathBuf::from(repo_name);
            zip_dir(&mut zip, &base, &rel, &prefix)?;
        }

        win.emit(
            "backup_progress",
            BackupProgress {
                current: 95,
                total: 100,
                message: "库文件打包完成".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        zip.finish().map_err(|e| e.to_string())?;

        win.emit(
            "backup_progress",
            BackupProgress {
                current: 100,
                total: 100,
                message: "备份完成".to_string(),
                databaseType: databaseType.clone(),
            },
        )
        .map_err(|e| e.to_string())?;

        Ok(BackupResult {
            success: true,
            backupPath: backup_zip_path.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[allow(non_snake_case)]
#[tauri::command]
pub async fn export_items(
    databaseType: String,
    exportPath: String,
    itemIds: Vec<i32>,
) -> Result<ExportResult, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut files: Vec<ImportedFileExport> = Vec::new();
    for id in itemIds.iter() {
        let row = conn
            .query_row(
                "SELECT id, uuid, original_filename, description, library, original_filetype,
             create_time, original_filesize, knowledge_repo, hash, revision_count, deleted
             FROM imported_files WHERE id = ?",
                params![id],
                |row| {
                    Ok(ImportedFileExport {
                        id: row.get(0)?,
                        uuid: row.get(1)?,
                        import_file_name: row.get(2)?,
                        description: row.get(3)?,
                        library: row.get(4)?,
                        tags: Vec::new(),
                        file_type: row.get(5)?,
                        import_file_create_time: row.get(6)?,
                        import_file_size: row.get(7)?,
                        knowledge_repo: row.get(8)?,
                        hash: row.get(9)?,
                        revision_count: row.get(10)?,
                        deleted: row.get(11)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        let mut ts = conn
            .prepare(
                "SELECT ift.name FROM imported_files_tags ift
             JOIN imported_files_tags_relations iftr ON ift.id = iftr.tag_id
             WHERE iftr.file_id = ?",
            )
            .map_err(|e| e.to_string())?;
        let tags = ts
            .query_map(params![row.id], |r| Ok(r.get::<_, String>(0)?))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?;
        let mut row = row;
        row.tags = tags;
        files.push(row);
    }

    let (devices, deletion_events) = load_devices_and_deletion_events(&conn)?;
    let notes: Vec<NoteExport> = Vec::new();
    let manifest = BackupManifest {
        version: 1,
        knowledge_repo: databaseType.clone(),
        exported_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        imported_files: files,
        notes,
        devices,
        deletion_events,
    };

    let fname = format!(
        "export-{}-{}.zip",
        databaseType,
        Local::now().format("%Y%m%d_%H%M%S")
    );
    let target = PathBuf::from(exportPath).join(&fname);
    let f = std::fs::File::create(&target).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(f);
    let json_bytes = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    start_file(&mut zip, "data/manifest.json", &json_bytes)?;

    let libraries_root = get_libraries_root();
    let base = PathBuf::from(&libraries_root).join("Libraries");
    let prefix = PathBuf::from("repo");
    for it in manifest.imported_files.iter() {
        let stem = file_stem(&it.import_file_name);
        let rel = PathBuf::from(&it.library).join(&stem);
        zip_dir(&mut zip, &base, &rel, &prefix)?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(ExportResult {
        success: true,
        exportPath: target.to_string_lossy().to_string(),
        exportedCount: itemIds.len(),
    })
}

fn extract_zip_to_dir(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(f).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let out_path = target_dir.join(file.mangled_name());
        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out_path.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn upsert_imported_file(
    conn: &rusqlite::Connection,
    src: &ImportedFileExport,
) -> Result<(), String> {
    let src_uuid = src
        .uuid
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let src_rev = src.revision_count.unwrap_or(1);
    let src_deleted = src.deleted.unwrap_or(0);

    debug!(
        "upsert_imported_file uuid={} name={} repo={} rev={} deleted={}",
        src_uuid,
        src.import_file_name,
        src.knowledge_repo,
        src_rev,
        src_deleted
    );

    let existing: Option<(i32, Option<i32>, String, Option<String>, i32)> = conn
        .query_row(
            "SELECT id, revision_count, create_time, hash, deleted FROM imported_files WHERE uuid = ? ORDER BY revision_count DESC, id DESC LIMIT 1",
            params![src_uuid],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let target_id = match existing {
        Some((id, rev, old_ctime, old_hash, old_deleted)) => {
            let old_rev = rev.unwrap_or(1);
            let newer_by_rev = src_rev > old_rev;
            let newer_by_time = !newer_by_rev && src.import_file_create_time >= old_ctime;
            let hash_diff = match (&src.hash, &old_hash) {
                (Some(a), Some(b)) => a != b,
                _ => false,
            };
            let deleted_changed = src_deleted != old_deleted;
            if newer_by_rev || newer_by_time || hash_diff || deleted_changed {
                debug!(
                    "upsert_imported_file update uuid={} id={} src_rev={} old_rev={} deleted_old={} deleted_new={}",
                    src_uuid,
                    id,
                    src_rev,
                    old_rev,
                    old_deleted,
                    src_deleted
                );
                conn.execute(
                    "UPDATE imported_files SET uuid = ?, original_filename = ?, description = ?, library = ?, original_filetype = ?, 
                 create_time = ?, original_filesize = ?, knowledge_repo = COALESCE(NULLIF(?, ''), knowledge_repo), hash = ?, revision_count = ?, deleted = ? WHERE id = ?",
                    params![
                        src_uuid,
                        src.import_file_name,
                        src.description,
                        src.library,
                        src.file_type,
                        src.import_file_create_time,
                        src.import_file_size,
                        src.knowledge_repo,
                        src.hash,
                        src_rev,
                        src_deleted,
                        id
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
            Some(id)
        }
        None => {
            let natural_existing: Option<i32> = conn
                .query_row(
                    "SELECT id FROM imported_files WHERE original_filename = ? AND library = ? AND knowledge_repo = ? ORDER BY revision_count DESC, id DESC LIMIT 1",
                    params![src.import_file_name, src.library, src.knowledge_repo],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;

            if let Some(id) = natural_existing {
                debug!(
                    "upsert_imported_file bind uuid={} to existing id={} via natural key",
                    src_uuid,
                    id
                );
                conn.execute(
                    "UPDATE imported_files SET uuid = ?, original_filename = ?, description = ?, library = ?, original_filetype = ?, 
                 create_time = ?, original_filesize = ?, knowledge_repo = COALESCE(NULLIF(?, ''), knowledge_repo), hash = ?, revision_count = ?, deleted = ? WHERE id = ?",
                    params![
                        src_uuid,
                        src.import_file_name,
                        src.description,
                        src.library,
                        src.file_type,
                        src.import_file_create_time,
                        src.import_file_size,
                        src.knowledge_repo,
                        src.hash,
                        src_rev,
                        src_deleted,
                        id
                    ],
                )
                .map_err(|e| e.to_string())?;
                Some(id)
            } else {
                debug!(
                    "upsert_imported_file insert uuid={} name={} repo={}",
                    src_uuid,
                    src.import_file_name,
                    src.knowledge_repo
                );
                conn.execute(
                    "INSERT INTO imported_files (uuid, original_filename, description, library, original_filetype, 
                 create_time, original_filesize, knowledge_repo, hash, revision_count, deleted)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        src_uuid,
                        src.import_file_name,
                        src.description,
                        src.library,
                        src.file_type,
                        src.import_file_create_time,
                        src.import_file_size,
                        src.knowledge_repo,
                        src.hash,
                        src_rev,
                        src_deleted,
                    ],
                )
                .map_err(|e| e.to_string())?;
                Some(conn.last_insert_rowid() as i32)
            }
        }
    };

    if let Some(file_id) = target_id {
        conn.execute(
            "DELETE FROM imported_files_tags_relations WHERE file_id = ?",
            params![file_id],
        )
        .map_err(|e| e.to_string())?;
        for tag in src.tags.iter() {
            let tag_id = conn
                .query_row(
                    "SELECT id FROM imported_files_tags WHERE name = ?",
                    params![tag],
                    |row| row.get(0),
                )
                .or_else(|_| {
                    conn.execute("INSERT INTO imported_files_tags (name) VALUES (?)", params![tag])?;
                    Ok(conn.last_insert_rowid())
                })
                .map_err(|e: rusqlite::Error| e.to_string())?;
            conn.execute(
                "INSERT INTO imported_files_tags_relations (file_id, tag_id) VALUES (?, ?)",
                params![file_id, tag_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn resolve_imported_file_id(conn: &rusqlite::Connection, note: &NoteExport) -> Result<Option<i32>, String> {
    if let Some(ref file_uuid) = note.imported_file_uuid {
        let by_uuid: Option<i32> = conn
            .query_row(
                "SELECT id FROM imported_files WHERE uuid = ?",
                params![file_uuid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if by_uuid.is_some() {
            return Ok(by_uuid);
        }
    }

    let lib = if note.from_imported_file_library.is_empty() {
        infer_library_from_note_path(&note.note_file_name).unwrap_or_default()
    } else {
        note.from_imported_file_library.clone()
    };
    let found: Option<i32> = conn
        .query_row(
            "SELECT id FROM imported_files WHERE original_filename = ? AND library = ? AND knowledge_repo = ? ORDER BY revision_count DESC, id DESC LIMIT 1",
            params![note.from_imported_file, lib, note.knowledge_repo],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if found.is_some() {
        return Ok(found);
    }
    if lib.is_empty() {
        let any: Option<i32> = conn
            .query_row(
                "SELECT id FROM imported_files WHERE original_filename = ? AND knowledge_repo = ? ORDER BY revision_count DESC, id DESC LIMIT 1",
                params![note.from_imported_file, note.knowledge_repo],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        return Ok(any);
    }
    Ok(None)
}

fn upsert_note(conn: &rusqlite::Connection, src: &NoteExport) -> Result<(), String> {
    let mapped_imported_file_id = resolve_imported_file_id(conn, src)?;
    if mapped_imported_file_id.is_none() {
        return Err("missing imported_file_id".to_string());
    }
    let imported_file_id = mapped_imported_file_id;

    let src_uuid = src
        .uuid
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let src_rev = src.revision_count.unwrap_or(1);
    let src_deleted = src.deleted.unwrap_or(0);

    debug!(
        "upsert_note uuid={} file={} repo={} rev={} deleted={}",
        src_uuid,
        src.note_file_name,
        src.knowledge_repo,
        src_rev,
        src_deleted
    );

    let existing: Option<(i32, Option<i32>, Option<String>, i32)> = if let Some(ref u) = src.uuid {
        conn
            .query_row(
                "SELECT id, revision_count, note_file_update_time, deleted FROM notes WHERE uuid = ? ORDER BY revision_count DESC, id DESC LIMIT 1",
                params![u],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?
    } else {
        let suffix = rel_path_after_libraries(&src.note_file_name)
            .unwrap_or_else(|| src.note_file_name.clone());
        conn
            .query_row(
                "SELECT id, revision_count, note_file_update_time, deleted FROM notes WHERE knowledge_repo = ? AND note_file_name LIKE '%' || ? ORDER BY revision_count DESC, id DESC LIMIT 1",
                params![&src.knowledge_repo, &suffix],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?
    };

    if let Some((id, rev, old_update, old_deleted)) = existing {
        let old_rev = rev.unwrap_or(1);
        let newer_by_rev = src_rev > old_rev;
        let newer_by_time = if newer_by_rev {
            false
        } else {
            match old_update {
                Some(ref old) => src.note_file_update_time > *old,
                None => true,
            }
        };
        let deleted_changed = src_deleted != old_deleted;
        if newer_by_rev || newer_by_time || deleted_changed {
            debug!(
                "upsert_note update uuid={} id={} src_rev={} old_rev={} deleted_old={} deleted_new={}",
                src_uuid,
                id,
                src_rev,
                old_rev,
                old_deleted,
                src_deleted
            );
            conn.execute(
                "UPDATE notes SET uuid = ?, description = ?, note_file_name = ?, note_file_type = ?, 
                 from_imported_file = ?, from_imported_file_library = COALESCE(NULLIF(?, ''), from_imported_file_library), note_file_create_time = ?, 
                 note_file_update_time = ?, knowledge_repo = COALESCE(NULLIF(?, ''), knowledge_repo), hash = ?, revision_count = ?, imported_file_id = ?, imported_file_uuid = ?, deleted = ? WHERE id = ?",
                params![
                    src_uuid,
                    src.description,
                    src.note_file_name,
                    src.note_file_type,
                    src.from_imported_file,
                    src.from_imported_file_library,
                    src.note_file_create_time,
                    src.note_file_update_time,
                    src.knowledge_repo,
                    src.hash,
                    src_rev,
                    imported_file_id,
                    src.imported_file_uuid,
                    src_deleted,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "DELETE FROM notes_tags_relations WHERE note_id = ?",
                params![id],
            )
            .map_err(|e| e.to_string())?;
            for tag in src.tags.iter() {
                let tag_id = conn
                    .query_row(
                        "SELECT id FROM notes_tags WHERE name = ?",
                        params![tag],
                        |row| row.get(0),
                    )
                    .or_else(|_| {
                        conn.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
                        Ok(conn.last_insert_rowid())
                    })
                    .map_err(|e: rusqlite::Error| e.to_string())?;
                conn.execute(
                    "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
                    params![id, tag_id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    } else {
        debug!(
            "upsert_note insert uuid={} file={} repo={}",
            src_uuid,
            src.note_file_name,
            src.knowledge_repo
        );
        conn.execute(
            "INSERT INTO notes (uuid, description, note_file_name, note_file_type, from_imported_file, 
             from_imported_file_library, note_file_create_time, note_file_update_time, 
             knowledge_repo, hash, revision_count, imported_file_id, imported_file_uuid, deleted)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                src_uuid,
                src.description,
                src.note_file_name,
                src.note_file_type,
                src.from_imported_file,
                src.from_imported_file_library,
                src.note_file_create_time,
                src.note_file_update_time,
                src.knowledge_repo,
                src.hash,
                src_rev,
                imported_file_id,
                src.imported_file_uuid,
                src_deleted,
            ],
        )
        .map_err(|e| e.to_string())?;
        let note_id = conn.last_insert_rowid() as i32;
        for tag in src.tags.iter() {
            let tag_id = conn
                .query_row(
                    "SELECT id FROM notes_tags WHERE name = ?",
                    params![tag],
                    |row| row.get(0),
                )
                .or_else(|_| {
                    conn.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
                    Ok(conn.last_insert_rowid())
                })
                .map_err(|e: rusqlite::Error| e.to_string())?;
            conn.execute(
                "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
                params![note_id, tag_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn upsert_device(conn: &rusqlite::Connection, src: &DeviceExport) -> Result<(), String> {
    let existing: Option<(i32, Option<String>, i32)> = conn
        .query_row(
            "SELECT id, name, is_active FROM devices WHERE device_uuid = ?",
            params![src.device_uuid],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some((id, _name, _active)) = existing {
        conn
            .execute(
                "UPDATE devices SET name = COALESCE(?, name), is_active = ? WHERE id = ?",
                params![src.name, src.is_active, id],
            )
            .map_err(|e| e.to_string())?;
    } else {
        conn
            .execute(
                "INSERT INTO devices (device_uuid, name, is_active) VALUES (?, ?, ?)",
                params![src.device_uuid, src.name, src.is_active],
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn upsert_deletion_event(
    conn: &rusqlite::Connection,
    src: &DeletionEventExport,
) -> Result<(), String> {
    let existing: Option<i32> = conn
        .query_row(
            "SELECT id FROM deletion_events WHERE object_type = ? AND object_uuid = ? AND device_uuid = ?",
            params![src.object_type, src.object_uuid, src.device_uuid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Ok(());
    }

    conn
        .execute(
            "INSERT INTO deletion_events (object_type, object_uuid, device_uuid, deleted_at) VALUES (?, ?, ?, ?)",
            params![src.object_type, src.object_uuid, src.device_uuid, src.deleted_at],
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn copy_library_tree(
    src_root: &Path,
    dst_root: &Path,
    lib: &str,
    name: &str,
) -> Result<(), String> {
    let stem = file_stem(name);
    let src = src_root.join(lib).join(&stem);
    if !src.exists() {
        return Ok(());
    }
    let dst = dst_root.join(lib).join(&stem);
    fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
    for entry in WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            let rel = p.strip_prefix(&src).map_err(|e| e.to_string())?;
            let target = dst.join(rel);
            let rel_str = rel.to_string_lossy();
            // Detect markdown note files for auto-merge
            let is_markdown_note = p
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, |ext| ext.eq_ignore_ascii_case("json"))
                && rel_str.to_lowercase().contains("notes");

            if target.exists() {
                let src_bytes = fs::read(p).map_err(|e| e.to_string())?;
                let dst_bytes = fs::read(&target).map_err(|e| e.to_string())?;
                let mut hs = Sha256::new();
                hs.update(&src_bytes);
                let sh = hs.finalize();
                let mut hd = Sha256::new();
                hd.update(&dst_bytes);
                let dh = hd.finalize();
                // Skip identical content
                if sh == dh {
                    continue;
                }
                if is_markdown_note {
                    let sm = fs::metadata(p).map_err(|e| e.to_string())?;
                    let dm = fs::metadata(&target).map_err(|e| e.to_string())?;
                    let src_newer = sm
                        .modified()
                        .ok()
                        .zip(dm.modified().ok())
                        .map(|(a, b)| a >= b)
                        .unwrap_or(true);
                    let src_str = String::from_utf8_lossy(&src_bytes);
                    let dst_str = String::from_utf8_lossy(&dst_bytes);
                    let merged = merge_note_json_content(&dst_str, &src_str);
                    let older_path = if src_newer { &target } else { p };
                    let conflict_base = dst.join("__conflicts");
                    let conflict_target = conflict_base.join(rel);
                    if let Some(parent) = conflict_target.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
                    let base = conflict_target.file_name().and_then(|n| n.to_str()).unwrap_or("conflict").to_string();
                    let conflict_name = format!("{}-{}", base, ts);
                    let conflict_path = conflict_target.with_file_name(conflict_name);
                    fs::copy(older_path, &conflict_path).map_err(|e| e.to_string())?;
                    if let Some(parent) = target.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                    fs::write(&target, merged.as_bytes()).map_err(|e| e.to_string())?;
                } else {
                    // Prefer newer file by modified time for non-markdown
                    let sm = fs::metadata(p).map_err(|e| e.to_string())?;
                    let dm = fs::metadata(&target).map_err(|e| e.to_string())?;
                    let overwrite = sm
                        .modified()
                        .ok()
                        .zip(dm.modified().ok())
                        .map(|(a, b)| a > b)
                        .unwrap_or(true);
                    if overwrite {
                        let conflict_base = dst.join("__conflicts");
                        let conflict_target = conflict_base.join(rel);
                        if let Some(parent) = conflict_target.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                        let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
                        let base = conflict_target.file_name().and_then(|n| n.to_str()).unwrap_or("conflict").to_string();
                        let conflict_name = format!("{}-{}", base, ts);
                        let conflict_path = conflict_target.with_file_name(conflict_name);
                        fs::copy(&target, &conflict_path).map_err(|e| e.to_string())?;
                        if let Some(parent) = target.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
                        fs::copy(p, &target).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::copy(p, &target).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn merge_note_json_content(a: &str, b: &str) -> String {
    fn parse_root(s: &str) -> Option<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(s).ok()
    }

    fn parse_ts(card: &serde_json::Value) -> Option<DateTime<FixedOffset>> {
        card.get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    }

    let va = parse_root(a);
    let vb = parse_root(b);

    let (ra, rb) = match (va, vb) {
        (Some(ra), Some(rb)) => (ra, rb),
        (Some(_), None) => return a.to_string(),
        (None, Some(_)) => return b.to_string(),
        (None, None) => return a.to_string(),
    };

    let mut cards_map: HashMap<String, (serde_json::Value, Option<DateTime<FixedOffset>>)> = HashMap::new();

    let mut push_cards = |root: &serde_json::Value| {
        if let Some(cards) = root.get("cards").and_then(|v| v.as_array()) {
            for card in cards {
                if let Some(id) = card.get("id").and_then(|v| v.as_str()) {
                    let ts = parse_ts(card);
                    match cards_map.get(id) {
                        None => {
                            cards_map.insert(id.to_string(), (card.clone(), ts));
                        }
                        Some((_, existing_ts)) => {
                            let replace = match (existing_ts, ts) {
                                (Some(e), Some(t)) => t >= *e,
                                (None, Some(_)) => true,
                                _ => false,
                            };
                            if replace {
                                cards_map.insert(id.to_string(), (card.clone(), ts));
                            }
                        }
                    }
                }
            }
        }
    };

    push_cards(&ra);
    push_cards(&rb);

    let mut merged_cards: Vec<serde_json::Value> = cards_map.into_values().map(|(v, _)| v).collect();

    merged_cards.sort_by(|a, b| {
        let tsa = parse_ts(a);
        let tsb = parse_ts(b);
        match (tsa, tsb) {
            (Some(ta), Some(tb)) => ta.cmp(&tb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    let mut obj = serde_json::Map::new();
    obj.insert("cards".to_string(), serde_json::Value::Array(merged_cards));
    let root = serde_json::Value::Object(obj);
    serde_json::to_string_pretty(&root).unwrap_or_else(|_| a.to_string())
}

pub fn gc_conflict_files() -> Result<(), String> {
    let libraries_root = get_libraries_root();
    let base = PathBuf::from(&libraries_root).join("Libraries");
    if !base.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |name| name == "__conflicts")
        {
            if let Err(e) = fs::remove_dir_all(path) {
                error!("Failed to remove conflict directory {:?}: {}", path, e);
            }
        }
    }
    Ok(())
}

#[allow(non_snake_case)]
#[tauri::command]
pub async fn import_items(_databaseType: String, importPath: String) -> Result<ImportResult, String> {
    let exe = exe_dir()?;
    let temp = exe
        .join("tmp")
        .join(format!("import-{}", Local::now().format("%Y%m%d_%H%M%S")));
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let src = PathBuf::from(&importPath);
    if src.is_file() && src.extension().map_or(false, |e| e == "zip") {
        extract_zip_to_dir(&src, &temp)?;
    } else {
        fs_extra::dir::copy(&src, &temp, &fs_extra::dir::CopyOptions::new())
            .map_err(|e| e.to_string())?;
    }

    let manifest_path = temp.join("data").join("manifest.json");
    if !manifest_path.exists() {
        return Err("missing manifest.json".into());
    }
    let data = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
    let mut manifest: BackupManifest = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    let notes_path1 = temp.join("data").join("notes.json");
    let notes_path2 = temp.join("data").join("notes-meta.json");
    let notes_path = if notes_path1.exists() {
        notes_path1
    } else {
        notes_path2
    };
    if notes_path.exists() {
        let notes_data = fs::read_to_string(&notes_path).map_err(|e| e.to_string())?;
        let notes: Vec<NoteExport> =
            serde_json::from_str(&notes_data).map_err(|e| e.to_string())?;
        manifest.notes = notes;
    }
    let src_repo_root = temp.join("repo");

    let conn = init_db().map_err(|e| e.to_string())?;
    for d in manifest.devices.iter() {
        upsert_device(&conn, d)?;
    }
    for e in manifest.deletion_events.iter() {
        upsert_deletion_event(&conn, e)?;
    }
    for f in manifest.imported_files.iter() {
        upsert_imported_file(&conn, f)?;
    }
    for n in manifest.notes.iter() {
        upsert_note(&conn, n)?;
    }

    let libraries_root = get_libraries_root();
    let dst_base = PathBuf::from(&libraries_root);
    for f in manifest.imported_files.iter() {
        copy_library_tree(&src_repo_root, &dst_base, &f.library, &f.import_file_name)?;
    }

    Ok(ImportResult {
        success: true,
        importedCount: manifest.imported_files.len(),
    })
}

#[allow(non_snake_case)]
#[tauri::command]
pub async fn recover_database(
    databaseType: String,
    backupPath: String,
    window: Window,
) -> Result<ImportResult, String> {
    let win = window.clone();
    tauri::async_runtime::spawn_blocking(move || {
        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 0, total: 100, message: "开始恢复...".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;

        let exe = exe_dir()?;
        let temp = exe.join("tmp").join(format!("recover-{}", Local::now().format("%Y%m%d_%H%M%S")));
        fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
        let src = PathBuf::from(&backupPath);
        extract_zip_to_dir(&src, &temp)?;
        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 15, total: 100, message: "已解压备份".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;

        let manifest_path = temp.join("data").join("manifest.json");
        if !manifest_path.exists() { return Err("missing manifest.json".into()); }
        let data = fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        let mut manifest: BackupManifest = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        let notes_path1 = temp.join("data").join("notes.json");
        let notes_path2 = temp.join("data").join("notes-meta.json");
        let notes_path = if notes_path1.exists() { notes_path1 } else { notes_path2 };
        if notes_path.exists() {
            let notes_data = fs::read_to_string(&notes_path).map_err(|e| e.to_string())?;
            let notes: Vec<NoteExport> = serde_json::from_str(&notes_data).map_err(|e| e.to_string())?;
            manifest.notes = notes;
        }
        let src_repo_root = temp.join("repo");
        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 35, total: 100, message: "已解析配置".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;

        let conn = init_db().map_err(|e| e.to_string())?;
        for d in manifest.devices.iter() { upsert_device(&conn, d)?; }
        for e in manifest.deletion_events.iter() { upsert_deletion_event(&conn, e)?; }
        for f in manifest.imported_files.iter() { upsert_imported_file(&conn, f)?; }
        for n in manifest.notes.iter() { upsert_note(&conn, n)?; }

        let links_path = temp.join("data").join("notes_links.json");
        if links_path.exists() {
            let links_data = fs::read_to_string(&links_path).map_err(|e| e.to_string())?;
            let links: Vec<NoteLink> = serde_json::from_str(&links_data).map_err(|e| e.to_string())?;
            for l in links.iter() {
                let src_note = manifest.notes.iter().find(|n| n.id == l.source_note_id);
                let tgt_note = manifest.notes.iter().find(|n| n.id == l.target_note_id);
                let sid: Option<i32> = if let Some(n) = src_note {
                    if let Some(ref h) = n.hash {
                        conn.query_row("SELECT id FROM notes WHERE hash = ?", params![h], |row| row.get(0)).optional().map_err(|e| e.to_string())?
                    } else {
                        conn.query_row("SELECT id FROM notes WHERE note_file_name = ? AND knowledge_repo = ?", params![n.note_file_name, n.knowledge_repo], |row| row.get(0)).optional().map_err(|e| e.to_string())?
                    }
                } else { None };
                let tid: Option<i32> = if let Some(n) = tgt_note {
                    if let Some(ref h) = n.hash {
                        conn.query_row("SELECT id FROM notes WHERE hash = ?", params![h], |row| row.get(0)).optional().map_err(|e| e.to_string())?
                    } else {
                        conn.query_row("SELECT id FROM notes WHERE note_file_name = ? AND knowledge_repo = ?", params![n.note_file_name, n.knowledge_repo], |row| row.get(0)).optional().map_err(|e| e.to_string())?
                    }
                } else { None };
                if let (Some(source_id), Some(target_id)) = (sid, tid) {
                    let exists: Option<i32> = conn
                        .query_row(
                            "SELECT id FROM notes_links WHERE source_note_id = ? AND target_note_id = ?",
                            params![source_id, target_id],
                            |row| row.get(0),
                        )
                        .optional()
                        .map_err(|e| e.to_string())?;
                    if exists.is_none() {
                        conn.execute(
                            "INSERT INTO notes_links (source_note_id, target_note_id, context) VALUES (?, ?, ?)",
                            params![source_id, target_id, l.context.clone()],
                        ).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 60, total: 100, message: "已写入数据库".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;

        let dst_base = PathBuf::from(get_libraries_root()).join("Libraries");
        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 80, total: 100, message: "开始恢复文件".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;
        for f in manifest.imported_files.iter() {
            copy_library_tree(&src_repo_root, &dst_base, &f.library, &f.import_file_name)?;
        }
        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 95, total: 100, message: "文件恢复完成".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;

        win.emit(
            "recovery_progress",
            RecoveryProgress { current: 100, total: 100, message: "恢复完成".to_string(), databaseType: databaseType.clone() },
        ).map_err(|e| e.to_string())?;

        Ok(ImportResult { success: true, importedCount: manifest.imported_files.len() })
    }).await.map_err(|e| e.to_string())?
}