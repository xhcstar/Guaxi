//! 库网站嵌入式服务。
//!
//! 静态站点(index.html / styles.css / app.js)由 `include_str!` 编进二进制,
//! 离线自足,不依赖运行期落盘资源。挂两处:
//! - `config_local`:挂到既有 8888 loopback App,供 Guaxi 内嵌 webview 访问,无 token;
//! - `config_lan`:供对外 LAN 端口使用,每个请求校验 token(按文件粒度暴露、可撤销)。
//!
//! 检索复用 fulltext(tantivy + jieba)。浏览用 ignore 遍历库目录 + 每库缓存,
//! 十万份也只在首次/刷新时遍历一次,分页是切片。

use actix_web::{web, HttpRequest, HttpResponse};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::SharedState;

const INDEX_HTML: &str = include_str!("../../library_site/index.html");
const STYLES_CSS: &str = include_str!("../../library_site/styles.css");
const APP_JS: &str = include_str!("../../library_site/app.js");

#[derive(Clone, Serialize)]
struct Doc {
    path: String,      // 相对库根的正斜杠路径
    name: String,
    ext: String,
    size: u64,
    mtime: i64,        // unix 秒
    lib: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    band: Option<String>,
    // —— 文件库(imported_files)条目附带的元信息,供前端展示与本地阅读器重建路径 ——
    #[serde(skip_serializing_if = "Option::is_none")]
    desc: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    library: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
    /// 已映射到磁盘的类型目录名(pdf/markdown/word/epub/html/txt),阅读器按它定位文件。
    #[serde(skip_serializing_if = "Option::is_none")]
    ftype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    import_file_id: Option<i64>,
    /// 入库时间的原始展示串(create_time,文本;优先于 mtime 展示)。
    #[serde(skip_serializing_if = "Option::is_none")]
    time_text: Option<String>,
    /// 摘录笔记相对路径(存在时才有),可查看/下载。
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note_name: Option<String>,
}

impl Doc {
    /// 联邦/兜底场景用的极简构造:只有基本字段,元信息留空。
    fn basic(path: String, name: String, ext: String, size: u64, mtime: i64, lib: String) -> Self {
        Doc {
            path, name, ext, size, mtime, lib,
            snippet: None, source: None, band: None,
            desc: None, tags: Vec::new(), library: None, repo: None,
            ftype: None, import_file_id: None, time_text: None, note: None, note_name: None,
        }
    }
}
// —— 文件库数据源(repos.db 的 imported_files;不再展示知识库) ——

const REPO_BAND: &str = "#c07a2b";

/// 文件库在磁盘上的根:<libraries_root>/Libraries。所有本机库文件都在其下,
/// 相对路径形如 <library>/<stem>/<type_dir>/<filename>,api_file 一律以此为 base。
fn repo_disk_root() -> PathBuf {
    PathBuf::from(crate::commands::settingview::get_libraries_root()).join("Libraries")
}

/// 把 DB 里的文件类型映射到磁盘上的类型目录名(与前端 useContextMenu.mapFileType 一致)。
/// 返回 None 表示未知类型,此时文件直接落在 <stem>/ 下,无类型子目录。
fn map_type_dir(raw: &str) -> Option<&'static str> {
    match raw.to_ascii_lowercase().as_str() {
        "pdf" => Some("pdf"),
        "md" | "markdown" => Some("markdown"),
        "html" | "htm" => Some("html"),
        "epub" => Some("epub"),
        "doc" | "docx" | "word" => Some("word"),
        "txt" => Some("txt"),
        _ => None,
    }
}

