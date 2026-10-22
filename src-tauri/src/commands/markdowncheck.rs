// src-tauri/src/commands/markdowncheck.rs
use serde::{Deserialize, Serialize};
use std::time::Instant;

// ==================== 错误类型定义 ====================
#[allow(dead_code)]
#[derive(Debug)]
pub enum MarkdownError {
    FormatError(String),
    ParseError(String),
    ConfigError(String),
    InternalError(String),
}

impl std::fmt::Display for MarkdownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarkdownError::FormatError(msg) => write!(f, "格式化失败: {}", msg),
            MarkdownError::ParseError(msg) => write!(f, "解析失败: {}", msg),
            MarkdownError::ConfigError(msg) => write!(f, "配置错误: {}", msg),
            MarkdownError::InternalError(msg) => write!(f, "内部错误: {}", msg),
        }
    }
}

impl serde::Serialize for MarkdownError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ==================== 配置选项定义 ====================
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarkdownFormatOptions {
    /// 最大行宽 (0 表示不限制)
    pub max_line_width: usize,
    /// 缩进宽度 (空格数)
    pub indent_width: usize,
    /// 列表项缩进
    pub list_indent: usize,
    /// 是否格式化表格
    pub format_tables: bool,
    /// 是否格式化列表
    pub format_lists: bool,
    /// 是否保留代码块格式
    pub preserve_code_blocks: bool,
    /// 是否移除多余空行
    pub remove_extra_blank_lines: bool,
    /// 是否修复多余的空白字符
    pub fix_extra_whitespace: bool,
}

impl Default for MarkdownFormatOptions {
    fn default() -> Self {
        Self {
            max_line_width: 120,
            indent_width: 2,
            list_indent: 2,
            format_tables: true,
            format_lists: true,
            preserve_code_blocks: true,
            remove_extra_blank_lines: true,
            fix_extra_whitespace: true,
        }
    }
}

// ==================== 生产级 Markdown 格式化器（Comrak） ====================
fn format_markdown_production(
    content: &str,
    options: &MarkdownFormatOptions,
) -> Result<String, MarkdownError> {
    let line_ending = detect_line_ending(content);

    // 探索者/Notebook 的自定义 cell 是“源码里的不透明区块”：
    // <!--math--> ... <!--/math-->、<!--py--> ... <!--/py-->、<!-- guaxi:pyout:...:start --> ... :end -->
    // 内部是原始 LaTeX / Python，不是 Markdown。先整块抽出来换成占位注释，
    // 格式化完再原样放回去——否则 comrak 会把 Python 代码揉成段落、把 `*args` 转义、
    // 把 `# 注释` 当成标题（实测如此），cell 语法直接坏掉。
    let extracted = extract_opaque_cells(content);

    if !options.format_lists {
        let formatted = SimpleMarkdownFormatter::new(options.clone()).format(&extracted.text)?;
        let restored = restore_opaque_cells(&formatted, &extracted.blocks);
        return Ok(apply_line_ending(&restored, line_ending));
    }

    let formatted = format_with_comrak(&extracted.text, options)?;
    let normalized = postprocess_markdown(&formatted, options);
    let restored = restore_opaque_cells(&normalized, &extracted.blocks);
    Ok(apply_line_ending(&restored, line_ending))
}

/// 需要整块原样保留的自定义 cell 类型。
#[derive(Clone, Copy, PartialEq, Eq)]
enum OpaqueCellKind {
    Math,
    Python,
    PythonOutput,
}

/// 图表 / 白板围栏的 info string（```chart、```excalidraw）。
///
/// 它们与 cell 一样属于「源码是不透明配置」的区块：围栏里是 YAML 风格的 spec 与
/// 内联 CSV/TSV 数据，行尾空格、缩进层级、`|` 分隔符都是语义的一部分。
/// 走一遍 Markdown 序列化虽然目前能原样保留，但那是解析器的实现细节：
/// 一旦有哪条后处理（裁行尾空白、按宽度重排、把 `-1,2` 当列表项）碰到它，
/// 图表就会静默变成空白或画错。所以直接按不透明块处理，不依赖下游行为。
const CHART_FENCE_INFOS: [&str; 2] = ["chart", "excalidraw"];

/// 识别图表 / 白板围栏的起始行：```chart、```excalidraw（含 `~~~` 与带属性的写法）。
fn chart_fence_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    let rest = if let Some(r) = trimmed.strip_prefix("```") {
        r
    } else if let Some(r) = trimmed.strip_prefix("~~~") {
        r
    } else {
        return false;
    };
    let info = rest.trim_start().to_ascii_lowercase();
    CHART_FENCE_INFOS
        .iter()
        .any(|name| info == *name || info.starts_with(&format!("{name} ")))
}

/// 判断某一行能否闭合 `opener` 打开的围栏。
///
/// 与 CommonMark 一致：必须同字符、长度不短于开启围栏，且闭合行不能带 info string。
fn is_fence_close(line: &str, opener: &str) -> bool {
    let trimmed = line.trim_start();
    let open_ch = opener.chars().next().unwrap_or('`');
    let close_ch = if trimmed.starts_with("```") {
        '`'
    } else if trimmed.starts_with("~~~") {
        '~'
    } else {
        return false;
    };
    if close_ch != open_ch {
        return false;
    }
    let close_len = trimmed.chars().take_while(|c| *c == close_ch).count();
    let open_len = opener.chars().take_while(|c| *c == open_ch).count();
    if close_len < open_len {
        return false;
    }
    // 闭合围栏行只允许围栏字符与空白（带 info string 的行是新的开启围栏）。
    trimmed[close_len..].trim().is_empty()
}

