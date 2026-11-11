//! 多格式离线词典解析。
//!
//! 目标：让「Guaxi 词典」能直接吃市面上常见词典软件的离线词库，而不是要求用户
//! 先自己转成 CSV。
//!
//! 支持的来源：
//! - **MDict**：`.mdx`（词条）+ `.mdd`（图片/发音/样式资源），欧路 / 深蓝 / GoldenDict
//!   用的都是这一套。头信息是 UTF-16LE 的 XML，正文按 key block / record block
//!   分块 zlib 压缩；`Encrypted=1|2` 时两块 block info 还会用 RIPEMD-128 派生的
//!   密钥异或加密。
//! - **StarDict**：`.ifo` + `.idx`/`.idx.gz` + `.dict`/`.dict.dz`，GoldenDict、
//!   星际译王、部分 Lingvo 转换包。
//! - **ABBYY / Lingoes DSL**：`.dsl`，UTF-16LE 文本，`[trn]`/`[m1]`/`[ex]` 标记。
//! - **Wiktionary (kaikki.org) JSONL**：日/韩/法/德/西/俄等语种的免费词库来源。
//!
//! 这里只负责「把文件读成一条条词条」，落库、去重、词形变化由 `offline_dict`
//! 统一处理——这样新增格式不会污染索引逻辑。
//!
//! 已知不支持：Babylon `.bgl`（私有二进制，含商业授权内容）、灵格斯 `.ld2`（私有）。

use base64::Engine;
use encoding_rs::Encoding;
use flate2::read::{GzDecoder, ZlibDecoder};
use regex::Regex;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// 解析出来的一条词条（与落库 schema 解耦）。
#[derive(Debug, Default, Clone)]
pub struct RawEntry {
    pub term: String,
    pub phonetic: String,
    pub pos: String,
    pub meanings: Vec<String>,
    pub examples: Vec<String>,
    pub synonyms: Vec<String>,
    /// 富文本释义（HTML）。MDict / StarDict / DSL 都有，CSV 类来源为空。
    pub definition: String,
    /// MDict 的 `@@@LINK=xxx` 跳转目标，落库时写成词形映射。
    pub link: Option<String>,
}

fn decode(bytes: &[u8], encoding: &'static Encoding) -> String {
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

// ── RIPEMD-128（MDict 加密块用） ────────────────────────────────────────
const ZL: [usize; 64] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 7, 4, 13, 1, 10, 6, 15, 3, 12, 0, 9, 5, 2,
    14, 11, 8, 3, 10, 14, 4, 9, 15, 8, 1, 2, 7, 0, 6, 13, 11, 5, 12, 1, 9, 11, 10, 0, 8, 12, 4, 13,
    3, 7, 15, 14, 5, 6, 2,
];
const ZR: [usize; 64] = [
    5, 14, 7, 0, 9, 2, 11, 4, 13, 6, 15, 8, 1, 10, 3, 12, 6, 11, 3, 7, 0, 13, 5, 10, 14, 15, 8, 12,
    4, 9, 1, 2, 15, 5, 1, 3, 7, 14, 6, 9, 11, 8, 12, 2, 10, 0, 4, 13, 8, 6, 4, 1, 3, 11, 15, 0, 5,
    12, 2, 13, 9, 7, 10, 14,
];
const SL: [u32; 64] = [
    11, 14, 15, 12, 5, 8, 7, 9, 11, 13, 14, 15, 6, 7, 9, 8, 7, 6, 8, 13, 11, 9, 7, 15, 7, 12, 15,
    9, 11, 7, 13, 12, 11, 13, 6, 7, 14, 9, 13, 15, 14, 8, 13, 6, 5, 12, 7, 5, 11, 12, 14, 15, 14,
    15, 9, 8, 9, 14, 5, 6, 8, 6, 5, 12,
];
const SR: [u32; 64] = [
    8, 9, 9, 11, 13, 15, 15, 5, 7, 7, 8, 11, 14, 14, 12, 6, 9, 13, 15, 7, 12, 8, 9, 11, 7, 7, 12,
    7, 6, 15, 13, 11, 9, 7, 15, 11, 8, 6, 6, 14, 12, 13, 5, 14, 13, 13, 7, 5, 15, 5, 8, 11, 14, 14,
    6, 14, 6, 9, 12, 9, 12, 5, 15, 8,
];
const KL: [u32; 4] = [0x0000_0000, 0x5A82_7999, 0x6ED9_EBA1, 0x8F1B_BCDC];
const KR: [u32; 4] = [0x50A2_8BE6, 0x5C4D_D124, 0x6D70_3EF3, 0x0000_0000];

fn rmd_f(round: usize, x: u32, y: u32, z: u32) -> u32 {
    match round {
        0 => x ^ y ^ z,
        1 => (x & y) | (!x & z),
        2 => (x | !y) ^ z,
        _ => (x & z) | (y & !z),
    }
}

/// RIPEMD-128。MDict 用它把「头校验和」派生成异或密钥。
pub fn ripemd128(data: &[u8]) -> [u8; 16] {
    let mut h: [u32; 4] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    for chunk in msg.chunks(64) {
        let mut x = [0u32; 16];
        for (i, word) in x.iter_mut().enumerate() {
            *word = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        let (mut al, mut bl, mut cl, mut dl) = (h[0], h[1], h[2], h[3]);
        let (mut ar, mut br, mut cr, mut dr) = (h[0], h[1], h[2], h[3]);
        for j in 0..64 {
            let t = al
                .wrapping_add(rmd_f(j / 16, bl, cl, dl))
                .wrapping_add(x[ZL[j]])
                .wrapping_add(KL[j / 16])
                .rotate_left(SL[j]);
            al = dl;
            dl = cl;
            cl = bl;
            bl = t;

            let t = ar
                .wrapping_add(rmd_f((63 - j) / 16, br, cr, dr))
                .wrapping_add(x[ZR[j]])
                .wrapping_add(KR[j / 16])
                .rotate_left(SR[j]);
            ar = dr;
            dr = cr;
            cr = br;
            br = t;
        }
        let last = dr.wrapping_add(cl).wrapping_add(h[1]);
        h[1] = h[2].wrapping_add(dl).wrapping_add(ar);
        h[2] = h[3].wrapping_add(al).wrapping_add(br);
        h[3] = h[0].wrapping_add(bl).wrapping_add(cr);
        h[0] = last;
    }
    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_le_bytes());
    }
    out
}

fn inflate(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ZlibDecoder::new(raw)
        .read_to_end(&mut out)
        .map_err(|e| format!("解压词典数据失败: {}", e))?;
    Ok(out)
}

fn read_be_u64(buf: &[u8], pos: &mut usize) -> Option<u64> {
    if *pos + 8 > buf.len() {
        return None;
    }
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&buf[*pos..*pos + 8]);
    *pos += 8;
    Some(u64::from_be_bytes(arr))
}

fn read_be_u16(buf: &[u8], pos: &mut usize) -> Option<u16> {
    if *pos + 2 > buf.len() {
        return None;
    }
    let mut arr = [0u8; 2];
    arr.copy_from_slice(&buf[*pos..*pos + 2]);
    *pos += 2;
    Some(u16::from_be_bytes(arr))
}

