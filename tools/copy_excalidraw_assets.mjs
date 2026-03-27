import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const log = (...args) => console.log('[copy_excalidraw_assets]', ...args)
const warn = (...args) => console.warn('[copy_excalidraw_assets]', ...args)

// 以脚本所在目录定位项目根，避免从 tools/ 下运行时路径偏移
const __dirname = path.dirname(fileURLToPath(import.meta.url))
const PROJECT_ROOT = path.resolve(__dirname, '..')

// 用 Excalidraw 的 dev 版资产，避免 prod 包内联自己的 React 造成双 React
const SRC_ASSETS_DIR = path.join(
  PROJECT_ROOT,
  'node_modules',
  '@excalidraw',
  'excalidraw',
  'dist',
  'dev',
)

// 复制到 public/excalidraw，与 EXCALIDRAW_ASSET_PATH 对齐
const DEST_ROOT = path.join(PROJECT_ROOT, 'public', 'excalidraw')
const SRC_VDITOR_DIST_DIR = path.join(PROJECT_ROOT, 'node_modules', 'vditor', 'dist')
const DEST_VDITOR_ROOT = path.join(PROJECT_ROOT, 'public', 'vditor')
const DEST_VDITOR_DIST_DIR = path.join(DEST_VDITOR_ROOT, 'dist')

async function pathExists(p) {
  try {
    await fs.access(p)
    return true
  } catch {
    return false
  }
}

async function main() {
  log('start', { SRC_ASSETS_DIR, DEST_ROOT, SRC_VDITOR_DIST_DIR, DEST_VDITOR_ROOT, DEST_VDITOR_DIST_DIR })

  if (!(await pathExists(SRC_ASSETS_DIR))) {
    warn('source assets dir not found, skip copy', { SRC_ASSETS_DIR })
  } else {
    await fs.mkdir(DEST_ROOT, { recursive: true })
    await fs.cp(SRC_ASSETS_DIR, DEST_ROOT, {
      recursive: true,
      force: true,
    })
  }

  if (!(await pathExists(SRC_VDITOR_DIST_DIR))) {
    warn('vditor dist dir not found, skip copy', { SRC_VDITOR_DIST_DIR })
  } else {
    // Vditor internally resolves runtime assets as `${cdn}/dist/...`,
    // so we must preserve the `dist` layer under `public/vditor/dist`.
    await fs.mkdir(DEST_VDITOR_DIST_DIR, { recursive: true })
    await fs.cp(SRC_VDITOR_DIST_DIR, DEST_VDITOR_DIST_DIR, {
      recursive: true,
      force: true,
    })
  }

  log('done')
}

main().catch((e) => {
  console.error('[copy_excalidraw_assets] failed:', e)
  process.exitCode = 1
})
