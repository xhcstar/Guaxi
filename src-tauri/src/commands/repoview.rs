use rusqlite::{params, Connection, Result};
use serde::Serialize;
use std::env;
use std::fs;

#[derive(Serialize)]
pub struct ImportedFile {
    id: i32,
    original_filesize: i32,
    title: String,
    create_time: String,
    original_filepath: String,
    original_filename: String,
    original_filetype: String,
    library: String,
    description: String,
    tags: Vec<String>,
    html_filename: String,
    html_filepath: String,
    html_change_time: String,
    markdown_filename: String,
    markdown_filepath: String,
    markdown_change_time: String,
    pdf_filename: String,
    pdf_filepath: String,
    pdf_change_time: String,
    word_filename: String,
    word_filepath: String,
    word_change_time: String,
    epub_filename: String,
    epub_filepath: String,
    epub_change_time: String,
    txt_filename: String,
    txt_filepath: String,
    txt_change_time: String,
    container_filename: String,
    container_filepath: String,
    container_change_time: String,
}

#[derive(Serialize)]
pub struct Note {
    id: i32,
    title: String,
    description: String,
    note_type: String,
    create_time: String,
    update_time: String,
    library: String,
    filename: String,
    from_filename: String,
    parent_note: Option<i32>, // 父笔记 ID
    tags: Vec<String>,        // 笔记的标签列表
    child_notes: Vec<i32>,    // 子笔记的 ID 列表
    link_to_notes: Vec<i32>,
    link_to_files: Vec<i32>,
}

pub fn init_db() -> Result<Connection> {
    let exe_path = env::current_exe().expect("无法获取当前可执行文件路径");
    let exe_dir = exe_path
        .parent()
        .expect("无法获取可执行文件所在目录")
        .to_path_buf();

    let repo_dir = exe_dir.join("repos");
    if !repo_dir.exists() {
        fs::create_dir_all(&repo_dir).expect("无法创建 repos 目录");
    }

    // 创建 SQLite 数据库
    let db_path = repo_dir.join("repos.db");
    let conn = Connection::open(db_path)?;

    // 创建 imported_files 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS imported_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            original_filesize INTEGER,
            title TEXT NOT NULL,
            create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            original_filepath TEXT NOT NULL,
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
        )",
        [],
    )?;

    // 创建 notes 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            description TEXT,
            note_type TEXT,
            create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            library TEXT,
            filename TEXT,
            from_filename TEXT,
            parent_note INTEGER,
            FOREIGN KEY(parent_note) REFERENCES notes(id)
        )",
        [],
    )?;

    // 创建 tags 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        )",
        [],
    )?;

    // 创建 note_tags 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS note_tags (
            note_id INTEGER,
            tag_id INTEGER,
            PRIMARY KEY (note_id, tag_id),
            FOREIGN KEY(note_id) REFERENCES notes(id),
            FOREIGN KEY(tag_id) REFERENCES tags(id)
        )",
        [],
    )?;

    Ok(conn)
}

#[tauri::command]
pub fn add_imported_file(
    original_filesize: i32,
    title: String,
    original_filepath: String,
    original_filename: String,
    original_filetype: String,
    library: String,
    description: String,
    tags: Vec<String>,
    html_filename: String,
    html_filepath: String,
    html_change_time: String,
    markdown_filename: String,
    markdown_filepath: String,
    markdown_change_time: String,
    pdf_filename: String,
    pdf_filepath: String,
    pdf_change_time: String,
    word_filename: String,
    word_filepath: String,
    word_change_time: String,
    epub_filename: String,
    epub_filepath: String,
    epub_change_time: String,
    txt_filename: String,
    txt_filepath: String,
    txt_change_time: String,
    container_filename: String,
    container_filepath: String,
    container_change_time: String,
) -> Result<i32, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 插入导入文件
    conn.execute(
        "INSERT INTO imported_files (
            original_filesize, title, original_filepath, original_filename, original_filetype,
            library, description, tags, html_filename, html_filepath, html_change_time,
            markdown_filename, markdown_filepath, markdown_change_time, pdf_filename, pdf_filepath,
            pdf_change_time, word_filename, word_filepath, word_change_time, epub_filename,
            epub_filepath, epub_change_time, txt_filename, txt_filepath, txt_change_time,
            container_filename, container_filepath, container_change_time
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
        params![
            original_filesize, title, original_filepath, original_filename, original_filetype,
            library, description, tags.join(","), html_filename, html_filepath, html_change_time,
            markdown_filename, markdown_filepath, markdown_change_time, pdf_filename, pdf_filepath,
            pdf_change_time, word_filename, word_filepath, word_change_time, epub_filename,
            epub_filepath, epub_change_time, txt_filename, txt_filepath, txt_change_time,
            container_filename, container_filepath, container_change_time
        ],
    ).map_err(|e| e.to_string())?;

    // 获取插入的文件 ID
    let file_id = conn.last_insert_rowid() as i32;
    Ok(file_id)
}

