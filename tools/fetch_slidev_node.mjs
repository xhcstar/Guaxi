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
import { copyFile, mkdir, readdir, rm, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const NODE_VERSION = process.env.GUAXI_NODE_VERSION || 'v22.11.0';
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
    console.log(`[slidev-node] node.exe 已存在（${(existing / 1048576).toFixed(1)} MB），跳过`);
    return;
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
  // Windows 自带 bsdtar 也能解 zip，但 Expand-Archive 在各版 PowerShell 上都可用，
  // 路径里出现空格时也不用额外转义。
  const extractCode = await run('powershell', [
    '-NoProfile',
    '-NonInteractive',
    '-Command',
    `Expand-Archive -LiteralPath '${archive.replaceAll("'", "''")}' -DestinationPath '${TEMP_DIR.replaceAll("'", "''")}' -Force`,
  ]);
  if (extractCode !== 0) {
    throw new Error(`解压失败（退出码 ${extractCode}）`);
  }

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
