// 辅助 Monaco 编辑器（探索者 cell / 模板管理器 / Slidev）与主编辑器共用的一套能力。
//
// 这里只放“与具体组件无关”的部分：
// 1. 读取设置里的字体/字号/小地图；
// 2. 代码补全片段（Markdown / LaTeX / Mermaid）；
// 3. 自定义右键菜单（与主编辑器同一套命令与视觉）；
// 4. Markdown 格式化（后端 comrak + 共享表格对齐）与 LSP 格式化；
// 5. 主题同步与 LSP 启动的便捷入口。
//
// 各组件只需要在 create 时套用 resolveSharedEditorDisplayOptions()，创建后调用
// attachSharedEditorFeatures()，即可获得与主编辑器一致的基础编辑体验。
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'
import {
  DEFAULT_EDITOR_DISPLAY_OPTIONS,
  resolveEditorFontFamily,
  resolveEditorLineHeight,
  useSettingsStore,
} from '../stores/settingsStore'
import { onGuaxiMonacoCodeThemeReapply } from './guaxiMonacoThemes'
import { formatMarkdownTablesInText } from '../utils/markdownTableFormat'
import {
  initLspClient,
  requestLspDocumentFormatting,
  supportsLspDocumentFormatting,
} from './lspClient'
import { buildLatexSnippets, buildMarkdownSnippets, buildMermaidSnippets } from './editorSnippets'

const logger = createLogger('EditorFeatures')

const registeredSnippetLanguages = new Set()

/** 设置页里「编辑器外观」的字体/字号/小地图，供各 Monaco 实例 create 时复用。 */
export const resolveSharedEditorDisplayOptions = () => {
  const store = useSettingsStore()
  const display = { ...DEFAULT_EDITOR_DISPLAY_OPTIONS, ...(store.editorDisplay || {}) }
  const rawSize = Number(display.font_size)
  const fontSize = Number.isFinite(rawSize) && rawSize > 0
    ? Math.round(rawSize)
    : DEFAULT_EDITOR_DISPLAY_OPTIONS.font_size

  return {
    fontFamily: resolveEditorFontFamily(display.font_family),
    fontSize,
    lineHeight: resolveEditorLineHeight(fontSize),
    minimap: {
      enabled: !!display.minimap_enabled,
      showSlider: 'always',
      size: 'fit',
    },
  }
}

const buildSnippetsForLanguage = (monaco, languageId) => {
  switch (languageId) {
    case 'markdown':
      return buildMarkdownSnippets(monaco)
    case 'latex':
      return buildLatexSnippets(monaco)
    case 'mermaid':
      return buildMermaidSnippets(monaco)
    default:
      return []
  }
}

/**
 * 为某种语言注册片段补全（幂等）。
 *
 * 不设置 triggerCharacters：探索者 cell 刻意关闭了快速建议，避免写正文时反复弹补全框；
 * 片段通过 Ctrl+Space 手动唤出，右键菜单里也提供了常用片段的直接入口。
 */
export const registerSnippetCompletion = (monaco, languageId) => {
  const normalized = String(languageId || '').trim().toLowerCase()
  if (!monaco?.languages || !normalized) return
  if (registeredSnippetLanguages.has(normalized)) return
  // 主编辑器已经注册了 Markdown 补全提供方时不再重复注册：补全按语言生效，
  // 主编辑器那份已经覆盖同窗口里的所有 Markdown 模型（包括 cell / 模板管理器）。
  if (normalized === 'markdown' && globalThis.__guaxiMarkdownCompletionRegistered) return

  const snippets = buildSnippetsForLanguage(monaco, normalized)
  if (!snippets.length) return

  registeredSnippetLanguages.add(normalized)

  monaco.languages.registerCompletionItemProvider(normalized, {
    provideCompletionItems(model, position) {
      // 注册顺序不确定（cell 可能先于主编辑器挂载），这里再兜底判一次：
      // 主编辑器提供方已就绪时返回空列表，避免建议框里出现两份完全相同的片段。
      if (normalized === 'markdown' && globalThis.__guaxiMarkdownCompletionRegistered) {
        return { suggestions: [] }
      }
      const lineContent = model.getLineContent(position.lineNumber)
      const beforeCursor = lineContent.slice(0, position.column - 1)
      const typed = normalized === 'latex'
        ? (/(\\[A-Za-z]*)$/.exec(beforeCursor)?.[1] || '').slice(1)
        : (/[\w-]+$/.exec(beforeCursor)?.[0] || '')
      const prefix = typed.toLowerCase()

      const wordStart = position.column - 1 - typed.length
      const range = new monaco.Range(
        position.lineNumber,
        Math.max(1, wordStart + 1),
        position.lineNumber,
        position.column
      )

      const suggestions = snippets
        .filter((item) => !prefix || String(item.key || item.label || '').toLowerCase().startsWith(prefix))
        .slice(0, 120)
        .map((item, index) => ({
          ...item,
          range,
          sortText: `0_${String(index).padStart(3, '0')}_${String(item.key || item.label || '')}`,
        }))

      return { suggestions }
    },
  })

  logger.info('[EditorFeatures] snippet completion registered', { languageId: normalized, count: snippets.length })
}

