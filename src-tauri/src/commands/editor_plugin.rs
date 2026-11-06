use crate::guaxiservices::bundled_resources;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::command;

/// 用户可编写的软件级插件。
/// 插件目录:utils/guaxi_editor_plugins/<id>/,内含 manifest.json + 入口脚本(默认 main.py)。
/// 契约:入口脚本定义 `def run(ctx): return {...}`;不同作用面(surfaces)给的 ctx 不同。
/// 现已实装 editor 作用面(transform);repo/knowledge-base/agent 先解析保留,逐步接线。
/// 启用/禁用状态持久化在插件根目录的 _state.json({"disabled": ["id", ...]})。
#[derive(Serialize, Debug, Clone)]
pub struct PluginArgSpec {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub default: serde_json::Value,
    pub options: Vec<String>,
    pub placeholder: String,
    pub required: bool,
    /// `file` / `folder` 类型用:允许的扩展名,宿主据此过滤文件选择器。
    pub accept: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct EditorPluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub surfaces: Vec<String>,
    pub scope: String,
    pub dir: String,
    pub entry: String,
    pub entry_path: String,
    pub enabled: bool,
    /// manifest 声明的参数；`arg_values` 是「默认值 + 用户保存值」合并后的结果。
    pub args: Vec<PluginArgSpec>,
    pub arg_values: serde_json::Map<String, serde_json::Value>,
    /// 声明为 true 时运行前读取系统剪贴板并注入 ctx.clipboard。
    pub clipboard: bool,
    /// 允许在空选区（光标处）运行；结果插入光标位置。
    pub allow_empty_selection: bool,
    /// 运行前需要当前知识库路径（编辑器会按需加载知识库列表）。
    pub needs_kb: bool,
    /// 强制每次运行都弹出参数面板；未设置时由前端按参数类型判断。
    pub prompt_on_run: bool,
}

#[derive(Serialize)]
pub struct PluginEntrySource {
    pub id: String,
    pub entry: String,
    pub path: String,
    pub content: String,
}

fn editor_plugins_dir() -> Result<PathBuf, String> {
    let dir = bundled_resources::ensure_relative_dir("utils/guaxi_editor_plugins")?;
    fs::create_dir_all(&dir).map_err(|e| format!("create editor plugins dir failed: {e}"))?;
    Ok(dir)
}

// PLACEHOLDER_CONSTS

const EXAMPLE_PLUGIN_MANIFEST: &str = r#"{
  "id": "pangu-spacing",
  "name": "中英文加空格",
  "description": "在中文与英文/数字之间自动插入空格,让排版更整齐(示例插件,可复制修改)",
  "entry": "main.py",
  "capabilities": ["transform"],
  "surfaces": ["editor"],
  "scope": "selection"
}
"#;

const EXAMPLE_PLUGIN_MAIN: &str = r#"# Guaxi 示例插件:中英文之间自动加空格(pangu 风格)。
# 契约:实现 run(ctx) 并返回 {"text": 新文本}。ctx 提供:
#   ctx["selection"] 当前选中文本 / ctx["document"] 全文 / ctx["args"] 预留参数
import re

_CJK = "一-鿿぀-ヿ㐀-䶿"


def _pangu(text):
    text = re.sub("([" + _CJK + "])([A-Za-z0-9])", r"\1 \2", text)
    text = re.sub("([A-Za-z0-9])([" + _CJK + "])", r"\1 \2", text)
    return text


def run(ctx):
    selection = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    return {"text": _pangu(selection)}
"#;

const NEW_PLUGIN_MAIN_TEMPLATE: &str = r#"# Guaxi 插件入口。实现 run(ctx) 并返回结果字典。
# editor 作用面契约:ctx 提供 selection/document/args,返回 {"text": 新文本} 会替换选区。
def run(ctx):
    selection = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    # TODO: 在这里处理 selection,返回替换文本
    return {"text": selection}
"#;

const REPO_EXAMPLE_MANIFEST: &str = r#"{
  "id": "repo-health-report",
  "name": "库健康报告",
  "description": "统计当前库的资源数、类型分布、标签覆盖(只读示例)",
  "entry": "main.py",
  "capabilities": ["batch"],
  "surfaces": ["repo"],
  "scope": "library"
}
"#;

const REPO_EXAMPLE_MAIN: &str = r###"# repo(文件库)作用面示例:ctx["resources"] 是资源元数据列表,返回 {"report": markdown}。
from collections import Counter


def run(ctx):
    res = ctx.get("resources", []) if isinstance(ctx, dict) else []
    types = Counter((r.get("type") or "未知") for r in res)
    tagged = sum(1 for r in res if r.get("tags"))
    lines = ["# 库健康报告", "", "资源总数: %d" % len(res), "已打标签: %d" % tagged, "", "## 类型分布"]
    for t, c in types.most_common():
        lines.append("- %s: %d" % (t, c))
    return {"report": "\n".join(lines)}
"###;

const KB_EXAMPLE_MANIFEST: &str = r#"{
  "id": "kb-stats",
  "name": "知识库统计",
  "description": "统计知识库文件数与 Markdown 字数(只读示例,读 kb_location)",
  "entry": "main.py",
  "capabilities": ["generator"],
  "surfaces": ["knowledge-base"],
  "scope": "kb"
}
"#;

const KB_EXAMPLE_MAIN: &str = r###"# knowledge-base 作用面示例:ctx["kb_location"] 是知识库目录,遍历统计,返回 {"report": markdown}。
import os


def run(ctx):
    loc = ctx.get("kb_location", "") if isinstance(ctx, dict) else ""
    n_files = 0
    md = 0
    chars = 0
    if loc and os.path.isdir(loc):
        for root, _dirs, files in os.walk(loc):
            for f in files:
                n_files += 1
                if f.lower().endswith(".md"):
                    md += 1
                    try:
                        with open(os.path.join(root, f), "r", encoding="utf-8", errors="ignore") as fh:
                            chars += len(fh.read())
                    except Exception:
                        pass
    lines = ["# 知识库统计", "", "文件总数: %d" % n_files, "Markdown 笔记: %d" % md, "Markdown 总字数(约): %d" % chars]
    return {"report": "\n".join(lines)}
"###;

// ===== 内置实用插件(随应用提供,首次运行各写入一次,用户可自由编辑/删除)=====

const BUILTIN_MD_TABLE_TOTAL: &str = r###"# Markdown 表格:规范对齐,并为纯数值列追加「合计」行。
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re


def _split_row(line):
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|"):
        s = s[:-1]
    return [c.strip() for c in s.split("|")]


def _is_sep(cells):
    return len(cells) > 0 and all(re.fullmatch(r":?-{1,}:?", c or "") for c in cells)


def _to_num(x):
    t = (x or "").replace(",", "").replace("%", "").strip()
    try:
        return float(t)
    except Exception:
        return None


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    lines = [l for l in text.splitlines() if l.strip()]
    if len(lines) < 2:
        return {"text": text}
    rows = [_split_row(l) for l in lines]
    if not _is_sep(rows[1]):
        return {"text": text}
    header = rows[0]
    ncol = len(header)
    body = [(r + [""] * ncol)[:ncol] for r in rows[2:]]
    totals = [""] * ncol
    has_total = False
    if body:
        for c in range(ncol):
            parsed = [_to_num(r[c]) for r in body]
            if parsed and all(v is not None for v in parsed):
                totals[c] = "%g" % sum(parsed)
                has_total = True
    if has_total:
        if not totals[0]:
            totals[0] = "合计"
        body = body + [totals]
    widths = [len(header[c]) for c in range(ncol)]
    for r in body:
        for c in range(ncol):
            widths[c] = max(widths[c], len(r[c]))

    def fmt(cells):
        return "| " + " | ".join(cells[c].ljust(widths[c]) for c in range(ncol)) + " |"

    out = [fmt(header), "| " + " | ".join("-" * max(3, widths[c]) for c in range(ncol)) + " |"]
    for r in body:
        out.append(fmt(r))
    return {"text": "\n".join(out)}
