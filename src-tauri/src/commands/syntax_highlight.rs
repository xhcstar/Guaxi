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
    "annotation",
    "argument",
    "boolean",
    "comment",
    "comment.documentation",
    "constant",
    "constant.builtin",
    "constant.numeric",
    "constructor",
    "constructor.builtin",
    "decorator",
    "embedded",
    "enum",
    "event",
    "escape",
    "field",
    "function",
    "function.builtin",
    "function.call",
    "function.macro",
    "function.method",
    "function.method.call",
    "function.special",
    "identifier",
    "interface",
    "keyword",
    "keyword.conditional",
    "keyword.directive",
    "keyword.exception",
    "keyword.function",
    "keyword.import",
    "keyword.operator",
    "keyword.repeat",
    "keyword.return",
    "modifier",
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
    "namespace",
    "number",
    "operator",
    "package",
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
    "type.parameter",
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

// Markdown 文档里的代码块以前是一个块一次 IPC、而且串行 await，几十个块就是几十次往返。
// 这里把整篇的块合并成一次请求：未变化的块在 collect_highlight_token_spans 里直接命中 LRU，
// 真正需要解析的只有当前正在编辑的那一两个块。
#[derive(Debug, Deserialize)]
pub struct HighlightTokensBatchRequest {
    pub blocks: Vec<HighlightTokensRequest>,
}

#[derive(Debug, Serialize)]
pub struct HighlightTokensBatchResponse {
    // 与请求里的 blocks 按位置一一对应，前端按顺序取回即可。
    pub blocks: Vec<HighlightTokensResponse>,
    pub elapsed_ms: u128,
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

#[derive(Debug, Clone, Serialize, Default)]
pub struct HighlightTokenSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub token_type: String,
    // 前端要把 span 变成 Monaco decoration，需要的是「块内行号 + UTF-16 列」，不是字节偏移。
    // 这段换算原本在前端逐字符做（每个字符还分配一个 Uint8Array），挪到这里一次前向扫描即可，
    // 而且能随 span 一起进缓存——前端命中缓存时不必重算。
    // line 为 0 基（相对传入的 code），列与 Monaco 一致：1 基、按 UTF-16 码元计。
    pub line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    // 0 基 UTF-16 偏移。token 类型判定（如"点号成员访问"）按 JS 字符串下标回看一个字符，
    // 需要的是这个而不是字节偏移——多字节字符下两者不相等。
    pub start_utf16: usize,
}

/// 把 UTF-8 字节偏移解析成 (0 基行号, 1 基 UTF-16 列)。
/// 游标只前进不回退：把每个 span 的起止偏移按升序排好一次扫完，整体 O(code 长度)。
///
/// 语义要点：只消费**起点早于目标偏移**的字符。因此当偏移正好落在换行符上时，
/// 停在上一行行尾（列 = 行尾 + 1）而不是下一行行首——前端原来的实现就是这么定义的。
fn resolve_token_span_positions(code: &str, spans: &mut [HighlightTokenSpan]) {
    if spans.is_empty() {
        return;
    }

    let mut pending: Vec<(usize, bool, usize)> = Vec::with_capacity(spans.len() * 2);
    for (index, span) in spans.iter().enumerate() {
        pending.push((span.start_byte, false, index));
        pending.push((span.end_byte, true, index));
    }
    pending.sort_unstable_by_key(|entry| entry.0);

    let mut byte = 0usize;
    let mut line = 0usize;
    let mut column = 1usize;
    let mut utf16 = 0usize;

    for (target, is_end, index) in pending {
        while byte < target {
            // tree-sitter 给出的偏移一定落在字符边界上。
            let Some(ch) = code[byte..].chars().next() else {
                break;
            };
            byte += ch.len_utf8();
            utf16 += ch.len_utf16();
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += ch.len_utf16();
            }
        }

        if is_end {
            spans[index].end_line = line;
            spans[index].end_column = column;
        } else {
            spans[index].line = line;
            spans[index].start_column = column;
            spans[index].start_utf16 = utf16;
        }
    }
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

