use crate::commands::settingview::{get_image_bed_root, get_knowledge_bases_root, get_libraries_root};
use crate::commands::repoview::{ResourceInput, add_resource};
use actix_web::{get, post, web, HttpResponse};
use mime_guess;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tracing::{info, warn};
use urlencoding::{decode, encode};

/// 预处理阶段已经解析成功的图片文件，以及这些图片所在的目录。
///
/// 背景：`serve_image` 原先只放行图床 / 库 / 知识库这三个根目录下的文件，于是从这三个目录
/// 之外打开的笔记（例如 `E:\Work\training\note.md` 里的 `![](./images/a.png)`）虽然能正确
/// 解析出磁盘路径、也能拼出 served_path，但浏览器真去取图时会被 403 挡掉 —— 表现就是
/// "路径明明存在，图片却渲染不出来"。
///
/// 这里不放开整个磁盘，只登记"Markdown 预处理确实解析出来、并且已经写进返回内容里的"图片：
/// - `files`：精确到单个文件，覆盖指向任意位置的绝对路径图片；
/// - `dirs`：这些图片所在的目录。同一篇笔记的插图基本都堆在同一个 images 目录里，登记目录后
///   即使某张图是后来加的、或本轮预处理漏了（前端分片渲染 + 渲染缓存命中时并不会重新请求
///   预处理接口），也不会再出现"同目录下有的图能显示、有的 403"这种随机现象。
struct ResolvedImageRegistry {
    files: HashSet<PathBuf>,
    dirs: HashSet<PathBuf>,
    /// 第几次"整表清空"。每清一次加一，前端据此判断手里的渲染缓存是否已经过期。
    ///
    /// 为什么要这个：`serve_image` 的放行依据有两层 —— 三个根目录，以及这张表。
    /// 笔记放在三个根之外时全靠后者。而表会在两种情况下丢掉已有登记：达到上限整体清空、
    /// 进程重启。此时前端**已经渲染并缓存**的 HTML 里还留着旧 URL，缓存命中就不会再请求
    /// 预处理接口，于是图片集体 403，要等用户手动重试或改一次文档才恢复。
    /// 把代次带出去，前端把它并进渲染缓存的 key，表一清空缓存就自然失效、自动重新预处理。
    epoch: u64,
}

fn resolved_image_registry() -> &'static Mutex<ResolvedImageRegistry> {
    static REGISTRY: OnceLock<Mutex<ResolvedImageRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(ResolvedImageRegistry {
            files: HashSet::new(),
            dirs: HashSet::new(),
            epoch: 0,
        })
    })
}

/// 单次会话里登记的条目上限。超过就整体清空重来：已渲染页面最多触发一次重新预处理，
/// 不会让集合无上限地涨下去。
///
/// 可用环境变量 `GUAXI_IMAGE_REGISTRY_LIMIT` 调小，仅用于验证「清空后渲染缓存是否跟着
/// 失效」这条链路 —— 正常运行时走 20000。
fn registry_limit() -> usize {
    std::env::var("GUAXI_IMAGE_REGISTRY_LIMIT")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or(RESOLVED_IMAGE_REGISTRY_LIMIT)
}

const RESOLVED_IMAGE_REGISTRY_LIMIT: usize = 20_000;

fn remember_resolved_image(path: &Path) {
    let Ok(mut registry) = resolved_image_registry().lock() else {
        return;
    };
    let limit = registry_limit();
    if registry.files.len() >= limit {
        warn!(
            limit,
            "resolved image registry is full, clearing"
        );
        registry.files.clear();
        registry.dirs.clear();
        registry.epoch = registry.epoch.wrapping_add(1);
    }
    if let Some(dir) = path.parent() {
        registry.dirs.insert(dir.to_path_buf());
    }
    registry.files.insert(path.to_path_buf());
}

/// 返回 (是否命中, 已登记文件数, 已登记目录数)，后两个只用于排查日志。
fn lookup_resolved_image(path: &Path) -> (bool, usize, usize) {
    let Ok(registry) = resolved_image_registry().lock() else {
        return (false, 0, 0);
    };
    let hit = registry.files.contains(path)
        || path
            .parent()
            .map(|dir| registry.dirs.contains(dir))
            .unwrap_or(false);
    (hit, registry.files.len(), registry.dirs.len())
}

