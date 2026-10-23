//! S/MIME：CMS 签名与加密（RFC 5652 / RFC 5751）。
//!
//! 为什么邮件端要有它：企业环境（尤其是对标 Outlook 的场景）通常是「S/MIME 证书
//! 由 AD / MDM 统一下发」，客户端只负责按证书签名、按收件人证书加密。
//! 这与本地 `crypto_lab` 的 TLS 证书工作台是两件事——这里不签发证书，只使用证书。
//!
//! 实现范围：
//! - **签名**：`SignedData`，SHA-256 + RSA PKCS#1 v1.5（国内企业证书绝大多数是 RSA）。
//!   这是收信端（Outlook / Exchange / Foxmail）兼容性最好的一种。
//! - **加密**：`EnvelopedData`，AES-256-CBC + RSA PKCS#1 v1.5 密钥传输
//!   （RFC 5751 要求的标准组合；ECDH 在部分旧客户端上支持并不完整）。
//! - **验签与解密**：给「打开加密邮件」用。
//!
//! 证书与私钥都存放在系统 keychain（service = `guaxi-mail-smime`），
//! 与口令同样的边界：备份数据库不会把私钥带出去。

use base64::{engine::general_purpose::STANDARD, Engine as _};
use cms::builder::{
    ContentEncryptionAlgorithm, EnvelopedDataBuilder, KeyEncryptionInfo,
    KeyTransRecipientInfoBuilder, SignedDataBuilder, SignerInfoBuilder,
};
use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::ContentInfo;
use cms::enveloped_data::{EnvelopedData, RecipientIdentifier};
use cms::signed_data::{EncapsulatedContentInfo, SignerIdentifier};
use der::asn1::OctetString;
use der::{Any, Decode, Encode, Tag};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha2::Sha256;
use spki::DecodePublicKey;
use x509_cert::Certificate;

/// keychain 里一份 S/MIME 身份的 JSON。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SmimeIdentity {
    /// 证书链（PEM，第一份是签名证书）。
    #[serde(default)]
    pub cert_pem: String,
    /// 私钥（PEM，PKCS#8 或 PKCS#1）。
    #[serde(default)]
    pub key_pem: String,
}

const SERVICE: &str = "guaxi-mail-smime";

fn entry(account_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account_id).map_err(|e| format!("访问系统凭据失败: {e}"))
}

pub fn load_identity(account_id: &str) -> Option<SmimeIdentity> {
    let raw = entry(account_id).ok()?.get_password().ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_identity(account_id: &str, identity: &SmimeIdentity) -> Result<(), String> {
    // 先验证一遍再存：存进去之后才发现解析不了，用户会以为「配置成功了」。
    parse_certificate(&identity.cert_pem)?;
    parse_private_key(&identity.key_pem)?;
    let json = serde_json::to_string(identity).map_err(|e| e.to_string())?;
    entry(account_id)?
        .set_password(&json)
        .map_err(|e| format!("写入系统凭据失败: {e}"))
}

pub fn delete_identity(account_id: &str) {
    if let Ok(e) = entry(account_id) {
        let _ = e.delete_credential();
    }
}

// ---------------------------------------------------------------------------
// 证书 / 私钥解析
// ---------------------------------------------------------------------------

/// 解析 PEM 里的第一份证书。
pub fn parse_certificate(pem_text: &str) -> Result<Certificate, String> {
    use der::Decode;
    let der = pem_block(pem_text, "CERTIFICATE")?;
    Certificate::from_der(&der).map_err(|e| format!("证书解析失败: {e}"))
}

fn pem_block(pem_text: &str, label: &str) -> Result<Vec<u8>, String> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let start = pem_text
        .find(&begin)
        .ok_or_else(|| format!("没有找到 {label} 块"))?
        + begin.len();
    let rest = &pem_text[start..];
    let stop = rest
        .find(&end)
        .ok_or_else(|| format!("{label} 块没有结束标记"))?;
    let body: String = rest[..stop].chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD
        .decode(body.as_bytes())
        .map_err(|e| format!("{label} 的 base64 无效: {e}"))
}

