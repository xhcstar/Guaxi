<template>
  <n-modal
    :show="show"
    preset="card"
    :bordered="false"
    :mask-closable="false"
    class="gm-modal"
    :style="{ width: '94vw', maxWidth: '1640px' }"
    @update:show="handleShowUpdate"
  >
    <template #header>
      <div class="gm-title">
        <span class="codicon codicon-git-merge gm-title-icon" aria-hidden="true" />
        <span class="gm-title-file" :title="filePath">{{ filePath }}</span>
        <span v-if="loading" class="gm-badge">读取冲突…</span>
        <span v-else-if="binary" class="gm-badge is-warning">二进制文件</span>
        <span v-else-if="conflicts.length" class="gm-badge is-warning">
          {{ conflicts.length }} 处冲突未解决
        </span>
        <span v-else class="gm-badge is-ok">没有冲突标记</span>
        <button class="gm-icon-button" type="button" title="重新载入" @click="reload">
          <span class="codicon codicon-refresh" aria-hidden="true" />
        </button>
      </div>
    </template>

    <div class="gm-body">
      <div v-if="loading" class="gm-loading">
        <n-spin size="large" description="正在读取冲突内容…" />
      </div>

      <div v-else-if="loadError" class="gm-error">{{ loadError }}</div>

      <div v-else-if="binary" class="gm-error">
        这是二进制文件，无法在合并编辑器中处理。请在外部工具中解决后，再在侧栏将其标记为已解决。
      </div>

      <template v-else>
        <div class="gm-toolbar">
          <div class="gm-nav">
            <button
              class="gm-icon-button"
              type="button"
              title="上一个冲突"
              :disabled="!conflicts.length"
              @click="prevConflict"
            >
              <span class="codicon codicon-chevron-up" aria-hidden="true" />
            </button>
            <span class="gm-counter">
              {{ conflicts.length ? `${focusedIndex + 1} / ${conflicts.length}` : '0' }} 处冲突
            </span>
            <button
              class="gm-icon-button"
              type="button"
              title="下一个冲突"
              :disabled="!conflicts.length"
              @click="nextConflict"
            >
              <span class="codicon codicon-chevron-down" aria-hidden="true" />
            </button>
          </div>

          <div class="gm-toolbar-actions">
            <button type="button" :disabled="!conflicts.length" @click="acceptAll('ours')">全部采用当前</button>
            <button type="button" :disabled="!conflicts.length" @click="acceptAll('theirs')">全部采用传入</button>
            <button type="button" :disabled="!conflicts.length" @click="resetResult">重置结果</button>
          </div>
        </div>

        <div class="gm-grid">
          <section class="gm-pane gm-pane-incoming">
            <header class="gm-pane-head is-incoming">
              <span class="gm-dot" />
              <span>传入</span>
              <span class="gm-pane-hint">incoming / theirs</span>
            </header>
            <div ref="incomingHost" class="gm-editor"></div>
          </section>

          <section class="gm-pane gm-pane-current">
            <header class="gm-pane-head is-current">
              <span class="gm-dot" />
              <span>当前</span>
              <span class="gm-pane-hint">current / HEAD</span>
            </header>
            <div ref="currentHost" class="gm-editor"></div>
          </section>

          <section class="gm-pane gm-pane-result">
            <header class="gm-pane-head is-result">
              <span class="gm-dot" />
              <span>合并结果</span>
              <span class="gm-pane-hint">editable result</span>
              <span class="gm-result-count" :class="{ 'is-pending': conflicts.length }">
                {{ conflicts.length ? `${conflicts.length} 处未解决` : '可以完成合并' }}
              </span>
            </header>
            <div ref="resultHost" class="gm-editor"></div>
          </section>
        </div>
      </template>
    </div>

    <template #footer>
      <div class="gm-footer">
        <span v-if="saveError" class="gm-save-error">{{ saveError }}</span>
        <span v-else-if="conflicts.length" class="gm-footer-hint">
          仍有 {{ conflicts.length }} 处冲突，全部解决后才能完成合并
        </span>
        <span v-else class="gm-footer-hint">结果区可直接编辑；完成合并会写入文件并 git add</span>
        <n-button @click="close">取消</n-button>
        <n-button
          type="primary"
          :loading="saving"
          :disabled="loading || saving || binary || conflicts.length > 0"
          @click="save"
        >
          完成合并
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { NModal, NButton, NSpin } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import * as monaco from 'monaco-editor'
import { useSettingsStore } from '../../stores/settingsStore'
import {
  defineGuaxiMonacoThemes,
  getGuaxiMonacoTheme,
  getLanguageForFile,
} from '../../monaco/guaxiMonacoThemes'
import { ensureCodiconStyles } from '../../utils/codiconFont'
import { createLogger } from '../../utils/logger'

