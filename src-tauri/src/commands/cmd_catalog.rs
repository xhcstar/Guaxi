//! 命令目录与场景模板：给「命令下发」提供可检索的命令说明、参数提示，以及一键生成的整套任务。
//!
//! 内容取自产品 CLI 手册（`infosec/source/cli_cn`）：Chapter13/14 = SSL 加速/侦听，
//! Chapter8 = SLB（后台服务 / 服务组 / 虚拟服务 / 策略）。语法行、参数取值都按手册抄录，
//! 供 UI 做"命令浏览器 + 参数表单 + 插入任务"。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::command;

use super::cmd_dispatch::{DispatchConfig, GlobalCfg, Targets, TaskAnswer, TaskTemplate};

#[derive(Debug, Clone, Serialize)]
pub struct CatalogParam {
    pub name: &'static str,
    pub required: bool,
    pub doc: &'static str,
    pub example: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogCommand {
    /// 稳定标识（场景模板与 UI 引用）。
    pub id: &'static str,
    /// 分组：SSL 主机 / SSL 证书 / SM2 / SSL 激活 / SLB。
    pub group: &'static str,
    pub name: &'static str,
    /// 手册里的语法行。
    pub syntax: &'static str,
    pub summary: &'static str,
    /// 可直接放进任务表 `cmd` 字段的模板（含 `{hostname}` `{domain}` `{index}` 占位符）。
    pub cmd_template: &'static str,
    /// 该命令是否会进入 PEM 粘贴流程（需要任务里配 `pem_file`）。
    pub pem: bool,
    pub params: Vec<CatalogParam>,
}

fn p(name: &'static str, required: bool, doc: &'static str, example: &'static str) -> CatalogParam {
    CatalogParam { name, required, doc, example }
}

/// 内置命令目录。
#[command]
pub fn cmd_catalog_list() -> Vec<CatalogCommand> {
    use CatalogCommand as C;
    vec![
        // ---------------- SSL 主机 ----------------
        C {
            id: "ssl_host_virtual", group: "SSL 主机", name: "ssl host virtual",
            syntax: "ssl host {virtual|real} <host_name> [slb_service]",
            summary: "创建 SSL 主机。virtual = SSL 虚拟主机（充当 SSL 服务端，关联 SLB 虚拟服务）；real = SSL 后台主机（充当 SSL 客户端，关联 SLB 后台服务）。关联前需先建好对应 SLB 服务；不写 slb_service 就只建主机。",
            cmd_template: "ssl host virtual {hostname}", pem: false,
            params: vec![
                p("host_name", true, "SSL 主机名，≤240 字节；不能以特殊字符开头，禁止 & * ! / \\ ` ( ) ; ' \" | < > $ 和空格", "vhost"),
                p("slb_service", false, "要绑定的 SLB 服务名。虚拟主机只能绑 HTTPS/TCPS/FTPS 虚拟服务；后台主机只能绑 HTTPS/TCPS 后台服务", "vs_https"),
            ],
        },
        C {
            id: "ssl_host_real", group: "SSL 主机", name: "ssl host real",
            syntax: "ssl host real <host_name> [slb_service]",
            summary: "创建 SSL 后台主机（作为 SSL 客户端，与 SLB 后台服务关联）。",
            cmd_template: "ssl host real {hostname}", pem: false,
            params: vec![p("host_name", true, "SSL 后台主机名", "rhost")],
        },
        C {
            id: "ssl_sni", group: "SSL 主机", name: "ssl sni",
            syntax: "ssl sni <host_name> <domain_name>",
            summary: "把域名关联到 SSL 虚拟主机。一个虚拟主机最多 64 个域名；支持通配 *.domain.com（只允许这种通配形式）。主机名 + 域名总长 ≤ 220 字符。",
            cmd_template: "ssl sni {hostname} {domain}", pem: false,
            params: vec![
                p("host_name", true, "SSL 主机名", "vhost"),
                p("domain_name", true, "域名；纯数字需加双引号；支持 *.domain.com", "www.example.com"),
            ],
        },
        C {
            id: "ssl_clear_host", group: "SSL 主机", name: "clear ssl host",
            syntax: "clear ssl host <host_name>",
            summary: "清除指定 SSL 主机及其全部配置（含私钥与证书）。重新配置需重新导入密钥/证书。目标本来就没配置时视为幂等成功。",
            cmd_template: "clear ssl host {hostname}", pem: false,
            params: vec![p("host_name", true, "要清除的 SSL 主机名", "vhost")],
        },
        C {
            id: "ssl_show_host", group: "SSL 主机", name: "show ssl host",
            syntax: "show ssl host",
            summary: "显示当前配置的 SSL 主机及其关联的 SLB 服务。",
            cmd_template: "show ssl host", pem: false, params: vec![],
        },
        C {
            id: "ssl_clear_sni", group: "SSL 主机", name: "clear ssl sni",
            syntax: "clear ssl sni <host_name> <domain_name>",
            summary: "删除域名与 SSL 主机的关联。注意：会一并删除该域名下的 SSL 配置（含密钥）。",
            cmd_template: "clear ssl sni {hostname} {domain}", pem: false,
            params: vec![p("domain_name", true, "要解绑的域名", "www.example.com")],
        },

        // ---------------- SSL 证书 ----------------
        C {
            id: "ssl_import_rootca", group: "SSL 证书", name: "ssl import rootca",
            syntax: "ssl import rootca [host_name] [domain_name] [tftp_ip|url] [file_name]",
            summary: "导入根 CA 证书（走 PEM 粘贴：设备提示后粘贴证书内容，以单独一行 ... 结束）。",
            cmd_template: "ssl import rootca {hostname}", pem: true,
            params: vec![p("host_name", false, "SSL 主机名；带域名的 SNI 流程把域名一起写上", "vhost")],
        },
        C {
            id: "ssl_import_interca", group: "SSL 证书", name: "ssl import interca",
            syntax: "ssl import interca <virtual_host_name> [domain_name] [tftp_ip|url] [file_name]",
            summary: "导入中间 CA 证书（需先导入对应的根 CA）。",
            cmd_template: "ssl import interca {hostname}", pem: true,
            params: vec![p("host_name", true, "SSL 主机名", "vhost")],
        },
        C {
            id: "ssl_import_crlca", group: "SSL 证书", name: "ssl import crlca",
            syntax: "ssl import crlca [host_name] [domain_name] [tftp_ip|url] [file_name]",
            summary: "导入 CRL 签发者 CA 证书（用于吊销列表校验链）。",
            cmd_template: "ssl import crlca {hostname}", pem: true,
            params: vec![p("host_name", false, "SSL 主机名", "vhost")],
        },
        C {
            id: "ssl_import_key", group: "SSL 证书", name: "ssl import key",
            syntax: "ssl import key <host_name> [key_index] [domain_name] [tftp_ip|url] [file_name|password]",
            summary: "导入私钥并与私钥索引关联（每主机最多 3 个）。PEM 粘贴时须以 -----BEGIN RSA PRIVATE KEY----- 开头、-----END RSA PRIVATE KEY----- 结尾。",
            cmd_template: "ssl import key {hostname} {index}", pem: true,
            params: vec![
                p("key_index", false, "私钥索引 1/2/3，默认 1", "1"),
                p("domain_name", false, "SNI 场景填域名；不关联域名则不写", "www.example.com"),
            ],
        },
        C {
            id: "ssl_import_certificate", group: "SSL 证书", name: "ssl import certificate",
            syntax: "ssl import certificate <host_name> [certificate_index] [domain_name] [tftp_ip|url] [filename|password]",
            summary: "导入证书并与证书索引关联（每主机最多 3 个）。需先导入与之匹配的私钥。",
            cmd_template: "ssl import certificate {hostname} {index}", pem: true,
            params: vec![p("certificate_index", false, "证书索引 1/2/3，默认 1", "1")],
        },
        C {
            id: "ssl_activate_certificate", group: "SSL 激活", name: "ssl activate certificate",
            syntax: "ssl activate certificate <host_name> [certificate_index] [domain_name] [certificate_type]",
            summary: "激活指定主机/域名下的证书。类型可取 rsa / ecc / sm2 / all（默认 all）。不关联域名时每主机可激活 RSA、ECC 各一及一对 SM2。建议在任务里配「校验命令」（如 show ssl host）和期望关键字，确认激活真的生效。",
            cmd_template: "ssl activate certificate {hostname} {index} \"\" all", pem: false,
            params: vec![
                p("certificate_index", false, "证书索引 1/2/3，默认 1", "1"),
                p("certificate_type", false, "rsa | ecc | sm2 | all，默认 all", "all"),
            ],
        },
        C {
            id: "ssl_backup_certificate", group: "SSL 证书", name: "ssl backup certificate",
            syntax: "ssl backup certificate <host_name|all> \"<name>.tgz\" \"<password>\"",
            summary: "把主机（或全部）的证书与私钥备份成 .tgz。实测：文件名只允许 0-9/a-z/A-Z/_，必须 .tgz；如只给主机名会再问文件名与密码。",
            cmd_template: "ssl backup certificate {hostname} \"guaxi_backup.tgz\" \"passw0rd\"", pem: false,
            params: vec![
                p("file_name", true, "备份文件名，必须以 .tgz 结尾，只允许字母数字下划线", "guaxi_backup.tgz"),
                p("password", true, "备份文件的保护密码", "passw0rd"),
            ],
        },
        C {
            id: "ssl_restore_certificate", group: "SSL 证书", name: "ssl restore certificate",
            syntax: "ssl restore certificate <host_name|all> \"<name>.tgz\" \"<password>\"",
            summary: "从 ssl backup certificate 生成的 .tgz 恢复。实测会先问 Type YES to proceed，再校验备份内容。",
            cmd_template: "ssl restore certificate {hostname} \"guaxi_backup.tgz\" \"passw0rd\"", pem: false,
            params: vec![
                p("file_name", true, "备份文件名（.tgz）", "guaxi_backup.tgz"),
                p("password", true, "备份文件的保护密码", "passw0rd"),
            ],
        },
        C {
            id: "ssl_csr", group: "SSL 证书", name: "ssl csr",
            syntax: "ssl csr <host_name> [key_length] [certificate_index] [signature_algorithm_index] [domain_name] [csr_type_index]",
            summary: "为指定主机生成 CSR 与密钥对（交互式，会依次询问国家/省份/城市/组织/部门）。同时生成一张仅供测试的证书。key_length 可取 1024/2048/4096。",
            cmd_template: "ssl csr {hostname} 2048 1 1", pem: false,
            params: vec![p("key_length", false, "密钥长度 1024 | 2048 | 4096", "2048")],
        },
        C {
            id: "ssl_deactivate_certificate", group: "SSL 激活", name: "ssl deactivate certificate",
            syntax: "ssl deactivate certificate <host_name>",
            summary: "停用指定主机的证书（实测会再问 Server Name Indication，输入 \"\" 表示跳过域名）。",
            cmd_template: "ssl deactivate certificate {hostname}", pem: false,
            params: vec![],
        },
        C {
            id: "ssl_start", group: "SSL 激活", name: "ssl start",
            syntax: "ssl start <host_name>",
            summary: "启动指定 SSL 主机。",
            cmd_template: "ssl start {hostname}", pem: false, params: vec![],
        },
        C {
            id: "ssl_stop", group: "SSL 激活", name: "ssl stop",
            syntax: "ssl stop <host_name>",
            summary: "停止指定 SSL 主机。",
            cmd_template: "ssl stop {hostname}", pem: false, params: vec![],
        },
        C {
            id: "ssl_settings_clientauth", group: "SSL 设置", name: "ssl settings clientauth（双向认证）",
            syntax: "ssl settings clientauth <host_name>",
            summary: "为 SSL 主机开启客户端证书认证（mTLS）。实测该子命令只接主机名，随后进入开关选择。",
            cmd_template: "ssl settings clientauth {hostname}", pem: false,
            params: vec![],
        },
        C {
            id: "ssl_settings_clientcert", group: "SSL 设置", name: "ssl settings clientcert",
            syntax: "ssl settings clientcert minkey <host_name> <bits>",
            summary: "限制客户端证书的最小密钥长度（minkey 子命令）。",
            cmd_template: "ssl settings clientcert minkey {hostname} 2048", pem: false,
            params: vec![p("bits", true, "最小密钥长度", "2048")],
        },
        C {
            id: "ssl_settings_protocol", group: "SSL 设置", name: "ssl settings protocol",
            syntax: "ssl settings protocol <host_name> \"<versions>\"",
            summary: "设置主机支持的 SSL/TLS 版本。实测格式：冒号连接，如 TLSv1:TLSv11:TLSv12 或 ALL。不含 SM2v11 时设备会提示移除国密套件。",
            cmd_template: "ssl settings protocol {hostname} \"TLSv1:TLSv11:TLSv12:TLSv13\"", pem: false,
            params: vec![p("versions", true, "版本串，冒号分隔；ALL 表示全部", "TLSv1:TLSv11:TLSv12:TLSv13")],
        },
        C {
            id: "ssl_settings_ciphersuite", group: "SSL 设置", name: "ssl settings ciphersuite",
            syntax: "ssl settings ciphersuite <host_name> \"<suites>\"",
            summary: "设置主机套件。实测需写成与当前 protocol 兼容的冒号串；不兼容时设备会给出该 protocol 下可用套件清单。",
            cmd_template: "ssl settings ciphersuite {hostname} \"AES256-GCM-SHA384:AES128-GCM-SHA256\"", pem: false,
            params: vec![p("suites", true, "套件串，冒号分隔", "AES256-GCM-SHA384:AES128-GCM-SHA256")],
        },
        C {
            id: "ssl_settings_curves", group: "SSL 设置", name: "ssl settings curves",
            syntax: "ssl settings curves <host_name> \"<curves>\"",
            summary: "设置主机支持的椭圆曲线（含 x25519_mlkem768 等混合组）。",
            cmd_template: "ssl settings curves {hostname} \"secp256r1:secp384r1:x25519_mlkem768\"", pem: false,
            params: vec![p("curves", true, "曲线串，冒号分隔", "secp256r1:secp384r1:x25519_mlkem768")],
        },
        C {
            id: "ssl_globals_protocol", group: "SSL 设置", name: "ssl globals protocol",
            syntax: "ssl globals protocol {virtual|real} \"<versions>\"",
            summary: "设置全局 SSL 版本（虚拟主机或后台主机分别设置）。",
            cmd_template: "ssl globals protocol virtual \"TLSv1:TLSv11:TLSv12:TLSv13\"", pem: false,
            params: vec![p("versions", true, "版本串", "TLSv1:TLSv11:TLSv12:TLSv13")],
        },

        // ---------------- SM2 ----------------
        C {
            id: "sm2_import_signkey", group: "SM2", name: "ssl sm2 import signkey",
            syntax: "ssl sm2 import signkey <host_name> [key_index] [domain_name] [tftp_ip|url] [file_name]",
            summary: "导入 SM2 签名私钥。",
            cmd_template: "ssl sm2 import signkey {hostname} {index}", pem: true,
            params: vec![p("key_index", false, "私钥索引，默认 1", "1")],
        },
        C {
            id: "sm2_import_signcertificate", group: "SM2", name: "ssl sm2 import signcertificate",
            syntax: "ssl sm2 import signcertificate <host_name> [certificate_index] [tftp_ip|url] [file_name]",
            summary: "导入 SM2 签名证书（需先导入对应签名私钥）。",
            cmd_template: "ssl sm2 import signcertificate {hostname} {index}", pem: true,
            params: vec![p("certificate_index", false, "证书索引，默认 1", "1")],
        },
        C {
            id: "sm2_import_enckey", group: "SM2", name: "ssl sm2 import enckey",
            syntax: "ssl sm2 import enckey <host_name> [key_index] [tftp_ip|url] [file_name]",
            summary: "导入 SM2 加密私钥（国密双证书里的加密密钥对）。",
            cmd_template: "ssl sm2 import enckey {hostname} {index}", pem: true,
            params: vec![p("key_index", false, "私钥索引，默认 1", "1")],
        },
        C {
            id: "sm2_import_enccertificate", group: "SM2", name: "ssl sm2 import enccertificate",
            syntax: "ssl sm2 import enccertificate <host_name> [certificate_index] [tftp_ip|url] [file_name]",
            summary: "导入 SM2 加密证书。注意：加密证书与签名证书的 Subject 必须完全一致。",
            cmd_template: "ssl sm2 import enccertificate {hostname} {index}", pem: true,
            params: vec![p("certificate_index", false, "证书索引，默认 1", "1")],
        },
        C {
            id: "sm2_sni_import_signkey", group: "SM2", name: "ssl sm2 sni import signkey",
            syntax: "ssl sm2 sni import signkey <host_name> [domain_name] [key_index] [tftp_ip|url] [file_name]",
            summary: "SNI（按域名）导入 SM2 签名私钥。",
            cmd_template: "ssl sm2 sni import signkey {hostname} {domain} {index}", pem: true,
            params: vec![p("domain_name", true, "域名", "www.example.com")],
        },
        C {
            id: "sm2_sni_import_signcertificate", group: "SM2", name: "ssl sm2 sni import signcertificate",
            syntax: "ssl sm2 sni import signcertificate <host_name> [domain_name] [certificate_index] [tftp_ip|url] [file_name]",
            summary: "SNI（按域名）导入 SM2 签名证书。",
            cmd_template: "ssl sm2 sni import signcertificate {hostname} {domain} {index}", pem: true,
            params: vec![p("domain_name", true, "域名", "www.example.com")],
        },
        C {
            id: "sm2_sni_import_enckey", group: "SM2", name: "ssl sm2 sni import enckey",
            syntax: "ssl sm2 sni import enckey <host_name> [domain_name] [key_index] [tftp_ip|url] [file_name]",
            summary: "SNI（按域名）导入 SM2 加密私钥。",
            cmd_template: "ssl sm2 sni import enckey {hostname} {domain} {index}", pem: true,
            params: vec![p("domain_name", true, "域名", "www.example.com")],
        },
        C {
            id: "sm2_sni_import_enccertificate", group: "SM2", name: "ssl sm2 sni import enccertificate",
            syntax: "ssl sm2 sni import enccertificate <host_name> [domain_name] [certificate_index] [tftp_ip|url] [file_name]",
            summary: "SNI（按域名）导入 SM2 加密证书。",
            cmd_template: "ssl sm2 sni import enccertificate {hostname} {domain} {index}", pem: true,
            params: vec![p("domain_name", true, "域名", "www.example.com")],
        },

        // ---------------- SSL 全局 / 设置 ----------------
        // 实测设备（ArrayOS APV 10.0.0.805）只接受配置级的 `ssl settings` / `ssl globals`，
        // 不存在 `show ssl settings` / `show ssl globals`。
        C {
            id: "ssl_settings", group: "SSL 全局", name: "ssl settings",
            syntax: "ssl settings [acceptchain|alpn|certfilter|ciphersuite|clientauth|clientcert|crl|curves|description|monitor|ocsp|protocol|reneg|reuse|servername|signalgo|sm2pqcgroup|tlsv13|verifycert ...]",
            summary: "调整 SSL 主机级参数（需在 config 模式）。不带参数时列出可用子命令；具体子命令参数可在设备上继续用 `?` 查看。",
            cmd_template: "ssl settings", pem: false, params: vec![],
        },
        C {
            id: "ssl_globals", group: "SSL 全局", name: "ssl globals",
            syntax: "ssl globals [certchkinterval|certexpdays|crl|ems|encryptkey|fastcrl|protocol|renegotiation|sessiontimeout|statistics|ticketlimit|verifycert ...]",
            summary: "调整 SSL 全局参数（需在 config 模式）。不带参数时列出可用子命令。",
            cmd_template: "ssl globals", pem: false, params: vec![],
        },
        C {
            id: "show_ssl_settings", group: "SSL 全局", name: "show ssl settings",
            syntax: "show ssl settings <host_name>",
            summary: "查看指定 SSL 主机的生效配置（实测：必须带主机名；不带会提示 Host name）。",
            cmd_template: "show ssl settings {hostname}", pem: false,
            params: vec![p("host_name", true, "SSL 主机名", "vhost")],
        },
        C {
            id: "show_ssl_globals", group: "SSL 全局", name: "show ssl globals",
            syntax: "show ssl globals",
            summary: "查看 SSL 全局配置（实测有效，无参数）。",
            cmd_template: "show ssl globals", pem: false, params: vec![],
        },

        // ---------------- SLB ----------------
        C {
            id: "slb_real", group: "SLB", name: "slb real <type>",
            syntax: "slb real {http|https|tcp|tcps|udp|ftp|dns|rdp|rtsp|sip...} <real_service> <ip|domain> [port]",
            summary: "定义后台服务（真正处理请求的服务器）。类型要与协议匹配：HTTPS/TCP 可被 SSL 后台主机关联。",
            cmd_template: "slb real https {real} {real_ip} {real_port}", pem: false,
            params: vec![
                p("real_service", true, "后台服务名（≤128 字节；含特殊字符时加双引号）", "rs_web"),
                p("ip|domain", true, "后台服务器 IP 或域名", "10.0.0.11"),
                p("port", false, "端口", "8443"),
            ],
        },
        C {
            id: "slb_group_method", group: "SLB", name: "slb group method",
            syntax: "slb group method <group_name> [method]",
            summary: "创建后台服务组并设置负载均衡算法。method 取值：rr 轮询 / grr 全局轮询 / lc 最少连接 / lb 最小带宽 / hi Hash(IP) / hip Hash(IP+Port)。不写 method 使用默认算法。",
            cmd_template: "slb group method {group} rr", pem: false,
            params: vec![
                p("group_name", true, "服务组名", "grp_web"),
                p("method", false, "rr | grr | lc | lb | hi | hip", "rr"),
            ],
        },
        C {
            id: "slb_group_member", group: "SLB", name: "slb group member",
            syntax: "slb group member <group_name> <real_service> [weight|cookie_value|url_tag_value] [priority]",
            summary: "把后台服务加入服务组。",
            cmd_template: "slb group member {group} {real}", pem: false,
            params: vec![
                p("group_name", true, "已存在的服务组名", "grp_web"),
                p("real_service", true, "已存在的后台服务名", "rs_web"),
            ],
        },
        C {
            id: "slb_virtual_https", group: "SLB", name: "slb virtual https",
            syntax: "slb virtual https <virtual_service> <vip> [vport] [arp_support] [max_connection]",
            summary: "定义 HTTPS 虚拟服务（对外统一接入点）。SSL 虚拟主机就是与这种虚拟服务关联的。",
            cmd_template: "slb virtual https {vs} {vip} {vport}", pem: false,
            params: vec![
                p("virtual_service", true, "虚拟服务名", "vs_https"),
                p("vip", true, "虚拟服务 IP", "192.0.2.10"),
                p("vport", false, "虚拟服务端口，默认 443", "443"),
            ],
        },
        C {
            id: "slb_virtual_http", group: "SLB", name: "slb virtual http",
            syntax: "slb virtual http <virtual_service> <vip> [vport] [arp_support] [max_connection]",
            summary: "定义 HTTP 虚拟服务。",
            cmd_template: "slb virtual http {vs} {vip} {vport}", pem: false,
            params: vec![p("virtual_service", true, "虚拟服务名", "vs_http")],
        },
        C {
            id: "slb_policy_static", group: "SLB", name: "slb policy static",
            syntax: "slb policy static <virtual_service> <real_service>",
            summary: "为虚拟服务配置静态策略，把访问该虚拟服务的请求转发到指定后台服务（每个虚拟服务只允许一条静态策略）。",
            cmd_template: "slb policy static {vs} {real}", pem: false,
            params: vec![
                p("virtual_service", true, "已存在的虚拟服务名", "vs_https"),
                p("real_service", true, "已存在的后台服务名", "rs_web"),
            ],
        },
        C {
            id: "slb_policy_hashurl", group: "SLB", name: "slb policy hashurl",
            syntax: "slb policy hashurl <policy_name> <virtual_service> <group_name>",
            summary: "按 URL 哈希把虚拟服务的流量分配到后台服务组。",
            cmd_template: "slb policy hashurl {policy} {vs} {group}", pem: false,
            params: vec![p("policy_name", true, "策略名", "pol_hashurl")],
        },
        C {
            id: "slb_show_real", group: "SLB", name: "show slb real",
            syntax: "show slb real [real_service]",
            summary: "查看后台服务配置，便于下发前确认现状。",
            cmd_template: "show slb real", pem: false, params: vec![],
        },
        C {
            id: "slb_show_virtual", group: "SLB", name: "show slb virtual",
            syntax: "show slb virtual [virtual_service]",
            summary: "查看虚拟服务配置。",
            cmd_template: "show slb virtual", pem: false, params: vec![],
        },
    ]
}

// ============================ 场景模板 ============================

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    /// 生成该场景需要的参数（UI 据此渲染表单）。
    pub params: Vec<CatalogParam>,
}

