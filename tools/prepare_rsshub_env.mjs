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
// - 本地 dev：拿不到网络/构建失败只告警，聚合功能降级，不阻断 build（同 fetch_codex_sidecar）。
// - 发布构建（RSSHUB_STRICT=1，CI 会设）：任何一步拿不到 dist 都直接失败，不允许降级发布。
// - 确实不需要聚合功能时显式 RSSHUB_SKIP=1。
//
// 上游兼容
// - @notionhq/client 5.x 的 CJS 产物不提供 ESM 命名导出 Client，RSSHub 的 build:routes
//   会整条失败；这里在构建前给它打一个最小补丁（见 patchNotionClientExport）。
//
// 用法：
//   node ./tools/prepare_rsshub_env.mjs           # 已构建好则跳过
//   node ./tools/prepare_rsshub_env.mjs --force   # 强制重新 clone + build
//   RSSHUB_GIT_REF=<commit|tag|branch> node ./tools/prepare_rsshub_env.mjs  # 指定版本
//   RSSHUB_GIT_URL=<mirror> ...                    # 指定镜像（GitHub 慢时用）

import { spawn } from 'node:child_process';
import path from 'node:path';
import { stat, rm, mkdir, readFile, writeFile } from 'node:fs/promises';

const projectRoot = process.cwd();
const utilsDir = path.join(projectRoot, 'src-tauri', 'utils');
const rsshubDir = path.join(utilsDir, 'rsshub');

// 固定源码版本：master 随时可能坏（2026-11-16 就因 @notionhq/client 5.x 的导出问题
// 整条构建失败），所以默认锁到验证过的 commit，保证不同时间/不同机器构建出一致的
// RSSHub。升级时改这里（或临时用 RSSHUB_GIT_REF 覆盖）并跑 --force。
const RSSHUB_GIT_REF = process.env.RSSHUB_GIT_REF || '38638e6947212e02b465d2bacb9c60caf36fcb92';
const RSSHUB_GIT_URL = process.env.RSSHUB_GIT_URL || 'https://github.com/DIYgod/RSSHub.git';
// 固定 pnpm 大版本（读取 pnpm-lock 需匹配）。corepack 坏了就用 npx。
const PNPM_SPEC = process.env.RSSHUB_PNPM_SPEC || 'pnpm@10';

// clone + install + build 总超时（依赖多、构建慢，给足）。
const STEP_TIMEOUT_MS = 30 * 60 * 1000;
const force = process.argv.includes('--force');
// 发布构建（CI）必须带上聚合功能：任何一步拿不到 dist 都直接失败，不再「降级发布」。
// 本地 dev 保持宽容，确实不需要时用 RSSHUB_SKIP=1 显式跳过。
const strict = process.env.RSSHUB_STRICT === '1';

function info(msg) { console.log(`[rsshub.prepare] ${msg}`); }
function warn(msg) { console.warn(`[rsshub.prepare] ${msg}`); }

/// 严格模式（发布构建）下直接抛错中止；否则只告警降级。
function failOrWarn(msg) {
  if (strict) {
    throw new Error(msg);
  }
  warn(msg);
}

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

// 失败时 node_modules 里常留着装了一半的依赖树（pnpm 的可选原生依赖构建失败会留下
// 指向不存在目录的符号链接），而没有 dist 它们也用不上。留着只会被 utils 打包器
// 整棵扫进去，白占几百 MB，还可能让打包直接失败。
async function dropBrokenInstall() {
  await rm(path.join(rsshubDir, 'node_modules'), { recursive: true, force: true }).catch(() => {});
  await rm(path.join(rsshubDir, 'dist'), { recursive: true, force: true }).catch(() => {});
  warn('已清掉 rsshub/node_modules 与 dist（残缺依赖树用不上，还会拖累打包）');
}

