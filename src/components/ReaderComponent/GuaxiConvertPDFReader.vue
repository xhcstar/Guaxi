<template>
  <div class="pdf-viewer-container">
    <!-- 顶部控制栏 -->
    <div class="pdf-controls">
      <div class="page-info">
        <span>第 {{ currentPage }} 页 / 共 {{ totalPages }} 页（{{ progressPercent }}%）</span>
        <span v-if="completedReadingMs">已完成：{{ formatDuration(completedReadingMs) }}</span>
      </div>

      <div class="page-navigation">
        <button @click="prevPage" :disabled="currentPage <= 1" title="上一页">
          <n-icon :component="ArrowBackOutline" />
          <span class="btn-text">上一页</span>
        </button>

        <input type="number" v-model.number="jumpPage" min="1" :max="totalPages" @keyup.enter="goToPage"
          style="font-size: 12px;">

        <button @click="nextPage" :disabled="currentPage >= totalPages" title="下一页">
          <span class="btn-text">下一页</span>
          <n-icon :component="ArrowForwardOutline" />
        </button>
      </div>

      <div class="right-controls">
        <button class="rotate-left" @click="rotate(-90)" title="向左旋转90度">
          <n-icon :component="RefreshCircleOutline" :rotate="90" />
          <span class="btn-text">左旋</span>
        </button>
        <button class="rotate-right" @click="rotate(90)" title="向右旋转90度">
          <n-icon :component="RefreshCircleOutline" :rotate="-90" />
          <span class="btn-text">右旋</span>
        </button>
        <button class="fit-width" @click="fitToWidth" title="将页面调整为适合宽度">
          <n-icon :component="ExpandOutline" />
          <span class="btn-text">适合宽度</span>
        </button>
        <button class="zoom-out" @click="zoomOut" :disabled="zoomLevel <= 0.5" title="缩小">
          <n-icon :component="RemoveOutline" />
        </button>
        <span class="zoom-level">{{ Math.round(zoomLevel * 100) }}%</span>
        <button class="zoom-in" @click="zoomIn" :disabled="zoomLevel >= 2" title="放大">
          <n-icon :component="AddOutline" />
        </button>

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

        <!-- 显示/隐藏笔记按钮 -->
        <button class="toggle-notes" @click="toggleNotesPanel" :title="showNotes ? '隐藏笔记' : '显示笔记'">
          <n-icon :component="DocumentTextOutline" />
        </button>

        <button class="toggle-translate" @click="toggleTranslate" :title="showTranslation ? '隐藏译文' : '显示译文'"
          :class="{ 'translating': isTranslating }">
          <n-icon :component="LanguageOutline" />
        </button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="pdf-main-content">
      <div v-if="pdfSearchVisible || mdSearchVisible" class="inpage-search-bar"
        :class="pdfSearchVisible ? 'inpage-search-bar-pdf' : 'inpage-search-bar-md'" @mousedown.stop @click.stop
        @dblclick.stop>
        <input v-if="pdfSearchVisible" ref="pdfSearchInput" class="inpage-search-input" v-model="pdfSearchQuery"
          placeholder="PDF搜索" @input="handlePdfSearchInput" @keydown.enter.prevent="gotoNextPdfSearch"
          @keydown.shift.enter.prevent="gotoPrevPdfSearch" @keydown.esc.prevent="closePdfSearch" />
        <input v-else ref="mdSearchInput" class="inpage-search-input" v-model="mdSearchQuery" placeholder="Markdown搜索"
          @input="applyMarkdownSearch" @keydown.enter.prevent="gotoNextMarkdownSearch"
          @keydown.shift.enter.prevent="gotoPrevMarkdownSearch" @keydown.esc.prevent="closeMarkdownSearch" />
        <span class="inpage-search-count">{{ pdfSearchVisible ? pdfSearchCountText : mdSearchCountText }}</span>
        <button v-if="pdfSearchVisible" class="inpage-search-btn" type="button" title="跳转"
          @click="jumpToActivePdfSearchResult">
          <n-icon :component="SearchOutline" />
        </button>
        <button class="inpage-search-btn" type="button" title="上一个"
          @click="pdfSearchVisible ? gotoPrevPdfSearch() : gotoPrevMarkdownSearch()">
          <n-icon :component="ArrowBackOutline" />
        </button>
        <button class="inpage-search-btn" type="button" title="下一个"
          @click="pdfSearchVisible ? gotoNextPdfSearch() : gotoNextMarkdownSearch()">
          <n-icon :component="ArrowForwardOutline" />
        </button>
        <button class="inpage-search-btn" type="button" title="关闭"
          @click="pdfSearchVisible ? closePdfSearch() : closeMarkdownSearch()">
          <n-icon :component="CloseOutline" />
        </button>
      </div>

      <div class="pdf-left-wrapper" v-show="viewMode !== 'full-right' || showOutline || pdfSearchVisible" :style="{ width: leftPanelWidth }">
        <div v-if="showOutline" class="outline-panel-wrapper outline-panel-wrapper-left" @click.stop>
          <GuaxiOutline />
        </div>

        <!-- 修改PDF渲染区域 -->
        <div class="pdf-render-panel" v-show="viewMode !== 'full-right'" ref="pdfPanel" @wheel="handleWheelZoom" @mousedown="startDrag"
          @mousemove="handleDrag" @mouseup="handlePdfPanelMouseUp" @mouseleave="endDrag" @mouseenter="setScrollTargetFromEvent"
          :style="{ cursor: isDragging ? 'grabbing' : 'text' }">

          <div class="pdf-image-container">
            <div class="pdf-page-container" ref="pdfPageContainer" v-show="!!currentPageData"
              :class="{ 'pdf-editor-active': annotationMode !== 'none' }" @click="handlePdfPageClick">
              <canvas ref="pdfCanvas" class="rendered-pdf-canvas"></canvas>
              <div ref="pdfHighlightLayer" class="pdf-highlight-layer"></div>
              <div ref="pdfAnnotationLayer" class="pdf-annotation-layer annotationLayer"></div>
              <div ref="pdfEditorLayer" class="pdf-editor-layer annotationEditorLayer"></div>
              <div ref="pdfTextLayer" class="pdf-text-layer textLayer"></div>
            </div>

            <div v-if="!currentPageData" class="pdf-loading-message">
              正在加载PDF页面...
            </div>
          </div>

          <div class="pdf-annotation-toolbar-wrap" @mousedown.stop @click.stop @dblclick.stop>
            <div class="pdf-annotation-toolbar">
              <button class="pdf-annotation-tool-btn" type="button" title="高亮（选中文本后点击；再次点击可取消已有高亮）" @click="addPdfHighlightFromSelection">
                <n-icon :component="ColorWandOutline" />
              </button>
              <button class="pdf-annotation-tool-btn" type="button" :disabled="!pdfActiveAnnotationId" title="取消高亮（删除当前高亮）" @click="deleteActivePdfHighlight">
                <n-icon :component="TrashOutline" />
              </button>
              <button class="pdf-annotation-tool-btn" type="button" title="添加标注（高亮+写标注）" @click="addPdfAnnotationFromSelection">
                <n-icon :component="ChatboxEllipsesOutline" />
              </button>
              <button class="pdf-annotation-tool-btn" type="button" title="保存到原PDF" @click="savePdfAnnotationsToOriginal">
                <n-icon :component="SaveOutline" />
              </button>
            </div>
          </div>
        </div>

        <div v-if="pdfAnnotationPanelVisible" class="pdf-annotations-panel" @mousedown.stop @click.stop @dblclick.stop>
          <div class="pdf-annotations-header">
            <div class="pdf-annotations-title">PDF 标注</div>
            <button class="pdf-annotations-close" type="button" title="关闭" @click="exitPdfAnnotationMode">
              <n-icon :component="CloseOutline" />
            </button>
          </div>

          <div class="pdf-annotations-list">
            <div v-if="!pdfAnnotations.length" class="pdf-annotations-empty">暂无标注</div>
            <div v-for="it in pdfAnnotations" :key="it.id" class="pdf-annotation-item"
              :class="{ 'pdf-annotation-item-active': it.id === pdfActiveAnnotationId }" @click="activatePdfAnnotation(it.id)">
              <div class="pdf-annotation-item-main">
                <div class="pdf-annotation-item-meta">第 {{ it.page }} 页</div>
                <div class="pdf-annotation-item-text">{{ it.selectionText }}</div>

                <div v-if="it.note && !it.editing" class="pdf-annotation-item-note">{{ it.note }}</div>

                <div v-if="it.editing" class="pdf-annotation-item-editor" @click.stop>
                  <textarea v-model="it.note" class="pdf-annotation-note-input" placeholder="输入标注内容..."></textarea>
                  <button class="pdf-annotation-note-save" type="button" title="保存标注" @click.stop="savePdfAnnotationNote(it.id)">
                    <n-icon :component="SaveOutline" />
                  </button>
                </div>
              </div>
              <button class="pdf-annotation-item-delete" type="button" title="删除" @click.stop="deletePdfAnnotation(it.id)">
                <n-icon :component="TrashOutline" />
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- 可拖动的中间线 -->
      <div class="resize-handle" v-show="viewMode === 'split'" @mousedown="startResize"></div>

      <!-- 右侧Markdown内容 -->
      <div class="markdown-panel" v-show="viewMode !== 'full-left' || mdSearchVisible" @click="handleMarkdownClickAlt" @dblclick="handleMarkdownClick" @mousemove="handleMarkdownHover"
        @mouseleave="handleMarkdownLeave" @mouseenter="setScrollTargetFromEvent"
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
                <template v-else-if="line.translationError">
                  <div class="translation-error-message">
                    {{ line.translationError }}
                  </div>
                  <!-- <div v-html="line.html" class="original-fallback"></div> -->
                </template>
                <template v-else>
                  <div v-html="line.translation"></div>
                </template>
              </div>
            </div>
          </template>
        </div>

        <div v-if="selectionToolbarVisible" class="selection-toolbar"
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
            <button class="selection-tool-btn" type="button" title="插入笔记" :disabled="selectionBusy || !selectionInsertText"
              @click="insertSelectionResult">
              <n-icon :component="AddOutline" />
            </button>
          </div>

          <div v-if="selectionBusy" class="selection-toolbar-body selection-toolbar-loading">正在翻译...</div>
          <div v-else-if="selectionInsertText" class="selection-toolbar-body" v-html="selectionPreviewHtml"></div>
        </div>
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
      <div class="notes-content" @mouseenter="setScrollTargetFromEvent">
        <n-card v-for="(card, index) in noteCards" :key="card.id" class="note-card" size="small" :title="card.title"
          :bordered="true" :segmented="{
            content: true,
            footer: 'soft'
          }" header-style="padding: 12px 16px;" content-style="padding: 0;">
          <template #header-extra>
            <div class="card-actions" v-if="activeCardIndex === index">
              <n-button size="tiny" type="primary" @click.stop="saveCardEdit(index)" title="保存">
                <n-icon :component="SaveOutline" size="12" />
              </n-button>
              <n-button size="tiny" text @click.stop="exitEditMode(index)" title="退出编辑">
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
          <div v-else-if="card.type === 'composite' && activeCardIndex !== index" class="composite-note-content"
            @click="enterEditMode(index)">
            <div v-for="(segment, segIndex) in compositeSegments[index]" :key="segIndex" class="note-segment"
              :class="getSegmentClass(segment)">
              <div v-html="segment.content"></div>
            </div>
          </div>

          <div v-else class="note-content">
            <textarea class="composite-note-editor" @input="markEditorDirty(index)" @keydown.meta.s.prevent="saveCardEdit(index)"
              @keydown.ctrl.s.prevent="saveCardEdit(index)" :ref="el => setContentEditableRef(index, el)"></textarea>
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
import { ref, computed, watchEffect, onMounted, onBeforeUnmount, watch, nextTick, h } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join, dirname } from '@tauri-apps/api/path'
import { convertFileSrc } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { NCard, NButton, NIcon, NText, useMessage, NDropdown } from 'naive-ui'
import usePdfRenderer from './usePdfRenderer'
import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import GuaxiOutline from '../HomeViewComponent/GuaxiOutline.vue'
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
  ColorWandOutline,
  ChatboxEllipsesOutline
} from '@vicons/ionicons5'

