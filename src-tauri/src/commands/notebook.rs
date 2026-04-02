use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    #[serde(rename = "type")]
    pub cell_type: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookSyncResult {
    pub cells: Vec<NotebookCell>,
    pub current_digest: String,
    pub sidecar_digest_match: bool,
    pub restored_from_sidecar: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct NotebookSidecarPayload {
    #[serde(default)]
    #[serde(rename = "sourceDigest")]
    source_digest: String,
    #[serde(default)]
    cells: Vec<NotebookCell>,
}

#[derive(Debug, Clone)]
struct PyBlock {
    start_line: usize,
    end_line: usize,
    code: String,
    meta: Map<String, Value>,
}

fn normalize_markdown_text(markdown: &str) -> String {
    markdown.replace("\r\n", "\n").replace('\r', "\n")
}

fn strip_output_blocks(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut kept = Vec::new();
    let mut inside_output = false;
    for line in lines {
        let text = line.trim();
        if text.starts_with("<!-- guaxi:pyout:") && text.ends_with(":start -->") {
            inside_output = true;
            continue;
        }
        if inside_output && text.starts_with("<!-- guaxi:pyout:") && text.ends_with(":end -->") {
            inside_output = false;
            continue;
        }
        if !inside_output {
            kept.push(line);
        }
    }
    kept.join("\n")
}

fn compute_digest(text: &str) -> String {
    let mut hash: u32 = 2166136261;
    for byte in text.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    format!("nb-{:x}-{}", hash, text.len())
}

fn default_python_cell_meta() -> Map<String, Value> {
    let mut meta = Map::new();
    meta.insert("title".to_string(), Value::String(String::new()));
    meta.insert("codeCollapsed".to_string(), Value::Bool(false));
    meta.insert("outputCollapsed".to_string(), Value::Bool(false));
    meta.insert("sessionCollapsed".to_string(), Value::Bool(true));
    meta.insert("consoleCollapsed".to_string(), Value::Bool(false));
    meta
}

fn normalize_python_cell_meta(raw: Option<&Value>) -> Map<String, Value> {
    let mut meta = default_python_cell_meta();
    if let Some(Value::Object(obj)) = raw {
        for (key, value) in obj {
            meta.insert(key.clone(), value.clone());
        }
    }
    if let Some(Value::String(title)) = meta.get("title") {
        meta.insert("title".to_string(), Value::String(title.trim().to_string()));
    }
    meta
}

fn try_parse_python_cell_meta(raw_meta: &str) -> Map<String, Value> {
    let text = raw_meta.trim();
    if text.is_empty() {
        return default_python_cell_meta();
    }
    match serde_json::from_str::<Value>(text) {
        Ok(value) => normalize_python_cell_meta(Some(&value)),
        Err(_) => default_python_cell_meta(),
    }
}

fn create_fallback_stable_id(seed: &str) -> String {
    format!("py-rs-{}", compute_digest(seed).replace("nb-", ""))
}

fn parse_python_cell_blocks(markdown: &str) -> Vec<PyBlock> {
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].trim();
        let (is_start, meta_text) = if line == "<!--py-->" {
            (true, "")
        } else if line.starts_with("<!--py ") && line.ends_with("-->") {
            let inner = line
                .trim_start_matches("<!--py")
                .trim_end_matches("-->")
                .trim();
            (true, inner)
        } else if line.starts_with("<!-- guaxi:py-cell") && line.ends_with("-->") {
            let inner = line
                .trim_start_matches("<!-- guaxi:py-cell")
                .trim_end_matches("-->")
                .trim();
            (true, inner)
        } else {
            (false, "")
        };

        if !is_start {
            index += 1;
            continue;
        }

        let mut end_index = None;
        for j in (index + 1)..lines.len() {
            let text = lines[j].trim();
            if text == "<!--/py-->" || text == "<!-- /py -->" || text == "<!--/guaxi:py-cell-->" || text == "<!-- /guaxi:py-cell -->" {
                end_index = Some(j);
                break;
            }
        }
        let Some(end) = end_index else {
            index += 1;
            continue;
        };

        let mut meta = try_parse_python_cell_meta(meta_text);
        if !meta.contains_key("id") || meta.get("id").and_then(Value::as_str).unwrap_or("").trim().is_empty() {
            let seed = format!("{}:{}:{}", index + 1, end + 1, lines[(index + 1)..end].join("\n"));
            meta.insert("id".to_string(), Value::String(create_fallback_stable_id(&seed)));
        }

        blocks.push(PyBlock {
            start_line: index + 1,
            end_line: end + 1,
            code: lines[(index + 1)..end].join("\n"),
            meta,
        });
        index = end + 1;
    }

    blocks
}

