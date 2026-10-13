//! 命令下发：配置驱动的批量设备 CLI 执行。
//!
//! 设计来源是 `cfgtool/Execmd` 那套工具（config.yaml → 展开 → DAG 排序 → 交互式 CLI 逐条下发），
//! 本模块把它做进终端调试工坊，并补齐健壮性：
//! - 配置格式**完全兼容**原工具：`global` / `targets{hostnames,domains}` / `tasks[...]` 原样可跑。
//! - 展开规则照搬（`only_if_domain` / `only_if_no_domain` / 命令含 `{domain}` 三个分支）。
//! - 依赖解析在"同 host+domain"基础上**优先匹配同 index**，解决原工具里同 id 多条（index 1/2/3）
//!   时 `exact[0]` 可能选错的问题；歧义与环都在计划里明确报出来。
//! - 执行走交互式 shell（`>` / `#` / `(config)#` 提示符状态机、enable、PEM 粘贴、yes/no、分页），
//!   见 [`DeviceCli`]。

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::command;

// ============================ 配置模型（兼容参考工具的 config.yaml） ============================

/// `index: 1` 与 `index: "1"` 都要能读——参考配置里写的是数字。
fn de_opt_string<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<serde_yaml::Value>::deserialize(d)?;
    Ok(v.and_then(|v| match v {
        serde_yaml::Value::String(s) => Some(s),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalCfg {
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    /// 设备 CLI 的 enable 口令（进特权模式 `#` 用）。
    #[serde(default)]
    pub enable_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Targets {
    #[serde(default)]
    pub hostnames: Vec<String>,
    /// 空串表示"不关联域名（host 级）"。
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub id: String,
    /// 命令模板，支持 `{hostname}` `{domain}` `{index}` 占位符。
    pub cmd: String,
    #[serde(default, deserialize_with = "de_opt_string")]
    pub pem_file: Option<String>,
    #[serde(default, deserialize_with = "de_opt_string")]
    pub index: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// 只在关联了域名的场景执行（SNI 流程）。
    #[serde(default)]
    pub only_if_domain: bool,
    /// 只在 host 级（无域名）执行。
    #[serde(default)]
    pub only_if_no_domain: bool,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DispatchConfig {
    #[serde(default)]
    pub global: GlobalCfg,
    #[serde(default)]
    pub targets: Targets,
    #[serde(default)]
    pub tasks: Vec<TaskTemplate>,
    /// 失败后是否继续（默认 false：失败即停，避免级联错误）。
    #[serde(default)]
    pub continue_on_error: bool,
}

// ============================ 展开：模板 → 具体任务 ============================

#[derive(Debug, Clone, Serialize)]
pub struct ConcreteTask {
    /// 全局唯一键：`id|hostname|domain|index`。
    pub task_key: String,
    pub id: String,
    pub cmd: String,
    pub pem_file: String,
    pub hostname: String,
    pub domain: String,
    pub index: String,
    pub depends_on: Vec<String>,
    pub description: Option<String>,
}

/// 渲染 `{hostname}` / `{domain}` / `{index}`（以及 `{id}`）。
///
/// 手写替换而不用 `format!`：命令里可能含 `{`（如正则/JSON），`format!` 会直接炸。
/// 未知占位符原样保留，便于用户在输出里看出自己写错了。
fn render(tpl: &str, t: &ConcreteTask) -> String {
    let mut out = String::with_capacity(tpl.len() + 16);
    let bytes: Vec<char> = tpl.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '{' {
            if let Some(close) = bytes[i + 1..].iter().position(|c| *c == '}') {
                let key: String = bytes[i + 1..i + 1 + close].iter().collect();
                let val = match key.as_str() {
                    "hostname" | "host" | "h" => Some(t.hostname.clone()),
                    "domain" | "d" => Some(t.domain.clone()),
                    "index" | "idx" | "i" => Some(t.index.clone()),
                    "id" => Some(t.id.clone()),
                    _ => None,
                };
                if let Some(v) = val {
                    out.push_str(&v);
                    i += close + 2;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// 把模板任务展开成具体任务（照搬 task_engine 的三分支域名选择）。
pub fn expand(cfg: &DispatchConfig) -> Vec<ConcreteTask> {
    let mut out = Vec::new();
    for t in &cfg.tasks {
        let iter_domains: Vec<String> = if t.only_if_domain {
            cfg.targets.domains.iter().filter(|d| !d.is_empty()).cloned().collect()
        } else if t.only_if_no_domain {
            vec![String::new()]
        } else if t.cmd.contains("{domain}") {
            cfg.targets.domains.clone()
        } else {
            vec![String::new()]
        };
        for h in &cfg.targets.hostnames {
            for d in &iter_domains {
                if t.only_if_domain && d.is_empty() {
                    continue;
                }
                if t.only_if_no_domain && !d.is_empty() {
                    continue;
                }
                let index = t.index.clone().unwrap_or_default();
                let mut task = ConcreteTask {
                    task_key: format!("{}|{}|{}|{}", t.id, h, d, index),
                    id: t.id.clone(),
                    cmd: String::new(),
                    pem_file: String::new(),
                    hostname: h.clone(),
                    domain: d.clone(),
                    index,
                    depends_on: t.depends_on.clone(),
                    description: t.description.clone(),
                };
                task.cmd = render(&t.cmd, &task);
                task.pem_file = t
                    .pem_file
                    .as_ref()
                    .map(|p| render(p, &task))
                    .unwrap_or_default();
                out.push(task);
            }
        }
    }
    out
}

// ============================ DAG：依赖解析 + 拓扑排序 ============================

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub steps: Vec<ConcreteTask>,
    /// 依赖匹配有歧义等需要用户知道但不致命的情况。
    pub warnings: Vec<String>,
    pub task_count: usize,
    pub target_count: usize,
}

/// 展开 + 拓扑排序，产出"将要逐条执行的命令"。**不下发任何命令**。
pub fn build_plan(cfg: &DispatchConfig) -> Result<Plan, String> {
    if cfg.targets.hostnames.is_empty() {
        return Err("targets.hostnames 为空：至少需要一个目标主机名".to_string());
    }
    if cfg.tasks.is_empty() {
        return Err("tasks 为空：至少需要一条任务".to_string());
    }
    let tasks = expand(cfg);
    let mut warnings: Vec<String> = Vec::new();

    // 按逻辑 id 分组，供 depends_on 解析。
    let mut by_id: HashMap<&str, Vec<&ConcreteTask>> = HashMap::new();
    for t in &tasks {
        by_id.entry(t.id.as_str()).or_default().push(t);
    }

    // 依赖按逻辑 id 声明、执行在具体实例上，需要"作用域化"解析：
    //   1) 同 host + 同 domain + 同 index   2) 同 host + 同 domain
    //   3) 同 host + host 级(domain="") + 同 index   4) 同 host + host 级
    // index 优先是为了正确对应原工具里 index 1/2/3 的同名任务（如 import_cert_host）。
    let tiers: [fn(&&ConcreteTask, &ConcreteTask) -> bool; 4] = [
        |c, cur| c.hostname == cur.hostname && c.domain == cur.domain && c.index == cur.index,
        |c, cur| c.hostname == cur.hostname && c.domain == cur.domain,
        |c, cur| c.hostname == cur.hostname && c.domain.is_empty() && c.index == cur.index,
        |c, cur| c.hostname == cur.hostname && c.domain.is_empty(),
    ];

    let key_set: HashSet<String> = tasks.iter().map(|t| t.task_key.clone()).collect();
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for t in &tasks {
        in_degree.entry(t.task_key.clone()).or_insert(0);
    }
    for t in &tasks {
        for dep in &t.depends_on {
            let dep_key = if key_set.contains(dep) {
                // 也允许直接写具体 task_key
                dep.clone()
            } else {
                let cands = by_id.get(dep.as_str()).cloned().unwrap_or_default();
                if cands.is_empty() {
                    return Err(format!(
                        "依赖找不到：任务 `{}`（host={} domain={} index={}）依赖 `{}`，\
                         但没有同名任务（检查 depends_on 拼写）",
                        t.id, t.hostname, t.domain, t.index, dep
                    ));
                }
                let mut chosen: Option<String> = None;
                for f in tiers {
                    let hit: Vec<&&ConcreteTask> = cands.iter().filter(|c| f(c, t)).collect();
                    if hit.is_empty() {
                        continue;
                    }
                    if hit.len() > 1 {
                        warnings.push(format!(
                            "依赖 `{}` 在任务 `{}` 的作用域内匹配到 {} 个候选，已取第一个（{}）；\
                             如需精确指定可在 depends_on 里直接写 task_key",
                            dep,
                            t.task_key,
                            hit.len(),
                            hit[0].task_key
                        ));
                    }
                    chosen = Some(hit[0].task_key.clone());
                    break;
                }
                match chosen {
                    Some(k) => k,
                    None => {
                        return Err(format!(
                            "依赖找不到：任务 `{}`（host={} domain={} index={}）依赖 `{}`，\
                             但没有任何候选落在同 host/host 级作用域内（检查 targets 与 only_if_* 条件）",
                            t.id, t.hostname, t.domain, t.index, dep
                        ))
                    }
                }
            };
            if dep_key == t.task_key {
                return Err(format!("任务 `{}` 依赖了自己", t.task_key));
            }
            graph.entry(dep_key.clone()).or_default().push(t.task_key.clone());
            *in_degree.entry(t.task_key.clone()).or_insert(0) += 1;
        }
    }

    // Kahn 拓扑排序。
    let order: Vec<String> = tasks.iter().map(|t| t.task_key.clone()).collect();
    let mut queue: VecDeque<String> = order
        .iter()
        .filter(|k| in_degree.get(*k).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();
    let mut sorted_keys: Vec<String> = Vec::new();
    while let Some(k) = queue.pop_front() {
        sorted_keys.push(k.clone());
        if let Some(nexts) = graph.get(&k) {
            for n in nexts {
                if let Some(d) = in_degree.get_mut(n) {
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(n.clone());
                    }
                }
            }
        }
    }

    if sorted_keys.len() != tasks.len() {
        // 把还留在环里的节点列出来，而不是只回一句 "cycle detected"。
        let done: HashSet<&String> = sorted_keys.iter().collect();
        let stuck: Vec<String> = tasks
            .iter()
            .filter(|t| !done.contains(&&t.task_key))
            .map(|t| format!("{} ({} → {})", t.task_key, t.cmd, t.depends_on.join(",")))
            .collect();
        return Err(format!(
            "依赖存在环，以下任务互相等待、无法排序：\n{}",
            stuck.join("\n")
        ));
    }

    let by_key: HashMap<String, ConcreteTask> =
        tasks.into_iter().map(|t| (t.task_key.clone(), t)).collect();
    let steps: Vec<ConcreteTask> = sorted_keys
        .into_iter()
        .filter_map(|k| by_key.get(&k).cloned())
        .collect();

    Ok(Plan {
        target_count: cfg.targets.hostnames.len(),
        task_count: steps.len(),
        steps,
        warnings,
    })
}

/// 从 YAML 文本解析配置（带可读的报错）。
pub fn parse_config(yaml: &str) -> Result<DispatchConfig, String> {
    serde_yaml::from_str::<DispatchConfig>(yaml).map_err(|e| {
        let loc = e
            .location()
            .map(|l| format!("第 {} 行第 {} 列: ", l.line(), l.column()))
            .unwrap_or_default();
        format!("配置解析失败：{loc}{e}")
    })
}

/// **只做展开 + 排序，不下发**：给 UI 预览与配置校验用。
#[command]
pub fn cmd_dispatch_plan(config_yaml: String) -> Result<Plan, String> {
    let cfg = parse_config(&config_yaml)?;
    build_plan(&cfg)
}

/// YAML → 结构化配置（给可视化任务表加载）。YAML 只由后端解析，避免前端再引一个 YAML 库。
#[command]
pub fn cmd_dispatch_parse(config_yaml: String) -> Result<DispatchConfig, String> {
    parse_config(&config_yaml)
}

/// 结构化配置 → YAML（任务表改动回写成 YAML，供预览/运行/导出）。
#[command]
pub fn cmd_dispatch_to_yaml(config: DispatchConfig) -> Result<String, String> {
    serde_yaml::to_string(&config).map_err(|e| format!("生成 YAML 失败: {e}"))
}

// __DISPATCH_RUNTIME__

// ============================ 设备交互引擎 ============================
//
// 网络设备的 CLI 是**交互式**的：要 `enable` 进特权、`config terminal force` 进配置模式，
// 导入证书/私钥时设备会进入"粘贴 PEM"状态（提示 `use "..." on a single line`，以单独一行 `...` 结束），
// 还会出现 yes/no 确认与 `--more--` 分页。所以这里不是 exec 一条命令，而是维持一个 shell 会话 +
// 提示符状态机 —— 与参考工具 executor.py 的模型一致。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::commands::ssh_terminal::{
    connect_authenticated, connect_explicit, enable_secret, ClientHandler,
};

/// 每条命令等待提示符的上限（设备慢/证书链长时给足）。
const CMD_TIMEOUT: Duration = Duration::from_secs(60);
/// PEM 导入后设备要解析证书，给更长。
const PEM_TIMEOUT: Duration = Duration::from_secs(120);
/// 探测提示符时的"安静期"。
const QUIET: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// 网络设备 CLI 的用户模式 `>`。
    User,
    /// 网络设备 CLI 的特权模式 `#`。
    Priv,
    /// 网络设备 CLI 的配置模式 `(config...)#`。
    Config,
    /// POSIX shell 提示符（Linux 编译机）：`user@host:~$` 或 `user@host:~#`。
    /// 这种目标不需要 enable/config terminal，命令就是 shell 命令。
    Shell,
    Password,
    PemWait,
    Unknown,
}

/// 会话类型：设备 CLI（要 enable/config）还是普通 Linux shell。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SessionKind {
    /// 自动探测（默认）：看首个提示符长什么样再决定。
    #[default]
    Auto,
    Device,
    Shell,
}

/// 取缓冲区最后一行非空内容（提示符总在最后）。
fn last_line(buf: &str) -> &str {
    buf.rsplit(['\n', '\r']).find(|l| !l.trim().is_empty()).unwrap_or("").trim()
}

/// 按最后一行判定提示符状态。
fn classify_prompt(buf: &str) -> Prompt {
    let line = last_line(buf);
    if line.is_empty() {
        return Prompt::Unknown;
    }
    let low = line.to_lowercase();
    // 先判密码提示：以 `:` 结尾且含 password/口令，避免被当成普通回显。
    if (low.ends_with(':') || low.ends_with("password"))
        && (low.contains("password") || line.contains("口令") || line.contains("密码"))
    {
        return Prompt::Password;
    }
    // PEM 粘贴状态（各版本措辞不同）。
    if low.contains("use \"...\" on a single line")
        || low.contains("to terminate import")
        || line.contains("以单独一行")
    {
        return Prompt::PemWait;
    }
    // POSIX shell（Linux 编译机）：`user@host:path$` / `user@host:path#` / `[root@host ~]#`。
    // 必须先于设备 CLI 判定：Linux 的 root 提示符也以 `#` 结尾，靠 `user@host` 特征区分。
    if is_shell_prompt(line) {
        return Prompt::Shell;
    }
    if line.ends_with('#') {
        return if line.contains("(config") { Prompt::Config } else { Prompt::Priv };
    }
    if line.ends_with('>') {
        return Prompt::User;
    }
    Prompt::Unknown
}

/// 识别 POSIX shell 提示符。
///
/// 覆盖常见形态：`user@host:~$`、`user@host:/path#`、`[root@host ~]#`、`bash-4.2$`、裸 `$` / `#`。
/// 判据是"以 `$` 结尾，或以 `#` 结尾且带 `@host:` 或 `[...]` 这类 shell 特征" —— 后者用于把
/// Linux root 的 `#` 与设备 CLI 的特权 `#` 区分开。
fn is_shell_prompt(line: &str) -> bool {
    let t = line.trim_end();
    if t.ends_with('$') {
        return true;
    }
    if t.ends_with('#') {
        let head = &t[..t.len() - 1];
        // `user@host:...` / `[user@host ...]` / `bash-5.1` 这类都算 shell；
        // 而设备 CLI 的特权提示符通常是 `hostname#` 或 `hostname>`（无 @、无方括号）。
        return head.contains('@') || head.trim_start().starts_with('[') || head.contains("bash");
    }
    false
}

/// 把可能很长的设备输出裁成"尾部若干行"，用于错误提示。
fn tail_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// shell 模式下用来探真实退出码的标记。
const RC_MARK: &str = "__DISPATCH_RC=";

/// 交互式 CLI 会话（设备 CLI 或 Linux shell 都由它驱动）。
struct DeviceCli {
    channel: russh::Channel<russh::client::Msg>,
    buf: String,
    /// 我们已读过的输出累计（用于日志/诊断），不受 buf 清空影响。
    transcript: String,
    /// enable 口令：命令中途若掉出配置模式，重新进入时还要用；shell 模式下也复用为 sudo 口令。
    enable_password: Option<String>,
    /// 已探测出的会话类型（不会是 Auto）。
    kind: SessionKind,
    /// shell 模式是否追加退出码探针（`cmd; echo __DISPATCH_RC=$?`）。
    /// 关键词启发式对 shell 不可靠（`cp` 失败不会打印任何关键词），拿真实退出码准得多。
    probe_rc: bool,
}

/// 从缓冲区尾部若干行里找 shell 提示符（用于 Auto 模式的兜底判定）。
fn looks_like_shell(buf: &str) -> bool {
    buf.lines().rev().take(4).any(is_shell_prompt)
}

/// 取命令里的 heredoc 结束标记（`<<EOF` / `<<'EOF'` / `<<"EOF"`）。
///
/// shell 模式下要把一段 PEM 喂给命令（如 `cat > /etc/ssl/x.crt <<'EOF'`）时用它：
/// 命令发出后紧接着把 PEM 正文与结束标记写进去，shell 就会把它们当作 heredoc 内容。
fn heredoc_marker(cmd: &str) -> Option<String> {
    let i = cmd.find("<<")?;
    let rest = cmd[i + 2..].trim_start().trim_start_matches(['-', '\'', '"']);
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if end == 0 {
        None
    } else {
        Some(rest[..end].to_string())
    }
}

impl DeviceCli {
    async fn open(
        handle: russh::client::Handle<ClientHandler>,
        enable_password: Option<String>,
        requested: SessionKind,
        probe_rc: bool,
    ) -> Result<Self, String> {
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开会话通道失败: {e}"))?;
        // PTY 是必须的：设备 CLI 依据终端宽度分页、并按交互式提示符工作。
        channel
            .request_pty(false, "vt100", 200, 60, 0, 0, &[])
            .await
            .map_err(|e| format!("请求 PTY 失败: {e}"))?;
        channel
            .request_shell(true)
            .await
            .map_err(|e| format!("请求 shell 失败: {e}"))?;

        let mut cli = DeviceCli {
            channel,
            buf: String::new(),
            transcript: String::new(),
            enable_password,
            kind: SessionKind::Auto,
            probe_rc,
        };

        // 探测会话类型：发一个空行看回显出来的提示符。
        cli.buf.clear();
        cli.send_raw("\n").await;
        for _ in 0..3 {
            match cli.wait_prompt(Duration::from_secs(8)).await {
                Ok(_) => break,
                Err(e) => {
                    // 一次探不到就再试一次；仍不行才报错，并把原始输出一起给出来。
                    if cli.buf.trim().is_empty() {
                        return Err(e);
                    }
                    cli.buf.clear();
                    cli.send_raw("\n").await;
                }
            }
        }
        cli.kind = match requested {
            SessionKind::Auto => match classify_prompt(&cli.buf) {
                Prompt::Shell => SessionKind::Shell,
                Prompt::User | Prompt::Priv | Prompt::Config => SessionKind::Device,
                // 认不出来：看整个尾部像不像 Linux shell（有 user@host:path$ 之类）。
                _ => {
                    if looks_like_shell(&cli.buf) {
                        SessionKind::Shell
                    } else {
                        SessionKind::Device
                    }
                }
            },
            k => k,
        };

        if cli.kind == SessionKind::Device {
            let pw = cli.enable_password.clone();
            cli.ensure_config_mode(pw).await?;
        }
        Ok(cli)
    }

    /// 把通道数据读进缓冲区，直到安静 `quiet` 时长或超时。
    async fn pump(&mut self, overall: Duration) -> Result<(), String> {
        let deadline = Instant::now() + overall;
        loop {
            let remain = deadline.saturating_duration_since(Instant::now());
            if remain.is_zero() {
                return Ok(());
            }
            let wait = remain.min(QUIET);
            match tokio::time::timeout(wait, self.channel.wait()).await {
                Ok(Some(russh::ChannelMsg::Data { data }))
                | Ok(Some(russh::ChannelMsg::ExtendedData { data, .. })) => {
                    let text = String::from_utf8_lossy(&data).to_string();
                    self.transcript.push_str(&text);
                    self.buf.push_str(&text);
                }
                Ok(Some(russh::ChannelMsg::ExitStatus { .. })) | Ok(None) => {
                    return Err("设备连接已关闭".to_string())
                }
                Ok(Some(_)) => {}
                // 安静期到：认为设备这一轮输出结束
                Err(_) => return Ok(()),
            }
        }
    }

    /// 写一行到设备。**必须 await**：写入顺序决定命令顺序，spawn 出去会乱序。
    async fn send_raw(&mut self, s: &str) {
        let _ = self.channel.data(s.as_bytes()).await;
    }

    /// 应答设备中途抛出的交互（确认、分页），返回是否处理过。
    ///
    /// shell 模式下**不自动答 yes/no** —— Linux 上的 `rm -i`、`apt` 之类确认不该被代答；
    /// 但分页（`less`/`more` 的 `--more--` 或 `:`）仍自动翻，避免卡住。
    async fn answer_interaction(&mut self) -> bool {
        let low = self.buf.to_lowercase();
        if low.contains("--more--") || low.ends_with("\n:") || low.ends_with("\r:") {
            self.buf.clear();
            self.send_raw(" ").await;
            return true;
        }
        if self.kind != SessionKind::Shell {
            let confirm = ["type yes to continue", "yes/no", "continue:", "are you sure"];
            if confirm.iter().any(|k| low.contains(k)) {
                self.buf.clear();
                self.send_raw("YES\n").await;
                return true;
            }
        }
        false
    }

    /// 等一个稳定提示符；期间自动应答交互。
    async fn wait_prompt(&mut self, overall: Duration) -> Result<Prompt, String> {
        let deadline = Instant::now() + overall;
        loop {
            if self.answer_interaction().await {
                // 已应答，继续等
            } else if let p @ (Prompt::User
            | Prompt::Priv
            | Prompt::Config
            | Prompt::Shell
            | Prompt::Password
            | Prompt::PemWait) = classify_prompt(&self.buf)
            {
                return Ok(p);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "等待提示符超时（最后输出）：\n{}",
                    tail_lines(&self.buf, 8)
                ));
            }
            let remain = deadline.saturating_duration_since(Instant::now());
            self.pump(QUIET.min(remain)).await?;
        }
    }

    /// 确保处于配置模式 `(config)#`。
    ///
    /// shell 模式（Linux 编译机）没有这一套，直接返回。
    async fn ensure_config_mode(&mut self, enable_password: Option<String>) -> Result<(), String> {
        if self.kind == SessionKind::Shell {
            return Ok(());
        }
        self.buf.clear();
        self.send_raw("\n").await;
        match self.wait_prompt(Duration::from_secs(3)).await? {
            Prompt::Config => return Ok(()),
            Prompt::Priv => {
                self.buf.clear();
                self.send_raw("config terminal force\n").await;
                if self.wait_prompt(Duration::from_secs(6)).await? == Prompt::Config {
                    return Ok(());
                }
            }
            _ => {}
        }
        // 需要先进特权模式。
        self.buf.clear();
        self.send_raw("enable\n").await;
        let mut p = self.wait_prompt(Duration::from_secs(6)).await?;
        if p == Prompt::Password {
            self.buf.clear();
            self.send_raw(&format!("{}\n", enable_password.clone().unwrap_or_default())).await;
            p = self.wait_prompt(Duration::from_secs(6)).await?;
        }
        if p == Prompt::User {
            // 有些设备 enable 后还要补一次回车。
            self.buf.clear();
            self.send_raw("\n").await;
            p = self.wait_prompt(Duration::from_secs(3)).await?;
        }
        if p != Prompt::Priv && p != Prompt::Config {
            return Err(format!(
                "无法进入特权/配置模式（最后输出）：\n{}",
                tail_lines(&self.buf, 8)
            ));
        }
        if p == Prompt::Priv {
            self.buf.clear();
            self.send_raw("config terminal force\n").await;
            if self.wait_prompt(Duration::from_secs(6)).await? != Prompt::Config {
                return Err(format!(
                    "无法进入配置模式（最后输出）：\n{}",
                    tail_lines(&self.buf, 8)
                ));
            }
        }
        Ok(())
    }

    /// 执行一条命令；`pem` 非空时按各自的输入流程补送。
    ///
    /// - 设备 CLI：进配置模式后发命令；设备若进入 PEM 粘贴状态，补送 PEM + 单独一行 `...`。
    /// - shell 模式：直接发命令；若命令里有 heredoc（`<<EOF`）且带 pem，就把 PEM 当作 heredoc 内容喂进去。
    async fn run_cmd(&mut self, cmd: &str, pem: Option<&str>) -> TaskOutcome {
        let pw = self.enable_password.clone();
        if let Err(e) = self.ensure_config_mode(pw.clone()).await {
            let out = self.buf.clone();
            return TaskOutcome::failed(cmd, &out, e);
        }
        self.buf.clear();
        // shell 模式追加退出码探针：`命令; echo __DISPATCH_RC=$?`。
        // 对 heredoc 也成立（整行是一条命令行，正文从下一行开始）。
        let probe_rc = self.kind == SessionKind::Shell && self.probe_rc;
        let to_send = if probe_rc {
            format!("{cmd}; echo \"{RC_MARK}$?\"")
        } else {
            cmd.to_string()
        };
        self.send_raw(&format!("{to_send}\n")).await;

        // shell + heredoc：命令发出后立刻把正文与结束标记写进去。
        let mut heredoc_fed = false;
        if self.kind == SessionKind::Shell {
            if let (Some(marker), Some(body)) = (heredoc_marker(cmd), pem) {
                if !body.is_empty() {
                    let mut payload = String::with_capacity(body.len() + marker.len() + 4);
                    payload.push_str(body);
                    if !payload.ends_with('\n') {
                        payload.push('\n');
                    }
                    payload.push_str(&marker);
                    payload.push('\n');
                    self.send_raw(&payload).await;
                    heredoc_fed = true;
                }
            }
        }

        let mut allow_pem = pem.is_some() && !heredoc_fed;
        let mut used_pem = heredoc_fed;
        let mut output = String::new();
        // shell 模式下 sudo 之类可能讨一次口令，最多喂一次。
        let mut pw_tries = if self.kind == SessionKind::Shell && pw.is_some() { 1 } else { 0 };

        // 最多两轮：第一轮可能撞上 PEM 粘贴/口令提示，处理后再等一次。
        for _ in 0..3 {
            let budget = if used_pem { PEM_TIMEOUT } else { CMD_TIMEOUT };
            let p = match self.wait_prompt(budget).await {
                Ok(p) => p,
                Err(e) => {
                    output.push_str(&self.buf);
                    return TaskOutcome::failed(cmd, &output, e);
                }
            };
            match p {
                Prompt::PemWait if allow_pem => {
                    output.push_str(&self.buf);
                    self.buf.clear();
                    used_pem = true;
                    allow_pem = false;
                    let body = pem.unwrap_or("");
                    let mut payload = String::with_capacity(body.len() + 8);
                    payload.push_str(body);
                    if !payload.ends_with('\n') {
                        payload.push('\n');
                    }
                    payload.push_str("...\n");
                    self.send_raw(&payload).await;
                }
                Prompt::Password if pw_tries > 0 => {
                    // shell 模式：把特权口令当 sudo 口令喂一次。
                    output.push_str(&self.buf);
                    self.buf.clear();
                    pw_tries -= 1;
                    self.send_raw(&format!("{}\n", pw.clone().unwrap_or_default())).await;
                }
                Prompt::Password => {
                    output.push_str(&self.buf);
                    let reason = format!(
                        "命令要求输入密码，但本任务未提供（最后输出）：\n{}",
                        tail_lines(&self.buf, 6)
                    );
                    return TaskOutcome::failed(cmd, &output, reason);
                }
                _ => {
                    output.push_str(&self.buf);
                    break;
                }
            }
        }
        let rc = if probe_rc { extract_rc(&output) } else { None };
        TaskOutcome::judge(cmd, &output, used_pem, rc, self.kind == SessionKind::Shell)
    }
}

/// 从输出里取最后一个退出码探针的值。
fn extract_rc(output: &str) -> Option<i32> {
    let mut found = None;
    for line in output.lines() {
        if let Some(i) = line.rfind(RC_MARK) {
            let digits: String = line[i + RC_MARK.len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if let Ok(v) = digits.parse::<i32>() {
                found = Some(v);
            }
        }
    }
    found
}

/// 把退出码探针相关的行从展示输出里去掉（含 PTY 回显的命令行与探针结果行）。
fn strip_rc_probe(output: &str) -> String {
    output
        .lines()
        .filter(|l| !l.contains(RC_MARK))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 单条任务的执行结果（判定依据一并带出，便于诊断"为什么算失败"）。
#[derive(Debug, Clone, Serialize)]
pub struct TaskOutcome {
    pub success: bool,
    pub idempotent: bool,
    /// 判定依据/失败原因（人话）。
    pub reason: String,
    pub output: String,
}

impl TaskOutcome {
    fn failed(cmd: &str, output: &str, reason: String) -> Self {
        TaskOutcome {
            success: false,
            idempotent: false,
            reason,
            output: if output.is_empty() { format!("[命令] {cmd}") } else { output.to_string() },
        }
    }

    /// 结果判定：硬失败标志 / 幂等成功标志 / "交互未完成"守卫。
    ///
    /// 相比参考工具的 `^` 粗暴匹配，这里把"脱字符指示行"限定为**整行只有 ^ 与 ~** 才算失败，
    /// 避免命令里出现 `^`（正则、路径）时被误判。
    fn judge(cmd: &str, output: &str, used_pem: bool, rc: Option<i32>, shell: bool) -> Self {
        // 0) shell 模式且拿到了真实退出码：以它为准，比关键词启发式可靠得多。
        if let Some(code) = rc {
            let shown = strip_rc_probe(output);
            let low = shown.to_lowercase();
            let soft = [
                "already exists",
                "already configured",
                "no change",
                "unchanged",
                "isn't configured",
                "not configured",
            ];
            let idem = soft.iter().find(|k| low.contains(**k)).map(|k| (*k).to_string());
            return TaskOutcome {
                success: code == 0,
                idempotent: code == 0 && idem.is_some(),
                reason: if code == 0 {
                    match idem {
                        Some(k) => format!("退出码 0（输出含 `{k}`，视为幂等）"),
                        None => "退出码 0".to_string(),
                    }
                } else {
                    format!("退出码 {code}")
                },
                output: shown,
            };
        }

        let low = output.to_lowercase();

        // 1) 交互没收干净就绝不判成功（否则会出现"报成功但其实还在等输入"）。
        //    这条只对设备 CLI 有意义：shell 的完成信号就是提示符回落。
        let success_markers = [
            "import successful",
            "key import successful",
            "certificate import successful",
            "(config",
        ];
        let pending_markers = [
            "type yes",
            "to continue:",
            "continue:",
            "use \"...\" on a single line",
            "to terminate import",
            "enter key",
            "enter the trusted root ca certificate file",
        ];
        let has_success = success_markers.iter().any(|k| low.contains(k));
        if !has_success && !shell {
            if let Some(k) = pending_markers.iter().find(|k| low.contains(**k)) {
                let reason = format!("交互未完成：输出里仍有待输入提示 `{k}`");
                return TaskOutcome { success: false, idempotent: false, reason, output: output.to_string() };
            }
        }

        let is_clear = cmd.to_lowercase().starts_with("clear ssl host");
        if is_clear {
            if low.contains("isn't configured") || low.contains("not configured") {
                let reason = "目标本来就没配置，视为幂等成功".to_string();
                return TaskOutcome { success: true, idempotent: true, reason, output: output.to_string() };
            }
        }

        // 2) 硬失败标志。
        let hard = [
            "invalid input",
            "syntax error",
            "access denied",
            "failed to execute",
            "aborted",
            "unknown command",
            "command not found",
            "permission denied",
            "incomplete command",
        ];
        if let Some(k) = hard.iter().find(|k| low.contains(**k)) {
            let reason = format!("命中失败标志 `{k}`");
            return TaskOutcome { success: false, idempotent: false, reason, output: output.to_string() };
        }
        // 脱字符指示行（整行只有 ^ 和 ~）表示上一行命令有语法问题。
        if output.lines().any(|l| {
            let t = l.trim();
            !t.is_empty() && t.starts_with('^') && t.chars().all(|c| c == '^' || c == '~' || c == ' ')
        }) {
            let reason = "设备用 ^ 指出了命令行语法问题".to_string();
            return TaskOutcome { success: false, idempotent: false, reason, output: output.to_string() };
        }
        // 失败字样（"import failed" 之类），但排除 "0 failed"。
        if low.contains("failed") && !low.contains("0 failed") {
            let reason = "输出中出现 `failed`".to_string();
            return TaskOutcome { success: false, idempotent: false, reason, output: output.to_string() };
        }

        // 3) 幂等成功。
        let soft = ["already exists", "already configured", "duplicate", "no change", "unchanged", "isn't configured", "not configured"];
        if let Some(k) = soft.iter().find(|k| low.contains(**k)) {
            let reason = format!("设备反馈 `{k}`，视为幂等成功");
            return TaskOutcome { success: true, idempotent: true, reason, output: output.to_string() };
        }

        let reason = if used_pem { "命令与 PEM 导入均已完成".to_string() } else { "命令已执行完成".to_string() };
        TaskOutcome { success: true, idempotent: false, reason, output: output.to_string() }
    }
}

// ============================ 运行编排 ============================

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOpts {
    /// PEM 相对路径的基准目录（导入 config.yaml 时取其所在目录）。
    #[serde(default)]
    pub base_dir: Option<String>,
    /// 覆盖配置里的 continue_on_error。
    #[serde(default)]
    pub continue_on_error: Option<bool>,
    /// 会话类型：`auto`（默认，自动探测）/ `device`（网络设备 CLI）/ `shell`（Linux 编译机 shell）。
    #[serde(default)]
    pub mode: Option<String>,
    /// shell 模式是否追加退出码探针（默认 true，判定更准）。
    #[serde(default)]
    pub probe_rc: Option<bool>,
}

impl RunOpts {
    fn session_kind(&self) -> SessionKind {
        match self.mode.as_deref().unwrap_or("auto") {
            "device" => SessionKind::Device,
            "shell" | "linux" => SessionKind::Shell,
            _ => SessionKind::Auto,
        }
    }
    fn probe_rc(&self) -> bool {
        self.probe_rc.unwrap_or(true)
    }
}

struct DispatchRun {
    cancel: Arc<AtomicBool>,
}

type RunMap = HashMap<String, DispatchRun>;
static RUNS: OnceLock<Mutex<RunMap>> = OnceLock::new();
fn runs() -> &'static Mutex<RunMap> {
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize)]
pub struct RunStarted {
    pub run_id: String,
    pub plan: Plan,
}

/// 解析这次运行要用的连接凭据：优先库内 SSH 档案，否则用配置里 `global` 段。
async fn open_device(
    app: &AppHandle,
    profile_id: Option<&str>,
    cfg: &DispatchConfig,
    kind: SessionKind,
    probe_rc: bool,
) -> Result<(DeviceCli, String), String> {
    let (handle, enable_pw) = if let Some(pid) = profile_id.filter(|s| !s.is_empty()) {
        let h = connect_authenticated(app, pid).await?;
        (h, enable_secret(pid))
    } else {
        let ip = cfg
            .global
            .ip
            .clone()
            .ok_or("未选择 SSH 档案，且配置里 global.ip 为空：无法确定目标设备")?;
        let user = cfg.global.username.clone().unwrap_or_default();
        let pw = cfg.global.password.clone();
        let h = connect_explicit(&ip, cfg.global.port.unwrap_or(22), &user, "password", pw, None).await?;
        (h, cfg.global.enable_password.clone())
    };
    let label = profile_id
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| cfg.global.ip.clone().unwrap_or_default());
    let cli = DeviceCli::open(handle, enable_pw, kind, probe_rc).await?;
    Ok((cli, label))
}

/// 解析任务里的 `pem_file`：绝对路径直接用，相对路径按 `base_dir` 拼。
fn read_pem(path: &str, base_dir: Option<&str>) -> Result<String, String> {
    if path.trim().is_empty() {
        return Ok(String::new());
    }
    let p = std::path::Path::new(path);
    let full = if p.is_absolute() {
        p.to_path_buf()
    } else if let Some(b) = base_dir.filter(|s| !s.trim().is_empty()) {
        std::path::Path::new(b).join(p)
    } else {
        p.to_path_buf()
    };
    std::fs::read_to_string(&full).map_err(|e| format!("读取 PEM 失败 {}: {e}", full.display()))
}

/// 日志落盘：每次运行一个 JSONL 文件，字段对齐参考工具（cmd/success/idempotent/pem_file/output/ts）。
fn run_log_path(app: &AppHandle, run_id: &str) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("cmd_dispatch")
        .join("runs");
    std::fs::create_dir_all(&dir).map_err(|e| format!("建目录失败: {e}"))?;
    Ok(dir.join(format!("{run_id}.jsonl")))
}

/// 启动一次下发：先算好计划返回给 UI，再后台逐条执行并推事件。
#[command]
pub async fn cmd_dispatch_run(
    app: AppHandle,
    run_id: String,
    profile_id: Option<String>,
    config_yaml: String,
    opts: Option<RunOpts>,
) -> Result<RunStarted, String> {
    let opts = opts.unwrap_or_default();
    let cfg = parse_config(&config_yaml)?;
    let plan = build_plan(&cfg)?;
    let continue_on_error = opts.continue_on_error.unwrap_or(cfg.continue_on_error);

    let cancel = Arc::new(AtomicBool::new(false));
    runs()
        .lock()
        .map_err(|_| "运行表锁失败".to_string())?
        .insert(run_id.clone(), DispatchRun { cancel: cancel.clone() });

    let log_path = run_log_path(&app, &run_id).ok();
    let kind = opts.session_kind();
    let probe_rc = opts.probe_rc();
    let steps = plan.steps.clone();
    let base_dir = opts.base_dir.clone();
    let rid = run_id.clone();
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let emit = |v: serde_json::Value| {
            let _ = app2.emit("cmd-dispatch-task", v);
        };
        // 连接设备（失败要立刻告诉用户，而不是让界面一直等）。
        let (mut cli, target) =
            match open_device(&app2, profile_id.as_deref(), &cfg, kind, probe_rc).await {
            Ok(v) => v,
            Err(e) => {
                let _ = app2.emit(
                    "cmd-dispatch-done",
                    json!({ "runId": rid, "ok": false, "error": e, "total": steps.len(), "done": 0, "failed": 0 }),
                );
                runs().lock().map(|mut m| m.remove(&rid)).ok();
                return;
            }
        };
        let _ = app2.emit("cmd-dispatch-log", json!({ "runId": rid, "text": format!("已连接设备 {target}\n") }));

        let mut failed = 0usize;
        let mut done = 0usize;
        let mut log = log_path.map(|p| std::fs::OpenOptions::new().create(true).append(true).open(p).ok()).flatten();

        for (i, t) in steps.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                emit(json!({ "runId": rid, "taskKey": t.task_key, "index": i, "status": "skipped",
                             "reason": "用户取消", "cmd": t.cmd }));
                break;
            }
            emit(json!({ "runId": rid, "taskKey": t.task_key, "index": i, "status": "running",
                         "id": t.id, "cmd": t.cmd, "hostname": t.hostname, "domain": t.domain,
                         "indexInTask": t.index, "description": t.description }));

            let pem = match read_pem(&t.pem_file, base_dir.as_deref()) {
                Ok(p) => p,
                Err(e) => {
                    failed += 1;
                    emit(json!({ "runId": rid, "taskKey": t.task_key, "index": i, "status": "fail",
                                 "reason": e, "cmd": t.cmd }));
                    if !continue_on_error { break }
                    continue;
                }
            };
            let outcome = cli.run_cmd(&t.cmd, if pem.is_empty() { None } else { Some(&pem) }).await;
            done += 1;
            if !outcome.success {
                failed += 1;
            }
            let status = if !outcome.success { "fail" } else if outcome.idempotent { "ok-idem" } else { "ok" };
            emit(json!({
                "runId": rid, "taskKey": t.task_key, "index": i, "status": status,
                "cmd": t.cmd, "reason": outcome.reason, "output": outcome.output,
                "idempotent": outcome.idempotent, "hostname": t.hostname, "domain": t.domain,
            }));
            if let Some(f) = log.as_mut() {
                use std::io::Write;
                let _ = writeln!(f, "{}", json!({
                    "task_key": t.task_key, "cmd": t.cmd, "pem_file": t.pem_file,
                    "success": outcome.success, "idempotent": outcome.idempotent,
                    "reason": outcome.reason, "output": outcome.output,
                    "ts": chrono::Local::now().to_rfc3339(),
                }));
            }
            if !outcome.success && !continue_on_error {
                break;
            }
        }

        let cancelled = cancel.load(Ordering::SeqCst);
        let _ = app2.emit(
            "cmd-dispatch-done",
            json!({ "runId": rid, "ok": failed == 0 && !cancelled, "total": steps.len(),
                    "done": done, "failed": failed, "cancelled": cancelled }),
        );
        runs().lock().map(|mut m| m.remove(&rid)).ok();
    });

    Ok(RunStarted { run_id, plan })
}