// 1. 组件属性定义
const props = defineProps({
  filePath: {
    type: String,
    required: true,
  },
  fileType: {
    type: String,
    required: true,
    validator: (value) => ['pdf'].includes(value)
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

// 2. 初始化状态和工具
const message = useMessage()
const store = repoStore()
const { librariesRoot } = storeToRefs(store)
const outlineStore = useOutlineStore()
const md = new MarkdownIt()

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
    if (se) lastScrollTarget.value = se
    return
  }
  if (t && t instanceof HTMLElement && isScrollableElement(t)) {
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
    const t = lastScrollTarget.value
    const el = t && t instanceof HTMLElement ? t : null
    const inPdf = el && (el.closest('.pdf-left-wrapper') || el.closest('.pdf-render-panel'))
    if (inPdf) {
      openPdfSearch()
    } else {
      openMarkdownSearch()
    }
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
const noteHtmls = ref([]);
const compositeSegments = ref([]);
let notesRenderToken = 0;

const selectionToolbarVisible = ref(false)
const selectionToolbarTop = ref(0)
const selectionToolbarLeft = ref(0)
const selectionText = ref('')
const selectionBusy = ref(false)
const selectionInsertText = ref('')

const selectionPreviewHtml = computed(() => {
  const v = selectionInsertText.value.trim()
  return v ? md.render(v) : ''
})

const pdfSearchVisible = ref(false)
const pdfSearchQuery = ref('')
const pdfSearchMatches = ref([])
const pdfSearchActiveIndex = ref(-1)
const pdfSearchInput = ref(null)

const pdfFullSearchBusy = ref(false)
const pdfFullSearchResults = ref([])
const pdfFullSearchActiveIndex = ref(-1)
let pdfFullSearchTimer = 0

const mdSearchVisible = ref(false)
const mdSearchQuery = ref('')
const mdSearchMatches = ref([])
const mdSearchActiveIndex = ref(-1)
const mdSearchInput = ref(null)

const pdfSearchCountText = computed(() => {
  if (pdfFullSearchBusy.value) return '搜索中…'

  const totalResults = pdfFullSearchResults.value.length
  if (totalResults) {
    const i = Math.max(0, pdfFullSearchActiveIndex.value)
    return `${Math.min(totalResults, i + 1)}/${totalResults}`
  }

  const total = pdfSearchMatches.value.length
  if (!total) return '0/0'
  return `${Math.max(0, pdfSearchActiveIndex.value) + 1}/${total}`
})

const mdSearchCountText = computed(() => {
  const total = mdSearchMatches.value.length
  if (!total) return '0/0'
  return `${Math.max(0, mdSearchActiveIndex.value) + 1}/${total}`
})

// 3. PDF渲染相关
const {
  renderMode,
  loadPage,
  renderContent,
  textLayer,
  isLoading,
  error,
  currentPageData,
  getOutlineFlat,
  searchText,
  ensureAnnotationEditorUi,
  setAnnotationEditorMode,
  saveDocumentBase64,
  getAnnotationEditorIdsForPage,
  removeAnnotationEditorById,
  highlightSelectionAsAnnotation
} = usePdfRenderer(props);

// 4. 视图状态
const zoomLevel = ref(1);
const rotation = ref(0);
const pdfPanel = ref(null);
const pdfCanvas = ref(null);
const pdfAnnotationLayer = ref(null);
const pdfTextLayer = ref(null);
const isResizing = ref(false);
const startX = ref(0);
const startLeftWidth = ref(0);
const leftPanelWidth = ref('50%');
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const pdfOutlineLoadedFor = ref('')

const readerStorageKey = computed(() => {
  const baseId = props.importFileId != null && props.importFileId !== 0
    ? `id:${props.importFileId}`
    : `path:${props.filePath || props.fileName || ''}`;
  const safeId = String(baseId).replace(/\\/g, '/');
  return `guaxi_reader_state_v1:${props.library}:${props.fileType}:${safeId}`;
});

const pendingRestorePage = ref(null);
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
  const normalizePdfAnnotations = (v) => {
    if (!Array.isArray(v)) return [];
    return v
      .map((it) => {
        const id = typeof it?.id === 'string' ? it.id : '';
        const page = Math.max(1, Number(it?.page) || 1);
        const selectionText = typeof it?.selectionText === 'string' ? it.selectionText : '';
        const note = typeof it?.note === 'string' ? it.note : '';

        const ax = Number(it?.anchor?.x);
        const ay = Number(it?.anchor?.y);
        const anchor = Number.isFinite(ax) && Number.isFinite(ay)
          ? { x: Math.max(0, Math.min(1, ax)), y: Math.max(0, Math.min(1, ay)) }
          : null;

        const color = typeof it?.color === 'string' && it.color.trim()
          ? it.color.trim()
          : 'rgba(64, 196, 255, 0.32)';

        const rects = Array.isArray(it?.rects)
          ? it.rects
            .map((r) => {
              const x = Number(r?.x);
              const y = Number(r?.y);
              const w = Number(r?.w);
              const h = Number(r?.h);
              if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(w) || !Number.isFinite(h)) return null;
              return {
                x: Math.max(0, Math.min(1, x)),
                y: Math.max(0, Math.min(1, y)),
                w: Math.max(0, Math.min(1, w)),
                h: Math.max(0, Math.min(1, h)),
              };
            })
            .filter(Boolean)
            .filter((r) => r.w > 0.002 && r.h > 0.002)
            .slice(0, 200)
          : [];

        const editorId = typeof it?.editorId === 'string' ? it.editorId : '';
        const noteEditorId = typeof it?.noteEditorId === 'string' ? it.noteEditorId : '';

        if (!id || !selectionText) return null;
        return {
          id,
          page,
          selectionText: selectionText.slice(0, 800),
          note: note.slice(0, 2000),
          anchor,
          rects,
          color,
          editorId,
          noteEditorId,
          editing: false,
        };
      })
      .filter(Boolean);
  };

  try {
    const raw = localStorage.getItem(readerStorageKey.value);
    if (!raw) {
      return { lastPage: 1, bookmarks: [], totalReadingMs: 0, completedAt: null, completedReadingMs: null, pdfAnnotations: [] };
    }
    const parsed = JSON.parse(raw);
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
      pdfAnnotations: normalizePdfAnnotations(parsed.pdfAnnotations),
    };
  } catch {
    return { lastPage: 1, bookmarks: [], totalReadingMs: 0, completedAt: null, completedReadingMs: null, pdfAnnotations: [] };
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
        pdfAnnotations: Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [],
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

// 5. Markdown相关
const markdownContent = ref('')
const markdownLines = ref([]);
const markdownFilePath = ref('')

function generateOutlineFromMarkdown(content) {
  if (!content) {
    outlineStore.setOutline([]);
    return;
  }
  const tokens = md.parse(content, {});
  const result = [];
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1));
      const nextToken = tokens[idx + 1];
      if (nextToken && nextToken.type === 'inline') {
        result.push({
          level,
          text: nextToken.content,
          line: result.length
        });
      }
    }
  });
  outlineStore.setOutline(result);
}

function jumpToHeading(index) {
  if (!markdownContainer.value) return;
  const headings = markdownContainer.value.querySelectorAll('h1, h2, h3, h4, h5, h6');
  const target = headings[index];
  if (target) {
    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}

// 6. 笔记相关
const showNotes = ref(false)
const notesContent = ref('')
const showNoteBubble = ref(false)
const isFitting = false
const noteCards = ref([])
const activeCardIndex = ref(-1)
const isNotesReady = ref(false)
const notesFilePath = ref('')

// 7. 拖拽和动画相关
const isDragging = ref(false)
const startDragPos = ref({ x: 0, y: 0 })
const startScrollPos = ref({ left: 0, top: 0 })
const velocity = ref({ x: 0, y: 0 })
const lastDragTime = ref(0)
const animationFrameId = ref(null)
const isDecelerating = ref(false)
let dragGlobalListenersActive = false

// 8. 翻译相关
const showTranslation = ref(false)
const isTranslating = ref(false)

// 9. Alt键收集器状态
const altKeyCollector = ref({
  isActive: false,
  currentCard: null,
  timer: null
})

// 10. 工具函数
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
        console.warn(`路径转换失败: ${src}`, error);
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
      theme: 'default'
    });
  } catch (e) {
    return `<div class="render-error">${e.message}</div>`;
  }
};

