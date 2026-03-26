import { spawn } from 'node:child_process';

const args = process.argv.slice(2);
const reuseFlag = '--reuse-utils-zip';
const reuseExisting = args.includes(reuseFlag);
const tauriArgs = args.filter((arg) => arg !== reuseFlag);
const tauriSubcommand = tauriArgs[0] ?? '';
const shouldPrepareForTauriCommand =
  tauriSubcommand === 'build' || tauriSubcommand === 'bundle';

function tauriCommand() {
  return process.platform === 'win32'
    ? '.\\node_modules\\.bin\\tauri.cmd'
    : './node_modules/.bin/tauri';
}

function run(command, commandArgs, extraEnv = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, commandArgs, {
      cwd: process.cwd(),
      stdio: 'inherit',
      shell: false,
      env: {
        ...process.env,
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
  }

  const tauriExitCode = await run(
    tauriCommand(),
    tauriArgs,
    reuseExisting ? { GUAXI_REUSE_UTILS_ZIP: '1' } : {},
  );
  process.exit(tauriExitCode);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