/// 识别 cell 起始行。语法与后端 notebook.rs 的解析保持一致：
/// `<!--math-->` / `<!--math {...}-->`、`<!--py-->` / `<!--py {...}-->`、`<!-- guaxi:pyout:...:start -->`。
fn opaque_cell_start_kind(line: &str) -> Option<OpaqueCellKind> {
    let text = line.trim();
    if text == "<!--math-->" || (text.starts_with("<!--math ") && text.ends_with("-->")) {
        return Some(OpaqueCellKind::Math);
    }
    if text == "<!--py-->" || (text.starts_with("<!--py ") && text.ends_with("-->")) {
        return Some(OpaqueCellKind::Python);
    }
    if text.starts_with("<!-- guaxi:pyout:") && text.ends_with(":start -->") {
        return Some(OpaqueCellKind::PythonOutput);
    }
    None
}

fn is_opaque_cell_end(kind: OpaqueCellKind, line: &str) -> bool {
    let text = line.trim();
    match kind {
        OpaqueCellKind::Math => text == "<!--/math-->",
        OpaqueCellKind::Python => text == "<!--/py-->",
        OpaqueCellKind::PythonOutput => {
            text.starts_with("<!-- guaxi:pyout:") && text.ends_with(":end -->")
        }
    }
}

struct OpaqueCells {
    /// 把 cell 换成占位注释后的正文，交给格式化器。
    text: String,
    /// 各 cell 的原始文本（含起止标记行），按占位编号索引。
    blocks: Vec<String>,
}

/// 抽出所有自定义 cell 与图表围栏，用单行 HTML 注释占位。
///
/// 占位用 HTML 注释是因为 comrak 会把注释块原样输出，不会被转义或重排（实测）。
/// 找不到结束标记时把剩余内容整体当不透明块：宁可少格式化，也不能把用户代码改坏。
fn extract_opaque_cells(content: &str) -> OpaqueCells {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 8);
    let mut blocks: Vec<String> = Vec::new();
    let mut i = 0usize;

    let push_block = |out: &mut Vec<String>, blocks: &mut Vec<String>, text: String| {
        blocks.push(text);
        out.push(String::new());
        out.push(format!("<!--guaxi:opaque:{}-->", blocks.len() - 1));
        out.push(String::new());
    };

    while i < lines.len() {
        if let Some(kind) = opaque_cell_start_kind(lines[i]) {
            let end_idx = (i + 1..lines.len())
                .find(|&j| is_opaque_cell_end(kind, lines[j]))
                .unwrap_or_else(|| lines.len().saturating_sub(1));

            push_block(&mut out, &mut blocks, lines[i..=end_idx].join("\n"));
            i = end_idx + 1;
            continue;
        }

        // 图表 / 白板围栏：连同内部 YAML 与内联数据整块保留。
        // 未闭合时按 CommonMark 语义延续到文末（与解析器一致），同样整块保留。
        if chart_fence_start(lines[i]) {
            let opener = lines[i].trim_start();
            let end_idx = (i + 1..lines.len())
                .find(|&j| is_fence_close(lines[j], opener))
                .unwrap_or_else(|| lines.len().saturating_sub(1));

            push_block(&mut out, &mut blocks, lines[i..=end_idx].join("\n"));
            i = end_idx + 1;
            continue;
        }

        out.push(lines[i].to_string());
        i += 1;
    }

    let mut text = out.join("\n");
    if content.ends_with('\n') {
        text.push('\n');
    }
    OpaqueCells { text, blocks }
}

fn restore_opaque_cells(formatted: &str, blocks: &[String]) -> String {
    if blocks.is_empty() {
        return formatted.to_string();
    }

    let mut result = formatted.to_string();
    for (idx, block) in blocks.iter().enumerate() {
        let marker = format!("<!--guaxi:opaque:{}-->", idx);
        if let Some(pos) = result.find(&marker) {
            result.replace_range(pos..pos + marker.len(), block);
        }
    }
    result
}

fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn apply_line_ending(content: &str, line_ending: &str) -> String {
    if line_ending == "\n" {
        return content.to_string();
    }

    let normalized = content.replace("\r\n", "\n");
    normalized.replace("\n", line_ending)
}

fn format_with_comrak(content: &str, options: &MarkdownFormatOptions) -> Result<String, MarkdownError> {
    use comrak::{format_commonmark, parse_document, Arena, Options};

    let arena = Arena::new();
    let mut comrak_options = Options::default();

    comrak_options.parse.smart = true;

    comrak_options.extension.autolink = true;
    comrak_options.extension.strikethrough = true;
    comrak_options.extension.table = options.format_tables;
    comrak_options.extension.tasklist = true;
    comrak_options.extension.footnotes = true;
    comrak_options.extension.description_lists = true;
    comrak_options.extension.front_matter_delimiter = Some("---".to_string());
    // 数学块必须原样保留。
    // 不开这个扩展时，`$$...$$` 只是一段普通段落文本，format_commonmark 序列化时会把里面的
    // 反斜杠转义成 `\\`——而 `\\` 在 LaTeX 里是换行，公式直接被改坏（用户反馈"格式化后公式渲染失败"）。
    // 开启后 `$...$` / `$$...$$` 解析成 Math 节点，cm.rs 的 format_math 用 Escaping::Literal
    // 原样写回，一字不动。
    // 识别规则与 GitHub 一致（开 `$` 后不能紧跟空白、闭 `$` 前不能是空白、闭 `$` 后不能是数字），
    // 所以正文里的货币写法（如 `$5 和 $10`）不会被误吞成公式。
    comrak_options.extension.math_dollars = true;

    comrak_options.render.width = if options.max_line_width == 0 { 0 } else { options.max_line_width };

    // 解析前先把块级公式的围栏和正文拆开：Math 节点是行内节点，
    // format_math 又是纯拼接，围栏紧贴正文时会被粘成 `其中：$$` / `$$ 后文`（见下方函数注释）。
    let isolated = isolate_display_math_blocks(content);
    let root = parse_document(&arena, &isolated, &comrak_options);
    let mut out = String::with_capacity(content.len().saturating_add(128));

    format_commonmark(root, &comrak_options, &mut out)
        .map_err(|e| MarkdownError::FormatError(e.to_string()))?;

    Ok(out)
}

