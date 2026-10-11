//! 证书 / 密钥 / 加解密工作台（纯 Rust，无 openssl / 无 NASM）。
//!
//! PKI 生成走 rcgen(ring 后端)；RSA 生成走 rsa crate 再喂给 rcgen 签名。
//! 对称/非对称加解密、签名验签走 RustCrypto；PFX 拆打包走 p12。
//! 解析复用 x509-parser / pkcs8 / sec1。文件导入由前端 plugin-fs 读入后传 base64/PEM。

use std::path::PathBuf;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}
fn b64_dec(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    b64().decode(compact.as_bytes()).map_err(|e| format!("base64 解码失败: {e}"))
}
fn b64_enc(d: &[u8]) -> String {
    b64().encode(d)
}

// ============================ 文件导入 / 识别 ============================

#[derive(Serialize)]
pub struct LoadedFile {
    /// 识别到的类型：certificate / private_key / public_key / csr / pkcs12 / der / unknown
    pub kind: String,
    /// 是否 PEM 文本
    pub is_pem: bool,
    /// PEM 文本（is_pem=true）或 DER/PFX 的 base64（is_pem=false）
    pub content: String,
    /// 若为 PEM，所有块的标签
    pub pem_labels: Vec<String>,
}

/// 读入文件字节（前端已 plugin-fs 读到 base64），识别类型，回传可供后续命令使用的内容。
#[command]
pub fn crypto_load_bytes(data_b64: String, file_name: Option<String>) -> Result<LoadedFile, String> {
    let bytes = b64_dec(&data_b64)?;
    let as_text = String::from_utf8(bytes.clone()).ok();
    if let Some(txt) = as_text.as_ref().filter(|t| t.contains("-----BEGIN")) {
        let mut labels = Vec::new();
        for p in pem::parse_many(txt.as_bytes()).unwrap_or_default() {
            labels.push(p.tag().to_string());
        }
        let kind = classify_pem_labels(&labels);
        return Ok(LoadedFile { kind, is_pem: true, content: txt.clone(), pem_labels: labels });
    }
    // 二进制：PFX 常见扩展名或 PKCS12 结构；否则按裸 DER
    let name = file_name.unwrap_or_default().to_lowercase();
    let kind = if name.ends_with(".pfx") || name.ends_with(".p12") {
        "pkcs12".to_string()
    } else {
        "der".to_string()
    };
    Ok(LoadedFile { kind, is_pem: false, content: b64_enc(&bytes), pem_labels: vec![] })
}

fn classify_pem_labels(labels: &[String]) -> String {
    let has = |needle: &str| labels.iter().any(|l| l.contains(needle));
    if has("CERTIFICATE REQUEST") {
        "csr".into()
    } else if has("CERTIFICATE") {
        "certificate".into()
    } else if has("PRIVATE KEY") {
        "private_key".into()
    } else if has("PUBLIC KEY") {
        "public_key".into()
    } else {
        "unknown".into()
    }
}

// __PART2__

// ============================ 密钥 / 证书生成（rcgen + rsa） ============================

use rcgen::{
    BasicConstraints, CertificateParams, CertificateRevocationListParams,
    CertificateSigningRequestParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyIdMethod, KeyPair, KeyUsagePurpose, RevocationReason, RevokedCertParams, SanType,
    SerialNumber, PKCS_ECDSA_P256_SHA256, PKCS_ECDSA_P384_SHA384, PKCS_ED25519,
};
use time::{Duration, OffsetDateTime};

#[derive(Serialize)]
pub struct GenKeyResult {
    pub private_pem: String,
    pub public_pem: String,
}

fn gen_keypair(
    algo: &str,
    rsa_bits: Option<u32>,
    curve: Option<String>,
) -> Result<(KeyPair, String, String), String> {
    match algo {
        "rsa" => {
            use rsa::pkcs8::{EncodePrivateKey, LineEnding};
            let mut rng = rand08::rngs::OsRng;
            let bits = rsa_bits.unwrap_or(2048).clamp(1024, 8192) as usize;
            let sk = rsa::RsaPrivateKey::new(&mut rng, bits)
                .map_err(|e| format!("RSA 生成失败: {e}"))?;
            let priv_pem = sk
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(|e| format!("RSA 私钥编码失败: {e}"))?
                .to_string();
            let kp = KeyPair::from_pem(&priv_pem).map_err(|e| format!("rcgen 载入 RSA 失败: {e}"))?;
            let pub_pem = kp.public_key_pem();
            Ok((kp, priv_pem, pub_pem))
        }
        "ec" => {
            let alg: &'static _ = match curve.as_deref() {
                Some("p384") => &PKCS_ECDSA_P384_SHA384,
                _ => &PKCS_ECDSA_P256_SHA256,
            };
            let kp = KeyPair::generate_for(alg).map_err(|e| format!("EC 生成失败: {e}"))?;
            let priv_pem = kp.serialize_pem();
            let pub_pem = kp.public_key_pem();
            Ok((kp, priv_pem, pub_pem))
        }
        "ed25519" => {
            let kp = KeyPair::generate_for(&PKCS_ED25519).map_err(|e| format!("Ed25519 生成失败: {e}"))?;
            let priv_pem = kp.serialize_pem();
            let pub_pem = kp.public_key_pem();
            Ok((kp, priv_pem, pub_pem))
        }
        _ => Err(format!("不支持的算法: {algo}（支持 rsa / ec / ed25519）")),
    }
}

#[command]
pub fn crypto_gen_key(
    algo: String,
    rsa_bits: Option<u32>,
    curve: Option<String>,
) -> Result<GenKeyResult, String> {
    let (_kp, private_pem, public_pem) = gen_keypair(&algo, rsa_bits, curve)?;
    Ok(GenKeyResult { private_pem, public_pem })
}

fn apply_profile(p: &mut CertificateParams, profile: &str, days: i64) {
    let now = OffsetDateTime::now_utc();
    p.not_before = now - Duration::hours(1);
    p.not_after = now + Duration::days(days.max(1));
    match profile {
        "ca" => {
            p.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            p.key_usages = vec![
                KeyUsagePurpose::KeyCertSign,
                KeyUsagePurpose::CrlSign,
                KeyUsagePurpose::DigitalSignature,
            ];
        }
        "server" => {
            p.is_ca = IsCa::NoCa;
            p.key_usages =
                vec![KeyUsagePurpose::DigitalSignature, KeyUsagePurpose::KeyEncipherment];
            p.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        }
        "client" => {
            p.is_ca = IsCa::NoCa;
            p.key_usages = vec![KeyUsagePurpose::DigitalSignature];
            p.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
        }
        _ => {
            p.is_ca = IsCa::NoCa;
        }
    }
}