#[command]
pub fn cmd_scenario_list() -> Vec<ScenarioInfo> {
    vec![
        ScenarioInfo {
            id: "ssl_chain",
            name: "SSL 虚拟主机 + 证书链 + 激活",
            summary: "clear → 建虚拟主机 →（SNI 域名）→ 导入 rootca/interca/crlca → 导入服务端私钥与证书 → 激活。等价于参考工具 config.yaml 的产物。",
            params: vec![
                p("hostname", true, "SSL 虚拟主机名", "vhost"),
                p("domains", false, "域名，逗号分隔；留空表示只做 host 级（非 SNI）", "www.example.com,api.example.com"),
                p("algo", false, "证书算法：rsa | ecc | sm2", "rsa"),
                p("pem_dir", false, "证书文件所在目录（相对/绝对均可）", "./cfgfiles/RSA"),
                p("index", false, "证书/私钥索引 1/2/3", "1"),
                p("do_clear", false, "是否先 clear ssl host（yes/no）", "yes"),
                p("activate_type", false, "激活类型 rsa | ecc | sm2 | all", "all"),
            ],
        },
        ScenarioInfo {
            id: "slb_https_ssl",
            name: "SLB 虚拟服务 + 后台服务 + SSL 证书链 + 激活",
            summary: "定义后台服务 → 建服务组并加成员 → 定义 HTTPS 虚拟服务 → 静态策略关联虚拟服务与后台服务 → 建 SSL 虚拟主机并绑定该虚拟服务 → SNI → 导入证书链与私钥 → 激活。",
            params: vec![
                p("real", true, "后台服务名", "rs_web"),
                p("real_ip", true, "后台服务器 IP", "10.0.0.11"),
                p("real_port", false, "后台服务端口", "8443"),
                p("group", true, "后台服务组名", "grp_web"),
                p("method", false, "服务组算法 rr|grr|lc|lb|hi|hip", "rr"),
                p("vs", true, "虚拟服务名", "vs_https"),
                p("vip", true, "虚拟服务 VIP", "192.0.2.10"),
                p("vport", false, "虚拟服务端口", "443"),
                p("hostname", true, "SSL 虚拟主机名", "vhost"),
                p("domains", false, "域名，逗号分隔", "www.example.com"),
                p("algo", false, "证书算法 rsa | ecc | sm2", "rsa"),
                p("pem_dir", false, "证书文件所在目录", "./cfgfiles/RSA"),
                p("index", false, "证书/私钥索引", "1"),
            ],
        },
    ]
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioOptions {
    #[serde(default)]
    pub ip: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub enable_password: Option<String>,
    /// 场景参数（未给的用默认值）。
    #[serde(default)]
    pub params: HashMap<String, String>,
}

fn get<'a>(p: &'a HashMap<String, String>, k: &str, d: &'a str) -> String {
    p.get(k)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(d)
        .to_string()
}

