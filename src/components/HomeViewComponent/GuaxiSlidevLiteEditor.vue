<template>
  <div class="slidev-lite-editor">
    <div v-if="activePath" class="breadcrumb-bar" ref="breadcrumbBarRef">
      <div ref="breadcrumbScrollRef" class="breadcrumb-scroll" @wheel.prevent="handleBreadcrumbWheel">
        <n-breadcrumb separator="›">
          <n-breadcrumb-item v-for="it in breadcrumbItems" :key="it.key">
            <span class="breadcrumb-item" :class="{ disabled: !!it.disabled }" :title="it.title"
              @click.stop="handleBreadcrumbClick(it)">
              <Icon v-if="it.icon" class="breadcrumb-icon" :icon="it.icon" width="14" height="14" />
              <span class="breadcrumb-label">{{ it.label }}</span>
            </span>
          </n-breadcrumb-item>
        </n-breadcrumb>
      </div>
    </div>

    <div class="host-wrap" ref="hostRef">
      <div v-show="tableToolbarVisible" class="floating-table-toolbar" :style="tableToolbarStyle" @mousedown.prevent>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="addTableRowBelow">
              <Icon :icon="tableRowPlusAfter" width="16" height="16" />
            </n-button>
          </template>
          新增行 (Ctrl+Alt+↓)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="deleteTableRow">
              <Icon :icon="tableRowRemove" width="16" height="16" />
            </n-button>
          </template>
          删除行 (Ctrl+Alt+Shift+↓)
        </n-tooltip>
        <div class="floating-table-toolbar-divider"></div>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="addTableColumnRight">
              <Icon :icon="tableColumnPlusAfter" width="16" height="16" />
            </n-button>
          </template>
          新增列 (Ctrl+Alt+→)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="deleteTableColumn">
              <Icon :icon="tableColumnRemove" width="16" height="16" />
            </n-button>
          </template>
          删除列 (Ctrl+Alt+Shift+→)
        </n-tooltip>
        <div class="floating-table-toolbar-divider"></div>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="setTableColumnAlign('left')">
              <Icon :icon="alignLeftIcon" width="16" height="16" />
            </n-button>
          </template>
          左对齐
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="setTableColumnAlign('center')">
              <Icon :icon="alignCenterIcon" width="16" height="16" />
            </n-button>
          </template>
          居中
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="setTableColumnAlign('right')">
              <Icon :icon="alignRightIcon" width="16" height="16" />
            </n-button>
          </template>
          右对齐
        </n-tooltip>
        <div class="floating-table-toolbar-divider"></div>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="formatTable">
              <Icon :icon="sparklesIcon" width="16" height="16" />
            </n-button>
          </template>
          格式化
        </n-tooltip>
      </div>

      <div v-show="floatingToolbarVisible" class="floating-md-toolbar" :style="floatingToolbarStyle" @mousedown.prevent>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="toggleBold">
              <Icon :icon="boldIcon" width="16" height="16" />
            </n-button>
          </template>
          加粗 (Ctrl+B)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="toggleItalic">
              <Icon :icon="italicIcon" width="16" height="16" />
            </n-button>
          </template>
          斜体 (Ctrl+I)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="toggleStrikethrough">
              <Icon :icon="strikethroughIcon" width="16" height="16" />
            </n-button>
          </template>
          删除线 (Ctrl+Shift+X)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="insertLink">
              <Icon :icon="linkIcon" width="16" height="16" />
            </n-button>
          </template>
          链接 (Ctrl+K)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="toggleInlineCode">
              <Icon :icon="codeIcon" width="16" height="16" />
            </n-button>
          </template>
          行内代码
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" :disabled="!canEdit" @click="toggleBlockquote">
              <Icon :icon="quoteIcon" width="16" height="16" />
            </n-button>
          </template>
          引用
        </n-tooltip>
      </div>

      <n-drawer v-model:show="outlineVisible" placement="right" :width="360" :mask="false" :trap-focus="false"
        :block-scroll="false" class="outline-drawer">
        <n-drawer-content title="大纲">
          <div class="outline-drawer-body">
            <GuaxiOutline />
          </div>
        </n-drawer-content>
      </n-drawer>

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
import MarkdownIt from 'markdown-it'
import { debounce } from 'lodash-es'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { createLogger } from '../../utils/logger'
import { useSettingsStore } from '../../stores/settingsStore'
import { useOutlineStore } from '../../stores/outlineStore'
import { NDropdown, NButton, NTooltip, NBreadcrumb, NBreadcrumbItem, useMessage } from 'naive-ui'
import { Icon } from '@iconify/vue'
import GuaxiOutline from './GuaxiOutline.vue'
import boldIcon from '@iconify-icons/lucide/bold'
import italicIcon from '@iconify-icons/lucide/italic'
import strikethroughIcon from '@iconify-icons/lucide/strikethrough'
import linkIcon from '@iconify-icons/lucide/link'
import quoteIcon from '@iconify-icons/lucide/quote'
import codeIcon from '@iconify-icons/lucide/code'
import listIcon from '@iconify-icons/lucide/list'
import imageIcon from '@iconify-icons/lucide/image'
import alignLeftIcon from '@iconify-icons/lucide/align-left'
import alignCenterIcon from '@iconify-icons/lucide/align-center'
import alignRightIcon from '@iconify-icons/lucide/align-right'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import folderIcon from '@iconify-icons/lucide/folder'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import hashIcon from '@iconify-icons/lucide/hash'
import tableRowPlusAfter from '@iconify-icons/mdi/table-row-plus-after'
import tableRowRemove from '@iconify-icons/mdi/table-row-remove'
import tableColumnPlusAfter from '@iconify-icons/mdi/table-column-plus-after'
import tableColumnRemove from '@iconify-icons/mdi/table-column-remove'

