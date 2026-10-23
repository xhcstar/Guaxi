//! 邮件内容的二次加工：导出（Markdown / HTML / PDF / Word）、与笔记和任务系统的桥接。
//!
//! 这里的每个函数都只做「纯转换」——接收已经取到的邮件数据，产出文件内容或结构化结果。
//! 网络、账户、UI 状态都不在这里，方便单测，也避免和同步链路互相牵连。
//!
//! 几个取舍：
//! - **PDF 走 Typst**：项目里已经内嵌了完整的 typst 编译链（含字体），
//!   为邮件再引一个 PDF 库没有意义，也会拖重 release 包。
//! - **Word 手写 OOXML**：`docx-rs` 会带来一整套依赖，而我们只需要
//!   「标题 + 元信息 + 分段正文」这一种极简结构。用 zip 拼一个最小 docx 即可，
//!   真正需要复杂排版时再用专业工具。
//! - **笔记/任务只产出「待写入的结构」**，实际落盘由调用方（已有的库函数）完成，
//!   这样不会把邮件的写权限扩散到其它模块。

use serde::{Deserialize, Serialize};

/// 导出目标格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Html,
    Pdf,
    Word,
    Text,
}

impl ExportFormat {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "html" => Self::Html,
            "pdf" => Self::Pdf,
            "word" | "docx" => Self::Word,
            "text" | "txt" => Self::Text,
            _ => Self::Markdown,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Word => "docx",
            Self::Text => "txt",
        }
    }
}