// ── MDict（.mdx / .mdd） ────────────────────────────────────────────────
//
// 关键细节（来自 MDict 2.0 格式逆向文档）：
// - 每个压缩块开头是 8 字节头：4 字节压缩类型（0=不压缩 / 1=LZO / 2=zlib）+ 4 字节
//   ADLER32（**属于解压后的数据**），真正的数据从第 8 字节开始。
// - key 索引（keyword index）在 `Encrypted & 2` 时会被「半字节交换 + 滚动异或」加密，
//   密钥 = RIPEMD128(该块自己的 ADLER32 + 0x95 0x36 0x00 0x00)。
// - 1.2 与 2.0 的数字宽度不同（4 字节 vs 8 字节），2.0 在 40 字节索引头后还多一个校验和。

struct MdictBlock {
    compressed: u64,
    decompressed: u64,
}

struct MdictReader {
    file: File,
    encoding: &'static Encoding,
    v2: bool,
    /// 文本长度单位：UTF-16 是 2 字节，UTF-8/GBK 是 1 字节。
    unit: usize,
    /// 头里的 `StyleSheet`：正文用 `` `N` `` 标记引用，导出时换成对应标签。
    stylesheet: std::collections::HashMap<String, (String, String)>,
    key_blocks: Vec<MdictBlock>,
    key_blocks_start: u64,
    record_blocks: Vec<MdictBlock>,
    record_blocks_start: u64,
    record_starts: Vec<u64>,
    /// 各 record 块在文件里的起始位置（压缩尺寸的累加），用于定位读盘。
    record_file_starts: Vec<u64>,
    total_record_size: u64,
    cache: Option<(usize, Vec<u8>)>,
}

fn read_num(buf: &[u8], pos: &mut usize, v2: bool) -> Option<u64> {
    if v2 {
        read_be_u64(buf, pos)
    } else {
        if *pos + 4 > buf.len() {
            return None;
        }
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&buf[*pos..*pos + 4]);
        *pos += 4;
        Some(u32::from_be_bytes(arr) as u64)
    }
}

fn read_file_num(file: &mut File, v2: bool) -> Result<u64, String> {
    if v2 {
        let mut buf = [0u8; 8];
        file.read_exact(&mut buf)
            .map_err(|_| "词典索引头不完整".to_string())?;
        Ok(u64::from_be_bytes(buf))
    } else {
        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)
            .map_err(|_| "词典索引头不完整".to_string())?;
        Ok(u32::from_be_bytes(buf) as u64)
    }
}

// ── LZO1X 解压 ──────────────────────────────────────────────────────────
//
// MdxBuilder 有两套压缩，默认那套是 LZO1X-1（块头 4 字节 = 1）。MDict 生态里
// LZO 词库占比很高，绕不开；而 lzo / minilzo 这些 crate 都要链 C 库，交叉编译
// 与自包含分发都不划算，所以这里按 miniLZO 的 `lzo1x_decompress_safe` 逐字节
// 复刻一份纯 Rust 版本。

fn lzo_byte(src: &[u8], ip: &mut usize) -> Result<usize, String> {
    if *ip >= src.len() {
        return Err("LZO 数据提前结束".to_string());
    }
    let value = src[*ip] as usize;
    *ip += 1;
    Ok(value)
}

fn lzo_literals(src: &[u8], ip: &mut usize, out: &mut Vec<u8>, count: usize) -> Result<(), String> {
    if *ip + count > src.len() {
        return Err("LZO 字面量越界".to_string());
    }
    out.extend_from_slice(&src[*ip..*ip + count]);
    *ip += count;
    Ok(())
}

/// 回溯复制。逐字节复制是必须的——LZO 允许距离小于长度（自重叠），
/// 用 copy_within 会读到还没写出来的数据。
fn lzo_back_copy(out: &mut Vec<u8>, pos: isize, count: usize) -> Result<(), String> {
    if pos < 0 {
        return Err("LZO 回溯距离越界".to_string());
    }
    let mut source = pos as usize;
    for _ in 0..count {
        if source >= out.len() {
            return Err("LZO 回溯距离越界".to_string());
        }
        let byte = out[source];
        out.push(byte);
        source += 1;
    }
    Ok(())
}

fn lzo1x_decompress(src: &[u8], hint: usize) -> Result<Vec<u8>, String> {
    enum State {
        Outer,
        FirstRun,
        Match(usize),
        Done,
        Next(usize),
    }
    let mut out: Vec<u8> = Vec::with_capacity(hint.max(64));
    let mut ip = 0usize;
    let mut state = if !src.is_empty() && src[0] > 17 {
        let t = src[0] as usize - 17;
        ip = 1;
        if t < 4 {
            // 短字面量交给 match_next 处理，避免重复拷贝
            State::Next(t)
        } else {
            lzo_literals(src, &mut ip, &mut out, t)?;
            State::FirstRun
        }
    } else {
        State::Outer
    };
    loop {
        match state {
            State::Outer => {
                let first = lzo_byte(src, &mut ip)?;
                if first >= 16 {
                    state = State::Match(first);
                    continue;
                }
                let mut len = first;
                if len == 0 {
                    loop {
                        let value = lzo_byte(src, &mut ip)?;
                        if value == 0 {
                            len += 255;
                        } else {
                            len += 15 + value;
                            break;
                        }
                    }
                }
                lzo_literals(src, &mut ip, &mut out, len + 3)?;
                state = State::FirstRun;
            }
            State::FirstRun => {
                let code = lzo_byte(src, &mut ip)?;
                if code >= 16 {
                    state = State::Match(code);
                    continue;
                }
                let low = lzo_byte(src, &mut ip)?;
                let pos = out.len() as isize - 1 - 0x0800 - (code >> 2) as isize - ((low << 2) as isize);
                lzo_back_copy(&mut out, pos, 3)?;
                state = State::Done;
            }
            State::Match(code) => {
                let current = out.len() as isize;
                if code >= 64 {
                    let low = lzo_byte(src, &mut ip)?;
                    let pos = current - 1 - ((code >> 2) & 7) as isize - ((low << 3) as isize);
                    lzo_back_copy(&mut out, pos, (code >> 5) - 1 + 2)?;
                } else if code >= 32 {
                    let mut len = code & 31;
                    if len == 0 {
                        loop {
                            let value = lzo_byte(src, &mut ip)?;
                            if value == 0 {
                                len += 255;
                            } else {
                                len += 31 + value;
                                break;
                            }
                        }
                    }
                    let low = lzo_byte(src, &mut ip)?;
                    let high = lzo_byte(src, &mut ip)?;
                    let pos = current - 1 - ((low >> 2) + (high << 6)) as isize;
                    lzo_back_copy(&mut out, pos, len + 2)?;
                } else if code >= 16 {
                    let high_bits = ((code & 8) << 11) as isize;
                    let mut len = code & 7;
                    if len == 0 {
                        loop {
                            let value = lzo_byte(src, &mut ip)?;
                            if value == 0 {
                                len += 255;
                            } else {
                                len += 7 + value;
                                break;
                            }
                        }
                    }
                    let low = lzo_byte(src, &mut ip)?;
                    let high = lzo_byte(src, &mut ip)?;
                    let mut pos = current - high_bits - ((low >> 2) + (high << 6)) as isize;
                    // 偏移等于当前输出位置 = 结束标记
                    if pos == current {
                        return Ok(out);
                    }
                    pos -= 0x4000;
                    lzo_back_copy(&mut out, pos, len + 2)?;
                } else {
                    let low = lzo_byte(src, &mut ip)?;
                    let pos = current - 1 - (code >> 2) as isize - ((low << 2) as isize);
                    lzo_back_copy(&mut out, pos, 2)?;
                }
                state = State::Done;
            }
            State::Done => {
                if ip < 2 {
                    return Err("LZO 数据流损坏".to_string());
                }
                // 上一个控制字节的低 2 位 = 后面跟的字面量个数
                let trailing = (src[ip - 2] & 3) as usize;
                state = if trailing == 0 {
                    State::Outer
                } else {
                    State::Next(trailing)
                };
            }
            State::Next(count) => {
                lzo_literals(src, &mut ip, &mut out, count)?;
                let code = lzo_byte(src, &mut ip)?;
                state = State::Match(code);
            }
        }
        if ip >= src.len() {
            return Ok(out);
        }
    }
}

