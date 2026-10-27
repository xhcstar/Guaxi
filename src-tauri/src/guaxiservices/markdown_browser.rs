use crate::commands::notebook::preprocess_markdown_for_static_render;
use crate::commands::knowledgebase::get_knowledge_base_list;
use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tracing::{error, info, warn};
use urlencoding::{decode, encode};

/// 预览页 vendor 资源的内容指纹（由 build.rs 计算并注入）。
///
/// 资源随二进制打包、运行期不会变，因此可以永久缓存；URL 上的指纹保证升级后
/// 浏览器拿到的是新文件，而不是一年前缓存下来的旧 mermaid / ECharts。
const PREVIEW_ASSET_VERSION: &str = env!("GUAXI_PREVIEW_ASSET_VERSION");

/// 静态 vendor 资源的缓存策略。只用于内容指纹已经进 URL 的端点。
const ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace("'", "&#39;")
}

fn is_markdown_file(path: &Path) -> bool {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
        Some(ext) if ext == "md" || ext == "markdown" => true,
        _ => false,
    }
}

fn filename_ascii_fallback(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ' ') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "download.md".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 用户在软件内**显式**点过「在浏览器中查看」的文件。
///
/// 预览端点的放行规则原本只有「文件落在某个知识库根目录下」，这对知识编辑模式够用，
/// 但文件编辑 / 任务编辑模式打开的是任意外部目录（含可配置的任务根目录），
/// 它们不在知识库根下，直接请求会 403。
///
/// 这里沿用 `markdown_image_process::ResolvedImageRegistry` 的思路：授权只来自软件内的一次
/// 明确动作（前端调用 `remember_browser_preview_path` 命令），HTTP 端点本身不扩大可见范围 ——
/// 没有这条登记，任意外部路径依然读不到。
const REMEMBERED_PREVIEW_LIMIT: usize = 2_000;

fn remembered_preview_files() -> &'static Mutex<HashSet<PathBuf>> {
    static REGISTRY: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
}

fn remember_preview_file(path: &Path) {
    let Ok(mut files) = remembered_preview_files().lock() else {
        return;
    };
    if files.len() >= REMEMBERED_PREVIEW_LIMIT {
        files.clear();
    }
    files.insert(path.to_path_buf());
}

fn is_remembered_preview_file(path: &Path) -> bool {
    remembered_preview_files()
        .lock()
        .map(|files| files.contains(path))
        .unwrap_or(false)
}

/// 该文件是否与某个「显式点开预览」的 Markdown 同目录。
///
/// 图表围栏的 `data: ./x.csv` 是相对笔记目录解析的；被预览的笔记既然已被用户显式放行，
/// 它自己目录下的数据文件也应可读，否则 plain/task 模式的图表在浏览器里全是读取失败。
fn is_sibling_of_remembered_preview(path: &Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    remembered_preview_files()
        .lock()
        .map(|files| files.iter().any(|f| f.parent() == Some(dir)))
        .unwrap_or(false)
}

/// 前端「在浏览器中查看」前调用：把该 Markdown 文件登记为本次会话可预览。
///
/// 只接受真实存在的 .md/.markdown 文件；目录、二进制一律拒绝，
/// 免得这条命令被当成通用的「放行任意路径」入口。
#[tauri::command]
pub fn remember_browser_preview_path(path: String) -> Result<(), String> {
    // 这里收到的是前端直接给的文件系统路径，不做百分号解码：
    // 文件名里带 `%` 时解码会失败，而 IPC 这条路本来就不需要编解码。
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Empty path".to_string());
    }
    // 只在 Windows 上剥掉 `\\?\` 前缀：这个前缀本身就是以两个反斜杠开头，
    // 在其它平台上无条件剥会把 UNC 风格的绝对路径改坏。
    let raw = trimmed.strip_prefix(r"\\?\").unwrap_or(trimmed);
    let file_canon = PathBuf::from(raw)
        .canonicalize()
        .map_err(|_| "File not found".to_string())?;

    if !file_canon.is_file() {
        return Err("Not a file".to_string());
    }
    if !is_markdown_file(&file_canon) {
        return Err("Not a markdown file".to_string());
    }

    remember_preview_file(&file_canon);
    info!(
        file = file_canon.to_string_lossy().as_ref(),
        "browser preview: path allowed for preview"
    );
    Ok(())
}

fn resolve_allowed_markdown_file(raw_path: &str) -> Result<PathBuf, String> {
    info!(raw_path = raw_path, "browser preview: resolve file start");

    let decoded_raw = decode(raw_path)
        .map_err(|_| "Invalid path encoding".to_string())?
        .into_owned();

    info!(
        decoded_raw = decoded_raw.as_str(),
        "browser preview: decoded path"
    );

    let decoded = decoded_raw.trim_start_matches(r"\\?\").to_string();
    info!(
        decoded = decoded.as_str(),
        "browser preview: decoded path trimmed"
    );

    let candidate = PathBuf::from(decoded);
    let file_canon = candidate
        .canonicalize()
        .map_err(|_| "File not found".to_string())?;

    info!(
        file_canon = file_canon.to_string_lossy().as_ref(),
        "browser preview: canonicalized"
    );

    if !file_canon.is_file() {
        return Err("Not a file".to_string());
    }

    if !is_markdown_file(&file_canon) {
        return Err("Not a markdown file".to_string());
    }

    // 用户在软件内显式点开过的文件直接放行：允许根可能一个都没有（没配知识库），
    // 也可能不覆盖 plain/task 模式的外部目录，这条登记就是补上后者。
    // 放在查询知识库之前，避免「没配知识库 / 库表打不开」把已授权的预览一起挡掉。
    if is_remembered_preview_file(&file_canon) {
        info!(
            file = file_canon.to_string_lossy().as_ref(),
            "browser preview: allowed by explicit user request"
        );
        return Ok(file_canon);
    }

    let kbs = get_knowledge_base_list().map_err(|e| e.to_string())?;
    info!(
        kb_count = kbs.len(),
        "browser preview: loaded knowledge bases"
    );

    let mut allowed_roots: Vec<PathBuf> = Vec::new();

    for kb in kbs {
        let root = PathBuf::from(kb.location);
        if let Ok(canon) = root.canonicalize() {
            allowed_roots.push(canon);
        }
    }

    if allowed_roots.is_empty() {
        warn!("browser preview: no allowed roots (no knowledge bases?)");
        return Err("No knowledge bases".to_string());
    }

    let mut matched_root: Option<&PathBuf> = None;
    for root in &allowed_roots {
        if file_canon.starts_with(root) {
            matched_root = Some(root);
            break;
        }
    }

    if let Some(root) = matched_root {
        info!(
            root = root.to_string_lossy().as_ref(),
            "browser preview: allowed root matched"
        );
    } else {
        warn!(
            file_canon = file_canon.to_string_lossy().as_ref(),
            "browser preview: forbidden (not under any knowledge base root)"
        );
        return Err("Forbidden".to_string());
    }

    Ok(file_canon)
}

/// 解析预览页要读取的数据文件，并限制在允许的根目录内。
///
/// 允许的根与 markdown 预览一致：所有知识库根 + 资源库根。
/// 只放行纯文本类扩展名 —— 这个端点是为了让图表读到 CSV/JSON，不是通用文件读取接口。
///
/// `note` 是预览页正在渲染的笔记路径。笔记本身若来自一次「在浏览器中查看」的明确授权，
/// 就放行它引用的数据文件 —— 与软件内一致：软件内的 `read_file_content` 本来就对
/// 已打开笔记的引用不做目录限制，否则白板场景放在 `~/Downloads`、图表数据放在
/// `D:/datasets` 这类手册里明确示范的写法，在浏览器预览里会全部 403。
fn resolve_allowed_data_file(raw_path: &str, note: Option<&str>) -> Result<PathBuf, String> {
    const DATA_EXTS: [&str; 6] = ["csv", "tsv", "json", "md", "markdown", "excalidraw"];
    let decoded = decode(raw_path)
        .map_err(|_| "Invalid path encoding".to_string())?
        .into_owned();
    let decoded = decoded.trim_start_matches(r"\\?\");
    let file_canon = PathBuf::from(decoded)
        .canonicalize()
        .map_err(|_| "File not found".to_string())?;

    if !file_canon.is_file() {
        return Err("Not a file".to_string());
    }
    let ext = file_canon
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !DATA_EXTS.contains(&ext.as_str()) {
        return Err("Unsupported data file".to_string());
    }

    let mut allowed_roots: Vec<PathBuf> = Vec::new();
    for kb in get_knowledge_base_list().unwrap_or_default() {
        if let Ok(canon) = PathBuf::from(kb.location).canonicalize() {
            allowed_roots.push(canon);
        }
    }
    let resource_root = crate::commands::settingview::get_image_bed_root();
    if let Ok(canon) = PathBuf::from(resource_root.trim()).canonicalize() {
        allowed_roots.push(canon);
    }

    // 已显式放行预览的笔记本身，及其同目录下的数据文件。
    if is_remembered_preview_file(&file_canon) || is_sibling_of_remembered_preview(&file_canon) {
        return Ok(file_canon);
    }

    // 预览页报告的当前笔记必须**本身**是已授权预览的笔记；
    // 否则任何人构造一个 `note=` 参数就能读到任意路径。
    if let Some(note) = note {
        let note_canon = PathBuf::from(note.trim_start_matches(r"\\?\")).canonicalize();
        if let Ok(note_canon) = note_canon {
            if is_remembered_preview_file(&note_canon) && is_markdown_file(&note_canon) {
                return Ok(file_canon);
            }
        }
    }

    if allowed_roots.is_empty() {
        return Err("No allowed roots".to_string());
    }
    if allowed_roots.iter().any(|root| file_canon.starts_with(root)) {
        Ok(file_canon)
    } else {
        Err("Forbidden".to_string())
    }
}

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(v) => v,
            None => continue,
        };

        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_markdown_files(&path, out)?;
            continue;
        }

        if is_markdown_file(&path) {
            out.push(path);
        }
    }

    Ok(())
}

