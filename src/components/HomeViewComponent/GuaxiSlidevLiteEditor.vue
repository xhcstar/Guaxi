<template>
  <div class="slidev-lite-editor">
    <div class="host-wrap" ref="hostRef">
      <div v-show="activePath" ref="editorRef" class="lite-editor"></div>

      <div v-if="blockedLargeFile" class="host-mask">
        文件过大，已阻止加载到编辑器以避免卡顿（{{ blockedInfo }}）
      </div>
      <div v-else-if="!activePath" class="host-mask">请选择一个 PPT 源文件以开始编辑</div>
      <div v-else-if="unsupported" class="host-mask">该文件不支持编辑</div>
    </div>

    <n-dropdown v-model:show="editorContextMenuVisible" :options="editorContextMenuOptions" placement="bottom-start"
      :x="editorContextMenuPosition.x" :y="editorContextMenuPosition.y" @select="handleEditorContextMenuSelect"
      @clickoutside="() => (editorContextMenuVisible = false)" />

    <div class="status-bar" v-if="activePath">
      <div class="status-left">
        <span class="status-item" :title="activePath">{{ fileName }}</span>
        <span v-if="dirty" class="status-dirty">未保存</span>
      </div>
      <div class="status-right">
        <span class="status-item">Ln {{ cursor.lineNumber }}, Col {{ cursor.column }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref, watch, nextTick } from 'vue'
import * as monaco from 'monaco-editor'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js'
import 'monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneCommandsQuickAccess.js'
import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'
import { NDropdown } from 'naive-ui'

const MAX_EDITABLE_CHARS = 800_000

const logger = createLogger('GuaxiSlidevLiteEditor')
const settingsStore = useSettingsStore()

const props = defineProps({
  activePath: { type: String, default: '' },
  content: { type: String, default: '' },
  unsupported: { type: Boolean, default: false },
})

const emit = defineEmits(['draft-flush', 'save-request'])

const hostRef = ref(null)
const editorRef = ref(null)

let editor = null
let model = null
let editorReady = false
let settingModel = false
let editorDisposables = []

let lastLayoutLogAt = 0
const logLayout = (stage, extra = {}) => {
  const now = Date.now()
  if (now - lastLayoutLogAt < 250) return
  lastLayoutLogAt = now

  try {
    const hostEl = hostRef.value
    const wrapEl = hostEl?.parentElement || null
    const rect = hostEl?.getBoundingClientRect?.() || { width: -1, height: -1, top: -1, left: -1 }
    const wrapRect = wrapEl?.getBoundingClientRect?.() || { width: -1, height: -1, top: -1, left: -1 }

    const hostStyle = hostEl ? window.getComputedStyle(hostEl) : null
    const wrapStyle = wrapEl ? window.getComputedStyle(wrapEl) : null

    logger.info('[layout]', {
      stage,
      hostClient: { w: hostEl?.clientWidth ?? -1, h: hostEl?.clientHeight ?? -1 },
      hostRect: { w: Math.round(rect.width), h: Math.round(rect.height), top: Math.round(rect.top) },
      hostCss: hostStyle ? { display: hostStyle.display, position: hostStyle.position, height: hostStyle.height } : null,
      wrapClient: { w: wrapEl?.clientWidth ?? -1, h: wrapEl?.clientHeight ?? -1 },
      wrapRect: { w: Math.round(wrapRect.width), h: Math.round(wrapRect.height), top: Math.round(wrapRect.top) },
      wrapCss: wrapStyle ? { display: wrapStyle.display, position: wrapStyle.position, height: wrapStyle.height, flex: wrapStyle.flex } : null,
      viewport: { w: window.innerWidth, h: window.innerHeight },
      ...extra,
    })
  } catch (e) {
    logger.warn('[layout] log failed', e)
  }
}