async function convertPdfRenderMarkdown(content, options = {}) {
  const processedContent = await processImages(content, options.libraryBasePath);
  try {
    let html = await renderMarkdown(processedContent, {
      preserveOriginal: true,
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: 'default'
    });

    // 确保DOM更新后添加按钮
    nextTick(() => {
      if (markdownContainer.value) {
        addCopyButtons(markdownContainer.value);
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
            // 新增：默认不跳过翻译
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
          // 添加MathJax相关标签
          'mjx-container', 'mjx-math', 'mjx-assistive-mml',
          // 修改代码高亮相关标签 - 移除无效选择器
          'div.hljs', 'code.hljs',
          // 添加所有hljs开头的类名元素
          '[class^="hljs-"]', '[class*=" hljs-"]'];

        if (keepWholeTags.some(tag => node.matches(tag))) {
          const outerHTML = node.outerHTML;

          // 新增：判断是否需要跳过翻译
          const skipTranslation = shouldSkipTranslation(node);

          result.push({
            raw: outerHTML,
            html: outerHTML,
            // 新增：标记是否需要跳过翻译
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
    console.error('[ERROR] 渲染失败:', error);
    return [{
      raw: content,
      html: `<div class="render-error">${error.message}</div>`,
      // 新增：错误内容也不翻译
      skipTranslation: true
    }];
  }
}

// 新增：判断节点是否需要跳过翻译
function shouldSkipTranslation(node) {
  // 首先检查 node 是否有效
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
    console.error('Error checking node:', error);
    return false;
  }
}

// 添加安全编码函数
function safeEncode(content) {
  if (!content) return '';

  // 首先确保是字符串
  content = String(content);

  // 替换保留字符
  return encodeURIComponent(content)
    .replace(/'/g, "%27")     // 单引号
    .replace(/"/g, "%22")     // 双引号
    .replace(/</g, "%3C")     // 小于号
    .replace(/>/g, "%3E")     // 大于号
    .replace(/&/g, "%26")     // &符号
    .replace(/\//g, "%2F")    // 斜杠
    .replace(/\\/g, "%5C")    // 反斜杠
    .replace(/\$/g, "%24")    // $符号（数学公式重要字符）
    .replace(/{/g, "%7B")     // 左大括号
    .replace(/}/g, "%7D")     // 右大括号
    .replace(/\^/g, "%5E")    // 脱字符
    .replace(/_/g, "%5F");    // 下划线
}

// 11. 笔记相关函数
function determineNoteType(target) {
  console.log('determineNoteType called with target:', target);

  // 1. 优先检查数学公式
  if (target.closest('mjx-container, mjx-math, .math-inline, .math-block')) {
    return 'math';
  }

  // 2. 检查Mermaid图表
  const mermaidContainer = target.closest('.mermaid, [data-processed="true"]');
  if (mermaidContainer) {
    // 确认是Mermaid而非普通代码块
    if (mermaidContainer.querySelector('svg') || mermaidContainer.classList.contains('mermaid')) {
      return 'diagram';
    }
  }

  // 3. 检查代码块
  const pre = target.closest('pre');
  if (pre) {
    // 排除图表
    if (!pre.classList.contains('mermaid')) {
      // 检测标准代码结构或高亮类
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
    // 解码URL并提取实际路径
    const rawSrc = imgElement.src;
    // console.log('原始图片路径：', rawSrc);
    let filePath = '';

    if (rawSrc.startsWith('http://asset.localhost/')) {
      filePath = decodeURIComponent(rawSrc.replace('http://asset.localhost/', ''));
      // console.log('处理本地图片路径', rawSrc, filePath);
    } else if (rawSrc.startsWith('file://')) {
      filePath = decodeURIComponent(new URL(rawSrc).pathname);
      // console.log('处理file协议图片路径', rawSrc, filePath);
    } else {
      // console.warn('非本地图片，无需处理:', rawSrc);
      return rawSrc; // 非本地图片直接返回原URL
    }

    const osPlatform = await platform();
    // 标准化Windows路径
    if (osPlatform === 'win32') {
      filePath = filePath.replace(/^\/([a-zA-Z]:)/, '$1');
    }

    // 准备目标路径
    const imageDir = await getImagesDirectory();
    const ext = filePath.split('.').pop().toLowerCase();
    const validExts = ['jpg', 'jpeg', 'png', 'gif', 'webp'];
    const finalExt = validExts.includes(ext) ? ext : 'png';
    const fileName = `note-img-${Date.now()}.${finalExt}`;

    // console.log('准备复制图片:', {
    //   srcPath: filePath,
    //   destDir: imageDir,
    //   fileName: fileName
    // });

    // 调用后端复制图片
    const relativePath = await invoke('copy_image_file', {
      srcPath: filePath,
      destDir: imageDir,
      fileName: fileName
    });

    return `../images/${relativePath}`;
  } catch (error) {
    console.error('图片处理失败:', error);
    return imgElement.src; // 失败时返回原URL
  }
}

// 在handleMarkdownClick函数中添加获取原始Markdown的逻辑
const handleMarkdownClickAlt = async (e) => {
  if (!e?.altKey) return
  return handleMarkdownClick(e)
}

const handleMarkdownClick = async (e) => {
  // 1. 阻止默认行为（防止Alt+Click触发浏览器默认动作）
  e.preventDefault();

  // 2. 获取关键状态
  const isAltPressed = e.altKey;
  const rawTarget = e.target;
  const hoveredElement = document.querySelector('.markdown-content .hovered');
  const target = hoveredElement && rawTarget instanceof Node && hoveredElement.contains(rawTarget)
    ? hoveredElement
    : rawTarget;
  const selection = window.getSelection();
  const selectedText = selection ? selection.toString().trim() : '';

  // 3. Alt键特殊处理（不再提前return）
  if (isAltPressed && !altKeyCollector.value.isActive) {
    startAltKeyCollection();
    // 继续执行后续逻辑，不return
  }

  // 4. 清除之前选中的元素
  document.querySelectorAll('.markdown-content .selected').forEach(el => {
    el.classList.remove('selected');
  });

  // 5. 元素类型判断
  let selectedElement = null;
  let message = '';
  let noteContent = '';
  let originalMarkdown = ''; // 新增：存储原始Markdown

  // 处理翻译内容（如果点击在翻译区域内）
  const translatedElement = target.closest('.translated-content');
  if (translatedElement) {
    // 获取最内层的div（实际包含翻译内容的元素）
    const contentDiv = translatedElement.querySelector('div');
    if (contentDiv) {
      selectedElement = translatedElement;
      noteContent = contentDiv.innerHTML; // 获取翻译内容的HTML
      message = '已选中译文';
      
      // 如果需要纯文本内容，可以使用:
      noteContent = contentDiv.textContent;
      originalMarkdown = noteContent;
    }
  }
  // 图片处理（现在支持Alt+Click）
  else if (target instanceof Element && (target.tagName === 'IMG' || target.closest('img') || target.querySelector('img'))) {
    const imgElement = target.tagName === 'IMG'
      ? target
      : (target.closest('img') || target.querySelector('img'));

    selectedElement = imgElement;
    try {
      const processedSrc = await processNoteImage(imgElement);
      message = '已选中图片';
      noteContent = `![图片](${processedSrc})`;
      originalMarkdown = noteContent; // 图片的原始Markdown就是noteContent
    } catch (error) {
      console.error('图片处理失败:', error);
      message = '图片处理失败，使用原始链接';
      noteContent = `![图片](${imgElement.src})`;
      originalMarkdown = noteContent;
    }
  }
  // 代码块
  else if (target.closest('pre, code, .code-block')) {
    const codeBlock = target.closest('pre, code, .code-block');
    selectedElement = codeBlock;

    // 克隆代码块节点以避免修改原始DOM
    const clonedBlock = codeBlock.cloneNode(true);

    const btn = clonedBlock.querySelector('.copy-button');
    if (btn) btn.remove();

    // 获取处理后的代码内容
    const codeContent = clonedBlock.textContent || '';
    message = `已选中代码块: ${codeContent.substring(0, 30)}${codeContent.length > 30 ? '...' : ''}`;
    console.log('选中的代码块:', codeBlock);

    // 获取原始Markdown（从data-md-original属性）
    originalMarkdown = codeBlock.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`\n${codeContent}\n\`\`\``;
    console.log('代码块原始Markdown:', originalMarkdown);

    originalMarkdown = decodeURIComponent(originalMarkdown);

    noteContent = originalMarkdown;
  }
  // 数学公式
  else if (target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered')) {
    // 获取最接近的数学元素（可能是容器或SVG）
    const mathElement = target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered');
    selectedElement = mathElement;

    // 调试信息
    // console.log('点击的数学元素:', mathElement);

    // 查找原始LaTeX内容（按优先级）
    let original = mathElement.getAttribute('data-md-original');
    if (!original && mathElement.closest('[data-md-original]')) {
      original = mathElement.closest('[data-md-original]').getAttribute('data-md-original');
    }
    if (!original && mathElement.querySelector('[data-md-original]')) {
      original = mathElement.querySelector('[data-md-original]').getAttribute('data-md-original');
    }

    // 安全解码原始内容
    let decodedOriginal = original;
    try {
      if (decodedOriginal) {
        decodedOriginal = decodeURIComponent(decodedOriginal);
      } else {
        console.warn('未找到data-md-original属性，尝试从元素内容重建');
        const isBlock = mathElement.classList.contains('math-block') ||
          mathElement.closest('.math-block') ||
          mathElement.tagName === 'MJX-BLOCK';
        const content = mathElement.textContent || '';
        decodedOriginal = isBlock ? `$$${content}$$` : `$${content}$`;
      }
    } catch (err) {
      console.warn('解码公式失败:', err);
      decodedOriginal = original || '无法获取公式内容';
    }

    // 构建笔记数据
    noteContent = decodedOriginal;
    originalMarkdown = decodedOriginal;

    // 确定是否为块级公式
    const isBlockFormula = decodedOriginal.startsWith('$$') &&
      decodedOriginal.endsWith('$$') &&
      decodedOriginal.length > 4;

    message = `已选中${isBlockFormula ? '块级' : '行内'}公式: ${decodedOriginal.substring(0, 30)}${decodedOriginal.length > 30 ? '...' : ''}`;

    // console.log('数学公式处理结果:', {
    // noteContent,
    // originalMarkdown,
    // isBlockFormula
    // });
  }
  // mermaid图表
  else if (target.closest('.mermaid, svg')) {
    selectedElement = target.closest('.mermaid, svg');
    const mermaidContent = selectedElement.querySelector('.mermaid')?.textContent || selectedElement.textContent || '';
    message = '已选中图表';

    // 获取原始Markdown（从data-md-original属性）
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`mermaid\n${mermaidContent}\n\`\`\``;

    // 解码原始Markdown
    originalMarkdown = decodeURIComponent(originalMarkdown);

    noteContent = originalMarkdown;
  }
  // 文本选择
  else if (selectedText && selection.rangeCount > 0) {
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;

    // 获取原始Markdown（从data-md-original属性）
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      selectedText;

    // 解码原始Markdown
    originalMarkdown = decodeURIComponent(originalMarkdown);
    console.log('原始Markdown:', originalMarkdown);

    noteContent = originalMarkdown;
  }
  // 段落点击
  else {
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      const textContent = paragraph.textContent || '';
      message = `已选中段落: ${textContent.substring(0, 30)}${textContent.length > 30 ? '...' : ''}`;

      // 获取原始Markdown（从data-md-original属性）
      originalMarkdown = paragraph.getAttribute('data-md-original') ||
        textContent;

      console.log('段落原始Markdown:', originalMarkdown);

      // 解码原始Markdown
      originalMarkdown = decodeURIComponent(originalMarkdown);

      noteContent = originalMarkdown;
    }
  }

  // 6. 应用选中效果和笔记
  if (selectedElement) {
    const hasContent = (originalMarkdown || noteContent || '').trim();
    if (!hasContent) {
      showSelectionMessage('未选中有效内容');
      return;
    }

    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    const noteType = determineNoteType(target);
    const noteData = {
      type: noteType,
      content: noteContent, // 使用原始Markdown作为内容
      originalMarkdown: originalMarkdown // 新增：存储原始Markdown
    };

    console.log('确定的笔记类型:', noteType);
    console.log('笔记数据:', noteData);

    // 7. 区分复合笔记和新建笔记
    if (altKeyCollector.value.isActive) {
      appendToCurrentNote(noteData);
    } else {
      // 新建笔记
      await addToNotes(noteData);
    }
  }

  // 8. Alt键释放检测
  if (!isAltPressed && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

// 开始Alt键收集模式
const startAltKeyCollection = () => {
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  altKeyCollector.value.isActive = true;

  // 创建新的复合笔记卡片
  const newCard = {
    id: Date.now().toString(),
    type: 'composite',
    title: '复合笔记 (0项)',
    content: '',
    items: [], // 初始化空数组
    timestamp: new Date().toISOString()
  };

  noteCards.value.push(newCard);
  activeCardIndex.value = -1;
  altKeyCollector.value.currentCard = newCard;

  // 设置30秒后自动结束收集
  altKeyCollector.value.timer = setTimeout(() => {
    if (altKeyCollector.value.isActive) endAltKeyCollection();
  }, 30000);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
  });

  message.info('已进入多选模式, 松开Alt键结束收集', { duration: 3000 });
};

// 结束Alt键收集模式
const endAltKeyCollection = () => {
  if (!altKeyCollector.value.isActive) return;

  // 清除定时器
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  // 如果有收集内容，则添加到笔记
  if (altKeyCollector.value.currentCard &&
    (altKeyCollector.value.currentCard.content || altKeyCollector.value.currentCard.originalMarkdown)) {
    // 确保卡片已经存在于noteCards中（因为我们在start时已经添加）
    const existingIndex = noteCards.value.findIndex(card => card.id === altKeyCollector.value.currentCard.id);
    if (existingIndex === -1) {
      noteCards.value.push(altKeyCollector.value.currentCard);
    }
    activeCardIndex.value = -1;
    updateBackendNotes('add', altKeyCollector.value.currentCard);

    nextTick(() => {
      const cards = document.querySelectorAll('.n-card');
      if (cards.length > 0) {
        cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
      }
    });
  }

  // 重置状态
  altKeyCollector.value.isActive = false;
  altKeyCollector.value.currentCard = null;

  message.success('多选模式已结束', { duration: 2000 });
};

/**
 * 解析复合笔记内容 - 直接从items数组中获取内容
 * @param {Array} items 笔记条目数组
 * @returns {Promise<Array>} 解析后的笔记片段数组
 */
async function parseCompositeContent(items) {
  if (!items || !Array.isArray(items)) {
    console.warn('无效的items参数:', items);
    return [];
  }

  try {
    // 并行处理所有items
    const segments = await Promise.all(
      items.map(async (item, index) => {
        // 使用item中的内容，优先使用original_markdown
        const content = item.original_markdown || item.originalMarkdown || item.content || '';

        // 渲染Markdown为HTML
        const html = await renderNoteContent(content);

        return {
          id: item.id || `${Date.now()}-${index}`,
          type: item.type || 'text', // 默认类型为text
          content: html,
          originalMarkdown: content,
          timestamp: item.timestamp || new Date().toISOString()
        };
      })
    );

    return segments;
  } catch (error) {
    console.error('解析复合笔记失败:', error);
    return [{
      type: 'error',
      content: `<div class="parse-error">解析失败: ${error.message}</div>`,
      originalMarkdown: '解析失败',
      timestamp: new Date().toISOString()
    }];
  }
}

// 获取片段样式类
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

  // 确保items数组存在
  if (!currentCard.items) {
    currentCard.items = [];
  }

  // 创建新条目
  const newItem = {
    id: Date.now().toString(),
    type: element.type,
    content: element.content || element.originalMarkdown,
    originalMarkdown: element.originalMarkdown || element.content,
    original_markdown: element.originalMarkdown || element.content,
    timestamp: new Date().toISOString()
  };

  // 添加到items数组
  currentCard.items.push(newItem);

  // 更新卡片内容（将所有条目内容合并，带编号）
  currentCard.content = currentCard.items.map((item, index) => {
    return `${index + 1}. ${item.originalMarkdown || item.content}`;
  }).join('\n\n');

  // 更新卡片类型和标题
  currentCard.type = 'composite';
  currentCard.title = `复合笔记 (${currentCard.items.length}项)`;
  currentCard.timestamp = new Date().toISOString();

  // 触发响应式更新
  noteCards.value = [...noteCards.value];

  // 显示追加成功的消息
  message.success(`已添加第 ${currentCard.items.length} 条内容`, { duration: 1500 });
};

// 15. UI辅助函数
const showSelectionMessage = (message) => {
  // 移除旧的消息
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

const translateSelection = async () => {
  const text = selectionText.value.trim()
  if (!text) return

  selectionBusy.value = true
  try {
    const response = await invoke('translate_text_free', { texts: [text] })
    const translated = response?.translated_texts?.[0] || ''
    if (!translated.trim()) {
      message.error('翻译结果为空')
      selectionInsertText.value = ''
      return
    }

    selectionInsertText.value = `**${text}**\n\n${translated}`
  } catch (error) {
    const msg = typeof error === 'object' ? error.message || error.toString() : String(error)
    message.error(`翻译失败: ${msg}`)
  } finally {
    selectionBusy.value = false
  }
}

const querySelection = () => {
  const text = selectionText.value.trim()
  if (!text) return

  const url = `https://www.bing.com/search?q=${encodeURIComponent(text)}`
  selectionInsertText.value = `[查询：${text}](${url})`
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

  const containers = [markdownContainer.value, pdfTextLayer.value, pdfPageContainer.value].filter((x) => x && x instanceof HTMLElement)
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

  selectionText.value = text

  const pad = 8
  const toolWidth = 140
  const toolHeight = 40

  let top = rect.top - toolHeight - 8
  if (top < pad) top = rect.bottom + 8

  let left = rect.left
  left = Math.max(pad, Math.min(left, window.innerWidth - toolWidth - pad))

  selectionToolbarTop.value = Math.round(top)
  selectionToolbarLeft.value = Math.round(left)
  selectionToolbarVisible.value = true
}

const handleGlobalMouseDown = (e) => {
  const t = e?.target
  if (!(t instanceof Element)) {
    closeSelectionToolbar()
    return
  }

  if (t.closest('.selection-toolbar')) return
  closeSelectionToolbar()
}

const updateActivePdfSearchMatch = (nextIndex) => {
  const total = pdfSearchMatches.value.length
  if (!total) return

  const normalized = ((nextIndex % total) + total) % total

  const prev = pdfSearchMatches.value[pdfSearchActiveIndex.value]
  if (prev) prev.classList.remove('inpage-match-active')

  pdfSearchActiveIndex.value = normalized

  const el = pdfSearchMatches.value[normalized]
  if (el) {
    el.classList.add('inpage-match-active')
    el.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'nearest' })
  }
}

const gotoNextPdfSearchMatch = () => {
  if (!pdfSearchMatches.value.length) return
  updateActivePdfSearchMatch(pdfSearchActiveIndex.value + 1)
}

const gotoPrevPdfSearchMatch = () => {
  if (!pdfSearchMatches.value.length) return
  updateActivePdfSearchMatch(pdfSearchActiveIndex.value - 1)
}

const updateActivePdfFullSearchResult = (nextIndex) => {
  const total = pdfFullSearchResults.value.length
  if (!total) {
    pdfFullSearchActiveIndex.value = -1
    return
  }
  const normalized = ((nextIndex % total) + total) % total
  pdfFullSearchActiveIndex.value = normalized
}

const jumpToActivePdfSearchResult = () => {
  if (!pdfFullSearchResults.value.length) return

  const i = pdfFullSearchActiveIndex.value >= 0 ? pdfFullSearchActiveIndex.value : 0
  const hit = pdfFullSearchResults.value[i]
  if (!hit?.page) return

  goToPage(hit.page)
}

const gotoNextPdfSearch = () => {
  if (pdfFullSearchResults.value.length) {
    updateActivePdfFullSearchResult(pdfFullSearchActiveIndex.value + 1)
    jumpToActivePdfSearchResult()
    return
  }
  gotoNextPdfSearchMatch()
}

const gotoPrevPdfSearch = () => {
  if (pdfFullSearchResults.value.length) {
    updateActivePdfFullSearchResult(pdfFullSearchActiveIndex.value - 1)
    jumpToActivePdfSearchResult()
    return
  }
  gotoPrevPdfSearchMatch()
}

const updateActiveMarkdownSearchMatch = (nextIndex) => {
  const total = mdSearchMatches.value.length
  if (!total) return

  const normalized = ((nextIndex % total) + total) % total

  const prev = mdSearchMatches.value[mdSearchActiveIndex.value]
  if (prev) prev.classList.remove('inpage-match-active')

  mdSearchActiveIndex.value = normalized

  const el = mdSearchMatches.value[normalized]
  if (el) {
    el.classList.add('inpage-match-active')
    el.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'nearest' })
  }
}

const gotoNextMarkdownSearch = () => {
  if (!mdSearchMatches.value.length) return
  updateActiveMarkdownSearchMatch(mdSearchActiveIndex.value + 1)
}

const gotoPrevMarkdownSearch = () => {
  if (!mdSearchMatches.value.length) return
  updateActiveMarkdownSearchMatch(mdSearchActiveIndex.value - 1)
}

const clearPdfSearchHighlights = () => {
  const layer = pdfTextLayer.value
  if (!layer) return

  const marks = layer.querySelectorAll('span.inpage-match, span.inpage-match-active')
  marks.forEach((el) => {
    el.classList.remove('inpage-match')
    el.classList.remove('inpage-match-active')
  })
}

const applyPdfSearch = () => {
  clearPdfSearchHighlights()

  pdfSearchMatches.value = []
  pdfSearchActiveIndex.value = -1

  const query = pdfSearchQuery.value.trim()
  if (!query) return

  const layer = pdfTextLayer.value
  if (!layer) return

  const qLower = query.toLowerCase()
  const spans = Array.from(layer.querySelectorAll('span'))

  spans.forEach((span) => {
    const t = String(span.textContent || '').trim()
    if (!t) return
    if (t.toLowerCase().includes(qLower)) {
      span.classList.add('inpage-match')
      pdfSearchMatches.value.push(span)
    }
  })

  if (pdfSearchMatches.value.length) {
    nextTick(() => updateActivePdfSearchMatch(0))
  }
}

const clearMarkdownSearchHighlights = () => {
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

const applyMarkdownSearch = () => {
  clearMarkdownSearchHighlights()

  mdSearchMatches.value = []
  mdSearchActiveIndex.value = -1

  const query = mdSearchQuery.value.trim()
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
      mdSearchMatches.value.push(span)

      last = idx + query.length
      idx = lower.indexOf(qLower, last)
    }

    if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)))

    const parent = textNode.parentNode
    if (parent) parent.replaceChild(frag, textNode)
  })

  if (mdSearchMatches.value.length) {
    nextTick(() => updateActiveMarkdownSearchMatch(0))
  }
}

