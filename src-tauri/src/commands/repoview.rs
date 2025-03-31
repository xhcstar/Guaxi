use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};

pub mod repofileproc;
pub mod sql_constants;

#[derive(Serialize, Deserialize)]
pub struct NoteInput {
    description: String,
    note_file_name: String,
    tags: Vec<String>,
    note_file_type: String,
    from_imported_file: String,
    from_imported_file_library: String,
    note_file_create_time: String,
    note_file_update_time: String,
}
#[derive(Serialize, Deserialize)]
pub struct ImportedFile {
    id: i32,
    import_file_name: String,
    description: String,
    library: String,
    tags: Vec<String>,
    file_type: String,
    import_file_create_time: String,
    import_file_size: String,
    converted_markdown_filename: String,
}

#[derive(Serialize, Deserialize)]
pub struct ImportedFileInput {
    import_file_name: String,
    description: String,
    library: String,
    tags: Vec<String>,
    file_type: String,
    import_file_create_time: String,
    import_file_size: String,
    converted_markdown_filename: String,
}

#[derive(Serialize, Deserialize)]
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
        original_filesize TEXT,
        create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        original_filename TEXT NOT NULL,
        original_filetype TEXT NOT NULL,
        library TEXT,
        description TEXT,
        html_filename TEXT,
        html_filepath TEXT,
        html_change_time TIMESTAMP,
        markdown_filename TEXT,
        markdown_filepath TEXT,
        markdown_change_time TIMESTAMP,
        pdf_filename TEXT,
        pdf_filepath TEXT,
        pdf_change_time TIMESTAMP,
        word_filename TEXT,
        word_filepath TEXT,
        word_change_time TIMESTAMP,
        epub_filename TEXT,
        epub_filepath TEXT,
        epub_change_time TIMESTAMP,
        txt_filename TEXT,
        txt_filepath TEXT,
        txt_change_time TIMESTAMP,
        container_filename TEXT,
        container_filepath TEXT,
        container_change_time TIMESTAMP
    ",
    )
    .map_err(|e| e.to_string())?;

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
        description TEXT,
        note_file_name TEXT NOT NULL,
        note_file_type TEXT,
        from_imported_file TEXT,
        from_imported_file_library TEXT,
        note_file_create_time TEXT,
        note_file_update_time TEXT
    ",
    )
    .map_err(|e| e.to_string())?;

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

    Ok(())
}