/** Markdown 格式化：后端 comrak + 共享表格对齐，和主编辑器右键菜单走同一条链路。 */
export const formatMarkdownEditor = async (editor, source = 'shared-editor') => {
  const model = editor?.getModel?.()
  if (!model) return false

  try {
    const store = useSettingsStore()
    const original = model.getValue()
    const response = await invoke('markdown_format', {
      request: {
        content: original,
        options: { ...store.markdownFormat },
      },
    })

    const formatted = response?.formatted_content ?? response?.formattedContent
    if (!response?.success || typeof formatted !== 'string') {
      throw new Error(typeof response?.error === 'string' ? response.error : 'Markdown 格式化失败')
    }

    const aligned = formatMarkdownTablesInText(formatted, source)
    if (aligned === original) return false

    editor.executeEdits('guaxi-markdown-format', [
      {
        range: model.getFullModelRange(),
        text: aligned,
        forceMoveMarkers: true,
      },
    ])
    logger.info('[EditorFeatures] markdown formatted', {
      source,
      originalLength: original.length,
      formattedLength: aligned.length,
    })
    return true
  } catch (error) {
    logger.error('[EditorFeatures] markdown format failed', { source, error })
    return false
  }
}

/** 格式化选中的 Markdown 片段（右键菜单 / 快捷键使用）。 */
export const formatMarkdownSelection = async (editor, source = 'shared-editor') => {
  const model = editor?.getModel?.()
  const selection = editor?.getSelection?.()
  if (!model || !selection || selection.isEmpty()) return false

  try {
    const store = useSettingsStore()
    const original = model.getValueInRange(selection)
    if (!original.trim()) return false

    const response = await invoke('markdown_format', {
      request: {
        content: original,
        options: { ...store.markdownFormat },
      },
    })

    const formatted = response?.formatted_content ?? response?.formattedContent
    if (!response?.success || typeof formatted !== 'string') {
      throw new Error(typeof response?.error === 'string' ? response.error : 'Markdown 格式化失败')
    }

    const aligned = formatMarkdownTablesInText(formatted, source)
    const next = original.endsWith('\n') ? aligned : aligned.replace(/\n$/, '')
    if (next === original) return false

    editor.executeEdits('guaxi-markdown-format-selection', [
      {
        range: selection,
        text: next,
        forceMoveMarkers: true,
      },
    ])
    logger.info('[EditorFeatures] markdown selection formatted', {
      source,
      originalLength: original.length,
      formattedLength: next.length,
    })
    return true
  } catch (error) {
    logger.error('[EditorFeatures] markdown selection format failed', { source, error })
    return false
  }
}

/** LSP 格式化（Python 等由语言服务器声明 formatProvider 的语言）。 */
export const formatEditorWithLsp = async (editor, languageId) => {
  const model = editor?.getModel?.()
  if (!model || !languageId) return false
  if (!supportsLspDocumentFormatting(languageId)) return false

  try {
    const edits = await requestLspDocumentFormatting(languageId, model, {
      tabSize: model.getOptions?.()?.tabSize,
      insertSpaces: model.getOptions?.()?.insertSpaces,
    })
    if (!Array.isArray(edits) || !edits.length) return false

    editor.executeEdits('guaxi-lsp-format', edits.map((edit) => ({
      range: {
        startLineNumber: edit.startLineNumber,
        startColumn: edit.startColumn,
        endLineNumber: edit.endLineNumber,
        endColumn: edit.endColumn,
      },
      text: edit.text,
      forceMoveMarkers: true,
    })))
    logger.info('[EditorFeatures] lsp formatted', { languageId, edits: edits.length })
    return true
  } catch (error) {
    logger.warn('[EditorFeatures] lsp format failed', { languageId, error })
    return false
  }
}