/// 让独占一行的 `$$` 围栏与相邻正文之间至少隔一个空行。
///
/// 背景：comrak 的 `math_dollars` 把 `$$...$$` 解析成 Math 节点，但序列化
/// （cm.rs 的 format_math）只是原样拼 `$$` + 内容 + `$$`，**不会补换行**。
/// 于是这种常见写法——正文下一行直接写 `$$`，中间不空行——
/// 会被粘成 `其中： $$` 和 `$$ 后文`，阅读器就认不出这是块级公式了
/// （用户反馈"格式化后数学公式渲染失败"的截图正是这种形态）。
///
/// 只处理行首没有缩进、并且整行就是 `$$` 的围栏：
/// 列表/引用里的 `$$` 带缩进或 `>` 前缀，硬插空行会改变它们的嵌套结构，
/// 这种交给解析器原样处理。代码围栏里的 `$$` 一律不碰。
fn isolate_display_math_blocks(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len() + 8);
    let mut in_display_math = false;
    let mut in_code_fence = false;
    let mut code_fence = "";

    for (idx, raw) in lines.iter().enumerate() {
        let trimmed_start = raw.trim_start();
        let is_code_fence = trimmed_start.starts_with("```") || trimmed_start.starts_with("~~~");

        if is_code_fence {
            if in_code_fence {
                if trimmed_start.starts_with(code_fence) {
                    in_code_fence = false;
                    code_fence = "";
                }
            } else {
                in_code_fence = true;
                code_fence = if trimmed_start.starts_with("~~~") { "~~~" } else { "```" };
            }
            out.push(raw);
            continue;
        }

        if in_code_fence {
            out.push(raw);
            continue;
        }

        let is_dollar_fence = raw.trim() == "$$";
        let is_flush_left = !raw.starts_with(' ') && !raw.starts_with('\t');

        if in_display_math {
            out.push(raw);
            if is_dollar_fence && is_flush_left {
                in_display_math = false;
                // 闭合围栏后面紧跟正文时补一个空行，避免 `$$ 后文`。
                if lines
                    .get(idx + 1)
                    .map_or(false, |next| !next.trim().is_empty())
                {
                    out.push("");
                }
            }
            continue;
        }

        if is_dollar_fence && is_flush_left {
            // 开围栏前面紧跟正文时补一个空行，避免 `其中：$$`。
            if out.last().map_or(false, |prev| !prev.trim().is_empty()) {
                out.push("");
            }
            out.push(raw);
            in_display_math = true;
            continue;
        }

        // 修复旧版本格式化器留下的粘连形态：`其中：$$` + 若干公式行 + 独占一行的 `$$`。
        // 只在「整行只有一个 `$$`、且在行尾」时才拆，避免误伤 `文字 $$x^2$$ 后文` 这类行内公式。
        if is_flush_left
            && !is_dollar_fence
            && raw.ends_with("$$")
            && raw.matches("$$").count() == 1
            && is_glued_display_math_opener(&lines, idx)
        {
            out.push(raw[..raw.len() - 2].trim_end());
            out.push("");
            out.push("$$");
            in_display_math = true;
            continue;
        }

        out.push(raw);
    }

    let mut result = out.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// 判断 `lines[idx]` 末尾的 `$$` 是不是一个被粘到正文行尾的块级公式开围栏：
/// 往下找必须在一段连续非空行里遇到独占一行的 `$$`（闭围栏），中间不能再出现别的 `$$`。
fn is_glued_display_math_opener(lines: &[&str], idx: usize) -> bool {
    for raw in lines.iter().skip(idx + 1) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return false;
        }
        if trimmed == "$$" {
            return true;
        }
        if trimmed.contains("$$") {
            return false;
        }
    }
    false
}

fn unescape_heading_dots(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let leading_len = line.len() - trimmed.len();

    let bytes = trimmed.as_bytes();
    let mut hash_count = 0usize;
    while hash_count < bytes.len() && bytes[hash_count] == b'#' {
        hash_count += 1;
    }

    if hash_count == 0 || hash_count > 6 {
        return None;
    }

    if hash_count >= bytes.len() || bytes[hash_count] != b' ' {
        return None;
    }

    let prefix_end = leading_len + hash_count + 1;
    if prefix_end > line.len() {
        return None;
    }

    let (prefix, rest) = line.split_at(prefix_end);
    let unescaped = rest.replace("\\.", ".");
    if unescaped == rest {
        return None;
    }

    Some(format!("{}{}", prefix, unescaped))
}

fn postprocess_markdown(formatted: &str, options: &MarkdownFormatOptions) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    let mut in_fence = false;
    let mut fence = "";

    for raw in formatted.lines() {
        let trimmed_start = raw.trim_start();
        let is_fence = trimmed_start.starts_with("```") || trimmed_start.starts_with("~~~");

        if is_fence {
            if in_fence {
                if trimmed_start.starts_with(fence) {
                    in_fence = false;
                    fence = "";
                }
            } else {
                in_fence = true;
                fence = if trimmed_start.starts_with("~~~") { "~~~" } else { "```" };
            }
            blank_run = 0;
            out.push(raw.to_string());
            continue;
        }

        if in_fence {
            if options.preserve_code_blocks {
                out.push(raw.to_string());
            } else {
                out.push(raw.trim_end().to_string());
            }
            continue;
        }

        let mut line = raw.to_string();

        if let Some(unescaped) = unescape_heading_dots(&line) {
            line = unescaped;
        }

        if options.fix_extra_whitespace {
            line = line.trim_end().to_string();

            if line.contains('\t') {
                let mut prefix = String::new();
                let mut idx = 0usize;
                for (i, ch) in line.char_indices() {
                    if ch == ' ' {
                        prefix.push(' ');
                        idx = i + ch.len_utf8();
                        continue;
                    }
                    if ch == '\t' {
                        prefix.push_str(&" ".repeat(options.indent_width.max(1)));
                        idx = i + ch.len_utf8();
                        continue;
                    }
                    break;
                }

                if idx > 0 {
                    line = format!("{}{}", prefix, &line[idx..]);
                }
            }
        }

        if options.list_indent != 2 {
            if let Some((indent, rest)) = split_leading_spaces(&line) {
                if is_list_marker(rest) {
                    let level = indent.len() / 2;
                    let next_indent = " ".repeat(level.saturating_mul(options.list_indent.max(1)));
                    line = format!("{}{}", next_indent, rest);
                }
            }
        }

        let is_blank = line.trim().is_empty();
        if is_blank {
            if options.remove_extra_blank_lines {
                blank_run += 1;
                if blank_run > 1 {
                    continue;
                }
            }
            out.push(String::new());
            continue;
        }

        blank_run = 0;
        out.push(line);
    }

    let mut result = out.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn split_leading_spaces(line: &str) -> Option<(String, &str)> {
    let mut n = 0usize;
    for c in line.chars() {
        if c == ' ' {
            n += 1;
        } else {
            break;
        }
    }
    Some((" ".repeat(n), &line[n..]))
}

