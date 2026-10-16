import { invoke } from '@tauri-apps/api/core'
import { createLogger } from './logger'

const logger = createLogger('MarkdownLinkRename')

// 重命名笔记后，自动更新知识库里指向它的链接（wiki 链接 + 相对路径链接）。
//
// 这一段**会改写多份文件**，所以规则刻意保守：
//   1. 只改「确实指向被改名那个文件」的链接——wiki 链接按 basename 解析、相对链接按绝对路径解析，
//      解析不出、或有歧义（全库里有多个同名笔记）时一律不动。
//   2. 代码围栏内的内容整段跳过，避免把示例代码里的 [[...]] 也改掉。
//   3. 已在编辑器里打开的文件整段跳过——它们可能有未保存的编辑，读盘再写回会丢掉用户正在改的内容。
//   4. 任何异常都只记录、不上抛：链接更新失败绝不能表现成"重命名失败"。
//   5. 只写内容真正变化的文件；行尾风格原样保留（按原文件判定 \r\n 还是 \n）。

const MARKDOWN_EXT_RE = /\.(md|markdown|mdown)$/i
const MAX_WALK_DEPTH = 24
const MAX_FILE_BYTES = 2 * 1024 * 1024

const toPosix = (value) => String(value || '').replace(/\\/g, '/')
// 是否根路径。splitPath 会丢掉前导空段，凡是要再拼回去的地方都得先记住这一点，
// 否则解析结果（kb/foo.md）永远和调用方给的绝对路径（/kb/foo.md）对不上，链接会静默匹配失败。
const isRootedPath = (value) => /^[/\\]/.test(String(value || ''))
const splitPath = (value) => toPosix(value).split('/').filter((seg) => seg.length > 0)
const baseNameOf = (value) => splitPath(value).pop() || ''
const dirNameOf = (value) => {
  const parts = splitPath(value)
  parts.pop()
  const joined = parts.join('/')
  return isRootedPath(value) ? `/${joined}` : joined
}
const stemOf = (value) => baseNameOf(value).replace(/\.[^.]+$/, '')

const isMarkdownPath = (value) => MARKDOWN_EXT_RE.test(toPosix(value))
const ensureMdExt = (value) => (isMarkdownPath(value) ? value : `${value}.md`)

// 把相对路径按 baseDir 展开成绝对（正斜杠）路径，处理 . / ..
const resolveRelative = (baseDir, relative) => {
  const parts = splitPath(baseDir)
  for (const seg of splitPath(relative)) {
    if (seg === '.') continue
    if (seg === '..') parts.pop()
    else parts.push(seg)
  }
  const joined = parts.join('/')
  return isRootedPath(baseDir) ? `/${joined}` : joined
}

// 从 fromDir 到 toAbs 的相对路径；同名同目录时返回文件名本身。
const relativePathFrom = (fromDir, toAbs) => {
  const a = splitPath(fromDir)
  const b = splitPath(toAbs)
  let i = 0
  while (i < a.length && i < b.length && a[i] === b[i]) i += 1
  const up = a.slice(i).map(() => '..')
  const down = b.slice(i)
  const joined = [...up, ...down].join('/')
  return joined || baseNameOf(toAbs)
}

// get_file_tree 只返回一层，这里自己递归；目录名以 is_directory 标记。
const collectMarkdownFiles = async (root, out, depth) => {
  if (depth > MAX_WALK_DEPTH) return out
  let nodes = []
  try {
    nodes = await invoke('get_file_tree', { path: root })
  } catch (error) {
    logger.debug('[Rename] list dir failed', { root }, error)
    return out
  }
  for (const node of Array.isArray(nodes) ? nodes : []) {
    const name = String(node?.name || '')
    if (!name || name.startsWith('.')) continue
    const full = `${root}/${name}`
    const isDir = node?.is_directory === true || node?.isDirectory === true
    if (isDir) {
      await collectMarkdownFiles(full, out, depth + 1)
    } else if (isMarkdownPath(name)) {
      out.push(full)
    }
  }
  return out
}