const MAX_EDITABLE_CHARS = 800_000

const logger = createLogger('GuaxiSlidevLiteEditor')
const settingsStore = useSettingsStore()
const outlineStore = useOutlineStore()
const message = useMessage()
const appWindow = getCurrentWindow()

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

const floatingToolbarVisible = ref(false)
const floatingToolbarStyle = ref({ left: '0px', top: '0px' })
const tableToolbarVisible = ref(false)
const tableToolbarStyle = ref({ left: '0px', top: '0px' })
const activeTableContext = ref(null)

const outlineVisible = ref(false)
const breadcrumbScrollRef = ref(null)
const breadcrumbBarRef = ref(null)

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
const normalizeSlashes = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$/g, '').trim()

const cursorLineNumber = computed(() => {
  const n = Number(cursor.value?.lineNumber || 1)
  return Number.isFinite(n) && n > 0 ? n : 1
})

const safeOutlineFlat = computed(() => {
  const v = outlineStore?.outline
  if (Array.isArray(v)) return v
  if (Array.isArray(v?.value)) return v.value
  return []
})

const buildHeadingBreadcrumbItems = (outlineFlat, lineNumber) => {
  const arr = Array.isArray(outlineFlat) ? outlineFlat : []
  if (!arr.length) return []

  const ln = Number(lineNumber || 1)
  const stack = []

  for (const h of arr) {
    const hLine = Number(h?.line || 0)
    const level = Number(h?.level || 0)
    const text = String(h?.text || '').trim()
    if (!text || !Number.isFinite(hLine) || hLine <= 0 || !Number.isFinite(level) || level <= 0) continue
    if (hLine > ln) break

    while (stack.length && stack[stack.length - 1].level >= level) stack.pop()
    stack.push({ level, text, line: hLine })
  }

  return stack.map((h) => ({
    key: `heading:${h.line}:${h.level}:${h.text}`,
    label: h.text,
    title: `Ln ${h.line}: ${h.text}`,
    icon: hashIcon,
    kind: 'heading',
    line: h.line
  }))
}

const buildBreadcrumbItems = (absPath) => {
  const target = normalizeSlashes(absPath)
  if (!target) {
    return [{ key: 'placeholder', label: '未打开文件', title: '未打开文件', icon: fileTextIcon, kind: 'placeholder', disabled: true }]
  }

  const parts = target.split('/').filter(Boolean)
  const items = []
  let cursorPath = target.startsWith('/') ? '' : ''

  for (let i = 0; i < parts.length; i += 1) {
    const part = parts[i]
    cursorPath = cursorPath ? `${cursorPath}/${part}` : part
    const isLast = i === parts.length - 1
    items.push({
      key: `path:${cursorPath}`,
      label: part,
      title: cursorPath,
      icon: isLast ? fileTextIcon : folderIcon,
      kind: isLast ? 'file' : 'dir',
      path: cursorPath
    })
  }

  return items
}

const breadcrumbItems = computed(() => {
  const absPath = activePath.value
  const pathItems = buildBreadcrumbItems(absPath)
  const headingItems = buildHeadingBreadcrumbItems(safeOutlineFlat.value, cursorLineNumber.value)
  return [...pathItems, ...headingItems]
})

const handleBreadcrumbClick = async (it) => {
  if (!it || it.disabled) return

  if (it.kind === 'heading' && Number.isFinite(Number(it.line))) {
    const ln = Number(it.line)
    logger.info('[Breadcrumb] jump to heading', { line: ln, label: it.label })
    jumpToLine(ln)
    return
  }

  const path = it?.path ? String(it.path) : ''
  if (!path) return

  const target = it.kind === 'dir' ? path : normalizeSlashes(path.split('/').slice(0, -1).join('/'))
  if (!target) return

  try {
    await invoke('open_in_file_explorer', { path: target })
    logger.info('[Breadcrumb] open in explorer', { path: target })
  } catch (e) {
    logger.error('[Breadcrumb] open in explorer failed', e)
    message.error('打开目录失败')
  }
}

const handleBreadcrumbWheel = (e) => {
  const el = breadcrumbScrollRef.value
  if (!el) return

  const dx = Number(e?.deltaX || 0)
  const dy = Number(e?.deltaY || 0)
  const delta = Math.abs(dy) >= Math.abs(dx) ? dy : dx

  if (!Number.isFinite(delta) || delta === 0) return

  el.scrollLeft += delta
}