fn tpl(id: &str, cmd: &str, deps: &[&str]) -> TaskTemplate {
    TaskTemplate {
        id: id.to_string(),
        cmd: cmd.to_string(),
        pem_file: None,
        index: None,
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        only_if_domain: false,
        only_if_no_domain: false,
        description: None,
        verify_cmd: None,
        expect: vec![],
        answers: vec![],
    }
}

/// 需要 YES 确认的命令（激活、恢复等）。
fn yes_answer(when: &str) -> TaskAnswer {
    TaskAnswer { when: when.into(), reply: "YES".into(), repeat: false }
}

fn with_pem(mut t: TaskTemplate, path: String) -> TaskTemplate {
    t.pem_file = Some(path);
    t
}
fn with_index(mut t: TaskTemplate, idx: String) -> TaskTemplate {
    t.index = Some(idx);
    t
}
fn only_domain(mut t: TaskTemplate) -> TaskTemplate {
    t.only_if_domain = true;
    t
}
fn only_host(mut t: TaskTemplate) -> TaskTemplate {
    t.only_if_no_domain = true;
    t
}
fn desc(mut t: TaskTemplate, d: &str) -> TaskTemplate {
    t.description = Some(d.to_string());
    t
}

/// 按场景 id 渲染一份完整的 config.yaml（结构由 serde_yaml 生成，保证合法）。
#[command]
pub fn cmd_scenario_render(id: String, opts: ScenarioOptions) -> Result<String, String> {
    let cfg = match id.as_str() {
        "ssl_chain" => build_ssl_chain(&opts),
        "slb_https_ssl" => build_slb_https_ssl(&opts),
        other => return Err(format!("未知场景: {other}")),
    };
    serde_yaml::to_string(&cfg).map_err(|e| format!("生成 YAML 失败: {e}"))
}

