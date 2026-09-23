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
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::state::SharedState;

const INDEX_HTML: &str = include_str!("../../library_site/index.html");
const STYLES_CSS: &str = include_str!("../../library_site/styles.css");
const APP_JS: &str = include_str!("../../library_site/app.js");

/// 缓存 TTL:超过则下次请求重新遍历库目录(捕捉新增/删除的文件)。
const LISTING_TTL_SECS: u64 = 30;
/// 单库遍历上限,兜底防止异常大目录拖垮内存。
const MAX_ENTRIES: usize = 500_000;

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
}

struct Cached {
    docs: Vec<Doc>,
    sort: String,
    built: Instant,
}

static LISTINGS: Lazy<Mutex<HashMap<String, Cached>>> = Lazy::new(|| Mutex::new(HashMap::new()));
// —— 库枚举与路径解析 ——

/// 两个根:知识库 与 文件库。库 id 形如 `kb` / `repo`(根本身),
/// 或 `kb::子目录名` / `repo::子目录名`(根下的直接子目录)。
fn roots() -> Vec<(&'static str, String, &'static str)> {
    // (kind, 绝对根路径, 展示色带)
    vec![
        ("kb", crate::commands::settingview::get_knowledge_bases_root(), "#1890ff"),
        ("repo", crate::commands::settingview::get_libraries_root(), "#c07a2b"),
    ]
}

fn root_for_kind(kind: &str) -> Option<(String, &'static str)> {
    roots()
        .into_iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, p, band)| (p, band))
}

/// 库 id → (绝对根路径, 色带)。非法 id 返回 None。
fn resolve_lib(id: &str) -> Option<(PathBuf, &'static str)> {
    let (kind, sub) = match id.split_once("::") {
        Some((k, s)) => (k, Some(s)),
        None => (id, None),
    };
    let (root, band) = root_for_kind(kind)?;
    let mut base = PathBuf::from(root);
    if let Some(sub) = sub {
        // 只允许单层子目录名,拒绝分隔符与 .. ,防路径穿越。
        if sub.is_empty() || sub.contains(['/', '\\']) || sub.contains("..") {
            return None;
        }
        base = base.join(sub);
    }
    Some((base, band))
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

/// 枚举本机库:每个根 + 其下直接子目录各算一个库。
fn enumerate_local() -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (kind, root, band) in roots() {
        let root_path = PathBuf::from(&root);
        if !root_path.is_dir() {
            continue;
        }
        let root_name = if kind == "kb" { "知识库" } else { "文件库" };
        out.push(json!({ "id": kind, "name": root_name, "band": band, "count": dir_file_count(&root_path) }));
        if let Ok(rd) = std::fs::read_dir(&root_path) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    let sub = e.file_name().to_string_lossy().to_string();
                    out.push(json!({
                        "id": format!("{}::{}", kind, sub),
                        "name": sub,
                        "band": band,
                        "count": dir_file_count(&e.path()),
                    }));
                }
            }
        }
    }
    out
}

/// 粗略统计目录下文件数(浅层,给侧栏一个量级即可,不深遍历)。
fn dir_file_count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter(|e| e.path().is_file()).count())
        .unwrap_or(0)
}
// —— 浏览列表:遍历库目录 + 每库缓存 ——

fn build_listing(lib: &str, base: &Path, band: &'static str) -> Vec<Doc> {
    let mut docs = Vec::new();
    let walker = ignore::WalkBuilder::new(base)
        .hidden(true)
        .git_ignore(false)
        .build();
    for entry in walker.flatten() {
        if docs.len() >= MAX_ENTRIES {
            break;
        }
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };
        let rel = path.strip_prefix(base).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        docs.push(Doc {
            path: rel,
            name,
            ext,
            size: meta.len(),
            mtime,
            lib: lib.to_string(),
            snippet: None,
            source: None,
            band: Some(band.to_string()),
        });
    }
    docs
}

fn sort_docs(docs: &mut [Doc], sort: &str) {
    match sort {
        "name" => docs.sort_by(|a, b| a.name.cmp(&b.name)),
        "size" => docs.sort_by(|a, b| b.size.cmp(&a.size)),
        _ => docs.sort_by(|a, b| b.mtime.cmp(&a.mtime)), // mtime 默认最近在前
    }
}