fn file_stem_of(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// 本机库 id 方案:
///   `repo`                    → 全部文件
///   `repo:<hexKR>:<hexLIB>`   → 指定知识库(kr)下的文件库(library);hexLIB 为空则整个 kr
/// kr/library 名可能含任意字符,用 hex 编码规避分隔符冲突与路径穿越。
fn decode_repo_scope(lib: &str) -> Option<(Option<String>, Option<String>)> {
    if lib == "repo" {
        return Some((None, None));
    }
    let rest = lib.strip_prefix("repo:")?;
    let (hk, hl) = rest.split_once(':').unwrap_or((rest, ""));
    let kr = hex::decode(hk).ok().and_then(|b| String::from_utf8(b).ok());
    let library = if hl.is_empty() {
        None
    } else {
        hex::decode(hl).ok().and_then(|b| String::from_utf8(b).ok())
    };
    Some((kr, library))
}

fn encode_repo_scope(kr: &str, library: &str) -> String {
    if library.is_empty() {
        format!("repo:{}:", hex::encode(kr.as_bytes()))
    } else {
        format!("repo:{}:{}", hex::encode(kr.as_bytes()), hex::encode(library.as_bytes()))
    }
}

/// 把请求里的相对 path 安全拼到库根下:规范化后必须仍在根内,否则拒绝(防穿越)。
fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/");
    if rel.contains("..") {
        return None;
    }
    let joined = base.join(rel.trim_start_matches('/'));
    let canon_base = base.canonicalize().ok()?;
    let canon = joined.canonicalize().ok()?;
    if canon.starts_with(&canon_base) {
        Some(canon)
    } else {
        None
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// 枚举本机文件库:顶部"全部文件" + 每个 (知识库, 文件库) 一个条目,计数走 SQL。
fn enumerate_local() -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let conn = match crate::commands::repoview::init_db() {
        Ok(c) => c,
        Err(_) => return out,
    };
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM imported_files WHERE deleted=0", [], |r| r.get(0))
        .unwrap_or(0);
    out.push(json!({ "id": "repo", "name": "全部文件", "band": REPO_BAND, "count": total }));
    if let Ok(mut stmt) = conn.prepare(
        "SELECT knowledge_repo, library, COUNT(*) FROM imported_files WHERE deleted=0 \
         GROUP BY knowledge_repo, library ORDER BY knowledge_repo, library",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        }) {
            for (kr, library, count) in rows.flatten() {
                let name = if kr.is_empty() { library.clone() } else { format!("{} / {}", kr, library) };
                out.push(json!({
                    "id": encode_repo_scope(&kr, &library),
                    "name": name,
                    "band": REPO_BAND,
                    "count": count,
                }));
            }
        }
    }
    out
}
// —— 浏览列表 / 元数据检索:直接查 imported_files(一条导入文件一行) ——

fn repo_order_clause(sort: &str) -> &'static str {
    match sort {
        "name" => "original_filename COLLATE NOCASE ASC",
        "size" => "original_filesize DESC",
        _ => "create_time DESC",
    }
}

/// 由一行 imported_files 记录构造 Doc:算磁盘相对路径、查标签、找摘录笔记。
#[allow(clippy::too_many_arguments)]
fn build_repo_doc(
    conn: &rusqlite::Connection,
    lib: &str,
    id: i64,
    fname: String,
    desc: Option<String>,
    library: String,
    ftype_raw: String,
    ctime: String,
    size: i64,
    repo: String,
    snippet: Option<String>,
) -> Doc {
    let stem = file_stem_of(&fname);
    let type_dir = map_type_dir(&ftype_raw).map(|s| s.to_string());
    let ext = fname.rsplit_once('.').map(|(_, x)| x.to_string()).unwrap_or_default();
    let rel = match &type_dir {
        Some(td) => format!("{}/{}/{}/{}", library, stem, td, fname),
        None => format!("{}/{}/{}", library, stem, fname),
    };
    // 标签
    let mut tags = Vec::new();
    if let Ok(mut st) = conn.prepare(
        "SELECT ift.name FROM imported_files_tags ift \
         JOIN imported_files_tags_relations iftr ON ift.id = iftr.tag_id WHERE iftr.file_id = ?",
    ) {
        if let Ok(rows) = st.query_map(rusqlite::params![id], |r| r.get::<_, String>(0)) {
            tags = rows.flatten().collect();
        }
    }
    // 摘录笔记(可选):notes.from_imported_file = 原始文件名,落在 <library>/<stem>/notes/ 下
    let mut note = None;
    let mut note_name = None;
    if let Ok(nname) = conn.query_row(
        "SELECT note_file_name FROM notes WHERE from_imported_file = ? LIMIT 1",
        rusqlite::params![fname],
        |r| r.get::<_, String>(0),
    ) {
        note = Some(format!("{}/{}/notes/{}", library, stem, nname));
        note_name = Some(nname);
    }
    Doc {
        path: rel,
        name: fname,
        ext,
        size: size.max(0) as u64,
        mtime: 0,
        lib: lib.to_string(),
        snippet,
        source: None,
        band: Some(REPO_BAND.to_string()),
        desc: desc.filter(|s| !s.is_empty()),
        tags,
        library: Some(library),
        repo: Some(repo),
        ftype: type_dir,
        import_file_id: Some(id),
        time_text: Some(ctime).filter(|s| !s.is_empty()),
        note,
        note_name,
    }
}

