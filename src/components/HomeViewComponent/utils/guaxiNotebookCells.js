import {
  buildPythonCellReferenceBlock,
  createPythonCellId,
  createPythonCellStableId,
  normalizePythonCellMeta,
  parsePythonCellBlocks,
} from './guaxiPythonCellSyntax'
import {
  buildMathCellBlock,
  buildMathCellReferenceBlock,
  createMathCellId,
  normalizeMathCellMeta,
  parseMathCellBlocks,
} from './guaxiMathCellSyntax'

const NOTEBOOK_MARKDOWN_CELL_PREFIX = 'nb-md'
const NOTEBOOK_PYTHON_CELL_PREFIX = 'nb-py'
const NOTEBOOK_MATH_CELL_PREFIX = 'nb-math'

const createStableId = (prefix) => {
  const randomPart = Math.random().toString(36).slice(2, 10)
  return `${prefix}-${Date.now().toString(36)}-${randomPart}`
}

export const createNotebookMarkdownCell = (content = '', options = {}) => ({
  type: 'markdown',
  key: String(options.key || createStableId(NOTEBOOK_MARKDOWN_CELL_PREFIX)),
  content: String(content || ''),
})

export const createNotebookPythonCell = (options = {}) => {
  const meta = normalizePythonCellMeta(options.meta || {})
  const stableId = String(meta.id || options.stableId || createPythonCellStableId() || createStableId(NOTEBOOK_PYTHON_CELL_PREFIX))
  meta.id = stableId
  return {
    type: 'python',
    key: String(options.key || stableId),
    code: String(options.code || ''),
    meta,
  }
}

export const createNotebookMathCell = (expression = '', options = {}) => {
  const meta = normalizeMathCellMeta(options.meta || {})
  const cell = {
    type: 'math',
    key: String(options.key || createStableId(NOTEBOOK_MATH_CELL_PREFIX)),
    expression: String(expression || ''),
    sourceType: ['fenced', 'bracket', 'dollar', 'marker'].includes(String(options.sourceType || ''))
      ? String(options.sourceType)
      : 'marker',
    fenceLanguage: String(options.fenceLanguage || 'math').trim() || 'math',
    meta,
  }
  // 保留 block 引用，用于 cellId 生成和位置追踪（marker 类型从 parseMathCellBlocks 获取）
  if (options.block && typeof options.block === 'object') {
    cell.block = options.block
  }
  return cell
}

export const normalizeNotebookCells = (cells = []) => {
  const normalized = []

  for (const rawCell of Array.isArray(cells) ? cells : []) {
    if (!rawCell || typeof rawCell !== 'object') continue

    if (rawCell.type === 'python') {
      normalized.push(createNotebookPythonCell(rawCell))
      continue
    }

    if (rawCell.type === 'math') {
      normalized.push(createNotebookMathCell(rawCell.expression || rawCell.content || '', rawCell))
      continue
    }

    normalized.push(createNotebookMarkdownCell(rawCell.content || '', { key: rawCell.key }))
  }

  if (!normalized.length) {
    normalized.push(createNotebookMarkdownCell(''))
  }

  return normalized
}