fn is_list_marker(rest: &str) -> bool {
    let s = rest.trim_start();
    if s.starts_with("- ") || s.starts_with("* ") || s.starts_with("+ ") {
        return true;
    }
    let mut chars = s.chars();
    let mut seen_digit = false;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            seen_digit = true;
            continue;
        }
        if seen_digit && c == '.' {
            return chars.next() == Some(' ');
        }
        break;
    }
    false
}

// ==================== 简单的 Markdown 格式化器 ====================
struct SimpleMarkdownFormatter {
    options: MarkdownFormatOptions,
}

impl SimpleMarkdownFormatter {
    pub fn new(options: MarkdownFormatOptions) -> Self {
        Self { options }
    }

    pub fn format(&self, content: &str) -> Result<String, MarkdownError> {
        let lines: Vec<&str> = content.lines().collect();
        let mut formatted_lines: Vec<String> = Vec::new(); // 改为 String 类型
        let mut in_display_math = false;

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];

            if line.trim().is_empty() {
                // 处理空行
                if !self.options.remove_extra_blank_lines
                    || (i > 0 && i < lines.len() - 1 && !lines[i - 1].trim().is_empty())
                {
                    formatted_lines.push("".to_string());
                }
                i += 1;
                continue;
            }

            // 检查是否是代码块
            if line.trim().starts_with("```") {
                let (code_lines, skip) = self.extract_code_block(&lines[i..]);
                formatted_lines.extend(code_lines);
                i += skip;
                continue;
            }

            // 块级公式（独占一行的 `$$`）内容一字不动，也不按宽度折行：
            // LaTeX 里的空格有语义（运算符间距、对齐），按词折行会把公式直接拆坏。
            if in_display_math {
                formatted_lines.push(line.to_string());
                if line.trim() == "$$" {
                    in_display_math = false;
                }
                i += 1;
                continue;
            }

            if line.trim() == "$$" && !line.starts_with(' ') && !line.starts_with('\t') {
                formatted_lines.push(line.to_string());
                in_display_math = true;
                i += 1;
                continue;
            }

            // 检查是否是列表项
            if self.is_list_item(line) && self.options.format_lists {
                let (list_lines, skip) = self.format_list(&lines[i..]);
                formatted_lines.extend(list_lines);
                i += skip;
                continue;
            }

            // 检查是否是标题
            if line.trim().starts_with('#') {
                formatted_lines.push(self.format_heading(line));
                i += 1;
                continue;
            }

            // 检查是否是表格
            if line.trim().starts_with('|') && self.options.format_tables {
                let (table_lines, skip) = self.extract_table(&lines[i..]);
                formatted_lines.extend(table_lines);
                i += skip;
                continue;
            }

            // 检查是否是引用块
            if line.trim().starts_with('>') {
                let (blockquote_lines, skip) = self.extract_blockquote(&lines[i..]);
                formatted_lines.extend(blockquote_lines);
                i += skip;
                continue;
            }

            // 普通文本
            let formatted_line = if self.options.max_line_width > 0 {
                self.wrap_text(line, self.options.max_line_width)
            } else if self.options.fix_extra_whitespace {
                self.fix_whitespace(line)
            } else {
                line.to_string()
            };