pub fn parse_private_key(pem_text: &str) -> Result<RsaPrivateKey, String> {
    use rsa::pkcs8::DecodePrivateKey;
    if let Ok(der) = pem_block(pem_text, "PRIVATE KEY") {
        return RsaPrivateKey::from_pkcs8_der(&der).map_err(|e| format!("私钥解析失败: {e}"));
    }
    let der = pem_block(pem_text, "RSA PRIVATE KEY")?;
    RsaPrivateKey::from_pkcs1_der(&der).map_err(|e| format!("私钥解析失败: {e}"))
}

/// 从证书里取公钥。
pub fn public_key_of(cert: &Certificate) -> Result<RsaPublicKey, String> {
    use der::Encode;
    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|e| format!("读取证书公钥失败: {e}"))?;
    RsaPublicKey::from_public_key_der(&spki_der)
        .map_err(|e| format!("证书里的公钥不是 RSA（当前只支持 RSA 证书）: {e}"))
}

/// 证书主题的 CN（展示用）。
pub fn subject_cn(cert: &Certificate) -> String {
    cert.tbs_certificate
        .subject
        .0
        .iter()
        .flat_map(|rdn| rdn.0.iter())
        .find(|atv| atv.oid == const_oid_cn())
        // 属性值在 DER 里是一个 ANY：常见的是 UTF8String / PrintableString，
        // 两者的内容都是文本，直接按 UTF-8 读即可；读不出来就留空。
        .map(|atv| String::from_utf8_lossy(atv.value.value()).to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default()
}

fn const_oid_cn() -> der::asn1::ObjectIdentifier {
    // 2.5.4.3 = id-at-commonName
    der::asn1::ObjectIdentifier::new_unwrap("2.5.4.3")
}

/// 证书是否已过期（`notAfter` 早于当前时间）。
pub fn certificate_expired(cert: &Certificate) -> bool {
    let not_after = cert.tbs_certificate.validity.not_after.to_unix_duration();
    match chrono::DateTime::from_timestamp(not_after.as_secs() as i64, 0) {
        Some(when) => when < chrono::Utc::now(),
        // 解析不出时间时不当作过期，交给人判断，避免误拦。
        None => false,
    }
}

/// 收件人证书库：`guaxi-mail-smime-peers`，每条是一个地址 + 证书 PEM。
///
/// 与自己的身份分开存：自己的身份带私钥（敏感），收件人证书是公开信息，
/// 混在一起会让「导出证书给别人」变得别扭。
const PEERS_SERVICE: &str = "guaxi-mail-smime-peers";

fn peer_entry(address: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(PEERS_SERVICE, &address.trim().to_ascii_lowercase())
        .map_err(|e| format!("访问系统凭据失败: {e}"))
}

/// 保存某个收件人的证书（导入对方的 S/MIME 证书时调用）。
pub fn save_peer_certificate(address: &str, cert_pem: &str) -> Result<(), String> {
    parse_certificate(cert_pem)?;
    peer_entry(address)?
        .set_password(cert_pem)
        .map_err(|e| format!("保存收件人证书失败: {e}"))
}

pub fn load_peer_certificate(address: &str) -> Option<String> {
    peer_entry(address).ok()?.get_password().ok()
}

pub fn delete_peer_certificate(address: &str) {
    if let Ok(entry) = peer_entry(address) {
        let _ = entry.delete_credential();
    }
}

/// 在收件人列表里找第一个有证书的地址。返回证书 PEM。
pub fn recipient_certificate(addresses: &[String]) -> Option<String> {
    for raw in addresses {
        // 地址可能是 `Name <a@b.c>` 形态，先拆出裸地址。
        let addr = raw
            .split('<')
            .nth(1)
            .map(|s| s.trim_end_matches('>').trim())
            .unwrap_or(raw.trim());
        if let Some(pem) = load_peer_certificate(addr) {
            return Some(pem);
        }
    }
    None
}

/// 把签名结果重新包成「待加密的 MIME 实体」。
///
/// 先签名再加密时，被加密的对象是那份 `multipart/signed` 报文本身
/// （含它的 Content-Type 头），所以 `sign()` 的返回值（它已经是完整 MIME 实体）
/// 可以原样使用；这个函数只是让调用点读起来更清楚。
pub fn wrap_signed_entity(signed: &[u8]) -> Vec<u8> {
    signed.to_vec()
}

// ---------------------------------------------------------------------------
// 签名 / 加密
// ---------------------------------------------------------------------------

/// 用账户自己的 S/MIME 身份签名一份 MIME 报文。
///
/// 返回 `application/pkcs7-mime; smime-type=signed-data` 的完整 MIME 实体
/// （含头），可以直接塞进 SMTP DATA。
pub fn sign(account_id: &str, raw_mime: &[u8]) -> Result<Vec<u8>, String> {
    let identity = load_identity(account_id)
        .ok_or_else(|| "还没有配置 S/MIME 证书与私钥（设置 → 账户 → S/MIME）".to_string())?;
    let cert = parse_certificate(&identity.cert_pem)?;
    let key = parse_private_key(&identity.key_pem)?;

    let content = EncapsulatedContentInfo {
        econtent_type: const_oid::db::rfc5911::ID_DATA,
        econtent: Some(
            Any::new(
                Tag::OctetString,
                OctetString::new(raw_mime.to_vec())
                    .map_err(|e| format!("封装正文失败: {e}"))?
                    .to_der()
                    .map_err(|e| format!("封装正文失败: {e}"))?,
            )
            .map_err(|e| format!("封装正文失败: {e}"))?,
        ),
    };
    let digest_algorithm = spki::AlgorithmIdentifierOwned {
        oid: const_oid::db::rfc5912::ID_SHA_256,
        parameters: None,
    };
    let signer = SigningKey::<Sha256>::new(key);
    let sid = SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
        issuer: cert.tbs_certificate.issuer.clone(),
        serial_number: cert.tbs_certificate.serial_number.clone(),
    });
    let signer_info = SignerInfoBuilder::new(&signer, sid, digest_algorithm.clone(), &content, None)
        .map_err(|e| format!("构造签名信息失败: {e}"))?;

    let mut builder = SignedDataBuilder::new(&content);
    let signed = builder
        .add_digest_algorithm(digest_algorithm)
        .and_then(|b| b.add_certificate(CertificateChoices::Certificate(cert)))
        .map_err(|e| format!("组装签名失败: {e}"))?;
    let ci = signed
        .add_signer_info::<SigningKey<Sha256>, rsa::pkcs1v15::Signature>(signer_info)
        .and_then(|b| b.build())
        .map_err(|e| format!("生成签名失败: {e}"))?;
    let der = ci
        .to_der()
        .map_err(|e| format!("签名数据编码失败: {e}"))?;

    Ok(wrap_pkcs7("signed-data", &der, raw_mime))
}