fn build_params(
    cn: &str,
    org: Option<String>,
    sans: &[String],
    profile: &str,
    days: i64,
) -> Result<CertificateParams, String> {
    let mut p = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, cn);
    if let Some(o) = org.filter(|s| !s.trim().is_empty()) {
        dn.push(DnType::OrganizationName, o);
    }
    p.distinguished_name = dn;
    for s in sans.iter().filter(|s| !s.trim().is_empty()) {
        if let Ok(ip) = s.parse::<std::net::IpAddr>() {
            p.subject_alt_names.push(SanType::IpAddress(ip));
        } else {
            let ia5 = s.clone().try_into().map_err(|_| format!("非法 DNS SAN: {s}"))?;
            p.subject_alt_names.push(SanType::DnsName(ia5));
        }
    }
    apply_profile(&mut p, profile, days);
    Ok(p)
}

#[command]
pub fn crypto_gen_csr(
    key_pem: String,
    common_name: String,
    org: Option<String>,
    sans: Vec<String>,
) -> Result<String, String> {
    let kp = KeyPair::from_pem(&key_pem).map_err(|e| format!("载入私钥失败: {e}"))?;
    let params = build_params(&common_name, org, &sans, "server", 825)?;
    let csr = params.serialize_request(&kp).map_err(|e| format!("CSR 生成失败: {e}"))?;
    csr.pem().map_err(|e| format!("CSR 编码失败: {e}"))
}

// __PART3__

#[derive(Serialize)]
pub struct IssueResult {
    pub cert_pem: String,
    /// 当后端代为生成了 subject 私钥时回传（PEM）。
    pub key_pem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_id: Option<String>,
}

/// 统一签发：self-signed(root CA)、CA 签 intermediate、CA 直签 server/client 都走这里。
/// 给了 issuer_* 则由其签发，否则自签。给了 subject_key_pem 则用之，否则按 algo 生成并回传。
#[command]
#[allow(clippy::too_many_arguments)]
pub fn crypto_issue_cert(
    subject_cn: String,
    org: Option<String>,
    sans: Vec<String>,
    profile: String,
    days: i64,
    subject_key_pem: Option<String>,
    algo: Option<String>,
    rsa_bits: Option<u32>,
    curve: Option<String>,
    issuer_cert_pem: Option<String>,
    issuer_key_pem: Option<String>,
) -> Result<IssueResult, String> {
    let (subject_kp, gen_key_pem) = match subject_key_pem {
        Some(pem) if !pem.trim().is_empty() => {
            (KeyPair::from_pem(&pem).map_err(|e| format!("载入 subject 私钥失败: {e}"))?, None)
        }
        _ => {
            let (kp, priv_pem, _pub) =
                gen_keypair(algo.as_deref().unwrap_or("ec"), rsa_bits, curve)?;
            (kp, Some(priv_pem))
        }
    };
    let params = build_params(&subject_cn, org, &sans, &profile, days)?;

    let cert_pem = match (issuer_cert_pem, issuer_key_pem) {
        (Some(ic), Some(ik)) if !ic.trim().is_empty() && !ik.trim().is_empty() => {
            let issuer_kp = KeyPair::from_pem(&ik).map_err(|e| format!("载入 issuer 私钥失败: {e}"))?;
            let issuer_params = CertificateParams::from_ca_cert_pem(&ic)
                .map_err(|e| format!("解析 issuer 证书失败: {e}"))?;
            let issuer_cert = issuer_params
                .self_signed(&issuer_kp)
                .map_err(|e| format!("重建 issuer 失败: {e}"))?;
            params
                .signed_by(&subject_kp, &issuer_cert, &issuer_kp)
                .map_err(|e| format!("签发失败: {e}"))?
                .pem()
        }
        _ => params
            .self_signed(&subject_kp)
            .map_err(|e| format!("自签失败: {e}"))?
            .pem(),
    };
    Ok(IssueResult { cert_pem, key_pem: gen_key_pem, serial: None, leaf_id: None })
}

/// 用 CA 直接签发一份已有 CSR。
#[command]
pub fn crypto_sign_csr(
    csr_pem: String,
    issuer_cert_pem: String,
    issuer_key_pem: String,
    profile: String,
    days: i64,
) -> Result<String, String> {
    let mut csrp = CertificateSigningRequestParams::from_pem(&csr_pem)
        .map_err(|e| format!("解析 CSR 失败: {e}"))?;
    apply_profile(&mut csrp.params, &profile, days);
    let issuer_kp =
        KeyPair::from_pem(&issuer_key_pem).map_err(|e| format!("载入 issuer 私钥失败: {e}"))?;
    let issuer_params = CertificateParams::from_ca_cert_pem(&issuer_cert_pem)
        .map_err(|e| format!("解析 issuer 证书失败: {e}"))?;
    let issuer_cert = issuer_params
        .self_signed(&issuer_kp)
        .map_err(|e| format!("重建 issuer 失败: {e}"))?;
    let signed = csrp
        .signed_by(&issuer_cert, &issuer_kp)
        .map_err(|e| format!("签发失败: {e}"))?;
    Ok(signed.pem())
}

fn pem_to_der_of(input: &str, want_tag_contains: &str) -> Result<Vec<u8>, String> {
    for p in pem::parse_many(input.as_bytes()).map_err(|e| format!("PEM 解析失败: {e}"))? {
        if p.tag().contains(want_tag_contains) {
            return Ok(p.contents().to_vec());
        }
    }
    Err(format!("未找到 {want_tag_contains} 块"))
}

/// 验证证书是否由给定颁发者证书签名。
#[command]
pub fn crypto_verify_cert(cert_pem: String, issuer_pem: String) -> Result<bool, String> {
    let cert_der = pem_to_der_of(&cert_pem, "CERTIFICATE")?;
    let issuer_der = pem_to_der_of(&issuer_pem, "CERTIFICATE")?;
    let (_, cert) = x509_parser::parse_x509_certificate(&cert_der)
        .map_err(|e| format!("解析证书失败: {e}"))?;
    let (_, issuer) = x509_parser::parse_x509_certificate(&issuer_der)
        .map_err(|e| format!("解析颁发者失败: {e}"))?;
    Ok(cert.verify_signature(Some(issuer.public_key())).is_ok())
}