const toggleOutlineVisible = () => {
  if (!outlineHasContent.value) return
  outlineVisible.value = !outlineVisible.value
  logger.info('[Outline] toggle', { visible: outlineVisible.value })
}

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

const md = new MarkdownIt()

const generateOutline = (content) => {
  if (detectLanguage(activePath.value) !== 'markdown') {
    outlineStore.setOutline([])
    return
  }
  const tokens = md.parse(String(content || ''), {})
  const result = []
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1))
      const nextToken = tokens[idx + 1]
      if (nextToken && nextToken.type === 'inline') {
        result.push({
          level,
          text: nextToken.content,
          line: token.map ? token.map[0] + 1 : 1
        })
      }
    }
  })
  outlineStore.setOutline(result)
}

const generateOutlineDebounced = debounce((content) => {
  generateOutline(content)
}, 200)

const jumpToLine = (line) => {
  if (!editor) return
  const ln = Number(line || 1)
  if (!Number.isFinite(ln) || ln <= 0) return
  editor.revealLineInCenter(ln)
  editor.setPosition({ lineNumber: ln, column: 1 })
  editor.focus()
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

const waitForWindowMinimized = async (timeoutMs = 1200, settleMs = 260) => {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    try {
      const minimized = await appWindow.isMinimized()
      if (minimized) {
        logger.debug('[Screenshot] window minimized, waiting settle')
        await sleep(settleMs)
        return true
      }
    } catch (e) {
      logger.warn('[Screenshot] check minimized failed', e)
      break
    }
    await sleep(50)
  }
  logger.warn('[Screenshot] wait minimized timeout')
  return false
}

const captureScreenshot = async () => {
  const p = activePath.value
  if (!p) {
    message.warning('请先打开一个 PPT 源文件')
    return
  }

  try {
    try {
      logger.debug('[Screenshot] minimize before capture')
      await appWindow.minimize()
      await waitForWindowMinimized()
    } catch (minErr) {
      logger.warn('[Screenshot] minimize failed', minErr)
    }

    logger.info('[Screenshot] start', { path: p })
    const result = await invoke('take_screenshot_markdown', { ownerFilePath: p, alt: 'screenshot' })

    const markdown = result?.markdown
    if (typeof markdown !== 'string' || !markdown) {
      throw new Error('后端未返回 markdown')
    }

    const inserted = await insertContent(`${markdown}\n`)
    if (!inserted) {
      throw new Error('插入失败：编辑器未就绪')
    }

    logger.info('[Screenshot] inserted', { len: markdown.length })
  } catch (err) {
    logger.error('[Screenshot] failed', err)
    message.error('截图失败')
  } finally {
    try {
      logger.debug('[Screenshot] restore window')
      await appWindow.unminimize()
      await appWindow.show()
      await appWindow.setFocus()
    } catch (restoreErr) {
      logger.warn('[Screenshot] restore failed', restoreErr)
    }
  }
}

