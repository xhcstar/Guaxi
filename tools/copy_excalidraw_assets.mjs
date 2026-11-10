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

async function collectFiles(dir, root, files, targetDir = null) {
  for (const entry of await fs.readdir(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name)
    const relative = path.relative(root, fullPath)
    if (entry.isDirectory()) {
      if (targetDir) {
        await fs.mkdir(path.join(targetDir, relative), { recursive: true })
      }
      await collectFiles(fullPath, root, files, targetDir)
    } else if (entry.isFile()) {
      files.push(relative)
    }
  }
}

async function copyFileIfChanged(sourcePath, targetPath) {
  const sourceStat = await fs.stat(sourcePath)
  let targetStat = null
  try {
    targetStat = await fs.stat(targetPath)
  } catch {
    targetStat = null
  }

  if (targetStat?.isFile() && targetStat.size === sourceStat.size) {
    // 尺寸相同且 mtime 也一致 → 直接跳过。
    if (Math.abs(targetStat.mtimeMs - sourceStat.mtimeMs) < 2) {
      return false
    }
    // 尺寸相同但 mtime 不同：可能是 `git checkout` 重新写了工作区文件。
    // 这种情况下必须比一次内容，否则会无谓地重写几百个字节完全相同的文件，
    // 并把 mtime 改回 node_modules 的时间，反复扰动 git 的 stat 缓存。
    const [sourceData, targetData] = await Promise.all([
      fs.readFile(sourcePath),
      fs.readFile(targetPath),
    ])
    if (sourceData.equals(targetData)) {
      return false
    }
  }

  await fs.mkdir(path.dirname(targetPath), { recursive: true })
  await fs.copyFile(sourcePath, targetPath)
  // Preserve mtime so the next run can skip unchanged files.
  await fs.utimes(targetPath, sourceStat.atime, sourceStat.mtime)
  return true
}

async function syncDirectory(sourceDir, targetDir, label) {
  const sourceFiles = []
  await fs.mkdir(targetDir, { recursive: true })
  await collectFiles(sourceDir, sourceDir, sourceFiles, targetDir)

  let copied = 0
  let skipped = 0
  for (const relative of sourceFiles) {
    const changed = await copyFileIfChanged(
      path.join(sourceDir, relative),
      path.join(targetDir, relative),
    )
    if (changed) {
      copied += 1
    } else {
      skipped += 1
    }
  }

  log('synced', { label, files: sourceFiles.length, copied, skipped })
}

async function main() {
  log('start', { SRC_ASSETS_DIR, DEST_ROOT, SRC_VDITOR_DIST_DIR, DEST_VDITOR_DIST_DIR })

  if (!(await pathExists(SRC_ASSETS_DIR))) {
    warn('source assets dir not found, skip copy', { SRC_ASSETS_DIR })
  } else {
    await syncDirectory(SRC_ASSETS_DIR, DEST_ROOT, 'excalidraw')
  }

  if (!(await pathExists(SRC_VDITOR_DIST_DIR))) {
    warn('vditor dist dir not found, skip copy', { SRC_VDITOR_DIST_DIR })
  } else {
    // Vditor internally resolves runtime assets as `${cdn}/dist/...`,
    // so we must preserve the `dist` layer under `public/vditor/dist`.
    // Only this dist subtree is managed; older root-level vditor files are left alone.
    await syncDirectory(SRC_VDITOR_DIST_DIR, DEST_VDITOR_DIST_DIR, 'vditor')
  }

  log('done')
}

main().catch((e) => {
  console.error('[copy_excalidraw_assets] failed:', e)
  process.exitCode = 1
})