// __PART4__

// ============================ PFX / PKCS#12 拆打包（p12） ============================

#[derive(Serialize)]
pub struct PfxContents {
    pub keys_pem: Vec<String>,
    pub certs_pem: Vec<String>,
}

fn der_to_pem(tag: &str, der: &[u8]) -> String {
    let p = pem::Pem::new(tag.to_string(), der.to_vec());
    pem::encode(&p)
}

#[command]
pub fn crypto_pfx_parse(data_b64: String, password: String) -> Result<PfxContents, String> {
    let der = b64_dec(&data_b64)?;
    let pfx = p12::PFX::parse(&der).map_err(|e| format!("PFX 解析失败: {e}"))?;
    let keys = pfx.key_bags(&password).map_err(|e| format!("取私钥失败(密码?): {e}"))?;
    let certs = pfx.cert_x509_bags(&password).map_err(|e| format!("取证书失败: {e}"))?;
    Ok(PfxContents {
        keys_pem: keys.iter().map(|d| der_to_pem("PRIVATE KEY", d)).collect(),
        certs_pem: certs.iter().map(|d| der_to_pem("CERTIFICATE", d)).collect(),
    })
}

#[command]
pub fn crypto_pfx_build(
    key_pem: String,
    certs_pem: Vec<String>,
    password: String,
    name: Option<String>,
) -> Result<String, String> {
    if certs_pem.is_empty() {
        return Err("至少需要一张证书".into());
    }
    let key_der = pem_to_der_of(&key_pem, "PRIVATE KEY")?;
    let mut cert_ders: Vec<Vec<u8>> = Vec::new();
    for c in &certs_pem {
        cert_ders.push(pem_to_der_of(c, "CERTIFICATE")?);
    }
    let leaf = &cert_ders[0];
    let cas: Vec<&[u8]> = cert_ders[1..].iter().map(|v| v.as_slice()).collect();
    let pfx = p12::PFX::new_with_cas(leaf, &key_der, &cas, &password, &name.unwrap_or_default())
        .ok_or_else(|| "PFX 打包失败".to_string())?;
    Ok(b64_enc(&pfx.to_der()))
}

// ============================ 对称加解密 ============================

#[command]
pub fn crypto_sym(
    algo: String,
    mode: String,
    key_hex: String,
    iv_hex: String,
    data_b64: String,
    encrypt: bool,
    aad_hex: Option<String>,
) -> Result<String, String> {
    let key = hex::decode(key_hex.trim()).map_err(|e| format!("key 非法 hex: {e}"))?;
    let iv = hex::decode(iv_hex.trim()).map_err(|e| format!("iv/nonce 非法 hex: {e}"))?;
    let data = b64_dec(&data_b64)?;
    let aad = match aad_hex {
        Some(a) if !a.trim().is_empty() => hex::decode(a.trim()).map_err(|e| format!("aad 非法 hex: {e}"))?,
        _ => Vec::new(),
    };
    let out = match algo.as_str() {
        "aes" => match mode.as_str() {
            "cbc" => aes_cbc(&key, &iv, &data, encrypt)?,
            "ctr" => aes_ctr(&key, &iv, &data)?,
            "gcm" => aes_gcm(&key, &iv, &data, &aad, encrypt)?,
            m => return Err(format!("AES 不支持模式: {m}")),
        },
        "3des" => return Err("3DES 暂未启用（与 PKCS12 库的 des 版本冲突，已改用 AES/ChaCha20）".into()),
        "chacha20poly1305" => chacha(&key, &iv, &data, &aad, encrypt)?,
        a => return Err(format!("不支持算法: {a}")),
    };
    Ok(b64_enc(&out))
}

fn aes_cbc(key: &[u8], iv: &[u8], data: &[u8], enc: bool) -> Result<Vec<u8>, String> {
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
    macro_rules! run {
        ($c:ty) => {{
            if enc {
                let e = <cbc::Encryptor<$c>>::new_from_slices(key, iv)
                    .map_err(|e| format!("参数错: {e}"))?;
                Ok(e.encrypt_padded_vec_mut::<Pkcs7>(data))
            } else {
                let d = <cbc::Decryptor<$c>>::new_from_slices(key, iv)
                    .map_err(|e| format!("参数错: {e}"))?;
                d.decrypt_padded_vec_mut::<Pkcs7>(data).map_err(|e| format!("解密失败: {e}"))
            }
        }};
    }
    match key.len() {
        16 => run!(aes::Aes128),
        24 => run!(aes::Aes192),
        32 => run!(aes::Aes256),
        n => Err(format!("AES 密钥长度非法: {n} 字节")),
    }
}

fn aes_ctr(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use ctr::cipher::{KeyIvInit, StreamCipher};
    macro_rules! run {
        ($c:ty) => {{
            let mut c = <$c>::new_from_slices(key, iv).map_err(|e| format!("参数错: {e}"))?;
            let mut buf = data.to_vec();
            c.apply_keystream(&mut buf);
            Ok(buf)
        }};
    }
    match key.len() {
        16 => run!(ctr::Ctr128BE<aes::Aes128>),
        24 => run!(ctr::Ctr128BE<aes::Aes192>),
        32 => run!(ctr::Ctr128BE<aes::Aes256>),
        n => Err(format!("AES 密钥长度非法: {n} 字节")),
    }
}

fn aes_gcm(key: &[u8], nonce: &[u8], data: &[u8], aad: &[u8], enc: bool) -> Result<Vec<u8>, String> {
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes128Gcm, Aes256Gcm, Nonce};
    let n = Nonce::from_slice(nonce);
    let pl = Payload { msg: data, aad };
    macro_rules! run {
        ($c:ty) => {{
            let c = <$c>::new_from_slice(key).map_err(|e| format!("key 错: {e}"))?;
            if enc {
                c.encrypt(n, pl).map_err(|e| format!("加密失败: {e}"))
            } else {
                c.decrypt(n, pl).map_err(|e| format!("解密失败(认证?): {e}"))
            }
        }};
    }
    match key.len() {
        16 => run!(Aes128Gcm),
        32 => run!(Aes256Gcm),
        n => Err(format!("AES-GCM 密钥需 16/32 字节，收到 {n}")),
    }
}

