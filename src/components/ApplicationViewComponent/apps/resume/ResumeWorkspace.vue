<template>
  <div class="rs" :data-theme="theme">
    <header class="rs-top">
      <button
        type="button"
        class="rs-btn rs-btn--ghost rs-btn--icon"
        :title="railOpen ? '收起简历库' : '展开简历库'"
        :aria-label="railOpen ? '收起简历库' : '展开简历库'"
        @click="toggleRail"
      >
        <Icon :icon="railOpen ? panelLeftCloseIcon : panelLeftOpenIcon" width="15" height="15" />
      </button>

      <div class="rs-brand">
        <span class="rs-brand-mark"><Icon :icon="penIcon" width="14" height="14" /></span>
        <span class="rs-brand-text">
          <strong>简历工坊</strong>
          <span>Paper Atelier</span>
        </span>
      </div>

      <div class="rs-title-block">
        <input
          class="rs-title-input"
          :value="store.doc?.title || ''"
          :disabled="!store.doc"
          placeholder="未命名简历"
          spellcheck="false"
          @input="onTitleInput"
        />
        <div class="rs-meta">
          <span class="rs-dot" :data-state="store.saveState" />
          <span class="rs-save-state">{{ saveLabel }}</span>
          <span v-if="store.lastSavedAt">· {{ formatTime(store.lastSavedAt) }}</span>
        </div>
      </div>

      <div class="rs-actions">
        <button type="button" class="rs-chip" :disabled="!store.doc" title="更换模板" @click="templateOpen = true">
          <span class="rs-chip-dot" :style="{ background: templateAccent }" />
          <span class="rs-chip-name">{{ store.activeTemplate.name }}</span>
          <Icon :icon="chevronDownIcon" width="12" height="12" />
        </button>

        <button type="button" class="rs-btn" :disabled="!store.doc" @click="versionOpen = true">
          <Icon :icon="clockIcon" width="14" height="14" />
          版本
          <span class="rs-count">{{ store.versions.length }}</span>
        </button>

        <button
          type="button"
          class="rs-btn rs-btn--icon"
          :class="{ 'is-on': previewOpen }"
          :title="previewOpen ? '隐藏预览' : '显示预览'"
          :aria-label="previewOpen ? '隐藏预览' : '显示预览'"
          @click="togglePreview"
        >
          <Icon :icon="previewOpen ? panelRightCloseIcon : panelRightOpenIcon" width="15" height="15" />
        </button>

        <div class="rs-menu-wrap rs-export">
          <button
            type="button"
            class="rs-btn rs-btn--primary"
            :disabled="!store.doc"
            @click.stop="exportOpen = !exportOpen"
          >
            <Icon :icon="downloadIcon" width="14" height="14" />
            导出
            <Icon :icon="chevronDownIcon" width="11" height="11" />
          </button>
          <div v-if="exportOpen" class="rs-menu" @click.stop>
            <button type="button" class="rs-menu-item" @click="run(io.exportHtml)">
              <Icon :icon="fileTextIcon" width="14" height="14" /> 导出 HTML
            </button>
            <button type="button" class="rs-menu-item" @click="run(io.exportPdf)">
              <Icon :icon="printerIcon" width="14" height="14" /> 打印 / 另存为 PDF
              <span class="rs-menu-hint">Ctrl+P</span>
            </button>
            <div class="rs-menu-sep" />
            <button type="button" class="rs-menu-item" @click="run(io.exportBundle)">
              <Icon :icon="packageIcon" width="14" height="14" /> 导出项目文件
              <span class="rs-menu-hint">含版本</span>
            </button>
            <button type="button" class="rs-menu-item" @click="run(io.importBundleFile)">
              <Icon :icon="uploadIcon" width="14" height="14" /> 导入项目文件
            </button>
          </div>
        </div>
      </div>
    </header>

    <div v-if="store.bootError" class="rs-boot-error">
      <strong>简历库初始化失败</strong>
      <span>{{ store.bootError }}</span>
      <button type="button" class="rs-btn rs-btn--sm" @click="store.boot()">重试</button>
    </div>

    <div
      class="rs-body"
      :class="{ 'no-rail': !railOpen, 'no-preview': !previewOpen, 'is-resizing': resizing }"
      :style="bodyStyle"
    >
      <ResumeLibrary
        v-show="railOpen"
        @import="io.importBundleFile"
        @resize="onRailResize"
        @resize-end="persistRailWidth"
      />

      <ResumeEditor :theme="theme" />

      <div
        v-if="previewOpen"
        class="rs-split"
        title="拖动调整预览宽度，双击恢复默认"
        @mousedown="startResize"
        @dblclick="resetPreviewWidth"
      />

      <ResumePreview v-if="previewOpen" />
    </div>

    <ResumePanels
      :template-open="templateOpen"
      :version-open="versionOpen"
      @close-templates="templateOpen = false"
      @close-versions="versionOpen = false"
    />
    <ConfirmHost />

    <div v-if="store.booting" class="rs-booting">
      <span class="rs-spinner" />
      <span>正在打开简历库…</span>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Icon } from '@iconify/vue'
