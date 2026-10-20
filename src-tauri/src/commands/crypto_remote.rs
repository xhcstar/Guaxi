//! 远程 Tongsuo/OpenSSL 证书工作流：SM2 / PQC 的密钥、CA、签发与验签。
//!
//! 本机不要求安装 Tongsuo：脚本会在用户指定的远端工具上执行，产物以 PEM 返回。
//! KeyUsage 按团队约定：
//! - CA：`keyUsage=keyCertSign,cRLSign` + `basicConstraints=critical,CA:TRUE`
//! - 签名证书：`keyUsage=digitalSignature`
//! - 加密证书：`keyUsage=keyEncipherment`
//! - 叶证书统一带 `extendedKeyUsage=serverAuth,clientAuth`（服务端/客户端模式可覆盖）
//!
//! 支持算法：sm2、falcon512、falcon1024、dilithium2、dilithium3、dilithium5。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};

use super::remote_tool::{run_remote_command, shell_quote};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCryptoOpts {
    #[serde(default)]
    pub tool_path: Option<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub env: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertSubject {
    #[serde(default)]
    pub c: Option<String>,
    #[serde(default)]
    pub st: Option<String>,
    #[serde(default)]
    pub l: Option<String>,
    #[serde(default)]
    pub o: Option<String>,
    #[serde(default)]
    pub ou: Option<String>,
    #[serde(default)]
    pub cn: String,
    /// 逗号分隔的 DNS / IP（空则不写 SAN）。
    #[serde(default)]
    pub sans: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub street: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub given_name: Option<String>,
    #[serde(default)]
    pub surname: Option<String>,
    #[serde(default)]
    pub initials: Option<String>,
    #[serde(default)]
    pub pseudonym: Option<String>,
    #[serde(default)]
    pub generation_qualifier: Option<String>,
    #[serde(default)]
    pub dn_qualifier: Option<String>,
    #[serde(default)]
    pub organization_identifier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCertMaterial {
    pub cert_pem: String,
    pub key_pem: String,
    pub csr_pem: String,
    pub chain_pem: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteKeyPair {
    pub key_pem: String,
    pub pub_pem: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSm2Dual {
    pub sign_cert_pem: String,
    pub sign_key_pem: String,
    pub enc_cert_pem: String,
    pub enc_key_pem: String,
    pub certs_pem: String,
    pub keys_pem: String,
    pub sign_chain_pem: String,
    pub enc_chain_pem: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteVerifyResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSignResult {
    pub signature_b64: String,
    pub output: String,
}

fn key_algo(alg: &str) -> Result<&'static str, String> {
    match alg
        .trim()
        .to_ascii_lowercase()
        .replace('-', "")
        .replace('_', "")
        .replace(' ', "")
        .as_str()
    {
        "sm2" => Ok("sm2"),
        "falcon512" => Ok("falcon512"),
        "falcon1024" => Ok("falcon1024"),
        "dilithium2" | "mldsa44" => Ok("dilithium2"),
        "dilithium3" | "mldsa65" => Ok("dilithium3"),
        "dilithium5" | "mldsa87" => Ok("dilithium5"),
        other => Err(format!(
            "不支持的算法: {other}（支持 sm2 / falcon512 / falcon1024 / dilithium2(ML-DSA-44) / dilithium3(ML-DSA-65) / dilithium5(ML-DSA-87)）"
        )),
    }
}

/// Tongsuo 8.5 起 NIST 名称优先（ML-DSA-44/65/87），旧版本仍可能用 Dilithium 名称。
fn algo_candidates(logical: &str) -> &'static [&'static str] {
    match logical {
        "sm2" => &["sm2", "SM2"],
        "falcon512" => &["falcon512", "falcon-512", "Falcon-512"],
        "falcon1024" => &["falcon1024", "falcon-1024", "Falcon-1024"],
        "dilithium2" => &["ML-DSA-44", "ml-dsa-44", "dilithium2", "Dilithium2"],
        "dilithium3" => &["ML-DSA-65", "ml-dsa-65", "dilithium3", "Dilithium3"],
        "dilithium5" => &["ML-DSA-87", "ml-dsa-87", "dilithium5", "Dilithium5"],
        _ => &[],
    }
}

static ALGO_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
fn algo_cache() -> &'static Mutex<HashMap<String, String>> {
    ALGO_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 查询远端工具实际注册的算法名并映射到可用名称。
///
/// 这一步解决 Tongsuo 8.5 把 Dilithium 改名为 ML-DSA-44/65/87 的兼容问题；
/// 查不到列表时退回首选名称，让 genpkey 自己给出更具体的错误。
async fn resolve_algorithm(
    app: &AppHandle,
    profile_id: &str,
    opts: &RemoteCryptoOpts,
    logical: &str,
) -> Result<String, String> {
    let tool = require_tool(opts)?;
    let env = opts.env.as_deref().unwrap_or("");
    let cache_key = format!("{profile_id}\u{1}{tool}\u{1}{env}");
    if let Ok(c) = algo_cache().lock() {
        if let Some(v) = c.get(&cache_key) {
            return Ok(v.clone());
        }
    }
    let list = run_remote_command(
        app,
        profile_id,
        "{tool} list -public-key-algorithms 2>&1",
        &tool,
        "",
        env,
        60,
    )
    .await
    .map(|r| r.output)
    .unwrap_or_default();
    if list.trim().is_empty() {
        // 列表不可用时不要武断报“不支持”，让后续 genpkey 给出真实错误。
        return Ok(algo_candidates(logical).first().copied().unwrap_or(logical).to_string());
    }
    let low = list.to_lowercase();
    if let Some(found) = algo_candidates(logical)
        .iter()
        .find(|c| low.contains(&c.to_lowercase()))
    {
        if let Ok(mut c) = algo_cache().lock() {
            c.insert(cache_key, (*found).to_string());
        }
        return Ok((*found).to_string());
    }
    if logical == "sm2" {
        return Ok("sm2".to_string());
    }
    let mut available: Vec<String> = Vec::new();
    for line in list.lines() {
        let t = line.trim();
        if let Some(name) = t.strip_prefix("Name:") {
            let name = name.trim();
            if name.to_lowercase().contains("ml-dsa")
                || name.to_lowercase().contains("dilithium")
                || name.to_lowercase().contains("falcon")
            {
                if !available.iter().any(|x| x.as_str() == name) {
                    available.push(name.to_string());
                }
            }
        }
    }
    let hint = if available.is_empty() {
        "远端算法列表里没有找到可用的 PQC 签名算法".to_string()
    } else {
        format!("可用算法：{}", available.join("；"))
    };
    Err(format!(
        "远端 Tongsuo 未提供 {logical}。{hint}。可点「检测算法」查看完整列表"
    ))
}

fn is_sm2(alg: &str) -> bool {
    alg == "sm2"
}

fn clean_subject_value(v: &str) -> Result<String, String> {
    if v.contains('\n') || v.contains('\0') {
        return Err("主题字段不能包含换行".into());
    }
    Ok(v.replace('\\', "\\\\").replace('/', "\\/"))
}

fn build_subject(s: &CertSubject) -> Result<String, String> {
    let cn = s.cn.trim();
    if cn.is_empty() {
        return Err("CN 不能为空".into());
    }
    let mut out = String::new();
    for (key, val) in [
        ("C", s.c.as_deref()),
        ("ST", s.st.as_deref()),
        ("L", s.l.as_deref()),
        ("O", s.o.as_deref()),
        ("OU", s.ou.as_deref()),
        ("emailAddress", s.email.as_deref()),
        ("serialNumber", s.serial_number.as_deref()),
        ("street", s.street.as_deref()),
        ("postalCode", s.postal_code.as_deref()),
        ("title", s.title.as_deref()),
        ("GN", s.given_name.as_deref()),
        ("SN", s.surname.as_deref()),
        ("initials", s.initials.as_deref()),
        ("pseudonym", s.pseudonym.as_deref()),
        ("generationQualifier", s.generation_qualifier.as_deref()),
        ("dnQualifier", s.dn_qualifier.as_deref()),
        ("organizationIdentifier", s.organization_identifier.as_deref()),
    ] {
        if let Some(v) = val.map(str::trim).filter(|v| !v.is_empty()) {
            out.push_str(&format!("/{key}={}", clean_subject_value(v)?));
        }
    }
    out.push_str(&format!("/CN={}", clean_subject_value(cn)?));
    Ok(out)
}

fn san_list(sans: Option<&str>) -> Result<Option<String>, String> {
    let Some(s) = sans.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if s.contains('\n') || s.contains('\0') {
        return Err("SAN 不能包含换行".into());
    }
    let mut parts = Vec::new();
    for raw in s.split(',') {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        if t.parse::<std::net::IpAddr>().is_ok() {
            parts.push(format!("IP:{t}"));
        } else {
            parts.push(format!("DNS:{t}"));
        }
    }
    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join(",")))
    }
}