// 设计说明：
// 1. token span 这条路径此前完全没有缓存：Markdown 的每个围栏代码块、探索者的每个 Python cell、
//    以及 TreeSitter 的可视区语义高亮，同一段代码每次都要新建 Highlighter 再解析一遍。
//    同文件里走 HTML 的 highlight_code_block 早就有 HIGHLIGHT_CACHE，token 路径漏了同样一层。
// 2. 键与 HTML 路径保持一致（语言 + 代码内容），但不带 theme——token span 只有类型，不含颜色。
// 3. 只缓存解析成功的结果：unsupported / 解析失败可能反映的是 server 侧状态，不该被记住。
const TOKEN_SPANS_CACHE_CAPACITY: usize = 1024;

static TOKEN_SPANS_CACHE: Lazy<Mutex<LruCache<u64, HighlightTokensResponse>>> = Lazy::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(TOKEN_SPANS_CACHE_CAPACITY).expect("non-zero token spans cache capacity"),
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

fn push_latex_span(spans: &mut Vec<HighlightTokenSpan>, start_byte: usize, end_byte: usize, token_type: &str) {
    if end_byte > start_byte {
        spans.push(HighlightTokenSpan {
            start_byte,
            end_byte,
            token_type: token_type.to_string(),
            ..Default::default()
        });
    }
}

fn latex_command_end(code: &str, index: usize) -> usize {
    let Some(ch) = code[index..].chars().next() else {
        return index;
    };
    let mut end = index + ch.len_utf8();
    while end < code.len() {
        let c = code[end..].chars().next().unwrap_or('\0');
        if c.is_ascii_alphabetic() || c == '@' || c == '*' {
            end += c.len_utf8();
        } else {
            break;
        }
    }
    if end == index + ch.len_utf8() && end < code.len() {
        end += code[end..].chars().next().unwrap_or('\0').len_utf8();
    }
    end
}

fn latex_command_name(command: &str) -> &str {
    command
        .trim_start_matches('\\')
        .trim_end_matches('*')
}

fn latex_command_token_type(command_name: &str) -> &'static str {
    match command_name {
        "begin" | "end" | "documentclass" | "usepackage" | "usetikzlibrary" | "input" | "include" => "keyword.import",
        "section" | "subsection" | "subsubsection" | "paragraph" | "title" | "author" | "date" | "maketitle"
        | "tableofcontents" | "item" | "caption" | "label" | "ref" | "eqref" | "cite" | "emph" | "textbf"
        | "textit" | "frac" | "sqrt" | "left" | "right" => "function",
        "int" | "sum" | "prod" | "lim" | "sin" | "cos" | "tan" | "log" | "ln" | "exp" => "function.builtin",
        _ => "keyword",
    }
}

fn latex_is_math_environment(name: &str) -> bool {
    matches!(
        name,
        "align" | "align*" | "aligned" | "equation" | "equation*" | "gather" | "gather*" | "multline"
            | "multline*" | "split" | "cases" | "matrix" | "pmatrix" | "bmatrix" | "vmatrix" | "Vmatrix"
    )
}

fn scan_braced_latex_argument(
    code: &str,
    index: usize,
    content_token_type: &str,
    spans: &mut Vec<HighlightTokenSpan>,
) -> Option<(usize, String)> {
    let mut cursor = index;
    while cursor < code.len() {
        let ch = code[cursor..].chars().next().unwrap_or('\0');
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    if !code[cursor..].starts_with('{') {
        return None;
    }

    let open = cursor;
    cursor += 1;
    let content_start = cursor;
    let mut depth = 1usize;
    while cursor < code.len() {
        let ch = code[cursor..].chars().next().unwrap_or('\0');
        if ch == '\\' {
            cursor = latex_command_end(code, cursor);
            continue;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let content_end = cursor;
                let close = cursor + ch.len_utf8();
                push_latex_span(spans, open, open + 1, "punctuation.bracket");
                push_latex_span(spans, content_start, content_end, content_token_type);
                push_latex_span(spans, cursor, close, "punctuation.bracket");
                return Some((close, code[content_start..content_end].trim().to_string()));
            }
        }
        cursor += ch.len_utf8();
    }
    None
}

