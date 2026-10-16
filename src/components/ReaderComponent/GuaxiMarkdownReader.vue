<template>
  <div class="markdown-viewer-container">
    <!-- 顶部控制栏 -->
    <div class="markdown-controls guaxi-reader-controls" :style="{ '--pdf-progress': `${progressPercent}%` }">
      <div class="page-info">
        <span>第 {{ currentPage }} 页 / 共 {{ totalPages }} 页（{{ progressPercent }}%）</span>
        <span v-if="completedReadingMs">已完成：{{ formatDuration(completedReadingMs) }}</span>
      </div>

      <div class="page-navigation">
        <button @click="prevPage" :disabled="currentPage <= 1" title="上一页">
          <n-icon :component="ArrowBackOutline" />
        </button>

        <input type="number" v-model.number="jumpPage" min="1" :max="totalPages" @keyup.enter="goToPage">

        <button @click="nextPage" :disabled="currentPage >= totalPages" title="下一页">
          <n-icon :component="ArrowForwardOutline" />
        </button>
      </div>

      <div class="right-controls">
        <button class="fit-width" @click="fitToWidth" title="将页面调整为适合宽度">
          <n-icon :component="ExpandOutline" />
        </button>

        <span class="ctl-sep" />

        <div class="ctl-group ctl-group--zoom">
          <button class="zoom-out" @click="zoomOut" :disabled="zoomLevel <= 0.5" title="缩小">
            <n-icon :component="RemoveOutline" />
          </button>
          <span class="zoom-level">{{ Math.round(zoomLevel * 100) }}%</span>
          <button class="zoom-in" @click="zoomIn" :disabled="zoomLevel >= 2" title="放大">
            <n-icon :component="AddOutline" />
          </button>
        </div>

        <span class="ctl-sep" />

        <div class="ctl-group">
          <n-dropdown trigger="click" :options="viewModeOptions" @select="handleViewModeSelect">
            <n-button class="layout-switch" text title="切换布局">
              <template #icon>
                <n-icon :component="OptionsOutline" />
              </template>
            </n-button>
          </n-dropdown>

          <n-button class="layout-switch" text
            :title="isBookmarkedCurrent ? `取消书签（第 ${currentPage} 页）` : `添加书签（第 ${currentPage} 页）`"
            @click="toggleBookmarkCurrentPage">
            <template #icon>
              <n-icon :component="BookmarksOutline" />
            </template>
          </n-button>

          <n-dropdown trigger="click" :options="bookmarkOptions" @select="handleBookmarkSelect">
            <n-button class="layout-switch" text title="书签列表">
              <template #icon>
                <n-icon :component="LibraryOutline" />
              </template>
            </n-button>
          </n-dropdown>
        </div>

        <span class="ctl-sep" />

        <div class="ctl-group">
          <!-- 显示/隐藏笔记按钮 -->
          <button class="toggle-notes" :class="{ active: showNotes }" @click="toggleNotesPanel"
            :title="showNotes ? '隐藏笔记' : '显示笔记'">
            <n-icon :component="DocumentTextOutline" />
          </button>
          <button class="toggle-assistant" :class="{ active: showAssistant }" @click="toggleAssistantPanel"
            :title="showAssistant ? '隐藏助手' : '显示助手'">
            <n-icon :component="ChatbubbleEllipsesOutline" />
          </button>

          <button class="toggle-translate" @click="toggleTranslate" :title="showTranslation ? '隐藏译文' : '显示译文'"
            :class="{ 'translating': isTranslating }">
            <n-icon :component="LanguageOutline" />
          </button>

          <!-- 逐句朗读：点开即从断点连续朗读，当前句高亮并跟随滚动 -->
          <button class="toggle-readaloud" :class="{ 'reading-aloud': readAloudPlaying }" @click="toggleReadAloud"
            :title="readAloudTitle">
            <n-icon :component="readAloudPlaying ? PauseOutline : PlayOutline" />
          </button>
        </div>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="markdown-main-content">
      <div class="markdown-left-wrapper" v-show="viewMode !== 'full-right' || showOutline" :style="{ width: leftPanelWidth }">
        <div v-if="showOutline" class="outline-panel-wrapper outline-panel-wrapper-left" @click.stop
          :style="{ flexBasis: `${outlineWidth}px` }">
          <GuaxiOutline />
        </div>
        <!-- 大纲列宽拖拽把手：原来那条"分隔线"只是 border-right，没有任何拖拽处理 -->
        <div v-if="showOutline" class="outline-resize-handle" @mousedown.prevent="startOutlineResize"></div>

        <!-- Markdown渲染区域 -->
        <div class="markdown-source-panel" ref="sourcePanel" v-show="viewMode !== 'full-right'" @mouseup="handleSelectionMouseUp">
          <div class="markdown-source-container">
            <!-- Markdown原文内容 -->
            <div v-if="markdownContent" class="markdown-source-content" ref="sourceContent">
            <div v-for="(srcLine, idx) in markdownSourceLines" :key="idx" class="markdown-source-line" :data-line="idx">{{ srcLine }}</div>
          </div>
            <!-- 加载状态 -->
            <div v-else class="markdown-loading-message">
              正在加载Markdown文件...
            </div>
          </div>
        </div>
      </div>

      <!-- 可拖动的中间线 -->
      <div class="resize-handle" v-show="viewMode === 'split'" @mousedown="startResize"></div>

      <!-- 右侧Markdown内容 -->
      <div class="markdown-panel guaxi-reader-scrollbar" ref="markdownPanel" v-show="viewMode !== 'full-left'" @click="handleMarkdownClickAlt" @dblclick="handleMarkdownClick"
        @mousemove="handleMarkdownHover" @mouseleave="handleMarkdownLeave" @mouseenter="setScrollTargetFromEvent"
        @mouseup="handleSelectionMouseUp">
        <div class="markdown-content" ref="markdownContainer">
          <template v-for="(line, index) in markdownLines" :key="index">
            <div class="markdown-line">
              <div class="original-content" v-html="line.html"></div>
              <div v-if="showTranslation" class="translated-content" :class="{
                'skipped-translation': line.skipTranslation,
                'error-translation': line.translationError
              }">
                <template v-if="line.skipTranslation">
                  <div v-html="line.html"></div>
                  <span class="translation-skipped-marker">(内容未翻译)</span>
                </template>
                <template v-if="line.translationError">
                  <div class="translation-error-message">
                    {{ line.translationError }}
                  </div>
                  <div v-html="line.html" class="original-fallback"></div>
                </template>
                <template v-else>
                  <div v-html="line.translation"></div>
                </template>
              </div>
            </div>
          </template>
        </div>

        <div v-if="inPageSearchVisible" class="inpage-search-bar" @mousedown.stop @click.stop @dblclick.stop>
          <input ref="inPageSearchInput" class="inpage-search-input" v-model="inPageSearchQuery" placeholder="页内搜索"
            @input="applyInPageSearch" @keydown.enter.prevent="gotoNextSearchMatch"
            @keydown.shift.enter.prevent="gotoPrevSearchMatch" @keydown.esc.prevent="closeInPageSearch" />
          <span class="inpage-search-count">{{ inPageSearchCountText }}</span>
          <button class="inpage-search-btn" type="button" title="上一个" @click="gotoPrevSearchMatch">
            <n-icon :component="ArrowBackOutline" />
          </button>
          <button class="inpage-search-btn" type="button" title="下一个" @click="gotoNextSearchMatch">
            <n-icon :component="ArrowForwardOutline" />
          </button>
          <button class="inpage-search-btn" type="button" title="关闭" @click="closeInPageSearch">
            <n-icon :component="CloseOutline" />
          </button>
        </div>

        <div v-if="selectionToolbarVisible" ref="selectionToolbarEl" class="selection-toolbar"
          :style="{ top: `${selectionToolbarTop}px`, left: `${selectionToolbarLeft}px` }" @mousedown.stop @click.stop @dblclick.stop>
          <div class="selection-toolbar-actions">
            <button class="selection-tool-btn" type="button" title="翻译" :disabled="selectionBusy || !selectionText"
              @click="translateSelection">
              <n-icon :component="LanguageOutline" />
            </button>
            <button class="selection-tool-btn" type="button" title="查询" :disabled="selectionBusy || !selectionText"
              @click="querySelection">
              <n-icon :component="SearchOutline" />
            </button>
            <button class="selection-tool-btn" type="button" title="问AI" :disabled="selectionBusy || !selectionText"
              @click="askSelectionWithAssistant">
              <n-icon :component="ChatbubbleEllipsesOutline" />
            </button>
            <button class="selection-tool-btn" type="button" title="用AI翻译" :disabled="selectionBusy || !selectionText"
              @click="openAssistantTranslateDialog">
              <n-icon :component="ColorWandOutline" />
            </button>
            <button class="selection-tool-btn" type="button" title="插入笔记" :disabled="selectionBusy || !selectionInsertText"
              @click="insertSelectionResult">
              <n-icon :component="AddOutline" />
            </button>
            <!-- 朗读所选文本：播放中再点即停 -->
            <button class="selection-tool-btn" type="button"
              :title="selectionReadAloudBusy ? '停止朗读' : '朗读所选文本'"
              :disabled="(!selectionText || selectionBusy) && !selectionReadAloudBusy"
              @click="toggleSelectionReadAloud(normalizeSelectionTextForLanguageTasks(selectionText))">
              <n-icon :component="selectionReadAloudBusy ? StopCircleOutline : VolumeHighOutline" />
            </button>
          </div>
        </div>

        <div v-if="selectionTranslatePanelVisible" class="selection-translate-layer">
          <div class="selection-translate-layer-inner">
            <GuaxiSelectionTranslatePanel :pos="selectionTranslatePanelPos" :raw="selectionTranslateRaw"
              :result="selectionTranslateResult" :error="selectionTranslateError" :busy="selectionTranslateBusy"
              :pinned="selectionTranslatePinned" :items="selectionTranslateItems" :from-lang="selectionTranslateFrom"
              :to-lang="selectionTranslateTo" :lang-options="selectionTranslateLangOptions" :word-meta="selectionTranslateWordMeta"
              @update:from-lang="(v) => (selectionTranslateFrom = String(v || 'auto'))"
              @update:to-lang="(v) => (selectionTranslateTo = String(v || 'zh'))" @swap-langs="swapSelectionTranslateLangs"
              @translate="translateSelectionText" @copy="copySelectionTranslateResult" @toggle-pin="toggleSelectionTranslatePinned"
              @update:pos="handleSelectionTranslatePanelUpdatePos" @close="closeSelectionTranslatePanel" />
          </div>
        </div>

        <n-modal v-model:show="assistantTranslateDialogVisible" preset="card" title="用AI翻译" style="width: 420px">
          <div style="display: flex; flex-direction: column; gap: 12px;">
            <n-text depth="3">请选择目标语言后发送到智能助手翻译。</n-text>
            <n-select v-model:value="assistantTranslateTargetLang" size="small" :options="assistantTranslateLangOptions"
              placeholder="请选择目标语言" />
            <div style="display: flex; justify-content: flex-end; gap: 8px;">
              <n-button size="small" @click="assistantTranslateDialogVisible = false">取消</n-button>
              <n-button size="small" type="primary" @click="sendSelectionAsAiTranslate">发送并翻译</n-button>
            </div>
          </div>
        </n-modal>

      </div>
    </div>

    <div class="assistant-panel" :class="{ 'assistant-panel-visible': showAssistant }">
      <div class="assistant-header">
        <h3>智能助手</h3>
        <button @click="toggleAssistantPanel" class="close-btn">
          <n-icon :component="CloseOutline" size="18" />
        </button>
      </div>
      <div class="assistant-content">
        <GuaxiAgentAssistant />
      </div>
    </div>

    <!-- 笔记面板 -->
    <div class="notes-panel" :class="{ 'notes-panel-visible': showNotes }">
      <div class="notes-header">
        <h3>我的笔记</h3>
        <button @click="toggleNotesPanel" class="close-btn">
          <n-icon :component="CloseOutline" size="18" />
        </button>
      </div>
      <div class="notes-content guaxi-reader-scrollbar" @mouseenter="setScrollTargetFromEvent">
        <n-card v-for="(card, index) in noteCards" :key="card.id" class="note-card" size="small" :title="card.title"
          :bordered="true" :segmented="{
            content: true,
            footer: 'soft'
          }" header-style="padding: 12px 16px;" content-style="padding: 0;">
          <template #header-extra>
            <div class="card-actions" v-if="activeCardIndex === index">
              <n-button v-if="card.type !== 'composite'" size="tiny" type="primary" @click.stop="saveCardEdit(index)" title="保存">
                <n-icon :component="SaveOutline" size="12" />
              </n-button>
              <n-button v-if="card.type !== 'composite'" size="tiny" text @click.stop="exitEditMode(index)" title="退出编辑">
                <n-icon :component="ExitOutline" size="12" />
              </n-button>
              <n-button size="tiny" text @click.stop="deleteCard(index)" title="删除">
                <n-icon :component="TrashOutline" size="12" />
              </n-button>
            </div>
          </template>

          <!-- 根据笔记类型使用不同的渲染方式 -->
          <div v-if="card.type !== 'composite' && activeCardIndex !== index" class="note-content"
            :contenteditable="false" v-html="noteHtmls[index]" @click="enterEditMode(index)"></div>

          <div v-else-if="card.type !== 'composite' && activeCardIndex === index" class="note-content">
            <textarea class="composite-note-editor" @input="markEditorDirty(index)" @keydown.meta.s.prevent="saveCardEdit(index)"
              @keydown.ctrl.s.prevent="saveCardEdit(index)" :ref="el => setContentEditableRef(index, el)"></textarea>
          </div>

          <!-- 添加复合笔记的专用渲染 -->
          <div v-else class="composite-note-content" @click="enterEditMode(index)">
            <div v-for="(segment, segIndex) in compositeSegments[index]" :key="segIndex" class="note-segment"
              :class="getSegmentClass(segment)">
              <div v-html="segment.content"></div>
            </div>
          </div>

          <template #footer>
            <n-text depth="3" style="font-size: 12px;">
              {{ formatTime(card.timestamp) }}
            </n-text>
          </template>
        </n-card>

        <div class="add-card-container">
          <n-button type="primary" dashed block @click="addNewCard">
            <template #icon>
              <n-icon :component="AddOutline" />
            </template>
            添加新卡片
          </n-button>
        </div>
      </div>
    </div>

    <!-- 添加笔记的提示气泡 -->
    <transition name="fade">
      <div v-if="showNoteBubble" class="note-added-bubble">
        <div class="bubble-content">
          <span>已添加到笔记</span>
          <i class="icon-check"></i>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, watchEffect, onMounted, onBeforeUnmount, watch, nextTick, h } from 'vue'