/** LSP 是否对当前语言可用且用户已启用（与主编辑器的开关一致）。 */
export const resolveLspEnabled = async () => {
  try {
    return !!await invoke('get_lsp_syntax_highlight_enabled')
  } catch (error) {
    logger.debug('[EditorFeatures] read lsp setting failed', { error })
    return false
  }
}

/** 启动 LSP 客户端；不满足条件时安静跳过。 */
export const startSharedLsp = async (editor, languageId, filePath, options = {}) => {
  if (!editor || !languageId) return false
  if (options.requireEnabled !== false && !await resolveLspEnabled()) return false
  try {
    initLspClient(editor, languageId, filePath || '', {
      requestSource: options.requestSource || 'aux-editor',
      documentUri: options.documentUri,
      workspacePath: options.workspacePath,
    })
    logger.info('[EditorFeatures] lsp start requested', { languageId, filePath })
    return true
  } catch (error) {
    logger.warn('[EditorFeatures] lsp start failed', { languageId, error })
    return false
  }
}

const CONTEXT_MENU_CLASS = 'guaxi-shared-context-menu'
const CONTEXT_MENU_STYLE_ID = 'guaxi-shared-context-menu-style'

const ensureContextMenuStyle = () => {
  if (document.getElementById(CONTEXT_MENU_STYLE_ID)) return
  const style = document.createElement('style')
  style.id = CONTEXT_MENU_STYLE_ID
  style.textContent = `
.${CONTEXT_MENU_CLASS} {
  position: fixed;
  z-index: 4000;
  min-width: 220px;
  padding: 4px;
  border: 1px solid var(--guaxi-border, #d0d7de);
  border-radius: 8px;
  background: var(--guaxi-panel-bg, #ffffff);
  color: var(--guaxi-text-primary, #1f2328);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  font-size: 12.5px;
  user-select: none;
}
.${CONTEXT_MENU_CLASS} .guaxi-menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  padding: 5px 10px;
  border-radius: 5px;
  cursor: pointer;
  white-space: nowrap;
}
.${CONTEXT_MENU_CLASS} .guaxi-menu-item:hover {
  background: rgba(45, 212, 191, 0.14);
}
.${CONTEXT_MENU_CLASS} .guaxi-menu-item.is-disabled {
  opacity: 0.45;
  cursor: default;
  pointer-events: none;
}
.${CONTEXT_MENU_CLASS} .guaxi-menu-shortcut {
  color: var(--guaxi-text-secondary, #57606a);
  font-size: 11px;
}
.${CONTEXT_MENU_CLASS} .guaxi-menu-separator {
  height: 1px;
  margin: 4px 6px;
  background: var(--guaxi-border, #d0d7de);
  opacity: 0.6;
}
`
  document.head.appendChild(style)
}

const runEditorCommand = (editor, actionId) => {
  if (!editor) return
  try {
    const action = editor.getAction?.(actionId)
    if (action) {
      action.run()
      return
    }
    editor.trigger('contextmenu', actionId, null)
  } catch (error) {
    logger.warn('[EditorFeatures] context menu action failed', { actionId, error })
  }
}

const insertSnippet = (editor, snippet) => {
  if (!editor || !snippet) return
  try {
    editor.trigger('contextmenu', 'editor.action.insertSnippet', { snippet })
  } catch (error) {
    logger.warn('[EditorFeatures] insert snippet failed', { error })
  }
}

const SNIPPET_MENU_ITEMS = {
  markdown: [
    { label: '插入标题', snippet: '## ${1:标题}' },
    { label: '插入表格', snippet: '| ${1:表头1} | ${2:表头2} |\n| --- | --- |\n| ${3:内容1} | ${4:内容2} |' },
    { label: '插入代码块', snippet: '```${1:language}\n${2:代码}\n```' },
    { label: '插入链接', snippet: '[${1:描述}](${2:链接})' },
    { label: '插入公式块', snippet: '$$\n${1:公式}\n$$' },
  ],
  latex: [
    { label: '插入分式', snippet: '\\frac{${1:a}}{${2:b}}' },
    { label: '插入根号', snippet: '\\sqrt{${1:x}}' },
    { label: '插入积分', snippet: '\\int_{${1:a}}^{${2:b}} ${3:f(x)}\\,d${4:x}' },
    { label: '插入矩阵', snippet: '\\begin{matrix}\n  ${1:a} & ${2:b}\\\\\n  ${3:c} & ${4:d}\n\\end{matrix}' },
    { label: '插入 aligned', snippet: '\\begin{aligned}\n  ${0}\n\\end{aligned}' },
  ],
}