const updateFloatingToolbar = () => {
  if (!editor) {
    floatingToolbarVisible.value = false
    return
  }

  const model = editor.getModel()
  const selection = editor.getSelection()
  if (!model || !selection || selection.isEmpty()) {
    floatingToolbarVisible.value = false
    return
  }

  if (model.getLanguageId() !== 'markdown') {
    floatingToolbarVisible.value = false
    return
  }

  const wrapperEl = hostRef.value
  const editorEl = editorRef.value
  if (!wrapperEl || !editorEl) {
    floatingToolbarVisible.value = false
    return
  }

  const startPos = selection.getStartPosition()
  if (!startPos) {
    floatingToolbarVisible.value = false
    return
  }

  const startVisible = editor.getScrolledVisiblePosition(startPos)
  if (!startVisible) {
    floatingToolbarVisible.value = false
    return
  }

  const wrapperRect = wrapperEl.getBoundingClientRect()
  const editorRect = editorEl.getBoundingClientRect()

  const leftBase = editorRect.left - wrapperRect.left
  const topBase = editorRect.top - wrapperRect.top

  const headerEl = breadcrumbBarRef.value
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0
  const minTop = headerHeight + 8

  const toolbarH = 34
  const toolbarW = 240
  const gap = 8

  const preferTop = topBase + startVisible.top - toolbarH - gap
  const fallbackTop = topBase + startVisible.top + startVisible.height + gap
  const top = preferTop >= minTop ? preferTop : fallbackTop
  const clampedTop = Math.min(
    Math.max(minTop, top),
    Math.max(minTop, wrapperRect.height - toolbarH - 8)
  )

  const layout = editor.getLayoutInfo ? editor.getLayoutInfo() : null
  const contentLeft = layout && Number.isFinite(layout.contentLeft) ? layout.contentLeft : 0
  const minLeft = Math.max(8, leftBase + contentLeft)

  const left = leftBase + startVisible.left
  const clampedLeft = Math.min(
    Math.max(minLeft, left),
    Math.max(minLeft, wrapperRect.width - toolbarW - 8)
  )

  floatingToolbarStyle.value = {
    left: `${clampedLeft}px`,
    top: `${clampedTop}px`
  }

  floatingToolbarVisible.value = true
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
    id: 'guaxi.table.addRowBelow',
    label: '表格：下方插入行',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.DownArrow],
    run: () => addTableRowBelow()
  })

  editor.addAction({
    id: 'guaxi.table.deleteRow',
    label: '表格：删除当前行',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyMod.Shift | monaco.KeyCode.DownArrow],
    run: () => deleteTableRow()
  })

  editor.addAction({
    id: 'guaxi.table.addColumnRight',
    label: '表格：右侧插入列',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.RightArrow],
    run: () => addTableColumnRight()
  })

  editor.addAction({
    id: 'guaxi.table.deleteColumn',
    label: '表格：删除当前列',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyMod.Shift | monaco.KeyCode.RightArrow],
    run: () => deleteTableColumn()
  })

  editor.addAction({
    id: 'guaxi.table.moveCellLeft',
    label: '表格：移动到左侧单元格',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyH],
    run: () => moveTableCell(-1, 0)
  })

  editor.addAction({
    id: 'guaxi.table.moveCellDown',
    label: '表格：移动到下方单元格',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyJ],
    run: () => moveTableCell(0, 1)
  })

  editor.addAction({
    id: 'guaxi.table.moveCellUp',
    label: '表格：移动到上方单元格',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyK],
    run: () => moveTableCell(0, -1)
  })

  editor.addAction({
    id: 'guaxi.table.moveCellRight',
    label: '表格：移动到右侧单元格',
    keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyL],
    run: () => moveTableCell(1, 0)
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
    fontSize: 18,
    fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    padding: { top: 8, bottom: 8 },
    contextmenu: {
      enabled: true,
      addDefaultActions: true,
      addNavigationActions: true
    }
  })

  outlineStore.setJumpToLine(jumpToLine)

  editorDisposables.push(
    editor.onDidChangeModelContent(() => {
      if (settingModel) return
      draft.value = editor.getValue()
      markDirty('typing')

      const model = editor.getModel()
      if (model?.getLanguageId?.() === 'markdown') {
        generateOutlineDebounced(draft.value)
      } else {
        outlineStore.setOutline([])
        outlineVisible.value = false
      }

      scheduleAutoFormatActiveTable()
      updateTableToolbar()
    }),
    editor.onDidChangeCursorPosition(() => {
      updateCursorFromEditor()
      updateTableToolbar()
    }),
    editor.onDidChangeCursorSelection(() => {
      updateFloatingToolbar()
      updateTableToolbar()
    }),
    editor.onDidScrollChange(() => {
      updateFloatingToolbar()
      updateTableToolbar()
    }),
    editor.onDidBlurEditorWidget(() => {
      onBlur()
      floatingToolbarVisible.value = false
      tableToolbarVisible.value = false
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

const canEdit = computed(() => !!activePath.value && !props.unsupported && !blockedLargeFile.value)

const outlineHasContent = computed(() => {
  const v = outlineStore?.outline
  const list = Array.isArray(v) ? v : Array.isArray(v?.value) ? v.value : []
  return list.length > 0
})

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

  if (lang === 'markdown') {
    generateOutlineDebounced(nextContent)
  } else {
    outlineStore.setOutline([])
    outlineVisible.value = false
  }

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

const isPotentialTableLine = (line) => {
  const trimmed = (line || '').trim()
  if (!trimmed) return false
  const pipeCount = (trimmed.match(/\|/g) || []).length
  return pipeCount >= 2
}

const isTableSeparatorLine = (line) => {
  const trimmed = (line || '').trim()
  if (!trimmed) return false
  return /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?$/.test(trimmed)
}

const normalizeTableRowCells = (line) => {
  const raw = (line || '').trim()
  let text = raw
  if (!text.startsWith('|')) text = `| ${text}`
  if (!text.endsWith('|')) text = `${text} |`
  const parts = text.split('|')
  return parts.slice(1, -1).map((c) => c.trim())
}

const detectSeparatorAlign = (cell) => {
  const v = String(cell || '').trim()
  const left = v.startsWith(':')
  const right = v.endsWith(':')
  if (left && right) return 'center'
  if (right) return 'right'
  if (left) return 'left'
  return null
}

const normalizeSeparatorCell = (cell, align) => {
  const base = (cell || '').trim()
  const hasMin = base.replace(/:/g, '').replace(/-/g, '').length === 0
  const detected = align || detectSeparatorAlign(base)

  if (!hasMin) {
    if (detected === 'left') return ':---'
    if (detected === 'center') return ':---:'
    if (detected === 'right') return '---:'
    return '---'
  }

  if (detected === 'left') return ':---'
  if (detected === 'center') return ':---:'
  if (detected === 'right') return '---:'
  return '---'
}

const isCombiningCodePoint = (cp) => {
  return (
    (cp >= 0x0300 && cp <= 0x036f) ||
    (cp >= 0x1ab0 && cp <= 0x1aff) ||
    (cp >= 0x1dc0 && cp <= 0x1dff) ||
    (cp >= 0x20d0 && cp <= 0x20ff) ||
    (cp >= 0xfe20 && cp <= 0xfe2f)
  )
}

const isFullWidthCodePoint = (cp) => {
  if (cp >= 0x1100 && (
    cp <= 0x115f ||
    cp === 0x2329 ||
    cp === 0x232a ||
    (cp >= 0x2e80 && cp <= 0xa4cf && cp !== 0x303f) ||
    (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0xfe10 && cp <= 0xfe19) ||
    (cp >= 0xfe30 && cp <= 0xfe6f) ||
    (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) ||
    (cp >= 0x1f300 && cp <= 0x1f64f) ||
    (cp >= 0x1f900 && cp <= 0x1f9ff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  )) return true

  return false
}

const getDisplayWidth = (input) => {
  const s = String(input ?? '')
  let width = 0

  for (const ch of s) {
    const cp = ch.codePointAt(0) || 0

    if (cp === 0) continue
    if (cp < 32 || cp === 0x7f) continue
    if (isCombiningCodePoint(cp)) continue

    width += isFullWidthCodePoint(cp) ? 2 : 1
  }

  return width
}

const padTableCell = (text, width, align) => {
  const raw = String(text ?? '').trim()
  const current = getDisplayWidth(raw)
  const w = Math.max(0, Number(width) || 0)
  if (current >= w) return raw

  const delta = w - current
  if (align === 'right') return `${' '.repeat(delta)}${raw}`
  if (align === 'center') {
    const left = Math.floor(delta / 2)
    const right = delta - left
    return `${' '.repeat(left)}${raw}${' '.repeat(right)}`
  }
  return `${raw}${' '.repeat(delta)}`
}

const buildAlignedTableLines = (rows) => {
  const safeRows = Array.isArray(rows) ? rows : []
  if (!safeRows.length) return []

  const colCount = safeRows.reduce((m, r) => Math.max(m, r.length), 0) || 1
  const padded = safeRows.map((r) => {
    const next = [...r]
    while (next.length < colCount) next.push('')
    return next
  })

  const sepRow = padded[1] || Array.from({ length: colCount }, () => '---')
  const aligns = sepRow.map((c) => detectSeparatorAlign(c))

  const widths = Array.from({ length: colCount }, (_, i) => {
    let max = 3
    for (const row of padded) {
      const val = row[i] ?? ''
      const w = getDisplayWidth(val)
      max = Math.max(max, w)
    }
    return max
  })

  return padded.map((cells, idx) => {
    if (idx === 1) {
      const sepCells = cells.map((c, i) => {
        const a = aligns[i]
        const w = Math.max(3, widths[i])
        const dashes = '-'.repeat(w)
        if (a === 'left') return `:${dashes}`
        if (a === 'center') return `:${dashes}:`
        if (a === 'right') return `${dashes}:`
        return dashes
      })
      return `| ${sepCells.join(' | ')} |`
    }

    const paddedCells = cells.map((c, i) => padTableCell(c, widths[i], aligns[i]))
    return `| ${paddedCells.join(' | ')} |`
  })
}

const getColumnIndexFromCursor = (line, column, colCount) => {
  const text = line || ''
  const positions = []
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '|') positions.push(i)
  }
  if (positions.length < 2) return 0

  const cursor = Math.max(0, Math.min(text.length, (column || 1) - 1))

  if (cursor <= positions[0]) return 0
  if (cursor > positions[positions.length - 1]) return Math.max(0, (colCount || positions.length - 1) - 1)

  for (let i = 0; i < positions.length - 1; i += 1) {
    if (cursor > positions[i] && cursor <= positions[i + 1]) {
      return Math.max(0, Math.min((colCount || positions.length - 1) - 1, i))
    }
  }

  return 0
}

const detectTableContext = () => {
  if (!editor) return null
  const model = editor.getModel()
  if (!model) return null
  if (model.getLanguageId() !== 'markdown') return null

  const pos = editor.getPosition()
  if (!pos) return null

  const lineNumber = pos.lineNumber
  const lineText = model.getLineContent(lineNumber)
  if (!isPotentialTableLine(lineText)) return null

  const total = model.getLineCount()
  let start = lineNumber
  while (start > 1 && isPotentialTableLine(model.getLineContent(start - 1))) {
    start -= 1
  }

  let end = lineNumber
  while (end < total && isPotentialTableLine(model.getLineContent(end + 1))) {
    end += 1
  }

  let separatorLine = -1
  for (let i = start; i <= end; i += 1) {
    if (isTableSeparatorLine(model.getLineContent(i))) {
      separatorLine = i
      break
    }
  }

  if (separatorLine === -1 || separatorLine === start) return null

  const tableStart = separatorLine - 1

  const rawLines = []
  for (let i = tableStart; i <= end; i += 1) {
    rawLines.push(model.getLineContent(i))
  }

  const rows = rawLines.map((l) => normalizeTableRowCells(l))
  const colCount = rows.reduce((m, r) => Math.max(m, r.length), 0) || 1
  const paddedRows = rows.map((r) => {
    const next = [...r]
    while (next.length < colCount) next.push('')
    return next
  })

  const activeRowIndex = lineNumber - tableStart
  const cursorColIndex = getColumnIndexFromCursor(rawLines[Math.max(0, Math.min(rawLines.length - 1, activeRowIndex))], pos.column, colCount)

  return {
    startLine: tableStart,
    endLine: end,
    separatorLine,
    colCount,
    rows: paddedRows,
    activeRowIndex,
    activeColIndex: cursorColIndex
  }
}

let tableAutoFormatBusy = false

const isTableAutoFormatDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_md_table') || '').trim().toLowerCase()
    return v === '1' || v === 'true' || v === 'yes' || v === 'on'
  } catch {
    return false
  }
}

