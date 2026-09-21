// 构建期准备「现代 RSSHub」运行环境（进程内集成，不用 Docker，不做首次下载）。
//
// 为什么不是 `npm install rsshub`
// - npm 上的 `rsshub` 是 2018 年的遗留版(1.0.0)，路由早已失效，抓不到现代内容。
// - 官方现代 RSSHub 只以 git 源码形式维护，需 clone + pnpm install + pnpm build 才能运行。
//
// 本脚本做的事（可在任意机器复现同一份 RSSHub）
// 1. 把官方仓库浅克隆到 `src-tauri/utils/rsshub`，检出**固定 git ref**（RSSHUB_GIT_REF）。
// 2. 用 pnpm 装依赖（--frozen-lockfile，锁定依赖树）并 `pnpm build` 产出 `dist/index.mjs`。
// 3. 写一份 `.rsshub-build.json` 记录实际构建的 commit，便于追溯/复现。
// 运行时由 `rsshub_server.rs` 用 SlidevEnv 的 node.exe 启动 `utils/rsshub/dist/index.mjs`，
// 共用那一份 node，不额外打包 Node 运行时。
//
// 复现性
// - RSSHUB_GIT_REF 固定源码版本；仓库自带的 pnpm-lock.yaml + --frozen-lockfile 固定依赖树。
//   两者共同保证不同机器构建出一致的 RSSHub。要升级就改 RSSHUB_GIT_REF 后重跑（--force）。
//
// 容错
// - 拿不到网络/构建失败**只告警不失败**：RSSHub 缺失时聚合功能降级（其余抓取照常），
//   不因为它阻断 dev / build（与 fetch_codex_sidecar.mjs 同样策略）。
//
// 用法：
//   node ./tools/prepare_rsshub_env.mjs           # 已构建好则跳过
//   node ./tools/prepare_rsshub_env.mjs --force   # 强制重新 clone + build
//   RSSHUB_GIT_REF=<commit|tag|branch> node ./tools/prepare_rsshub_env.mjs  # 指定版本
//   RSSHUB_GIT_URL=<mirror> ...                    # 指定镜像（GitHub 慢时用）

import { spawn } from 'node:child_process';
import path from 'node:path';
import { stat, rm, mkdir, writeFile } from 'node:fs/promises';

const projectRoot = process.cwd();
const utilsDir = path.join(projectRoot, 'src-tauri', 'utils');
const rsshubDir = path.join(utilsDir, 'rsshub');

// 固定源码版本：默认 master（首次构建以当时最新为准，并把 commit 记进 marker）。
// 要跨机器**逐字节复现**同一份，请显式设置成某个 commit / tag。
const RSSHUB_GIT_REF = process.env.RSSHUB_GIT_REF || 'master';
const RSSHUB_GIT_URL = process.env.RSSHUB_GIT_URL || 'https://github.com/DIYgod/RSSHub.git';
// 固定 pnpm 大版本（读取 pnpm-lock 需匹配）。corepack 坏了就用 npx。
const PNPM_SPEC = process.env.RSSHUB_PNPM_SPEC || 'pnpm@10';

// clone + install + build 总超时（依赖多、构建慢，给足）。
const STEP_TIMEOUT_MS = 30 * 60 * 1000;
const force = process.argv.includes('--force');

function info(msg) { console.log(`[rsshub.prepare] ${msg}`); }
function warn(msg) { console.warn(`[rsshub.prepare] ${msg}`); }

async function exists(target) {
  try { await stat(target); return true; } catch { return false; }
}

function run(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: options.cwd || projectRoot,
      stdio: 'inherit',
      shell: process.platform === 'win32', // Windows 上 git/npx 常是 .cmd，需经 shell
      env: { ...process.env, ...(options.env || {}) },
    });
    const timer = setTimeout(() => {
      warn(`command timed out after ${STEP_TIMEOUT_MS}ms, killing: ${command} ${args.join(' ')}`);
      child.kill('SIGKILL');
    }, STEP_TIMEOUT_MS);
    child.on('error', (err) => { clearTimeout(timer); warn(`spawn failed: ${err.message}`); resolve(1); });
    child.on('close', (code) => { clearTimeout(timer); resolve(code ?? 1); });
  });
}

// 捕获 stdout（用于读取 commit hash）。
function runCapture(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: options.cwd || projectRoot,
      shell: process.platform === 'win32',
    });
    let out = '';
    child.stdout.on('data', (c) => (out += c));
    child.on('error', () => resolve({ code: 1, out: '' }));
    child.on('close', (code) => resolve({ code: code ?? 1, out: out.trim() }));
  });
}

// RSSHub 现代构建产物入口（tsdown 产出 dist/index.mjs，start = node dist/index.mjs）。
function rsshubEntry() {
  return path.join(rsshubDir, 'dist', 'index.mjs');
}

