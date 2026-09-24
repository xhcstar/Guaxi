//! 网络工具箱 —— 报文解析 / 构造（批 N1）。
//!
//! 用 etherparse（纯 Rust）解析与构造以太网/IPv4/IPv6/TCP/UDP/ICMP 报文：
//! - 解析：粘贴 hex/base64 → 分层字段树（Wireshark 式）+ 载荷。
//! - 构造：填写各层字段 + 载荷，etherparse 自动计算 IP/TCP/UDP/ICMP 校验码 → hex。
//! 说明：原始套接字收发（L2 注入/抓包、ICMP 直发）在 Windows 需 Npcap + 管理员，
//! 与“离线自足”冲突，本模块只做构造/解析；套接字级收发见 net_client/server（另批）。

use std::net::Ipv4Addr;

use etherparse::{
    LinkSlice, NetSlice, PacketBuilder, SlicedPacket, TransportSlice,
};
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Serialize)]
pub struct NetLayer {
    pub name: String,
    pub fields: Vec<(String, String)>,
}

#[derive(Serialize)]
pub struct NetParseResult {
    pub layers: Vec<NetLayer>,
    pub payload_hex: String,
    pub total_len: usize,
}

fn decode_input(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return Err("输入为空".to_string());
    }
    // 优先按 hex（含可选 0x 前缀 / 冒号分隔）
    let hexish: String = compact.trim_start_matches("0x").replace([':', '-'], "");
    if hexish.len() % 2 == 0 && hexish.chars().all(|c| c.is_ascii_hexdigit()) {
        return hex::decode(&hexish).map_err(|e| format!("hex 解码失败: {e}"));
    }
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(&compact)
        .map_err(|_| "无法识别输入：需要 hex 或 base64".to_string())
}

/// 解析报文为分层字段树。has_ethernet=true 时从以太网帧起，否则从 IP 层起。
#[command]
pub fn net_parse_packet(input: String, has_ethernet: bool) -> Result<NetParseResult, String> {
    let bytes = decode_input(&input)?;
    let total_len = bytes.len();
    let sliced = if has_ethernet {
        SlicedPacket::from_ethernet(&bytes).map_err(|e| format!("解析失败: {e}"))?
    } else {
        SlicedPacket::from_ip(&bytes).map_err(|e| format!("解析失败: {e}"))?
    };

    let mut layers: Vec<NetLayer> = Vec::new();

    if let Some(LinkSlice::Ethernet2(eth)) = &sliced.link {
        layers.push(NetLayer {
            name: "以太网 II".into(),
            fields: vec![
                ("源 MAC".into(), mac(&eth.source())),
                ("目的 MAC".into(), mac(&eth.destination())),
                ("类型".into(), format!("0x{:04x}", u16::from(eth.ether_type()))),
            ],
        });
    }

    match &sliced.net {
        Some(NetSlice::Ipv4(ip)) => {
            let h = ip.header();
            layers.push(NetLayer {
                name: "IPv4".into(),
                fields: vec![
                    ("版本".into(), "4".into()),
                    ("源地址".into(), h.source_addr().to_string()),
                    ("目的地址".into(), h.destination_addr().to_string()),
                    ("TTL".into(), h.ttl().to_string()),
                    ("协议".into(), format!("{:?}", h.protocol())),
                    ("首部校验和".into(), format!("0x{:04x}", h.header_checksum())),
                ],
            });
        }
        Some(NetSlice::Ipv6(ip)) => {
            let h = ip.header();
            layers.push(NetLayer {
                name: "IPv6".into(),
                fields: vec![
                    ("源地址".into(), std::net::Ipv6Addr::from(h.source()).to_string()),
                    ("目的地址".into(), std::net::Ipv6Addr::from(h.destination()).to_string()),
                    ("跳数限制".into(), h.hop_limit().to_string()),
                ],
            });
        }
        None => {}
    }

    let mut payload: &[u8] = &[];
    match &sliced.transport {
        Some(TransportSlice::Tcp(t)) => {
            let mut flags = String::new();
            if t.syn() { flags.push_str("SYN "); }
            if t.ack() { flags.push_str("ACK "); }
            if t.fin() { flags.push_str("FIN "); }
            if t.rst() { flags.push_str("RST "); }
            if t.psh() { flags.push_str("PSH "); }
            if t.urg() { flags.push_str("URG "); }
            layers.push(NetLayer {
                name: "TCP".into(),
                fields: vec![
                    ("源端口".into(), t.source_port().to_string()),
                    ("目的端口".into(), t.destination_port().to_string()),
                    ("序列号".into(), t.sequence_number().to_string()),
                    ("确认号".into(), t.acknowledgment_number().to_string()),
                    ("窗口".into(), t.window_size().to_string()),
                    ("标志".into(), flags.trim().to_string()),
                    ("校验和".into(), format!("0x{:04x}", t.checksum())),
                ],
            });
            payload = t.payload();
        }
        Some(TransportSlice::Udp(u)) => {
            layers.push(NetLayer {
                name: "UDP".into(),
                fields: vec![
                    ("源端口".into(), u.source_port().to_string()),
                    ("目的端口".into(), u.destination_port().to_string()),
                    ("长度".into(), u.length().to_string()),
                    ("校验和".into(), format!("0x{:04x}", u.checksum())),
                ],
            });
            payload = u.payload();
        }
        Some(TransportSlice::Icmpv4(i)) => {
            layers.push(NetLayer {
                name: "ICMPv4".into(),
                fields: vec![("类型".into(), format!("{:?}", i.icmp_type()))],
            });
            payload = i.payload();
        }
        Some(TransportSlice::Icmpv6(i)) => {
            layers.push(NetLayer {
                name: "ICMPv6".into(),
                fields: vec![("类型".into(), format!("{:?}", i.icmp_type()))],
            });
            payload = i.payload();
        }
        None => {}
    }

    Ok(NetParseResult {
        layers,
        payload_hex: hex::encode(payload),
        total_len,
    })
}

