import { save, open } from '@tauri-apps/plugin-dialog'
import { writeFile } from '@tauri-apps/plugin-fs'
import {
  createPythonCellId,
  createPythonSessionId,
  dataframeToMarkdownTable,
  normalizePythonCellMeta,
  parsePythonCellBlocks,
  replacePythonCellBlock,
} from './guaxiPythonCellSyntax'

// 设计说明：
// 1. 编辑模式下保留 Markdown 源码可见，Python Cell 卡片作为附加 view zone 渲染在源码块下方。
// 2. 这样用户既能直接编辑 Markdown 原文，也能立刻获得接近 Notebook 的执行 UI，避免“插入后源码消失”的割裂体验。
// 3. 卡片中的草稿代码先保存在内存态，保存文件/切换文件前统一 flush 回 Markdown 源文本。
// 4. 输出、变量、表格和“写回 Markdown”都围绕同一个文件级 Python session 运行，确保体验接近 Notebook。

const escapeHtml = (value) => String(value == null ? '' : value)
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const sanitizePythonIdentifier = (value, fallback = 'df') => {
  const normalized = String(value || '')
    .replace(/\.[^.]+$/, '')
    .replace(/[^\w]+/g, '_')
    .replace(/^_+/, '')
  if (!normalized) return fallback
  if (/^\d/.test(normalized)) return `${fallback}_${normalized}`
  return normalized
}

const formatPythonExecOutput = (res) => {
  const chunks = []
  const stdout = String(res?.stdout || '').trim()
  const stderr = String(res?.stderr || '').trim()
  const resultRepr = String(res?.result_repr || '').trim()

  if (stdout) chunks.push(stdout)
  if (stderr) chunks.push(stderr)
  if (resultRepr && !stdout.includes(resultRepr)) chunks.push(resultRepr)
  return chunks.join('\n\n').trim()
}

const getDisplayFromResult = (res) => {
  const value = res?.result_json?.__guaxi_display__
  return value && typeof value === 'object' ? value : null
}

const getDataframeFromResult = (res) => {
  const value = res?.result_json?.__guaxi_dataframe__
  if (!value || typeof value !== 'object') return null
  return {
    columns: Array.isArray(value.columns) ? value.columns.map((item) => String(item ?? '')) : [],
    rows: Array.isArray(value.rows) ? value.rows : [],
    rowCount: Number(value.row_count || 0),
    truncated: !!value.truncated,
    dtypes: value.dtypes || {},
    sourcePath: String(value.source_path || '').trim(),
  }
}

const toDelimitedText = (table, delimiter = ',') => {
  const escapeField = (value) => {
    const text = String(value == null ? '' : value)
    if (!text.includes(delimiter) && !text.includes('"') && !text.includes('\n')) return text
    return `"${text.replace(/"/g, '""')}"`
  }
  const lines = []
  lines.push((table.columns || []).map(escapeField).join(delimiter))
  for (const row of table.rows || []) {
    lines.push((Array.isArray(row) ? row : []).map(escapeField).join(delimiter))
  }
  return lines.join('\n')
}

const writeMarkdownOutputBlock = (markdown, block, cellId, markdownOutput) => {
  const startMarker = `<!-- guaxi:pyout:${cellId}:start -->`
  const endMarker = `<!-- guaxi:pyout:${cellId}:end -->`
  const lines = String(markdown || '').replace(/\r\n?/g, '\n').split('\n')
  const blockEndIndex = Math.max(0, Number(block?.endLine || 1) - 1)
  const existingStart = lines.findIndex((line) => String(line || '').trim() === startMarker)

  const nextChunk = ['', startMarker, ...String(markdownOutput || '').replace(/\r\n?/g, '\n').split('\n'), endMarker]

  if (existingStart >= 0) {
    const existingEnd = lines.findIndex((line, index) => index > existingStart && String(line || '').trim() === endMarker)
    if (existingEnd >= 0) {
      lines.splice(existingStart, existingEnd - existingStart + 1, ...nextChunk)
      return lines.join('\n')
    }
  }

  lines.splice(blockEndIndex + 1, 0, ...nextChunk)
  return lines.join('\n')
}