/**
 * 自定义右键菜单（与主编辑器同一套命令，DOM 实现，不依赖宿主的模板结构）。
 *
 * 返回 dispose；组件卸载或编辑器重建时调用。
 */
export const installSharedContextMenu = (editor, monaco, options = {}) => {
  if (!editor) return () => {}
  ensureContextMenuStyle()

  const menu = document.createElement('div')
  menu.className = CONTEXT_MENU_CLASS
  menu.style.display = 'none'
  document.body.appendChild(menu)

  let unsubscribe = null
  let opened = false

  const close = () => {
    if (!opened && menu.style.display === 'none') return
    opened = false
    menu.style.display = 'none'
    document.removeEventListener('mousedown', onDocumentMouseDown, true)
    document.removeEventListener('keydown', onDocumentKeyDown, true)
    window.removeEventListener('blur', close)
    window.removeEventListener('resize', close)
  }

  const onDocumentMouseDown = (event) => {
    if (!menu.contains(event.target)) close()
  }
  const onDocumentKeyDown = (event) => {
    if (event.key === 'Escape') close()
  }

  const addItem = (item) => {
    if (item.separator) {
      const sep = document.createElement('div')
      sep.className = 'guaxi-menu-separator'
      menu.appendChild(sep)
      return
    }

    const row = document.createElement('div')
    row.className = `guaxi-menu-item${item.disabled ? ' is-disabled' : ''}`
    const label = document.createElement('span')
    label.textContent = item.label
    row.appendChild(label)
    if (item.shortcut) {
      const hint = document.createElement('span')
      hint.className = 'guaxi-menu-shortcut'
      hint.textContent = item.shortcut
      row.appendChild(hint)
    }
    row.addEventListener('mousedown', (event) => {
      event.preventDefault()
      event.stopPropagation()
      if (item.disabled) return
      close()
      item.run?.()
    })
    menu.appendChild(row)
  }

  const buildItems = () => {
    const model = editor.getModel?.()
    const language = String(model?.getLanguageId?.() || options.language || '').toLowerCase()
    const selection = editor.getSelection?.()
    const hasSelection = !!selection && !selection.isEmpty()
    const readOnly = !!editor.getOption?.(monaco.editor.EditorOption.readOnly)

    const items = [
      { label: '撤销', shortcut: 'Ctrl+Z', disabled: readOnly, run: () => runEditorCommand(editor, 'undo') },
      { label: '重做', shortcut: 'Ctrl+Y', disabled: readOnly, run: () => runEditorCommand(editor, 'redo') },
      { separator: true },
      { label: '剪切', shortcut: 'Ctrl+X', disabled: readOnly || !hasSelection, run: () => runEditorCommand(editor, 'editor.action.clipboardCutAction') },
      { label: '复制', shortcut: 'Ctrl+C', disabled: !hasSelection, run: () => runEditorCommand(editor, 'editor.action.clipboardCopyAction') },
      { label: '粘贴', shortcut: 'Ctrl+V', disabled: readOnly, run: () => runEditorCommand(editor, 'editor.action.clipboardPasteAction') },
      { label: '全选', shortcut: 'Ctrl+A', run: () => runEditorCommand(editor, 'editor.action.selectAll') },
      { separator: true },
      { label: '查找', shortcut: 'Ctrl+F', run: () => runEditorCommand(editor, 'actions.find') },
      { label: '命令面板', shortcut: 'Ctrl+Shift+P', run: () => runEditorCommand(editor, 'editor.action.quickCommand') },
    ]

    if (!readOnly && (language === 'markdown' || language === 'python')) {
      items.push({ separator: true })
      items.push({
        label: language === 'markdown' ? 'Markdown 格式化' : '格式化文档',
        disabled: language === 'python' && !supportsLspDocumentFormatting('python'),
        run: async () => {
          if (language === 'markdown') {
            await formatMarkdownEditor(editor, 'context-menu')
          } else {
            await formatEditorWithLsp(editor, language)
          }
        },
      })
    }

    const snippetItems = !readOnly ? (SNIPPET_MENU_ITEMS[language] || []) : []
    if (snippetItems.length) {
      items.push({ separator: true })
      items.push({ label: '插入片段', disabled: true })
      for (const item of snippetItems) {
        items.push({ label: `　${item.label}`, run: () => insertSnippet(editor, item.snippet) })
      }
    }

    return items
  }

  const open = (x, y) => {
    menu.innerHTML = ''
    for (const item of buildItems()) addItem(item)
    menu.style.display = 'block'
    menu.style.left = '0px'
    menu.style.top = '0px'

    const rect = menu.getBoundingClientRect()
    const maxX = Math.max(8, window.innerWidth - rect.width - 8)
    const maxY = Math.max(8, window.innerHeight - rect.height - 8)
    menu.style.left = `${Math.min(Math.max(8, x), maxX)}px`
    menu.style.top = `${Math.min(Math.max(8, y), maxY)}px`

    opened = true
    document.addEventListener('mousedown', onDocumentMouseDown, true)
    document.addEventListener('keydown', onDocumentKeyDown, true)
    window.addEventListener('blur', close)
    window.addEventListener('resize', close)
  }

  const disposable = editor.onContextMenu((event) => {
    const browserEvent = event?.event?.browserEvent || {}
    const x = event?.event?.posx ?? browserEvent.clientX ?? 0
    const y = event?.event?.posy ?? browserEvent.clientY ?? 0
    event?.event?.preventDefault?.()
    event?.event?.stopPropagation?.()
    open(x, y)
  })
  unsubscribe = () => { try { disposable?.dispose?.() } catch { /* ignore */ } }

  return () => {
    close()
    unsubscribe?.()
    unsubscribe = null
    menu.remove()
  }
}

