#!/usr/bin/env node
// 准备语音功能所需的原生库与模型。
//
// 为什么要有这个脚本：
//   · sherpa-onnx 的 Rust crate 默认会在构建时自己去 GitHub 下载预编译库，而构建机
//     常常只有系统代理（注册表里）、没有 *_PROXY 环境变量，cargo 会直接卡在下载上。
//     所以这里把归档预先取到本地，构建期零联网（见 src-tauri/.cargo/config.toml）。
//   · 模型都不进 git（.gitignore 忽略了 utils/Models 下的相关目录），只能靠脚本获取。
//
// 用法：
//   node tools/fetch_sherpa_onnx_libs.mjs            # 缺什么补什么
//   node tools/fetch_sherpa_onnx_libs.mjs --force    # 全部重下
//   node tools/fetch_sherpa_onnx_libs.mjs --cuda     # 同时取 CUDA 版原生库与 CUDA 运行时（约 2GB）
//
// 下载走 curl：它原生支持代理、重定向、断点续传和「低速即放弃」，不用自己重建这些机制。

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, copyFile, readFile, readdir, rename, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const VERSION = '1.13.8';
const REPO = 'https://github.com/k2-fsa/sherpa-onnx/releases/download';
const RELEASE = `${REPO}/v${VERSION}`;
const TTS_MODELS = `${REPO}/tts-models`;
const ASR_MODELS = `${REPO}/asr-models`;
const PUNCT_MODELS = `${REPO}/punctuation-models`;

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC_TAURI = resolve(HERE, '..', 'src-tauri');
const LIBS_ROOT = join(SRC_TAURI, '.sherpa-onnx-libs', VERSION);
const MODELS_ROOT = join(SRC_TAURI, 'utils', 'Models');

// 下载策略：模型动辄几百 MB，而 runner / 公司网络到 GitHub release CDN 的单条连接
// 经常被限速到几十 KB/s。所以这里三件事一起做：
//   · 单次尝试只给 20 分钟，超时就重试（配合 -C - 从 .part 断点续传，进度不会丢）；
//   · 连续 60 秒低于 10 KB/s 才判定为「死连接」并重连——旧值 1 KB/s 等于「再慢也不断」，
//     日志里看起来就是一条卡了几十分钟不动的下载；
//   · 多个文件并发下载，把总带宽用起来。
const ATTEMPT_TIMEOUT_MS = 20 * 60 * 1000;
const STALL_TIMEOUT_MS = 60 * 1000;
const STALL_SPEED_LIMIT = 10 * 1024;
const DOWNLOAD_RETRIES = 6;
const DOWNLOAD_CONCURRENCY = 3;

// 解压看门狗。bz2 解压是单线程的，一个大包卡住时整条构建会静默耗到 job 超时
// （v0.1.1 的 CI 就是这样躺了 6 小时，日志停在「下载完成」之后再无输出）。
// 所以解压期间每 15 秒报一次已写入字节数，并盯着目录是否还在增长：
//   · 连续 4 分钟没有增长 → 判定卡死，杀掉进程树换下一个 tar；
//   · 单包最长 40 分钟，即使进展极慢也不再等。
const EXTRACT_PROGRESS_MS = 15 * 1000;
const EXTRACT_STALL_MS = 4 * 60 * 1000;
const EXTRACT_TIMEOUT_MS = 40 * 60 * 1000;

const force = process.argv.includes('--force');
const withCuda = process.argv.includes('--cuda');
const skipModels = process.argv.includes('--libs-only') || process.env.SHERPA_SKIP_MODELS === '1';

function info(message) {
  console.log(`[sherpa] ${message}`);
}

function warn(message) {
  console.warn(`[sherpa] ${message}`);
}

function run(command, args, options = {}) {
  return new Promise((resolvePromise) => {
    let stderr = '';
    const child = spawn(command, args, { stdio: ['ignore', 'ignore', 'pipe'], ...options });
    child.stderr?.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('close', (code) => resolvePromise({ code: code ?? 1, stderr }));
    child.on('error', (error) => resolvePromise({ code: 1, stderr: error.message }));
  });
}

function capture(command, args) {
  return new Promise((resolvePromise) => {
    let stdout = '';
    const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'ignore'] });
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.on('close', (code) => resolvePromise({ code: code ?? 1, stdout }));
    child.on('error', () => resolvePromise({ code: 1, stdout: '' }));
  });
}