let guaxiMonacoThemesReady = false
const ensureGuaxiMonacoThemes = () => {
  if (guaxiMonacoThemesReady) return

  monaco.editor.defineTheme('guaxi-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: 'a0a1a7' },
      { token: 'keyword', foreground: 'a626a4' },
      { token: 'string', foreground: '50a14f' },
      { token: 'number', foreground: '986801' },
      { token: 'regexp', foreground: '0184bc' },
      { token: 'type', foreground: 'c18401' },
      { token: 'delimiter', foreground: '383a42' },
      { token: 'variable', foreground: 'e45649' },
      { token: 'parameter', foreground: '4078f2' },
      { token: 'function', foreground: '4078f2' },
      { token: 'constant', foreground: '986801' },
      { token: 'class', foreground: 'c18401' },
      { token: 'identifier', foreground: '383a42' },
      { token: 'tag', foreground: 'e45649' },
      { token: 'attribute.name', foreground: 'c18401' },
      { token: 'meta.embedded', foreground: 'a0a1a7' }
    ],
    colors: {
      'editor.background': '#fafafa',
      'editor.foreground': '#383a42',
      'editorLineNumber.foreground': '#a0a1a7',
      'editorLineNumber.activeForeground': '#383a42',
      'editorCursor.foreground': '#4078f2',
      'editor.selectionBackground': '#d7dae0',
      'editor.inactiveSelectionBackground': '#e3f2fd',
      'editor.lineHighlightBackground': '#f0f0f0',
      'editorBracketMatch.background': '#e1f5fe',
      'editorBracketMatch.border': '#90caf9',
      'editor.selectionHighlightBackground': '#ffe0b2',
      'editor.foldBackground': '#f7f9fc',
      'editorWhitespace.foreground': '#d0d0d0',
      'editorIndentGuide.background': '#d0d0d0',
      'editorIndentGuide.activeBackground': '#b0bec5',
      'editorHoverWidget.background': '#ffffff',
      'editorHoverWidget.border': '#e0e0e0',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#e0e0e0',
      'editorSuggestWidget.foreground': '#383a42',
      'editorSuggestWidget.selectedBackground': '#e3f2fd',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#e0e0e0',
      'editorGutter.background': '#fafafa',
      'scrollbar.shadow': '#00000033'
    }
  })

  monaco.editor.defineTheme('guaxi-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '5c6370' },
      { token: 'keyword', foreground: 'c678dd' },
      { token: 'string', foreground: '98c379' },
      { token: 'number', foreground: 'd19a66' },
      { token: 'regexp', foreground: '56b6c2' },
      { token: 'type', foreground: 'e5c07b' },
      { token: 'delimiter', foreground: 'abb2bf' },
      { token: 'variable', foreground: 'e06c75' },
      { token: 'parameter', foreground: '61afef' },
      { token: 'function', foreground: '61afef' },
      { token: 'constant', foreground: 'd19a66' },
      { token: 'class', foreground: 'e5c07b' },
      { token: 'identifier', foreground: 'abb2bf' },
      { token: 'tag', foreground: 'e06c75' },
      { token: 'attribute.name', foreground: 'e5c07b' },
      { token: 'meta.embedded', foreground: '5c6370' }
    ],
    colors: {
      'editor.background': '#282c34',
      'editor.foreground': '#abb2bf',
      'editorLineNumber.foreground': '#5c6370',
      'editorLineNumber.activeForeground': '#abb2bf',
      'editorCursor.foreground': '#528bff',
      'editor.selectionBackground': '#3e4451',
      'editor.inactiveSelectionBackground': '#3e4451',
      'editor.lineHighlightBackground': '#2c313a',
      'editorBracketMatch.background': '#3e4451',
      'editorBracketMatch.border': '#528bff',
      'editor.selectionHighlightBackground': '#3e4451',
      'editor.foldBackground': '#21252b',
      'editorWhitespace.foreground': '#4b5263',
      'editorIndentGuide.background': '#4b5263',
      'editorIndentGuide.activeBackground': '#656c7c',
      'editorHoverWidget.background': '#282c34',
      'editorHoverWidget.border': '#3b4048',
      'editorSuggestWidget.background': '#282c34',
      'editorSuggestWidget.border': '#3b4048',
      'editorSuggestWidget.foreground': '#abb2bf',
      'editorSuggestWidget.selectedBackground': '#2c313a',
      'editorWidget.background': '#282c34',
      'editorWidget.border': '#3b4048',
      'editorGutter.background': '#282c34',
      'scrollbar.shadow': '#00000066'
    }
  })

  guaxiMonacoThemesReady = true
}