fn detect_frontmatter_end(lines: &[&str]) -> Option<usize> {
    if lines.len() < 3 || lines.first().map(|v| v.trim()) != Some("---") {
        return None;
    }
    for index in 1..lines.len() {
        let text = lines[index].trim();
        if text == "---" || text == "..." {
            let mut end_exclusive = index + 1;
            while end_exclusive < lines.len() && lines[end_exclusive].trim().is_empty() {
                end_exclusive += 1;
            }
            return Some(end_exclusive);
        }
    }
    None
}

fn split_markdown_segments(markdown: &str) -> Vec<String> {
    if markdown.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut segments = Vec::new();
    let mut start_line = 0usize;

    if let Some(frontmatter_end) = detect_frontmatter_end(&lines) {
        let segment = lines[0..frontmatter_end].join("\n");
        if !segment.is_empty() {
            segments.push(segment);
        }
        start_line = frontmatter_end;
    }

    let mut heading_indexes = Vec::new();
    let mut fence_active = false;
    let mut fence_char = '\0';
    let mut fence_len = 0usize;

    for index in start_line..lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if !fence_active && trimmed.starts_with('#') && trimmed.chars().take_while(|c| *c == '#').count() <= 6 {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            let rest = trimmed.chars().nth(hash_count);
            if matches!(rest, Some(' ') | None) {
                heading_indexes.push(index);
            }
        }
        let fence_trimmed = trimmed;
        if fence_trimmed.starts_with("```") || fence_trimmed.starts_with("~~~") {
            let current_char = fence_trimmed.chars().next().unwrap_or('\0');
            let current_len = fence_trimmed.chars().take_while(|c| *c == current_char).count();
            if !fence_active {
                fence_active = true;
                fence_char = current_char;
                fence_len = current_len;
            } else if fence_char == current_char && current_len >= fence_len {
                fence_active = false;
                fence_char = '\0';
                fence_len = 0;
            }
        }
    }

    let push_segment = |segments: &mut Vec<String>, slice: &[&str]| {
        let segment = slice.join("\n");
        if !segment.is_empty() {
            segments.push(segment);
        }
    };

    if !heading_indexes.is_empty() {
        if heading_indexes[0] > start_line {
            push_segment(&mut segments, &lines[start_line..heading_indexes[0]]);
        }
        for (idx, heading_start) in heading_indexes.iter().enumerate() {
            let end = *heading_indexes.get(idx + 1).unwrap_or(&lines.len());
            push_segment(&mut segments, &lines[*heading_start..end]);
        }
    } else {
        let mut chunk_start = start_line;
        fence_active = false;
        fence_char = '\0';
        fence_len = 0;
        let mut index = start_line;
        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                let current_char = trimmed.chars().next().unwrap_or('\0');
                let current_len = trimmed.chars().take_while(|c| *c == current_char).count();
                if !fence_active {
                    fence_active = true;
                    fence_char = current_char;
                    fence_len = current_len;
                } else if fence_char == current_char && current_len >= fence_len {
                    fence_active = false;
                    fence_char = '\0';
                    fence_len = 0;
                }
            }
            if !fence_active && line.trim().is_empty() {
                let mut blank_end = index;
                while blank_end + 1 < lines.len() && lines[blank_end + 1].trim().is_empty() {
                    blank_end += 1;
                }
                if blank_end > index {
                    push_segment(&mut segments, &lines[chunk_start..(blank_end + 1)]);
                    chunk_start = blank_end + 1;
                    index = blank_end;
                }
            }
            index += 1;
        }
        if chunk_start < lines.len() {
            push_segment(&mut segments, &lines[chunk_start..]);
        }
    }

    segments
}