const scheduleAutoFormatActiveTable = debounce(() => {
  if (!editor) return
  if (tableAutoFormatBusy) return

  const model = editor.getModel()
  const selection = editor.getSelection()
  const pos = editor.getPosition()
  if (!model || !pos || !selection || !selection.isEmpty()) return
  if (model.getLanguageId() !== 'markdown') return

  const ctx = detectTableContext()
  if (!ctx) return

  const rawLines = []
  for (let i = ctx.startLine; i <= ctx.endLine; i += 1) {
    rawLines.push(model.getLineContent(i))
  }

  const nextRows = ctx.rows.map((row, idx) => {
    if (idx !== 1) return row
    return row.map((c) => normalizeSeparatorCell(c, null))
  })

  const nextLines = buildAlignedTableLines(nextRows)
  const currentText = rawLines.join('\n')
  const nextText = nextLines.join('\n')
  if (currentText === nextText) return

  const activeRowIndex = Math.max(0, Math.min(rawLines.length - 1, pos.lineNumber - ctx.startLine))
  const activeColIndex = getColumnIndexFromCursor(rawLines[activeRowIndex] || '', pos.column, ctx.colCount)

  const currentCellColumn = getCursorColumnForTableCell(rawLines[activeRowIndex] || '', activeColIndex, 0)
  const offset = Math.max(0, (pos.column || 1) - currentCellColumn)

  if (isTableAutoFormatDebug()) {
    logger.debug('[MarkdownTable] auto-format', {
      startLine: ctx.startLine,
      endLine: ctx.endLine,
      activeRowIndex,
      activeColIndex,
      offset
    })
  }

  tableAutoFormatBusy = true
  try {
    const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine))
    editor.executeEdits('md-table-auto-format', [
      {
        range,
        text: nextText,
        forceMoveMarkers: true
      }
    ])

    const targetText = nextLines[activeRowIndex] || ''
    const targetCol = getCursorColumnForTableCell(targetText, activeColIndex, offset)
    editor.setPosition(new monaco.Position(ctx.startLine + activeRowIndex, targetCol))
  } finally {
    nextTick(() => {
      tableAutoFormatBusy = false
      updateTableToolbar()
    })
  }
}, 220)