fn normalize_list_display_math(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut in_fence = false;
    let mut fence_marker: Option<char> = None;
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        let fence = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };

        if let Some(marker) = fence {
            if !in_fence {
                in_fence = true;
                fence_marker = Some(marker);
            } else if fence_marker == Some(marker) {
                in_fence = false;
                fence_marker = None;
            }
            out.push(line.to_string());
            i += 1;
            continue;
        }

        if in_fence {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let open_is_math = line.trim() == "$$";
        if !open_is_math {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let mut close_at: Option<usize> = None;
        let mut j = i + 1;
        while j < lines.len() {
            let inner = lines[j];
            let inner_trim = inner.trim_start();
            if inner.trim() == "$$" {
                close_at = Some(j);
                break;
            }
            if inner_trim.starts_with("```") || inner_trim.starts_with("~~~") {
                break;
            }
            j += 1;
        }

        let Some(close_idx) = close_at else {
            out.push(line.to_string());
            i += 1;
            continue;
        };

        let prev_non_empty = (0..i)
            .rev()
            .find_map(|k| {
                let s = lines[k];
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .unwrap_or("");

        let indent_len = line.len() - line.trim_start().len();
        let list_context = {
            let prev_trim = prev_non_empty.trim_start();
            let is_list_marker = prev_trim.starts_with("- ")
                || prev_trim.starts_with("* ")
                || prev_trim.starts_with("+ ")
                || prev_trim
                    .split_once('.')
                    .map(|(left, right)| {
                        !left.is_empty()
                            && left.chars().all(|c| c.is_ascii_digit())
                            && right.starts_with(' ')
                    })
                    .unwrap_or(false);
            is_list_marker || prev_non_empty.starts_with(' ') || indent_len >= 2
        };

        if !list_context {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        let expr = lines[(i + 1)..close_idx]
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if expr.is_empty() {
            out.push(line.to_string());
            out.push(lines[close_idx].to_string());
            i = close_idx + 1;
            continue;
        }

        let indent = " ".repeat(indent_len);
        out.push(format!("{indent}$$ {expr} $$"));
        i = close_idx + 1;
    }

    out.join("\n")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkdownRequest {
    content: String,
    base_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMapping {
    original_path: String,
    served_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceRegisterRequest {
    resource_type: Option<String>,
    file_name: Option<String>,
    relative_path: Option<String>,
    absolute_path: Option<String>,
    owner_file_path: String,
    created_at: Option<String>,
    extra: Option<serde_json::Value>,
}

fn remote_proxy_url(url: &str) -> String {
    format!(
        "http://localhost:8888/markdown/api/remote_image?url={}",
        encode(url)
    )
}

/// Tauri asset 协议地址（前端 `convertFileSrc()` 的产物）。
///
/// 阅读器在渲染前会把相对路径图片转成 asset 地址，让 WebView 通过 assetProtocol
/// 直接读盘。这类地址**不是外链**：`asset.localhost` 解析到回环地址，交给远程图片
/// 代理只会被 SSRF 防护拒掉（"拒绝代理内网或本机地址"），整张图退化成破图占位。
/// 各阅读器的 Markdown 图片长期显示「图片无法加载」就是这个原因。
///
/// 原样保留即可：CSP 的 img-src 与 tauri.conf.json 的 assetProtocol scope 都已放行。
fn is_local_asset_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://asset.localhost/")
        || lower.starts_with("https://asset.localhost/")
        || lower.starts_with("asset://localhost/")
        || lower.starts_with("asset://")
}

/// 把本地图片路径解析成 `serve_image` 能放行并服务的 URL。
/// 解析失败（没有 base_path、文件不存在）返回 None，由调用方决定降级方式。
fn resolve_local_image_url(img_path: &str, base_path: &Path) -> Option<String> {
    // 没有 base_path 时 join 出来的是相对路径，canonicalize 会悄悄按进程工作目录解析，
    // 有可能命中一个完全无关的同名文件；这里直接判定为"无从解析"。
    if base_path.as_os_str().is_empty() {
        return None;
    }

    let normalized_path = img_path.replace('\\', "/");
    served_url_for_file(&base_path.join(&normalized_path))
}

/// 把一个确定存在的文件登记为可服务，并返回 `serve_image` 的 URL。
///
/// 抽出来是为了让「破图重试」复用同一套登记逻辑：重试时文件可能是刚刚才被补回来的，
/// 走这条路径才会补上 `ResolvedImageRegistry` 的授权，否则浏览器取图仍然 403。
fn served_url_for_file(full_path: &Path) -> Option<String> {
    let canonical = full_path.canonicalize().ok()?;
    if !canonical.is_file() {
        return None;
    }

    // 登记这个文件及其所在目录，让 serve_image 放行（详见 ResolvedImageRegistry 的说明）。
    remember_resolved_image(&canonical);

    let abs = canonical
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .trim_start_matches('/')
        .replace('\\', "/");
    Some(format!(
        "http://localhost:8888/markdown/api/images/{}",
        encode(&abs)
    ))
}

/// Markdown 里的原始 HTML 图片（作者头像、封面在转换产物里常是 `<img>`）。
/// http(s) 改走本地代理，能解析出来的本地相对路径改走 serve_image，
/// 其余保持原样；返回改写后的内容与映射。
fn proxy_html_images(content: &str, base_path: &Path, mappings: &mut Vec<ImageMapping>) -> String {
    let tag_re = Regex::new(r"(?is)<img\b[^>]*>").unwrap();
    // 匹配任意 src（不再限定 http），以便识别并处理相对路径。
    let src_re = Regex::new(r#"(?i)\bsrc\s*=\s*(["'])([^"']*)["']"#).unwrap();
    let alt_re = Regex::new(r#"(?is)\balt\s*=\s*(["'])(.*?)["']"#).unwrap();
    tag_re
        .replace_all(content, |caps: &regex::Captures| {
            let tag = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
            let Some(src_caps) = src_re.captures(&tag) else {
                return tag;
            };
            let quote = src_caps.get(1).map(|m| m.as_str()).unwrap_or("\"");
            let url = src_caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if url.is_empty() {
                return tag;
            }
            // 已经是本地服务、本地 asset 协议或内联数据，原样保留。
            if url.starts_with("http://localhost:8888/")
                || url.starts_with("data:")
                || url.starts_with("blob:")
                || is_local_asset_url(url)
            {
                return tag;
            }
            if url.starts_with("http://") || url.starts_with("https://") {
                let served = remote_proxy_url(url);
                mappings.push(ImageMapping {
                    original_path: url.to_string(),
                    served_path: served.clone(),
                });
                return src_re
                    .replace_all(&tag, format!("src={quote}{served}{quote}"))
                    .to_string();
            }
            // 相对路径：Markdown 图片语法之外，HTML `<img>` 里也会出现。
            // 能解析出来就照常服务；解析不出来整段换成内联占位图——否则
            // 浏览器会去请求 WebView origin 下的不存在文件（net::ERR_CONNECTION_CLOSED）。
            if let Some(served) = resolve_local_image_url(url, base_path) {
                info!(src = url, served = %served, "resolved relative <img src>");
                mappings.push(ImageMapping {
                    original_path: url.to_string(),
                    served_path: served.clone(),
                });
                return src_re
                    .replace_all(&tag, format!("src={quote}{served}{quote}"))
                    .to_string();
            }
            warn!(
                src = url,
                "relative <img src> cannot be resolved; replaced with placeholder"
            );
            let alt = alt_re
                .captures(&tag)
                .and_then(|m| m.get(2).map(|v| v.as_str().to_string()))
                .unwrap_or_default();
            mappings.push(ImageMapping {
                original_path: url.to_string(),
                // 没有可服务的地址：占位图是内联 SVG，调用方据此跳过二次替换。
                served_path: String::new(),
            });
            broken_image_placeholder(&alt, url, base_path)
        })
        .to_string()
}

fn process_image_paths(content: &str, base_path: &Path) -> (String, Vec<ImageMapping>) {
    let mut mappings = Vec::new();
    // 只匹配图片语法，替换也只落在这一整段匹配内。
    // 旧实现对 `](path)` 做全局字符串替换，正文里的普通链接 `[说明](x.png)`
    // 也会被一起改写成图片占位图，整条链接退化成源码文本。
    let re = Regex::new(r#"!\[([^\]]*)\]\(([^)\s]+)((?:\s+"[^"]*")?)\)"#).unwrap();

    let processed = re
        .replace_all(content, |caps: &regex::Captures| {
            let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let img_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            // 可选的标题（含前导空白），拼回去时必须保留。
            let title = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let keep = || caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();

            // 已经是本地服务、本地 asset 协议或内联数据，原样保留。
            if img_path.starts_with("data:")
                || img_path.starts_with("blob:")
                || img_path.starts_with("http://localhost:8888/")
                || is_local_asset_url(img_path)
            {
                return keep();
            }

            if img_path.starts_with("http://") || img_path.starts_with("https://") {
                // 外链图片交给本地后端代理，不再让 WebView 直连第三方：
                // 1) 避免 WebView2 跟踪防护对第三方图片存储访问刷 Tracking Prevention 警告；
                // 2) 绕开部分站点的防盗链（后端不像浏览器那样带来源页）；
                // 3) 代理侧落盘缓存，同一张图只取一次。
                let served_path = remote_proxy_url(img_path);
                mappings.push(ImageMapping {
                    original_path: img_path.to_string(),
                    served_path: served_path.clone(),
                });
                return format!("![{alt}]({served_path}{title})");
            }

            if let Some(served_path) = resolve_local_image_url(img_path, base_path) {
                mappings.push(ImageMapping {
                    original_path: img_path.to_string(),
                    served_path: served_path.clone(),
                });
                return format!("![{alt}]({served_path}{title})");
            }

            // 路径解析不出来（通常是 base_path 为空，或文件已不在）。
            //
            // 直接留原样的话，浏览器会拿这个相对路径去请求 WebView 自己的
            // origin——那里根本没有这个文件，请求以 net::ERR_CONNECTION_CLOSED
            // 收场，控制台一片红。这里换成内联 SVG 占位图：不产生任何网络请求，
            // 颜色跟随明暗主题，用户看到的是一块明确的"图片不可用"。
            warn!(
                path = img_path,
                "image path unresolved (missing base_path or file gone); replaced with placeholder"
            );
            mappings.push(ImageMapping {
                original_path: img_path.to_string(),
                // 没有可服务的地址：占位块是内联 HTML，调用方据此跳过二次替换。
                served_path: String::new(),
            });
            broken_image_placeholder(alt, img_path, base_path)
        })
        .to_string();

    (processed, mappings)
}

/// 先处理 Markdown 语法里的图片，再处理原始 HTML 的 `<img>`。
/// 两步都只改写外链；本地路径仍走 serve_image 的授权登记。
fn process_all_images(content: &str, base_path: &Path) -> (String, Vec<ImageMapping>) {
    let (markdown_processed, mut mappings) = process_image_paths(content, base_path);
    let processed = proxy_html_images(&markdown_processed, base_path, &mut mappings);
    (processed, mappings)
}

/// 解析调用方给的基准路径。
///
/// 约定是「目录」，但调用方历史上有的传的是文档**文件**路径（任务/org 预览把
/// `documentPath` 直接传下来）。拿文件路径去 join `./images/x.png` 会拼成
/// `<文件>/images/x.png`，永远解析不到。这里统一归一：是存在的文件就取它所在目录。
fn resolve_base_path(req: &MarkdownRequest) -> PathBuf {
    let path = req
        .base_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_default();
    if path.is_file() {
        if let Some(parent) = path.parent() {
            return parent.to_path_buf();
        }
    }
    path
}

#[post("/api/markdown/image_process")]
async fn process_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = resolve_base_path(&req);
    let (processed, mappings) = process_all_images(&req.content, &base_path);

    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings,
        // 前端把它并进渲染缓存的 key：注册表被清空（达到上限 / 进程重启）后这个值变化，
        // 缓存自动失效并重新走一次预处理，图片重新登记，不必等用户手动重试。
        "registry_epoch": current_registry_epoch(),
    }))
}

#[post("/api/markdown/chat_process")]
async fn process_chat_markdown(req: web::Json<MarkdownRequest>) -> HttpResponse {
    let base_path = resolve_base_path(&req);
    let (processed, mappings) = process_all_images(&req.content, &base_path);
    let processed = normalize_list_display_math(&processed);

    HttpResponse::Ok().json(serde_json::json!({
        "processed_content": processed,
        "mappings": mappings,
        "registry_epoch": current_registry_epoch(),
    }))
}

/// 当前登记表代次。前端据此判断渲染缓存是否还认这张表。
fn current_registry_epoch() -> u64 {
    resolved_image_registry()
        .lock()
        .map(|registry| registry.epoch)
        .unwrap_or(0)
}

/// 给 Tauri 命令用的公开版本（前端在「决定要不要复用渲染缓存」时问一次）。
pub fn registry_epoch() -> u64 {
    current_registry_epoch()
}

#[derive(Debug, Deserialize)]
struct ResolveImageRequest {
    /// 原始路径：绝对路径优先，其次才是 Markdown 里的相对路径。
    path: String,
    #[serde(default)]
    base_path: Option<String>,
}

/// 破图占位块上「重新加载」按钮的后端。
///
/// 为什么不复用 `/api/markdown/image_process`：那条链路要重新处理整篇文档，
/// 前端还有片段缓存、全文缓存、后端 sqlite 缓存三层，重试一张图要么撞缓存拿到旧结果，
/// 要么为了一个文件把所有缓存打穿。这里只解决"这个文件现在能不能服务"，
/// 成功就返回 serve_image 的地址，由前端把占位块就地换成 `<img>`。
#[post("/api/markdown/resolve_image")]
async fn resolve_image(req: web::Json<ResolveImageRequest>) -> HttpResponse {
    let raw = req.path.trim();
    if raw.is_empty() {
        return HttpResponse::Ok().json(serde_json::json!({
            "served_url": serde_json::Value::Null,
            "reason": "empty_path"
        }));
    }

    let candidate = PathBuf::from(raw.replace('\\', "/"));
    let full_path = if candidate.is_absolute() {
        candidate
    } else {
        match req.base_path.as_deref().map(str::trim) {
            Some(base) if !base.is_empty() => PathBuf::from(base).join(&candidate),
            _ => {
                return HttpResponse::Ok().json(serde_json::json!({
                    "served_url": serde_json::Value::Null,
                    "reason": "missing_base_path"
                }));
            }
        }
    };

    match served_url_for_file(&full_path) {
        Some(url) => {
            info!(path = raw, served = %url, "broken image retry resolved");
            HttpResponse::Ok().json(serde_json::json!({
                "served_url": url,
                "reason": serde_json::Value::Null
            }))
        }
        None => HttpResponse::Ok().json(serde_json::json!({
            "served_url": serde_json::Value::Null,
            "reason": "not_found"
        })),
    }
}

#[get("/markdown/api/images/{path:.*}")]
async fn serve_image(path: web::Path<String>) -> HttpResponse {
    let decoded = match decode(&path) {
        Ok(v) => v.into_owned(),
        Err(_) => return HttpResponse::BadRequest().body("Invalid path encoding"),
    };

    let decoded = decoded.trim_start_matches(r"\\?\").to_string();
    let candidate = PathBuf::from(decoded);

    let file_canon = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    if !file_canon.is_file() {
        return HttpResponse::NotFound().finish();
    }

    // 放行条件二选一：
    // 1. 文件在配置的三个根目录（图床 / 库 / 知识库）之下；
    // 2. 文件是 Markdown 预处理阶段解析出来的图片，或位于这类图片所在的目录（笔记可以放在任意目录）。
    let (allowed_by_registry, known_files, known_dirs) = lookup_resolved_image(&file_canon);

    let mut allowed_roots: Vec<PathBuf> = Vec::new();
    for raw in [get_image_bed_root(), get_libraries_root(), get_knowledge_bases_root()] {
        let p = PathBuf::from(raw);
        if let Ok(canon) = p.canonicalize() {
            allowed_roots.push(canon);
        }
    }

    let allowed_by_root = allowed_roots.iter().any(|r| file_canon.starts_with(r));

    if !allowed_by_registry && !allowed_by_root {
        warn!(
            file = file_canon.to_string_lossy().as_ref(),
            roots = allowed_roots.len(),
            known_files,
            known_dirs,
            "serve_image forbidden: file is neither under an allowed root nor a resolved markdown image"
        );
        return HttpResponse::Forbidden().finish();
    }

    info!(
        file = file_canon.to_string_lossy().as_ref(),
        by_registry = allowed_by_registry,
        by_root = allowed_by_root,
        "serve_image allowed"
    );

    match fs::read(&file_canon) {
        Ok(content) => HttpResponse::Ok()
            .content_type(mime_guess::from_path(&file_canon).first_or_octet_stream())
            .body(content),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

#[get("/markdown/api/bed/{rel:.*}")]
async fn serve_bed_image(rel: web::Path<String>) -> HttpResponse {
    let root = PathBuf::from(get_image_bed_root());
    let rel_raw = rel.into_inner();

    let decoded_rel = match decode(&rel_raw) {
        Ok(v) => v.into_owned(),
        Err(_) => return HttpResponse::BadRequest().body("Invalid path encoding"),
    };

    let safe_rel = decoded_rel.replace('\\', "/");
    let candidate = root.join(&safe_rel);

    let root_canon = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let file_canon = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    if !file_canon.starts_with(&root_canon) {
        return HttpResponse::Forbidden().finish();
    }

    match fs::read(&file_canon) {
        Ok(content) => HttpResponse::Ok()
            .content_type(mime_guess::from_path(&file_canon).first_or_octet_stream())
            .body(content),
        Err(_) => HttpResponse::NotFound().finish(),
    }
}

#[post("/markdown/api/resources")]
async fn register_resource(req: web::Json<ResourceRegisterRequest>) -> HttpResponse {
    let root = PathBuf::from(get_image_bed_root());

    let root_canon = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let (relative_path, file_canon) = if let Some(rel) = req.relative_path.as_ref() {
        let rel = rel.replace('\\', "/");
        let candidate = root.join(&rel);
        let file_canon = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => return HttpResponse::BadRequest().body("Invalid resource path"),
        };
        (rel, file_canon)
    } else if let Some(abs) = req.absolute_path.as_ref() {
        let candidate = PathBuf::from(abs);
        let file_canon = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => return HttpResponse::BadRequest().body("Invalid resource path"),
        };

        let rel = match file_canon.strip_prefix(&root_canon) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => return HttpResponse::Forbidden().finish(),
        };

        (rel, file_canon)
    } else {
        return HttpResponse::BadRequest().body("Missing relative_path or absolute_path");
    };

    if !file_canon.starts_with(&root_canon) {
        return HttpResponse::Forbidden().finish();
    }

    let absolute_path = file_canon.to_string_lossy().to_string();

    let file_name = if let Some(v) = req.file_name.as_ref() {
        v.clone()
    } else {
        match file_canon.file_name().and_then(|n| n.to_str()) {
            Some(v) => v.to_string(),
            None => return HttpResponse::BadRequest().body("Invalid file name"),
        }
    };

    let resource_type = if let Some(v) = req.resource_type.as_ref() {
        v.clone()
    } else {
        match Path::new(&file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("webp") | Some("bmp") | Some("svg") => {
                "image".to_string()
            }
            Some("mp4") | Some("mov") | Some("avi") | Some("mkv") | Some("webm") => "video".to_string(),
            Some("pdf")
            | Some("doc")
            | Some("docx")
            | Some("xls")
            | Some("xlsx")
            | Some("ppt")
            | Some("pptx")
            | Some("txt")
            | Some("md")
            | Some("log") => "document".to_string(),
            Some("exe") | Some("dll") | Some("bin") | Some("so") | Some("dylib") => "binary".to_string(),
            _ => "other".to_string(),
        }
    };

    let created_at = req.created_at.clone().unwrap_or_default();
    let extra_str = req.extra.as_ref().map(|v| v.to_string());

    let input = ResourceInput::new(
        resource_type,
        file_name,
        relative_path,
        absolute_path,
        req.owner_file_path.clone(),
        created_at,
        extra_str,
    );

    match add_resource(input) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "success": true })),
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "success": false, "error": e })),
    }
}