fn ca_ext() -> &'static str {
    // 与团队 gencert_sm2.sh 保持一致。
    "keyUsage=keyCertSign,cRLSign\n\
     basicConstraints=critical,CA:TRUE\n"
}

/// usage: sign（digitalSignature）/ enc（keyEncipherment）/ server / client。
fn leaf_ext(usage: &str, sans: Option<&str>) -> Result<String, String> {
    let (ku, eku) = match usage {
        "sign" => ("digitalSignature", "serverAuth,clientAuth"),
        "enc" => ("keyEncipherment", "serverAuth,clientAuth"),
        "server" => ("digitalSignature,keyEncipherment", "serverAuth"),
        "client" => ("digitalSignature,keyEncipherment", "clientAuth"),
        other => return Err(format!("未知用途: {other}（sign|enc|server|client）")),
    };
    let mut s = format!("keyUsage={ku}\nextendedKeyUsage={eku}\n");
    if let Some(san) = san_list(sans)? {
        s.push_str(&format!("subjectAltName={san}\n"));
    }
    Ok(s)
}

fn ensure_pem(content: &str, field: &str) -> Result<(), String> {
    if !content.contains("-----BEGIN ") {
        return Err(format!("{field} 不是 PEM 内容"));
    }
    if content.contains("GUAXI_PEM_EOF") {
        return Err(format!("{field} 含有保留分隔符"));
    }
    Ok(())
}

