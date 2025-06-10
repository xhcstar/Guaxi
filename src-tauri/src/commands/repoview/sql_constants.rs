// // 创建表的 SQL 语句
// pub const CREATE_TABLE_IMPORTED_FILES: &str = "
//             id INTEGER PRIMARY KEY AUTOINCREMENT,
//             original_filesize TEXT,
//             create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//             original_filepath TEXT NOT NULL,
//             original_filename TEXT NOT NULL,
//             original_filetype TEXT NOT NULL,
//             library TEXT,
//             description TEXT,
//             tags TEXT,
//             html_filename TEXT,
//             html_filepath TEXT,
//             html_change_time TIMESTAMP,
//             markdown_filename TEXT,
//             markdown_filepath TEXT,
//             markdown_change_time TIMESTAMP,
//             pdf_filename TEXT,
//             pdf_filepath TEXT,
//             pdf_change_time TIMESTAMP,
//             word_filename TEXT,
//             word_filepath TEXT,
//             word_change_time TIMESTAMP,
//             epub_filename TEXT,
//             epub_filepath TEXT,
//             epub_change_time TIMESTAMP,
//             txt_filename TEXT,
//             txt_filepath TEXT,
//             txt_change_time TIMESTAMP,
//             container_filename TEXT,
//             container_filepath TEXT,
//             container_change_time TIMESTAMP
//     ";

// pub const CREATE_TABLE_NOTES: &str = "
//             id INTEGER PRIMARY KEY AUTOINCREMENT,
//             description TEXT,
//             note_type TEXT,
//             create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//             update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//             library TEXT,
//             filename TEXT,
//             from_filename TEXT,
//             parent_note INTEGER,
//             FOREIGN KEY(parent_note) REFERENCES notes(id)
//     ";

// 创建导入文件表
pub const CREATE_TABLE_IMPORTED_FILES: &str = "
-- 导入文件表
CREATE TABLE imported_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    original_filesize TEXT,
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
);
";

pub const CREATE_TABLE_IMPORTED_FILES_TAGS: &str = "
-- 导入文件标签表
CREATE TABLE imported_file_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);
";

pub const CREATE_TABLE_IMPORTED_FILES_TAG_RELATIONS: &str = "
-- 导入文件与标签的关联表
CREATE TABLE imported_file_tag_relations (
    file_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    FOREIGN KEY(file_id) REFERENCES imported_files(id),
    FOREIGN KEY(tag_id) REFERENCES imported_file_tags(id),
    PRIMARY KEY(file_id, tag_id)
);
";

// // 增加操作
// pub const INSERT_IMPORTED_FILE: &str = "
// INSERT INTO imported_files (
//     original_filesize, original_filepath, original_filename, original_filetype,
//     library, description, html_filename, html_filepath, html_change_time,
//     markdown_filename, markdown_filepath, markdown_change_time, pdf_filename,
//     pdf_filepath, pdf_change_time, word_filename, word_filepath, word_change_time,
//     epub_filename, epub_filepath, epub_change_time, txt_filename, txt_filepath,
//     txt_change_time, container_filename, container_filepath, container_change_time
// ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
// ";

// pub const INSERT_IMPORTED_FILE_TAG: &str = "
// INSERT INTO imported_file_tags (name) VALUES (?);
// ";

// pub const INSERT_IMPORTED_FILE_TAG_RELATION: &str = "
// INSERT INTO imported_file_tag_relations (file_id, tag_id) VALUES (?, ?);
// ";

// // 修改操作
// pub const UPDATE_IMPORTED_FILE_TAG: &str = "
// UPDATE imported_file_tags SET name = ? WHERE id = ?;
// ";

// // 删除操作
// pub const DELETE_IMPORTED_FILE: &str = "
// DELETE FROM imported_files WHERE id = ?;
// ";

// pub const DELETE_IMPORTED_FILE_TAG: &str = "
// DELETE FROM imported_file_tags WHERE id = ?;
// ";

// pub const DELETE_IMPORTED_FILE_TAG_RELATION: &str = "
// DELETE FROM imported_file_tag_relations WHERE file_id = ? AND tag_id = ?;
// ";

// // 查询操作
// pub const QUERY_ALL_IMPORTED_FILE_TAGS: &str = "
// SELECT id, name FROM imported_file_tags;
// ";

