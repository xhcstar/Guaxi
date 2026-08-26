// 获取随 Guaxi 分发的 codex 可执行文件。
//
// 设计说明
// - codex 官方 release 的裸 exe 有 ~300MB，直接塞进安装包不现实；这里下载 zstd 压缩资产
//   （Windows x64 约 73MB）并解压到 `src-tauri/utils/codex/`，走项目已有的 utils 外部分包
//   机制分发（与 utils/llama、utils/sherpa-onnx 同一条路），安装包本体不会因此膨胀。
// - 版本固定在 PINNED_TAG：codex 的 `exec --json` 事件结构会随版本演进，升级要走一次回归。
// - 拿不到网络时只告警不失败：codex 缺失时智能助手会自动回退到内置 Python Agent，
//   不能因为下载失败就让整个 dev / build 流程挂掉。
//
// 用法：
//   node ./tools/fetch_codex_sidecar.mjs          # 已存在则跳过
//   node ./tools/fetch_codex_sidecar.mjs --force  # 强制重新下载

import { createHash } from 'node:crypto';
import { mkdir, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

const PINNED_TAG = 'rust-v0.149.1';
const RELEASE_BASE = `https://github.com/openai/codex/releases/download/${PINNED_TAG}`;
// 整个下载的上限，超过就放弃（归档约 100MB）。
const TOTAL_TIMEOUT_MS = 15 * 60 * 1000;
// 多久没有新数据就认为连接已经卡死。
const STALL_TIMEOUT_MS = 45 * 1000;
// 建连 + 拿到响应头的上限。
const HANDSHAKE_TIMEOUT_MS = 30 * 1000;

const projectRoot = process.cwd();
const targetDir = path.join(projectRoot, 'src-tauri', 'utils', 'codex');
const force = process.argv.includes('--force');

// release 资产名 → 平台。key 为 `${process.platform}-${process.arch}`。
// `base` 是归档内可执行文件的名字，归档名固定为 `${base}.tar.gz`
// （统一用 tar.gz 而不是体积更小的 zst：系统自带的 tar 就能解，不需要额外依赖）。
// `sha256` 是归档的校验和，已核对过的平台才有；留空表示只记录不校验。
const ASSETS = {
  'win32-x64': {
    base: 'codex-x86_64-pc-windows-msvc.exe',
    binary: 'codex.exe',
    sha256: '3f770d347380a1ccec08ac2b4d43f86dcf7bee1f924bf48c57241f66b6901959',
  },
  'win32-arm64': { base: 'codex-aarch64-pc-windows-msvc.exe', binary: 'codex.exe', sha256: '' },
  'darwin-x64': { base: 'codex-x86_64-apple-darwin', binary: 'codex', sha256: '' },
  'darwin-arm64': { base: 'codex-aarch64-apple-darwin', binary: 'codex', sha256: '' },
  'linux-x64': { base: 'codex-x86_64-unknown-linux-musl', binary: 'codex', sha256: '' },
  'linux-arm64': { base: 'codex-aarch64-unknown-linux-musl', binary: 'codex', sha256: '' },
};

function warn(message) {
  console.warn(`[codex.fetch] ${message}`);
}

function info(message) {
  console.log(`[codex.fetch] ${message}`);
}

async function exists(target) {
  try {
    await stat(target);
    return true;
  } catch {
    return false;
  }
}

function run(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: options.cwd || projectRoot,
      stdio: 'inherit',
      shell: false,
      env: { ...process.env, ...(options.env || {}) },
    });
    child.on('exit', (code) => resolve(code ?? 1));
    child.on('error', () => resolve(1));
  });
}

function capture(command, args) {
  return new Promise((resolve) => {
    const child = spawn(command, args, { cwd: projectRoot, shell: false });
    let out = '';
    child.stdout?.on('data', (chunk) => {
      out += chunk.toString();
    });
    child.on('exit', (code) => resolve({ code: code ?? 1, stdout: out }));
    child.on('error', () => resolve({ code: 1, stdout: '' }));
  });
}