fn heredoc(file: &str, content: &str) -> String {
    format!(
        "cat > \"$D/{file}\" <<'GUAXI_PEM_EOF'\n{}\nGUAXI_PEM_EOF\n",
        content.trim()
    )
}

fn emit(file: &str) -> String {
    format!("printf '\\n===GUAXI:{file}===\\n'; cat \"$D/{file}\"; printf '\\n===GUAXI:END===\\n'\n")
}

fn artifact(out: &str, name: &str) -> Option<String> {
    let tag = format!("===GUAXI:{name}===");
    let start = out.find(&tag)? + tag.len();
    let rest = &out[start..];
    let end = rest.find("===GUAXI:END===")?;
    Some(rest[..end].trim().to_string()).filter(|s| !s.is_empty())
}

/// 没有退出码时以产物是否齐全为准；有退出码时严格看 0。
fn ensure_ok(r: &super::remote_tool::RemoteToolResult, what: &str) -> Result<(), String> {
    if let Some(code) = r.exit_code {
        if code != 0 {
            return Err(format!(
                "{what}失败（退出码 {code}）：{}",
                r.output.trim()
            ));
        }
    }
    Ok(())
}

fn script_prelude(opts: &RemoteCryptoOpts) -> String {
    let mut s = String::from("set -e\n");
    if let Some(env) = opts.env.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // 环境变量来自用户自己的输入；不允许换行，避免破坏脚本结构。
        s.push_str(&format!("export {}\n", env.replace('\n', " ").replace('\0', "")));
    }
    if let Some(wd) = opts.workdir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        s.push_str(&format!("cd {} || exit 1\n", shell_quote(wd)));
    }
    s.push_str("D=$(mktemp -d /tmp/guaxi-crypto.XXXXXX)\ntrap 'rm -rf \"$D\"' EXIT\ncd \"$D\"\n");
    s
}

fn require_tool(opts: &RemoteCryptoOpts) -> Result<String, String> {
    opts.tool_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "需要远端工具路径（Tongsuo/openssl）".to_string())
}