"###;

const BUILTIN_CODE_TIDY: &str = r###"# 代码块整理:对选区内每个 ``` 围栏块,去行尾空白、Tab 转 4 空格;json 块美化缩进。
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re
import json

_FENCE = re.compile(r"(^|\n)(```|~~~)([^\n`~]*)\n(.*?)\n(\2)(?=\n|$)", re.DOTALL)


def _tidy_block(lang, code):
    code = code.replace("\t", "    ")
    code = "\n".join(line.rstrip() for line in code.split("\n"))
    l = (lang or "").strip().lower()
    if l in ("json", "jsonc"):
        try:
            code = json.dumps(json.loads(code), ensure_ascii=False, indent=2)
        except Exception:
            pass
    return code


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""

    def repl(m):
        return m.group(1) + m.group(2) + m.group(3) + "\n" + _tidy_block(m.group(3), m.group(4)) + "\n" + m.group(5)

    return {"text": _FENCE.sub(repl, text)}
"###;

const BUILTIN_CITE_TIDY: &str = r###"# 参考文献整理:逐行去掉原有项目符号/编号、折叠多余空白,统一重新编号为 [1][2]...
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    items = []
    for raw in text.splitlines():
        s = raw.strip()
        if not s:
            continue
        s = re.sub(r"^\s*(?:[-*+]|\[\d+\]|\(?\d+[.)\]]|\d+、)\s*", "", s)
        s = re.sub(r"\s+", " ", s).strip()
        if s:
            items.append(s)
    if not items:
        return {"text": text}
    return {"text": "\n".join("[%d] %s" % (i + 1, it) for i, it in enumerate(items))}
"###;

const BUILTIN_DATA_SUMMARY: &str = r###"# 数据摘要:从选区中抽取所有数字,算计数/求和/均值/中位数/极值/标准差,
# 在选区下方追加一张 Markdown 摘要表(保留原选区内容)。
# editor transform 契约:返回 {"text": 新文本} 替换选区。
import re
import math


def _nums(text):
    out = []
    for tok in re.split(r"[\s,;]+", text.strip()):
        try:
            out.append(float(tok.replace("%", "")))
        except Exception:
            pass
    return out


def run(ctx):
    text = ctx.get("selection", "") if isinstance(ctx, dict) else ""
    xs = _nums(text)
    if not xs:
        return {"text": text}
    n = len(xs)
    srt = sorted(xs)
    mid = n // 2
    median = srt[mid] if n % 2 else (srt[mid - 1] + srt[mid]) / 2
    mean = sum(xs) / n
    std = math.sqrt(sum((x - mean) ** 2 for x in xs) / n)

    def g(v):
        return "%g" % v

    rows = [
        ("计数", str(n)), ("求和", g(sum(xs))), ("均值", g(mean)), ("中位数", g(median)),
        ("最小", g(min(xs))), ("最大", g(max(xs))), ("标准差(总体)", g(std)),
    ]
    table = ["| 指标 | 值 |", "| --- | --- |"] + ["| %s | %s |" % (k, v) for k, v in rows]
    return {"text": text.rstrip() + "\n\n" + "\n".join(table) + "\n"}
"###;

const BUILTIN_FRONTMATTER_DOCTOR: &str = r###"# 知识库 frontmatter 体检:遍历 .md,检查缺失 title/date/tags、日期格式是否规范。
# knowledge-base 契约:ctx["kb_location"] 是知识库目录,返回 {"report": markdown}。
import os
import re

_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}")


def _front(text):
    if not text.startswith("---"):
        return None
    end = text.find("\n---", 3)
    return None if end == -1 else text[3:end]


def run(ctx):
    loc = ctx.get("kb_location", "") if isinstance(ctx, dict) else ""
    if not loc or not os.path.isdir(loc):
        return {"report": "# frontmatter 体检\n\n未找到知识库目录。"}
    total = 0
    buckets = {"缺少 frontmatter": [], "缺少 title": [], "缺少 date": [], "缺少 tags": [], "date 非 YYYY-MM-DD": []}
    for root, _d, files in os.walk(loc):
        for f in files:
            if not f.lower().endswith(".md"):
                continue
            total += 1
            rel = os.path.relpath(os.path.join(root, f), loc).replace("\\", "/")
            try:
                txt = open(os.path.join(root, f), "r", encoding="utf-8", errors="ignore").read()
            except Exception:
                continue
            fm = _front(txt)
            if fm is None:
                buckets["缺少 frontmatter"].append(rel)
                continue
            keys = {}
            for line in fm.split("\n"):
                m = re.match(r"^([A-Za-z0-9_\-]+)\s*:\s*(.*)$", line)
                if m:
                    keys[m.group(1).lower()] = m.group(2).strip()
            for k in ("title", "date", "tags"):
                if not keys.get(k):
                    buckets["缺少 %s" % k].append(rel)
            d = keys.get("date", "").strip("\"'")
            if d and not _DATE.match(d):
                buckets["date 非 YYYY-MM-DD"].append(rel)
    lines = ["# frontmatter 体检", "", "Markdown 笔记总数: %d" % total, ""]
    for title, items in buckets.items():
        lines.append("## %s (%d)" % (title, len(items)))
        lines.extend(["- %s" % it for it in items[:50]] or ["- (无)"])
        if len(items) > 50:
            lines.append("- …其余 %d 条" % (len(items) - 50))
        lines.append("")
    return {"report": "\n".join(lines)}
"###;

// ===== 实用插件(2026-11):模型处理、剪贴板表格、网页正文、知识库沉淀、数据生成、导入、表单 =====

const BUILTIN_LLM_TOOL: &str = r###"# 模型处理:对选中文本做润色/精简/扩写/翻译/摘要/纠错。
# 通过本机 8888 的 /api/plugin/llm 调用用户在设置里配置的模型(密钥不出后端)。
# editor transform 契约:返回 {"text": 新文本}。
import json
import urllib.request

_ENDPOINT = "http://127.0.0.1:8888/api/plugin/llm"