// 解析系统代理。
//
// 设计说明：GitHub release 在国内经常直连超时，而用户往往已经有一个可用的系统代理。
// 优先级：显式的 *_PROXY 环境变量 > Windows 注册表里的 IE/系统代理。都没有就直连。
async function resolveSystemProxy() {
  const fromEnv =
    process.env.HTTPS_PROXY ||
    process.env.https_proxy ||
    process.env.ALL_PROXY ||
    process.env.all_proxy ||
    process.env.HTTP_PROXY ||
    process.env.http_proxy;
  if (fromEnv) {
    return { url: fromEnv, source: 'env' };
  }

  if (process.platform !== 'win32') {
    return null;
  }

  const key = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings';
  const { code, stdout } = await capture('reg', ['query', key]);
  if (code !== 0) {
    return null;
  }
  // ProxyEnable 是 REG_DWORD，关闭时为 0x0；关闭状态下 ProxyServer 里可能还留着旧值，必须先看开关。
  const enabled = /ProxyEnable\s+REG_DWORD\s+0x1/i.test(stdout);
  if (!enabled) {
    return null;
  }
  const match = stdout.match(/ProxyServer\s+REG_SZ\s+(\S+)/i);
  if (!match) {
    return null;
  }
  // ProxyServer 可能是 `host:port`，也可能是 `http=host:port;https=host:port` 这种分协议写法。
  const raw = match[1];
  let host = raw;
  if (raw.includes('=')) {
    const perProtocol = Object.fromEntries(
      raw.split(';').map((part) => {
        const [scheme, value] = part.split('=');
        return [scheme?.trim().toLowerCase(), value?.trim()];
      }),
    );
    host = perProtocol.https || perProtocol.http || '';
  }
  if (!host) {
    return null;
  }
  return { url: /^\w+:\/\//.test(host) ? host : `http://${host}`, source: 'windows-registry' };
}

// 用 curl 下载。
//
// 优先走 curl 而不是 node 的 fetch：curl 原生支持代理、重定向、断点续传和"低速即放弃"，
// 不用自己在 JS 里重建这些机制。Windows 10 1803+ / macOS / 主流 Linux 都自带 curl。
async function downloadWithCurl(url, destination, proxy) {
  const args = [
    '-fL',
    '--connect-timeout',
    '30',
    '--max-time',
    String(Math.round(TOTAL_TIMEOUT_MS / 1000)),
    // 低速看门狗：连续 STALL_TIMEOUT 秒速度低于 1KB/s 就放弃，避免卡死构建。
    '--speed-time',
    String(Math.round(STALL_TIMEOUT_MS / 1000)),
    '--speed-limit',
    '1024',
    '--retry',
    '2',
    '--progress-bar',
    '-o',
    destination,
    url,
  ];
  if (proxy) {
    args.unshift('--proxy', proxy.url);
  }
  return run('curl', args);
}

async function downloadWithFetch(url, destination) {
  const { createWriteStream } = await import('node:fs');
  const { PassThrough, Readable } = await import('node:stream');
  const { pipeline } = await import('node:stream/promises');

  // 没有 curl 时的兜底路径。注意 node 的 fetch 不读 *_PROXY 环境变量，
  // 所以这条路走不通代理；有代理需求时应该让 curl 可用。
  const controller = new AbortController();
  // 握手阶段单独限时：连接建立不起来时 fetch 自己不会返回，后面的看门狗也就没机会装上。
  const handshakeTimer = setTimeout(() => controller.abort(), HANDSHAKE_TIMEOUT_MS);
  let response;
  try {
    response = await fetch(url, { redirect: 'follow', signal: controller.signal });
  } finally {
    clearTimeout(handshakeTimer);
  }
  if (!response.ok || !response.body) {
    throw new Error(`HTTP ${response.status} ${response.statusText} for ${url}`);
  }

  const source = Readable.fromWeb(response.body);
  // 统计字节数必须放在管道里做。直接给 source 挂 'data' 会把它切到 flowing 模式，
  // 数据会被监听器吃掉，pipeline 那头就收不到完整内容了。
  const meter = new PassThrough();
  const total = Number(response.headers.get('content-length') || 0);
  const startedAt = Date.now();
  let received = 0;
  let lastLogged = 0;
  let lastProgressAt = Date.now();

  meter.on('data', (chunk) => {
    received += chunk.length;
    lastProgressAt = Date.now();
    if (received - lastLogged >= 16 * 1024 * 1024) {
      lastLogged = received;
      const totalText = total ? ` / ${(total / 1048576).toFixed(0)}MB` : '';
      info(`已下载 ${(received / 1048576).toFixed(0)}MB${totalText}`);
    }
  });

  // 看门狗用"直接 reject 外层 promise"的方式生效，不依赖销毁流能否把错误传出来：
  // 被墙的连接销毁后底层 socket 有时仍不返回，只有让调用方先走掉才能保证不卡住构建。
  // 真正的资源回收交给脚本末尾的 process.exit。
  let watchdog = null;
  const stalled = new Promise((_, reject) => {
    watchdog = setInterval(() => {
      const idleMs = Date.now() - lastProgressAt;
      if (idleMs > STALL_TIMEOUT_MS) {
        controller.abort();
        source.destroy();
        reject(new Error(`下载停滞 ${Math.round(idleMs / 1000)} 秒（已收到 ${received} 字节）`));
      } else if (Date.now() - startedAt > TOTAL_TIMEOUT_MS) {
        controller.abort();
        source.destroy();
        reject(new Error(`下载超过 ${Math.round(TOTAL_TIMEOUT_MS / 60000)} 分钟仍未完成`));
      }
    }, 2000);
  });

  try {
    await Promise.race([pipeline(source, meter, createWriteStream(destination)), stalled]);
  } finally {
    if (watchdog) clearInterval(watchdog);
  }
}

// 下载入口：能用 curl 就用 curl（支持系统代理），否则退回 node fetch。
async function download(url, destination) {
  const proxy = await resolveSystemProxy();
  const hasCurl = (await capture('curl', ['--version'])).code === 0;

  if (hasCurl) {
    if (proxy) {
      info(`使用系统代理下载（来源：${proxy.source}）：${proxy.url}`);
    }
    const code = await downloadWithCurl(url, destination, proxy);
    if (code === 0) {
      return;
    }
    // 代理不通时再直连试一次：有些环境的代理只对特定站点生效。
    if (proxy) {
      warn(`经代理下载失败（curl 退出码 ${code}），改为直连重试`);
      const directCode = await downloadWithCurl(url, destination, null);
      if (directCode === 0) {
        return;
      }
      throw new Error(`curl 下载失败（代理与直连都失败，退出码 ${directCode}）`);
    }
    throw new Error(`curl 下载失败（退出码 ${code}）`);
  }

  if (proxy) {
    warn('未找到 curl，无法使用系统代理，改用 node fetch 直连');
  }
  await downloadWithFetch(url, destination);
}

async function sha256(file) {
  const hash = createHash('sha256');
  hash.update(await readFile(file));
  return hash.digest('hex');
}

// 记录已安装的版本与校验和，既让重复执行可以跳过，也方便把 sha256 固定进版本库。
const markerPath = () => path.join(targetDir, '.codex-version.json');

async function readMarker() {
  try {
    return JSON.parse(await readFile(markerPath(), 'utf8'));
  } catch {
    return null;
  }
}

async function main() {
  const platformKey = `${process.platform}-${process.arch}`;
  const entry = ASSETS[platformKey];
  if (!entry) {
    warn(`当前平台 ${platformKey} 没有对应的 codex release 资产，跳过。Agent 将回退到内置引擎。`);
    return;
  }

  const binaryPath = path.join(targetDir, entry.binary);
  const marker = await readMarker();
  if (!force && (await exists(binaryPath)) && marker?.tag === PINNED_TAG) {
    info(`已存在 ${PINNED_TAG} 的 codex，跳过下载：${binaryPath}`);
    return;
  }

  await mkdir(targetDir, { recursive: true });
  const archiveName = `${entry.base}.tar.gz`;
  const url = `${RELEASE_BASE}/${archiveName}`;
  const archivePath = path.join(targetDir, `.${archiveName}.download`);

  info(`下载 ${url}`);
  try {
    await download(url, archivePath);
  } catch (error) {
    // 下载失败不能让构建挂掉：codex 缺失时 Agent 会自动回退到内置 Python 引擎。
    warn(`下载 codex 失败：${error instanceof Error ? error.message : String(error)}`);
    warn('Agent 将回退到内置 Python 引擎。可稍后重试 `npm run codex:fetch`。');
    // Windows 上写句柄可能还没释放，删不掉半成品也不算错误；下次运行会覆盖它。
    await rm(archivePath, { force: true }).catch(() => {});
    return;
  }

  const digest = await sha256(archivePath);
  info(`sha256(${archiveName}) = ${digest}`);
  if (entry.sha256 && entry.sha256 !== digest) {
    // 校验和不符可能是版本被重打包，也可能是下载被中间人篡改，两种都不该继续解压。
    await rm(archivePath, { force: true }).catch(() => {});
    warn(`校验和不匹配：期望 ${entry.sha256}，实际 ${digest}。已丢弃下载内容。`);
    return;
  }

  // 解压时把工作目录切到 targetDir，只传相对文件名。
  // Windows 上 PATH 里的 tar 可能是 GNU tar（Git for Windows 自带），它会把 `D:\...`
  // 当成 `host:path` 远程规格去解析，直接报 "Cannot connect to D"。用相对路径绕开冒号，
  // 这样 GNU tar 和 Windows 自带的 bsdtar 都能正常工作。
  const extractCode = await run('tar', ['-xzf', path.basename(archivePath)], { cwd: targetDir });
  await rm(archivePath, { force: true }).catch(() => {});
  if (extractCode !== 0) {
    warn('解压 codex 归档失败，Agent 将回退到内置 Python 引擎。');
    return;
  }

  // 归档里的可执行文件名带 target triple，重命名成固定名字，运行时才好定位。
  const extractedPath = path.join(targetDir, entry.base);
  if (await exists(extractedPath)) {
    await rename(extractedPath, binaryPath);
  }
  if (!(await exists(binaryPath))) {
    warn(`解压后未找到 ${entry.binary}，请检查归档结构：${targetDir}`);
    return;
  }

  if (process.platform !== 'win32') {
    await run('chmod', ['+x', path.basename(binaryPath)], { cwd: targetDir });
  }

  await writeFile(
    markerPath(),
    `${JSON.stringify({ tag: PINNED_TAG, asset: archiveName, sha256: digest }, null, 2)}\n`,
    'utf8',
  );
  info(`codex ${PINNED_TAG} 就绪：${binaryPath}`);
}

main()
  .catch((error) => {
    // 只告警：这个步骤是构建的可选增强，不是硬依赖。
    warn(error instanceof Error ? error.message : String(error));
  })
  .finally(() => {
    // 显式退出。下载被中断时 undici 的 socket 句柄可能仍然活着，
    // 事件循环不会自己排空，进程会挂住把 `npm run tauri` 一起拖死。
    process.exit(0);
  });