/// 用收件人证书加密一份 MIME 报文。
pub fn encrypt(account_id: &str, raw_mime: &[u8], recipient_cert_pem: &str) -> Result<Vec<u8>, String> {
    let _ = account_id;
    let cert = parse_certificate(recipient_cert_pem)?;
    let public = public_key_of(&cert)?;
    // 收件人信息持有 rng 的可变借用；它必须在 `build_with_rng` 之前先 drop，
    // 否则同一个 rng 会被借两次。所以把它的构造放进一个局部块，
    // 由 `RecipientInfoBuilder` 在 `build` 阶段把密钥加密封装好之后释放借用。
    // 注意 rng 的生命周期：`KeyTransRecipientInfoBuilder` 会**持有** rng 的可变借用
    // 直到 `build_with_rng` 完成（它需要在 build 阶段加密会话密钥）。
    // 两个阶段因此必须用两个独立的 rng，否则同一个 rng 会被借两次。
    let mut detail_rng = rand08::rngs::OsRng;
    let recipient = KeyTransRecipientInfoBuilder::new(
        RecipientIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer.clone(),
            serial_number: cert.tbs_certificate.serial_number.clone(),
        }),
        KeyEncryptionInfo::Rsa(public),
        &mut detail_rng,
    )
    .map_err(|e| format!("构造收件人信息失败: {e}"))?;

    let mut builder = EnvelopedDataBuilder::new(
        None,
        raw_mime,
        ContentEncryptionAlgorithm::Aes256Cbc,
        None,
    )
    .map_err(|e| format!("构造加密数据失败: {e}"))?;
    let mut build_rng = rand08::rngs::OsRng;
    let enveloped = builder
        .add_recipient_info(recipient)
        .and_then(|b| b.build_with_rng(&mut build_rng))
        .map_err(|e| format!("加密失败: {e}"))?;
    let der = enveloped
        .to_der()
        .map_err(|e| format!("加密数据编码失败: {e}"))?;
    Ok(wrap_pkcs7("enveloped-data", &der, raw_mime))
}

