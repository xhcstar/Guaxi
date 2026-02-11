import fs from 'node:fs/promises'
import path from 'node:path'

const log = (...args) => console.log('[copy_excalidraw_assets]', ...args)
const warn = (...args) => console.warn('[copy_excalidraw_assets]', ...args)

const PROJECT_ROOT = process.cwd()

// Excalidraw v0.18.x：字体在 dist/prod/fonts
const SRC_FONTS_DIR = path.join(
  PROJECT_ROOT,
  'node_modules',
  '@excalidraw',
  'excalidraw',
  'dist',
  'prod',
  'fonts',
)

// 复制到 public/excalidraw/fonts，并配合 index.html 的 EXCALIDRAW_ASSET_PATH 使用
const DEST_ROOT = path.join(PROJECT_ROOT, 'public', 'excalidraw')
const DEST_FONTS_DIR = path.join(DEST_ROOT, 'fonts')

async function pathExists(p) {
  try {
    await fs.access(p)
    return true
  } catch {
    return false
  }
}

async function main() {
  log('start', { SRC_FONTS_DIR, DEST_FONTS_DIR })

  if (!(await pathExists(SRC_FONTS_DIR))) {
    warn('source fonts dir not found, skip copy', { SRC_FONTS_DIR })
    return
  }

  await fs.mkdir(DEST_FONTS_DIR, { recursive: true })

  await fs.cp(SRC_FONTS_DIR, DEST_FONTS_DIR, {
    recursive: true,
    force: true,
  })

  log('done')
}

main().catch((e) => {
  console.error('[copy_excalidraw_assets] failed:', e)
  process.exitCode = 1
})