ensureCodiconStyles()

const logger = createLogger('GitMergeEditor')
const settingsStore = useSettingsStore()

const props = defineProps({
  show: { type: Boolean, default: false },
  workspacePath: { type: String, required: true },
  filePath: { type: String, default: '' },
})

const emit = defineEmits(['update:show', 'saved'])

const loading = ref(false)
const saving = ref(false)
const loadError = ref('')
const saveError = ref('')
const binary = ref(false)

const incomingText = ref('')
const currentText = ref('')
const resultOriginal = ref('')
const result = ref('')
const eol = ref('\n')

const focusedIndex = ref(0)

const incomingHost = ref(null)
const currentHost = ref(null)
const resultHost = ref(null)

let incomingEditor = null
let currentEditor = null
let resultEditor = null
let incomingDecorations = null
let currentDecorations = null
let resultDecorations = null
let resultZoneIds = []
let refreshTimer = null
let loadedFilePath = ''
let themesReady = false
let syncingScroll = false

const ensureThemes = () => {
  if (themesReady) return
  defineGuaxiMonacoThemes(monaco)
  themesReady = true
}

const getMonacoTheme = () => {
  ensureThemes()
  return getGuaxiMonacoTheme(settingsStore.appearanceTheme)
}

const normalizeEol = (text) => String(text || '').replace(/\r\n/g, '\n')

const parseConflictBlocks = (text) => {
  const blocks = []
  const normalized = normalizeEol(text)
  const pattern = /^<{7}[^\n]*\n([\s\S]*?)(?:^\|{7}[^\n]*\n([\s\S]*?))?^={7}\n([\s\S]*?)^>{7}[^\n]*(?:\n|$)/gm
  let match = pattern.exec(normalized)
  while (match) {
    blocks.push({
      start: match.index,
      end: match.index + match[0].length,
      raw: match[0],
      ours: match[1] ?? '',
      base: match[2] ?? '',
      theirs: match[3] ?? '',
    })
    match = pattern.exec(normalized)
  }
  return blocks
}

const lineAt = (text, offset) => String(text).slice(0, Math.max(0, offset)).split('\n').length

const makeLineRange = (text, start, end) => {
  const startLine = lineAt(text, start)
  const lastOffset = Math.max(start, (end ?? start) - 1)
  return {
    startLine,
    endLine: Math.max(startLine, lineAt(text, lastOffset)),
  }
}

/**
 * 在完整版本里定位冲突片段。
 *
 * git 写进工作区的冲突块就是 ours/theirs 文件的连续行，所以顺序 indexOf 能精确
 * 找到它在左侧/右侧编辑器中的位置；用游标而不是全局搜索，避免同一段代码重复出现时
 * 所有冲突都指向第一处。
 */
const locateFragment = (text, fragment, cursor) => {
  if (!fragment) return null
  const normalized = normalizeEol(fragment)
  const candidates = [normalized, normalized.replace(/\n$/, '')]
  for (const candidate of candidates) {
    if (!candidate) continue
    const index = text.indexOf(candidate, cursor)
    if (index >= 0) {
      const end = index + candidate.length
      return { start: index, end, nextCursor: end }
    }
  }
  return null
}