#[tauri::command]
pub fn get_imported_files() -> Result<Vec<ImportedFile>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, original_filesize, title, create_time, original_filepath, original_filename,
         original_filetype, library, description, tags, html_filename, html_filepath, html_change_time,
         markdown_filename, markdown_filepath, markdown_change_time, pdf_filename, pdf_filepath,
         pdf_change_time, word_filename, word_filepath, word_change_time, epub_filename, epub_filepath,
         epub_change_time, txt_filename, txt_filepath, txt_change_time, container_filename,
         container_filepath, container_change_time FROM imported_files"
    ).map_err(|e| e.to_string())?;

    let imported_files = stmt
        .query_map([], |row| {
            Ok(ImportedFile {
                id: row.get(0)?,
                original_filesize: row.get(1)?,
                title: row.get(2)?,
                create_time: row.get(3)?,
                original_filepath: row.get(4)?,
                original_filename: row.get(5)?,
                original_filetype: row.get(6)?,
                library: row.get(7)?,
                description: row.get(8)?,
                tags: row
                    .get::<_, String>(9)?
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                html_filename: row.get(10)?,
                html_filepath: row.get(11)?,
                html_change_time: row.get(12)?,
                markdown_filename: row.get(13)?,
                markdown_filepath: row.get(14)?,
                markdown_change_time: row.get(15)?,
                pdf_filename: row.get(16)?,
                pdf_filepath: row.get(17)?,
                pdf_change_time: row.get(18)?,
                word_filename: row.get(19)?,
                word_filepath: row.get(20)?,
                word_change_time: row.get(21)?,
                epub_filename: row.get(22)?,
                epub_filepath: row.get(23)?,
                epub_change_time: row.get(24)?,
                txt_filename: row.get(25)?,
                txt_filepath: row.get(26)?,
                txt_change_time: row.get(27)?,
                container_filename: row.get(28)?,
                container_filepath: row.get(29)?,
                container_change_time: row.get(30)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for file in imported_files {
        result.push(file.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn delete_imported_file(id: i32) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM imported_files WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_imported_file(
    id: i32,
    original_filesize: i32,
    title: String,
    original_filepath: String,
    original_filename: String,
    original_filetype: String,
    library: String,
    description: String,
    tags: Vec<String>,
    html_filename: String,
    html_filepath: String,
    html_change_time: String,
    markdown_filename: String,
    markdown_filepath: String,
    markdown_change_time: String,
    pdf_filename: String,
    pdf_filepath: String,
    pdf_change_time: String,
    word_filename: String,
    word_filepath: String,
    word_change_time: String,
    epub_filename: String,
    epub_filepath: String,
    epub_change_time: String,
    txt_filename: String,
    txt_filepath: String,
    txt_change_time: String,
    container_filename: String,
    container_filepath: String,
    container_change_time: String,
) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE imported_files SET
         original_filesize = ?1, title = ?2, original_filepath = ?3, original_filename = ?4,
         original_filetype = ?5, library = ?6, description = ?7, tags = ?8, html_filename = ?9,
         html_filepath = ?10, html_change_time = ?11, markdown_filename = ?12, markdown_filepath = ?13,
         markdown_change_time = ?14, pdf_filename = ?15, pdf_filepath = ?16, pdf_change_time = ?17,
         word_filename = ?18, word_filepath = ?19, word_change_time = ?20, epub_filename = ?21,
         epub_filepath = ?22, epub_change_time = ?23, txt_filename = ?24, txt_filepath = ?25,
         txt_change_time = ?26, container_filename = ?27, container_filepath = ?28,
         container_change_time = ?29 WHERE id = ?30",
        params![
            original_filesize, title, original_filepath, original_filename, original_filetype,
            library, description, tags.join(","), html_filename, html_filepath, html_change_time,
            markdown_filename, markdown_filepath, markdown_change_time, pdf_filename, pdf_filepath,
            pdf_change_time, word_filename, word_filepath, word_change_time, epub_filename,
            epub_filepath, epub_change_time, txt_filename, txt_filepath, txt_change_time,
            container_filename, container_filepath, container_change_time, id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn add_note(
    title: String,
    description: String,
    note_type: String,
    library: String,
    filename: String,
    from_filename: String,
    parent_note: Option<i32>,
    tags: Vec<String>,
    link_to_notes: Vec<i32>,
    link_to_files: Vec<i32>,
) -> Result<i32, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 插入笔记
    conn.execute(
        "INSERT INTO notes (title, description, note_type, library, filename, from_filename, parent_note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![title, description, note_type, library, filename, from_filename, parent_note],
    ).map_err(|e| e.to_string())?;

    // 获取插入的笔记 ID
    let note_id = conn.last_insert_rowid() as i32;

    // 插入标签
    for tag in tags {
        // 插入标签（如果不存在）
        conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )
        .map_err(|e| e.to_string())?;

        // 获取标签 ID
        let tag_id: i32 = conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        // 关联笔记和标签
        conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![note_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // 插入笔记链接
    for linked_note_id in link_to_notes {
        conn.execute(
            "INSERT INTO note_links (note_id, linked_note_id) VALUES (?1, ?2)",
            params![note_id, linked_note_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // 插入文件链接
    for linked_file_id in link_to_files {
        conn.execute(
            "INSERT INTO file_links (note_id, linked_file_id) VALUES (?1, ?2)",
            params![note_id, linked_file_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(note_id)
}

#[tauri::command]
pub fn get_notes() -> Result<Vec<Note>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 查询所有笔记
    let mut stmt = conn.prepare(
        "SELECT id, title, description, note_type, create_time, update_time, library, filename, from_filename, parent_note FROM notes"
    ).map_err(|e| e.to_string())?;

    let notes = stmt
        .query_map([], |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                note_type: row.get(3)?,
                create_time: row.get(4)?,
                update_time: row.get(5)?,
                library: row.get(6)?,
                filename: row.get(7)?,
                from_filename: row.get(8)?,
                parent_note: row.get(9)?,
                tags: Vec::new(),          // 先初始化为空，后续填充标签
                child_notes: Vec::new(),   // 先初始化为空，后续填充子笔记
                link_to_notes: Vec::new(), // 先初始化为空，后续填充链接笔记
                link_to_files: Vec::new(), // 先初始化为空，后续填充链接文件
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for note in notes {
        let mut note = note.map_err(|e| e.to_string())?;

        // 查询笔记的标签
        let mut stmt = conn
            .prepare(
                "SELECT tags.name FROM tags
             JOIN note_tags ON tags.id = note_tags.tag_id
             WHERE note_tags.note_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        let tags = stmt
            .query_map(params![note.id], |row| Ok(row.get(0)?))
            .map_err(|e| e.to_string())?;

        note.tags = tags
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())?;

        // 查询笔记的子笔记
        let mut stmt = conn
            .prepare("SELECT id FROM notes WHERE parent_note = ?1")
            .map_err(|e| e.to_string())?;

        let child_notes = stmt
            .query_map(params![note.id], |row| Ok(row.get(0)?))
            .map_err(|e| e.to_string())?;

        note.child_notes = child_notes
            .collect::<Result<Vec<i32>, _>>()
            .map_err(|e| e.to_string())?;

        // 查询笔记的链接笔记
        let mut stmt = conn
            .prepare("SELECT linked_note_id FROM note_links WHERE note_id = ?1")
            .map_err(|e| e.to_string())?;

        let link_to_notes = stmt
            .query_map(params![note.id], |row| Ok(row.get(0)?))
            .map_err(|e| e.to_string())?;

        note.link_to_notes = link_to_notes
            .collect::<Result<Vec<i32>, _>>()
            .map_err(|e| e.to_string())?;

        // 查询笔记的链接文件
        let mut stmt = conn
            .prepare("SELECT linked_file_id FROM file_links WHERE note_id = ?1")
            .map_err(|e| e.to_string())?;

        let link_to_files = stmt
            .query_map(params![note.id], |row| Ok(row.get(0)?))
            .map_err(|e| e.to_string())?;

        note.link_to_files = link_to_files
            .collect::<Result<Vec<i32>, _>>()
            .map_err(|e| e.to_string())?;

        result.push(note);
    }

    Ok(result)
}

#[tauri::command]
pub fn delete_note(id: i32) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 删除笔记的标签关联
    conn.execute("DELETE FROM note_tags WHERE note_id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    // 删除笔记的链接笔记
    conn.execute(
        "DELETE FROM note_links WHERE note_id = ?1 OR linked_note_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    // 删除笔记的链接文件
    conn.execute("DELETE FROM file_links WHERE note_id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    // 删除笔记
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn update_note(
    id: i32,
    title: String,
    description: String,
    note_type: String,
    library: String,
    filename: String,
    from_filename: String,
    parent_note: Option<i32>,
    tags: Vec<String>,
) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 更新笔记
    conn.execute(
        "UPDATE notes SET title = ?1, description = ?2, note_type = ?3, library = ?4, filename = ?5, from_filename = ?6, parent_note = ?7
         WHERE id = ?8",
        params![title, description, note_type, library, filename, from_filename, parent_note, id],
    ).map_err(|e| e.to_string())?;

    // 删除旧的标签关联
    conn.execute("DELETE FROM note_tags WHERE note_id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    // 插入新的标签
    for tag in tags {
        // 插入标签（如果不存在）
        conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )
        .map_err(|e| e.to_string())?;

        // 获取标签 ID
        let tag_id: i32 = conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        // 关联笔记和标签
        conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}
