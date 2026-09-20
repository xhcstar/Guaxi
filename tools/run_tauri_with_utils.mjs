import { cp, mkdir, readdir, rm, stat, writeFile } from 'node:fs/promises';
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

async function publishExternalWindowsBundles() {
  if (process.platform !== 'win32') {
    return;
  }

  // Windows installers do not embed the large utils runtime payload anymore.
  // Instead, release builds publish external bundle files next to the setup EXE,
  // and the NSIS post-install hook expands them into `$INSTDIR\\utils`.
  let sourceEntries = [];
  try {
    sourceEntries = await readdir(utilsBundlesSourceDir, { withFileTypes: true });
  } catch (error) {
    throw new Error(`读取 utils 分包目录失败: ${utilsBundlesSourceDir} (${error.message})`);
  }

  const zipEntries = [];
  for (const entry of sourceEntries) {
    if (
      !entry.isFile() ||
      !entry.name.toLowerCase().endsWith('.tar.zst')
    ) {
      continue;
    }
    const fullPath = path.join(utilsBundlesSourceDir, entry.name);
    const fileStat = await stat(fullPath);
    zipEntries.push({ name: entry.name, fullPath, size: fileStat.size });
  }

  if (zipEntries.length === 0) {
    throw new Error(`未找到可发布的 utils 分包: ${utilsBundlesSourceDir}`);
  }

  await mkdir(windowsNsisOutputDir, { recursive: true });
  await rm(windowsExternalBundlesDir, { recursive: true, force: true });
  await mkdir(windowsExternalBundlesDir, { recursive: true });

  for (const entry of zipEntries) {
    await cp(entry.fullPath, path.join(windowsExternalBundlesDir, entry.name));
  }
  await cp(windowsExtractHelperSource, windowsExtractHelperOutput);

  const totalBytes = zipEntries.reduce((sum, entry) => sum + entry.size, 0);
  const readme = [
    'Keep "utils_bundles" and "extract_utils_bundles.exe" next to the Guaxi setup executable.',
    'The installer reads these external tar.zst bundle files during installation and expands them into the installed utils directory.',
    '',
    `Bundle count: ${zipEntries.length}`,
    `Total bytes: ${totalBytes}`,
    '',
    ...zipEntries.map((entry) => `${entry.name}\t${entry.size}`),
    '',
  ].join('\n');
  await writeFile(windowsExternalReadme, readme, 'utf8');

  console.log(
    `[utils.bundle] published external windows bundles dir=${windowsExternalBundlesDir} bundleCount=${zipEntries.length} totalBytes=${totalBytes}`,
  );
}

async function main() {
  // Codex Agent 引擎依赖 `src-tauri/utils/codex/codex.exe`。这一步是幂等的（已存在即跳过），
  // 并且刻意不校验退出码：拿不到 codex 时智能助手会回退到内置 Python Agent，
  // 不应该因为下载失败就阻断 dev / build。
  await run('node', ['./tools/fetch_codex_sidecar.mjs']);

  if (shouldPrepareForTauriCommand) {
    // Release packaging depends on pre-copied frontend assets because the final
    // bundle cannot fetch editor/viewer assets from the network at runtime.
    const prepareAssetsExitCode = await run('node', ['./tools/copy_excalidraw_assets.mjs']);
    if (prepareAssetsExitCode !== 0) {
      process.exit(prepareAssetsExitCode);
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
    // `build.rs` also knows how to reuse existing bundles. We forward the flag
    // through an env var so Cargo-triggered bundle preparation stays consistent
    // with the outer JS wrapper.
    {
      ...releaseCargoEnv(),
      ...(reuseExisting ? { GUAXI_REUSE_UTILS_ZIP: '1' } : {}),
    },
  );
  if (tauriExitCode !== 0) {
    process.exit(tauriExitCode);
  }

  if (shouldPrepareForTauriCommand && (tauriSubcommand === 'build' || tauriSubcommand === 'bundle')) {
    await publishExternalWindowsBundles();
  }

  process.exit(tauriExitCode);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