def _call(system, prompt):
    body = json.dumps({"system": system, "prompt": prompt}, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(_ENDPOINT, data=body, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except Exception as e:
        return None, "调用本地模型失败:%s" % e
    if not data.get("success"):
        return None, str(data.get("message") or "模型调用失败")
    text = str(data.get("text") or "").strip()
    if not text:
        return None, "模型返回为空"
    return text, ""


def run(ctx):
    text = str(ctx.get("selection") or "").strip()
    if not text:
        return {"error": "请先选中要处理的文本"}
    args = ctx.get("args") or {}
    mode = str(args.get("mode") or "润色")
    lang = str(args.get("target_lang") or "英文").strip() or "英文"
    extra = str(args.get("instruction") or "").strip()

    system = "你是严谨的中文写作与编辑助手。只输出处理后的正文,不要解释、不要加前缀。"
    if mode == "翻译":
        prompt = "请把下面的文本翻译成%s,保持原有 Markdown 结构和专有名词,只输出译文:\n\n%s" % (lang, text)
    elif mode == "摘要":
        prompt = "请用中文为下面文本写一段不超过 150 字的摘要,只输出摘要:\n\n%s" % text
    elif mode == "精简":
        prompt = "请在不丢失关键信息的前提下精简下面文本,只输出精简后的正文:\n\n%s" % text
    elif mode == "扩写":
        prompt = "请在保持原意的前提下扩写下面文本,补充必要的过渡与细节,只输出扩写后的正文:\n\n%s" % text
    elif mode == "纠错":
        prompt = "请修正下面文本的错别字、标点和语病,尽量不改变原句,只输出修正后的正文:\n\n%s" % text
    else:
        prompt = "请润色下面文本,使其通顺、准确、书面化,不改变事实与结构,只输出润色后的正文:\n\n%s" % text
    if extra:
        prompt += "\n\n额外要求:" + extra
    out, err = _call(system, prompt)
    if err:
        return {"error": err}
    return {"text": out}
"###;

const BUILTIN_CLIP_TABLE: &str = r###"# 剪贴板表格 → Markdown:从 Excel / 网页复制区域,在光标处生成 Markdown 表格。
# 支持 Tab 分隔(Excel 默认)与 CSV;纯数值列可自动右对齐。
import csv
import io


def _clean(v):
    s = str(v if v is not None else "").replace("\r\n", "\n").replace("\r", "\n").strip()
    return s.replace("|", "\\|").replace("\n", "<br>")


def _is_num(v):
    t = str(v).replace(",", "").replace("%", "").replace("￥", "").replace("$", "").strip()
    if not t:
        return False
    try:
        float(t)
        return True
    except Exception:
        return False


def _parse(text):
    text = text.replace("\ufeff", "").replace("\r\n", "\n").replace("\r", "\n")
    lines = [l for l in text.split("\n") if l.strip() != ""]
    if not lines:
        return []
    if "\t" in text:
        return [l.split("\t") for l in lines]
    try:
        dialect = csv.Sniffer().sniff(text[:4096], delimiters=",;\t")
        return list(csv.reader(io.StringIO(text), dialect))
    except Exception:
        return [[l] for l in lines]


def _flag(v, default=True):
    if isinstance(v, bool):
        return v
    if v is None:
        return default
    return str(v).strip().lower() not in ("false", "0", "no", "")


def run(ctx):
    raw = ctx.get("clipboard") or ""
    if not str(raw).strip():
        return {"error": "剪贴板为空:请先在 Excel / 网页表格里复制一块区域"}
    args = ctx.get("args") or {}
    has_header = _flag(args.get("header"), True)
    numeric_align = _flag(args.get("numeric_align"), True)
    first_label = str(args.get("first_col_label") or "列").strip() or "列"

    rows = [[_clean(c) for c in r] for r in _parse(str(raw))]
    if not rows:
        return {"error": "没有解析到表格行"}
    width = max(len(r) for r in rows)
    rows = [(r + [""] * width)[:width] for r in rows]
    if has_header:
        header = rows[0]
        body = rows[1:]
    else:
        header = ["%s%d" % (first_label, i + 1) for i in range(width)]
        body = rows

    num_cols = []
    for c in range(width):
        vals = [r[c] for r in body if str(r[c]).strip() != ""]
        num_cols.append(bool(vals) and all(_is_num(v) for v in vals))

    def line(cells):
        return "| " + " | ".join(cells) + " |"

    sep = ["---:" if numeric_align and num_cols[c] else "---" for c in range(width)]
    out = [line(header), "| " + " | ".join(sep) + " |"]
    out.extend(line(r) for r in body)
    return {"text": "\n".join(out) + "\n"}
"###;

const BUILTIN_URL_TO_MD: &str = r###"# 链接转 Markdown:抓取选中的网址,用 trafilatura 抽正文并转成 Markdown。
import re
import urllib.request

_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"


def _bool(v, default=False):
    if isinstance(v, bool):
        return v
    if v is None:
        return default
    return str(v).lower() not in ("false", "0", "no", "")


def _fetch(url):
    try:
        import httpx
        with httpx.Client(follow_redirects=True, timeout=30.0, headers={"User-Agent": _UA}) as client:
            resp = client.get(url)
            resp.raise_for_status()
            return resp.text, str(resp.url)
    except Exception:
        req = urllib.request.Request(url, headers={"User-Agent": _UA})
        with urllib.request.urlopen(req, timeout=30) as resp:
            charset = resp.headers.get_content_charset() or "utf-8"
            return resp.read().decode(charset, "ignore"), url


def _strip_html(html):
    html = re.sub(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>", " ", html)
    text = re.sub(r"(?s)<[^>]+>", "\n", html)
    text = re.sub(r"[ \t]+", " ", text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip()


def run(ctx):
    raw = str(ctx.get("selection") or "").strip()
    m = re.match(r"^<?(https?://[^\s<>]+?)>?$", raw)
    if not m:
        return {"error": "请先选中一个 http/https 链接"}
    url = m.group(1)
    args = ctx.get("args") or {}
    keep_links = _bool(args.get("keep_links"), True)
    keep_images = _bool(args.get("keep_images"), False)
    try:
        max_chars = int(float(args.get("max_chars") or 20000))
    except Exception:
        max_chars = 20000
    try:
        html, final_url = _fetch(url)
    except Exception as e:
        return {"error": "抓取失败:%s" % e}

    md = ""
    title = ""
    try:
        import trafilatura
        md = trafilatura.extract(
            html,
            url=final_url,
            output_format="markdown",
            include_links=keep_links,
            include_images=keep_images,
            include_comments=False,
            include_tables=True,
        ) or ""
        meta = trafilatura.extract_metadata(html, default_url=final_url)
        if meta is not None:
            title = (meta.title or "").strip()
    except Exception:
        md = ""
    if not md.strip():
        md = _strip_html(html)
    md = md.strip()
    if max_chars > 0 and len(md) > max_chars:
        md = md[:max_chars].rstrip() + "\n\n> 已截断,原文共 %d 字" % len(md)
    head = ("# %s\n\n" % title) if title else ""
    return {"text": head + "> 来源:<%s>\n\n" % final_url + md + "\n"}
"###;

const BUILTIN_SELECTION_TO_KB: &str = r###"# 选中内容 → 知识库笔记:写入当前知识库,并按标签/标题重叠补 [[双链]]。
import datetime
import os
import re


def _today():
    return datetime.date.today().isoformat()


def _slug(title):
    s = re.sub(r'[\\/:*?"<>|]+', "", title).strip().strip(".")
    s = re.sub(r"\s+", "-", s)
    return s[:80] or "note"


def _front(text):
    if not text.startswith("---"):
        return {}
    end = text.find("\n---", 3)
    if end == -1:
        return {}
    out = {}
    for line in text[3:end].split("\n"):
        m = re.match(r"^([A-Za-z0-9_\-]+)\s*:\s*(.*)$", line)
        if m:
            out[m.group(1).lower()] = m.group(2).strip().strip("\"'")
    return out


def _tags(raw):
    raw = str(raw or "").strip()
    if raw.startswith("[") and raw.endswith("]"):
        raw = raw[1:-1]
    return [t for t in (x.strip() for x in re.split(r"[,，;；\s]+", raw)) if t]


def _related(loc, title, tags, limit=5):
    lower = title.lower()
    wanted = {t.lower() for t in tags}
    scored = []
    for root, _dirs, files in os.walk(loc):
        for f in files:
            if not f.lower().endswith(".md"):
                continue
            path = os.path.join(root, f)
            try:
                with open(path, "r", encoding="utf-8", errors="ignore") as fh:
                    head = fh.read(2000)
            except Exception:
                continue
            fm = _front(head)
            other = (fm.get("title") or f[:-3]).strip()
            if not other or other == title:
                continue
            score = len({t.lower() for t in _tags(fm.get("tags", ""))} & wanted)
            if lower and (lower in other.lower() or other.lower() in lower):
                score += 2
            if score > 0:
                scored.append((score, f[:-3], other))
    scored.sort(key=lambda x: (-x[0], x[1]))
    return [(stem, name) for _s, stem, name in scored[:limit]]


def run(ctx):
    content = str(ctx.get("selection") or "").strip()
    if not content:
        return {"error": "请先选中要存入知识库的内容"}
    loc = str(ctx.get("kb_location") or "").strip()
    if not loc or not os.path.isdir(loc):
        return {"error": "未找到当前知识库目录:请先在左侧选中一个知识库"}
    args = ctx.get("args") or {}
    folder = str(args.get("folder") or "收集").strip() or "收集"
    tags = _tags(args.get("tags"))

    title = str(args.get("title") or "").strip()
    if not title:
        for line in content.splitlines():
            s = line.strip()
            if s.startswith("#"):
                title = s.lstrip("#").strip()
                if title:
                    break
    if not title:
        first = next((l.strip() for l in content.splitlines() if l.strip()), "")
        title = first[:40] if first else "未命名笔记"
    title = title.replace("\n", " ").strip()

    target_dir = os.path.join(loc, folder)
    os.makedirs(target_dir, exist_ok=True)
    base = _slug(title)
    path = os.path.join(target_dir, base + ".md")
    n = 2
    while os.path.exists(path):
        path = os.path.join(target_dir, "%s-%d.md" % (base, n))
        n += 1

    source = str(ctx.get("file_path") or "").replace("\\", "/")
    title_yaml = title.replace("\\", "\\\\").replace('"', '\\"')
    source_yaml = source.replace("\\", "\\\\").replace('"', '\\"')
    tag_yaml = "[" + ", ".join('"%s"' % t.replace('"', '\\"') for t in tags) + "]"
    fm = [
        "---",
        'title: "%s"' % title_yaml,
        "date: %s" % _today(),
        "tags: %s" % tag_yaml,
        'source: "%s"' % source_yaml,
        "---",
        "",
    ]
    related = _related(loc, title, tags)
    body = [content, ""]
    if related:
        body.append("## 相关")
        for stem, name in related:
            body.append("- [[%s|%s]]" % (stem, name) if name != stem else "- [[%s]]" % stem)
        body.append("")
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(fm) + "\n".join(body))
    stem = os.path.splitext(os.path.basename(path))[0]
    return {"text": content + "\n\n> 已存入知识库:%s[[%s]]" % (folder + "/", stem)}
"###;

const BUILTIN_GEN_DATA: &str = r###"# 数据生成:按列规格生成示例数据(Markdown / CSV / JSON),在光标处插入。
import datetime
import json
import random

_SURNAMES = "赵钱孙李周吴郑王冯陈褚卫蒋沈韩杨朱秦尤许何吕施张孔曹严华金魏陶姜"
_GIVEN = ["子涵", "雨桐", "浩然", "思远", "佳怡", "宇轩", "欣怡", "明哲", "诗涵", "俊杰", "嘉宁", "若彤"]
_DOMAINS = ["example.com", "mail.com", "test.cn"]
_STATUS = ["待处理", "进行中", "已完成"]


def _value(kind, rng, i):
    k = (kind or "").strip().lower()
    if k in ("name", "姓名", "人名"):
        return rng.choice(_SURNAMES) + rng.choice(_GIVEN)
    if k in ("email", "邮箱"):
        return "user%03d@%s" % (i + 1, rng.choice(_DOMAINS))
    if k in ("phone", "手机", "电话"):
        return "1%d%08d" % (rng.choice([3, 5, 7, 8, 9]), rng.randrange(10 ** 8))
    if k in ("date", "日期"):
        return (datetime.date(2025, 1, 1) + datetime.timedelta(days=rng.randrange(365))).isoformat()
    if k in ("amount", "金额", "price", "价格"):
        return "%.2f" % (rng.random() * 10000)
    if k in ("int", "整数", "number", "数字"):
        return str(rng.randrange(1, 1000))
    if k in ("status", "状态"):
        return rng.choice(_STATUS)
    if k in ("bool", "布尔"):
        return rng.choice(["是", "否"])
    return "%s-%d" % (kind or "value", i + 1)


def _columns(spec):
    cols = []
    for part in str(spec or "").split(","):
        part = part.strip()
        if not part:
            continue
        if ":" in part:
            label, kind = part.split(":", 1)
        elif "：" in part:
            label, kind = part.split("：", 1)
        else:
            label, kind = part, part
        cols.append((label.strip(), kind.strip()))
    return cols or [("姓名", "name"), ("邮箱", "email"), ("金额", "amount")]


def _cell(v):
    return str(v).replace("|", "\\|").replace("\n", "<br>")


def _md_table(header, rows):
    out = ["| " + " | ".join(_cell(h) for h in header) + " |",
           "| " + " | ".join("---" for _ in header) + " |"]
    out.extend("| " + " | ".join(_cell(c) for c in r) + " |" for r in rows)
    return "\n".join(out)


def run(ctx):
    args = ctx.get("args") or {}
    try:
        rows_n = int(float(args.get("rows") or 20))
    except Exception:
        rows_n = 20
    rows_n = max(1, min(rows_n, 1000))
    try:
        seed = int(float(args.get("seed") or 42))
    except Exception:
        seed = 42
    fmt = str(args.get("format") or "Markdown").strip()
    cols = _columns(args.get("columns"))
    rng = random.Random(seed)
    rows = [[_value(kind, rng, i) for _label, kind in cols] for i in range(rows_n)]
    header = [label for label, _kind in cols]
    if fmt.lower() == "csv":
        lines = [",".join(header)]
        lines.extend(",".join('"%s"' % str(c).replace('"', '""') for c in r) for r in rows)
        return {"text": "\n".join(lines) + "\n"}
    if fmt.lower() == "json":
        return {"text": json.dumps([dict(zip(header, r)) for r in rows], ensure_ascii=False, indent=2) + "\n"}
    return {"text": _md_table(header, rows) + "\n"}
"###;

const BUILTIN_IMPORT_TABLE: &str = r###"# 导入表格:读取 CSV / TSV / XLSX / JSON,在光标处生成 Markdown 表格。
# 文件路径可写在插件参数里;留空时尝试使用剪贴板中的路径。
import csv
import io
import json
import os


def _cell(v):
    return str(v).replace("|", "\\|").replace("\n", "<br>")


def _md(header, rows):
    out = ["| " + " | ".join(_cell(h) for h in header) + " |",
           "| " + " | ".join("---" for _ in header) + " |"]
    out.extend("| " + " | ".join(_cell(c) for c in r) + " |" for r in rows)
    return "\n".join(out)


def _rows_from_json(data):
    if isinstance(data, list):
        if data and all(isinstance(x, dict) for x in data):
            header = []
            for item in data:
                for k in item.keys():
                    if k not in header:
                        header.append(k)
            return header, [[item.get(k, "") for k in header] for item in data]
        if data and all(isinstance(x, (list, tuple)) for x in data):
            return list(data[0]), [list(x) for x in data[1:]]
    if isinstance(data, dict):
        if data and all(isinstance(v, list) for v in data.values()):
            header = list(data.keys())
            n = max(len(v) for v in data.values())
            return header, [[data[k][i] if i < len(data[k]) else "" for k in header] for i in range(n)]
        return list(data.keys()), [list(data.values())]
    return ["值"], [[data]]


def run(ctx):
    args = ctx.get("args") or {}
    path = str(args.get("path") or "").strip().strip('"')
    if not path:
        clip = str(ctx.get("clipboard") or "").strip().strip('"')
        if clip and os.path.isfile(clip):
            path = clip
    if not path:
        return {"error": "请先在插件参数里填写文件路径,或把文件路径复制到剪贴板"}
    if not os.path.isfile(path):
        return {"error": "文件不存在:%s" % path}
    try:
        max_rows = max(1, min(int(float(args.get("max_rows") or 200)), 5000))
    except Exception:
        max_rows = 200
    ext = os.path.splitext(path)[1].lower()
    try:
        if ext in (".xlsx", ".xlsm", ".xls"):
            import pandas as pd
            sheet = str(args.get("sheet") or "").strip()
            df = pd.read_excel(path, sheet_name=(sheet or 0))
            header = [str(c) for c in df.columns]
            rows = df.astype(str).values.tolist()
        elif ext == ".json":
            with open(path, "r", encoding="utf-8-sig") as fh:
                header, rows = _rows_from_json(json.load(fh))
        elif ext == ".tsv":
            with open(path, "r", encoding="utf-8-sig", errors="ignore") as fh:
                rows = [line.split("\t") for line in fh.read().splitlines() if line.strip()]
            header, rows = rows[0], rows[1:]
        else:
            with open(path, "r", encoding="utf-8-sig", errors="ignore") as fh:
                raw = fh.read()
            try:
                dialect = csv.Sniffer().sniff(raw[:4096], delimiters=",;\t")
                rows = list(csv.reader(io.StringIO(raw), dialect))
            except Exception:
                rows = [line.split(",") for line in raw.splitlines() if line.strip()]
            header, rows = rows[0], rows[1:]
    except Exception as e:
        return {"error": "读取失败:%s" % e}
    if not header:
        return {"error": "文件里没有表头"}
    truncated = len(rows) > max_rows
    rows = rows[:max_rows]
    width = len(header)
    rows = [(list(r) + [""] * width)[:width] for r in rows]
    note = "\n\n> 已截断,只显示前 %d 行" % max_rows if truncated else ""
    return {"text": _md(header, rows) + "\n" + note}
"###;

const BUILTIN_FORM_FILL: &str = r###"# 表单填充助手:从当前笔记/选中内容(或任务查询)提取字段,生成预填好的表单链接。
# 默认只输出链接与字段预览,不会自动提交;auto_open 打开时用默认浏览器打开 GET 链接。
import json
import re
import urllib.parse
import urllib.request
import webbrowser

_MCP = "http://127.0.0.1:8888/mcp"


def _bool(v, default=False):
    if isinstance(v, bool):
        return v
    if v is None:
        return default
    return str(v).lower() not in ("false", "0", "no", "")


def _mcp_call(name, arguments):
    body = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    }, ensure_ascii=False).encode("utf-8")
    req = urllib.request.Request(_MCP, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as resp:
        data = json.loads(resp.read().decode("utf-8"))
    if data.get("error"):
        raise RuntimeError(str(data["error"]))
    content = ((data.get("result") or {}).get("content") or [])
    text = content[0].get("text") if content else ""
    if not text:
        return None
    try:
        return json.loads(text)
    except Exception:
        return text


def _fields_from_text(text):
    fields = {}
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        for line in lines[1:]:
            if line.strip() == "---":
                break
            m = re.match(r"^([A-Za-z0-9_\-\u4e00-\u9fff]+)\s*:\s*(.*)$", line)
            if m:
                fields[m.group(1).strip()] = m.group(2).strip().strip("\"'")
    for line in lines:
        if not line.strip().startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) == 2 and cells[0] and not re.fullmatch(r":?-{2,}:?", cells[0]) and not re.fullmatch(r":?-{2,}:?", cells[1]):
            if cells[0] in ("字段", "名称", "键", "Key", "key", "Field", "field"):
                continue
            fields.setdefault(cells[0], cells[1])
    for line in lines:
        if line.strip().startswith("|"):
            continue
        m = re.match(r"^\s*[-*]?\s*([^:：=]{1,40})[:：=]\s*(.+?)\s*$", line)
        if m and m.group(1).strip() != "---":
            fields.setdefault(m.group(1).strip(), m.group(2).strip().strip("\"'"))
    return fields


def _task_fields(keyword):
    args = {"keyword": keyword} if keyword else {}
    data = _mcp_call("guaxi_task_query", args)
    if isinstance(data, str):
        return {"任务查询结果": data}
    items = data if isinstance(data, list) else (data or {}).get("tasks") or (data or {}).get("items") or []
    if not items:
        return {}
    first = items[0] if isinstance(items[0], dict) else {"内容": str(items[0])}
    fields = {str(k): str(v) for k, v in first.items() if isinstance(v, (str, int, float, bool))}
    fields["任务总数"] = str(len(items))
    return fields


def run(ctx):
    args = ctx.get("args") or {}
    template = str(args.get("url_template") or "").strip()
    if not template:
        return {"error": "请先在插件参数里填写表单链接模板,例如 https://example.com/new?title={{title}}"}
    content = str(ctx.get("selection") or "").strip() or str(ctx.get("document") or "").strip()
    if not content:
        return {"error": "当前没有可用的笔记内容"}
    fields = _fields_from_text(content)
    if "任务" in str(args.get("source") or ""):
        try:
            fields.update(_task_fields(str(args.get("task_keyword") or "").strip()))
        except Exception as e:
            return {"error": "查询任务失败:%s(请确认本地服务已启动)" % e}
    if not fields:
        return {"error": "没有解析到字段,请用 key: value 或两列表格整理内容"}
    fields.setdefault("content", content)
    fields.setdefault("selection", content)

    def repl(m):
        return urllib.parse.quote(str(fields.get(m.group(1).strip(), "")), safe="")

    url = re.sub(r"\{\{\s*([^{}]+?)\s*\}\}", repl, template)
    preview = ["已生成表单链接(发送前请自行确认):", "", "[打开表单](<%s>)" % url, "", url, "", "| 字段 | 值 |", "| --- | --- |"]
    for k, v in fields.items():
        if k in ("content", "selection"):
            continue
        preview.append("| %s | %s |" % (
            str(k).replace("|", "\\|"),
            str(v).replace("|", "\\|").replace("\n", "<br>"),
        ))
    if _bool(args.get("auto_open"), False):
        try:
            webbrowser.open(url)
            preview.insert(2, "已在默认浏览器打开。\n")
        except Exception:
            pass
    return {"text": "\n".join(preview) + "\n"}
"###;

struct BuiltinPlugin {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    surface: &'static str,
    capability: &'static str,
    scope: &'static str,
    main: &'static str,
    /// manifest.args 的 JSON 文本;空串表示无参数。
    args: &'static str,
    clipboard: bool,
    allow_empty_selection: bool,
    needs_kb: bool,
}

const BUILTIN_PLUGINS: &[BuiltinPlugin] = &[
    BuiltinPlugin {
        id: "md-table-total",
        name: "表格合计与对齐",
        description: "选中 Markdown 表格:规范对齐,并为纯数值列自动追加「合计」行",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_MD_TABLE_TOTAL,
        args: "",
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "code-tidy",
        name: "代码块整理",
        description: "选中含 ``` 围栏的文本:去行尾空白、Tab 转空格,JSON 块自动美化缩进",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_CODE_TIDY,
        args: "",
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "cite-tidy",
        name: "参考文献整理",
        description: "选中参考条目:去掉原有符号/编号、折叠多余空白,统一重新编号",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_CITE_TIDY,
        args: "",
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "data-summary",
        name: "数据摘要",
        description: "选中一组数字:在下方追加计数/求和/均值/中位数/极值/标准差摘要表",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_DATA_SUMMARY,
        args: "",
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "frontmatter-doctor",
        name: "frontmatter 体检",
        description: "遍历知识库 .md:报告缺失 title/date/tags 与非法日期格式的笔记",
        surface: "knowledge-base",
        capability: "generator",
        scope: "kb",
        main: BUILTIN_FRONTMATTER_DOCTOR,
        args: "",
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "llm-tool",
        name: "模型润色 / 翻译",
        description: "选中文本:按设置的模型做润色、精简、扩写、翻译、摘要或纠错",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_LLM_TOOL,
        args: r#"[{"key":"mode","label":"处理方式","type":"select","options":["润色","精简","扩写","翻译","摘要","纠错"],"default":"润色"},{"key":"target_lang","label":"翻译目标语言","type":"text","default":"英文"},{"key":"instruction","label":"额外要求","type":"text","default":"","placeholder":"可选,例如:保留术语"}]"#,
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "clip-table",
        name: "剪贴板表格 → Markdown",
        description: "在 Excel / 网页表格复制区域:在光标处生成 Markdown 表格(数值列右对齐)",
        surface: "editor",
        capability: "transform",
        scope: "insert",
        main: BUILTIN_CLIP_TABLE,
        args: r#"[{"key":"header","label":"首行作为表头","type":"bool","default":true},{"key":"numeric_align","label":"数值列右对齐","type":"bool","default":true},{"key":"first_col_label","label":"无表头时列名前缀","type":"text","default":"列"}]"#,
        clipboard: true,
        allow_empty_selection: true,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "url-to-md",
        name: "链接抓正文",
        description: "选中网址:抓取网页正文并转成 Markdown(替换选区)",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_URL_TO_MD,
        args: r#"[{"key":"max_chars","label":"最大字符数(0=不限)","type":"number","default":20000},{"key":"keep_links","label":"保留链接","type":"bool","default":true},{"key":"keep_images","label":"保留图片","type":"bool","default":false}]"#,
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "selection-to-kb",
        name: "存入知识库并双链",
        description: "选中内容:写入当前知识库笔记,并按标签/标题重叠补 [[相关链接]]",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_SELECTION_TO_KB,
        args: r#"[{"key":"folder","label":"子目录","type":"text","default":"收集"},{"key":"tags","label":"标签(逗号分隔)","type":"text","default":""},{"key":"title","label":"笔记标题(留空取首个标题)","type":"text","default":""}]"#,
        clipboard: false,
        allow_empty_selection: false,
        needs_kb: true,
    },
    BuiltinPlugin {
        id: "gen-data",
        name: "生成示例数据",
        description: "按列规格生成 Markdown / CSV / JSON 示例数据,插入光标处",
        surface: "editor",
        capability: "transform",
        scope: "insert",
        main: BUILTIN_GEN_DATA,
        args: r#"[{"key":"rows","label":"行数","type":"number","default":20},{"key":"columns","label":"列规格","type":"text","default":"姓名:name,邮箱:email,金额:amount","placeholder":"列名:类型, 类型可选 name/email/phone/date/amount/int/status/bool"},{"key":"format","label":"输出格式","type":"select","options":["Markdown","CSV","JSON"],"default":"Markdown"},{"key":"seed","label":"随机种子","type":"number","default":42}]"#,
        clipboard: false,
        allow_empty_selection: true,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "import-table",
        name: "导入表格文件",
        description: "读取 CSV / TSV / XLSX / JSON,在光标处生成 Markdown 表格",
        surface: "editor",
        capability: "transform",
        scope: "insert",
        main: BUILTIN_IMPORT_TABLE,
        args: r#"[{"key":"path","label":"文件路径","type":"file","accept":["csv","tsv","xlsx","xlsm","xls","json"],"default":"","placeholder":"选择 CSV/TSV/XLSX/JSON 文件;留空则读剪贴板路径"},{"key":"sheet","label":"工作表(Excel)","type":"text","default":""},{"key":"max_rows","label":"最大行数","type":"number","default":200}]"#,
        clipboard: true,
        allow_empty_selection: true,
        needs_kb: false,
    },
    BuiltinPlugin {
        id: "form-fill",
        name: "表单填充助手",
        description: "从笔记/任务提取字段,生成预填好的表单链接(默认只预览不提交)",
        surface: "editor",
        capability: "transform",
        scope: "insert",
        main: BUILTIN_FORM_FILL,
        args: r#"[{"key":"url_template","label":"表单链接模板","type":"text","default":"","placeholder":"https://example.com/new?title={{title}}&body={{content}}","required":true},{"key":"source","label":"字段来源","type":"select","options":["选中文本/当前笔记","任务查询"],"default":"选中文本/当前笔记"},{"key":"task_keyword","label":"任务关键词(仅任务查询)","type":"text","default":""},{"key":"auto_open","label":"生成后在浏览器打开","type":"bool","default":false}]"#,
        clipboard: false,
        allow_empty_selection: true,
        needs_kb: false,
    },
];

/// 内置插件清单版本。递增后会刷新已存在内置插件的「声明式字段」，
/// 但不会覆盖 main.py，也不会改动用户自己改过的 name/description/entry。
const BUILTIN_REV: u64 = 2;

fn builtin_args(b: &BuiltinPlugin) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(b.args)
        .unwrap_or_else(|_| serde_json::json!([]))
}