/// 把 CMS 数据包成 MIME 实体。`base64` 是 S/MIME 的标准传输编码（RFC 5751 §3.1.1）。
fn wrap_pkcs7(smime_type: &str, der: &[u8], _original: &[u8]) -> Vec<u8> {
    let encoded = STANDARD.encode(der);
    // 每 76 字符折行（RFC 2045 对 base64 的要求）。
    let mut body = String::with_capacity(encoded.len() + encoded.len() / 76 + 8);
    for chunk in encoded.as_bytes().chunks(76) {
        body.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        body.push_str("\r\n");
    }
    format!(
        "Content-Type: application/pkcs7-mime; smime-type={smime_type}; name=smime.p7m\r\n\
         Content-Transfer-Encoding: base64\r\n\
         Content-Disposition: attachment; filename=smime.p7m\r\n\
         \r\n{body}"
    )
    .into_bytes()
}

/// 给签名用的「原始 MIME」。S/MIME 的 canonicalization 是 CRLF，
/// 但我们的 `mime::build` 已经输出 CRLF，这里只做一次防御性替换。
pub fn canonicalize(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / 32);
    let mut i = 0;
    while i < raw.len() {
        let byte = raw[i];
        if byte == b'\r' {
            out.push(b'\r');
            if raw.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
            out.push(b'\n');
        } else if byte == b'\n' {
            // 孤立的 LF（裸 LF）补成 CRLF。
            out.push(b'\r');
            out.push(b'\n');
        } else {
            out.push(byte);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// 验签 / 解密（打开加密邮件）
// ---------------------------------------------------------------------------



/// 解密 `EnvelopedData`，返回正文。
pub fn decrypt(account_id: &str, encrypted_mime: &str) -> Result<Vec<u8>, String> {
    let identity = load_identity(account_id)
        .ok_or_else(|| "这封邮件是加密的，但还没有导入你的 S/MIME 私钥".to_string())?;
    let key = parse_private_key(&identity.key_pem)?;
    let der = pem_block(encrypted_mime, "ENVELOPED MESSAGE")?;
    let ci = ContentInfo::from_der(&der).map_err(|e| format!("解析加密结构失败: {e}"))?;
    let env_bytes = ci
        .content
        .to_der()
        .map_err(|e| format!("读取加密内容失败: {e}"))?;
    let enveloped =
        EnvelopedData::from_der(&env_bytes).map_err(|e| format!("解析 EnvelopedData 失败: {e}"))?;
    super::smime_crypto::decrypt_enveloped(&enveloped, &key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_normalises_line_endings() {
        assert_eq!(canonicalize(b"a\nb\r\nc"), b"a\r\nb\r\nc".to_vec());
    }

    #[test]
    fn pem_block_reads_first_certificate() {
        // 一段结构合法的空证书块（内容随便，只要能 base64 解码）。
        let pem = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
        assert_eq!(pem_block(pem, "CERTIFICATE").unwrap(), b"ABC".to_vec());
    }

    #[test]
    fn wrap_pkcs7_folds_at_76_columns() {
        let der = vec![0u8; 120];
        let text = String::from_utf8(wrap_pkcs7("signed-data", &der, b"")).unwrap();
        assert!(text.contains("smime-type=signed-data"));
        assert!(text.contains("Content-Transfer-Encoding: base64"));
        // 正文的每一行都不超过 76 个可见字符。
        for line in text.split("\r\n").skip(4) {
            assert!(line.len() <= 76, "行过长: {}", line.len());
        }
    }
}
