//! OpenPGP（RFC 9580）：邮件签名 / 加密。
//!
//! 目标场景与 S/MIME 互补：S/MIME 靠企业 CA 下发的证书，PGP 靠用户自己交换的公钥
//! （Web of Trust / keyserver）。企业客户多半要 S/MIME，技术圈 / 开源社群则用 PGP。
//!
//! 支持范围：
//! - 签名（`MessageBuilder::sign`，分离在报文内的一体化签名 `multipart/signed`）；
//! - 加密（`seipd_v1` + AES256，兼容性最广的容器格式）；
//! - 解密与验签（`Message::decrypt` / `verify`）。
//!
//! 密钥只存两份：自己的私钥环（可选口令保护）与联系人的公钥环，
//! 都落在用户数据目录的 `mail/pgp/` 下——PGP 私钥本来就是用户自己的文件资产，
//! 藏在 keychain 里反而不利于用户备份或迁移到其它客户端。口令进 keychain。
//!
//! 与 S/MIME 一样，这里**不做证书链 / Web-of-Trust 判定**：只回答「签名在数学上
//! 是否有效」，信任关系交给用户。

use pgp::composed::{Deserializable, Message, MessageBuilder, SignedPublicKey, SignedSecretKey};
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::crypto::hash::HashAlgorithm;
use pgp::types::{KeyDetails, Password};
use std::path::{Path, PathBuf};

/// PGP 密钥库路径：`<mail data root>/pgp`。
/// PGP 钥匙串目录。由 `mod.rs` 解析好路径传进来，模块本身不依赖 Tauri。
pub fn key_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = super::store::data_root(app)?.join("pgp");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 PGP 目录失败: {e}"))?;
    Ok(dir)
}

pub fn public_ring_path(dir: &Path) -> PathBuf {
    dir.join("public-keys.asc")
}

pub fn secret_ring_path(dir: &Path) -> PathBuf {
    dir.join("secret-keys.asc")
}

/// 读取公钥环里的全部公钥（每段以 `-----BEGIN PGP PUBLIC KEY BLOCK-----` 开头）。
pub fn load_public_keys(dir: &Path) -> Vec<SignedPublicKey> {
    let path = public_ring_path(dir);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    parse_public_ring(&text)
}

fn parse_public_ring(text: &str) -> Vec<SignedPublicKey> {
    split_armor_blocks(text, "PGP PUBLIC KEY BLOCK")
        .into_iter()
        .filter_map(|block| SignedPublicKey::from_string(&block).ok().map(|(k, _)| k))
        .collect()
}

/// 读取私钥环。私钥可能带口令：`passphrase` 由调用方从 keychain 取。
pub fn load_secret_keys(dir: &Path) -> Vec<SignedSecretKey> {
    let path = secret_ring_path(dir);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    parse_secret_ring(&text)
}

fn parse_secret_ring(text: &str) -> Vec<SignedSecretKey> {
    split_armor_blocks(text, "PGP PRIVATE KEY BLOCK")
        .into_iter()
        .filter_map(|block| SignedSecretKey::from_string(&block).ok().map(|(k, _)| k))
        .collect()
}

/// 把一段多块装甲文本切成独立块。
fn split_armor_blocks(text: &str, label: &str) -> Vec<String> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&begin) {
        let after = &rest[start..];
        let Some(stop) = after.find(&end) else { break };
        out.push(after[..stop + end.len()].to_string());
        rest = &after[stop + end.len()..];
    }
    out
}

fn fingerprint_of(key: &SignedPublicKey) -> String {
    key.fingerprint().to_string()
}

/// 公钥环摘要（展示给用户：哪把钥匙属于谁）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgpKeyInfo {
    pub fingerprint: String,
    pub user_id: String,
    pub is_secret: bool,
    /// 私钥是否带口令保护。
    pub encrypted: bool,
}

pub fn public_key_summaries(app: &tauri::AppHandle) -> Vec<PgpKeyInfo> {
    let Ok(dir) = key_dir(app) else {
        return Vec::new();
    };
    let mut out: Vec<PgpKeyInfo> = load_public_keys(&dir)
        .iter()
        .map(|k| PgpKeyInfo {
            fingerprint: fingerprint_of(k),
            user_id: k
                .details
                .users
                .first()
                .map(|u| String::from_utf8_lossy(u.id.id()).to_string())
                .unwrap_or_default(),
            is_secret: false,
            encrypted: false,
        })
        .collect();
    for k in load_secret_keys(&dir) {
        out.push(PgpKeyInfo {
            fingerprint: k.fingerprint().to_string(),
            user_id: k
                .details
                .users
                .first()
                .map(|u| String::from_utf8_lossy(u.id.id()).to_string())
                .unwrap_or_default(),
            is_secret: true,
            encrypted: k.primary_key.secret_params().is_encrypted(),
        });
    }
    out
}