fn global_of(opts: &ScenarioOptions) -> GlobalCfg {
    GlobalCfg {
        ip: opts.ip.clone(),
        port: opts.port.or(Some(22)),
        username: opts.username.clone(),
        password: opts.password.clone(),
        enable_password: opts.enable_password.clone(),
    }
}

fn split_domains(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    // 总带一个 host 级（空域名）目标，保证非 SNI 的任务也能展开。
    v.insert(0, String::new());
    v
}

/// 生成 SSL 证书链那一组任务（host 级 + SNI 级），供两个场景共用。
/// `slb_service` 非空时，`create` 会绑定到该 SLB 虚拟服务。
fn ssl_chain_tasks(p: &HashMap<String, String>, slb_service: Option<&str>) -> Vec<TaskTemplate> {
    let algo = get(p, "algo", "rsa").to_lowercase();
    let dir = get(p, "pem_dir", "./cfgfiles/RSA");
    let index = get(p, "index", "1");
    let do_clear = !get(p, "do_clear", "yes").eq_ignore_ascii_case("no");
    let act_type = get(p, "activate_type", "all");
    let mut t: Vec<TaskTemplate> = Vec::new();

    if do_clear {
        t.push(only_host(desc(
            tpl("clear", "clear ssl host {hostname}", &[]),
            "先清掉同名主机的旧配置（本来没配置也算成功）",
        )));
    }
    let create_cmd = match slb_service {
        Some(s) if !s.trim().is_empty() => format!("ssl host virtual {{hostname}} {}", s.trim()),
        _ => "ssl host virtual {hostname}".to_string(),
    };
    let create_deps: &[&str] = if do_clear { &["clear"] } else { &[] };
    t.push(desc(tpl("create", &create_cmd, create_deps), "创建 SSL 虚拟主机"));
    t.push(only_domain(desc(
        tpl("sni", "ssl sni {hostname} {domain}", &["create"]),
        "把域名关联到该虚拟主机（SNI）",
    )));

    // 证书链：rootca → interca → crlca → key → certificate；host 级与 SNI 级各一份。
    for suffix in ["host", "sni"] {
        let sni = suffix == "sni";
        let mk: fn(TaskTemplate) -> TaskTemplate = if sni { only_domain } else { only_host };
        let tag = |id: &str| format!("{id}_{suffix}");
        let dom = if sni { " {domain}" } else { "" };
        let dep = if sni { "sni" } else { "create" };

        t.push(mk(with_pem(
            tpl(&tag("rootca"), &format!("ssl import rootca {{hostname}}{dom}"), &[dep]),
            format!("{dir}/rootca_{algo}.crt"),
        )));
        t.push(mk(with_pem(
            tpl(&tag("interca"), &format!("ssl import interca {{hostname}}{dom}"), &[&tag("rootca")]),
            format!("{dir}/interca_{algo}.crt"),
        )));
        t.push(mk(with_pem(
            tpl(&tag("crlca"), &format!("ssl import crlca {{hostname}}{dom}"), &[&tag("interca")]),
            format!("{dir}/rootca_{algo}.crt"),
        )));
        t.push(mk(with_index(
            with_pem(
                tpl(&tag("key"), &format!("ssl import key {{hostname}} {{index}}{dom}"), &[&tag("crlca")]),
                format!("{dir}/serverkey_{algo}.key"),
            ),
            index.clone(),
        )));
        t.push(mk(with_index(
            with_pem(
                tpl(&tag("cert"), &format!("ssl import certificate {{hostname}} {{index}}{dom}"), &[&tag("key")]),
                format!("{dir}/servercert_{algo}.crt"),
            ),
            index.clone(),
        )));
    }

    // 激活：host 级与 SNI 级各自依赖本级的导入任务。
    t.push(only_host(with_index(
        desc(
            tpl(
                "activate_host",
                &format!("ssl activate certificate {{hostname}} {{index}} \"\" {act_type}"),
                &["cert_host", "key_host"],
            ),
            "激活 host 级证书",
        ),
        index.clone(),
    )));
    t.push(only_domain(with_index(
        desc(
            tpl(
                "activate_sni",
                &format!("ssl activate certificate {{hostname}} {{index}} {{domain}} {act_type}"),
                &["cert_sni", "key_sni"],
            ),
            "激活 SNI 域名级证书",
        ),
        index,
    )));
    t
}