fn build_markdown_cells(markdown: &str) -> Vec<NotebookCell> {
    let text = normalize_markdown_text(markdown);
    let lines: Vec<&str> = text.split('\n').collect();
    let blocks = parse_python_cell_blocks(&text);
    let mut cells = Vec::new();
    let mut cursor = 0usize;

    for (block_index, block) in blocks.iter().enumerate() {
        let start_index = block.start_line.saturating_sub(1);
        let end_exclusive = block.end_line;
        let leading = lines[cursor..start_index].join("\n");
        let leading_cells = split_markdown_segments(&leading);
        if !leading_cells.is_empty() {
            for (leading_index, content) in leading_cells.iter().enumerate() {
                cells.push(NotebookCell {
                    cell_type: "markdown".to_string(),
                    key: format!("nb-md-{}-{}-{}", block_index, cursor, leading_index),
                    content: Some(content.clone()),
                    code: None,
                    meta: None,
                });
            }
        } else if cells.is_empty() {
            cells.push(NotebookCell {
                cell_type: "markdown".to_string(),
                key: format!("nb-md-{}-{}-empty", block_index, cursor),
                content: Some(String::new()),
                code: None,
                meta: None,
            });
        }

        let stable_id = block
            .meta
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        cells.push(NotebookCell {
            cell_type: "python".to_string(),
            key: stable_id.clone(),
            content: None,
            code: Some(block.code.clone()),
            meta: Some(Value::Object(block.meta.clone())),
        });
        cursor = end_exclusive;
    }

    let trailing = lines[cursor..].join("\n");
    let trailing_cells = split_markdown_segments(&trailing);
    if !trailing_cells.is_empty() {
        for (index, content) in trailing_cells.iter().enumerate() {
            cells.push(NotebookCell {
                cell_type: "markdown".to_string(),
                key: format!("nb-md-tail-{}-{}", cursor, index),
                content: Some(content.clone()),
                code: None,
                meta: None,
            });
        }
    } else if cells.is_empty() || cells.last().map(|cell| cell.cell_type.as_str()) == Some("python") {
        cells.push(NotebookCell {
            cell_type: "markdown".to_string(),
            key: format!("nb-md-tail-{}-empty", cursor),
            content: Some(String::new()),
            code: None,
            meta: None,
        });
    }

    cells
}

fn cell_digest(cell: &NotebookCell) -> String {
    match cell.cell_type.as_str() {
        "python" => compute_digest(cell.code.as_deref().unwrap_or("")),
        _ => compute_digest(cell.content.as_deref().unwrap_or("")),
    }
}

fn cell_stable_id(cell: &NotebookCell) -> String {
    cell.meta
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|meta| meta.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn reconcile_cells_with_sidecar(scanned_cells: Vec<NotebookCell>, sidecar: Option<&NotebookSidecarPayload>) -> Vec<NotebookCell> {
    let Some(sidecar_payload) = sidecar else {
        return scanned_cells;
    };
    if sidecar_payload.cells.is_empty() {
        return scanned_cells;
    }

    let mut by_stable_id = std::collections::HashMap::new();
    let mut by_code_digest: std::collections::HashMap<String, Vec<NotebookCell>> = std::collections::HashMap::new();
    for cell in &sidecar_payload.cells {
        if cell.cell_type != "python" {
            continue;
        }
        let stable_id = cell_stable_id(cell);
        if !stable_id.is_empty() {
            by_stable_id.insert(stable_id, cell.clone());
        }
        by_code_digest
            .entry(cell_digest(cell))
            .or_default()
            .push(cell.clone());
    }

    scanned_cells
        .into_iter()
        .map(|cell| {
            if cell.cell_type != "python" {
                return cell;
            }
            let stable_id = cell_stable_id(&cell);
            let persisted = if !stable_id.is_empty() {
                by_stable_id.get(&stable_id).cloned()
            } else {
                None
            }
            .or_else(|| {
                let digest = cell_digest(&cell);
                let candidates = by_code_digest.get(&digest)?;
                if candidates.len() == 1 {
                    Some(candidates[0].clone())
                } else {
                    None
                }
            });

            if let Some(persisted_cell) = persisted {
                let mut next = cell.clone();
                if let (Some(Value::Object(mut persisted_meta)), Some(Value::Object(current_meta))) =
                    (persisted_cell.meta.clone(), cell.meta.clone())
                {
                    for (key, value) in current_meta {
                        persisted_meta.insert(key, value);
                    }
                    next.meta = Some(Value::Object(persisted_meta));
                }
                next
            } else {
                cell
            }
        })
        .collect()
}

#[tauri::command]
pub fn notebook_sync_from_markdown(markdown: String, sidecar_json: Option<String>) -> Result<NotebookSyncResult, String> {
    let normalized = normalize_markdown_text(&markdown);
    let normalized_for_digest = strip_output_blocks(&normalized);
    let current_digest = compute_digest(&normalized_for_digest);
    let sidecar = sidecar_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<NotebookSidecarPayload>(raw).ok());
    let sidecar_digest_match = sidecar
        .as_ref()
        .map(|payload| !payload.source_digest.trim().is_empty() && payload.source_digest.trim() == current_digest)
        .unwrap_or(false);
    let scanned_cells = build_markdown_cells(&normalized);
    let reconciled_cells = reconcile_cells_with_sidecar(scanned_cells, sidecar.as_ref());

    Ok(NotebookSyncResult {
        cells: reconciled_cells,
        current_digest,
        sidecar_digest_match,
        restored_from_sidecar: sidecar.as_ref().map(|payload| !payload.cells.is_empty()).unwrap_or(false),
    })
}