// ───────────────────────── 远程图片代理 ─────────────────────────
//
// 只代理图片，且目标不能是回环 / 内网地址：Markdown 来自不可信内容，
// `![](http://127.0.0.1:8080/...)` 不能借代理变成一次 SSRF。
// 缓存落在 exe 同级 cache/remote-images/，按 URL 哈希命名。

/// 无法解析的图片替换成这段内联 HTML 占位块。
///
/// 为什么不是 data URI：markdown-it 的 validateLink 只放行
/// `data:image/(gif|png|jpeg|webp)`，`data:image/svg+xml` 会被判为不安全，
/// 图片/链接语法整段退化成源码文本（就是"预览里出现一长串 base64"的原因）。
/// 内联 HTML 走原始 HTML 通道（预览器 html: true），不受链接校验影响。
///
/// 颜色一律用 `currentColor` + 透明度：浅色主题继承深色文字、暗色主题继承浅色文字，
/// 底色/描边/文字自动跟着明暗主题走，不需要为两套主题各准备一套样式。
///
/// 为什么从裸 `<svg>` 改成 `<span>`：破图同样需要「重新加载 / 复制路径」两个动作，
/// 裸 SVG 里塞不进可点击元素，用户除了看到一块灰图什么也做不了。
///
/// 几个约定：
/// - `data-gx-broken-src` 保留 Markdown 里写的原始路径，供「复制路径」直接取用；
/// - `data-gx-broken-abs` 是拼出来的绝对路径，前端拿它调 `/api/markdown/resolve_image`
///   重试即可，不必为了补一张图重渲染整篇文档（那还会撞上各级渲染缓存）；
/// - 整个块必须写成一行：Markdown 的原始 HTML 通道按行解析，换行会被拆成普通文本。
fn broken_image_placeholder(alt: &str, original: &str, base_path: &Path) -> String {
    let escaped_alt = html_escape_attr(alt);
    let escaped_src = html_escape_attr(original);
    let label = if escaped_alt.trim().is_empty() {
        "图片无法加载"
    } else {
        escaped_alt.as_str()
    };

    // 文件名单独展示一行：多数破图是「路径写错 / 文件被挪走」，看路径比看 alt 更好定位。
    let file_name = original
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let escaped_name = html_escape_attr(if file_name.is_empty() {
        "未命名图片"
    } else {
        file_name.as_str()
    });

    // base_path 为空说明当前文档没有可用基准目录，拼出来的绝对路径没有意义，
    // 重试按钮会退化成"再查一次相对路径"（必然失败），所以干脆不写。
    let abs = if base_path.as_os_str().is_empty() {
        String::new()
    } else {
        base_path
            .join(original.replace('\\', "/"))
            .to_string_lossy()
            .replace('\\', "/")
    };
    let escaped_abs = html_escape_attr(&abs);

    format!(
        "<span class=\"guaxi-broken-image\" data-gx-broken-src=\"{escaped_src}\" \
data-gx-broken-abs=\"{escaped_abs}\" data-gx-broken-alt=\"{escaped_alt}\">\
<span class=\"guaxi-broken-image__icon\" aria-hidden=\"true\">\
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"22\" height=\"22\" viewBox=\"0 0 24 24\" \
fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.6\" stroke-linecap=\"round\" \
stroke-linejoin=\"round\"><rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2.5\"/>\
<circle cx=\"8.8\" cy=\"9.2\" r=\"1.5\"/><path d=\"M3.5 17.2l4.6-4.6 3.4 3.4 2.8-2.8 5.9 5.9\"/>\
<path d=\"M4 20L20 4\"/></svg></span>\
<span class=\"guaxi-broken-image__body\">\
<span class=\"guaxi-broken-image__title\">{label}</span>\
<span class=\"guaxi-broken-image__name\" title=\"{escaped_src}\">{escaped_name}</span></span>\
<span class=\"guaxi-broken-image__actions\">\
<button type=\"button\" class=\"guaxi-broken-image__btn\" data-gx-broken-action=\"retry\">重新加载</button>\
<button type=\"button\" class=\"guaxi-broken-image__btn\" data-gx-broken-action=\"copy\">复制路径</button>\
</span></span>"
    )
}

