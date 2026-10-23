//! 邮件本地加密。
//!
//! 威胁模型很具体：`mail.db` 是一个明文 SQLite 文件，用户备份、同步到网盘、
//! 或者换机迁移时会整份带走。里面有邮件正文与附件——等于把所有往来信件打包送人。
//! 所以这里对**正文与附件**做静态加密（at-rest），而不是对整库加密：
//! 整库加密就得换 SQLCipher（一个 C 依赖，且 Windows 构建要额外准备），
//! 而列表页需要的字段（发件人、主题、时间）本来就要能查、要能建 FTS 索引。
//!
//! 密钥走系统 keychain（services = `guaxi-mail-crypto`，account = 账户 id），
//! 与现有 `settingview` 的 `guaxi_config_master_key` 同一套思路：
//! - keychain 里有就用；
//! - 没有就随机生成 32 字节并写进去；
//! - keychain 不可用（极少数精简系统）时降到「按设备派生」并打警告——
//!   这种情况仍比明文好，但要如实告诉用户它绑在这台机器上。
//!
//! 格式：`enc:v1:` + base64( nonce[12] || ciphertext+tag )，与 `settingview` 的字段加密一致，
//! 这样同一份代码里两种密文形态不会让人困惑。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::rngs::OsRng;
use rand::TryRngCore;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{error, info, warn};

const KEYRING_SERVICE: &str = "guaxi-mail-crypto";
pub const ENC_PREFIX: &str = "enc:v1:";

/// 进程内缓存：每天几百次正文读写都去敲 keychain 会明显变慢。
static KEY_CACHE: Mutex<Option<HashMap<String, [u8; 32]>>> = Mutex::new(None);

fn cached_key(account_id: &str) -> Option<[u8; 32]> {
    KEY_CACHE
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().and_then(|map| map.get(account_id).copied()))
}

fn remember(account_id: &str, key: [u8; 32]) {
    if let Ok(mut guard) = KEY_CACHE.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(account_id.to_string(), key);
    }
}

fn random_key() -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut key)
        .map_err(|e| format!("生成加密密钥失败: {e}"))?;
    Ok(key)
}

fn entry(account_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, account_id).map_err(|e| format!("访问系统凭据失败: {e}"))
}

/// 取账户的本地加密密钥；没有就生成并写入 keychain。
pub fn account_key(account_id: &str) -> Result<[u8; 32], String> {
    if let Some(key) = cached_key(account_id) {
        return Ok(key);
    }
    if account_id.trim().is_empty() {
        return Err("账户 id 为空，无法取加密密钥".to_string());
    }

    match entry(account_id).and_then(|e| match e.get_password() {
        Ok(secret) => Ok(secret),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(e) => Err(format!("读取系统凭据失败: {e}")),
    }) {
        Ok(secret) if !secret.trim().is_empty() => {
            let raw = STANDARD
                .decode(secret.trim())
                .map_err(|e| format!("加密密钥格式无效: {e}"))?;
            if raw.len() != 32 {
                return Err(format!("加密密钥长度异常（{} 字节）", raw.len()));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&raw);
            remember(account_id, key);
            Ok(key)
        }
        Ok(_) => {
            // keychain 可用但还没密钥：生成并持久化。
            let key = random_key()?;
            match entry(account_id).and_then(|e| {
                e.set_password(&STANDARD.encode(key))
                    .map_err(|e| format!("写入系统凭据失败: {e}"))
            }) {
                Ok(()) => {
                    info!(account = account_id, "mail: 已生成并保存本地加密密钥");
                    remember(account_id, key);
                    Ok(key)
                }
                Err(e) => {
                    // 写不进去（企业策略禁用凭据管理器）：退回设备派生密钥。
                    // 这条要打 warn，因为换机后旧正文将无法解密。
                    warn!(
                        account = account_id,
                        error = %e,
                        "mail: 无法写入 keychain，改用设备派生密钥（仅本机可解）"
                    );
                    let key = device_key(account_id)?;
                    remember(account_id, key);
                    Ok(key)
                }
            }
        }
        Err(e) => {
            warn!(
                account = account_id,
                error = %e,
                "mail: keychain 不可用，改用设备派生密钥（仅本机可解）"
            );
            let key = device_key(account_id)?;
            remember(account_id, key);
            Ok(key)
        }
    }
}

/// 终极兜底：用设备 id + 账户 id 派生一个稳定密钥。
///
/// 不是密码学上的强隔离（拿到设备和配置文件的人可以重算），但它保证
/// 「数据库被单独拷走」时读不出内容——这正是要防的主要场景。
fn device_key(account_id: &str) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    let device = crate::commands::settingview::load_config().device_id;
    if device.trim().is_empty() {
        return Err("无法获取设备标识，本地加密不可用".to_string());
    }
    let mut hasher = Sha256::new();
    hasher.update(b"guaxi-mail-at-rest-v1");
    hasher.update(device.as_bytes());
    hasher.update(b"\0");
    hasher.update(account_id.as_bytes());
    let digest = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);
    Ok(key)
}

fn cipher(account_id: &str) -> Result<Aes256Gcm, String> {
    let key = account_key(account_id)?;
    Aes256Gcm::new_from_slice(&key).map_err(|e| format!("初始化加密器失败: {e}"))
}