const REPO_COLS: &str = "id, original_filename, description, library, original_filetype, create_time, original_filesize, knowledge_repo";

/// 把一行查询结果读成 build_repo_doc 需要的元组。
fn read_repo_row(
    r: &rusqlite::Row,
) -> rusqlite::Result<(i64, String, Option<String>, String, String, String, i64, String)> {
    Ok((
        r.get::<_, i64>(0)?,
        r.get::<_, String>(1)?,
        r.get::<_, Option<String>>(2)?,
        r.get::<_, String>(3)?,
        r.get::<_, String>(4)?,
        r.get::<_, String>(5)?,
        r.get::<_, i64>(6)?,
        r.get::<_, String>(7)?,
    ))
}

/// 浏览分页:按 scope(全部 / kr / kr+library)过滤 imported_files。
fn repo_list_page(lib: &str, sort: &str, offset: usize, limit: usize) -> (usize, Vec<Doc>) {
    let Some((kr, library)) = decode_repo_scope(lib) else {
        return (0, Vec::new());
    };
    let conn = match crate::commands::repoview::init_db() {
        Ok(c) => c,
        Err(_) => return (0, Vec::new()),
    };
    let mut cond = String::from("deleted=0");
    let mut vals: Vec<String> = Vec::new();
    if let Some(kr) = &kr {
        cond.push_str(" AND knowledge_repo=?");
        vals.push(kr.clone());
    }
    if let Some(library) = &library {
        cond.push_str(" AND library=?");
        vals.push(library.clone());
    }
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM imported_files WHERE {cond}"),
            rusqlite::params_from_iter(vals.iter()),
            |r| r.get(0),
        )
        .unwrap_or(0);
    let sql = format!(
        "SELECT {REPO_COLS} FROM imported_files WHERE {cond} ORDER BY {} LIMIT ? OFFSET ?",
        repo_order_clause(sort)
    );
    let mut page_vals = vals.clone();
    page_vals.push(limit.to_string());
    page_vals.push(offset.to_string());
    let mut docs = Vec::new();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(page_vals.iter()), read_repo_row) {
            for row in rows.flatten() {
                let (id, fname, desc, lib_name, ftype, ctime, size, repo) = row;
                docs.push(build_repo_doc(&conn, lib, id, fname, desc, lib_name, ftype, ctime, size, repo, None));
            }
        }
    }
    (total as usize, docs)
}

/// 元数据检索:文件名 / 描述 / 标签 LIKE 匹配,受 scope 约束。
fn repo_meta_search(lib: &str, query: &str, offset: usize, limit: usize) -> (usize, Vec<Doc>) {
    let (kr, library) = decode_repo_scope(lib).unwrap_or((None, None));
    let conn = match crate::commands::repoview::init_db() {
        Ok(c) => c,
        Err(_) => return (0, Vec::new()),
    };
    let like = format!("%{}%", query);
    let mut cond = String::from(
        "deleted=0 AND (original_filename LIKE ? OR description LIKE ? OR EXISTS (\
         SELECT 1 FROM imported_files_tags_relations r JOIN imported_files_tags t ON t.id=r.tag_id \
         WHERE r.file_id=imported_files.id AND t.name LIKE ?))",
    );
    let mut vals: Vec<String> = vec![like.clone(), like.clone(), like];
    if let Some(kr) = &kr {
        cond.push_str(" AND knowledge_repo=?");
        vals.push(kr.clone());
    }
    if let Some(library) = &library {
        cond.push_str(" AND library=?");
        vals.push(library.clone());
    }
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM imported_files WHERE {cond}"),
            rusqlite::params_from_iter(vals.iter()),
            |r| r.get(0),
        )
        .unwrap_or(0);
    let sql = format!(
        "SELECT {REPO_COLS} FROM imported_files WHERE {cond} ORDER BY create_time DESC LIMIT ? OFFSET ?"
    );
    let mut page_vals = vals.clone();
    page_vals.push(limit.to_string());
    page_vals.push(offset.to_string());
    let mut docs = Vec::new();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(rusqlite::params_from_iter(page_vals.iter()), read_repo_row) {
            for row in rows.flatten() {
                let (id, fname, desc, lib_name, ftype, ctime, size, repo) = row;
                docs.push(build_repo_doc(&conn, lib, id, fname, desc, lib_name, ftype, ctime, size, repo, None));
            }
        }
    }
    (total as usize, docs)
}

