import { defineConfig } from 'vite'

// Android/iOS 真机调试时，Tauri 会把 devUrl host 替换成电脑局域网 IP。
// Vite 必须监听 0.0.0.0 或 TAURI_DEV_HOST，否则手机访问不到 192.168.x.x:1430。
export default defineConfig({
  clearScreen: false,
  server: {
    host: process.env.TAURI_DEV_HOST || '0.0.0.0',
    port: 1430,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: false,
    sourcemap: true,
  },
})