/// 取消一次运行：置标志（正在进行的命令会在下一次检查点退出）。
#[command]
pub fn cmd_dispatch_cancel(run_id: String) -> Result<(), String> {
    if let Ok(m) = runs().lock() {
        if let Some(r) = m.get(&run_id) {
            r.cancel.store(true, Ordering::SeqCst);
        }
    }
    Ok(())
}

// ============================ 策略库（把一组命令序列存下来复用） ============================

/// 一条命名策略：targets + tasks（**不保存凭据**，global 一律清空）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created: String,
    pub config: DispatchConfig,
}

#[derive(Serialize)]
pub struct StrategySummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created: String,
    pub task_count: usize,
    pub hostnames: Vec<String>,
    pub domains: Vec<String>,
}

fn strategies_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("cmd_dispatch");
    std::fs::create_dir_all(&dir).map_err(|e| format!("建目录失败: {e}"))?;
    Ok(dir.join("strategies.json"))
}

fn read_strategies(app: &AppHandle) -> Result<Vec<Strategy>, String> {
    let p = strategies_path(app)?;
    if !p.exists() {
        return Ok(Vec::new());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| format!("读取策略库失败: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("解析策略库失败: {e}"))
}

fn write_strategies(app: &AppHandle, v: &[Strategy]) -> Result<(), String> {
    let p = strategies_path(app)?;
    std::fs::write(&p, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写策略库失败: {e}"))
}

fn strategy_summary(s: &Strategy) -> StrategySummary {
    StrategySummary {
        id: s.id.clone(),
        name: s.name.clone(),
        description: s.description.clone(),
        created: s.created.clone(),
        task_count: s.config.tasks.len(),
        hostnames: s.config.targets.hostnames.clone(),
        domains: s.config.targets.domains.clone(),
    }
}

#[command]
pub fn cmd_strategy_list(app: AppHandle) -> Result<Vec<StrategySummary>, String> {
    Ok(read_strategies(&app)?.iter().map(strategy_summary).collect())
}

/// 保存策略。`id` 为空=新建，否则覆盖同名 id。
#[command]
pub fn cmd_strategy_save(
    app: AppHandle,
    id: Option<String>,
    name: String,
    description: Option<String>,
    config_yaml: String,
) -> Result<StrategySummary, String> {
    if name.trim().is_empty() {
        return Err("策略名不能为空".into());
    }
    let mut cfg = parse_config(&config_yaml)?;
    // 凭据不进策略库：策略是可分享/可复用的"命令序列"，不该夹带密码。
    cfg.global = GlobalCfg::default();
    let mut all = read_strategies(&app)?;
    let sid = id.filter(|s| !s.is_empty()).unwrap_or_else(|| Uuid::new_v4().to_string());
    let entry = match all.iter_mut().find(|s| s.id == sid) {
        Some(existing) => {
            existing.name = name;
            existing.description = description;
            existing.config = cfg;
            existing.clone()
        }
        None => {
            let e = Strategy {
                id: sid,
                name,
                description,
                created: chrono::Local::now().to_rfc3339(),
                config: cfg,
            };
            all.push(e.clone());
            e
        }
    };
    write_strategies(&app, &all)?;
    Ok(strategy_summary(&entry))
}

/// 载入策略：返回 config YAML（直接喂给任务表/预览/运行）。
#[command]
pub fn cmd_strategy_load(app: AppHandle, id: String) -> Result<String, String> {
    let all = read_strategies(&app)?;
    let s = all.iter().find(|s| s.id == id).ok_or("策略不存在")?;
    serde_yaml::to_string(&s.config).map_err(|e| format!("生成 YAML 失败: {e}"))
}

#[command]
pub fn cmd_strategy_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut all = read_strategies(&app)?;
    all.retain(|s| s.id != id);
    write_strategies(&app, &all)
}

/// 把一段 PEM（来自 CA 库 / 终端证书库 / 任意来源）落到受管目录并返回绝对路径。
///
/// 任务表的 `pem_file` 需要的是**文件路径**，而从证书库取出来的是内容；
/// 这里统一落到 `app_data/cmd_dispatch/pem/` 下，路径稳定、可重复引用、可打进策略里。
#[command]
pub fn cmd_dispatch_stage_pem(app: AppHandle, name: String, content: String) -> Result<String, String> {
    if content.trim().is_empty() {
        return Err("PEM 内容为空".into());
    }
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("cmd_dispatch")
        .join("pem");
    std::fs::create_dir_all(&dir).map_err(|e| format!("建目录失败: {e}"))?;
    // 文件名：安全化 + 用一个短哈希防同名覆盖（同一内容复用同一个文件）。
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(content.as_bytes());
    let tag: String = h.iter().take(4).map(|b| format!("{b:02x}")).collect();
    let path = dir.join(format!("{safe}-{tag}.pem"));
    if !path.exists() {
        std::fs::write(&path, &content).map_err(|e| format!("写入 PEM 失败: {e}"))?;
    }
    Ok(path.to_string_lossy().to_string())
}

// ============================ 测试（纯逻辑，可离线跑） ============================
#[cfg(test)]
mod tests {
    use super::*;

    /// 参考工具 config.yaml 的精简等价版，用于验证展开与排序。
    const CFG: &str = r#"
targets:
  hostnames: [vhost, rhost]
  domains: ["", "www.xian.com"]
tasks:
- id: clear
  cmd: "clear ssl host {hostname}"
- id: create
  cmd: "ssl host virtual {hostname}"
  depends_on: [clear]
- id: sni
  cmd: "ssl sni {hostname} {domain}"
  only_if_domain: true
  depends_on: [create]
- id: import_key_host
  cmd: "ssl import key {hostname} {index}"
  pem_file: "./cfg/{hostname}_{index}.key"
  index: 1
  only_if_no_domain: true
  depends_on: [create]
- id: import_cert_host
  cmd: "ssl import certificate {hostname} {index}"
  index: 1
  only_if_no_domain: true
  depends_on: [import_key_host]
- id: activate_host
  cmd: "ssl activate certificate {hostname} {index} \"\" all"
  index: 1
  only_if_no_domain: true
  depends_on: [import_cert_host]
"#;

    #[test]
    fn expand_follows_reference_rules() {
        let cfg = parse_config(CFG).unwrap();
        let tasks = expand(&cfg);
        // clear/create/import_key/import_cert/activate 各 2（host 级），sni 2（域名级）
        assert_eq!(tasks.len(), 12, "host 级任务应各展开 2 份、sni 展开 2 份");
        let sni: Vec<_> = tasks.iter().filter(|t| t.id == "sni").collect();
        assert_eq!(sni.len(), 2);
        assert!(sni.iter().all(|t| t.domain == "www.xian.com"), "SNI 只应在有域名时展开");
        let host_only: Vec<_> = tasks.iter().filter(|t| t.id == "clear").collect();
        assert!(host_only.iter().all(|t| t.domain.is_empty()), "无 {{domain}} 的任务只跑 host 级");
    }

    #[test]
    fn render_substitutes_placeholders_and_keeps_unknown() {
        let cfg = parse_config(CFG).unwrap();
        let tasks = expand(&cfg);
        let t = tasks
            .iter()
            .find(|t| t.id == "import_key_host" && t.hostname == "vhost")
            .unwrap();
        assert_eq!(t.cmd, "ssl import key vhost 1");
        // pem_file 模板也要渲染
        assert_eq!(t.pem_file, "./cfg/vhost_1.key");
        // 未知占位符原样保留（命令里可能有 { } ）
        let dummy = ConcreteTask {
            task_key: "k".into(),
            id: "x".into(),
            cmd: String::new(),
            pem_file: String::new(),
            hostname: "h".into(),
            domain: "d".into(),
            index: "1".into(),
            depends_on: vec![],
            description: None,
        };
        assert_eq!(render("a {unknown} b {hostname}", &dummy), "a {unknown} b h");
        assert_eq!(render("json { \"a\": 1 }", &dummy), "json { \"a\": 1 }");
    }

    #[test]
    fn plan_orders_dependencies() {
        let cfg = parse_config(CFG).unwrap();
        let plan = build_plan(&cfg).unwrap();
        assert_eq!(plan.task_count, 12);
        assert!(plan.warnings.is_empty(), "不该有歧义警告: {:?}", plan.warnings);
        let pos = |pred: &dyn Fn(&ConcreteTask) -> bool| -> usize {
            plan.steps.iter().position(|t| pred(t)).expect("step missing")
        };
        let clear = pos(&|t| t.id == "clear" && t.hostname == "vhost");
        let create = pos(&|t| t.id == "create" && t.hostname == "vhost");
        let key = pos(&|t| t.id == "import_key_host" && t.hostname == "vhost");
        let cert = pos(&|t| t.id == "import_cert_host" && t.hostname == "vhost");
        let act = pos(&|t| t.id == "activate_host" && t.hostname == "vhost");
        assert!(clear < create && create < key && key < cert && cert < act, "拓扑顺序不对");
    }

    #[test]
    fn dependency_prefers_same_index_among_duplicates() {
        // 原工具里同 id 会有 index 1/2/3 多条；activate(index=2) 必须绑到 import(index=2)。
        let yaml = r#"
targets: { hostnames: [vhost], domains: [""] }
tasks:
- id: create
  cmd: "ssl host virtual {hostname}"
- id: import_key
  cmd: "ssl import key {hostname} {index}"
  index: 1
  depends_on: [create]
- id: import_key
  cmd: "ssl import key {hostname} {index}"
  index: 2
  depends_on: [create]
- id: activate
  cmd: "ssl activate certificate {hostname} {index} \"\" all"
  index: 2
  depends_on: [import_key]
"#;
        let cfg = parse_config(yaml).unwrap();
        let plan = build_plan(&cfg).unwrap();
        let act = plan.steps.iter().find(|t| t.id == "activate").unwrap();
        let key2 = plan.steps.iter().find(|t| t.id == "import_key" && t.index == "2").unwrap();
        let key1 = plan.steps.iter().find(|t| t.id == "import_key" && t.index == "1").unwrap();
        let at = |k: &str| plan.steps.iter().position(|t| t.task_key == k).unwrap();
        assert!(at(&key2.task_key) < at(&act.task_key), "activate 应排在 index=2 的 key 之后");
        assert!(at(&key1.task_key) < at(&act.task_key));
    }

    #[test]
    fn missing_dependency_reports_context() {
        let yaml = r#"
targets: { hostnames: [vhost], domains: [""] }
tasks:
- id: a
  cmd: "x {hostname}"
  depends_on: [nonexistent]
"#;
        let err = build_plan(&parse_config(yaml).unwrap()).unwrap_err();
        assert!(err.contains("nonexistent"), "报错应指出缺失的依赖: {err}");
        assert!(err.contains("a"), "报错应指出是哪个任务: {err}");
    }

    #[test]
    fn cycle_reports_nodes() {
        let yaml = r#"
targets: { hostnames: [vhost], domains: [""] }
tasks:
- id: a
  cmd: "a {hostname}"
  depends_on: [b]
- id: b
  cmd: "b {hostname}"
  depends_on: [a]
"#;
        let err = build_plan(&parse_config(yaml).unwrap()).unwrap_err();
        assert!(err.contains("环"), "{err}");
        assert!(err.contains('a') && err.contains('b'), "应列出环上的节点: {err}");
    }

    #[test]
    fn index_accepts_number_and_string() {
        let yaml = r#"
targets: { hostnames: [h], domains: [""] }
tasks:
- id: a
  cmd: "a {hostname} {index}"
  index: 1
- id: b
  cmd: "b {hostname} {index}"
  index: "2"
"#;
        let cfg = parse_config(yaml).unwrap();
        assert_eq!(cfg.tasks[0].index.as_deref(), Some("1"));
        assert_eq!(cfg.tasks[1].index.as_deref(), Some("2"));
    }

    #[test]
    fn parse_error_is_readable() {
        let err = parse_config("targets: [oops").unwrap_err();
        assert!(err.contains("配置解析失败"), "{err}");
    }
}
