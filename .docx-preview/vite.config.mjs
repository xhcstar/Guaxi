import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import path from 'node:path'
const repoRoot = path.resolve(__dirname, '..')
export default defineConfig({
  root: __dirname,
  plugins: [vue()],
  resolve: { alias: { '@': path.resolve(repoRoot, 'src'), vue: path.resolve(repoRoot, 'node_modules/vue') } },
  server: { port: 8981, strictPort: true, host: true, fs: { allow: [repoRoot] } },
})