import { markReaderScrollbarActive } from './readerScrollbar'
import { invoke } from '@tauri-apps/api/core'
import { join, dirname } from '@tauri-apps/api/path'
import { convertFileSrc } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';
import { openUrl } from '@tauri-apps/plugin-opener';
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'
import { renderMarkdown, addCopyButtons, addMediaViewerButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { maskPythonCellBlocksForMarkdown } from '../HomeViewComponent/utils/guaxiPythonCellSyntax'
import { NCard, NButton, NIcon, NText, NModal, NSelect, useMessage, NDropdown } from 'naive-ui'
import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import GuaxiOutline from '../HomeViewComponent/GuaxiOutline.vue'
import GuaxiAgentAssistant from '../HomeViewComponent/GuaxiAgentAssistant.vue'
import GuaxiSelectionTranslatePanel from '../HomeViewComponent/GuaxiSelectionTranslatePanel.vue'
import {
  ArrowBackOutline,
  ArrowForwardOutline,
  RefreshCircleOutline,
  ExpandOutline,
  TrashOutline,
  AddOutline,
  RemoveOutline,
  DocumentTextOutline,
  DownloadOutline,
  CloseOutline,
  LanguageOutline,
  SaveOutline,
  ExitOutline,
  OptionsOutline,
  BookmarksOutline,
  BookmarkOutline,
  LibraryOutline,
  SearchOutline,
  ChatbubbleEllipsesOutline,
  ColorWandOutline,
  PauseOutline,
  PlayOutline,
  StopCircleOutline,
  VolumeHighOutline
} from '@vicons/ionicons5'
import { useReaderReadAloud } from '../../composables/useReaderReadAloud'
import { useSelectionReadAloud } from '../../composables/useSelectionReadAloud'

const logger = createLogger('GuaxiMarkdownReader')
// 组件属性定义
const props = defineProps({
  filePath: {
    type: String,
    required: true,
  },
  fileType: {
    type: String,
    required: true,
    validator: (value) => ['md', 'markdown'].includes(value)
  },
  fileName: {
    type: String,
    required: true,
  },
  library: {
    type: String,
    required: true
  },
  importFileId: {
    type: Number,
    default: null,
  }
})

// 初始化状态和工具
const message = useMessage()
const store = repoStore()
const { librariesRoot } = storeToRefs(store)
const outlineStore = useOutlineStore()
const rightSiderStore = useRightSiderStore()
const md = new MarkdownIt()

const resolveMarkdownTheme = () => {
  try {
    const root = document?.documentElement
    const isDark =
      !!root?.classList?.contains?.('dark') ||
      !!root?.classList?.contains?.('dark-mode')
    return isDark ? 'dark' : 'default'
  } catch {
    return 'default'
  }
}

const isEditableElement = (el) => {
  if (!el) return false
  const t = el.tagName ? el.tagName.toLowerCase() : ''
  if (t === 'input' || t === 'textarea' || t === 'select') return true
  return Boolean(el.isContentEditable)
}

const lastScrollTarget = ref(null)

const setScrollTargetFromEvent = (e) => {
  const t = e?.currentTarget
  if (t && t instanceof HTMLElement) {
    lastScrollTarget.value = t
  }
}

const isScrollableElement = (el) => {
  if (!el) return false
  const hasV = el.scrollHeight > el.clientHeight
  const hasH = el.scrollWidth > el.clientWidth
  return hasV || hasH
}

const handleAnyScroll = (e) => {
  const t = e.target
  if (t === document) {
    const se = document.scrollingElement
    if (se) {
      markReaderScrollbarActive(se)
      lastScrollTarget.value = se
    }
    return
  }
  if (t && t instanceof HTMLElement && isScrollableElement(t)) {
    if (lastScrollTarget.value !== t) {
      logger.debug('[ReaderScrollbar] active scroll target changed', {
        className: t.className,
        scrollHeight: t.scrollHeight,
        clientHeight: t.clientHeight
      })
    }
    markReaderScrollbarActive(t)
    lastScrollTarget.value = t
  }
}

const handleReaderShortcuts = (e) => {
  if (isEditableElement(e.target)) return

  if (e.key === 'ArrowUp' || e.key === 'ArrowDown' || e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
    const target = lastScrollTarget.value
    if (target && isScrollableElement(target)) {
      const step = e.shiftKey ? 160 : 40
      const top = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0
      const left = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0
      if (top !== 0 || left !== 0) {
        e.preventDefault()
        target.scrollBy({ top, left, behavior: 'auto' })
        return
      }
    }
  }

  const ctrl = e.ctrlKey || e.metaKey

  if (e.key === 'PageUp') {
    e.preventDefault()
    prevPage()
    return
  }

  if (e.key === 'PageDown') {
    e.preventDefault()
    nextPage()
    return
  }

  if (ctrl && e.key === '0') {
    e.preventDefault()
    fitToWidth()
    return
  }

  if (ctrl && (e.key === '=' || e.key === '+')) {
    e.preventDefault()
    zoomIn()
    return
  }

  if (ctrl && e.key === '-') {
    e.preventDefault()
    zoomOut()
    return
  }

  if (ctrl && (e.key === 'n' || e.key === 'N')) {
    e.preventDefault()
    toggleNotesPanel()
    return
  }

  if (ctrl && (e.key === 'f' || e.key === 'F')) {
    e.preventDefault()
    openInPageSearch()
    return
  }

  if (ctrl && (e.key === 't' || e.key === 'T')) {
    e.preventDefault()
    toggleTranslate()
    return
  }

  if (ctrl && e.key === '1') {
    e.preventDefault()
    handleViewModeSelect('split')
    return
  }

  if (ctrl && e.key === '2') {
    e.preventDefault()
    handleViewModeSelect('full-left')
    return
  }

  if (ctrl && e.key === '3') {
    e.preventDefault()
    handleViewModeSelect('full-right')
    return
  }

  if (ctrl && e.key === '4') {
    e.preventDefault()
    handleViewModeSelect('outline')
  }
}

onMounted(() => {
  window.addEventListener('scroll', handleAnyScroll, true)
  window.addEventListener('keydown', handleReaderShortcuts, true)
  window.addEventListener('mousedown', handleGlobalMouseDown, true)

  nextTick(() => {
    if (lastScrollTarget.value) return
    const defaultTarget =
      document.querySelector('.markdown-panel') ||
      document.querySelector('.notes-content') ||
      document.scrollingElement
    if (defaultTarget && defaultTarget instanceof HTMLElement) {
      lastScrollTarget.value = defaultTarget
    }
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('scroll', handleAnyScroll, true)
  window.removeEventListener('keydown', handleReaderShortcuts, true)
  window.removeEventListener('mousedown', handleGlobalMouseDown, true)
})

const markdownContainer = ref(null);
const sourceContent = ref(null);
const sourcePanel = ref(null);
const noteHtmls = ref([]);
const compositeSegments = ref([]);
let notesRenderToken = 0;

const selectionToolbarVisible = ref(false)
const selectionToolbarTop = ref(0)
const selectionToolbarLeft = ref(0)
const selectionToolbarEl = ref(null)
// 划词浮动条的外框尺寸，与 .selection-toolbar 的 max-content 对应（6 个 34px 按钮 + 5×6px 间距 + 12px 内边距 + 2px 边框）。
// 只用于首帧兜底：真实宽度在 nextTick 里按 offsetWidth 实测修正。
const SELECTION_TOOLBAR_WIDTH = 248
const SELECTION_TOOLBAR_HEIGHT = 42
const selectionText = ref('')
const selectionBusy = ref(false)
const selectionInsertText = ref('')
// 划词朗读：一小段文本走一次性合成，与逐句朗读共用引擎但**不同实例**，互不阻塞。
// 浮动条一关就停止（否则工具栏没了、声音还在响，用户没有停止入口）。
const { busy: selectionReadAloudBusy, toggle: toggleSelectionReadAloud } = useSelectionReadAloud({
  visible: selectionToolbarVisible,
  message,
})
const showAssistant = ref(false)

const selectionTranslatePanelVisible = ref(false)
const selectionTranslatePinned = ref(false)
const selectionTranslatePanelPos = ref({ left: 8, top: 8 })

const selectionTranslateFrom = ref('auto')
const selectionTranslateTo = ref('zh')

const selectionTranslateLangOptions = [
  { label: '自动检测', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
  { label: '日文', value: 'ja' },
  { label: '韩文', value: 'ko' },
  { label: '法文', value: 'fr' },
  { label: '德文', value: 'de' },
  { label: '西班牙语', value: 'es' },
  { label: '俄语', value: 'ru' },
  { label: '葡萄牙语', value: 'pt' },
  { label: '意大利语', value: 'it' },
  { label: '荷兰语', value: 'nl' },
  { label: '越南语', value: 'vi' },
  { label: '泰语', value: 'th' }
]
const assistantTranslateDialogVisible = ref(false)
const assistantTranslateTargetLang = ref('en')
const assistantTranslateSourceText = ref('')
const assistantTranslateLangOptions = selectionTranslateLangOptions.filter((x) => x.value !== 'auto')

const selectionTranslateBusy = ref(false)
const selectionTranslateRaw = ref('')
const selectionTranslateResult = ref('')
const selectionTranslateError = ref('')
const selectionTranslateItems = ref([])
const selectionTranslateWordMeta = ref(null)

const selectionTranslateMultiAvailable = ref(true)
let selectionTranslateMultiNotFoundWarned = false

let selectionTranslateScheduleTimer = null
const scheduleSelectionTranslate = () => {
  if (!selectionTranslatePanelVisible.value) return
  if (selectionTranslateBusy.value) return

  const raw = String(selectionTranslateRaw.value || '').trim()
  if (!raw) return

  clearTimeout(selectionTranslateScheduleTimer)
  selectionTranslateScheduleTimer = setTimeout(() => {
    translateSelectionText(raw)
  }, 250)
}

watch([selectionTranslateFrom, selectionTranslateTo], ([from, to], [prevFrom, prevTo]) => {
  if (!selectionTranslatePanelVisible.value) return
  if (from === prevFrom && to === prevTo) return
  scheduleSelectionTranslate()
})

const selectionPreviewHtml = computed(() => {
  const v = selectionInsertText.value.trim()
  return v ? md.render(v) : ''
})

const isSelectionDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_selection') || '').trim().toLowerCase()
    return v === '1' || v === 'true' || v === 'yes' || v === 'on'
  } catch {
    return false
  }
}

const selDlog = (...args) => {
  if (!isSelectionDebug()) return
  logger.info('[Selection][MarkdownReader]', ...args)
}

const handleSelectionTranslatePanelUpdatePos = (pos) => {
  if (!pos) return
  const left = Number(pos.left)
  const top = Number(pos.top)
  if (!Number.isFinite(left) || !Number.isFinite(top)) return

  const cur = selectionTranslatePanelPos.value
  if (cur && Number(cur.left) === left && Number(cur.top) === top) return

  selectionTranslatePanelPos.value = { left, top }
}

const toggleSelectionTranslatePinned = () => {
  selectionTranslatePinned.value = !selectionTranslatePinned.value
  logger.info('[SelectionTranslate][MarkdownReader] pin:', { pinned: selectionTranslatePinned.value })
}

const swapSelectionTranslateLangs = () => {
  const tmp = selectionTranslateFrom.value
  selectionTranslateFrom.value = selectionTranslateTo.value
  selectionTranslateTo.value = tmp
  logger.info('[SelectionTranslate][MarkdownReader] swap langs', { from: selectionTranslateFrom.value, to: selectionTranslateTo.value })

  if (selectionTranslatePanelVisible.value && !selectionTranslateBusy.value) {
    translateSelectionText(selectionTranslateRaw.value)
  }
}

const closeSelectionTranslatePanel = () => {
  selectionTranslatePanelVisible.value = false
  selectionTranslatePinned.value = false
}

const copySelectionTranslateResult = async (payload) => {
  const t = typeof payload === 'string' ? payload : payload?.text
  const text = String(t || selectionTranslateResult.value || '')
  if (!text) return

  try {
    await navigator.clipboard.writeText(text)
    message.success('已复制译文', { duration: 1200 })
  } catch {
    message.warning('复制失败：无剪贴板权限')
  }
}

const inPageSearchVisible = ref(false)
const inPageSearchQuery = ref('')
const inPageSearchMatches = ref([])
const inPageSearchActiveIndex = ref(-1)
const inPageSearchInput = ref(null)

const inPageSearchCountText = computed(() => {
  const total = inPageSearchMatches.value.length
  if (!total) return '0/0'
  return `${Math.max(0, inPageSearchActiveIndex.value) + 1}/${total}`
})

// 视图状态
const zoomLevel = ref(1);
const markdownPanel = ref(null);
const isResizing = ref(false);
const startX = ref(0);
const startLeftWidth = ref(0);
const leftPanelWidth = ref('50%');
// 大纲列宽：可拖拽调整。原来写死 260px，且那条"分隔线"只是 border-right，没有任何拖拽处理。
const outlineWidth = ref(260);
const isOutlineResizing = ref(false);
let outlineStartX = 0;
let outlineStartWidth = 0;

const startOutlineResize = (e) => {
  isOutlineResizing.value = true;
  outlineStartX = e.clientX;
  outlineStartWidth = outlineWidth.value;
  document.addEventListener('mousemove', handleOutlineResizeMove);
  document.addEventListener('mouseup', stopOutlineResize);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

const handleOutlineResizeMove = (e) => {
  if (!isOutlineResizing.value) return;
  const container = document.querySelector('.markdown-main-content');
  // 上限同时受窗口宽度约束，避免把正文挤没
  const maxWidth = Math.min(560, (container?.offsetWidth || window.innerWidth) * 0.6);
  outlineWidth.value = Math.max(160, Math.min(outlineStartWidth + e.clientX - outlineStartX, maxWidth));
};

const stopOutlineResize = () => {
  isOutlineResizing.value = false;
  document.removeEventListener('mousemove', handleOutlineResizeMove);
  document.removeEventListener('mouseup', stopOutlineResize);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};
const currentPage = ref(1)
const totalPages = ref(1)
const jumpPage = ref(1)

const readerStorageKey = computed(() => {
  const baseId = props.importFileId != null && props.importFileId !== 0
    ? `id:${props.importFileId}`
    : `path:${props.filePath || props.fileName || ''}`;
  const safeId = String(baseId).replace(/\\/g, '/');
  return `guaxi_reader_state_v1:${props.library}:${props.fileType}:${safeId}`;
});

const pendingRestorePage = ref(null);
// 朗读断点（第几段）。与页码一起存进阅读状态，重开文档即从这一句继续。
const speechIndex = ref(-1);
const isHydrated = ref(false);

const bookmarks = ref([]);
const totalReadingMs = ref(0);
const completedAt = ref(null);
const completedReadingMs = ref(null);
const sessionStartedAt = ref(0);

const progressPercent = computed(() => {
  if (!totalPages.value) return 0;
  return Math.min(100, Math.max(0, Math.round((currentPage.value / totalPages.value) * 1000) / 10));
});

const formatDuration = (ms) => {
  const v = Number(ms);
  if (!Number.isFinite(v) || v <= 0) return '0s';
  const totalSeconds = Math.floor(v / 1000);
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
};

const loadReaderState = () => {
  try {
    const raw = localStorage.getItem(readerStorageKey.value);
    if (!raw) {
      return { lastPage: 1, bookmarks: [], totalReadingMs: 0, completedAt: null, completedReadingMs: null, speechIndex: -1 };
    }
    const parsed = JSON.parse(raw);
    const storedSpeech = Number(parsed.speechIndex);
    return {
      lastPage: Math.max(1, Number(parsed.lastPage) || 1),
      bookmarks: Array.isArray(parsed.bookmarks)
        ? parsed.bookmarks
          .map((n) => Number(n))
          .filter((n) => Number.isFinite(n) && n > 0)
        : [],
      totalReadingMs: Math.max(0, Number(parsed.totalReadingMs) || 0),
      completedAt: parsed.completedAt || null,
      completedReadingMs: parsed.completedReadingMs == null ? null : Math.max(0, Number(parsed.completedReadingMs) || 0),
      // 0 是合法段号，不能用 `||` 兜底（会被当成假值丢掉）。
      speechIndex: Number.isFinite(storedSpeech) ? Math.max(-1, Math.trunc(storedSpeech)) : -1,
    };
  } catch {
    return { lastPage: 1, bookmarks: [], totalReadingMs: 0, completedAt: null, completedReadingMs: null, speechIndex: -1 };
  }
};

const persistReaderState = () => {
  try {
    localStorage.setItem(
      readerStorageKey.value,
      JSON.stringify({
        v: 1,
        lastPage: currentPage.value,
        bookmarks: [...new Set(bookmarks.value)].sort((a, b) => a - b),
        totalReadingMs: totalReadingMs.value,
        completedAt: completedAt.value,
        completedReadingMs: completedReadingMs.value,
        speechIndex: speechIndex.value,
      })
    );
  } catch {
  }
};

const startReadingSession = () => {
  sessionStartedAt.value = Date.now();
};

const endReadingSession = () => {
  if (!sessionStartedAt.value) return;
  const elapsed = Math.max(0, Date.now() - sessionStartedAt.value);
  totalReadingMs.value += elapsed;
  sessionStartedAt.value = 0;
};

const markCompletedIfNeeded = () => {
  if (completedAt.value) return;
  if (!totalPages.value) return;
  if (currentPage.value < totalPages.value) return;
  completedAt.value = new Date().toISOString();
  const elapsed = sessionStartedAt.value ? Math.max(0, Date.now() - sessionStartedAt.value) : 0;
  completedReadingMs.value = totalReadingMs.value + elapsed;
};

const isBookmarkedCurrent = computed(() => bookmarks.value.includes(currentPage.value));

const toggleBookmarkCurrentPage = () => {
  const p = currentPage.value;
  if (!p) return;
  const i = bookmarks.value.indexOf(p);
  if (i >= 0) {
    bookmarks.value.splice(i, 1);
  } else {
    bookmarks.value.push(p);
  }
  persistReaderState();
};

const renderBookmarkOptionIcon = (iconComp) => () =>
  h(NIcon, null, { default: () => h(iconComp) });

const bookmarkOptions = computed(() => {
  const opts = [
    {
      label: isBookmarkedCurrent.value
        ? `取消书签（第 ${currentPage.value} 页）`
        : `添加书签（第 ${currentPage.value} 页）`,
      key: 'toggle-current',
      icon: renderBookmarkOptionIcon(BookmarksOutline)
    }
  ];

  const sorted = [...new Set(bookmarks.value)].sort((a, b) => a - b);
  if (sorted.length) {
    opts.push({
      label: '书签列表',
      key: 'bookmarks-title',
      disabled: true,
      icon: renderBookmarkOptionIcon(BookmarksOutline)
    });
    sorted.forEach((p) => {
      opts.push({
        label: `跳转到第 ${p} 页`,
        key: `page:${p}`,
        icon: renderBookmarkOptionIcon(BookmarkOutline)
      });
    });
    opts.push({ label: '清空书签', key: 'clear', icon: renderBookmarkOptionIcon(TrashOutline) });
  }

  return opts;
});

const handleBookmarkSelect = (key) => {
  if (key === 'toggle-current') return toggleBookmarkCurrentPage();
  if (key === 'clear') {
    bookmarks.value = [];
    persistReaderState();
    return;
  }
  if (typeof key === 'string' && key.startsWith('page:')) {
    const p = Number(key.slice('page:'.length));
    if (Number.isFinite(p)) goToPage(p);
  }
};

const calculateTotalPages = async () => {
  await nextTick();
  const panel = markdownPanel.value;
  if (!panel) {
    totalPages.value = 1;
    return;
  }
  const viewportHeight = panel.clientHeight || 1;
  const contentHeight = panel.scrollHeight || viewportHeight;
  totalPages.value = Math.max(1, Math.ceil(contentHeight / viewportHeight));
};

const elements = ref([])
const viewMode = ref('split')
const previousLeftWidth = ref(leftPanelWidth.value)
const showOutline = ref(false)

const viewModeOptions = [
  { label: '左右分栏', key: 'split' },
  { label: '仅左侧', key: 'full-left' },
  { label: '仅右侧', key: 'full-right' },
  { label: '大纲', key: 'outline' }
]

const handleViewModeSelect = (key) => {
  if (key === 'outline') {
    showOutline.value = !showOutline.value
  } else {
    setViewMode(key)
  }
}

const setViewMode = (mode) => {
  if (mode === viewMode.value) return;

  if (mode === 'split') {
    leftPanelWidth.value = previousLeftWidth.value || '50%';
  } else if (mode === 'full-left') {
    previousLeftWidth.value = leftPanelWidth.value;
    leftPanelWidth.value = '100%';
  } else if (mode === 'full-right') {
    if (viewMode.value === 'full-left') {
      leftPanelWidth.value = previousLeftWidth.value || '50%';
    }
  }

  viewMode.value = mode;
}

// Markdown相关
const markdownContent = ref('')
const markdownSourceLines = computed(() => {
  const s = markdownContent.value
  return s ? String(s).split('\n') : []
})
const outlineHeadingSourceLines = ref([])
const markdownHtml = ref('')
const markdownLines = ref([]);
const markdownFilePath = ref('')

// 笔记相关
const showNotes = ref(false)
const notesContent = ref('')
const showNoteBubble = ref(false)
const isFitting = false
const noteCards = ref([])
const activeCardIndex = ref(-1)
const isNotesReady = ref(false)
const notesFilePath = ref('')

// 拖拽和动画相关
const isDragging = ref(false)
const startDragPos = ref({ x: 0, y: 0 })
const startScrollPos = ref({ left: 0, top: 0 })
const velocity = ref({ x: 0, y: 0 })
const lastDragTime = ref(0)
const animationFrameId = ref(null)
const isDecelerating = ref(false)

// 翻译相关
const showTranslation = ref(false)
const isTranslating = ref(false)

// Alt键收集器状态
const altKeyCollector = ref({
  isActive: false,
  currentCard: null,
  timer: null
})

// 工具函数
async function processImages(content, basePath = '') {
  const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
  const replacements = [];

  for (const match of content.matchAll(imgRegex)) {
    const [full, alt, src] = match;
    if (!src.match(/^(http|\/|data:image)/i)) {
      try {
        const fullPath = await join(basePath, src);
        const safeSrc = await convertFileSrc(fullPath);
        replacements.push({ from: full, to: `![${alt}](${safeSrc})` });
      } catch (error) {
        logger.warn(`路径转换失败: ${src}`, error);
      }
    }
  }
  return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
}

// 渲染笔记内容为HTML
const renderNoteContent = async (markdown) => {
  if (!markdown) return '';
  try {
    const imageDir = await getImagesDirectory();
    const processedContent = await processImages(markdown, imageDir);
    return await renderMarkdown(processedContent, {
      preserveOriginal: false,
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      buttons: false,
      theme: resolveMarkdownTheme()
    });
  } catch (e) {
    return `<div class="render-error">${e.message}</div>`;
  }
};

async function convertMarkdownToLines(content, options = {}) {
  const processedContent = await processImages(content, options.libraryBasePath);
  try {
    const theme = resolveMarkdownTheme()
    let html = await renderMarkdown(processedContent, {
      preserveOriginal: true,
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme
    });

    nextTick(() => {
      if (markdownContainer.value) {
        addCopyButtons(markdownContainer.value, { theme });
        addMediaViewerButtons(markdownContainer.value, { theme });
      }
    });

    // 创建临时DOM元素来解析HTML
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = html;

    // 递归遍历DOM节点并拆分
    const splitByNodes = (node) => {
      const result = [];

      // 处理文本节点
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent.trim();
        if (text) {
          result.push({
            raw: text,
            html: text,
            skipTranslation: false
          });
        }
        return result;
      }

      // 处理元素节点
      if (node.nodeType === Node.ELEMENT_NODE) {
        // 特殊处理这些元素，保持其完整性
        const keepWholeTags = ['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
          'img', 'table', 'pre', 'code', 'div.katex-display',
          'span.katex-inline', 'div.mermaid', 'svg.mermaid',
          'mjx-container', 'mjx-math', 'mjx-assistive-mml',
          'div.hljs', 'code.hljs',
          '[class^="hljs-"]', '[class*=" hljs-"]'];

        if (keepWholeTags.some(tag => node.matches(tag))) {
          const outerHTML = node.outerHTML;
          const skipTranslation = shouldSkipTranslation(node);

          result.push({
            raw: outerHTML,
            html: outerHTML,
            skipTranslation
          });
          return result;
        }

        // 递归处理子节点
        for (const child of node.childNodes) {
          result.push(...splitByNodes(child));
        }
        return result;
      }

      return result;
    };

    // 从临时div的子节点开始处理
    const lines = [];
    for (const child of tempDiv.childNodes) {
      lines.push(...splitByNodes(child));
    }

    return lines;
  } catch (error) {
    logger.error('[ERROR] 渲染失败:', error);
    return [{
      raw: content,
      html: `<div class="render-error">${error.message}</div>`,
      skipTranslation: true
    }];
  }
}

function generateOutlineFromMarkdown(content) {
  if (!content) {
    outlineHeadingSourceLines.value = [];
    outlineStore.setOutline([]);
    return;
  }
  // Python Cell 使用 Markdown 注释哨兵存储，生成 Reader 大纲前先屏蔽源码区，避免 Python `#` 注释被当作标题。
  const outlineSource = maskPythonCellBlocksForMarkdown(content);
  const tokens = md.parse(outlineSource, {});
  const result = [];
  const headingLines = [];
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1));
      const nextToken = tokens[idx + 1];
      if (nextToken && nextToken.type === 'inline') {
        const srcLine = Array.isArray(token.map) ? Number(token.map[0]) : 0;
        headingLines.push(Number.isFinite(srcLine) ? srcLine : 0);
        result.push({
          level,
          text: nextToken.content,
          line: result.length
        });
      }
    }
  });
  outlineHeadingSourceLines.value = headingLines;
  outlineStore.setOutline(result);
}

