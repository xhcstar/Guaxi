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
    subject: Option<Subject>,
) -> Result<String, String> {
    let kp = KeyPair::from_pem(&key_pem).map_err(|e| format!("载入私钥失败: {e}"))?;
    let has_subject = subject.is_some();
    let mut params = match subject {
        Some(s) => {
            let mut p = CertificateParams::default();
            p.distinguished_name = dn_from_subject(&s);
            p
        }
        None => build_params(&common_name, org, &sans, "server", 825)?,
    };
    if has_subject {
        push_sans(&mut params, &sans)?;
        apply_profile(&mut params, "server", 825);
    }
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
#[allow(clippy::too_many_arguments)]
pub fn crypto_sym(
    algo: String,
    mode: String,
    key_hex: String,
    iv_hex: String,
    data_b64: String,
    encrypt: bool,
    aad_hex: Option<String>,
    padding: Option<bool>,
) -> Result<String, String> {
    let key = hex::decode(key_hex.trim()).map_err(|e| format!("key 非法 hex: {e}"))?;
    let iv = hex::decode(iv_hex.trim()).map_err(|e| format!("iv/nonce 非法 hex: {e}"))?;
    let data = b64_dec(&data_b64)?;
    let aad = match aad_hex {
        Some(a) if !a.trim().is_empty() => hex::decode(a.trim()).map_err(|e| format!("aad 非法 hex: {e}"))?,
        _ => Vec::new(),
    };
    // 块密码通用分发：$c = 密码类型，$ctr = 对应位宽的 CTR 包装，$bs = 分组字节数。
    // 支持模式：ecb / cbc / cfb（要求块对齐，可 PKCS7 填充）；ofb / ctr（流式，无填充）。
    macro_rules! blk {
        ($c:ty, $ctr:ty, $bs:expr) => {{
            use cipher::{
                block_padding::{NoPadding, Pkcs7},
                BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, StreamCipher,
            };
            let pad = padding.unwrap_or(true);
            let aligned = data.len() % $bs == 0;
            match mode.as_str() {
                "ecb" => {
                    if pad {
                        if encrypt {
                            let e = <ecb::Encryptor<$c>>::new_from_slice(&key)
                                .map_err(|e| format!("密钥长度错: {e}"))?;
                            Ok(e.encrypt_padded_vec_mut::<Pkcs7>(&data))
                        } else {
                            let d = <ecb::Decryptor<$c>>::new_from_slice(&key)
                                .map_err(|e| format!("密钥长度错: {e}"))?;
                            d.decrypt_padded_vec_mut::<Pkcs7>(&data).map_err(|e| format!("解密失败: {e}"))
                        }
                    } else {
                        if !aligned {
                            return Err(format!("ECB 无填充要求输入为 {} 字节的整数倍（当前 {}）", $bs, data.len()));
                        }
                        if encrypt {
                            let e = <ecb::Encryptor<$c>>::new_from_slice(&key)
                                .map_err(|e| format!("密钥长度错: {e}"))?;
                            Ok(e.encrypt_padded_vec_mut::<NoPadding>(&data))
                        } else {
                            let d = <ecb::Decryptor<$c>>::new_from_slice(&key)
                                .map_err(|e| format!("密钥长度错: {e}"))?;
                            d.decrypt_padded_vec_mut::<NoPadding>(&data).map_err(|e| format!("解密失败: {e}"))
                        }
                    }
                }
                "cbc" => {
                    if pad {
                        if encrypt {
                            let e = <cbc::Encryptor<$c>>::new_from_slices(&key, &iv)
                                .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                            Ok(e.encrypt_padded_vec_mut::<Pkcs7>(&data))
                        } else {
                            let d = <cbc::Decryptor<$c>>::new_from_slices(&key, &iv)
                                .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                            d.decrypt_padded_vec_mut::<Pkcs7>(&data).map_err(|e| format!("解密失败: {e}"))
                        }
                    } else {
                        if !aligned {
                            return Err(format!("CBC 无填充要求输入为 {} 字节的整数倍（当前 {}）", $bs, data.len()));
                        }
                        if encrypt {
                            let e = <cbc::Encryptor<$c>>::new_from_slices(&key, &iv)
                                .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                            Ok(e.encrypt_padded_vec_mut::<NoPadding>(&data))
                        } else {
                            let d = <cbc::Decryptor<$c>>::new_from_slices(&key, &iv)
                                .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                            d.decrypt_padded_vec_mut::<NoPadding>(&data).map_err(|e| format!("解密失败: {e}"))
                        }
                    }
                }
                "cfb" => {
                    if !aligned {
                        return Err(format!("CFB 要求输入为 {} 字节的整数倍（当前 {}）", $bs, data.len()));
                    }
                    if encrypt {
                        let e = <cfb_mode::Encryptor<$c>>::new_from_slices(&key, &iv)
                            .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                        Ok(e.encrypt_padded_vec_mut::<NoPadding>(&data))
                    } else {
                        let d = <cfb_mode::Decryptor<$c>>::new_from_slices(&key, &iv)
                            .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                        d.decrypt_padded_vec_mut::<NoPadding>(&data).map_err(|e| format!("解密失败: {e}"))
                    }
                }
                "ofb" => {
                    let mut buf = data.clone();
                    let mut c = <ofb::Ofb<$c>>::new_from_slices(&key, &iv)
                        .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                    c.apply_keystream(&mut buf);
                    Ok(buf)
                }
                "ctr" => {
                    let mut buf = data.clone();
                    let mut c = <$ctr>::new_from_slices(&key, &iv)
                        .map_err(|e| format!("密钥/IV 长度错: {e}"))?;
                    c.apply_keystream(&mut buf);
                    Ok(buf)
                }
                m => Err(format!("该算法不支持模式 {m}（支持 ecb/cbc/cfb/ofb/ctr）")),
            }
        }};
    }

    let out = match algo.as_str() {
        "aes128" => blk!(aes::Aes128, ctr::Ctr128BE<aes::Aes128>, 16),
        "aes192" => blk!(aes::Aes192, ctr::Ctr128BE<aes::Aes192>, 16),
        "aes256" => blk!(aes::Aes256, ctr::Ctr128BE<aes::Aes256>, 16),
        // 旧值 "aes"：按密钥长度自动选 128/192/256
        "aes" => match key.len() {
            16 => blk!(aes::Aes128, ctr::Ctr128BE<aes::Aes128>, 16),
            24 => blk!(aes::Aes192, ctr::Ctr128BE<aes::Aes192>, 16),
            32 => blk!(aes::Aes256, ctr::Ctr128BE<aes::Aes256>, 16),
            n => return Err(format!("AES 密钥需 16/24/32 字节，收到 {n}")),
        },
        "sm4" => blk!(sm4::Sm4, ctr::Ctr128BE<sm4::Sm4>, 16),
        "camellia128" => blk!(camellia::Camellia128, ctr::Ctr128BE<camellia::Camellia128>, 16),
        "camellia192" => blk!(camellia::Camellia192, ctr::Ctr128BE<camellia::Camellia192>, 16),
        "camellia256" => blk!(camellia::Camellia256, ctr::Ctr128BE<camellia::Camellia256>, 16),
        "twofish" => blk!(twofish::Twofish, ctr::Ctr128BE<twofish::Twofish>, 16),
        "serpent" => blk!(serpent::Serpent, ctr::Ctr128BE<serpent::Serpent>, 16),
        // ---- 64 位分组的经典密码 ----
        "des" => blk!(des::Des, ctr::Ctr64BE<des::Des>, 8),
        "3des" => blk!(des::TdesEde3, ctr::Ctr64BE<des::TdesEde3>, 8),
        "blowfish" => blk!(blowfish::Blowfish, ctr::Ctr64BE<blowfish::Blowfish>, 8),
        "cast5" => blk!(cast5::Cast5, ctr::Ctr64BE<cast5::Cast5>, 8),
        "rc2" => blk!(rc2::Rc2, ctr::Ctr64BE<rc2::Rc2>, 8),
        "idea" => blk!(idea::Idea, ctr::Ctr64BE<idea::Idea>, 8),
        // ---- 流密码 ----
        "chacha20" => {
            use chacha20::cipher::{KeyIvInit, StreamCipher};
            let mut buf = data.clone();
            let mut c = chacha20::ChaCha20::new_from_slices(&key, &iv)
                .map_err(|e| format!("密钥/nonce 长度错: {e}"))?;
            c.apply_keystream(&mut buf);
            Ok(buf)
        }
        "salsa20" => {
            use salsa20::cipher::{KeyIvInit, StreamCipher};
            let mut buf = data.clone();
            let mut c = salsa20::Salsa20::new_from_slices(&key, &iv)
                .map_err(|e| format!("密钥/nonce 长度错: {e}"))?;
            c.apply_keystream(&mut buf);
            Ok(buf)
        }
        // ---- AEAD（加密输出 = 密文 ‖ 认证标签；解密需 aad 一致）----
        "aes128gcm" | "aes256gcm" | "aesgcm" => {
            use aes_gcm::aead::{Aead, KeyInit, Payload};
            use aes_gcm::{Aes128Gcm, Aes256Gcm, Nonce};
            let n = Nonce::from_slice(&iv);
            let pl = Payload { msg: &data, aad: &aad };
            macro_rules! gcm {
                ($c:ty) => {{
                    let c = <$c>::new_from_slice(&key).map_err(|e| format!("密钥长度错: {e}"))?;
                    if encrypt { c.encrypt(n, pl) } else { c.decrypt(n, pl) }
                        .map_err(|e| format!("{}失败（密钥/nonce/AAD 是否一致？）: {e}", if encrypt { "加密" } else { "解密" }))
                }};
            }
            match (algo.as_str(), key.len()) {
                ("aes128gcm", 16) | ("aesgcm", 16) => gcm!(Aes128Gcm),
                ("aes256gcm", 32) | ("aesgcm", 32) => gcm!(Aes256Gcm),
                ("aesgcm", n) => return Err(format!("AES-GCM 密钥需 16/32 字节，收到 {n}")),
                (a, n) => return Err(format!("{a} 密钥长度应为 {n} 字节")),
            }
        }
        "chacha20poly1305" => chacha(&key, &iv, &data, &aad, encrypt),
        a => return Err(format!("不支持的算法: {a}")),
    };
    Ok(b64_enc(&out?))
}

/// ChaCha20-Poly1305（AEAD）：nonce 12 字节，可选 AAD；加密输出为 密文‖16 字节标签。
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
        c.decrypt(n, pl).map_err(|e| format!("解密失败（密钥/nonce/AAD 是否一致？）: {e}"))
    }
}