import './resume.css'
import penIcon from '@iconify-icons/lucide/pen-line'
import panelLeftCloseIcon from '@iconify-icons/lucide/panel-left-close'
import panelLeftOpenIcon from '@iconify-icons/lucide/panel-left-open'
import panelRightCloseIcon from '@iconify-icons/lucide/panel-right-close'
import panelRightOpenIcon from '@iconify-icons/lucide/panel-right-open'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import clockIcon from '@iconify-icons/lucide/clock-3'
import downloadIcon from '@iconify-icons/lucide/download'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import printerIcon from '@iconify-icons/lucide/printer'
import packageIcon from '@iconify-icons/lucide/package'
import uploadIcon from '@iconify-icons/lucide/upload'
import { useResumeStore, formatTime } from './store/resumeStore'
import { useResumeExport } from './useResumeExport'
import { useSettingsStore } from '../../../../stores/settingsStore'
import ResumeLibrary from './components/ResumeLibrary.vue'
import ResumeEditor from './components/ResumeEditor.vue'
import ResumePreview from './components/ResumePreview.vue'
import ResumePanels from './components/ResumePanels.vue'
import ConfirmHost from './components/ConfirmHost.vue'

const store = useResumeStore()
const settingsStore = useSettingsStore()
const io = useResumeExport(store)

const theme = computed(() => (settingsStore.appearanceTheme === 'dark' ? 'dark' : 'light'))
const templateAccent = computed(() => store.activeTemplate.accent || '#8a6d4b')

const railOpen = ref(localStorage.getItem('guaxi.resume.railOpen') !== '0')
const previewOpen = ref(localStorage.getItem('guaxi.resume.previewOpen') !== '0')
const storedRailWidth = Number(localStorage.getItem('guaxi.resume.railWidth'))
const railWidth = ref(storedRailWidth >= 210 && storedRailWidth <= 520 ? storedRailWidth : 266)
const previewWidth = ref(Number(localStorage.getItem('guaxi.resume.previewWidth')) || 560)
const templateOpen = ref(false)
const versionOpen = ref(false)
const exportOpen = ref(false)
const resizing = ref(false)

const saveLabel = computed(() => {
  switch (store.saveState) {
    case 'dirty': return '正在编辑'
    case 'saving': return '保存中'
    case 'saved': return '已保存'
    case 'error': return '保存失败'
    default: return '就绪'
  }
})

const bodyStyle = computed(() => {
  const rail = railOpen.value ? `${railWidth.value}px` : '0px'
  if (!previewOpen.value) {
    return { gridTemplateColumns: `${rail} minmax(0, 1fr)` }
  }
  return { gridTemplateColumns: `${rail} minmax(360px, 1fr) 5px ${previewWidth.value}px` }
})

function onRailResize(width) {
  railWidth.value = width
}

function persistRailWidth() {
  localStorage.setItem('guaxi.resume.railWidth', String(Math.round(railWidth.value)))
}

