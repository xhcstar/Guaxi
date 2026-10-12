# 终端调试工坊 · 使用文档

> 适用版本：分支 `video`。设计见 [remote-terminal-workshop-design.md](./remote-terminal-workshop-design.md)。
> 入口：应用页「工具类 → 终端调试工坊」，在独立窗口打开。顶部五个标签：日志分析 / 实时终端 / GDB 调试 / 工具箱 / 网络。切标签内容与会话都会保留。亮/暗主题跟随应用主题。

## 1. 实时终端

1. 左栏「连接会话」→「新建」，选 SSH / 串口 / Telnet：
   - SSH：名称、分组（文件夹，空=未分组）、主机/端口/用户、认证方式（默认密钥 / 指定私钥 / 密码，密码存系统 keychain）。
   - 串口：名称、分组、串口（点刷新枚举）、波特率。
   - Telnet：名称、分组、主机/端口。
   保存后出现在左栏，按分组文件夹折叠、列表可垂直滚动。点条目即连接。
2. 连接成功后：工具条可查找/复制/粘贴/保存、选中即复制；底部可录制宏并重放；SSH 会自动连同主机 **SFTP**（左栏下方，可上传/下载/新建/删除/重命名，上传新文件已修复）。
3. 「会话录制」在右侧栏（可折叠/拖宽），点一条可在「日志分析」里回放分析。

## 2. GDB 调试

1. 选 SSH 档案，选模式：
   - **启动程序**：填可执行文件路径（+参数）。
   - **附加进程**：填 PID；若目标进程非本会话子进程，常需 **勾选 sudo**（要求目标机免密 sudo，即 `NOPASSWD`）。
2. 点「启动 gdb」。附加模式会自动 `-target-attach`，成功后进程**已停住**，状态栏提示"加断点后点继续"。
3. 若附加失败：空状态区会显示 gdb/sudo 的真实报错并给出对策：
   - `ptrace: Operation not permitted`：勾 sudo 重试；或目标机 root 执行 `echo 0 > /proc/sys/kernel/yama/ptrace_scope`；或确保与目标进程同一用户。
   - `must have a tty` / `a password is required`：目标机 sudo 需配 `NOPASSWD`（PTY 已自动申请）。
4. 调试面板：断点、线程、调用栈（点帧切换）、局部变量、变量树（可切 hex/十进制/二进制看复杂结构）、Watch、寄存器、内存（hex+ASCII）。
5. 源码对照：居中上方面板选「本地源码 / 远端源码」，填「编译路径前缀 → 本地/远端根」映射，停在断点时自动高亮当前行。控制台字号可调。

## 3. 工具箱

- **计算器 → IP 地址转换**：IPv4 与 **IPv6** 都支持。IPv4 可输点分/十进制/0x 十六进制/CIDR；IPv6 可输 `2001:db8::1`、`::1`、`fe80::1`、`::ffff:192.168.1.1`，或 CIDR `2001:db8::/32`。回车或输入即出结果：压缩/展开写法、128 位十六进制与十进制、二进制分组、地址类型（环回/链路本地/ULA/组播/全球单播/6to4/Teredo 等）；IPv6 CIDR 另给前缀掩码、网络与末地址、可用范围、地址总数与可用主机数。
- **Coredump**：选档案 + 可执行文件 + core 路径 →「分析」，输出满高、字号可调。
- **远程执行**：选档案 + 命令执行；可定时采集，输出可「导入分析」。
- **二进制分析**：选 ELF/PE，看各段熵（>7 多为加密/压缩）。

### 3.1 证书 / 密钥（工具箱 → 证书·密钥）
- **解析/导入**：选 cert/key/csr/**pfx** 文件（PFX 填密码），或粘贴 PEM/base64/hex；出摘要 + ASN.1。
- **证书链/签发**（推荐流程，CA 库驱动，无需粘贴 PEM）：
  1. 填「主题信息」(C/ST/L/O/OU) + 算法/天数（共用）。
  2.「新建 CA」：填名称 + CN，上级留空=自签根 CA、选上级=中间 CA → 生成后进「CA 库」。
  3.「签发终端证书」：选签发 CA（下拉）+ server/client + CN + SAN → 出证书与私钥，可保存。
  4. CA 库可导出（.crt/.key）、删除；也可「导入已有 CA 到库」。
  5. 页内还提供独立「密钥生成 / CSR / 签名验签」区（即用即生成、可保存）。
- **PFX**：两个方向都支持。
  - 打包：填私钥 + 证书链（叶证书在前）+ 密码 → 保存 .pfx。
  - **解包**：填该 PFX 的保护密码（未加密留空）→ 点文件夹图标选外部 .pfx/.p12 → 列出包内全部私钥与证书，逐个「查看」(眼图标)/「保存」。
- **加解密**（单标签，分三页，每项都有悬停说明）：
  - 对称：算法下拉分三组——**分组密码**（AES-128/192/256、SM4、Camellia-128/192/256、Twofish、Serpent、DES、3DES、Blowfish、CAST5、RC2、IDEA）、**流密码**（ChaCha20、Salsa20）、**AEAD**（AES-128/256-GCM、ChaCha20-Poly1305）。分组密码再选模式（ECB/CBC/CFB/OFB/CTR），ECB 不要 IV，ECB/CBC 可勾 PKCS7 填充（不勾则要求块对齐）；AEAD 可填 AAD。key/IV 旁都有「生成」按钮（按算法自动取正确长度），界面会提示当前算法需要多少字节。
  - 非对称(RSA)：公钥加密 / 私钥解密，方案可选 OAEP+SHA-1/256/384/512 或 PKCS#1 v1.5。
  - 散列：MD5/SHA-1/256/384/512。
- **PFX** 与「加解密」页的按钮均为图标，悬停即见说明。

## 4. 网络
报文构造/解析、TCP/TLS 握手探测（协议/套件/证书链）、抓包（需装 Npcap）。TLCP(国密) 探测随下文 `tlcp` 特性启用。

## 5. [可选] 启用 SM2 / TLCP / PQC（Tongsuo，cargo feature `tlcp`）

默认构建为纯 Rust、不含国密 TLCP。要启用：

1. 编译安装 **Tongsuo 8.5+**：
   ```bash
   git clone https://github.com/Tongsuo-Project/Tongsuo   # 国内镜像 https://gitee.com/babassl/Tongsuo
   cd Tongsuo && git checkout 8.5-stable
   ./config shared --prefix=/opt/tongsuo85 && make -j && make install
   /opt/tongsuo85/bin/tongsuo version -a   # 应含 SM2/SM3/SM4/ML-KEM/ML-DSA
   ```
   Windows 需 Strawberry Perl + VS Build Tools + NASM。
2. 带特性构建（指向 Tongsuo，勿用 vendored）：
   ```bash
   OPENSSL_DIR=/opt/tongsuo85 OPENSSL_STATIC=0 \
     cargo build --features tlcp --bin guaxi
   ```
3. 启用后：证书页的「SM2」类 CA 可本地签发 SM2 证书链；网络页出现「TLCP(国密)」探测（双证书 + 国密套件）；加解密/密钥页出现 SM2/SM3/SM4 与 ML-KEM/ML-DSA（PQC 为叠加增强，非密评替代项）。
4. 排错：`cargo tree -i openssl-sys` 确认只有一个实例（避免与其它 TLS 库符号冲突）；TLCP 握手失败先抓包看 Certificate 报文中签名/加密证书顺序；NTLS FFI 函数名须与所装 Tongsuo 版本一致。

> 注：`tlcp` 路径依赖本机已装 Tongsuo，需在该机器上自行验证；它不随默认 release 打包。