fn build_ssl_chain(opts: &ScenarioOptions) -> DispatchConfig {
    let p = &opts.params;
    let hostname = get(p, "hostname", "vhost");
    DispatchConfig {
        global: global_of(opts),
        targets: Targets {
            hostnames: vec![hostname],
            domains: split_domains(&get(p, "domains", "")),
        },
        tasks: ssl_chain_tasks(p, None),
        continue_on_error: false,
    }
}

fn build_slb_https_ssl(opts: &ScenarioOptions) -> DispatchConfig {
    let p = &opts.params;
    let real = get(p, "real", "rs_web");
    let real_ip = get(p, "real_ip", "10.0.0.11");
    let real_port = get(p, "real_port", "8443");
    let group = get(p, "group", "grp_web");
    let method = get(p, "method", "rr");
    let vs = get(p, "vs", "vs_https");
    let vip = get(p, "vip", "192.0.2.10");
    let vport = get(p, "vport", "443");
    let hostname = get(p, "hostname", "vhost");

    let mut head: Vec<TaskTemplate> = vec![
        desc(tpl("slb_real", &format!("slb real https {real} {real_ip} {real_port}"), &[]), "定义后台服务"),
        desc(tpl("slb_group", &format!("slb group method {group} {method}"), &["slb_real"]), "创建后台服务组并设置算法"),
        desc(tpl("slb_member", &format!("slb group member {group} {real}"), &["slb_group"]), "把后台服务加入服务组"),
        desc(tpl("slb_virtual", &format!("slb virtual https {vs} {vip} {vport}"), &[]), "定义 HTTPS 虚拟服务"),
        desc(tpl("slb_policy", &format!("slb policy static {vs} {real}"), &["slb_virtual", "slb_real"]), "静态策略：虚拟服务 → 后台服务"),
    ];
    let mut tasks = ssl_chain_tasks(p, Some(&vs));
    // SSL 虚拟主机必须建在虚拟服务之后。
    for t in tasks.iter_mut() {
        if t.id == "create" {
            t.depends_on = vec!["slb_virtual".to_string()];
            t.description = Some(format!("创建 SSL 虚拟主机并绑定 SLB 虚拟服务 {vs}"));
        }
    }
    head.append(&mut tasks);
    DispatchConfig {
        global: global_of(opts),
        targets: Targets {
            hostnames: vec![hostname],
            domains: split_domains(&get(p, "domains", "")),
        },
        tasks: head,
        continue_on_error: false,
    }
}