fn html_escape_attr(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// 远程图片代理失败时的响应体。
///
/// 这是一个真实的 HTTP 图片响应（内容不能是 data URI 字符串本身），
/// 颜色写死为浅色系——它只出现在"外链图片连不上"的降级场景里。
/// 文案与内联占位块保持一致：这里拿不到原路径，也没有交互能力（它是一张位图响应），
/// 所以只能告诉用户"这张外链图取不回来"。
const BROKEN_IMAGE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"320\" \
height=\"200\" viewBox=\"0 0 320 200\"><rect width=\"320\" height=\"200\" fill=\"#f2f2f2\"/>\
<path d=\"M30 30l260 140M290 30L30 170\" stroke=\"#ccc\" stroke-width=\"2\"/>\
<text x=\"160\" y=\"105\" font-family=\"sans-serif\" font-size=\"14\" fill=\"#999\" \
text-anchor=\"middle\">图片无法加载</text></svg>";

const REMOTE_IMAGE_MAX_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct RemoteImageQuery {
    url: String,
}

fn remote_image_cache_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("cache").join("remote-images");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn remote_image_ext(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "image/avif" => Some("avif"),
        "image/bmp" => Some("bmp"),
        _ => None,
    }
}

fn remote_image_content_type(ext: &str) -> &'static str {
    match ext {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

/// 按文件头识别图片类型，作为 Content-Type 的兜底。
///
/// 不少图床/CDN 不按图片类型返回响应头（实测 img.36krcdn.com 对一张 PNG 返回
/// `application/octet-stream`）。只信头部的话，明明是图片的内容会被当成
/// "不是图片类型" 拒掉，最终显示破图占位 —— 这类外链图片「怎么都加载不出来」
/// 就是被这里拦下的。字节已经到了，用魔数判断比相信头部可靠。
fn sniff_image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some("png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    if bytes.starts_with(b"BM") {
        return Some("bmp");
    }
    // AVIF/AVIS：ISO BMFF，第 5..8 字节是 "ftyp"，随后是品牌名。
    if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && (&bytes[8..12] == b"avif" || &bytes[8..12] == b"avis" || &bytes[8..12] == b"mif1")
    {
        return Some("avif");
    }
    let head = &bytes[..bytes.len().min(1024)];
    if let Ok(text) = std::str::from_utf8(head) {
        if text.trim_start().starts_with("<svg") {
            return Some("svg");
        }
    }
    None
}

