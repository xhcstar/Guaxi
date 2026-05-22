import { save, open } from '@tauri-apps/plugin-dialog'
import { writeFile } from '@tauri-apps/plugin-fs'
import {
  buildPythonCellBlock,
  createPythonCellId,
  createPythonSessionId,
  dataframeToMarkdownTable,
  normalizePythonCellMeta,
  parsePythonCellBlocks,
  replacePythonCellBlock,
} from './guaxiPythonCellSyntax'
import {
  deletePythonCellSnapshot,
  getPythonCellSnapshot,
  patchPythonCellSnapshot,
} from './guaxiPythonCellState'

// 设计说明：
// 1. 编辑模式下，Python Cell 以“嵌入式单元格”替代 Markdown 源码区域，整体体验尽量贴近 Jupyter Notebook。
// 2. Markdown 中的 <!--py-->...<!--/py--> 仍然作为真实存储格式存在，但在编辑视图里会被隐藏，由 Cell UI 接管对应区域。
// 3. 卡片中的草稿代码先保存在内存态，保存文件/切换文件前统一 flush 回 Markdown 源文本。
// 4. 输出、变量、表格和“写回 Markdown”都围绕同一个文件级 Python session 运行，确保体验接近 Notebook。

const escapeHtml = (value) => String(value == null ? '' : value)
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const extractHighlightedCodeHtml = (html = '') => {
  const template = document.createElement('template')
  template.innerHTML = String(html || '')
  return template.content.querySelector('code')?.innerHTML || escapeHtml(html)
}

const syncHighlightScroll = (textarea, highlightLayer) => {
  if (!textarea || !highlightLayer) return
  highlightLayer.scrollTop = textarea.scrollTop
  highlightLayer.scrollLeft = textarea.scrollLeft
}

const createPythonCellHighlightController = ({
  textarea,
  highlightLayer,
  invoke,
  logger,
  getCode,
} = {}) => {
  // 设计说明：
  // 1. 探索者模式的 Python Cell 仍保留 textarea 作为真实输入控件，避免重写现有执行、保存和焦点逻辑。
  // 2. tree-sitter 高亮放在底层 pre/code 中，textarea 文字透明但光标可见，从而只替换“源码高亮”这一层。
  // 3. 请求做防抖和序号校验，用户快速输入时只应用最后一次结果，避免旧高亮闪回。
  let disposed = false
  let timer = null
  let requestSeq = 0
  let lastCode = ''

  const renderPlain = (code) => {
    if (!highlightLayer) return
    highlightLayer.innerHTML = `<code>${escapeHtml(code || '')}</code>`
  }

  const schedule = (reason = 'input') => {
    if (disposed) return
    const code = String(getCode?.() || '')
    if (code === lastCode && reason !== 'init') return
    lastCode = code
    renderPlain(code)
    clearTimeout(timer)
    timer = setTimeout(async () => {
      if (disposed) return
      const seq = ++requestSeq
      const startedAt = performance.now()
      logger?.debug?.('[PyCellEditor][TreeSitterHighlight] request start', {
        frontendRequestId: seq,
        reason,
        codeLen: code.length,
      })
      try {
        const response = await invoke?.('syntax_highlight_code', {
          request: {
            code,
            language: 'python',
            theme: 'guaxi',
          },
        })
        if (disposed || seq !== requestSeq) return
        highlightLayer.innerHTML = `<code>${extractHighlightedCodeHtml(response?.html || '')}</code>`
        syncHighlightScroll(textarea, highlightLayer)
        logger?.debug?.('[PyCellEditor] tree-sitter highlight applied', {
          frontendRequestId: seq,
          backendDebugId: response?.debug_id,
          reason,
          codeLen: code.length,
          cached: !!response?.cached,
          supported: !!response?.supported,
          engine: response?.engine || '',
          backendElapsedMs: response?.elapsed_ms,
          frontendElapsedMs: Math.round(performance.now() - startedAt),
        })
      } catch (error) {
        if (seq === requestSeq) {
          logger?.warn?.('[PyCellEditor][TreeSitterHighlight] request failed, fallback to plain text', {
            frontendRequestId: seq,
            reason,
            codeLen: code.length,
            frontendElapsedMs: Math.round(performance.now() - startedAt),
            error,
          })
          renderPlain(code)
        }
      }
    }, reason === 'init' ? 0 : 180)
  }

  return {
    schedule,
    dispose() {
      disposed = true
      clearTimeout(timer)
    },
  }
}

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