// ============================ 内置策略（与用户策略同一个库，不可删除） ============================

pub(crate) struct BuiltinStrategyDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub config: DispatchConfig,
}

/// 随应用提供的内置策略。`cmd_strategy_list` 会把它们和用户策略合并返回，
/// 并保证策略库永远非空；它们不可删除、不可覆盖（保存时自动另存为新策略）。
pub(crate) fn builtin_strategies() -> Vec<BuiltinStrategyDef> {
    let mut out = vec![
        BuiltinStrategyDef {
            id: "builtin:sm2_import",
            name: "SM2 国密双证书导入 + 激活",
            description: "导入 SM2 签名/加密私钥与证书，再激活 host 级证书。PEM 路径按 ./cfgfiles/SM2 预填，可在任务表里改。",
            config: build_sm2_import(),
        },
        BuiltinStrategyDef {
            id: "builtin:ssl_chain_rsa",
            name: "SSL 主机 + 证书链 + 激活（RSA）",
            description: "clear → 建虚拟主机 → 导入 rootca/interca/crlca → 导入 RSA 私钥与证书 → 激活；PEM 目录默认 ./cfgfiles/RSA。",
            config: no_creds(build_ssl_chain(&ScenarioOptions::default())),
        },
    ];

    let mut ecc = HashMap::new();
    ecc.insert("algo".to_string(), "ecc".to_string());
    ecc.insert("pem_dir".to_string(), "./cfgfiles/ECC".to_string());
    out.push(BuiltinStrategyDef {
        id: "builtin:ssl_chain_ecc",
        name: "SSL 主机 + 证书链 + 激活（ECC）",
        description: "同 RSA 流程，证书与私钥目录默认 ./cfgfiles/ECC。",
        config: no_creds(build_ssl_chain(&ScenarioOptions {
            params: ecc,
            ..ScenarioOptions::default()
        })),
    });

    out.push(BuiltinStrategyDef {
        id: "builtin:slb_https_ssl",
        name: "SLB HTTPS + SSL 证书链 + 激活",
        description: "后台服务 → 服务组 → HTTPS 虚拟服务 → 静态策略 → SSL 虚拟主机绑定 → SNI → 证书链 → 激活。",
        config: no_creds(build_slb_https_ssl(&ScenarioOptions::default())),
    });

    out.push(BuiltinStrategyDef {
        id: "builtin:import_one",
        name: "单独导入某类证书与私钥",
        description: "只导入一组私钥 + 证书（可再激活）。默认按 RSA 模板，改算法时把命令换成 ssl sm2 import ... 或与 PQC 对应即可。",
        config: build_import_one(),
    });
    out.push(BuiltinStrategyDef {
        id: "builtin:backup_restore",
        name: "全量备份 → 恢复",
        description: "先导入并激活 host 级、域名级证书，再 ssl backup certificate all，最后 ssl restore certificate 并确认 YES。实测备份文件名须为字母数字下划线 + .tgz。",
        config: build_backup_restore(),
    });
    out.push(BuiltinStrategyDef {
        id: "builtin:host_mtls",
        name: "新建主机 + 导入 + 激活 + 双向认证",
        description: "建 vhost/rhost → 导入证书链与私钥 → 激活 → 开启 clientauth → 设置 protocol/ciphersuite。",
        config: build_host_mtls(),
    });

    out
}