// @notionhq/client 5.x 的 CJS 产物把 Client 定义成 `Object.defineProperty(exports, "Client",
// { get: () => __importDefault(...).default })`，Node 的 ESM 命名导出探测识别不了这种写法，
// `import { Client }` 直接抛 "does not provide an export named 'Client'"，RSSHub 的
// build:routes 因此整条失败，聚合功能会整体缺失。从默认导出（CJS module.exports）上取回
// 同名类即可，其余命名导出保持不变。上游改掉这行之后匹配会落空，自动跳过。
const NOTION_IMPORT =
  "import { APIErrorCode, Client, isFullPage, isNotionClientError } from '@notionhq/client';";

async function patchNotionClientExport() {
  const file = path.join(rsshubDir, 'lib', 'routes', 'notion', 'database.ts');
  const source = await readFile(file, 'utf8').catch(() => null);
  if (source === null) {
    warn('找不到 notion 路由源码，跳过 @notionhq/client 兼容补丁');
    return;
  }
  if (!source.includes(NOTION_IMPORT)) {
    info('notion 路由的 Client 引入方式已变化（或补丁已打过），跳过兼容补丁');
    return;
  }
  const replacement = [
    "import NotionClientModule, { APIErrorCode, isFullPage, isNotionClientError } from '@notionhq/client';",
    '',
    '// @notionhq/client 5.x 把 Client 挂在默认导出（CJS module.exports）上，命名导出取不到。',
    'const Client = (NotionClientModule as any).Client ?? NotionClientModule;',
  ].join('\n');
  await writeFile(file, source.replace(NOTION_IMPORT, replacement));
  info('已为 notion 路由打上 @notionhq/client 命名导出兼容补丁');
}

async function main() {
  if (process.env.RSSHUB_SKIP === '1') {
    info('RSSHUB_SKIP=1，跳过 RSSHub 构建（聚合功能降级）');
    return;
  }
  // 运行时前置：SlidevEnv 的 node.exe（RSSHub 运行时复用它）。没有它 dist 也起不来，
  // 发布构建必须当场失败。
  const nodeBin = path.join(utilsDir, 'SlidevEnv', process.platform === 'win32' ? 'node.exe' : 'node');
  if (!(await exists(nodeBin))) {
    failOrWarn(`SlidevEnv node runtime not found at ${nodeBin}；没有它 RSSHub 起不来`);
  }

  const entry = rsshubEntry();
  if (!force && (await exists(entry))) {
    info(`RSSHub already built (${entry}); skip. 用 --force 可重建。`);
    return;
  }

  // 构建期工具：host git + host npx（跑 pnpm）。
  if ((await run('git', ['--version'])) !== 0) {
    failOrWarn('git 不可用，无法 clone RSSHub，聚合功能将缺失');
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
      failOrWarn('git clone 失败（网络/镜像问题？可设 RSSHUB_GIT_URL 用镜像）');
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

  await patchNotionClientExport();

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
    await dropBrokenInstall();
    failOrWarn(`pnpm install 失败（code ${installCode}），聚合功能将缺失`);
    return;
  }

  info('pnpm build (生成 dist/index.mjs) ...');
  const buildCode = await run('npx', ['--yes', PNPM_SPEC, 'build'], { cwd: rsshubDir });
  if (buildCode !== 0) {
    await dropBrokenInstall();
    failOrWarn(`pnpm build 失败（code ${buildCode}），聚合功能将缺失`);
    return;
  }

  if (await exists(entry)) {
    await writeFile(
      path.join(rsshubDir, '.rsshub-build.json'),
      JSON.stringify({ ref: RSSHUB_GIT_REF, commit, builtAt: new Date().toISOString() }, null, 2),
    ).catch(() => {});
    info(`RSSHub ready: ${entry} (commit ${commit || 'unknown'})`);
  } else {
    failOrWarn(`build 完成但未找到 ${entry}。RSSHub 的构建产物路径可能已变，请检查其 package.json 的 scripts.build 与 start。`);
  }
}

main().catch((err) => {
  if (strict) {
    warn(`中止（RSSHUB_STRICT=1）：${err?.message || err}`);
    process.exit(1);
  }
  // 本地 dev 不 exit(非0)：RSSHub 缺失只降级聚合功能，不阻断 dev / build。
  warn(`unexpected error: ${err?.stack || err}`);
});