// 逐行扫描并改写链接；围栏内的行原样保留。
const rewriteLinksInText = ({ text, selfPath, oldPath, newPath, oldStem, newStem, oldStemIsUnique }) => {
  const selfDir = dirNameOf(selfPath)
  const eol = text.includes('\r\n') ? '\r\n' : '\n'
  const lines = text.replace(/\r\n/g, '\n').split('\n')

  let inFence = false
  let fenceToken = ''
  let changed = 0

  const rewritten = lines.map((line) => {
    const fenceHit = /^\s*(`{3,}|~{3,})/.exec(line)
    if (fenceHit) {
      const token = fenceHit[1]
      if (!inFence) {
        inFence = true
        fenceToken = token
      } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
        inFence = false
        fenceToken = ''
      }
      return line
    }
    if (inFence) return line

    let next = line

    // 1) wiki 链接：[[目标]] / [[目标|别名]] / [[目标#锚点]]
    next = next.replace(/\[\[([^\]\n|#]+)(#[^\]\n|]*)?(\|[^\]\n]*)?\]\]/g, (full, rawTarget, anchor = '', alias = '') => {
      const target = String(rawTarget || '').trim()
      if (!target) return full

      const bare = !target.includes('/')
      let hit = false
      if (bare) {
        // 裸名字有歧义（全库多个同名）时不动，避免改错别的笔记。
        hit = oldStemIsUnique && stemOf(target) === oldStem
      } else {
        hit = resolveRelative(selfDir, ensureMdExt(target)) === oldPath
      }
      if (!hit) return full

      const nextTarget = bare ? newStem : relativePathFrom(selfDir, newPath).replace(/\.md$/i, '')
      const keepExt = !bare && /\.md$/i.test(target)
      changed += 1
      return `[[${keepExt ? `${nextTarget}.md` : nextTarget}${anchor}${alias}]]`
    })

    // 2) 相对路径链接：[文字](相对路径.md "可选标题")，跳过 http/绝对/锚点
    next = next.replace(/\]\(\s*([^)\s]+)(\s+"[^"]*")?\s*\)/g, (full, rawHref, title = '') => {
      const href = String(rawHref || '').trim()
      if (!href || /^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(href)) return full

      const hashAt = href.indexOf('#')
      const pathPart = hashAt >= 0 ? href.slice(0, hashAt) : href
      const anchorPart = hashAt >= 0 ? href.slice(hashAt) : ''
      if (!pathPart || !isMarkdownPath(pathPart)) return full
      if (resolveRelative(selfDir, pathPart) !== oldPath) return full

      const nextHref = `${relativePathFrom(selfDir, newPath)}${anchorPart}`
      changed += 1
      return `](${nextHref}${title})`
    })

    return next
  })

  if (!changed) return { text, changed: 0 }
  return { text: rewritten.join(eol), changed }
}

/**
 * 重命名后更新全库链接。
 * @returns {{filesScanned:number, filesChanged:number, linksChanged:number, skippedOpen:number, skippedLarge:number}}
 */
export const updateMarkdownLinksAfterRename = async ({ repoRoot, oldPath, newPath, openPaths = [] }) => {
  const summary = { filesScanned: 0, filesChanged: 0, linksChanged: 0, skippedOpen: 0, skippedLarge: 0 }
  const root = toPosix(repoRoot || '').replace(/\/$/, '')
  const oldAbs = toPosix(oldPath || '')
  const newAbs = toPosix(newPath || '')
  if (!root || !oldAbs || !newAbs || !isMarkdownPath(oldAbs) || !isMarkdownPath(newAbs)) return summary

  const openSet = new Set((Array.isArray(openPaths) ? openPaths : []).map((p) => toPosix(p)))

  try {
    const files = await collectMarkdownFiles(root, [], 0)
    const oldStem = stemOf(oldAbs)
    // 裸 [[名字]] 只有在全库唯一时才敢改，否则可能改到别的同名笔记。
    const oldStemIsUnique = files.filter((f) => stemOf(f) === oldStem).length === 1

    for (const file of files) {
      // 打开着的文件可能带未保存编辑：读盘再写回会丢掉用户正在改的内容。
      if (openSet.has(file)) {
        summary.skippedOpen += 1
        continue
      }
      summary.filesScanned += 1
      try {
        const content = String(await invoke('read_file_content', { path: file }) || '')
        if (content.length > MAX_FILE_BYTES) {
          summary.skippedLarge += 1
          continue
        }
        const { text, changed } = rewriteLinksInText({
          text: content,
          selfPath: file,
          oldPath: oldAbs,
          newPath: newAbs,
          oldStem,
          newStem: stemOf(newAbs),
          oldStemIsUnique,
        })
        if (!changed) continue
        await invoke('write_file_content', { path: file, content: text })
        summary.filesChanged += 1
        summary.linksChanged += changed
        logger.info('[Rename] markdown links updated', { file, changed })
      } catch (error) {
        // 单个文件失败不影响其它文件，更不影响已经完成的重命名本身。
        logger.warn('[Rename] update links in file failed', { file }, error)
      }
    }
  } catch (error) {
    logger.warn('[Rename] link update aborted', { root, oldAbs, newAbs }, error)
  }

  logger.info('[Rename] link update finished', { oldAbs, newAbs, ...summary })
  return summary
}
