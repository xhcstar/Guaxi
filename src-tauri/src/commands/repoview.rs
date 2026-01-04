use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};
use sha2::{Digest, Sha256};
use hex;
use uuid::Uuid;
use chrono::Local;
use crate::commands::settingview::get_device_id;

pub mod repofileproc;
pub mod sql_constants;

#[derive(Serialize, Deserialize, Debug)]
pub struct Note {
    id: i32,
    description: String,
    note_file_name: String,
    tags: Vec<String>,
    note_file_type: String,
    from_imported_file: String,
    from_imported_file_library: String,
    note_file_create_time: String,
    note_file_update_time: String,
    knowledge_repo: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NoteInput {
    description: String,
    note_file_name: String,
    tags: Vec<String>,
    note_file_type: String,
    from_imported_file: String,
    from_imported_file_library: String,
    note_file_create_time: String,
    note_file_update_time: String,
    knowledge_repo: String,
    imported_file_id: i32,
}

impl NoteInput {
    pub fn new(
        description: String,
        note_file_name: String,
        tags: Vec<String>,
        note_file_type: String,
        from_imported_file: String,
        from_imported_file_library: String,
        note_file_create_time: String,
        note_file_update_time: String,
        knowledge_repo: String,
        imported_file_id: i32,
    ) -> Self {
        Self {
            description,
            note_file_name,
            tags,
            note_file_type,
            from_imported_file,
            from_imported_file_library,
            note_file_create_time,
            note_file_update_time,
            knowledge_repo,
            imported_file_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImportedFile {
    id: i32,
    import_file_name: String,
    description: String,
    library: String,
    tags: Vec<String>,
    file_type: String,
    import_file_create_time: String,
    import_file_size: u64,
    knowledge_repo: String,
    note_file: Option<Note>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImportedFileInput {
    import_file_name: String,
    description: String,
    library: String,
    tags: Vec<String>,
    file_type: String,
    import_file_create_time: String,
    import_file_size: u64,
    knowledge_repo: String,
    note_file_input: Option<NoteInput>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resource {
    id: i32,
    uuid: String,
    device_uuid: String,
    resource_type: String,
    file_name: String,
    relative_path: String,
    absolute_path: String,
    owner_file_path: String,
    created_at: String,
    extra: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ResourceInput {
    resource_type: String,
    file_name: String,
    relative_path: String,
    absolute_path: String,
    owner_file_path: String,
    created_at: String,
    extra: Option<String>,
}

impl ResourceInput {
    pub fn new(
        resource_type: String,
        file_name: String,
        relative_path: String,
        absolute_path: String,
        owner_file_path: String,
        created_at: String,
        extra: Option<String>,
    ) -> Self {
        Self {
            resource_type,
            file_name,
            relative_path,
            absolute_path,
            owner_file_path,
            created_at,
            extra,
        }
    }
}

// 创建数据库文件路径
pub fn create_db_file() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path
        .parent()
        .ok_or("无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    let repo_dir = exe_dir.join("repos");
    if !repo_dir.exists() {
        fs::create_dir_all(&repo_dir).map_err(|e| e.to_string())?;
    }
    // 数据库文件路径
    let db_path = repo_dir.join("repos.db");
    Ok(db_path)
}

// 初始化数据库连接
pub fn init_db() -> Result<Connection, rusqlite::Error> {
    let db_path = create_db_file().map_err(|e| rusqlite::Error::InvalidPath(e.into()))?;
    let conn = Connection::open(&db_path)?;
    Ok(conn)
}

// 动态创建表（如果表不存在）
pub fn create_table_if_not_exists(
    conn: &Connection,
    table_name: &str,
    schema: &str,
) -> Result<(), rusqlite::Error> {
    if !table_exists(conn, table_name)? {
        let query = format!("CREATE TABLE {} ({})", table_name, schema);
        conn.execute(&query, [])?;
    }
    Ok(())
}

// 判断表是否存在
pub fn table_exists(conn: &Connection, table_name: &str) -> Result<bool, rusqlite::Error> {
    let query = format!(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='{}'",
        table_name
    );
    let mut stmt = conn.prepare(&query)?;
    let exists = stmt.exists([])?;
    Ok(exists)
}

// 初始化数据库表结构
pub fn setup_db() -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 创建导入文件表（如果不存在）
    create_table_if_not_exists(
        &conn,
        "imported_files",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        uuid TEXT,
        original_filesize INTEGER,
        create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        original_filename TEXT NOT NULL,
        original_filetype TEXT NOT NULL,
        library TEXT,
        description TEXT,
        knowledge_repo TEXT NOT NULL,
        hash TEXT,
        revision_count INTEGER NOT NULL DEFAULT 1,
        deleted INTEGER NOT NULL DEFAULT 0
    ",
    )
    .map_err(|e| e.to_string())?;

    let _ = conn.execute("ALTER TABLE imported_files ADD COLUMN uuid TEXT", []);
    let _ = conn.execute("ALTER TABLE imported_files ADD COLUMN hash TEXT", []);
    let _ = conn.execute("ALTER TABLE imported_files ADD COLUMN revision_count INTEGER NOT NULL DEFAULT 1", []);
    let _ = conn.execute("ALTER TABLE imported_files ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE imported_files ADD COLUMN deleted_at TEXT", []);

    // 创建标签表（如果不存在）
    create_table_if_not_exists(
        &conn,
        "imported_files_tags",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE
    ",
    )
    .map_err(|e| e.to_string())?;

    // 创建导入文件与标签的关联表（如果不存在）
    create_table_if_not_exists(
        &conn,
        "imported_files_tags_relations",
        "
        file_id INTEGER NOT NULL,
        tag_id INTEGER NOT NULL,
        FOREIGN KEY(file_id) REFERENCES imported_files(id),
        FOREIGN KEY(tag_id) REFERENCES imported_files_tags(id),
        PRIMARY KEY(file_id, tag_id)
    ",
    )
    .map_err(|e| e.to_string())?;

    // 创建笔记表
    create_table_if_not_exists(
        &conn,
        "notes",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        uuid TEXT,
        description TEXT,
        note_file_name TEXT NOT NULL,
        note_file_type TEXT,
        from_imported_file TEXT,
        from_imported_file_library TEXT,
        note_file_create_time TEXT,
        note_file_update_time TEXT,
        knowledge_repo TEXT NOT NULL,
        imported_file_id INTEGER,
        imported_file_uuid TEXT,
        hash TEXT,
        revision_count INTEGER NOT NULL DEFAULT 1,
        deleted INTEGER NOT NULL DEFAULT 0
    ",
    )
    .map_err(|e| e.to_string())?;

    let _ = conn.execute("ALTER TABLE notes ADD COLUMN uuid TEXT", []);
    let _ = conn.execute("ALTER TABLE notes ADD COLUMN imported_file_id INTEGER", []);
    let _ = conn.execute("ALTER TABLE notes ADD COLUMN imported_file_uuid TEXT", []);
    let _ = conn.execute("ALTER TABLE notes ADD COLUMN revision_count INTEGER NOT NULL DEFAULT 1", []);
    let _ = conn.execute("ALTER TABLE notes ADD COLUMN hash TEXT", []);
    let _ = conn.execute("ALTER TABLE notes ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE notes ADD COLUMN deleted_at TEXT", []);

    // 创建标签表
    create_table_if_not_exists(
        &conn,
        "notes_tags",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE
    ",
    )
    .map_err(|e| e.to_string())?;

    // 创建笔记与标签的关联表
    create_table_if_not_exists(
        &conn,
        "notes_tags_relations",
        "
        note_id INTEGER NOT NULL,
        tag_id INTEGER NOT NULL,
        FOREIGN KEY(note_id) REFERENCES notes(id) ON DELETE CASCADE,
        FOREIGN KEY(tag_id) REFERENCES notes_tags(id) ON DELETE CASCADE,
        PRIMARY KEY(note_id, tag_id)
    ",
    )
    .map_err(|e| e.to_string())?;

    // 创建笔记链接表
    create_table_if_not_exists(
        &conn,
        "notes_links",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_note_id INTEGER NOT NULL,
        target_note_id INTEGER NOT NULL,
        context TEXT,
        FOREIGN KEY(source_note_id) REFERENCES notes(id),
        FOREIGN KEY(target_note_id) REFERENCES notes(id)
    ",
    )
    .map_err(|e| e.to_string())?;

    create_table_if_not_exists(
        &conn,
        "devices",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        device_uuid TEXT NOT NULL UNIQUE,
        name TEXT,
        is_active INTEGER NOT NULL DEFAULT 1
    ",
    )
    .map_err(|e| e.to_string())?;

    create_table_if_not_exists(
        &conn,
        "deletion_events",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        object_type TEXT NOT NULL,
        object_uuid TEXT NOT NULL,
        device_uuid TEXT NOT NULL,
        deleted_at TEXT NOT NULL
    ",
    )
    .map_err(|e| e.to_string())?;

    create_table_if_not_exists(
        &conn,
        "resources",
        "
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        uuid TEXT NOT NULL,
        device_uuid TEXT NOT NULL,
        resource_type TEXT NOT NULL,
        file_name TEXT NOT NULL,
        relative_path TEXT NOT NULL,
        absolute_path TEXT NOT NULL,
        owner_file_path TEXT NOT NULL,
        created_at TEXT NOT NULL,
        extra TEXT
    ",
    )
    .map_err(|e| e.to_string())?;

    let device_uuid = get_device_id();
    let device_name = env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    conn.execute(
        "INSERT OR IGNORE INTO devices (device_uuid, name, is_active) VALUES (?, ?, 1)",
        params![device_uuid, device_name],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE devices SET is_active = 1, name = ? WHERE device_uuid = ?",
        params![device_name, device_uuid],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// 获取导入文件列表
#[tauri::command]
pub fn get_imported_files(knowledge_repo: Option<String>) -> Result<Vec<ImportedFile>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询 imported_files 记录
    let mut stmt = conn
        .prepare(
            "SELECT id, original_filename, description, library, original_filetype, 
            create_time, original_filesize, knowledge_repo 
            FROM imported_files 
            WHERE knowledge_repo = ? AND deleted = 0",
        )
        .map_err(|e| e.to_string())?;
    let files = stmt
        .query_map(params![knowledge_repo], |row| {
            Ok(ImportedFile {
                id: row.get(0)?,
                import_file_name: row.get(1)?,
                description: row.get(2)?,
                library: row.get(3)?,
                tags: Vec::new(), // 需要从关联表查询标签
                file_type: row.get(4)?,
                import_file_create_time: row.get(5)?,
                import_file_size: row.get(6)?,
                knowledge_repo: row.get(7)?,
                note_file: None, // 初始化为None，后面再查询
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for file in files {
        let mut file = file.map_err(|e| e.to_string())?;

        // 查询标签
        let mut stmt = conn
            .prepare(
                "SELECT ift.name FROM imported_files_tags ift JOIN imported_files_tags_relations iftr ON ift.id = iftr.tag_id WHERE iftr.file_id = ?",
            )
            .map_err(|e| e.to_string())?;
        let tags = stmt
            .query_map(params![file.id], |row| Ok(row.get(0)?))
            .map_err(|e| e.to_string())?;
        file.tags = tags.collect::<Result<Vec<String>, _>>().map_err(|e| e.to_string())?;

        // 查询关联的笔记（通过from_imported_file字段关联）
        let mut stmt = conn
            .prepare(
                "SELECT id, description, note_file_name, note_file_type, from_imported_file, 
                from_imported_file_library, note_file_create_time, note_file_update_time, knowledge_repo 
                FROM notes WHERE from_imported_file = ?",
            )
            .map_err(|e| e.to_string())?;
        if let Ok(note) = stmt.query_row(params![file.import_file_name], |row| {
            Ok(Note {
                id: row.get(0)?,
                description: row.get(1)?,
                note_file_name: row.get(2)?,
                tags: Vec::new(), // 需要单独查询标签
                note_file_type: row.get(3)?,
                from_imported_file: row.get(4)?,
                from_imported_file_library: row.get(5)?,
                note_file_create_time: row.get(6)?,
                note_file_update_time: row.get(7)?,
                knowledge_repo: row.get(8)?,
            })
        }) {
            // 查询笔记的标签
            let mut stmt = conn
                .prepare(
                    "SELECT nt.name FROM notes_tags nt JOIN notes_tags_relations ntr ON nt.id = ntr.tag_id WHERE ntr.note_id = ?",
                )
                .map_err(|e| e.to_string())?;
            let tags = stmt
                .query_map(params![note.id], |row| Ok(row.get(0)?))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|e| e.to_string())?;
            
            let mut note_with_tags = note;
            note_with_tags.tags = tags;
            file.note_file = Some(note_with_tags);
        }

        result.push(file);
    }
    Ok(result)
}

#[tauri::command]
pub fn add_imported_file(file: ImportedFileInput) -> Result<i32, String> {
    let mut conn = init_db().map_err(|e: rusqlite::Error| e.to_string())?;
    let tx = conn.transaction().map_err(|e: rusqlite::Error| e.to_string())?;

    let uuid = Uuid::new_v4().to_string();

    let mut hasher = Sha256::new();
    hasher.update(file.import_file_name.as_bytes());
    hasher.update(file.description.as_bytes());
    hasher.update(file.library.as_bytes());
    hasher.update(file.file_type.as_bytes());
    hasher.update(file.import_file_size.to_le_bytes());
    let hash = hex::encode(hasher.finalize());

    tx.execute(
        "INSERT INTO imported_files (uuid, original_filename, description, library, original_filetype, 
        create_time, original_filesize, knowledge_repo, hash) 
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            uuid,
            file.import_file_name,
            file.description,
            file.library,
            file.file_type,
            file.import_file_create_time,
            file.import_file_size,
            file.knowledge_repo,
            hash,
        ],
    ).map_err(|e: rusqlite::Error| e.to_string())?;
    
    let file_id = tx.last_insert_rowid() as i32;

    // 处理标签
    for tag in &file.tags {
        let tag_id = tx
            .query_row(
                "SELECT id FROM imported_files_tags WHERE name = ?",
                params![tag],
                |row| row.get(0),
            )
            .or_else(|_| {
                tx.execute(
                    "INSERT INTO imported_files_tags (name) VALUES (?)",
                    params![tag],
                )?;
                Ok(tx.last_insert_rowid())
            })
            .map_err(|e: rusqlite::Error| e.to_string())?;

        tx.execute(
            "INSERT INTO imported_files_tags_relations (file_id, tag_id) VALUES (?, ?)",
            params![file_id, tag_id],
        ).map_err(|e| e.to_string())?;
    }

    // 处理关联笔记
    // if let Some(note_input) = file.note_file_input {
    //     // 插入 notes 记录
    //     tx.execute(
    //         "INSERT INTO notes (description, note_file_name, note_file_type, from_imported_file, 
    //         from_imported_file_library, note_file_create_time, note_file_update_time, 
    //         knowledge_repo) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    //         params![
    //             note_input.description,
    //             note_input.note_file_name,
    //             note_input.note_file_type,
    //             file.import_file_name, // 使用导入文件名作为来源
    //             file.library,
    //             note_input.note_file_create_time,
    //             note_input.note_file_update_time,
    //             note_input.knowledge_repo
    //         ],
    //     ).map_err(|e: rusqlite::Error| e.to_string())?;

    //     let note_id = tx.last_insert_rowid() as i32;

    //     // 处理笔记标签
    //     for tag in &note_input.tags {
    //         let tag_id = tx
    //             .query_row(
    //                 "SELECT id FROM notes_tags WHERE name = ?",
    //                 params![tag],
    //                 |row| row.get(0),
    //             )
    //             .or_else(|_| {
    //                 tx.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
    //                 Ok(tx.last_insert_rowid())
    //             })
    //             .map_err(|e: rusqlite::Error| e.to_string())?;

    //         tx.execute(
    //             "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
    //             params![note_id, tag_id],
    //         ).map_err(|e| e.to_string())?;
    //     }
    // }

    tx.commit().map_err(|e: rusqlite::Error| e.to_string())?;
    Ok(file_id)
}

#[tauri::command]
pub fn update_imported_file(id: i32, file: ImportedFileInput) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    info!("update_imported_file: {:?}", file);
    // 获取原始文件名，用于更新关联的笔记
    let original_filename: String = tx
        .query_row("SELECT original_filename FROM imported_files WHERE id = ?", params![id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut hasher = Sha256::new();
    hasher.update(file.import_file_name.as_bytes());
    hasher.update(file.description.as_bytes());
    hasher.update(file.library.as_bytes());
    hasher.update(file.file_type.as_bytes());
    hasher.update(file.import_file_size.to_le_bytes());
    let hash = hex::encode(hasher.finalize());

    tx.execute(
        "UPDATE imported_files SET original_filename = ?, description = ?, library = ?, original_filetype = ?, 
        create_time = ?, original_filesize = ?, knowledge_repo = ?, hash = ?, revision_count = revision_count + 1 WHERE id = ?",
        params![
            file.import_file_name,
            file.description,
            file.library,
            file.file_type,
            file.import_file_create_time,
            file.import_file_size,
            file.knowledge_repo,
            hash,
            id
        ],
    ).map_err(|e| e.to_string())?;

    // 更新标签
    tx.execute(
        "DELETE FROM imported_files_tags_relations WHERE file_id = ?",
        params![id],
    ).map_err(|e| e.to_string())?;

    for tag in &file.tags {
        let tag_id = tx
            .query_row(
                "SELECT id FROM imported_files_tags WHERE name = ?",
                params![tag],
                |row| row.get(0),
            )
            .or_else(|_| {
                tx.execute(
                    "INSERT INTO imported_files_tags (name) VALUES (?)",
                    params![tag],
                )?;
                Ok(tx.last_insert_rowid())
            })
            .map_err(|e: rusqlite::Error| e.to_string())?;

        tx.execute(
            "INSERT INTO imported_files_tags_relations (file_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        ).map_err(|e| e.to_string())?;
    }

    // 处理关联笔记
    if let Some(note_input) = file.note_file_input {
        // 检查是否已存在关联笔记
        let existing_note_id: Option<i32> = tx
            .query_row(
                "SELECT id FROM notes WHERE from_imported_file = ?",
                params![original_filename],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some(note_id) = existing_note_id {
            // 更新现有笔记
            let mut hasher = Sha256::new();
            hasher.update(note_input.description.as_bytes());
            hasher.update(note_input.note_file_name.as_bytes());
            hasher.update(note_input.note_file_type.as_bytes());
            hasher.update(file.import_file_name.as_bytes());
            hasher.update(file.library.as_bytes());
            hasher.update(note_input.note_file_create_time.as_bytes());
            hasher.update(note_input.note_file_update_time.as_bytes());
            hasher.update(note_input.knowledge_repo.as_bytes());
            for t in note_input.tags.iter() { hasher.update(t.as_bytes()); }
            let note_hash = hex::encode(hasher.finalize());

            tx.execute(
                "UPDATE notes SET description = ?, note_file_name = ?, note_file_type = ?, 
                from_imported_file = ?, from_imported_file_library = ?, note_file_create_time = ?, 
                note_file_update_time = ?, knowledge_repo = ?, hash = ?, revision_count = revision_count + 1 WHERE id = ?",
                params![
                    note_input.description,
                    note_input.note_file_name,
                    note_input.note_file_type,
                    file.import_file_name,
                    file.library,
                    note_input.note_file_create_time,
                    note_input.note_file_update_time,
                    note_input.knowledge_repo,
                    note_hash,
                    note_id
                ],
            ).map_err(|e| e.to_string())?;

            // 更新笔记标签
            tx.execute(
                "DELETE FROM notes_tags_relations WHERE note_id = ?",
                params![note_id],
            ).map_err(|e| e.to_string())?;

            for tag in &note_input.tags {
                let tag_id = tx
                    .query_row(
                        "SELECT id FROM notes_tags WHERE name = ?",
                        params![tag],
                        |row| row.get(0),
                    )
                    .or_else(|_| {
                        tx.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
                        Ok(tx.last_insert_rowid())
                    })
                    .map_err(|e: rusqlite::Error| e.to_string())?;

                tx.execute(
                    "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
                    params![note_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }
        } else {
            // 创建新笔记
            let mut hasher = Sha256::new();
            hasher.update(note_input.description.as_bytes());
            hasher.update(note_input.note_file_name.as_bytes());
            hasher.update(note_input.note_file_type.as_bytes());
            hasher.update(file.import_file_name.as_bytes());
            hasher.update(file.library.as_bytes());
            hasher.update(note_input.note_file_create_time.as_bytes());
            hasher.update(note_input.note_file_update_time.as_bytes());
            hasher.update(note_input.knowledge_repo.as_bytes());
            for t in note_input.tags.iter() { hasher.update(t.as_bytes()); }
            let note_hash = hex::encode(hasher.finalize());

            tx.execute(
                "INSERT INTO notes (description, note_file_name, note_file_type, from_imported_file, 
                from_imported_file_library, note_file_create_time, note_file_update_time, 
                knowledge_repo, hash) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    note_input.description,
                    note_input.note_file_name,
                    note_input.note_file_type,
                    file.import_file_name,
                    file.library,
                    note_input.note_file_create_time,
                    note_input.note_file_update_time,
                    note_input.knowledge_repo,
                    note_hash
                ],
            ).map_err(|e| e.to_string())?;

            let note_id = tx.last_insert_rowid() as i32;

            // 添加笔记标签
            for tag in &note_input.tags {
                let tag_id = tx
                    .query_row(
                        "SELECT id FROM notes_tags WHERE name = ?",
                        params![tag],
                        |row| row.get(0),
                    )
                    .or_else(|_| {
                        tx.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
                        Ok(tx.last_insert_rowid())
                    })
                    .map_err(|e: rusqlite::Error| e.to_string())?;

                tx.execute(
                    "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
                    params![note_id, tag_id],
                ).map_err(|e| e.to_string())?;
            }
        }
    } else {
        // 如果没有提供笔记输入，删除可能存在的关联笔记
        if let Ok(note_id) = tx.query_row(
            "SELECT id FROM notes WHERE from_imported_file = ?",
            params![original_filename],
            |row| row.get::<_, i32>(0),
        ) {
            // 删除笔记标签关联
            tx.execute(
                "DELETE FROM notes_tags_relations WHERE note_id = ?",
                params![note_id],
            ).map_err(|e| e.to_string())?;

            // 删除笔记
            tx.execute(
                "DELETE FROM notes WHERE id = ?",
                params![note_id],
            ).map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_imported_file(id: i32) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    info!("Soft deleting imported_file id={}", id);

    let device_uuid = get_device_id();
    let deleted_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let file_uuid: String = tx
        .query_row(
            "SELECT uuid FROM imported_files WHERE id = ?",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE imported_files SET deleted = 1, deleted_at = ?, revision_count = revision_count + 1 WHERE id = ?",
        params![deleted_at, id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE notes SET deleted = 1, deleted_at = ?, revision_count = revision_count + 1 WHERE imported_file_id = ?",
        params![deleted_at, id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT OR IGNORE INTO deletion_events (object_type, object_uuid, device_uuid, deleted_at) VALUES ('imported_file', ?, ?, ?)",
        params![file_uuid, device_uuid, deleted_at],
    )
    .map_err(|e| e.to_string())?;

    let note_uuids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT uuid FROM notes WHERE imported_file_id = ?")
            .map_err(|e| e.to_string())?;
        let note_uuids_iter = stmt
            .query_map(params![id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        note_uuids_iter
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?
    };

    for nu in note_uuids {
        tx.execute(
            "INSERT OR IGNORE INTO deletion_events (object_type, object_uuid, device_uuid, deleted_at) VALUES ('note', ?, ?, ?)",
            params![nu, device_uuid, deleted_at],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// 获取笔记列表
#[tauri::command]
pub fn get_notes() -> Result<Vec<Note>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询 notes 记录
    let mut stmt = conn
        .prepare(
            "SELECT id, description, note_file_name, note_file_type, from_imported_file, 
            from_imported_file_library, note_file_create_time, note_file_update_time, knowledge_repo 
            FROM notes WHERE deleted = 0",
        )
        .map_err(|e| e.to_string())?;
    let notes = stmt
        .query_map([], |row| {
            Ok(Note {
                id: row.get(0)?,
                description: row.get(1)?,
                note_file_name: row.get(2)?,
                tags: Vec::new(), // 需要从关联表查询标签
                note_file_type: row.get(3)?,
                from_imported_file: row.get(4)?,
                from_imported_file_library: row.get(5)?,
                note_file_create_time: row.get(6)?,
                note_file_update_time: row.get(7)?,
                knowledge_repo: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for note in notes {
        let mut note = note.map_err(|e| e.to_string())?;

        // 查询标签
        let mut stmt = conn
            .prepare(
                "SELECT nt.name FROM notes_tags nt JOIN notes_tags_relations ntr ON nt.id = ntr.tag_id WHERE ntr.note_id = ?",
            )
            .map_err(|e| e.to_string())?;
        let tags = stmt
            .query_map(params![note.id], |row| Ok(row.get(0)?))
            .map_err(|e| e.to_string())?;
        note.tags = tags
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?;

        result.push(note);
    }
    Ok(result)
}

#[tauri::command]
pub fn add_note(note: NoteInput) -> Result<i32, String> {
    let mut conn = init_db().map_err(|e: rusqlite::Error| e.to_string())?;
    let tx = conn
        .transaction()
        .map_err(|e: rusqlite::Error| e.to_string())?;

    let uuid = Uuid::new_v4().to_string();

    let mut hasher = Sha256::new();
    hasher.update(note.description.as_bytes());
    hasher.update(note.note_file_name.as_bytes());
    hasher.update(note.note_file_type.as_bytes());
    hasher.update(note.from_imported_file.as_bytes());
    hasher.update(note.from_imported_file_library.as_bytes());
    hasher.update(note.note_file_create_time.as_bytes());
    hasher.update(note.note_file_update_time.as_bytes());
    hasher.update(note.knowledge_repo.as_bytes());
    for t in note.tags.iter() { hasher.update(t.as_bytes()); }
    let note_hash = hex::encode(hasher.finalize());

    let imported_file_uuid: Option<String> = tx
        .query_row(
            "SELECT uuid FROM imported_files WHERE id = ?",
            params![note.imported_file_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    // 插入 notes 记录
    tx.execute(
        "INSERT INTO notes (uuid, description, note_file_name, note_file_type, from_imported_file, 
        from_imported_file_library, note_file_create_time, note_file_update_time, 
        knowledge_repo, imported_file_id, imported_file_uuid, hash) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            uuid,
            note.description,
            note.note_file_name,
            note.note_file_type,
            note.from_imported_file,
            note.from_imported_file_library,
            note.note_file_create_time,
            note.note_file_update_time,
            note.knowledge_repo,
            note.imported_file_id,
            imported_file_uuid,
            note_hash,
        ],
    ).map_err(|e: rusqlite::Error| e.to_string())?;

    // 获取新插入笔记的 note_id
    let note_id = tx.last_insert_rowid() as i32;
    info!("Inserted note with ID: {}", note_id);

    // 插入标签并关联到 notes_tags_relations
    for tag in &note.tags {
        info!("Inserting tag: {}", tag);
        info!("note.tags: {:?}", &note.tags.clone());

        // 查询或插入标签
        let tag_id = tx
            .query_row(
                "SELECT id FROM notes_tags WHERE name = ?",
                params![tag],
                |row| row.get(0),
            )
            .or_else(|_| {
                // 如果标签不存在，插入新标签
                tx.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
                Ok(tx.last_insert_rowid())
            })
            .map_err(|e: rusqlite::Error| e.to_string())?;
        info!("Inserted tag with ID: {}", tag_id);

        // 插入 notes_tags_relations 记录
        match tx.execute(
            "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
            params![note_id, tag_id],
        ) {
            Ok(_) => info!(
                "Inserted notes_tags_relations with note_id: {} tag_id: {}",
                note_id, tag_id
            ),
            Err(e) => {
                error!("Failed to insert notes_tags_relations: {}", e);
                return Err(e.to_string());
            }
        }
    }

    // 提交事务
    tx.commit().map_err(|e: rusqlite::Error| e.to_string())?;
    Ok(note_id)
}

// 更新笔记
#[tauri::command]
pub fn update_note(id: i32, note: NoteInput) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let imported_file_uuid: Option<String> = tx
        .query_row(
            "SELECT uuid FROM imported_files WHERE id = ?",
            params![note.imported_file_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let mut hasher = Sha256::new();
    hasher.update(note.description.as_bytes());
    hasher.update(note.note_file_name.as_bytes());
    hasher.update(note.note_file_type.as_bytes());
    hasher.update(note.from_imported_file.as_bytes());
    hasher.update(note.from_imported_file_library.as_bytes());
    hasher.update(note.note_file_create_time.as_bytes());
    hasher.update(note.note_file_update_time.as_bytes());
    hasher.update(note.knowledge_repo.as_bytes());
    for t in note.tags.iter() { hasher.update(t.as_bytes()); }
    let note_hash = hex::encode(hasher.finalize());

    // 更新 notes 记录
    tx.execute(
        "UPDATE notes SET description = ?, note_file_name = ?, note_file_type = ?, 
        from_imported_file = ?, from_imported_file_library = ?, note_file_create_time = ?, 
        note_file_update_time = ?, knowledge_repo = ?, imported_file_id = ?, imported_file_uuid = ?, hash = ?, revision_count = revision_count + 1 WHERE id = ?",
        params![
            note.description,
            note.note_file_name,
            note.note_file_type,
            note.from_imported_file,
            note.from_imported_file_library,
            note.note_file_create_time,
            note.note_file_update_time,
            note.knowledge_repo,
            note.imported_file_id,
            imported_file_uuid,
            note_hash,
            id
        ]
    ).map_err(|e| e.to_string())?;

    // 删除旧标签关联
    tx.execute(
        "DELETE FROM notes_tags_relations WHERE note_id = ?",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // 插入新标签并关联到 notes_tags_relations
    for tag in &note.tags {
        // 查询或插入标签
        let tag_id = tx
            .query_row(
                "SELECT id FROM notes_tags WHERE name = ?",
                params![tag],
                |row| row.get(0),
            )
            .or_else(|_| {
                tx.execute("INSERT INTO notes_tags (name) VALUES (?)", params![tag])?;
                Ok(tx.last_insert_rowid())
            })
            .map_err(|e: rusqlite::Error| e.to_string())?;

        // 插入 notes_tags_relations 记录
        tx.execute(
            "INSERT INTO notes_tags_relations (note_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// 删除笔记
#[tauri::command]
pub fn delete_note(id: i32) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    info!("Soft deleting note id={}", id);

    let device_uuid = get_device_id();
    let deleted_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let note_uuid: String = tx
        .query_row(
            "SELECT uuid FROM notes WHERE id = ?",
            params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE notes SET deleted = 1, deleted_at = ?, revision_count = revision_count + 1 WHERE id = ?",
        params![deleted_at, id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT OR IGNORE INTO deletion_events (object_type, object_uuid, device_uuid, deleted_at) VALUES ('note', ?, ?, ?)",
        params![note_uuid, device_uuid, deleted_at],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn search_notes_tags(query: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询 notes_tags 表
    let mut stmt = conn
        .prepare("SELECT name FROM notes_tags WHERE name LIKE ?")
        .map_err(|e| e.to_string())?;
    let tags = stmt
        .query_map(params![format!("%{}%", query)], |row| Ok(row.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(tags)
}

#[tauri::command]
pub fn search_imported_files_tags(query: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询 imported_files_tags 表
    let mut stmt = conn
        .prepare("SELECT name FROM imported_files_tags WHERE name LIKE ?")
        .map_err(|e| e.to_string())?;
    let tags = stmt
        .query_map(params![format!("%{}%", query)], |row| Ok(row.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(tags)
}

#[tauri::command]
pub fn search_imported_files(query: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT original_filename FROM imported_files WHERE original_filename LIKE ? AND deleted = 0")
        .map_err(|e| e.to_string())?;
    let files = stmt
        .query_map(params![format!("%{}%", query)], |row| Ok(row.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(files)
}

#[tauri::command]
pub fn search_libraries(query: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT DISTINCT library FROM imported_files WHERE library LIKE ? AND deleted = 0")
        .map_err(|e| e.to_string())?;
    let libraries = stmt
        .query_map(params![format!("%{}%", query)], |row| Ok(row.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(libraries)
}

#[tauri::command]
pub fn search_imported_file_libraries(query: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询 imported_files 表中的 library 字段
    let mut stmt = conn
        .prepare("SELECT DISTINCT library FROM imported_files WHERE library LIKE ? AND deleted = 0")
        .map_err(|e| e.to_string())?;
    let libraries = stmt
        .query_map(params![format!("%{}%", query)], |row| Ok(row.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(libraries)
}

#[tauri::command]
pub fn get_knowledge_repos() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT DISTINCT knowledge_repo FROM imported_files WHERE deleted = 0")
        .map_err(|e| e.to_string())?;
    
    let repos = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(repos)
}

#[tauri::command]
pub fn gc_deleted_items() -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;

    let devices: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT device_uuid FROM devices WHERE is_active = 1")
            .map_err(|e| e.to_string())?;
        let device_iter = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        device_iter
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?
    };
    if devices.is_empty() {
        return Ok(());
    }
    let active_count = devices.len() as i64;

    let uuids: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT uuid FROM imported_files WHERE deleted = 1 AND uuid IS NOT NULL AND uuid IN (
                    SELECT de.object_uuid FROM deletion_events de
                    JOIN devices d ON de.device_uuid = d.device_uuid
                    WHERE de.object_type = 'imported_file' AND d.is_active = 1
                    GROUP BY de.object_uuid
                    HAVING COUNT(DISTINCT de.device_uuid) >= ?
                )",
            )
            .map_err(|e| e.to_string())?;
        let uuid_iter = stmt
            .query_map(params![active_count], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        uuid_iter
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?
    };

    for u in uuids {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let (file_id, library, original_filename): (i32, String, String) = tx
            .query_row(
                "SELECT id, library, original_filename FROM imported_files WHERE uuid = ?",
                params![u],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| e.to_string())?;

        let libraries_root = crate::commands::settingview::get_libraries_root();
        let base = PathBuf::from(&libraries_root).join("Libraries");
        let stem = std::path::Path::new(&original_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !stem.is_empty() {
            let dir_path = base.join(&library).join(&stem);
            if dir_path.exists() {
                let _ = fs::remove_dir_all(&dir_path);
            }
        }

        tx.execute(
            "DELETE FROM notes_links WHERE source_note_id IN (SELECT id FROM notes WHERE imported_file_id = ?) OR target_note_id IN (SELECT id FROM notes WHERE imported_file_id = ?)",
            params![file_id, file_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM notes WHERE imported_file_id = ?",
            params![file_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM imported_files_tags_relations WHERE file_id = ?",
            params![file_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM imported_files WHERE id = ?",
            params![file_id],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn add_resource(resource: ResourceInput) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let uuid = Uuid::new_v4().to_string();
    let device_uuid = get_device_id();
    let created_at = if resource.created_at.is_empty() {
        Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        resource.created_at
    };

    conn
        .execute(
            "INSERT INTO resources (uuid, device_uuid, resource_type, file_name, relative_path, absolute_path, owner_file_path, created_at, extra)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid,
                device_uuid,
                resource.resource_type,
                resource.file_name,
                resource.relative_path,
                resource.absolute_path,
                resource.owner_file_path,
                created_at,
                resource.extra
            ],
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_resources_by_owner(owner_file_path: String) -> Result<Vec<Resource>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, uuid, device_uuid, resource_type, file_name, relative_path, absolute_path, owner_file_path, created_at, extra
             FROM resources
             WHERE owner_file_path = ?
             ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![owner_file_path], |row| {
            Ok(Resource {
                id: row.get(0)?,
                uuid: row.get(1)?,
                device_uuid: row.get(2)?,
                resource_type: row.get(3)?,
                file_name: row.get(4)?,
                relative_path: row.get(5)?,
                absolute_path: row.get(6)?,
                owner_file_path: row.get(7)?,
                created_at: row.get(8)?,
                extra: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut resources = Vec::new();
    for r in rows {
        resources.push(r.map_err(|e| e.to_string())?);
    }

    Ok(resources)
}