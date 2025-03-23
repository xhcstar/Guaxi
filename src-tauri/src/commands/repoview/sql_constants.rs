// 创建表的 SQL 语句
pub const CREATE_TABLE_IMPORTED_FILES: &str = "
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            original_filesize TEXT,
            create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            original_filepath TEXT NOT NULL,
            original_filename TEXT NOT NULL,
            original_filetype TEXT NOT NULL,
            library TEXT,
            description TEXT,
            tags TEXT,
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
    ";

pub const CREATE_TABLE_NOTES: &str = "
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            description TEXT,
            note_type TEXT,
            create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            library TEXT,
            filename TEXT,
            from_filename TEXT,
            parent_note INTEGER,
            FOREIGN KEY(parent_note) REFERENCES notes(id)
    ";

pub const CREATE_TABLE_TAGS: &str = "
        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        )
    ";

pub const CREATE_TABLE_NOTE_TAGS: &str = "
        CREATE TABLE IF NOT EXISTS note_tags (
            note_id INTEGER,
            tag_id INTEGER,
            PRIMARY KEY (note_id, tag_id),
            FOREIGN KEY(note_id) REFERENCES notes(id),
            FOREIGN KEY(tag_id) REFERENCES tags(id)
        )
    ";

// 插入数据的 SQL 语句
pub const INSERT_IMPORTED_FILE_TEMPLATE: &str = "
        INSERT INTO {} (
            original_filesize, title, original_filepath, original_filename, original_filetype,
            library, description, html_filename, html_filepath, html_change_time,
            markdown_filename, markdown_filepath, markdown_change_time, pdf_filename, pdf_filepath,
            pdf_change_time, word_filename, word_filepath, word_change_time, epub_filename,
            epub_filepath, epub_change_time, txt_filename, txt_filepath, txt_change_time,
            container_filename, container_filepath, container_change_time
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)
        ";


// 查询数据的 SQL 语句
pub const SELECT_IMPORTED_FILES: &str = "
        SELECT id, original_filesize, title, create_time, original_filepath, original_filename,
        original_filetype, library, description, html_filename, html_filepath, html_change_time,
        markdown_filename, markdown_filepath, markdown_change_time, pdf_filename, pdf_filepath,
        pdf_change_time, word_filename, word_filepath, word_change_time, epub_filename, epub_filepath,
        epub_change_time, txt_filename, txt_filepath, txt_change_time, container_filename,
        container_filepath, container_change_time FROM {}
    ";

// 更新数据的 SQL 语句
pub const UPDATE_IMPORTED_FILE: &str = "
        UPDATE imported_files SET
        original_filesize = ?1, title = ?2, original_filepath = ?3, original_filename = ?4,
        original_filetype = ?5, library = ?6, description = ?7, html_filename = ?8,
        html_filepath = ?9, html_change_time = ?10, markdown_filename = ?11, markdown_filepath = ?12,
        markdown_change_time = ?13, pdf_filename = ?14, pdf_filepath = ?15, pdf_change_time = ?16,
        word_filename = ?17, word_filepath = ?18, word_change_time = ?19, epub_filename = ?20,
        epub_filepath = ?21, epub_change_time = ?22, txt_filename = ?23, txt_filepath = ?24,
        txt_change_time = ?25, container_filename = ?26, container_filepath = ?27,
        container_change_time = ?28 WHERE id = ?29
    ";

pub const UPDATE_TAG: &str = "
        UPDATE tags SET name = ?1 WHERE id = ?2
    ";

// 删除数据的 SQL 语句
pub const DELETE_IMPORTED_FILE: &str = "
        DELETE FROM imported_files WHERE id = ?1
    ";

pub const DELETE_NOTE_TAG: &str = "
        DELETE FROM note_tags WHERE note_id = ?1 AND tag_id = ?2
    ";

pub const INSERT_NOTE: &str = "
        INSERT INTO notes (description, note_type, library, filename, from_filename, parent_note)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
    ";

pub const SELECT_NOTES: &str = "
        SELECT id, description, note_type, create_time, update_time, library, filename, from_filename, parent_note
        FROM notes
    ";

pub const DELETE_NOTE: &str = "
        DELETE FROM notes WHERE id = ?1
    ";

pub const UPDATE_NOTE: &str = "
        UPDATE notes SET
        title = ?1, description = ?2, note_type = ?3, library = ?4, filename = ?5, from_filename = ?6, parent_note = ?7
        WHERE id = ?8
    ";

pub const INSERT_TAG: &str = "
        INSERT OR IGNORE INTO tags (name) VALUES (?1)
    ";

pub const SELECT_TAG_ID: &str = "
        SELECT id FROM tags WHERE name = ?1
    ";

pub const INSERT_NOTE_TAG: &str = "
        INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)
    ";

pub const INSERT_NOTE_LINK: &str = "
        INSERT INTO note_links (note_id, linked_note_id) VALUES (?1, ?2)
    ";

pub const INSERT_FILE_LINK: &str = "
        INSERT INTO file_links (note_id, linked_file_id) VALUES (?1, ?2)
    ";

pub const SELECT_TAGS_BY_NOTE: &str = "
        SELECT tags.name FROM tags
        JOIN note_tags ON tags.id = note_tags.tag_id
        WHERE note_tags.note_id = ?1
    ";

pub const SELECT_CHILD_NOTES: &str = "
        SELECT id FROM notes WHERE parent_note = ?1
    ";

pub const SELECT_LINKED_NOTES: &str = "
        SELECT linked_note_id FROM note_links WHERE note_id = ?1
    ";

pub const SELECT_LINKED_FILES: &str = "
        SELECT linked_file_id FROM file_links WHERE note_id = ?1
    ";

pub const DELETE_NOTE_TAGS: &str = "
        DELETE FROM note_tags WHERE note_id = ?1
    ";

pub const DELETE_NOTE_LINKS: &str = "
        DELETE FROM note_links WHERE note_id = ?1 OR linked_note_id = ?1
    ";

pub const DELETE_FILE_LINKS: &str = "
        DELETE FROM file_links WHERE note_id = ?1
    ";