#[get("/favicon.ico")]
async fn favicon() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

#[get("/browser")]
async fn browser_index() -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", "/browser/knowledge-bases"))
        .finish()
}

#[get("/browser/knowledge-bases")]
async fn browser_knowledge_bases() -> HttpResponse {
    let result = web::block(|| get_knowledge_base_list()).await;

    let kbs = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let mut items = String::new();
    for kb in kbs {
        items.push_str(&format!(
            "<li><a href=\"/browser/knowledge-bases/{id}\">{name}</a><div class=\"sub\">{loc}</div></li>",
            id = kb.id,
            name = escape_html(&kb.knowledge_base_name),
            loc = escape_html(&kb.location)
        ));
    }

    let html = format!(
        r#"<!doctype html>
        <html lang="zh">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <title>Guaxi - 知识库</title>
          <style>
            body {{ font-family: -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; margin: 24px; color: #1f2328; }}
            h1 {{ font-size: 18px; margin: 0 0 14px; }}
            ul {{ list-style: none; padding: 0; margin: 0; }}
            li {{ padding: 10px 12px; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px; margin-bottom: 10px; }}
            a {{ color: #0969da; text-decoration: none; font-weight: 600; }}
            a:hover {{ text-decoration: underline; }}
            .sub {{ margin-top: 6px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
          </style>
        </head>
        <body>
          <h1>知识库列表</h1>
          <ul>{items}</ul>
        </body>
        </html>"#,
        items = items
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[get("/browser/knowledge-bases/{id}")]
async fn browser_knowledge_base_files(id: web::Path<i32>) -> HttpResponse {
    let kb_id = id.into_inner();

    let kbs = match web::block(get_knowledge_base_list).await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let Some(kb) = kbs.into_iter().find(|x| x.id == kb_id) else {
        return HttpResponse::NotFound().body("Knowledge base not found");
    };
    let kb_name = kb.knowledge_base_name.clone();

    let listing = web::block(move || {
        let (root, files) = kb_root_and_files(&kb.location)?;
        Ok::<_, String>(
            files
                .into_iter()
                .map(|file| {
                    let (rel, name) = kb_rel_and_name(&root, &file);
                    let abs = file.to_string_lossy().trim_start_matches(r"\\?\").to_string();
                    (abs, rel, name)
                })
                .collect::<Vec<_>>(),
        )
    })
    .await;

    let files = match listing {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::NotFound().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    info!(
        kb_id = kb_id,
        file_count = files.len(),
        "browser preview: knowledge base markdown files collected"
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(kb_list_page_html(kb_id, &kb_name, &files))
}

/// 知识库文件列表页的 HTML（首屏就带完整列表，检索在其上增量过滤）。
fn kb_list_page_html(kb_id: i32, kb_name: &str, files: &[(String, String, String)]) -> String {
    let mut items = String::with_capacity(files.len() * 220);
    for (abs, rel, name) in files {
        items.push_str(&kb_file_row(abs, rel, name, None, None));
    }
    let html = format!(
        r#"<!doctype html>
        <html lang="zh">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <title>Guaxi - {kb_name}</title>
          <style>
            :root {{ color-scheme: light; }}
            body {{ font-family: -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; margin: 0; color: #1f2328; }}
            .wrap {{ max-width: 1020px; margin: 0 auto; padding: 24px 20px 60px; }}
            h1 {{ font-size: 18px; margin: 0 0 12px; }}
            .back {{ display: inline-block; margin: 0 0 14px; font-size: 13px; }}
            .searchbar {{ position: sticky; top: 0; z-index: 5; display: flex; flex-wrap: wrap; gap: 8px; align-items: center;
              padding: 10px 0 12px; margin-bottom: 6px; background: rgba(255,255,255,0.94); backdrop-filter: blur(8px);
              border-bottom: 1px solid rgba(0,0,0,0.08); }}
            .searchbar input[type="search"] {{ flex: 1 1 240px; min-width: 0; padding: 8px 10px; font-size: 13px;
              border: 1px solid rgba(0,0,0,0.16); border-radius: 8px; outline: none; }}
            .searchbar input[type="search"]:focus {{ border-color: rgba(9,105,218,0.55); box-shadow: 0 0 0 3px rgba(9,105,218,0.12); }}
            .searchbar label {{ display: inline-flex; align-items: center; gap: 5px; font-size: 12px; color: rgba(31,35,40,0.72); white-space: nowrap; }}
            .status {{ font-size: 12px; color: rgba(31,35,40,0.6); margin: 8px 0 12px; min-height: 16px; }}
            ul {{ list-style: none; padding: 0; margin: 0; }}
            li {{ padding: 10px 12px; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px; margin-bottom: 10px; }}
            li.hidden {{ display: none; }}
            a {{ color: #0969da; text-decoration: none; font-weight: 600; }}
            a:hover {{ text-decoration: underline; }}
            .row {{ display: flex; align-items: center; justify-content: space-between; gap: 10px; }}
            .name {{ min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
            .dl {{ flex: 0 0 auto; font-weight: 700; padding: 2px 6px; border-radius: 8px; color: rgba(31,35,40,0.75); border: 1px solid rgba(0,0,0,0.08); }}
            .dl:hover {{ text-decoration: none; color: #0969da; border-color: rgba(9,105,218,0.35); background: rgba(9,105,218,0.06); }}
            .sub {{ margin-top: 6px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
            .hits {{ margin-top: 8px; display: flex; flex-direction: column; gap: 4px; }}
            .hit {{ font-size: 12px; line-height: 1.5; padding: 5px 8px; border-radius: 6px; background: rgba(9,105,218,0.05);
              color: rgba(31,35,40,0.8); text-decoration: none; font-weight: 400; word-break: break-all; }}
            .hit:hover {{ background: rgba(9,105,218,0.1); text-decoration: none; }}
            .hit .ln {{ color: rgba(31,35,40,0.45); margin-right: 6px; font-variant-numeric: tabular-nums; }}
            mark {{ background: #fff3b0; color: inherit; padding: 0 1px; border-radius: 2px; }}
            .empty {{ display: none; padding: 24px 8px; text-align: center; font-size: 13px; color: rgba(31,35,40,0.5); }}
            .empty.show {{ display: block; }}
          </style>
        </head>
        <body>
          <div class="wrap">
            <a class="back" href="/browser/knowledge-bases">← 返回知识库列表</a>
            <h1>{kb_name}</h1>
            <div class="searchbar">
              <input id="q" type="search" placeholder="按文件名或路径筛选，回车直接搜正文" autocomplete="off" spellcheck="false" />
              <label><input id="content-mode" type="checkbox" /> 搜正文</label>
              <label id="content-hint" style="display:none;color:rgba(31,35,40,0.5)">正文检索按行命中，最多每篇 5 行</label>
            </div>
            <div class="status" id="status"></div>
            <ul id="list">{items}</ul>
            <div class="empty" id="empty">没有匹配的文件</div>
          </div>
          <script>
            const KB_ID = {kb_id};
            const listEl = document.getElementById('list');
            const qEl = document.getElementById('q');
            const contentModeEl = document.getElementById('content-mode');
            const contentHintEl = document.getElementById('content-hint');
            const statusEl = document.getElementById('status');
            const emptyEl = document.getElementById('empty');
            // 首屏行（含锚点数据）缓存下来：正文模式退出时直接还原，不必重新请求。
            const rows = new Map();
            for (const li of listEl.querySelectorAll('li')) {{
              rows.set(li.dataset.path, {{
                el: li,
                name: (li.dataset.name || '').toLowerCase(),
                rel: (li.dataset.rel || '').toLowerCase(),
              }});
            }}

            const escapeHtml = (s) => String(s)
              .replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');

            function setStatus(text) {{ statusEl.textContent = text || ''; }}

            function markText(text, needle) {{
              const safe = escapeHtml(text);
              if (!needle) return safe;
              const idx = safe.toLowerCase().indexOf(needle.toLowerCase());
              if (idx < 0) return safe;
              return safe.slice(0, idx) + '<mark>' + safe.slice(idx, idx + needle.length) + '</mark>'
                + safe.slice(idx + needle.length);
            }}

            /** 本地按文件名/路径过滤：几十毫秒内完成，不必打后端。 */
            function filterByName(needle) {{
              const q = String(needle || '').trim().toLowerCase();
              let shown = 0;
              for (const row of rows.values()) {{
                const hit = !q || row.name.includes(q) || row.rel.includes(q);
                row.el.classList.toggle('hidden', !hit);
                const nameLink = row.el.querySelector('.name');
                if (nameLink) {{
                  const raw = row.el.dataset.name || '';
                  nameLink.innerHTML = markText(raw, q);
                }}
                if (hit) shown += 1;
              }}
              emptyEl.classList.toggle('show', shown === 0);
              setStatus(q ? ('匹配 ' + shown + ' / ' + rows.size + ' 个文件') : (rows.size + ' 个文件'));
            }}

            /** 正文检索：走后端端点，结果按文件分组渲染在对应行下面。 */
            async function searchContent(needle) {{
              const q = String(needle || '').trim();
              if (!q) {{ restoreRows(); filterByName(qEl.value); return; }}
              setStatus('正在检索正文…');
              try {{
                const resp = await fetch('/browser/knowledge-bases/' + KB_ID + '/search?content='
                  + encodeURIComponent(q) + '&q=' + encodeURIComponent(qEl.value.trim()));
                if (!resp.ok) throw new Error('HTTP ' + resp.status + ' ' + (await resp.text()).slice(0, 80));
                const data = await resp.json();
                renderHits(data.hits || [], q);
                const fileCount = new Set((data.hits || []).map((h) => h.path)).size;
                setStatus('正文命中 ' + (data.total || 0) + ' 行，分布在 ' + fileCount + ' 个文件');
              }} catch (error) {{
                setStatus('正文检索失败：' + ((error && error.message) || error));
              }}
            }}

            function renderHits(hits, needle) {{
              restoreRows();
              const byPath = new Map();
              for (const hit of hits) {{
                if (!byPath.has(hit.path)) byPath.set(hit.path, []);
                byPath.get(hit.path).push(hit);
              }}
              for (const row of rows.values()) row.el.classList.add('hidden');
              for (const [path, list] of byPath) {{
                const row = rows.get(path);
                if (!row) continue;
                row.el.classList.remove('hidden');
                const nameLink = row.el.querySelector('.name');
                if (nameLink) nameLink.innerHTML = markText(row.el.dataset.name || '', needle);
                const box = document.createElement('div');
                box.className = 'hits';
                for (const hit of list) {{
                  const a = document.createElement('a');
                  a.className = 'hit';
                  a.href = '/browser/preview?path=' + encodeURIComponent(path);
                  a.innerHTML = '<span class="ln">' + (hit.line_number || '') + '</span>'
                    + markText(hit.line || '', needle);
                  box.appendChild(a);
                }}
                row.el.appendChild(box);
              }}
              emptyEl.classList.toggle('show', byPath.size === 0);
            }}

            /** 清掉正文检索挂上去的命中块，回到初始行结构。 */
            function restoreRows() {{
              for (const row of rows.values()) {{
                row.el.querySelectorAll('.hits').forEach((el) => el.remove());
                row.el.classList.remove('hidden');
                const nameLink = row.el.querySelector('.name');
                if (nameLink) nameLink.textContent = row.el.dataset.name || '';
              }}
              emptyEl.classList.remove('show');
            }}

            let timer = 0;
            qEl.addEventListener('input', () => {{
              clearTimeout(timer);
              const value = qEl.value;
              timer = setTimeout(() => {{
                if (contentModeEl.checked) waitAndSearch(value);
                else {{ restoreRows(); filterByName(value); }}
              }}, 120);
            }});
            qEl.addEventListener('keydown', (event) => {{
              if (event.key !== 'Enter') return;
              event.preventDefault();
              if (!contentModeEl.checked) {{
                contentModeEl.checked = true;
                contentHintEl.style.display = '';
              }}
              searchContent(qEl.value);
            }});
            contentModeEl.addEventListener('change', () => {{
              contentHintEl.style.display = contentModeEl.checked ? '' : 'none';
              if (contentModeEl.checked) searchContent(qEl.value);
              else {{ restoreRows(); filterByName(qEl.value); }}
            }});

            function waitAndSearch(value) {{
              clearTimeout(timer);
              timer = setTimeout(() => searchContent(value), 320);
            }}

            filterByName('');
            qEl.focus();
          </script>
        </body>
        </html>"#,
        kb_name = escape_html(kb_name),
        kb_id = kb_id,
        items = items
    );

    html
}

/// 知识库文件列表的一行。`line` / `line_number` 给出时附带一条命中预览。
fn kb_file_row(abs: &str, rel: &str, name: &str, line: Option<&str>, line_number: Option<u64>) -> String {
    let href_preview = format!("/browser/preview?path={}", encode(abs));
    let href_download = format!("/browser/download?path={}", encode(abs));
    let hits = match (line, line_number) {
        (Some(text), Some(no)) => format!(
            "<div class=\"hits\"><a class=\"hit\" href=\"{href_preview}\"><span class=\"ln\">{no}</span>{text}</a></div>",
            href_preview = href_preview,
            no = no,
            text = escape_html(text)
        ),
        _ => String::new(),
    };
    format!(
        "<li data-path=\"{abs}\" data-name=\"{name}\" data-rel=\"{rel}\"><div class=\"row\"><a class=\"name\" href=\"{href_preview}\">{name}</a><a class=\"dl\" href=\"{href_download}\" title=\"下载\" aria-label=\"下载\">⭳</a></div><div class=\"sub\">{rel}</div>{hits}</li>",
        abs = escape_html(abs),
        name = escape_html(name),
        rel = escape_html(rel),
        href_preview = href_preview,
        href_download = href_download,
        hits = hits
    )
}

#[derive(Deserialize)]
struct KbFileQuery {
    /// 文件名模糊检索词。空 = 不筛选（但仍受 `q` 的内容筛选约束）。
    #[serde(default)]
    q: Option<String>,
    /// 正文检索词。非空时按行搜内容，结果里带命中行与行号。
    #[serde(default)]
    content: Option<String>,
}

/// 知识库文件列表的检索结果（`/browser/knowledge-bases/{id}/search` 的响应）。
#[derive(serde::Serialize)]
struct KbSearchHit {
    path: String,
    name: String,
    rel: String,
    /// 命中行文本（正文检索时才有）。
    line: Option<String>,
    line_number: Option<u64>,
}

#[derive(serde::Serialize)]
struct KbSearchResponse {
    total: usize,
    hits: Vec<KbSearchHit>,
}

/// 解析知识库根目录并收集其中的 Markdown 文件（阻塞版本，供 `web::block` 调用）。
fn kb_root_and_files(kb_location: &str) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let root = PathBuf::from(kb_location)
        .canonicalize()
        .map_err(|_| "Knowledge base path not found".to_string())?;
    let mut out = Vec::new();
    collect_markdown_files(&root, &mut out).map_err(|e| e.to_string())?;
    Ok((root, out))
}

fn kb_rel_and_name(root: &Path, file: &Path) -> (String, String) {
    let rel = file
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file.to_string_lossy().replace('\\', "/"));
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    (rel, name)
}

/// 文件名检索：按相对路径做不区分大小写的子串匹配。
///
/// 刻意不用模糊匹配：知识库动辄几百上千篇，模糊排序会把「看起来像」的结果
/// 混在前面，用户找的是「我记得名字里有这几个字」，子串语义更可预期。
fn kb_filter_by_name(files: Vec<PathBuf>, root: &Path, needle: &str) -> Vec<(PathBuf, String, String)> {
    let needle = needle.trim().to_lowercase();
    files
        .into_iter()
        .filter_map(|file| {
            let (rel, name) = kb_rel_and_name(root, &file);
            if needle.is_empty() || rel.to_lowercase().contains(&needle) {
                Some((file, rel, name))
            } else {
                None
            }
        })
        .collect()
}

/// 正文检索：复用软件内全文搜索的同一套 matcher（字面量、智能大小写、全词分词）。
///
/// 与软件内搜索面板同一套语义，用户不必在两处记两套规则。
fn kb_search_content(
    files: Vec<(PathBuf, String, String)>,
    pattern: &str,
) -> Result<Vec<KbSearchHit>, String> {
    use crate::commands::searcher::{build_matcher, grep_file_matches, MatchOptions};

    let matcher = build_matcher(pattern.trim(), &MatchOptions::default())?;
    let mut hits = Vec::new();

    for (file, rel, name) in files {
        let mtime = crate::commands::searcher::file_mtime_ms(&file);
        // 每个文件最多取 5 行，避免一篇长文档把整个结果列表刷满。
        let Some((_, file_hits)) = grep_file_matches(&matcher, &file, &rel, mtime, Some(5)) else {
            continue;
        };
        for hit in file_hits {
            hits.push(KbSearchHit {
                path: file.to_string_lossy().trim_start_matches(r"\\?\").to_string(),
                name: name.clone(),
                rel: rel.clone(),
                line: hit.preview.map(|p| p.trim_end().to_string()),
                line_number: hit.line_number,
            });
        }
    }

    Ok(hits)
}

/// 知识库文件检索的 JSON 接口：给列表页的搜索框用。
///
/// 做成独立端点而不是「重渲染整页」：整页接口每次都要重新遍历目录，用户在搜索框里
/// 每敲一个字就重排一次滚动位置，体验很差。这里只返回结构化结果，由页面增量渲染。
#[get("/browser/knowledge-bases/{id}/search")]
async fn browser_kb_search(id: web::Path<i32>, query: web::Query<KbFileQuery>) -> HttpResponse {
    let kb_id = id.into_inner();
    let kbs = match web::block(get_knowledge_base_list).await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let Some(kb) = kbs.into_iter().find(|x| x.id == kb_id) else {
        return HttpResponse::NotFound().body("Knowledge base not found");
    };

    let name_query = query.q.clone().unwrap_or_default();
    let content_query = query.content.clone().unwrap_or_default();

    let result = web::block(move || -> Result<KbSearchResponse, String> {
        let (root, files) = kb_root_and_files(&kb.location)?;
        let candidates = kb_filter_by_name(files, &root, &name_query);
        if content_query.trim().is_empty() {
            let hits: Vec<KbSearchHit> = candidates
                .into_iter()
                .map(|(file, rel, name)| KbSearchHit {
                    path: file.to_string_lossy().trim_start_matches(r"\\?\").to_string(),
                    name,
                    rel,
                    line: None,
                    line_number: None,
                })
                .collect();
            Ok(KbSearchResponse {
                total: hits.len(),
                hits,
            })
        } else {
            let hits = kb_search_content(candidates, &content_query)?;
            Ok(KbSearchResponse {
                total: hits.len(),
                hits,
            })
        }
    })
    .await;

    match result {
        Ok(Ok(payload)) => HttpResponse::Ok()
            .content_type("application/json; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .json(payload),
        Ok(Err(msg)) => HttpResponse::BadRequest().body(msg),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[derive(Deserialize)]
struct PathQuery {
    path: String,
}

/// `/browser/file` 的查询参数：额外带上当前笔记，用于放行它引用的数据文件。
#[derive(Deserialize)]
struct DataFileQuery {
    path: String,
    #[serde(default)]
    note: Option<String>,
}

#[get("/browser/raw")]
async fn browser_raw(query: web::Query<PathQuery>) -> HttpResponse {
    info!(path = query.path.as_str(), "browser preview: /browser/raw");

    let file_path = match resolve_allowed_markdown_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            warn!(
                path = query.path.as_str(),
                error = msg.as_str(),
                "browser preview: /browser/raw resolve failed"
            );
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            };
        }
    };

    info!(
        file = file_path.to_string_lossy().as_ref(),
        "browser preview: /browser/raw resolved"
    );

    match web::block(move || fs::read_to_string(file_path)).await {
        Ok(Ok(content)) => {
            let processed = preprocess_markdown_for_static_render(&content);
            info!(
                original_len = content.len(),
                processed_len = processed.len(),
                "browser preview: preprocessed markdown for static preview"
            );
            HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(processed)
        }
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/raw read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/raw worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// 预览页读取**数据文件**（图表数据 / 白板场景）用。
///
/// 与 /browser/raw 的区别只有两点：不限定 .md 扩展名，且不做 Markdown 预处理 ——
/// 图表要的是原始 CSV/JSON 文本。允许的根目录与 markdown 预览完全一致
/// （知识库根 + 资源库根），不会因为多开一个端点而放宽可见范围。
#[get("/browser/file")]
async fn browser_file(query: web::Query<DataFileQuery>) -> HttpResponse {
    let file_path = match resolve_allowed_data_file(&query.path, query.note.as_deref()) {
        Ok(v) => v,
        Err(msg) => {
            warn!(
                path = query.path.as_str(),
                error = msg.as_str(),
                "browser preview: /browser/file resolve failed"
            );
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            };
        }
    };

    match web::block(move || fs::read_to_string(file_path)).await {
        Ok(Ok(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(content),
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/file read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/file worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// 预览页按**资源库相对路径**读取数据文件。
///
/// 图表的 `resource:` 引用在软件内有后端命令解析；静态预览页没有 Tauri IPC，
/// 只能用 HTTP，于是这里提供等价的一条路。相对路径里的 `..` 直接拒绝。
#[get("/browser/resource")]
async fn browser_resource(query: web::Query<PathQuery>) -> HttpResponse {
    let rel = query.path.trim().replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|seg| seg == "..") {
        return HttpResponse::BadRequest().body("Invalid resource path");
    }

    let root = crate::commands::settingview::get_image_bed_root();
    let root = root.trim();
    if root.is_empty() {
        return HttpResponse::BadRequest().body("Resource root not configured");
    }
    let abs = PathBuf::from(format!(
        "{}/{}",
        root.trim_end_matches(['/', '\\']),
        rel
    ));
    // 只有落在资源库根目录内的文件才允许读。
    let (Ok(canon), Ok(root_canon)) = (abs.canonicalize(), Path::new(root).canonicalize()) else {
        return HttpResponse::NotFound().body("File not found");
    };
    if !canon.starts_with(&root_canon) {
        return HttpResponse::Forbidden().body("Forbidden");
    }

    match web::block(move || fs::read_to_string(canon)).await {
        Ok(Ok(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(content),
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/resource read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/resource worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/browser/download")]
async fn browser_download(query: web::Query<PathQuery>) -> HttpResponse {
    info!(
        path = query.path.as_str(),
        "browser preview: /browser/download"
    );

    let file_path = match resolve_allowed_markdown_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            warn!(
                path = query.path.as_str(),
                error = msg.as_str(),
                "browser preview: /browser/download resolve failed"
            );
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            };
        }
    };

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download.md")
        .to_string();

    let filename_fallback = filename_ascii_fallback(&filename);
    let filename_utf8 = encode(&filename);

    let content_disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        filename_fallback, filename_utf8
    );

    match web::block(move || fs::read(file_path)).await {
        Ok(Ok(bytes)) => HttpResponse::Ok()
            .content_type("application/octet-stream")
            .append_header(("Cache-Control", "no-store"))
            .append_header(("Content-Disposition", content_disposition))
            .body(bytes),
        Ok(Err(e)) => {
            error!(error = %e, "browser preview: /browser/download read failed");
            HttpResponse::InternalServerError().finish()
        }
        Err(e) => {
            error!(error = %e, "browser preview: /browser/download worker failed");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/browser/preview")]
async fn browser_preview(query: web::Query<PathQuery>) -> HttpResponse {
    info!(
        path = query.path.as_str(),
        "browser preview: /browser/preview"
    );

    let file_path = match resolve_allowed_markdown_file(&query.path) {
        Ok(v) => v,
        Err(msg) => {
            return match msg.as_str() {
                "Forbidden" => HttpResponse::Forbidden().body(msg),
                "File not found" => HttpResponse::NotFound().body(msg),
                _ => HttpResponse::BadRequest().body(msg),
            }
        }
    };

    let title = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Guaxi Preview");

    let base_dir = file_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy();
    let base_dir = base_dir.trim_start_matches(r"\\?\").to_string();

    let abs = file_path.to_string_lossy();
    let abs = abs.trim_start_matches(r"\\?\");
    let encoded_path = encode(abs).to_string();

    let html = format!(
        r#"<!doctype html>
        <html lang="zh">
        <head>
          <meta charset="utf-8" />
          <meta name="viewport" content="width=device-width, initial-scale=1" />
          <title>{title}</title>
          <style>
            body {{ font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace; BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; line-height: 1.5; word-wrap: break-word; margin: 0; color: #1f2328; }}
            .topbar {{ position: sticky; top: 0; background: rgba(255,255,255,0.92); backdrop-filter: blur(10px); border-bottom: 1px solid rgba(0,0,0,0.08); padding: 10px 14px; }}
            .titlebar {{ display: flex; align-items: center; justify-content: space-between; gap: 10px; }}
            .title {{ font-weight: 600; font-size: 13px; word-break: break-all; }}
            .dl {{ flex: 0 0 auto; font-weight: 700; padding: 2px 6px; border-radius: 8px; color: rgba(31,35,40,0.75); border: 1px solid rgba(0,0,0,0.08); text-decoration: none; }}
            .dl:hover {{ color: #0969da; border-color: rgba(9,105,218,0.35); background: rgba(9,105,218,0.06); }}
            .sub {{ margin-top: 4px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
            .content {{ max-width: 980px; margin: 0 auto; padding: 18px 16px 60px; }}
            pre {{ background: #ffffff; border: 1px solid rgba(0,0,0,0.08); padding: 12px; border-radius: 10px; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(0,0,0,0.22) transparent; }}
            pre::-webkit-scrollbar {{ width: 6px; height: 6px; }}
            pre::-webkit-scrollbar-thumb {{ background-color: rgba(0,0,0,0.22); border-radius: 999px; }}
            pre::-webkit-scrollbar-thumb:hover {{ background-color: rgba(0,0,0,0.34); }}
            pre::-webkit-scrollbar-track {{ background-color: transparent; }}
            pre code.hljs, code.hljs, .hljs {{ background: transparent !important; }}
            code {{ font-family: ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,'Liberation Mono','Courier New',monospace; }}
            img {{ max-width: 100%; }}
            .guaxi-chart {{ width: 100%; min-height: 240px; margin: 14px 0; }}
            .guaxi-excalidraw {{
              margin: 14px 0;
              padding: 10px;
              border: 1px solid rgba(0,0,0,0.12);
              border-radius: 8px;
              background: #fff;
              text-align: center;
              overflow: auto;
            }}
            .guaxi-excalidraw svg {{ display: block; max-width: 100%; height: auto; margin: 0 auto; }}
            .guaxi-excalidraw-note {{ margin: 12px 0; padding: 10px 12px; border: 1px dashed rgba(0,0,0,0.16); border-radius: 8px; font-size: 13px; color: rgba(31,35,40,0.7); }}
            a {{ color: #0969da; }}
            .content a[id^="guaxi-toc-"] {{ display: block; position: relative; top: -72px; visibility: hidden; pointer-events: none; }}
            .guaxi-media-group {{
              --guaxi-media-gap: clamp(14px, 2vw, 24px);
              display: flex !important;
              flex-wrap: wrap;
              justify-content: center !important;
              align-items: stretch;
              align-content: flex-start;
              gap: var(--guaxi-media-gap);
              width: 100%;
              max-width: 1000px;
              margin: 22px auto;
              padding: 6px 0;
            }}
            .guaxi-media-group.is-single {{
              display: block !important;
              width: min(100%, 760px);
              gap: 0;
            }}
            .guaxi-media-group.is-gallery {{
              display: grid !important;
              grid-template-columns: repeat(auto-fit, minmax(220px, 320px));
              justify-content: center !important;
              justify-items: center;
              align-items: start;
              padding-left: 6px;
              padding-right: 6px;
            }}
            .guaxi-media-group__item {{
              display: flex;
              align-items: center;
              justify-content: center;
              min-width: 0;
              width: fit-content;
              max-width: 100%;
              box-sizing: border-box;
              padding: 0;
              border: none;
              background: transparent;
              box-shadow: none;
              overflow: visible;
              text-decoration: none;
              transition: opacity 0.18s ease;
            }}
            .guaxi-media-group__item:hover {{ opacity: 0.98; }}
            .guaxi-media-group.is-single > .guaxi-media-group__item {{
              display: flex;
              flex: 0 1 auto;
              min-width: 0;
              width: fit-content;
              max-width: min(100%, 760px);
              margin-left: auto;
              margin-right: auto;
            }}
            .guaxi-media-group__image {{
              display: block;
              width: 100%;
              max-width: 100%;
              max-height: min(56vh, 420px);
              height: auto;
              margin: 0;
              border-radius: 16px;
              object-fit: contain;
            }}
            .guaxi-media-group.is-single .guaxi-media-group__image {{
              width: auto;
              max-width: min(100%, 760px);
              max-height: min(72vh, 640px);
            }}
            table {{ border-collapse: collapse; width: 100%; margin: 12px 0; }}
            th, td {{ border: 1px solid rgba(0,0,0,0.16); padding: 6px 10px; }}
            thead th {{ background: rgba(0,0,0,0.04); }}
            @media (max-width: 760px) {{
              .guaxi-media-group,
              .guaxi-media-group.is-gallery {{
                width: 100%;
                max-width: 100%;
              }}
              .guaxi-media-group__item {{
                max-width: 100%;
                width: 100%;
              }}
            }}
          </style>
          <link rel="stylesheet" href="/browser/assets/highlightjs/styles/atom-one-light.css?v={assetVersion}" />
          <script>
            window.MathJax = {{
              tex: {{
                inlineMath: [['$', '$'], ['\\(', '\\)']],
                displayMath: [['$$', '$$'], ['\\[', '\\]']],
                processEscapes: true,
                tags: 'ams'
                }},
              svg: {{
                fontCache: 'global'
              }}
            }};
          </script>
          <script src="/browser/assets/markdown-it/markdown-it.min.js?v={assetVersion}"></script>
          <script src="/browser/assets/highlightjs/highlight.min.js?v={assetVersion}"></script>
          <script>window.process={{env:{{NODE_ENV:'production'}},platform:'browser',versions:{{}}}};window.global=window;</script>
          <!-- mermaid / MathJax / ECharts / Excalidraw（合计约 14MB）按文档内容按需注入，见 ensurePreviewAssets。 -->
          </head>
        <body>
          <div class="topbar">
            <div class="titlebar">
              <div class="title">{title}</div>
              <a class="dl" href="/browser/download?path={encodedPath}" title="下载" aria-label="下载">⭳</a>
            </div>
            <div class="sub">{path}</div>
          </div>
          <div class="content" id="content">加载中…</div>

          <script>
            const contentEl = document.getElementById('content');
            const filePathEncoded = {filePathJson};
            // 原始绝对路径：使用时统一 encodeURIComponent 一次，避免二次编码。
            const notePath = {notePathJson};
            const baseDir = {baseDirJson};

            // ── 重型渲染资源按需加载 ─────────────────────────────────────────────
            //
            // 以前这些依赖全写在 <head> 里同步加载：一篇纯文字的笔记也要下载并执行
            // 约 14MB（mermaid 2.7MB、MathJax 2.1MB、ECharts 1.1MB、Excalidraw 7.8MB）。
            // 现在正文渲染完先看实际用到了什么，再按需注入对应脚本。
            const ASSET_VERSION = {assetVersionJson};
            const loadedAssets = new Map();

            function assetUrl(path) {{
              return path + (path.indexOf('?') >= 0 ? '&' : '?') + 'v=' + ASSET_VERSION;
            }}

            function loadAsset(path) {{
              const url = assetUrl(path);
              if (!loadedAssets.has(url)) {{
                const task = new Promise((resolve, reject) => {{
                  const script = document.createElement('script');
                  script.src = url;
                  script.onload = () => resolve();
                  script.onerror = () => reject(new Error('资源加载失败：' + url));
                  document.head.appendChild(script);
                }});
                // 失败的任务不留在缓存里，下一轮自动刷新还能重试。
                task.catch(() => loadedAssets.delete(url));
                loadedAssets.set(url, task);
              }}
              return loadedAssets.get(url);
            }}

            // 按「这次实际渲染出了什么」判断要加载哪些资源：
            // 图表 / 白板直接看围栏收集结果，mermaid 看渲染后的代码块，
            // 比扫原文更准（列表、引用里的围栏也算数）。
            function detectPreviewAssets(root, chartDrafts, excalidrawDrafts, source) {{
              const text = String(source || '');
              return {{
                chart: chartDrafts.length > 0,
                excalidraw: excalidrawDrafts.length > 0,
                mermaid: !!root.querySelector('pre code.language-mermaid, pre code.mermaid'),
                math: /\$\$[\s\S]+?\$\$/.test(text)
                  || /\\\(|\\\[/.test(text)
                  || /(^|[^\\$])\$[^$\n]+?\$/.test(text),
              }};
            }}

            async function ensurePreviewAssets(need) {{
              if (need.chart) {{
                // liquidfill / wordcloud 是 ECharts 的副作用扩展，加载时必须已经存在 window.echarts。
                await loadAsset('/browser/assets/echarts/echarts.min.js');
                await Promise.all([
                  loadAsset('/browser/assets/echarts/echarts-liquidfill.min.js'),
                  loadAsset('/browser/assets/echarts/echarts-wordcloud.min.js'),
                  loadAsset('/browser/assets/chart-preview/guaxi-chart-preview.js'),
                ]);
              }}
              const rest = [];
              if (need.excalidraw) {{
                // 白板场景的读取也走 loadChartData，它依赖共享图表模块（仅 32KB）。
                rest.push(loadAsset('/browser/assets/chart-preview/guaxi-chart-preview.js'));
                rest.push(loadAsset('/browser/assets/excalidraw/excalidraw-export.js'));
              }}
              if (need.mermaid) rest.push(loadAsset('/browser/assets/mermaid/mermaid.min.js'));
              if (need.math) rest.push(loadAsset('/browser/assets/mathjax/tex-svg.js'));
              await Promise.all(rest);
            }}

            function escapeHtml(s) {{
              return String(s)
                .replaceAll('&', '&amp;')
                .replaceAll('<', '&lt;')
                .replaceAll('>', '&gt;');
            }}

            function enhanceMediaGroups(root) {{
              if (!(root instanceof Element) || !root.querySelector('img')) {{
                return;
              }}

              const blocks = Array.from(root.querySelectorAll('p'));
              const isWhitespaceNode = (node) => {{
                if (!node) return true;
                if (node.nodeType === Node.TEXT_NODE) return !String(node.textContent || '').trim();
                if (node.nodeType === Node.ELEMENT_NODE && node instanceof HTMLBRElement) return true;
                return false;
              }};
              const resolveMediaItem = (node) => {{
                if (!(node instanceof Element)) return null;
                if (node instanceof HTMLImageElement) {{
                  return {{ kind: 'image', host: node, image: node }};
                }}
                if (node instanceof HTMLAnchorElement) {{
                  const childElements = Array.from(node.children || []);
                  if (childElements.length !== 1 || !(childElements[0] instanceof HTMLImageElement)) return null;
                  const hasNonMediaChild = Array.from(node.childNodes || []).some((child) => child !== childElements[0] && !isWhitespaceNode(child));
                  if (hasNonMediaChild) return null;
                  return {{ kind: 'linked-image', host: node, image: childElements[0] }};
                }}
                return null;
              }};

              blocks.forEach((block) => {{
                const meaningfulNodes = Array.from(block.childNodes || []).filter((node) => !isWhitespaceNode(node));
                if (!meaningfulNodes.length) return;
                const mediaItems = meaningfulNodes.map(resolveMediaItem);
                if (mediaItems.some((item) => !item)) return;

                block.classList.add('guaxi-media-group');
                block.classList.toggle('is-single', mediaItems.length === 1);
                block.classList.toggle('is-gallery', mediaItems.length > 1);

                mediaItems.forEach((item) => {{
                  if (item.host === item.image) {{
                    const wrapper = document.createElement('span');
                    wrapper.setAttribute('data-guaxi-media-item', item.kind);
                    item.image.replaceWith(wrapper);
                    wrapper.appendChild(item.image);
                    item.host = wrapper;
                  }} else {{
                    item.host.setAttribute('data-guaxi-media-item', item.kind);
                  }}
                  item.host.classList.add('guaxi-media-group__item');
                  item.image.classList.add('guaxi-media-group__image');
                }});
              }});
            }}

            /**
             * 图表主题：固定浅色。
             *
             * 静态预览页是一张白底网页（body / .content 都是白底），图表必须跟它一致。
             * 早先这里传 null（ECharts 默认浅色）本身是对的；真正的问题是软件内：
             * 深色主题下实时图表用了 dark 主题，导出时又被强制改成浅色，
             * 导致「预览里能看见、导出里看不见」。两边统一成浅色后，所见即所得。
             */
            function guaxiPreviewChartTheme() {{
              return null;
            }}

            /**
             * 等词云的异步布局落笔。
             *
             * 词云（echarts-wordcloud）在离屏 canvas 上逐轮摆放文字，`setOption` 返回时
             * 画面还是空的；此时若被截图或打印，结果就是一张空图。
             *
             * 其余图表由 ECharts 同步绘制，无需等待。早先这里按 SVG 子元素数量判断
             * 「画完了」，但 zrender 的 SVG 输出固定是「背景 rect + <g id="main">」
             * （只有渐变/裁剪才多一个 <defs>），阈值永远不会满足，
             * 于是非词云图表每张都空等满 3 秒超时。
             */
            async function waitForChartPaint(chart, option, timeoutMs = 3000) {{
              const series = Array.isArray(option && option.series) ? option.series : [];
              const isWordCloud = series.some((s) => /^wordcloud$/i.test(String((s && s.type) || '')));
              if (!isWordCloud) return true;
              const deadline = Date.now() + timeoutMs;
              const painted = () => {{
                try {{
                  chart.getZr().refreshImmediately();
                  const svg = chart.getDom() && chart.getDom().querySelector('svg');
                  return !!svg && !!svg.querySelector('text');
                }} catch (_) {{
                  return true;
                }}
              }};
              while (Date.now() < deadline) {{
                if (painted()) return true;
                await new Promise((resolve) => setTimeout(resolve, 50));
              }}
              return painted();
            }}

            /**
             * 渲染 excalidraw 白板：读场景 JSON → 交给官方 exportToSvg → 内联 SVG。
             *
             * 之前静态页只显示一行「请在软件内查看」的提示，内容等于丢失。
             * 现在把官方导出器随页面分发，场景文件走与图表数据相同的读取端点，
             * 因此相对路径 / 绝对路径 / resource: 三种写法行为一致。
             */
            async function renderExcalidrawDrafts(root, drafts, baseDirHint) {{
              if (!drafts || !drafts.length) return;
              const exporter = window.GuaxiExcalidrawExport;
              for (const draft of drafts) {{
                const el = root.querySelector('#' + draft.id);
                if (!el) continue;
                if (!exporter || typeof exporter.exportToSvg !== 'function') {{
                  el.innerHTML = '<div class="guaxi-excalidraw-note">白板导出器未加载，无法渲染：'
                    + escapeHtml(draft.source) + '</div>';
                  continue;
                }}
                try {{
                  const text = await loadChartData(draft.source, baseDirHint);
                  const scene = JSON.parse(text);
                  const elements = Array.isArray(scene.elements) ? scene.elements : [];
                  const appState = scene.appState && typeof scene.appState === 'object' ? scene.appState : {{}};
                  if (!elements.length) throw new Error('场景里没有图形元素');
                  const svg = await exporter.exportToSvg({{
                    elements: elements,
                    appState: Object.assign({{}}, appState, {{
                      exportBackground: true,
                      viewBackgroundColor: appState.viewBackgroundColor || '#ffffff',
                    }}),
                    files: scene.files || {{}},
                  }});
                  el.innerHTML = '';
                  el.appendChild(svg);
                }} catch (error) {{
                  el.innerHTML = '<div class="guaxi-excalidraw-note">白板渲染失败：'
                    + escapeHtml((error && error.message) || String(error)) + '</div>';
                }}
              }}
            }}

            // 图表实例统一登记：重渲染前逐个 dispose，resize 只挂一个全局监听。
            // 之前每张图各自 addEventListener('resize')，自动刷新一次就多一批监听，
            // 旧实例也从不销毁，预览页开久了会越来越卡。
            const previewCharts = [];

            function disposePreviewCharts() {{
              while (previewCharts.length) {{
                try {{ previewCharts.pop().dispose(); }} catch (_) {{}}
              }}
            }}

            window.addEventListener('resize', () => {{
              previewCharts.forEach((chart) => {{
                try {{ chart.resize(); }} catch (_) {{}}
              }});
            }}, {{ passive: true }});

            async function renderChartDrafts(root, drafts, baseDirHint) {{
              if (!drafts || !drafts.length) return;
              if (!window.echarts) {{
                drafts.forEach((draft) => {{
                  const el = root.querySelector('#' + draft.id);
                  if (el) el.textContent = '图表渲染需要 ECharts，但资源未加载';
                }});
                return;
              }}

              for (const draft of drafts) {{
                const el = root.querySelector('#' + draft.id);
                if (!el) continue;
                try {{
                  const built = await buildChartPreviewOption(draft.source, baseDirHint);
                  // 数据为空只在「围栏声明了 data」时算错误。手册允许直接贴整段
                  // ECharts option，那种围栏本来就没有 data，数据全在 option 里。
                  if (built.expectsData && !built.records.length) throw new Error('数据为空');
                  el.style.height = '360px';
                  const chart = window.echarts.init(el, guaxiPreviewChartTheme(), {{ renderer: 'svg' }});
                  previewCharts.push(chart);
                  chart.setOption(built.option, true);
                  // 只有词云是异步布局，其余类型这里立刻返回，不再白白等满超时。
                  await waitForChartPaint(chart, built.option);
                }} catch (error) {{
                  el.textContent = '图表渲染失败：' + ((error && error.message) || error);
                }}
              }}
            }}

            // ── 图表：解析与 option 组装复用软件内同一份模块 ──────────────────────
            //
            // 这些函数以前在这里各写了一份「精简版」，结果是两边逐渐漂移：
            // 软件内已登记的树图 / 旭日 / 关系 / 热力 / K线 / 甘特在软件里能画，
            // 浏览器预览却报「静态预览暂不支持该图表类型」；连「直接贴完整 ECharts
            // option」这种手册明确承诺的写法也会被判成非法 spec。
            //
            // 现在解析与组装全部交给 /browser/assets/chart-preview/guaxi-chart-preview.js
            // （由 tools/build_chart_preview_bundle.mjs 从 guaxiChartData / guaxiChartTypes
            // 打包而来）。这里只保留静态页特有的两件事：取数据、交给 ECharts。

            /** 取不到共享模块时给出可读原因，而不是让每张图各自报一句空话。 */
            function chartPreviewApi() {{
              const api = window.GuaxiChartPreview;
              if (!api || typeof api.buildChartOption !== 'function') {{
                throw new Error('图表模块未加载（/browser/assets/chart-preview/guaxi-chart-preview.js）');
              }}
              return api;
            }}

            /** bundled / 资源库 / 相对 / 绝对四种引用统一的取文本入口。 */
            async function loadChartData(ref, baseDirHint) {{
              const api = chartPreviewApi();
              const value = String(ref || '').trim();
              // bundled: 样本已随包打进页面，不碰文件系统，任何机器都能画。
              if (api.isBundledRef(value)) {{
                const sample = api.getBundledSample(value);
                if (!sample) {{
                  throw new Error('没有内置样本「' + value + '」。可用：\n'
                    + api.listBundledSamples().map((k) => '  - bundled:' + k).join('\n'));
                }}
                return sample.text;
              }}
              // 1) 资源库引用：交给后端解析，前端不需要知道库根目录。
              if (/^resource:/i.test(value)) {{
                const rel = value.replace(/^resource:/i, '').replace(/^[/\\]+/, '');
                const resp = await fetch('/browser/resource?path=' + encodeURIComponent(rel));
                if (!resp.ok) throw new Error('资源库数据读取失败: ' + resp.status);
                return await resp.text();
              }}
              // 2) 相对路径：相对当前笔记目录，经 /browser/file 的静态读取端点。
              const absolute = /^([a-zA-Z]:[\\/]|\/|~[\\/])/.test(value)
                ? value
                : ((baseDirHint ? baseDirHint.replace(/[\\/]+$/, '') + '/' : '') + value.replace(/^\.\//, ''));
              const resp = await fetch('/browser/file?path=' + encodeURIComponent(absolute)
                + '&note=' + encodeURIComponent(notePath));
              if (!resp.ok) throw new Error('数据文件读取失败: ' + resp.status + ' ' + absolute);
              return await resp.text();
            }}

            /** 围栏里 `data:` 写的是外部引用，而不是内联文本。 */
            function isExternalDataRef(parsed) {{
              const value = String(parsed && parsed.data ? parsed.data : '');
              if (!value || parsed.dataInline) return false;
              if (/\r?\n/.test(value)) return false;
              return /^(bundled|resource):/i.test(value)
                || /^([a-zA-Z]:[\\/]|\/|~[\\/])/.test(value)
                || /^(\.[\\/]|\.\.[\\/])/.test(value)
                || /\.(csv|tsv|json|excalidraw)$/i.test(value);
            }}

            /** 围栏正文 → ECharts option，语义与软件内渲染链路完全一致。 */
            async function buildChartPreviewOption(source, baseDirHint) {{
              const api = chartPreviewApi();
              const parsed = api.parseChartFence(source);
              if (!parsed.spec) {{
                throw new Error('无法解析 chart 围栏（spec 需要是 JSON 或 key: value 列表）');
              }}
              let raw = parsed.data;
              let format = parsed.dataFormat;
              if (isExternalDataRef(parsed)) {{
                raw = await loadChartData(parsed.data, baseDirHint);
                format = 'auto';
              }}
              const records = api.normalizeDataset(raw, format);
              const option = api.buildChartOption(parsed.spec, records);
              // 「围栏写了 data」= 需要外部数据；直接贴整段 option 的写法没有 data。
              return {{ option: option, records: records, expectsData: !!raw && raw.trim() !== '' }};
            }}

            // 轮询与手动刷新共用同一个入口：同一时间只允许一轮渲染。
            // 以前文件在渲染途中再次变化时，两轮渲染会互相覆盖，
            // 图表实例与监听也会成倍叠加。
            let renderBusy = false;

            async function loadAndRender() {{
              if (renderBusy) return;
              renderBusy = true;
              try {{
                await renderPreviewContent();
              }} finally {{
                renderBusy = false;
              }}
            }}

            async function renderPreviewContent() {{
              if (!contentEl) {{
                throw new Error('Missing #content element');
              }}

              const rawResp = await fetch('/browser/raw?path=' + filePathEncoded);
              if (!rawResp.ok) {{
                let detail = '';
                try {{ detail = await rawResp.text(); }} catch (_) {{}}
                contentEl.textContent = '读取失败: ' + rawResp.status + (detail ? (' - ' + detail) : '');
                return;
              }}

              const md = await rawResp.text();

              // 基于内容的简易变更检测：如果内容未变化，则不重复渲染
              if (window.__guaxiLastPreviewMd !== undefined && window.__guaxiLastPreviewMd === md) {{
                return;
              }}
              window.__guaxiLastPreviewMd = md;

              let processed = md;
              try {{
                const procResp = await fetch('/api/markdown/image_process', {{
                  method: 'POST',
                  headers: {{ 'Content-Type': 'application/json' }},
                  body: JSON.stringify({{ content: md, base_path: baseDir }})
                }});
                if (procResp.ok) {{
                  const json = await procResp.json();
                  processed = json.processed_content || md;
                }}
              }} catch (e) {{}}

              if (!window.markdownit) {{
                contentEl.innerHTML = '<pre><code>' + escapeHtml(md) + '</code></pre>';
                return;
              }}

              const mdIt = window.markdownit({{ html: true, linkify: true, breaks: true }});
              // ```chart 围栏：先转成占位 div，渲染完 markdown 后再交给 ECharts。
              const chartDrafts = [];
              const excalidrawDrafts = [];
              const defaultFence = mdIt.renderer.rules.fence;
              mdIt.renderer.rules.fence = (tokens, idx, options, env, self) => {{
                const token = tokens[idx];
                const info = String((token && token.info) || '').trim();
                if (/^chart(\s|$)/i.test(info)) {{
                  const id = 'guaxi-chart-' + chartDrafts.length;
                  chartDrafts.push({{ id: id, source: String(token.content || '') }});
                  return '<div class="guaxi-chart" id="' + id + '"></div>';
                }}
                // excalidraw 围栏：静态页里用官方 exportToSvg 渲染成只读 SVG。
                // 导出器随页面一起分发（见 /browser/assets/excalidraw/excalidraw-export.js），
                // 场景文件读取复用图表数据那条端点。
                if (/^excalidraw(\s|$)/i.test(info)) {{
                  const scenePath = String(token.content || '').trim();
                  const id = 'guaxi-excalidraw-' + excalidrawDrafts.length;
                  excalidrawDrafts.push({{ id: id, source: scenePath }});
                  return '<div class="guaxi-excalidraw" id="' + id + '"></div>';
                }}
                return defaultFence(tokens, idx, options, env, self);
              }};

              // 旧 DOM 即将被整体替换，先把挂在它上面的图表实例销毁，避免泄漏。
              disposePreviewCharts();
              contentEl.innerHTML = mdIt.render(processed);
              enhanceMediaGroups(contentEl);

              // 只有这次渲染真的用到某类图表时才注入对应脚本；不含图表的文档不会下载任何重型资源。
              try {{
                await ensurePreviewAssets(
                  detectPreviewAssets(contentEl, chartDrafts, excalidrawDrafts, processed)
                );
              }} catch (error) {{
                console.warn('[Guaxi Preview] 渲染资源加载失败：', error);
              }}

              await renderChartDrafts(contentEl, chartDrafts, baseDir);
              await renderExcalidrawDrafts(contentEl, excalidrawDrafts, baseDir);

              if (window.MathJax) {{
                try {{
                  if (window.MathJax.typesetPromise) {{
                    await window.MathJax.typesetPromise([contentEl]);
                  }} else if (window.MathJax.startup && window.MathJax.startup.promise) {{
                    await window.MathJax.startup.promise;
                    if (window.MathJax.typesetPromise) {{
                      await window.MathJax.typesetPromise([contentEl]);
                    }}
                  }}
                }} catch (_) {{
                }}
              }}

              if (window.mermaid) {{
                try {{
                  window.mermaid.initialize({{ startOnLoad: false }});

                  const mermaidCodeBlocks = contentEl.querySelectorAll('pre code.language-mermaid, pre code.mermaid');
                  mermaidCodeBlocks.forEach((codeEl) => {{
                    const preEl = codeEl.parentElement;
                    if (!preEl) return;

                    const container = document.createElement('div');
                    container.className = 'mermaid';
                    container.textContent = codeEl.textContent || '';

                    preEl.replaceWith(container);
                  }});

                  const nodes = contentEl.querySelectorAll('.mermaid');
                  if (nodes.length) {{
                    await window.mermaid.run({{ nodes }});
                  }}
                }} catch (_) {{
                }}
              }}

              if (window.hljs) {{
                try {{
                  const codeBlocks = contentEl.querySelectorAll('pre code');
                  codeBlocks.forEach((codeEl) => {{
                    try {{
                      window.hljs.highlightElement(codeEl);
                    }} catch (_) {{
                    }}
                  }});
                }} catch (_) {{
                }}
              }}
            }}

            loadAndRender().catch((e) => {{
              if (!contentEl) return;
              contentEl.textContent = (e && e.message) ? e.message : '渲染失败';
            }});

            // 启动轮询检测，文件保存后自动刷新预览
            const POLL_INTERVAL_MS = 1200;

            function startAutoReload() {{
              if (window.__guaxiPreviewAutoReloadStarted) {{
                return;
              }}
              window.__guaxiPreviewAutoReloadStarted = true;

              console.log('[Guaxi Preview] Auto-reload watcher started, interval =', POLL_INTERVAL_MS, 'ms');

              setInterval(() => {{
                loadAndRender().catch((e) => {{
                  console.warn('[Guaxi Preview] Auto-reload failed:', e);
                }});
              }}, POLL_INTERVAL_MS);
            }}

            startAutoReload();
          </script>
        </body>
        </html>"#,
        title = escape_html(title),
        path = escape_html(file_path.to_string_lossy().as_ref()),
        filePathJson = serde_json::to_string(&encoded_path).unwrap_or_else(|_| "\"\"".to_string()),
        // 数据文件端点要带着当前笔记一起请求：只有「在软件内点开预览」的那份笔记
        // 才有资格放行它引用的白板场景 / CSV，避免这条通道变成任意路径读取。
        notePathJson = serde_json::to_string(abs).unwrap_or_else(|_| "\"\"".to_string()),
        baseDirJson = serde_json::to_string(&base_dir).unwrap_or_else(|_| "\"\"".to_string()),
        encodedPath = encoded_path,
        assetVersion = PREVIEW_ASSET_VERSION,
        assetVersionJson =
            serde_json::to_string(PREVIEW_ASSET_VERSION).unwrap_or_else(|_| "\"\"".to_string())
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(html)
}

#[get("/browser/assets/markdown-it/markdown-it.min.js")]
async fn browser_asset_markdown_it() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/markdown-it.min.js"))
}

#[get("/browser/assets/mermaid/mermaid.min.js")]
async fn browser_asset_mermaid() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/mermaid.min.js"))
}

#[get("/browser/assets/mathjax/tex-svg.js")]
async fn browser_asset_mathjax() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/tex-svg.js"))
}

#[get("/browser/assets/highlightjs/highlight.min.js")]
async fn browser_asset_highlight_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/highlight.min.js"))
}

/// ECharts UMD 包，供 ```chart 围栏在静态预览页里画图。
///
/// 预览页是独立于前端渲染器的静态页面（markdown-it + 少量内联脚本），
/// 不共享前端的图表模块，因此这里直接把官方 UMD 包分发给页面。
#[get("/browser/assets/echarts/echarts.min.js")]
async fn browser_asset_echarts() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/echarts.min.js"))
}

/// 图表扩展包（水球图 / 词云）。它们靠副作用把系列注册进 `window.echarts`。
#[get("/browser/assets/echarts/echarts-liquidfill.min.js")]
async fn browser_asset_echarts_liquidfill() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/echarts-liquidfill.min.js"))
}

#[get("/browser/assets/echarts/echarts-wordcloud.min.js")]
async fn browser_asset_echarts_wordcloud() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/echarts-wordcloud.min.js"))
}

/// Excalidraw 官方导出器的独立构建（含内嵌字体），供静态预览页把白板场景渲染成 SVG。
///
/// 为什么要单独打一份：前端用的是 React 组件包，静态页没有 React、也没有打包器。
/// 这个 IIFE 暴露 `window.GuaxiExcalidrawExport.exportToSvg`，产物里字体与图片都是
/// data URL，因此离线打开预览页也能正确显示。
#[get("/browser/assets/excalidraw/excalidraw-export.js")]
async fn browser_asset_excalidraw_export() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/excalidraw-export.js"))
}

/// 图表解析 / option 组装的共享模块（软件内同一份纯逻辑的 IIFE 打包）。
///
/// 页面里不再自备一份精简解析器：那份副本已经和软件内漂移，导致树图、旭日、
/// 关系图、热力图、K 线、甘特图在浏览器预览里全部报「暂不支持」。
/// 产物由 `node ./tools/build_chart_preview_bundle.mjs` 生成。
#[get("/browser/assets/chart-preview/guaxi-chart-preview.js")]
async fn browser_asset_chart_preview() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/guaxi-chart-preview.js"))
}

#[get("/browser/assets/highlightjs/styles/atom-one-light.css")]
async fn browser_asset_highlight_css() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css; charset=utf-8")
        .append_header(("Cache-Control", ASSET_CACHE_CONTROL))
        .body(include_str!("../../assets/vendor/atom-one-light.css"))
}

pub fn browser_preview_config(cfg: &mut web::ServiceConfig) {
    cfg.service(favicon)
        .service(browser_index)
        .service(browser_knowledge_bases)
        .service(browser_knowledge_base_files)
        .service(browser_kb_search)
        .service(browser_raw)
        .service(browser_file)
        .service(browser_resource)
        .service(browser_download)
        .service(browser_asset_markdown_it)
        .service(browser_asset_mermaid)
        .service(browser_asset_mathjax)
        .service(browser_asset_highlight_js)
        .service(browser_asset_echarts)
        .service(browser_asset_echarts_liquidfill)
        .service(browser_asset_echarts_wordcloud)
        .service(browser_asset_excalidraw_export)
        .service(browser_asset_chart_preview)
        .service(browser_asset_highlight_css)
        .service(browser_preview);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "guaxi_preview_test_{}_{}",
            std::process::id(),
            name
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn explicit_request_allows_markdown_outside_knowledge_bases() {
        // 这条登记就是为 plain/task 模式的外部目录准备的：它们不在任何知识库根下。
        let dir = unique_temp_file("allow");
        let file = dir.join("note.md");
        fs::write(&file, "# hi").expect("write temp md");
        let canon = file.canonicalize().expect("canonicalize");

        assert!(!is_remembered_preview_file(&canon));
        remember_preview_file(&canon);
        assert!(is_remembered_preview_file(&canon));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn only_the_exact_file_and_its_neighbours_are_allowed() {
        let dir = unique_temp_file("scope");
        let note = dir.join("note.md");
        let sibling = dir.join("data.csv");
        let outsider = dir.join("sub").join("other.csv");
        fs::write(&note, "# hi").expect("write note");
        fs::write(&sibling, "a,b").expect("write sibling");
        fs::create_dir_all(outsider.parent().unwrap()).expect("create sub");
        fs::write(&outsider, "a,b").expect("write outsider");

        let note_canon = note.canonicalize().expect("canon note");
        remember_preview_file(&note_canon);

        // 同目录的数据文件（图表 data: ./x.csv）放行，其它目录不放行。
        assert!(is_sibling_of_remembered_preview(
            &sibling.canonicalize().expect("canon sibling")
        ));
        assert!(!is_sibling_of_remembered_preview(
            &outsider.canonicalize().expect("canon outsider")
        ));
        assert!(!is_remembered_preview_file(
            &outsider.canonicalize().expect("canon outsider")
        ));

        fs::remove_dir_all(&dir).ok();
    }

    /// 预览页必须真的把白板交给导出器，而不是退回「请在软件内查看」的提示。
    ///
    /// 这条约束曾经以两种方式被破坏：脚本没引入、或围栏仍渲染成旧提示文本。
    /// 页面的 HTML 是 `format!` 字符串，编译期不会检查其中脚本内容，所以用测试兜住。
    #[test]
    fn preview_page_wires_excalidraw_exporter() {
        let source = include_str!("markdown_browser.rs");
        assert!(
            source.contains("/browser/assets/excalidraw/excalidraw-export.js"),
            "预览页缺少 excalidraw 导出器脚本"
        );
        assert!(
            source.contains("renderExcalidrawDrafts"),
            "预览页缺少白板渲染函数"
        );
        assert!(
            !source.contains("guaxi-excalidraw-note\">Excalidraw 白板："),
            "白板仍会退化成「请在软件内查看」的提示"
        );
    }

    /// 图表必须在静态页里等异步布局完成，否则词云会导出成空图。
    #[test]
    fn preview_page_waits_for_async_chart_layout() {
        let source = include_str!("markdown_browser.rs");
        assert!(source.contains("waitForChartPaint"), "缺少等待绘制的逻辑");
        assert!(
            source.contains("await waitForChartPaint(chart, built.option)"),
            "渲染入口没有调用等待逻辑"
        );
    }

    /// 非词云图表必须立刻返回。
    ///
    /// 早先按 SVG 子元素数量判定「画完了」，但 zrender 的 SVG 输出固定是
    /// 「背景 rect + <g id="main">」，于是每张普通图表都空等满 3 秒超时。
    #[test]
    fn preview_page_does_not_wait_for_synchronous_charts() {
        let source = include_str!("markdown_browser.rs");
        // 拼出名字再比对：直接写全名会让断言匹配到它自己。
        let needle = format!("childElementCount > {}", 2);
        assert!(
            !source.contains(&needle),
            "同步绘制的图表仍会按子元素数量判定，导致空等超时"
        );
        assert!(
            source.contains("if (!isWordCloud) return true"),
            "非词云图表必须直接返回，不做等待"
        );
    }

    /// 重型渲染资源必须按需注入，而不是在 <head> 里同步加载。
    ///
    /// mermaid / MathJax / ECharts / Excalidraw 合计约 14MB，一篇纯文字的笔记
    /// 不该为它们付出下载与解析的代价。
    #[test]
    fn preview_page_loads_heavy_assets_on_demand() {
        let source = include_str!("markdown_browser.rs");
        assert!(source.contains("ensurePreviewAssets"), "缺少按需加载入口");
        assert!(source.contains("detectPreviewAssets"), "缺少资源需求探测");
        // 路径与标签分开拼：整串写出来会让断言匹配到测试自己。
        let prefix = "<script src=\"/browser/assets/";
        for path in [
            "mermaid/mermaid.min.js",
            "mathjax/tex-svg.js",
            "echarts/echarts.min.js",
            "excalidraw/excalidraw-export.js",
        ] {
            let needle = format!("{prefix}{path}\"");
            assert!(!source.contains(&needle), "重型资源仍是同步加载：{path}");
        }
    }

    /// vendor 资源必须允许浏览器长期缓存，且 URL 带内容指纹。
    #[test]
    fn preview_assets_are_cacheable_and_versioned() {
        let source = include_str!("markdown_browser.rs");
        assert!(
            source.contains("public, max-age=31536000, immutable"),
            "静态资源没有长缓存，每次打开预览都要重传"
        );
        assert!(
            source.contains("PREVIEW_ASSET_VERSION"),
            "静态资源 URL 缺少内容指纹，升级后会命中旧缓存"
        );
    }

    /// 图表解析必须复用软件内的共享模块，而不是页面里自带的精简副本。
    ///
    /// 这条约束曾经以最隐蔽的方式失效：页面里那份副本能画出折线/柱状，
    /// 于是「预览看起来是好的」；直到用户画树图、旭日、关系图、热力、K 线或甘特，
    /// 才报「静态预览暂不支持该图表类型」。共享模块一旦被删回去，这里立刻报警。
    #[test]
    fn preview_page_reuses_shared_chart_module() {
        let source = include_str!("markdown_browser.rs");
        assert!(
            source.contains("/browser/assets/chart-preview/guaxi-chart-preview.js"),
            "预览页没有引入共享图表模块"
        );
        assert!(
            source.contains("buildChartPreviewOption"),
            "预览页没有走共享模块的解析入口"
        );
        // 拼出名字再比对：直接写全名会让断言匹配到它自己。
        for gone in ["parseChartSource", "parseChartRows", "parseSimpleYaml"] {
            let needle = format!("function {}", gone);
            assert!(
                !source.contains(&needle),
                "页面里又出现了自带的精简解析器：{needle}"
            );
        }
    }

    /// 知识库列表页必须带检索能力，而不是只能一篇篇点。
    #[test]
    fn knowledge_base_page_offers_search() {
        let source = include_str!("markdown_browser.rs");
        assert!(
            source.contains("/browser/knowledge-bases/{id}/search"),
            "缺少正文检索端点"
        );
        assert!(source.contains("filterByName"), "缺少文件名筛选");
        assert!(source.contains("searchContent"), "缺少正文检索");
    }
}