function toggleRail() {
  railOpen.value = !railOpen.value
  localStorage.setItem('guaxi.resume.railOpen', railOpen.value ? '1' : '0')
}

function togglePreview() {
  previewOpen.value = !previewOpen.value
  localStorage.setItem('guaxi.resume.previewOpen', previewOpen.value ? '1' : '0')
}

let resizeStartX = 0
let resizeStartWidth = 0
function startResize(event) {
  event.preventDefault()
  resizing.value = true
  resizeStartX = event.clientX
  resizeStartWidth = previewWidth.value
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  document.addEventListener('mousemove', onResizeMove)
  document.addEventListener('mouseup', stopResize)
}

function onResizeMove(event) {
  if (!resizing.value) return
  const next = resizeStartWidth - (event.clientX - resizeStartX)
  previewWidth.value = Math.max(320, Math.min(next, Math.round(window.innerWidth * 0.68)))
}

function stopResize() {
  if (!resizing.value) return
  resizing.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
  localStorage.setItem('guaxi.resume.previewWidth', String(previewWidth.value))
}

function resetPreviewWidth() {
  previewWidth.value = 560
  localStorage.setItem('guaxi.resume.previewWidth', '560')
}

function onTitleInput(event) {
  store.setTitle(event.target.value)
}

async function run(action) {
  exportOpen.value = false
  await action()
}

function isTypingTarget(target) {
  if (!target) return false
  const tag = target.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable || Boolean(target.closest?.('.monaco-editor'))
}

function onKeydown(event) {
  const meta = event.ctrlKey || event.metaKey
  if (!meta) {
    if (event.key === 'Escape') {
      templateOpen.value = false
      versionOpen.value = false
      exportOpen.value = false
    }
    return
  }
  const key = String(event.key || '').toLowerCase()
  if (key === 's') {
    event.preventDefault()
    event.stopPropagation()
    store.saveNow({ reason: 'manual', silent: false })
    return
  }
  if (key === 'p') {
    event.preventDefault()
    event.stopPropagation()
    io.exportPdf()
    return
  }
  if (key === 'z' && !isTypingTarget(event.target)) {
    event.preventDefault()
    if (event.shiftKey) store.redo()
    else store.undo()
    return
  }
  if (key === 'y' && !isTypingTarget(event.target)) {
    event.preventDefault()
    store.redo()
  }
}

function closeMenus(event) {
  if (!event.target.closest('.rs-export')) exportOpen.value = false
}

function flushSave() {
  if (store.saveState === 'dirty' || store.saveState === 'saving') {
    store.saveNow({ silent: true }).catch(() => {})
  }
}

function onVisibilityChange() {
  if (document.visibilityState === 'hidden') flushSave()
}

onMounted(() => {
  store.boot()
  window.addEventListener('keydown', onKeydown, true)
  document.addEventListener('click', closeMenus)
  document.addEventListener('visibilitychange', onVisibilityChange)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown, true)
  document.removeEventListener('click', closeMenus)
  document.removeEventListener('visibilitychange', onVisibilityChange)
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
  flushSave()
})
</script>

<style scoped>
.rs-count {
  font-family: var(--rs-label);
  font-size: 9.5px;
  letter-spacing: .05em;
  color: var(--rs-muted);
  border: 1px solid var(--rs-line);
  border-radius: 999px;
  padding: 0 5px;
  line-height: 14px;
  min-width: 18px;
  text-align: center;
}
.rs-btn.is-on .rs-count { color: inherit; border-color: currentColor; }

.rs-boot-error {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--rs-line);
  background: var(--rs-accent-soft);
  color: var(--rs-accent);
  font-size: 12px;
}
.rs-boot-error strong { font-weight: 700; }
.rs-boot-error span { flex: 1; color: var(--rs-ink-soft); }

.rs-booting {
  position: absolute;
  inset: 0;
  z-index: 300;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background: color-mix(in srgb, var(--rs-canvas) 88%, transparent);
  color: var(--rs-muted);
  font-size: 12.5px;
  letter-spacing: .04em;
  backdrop-filter: blur(2px);
}
</style>