/// 加密一段文本。空串原样返回（没必要给空值加开销）。
pub fn encrypt(account_id: &str, plain: &str) -> Result<String, String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    if plain.starts_with(ENC_PREFIX) {
        // 已经是密文（重复调用），原样返回，避免二次加密。
        return Ok(plain.to_string());
    }
    let cipher = cipher(account_id)?;
    let mut nonce_bytes = [0u8; 12];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| format!("生成随机数失败: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let sealed = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|e| format!("加密失败: {e}"))?;
    let mut blob = Vec::with_capacity(12 + sealed.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&sealed);
    Ok(format!("{ENC_PREFIX}{}", STANDARD.encode(blob)))
}

/// 解密。返回值区分三种情况，调用方才能正确展示：
/// - `Ok(Some(明文))`：正常解密；
/// - `Ok(None)`：不是密文（历史明文数据），调用方直接当明文用；
/// - `Err(..)`：是密文但解不开（换机器 / 密钥丢失），调用方要给出可读提示，
///   不能直接显示一串 base64。
pub fn decrypt(account_id: &str, stored: &str) -> Result<Option<String>, String> {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return Ok(Some(String::new()));
    }
    let Some(payload) = trimmed.strip_prefix(ENC_PREFIX) else {
        return Ok(None);
    };
    let blob = STANDARD
        .decode(payload)
        .map_err(|e| format!("密文格式无效: {e}"))?;
    if blob.len() <= 12 {
        return Err("密文长度异常".to_string());
    }
    let cipher = cipher(account_id)?;
    let nonce = Nonce::from_slice(&blob[..12]);
    let plain = cipher
        .decrypt(nonce, &blob[12..])
        .map_err(|_| "无法解密（密钥已变更或数据来自另一台设备）".to_string())?;
    String::from_utf8(plain)
        .map(Some)
        .map_err(|e| format!("解密结果不是有效文本: {e}"))
}

/// 字节版本（附件）。
pub fn encrypt_bytes(account_id: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = cipher(account_id)?;
    let mut nonce_bytes = [0u8; 12];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| format!("生成随机数失败: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let sealed = cipher
        .encrypt(nonce, plain)
        .map_err(|e| format!("加密附件失败: {e}"))?;
    let mut out = Vec::with_capacity(12 + sealed.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub fn decrypt_bytes(account_id: &str, blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() <= 12 {
        return Err("附件密文长度异常".to_string());
    }
    let cipher = cipher(account_id)?;
    let nonce = Nonce::from_slice(&blob[..12]);
    cipher
        .decrypt(nonce, &blob[12..])
        .map_err(|_| "无法解密附件（密钥已变更或数据来自另一台设备）".to_string())
}

/// 清掉密钥缓存（账户删除后调用，避免内存里留着一把没用的密钥）。
pub fn forget(account_id: &str) {
    if let Ok(mut guard) = KEY_CACHE.lock() {
        if let Some(map) = guard.as_mut() {
            map.remove(account_id);
        }
    }
}

/// 删除账户密钥（账户被移除时）。
pub fn delete_key(account_id: &str) {
    forget(account_id);
    if let Ok(e) = entry(account_id) {
        match e.delete_credential() {
            Ok(()) => info!(account = account_id, "mail: 已删除本地加密密钥"),
            Err(keyring::Error::NoEntry) => {}
            Err(err) => error!(account = account_id, error = %err, "mail: 删除加密密钥失败"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_text() {
        let account = format!("test-{}", uuid::Uuid::new_v4());
        let plain = "季度报告：附件里有完整数据 🙂";
        let sealed = encrypt(&account, plain).unwrap();
        assert!(sealed.starts_with(ENC_PREFIX));
        assert_ne!(sealed, plain);
        // 两次加密结果不同（nonce 随机）
        let again = encrypt(&account, plain).unwrap();
        assert_ne!(sealed, again);
        assert_eq!(decrypt(&account, &sealed).unwrap().as_deref(), Some(plain));
        delete_key(&account);
    }

    #[test]
    fn plaintext_is_passed_through() {
        let account = format!("test-{}", uuid::Uuid::new_v4());
        // 历史明文数据：返回 None 表示「不是密文」，调用方当明文用。
        assert_eq!(decrypt(&account, "旧数据").unwrap(), None);
    }

    #[test]
    fn empty_stays_empty() {
        let account = format!("test-{}", uuid::Uuid::new_v4());
        assert_eq!(encrypt(&account, "").unwrap(), "");
        assert_eq!(decrypt(&account, "").unwrap(), Some(String::new()));
    }

    #[test]
    fn double_encrypt_is_idempotent() {
        let account = format!("test-{}", uuid::Uuid::new_v4());
        let once = encrypt(&account, "hello").unwrap();
        let twice = encrypt(&account, &once).unwrap();
        assert_eq!(once, twice, "已经是密文时不应再加密一层");
        delete_key(&account);
    }

    #[test]
    fn wrong_account_cannot_decrypt() {
        let a = format!("test-{}", uuid::Uuid::new_v4());
        let b = format!("test-{}", uuid::Uuid::new_v4());
        let sealed = encrypt(&a, "secret").unwrap();
        assert!(decrypt(&b, &sealed).is_err(), "不同账户的密钥必须解不开");
        delete_key(&a);
        delete_key(&b);
    }

    #[test]
    fn bytes_roundtrip() {
        let account = format!("test-{}", uuid::Uuid::new_v4());
        let data = vec![0u8, 1, 2, 255, 128, 64];
        let sealed = encrypt_bytes(&account, &data).unwrap();
        assert_ne!(sealed, data);
        assert_eq!(decrypt_bytes(&account, &sealed).unwrap(), data);
        delete_key(&account);
    }
}
