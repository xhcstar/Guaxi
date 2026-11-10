import { copyFile, mkdir, readdir, readFile, rm, stat, utimes, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
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
const utilsBundlesSourceDir = path.join(projectRoot, 'src-tauri', 'utils', '__bundles');
const windowsNsisOutputDir = path.join(
  projectRoot,
  'src-tauri',
  'target',
  'release',
  'bundle',
  'nsis',
);
const windowsExternalBundlesDir = path.join(windowsNsisOutputDir, 'utils_bundles');
const windowsExternalReadme = path.join(windowsNsisOutputDir, 'README_utils_bundles.txt');
const windowsExtractHelperSource = path.join(
  projectRoot,
  'src-tauri',
  'target',
  'release',
  'extract_utils_bundles.exe',
);
const windowsExtractHelperOutput = path.join(
  windowsNsisOutputDir,
  'extract_utils_bundles.exe',
);
const windowsCudaRuntimeSourceBundlesDir = path.join(
  projectRoot,
  'src-tauri',
  'target',
  'cuda-runtime-bundles',
);
const windowsCudaRuntimeBundlesDir = path.join(
  windowsNsisOutputDir,
  'cuda_runtime_bundles',
);
const BUNDLE_MANIFEST_FILE = 'bundle-manifest.json';

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

  const releaseDir = path.join(projectRoot, 'src-tauri', 'target', 'release');
  if (!existsSync(releaseDir)) {
    console.warn(`[tauri] 未找到 release 产物目录，跳过 CUDA 自检：${releaseDir}`);
    return;
  }

  const buildExitCode = await run('cargo', [
    'build',
    '--quiet',
    '--manifest-path',
    'src-tauri/Cargo.toml',
    '--bin',
    'verify_cuda_runtime',
  ]);
  if (buildExitCode !== 0) {
    console.warn('[tauri] CUDA 自检工具构建失败，跳过自检。');
    return;
  }

  const verifier = path.join(projectRoot, 'src-tauri', 'target', 'debug', 'verify_cuda_runtime.exe');
  if (!existsSync(verifier)) {
    console.warn(`[tauri] 未找到 CUDA 自检工具，跳过自检：${verifier}`);
    return;
  }

  console.log('[tauri] GPU 版自检：检查安装目录旁的 CUDA 运行库能否加载 …');
  const exitCode = await run(verifier, ['--dir', releaseDir]);
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

function run(command, commandArgs, extraEnv = {}) {
  return new Promise((resolve, reject) => {
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

    child.on('exit', (code, signal) => {
      if (signal) {
        reject(new Error(`${command} terminated by signal ${signal}`));
        return;
      }
      resolve(code ?? 0);
    });

    child.on('error', reject);
  });
}

function releaseCargoEnv() {
  if (process.platform !== 'win32') {
    return {};
  }

  return {
    // Windows release builds for this crate are memory-heavy. Serializing the
    // build and disabling incremental state makes rustc/LLVM noticeably less
    // likely to run out of memory on packaging machines.
    CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '1',
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
}

async function main() {
  // Codex Agent 引擎依赖 `src-tauri/utils/codex/codex.exe`。这一步是幂等的（已存在即跳过），
  // 并且刻意不校验退出码：拿不到 codex 时智能助手会回退到内置 Python Agent，
  // 不应该因为下载失败就阻断 dev / build。
  await run('node', ['./tools/fetch_codex_sidecar.mjs']);

  // 语音功能的原生库与模型。必须放在这里（而不是 only-in-build 分支里），原因有两条：
  //   · 原生库要赶在**任何** cargo 构建之前就位——sherpa-onnx 的 build.rs 找不到归档时
  //     会自己去联网下载，而只有系统代理（注册表）的环境下会直接卡住；
  //   · 模型要赶在 prepare_utils_zip 打包之前就位，否则不会被打进离线包。
  // 脚本自身幂等（已存在即跳过），所以 dev 下重复执行没有代价。
  const sherpaExitCode = await run('node', ['./tools/fetch_sherpa_onnx_libs.mjs']);
  if (sherpaExitCode !== 0) {
    console.error(
      '[tauri] 语音库与模型准备失败。原生库缺失会让 cargo 构建直接失败，故在此中止；\n' +
        '        可单独重试：node tools/fetch_sherpa_onnx_libs.mjs',
    );
    process.exit(sherpaExitCode);
  }

  // 仅 GPU 构建时非空：见下面那段说明，它只在最终 tauri 调用时注入。
  let gpuSherpaLibDir = null;

  if (usesGpuFeature(tauriArgs)) {
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
    console.log(`[tauri] GPU 构建：SHERPA_ONNX_LIB_DIR=${gpuSherpaLibDir}`);
    if (shouldPrepareForTauriCommand) {
      // 2.7GB 的 CUDA 运行库不能作为 bundle.resources 塞进安装包：连同
      // guaxi.exe 会超过 makensis 数据块的 ~2GB 上限，直接报
      // "error mmapping datablock" 打包失败。这里沿用 utils 的外置分包机制，
      // 构建期压成 tar.zst，安装时由 extract_utils_bundles 解压到 $INSTDIR
      // （guaxi.exe 旁边，Windows 加载器与 onnxruntime 都在那里找 DLL）。
      const prepareArgs = [
        'run',
        '--quiet',
        '--manifest-path',
        'src-tauri/Cargo.toml',
        '--bin',
        'prepare_cuda_runtime_bundle',
        '--',
        '--source-dir',
        gpuSherpaLibDir,
        '--bundle-dir',
        windowsCudaRuntimeSourceBundlesDir,
      ];
      if (reuseExisting) {
        prepareArgs.push('--reuse-existing');
      }
      const prepareExitCode = await run('cargo', prepareArgs);
      if (prepareExitCode !== 0) {
        process.exit(prepareExitCode);
      }
    }
  }

  if (shouldPrepareForTauriCommand) {
    // Release packaging depends on pre-copied frontend assets because the final
    // bundle cannot fetch editor/viewer assets from the network at runtime.
    const prepareAssetsExitCode = await run('node', ['./tools/copy_excalidraw_assets.mjs']);
    if (prepareAssetsExitCode !== 0) {
      process.exit(prepareAssetsExitCode);
    }

    // 浏览器预览页（/browser/preview）的 ```chart 围栏复用软件内同一份图表模块。
    // 这里生成它的单文件产物，产物由 Rust 侧 include_str! 打进二进制；
    // 不重新生成的话，发布包里会带着 build 前那一版（或根本不存在，直接编译失败）。
    const prepareChartPreviewExitCode = await run('node', [
      './tools/build_chart_preview_bundle.mjs',
    ]);
    if (prepareChartPreviewExitCode !== 0) {
      process.exit(prepareChartPreviewExitCode);
    }

    // RSSHub 进程内集成：把 rsshub 装进 SlidevEnv/node_modules（复用其 node.exe），
    // 必须在 prepare_utils_zip 打包 utils 树之前，装好的 rsshub 才会被一起打进离线包。
    // 幂等（已装则跳过）、不校验退出码（缺失只降级，不阻断 build）。
    await run('node', ['./tools/prepare_rsshub_env.mjs']);

    // Bundle the heavy `src-tauri/utils` runtime tree into external tar.zst
    // parts before Tauri starts the actual release build.
    const prepareArgs = [
      'run',
      '--quiet',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--bin',
      'prepare_utils_zip',
    ];

    if (reuseExisting) {
      prepareArgs.push('--', '--reuse-existing');
    }

    const prepareExitCode = await run('cargo', prepareArgs);
    if (prepareExitCode !== 0) {
      process.exit(prepareExitCode);
    }

    if (process.platform === 'win32') {
      // The installer invokes this helper during installation to expand the
      // external utils bundles into the final install directory.
      const helperBuildExitCode = await run(
        'cargo',
        [
          'build',
          '--quiet',
          '--manifest-path',
          'src-tauri/Cargo.toml',
          '--release',
          '--bin',
          'extract_utils_bundles',
        ],
        {
          ...releaseCargoEnv(),
          GUAXI_SKIP_BUNDLE_PREPARE: '1',
          ...(reuseExisting ? { GUAXI_REUSE_UTILS_ZIP: '1' } : {}),
        },
      );
      if (helperBuildExitCode !== 0) {
        process.exit(helperBuildExitCode);
      }
    }
  }

  const tauri = tauriInvocation();
  const tauriExitCode = await run(
    tauri.command,
    tauri.args,
    // The wrapper already prepared/validated the bundles above. Tell build.rs
    // to trust that manifest instead of scanning the 120k-file utils tree a
    // second time; a direct `cargo build --release` still uses the incremental
    // path because this env var is absent there.
    {
      ...releaseCargoEnv(),
      ...(shouldPrepareForTauriCommand ? { GUAXI_REUSE_UTILS_ZIP: '1' } : {}),
      // GPU 构建才注入，且只注入到这一层：tauri 拉起的 cargo 会继承它，
      // 而上面那些辅助构建不会。
      ...(gpuSherpaLibDir ? { SHERPA_ONNX_LIB_DIR: gpuSherpaLibDir } : {}),
    },
  );
  if (tauriExitCode !== 0) {
    process.exit(tauriExitCode);
  }

  if (shouldPrepareForTauriCommand && (tauriSubcommand === 'build' || tauriSubcommand === 'bundle')) {
    const isGpuBuild = usesGpuFeature(tauriArgs);
    if (isGpuBuild) {
      await verifyCudaRuntimeForRelease();
    }
    await publishExternalWindowsBundles(isGpuBuild);
  }

  process.exit(tauriExitCode);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