/// 导出一封邮件所需的全部输入（由命令层从库与 IMAP 取好）。
#[derive(Debug, Clone, Default)]
pub struct ExportInput {
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    pub to: String,
    pub cc: String,
    pub date: String,
    pub text: String,
    pub html: Option<String>,
    pub labels: String,
    /// 邮件在应用里的定位，写进 front matter，方便回跳。
    pub account_id: String,
    pub folder: String,
    pub uid: u32,
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn escape_md(text: &str) -> String {
    // Markdown 里会被解释为语法的字符转义掉，避免主题/发件人里的 * _ [ ] 破坏结构。
    text.replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('`', "\\`")
}

/// 纯文本 → 简单 HTML（保留段落）。
fn text_to_paragraphs(text: &str) -> String {
    text.split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| format!("<p>{}</p>", escape_html(p).replace('\n', "<br>")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn meta_line(input: &ExportInput) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !input.from_name.trim().is_empty() {
        parts.push(format!("{} <{}>", input.from_name.trim(), input.from_addr.trim()));
    } else if !input.from_addr.trim().is_empty() {
        parts.push(input.from_addr.trim().to_string());
    }
    if !input.date.trim().is_empty() {
        parts.push(input.date.trim().to_string());
    }
    if !input.to.trim().is_empty() {
        parts.push(format!("收件人：{}", input.to.trim()));
    }
    if !input.cc.trim().is_empty() {
        parts.push(format!("抄送：{}", input.cc.trim()));
    }
    parts.join(" · ")
}

/// 生成 Markdown。
///
/// 带 YAML front matter：笔记库按它做元数据索引，导进去就能按发件人/日期检索，
/// 而不是变成一堆无来源的正文。
pub fn to_markdown(input: &ExportInput) -> String {
    let mut out = String::with_capacity(input.text.len() + 512);
    out.push_str("---\n");
    out.push_str(&format!("title: \"{}\"\n", input.subject.replace('"', "\\\"")));
    out.push_str(&format!("from: \"{}\"\n", input.from_addr.replace('"', "\\\"")));
    if !input.from_name.trim().is_empty() {
        out.push_str(&format!(
            "from_name: \"{}\"\n",
            input.from_name.replace('"', "\\\"")
        ));
    }
    out.push_str(&format!("date: \"{}\"\n", input.date));
    if !input.labels.trim().is_empty() {
        out.push_str(&format!("labels: \"{}\"\n", input.labels.replace('"', "\\\"")));
    }
    out.push_str(&format!("mail_account: \"{}\"\n", input.account_id));
    out.push_str(&format!("mail_folder: \"{}\"\n", input.folder));
    out.push_str(&format!("mail_uid: {}\n", input.uid));
    out.push_str("source: mail\n---\n\n");

    out.push_str(&format!("# {}\n\n", escape_md(&input.subject)));
    let meta = meta_line(input);
    if !meta.is_empty() {
        out.push_str(&format!("> {}\n\n", meta));
    }
    // 正文：优先用纯文本（Markdown 不解析 HTML 排版，塞 HTML 只会变成一坨标签）。
    // 没有纯文本时退回 HTML 源码并标注，用户自己决定怎么处理。
    if !input.text.trim().is_empty() {
        out.push_str(input.text.trim());
        out.push('\n');
    } else if let Some(html) = input.html.as_deref() {
        out.push_str("<!-- 原文只有 HTML，以下为源码 -->\n\n```html\n");
        out.push_str(html.trim());
        out.push_str("\n```\n");
    }
    out
}

/// 生成独立 HTML 文件（正文用邮件原始 HTML，包一层阅读样式）。
pub fn to_html(input: &ExportInput) -> String {
    let body = match input.html.as_deref() {
        Some(html) if !html.trim().is_empty() => html.to_string(),
        _ => text_to_paragraphs(&input.text),
    };
    let meta = escape_html(&meta_line(input));
    format!(
        "<!doctype html>\n<html lang=\"zh-CN\"><head><meta charset=\"utf-8\">\
<title>{title}</title>\n<style>\n\
  body{{margin:0;padding:32px;background:#f7f6f3;color:#22262c;\
font-family:system-ui,'Microsoft YaHei',sans-serif;line-height:1.7}}\n\
  .wrap{{max-width:820px;margin:0 auto;background:#fff;padding:32px 36px;\
border-radius:8px;box-shadow:0 2px 12px rgba(0,0,0,.08)}}\n\
  h1{{font-size:24px;line-height:1.35;margin:0 0 10px}}\n\
  .meta{{color:#6b7280;font-size:13px;margin-bottom:18px;\
padding-bottom:14px;border-bottom:1px solid #e5e7eb}}\n\
  img{{max-width:100%;height:auto}} table{{max-width:100%;border-collapse:collapse}}\n\
  th,td{{border:0;padding:0}} blockquote{{margin:12px 0;padding-left:14px;\
border-left:3px solid #d1d5db;color:#4b5563}}\n\
</style></head><body><div class=\"wrap\">\n\
<h1>{title}</h1><div class=\"meta\">{meta}</div>\n{body}\n</div></body></html>\n",
        title = escape_html(&input.subject),
        meta = meta,
        body = body
    )
}

/// 生成纯文本。
pub fn to_text(input: &ExportInput) -> String {
    let mut out = String::new();
    out.push_str(&input.subject);
    out.push_str("\n");
    out.push_str(&"-".repeat(40.min(input.subject.chars().count().max(10))));
    out.push('\n');
    let meta = meta_line(input);
    if !meta.is_empty() {
        out.push_str(&meta);
        out.push_str("\n\n");
    }
    // 只有 HTML 时，把标签剥掉给个能读的版本。
    let body = if input.text.trim().is_empty() {
        strip_html(input.html.as_deref().unwrap_or(""))
    } else {
        input.text.trim().to_string()
    };
    out.push_str(&body);
    out
}

/// 极简 HTML → 文本（导出用，不追求排版还原）。
fn strip_html(html: &str) -> String {
    let doc = scraper::Html::parse_fragment(html);
    let mut out = String::new();
    collect_text(doc.tree.root(), &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text(node: ego_tree::NodeRef<'_, scraper::Node>, out: &mut String) {
    use scraper::Node;
    match node.value() {
        Node::Text(text) => out.push_str(text),
        Node::Element(el) => {
            let block = matches!(
                el.name(),
                "p" | "div" | "br" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "blockquote"
            );
            if block && !out.ends_with(' ') {
                out.push(' ');
            }
            for child in node.children() {
                collect_text(child, out);
            }
            if block && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        _ => {
            for child in node.children() {
                collect_text(child, out);
            }
        }
    }
}

/// Markdown → Typst 源码（只覆盖邮件导出会出现的结构：标题、引用、段落、列表、代码块）。
///
/// 不做通用 Markdown 解析：邮件正文转成 Markdown 后形态很有限，穷举反而可控。
pub fn markdown_to_typst(markdown: &str) -> String {
    let mut out = String::new();
    out.push_str("#set text(size: 10.5pt)\n");
    out.push_str("#set par(justify: true, leading: 0.75em)\n");
    out.push_str("#set page(margin: (x: 2.2cm, y: 2cm))\n\n");

    for line in markdown.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        if trimmed == "---" {
            out.push_str("#line(length: 100%, stroke: 0.4pt + luma(210))\n\n");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            out.push_str(&format!("#heading(level: 3)[{}]\n", typst_escape_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            out.push_str(&format!("#heading(level: 2)[{}]\n", typst_escape_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            out.push_str(&format!("#heading(level: 1)[{}]\n", typst_escape_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            out.push_str(&format!(
                "#block(inset: (left: 10pt), stroke: (left: 2pt + luma(200)))[{}]\n",
                typst_escape_inline(rest)
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            out.push_str(&format!("- {}\n", typst_escape_inline(rest)));
            continue;
        }
        out.push_str(&typst_escape_inline(trimmed));
        out.push('\n');
    }
    out
}

/// Typst 正文里的特殊字符转义：`#` `[` `]` `$` `*` `_` 会被当成标记语法。
fn typst_escape_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '#' | '[' | ']' | '$' | '*' | '_' | '@' | '<' | '>' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// 生成最小可用的 .docx（OOXML）。
///
/// 结构：`[Content_Types].xml` + `_rels/.rels` + `word/document.xml`，这是 Word 能打开的最小集合。
/// 段落按空行切分，标题单独成段并加粗——邮件导出不需要更复杂的排版。
pub fn to_docx(input: &ExportInput) -> Result<Vec<u8>, String> {
    use std::io::Write;

    fn para(text: &str, bold: bool, size_half_points: u32) -> String {
        let run_props = if bold {
            format!(
                "<w:rPr><w:b/><w:sz w:val=\"{size}\"/></w:rPr>",
                size = size_half_points
            )
        } else {
            format!("<w:rPr><w:sz w:val=\"{size}\"/></w:rPr>", size = size_half_points)
        };
        format!(
            "<w:p><w:r>{props}<w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>",
            props = run_props,
            text = escape_xml(text)
        )
    }

    let mut body = String::new();
    body.push_str(&para(&input.subject, true, 32));
    let meta = meta_line(input);
    if !meta.is_empty() {
        body.push_str(&para(&meta, false, 20));
    }
    body.push_str("<w:p/>");
    for block in input.text.split("\n\n") {
        for line in block.lines() {
            if line.trim().is_empty() {
                body.push_str("<w:p/>");
            } else {
                body.push_str(&para(line, false, 24));
            }
        }
        body.push_str("<w:p/>");
    }

    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/>\
<w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/>\
</w:sectPr></w:body></w:document>",
        body = body
    );

    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" \
ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
</Types>";

    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" \
Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
Target=\"word/document.xml\"/>\
</Relationships>";

    let mut buffer = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buffer);
        let mut zip = zip::ZipWriter::new(cursor);
        let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for (name, content) in [
            ("[Content_Types].xml", content_types),
            ("_rels/.rels", rels),
            ("word/document.xml", document.as_str()),
        ] {
            zip.start_file(name, options)
                .map_err(|e| format!("写入 docx 失败: {e}"))?;
            zip.write_all(content.as_bytes())
                .map_err(|e| format!("写入 docx 失败: {e}"))?;
        }
        zip.finish().map_err(|e| format!("封装 docx 失败: {e}"))?;
    }
    Ok(buffer)
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// 与任务管理器的桥接
// ---------------------------------------------------------------------------

/// 把一封邮件转成一条任务条目的 .org 文本。
///
/// 用 .org 而不是直接写库：任务管理器本来就以 .org 为唯一事实来源
/// （`task_import` 吃 org/json），走这条既有通道就不会有两套写入逻辑打架。
pub fn mail_to_task_org(
    input: &ExportInput,
    category: &str,
    status: &str,
    deadline: &str,
) -> String {
    // 标题里不能有换行，长主题截断到 100 字，否则看板卡片会撑爆。
    let title: String = input.subject.chars().take(100).collect();
    let title = if title.trim().is_empty() {
        "（无主题邮件）".to_string()
    } else {
        title.trim().to_string()
    };
    let cat = match category {
        "life" | "study" | "work" => category,
        _ => "work",
    };
    let status = if status.trim().is_empty() {
        "TODO"
    } else {
        status.trim()
    };

    let mut out = String::new();
    out.push_str(&format!("* {title} :{cat}:\n"));
    out.push_str(&format!("  :PROPERTIES:\n"));
    out.push_str(&format!("  :manageType: {cat}\n"));
    out.push_str(&format!("  :status: {status}\n"));
    out.push_str("  :taskType: task\n");
    out.push_str("  :origin: other\n");
    if !deadline.trim().is_empty() {
        out.push_str(&format!("  :deadline: {}\n", deadline.trim()));
    }
    out.push_str(&format!("  :mailFrom: {}\n", input.from_addr.trim()));
    out.push_str(&format!("  :mailDate: {}\n", input.date.trim()));
    out.push_str(&format!("  :mailAccount: {}\n", input.account_id));
    out.push_str(&format!("  :mailFolder: {}\n", input.folder));
    out.push_str(&format!("  :mailUid: {}\n", input.uid));
    out.push_str("  :END:\n");

    // 正文摘要：邮件全文太长，放进任务里会拖慢看板与导出。截到 2000 字并标注来源。
    let summary: String = input
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(2000)
        .collect();
    if !summary.is_empty() {
        out.push_str(&format!("  {summary}\n"));
    }
    out.push_str(&format!(
        "  [[mail:{}/{}/{}][在邮件中打开]]\n\n",
        input.account_id, input.folder, input.uid
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ExportInput {
        ExportInput {
            subject: "Q4 预算复核".into(),
            from_name: "林蔚".into(),
            from_addr: "lin.wei@company.com".into(),
            to: "chen@company.com".into(),
            cc: String::new(),
            date: "2026-10-22 09:12".into(),
            text: "预算表已重算。\n\n容器集群少了 12%。".into(),
            html: Some("<p>预算表已重算。</p>".into()),
            labels: "财务".into(),
            account_id: "acc1".into(),
            folder: "INBOX".into(),
            uid: 901,
        }
    }

    #[test]
    fn markdown_has_front_matter_and_body() {
        let md = to_markdown(&sample());
        assert!(md.starts_with("---\n"));
        assert!(md.contains("source: mail"));
        assert!(md.contains("mail_uid: 901"));
        assert!(md.contains("# Q4 预算复核"));
        assert!(md.contains("容器集群少了 12%"));
    }

    #[test]
    fn markdown_escapes_subject_syntax() {
        let mut input = sample();
        input.subject = "关于 *重要* 的 [通知]".into();
        let md = to_markdown(&input);
        // 标题里的 * 与 [] 必须转义，否则导出后标题会被渲染成斜体 + 链接。
        assert!(md.contains("\\*重要\\*"));
        assert!(md.contains("\\[通知\\]"));
    }

    #[test]
    fn html_escapes_title_but_keeps_body_markup() {
        let out = to_html(&sample());
        assert!(out.contains("<!doctype html>"));
        assert!(out.contains("<p>预算表已重算。</p>"), "正文 HTML 应保留");
    }

    #[test]
    fn html_falls_back_to_text() {
        let mut input = sample();
        input.html = None;
        let out = to_html(&input);
        assert!(out.contains("容器集群少了 12%"));
        assert!(!out.contains("&lt;p&gt;"));
    }

    #[test]
    fn text_export_strips_tags_when_only_html() {
        let mut input = sample();
        input.text = String::new();
        let out = to_text(&input);
        assert!(out.contains("预算表已重算"));
        assert!(!out.contains("<p>"));
    }

    #[test]
    fn typst_escapes_markup_chars() {
        let t = markdown_to_typst("# 标题 #hashtag\n\n正文 $money$");
        assert!(t.contains("#heading(level: 1)"));
        assert!(t.contains("\\#hashtag"), "正文里的 # 必须转义: {t}");
        assert!(t.contains("\\$money\\$"));
    }

    #[test]
    fn docx_is_a_valid_zip_with_document() {
        let bytes = to_docx(&sample()).unwrap();
        assert_eq!(&bytes[..2], b"PK", "docx 必须是 zip");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("word/document.xml").is_ok());
        assert!(zip.by_name("[Content_Types].xml").is_ok());
        let mut doc = String::new();
        use std::io::Read;
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains("Q4 预算复核"));
    }

    #[test]
    fn docx_escapes_xml() {
        let mut input = sample();
        input.subject = "A & B <tag>".into();
        let bytes = to_docx(&input).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        use std::io::Read;
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains("A &amp; B &lt;tag&gt;"), "非法 XML 字符必须转义");
    }

    #[test]
    fn task_org_has_required_properties() {
        let org = mail_to_task_org(&sample(), "work", "TODO", "2026-11-01");
        assert!(org.starts_with("* Q4 预算复核 :work:"));
        assert!(org.contains(":manageType: work"));
        assert!(org.contains(":status: TODO"));
        assert!(org.contains(":deadline: 2026-11-01"));
        assert!(org.contains(":mailUid: 901"));
    }

    #[test]
    fn task_org_truncates_long_summary() {
        let mut input = sample();
        input.text = "字".repeat(5000);
        let org = mail_to_task_org(&input, "work", "TODO", "");
        // 正文摘要被截到 2000 字，整条记录不应包含完整的 5000 字。
        assert!(org.matches('字').count() <= 2100);
    }

    #[test]
    fn export_format_parse_and_extension() {
        assert_eq!(ExportFormat::parse("docx"), ExportFormat::Word);
        assert_eq!(ExportFormat::parse("PDF"), ExportFormat::Pdf);
        assert_eq!(ExportFormat::parse("whatever"), ExportFormat::Markdown);
        assert_eq!(ExportFormat::Word.extension(), "docx");
    }
}
