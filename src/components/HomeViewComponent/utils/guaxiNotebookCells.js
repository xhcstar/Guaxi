import {
  buildPythonCellBlock,
  createPythonCellId,
  createPythonCellStableId,
  normalizePythonCellMeta,
  parsePythonCellBlocks,
} from './guaxiPythonCellSyntax'

const NOTEBOOK_MARKDOWN_CELL_PREFIX = 'nb-md'
const NOTEBOOK_PYTHON_CELL_PREFIX = 'nb-py'

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

export const normalizeNotebookCells = (cells = []) => {
  const normalized = []

  for (const rawCell of Array.isArray(cells) ? cells : []) {
    if (!rawCell || typeof rawCell !== 'object') continue

    if (rawCell.type === 'python') {
      normalized.push(createNotebookPythonCell(rawCell))
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
      return buildPythonCellBlock({
        code: cell.code || '',
        meta: cell.meta || {},
      })
    }
    return String(cell.content || '')
  })

  return joinCellSource(parts)
}

export const createNotebookCellsFromMarkdown = (markdown = '', { filePath = '' } = {}) => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n')
  const lines = text.split('\n')
  const blocks = parsePythonCellBlocks(text)
  const cells = []
  let cursor = 0

  blocks.forEach((block, index) => {
    const startIndex = Math.max(0, Number(block.startLine || 1) - 1)
    const endExclusive = Math.max(startIndex, Number(block.endLine || startIndex + 1))
    const leading = lines.slice(cursor, startIndex).join('\n')

    const leadingCells = splitMarkdownIntoNotebookCellSources(leading)
    if (leadingCells.length) {
      leadingCells.forEach((content, leadingIndex) => {
        cells.push(createNotebookMarkdownCell(content, {
          key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-${index}-${cursor}-${leadingIndex}`,
        }))
      })
    } else if (!cells.length) {
      cells.push(createNotebookMarkdownCell('', {
        key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-${index}-${cursor}-empty`,
      }))
    }

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

    cursor = endExclusive
  })

  const trailing = lines.slice(cursor).join('\n')
  const trailingCells = splitMarkdownIntoNotebookCellSources(trailing)
  if (trailingCells.length) {
    trailingCells.forEach((content, trailingIndex) => {
      cells.push(createNotebookMarkdownCell(content, {
        key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-tail-${cursor}-${trailingIndex}`,
      }))
    })
  } else if (!cells.length || cells[cells.length - 1]?.type === 'python') {
    cells.push(createNotebookMarkdownCell('', {
      key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-tail-${cursor}-empty`,
    }))
  }

  return normalizeNotebookCells(cells).map((cell, index) => {
    if (cell.type !== 'python') {
      return {
        ...cell,
        index,
      }
    }

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
    : createNotebookMarkdownCell('')
  next.splice(insertAt, 0, cell)
  return normalizeNotebookCells(next)
}

export const deleteNotebookCell = (cells = [], key) => {
  const filtered = normalizeNotebookCells(cells).filter((cell) => String(cell.key) !== String(key))
  return normalizeNotebookCells(filtered)
}