function jumpToHeading(index) {
  const idx = Number(index);
  if (!Number.isFinite(idx)) return;

  if (markdownContainer.value) {
    const headings = markdownContainer.value.querySelectorAll('h1, h2, h3, h4, h5, h6');
    const target = headings[idx];
    if (target) {
      target.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }

  const srcLine = Array.isArray(outlineHeadingSourceLines.value)
    ? outlineHeadingSourceLines.value[idx]
    : null;
  const lineNum = Number(srcLine);
  if (!Number.isFinite(lineNum) || lineNum < 0) return;

  const root = sourceContent?.value;
  if (!(root instanceof HTMLElement)) return;
  const el = root.querySelector(`[data-line="${lineNum}"]`);
  if (el && el instanceof HTMLElement) {
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}

function shouldSkipTranslation(node) {
  if (!node || typeof node.matches !== 'function') {
    return false;
  }

  try {
    // 1. 图片元素
    if (node.tagName === 'IMG') {
      return true;
    }

    // 2. 数学公式相关元素
    if (node.classList && (
      node.classList.contains('katex-display') ||
      node.classList.contains('katex-inline') ||
      node.tagName === 'MJX-CONTAINER' ||
      node.tagName === 'MJX-MATH'
    )) {
      return true;
    }

    // 3. 图表相关元素
    if (node.classList && (
      node.classList.contains('mermaid') ||
      (node.tagName === 'SVG' && node.classList.contains('mermaid'))
    )) {
      return true;
    }

    // 4. 代码块
    if (['PRE', 'CODE'].includes(node.tagName) ||
      (node.classList && (
        node.classList.contains('hljs') ||
        Array.from(node.classList).some(cls => cls.startsWith('hljs-'))
      ))) {
      return true;
    }

    // 5. 表格
    if (node.tagName === 'TABLE') {
      return true;
    }

    return false;
  } catch (error) {
    logger.error('Error checking node:', error);
    return false;
  }
}

function determineNoteType(target) {
  // 1. 优先检查数学公式
  if (target?.closest?.('mjx-container, mjx-math, .math-inline, .math-block')) {
    return 'math';
  }

  // 2. 检查Mermaid图表
  const mermaidContainer = target?.closest?.('.mermaid, [data-processed="true"]');
  if (mermaidContainer) {
    if (mermaidContainer.querySelector('svg') || mermaidContainer.classList.contains('mermaid')) {
      return 'diagram';
    }
  }

  // 3. 检查代码块
  const pre = target?.closest?.('pre');
  if (pre) {
    if (!pre.classList.contains('mermaid')) {
      if (pre.querySelector('code') ||
        [...pre.classList].some(c => ['hovered', 'hljs', 'code'].includes(c))) {
        return 'code';
      }
    }
  }

  // 4. 检查图片
  const el = target instanceof Element ? target : target?.parentElement
  if (el && (el.tagName === 'IMG' || el.closest('img') || el.querySelector('img'))) {
    return 'image';
  }

  // 默认文本类型
  return 'text';
}

async function processNoteImage(imgElement) {
  try {
    const rawSrc = imgElement.src;
    let filePath = '';

    if (rawSrc.startsWith('http://asset.localhost/')) {
      filePath = decodeURIComponent(rawSrc.replace('http://asset.localhost/', ''));
    } else if (rawSrc.startsWith('file://')) {
      filePath = decodeURIComponent(new URL(rawSrc).pathname);
    } else {
      return rawSrc;
    }

    const osPlatform = await platform();
    if (osPlatform === 'win32') {
      filePath = filePath.replace(/^\/([a-zA-Z]:)/, '$1');
    }

    const imageDir = await getImagesDirectory();
    const ext = filePath.split('.').pop().toLowerCase();
    const validExts = ['jpg', 'jpeg', 'png', 'gif', 'webp'];
    const finalExt = validExts.includes(ext) ? ext : 'png';
    const fileName = `note-img-${Date.now()}.${finalExt}`;

    const relativePath = await invoke('copy_image_file', {
      srcPath: filePath,
      destDir: imageDir,
      fileName: fileName
    });

    return `../images/${relativePath}`;
  } catch (error) {
    logger.error('图片处理失败:', error);
    return imgElement.src;
  }
}

// ───────────────────────── 逐句朗读 ─────────────────────────
// 句子切分、合成、缓存全在后端（commands/readaloud.rs）；高亮、滚动跟随、点击定位的接线
// 收在 useReaderReadAloud 里，五个阅读器共用同一份实现。
const readAloud = useReaderReadAloud({
  // 正文与滚动容器都"用时现取"：mount 时内容可能还没渲染出来
  getContext: () => ({
    root: markdownContainer.value,
    scrollRoot: markdownPanel.value,
    win: window,
  }),
  getIdentity: () => ({ library: props.library || '', fileName: props.fileName || '' }),
  speechIndex,
  onIndexChange: () => persistReaderState(),
  message,
  ownerLabel: 'GuaxiMarkdownReader',
})

const readAloudPlaying = readAloud.isPlaying
const readAloudBuffering = readAloud.isBuffering
const readAloudTitle = readAloud.title
const toggleReadAloud = () => readAloud.toggle()
const seekReadAloudAtPoint = (event) => readAloud.seekAtPoint(event)

onBeforeUnmount(() => {
  readAloud.clearHighlight()
})

const handleMarkdownClickAlt = async (e) => {
  // 朗读进行中：点正文任意位置就从那一句开始读（"点哪读哪"，不用记快捷键）
  if ((readAloudPlaying.value || readAloudBuffering.value) && seekReadAloudAtPoint(e)) {
    return
  }
  if (!e?.altKey) return
  return handleMarkdownClick(e)
}

const handleMarkdownClick = async (e) => {
  e.preventDefault();

  const isAltPressed = e.altKey;
  const rawTarget = e.target;
  const hoveredElement = document.querySelector('.markdown-content .hovered');
  const target = hoveredElement && rawTarget instanceof Node && hoveredElement.contains(rawTarget)
    ? hoveredElement
    : rawTarget;
  const selection = window.getSelection();
  const selectedText = selection ? selection.toString().trim() : '';

  if (isAltPressed && !altKeyCollector.value.isActive) {
    startAltKeyCollection();
  }

  document.querySelectorAll('.markdown-content .selected').forEach(el => {
    el.classList.remove('selected');
  });

  let selectedElement = null;
  let message = '';
  let noteContent = '';
  let originalMarkdown = '';

  const translatedElement = target.closest('.translated-content');
  if (translatedElement) {
    const contentDiv = translatedElement.querySelector('div');
    if (contentDiv) {
      selectedElement = translatedElement;
      noteContent = contentDiv.innerHTML;
      message = '已选中译文';
      noteContent = contentDiv.textContent;
      originalMarkdown = noteContent;
    }
  }
  else if (target instanceof Element && (target.tagName === 'IMG' || target.closest('img') || target.querySelector('img'))) {
    const imgElement = target.tagName === 'IMG'
      ? target
      : (target.closest('img') || target.querySelector('img'));

    selectedElement = imgElement;
    try {
      const processedSrc = await processNoteImage(imgElement);
      message = '已选中图片';
      noteContent = `![图片](${processedSrc})`;
      originalMarkdown = noteContent;
    } catch (error) {
      logger.error('图片处理失败:', error);
      message = '图片处理失败，使用原始链接';
      noteContent = `![图片](${imgElement.src})`;
      originalMarkdown = noteContent;
    }
  }
  else if (target.closest('pre, code, .code-block')) {
    const codeBlock = target.closest('pre, code, .code-block');
    selectedElement = codeBlock;
    const clonedBlock = codeBlock.cloneNode(true);
    const btn = clonedBlock.querySelector('.copy-button');
    if (btn) btn.remove();
    const codeContent = clonedBlock.textContent || '';
    message = `已选中代码块: ${codeContent.substring(0, 30)}${codeContent.length > 30 ? '...' : ''}`;
    originalMarkdown = codeBlock.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`\n${codeContent}\n\`\`\``;
    originalMarkdown = decodeURIComponent(originalMarkdown);
    noteContent = originalMarkdown;
  }
  else if (target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered')) {
    const mathElement = target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered');
    selectedElement = mathElement;
    let original = mathElement.getAttribute('data-md-original');
    if (!original && mathElement.closest('[data-md-original]')) {
      original = mathElement.closest('[data-md-original]').getAttribute('data-md-original');
    }
    if (!original && mathElement.querySelector('[data-md-original]')) {
      original = mathElement.querySelector('[data-md-original]').getAttribute('data-md-original');
    }
    let decodedOriginal = original;
    try {
      if (decodedOriginal) {
        decodedOriginal = decodeURIComponent(decodedOriginal);
      } else {
        const isBlock = mathElement.classList.contains('math-block') ||
          mathElement.closest('.math-block') ||
          mathElement.tagName === 'MJX-BLOCK';
        const content = mathElement.textContent || '';
        decodedOriginal = isBlock ? `$$${content}$$` : `$${content}$`;
      }
    } catch (err) {
      decodedOriginal = original || '无法获取公式内容';
    }
    noteContent = decodedOriginal;
    originalMarkdown = decodedOriginal;
    const isBlockFormula = decodedOriginal.startsWith('$$') &&
      decodedOriginal.endsWith('$$') &&
      decodedOriginal.length > 4;
    message = `已选中${isBlockFormula ? '块级' : '行内'}公式: ${decodedOriginal.substring(0, 30)}${decodedOriginal.length > 30 ? '...' : ''}`;
  }
  else if (target.closest('.mermaid, svg')) {
    selectedElement = target.closest('.mermaid, svg');
    const mermaidContent = selectedElement.querySelector('.mermaid')?.textContent || selectedElement.textContent || '';
    message = '已选中图表';
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`mermaid\n${mermaidContent}\n\`\`\``;
    originalMarkdown = decodeURIComponent(originalMarkdown);
    noteContent = originalMarkdown;
  }
  else if (selectedText && selection.rangeCount > 0) {
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      selectedText;
    originalMarkdown = decodeURIComponent(originalMarkdown);
    noteContent = originalMarkdown;
  }
  else {
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      const textContent = paragraph.textContent || '';
      message = `已选中段落: ${textContent.substring(0, 30)}${textContent.length > 30 ? '...' : ''}`;
      originalMarkdown = paragraph.getAttribute('data-md-original') ||
        textContent;
      originalMarkdown = decodeURIComponent(originalMarkdown);
      noteContent = originalMarkdown;
    }
  }

  if (selectedElement) {
    const hasContent = (originalMarkdown || noteContent || '').trim();
    if (!hasContent) {
      showSelectionMessage('未选中有效内容');
      return;
    }

    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    const noteType = determineNoteType(selectedElement);
    const noteData = {
      type: noteType,
      content: noteContent,
      originalMarkdown: originalMarkdown
    };

    if (altKeyCollector.value.isActive) {
      appendToCurrentNote(noteData);
    } else {
      await addToNotes(noteData);
    }
  }

  if (!isAltPressed && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

const startAltKeyCollection = () => {
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  altKeyCollector.value.isActive = true;

  const newCard = {
    id: Date.now().toString(),
    type: 'composite',
    title: '复合笔记 (0项)',
    content: '',
    items: [],
    timestamp: new Date().toISOString()
  };

  noteCards.value.push(newCard);
  activeCardIndex.value = -1;
  altKeyCollector.value.currentCard = newCard;

  altKeyCollector.value.timer = setTimeout(() => {
    if (altKeyCollector.value.isActive) endAltKeyCollection();
  }, 30000);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
  });

  message.info('已进入多选模式, 松开Alt键结束收集', { duration: 3000 });
};

const endAltKeyCollection = () => {
  if (!altKeyCollector.value.isActive) return;

  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  const currentCard = altKeyCollector.value.currentCard;
  const isEmpty = !currentCard?.items?.length && !(currentCard?.content || currentCard?.originalMarkdown);
  if (isEmpty && currentCard?.id) {
    const idx = noteCards.value.findIndex(card => card.id === currentCard.id);
    if (idx !== -1) {
      noteCards.value.splice(idx, 1);
      if (activeCardIndex.value === idx) {
        activeCardIndex.value = -1;
      } else if (activeCardIndex.value > idx) {
        activeCardIndex.value--;
      }
    }
  }

  if (currentCard && (currentCard.content || currentCard.originalMarkdown)) {
    const existingIndex = noteCards.value.findIndex(card => card.id === currentCard.id);
    if (existingIndex === -1) {
      noteCards.value.push(currentCard);
    }
    activeCardIndex.value = -1;
    updateBackendNotes('add', currentCard);

    nextTick(() => {
      const cards = document.querySelectorAll('.n-card');
      if (cards.length > 0) {
        cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
      }
    });
  }

  altKeyCollector.value.isActive = false;
  altKeyCollector.value.currentCard = null;

  message.success('多选模式已结束', { duration: 2000 });
};

async function parseCompositeContent(items) {
  if (!items || !Array.isArray(items)) {
    return [];
  }

  try {
    const segments = await Promise.all(
      items.map(async (item, index) => {
        const content = item.original_markdown || item.originalMarkdown || item.content || '';
        const html = await renderNoteContent(content);

        return {
          id: item.id || `${Date.now()}-${index}`,
          type: item.type || 'text',
          content: html,
          originalMarkdown: content,
          timestamp: item.timestamp || new Date().toISOString()
        };
      })
    );

    return segments;
  } catch (error) {
    return [{
      type: 'error',
      content: `<div class="parse-error">解析失败: ${error.message}</div>`,
      originalMarkdown: '解析失败',
      timestamp: new Date().toISOString()
    }];
  }
}

const getSegmentClass = (segment) => {
  switch (segment.type) {
    case 'code': return 'code-segment';
    case 'image': return 'image-segment';
    case 'diagram': return 'diagram-segment';
    case 'math': return 'math-segment';
    default: return 'text-segment';
  }
};

const appendToCurrentNote = (element) => {
  if (!altKeyCollector.value.isActive || !altKeyCollector.value.currentCard) return;

  const currentCard = altKeyCollector.value.currentCard;

  if (!currentCard.items) {
    currentCard.items = [];
  }

  const newItem = {
    id: Date.now().toString(),
    type: element.type,
    content: element.content || element.originalMarkdown,
    originalMarkdown: element.originalMarkdown || element.content,
    original_markdown: element.originalMarkdown || element.content,
    timestamp: new Date().toISOString()
  };

  currentCard.items.push(newItem);
  currentCard.content = currentCard.items.map((item, index) => {
    return `${index + 1}. ${item.originalMarkdown || item.content}`;
  }).join('\n\n');

  currentCard.type = 'composite';
  currentCard.title = `复合笔记 (${currentCard.items.length}项)`;
  currentCard.timestamp = new Date().toISOString();

  noteCards.value = [...noteCards.value];

  message.success(`已添加第 ${currentCard.items.length} 条内容`, { duration: 1500 });
};

const showSelectionMessage = (message) => {
  const oldMessage = document.querySelector('.selection-message');
  if (oldMessage) oldMessage.remove();

  const messageEl = document.createElement('div');
  messageEl.className = 'selection-message';
  messageEl.textContent = message;
  document.body.appendChild(messageEl);

  setTimeout(() => {
    messageEl.classList.add('fade-out');
    setTimeout(() => messageEl.remove(), 500);
  }, 1500);
}

const closeSelectionToolbar = () => {
  selectionToolbarVisible.value = false
  selectionText.value = ''
  selectionInsertText.value = ''
}

const normalizeSelectionTextForLanguageTasks = (raw) => {
  const text = String(raw || '').replace(/\r\n?/g, '\n')
  if (!text.trim()) return ''
  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/([^\n。！？.!?;；:：])\n(?!\n)/g, '$1 ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

const isWordSelection = (text) => {
  const t = String(text || '').trim()
  if (!t) return false
  if (t.length > 48) return false
  if (/\s/.test(t)) return false
  return /^[\p{L}\p{N}'-]+$/u.test(t)
}

const insertIntoActiveNoteEditor = (text) => {
  const idx = activeCardIndex.value
  if (idx < 0) return false

  const el = contentEditables.value[idx]
  if (!el) return false

  const payload = `\n\n${text}\n\n`

  if (el instanceof HTMLTextAreaElement) {
    el.focus()
    const start = typeof el.selectionStart === 'number' ? el.selectionStart : el.value.length
    const end = typeof el.selectionEnd === 'number' ? el.selectionEnd : el.value.length
    el.value = el.value.slice(0, start) + payload + el.value.slice(end)
    const cursor = start + payload.length
    el.setSelectionRange(cursor, cursor)
    return true
  }

  if (el instanceof HTMLElement && el.isContentEditable) {
    el.focus()
    const selection = window.getSelection()
    if (!selection) return false

    let range = selection.rangeCount ? selection.getRangeAt(0) : null
    if (!range || !el.contains(range.commonAncestorContainer)) {
      range = document.createRange()
      range.selectNodeContents(el)
      range.collapse(false)
      selection.removeAllRanges()
      selection.addRange(range)
    }

    range.deleteContents()
    const node = document.createTextNode(payload)
    range.insertNode(node)
    range.setStartAfter(node)
    range.collapse(true)
    selection.removeAllRanges()
    selection.addRange(range)
    return true
  }

  return false
}

const openSelectionTranslatePanel = async () => {
  const text = selectionText.value.trim()
  if (!text) return

  const pw = window.innerWidth
  const ph = window.innerHeight

  const pad = 8
  const panelW = 360
  const panelH = 260
  const maxLeft = Math.max(pad, pw - panelW - pad)
  const maxTop = Math.max(pad, ph - panelH - pad)

  const baseLeft = Number(selectionToolbarLeft.value)
  const baseTop = Number(selectionToolbarTop.value)

  const nextLeft = Number.isFinite(baseLeft) ? baseLeft : pad
  const nextTop = Number.isFinite(baseTop) ? baseTop + 44 : 52

  selectionTranslatePanelPos.value = {
    left: Math.min(Math.max(pad, nextLeft), maxLeft),
    top: Math.min(Math.max(pad, nextTop), maxTop)
  }

  selectionTranslatePanelVisible.value = true

  await translateSelectionText(text)
}

const translateSelectionText = async (overrideText) => {
  const text = String((overrideText ?? selectionText.value) || '').trim()
  if (!text) return

  if (selectionTranslateBusy.value) return

  selectionBusy.value = true
  selectionTranslateBusy.value = true
  selectionTranslateRaw.value = text
  selectionTranslateResult.value = ''
  selectionTranslateError.value = ''
  selectionTranslateItems.value = []
  selectionTranslateWordMeta.value = null

  logger.info('[SelectionTranslate][MarkdownReader] start', { len: text.length, from: selectionTranslateFrom.value, to: selectionTranslateTo.value })

  try {
    // 词条模块（音标/短语/例句）和译文互不依赖，这里先并发发出去。
    // 原来是「译文 → 基础词条 → 详细词条」串行三跳，慢的时候要等七八秒。
    const needWordMeta = isWordSelection(text)
    const wordBasicPromise = needWordMeta
      ? invoke('language_learning_enrich_word_basic', {
        word: text,
        sourceLang: selectionTranslateFrom.value,
        targetLang: selectionTranslateTo.value
      }).catch((err) => {
        logger.warn('[SelectionTranslate][MarkdownReader] enrich basic failed:', err)
        return null
      })
      : Promise.resolve(null)
    const wordDetailsPromise = needWordMeta
      ? invoke('language_learning_enrich_word_details', {
        word: text,
        sourceLang: selectionTranslateFrom.value,
        targetLang: selectionTranslateTo.value
      }).catch((err) => {
        logger.warn('[SelectionTranslate][MarkdownReader] enrich details failed:', err)
        return null
      })
      : Promise.resolve(null)

    let hasUsableMultiResult = false
    if (selectionTranslateMultiAvailable.value) {
      try {
        const response = await invoke('selection_translate_free_multi', {
          text,
          from: selectionTranslateFrom.value,
          to: selectionTranslateTo.value
        })

        const items = Array.isArray(response?.items) ? response.items : []
        selectionTranslateItems.value = items.map((it) => ({
          engine: String(it?.engine || ''),
          success: Boolean(it?.success),
          text: String(it?.text || ''),
          error: it?.error ? String(it.error) : ''
        }))

        const primary = selectionTranslateItems.value.find((it) => it.success && String(it.text || '').trim())
        if (primary) {
          selectionTranslateResult.value = String(primary.text || '')
          selectionTranslateError.value = ''
          hasUsableMultiResult = true
          logger.info('[SelectionTranslate][MarkdownReader] multi success', { engines: selectionTranslateItems.value.length })
        } else {
          const err = String(response?.error || '翻译结果为空')
          throw new Error(err)
        }
      } catch (err) {
        const msg = String(err?.message || err || '')
        if (msg.includes('selection_translate_free_multi') && msg.toLowerCase().includes('not found')) {
          selectionTranslateMultiAvailable.value = false
          if (!selectionTranslateMultiNotFoundWarned) {
            selectionTranslateMultiNotFoundWarned = true
            logger.warn('[SelectionTranslate][MarkdownReader] multi command not found, disabled multi path. Please restart tauri backend.')
          }
        } else {
          logger.warn('[SelectionTranslate][MarkdownReader] multi failed, fallback to single:', err)
        }
      }
    }

    if (!hasUsableMultiResult) {
      const response = await invoke('translate_text_free', {
        texts: [text],
        from: selectionTranslateFrom.value,
        to: selectionTranslateTo.value
      })

      const translated = (response?.translated_texts?.[0] || '').toString()

      if (!translated.trim() || translated.includes('[Translation Failed]')) {
        const err = String(response?.error || '翻译结果为空')
        throw new Error(err)
      }

      if (!selectionTranslateResult.value) {
        selectionTranslateItems.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
        selectionTranslateResult.value = translated
        selectionTranslateError.value = ''
        logger.info('[SelectionTranslate][MarkdownReader] single success', { len: translated.length })
      }
    }

    const finalTranslated = String(selectionTranslateResult.value || '').trim()
    if (needWordMeta) {
      try {
        const [basic, details] = await Promise.all([wordBasicPromise, wordDetailsPromise])
        const base = {
          word: String(basic?.word || text).trim(),
          phonetic: String(basic?.phonetic || '').trim(),
          phonetic_uk: String(basic?.phonetic_uk || '').trim(),
          phonetic_us: String(basic?.phonetic_us || '').trim(),
          part_of_speech: String(basic?.part_of_speech || '').trim(),
          meanings: Array.isArray(basic?.meanings) ? basic.meanings : [],
          synonyms: [],
          antonyms: [],
          associates: [],
          synonyms_translations: [],
          antonyms_translations: [],
          associates_translations: [],
          phrases: [],
          phrase_translations: [],
          examples: Array.isArray(basic?.examples) ? basic.examples : [],
          example_translations: [],
        }
        selectionTranslateWordMeta.value = details
          ? {
            ...base,
            synonyms: Array.isArray(details?.synonyms) ? details.synonyms : [],
            antonyms: Array.isArray(details?.antonyms) ? details.antonyms : [],
            associates: Array.isArray(details?.associates) ? details.associates : [],
            synonyms_translations: Array.isArray(details?.synonyms_translations) ? details.synonyms_translations : [],
            antonyms_translations: Array.isArray(details?.antonyms_translations) ? details.antonyms_translations : [],
            associates_translations: Array.isArray(details?.associates_translations) ? details.associates_translations : [],
            phrases: Array.isArray(details?.phrases) ? details.phrases : [],
            phrase_translations: Array.isArray(details?.phrase_translations) ? details.phrase_translations : [],
            examples: Array.isArray(details?.examples) && details.examples.length ? details.examples : base.examples,
            example_translations: Array.isArray(details?.example_translations) ? details.example_translations : [],
          }
          : base
      } catch (err) {
        logger.warn('[SelectionTranslate][MarkdownReader] enrich word failed:', err)
      }
    }

    if (finalTranslated) {
      selectionInsertText.value = `**${text}**\n\n${finalTranslated}`
      try {
        const primaryItem = selectionTranslateItems.value.find((it) => it.success && String(it.text || '').trim())
        await invoke('language_learning_record_translation', {
          input: {
            source_text: text,
            translated_text: finalTranslated,
            source_lang: selectionTranslateFrom.value,
            target_lang: selectionTranslateTo.value,
            engine: String(primaryItem?.engine || 'Free'),
            record_type: isWordSelection(text) ? 'word' : undefined,
            word_meta: selectionTranslateWordMeta.value || undefined,
          }
        })
      } catch (err) {
        logger.warn('[SelectionTranslate][MarkdownReader] record save failed:', err)
      }
    }
  } catch (err) {
    const msg = String(err?.message || err || '翻译失败')
    selectionTranslateError.value = msg
    selectionTranslateItems.value = [{ engine: 'Free', success: false, text: '', error: msg }]
    selectionTranslateResult.value = ''
    selectionInsertText.value = ''
    logger.error('[SelectionTranslate][MarkdownReader] failed:', err)
    message.error(`翻译失败: ${msg}`)
  } finally {
    selectionTranslateBusy.value = false
    selectionBusy.value = false
  }
}

const translateSelection = async () => {
  selectionToolbarVisible.value = false
  await openSelectionTranslatePanel()
}

const querySelection = async () => {
  const text = selectionText.value.trim()
  if (!text) return

  const url = `https://www.bing.com/search?q=${encodeURIComponent(text)}`
  // 写入可插入笔记的链接，并在系统默认浏览器中打开搜索（用 plugin-opener 真正打开）。
  selectionInsertText.value = `[查询：${text}](${url})`
  try {
    await openUrl(url)
  } catch (e) {
    /* 打开失败时静默：链接已写入可插入笔记 */
  }
}

const getTranslateLangLabel = (value) => {
  const item = selectionTranslateLangOptions.find((x) => x.value === value)
  return item ? String(item.label || '') : '目标语言'
}

const sendSelectionToAssistant = (overrideText, options = {}) => {
  const text = normalizeSelectionTextForLanguageTasks(overrideText ?? selectionText.value)
  if (!text) {
    message.warning('请先框选要发送到助手的文本')
    return false
  }
  const autoSend = options.autoSend !== false
  const disableWebSearch = !!options.disableWebSearch
  rightSiderStore.setPanel('assistant')
  rightSiderStore.open()
  rightSiderStore.appendAssistantDraft(text, { autoSend, disableWebSearch })
  showAssistant.value = true
  selectionToolbarVisible.value = false
  message.success(options.successMessage || (autoSend ? '已发送到助手并开始提问' : '已插入到助手输入框'))
  return true
}

const askSelectionWithAssistant = () => {
  sendSelectionToAssistant(undefined, { autoSend: true, successMessage: '已发送到助手并开始提问' })
}

const openAssistantTranslateDialog = () => {
  const text = normalizeSelectionTextForLanguageTasks(selectionText.value)
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }
  assistantTranslateSourceText.value = text
  assistantTranslateDialogVisible.value = true
  selectionToolbarVisible.value = false
}

const sendSelectionAsAiTranslate = () => {
  const text = normalizeSelectionTextForLanguageTasks(assistantTranslateSourceText.value)
  if (!text) {
    assistantTranslateDialogVisible.value = false
    message.warning('请先框选要翻译的文本')
    return
  }
  const lang = String(assistantTranslateTargetLang.value || 'en')
  const label = getTranslateLangLabel(lang)
  const prompt = `请将以下内容翻译成${label}，保持原意并保留术语、代码与格式，只输出译文：\n\n${text}`
  const ok = sendSelectionToAssistant(prompt, {
    autoSend: true,
    disableWebSearch: true,
    successMessage: `已发送到助手并翻译为${label}`,
  })
  if (ok) {
    assistantTranslateDialogVisible.value = false
  }
}

const insertSelectionResult = async () => {
  const text = selectionInsertText.value.trim()
  if (!text) return

  const ok = insertIntoActiveNoteEditor(text)
  if (!ok) {
    await addToNotes({ type: 'text', content: text, originalMarkdown: text })
  }

  message.success('已插入笔记', { duration: 1200 })
  closeSelectionToolbar()
}

const handleSelectionMouseUp = () => {
  const selection = window.getSelection()
  const text = selection ? selection.toString().trim() : ''
  if (!text) {
    closeSelectionToolbar()
    return
  }

  const containers = [markdownContainer.value, sourceContent.value].filter((x) => x && x instanceof HTMLElement)
  if (!selection || selection.rangeCount === 0 || !containers.length) {
    closeSelectionToolbar()
    return
  }

  const range = selection.getRangeAt(0)
  const common = range.commonAncestorContainer
  const commonEl = common instanceof Element ? common : common.parentElement
  const container = commonEl ? containers.find((x) => x.contains(commonEl)) : null
  if (!container) {
    closeSelectionToolbar()
    return
  }

  const rect = range.getBoundingClientRect()
  if (!rect || (rect.width === 0 && rect.height === 0)) {
    closeSelectionToolbar()
    return
  }

  selDlog('mouseUp', {
    textLen: text.length,
    textPreview: text.slice(0, 80),
    rect: {
      left: Math.round(rect.left),
      top: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    },
    commonTag: commonEl?.tagName || null,
  })

  selDlog('mouseUp', {
    textLen: text.length,
    textPreview: text.slice(0, 80),
    rect: {
      left: Math.round(rect.left),
      top: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    },
    commonTag: commonEl?.tagName || null,
  })

  selectionText.value = text

  const pad = 8
  const toolHeight = SELECTION_TOOLBAR_HEIGHT

  let top = rect.top - toolHeight - 8
  if (top < pad) top = rect.bottom + 8

  const left = rect.left
  selectionToolbarTop.value = Math.round(top)
  selectionToolbarLeft.value = Math.round(
    Math.max(pad, Math.min(left, window.innerWidth - SELECTION_TOOLBAR_WIDTH - pad))
  )
  selectionToolbarVisible.value = true
  // 元素是刚创建的，下一帧按实测宽度把右边界夹取修准（常量只兜首帧）
  nextTick(() => {
    const el = selectionToolbarEl.value
    if (!el) return
    selectionToolbarLeft.value = Math.round(
      Math.max(pad, Math.min(left, window.innerWidth - el.offsetWidth - pad))
    )
  })
}

const handleGlobalMouseDown = (e) => {
  const t = e?.target
  if (!(t instanceof Element)) {
    closeSelectionToolbar()
    return
  }

  if (t.closest('.selection-toolbar')) return
  if (t.closest('.pot-selection-translate-panel')) return
  closeSelectionToolbar()
}

const updateActiveSearchMatch = (nextIndex) => {
  const total = inPageSearchMatches.value.length
  if (!total) return

  const normalized = ((nextIndex % total) + total) % total

  const prev = inPageSearchMatches.value[inPageSearchActiveIndex.value]
  if (prev) prev.classList.remove('inpage-match-active')

  inPageSearchActiveIndex.value = normalized

  const el = inPageSearchMatches.value[normalized]
  if (el) {
    el.classList.add('inpage-match-active')
    el.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'nearest' })
  }
}

