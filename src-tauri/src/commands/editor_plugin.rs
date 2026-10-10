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

struct BuiltinPlugin {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    surface: &'static str,
    capability: &'static str,
    scope: &'static str,
    main: &'static str,
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
    },
    BuiltinPlugin {
        id: "code-tidy",
        name: "代码块整理",
        description: "选中含 ``` 围栏的文本:去行尾空白、Tab 转空格,JSON 块自动美化缩进",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_CODE_TIDY,
    },
    BuiltinPlugin {
        id: "cite-tidy",
        name: "参考文献整理",
        description: "选中参考条目:去掉原有符号/编号、折叠多余空白,统一重新编号",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_CITE_TIDY,
    },
    BuiltinPlugin {
        id: "data-summary",
        name: "数据摘要",
        description: "选中一组数字:在下方追加计数/求和/均值/中位数/极值/标准差摘要表",
        surface: "editor",
        capability: "transform",
        scope: "selection",
        main: BUILTIN_DATA_SUMMARY,
    },
    BuiltinPlugin {
        id: "frontmatter-doctor",
        name: "frontmatter 体检",
        description: "遍历知识库 .md:报告缺失 title/date/tags 与非法日期格式的笔记",
        surface: "knowledge-base",
        capability: "generator",
        scope: "kb",
        main: BUILTIN_FRONTMATTER_DOCTOR,
    },
];

/// 把内置插件写入插件目录。用 `_builtins.json` 记录"已供给过"的 id,保证每个内置插件
/// 一生只写入一次:用户此后可自由编辑或删除,不会被重新生成;将来新增的内置插件会补写。
fn provision_builtin_plugins(dir: &Path) {
    let marker = dir.join("_builtins.json");
    let mut done: Vec<String> = fs::read_to_string(&marker)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .map(|v| str_array(&v, "provisioned"))
        .unwrap_or_default();
    let mut changed = false;
    for b in BUILTIN_PLUGINS {
        if done.iter().any(|d| d == b.id) {
            continue;
        }
        let pdir = dir.join(b.id);
        if !pdir.exists() && fs::create_dir_all(&pdir).is_ok() {
            let manifest = serde_json::json!({
                "id": b.id,
                "name": b.name,
                "description": b.description,
                "entry": "main.py",
                "capabilities": [b.capability],
                "surfaces": [b.surface],
                "scope": b.scope,
            });
            let _ = fs::write(
                pdir.join("manifest.json"),
                serde_json::to_string_pretty(&manifest).unwrap_or_default(),
            );
            let _ = fs::write(pdir.join("main.py"), b.main);
        }
        done.push(b.id.to_string());
        changed = true;
    }
    if changed {
        let _ = fs::write(
            &marker,
            serde_json::to_string_pretty(&serde_json::json!({ "provisioned": done }))
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

fn read_disabled_set(dir: &Path) -> Vec<String> {
    match fs::read_to_string(state_path(dir)) {
        Ok(raw) => serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .map(|v| str_array(&v, "disabled"))
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn write_disabled_set(dir: &Path, disabled: &[String]) -> Result<(), String> {
    let body = serde_json::json!({ "disabled": disabled });
    fs::write(
        state_path(dir),
        serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string()),
    )
    .map_err(|e| format!("write plugin state failed: {e}"))
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

fn read_plugin_info(plugin_dir: &Path, disabled: &[String]) -> Option<EditorPluginInfo> {
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
        enabled: !disabled.iter().any(|d| d == &id),
    })
}

fn collect_all_plugins(dir: &Path) -> Vec<EditorPluginInfo> {
    let disabled = read_disabled_set(dir);
    let mut plugins = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            // 跳过下划线开头的内部目录(_state.json、_exports 等)
            if p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with('_')).unwrap_or(false) {
                continue;
            }
            if let Some(info) = read_plugin_info(&p, &disabled) {
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
    let mut disabled = read_disabled_set(&dir);
    if enabled {
        disabled.retain(|d| d != &id);
    } else if !disabled.iter().any(|d| d == &id) {
        disabled.push(id);
    }
    write_disabled_set(&dir, &disabled)
}

#[command]
pub fn delete_plugin(id: String) -> Result<(), String> {
    let dir = editor_plugins_dir()?;
    let target = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    if !target.starts_with(&dir) {
        return Err("refuse to delete outside plugins dir".to_string());
    }
    fs::remove_dir_all(&target).map_err(|e| format!("delete plugin failed: {e}"))?;
    let mut disabled = read_disabled_set(&dir);
    disabled.retain(|d| d != id.trim());
    let _ = write_disabled_set(&dir, &disabled);
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
    let disabled = read_disabled_set(&dir);
    read_plugin_info(&plugin_dir, &disabled).ok_or_else(|| "create plugin failed".to_string())
}

#[command]
pub fn read_plugin_entry(id: String) -> Result<PluginEntrySource, String> {
    let dir = editor_plugins_dir()?;
    let disabled = read_disabled_set(&dir);
    let plugin_dir = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    let info = read_plugin_info(&plugin_dir, &disabled).ok_or_else(|| format!("plugin invalid: {id}"))?;
    let content = fs::read_to_string(&info.entry_path).map_err(|e| format!("read entry failed: {e}"))?;
    Ok(PluginEntrySource { id: info.id, entry: info.entry, path: info.entry_path, content })
}

#[command]
pub fn write_plugin_entry(id: String, content: String) -> Result<(), String> {
    let dir = editor_plugins_dir()?;
    let disabled = read_disabled_set(&dir);
    let plugin_dir = find_plugin_dir(&dir, id.trim()).ok_or_else(|| format!("plugin not found: {id}"))?;
    let info = read_plugin_info(&plugin_dir, &disabled).ok_or_else(|| format!("plugin invalid: {id}"))?;
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
    let disabled = read_disabled_set(&dir);
    read_plugin_info(&target, &disabled).ok_or_else(|| "导入的插件 manifest 无效".to_string())
}