/// 解掉 8 字节块头并按类型解压。
fn decompress_block(raw: &[u8], hint: usize) -> Result<Vec<u8>, String> {
    if raw.len() < 8 {
        return Err("词典块长度异常".to_string());
    }
    let comp_type = raw[0];
    let payload = &raw[8..];
    match comp_type {
        0 => Ok(payload.to_vec()),
        1 => lzo1x_decompress(payload, hint),
        2 => inflate(payload),
        _ => {
            // 少数老文件没有块头，直接当 zlib 流试一次。
            inflate(raw)
        }
    }
}

fn key_with_suffix(seed: &[u8]) -> [u8; 16] {
    let mut material = seed.to_vec();
    material.extend_from_slice(&[0x95, 0x36, 0x00, 0x00]);
    ripemd128(&material)
}

fn swap_nibble(byte: u8) -> u8 {
    byte.rotate_left(4)
}

/// MDict 的「快速加密」：解密 compressed_data 部分，前 8 字节块头原样保留。
fn nibble_decrypt(raw: &[u8], key: &[u8; 16]) -> Vec<u8> {
    let mut out = raw.to_vec();
    let mut prev: u8 = 0x36;
    for i in 8..out.len() {
        let encrypted = out[i];
        let plain = swap_nibble(encrypted)
            ^ ((i - 8) as u8)
            ^ key[(i - 8) % 16]
            ^ prev;
        out[i] = plain;
        prev = encrypted;
    }
    out
}

/// 索引块可能加密也可能没有，且不同工具的实现细节有差异：
/// 依次尝试「原样 / 用自身校验和派生的密钥解密」，只接受能解压且长度对得上的结果。
fn decode_index_block(raw: &[u8], expected_len: u64) -> Result<Vec<u8>, String> {
    let mut candidates: Vec<Vec<u8>> = vec![raw.to_vec()];
    if raw.len() > 8 {
        candidates.push(nibble_decrypt(raw, &key_with_suffix(&raw[4..8])));
    }
    let mut fallback: Option<Vec<u8>> = None;
    for candidate in candidates {
        if let Ok(data) = decompress_block(&candidate, expected_len as usize) {
            if expected_len == 0 || data.len() as u64 == expected_len {
                return Ok(data);
            }
            fallback = Some(data);
        }
    }
    fallback.ok_or_else(|| "词典索引解密 / 解压失败，可能是加密或 LZO 压缩的词典".to_string())
}

fn parse_key_index(data: &[u8], v2: bool, unit: usize) -> Vec<MdictBlock> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let Some(_count) = read_num(data, &mut pos, v2) else {
            break;
        };
        let first_len = if v2 {
            read_be_u16(data, &mut pos).unwrap_or(0) as usize
        } else {
            if pos >= data.len() {
                break;
            }
            let value = data[pos] as usize;
            pos += 1;
            value
        };
        // 2.x 在文本后多一个终止符（UTF-16 下算 2 字节），1.x 没有。
        let term = if v2 { unit } else { 0 };
        pos += first_len * unit + term;
        let last_len = if v2 {
            read_be_u16(data, &mut pos).unwrap_or(0) as usize
        } else {
            if pos >= data.len() {
                break;
            }
            let value = data[pos] as usize;
            pos += 1;
            value
        };
        pos += last_len * unit + term;
        let Some(compressed) = read_num(data, &mut pos, v2) else {
            break;
        };
        let Some(decompressed) = read_num(data, &mut pos, v2) else {
            break;
        };
        if compressed == 0 || pos > data.len() {
            break;
        }
        out.push(MdictBlock {
            compressed,
            decompressed,
        });
    }
    out
}

fn parse_record_index(data: &[u8], v2: bool) -> Vec<MdictBlock> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        let Some(compressed) = read_num(data, &mut pos, v2) else {
            break;
        };
        let Some(decompressed) = read_num(data, &mut pos, v2) else {
            break;
        };
        if compressed == 0 {
            break;
        }
        out.push(MdictBlock {
            compressed,
            decompressed,
        });
    }
    out
}

impl MdictReader {
    fn open(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("打开词典文件失败: {}", e))?;
        let mut len_buf = [0u8; 4];
        file.read_exact(&mut len_buf)
            .map_err(|_| "词典文件头部损坏".to_string())?;
        let header_len = u32::from_be_bytes(len_buf) as usize;
        if header_len < 4 || header_len > 1_048_576 {
            return Err("词典头长度异常，可能不是 MDict 文件".to_string());
        }
        // 头部长度包含结尾的 \x00\x00，必须按原长度精确读取（不按 4 字节对齐）。
        let mut header_buf = vec![0u8; header_len];
        file.read_exact(&mut header_buf)
            .map_err(|_| "词典头读取不完整".to_string())?;
        let body = if header_buf.ends_with(&[0, 0]) {
            &header_buf[..header_len - 2]
        } else {
            &header_buf[..]
        };
        let header_text: String = String::from_utf16_lossy(
            &body
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<u16>>(),
        );
        let attrs = parse_header_attrs(&header_text);
        let version = attrs
            .get("GeneratedByEngineVersion")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(2.0);
        let v2 = version >= 2.0;
        // 头部之后固定跟 4 字节 ADLER32（1.x 与 2.x 都有）
        let mut checksum = [0u8; 4];
        file.read_exact(&mut checksum)
            .map_err(|_| "词典校验和缺失".to_string())?;