const updateTableToolbar = () => {
  if (!editor) {
    tableToolbarVisible.value = false
    activeTableContext.value = null
    return
  }

  const model = editor.getModel()
  if (!model || model.getLanguageId() !== 'markdown') {
    tableToolbarVisible.value = false
    activeTableContext.value = null
    return
  }

  const ctx = detectTableContext()
  if (!ctx) {
    tableToolbarVisible.value = false
    activeTableContext.value = null
    return
  }

  const wrapperEl = hostRef.value
  const editorEl = editorRef.value
  if (!wrapperEl || !editorEl) {
    tableToolbarVisible.value = false
    activeTableContext.value = null
    return
  }

  const wrapperRect = wrapperEl.getBoundingClientRect()
  const editorRect = editorEl.getBoundingClientRect()

  const leftBase = editorRect.left - wrapperRect.left
  const topBase = editorRect.top - wrapperRect.top

  const cursorPos = editor.getPosition()
  const anchorPos = cursorPos
    ? new monaco.Position(cursorPos.lineNumber, cursorPos.column)
    : new monaco.Position(ctx.startLine, 1)

  const visiblePos = editor.getScrolledVisiblePosition(anchorPos)
  if (!visiblePos) {
    tableToolbarVisible.value = false
    activeTableContext.value = null
    return
  }

  const headerEl = breadcrumbBarRef.value
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0
  const minTop = headerHeight + 8

  const toolbarH = 36
  const toolbarW = 320
  const gap = 8

  const preferTop = topBase + visiblePos.top - toolbarH - gap
  const fallbackTop = topBase + visiblePos.top + visiblePos.height + gap
  const top = preferTop >= minTop ? preferTop : fallbackTop
  const clampedTop = Math.min(
    Math.max(minTop, top),
    Math.max(minTop, wrapperRect.height - toolbarH - 8)
  )

  const layout = editor.getLayoutInfo ? editor.getLayoutInfo() : null
  const contentLeft = layout && Number.isFinite(layout.contentLeft) ? layout.contentLeft : 0
  const minLeft = Math.max(8, leftBase + contentLeft)

  const left = leftBase + visiblePos.left
  const clampedLeft = Math.min(
    Math.max(minLeft, left),
    Math.max(minLeft, wrapperRect.width - toolbarW - 8)
  )

  tableToolbarStyle.value = {
    left: `${clampedLeft}px`,
    top: `${clampedTop}px`
  }

  if (isTableAutoFormatDebug()) {
    logger.debug('[MarkdownTable] toolbar', {
      left: clampedLeft,
      top: clampedTop,
      contentLeft,
      anchor: { lineNumber: anchorPos.lineNumber, column: anchorPos.column }
    })
  }

  activeTableContext.value = ctx
  tableToolbarVisible.value = true
}