fn chacha(key: &[u8], nonce: &[u8], data: &[u8], aad: &[u8], enc: bool) -> Result<Vec<u8>, String> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
    if key.len() != 32 {
        return Err(format!("ChaCha20Poly1305 密钥需 32 字节，收到 {}", key.len()));
    }
    let c = ChaCha20Poly1305::new(Key::from_slice(key));
    let n = Nonce::from_slice(nonce);
    let pl = Payload { msg: data, aad };
    if enc {
        c.encrypt(n, pl).map_err(|e| format!("加密失败: {e}"))
    } else {
        c.decrypt(n, pl).map_err(|e| format!("解密失败(认证?): {e}"))
    }
}

// __PART5__

// ============================ 非对称加解密 / 签名 / 验签 / 散列 ============================

use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};

fn load_rsa_public(pem: &str) -> Result<RsaPublicKey, String> {
    if let Ok(pk) = RsaPublicKey::from_public_key_pem(pem) {
        return Ok(pk);
    }
    let sk = RsaPrivateKey::from_pkcs8_pem(pem).map_err(|e| format!("载入 RSA 公钥/私钥失败: {e}"))?;
    Ok(RsaPublicKey::from(&sk))
}

#[command]
pub fn crypto_rsa_crypt(
    op: String,
    key_pem: String,
    data_b64: String,
    scheme: String,
) -> Result<String, String> {
    use rsa::{Oaep, Pkcs1v15Encrypt};
    let data = b64_dec(&data_b64)?;
    if op == "encrypt" {
        let pk = load_rsa_public(&key_pem)?;
        let mut rng = rand08::rngs::OsRng;
        let ct = if scheme == "pkcs1" {
            pk.encrypt(&mut rng, Pkcs1v15Encrypt, &data)
        } else {
            pk.encrypt(&mut rng, Oaep::new::<sha2::Sha256>(), &data)
        }
        .map_err(|e| format!("RSA 加密失败: {e}"))?;
        Ok(b64_enc(&ct))
    } else {
        let sk = RsaPrivateKey::from_pkcs8_pem(&key_pem).map_err(|e| format!("载入私钥失败: {e}"))?;
        let pt = if scheme == "pkcs1" {
            sk.decrypt(Pkcs1v15Encrypt, &data)
        } else {
            sk.decrypt(Oaep::new::<sha2::Sha256>(), &data)
        }
        .map_err(|e| format!("RSA 解密失败: {e}"))?;
        Ok(b64_enc(&pt))
    }
}

#[command]
pub fn crypto_sign(key_pem: String, data_b64: String, scheme: String) -> Result<String, String> {
    let data = b64_dec(&data_b64)?;
    let sig: Vec<u8> = match scheme.as_str() {
        "rsa-pkcs1-sha256" => {
            use rsa::signature::{SignatureEncoding, Signer};
            let sk = RsaPrivateKey::from_pkcs8_pem(&key_pem).map_err(|e| e.to_string())?;
            let signer = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(sk);
            signer.sign(&data).to_vec()
        }
        "rsa-pss-sha256" => {
            use rsa::signature::{RandomizedSigner, SignatureEncoding};
            let sk = RsaPrivateKey::from_pkcs8_pem(&key_pem).map_err(|e| e.to_string())?;
            let signer = rsa::pss::SigningKey::<sha2::Sha256>::new(sk);
            let mut rng = rand08::rngs::OsRng;
            signer.sign_with_rng(&mut rng, &data).to_vec()
        }
        "ecdsa-p256" => {
            use p256::ecdsa::{signature::Signer, Signature, SigningKey};
            let sk = SigningKey::from_pkcs8_pem(&key_pem).map_err(|e| format!("载入 EC 私钥失败: {e}"))?;
            let s: Signature = sk.sign(&data);
            s.to_der().as_bytes().to_vec()
        }
        "ecdsa-p384" => {
            use p384::ecdsa::{signature::Signer, Signature, SigningKey};
            let sk = SigningKey::from_pkcs8_pem(&key_pem).map_err(|e| format!("载入 EC 私钥失败: {e}"))?;
            let s: Signature = sk.sign(&data);
            s.to_der().as_bytes().to_vec()
        }
        "ed25519" => {
            use ed25519_dalek::pkcs8::DecodePrivateKey;
            use ed25519_dalek::{Signer, SigningKey};
            let sk = SigningKey::from_pkcs8_pem(&key_pem).map_err(|e| format!("载入 Ed25519 私钥失败: {e}"))?;
            sk.sign(&data).to_bytes().to_vec()
        }
        s => return Err(format!("不支持的签名方案: {s}")),
    };
    Ok(b64_enc(&sig))
}

#[command]
pub fn crypto_verify(
    pub_pem: String,
    data_b64: String,
    sig_b64: String,
    scheme: String,
) -> Result<bool, String> {
    let data = b64_dec(&data_b64)?;
    let sig = b64_dec(&sig_b64)?;
    let ok = match scheme.as_str() {
        "rsa-pkcs1-sha256" => {
            use rsa::signature::Verifier;
            let pk = load_rsa_public(&pub_pem)?;
            let vk = rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(pk);
            match rsa::pkcs1v15::Signature::try_from(sig.as_slice()) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }
        "rsa-pss-sha256" => {
            use rsa::signature::Verifier;
            let pk = load_rsa_public(&pub_pem)?;
            let vk = rsa::pss::VerifyingKey::<sha2::Sha256>::new(pk);
            match rsa::pss::Signature::try_from(sig.as_slice()) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }
        "ecdsa-p256" => {
            use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
            let vk = VerifyingKey::from_public_key_pem(&pub_pem).map_err(|e| e.to_string())?;
            match Signature::from_der(&sig) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }
        "ecdsa-p384" => {
            use p384::ecdsa::{signature::Verifier, Signature, VerifyingKey};
            let vk = VerifyingKey::from_public_key_pem(&pub_pem).map_err(|e| e.to_string())?;
            match Signature::from_der(&sig) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }
        "ed25519" => {
            use ed25519_dalek::pkcs8::DecodePublicKey;
            use ed25519_dalek::{Signature, Verifier, VerifyingKey};
            let vk = VerifyingKey::from_public_key_pem(&pub_pem).map_err(|e| e.to_string())?;
            match Signature::from_slice(&sig) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }
        s => return Err(format!("不支持的验签方案: {s}")),
    };
    Ok(ok)
}

