use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::Local;
use rusqlite::{params, Connection};
use std::env;

/// 知识库新建时的输入结构体
#[derive(Deserialize)]
pub struct KnowledgeBaseInput {
    pub knowledge_base_name: String,
    #[serde(default)]
    pub knowledge_base_type: Option<String>,
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

    let root = crate::commands::settingview::get_knowledge_bases_root();
    let root_dir = PathBuf::from(root);

    if !root_dir.exists() {
        fs::create_dir_all(&root_dir).map_err(|e| e.to_string())?;
    }

    let meta_dir = root_dir.join(".meta");
    if !meta_dir.exists() {
        fs::create_dir_all(&meta_dir).map_err(|e| e.to_string())?;
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
    let kb_type = input
        .knowledge_base_type
        .clone()
        .unwrap_or_else(|| "未分类".to_string());

    let meta = serde_json::json!({
        "knowledge_base_name": input.knowledge_base_name,
        "knowledge_base_type": kb_type,
        "knowledge_base_tags": input.knowledge_base_tags,
        "knowledge_base_description": input.knowledge_base_description,
        "knowledge_base_created_at": now,
        "knowledge_base_modified_at": now,
        "knowledge_base_location": kb_dir.to_string_lossy(),
        "knowledge_base_owner": "当前用户",
        "knowledge_base_shared": false
    });
    let meta_file = kb_meta_dir.join("meta.json");
    fs::write(meta_file, serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;

    // 额外：为“知识库管理器”生成根目录元数据（KnowledgeBases/.meta/xxx_meta/xxx_meta.json）
    crate::commands::kbmeta::init_kb_meta_for_new_kb(&input.knowledge_base_name, &kb_dir, &now)?;

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
    let path = PathBuf::from(&root_path);
    if !path.exists() {
        return Err(format!("路径不存在: {}", root_path));
    }
    if !path.is_dir() {
        return Err(format!("不是目录: {}", root_path));
    }
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


const MAX_MD_BYTES: u64 = 1024 * 1024;
const MAX_SECTIONS_PER_FILE: usize = 200;
const MAX_PARAGRAPHS_PER_FILE: usize = 300;

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

fn rel_path_from(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or(abs)
        .to_string_lossy()
        .replace('\\', "/")
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn excerpt_one_line(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut count: usize = 0;
    let mut last_space = false;

    for ch in s.chars() {
        if ch.is_whitespace() {
            if out.is_empty() || last_space {
                continue;
            }
            out.push(' ');
            count += 1;
            last_space = true;
        } else {
            out.push(ch);
            count += 1;
            last_space = false;
        }

        if count >= max_chars {
            break;
        }
    }

    out.trim().to_string()
}

fn sanitize_meta_stem(name: &str) -> String {
    let trimmed = name.trim().trim_matches(|c| c == ' ' || c == '.');
    if trimmed.is_empty() {
        return "_".to_string();
    }

    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        let ok = ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' ' | '@' | '+' | '=');
        if ok {
            out.push(ch);
        } else {
            out.push('_');
        }
    }

    let out2 = out.trim().trim_matches(|c| c == ' ' || c == '.').to_string();
    if out2.is_empty() { "_".to_string() } else { out2 }
}

fn normalize_related_ref(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }

    let t = t
        .strip_prefix("para_")
        .or_else(|| t.strip_prefix("section_"))
        .or_else(|| t.strip_prefix("node_"))
        .unwrap_or(t)
        .trim();

    if t.is_empty() { None } else { Some(t.to_string()) }
}

#[derive(Deserialize)]
struct FileStructureNodeLite {
    attrs: serde_json::Value,
    related_nodes: Vec<String>,
}

#[derive(Deserialize)]
struct FileMetaFileLite {
    structure_nodes: Vec<FileStructureNodeLite>,
}

fn load_kb_structure_relations(kb_name: &str) -> HashMap<String, Vec<String>> {
    let root = PathBuf::from(crate::commands::settingview::get_knowledge_bases_root());
    let meta_dir = root
        .join(".meta")
        .join(format!("{}_meta", sanitize_meta_stem(kb_name)));

    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    if !meta_dir.exists() {
        return out;
    }

    let kb_meta_file = format!("{}_meta.json", sanitize_meta_stem(kb_name));

    let Ok(entries) = fs::read_dir(&meta_dir) else {
        return out;
    };

    for ent in entries.flatten() {
        let p = ent.path();
        if !p.is_file() {
            continue;
        }
        let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if fname.is_empty() || fname == kb_meta_file {
            continue;
        }

        let Ok(s) = fs::read_to_string(&p) else {
            continue;
        };
        let Ok(m) = serde_json::from_str::<FileMetaFileLite>(&s) else {
            continue;
        };

        for n in m.structure_nodes {
            let kind = n
                .attrs
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if kind != "paragraph" && kind != "section" {
                continue;
            }

            let Some(from_sid) = n.attrs.get("stable_id").and_then(|v| v.as_str()) else {
                continue;
            };

            let tos: Vec<String> = n
                .related_nodes
                .iter()
                .filter_map(|x| normalize_related_ref(x))
                .collect();

            if tos.is_empty() {
                continue;
            }

            out.entry(from_sid.to_string())
                .or_default()
                .extend(tos);
        }
    }

    out
}

#[derive(Clone)]
struct MdSection {
    level: u8,
    title: String,
    start_line: u32,
    stable_id: String,
}

#[derive(Clone)]
struct MdParagraph {
    start_line: u32,
    end_line: u32,
    excerpt: String,
    stable_id: String,
    parent_section: Option<String>,
}

fn parse_markdown_structure(file_rel: &str, content: &str) -> (Vec<MdSection>, Vec<MdParagraph>) {
    let mut sections: Vec<MdSection> = Vec::new();
    let mut paragraphs: Vec<MdParagraph> = Vec::new();

    let mut in_fence = false;

    let mut para_lines: Vec<String> = Vec::new();
    let mut para_start_line: u32 = 0;
    let mut last_section: Option<String> = None;

    let mut flush_para = |end_line: u32,
                          para_lines: &mut Vec<String>,
                          para_start_line: &mut u32,
                          last_section: &Option<String>,
                          paragraphs: &mut Vec<MdParagraph>| {
        if para_lines.is_empty() {
            return;
        }

        let text = para_lines.join("\n").trim().to_string();
        para_lines.clear();
        if text.is_empty() {
            return;
        }

        let excerpt = excerpt_one_line(&text, 80);
        if excerpt.is_empty() {
            return;
        }

        let stable_id = sha256_hex(&format!("{}\nP\nL{}\n{}", file_rel, *para_start_line, text));
        paragraphs.push(MdParagraph {
            start_line: *para_start_line,
            end_line,
            excerpt,
            stable_id,
            parent_section: last_section.clone(),
        });

        *para_start_line = 0;
    };

    for (idx, line) in content.lines().enumerate() {
        let line_num = (idx + 1) as u32;
        let l = line.trim_end();
        let lt = l.trim_start();

        if lt.starts_with("```") {
            flush_para(
                line_num.saturating_sub(1),
                &mut para_lines,
                &mut para_start_line,
                &last_section,
                &mut paragraphs,
            );
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        let hashes = lt.chars().take_while(|c| *c == '#').count();
        if hashes >= 1 && hashes <= 6 {
            let rest = lt[hashes..].trim();
            if !rest.is_empty() {
                flush_para(
                    line_num.saturating_sub(1),
                    &mut para_lines,
                    &mut para_start_line,
                    &last_section,
                    &mut paragraphs,
                );

                let stable_id = sha256_hex(&format!(
                    "{}\nH{}\nL{}\n{}",
                    file_rel,
                    hashes,
                    line_num,
                    rest
                ));
                last_section = Some(stable_id.clone());
                sections.push(MdSection {
                    level: hashes as u8,
                    title: rest.to_string(),
                    start_line: line_num,
                    stable_id,
                });
                continue;
            }
        }

        if lt.trim().is_empty() {
            flush_para(
                line_num.saturating_sub(1),
                &mut para_lines,
                &mut para_start_line,
                &last_section,
                &mut paragraphs,
            );
            continue;
        }

        if para_lines.is_empty() {
            para_start_line = line_num;
        }
        para_lines.push(l.to_string());
    }

    flush_para(
        content.lines().count() as u32,
        &mut para_lines,
        &mut para_start_line,
        &last_section,
        &mut paragraphs,
    );

    (sections, paragraphs)
}

// 添加图形数据结构体
#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub path: Option<String>,
    pub size: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub strength: f64,
    pub r#type: Option<String>,
}

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// 获取知识库图形数据命令
#[tauri::command]
pub fn get_knowledge_graph_data(knowledge_base_id: i32) -> Result<GraphData, String> {
    let db_path = create_db_file().map_err(|e| e.to_string())?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    let kb_path: String = conn
        .query_row(
            "SELECT location FROM knowledge_bases WHERE id = ?",
            [knowledge_base_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let kb_path = PathBuf::from(kb_path);
    if !kb_path.exists() {
        return Err("知识库路径不存在".to_string());
    }

    let kb_id = format!("kb_{}", knowledge_base_id);
    let kb_name = kb_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let kb_abs = kb_path.to_string_lossy().to_string();

    let structure_relations = load_kb_structure_relations(&kb_name);
    let mut stable_to_node_id: HashMap<String, String> = HashMap::new();

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    nodes.push(GraphNode {
        id: kb_id.clone(),
        name: kb_name,
        r#type: "knowledge-base".to_string(),
        path: Some(kb_abs),
        size: Some(1.0),
        file_path: None,
        start_line: None,
        end_line: None,
        stable_id: None,
        depth: Some(0),
    });

    fn dir_id(rel: &str) -> String {
        format!("dir_{}", sha256_hex(rel))
    }

    fn file_id(rel: &str) -> String {
        format!("file_{}", sha256_hex(rel))
    }

    fn section_id(stable_id: &str) -> String {
        format!("section_{}", stable_id)
    }

    fn para_id(stable_id: &str) -> String {
        format!("para_{}", stable_id)
    }

    fn walk_dir(
        base: &Path,
        cur: &Path,
        parent_id: &str,
        parent_depth: u32,
        nodes: &mut Vec<GraphNode>,
        edges: &mut Vec<GraphEdge>,
        stable_to_node_id: &mut HashMap<String, String>,
    ) -> Result<(), String> {
        let mut entries: Vec<PathBuf> = fs::read_dir(cur)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|x| x.path()))
            .collect();

        entries.sort_by(|a, b| a.to_string_lossy().to_lowercase().cmp(&b.to_string_lossy().to_lowercase()));

        for path in entries {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.is_empty() || is_hidden_name(name) || name == ".meta" {
                continue;
            }

            let rel = rel_path_from(base, &path);
            let abs_str = path.to_string_lossy().to_string();

            if path.is_dir() {
                let id = dir_id(&rel);
                nodes.push(GraphNode {
                    id: id.clone(),
                    name: name.to_string(),
                    r#type: "directory".to_string(),
                    path: Some(abs_str),
                    size: Some(0.5),
                    file_path: None,
                    start_line: None,
                    end_line: None,
                    stable_id: None,
                    depth: Some(parent_depth + 1),
                });
                edges.push(GraphEdge {
                    from: parent_id.to_string(),
                    to: id.clone(),
                    strength: 0.7,
                    r#type: Some("contains".to_string()),
                });
                walk_dir(base, &path, &id, parent_depth + 1, nodes, edges, stable_to_node_id)?;
                continue;
            }

            let id = file_id(&rel);
            nodes.push(GraphNode {
                id: id.clone(),
                name: name.to_string(),
                r#type: "file".to_string(),
                path: Some(abs_str.clone()),
                size: Some(0.3),
                file_path: Some(abs_str.clone()),
                start_line: None,
                end_line: None,
                stable_id: None,
                depth: Some(parent_depth + 1),
            });
            edges.push(GraphEdge {
                from: parent_id.to_string(),
                to: id.clone(),
                strength: 0.5,
                r#type: Some("contains".to_string()),
            });

            if !is_markdown_path(&path) {
                continue;
            }

            let too_large = fs::metadata(&path)
                .ok()
                .map(|m| m.len() > MAX_MD_BYTES)
                .unwrap_or(false);
            if too_large {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let (sections, paragraphs) = parse_markdown_structure(&rel, &content);

            for sec in sections.iter().take(MAX_SECTIONS_PER_FILE) {
                let sec_node_id = section_id(&sec.stable_id);
                stable_to_node_id.insert(sec.stable_id.clone(), sec_node_id.clone());

                nodes.push(GraphNode {
                    id: sec_node_id.clone(),
                    name: sec.title.clone(),
                    r#type: "section".to_string(),
                    path: Some(abs_str.clone()),
                    size: Some(0.22),
                    file_path: Some(abs_str.clone()),
                    start_line: Some(sec.start_line),
                    end_line: None,
                    stable_id: Some(sec.stable_id.clone()),
                    depth: Some(parent_depth + 2),
                });
                edges.push(GraphEdge {
                    from: id.clone(),
                    to: sec_node_id,
                    strength: 0.45,
                    r#type: Some("contains".to_string()),
                });
            }

            let mut prev_para: Option<String> = None;
            for p in paragraphs.iter().take(MAX_PARAGRAPHS_PER_FILE) {
                let p_node_id = para_id(&p.stable_id);
                stable_to_node_id.insert(p.stable_id.clone(), p_node_id.clone());

                nodes.push(GraphNode {
                    id: p_node_id.clone(),
                    name: p.excerpt.clone(),
                    r#type: "paragraph".to_string(),
                    path: Some(abs_str.clone()),
                    size: Some(0.16),
                    file_path: Some(abs_str.clone()),
                    start_line: Some(p.start_line),
                    end_line: Some(p.end_line),
                    stable_id: Some(p.stable_id.clone()),
                    depth: Some(parent_depth + 3),
                });

                let parent = p
                    .parent_section
                    .as_ref()
                    .map(|sid| section_id(sid))
                    .unwrap_or_else(|| id.clone());

                edges.push(GraphEdge {
                    from: parent,
                    to: p_node_id.clone(),
                    strength: 0.28,
                    r#type: Some("contains".to_string()),
                });

                if let Some(prev) = prev_para.take() {
                    edges.push(GraphEdge {
                        from: prev,
                        to: p_node_id.clone(),
                        strength: 0.12,
                        r#type: Some("next".to_string()),
                    });
                }
                prev_para = Some(p_node_id);
            }
        }

        Ok(())
    }

    walk_dir(
        &kb_path,
        &kb_path,
        &kb_id,
        0,
        &mut nodes,
        &mut edges,
        &mut stable_to_node_id,
    )?;

    let mut added: HashSet<(String, String)> = HashSet::new();
    for (from_sid, tos) in structure_relations {
        let Some(from_id) = stable_to_node_id.get(&from_sid) else {
            continue;
        };
        for to_sid in tos {
            let Some(to_id) = stable_to_node_id.get(&to_sid) else {
                continue;
            };
            if from_id == to_id {
                continue;
            }
            let key = (from_id.clone(), to_id.clone());
            if !added.insert(key) {
                continue;
            }
            edges.push(GraphEdge {
                from: from_id.clone(),
                to: to_id.clone(),
                strength: 0.26,
                r#type: Some("related".to_string()),
            });
        }
    }

    Ok(GraphData { nodes, edges })
}