const getMonacoTheme = () => {
  ensureGuaxiMonacoThemes()
  return settingsStore.appearanceTheme === 'dark' ? 'guaxi-dark' : 'guaxi-light'
}

const applyMonacoTheme = () => {
  try {
    monaco.editor.setTheme(getMonacoTheme())
  } catch (e) {
    logger.warn('[theme] apply failed', e)
  }
}

const disposeMonaco = () => {
  editorDisposables.forEach((d) => d?.dispose?.())
  editorDisposables = []
  if (editor) {
    editor.dispose()
    editor = null
  }
  if (model) {
    model.dispose()
    model = null
  }
  editorReady = false
}

let localDirty = false
let lastDirtyAt = 0
let lastDirtyLogAt = 0

const editorBusy = ref(false)
const blockedLargeFile = ref(false)
const blockedInfo = ref('')

const dirty = ref(false)
const cursor = ref({ lineNumber: 1, column: 1 })
const draft = ref('')

const editorContextMenuVisible = ref(false)
const editorContextMenuPosition = ref({ x: 0, y: 0 })
const editorContextMenuOptions = ref([])

const buildEditorContextMenuOptions = () => {
  const model = editor?.getModel?.()
  const isMarkdown = (model?.getLanguageId?.() || '') === 'markdown'
  const selection = editor?.getSelection?.()
  const hasSelection = !!selection && !selection.isEmpty()

  return [
    { label: '剪切', key: 'editor.action.clipboardCutAction' },
    { label: '复制', key: 'editor.action.clipboardCopyAction' },
    { label: '粘贴', key: 'editor.action.clipboardPasteAction' },
    { type: 'divider' },
    { label: '文件内搜索 (Ctrl+F)', key: 'actions.find' },
    { type: 'divider' },
    { label: 'Markdown 格式化（全文）', key: 'guaxi.markdown.formatDocument', disabled: !isMarkdown },
    { label: 'Markdown 格式化（选中）', key: 'guaxi.markdown.formatSelection', disabled: !isMarkdown || !hasSelection },
    { type: 'divider' },
    { label: '跳转到上次编辑位置 (Alt+←)', key: 'guaxi.edit.navigateBack' },
    { label: '跳转到下一编辑位置 (Alt+→)', key: 'guaxi.edit.navigateForward' },
    { type: 'divider' },
    { label: '插入Markdown模板', key: 'insert.md.template' },
    { type: 'divider' },
    { label: '命令面板', key: 'editor.action.quickCommand' }
  ]
}

const runEditorAction = async (actionId) => {
  if (!editor) return
  const act = editor.getAction?.(actionId)
  if (act) {
    await act.run()
    return
  }
  try {
    editor.trigger('contextmenu', actionId, null)
  } catch (e) {
    logger.warn('[contextmenu] action failed', { actionId, error: e })
  }
}

const handleEditorContextMenuSelect = async (key) => {
  editorContextMenuVisible.value = false
  await runEditorAction(key)
}

const markDirty = (reason) => {
  localDirty = true
  dirty.value = true

  const now = Date.now()
  lastDirtyAt = now
  if (now - lastDirtyLogAt > 1500) {
    lastDirtyLogAt = now
    logger.debug('[draft] marked dirty', { path: activePath.value, reason })
  }
}