fn ip_is_forbidden(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

fn url_host_is_forbidden(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return true;
    };
    let host = host.trim_matches(['[', ']']);
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip_is_forbidden(ip);
    }
    let lower = host.to_ascii_lowercase();
    lower == "localhost" || lower.ends_with(".localhost") || lower.ends_with(".local")
}

async fn ensure_public_host(url: &reqwest::Url) -> Result<(), String> {
    let host = url.host_str().ok_or_else(|| "缺少主机名".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "缺少端口".to_string())?;
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("解析域名失败: {e}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        if ip_is_forbidden(addr.ip()) {
            return Err("拒绝代理内网或本机地址".to_string());
        }
    }
    if !any {
        return Err("域名没有解析到地址".to_string());
    }
    Ok(())
}

fn remote_image_cache_key(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 图片代理的 User-Agent。
///
/// 用浏览器 UA 而不是自定义标识：实测不少图片 CDN 见到 `Guaxi/1.0` 这类
/// 非浏览器 UA 直接 403，哪怕 Referer 正确。这不是伪装，图片代理本质就是
/// 替用户浏览器取图。
const IMAGE_PROXY_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0 Safari/537.36";

/// 为图片请求构造 Referer：取图片 URL 的源（scheme + host）。
///
/// 防盗链站点校验的是"这个请求是否来自本站",所以用图片自己的域名做 Referer
/// 是语义上最贴近真实浏览行为的做法，也不需要把文章来源页传进来。
fn referer_for(url: &reqwest::Url) -> String {
    format!(
        "{}://{}/",
        url.scheme(),
        url.host_str().unwrap_or_default()
    )
}

fn read_cached_remote(url: &str) -> Option<(String, Vec<u8>)> {
    let dir = remote_image_cache_dir()?;
    let key = remote_image_cache_key(url);
    for ext in ["jpg", "png", "gif", "webp", "svg", "avif", "bmp"] {
        let path = dir.join(format!("{key}.{ext}"));
        if let Ok(bytes) = fs::read(&path) {
            return Some((remote_image_content_type(ext).to_string(), bytes));
        }
    }
    None
}

async fn fetch_remote_image(url_str: &str) -> Result<(String, Vec<u8>), String> {
    if let Some(cached) = read_cached_remote(url_str) {
        return Ok(cached);
    }
    let url = reqwest::Url::parse(url_str).map_err(|e| format!("无效的图片地址: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只代理 http/https 图片".to_string());
    }
    if url_host_is_forbidden(&url) {
        return Err("拒绝代理内网或本机地址".to_string());
    }
    ensure_public_host(&url).await?;

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 || url_host_is_forbidden(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }));

    // 图片 CDN 常与文章站同域或同区域，不跟随系统代理时，
    // 在大陆会表现为连不上（而不是 403）——同样要把代理接上。
    if let Some(proxy) = crate::commands::mail::net::system_proxy_url() {
        if let Ok(p) = reqwest::Proxy::all(&proxy) {
            // 本地图床/库图片走 8888，不能被代理绕出去。
            builder = builder.proxy(
                p.no_proxy(reqwest::NoProxy::from_string("localhost,127.0.0.1,::1")),
            );
        }
    }

    let client = builder.build().map_err(|e| e.to_string())?;

    let resp = client
        .get(url.clone())
        .header(reqwest::header::USER_AGENT, IMAGE_PROXY_USER_AGENT)
        // 防盗链：不少 CDN（实测少数派 cdnfile.sspai.com）要求 Referer 与图片
        // 所在站点同源，否则一律 403。带上图片域名自身的 Referer 就能通过，
        // 这比伪造搜索引擎来源更保守，也不涉及跨站伪装。
        .header(reqwest::header::REFERER, referer_for(&url))
        .send()
        .await
        .map_err(|e| format!("下载失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("上游返回 {}", resp.status()));
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    // 头部只是优先线索：真正的类型判断放到读完字节之后（见下面的 sniff 兜底）。
    let ext_by_header = remote_image_ext(&content_type);
    if let Some(len) = resp.content_length() {
        if len as usize > REMOTE_IMAGE_MAX_BYTES {
            return Err("图片过大".to_string());
        }
    }
    let bytes = resp.bytes().await.map_err(|e| format!("读取图片失败: {e}"))?;
    if bytes.len() > REMOTE_IMAGE_MAX_BYTES {
        return Err("图片过大".to_string());
    }
    let ext = ext_by_header
        .or_else(|| sniff_image_ext(&bytes))
        .ok_or_else(|| format!("不是图片类型: {content_type}"))?;

    if let Some(dir) = remote_image_cache_dir() {
        let key = remote_image_cache_key(url_str);
        let path = dir.join(format!("{key}.{ext}"));
        let tmp = dir.join(format!("{key}.{ext}.tmp"));
        if fs::write(&tmp, &bytes).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
    Ok((remote_image_content_type(ext).to_string(), bytes.to_vec()))
}

async fn remote_image_response(query: web::Query<RemoteImageQuery>) -> HttpResponse {
    match fetch_remote_image(query.url.trim()).await {
        Ok((content_type, bytes)) => HttpResponse::Ok()
            .insert_header(("Content-Type", content_type))
            .insert_header(("Cache-Control", "public, max-age=604800"))
            .body(bytes),
        Err(error) => {
            warn!(url = %query.url, error = %error, "remote image proxy failed");
            // 失败时返回占位图（200），而不是 502。
            //
            // 这些失败绝大多数是上游图片本身失效——实测 CDC 的旧配图被 Akamai
            // 直接 403，真实浏览器打开也是 404。既然是用户无法处理的内容问题，
            // 就不该让它在 DevTools 里刷红色错误：<img> 收到非 2xx 会留下
            // "GET ... 502 (Bad Gateway)"，而 200 + SVG 只是安静地显示图缺。
            //
            // 5 分钟缓存：避免滚动时对同一张失效图反复请求上游，同时保证
            // 临时性网络故障恢复后能较快重试成功。
            HttpResponse::Ok()
                .content_type("image/svg+xml")
                .insert_header(("Cache-Control", "public, max-age=300"))
                .body(BROKEN_IMAGE_SVG)
        }
    }
}

#[get("/markdown/api/remote_image")]
async fn remote_image(query: web::Query<RemoteImageQuery>) -> HttpResponse {
    remote_image_response(query).await
}

/// 兼容别名：早期版本的代理改写用的是 /api/markdown/remote_image，
/// 已经渲染并缓存的 HTML 可能还留着那个地址。新内容只生成 /markdown/api/…。
#[get("/api/markdown/remote_image")]
async fn remote_image_legacy(query: web::Query<RemoteImageQuery>) -> HttpResponse {
    remote_image_response(query).await
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(process_markdown)
        .service(process_chat_markdown)
        .service(resolve_image)
        .service(serve_image)
        .service(serve_bed_image)
        .service(register_resource)
        .service(remote_image)
        .service(remote_image_legacy);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_relative_image_becomes_placeholder() {
        // 复现线上问题：没有 base_path 时，相对路径图片解析不出来，
        // 原来是"保持原样"，浏览器就会去请求 WebView origin 下的不存在文件
        // （net::ERR_CONNECTION_CLOSED）。现在必须换成内联占位图。
        let (out, mappings) = process_image_paths("![](lark2pad-1793718531-2.jpg)", Path::new(""));
        assert!(
            out.contains("class=\"guaxi-broken-image\""),
            "unresolved relative image must become the inline placeholder, got: {out}"
        );
        // 颜色必须走 currentColor，才能跟随明暗主题；不能退回写死颜色的 data URI。
        assert!(
            out.contains("currentColor") && !out.contains("data:image/"),
            "placeholder must adapt to the theme and avoid data URIs, got: {out}"
        );
        // 原始路径会保留在 data 属性里供「复制路径」使用，但绝不能再出现在图片位置：
        // 只要输出里没有 `<img`，浏览器就不会去请求这个不存在的文件。
        assert!(
            !out.contains("<img"),
            "broken image must not survive as an <img> tag, got: {out}"
        );
        assert!(
            out.contains("data-gx-broken-src=\"lark2pad-1793718531-2.jpg\""),
            "original path must be kept in data attribute for the copy action, got: {out}"
        );
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].original_path, "lark2pad-1793718531-2.jpg");
        assert_eq!(mappings[0].served_path, "");
    }

    #[test]
    fn placeholder_exposes_retry_and_copy_actions() {
        // 占位块必须带两个可点击动作，否则用户只能看着一块灰图。
        let dir = std::env::temp_dir().join("guaxi-note-dir");
        let (out, _) = process_image_paths("![图](images/nope.png)", &dir);

        assert!(
            out.contains("data-gx-broken-action=\"retry\""),
            "placeholder must expose a retry action, got: {out}"
        );
        assert!(
            out.contains("data-gx-broken-action=\"copy\""),
            "placeholder must expose a copy-path action, got: {out}"
        );
        // 有 base_path 时必须给出绝对路径，重试接口才能不依赖文档上下文。
        assert!(
            out.contains("data-gx-broken-abs=\""),
            "placeholder must carry a resolved absolute path when base_path is known, got: {out}"
        );
        assert!(
            out.contains("images/nope.png"),
            "placeholder must show the file name, got: {out}"
        );
        // 中文文案：界面上不该再出现英文 image unavailable。
        assert!(
            out.contains("图片无法加载") && !out.contains("image unavailable"),
            "placeholder copy must be localized, got: {out}"
        );
    }

    #[test]
    fn plain_markdown_links_are_never_rewritten() {
        // 回归：旧实现按 `](path)` 做全局替换，普通链接会被一起改成占位图，
        // 预览里整条链接退化成 `[相对链接](data:...)` 源码文本。
        let content = "[相对链接](../123/bug129217.png)\n\n![image](../123/bug129217.png)";
        let (out, mappings) = process_image_paths(content, Path::new(""));

        assert!(
            out.contains("[相对链接](../123/bug129217.png)"),
            "plain link must stay untouched, got: {out}"
        );
        assert!(
            out.contains("class=\"guaxi-broken-image\""),
            "image destination must become the placeholder, got: {out}"
        );
        assert_eq!(mappings.len(), 1);
    }

    #[test]
    fn placeholder_keeps_alt_and_escapes_it() {
        let (out, _) = process_image_paths("![封面\"图](../nope.png)", Path::new(""));
        assert!(out.contains("data-gx-broken-alt=\"封面&quot;图\""), "got: {out}");
    }

    #[test]
    fn inline_data_images_are_left_alone() {
        let content = "![x](data:image/png;base64,AAA)";
        let (out, mappings) = process_image_paths(content, Path::new(""));
        assert_eq!(out, content);
        assert!(mappings.is_empty());
    }

    #[test]
    fn existing_relative_image_is_served() {
        let dir = std::env::temp_dir().join(format!("guaxi-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ok.png");
        std::fs::write(&file, b"png").unwrap();

        let (out, mappings) = process_image_paths("![a](ok.png)", &dir);
        assert!(
            out.contains("/markdown/api/images/"),
            "existing image must be served, got: {out}"
        );
        assert!(!out.contains("guaxi-broken-image"), "got: {out}");
        // 拼接不能多出括号：`![a](URL)` 恰好一个收尾右括号。
        assert!(out.starts_with("![a](http"), "got: {out}");
        assert!(out.ends_with(".png)"), "got: {out}");
        assert!(!out.contains("))"), "got: {out}");
        assert_eq!(mappings.len(), 1);

        // 图片标题必须原样保留。
        let (out, _) = process_image_paths("![a](ok.png \"标题\")", &dir);
        assert!(out.ends_with("\"标题\")"), "got: {out}");
        assert!(!out.contains("))"), "got: {out}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unresolved_relative_html_img_becomes_placeholder() {
        let mut mappings = Vec::new();
        let out = proxy_html_images(
            r#"<img src="lark2pad-1793718531-2.jpg" alt="x">"#,
            Path::new(""),
            &mut mappings,
        );
        assert!(out.contains("class=\"guaxi-broken-image\""), "got: {out}");
        assert!(out.contains("data-gx-broken-alt=\"x\""), "got: {out}");
        // 原始路径只允许出现在 data 属性里，不能留在 src 上被浏览器请求。
        assert!(!out.contains("<img"), "broken tag must be fully replaced, got: {out}");
    }

    #[test]
    fn existing_relative_html_img_is_served() {
        let dir = std::env::temp_dir().join(format!("guaxi-html-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ok.png"), b"png").unwrap();

        let mut mappings = Vec::new();
        let out = proxy_html_images(r#"<img src="ok.png">"#, &dir, &mut mappings);
        assert!(
            out.contains("/markdown/api/images/"),
            "existing <img src> must be served, got: {out}"
        );
        assert_eq!(mappings.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn inline_and_local_urls_are_left_alone() {
        let mut mappings = Vec::new();
        // data URI 与本地服务地址不该被当作相对路径处理
        let html = r#"<img src="data:image/png;base64,AAA"><img src="http://localhost:8888/x.png">"#;
        let out = proxy_html_images(html, Path::new(""), &mut mappings);
        assert_eq!(out, html);
        assert!(mappings.is_empty());
    }

    #[test]
    fn tauri_asset_urls_are_left_alone() {
        // 阅读器把相对图片转成 asset 协议地址交给 WebView 直读。这类地址不是外链：
        // 交给远程代理会被 SSRF 防护拒掉（asset.localhost 解析到回环地址），
        // 整张图退化成破图占位 —— 各阅读器 Markdown 图片集体「无法加载」的根因。
        let md = "![a](http://asset.localhost/D%3A%5Ctmp%5Ca.png)";
        let (out, mappings) = process_image_paths(md, Path::new(""));
        assert_eq!(out, md);
        assert!(mappings.is_empty());

        let mut mappings = Vec::new();
        let html = r#"<img src="http://asset.localhost/D%3A%5Ctmp%5Ca.png">"#;
        let out = proxy_html_images(html, Path::new(""), &mut mappings);
        assert_eq!(out, html);
        assert!(mappings.is_empty());

        assert!(is_local_asset_url("asset://localhost/tmp/a.png"));
        assert!(is_local_asset_url("https://asset.localhost/tmp/a.png"));
        assert!(!is_local_asset_url("https://example.com/a.png"));
    }

    #[test]
    fn sniffs_image_type_when_header_lies() {
        // 实测 36kr CDN：PNG 内容 + application/octet-stream 响应头。
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0];
        assert_eq!(sniff_image_ext(&png), Some("png"));
        assert_eq!(sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image_ext(b"GIF89a......"), Some("gif"));
        assert_eq!(sniff_image_ext(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(sniff_image_ext(b"\0\0\0\x1cftypavif"), Some("avif"));
        assert_eq!(
            sniff_image_ext(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"),
            Some("svg")
        );
        // HTML 错误页不能当成图片收下。
        assert_eq!(sniff_image_ext(b"<html><body>404</body></html>"), None);
        assert_eq!(sniff_image_ext(b""), None);
    }

    #[test]
    fn base_path_normalizes_document_file_to_its_directory() {
        let dir = std::env::temp_dir().join(format!("guaxi-basepath-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("note.md");
        std::fs::write(&doc, b"# t").unwrap();

        // 传文件路径（任务/org 预览就是这样传的）→ 归一成所在目录。
        let req = MarkdownRequest {
            content: String::new(),
            base_path: Some(doc.to_string_lossy().to_string()),
        };
        assert_eq!(resolve_base_path(&req), dir);

        // 本来就是目录：原样保留。
        let req = MarkdownRequest {
            content: String::new(),
            base_path: Some(dir.to_string_lossy().to_string()),
        };
        assert_eq!(resolve_base_path(&req), dir);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn referer_uses_the_image_origin() {
        // 防盗链站点校验的是"是否来自本站"，所以 Referer 取图片自己的源最合适。
        let u = reqwest::Url::parse(
            "https://cdnfile.sspai.com/2026/10/22/a.jpg?imageView2/2/w/1120",
        )
        .unwrap();
        assert_eq!(referer_for(&u), "https://cdnfile.sspai.com/");

        let u2 = reqwest::Url::parse("http://example.com/img.png").unwrap();
        assert_eq!(referer_for(&u2), "http://example.com/");
    }

    #[test]
    fn proxies_markdown_remote_images() {
        let (out, mappings) =
            process_image_paths("![](https://example.com/a.png)", Path::new(""));
        assert!(out.contains("/markdown/api/remote_image?url="), "out={out}");
        assert!(out.contains("https%3A%2F%2Fexample.com%2Fa.png"), "out={out}");
        assert_eq!(mappings.len(), 1);
    }

    #[test]
    fn proxies_html_img_src() {
        let mut mappings = Vec::new();
        let out = proxy_html_images(
            r#"<p><img src="https://example.com/a.png" alt="x"></p>"#,
            Path::new(""),
            &mut mappings,
        );
        assert!(out.contains("/markdown/api/remote_image?url="), "out={out}");
        assert!(!out.contains(r#"src="https://example.com"#), "out={out}");
        assert_eq!(mappings.len(), 1);
    }

    #[test]
    fn forbids_private_hosts() {
        for entry in [
            "http://127.0.0.1/x",
            "http://localhost/x",
            "http://192.168.1.2/x",
            "http://10.0.0.1/x",
            "http://[::1]/x",
        ] {
            let url = reqwest::Url::parse(entry).unwrap();
            assert!(url_host_is_forbidden(&url), "{entry} 应被拒绝");
        }
        let public = reqwest::Url::parse("https://example.com/x").unwrap();
        assert!(!url_host_is_forbidden(&public));
    }
}
