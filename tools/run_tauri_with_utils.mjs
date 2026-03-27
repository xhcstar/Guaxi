import { cp, mkdir, readdir, rm, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { spawn } from 'node:child_process';

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

async function publishExternalWindowsBundles() {
  if (process.platform !== 'win32') {
    return;
  }

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
  if (shouldPrepareForTauriCommand) {
    const prepareAssetsExitCode = await run('node', ['./tools/copy_excalidraw_assets.mjs']);
    if (prepareAssetsExitCode !== 0) {
      process.exit(prepareAssetsExitCode);
    }

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
      const helperBuildExitCode = await run('cargo', [
        'build',
        '--quiet',
        '--manifest-path',
        'src-tauri/Cargo.toml',
        '--release',
        '--bin',
        'extract_utils_bundles',
      ]);
      if (helperBuildExitCode !== 0) {
        process.exit(helperBuildExitCode);
      }
    }
  }

  const tauri = tauriInvocation();
  const tauriExitCode = await run(
    tauri.command,
    tauri.args,
    reuseExisting ? { GUAXI_REUSE_UTILS_ZIP: '1' } : {},
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