const flushDraft = (path, tag) => {
  const p = normalizePath(path)
  if (!p) return

  if (!localDirty) {
    logger.debug('[draft] skip flush (clean)', { path: p, tag })
    return
  }

  const text = String(draft.value ?? '')
  emit('draft-flush', { path: p, content: text, tag })

  localDirty = false

  logger.info('[draft] flushed', { path: p, tag, len: text.length })
}

const normalizePath = (p) => String(p || '').replace(/\\/g, '/')

const detectLanguage = (p) => {
  const s = String(p || '').toLowerCase()
  if (s.endsWith('.md') || s.endsWith('.mdx')) return 'markdown'
  if (s.endsWith('.yml') || s.endsWith('.yaml')) return 'yaml'
  if (s.endsWith('.json')) return 'json'
  if (s.endsWith('.js')) return 'javascript'
  if (s.endsWith('.ts')) return 'typescript'
  if (s.endsWith('.vue')) return 'html'
  return 'plaintext'
}

const updateEditorOptions = () => {
  if (!editor) return
  const readOnly = blockedLargeFile.value || props.unsupported || !activePath.value
  editor.updateOptions({ readOnly })
}

const updateCursorFromEditor = () => {
  if (!editor) return
  const pos = editor.getPosition()
  if (!pos) return
  cursor.value = { lineNumber: pos.lineNumber, column: pos.column }
}

const registerMarkdownActions = () => {
  if (!editor) return

  editor.addAction({
    id: 'guaxi.markdown.toggleBold',
    label: '加粗',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyB],
    run: () => toggleBold()
  })

  editor.addAction({
    id: 'guaxi.markdown.toggleItalic',
    label: '斜体',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyI],
    run: () => toggleItalic()
  })

  editor.addAction({
    id: 'guaxi.markdown.toggleStrikethrough',
    label: '删除线',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyX],
    run: () => toggleStrikethrough()
  })

  editor.addAction({
    id: 'guaxi.markdown.insertLink',
    label: '插入链接',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyK],
    run: () => insertLink()
  })

  editor.addAction({
    id: 'guaxi.markdown.insertTable',
    label: '插入表格',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyT],
    run: () => insertTableWithSize(2, 2)
  })

  editor.addAction({
    id: 'guaxi.editor.find',
    label: '文件内搜索',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyF],
    run: () => runEditorAction('actions.find')
  })

  editor.addAction({
    id: 'guaxi.editor.commandPalette',
    label: '命令面板',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyP],
    run: () => runEditorAction('editor.action.quickCommand')
  })
}

const ensureEditor = async () => {
  if (editorReady || !editorRef.value) return

  editor = monaco.editor.create(editorRef.value, {
    automaticLayout: true,
    readOnly: false,
    wordWrap: 'on',
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    fontSize: 16,
    fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    padding: { top: 8, bottom: 8 },
    contextmenu: {
      enabled: true,
      addDefaultActions: true,
      addNavigationActions: true
    }
  })

  editorDisposables.push(
    editor.onDidChangeModelContent(() => {
      if (settingModel) return
      draft.value = editor.getValue()
      markDirty('typing')
    }),
    editor.onDidChangeCursorPosition(() => {
      updateCursorFromEditor()
    }),
    editor.onDidBlurEditorWidget(() => {
      onBlur()
    }),
    editor.onContextMenu((e) => {
      const posx = e.event?.posx ?? e.event?.browserEvent?.clientX ?? 0
      const posy = e.event?.posy ?? e.event?.browserEvent?.clientY ?? 0
      editor.focus()
      editorContextMenuPosition.value = { x: posx, y: posy }
      editorContextMenuOptions.value = buildEditorContextMenuOptions()
      editorContextMenuVisible.value = true
      e.event?.preventDefault?.()
    })
  )

  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
    const p = activePath.value
    if (!p) {
      logger.warn('[save] hotkey ignored: no active file')
      return
    }
    emit('save-request', { path: p })
    logger.info('[save] hotkey', { path: p })
  })

  registerMarkdownActions()

  applyMonacoTheme()
  updateEditorOptions()
  editorReady = true
  logger.info('[editor] created')
}