        let is_mdd = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("mdd"))
            .unwrap_or(false)
            || header_text.contains("Library_Data");
        let encoding = if is_mdd {
            encoding_rs::UTF_16LE
        } else {
            attrs
                .get("Encoding")
                .map(|value| value.as_str())
                .and_then(|value| Encoding::for_label(value.as_bytes()))
                .unwrap_or(encoding_rs::UTF_8)
        };
        let unit = if std::ptr::eq(encoding, encoding_rs::UTF_16LE)
            || std::ptr::eq(encoding, encoding_rs::UTF_16BE)
        {
            2
        } else {
            1
        };
        let encrypted = attrs
            .get("Encrypted")
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                if value == "yes" {
                    2
                } else {
                    value.parse::<u32>().unwrap_or(0)
                }
            })
            .unwrap_or(0);
        if encrypted & 1 == 1 {
            return Err(
                "该词典的索引头使用 Salsa20 加密（需要注册码 / 设备 ID），暂不支持导入".to_string(),
            );
        }
        // StyleSheet 是「编号 / 开标签 / 闭标签」三行一组，属性值里用实体转义过。
        let mut stylesheet = std::collections::HashMap::new();
        if let Some(raw) = attrs.get("StyleSheet") {
            let decoded = decode_entities(raw).replace("\r\n", "\n");
            let lines: Vec<&str> = decoded.split('\n').collect();
            for chunk in lines.chunks(3) {
                if chunk.len() < 3 {
                    break;
                }
                let id = chunk[0].trim();
                if id.is_empty() {
                    continue;
                }
                stylesheet.insert(id.to_string(), (chunk[1].to_string(), chunk[2].to_string()));
            }
        }

        // key 区头部：1.x 是 4 个数字（无 adler），2.x 是 5 个数字 + 4 字节 adler。
        let num_key_blocks = read_file_num(&mut file, v2)?;
        let _num_entries = read_file_num(&mut file, v2)?;
        let mut key_index_decomp_len = 0;
        let key_index_comp_len;
        let key_blocks_len;
        if v2 {
            key_index_decomp_len = read_file_num(&mut file, v2)?;
            key_index_comp_len = read_file_num(&mut file, v2)?;
            key_blocks_len = read_file_num(&mut file, v2)?;
            let mut checksum = [0u8; 4];
            file.read_exact(&mut checksum)
                .map_err(|_| "词典索引校验和缺失".to_string())?;
        } else {
            key_index_comp_len = read_file_num(&mut file, v2)?;
            key_blocks_len = read_file_num(&mut file, v2)?;
        }
        if key_index_comp_len == 0 || key_index_comp_len > 256 * 1024 * 1024 {
            return Err("词典 key 索引长度异常".to_string());
        }
        let mut key_index_raw = vec![0u8; key_index_comp_len as usize];
        file.read_exact(&mut key_index_raw)
            .map_err(|_| "词典 key 索引读取失败".to_string())?;
        // 1.x 的 key block info 是明文数字数组，2.x 才是 zlib 块
        let key_index = if v2 {
            decode_index_block(&key_index_raw, key_index_decomp_len)?
        } else {
            key_index_raw
        };
        let key_blocks = parse_key_index(&key_index, v2, unit);
        if key_blocks.is_empty() {
            return Err("词典 key 索引为空".to_string());
        }
        if num_key_blocks > 0 && key_blocks.len() as u64 != num_key_blocks {
            // 只告警不失败：个别词典的计数不准，但块信息仍然可用。
            tracing::warn!(
                expected = num_key_blocks,
                parsed = key_blocks.len(),
                "mdict key block count mismatch"
            );
        }
        let key_blocks_start = file
            .stream_position()
            .map_err(|e| format!("定位词典失败: {}", e))?;

        // record 区紧跟在所有 key block 之后
        let record_head_pos = key_blocks_start + key_blocks_len;
        file.seek(SeekFrom::Start(record_head_pos))
            .map_err(|e| format!("定位 record 区失败: {}", e))?;
        let num_record_blocks = read_file_num(&mut file, v2)?;
        let _entries = read_file_num(&mut file, v2)?;
        let record_index_len = read_file_num(&mut file, v2)?;
        let _record_blocks_len = read_file_num(&mut file, v2)?;
        if record_index_len > 512 * 1024 * 1024 {
            return Err("词典 record 索引长度异常".to_string());
        }
        // record block info 在 1.x / 2.x 都是明文数字数组（不压缩、不加密）
        let mut record_index_raw = vec![0u8; record_index_len as usize];
        file.read_exact(&mut record_index_raw)
            .map_err(|_| "词典 record 索引读取失败".to_string())?;
        let record_blocks = parse_record_index(&record_index_raw, v2);
        if record_blocks.is_empty() {
            return Err("词典 record 索引为空".to_string());
        }
        if num_record_blocks > 0 && record_blocks.len() as u64 != num_record_blocks {
            tracing::warn!(
                expected = num_record_blocks,
                parsed = record_blocks.len(),
                "mdict record block count mismatch"
            );
        }
        let record_blocks_start = file
            .stream_position()
            .map_err(|e| format!("定位 record 区失败: {}", e))?;
        let mut record_starts = Vec::with_capacity(record_blocks.len());
        let mut record_file_starts = Vec::with_capacity(record_blocks.len());
        let mut acc = 0u64;
        let mut file_acc = 0u64;
        for block in &record_blocks {
            record_starts.push(acc);
            record_file_starts.push(file_acc);
            acc += block.decompressed;
            file_acc += block.compressed;
        }

        Ok(Self {
            file,
            encoding,
            v2,
            unit,
            key_blocks,
            key_blocks_start,
            record_blocks,
            record_blocks_start,
            record_starts,
            record_file_starts,
            total_record_size: acc,
            cache: None,
            stylesheet,
        })
    }

    /// 把正文里的 `` `N` `` 标记换成头里声明的样式标签。
    /// 朗文、21 世纪这类中文词库都靠它排版，不去替换正文里会残留一堆反引号。
    fn apply_styles(&self, text: &str) -> String {
        if self.stylesheet.is_empty() || !text.contains('`') {
            return text.to_string();
        }
        let re = re_style_marker();
        let ids: Vec<String> = re
            .captures_iter(text)
            .map(|cap| cap[1].to_string())
            .collect();
        if ids.is_empty() {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len() + ids.len() * 24);
        for (index, segment) in re.split(text).enumerate() {
            if index == 0 {
                out.push_str(segment);
                continue;
            }
            match ids.get(index - 1).and_then(|id| self.stylesheet.get(id)) {
                Some((open, close)) => {
                    out.push_str(open);
                    out.push_str(segment);
                    out.push_str(close);
                }
                None => out.push_str(segment),
            }
        }
        out
    }

    fn collect_keys(&mut self) -> Result<Vec<(String, u64)>, String> {
        let mut out = Vec::new();
        let mut block_pos = self.key_blocks_start;
        let v2 = self.v2;
        let unit = self.unit;
        let encoding = self.encoding;
        for block in &self.key_blocks {
            self.file
                .seek(SeekFrom::Start(block_pos))
                .map_err(|e| format!("读取词典 key 块失败: {}", e))?;
            let mut raw = vec![0u8; block.compressed as usize];
            self.file
                .read_exact(&mut raw)
                .map_err(|_| "词典 key 块不完整".to_string())?;
            block_pos += block.compressed;
            let data = decompress_block(&raw, block.decompressed as usize)?;
            let mut pos = 0usize;
            while pos < data.len() {
                let Some(offset) = read_num(&data, &mut pos, v2) else {
                    break;
                };
                let start = pos;
                let mut end = None;
                if unit == 2 {
                    while pos + 1 < data.len() {
                        if data[pos] == 0 && data[pos + 1] == 0 {
                            end = Some(pos);
                            pos += 2;
                            break;
                        }
                        pos += 2;
                    }
                } else {
                    while pos < data.len() {
                        if data[pos] == 0 {
                            end = Some(pos);
                            pos += 1;
                            break;
                        }
                        pos += 1;
                    }
                }
                let Some(end) = end else { break };
                if end <= start {
                    continue;
                }
                let key = decode(&data[start..end], encoding);
                if !key.trim().is_empty() {
                    out.push((key, offset));
                }
            }
        }
        Ok(out)
    }

    fn block_of(&self, offset: u64) -> Option<usize> {
        if self.record_blocks.is_empty() {
            return None;
        }
        match self.record_starts.binary_search(&offset) {
            Ok(i) => Some(i),
            Err(0) => Some(0),
            Err(i) => Some(i - 1),
        }
    }

    fn block_data(&mut self, idx: usize) -> Result<Vec<u8>, String> {
        if let Some((cached_idx, data)) = &self.cache {
            if *cached_idx == idx {
                return Ok(data.clone());
            }
        }
        let (compressed, decompressed) = {
            let block = self
                .record_blocks
                .get(idx)
                .ok_or_else(|| "词典 record 块越界".to_string())?;
            (block.compressed, block.decompressed)
        };
        self.file
            .seek(SeekFrom::Start(
                self.record_blocks_start + self.record_file_starts[idx],
            ))
            .map_err(|e| format!("定位 record 块失败: {}", e))?;
        let mut raw = vec![0u8; compressed as usize];
        self.file
            .read_exact(&mut raw)
            .map_err(|_| "record 块读取不完整".to_string())?;
        // 每个 record 块都以 4 字节压缩类型开头（`\x00|\x01|\x02` + 三个 0）。
        // 用「长度不等」判断是否压缩并不可靠：LZO 压缩后大小恰好等于原大小是可能的。
        let has_block_header = raw.len() >= 8 && raw[1..4] == [0, 0, 0];
        let data = if has_block_header && raw[0] <= 2 {
            decompress_block(&raw, decompressed as usize)?
        } else {
            raw
        };
        self.cache = Some((idx, data.clone()));
        Ok(data)
    }

    fn read_range(&mut self, start: u64, end: u64) -> Result<Vec<u8>, String> {
        let end = end.min(self.total_record_size).max(start);
        let mut out = Vec::with_capacity((end - start) as usize);
        let mut cursor = start;
        while cursor < end {
            let idx = self
                .block_of(cursor)
                .ok_or_else(|| "词典 record 偏移越界".to_string())?;
            let base = self.record_starts[idx];
            let data = self.block_data(idx)?;
            let local = (cursor - base) as usize;
            if local >= data.len() {
                break;
            }
            let take = ((end - cursor) as usize).min(data.len() - local);
            out.extend_from_slice(&data[local..local + take]);
            cursor += take as u64;
        }
        Ok(out)
    }

    fn read_cstring(&mut self, start: u64, max: usize) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut cursor = start;
        let unit = self.unit;
        while out.len() < max && cursor < self.total_record_size {
            let idx = self
                .block_of(cursor)
                .ok_or_else(|| "词典 record 偏移越界".to_string())?;
            let base = self.record_starts[idx];
            let data = self.block_data(idx)?;
            let local = (cursor - base) as usize;
            if local >= data.len() {
                break;
            }
            let slice = &data[local..];
            let mut pos = 0usize;
            let mut hit = false;
            if unit == 2 {
                while pos + 1 < slice.len() {
                    if slice[pos] == 0 && slice[pos + 1] == 0 {
                        hit = true;
                        break;
                    }
                    pos += 2;
                }
            } else {
                while pos < slice.len() {
                    if slice[pos] == 0 {
                        hit = true;
                        break;
                    }
                    pos += 1;
                }
            }
            out.extend_from_slice(&slice[..pos]);
            if hit {
                return Ok(out);
            }
            cursor = base + data.len() as u64;
        }
        Ok(out)
    }
}

