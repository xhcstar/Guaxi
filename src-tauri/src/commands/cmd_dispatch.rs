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
    User,
    Priv,
    Config,
    Password,
    PemWait,
    Unknown,
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
    if line.ends_with('#') {
        return if line.contains("(config") { Prompt::Config } else { Prompt::Priv };
    }
    if line.ends_with('>') {
        return Prompt::User;
    }
    Prompt::Unknown
}

/// 把可能很长的设备输出裁成"尾部若干行"，用于错误提示。
fn tail_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

/// 交互式设备 CLI 会话。
struct DeviceCli {
    channel: russh::Channel<russh::client::Msg>,
    buf: String,
    /// 我们已读过的输出累计（用于日志/诊断），不受 buf 清空影响。
    transcript: String,
    /// enable 口令：命令中途若掉出配置模式，重新进入时还要用。
    enable_password: Option<String>,
}

impl DeviceCli {
    async fn open(
        handle: russh::client::Handle<ClientHandler>,
        enable_password: Option<String>,
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

        let mut cli = DeviceCli { channel, buf: String::new(), transcript: String::new(), enable_password };
        let pw = cli.enable_password.clone();
        cli.ensure_config_mode(pw).await?;
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
    async fn answer_interaction(&mut self) -> bool {
        let low = self.buf.to_lowercase();
        if low.contains("--more--") {
            self.buf.clear();
            self.send_raw(" ").await;
            return true;
        }
        let confirm = ["type yes to continue", "yes/no", "continue:", "are you sure"];
        if confirm.iter().any(|k| low.contains(k)) {
            self.buf.clear();
            self.send_raw("YES\n").await;
            return true;
        }
        false
    }

    /// 等一个稳定提示符；期间自动应答交互。
    async fn wait_prompt(&mut self, overall: Duration) -> Result<Prompt, String> {
        let deadline = Instant::now() + overall;
        loop {
            if self.answer_interaction().await {
                // 已应答，继续等
            } else if let p @ (Prompt::User | Prompt::Priv | Prompt::Config | Prompt::Password | Prompt::PemWait) =
                classify_prompt(&self.buf)
            {
                return Ok(p);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "等待设备提示符超时（最后输出）：\n{}",
                    tail_lines(&self.buf, 8)
                ));
            }
            let remain = deadline.saturating_duration_since(Instant::now());
            self.pump(QUIET.min(remain)).await?;
        }
    }

    /// 确保处于配置模式 `(config)#`。
    async fn ensure_config_mode(&mut self, enable_password: Option<String>) -> Result<(), String> {
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

    /// 执行一条命令；`pem` 非空时按 PEM 粘贴流程补送。
    async fn run_cmd(&mut self, cmd: &str, pem: Option<&str>) -> TaskOutcome {
        // 每条命令前都确保在配置模式（上一条可能把模式改了）。
        let pw = self.enable_password.clone();
        if let Err(e) = self.ensure_config_mode(pw).await {
            let out = self.buf.clone();
            return TaskOutcome::failed(cmd, &out, e);
        }
        self.buf.clear();
        self.send_raw(&format!("{cmd}\n")).await;

        let mut allow_pem = pem.is_some();
        let mut used_pem = false;
        let mut output = String::new();

        // 最多两轮：第一轮可能撞上 PEM 粘贴状态，补送 PEM 后再等一次。
        for _ in 0..2 {
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
                    // PEM 正文 + 独立一行 `...` 结束输入。
                    let body = pem.unwrap_or("");
                    let mut payload = String::with_capacity(body.len() + 8);
                    payload.push_str(body);
                    if !payload.ends_with('\n') {
                        payload.push('\n');
                    }
                    payload.push_str("...\n");
                    self.send_raw(&payload).await;
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
        TaskOutcome::judge(cmd, &output, used_pem)
    }
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
    fn judge(cmd: &str, output: &str, used_pem: bool) -> Self {
        let low = output.to_lowercase();

        // 1) 交互没收干净就绝不判成功（否则会出现"报成功但其实还在等输入"）。
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
        if !has_success {
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
    let cli = DeviceCli::open(handle, enable_pw).await?;
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
    let steps = plan.steps.clone();
    let base_dir = opts.base_dir.clone();
    let rid = run_id.clone();
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let emit = |v: serde_json::Value| {
            let _ = app2.emit("cmd-dispatch-task", v);
        };
        // 连接设备（失败要立刻告诉用户，而不是让界面一直等）。
        let (mut cli, target) = match open_device(&app2, profile_id.as_deref(), &cfg).await {
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
