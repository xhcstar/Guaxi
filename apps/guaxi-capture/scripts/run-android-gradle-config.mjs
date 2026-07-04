import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, isAbsolute, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const shellScript = resolve(scriptDir, 'configure-android-gradle.sh');
const forwardedArgs = process.argv.slice(2);

// 设计说明：
// Windows 上的 bash 可能指向 WSL 启动器；如果机器没有安装 WSL 发行版，
// 直接在 package.json 里执行 bash 会失败。这里先探测可用 Bash，再运行真正的 shell 脚本。
const bashCandidates = [
  process.env.GUAXI_BASH,
  'bash',
  'C:\\Program Files\\Git\\bin\\bash.exe',
  'C:\\Program Files (x86)\\Git\\bin\\bash.exe',
  'D:\\Git\\bin\\bash.exe',
].filter(Boolean);

function canUseBash(command) {
  if (isAbsolute(command) && !existsSync(command)) {
    return false;
  }

  const result = spawnSync(command, ['--version'], {
    encoding: 'utf8',
    windowsHide: true,
  });

  if (result.status !== 0) {
    return false;
  }

  return /bash/i.test(`${result.stdout}\n${result.stderr}`);
}

const bash = bashCandidates.find(canUseBash);

if (!bash) {
  console.error('[capture-android] no usable Bash found.');
  console.error('[capture-android] Install Git for Windows, or set GUAXI_BASH to your bash.exe path.');
  process.exit(1);
}

console.log(`[capture-android] using Bash: ${bash}`);

const result = spawnSync(bash, [shellScript, ...forwardedArgs], {
  stdio: 'inherit',
  windowsHide: true,
});

process.exit(result.status ?? 1);
