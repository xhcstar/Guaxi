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

    if !options.format_lists {
        let formatted = SimpleMarkdownFormatter::new(options.clone()).format(content)?;
        return Ok(apply_line_ending(&formatted, line_ending));
    }

    let formatted = format_with_comrak(content, options)?;
    let normalized = postprocess_markdown(&formatted, options);
    Ok(apply_line_ending(&normalized, line_ending))
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

    comrak_options.render.width = if options.max_line_width == 0 { 0 } else { options.max_line_width };

    let root = parse_document(&arena, content, &comrak_options);
    let mut out = String::with_capacity(content.len().saturating_add(128));

    format_commonmark(root, &comrak_options, &mut out)
        .map_err(|e| MarkdownError::FormatError(e.to_string()))?;

    Ok(out)
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
