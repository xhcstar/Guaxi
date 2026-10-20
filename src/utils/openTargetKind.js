// 文件类型判定与「打开方式」路由。
//
// 设计约束：这是纯新增模块，既有 `fetchFileContent → openFileInTab` 的文本路径一个字节都没改。
// 它只在「用户主动打开一个文件」时被调用，用于决定这个文件该进 Monaco 还是进阅读器。
//
// 判定顺序刻意与后端 `sniff_file_kind` 保持一致：
//   1) 前端先按扩展名快速判一次（同步、零 IO，覆盖 99% 的情况）；
//   2) 扩展名不在表里、或扩展名与常见二进制冲突时，再问后端嗅探魔数；
//   3) 后端不可用时退回「按扩展名当文本」——也就是今天的行为。
//
// 这样做的意义：既有的 .md/.txt/.py 等文件永远不会触发额外的 IPC，打开速度不变。

import { invoke } from '@tauri-apps/api/core'

/** 阅读器分组 → 前端渲染器 key。与 ReaderPane 的分支一一对应。 */
export const READER_KIND = Object.freeze({
  TEXT: 'text',
  PDF: 'pdf',
  WORD: 'word',
  EBOOK: 'ebook',
  SPREADSHEET: 'spreadsheet',
  SLIDES: 'slides',
  IMAGE: 'image',
  MEDIA: 'media',
  HTML: 'html',
  BINARY: 'binary',
})

/** 图片扩展名 → MIME。用于免除后端嗅探，直接给 `<img>` 渲染。 */
const IMAGE_MIME_BY_EXT = Object.freeze({
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  jfif: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  bmp: 'image/bmp',
  ico: 'image/x-icon',
  svg: 'image/svg+xml',
  tif: 'image/tiff',
  tiff: 'image/tiff',
  avif: 'image/avif',
  heic: 'image/heic',
})

const AUDIO_EXTS = new Set(['mp3', 'wav', 'flac', 'ogg', 'm4a', 'aac', 'opus', 'wma'])
const VIDEO_EXTS = new Set(['mp4', 'mkv', 'webm', 'avi', 'mov', 'm4v', 'wmv', 'flv'])

/** 电子书（交给 ebook-rs）。 */
const EBOOK_EXTS = new Set(['epub', 'mobi', 'azw', 'azw3', 'kfx', 'fb2', 'lit', 'cbz'])

/** 表格（交给 calamine）。csv/tsv 不算——它们本来就是文本，进编辑器更合理。 */
const SPREADSHEET_EXTS = new Set(['xlsx', 'xls', 'ods'])

/** 幻灯片（交给 pptx 预览 / anydoc）。 */
const SLIDES_EXTS = new Set(['pptx', 'ppt', 'odp'])

/** Word（docx 由 docx-preview 直接渲染；doc 走 anydoc 转 Markdown）。 */
const WORD_EXTS = new Set(['docx', 'doc', 'rtf', 'odt'])

/** 明确应当留在编辑器里的文本扩展名。 */
const TEXT_EXTS = new Set([
  'md', 'markdown', 'mdx', 'txt', 'text', 'log', 'csv', 'tsv',
  'json', 'jsonc', 'json5', 'yaml', 'yml', 'toml', 'ini', 'conf', 'cfg',
  'xml', 'html', 'htm', 'xhtml', 'css', 'scss', 'sass', 'less',
  'js', 'mjs', 'cjs', 'jsx', 'ts', 'tsx', 'vue', 'svelte',
  'py', 'pyw', 'ipynb', 'java', 'c', 'h', 'cpp', 'cc', 'cxx', 'hpp', 'hh',
  'cs', 'go', 'rs', 'lua', 'rb', 'php', 'swift', 'kt', 'kts', 'scala', 'dart',
  'sh', 'bash', 'zsh', 'fish', 'bat', 'cmd', 'ps1', 'psm1', 'psd1',
  'sql', 'r', 'm', 'jl', 'pl', 'ex', 'exs', 'erl', 'hs', 'clj', 'cljs',
  'tex', 'latex', 'bib', 'sty', 'cls', 'typ', 'org', 'rst', 'adoc', 'asciidoc',
  'mmd', 'mermaid', 'puml', 'plantuml', 'd2', 'dot', 'gv',
  'gitignore', 'gitattributes', 'gitmodules', 'env', 'editorconfig',
  'dockerfile', 'makefile', 'cmake', 'gradle', 'properties', 'lock',
  'diff', 'patch', 'srt', 'vtt', 'ass', 'ssa',
])