            formatted_lines.push(formatted_line);
            i += 1;
        }

        // 确保最后一行是空行
        if !formatted_lines.is_empty() && !formatted_lines.last().unwrap().is_empty() {
            formatted_lines.push("".to_string());
        }

        Ok(formatted_lines.join("\n"))
    }

    fn is_list_item(&self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with('-')
            || trimmed.starts_with('*')
            || trimmed.starts_with('+')
            || (trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('.'))
    }

    fn format_heading(&self, line: &str) -> String {
        let trimmed = line.trim();
        let mut result = String::new();

        // 计算标题级别
        let mut level = 0;
        for c in trimmed.chars() {
            if c == '#' {
                level += 1;
            } else {
                break;
            }
        }

        // 添加标题标记
        result.push_str(&"#".repeat(level));

        // 添加一个空格
        if level > 0 {
            result.push(' ');
        }

        // 添加标题内容
        let content = &trimmed[level..].trim();
        result.push_str(content);

        result
    }

    fn format_list(&self, lines: &[&str]) -> (Vec<String>, usize) {
        let mut result = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.trim().is_empty() {
                break;
            }

            if self.is_list_item(line) {
                // 计算缩进
                let indent_level = line.chars().take_while(|c| c.is_whitespace()).count();
                let _indent = " ".repeat(indent_level);

                // 格式化列表项
                let formatted_item = if self.options.fix_extra_whitespace {
                    self.fix_whitespace(line)
                } else {
                    line.to_string()
                };

                result.push(formatted_item);
                i += 1;

                // 处理多行列表项
                while i < lines.len() && !lines[i].trim().is_empty() && !self.is_list_item(lines[i])
                {
                    // 子项缩进
                    let sub_indent = " ".repeat(indent_level + self.options.list_indent);
                    let sub_line = if self.options.fix_extra_whitespace {
                        self.fix_whitespace(lines[i])
                    } else {
                        lines[i].to_string()
                    };

                    // 如果行太长，进行换行
                    let wrapped = if self.options.max_line_width > 0 {
                        self.wrap_text(
                            &format!("{}{}", sub_indent, sub_line.trim()),
                            self.options.max_line_width,
                        )
                    } else {
                        format!("{}{}", sub_indent, sub_line.trim())
                    };

                    result.push(wrapped);
                    i += 1;
                }
            } else {
                break;
            }
        }

        (result, i)
    }

    fn wrap_text(&self, text: &str, width: usize) -> String {
        if text.len() <= width {
            return text.to_string();
        }

        let words: Vec<&str> = text.split_whitespace().collect();
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for word in words {
            if current_line.len() + word.len() + 1 > width {
                if !current_line.is_empty() {
                    lines.push(current_line.trim().to_string());
                }
                current_line = word.to_string();
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line.trim().to_string());
        }

        lines.join("\n")
    }

    fn fix_whitespace(&self, text: &str) -> String {
        // 修复多个连续空格
        let parts: Vec<&str> = text.split_whitespace().collect();
        parts.join(" ")
    }

    fn extract_code_block(&self, lines: &[&str]) -> (Vec<String>, usize) {
        let mut result = Vec::new();
        let mut i = 0;

        if i < lines.len() {
            // 添加代码块开始标记
            result.push(lines[i].to_string());
            i += 1;

            // 添加代码内容
            while i < lines.len() && !lines[i].trim().starts_with("```") {
                result.push(lines[i].to_string());
                i += 1;
            }

            // 添加代码块结束标记
            if i < lines.len() {
                result.push(lines[i].to_string());
                i += 1;
            }
        }

        (result, i)
    }

    fn extract_table(&self, lines: &[&str]) -> (Vec<String>, usize) {
        let mut result = Vec::new();
        let mut i = 0;

        while i < lines.len() && lines[i].trim().starts_with('|') {
            result.push(lines[i].to_string());
            i += 1;
        }

        (result, i)
    }

    fn extract_blockquote(&self, lines: &[&str]) -> (Vec<String>, usize) {
        let mut result = Vec::new();
        let mut i = 0;

        while i < lines.len() && lines[i].trim().starts_with('>') {
            result.push(lines[i].to_string());
            i += 1;
        }

        (result, i)
    }
}

// ==================== 请求响应结构 ====================
#[derive(Debug, Deserialize)]
pub struct FormatRequest {
    pub content: String,
    pub options: Option<MarkdownFormatOptions>,
}

#[derive(Debug, Serialize)]
pub struct FormatResponse {
    pub success: bool,
    pub formatted_content: String,
    pub error: Option<String>,
    pub stats: Option<FormatStats>,
}

#[derive(Debug, Serialize)]
pub struct FormatStats {
    pub original_length: usize,
    pub formatted_length: usize,
    pub line_count: usize,
    pub processing_time_ms: u128,
}

#[derive(Debug, Deserialize)]
pub struct BatchFormatRequest {
    pub files: Vec<FileFormatRequest>,
}

#[derive(Debug, Deserialize)]
pub struct FileFormatRequest {
    pub file_path: String,
    pub content: String,
    pub options: Option<MarkdownFormatOptions>,
}

#[derive(Debug, Serialize)]
pub struct BatchFormatResponse {
    pub results: Vec<FileFormatResult>,
    pub total_processed: usize,
    pub total_succeeded: usize,
    pub total_failed: usize,
}

#[derive(Debug, Serialize)]
pub struct FileFormatResult {
    pub file_path: String,
    pub success: bool,
    pub formatted_content: String,
    pub error: Option<String>,
    pub stats: Option<FormatStats>,
}

#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub warning_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ValidationIssue {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub severity: IssueSeverity,
}

#[derive(Debug, Serialize)]
pub enum IssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
pub struct MarkdownStats {
    pub total_lines: usize,
    pub total_chars: usize,
    pub word_count: usize,
    pub heading_count: usize,
    pub code_block_count: usize,
    pub link_count: usize,
    pub image_count: usize,
    pub list_count: usize,
}

// ==================== 主接口实现 ====================
#[tauri::command]
pub async fn markdown_format(request: FormatRequest) -> Result<FormatResponse, MarkdownError> {
    let start_time = Instant::now();

    let options = request.options.unwrap_or_default();
    let options_for_format = options.clone();

    let original_content = request.content;
    let original_length = original_content.len();
    let content_for_format = original_content.clone();

    let format_result = tauri::async_runtime::spawn_blocking(move || {
        format_markdown_production(&content_for_format, &options_for_format)
    })
    .await
    .map_err(|e| MarkdownError::InternalError(e.to_string()))?;

    match format_result {
        Ok(formatted_content) => {
            let processing_time = start_time.elapsed().as_millis();
            let stats = FormatStats {
                original_length,
                formatted_length: formatted_content.len(),
                line_count: formatted_content.lines().count(),
                processing_time_ms: processing_time,
            };

            tracing::info!(
                "Markdown格式化成功: {} 字符 -> {} 字符",
                original_length,
                formatted_content.len()
            );

            Ok(FormatResponse {
                success: true,
                formatted_content,
                error: None,
                stats: Some(stats),
            })
        }
        Err(e) => {
            tracing::error!("Markdown格式化失败: {}", e);
            Ok(FormatResponse {
                success: false,
                formatted_content: original_content,
                error: Some(e.to_string()),
                stats: None,
            })
        }
    }
}