const gotoNextSearchMatch = () => {
  if (!inPageSearchMatches.value.length) return
  updateActiveSearchMatch(inPageSearchActiveIndex.value + 1)
}

const gotoPrevSearchMatch = () => {
  if (!inPageSearchMatches.value.length) return
  updateActiveSearchMatch(inPageSearchActiveIndex.value - 1)
}

const clearInPageSearchHighlights = () => {
  const container = markdownContainer.value
  if (!container) return

  const marks = container.querySelectorAll('span.inpage-match, span.inpage-match-active')
  marks.forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
    parent.normalize()
  })
}

const applyInPageSearch = () => {
  clearInPageSearchHighlights()

  inPageSearchMatches.value = []
  inPageSearchActiveIndex.value = -1

  const query = inPageSearchQuery.value.trim()
  if (!query) return

  const container = markdownContainer.value
  if (!container) return

  const qLower = query.toLowerCase()

  const walker = document.createTreeWalker(
    container,
    NodeFilter.SHOW_TEXT,
    {
      acceptNode(node) {
        if (!node?.textContent || !node.textContent.trim()) return NodeFilter.FILTER_REJECT
        const parent = node.parentElement
        if (!parent) return NodeFilter.FILTER_REJECT
        if (parent.closest('script, style')) return NodeFilter.FILTER_REJECT
        return NodeFilter.FILTER_ACCEPT
      }
    }
  )

  const textNodes = []
  while (walker.nextNode()) textNodes.push(walker.currentNode)

  textNodes.forEach((textNode) => {
    const text = textNode.textContent || ''
    const lower = text.toLowerCase()

    let idx = lower.indexOf(qLower)
    if (idx === -1) return

    const frag = document.createDocumentFragment()
    let last = 0

    while (idx !== -1) {
      if (idx > last) frag.appendChild(document.createTextNode(text.slice(last, idx)))

      const span = document.createElement('span')
      span.className = 'inpage-match'
      span.textContent = text.slice(idx, idx + query.length)
      frag.appendChild(span)
      inPageSearchMatches.value.push(span)

      last = idx + query.length
      idx = lower.indexOf(qLower, last)
    }

    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)))

    const parent = textNode.parentNode
    if (parent) parent.replaceChild(frag, textNode)
  })

  if (inPageSearchMatches.value.length) {
    nextTick(() => updateActiveSearchMatch(0))
  }
}

