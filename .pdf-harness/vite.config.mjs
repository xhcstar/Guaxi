import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const dir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(dir, '..')

// 只为在浏览器里单挂 GuaxiPdfViewer + ReaderPane 的 PDF 布局用；不参与应用构建。
export default defineConfig({
  root: dir,
  plugins: [vue()],
  publicDir: path.join(dir, 'public'),
  server: { port: 8932, strictPort: true, fs: { allow: [repoRoot] } },
})