// 解析系统代理：显式环境变量优先，其次是 Windows 注册表里的系统代理。
//
// 设计说明：GitHub release 在国内经常直连超时，而用户往往已经有一个可用的系统代理。
// 取不到就直连，不影响在能直连的机器上使用。
async function resolveSystemProxy() {
  const fromEnv =
    process.env.HTTPS_PROXY ||
    process.env.https_proxy ||
    process.env.ALL_PROXY ||
    process.env.all_proxy ||
    process.env.HTTP_PROXY ||
    process.env.http_proxy;
  if (fromEnv) {
    return fromEnv;
  }
  if (process.platform !== 'win32') {
    return null;
  }

  const key = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings';
  const { code, stdout } = await capture('reg', ['query', key]);
  if (code !== 0) {
    return null;
  }
  // ProxyEnable 关掉时 ProxyServer 里可能还留着旧值，必须先看开关。
  if (!/ProxyEnable\s+REG_DWORD\s+0x1/i.test(stdout)) {
    return null;
  }
  const match = stdout.match(/ProxyServer\s+REG_SZ\s+(\S+)/i);
  if (!match) {
    return null;
  }
  // 可能是 `host:port`，也可能是 `http=host:port;https=host:port` 的分协议写法。
  const raw = match[1];
  let host = raw;
  if (raw.includes('=')) {
    const parts = Object.fromEntries(
      raw.split(';').map((part) => {
        const [scheme, value] = part.split('=');
        return [scheme?.trim().toLowerCase(), value?.trim()];
      }),
    );
    host = parts.https || parts.http || '';
  }
  if (!host) {
    return null;
  }
  return /^\w+:\/\//.test(host) ? host : `http://${host}`;
}

async function download(url, destination, proxy) {
  const partial = `${destination}.part`;
  await mkdir(dirname(partial), { recursive: true });
  if (force) {
    await rm(partial, { force: true });
  }

  const args = [
    '-fL',
    '--connect-timeout',
    '30',
    '--max-time',
    String(Math.round(ATTEMPT_TIMEOUT_MS / 1000)),
    '--speed-limit',
    String(STALL_SPEED_LIMIT),
    '--speed-time',
    String(Math.round(STALL_TIMEOUT_MS / 1000)),
    '--retry',
    String(DOWNLOAD_RETRIES),
    '--retry-delay',
    '2',
    '--retry-all-errors',
    '--progress-bar',
    '-o',
    partial,
  ];
  if (proxy) {
    args.push('--proxy', proxy);
  }
  // 断点续传：模型动辄上 G，而链路经常中途挂住。保留 .part 让 curl 从断点继续，
  // 比整个重下划算得多；--retry-all-errors 才能把连接层错误也纳入重试。
  args.push('-C', '-');
  args.push(url);

  // curl 的进度条走 stderr，被 run() 收进内存不显示，日志里只剩编排层的心跳。
  // 这里自己按文件大小报进度和平均速率，卡住时一眼能看出是真慢还是断了。
  const label = basename(destination);
  const startedAt = Date.now();
  const progressTimer = setInterval(async () => {
    const bytes = await fileSize(partial);
    const seconds = Math.max(1, (Date.now() - startedAt) / 1000);
    info(
      `下载中 ${label}：${(bytes / 1048576).toFixed(1)} MB · 平均 ${(bytes / 1024 / seconds).toFixed(0)} KB/s`,
    );
  }, 15000);
  progressTimer.unref?.();

  let result;
  try {
    result = await run('curl', args);
  } finally {
    clearInterval(progressTimer);
  }

  if (result.code !== 0) {
    // 失败时保留 .part，下次继续；真正的完整性由后续解压（bzip2 CRC）把关。
    throw new Error(`下载失败：${url}\n${result.stderr.trim()}`);
  }
  await rm(destination, { force: true });
  await rename(partial, destination);
  info(`完成下载 ${label}：${((await fileSize(destination)) / 1048576).toFixed(1)} MB`);
}

async function fileSize(path) {
  try {
    return (await stat(path)).size;
  } catch {
    return 0;
  }
}

async function directorySize(path) {
  let total = 0;
  const entries = await readdir(path, { withFileTypes: true }).catch(() => []);
  for (const entry of entries) {
    const child = join(path, entry.name);
    total += entry.isDirectory() ? await directorySize(child) : await fileSize(child);
  }
  return total;
}

// 解压 .tar.bz2。
//
// Windows 上有个坑：从 Git Bash 里调起的 GNU tar 会把 `D:\...` 当成 `主机:路径`
// 去连远程（报 "Cannot connect to D: resolve failed"），而从 cmd 里调起的 bsdtar 又正常。
// 所以这里一律传**相对文件名**并指定 cwd，彻底绕开盘符里的冒号。
//
// 另一个坑是解压工具本身：v0.1.1 的 CI 卡在这里耗掉了 6 小时——下载早就完成，三个 tar
// 进程都还活着、没有任何输出、一个包都没落地。Windows 自带的 bsdtar 解大 .tar.bz2 时
// 会偶发这种静默卡死；Git for Windows 的 GNU tar 走 bzip2 子进程，实测更稳，所以优先用它，
// 失败再退回 PATH 里的 tar。无论用哪个都带看门狗，卡死几分钟内就会暴露并自动换下一个。
function tarCandidates() {
  const candidates = new Set();
  const roots = [
    process.env.ProgramFiles,
    process.env['ProgramFiles(x86)'],
    process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, 'Programs') : null,
  ].filter(Boolean);
  for (const root of roots) {
    const candidate = join(root, 'Git', 'usr', 'bin', 'tar.exe');
    if (existsSync(candidate)) {
      candidates.add(candidate);
    }
  }
  return candidates;
}