const NOTEBOOK_FENCE_RE = /^\s*(`{3,}|~{3,})/
const NOTEBOOK_ATX_HEADING_RE = /^\s{0,3}#{1,6}(?:\s+|$)/
const NOTEBOOK_FRONTMATTER_RE = /^\s*---\s*$/
const NOTEBOOK_FRONTMATTER_END_RE = /^\s*(---|\.\.\.)\s*$/
const normalizeNotebookMarkdownText = (markdown = '') => String(markdown || '').replace(/\r\n?/g, '\n')

const detectFenceState = (line, state) => {
  const match = String(line || '').match(NOTEBOOK_FENCE_RE)
  if (!match) return state

  const fenceText = String(match[1] || '')
  const nextState = { ...state }
  const fenceChar = fenceText[0] || ''
  const fenceLength = fenceText.length

  if (!nextState.active) {
    nextState.active = true
    nextState.char = fenceChar
    nextState.length = fenceLength
    return nextState
  }

  if (nextState.char === fenceChar && fenceLength >= nextState.length) {
    nextState.active = false
    nextState.char = ''
    nextState.length = 0
  }
  return nextState
}

const detectFrontmatterEnd = (lines = []) => {
  if (!Array.isArray(lines) || lines.length < 3) return -1
  if (!NOTEBOOK_FRONTMATTER_RE.test(String(lines[0] || ''))) return -1

  for (let index = 1; index < lines.length; index += 1) {
    if (!NOTEBOOK_FRONTMATTER_END_RE.test(String(lines[index] || ''))) continue
    let endExclusive = index + 1
    while (endExclusive < lines.length && String(lines[endExclusive] || '').trim() === '') {
      endExclusive += 1
    }
    return endExclusive
  }

  return -1
}

// 设计说明：
// 1. 探索者模式需要把 Markdown 拆成更接近“文档大纲”的多个 cell，而不是把整段文本塞进一个超大 cell。
// 2. 这里优先按标题切分；如果某一段本身没有标题，则退化成按空行块切分，这样普通笔记也不会退化成单大块。
// 3. 切分时会避开 fenced code block 和 frontmatter，避免把代码块、YAML 头错误拆散。
export const createMarkdownPreviewSegments = (markdown = '') => {
  const text = normalizeNotebookMarkdownText(markdown)
  if (!text) return []

  const lines = text.split('\n')
  const segments = []
  const frontmatterEnd = detectFrontmatterEnd(lines)
  let startLine = 0

  const pushSegmentRange = (startIndex, endExclusive) => {
    const safeStart = Math.max(0, Number(startIndex || 0))
    const safeEnd = Math.max(safeStart, Number(endExclusive || safeStart))
    const segmentText = lines.slice(safeStart, safeEnd).join('\n')
    if (!segmentText) return
    segments.push({
      content: segmentText,
      startLine: safeStart + 1,
      endLine: Math.max(safeStart + 1, safeEnd),
    })
  }

  if (frontmatterEnd > 0) {
    pushSegmentRange(0, frontmatterEnd)
    startLine = frontmatterEnd
  }

  const headingStartIndexes = []
  let fenceState = { active: false, char: '', length: 0 }
  for (let index = startLine; index < lines.length; index += 1) {
    const line = String(lines[index] || '')
    if (!fenceState.active && NOTEBOOK_ATX_HEADING_RE.test(line)) {
      headingStartIndexes.push(index)
    }
    fenceState = detectFenceState(line, fenceState)
  }

  if (headingStartIndexes.length) {
    if (headingStartIndexes[0] > startLine) {
      pushSegmentRange(startLine, headingStartIndexes[0])
    }

    headingStartIndexes.forEach((headingStart, index) => {
      const nextHeadingStart = headingStartIndexes[index + 1] ?? lines.length
      pushSegmentRange(headingStart, nextHeadingStart)
    })
  } else {
    let chunkStart = startLine
    fenceState = { active: false, char: '', length: 0 }

    for (let index = startLine; index < lines.length; index += 1) {
      const line = String(lines[index] || '')
      fenceState = detectFenceState(line, fenceState)
      if (fenceState.active || line.trim() !== '') continue

      let blankEnd = index
      while (blankEnd + 1 < lines.length && String(lines[blankEnd + 1] || '').trim() === '') {
        blankEnd += 1
      }

      const hasDoubleBlankBoundary = blankEnd > index
      if (!hasDoubleBlankBoundary) continue

      pushSegmentRange(chunkStart, blankEnd + 1)
      chunkStart = blankEnd + 1
      index = blankEnd
    }

    if (chunkStart < lines.length) {
      pushSegmentRange(chunkStart, lines.length)
    }
  }

  return segments
}

const splitMarkdownIntoNotebookCellSources = (markdown = '') => {
  return createMarkdownPreviewSegments(markdown).map((segment) => String(segment?.content || ''))
}

const pushMarkdownCellsFromSource = (cells, markdown = '', keyPrefix = 'md') => {
  const sources = splitMarkdownIntoNotebookCellSources(markdown)
  sources.forEach((content, index) => {
    cells.push(createNotebookMarkdownCell(content, {
      key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-${keyPrefix}-${index}`,
    }))
  })
}