fn build_import_one() -> DispatchConfig {
    let mut key = tpl("import_key", "ssl import key {hostname} {index}", &[]);
    key = only_host(with_index(with_pem(key, "./cfgfiles/RSA/serverkey_rsa.key".into()), "1".into()));
    key.description = Some("导入私钥（RSA/ECC 用 ssl import key；国密改成 ssl sm2 import signkey）".into());
    let mut cert = tpl("import_cert", "ssl import certificate {hostname} {index}", &["import_key"]);
    cert = only_host(with_index(with_pem(cert, "./cfgfiles/RSA/servercert_rsa.crt".into()), "1".into()));
    cert.description = Some("导入证书（国密改成 ssl sm2 import signcertificate）".into());
    let mut act = tpl("activate", "ssl activate certificate {hostname} {index} \"\" all", &["import_cert"]);
    act = only_host(with_index(act, "1".into()));
    act.description = Some("激活证书".into());
    act.verify_cmd = Some("show ssl settings {hostname}".into());
    act.expect = vec!["{hostname}".into()];
    DispatchConfig {
        global: GlobalCfg::default(),
        targets: Targets { hostnames: vec!["vhost".into()], domains: vec![String::new()] },
        tasks: vec![key, cert, act],
        continue_on_error: false,
    }
}

fn build_backup_restore() -> DispatchConfig {
    let id = "1".to_string();
    let mk_host = |tid: &str, cmd: &str, pem: Option<&str>, deps: &[&str], desc: &str| {
        let mut t = tpl(tid, cmd, deps);
        t.index = Some(id.clone());
        t.only_if_no_domain = true;
        t.description = Some(desc.into());
        match pem {
            Some(p) => with_pem(t, p.into()),
            None => t,
        }
    };
    let mk_sni = |tid: &str, cmd: &str, pem: Option<&str>, deps: &[&str], desc: &str| {
        let mut t = tpl(tid, cmd, deps);
        t.index = Some(id.clone());
        t.only_if_domain = true;
        t.description = Some(desc.into());
        match pem {
            Some(p) => with_pem(t, p.into()),
            None => t,
        }
    };
    let mut backup = tpl("backup_all", "ssl backup certificate all \"guaxi_backup.tgz\" \"changeit\"", &["activate_sni"]);
    backup.description = Some("备份全量证书（文件名只能字母数字下划线，后缀 .tgz）".into());
    let mut restore = tpl("restore_all", "ssl restore certificate all \"guaxi_backup.tgz\" \"changeit\"", &["backup_all"]);
    restore.description = Some("恢复备份（设备会问 YES，自动应答）".into());
    restore.answers = vec![yes_answer("type yes to proceed")];
    restore.verify_cmd = Some("show ssl host".into());

    DispatchConfig {
        global: GlobalCfg::default(),
        targets: Targets { hostnames: vec!["vhost".into()], domains: vec![String::new(), "www.example.com".into()] },
        tasks: vec![
            mk_host("rootca_host", "ssl import rootca {hostname}", Some("./cfgfiles/RSA/rootca_rsa.crt"), &[], "host 级：导入根 CA"),
            mk_sni("rootca_sni", "ssl import rootca {hostname} {domain}", Some("./cfgfiles/RSA/rootca_rsa.crt"), &[], "域名级：导入根 CA"),
            mk_host("interca_host", "ssl import interca {hostname}", Some("./cfgfiles/RSA/interca_rsa.crt"), &["rootca_host"], "host 级：导入中间 CA"),
            mk_sni("interca_sni", "ssl import interca {hostname} {domain}", Some("./cfgfiles/RSA/interca_rsa.crt"), &["rootca_sni"], "域名级：导入中间 CA"),
            mk_host("key_host", "ssl import key {hostname} {index}", Some("./cfgfiles/RSA/serverkey_rsa.key"), &["interca_host"], "host 级：导入私钥"),
            mk_host("cert_host", "ssl import certificate {hostname} {index}", Some("./cfgfiles/RSA/servercert_rsa.crt"), &["key_host"], "host 级：导入证书"),
            mk_host("activate_host", "ssl activate certificate {hostname} {index} \"\" all", None, &["cert_host"], "host 级：激活"),
            mk_sni("key_sni", "ssl import key {hostname} {index} {domain}", Some("./cfgfiles/RSA/serverkey_rsa.key"), &["interca_sni"], "域名级：导入私钥"),
            mk_sni("cert_sni", "ssl import certificate {hostname} {index} {domain}", Some("./cfgfiles/RSA/servercert_rsa.crt"), &["key_sni"], "域名级：导入证书"),
            mk_sni("activate_sni", "ssl activate certificate {hostname} {index} {domain} all", None, &["cert_sni"], "域名级：激活"),
            backup,
            restore,
        ],
        continue_on_error: false,
    }
}

