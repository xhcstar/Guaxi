import { networkInterfaces } from 'node:os';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const port = process.env.PORT || '1430';
const viteCli = resolve('..', '..', 'node_modules', 'vite', 'bin', 'vite.js');

function detectLanHost() {
  const addresses = [];
  for (const entries of Object.values(networkInterfaces())) {
    for (const entry of entries || []) {
      if (entry.family !== 'IPv4' || entry.internal) continue;
      if (entry.address.startsWith('169.254.')) continue;
      addresses.push(entry.address);
    }
  }

  const preferred = addresses.find((address) => address.startsWith('192.168.'))
    || addresses.find((address) => address.startsWith('10.'))
    || addresses.find((address) => address.startsWith('172.'))
    || addresses[0]
    || '';

  return preferred;
}

// Design note:
// Tauri Android rewrites devUrl to the PC LAN IP, but Vite HMR can still
// advertise localhost to the Android WebView. That causes a blank-looking app
// with console websocket errors. We keep Vite listening on all interfaces and
// explicitly tell the HMR client to use the detected LAN address.
const hmrHost = process.env.VITE_HMR_HOST || process.env.TAURI_DEV_HOST || detectLanHost();
const env = {
  ...process.env,
  VITE_HMR_HOST: hmrHost,
  VITE_GUAXI_PC_HOST: process.env.VITE_GUAXI_PC_HOST || hmrHost,
};

console.info('[capture-dev] starting Vite dev server', {
  host: '0.0.0.0',
  port,
  hmrHost: hmrHost || '(auto)',
  guaxiPcHost: env.VITE_GUAXI_PC_HOST || '(manual)',
});

const result = spawnSync(process.execPath, [viteCli, '--host', '0.0.0.0', '--port', port], {
  stdio: 'inherit',
  env,
  windowsHide: true,
});

process.exit(result.status ?? 1);
