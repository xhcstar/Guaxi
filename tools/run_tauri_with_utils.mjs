import { copyFile, mkdir, readdir, readFile, rm, stat, utimes, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

// This wrapper is the single user-facing entry for `npm run tauri ...`.
// It keeps dev lightweight, while build/bundle runs the heavier asset and
// external utils bundle preparation steps required by release packaging.
const args = process.argv.slice(2);
const reuseFlag = '--reuse-utils-zip';
const reuseExisting = args.includes(reuseFlag);
const tauriArgs = args.filter((arg) => arg !== reuseFlag);
const tauriSubcommand = tauriArgs[0] ?? '';
const shouldPrepareForTauriCommand =
  tauriSubcommand === 'build' || tauriSubcommand === 'bundle';
const projectRoot = process.cwd();

// ---------------------------------------------------------------------------
// CPU 版与 GPU 版各用一份 target 目录
//
// 两套特性的依赖图不是同一份：`gpu` 把 sherpa-onnx 从 static 换成 shared，连带
// libgit2-sys / pyo3 / onnxruntime 这些又慢又大的 crate 都要重编。以前两种构建
// 共用 `src-tauri/target`，在 CPU ↔ GPU 之间来回切一次就等于「全量重编一轮」，
// 17 分钟的编译时间里有一大半是这么白烧的。
//
// 拆开之后：同一台机器上切特性只是换一个 target 目录，各自的增量仍然有效。
// CPU(默认) 继续用原来的 `target`，GPU 用 `target-gpu`（已在 .gitignore 里）。
// ---------------------------------------------------------------------------
const isGpuBuild = usesGpuFeature(tauriArgs);
const cargoTargetDir = path.join(
  projectRoot,
  'src-tauri',
  isGpuBuild ? 'target-gpu' : 'target',
);
const cargoReleaseDir = path.join(cargoTargetDir, 'release');
// 打包/安装辅助工具是独立 crate（src-tauri/bundle-tools），用固定的 target 目录，
// CPU / GPU 两种构建共用同一份工具产物，不再和应用的依赖图纠缠。
const bundleToolsDir = path.join(projectRoot, 'src-tauri', 'bundle-tools');
const bundleToolsTargetDir = path.join(projectRoot, 'src-tauri', 'target-bundle-tools');
const utilsBundlesSourceDir = path.join(projectRoot, 'src-tauri', 'utils', '__bundles');
const windowsNsisOutputDir = path.join(
  cargoReleaseDir,
  'bundle',
  'nsis',
);
const windowsExternalBundlesDir = path.join(windowsNsisOutputDir, 'utils_bundles');
const windowsExternalReadme = path.join(windowsNsisOutputDir, 'README_utils_bundles.txt');
const windowsExtractHelperSource = path.join(
  bundleToolsTargetDir,
  'release',
  'extract_utils_bundles.exe',
);
const windowsExtractHelperOutput = path.join(
  windowsNsisOutputDir,
  'extract_utils_bundles.exe',
);
const windowsCudaRuntimeSourceBundlesDir = path.join(
  cargoTargetDir,
  'cuda-runtime-bundles',
);
const windowsCudaRuntimeBundlesDir = path.join(
  windowsNsisOutputDir,
  'cuda_runtime_bundles',
);
const BUNDLE_MANIFEST_FILE = 'bundle-manifest.json';

// ---------------------------------------------------------------------------
// 构建日志
//
// 这条链路上有几段是「一句话都不说地跑几分钟」：扫十几万个文件、把几 GB 数据压成
// tar.zst、makensis 封安装包。以前只有结束时的摘要，中途完全空白，很容易被当成卡死。
// 这里给每一步加三样东西：
//   · 阶段头（第几步 / 共几步 + 这一步在做什么）；
//   · 心跳（子进程长时间没输出时，每 20s 报一次已用时间）；
//   · 结束耗时（最后汇总成一张阶段耗时表，方便定位是哪一步变慢了）。
//
// 环境变量：
//   GUAXI_BUILD_HEARTBEAT_MS  心跳间隔，0 关闭（默认 20000）
//   GUAXI_BUILD_VERBOSE=1     额外打印每一步的完整命令行
// ---------------------------------------------------------------------------
const HEARTBEAT_MS = (() => {
  const raw = Number(process.env.GUAXI_BUILD_HEARTBEAT_MS ?? 20000);
  return Number.isFinite(raw) && raw >= 0 ? raw : 20000;
})();
const VERBOSE = ['1', 'true', 'yes', 'on'].includes(
  String(process.env.GUAXI_BUILD_VERBOSE ?? '').toLowerCase(),
);
const stageTimings = [];
let stageIndex = 0;
let stageTotal = 0;
const buildStartedAt = Date.now();

function formatDuration(ms) {
  const seconds = ms / 1000;
  if (seconds < 10) {
    return `${seconds.toFixed(1)}s`;
  }
  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes}m${String(rest).padStart(2, '0')}s`;
}

function formatBytes(bytes) {
  const MB = 1024 * 1024;
  const GB = MB * 1024;
  if (bytes >= GB) {
    return `${(bytes / GB).toFixed(2)} GB`;
  }
  if (bytes >= MB) {
    return `${Math.round(bytes / MB)} MB`;
  }
  return `${Math.round(bytes / 1024)} KB`;
}

/// 打印阶段头。detail 用来交代「为什么要做这一步」和预期耗时。
function stage(title, detail) {
  stageIndex += 1;
  console.log(
    `\n[tauri] [${stageIndex}/${stageTotal}] ${title}${detail ? ` · ${detail}` : ''}`,
  );
  return { title, startedAt: Date.now() };
}

function stageDone(reference, note) {
  const elapsed = Date.now() - reference.startedAt;
  stageTimings.push({ title: reference.title, elapsed });
  console.log(
    `[tauri] [${stageIndex}/${stageTotal}] ${reference.title} 完成 · ${formatDuration(elapsed)}${note ? ` · ${note}` : ''}`,
  );
}

/// 跳过某个阶段时也留一行，否则日志里会「少一步」而看不出是刻意跳过。
function stageSkipped(title, reason) {
  stageIndex += 1;
  stageTimings.push({ title, elapsed: 0, skipped: true });
  console.log(`[tauri] [${stageIndex}/${stageTotal}] ${title} 跳过 · ${reason}`);
}

/// 「Tauri 构建与打包」内部还有三个子步骤，最长的那个能跑十几分钟。
///
/// tauri CLI 的 stdout 是 inherit 的（不经过本进程），没法靠解析输出来切分阶段；
/// 但三步各自会落一个产物文件，于是改成盯文件：产物一出现就打印一条带耗时的
/// 里程碑，心跳也顺便告诉你「现在卡在哪一步」。
///
/// probe() 返回产物的 mtimeMs，拿不到就返回 null（这一步还没开始）。
function trackBuildPhases(phases, startedAt) {
  const startedAtMs = startedAt;
  let index = 0;
  let phaseStartedAt = startedAtMs;
  let polling = false;
  let stopped = false;

  const probe = async (phase) => {
    try {
      return await phase.probe();
    } catch {
      return null;
    }
  };
  // 产物是这一步写出来的才算数：上一轮留下的旧文件不能拿来充数。
  const landed = (stamp) => Boolean(stamp) && stamp >= startedAtMs - 1000;

  console.log(
    `[tauri]      阶段内里程碑：${phases.map((phase) => phase.label).join(' → ')}（逐个打印耗时）`,
  );

  const tick = async () => {
    if (stopped || polling) {
      return;
    }
    polling = true;
    try {
      while (index < phases.length) {
        const phase = phases[index];
        if (landed(await probe(phase))) {
          const now = Date.now();
          console.log(
            `[tauri]      ✓ ${phase.label} 完成 · 本步 ${formatDuration(now - phaseStartedAt)} · 阶段内 ${formatDuration(now - startedAtMs)}`,
          );
          index += 1;
          phaseStartedAt = now;
          continue;
        }
        // 后面的步骤已经落产物、而这一步还没有 → cargo/tauri 这轮把它跳过了
        // （比如源码没变，链接被判定为 fresh）。不然后面的里程碑会被一步卡死。
        let skipped = false;
        for (let later = index + 1; later < phases.length; later += 1) {
          if (landed(await probe(phases[later]))) {
            skipped = true;
            break;
          }
        }
        if (!skipped) {
          break;
        }
        const now = Date.now();
        console.log(
          `[tauri]      · ${phase.label} 未产生新产物（本轮跳过）· 阶段内 ${formatDuration(now - startedAtMs)}`,
        );
        index += 1;
        phaseStartedAt = now;
      }
    } finally {
      polling = false;
    }
  };

  const timer = setInterval(tick, 2000);
  timer.unref?.();

  return {
    currentLabel() {
      if (index >= phases.length) {
        return null;
      }
      return phases[index].label;
    },
    currentElapsedMs() {
      return Date.now() - phaseStartedAt;
    },
    remainingLabels() {
      return phases.slice(index).map((phase) => phase.label);
    },
    /// 阶段结束时补一次探测：最后一步往往在两次轮询之间就完成了（NSIS 只跑 3 分钟，
    /// 但真正落盘是在收尾那一秒），不补这一次就会漏掉最后一条里程碑。
    async stop() {
      clearInterval(timer);
      // 先补跑一次 tick 再置 stopped：tick 开头会挡掉 stopped 之后的调用。
      await tick();
      stopped = true;
      // 只有「真的跑了很久却什么都没观测到」才值得提一句；秒过的空构建不必刷屏。
      if (Date.now() - startedAtMs > 120000 && index < phases.length) {
        console.log(
          `[tauri]      未观测到产物的子步骤：${this.remainingLabels().join(' → ')}`,
        );
      }
    },
  };
}

/// 取某个文件/目录里最新 .exe 的 mtime，用来判断 NSIS 是否已经产出安装包。
async function newestExeMtime(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  let newest = null;
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.toLowerCase().endsWith('.exe')) {
      continue;
    }
    const info = await stat(path.join(dir, entry.name));
    newest = newest === null ? info.mtimeMs : Math.max(newest, info.mtimeMs);
  }
  return newest;
}

async function fileMtime(target) {
  return (await stat(target)).mtimeMs;
}

/// 中文字符在终端占 2 列，用 padEnd 对不齐，这里按显示宽度补空格。
function padLabel(text, width) {
  let used = 0;
  for (const char of text) {
    used += /[\u3000-\u9fff\uff00-\uffef]/.test(char) ? 2 : 1;
  }
  return text + ' '.repeat(Math.max(1, width - used));
}

function printSummary() {
  if (stageTimings.length === 0) {
    return;
  }
  const total = Date.now() - buildStartedAt;
  console.log(`\n[tauri] ───── 构建结束 · 总用时 ${formatDuration(total)} ─────`);
  for (const entry of stageTimings) {
    const label = entry.skipped ? '跳过' : formatDuration(entry.elapsed);
    console.log(`[tauri]   ${padLabel(entry.title, 30)}${label}`);
  }
}


// 语音功能的 GPU 构建需要 SHERPA_ONNX_LIB_DIR 指向已装配的 CUDA 库目录。
// 与其让每台机器各自 export 一遍，不如在这里按 `--features gpu` 自动补上——
// 命令因此保持成一条，文档和 CI 都不必再记环境变量。
const SHERPA_VERSION = '1.13.8';
const sherpaCudaLibDir = path.join(
  projectRoot,
  'src-tauri',
  '.sherpa-onnx-libs',
  SHERPA_VERSION,
  'cuda-lib',
);

/// 只认 `--features gpu` / `--features=a,b` 这类显式写法，避免误匹配别的参数。
function usesGpuFeature(commandArgs) {
  for (let index = 0; index < commandArgs.length; index += 1) {
    const arg = commandArgs[index];
    if (arg === '--features' || arg === '-F') {
      if ((commandArgs[index + 1] ?? '').split(',').includes('gpu')) {
        return true;
      }
    } else if (arg.startsWith('--features=') || arg.startsWith('-F=')) {
      if (arg.slice(arg.indexOf('=') + 1).split(',').includes('gpu')) {
        return true;
      }
    }
  }
  return false;
}

/// GPU 版构建完，在真正的产物目录上跑一次 CUDA 自检。
///
/// 权限分明：退出码 2（目录里缺该有的文件）是打包缺陷，直接中断构建，别发出去；
/// 退出码 3（文件都在但本机加载不了——没插 N 卡、驱动太旧之类）只提醒，
/// 因为包本身可能是好的，换台有 N 卡的机器就能用。
async function verifyCudaRuntimeForRelease() {
  if (process.platform !== 'win32') {
    return;
  }

  const releaseDir = cargoReleaseDir;
  if (!existsSync(releaseDir)) {
    console.warn(`[tauri] 未找到 release 产物目录，跳过 CUDA 自检：${releaseDir}`);
    return;
  }

  const buildExitCode = await ensureCargoBinBuilt(
    'verify_cuda_runtime',
    {},
    { label: 'CUDA 自检工具编译' },
  );
  if (buildExitCode !== 0) {
    console.warn('[tauri] CUDA 自检工具构建失败，跳过自检。');
    return;
  }

  const verifier = await binTargetPath('verify_cuda_runtime', true);
  if (!existsSync(verifier)) {
    console.warn(`[tauri] 未找到 CUDA 自检工具，跳过自检：${verifier}`);
    return;
  }

  console.log('[tauri] GPU 版自检：检查安装目录旁的 CUDA 运行库能否加载 …');
  const exitCode = await run(verifier, ['--dir', releaseDir], {}, { label: 'CUDA 自检' });
  if (exitCode === 2) {
    console.error(
      '[tauri] GPU 版自检发现打包缺陷（安装目录缺少 CUDA 运行库），已中止构建。\n' +
        '        修复：运行 npm run sherpa:fetch:cuda 重新装配原生库。',
    );
    process.exit(2);
  }
  if (exitCode !== 0) {
    console.warn(
      '[tauri] GPU 版自检未通过：这份包在当前机器上不会启用 GPU（构建继续）。\n' +
        '        目标是别的有 N 卡的机器时可以先忽略；要在本机用上 GPU，见上面自检输出的原因。',
    );
  }
}

function tauriInvocation() {
  // On Windows we invoke the generated `tauri.cmd` through cmd.exe.
  // Spawning the cmd shim directly is less reliable across shells.
  if (process.platform === 'win32') {
    return {
      command: 'cmd.exe',
      args: ['/d', '/s', '/c', '.\\node_modules\\.bin\\tauri.cmd', ...tauriArgs],
    };
  }

  return {
    command: './node_modules/.bin/tauri',
    args: tauriArgs,
  };
}

function run(command, commandArgs, extraEnv = {}, options = {}) {
  const { label, heartbeatMs = HEARTBEAT_MS, heartbeatNote } = options;
  if (VERBOSE) {
    console.log(`[tauri]     $ ${[command, ...commandArgs].join(' ')}`);
  }
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    // Disable npm's own update notifier so build logs stay focused on the
    // project pipeline instead of tool-upgrade suggestions.
    const child = spawn(command, commandArgs, {
      cwd: projectRoot,
      stdio: 'inherit',
      shell: false,
      env: {
        ...process.env,
        NO_UPDATE_NOTIFIER: '1',
        npm_config_update_notifier: 'false',
        ...extraEnv,
      },
    });

    // 子进程自己多久没输出是无法直接观测的（stdio 走 inherit），所以这里按固定
    // 间隔无条件报一次「还在跑」。宁可偶尔多说一句，也不要让终端长时间定格。
    let heartbeatTimer = null;
    if (label && heartbeatMs > 0) {
      heartbeatTimer = setInterval(() => {
        let note = null;
        if (typeof heartbeatNote === 'function') {
          try {
            note = heartbeatNote();
          } catch {
            note = null;
          }
        }
        console.log(
          `[tauri]     … ${label} 仍在进行，已用 ${formatDuration(Date.now() - startedAt)}${note ? ` · ${note}` : ''}`,
        );
      }, heartbeatMs);
      heartbeatTimer.unref?.();
    }
    const stopHeartbeat = () => {
      if (heartbeatTimer) {
        clearInterval(heartbeatTimer);
        heartbeatTimer = null;
      }
    };

    child.on('exit', (code, signal) => {
      stopHeartbeat();
      if (signal) {
        reject(new Error(`${command} terminated by signal ${signal}`));
        return;
      }
      resolve(code ?? 0);
    });

    child.on('error', (error) => {
      stopHeartbeat();
      reject(error);
    });
  });
}

// 打包辅助 bin（prepare_utils_zip / prepare_cuda_runtime_bundle / extract_utils_bundles /
// verify_cuda_runtime）现在位于独立 crate `src-tauri/bundle-tools`，只依赖
// rayon/zstd/tar/serde/blake3，不再牵动应用的整张依赖图（也不会触发 sherpa 的原生库
// 构建脚本）。这里保留「exe 已是最新就直接执行」的快路径，免掉 cargo 的固定校验开销。
async function binTargetPath(bin, release) {
  const exeName = process.platform === 'win32' ? `${bin}.exe` : bin;
  return path.join(bundleToolsTargetDir, release ? 'release' : 'debug', exeName);
}

function binInputFiles(bin) {
  return [
    path.join(projectRoot, 'src-tauri', 'utils_zip_support.rs'),
    path.join(bundleToolsDir, 'src', 'bin', `${bin}.rs`),
    path.join(bundleToolsDir, 'Cargo.toml'),
    path.join(bundleToolsDir, 'Cargo.lock'),
  ];
}

async function isPrebuiltBinFresh(bin, release) {
  let exeStat;
  try {
    exeStat = await stat(await binTargetPath(bin, release));
  } catch {
    return false;
  }
  if (!exeStat.isFile()) {
    return false;
  }
  for (const source of binInputFiles(bin)) {
    try {
      if ((await stat(source)).mtimeMs > exeStat.mtimeMs) {
        return false;
      }
    } catch {
      // 输入文件缺失时无法证明 exe 是最新的，交给 cargo 判断。
      return false;
    }
  }
  return true;
}

function cargoBinArgs(bin, args, release) {
  const cargoArgs = [
    'run',
    '--quiet',
    ...(release ? ['--release'] : []),
    '--manifest-path',
    'src-tauri/bundle-tools/Cargo.toml',
    '--bin',
    bin,
  ];
  if (args.length > 0) {
    cargoArgs.push('--', ...args);
  }
  return cargoArgs;
}

async function runCargoBin(bin, args, extraEnv = {}, options = {}, release = true) {
  if (await isPrebuiltBinFresh(bin, release)) {
    if (VERBOSE) {
      console.log(`[tauri]     ${bin}: 复用已编译好的可执行文件（输入未变）`);
    }
    return run(await binTargetPath(bin, release), args, extraEnv, options);
  }
  if (VERBOSE || options.label) {
    console.log(`[tauri]     ${bin}: 可执行文件缺失或已过期，先 cargo build（首次可能要编译一批依赖）`);
  }
  return run(
    'cargo',
    cargoBinArgs(bin, args, release),
    { ...bundleToolsEnv(), ...extraEnv },
    options,
  );
}

async function ensureCargoBinBuilt(bin, extraEnv = {}, options = {}, release = true) {
  if (await isPrebuiltBinFresh(bin, release)) {
    if (VERBOSE) {
      console.log(`[tauri]     ${bin}: 复用已编译好的可执行文件（输入未变）`);
    }
    return 0;
  }
  return run(
    'cargo',
    [
      'build',
      '--quiet',
      ...(release ? ['--release'] : []),
      '--manifest-path',
      'src-tauri/bundle-tools/Cargo.toml',
      '--bin',
      bin,
    ],
    { ...bundleToolsEnv(), ...extraEnv },
    options,
  );
}

/// 打包工具是独立 crate，固定一份 target 目录即可；和应用的 target/target-gpu 无关。
function bundleToolsEnv() {
  return { CARGO_TARGET_DIR: bundleToolsTargetDir };
}

/// 所有 cargo 调用都用同一个 target 目录，和上面算出来的产物路径一一对应。
/// CPU 版 / GPU 版各走各的，互不清空对方的增量缓存。
function cargoTargetEnv() {
  return { CARGO_TARGET_DIR: cargoTargetDir };
}

/// 单个 rustc codegen 任务的峰值内存估算（GB）。
///
/// 这个 crate 20 万行、release 下 `codegen-units = 16`，单个 CGU 的 LLVM 峰值实测
/// 在 1.5–2.5 GB。用 2.5 当前界：宁可少开两个 job，也不要 OOM 掉一整轮编译
/// （32GB 机器、15GB 可用内存开 10 个 job 时，rustc 会在第 10 分钟直接
/// `rustc-LLVM ERROR: out of memory`，前面十几分钟全白烧）。
const RUSTC_PEAK_GB_PER_JOB = 2.5;

/// 可用内存里只敢用掉这么多：IDE、浏览器、杀软扫描、vite 残留都还要吃内存。
const MEMORY_HEADROOM = 0.7;

/// release 构建的并行度：**按可用内存算，再受核数约束**。
///
/// 早先写死 4 是为了不 OOM，代价是 20 线程的机器只用了 1/5 的算力；改成「核数一半」
/// 之后又反过来炸了内存——因为真正决定能不能开 10 个 job 的不是核数，而是此刻还剩
/// 多少内存。`os.freemem()` 在 Windows 上返回的就是「可用物理内存」，正好用来判断。
///
/// 结果取两者较小值，并且夹在 2–10 之间：内存紧张时降到 2 也比 OOM 强。
/// 想自己控制就用 `CARGO_BUILD_JOBS` 覆盖。
function releaseJobsBudget() {
  const cores = typeof os.availableParallelism === 'function'
    ? os.availableParallelism()
    : os.cpus().length;
  const coreLimit = Math.max(4, Math.min(10, Math.round(cores / 2)));
  const freeGb = os.freemem() / 1024 ** 3;
  const usableGb = freeGb * MEMORY_HEADROOM;
  const memoryLimit = Math.floor(usableGb / RUSTC_PEAK_GB_PER_JOB);
  const jobs = Math.max(2, Math.min(coreLimit, memoryLimit));
  return { jobs, cores, coreLimit, freeGb, memoryLimit };
}

function defaultReleaseJobs() {
  return releaseJobsBudget().jobs;
}

function releaseCargoEnv() {
  if (process.platform !== 'win32') {
    return {};
  }

  return {
    CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || String(defaultReleaseJobs()),
    CARGO_INCREMENTAL: '0',
  };
}

async function collectBundleEntries(dir, label) {
  let entries = [];
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch (error) {
    throw new Error(`读取 ${label} 分包目录失败: ${dir} (${error.message})`);
  }

  const files = [];
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.toLowerCase().endsWith('.tar.zst')) {
      continue;
    }
    const fullPath = path.join(dir, entry.name);
    const fileStat = await stat(fullPath);
    files.push({ name: entry.name, fullPath, size: fileStat.size });
  }

  if (files.length === 0) {
    throw new Error(`未找到可发布的 ${label} 分包: ${dir}`);
  }
  return files;
}

// Copy only when the destination is missing or differs from the source.
// Preserving the source mtime lets the next build skip the stat/compare/hash
// work entirely; multi-GB bundle directories must never be deleted and copied
// wholesale again.
async function copyFileIfChanged(sourcePath, targetPath) {
  const sourceStat = await stat(sourcePath);
  let targetStat = null;
  try {
    targetStat = await stat(targetPath);
  } catch {
    targetStat = null;
  }

  if (
    targetStat?.isFile() &&
    targetStat.size === sourceStat.size &&
    Math.abs(targetStat.mtimeMs - sourceStat.mtimeMs) < 2
  ) {
    return false;
  }

  await mkdir(path.dirname(targetPath), { recursive: true });
  await copyFile(sourcePath, targetPath);
  await utimes(targetPath, sourceStat.atime, sourceStat.mtime);
  return true;
}

async function writeFileIfChanged(targetPath, content) {
  try {
    if ((await readFile(targetPath, 'utf8')) === content) {
      return false;
    }
  } catch {
    // Missing/unreadable target: fall through and write it.
  }
  await mkdir(path.dirname(targetPath), { recursive: true });
  await writeFile(targetPath, content, 'utf8');
  return true;
}

async function publishBundleDir(sourceDir, targetDir) {
  await mkdir(targetDir, { recursive: true });

  const sourceEntries = await readdir(sourceDir, { withFileTypes: true });
  const sourceFiles = sourceEntries
    .filter(
      (entry) =>
        entry.isFile() &&
        (entry.name.toLowerCase().endsWith('.tar.zst') ||
          entry.name === BUNDLE_MANIFEST_FILE),
    )
    .map((entry) => entry.name);

  if (!sourceFiles.includes(BUNDLE_MANIFEST_FILE)) {
    throw new Error(`资源目录缺少 ${BUNDLE_MANIFEST_FILE}: ${sourceDir}`);
  }

  // Remove stale bundles from previous builds, but leave unrelated files alone.
  const keep = new Set(sourceFiles);
  for (const entry of await readdir(targetDir, { withFileTypes: true })) {
    if (
      entry.isFile() &&
      (entry.name.toLowerCase().endsWith('.tar.zst') ||
        entry.name === BUNDLE_MANIFEST_FILE) &&
      !keep.has(entry.name)
    ) {
      await rm(path.join(targetDir, entry.name), { force: true });
    }
  }

  let copied = 0;
  let skipped = 0;
  let nextIndex = 0;
  const workers = Array.from(
    { length: Math.min(4, sourceFiles.length) },
    async () => {
      while (nextIndex < sourceFiles.length) {
        const name = sourceFiles[nextIndex];
        nextIndex += 1;
        const changed = await copyFileIfChanged(
          path.join(sourceDir, name),
          path.join(targetDir, name),
        );
        if (changed) {
          copied += 1;
        } else {
          skipped += 1;
        }
      }
    },
  );
  await Promise.all(workers);
  return { copied, skipped, files: sourceFiles.length };
}

async function publishExternalWindowsBundles(includeCudaRuntime) {
  if (process.platform !== 'win32') {
    return;
  }

  // Windows installers do not embed the large utils runtime payload anymore.
  // Instead, release builds publish external bundle files next to the setup EXE,
  // and the NSIS post-install hook expands them into `$INSTDIR\\utils`.
  const utilsEntries = await collectBundleEntries(utilsBundlesSourceDir, 'utils');
  // The GPU build externalizes the CUDA runtime for the same reason: bundled
  // into the installer it pushes the NSIS data block past its ~2GB limit.
  const cudaEntries = includeCudaRuntime
    ? await collectBundleEntries(windowsCudaRuntimeSourceBundlesDir, 'CUDA 运行库')
    : [];

  await mkdir(windowsNsisOutputDir, { recursive: true });
  const totalSourceBytes = [...utilsEntries, ...cudaEntries].reduce(
    (sum, entry) => sum + entry.size,
    0,
  );
  console.log(
    `[tauri]     待发布 ${utilsEntries.length + cudaEntries.length} 个分包 · ${formatBytes(totalSourceBytes)}（内容未变的会跳过复制）`,
  );
  const publishStartedAt = Date.now();
  const utilsPublish = await publishBundleDir(
    utilsBundlesSourceDir,
    windowsExternalBundlesDir,
  );
  const helperChanged = await copyFileIfChanged(
    windowsExtractHelperSource,
    windowsExtractHelperOutput,
  );
  let cudaPublish = null;
  if (includeCudaRuntime) {
    cudaPublish = await publishBundleDir(
      windowsCudaRuntimeSourceBundlesDir,
      windowsCudaRuntimeBundlesDir,
    );
  } else {
    // A CPU build must not ship CUDA DLLs left over from a previous GPU build
    // in the same target directory: the installer would happily extract them.
    await rm(windowsCudaRuntimeBundlesDir, { recursive: true, force: true });
  }

  const totalBytes = [...utilsEntries, ...cudaEntries].reduce(
    (sum, entry) => sum + entry.size,
    0,
  );
  const readme = [
    'Keep "utils_bundles", "cuda_runtime_bundles" (GPU builds) and "extract_utils_bundles.exe" next to the Guaxi setup executable.',
    'Each bundle folder must also contain bundle-manifest.json; the installer uses it to skip unchanged resources and to verify archive integrity.',
    'The installer reads these external tar.zst bundle files during installation: utils bundles expand into the installed utils directory, CUDA runtime bundles into the install root next to guaxi.exe.',
    '',
    `Utils bundle count: ${utilsEntries.length}`,
    ...(includeCudaRuntime ? [`CUDA runtime bundle count: ${cudaEntries.length}`] : []),
    `Total bytes: ${totalBytes}`,
    '',
    ...utilsEntries.map((entry) => `utils_bundles/${entry.name}\t${entry.size}`),
    ...cudaEntries.map((entry) => `cuda_runtime_bundles/${entry.name}\t${entry.size}`),
    '',
  ].join('\n');
  await writeFileIfChanged(windowsExternalReadme, readme);

  console.log(
    `[utils.bundle] published external windows bundles dir=${windowsExternalBundlesDir} bundleCount=${utilsEntries.length} totalBytes=${totalBytes} copied=${utilsPublish.copied} skipped=${utilsPublish.skipped} helperChanged=${helperChanged}`,
  );
  if (includeCudaRuntime && cudaPublish) {
    console.log(
      `[cuda.bundle] published external windows bundles dir=${windowsCudaRuntimeBundlesDir} bundleCount=${cudaEntries.length} copied=${cudaPublish.copied} skipped=${cudaPublish.skipped}`,
    );
  }
  console.log(
    `[tauri]     发布完成 · 用时 ${formatDuration(Date.now() - publishStartedAt)} · 目录 ${windowsNsisOutputDir}`,
  );
}

async function main() {
  const isWindows = process.platform === 'win32';

  // 阶段总数先算出来，日志里才能显示「第几步 / 共几步」，让人知道离结束还有多远。
  stageTotal = 2;
  if (isGpuBuild && shouldPrepareForTauriCommand) {
    stageTotal += 1;
  }
  if (shouldPrepareForTauriCommand) {
    stageTotal += 5 + (isWindows ? 1 : 0);
  }
  stageTotal += 1;
  if (shouldPrepareForTauriCommand) {
    stageTotal += (isGpuBuild ? 1 : 0) + 1;
  }

  console.log(
    `[tauri] 启动 ${tauriArgs.join(' ') || '(无子命令)'} · 平台 ${process.platform} · ` +
      `${isGpuBuild ? 'GPU(shared)' : 'CPU(static)'}${reuseExisting ? ' · 复用已有分包' : ''} · 共 ${stageTotal} 步`,
  );
  console.log(
    `[tauri] target 目录 ${path.relative(projectRoot, cargoTargetDir) || '.'}（CPU / GPU 各一份，互不覆盖增量）`,
  );
  if (VERBOSE) {
    console.log(`[tauri] 心跳间隔 ${HEARTBEAT_MS}ms · 详细模式已开启`);
  }
  if (isWindows && !shouldPrepareForTauriCommand) {
    // dev 不套 release 那套环境变量，这里明说一句，免得看到「没有并行度限制」以为是漏配。
    console.log(
      '[tauri] 非 build/bundle（如 dev）：增量编译保持开启、并行度交给 cargo 默认，不注入 release 变量；\n' +
        '[tauri]     编译进度由 cargo 自己打印（Compiling / Building [===>]），这里不发心跳',
    );
  }
  if (isWindows && shouldPrepareForTauriCommand) {
    if (process.env.CARGO_BUILD_JOBS) {
      console.log(
        `[tauri] release 并行度 CARGO_BUILD_JOBS=${process.env.CARGO_BUILD_JOBS}（来自环境变量）`,
      );
    } else {
      const budget = releaseJobsBudget();
      console.log(
        `[tauri] release 并行度 CARGO_BUILD_JOBS=${budget.jobs}` +
          `（${budget.cores} 线程上限 ${budget.coreLimit} · 可用内存 ${budget.freeGb.toFixed(1)}GB 上限 ${budget.memoryLimit} · 单个 rustc 峰值按 ${RUSTC_PEAK_GB_PER_JOB}GB 估）`,
      );
    }
  }

  // Codex Agent 引擎依赖 `src-tauri/utils/codex/codex.exe`。这一步是幂等的（已存在即跳过），
  // 并且刻意不校验退出码：拿不到 codex 时智能助手会回退到内置 Python Agent，
  // 不应该因为下载失败就阻断 dev / build。
  const codexStage = stage('Codex 侧车', 'src-tauri/utils/codex，缺失只告警不阻断');
  await run('node', ['./tools/fetch_codex_sidecar.mjs'], {}, { label: 'Codex 侧车准备' });
  stageDone(codexStage);

  // PDF 阅读与转换依赖 PDFium 动态库；Tauri 的 resources 会引用 src-tauri/libs。
  // 二进制不随仓库分发，构建前按固定版本取一份；已有则跳过。
  const pdfiumStage = stage('PDFium 运行时', 'src-tauri/libs，缺失则按固定版本下载');
  const pdfiumExitCode = await run(
    'node',
    ['./tools/fetch_pdfium.mjs'],
    {},
    { label: 'PDFium 运行时准备' },
  );
  if (pdfiumExitCode !== 0) {
    console.error(
      '[tauri] PDFium 运行时准备失败。PDF 阅读/转换依赖该动态库，故在此中止；\n' +
        '        可单独重试：node tools/fetch_pdfium.mjs',
    );
    process.exit(pdfiumExitCode);
  }
  stageDone(pdfiumStage);

  // 语音功能的原生库与模型。必须放在这里（而不是 only-in-build 分支里），原因有两条：
  //   · 原生库要赶在**任何** cargo 构建之前就位——sherpa-onnx 的 build.rs 找不到归档时
  //     会自己去联网下载，而只有系统代理（注册表）的环境下会直接卡住；
  //   · 模型要赶在 prepare_utils_zip 打包之前就位，否则不会被打进离线包。
  // 脚本自身幂等（已存在即跳过），所以 dev 下重复执行没有代价。
  const sherpaStage = stage(
    '语音原生库与模型',
    'sherpa-onnx 静态库 + TTS/ASR 模型，已就位则秒过',
  );
  const sherpaExitCode = await run(
    'node',
    ['./tools/fetch_sherpa_onnx_libs.mjs'],
    {},
    { label: '语音库预取' },
  );
  if (sherpaExitCode !== 0) {
    console.error(
      '[tauri] 语音库与模型准备失败。原生库缺失会让 cargo 构建直接失败，故在此中止；\n' +
        '        可单独重试：node tools/fetch_sherpa_onnx_libs.mjs',
    );
    process.exit(sherpaExitCode);
  }
  stageDone(sherpaStage);

  // 仅 GPU 构建时非空：见下面那段说明，它只在最终 tauri 调用时注入。
  let gpuSherpaLibDir = null;

  if (isGpuBuild) {
    if (!existsSync(sherpaCudaLibDir)) {
      console.error(
        `[tauri] 指定了 gpu 特性，但 CUDA 库目录不存在：${sherpaCudaLibDir}\n` +
          '        请先运行：npm run sherpa:fetch:cuda',
      );
      process.exit(1);
    }
    // SHERPA_ONNX_LIB_DIR 只能给**最终的 tauri 调用**，绝不能挂在 process.env 上：
    // 它会让 sherpa-onnx-sys 去这个目录找原生库，而 cuda-lib 里只有共享库与导入库，
    // 下面那些默认特性（static）的辅助构建会直接报
    // "could not find native static library `sherpa-onnx-core`"。
    gpuSherpaLibDir = process.env.SHERPA_ONNX_LIB_DIR || sherpaCudaLibDir;
    console.log(`[tauri]     SHERPA_ONNX_LIB_DIR=${gpuSherpaLibDir}`);
    if (shouldPrepareForTauriCommand) {
      // 2.7GB 的 CUDA 运行库不能作为 bundle.resources 塞进安装包：连同
      // guaxi.exe 会超过 makensis 数据块的 ~2GB 上限，直接报
      // "error mmapping datablock" 打包失败。这里沿用 utils 的外置分包机制，
      // 构建期压成 tar.zst，安装时由 extract_utils_bundles 解压到 $INSTDIR
      // （guaxi.exe 旁边，Windows 加载器与 onnxruntime 都在那里找 DLL）。
      const cudaStage = stage(
        'CUDA 运行时分包',
        '约 2.8 GB DLL 压成 tar.zst，首次数分钟、之后只压变化的包',
      );
      const prepareArgs = [
        '--source-dir',
        gpuSherpaLibDir,
        '--bundle-dir',
        windowsCudaRuntimeSourceBundlesDir,
      ];
      if (reuseExisting) {
        prepareArgs.push('--reuse-existing');
      }
      // 这个工具要压 2.8GB 数据，debug profile 下 zstd 的 C 代码是 -O0、blake3 也未优化，
      // 实测只有 ~80MB/s；release 构建本身很便宜，能快数倍。build.rs 不必再预处理一遍。
      const prepareExitCode = await runCargoBin(
        'prepare_cuda_runtime_bundle',
        prepareArgs,
        { GUAXI_SKIP_BUNDLE_PREPARE: '1' },
        { label: 'CUDA 运行库压缩' },
      );
      if (prepareExitCode !== 0) {
        process.exit(prepareExitCode);
      }
      stageDone(cudaStage);
    }
  }

  if (shouldPrepareForTauriCommand) {
    // Release packaging depends on pre-copied frontend assets because the final
    // bundle cannot fetch editor/viewer assets from the network at runtime.
    const assetsStage = stage('前端离线资源', 'Excalidraw 资源整包预拷贝');
    const prepareAssetsExitCode = await run(
      'node',
      ['./tools/copy_excalidraw_assets.mjs'],
      {},
      { label: '前端资源拷贝' },
    );
    if (prepareAssetsExitCode !== 0) {
      process.exit(prepareAssetsExitCode);
    }
    stageDone(assetsStage);

    // 浏览器预览页（/browser/preview）的 ```chart 围栏复用软件内同一份图表模块。
    // 这里生成它的单文件产物，产物由 Rust 侧 include_str! 打进二进制；
    // 不重新生成的话，发布包里会带着 build 前那一版（或根本不存在，直接编译失败）。
    const chartStage = stage('图表预览产物', 'Vite + esbuild 打成单文件');
    const prepareChartPreviewExitCode = await run(
      'node',
      ['./tools/build_chart_preview_bundle.mjs'],
      {},
      { label: '图表预览产物生成' },
    );
    if (prepareChartPreviewExitCode !== 0) {
      process.exit(prepareChartPreviewExitCode);
    }
    stageDone(chartStage);

    // Slidev / LSP / RSSHub 的运行时都是内置的 `utils/SlidevEnv/node.exe`。
    // 它是平台二进制、不进 git，干净机器和 CI 上都没有；缺了它，离线包即使带着
    // rsshub/dist 也起不来。这里按固定版本补齐，幂等（已有则秒过）；失败必须中止——
    // 打出一个聚合/Slidev 都启动不了的包，比构建失败更糟。
    const nodeStage = stage('内置 Node 运行时', 'utils/SlidevEnv/node.exe，已有则秒过');
    const nodeRuntimeExitCode = await run('node', ['./tools/fetch_slidev_node.mjs'], {}, { label: 'Node 运行时准备' });
    if (nodeRuntimeExitCode !== 0) {
      console.error(
        '[tauri] 内置 Node 运行时准备失败。Slidev / LSP / RSSHub 都靠它启动，故在此中止；\n' +
          '        可单独重试：node tools/fetch_slidev_node.mjs',
      );
      process.exit(nodeRuntimeExitCode);
    }
    stageDone(nodeStage);

    // RSSHub 进程内集成：把 rsshub 装进 SlidevEnv/node_modules（复用其 node.exe），
    // 必须在 prepare_utils_zip 打包 utils 树之前，装好的 rsshub 才会被一起打进离线包。
    // 幂等（已装则跳过）、不校验退出码（缺失只降级，不阻断 build）。
    const rsshubStage = stage('RSSHub 环境', '缺失只降级聚合功能，不阻断构建');
    await run('node', ['./tools/prepare_rsshub_env.mjs'], {}, { label: 'RSSHub 环境准备' });
    stageDone(rsshubStage);

    // Bundle the heavy `src-tauri/utils` runtime tree into external tar.zst
    // parts before Tauri starts the actual release build.
    //
    // 复用模式（--reuse-utils-zip）下这里刻意什么都不做：`prepare_utils_zip
    // --reuse-existing` 本身只耗时 ~0.2s，但为了跑它要额外起一个 `cargo run`
    // 进程（首次编译后稳定在 ~6s）。而 build.rs 在同一份清单上做的判断完全等价
    // ——调用 tauri 时已经带上 GUAXI_REUSE_UTILS_ZIP=1，它会走同一条
    // 「清单完整就跳过扫描」的快路径。少一次进程、少一处重复判断。
    if (reuseExisting) {
      stageSkipped('utils 外置分包', '--reuse-utils-zip：交给 build.rs 直接信任现有清单');
    } else {
      const utilsStage = stage('utils 外置分包', '扫描 12 万文件并压成 tar.zst');
      console.log(
        '[tauri]     首次或源文件变动时这一步最慢（分钟级）；未变的分包会按摘要自动跳过。',
      );
      const prepareExitCode = await runCargoBin(
        'prepare_utils_zip',
        [],
        {
          // release profile 会触发 build.rs 的 release 分支，这里跳过：
          // 分包正是本进程要做的，重复处理只会把同一批文件压两次。
          GUAXI_SKIP_BUNDLE_PREPARE: '1',
        },
        { label: 'utils 分包压缩' },
      );
      if (prepareExitCode !== 0) {
        process.exit(prepareExitCode);
      }
      stageDone(utilsStage);
    }

    if (isWindows) {
      // The installer invokes this helper during installation to expand the
      // external utils bundles into the final install directory.
      const helperStage = stage('安装期解包 helper', 'extract_utils_bundles.exe，安装时调用');
      const helperBuildExitCode = await ensureCargoBinBuilt(
        'extract_utils_bundles',
        {
          ...releaseCargoEnv(),
          // helper 只是打包用的 bin，同样会触发 build.rs。它的分包要么已由上面
          // 准备好、要么马上由主构建的 build.rs 处理，所以这里无条件跳过准备，
          // 不必再叠加 GUAXI_REUSE_UTILS_ZIP（skip 的判断在 reuse 之前）。
          GUAXI_SKIP_BUNDLE_PREPARE: '1',
        },
        { label: '解包 helper 编译' },
      );
      if (helperBuildExitCode !== 0) {
        process.exit(helperBuildExitCode);
      }
      stageDone(helperStage);
    }

    console.log(
      '[tauri]     下一阶段由 Tauri CLI 接管：前端构建 → rustc release → makensis。\n' +
        '[tauri]     中途出现 "Finished `release` profile" 后又长时间无输出，是 makensis 在\n' +
        '[tauri]     压缩安装包（utils 已外置，通常 1–3 分钟），属正常现象。',
    );
  }

  const tauri = tauriInvocation();
  const tauriIsBuild = shouldPrepareForTauriCommand;
  const tauriTitle = tauriIsBuild ? 'Tauri 构建与打包' : `Tauri ${tauriSubcommand || 'CLI'}`;
  const tauriDetail = tauriIsBuild
    ? 'vite + rustc + NSIS，通常 5–15 分钟'
    : tauriSubcommand === 'dev'
      ? '首次编译依赖较慢，之后热重载'
      : '执行 CLI 子命令';
  const tauriStage = stage(tauriTitle, tauriDetail);
  const tauriStartedAt = Date.now();
  const buildPhases = tauriIsBuild
    ? [
        {
          label: '前端构建（vite）',
          probe: () => fileMtime(path.join(projectRoot, 'dist', 'index.html')),
        },
        {
          label: 'rustc 编译 + 链接',
          probe: () =>
            fileMtime(
              path.join(cargoReleaseDir, isWindows ? 'guaxi.exe' : 'guaxi'),
            ),
        },
        {
          label: 'NSIS 生成安装包',
          probe: () => newestExeMtime(windowsNsisOutputDir),
        },
      ]
    : [];
  const phaseTracker =
    buildPhases.length > 0 ? trackBuildPhases(buildPhases, tauriStartedAt) : null;
  const tauriExitCode = await run(
    tauri.command,
    tauri.args,
    // The wrapper already prepared/validated the bundles above. Tell build.rs
    // to trust that manifest instead of scanning the 120k-file utils tree a
    // second time; a direct `cargo build --release` still uses the incremental
    // path because this env var is absent there.
    {
      ...cargoTargetEnv(),
      // 只在 build / bundle 时套 release 环境：`CARGO_INCREMENTAL=0` 一旦带到 dev，
      // 每改一行 Rust 都要把 20 万行的 crate 从头 codegen 一遍（实测 19 分钟），
      // dev 的意义就没了。dev 用 cargo 自己的默认值：增量开、并行吃满所有核。
      ...(shouldPrepareForTauriCommand ? releaseCargoEnv() : {}),
      ...(shouldPrepareForTauriCommand ? { GUAXI_REUSE_UTILS_ZIP: '1' } : {}),
      // GPU 构建才注入，且只注入到这一层：tauri 拉起的 cargo 会继承它，
      // 而上面那些辅助构建不会。
      ...(gpuSherpaLibDir ? { SHERPA_ONNX_LIB_DIR: gpuSherpaLibDir } : {}),
    },
    {
      label: tauriTitle,
      // dev 是常驻会话，不是「有始有终的构建步骤」：心跳会每 20s 刷一行，一直刷到关窗口，
      // 还会跟 cargo 的 `Building [====>]` 进度条互相撕行。cargo 自己就会持续输出编译进度，
      // 启动后应用日志也在滚动，这里不需要心跳来证明「没卡死」。
      heartbeatMs: shouldPrepareForTauriCommand ? HEARTBEAT_MS : 0,
      heartbeatNote: phaseTracker
        ? () =>
            `当前：${phaseTracker.currentLabel() ?? '收尾'}（已 ${formatDuration(phaseTracker.currentElapsedMs())}）`
        : undefined,
    },
  );
  await phaseTracker?.stop();
  if (tauriExitCode !== 0) {
    process.exit(tauriExitCode);
  }
  stageDone(tauriStage);

  if (shouldPrepareForTauriCommand && (tauriSubcommand === 'build' || tauriSubcommand === 'bundle')) {
    if (isGpuBuild) {
      const verifyStage = stage('CUDA 运行时自检', '检查安装目录旁的 DLL 能否加载');
      await verifyCudaRuntimeForRelease();
      stageDone(verifyStage);
    }
    if (isWindows) {
      const publishStage = stage('发布外置分包', '把分包与 helper 摆到安装包同级目录');
      await publishExternalWindowsBundles(isGpuBuild);
      stageDone(publishStage);
    } else {
      // publishExternalWindowsBundles 在非 Windows 直接返回；Linux 走 resources 内嵌。
      stageSkipped('发布外置分包', '仅 Windows 走外置分包；Linux 把分包作为 resources 内嵌');
    }
  }

  printSummary();
  process.exit(tauriExitCode);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  console.error(`[tauri] 失败于第 ${stageIndex}/${stageTotal} 步（见上面的阶段头）`);
  printSummary();
  process.exit(1);
});