fn mac(b: &[u8; 6]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(":")
}

fn parse_ipv4(s: &str) -> Result<[u8; 4], String> {
    s.trim().parse::<Ipv4Addr>().map(|a| a.octets()).map_err(|_| format!("无效 IPv4: {s}"))
}

#[derive(Deserialize)]
pub struct BuildSpec {
    pub ip_src: String,
    pub ip_dst: String,
    #[serde(default = "default_ttl")]
    pub ttl: u8,
    /// "tcp" | "udp" | "icmp-echo"
    pub transport: String,
    #[serde(default)]
    pub src_port: u16,
    #[serde(default)]
    pub dst_port: u16,
    #[serde(default)]
    pub seq: u32,
    #[serde(default = "default_window")]
    pub window: u16,
    #[serde(default)]
    pub icmp_id: u16,
    #[serde(default)]
    pub icmp_seq: u16,
    /// 载荷：hex 或纯文本（由 payload_is_hex 决定）
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub payload_is_hex: bool,
}

fn default_ttl() -> u8 { 64 }
fn default_window() -> u16 { 65535 }

#[derive(Serialize)]
pub struct BuildResult {
    pub hex: String,
    pub len: usize,
}

/// 构造报文，etherparse 自动填 IP/TCP/UDP/ICMP 校验码。
#[command]
pub fn net_build_packet(spec: BuildSpec) -> Result<BuildResult, String> {
    let src = parse_ipv4(&spec.ip_src)?;
    let dst = parse_ipv4(&spec.ip_dst)?;
    let payload: Vec<u8> = if spec.payload_is_hex {
        let compact: String = spec.payload.chars().filter(|c| !c.is_whitespace()).collect();
        hex::decode(&compact).map_err(|e| format!("载荷 hex 解码失败: {e}"))?
    } else {
        spec.payload.into_bytes()
    };

    let mut out: Vec<u8> = Vec::new();
    match spec.transport.as_str() {
        "tcp" => {
            let b = PacketBuilder::ipv4(src, dst, spec.ttl)
                .tcp(spec.src_port, spec.dst_port, spec.seq, spec.window);
            out.reserve(b.size(payload.len()));
            b.write(&mut out, &payload).map_err(|e| format!("写 TCP 失败: {e}"))?;
        }
        "udp" => {
            let b = PacketBuilder::ipv4(src, dst, spec.ttl).udp(spec.src_port, spec.dst_port);
            out.reserve(b.size(payload.len()));
            b.write(&mut out, &payload).map_err(|e| format!("写 UDP 失败: {e}"))?;
        }
        "icmp-echo" => {
            let b = PacketBuilder::ipv4(src, dst, spec.ttl)
                .icmpv4_echo_request(spec.icmp_id, spec.icmp_seq);
            out.reserve(b.size(payload.len()));
            b.write(&mut out, &payload).map_err(|e| format!("写 ICMP 失败: {e}"))?;
        }
        other => return Err(format!("不支持的传输层: {other}")),
    }
    Ok(BuildResult { len: out.len(), hex: hex::encode(&out) })
}