#[command]
pub fn crypto_hash(algo: String, data_b64: String) -> Result<String, String> {
    use sha2::Digest;
    let data = b64_dec(&data_b64)?;
    let hex_out = match algo.as_str() {
        "md5" => {
            use md5::Digest as _;
            hex::encode(md5::Md5::digest(&data))
        }
        "sha1" => {
            use sha1::Digest as _;
            hex::encode(sha1::Sha1::digest(&data))
        }
        "sha256" => hex::encode(sha2::Sha256::digest(&data)),
        "sha384" => hex::encode(sha2::Sha384::digest(&data)),
        "sha512" => hex::encode(sha2::Sha512::digest(&data)),
        a => return Err(format!("不支持的散列: {a}")),
    };
    Ok(hex_out)
}

// ============================ CA 库（后台记录 CA，签发时下拉选择，无需粘贴 PEM） ============================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub org: Option<String>,
    #[serde(default)]
    pub org_unit: Option<String>,
    pub common_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokedRec {
    pub serial: u64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaEntry {
    pub id: String,
    pub name: String,
    pub algo: String,
    pub subject_cn: String,
    pub cert_pem: String,
    pub key_pem: String,
    pub is_root: bool,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// 分类库：rsa | ecc | sm2 | pqc（旧档案无此字段，按 algo 推断）。
    #[serde(default)]
    pub category: String,
    /// 下一个要分配的证书序列号（CA 自身用 1）。
    #[serde(default = "default_serial_counter")]
    pub serial_counter: u64,
    /// 下一个 CRL 号。
    #[serde(default = "default_crl_number")]
    pub crl_number: u64,
    #[serde(default)]
    pub revoked: Vec<RevokedRec>,
}

fn default_serial_counter() -> u64 {
    2
}
fn default_crl_number() -> u64 {
    1
}

fn algo_to_category(algo: &str) -> String {
    match algo {
        "rsa" => "rsa",
        "ec" => "ecc",
        "sm2" => "sm2",
        s if s.starts_with("ml-") || s == "pqc" => "pqc",
        _ => "ecc",
    }
    .to_string()
}
fn ca_category(e: &CaEntry) -> String {
    if e.category.is_empty() {
        algo_to_category(&e.algo)
    } else {
        e.category.clone()
    }
}

#[derive(Serialize)]
pub struct CaSummary {
    pub id: String,
    pub name: String,
    pub algo: String,
    pub category: String,
    pub subject_cn: String,
    pub is_root: bool,
    pub parent_id: Option<String>,
    pub revoked_count: usize,
    pub crl_number: u64,
}

fn ca_summary(e: &CaEntry) -> CaSummary {
    CaSummary {
        id: e.id.clone(),
        name: e.name.clone(),
        algo: e.algo.clone(),
        category: ca_category(e),
        subject_cn: e.subject_cn.clone(),
        is_root: e.is_root,
        parent_id: e.parent_id.clone(),
        revoked_count: e.revoked.len(),
        crl_number: e.crl_number,
    }
}

fn ca_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("crypto_lab");
    std::fs::create_dir_all(&dir).map_err(|e| format!("建目录失败: {e}"))?;
    Ok(dir.join("ca_store.json"))
}

fn read_cas(app: &AppHandle) -> Result<Vec<CaEntry>, String> {
    let p = ca_store_path(app)?;
    if !p.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&p).map_err(|e| format!("读 CA 库失败: {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("解析 CA 库失败: {e}"))
}

fn write_cas(app: &AppHandle, cas: &[CaEntry]) -> Result<(), String> {
    let p = ca_store_path(app)?;
    let json = serde_json::to_string_pretty(cas).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("写 CA 库失败: {e}"))
}

fn dn_from_subject(s: &Subject) -> DistinguishedName {
    let mut dn = DistinguishedName::new();
    for (t, v) in [
        (DnType::CountryName, &s.country),
        (DnType::StateOrProvinceName, &s.state),
        (DnType::LocalityName, &s.locality),
        (DnType::OrganizationName, &s.org),
        (DnType::OrganizationalUnitName, &s.org_unit),
    ] {
        if let Some(x) = v.as_ref().filter(|x| !x.trim().is_empty()) {
            dn.push(t, x.clone());
        }
    }
    dn.push(DnType::CommonName, s.common_name.clone());
    dn
}

fn push_sans(p: &mut CertificateParams, sans: &[String]) -> Result<(), String> {
    for s in sans.iter().filter(|s| !s.trim().is_empty()) {
        if let Ok(ip) = s.parse::<std::net::IpAddr>() {
            p.subject_alt_names.push(SanType::IpAddress(ip));
        } else {
            let ia5 = s.clone().try_into().map_err(|_| format!("非法 DNS SAN: {s}"))?;
            p.subject_alt_names.push(SanType::DnsName(ia5));
        }
    }
    Ok(())
}

/// 重建一个 CA 的 rcgen Issuer 证书（用于读取 DN + 用其私钥签名）。
fn ca_issuer(ca: &CaEntry) -> Result<(KeyPair, rcgen::Certificate), String> {
    let kp = KeyPair::from_pem(&ca.key_pem).map_err(|e| format!("载入 CA 私钥失败: {e}"))?;
    let params = CertificateParams::from_ca_cert_pem(&ca.cert_pem)
        .map_err(|e| format!("解析 CA 证书失败: {e}"))?;
    let cert = params.self_signed(&kp).map_err(|e| format!("重建 CA 失败: {e}"))?;
    Ok((kp, cert))
}

#[command]
pub fn crypto_ca_list(app: AppHandle) -> Result<Vec<CaSummary>, String> {
    Ok(read_cas(&app)?.iter().map(ca_summary).collect())
}