const buildMarkdownOutputFromState = (state) => {
  if (state?.dataframe) {
    return dataframeToMarkdownTable(state.dataframe)
  }

  const display = state?.display
  if (display?.type === 'markdown' && display.markdown) {
    return String(display.markdown)
  }
  if (display?.type === 'html' && display.html) {
    return String(display.html)
  }
  if (display?.type === 'svg' && display.svg) {
    return String(display.svg)
  }
  if (display?.type === 'image') {
    const dataUrl = display.dataUrl && String(display.dataUrl).startsWith('data:')
      ? String(display.dataUrl)
      : display.data_base64
        ? `data:${String(display.mime || 'image/png')};base64,${String(display.data_base64)}`
        : ''
    if (dataUrl) {
      return `![Python Cell 输出](${dataUrl})`
    }
  }

  const text = String(state?.outputText || '').trim()
  if (!text) return ''
  return `\`\`\`text\n${text}\n\`\`\``
}

const createDataFrameCard = (table, { onImport, onExport, onInsertMarkdown } = {}) => {
  const wrap = document.createElement('div')
  wrap.className = 'guaxi-inline-dataframe'

  const toolbar = document.createElement('div')
  toolbar.className = 'guaxi-inline-dataframe__toolbar'
  toolbar.innerHTML = `
    <div class="guaxi-inline-dataframe__meta">${escapeHtml(`${table.rowCount} 行 × ${(table.columns || []).length} 列${table.truncated ? ' · 已截断预览' : ''}${table.sourcePath ? ` · ${table.sourcePath.split(/[\\/]/).pop()}` : ''}`)}</div>
  `

  const actions = document.createElement('div')
  actions.className = 'guaxi-inline-dataframe__actions'

  const appendButton = (label, handler) => {
    const btn = document.createElement('button')
    btn.type = 'button'
    btn.className = 'guaxi-inline-dataframe__btn'
    btn.textContent = label
    btn.addEventListener('click', (event) => {
      event.preventDefault()
      event.stopPropagation()
      void handler?.()
    })
    actions.appendChild(btn)
  }

  appendButton('导入文件', onImport)
  appendButton('插入 Markdown 表格', onInsertMarkdown)
  appendButton('导出 CSV', () => onExport?.('csv'))
  appendButton('导出 JSON', () => onExport?.('json'))
  toolbar.appendChild(actions)

  const viewport = document.createElement('div')
  viewport.className = 'guaxi-inline-dataframe__viewport'

  const tableEl = document.createElement('table')
  tableEl.className = 'guaxi-inline-dataframe__table'
  const thead = document.createElement('thead')
  const headerRow = document.createElement('tr')
  ;(table.columns || []).forEach((column) => {
    const th = document.createElement('th')
    th.textContent = String(column ?? '')
    headerRow.appendChild(th)
  })
  thead.appendChild(headerRow)
  tableEl.appendChild(thead)

  const tbody = document.createElement('tbody')
  ;(table.rows || []).forEach((row) => {
    const tr = document.createElement('tr')
    ;(table.columns || []).forEach((_, index) => {
      const td = document.createElement('td')
      td.textContent = String(Array.isArray(row) ? row[index] ?? '' : '')
      tr.appendChild(td)
    })
    tbody.appendChild(tr)
  })
  tableEl.appendChild(tbody)
  viewport.appendChild(tableEl)

  wrap.appendChild(toolbar)
  wrap.appendChild(viewport)
  return wrap
}