/// 把一份新公钥追加到公钥环文件（按指纹去重）。
pub fn add_public_key(dir: &Path, armored: &str) -> Result<PgpKeyInfo, String> {
    let (key, _) = SignedPublicKey::from_string(armored)
        .map_err(|e| format!("公钥解析失败（需要 armored PGP 公钥块）: {e}"))?;
    let fp = fingerprint_of(&key);
    let path = public_ring_path(dir);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if parse_public_ring(&existing)
        .iter()
        .any(|k| fingerprint_of(k) == fp)
    {
        return Err("这把公钥已经在钥匙串里了".to_string());
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(armored.trim());
    next.push('\n');
    std::fs::write(&path, next).map_err(|e| format!("写入公钥环失败: {e}"))?;
    Ok(PgpKeyInfo {
        fingerprint: fp,
        user_id: key
            .details
            .users
            .first()
            .map(|u| String::from_utf8_lossy(u.id.id()).to_string())
            .unwrap_or_default(),
        is_secret: false,
        encrypted: false,
    })
}

/// 找到某个收件人地址对应的公钥。按 User ID 里的地址（不区分大小写）匹配。
pub fn key_for_address(dir: &Path, address: &str) -> Option<SignedPublicKey> {
    let needle = address.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return None;
    }
    load_public_keys(dir).into_iter().find(|key| {
        key.details.users.iter().any(|uid| {
            String::from_utf8_lossy(uid.id.id())
                .to_ascii_lowercase()
                .contains(&needle)
        })
    })
}

/// 用收件人公钥加密一份 MIME 报文。
pub fn encrypt(dir: &Path, recipient: &str, raw: &[u8]) -> Result<Vec<u8>, String> {
    let key = key_for_address(dir, recipient)
        .ok_or_else(|| format!("没有 {recipient} 的 PGP 公钥，请先在设置里导入"))?;
    // `EncryptionKey` 不是 dyn-safe 的（含泛型方法），所以这里不擦除类型：
    // 两个具体类型各走一条分支。绝大多数密钥都有加密子密钥，
    // 退回主密钥只是给极旧的密钥留一条路。
    let enc_subkey = key
        .public_subkeys
        .iter()
        .find(|sk| sk.algorithm().can_encrypt())
        .map(|sk| sk.key.clone());

    let mut rng = rand08::thread_rng();
    let mut builder = MessageBuilder::from_bytes("", raw.to_vec())
        .seipd_v1(&mut rng, SymmetricKeyAlgorithm::AES256);
    match enc_subkey {
        Some(subkey) => {
            builder
                .encrypt_to_key(&mut rng, &subkey)
                .map_err(|e| format!("PGP 加密失败: {e}"))?;
        }
        None => {
            let primary = key.primary_key.clone();
            if !primary.algorithm().can_encrypt() {
                return Err("这把 PGP 公钥没有可用于加密的（子）密钥".to_string());
            }
            builder
                .encrypt_to_key(&mut rng, &primary)
                .map_err(|e| format!("PGP 加密失败: {e}"))?;
        }
    };
    builder
        .to_vec(&mut rng)
        .map_err(|e| format!("PGP 报文生成失败: {e}"))
}

/// 用私钥签名一份 MIME 报文（一体化签名，`multipart/signed` 的内层）。
pub fn sign(dir: &Path, raw: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let keys = load_secret_keys(dir);
    let key = keys.first().ok_or_else(|| "还没有导入 PGP 私钥".to_string())?;
    let signing: &dyn pgp::types::SigningKey = key
        .secret_subkeys
        .iter()
        .find(|sk| sk.key.algorithm().can_sign())
        .map(|sk| &sk.key as &dyn pgp::types::SigningKey)
        .unwrap_or(&key.primary_key);
    let mut rng = rand08::thread_rng();
    let mut builder = MessageBuilder::from_bytes("", raw.to_vec());
    builder.sign(signing, Password::from(passphrase), HashAlgorithm::Sha256);
    builder
        .to_vec(&mut rng)
        .map_err(|e| format!("PGP 签名失败: {e}"))
}

/// 解密一段 PGP 报文。
pub fn decrypt(dir: &Path, data: &[u8], passphrase: &str) -> Result<Vec<u8>, String> {
    let keys = load_secret_keys(dir);
    if keys.is_empty() {
        return Err("这封邮件是 PGP 加密的，但还没有导入你的 PGP 私钥".to_string());
    }
    let msg = Message::from_bytes(data).map_err(|e| format!("PGP 报文解析失败: {e}"))?;
    let mut decrypted = msg
        .decrypt(&Password::from(passphrase), &keys[0])
        .map_err(|e| format!("PGP 解密失败（口令不对或不是发给这把钥匙）: {e}"))?;
    if decrypted.is_compressed() {
        decrypted = decrypted
            .decompress()
            .map_err(|e| format!("PGP 解压失败: {e}"))?;
    }
    decrypted
        .as_data_vec()
        .map_err(|e| format!("读取 PGP 正文失败: {e}"))
}

/// 验签结果。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgpVerifyResult {
    pub signed: bool,
    pub valid: bool,
    pub key_id: String,
    pub detail: String,
    pub body: Vec<u8>,
}