/// 这些正则会在每个词条上跑一遍，十几万词条的词库就是百万次调用——
/// 必须编译一次复用，否则光编译正则就能拖到几分钟。
macro_rules! cached_regex {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static CELL: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
            CELL.get_or_init(|| Regex::new($pattern).unwrap())
        }
    };
}

cached_regex!(re_header_attr, r#"([A-Za-z_][\w.]*)\s*=\s*"([^"]*)""#);
cached_regex!(
    re_inline_event,
    r#"(?i)\son[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)"#
);
cached_regex!(
    re_js_url,
    r#"(?i)(href|src)\s*=\s*("\s*javascript:[^"]*"|'\s*javascript:[^']*')"#
);
cached_regex!(re_entity_num, r"&#(x?)([0-9a-fA-F]+);");
cached_regex!(re_line_break, r"(?i)<(br|/p|/div|/li|/tr|/h[1-6]|hr)\s*/?>");
cached_regex!(re_html_tag, r"(?s)<[^>]*>");
cached_regex!(re_link_tag, r#"(?is)<link[^>]*>"#);
cached_regex!(re_dsl_tag, r"(?i)\[\s*(/?)\s*([a-z0-9*]+)(?:[^\]]*)\]");
cached_regex!(re_res_attr, r#"(?i)(src|href)\s*=\s*("([^"]*)"|'([^']*)')"#);
cached_regex!(re_css_url, r#"(?i)url\(\s*['"]?([^'")]+)['"]?\s*\)"#);
cached_regex!(
    re_phonetic_class,
    r#"(?i)(?:class\s*=\s*"[^"]*phonetic[^"]*"[^>]*>\s*)([^<]{1,48})"#
);
cached_regex!(
    re_phonetic_slash,
    r"/([A-Za-zɪəʊæɑʌɜːθðʃʒŋˈˌ\s'.,\-]{1,48})/"
);
cached_regex!(
    re_pos_prefix,
    r"(?i)^\s*(n|v|vt|vi|adj|adv|prep|pron|conj|num|art|int|aux|abbr)\s*\."
);
cached_regex!(re_style_marker, r"`(\d+)`");
cached_regex!(
    re_phonetic_face,
    r#"(?i)<font[^>]*face\s*=\s*"[^"]*phonetic[^"]*"[^>]*>\s*([^<]{1,48})</font>"#
);

fn parse_header_attrs(header: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for cap in re_header_attr().captures_iter(header) {
        out.insert(cap[1].to_string(), cap[2].to_string());
    }
    out
}

/// 解码后的 HTML 里可能带脚本（字典本身不该有，但导入的文件不可信）。
pub fn sanitize_html(html: &str) -> String {
    let mut out = html.to_string();
    for (open, close) in [
        ("<script", "</script>"),
        ("<iframe", "</iframe>"),
        ("<object", "</object>"),
        ("<embed", "</embed>"),
    ] {
        let mut guard = 0;
        while guard < 32 {
            guard += 1;
            let lower = out.to_lowercase();
            let Some(start) = lower.find(open) else { break };
            let Some(rel_end) = lower[start..].find(close) else {
                out.truncate(start);
                break;
            };
            let end = start + rel_end + close.len();
            out.replace_range(start..end, "");
        }
    }
    let out = re_inline_event().replace_all(&out, "").to_string();
    re_js_url().replace_all(&out, "").to_string()
}

fn decode_entities(text: &str) -> String {
    let mut out = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    out = re_entity_num()
        .replace_all(&out, |caps: &regex::Captures| {
            let radix = if &caps[1] == "x" { 16 } else { 10 };
            u32::from_str_radix(&caps[2], radix)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string())
                .unwrap_or_default()
        })
        .to_string();
    out
}

/// HTML → 纯文本：保留段落换行，丢掉样式与脚本。
pub fn html_to_text(html: &str) -> String {
    let cleaned = sanitize_html(html);
    let with_breaks = re_line_break().replace_all(&cleaned, "\n").to_string();
    let text = decode_entities(&re_html_tag().replace_all(&with_breaks, ""));
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.replace('\u{00a0}', " ");
        let trimmed = trimmed.trim();
        if trimmed.is_empty() {
            if lines.last().map(|l: &String| !l.is_empty()).unwrap_or(false) {
                lines.push(String::new());
            }
            continue;
        }
        lines.push(trimmed.to_string());
    }
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

fn extract_phonetic(html: &str) -> String {
    if let Some(cap) = re_phonetic_class().captures(html) {
        let value = cap[1].trim().trim_matches('/').trim();
        if !value.is_empty() {
            return format!("/{}/", value);
        }
    }
    // 中文排版型词库把音标放在 face="... Phonetic ..." 的字体里
    if let Some(cap) = re_phonetic_face().captures(html) {
        let value = cap[1].trim().trim_matches('/').trim();
        if !value.is_empty() {
            return format!("/{}/", value);
        }
    }
    if let Some(cap) = re_phonetic_slash().captures(&html_to_text(html)) {
        return cap[0].trim().to_string();
    }
    String::new()
}

fn extract_pos(text: &str) -> String {
    re_pos_prefix()
        .captures(text)
        .map(|c| format!("{}.", c[1].to_lowercase()))
        .unwrap_or_default()
}

fn meanings_from_text(text: &str, term: &str) -> Vec<String> {
    // 排版型词库（朗文系、21 世纪系）会把一个词条拆成「词头 / 音标 / 词性 / 义项」，
    // 摘要只需要义项。够长的行优先；只有一行长行时说明那可能是例句，
    // 这时保留原顺序把短行也带上，免得释义被例句挤掉。
    let mut lines: Vec<(String, bool)> = Vec::new();
    let mut strong_count = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.eq_ignore_ascii_case(term) {
            continue;
        }
        // 纯音标行没有释义价值
        if line.starts_with('/') && line.ends_with('/') && line.len() < 48 {
            continue;
        }
        // `<<名詞>>` 这类是词性标注
        if line.starts_with("<<") && line.ends_with(">>") {
            continue;
        }
        let clipped = if line.chars().count() > 220 {
            line.chars().take(220).collect::<String>()
        } else {
            line.to_string()
        };
        let strong = line.chars().count() >= 8;
        if strong {
            strong_count += 1;
        }
        lines.push((clipped, strong));
        if lines.len() >= 20 {
            break;
        }
    }
    let keep_weak = strong_count < 2;
    let mut out: Vec<String> = lines
        .into_iter()
        .filter(|(_, strong)| keep_weak || *strong)
        .map(|(line, _)| line)
        .collect();
    out.truncate(10);
    out
}

/// MDX：逐条回调 (term, html)，避免整本词典驻留内存。
pub fn parse_mdx(
    path: &Path,
    mut sink: impl FnMut(RawEntry) -> Result<(), String>,
) -> Result<i64, String> {
    let mut reader = MdictReader::open(path)?;
    let keys = reader.collect_keys()?;
    let mut count = 0i64;
    let mut buffer = Vec::new();

    for (key, offset) in keys {
        let bytes = match reader.read_cstring(offset, 512 * 1024) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if bytes.is_empty() {
            continue;
        }
        let html_raw = decode(&bytes, reader.encoding);
        let term = key.trim().to_string();
        if term.is_empty() {
            continue;
        }
        let html_raw = html_raw.trim_end_matches('\u{0}').to_string();
        // `@@@LINK=` 是纯跳转，不做样式替换
        let html_raw = reader.apply_styles(&html_raw);
        if let Some(target) = html_raw.trim().strip_prefix("@@@LINK=") {
            let target = target.lines().next().unwrap_or("").trim().to_string();
            if !target.is_empty() {
                sink(RawEntry {
                    term,
                    link: Some(target),
                    ..Default::default()
                })?;
                count += 1;
            }
            continue;
        }
        // MDict 的 <link rel="stylesheet"> 在本地渲染没有意义，去掉免得样式串台。
        let html = re_link_tag().replace_all(&html_raw, "").to_string();
        let text = html_to_text(&html);
        let meanings = meanings_from_text(&text, &term);
        let raw = RawEntry {
            phonetic: extract_phonetic(&html),
            pos: extract_pos(&text),
            meanings,
            definition: sanitize_html(&html).trim().to_string(),
            term,
            ..Default::default()
        };
        buffer.push(raw);
        if buffer.len() >= 2000 {
            count += buffer.len() as i64;
            for entry in buffer.drain(..) {
                sink(entry)?;
            }
        }
    }
    count += buffer.len() as i64;
    for entry in buffer {
        sink(entry)?;
    }
    Ok(count)
}

fn is_resource_ext(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    const OK: [&str; 12] = [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".svg", ".css", ".woff", ".woff2",
        ".ttf", ".otf",
    ];
    OK.iter().any(|ext| lower.ends_with(ext))
}

fn safe_relative_path(raw: &str) -> Option<PathBuf> {
    let cleaned = raw.trim().trim_start_matches(['\\', '/']).replace('\\', "/");
    if cleaned.is_empty() {
        return None;
    }
    let mut out = PathBuf::new();
    for part in cleaned.split('/') {
        let part = part.trim();
        if part.is_empty() || part == "." || part == ".." {
            continue;
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

/// MDD：把图片 / 样式解包到 `<pack>/resources/`，正文里的相对引用按路径命中。
pub fn parse_mdd(
    path: &Path,
    res_dir: &Path,
    max_total_bytes: u64,
    mut on_saved: impl FnMut(&str, u64) -> Result<(), String>,
) -> Result<i64, String> {
    let mut reader = MdictReader::open(path)?;
    let keys = reader.collect_keys()?;
    fs::create_dir_all(res_dir).map_err(|e| format!("创建资源目录失败: {}", e))?;
    let mut saved = 0i64;
    let mut total = 0u64;
    let mut order: Vec<(String, u64)> = keys;
    // record 偏移按 key 顺序递增；排序后可用「下一条的起点」精确切分二进制资源。
    order.sort_by_key(|(_, offset)| *offset);
    let count = order.len();
    for i in 0..count {
        let (key, start) = order[i].clone();
        if !is_resource_ext(&key) {
            continue;
        }
        let end = order.get(i + 1).map(|(_, o)| *o).unwrap_or(reader.total_record_size);
        if end <= start {
            continue;
        }
        let size = end - start;
        if size > 8 * 1024 * 1024 || total + size > max_total_bytes {
            continue;
        }
        let Some(rel) = safe_relative_path(&key) else {
            continue;
        };
        let Some(name) = rel.file_name().map(|x| x.to_string_lossy().to_string()) else {
            continue;
        };
        let target_dir = res_dir.join(rel.parent().unwrap_or(Path::new("")));
        if fs::create_dir_all(&target_dir).is_err() {
            continue;
        }
        let Ok(mut data) = reader.read_range(start, end) else {
            continue;
        };
        while data.last() == Some(&0) {
            data.pop();
        }
        if data.is_empty() {
            continue;
        }
        if fs::write(target_dir.join(name), &data).is_ok() {
            total += size;
            saved += 1;
            if saved % 200 == 0 {
                on_saved(&key, total)?;
            }
        }
    }
    Ok(saved)
}

// ── StarDict ───────────────────────────────────────────────────────────
fn read_text_any(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;
    Ok(decode_auto(&bytes))
}

/// 猜编码：有 BOM 用 BOM，否则试 UTF-8，失败按 GB18030。
pub fn decode_auto(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        return text.into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        return text.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => decode(bytes, encoding_rs::GB18030),
    }
}

fn gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    GzDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| format!("解压失败: {}", e))?;
    Ok(out)
}

fn stardict_push(kind: char, body: &[u8], text_parts: &mut Vec<String>, html_parts: &mut Vec<String>) {
    let value = decode_auto(body).trim().to_string();
    if value.is_empty() {
        return;
    }
    match kind {
        'g' | 'h' | 'x' | 'k' | 'w' => {
            html_parts.push(sanitize_html(&value));
            text_parts.push(html_to_text(&value));
        }
        'P' | 'X' | 'W' => {}
        _ => {
            text_parts.push(value.clone());
            html_parts.push(escape_html(&value).replace('\n', "<br>"));
        }
    }
}

fn stardict_payload(data: &[u8], same_seq: &str) -> (String, String) {
    let mut text_parts: Vec<String> = Vec::new();
    let mut html_parts: Vec<String> = Vec::new();

    if !same_seq.is_empty() && same_seq.len() == 1 {
        stardict_push(
            same_seq.chars().next().unwrap_or('m'),
            data,
            &mut text_parts,
            &mut html_parts,
        );
    } else if !same_seq.is_empty() {
        // 多字段类型串：定长类型（W/P/X）无法从后往前切，退化为整体按纯文本处理。
        stardict_push('m', data, &mut text_parts, &mut html_parts);
    } else {
        let mut pos = 0usize;
        while pos < data.len() {
            let kind = data[pos] as char;
            pos += 1;
            if kind.is_ascii_uppercase() && kind != 'W' && kind != 'P' && kind != 'X' {
                break;
            }
            if kind == 'W' {
                if pos + 4 <= data.len() {
                    let len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                    pos += 4 + len;
                } else {
                    break;
                }
                continue;
            }
            if kind == 'P' {
                if pos + 4 <= data.len() {
                    let len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                        as usize;
                    pos += 4 + len;
                } else {
                    break;
                }
                continue;
            }
            let start = pos;
            while pos < data.len() && data[pos] != 0 {
                pos += 1;
            }
            stardict_push(kind, &data[start..pos], &mut text_parts, &mut html_parts);
            if pos < data.len() {
                pos += 1;
            }
        }
    }
    (text_parts.join("\n"), html_parts.join("<br>"))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn parse_stardict(
    ifo_path: &Path,
    mut sink: impl FnMut(RawEntry) -> Result<(), String>,
) -> Result<(i64, String, String), String> {
    let ifo_text = read_text_any(ifo_path)?;
    let base = ifo_path.with_extension("");
    let mut book_name = String::new();
    let mut same_seq = String::new();
    let mut idx_name = String::new();
    let mut dict_name = String::new();
    for line in ifo_text.lines() {
        let lower = line.to_ascii_lowercase();
        let value = line
            .split_once('=')
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default();
        if lower.starts_with("bookname=") {
            book_name = value.clone();
        } else if lower.starts_with("sametypesequence=") {
            same_seq = value.clone();
        } else if lower.starts_with("idxfile=") {
            idx_name = value.clone();
        } else if lower.starts_with("dictfile=") {
            dict_name = value.clone();
        }
    }
    let idx_path = if idx_name.is_empty() {
        find_sidecar(&base, "idx")
    } else {
        Some(ifo_path.with_file_name(idx_name))
    }
    .ok_or_else(|| "StarDict 缺少 .idx 索引".to_string())?;
    let dict_path = if dict_name.is_empty() {
        find_sidecar(&base, "dict")
    } else {
        Some(ifo_path.with_file_name(dict_name))
    }
    .ok_or_else(|| "StarDict 缺少 .dict 数据".to_string())?;

    let idx_raw = fs::read(&idx_path).map_err(|e| format!("读取 .idx 失败: {}", e))?;
    let idx_bytes = if idx_path.extension().and_then(|x| x.to_str()) == Some("gz") {
        gunzip(&idx_raw)?
    } else {
        idx_raw
    };
    let dict_raw = fs::read(&dict_path).map_err(|e| format!("读取 .dict 失败: {}", e))?;
    let dict_bytes = if dict_path.to_string_lossy().to_ascii_lowercase().ends_with(".dz") {
        gunzip(&dict_raw)?
    } else {
        dict_raw
    };

    let mut count = 0i64;
    let mut pos = 0usize;
    while pos < idx_bytes.len() {
        let start = pos;
        while pos < idx_bytes.len() && idx_bytes[pos] != 0 {
            pos += 1;
        }
        if pos >= idx_bytes.len() {
            break;
        }
        let word = decode_auto(&idx_bytes[start..pos]);
        pos += 1;
        if pos + 8 > idx_bytes.len() {
            break;
        }
        let offset = u32::from_be_bytes([idx_bytes[pos], idx_bytes[pos + 1], idx_bytes[pos + 2], idx_bytes[pos + 3]])
            as usize;
        let size = u32::from_be_bytes([idx_bytes[pos + 4], idx_bytes[pos + 5], idx_bytes[pos + 6], idx_bytes[pos + 7]])
            as usize;
        pos += 8;
        if offset >= dict_bytes.len() {
            continue;
        }
        let end = (offset + size).min(dict_bytes.len());
        let (text, html) = stardict_payload(&dict_bytes[offset..end], &same_seq);
        let meanings = meanings_from_text(&text, word.trim());
        if meanings.is_empty() {
            continue;
        }
        sink(RawEntry {
            term: word.trim().to_string(),
            pos: extract_pos(&text),
            meanings,
            definition: html,
            ..Default::default()
        })?;
        count += 1;
    }
    Ok((count, book_name, same_seq))
}

fn find_sidecar(base: &Path, ext: &str) -> Option<PathBuf> {
    for candidate in [
        base.with_extension(ext),
        base.with_extension(format!("{}.gz", ext)),
        base.with_extension(format!("{}.dz", ext)),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ── ABBYY / Lingoes DSL ────────────────────────────────────────────────
fn dsl_line_to_html(line: &str) -> String {
    let escaped = escape_html(line);
    let tag = re_dsl_tag();
    tag.replace_all(&escaped, |caps: &regex::Captures| {
        let closing = &caps[1] == "/";
        let name = caps[2].to_ascii_lowercase();
        let mapped = match name.as_str() {
            "b" => "b",
            "i" => "i",
            "u" => "u",
            "m" | "m1" | "m2" | "m3" => "div",
            "trn" => "span class=\"dsl-trn\"",
            "ex" | "t" => "span class=\"dsl-ex\"",
            "p" => "span class=\"dsl-pos\"",
            "c" => "span class=\"dsl-color\"",
            "*" => "span class=\"dsl-note\"",
            "ref" | "url" => "span class=\"dsl-ref\"",
            _ => return String::new(),
        };
        let tag_name = mapped.split(' ').next().unwrap_or("span");
        if closing {
            format!("</{}>", tag_name)
        } else {
            format!("<{}>", mapped)
        }
    })
    .to_string()
}

pub fn parse_dsl(path: &Path, mut sink: impl FnMut(RawEntry) -> Result<(), String>) -> Result<(i64, String, String, String), String> {
    let text = read_text_any(path)?;
    let mut name = String::new();
    let mut source_lang = String::new();
    let mut target_lang = String::new();
    let mut headword = String::new();
    let mut body: Vec<String> = Vec::new();
    let mut count = 0i64;

    let flush = |headword: &mut String, body: &mut Vec<String>, sink: &mut dyn FnMut(RawEntry) -> Result<(), String>, count: &mut i64| -> Result<(), String> {
        if headword.trim().is_empty() {
            body.clear();
            return Ok(());
        }
        let term = headword.trim().to_string();
        let html_body = body
            .iter()
            .map(|line| dsl_line_to_html(line.trim()))
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("<br>");
        let plain = html_to_text(&html_body);
        let meanings = meanings_from_text(&plain, &term);
        if !meanings.is_empty() {
            sink(RawEntry {
                term,
                pos: extract_pos(&plain),
                meanings,
                definition: html_body,
                ..Default::default()
            })?;
            *count += 1;
        }
        headword.clear();
        body.clear();
        Ok(())
    };

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            flush(&mut headword, &mut body, &mut sink, &mut count)?;
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("#name") {
                name = trimmed.trim_start_matches(|c: char| c != '"').trim_matches('"').to_string();
            } else if lower.starts_with("#index_language") || lower.starts_with("#source_language") {
                source_lang = trimmed.split('"').nth(1).unwrap_or("").to_string();
            } else if lower.starts_with("#contents_language") || lower.starts_with("#target_language") {
                target_lang = trimmed.split('"').nth(1).unwrap_or("").to_string();
            }
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            body.push(trimmed.to_string());
            continue;
        }
        // 顶格行 = 新词条头
        flush(&mut headword, &mut body, &mut sink, &mut count)?;
        headword = line.trim().to_string();
    }
    flush(&mut headword, &mut body, &mut sink, &mut count)?;
    Ok((count, name, source_lang, target_lang))
}

// ── Wiktionary (kaikki.org) JSONL ───────────────────────────────────────
pub fn parse_wiktionary_jsonl(
    path: &Path,
    mut sink: impl FnMut(RawEntry) -> Result<(), String>,
) -> Result<i64, String> {
    let file = File::open(path).map_err(|e| format!("打开 JSONL 词库失败: {}", e))?;
    let reader: Box<dyn BufRead> = if path.to_string_lossy().ends_with(".gz") {
        Box::new(BufReader::new(GzDecoder::new(file)))
    } else {
        Box::new(BufReader::with_capacity(1 << 20, file))
    };
    let mut count = 0i64;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(term) = value.get("word").and_then(|v| v.as_str()).map(|v| v.trim().to_string()) else {
            continue;
        };
        if term.is_empty() {
            continue;
        }
        let pos = value.get("pos").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let key = format!("{}|{}", term.to_lowercase(), pos);
        if !seen.insert(key) {
            continue;
        }
        let mut meanings: Vec<String> = Vec::new();
        let mut examples: Vec<String> = Vec::new();
        if let Some(senses) = value.get("senses").and_then(|v| v.as_array()) {
            for sense in senses {
                if let Some(glosses) = sense.get("glosses").and_then(|v| v.as_array()) {
                    for gloss in glosses {
                        if let Some(g) = gloss.as_str() {
                            let g = g.trim();
                            if !g.is_empty() && !meanings.iter().any(|m| m == g) {
                                meanings.push(g.to_string());
                            }
                        }
                    }
                }
                if let Some(items) = sense.get("examples").and_then(|v| v.as_array()) {
                    for item in items {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            let text = text.trim();
                            if !text.is_empty() && examples.len() < 12 {
                                examples.push(text.to_string());
                            }
                        }
                    }
                }
                if meanings.len() >= 10 {
                    break;
                }
            }
        }
        if meanings.is_empty() {
            continue;
        }
        let mut synonyms: Vec<String> = Vec::new();
        if let Some(senses) = value.get("senses").and_then(|v| v.as_array()) {
            for sense in senses {
                if let Some(items) = sense.get("synonyms").and_then(|v| v.as_array()) {
                    for item in items {
                        if let Some(word) = item.get("word").and_then(|v| v.as_str()) {
                            let word = word.trim();
                            if !word.is_empty() && !synonyms.iter().any(|s| s == word) {
                                synonyms.push(word.to_string());
                            }
                        }
                    }
                }
            }
        }
        let mut phonetic = String::new();
        if let Some(sounds) = value.get("sounds").and_then(|v| v.as_array()) {
            for sound in sounds {
                if let Some(ipa) = sound.get("ipa").and_then(|v| v.as_str()) {
                    let ipa = ipa.trim();
                    if !ipa.is_empty() {
                        phonetic = format!("/{}/", ipa.trim_matches('/'));
                        break;
                    }
                }
            }
        }
        let html = {
            let mut parts = Vec::new();
            if !pos.is_empty() {
                parts.push(format!("<div class=\"dsl-pos\">{}</div>", escape_html(&pos)));
            }
            for meaning in &meanings {
                parts.push(format!("<div>{}</div>", escape_html(meaning)));
            }
            parts.join("")
        };
        sink(RawEntry {
            term,
            phonetic,
            pos: pos.clone(),
            meanings,
            examples,
            synonyms,
            definition: html,
            link: None,
        })?;
        count += 1;
    }
    Ok(count)
}

// ── 资源引用改写 ────────────────────────────────────────────────────────
fn mime_for(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".css") {
        "text/css"
    } else if lower.ends_with(".woff2") {
        "font/woff2"
    } else if lower.ends_with(".woff") {
        "font/woff"
    } else if lower.ends_with(".ttf") {
        "font/ttf"
    } else if lower.ends_with(".otf") {
        "font/otf"
    } else {
        "application/octet-stream"
    }
}