const conflicts = computed(() => {
  const text = result.value
  const blocks = parseConflictBlocks(text)
  let currentCursor = 0
  let incomingCursor = 0

  return blocks.map((block, index) => {
    const resultRange = makeLineRange(text, block.start, block.end)

    const currentHit = locateFragment(currentText.value, block.ours, currentCursor)
    if (currentHit) currentCursor = currentHit.nextCursor

    const incomingHit = locateFragment(incomingText.value, block.theirs, incomingCursor)
    if (incomingHit) incomingCursor = incomingHit.nextCursor

    return {
      ...block,
      index,
      resultRange,
      currentRange: currentHit
        ? makeLineRange(currentText.value, currentHit.start, currentHit.end)
        : null,
      incomingRange: incomingHit
        ? makeLineRange(incomingText.value, incomingHit.start, incomingHit.end)
        : null,
    }
  })
})

watch(  () => conflicts.value.length,
  (count) => {
    if (focusedIndex.value >= count) {
      focusedIndex.value = Math.max(0, count - 1)
    }
  }
)

const baseEditorOptions = (readOnly) => ({
  value: '',
  language: getLanguageForFile(props.filePath),
  theme: getMonacoTheme(),
  readOnly,
  domReadOnly: readOnly,
  automaticLayout: true,
  minimap: { enabled: false },
  scrollBeyondLastLine: false,
  fontSize: 13,
  lineHeight: 20,
  lineNumbersMinChars: 4,
  renderWhitespace: 'selection',
  wordWrap: 'off',
  folding: false,
  stickyScroll: { enabled: false },
  scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10 },
})

const disposeEditors = () => {
  if (refreshTimer) {
    clearTimeout(refreshTimer)
    refreshTimer = null
  }
  for (const editor of [incomingEditor, currentEditor, resultEditor]) {
    if (!editor) continue
    const model = editor.getModel()
    editor.dispose()
    model?.dispose?.()
  }
  incomingEditor = null
  currentEditor = null
  resultEditor = null
  incomingDecorations = null
  currentDecorations = null
  resultDecorations = null
  resultZoneIds = []
}

const withNewline = (fragment) => {
  if (!fragment) return ''
  return fragment.endsWith('\n') ? fragment : `${fragment}\n`
}

const buildLineDecorations = (entries, kindName, focused) => entries
  .filter((entry) => entry[`${kindName}Range`])
  .map((entry) => {
    const range = entry[`${kindName}Range`]
    const focusedClass = entry.index === focused ? ' gm-region-focused' : ''
    return {
      range: new monaco.Range(range.startLine, 1, range.endLine, 1),
      options: {
        isWholeLine: true,
        className: `gm-line gm-line-${kindName}${focusedClass}`,
        linesDecorationsClassName: `gm-gutter gm-gutter-${kindName}`,
        overviewRuler: {
          color: kindName === 'incoming'
            ? '#58a6ff'
            : (kindName === 'current' ? '#3fb950' : '#d29922'),
          position: monaco.editor.OverviewRulerLane.Left,
        },
      },
    }
  })

const renderResultZones = () => {
  if (!resultEditor) return

  const previousZoneIds = resultZoneIds
  resultZoneIds = []
  const entries = conflicts.value

  resultEditor.changeViewZones((accessor) => {
    previousZoneIds.forEach((zoneId) => accessor.removeZone(zoneId))
    if (!entries.length) return

    for (const entry of entries) {
      const dom = document.createElement('div')
      dom.className = 'gm-zone-actions'

      const label = document.createElement('span')
      label.className = 'gm-zone-label'
      label.textContent = `冲突 ${entry.index + 1}`
      dom.appendChild(label)

      const addButton = (text, choice, className) => {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = `gm-zone-button${className ? ` ${className}` : ''}`
        button.textContent = text
        button.addEventListener('mousedown', (event) => event.preventDefault())
        button.addEventListener('click', (event) => {
          event.preventDefault()
          event.stopPropagation()
          applyChoice(entry.index, choice)
        })
        dom.appendChild(button)
      }

      addButton('采用传入', 'theirs', 'is-incoming')
      addButton('采用当前', 'ours', 'is-current')
      addButton('两者都保留', 'both')

      const zoneId = accessor.addZone({
        afterLineNumber: Math.max(0, entry.resultRange.startLine - 1),
        heightInLines: 1.7,
        domNode: dom,
      })
      resultZoneIds.push(zoneId)
    }
  })
}