const clearEditorModel = () => {
  if (!editor) return
  if (model) model.dispose()
  model = monaco.editor.createModel('', 'plaintext')
  editor.setModel(model)
  draft.value = ''
  dirty.value = false
  localDirty = false
}

const activePath = computed(() => normalizePath(props.activePath || ''))

const fileName = computed(() => {
  const p = activePath.value
  if (!p) return ''
  const parts = p.split('/')
  return parts[parts.length - 1] || p
})

// 绑定路径并刷新模型内容
const setModelForPath = async (path, content) => {
  const p = normalizePath(path)
  if (!p) return

  blockedLargeFile.value = false
  blockedInfo.value = ''

  const nextContent = String(content ?? '')

  if (nextContent.length > MAX_EDITABLE_CHARS) {
    blockedLargeFile.value = true
    blockedInfo.value = `${nextContent.length} chars`
    logger.warn('[model] blocked large file', { path: p, len: nextContent.length, limit: MAX_EDITABLE_CHARS })
    await ensureEditor()
    clearEditorModel()
    updateEditorOptions()
    return
  }

  await ensureEditor()
  if (!editor) return

  const lang = detectLanguage(p)
  if (model) model.dispose()
  model = monaco.editor.createModel(nextContent, lang, monaco.Uri.file(p))
  editor.setModel(model)

  draft.value = nextContent
  dirty.value = false
  localDirty = false
  updateEditorOptions()
  updateCursorFromEditor()
  logger.info('[model] set model', { path: p, len: nextContent.length, lang })
}

const onBlur = () => {
  const p = activePath.value
  if (!p) return
  flushDraft(p, 'blur')
}

const insertContent = async (text) => {
  const p = activePath.value
  if (!p) {
    logger.warn('[insert] failed: no active file', { path: p })
    return false
  }

  await ensureEditor()
  if (!editor || !model) {
    logger.warn('[insert] failed: editor not ready', { path: p })
    return false
  }

  const t = String(text ?? '')
  const selection = editor.getSelection()
  const pos = editor.getPosition() || new monaco.Position(1, 1)
  const range = selection || new monaco.Range(pos.lineNumber, pos.column, pos.lineNumber, pos.column)

  editor.executeEdits('insert-content', [
    { range, text: t, forceMoveMarkers: true }
  ])

  editor.focus()
  updateCursorFromEditor()

  logger.info('[insert] ok', { path: p, len: t.length })
  return true
}

const wrapSelection = (prefix, suffix, placeholder = '') => {
  if (!editor) return
  const m = editor.getModel()
  const selection = editor.getSelection()
  if (!m || !selection) return
  const selectedText = m.getValueInRange(selection)
  const finalText = prefix + (selectedText || placeholder) + suffix
  editor.executeEdits('md-wrap', [{ range: selection, text: finalText, forceMoveMarkers: true }])

  if (!selectedText && placeholder) {
    const startLineNumber = selection.startLineNumber
    const startColumn = selection.startColumn + prefix.length
    const endColumn = startColumn + placeholder.length
    editor.setSelection(new monaco.Selection(startLineNumber, startColumn, startLineNumber, endColumn))
  }

  editor.focus()
}

const insertBlockSnippet = (snippet) => {
  if (!editor) return
  const pos = editor.getPosition() || new monaco.Position(1, 1)
  const range = new monaco.Range(pos.lineNumber, 1, pos.lineNumber, 1)
  editor.executeEdits('md-block', [{ range, text: snippet, forceMoveMarkers: true }])
  editor.focus()
}