fn load_resource(res_dir: &Path, raw_ref: &str) -> Option<String> {
    let cleaned = raw_ref
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['"', '\''])
        .to_string();
    if cleaned.is_empty() || cleaned.starts_with("data:") || cleaned.starts_with("http") {
        return None;
    }
    let rel = safe_relative_path(&cleaned)?;
    let mut candidates = vec![res_dir.join(&rel)];
    // 正文里的引用常常写成 `\image\x.png` 或 `image/x.png`，两层都试一遍。
    if let Some(name) = rel.file_name() {
        candidates.push(res_dir.join(name));
    }
    for candidate in candidates {
        let Ok(meta) = fs::metadata(&candidate) else { continue };
        if !meta.is_file() || meta.len() > 4 * 1024 * 1024 {
            continue;
        }
        let Ok(bytes) = fs::read(&candidate) else { continue };
        let mime = mime_for(&candidate.to_string_lossy());
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        return Some(format!("data:{};base64,{}", mime, encoded));
    }
    None
}

/// 把 HTML 里的 `src` / `href` / `url(...)` 指向的本地资源换成 data URI。
/// 词典正文是离线包自带的图片，没必要再走自定义协议。
pub fn resolve_resources(html: &str, res_dir: &Path) -> String {
    if html.is_empty() || !res_dir.is_dir() {
        return html.to_string();
    }
    let attr = re_res_attr();
    let with_attr = attr
        .replace_all(html, |caps: &regex::Captures| {
            let name = caps[1].to_string();
            let raw = caps.get(3).or_else(|| caps.get(4)).map(|m| m.as_str()).unwrap_or("");
            match load_resource(res_dir, raw) {
                Some(data) => format!("{}=\"{}\"", name, data),
                None => caps[0].to_string(),
            }
        })
        .to_string();
    let css = re_css_url();
    css.replace_all(&with_attr, |caps: &regex::Captures| {
        match load_resource(res_dir, &caps[1]) {
            Some(data) => format!("url(\"{}\")", data),
            None => caps[0].to_string(),
        }
    })
    .to_string()
}

/// 供导入时按扩展名判断格式。
pub fn detect_format(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "mdx" => "mdx",
        "mdd" => "mdd",
        "ifo" => "stardict",
        "dsl" => "dsl",
        "jsonl" => "wiktionary",
        "csv" => "csv",
        "json" => "json",
        "zip" => "zip",
        _ => "",
    }
}