const invokeWithTimeout = (p, ms) => {
  let t = 0
  const timeout = new Promise((_, reject) => {
    t = window.setTimeout(() => reject(new Error('timeout')), ms)
  })
  return Promise.race([p, timeout]).finally(() => {
    if (t) {
      clearTimeout(t)
    }
  })
}

const searchFullPdfDocument = () => {
  if (pdfFullSearchTimer) {
    clearTimeout(pdfFullSearchTimer)
    pdfFullSearchTimer = 0
  }

  const query = pdfSearchQuery.value.trim()
  if (!query) {
    pdfFullSearchBusy.value = false
    pdfFullSearchResults.value = []
    pdfFullSearchActiveIndex.value = -1
    return
  }

  pdfFullSearchTimer = window.setTimeout(async () => {
    try {
      pdfFullSearchBusy.value = true
      pdfFullSearchResults.value = []
      pdfFullSearchActiveIndex.value = -1

      const hits = typeof searchText === 'function' ? await searchText(query) : []
      pdfFullSearchResults.value = Array.isArray(hits) ? hits : []

      const currentIdx = pdfFullSearchResults.value.findIndex((m) => m?.page === currentPage.value)
      pdfFullSearchActiveIndex.value = currentIdx >= 0
        ? currentIdx
        : (pdfFullSearchResults.value.length ? 0 : -1)
    } catch {
      pdfFullSearchResults.value = []
      pdfFullSearchActiveIndex.value = -1
    } finally {
      pdfFullSearchBusy.value = false
    }
  }, 250)
}

const handlePdfSearchInput = () => {
  applyPdfSearch()
  searchFullPdfDocument()
}

const openPdfSearch = async () => {
  closeMarkdownSearch()
  pdfSearchVisible.value = true
  await nextTick()
  const input = pdfSearchInput.value
  if (input && input instanceof HTMLInputElement) {
    input.focus()
    input.select()
  }
  handlePdfSearchInput()
}

const closePdfSearch = () => {
  pdfSearchVisible.value = false
  pdfSearchQuery.value = ''

  if (pdfFullSearchTimer) {
    clearTimeout(pdfFullSearchTimer)
    pdfFullSearchTimer = 0
  }

  pdfFullSearchBusy.value = false
  pdfFullSearchResults.value = []
  pdfFullSearchActiveIndex.value = -1

  clearPdfSearchHighlights()
  pdfSearchMatches.value = []
  pdfSearchActiveIndex.value = -1
}

const openMarkdownSearch = async () => {
  closePdfSearch()
  mdSearchVisible.value = true
  await nextTick()
  const input = mdSearchInput.value
  if (input && input instanceof HTMLInputElement) {
    input.focus()
    input.select()
  }
  applyMarkdownSearch()
}

const closeMarkdownSearch = () => {
  mdSearchVisible.value = false
  mdSearchQuery.value = ''
  clearMarkdownSearchHighlights()
  mdSearchMatches.value = []
  mdSearchActiveIndex.value = -1
}

const handleMarkdownHover = (e) => {
  // 清除之前的悬停效果
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
  // 清除所有悬停效果
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });
}

// 16. 路径处理
const updateMarkdownPath = async () => {
  const baseFileName = props.fileName.replace('.pdf', '')
  markdownFilePath.value = await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'markdown',
    baseFileName,
    `${baseFileName}.md`
  )
}

const getNotesDirectory = async () => {
  const baseFileName = props.fileName.replace('.pdf', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'notes'
  );
};

const getImagesDirectory = async () => {
  const baseFileName = props.fileName.replace('.pdf', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'images'
  );
};

// 17. 数据加载
const isPdfDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_pdfjs') || '').trim().toLowerCase()
    return v === '1' || v === 'true' || v === 'yes' || v === 'on'
  } catch {
    return false
  }
}

const pdfDlog = (...args) => {
  if (!isPdfDebug()) return
  console.log('[PDFJS][Reader]', ...args)
}

const isPdfHighlightDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_pdfhl') || '').trim().toLowerCase()
    return v === '1' || v === 'true' || v === 'yes' || v === 'on'
  } catch {
    return false
  }
}

const pdfHlog = (...args) => {
  if (!isPdfHighlightDebug()) return
  console.log('[PDFHL][Reader]', ...args)
}

const pdfPageContainer = ref(null)
const pdfEditorLayer = ref(null)
const pdfHighlightLayer = ref(null)

const pdfAnnotations = ref([])
const pdfActiveAnnotationId = ref(null)
const pdfAnnotationPanelVisible = ref(false)

const clearPdfHighlightLayer = () => {
  const el = pdfHighlightLayer.value
  if (el && el instanceof HTMLElement) el.innerHTML = ''
}

const normalizeClientRectsToPage = (domRects, pageRect) => {
  const pr = pageRect && pageRect.width > 0 && pageRect.height > 0
    ? pageRect
    : null
  if (!pr) return []

  const w = pr.width || 1
  const h = pr.height || 1

  return (Array.isArray(domRects) ? domRects : [])
    .map((r) => {
      if (!r) return null
      const x = (Number(r.left) - pr.left) / w
      const y = (Number(r.top) - pr.top) / h
      const rw = Number(r.width) / w
      const rh = Number(r.height) / h
      if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(rw) || !Number.isFinite(rh)) return null
      return {
        x: Math.max(0, Math.min(1, x)),
        y: Math.max(0, Math.min(1, y)),
        w: Math.max(0, Math.min(1, rw)),
        h: Math.max(0, Math.min(1, rh)),
      }
    })
    .filter(Boolean)
    .filter((r) => r.w > 0.002 && r.h > 0.002)
    .slice(0, 200)
}

const findPdfJsEditorElement = (editorId) => {
  const root = pdfEditorLayer.value
  if (!(root instanceof HTMLElement)) return null
  const id = String(editorId || '').trim()
  if (!id) return null

  const sel = []
  sel.push(`#${typeof CSS !== 'undefined' && CSS.escape ? CSS.escape(id) : id}`)
  sel.push(`[data-editor-id="${id}"]`)
  sel.push(`[data-id="${id}"]`)
  sel.push(`[data-annotation-id="${id}"]`)
  sel.push(`[data-element-id="${id}"]`)

  for (const s of sel) {
    try {
      const el = root.querySelector(s)
      if (el && el instanceof HTMLElement) return el
    } catch {
    }
  }

  try {
    const all = Array.from(root.querySelectorAll('*'))
    for (let i = 0; i < all.length; i += 1) {
      const el = all[i]
      if (!(el instanceof HTMLElement)) continue
      if (el.id === id) return el
      const v = el.getAttribute('data-editor-id') || el.getAttribute('data-id') || el.getAttribute('data-annotation-id') || el.getAttribute('data-element-id')
      if (v === id) return el
    }
  } catch {
  }

  return null
}

const getHighlightRectsFromPdfJsEditor = (editorId) => {
  const pageEl = pdfPageContainer.value
  if (!(pageEl instanceof HTMLElement)) return []

  const editorEl = findPdfJsEditorElement(editorId)
  if (!(editorEl instanceof HTMLElement)) {
    if (isPdfHighlightDebug()) {
      pdfHlog('pdfjsEditorNotFound', { editorId: String(editorId || '') })
    }
    return []
  }

  const pageRect = pageEl.getBoundingClientRect()
  const rects = normalizeClientRectsToPage(Array.from(editorEl.getClientRects()), pageRect)

  if (isPdfHighlightDebug()) {
    const er = editorEl.getBoundingClientRect()
    pdfHlog('pdfjsEditorRects', {
      editorId: String(editorId || ''),
      editorTag: editorEl.tagName,
      editorClass: typeof editorEl.className === 'string' ? editorEl.className : '',
      editorRect: { left: Math.round(er.left), top: Math.round(er.top), width: Math.round(er.width), height: Math.round(er.height) },
      rectsPreview: rects.slice(0, 10).map((r) => ({
        x: Number((Number(r.x) || 0).toFixed(6)),
        y: Number((Number(r.y) || 0).toFixed(6)),
        w: Number((Number(r.w) || 0).toFixed(6)),
        h: Number((Number(r.h) || 0).toFixed(6)),
      })),
    })
  }

  return rects
}

const pdfEditorRectHydrated = new Set()

const handlePdfPageClick = (e) => {
  const pageEl = pdfPageContainer.value
  if (!(pageEl instanceof HTMLElement)) return

  const clientX = Number(e?.clientX)
  const clientY = Number(e?.clientY)
  if (!Number.isFinite(clientX) || !Number.isFinite(clientY)) return

  const pageRect = pageEl.getBoundingClientRect()
  if (!pageRect.width || !pageRect.height) return

  const x = (clientX - pageRect.left) / pageRect.width
  const y = (clientY - pageRect.top) / pageRect.height

  const tolX = 3 / pageRect.width
  const tolY = 3 / pageRect.height

  const pageNum = Number(currentPage.value) || 1
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []

  const hit = list.find((it) => {
    if (Number(it?.page) !== pageNum) return false
    const rects = Array.isArray(it?.rects) ? it.rects : []
    for (let i = 0; i < rects.length; i += 1) {
      const r = rects[i]
      const rx = Number(r?.x)
      const ry = Number(r?.y)
      const rw = Number(r?.w)
      const rh = Number(r?.h)
      if (!Number.isFinite(rx) || !Number.isFinite(ry) || !Number.isFinite(rw) || !Number.isFinite(rh)) continue
      if (x >= rx - tolX && x <= rx + rw + tolX && y >= ry - tolY && y <= ry + rh + tolY) return true
    }
    return false
  })

  if (!hit?.id) return

  pdfActiveAnnotationId.value = hit.id
  pdfAnnotationPanelVisible.value = true
  nextTick(() => {
    renderPdfActiveHighlightForCurrentPage()
  })

  try {
    e.preventDefault()
    e.stopPropagation()
  } catch {
  }
}

const renderPdfActiveHighlightForCurrentPage = () => {
  clearPdfHighlightLayer()
  const layerEl = pdfHighlightLayer.value
  const pageEl = pdfPageContainer.value
  if (!(layerEl instanceof HTMLElement) || !(pageEl instanceof HTMLElement)) return

  const pageNum = Number(currentPage.value) || 1
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []
  const onPage = list.filter((x) => Number(x?.page) === pageNum)
  if (!onPage.length) return

  const pr = pageEl.getBoundingClientRect()
  const w = pr.width
  const h = pr.height
  if (!w || !h) return

  const activeId = pdfActiveAnnotationId.value
  let mutated = false

  if (isPdfHighlightDebug()) {
    pdfHlog('renderHighlights', {
      page: pageNum,
      count: onPage.length,
      activeId: typeof activeId === 'string' ? activeId : String(activeId || ''),
      pageClient: { w: pageEl.clientWidth, h: pageEl.clientHeight },
      pageRect: { w: Number(pr.width.toFixed(3)), h: Number(pr.height.toFixed(3)), left: Number(pr.left.toFixed(3)), top: Number(pr.top.toFixed(3)) },
    })
  }

  onPage.forEach((it) => {
    if (!it) return

    let rects = Array.isArray(it.rects) ? it.rects : []

    const eid = typeof it.editorId === 'string' ? it.editorId : ''
    if (eid && !pdfEditorRectHydrated.has(eid)) {
      const fromPdfJs = getHighlightRectsFromPdfJsEditor(eid)
      if (fromPdfJs.length) {
        it.rects = fromPdfJs
        rects = fromPdfJs
        pdfEditorRectHydrated.add(eid)
        mutated = true
      }
    }

    const color = typeof it.color === 'string' ? it.color : 'rgba(64, 196, 255, 0.32)'
    const isActive = activeId && it.id === activeId

    rects.forEach((r) => {
      const x = (Number(r?.x) || 0) * w
      const y = (Number(r?.y) || 0) * h
      const rw = (Number(r?.w) || 0) * w
      const rh = (Number(r?.h) || 0) * h
      if (rw <= 0 || rh <= 0) return

      const div = document.createElement('div')
      div.className = 'pdf-highlight-rect'
      div.style.position = 'absolute'
      div.style.borderRadius = '3px'
      div.style.left = `${Number(x.toFixed(2))}px`
      div.style.top = `${Number(y.toFixed(2))}px`
      div.style.width = `${Number(rw.toFixed(2))}px`
      div.style.height = `${Number(rh.toFixed(2))}px`
      div.style.background = color
      if (isActive) {
        div.style.boxShadow = '0 0 0 1px rgba(64, 196, 255, 0.55)'
      }
      layerEl.appendChild(div)
    })
  })

  if (mutated) {
    pdfAnnotations.value = [...list]
    persistReaderState()
  }
}

const exitPdfAnnotationMode = () => {
  pdfAnnotationPanelVisible.value = false
  pdfActiveAnnotationId.value = null
  clearPdfHighlightLayer()
}