/// 把内置插件写入插件目录。用 `_builtins.json` 记录"已供给过"的 id,保证每个内置插件
/// 一生只写入一次:用户此后可自由编辑或删除,不会被重新生成;将来新增的内置插件会补写。
fn provision_builtin_plugins(dir: &Path) {
    let marker = dir.join("_builtins.json");
    let state: serde_json::Value = fs::read_to_string(&marker)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(serde_json::Value::Null);
    let mut done: Vec<String> = str_array(&state, "provisioned");
    let rev = state.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
    let mut changed = rev < BUILTIN_REV;

    for b in BUILTIN_PLUGINS {
        let pdir = dir.join(b.id);
        let known = done.iter().any(|d| d == b.id);

        if !known {
            if !pdir.exists() && fs::create_dir_all(&pdir).is_ok() {
                let manifest = serde_json::json!({
                    "id": b.id,
                    "name": b.name,
                    "description": b.description,
                    "entry": "main.py",
                    "capabilities": [b.capability],
                    "surfaces": [b.surface],
                    "scope": b.scope,
                    "args": builtin_args(b),
                    "clipboard": b.clipboard,
                    "allow_empty_selection": b.allow_empty_selection,
                    "needs_kb": b.needs_kb,
                });
                let _ = fs::write(
                    pdir.join("manifest.json"),
                    serde_json::to_string_pretty(&manifest).unwrap_or_default(),
                );
                let _ = fs::write(pdir.join("main.py"), b.main);
            }
            done.push(b.id.to_string());
            changed = true;
            continue;
        }

        // 已供给过:版本升级时只刷新参数/作用面等声明式字段,保留用户内容。
        if rev < BUILTIN_REV && pdir.is_dir() {
            let mpath = pdir.join("manifest.json");
            let manifest_text = match fs::read_to_string(&mpath) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let mut manifest: serde_json::Value = match serde_json::from_str(&manifest_text) {
                Ok(value) => value,
                // manifest 本身损坏时不要用半份数据覆盖它,留给用户修。
                Err(_) => continue,
            };
            if let Some(obj) = manifest.as_object_mut() {
                obj.insert("args".to_string(), builtin_args(b));
                obj.insert("capabilities".to_string(), serde_json::json!([b.capability]));
                obj.insert("surfaces".to_string(), serde_json::json!([b.surface]));
                obj.insert("scope".to_string(), serde_json::json!(b.scope));
                obj.insert("clipboard".to_string(), serde_json::json!(b.clipboard));
                obj.insert(
                    "allow_empty_selection".to_string(),
                    serde_json::json!(b.allow_empty_selection),
                );
                obj.insert("needs_kb".to_string(), serde_json::json!(b.needs_kb));
            }
            if fs::write(
                &mpath,
                serde_json::to_string_pretty(&manifest).unwrap_or_default(),
            )
            .is_ok()
            {
                changed = true;
            }
        }
    }

    if changed {
        let _ = fs::write(
            &marker,
            serde_json::to_string_pretty(&serde_json::json!({
                "provisioned": done,
                "rev": BUILTIN_REV,
            }))
            .unwrap_or_else(|_| "{}".to_string()),
        );
    }
}

