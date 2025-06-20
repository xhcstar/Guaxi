use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{PathBuf};
use chrono::Local;
use rusqlite::{params, Connection};
use std::env;

/// 知识库新建时的输入结构体
#[derive(Deserialize)]
pub struct KnowledgeBaseInput {
    pub knowledge_base_name: String,
    pub knowledge_base_tags: String,
    pub knowledge_base_description: String,
}

/// 知识库列表项结构体
#[derive(Serialize)]
pub struct KnowledgeBaseListItem {
    pub id: i32,
    pub knowledge_base_name: String,
    pub knowledge_base_tags: String,
    pub knowledge_base_description: String,
    pub location: String,
}

/// 目录树节点结构体
#[derive(Serialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileNode>>,
}

/// 创建数据库文件路径（如不存在则自动创建目录）
fn create_db_file() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path
        .parent()
        .ok_or("无法获取可执行文件所在目录".to_string())?
        .to_path_buf();

    let repo_dir = exe_dir.join("repos");
    if !repo_dir.exists() {
        fs::create_dir_all(&repo_dir).map_err(|e| e.to_string())?;
    }
    let db_path = repo_dir.join("knowledge_bases.db");
    Ok(db_path)
}

/// 初始化数据库连接（如不存在则自动创建数据库文件）
fn init_db() -> Result<Connection, rusqlite::Error> {
    let db_path = create_db_file().map_err(|e| rusqlite::Error::InvalidPath(e.into()))?;
    let conn = Connection::open(&db_path)?;
    Ok(conn)
}

/// 自动创建知识库表（如不存在则自动创建）
fn setup_db() -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS knowledge_bases (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            knowledge_base_name TEXT NOT NULL UNIQUE,
            knowledge_base_tags TEXT,
            knowledge_base_description TEXT,
            created_at TEXT,
            location TEXT,
            owner TEXT,
            shared INTEGER
        )",
        [],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// 新建知识库命令
#[tauri::command]
pub fn create_knowledge_base(input: KnowledgeBaseInput) -> Result<(), String> {
    // 确保数据库和表存在
    setup_db()?; 

    // 1. 获取软件当前目录
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法获取可执行文件所在目录")?
        .to_path_buf();

    // 2. 创建总目录 KnowledgeBases
    let root_dir = exe_dir.join("KnowledgeBases");
    if !root_dir.exists() {
        fs::create_dir(&root_dir).map_err(|e| e.to_string())?;
    }

    // 3. 创建隐藏的元数据目录 .meta
    let meta_dir = root_dir.join(".meta");
    if !meta_dir.exists() {
        fs::create_dir(&meta_dir).map_err(|e| e.to_string())?;
    }

    // 4. 创建知识库目录
    let kb_dir = root_dir.join(&input.knowledge_base_name);
    if !kb_dir.exists() {
        fs::create_dir(&kb_dir).map_err(|e| e.to_string())?;
    }

    // 5. 创建知识库自己的隐藏元数据目录
    let kb_meta_dir = kb_dir.join(".meta");
    if !kb_meta_dir.exists() {
        fs::create_dir(&kb_meta_dir).map_err(|e| e.to_string())?;
    }

    // 6. 写入元数据到json文件
    let now = Local::now().to_rfc3339();
    let meta = serde_json::json!({
        "knowledge_base_name": input.knowledge_base_name,
        "knowledge_base_tags": input.knowledge_base_tags,
        "knowledge_base_description": input.knowledge_base_description,
        "knowledge_base_created_at": now,
        "knowledge_base_location": kb_dir.to_string_lossy(),
        "knowledge_base_owner": "当前用户",
        "knowledge_base_shared": false
    });
    let meta_file = kb_meta_dir.join("meta.json");
    fs::write(meta_file, serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;

    // 7. 写入数据库
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO knowledge_bases (knowledge_base_name, knowledge_base_tags, knowledge_base_description, created_at, location, owner, shared) VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            input.knowledge_base_name,
            input.knowledge_base_tags,
            input.knowledge_base_description,
            now,
            kb_dir.to_string_lossy(),
            "当前用户",
            0
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// 获取知识库列表命令
#[tauri::command]
pub fn get_knowledge_base_list() -> Result<Vec<KnowledgeBaseListItem>, String> {
    // 确保数据库和表存在
    setup_db()?; 
    let db_path = create_db_file().map_err(|e| e.to_string())?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, knowledge_base_name, knowledge_base_tags, knowledge_base_description, location FROM knowledge_bases"
    ).map_err(|e| e.to_string())?;

    let kb_iter = stmt
        .query_map([], |row| {
            Ok(KnowledgeBaseListItem {
                id: row.get(0)?,
                knowledge_base_name: row.get(1)?,
                knowledge_base_tags: row.get(2)?,
                knowledge_base_description: row.get(3)?,
                location: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for kb in kb_iter {
        result.push(kb.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

/// 获取知识库目录树命令
#[tauri::command]
pub fn get_knowledge_base_tree(root_path: String) -> Result<FileNode, String> {
    /// 递归读取目录结构
    fn read_dir_recursive(path: &PathBuf) -> Result<FileNode, String> {
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "".to_string());
        let is_dir = path.is_dir();
        let children = if is_dir {
            let mut nodes = Vec::new();
            for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let child_path = entry.path();
                // 跳过隐藏目录
                if let Some(fname) = child_path.file_name() {
                    if fname.to_string_lossy().starts_with('.') {
                        continue;
                    }
                }
                nodes.push(read_dir_recursive(&child_path)?);
            }
            Some(nodes)
        } else {
            None
        };
        Ok(FileNode {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            children,
        })
    }
    let path = PathBuf::from(root_path);
    read_dir_recursive(&path)
}