const getPdfSelectionInfo = () => {
  const layer = pdfTextLayer.value
  const pageEl = pdfPageContainer.value
  if (!(layer instanceof HTMLElement) || !(pageEl instanceof HTMLElement)) return null

  const sel = window.getSelection()
  const text = sel?.toString()?.trim() || ''
  if (!text) return null

  const range = sel && sel.rangeCount ? sel.getRangeAt(0) : null
  if (!range) return null

  const inLayer = layer.contains(range.startContainer) && layer.contains(range.endContainer)
  if (!inLayer) return null

  const pageRect = pageEl.getBoundingClientRect()
  const layerRect = layer.getBoundingClientRect()
  const basisRect = layerRect && layerRect.width > 0 && layerRect.height > 0
    ? layerRect
    : pageRect

  const bw = basisRect.width || 1
  const bh = basisRect.height || 1

  const br = range.getBoundingClientRect()
  const rect = br && br.width > 0 && br.height > 0
    ? br
    : (range.getClientRects()?.[0] || null)
  if (!rect) return null

  const ax = (rect.left + rect.width / 2 - pageRect.left) / (pageRect.width || 1)
  const ay = (rect.top + rect.height / 2 - pageRect.top) / (pageRect.height || 1)

  const rawClientRects = Array.from(range.getClientRects())

  const rects = rawClientRects
    .map((r) => ({
      x: (r.left - basisRect.left) / bw,
      y: (r.top - basisRect.top) / bh,
      w: r.width / bw,
      h: r.height / bh,
    }))
    .filter((r) => Number.isFinite(r.x) && Number.isFinite(r.y) && Number.isFinite(r.w) && Number.isFinite(r.h))
    .map((r) => ({
      x: Math.max(0, Math.min(1, r.x)),
      y: Math.max(0, Math.min(1, r.y)),
      w: Math.max(0, Math.min(1, r.w)),
      h: Math.max(0, Math.min(1, r.h)),
    }))
    .filter((r) => r.w > 0.002 && r.h > 0.002)
    .slice(0, 200)

  if (isPdfHighlightDebug()) {
    const sn = (n) => {
      if (!n) return null
      if (n instanceof Element) {
        return {
          node: n.tagName,
          cls: typeof n.className === 'string' ? n.className : '',
          text: String(n.textContent || '').trim().slice(0, 32),
        }
      }
      return { node: n.nodeType, text: String(n.textContent || '').trim().slice(0, 32) }
    }

    const toRect = (r) => ({
      left: Math.round(r.left),
      top: Math.round(r.top),
      width: Math.round(r.width),
      height: Math.round(r.height),
    })

    const raw = rawClientRects.slice(0, 24).map((r) => ({
      ...toRect(r),
      relLeft: Number((((r.left - pageRect.left) / (pageRect.width || 1)) || 0).toFixed(6)),
      relTop: Number((((r.top - pageRect.top) / (pageRect.height || 1)) || 0).toFixed(6)),
    }))

    const norm = rects.slice(0, 24).map((r) => ({
      x: Number((Number(r.x) || 0).toFixed(6)),
      y: Number((Number(r.y) || 0).toFixed(6)),
      w: Number((Number(r.w) || 0).toFixed(6)),
      h: Number((Number(r.h) || 0).toFixed(6)),
    }))

    const stats = (() => {
      const xs = rects.map((r) => Number(r.x)).filter((v) => Number.isFinite(v))
      const ys = rects.map((r) => Number(r.y)).filter((v) => Number.isFinite(v))
      if (!xs.length || !ys.length) return null
      return {
        minX: Number(Math.min(...xs).toFixed(6)),
        maxX: Number(Math.max(...xs).toFixed(6)),
        minY: Number(Math.min(...ys).toFixed(6)),
        maxY: Number(Math.max(...ys).toFixed(6)),
        count: rects.length,
      }
    })()

    let spanRects = []
    try {
      const spans = Array.from(layer.querySelectorAll('span'))
      for (let i = 0; i < spans.length && spanRects.length < 14; i += 1) {
        const s = spans[i]
        let hit = false
        try {
          hit = range.intersectsNode(s)
        } catch {
          hit = false
        }
        if (!hit) continue
        const rr = s.getBoundingClientRect()
        spanRects.push({
          text: String(s.textContent || '').replace(/\s+/g, ' ').trim().slice(0, 24),
          ...toRect(rr),
          relLeft: Number((((rr.left - pageRect.left) / (pageRect.width || 1)) || 0).toFixed(6)),
        })
      }
    } catch {
      spanRects = []
    }

    pdfHlog('selectionInfo', {
      textLen: text.length,
      textPreview: text.slice(0, 80),
      pageRect: toRect(pageRect),
      layerRect: toRect(layerRect),
      basisRect: toRect(basisRect),
      boundRect: toRect(rect),
      start: sn(range.startContainer instanceof Element ? range.startContainer : range.startContainer?.parentElement),
      end: sn(range.endContainer instanceof Element ? range.endContainer : range.endContainer?.parentElement),
      rawClientRects: raw,
      normalizedRects: norm,
      stats,
      selectedSpanRects: spanRects,
    })
  }

  return {
    text,
    range: range.cloneRange(),
    rect,
    rects,
    anchor: {
      x: Math.max(0, Math.min(1, ax)),
      y: Math.max(0, Math.min(1, ay)),
    },
  }
}

const applyPdfJsHighlightForCurrentSelection = async (providedInfo) => {
  const info = providedInfo || getPdfSelectionInfo()
  if (!info) return null

  let editorId = null

  try {
    const sel = window.getSelection()
    sel?.removeAllRanges()
    sel?.addRange(info.range)

    editorId = await highlightSelectionAsAnnotation({ color: '#40C4FF' })

    await nextTick()
    await new Promise((r) => setTimeout(r, 0))
  } finally {
    try {
      window.getSelection()?.removeAllRanges()
    } catch {
    }
  }

  return { info, editorId }
}

const deleteActivePdfHighlight = async () => {
  const id = typeof pdfActiveAnnotationId.value === 'string' ? pdfActiveAnnotationId.value : ''
  if (!id) return
  await deletePdfAnnotation(id)
}

const rectOverlapRatio = (a, b) => {
  const ax = Number(a?.x)
  const ay = Number(a?.y)
  const aw = Number(a?.w)
  const ah = Number(a?.h)
  const bx = Number(b?.x)
  const by = Number(b?.y)
  const bw = Number(b?.w)
  const bh = Number(b?.h)
  if (!Number.isFinite(ax) || !Number.isFinite(ay) || !Number.isFinite(aw) || !Number.isFinite(ah)) return 0
  if (!Number.isFinite(bx) || !Number.isFinite(by) || !Number.isFinite(bw) || !Number.isFinite(bh)) return 0
  if (aw <= 0 || ah <= 0 || bw <= 0 || bh <= 0) return 0

  const x1 = Math.max(ax, bx)
  const y1 = Math.max(ay, by)
  const x2 = Math.min(ax + aw, bx + bw)
  const y2 = Math.min(ay + ah, by + bh)
  const iw = Math.max(0, x2 - x1)
  const ih = Math.max(0, y2 - y1)
  const inter = iw * ih
  if (inter <= 0) return 0

  const aArea = aw * ah
  const bArea = bw * bh
  const denom = Math.max(1e-9, Math.min(aArea, bArea))
  return inter / denom
}

const selectionOverlapsAnnotation = (selRects, annRects) => {
  const s = Array.isArray(selRects) ? selRects : []
  const a = Array.isArray(annRects) ? annRects : []
  if (!s.length || !a.length) return false

  for (let i = 0; i < s.length; i += 1) {
    for (let j = 0; j < a.length; j += 1) {
      if (rectOverlapRatio(s[i], a[j]) >= 0.28) return true
    }
  }
  return false
}

const addPdfHighlightFromSelection = async () => {
  const info = getPdfSelectionInfo()
  if (!info) return

  const page = Number(currentPage.value) || 1
  const selRects = Array.isArray(info.rects) ? info.rects : []
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []

  const hit = list.find((x) => Number(x?.page) === page && selectionOverlapsAnnotation(selRects, x?.rects))
  if (hit?.id) {
    await deletePdfAnnotation(hit.id)
    return
  }

  const id = `pdfhl_${Date.now().toString(16)}_${Math.random().toString(16).slice(2)}`
  const item = {
    id,
    page,
    selectionText: String(info.text || '').slice(0, 800),
    note: '',
    anchor: info.anchor || null,
    rects: Array.isArray(info.rects) ? info.rects : [],
    color: 'rgba(64, 196, 255, 0.32)',
    editorId: '',
    noteEditorId: '',
    editing: false,
  }

  pdfAnnotations.value = [item, ...list]
  pdfActiveAnnotationId.value = id
  persistReaderState()

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()

  const r = await applyPdfJsHighlightForCurrentSelection(info)
  if (r?.editorId) {
    item.editorId = r.editorId

    const fromPdfJs = getHighlightRectsFromPdfJsEditor(r.editorId)
    if (fromPdfJs.length) {
      item.rects = fromPdfJs
      await nextTick()
      renderPdfActiveHighlightForCurrentPage()
    }

    pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
    persistReaderState()
  }
}

const addPdfAnnotationFromSelection = async () => {
  const info = getPdfSelectionInfo()
  pdfAnnotationPanelVisible.value = true
  if (!info) return

  const id = `pdfnote_${Date.now().toString(16)}_${Math.random().toString(16).slice(2)}`
  const next = {
    id,
    page: Number(currentPage.value) || 1,
    selectionText: String(info.text || '').slice(0, 800),
    note: '',
    anchor: info.anchor || null,
    rects: Array.isArray(info.rects) ? info.rects : [],
    color: 'rgba(64, 196, 255, 0.32)',
    editorId: '',
    noteEditorId: '',
    editing: true,
  }

  pdfAnnotations.value = [
    next,
    ...((Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []).map((x) => ({ ...x, editing: false }))),
  ]

  pdfActiveAnnotationId.value = id
  persistReaderState()

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()

  const r = await applyPdfJsHighlightForCurrentSelection(info)
  if (r?.editorId) {
    next.editorId = r.editorId

    const fromPdfJs = getHighlightRectsFromPdfJsEditor(r.editorId)
    if (fromPdfJs.length) {
      next.rects = fromPdfJs
      await nextTick()
      renderPdfActiveHighlightForCurrentPage()
    }

    pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
    persistReaderState()
  }
}

const activatePdfAnnotation = async (id) => {
  const it = (Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []).find((x) => x?.id === id)
  if (!it) return
  pdfActiveAnnotationId.value = id
  pdfAnnotationPanelVisible.value = true

  const targetPage = Number(it.page) || 1
  if (targetPage !== Number(currentPage.value)) {
    await loadPdfPage(targetPage)
  }

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()
}

const dispatchFreeTextCreateAtClientPoint = async ({ clientX, clientY, text }) => {
  const editor = pdfEditorLayer.value
  if (!(editor instanceof HTMLElement)) return false

  await setPdfAnnotationMode('freetext')
  await nextTick()

  const common = { bubbles: true, cancelable: true, clientX, clientY, button: 0 }
  try {
    editor.dispatchEvent(new MouseEvent('mousedown', common))
    editor.dispatchEvent(new MouseEvent('mouseup', common))
    editor.dispatchEvent(new MouseEvent('click', common))
    editor.dispatchEvent(new MouseEvent('dblclick', common))
  } catch {
  }

  await nextTick()
  await new Promise((r) => setTimeout(r, 0))

  const textareas = Array.from(editor.querySelectorAll('textarea'))
  const editables = Array.from(editor.querySelectorAll('[contenteditable="true"]'))
  const ta = textareas.length ? textareas[textareas.length - 1] : null
  const ed = !ta && editables.length ? editables[editables.length - 1] : null

  try {
    if (ta && ta instanceof HTMLTextAreaElement) {
      ta.focus()
      ta.value = String(text || '')
      ta.dispatchEvent(new Event('input', { bubbles: true }))
      ta.blur()
    } else if (ed && ed instanceof HTMLElement) {
      ed.focus()
      ed.textContent = String(text || '')
      ed.dispatchEvent(new Event('input', { bubbles: true }))
      ed.blur()
    }
  } catch {
  }

  await setPdfAnnotationMode('none')
  return true
}

const savePdfAnnotationNote = async (id) => {
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []
  const idx = list.findIndex((x) => x?.id === id)
  if (idx < 0) return

  const it = list[idx]
  const noteText = String(it?.note || '').trim()
  it.editing = false
  pdfAnnotations.value = [...list]
  persistReaderState()

  if (!noteText) {
    if (it?.noteEditorId) {
      await removeAnnotationEditorById(it.noteEditorId)
      it.noteEditorId = ''
      pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
      persistReaderState()
    }
    await savePdfAnnotationsToOriginal()
    return
  }

  const targetPage = Number(it.page) || 1
  if (targetPage !== Number(currentPage.value)) {
    await loadPdfPage(targetPage)
  }

  await nextTick()

  if (it?.noteEditorId) {
    await removeAnnotationEditorById(it.noteEditorId)
    it.noteEditorId = ''
  }

  const pageEl = pdfPageContainer.value
  if (!(pageEl instanceof HTMLElement)) {
    await savePdfAnnotationsToOriginal()
    return
  }

  const pageIndex = Math.max(0, (Number(targetPage) || 1) - 1)
  const beforeIds = getAnnotationEditorIdsForPage(pageIndex)

  const r = pageEl.getBoundingClientRect()
  const ax = Number(it?.anchor?.x)
  const ay = Number(it?.anchor?.y)
  const cx = r.left + (Number.isFinite(ax) ? Math.max(0, Math.min(1, ax)) : 0.5) * r.width
  const cy = r.top + (Number.isFinite(ay) ? Math.max(0, Math.min(1, ay)) : 0.5) * r.height

  await dispatchFreeTextCreateAtClientPoint({ clientX: cx, clientY: cy, text: noteText })

  await nextTick()
  await new Promise((r) => setTimeout(r, 0))

  const afterIds = getAnnotationEditorIdsForPage(pageIndex)
  const created = afterIds.filter((x) => !beforeIds.includes(x))
  const newId = created.length ? created[created.length - 1] : ''
  if (newId) {
    it.noteEditorId = newId
    pdfAnnotations.value = [...(Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : [])]
    persistReaderState()
  }

  await savePdfAnnotationsToOriginal()
}

