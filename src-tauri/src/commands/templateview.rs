use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, WebviewUrl, WebviewWindowBuilder};
use tauri::Manager;
use urlencoding::encode;

#[derive(Serialize, Debug, Clone)]
pub struct TemplateEntry {
    pub name: String,
    pub template_type: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct TemplateData {
    pub name: String,
    pub template_type: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TemplateMeta {
    template_type: String,
}

fn normalize_template_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("模板名称不能为空".to_string());
    }
    if n.contains('/') || n.contains('\\') || n.contains("..") {
        return Err("模板名称不合法".to_string());
    }
    Ok(n.to_string())
}

fn template_root_dir() -> Result<PathBuf, String> {
    crate::commands::settingview::init_template_root()?;
    let root = crate::commands::settingview::get_template_repo_path();
    Ok(PathBuf::from(root))
}

fn template_md_path(name: &str) -> Result<PathBuf, String> {
    let n = normalize_template_name(name)?;
    let file_name = if n.to_lowercase().ends_with(".md") {
        n
    } else {
        format!("{n}.md")
    };
    Ok(template_root_dir()?.join(file_name))
}

fn template_meta_path(stem_name: &str) -> Result<PathBuf, String> {
    let n = normalize_template_name(stem_name)?;
    Ok(template_root_dir()?.join(format!("{n}.meta.json")))
}

fn read_meta(stem_name: &str) -> TemplateMeta {
    let default = TemplateMeta {
        template_type: "未分类".to_string(),
    };
    let Ok(path) = template_meta_path(stem_name) else {
        return default;
    };
    let Ok(s) = fs::read_to_string(path) else {
        return default;
    };
    serde_json::from_str(&s).unwrap_or(default)
}