const renderDecorations = () => {
  const entries = conflicts.value
  if (incomingDecorations) {
    incomingDecorations.set(buildLineDecorations(entries, 'incoming', focusedIndex.value))
  }
  if (currentDecorations) {
    currentDecorations.set(buildLineDecorations(entries, 'current', focusedIndex.value))
  }
  if (resultDecorations) {
    resultDecorations.set(entries.map((entry) => ({
      range: new monaco.Range(entry.resultRange.startLine, 1, entry.resultRange.endLine, 1),
      options: {
        isWholeLine: true,
        className: `gm-line gm-line-result${entry.index === focusedIndex.value ? ' gm-region-focused' : ''}`,
        linesDecorationsClassName: 'gm-gutter gm-gutter-result',
      },
    })))
  }
}

const renderAll = () => {
  renderDecorations()
  renderResultZones()
}

const scheduleRender = () => {
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = setTimeout(() => {
    refreshTimer = null
    renderAll()
  }, 60)
}

const focusConflict = (index, reveal = true) => {
  const entries = conflicts.value
  if (!entries.length) {
    focusedIndex.value = 0
    renderAll()
    return
  }
  const nextIndex = Math.max(0, Math.min(index, entries.length - 1))
  focusedIndex.value = nextIndex
  renderDecorations()

  if (!reveal) return
  const entry = entries[nextIndex]
  if (!entry) return
  if (resultEditor) {
    resultEditor.revealLineInCenter(entry.resultRange.startLine)
  }
  if (currentEditor && entry.currentRange) {
    currentEditor.revealLineInCenter(entry.currentRange.startLine)
  }
  if (incomingEditor && entry.incomingRange) {
    incomingEditor.revealLineInCenter(entry.incomingRange.startLine)
  }
}

const prevConflict = () => focusConflict(focusedIndex.value - 1)
const nextConflict = () => focusConflict(focusedIndex.value + 1)

const applyChoice = (index, choice) => {
  if (!resultEditor) return
  const model = resultEditor.getModel()
  if (!model) return

  const blocks = parseConflictBlocks(model.getValue())
  const block = blocks[index]
  if (!block) return

  let replacement = block.raw
  if (choice === 'ours') replacement = withNewline(block.ours)
  else if (choice === 'theirs') replacement = withNewline(block.theirs)
  else if (choice === 'both') replacement = `${withNewline(block.ours)}${withNewline(block.theirs)}`

  const start = model.getPositionAt(block.start)
  const end = model.getPositionAt(block.end)
  resultEditor.executeEdits('gm-accept', [{
    range: monaco.Range.fromPositions(start, end),
    text: replacement,
    forceMoveMarkers: true,
  }])
  resultEditor.pushUndoStop()

  const remaining = parseConflictBlocks(model.getValue()).length
  focusedIndex.value = Math.min(index, Math.max(0, remaining - 1))
  scheduleRender()
  window.requestAnimationFrame(() => focusConflict(focusedIndex.value, false))
}

const acceptAll = (choice) => {
  if (!resultEditor) return
  const model = resultEditor.getModel()
  if (!model) return

  const blocks = parseConflictBlocks(model.getValue())
  if (!blocks.length) return

  const edits = []
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index]
    let replacement = block.raw
    if (choice === 'ours') replacement = withNewline(block.ours)
    else if (choice === 'theirs') replacement = withNewline(block.theirs)
    else if (choice === 'both') replacement = `${withNewline(block.ours)}${withNewline(block.theirs)}`

    edits.push({
      range: monaco.Range.fromPositions(
        model.getPositionAt(block.start),
        model.getPositionAt(block.end)
      ),
      text: replacement,
      forceMoveMarkers: true,
    })
  }

  resultEditor.executeEdits('gm-accept-all', edits)
  resultEditor.pushUndoStop()
  focusedIndex.value = 0
  scheduleRender()
}

const resetResult = () => {
  if (!resultEditor) return
  const model = resultEditor.getModel()
  if (!model) return
  model.setValue(resultOriginal.value)
  result.value = resultOriginal.value
  focusedIndex.value = 0
  scheduleRender()
  window.requestAnimationFrame(() => focusConflict(0, false))
}