/// 新建 CA：issuer_ca_id 为空=自签根 CA；否则由该 CA 签发为中间 CA。后台记录 cert+key。
/// category: rsa | ecc 本地 rcgen 签发；sm2 | pqc 需 `tlcp` 特性(Tongsuo)，否则报错。
#[command]
#[allow(clippy::too_many_arguments)]
pub fn crypto_ca_create(
    app: AppHandle,
    name: String,
    category: String,
    subject: Subject,
    rsa_bits: Option<u32>,
    curve: Option<String>,
    days: i64,
    issuer_ca_id: Option<String>,
) -> Result<CaSummary, String> {
    let cat = if category.trim().is_empty() { "ecc".to_string() } else { category };
    let mut cas = read_cas(&app)?;
    let parent_idx = match issuer_ca_id.as_ref().filter(|s| !s.is_empty()) {
        Some(pid) => Some(cas.iter().position(|c| &c.id == pid).ok_or("上级 CA 不存在")?),
        None => None,
    };
    let is_root = parent_idx.is_none();

    let (algo_stored, cert_pem, key_pem) = if cat == "sm2" || cat == "pqc" {
        // SM2/PQC 证书用 Tongsuo 引擎（feature=tlcp）；默认构建会返回未启用错误。
        let parent = parent_idx.map(|i| cas[i].clone());
        let (cert, key) = engine_ca_create(&cat, &subject, days, parent.as_ref())?;
        (cat.clone(), cert, key)
    } else {
        let algo = if cat == "rsa" { "rsa" } else { "ec" };
        let (kp, priv_pem, _pub) = gen_keypair(algo, rsa_bits, curve)?;
        let mut params = CertificateParams::default();
        params.distinguished_name = dn_from_subject(&subject);
        apply_profile(&mut params, "ca", days);
        let cert_pem = match parent_idx {
            Some(i) => {
                let sn = take_serial(&mut cas, i);
                params.serial_number = Some(SerialNumber::from(sn));
                let parent = cas[i].clone();
                let (issuer_kp, issuer_cert) = ca_issuer(&parent)?;
                params
                    .signed_by(&kp, &issuer_cert, &issuer_kp)
                    .map_err(|e| format!("中间 CA 签发失败: {e}"))?
                    .pem()
            }
            None => {
                params.serial_number = Some(SerialNumber::from(1u64));
                params.self_signed(&kp).map_err(|e| format!("根 CA 自签失败: {e}"))?.pem()
            }
        };
        (algo.to_string(), cert_pem, priv_pem)
    };

    let entry = CaEntry {
        id: Uuid::new_v4().to_string(),
        name,
        algo: algo_stored,
        subject_cn: subject.common_name,
        cert_pem,
        key_pem,
        is_root,
        parent_id: parent_idx.map(|i| cas[i].id.clone()),
        category: cat,
        serial_counter: 2,
        crl_number: 1,
        revoked: Vec::new(),
    };
    let summary = ca_summary(&entry);
    cas.push(entry);
    write_cas(&app, &cas)?;
    Ok(summary)
}

/// 取 CA 的下一个序列号并自增（内存中），调用方负责随后 write_cas 落盘。
fn take_serial(cas: &mut [CaEntry], idx: usize) -> u64 {
    let n = cas[idx].serial_counter.max(2);
    cas[idx].serial_counter = n + 1;
    n
}

#[command]
pub fn crypto_ca_import(
    app: AppHandle,
    name: String,
    cert_pem: String,
    key_pem: String,
) -> Result<CaSummary, String> {
    let probe = CaEntry {
        id: String::new(),
        name: String::new(),
        algo: String::new(),
        subject_cn: String::new(),
        cert_pem: cert_pem.clone(),
        key_pem: key_pem.clone(),
        is_root: true,
        parent_id: None,
        category: "imported".into(),
        serial_counter: 2,
        crl_number: 1,
        revoked: Vec::new(),
    };
    let _ = ca_issuer(&probe)?;
    let der = pem_to_der_of(&cert_pem, "CERTIFICATE")?;
    let subject_cn = x509_parser::parse_x509_certificate(&der)
        .ok()
        .map(|(_, c)| c.subject().to_string())
        .unwrap_or_default();
    let mut cas = read_cas(&app)?;
    let entry = CaEntry { id: Uuid::new_v4().to_string(), name, subject_cn, ..probe };
    let summary = ca_summary(&entry);
    cas.push(entry);
    write_cas(&app, &cas)?;
    Ok(summary)
}

#[command]
pub fn crypto_ca_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut cas = read_cas(&app)?;
    cas.retain(|c| c.id != id);
    write_cas(&app, &cas)
}

#[derive(Serialize)]
pub struct CaExport {
    pub cert_pem: String,
    pub key_pem: String,
}

#[command]
pub fn crypto_ca_export(app: AppHandle, id: String) -> Result<CaExport, String> {
    let cas = read_cas(&app)?;
    let ca = cas.iter().find(|c| c.id == id).ok_or("CA 不存在")?;
    Ok(CaExport { cert_pem: ca.cert_pem.clone(), key_pem: ca.key_pem.clone() })
}

/// 用库里的 CA 直接签发 server/client 终端证书：分配序列号、存入终端证书库、返回证书+私钥。
/// 终端证书的算法默认跟随 CA 的类别（SM2/PQC 走 Tongsuo 引擎）。
#[command]
pub fn crypto_issue_from_ca(
    app: AppHandle,
    ca_id: String,
    subject: Subject,
    profile: String,
    sans: Vec<String>,
    days: i64,
) -> Result<IssueResult, String> {
    let mut cas = read_cas(&app)?;
    let idx = cas.iter().position(|c| c.id == ca_id).ok_or("签发 CA 不存在")?;
    let cat = ca_category(&cas[idx]);
    let serial = take_serial(&mut cas, idx);

    let (cert_pem, key_pem) = if cat == "sm2" || cat == "pqc" {
        let ca = cas[idx].clone();
        engine_issue_leaf(&ca, &subject, &profile, &sans, days, serial)?
    } else {
        let algo = if cat == "rsa" { "rsa" } else { "ec" };
        let (kp, priv_pem, _pub) = gen_keypair(algo, None, None)?;
        let mut params = CertificateParams::default();
        params.distinguished_name = dn_from_subject(&subject);
        push_sans(&mut params, &sans)?;
        apply_profile(&mut params, &profile, days);
        params.serial_number = Some(SerialNumber::from(serial));
        let ca = cas[idx].clone();
        let (issuer_kp, issuer_cert) = ca_issuer(&ca)?;
        let cert = params
            .signed_by(&kp, &issuer_cert, &issuer_kp)
            .map_err(|e| format!("签发失败: {e}"))?;
        (cert.pem(), priv_pem)
    };

    // 持久化：CA 序列号自增落盘 + 终端证书入库。
    write_cas(&app, &cas)?;
    let leaf = LeafEntry {
        id: Uuid::new_v4().to_string(),
        name: subject.common_name.clone(),
        kind: profile.clone(),
        ca_id: ca_id.clone(),
        subject_cn: subject.common_name,
        serial,
        cert_pem: cert_pem.clone(),
        key_pem: key_pem.clone(),
        created: now_str(),
    };
    let leaf_id = leaf.id.clone();
    let mut leaves = read_leaves(&app)?;
    leaves.push(leaf);
    write_leaves(&app, &leaves)?;

    Ok(IssueResult {
        cert_pem,
        key_pem: Some(key_pem),
        serial: Some(serial),
        leaf_id: Some(leaf_id),
    })
}