async function main() {
  // 运行时前置：SlidevEnv 的 node.exe（RSSHub 运行时复用它）。缺失只告警——构建仍可进行，
  // 但要提醒用户运行时无 node 可用。
  const nodeBin = path.join(utilsDir, 'SlidevEnv', process.platform === 'win32' ? 'node.exe' : 'node');
  if (!(await exists(nodeBin))) {
    warn(`SlidevEnv node runtime not found at ${nodeBin}；运行时将无 node 启动 RSSHub（构建仍继续）`);
  }

  const entry = rsshubEntry();
  if (!force && (await exists(entry))) {
    info(`RSSHub already built (${entry}); skip. 用 --force 可重建。`);
    return;
  }

  // 构建期工具：host git + host npx（跑 pnpm）。缺任一则跳过（降级）。
  if ((await run('git', ['--version'])) !== 0) {
    warn('git 不可用，无法 clone RSSHub，跳过（聚合功能将不启用 RSSHub 源）');
    return;
  }

  // 1. 克隆源码到 utils/rsshub（--force 时先清空重来）。
  if (force && (await exists(rsshubDir))) {
    info('--force: 清空旧的 utils/rsshub');
    await rm(rsshubDir, { recursive: true, force: true });
  }
  await mkdir(utilsDir, { recursive: true });

  if (!(await exists(path.join(rsshubDir, '.git')))) {
    info(`cloning RSSHub (${RSSHUB_GIT_URL}) ...`);
    // 浅克隆默认分支，随后精确检出目标 ref（支持 commit/tag/branch）。
    const cloneCode = await run('git', ['clone', '--depth', '1', RSSHUB_GIT_URL, rsshubDir]);
    if (cloneCode !== 0) {
      warn('git clone 失败，跳过（网络/镜像问题？可设 RSSHUB_GIT_URL 用镜像）');
      return;
    }
  }

  // 检出固定 ref（master 以外的 commit/tag 需要额外 fetch）。
  if (RSSHUB_GIT_REF !== 'master') {
    info(`fetching + checkout ref: ${RSSHUB_GIT_REF}`);
    await run('git', ['fetch', '--depth', '1', 'origin', RSSHUB_GIT_REF], { cwd: rsshubDir });
    const co = await run('git', ['checkout', 'FETCH_HEAD'], { cwd: rsshubDir });
    if (co !== 0) {
      warn(`checkout ${RSSHUB_GIT_REF} 失败，将使用默认分支 HEAD`);
    }
  }

  // 记录实际 commit，便于复现/追溯。
  const { out: commit } = await runCapture('git', ['rev-parse', 'HEAD'], { cwd: rsshubDir });

  // 2. pnpm install（--frozen-lockfile 锁依赖树）+ build。用 npx 跑固定大版本 pnpm。
  //
  // 关键：--config.node-linker=hoisted 让 pnpm 装成**扁平** node_modules（和 npm 一样，
  // 全是实体文件、无符号链接）。默认的 pnpm 布局用 .pnpm store + 大量 symlink，
  // 而 utils 的 tar.zst 打包器只处理目录与普通文件、遇到 symlink 条目会直接报错，
  // 离线包就会缺文件。hoisted 布局才能被正确打包进离线安装包。
  info(`pnpm install (frozen lockfile, hoisted) via npx ${PNPM_SPEC} ... 依赖多，耗时较长`);
  const installCode = await run(
    'npx',
    ['--yes', PNPM_SPEC, 'install', '--frozen-lockfile', '--config.node-linker=hoisted'],
    { cwd: rsshubDir }
  );
  if (installCode !== 0) {
    warn(`pnpm install 失败（code ${installCode}），跳过（RSSHub 不可用，其余抓取不受影响）`);
    return;
  }

  info('pnpm build (生成 dist/index.mjs) ...');
  const buildCode = await run('npx', ['--yes', PNPM_SPEC, 'build'], { cwd: rsshubDir });
  if (buildCode !== 0) {
    warn(`pnpm build 失败（code ${buildCode}），跳过`);
    return;
  }

  if (await exists(entry)) {
    await writeFile(
      path.join(rsshubDir, '.rsshub-build.json'),
      JSON.stringify({ ref: RSSHUB_GIT_REF, commit, builtAt: new Date().toISOString() }, null, 2),
    ).catch(() => {});
    info(`RSSHub ready: ${entry} (commit ${commit || 'unknown'})`);
  } else {
    warn(`build 完成但未找到 ${entry}。RSSHub 的构建产物路径可能已变，请检查其 package.json 的 scripts.build 与 start。`);
  }
}

main().catch((err) => {
  warn(`unexpected error: ${err?.stack || err}`);
  // 刻意不 exit(非0)：不阻断 dev / build。
});
