import { existsSync } from 'node:fs';
import { delimiter, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const packageName = process.env.GUAXI_CAPTURE_ANDROID_PACKAGE || 'com.guaxi.capture';
const serialFromEnv = process.env.GUAXI_ANDROID_SERIAL || process.env.ANDROID_SERIAL || '';
const command = process.argv[2] || 'diagnose';

function adbCandidates() {
  const names = process.platform === 'win32' ? ['adb.exe', 'adb'] : ['adb'];
  const roots = [
    process.env.ANDROID_HOME,
    process.env.ANDROID_SDK_ROOT,
  ].filter(Boolean);

  const sdkCandidates = roots.flatMap((root) => names.map((name) => join(root, 'platform-tools', name)));
  const pathCandidates = (process.env.PATH || '')
    .split(delimiter)
    .filter(Boolean)
    .flatMap((dir) => names.map((name) => join(dir, name)));

  return [...sdkCandidates, ...pathCandidates, ...names];
}

function findAdb() {
  for (const candidate of adbCandidates()) {
    if (candidate.includes('\\') || candidate.includes('/')) {
      if (!existsSync(candidate)) continue;
    }

    const result = spawnSync(candidate, ['version'], {
      encoding: 'utf8',
      windowsHide: true,
    });

    if (result.status === 0) {
      return candidate;
    }
  }

  console.error('[capture-android] adb not found. Set ANDROID_HOME or add platform-tools to PATH.');
  process.exit(1);
}

const adb = findAdb();

function runAdb(args, options = {}) {
  const fullArgs = [];
  if (options.withSerial !== false && currentSerial) {
    fullArgs.push('-s', currentSerial);
  }
  fullArgs.push(...args);

  console.log(`[capture-android] adb ${fullArgs.join(' ')}`);
  return spawnSync(adb, fullArgs, {
    encoding: 'utf8',
    stdio: options.inherit ? 'inherit' : 'pipe',
    windowsHide: true,
  });
}

function parseDevices(output) {
  return output
    .split(/\r?\n/)
    .slice(1)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [serial, state] = line.split(/\s+/);
      return { serial, state, raw: line };
    })
    .filter((device) => device.state === 'device');
}

function listDevices() {
  const result = spawnSync(adb, ['devices', '-l'], {
    encoding: 'utf8',
    windowsHide: true,
  });

  console.log(result.stdout.trim() || '[capture-android] no adb device output');
  if (result.stderr?.trim()) {
    console.error(result.stderr.trim());
  }

  return parseDevices(result.stdout || '');
}

function resolveSerial() {
  if (serialFromEnv) {
    return serialFromEnv;
  }

  const devices = listDevices();
  if (devices.length === 1) {
    return devices[0].serial;
  }

  if (devices.length > 1) {
    console.error('[capture-android] multiple Android devices found. Set GUAXI_ANDROID_SERIAL to one serial.');
  } else {
    console.error('[capture-android] no online Android device found. Check USB debugging and adb authorization.');
  }

  return '';
}

let currentSerial = '';
if (!['devices'].includes(command)) {
  currentSerial = resolveSerial();
  if (!currentSerial) {
    process.exit(1);
  }
}

function printResult(result) {
  if (result.stdout?.trim()) console.log(result.stdout.trim());
  if (result.stderr?.trim()) console.error(result.stderr.trim());
  return result.status ?? 1;
}

function uninstall({ ignoreFail = false } = {}) {
  // Design note:
  // `Failure [-99]` happens after the APK is built, during device-side install.
  // A stale install with another signature/user state is a common trigger, so
  // this cleanup removes both the normal package and the current Android user copy.
  console.log(`[capture-android] cleaning installed package: ${packageName}`);

  const normal = runAdb(['uninstall', packageName]);
  printResult(normal);

  const user = runAdb(['shell', 'pm', 'uninstall', '--user', '0', packageName]);
  printResult(user);

  const ok = normal.status === 0 || user.status === 0;
  if (!ok && !ignoreFail) {
    console.error('[capture-android] uninstall did not report success. It may already be absent.');
    process.exit(1);
  }
}

function diagnose() {
  console.log(`[capture-android] adb: ${adb}`);
  console.log(`[capture-android] serial: ${currentSerial}`);

  const checks = [
    ['shell', 'getprop', 'ro.product.manufacturer'],
    ['shell', 'getprop', 'ro.product.model'],
    ['shell', 'getprop', 'ro.build.version.sdk'],
    ['shell', 'pm', 'list', 'packages', packageName],
    ['shell', 'pm', 'path', packageName],
  ];

  for (const args of checks) {
    printResult(runAdb(args));
  }
}

function freshDev() {
  uninstall({ ignoreFail: true });

  const sharePatch = resolve('scripts', 'configure-android-share-target.mjs');
  console.log('[capture-android] patching Android share target before dev');
  const patchResult = spawnSync(process.execPath, [sharePatch, '--optional'], {
    stdio: 'inherit',
    windowsHide: true,
  });
  if ((patchResult.status ?? 1) !== 0) {
    process.exit(patchResult.status ?? 1);
  }

  const tauriCli = resolve('..', '..', 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
  console.log('[capture-android] starting Tauri Android dev after cleanup');
  const result = spawnSync(process.execPath, [tauriCli, 'android', 'dev'], {
    stdio: 'inherit',
    windowsHide: true,
  });
  process.exit(result.status ?? 1);
}

switch (command) {
  case 'devices':
    listDevices();
    break;
  case 'diagnose':
    diagnose();
    break;
  case 'uninstall':
    uninstall();
    break;
  case 'fresh-dev':
    freshDev();
    break;
  default:
    console.error(`[capture-android] unknown command: ${command}`);
    console.error('[capture-android] use one of: devices, diagnose, uninstall, fresh-dev');
    process.exit(1);
}