const getPipePositions = (line) => {
  const text = String(line || '')
  const positions = []
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '|') positions.push(i)
  }
  return positions
}

const getCursorColumnForTableCell = (line, colIndex, offset = 0) => {
  const text = String(line || '')
  const pipes = getPipePositions(text)
  if (pipes.length < 2) return 1

  const col = Math.max(0, Math.min(pipes.length - 2, Number(colIndex) || 0))
  const left = pipes[col]
  const right = pipes[col + 1]
  const cellStart = Math.min(text.length, left + 2)
  const cellEnd = Math.max(cellStart, right - 1)
  const off = Math.max(0, Math.min(cellEnd - cellStart, Number(offset) || 0))

  return Math.max(1, Math.min(text.length + 1, cellStart + off + 1))
}

const applyTableEdits = (nextRows, nextActiveRowIndex, nextActiveColIndex) => {
  if (!editor) return
  const model = editor.getModel()
  const ctx = activeTableContext.value
  if (!model || !ctx) return

  const normalizedRows = nextRows.map((row, idx) => {
    if (idx !== 1) return row
    return row.map((c) => normalizeSeparatorCell(c, null))
  })

  const textLines = buildAlignedTableLines(normalizedRows)

  const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine))
  editor.executeEdits('md-table-toolbar', [
    {
      range,
      text: textLines.join('\n'),
      forceMoveMarkers: true
    }
  ])

  const row = Math.max(2, Math.min(textLines.length - 1, nextActiveRowIndex))
  const col = Math.max(0, Math.min((normalizedRows[0]?.length || 1) - 1, nextActiveColIndex))
  const targetLine = ctx.startLine + row
  const targetText = textLines[row] || ''
  const targetCol = getCursorColumnForTableCell(targetText, col, 0)

  editor.setPosition(new monaco.Position(targetLine, targetCol))
  editor.focus()

  nextTick(() => {
    updateTableToolbar()
  })
}

const addTableRowBelow = () => {
  const ctx = activeTableContext.value
  if (!ctx || !ctx.rows?.length) return

  const insertAt = ctx.activeRowIndex <= 1 ? 2 : Math.min(ctx.rows.length, ctx.activeRowIndex + 1)
  const empty = Array.from({ length: ctx.colCount }, () => '')

  const nextRows = [...ctx.rows]
  nextRows.splice(insertAt, 0, empty)

  applyTableEdits(nextRows, insertAt, ctx.activeColIndex)
}

const deleteTableRow = () => {
  const ctx = activeTableContext.value
  if (!ctx || !ctx.rows?.length) return

  if (ctx.activeRowIndex <= 1) {
    message.warning('无法删除表头或分隔行')
    return
  }

  const nextRows = [...ctx.rows]
  if (nextRows.length <= 2) return

  nextRows.splice(ctx.activeRowIndex, 1)
  const nextRow = Math.min(nextRows.length - 1, ctx.activeRowIndex)

  applyTableEdits(nextRows, nextRow, ctx.activeColIndex)
}

const addTableColumnRight = () => {
  const ctx = activeTableContext.value
  if (!ctx || !ctx.rows?.length) return

  const insertAt = Math.min(ctx.colCount, ctx.activeColIndex + 1)
  const nextRows = ctx.rows.map((row, rowIndex) => {
    const next = [...row]
    if (rowIndex === 1) {
      next.splice(insertAt, 0, '---')
    } else {
      next.splice(insertAt, 0, '')
    }
    return next
  })

  const nextCol = insertAt
  applyTableEdits(nextRows, ctx.activeRowIndex, nextCol)
}

const deleteTableColumn = () => {
  const ctx = activeTableContext.value
  if (!ctx || !ctx.rows?.length) return

  if (ctx.colCount <= 1) {
    message.warning('至少保留 1 列')
    return
  }

  const removeAt = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex))
  const nextRows = ctx.rows.map((row) => {
    const next = [...row]
    next.splice(removeAt, 1)
    return next
  })

  const nextCol = Math.max(0, Math.min(ctx.colCount - 2, removeAt))
  applyTableEdits(nextRows, ctx.activeRowIndex, nextCol)
}

const setTableColumnAlign = (align) => {
  const ctx = activeTableContext.value
  if (!ctx || !ctx.rows?.length) return

  const col = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex))
  const nextRows = ctx.rows.map((row, rowIndex) => {
    if (rowIndex !== 1) return row
    const next = [...row]
    next[col] = normalizeSeparatorCell(next[col], align)
    return next
  })

  applyTableEdits(nextRows, ctx.activeRowIndex, ctx.activeColIndex)
}

const formatTable = () => {
  const ctx = activeTableContext.value
  if (!ctx || !ctx.rows?.length) return

  const nextRows = ctx.rows.map((row, rowIndex) => {
    if (rowIndex === 1) {
      return row.map((c) => normalizeSeparatorCell(c, null))
    }
    return row.map((c) => (c ?? '').trim())
  })

  applyTableEdits(nextRows, ctx.activeRowIndex, ctx.activeColIndex)
}

