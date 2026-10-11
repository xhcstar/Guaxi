//! SM2 / TLCP / PQC 引擎 —— 仅在 cargo feature `tlcp` 下编译，绑定 Tongsuo(铜锁)。
//!
//! 构建：安装 Tongsuo 8.5+ 后 `OPENSSL_DIR=/opt/tongsuo85 OPENSSL_STATIC=0 cargo build --features tlcp`。
//! 不开 vendored，`openssl`/`openssl-sys` 直接绑定你的 Tongsuo（ABI 与 OpenSSL 3.x 兼容）。
//!
//! 本模块给 crypto_lab 的 SM2/PQC 分类库提供证书签发能力；TLCP 握手入口见 [`ntls`] 垫片。
//! 注意：此文件在本仓库的默认（纯 Rust）构建中**不参与编译**，需在装有 Tongsuo 的环境验证。

use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName};
use openssl::x509::{X509, X509NameBuilder};

use super::crypto_lab::{CaEntry, Subject};

fn sm2_key() -> Result<PKey<Private>, String> {
    let group = EcGroup::from_curve_name(Nid::SM2).map_err(|e| format!("SM2 组失败: {e}"))?;
    let ec = EcKey::generate(&group).map_err(|e| format!("SM2 生成失败: {e}"))?;
    PKey::from_ec_key(ec).map_err(|e| format!("SM2 PKey 失败: {e}"))
}

fn name_of(subject: &Subject) -> Result<openssl::x509::X509Name, String> {
    let mut nb = X509NameBuilder::new().map_err(|e| e.to_string())?;
    let push = |nb: &mut X509NameBuilder, k: &str, v: &Option<String>| {
        if let Some(x) = v.as_ref().filter(|x| !x.trim().is_empty()) {
            let _ = nb.append_entry_by_text(k, x);
        }
    };
    push(&mut nb, "C", &subject.country);
    push(&mut nb, "ST", &subject.state);
    push(&mut nb, "L", &subject.locality);
    push(&mut nb, "O", &subject.org);
    push(&mut nb, "OU", &subject.org_unit);
    nb.append_entry_by_text("CN", &subject.common_name).map_err(|e| e.to_string())?;
    Ok(nb.build())
}

fn serial() -> Result<openssl::asn1::Asn1Integer, String> {
    let mut bn = BigNum::new().map_err(|e| e.to_string())?;
    bn.rand(128, MsbOption::MAYBE_ZERO, false).map_err(|e| e.to_string())?;
    bn.to_asn1_integer().map_err(|e| e.to_string())
}

// __ENGINE_BODY__