#[tauri::command]
pub async fn markdown_batch_format(
    request: BatchFormatRequest,
) -> Result<BatchFormatResponse, MarkdownError> {
    let mut results = Vec::new();
    let mut total_succeeded = 0;
    let mut total_failed = 0;

    tracing::info!("开始批量格式化 {} 个文件", request.files.len());

    for file_request in request.files {
        // 先克隆内容，避免移动后使用
        let content_clone = file_request.content.clone();

        let format_result = markdown_format(FormatRequest {
            content: file_request.content,
            options: file_request.options,
        })
        .await;

        match format_result {
            Ok(response) => {
                if response.success {
                    total_succeeded += 1;
                } else {
                    total_failed += 1;
                }

                results.push(FileFormatResult {
                    file_path: file_request.file_path,
                    success: response.success,
                    formatted_content: response.formatted_content,
                    error: response.error,
                    stats: response.stats,
                });
            }
            Err(e) => {
                total_failed += 1;
                results.push(FileFormatResult {
                    file_path: file_request.file_path,
                    success: false,
                    formatted_content: content_clone, // 使用克隆的内容
                    error: Some(e.to_string()),
                    stats: None,
                });
            }
        }
    }

    tracing::info!(
        "批量格式化完成: 成功 {} 个, 失败 {} 个",
        total_succeeded,
        total_failed
    );

    Ok(BatchFormatResponse {
        results,
        total_processed: total_succeeded + total_failed,
        total_succeeded,
        total_failed,
    })
}

#[tauri::command]
pub async fn get_default_format_options() -> MarkdownFormatOptions {
    MarkdownFormatOptions::default()
}

#[tauri::command]
pub async fn validate_markdown(content: String) -> Result<ValidationResult, MarkdownError> {
    // 简单的语法检查
    let mut issues = Vec::new();

    // 检查未闭合的代码块
    let code_block_count = content.matches("```").count();
    if code_block_count % 2 != 0 {
        issues.push(ValidationIssue {
            line: 0,
            column: 0,
            message: "未闭合的代码块".to_string(),
            severity: IssueSeverity::Error,
        });
    }

    // 检查标题格式
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // 检查标题格式
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            if hash_count > 6 {
                issues.push(ValidationIssue {
                    line: i + 1,
                    column: 0,
                    message: format!("标题级别 {} 超过最大级别 6", hash_count),
                    severity: IssueSeverity::Warning,
                });
            }

            // 检查标题后是否有空格
            if hash_count < trimmed.len() && trimmed.chars().nth(hash_count) != Some(' ') {
                issues.push(ValidationIssue {
                    line: i + 1,
                    column: hash_count,
                    message: "标题标记后应有一个空格".to_string(),
                    severity: IssueSeverity::Warning,
                });
            }
        }

        // 检查链接格式
        if trimmed.contains("](") && !trimmed.contains(")") {
            issues.push(ValidationIssue {
                line: i + 1,
                column: 0,
                message: "可能未闭合的链接".to_string(),
                severity: IssueSeverity::Warning,
            });
        }
    }

    let warning_count = issues
        .iter()
        .filter(|i| matches!(i.severity, IssueSeverity::Warning))
        .count();
    let error_count = issues.len() - warning_count;

    Ok(ValidationResult {
        valid: error_count == 0,
        issues,
        warning_count,
        error_count,
    })
}

#[tauri::command]
pub async fn markdown_clean(content: String) -> Result<String, MarkdownError> {
    // 简单的清理函数，移除多余空行和空格
    let lines: Vec<&str> = content.lines().collect();
    let mut cleaned_lines = Vec::new();
    let mut last_was_blank = false;

    for line in lines {
        let trimmed = line.trim();
        let is_blank = trimmed.is_empty();

        if !is_blank {
            // 修复行内多余空格
            let cleaned_line = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            cleaned_lines.push(cleaned_line);
            last_was_blank = false;
        } else if !last_was_blank {
            cleaned_lines.push("".to_string());
            last_was_blank = true;
        }
    }

    // 移除结尾的空行
    while !cleaned_lines.is_empty() && cleaned_lines.last() == Some(&"".to_string()) {
        cleaned_lines.pop();
    }

    let result = cleaned_lines.join("\n");
    if !result.is_empty() && !result.ends_with('\n') {
        Ok(result + "\n")
    } else {
        Ok(result)
    }
}

#[tauri::command]
pub async fn markdown_stats(content: String) -> Result<MarkdownStats, MarkdownError> {
    let lines: Vec<&str> = content.lines().collect();

    let mut heading_count = 0;
    let mut code_block_count = 0;
    let mut link_count = 0;
    let mut image_count = 0;
    let mut list_count = 0;
    let mut in_code_block = false;

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            code_block_count += 1;
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            continue;
        }

        if trimmed.starts_with('#') {
            heading_count += 1;
        }

        if trimmed.contains("](") && trimmed.contains("![") {
            image_count += 1;
        } else if trimmed.contains("](") {
            link_count += 1;
        }

        if trimmed.starts_with('-')
            || trimmed.starts_with('*')
            || trimmed.starts_with('+')
            || (trimmed.chars().next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('.'))
        {
            list_count += 1;
        }
    }

    // 实际的代码块数量是标记对数
    code_block_count /= 2;

    Ok(MarkdownStats {
        total_lines: lines.len(),
        total_chars: content.len(),
        word_count: content.split_whitespace().count(),
        heading_count,
        code_block_count,
        link_count,
        image_count,
        list_count,
    })
}

// 快速格式化接口 - 使用默认选项
#[allow(dead_code)]
#[tauri::command]
pub async fn markdown_format_quick(content: String) -> Result<FormatResponse, MarkdownError> {
    let request = FormatRequest {
        content,
        options: None,
    };

    markdown_format(request).await
}

// ==================== 数学块格式化回归测试 ====================
#[cfg(test)]
mod math_block_format_tests {
    use super::*;

    fn format_with(options: MarkdownFormatOptions, input: &str) -> String {
        format_markdown_production(input, &options).expect("格式化失败")
    }

    fn default_options() -> MarkdownFormatOptions {
        MarkdownFormatOptions {
            max_line_width: 80,
            ..Default::default()
        }
    }