const splitMarkdownIntoNotebookCells = (markdown = '', keyPrefix = 'chunk') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n')
  if (!text) return []
  const lines = text.split('\n')
  const cells = []
  let markdownStart = 0

  const flushMarkdown = (endExclusive) => {
    if (endExclusive <= markdownStart) return
    const source = lines.slice(markdownStart, endExclusive).join('\n')
    pushMarkdownCellsFromSource(cells, source, `${keyPrefix}-${markdownStart}`)
  }

  // 设计说明：
  // 1. $$...$$ / \[...\] / ```math 代码块不再自动创建 math cell，
  //    而是作为普通 Markdown 内容保留在 markdown cell 中。
  // 2. Math cell 现在只能通过 <!--math-->...<!--/math--> 注释语法显式创建。
  // 3. splitMarkdownIntoNotebookCells 只负责按标题/段落切分 markdown cell，
  //    不再负责检测数学块。

  flushMarkdown(lines.length)
  return cells
}

const joinCellSource = (parts) => {
  let output = ''

  for (const part of parts) {
    const chunk = String(part || '')
    if (!chunk) continue
    if (!output) {
      output = chunk
      continue
    }

    const needsBreak =
      !output.endsWith('\n') &&
      !chunk.startsWith('\n')

    if (needsBreak) {
      output += '\n\n'
    }
    output += chunk
  }

  return output
}

export const serializeNotebookCells = (cells = []) => {
  const normalized = normalizeNotebookCells(cells)
  const parts = normalized.map((cell) => {
    if (cell.type === 'python') {
      // 设计说明：
      // 1. 探索者模式保存时不再把 Python 源码写回 Markdown 正文，而是写入同名 `.cellmeta`。
      // 2. Markdown 仅保留引用块作为 cell 的位置锚点，避免源码编辑模式被大段 `<!--py-->` 内容干扰。
      // 3. 旧 Markdown 中已有的完整 Python Cell 仍能在加载时解析，下一次保存会自动迁移到 sidecar 存储。
      return buildPythonCellReferenceBlock({
        meta: cell.meta || {},
      })
    }
    if (cell.type === 'math') {
      const expression = String(cell.expression || '').replace(/\r\n?/g, '\n')
      // 设计说明：
      // 1. marker 类型使用 <!--math-->...<!--/math--> 注释语法序列化，
      //    与 Python Cell 的 <!--py--> 语法对称。
      // 2. Math 表达式通常较短，直接在 Markdown 中内联存储，
      //    避免模式切换时依赖 sidecar 导致表达式丢失。
      // 3. 旧式 fenced / bracket / dollar 序列化保留作为降级路径。
      if (cell.sourceType === 'marker') {
        return buildMathCellBlock({
          expression,
          meta: cell.meta || {},
        })
      }
      if (cell.sourceType === 'fenced') {
        const lang = String(cell.fenceLanguage || 'math').trim() || 'math'
        return `\`\`\`${lang}\n${expression}\n\`\`\``
      }
      if (cell.sourceType === 'bracket') {
        return `\\[\n${expression}\n\\]`
      }
      return `$$\n${expression}\n$$`
    }
    return String(cell.content || '')
  })

  return joinCellSource(parts)
}