const bindScrollSync = () => {
  const bind = (source, target) => {
    if (!source || !target) return
    source.onDidScrollChange((event) => {
      if (syncingScroll || !event.scrollTopChanged) return
      syncingScroll = true
      target.setScrollTop(event.scrollTop)
      syncingScroll = false
    })
  }
  bind(incomingEditor, currentEditor)
  bind(currentEditor, incomingEditor)
}

const createEditors = () => {
  disposeEditors()
  if (binary.value) return
  if (!incomingHost.value || !currentHost.value || !resultHost.value) return

  incomingEditor = monaco.editor.create(incomingHost.value, {
    ...baseEditorOptions(true),
    value: incomingText.value,
  })
  currentEditor = monaco.editor.create(currentHost.value, {
    ...baseEditorOptions(true),
    value: currentText.value,
  })
  resultEditor = monaco.editor.create(resultHost.value, {
    ...baseEditorOptions(false),
    value: result.value,
  })

  incomingDecorations = incomingEditor.createDecorationsCollection()
  currentDecorations = currentEditor.createDecorationsCollection()
  resultDecorations = resultEditor.createDecorationsCollection()

  resultEditor.onDidChangeModelContent(() => {
    const value = resultEditor.getValue()
    if (value !== result.value) {
      result.value = value
    }
    scheduleRender()
  })

  resultEditor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
    save()
  })

  bindScrollSync()
  renderAll()

  window.setTimeout(() => {
    if (!resultEditor) return
    const totalLines = resultEditor.getModel()?.getLineCount() || 0
    if (conflicts.value.length) {
      focusConflict(0, true)
    } else if (totalLines > 0) {
      resultEditor.revealLine(1)
    }
  }, 80)
}

const loadConflict = async () => {
  loading.value = true
  loadError.value = ''
  saveError.value = ''
  loadedFilePath = props.filePath
  try {
    const data = await invoke('get_merge_conflict', {
      path: props.workspacePath,
      filePath: props.filePath,
    })
    const working = String(data?.working || '')
    eol.value = working.includes('\r\n') ? '\r\n' : '\n'
    binary.value = !!data?.binary
    incomingText.value = normalizeEol(data?.theirs || '')
    currentText.value = normalizeEol(data?.ours || '')
    resultOriginal.value = normalizeEol(working)
    result.value = resultOriginal.value
    focusedIndex.value = 0
    logger.info('[MergeEditor] Loaded conflict stages', {
      filePath: props.filePath,
      binary: binary.value,
      conflicts: conflicts.value.length,
    })
  } catch (error) {
    loadError.value = String(error?.message || error)
    logger.error('[MergeEditor] Failed to load conflict', {
      filePath: props.filePath,
      error: loadError.value,
    })
  } finally {
    loading.value = false
  }
}

const reload = async () => {
  await loadConflict()
  await nextTick()
  createEditors()
}

const close = () => {
  saveError.value = ''
  emit('update:show', false)
}

const handleShowUpdate = (visible) => {
  if (!visible) {
    saveError.value = ''
    emit('update:show', false)
  }
}

const save = async () => {
  if (conflicts.value.length > 0 || saving.value || binary.value) return
  saving.value = true
  saveError.value = ''
  try {
    const content = resultEditor ? resultEditor.getValue() : result.value
    const payload = eol.value === '\r\n' ? content.replace(/\n/g, '\r\n') : content
    await invoke('resolve_conflict', {
      path: props.workspacePath,
      filePath: props.filePath,
      content: payload,
    })
    logger.info('[MergeEditor] Conflict resolved', { filePath: props.filePath })
    emit('saved', props.filePath)
    close()
  } catch (error) {
    saveError.value = String(error?.message || error)
    logger.error('[MergeEditor] Failed to resolve conflict', {
      filePath: props.filePath,
      error: saveError.value,
    })
  } finally {
    saving.value = false
  }
}

watch(
  () => props.show,
  async (visible) => {
    if (!visible) {
      disposeEditors()
      return
    }
    await loadConflict()
    await nextTick()
    createEditors()
  },
  { immediate: true }
)

watch(
  () => props.filePath,
  async () => {
    if (!props.show || loadedFilePath === props.filePath) return
    await reload()
  }
)