// ============================ 非对称加解密 / 签名 / 验签 / 散列 ============================

use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
// FromDer 提供 X509CertificationRequest / CertificateRevocationList 的 from_der，
// 需要在模块级引入（guess_der_tag 与命令体内都会用到）。
use x509_parser::prelude::FromDer;

fn load_rsa_public(pem: &str) -> Result<RsaPublicKey, String> {
    if let Ok(pk) = RsaPublicKey::from_public_key_pem(pem) {
        return Ok(pk);
    }
    let sk = RsaPrivateKey::from_pkcs8_pem(pem).map_err(|e| format!("载入 RSA 公钥/私钥失败: {e}"))?;
    Ok(RsaPublicKey::from(&sk))
}

/// RSA 加解密。scheme：pkcs1（PKCS#1 v1.5）|oaep-sha1|oaep-sha256|oaep-sha384|oaep-sha512。
#[command]
pub fn crypto_rsa_crypt(
    op: String,
    key_pem: String,
    data_b64: String,
    scheme: String,
) -> Result<String, String> {
    use rsa::{Oaep, Pkcs1v15Encrypt};
    let data = b64_dec(&data_b64)?;
    // 按方案构造填充；用闭包把 pk/sk 的分支收敛成一次 match。
    if op == "encrypt" {
        let pk = load_rsa_public(&key_pem)?;
        let mut rng = rand08::rngs::OsRng;
        let ct = match scheme.as_str() {
            "pkcs1" => pk.encrypt(&mut rng, Pkcs1v15Encrypt, &data),
            "oaep-sha1" => pk.encrypt(&mut rng, Oaep::new::<sha1::Sha1>(), &data),
            "oaep-sha256" => pk.encrypt(&mut rng, Oaep::new::<sha2::Sha256>(), &data),
            "oaep-sha384" => pk.encrypt(&mut rng, Oaep::new::<sha2::Sha384>(), &data),
            "oaep-sha512" => pk.encrypt(&mut rng, Oaep::new::<sha2::Sha512>(), &data),
            s => return Err(format!("不支持的 RSA 加密方案: {s}")),
        }
        .map_err(|e| format!("RSA 加密失败（明文过长或密钥太小？）: {e}"))?;
        Ok(b64_enc(&ct))
    } else {
        let sk = RsaPrivateKey::from_pkcs8_pem(&key_pem).map_err(|e| format!("载入私钥失败: {e}"))?;
        let pt = match scheme.as_str() {
            "pkcs1" => sk.decrypt(Pkcs1v15Encrypt, &data),
            "oaep-sha1" => sk.decrypt(Oaep::new::<sha1::Sha1>(), &data),
            "oaep-sha256" => sk.decrypt(Oaep::new::<sha2::Sha256>(), &data),
            "oaep-sha384" => sk.decrypt(Oaep::new::<sha2::Sha384>(), &data),
            "oaep-sha512" => sk.decrypt(Oaep::new::<sha2::Sha512>(), &data),
            s => return Err(format!("不支持的 RSA 解密方案: {s}")),
        }
        .map_err(|e| format!("RSA 解密失败: {e}"))?;
        Ok(b64_enc(&pt))
    }
}

