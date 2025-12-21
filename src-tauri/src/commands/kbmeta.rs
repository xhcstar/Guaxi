use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct KbStats {
    pub file_count: u64,
    pub dir_count: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KbFsNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub children: Option<Vec<KbFsNode>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KbMetaFile {
    pub knowledge_base_name: String,
    pub knowledge_base_type: String,
    pub knowledge_base_location: String,
    pub created_at: String,
    pub modified_at: String,
    pub tree: KbFsNode,
    pub stats: KbStats,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FileStructureNode {
    pub title: String,
    pub level: u8,
    pub tags: Vec<String>,
    pub attrs: serde_json::Value,
    pub related_nodes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMetaFile {
    pub file_name: String,
    pub rel_path: String,
    pub created_at: String,
    pub modified_at: String,
    pub file_tags: Vec<String>,
    pub related_nodes: Vec<String>,
    pub structure_nodes: Vec<FileStructureNode>,
}

#[derive(Serialize, Debug, Clone)]
pub struct KbMetaSummary {
    pub knowledge_base_name: String,
    pub knowledge_base_type: String,
    pub knowledge_base_location: String,
    pub created_at: String,
    pub modified_at: String,
    pub file_count: u64,
    pub dir_count: u64,
}

fn now_rfc3339() -> String {
    Local::now().to_rfc3339()
}

fn system_time_to_rfc3339(t: SystemTime) -> String {
    chrono::DateTime::<Local>::from(t).to_rfc3339()
}

fn file_times(path: &Path) -> (String, String) {
    let now = now_rfc3339();
    let Ok(md) = fs::metadata(path) else {
        return (now.clone(), now);
    };

    let created = md.created().ok().map(system_time_to_rfc3339).unwrap_or_else(|| now.clone());
    let modified = md.modified().ok().map(system_time_to_rfc3339).unwrap_or(now);
    (created, modified)
}

fn knowledge_bases_root_dir() -> PathBuf {
    PathBuf::from(crate::commands::settingview::get_knowledge_bases_root())
}

fn kb_root_meta_dir(root: &Path) -> PathBuf {
    root.join(".meta")
}

fn ensure_root_meta_dir(root: &Path) -> Result<(), String> {
    let meta = kb_root_meta_dir(root);
    if !meta.exists() {
        fs::create_dir_all(&meta).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn is_hidden_name(name: &str) -> bool {
    name.trim().starts_with('.')
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

fn kb_meta_dir_name(kb_name: &str) -> String {
    format!("{}_meta", sanitize_meta_stem(kb_name))
}

fn kb_meta_file_name(kb_name: &str) -> String {
    format!("{}_meta.json", sanitize_meta_stem(kb_name))
}

fn kb_meta_paths(root: &Path, kb_name: &str) -> (PathBuf, PathBuf) {
    let dir = kb_root_meta_dir(root).join(kb_meta_dir_name(kb_name));
    let file = dir.join(kb_meta_file_name(kb_name));
    (dir, file)
}

fn read_kb_meta_file(path: &Path) -> Option<KbMetaFile> {
    let s = fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_kb_meta_file(path: &Path, meta: &KbMetaFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let s = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

fn collect_stats(dir: &Path) -> Result<KbStats, String> {
    let mut file_count: u64 = 0;
    let mut dir_count: u64 = 0;

    fn walk(p: &Path, file_count: &mut u64, dir_count: &mut u64) -> Result<(), String> {
        let entries = fs::read_dir(p).map_err(|e| e.to_string())?;
        for ent in entries {
            let ent = ent.map_err(|e| e.to_string())?;
            let path = ent.path();
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if is_hidden_name(&name) {
                continue;
            }
            if name == ".meta" {
                continue;
            }

            let md = ent.metadata().map_err(|e| e.to_string())?;
            if md.is_dir() {
                *dir_count += 1;
                walk(&path, file_count, dir_count)?;
            } else {
                *file_count += 1;
            }
        }
        Ok(())
    }

    walk(dir, &mut file_count, &mut dir_count)?;
    Ok(KbStats { file_count, dir_count })
}

fn rel_path_string(base: &Path, target: &Path) -> String {
    target
        .strip_prefix(base)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| target.to_string_lossy().replace('\\', "/"))
}

fn safe_rel_join(base: &Path, rel: &str) -> Result<PathBuf, String> {
    use std::path::Component;

    let rel = rel.trim().replace('\\', "/");
    if rel.is_empty() {
        return Ok(base.to_path_buf());
    }

    let p = PathBuf::from(&rel);
    for c in p.components() {
        match c {
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err("rel_path 不合法".to_string());
            }
            _ => {}
        }
    }

    Ok(base.join(p))
}

fn build_kb_tree(base: &Path, current: &Path) -> Result<KbFsNode, String> {
    let name = current
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let rel = rel_path_string(base, current);
    let md = fs::metadata(current).map_err(|e| e.to_string())?;
    let is_dir = md.is_dir();

    if !is_dir {
        return Ok(KbFsNode {
            name,
            rel_path: rel,
            is_dir: false,
            children: None,
        });
    }

    let mut children: Vec<KbFsNode> = Vec::new();
    for ent in fs::read_dir(current).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if n.is_empty() || is_hidden_name(n) || n == ".meta" {
            continue;
        }
        children.push(build_kb_tree(base, &p)?);
    }

    children.sort_by(|a, b| {
        let ad = if a.is_dir { 0 } else { 1 };
        let bd = if b.is_dir { 0 } else { 1 };
        ad.cmp(&bd)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(KbFsNode {
        name,
        rel_path: rel,
        is_dir: true,
        children: Some(children),
    })
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
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

fn merge_attrs(scanned: &serde_json::Value, user: &serde_json::Value) -> serde_json::Value {
    let Some(s_obj) = scanned.as_object() else {
        return scanned.clone();
    };

    let mut out = serde_json::Map::new();
    for (k, v) in s_obj.iter() {
        out.insert(k.clone(), v.clone());
    }

    let Some(u_obj) = user.as_object() else {
        return serde_json::Value::Object(out);
    };

    let reserved = ["stable_id", "kind", "start_line", "end_line", "parent_section"];
    for (k, v) in u_obj.iter() {
        if reserved.iter().any(|x| x == k) {
            continue;
        }
        out.insert(k.clone(), v.clone());
    }

    serde_json::Value::Object(out)
}

fn scan_markdown_structure(file_rel: &str, content: &str) -> Vec<FileStructureNode> {
    let mut out: Vec<FileStructureNode> = Vec::new();

    let mut in_fence = false;

    let mut para_lines: Vec<String> = Vec::new();
    let mut para_start_line: u32 = 0;
    let mut last_section: Option<String> = None;

    let mut flush_para = |end_line: u32,
                          para_lines: &mut Vec<String>,
                          para_start_line: &mut u32,
                          last_section: &Option<String>,
                          out: &mut Vec<FileStructureNode>| {
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
        out.push(FileStructureNode {
            title: excerpt,
            level: 0,
            tags: Vec::new(),
            attrs: serde_json::json!({
                "kind": "paragraph",
                "stable_id": stable_id,
                "start_line": *para_start_line,
                "end_line": end_line,
                "parent_section": last_section.clone()
            }),
            related_nodes: Vec::new(),
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
                &mut out,
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
                    &mut out,
                );

                let stable_id = sha256_hex(&format!(
                    "{}\nH{}\nL{}\n{}",
                    file_rel,
                    hashes,
                    line_num,
                    rest
                ));
                last_section = Some(stable_id.clone());

                out.push(FileStructureNode {
                    title: rest.to_string(),
                    level: hashes as u8,
                    tags: Vec::new(),
                    attrs: serde_json::json!({
                        "kind": "section",
                        "stable_id": stable_id,
                        "start_line": line_num
                    }),
                    related_nodes: Vec::new(),
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
                &mut out,
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
        &mut out,
    );

    out
}

fn path_is_under_kb_root(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
}

fn path_is_under_root_meta(root: &Path, path: &Path) -> bool {
    path.starts_with(&kb_root_meta_dir(root))
}

fn kb_name_from_path(root: &Path, path: &Path) -> Option<String> {
    if !path_is_under_kb_root(root, path) {
        return None;
    }
    let rel = path.strip_prefix(root).ok()?;
    let first = rel.components().next()?;
    let s = first.as_os_str().to_string_lossy().to_string();
    if s.is_empty() || s == ".meta" || is_hidden_name(&s) {
        None
    } else {
        Some(s)
    }
}

fn is_top_level_kb_dir(root: &Path, path: &Path) -> bool {
    if let Some(parent) = path.parent() {
        parent == root
    } else {
        false
    }
}

fn kb_meta_file_path(root: &Path, kb_name: &str) -> PathBuf {
    let (_, meta_file) = kb_meta_paths(root, kb_name);
    meta_file
}

fn file_meta_file_path(root: &Path, kb_name: &str, rel_path: &str) -> PathBuf {
    let (meta_dir, _) = kb_meta_paths(root, kb_name);
    let key = sanitize_meta_stem(&rel_path.replace('/', "__"));
    meta_dir.join(format!("{key}_meta.json"))
}

fn read_file_meta_file(path: &Path) -> Option<FileMetaFile> {
    let s = fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_file_meta_file(path: &Path, meta: &FileMetaFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let s = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

fn collect_visible_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::new();

    fn walk(cur: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        for ent in fs::read_dir(cur).map_err(|e| e.to_string())? {
            let ent = ent.map_err(|e| e.to_string())?;
            let path = ent.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.is_empty() || is_hidden_name(name) || name == ".meta" {
                continue;
            }

            let md = ent.metadata().map_err(|e| e.to_string())?;
            if md.is_dir() {
                walk(&path, out)?;
            } else {
                out.push(path);
            }
        }
        Ok(())
    }

    if dir.exists() {
        walk(dir, &mut out)?;
    }
    Ok(out)
}

fn upsert_file_meta_for_abs(
    root: &Path,
    kb_name: &str,
    kb_dir: &Path,
    abs: &Path,
    base: Option<FileMetaFile>,
) -> Result<(), String> {
    let rel = rel_path_string(kb_dir, abs);
    let meta_path = file_meta_file_path(root, kb_name, &rel);

    let existing_disk = read_file_meta_file(&meta_path);
    let base_ref = base.as_ref().or(existing_disk.as_ref());

    let (created_fs, modified_fs) = file_times(abs);
    let created_at = base_ref
        .map(|m| m.created_at.clone())
        .unwrap_or(created_fs);

    let file_tags = base_ref
        .map(|m| m.file_tags.clone())
        .unwrap_or_default();
    let related_nodes = base_ref
        .map(|m| m.related_nodes.clone())
        .unwrap_or_default();

    let mut structure_nodes: Vec<FileStructureNode> = Vec::new();
    if is_markdown_path(abs) {
        let content = fs::read_to_string(abs).unwrap_or_default();
        let scanned = scan_markdown_structure(&rel, &content);
        for mut n in scanned {
            if let Some(prev) = base_ref.and_then(|m| {
                let sid = n.attrs.get("stable_id").and_then(|v| v.as_str());
                if let Some(sid) = sid {
                    if let Some(x) = m.structure_nodes.iter().find(|x| x.attrs.get("stable_id").and_then(|v| v.as_str()) == Some(sid)) {
                        return Some(x);
                    }
                }
                m.structure_nodes
                    .iter()
                    .find(|x| x.level == n.level && x.title == n.title)
            }) {
                n.tags = prev.tags.clone();
                n.related_nodes = prev.related_nodes.clone();
                n.attrs = merge_attrs(&n.attrs, &prev.attrs);
            }
            structure_nodes.push(n);
        }
    }

    let file_name = abs
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let meta = FileMetaFile {
        file_name,
        rel_path: rel,
        created_at,
        modified_at: modified_fs,
        file_tags,
        related_nodes,
        structure_nodes,
    };

    write_file_meta_file(&meta_path, &meta)
}

fn cleanup_file_meta_prefix(root: &Path, kb_name: &str, rel_prefix: &str) -> Result<(), String> {
    let (meta_dir, _) = kb_meta_paths(root, kb_name);
    if !meta_dir.exists() {
        return Ok(());
    }

    let prefix = rel_prefix.trim().trim_start_matches('/').to_string();
    let meta_file = kb_meta_file_name(kb_name);

    for ent in fs::read_dir(&meta_dir).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let p = ent.path();
        if !p.is_file() {
            continue;
        }
        let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if fname == meta_file {
            continue;
        }

        let Some(m) = read_file_meta_file(&p) else {
            continue;
        };

        if m.rel_path == prefix || (!prefix.is_empty() && m.rel_path.starts_with(&(prefix.clone() + "/"))) {
            let _ = fs::remove_file(&p);
        }
    }

    Ok(())
}

fn sync_kb_meta_from_fs(kb_name: &str, kb_dir: &Path, created_at_hint: Option<&str>) -> Result<KbMetaFile, String> {
    let root = knowledge_bases_root_dir();
    ensure_root_meta_dir(&root)?;

    let meta_path = kb_meta_file_path(&root, kb_name);
    let existing = read_kb_meta_file(&meta_path);

    let created_at = existing
        .as_ref()
        .map(|m| m.created_at.clone())
        .or_else(|| created_at_hint.map(|s| s.to_string()))
        .unwrap_or_else(now_rfc3339);

    let kb_type = existing
        .as_ref()
        .map(|m| m.knowledge_base_type.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未分类".to_string());

    let stats = collect_stats(kb_dir).unwrap_or_default();
    let tree = build_kb_tree(kb_dir, kb_dir)?;
    let modified_at = now_rfc3339();

    let meta = KbMetaFile {
        knowledge_base_name: kb_name.to_string(),
        knowledge_base_type: kb_type,
        knowledge_base_location: kb_dir.to_string_lossy().to_string(),
        created_at,
        modified_at,
        tree,
        stats,
    };

    write_kb_meta_file(&meta_path, &meta)?;
    Ok(meta)
}

pub fn init_kb_meta_for_new_kb(kb_name: &str, kb_dir: &Path, created_at: &str) -> Result<(), String> {
    let root = knowledge_bases_root_dir();
    ensure_root_meta_dir(&root)?;

    let (meta_dir, _) = kb_meta_paths(&root, kb_name);
    if !meta_dir.exists() {
        fs::create_dir_all(&meta_dir).map_err(|e| e.to_string())?;
    }

    let _ = sync_kb_meta_from_fs(kb_name, kb_dir, Some(created_at))?;
    Ok(())
}

pub fn after_write_file(path: &Path, content: &str) -> Result<(), String> {
    let root = knowledge_bases_root_dir();
    if !crate::commands::settingview::is_knowledge_bases_root_path(&path.to_string_lossy()) {
        return Ok(());
    }
    if path_is_under_root_meta(&root, path) {
        return Ok(());
    }

    let Some(kb_name) = kb_name_from_path(&root, path) else {
        return Ok(());
    };
    let kb_dir = root.join(&kb_name);

    let _ = sync_kb_meta_from_fs(&kb_name, &kb_dir, None)?;

    if is_markdown_path(path) {
        let rel = rel_path_string(&kb_dir, path);
        let meta_path = file_meta_file_path(&root, &kb_name, &rel);
        let existing = read_file_meta_file(&meta_path);

        let (created_fs, modified_fs) = file_times(path);
        let created_at = existing.as_ref().map(|m| m.created_at.clone()).unwrap_or(created_fs);

        let mut scanned = scan_markdown_structure(&rel, content);
        if let Some(old) = existing.as_ref() {
            for n in scanned.iter_mut() {
                let sid = n.attrs.get("stable_id").and_then(|v| v.as_str());
                let prev = sid
                    .and_then(|sid| old.structure_nodes.iter().find(|x| x.attrs.get("stable_id").and_then(|v| v.as_str()) == Some(sid)))
                    .or_else(|| old.structure_nodes.iter().find(|x| x.level == n.level && x.title == n.title));

                if let Some(prev) = prev {
                    n.tags = prev.tags.clone();
                    n.related_nodes = prev.related_nodes.clone();
                    n.attrs = merge_attrs(&n.attrs, &prev.attrs);
                }
            }
        }

        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let meta = FileMetaFile {
            file_name,
            rel_path: rel,
            created_at,
            modified_at: modified_fs,
            file_tags: existing.as_ref().map(|m| m.file_tags.clone()).unwrap_or_default(),
            related_nodes: existing.as_ref().map(|m| m.related_nodes.clone()).unwrap_or_default(),
            structure_nodes: scanned,
        };

        write_file_meta_file(&meta_path, &meta)?;
    }

    Ok(())
}

pub fn after_create_path(path: &Path, is_dir: bool, content: Option<&str>) -> Result<(), String> {
    let root = knowledge_bases_root_dir();
    if !crate::commands::settingview::is_knowledge_bases_root_path(&path.to_string_lossy()) {
        return Ok(());
    }
    if path_is_under_root_meta(&root, path) {
        return Ok(());
    }

    if is_dir && is_top_level_kb_dir(&root, path) {
        let kb_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if kb_name.is_empty() || kb_name == ".meta" || is_hidden_name(&kb_name) {
            return Ok(());
        }
        let created = now_rfc3339();
        return init_kb_meta_for_new_kb(&kb_name, path, &created);
    }

    let Some(kb_name) = kb_name_from_path(&root, path) else {
        return Ok(());
    };
    let kb_dir = root.join(&kb_name);

    let _ = sync_kb_meta_from_fs(&kb_name, &kb_dir, None)?;

    if !is_dir {
        let rel = rel_path_string(&kb_dir, path);
        let meta_path = file_meta_file_path(&root, &kb_name, &rel);
        if !meta_path.exists() {
            let (created_at, modified_at) = file_times(path);
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let structure_nodes = if is_markdown_path(path) {
                scan_markdown_structure(&rel, content.unwrap_or(""))
            } else {
                Vec::new()
            };

            let meta = FileMetaFile {
                file_name,
                rel_path: rel,
                created_at,
                modified_at,
                file_tags: Vec::new(),
                related_nodes: Vec::new(),
                structure_nodes,
            };
            write_file_meta_file(&meta_path, &meta)?;
        }
    }

    Ok(())
}

pub fn after_delete_path(path: &Path, is_dir: bool) -> Result<(), String> {
    let root = knowledge_bases_root_dir();
    if !crate::commands::settingview::is_knowledge_bases_root_path(&path.to_string_lossy()) {
        return Ok(());
    }
    if path_is_under_root_meta(&root, path) {
        return Ok(());
    }

    if is_dir && is_top_level_kb_dir(&root, path) {
        let kb_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if kb_name.is_empty() || kb_name == ".meta" || is_hidden_name(&kb_name) {
            return Ok(());
        }
        let (meta_dir, _) = kb_meta_paths(&root, &kb_name);
        if meta_dir.exists() {
            let _ = fs::remove_dir_all(&meta_dir);
        }
        return Ok(());
    }

    let Some(kb_name) = kb_name_from_path(&root, path) else {
        return Ok(());
    };
    let kb_dir = root.join(&kb_name);

    if !is_dir {
        let rel = rel_path_string(&kb_dir, path);
        let meta_path = file_meta_file_path(&root, &kb_name, &rel);
        if meta_path.exists() {
            let _ = fs::remove_file(&meta_path);
        }
    }

    let _ = sync_kb_meta_from_fs(&kb_name, &kb_dir, None)?;
    Ok(())
}

pub fn after_move_path(source_path: &Path, destination_path: &Path) -> Result<(), String> {
    let root = knowledge_bases_root_dir();
    let src_in = crate::commands::settingview::is_knowledge_bases_root_path(&source_path.to_string_lossy());
    let dst_in = crate::commands::settingview::is_knowledge_bases_root_path(&destination_path.to_string_lossy());
    if !src_in && !dst_in {
        return Ok(());
    }
    if path_is_under_root_meta(&root, source_path) || path_is_under_root_meta(&root, destination_path) {
        return Ok(());
    }

    let dst_is_dir = fs::metadata(destination_path)
        .map(|m| m.is_dir())
        .unwrap_or(false);

    let src_top = is_top_level_kb_dir(&root, source_path);
    let dst_top = is_top_level_kb_dir(&root, destination_path);

    if dst_is_dir && src_top && dst_top {
        let old_name = source_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let new_name = destination_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if !old_name.is_empty()
            && !new_name.is_empty()
            && old_name != ".meta"
            && new_name != ".meta"
            && !is_hidden_name(&old_name)
            && !is_hidden_name(&new_name)
        {
            ensure_root_meta_dir(&root)?;
            let (old_meta_dir, _) = kb_meta_paths(&root, &old_name);
            let (new_meta_dir, _) = kb_meta_paths(&root, &new_name);
            if old_meta_dir.exists() && !new_meta_dir.exists() {
                let _ = fs::rename(&old_meta_dir, &new_meta_dir);
            }

            let _ = sync_kb_meta_from_fs(&new_name, destination_path, None)?;
            return Ok(());
        }
    }

    let src_kb = kb_name_from_path(&root, source_path);
    let dst_kb = kb_name_from_path(&root, destination_path);

    if src_kb.is_none() && dst_kb.is_none() {
        return Ok(());
    }

    if let Some(dst_name) = dst_kb.as_ref() {
        let dst_dir = root.join(dst_name);

        if dst_is_dir {
            let files = collect_visible_files(destination_path)?;
            for f in files {
                let rel_sub = f.strip_prefix(destination_path).ok();

                let mut base: Option<FileMetaFile> = None;
                let mut old_meta_path: Option<PathBuf> = None;

                if let (Some(src_name), Some(rel_sub)) = (src_kb.as_ref(), rel_sub) {
                    let src_dir = root.join(src_name);
                    let old_abs = source_path.join(rel_sub);
                    let old_rel = rel_path_string(&src_dir, &old_abs);
                    let p = file_meta_file_path(&root, src_name, &old_rel);
                    base = read_file_meta_file(&p);
                    old_meta_path = Some(p);
                }

                upsert_file_meta_for_abs(&root, dst_name, &dst_dir, &f, base)?;

                if let Some(p) = old_meta_path {
                    if p.exists() {
                        let _ = fs::remove_file(&p);
                    }
                }
            }
        } else {
            let mut base: Option<FileMetaFile> = None;
            let mut old_meta_path: Option<PathBuf> = None;

            if let Some(src_name) = src_kb.as_ref() {
                let src_dir = root.join(src_name);
                let old_rel = rel_path_string(&src_dir, source_path);
                let p = file_meta_file_path(&root, src_name, &old_rel);
                base = read_file_meta_file(&p);
                old_meta_path = Some(p);
            }

            upsert_file_meta_for_abs(&root, dst_name, &dst_dir, destination_path, base)?;

            if let Some(p) = old_meta_path {
                if p.exists() {
                    let _ = fs::remove_file(&p);
                }
            }
        }
    }

    if let Some(src_name) = src_kb.as_ref() {
        if dst_kb.is_none() {
            let src_dir = root.join(src_name);
            let prefix = rel_path_string(&src_dir, source_path);

            if dst_is_dir {
                let _ = cleanup_file_meta_prefix(&root, src_name, &prefix);
            } else {
                let p = file_meta_file_path(&root, src_name, &prefix);
                if p.exists() {
                    let _ = fs::remove_file(&p);
                }
            }
        }
    }

    if let Some(kb_name) = src_kb {
        let kb_dir = root.join(&kb_name);
        let _ = sync_kb_meta_from_fs(&kb_name, &kb_dir, None);
    }
    if let Some(kb_name) = dst_kb {
        let kb_dir = root.join(&kb_name);
        let _ = sync_kb_meta_from_fs(&kb_name, &kb_dir, None);
    }

    Ok(())
}

#[tauri::command]
pub async fn create_knowledge_base_manager_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
) -> Result<(), String> {
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let url = WebviewUrl::App("/kb-manager".into());

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1000.0, 720.0)
        .min_inner_size(820.0, 560.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn kbmeta_sync_all() -> Result<Vec<KbMetaSummary>, String> {
    let root = knowledge_bases_root_dir();
    ensure_root_meta_dir(&root)?;

    if !root.exists() {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    }

    let mut out: Vec<KbMetaSummary> = Vec::new();
    for ent in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let ent = ent.map_err(|e| e.to_string())?;
        let path = ent.path();
        let md = ent.metadata().map_err(|e| e.to_string())?;
        if !md.is_dir() {
            continue;
        }

        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if name.is_empty() || name == ".meta" || is_hidden_name(&name) {
            continue;
        }

        let meta = sync_kb_meta_from_fs(&name, &path, None)?;
        out.push(KbMetaSummary {
            knowledge_base_name: meta.knowledge_base_name,
            knowledge_base_type: meta.knowledge_base_type,
            knowledge_base_location: meta.knowledge_base_location,
            created_at: meta.created_at,
            modified_at: meta.modified_at,
            file_count: meta.stats.file_count,
            dir_count: meta.stats.dir_count,
        });
    }

    out.sort_by(|a, b| a.knowledge_base_name.to_lowercase().cmp(&b.knowledge_base_name.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn kbmeta_read_kb_meta(knowledge_base_name: String) -> Result<KbMetaFile, String> {
    let root = knowledge_bases_root_dir();
    let kb_dir = root.join(&knowledge_base_name);
    if !kb_dir.exists() {
        return Err("知识库不存在".to_string());
    }
    sync_kb_meta_from_fs(&knowledge_base_name, &kb_dir, None)
}

#[tauri::command]
pub fn kbmeta_save_kb_meta(meta: KbMetaFile) -> Result<(), String> {
    let kb_name = meta.knowledge_base_name.trim().to_string();
    if kb_name.is_empty() {
        return Err("知识库名不能为空".to_string());
    }

    let root = knowledge_bases_root_dir();
    let kb_dir = root.join(&kb_name);
    if !kb_dir.exists() {
        return Err("知识库不存在".to_string());
    }

    let mut fresh = sync_kb_meta_from_fs(&kb_name, &kb_dir, None)?;
    let tp = meta.knowledge_base_type.trim().to_string();
    if !tp.is_empty() {
        fresh.knowledge_base_type = tp;
    }

    let meta_path = kb_meta_file_path(&root, &kb_name);
    write_kb_meta_file(&meta_path, &fresh)?;
    Ok(())
}

#[tauri::command]
pub fn kbmeta_read_file_meta(knowledge_base_name: String, rel_path: String) -> Result<FileMetaFile, String> {
    let kb_name = knowledge_base_name.trim().to_string();
    if kb_name.is_empty() {
        return Err("知识库名不能为空".to_string());
    }

    let root = knowledge_bases_root_dir();
    let kb_dir = root.join(&kb_name);
    if !kb_dir.exists() {
        return Err("知识库不存在".to_string());
    }

    let abs = safe_rel_join(&kb_dir, &rel_path)?;
    if !abs.exists() {
        return Err("文件不存在".to_string());
    }

    let rel = rel_path_string(&kb_dir, &abs);
    let meta_path = file_meta_file_path(&root, &kb_name, &rel);

    if let Some(m) = read_file_meta_file(&meta_path) {
        return Ok(m);
    }

    let (created_at, modified_at) = file_times(&abs);
    let file_name = abs.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let content = if is_markdown_path(&abs) { fs::read_to_string(&abs).unwrap_or_default() } else { String::new() };

    let meta = FileMetaFile {
        file_name,
        rel_path: rel.clone(),
        created_at,
        modified_at,
        file_tags: Vec::new(),
        related_nodes: Vec::new(),
        structure_nodes: if is_markdown_path(&abs) { scan_markdown_structure(&rel, &content) } else { Vec::new() },
    };

    write_file_meta_file(&meta_path, &meta)?;
    Ok(meta)
}

#[tauri::command]
pub fn kbmeta_save_file_meta(knowledge_base_name: String, rel_path: String, meta: FileMetaFile) -> Result<(), String> {
    let kb_name = knowledge_base_name.trim().to_string();
    if kb_name.is_empty() {
        return Err("知识库名不能为空".to_string());
    }

    let root = knowledge_bases_root_dir();
    let kb_dir = root.join(&kb_name);
    if !kb_dir.exists() {
        return Err("知识库不存在".to_string());
    }

    let abs = safe_rel_join(&kb_dir, &rel_path)?;
    if !abs.exists() {
        return Err("文件不存在".to_string());
    }

    let rel = rel_path_string(&kb_dir, &abs);
    let meta_path = file_meta_file_path(&root, &kb_name, &rel);

    let existing = read_file_meta_file(&meta_path);
    let (created_fs, modified_fs) = file_times(&abs);
    let created_at = existing.as_ref().map(|m| m.created_at.clone()).unwrap_or(created_fs);

    let mut structure_nodes = Vec::new();
    if is_markdown_path(&abs) {
        let content = fs::read_to_string(&abs).unwrap_or_default();
        let scanned = scan_markdown_structure(&rel, &content);
        for mut n in scanned {
            let sid = n.attrs.get("stable_id").and_then(|v| v.as_str());
            let user = sid
                .and_then(|sid| meta.structure_nodes.iter().find(|x| x.attrs.get("stable_id").and_then(|v| v.as_str()) == Some(sid)))
                .or_else(|| meta.structure_nodes.iter().find(|x| x.level == n.level && x.title == n.title));

            if let Some(user) = user {
                n.tags = user.tags.clone();
                n.related_nodes = user.related_nodes.clone();
                n.attrs = merge_attrs(&n.attrs, &user.attrs);
            }
            structure_nodes.push(n);
        }
    }

    let file_name = abs.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let saved = FileMetaFile {
        file_name,
        rel_path: rel,
        created_at,
        modified_at: modified_fs,
        file_tags: meta.file_tags,
        related_nodes: meta.related_nodes,
        structure_nodes,
    };

    write_file_meta_file(&meta_path, &saved)?;
    let _ = sync_kb_meta_from_fs(&kb_name, &kb_dir, None);
    Ok(())
}