watch(
  () => settingsStore.appearanceTheme,
  () => {
    monaco.editor.setTheme(getMonacoTheme())
  }
)

onBeforeUnmount(() => {
  disposeEditors()
})
</script>

<style scoped>
.gm-modal :deep(.n-card__content) {
  padding: 8px 0 4px;
}

.gm-title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.gm-title-icon {
  color: var(--text-color-secondary);
}

.gm-title-file {
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.gm-badge {
  flex: 0 0 auto;
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px solid var(--divider-color);
  font-size: 11px;
  color: var(--text-color-secondary);
}

.gm-badge.is-warning {
  color: #d29922;
  border-color: color-mix(in srgb, #d29922 45%, transparent);
  background-color: color-mix(in srgb, #d29922 12%, transparent);
}

.gm-badge.is-ok {
  color: #2da44e;
  border-color: color-mix(in srgb, #2da44e 45%, transparent);
  background-color: color-mix(in srgb, #2da44e 12%, transparent);
}

.gm-icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-color-secondary);
  cursor: pointer;
}

.gm-icon-button:hover:not(:disabled) {
  background-color: var(--hover-color);
  color: var(--text-color);
}

.gm-icon-button:disabled {
  opacity: 0.4;
  cursor: default;
}

.gm-title .gm-icon-button {
  margin-left: auto;
}

.gm-body {
  display: flex;
  flex-direction: column;
  height: min(74vh, 860px);
  min-height: 420px;
}

.gm-loading {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.gm-error {
  flex: 1;
  padding: 24px;
  color: #f85149;
  font-size: 13px;
  line-height: 1.7;
}

.gm-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 2px 8px;
  flex: 0 0 auto;
}

.gm-nav {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.gm-counter {
  font-size: 11.5px;
  color: var(--text-color-secondary);
  min-width: 58px;
  text-align: center;
}

.gm-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.gm-toolbar-actions button {
  padding: 3px 8px;
  border: 1px solid var(--divider-color);
  border-radius: 4px;
  background: transparent;
  color: var(--text-color);
  font: inherit;
  font-size: 11.5px;
  cursor: pointer;
}

.gm-toolbar-actions button:hover:not(:disabled) {
  background-color: var(--hover-color);
}

.gm-toolbar-actions button:disabled {
  opacity: 0.45;
  cursor: default;
}

.gm-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  grid-template-rows: minmax(0, 0.92fr) minmax(0, 1.08fr);
  gap: 8px;
}

.gm-pane {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  border: 1px solid var(--divider-color);
  border-radius: 6px;
  overflow: hidden;
  background-color: var(--git-monaco-bg, var(--bg-color));
}

.gm-pane-result {
  grid-column: 1 / -1;
}

.gm-pane-head {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
  height: 26px;
  padding: 0 8px;
  border-bottom: 1px solid var(--divider-color);
  background-color: var(--card-bg);
  font-size: 11.5px;
  font-weight: 600;
}

.gm-pane-head .gm-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.gm-pane-head.is-incoming .gm-dot {
  background-color: #58a6ff;
}

.gm-pane-head.is-current .gm-dot {
  background-color: #3fb950;
}

.gm-pane-head.is-result .gm-dot {
  background-color: #d29922;
}

.gm-pane-hint {
  font-weight: 400;
  color: var(--text-color-secondary);
  font-size: 10.5px;
}

.gm-result-count {
  margin-left: auto;
  font-weight: 400;
  font-size: 11px;
  color: #2da44e;
}

.gm-result-count.is-pending {
  color: #d29922;
}

.gm-editor {
  flex: 1;
  min-height: 0;
}

.gm-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.gm-footer-hint {
  margin-right: auto;
  font-size: 12px;
  color: var(--text-color-secondary);
}

.gm-save-error {
  margin-right: auto;
  font-size: 12px;
  color: #f85149;
}

@media (max-width: 960px) {
  .gm-grid {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1.1fr);
  }

  .gm-pane-result {
    grid-column: auto;
  }

  .gm-body {
    height: min(80vh, 900px);
  }
}
</style>

<style>
/* Monaco 内部由装饰器和 view zone 注入的 DOM 不在 Vue 模板里，样式必须全局；
   统一用 gm- 前缀，避免影响主编辑器的 diff 装饰。 */
.gm-line {
  box-sizing: border-box;
}

.gm-line-incoming {
  background-color: rgba(88, 166, 255, 0.13);
  box-shadow: inset 2px 0 0 rgba(88, 166, 255, 0.75),
    inset 0 1px 0 rgba(88, 166, 255, 0.35),
    inset 0 -1px 0 rgba(88, 166, 255, 0.35);
}

.gm-line-current {
  background-color: rgba(63, 185, 80, 0.13);
  box-shadow: inset 2px 0 0 rgba(63, 185, 80, 0.75),
    inset 0 1px 0 rgba(63, 185, 80, 0.35),
    inset 0 -1px 0 rgba(63, 185, 80, 0.35);
}

.gm-line-result {
  background-color: rgba(210, 153, 34, 0.12);
  box-shadow: inset 2px 0 0 rgba(210, 153, 34, 0.8),
    inset 0 1px 0 rgba(210, 153, 34, 0.35),
    inset 0 -1px 0 rgba(210, 153, 34, 0.35);
}

.gm-line.gm-region-focused {
  background-color: rgba(210, 153, 34, 0.2);
  box-shadow: inset 2px 0 0 #d29922,
    inset 0 1px 0 rgba(210, 153, 34, 0.6),
    inset 0 -1px 0 rgba(210, 153, 34, 0.6);
}

.gm-line-incoming.gm-region-focused {
  background-color: rgba(88, 166, 255, 0.2);
  box-shadow: inset 2px 0 0 #58a6ff,
    inset 0 1px 0 rgba(88, 166, 255, 0.6),
    inset 0 -1px 0 rgba(88, 166, 255, 0.6);
}

.gm-line-current.gm-region-focused {
  background-color: rgba(63, 185, 80, 0.2);
  box-shadow: inset 2px 0 0 #3fb950,
    inset 0 1px 0 rgba(63, 185, 80, 0.6),
    inset 0 -1px 0 rgba(63, 185, 80, 0.6);
}

.gm-gutter {
  width: 4px !important;
  margin-left: 2px;
  border-radius: 2px;
}

.gm-gutter-incoming {
  background-color: rgba(88, 166, 255, 0.8);
}

.gm-gutter-current {
  background-color: rgba(63, 185, 80, 0.8);
}

.gm-gutter-result {
  background-color: rgba(210, 153, 34, 0.85);
}

.gm-zone-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 100%;
  padding: 0 6px;
  box-sizing: border-box;
  border: 1px solid var(--vscode-editorWidget-border, rgba(128, 128, 128, 0.35));
  border-radius: 4px;
  background-color: var(--vscode-editor-background, #1e1e1e);
  color: var(--vscode-editor-foreground, #d4d4d4);
  font-family: var(--guaxi-editor-font-family, Consolas, monospace);
  font-size: 11px;
  overflow: hidden;
}

.gm-zone-label {
  flex: 0 0 auto;
  padding-right: 2px;
  color: var(--vscode-descriptionForeground, #9da7b3);
}

.gm-zone-button {
  flex: 0 0 auto;
  padding: 1px 7px;
  border: 1px solid var(--vscode-button-border, transparent);
  border-radius: 3px;
  background-color: var(--vscode-button-secondaryBackground, rgba(128, 128, 128, 0.18));
  color: var(--vscode-button-secondaryForeground, var(--vscode-editor-foreground, #333333));
  font: inherit;
  cursor: pointer;
  line-height: 16px;
}

.gm-zone-button:hover {
  background-color: var(--vscode-button-secondaryHoverBackground, rgba(128, 128, 128, 0.3));
}

.gm-zone-button.is-incoming {
  border-color: rgba(88, 166, 255, 0.6);
  background-color: rgba(88, 166, 255, 0.18);
  color: var(--vscode-editor-foreground, #333333);
}

.gm-zone-button.is-current {
  border-color: rgba(63, 185, 80, 0.6);
  background-color: rgba(63, 185, 80, 0.18);
  color: var(--vscode-editor-foreground, #333333);
}
</style>