async fn run_script(
    app: &AppHandle,
    profile_id: &str,
    opts: &RemoteCryptoOpts,
    script: String,
    timeout: u64,
) -> Result<super::remote_tool::RemoteToolResult, String> {
    let tool = require_tool(opts)?;
    run_remote_command(app, profile_id, &script, &tool, "", "", timeout).await
}

fn decode_b64(s: &str, field: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD
        .decode(cleaned)
        .map_err(|e| format!("{field} 不是有效的 base64: {e}"))
}

/// 生成密钥对（SM2 / PQC）。
#[command]
pub async fn crypto_remote_gen_key(
    app: AppHandle,
    profile_id: String,
    algorithm: String,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteKeyPair, String> {
    let logical = key_algo(&algorithm)?;
    let o = opts.unwrap_or_default();
    let alg = resolve_algorithm(&app, &profile_id, &o, logical).await?;
    let mut s = script_prelude(&o);
    s.push_str(&format!(
        "{{tool}} genpkey -algorithm {alg} -out key.pem\n{{tool}} pkey -in key.pem -pubout -out pub.pem\n"
    ));
    s.push_str(&emit("key.pem"));
    s.push_str(&emit("pub.pem"));
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(180)).await?;
    ensure_ok(&r, "生成密钥")?;
    Ok(RemoteKeyPair {
        key_pem: artifact(&r.output, "key.pem").ok_or("远端没有返回私钥")?,
        pub_pem: artifact(&r.output, "pub.pem").ok_or("远端没有返回公钥")?,
        output: r.output,
    })
}