/// 验签：返回正文与「签名是否由已知公钥签署」。
pub fn verify(dir: &Path, data: &[u8]) -> Result<PgpVerifyResult, String> {
    let mut msg = Message::from_bytes(data).map_err(|e| format!("PGP 报文解析失败: {e}"))?;
    // 压缩层必须先解开，`verify_nested` 不接受压缩后的报文。
    if msg.is_compressed() {
        msg = msg.decompress().map_err(|e| format!("PGP 解压失败: {e}"))?;
    }
    let signed = msg.is_signed();
    let keys = load_public_keys(dir);
    let key_refs: Vec<&dyn pgp::types::VerifyingKey> = keys
        .iter()
        .map(|k| k as &dyn pgp::types::VerifyingKey)
        .collect();
    let results = msg
        .verify_nested(&key_refs)
        .map_err(|e| format!("PGP 验签失败: {e}"))?;
    let body = msg.as_data_vec().unwrap_or_default();

    let valid = results
        .iter()
        .find_map(|r| match r {
            pgp::composed::VerificationResult::Valid(sig) => Some(
                sig.issuer_fingerprint()
                    .first()
                    .map(|fp| format!("{fp}"))
                    .or_else(|| sig.issuer_key_id().first().map(|id| format!("{id}")))
                    .unwrap_or_default(),
            ),
            pgp::composed::VerificationResult::Invalid => None,
        });
    Ok(match valid {
        Some(key_id) => PgpVerifyResult {
            signed,
            valid: true,
            detail: format!("签名有效，签发者 {key_id}"),
            key_id,
            body,
        },
        None => PgpVerifyResult {
            signed,
            valid: false,
            key_id: String::new(),
            detail: if signed {
                "签名存在，但钥匙串里没有对应的公钥".to_string()
            } else {
                "这段数据没有 PGP 签名".to_string()
            },
            body,
        },
    })
}


/// 从文件导入公钥 / 私钥环。
pub fn import_from_file(dir: &Path, path: &Path, secret: bool) -> Result<usize, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("读取密钥文件失败: {e}"))?;
    let target = if secret {
        secret_ring_path(dir)
    } else {
        public_ring_path(dir)
    };
    let label = if secret {
        "PGP PRIVATE KEY BLOCK"
    } else {
        "PGP PUBLIC KEY BLOCK"
    };
    let blocks = split_armor_blocks(&text, label);
    if blocks.is_empty() {
        return Err(format!(
            "文件里没有 {} 块",
            if secret { "PGP 私钥" } else { "PGP 公钥" }
        ));
    }
    // 先验证能解析，避免把坏数据写进钥匙串。
    let parsed = if secret {
        blocks
            .iter()
            .filter(|b| SignedSecretKey::from_string(b).is_ok())
            .count()
    } else {
        blocks
            .iter()
            .filter(|b| SignedPublicKey::from_string(b).is_ok())
            .count()
    };
    if parsed == 0 {
        return Err("密钥块无法解析".to_string());
    }
    let existing = std::fs::read_to_string(&target).unwrap_or_default();
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&blocks.join("\n"));
    next.push('\n');
    std::fs::write(&target, next).map_err(|e| format!("写入钥匙串失败: {e}"))?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_armor_blocks_extracts_each_block() {
        let text = "junk\n-----BEGIN PGP PUBLIC KEY BLOCK-----\nA\n-----END PGP PUBLIC KEY BLOCK-----\n\
                    mid\n-----BEGIN PGP PUBLIC KEY BLOCK-----\nB\n-----END PGP PUBLIC KEY BLOCK-----\n";
        let blocks = split_armor_blocks(text, "PGP PUBLIC KEY BLOCK");
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains('A'));
        assert!(blocks[1].contains('B'));
    }
}

/// 把一段二进制 PGP 密文包成 `application/pgp-encrypted` 的 MIME 实体。
///
/// 这里用 base64 而不是 8bit：8bit 需要服务器声明 8BITMIME，而部分国内邮箱
/// 对含二进制字节的报文会做不可预期的改写，导致收信端签名/解密校验失败。
pub fn wrap_encrypted(data: &[u8]) -> Vec<u8> {
    wrap_block("application/pgp-encrypted", data)
}

/// 把一体化签名报文包成 `application/pgp-signature` 的 MIME 实体。
pub fn wrap_signed(data: &[u8]) -> Vec<u8> {
    wrap_block("application/pgp-signature", data)
}

fn wrap_block(content_type: &str, data: &[u8]) -> Vec<u8> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let encoded = STANDARD.encode(data);
    let mut body = String::with_capacity(encoded.len() + encoded.len() / 76 + 8);
    for chunk in encoded.as_bytes().chunks(76) {
        body.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        body.push_str("\r\n");
    }
    format!(
        "Content-Type: {content_type}\r\nContent-Transfer-Encoding: base64\r\n\r\n{body}"
    )
    .into_bytes()
}