const toggleHeading = (level) => {
  if (!editor) return
  const m = editor.getModel()
  const selection = editor.getSelection()
  if (!m || !selection) return

  const startLine = selection.startLineNumber
  const endLine = selection.endLineNumber
  const edits = []

  for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
    const text = m.getLineContent(lineNumber)
    const content = text.replace(/^#{1,6}\s+/, '')
    let next = content
    if (level > 0) {
      const sameLevel = new RegExp(`^#{${level}}\\s+`).test(text)
      next = sameLevel ? content : `${'#'.repeat(level)} ${content || '标题'}`
    }
    edits.push({
      range: new monaco.Range(lineNumber, 1, lineNumber, m.getLineMaxColumn(lineNumber)),
      text: next,
      forceMoveMarkers: true
    })
  }

  editor.executeEdits('md-heading', edits)
  editor.focus()
}

const toggleBold = () => {
  wrapSelection('**', '**', '粗体文本')
}

const toggleItalic = () => {
  wrapSelection('*', '*', '斜体文本')
}

const toggleStrikethrough = () => {
  wrapSelection('~~', '~~', '删除线')
}

const toggleInlineCode = () => {
  wrapSelection('`', '`', 'code')
}

const insertLink = () => {
  wrapSelection('[', '](https://)', '链接文字')
}

const toggleLinePrefix = (prefix, pattern, buildLine) => {
  if (!editor) return
  const m = editor.getModel()
  const selection = editor.getSelection()
  if (!m || !selection) return

  const startLine = selection.startLineNumber
  const endLine = selection.endLineNumber
  const lines = []
  let allMatch = true

  for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
    const text = m.getLineContent(lineNumber)
    lines.push(text)
    if (!pattern.test(text)) allMatch = false
  }

  const edits = lines.map((text, index) => {
    const lineNumber = startLine + index
    const next = allMatch
      ? text.replace(pattern, '')
      : (buildLine ? buildLine(text, index) : `${prefix}${text}`)

    return {
      range: new monaco.Range(lineNumber, 1, lineNumber, m.getLineMaxColumn(lineNumber)),
      text: next,
      forceMoveMarkers: true
    }
  })

  editor.executeEdits('md-line-prefix', edits)
  editor.focus()
}

const toggleBlockquote = () => {
  toggleLinePrefix('> ', /^>\s+/, null)
}

const toggleUnorderedList = () => {
  toggleLinePrefix('- ', /^-\s+/, null)
}

const toggleOrderedList = () => {
  toggleLinePrefix('1. ', /^\d+\.\s+/, (text, index) => `${index + 1}. ${text}`)
}

const toggleTaskList = () => {
  toggleLinePrefix('- [ ] ', /^-\s\[( |x|X)\]\s+/, null)
}

const insertImage = () => {
  insertBlockSnippet('![图片描述](路径或URL)\n')
}

const insertCodeBlock = (lang) => {
  const v = String(lang || '').trim()
  const prefix = v ? `\`\`\`${v}\n` : '```\n'
  wrapSelection(prefix, '\n```\n', '代码')
}

const insertHorizontalRule = () => {
  insertBlockSnippet('---\n')
}

const insertTableWithSize = (rows, cols) => {
  if (!editor) return
  const r = Math.max(2, Math.floor(Number(rows) || 2))
  const c = Math.max(1, Math.floor(Number(cols) || 2))
  const header = Array.from({ length: c }, (_, i) => `列${i + 1}`)
  const sep = Array.from({ length: c }, () => '---')
  const body = Array.from({ length: Math.max(1, r - 1) }, (_, i) => `| ${Array.from({ length: c }, (_, j) => (i === 0 ? `内容${j + 1}` : '')).join(' | ')} |`).join('\n')
  const snippet = `| ${header.join(' | ')} |\n| ${sep.join(' | ')} |\n${body}\n`
  insertBlockSnippet(snippet)
  logger.info('[table] insert', { rows: r, cols: c })
}