const attachPressHandler = (element, handler) => {
  if (!element || typeof handler !== 'function') return
  let keyboardTrigger = false

  element.addEventListener('pointerdown', (event) => {
    if (event.__guaxiPressHandled) return
    if (event.button !== 0) return
    event.__guaxiPressHandled = true
    event.preventDefault()
    event.stopPropagation()
    void handler(event)
  })

  element.addEventListener('keydown', (event) => {
    if (event.__guaxiPressHandled) return
    if (event.key !== 'Enter' && event.key !== ' ') return
    keyboardTrigger = true
    event.__guaxiPressHandled = true
    event.preventDefault()
    event.stopPropagation()
    void handler(event)
  })

  element.addEventListener('click', (event) => {
    if (event.__guaxiPressHandled) return
    event.__guaxiPressHandled = true
    event.preventDefault()
    event.stopPropagation()
    if (keyboardTrigger) {
      keyboardTrigger = false
    }
  })
}

const stopEditorEvent = (event) => {
  if (!event) return
  event.stopPropagation?.()
  if (typeof event.stopImmediatePropagation === 'function') {
    event.stopImmediatePropagation()
  }
}

const consumeContextMenuEvent = (event) => {
  if (!event) return
  event.preventDefault?.()
  stopEditorEvent(event)
}

