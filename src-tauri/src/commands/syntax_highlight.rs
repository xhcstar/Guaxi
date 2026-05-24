use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use lru::LruCache;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tree_sitter::{Language, Parser};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use tracing::{debug, info, warn};

const HIGHLIGHT_CACHE_CAPACITY: usize = 1024;
const MAX_HIGHLIGHT_CODE_BYTES: usize = 512 * 1024;
static HIGHLIGHT_REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

// 设计说明：
// 1. Markdown 源码编辑器中的围栏代码块依赖这里的 tree-sitter capture 名称生成 token span。
// 2. 各语言 query 常用 `function.method`、`function.macro`、`keyword.return` 等细分名称；
//    如果没有在白名单中配置，前端只能拿到零散的基础 token，表现为函数/宏/关键字缺色。
// 3. 这里保留基础名并补齐常见细分名，让 HTML 预览和 Monaco decoration 共用同一套稳定捕获。
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "comment.documentation",
    "constant",
    "constant.builtin",
    "constructor",
    "constructor.builtin",
    "embedded",
    "escape",
    "function",
    "function.builtin",
    "function.call",
    "function.macro",
    "function.method",
    "function.special",
    "keyword",
    "keyword.conditional",
    "keyword.directive",
    "keyword.exception",
    "keyword.function",
    "keyword.import",
    "keyword.operator",
    "keyword.repeat",
    "keyword.return",
    "markup",
    "markup.bold",
    "markup.heading",
    "markup.italic",
    "markup.link",
    "markup.link.url",
    "markup.list",
    "markup.quote",
    "markup.raw",
    "markup.raw.block",
    "markup.raw.inline",
    "module",
    "number",
    "operator",
    "property",
    "property.builtin",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.escape",
    "string.regexp",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.member",
    "variable.parameter",
];

#[derive(Debug, Deserialize)]
pub struct HighlightCodeRequest {
    pub code: String,
    pub language: Option<String>,
    pub theme: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HighlightTokensRequest {
    pub code: String,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HighlightBatchRequest {
    pub blocks: Vec<HighlightCodeBlockRequest>,
    pub theme: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HighlightCodeBlockRequest {
    pub id: String,
    pub code: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HighlightCodeResponse {
    pub id: Option<String>,
    pub debug_id: u64,
    pub language: String,
    pub requested_language: String,
    pub html: String,
    pub supported: bool,
    pub cached: bool,
    pub engine: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct HighlightBatchResponse {
    pub blocks: Vec<HighlightCodeResponse>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct HighlightTokenSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HighlightTokensResponse {
    pub debug_id: u64,
    pub language: String,
    pub requested_language: String,
    pub supported: bool,
    pub spans: Vec<HighlightTokenSpan>,
    pub engine: String,
    pub elapsed_ms: u128,
}

#[derive(Clone)]
struct LanguageSpec {
    id: &'static str,
    language: Language,
    highlights_query: &'static str,
    injections_query: &'static str,
    locals_query: &'static str,
}

static HIGHLIGHT_CACHE: Lazy<Mutex<LruCache<u64, HighlightCodeResponse>>> = Lazy::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(HIGHLIGHT_CACHE_CAPACITY).expect("non-zero highlight cache capacity"),
    ))
});

static HIGHLIGHT_CONFIGS: Lazy<HashMap<&'static str, HighlightConfiguration>> = Lazy::new(|| {
    let mut configs = HashMap::new();
    for spec in language_specs() {
        match HighlightConfiguration::new(
            spec.language,
            spec.id,
            spec.highlights_query,
            spec.injections_query,
            spec.locals_query,
        ) {
            Ok(mut config) => {
                config.configure(HIGHLIGHT_NAMES);
                configs.insert(spec.id, config);
            }
            Err(error) => {
                warn!(
                    target: "syntax_highlight",
                    language = spec.id,
                    error = %error,
                    "tree-sitter highlight configuration failed"
                );
            }
        }
    }
    configs
});