fn now_str() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

// ============================ 终端证书库（client/server leaf_store.json） ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub ca_id: String,
    pub subject_cn: String,
    pub serial: u64,
    pub cert_pem: String,
    pub key_pem: String,
    #[serde(default)]
    pub created: String,
}

#[derive(Serialize)]
pub struct LeafSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub ca_id: String,
    pub ca_name: String,
    pub subject_cn: String,
    pub serial: u64,
    pub created: String,
}

fn leaf_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("crypto_lab");
    std::fs::create_dir_all(&dir).map_err(|e| format!("建目录失败: {e}"))?;
    Ok(dir.join("leaf_store.json"))
}
fn read_leaves(app: &AppHandle) -> Result<Vec<LeafEntry>, String> {
    let p = leaf_store_path(app)?;
    if !p.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
        .map_err(|e| format!("解析终端证书库失败: {e}"))
}
fn write_leaves(app: &AppHandle, v: &[LeafEntry]) -> Result<(), String> {
    let p = leaf_store_path(app)?;
    std::fs::write(&p, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写终端证书库失败: {e}"))
}

#[command]
pub fn crypto_leaf_list(app: AppHandle) -> Result<Vec<LeafSummary>, String> {
    let cas = read_cas(&app)?;
    let name_of = |id: &str| cas.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
    Ok(read_leaves(&app)?
        .into_iter()
        .map(|l| LeafSummary {
            ca_name: name_of(&l.ca_id),
            id: l.id,
            name: l.name,
            kind: l.kind,
            ca_id: l.ca_id,
            subject_cn: l.subject_cn,
            serial: l.serial,
            created: l.created,
        })
        .collect())
}

#[command]
pub fn crypto_leaf_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut v = read_leaves(&app)?;
    v.retain(|l| l.id != id);
    write_leaves(&app, &v)
}

#[derive(Serialize)]
pub struct LeafExport {
    pub cert_pem: String,
    pub key_pem: String,
    /// 叶证书 + 一路到根的 CA 证书，拼成完整链。
    pub chain_pem: String,
}

/// 顺 ca_id→parent_id 取到根，返回"叶 + 各级 CA"证书链。
fn build_chain(cas: &[CaEntry], leaf_cert: &str, ca_id: &str) -> String {
    let mut out = String::from(leaf_cert);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    let mut cur = Some(ca_id.to_string());
    let mut guard = 0;
    while let Some(id) = cur {
        guard += 1;
        if guard > 32 {
            break;
        }
        match cas.iter().find(|c| c.id == id) {
            Some(ca) => {
                out.push_str(&ca.cert_pem);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                cur = ca.parent_id.clone();
            }
            None => break,
        }
    }
    out
}

#[command]
pub fn crypto_leaf_export(app: AppHandle, id: String) -> Result<LeafExport, String> {
    let leaves = read_leaves(&app)?;
    let l = leaves.iter().find(|x| x.id == id).ok_or("证书不存在")?;
    let cas = read_cas(&app)?;
    Ok(LeafExport {
        cert_pem: l.cert_pem.clone(),
        key_pem: l.key_pem.clone(),
        chain_pem: build_chain(&cas, &l.cert_pem, &l.ca_id),
    })
}

// ============================ 吊销 + CRL ============================

fn revocation_reason(s: &str) -> Option<RevocationReason> {
    match s {
        "keyCompromise" => Some(RevocationReason::KeyCompromise),
        "caCompromise" => Some(RevocationReason::CaCompromise),
        "affiliationChanged" => Some(RevocationReason::AffiliationChanged),
        "superseded" => Some(RevocationReason::Superseded),
        "cessationOfOperation" => Some(RevocationReason::CessationOfOperation),
        "certificateHold" => Some(RevocationReason::CertificateHold),
        "privilegeWithdrawn" => Some(RevocationReason::PrivilegeWithdrawn),
        _ => Some(RevocationReason::Unspecified),
    }
}

#[command]
pub fn crypto_ca_revoke(app: AppHandle, ca_id: String, serial: u64, reason: String) -> Result<(), String> {
    let mut cas = read_cas(&app)?;
    let ca = cas.iter_mut().find(|c| c.id == ca_id).ok_or("CA 不存在")?;
    if !ca.revoked.iter().any(|r| r.serial == serial) {
        ca.revoked.push(RevokedRec { serial, reason, at: now_str() });
    }
    write_cas(&app, &cas)
}

/// 生成该 CA 的 CRL（PEM），crl_number 自增。SM2/PQC 的 CRL 走引擎。
#[command]
pub fn crypto_ca_gen_crl(app: AppHandle, ca_id: String) -> Result<String, String> {
    let mut cas = read_cas(&app)?;
    let idx = cas.iter().position(|c| c.id == ca_id).ok_or("CA 不存在")?;
    let cat = ca_category(&cas[idx]);
    let crl_number = cas[idx].crl_number.max(1);
    let ca = cas[idx].clone();

    let pem = if cat == "sm2" || cat == "pqc" {
        engine_gen_crl(&ca, crl_number)?
    } else {
        let (issuer_kp, issuer_cert) = ca_issuer(&ca)?;
        let now = OffsetDateTime::now_utc();
        let revoked_certs = ca
            .revoked
            .iter()
            .map(|r| RevokedCertParams {
                serial_number: SerialNumber::from(r.serial),
                revocation_time: now,
                reason_code: revocation_reason(&r.reason),
                invalidity_date: None,
            })
            .collect();
        let params = CertificateRevocationListParams {
            this_update: now,
            next_update: now + Duration::days(30),
            crl_number: SerialNumber::from(crl_number),
            issuing_distribution_point: None,
            revoked_certs,
            key_identifier_method: KeyIdMethod::Sha256,
        };
        params
            .signed_by(&issuer_cert, &issuer_kp)
            .map_err(|e| format!("CRL 生成失败: {e}"))?
            .pem()
            .map_err(|e| format!("CRL 编码失败: {e}"))?
    };

    cas[idx].crl_number = crl_number + 1;
    write_cas(&app, &cas)?;
    Ok(pem)
}