fn write_meta(stem_name: &str, template_type: &str) -> Result<(), String> {
    let path = template_meta_path(stem_name)?;
    let meta = TemplateMeta {
        template_type: template_type.trim().to_string(),
    };
    fs::write(path, serde_json::to_string(&meta).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn is_md_file(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

fn file_stem_string(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[tauri::command]
pub async fn create_template_manager_window(
    app: tauri::AppHandle,
    label: String,
    title: String,
    theme: Option<String>,
) -> Result<(), String> {
    if app.get_webview_window(&label).is_some() {
        return Err(format!("窗口 {} 已存在", label));
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/templates?theme={}", encode(&theme)).into())
    } else {
        WebviewUrl::App("/templates".into())
    };

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1000.0, 720.0)
        .min_inner_size(820.0, 560.0)
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn open_application_window(
    app: tauri::AppHandle,
    app_id: String,
    title: String,
    theme: Option<String>,
) -> Result<(), String> {
    let app_id = app_id.trim().to_string();
    if app_id.is_empty() {
        return Err("app_id 不能为空".to_string());
    }

    let label = format!("app-{}", app_id);

    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(|e| format!("显示窗口失败: {}", e))?;
        w.set_focus().ok();
        return Ok(());
    }

    let theme = theme.unwrap_or_default().trim().to_lowercase();
    let url = if theme == "dark" || theme == "light" {
        WebviewUrl::App(format!("/app/{}?theme={}", app_id, encode(&theme)).into())
    } else {
        WebviewUrl::App(format!("/app/{}", app_id).into())
    };

    // 工作台类应用需要完整三栏空间；普通小工具继续使用紧凑窗口。
    // 说明：inner_size 只作为“还原”尺寸，取一个能放进常见 1366×768 笔记本的保守值，
    // 避免初始尺寸超过屏幕导致部分内容在屏幕外；工作台类应用随后直接最大化。
    let (inner_w, inner_h, min_w, min_h, maximize) = match app_id.as_str() {
        "creation-ai-film-studio" => (1280.0, 800.0, 1024.0, 640.0, true),
        "tools-tts-book" => (1280.0, 800.0, 1024.0, 680.0, true),
        "tools-remote-terminal" => (1280.0, 800.0, 1024.0, 640.0, true),
        "tools-resource-hunter" => (1280.0, 800.0, 1024.0, 640.0, true),
        // 邮箱客户端：三栏 + 阅读区，窄了会挤掉邮件列表，和工作台一样默认最大化。
        "tools-mail" => (1440.0, 900.0, 1120.0, 680.0, true),
        // 简历工坊：简历库 + 编辑器 + 预览三栏，窄了预览会被压成一条。
        "work-resume" => (1440.0, 900.0, 1120.0, 680.0, true),
        _ => (1024.0, 720.0, 820.0, 560.0, false),
    };

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(inner_w, inner_h)
        .min_inner_size(min_w, min_h)
        // 先居中，保证即使不最大化，还原尺寸也完整落在主显示器内。
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口失败: {}", e))?;

    if maximize {
        // 工作台类应用默认最大化以获得完整布局；用户仍可还原为居中的 inner_size。
        window.maximize().ok();
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    window.show().map_err(|e| format!("显示窗口失败: {}", e))?;
    window.set_focus().ok();
    Ok(())
}

#[tauri::command]
pub fn emit_template_insert(app: AppHandle, content: String) -> Result<(), String> {
    let payload = serde_json::json!({ "content": content });
    app.emit("template_insert_event", payload).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_templates() -> Result<Vec<TemplateEntry>, String> {
    let root = template_root_dir()?;
    let entries = fs::read_dir(&root).map_err(|e| e.to_string())?;

    let mut items: Vec<TemplateEntry> = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() || !is_md_file(&path) {
            continue;
        }

        let stem = file_stem_string(&path);
        if stem.is_empty() {
            continue;
        }

        let meta = read_meta(&stem);
        items.push(TemplateEntry {
            name: stem,
            template_type: if meta.template_type.trim().is_empty() {
                "未分类".to_string()
            } else {
                meta.template_type
            },
        });
    }

    items.sort_by(|a, b| {
        let ta = a.template_type.to_lowercase();
        let tb = b.template_type.to_lowercase();
        ta.cmp(&tb).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(items)
}

#[tauri::command]
pub fn read_template(name: String) -> Result<TemplateData, String> {
    let name = normalize_template_name(&name)?;
    let md_path = template_md_path(&name)?;
    let content = fs::read_to_string(&md_path).map_err(|e| e.to_string())?;
    let meta = read_meta(&name);

    Ok(TemplateData {
        name,
        template_type: if meta.template_type.trim().is_empty() {
            "未分类".to_string()
        } else {
            meta.template_type
        },
        content,
    })
}

#[tauri::command]
pub fn save_template(name: String, template_type: String, content: String) -> Result<(), String> {
    let name = normalize_template_name(&name)?;
    let md_path = template_md_path(&name)?;
    fs::write(&md_path, content).map_err(|e| e.to_string())?;
    let t = if template_type.trim().is_empty() {
        "未分类"
    } else {
        template_type.trim()
    };
    write_meta(&name, t)?;
    Ok(())
}

#[tauri::command]
pub fn rename_template(old_name: String, new_name: String) -> Result<(), String> {
    let old_n = normalize_template_name(&old_name)?;
    let new_n = normalize_template_name(&new_name)?;
    if old_n == new_n {
        return Ok(());
    }

    let old_md = template_md_path(&old_n)?;
    let new_md = template_md_path(&new_n)?;
    if !old_md.exists() {
        return Err("原模板不存在".to_string());
    }
    if new_md.exists() {
        return Err("目标模板已存在".to_string());
    }

    fs::rename(&old_md, &new_md).map_err(|e| e.to_string())?;

    let old_meta = template_meta_path(&old_n)?;
    let new_meta = template_meta_path(&new_n)?;
    if old_meta.exists() && !new_meta.exists() {
        let _ = fs::rename(&old_meta, &new_meta);
    }

    Ok(())
}

#[tauri::command]
pub fn delete_template(name: String) -> Result<(), String> {
    let name = normalize_template_name(&name)?;
    let md_path = template_md_path(&name)?;
    if md_path.exists() {
        fs::remove_file(md_path).map_err(|e| e.to_string())?;
    }

    let meta_path = template_meta_path(&name)?;
    if meta_path.exists() {
        let _ = fs::remove_file(meta_path);
    }

    Ok(())
}

#[tauri::command]
pub fn import_template_from_file(
    source_path: String,
    name: String,
    template_type: String,
) -> Result<(), String> {
    let src = PathBuf::from(source_path);
    if !src.exists() {
        return Err("源文件不存在".to_string());
    }
    if !src.is_file() {
        return Err("源路径不是文件".to_string());
    }
    if !is_md_file(&src) {
        return Err("仅支持导入 Markdown 文件".to_string());
    }

    let template_name = normalize_template_name(&name)?;
    let content = fs::read_to_string(&src).map_err(|e| e.to_string())?;

    save_template(
        template_name,
        if template_type.trim().is_empty() {
            "未分类".to_string()
        } else {
            template_type
        },
        content,
    )
}