fn language_specs() -> Vec<LanguageSpec> {
    vec![
        LanguageSpec {
            id: "markdown",
            language: tree_sitter_md::LANGUAGE.into(),
            highlights_query: tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            injections_query: tree_sitter_md::INJECTION_QUERY_BLOCK,
            locals_query: "",
        },
        LanguageSpec {
            id: "javascript",
            language: tree_sitter_javascript::LANGUAGE.into(),
            highlights_query: tree_sitter_javascript::HIGHLIGHT_QUERY,
            injections_query: tree_sitter_javascript::INJECTIONS_QUERY,
            locals_query: tree_sitter_javascript::LOCALS_QUERY,
        },
        LanguageSpec {
            id: "typescript",
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            highlights_query: tree_sitter_typescript::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: tree_sitter_typescript::LOCALS_QUERY,
        },
        LanguageSpec {
            id: "tsx",
            language: tree_sitter_typescript::LANGUAGE_TSX.into(),
            highlights_query: tree_sitter_typescript::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: tree_sitter_typescript::LOCALS_QUERY,
        },
        LanguageSpec {
            id: "python",
            language: tree_sitter_python::LANGUAGE.into(),
            highlights_query: tree_sitter_python::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LanguageSpec {
            id: "rust",
            language: tree_sitter_rust::LANGUAGE.into(),
            highlights_query: tree_sitter_rust::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_rust::INJECTIONS_QUERY,
            locals_query: "",
        },
        LanguageSpec {
            id: "bash",
            language: tree_sitter_bash::LANGUAGE.into(),
            highlights_query: tree_sitter_bash::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LanguageSpec {
            id: "lua",
            language: tree_sitter_lua::LANGUAGE.into(),
            highlights_query: tree_sitter_lua::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_lua::INJECTIONS_QUERY,
            locals_query: tree_sitter_lua::LOCALS_QUERY,
        },
        LanguageSpec {
            id: "java",
            language: tree_sitter_java::LANGUAGE.into(),
            highlights_query: tree_sitter_java::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LanguageSpec {
            id: "html",
            language: tree_sitter_html::LANGUAGE.into(),
            highlights_query: tree_sitter_html::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_html::INJECTIONS_QUERY,
            locals_query: "",
        },
        LanguageSpec {
            id: "c",
            language: tree_sitter_c::LANGUAGE.into(),
            highlights_query: tree_sitter_c::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LanguageSpec {
            id: "cpp",
            language: tree_sitter_cpp::LANGUAGE.into(),
            highlights_query: tree_sitter_cpp::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
    ]
}

fn normalize_language(raw: &str) -> String {
    let lang = raw
        .trim()
        .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '{')
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches("language-")
        .to_ascii_lowercase();

    match lang.as_str() {
        "" | "text" | "txt" | "plain" | "plaintext" => "plaintext",
        "md" | "markdown" | "mdown" => "markdown",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "py" | "python3" => "python",
        "rs" => "rust",
        "sh" | "shell" | "zsh" | "fish" | "powershell" | "ps1" => "bash",
        "tex" | "latex" => "latex",
        "c++" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "htm" | "xml" | "vue" => "html",
        other => other,
    }
    .to_string()
}

fn cache_key(code: &str, language: &str, theme: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    language.hash(&mut hasher);
    theme.hash(&mut hasher);
    code.hash(&mut hasher);
    hasher.finish()
}

fn css_class_for_highlight(name: &str) -> &'static str {
    match name {
        "attribute" => "hljs-attr",
        "boolean" => "hljs-literal",
        "comment" | "comment.documentation" => "hljs-comment",
        "constant" | "constant.builtin" => "hljs-literal",
        "constructor" | "constructor.builtin" => "hljs-title",
        "escape" | "string.escape" => "hljs-subst",
        "function" | "function.builtin" | "function.call" | "function.method" | "function.special" => {
            "hljs-title function_"
        }
        "function.macro" => "hljs-title function_ hljs-meta",
        "keyword"
        | "keyword.conditional"
        | "keyword.directive"
        | "keyword.exception"
        | "keyword.function"
        | "keyword.import"
        | "keyword.repeat"
        | "keyword.return" => "hljs-keyword",
        "keyword.operator" => "hljs-operator",
        "markup.heading" => "hljs-section",
        "markup.link" | "markup.link.url" => "hljs-link",
        "markup.raw" | "markup.raw.block" | "markup.raw.inline" => "hljs-code",
        "module" => "hljs-title class_",
        "number" => "hljs-number",
        "operator" => "hljs-operator",
        "property" | "property.builtin" | "variable.member" => "hljs-property",
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" | "punctuation.special" => {
            "hljs-punctuation"
        }
        "string" | "string.regexp" | "string.special" => "hljs-string",
        "tag" => "hljs-tag",
        "type" | "type.builtin" => "hljs-type",
        "variable" | "variable.builtin" | "variable.parameter" => "hljs-variable",
        _ => "hljs",
    }
}

fn escape_html(source: &str) -> String {
    let mut escaped = String::with_capacity(source.len());
    for ch in source.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn wrap_code_html(code_html: String, language: &str, supported: bool) -> String {
    let support_class = if supported { "guaxi-ts-code" } else { "guaxi-ts-code guaxi-ts-code-plain" };
    format!(
        r#"<pre class="hljs {support_class}" data-guaxi-highlight-engine="tree-sitter"><code class="language-{language}">{code_html}</code></pre>"#
    )
}

fn next_debug_id() -> u64 {
    HIGHLIGHT_REQUEST_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn render_plain(
    code: &str,
    language: &str,
    requested_language: &str,
    id: Option<String>,
    debug_id: u64,
    elapsed_ms: u128,
) -> HighlightCodeResponse {
    HighlightCodeResponse {
        id,
        debug_id,
        language: language.to_string(),
        requested_language: requested_language.to_string(),
        html: wrap_code_html(escape_html(code), language, false),
        supported: false,
        cached: false,
        engine: "tree-sitter/plaintext".to_string(),
        elapsed_ms,
    }
}

fn render_latex(
    code: &str,
    requested_language: &str,
    id: Option<String>,
    debug_id: u64,
    start: Instant,
) -> HighlightCodeResponse {
    // 设计思路：
    // 1. tree-sitter-latex 0.1.0 在 Windows/MSVC 下没有正确链接 external scanner，
    //    会让整个 Tauri 程序链接失败，因此生产构建不能硬依赖它。
    // 2. Markdown/代码文件/Python Cell 的核心高亮仍走 tree-sitter-highlight；LaTeX 先保留
    //    轻量 scanner，避免用户的 tex 代码块直接退化成纯文本。
    // 3. 后续如果换成带完整 scanner 与 highlights.scm 的 LaTeX grammar，只需要在
    //    language_specs 中补回标准 HighlightConfiguration。
    debug!(target: "syntax_highlight", bytes = code.len(), "using latex fallback scanner");
    let mut html = String::with_capacity(code.len() + 256);
    let bytes = code.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ch = code[index..].chars().next().unwrap_or('\0');
        if ch == '%' {
            let end = code[index..].find('\n').map(|n| index + n).unwrap_or(code.len());
            html.push_str(r#"<span class="hljs-comment">"#);
            html.push_str(&escape_html(&code[index..end]));
            html.push_str("</span>");
            index = end;
            continue;
        }
        if ch == '\\' {
            let mut end = index + ch.len_utf8();
            while end < code.len() {
                let c = code[end..].chars().next().unwrap_or('\0');
                if c.is_ascii_alphabetic() || c == '*' {
                    end += c.len_utf8();
                } else {
                    break;
                }
            }
            if end == index + 1 && end < code.len() {
                end += code[end..].chars().next().unwrap_or('\0').len_utf8();
            }
            html.push_str(r#"<span class="hljs-keyword">"#);
            html.push_str(&escape_html(&code[index..end]));
            html.push_str("</span>");
            index = end;
            continue;
        }
        if ch == '$' {
            let end = if code[index..].starts_with("$$") {
                index + 2
            } else {
                index + 1
            };
            html.push_str(r#"<span class="hljs-string">"#);
            html.push_str(&escape_html(&code[index..end]));
            html.push_str("</span>");
            index = end;
            continue;
        }
        if "{}[]()".contains(ch) {
            let end = index + ch.len_utf8();
            html.push_str(r#"<span class="hljs-punctuation">"#);
            html.push_str(&escape_html(&code[index..end]));
            html.push_str("</span>");
            index = end;
            continue;
        }
        let end = index + ch.len_utf8();
        html.push_str(&escape_html(&code[index..end]));
        index = end;
    }

    HighlightCodeResponse {
        id,
        debug_id,
        language: "latex".to_string(),
        requested_language: requested_language.to_string(),
        html: wrap_code_html(html, "latex", true),
        supported: true,
        cached: false,
        engine: "tree-sitter/latex-scanner".to_string(),
        elapsed_ms: start.elapsed().as_millis(),
    }
}

fn collect_latex_token_spans(
    code: &str,
    requested_language: &str,
    debug_id: u64,
    start: Instant,
) -> HighlightTokensResponse {
    // 设计思路：
    // 1. 这里与 render_latex 使用同一套轻量扫描规则，保证 HTML 预览高亮和 Monaco 语义 token 高亮一致。
    // 2. token 坐标统一返回 UTF-8 byte offset，前端再根据原文映射到 Monaco 的 UTF-16 column，
    //    这样中文和其他多字节字符不会把高亮位置挤偏。
    // 3. 日志记录 span 数量与耗时，便于排查 .tex 文件打开后是否真正命中了后端高亮链路。
    let mut spans = Vec::new();
    let bytes = code.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let ch = code[index..].chars().next().unwrap_or('\0');
        if ch == '%' {
            let end = code[index..].find('\n').map(|n| index + n).unwrap_or(code.len());
            if end > index {
                spans.push(HighlightTokenSpan {
                    start_byte: index,
                    end_byte: end,
                    token_type: "comment".to_string(),
                });
            }
            index = end;
            continue;
        }

        if ch == '\\' {
            let mut end = index + ch.len_utf8();
            while end < code.len() {
                let c = code[end..].chars().next().unwrap_or('\0');
                if c.is_ascii_alphabetic() || c == '*' {
                    end += c.len_utf8();
                } else {
                    break;
                }
            }
            if end == index + ch.len_utf8() && end < code.len() {
                end += code[end..].chars().next().unwrap_or('\0').len_utf8();
            }
            spans.push(HighlightTokenSpan {
                start_byte: index,
                end_byte: end,
                token_type: "keyword".to_string(),
            });
            index = end;
            continue;
        }

        if ch == '$' {
            let end = if code[index..].starts_with("$$") {
                index + 2
            } else {
                index + 1
            };
            spans.push(HighlightTokenSpan {
                start_byte: index,
                end_byte: end,
                token_type: "string".to_string(),
            });
            index = end;
            continue;
        }

        if "{}[]()".contains(ch) {
            let end = index + ch.len_utf8();
            spans.push(HighlightTokenSpan {
                start_byte: index,
                end_byte: end,
                token_type: "punctuation".to_string(),
            });
            index = end;
            continue;
        }

        index += ch.len_utf8();
    }

    debug!(
        target: "syntax_highlight",
        debug_id,
        span_count = spans.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "latex token fallback completed"
    );

    HighlightTokensResponse {
        debug_id,
        language: "latex".to_string(),
        requested_language: requested_language.to_string(),
        supported: true,
        spans,
        engine: "tree-sitter/latex-scanner".to_string(),
        elapsed_ms: start.elapsed().as_millis(),
    }
}

fn collect_highlight_token_spans(code: &str, raw_language: &str, debug_id: u64) -> HighlightTokensResponse {
    let start = Instant::now();
    let requested_language = raw_language.to_string();
    let language = normalize_language(raw_language);
    debug!(
        target: "syntax_highlight",
        debug_id,
        requested_language,
        normalized_language = language,
        code_bytes = code.len(),
        "tree-sitter token request received"
    );

    if language == "latex" {
        return collect_latex_token_spans(code, &requested_language, debug_id, start);
    }

    let Some(config) = HIGHLIGHT_CONFIGS.get(language.as_str()) else {
        warn!(
            target: "syntax_highlight",
            debug_id,
            requested_language,
            normalized_language = language,
            "unsupported tree-sitter token language"
        );
        return HighlightTokensResponse {
            debug_id,
            language,
            requested_language,
            supported: false,
            spans: Vec::new(),
            engine: "tree-sitter-highlight".to_string(),
            elapsed_ms: start.elapsed().as_millis(),
        };
    };

    let mut highlighter = Highlighter::new();
    let events = highlighter.highlight(config, code.as_bytes(), None, |injected| {
        let normalized = normalize_language(injected);
        HIGHLIGHT_CONFIGS.get(normalized.as_str())
    });

    let mut active_stack: Vec<String> = Vec::new();
    let mut spans = Vec::new();
    match events {
        Ok(events) => {
            for event in events {
                match event {
                    Ok(HighlightEvent::Source { start, end }) => {
                        if start < end {
                            if let Some(token_type) = active_stack.last() {
                                spans.push(HighlightTokenSpan {
                                    start_byte: start,
                                    end_byte: end,
                                    token_type: token_type.clone(),
                                });
                            }
                        }
                    }
                    Ok(HighlightEvent::HighlightStart(highlight)) => {
                        let name = HIGHLIGHT_NAMES
                            .get(highlight.0)
                            .copied()
                            .unwrap_or("variable");
                        active_stack.push(name.to_string());
                    }
                    Ok(HighlightEvent::HighlightEnd) => {
                        active_stack.pop();
                    }
                    Err(error) => {
                        warn!(target: "syntax_highlight", language, error = ?error, "token event failed");
                    }
                }
            }
        }
        Err(error) => {
            warn!(target: "syntax_highlight", debug_id, language, error = ?error, "tree-sitter token highlight failed");
            return HighlightTokensResponse {
                debug_id,
                language,
                requested_language,
                supported: false,
                spans: Vec::new(),
                engine: "tree-sitter-highlight".to_string(),
                elapsed_ms: start.elapsed().as_millis(),
            };
        }
    }

    let mut token_type_counts: HashMap<String, usize> = HashMap::new();
    for span in &spans {
        *token_type_counts.entry(span.token_type.clone()).or_insert(0) += 1;
    }

    debug!(
        target: "syntax_highlight",
        debug_id,
        language,
        span_count = spans.len(),
        token_type_counts = ?token_type_counts,
        elapsed_ms = start.elapsed().as_millis(),
        "tree-sitter token request completed"
    );

    HighlightTokensResponse {
        debug_id,
        language,
        requested_language,
        supported: true,
        spans,
        engine: "tree-sitter-highlight".to_string(),
        elapsed_ms: start.elapsed().as_millis(),
    }
}

fn highlight_code_block(
    code: &str,
    raw_language: &str,
    theme: &str,
    id: Option<String>,
    debug_id: u64,
) -> HighlightCodeResponse {
    let start = Instant::now();
    let requested_language = raw_language.to_string();
    let language = normalize_language(raw_language);
    let key = cache_key(code, &language, theme);
    debug!(
        target: "syntax_highlight",
        debug_id,
        requested_language,
        normalized_language = language,
        theme,
        code_bytes = code.len(),
        "tree-sitter html highlight request received"
    );

    if let Some(mut cached) = HIGHLIGHT_CACHE.lock().get(&key).cloned() {
        cached.id = id;
        cached.debug_id = debug_id;
        cached.cached = true;
        cached.elapsed_ms = start.elapsed().as_millis();
        debug!(
            target: "syntax_highlight",
            debug_id,
            language,
            elapsed_ms = cached.elapsed_ms,
            "tree-sitter html highlight cache hit"
        );
        return cached;
    }

    if language == "plaintext" || code.len() > MAX_HIGHLIGHT_CODE_BYTES {
        let response = render_plain(code, &language, &requested_language, id, debug_id, start.elapsed().as_millis());
        HIGHLIGHT_CACHE.lock().put(key, HighlightCodeResponse { id: None, ..response.clone() });
        debug!(
            target: "syntax_highlight",
            debug_id,
            language,
            supported = response.supported,
            elapsed_ms = response.elapsed_ms,
            "tree-sitter html highlight returned plain text"
        );
        return response;
    }

    if language == "latex" {
        let response = render_latex(code, &requested_language, id, debug_id, start);
        HIGHLIGHT_CACHE.lock().put(key, HighlightCodeResponse { id: None, ..response.clone() });
        debug!(
            target: "syntax_highlight",
            debug_id,
            language = response.language,
            elapsed_ms = response.elapsed_ms,
            "tree-sitter html highlight returned latex fallback"
        );
        return response;
    }

    let Some(config) = HIGHLIGHT_CONFIGS.get(language.as_str()) else {
        warn!(
            target: "syntax_highlight",
            debug_id,
            requested_language,
            normalized_language = language,
            "unsupported tree-sitter language, fallback to plaintext"
        );
        let response = render_plain(code, &language, &requested_language, id, debug_id, start.elapsed().as_millis());
        HIGHLIGHT_CACHE.lock().put(key, HighlightCodeResponse { id: None, ..response.clone() });
        return response;
    };

    let mut highlighter = Highlighter::new();
    let mut html = String::with_capacity(code.len() + 256);
    let events = highlighter.highlight(config, code.as_bytes(), None, |injected| {
        let normalized = normalize_language(injected);
        HIGHLIGHT_CONFIGS.get(normalized.as_str())
    });

    match events {
        Ok(events) => {
            for event in events {
                match event {
                    Ok(HighlightEvent::Source { start, end }) => {
                        html.push_str(&escape_html(&code[start..end]));
                    }
                    Ok(HighlightEvent::HighlightStart(highlight)) => {
                        let name = HIGHLIGHT_NAMES
                            .get(highlight.0)
                            .copied()
                            .unwrap_or("unknown");
                        html.push_str(r#"<span class=""#);
                        html.push_str(css_class_for_highlight(name));
                        html.push_str(r#"">"#);
                    }
                    Ok(HighlightEvent::HighlightEnd) => html.push_str("</span>"),
                    Err(error) => {
                        warn!(target: "syntax_highlight", language, error = ?error, "highlight event failed");
                    }
                }
            }
        }
        Err(error) => {
            warn!(target: "syntax_highlight", debug_id, language, error = ?error, "tree-sitter highlight failed");
            let response = render_plain(code, &language, &requested_language, id, debug_id, start.elapsed().as_millis());
            HIGHLIGHT_CACHE.lock().put(key, HighlightCodeResponse { id: None, ..response.clone() });
            return response;
        }
    }

    let response = HighlightCodeResponse {
        id,
        debug_id,
        language: language.clone(),
        requested_language,
        html: wrap_code_html(html, &language, true),
        supported: true,
        cached: false,
        engine: "tree-sitter-highlight".to_string(),
        elapsed_ms: start.elapsed().as_millis(),
    };
    HIGHLIGHT_CACHE.lock().put(key, HighlightCodeResponse { id: None, ..response.clone() });
    debug!(
        target: "syntax_highlight",
        debug_id,
        language,
        html_bytes = response.html.len(),
        elapsed_ms = response.elapsed_ms,
        "tree-sitter html highlight request completed"
    );
    response
}

pub fn highlight_batch_impl(request: HighlightBatchRequest) -> HighlightBatchResponse {
    let start = Instant::now();
    let theme = request.theme.unwrap_or_else(|| "default".to_string());
    let blocks = request
        .blocks
        .into_iter()
        .map(|block| {
            let debug_id = next_debug_id();
            highlight_code_block(
                &block.code,
                block.language.as_deref().unwrap_or("plaintext"),
                &theme,
                Some(block.id),
                debug_id,
            )
        })
        .collect::<Vec<_>>();

    info!(
        target: "syntax_highlight",
        block_count = blocks.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "tree-sitter highlight batch completed"
    );

    HighlightBatchResponse {
        blocks,
        elapsed_ms: start.elapsed().as_millis(),
    }
}

#[tauri::command]
pub async fn syntax_highlight_code(request: HighlightCodeRequest) -> Result<HighlightCodeResponse, String> {
    let debug_id = next_debug_id();
    let code_bytes = request.code.len();
    let requested_language = request.language.clone().unwrap_or_else(|| "plaintext".to_string());
    let theme = request.theme.unwrap_or_else(|| "default".to_string());
    debug!(
        target: "syntax_highlight",
        debug_id,
        requested_language,
        code_bytes,
        "syntax_highlight_code command scheduled"
    );
    tokio::task::spawn_blocking(move || {
        highlight_code_block(
            &request.code,
            request.language.as_deref().unwrap_or("plaintext"),
            &theme,
            None,
            debug_id,
        )
    })
    .await
    .map_err(|error| format!("tree-sitter highlight task failed: {error}"))
}

#[tauri::command]
pub async fn syntax_highlight_tokens(request: HighlightTokensRequest) -> Result<HighlightTokensResponse, String> {
    let debug_id = next_debug_id();
    let code_bytes = request.code.len();
    let requested_language = request.language.clone().unwrap_or_else(|| "plaintext".to_string());
    debug!(
        target: "syntax_highlight",
        debug_id,
        requested_language,
        code_bytes,
        "syntax_highlight_tokens command scheduled"
    );
    tokio::task::spawn_blocking(move || {
        collect_highlight_token_spans(
            &request.code,
            request.language.as_deref().unwrap_or("plaintext"),
            debug_id,
        )
    })
    .await
    .map_err(|error| format!("tree-sitter token task failed: {error}"))
}

#[tauri::command]
pub async fn syntax_highlight_batch(request: HighlightBatchRequest) -> Result<HighlightBatchResponse, String> {
    tokio::task::spawn_blocking(move || highlight_batch_impl(request))
        .await
        .map_err(|error| format!("tree-sitter highlight batch task failed: {error}"))
}

#[tauri::command]
pub fn syntax_supported_languages() -> Vec<String> {
    let mut languages = HIGHLIGHT_CONFIGS
        .keys()
        .copied()
        .map(str::to_string)
        .collect::<Vec<_>>();
    languages.push("latex".to_string());
    languages.push("plaintext".to_string());
    languages.sort();
    languages.dedup();
    languages
}

#[tauri::command]
pub fn syntax_analyze_document(content: String, language: Option<String>) -> Result<serde_json::Value, String> {
    let lang = normalize_language(language.as_deref().unwrap_or("markdown"));
    let parser_language: Option<Language> = match lang.as_str() {
        "markdown" => Some(tree_sitter_md::LANGUAGE.into()),
        "latex" => return Err("latex structural analysis is unavailable in this build".to_string()),
        other => HIGHLIGHT_CONFIGS.get(other).map(|config| config.language.clone()),
    };

    let Some(parser_language) = parser_language else {
        return Err(format!("unsupported syntax analysis language: {lang}"));
    };

    let mut parser = Parser::new();
    parser
        .set_language(&parser_language)
        .map_err(|error| format!("set tree-sitter language failed: {error}"))?;
    let tree = parser
        .parse(&content, None)
        .ok_or_else(|| "tree-sitter parse returned no tree".to_string())?;

    let root = tree.root_node();
    let mut node_kinds = HashSet::new();
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        node_kinds.insert(node.kind().to_string());
    }

    Ok(serde_json::json!({
        "language": lang,
        "rootKind": root.kind(),
        "hasError": root.has_error(),
        "startByte": root.start_byte(),
        "endByte": root.end_byte(),
        "startPosition": { "row": root.start_position().row, "column": root.start_position().column },
        "endPosition": { "row": root.end_position().row, "column": root.end_position().column },
        "topLevelNodeKinds": node_kinds.into_iter().collect::<Vec<_>>(),
    }))
}