/// 候选 tar：Git for Windows 自带的 GNU tar 优先，其次 PATH 里的 tar。
/// Git 不一定装在 Program Files（也可以是 scoop / 自定义目录），所以再按 `where git.exe`
/// 的位置反推 `..\usr\bin\tar.exe`。
async function resolveTarCandidates() {
  const candidates = tarCandidates();
  if (process.platform === 'win32') {
    const { code, stdout } = await capture('where.exe', ['git.exe']);
    if (code === 0) {
      for (const line of stdout.split(/\r?\n/)) {
        const git = line.trim();
        const candidate = git ? join(dirname(git), '..', 'usr', 'bin', 'tar.exe') : '';
        if (candidate && existsSync(candidate)) {
          candidates.add(candidate);
        }
      }
    }
  }
  return [...candidates, 'tar'];
}

function killProcessTree(child) {
  if (process.platform === 'win32' && child.pid) {
    // tar 只负责编排，真正解 bz2 的是它拉起的子进程，必须整棵树结束。
    spawn('taskkill', ['/pid', String(child.pid), '/t', '/f'], { stdio: 'ignore' });
    return;
  }
  child.kill('SIGKILL');
}

function extractWithTar(tar, archiveName, destinationName, cwd, destination) {
  return new Promise((resolvePromise) => {
    const child = spawn(tar, ['-xjf', archiveName, '-C', destinationName], {
      cwd,
      stdio: ['ignore', 'ignore', 'pipe'],
    });
    let stderr = '';
    child.stderr?.on('data', (chunk) => {
      stderr += chunk;
    });

    const startedAt = Date.now();
    let written = 0;
    let grownAt = startedAt;
    let stopReason = null;
    let probing = false;
    let settled = false;
    let timer = null;

    const settle = (code, error) => {
      if (settled) {
        return;
      }
      settled = true;
      clearInterval(timer);
      resolvePromise({ code, stopReason, stderr: error ? `${stderr}\n${error}`.trim() : stderr });
    };

    timer = setInterval(async () => {
      if (probing || stopReason) {
        return;
      }
      probing = true;
      try {
        const now = Date.now();
        const bytes = await directorySize(destination);
        if (bytes > written) {
          written = bytes;
          grownAt = now;
        }
        const seconds = Math.max(1, (now - startedAt) / 1000);
        info(
          `    解压 ${basename(archiveName)}：已写入 ${(bytes / 1048576).toFixed(1)} MB · 平均 ${(bytes / 1024 / seconds).toFixed(0)} KB/s`,
        );
        if (now - grownAt >= EXTRACT_STALL_MS) {
          stopReason = 'stalled';
        } else if (now - startedAt >= EXTRACT_TIMEOUT_MS) {
          stopReason = 'timeout';
        }
      } finally {
        probing = false;
      }
      if (stopReason) {
        warn(
          `解压${stopReason === 'stalled' ? '卡住：目标目录连续 4 分钟没有增长' : '超时'}，终止 ${basename(tar)}`,
        );
        killProcessTree(child);
        // taskkill 失败时别无限等 close 事件。
        setTimeout(() => settle(1), 15000);
      }
    }, EXTRACT_PROGRESS_MS);
    timer.unref?.();

    child.on('close', (code) => settle(code ?? 1));
    child.on('error', (error) => settle(1, error.message));
  });
}

async function extractTarBz2(archiveName, destinationName, cwd) {
  const destination = join(cwd, destinationName);
  const failures = [];
  for (const tar of await resolveTarCandidates()) {
    info(`解压 ${archiveName}（${basename(tar)}）`);
    const result = await extractWithTar(tar, archiveName, destinationName, cwd, destination);
    if (result.code === 0) {
      info(`解压完成 ${archiveName}：${((await directorySize(destination)) / 1048576).toFixed(1)} MB`);
      return;
    }
    const reason = result.stopReason ? `被看门狗终止（${result.stopReason}）` : `退出码 ${result.code}`;
    const tail = result.stderr
      .trim()
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .slice(-4)
      .join(' | ');
    warn(`解压失败（${basename(tar)} · ${reason}）：${tail || '无输出'}`);
    failures.push(`${basename(tar)}: ${reason}${tail ? ` · ${tail}` : ''}`);
    await rm(destination, { recursive: true, force: true });
    await mkdir(destination, { recursive: true });
  }
  throw new Error(`解压失败：${archiveName}\n${failures.join('\n')}`);
}

/// 当前平台对应的静态库归档名。
///
/// 必须与 `sherpa-onnx-sys` 的 build.rs 里写死的名字逐字一致，
/// 否则 `SHERPA_ONNX_ARCHIVE_DIR` 里找不到它，构建会退回去联网下载（多半会卡住）。
function nativeArchiveName() {
  const arm = process.arch === 'arm64';
  if (process.platform === 'win32') {
    return arm
      ? `sherpa-onnx-v${VERSION}-win-arm64-static-MT-Release-lib.tar.bz2`
      : `sherpa-onnx-v${VERSION}-win-x64-static-MT-Release-lib.tar.bz2`;
  }
  if (process.platform === 'darwin') {
    return arm
      ? `sherpa-onnx-v${VERSION}-osx-arm64-static-lib.tar.bz2`
      : `sherpa-onnx-v${VERSION}-osx-x64-static-lib.tar.bz2`;
  }
  return arm
    ? `sherpa-onnx-v${VERSION}-linux-aarch64-static-lib.tar.bz2`
    : `sherpa-onnx-v${VERSION}-linux-x64-static-lib.tar.bz2`;
}