fn str_array(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('-');
        } else if !ch.is_ascii() {
            // 非 ASCII(如中文名)转成 unicode 码点片段,保证目录名可用且唯一性尚可。
            out.push_str(&format!("u{:x}", ch as u32));
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        format!("plugin-{}", std::process::id())
    } else {
        trimmed
    }
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join("_state.json")
}

#[derive(Default)]
struct PluginState {
    disabled: Vec<String>,
    /// 每个插件最近一次保存的参数值: {"<plugin-id>": {"<key>": value}}
    args: serde_json::Map<String, serde_json::Value>,
}

fn read_state(dir: &Path) -> PluginState {
    let raw = fs::read_to_string(state_path(dir)).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    PluginState {
        disabled: str_array(&v, "disabled"),
        args: v
            .get("args")
            .and_then(|a| a.as_object())
            .cloned()
            .unwrap_or_default(),
    }
}

fn write_state(dir: &Path, state: &PluginState) -> Result<(), String> {
    let body = serde_json::json!({
        "disabled": state.disabled,
        "args": state.args,
    });
    fs::write(
        state_path(dir),
        serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("write plugin state failed: {e}"))
}

/// 解析 manifest.args。单个参数缺少 key 时跳过，避免一个手滑参数拖垮整个插件。
fn parse_arg_specs(manifest: &serde_json::Value) -> Vec<PluginArgSpec> {
    manifest
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let key = a.get("key").and_then(|v| v.as_str())?.trim();
                    if key.is_empty() {
                        return None;
                    }
                    Some(PluginArgSpec {
                        key: key.to_string(),
                        label: a
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or(key)
                            .to_string(),
                        kind: a
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("text")
                            .to_string(),
                        default: a.get("default").cloned().unwrap_or(serde_json::Value::Null),
                        options: str_array(a, "options"),
                        placeholder: a
                            .get("placeholder")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        required: a.get("required").and_then(|v| v.as_bool()).unwrap_or(false),
                        accept: str_array(a, "accept"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 默认值打底、用户保存值覆盖；只接受 manifest 声明过的 key。
fn merge_arg_values(
    specs: &[PluginArgSpec],
    stored: Option<&serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    for s in specs {
        out.insert(s.key.clone(), s.default.clone());
    }
    if let Some(obj) = stored.and_then(|v| v.as_object()) {
        for (k, v) in obj {
            if out.contains_key(k) {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

fn seed_example_plugin_if_empty(dir: &Path) {
    let has_any = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.path().is_dir()
                    && e.file_name().to_str().map(|n| !n.starts_with('_')).unwrap_or(false)
            })
        })
        .unwrap_or(false);
    if has_any {
        return;
    }
    let example_dir = dir.join("pangu-spacing");
    if fs::create_dir_all(&example_dir).is_err() {
        return;
    }
    let _ = fs::write(example_dir.join("manifest.json"), EXAMPLE_PLUGIN_MANIFEST);
    let _ = fs::write(example_dir.join("main.py"), EXAMPLE_PLUGIN_MAIN);

    // 再各放一个 repo / knowledge-base 作用面的只读示例,展示三种作用面的写法。
    let repo_dir = dir.join("repo-health-report");
    if fs::create_dir_all(&repo_dir).is_ok() {
        let _ = fs::write(repo_dir.join("manifest.json"), REPO_EXAMPLE_MANIFEST);
        let _ = fs::write(repo_dir.join("main.py"), REPO_EXAMPLE_MAIN);
    }
    let kb_dir = dir.join("kb-stats");
    if fs::create_dir_all(&kb_dir).is_ok() {
        let _ = fs::write(kb_dir.join("manifest.json"), KB_EXAMPLE_MANIFEST);
        let _ = fs::write(kb_dir.join("main.py"), KB_EXAMPLE_MAIN);
    }
}

fn read_plugin_info(plugin_dir: &Path, state: &PluginState) -> Option<EditorPluginInfo> {
    if !plugin_dir.is_dir() {
        return None;
    }
    let raw = fs::read_to_string(plugin_dir.join("manifest.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let dir_name = plugin_dir.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    let id = manifest.get("id").and_then(|v| v.as_str()).unwrap_or(&dir_name).to_string();
    if id.is_empty() {
        return None;
    }
    let name = manifest.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
    let description = manifest.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let capabilities = str_array(&manifest, "capabilities");
    let surfaces = {
        let s = str_array(&manifest, "surfaces");
        if s.is_empty() { vec!["editor".to_string()] } else { s }
    };
    let scope = manifest.get("scope").and_then(|v| v.as_str()).unwrap_or("selection").to_string();
    let entry = manifest.get("entry").and_then(|v| v.as_str()).unwrap_or("main.py").to_string();
    let entry_path = plugin_dir.join(&entry);
    if !entry_path.is_file() {
        return None;
    }
    let args = parse_arg_specs(&manifest);
    let arg_values = merge_arg_values(&args, state.args.get(&id));
    Some(EditorPluginInfo {
        id: id.clone(),
        name,
        description,
        capabilities,
        surfaces,
        scope,
        dir: plugin_dir.to_string_lossy().to_string(),
        entry,
        entry_path: entry_path.to_string_lossy().to_string(),
        enabled: !state.disabled.iter().any(|d| d == &id),
        args,
        arg_values,
        clipboard: manifest.get("clipboard").and_then(|v| v.as_bool()).unwrap_or(false),
        allow_empty_selection: manifest
            .get("allow_empty_selection")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        needs_kb: manifest.get("needs_kb").and_then(|v| v.as_bool()).unwrap_or(false),
        prompt_on_run: manifest.get("prompt_on_run").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

fn collect_all_plugins(dir: &Path) -> Vec<EditorPluginInfo> {
    let state = read_state(dir);
    let mut plugins = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            // 跳过下划线开头的内部目录(_state.json、_exports 等)
            if p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with('_')).unwrap_or(false) {
                continue;
            }
            if let Some(info) = read_plugin_info(&p, &state) {
                plugins.push(info);
            }
        }
    }
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

fn find_plugin_dir(dir: &Path, id: &str) -> Option<PathBuf> {
    collect_all_plugins(dir)
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| PathBuf::from(p.dir))
}

// PLACEHOLDER_REST

#[command]
pub fn get_editor_plugins_dir() -> Result<String, String> {
    Ok(editor_plugins_dir()?.to_string_lossy().to_string())
}

/// 编辑器运行器用:只返回“已启用 + 含 editor 作用面”的插件。
#[command]
pub fn list_editor_plugins() -> Result<Vec<EditorPluginInfo>, String> {
    let dir = editor_plugins_dir()?;
    seed_example_plugin_if_empty(&dir);
    provision_builtin_plugins(&dir);
    Ok(collect_all_plugins(&dir)
        .into_iter()
        .filter(|p| p.enabled && p.surfaces.iter().any(|s| s == "editor"))
        .collect())
}

/// 插件管理器用:返回全部插件(所有作用面、含已禁用),带 enabled 状态。
#[command]
pub fn list_all_plugins() -> Result<Vec<EditorPluginInfo>, String> {
    let dir = editor_plugins_dir()?;
    seed_example_plugin_if_empty(&dir);
    provision_builtin_plugins(&dir);
    Ok(collect_all_plugins(&dir))
}

#[command]
pub fn set_plugin_enabled(id: String, enabled: bool) -> Result<(), String> {
    let dir = editor_plugins_dir()?;
    let id = id.trim().to_string();
    let mut state = read_state(&dir);
    if enabled {
        state.disabled.retain(|d| d != &id);
    } else if !state.disabled.iter().any(|d| d == &id) {
        state.disabled.push(id);
    }
    write_state(&dir, &state)
}

/// 保存插件参数值。只保留 manifest 声明过的 key,未声明的忽略。
#[command]
pub fn set_plugin_args(id: String, args: serde_json::Value) -> Result<(), String> {
    let dir = editor_plugins_dir()?;
    let id = id.trim().to_string();
    let plugin_dir = find_plugin_dir(&dir, &id).ok_or_else(|| format!("plugin not found: {id}"))?;
    let state = read_state(&dir);
    let info = read_plugin_info(&plugin_dir, &state).ok_or_else(|| format!("plugin invalid: {id}"))?;
    let incoming = args.as_object().cloned().unwrap_or_default();
    let mut values = serde_json::Map::new();
    for spec in &info.args {
        if let Some(v) = incoming.get(&spec.key) {
            values.insert(spec.key.clone(), v.clone());
        }
    }
    let mut state = state;
    state.args.insert(id, serde_json::Value::Object(values));
    write_state(&dir, &state)
}

#[command]
pub fn delete_plugin(id: String) -> Result<(), String> {
    let dir = editor_plugins_dir()?;
    let target = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    if !target.starts_with(&dir) {
        return Err("refuse to delete outside plugins dir".to_string());
    }
    fs::remove_dir_all(&target).map_err(|e| format!("delete plugin failed: {e}"))?;
    let mut state = read_state(&dir);
    state.disabled.retain(|d| d != id.trim());
    state.args.remove(id.trim());
    let _ = write_state(&dir, &state);
    Ok(())
}

#[command]
pub fn create_plugin(name: String, surface: Option<String>) -> Result<EditorPluginInfo, String> {
    let dir = editor_plugins_dir()?;
    let base = slugify(&name);
    let mut id = base.clone();
    let mut n = 1;
    while dir.join(&id).exists() {
        n += 1;
        id = format!("{base}-{n}");
    }
    let plugin_dir = dir.join(&id);
    fs::create_dir_all(&plugin_dir).map_err(|e| format!("create plugin dir failed: {e}"))?;
    let display_name = if name.trim().is_empty() { id.clone() } else { name.trim().to_string() };
    let surface = surface.unwrap_or_else(|| "editor".to_string());
    let manifest = serde_json::json!({
        "id": id,
        "name": display_name,
        "description": "",
        "entry": "main.py",
        "capabilities": ["transform"],
        "surfaces": [surface],
        "scope": "selection"
    });
    fs::write(
        plugin_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap_or_default(),
    )
    .map_err(|e| format!("write manifest failed: {e}"))?;
    fs::write(plugin_dir.join("main.py"), NEW_PLUGIN_MAIN_TEMPLATE)
        .map_err(|e| format!("write entry failed: {e}"))?;
    let state = read_state(&dir);
    read_plugin_info(&plugin_dir, &state).ok_or_else(|| "create plugin failed".to_string())
}

#[command]
pub fn read_plugin_entry(id: String) -> Result<PluginEntrySource, String> {
    let dir = editor_plugins_dir()?;
    let state = read_state(&dir);
    let plugin_dir = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    let info = read_plugin_info(&plugin_dir, &state).ok_or_else(|| format!("plugin invalid: {id}"))?;
    let content = fs::read_to_string(&info.entry_path).map_err(|e| format!("read entry failed: {e}"))?;
    Ok(PluginEntrySource { id: info.id, entry: info.entry, path: info.entry_path, content })
}

#[command]
pub fn write_plugin_entry(id: String, content: String) -> Result<(), String> {
    let dir = editor_plugins_dir()?;
    let state = read_state(&dir);
    let plugin_dir = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    let info = read_plugin_info(&plugin_dir, &state).ok_or_else(|| format!("plugin invalid: {id}"))?;
    let entry_path = PathBuf::from(&info.entry_path);
    if !entry_path.starts_with(&dir) {
        return Err("refuse to write outside plugins dir".to_string());
    }
    fs::write(&entry_path, content).map_err(|e| format!("write entry failed: {e}"))
}

// PLACEHOLDER_ZIP

fn common_top_dir(names: &[String]) -> Option<String> {
    let mut prefix: Option<String> = None;
    for n in names {
        if !n.contains('/') {
            return None; // 有条目直接在根,不是单一顶层目录
        }
        let top = format!("{}/", n.split('/').next().unwrap_or(""));
        match &prefix {
            None => prefix = Some(top),
            Some(p) if p != &top => return None,
            _ => {}
        }
    }
    prefix
}

/// 导出插件为 zip,写到 <plugins_dir>/_exports/<id>.zip,返回该路径(前端可再另存)。
#[command]
pub fn export_plugin_to_zip(id: String) -> Result<String, String> {
    let dir = editor_plugins_dir()?;
    let plugin_dir = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    let exports = dir.join("_exports");
    fs::create_dir_all(&exports).map_err(|e| format!("create exports dir failed: {e}"))?;
    let zip_path = exports.join(format!("{}.zip", id.trim()));
    let file = fs::File::create(&zip_path).map_err(|e| format!("create zip failed: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();

    let mut stack = vec![plugin_dir.clone()];
    while let Some(cur) = stack.pop() {
        for entry in fs::read_dir(&cur).map_err(|e| format!("read dir failed: {e}"))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let rel = path.strip_prefix(&plugin_dir).map_err(|e| e.to_string())?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.is_empty() {
                continue;
            }
            if path.is_dir() {
                let _ = zip.add_directory(format!("{rel_str}/"), options);
                stack.push(path);
            } else {
                zip.start_file(rel_str, options).map_err(|e| e.to_string())?;
                let bytes = fs::read(&path).map_err(|e| e.to_string())?;
                zip.write_all(&bytes).map_err(|e| e.to_string())?;
            }
        }
    }
    zip.finish().map_err(|e| format!("finish zip failed: {e}"))?;
    Ok(zip_path.to_string_lossy().to_string())
}

/// 从 zip 导入插件:解压到 <plugins_dir>/<id>/(自动剥掉单一顶层目录),校验 manifest.json 存在。
#[command]
pub fn import_plugin_from_zip(zip_path: String) -> Result<EditorPluginInfo, String> {
    let dir = editor_plugins_dir()?;
    let file = fs::File::open(&zip_path).map_err(|e| format!("open zip failed: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read zip failed: {e}"))?;

    let base = slugify(
        Path::new(&zip_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported-plugin"),
    );
    let mut target_name = base.clone();
    let mut n = 1;
    while dir.join(&target_name).exists() {
        n += 1;
        target_name = format!("{base}-{n}");
    }
    let target = dir.join(&target_name);
    fs::create_dir_all(&target).map_err(|e| format!("create target failed: {e}"))?;

    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().replace('\\', "/")))
        .collect();
    let top_prefix = common_top_dir(&names);

    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().replace('\\', "/");
        let rel = match &top_prefix {
            Some(p) => name.strip_prefix(p).unwrap_or(&name).to_string(),
            None => name.clone(),
        };
        if rel.is_empty() {
            continue;
        }
        let out_path = target.join(&rel);
        if !out_path.starts_with(&target) {
            continue; // 防 zip-slip
        }
        if name.ends_with('/') {
            let _ = fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            fs::write(&out_path, bytes).map_err(|e| e.to_string())?;
        }
    }

    if !target.join("manifest.json").is_file() {
        let _ = fs::remove_dir_all(&target);
        return Err("导入失败:zip 里没有 manifest.json".to_string());
    }
    let state = read_state(&dir);
    read_plugin_info(&target, &state).ok_or_else(|| "导入的插件 manifest 无效".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_args_skips_missing_key_and_fills_defaults() {
        let manifest = serde_json::json!({
            "args": [
                {"key": "mode", "label": "方式", "type": "select", "options": ["a", "b"], "default": "a"},
                {"label": "没有 key 的参数"},
                {"key": "rows", "type": "number", "default": 20}
            ]
        });
        let specs = parse_arg_specs(&manifest);
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].kind, "select");
        assert_eq!(specs[0].options, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(specs[1].label, "rows");

        let merged = merge_arg_values(&specs, Some(&serde_json::json!({"mode": "b", "unknown": 1})));
        assert_eq!(merged.get("mode").unwrap(), &serde_json::json!("b"));
        assert_eq!(merged.get("rows").unwrap(), &serde_json::json!(20));
        assert!(!merged.contains_key("unknown"));
    }

    #[test]
    fn plugin_state_round_trips_args() {
        let body = serde_json::json!({
            "disabled": ["p1"],
            "args": {"p1": {"mode": "b"}}
        });
        let parsed: serde_json::Value = serde_json::from_str(&body.to_string()).unwrap();
        assert_eq!(str_array(&parsed, "disabled"), vec!["p1".to_string()]);
        assert_eq!(
            parsed.get("args").and_then(|v| v.get("p1")).and_then(|v| v.get("mode")),
            Some(&serde_json::json!("b"))
        );
    }
}