const openInPageSearch = async () => {
  inPageSearchVisible.value = true
  await nextTick()
  const input = inPageSearchInput.value
  if (input && input instanceof HTMLInputElement) {
    input.focus()
    input.select()
  }
  applyInPageSearch()
}

const closeInPageSearch = () => {
  inPageSearchVisible.value = false
  inPageSearchQuery.value = ''
  clearInPageSearchHighlights()
  inPageSearchMatches.value = []
  inPageSearchActiveIndex.value = -1
}

const handleMarkdownHover = (e) => {
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });

  const element = e.target.closest(
    'p, h1, h2, h3, h4, h5, h6, li, blockquote, ' +
    'pre, code, .code-block, ' +
    '.katex-display, .katex-inline, ' +
    '.mermaid, svg'
  );

  if (element) {
    element.classList.add('hovered');
  }
}

const handleMarkdownLeave = () => {
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });
}

const updateMarkdownPath = async () => {
  const baseFileName = props.fileName.replace('.md', '')
  markdownFilePath.value = await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'md',
    `${baseFileName}.md`
  )
}

const getNotesDirectory = async () => {
  const baseFileName = props.fileName.replace('.md', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'notes'
  );
};

const getImagesDirectory = async () => {
  const baseFileName = props.fileName.replace('.md', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'images'
  );
};