/// RSA 签名方案：rsa-pkcs1-sha256|sha384|sha512、rsa-pss-sha256|sha384|sha512。
/// 用宏按摘要展开，避免六份重复代码。
#[command]
pub fn crypto_sign(key_pem: String, data_b64: String, scheme: String) -> Result<String, String> {
    let data = b64_dec(&data_b64)?;
    macro_rules! pkcs1 {
        ($d:ty) => {{
            use rsa::signature::{SignatureEncoding, Signer};
            let sk = RsaPrivateKey::from_pkcs8_pem(&key_pem).map_err(|e| e.to_string())?;
            rsa::pkcs1v15::SigningKey::<$d>::new(sk).sign(&data).to_vec()
        }};
    }
    macro_rules! pss {
        ($d:ty) => {{
            use rsa::signature::{RandomizedSigner, SignatureEncoding};
            let sk = RsaPrivateKey::from_pkcs8_pem(&key_pem).map_err(|e| e.to_string())?;
            let mut rng = rand08::rngs::OsRng;
            rsa::pss::SigningKey::<$d>::new(sk).sign_with_rng(&mut rng, &data).to_vec()
        }};
    }
    let sig: Vec<u8> = match scheme.as_str() {
        "rsa-pkcs1-sha256" => pkcs1!(sha2::Sha256),
        "rsa-pkcs1-sha384" => pkcs1!(sha2::Sha384),
        "rsa-pkcs1-sha512" => pkcs1!(sha2::Sha512),
        "rsa-pss-sha256" => pss!(sha2::Sha256),
        "rsa-pss-sha384" => pss!(sha2::Sha384),
        "rsa-pss-sha512" => pss!(sha2::Sha512),
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
    macro_rules! pkcs1_v {
        ($d:ty) => {{
            use rsa::signature::Verifier;
            let pk = load_rsa_public(&pub_pem)?;
            let vk = rsa::pkcs1v15::VerifyingKey::<$d>::new(pk);
            match rsa::pkcs1v15::Signature::try_from(sig.as_slice()) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }};
    }
    macro_rules! pss_v {
        ($d:ty) => {{
            use rsa::signature::Verifier;
            let pk = load_rsa_public(&pub_pem)?;
            let vk = rsa::pss::VerifyingKey::<$d>::new(pk);
            match rsa::pss::Signature::try_from(sig.as_slice()) {
                Ok(s) => vk.verify(&data, &s).is_ok(),
                Err(_) => false,
            }
        }};
    }
    let ok = match scheme.as_str() {
        "rsa-pkcs1-sha256" => pkcs1_v!(sha2::Sha256),
        "rsa-pkcs1-sha384" => pkcs1_v!(sha2::Sha384),
        "rsa-pkcs1-sha512" => pkcs1_v!(sha2::Sha512),
        "rsa-pss-sha256" => pss_v!(sha2::Sha256),
        "rsa-pss-sha384" => pss_v!(sha2::Sha384),
        "rsa-pss-sha512" => pss_v!(sha2::Sha512),
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
    // rcgen 0.13 只内置了五个常用 DN 类型，其余用标准 OID 作为 CustomDnType。
    let extras: [(&[u64], &Option<String>); 12] = [
        (&[1, 2, 840, 113549, 1, 9, 1], &s.email),
        (&[2, 5, 4, 5], &s.serial_number),
        (&[2, 5, 4, 9], &s.street),
        (&[2, 5, 4, 17], &s.postal_code),
        (&[2, 5, 4, 12], &s.title),
        (&[2, 5, 4, 42], &s.given_name),
        (&[2, 5, 4, 4], &s.surname),
        (&[2, 5, 4, 43], &s.initials),
        (&[2, 5, 4, 65], &s.pseudonym),
        (&[2, 5, 4, 44], &s.generation_qualifier),
        (&[2, 5, 4, 46], &s.dn_qualifier),
        (&[2, 5, 4, 97], &s.organization_identifier),
    ];
    for (oid, v) in extras {
        if let Some(x) = v.as_ref().filter(|x| !x.trim().is_empty()) {
            dn.push(DnType::CustomDnType(oid.to_vec()), x.clone());
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

/// 把远端 Tongsuo 生成的 CA 直接入库（不做 rcgen 解析，SM2/PQC 也能存）。
#[command]
pub fn crypto_ca_import_pem(
    app: AppHandle,
    name: String,
    category: String,
    algo: Option<String>,
    cert_pem: String,
    key_pem: String,
    subject_cn: Option<String>,
    parent_id: Option<String>,
) -> Result<CaSummary, String> {
    if name.trim().is_empty() {
        return Err("CA 名称不能为空".into());
    }
    if !cert_pem.contains("-----BEGIN ") || !key_pem.contains("-----BEGIN ") {
        return Err("CA 证书或私钥不是 PEM 内容".into());
    }
    let parsed_cn = match pem_to_der_of(&cert_pem, "CERTIFICATE") {
        Ok(d) => x509_parser::parse_x509_certificate(&d)
            .ok()
            .map(|(_, c)| c.subject().to_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    };
    let subject_cn = subject_cn
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| if parsed_cn.is_empty() { name.clone() } else { parsed_cn });
    let mut cas = read_cas(&app)?;
    let entry = CaEntry {
        id: Uuid::new_v4().to_string(),
        name,
        algo: algo.unwrap_or_else(|| category.clone()),
        subject_cn,
        cert_pem,
        key_pem,
        is_root: parent_id.is_none(),
        parent_id,
        category,
        serial_counter: 2,
        crl_number: 1,
        revoked: Vec::new(),
    };
    let summary = ca_summary(&entry);
    cas.push(entry);
    write_cas(&app, &cas)?;
    Ok(summary)
}

fn cert_serial_from_pem(pem_text: &str) -> u64 {
    match pem_to_der_of(pem_text, "CERTIFICATE") {
        Ok(d) => x509_parser::parse_x509_certificate(&d)
            .ok()
            .map(|(_, c)| {
                c.raw_serial()
                    .iter()
                    .fold(0u64, |acc, b| acc.wrapping_shl(8) | *b as u64)
            })
            .unwrap_or(0),
        Err(_) => 0,
    }
}

/// 把远端 Tongsuo 签发的终端证书入库。
#[command]
pub fn crypto_leaf_import_pem(
    app: AppHandle,
    name: String,
    kind: String,
    ca_id: String,
    cert_pem: String,
    key_pem: String,
    subject_cn: Option<String>,
) -> Result<LeafSummary, String> {
    if !cert_pem.contains("-----BEGIN ") || !key_pem.contains("-----BEGIN ") {
        return Err("终端证书或私钥不是 PEM 内容".into());
    }
    let cas = read_cas(&app)?;
    let ca = cas.iter().find(|c| c.id == ca_id).ok_or("签发 CA 不存在")?;
    let parsed_cn = match pem_to_der_of(&cert_pem, "CERTIFICATE") {
        Ok(d) => x509_parser::parse_x509_certificate(&d)
            .ok()
            .map(|(_, c)| c.subject().to_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    };
    let subject_cn = subject_cn
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(parsed_cn);
    let entry = LeafEntry {
        id: Uuid::new_v4().to_string(),
        name,
        kind,
        ca_id: ca.id.clone(),
        subject_cn,
        serial: cert_serial_from_pem(&cert_pem),
        cert_pem,
        key_pem,
        created: now_str(),
    };
    let summary = LeafSummary {
        id: entry.id.clone(),
        name: entry.name.clone(),
        kind: entry.kind.clone(),
        ca_id: entry.ca_id.clone(),
        ca_name: ca.name.clone(),
        subject_cn: entry.subject_cn.clone(),
        serial: entry.serial,
        created: entry.created.clone(),
    };
    let mut leaves = read_leaves(&app)?;
    leaves.push(entry);
    write_leaves(&app, &leaves)?;
    Ok(summary)
}

#[command]
pub fn crypto_ca_delete(app: AppHandle, id: String) -> Result<(), String> {    let mut cas = read_cas(&app)?;
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

// ============================ 证书/私钥/CSR/CRL 详情（查看弹窗用）============================

#[derive(Serialize)]
pub struct PemDetails {
    /// certificate | private_key | public_key | csr | crl | unknown
    pub kind: String,
    /// 归一化后的 PEM（供展示与复制）
    pub pem: String,
    /// DER 的 base64（供展示与复制）
    pub der_b64: String,
    /// DER 的 SHA-256 指纹（冒号分隔大写十六进制，便于与 openssl 对照）
    pub fingerprint: String,
    pub fields: Vec<(String, String)>,
    pub sans: Vec<String>,
}

/// 把 PEM / base64 / hex 输入统一成 (标签, DER, PEM 文本)。
fn normalize_input(input: &str) -> Result<(String, Vec<u8>, String), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("输入为空".into());
    }
    if trimmed.contains("-----BEGIN") {
        let block = pem::parse(trimmed.as_bytes()).map_err(|e| format!("PEM 解析失败: {e}"))?;
        let tag = block.tag().to_string();
        let der = block.contents().to_vec();
        // 直接用原文本，避免重新编码改变换行风格。
        return Ok((tag, der, trimmed.to_string()));
    }
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let der = if let Ok(b) = b64().decode(compact.as_bytes()) {
        b
    } else if let Ok(b) = hex::decode(&compact) {
        b
    } else {
        return Err("无法识别：需要 PEM、base64 或 hex 编码的 DER".into());
    };
    // 裸 DER：按内容猜测标签并回编成 PEM
    let tag = guess_der_tag(&der);
    let pem_text = pem::encode(&pem::Pem::new(tag.clone(), der.clone()));
    Ok((tag, der, pem_text))
}

fn guess_der_tag(der: &[u8]) -> String {
    if x509_parser::parse_x509_certificate(der).is_ok() {
        "CERTIFICATE".into()
    } else if x509_parser::certification_request::X509CertificationRequest::from_der(der).is_ok() {
        "CERTIFICATE REQUEST".into()
    } else if x509_parser::revocation_list::CertificateRevocationList::from_der(der).is_ok() {
        "X509 CRL".into()
    } else if der.first() == Some(&0x30) {
        "PRIVATE KEY".into()
    } else {
        "UNKNOWN".into()
    }
}

fn fingerprint_of(der: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let d = Sha256::digest(der);
    d.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

fn san_list(cert: &x509_parser::certificate::X509Certificate) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(Some(ext)) = cert.subject_alternative_name() {
        for gn in &ext.value.general_names {
            out.push(format!("{gn}"));
        }
    }
    out
}

/// 解析一份证书/私钥/CSR/CRL，给出可读字段 + PEM + DER(base64) + 指纹。
#[command]
pub fn crypto_pem_details(input: String) -> Result<PemDetails, String> {
    use x509_parser::prelude::X509CertificationRequest;
    let (tag, der, pem_text) = normalize_input(&input)?;
    let mut fields: Vec<(String, String)> = vec![("PEM 标签".into(), tag.clone())];
    let mut sans: Vec<String> = Vec::new();
    let mut kind = "unknown".to_string();

    if tag.contains("CERTIFICATE") && !tag.contains("REQUEST") {
        match x509_parser::parse_x509_certificate(&der) {
            Ok((_, c)) => {
                kind = "certificate".into();
                fields.push(("版本".into(), format!("v{}", c.version().0 + 1)));
                fields.push(("序列号".into(), c.raw_serial_as_string()));
                fields.push(("主题".into(), c.subject().to_string()));
                fields.push(("颁发者".into(), c.issuer().to_string()));
                fields.push(("生效时间".into(), c.validity().not_before.to_string()));
                fields.push(("失效时间".into(), c.validity().not_after.to_string()));
                fields.push(("签名算法".into(), c.signature_algorithm.algorithm.to_id_string()));
                fields.push(("公钥算法".into(), c.public_key().algorithm.algorithm.to_id_string()));
                fields.push(("是否 CA".into(), if c.is_ca() { "是".into() } else { "否".to_string() }));
                fields.push(("自签".into(), if c.subject() == c.issuer() { "是".into() } else { "否".to_string() }));
                sans = san_list(&c);
            }
            Err(e) => fields.push(("证书解析".into(), format!("失败: {e}"))),
        }
    } else if tag.contains("REQUEST") {
        kind = "csr".into();
        match X509CertificationRequest::from_der(&der) {
            Ok((_, csr)) => {
                fields.push(("主题".into(), csr.certification_request_info.subject.to_string()));
                fields.push(("签名算法".into(), csr.signature_algorithm.algorithm.to_id_string()));
                if let Some(exts) = csr.requested_extensions() {
                    for e in exts {
                        if let x509_parser::extensions::ParsedExtension::SubjectAlternativeName(san) = e {
                            for gn in &san.general_names {
                                sans.push(format!("{gn}"));
                            }
                        }
                    }
                }
            }
            Err(e) => fields.push(("CSR 解析".into(), format!("失败: {e}"))),
        }
    } else if tag.contains("CRL") {
        kind = "crl".into();
        match x509_parser::revocation_list::CertificateRevocationList::from_der(&der) {
            Ok((_, crl)) => {
                fields.push(("颁发者".into(), crl.issuer().to_string()));
                fields.push(("签名算法".into(), crl.signature_algorithm.algorithm.to_id_string()));
                fields.push(("本次更新".into(), crl.last_update().to_string()));
                if let Some(n) = crl.next_update() {
                    fields.push(("下次更新".into(), n.to_string()));
                }
                fields.push(("吊销条目".into(), crl.iter_revoked_certificates().count().to_string()));
            }
            Err(e) => fields.push(("CRL 解析".into(), format!("失败: {e}"))),
        }
    } else if tag.contains("PRIVATE KEY") {
        kind = "private_key".into();
        describe_key(&pem_text, &der, &mut fields, true);
    } else if tag.contains("PUBLIC KEY") {
        kind = "public_key".into();
        describe_key(&pem_text, &der, &mut fields, false);
    }

    fields.push(("DER 长度".into(), format!("{} 字节", der.len())));
    Ok(PemDetails {
        kind,
        pem: pem_text,
        der_b64: b64_enc(&der),
        fingerprint: fingerprint_of(&der),
        fields,
        sans,
    })
}

/// 识别私钥/公钥算法与规格（RSA 位数 / EC 曲线 / Ed25519）。
fn describe_key(pem_text: &str, der: &[u8], fields: &mut Vec<(String, String)>, is_private: bool) {
    // RSA
    if is_private {
        if let Ok(sk) = RsaPrivateKey::from_pkcs8_pem(pem_text) {
            fields.push(("算法".into(), "RSA".into()));
            fields.push(("密钥长度".into(), format!("{} 位", sk.n().bits())));
            return;
        }
    } else if let Ok(pk) = RsaPublicKey::from_public_key_pem(pem_text) {
        fields.push(("算法".into(), "RSA".into()));
        fields.push(("密钥长度".into(), format!("{} 位", pk.n().bits())));
        return;
    }
    // EC（P-256 / P-384）
    if is_private {
        if p256::SecretKey::from_pkcs8_pem(pem_text).is_ok() {
            fields.push(("算法".into(), "ECDSA".into()));
            fields.push(("曲线".into(), "P-256 (prime256v1)".into()));
            return;
        }
        if p384::SecretKey::from_pkcs8_pem(pem_text).is_ok() {
            fields.push(("算法".into(), "ECDSA".into()));
            fields.push(("曲线".into(), "P-384 (secp384r1)".into()));
            return;
        }
    } else if let Ok(k) = p256::PublicKey::from_public_key_pem(pem_text) {
        let _ = k;
        fields.push(("算法".into(), "ECDSA".into()));
        fields.push(("曲线".into(), "P-256 (prime256v1)".into()));
        return;
    } else if p384::PublicKey::from_public_key_pem(pem_text).is_ok() {
        fields.push(("算法".into(), "ECDSA".into()));
        fields.push(("曲线".into(), "P-384 (secp384r1)".into()));
        return;
    }
    // Ed25519
    {
        use ed25519_dalek::pkcs8::{DecodePrivateKey, DecodePublicKey};
        if is_private {
            if ed25519_dalek::SigningKey::from_pkcs8_pem(pem_text).is_ok() {
                fields.push(("算法".into(), "Ed25519".into()));
                fields.push(("密钥长度".into(), "256 位 (EdDSA)".into()));
                return;
            }
        } else if ed25519_dalek::VerifyingKey::from_public_key_pem(pem_text).is_ok() {
            fields.push(("算法".into(), "Ed25519".into()));
            fields.push(("密钥长度".into(), "256 位 (EdDSA)".into()));
            return;
        }
    }
    fields.push(("算法".into(), format!("未识别（DER {} 字节）", der.len())));
}





