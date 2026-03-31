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

    const markdownCell = createNotebookMarkdownCell(rawCell.content || '', { key: rawCell.key })
    const prev = normalized[normalized.length - 1]
    if (prev?.type === 'markdown') {
      prev.content = `${prev.content}${markdownCell.content}`
    } else {
      normalized.push(markdownCell)
    }
  }

  if (!normalized.length) {
    normalized.push(createNotebookMarkdownCell(''))
  }

  return normalized
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

    if (leading || !cells.length) {
      cells.push(createNotebookMarkdownCell(leading, {
        key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-${index}-${cursor}`,
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
  if (trailing || !cells.length || cells[cells.length - 1]?.type === 'python') {
    cells.push(createNotebookMarkdownCell(trailing, {
      key: `${NOTEBOOK_MARKDOWN_CELL_PREFIX}-tail-${cursor}`,
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
      code: '# Python Cell\nprint("Hello from Guaxi Python Cell")',
    })
    : createNotebookMarkdownCell('')
  next.splice(insertAt, 0, cell)
  return normalizeNotebookCells(next)
}

export const deleteNotebookCell = (cells = [], key) => {
  const filtered = normalizeNotebookCells(cells).filter((cell) => String(cell.key) !== String(key))
  return normalizeNotebookCells(filtered)
}