const loadMarkdownPage = async () => {
  try {
    if (!markdownFilePath.value) {
      await updateMarkdownPath();
    }

    logger.info('Loading markdown from:', markdownFilePath.value);
    const response = await invoke('markdown_reader_load_markdown_file', {
      filepath: markdownFilePath.value,
    });

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    const mdDir = await dirname(markdownFilePath.value);
    markdownContent.value = response.content;
    markdownLines.value = await convertMarkdownToLines(markdownContent.value, { libraryBasePath: mdDir });

    await nextTick();
    requestAnimationFrame(() => {
      const container = markdownContainer.value;
      if (container) {
        const theme = resolveMarkdownTheme();
        addCopyButtons(container, { theme });
        addMediaViewerButtons(container, { theme });
        logger.info('[MarkdownReader] media buttons refreshed:', {
          images: container.querySelectorAll('img').length,
          mermaid: container.querySelectorAll('.mermaid, svg').length,
        });
      }
    });

    generateOutlineFromMarkdown(markdownContent.value);

    await calculateTotalPages();

  } catch (error) {
    logger.error('加载Markdown失败:', error);
    markdownLines.value = [{
      raw: '',
      html: `<div class="error">加载错误: ${error.message}</div>`
    }];
    outlineStore.setOutline([]);
    totalPages.value = 1;
  }
};

const ensureNotesPath = async () => {
  if (!notesFilePath.value) {
    const notesDir = await getNotesDirectory()
    notesFilePath.value = await join(notesDir, `${props.fileName.replace('.md', '')}.json`)
  }
  return notesFilePath.value
}

const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString()
}

const addNewCard = () => {
  const now = Date.now();
  const placeholder = '点击编辑内容...';
  const newCard = {
    id: now.toString(),
    type: 'single',
    title: '新笔记',
    timestamp: new Date().toISOString(),
    items: [{
      id: `${now}-0`,
      type: 'text',
      content: placeholder,
      originalMarkdown: placeholder,
      original_markdown: placeholder,
      timestamp: new Date().toISOString()
    }]
  }

  noteCards.value.push(newCard)
  updateBackendNotes('add', newCard)
}

const deleteCard = async (index) => {
  if (index >= 0 && index < noteCards.value.length) {
    const cardId = noteCards.value[index].id;
    noteCards.value.splice(index, 1);

    await updateBackendNotes('delete', { id: cardId });

    if (activeCardIndex.value === index) {
      activeCardIndex.value = -1;
    } else if (activeCardIndex.value > index) {
      activeCardIndex.value--;
    }
  }
};

const addToNotes = async (element) => {
  if (altKeyCollector.value.isActive) {
    appendToCurrentNote(element);
    return;
  }

  const safeContent = (element?.originalMarkdown || element?.content || '').trim();
  if (!safeContent) {
    showSelectionMessage('未选中有效内容');
    return;
  }

  const normalizedElement = {
    ...element,
    content: element?.content || safeContent,
    originalMarkdown: element?.originalMarkdown || safeContent
  };

  const newCard = {
    id: Date.now().toString(),
    type: 'single',
    title: getCardTitle(normalizedElement.type),
    timestamp: new Date().toISOString(),
    items: [{
      id: `${Date.now()}-0`,
      type: normalizedElement.type,
      content: normalizedElement.content,
      originalMarkdown: normalizedElement.originalMarkdown,
      original_markdown: normalizedElement.originalMarkdown,
      timestamp: new Date().toISOString()
    }]
  };

  noteCards.value.push(newCard);
  await updateBackendNotes('add', newCard);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) {
      cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
    }
  });
};

const getCardTitle = (type) => {
  const titles = {
    text: '文本摘录',
    image: '图片摘录',
    code: '代码摘录',
    math: '公式摘录',
    diagram: '图表摘录'
  }
  return titles[type] || '笔记'
}

const loadExistingNotes = async () => {
  try {
    const response = await invoke('load_notes_file', {
      filePath: notesFilePath.value
    });

    const notesData = typeof response === 'string' ? JSON.parse(response) : response;

    if (notesData?.cards?.length) {
      noteCards.value = notesData.cards.map(card => {
        const items = card.items || [{
          id: card.id || Date.now().toString(),
          type: card.type === 'composite' ? 'text' : card.type,
          content: card.content,
          originalMarkdown: card.originalMarkdown || card.content,
          timestamp: card.timestamp || new Date().toISOString()
        }];

        return {
          ...card,
          id: card.id || Date.now().toString(),
          type: items.length > 1 ? 'composite' : 'single',
          items,
          timestamp: card.timestamp || new Date().toISOString()
        };
      });
    }
  } catch (error) {
    logger.error('加载笔记失败:', error);
    noteCards.value = [];
  }
};

const updateBackendNotes = async (action, cardData) => {
  try {
    if (!cardData?.id) {
      throw new Error('无效的笔记ID');
    }

    const items = Array.isArray(cardData.items) ? cardData.items : [];

    const payload = {
      action,
      data: {
        id: cardData.id,
        type: cardData.type || 'single',
        title: cardData.title || '',
        items: items.map(item => ({
          id: item.id || `${cardData.id}-${Date.now()}`,
          type: item.type || 'text',
          content: item.content || '',
          original_markdown: item.original_markdown || item.originalMarkdown || item.content || '',
          timestamp: item.timestamp || new Date().toISOString()
        })),
        timestamp: cardData.timestamp || new Date().toISOString()
      },
      filePath: await ensureNotesPath(),
      importFileId: Number(props.importFileId) || 0,
      importFileName: props.fileName || '',
    };

    await invoke('update_notes_file', payload);
    message.success('笔记已保存', { duration: 1500 });

  } catch (error) {
    const errorMessage = typeof error === 'object'
      ? error.message || error.toString()
      : String(error);

    logger.error('更新笔记失败:', {
      error: errorMessage,
      cardData: JSON.parse(JSON.stringify(cardData || {}))
    });

    const normalizedError = errorMessage.toLowerCase();
    if (normalizedError.includes('未找到') || normalizedError.includes('not found')) {
      await handleMissingNote(action, cardData);
    } else {
      message.error(`保存失败: ${errorMessage}`, { duration: 3000 });
    }
  }
};

const handleMissingNote = async (action, cardData) => {
  try {
    await loadExistingNotes();
    const exists = noteCards.value.some(card => card.id === cardData?.id);

    if (!exists) {
      const completeCard = {
        ...cardData,
        items: Array.isArray(cardData.items) ? cardData.items : [],
        timestamp: cardData.timestamp || new Date().toISOString()
      };

      await updateBackendNotes('add', completeCard);
    }
  } catch (error) {
    logger.error('处理缺失笔记失败:', error);
    message.error('自动恢复失败，请手动添加笔记');
  }
};