export const createNotebookCellsFromMarkdown = (markdown = '', { filePath = '' } = {}) => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n')
  const lines = text.split('\n')

  // 设计说明：
  // 1. 同时解析 Python Cell（<!--py-->）和 Math Cell（<!--math-->）标记块。
  // 2. 合并后按行号排序，逐一插入 cells 数组；块之间的 Markdown 文本拆分为 markdown cells。
  // 3. $$、\[、```math 不再被识别为 math cell —— 它们现在作为普通 Markdown 内容保留。
  const pyBlocks = parsePythonCellBlocks(text)
  const mathBlocks = parseMathCellBlocks(text)

  const allBlocks = [
    ...pyBlocks.map((b) => ({ ...b, blockType: 'python' })),
    ...mathBlocks.map((b) => ({ ...b, blockType: 'math' })),
  ].sort((a, b) => a.startLine - b.startLine)

  const cells = []
  let cursor = 0

  allBlocks.forEach((block) => {
    const startIndex = Math.max(0, Number(block.startLine || 1) - 1)
    const endExclusive = Math.max(startIndex, Number(block.endLine || startIndex + 1))

    // 跳过已被前一个块覆盖的区域（理论上不应重叠，但做防护）
    if (startIndex < cursor) return

    const leading = lines.slice(cursor, startIndex).join('\n')
    const leadingCells = splitMarkdownIntoNotebookCells(leading, `${block.blockType}-${cursor}`)
    if (leadingCells.length) {
      cells.push(...leadingCells)
    } else if (!cells.length) {
      cells.push(createNotebookMarkdownCell('', {
        key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-${block.blockType}-${cursor}-empty`,
      }))
    }

    if (block.blockType === 'python') {
      const meta = normalizePythonCellMeta(block.meta || {})
      if (!meta.id) {
        meta.id = createPythonCellStableId()
      }
      cells.push({
        type: 'python',
        key: String(meta.id),
        code: String(block.code || ''),
        meta,
        block,
        cellId: createPythonCellId(filePath, { ...block, meta }),
      })
    } else if (block.blockType === 'math') {
      const meta = normalizeMathCellMeta(block.meta || {})
      cells.push(createNotebookMathCell(block.expression || '', {
        key: `${NOTEBOOK_MATH_CELL_PREFIX}-${block.blockType}-${startIndex}`,
        sourceType: 'marker',
        meta,
        block,
      }))
    }

    cursor = endExclusive
  })

  const trailing = lines.slice(cursor).join('\n')
  const trailingCells = splitMarkdownIntoNotebookCells(trailing, `tail-${cursor}`)
  if (trailingCells.length) {
    cells.push(...trailingCells)
  } else if (!cells.length || (cells[cells.length - 1]?.type !== 'markdown')) {
    cells.push(createNotebookMarkdownCell('', {
      key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-tail-${cursor}-empty`,
    }))
  }

  return normalizeNotebookCells(cells).map((cell, index) => {
    if (cell.type === 'python') {
      const block = parsePythonCellBlocks(serializeNotebookCells([cell]))[0] || null
      return {
        ...cell,
        index,
        block,
        cellId: createPythonCellId(filePath, {
          ...(block || {}),
          meta: cell.meta,
        }),
      }
    }
    if (cell.type === 'math') {
      return {
        ...cell,
        index,
        cellId: createMathCellId(filePath, cell.block || {}),
      }
    }
    return {
      ...cell,
      index,
    }
  })
}

export const replaceNotebookCell = (cells = [], key, patch = {}) => {
  return normalizeNotebookCells(cells).map((cell) => {
    if (String(cell.key) !== String(key)) return cell
    if (cell.type === 'python') {
      return createNotebookPythonCell({
        ...cell,
        ...patch,
        meta: {
          ...(cell.meta || {}),
          ...(patch.meta || {}),
        },
      })
    }
    if (cell.type === 'math') {
      return createNotebookMathCell(
        Object.prototype.hasOwnProperty.call(patch, 'expression') ? patch.expression : cell.expression,
        {
          ...cell,
          ...patch,
          key: cell.key,
        },
      )
    }
    return createNotebookMarkdownCell(
      Object.prototype.hasOwnProperty.call(patch, 'content') ? patch.content : cell.content,
      { key: cell.key },
    )
  })
}

export const insertNotebookCell = (cells = [], index, type = 'markdown') => {
  const normalized = normalizeNotebookCells(cells)
  const next = normalized.slice()
  const insertAt = Math.max(0, Math.min(normalized.length, Number(index || 0)))
  const cell = type === 'python'
    ? createNotebookPythonCell({
      code: '',
    })
    : type === 'math'
      ? createNotebookMathCell('')
      : createNotebookMarkdownCell('')
  next.splice(insertAt, 0, cell)
  return normalizeNotebookCells(next)
}

export const deleteNotebookCell = (cells = [], key) => {
  const filtered = normalizeNotebookCells(cells).filter((cell) => String(cell.key) !== String(key))
  return normalizeNotebookCells(filtered)
}