/// 取某库的分页切片。命中缓存(未过期且排序一致)直接切片;否则重建/重排。
fn list_page(lib: &str, sort: &str, offset: usize, limit: usize) -> (usize, Vec<Doc>) {
    let Some((base, band)) = resolve_lib(lib) else {
        return (0, Vec::new());
    };
    if !base.is_dir() {
        return (0, Vec::new());
    }
    let mut guard = LISTINGS.lock().unwrap();
    let stale = guard
        .get(lib)
        .map(|c| c.built.elapsed().as_secs() > LISTING_TTL_SECS)
        .unwrap_or(true);
    if stale {
        let mut docs = build_listing(lib, &base, band);
        sort_docs(&mut docs, sort);
        guard.insert(lib.to_string(), Cached { docs, sort: sort.to_string(), built: Instant::now() });
    } else if guard.get(lib).map(|c| c.sort != sort).unwrap_or(false) {
        if let Some(c) = guard.get_mut(lib) {
            sort_docs(&mut c.docs, sort);
            c.sort = sort.to_string();
        }
    }
    let cached = guard.get(lib).unwrap();
    let total = cached.docs.len();
    let end = offset.saturating_add(limit).min(total);
    let slice = if offset < total { cached.docs[offset..end].to_vec() } else { Vec::new() };
    (total, slice)
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
    let lib = q.get("lib").cloned().unwrap_or_else(|| "kb".into());
    let sort = q.get("sort").cloned().unwrap_or_else(|| "mtime".into());
    let offset = q.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let limit = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(200usize).min(1000);
    // 联邦库(其他设备):走 p2p 拉清单,分页切片。
    if lib.starts_with("peer:") {
        let (total, items) = peer_list_page(&lib, &sort, offset, limit).await;
        return HttpResponse::Ok().json(json!({ "total": total, "items": items }));
    }
    let (total, items) = web::block(move || list_page(&lib, &sort, offset, limit))
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
    let lib = q.get("lib").cloned().unwrap_or_else(|| "kb".into());
    let offset = q.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0usize);
    let limit = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(200usize).min(1000);
    // 联邦库:跨设备正文检索(对端 tantivy)。
    if lib.starts_with("peer:") {
        let band = peer_band_hex(lib.strip_prefix("peer:").and_then(|s| s.split_once(':').map(|(p, _)| p)).unwrap_or(""));
        return match crate::commands::p2psync::federation_search(&lib, &pattern, offset, limit).await {
            Ok((hits, has_more)) => {
                let items: Vec<Doc> = hits
                    .into_iter()
                    .map(|h| {
                        let ext = h.name.rsplit_once('.').map(|(_, x)| x.to_string()).unwrap_or_default();
                        Doc {
                            path: h.rel,
                            name: h.name,
                            ext,
                            size: 0,
                            mtime: h.mtime_ms / 1000,
                            lib: lib.clone(),
                            snippet: h.snippet,
                            source: None,
                            band: Some(band.clone()),
                        }
                    })
                    .collect();
                let total = offset + items.len() + if has_more { limit } else { 0 };
                HttpResponse::Ok().json(json!({ "total": total, "items": items }))
            }
            Err(e) => HttpResponse::Ok().json(json!({ "total": 0, "items": [], "error": e })),
        };
    }
    let Some((base, band)) = resolve_lib(&lib) else {
        return HttpResponse::Ok().json(json!({ "total": 0, "items": [] }));
    };
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
            let items: Vec<Doc> = hits
                .into_iter()
                .map(|h| {
                    let abs = PathBuf::from(&h.path);
                    let rel = abs.strip_prefix(&base).unwrap_or(&abs).to_string_lossy().replace('\\', "/");
                    let ext = abs.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    Doc {
                        path: rel,
                        name: h.file_name,
                        ext,
                        size: 0,
                        mtime: h.modified_ms.map(|ms| ms / 1000).unwrap_or(0),
                        lib: lib.clone(),
                        snippet: h.preview,
                        source: None,
                        band: Some(band.to_string()),
                    }
                })
                .collect();
            // fulltext 不回总数;用 offset+本页+是否还有更多推一个够前端滚动的总数。
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
    // 联邦文件:按需从对端拉取到内存后返回,不落地。
    if lib.starts_with("peer:") {
        return match crate::commands::p2psync::federation_pull_file(&lib, &rel).await {
            Ok(bytes) => {
                let mime = mime_guess::from_path(&rel).first_or_octet_stream();
                HttpResponse::Ok().content_type(mime.as_ref()).body(bytes)
            }
            Err(e) => HttpResponse::NotFound().body(format!("联邦拉取失败: {}", e)),
        };
    }
    let Some((base, _)) = resolve_lib(&lib) else {
        return HttpResponse::BadRequest().body("非法库");
    };
    let Some(full) = safe_join(&base, &rel) else {
        return HttpResponse::Forbidden().body("路径越界");
    };
    match std::fs::read(&full) {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&full).first_or_octet_stream();
            HttpResponse::Ok().content_type(mime.as_ref()).body(bytes)
        }
        Err(e) => HttpResponse::NotFound().body(format!("读取失败: {}", e)),
    }
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
            Doc {
                path: e.rel.clone(),
                name,
                ext,
                size: e.size,
                mtime: e.mtime_ms / 1000,
                lib: lib.to_string(),
                snippet: None,
                source: Some(peer_id.clone()),
                band: Some(band.clone()),
            }
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