/// 取归档里的第一个子目录路径（模型包都解出一层同名目录）。
async function firstSubdir(parent) {
  const entries = await readdir(parent, { withFileTypes: true });
  const found = entries.find((entry) => entry.isDirectory());
  return found ? join(parent, found.name) : null;
}

/// 找到 MSVC 的 dumpbin/lib。
///
/// GPU 路径需要它们把 onnxruntime.dll 转成导入库——官方 CUDA 包只给了 DLL 和
/// sherpa-onnx 自己的导入库，唯独缺 onnxruntime.lib，而 crate 的动态链接模式明确要求它。
async function findMsvcBinutils() {
  const vswhere = join(
    process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)',
    'Microsoft Visual Studio',
    'Installer',
    'vswhere.exe',
  );
  if (!existsSync(vswhere)) {
    return null;
  }
  const { code, stdout } = await capture(vswhere, [
    '-latest',
    '-products',
    '*',
    '-property',
    'installationPath',
  ]);
  if (code !== 0) {
    return null;
  }
  const root = stdout.trim().split(/\r?\n/)[0]?.trim();
  if (!root) {
    return null;
  }
  const toolsDir = join(root, 'VC', 'Tools', 'MSVC');
  if (!existsSync(toolsDir)) {
    return null;
  }
  const versions = (await readdir(toolsDir)).sort().reverse();
  for (const version of versions) {
    const bin = join(toolsDir, version, 'bin', 'Hostx64', 'x64');
    const dumpbin = join(bin, 'dumpbin.exe');
    const lib = join(bin, 'lib.exe');
    if (existsSync(dumpbin) && existsSync(lib)) {
      return { dumpbin, lib };
    }
  }
  return null;
}

/// 从 onnxruntime.dll 导出表生成 onnxruntime.lib。
///
/// 不这么做就只有两条路：自己编译带 GPU 的 sherpa-onnx（要 CUDA toolkit + cuDNN + 长构建），
/// 或者放弃 GPU。实测这一步能跑通，且产出的链接结果可用。
async function generateOnnxRuntimeImportLib(binutils, dllPath, outLibPath, workDir) {
  const exportsTxt = join(workDir, 'onnxruntime_exports.txt');
  const defFile = join(workDir, 'onnxruntime.def');

  // 直接抓 dumpbin 的 stdout 再由 Node 落盘：绕开 `cmd /s /c` 对
  // 「带空格且以引号开头的命令」那套引号剥离规则，少一类只在 Windows 上翻车的问题。
  const dumpResult = await capture(binutils.dumpbin, ['/nologo', '/exports', dllPath]);
  if (dumpResult.code !== 0 || !dumpResult.stdout.trim()) {
    throw new Error('dumpbin 解析 onnxruntime.dll 导出表失败');
  }
  await writeFile(exportsTxt, dumpResult.stdout);
  const text = dumpResult.stdout;
  const names = [];
  for (const line of text.split(/\r?\n/)) {
    // 形如：  1    0 000587D0 OrtGetApiBase
    const match = line.match(/^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]+\s+(\S+)\s*$/);
    if (match) {
      names.push(match[1]);
    }
  }
  if (names.length === 0) {
    throw new Error('未从 onnxruntime.dll 解析出任何导出符号');
  }
  await writeFile(defFile, `LIBRARY onnxruntime.dll\nEXPORTS\n${names.map((n) => `    ${n}`).join('\n')}\n`);

  const result = await run(
    binutils.lib,
    [`/def:${defFile}`, '/machine:x64', `/out:${outLibPath}`],
    { stdio: 'ignore' },
  );
  if (result.code !== 0) {
    throw new Error(`lib.exe 生成导入库失败：${result.stderr.trim()}`);
  }
  return names.length;
}