const deletePdfAnnotation = async (id) => {
  const list = Array.isArray(pdfAnnotations.value) ? pdfAnnotations.value : []
  const it = list.find((x) => x?.id === id)

  if (it?.editorId) {
    await removeAnnotationEditorById(it.editorId)
  }
  if (it?.noteEditorId) {
    await removeAnnotationEditorById(it.noteEditorId)
  }

  pdfAnnotations.value = list.filter((x) => x?.id !== id)
  if (pdfActiveAnnotationId.value === id) {
    pdfActiveAnnotationId.value = null
  }
  persistReaderState()

  await nextTick()
  renderPdfActiveHighlightForCurrentPage()
}

const annotationMode = ref('none')

const setPdfAnnotationMode = async (mode) => {
  annotationMode.value = String(mode || 'none')
  await setAnnotationEditorMode(annotationMode.value)
}

const togglePdfHighlight = async () => {
  const next = annotationMode.value === 'highlight' ? 'none' : 'highlight'
  await setPdfAnnotationMode(next)
}

const togglePdfFreeText = async () => {
  const next = annotationMode.value === 'freetext' ? 'none' : 'freetext'
  await setPdfAnnotationMode(next)
}

const savePdfAnnotationsToOriginal = async () => {
  try {
    const b64 = await saveDocumentBase64()
    await invoke('write_file_bytes_base64', { path: String(props.filePath || ''), base64: b64 })
    message.success('PDF标注已保存到原文件', { duration: 1200 })
  } catch (e) {
    const msg = typeof e === 'object' ? e.message || e.toString() : String(e)
    message.error(`保存PDF标注失败: ${msg}`)
  }
}

const loadPdfPage = async (pageNum) => {
  try {
    await ensureAnnotationEditorUi({ containerEl: pdfPanel.value, viewerEl: pdfPageContainer.value })
    await setPdfAnnotationMode(annotationMode.value)

    const data = await loadPage(pageNum, {
      scale: zoomLevel.value,
      rotation: rotation.value,
      canvasEl: pdfCanvas.value,
      textLayerEl: pdfTextLayer.value,
      annotationLayerEl: pdfAnnotationLayer.value,
      editorLayerEl: pdfEditorLayer.value,
      onGoToPage: (p) => goToPage(p)
    });

    currentPage.value = data.page_number;
    totalPages.value = data.total_pages;

    await nextTick()
    renderPdfActiveHighlightForCurrentPage()

    if (pdfSearchVisible.value && pdfSearchQuery.value.trim()) {
      nextTick(() => applyPdfSearch())
    }

    const fp = String(props.filePath || '')
    if (fp && pdfOutlineLoadedFor.value !== fp && typeof getOutlineFlat === 'function') {
      pdfOutlineLoadedFor.value = fp
      try {
        const outline = await getOutlineFlat()
        outlineStore.setOutline(Array.isArray(outline) ? outline : [])
        outlineStore.setJumpToLine((p) => {
          const n = Number(p)
          if (Number.isFinite(n)) goToPage(n)
        })
      } catch {
        outlineStore.setOutline([])
      }
    }

    if (isPdfDebug()) {
      const spanCount = pdfTextLayer.value ? pdfTextLayer.value.querySelectorAll('span').length : -1
      pdfDlog('pageLoaded', {
        page: currentPage.value,
        total: totalPages.value,
        pageW: Number(data?.page_width) || 0,
        pageH: Number(data?.page_height) || 0,
        spanCount
      })
    }
  } catch (err) {
    console.error('页面加载失败:', err);
    message.error(`加载失败: ${err.message}`);
  }
};

// 加载Markdown页面
const loadMarkdownPage = async (pageNum) => {
  try {
    if (!markdownFilePath.value) {
      await updateMarkdownPath();
    }

    const response = await invoke('load_markdown_content', {
      filepath: markdownFilePath.value,
      page: pageNum - 1
    });

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    // 获取markdown文件所在目录
    const mdDir = await dirname(markdownFilePath.value);
    markdownContent.value = response.content;
    // console.log("markdownContent.value:", markdownContent.value);
    markdownLines.value = await convertPdfRenderMarkdown(markdownContent.value,
      { libraryBasePath: mdDir }
    );
    console.log("markdownLines.value:", markdownLines.value);

    await nextTick()
    if (mdSearchVisible.value && mdSearchQuery.value.trim()) {
      applyMarkdownSearch()
    }

  } catch (error) {
    console.error('加载Markdown失败:', error);
    markdownLines.value = [{
      raw: '',
      html: `<div class="error">加载错误: ${error.message}</div>`
    }];
  }
};

// 生成唯一文件名（浏览器兼容方案）
function generateUniqueSvgName(content, type = 'mermaid') {
  // 简单哈希函数（替代Node.js的crypto）
  const simpleHash = (str) => {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash = hash & hash; // Convert to 32bit integer
    }
    return Math.abs(hash).toString(16).substring(0, 8);
  };

  // 使用内容哈希和时间戳
  const contentHash = simpleHash(content);
  const timestamp = Date.now();

  return `${type}-${contentHash}-${timestamp}.svg`;
}

async function processRenderedSvg(content, type = 'mermaid') {
  try {
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = content;

    const svgElement = tempDiv.querySelector('svg');
    if (!svgElement) return content;

    // 克隆并清理SVG
    const svgClone = svgElement.cloneNode(true);
    svgClone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    const svgContent = svgClone.outerHTML;

    // 生成唯一文件名（浏览器兼容）
    const fileName = generateUniqueSvgName(svgContent, type);
    const imageDir = await getImagesDirectory();

    // 调用Tauri后端保存
    const { relative_path } = await invoke('save_svg_file', {
      content: svgContent,
      fileName: fileName,
      imageDir: imageDir
    });
    const processedImagesPath = await processImages(`![${type}图表](../${relative_path})`, imageDir);
    console.log(`${type}图表已保存到: ${processedImagesPath}`);
    return processedImagesPath;

  } catch (error) {
    console.error(`处理${type} SVG失败:`, error);
    // 失败时返回原始内容，但添加错误标记
    return `<!-- SVG处理失败 -->\n${content}`;
  }
}

// 从渲染后的HTML代码块中提取原始代码
function extractCodeFromRendered(htmlContent) {
  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = htmlContent;

  // 尝试从不同结构的代码块中提取
  const codeElement = tempDiv.querySelector('pre code') ||
    tempDiv.querySelector('.hljs code') ||
    tempDiv.querySelector('code');

  if (codeElement) {
    return codeElement.textContent;
  }

  // 如果无法提取，尝试从原始内容中获取
  const match = htmlContent.match(/<code[^>]*>([\s\S]*?)<\/code>/);
  return match ? match[1] : htmlContent;
}

// 处理代码块内容
function processCodeContent(content, htmlContent) {
  // 提取原始代码
  const rawCode = extractCodeFromRendered(htmlContent || content);

  // 检测语言类型
  let language = '';
  const langMatch = content.match(/^```(\w+)/) ||
    (htmlContent || '').match(/language-(\w+)/);
  if (langMatch) language = langMatch[1];

  // 生成标准Markdown代码块
  return `\`\`\`${language}
${rawCode.trim()}
\`\`\``;
}

// 确保路径初始化的专用函数
const ensureNotesPath = async () => {
  if (!notesFilePath.value) {
    const notesDir = await getNotesDirectory()
    notesFilePath.value = await join(notesDir, `${props.fileName.replace('.pdf', '')}.json`)
  }
  return notesFilePath.value
}

// 时间格式化
const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString()
}

// 添加新卡片
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

    // 调整选中状态
    if (activeCardIndex.value === index) {
      activeCardIndex.value = -1;
    } else if (activeCardIndex.value > index) {
      activeCardIndex.value--;
    }
  }
};

const addToNotes = async (element) => {
  // 如果处于Alt键收集模式，则追加到当前卡片
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

  // 创建新卡片（单条笔记）
  const newCard = {
    id: Date.now().toString(),
    type: 'single',  // 单条笔记类型
    title: getCardTitle(normalizedElement.type),
    timestamp: new Date().toISOString(),
    // 单条笔记的items数组只包含一个元素
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

// 获取卡片标题
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

// 加载现有笔记
const loadExistingNotes = async () => {
  try {
    const response = await invoke('load_notes_file', {
      filePath: notesFilePath.value
    });

    // 确保返回的是解析后的JSON数据
    const notesData = typeof response === 'string' ? JSON.parse(response) : response;

    if (notesData?.cards?.length) {
      noteCards.value = notesData.cards.map(card => {
        // 兼容旧数据格式
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
          type: items.length > 1 ? 'composite' : 'single', // 根据条目数量确定类型
          items,
          timestamp: card.timestamp || new Date().toISOString()
        };
      });
    }
  } catch (error) {
    console.error('加载笔记失败:', error);
    noteCards.value = [];
  }
};

const handleMissingNote = async (action, cardData) => {
  try {
    // 1. 尝试重新加载数据
    await loadExistingNotes();

    // 2. 检查笔记是否存在
    const exists = noteCards.value.some(card => card.id === cardData?.id);

    if (!exists) {
      console.warn('笔记不存在，自动转为新增操作', cardData);

      // 3. 确保数据完整
      const completeCard = {
        ...cardData,
        items: Array.isArray(cardData.items) ? cardData.items : [],
        timestamp: cardData.timestamp || new Date().toISOString()
      };

      await updateBackendNotes('add', completeCard);
    }
  } catch (error) {
    console.error('处理缺失笔记失败:', error);
    message.error('自动恢复失败，请手动添加笔记');
  }
};

// 修改updateBackendNotes函数，确保原始Markdown被保存到文件
const updateBackendNotes = async (action, cardData) => {
  try {
    // 1. 验证必要字段
    if (!cardData?.id) {
      throw new Error('无效的笔记ID');
    }

    // 2. 确保items数组存在
    const items = Array.isArray(cardData.items) ? cardData.items : [];

    // 3. 构建与后端匹配的数据结构
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
      filePath: await ensureNotesPath(), // 确保路径已初始化
      importFileId: Number(props.importFileId) || 0,
      importFileName: props.fileName || '',
    };

    // 4. 调试日志
    console.debug('发送更新请求:', {
      action,
      cardId: cardData.id,
      payload: JSON.parse(JSON.stringify(payload)) // 深度拷贝
    });

    await invoke('update_notes_file', payload);
    message.success('笔记已保存', { duration: 1500 });

  } catch (error) {
    // 5. 更健壮的错误处理
    const errorMessage = typeof error === 'object'
      ? error.message || error.toString()
      : String(error);

    console.error('更新笔记失败:', {
      error: errorMessage,
      cardData: JSON.parse(JSON.stringify(cardData || {}))
    });

    // 6. 安全地检查错误信息
    const normalizedError = errorMessage.toLowerCase();
    if (normalizedError.includes('未找到') || normalizedError.includes('not found')) {
      await handleMissingNote(action, cardData);
    } else {
      message.error(`保存失败: ${errorMessage}`, { duration: 3000 });
    }
  }
};