const exportNotes = () => {
  if (!notesContent.value) {
    alert('笔记内容为空');
    return;
  }

  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = notesContent.value;

  let exportContent = '';
  const items = tempDiv.querySelectorAll('.note-item');
  items.forEach(item => {
    const type = item.querySelector('.note-type')?.textContent || '笔记';
    const time = item.querySelector('.note-time')?.textContent || '';
    const content = item.querySelector('.note-content')?.textContent || '';

    exportContent += `### ${type} (${time})\n\n`;
    exportContent += `${content}\n\n---\n\n`;
  });

  const blob = new Blob([exportContent], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);

  const a = document.createElement('a');
  a.href = url;
  a.download = `Markdown笔记_${props.fileName.replace('.md', '')}_${new Date().toISOString().slice(0, 10)}.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

const startDrag = (e) => {
  try {
    if (e.button !== 0 || !markdownPanel.value) return;

    if (animationFrameId.value) {
      cancelAnimationFrame(animationFrameId.value);
      animationFrameId.value = null;
    }

    isDragging.value = true;
    isDecelerating.value = false;
    startDragPos.value = {
      x: e.clientX,
      y: e.clientY
    };
    startScrollPos.value = {
      left: markdownPanel.value.scrollLeft,
      top: markdownPanel.value.scrollTop
    };
    velocity.value = { x: 0, y: 0 };
    lastDragTime.value = performance.now();

    document.body.style.userSelect = 'none';
    markdownPanel.value.style.cursor = 'grabbing';

    e.preventDefault();
  } catch (error) {
    logger.error('Error in startDrag:', error);
    resetDragState();
  }
};

const handleDrag = (e) => {
  try {
    if (!isDragging.value || !markdownPanel.value) return;

    const now = performance.now();
    const deltaTime = now - lastDragTime.value;
    lastDragTime.value = now;

    const dx = e.clientX - startDragPos.value.x;
    const dy = e.clientY - startDragPos.value.y;

    markdownPanel.value.scrollLeft = startScrollPos.value.left - dx;
    markdownPanel.value.scrollTop = startScrollPos.value.top - dy;

    if (deltaTime > 0) {
      velocity.value = {
        x: dx / deltaTime,
        y: dy / deltaTime
      };
    }

    e.preventDefault();
  } catch (error) {
    logger.error('Error in handleDrag:', error);
    endDrag();
  }
};

const endDrag = () => {
  try {
    if (!isDragging.value) return;

    resetDragState();

    const speedThreshold = 0.1;
    if (Math.abs(velocity.value.x) > speedThreshold ||
      Math.abs(velocity.value.y) > speedThreshold) {
      isDecelerating.value = true;
      decelerate();
    }
  } catch (error) {
    logger.error('Error in endDrag:', error);
    resetDragState();
  }
};

const resetDragState = () => {
  isDragging.value = false;
  isDecelerating.value = false;
  document.body.style.userSelect = '';
  if (markdownPanel.value) {
    markdownPanel.value.style.cursor = 'grab';
  }
};

const decelerate = () => {
  if (!isDecelerating.value || !markdownPanel.value) return;

  const friction = 0.95;
  const minSpeed = 0.5;

  velocity.value.x *= friction;
  velocity.value.y *= friction;

  markdownPanel.value.scrollLeft += velocity.value.x * 16;
  markdownPanel.value.scrollTop += velocity.value.y * 16;

  if (Math.abs(velocity.value.x) > minSpeed || Math.abs(velocity.value.y) > minSpeed) {
    animationFrameId.value = requestAnimationFrame(decelerate);
  } else {
    isDecelerating.value = false;
    velocity.value = { x: 0, y: 0 };
  }
};

const handleWheelZoom = (e) => {
  if (e.ctrlKey) {
    e.preventDefault()
    const delta = e.deltaY < 0 ? 0.1 : -0.1
    zoomLevel.value = Math.max(0.5, Math.min(2, zoomLevel.value + delta))

    nextTick(() => {
      if (markdownPanel.value && markdownContent.value) {
        const panel = markdownPanel.value
        const content = markdownContent.value

        panel.scrollLeft = Math.max(0, (content.clientWidth * zoomLevel.value - panel.clientWidth) / 2)
        panel.scrollTop = Math.max(0, (content.clientHeight * zoomLevel.value - panel.clientHeight) / 2)
      }
    })
  }
}

const prevPage = () => {
  if (currentPage.value > 1) {
    goToPage(currentPage.value - 1);
  }
};

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    goToPage(currentPage.value + 1);
  }
};

const goToPage = (page) => {
  const pageNum = typeof page === 'number' ? page : jumpPage.value;
  const targetPage = Math.max(1, Math.min(pageNum, totalPages.value));
  currentPage.value = targetPage;
  jumpPage.value = targetPage;

  const targets = [markdownPanel.value, sourcePanel.value].filter((x) => x && x instanceof HTMLElement)
  if (!targets.length) return

  for (const el of targets) {
    const viewportHeight = el.clientHeight || 1
    el.scrollTo({ top: (targetPage - 1) * viewportHeight, behavior: 'smooth' })
  }
};

const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value;
  if (showNotes.value) {
    noteCards.value = [...noteCards.value];
  }
};

const toggleAssistantPanel = () => {
  showAssistant.value = !showAssistant.value
  if (showAssistant.value) {
    showNotes.value = false
    rightSiderStore.setPanel('assistant')
    rightSiderStore.open()
  }
}

const zoomIn = () => {
  zoomLevel.value = Math.min(zoomLevel.value + 0.1, 2);
};

const zoomOut = () => {
  zoomLevel.value = Math.max(zoomLevel.value - 0.1, 0.5);
};

const startResize = (e) => {
  isResizing.value = true;
  startX.value = e.clientX;
  startLeftWidth.value = parseInt(document.defaultView.getComputedStyle(document.querySelector('.markdown-left-wrapper')).width, 10);
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResize);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

const handleMouseMove = (e) => {
  if (!isResizing.value) return;

  const containerWidth = document.querySelector('.markdown-main-content').offsetWidth;
  const dx = e.clientX - startX.value;
  let newLeftWidth = ((startLeftWidth.value + dx) * 100) / containerWidth;

  newLeftWidth = Math.max(30, Math.min(70, newLeftWidth));

  leftPanelWidth.value = `${newLeftWidth}%`;
};

const stopResize = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};

const fitToWidth = () => {
  let isFitting = false;
  if (!markdownPanel.value || !markdownContent.value || isFitting.value) return;
  isFitting = true;

  try {
    const panel = markdownPanel.value;
    const content = markdownContent.value;

    if (!content.clientWidth) {
      return;
    }

    const panelWidth = panel.clientWidth;
    const panelHeight = panel.clientHeight;

    const horizontalMargin = Math.min(20, panelWidth * 0.05);
    const verticalMargin = Math.min(20, panelHeight * 0.05);

    const minScale = 0.5;
    const widthRatio = (panelWidth - horizontalMargin * 2) / content.clientWidth;
    const heightRatio = (panelHeight - verticalMargin * 2) / content.clientHeight;

    let newZoom = Math.max(minScale, Math.min(widthRatio, heightRatio));

    if (content.clientWidth * newZoom < panelWidth * 0.8 &&
      content.clientHeight * newZoom < panelHeight * 0.8) {
      newZoom = Math.max(widthRatio, heightRatio);
    }

    zoomLevel.value = newZoom;

    requestAnimationFrame(() => {
      const scaledWidth = content.clientWidth * zoomLevel.value;
      const scaledHeight = content.clientHeight * zoomLevel.value;

      velocity.value = { x: 0, y: 0 };
      isDecelerating.value = false;

      panel.scrollLeft = Math.max(0, Math.min(
        (scaledWidth - panelWidth) / 2,
        scaledWidth - panelWidth
      ));
      panel.scrollTop = Math.max(0, Math.min(
        (scaledHeight - panelHeight) / 2,
        scaledHeight - panelHeight
      ));
    });

  } catch (error) {
    logger.error('fitToWidth错误:', error);
  } finally {
    isFitting = false;
  }
}

const debounce = (fn, delay) => {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), delay);
  };
};

const translateQueue = ref([]);
const lastTranslateTime = ref(0);
const translateBatchSize = 5;
const translateInterval = 200;

const translateMarkdown = async () => {
  if (isTranslating.value) return;

  const linesToTranslate = markdownLines.value
    .filter(line => !line.skipTranslation && !line.translation)
    .map(line => {
      const tempDiv = document.createElement('div');
      tempDiv.innerHTML = line.html;
      const textNodes = [];
      const extractText = (node) => {
        if (shouldSkipTranslation(node)) return;

        if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
          textNodes.push(node.textContent.trim());
        } else if (node.childNodes) {
          for (const child of node.childNodes) {
            extractText(child);
          }
        }
      };
      extractText(tempDiv);

      return {
        line,
        text: textNodes.join(' '),
        html: line.html
      };
    })
    .filter(item => item.text.length > 0);

  translateQueue.value.push(...linesToTranslate);
  processTranslateQueue();
};

const processTranslateQueue = async () => {
  if (isTranslating.value || translateQueue.value.length === 0) return;

  const now = Date.now();
  if (now - lastTranslateTime.value < translateInterval) {
    setTimeout(processTranslateQueue, translateInterval - (now - lastTranslateTime.value));
    return;
  }

  isTranslating.value = true;
  try {
    const currentBatch = translateQueue.value.splice(0, translateBatchSize);
    const texts = currentBatch.map(item => item.text);

    const response = await invoke('translate_text_free', {
      texts: texts,
    });

    if (response && response.translated_texts?.length === texts.length) {
      currentBatch.forEach((item, index) => {
        const translatedText = response.translated_texts[index];
        if (translatedText) {
          item.line.translation = wrapTranslation(item.line.html, translatedText);
          item.line.translationError = null;
        } else {
          item.line.translationError = '翻译结果为空';
        }
      });
    } else {
      const errorMsg = response?.error ||
        (response?.translated_texts ? '返回结果数量不匹配' : '无效的响应格式');
      currentBatch.forEach(item => {
        item.line.translationError = `翻译失败: ${errorMsg}`;
      });
      logger.error('翻译错误:', response);
    }

    lastTranslateTime.value = Date.now();
  } catch (error) {
    logger.error('翻译异常:', error);
    translateQueue.value.forEach(item => {
      item.line.translationError = `翻译异常: ${error.message || '未知错误'}`;
    });
  } finally {
    isTranslating.value = false;
    if (translateQueue.value.length > 0) {
      setTimeout(processTranslateQueue, translateInterval);
    }
  }
};

const wrapTranslation = (html, translatedText) => {
  if (!translatedText || translatedText.trim() === '') {
    return html;
  }

  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = html;

  const textNodes = [];
  const findTextNodes = (node) => {
    if (shouldSkipTranslation(node)) return;

    if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
      textNodes.push(node);
    } else if (node.childNodes) {
      for (const child of node.childNodes) {
        findTextNodes(child);
      }
    }
  };
  findTextNodes(tempDiv);

  let textPos = 0;
  for (const node of textNodes) {
    const originalText = node.textContent.trim();
    if (textPos < translatedText.length) {
      const endPos = Math.min(textPos + originalText.length, translatedText.length);
      node.textContent = translatedText.substring(textPos, endPos);
      textPos = endPos + 1;
    }
  }

  return tempDiv.innerHTML;
};

const toggleTranslate = async () => {
  showTranslation.value = !showTranslation.value;

  if (showTranslation.value) {
    markdownLines.value = markdownLines.value.map(line => {
      if (!line.translation) {
        line.skipTranslation = shouldSkipTranslation(line.raw, line.html);
        if (line.skipTranslation) {
          line.translation = line.html;
        }
      }
      return line;
    });

    await translateMarkdown();
  } else {
    // 新增：隐藏译文时重置翻译状态
    isTranslating.value = false;
    translateQueue.value = [];
    lastTranslateTime.value = 0;

    // 移除所有行的翻译错误状态
    markdownLines.value = markdownLines.value.map(line => {
      line.translationError = null;
      return line;
    });
  }
};
const contentEditables = ref([]);
let autoSaveInterval = null;

const setContentEditableRef = (index, el) => {
  const idx = Number(index);
  if (!Number.isFinite(idx) || idx < 0) return;

  if (contentEditables.value[idx] === el) return;
  contentEditables.value = [...contentEditables.value];
  contentEditables.value[idx] = el || null;
};

const editorDirty = ref([]);

const setEditorDirty = (index, dirty) => {
  const idx = Number(index);
  if (!Number.isFinite(idx) || idx < 0) return;
  if (editorDirty.value[idx] === Boolean(dirty)) return;
  editorDirty.value = [...editorDirty.value];
  editorDirty.value[idx] = Boolean(dirty);
};

const markEditorDirty = (index) => setEditorDirty(index, true);
const resetEditorDirty = (index) => setEditorDirty(index, false);

const getSingleEditorText = (card) => {
  const item = card?.items?.[0] || {};
  return String(item.original_markdown || item.originalMarkdown || item.content || '');
};

const saveCardEdit = async (index) => {
  try {
    if (index < 0 || index >= noteCards.value.length) {
      return;
    }

    const editableDiv = contentEditables.value[index];
    if (!editableDiv) {
      return;
    }

    const currentCard = JSON.parse(JSON.stringify(noteCards.value[index]));

    if (currentCard.type === 'composite') {
      currentCard.content = editableDiv.innerHTML;
    } else {
      if (!Array.isArray(currentCard.items)) {
        currentCard.items = [];
      }

      let raw = editableDiv?.tagName === 'TEXTAREA' ? editableDiv.value : editableDiv.innerText;
      if (editableDiv?.tagName === 'TEXTAREA' && !editorDirty.value[index] && !String(raw || '').trim()) {
        raw = getSingleEditorText(currentCard);
      }

      if (currentCard.items[0]) {
        currentCard.items[0].content = raw;
        currentCard.items[0].original_markdown = raw;
        currentCard.items[0].originalMarkdown = raw;
      } else {
        currentCard.items[0] = {
          id: `${currentCard.id}-0`,
          type: 'text',
          content: raw,
          original_markdown: raw,
          originalMarkdown: raw,
          timestamp: new Date().toISOString()
        };
      }
    }

    currentCard.timestamp = new Date().toISOString();
    await updateBackendNotes('update', currentCard);

    noteCards.value.splice(index, 1, currentCard);
    resetEditorDirty(index);
    activeCardIndex.value = -1;

  } catch (error) {
    logger.error('保存编辑失败:', {
      error,
      index,
      card: noteCards.value[index]
    });
    message.error(`保存失败: ${error.message}`);
  }
};

const exitEditMode = (index) => {
  const card = noteCards.value[index];
  const editableDiv = contentEditables.value[index];

  if (card?.type !== 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    const before = getSingleEditorText(card);
    if (editableDiv.value !== before) {
      if (!confirm('有未保存的修改，确定要取消吗？')) return;
    }
    resetEditorDirty(index);
    activeCardIndex.value = -1;
    return;
  }

  if (editableDiv && editableDiv.innerHTML !== noteCards.value[index].content) {
    if (!confirm('有未保存的修改，确定要取消吗？')) return;
  }
  activeCardIndex.value = -1;
};

const enterEditMode = async (index) => {
  activeCardIndex.value = index;
  await nextTick();
  const editableDiv = contentEditables.value[index];
  const card = noteCards.value[index];

  if (card?.type !== 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    resetEditorDirty(index);
    if (!editableDiv.value || editableDiv.value.trim() === '') {
      editableDiv.value = getSingleEditorText(card);
    }
    editableDiv.focus();
    editableDiv.setSelectionRange(editableDiv.value.length, editableDiv.value.length);
    return;
  }

  if (editableDiv) {
    if (!editableDiv.innerHTML || editableDiv.innerHTML.trim() === '') {
      editableDiv.innerHTML = noteHtmls.value[index] || '';
    }
    editableDiv.focus();
    const range = document.createRange();
    range.selectNodeContents(editableDiv);
    range.collapse(false);
    const selection = window.getSelection();
    if (selection) {
      selection.removeAllRanges();
      selection.addRange(range);
    }
  }
};

const debouncedSave = debounce(saveCardEdit, 500);

const handleKeyUp = (e) => {
  if (e.key === 'Alt' && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

onMounted(async () => {
  await store.get_librariesRoot()
  await updateMarkdownPath()
  outlineStore.setJumpToLine(jumpToHeading)

  const state = loadReaderState();
  bookmarks.value = [...new Set(state.bookmarks)].sort((a, b) => a - b);
  totalReadingMs.value = state.totalReadingMs;
  completedAt.value = state.completedAt;
  completedReadingMs.value = state.completedReadingMs;
  pendingRestorePage.value = state.lastPage;
  isHydrated.value = true;
  startReadingSession();

  const handlePanelScroll = debounce(() => {
    const panel = markdownPanel.value;
    if (!panel) return;
    const viewportHeight = panel.clientHeight || 1;
    const tp = totalPages.value || 1;
    const page = Math.max(1, Math.min(tp, Math.floor(panel.scrollTop / viewportHeight) + 1));
    if (page !== currentPage.value) {
      currentPage.value = page;
      jumpPage.value = page;
    }
  }, 100);

  await Promise.all([
    (async () => {
      await ensureNotesPath()
      await loadExistingNotes()
    })(),
    loadMarkdownPage()
  ])

  if (markdownPanel.value) {
    markdownPanel.value.addEventListener('scroll', handlePanelScroll, { passive: true });
  }

  if (pendingRestorePage.value != null && totalPages.value) {
    const p = Math.max(1, Math.min(Number(pendingRestorePage.value) || 1, totalPages.value));
    pendingRestorePage.value = null;
    nextTick(() => goToPage(p));
  }

  watch(currentPage, () => {
    if (!isHydrated.value) return;
    markCompletedIfNeeded();
    persistReaderState();
  });

  watch(bookmarks, () => {
    if (!isHydrated.value) return;
    persistReaderState();
  }, { deep: true });

  window.addEventListener('keyup', handleKeyUp);

  nextTick(() => {
    window.addEventListener('resize', debounce(() => {
      if (isNotesReady.value) fitToWidth()
    }, 300))

    const checkContentLoaded = () => {
      if (markdownContent.value?.clientWidth > 0) {
        fitToWidth()
      } else {
        setTimeout(checkContentLoaded, 100)
      }
    }
    checkContentLoaded()
  })

  autoSaveInterval = setInterval(() => {
    const idx = activeCardIndex.value;
    if (idx < 0 || idx >= noteCards.value.length) return;
    if (!contentEditables.value?.[idx]) return;
    saveCardEdit(idx);
  }, 30000);
})

onBeforeUnmount(() => {
  if (autoSaveInterval) {
    clearInterval(autoSaveInterval);
    autoSaveInterval = null;
  }

  endReadingSession();
  markCompletedIfNeeded();
  if (isHydrated.value) persistReaderState();

  window.removeEventListener('resize', handleResize);
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  window.removeEventListener('keyup', handleKeyUp);
  outlineStore.setOutline([]);
});

const handleResize = () => {
  if (markdownPanel.value && markdownContent.value) {
    fitToWidth();
  }
};

watch(() => props.filePath, async (newPath) => {
  if (newPath) {
    await updateMarkdownPath();
    currentPage.value = 1;
    loadMarkdownPage(1);
    notesContent.value = '';
  }
});

watchEffect((onInvalidate) => {
  const token = ++notesRenderToken;
  const cards = noteCards.value.slice();
  let cancelled = false;
  onInvalidate(() => {
    cancelled = true;
  });

  (async () => {
    const htmls = await Promise.all(
      cards.map(async card => {
        if (card.type === 'single') {
          const item = card.items?.[0] || {};
          return await renderNoteContent(item.original_markdown || item.originalMarkdown || item.content || '');
        }
        return "";
      })
    );

    if (cancelled || token !== notesRenderToken) return;
    noteHtmls.value = htmls;

    const segments = await Promise.all(
      cards.map(card =>
        card.type === 'composite'
          ? parseCompositeContent(card.items)
          : []
      )
    );

    if (cancelled || token !== notesRenderToken) return;
    compositeSegments.value = segments;

    nextTick(() => {
      if (cancelled || token !== notesRenderToken) return;
      document.querySelectorAll('.notes-panel .mermaid').forEach(el => {
        try {
          if (!el.querySelector('svg')) {
            mermaid.run({ nodes: [el] });
          }
        } catch (e) {
        }
      });
    });
  })();
});
</script>

<style scoped>
.markdown-left-wrapper {
  display: flex;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.markdown-source-panel {
  flex: 1 1 auto;
  min-width: 0;
  overflow: auto;
  position: relative;
  background-color: var(--guaxi-content-bg);
  padding: 1px;
}

.markdown-source-container {
  width: 100%;
  height: 100%;
  overflow: auto;
}

.markdown-source-content {
  width: 100%;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  font-size: 18px;
  line-height: 1.6;
  color: var(--guaxi-text-primary);
  tab-size: 2;
  padding: 4px;
}

.markdown-source-line {
  white-space: pre;
}

.markdown-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 18px;
  line-height: 1.6;
  color: var(--guaxi-text-primary);
  background-color: var(--guaxi-content-bg);
  user-select: text;
}

.markdown-viewer-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
}

button {
  padding: 5px 8px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

button:hover {
  background-color: var(--guaxi-header-bg);
  border-color: var(--guaxi-border);
  transform: translateY(-1px);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

button:disabled {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}

.markdown-main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  gap: 0;
}

.markdown-content-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
  background-color: var(--guaxi-content-bg);
  transform-origin: center center !important;
  transition: transform 0.3s ease-out;
  will-change: transform;
}

.rendered-markdown {
  width: 100%;
  height: 100%;
  padding: 20px;
  overflow: auto;
}

.markdown-loading-message {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  color: var(--guaxi-text-secondary);
  font-size: 14px;
}

.resize-handle {
  width: 3px;
  height: 100%;
  background-color: var(--guaxi-border);
  cursor: col-resize;
  position: relative;
  z-index: 10;
  transition: background-color 0.2s;
}

.resize-handle:hover {
  background-color: #4acadb;
}

.resize-handle:active {
  background-color: #55ebd7;
}

.markdown-panel {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 5px;
  background-color: var(--guaxi-content-bg);
  display: flex;
}

.outline-panel-wrapper {
  /* 宽度由内联 flex-basis（outlineWidth）决定，可拖拽调整 */
  flex: 0 0 auto;
  min-width: 0;
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
  background-color: var(--guaxi-panel-bg);
}

/* 大纲与正文之间的拖拽把手，兼作分隔线（原来只有一条拖不动的 border-right） */
.outline-resize-handle {
  flex: 0 0 3px;
  width: 3px;
  height: 100%;
  background-color: var(--guaxi-border);
  cursor: col-resize;
  z-index: 10;
  transition: background-color 0.2s;
}

.outline-resize-handle:hover {
  background-color: #4acadb;
}

.markdown-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 18px;
  line-height: 1.6;
  color: var(--guaxi-text-primary);
  background-color: var(--guaxi-content-bg);
  user-select: text;
  padding: 10px;
  cursor: text;
  word-wrap: break-word;
  overflow-wrap: break-word;
  word-break: break-word;
}

.markdown-content :deep(a),
.markdown-content :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .markdown-content :deep(a),
.dark-mode .markdown-content :deep(a:visited),
.dark .markdown-content :deep(a),
.dark .markdown-content :deep(a:visited),
:root[data-guaxi-theme='dark'] .markdown-content :deep(a),
:root[data-guaxi-theme='dark'] .markdown-content :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.markdown-content :deep(a:hover) {
  opacity: 0.92;
}

.markdown-content :deep(p),
.markdown-content :deep(h1),
.markdown-content :deep(h2),
.markdown-content :deep(h3),
.markdown-content :deep(h4),
.markdown-content :deep(h5),
.markdown-content :deep(h6),
.markdown-content :deep(li),
.markdown-content :deep(blockquote) {
  position: relative;
  transition: all 0.2s ease;
  padding: 4px 8px;
  margin: 4px 0;
  border-radius: 4px;
}

.markdown-content :deep(.markdown-image) {
  max-width: 100%;
  height: auto;
  margin: 10px 0;
  border-radius: 4px;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
  cursor: pointer;
  transition: transform 0.2s;
}

.markdown-content :deep(.markdown-image:hover) {
  transform: scale(1.02);
}

.markdown-content :deep(.selected),
.markdown-content :deep(.hovered) {
  background-color: rgba(72, 215, 240, 0.2);
  box-shadow: 0 0 0 2px rgba(65, 210, 230, 0.5);
  transform: scale(1.005);
  outline: none;
  cursor: pointer;
}

.markdown-content :deep(.selected) {
  background-color: rgba(33, 213, 236, 0.3);
  box-shadow: 0 0 0 3px rgba(85, 231, 207, 0.7);
}

.selection-message {
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  background-color: rgba(0, 0, 0, 0.8);
  color: white;
  padding: 10px 20px;
  border-radius: 20px;
  z-index: 1001;
  opacity: 1;
  transition: opacity 0.5s ease;
  font-size: 14px;
  pointer-events: none;
}

.selection-message.fade-out {
  opacity: 0;
}

.selection-toolbar {
  position: fixed;
  z-index: 1100;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  padding: 6px;
  /* 按图标内容收缩：固定 320px 会在按钮右侧留一大段空条 */
  width: max-content;
  max-width: calc(100vw - 16px);
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  backdrop-filter: blur(8px);
  box-shadow: var(--guaxi-header-shadow);
  color: var(--guaxi-text-primary);
}

.selection-translate-layer {
  position: fixed;
  left: 0;
  top: 0;
  width: 100vw;
  height: 100vh;
  z-index: 1150;
  pointer-events: none;
}

.selection-translate-layer-inner {
  position: relative;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.selection-translate-layer-inner :deep(.pot-selection-translate-panel) {
  pointer-events: auto;
}

.selection-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.selection-toolbar-body {
  max-height: 220px;
  overflow: auto;
  padding: 8px 10px;
  background: var(--guaxi-header-bg);
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}

.selection-toolbar-body :deep(p) {
  margin: 0 0 6px;
}

.selection-toolbar-body :deep(p:last-child) {
  margin-bottom: 0;
}

.selection-toolbar-loading {
  opacity: 0.85;
}

.selection-tool-btn {
  width: 32px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.selection-tool-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.inpage-search-bar {
  position: fixed;
  top: 64px;
  right: 16px;
  z-index: 1200;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  backdrop-filter: blur(8px);
  box-shadow: var(--guaxi-header-shadow);
}

.inpage-search-input {
  width: 220px;
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  outline: none;
}

.inpage-search-count {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  min-width: 48px;
  text-align: center;
}

.inpage-search-btn {
  width: 32px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.markdown-content :deep(.inpage-match) {
  background: rgba(255, 230, 120, 0.55);
  border-radius: 4px;
  padding: 0 2px;
}

.markdown-content :deep(.inpage-match-active) {
  background: rgba(255, 180, 80, 0.75);
  box-shadow: 0 0 0 2px rgba(255, 180, 80, 0.35);
}

.notes-panel {
  position: fixed;
  top: 0;
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 100;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--guaxi-border);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 14px;
}

.notes-panel-visible {
  right: 0;
}

/* 笔记头部：原来是写死的饱和蓝色块 + 白字，暗色主题下就是面板顶上一整块突兀的亮色，
   和面板自身的主题色也对不上。改成跟随主题的安静表面——与面板同色，靠一条分隔线
   与内容区分开，标题用主题文字色。 */
.notes-header {
  padding: 12px 16px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

/* 关闭键也要跟着走主题。头部原来是白字，而按钮底色由全局 button 规则给
   （var(--guaxi-panel-bg)），叠在饱和色头部上就是"浅底白字"；暗色主题下同样对不上。 */
.notes-header .close-btn,
.assistant-header .close-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  box-shadow: none;
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}

.notes-header .close-btn:hover,
.assistant-header .close-btn:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
  transform: none;
  box-shadow: none;
}

.notes-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.notes-content {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}

.assistant-panel {
  position: fixed;
  top: 0;
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 101;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--guaxi-border);
}

.assistant-panel-visible {
  right: 0;
}

.assistant-header {
  padding: 12px 16px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.assistant-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

.assistant-content {
  flex: 1;
  min-height: 0;
  background-color: var(--guaxi-content-bg);
}

.note-content {
  padding: 12px 16px;
  min-height: 60px;
  outline: none;
}

.note-content :deep(a),
.note-content :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .note-content :deep(a),
.dark-mode .note-content :deep(a:visited),
.dark .note-content :deep(a),
.dark .note-content :deep(a:visited),
:root[data-guaxi-theme='dark'] .note-content :deep(a),
:root[data-guaxi-theme='dark'] .note-content :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.note-content :deep(a:hover) {
  opacity: 0.92;
}

.note-content.saved-feedback {
  animation: savedPulse 1s;
}

@keyframes savedPulse {
  0% {
    box-shadow: 0 0 0 0 rgba(77, 171, 247, 0.4);
  }

  70% {
    box-shadow: 0 0 0 10px rgba(77, 171, 247, 0);
  }

  100% {
    box-shadow: 0 0 0 0 rgba(77, 171, 247, 0);
  }
}

.note-content[contenteditable="true"] {
  padding: 12px;
  border-radius: 8px;
  background-color: rgba(3, 101, 167, 0.05);
}

.composite-note-content[contenteditable="true"] {
  min-height: 100px;
  padding: 12px;
}

.note-card {
  transition: all 0.2s var(--n-bezier);
  cursor: pointer;
}

.note-card:hover {
  box-shadow: 0 1px 2px -2px rgba(0, 0, 0, 0.08),
    0 3px 6px 0 rgba(0, 0, 0, 0.06),
    0 5px 12px 4px rgba(0, 0, 0, 0.04);
}

.add-card-container {
  margin-top: 8px;
  margin-bottom: 16px;
}

.close-btn {
  background: none;
  border: none;
  color: white;
  cursor: pointer;
  padding: 4px;
  border-radius: 50%;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background-color: rgba(255, 255, 255, 0.2);
  transform: rotate(90deg);
}

.note-added-bubble {
  position: fixed;
  top: 70px;
  right: 20px;
  background-color: #27ae60;
  color: white;
  padding: 10px 20px;
  border-radius: 20px;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
  z-index: 1000;
  display: flex;
  align-items: center;
  gap: 10px;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.composite-note-content {
  padding: 12px;
}

.note-segment {
  margin-bottom: 16px;
  padding-bottom: 16px;
  border-bottom: 1px dashed #e0e0e0;
}

.text-segment {
  font-size: 14px;
  line-height: 1.6;
}

.code-segment {
  background: #ffffff;
  padding: 12px;
  border-radius: 6px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  overflow-x: auto;
}

.image-segment {
  text-align: center;
}

.image-segment img {
  max-width: 100%;
  border-radius: 4px;
}

.diagram-segment {
  background: white;
  padding: 16px;
  border-radius: 8px;
  overflow-x: auto;
}

.math-segment {
  padding: 8px;
  overflow-x: auto;
}

.card-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.n-button--tiny .n-icon {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 按钮悬停效果 */
.n-button--tiny:hover {
  transform: scale(1.1);
  transition: transform 0.2s ease;
}

/* 删除按钮特殊样式 */
.n-button--tiny.text-type[data-v-xxxxxx]:hover {
  color: #ff6b6b;
  background-color: rgba(255, 107, 107, 0.1);
}


.composite-note-editor {
  box-sizing: border-box;
  width: 100%;
  max-width: 100%;
  min-height: 180px;
  padding: 14px 16px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  outline: none;
  resize: vertical;
  background-color: var(--guaxi-content-bg, var(--guaxi-panel-bg));
  color: var(--guaxi-text-primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 15px;
  line-height: 1.65;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
}

.composite-note-editor:focus {
  border-color: rgba(77, 171, 247, 0.9);
  box-shadow: 0 0 0 3px rgba(77, 171, 247, 0.18);
}

.dark .composite-note-editor {
  background-color: rgba(255, 255, 255, 0.04);
}

.translation-error-message {
  color: #ff6b6b;
  font-size: 0.9em;
  margin-bottom: 8px;
  font-weight: bold;
}

.original-fallback {
  opacity: 0.7;
  border-top: 1px dashed #ccc;
  padding-top: 8px;
  margin-top: 8px;
}

.translation-error {
  color: #ff6b6b;
  font-size: 0.9em;
  font-style: italic;
}

.translation-error-marker {
  display: inline-block;
  font-size: 0.8em;
  background-color: #ffebee;
  color: #ff6b6b;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
}

.translating-indicator {
  display: inline-block;
  width: 1em;
  text-align: left;
}

.translating {
  position: relative;
  opacity: 0.7;
}

.translating::after {
  content: "";
  position: absolute;
  bottom: -2px;
  left: 0;
  right: 0;
  height: 2px;
  background-color: var(--n-color-primary, #4dabf7);
  animation: translating-progress 2s infinite;
}

@keyframes translating-progress {
  0% {
    transform: translateX(-100%);
  }

  100% {
    transform: translateX(100%);
  }
}

.translation-skipped-marker {
  display: inline-block;
  font-size: 0.8em;
  background-color: #f0f0f0;
  color: #999;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
}

.markdown-content :deep(.mermaid) {
  background-color: #ffffff;
  border: 1px solid var(--guaxi-border, #e1e4e8);
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  overflow: auto;
  color: #1f2328;
}
</style>