// ── CUDA 运行时（GPU 构建必需）────────────────────────────────────────────
//
// onnxruntime 那个 CUDA EP 是按 CUDA 12.9 的符号集链接的，光有 onnxruntime*.dll 不够：
// 少一个运行库就会 LoadLibrary 失败（error 126 / 127），或者在 EP 自己的初始化里崩掉
// （error 1114），最终表现成「机器明明有 GPU 却在跑 CPU」，严重时直接 abort 进程。
// 也不能指望构建机上装没装 CUDA Toolkit——一律取 NVIDIA 官方在 PyPI 上的
// redistributable，版本对齐上面那个 cuda-12.x-cudnn-9.x-ORT 包：
//   · CUDA 12.9 是因为 EP 要的 cudaLibrary* 系列符号到 12.8 才有；
//   · cuDNN 只认 9.x（EP 里写死 CUDNN version < 9.0 就报错）。
//
// 这些 DLL 会被 sherpa-onnx-sys 的 build.rs 一并拷到 exe 旁，release 打包再按 resources
// 收进安装包，所以 GPU 版分发包会因此大出 1GB 以上；CPU 构建不会走到这里。
const CUDA_SERIES = '12.9';
const CUDA_RUNTIME_STAMP = `cuda-runtime-v1:${CUDA_SERIES}`;
// 每项的 series 是该**包自己的**版本前缀，不是 CUDA 版本——各家编号不统一：
//   cudart/nvrtc 跟着 CUDA 版本（12.9.x），cuBLAS 也是（12.9.x），
//   而 cuFFT 用的是库版本（11.x），cuDNN 用 9.x。包的 cu12 后缀已经限定了 CUDA 大版本。
const CUDA_RUNTIME_WHEELS = [
  {
    pkg: 'nvidia-cuda-runtime-cu12',
    series: CUDA_SERIES,
    bin: 'nvidia/cuda_runtime/bin',
    match: /^cudart64_\d+\.dll$/i,
    what: 'CUDA 运行时 cudart',
  },
  {
    pkg: 'nvidia-cublas-cu12',
    series: CUDA_SERIES,
    bin: 'nvidia/cublas/bin',
    match: /^cublas(Lt)?64_\d+\.dll$/i,
    what: 'cuBLAS / cuBLASLt',
  },
  {
    pkg: 'nvidia-cufft-cu12',
    series: '11.',
    bin: 'nvidia/cufft/bin',
    match: /^cufft64_\d+\.dll$/i,
    what: 'cuFFT',
  },
  {
    pkg: 'nvidia-cuda-nvrtc-cu12',
    series: CUDA_SERIES,
    bin: 'nvidia/cuda_nvrtc/bin',
    match: /^nvrtc64_\d+_\d+\.dll$/i,
    what: 'NVRTC',
  },
  {
    pkg: 'nvidia-cudnn-cu12',
    series: '9.',
    bin: 'nvidia/cudnn/bin',
    match: /^cudnn[A-Za-z_]*64_9.*\.dll$/i,
    what: 'cuDNN 9',
  },
];

function proxyArgs(proxy) {
  return proxy ? ['--proxy', proxy] : [];
}

/// 版本号按数字段比较：`12.9.79` 要大于 `12.9.2.10`，按字符串排会判反。
function compareVersions(a, b) {
  const pa = a.split('.').map((part) => Number.parseInt(part, 10) || 0);
  const pb = b.split('.').map((part) => Number.parseInt(part, 10) || 0);
  for (let index = 0; index < Math.max(pa.length, pb.length); index += 1) {
    const diff = (pa[index] || 0) - (pb[index] || 0);
    if (diff !== 0) {
      return diff;
    }
  }
  return 0;
}

/// PyPI 索引候选：先镜像、后官方源。
///
/// 国内直连 pypi.org 经常直接断开（实测连 TLS 握手都过不去），而 python 运行时的
/// pip.ini 本来就用清华源，这里保持一致；能直连的机器会自然落到官方源。
function pypiIndexCandidates() {
  const override = process.env.GUAXI_PYPI_INDEX;
  return [
    ...(override ? [override.replace(/\/+$/, '')] : []),
    'https://pypi.tuna.tsinghua.edu.cn',
    'https://pypi.org',
  ];
}

/// 从 wheel 文件名里取版本号：`{发行版}-{版本}-{pytag}-{abitag}-{平台}.whl`。
function versionFromWheel(filename, pkg) {
  const normalized = pkg.replace(/[-_.]+/g, '_').toLowerCase();
  const stem = filename.replace(/\.whl$/i, '');
  const prefix = `${normalized}-`;
  if (!stem.toLowerCase().startsWith(prefix)) {
    return null;
  }
  return stem.slice(prefix.length).split('-')[0];
}