/**
 * 一次性接好辅助编辑器的基础能力。
 *
 * @param {object} params
 * @param {typeof import('monaco-editor')} params.monaco
 * @param {monaco.editor.IStandaloneCodeEditor} params.editor
 * @param {string} params.language
 * @param {string} [params.filePath]
 * @param {boolean} [params.enableLsp]
 * @param {boolean} [params.enableContextMenu]
 * @param {() => void} [params.ensureThemes] 独立窗口里先注册本地主题（重注册同名校验用）
 */
export const attachSharedEditorFeatures = (params = {}) => {
  const { monaco, editor, language = '', filePath = '' } = params
  const disposers = []

  if (!editor) return () => {}

  if (params.enableSnippets !== false) {
    try {
      registerSnippetCompletion(monaco, language)
    } catch (error) {
      logger.warn('[EditorFeatures] register snippets failed', { language, error })
    }
  }

  if (params.enableContextMenu !== false) {
    try {
      disposers.push(installSharedContextMenu(editor, monaco, { language }))
    } catch (error) {
      logger.warn('[EditorFeatures] install context menu failed', { language, error })
    }
  }

  // 主题：主编辑器/宿主注册同名主题；独立窗口通过 ensureThemes 自己注册。
  // 代码主题变化时主编辑器会重注册同名主题，这里再注册+setTheme 一次让当前实例刷新。
  const applyTheme = () => {
    try {
      const dark = String(document.documentElement?.dataset?.guaxiTheme || 'light') === 'dark'
      // 先注册（独立窗口的本地主题），再 setTheme，否则 setTheme 时主题还没更新。
      params.ensureThemes?.()
      const themeName = params.resolveThemeName?.(dark) || (dark ? 'guaxi-dark' : 'guaxi-light')
      monaco.editor.setTheme(themeName)
    } catch (error) {
      logger.debug('[EditorFeatures] apply theme failed', { error })
    }
  }
  applyTheme()
  const disposeThemeReapply = onGuaxiMonacoCodeThemeReapply(() => applyTheme())
  disposers.push(() => { try { disposeThemeReapply?.() } catch { /* ignore */ } })

  if (params.enableLsp && language) {
    void startSharedLsp(editor, language, filePath, {
      requestSource: params.lspRequestSource || 'aux-editor',
      documentUri: params.lspDocumentUri,
      workspacePath: params.lspWorkspacePath,
    })
  }

  return () => {
    for (const dispose of disposers) {
      try {
        dispose?.()
      } catch (error) {
        logger.debug('[EditorFeatures] dispose feature failed', { error })
      }
    }
    disposers.length = 0
  }
}