const promoteZoneInteractivity = (card) => {
  if (!card) return
  requestAnimationFrame(() => {
    let node = card
    let depth = 0
    while (node && depth < 5) {
      if (node.style) {
        node.style.pointerEvents = 'auto'
        if (depth > 0 && !node.style.zIndex) node.style.zIndex = '5'
      }
      if (node.classList?.contains('view-zones')) break
      node = node.parentElement
      depth += 1
    }
  })
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
    attachPressHandler(btn, () => handler?.())
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
  onStateChange,
}) => {
  let disposed = false
  let zoneIds = []
  let contentWidgets = []
  let overlayRoot = null
  const cellStates = new Map()
  const managerDisposables = []
  const highlightControllers = []
  let refreshScheduled = false
  let refreshInFlight = false
  let lastRefreshSignature = ''
  let pendingFocusCellId = ''

  try {
    managerDisposables.push(editor?.onDidScrollChange?.(() => scheduleRefresh()))
    managerDisposables.push(editor?.onDidLayoutChange?.(() => scheduleRefresh()))
  } catch {
  }

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

  const clearContentWidgets = () => {
    if (!editor || !contentWidgets.length) return
    contentWidgets.forEach((widget) => {
      try {
        editor.removeContentWidget(widget)
      } catch {
      }
    })
    contentWidgets = []
  }

  const ensureOverlayRoot = () => {
    if (overlayRoot?.isConnected) return overlayRoot
    const editorDomNode = editor?.getDomNode?.()
    const host = editorDomNode?.closest?.('.editor-container') || editorDomNode?.parentElement
    if (!host) return null
    let root = host.querySelector(':scope > .guaxi-pycell-overlay-root')
    if (!(root instanceof HTMLElement)) {
      root = document.createElement('div')
      root.className = 'guaxi-pycell-overlay-root'
      host.appendChild(root)
    }
    root.style.position = 'absolute'
    root.style.inset = '0'
    root.style.pointerEvents = 'none'
    root.style.zIndex = '40'
    root.style.overflow = 'visible'
    overlayRoot = root
    return overlayRoot
  }

  const clearOverlayCards = () => {
    highlightControllers.forEach((item) => {
      try {
        item?.dispose?.()
      } catch {
      }
    })
    highlightControllers.length = 0
    if (overlayRoot) {
      overlayRoot.innerHTML = ''
    }
  }

  const clearHiddenAreas = () => {
    try {
      editor.setHiddenAreas([], 'guaxi-python-cells')
    } catch {
    }
  }

  const scheduleRefresh = () => {
    if (disposed || refreshScheduled) return
    refreshScheduled = true
    requestAnimationFrame(() => {
      refreshScheduled = false
      refresh()
    })
  }

  const syncSnapshot = (cellId, state, extra = {}) => {
    const snapshot = patchPythonCellSnapshot(getFilePath?.() || '', cellId, {
      draftCode: state?.draftCode ?? '',
      outputText: state?.outputText || '',
      display: state?.display || null,
      dataframe: state?.dataframe || null,
      sessionVars: Array.isArray(state?.sessionVars) ? state.sessionVars : [],
      result: state?.result || null,
      meta: normalizePythonCellMeta(state?.meta || {}),
      ...extra,
    })
    onStateChange?.(cellId, snapshot)
    return snapshot
  }

  const withCurrentBlock = (block, action, handler) => {
    const currentMarkdown = applyDrafts(`${action}-sync`)
    const filePath = getFilePath?.() || ''
    const blocks = parsePythonCellBlocks(currentMarkdown)
    const currentCellId = createPythonCellId(filePath, block)
    const currentBlock = blocks.find((item) => createPythonCellId(filePath, item) === currentCellId)
    if (!currentBlock) return null
    return handler({ currentMarkdown, filePath, blocks, currentBlock, currentCellId })
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

  const estimateCardHeight = (state) => {
    const codeLineCount = Math.max(
      4,
      String(state?.draftCode || '')
        .replace(/\r\n?/g, '\n')
        .split('\n')
        .length,
    )

    let nextHeight = 126 + Math.min(codeLineCount, 18) * 24

    if (!state?.meta?.outputCollapsed) {
      if (state?.dataframe) {
        nextHeight += Math.min(260, 108 + Math.max(0, Number(state.dataframe.rowCount || 0)) * 26)
      } else if (state?.display?.type === 'image') {
        nextHeight += 236
      } else {
        const outputLineCount = Math.max(
          2,
          String(state?.outputText || state?.display?.markdown || '')
            .replace(/\r\n?/g, '\n')
            .split('\n')
            .length,
        )
        nextHeight += Math.min(220, 44 + outputLineCount * 20)
      }
    }

    if (!state?.meta?.sessionCollapsed) {
      nextHeight += Math.min(220, 72 + (Array.isArray(state?.sessionVars) ? state.sessionVars.length : 0) * 36)
    }

    return Math.max(280, nextHeight)
  }

  const createSpacerDom = (heightInPx) => {
    const spacer = document.createElement('div')
    spacer.className = 'guaxi-pycell-spacer'
    spacer.style.height = `${Math.max(1, Math.ceil(heightInPx || 1))}px`
    spacer.style.pointerEvents = 'none'
    return spacer
  }

  const mountOverlayCard = (block, card, state) => {
    const root = ensureOverlayRoot()
    if (!root) return

    const wrapper = document.createElement('div')
    wrapper.className = 'guaxi-pycell-overlay-card'

    const layout = editor?.getLayoutInfo?.()
    const contentLeft = Math.max(0, Number(layout?.contentLeft || 0))
    const contentWidth = Math.max(320, Number(layout?.contentWidth || 640) - 24)
    const scrollTop = Number(editor?.getScrollTop?.() || 0)
    const lineHeight = Number(layout?.lineHeight || editor?.getOption?.(67) || 22)
    const model = editor?.getModel?.()
    const lineCount = Math.max(1, Number(model?.getLineCount?.() || 1))
    const startLine = Math.max(1, Math.min(lineCount, Number(block?.startLine || 1)))
    const top = startLine > 1
      ? Number(editor?.getTopForLineNumber?.(startLine - 1) || 0) - scrollTop + lineHeight
      : 0 - scrollTop

    wrapper.style.left = `${contentLeft + 8}px`
    wrapper.style.top = `${Math.max(0, top)}px`
    wrapper.style.width = `${contentWidth}px`
    wrapper.style.position = 'absolute'
    wrapper.style.pointerEvents = 'auto'
    wrapper.style.zIndex = '30'

    card.style.width = '100%'
    card.style.maxWidth = '100%'
    card.style.pointerEvents = 'auto'
    ;['pointerdown', 'mousedown', 'mouseup', 'click', 'dblclick'].forEach((eventName) => {
      wrapper.addEventListener(eventName, (event) => {
        const target = event?.target
        if (!(target instanceof HTMLElement)) return
        if (!target.closest('.guaxi-pycell-card')) return
        stopEditorEvent(event)
      })
    })
    wrapper.addEventListener('contextmenu', (event) => {
      const target = event?.target
      if (!(target instanceof HTMLElement)) return
      if (!target.closest('.guaxi-pycell-card')) return
      consumeContextMenuEvent(event)
    })
    wrapper.appendChild(card)
    root.appendChild(wrapper)
  }

  const createContentWidget = (block, card, state) => {
    const filePath = getFilePath?.() || ''
    const cellId = createPythonCellId(filePath, block)
    const model = editor?.getModel?.()

    const applyLayout = () => {
      const layout = editor?.getLayoutInfo?.()
      const width = Math.max(320, Number(layout?.contentWidth || 640) - 24)
      card.style.width = `${width}px`
      card.style.maxWidth = `${width}px`
    }

    const resolveAnchor = () => {
      const lineCount = Math.max(1, Number(model?.getLineCount?.() || 1))
      const startLine = Math.max(1, Math.min(lineCount, Number(block?.startLine || 1)))
      const endLine = Math.max(startLine, Math.min(lineCount, Number(block?.endLine || startLine)))

      if (startLine > 1) {
        const anchorLine = startLine - 1
        return {
          position: {
            lineNumber: anchorLine,
            column: Number(model?.getLineMaxColumn?.(anchorLine) || 1),
          },
          preference: [monaco.editor.ContentWidgetPositionPreference.BELOW],
        }
      }

      if (endLine < lineCount) {
        return {
          position: { lineNumber: endLine + 1, column: 1 },
          preference: [monaco.editor.ContentWidgetPositionPreference.ABOVE],
        }
      }

      return {
        position: { lineNumber: 1, column: 1 },
        preference: [monaco.editor.ContentWidgetPositionPreference.EXACT],
      }
    }

    applyLayout()

    return {
      allowEditorOverflow: true,
      suppressMouseDown: true,
      getId() {
        return `guaxi.pycell.widget.${cellId}`
      },
      getDomNode() {
        applyLayout()
        return card
      },
      getPosition() {
        return resolveAnchor()
      },
      beforeRender() {
        applyLayout()
        return {
          width: Math.max(320, Number(editor?.getLayoutInfo?.()?.contentWidth || 640) - 24),
          height: estimateCardHeight(state),
        }
      },
      afterRender() {
        applyLayout()
      },
    }
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
          // 设计说明：
          // 导入表格属于“把数据带进当前会话”的动作，这里保留 dataframe 结果卡片和变量刷新，
          // 但不额外输出“已导入到变量 xxx”的文本，避免把真正的表格结果往下挤。
          state.outputText = ''
          renderCellOutput(state, outputContainer, block)
          logger?.info?.('[PyCellEditor] table imported into session', { variableName, sourcePath: String(selected) })
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
    const filePath = getFilePath?.() || ''
    const cellId = createPythonCellId(filePath, block)
    if (!code.trim()) {
      message?.warning?.('Python Cell 代码为空')
      return
    }

    const card = outputContainer?.closest?.('.guaxi-pycell-card')
    state.busy = true
    card?.classList?.add('is-running')
    syncSnapshot(cellId, state)
    renderCellOutput({ ...state, outputText: '运行中...' }, outputContainer, block)

    try {
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
      syncSnapshot(cellId, state)
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
      state.sessionVars = []
      state.result = null
      syncSnapshot(cellId, state)
      renderCellOutput(state, outputContainer, block)
      logger?.error?.('[PyCellEditor] cell execution failed', error)
      message?.error?.(`运行失败：${String(error?.message || error)}`)
    } finally {
      state.busy = false
      card?.classList?.remove('is-running')
      syncSnapshot(cellId, state)
    }
  }

  const createCardDom = (block) => {
    const filePath = getFilePath?.() || ''
    const cellId = createPythonCellId(filePath, block)
    const existing = cellStates.get(cellId) || {}
    const persisted = getPythonCellSnapshot(filePath, cellId) || {}
    const state = {
      draftCode: typeof existing.draftCode === 'string'
        ? existing.draftCode
        : typeof persisted.draftCode === 'string'
          ? persisted.draftCode
          : block.code,
      busy: false,
      meta: normalizePythonCellMeta({
        ...(block.meta || {}),
        ...(existing.meta || {}),
        ...(persisted.meta || {}),
      }),
      outputText: persisted.outputText || existing.outputText || '',
      display: persisted.display || existing.display || null,
      dataframe: persisted.dataframe || existing.dataframe || null,
      sessionVars: persisted.sessionVars || existing.sessionVars || [],
      result: persisted.result || existing.result || null,
    }
    cellStates.set(cellId, state)
    syncSnapshot(cellId, state)

    const card = document.createElement('section')
    card.className = 'guaxi-pycell-card'
    card.dataset.pyCellId = cellId
    card.style.pointerEvents = 'auto'
    card.style.position = 'relative'
    card.style.zIndex = '6'

    const title = state.meta.title || `Python Cell`
    card.innerHTML = `
      <div class="guaxi-pycell-card__header">
        <div class="guaxi-pycell-card__prompt">In [ ]</div>
        <div class="guaxi-pycell-card__title-wrap">
          <div class="guaxi-pycell-card__title">${escapeHtml(title)}</div>
          <div class="guaxi-pycell-card__meta">文件级会话 · 关闭文件后销毁沙箱</div>
        </div>
        <div class="guaxi-pycell-card__actions">
          <button type="button" class="guaxi-pycell-card__btn" data-action="insert-above">上方插入</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="insert-below">下方插入</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="run">运行</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="run-writeback">运行并写回</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="toggle-output">${state.meta.outputCollapsed ? '显示输出' : '隐藏输出'}</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="toggle-session">${state.meta.sessionCollapsed ? '显示变量' : '隐藏变量'}</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="open-console">控制台</button>
          <button type="button" class="guaxi-pycell-card__btn" data-action="delete-cell">删除</button>
        </div>
      </div>
      <div class="guaxi-pycell-card__body">
        <div class="guaxi-pycell-card__code-shell">
          <pre class="guaxi-pycell-card__highlight" aria-hidden="true"><code></code></pre>
          <textarea class="guaxi-pycell-card__editor" spellcheck="false"></textarea>
        </div>
        <div class="guaxi-pycell-card__panel guaxi-pycell-card__panel--output ${state.meta.outputCollapsed ? 'is-collapsed' : ''}" data-panel="output"></div>
        <div class="guaxi-pycell-card__panel guaxi-pycell-card__panel--session ${state.meta.sessionCollapsed ? 'is-collapsed' : ''}" data-panel="session"></div>
      </div>
    `

    const textarea = card.querySelector('.guaxi-pycell-card__editor')
    const highlightLayer = card.querySelector('.guaxi-pycell-card__highlight')
    const outputContainer = card.querySelector('[data-panel="output"]')
    const sessionContainer = card.querySelector('[data-panel="session"]')
    const metaEl = card.querySelector('.guaxi-pycell-card__meta')
    textarea.value = state.draftCode
    textarea.style.pointerEvents = 'auto'
    const highlightController = createPythonCellHighlightController({
      textarea,
      highlightLayer,
      invoke,
      logger,
      getCode: () => textarea.value,
    })
    highlightControllers.push(highlightController)

    const isInteractiveTarget = (target) => {
      if (!(target instanceof HTMLElement)) return false
      return !!target.closest('button, textarea, input, select, option, a, [data-action]')
    }

    ;['pointerdown', 'mousedown', 'mouseup', 'click', 'dblclick'].forEach((eventName) => {
      card.addEventListener(eventName, (event) => {
        if (!isInteractiveTarget(event.target)) return
        event.stopPropagation()
      }, true)
    })
    card.addEventListener('contextmenu', (event) => {
      consumeContextMenuEvent(event)
    }, true)

    const resizeTextarea = () => {
      textarea.style.height = 'auto'
      textarea.style.height = `${Math.max(96, textarea.scrollHeight + 4)}px`
      scheduleRefresh()
    }

    textarea.addEventListener('input', () => {
      state.draftCode = textarea.value
      syncSnapshot(cellId, state)
      highlightController.schedule('input')
      resizeTextarea()
    })
    textarea.addEventListener('scroll', () => syncHighlightScroll(textarea, highlightLayer))

    textarea.addEventListener('keydown', (event) => {
      if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
        event.preventDefault()
        void runCell(block, state, outputContainer, sessionContainer, { writeBack: false })
      }
    })

    const togglePanel = (panel, key) => {
      state.meta[key] = !state.meta[key]
      panel.classList.toggle('is-collapsed', !!state.meta[key])
      syncSnapshot(cellId, state)
      scheduleRefresh()
    }

    const focusEditorSoon = () => {
      requestAnimationFrame(() => {
        try {
          textarea.focus()
          const length = textarea.value.length
          textarea.setSelectionRange(length, length)
        } catch {
        }
      })
    }

    const insertRelativeCell = (direction = 'below') => {
      const result = withCurrentBlock(block, `pycell-insert-${direction}`, ({ currentMarkdown, currentBlock, blocks, filePath }) => {
        const nextLines = String(currentMarkdown || '').replace(/\r\n?/g, '\n').split('\n')
        const newBlockText = buildPythonCellBlock({
          code: '',
          meta: { title: '', sessionCollapsed: true },
        })
        const insertionLines = [...newBlockText.split('\n'), '']
        const insertAt = direction === 'above' ? currentBlock.startLine - 1 : currentBlock.endLine
        nextLines.splice(insertAt, 0, ...insertionLines)
        const nextMarkdown = nextLines.join('\n')
        const nextBlocks = parsePythonCellBlocks(nextMarkdown)
        const currentIndex = blocks.findIndex((item) => createPythonCellId(filePath, item) === createPythonCellId(filePath, currentBlock))
        const insertedBlock = nextBlocks[direction === 'above' ? currentIndex : currentIndex + 1]
        const focusCellId = insertedBlock ? createPythonCellId(filePath, insertedBlock) : ''
        if (focusCellId) pendingFocusCellId = focusCellId
        applyMarkdown?.(nextMarkdown, { action: `pycell-insert-${direction}` })
        return focusCellId
      })
      if (result) {
        requestAnimationFrame(() => refresh())
      }
    }

    const deleteCurrentCell = () => {
      withCurrentBlock(block, 'pycell-delete', ({ currentMarkdown, currentBlock, blocks, currentCellId, filePath }) => {
        const nextLines = String(currentMarkdown || '').replace(/\r\n?/g, '\n').split('\n')
        let deleteStart = currentBlock.startLine - 1
        let deleteEnd = currentBlock.endLine - 1
        while (deleteEnd + 1 < nextLines.length && String(nextLines[deleteEnd + 1] || '').trim() === '') {
          deleteEnd += 1
          break
        }
        nextLines.splice(deleteStart, deleteEnd - deleteStart + 1)
        const nextMarkdown = nextLines.join('\n')
        const nextBlocks = parsePythonCellBlocks(nextMarkdown)
        const currentIndex = blocks.findIndex((item) => createPythonCellId(filePath, item) === currentCellId)
        const fallbackBlock = nextBlocks[currentIndex] || nextBlocks[currentIndex - 1] || null
        pendingFocusCellId = fallbackBlock ? createPythonCellId(filePath, fallbackBlock) : ''
        cellStates.delete(currentCellId)
        deletePythonCellSnapshot(filePath, currentCellId)
        applyMarkdown?.(nextMarkdown, { action: 'pycell-delete' })
        onStateChange?.(currentCellId, null)
        requestAnimationFrame(() => refresh())
      })
    }

    const handleActionFromButton = async (button) => {
      if (!(button instanceof HTMLElement)) return
      const action = button.getAttribute('data-action')
      if (action === 'insert-above') {
        insertRelativeCell('above')
        return
      }
      if (action === 'insert-below') {
        insertRelativeCell('below')
        return
      }
      if (action === 'run') {
        await runCell(block, state, outputContainer, sessionContainer, { writeBack: false })
        return
      }
      if (action === 'run-writeback') {
        await runCell(block, state, outputContainer, sessionContainer, { writeBack: true })
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
        return
      }
      if (action === 'delete-cell') {
        deleteCurrentCell()
      }
    }

    card.querySelectorAll('[data-action]').forEach((button) => {
      button.addEventListener('mousedown', (event) => {
        stopEditorEvent(event)
      })
      button.addEventListener('click', async (event) => {
        event.preventDefault?.()
        stopEditorEvent(event)
        if (metaEl) metaEl.textContent = `click:${button.getAttribute('data-action') || ''}`
        await handleActionFromButton(button)
      })
    })

    renderCellOutput(state, outputContainer, block)
    renderSessionVars(state, sessionContainer)
    highlightController.schedule('init')
    resizeTextarea()
    if (pendingFocusCellId && pendingFocusCellId === cellId) {
      pendingFocusCellId = ''
      focusEditorSoon()
    }
    return card
  }

  const refresh = () => {
    if (disposed || !editor || refreshInFlight) return
    refreshInFlight = true
    try {
      const markdown = getMarkdown?.()
      const blocks = parsePythonCellBlocks(markdown)

      clearOverlayCards()
      clearContentWidgets()
      clearZones()
      clearHiddenAreas()

      if (!blocks.length) {
        lastRefreshSignature = ''
        logger?.debug?.('[PyCellEditor] no explicit python cell blocks found', {
          path: getFilePath?.() || '',
        })
        return
      }

      const hiddenRanges = []
      editor.changeViewZones((accessor) => {
        const model = editor.getModel()
        blocks.forEach((block) => {
          const card = createCardDom(block)
          const cellId = createPythonCellId(getFilePath?.() || '', block)
          const state = cellStates.get(cellId)
          mountOverlayCard(block, card, state)
          const zoneId = accessor.addZone({
            // 关键设计：
            // view zone 插在 Cell 起始行之前，同时隐藏源码区间，实现“单元格接管原区域”的嵌入式效果。
            afterLineNumber: Math.max(0, block.startLine - 1),
            // 根因说明：
            // Monaco 会检查 zone 后面的 model position 是否可见；而我们紧接着隐藏了整个 Cell 源码区间，
            // 导致这个 zone 被判定为“位于 hidden area 内”，最终高度变成 0、界面看起来像“没有 UI”。
            // 显式开启 showInHiddenAreas 后，zone 才能真正接管这段被隐藏的源码区域。
            showInHiddenAreas: true,
            heightInPx: estimateCardHeight(state),
            domNode: createSpacerDom(estimateCardHeight(state)),
            suppressMouseDown: false,
          })
          zoneIds.push(zoneId)
          if (model) {
            hiddenRanges.push(new monaco.Range(
              block.startLine,
              1,
              block.endLine,
              model.getLineMaxColumn(block.endLine),
            ))
          }
        })
      })

      editor.setHiddenAreas(hiddenRanges, 'guaxi-python-cells')

      const nextSignature = `${blocks.length}:${getFilePath?.() || ''}:${blocks.map((item) => `${item.startLine}-${item.endLine}`).join('|')}`
      if (lastRefreshSignature !== nextSignature) {
        lastRefreshSignature = nextSignature
        logger?.info?.('[PyCellEditor] view zones refreshed', {
          count: blocks.length,
          path: getFilePath?.() || '',
          mode: 'embedded-cell',
        })
      }
      editor?.layout?.()
    } catch (error) {
      clearZones()
      clearHiddenAreas()
      logger?.warn?.('[PyCellEditor] view zone render failed', error)
    } finally {
      refreshInFlight = false
    }
  }

  return {
    refresh,
    flushDrafts,
    applyDrafts,
    requestFocusCell(cellId) {
      pendingFocusCellId = String(cellId || '').trim()
      if (pendingFocusCellId) scheduleRefresh()
    },
    dispose() {
      disposed = true
      managerDisposables.forEach((item) => {
        try {
          item?.dispose?.()
        } catch {
        }
      })
      managerDisposables.length = 0
      clearOverlayCards()
      clearContentWidgets()
      clearZones()
      clearHiddenAreas()
      cellStates.clear()
    },
  }
}
