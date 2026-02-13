import fs from 'node:fs/promises'
import path from 'node:path'

const log = (...args) => console.log('[copy_excalidraw_assets]', ...args)
const warn = (...args) => console.warn('[copy_excalidraw_assets]', ...args)

const PROJECT_ROOT = process.cwd()

// Excalidraw v0.18.x：生产资源在 dist/prod
const SRC_ASSETS_DIR = path.join(
  PROJECT_ROOT,
  'node_modules',
  '@excalidraw',
  'excalidraw',
  'dist',
  'prod',
)

// 复制到 public/excalidraw，并配合 index.html 的 EXCALIDRAW_ASSET_PATH 使用
const DEST_ROOT = path.join(PROJECT_ROOT, 'public', 'excalidraw')

async function pathExists(p) {
  try {
    await fs.access(p)
    return true
  } catch {
    return false
  }
}

async function main() {
  log('start', { SRC_ASSETS_DIR, DEST_ROOT })

  if (!(await pathExists(SRC_ASSETS_DIR))) {
    warn('source assets dir not found, skip copy', { SRC_ASSETS_DIR })
    return
  }

  await fs.mkdir(DEST_ROOT, { recursive: true })

  await fs.cp(SRC_ASSETS_DIR, DEST_ROOT, {
    recursive: true,
    force: true,
  })

  log('done')
}

main().catch((e) => {
  console.error('[copy_excalidraw_assets] failed:', e)
  process.exitCode = 1
})