    /// 正文下一行直接写 `$$`（中间不空行）是最常见的写法。
    /// 格式化后开围栏必须仍在行首，否则阅读器认不出这是块级公式。
    #[test]
    fn display_math_directly_after_text_keeps_its_own_block() {
        let input = r"其中：
$$
\hat{H} = -\frac{\hbar^2}{2m} \nabla^2 + V(\mathbf{r})
$$
";
        let out = format_with(default_options(), input);

        assert!(
            out.contains(r"其中：

$$"),
            "开围栏没有和正文拆开:\n{out}"
        );
        assert!(
            out.contains(r"\hat{H} = -\frac{\hbar^2}{2m} \nabla^2 + V(\mathbf{r})"),
            "公式内容被改写（反斜杠/空格不能变）:\n{out}"
        );
    }

    #[test]
    fn display_math_directly_before_text_is_separated() {
        let input = "$$\nx^2\n$$\n后面这句话\n";
        let out = format_with(default_options(), input);

        assert!(
            out.contains("$$\n\n后面这句话"),
            "闭围栏粘到了后文上:\n{out}"
        );
    }

    /// 行内公式和货币写法不能被当成块级围栏处理。
    #[test]
    fn inline_math_and_currency_stay_intact() {
        let input = "花费 $5 和 $10，行内公式 $x^2$ 保持行内。\n";
        let out = format_with(default_options(), input);

        assert!(out.contains("$5 和 $10"), "货币写法被改动:\n{out}");
        assert!(out.contains("$x^2$"), "行内公式被改动:\n{out}");
    }

    /// 旧版格式化器已经把 `其中：$$` 粘连过一次的文件，再格式化一次应当被拆回块级公式。
    #[test]
    fn repairs_previously_glued_math_opener() {
        let input = r"其中：$$
\hat{H} = -\frac{\hbar^2}{2m} \nabla^2 + V(\mathbf{r})
$$
";
        let out = format_with(default_options(), input);

        assert!(
            out.contains(r"其中：

$$"),
            "粘连的开围栏没有被拆回独立段落:\n{out}"
        );
        assert!(
            out.contains(r"\hat{H} = -\frac{\hbar^2}{2m} \nabla^2 + V(\mathbf{r})"),
            "公式内容被改写:\n{out}"
        );
    }

    /// 代码块里的 `$$` 是普通文本，不能被当成公式围栏拆开。
    #[test]
    fn dollar_fences_inside_code_block_are_untouched() {
        let input = "```text\n$$\nx^2\n$$\n```\n\n正文\n";
        let out = format_with(default_options(), input);

        assert!(
            out.contains("```text\n$$\nx^2\n$$\n```"),
            "代码块里的 $$ 被改动了:\n{out}"
        );
    }

    /// 关掉「格式化列表」会走 SimpleMarkdownFormatter，它也必须原样保留公式，
    /// 尤其不能按行宽折行——LaTeX 里的空格是有语义的。
    #[test]
    fn simple_formatter_keeps_display_math_verbatim() {
        let options = MarkdownFormatOptions {
            format_lists: false,
            max_line_width: 40,
            ..Default::default()
        };
        let input = r"$$
\hat{H} = -\frac{\hbar^2}{2m} \nabla^2 + V(\mathbf{r})
$$
";
        let out = format_with(options, input);

        assert!(
            out.contains(r"\hat{H} = -\frac{\hbar^2}{2m} \nabla^2 + V(\mathbf{r})"),
            "极速格式化把公式折行/改写了:\n{out}"
        );
        assert_eq!(
            out.lines().filter(|line| line.contains(r"\frac")).count(),
            1,
            "公式被拆成了多行:\n{out}"
        );
    }
}

// ==================== 自定义 cell 原样保留的回归测试 ====================
#[cfg(test)]
mod opaque_cell_format_tests {
    use super::*;

    fn format_with(options: MarkdownFormatOptions, input: &str) -> String {
        format_markdown_production(input, &options).expect("格式化失败")
    }

    fn default_options() -> MarkdownFormatOptions {
        MarkdownFormatOptions {
            max_line_width: 80,
            ..Default::default()
        }
    }

    /// Python cell 里是原始 Python，不是 Markdown：缩进、`|`、`*args`、`#` 注释都不能动。
    #[test]
    fn python_cell_content_is_kept_verbatim() {
        let cell = r#"<!--py {"id":"py-1","storage":"cellmeta"}-->
# Python Cell
x = a | b
y = *args
print(f"hello {x}")
<!--/py-->"#;
        let input = format!("正文\n\n{}\n\n后续段落\n", cell);
        let out = format_with(default_options(), &input);

        assert!(out.contains(cell), "Python cell 内容被改写:\n{out}");
    }

    #[test]
    fn math_cell_content_is_kept_verbatim() {
        let cell = r#"<!--math {"id":"math-1","storage":"cellmeta"}-->
$$
\begin{aligned}
\nabla \cdot \mathbf{E} &= \frac{\rho}{\varepsilon_0}
\end{aligned}
$$
<!--/math-->"#;
        let input = format!("正文\n\n{}\n\n后续段落\n", cell);
        let out = format_with(default_options(), &input);

        assert!(out.contains(cell), "Math cell 内容被改写:\n{out}");
    }

    #[test]
    fn empty_math_cell_keeps_markers_adjacent() {
        let cell = r#"<!--math {"id":"math-1","storage":"cellmeta"}-->
<!--/math-->"#;
        let input = format!("正文\n\n{}\n", cell);
        let out = format_with(default_options(), &input);

        assert!(out.contains(cell), "空 cell 的起止标记被拆开:\n{out}");
    }

    #[test]
    fn python_output_block_is_kept_verbatim() {
        let cell = r#"<!-- guaxi:pyout:abc:start -->
输出内容
- 列表项
<!-- guaxi:pyout:abc:end -->"#;
        let input = format!("正文\n\n{}\n", cell);
        let out = format_with(default_options(), &input);

        assert!(out.contains(cell), "输出块内容被改写:\n{out}");
    }

    /// 保护只针对 cell：cell 之外的正文仍然要正常格式化。
    #[test]
    fn text_outside_cells_is_still_formatted() {
        let input = "#    标题\n\n普通段落\n\n<!--py-->\nx = 1\n<!--/py-->\n";
        let out = format_with(default_options(), input);

        assert!(out.contains("# 标题"), "cell 外的标题没有被格式化:\n{out}");
        assert!(
            out.contains("<!--py-->\nx = 1\n<!--/py-->"),
            "cell 内容被改写:\n{out}"
        );
    }

    /// 未闭合的 cell（用户还在写）：宁可少格式化，也不能把后面的代码重新排版。
    #[test]
    fn unterminated_cell_is_preserved_to_end_of_file() {
        let input = "正文\n\n<!--py-->\ndef f(*args):\n    return args\n";
        let out = format_with(default_options(), input);

        assert!(
            out.contains("<!--py-->\ndef f(*args):\n    return args"),
            "未闭合的 cell 被重新排版:\n{out}"
        );
    }

    /// 关掉「格式化列表」走的极速路径同样要跳过 cell。
    #[test]
    fn simple_formatter_path_also_keeps_cells() {
        let options = MarkdownFormatOptions {
            format_lists: false,
            max_line_width: 40,
            ..Default::default()
        };
        let cell = r#"<!--py-->
x = a | b
y = *args
<!--/py-->"#;
        let input = format!("{}\n", cell);
        let out = format_with(options, &input);

        assert!(out.contains(cell), "极速格式化改写了 cell:\n{out}");
    }

    // ───── 图表 / 白板围栏：整块不透明，格式化不能改动其中任何一个字节 ─────
    //
    // 围栏里是 YAML 风格的 spec 与内联 CSV/TSV。行尾空格、缩进层级、`|` 分隔符
    // 都是语义的一部分，改动任意一处都会让图表静默变空或画错。

    const CHART_FENCE: &str = r#"```chart
spec:
  type: line
  x: month
  y: [revenue, cost]
  title: 月度趋势
data: |
  month,revenue,cost
  一月,120,80
  二月,150,95
```"#;

    const EXCALIDRAW_FENCE: &str = r#"```excalidraw
./assets/architecture.excalidraw
```"#;

    #[test]
    fn chart_fence_is_preserved_verbatim() {
        let input = format!("# 标题\n\n{CHART_FENCE}\n\n正文段落。\n");
        let out = format_with(default_options(), input.as_str());
        assert!(out.contains(CHART_FENCE), "图表围栏被改写:\n{out}");
    }

    #[test]
    fn excalidraw_fence_is_preserved_verbatim() {
        let input = format!("{EXCALIDRAW_FENCE}\n");
        let out = format_with(default_options(), input.as_str());
        assert!(out.contains(EXCALIDRAW_FENCE), "白板围栏被改写:\n{out}");
    }

    /// 围栏内容看起来像 Markdown（标题 / 列表 / 竖线）时也不能被当成正文重排。
    #[test]
    fn chart_content_that_looks_like_markdown_is_not_reformatted() {
        let fence = "```chart\nspec:\n  title: \"# 不是标题\"\n  x: \"- 不是列表\"\ndata: |\n  a|b\n  -1,2\n```";
        let input = format!("{fence}\n");
        let out = format_with(default_options(), &input);
        assert!(out.contains(fence), "图表内容被当成正文重排:\n{out}");
    }

    /// YAML 块标量里的行尾空格是有意义的：关掉「保留代码块」也不能裁掉。
    #[test]
    fn chart_fence_keeps_trailing_spaces_even_without_preserve_code_blocks() {
        let fence = "```chart\nspec:\n  type: line\ndata: |\n  a,b  \n  1,2  \n```";
        let options = MarkdownFormatOptions {
            preserve_code_blocks: false,
            ..default_value_for_chart_tests()
        };
        let out = format_with(options, fence);
        assert!(out.contains(fence), "行尾空格被裁掉:\n{out:?}");
    }

    /// `~~~~chart` 与带属性的 info string 同样要按图表块处理。
    #[test]
    fn tilde_and_attributed_chart_fences_are_preserved() {
        let tilde = "~~~chart\nspec:\n  type: line\ndata: |\n  月,值\n  一月,1\n~~~";
        let attributed = "```chart width=800\nspec:\n  type: line\n```";
        let input = format!("{tilde}\n\n{attributed}\n");
        let out = format_with(default_options(), input.as_str());
        assert!(out.contains(tilde), "~~~chart 围栏被改写:\n{out}");
        assert!(out.contains(attributed), "带属性的图表围栏被改写:\n{out}");
    }

    /// 极速路径（关闭「格式化列表」）也必须跳过图表围栏。
    /// 这条曾经真的会坏：那条路径只认 ``` 围栏，`~~~chart` 会被按普通文本重排。
    #[test]
    fn simple_formatter_path_also_keeps_chart_fences() {
        let options = MarkdownFormatOptions {
            format_lists: false,
            max_line_width: 40,
            ..default_value_for_chart_tests()
        };
        let tilde = "~~~chart\nspec:\n  type: line\ndata: |\n  月,值\n  一月,1\n~~~";
        let out = format_with(options.clone(), &format!("{tilde}\n"));
        assert!(out.contains(tilde), "极速路径改写了 ~~~chart:\n{out}");

        let out = format_with(options, &format!("{CHART_FENCE}\n"));
        assert!(out.contains(CHART_FENCE), "极速路径改写了图表围栏:\n{out}");
    }

    /// 未闭合的图表围栏（用户还在写）：延续到文末整块保留，不能重排后面的内容。
    #[test]
    fn unterminated_chart_fence_is_preserved_to_end_of_file() {
        let input = "正文\n\n```chart\nspec:\n  type: line\n  # 注释\n";
        let out = format_with(default_options(), input);
        assert!(
            out.contains("```chart\nspec:\n  type: line\n  # 注释"),
            "未闭合的图表围栏被重排:\n{out}"
        );
    }

    fn default_value_for_chart_tests() -> MarkdownFormatOptions {
        MarkdownFormatOptions {
            max_line_width: 80,
            ..Default::default()
        }
    }
}