// ============================ SM2/PQC 引擎挂钩（feature=tlcp，默认未启用即报错） ============================

#[cfg(not(feature = "tlcp"))]
fn engine_ca_create(
    _cat: &str,
    _subject: &Subject,
    _days: i64,
    _parent: Option<&CaEntry>,
) -> Result<(String, String), String> {
    Err("SM2/PQC 证书需启用 `tlcp` 特性并安装 Tongsuo（见 docs/remote-terminal-workshop-design.md §5.3）".into())
}
#[cfg(not(feature = "tlcp"))]
fn engine_issue_leaf(
    _ca: &CaEntry,
    _subject: &Subject,
    _profile: &str,
    _sans: &[String],
    _days: i64,
    _serial: u64,
) -> Result<(String, String), String> {
    Err("SM2/PQC 签发需启用 `tlcp` 特性并安装 Tongsuo".into())
}
#[cfg(not(feature = "tlcp"))]
fn engine_gen_crl(_ca: &CaEntry, _crl_number: u64) -> Result<String, String> {
    Err("SM2/PQC CRL 需启用 `tlcp` 特性并安装 Tongsuo".into())
}

#[cfg(feature = "tlcp")]
fn engine_ca_create(
    cat: &str,
    subject: &Subject,
    days: i64,
    parent: Option<&CaEntry>,
) -> Result<(String, String), String> {
    crate::commands::crypto_engine::ca_create(cat, subject, days, parent)
}
#[cfg(feature = "tlcp")]
fn engine_issue_leaf(
    ca: &CaEntry,
    subject: &Subject,
    profile: &str,
    sans: &[String],
    days: i64,
    serial: u64,
) -> Result<(String, String), String> {
    crate::commands::crypto_engine::issue_leaf(ca, subject, profile, sans, days, serial)
}
#[cfg(feature = "tlcp")]
fn engine_gen_crl(ca: &CaEntry, crl_number: u64) -> Result<String, String> {
    crate::commands::crypto_engine::gen_crl(ca, crl_number)
}

// ============================ 受管资产库：私钥 / CSR / CRL（asset_store.json） ============================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: String,
    /// key | csr | crl
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub algo: String,
    pub pem: String,
    /// key 的公钥 PEM（可空）。
    #[serde(default)]
    pub pem2: String,
    #[serde(default)]
    pub created: String,
}

#[derive(Serialize)]
pub struct AssetSummary {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub algo: String,
    pub created: String,
}

fn asset_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("取应用数据目录失败: {e}"))?
        .join("crypto_lab");
    std::fs::create_dir_all(&dir).map_err(|e| format!("建目录失败: {e}"))?;
    Ok(dir.join("asset_store.json"))
}
fn read_assets(app: &AppHandle) -> Result<Vec<AssetEntry>, String> {
    let p = asset_store_path(app)?;
    if !p.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| e.to_string())?)
        .map_err(|e| format!("解析资产库失败: {e}"))
}
fn write_assets(app: &AppHandle, v: &[AssetEntry]) -> Result<(), String> {
    let p = asset_store_path(app)?;
    std::fs::write(&p, serde_json::to_string_pretty(v).map_err(|e| e.to_string())?)
        .map_err(|e| format!("写资产库失败: {e}"))
}

/// 列出资产；kind 为空=全部，否则按 key|csr|crl 过滤。
#[command]
pub fn crypto_asset_list(app: AppHandle, kind: Option<String>) -> Result<Vec<AssetSummary>, String> {
    let want = kind.unwrap_or_default();
    Ok(read_assets(&app)?
        .into_iter()
        .filter(|a| want.is_empty() || a.kind == want)
        .map(|a| AssetSummary { id: a.id, kind: a.kind, name: a.name, algo: a.algo, created: a.created })
        .collect())
}

/// 保存/导入一项资产（key/csr/crl）。生成或导入都走这里。
#[command]
pub fn crypto_asset_save(
    app: AppHandle,
    kind: String,
    name: String,
    algo: Option<String>,
    pem: String,
    pem2: Option<String>,
) -> Result<AssetSummary, String> {
    if pem.trim().is_empty() {
        return Err("内容为空".into());
    }
    let mut v = read_assets(&app)?;
    let entry = AssetEntry {
        id: Uuid::new_v4().to_string(),
        kind,
        name,
        algo: algo.unwrap_or_default(),
        pem,
        pem2: pem2.unwrap_or_default(),
        created: now_str(),
    };
    let summary = AssetSummary {
        id: entry.id.clone(),
        kind: entry.kind.clone(),
        name: entry.name.clone(),
        algo: entry.algo.clone(),
        created: entry.created.clone(),
    };
    v.push(entry);
    write_assets(&app, &v)?;
    Ok(summary)
}

#[derive(Serialize)]
pub struct AssetFull {
    pub kind: String,
    pub name: String,
    pub algo: String,
    pub pem: String,
    pub pem2: String,
}

#[command]
pub fn crypto_asset_get(app: AppHandle, id: String) -> Result<AssetFull, String> {
    let v = read_assets(&app)?;
    let a = v.iter().find(|x| x.id == id).ok_or("资产不存在")?;
    Ok(AssetFull {
        kind: a.kind.clone(),
        name: a.name.clone(),
        algo: a.algo.clone(),
        pem: a.pem.clone(),
        pem2: a.pem2.clone(),
    })
}

#[command]
pub fn crypto_asset_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut v = read_assets(&app)?;
    v.retain(|a| a.id != id);
    write_assets(&app, &v)
}