fn build_sm2_cert(
    subject: &Subject,
    days: i64,
    key: &PKey<Private>,
    is_ca: bool,
    eku_server: Option<bool>,
    sans: &[String],
    issuer: Option<(&X509, &PKey<Private>)>,
) -> Result<X509, String> {
    let mut b = X509::builder().map_err(|e| e.to_string())?;
    b.set_version(2).map_err(|e| e.to_string())?;
    b.set_serial_number(&serial()?).map_err(|e| e.to_string())?;
    let subj_name = name_of(subject)?;
    b.set_subject_name(&subj_name).map_err(|e| e.to_string())?;
    match issuer {
        Some((ca, _)) => b.set_issuer_name(ca.subject_name()).map_err(|e| e.to_string())?,
        None => b.set_issuer_name(&subj_name).map_err(|e| e.to_string())?,
    }
    b.set_pubkey(key).map_err(|e| e.to_string())?;
    b.set_not_before(&Asn1Time::days_from_now(0).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    b.set_not_after(&Asn1Time::days_from_now(days.max(1) as u32).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if is_ca {
        b.append_extension(BasicConstraints::new().critical().ca().build().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        b.append_extension(KeyUsage::new().critical().key_cert_sign().crl_sign().digital_signature().build().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    } else {
        b.append_extension(BasicConstraints::new().build().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        b.append_extension(KeyUsage::new().critical().digital_signature().key_encipherment().build().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if let Some(server) = eku_server {
            let mut eku = ExtendedKeyUsage::new();
            if server { eku.server_auth(); } else { eku.client_auth(); }
            b.append_extension(eku.build().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        }
        if !sans.is_empty() {
            let ctx = b.x509v3_context(issuer.map(|(c, _)| c), None);
            let mut san = SubjectAlternativeName::new();
            for s in sans.iter().filter(|s| !s.trim().is_empty()) {
                if s.parse::<std::net::IpAddr>().is_ok() { san.ip(s); } else { san.dns(s); }
            }
            let ext = san.build(&ctx).map_err(|e| e.to_string())?;
            b.append_extension(ext).map_err(|e| e.to_string())?;
        }
    }
    // SM2 签名用 SM3 摘要；自签用自身 key，否则用 issuer key。
    let sign_key = issuer.map(|(_, k)| k).unwrap_or(key);
    b.sign(sign_key, MessageDigest::sm3()).map_err(|e| format!("SM2 签名失败: {e}"))?;
    Ok(b.build())
}

fn pem_pair(cert: &X509, key: &PKey<Private>) -> Result<(String, String), String> {
    let cert_pem = String::from_utf8(cert.to_pem().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let key_pem = String::from_utf8(key.private_key_to_pem_pkcs8().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok((cert_pem, key_pem))
}

fn load_issuer(ca: &CaEntry) -> Result<(X509, PKey<Private>), String> {
    let cert = X509::from_pem(ca.cert_pem.as_bytes()).map_err(|e| format!("解析 CA 证书失败: {e}"))?;
    let key = PKey::private_key_from_pem(ca.key_pem.as_bytes()).map_err(|e| format!("解析 CA 私钥失败: {e}"))?;
    Ok((cert, key))
}

pub fn ca_create(cat: &str, subject: &Subject, days: i64, parent: Option<&CaEntry>) -> Result<(String, String), String> {
    if cat == "pqc" {
        return Err("PQC(ML-DSA) 证书签发需用 Tongsuo EVP 原生接口，当前引擎暂以 SM2 为主，PQC 待在 Tongsuo 环境完善".into());
    }
    let key = sm2_key()?;
    let cert = match parent {
        Some(p) => {
            let (ic, ik) = load_issuer(p)?;
            build_sm2_cert(subject, days, &key, true, None, &[], Some((&ic, &ik)))?
        }
        None => build_sm2_cert(subject, days, &key, true, None, &[], None)?,
    };
    pem_pair(&cert, &key)
}

pub fn issue_leaf(ca: &CaEntry, subject: &Subject, profile: &str, sans: &[String], days: i64, _serial: u64) -> Result<(String, String), String> {
    let key = sm2_key()?;
    let (ic, ik) = load_issuer(ca)?;
    let cert = build_sm2_cert(subject, days, &key, false, Some(profile == "server"), sans, Some((&ic, &ik)))?;
    pem_pair(&cert, &key)
}

pub fn gen_crl(_ca: &CaEntry, _crl_number: u64) -> Result<String, String> {
    // openssl crate 0.10 的安全 API 不含 CRL 构造；需用 openssl-sys FFI(X509_CRL_*)实现，待 Tongsuo 环境完善。
    Err("SM2/PQC 的 CRL 生成待在 Tongsuo 环境用 X509_CRL FFI 完善".into())
}

/// TLCP(国密双证书) 握手入口垫片——Tongsuo 独有、不在 openssl crate 安全封装内。
/// 函数签名须与所链 Tongsuo 版本头文件对齐（跨小版本可能改名，见设计文档 §5.3 风险）。
pub mod ntls {
    use std::os::raw::{c_int, c_void};
    extern "C" {
        pub fn NTLS_client_method() -> *const c_void;
        pub fn NTLS_server_method() -> *const c_void;
        pub fn SSL_CTX_enable_ntls(ctx: *mut c_void);
        pub fn SSL_CTX_use_sign_certificate_file(ctx: *mut c_void, file: *const i8, typ: c_int) -> c_int;
        pub fn SSL_CTX_use_enc_certificate_file(ctx: *mut c_void, file: *const i8, typ: c_int) -> c_int;
        pub fn SSL_CTX_use_sign_PrivateKey_file(ctx: *mut c_void, file: *const i8, typ: c_int) -> c_int;
        pub fn SSL_CTX_use_enc_PrivateKey_file(ctx: *mut c_void, file: *const i8, typ: c_int) -> c_int;
    }
    // TLCP 客户端握手/探测在此之上实现：构造 SslContextBuilder→enable_ntls→加载双证书→
    // set_cipher_list("ECC-SM2-SM4-CBC-SM3")→set_groups_list("SM2MLKEM768:X25519MLKEM768:...")。
    // 具体连接复用 tokio-openssl fork（见设计文档 §5.3 第六步）。
}