const moveTableCell = (dx, dy) => {
  if (!editor) return

  const model = editor.getModel()
  const pos = editor.getPosition()
  const selection = editor.getSelection()
  if (!model || !pos || !selection) return
  if (model.getLanguageId() !== 'markdown') return
  if (!selection.isEmpty()) return

  const ctx = detectTableContext()
  if (!ctx) return

  const maxRow = Math.max(0, ctx.rows.length - 1)
  const nextCol = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex + (Number(dx) || 0)))

  let nextRow = Math.max(0, Math.min(maxRow, ctx.activeRowIndex + (Number(dy) || 0)))
  if (nextRow === 1 && dy < 0) nextRow = 0
  if (nextRow === 1 && dy > 0) nextRow = Math.min(maxRow, 2)

  const currentLine = model.getLineContent(ctx.startLine + ctx.activeRowIndex)
  const currentCellCol = getCursorColumnForTableCell(currentLine, ctx.activeColIndex, 0)
  const offset = Math.max(0, (pos.column || 1) - currentCellCol)

  const targetLineNumber = ctx.startLine + nextRow
  const targetLine = model.getLineContent(targetLineNumber)
  const targetCol = getCursorColumnForTableCell(targetLine, nextCol, offset)

  editor.setPosition(new monaco.Position(targetLineNumber, targetCol))
  editor.focus()
  updateTableToolbar()
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
  redo,
  toggleOutlineVisible,
  captureScreenshot,
  outlineHasContent
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

.breadcrumb-bar {
  display: flex;
  align-items: center;
  height: 30px;
  min-height: 30px;
  padding: 0 8px;
  background-color: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  box-sizing: border-box;
  overflow: hidden;
}

.breadcrumb-scroll {
  flex: 1 1 auto;
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-gutter: stable;
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.22) transparent;
}

.breadcrumb-scroll::-webkit-scrollbar {
  height: 6px;
}

.breadcrumb-scroll::-webkit-scrollbar-track {
  background: transparent;
}

.breadcrumb-scroll::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.18);
  border-radius: 999px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.breadcrumb-scroll::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.28);
}

:global(:root[data-guaxi-theme='dark']) .breadcrumb-scroll {
  scrollbar-color: rgba(255, 255, 255, 0.22) transparent;
}

:global(:root[data-guaxi-theme='dark']) .breadcrumb-scroll::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.18);
}

:global(:root[data-guaxi-theme='dark']) .breadcrumb-scroll::-webkit-scrollbar-thumb:hover {
  background-color: rgba(255, 255, 255, 0.26);
}

.breadcrumb-bar :deep(.n-breadcrumb) {
  white-space: nowrap;
}

.breadcrumb-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 320px;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  vertical-align: bottom;
  cursor: pointer;
  user-select: none;
  padding: 0 4px;
  border-radius: 4px;
}

.breadcrumb-item.disabled {
  cursor: default;
  opacity: 0.6;
}

.breadcrumb-icon {
  flex: 0 0 auto;
  opacity: 0.85;
}

.breadcrumb-label {
  display: inline-block;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.breadcrumb-item:hover {
  color: var(--n-color-primary, #18a058);
  background: rgba(127, 127, 127, 0.12);
}

.breadcrumb-item.disabled:hover {
  color: inherit;
  background: transparent;
}

.floating-md-toolbar {
  position: absolute;
  z-index: 9;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border-radius: 8px;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
}

.floating-md-toolbar :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 6px;
}

.floating-table-toolbar {
  position: absolute;
  z-index: 9;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border-radius: 8px;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
}

.floating-table-toolbar :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 6px;
}

.floating-table-toolbar-divider {
  width: 1px;
  height: 16px;
  background-color: var(--guaxi-border);
  margin: 0 2px;
}

.outline-drawer :deep(.n-drawer-body) {
  padding: 0;
}

.outline-drawer-body {
  height: 100%;
  padding: 0;
  overflow: hidden;
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

.lite-editor :deep(.monaco-scrollable-element) {
  scrollbar-width: thin;
  scrollbar-color: rgba(120, 120, 120, 0.45) transparent;
}

.lite-editor :deep(.monaco-scrollable-element::-webkit-scrollbar) {
  width: 8px;
  height: 8px;
}

.lite-editor :deep(.monaco-scrollable-element::-webkit-scrollbar-track) {
  background: transparent;
}

.lite-editor :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(120, 120, 120, 0.45);
  border-radius: 999px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.lite-editor :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(120, 120, 120, 0.65);
}

:global(:root[data-guaxi-theme='dark']) .lite-editor :deep(.monaco-scrollable-element) {
  scrollbar-color: rgba(255, 255, 255, 0.28) transparent;
}

:global(:root[data-guaxi-theme='dark']) .lite-editor :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(255, 255, 255, 0.28);
}

:global(:root[data-guaxi-theme='dark']) .lite-editor :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(255, 255, 255, 0.45);
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