/// 全文检索命中(磁盘绝对路径)回填 imported_files 元信息,拿到 id/类型目录/知识库,
/// 便于本地阅读器打开;查不到就返回 None(由调用方兜底成裸命中)。
fn enrich_fulltext_hit(conn: &rusqlite::Connection, lib: &str, rel: &str, snippet: Option<String>) -> Option<Doc> {
    let mut parts = rel.split('/');
    let library = parts.next()?.to_string();
    let fname = rel.rsplit('/').next()?.to_string();
    let (id, desc, ftype, ctime, size, repo): (i64, Option<String>, String, String, i64, String) = conn
        .query_row(
            "SELECT id, description, original_filetype, create_time, original_filesize, knowledge_repo \
             FROM imported_files WHERE deleted=0 AND library=? AND original_filename=? LIMIT 1",
            rusqlite::params![library, fname],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .ok()?;
    Some(build_repo_doc(conn, lib, id, fname, desc, library, ftype, ctime, size, repo, snippet))
}
// —— Token 鉴权(对外 LAN 挂载启用;loopback 不启用) ——

/// 当前对外分享 token。None 表示未开启对外分享,LAN 挂载一律拒绝。
static SHARE_TOKEN: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// 每个挂载点是否需要校验 token。作为 app_data 注入,区分 loopback / LAN。
#[derive(Clone, Copy)]
struct SiteAuth {
    require_token: bool,
}

pub fn set_share_token(token: Option<String>) {
    *SHARE_TOKEN.lock().unwrap() = token;
}

fn token_ok(req: &HttpRequest) -> bool {
    let expect = SHARE_TOKEN.lock().unwrap().clone();
    let Some(expect) = expect else { return false };
    let from_header = req.headers().get("X-Guaxi-Token").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let from_query = req.query_string()
        .split('&')
        .find_map(|kv| kv.strip_prefix("t=").map(|v| v.to_string()));
    let got = from_header.or(from_query);
    got.as_deref() == Some(expect.as_str())
}

fn guard(req: &HttpRequest, auth: &SiteAuth) -> bool {
    !auth.require_token || token_ok(req)
}

// —— 静态站点 ——

async fn serve_index(req: HttpRequest, auth: web::Data<SiteAuth>) -> HttpResponse {
    if !guard(&req, auth.get_ref()) {
        return HttpResponse::Unauthorized().body("无效或缺失的分享令牌");
    }
    HttpResponse::Ok().content_type("text/html; charset=utf-8").body(INDEX_HTML)
}
async fn serve_css() -> HttpResponse {
    HttpResponse::Ok().content_type("text/css; charset=utf-8").body(STYLES_CSS)
}
async fn serve_js() -> HttpResponse {
    HttpResponse::Ok().content_type("application/javascript; charset=utf-8").body(APP_JS)
}

// —— API ——

async fn api_libraries(req: HttpRequest, auth: web::Data<SiteAuth>) -> HttpResponse {
    if !guard(&req, auth.get_ref()) {
        return HttpResponse::Unauthorized().finish();
    }
    HttpResponse::Ok().json(json!({ "local": enumerate_local(), "peers": peer_libs().await }))
}

async fn api_list(req: HttpRequest, auth: web::Data<SiteAuth>, q: web::Query<HashMap<String, String>>) -> HttpResponse {
    if !guard(&req, auth.get_ref()) {
        return HttpResponse::Unauthorized().finish();
    }
    let lib = q.get("lib").cloned().unwrap_or_else(|| "repo".into());
    let sort = q.get("sort").cloned().unwrap_or_else(|| "mtime".into());
    let offset = q.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let limit = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(200usize).min(1000);
    // 联邦库(其他设备):走 p2p 拉清单,分页切片。
    if lib.starts_with("peer:") {
        let (total, items) = peer_list_page(&lib, &sort, offset, limit).await;
        return HttpResponse::Ok().json(json!({ "total": total, "items": items }));
    }
    let (total, items) = web::block(move || repo_list_page(&lib, &sort, offset, limit))
        .await
        .unwrap_or((0, Vec::new()));
    HttpResponse::Ok().json(json!({ "total": total, "items": items }))
}
async fn api_search(
    req: HttpRequest,
    auth: web::Data<SiteAuth>,
    state: web::Data<SharedState>,
    q: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    if !guard(&req, auth.get_ref()) {
        return HttpResponse::Unauthorized().finish();
    }
    let pattern = q.get("q").cloned().unwrap_or_default();
    if pattern.trim().is_empty() {
        return HttpResponse::Ok().json(json!({ "total": 0, "items": [] }));
    }
    let lib = q.get("lib").cloned().unwrap_or_else(|| "repo".into());
    let offset = q.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0usize);
    let limit = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(200usize).min(1000);
    // 检索模式:meta(元数据:文件名/描述/标签) 或 fulltext(正文 tantivy)。默认 meta。
    let mode = q.get("mode").cloned().unwrap_or_else(|| "meta".into());
    // 联邦库:跨设备正文检索(对端 tantivy)。
    if lib.starts_with("peer:") {
        let band = peer_band_hex(lib.strip_prefix("peer:").and_then(|s| s.split_once(':').map(|(p, _)| p)).unwrap_or(""));
        return match crate::commands::p2psync::federation_search(&lib, &pattern, offset, limit).await {
            Ok((hits, has_more)) => {
                let items: Vec<Doc> = hits
                    .into_iter()
                    .map(|h| {
                        let ext = h.name.rsplit_once('.').map(|(_, x)| x.to_string()).unwrap_or_default();
                        let mut d = Doc::basic(h.rel, h.name, ext, 0, h.mtime_ms / 1000, lib.clone());
                        d.snippet = h.snippet;
                        d.band = Some(band.clone());
                        d
                    })
                    .collect();
                let total = offset + items.len() + if has_more { limit } else { 0 };
                HttpResponse::Ok().json(json!({ "total": total, "items": items }))
            }
            Err(e) => HttpResponse::Ok().json(json!({ "total": 0, "items": [], "error": e })),
        };
    }
    // 本机文件库 —— 元数据检索:走 SQL(文件名/描述/标签)。
    if mode != "fulltext" {
        let (total, items) = web::block(move || repo_meta_search(&lib, &pattern, offset, limit))
            .await
            .unwrap_or((0, Vec::new()));
        return HttpResponse::Ok().json(json!({ "total": total, "items": items }));
    }
    // 本机文件库 —— 全文检索:tantivy 检索磁盘正文,再按 scope 过滤、回填元信息。
    let Some((_, library)) = decode_repo_scope(&lib) else {
        return HttpResponse::Ok().json(json!({ "total": 0, "items": [] }));
    };
    let base = repo_disk_root();
    let Some(app) = state.get_app_handle() else {
        return HttpResponse::InternalServerError().body("app handle 未就绪");
    };
    let root = base.to_string_lossy().to_string();
    let result = crate::commands::fulltext::fulltext_search(
        app, pattern, root, Some(limit), Some(offset), None, None, None, None, None,
    )
    .await;
    match result {
        Ok((hits, has_more)) => {
            let conn = crate::commands::repoview::init_db().ok();
            let items: Vec<Doc> = hits
                .into_iter()
                .filter_map(|h| {
                    let abs = PathBuf::from(&h.path);
                    let rel = abs.strip_prefix(&base).unwrap_or(&abs).to_string_lossy().replace('\\', "/");
                    // scope 到具体 library 时,只保留该库命中。
                    if let Some(want) = &library {
                        if rel.split('/').next() != Some(want.as_str()) {
                            return None;
                        }
                    }
                    let snippet = h.preview.clone();
                    if let Some(conn) = &conn {
                        if let Some(doc) = enrich_fulltext_hit(conn, &lib, &rel, snippet.clone()) {
                            return Some(doc);
                        }
                    }
                    // 回填失败(如 notes 里的正文命中):退回裸命中,仍可查看/下载。
                    let ext = abs.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let mut d = Doc::basic(rel, h.file_name, ext, 0, h.modified_ms.map(|ms| ms / 1000).unwrap_or(0), lib.clone());
                    d.snippet = snippet;
                    d.band = Some(REPO_BAND.to_string());
                    Some(d)
                })
                .collect();
            let total = offset + items.len() + if has_more { limit } else { 0 };
            HttpResponse::Ok().json(json!({ "total": total, "items": items }))
        }
        Err(e) => HttpResponse::Ok().json(json!({ "total": 0, "items": [], "error": e })),
    }
}