// 导出笔记
const exportNotes = () => {
  if (!notesContent.value) {
    alert('笔记内容为空');
    return;
  }

  // 创建临时div来提取文本内容
  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = notesContent.value;

  // 转换内容为Markdown格式
  let exportContent = '';

  // 处理每个笔记条目
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
  a.download = `PDF笔记_${props.fileName.replace('.pdf', '')}_${new Date().toISOString().slice(0, 10)}.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

// 20. 拖拽和缩放功能
const startDrag = (e) => {
  try {
    if (isPdfDebug()) {
      pdfDlog('mousedown', {
        altKey: Boolean(e.altKey),
        button: e.button,
        target: e?.target?.tagName,
        inTextLayer: Boolean(e?.target instanceof Element && e.target.closest('.pdf-text-layer')),
      })
    }

    if (e.button !== 0 || !pdfPanel.value) return;
    if (!e.altKey) return;

    // 取消任何进行中的动画
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
      left: pdfPanel.value.scrollLeft,
      top: pdfPanel.value.scrollTop
    };
    velocity.value = { x: 0, y: 0 };
    lastDragTime.value = performance.now();

    document.body.style.userSelect = 'none';
    pdfPanel.value.style.cursor = 'grabbing';

    if (!dragGlobalListenersActive) {
      dragGlobalListenersActive = true
      window.addEventListener('mouseup', endDrag, true)
      window.addEventListener('blur', endDrag, true)
    }

    e.preventDefault();
  } catch (error) {
    console.error('Error in startDrag:', error);
    resetDragState();
  }
};

const handleDrag = (e) => {
  try {
    if (!isDragging.value || !pdfPanel.value) return;

    const now = performance.now();
    const deltaTime = now - lastDragTime.value;
    lastDragTime.value = now;

    // 计算移动距离
    const dx = e.clientX - startDragPos.value.x;
    const dy = e.clientY - startDragPos.value.y;

    // 更新滚动位置
    pdfPanel.value.scrollLeft = startScrollPos.value.left - dx;
    pdfPanel.value.scrollTop = startScrollPos.value.top - dy;

    // 计算速度（像素/毫秒）
    if (deltaTime > 0) {
      velocity.value = {
        x: dx / deltaTime,
        y: dy / deltaTime
      };
    }

    e.preventDefault();
  } catch (error) {
    console.error('Error in handleDrag:', error);
    endDrag();
  }
};

const endDrag = () => {
  try {
    if (!isDragging.value) return;

    resetDragState();

    // 如果速度足够大，启动惯性滚动
    const speedThreshold = 0.1;
    if (Math.abs(velocity.value.x) > speedThreshold ||
      Math.abs(velocity.value.y) > speedThreshold) {
      isDecelerating.value = true;
      decelerate();
    }
  } catch (error) {
    console.error('Error in endDrag:', error);
    resetDragState();
  }
};

const handlePdfPanelMouseUp = () => {
  endDrag()
  handleSelectionMouseUp()
}

const resetDragState = () => {
  isDragging.value = false;
  isDecelerating.value = false;
  document.body.style.userSelect = '';

  if (dragGlobalListenersActive) {
    dragGlobalListenersActive = false
    window.removeEventListener('mouseup', endDrag, true)
    window.removeEventListener('blur', endDrag, true)
  }

  if (pdfPanel.value) {
    pdfPanel.value.style.cursor = 'text';
  }
};

// 21. 惯性滚动处理
const decelerate = () => {
  if (!isDecelerating.value || !pdfPanel.value) return;

  const friction = 0.95; // 摩擦系数
  const minSpeed = 0.5; // 最小速度阈值

  // 应用摩擦减速
  velocity.value.x *= friction;
  velocity.value.y *= friction;

  // 更新滚动位置
  pdfPanel.value.scrollLeft += velocity.value.x * 16; // 16ms为一帧
  pdfPanel.value.scrollTop += velocity.value.y * 16;

  // 检查是否需要继续减速
  if (Math.abs(velocity.value.x) > minSpeed || Math.abs(velocity.value.y) > minSpeed) {
    animationFrameId.value = requestAnimationFrame(decelerate);
  } else {
    isDecelerating.value = false;
    velocity.value = { x: 0, y: 0 };
  }
};

// 22. 缩放控制
const handleWheelZoom = async (e) => {
  if (!e.ctrlKey) return

  e.preventDefault()
  const delta = e.deltaY < 0 ? 0.1 : -0.1
  zoomLevel.value = Math.max(0.5, Math.min(2, zoomLevel.value + delta))

  await loadPdfPage(currentPage.value)

  await nextTick()
  if (!pdfPanel.value) return

  const panel = pdfPanel.value
  const w = Number(currentPageData.value?.page_width) || 0
  const h = Number(currentPageData.value?.page_height) || 0
  if (!w || !h) return

  panel.scrollLeft = Math.max(0, (w - panel.clientWidth) / 2)
  panel.scrollTop = Math.max(0, (h - panel.clientHeight) / 2)
}

// 23. 分页控制
const prevPage = () => {
  if (currentPage.value > 1) {
    const newPage = currentPage.value - 1;
    currentPage.value = newPage;
    jumpPage.value = newPage;
    loadPdfPage(newPage);
    loadMarkdownPage(newPage);
  }
};

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    const newPage = currentPage.value + 1;
    currentPage.value = newPage;
    jumpPage.value = newPage;
    loadPdfPage(newPage);
    loadMarkdownPage(newPage);
  }
};

const goToPage = (page) => {
  const pageNum = typeof page === 'number' ? page : jumpPage.value;
  const targetPage = Math.max(1, Math.min(pageNum, totalPages.value));
  currentPage.value = targetPage;
  jumpPage.value = targetPage;
  loadPdfPage(targetPage);
  loadMarkdownPage(targetPage);
};

// 24. 面板控制
const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value;
  if (showNotes.value) {
    noteCards.value = [...noteCards.value];
  }
};

// 25. 缩放功能
const zoomIn = async () => {
  zoomLevel.value = Math.min(zoomLevel.value + 0.1, 2)
  await loadPdfPage(currentPage.value)
};

const zoomOut = async () => {
  zoomLevel.value = Math.max(zoomLevel.value - 0.1, 0.5)
  await loadPdfPage(currentPage.value)
};

// 26. 面板大小调整
const startResize = (e) => {
  isResizing.value = true;
  startX.value = e.clientX;

  const leftEl = document.querySelector('.pdf-left-wrapper')
  startLeftWidth.value = leftEl
    ? parseInt(document.defaultView.getComputedStyle(leftEl).width, 10)
    : 0

  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResize);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

// 处理鼠标移动
const handleMouseMove = (e) => {
  if (!isResizing.value) return;

  const containerEl = document.querySelector('.pdf-main-content');
  if (!containerEl) return;

  // Split resize should be based on the available width for panels (exclude the resize handle itself)
  const handleEl = document.querySelector('.resize-handle');
  const handleWidth = handleEl ? handleEl.getBoundingClientRect().width : 0;

  const containerWidth = Math.max(1, containerEl.getBoundingClientRect().width - handleWidth);
  const dx = e.clientX - startX.value;
  let newLeftWidth = ((startLeftWidth.value + dx) * 100) / containerWidth;

  newLeftWidth = Math.max(30, Math.min(70, newLeftWidth));

  leftPanelWidth.value = `${newLeftWidth}%`;
  previousLeftWidth.value = leftPanelWidth.value;
};

// 停止拖动
const stopResize = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};

// 27. 旋转功能
const rotate = async (degrees) => {
  rotation.value = (rotation.value + degrees) % 360
  await loadPdfPage(currentPage.value)
  await fitToWidth()
}

// 28. 自适应宽度
const fitToWidth = async () => {
  const panel = pdfPanel.value
  if (!panel) return

  // Use actual available width to fit page (do not involve height constraints)
  const panelWidth = panel.getBoundingClientRect().width || panel.clientWidth
  if (!panelWidth) return

  const horizontalMargin = Math.min(20, panelWidth * 0.05)
  const safeWidth = Math.max(0, panelWidth - horizontalMargin)

  const base = await loadPage(currentPage.value, {
    scale: 1,
    rotation: rotation.value
  })

  const baseW = Number(base?.page_width) || 0
  if (!baseW) return

  const minScale = 0.5
  const scale = safeWidth / baseW
  zoomLevel.value = Math.max(minScale, Math.min(scale, 2))

  await loadPdfPage(currentPage.value)

  await nextTick()
  const w = Number(currentPageData.value?.page_width) || 0
  if (!w) return

  // Re-center using current clientWidth after render (accounts for scrollbar changes)
  panel.scrollLeft = Math.max(0, (w - panel.clientWidth) / 2)
  panel.scrollTop = 0
};

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
};

// 29. 防抖函数
const debounce = (fn, delay) => {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), delay);
  };
};

// 30. 翻译功能
// const toggleTranslate = async () => {
//   showTranslation.value = !showTranslation.value;

//   if (showTranslation.value) {
//     // 先标记所有需要跳过的行
//     markdownLines.value = markdownLines.value.map(line => {
//       if (!line.translation) {
//         line.skipTranslation = shouldSkipTranslation(line.raw, line.html);
//         if (line.skipTranslation) {
//           line.translation = line.html; // 直接使用原始内容作为"翻译"
//         }
//       }
//       return line;
//     });

//     // 只翻译未标记为跳过的行
//     await translateMarkdown();
//   }
// };

// 翻译相关状态
const translateQueue = ref([]);
// const isTranslating = ref(false);
const lastTranslateTime = ref(0);
const translateBatchSize = 5; // 每次批量翻译5条
const translateInterval = 200; // 间隔200ms，确保不超过5次/秒

// 修改后的翻译函数
const translateMarkdown = async () => {
  if (isTranslating.value) return;

  // 收集需要翻译的行
  const linesToTranslate = markdownLines.value
    .filter(line => !line.skipTranslation && !line.translation)
    .map(line => {
      // 提取纯文本内容
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
        text: textNodes.join(' '), // 合并所有文本节点
        html: line.html
      };
    })
    .filter(item => item.text.length > 0);

  // 添加到队列
  translateQueue.value.push(...linesToTranslate);

  // 开始处理队列
  processTranslateQueue();
};

// 处理翻译队列
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

    // 付费翻译接口
    // const response = await invoke('translate_text', {
    //   texts: texts,
    //   secretId: "1122334",
    //   secretKey: "5566778",
    // });

    // 免费翻译接口
    const response = await invoke('translate_text_free', {
      texts: texts,
    });

    // 调试日志
    console.log('翻译响应:', response);

    // 修复点1: 检查响应结构是否匹配
    if (response && response.translated_texts?.length === texts.length) {
      currentBatch.forEach((item, index) => {
        // 修复点2: 确保使用正确的字段名
        const translatedText = response.translated_texts[index];
        if (translatedText) {
          item.line.translation = wrapTranslation(item.line.html, translatedText);
          item.line.translationError = null;
        } else {
          item.line.translationError = '翻译结果为空';
        }
      });
    } else {
      // 修复点3: 提供更详细的错误信息
      const errorMsg = response?.error ||
        (response?.translated_texts ? '返回结果数量不匹配' : '无效的响应格式');
      currentBatch.forEach(item => {
        item.line.translationError = `翻译失败: ${errorMsg}`;
      });
      console.error('翻译错误:', response);
    }

    lastTranslateTime.value = Date.now();
  } catch (error) {
    // 修复点4: 更好的错误处理
    console.error('翻译异常:', error);
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
    return html; // 返回原始内容
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

// 修改toggleTranslate函数
const toggleTranslate = async () => {
  showTranslation.value = !showTranslation.value;

  if (showTranslation.value) {
    // 先标记所有需要跳过的行
    markdownLines.value = markdownLines.value.map(line => {
      if (!line.translation) {
        line.skipTranslation = shouldSkipTranslation(line.raw, line.html);
        if (line.skipTranslation) {
          line.translation = line.html; // 直接使用原始内容作为"翻译"
        }
      }
      return line;
    });

    // 只翻译未标记为跳过的行
    await translateMarkdown();
  }
};

// 1. 定义必要的 ref
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

// 2. 修改保存函数
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

    // 创建卡片的深拷贝
    const currentCard = JSON.parse(JSON.stringify(noteCards.value[index]));

    // 统一处理内容更新
    if (currentCard.type === 'composite') {
      let raw = editableDiv?.tagName === 'TEXTAREA' ? editableDiv.value : editableDiv.innerText;
      if (editableDiv?.tagName === 'TEXTAREA' && !editorDirty.value[index] && !String(raw || '').trim()) {
        raw = getCompositeEditorText(currentCard);
      }

      const items = parseCompositeEditorTextToItems(raw, currentCard.id);
      currentCard.items = items;
      currentCard.type = items.length > 1 ? 'composite' : 'single';
      if (currentCard.type === 'composite') {
        currentCard.title = `复合笔记 (${items.length}项)`;
      }
      currentCard.content = raw;
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
    console.error('保存编辑失败:', {
      error,
      index,
      card: noteCards.value[index]
    });
    message.error(`保存失败: ${error.message}`);
  }
};

// 4. 退出编辑模式
const getCompositeEditorText = (card) => {
  const items = Array.isArray(card?.items) ? card.items : [];
  return items
    .map(item => (item?.original_markdown || item?.originalMarkdown || item?.content || '').trim())
    .filter(Boolean)
    .join('\n\n---\n\n');
};

const parseCompositeEditorTextToItems = (text, cardId) => {
  const normalized = String(text || '').replace(/\r\n/g, '\n').trim();
  if (!normalized) return [];

  const parts = normalized
    .split(/\n\s*---\s*\n/g)
    .map(p => p.trim())
    .filter(Boolean);

  const now = Date.now();
  const ts = new Date().toISOString();

  return parts.map((part, idx) => ({
    id: `${cardId || now}-${idx}-${now}`,
    type: 'text',
    content: part,
    originalMarkdown: part,
    original_markdown: part,
    timestamp: ts
  }));
};

const exitEditMode = (index) => {
  const card = noteCards.value[index];
  const editableDiv = contentEditables.value[index];

  if (card?.type === 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    const before = getCompositeEditorText(card);
    if (editableDiv.value !== before) {
      if (!confirm('有未保存的修改，确定要取消吗？')) return;
    }
    resetEditorDirty(index);
    activeCardIndex.value = -1;
    return;
  }

  if (card?.type !== 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    const before = getSingleEditorText(card);
    if (editableDiv.value !== before) {
      if (!confirm('有未保存的修改，确定要取消吗？')) return;
    }
    resetEditorDirty(index);
    activeCardIndex.value = -1;
    return;
  }

  if (editableDiv && editableDiv.tagName !== 'TEXTAREA' && editableDiv.innerHTML !== noteCards.value[index].content) {
    if (!confirm('有未保存的修改，确定要取消吗？')) return;
  }
  activeCardIndex.value = -1;
};

const enterEditMode = async (index) => {
  activeCardIndex.value = index;
  await nextTick();
  const editableDiv = contentEditables.value[index];
  const card = noteCards.value[index];

  if (card?.type === 'composite' && editableDiv?.tagName === 'TEXTAREA') {
    resetEditorDirty(index);
    if (!editableDiv.value || editableDiv.value.trim() === '') {
      editableDiv.value = getCompositeEditorText(card);
    }
    editableDiv.focus();
    editableDiv.setSelectionRange(editableDiv.value.length, editableDiv.value.length);
    return;
  }

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

// 31. 生命周期和事件监听
onMounted(async () => {
  console.log('[PDFJS][Reader] mounted')

  document.addEventListener('selectionchange', () => {
    if (!isPdfDebug()) return
    try {
      const sel = window.getSelection()
      const text = sel?.toString() || ''
      if (text.trim()) {
        pdfDlog('selectionchange', {
          len: text.trim().length,
          sample: text.trim().slice(0, 80)
        })
      }
    } catch {
    }
  })

  await store.get_librariesRoot()
  await updateMarkdownPath()
  outlineStore.setJumpToLine((p) => {
    const n = Number(p)
    if (Number.isFinite(n)) goToPage(n)
  })

  const state = loadReaderState();
  bookmarks.value = [...new Set(state.bookmarks)].sort((a, b) => a - b);
  pdfAnnotations.value = Array.isArray(state.pdfAnnotations) ? state.pdfAnnotations : [];
  totalReadingMs.value = state.totalReadingMs;
  completedAt.value = state.completedAt;
  completedReadingMs.value = state.completedReadingMs;
  pendingRestorePage.value = state.lastPage;
  isHydrated.value = true;
  startReadingSession();

  // 并行加载但不包括fitToWidth
  await Promise.all([
    (async () => {
      await ensureNotesPath()
      await loadExistingNotes()
    })(),
    loadPdfPage(1),
    loadMarkdownPage(1)
  ])

  if (pendingRestorePage.value != null && totalPages.value) {
    const p = Math.max(1, Math.min(Number(pendingRestorePage.value) || 1, totalPages.value));
    pendingRestorePage.value = null;
    if (p !== 1) goToPage(p);
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

  watch(pdfAnnotations, () => {
    if (!isHydrated.value) return;
    persistReaderState();
  }, { deep: true });

  // 添加全局键盘事件监听
  window.addEventListener('keyup', handleKeyUp);

  nextTick(() => {
    window.addEventListener('resize', debounce(() => {
      if (isNotesReady.value) fitToWidth()
    }, 300))

    fitToWidth()
  })

  autoSaveInterval = setInterval(() => {
    const idx = activeCardIndex.value;
    if (idx < 0 || idx >= noteCards.value.length) return;
    if (!contentEditables.value?.[idx]) return;
    saveCardEdit(idx);
  }, 30000); // 每30秒自动保存
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
  // 清理拖动事件监听
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  window.removeEventListener('keyup', handleKeyUp);
  outlineStore.setOutline([]);
});

const handleResize = () => {
  if (pdfPanel.value && currentPageData.value) {
    fitToWidth();
  }
};

// 32. 文件路径变化监听
watch(() => props.filePath, async (newPath) => {
  if (newPath) {
    await updateMarkdownPath();
    currentPage.value = 1;
    loadPdfPage(1);
    loadMarkdownPage(1);
    notesContent.value = '';
  }
});

// 33. 图片加载处理
const handleImageLoad = () => {
  nextTick(() => {
    fitToWidth();
  });
};

// 监听 noteCards，自动渲染所有笔记
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
/* 34. 主容器样式 */
.pdf-viewer-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
}

/* 35. 控制栏样式 */
.pdf-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 28px;
  padding: 6px 8px;
  background-color: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  overflow: hidden;
}

/* 36. 按钮基础样式 */
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
  gap: 8px;
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

/* 37. 页面信息样式 */
.page-info {
  font-size: 12px;
  font-weight: 500;
  color: var(--guaxi-text-primary);
  padding: 5px 8px;
  background-color: var(--guaxi-panel-bg);
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

/* 38. 页面导航样式 */
.page-navigation {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.page-navigation input {
  width: 50px;
  padding: 6px;
  text-align: center;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

/* 39. 右侧控制区样式 */
.right-controls {
  display: flex;
  gap: 10px;
  flex-shrink: 0;
}

/* 40. 缩放控制样式 */
.zoom-level {
  min-width: 20px;
  text-align: center;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 5px 8px;
}

.zoom-in,
.zoom-out {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
}

.zoom-in:hover,
.zoom-out:hover {
  background-color: var(--guaxi-header-bg);
}

.zoom-in:disabled,
.zoom-out:disabled {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  cursor: not-allowed;
}

/* 41. 旋转按钮样式 */
.rotate-left,
.rotate-right,
.fit-width {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
  padding: 5px 8px;
}

.rotate-left:hover,
.rotate-right:hover,
.fit-width:hover {
  background-color: var(--guaxi-header-bg);
}

/* 42. 笔记按钮样式 */
.toggle-notes {
  background-color: #4dabf7;
  color: white;
  border: 1px solid #339af0;
  transition: all 0.2s;
}

.toggle-notes:hover {
  background-color: #339af0;
  box-shadow: 0 2px 8px rgba(77, 171, 247, 0.3);
}

/* 43. 导出按钮样式 */
.export-notes {
  background-color: #20c997;
  color: white;
  border: 1px solid #12b886;
  transition: all 0.2s;
}

.export-notes:hover {
  background-color: #12b886;
  box-shadow: 0 2px 8px rgba(32, 201, 151, 0.3);
}

/* 44. 主内容区样式 */
.pdf-main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  gap: 0;
  min-height: 0;
}

.pdf-left-wrapper {
  display: flex;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

/* 45. PDF渲染面板样式 */
.pdf-render-panel {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  position: relative;
  background-color: var(--guaxi-content-bg);
  overscroll-behavior: none;
  scroll-behavior: auto;
}

/* 46. PDF页面容器样式 */
.pdf-image-container {
  width: 100%;
  min-height: 0;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding: 16px 0;
  background-color: var(--guaxi-content-bg);
  box-sizing: border-box;
}

.outline-panel-wrapper {
  flex: 0 0 260px;
  min-width: 260px;
  height: 100%;
  overflow: hidden;
  border-right: 1px solid var(--guaxi-border);
  box-sizing: border-box;
  background-color: var(--guaxi-panel-bg);
}

.pdf-page-container {
  position: relative;
  display: inline-block;
  background: #fff;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.rendered-pdf-canvas {
  display: block;
  pointer-events: none;
}

.pdf-annotation-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  z-index: 2;
  pointer-events: none;
}

.pdf-text-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  line-height: 1;
  user-select: text;
  z-index: 3;
}

.pdf-editor-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  z-index: 4;
  pointer-events: none;
}

.pdf-page-container.pdf-editor-active .pdf-editor-layer {
  pointer-events: auto;
}

.pdf-highlight-layer {
  position: absolute;
  left: 0;
  top: 0;
  right: 0;
  bottom: 0;
  overflow: hidden;
  z-index: 2.3;
  pointer-events: none;
}

.pdf-highlight-rect {
  position: absolute;
  border-radius: 3px;
}

:deep(.pdf-annotation-layer a),
:deep(.pdf-annotation-layer input),
:deep(.pdf-annotation-layer textarea),
:deep(.pdf-annotation-layer select),
:deep(.pdf-annotation-layer button) {
  pointer-events: auto;
}

:deep(.pdf-text-layer span) {
  color: transparent;
  position: absolute;
  white-space: pre;
  transform-origin: 0% 0%;
  pointer-events: auto;
}

/* 48. 加载消息样式 */
.pdf-loading-message {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  color: var(--guaxi-text-secondary);
  font-size: 14px;
}

/* 49. 调整大小手柄样式 */
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

/* 50. Markdown面板样式 */
.markdown-panel {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 5px;
  background-color: var(--guaxi-content-bg);
  display: flex;
}

.outline-panel-wrapper {
  flex: 0 0 260px;
  min-width: 260px;
  height: 100%;
  overflow: hidden;
  border-right: 1px solid var(--guaxi-border);
  box-sizing: border-box;
}

/* 51. Markdown内容样式 */
.markdown-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 18px;
  line-height: 1.6;
  color: var(--guaxi-text-primary);
  background-color: var(--guaxi-content-bg);
  user-select: text;
  padding: 10px;
  cursor: text;
}

/* 52. Markdown元素样式 */
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

/* 53. 图片样式 */
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

/* 54. 选中和高亮样式 */
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

/* 55. 选择消息样式 */
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
  width: 320px;
  max-width: calc(100vw - 16px);
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  backdrop-filter: blur(8px);
  box-shadow: var(--guaxi-header-shadow);
  color: var(--guaxi-text-primary);
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

.pdf-annotation-toolbar-wrap {
  position: sticky;
  bottom: 14px;
  z-index: 900;
  display: flex;
  justify-content: center;
  pointer-events: none;
}

.pdf-annotation-toolbar {
  pointer-events: auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: 12px;
  background: rgba(20, 20, 20, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.12);
  backdrop-filter: blur(10px);
}

.pdf-annotations-panel {
  flex: 0 0 320px;
  width: 320px;
  min-width: 280px;
  height: 100%;
  border-left: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.pdf-annotations-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.pdf-annotations-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.pdf-annotations-close {
  width: 30px;
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

.pdf-annotations-list {
  flex: 1 1 auto;
  overflow: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pdf-annotations-empty {
  padding: 10px 8px;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
}

.pdf-annotation-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-header-bg);
  cursor: pointer;
}

.pdf-annotation-item-active {
  box-shadow: 0 0 0 2px rgba(255, 180, 80, 0.35);
  border-color: rgba(255, 180, 80, 0.65);
}

.pdf-annotation-item-meta {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 6px;
}

.pdf-annotation-item-text {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  line-height: 1.45;
  word-break: break-word;
}

.pdf-annotation-item-delete {
  width: 30px;
  height: 28px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotation-item-delete:hover {
  color: var(--guaxi-text-primary);
  background: rgba(0, 0, 0, 0.04);
}

.pdf-annotation-item-main {
  flex: 1 1 auto;
  min-width: 0;
}

.pdf-annotation-item-note {
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: rgba(64, 196, 255, 0.08);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.pdf-annotation-item-editor {
  margin-top: 8px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.pdf-annotation-note-input {
  flex: 1 1 auto;
  width: 100%;
  min-width: 0;
  min-height: 86px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  line-height: 1.45;
  resize: vertical;
  outline: none;
  box-sizing: border-box;
}

.pdf-annotation-note-save {
  flex: 0 0 auto;
  width: 34px;
  height: 34px;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotation-tool-btn {
  width: 36px;
  height: 32px;
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.25);
  color: rgba(255, 255, 255, 0.9);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.pdf-annotation-tool-btn-active {
  background: rgba(77, 171, 247, 0.35);
  border-color: rgba(77, 171, 247, 0.65);
}

.pdf-annotation-tool-btn:hover {
  background: rgba(0, 0, 0, 0.35);
}

.pdf-annotation-tool-btn:active {
  transform: translateY(1px);
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

.inpage-search-bar-pdf {
  left: 16px;
  right: auto;
}

.inpage-search-bar-md {
  right: 16px;
  left: auto;
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

.markdown-content :deep(.inpage-match),
.pdf-text-layer :deep(.inpage-match) {
  background: rgba(255, 230, 120, 0.55);
  border-radius: 4px;
  padding: 0 2px;
}

.markdown-content :deep(.inpage-match-active),
.pdf-text-layer :deep(.inpage-match-active) {
  background: rgba(255, 180, 80, 0.75);
  box-shadow: 0 0 0 2px rgba(255, 180, 80, 0.35);
}

/* 56. 笔记面板样式 */
.notes-panel {
  position: fixed;
  top: 0;
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: var(--guaxi-panel-bg);
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

/* 57. 笔记头部样式 */
.notes-header {
  padding: 12px 16px;
  background-color: #4dabf7;
  color: white;
  display: flex;
  justify-content: space-between;
  align-items: center;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.notes-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

/* 58. 笔记内容区样式 */
.notes-content {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}

/* 60. 笔记内容样式 */
.note-content {
  padding: 12px 16px;
  min-height: 60px;
  outline: none;
}

/* 保存反馈动画 */
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

/* 编辑状态更明显的视觉反馈 */
.note-content[contenteditable="true"] {
  /* outline: 2px solid #4dabf7; */
  padding: 12px;
  border-radius: 8px;
  background-color: rgba(3, 101, 167, 0.05);
}

/* 复合笔记的编辑状态 */
.composite-note-content[contenteditable="true"] {
  min-height: 100px;
  padding: 12px;
}

/* 59. 笔记卡片样式 */
.note-card {
  transition: all 0.2s var(--n-bezier);
  cursor: pointer;
}

.note-card:hover {
  box-shadow: 0 1px 2px -2px rgba(0, 0, 0, 0.08),
    0 3px 6px 0 rgba(0, 0, 0, 0.06),
    0 5px 12px 4px rgba(0, 0, 0, 0.04);
}

/* 61. 添加卡片容器样式 */
.add-card-container {
  margin-top: 8px;
  margin-bottom: 16px;
}

/* 62. 关闭按钮样式 */
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

/* 63. 笔记气泡样式 */
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

/* 64. 淡入淡出动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* 65. 复合笔记样式 */
.composite-note-content {
  padding: 12px;
}

.note-segment {
  margin-bottom: 16px;
  padding-bottom: 16px;
  border-bottom: 1px dashed #e0e0e0;
}

/* 66. 不同类型笔记段落的样式 */
.text-segment {
  font-size: 14px;
  line-height: 1.6;
}

.code-segment {
  background: #f6f8fa;
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

/* 卡片操作按钮容器 */
.card-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  /* 调整按钮间距 */
}

/* 按钮图标样式 */
.n-button--tiny .n-icon {
  margin: 0;
  /* 移除默认外边距 */
  display: flex;
  /* 确保图标居中 */
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


/* 68. 翻译按钮样式 */
.toggle-translate {
  background-color: #7950f2;
  color: white;
  border: 1px solid #6741d9;
  transition: all 0.2s;
}

.toggle-translate:hover {
  background-color: #6741d9;
  box-shadow: 0 2px 8px rgba(121, 80, 242, 0.3);
}

/* 69. Markdown行样式 */
.markdown-line {
  margin-bottom: 1em;
}

/* 70. 原文和译文样式 */
.original-content {
  padding: 8px;
  border-radius: 4px;
  background-color: rgba(255, 255, 255, 0.8);
}

.translated-content {
  padding: 8px;
  margin-top: 4px;
  border-radius: 4px;
  background-color: rgba(245, 245, 245, 0.8);
  border-left: 3px solid #7950f2;
  color: #555;
}

.error-translation {
  border-left: 3px solid #ff6b6b;
  background-color: rgba(255, 107, 107, 0.05);
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
  background-color: #4dabf7;
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

.skipped-translation {
  opacity: 0.7;
  border-left: 3px solid #ccc;
  padding-left: 8px;
  margin-left: 4px;
  font-style: italic;
  color: #666;
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
</style>

<style>
/* 71. 数学公式全局样式 */
/* .math-inline {
  display: inline;
  vertical-align: middle;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
} */

/* 72. Mermaid图表全局样式 */
.mermaid {
  background-color: white;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}

.dark .mermaid {
  background-color: #2d2d2d;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}

/* 73. 代码块全局样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #f6f8fa !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
}

.hljs code {
  font-family: inherit !important;
}

code:not(.hljs code) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
  font-size: 0.9em;
}

.hljs:hover::before {
  opacity: 1;
}

/* 74. 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
}

.dark .hljs::before {
  background: #444;
  color: #fff;
}

/* 75. 复制按钮样式 */
.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

/* 76. 不同类型笔记的样式 */
.text-note {
  white-space: pre-wrap;
}

.code-note pre {
  background-color: #f6f8fa;
  padding: 12px;
  border-radius: 4px;
  overflow-x: auto;
}

.math-note {
  overflow-x: auto;
}

.diagram-note {
  text-align: center;
}

/* 确保数学公式容器正确显示 */
.math-inline-container,
.math-block-container {
  display: inline-block;
}

mjx-container.math-inline {
  display: inline;
  vertical-align: middle;
}

mjx-container.math-block {
  display: block;
  margin: 1em 0;
  text-align: center;
}
</style>