export const createGuaxiPythonCellEditorManager = ({
  editor,
  monaco,
  logger,
  message,
  invoke,
  getMarkdown,
  applyMarkdown,
  getFilePath,
  onRequestConsole,
}) => {
  let disposed = false
  let zoneIds = []
  const cellStates = new Map()
  let refreshScheduled = false
  let refreshInFlight = false
  let lastRefreshSignature = ''

  const clearZones = () => {
    if (!editor) return
    try {
      editor.changeViewZones((accessor) => {
        zoneIds.forEach((zoneId) => {
          try {
            accessor.removeZone(zoneId)
          } catch {
          }
        })
      })
    } catch {
    }
    zoneIds = []
  }

  const scheduleRefresh = () => {
    if (disposed || refreshScheduled) return
    refreshScheduled = true
    requestAnimationFrame(() => {
      refreshScheduled = false
      refresh()
    })
  }

  const saveTableToFile = async (table, format = 'csv') => {
    const normalized = String(format || 'csv').trim().toLowerCase()
    const defaultName = `python-table.${normalized === 'json' ? 'json' : 'csv'}`
    const outputPath = await save({
      defaultPath: defaultName,
      filters: [{ name: 'Table', extensions: [normalized === 'json' ? 'json' : 'csv'] }],
    })
    if (!outputPath) return false

    const text = normalized === 'json'
      ? JSON.stringify((table.rows || []).map((row) => {
        const item = {}
        ;(table.columns || []).forEach((column, index) => {
          item[column] = Array.isArray(row) ? row[index] ?? null : null
        })
        return item
      }), null, 2)
      : toDelimitedText(table, ',')

    await writeFile(outputPath, new TextEncoder().encode(text))
    logger?.info?.('[PyCellEditor] dataframe exported', { outputPath, format: normalized })
    message?.success?.('表格已导出')
    return true
  }

  const flushDrafts = () => {
    const markdown = getMarkdown?.()
    const blocks = parsePythonCellBlocks(markdown)
    if (!blocks.length) return markdown

    let nextMarkdown = markdown
    for (let i = blocks.length - 1; i >= 0; i -= 1) {
      const block = blocks[i]
      const filePath = getFilePath?.() || ''
      const cellId = createPythonCellId(filePath, block)
      const state = cellStates.get(cellId)
      if (!state) continue
      const draftCode = typeof state.draftCode === 'string' ? state.draftCode : block.code
      const draftMeta = normalizePythonCellMeta({
        ...(block.meta || {}),
        ...(state.meta || {}),
      })
      nextMarkdown = replacePythonCellBlock(nextMarkdown, block, draftCode, draftMeta)
    }
    return nextMarkdown
  }

  const applyDrafts = (action = 'pycell-flush') => {
    const nextMarkdown = flushDrafts()
    if (String(nextMarkdown) === String(getMarkdown?.() || '')) return nextMarkdown
    applyMarkdown?.(nextMarkdown, { action })
    return nextMarkdown
  }

  const setStateOutput = (state, result) => {
    state.result = result
    state.outputText = formatPythonExecOutput(result)
    state.display = getDisplayFromResult(result)
    state.dataframe = getDataframeFromResult(result)
    state.sessionVars = Array.isArray(result?.session_vars) ? result.session_vars : []
  }

  const renderCellOutput = (state, outputContainer, block) => {
    outputContainer.innerHTML = ''

    if (state.dataframe) {
      outputContainer.appendChild(createDataFrameCard(state.dataframe, {
        onImport: async () => {
          const selected = await open({
            multiple: false,
            filters: [{ name: 'Table', extensions: ['csv', 'tsv', 'json', 'xlsx', 'xls', 'parquet'] }],
          })
          if (!selected || Array.isArray(selected)) return

          const imported = await invoke('python_runtime_import_table_file', { request: { path: String(selected) } })
          if (!imported?.ok || !imported?.table) {
            throw new Error(String(imported?.error || '导入失败'))
          }

          const fileName = String(selected).split(/[\\/]/).pop() || 'df'
          const variableName = sanitizePythonIdentifier(fileName, 'df')
          const code = [
            'from pathlib import Path',
            `__guaxi_table_path = ${JSON.stringify(String(selected))}`,
            `__guaxi_var_name = ${JSON.stringify(variableName)}`,
            '__guaxi_ext = Path(__guaxi_table_path).suffix.lower()',
            'import pandas as pd',
            "if __guaxi_ext in ('.csv', '.txt'):",
            '    __guaxi_value = pd.read_csv(__guaxi_table_path)',
            "elif __guaxi_ext == '.tsv':",
            "    __guaxi_value = pd.read_csv(__guaxi_table_path, sep='\\t')",
            "elif __guaxi_ext in ('.xlsx', '.xls'):",
            '    __guaxi_value = pd.read_excel(__guaxi_table_path)',
            "elif __guaxi_ext == '.json':",
            '    __guaxi_value = pd.read_json(__guaxi_table_path)',
            "elif __guaxi_ext == '.parquet':",
            '    __guaxi_value = pd.read_parquet(__guaxi_table_path)',
            'else:',
            "    raise ValueError(f'Unsupported table format: {__guaxi_ext or __guaxi_table_path}')",
            'globals()[__guaxi_var_name] = __guaxi_value',
            '_result = __guaxi_value',
          ].join('\n')

          const filePath = getFilePath?.() || ''
          const sessionId = createPythonSessionId(filePath)
          const res = await invoke('python_exec', {
            request: {
              code,
              persist: true,
              expect_json: true,
              vars: {
                _guaxi_session: sessionId,
                _doc_path: filePath,
                _py_cell_id: createPythonCellId(filePath, block),
              },
            },
          })
          setStateOutput(state, res)
          renderCellOutput(state, outputContainer, block)
          message?.success?.(`表格已导入到会话变量 ${variableName}`)
        },
        onExport: async (format) => saveTableToFile(state.dataframe, format),
        onInsertMarkdown: async () => {
          const markdownTable = dataframeToMarkdownTable(state.dataframe)
          if (!markdownTable) {
            message?.warning?.('当前没有可插入的表格数据')
            return
          }
          const currentMarkdown = applyDrafts('pycell-insert-table-sync')
          const blocks = parsePythonCellBlocks(currentMarkdown)
          const currentBlock = blocks.find((item) => createPythonCellId(getFilePath?.() || '', item) === createPythonCellId(getFilePath?.() || '', block))
          const cellId = createPythonCellId(getFilePath?.() || '', currentBlock || block)
          const nextMarkdown = writeMarkdownOutputBlock(currentMarkdown, currentBlock || block, cellId, markdownTable)
          applyMarkdown?.(nextMarkdown, { action: 'pycell-insert-table-markdown' })
          message?.success?.('Markdown 表格已插入到文档')
        },
      }))
      return
    }

    if (state.display?.type === 'html' && state.display.html) {
      const box = document.createElement('div')
      box.className = 'guaxi-pycell-card__rich guaxi-pycell-card__rich--html'
      box.innerHTML = state.display.html
      outputContainer.appendChild(box)
    } else if (state.display?.type === 'svg' && state.display.svg) {
      const box = document.createElement('div')
      box.className = 'guaxi-pycell-card__rich guaxi-pycell-card__rich--svg'
      box.innerHTML = state.display.svg
      outputContainer.appendChild(box)
    } else if (state.display?.type === 'markdown' && state.display.markdown) {
      const pre = document.createElement('pre')
      pre.className = 'guaxi-pycell-card__rich guaxi-pycell-card__rich--markdown'
      pre.textContent = state.display.markdown
      outputContainer.appendChild(pre)
    } else if (state.display?.type === 'image') {
      const src = state.display.dataUrl && String(state.display.dataUrl).startsWith('data:')
        ? state.display.dataUrl
        : state.display.data_base64
          ? `data:${String(state.display.mime || 'image/png')};base64,${String(state.display.data_base64)}`
          : ''
      if (src) {
        const img = document.createElement('img')
        img.className = 'guaxi-pycell-card__image'
        img.src = src
        outputContainer.appendChild(img)
      }
    }

    if (String(state.outputText || '').trim()) {
      const pre = document.createElement('pre')
      pre.className = 'guaxi-pycell-card__stdout'
      pre.textContent = state.outputText
      outputContainer.appendChild(pre)
    }

    if (!outputContainer.childNodes.length) {
      const empty = document.createElement('div')
      empty.className = 'guaxi-pycell-card__empty'
      empty.textContent = '当前还没有输出结果'
      outputContainer.appendChild(empty)
    }
  }

  const renderSessionVars = (state, sessionContainer) => {
    sessionContainer.innerHTML = ''
    const vars = Array.isArray(state.sessionVars) ? state.sessionVars : []
    if (!vars.length) {
      const empty = document.createElement('div')
      empty.className = 'guaxi-pycell-card__empty'
      empty.textContent = '当前会话中还没有可见变量'
      sessionContainer.appendChild(empty)
      return
    }

    vars.forEach((item) => {
      const row = document.createElement('div')
      row.className = 'guaxi-pycell-card__var'
      row.innerHTML = `
        <div class="guaxi-pycell-card__var-main">
          <span class="guaxi-pycell-card__var-name">${escapeHtml(item?.name || '')}</span>
          <span class="guaxi-pycell-card__var-type">${escapeHtml(item?.value_type || item?.type || '')}</span>
        </div>
        <div class="guaxi-pycell-card__var-preview">${escapeHtml(item?.preview || '')}</div>
      `
      sessionContainer.appendChild(row)
    })
  }

  const runCell = async (block, state, outputContainer, sessionContainer, { writeBack = false } = {}) => {
    if (state.busy) return
    const code = String(state.draftCode ?? block.code ?? '')
    if (!code.trim()) {
      message?.warning?.('Python Cell 代码为空')
      return
    }

    state.busy = true
    renderCellOutput({ ...state, outputText: '运行中...' }, outputContainer, block)

    try {
      const filePath = getFilePath?.() || ''
      const cellId = createPythonCellId(filePath, block)
      const sessionId = createPythonSessionId(filePath)
      logger?.info?.('[PyCellEditor] executing cell', { cellId, sessionId, codeLen: code.length })

      const result = await invoke('python_exec', {
        request: {
          code,
          persist: true,
          expect_json: true,
          vars: {
            _guaxi_session: sessionId,
            _doc_path: filePath,
            _py_cell_id: cellId,
          },
        },
      })

      setStateOutput(state, result)
      renderCellOutput(state, outputContainer, block)
      renderSessionVars(state, sessionContainer)

      if (writeBack) {
        const markdownOutput = buildMarkdownOutputFromState(state)
        if (!markdownOutput) {
          message?.warning?.('当前结果暂时无法转换为 Markdown')
          return
        }
        const currentMarkdown = applyDrafts('pycell-run-writeback-sync')
        const blocks = parsePythonCellBlocks(currentMarkdown)
        const currentBlock = blocks.find((item) => createPythonCellId(filePath, item) === cellId)
        const nextMarkdown = writeMarkdownOutputBlock(currentMarkdown, currentBlock || block, cellId, markdownOutput)
        applyMarkdown?.(nextMarkdown, { action: 'pycell-run-writeback' })
        message?.success?.('Python Cell 输出已写回 Markdown')
        return
      }

      message?.success?.('Python Cell 运行完成')
    } catch (error) {
      state.outputText = `运行失败：${String(error?.message || error)}`
      state.display = null
      state.dataframe = null
      renderCellOutput(state, outputContainer, block)
      logger?.error?.('[PyCellEditor] cell execution failed', error)
      message?.error?.(`运行失败：${String(error?.message || error)}`)
    } finally {
      state.busy = false
    }
  }

  const createCardDom = (block) => {
    const filePath = getFilePath?.() || ''
    const cellId = createPythonCellId(filePath, block)
    const existing = cellStates.get(cellId) || {}
    const state = {
      draftCode: typeof existing.draftCode === 'string' ? existing.draftCode : block.code,
      busy: false,
      meta: normalizePythonCellMeta({
        ...(block.meta || {}),
        ...(existing.meta || {}),
      }),
      outputText: existing.outputText || '',
      display: existing.display || null,
      dataframe: existing.dataframe || null,
      sessionVars: existing.sessionVars || [],
      result: existing.result || null,
    }
    cellStates.set(cellId, state)

    const card = document.createElement('section')
    card.className = 'guaxi-pycell-card'
    card.dataset.pyCellId = cellId

    const title = state.meta.title || `Python Cell · 第 ${block.startLine} 行`
    card.innerHTML = `
      <div class="guaxi-pycell-card__header">
        <div class="guaxi-pycell-card__title-wrap">
          <div class="guaxi-pycell-card__eyebrow">Python Cell</div>
          <div class="guaxi-pycell-card__title">${escapeHtml(title)}</div>
          <div class="guaxi-pycell-card__meta">文件级会话 · 隐藏源码块 · 关闭文件后销毁沙箱</div>
        </div>
        <div class="guaxi-pycell-card__actions">
          <button type="button" class="guaxi-pycell-card__btn" data-action="run">运行</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="run-writeback">运行并写回</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="toggle-code">${state.meta.codeCollapsed ? '显示代码' : '隐藏代码'}</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="toggle-output">${state.meta.outputCollapsed ? '显示输出' : '隐藏输出'}</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="toggle-session">${state.meta.sessionCollapsed ? '显示变量' : '隐藏变量'}</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="open-console">控制台</button>
        </div>
      </div>
      <div class="guaxi-pycell-card__body">
        <div class="guaxi-pycell-card__code-shell ${state.meta.codeCollapsed ? 'is-collapsed' : ''}">
          <textarea class="guaxi-pycell-card__editor" spellcheck="false"></textarea>
        </div>
        <div class="guaxi-pycell-card__panel ${state.meta.outputCollapsed ? 'is-collapsed' : ''}" data-panel="output"></div>
        <div class="guaxi-pycell-card__panel ${state.meta.sessionCollapsed ? 'is-collapsed' : ''}" data-panel="session"></div>
      </div>
    `

    const textarea = card.querySelector('.guaxi-pycell-card__editor')
    const outputContainer = card.querySelector('[data-panel="output"]')
    const sessionContainer = card.querySelector('[data-panel="session"]')
    const codeShell = card.querySelector('.guaxi-pycell-card__code-shell')
    textarea.value = state.draftCode

    const resizeTextarea = () => {
      textarea.style.height = 'auto'
      textarea.style.height = `${Math.max(180, textarea.scrollHeight + 4)}px`
    }

    textarea.addEventListener('input', () => {
      state.draftCode = textarea.value
      resizeTextarea()
    })

    textarea.addEventListener('keydown', (event) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
        event.preventDefault()
        void runCell(block, state, outputContainer, sessionContainer, { writeBack: false })
      }
    })

    const togglePanel = (panel, key) => {
      state.meta[key] = !state.meta[key]
      panel.classList.toggle('is-collapsed', !!state.meta[key])
      if (key === 'codeCollapsed') {
        codeShell.classList.toggle('is-collapsed', !!state.meta[key])
      }
      scheduleRefresh()
    }

    card.querySelectorAll('[data-action]').forEach((button) => {
      button.addEventListener('click', async (event) => {
        event.preventDefault()
        const action = button.getAttribute('data-action')
        if (action === 'run') {
          await runCell(block, state, outputContainer, sessionContainer, { writeBack: false })
          return
        }
        if (action === 'run-writeback') {
          await runCell(block, state, outputContainer, sessionContainer, { writeBack: true })
          return
        }
        if (action === 'toggle-code') {
          togglePanel(codeShell, 'codeCollapsed')
          button.textContent = state.meta.codeCollapsed ? '显示代码' : '隐藏代码'
          return
        }
        if (action === 'toggle-output') {
          togglePanel(outputContainer, 'outputCollapsed')
          button.textContent = state.meta.outputCollapsed ? '显示输出' : '隐藏输出'
          return
        }
        if (action === 'toggle-session') {
          togglePanel(sessionContainer, 'sessionCollapsed')
          button.textContent = state.meta.sessionCollapsed ? '显示变量' : '隐藏变量'
          return
        }
        if (action === 'open-console') {
          onRequestConsole?.()
        }
      })
    })

    renderCellOutput(state, outputContainer, block)
    renderSessionVars(state, sessionContainer)
    resizeTextarea()
    return card
  }

  const refresh = () => {
    if (disposed || !editor || refreshInFlight) return
    refreshInFlight = true
    try {
      const markdown = getMarkdown?.()
      const blocks = parsePythonCellBlocks(markdown)

      clearZones()

      if (!blocks.length) {
        lastRefreshSignature = ''
        logger?.debug?.('[PyCellEditor] no explicit python cell blocks found', {
          path: getFilePath?.() || '',
        })
        return
      }

      editor.changeViewZones((accessor) => {
        blocks.forEach((block) => {
          const card = createCardDom(block)
          const zoneId = accessor.addZone({
            // 关键设计：
            // 卡片挂在 Python Cell 源码块之后，这样源码继续可见，卡片也会立即出现在当前编辑位置。
            afterLineNumber: Math.max(0, block.endLine),
            heightInPx: Math.max(280, Math.ceil(card.scrollHeight || card.getBoundingClientRect().height || 280)),
            domNode: card,
            suppressMouseDown: false,
          })
          zoneIds.push(zoneId)
        })
      })

      const nextSignature = `${blocks.length}:${getFilePath?.() || ''}:${blocks.map((item) => `${item.startLine}-${item.endLine}`).join('|')}`
      if (lastRefreshSignature !== nextSignature) {
        lastRefreshSignature = nextSignature
        logger?.info?.('[PyCellEditor] view zones refreshed', {
          count: blocks.length,
          path: getFilePath?.() || '',
          mode: 'source-visible',
        })
      }
      editor?.layout?.()
    } catch (error) {
      clearZones()
      logger?.warn?.('[PyCellEditor] view zone render failed', error)
    } finally {
      refreshInFlight = false
    }
  }

  return {
    refresh,
    flushDrafts,
    applyDrafts,
    dispose() {
      disposed = true
      clearZones()
      cellStates.clear()
    },
  }
}