async fn api_file(req: HttpRequest, auth: web::Data<SiteAuth>, q: web::Query<HashMap<String, String>>) -> HttpResponse {
    if !guard(&req, auth.get_ref()) {
        return HttpResponse::Unauthorized().finish();
    }
    let lib = q.get("lib").cloned().unwrap_or_default();
    let rel = q.get("path").cloned().unwrap_or_default();
    // dl=1 → 作为附件下载(Content-Disposition: attachment);否则内联(浏览/预览)。
    let download = q.get("dl").map(|v| v == "1" || v == "true").unwrap_or(false);
    let fname = rel.rsplit('/').next().unwrap_or("file").to_string();
    // 联邦文件:按需从对端拉取到内存后返回,不落地。
    if lib.starts_with("peer:") {
        return match crate::commands::p2psync::federation_pull_file(&lib, &rel).await {
            Ok(bytes) => {
                let mime = mime_guess::from_path(&rel).first_or_octet_stream();
                file_response(bytes, mime.as_ref(), &fname, download)
            }
            Err(e) => HttpResponse::NotFound().body(format!("联邦拉取失败: {}", e)),
        };
    }
    // 本机文件库:一律以 <libraries_root>/Libraries 为 base,rel 已含 <library>/... 前缀。
    if !lib.starts_with("repo") {
        return HttpResponse::BadRequest().body("非法库");
    }
    let base = repo_disk_root();
    let Some(full) = safe_join(&base, &rel) else {
        return HttpResponse::Forbidden().body("路径越界");
    };
    match std::fs::read(&full) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&full).first_or_octet_stream();
            file_response(bytes, mime.as_ref(), &fname, download)
        }
        Err(e) => HttpResponse::NotFound().body(format!("读取失败: {}", e)),
    }
}