/// 生成自签 CA；提供上级 CA 时生成由它签发的中间 CA。
#[command]
pub async fn crypto_remote_create_ca(
    app: AppHandle,
    profile_id: String,
    algorithm: String,
    subject: CertSubject,
    days: Option<u32>,
    opts: Option<RemoteCryptoOpts>,
    parent_cert_pem: Option<String>,
    parent_key_pem: Option<String>,
) -> Result<RemoteCertMaterial, String> {
    let logical = key_algo(&algorithm)?;
    let o = opts.unwrap_or_default();
    let alg = resolve_algorithm(&app, &profile_id, &o, logical).await?;
    let subj = build_subject(&subject)?;
    let days = days.unwrap_or(3650).clamp(1, 36500);
    let signed_by_parent = parent_cert_pem
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some();
    if signed_by_parent {
        ensure_pem(parent_cert_pem.as_deref().unwrap_or(""), "上级 CA 证书")?;
        ensure_pem(parent_key_pem.as_deref().unwrap_or(""), "上级 CA 私钥")?;
    }
    let mut s = script_prelude(&o);
    s.push_str(&heredoc("ext.cnf", ca_ext()));
    if signed_by_parent {
        s.push_str(&heredoc("parent.crt", parent_cert_pem.as_deref().unwrap_or("")));
        s.push_str(&heredoc("parent.key", parent_key_pem.as_deref().unwrap_or("")));
    }
    s.push_str(&format!(
        "{{tool}} genpkey -algorithm {alg} -out ca.key\n\
         {{tool}} req -new -key ca.key -subj {} -out ca.csr\n",
        shell_quote(&subj)
    ));
    if signed_by_parent {
        s.push_str(&format!(
            "{{tool}} x509 -req -in ca.csr -CA parent.crt -CAkey parent.key -CAcreateserial -days {days} -extfile ext.cnf -out ca.crt\n"
        ));
    } else {
        s.push_str(&format!(
            "{{tool}} x509 -req -in ca.csr -signkey ca.key -days {days} -extfile ext.cnf -out ca.crt\n"
        ));
    }
    s.push_str(&emit("ca.crt"));
    s.push_str(&emit("ca.key"));
    s.push_str(&emit("ca.csr"));
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(300)).await?;
    ensure_ok(&r, "生成 CA")?;
    Ok(RemoteCertMaterial {
        cert_pem: artifact(&r.output, "ca.crt").ok_or("远端没有返回 CA 证书")?,
        key_pem: artifact(&r.output, "ca.key").ok_or("远端没有返回 CA 私钥")?,
        csr_pem: artifact(&r.output, "ca.csr").unwrap_or_default(),
        chain_pem: String::new(),
        output: r.output,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCsr {
    pub csr_pem: String,
    pub output: String,
}

/// 用现有私钥生成 CSR（SM2/PQC 同样适用）。
#[command]
pub async fn crypto_remote_gen_csr(
    app: AppHandle,
    profile_id: String,
    key_pem: String,
    subject: CertSubject,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteCsr, String> {
    ensure_pem(&key_pem, "私钥")?;
    let o = opts.unwrap_or_default();
    let subj = build_subject(&subject)?;
    let san = san_list(subject.sans.as_deref())?;
    let mut req = format!(
        "{{tool}} req -new -key key.pem -subj {}",
        shell_quote(&subj)
    );
    if let Some(san) = san {
        req.push_str(&format!(
            " -addext {}",
            shell_quote(&format!("subjectAltName={san}"))
        ));
    }
    req.push_str(" -out csr.pem\n");
    let mut s = script_prelude(&o);
    s.push_str(&heredoc("key.pem", &key_pem));
    s.push_str(&req);
    s.push_str(&emit("csr.pem"));
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(180)).await?;
    ensure_ok(&r, "生成 CSR")?;
    Ok(RemoteCsr {
        csr_pem: artifact(&r.output, "csr.pem").ok_or("远端没有返回 CSR")?,
        output: r.output,
    })
}

/// 用给定 CA 签发一张叶证书。
#[command]
pub async fn crypto_remote_issue_cert(
    app: AppHandle,
    profile_id: String,
    algorithm: String,
    ca_cert_pem: String,
    ca_key_pem: String,
    subject: CertSubject,
    days: Option<u32>,
    usage: Option<String>,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteCertMaterial, String> {
    let logical = key_algo(&algorithm)?;
    ensure_pem(&ca_cert_pem, "CA 证书")?;
    ensure_pem(&ca_key_pem, "CA 私钥")?;
    let o = opts.unwrap_or_default();
    let alg = resolve_algorithm(&app, &profile_id, &o, logical).await?;
    let subj = build_subject(&subject)?;
    let days = days.unwrap_or(365).clamp(1, 36500);
    let usage = usage.unwrap_or_else(|| "server".to_string());
    let ext = leaf_ext(&usage, subject.sans.as_deref())?;

    let mut s = script_prelude(&o);
    s.push_str(&heredoc("ca.crt", &ca_cert_pem));
    s.push_str(&heredoc("ca.key", &ca_key_pem));
    s.push_str(&heredoc("ext.cnf", &ext));
    s.push_str(&format!(
        "{{tool}} genpkey -algorithm {alg} -out leaf.key\n\
         {{tool}} req -new -key leaf.key -subj {} -out leaf.csr\n\
         {{tool}} x509 -req -in leaf.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days {days} -extfile ext.cnf -out leaf.crt\n\
         cat leaf.crt ca.crt > chain.pem\n",
        shell_quote(&subj)
    ));
    s.push_str(&emit("leaf.crt"));
    s.push_str(&emit("leaf.key"));
    s.push_str(&emit("leaf.csr"));
    s.push_str(&emit("chain.pem"));
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(300)).await?;
    ensure_ok(&r, "签发证书")?;
    Ok(RemoteCertMaterial {
        cert_pem: artifact(&r.output, "leaf.crt").ok_or("远端没有返回证书")?,
        key_pem: artifact(&r.output, "leaf.key").ok_or("远端没有返回私钥")?,
        csr_pem: artifact(&r.output, "leaf.csr").unwrap_or_default(),
        chain_pem: artifact(&r.output, "chain.pem").unwrap_or_default(),
        output: r.output,
    })
}

/// SM2 双证书：签名证书（digitalSignature）+ 加密证书（keyEncipherment）。
#[command]
pub async fn crypto_remote_issue_sm2_dual(
    app: AppHandle,
    profile_id: String,
    ca_cert_pem: String,
    ca_key_pem: String,
    subject: CertSubject,
    days: Option<u32>,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteSm2Dual, String> {
    ensure_pem(&ca_cert_pem, "CA 证书")?;
    ensure_pem(&ca_key_pem, "CA 私钥")?;
    let o = opts.unwrap_or_default();
    let subj = build_subject(&subject)?;
    let days = days.unwrap_or(365).clamp(1, 36500);
    let sign_ext = leaf_ext("sign", subject.sans.as_deref())?;
    let enc_ext = leaf_ext("enc", subject.sans.as_deref())?;

    let mut s = script_prelude(&o);
    s.push_str(&heredoc("ca.crt", &ca_cert_pem));
    s.push_str(&heredoc("ca.key", &ca_key_pem));
    s.push_str(&heredoc("sign.cnf", &sign_ext));
    s.push_str(&heredoc("enc.cnf", &enc_ext));
    s.push_str(&format!(
        "{{tool}} genpkey -algorithm sm2 -out sign.key\n\
         {{tool}} req -new -key sign.key -subj {} -out sign.csr\n\
         {{tool}} x509 -req -in sign.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days {days} -extfile sign.cnf -out sign.crt\n\
         {{tool}} genpkey -algorithm sm2 -out enc.key\n\
         {{tool}} req -new -key enc.key -subj {} -out enc.csr\n\
         {{tool}} x509 -req -in enc.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days {days} -extfile enc.cnf -out enc.crt\n\
         cat sign.crt enc.crt ca.crt > certs.pem\n\
         cat sign.key enc.key > keys.pem\n\
         cat sign.crt ca.crt > sign-chain.pem\n\
         cat enc.crt ca.crt > enc-chain.pem\n",
        shell_quote(&subj),
        shell_quote(&subj)
    ));
    for f in [
        "sign.crt",
        "sign.key",
        "enc.crt",
        "enc.key",
        "certs.pem",
        "keys.pem",
        "sign-chain.pem",
        "enc-chain.pem",
    ] {
        s.push_str(&emit(f));
    }
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(300)).await?;
    ensure_ok(&r, "生成 SM2 双证书")?;
    Ok(RemoteSm2Dual {
        sign_cert_pem: artifact(&r.output, "sign.crt").ok_or("远端没有返回签名证书")?,
        sign_key_pem: artifact(&r.output, "sign.key").ok_or("远端没有返回签名私钥")?,
        enc_cert_pem: artifact(&r.output, "enc.crt").ok_or("远端没有返回加密证书")?,
        enc_key_pem: artifact(&r.output, "enc.key").ok_or("远端没有返回加密私钥")?,
        certs_pem: artifact(&r.output, "certs.pem").ok_or("远端没有返回证书拼接")?,
        keys_pem: artifact(&r.output, "keys.pem").ok_or("远端没有返回私钥拼接")?,
        sign_chain_pem: artifact(&r.output, "sign-chain.pem").unwrap_or_default(),
        enc_chain_pem: artifact(&r.output, "enc-chain.pem").unwrap_or_default(),
        output: r.output,
    })
}

/// 用 `openssl verify -CAfile` 校验证书链。
#[command]
pub async fn crypto_remote_verify_chain(
    app: AppHandle,
    profile_id: String,
    cert_pem: String,
    ca_pem: String,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteVerifyResult, String> {
    ensure_pem(&cert_pem, "证书")?;
    ensure_pem(&ca_pem, "CA 证书")?;
    let o = opts.unwrap_or_default();
    let mut s = script_prelude(&o);
    // verify 失败要拿到输出与退出码，所以这里不用 set -e。
    s = s.replacen("set -e\n", "set +e\n", 1);
    s.push_str(&heredoc("cert.crt", &cert_pem));
    s.push_str(&heredoc("ca.crt", &ca_pem));
    s.push_str(
        "{tool} verify -CAfile ca.crt cert.crt 2>&1\nrc=$?\necho \"GUAXI_RC=$rc\"\nexit $rc\n",
    );
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(120)).await?;
    Ok(RemoteVerifyResult {
        ok: r.exit_code == Some(0),
        exit_code: r.exit_code,
        output: r.output,
    })
}

/// 用私钥对数据签名；SM2 用 SM3，PQC 先试 dgst，失败再退回 pkeyutl。
#[command]
pub async fn crypto_remote_sign(
    app: AppHandle,
    profile_id: String,
    algorithm: String,
    key_pem: String,
    data_b64: String,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteSignResult, String> {
    let alg = key_algo(&algorithm)?;
    ensure_pem(&key_pem, "私钥")?;
    decode_b64(&data_b64, "数据")?;
    let o = opts.unwrap_or_default();
    let mut s = script_prelude(&o);
    s.push_str(&heredoc("key.pem", &key_pem));
    s.push_str(&format!(
        "printf '%s' {} | base64 -d > data.bin\n",
        shell_quote(data_b64.trim())
    ));
    if is_sm2(alg) {
        s.push_str(
            "{tool} dgst -sm3 -sign key.pem -out sig.bin data.bin\n",
        );
    } else {
        s.push_str(
            "if ! {tool} dgst -sha256 -sign key.pem -out sig.bin data.bin 2>/dev/null; then\n\
               {tool} pkeyutl -sign -rawin -in data.bin -inkey key.pem -out sig.bin\n\
             fi\n",
        );
    }
    s.push_str("base64 sig.bin > sig.b64\n");
    s.push_str(&emit("sig.b64"));
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(180)).await?;
    ensure_ok(&r, "签名")?;
    let sig = artifact(&r.output, "sig.b64").ok_or("远端没有返回签名")?;
    Ok(RemoteSignResult {
        signature_b64: sig.split_whitespace().collect::<String>(),
        output: r.output,
    })
}

/// 验签；公开材料可以是证书 PEM 或公钥 PEM。
#[command]
pub async fn crypto_remote_verify_sign(
    app: AppHandle,
    profile_id: String,
    algorithm: String,
    pub_pem: String,
    data_b64: String,
    signature_b64: String,
    opts: Option<RemoteCryptoOpts>,
) -> Result<RemoteVerifyResult, String> {
    let alg = key_algo(&algorithm)?;
    ensure_pem(&pub_pem, "证书或公钥")?;
    decode_b64(&data_b64, "数据")?;
    decode_b64(&signature_b64, "签名")?;
    let o = opts.unwrap_or_default();
    let mut s = script_prelude(&o);
    s = s.replacen("set -e\n", "set +e\n", 1);
    s.push_str(&heredoc("pub.pem", &pub_pem));
    s.push_str(&format!(
        "printf '%s' {} | base64 -d > data.bin\nprintf '%s' {} | base64 -d > sig.bin\n",
        shell_quote(data_b64.trim()),
        shell_quote(signature_b64.trim())
    ));
    s.push_str("if grep -q 'BEGIN CERTIFICATE' pub.pem; then {tool} x509 -in pub.pem -pubkey -noout > pubkey.pem; else cp pub.pem pubkey.pem; fi\n");
    if is_sm2(alg) {
        s.push_str("{tool} dgst -sm3 -verify pubkey.pem -signature sig.bin data.bin 2>&1\nrc=$?\n");
    } else {
        s.push_str(
            "{tool} dgst -sha256 -verify pubkey.pem -signature sig.bin data.bin 2>&1\n\
             rc=$?\n\
             if [ $rc -ne 0 ]; then {tool} pkeyutl -verify -rawin -in data.bin -pubin -inkey pubkey.pem -sigfile sig.bin 2>&1; rc=$?; fi\n",
        );
    }
    s.push_str("echo \"GUAXI_RC=$rc\"\nexit $rc\n");
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(120)).await?;
    Ok(RemoteVerifyResult {
        ok: r.exit_code == Some(0),
        exit_code: r.exit_code,
        output: r.output,
    })
}

/// 列出远端工具支持的公开密钥算法（用于确认 PQC 算法名）。
#[command]
pub async fn crypto_remote_algorithms(
    app: AppHandle,
    profile_id: String,
    opts: Option<RemoteCryptoOpts>,
) -> Result<String, String> {
    let o = opts.unwrap_or_default();
    let mut s = script_prelude(&o);
    s = s.replacen("set -e\n", "set +e\n", 1);
    s.push_str("{tool} list -public-key-algorithms 2>&1\nexit 0\n");
    let r = run_script(&app, &profile_id, &o, s, o.timeout_secs.unwrap_or(60)).await?;
    Ok(r.output)
}
