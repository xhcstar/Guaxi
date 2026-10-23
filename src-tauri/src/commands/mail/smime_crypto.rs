//! CMS `EnvelopedData` 的解密。
//!
//! `cms` crate 只提供「构造」能力，没有解密路径；这里用它的数据结构
//! + RustCrypto 的 aes/cbc/rsa 把 RFC 5652 §6.1 的流程补上：
//!
//! ```text
//! encryptedContent (OCTET STRING，AES-CBC 密文)
//!    └─ AES-256-CBC 解密（密钥是会话密钥，IV 是密文前 16 字节）
//!         └─ 得到原文
//! 会话密钥本身：enc_key 用私钥 RSA 解密（PKCS#1 v1.5）
//! ```
//!
//! 只支持 AES-CBC 与 RSA 密钥传输这一条路径——这是邮件场景（RFC 5751）的标准组合，
//! 其它算法组合在真实邮件里基本见不到，留口子反而增加误判风险。

use aes::cipher::{
    block_padding::Pkcs7, BlockDecrypt, BlockDecryptMut, BlockSizeUser, KeyInit, KeyIvInit,
    KeySizeUser,
};
use cms::enveloped_data::{EncryptedContentInfo, EnvelopedData, RecipientInfo};
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey};

/// 解密一个 `EnvelopedData`，返回原始 MIME 字节。
pub fn decrypt_enveloped(enveloped: &EnvelopedData, key: &RsaPrivateKey) -> Result<Vec<u8>, String> {
    // 1) 找到属于我们这把私钥的 recipient info。多收件人时可能会有好几条，
    //    逐个尝试 RSA 解密，失败就换下一条——比按 issuer/serial 精确匹配更稳，
    //    因为部分客户端填的 serial 并不符合 RFC。
    let content_key = recover_content_key(enveloped, key)?;

    // 2) 用会话密钥解正文。
    let info = &enveloped.encrypted_content;
    let algorithm = algorithm_kind(&info)?;
    let cipher_bytes = info
        .encrypted_content
        .as_ref()
        .ok_or_else(|| "加密邮件里没有正文内容".to_string())?
        .as_bytes();

    match algorithm {
        KeyCipher::Aes128 => decrypt_cbc::<aes::Aes128>(cipher_bytes, &content_key),
        KeyCipher::Aes192 => decrypt_cbc::<aes::Aes192>(cipher_bytes, &content_key),
        KeyCipher::Aes256 => decrypt_cbc::<aes::Aes256>(cipher_bytes, &content_key),
    }
}

enum KeyCipher {
    Aes128,
    Aes192,
    Aes256,
}

fn algorithm_kind(info: &EncryptedContentInfo) -> Result<KeyCipher, String> {
    let oid = info.content_enc_alg.oid.to_string();
    match oid.as_str() {
        // AES-128-CBC / AES-192-CBC / AES-256-CBC
        "2.16.840.1.101.3.4.1.2" => Ok(KeyCipher::Aes128),
        "2.16.840.1.101.3.4.1.22" => Ok(KeyCipher::Aes192),
        "2.16.840.1.101.3.4.1.42" => Ok(KeyCipher::Aes256),
        other => Err(format!(
            "不支持的邮件加密算法（{other}）：当前只支持 AES-CBC 的 S/MIME 加密邮件"
        )),
    }
}

fn recover_content_key(enveloped: &EnvelopedData, key: &RsaPrivateKey) -> Result<Vec<u8>, String> {
    let mut last_error = String::from("加密邮件里没有收件人信息");
    for recipient in enveloped.recip_infos.0.iter() {
        let RecipientInfo::Ktri(ktri) = recipient else {
            // 密钥协商 / KEK / 口令方式的收件人信息不支持。
            continue;
        };
        // 算法必须是 rsaEncryption（1.2.840.113549.1.1.1）。
        let enc_alg = ktri.key_enc_alg.oid.to_string();
        if enc_alg != "1.2.840.113549.1.1.1" {
            last_error = format!("不支持的密钥传输算法（{enc_alg}）");
            continue;
        }
        match key.decrypt(Pkcs1v15Encrypt, ktri.enc_key.as_bytes()) {
            Ok(bytes) => return Ok(bytes),
            Err(e) => {
                last_error = format!("会话密钥解密失败: {e}");
                continue;
            }
        }
    }
    Err(last_error)
}

/// AES-CBC + PKCS#7 解填充。S/MIME 的 IV 就是密文的前一个块。
///
/// 泛型参数 `C` 只要求「块密码 + 固定密钥长度」：`aes` crate 的 `Aes128/192/256`
/// 都满足，而 `cbc` 模式自己处理分组，不需要再约束加密/解密 trait。
fn decrypt_cbc<C>(cipher_bytes: &[u8], key_bytes: &[u8]) -> Result<Vec<u8>, String>
where
    C: aes::cipher::BlockCipher + KeyInit + BlockSizeUser + BlockDecrypt,
{
    let block = <C as BlockSizeUser>::block_size();
    if cipher_bytes.len() < block * 2 {
        return Err("加密正文长度异常".to_string());
    }
    let need = <C as KeySizeUser>::key_size();
    if key_bytes.len() != need {
        return Err(format!(
            "会话密钥长度（{} 字节）与算法要求的 {need} 字节不一致",
            key_bytes.len()
        ));
    }
    let (iv, body) = cipher_bytes.split_at(block);
    let mut buf = body.to_vec();
    cbc::Decryptor::<C>::new_from_slices(key_bytes, iv)
        .map_err(|e| format!("初始化解密器失败: {e}"))?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map(|plain| plain.to_vec())
        .map_err(|e| format!("S/MIME 正文解密失败: {e}"))
}