const undo = () => {
  if (!editor) return
  editor.focus()
  editor.trigger('toolbar', 'undo', null)
}

const redo = () => {
  if (!editor) return
  editor.focus()
  editor.trigger('toolbar', 'redo', null)
}

const getContent = () => {
  const p = activePath.value
  if (!p || !model) return { ok: false, path: '', content: '' }
  const text = model.getValue()
  return { ok: true, path: p, content: text }
}

const markSaved = (path) => {
  const p = normalizePath(path)
  if (!p || p !== activePath.value) {
    logger.debug('[save] markSaved skipped', { path: p, active: activePath.value })
    return
  }

  localDirty = false
  dirty.value = false
  logger.info('[save] markSaved', { path: p })
}

defineExpose({
  insertContent,
  getContent,
  markSaved,
  runEditorAction,
  toggleHeading,
  toggleBold,
  toggleItalic,
  toggleStrikethrough,
  toggleInlineCode,
  insertLink,
  toggleBlockquote,
  toggleUnorderedList,
  toggleOrderedList,
  toggleTaskList,
  insertCodeBlock,
  insertImage,
  insertHorizontalRule,
  insertTable: (rows, cols) => insertTableWithSize(rows, cols),
  undo,
  redo
})

watch(
  () => activePath.value,
  async (p, prev) => {
    logger.info('[watch] activePath changed', { prev, next: p })
    logLayout('watch-activePath', { prev, next: p })
    if (prev && prev !== p) {
      flushDraft(prev, 'switch')
    }
    if (!p) {
      await ensureEditor()
      clearEditorModel()
      updateEditorOptions()
      return
    }
    await setModelForPath(p, props.content)
    logLayout('after-setModelForPath', { path: p })
    await nextTick()
    updateCursorFromEditor()
  },
  { immediate: true, flush: 'post' }
)

watch(
  () => props.content,
  async (next) => {
    const p = activePath.value
    if (!p) return
    if (localDirty) return
    const target = String(next ?? '')
    if (model && model.getValue() === target) return
    logger.info('[watch] external content update', { path: p, len: target.length })
    await setModelForPath(p, target)
    await nextTick()
    updateCursorFromEditor()
  },
  { flush: 'post' }
)

watch(
  () => settingsStore.appearanceTheme,
  () => {
    applyMonacoTheme()
  },
  { immediate: true }
)

watch(
  () => [activePath.value, props.unsupported, blockedLargeFile.value],
  () => {
    updateEditorOptions()
  }
)

onMounted(async () => {
  logger.info('[mount]')
  await ensureEditor()
  await nextTick()
  logLayout('mounted-nextTick')
})

onUnmounted(() => {
  logger.info('[unmount]')
  if (activePath.value) {
    flushDraft(activePath.value, 'unmount')
  }
  disposeMonaco()
})
</script>

<style scoped>
.slidev-lite-editor {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--guaxi-content-bg, transparent);
}


.host-wrap {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  position: relative;
  overflow: hidden;
}

.host {
  position: absolute;
  inset: 0;
}

.lite-editor {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
}

.host-mask {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.25);
  color: #fff;
  font-size: 12px;
  user-select: none;
  padding: 12px;
  text-align: center;
}

.status-bar {
  flex: 0 0 auto;
  margin-top: auto;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--statusBar-border, rgba(128, 128, 128, 0.25));
  background: var(--statusBar-background, rgba(255, 255, 255, 0.02));
  padding: 0 10px;
  box-sizing: border-box;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #8a8f98);
}

.status-left,
.status-right {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.status-item {
  max-width: 520px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-dirty {
  color: #d97706;
}

:deep(.search-highlight) {
  background-color: rgb(241, 168, 10) !important;
  color: #000 !important;
  border-radius: 2px;
  padding: 0 2px;
}

:deep(.dark .search-highlight) {
  background-color: rgb(255, 208, 0) !important;
  color: #000 !important;
}
</style>