/** 取小写扩展名；无扩展名返回空串。 */
export function extOf(path) {
  const name = String(path || '').split(/[\\/]/).pop() || ''
  const dot = name.lastIndexOf('.')
  if (dot <= 0) return ''
  return name.slice(dot + 1).toLowerCase()
}

/** 取不含扩展名的文件名。 */
export function stemOf(path) {
  const name = String(path || '').split(/[\\/]/).pop() || ''
  const dot = name.lastIndexOf('.')
  return dot > 0 ? name.slice(0, dot) : name
}

/**
 * 纯前端快速判定。返回 null 表示「拿不准，需要后端嗅探」。
 * @param {string} path
 * @returns {{kind:string, reader:string, mime:string}|null}
 */
export function classifyByExtension(path) {
  const ext = extOf(path)
  if (!ext) return null

  if (ext in IMAGE_MIME_BY_EXT) {
    // SVG 是文本，进编辑器更好用；但作为「打开图片」也说得通，这里交给编辑器。
    if (ext === 'svg') return { kind: 'text', reader: READER_KIND.TEXT, mime: '' }
    return { kind: 'image', reader: READER_KIND.IMAGE, mime: IMAGE_MIME_BY_EXT[ext] }
  }
  if (AUDIO_EXTS.has(ext)) return { kind: 'audio', reader: READER_KIND.MEDIA, mime: `audio/${ext}` }
  if (VIDEO_EXTS.has(ext)) return { kind: 'video', reader: READER_KIND.MEDIA, mime: `video/${ext}` }
  if (ext === 'pdf') return { kind: 'pdf', reader: READER_KIND.PDF, mime: 'application/pdf' }
  if (EBOOK_EXTS.has(ext)) return { kind: ext, reader: READER_KIND.EBOOK, mime: '' }
  if (SPREADSHEET_EXTS.has(ext)) return { kind: ext, reader: READER_KIND.SPREADSHEET, mime: '' }
  if (SLIDES_EXTS.has(ext)) return { kind: ext, reader: READER_KIND.SLIDES, mime: '' }
  if (WORD_EXTS.has(ext)) return { kind: ext, reader: READER_KIND.WORD, mime: '' }
  if (TEXT_EXTS.has(ext)) return { kind: 'text', reader: READER_KIND.TEXT, mime: '' }
  return null
}

/**
 * 决定一个文件的打开方式。
 *
 * @param {string} path 绝对路径
 * @param {'auto'|'editor'|'reader'} [mode] 用户意图：
 *   - auto（默认）：按类型自动分流；
 *   - editor：强制进编辑器（右键「用编辑器打开」）；
 *   - reader：强制进阅读器（即便能当文本读）。
 * @returns {Promise<{kind:string, reader:string, mime:string, size:number, container:boolean, forced:boolean}>}
 */
export async function resolveOpenTarget(path, mode = 'auto') {
  const quick = classifyByExtension(path)

  if (mode === 'editor') {
    return { kind: 'text', reader: READER_KIND.TEXT, mime: '', size: 0, container: false, forced: true }
  }

  if (mode === 'auto' && quick && quick.reader !== READER_KIND.TEXT) {
    // 扩展名已经明确是二进制/文档：直接走阅读器，不做额外的 IPC 嗅探。
    return { ...quick, size: 0, container: false, forced: false }
  }

  if (mode === 'auto' && quick && quick.reader === READER_KIND.TEXT) {
    // 文本扩展名：保持今天的路径，绝不额外 invoke。
    return { ...quick, size: 0, container: false, forced: false }
  }

  // 扩展名不认识（或用户强制走阅读器）：问后端嗅探真实类型。
  try {
    const info = await invoke('sniff_file_kind', { path })
    if (!info) throw new Error('empty sniff result')
    if (mode === 'reader' && info.reader === READER_KIND.TEXT) {
      return { ...info, reader: READER_KIND.TEXT, forced: true }
    }
    return { ...info, forced: mode === 'reader' }
  } catch (error) {
    console.warn('[openTarget] sniff failed, fallback to text', { path, error })
    return { kind: 'text', reader: READER_KIND.TEXT, mime: '', size: 0, container: false, forced: false }
  }
}

/** 该 reader 分组是否需要外部阅读器组件（而非 Monaco）。 */
export function needsReader(reader) {
  return !!reader && reader !== READER_KIND.TEXT
}

/**
 * 文件树图标的 reader 提示：让「打开会进阅读器」的文件在树上有个视觉线索。
 * 返回 null 表示沿用原有图标逻辑。
 */
export function readerBadgeFor(path) {
  const quick = classifyByExtension(path)
  if (!quick) return null
  return quick.reader !== READER_KIND.TEXT ? quick.reader : null
}