/// 挑一个 wheel：该包最新一个匹配版本前缀、且带 win_amd64 轮子的发行版。
///
/// 认的是 PEP 503 简单索引页，而不是 PyPI 的 JSON API：镜像的 JSON 经常滞后
/// （实测清华源的 /pypi/nvidia-cuda-runtime-cu12/json 停在 12.6.37，同一个源的
/// simple 索引里已经有 12.9.79），照 JSON 找会找不到版本、白白退回直连官方源。
async function resolveWheelUrl(pkg, series, proxy) {
  const attempts = [];
  for (const index of pypiIndexCandidates()) {
    const pageUrl = `${index}/simple/${pkg}/`;
    const { code, stdout } = await capture('curl', ['-fsSL', ...proxyArgs(proxy), pageUrl]);
    if (code !== 0 || !stdout.trim()) {
      attempts.push(`${index}：curl 退出码 ${code}`);
      continue;
    }

    const candidates = [];
    for (const match of stdout.matchAll(/<a[^>]+href="([^"#]+)[^"]*"[^>]*>([^<]+)<\/a>/gi)) {
      const href = match[1].replace(/&amp;/g, '&');
      const filename = match[2].trim();
      if (!filename.toLowerCase().endsWith('.whl') || !filename.includes('win_amd64')) {
        continue;
      }
      const version = versionFromWheel(filename, pkg);
      if (version && version.startsWith(series)) {
        candidates.push({ version, href, filename });
      }
    }

    if (candidates.length === 0) {
      attempts.push(`${index}：索引页里没有 ${series}* 的 win_amd64 轮子`);
      continue;
    }

    candidates.sort((a, b) => compareVersions(a.version, b.version));
    const best = candidates[candidates.length - 1];
    return {
      version: best.version,
      // 索引页里的 href 通常是 `../../packages/...` 这种相对路径，按页面地址解析。
      url: new URL(best.href, pageUrl).href,
      filename: best.filename,
      index,
    };
  }
  throw new Error(`找不到 ${pkg} ${series}* 的 win_amd64 轮子：\n    ${attempts.join('\n    ')}`);
}

/// 把 CUDA 运行时的 DLL 放进 cuda-lib 目录。
///
/// 放这一处就够了：sherpa-onnx-sys 的 build.rs 在 shared（gpu）模式下会把该目录下
/// 所有 DLL 拷到 exe 旁，dev 与 release 一并覆盖。
async function stageCudaRuntimeDlls(libDir, proxy) {
  // wheel 缓存在这里不删：全套 1.5GB 上下，重下一次代价太高，
  // 断点续传和「只缺一个组件时重跑」都要靠它。解压产物每轮清掉。
  const work = join(LIBS_ROOT, 'cuda-runtime-tmp');
  await mkdir(work, { recursive: true });

  let totalBytes = 0;
  for (const component of CUDA_RUNTIME_WHEELS) {
    const wheel = await resolveWheelUrl(component.pkg, component.series, proxy);
    const archive = join(work, wheel.filename);
    if (force || !existsSync(archive)) {
      info(`下载 ${component.what}：${wheel.filename}（来自 ${wheel.index}）`);
      await download(wheel.url, archive, proxy);
    } else {
      info(`复用已下载的 ${wheel.filename}`);
    }

    const unpackedName = `${wheel.filename}.unpacked`;
    const unpacked = join(work, unpackedName);
    await rm(unpacked, { recursive: true, force: true });
    await mkdir(unpacked, { recursive: true });
    // wheel 就是个 zip。用相对名 + cwd 传给解压工具，避开 GNU tar 把盘符当主机名的坑。
    await extractWheel(wheel.filename, unpackedName, work);

    const binDir = join(unpacked, ...component.bin.split('/'));
    if (!existsSync(binDir)) {
      throw new Error(`${wheel.filename} 里没有 ${component.bin} 目录`);
    }
    const picked = (await readdir(binDir)).filter((name) => component.match.test(name)).sort();
    if (picked.length === 0) {
      throw new Error(`${wheel.filename} 的 ${component.bin} 里没有匹配 ${component.match} 的 DLL`);
    }

    for (const name of picked) {
      const target = join(libDir, name);
      await copyFile(join(binDir, name), target);
      totalBytes += await fileSize(target);
    }
    info(`  ${component.what}：放入 ${picked.length} 个（${picked.join(', ')}）`);

    // 每轮用完就清掉解压产物，只留 wheel（见函数开头对缓存的说明）。
    await rm(unpacked, { recursive: true, force: true });
  }

  info(`CUDA 运行时已就绪，合计 ${(totalBytes / 1024 / 1024).toFixed(0)} MB`);
}

/// 解压 wheel（wheel 本身就是个 zip）。
///
/// 不能直接调 `tar`：从 Git Bash 里起到的 `tar` 是 GNU tar，它不认 zip
/// （实测报 "This does not look like a tar archive"）；Windows 自带的 bsdtar 在
/// System32 下，用绝对路径调它最稳。其它平台退回 unzip / GNU tar。
async function extractWheel(archiveName, destinationName, cwd) {
  const candidates = [];
  if (process.platform === 'win32') {
    const systemTar = join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe');
    if (existsSync(systemTar)) {
      candidates.push([systemTar, ['-xf', archiveName, '-C', destinationName]]);
    }
  }
  candidates.push(['unzip', ['-oq', archiveName, '-d', destinationName]]);
  candidates.push(['tar', ['-xf', archiveName, '-C', destinationName]]);

  const failures = [];
  for (const [command, args] of candidates) {
    const result = await run(command, args, { cwd });
    if (result.code === 0) {
      return;
    }
    failures.push(`${command}：${result.stderr.trim() || `退出码 ${result.code}`}`);
  }
  throw new Error(`解压 wheel 失败：${archiveName}\n    ${failures.join('\n    ')}`);
}

/// 下载并装配 GPU 版原生库目录。
///
/// 产出的目录同时放导入库与 DLL：前者供链接，后者会被 sherpa-onnx-sys 的 build.rs
/// 自动拷到 exe 旁，省得再手工分发。
async function prepareCudaLibs(proxy) {
  if (process.platform !== 'win32') {
    warn(
      'GPU 预编译包目前只处理 Windows。其它平台需要自行构建带 CUDA 的 sherpa-onnx，' +
        '再用 SHERPA_ONNX_LIB_DIR 指向它的 lib 目录。',
    );
    return;
  }

  const name = `sherpa-onnx-v${VERSION}-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-win-x64-cuda.tar.bz2`;
  const libDir = join(LIBS_ROOT, 'cuda-lib');
  const marker = join(libDir, '.prepared');
  if (existsSync(marker) && !force) {
    // 标记里带上「装配了什么」的口径。只看归档名（旧标记）会让后来新增的 CUDA 运行时
    // 在已经跑过一次的构建机上永远补不进去——那台机器就会一直产不出能用的 GPU 版。
    const stamped = (await readFile(marker, 'utf8')).includes(CUDA_RUNTIME_STAMP);
    if (stamped) {
      info(`CUDA 库目录已就绪，跳过：${libDir}`);
      return;
    }
    info(`CUDA 库目录缺少运行时标记（${CUDA_RUNTIME_STAMP}），重新装配…`);
  }

  const binutils = await findMsvcBinutils();
  if (!binutils) {
    warn('未找到 MSVC 的 dumpbin/lib（需要 VS 的 C++ 工具链），无法生成 onnxruntime.lib，跳过 GPU 准备。');
    return;
  }

  // 原生库那部分（解压归档、拷 DLL/导入库）很贵——595MB 的 tar.bz2 解一次要几分钟。
  // 已经装配好就跳过，只补 CUDA 运行时，这样「上次只差运行时没补齐」的重跑是秒级的。
  const nativeReady =
    existsSync(join(libDir, 'onnxruntime_providers_cuda.dll')) &&
    existsSync(join(libDir, 'onnxruntime.lib'));

  if (nativeReady && !force) {
    info(`原生库已装配，跳过解压，只补 CUDA 运行时：${libDir}`);
  } else {
    const archive = join(LIBS_ROOT, name);
    if (force || !existsSync(archive)) {
      info(`下载 CUDA 版原生库（约 595MB）：${name}`);
      await download(`${RELEASE}/${name}`, archive, proxy);
    } else {
      info(`复用已下载的 ${name}`);
    }

    const work = join(LIBS_ROOT, `${name}.extracted`);
    await rm(work, { recursive: true, force: true });
    await mkdir(work, { recursive: true });
    await extractTarBz2(name, `${name}.extracted`, LIBS_ROOT);

    const root = join(work, name.replace(/\.tar\.bz2$/i, ''));
    const srcLib = join(root, 'lib');
    if (!existsSync(srcLib)) {
      throw new Error(`CUDA 包里没有 lib 目录：${srcLib}`);
    }

    await rm(libDir, { recursive: true, force: true });
    await mkdir(libDir, { recursive: true });

    // DLL 与导入库都要：DLL 会被 build.rs 拷到 exe 旁。
    for (const entry of await readdir(srcLib)) {
      if (/\.(dll|lib)$/i.test(entry)) {
        await copyFile(join(srcLib, entry), join(libDir, entry));
      }
    }

    const count = await generateOnnxRuntimeImportLib(
      binutils,
      join(srcLib, 'onnxruntime.dll'),
      join(libDir, 'onnxruntime.lib'),
      work,
    );
    info(`已生成 onnxruntime.lib（${count} 个导出符号）`);
    await rm(work, { recursive: true, force: true });
  }

  // 归档里只有 sherpa/onnxruntime 自己的 DLL，CUDA 运行时（cudart/cublas/cudnn…）
  // 得另外补进来，否则 GPU 版装到机器上 EP 库根本加载不起来。
  await stageCudaRuntimeDlls(libDir, proxy);

  await writeFile(marker, `${name}\n${CUDA_RUNTIME_STAMP}\n`);
  info(`GPU 库目录就绪：${libDir}`);
  info('用 GPU 构建：npm run tauri -- build -- --no-default-features --features gpu（会自己带上 SHERPA_ONNX_LIB_DIR 与打包配置）');
}

/// 阶段一：把缺失的归档下载到 work。
async function prefetchArchive(task, work, proxy) {
  const archive = join(work, task.name);

  // 归档还在就直接复用：模型动辄上 G，重下代价太高。
  if (force || !existsSync(archive)) {
    info(`下载 ${task.name} …`);
    await download(task.url, archive, proxy);
  } else {
    info(`复用已下载的 ${task.name}`);
  }
}

/// 阶段二：把归档解到目标位置。
async function installTask(task, work) {
  const archive = join(work, task.name);

  await rm(task.target, { recursive: true, force: true });
  await mkdir(dirname(task.target), { recursive: true });

  if (task.extract === false || task.unpack === 'raw') {
    // 原生库归档保持原样放置（由 build.rs 解压）；VAD 本身就是单个文件。
    await rename(archive, task.target);
  } else {
    const stagingName = `${task.name}.extracted`;
    const staging = join(work, stagingName);
    await rm(staging, { recursive: true, force: true });
    await mkdir(staging, { recursive: true });
    await extractTarBz2(task.name, stagingName, work);

    const stem = task.name.replace(/\.tar\.bz2$/i, '');
    const nested = join(staging, stem);
    const source = existsSync(nested) ? nested : (await firstSubdir(staging)) || staging;
    await rename(source, task.target);
    await rm(staging, { recursive: true, force: true });
    await rm(archive, { force: true });
  }
  info(`完成：${task.target}`);
}

async function main() {
  const proxy = await resolveSystemProxy();
  info(proxy ? `使用代理：${proxy}` : '未检测到代理，直连下载');

  const tasks = [];

  // 1) 原生静态库。文件名随平台而异（见 nativeArchiveName），
  //    必须与 sherpa-onnx-sys 的 build.rs 期望的一致。
  const nativeName = nativeArchiveName();
  tasks.push({
    name: nativeName,
    url: `${RELEASE}/${nativeName}`,
    target: join(LIBS_ROOT, nativeName),
    ready: async () => (await fileSize(join(LIBS_ROOT, nativeName))) > 50 * 1024 * 1024,
    // 原生库只是把归档放到目录里，构建时由 build.rs 自己解压，这里不展开。
    extract: false,
  });

  // 2) TTS：kokoro 多语模型。
  tasks.push({
    name: 'kokoro-multi-lang-v1_1.tar.bz2',
    url: `${TTS_MODELS}/kokoro-multi-lang-v1_1.tar.bz2`,
    target: join(MODELS_ROOT, 'tts', 'kokoro_multi_lang'),
    ready: async () => (await fileSize(join(MODELS_ROOT, 'tts', 'kokoro_multi_lang', 'model.onnx'))) > 50 * 1024 * 1024,
  });

  // 3) ASR：SenseVoice 多语模型（默认配置指向这个目录）。
  tasks.push({
    name: 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2',
    url: `${ASR_MODELS}/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2`,
    target: join(MODELS_ROOT, 'asr', 'sense-voice'),
    ready: async () => (await fileSize(join(MODELS_ROOT, 'asr', 'sense-voice', 'tokens.txt'))) > 1024,
  });

  // 4) 语言学习：whisper ONNX（时间轴由 VAD 提供，模型本身不需要 attention 输出）。
  //    目标保留归档里的同名子目录，Rust 侧是按「目录下含 -tokens.txt」来定位的。
  tasks.push({
    name: 'sherpa-onnx-whisper-base.en.tar.bz2',
    url: `${ASR_MODELS}/sherpa-onnx-whisper-base.en.tar.bz2`,
    target: join(MODELS_ROOT, 'whisper_onnx', 'sherpa-onnx-whisper-base.en'),
    ready: async () =>
      (await fileSize(join(MODELS_ROOT, 'whisper_onnx', 'sherpa-onnx-whisper-base.en', 'base.en-tokens.txt'))) > 1024,
  });

  // 5) VAD：切分语音段用，模型很小。
  tasks.push({
    name: 'silero_vad.onnx',
    url: `${ASR_MODELS}/silero_vad.onnx`,
    target: join(MODELS_ROOT, 'vad', 'silero_vad.onnx'),
    unpack: 'raw',
    ready: async () => (await fileSize(join(MODELS_ROOT, 'vad', 'silero_vad.onnx'))) > 100 * 1024,
  });

  // 6) 标点模型：识别模型本身不输出标点，实时听写要靠它补断句。
  //    中英混排的 CT-Transformer，约 70MB。
  tasks.push({
    name: 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12.tar.bz2',
    url: `${PUNCT_MODELS}/sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12.tar.bz2`,
    target: join(MODELS_ROOT, 'punctuation', 'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12'),
    ready: async () =>
      (await fileSize(
        join(
          MODELS_ROOT,
          'punctuation',
          'sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12',
          'model.onnx',
        ),
      )) > 10 * 1024 * 1024,
  });

  if (skipModels) {
    info('SHERPA_SKIP_MODELS 已启用：仅准备原生库，跳过 TTS/ASR/标点模型');
    tasks.splice(1);
  }

  const work = join(SRC_TAURI, '.sherpa-download-tmp');
  await mkdir(work, { recursive: true });

  const pending = [];
  for (const task of tasks) {
    if (!force && (await task.ready())) {
      info(`已存在，跳过：${task.target}`);
      continue;
    }
    pending.push(task);
  }

  // 下载并发、解压串行：网络能靠多条连接叠带宽，而 bz2 解压本身是单线程，
  // 三个大包同时解只会互相抢 CPU 和磁盘，日志里也看不出卡在哪个包。
  let nextTaskIndex = 0;
  const workers = Array.from(
    { length: Math.min(DOWNLOAD_CONCURRENCY, pending.length) },
    async () => {
      while (true) {
        const index = nextTaskIndex;
        nextTaskIndex += 1;
        if (index >= pending.length) {
          return;
        }
        await prefetchArchive(pending[index], work, proxy);
      }
    },
  );
  await Promise.all(workers);

  for (const task of pending) {
    await installTask(task, work);
  }

  if (withCuda) {
    await prepareCudaLibs(proxy);
  }

  info('全部就绪。');
}

main().catch((error) => {
  warn(error?.message || String(error));
  process.exit(1);
});
