use crate::commands::knowledgebase::get_knowledge_base_list;
use actix_web::{get, web, App, HttpResponse, HttpServer};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use urlencoding::{decode, encode};
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

fn collect_markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
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

    let result = web::block(|| get_knowledge_base_list()).await;
    let kbs = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return HttpResponse::InternalServerError().body(e),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let kb = match kbs.into_iter().find(|x| x.id == kb_id) {
        Some(v) => v,
        None => return HttpResponse::NotFound().body("Knowledge base not found"),
    };

    let root = PathBuf::from(&kb.location);
    let root_canon = match root.canonicalize() {
        Ok(v) => v,
        Err(_) => return HttpResponse::NotFound().body("Knowledge base path not found"),
    };

    let root_canon_for_list = root_canon.clone();
    let files = match web::block(move || {
        let mut out = Vec::new();
        collect_markdown_files(&root_canon_for_list, &mut out).map(|_| out)
    })
    .await
    {
        Ok(Ok(v)) => v,
        _ => return HttpResponse::InternalServerError().finish(),
    };

    info!(
        kb_id = kb_id,
        file_count = files.len(),
        "browser preview: knowledge base markdown files collected"
    );

    let mut items = String::new();
    for file in files {
        let display = file
            .strip_prefix(&root_canon)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| file.to_string_lossy().replace('\\', "/"));

        let abs = file.to_string_lossy();
        let abs = abs.trim_start_matches(r"\\?\");

        let href = format!("/browser/preview?path={}", encode(abs));

        items.push_str(&format!(
            "<li><a href=\"{href}\">{name}</a><div class=\"sub\">{path}</div></li>",
            href = href,
            name = escape_html(file.file_name().and_then(|n| n.to_str()).unwrap_or("")),
            path = escape_html(&display)
        ));
    }

    let html = format!(
        r#"<!doctype html>
<html lang="zh">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Guaxi - {kb_name}</title>
  <style>
    body {{ font-family: -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Arial,'Noto Sans','PingFang SC','Microsoft YaHei',sans-serif; margin: 24px; color: #1f2328; }}
    h1 {{ font-size: 18px; margin: 0 0 8px; }}
    .back {{ display: inline-block; margin: 0 0 14px; font-size: 13px; }}
    ul {{ list-style: none; padding: 0; margin: 0; }}
    li {{ padding: 10px 12px; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px; margin-bottom: 10px; }}
    a {{ color: #0969da; text-decoration: none; font-weight: 600; }}
    a:hover {{ text-decoration: underline; }}
    .sub {{ margin-top: 6px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
  </style>
</head>
<body>
  <a class="back" href="/browser/knowledge-bases">← 返回知识库列表</a>
  <h1>{kb_name}</h1>
  <ul>{items}</ul>
</body>
</html>"#,
        kb_name = escape_html(&kb.knowledge_base_name),
        items = items
    );

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[derive(Deserialize)]
struct PathQuery {
    path: String,
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
        Ok(Ok(content)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .append_header(("Cache-Control", "no-store"))
            .body(content),
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
    .title {{ font-weight: 600; font-size: 13px; word-break: break-all; }}
    .sub {{ margin-top: 4px; font-size: 12px; color: rgba(31,35,40,0.6); word-break: break-all; }}
    .content {{ max-width: 980px; margin: 0 auto; padding: 18px 16px 60px; }}
    pre {{ background: rgba(0,0,0,0.04); padding: 12px; border-radius: 10px; overflow: auto; }}
    code {{ font-family: ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,'Liberation Mono','Courier New',monospace; }}
    img {{ max-width: 100%; }}
    a {{ color: #0969da; }}
    table {{ border-collapse: collapse; width: 100%; margin: 12px 0; }}
    th, td {{ border: 1px solid rgba(0,0,0,0.16); padding: 6px 10px; }}
    thead th {{ background: rgba(0,0,0,0.04); }}
  </style>
  <link rel="stylesheet" href="/browser/assets/highlightjs/styles/atom-one-light.css" />
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
  <script src="/browser/assets/markdown-it/markdown-it.min.js"></script>
  <script src="/browser/assets/mermaid/mermaid.min.js"></script>
  <script id="MathJax-script" src="/browser/assets/mathjax/tex-svg.js"></script>
  <script src="/browser/assets/highlightjs/highlight.min.js"></script>
  </head>
<body>
  <div class="topbar">
    <div class="title">{title}</div>
    <div class="sub">{path}</div>
  </div>
  <div class="content" id="content">加载中…</div>

  <script>
    const contentEl = document.getElementById('content');
    const filePathEncoded = {filePathJson};
    const baseDir = {baseDirJson};

    function escapeHtml(s) {{
      return String(s)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;');
    }}

    async function loadAndRender() {{
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

      contentEl.innerHTML = mdIt.render(processed);

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
  </script>
</body>
</html>"#,
        title = escape_html(title),
        path = escape_html(file_path.to_string_lossy().as_ref()),
        filePathJson = serde_json::to_string(&encoded_path).unwrap_or_else(|_| "\"\"".to_string()),
        baseDirJson = serde_json::to_string(&base_dir).unwrap_or_else(|_| "\"\"".to_string())
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
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/markdown-it.min.js"))
}

#[get("/browser/assets/mermaid/mermaid.min.js")]
async fn browser_asset_mermaid() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/mermaid.min.js"))
}

#[get("/browser/assets/mathjax/tex-svg.js")]
async fn browser_asset_mathjax() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/tex-svg.js"))
}

#[get("/browser/assets/highlightjs/highlight.min.js")]
async fn browser_asset_highlight_js() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/highlight.min.js"))
}

#[get("/browser/assets/highlightjs/styles/atom-one-light.css")]
async fn browser_asset_highlight_css() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css; charset=utf-8")
        .append_header(("Cache-Control", "no-store"))
        .body(include_str!("../../assets/vendor/atom-one-light.css"))
}

pub fn browser_preview_config(cfg: &mut web::ServiceConfig) {
    cfg.service(favicon)
        .service(browser_index)
        .service(browser_knowledge_bases)
        .service(browser_knowledge_base_files)
        .service(browser_raw)
        .service(browser_asset_markdown_it)
        .service(browser_asset_mermaid)
        .service(browser_asset_mathjax)
        .service(browser_asset_highlight_js)
        .service(browser_asset_highlight_css)
        .service(browser_preview);
}