/// 统一构造文件响应:下载时带 Content-Disposition: attachment(文件名 RFC5987 编码)。
fn file_response(bytes: Vec<u8>, mime: &str, fname: &str, download: bool) -> HttpResponse {
    let mut builder = HttpResponse::Ok();
    builder.content_type(mime);
    if download {
        // filename*=UTF-8''… 兼容中文名;同时给 ASCII 回退。
        let encoded = urlencoding::encode(fname);
        let disp = format!("attachment; filename=\"{}\"; filename*=UTF-8''{}", fname.replace('"', ""), encoded);
        builder.insert_header(("Content-Disposition", disp));
    }
    builder.body(bytes)
}

/// 联邦来源(其他设备的库):向可信设备查询开放的库清单,带来源设备名与色带。
/// 任一步失败都返回空,不阻断本机库展示。
async fn peer_libs() -> Vec<serde_json::Value> {
    match crate::commands::p2psync::federation_list_peer_libraries().await {
        Ok(libs) => libs
            .into_iter()
            .map(|l| {
                json!({
                    "id": l.id,
                    "name": l.name,
                    "peer": l.peer,
                    "count": l.count,
                    "band": l.band,
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

async fn api_discover(req: HttpRequest, auth: web::Data<SiteAuth>) -> HttpResponse {
    if !guard(&req, auth.get_ref()) {
        return HttpResponse::Unauthorized().finish();
    }
    // 触发发现:探测可信 peer 的开放库清单并返回。
    let peers = peer_libs().await;
    HttpResponse::Ok().json(json!({ "ok": true, "peers": peers, "ts": now_secs() }))
}

/// 联邦库分页:拉对端清单后按 sort 排序、切片。band 由 peer_id 哈希得到,和侧栏一致。
async fn peer_list_page(lib: &str, sort: &str, offset: usize, limit: usize) -> (usize, Vec<Doc>) {
    let entries = match crate::commands::p2psync::federation_list_files(lib).await {
        Ok(e) => e,
        Err(_) => return (0, Vec::new()),
    };
    let peer_id = lib.strip_prefix("peer:").and_then(|s| s.split_once(':').map(|(p, _)| p.to_string())).unwrap_or_default();
    let band = peer_band_hex(&peer_id);
    let mut docs: Vec<Doc> = entries
        .into_iter()
        .map(|e| {
            let name = e.rel.rsplit('/').next().unwrap_or(&e.rel).to_string();
            let ext = name.rsplit_once('.').map(|(_, x)| x.to_string()).unwrap_or_default();
            let mut d = Doc::basic(e.rel.clone(), name, ext, e.size, e.mtime_ms / 1000, lib.to_string());
            d.source = Some(peer_id.clone());
            d.band = Some(band.clone());
            d
        })
        .collect();
    match sort {
        "name" => docs.sort_by(|a, b| a.name.cmp(&b.name)),
        "size" => docs.sort_by(|a, b| b.size.cmp(&a.size)),
        _ => docs.sort_by(|a, b| b.mtime.cmp(&a.mtime)),
    }
    let total = docs.len();
    let end = offset.saturating_add(limit).min(total);
    let slice = if offset < total { docs[offset..end].to_vec() } else { Vec::new() };
    (total, slice)
}

/// 与 p2psync::peer_band 同款调色板,前后端一致地按 peer_id 上色。
fn peer_band_hex(peer_id: &str) -> String {
    const PALETTE: [&str; 6] = ["#22863a", "#a626a4", "#e45649", "#0969da", "#986801", "#007c89"];
    let h = blake3::hash(peer_id.as_bytes());
    let idx = (h.as_bytes()[0] as usize) % PALETTE.len();
    PALETTE[idx].to_string()
}
// —— 路由注册 ——

async fn redirect_slash() -> HttpResponse {
    HttpResponse::MovedPermanently().insert_header(("Location", "/lib/")).finish()
}

fn register_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/lib", web::get().to(redirect_slash))
        .route("/lib/", web::get().to(serve_index))
        .route("/lib/index.html", web::get().to(serve_index))
        .route("/lib/styles.css", web::get().to(serve_css))
        .route("/lib/app.js", web::get().to(serve_js))
        .route("/lib/api/libraries", web::get().to(api_libraries))
        .route("/lib/api/list", web::get().to(api_list))
        .route("/lib/api/search", web::get().to(api_search))
        .route("/lib/api/file", web::get().to(api_file))
        .route("/lib/api/discover", web::get().to(api_discover));
}

/// 挂到 8888 loopback App:Guaxi 内嵌 webview 访问,无需 token。
pub fn config_local(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::Data::new(SiteAuth { require_token: false }));
    register_routes(cfg);
}

/// 挂到对外 LAN 端口:每个请求校验 token。SharedState 由调用方一并注入。
pub fn config_lan(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::Data::new(SiteAuth { require_token: true }));
    register_routes(cfg);
}

/// Guaxi 内嵌 webview 用的本机地址。
#[tauri::command]
pub fn library_site_local_url() -> String {
    "http://127.0.0.1:8888/lib/".to_string()
}

/// 库网站独立窗口的 label。
const LIBRARY_SITE_WINDOW: &str = "library-site";

/// 打开"资料库 · 分享与发布"独立窗口(承载库网站 iframe + 分享抽屉)。
/// 已存在则聚焦。窗口权限在 capabilities/default.json 的 windows 数组里授予。
#[tauri::command]
pub async fn open_library_site_window(app: tauri::AppHandle, theme: Option<String>) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    if let Some(w) = app.get_webview_window(LIBRARY_SITE_WINDOW) {
        let _ = w.set_focus();
        return Ok(());
    }
    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let qs = if theme == "dark" || theme == "light" {
        format!("?theme={}", theme)
    } else {
        String::new()
    };
    let url = WebviewUrl::App(format!("/library-site{}", qs).into());
    WebviewWindowBuilder::new(&app, LIBRARY_SITE_WINDOW, url)
        .title("资料库 · 分享与发布")
        .inner_size(1200.0, 820.0)
        .min_inner_size(960.0, 640.0)
        .build()
        .map_err(|e| format!("创建库网站窗口失败: {}", e))?;
    Ok(())
}

// —— 对外 LAN 分享:带 token 的独立端口(微信/QQ/浏览器/二维码) ——

const SHARE_PORT: u16 = 8899;

struct ShareServer {
    handle: actix_web::dev::ServerHandle,
    url: String,
    token: String,
}
static SHARE_SERVER: Lazy<Mutex<Option<ShareServer>>> = Lazy::new(|| Mutex::new(None));

/// 探测本机 LAN IP:向公网地址"连接"一个 UDP 套接字(不实际发包),读回本地址。
fn local_ip() -> String {
    use std::net::UdpSocket;
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn gen_token() -> String {
    let bytes: Vec<u8> = (0..24).map(|_| rand::random::<u8>()).collect();
    hex::encode(bytes)
}

#[derive(Serialize)]
pub struct ShareStatus {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

/// 开启对外分享:起 LAN 端口 + 生成 token + 注入 SharedState(供检索用 AppHandle)。
#[tauri::command]
pub async fn library_site_start_share(app: tauri::AppHandle) -> Result<ShareStatus, String> {
    // 已开启则直接返回当前状态(幂等)。
    if let Some(s) = SHARE_SERVER.lock().unwrap().as_ref() {
        return Ok(ShareStatus { enabled: true, url: Some(s.url.clone()), token: Some(s.token.clone()) });
    }
    let token = gen_token();
    set_share_token(Some(token.clone()));

    // 为 LAN App 准备一个带 AppHandle 的 SharedState,让 /lib/api/search 能调 fulltext。
    let shared = SharedState::new();
    shared.set_app_handle(app);

    let srv = actix_web::HttpServer::new(move || {
        let cors = actix_cors::Cors::default().allow_any_origin().allowed_methods(vec!["GET"]).max_age(3600);
        actix_web::App::new()
            .app_data(web::Data::new(shared.clone()))
            .wrap(cors)
            .configure(config_lan)
    })
    .bind(("0.0.0.0", SHARE_PORT))
    .map_err(|e| format!("绑定分享端口 {} 失败: {}", SHARE_PORT, e))?
    .run();

    let handle = srv.handle();
    tokio::spawn(srv);

    let url = format!("http://{}:{}/lib/?t={}", local_ip(), SHARE_PORT, token);
    *SHARE_SERVER.lock().unwrap() = Some(ShareServer { handle, url: url.clone(), token: token.clone() });
    tracing::info!("[library_site] 对外分享已开启: {}", url);
    Ok(ShareStatus { enabled: true, url: Some(url), token: Some(token) })
}

/// 关闭对外分享:停端口 + 撤销 token(立即失效,已发出的链接作废)。
#[tauri::command]
pub async fn library_site_stop_share() -> Result<ShareStatus, String> {
    let server = SHARE_SERVER.lock().unwrap().take();
    if let Some(s) = server {
        s.handle.stop(false).await;
    }
    set_share_token(None);
    Ok(ShareStatus { enabled: false, url: None, token: None })
}

#[tauri::command]
pub fn library_site_share_status() -> ShareStatus {
    match SHARE_SERVER.lock().unwrap().as_ref() {
        Some(s) => ShareStatus { enabled: true, url: Some(s.url.clone()), token: Some(s.token.clone()) },
        None => ShareStatus { enabled: false, url: None, token: None },
    }
}

/// 把给定文本(通常是分享链接)编成二维码 SVG,前端直接内联渲染。
#[tauri::command]
pub fn library_site_qr_svg(text: String) -> Result<String, String> {
    use qrcode::render::svg;
    use qrcode::{EcLevel, QrCode};
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|e| format!("二维码生成失败: {}", e))?;
    let image = code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .dark_color(svg::Color("#1f2328"))
        .light_color(svg::Color("#ffffff00"))
        .build();
    Ok(image)
}

#[derive(Serialize)]
pub struct FileShareLinks {
    pub enabled: bool,
    /// 浏览 / 内联打开(图片、PDF 等可直接在浏览器查看)。
    pub view_url: String,
    /// 强制下载(Content-Disposition: attachment)。
    pub download_url: String,
    /// 整库分享站点首页(带 token),作为"顺带浏览整库"的入口。
    pub site_url: String,
}

/// 为单个文件库条目生成「局域网直达链接」(浏览 + 下载),供右键分享 / 微信·QQ 扫码。
/// 需要时自动开启对外分享(幂等);返回链接内嵌当前 token,一旦关闭分享即失效。
/// 磁盘相对路径方案与 `build_repo_doc` 严格一致:<library>/<stem>/<type_dir>/<file>。
#[tauri::command]
pub async fn library_site_file_share_link(
    app: tauri::AppHandle,
    library: String,
    file_name: String,
    file_type: String,
) -> Result<FileShareLinks, String> {
    let status = library_site_start_share(app).await?;
    let token = status.token.unwrap_or_default();
    let site_url = status.url.unwrap_or_default();
    let stem = file_stem_of(&file_name);
    let rel = match map_type_dir(&file_type) {
        Some(td) => format!("{}/{}/{}/{}", library, stem, td, file_name),
        None => format!("{}/{}/{}", library, stem, file_name),
    };
    let enc = urlencoding::encode(&rel);
    let origin = format!("http://{}:{}", local_ip(), SHARE_PORT);
    let view_url = format!("{}/lib/api/file?lib=repo&path={}&t={}", origin, enc, token);
    let download_url = format!("{}&dl=1", view_url);
    Ok(FileShareLinks { enabled: true, view_url, download_url, site_url })
}


