#!/usr/bin/env node
// 准备 `src-tauri/utils/SlidevEnv/node.exe`。
//
// 为什么需要它：
//   · Slidev / LSP / RSSHub 运行时不依赖用户机器上的 Node，只认软件内置的
//     `utils/SlidevEnv/node.exe`（见 src-tauri/src/commands/slidev.rs 与 rsshub_server.rs）。
//   · 这个二进制太大、又是平台相关产物，不适合进 git；开发机通常手动放过一份，
//     所以 CI 与干净机器上一直是缺的——打出来的离线包里有 rsshub/dist 却起不来。
//   · 这里按固定版本从 nodejs.org 取官方 win-x64 包，只留 node.exe 与许可证。
//
// 用法：
//   node tools/fetch_slidev_node.mjs          # 缺失才下载
//   node tools/fetch_slidev_node.mjs --force  # 重新下载
//
// 非 Windows 平台直接跳过：当前的 Slidev/RSSHub 运行路径只针对 Windows 适配。

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { copyFile, mkdir, readdir, rm, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// 版本必须跟着 RSSHub 的 engines 走（当前 ^24.15.0 || ^26.13.0）：v22.11 的 zlib 里
// 没有 createZstdDecompress，RSSHub 依赖的 undici 8 一启动就崩。用 24.x LTS。
const NODE_VERSION = process.env.GUAXI_NODE_VERSION || 'v24.22.3';
const DIST_BASE = process.env.GUAXI_NODE_DIST || 'https://nodejs.org/dist';
const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
const SLIDEV_DIR = join(ROOT, 'src-tauri', 'utils', 'SlidevEnv');
const TEMP_DIR = join(SLIDEV_DIR, '.node-download');
// node.exe 正常有 80 MB 上下；明显偏小说明下载/解压成了半截文件。
const MIN_NODE_BYTES = 40 * 1024 * 1024;
const force = process.argv.includes('--force');

async function sizeOf(path) {
  try {
    return (await stat(path)).size;
  } catch {
    return 0;
  }
}

function run(command, args, options = {}) {
  return new Promise((resolvePromise) => {
    const child = spawn(command, args, { stdio: 'inherit', ...options });
    child.on('close', (code) => resolvePromise(code ?? 1));
    child.on('error', () => resolvePromise(1));
  });
}

function capture(command, args) {
  return new Promise((resolvePromise) => {
    let out = '';
    const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'ignore'] });
    child.stdout.on('data', (chunk) => (out += chunk));
    child.on('close', (code) => resolvePromise({ code: code ?? 1, out: out.trim() }));
    child.on('error', () => resolvePromise({ code: 1, out: '' }));
  });
}

// 解 zip 用 Windows 自带的 bsdtar：它不会踩 PowerShell 的坑。
// 之前在 CI 上 `Expand-Archive -Force` 直接失败——PS7 的 Archive 模块去调
// ExtractToDirectory 的三参数重载，报 "Cannot find an overload ... argument count: 3"，
// 结果 node.exe 根本没解出来，而编排层又把这一步当成成功，离线包里的 RSSHub 起不来。
// PowerShell 只作为退路，且不带 -Force（三参数重载正是它炸掉的地方，目标目录是空的）。
async function extractZip(archive, destination) {
  const candidates = [];
  const systemTar = join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'tar.exe');
  if (existsSync(systemTar)) {
    candidates.push([systemTar, ['-xf', archive, '-C', destination]]);
  }
  candidates.push([
    'powershell',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      `Expand-Archive -LiteralPath '${archive.replaceAll("'", "''")}' -DestinationPath '${destination.replaceAll("'", "''")}'`,
    ],
  ]);

  const failures = [];
  for (const [command, args] of candidates) {
    const code = await run(command, args);
    if (code === 0) {
      return;
    }
    failures.push(`${command}: 退出码 ${code}`);
  }
  throw new Error(`解压失败（${failures.join('；')}）`);
}

async function findFile(root, name) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      const found = await findFile(path, name);
      if (found) return found;
    } else if (entry.name === name) {
      return path;
    }
  }
  return null;
}

async function main() {
  if (process.platform !== 'win32') {
    console.log(`[slidev-node] 跳过：当前平台 ${process.platform} 不使用内置 Node 运行时`);
    return;
  }

  await mkdir(SLIDEV_DIR, { recursive: true });
  const destination = join(SLIDEV_DIR, 'node.exe');

  const existing = await sizeOf(destination);
  if (!force && existing >= MIN_NODE_BYTES) {
    // 只比大小会漏掉版本升级：老的 node.exe 同样「看起来正常」，但跑不了新版 RSSHub。
    const { out: currentVersion } = await capture(destination, ['--version']);
    if (currentVersion === NODE_VERSION) {
      console.log(`[slidev-node] node.exe ${currentVersion} 已就绪，跳过`);
      return;
    }
    console.log(
      `[slidev-node] 现有 node.exe 是 ${currentVersion || '未知版本'}，需要 ${NODE_VERSION}，重新下载`,
    );
  }

  const archiveName = `node-${NODE_VERSION}-win-x64.zip`;
  const url = `${DIST_BASE}/${NODE_VERSION}/${archiveName}`;
  const archive = join(TEMP_DIR, archiveName);

  await rm(TEMP_DIR, { recursive: true, force: true });
  await mkdir(TEMP_DIR, { recursive: true });

  console.log(`[slidev-node] 下载 ${url}`);
  const downloadCode = await run('curl', [
    '-fL',
    '--retry', '3',
    '--connect-timeout', '30',
    '--speed-time', '60',
    '--speed-limit', '1024',
    '-o', archive,
    url,
  ]);
  if (downloadCode !== 0) {
    throw new Error(`curl 下载失败（退出码 ${downloadCode}）`);
  }

  const archiveSize = await sizeOf(archive);
  if (archiveSize < 10 * 1024 * 1024) {
    throw new Error(`下载到的压缩包大小异常（${archiveSize} 字节）`);
  }

  console.log(`[slidev-node] 解压 ${archiveName}`);
  await extractZip(archive, TEMP_DIR);

  const extractedNode = await findFile(TEMP_DIR, 'node.exe');
  if (!extractedNode) {
    throw new Error(`${archiveName} 里没有 node.exe`);
  }
  const extractedSize = await sizeOf(extractedNode);
  if (extractedSize < MIN_NODE_BYTES) {
    throw new Error(`解压出的 node.exe 大小异常（${extractedSize} 字节）`);
  }

  await copyFile(extractedNode, destination);

  // Node.js 以 MIT 许可分发，随包分发时需要保留许可证文本。
  const license = await findFile(TEMP_DIR, 'LICENSE');
  if (license) {
    await copyFile(license, join(SLIDEV_DIR, 'NodeJS-LICENSE.txt'));
  }

  await rm(TEMP_DIR, { recursive: true, force: true });
  console.log(`[slidev-node] 就绪：${destination}（${(extractedSize / 1048576).toFixed(1)} MB）`);
}

main().catch((error) => {
  console.error(`[slidev-node] ${error.message}`);
  process.exit(1);
});