fn scan_optional_latex_argument(code: &str, index: usize, spans: &mut Vec<HighlightTokenSpan>) -> Option<usize> {
    let mut cursor = index;
    while cursor < code.len() {
        let ch = code[cursor..].chars().next().unwrap_or('\0');
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    if !code[cursor..].starts_with('[') {
        return None;
    }

    let start = cursor;
    cursor += 1;
    while cursor < code.len() {
        let ch = code[cursor..].chars().next().unwrap_or('\0');
        cursor += ch.len_utf8();
        if ch == ']' {
            push_latex_span(spans, start, cursor, "attribute");
            return Some(cursor);
        }
    }
    None
}

fn scan_latex_token_spans(code: &str) -> Vec<HighlightTokenSpan> {
    // 设计思路：
    // 1. 这仍然是轻量 fallback scanner，但输出 token 类型要贴近 tree-sitter capture，前端才能按 One Dark 语义色板统一渲染。
    // 2. LaTeX 文本区只高亮命令和参数，数学区才识别变量/数字/运算符，避免把普通英文段落误染成公式变量。
    // 3. span 使用 UTF-8 byte offset，和 tree-sitter-highlight 的返回格式保持一致，中文内容不会造成 Monaco 列号偏移。
    let mut spans = Vec::new();
    let mut index = 0;
    let mut math_delimiter_stack: Vec<&'static str> = Vec::new();
    let mut math_environment_depth = 0usize;

    while index < code.len() {
        let ch = code[index..].chars().next().unwrap_or('\0');
        let in_math = !math_delimiter_stack.is_empty() || math_environment_depth > 0;

        if ch == '%' {
            let end = code[index..].find('\n').map(|n| index + n).unwrap_or(code.len());
            push_latex_span(&mut spans, index, end, "comment");
            index = end;
            continue;
        }

        if ch == '\\' {
            if code[index..].starts_with("\\[") || code[index..].starts_with("\\(") {
                let delimiter = &code[index..index + 2];
                push_latex_span(&mut spans, index, index + 2, "punctuation.special");
                math_delimiter_stack.push(if delimiter == "\\[" { "\\]" } else { "\\)" });
                index += 2;
                continue;
            }
            if code[index..].starts_with("\\]") || code[index..].starts_with("\\)") {
                push_latex_span(&mut spans, index, index + 2, "punctuation.special");
                math_delimiter_stack.pop();
                index += 2;
                continue;
            }

            let command_end = latex_command_end(code, index);
            let command = &code[index..command_end];
            let command_name = latex_command_name(command);
            push_latex_span(&mut spans, index, command_end, latex_command_token_type(command_name));

            let mut next = command_end;
            if let Some(after_optional) = scan_optional_latex_argument(code, next, &mut spans) {
                next = after_optional;
            }

            if command_name == "begin" || command_name == "end" {
                if let Some((after_arg, env_name)) = scan_braced_latex_argument(code, next, "type", &mut spans) {
                    if latex_is_math_environment(&env_name) {
                        if command_name == "begin" {
                            math_environment_depth += 1;
                        } else {
                            math_environment_depth = math_environment_depth.saturating_sub(1);
                        }
                    }
                    index = after_arg;
                    continue;
                }
            } else if matches!(command_name, "documentclass" | "usepackage" | "usetikzlibrary" | "input" | "include") {
                if let Some((after_arg, _)) = scan_braced_latex_argument(code, next, "module", &mut spans) {
                    index = after_arg;
                    continue;
                }
            } else if matches!(command_name, "section" | "subsection" | "subsubsection" | "paragraph" | "title" | "caption" | "label" | "ref" | "eqref" | "cite") {
                if let Some((after_arg, _)) = scan_braced_latex_argument(code, next, "variable.parameter", &mut spans) {
                    index = after_arg;
                    continue;
                }
            }

            index = command_end;
            continue;
        }

        if ch == '$' {
            let is_double = code[index..].starts_with("$$");
            let end = index + if is_double { 2 } else { 1 };
            push_latex_span(&mut spans, index, end, "punctuation.special");
            if math_delimiter_stack.last().copied() == Some(if is_double { "$$" } else { "$" }) {
                math_delimiter_stack.pop();
            } else {
                math_delimiter_stack.push(if is_double { "$$" } else { "$" });
            }
            index = end;
            continue;
        }

        if "{}[]()".contains(ch) {
            let end = index + ch.len_utf8();
            push_latex_span(&mut spans, index, end, "punctuation.bracket");
            index = end;
            continue;
        }

        if in_math && ch.is_ascii_digit() {
            let start = index;
            index += ch.len_utf8();
            while index < code.len() {
                let c = code[index..].chars().next().unwrap_or('\0');
                if c.is_ascii_digit() || c == '.' {
                    index += c.len_utf8();
                } else {
                    break;
                }
            }
            push_latex_span(&mut spans, start, index, "number");
            continue;
        }

        if in_math && ch.is_ascii_alphabetic() {
            let start = index;
            index += ch.len_utf8();
            while index < code.len() {
                let c = code[index..].chars().next().unwrap_or('\0');
                if c.is_ascii_alphanumeric() {
                    index += c.len_utf8();
                } else {
                    break;
                }
            }
            push_latex_span(&mut spans, start, index, "variable");
            continue;
        }

        if in_math && "+-=*/^_&|<>".contains(ch) {
            let end = index + ch.len_utf8();
            push_latex_span(&mut spans, index, end, "operator");
            index = end;
            continue;
        }

        index += ch.len_utf8();
    }

    spans
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
    debug!(target: "syntax_highlight", debug_id, bytes = code.len(), "using latex html fallback scanner");
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
    let spans = scan_latex_token_spans(code);

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

fn compute_highlight_token_spans(code: &str, raw_language: &str, debug_id: u64) -> HighlightTokensResponse {
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
        let mut response = collect_latex_token_spans(code, &requested_language, debug_id, start);
        resolve_token_span_positions(code, &mut response.spans);
        return response;
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
                                    ..Default::default()
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

    // 在这里一次算好行列，结果随 span 一起进 LRU：前端命中缓存时无需再逐字符换算。
    resolve_token_span_positions(code, &mut spans);

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

// 带 LRU 的对外入口：命中时只回填本次的 debug_id / 耗时，与 highlight_code_block 的做法一致。
fn collect_highlight_token_spans(code: &str, raw_language: &str, debug_id: u64) -> HighlightTokensResponse {
    let start = Instant::now();
    let language = normalize_language(raw_language);
    let key = cache_key(code, &language, "");

    if let Some(cached) = TOKEN_SPANS_CACHE.lock().get(&key).cloned() {
        debug!(
            target: "syntax_highlight",
            debug_id,
            language,
            code_bytes = code.len(),
            span_count = cached.spans.len(),
            elapsed_ms = start.elapsed().as_millis(),
            "tree-sitter token cache hit"
        );
        return HighlightTokensResponse {
            debug_id,
            elapsed_ms: start.elapsed().as_millis(),
            ..cached
        };
    }

    let response = compute_highlight_token_spans(code, raw_language, debug_id);
    if response.supported {
        TOKEN_SPANS_CACHE.lock().put(
            key,
            HighlightTokensResponse {
                debug_id: 0,
                elapsed_ms: 0,
                ..response.clone()
            },
        );
    }
    response
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
pub async fn syntax_highlight_tokens_batch(
    request: HighlightTokensBatchRequest,
) -> Result<HighlightTokensBatchResponse, String> {
    tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        let block_count = request.blocks.len();
        // 逐个走带缓存的计算：改动的块才真正解析，其余直接命中 LRU。
        let blocks = request
            .blocks
            .into_iter()
            .map(|block| {
                collect_highlight_token_spans(
                    &block.code,
                    block.language.as_deref().unwrap_or("plaintext"),
                    next_debug_id(),
                )
            })
            .collect::<Vec<_>>();

        info!(
            target: "syntax_highlight",
            block_count,
            elapsed_ms = start.elapsed().as_millis(),
            "tree-sitter token batch completed"
        );

        HighlightTokensBatchResponse {
            blocks,
            elapsed_ms: start.elapsed().as_millis(),
        }
    })
    .await
    .map_err(|error| format!("tree-sitter token batch task failed: {error}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 独立参考实现：直接从「目标偏移之前的那段文本」推导行列，不复用被测逻辑。
    /// 语义与前端原实现一致——列是 1 基 UTF-16 码元数；偏移落在换行符上时停在上一行行尾。
    fn reference_position(code: &str, byte_offset: usize) -> (usize, usize) {
        let prefix = &code[..byte_offset];
        let line = prefix.matches('\n').count();
        let tail = prefix.rsplit('\n').next().unwrap_or("");
        let column = tail.chars().map(|ch| ch.len_utf16()).sum::<usize>() + 1;
        (line, column)
    }

    /// 独立参考实现：UTF-16 偏移就是「目标偏移之前那段文本按 UTF-16 计的长度」。
    fn reference_utf16_offset(code: &str, byte_offset: usize) -> usize {
        code[..byte_offset].encode_utf16().count()
    }

    fn span_at(start_byte: usize, end_byte: usize) -> HighlightTokenSpan {
        HighlightTokenSpan {
            start_byte,
            end_byte,
            token_type: "variable".to_string(),
            ..Default::default()
        }
    }

    /// 这段换算一旦偏一个码元，中文笔记里的代码块配色就会整片错位，所以逐个字符边界比对。
    #[test]
    fn resolves_positions_at_every_char_boundary() {
        let samples = [
            "",
            "\n",
            "let a = 1;\nlet b = 2;\n",
            "中文变量 = 2;\n另一行 = 3;\n",
            "let e = \"😀\";\nlet f = 1;\n",
            "a\n\n\nb",
            "混合 mix 中文 😀\n第二行 second",
            "没有换行的中文一行",
        ];

        for code in samples {
            let mut offsets: Vec<usize> = code.char_indices().map(|(index, _)| index).collect();
            offsets.push(code.len());

            // 零宽区间（每个边界）+ 跨字符区间（每对相邻边界），覆盖 start/end 两种解析。
            let mut spans: Vec<HighlightTokenSpan> =
                offsets.iter().map(|offset| span_at(*offset, *offset)).collect();
            for window in offsets.windows(2) {
                spans.push(span_at(window[0], window[1]));
            }

            resolve_token_span_positions(code, &mut spans);

            for span in &spans {
                assert_eq!(
                    (span.line, span.start_column),
                    reference_position(code, span.start_byte),
                    "start mismatch in {code:?} at byte {}",
                    span.start_byte
                );
                assert_eq!(
                    (span.end_line, span.end_column),
                    reference_position(code, span.end_byte),
                    "end mismatch in {code:?} at byte {}",
                    span.end_byte
                );
                assert_eq!(
                    span.start_utf16,
                    reference_utf16_offset(code, span.start_byte),
                    "start utf16 mismatch in {code:?} at byte {}",
                    span.start_byte
                );
            }
        }
    }

    /// 空输入不应该 panic，也不应该改动任何东西。
    #[test]
    fn handles_empty_span_list() {
        let mut spans: Vec<HighlightTokenSpan> = Vec::new();
        resolve_token_span_positions("let a = 1;", &mut spans);
        assert!(spans.is_empty());
    }
}
