//! cc-switch 集成 — 只读探测
//!
//! cc-switch（<https://github.com/farion1231/cc-switch>）是第三方供应商切换工具。Guaxi 的接入
//! 方式是"走它的本地代理"：把 base_url 指向 cc-switch 监听的地址，供应商切换、故障转移、用量统计
//! 全部交给 cc-switch，Guaxi 侧只保存一个入口地址。
//!
//! 本模块只做**只读探测**，用于设置页的"自动探测"按钮回填端口：
//! - `~/.cc-switch/settings.json` → `enableLocalProxy` / `currentProviderCodex`
//! - `~/.cc-switch/cc-switch.db`（SQLite）→ `proxy_config` 表的监听地址端口、`providers` 表的当前项名
//!
//! 刻意不做的事：
//! - 不写 cc-switch 的任何文件或数据库（它的 schema 会随版本变化，写入风险太高）
//! - 不读 `providers.settings_config`（里面是别家服务的密钥）
//! - 任一环节失败都降级为 `found = false` + 提示文案，绝不让主流程报错

use serde::Serialize;
use std::path::PathBuf;
use tracing::{info, warn};

/// cc-switch 代理的默认监听端口（proxy_config.listen_port 的出厂值）。
const DEFAULT_PROXY_PORT: u16 = 15721;

#[derive(Serialize, Debug, Default)]
pub struct CcSwitchDetection {
    /// 是否找到 cc-switch 安装目录。为 false 时其余字段无意义，只看 hint。
    pub found: bool,
    pub home: String,
    /// cc-switch 是否已开启本地代理（settings.json 的开关与 proxy_config 的 enabled 同时为真）。
    pub proxy_enabled: bool,
    pub listen_address: String,
    pub listen_port: u16,
    /// 拼好的 OpenAI 兼容入口，可直接回填到设置页。
    pub base_url: String,
    pub current_provider_id: String,
    pub current_provider_name: String,
    pub hint: String,
}

fn user_home() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

fn cc_switch_home() -> Option<PathBuf> {
    let dir = user_home()?.join(".cc-switch");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// 读 settings.json 里的开关与当前 Codex 供应商 id。文件缺失/损坏时返回默认值而不报错。
fn read_settings(home: &PathBuf) -> (bool, String) {
    let path = home.join("settings.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (false, String::new());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        warn!(path = %path.to_string_lossy(), "[cc_switch] settings.json 解析失败");
        return (false, String::new());
    };

    let enable_local_proxy = value
        .get("enableLocalProxy")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let current = value
        .get("currentProviderCodex")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    (enable_local_proxy, current)
}

/// 读 cc-switch.db。
///
/// 设计说明：cc-switch 运行时对该库持有 WAL 写连接，直接以只读方式打开原文件在 Windows 上
/// 可能因锁失败，因此先复制到临时目录再查。复制失败就跳过，返回 None 让上层降级为默认端口。
fn read_db(home: &PathBuf, provider_id: &str) -> Option<(bool, String, u16, String)> {
    let db_path = home.join("cc-switch.db");
    if !db_path.is_file() {
        return None;
    }

    let tmp_path = std::env::temp_dir().join("guaxi-cc-switch-probe.db");
    if let Err(err) = std::fs::copy(&db_path, &tmp_path) {
        warn!(error = %err, "[cc_switch] 复制 cc-switch.db 失败，跳过数据库探测");
        return None;
    }

    let conn = match rusqlite::Connection::open_with_flags(
        &tmp_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(c) => c,
        Err(err) => {
            warn!(error = %err, "[cc_switch] 打开 cc-switch.db 副本失败");
            return None;
        }
    };

    let proxy = conn
        .query_row(
            "SELECT enabled, listen_address, listen_port FROM proxy_config WHERE app_type = 'codex'",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0).unwrap_or(0) != 0,
                    row.get::<_, String>(1)
                        .unwrap_or_else(|_| "127.0.0.1".to_string()),
                    row.get::<_, i64>(2).unwrap_or(DEFAULT_PROXY_PORT as i64) as u16,
                ))
            },
        )
        .ok();

    // 当前供应商名：优先用 settings.json 给的 id，取不到就退回 is_current 标记。
    let name = conn
        .query_row(
            "SELECT name FROM providers WHERE app_type = 'codex' AND id = ?1",
            [provider_id],
            |row| row.get::<_, String>(0),
        )
        .or_else(|_| {
            conn.query_row(
                "SELECT name FROM providers WHERE app_type = 'codex' AND is_current = 1",
                [],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap_or_default();

    let (enabled, address, port) =
        proxy.unwrap_or((false, "127.0.0.1".to_string(), DEFAULT_PROXY_PORT));
    Some((enabled, address, port, name))
}

/// 探测本机 cc-switch 配置，供设置页"自动探测"按钮回填。
#[tauri::command]
pub fn detect_cc_switch_config() -> CcSwitchDetection {
    let Some(home) = cc_switch_home() else {
        return CcSwitchDetection {
            found: false,
            hint: "未找到 ~/.cc-switch 目录，请确认已安装 cc-switch，或手动填写代理地址".to_string(),
            listen_port: DEFAULT_PROXY_PORT,
            base_url: format!("http://127.0.0.1:{}/v1", DEFAULT_PROXY_PORT),
            ..Default::default()
        };
    };

    let (enable_local_proxy, provider_id) = read_settings(&home);
    let (db_enabled, address, port, provider_name) = read_db(&home, &provider_id)
        .unwrap_or((false, "127.0.0.1".to_string(), DEFAULT_PROXY_PORT, String::new()));

    // 两个开关都为真才算代理真正可用：settings.json 是总开关，proxy_config 是按 app_type 的开关。
    let proxy_enabled = enable_local_proxy && db_enabled;
    let hint = if proxy_enabled {
        "已检测到 cc-switch 本地代理".to_string()
    } else if enable_local_proxy {
        "cc-switch 总开关已开，但 Codex 代理未启用；请在 cc-switch 中为 Codex 打开本地代理"
            .to_string()
    } else {
        "cc-switch 未启用本地代理，请在 cc-switch 设置中开启后重新探测".to_string()
    };

    let detection = CcSwitchDetection {
        found: true,
        home: home.to_string_lossy().to_string(),
        proxy_enabled,
        base_url: format!("http://{}:{}/v1", address, port),
        listen_address: address,
        listen_port: port,
        current_provider_id: provider_id,
        current_provider_name: provider_name,
        hint,
    };

    info!(
        proxy_enabled = detection.proxy_enabled,
        base_url = %detection.base_url,
        provider = %detection.current_provider_name,
        "[cc_switch] detection finished"
    );
    detection
}