// pub const QUERY_IMPORTED_FILE_TAG_BY_ID: &str = "
// SELECT id, name FROM imported_file_tags WHERE id = ?;
// ";

// pub const QUERY_IMPORTED_FILE_TAGS: &str = "
// SELECT t.name
// FROM imported_file_tags t
// JOIN imported_file_tag_relations ift ON t.id = ift.tag_id
// WHERE ift.file_id = ?;
// ";

// // 创建笔记表
// pub const CREATE_TABLES_NOTES: &str = "
//         -- 笔记表
//         CREATE TABLE notes (
//             id INTEGER PRIMARY KEY AUTOINCREMENT,
//             description TEXT,
//             note_type TEXT,
//             create_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//             update_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
//             library TEXT,
//             filename TEXT,
//             from_filename TEXT
//         );
//         ";

// pub const CREATE_TABLES_NOTE_TAGS: &str = "
//         -- 笔记标签表
//         CREATE TABLE notes_tags (
//             id INTEGER PRIMARY KEY AUTOINCREMENT,
//             name TEXT NOT NULL UNIQUE
//         );
//         ";

// pub const CREATE_TABLES_NOTE_TAGS_RELATIONS: &str = "
//         -- 笔记与标签的关联表
//         CREATE TABLE notes_tags_relations (
//             note_id INTEGER NOT NULL,
//             tag_id INTEGER NOT NULL,
//             FOREIGN KEY(note_id) REFERENCES notes(id),
//             FOREIGN KEY(tag_id) REFERENCES note_tags(id),
//             PRIMARY KEY(note_id, tag_id)
//         );
//         ";
// pub const CREATE_TABLES_NOTE_LINKS: &str = "
//         -- 笔记链接表
//         CREATE TABLE notes_links (
//             id INTEGER PRIMARY KEY AUTOINCREMENT,
//             source_note_id INTEGER NOT NULL,
//             target_note_id INTEGER NOT NULL,
//             context TEXT,
//             FOREIGN KEY(source_note_id) REFERENCES notes(id),
//             FOREIGN KEY(target_note_id) REFERENCES notes(id)
//         );
//     ";

// // 增加操作
// pub const INSERT_NOTE: &str = "
//         INSERT INTO notes (
//             description, note_type, library, filename, from_filename
//         ) VALUES (?, ?, ?, ?, ?);
//     ";

// pub const INSERT_NOTE_TAG: &str = "
//         INSERT INTO note_tags (name) VALUES (?);
//     ";

// pub const INSERT_NOTE_TAG_RELATION: &str = "
//         INSERT INTO note_tag_relations (note_id, tag_id) VALUES (?, ?);
//     ";

// pub const INSERT_NOTE_LINK: &str = "
//         INSERT INTO note_links (source_note_id, target_note_id, context) VALUES (?, ?, ?);
//     ";

// // 修改操作
// pub const UPDATE_NOTE_TAG: &str = "
//         UPDATE note_tags SET name = ? WHERE id = ?;
//     ";

// // 删除操作
// pub const DELETE_NOTE: &str = "
//         DELETE FROM notes WHERE id = ?;
//     ";

// pub const DELETE_NOTE_TAG: &str = "
//         DELETE FROM note_tags WHERE id = ?;
//     ";

// pub const DELETE_NOTE_TAG_RELATION: &str = "
//         DELETE FROM note_tag_relations WHERE note_id = ? AND tag_id = ?;
//     ";

// pub const DELETE_NOTE_LINK: &str = "
//         DELETE FROM note_links WHERE id = ?;
//     ";

// // 查询操作
// pub const QUERY_ALL_NOTE_TAGS: &str = "
//         SELECT id, name FROM note_tags;
//     ";

// pub const QUERY_NOTE_TAG_BY_ID: &str = "
//         SELECT id, name FROM note_tags WHERE id = ?;
//     ";

// pub const QUERY_NOTE_TAGS: &str = "
//         SELECT t.name
//         FROM note_tags t
//         JOIN note_tag_relations nt ON t.id = nt.tag_id
//         WHERE nt.note_id = ?;
//     ";

// pub const QUERY_NOTE_LINKS: &str = "
//         SELECT n.description, nl.context
//         FROM note_links nl
//         JOIN notes n ON nl.target_note_id = n.id
//         WHERE nl.source_note_id = ?;
//     ";