// 获取导入文件列表
#[tauri::command]
pub fn get_imported_files() -> Result<Vec<ImportedFile>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询 imported_files 记录
    let mut stmt = conn
        .prepare(
            "SELECT id, original_filename, description, library, original_filetype, create_time, original_filesize, markdown_filename FROM imported_files",
        )
        .map_err(|e| e.to_string())?;
    let files = stmt
        .query_map([], |row| {
            Ok(ImportedFile {
                id: row.get(0)?,
                import_file_name: row.get(1)?,
                description: row.get(2)?,
                library: row.get(3)?,
                tags: Vec::new(), // 需要从关联表查询标签
                file_type: row.get(4)?,
                import_file_create_time: row.get(5)?,
                import_file_size: row.get(6)?,
                converted_markdown_filename: row.get(7)?,
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
        file.tags = tags
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?;

        result.push(file);
    }
    Ok(result)
}

#[tauri::command]
pub fn add_imported_file(file: ImportedFileInput) -> Result<i32, String> {
    let mut conn = init_db().map_err(|e: rusqlite::Error| e.to_string())?;
    let tx = conn
        .transaction()
        .map_err(|e: rusqlite::Error| e.to_string())?;

    // 插入 imported_files 记录
    tx.execute(
        "INSERT INTO imported_files (original_filename, description, library, original_filetype, create_time, original_filesize, markdown_filename) 
        VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            file.import_file_name,
            file.description,
            file.library,
            file.file_type,
            file.import_file_create_time,
            file.import_file_size,
            file.converted_markdown_filename,
        ],
    ).map_err(|e: rusqlite::Error| e.to_string())? as i32;
    // 获取新插入的 file_id
    let file_id = tx.last_insert_rowid() as i32;
    info!("Inserted imported file with ID: {}", file_id);

    // 插入标签并关联到 imported_files_tags_relations
    for tag in &file.tags {
        info!("Inserting tag: {}", tag);
        info!("file.tags: {:?}", &file.tags.clone());

        // 查询或插入标签
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
        info!("Inserted tag with ID: {}", tag_id);

        // 插入 imported_files_tags_relations 记录
        match tx.execute(
            "INSERT INTO imported_files_tags_relations (file_id, tag_id) VALUES (?, ?)",
            params![file_id, tag_id],
        ) {
            Ok(_) => info!(
                "Inserted imported_files_tags_relations with file_id: {} tag_id: {}",
                file_id, tag_id
            ),
            Err(e) => {
                error!("Failed to insert imported_files_tags_relations: {}", e);
                return Err(e.to_string());
            }
        }
    }

    tx.commit().map_err(|e: rusqlite::Error| e.to_string())?;
    Ok(file_id)
}

// 更新导入文件
#[tauri::command]
pub fn update_imported_file(id: i32, file: ImportedFileInput) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 更新 imported_files 记录
    tx.execute(
        "UPDATE imported_files SET original_filename = ?, description = ?, library = ?, original_filetype = ?, create_time = ?, original_filesize = ? WHERE id = ?",
        params![
            file.import_file_name,
            file.description,
            file.library,
            file.file_type,
            file.import_file_create_time,
            file.import_file_size,
            id
        ],
    ).map_err(|e| e.to_string())?;

    // 删除旧标签关联
    tx.execute(
        "DELETE FROM imported_files_tags_relations WHERE file_id = ?",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // 插入新标签并关联到 imported_files_tags_relations
    for tag in &file.tags {
        // 查询或插入标签
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

        // 插入 imported_files_tags_relations 记录
        tx.execute(
            "INSERT INTO imported_files_tags_relations (file_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_imported_file(id: i32) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 删除导入文件的标签关联
    tx.execute(
        "DELETE FROM imported_files_tags_relations WHERE file_id = ?",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // 删除导入文件记录
    tx.execute("DELETE FROM imported_files WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;

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
            "SELECT id, description, note_file_name, note_file_type, from_imported_file, from_imported_file_library, note_file_create_time, note_file_update_time FROM notes",
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

    // 插入 notes 记录
    tx.execute(
        "INSERT INTO notes (description, note_file_name, note_file_type, from_imported_file, from_imported_file_library, note_file_create_time, note_file_update_time) VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            note.description,
            note.note_file_name,
            note.note_file_type,
            note.from_imported_file,
            note.from_imported_file_library,
            note.note_file_create_time,
            note.note_file_update_time
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

    // 更新 notes 记录
    tx.execute(
        "UPDATE notes SET description = ?, note_file_name = ?, note_file_type = ?, from_imported_file = ?, from_imported_file_library = ?, note_file_create_time = ?, note_file_update_time = ? WHERE id = ?",
        params![note.description, note.note_file_name, note.note_file_type, note.from_imported_file, note.from_imported_file_library, note.note_file_create_time, note.note_file_update_time, id]
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

    // 删除笔记的标签关联
    tx.execute(
        "DELETE FROM notes_tags_relations WHERE note_id = ?",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // 删除笔记记录
    tx.execute("DELETE FROM notes WHERE id = ?", params![id])
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
        .prepare("SELECT original_filename FROM imported_files WHERE original_filename LIKE ?")
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
        .prepare("SELECT DISTINCT library FROM imported_files WHERE library LIKE ?")
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
        .prepare("SELECT DISTINCT library FROM imported_files WHERE library LIKE ?")
        .map_err(|e| e.to_string())?;
    let libraries = stmt
        .query_map(params![format!("%{}%", query)], |row| Ok(row.get(0)?))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(libraries)
}