fn build_host_mtls() -> DispatchConfig {
    let mut create = tpl("create", "ssl host virtual {hostname}", &[]);
    create.description = Some("新建虚拟主机（后台主机改成 ssl host real {hostname}）".into());
    let mut sni = tpl("sni", "ssl sni {hostname} {domain}", &["create"]);
    sni.only_if_domain = true;
    let mut key = with_pem(tpl("key", "ssl import key {hostname} {index}", &["create"]), "./cfgfiles/RSA/serverkey_rsa.key".into());
    key.index = Some("1".into());
    key.only_if_no_domain = true;
    let mut cert = with_pem(tpl("cert", "ssl import certificate {hostname} {index}", &["key"]), "./cfgfiles/RSA/servercert_rsa.crt".into());
    cert.index = Some("1".into());
    cert.only_if_no_domain = true;
    let mut act = tpl("activate", "ssl activate certificate {hostname} {index} \"\" all", &["cert"]);
    act.index = Some("1".into());
    act.only_if_no_domain = true;
    act.verify_cmd = Some("show ssl settings {hostname}".into());

    let mut clientauth = tpl("clientauth", "ssl settings clientauth {hostname}", &["activate"]);
    clientauth.description = Some("开启双向认证（mTLS）".into());
    let mut protocol = tpl("protocol", "ssl settings protocol {hostname} \"TLSv1:TLSv11:TLSv12:TLSv13\"", &["clientauth"]);
    protocol.description = Some("设置 TLS 版本".into());
    let mut cipher = tpl("ciphersuite", "ssl settings ciphersuite {hostname} \"AES256-GCM-SHA384:AES128-GCM-SHA256\"", &["protocol"]);
    cipher.description = Some("设置套件（需与 protocol 兼容）".into());
    let mut start = tpl("start", "ssl start {hostname}", &["ciphersuite"]);
    start.description = Some("启动主机".into());

    DispatchConfig {
        global: GlobalCfg::default(),
        targets: Targets { hostnames: vec!["vhost".into()], domains: vec![String::new(), "www.example.com".into()] },
        tasks: vec![create, sni, key, cert, act, clientauth, protocol, cipher, start],
        continue_on_error: false,
    }
}

/// 内置策略不携带任何连接凭据。
fn no_creds(mut cfg: DispatchConfig) -> DispatchConfig {
    cfg.global = GlobalCfg::default();
    cfg
}

fn sm2_task(
    id: &str,
    cmd: &str,
    pem: Option<String>,
    deps: &[&str],
    description: &str,
) -> TaskTemplate {
    TaskTemplate {
        id: id.to_string(),
        cmd: cmd.to_string(),
        pem_file: pem,
        index: Some("1".to_string()),
        depends_on: deps.iter().map(|s| s.to_string()).collect(),
        only_if_domain: false,
        only_if_no_domain: true,
        description: Some(description.to_string()),
        verify_cmd: None,
        expect: vec![],
        answers: vec![],
    }
}

fn build_sm2_import() -> DispatchConfig {
    let dir = "./cfgfiles/SM2";
    DispatchConfig {
        global: GlobalCfg::default(),
        targets: Targets {
            hostnames: vec!["vhost".to_string()],
            domains: vec![String::new()],
        },
        tasks: vec![
            sm2_task(
                "sm2_sign_key_host",
                "ssl sm2 import signkey {hostname} {index}",
                Some(format!("{dir}/sm2signkey.key")),
                &[],
                "导入 SM2 签名私钥",
            ),
            sm2_task(
                "sm2_enc_key_host",
                "ssl sm2 import enckey {hostname} {index}",
                Some(format!("{dir}/sm2enckey.key")),
                &[],
                "导入 SM2 加密私钥",
            ),
            sm2_task(
                "sm2_sign_cert_host",
                "ssl sm2 import signcertificate {hostname} {index}",
                Some(format!("{dir}/sm2signcert.crt")),
                &["sm2_sign_key_host"],
                "导入 SM2 签名证书（需先导入签名私钥）",
            ),
            sm2_task(
                "sm2_enc_cert_host",
                "ssl sm2 import enccertificate {hostname} {index}",
                Some(format!("{dir}/sm2encert.crt")),
                &["sm2_enc_key_host"],
                "导入 SM2 加密证书（需先导入加密私钥）",
            ),
            sm2_task(
                "activate_host",
                "ssl activate certificate {hostname} {index} \"\" all",
                None,
                &["sm2_sign_cert_host", "sm2_enc_cert_host"],
                "激活证书；建议给该任务配校验命令（如 show ssl host）与期望关键字",
            ),
        ],
        continue_on_error: false,
    }
}
