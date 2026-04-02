<template>
  <div ref="notebookRootRef" class="guaxi-notebook" :class="themeClass">
    <div class="guaxi-notebook__toolbar">
      <div class="guaxi-notebook__toolbar-main">
        <button
          type="button"
          class="guaxi-notebook__icon-btn"
          :disabled="!canUndoNotebookHistory"
          title="撤销"
          aria-label="撤销"
          @click="undoNotebookHistory"
        >
          <Icon :icon="undoIcon" width="16" height="16" />
        </button>
        <button
          type="button"
          class="guaxi-notebook__icon-btn"
          :disabled="!canRedoNotebookHistory"
          title="重做"
          aria-label="重做"
          @click="redoNotebookHistory"
        >
          <Icon :icon="redoIcon" width="16" height="16" />
        </button>
        <button
          type="button"
          class="guaxi-notebook__icon-btn"
          title="保存当前文档"
          aria-label="保存当前文档"
          @click="requestNotebookSave"
        >
          <Icon :icon="saveIcon" width="16" height="16" />
        </button>
        <button
          type="button"
          class="guaxi-notebook__icon-btn"
          title="搜索探索模式内容"
          aria-label="搜索探索模式内容"
          @click="openNotebookSearch"
        >
          <Icon :icon="searchIcon" width="16" height="16" />
        </button>
        <button
          type="button"
          class="guaxi-notebook__icon-btn primary"
          title="新增 Python Cell"
          aria-label="新增 Python Cell"
          @click="appendCell('python')"
        >
          <Icon :icon="terminalSquareIcon" width="16" height="16" />
        </button>
        <button
          type="button"
          class="guaxi-notebook__icon-btn"
          title="新增 Markdown Cell"
          aria-label="新增 Markdown Cell"
          @click="appendCell('markdown')"
        >
          <Icon :icon="filePlusIcon" width="16" height="16" />
        </button>
      </div>
    </div>

    <div v-show="notebookSearchVisible" class="guaxi-notebook__search-panel-shell">
      <div class="guaxi-notebook__search-panel" @mousedown.stop>
        <input
          ref="notebookSearchInputRef"
          v-model="notebookSearchKeyword"
          class="guaxi-notebook__search-input"
          type="text"
          placeholder="搜索探索模式内容..."
          @keydown.enter.prevent="focusNextNotebookSearchMatch"
          @keydown.shift.enter.prevent="focusPrevNotebookSearchMatch"
        />
        <span class="guaxi-notebook__search-count">{{ notebookSearchMatchCountText }}</span>
        <button class="guaxi-notebook__search-btn" type="button" @click="focusPrevNotebookSearchMatch">上一个</button>
        <button class="guaxi-notebook__search-btn" type="button" @click="focusNextNotebookSearchMatch">下一个</button>
        <button class="guaxi-notebook__search-btn guaxi-notebook__search-btn--danger" type="button" @click="closeNotebookSearch">关闭</button>
      </div>
    </div>

    <template v-for="(cell, index) in notebookCells" :key="cell.key">
      <div v-if="index === 0" class="guaxi-notebook__insert-strip">
        <div class="guaxi-notebook__insert-line"></div>
        <div class="guaxi-notebook__insert-trigger" aria-hidden="true">
          <Icon :icon="plusIcon" width="12" height="12" />
        </div>
        <div class="guaxi-notebook__insert-actions">
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入 Markdown Cell"
            aria-label="在这里插入 Markdown Cell"
            @click="insertCell(index, 'above', 'markdown')"
          >
            <Icon :icon="filePlusIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入 Python Cell"
            aria-label="在这里插入 Python Cell"
            @click="insertCell(index, 'above', 'python')"
          >
            <Icon :icon="terminalSquareIcon" width="16" height="16" />
          </button>
        </div>
      </div>

      <div
        class="guaxi-notebook__cell"
        :class="[
          `is-${cell.type}`,
          {
            'is-search-match': isNotebookSearchMatchedCell(cell),
            'is-search-active': isNotebookSearchActiveCell(cell),
          },
        ]"
        :data-cell-key="cell.key"
      >
      <template v-if="cell.type === 'markdown'">
        <div
          class="guaxi-notebook__markdown-surface"
          :class="{ 'is-editing': isMarkdownCellEditing(cell) }"
        >
          <div class="guaxi-notebook__markdown-actions">
            <button
              v-if="isMarkdownCellEditing(cell)"
              type="button"
              class="guaxi-notebook__icon-btn"
              title="完成 Markdown 编辑"
              aria-label="完成 Markdown 编辑"
              @click="exitMarkdownEditMode(cell.key, 'toolbar-finish')"
            >
              <Icon :icon="checkIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn danger"
              title="删除当前 Cell"
              aria-label="删除当前 Cell"
                @click="confirmRemoveCell(cell)"
            >
              <Icon :icon="trash2Icon" width="16" height="16" />
            </button>
          </div>

          <div v-if="isMarkdownCellEditing(cell)" class="guaxi-notebook__markdown-edit-shell">
            <GuaxiNotebookMarkdownEditor
              :ref="(instance) => setMarkdownEditorRef(cell.key, instance)"
              :cell-key="cell.key"
              :model-value="cell.content"
              :theme="theme"
              @update:model-value="updateMarkdownCell(cell.key, $event)"
              @request-exit="exitMarkdownEditMode(cell.key, $event)"
            />
          </div>
          <div
            v-else
            class="guaxi-notebook__markdown-preview-shell is-clickable"
            role="button"
            tabindex="0"
            @pointerdown.capture="handleMarkdownPreviewPointerDown(cell.key, $event)"
            @click="enterMarkdownEditMode(cell.key, 'preview-click')"
            @keydown.enter.prevent="enterMarkdownEditMode(cell.key, 'preview-enter')"
            @keydown.space.prevent="enterMarkdownEditMode(cell.key, 'preview-space')"
          >
            <div
              v-if="String(cell.content || '').trim()"
              class="guaxi-notebook__markdown-preview markdown-body"
              v-html="getRenderedMarkdownCellHtml(cell)"
            ></div>
            <div v-else class="guaxi-notebook__empty"></div>
          </div>
        </div>
      </template>

      <template v-else>
        <div class="guaxi-notebook__python-layout">
          <div class="guaxi-notebook__python-toolbar">
            <div class="guaxi-notebook__python-toolbar-copy">
              <span class="guaxi-notebook__python-badge">Python</span>
              <span v-if="String(cell.meta?.title || '').trim()" class="guaxi-notebook__python-title">{{ cell.meta?.title }}</span>
            </div>
            <div class="guaxi-notebook__python-toolbar-actions">
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                :disabled="isCellBusy(cell)"
                title="从外部导入表格到当前 Python 会话"
                aria-label="从外部导入表格到当前 Python 会话"
                @click="importTableToPythonSession(cell)"
              >
                <Icon :icon="uploadIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn primary"
                :disabled="isCellBusy(cell)"
                title="运行当前 Python Cell"
                aria-label="运行当前 Python Cell"
                @click="runPythonCell(cell, { writeBack: false })"
              >
                <Icon :icon="playIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                :disabled="isCellBusy(cell)"
                title="运行并将输出写回文档"
                aria-label="运行并将输出写回文档"
                @click="runPythonCell(cell, { writeBack: true })"
              >
                <Icon :icon="checkCircleIcon" width="16" height="16" />
              </button>
              <button
                v-if="hasVisibleOutput(cell)"
                type="button"
                class="guaxi-notebook__icon-btn"
                :title="getCellSnapshot(cell).meta.outputCollapsed ? '显示输出' : '隐藏输出'"
                :aria-label="getCellSnapshot(cell).meta.outputCollapsed ? '显示输出' : '隐藏输出'"
                @click="toggleCellOutput(cell)"
              >
                <Icon :icon="getCellSnapshot(cell).meta.outputCollapsed ? eyeIcon : eyeOffIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                :title="getCellSnapshot(cell).meta.sessionCollapsed ? '显示变量' : '隐藏变量'"
                :aria-label="getCellSnapshot(cell).meta.sessionCollapsed ? '显示变量' : '隐藏变量'"
                @click="toggleCellSession(cell)"
              >
                <Icon :icon="variableIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn"
                title="打开 Python 控制台"
                aria-label="打开 Python 控制台"
                @click="$emit('open-console')"
              >
                <Icon :icon="terminalIcon" width="16" height="16" />
              </button>
              <button
                type="button"
                class="guaxi-notebook__icon-btn danger"
                title="删除当前 Cell"
                aria-label="删除当前 Cell"
                @click="confirmRemoveCell(cell)"
              >
                <Icon :icon="trash2Icon" width="16" height="16" />
              </button>
            </div>
          </div>

          <GuaxiNotebookPythonEditor
            :cell-key="cell.key"
            :model-value="cell.code"
            :theme="theme"
            @update:model-value="updatePythonCell(cell.key, $event)"
          />

          <div v-if="hasVisibleOutput(cell) && !getCellSnapshot(cell).meta.outputCollapsed" class="guaxi-notebook__panel">
            <div class="guaxi-notebook__panel-head">
              <div class="guaxi-notebook__panel-title">输出</div>
              <div class="guaxi-notebook__panel-actions">
                <button type="button" class="guaxi-notebook__panel-btn" @click="writeCellOutputToMarkdown(cell)">写回 Markdown</button>
                <button type="button" class="guaxi-notebook__panel-btn" @click="exportCellOutputToFile(cell)">导出文件</button>
              </div>
            </div>
            <div class="guaxi-notebook__panel-content">
              <div v-if="getCellSnapshot(cell).display?.type === 'html'" class="guaxi-notebook__rich" v-html="String(getCellSnapshot(cell).display.html || '')"></div>
              <div v-else-if="getCellSnapshot(cell).display?.type === 'svg'" class="guaxi-notebook__rich" v-html="String(getCellSnapshot(cell).display.svg || '')"></div>
              <div
                v-else-if="getCellSnapshot(cell).display?.type === 'markdown'"
                class="guaxi-notebook__rich markdown-body"
                v-html="getRenderedOutputMarkdownHtml(cell)"
              ></div>
              <img
                v-else-if="getCellDisplayKind(cell) === 'image' && getCellDisplayUrl(cell)"
                class="guaxi-notebook__image"
                :src="getCellDisplayUrl(cell)"
                alt="Python Cell Output"
              />
              <audio
                v-else-if="getCellDisplayKind(cell) === 'audio' && getCellDisplayUrl(cell)"
                class="guaxi-notebook__media"
                :src="getCellDisplayUrl(cell)"
                controls
                preload="metadata"
              ></audio>
              <video
                v-else-if="getCellDisplayKind(cell) === 'video' && getCellDisplayUrl(cell)"
                class="guaxi-notebook__media guaxi-notebook__media--video"
                :src="getCellDisplayUrl(cell)"
                controls
                preload="metadata"
              ></video>
              <div v-else-if="getCellSnapshot(cell).dataframe" class="guaxi-notebook__dataframe">
                <table>
                  <thead>
                    <tr>
                      <th v-for="column in getCellSnapshot(cell).dataframe.columns || []" :key="column">{{ column }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="(row, rowIndex) in getCellSnapshot(cell).dataframe.rows || []" :key="`${cell.key}-row-${rowIndex}`">
                      <td v-for="(value, colIndex) in row" :key="`${cell.key}-${rowIndex}-${colIndex}`">{{ value }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <pre v-if="getCellSnapshot(cell).outputText" class="guaxi-notebook__output-text">{{ getCellSnapshot(cell).outputText }}</pre>
            </div>
          </div>

          <div v-if="!getCellSnapshot(cell).meta.sessionCollapsed" class="guaxi-notebook__panel">
            <div class="guaxi-notebook__panel-title">会话变量</div>
            <div v-if="getCellSessionVars(cell).length" class="guaxi-notebook__vars">
              <div v-for="item in getCellSessionVars(cell)" :key="`${cell.key}-${item.name}`" class="guaxi-notebook__var">
                <div class="guaxi-notebook__var-main">
                  <span class="guaxi-notebook__var-name">{{ item.name }}</span>
                  <span class="guaxi-notebook__var-type">{{ item.value_type || item.type || 'value' }}</span>
                </div>
                <div class="guaxi-notebook__var-preview">{{ item.preview || '' }}</div>
              </div>
            </div>
            <div v-else class="guaxi-notebook__empty">当前会话里还没有可见变量</div>
          </div>
        </div>
      </template>
      </div>

      <div class="guaxi-notebook__insert-strip">
        <div class="guaxi-notebook__insert-line"></div>
        <div class="guaxi-notebook__insert-trigger" aria-hidden="true">
          <Icon :icon="plusIcon" width="12" height="12" />
        </div>
        <div class="guaxi-notebook__insert-actions">
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入 Markdown Cell"
            aria-label="在这里插入 Markdown Cell"
            @click="insertCell(index, 'below', 'markdown')"
          >
            <Icon :icon="filePlusIcon" width="16" height="16" />
          </button>
          <button
            type="button"
            class="guaxi-notebook__icon-btn"
            title="在这里插入 Python Cell"
            aria-label="在这里插入 Python Cell"
            @click="insertCell(index, 'below', 'python')"
          >
            <Icon :icon="terminalSquareIcon" width="16" height="16" />
          </button>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { useMessage } from 'naive-ui'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { confirm as confirmDialog, open, save } from '@tauri-apps/plugin-dialog'
import { readFile, writeFile } from '@tauri-apps/plugin-fs'
import mermaid from 'mermaid'
import MarkdownIt from 'markdown-it'
import filePlusIcon from '@iconify-icons/lucide/file-plus'
import terminalSquareIcon from '@iconify-icons/lucide/terminal-square'
import playIcon from '@iconify-icons/lucide/play'
import checkIcon from '@iconify-icons/lucide/check'
import checkCircleIcon from '@iconify-icons/lucide/check-circle-2'
import plusIcon from '@iconify-icons/lucide/plus'
import searchIcon from '@iconify-icons/lucide/search'
import saveIcon from '@iconify-icons/lucide/save'
import uploadIcon from '@iconify-icons/lucide/upload'
import eyeIcon from '@iconify-icons/lucide/eye'
import eyeOffIcon from '@iconify-icons/lucide/eye-off'
import variableIcon from '@iconify-icons/lucide/variable'
import terminalIcon from '@iconify-icons/lucide/terminal'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import undoIcon from '@iconify-icons/lucide/undo'
import redoIcon from '@iconify-icons/lucide/redo'
import { createLogger } from '../../utils/logger'
import GuaxiNotebookPythonEditor from './GuaxiNotebookPythonEditor.vue'
import GuaxiNotebookMarkdownEditor from './GuaxiNotebookMarkdownEditor.vue'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import {
  createPythonCellId,
  createPythonSessionId,
  dataframeToMarkdownTable,
  parsePythonCellBlocks,
  replacePythonCellBlock,
} from './utils/guaxiPythonCellSyntax'
import {
  createNotebookCellsFromMarkdown,
  createNotebookMarkdownCell,
  createNotebookPythonCell,
  normalizeNotebookCells,
  serializeNotebookCells,
} from './utils/guaxiNotebookCells'
import {
  getPythonCellSnapshot,
  patchPythonCellSnapshot,
  replacePythonCellSnapshots,
} from './utils/guaxiPythonCellState'

const props = defineProps({
  markdown: {
    type: String,
    default: '',
  },
  filePath: {
    type: String,
    default: '',
  },
  theme: {
    type: String,
    default: 'light',
  },
})

const emit = defineEmits(['update:markdown', 'state-change', 'open-console', 'request-save', 'outline-change'])

const message = useMessage()
const logger = createLogger('GuaxiNotebookView')
const notebookOutlineMarkdownParser = new MarkdownIt()

const refreshTick = ref(0)
const pendingFocusCellKey = ref('')
const busyMap = ref({})
const notebookRootRef = ref(null)
const notebookSearchInputRef = ref(null)
const editingMarkdownCellKey = ref('')
const renderedMarkdownHtmlMap = ref({})
const renderedOutputMarkdownHtmlMap = ref({})
const resolvedDisplayUrlMap = ref({})
const markdownRenderVersion = ref(0)
const notebookCells = ref([])
const notebookSearchVisible = ref(false)
const notebookSearchKeyword = ref('')
const notebookSearchCellMatches = ref([])
const notebookSearchDomMatches = ref([])
const notebookSearchActiveIndex = ref(-1)
const notebookSearchIndexReady = ref(false)
const notebookMarkdownWorkerRef = ref(null)
const notebookMarkdownWorkerReady = ref(false)
const lastEmittedMarkdown = ref(String(props.markdown || '').replace(/\r\n?/g, '\n'))
const imageBedRootCache = ref('')
const notebookSidecarLoadVersion = ref(0)
const notebookHistoryEntries = ref([])
const notebookHistoryIndex = ref(-1)
const notebookHistoryApplying = ref(false)
const markdownEditorRefMap = new Map()
const notebookMarkdownRenderCache = new Map()
const notebookDisplayUrlCache = new Map()
const notebookMarkdownWorkerPendingTasks = new Map()
let notebookSidecarPersistTimer = 0
let notebookMarkdownEmitTimer = 0
let notebookSearchIndexSyncTimer = 0
let notebookSearchQueryTimer = 0
let notebookMarkdownWorkerTaskId = 0
let notebookMermaidRenderCounter = 0

const themeClass = computed(() => (String(props.theme || '').toLowerCase() === 'dark' ? 'theme-dark' : 'theme-light'))

const bumpRefresh = () => {
  refreshTick.value += 1
  emit('state-change')
}

const normalizeNotebookMarkdownText = (markdown = '') => String(markdown || '').replace(/\r\n?/g, '\n')
const escapeRegExp = (raw) => String(raw || '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
const escapeHtmlAttribute = (raw) => String(raw || '')
  .replace(/&/g, '&amp;')
  .replace(/"/g, '&quot;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes
}

const uint8ArrayToBase64 = (bytes) => {
  const buffer = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes || [])
  let binary = ''
  const chunkSize = 0x8000
  for (let offset = 0; offset < buffer.length; offset += chunkSize) {
    const chunk = buffer.subarray(offset, Math.min(buffer.length, offset + chunkSize))
    binary += String.fromCharCode(...chunk)
  }
  return btoa(binary)
}

// 设计说明：
// 1. Notebook 写回输出时需要把二进制资源交给后端统一落盘，避免前端 capability scope 限制。
// 2. 这里保留一个通用二进制写入助手，供“导出文件”和“资源库入库”两条链路复用。
const writeBytesToPath = async (path, bytes, reason = 'unknown') => {
  const normalizedPath = String(path || '').trim()
  if (!normalizedPath) {
    throw new Error('缺少目标文件路径')
  }
  const base64 = uint8ArrayToBase64(bytes)
  await invoke('write_file_bytes_base64', {
    path: normalizedPath,
    base64,
  })
  logger.info('[Notebook] wrote binary file via backend command', {
    path: normalizedPath,
    byteLength: Number(bytes?.byteLength || bytes?.length || 0),
    reason,
  })
}

const readBytesFromPath = async (path, reason = 'unknown') => {
  const normalizedPath = String(path || '').trim()
  if (!normalizedPath) {
    throw new Error('缺少源文件路径')
  }
  const base64 = await invoke('read_file_bytes_base64', { path: normalizedPath })
  const bytes = base64ToUint8Array(base64)
  logger.info('[Notebook] read binary file via backend command', {
    path: normalizedPath,
    byteLength: Number(bytes?.byteLength || bytes?.length || 0),
    reason,
  })
  return bytes
}
const sanitizeFileSegment = (raw, fallback = 'output') => {
  const normalized = String(raw || '')
    .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
  return normalized || fallback
}
const joinNotebookPath = (...parts) => parts
  .map((part, index) => {
    const normalized = String(part || '').replace(/\\/g, '/')
    return index === 0 ? normalized.replace(/\/+$/g, '') : normalized.replace(/^\/+|\/+$/g, '')
  })
  .filter(Boolean)
  .join('/')
const getNotebookDocDir = () => {
  const filePath = String(props.filePath || '').trim().replace(/\\/g, '/')
  if (!filePath) return ''
  const lastSlash = filePath.lastIndexOf('/')
  return lastSlash >= 0 ? filePath.slice(0, lastSlash) : ''
}
const getNotebookDocStem = () => {
  const filePath = String(props.filePath || '').trim().replace(/\\/g, '/')
  const baseName = filePath.split('/').pop() || 'notebook'
  return sanitizeFileSegment(baseName.replace(/\.[^.]+$/, ''), 'notebook')
}
const getRelativeNotebookPath = (targetPath) => {
  const docDir = getNotebookDocDir()
  const normalizedTarget = String(targetPath || '').replace(/\\/g, '/')
  if (docDir && normalizedTarget.startsWith(`${docDir}/`)) {
    return normalizedTarget.slice(docDir.length + 1)
  }
  return normalizedTarget
}
const getNotebookSidecarPath = () => {
  const filePath = String(props.filePath || '').trim()
  if (!filePath) return ''
  return `${filePath}.cellmeta`
}
const stripNotebookOutputBlocks = (markdown = '') => {
  const lines = String(markdown || '').replace(/\r\n?/g, '\n').split('\n')
  const kept = []
  let insideOutput = false
  for (const line of lines) {
    const text = String(line || '').trim()
    if (/^<!--\s+guaxi:pyout:.*:start\s+-->$/i.test(text)) {
      insideOutput = true
      continue
    }
    if (insideOutput && /^<!--\s+guaxi:pyout:.*:end\s+-->$/i.test(text)) {
      insideOutput = false
      continue
    }
    if (!insideOutput) {
      kept.push(line)
    }
  }
  return kept.join('\n')
}
const computeNotebookDigest = (text = '') => {
  const normalized = String(text || '')
  let hash = 2166136261
  for (let index = 0; index < normalized.length; index += 1) {
    hash ^= normalized.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return `nb-${(hash >>> 0).toString(16)}-${normalized.length}`
}
const normalizeNotebookSourceForSidecar = (markdown = '') => {
  return stripNotebookOutputBlocks(normalizeNotebookMarkdownText(markdown))
}
const getNotebookCellStableId = (cell) => String(cell?.meta?.id || '').trim()
const getNotebookCellContentDigest = (cell) => {
  if (cell?.type === 'python') {
    return computeNotebookDigest(String(cell?.code || ''))
  }
  return computeNotebookDigest(String(cell?.content || ''))
}
const buildNotebookCellRelation = (cells = [], index = 0) => {
  const current = cells[index] || null
  const previous = index > 0 ? cells[index - 1] : null
  const next = index + 1 < cells.length ? cells[index + 1] : null
  return {
    index,
    currentDigest: current ? getNotebookCellContentDigest(current) : '',
    previousType: String(previous?.type || ''),
    previousStableId: previous?.type === 'python' ? getNotebookCellStableId(previous) : '',
    previousDigest: previous ? getNotebookCellContentDigest(previous) : '',
    nextType: String(next?.type || ''),
    nextStableId: next?.type === 'python' ? getNotebookCellStableId(next) : '',
    nextDigest: next ? getNotebookCellContentDigest(next) : '',
  }
}
const cloneNotebookCellsForHistory = (cells = []) => {
  return normalizeNotebookCells(cells).map((cell) => {
    if (cell?.type === 'python') {
      return createNotebookPythonCell({
        key: cell.key,
        code: String(cell.code || ''),
        meta: {
          ...(cell.meta || {}),
        },
      })
    }
    return createNotebookMarkdownCell(String(cell?.content || ''), { key: cell?.key })
  })
}
const canUndoNotebookHistory = computed(() => notebookHistoryIndex.value > 0)
const canRedoNotebookHistory = computed(
  () => notebookHistoryIndex.value >= 0 && notebookHistoryIndex.value < notebookHistoryEntries.value.length - 1,
)
const serializeNotebookCellsForSidecar = (cells = []) => {
  const normalizedCells = normalizeNotebookCells(cells)
  return normalizedCells.map((cell, index) => {
    const relation = buildNotebookCellRelation(normalizedCells, index)
    if (cell?.type === 'python') {
      return {
        type: 'python',
        key: String(cell.key || ''),
        code: String(cell.code || ''),
        meta: {
          ...(cell.meta || {}),
        },
        relation,
      }
    }
    return {
      type: 'markdown',
      key: String(cell?.key || ''),
      content: String(cell?.content || ''),
      relation,
    }
  })
}
const serializeNotebookSnapshotsForSidecar = (cells = []) => {
  const snapshots = {}
  normalizeNotebookCells(cells).forEach((cell) => {
    if (cell?.type !== 'python') return
    const cellId = String(cell?.cellId || '')
    const stableId = getNotebookCellStableId(cell)
    const snapshotKey = stableId || cellId
    if (!snapshotKey) return
    const snapshot = getCellSnapshot(cell)
    snapshots[snapshotKey] = {
      stableId,
      cellId,
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: String(snapshot?.outputText || ''),
      display: snapshot?.display || null,
      dataframe: snapshot?.dataframe || null,
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      meta: snapshot?.meta || {},
    }
  })
  return snapshots
}
const applyNotebookSidecarSnapshots = (cells = [], snapshots = {}) => {
  const nextSnapshots = {}
  let restoredCount = 0
  normalizeNotebookCells(cells).forEach((cell) => {
    if (cell?.type !== 'python') return
    const cellId = String(cell?.cellId || '')
    const stableId = getNotebookCellStableId(cell)
    if (!cellId && !stableId) return
    const persisted = snapshots && typeof snapshots === 'object'
      ? snapshots[stableId] || snapshots[cellId] || null
      : null
    if (!persisted || typeof persisted !== 'object') return
    nextSnapshots[cellId] = {
      outputText: String(persisted.outputText || ''),
      display: persisted.display || null,
      dataframe: persisted.dataframe || null,
      sessionVars: Array.isArray(persisted.sessionVars) ? persisted.sessionVars : [],
      meta: persisted.meta && typeof persisted.meta === 'object' ? persisted.meta : {},
    }
    restoredCount += 1
  })
  replacePythonCellSnapshots(props.filePath, nextSnapshots)
  logger.info('[Notebook] applied sidecar python snapshots', {
    cellCount: normalizeNotebookCells(cells).filter((cell) => cell?.type === 'python').length,
    restoredCount,
    snapshotCount: Object.keys(snapshots && typeof snapshots === 'object' ? snapshots : {}).length,
  })
}

// 设计说明：
// 1. sidecar 不再直接替代当前 Markdown 扫描结果，而是作为“探索模式状态层”叠加回最新扫描结果。
// 2. 这样即使用户在源 Markdown 中新增/删除/重排内容，我们也能先重建当前 cell 结构，再把已知状态映射回来。
// 3. Python Cell 优先通过稳定 meta.id 对齐；若外部编辑导致稳定 id 丢失，再退化到代码摘要匹配。
// 4. Markdown Cell 始终以源文件最新内容为准，避免旧 sidecar 覆盖正文变更。
const reconcileNotebookCellsWithSidecar = (markdownCells = [], sidecar = null) => {
  const normalizedMarkdownCells = normalizeNotebookCells(markdownCells)
  const sidecarCells = Array.isArray(sidecar?.cells) ? normalizeNotebookCells(sidecar.cells) : []
  if (!sidecarCells.length) return normalizedMarkdownCells

  const sidecarPythonByStableId = new Map()
  const sidecarPythonByCodeDigest = new Map()
  sidecarCells.forEach((cell) => {
    if (cell?.type !== 'python') return
    const stableId = getNotebookCellStableId(cell)
    const codeDigest = getNotebookCellContentDigest(cell)
    if (stableId) {
      sidecarPythonByStableId.set(stableId, cell)
    }
    if (codeDigest) {
      const bucket = sidecarPythonByCodeDigest.get(codeDigest) || []
      bucket.push(cell)
      sidecarPythonByCodeDigest.set(codeDigest, bucket)
    }
  })

  let stableIdMatchedCount = 0
  let codeDigestMatchedCount = 0
  const reconciledCells = normalizedMarkdownCells.map((cell) => {
    if (cell?.type !== 'python') {
      return cell
    }
    const stableId = getNotebookCellStableId(cell)
    let persistedCell = stableId ? sidecarPythonByStableId.get(stableId) : null
    let matchStrategy = persistedCell ? 'stable-id' : 'none'
    if (!persistedCell) {
      const codeDigest = getNotebookCellContentDigest(cell)
      const digestCandidates = codeDigest ? sidecarPythonByCodeDigest.get(codeDigest) || [] : []
      if (digestCandidates.length === 1) {
        persistedCell = digestCandidates[0]
        matchStrategy = 'code-digest'
      }
    }
    if (!persistedCell) {
      return cell
    }
    if (matchStrategy === 'stable-id') {
      stableIdMatchedCount += 1
    } else if (matchStrategy === 'code-digest') {
      codeDigestMatchedCount += 1
    }
    return createNotebookPythonCell({
      ...cell,
      code: String(cell.code || ''),
      meta: {
        ...(persistedCell.meta || {}),
        ...(cell.meta || {}),
        id: getNotebookCellStableId(persistedCell) || stableId,
      },
    })
  })
  logger.info('[Notebook] reconciled notebook cells with sidecar', {
    markdownCellCount: normalizedMarkdownCells.filter((cell) => cell?.type === 'markdown').length,
    pythonCellCount: normalizedMarkdownCells.filter((cell) => cell?.type === 'python').length,
    stableIdMatchedCount,
    codeDigestMatchedCount,
  })
  return reconciledCells
}
const guessExtensionFromMime = (mime, fallback = 'bin') => {
  const normalized = String(mime || '').trim().toLowerCase()
  if (normalized.includes('markdown')) return 'md'
  if (normalized.includes('svg')) return 'svg'
  if (normalized.includes('html')) return 'html'
  if (normalized.includes('json')) return 'json'
  if (normalized.includes('plain')) return 'txt'
  if (normalized.includes('png')) return 'png'
  if (normalized.includes('jpeg') || normalized.includes('jpg')) return 'jpg'
  if (normalized.includes('gif')) return 'gif'
  if (normalized.includes('webp')) return 'webp'
  if (normalized.includes('mp3') || normalized.includes('mpeg')) return 'mp3'
  if (normalized.includes('wav')) return 'wav'
  if (normalized.includes('ogg')) return 'ogg'
  if (normalized.includes('mp4')) return 'mp4'
  if (normalized.includes('webm')) return 'webm'
  return fallback
}

const getImageBedRoot = async () => {
  if (String(imageBedRootCache.value || '').trim()) {
    return String(imageBedRootCache.value || '').trim()
  }
  const root = String(await invoke('get_image_bed_root') || '').trim()
  imageBedRootCache.value = root
  return root
}

const buildResourceRepoTarget = (cellId, extension) => {
  const now = new Date()
  const year = String(now.getFullYear())
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  const stamp = [
    year,
    month,
    day,
    '_',
    String(now.getHours()).padStart(2, '0'),
    String(now.getMinutes()).padStart(2, '0'),
    String(now.getSeconds()).padStart(2, '0'),
    '_',
    String(now.getMilliseconds()).padStart(3, '0'),
  ].join('')
  const safeStem = getNotebookDocStem()
  const safeCellId = sanitizeFileSegment(cellId, 'cell')
  const safeExt = String(extension || 'bin').replace(/^\.+/, '') || 'bin'
  const fileName = `${safeStem}-${safeCellId}-${stamp}.${safeExt}`
  const relativePath = joinNotebookPath('images', year, month, day, fileName)
  return {
    fileName,
    relativePath,
  }
}

const registerResourceFile = async ({
  bytes,
  resourceType,
  extension,
  ownerFilePath,
  cellId,
  source,
}) => {
  const root = await getImageBedRoot()
  if (!root) {
    throw new Error('资源库根目录不可用')
  }
  const target = buildResourceRepoTarget(cellId, extension)
  const absolutePath = joinNotebookPath(root, target.relativePath)
  await writeBytesToPath(absolutePath, bytes, 'resource-repo-register')
  await invoke('add_resource', {
    resource: {
      resource_type: String(resourceType || 'file'),
      file_name: target.fileName,
      relative_path: target.relativePath,
      absolute_path: absolutePath,
      owner_file_path: String(ownerFilePath || ''),
      created_at: '',
      extra: JSON.stringify({
        source: source || 'python-cell-output',
        cell_id: String(cellId || ''),
      }),
    },
  })
  const url = `http://localhost:8888/markdown/api/bed/${encodeURIComponent(target.relativePath)}`
  logger.info('[Notebook] registered resource file for markdown writeback', {
    cellId: String(cellId || ''),
    resourceType: String(resourceType || 'file'),
    absolutePath,
    relativePath: target.relativePath,
    url,
  })
  return {
    url,
    relativePath: target.relativePath,
    absolutePath,
  }
}
const normalizeDisplayType = (display) => {
  const explicitType = String(display?.type || '').trim().toLowerCase()
  const mime = String(display?.mime || '').trim().toLowerCase()
  if (explicitType) return explicitType
  if (mime.startsWith('image/')) return 'image'
  if (mime.startsWith('audio/')) return 'audio'
  if (mime.startsWith('video/')) return 'video'
  if (mime.includes('markdown')) return 'markdown'
  if (mime.includes('html')) return 'html'
  return ''
}
const getDisplaySourcePath = (display) => String(
  display?.path ||
  display?.file_path ||
  display?.src ||
  display?.source_path ||
  display?.url ||
  '',
).trim()
const getDisplayDataUrl = (display) => {
  const directDataUrl = String(display?.dataUrl || display?.data_url || '').trim()
  if (directDataUrl.startsWith('data:')) return directDataUrl
  const base64 = String(display?.data_base64 || display?.base64 || '').trim()
  if (!base64) return ''
  return `data:${String(display?.mime || 'application/octet-stream')};base64,${base64}`
}

const decorateNotebookCells = (cells = []) => {
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
      cellId: createPythonCellId(props.filePath, {
        ...(block || {}),
        meta: cell.meta,
      }),
    }
  })
}

const buildNotebookSidecarPayload = (cells = [], markdown = '') => {
  const normalizedCells = decorateNotebookCells(cells)
  const sourceCore = normalizeNotebookSourceForSidecar(markdown)
  return {
    version: 2,
    filePath: String(props.filePath || ''),
    updatedAt: new Date().toISOString(),
    sourceDigest: computeNotebookDigest(sourceCore),
    cells: serializeNotebookCellsForSidecar(normalizedCells),
    pythonSnapshots: serializeNotebookSnapshotsForSidecar(normalizedCells),
  }
}

const persistNotebookSidecar = async (reason = 'unknown') => {
  const sidecarPath = getNotebookSidecarPath()
  if (!sidecarPath) return
  const payload = buildNotebookSidecarPayload(
    notebookCells.value,
    lastEmittedMarkdown.value || props.markdown,
  )
  await invoke('write_file_content', {
    path: sidecarPath,
    content: JSON.stringify(payload, null, 2),
  })
  logger.info('[Notebook] persisted sidecar file', {
    reason,
    sidecarPath,
    cellCount: Array.isArray(payload.cells) ? payload.cells.length : 0,
    snapshotCount: Object.keys(payload.pythonSnapshots || {}).length,
  })
}

const schedulePersistNotebookSidecar = (reason = 'unknown', delay = 220) => {
  if (notebookSidecarPersistTimer) {
    window.clearTimeout(notebookSidecarPersistTimer)
  }
  notebookSidecarPersistTimer = window.setTimeout(() => {
    notebookSidecarPersistTimer = 0
    persistNotebookSidecar(reason).catch((error) => {
      logger.warn('[Notebook] persist sidecar failed', {
        reason,
        sidecarPath: getNotebookSidecarPath(),
        error: String(error?.message || error),
      })
    })
  }, delay)
}

const loadNotebookSidecar = async (markdown, reason = 'unknown') => {
  const sidecarPath = getNotebookSidecarPath()
  if (!sidecarPath) return null
  try {
    const raw = await invoke('read_file_content', { path: sidecarPath })
    const parsed = JSON.parse(String(raw || '{}'))
    const cells = Array.isArray(parsed?.cells) ? parsed.cells : []
    const sourceDigest = String(parsed?.sourceDigest || '').trim()
    const currentDigest = computeNotebookDigest(normalizeNotebookSourceForSidecar(markdown))
    if (!cells.length) {
      logger.info('[Notebook] ignore empty sidecar payload', {
        reason,
        sidecarPath,
      })
      return null
    }
    const digestMatch = !!sourceDigest && sourceDigest === currentDigest
    logger.info('[Notebook] loaded sidecar file', {
      reason,
      sidecarPath,
      cellCount: cells.length,
      digestMatch,
      sourceDigest,
      currentDigest,
      snapshotCount: Object.keys(parsed?.pythonSnapshots || {}).length,
    })
    return {
      ...parsed,
      digestMatch,
      currentDigest,
    }
  } catch (error) {
    const messageText = String(error?.message || error || '')
    if (/not found|系统找不到指定的文件|cannot find the file/i.test(messageText)) {
      logger.debug('[Notebook] sidecar file does not exist yet', {
        reason,
        sidecarPath,
      })
      return null
    }
    logger.warn('[Notebook] failed to load sidecar file', {
      reason,
      sidecarPath,
      error: messageText,
    })
    return null
  }
}

// 设计说明：
// 1. 探索模式需要允许“空 cell”暂时存在，等待用户填写，因此不能在每次 props.markdown 更新时都重新解析覆盖本地 cell 状态。
// 2. 这里把 notebookCells 作为本地状态维护，只在外部文档真正发生变化时再重新同步，既保留空 cell，也能明显减少重复切分带来的卡顿。
const syncNotebookCellsFromMarkdown = async (markdown, reason = 'external-sync', { force = false } = {}) => {
  flushPendingNotebookMarkdown(`before-sync:${reason}`)
  const normalizedMarkdown = normalizeNotebookMarkdownText(markdown)
  if (!force && normalizedMarkdown === lastEmittedMarkdown.value) {
    logger.debug('[Notebook] skip notebook cells sync', {
      reason,
      markdownLength: normalizedMarkdown.length,
    })
    return
  }

  const loadVersion = notebookSidecarLoadVersion.value + 1
  notebookSidecarLoadVersion.value = loadVersion
  const sidecar = await loadNotebookSidecar(normalizedMarkdown, reason)
  if (loadVersion !== notebookSidecarLoadVersion.value) {
    logger.debug('[Notebook] skip stale sidecar hydration result', {
      reason,
      loadVersion,
      latestVersion: notebookSidecarLoadVersion.value,
    })
    return
  }
  let nextCells = []
  let sidecarDigestMatch = !!sidecar?.digestMatch
  let restoredFromSidecar = !!sidecar?.cells?.length
  try {
    const backendResult = await invoke('notebook_sync_from_markdown', {
      markdown: normalizedMarkdown,
      sidecarJson: sidecar ? JSON.stringify(sidecar) : null,
    })
    nextCells = decorateNotebookCells(Array.isArray(backendResult?.cells) ? backendResult.cells : [])
    sidecarDigestMatch = !!backendResult?.sidecar_digest_match
    restoredFromSidecar = !!backendResult?.restored_from_sidecar
    logger.info('[Notebook] synced notebook cells via backend command', {
      reason,
      cellCount: nextCells.length,
      sidecarDigestMatch,
      restoredFromSidecar,
    })
  } catch (error) {
    logger.warn('[Notebook] backend notebook sync failed, fallback to frontend parser', {
      reason,
      error: String(error?.message || error),
    })
    const scannedCells = createNotebookCellsFromMarkdown(normalizedMarkdown, {
      filePath: props.filePath,
    })
    nextCells = decorateNotebookCells(reconcileNotebookCellsWithSidecar(scannedCells, sidecar))
  }

  notebookCells.value = nextCells
  notebookHistoryEntries.value = [{
    signature: serializeNotebookCells(cloneNotebookCellsForHistory(nextCells)),
    cells: cloneNotebookCellsForHistory(nextCells),
  }]
  notebookHistoryIndex.value = 0
  notebookHistoryApplying.value = false
  applyNotebookSidecarSnapshots(nextCells, sidecar?.pythonSnapshots || {})
  lastEmittedMarkdown.value = normalizedMarkdown

  logger.info('[Notebook] sync notebook cells from markdown', {
    reason,
    markdownLength: normalizedMarkdown.length,
    cellCount: nextCells.length,
    restoredFromSidecar,
    sidecarDigestMatch,
  })
  if (sidecar && !sidecarDigestMatch) {
    schedulePersistNotebookSidecar('rescan-source-markdown', 40)
  }
}

const getNotebookBasePath = () => {
  const filePath = String(props.filePath || '').trim()
  if (!filePath) return ''
  const normalized = filePath.replace(/\\/g, '/')
  const lastSlashIndex = normalized.lastIndexOf('/')
  return lastSlashIndex >= 0 ? normalized.slice(0, lastSlashIndex) : ''
}

const buildMarkdownRenderOptions = () => ({
  preserveOriginal: false,
  mermaid: true,
  math: true,
  codeHighlight: true,
  buttons: false,
  pythonCells: false,
  codeCells: false,
  theme: props.theme,
  basePath: getNotebookBasePath(),
})

const getNotebookSerializedLineCount = (content) => String(content || '').replace(/\r\n?/g, '\n').split('\n').length

const buildNotebookCellStartLineMap = (cells = []) => {
  const normalizedCells = Array.isArray(cells) ? cells : []
  const lineMap = new Map()
  let currentLine = 1
  let previousChunk = ''

  normalizedCells.forEach((cell) => {
    const cellKey = String(cell?.key || '')
    if (!cellKey) return

    const cellContent = cell?.type === 'python'
      ? serializeNotebookCells([cell])
      : String(cell?.content || '')

    // 设计说明：
    // 1. 探索者模式的大纲跳转仍然要对应“序列化后的 markdown 行号”，这样右侧大纲、顶部面包屑与原始文档保持同一套定位语义。
    // 2. 这里复用 notebook 序列化时的拼接规则：当相邻两个 chunk 之间没有显式换行时，补一个空行分隔。
    const needsSeparator =
      previousChunk &&
      !previousChunk.endsWith('\n') &&
      !cellContent.startsWith('\n')

    if (needsSeparator) {
      currentLine += 2
    }

    lineMap.set(cellKey, currentLine)
    currentLine += getNotebookSerializedLineCount(cellContent) - 1
    previousChunk = cellContent
  })

  return lineMap
}

const notebookOutlineEntries = computed(() => {
  const result = []
  const cellStartLineMap = buildNotebookCellStartLineMap(notebookCells.value)

  for (const cell of Array.isArray(notebookCells.value) ? notebookCells.value : []) {
    if (cell?.type !== 'markdown') continue

    const cellKey = String(cell.key || '')
    const content = String(cell.content || '')
    const baseLine = Number(cellStartLineMap.get(cellKey) || 1)
    const tokens = notebookOutlineMarkdownParser.parse(content, {})
    let cellHeadingIndex = 0

    tokens.forEach((token, tokenIndex) => {
      if (token.type !== 'heading_open') return

      const level = Number(String(token.tag || '').slice(1))
      const nextToken = tokens[tokenIndex + 1]
      const text = String(nextToken?.content || '').trim()
      if (!Number.isFinite(level) || level <= 0 || !text) return

      const line = token.map ? baseLine + Number(token.map[0] || 0) : baseLine
      result.push({
        level,
        text,
        line,
        cellKey,
        outlineId: `nb-outline-${cellKey}-${cellHeadingIndex}`,
      })
      cellHeadingIndex += 1
    })
  }

  return result
})

const notebookOutlineEntriesByCellKey = computed(() => {
  const bucket = new Map()
  for (const entry of notebookOutlineEntries.value) {
    const cellKey = String(entry?.cellKey || '')
    if (!cellKey) continue
    const current = bucket.get(cellKey) || []
    current.push(entry)
    bucket.set(cellKey, current)
  }
  return bucket
})

const createNotebookOutlinePayload = () => {
  return notebookOutlineEntries.value.map((item) => ({
    level: Number(item.level || 0),
    text: String(item.text || ''),
    line: Number(item.line || 0),
  }))
}

const decorateNotebookHeadingAnchors = (html, cellKey) => {
  const rawHtml = String(html || '')
  if (!rawHtml.trim()) return ''

  const outlineEntries = notebookOutlineEntriesByCellKey.value.get(String(cellKey || '')) || []
  if (!outlineEntries.length) return rawHtml

  const container = document.createElement('div')
  container.innerHTML = rawHtml

  const headings = Array.from(container.querySelectorAll('h1, h2, h3, h4, h5, h6'))
  headings.forEach((headingNode, index) => {
    if (!(headingNode instanceof HTMLElement)) return
    const entry = outlineEntries[index]
    if (!entry) return
    headingNode.setAttribute('data-guaxi-outline-id', String(entry.outlineId || ''))
    headingNode.setAttribute('data-guaxi-outline-line', String(entry.line || ''))
  })

  return container.innerHTML
}

// 设计说明：
// 1. Worker 负责 markdown-it、代码高亮和目录/任务列表这类纯字符串计算，先把最耗时的一段移出 UI 线程。
// 2. Mermaid SVG 和数学公式节点整理还依赖 DOM，因此保留在主线程做轻量收尾。
// 3. 如果 Worker 初始化或运行失败，会自动回退到原先的主线程渲染，避免探索模式不可用。
const initNotebookMarkdownWorker = () => {
  if (notebookMarkdownWorkerRef.value) return notebookMarkdownWorkerRef.value
  try {
    const worker = new Worker(new URL('../../workers/notebookMarkdownRenderWorker.js', import.meta.url), {
      type: 'module',
    })
    worker.onmessage = (event) => {
      const payload = event?.data || {}
      const taskId = Number(payload?.id || 0)
      const task = notebookMarkdownWorkerPendingTasks.get(taskId)
      if (!task) return
      notebookMarkdownWorkerPendingTasks.delete(taskId)
      if (payload?.ok) {
        task.resolve(String(payload?.html || ''))
        return
      }
      task.reject(new Error(String(payload?.error || 'Notebook markdown worker failed')))
    }
    worker.onerror = (error) => {
      notebookMarkdownWorkerReady.value = false
      logger.error('[Notebook] markdown worker runtime error', {
        error: String(error?.message || error),
      })
    }
    notebookMarkdownWorkerRef.value = worker
    notebookMarkdownWorkerReady.value = true
    logger.info('[Notebook] markdown worker initialized')
    return worker
  } catch (error) {
    notebookMarkdownWorkerReady.value = false
    logger.warn('[Notebook] failed to initialize markdown worker, fallback to main thread', {
      error: String(error?.message || error),
    })
    return null
  }
}

const disposeNotebookMarkdownWorker = (reason = 'dispose') => {
  if (notebookMarkdownWorkerRef.value) {
    notebookMarkdownWorkerRef.value.terminate()
  }
  notebookMarkdownWorkerRef.value = null
  notebookMarkdownWorkerReady.value = false
  if (notebookMarkdownWorkerPendingTasks.size) {
    for (const [, task] of notebookMarkdownWorkerPendingTasks) {
      task.reject(new Error(`Notebook markdown worker disposed: ${reason}`))
    }
    notebookMarkdownWorkerPendingTasks.clear()
  }
  logger.info('[Notebook] markdown worker disposed', {
    reason,
  })
}

const renderMarkdownHtmlWithWorker = (content, reason = 'unknown') => {
  const markdownText = String(content || '')
  if (!markdownText.trim()) return Promise.resolve('')
  const worker = notebookMarkdownWorkerRef.value || initNotebookMarkdownWorker()
  if (!worker) {
    return Promise.reject(new Error('Notebook markdown worker unavailable'))
  }
  const taskId = ++notebookMarkdownWorkerTaskId
  return new Promise((resolve, reject) => {
    notebookMarkdownWorkerPendingTasks.set(taskId, { resolve, reject })
    worker.postMessage({
      id: taskId,
      content: markdownText,
      options: buildMarkdownRenderOptions(),
      reason,
    })
  })
}

const finalizeNotebookRenderedMath = (container) => {
  container
    .querySelectorAll('.math-inline-container, .math-block-container')
    .forEach((element) => {
      const mathNode = element.querySelector('mjx-container')
      if (!(mathNode instanceof HTMLElement)) return
      mathNode.setAttribute('data-md-original', element.getAttribute('data-md-original') || '')
      mathNode.classList.add(
        element.classList.contains('math-inline-container')
          ? 'math-inline'
          : 'math-block',
      )
      element.replaceWith(mathNode)
    })
}

const finalizeNotebookRenderedMermaid = async (container, options = {}, reason = 'unknown') => {
  if (!options?.mermaid) return
  const mermaidFontFamily = '"Trebuchet MS", "Noto Sans SC", "Microsoft YaHei", "PingFang SC", "Helvetica Neue", Arial, sans-serif'
  mermaid.initialize({
    startOnLoad: false,
    // 设计说明：
    // Mermaid 图表延续项目里既有的“始终亮色渲染”策略，
    // 避免在探索者模式里因为外层深浅主题切换而得到不同配色的图。
    theme: 'default',
    securityLevel: 'loose',
    themeVariables: {
      background: '#ffffff',
      fontFamily: mermaidFontFamily,
    },
  })

  const mermaidElements = Array.from(container.querySelectorAll('.mermaid'))
  for (const element of mermaidElements) {
    try {
      const source = String(element.textContent || '')
        .replace(/&gt;/g, '>')
        .replace(/&lt;/g, '<')
        .replace(/&amp;/g, '&')
      if (!source.trim()) continue

      const renderId = `guaxi-notebook-mermaid-${Date.now()}-${notebookMermaidRenderCounter++}`
      const rendered = await Promise.resolve(mermaid.render(renderId, source))
      const svg = typeof rendered === 'string' ? rendered : String(rendered?.svg || '')
      if (!svg.trim()) continue

      const wrapper = document.createElement('div')
      wrapper.className = 'mermaid'
      wrapper.setAttribute('data-processed', 'true')
      wrapper.setAttribute('data-mermaid-source', source)
      const originalSource = element.getAttribute('data-md-original')
      if (originalSource) {
        wrapper.setAttribute('data-md-original', originalSource)
      }
      wrapper.innerHTML = svg
      const svgElement = wrapper.querySelector('svg')
      if (svgElement instanceof SVGElement) {
        svgElement.style.fontFamily = mermaidFontFamily
        svgElement.setAttribute('font-family', mermaidFontFamily)
        svgElement.querySelectorAll('text, tspan, foreignObject, foreignObject *').forEach((node) => {
          if (!(node instanceof Element)) return
          node.setAttribute('font-family', mermaidFontFamily)
          node.setAttribute('style', `${String(node.getAttribute('style') || '').trim()};font-family:${mermaidFontFamily};`)
        })
      }
      element.replaceWith(wrapper)

      if (typeof rendered?.bindFunctions === 'function') {
        try {
          rendered.bindFunctions(wrapper)
        } catch (bindError) {
          logger.debug('[Notebook] mermaid bindFunctions failed', {
            reason,
            error: String(bindError?.message || bindError),
          })
        }
      }
    } catch (error) {
      logger.error('[Notebook] finalize mermaid failed', {
        reason,
        error: String(error?.message || error),
      })
    }
  }
}

const finalizeNotebookRenderedHtml = async (html, reason = 'unknown') => {
  const rawHtml = String(html || '')
  if (!rawHtml.trim()) return ''
  const container = document.createElement('div')
  container.innerHTML = rawHtml
  const options = buildMarkdownRenderOptions()
  await finalizeNotebookRenderedMermaid(container, options, reason)
  if (options.math) {
    finalizeNotebookRenderedMath(container)
  }
  return container.innerHTML
}

const notebookSearchMatchCountText = computed(() => {
  const total = Array.isArray(notebookSearchDomMatches.value) && notebookSearchDomMatches.value.length
    ? notebookSearchDomMatches.value.length
    : Array.isArray(notebookSearchCellMatches.value)
      ? notebookSearchCellMatches.value.length
      : 0
  if (!total) return '0 / 0'
  const index = Math.max(0, Number(notebookSearchActiveIndex.value || 0))
  return `${Math.min(index + 1, total)} / ${total}`
})

const notebookSearchMatchKeySet = computed(() => {
  const keySet = new Set()
  ;(Array.isArray(notebookSearchCellMatches.value) ? notebookSearchCellMatches.value : [])
    .map((item) => String(item?.cell_key || ''))
    .filter(Boolean)
    .forEach((key) => keySet.add(key))

  ;(Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value : [])
    .map((node) => String(node?.closest?.('.guaxi-notebook__cell')?.getAttribute?.('data-cell-key') || ''))
    .filter(Boolean)
    .forEach((key) => keySet.add(key))

  return keySet
})

const activeNotebookSearchMatch = computed(() => {
  const domMatches = Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value : []
  const domActive = domMatches[Number(notebookSearchActiveIndex.value || 0)] || null
  if (domActive instanceof HTMLElement) {
    return {
      cell_key: String(domActive.closest('.guaxi-notebook__cell')?.getAttribute('data-cell-key') || ''),
    }
  }

  return Array.isArray(notebookSearchCellMatches.value)
    ? notebookSearchCellMatches.value[Number(notebookSearchActiveIndex.value || 0)] || null
    : null
})

const isNotebookSearchMatchedCell = (cell) => {
  return notebookSearchMatchKeySet.value.has(String(cell?.key || ''))
}

const isNotebookSearchActiveCell = (cell) => {
  return String(activeNotebookSearchMatch.value?.cell_key || '') === String(cell?.key || '')
}


const resizeTextareaToContent = (target) => {
  if (!(target instanceof HTMLTextAreaElement)) return
  target.style.height = 'auto'
  target.style.overflowY = 'hidden'
  const nextHeight = Math.max(target.scrollHeight, 88)
  target.style.height = `${nextHeight}px`
}

const syncNotebookTextareaHeights = (reason = 'unknown') => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return
  root.querySelectorAll('textarea').forEach((node) => {
    resizeTextareaToContent(node)
  })
}

const clearNotebookSearchDomHighlights = ({ resetIndex = true } = {}) => {
  const root = notebookRootRef.value
  if (root instanceof HTMLElement) {
    root.querySelectorAll('span.guaxi-notebook__search-hit').forEach((mark) => {
      const parent = mark.parentNode
      if (!parent) return
      parent.replaceChild(document.createTextNode(mark.textContent || ''), mark)
      parent.normalize()
    })
  }

  notebookSearchDomMatches.value = []
  if (resetIndex) {
    notebookSearchActiveIndex.value = -1
  }
}

const clearNotebookSearchHighlights = () => {
  clearNotebookSearchDomHighlights()
  notebookSearchCellMatches.value = []
}

// 设计说明：
// 1. Explorer 模式的后端索引继续负责 Cell 级命中定位，避免 Python 输出、变量等非纯 DOM 内容失去可搜索性。
// 2. 前端额外补一层 DOM 文本高亮，让用户在可见内容里能直接看到关键词，而不是只看到 Cell 外轮廓。
// 3. 两层结果共用一套 active index，优先在 DOM 命中间跳转，没有 DOM 命中时再回退到 Cell 级跳转。
const applyNotebookSearchDomHighlights = (reason = 'unknown') => {
  clearNotebookSearchDomHighlights({ resetIndex: false })

  const root = notebookRootRef.value
  const keyword = String(notebookSearchKeyword.value || '').trim()
  if (!(root instanceof HTMLElement) || !keyword) {
    if (!Array.isArray(notebookSearchCellMatches.value) || !notebookSearchCellMatches.value.length) {
      notebookSearchActiveIndex.value = -1
    }
    return
  }

  const regex = new RegExp(escapeRegExp(keyword), 'gi')
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const text = String(node?.nodeValue || '')
      if (!text) return NodeFilter.FILTER_REJECT
      const parent = node.parentElement
      if (!parent) return NodeFilter.FILTER_REJECT
      if (parent.closest('.guaxi-notebook__search-panel, script, style, textarea, input, button, .monaco-editor')) {
        return NodeFilter.FILTER_REJECT
      }
      if (parent.closest('.guaxi-notebook__search-hit')) return NodeFilter.FILTER_REJECT
      regex.lastIndex = 0
      return regex.test(text) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT
    },
  })

  const textNodes = []
  let currentNode = walker.nextNode()
  while (currentNode) {
    textNodes.push(currentNode)
    currentNode = walker.nextNode()
  }

  const matches = []
  textNodes.forEach((node) => {
    const text = String(node.nodeValue || '')
    const parent = node.parentNode
    if (!parent) return

    regex.lastIndex = 0
    let lastIndex = 0
    let matched = regex.exec(text)
    if (!matched) return

    const fragment = document.createDocumentFragment()
    while (matched) {
      const index = Number(matched.index || 0)
      const matchedText = String(matched[0] || '')
      if (index > lastIndex) {
        fragment.appendChild(document.createTextNode(text.slice(lastIndex, index)))
      }
      const span = document.createElement('span')
      span.className = 'guaxi-notebook__search-hit'
      span.textContent = matchedText
      fragment.appendChild(span)
      matches.push(span)
      lastIndex = index + matchedText.length
      matched = regex.exec(text)
    }
    if (lastIndex < text.length) {
      fragment.appendChild(document.createTextNode(text.slice(lastIndex)))
    }
    parent.replaceChild(fragment, node)
  })

  notebookSearchDomMatches.value = matches
  logger.info('[Notebook] apply DOM search highlights', {
    reason,
    keywordLength: keyword.length,
    domMatchCount: matches.length,
    cellMatchCount: Array.isArray(notebookSearchCellMatches.value) ? notebookSearchCellMatches.value.length : 0,
  })

  if (matches.length) {
    const nextIndex = notebookSearchActiveIndex.value >= 0
      ? Math.min(notebookSearchActiveIndex.value, matches.length - 1)
      : 0
    focusNotebookSearchMatchAt(nextIndex, { scroll: false })
    return
  }

  if (Array.isArray(notebookSearchCellMatches.value) && notebookSearchCellMatches.value.length) {
    notebookSearchActiveIndex.value = Math.min(
      Math.max(0, Number(notebookSearchActiveIndex.value || 0)),
      notebookSearchCellMatches.value.length - 1,
    )
    return
  }

  notebookSearchActiveIndex.value = -1
}

const getNotebookSearchIndexKey = () => {
  const filePath = String(props.filePath || '').trim()
  return filePath || '__guaxi_notebook_memory__'
}

const stripHtmlTags = (html = '') => {
  return String(html || '')
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

const buildNotebookSearchDocuments = (cells = []) => {
  return (Array.isArray(cells) ? cells : []).map((cell) => {
    if (cell?.type === 'python') {
      const snapshot = getCellSnapshot(cell)
      const dataframeMarkdown = snapshot?.dataframe ? dataframeToMarkdownTable(snapshot.dataframe) : ''
      const displayMarkdown = snapshot?.display?.type === 'markdown'
        ? String(snapshot.display.markdown || '')
        : snapshot?.display?.type === 'html'
          ? stripHtmlTags(String(snapshot.display.html || ''))
          : snapshot?.display?.type === 'svg'
            ? stripHtmlTags(String(snapshot.display.svg || ''))
            : ''
      const sessionText = Array.isArray(snapshot?.sessionVars)
        ? snapshot.sessionVars
          .map((item) => `${item?.name || ''} ${item?.value_type || item?.type || ''} ${item?.preview || ''}`.trim())
          .join('\n')
        : ''
      const parts = [
        String(cell?.meta?.title || ''),
        String(cell?.code || ''),
        String(snapshot?.outputText || ''),
        dataframeMarkdown,
        displayMarkdown,
        sessionText,
      ].filter(Boolean)
      const searchText = parts.join('\n\n')
      return {
        cell_key: String(cell?.key || ''),
        cell_type: 'python',
        digest: computeNotebookDigest(searchText),
        search_text: searchText,
        summary_text: [String(cell?.meta?.title || ''), String(snapshot?.outputText || ''), displayMarkdown]
          .filter(Boolean)
          .join(' ')
          .trim(),
      }
    }

    const content = String(cell?.content || '')
    return {
      cell_key: String(cell?.key || ''),
      cell_type: 'markdown',
      digest: computeNotebookDigest(content),
      search_text: content,
      summary_text: content,
    }
  }).filter((item) => item.cell_key)
}

const updateNotebookSearchIndex = async (reason = 'unknown') => {
  const indexKey = getNotebookSearchIndexKey()
  const documents = buildNotebookSearchDocuments(notebookCells.value)
  try {
    const result = await invoke('notebook_update_search_index', {
      filePath: indexKey,
      documents,
    })
    notebookSearchIndexReady.value = true
    logger.info('[Notebook] updated backend search index', {
      reason,
      filePath: indexKey,
      documentCount: Number(result?.document_count || documents.length),
      changedCount: Number(result?.changed_count || 0),
      reusedCount: Number(result?.reused_count || 0),
    })
  } catch (error) {
    notebookSearchIndexReady.value = false
    logger.warn('[Notebook] update backend search index failed', {
      reason,
      error: String(error?.message || error),
    })
  }
}

const scheduleNotebookSearchIndexUpdate = (reason = 'unknown', delay = 120) => {
  if (notebookSearchIndexSyncTimer) {
    window.clearTimeout(notebookSearchIndexSyncTimer)
  }
  notebookSearchIndexSyncTimer = window.setTimeout(() => {
    notebookSearchIndexSyncTimer = 0
    updateNotebookSearchIndex(reason)
  }, delay)
}

const searchNotebookViaBackend = async (keyword, reason = 'unknown') => {
  const normalizedKeyword = String(keyword || '').trim()
  if (!normalizedKeyword) {
    clearNotebookSearchHighlights()
    return
  }

  if (!notebookSearchIndexReady.value) {
    await updateNotebookSearchIndex(`${reason}:warmup`)
  }

  const indexKey = getNotebookSearchIndexKey()
  try {
    const response = await invoke('notebook_search', {
      filePath: indexKey,
      query: normalizedKeyword,
      limit: 200,
    })
    notebookSearchCellMatches.value = Array.isArray(response?.results) ? response.results : []
    notebookSearchActiveIndex.value = notebookSearchCellMatches.value.length ? 0 : -1
    logger.info('[Notebook] backend search finished', {
      reason,
      filePath: indexKey,
      keywordLength: normalizedKeyword.length,
      matchCount: notebookSearchCellMatches.value.length,
      totalMatches: Number(response?.total_matches || 0),
    })
  } catch (error) {
    logger.warn('[Notebook] backend search failed', {
      reason,
      error: String(error?.message || error),
    })
    const lowerKeyword = normalizedKeyword.toLowerCase()
    const fallbackResults = buildNotebookSearchDocuments(notebookCells.value)
      .map((document) => {
        const lowerText = String(document.search_text || '').toLowerCase()
        const matchCount = lowerText ? lowerText.split(lowerKeyword).length - 1 : 0
        if (matchCount <= 0) return null
        return {
          cell_key: document.cell_key,
          cell_type: document.cell_type,
          match_count: matchCount,
          snippet: String(document.summary_text || document.search_text || '').slice(0, 120),
        }
      })
      .filter(Boolean)
    notebookSearchCellMatches.value = fallbackResults
    notebookSearchActiveIndex.value = notebookSearchCellMatches.value.length ? 0 : -1
  }

  await nextTick()
  applyNotebookSearchDomHighlights(`${reason}:dom-highlight`)
}

const getNotebookSearchNavigationTargets = () => {
  const domMatches = (Array.isArray(notebookSearchDomMatches.value) ? notebookSearchDomMatches.value : [])
    .filter((node) => node instanceof HTMLElement && node.isConnected)

  if (domMatches.length) {
    if (domMatches.length !== notebookSearchDomMatches.value.length) {
      notebookSearchDomMatches.value = domMatches
    }
    return {
      mode: 'dom',
      matches: domMatches,
    }
  }

  return {
    mode: 'cell',
    matches: Array.isArray(notebookSearchCellMatches.value) ? notebookSearchCellMatches.value : [],
  }
}

const scheduleNotebookSearchQuery = (reason = 'unknown', delay = 90) => {
  if (notebookSearchQueryTimer) {
    window.clearTimeout(notebookSearchQueryTimer)
  }
  notebookSearchQueryTimer = window.setTimeout(() => {
    notebookSearchQueryTimer = 0
    searchNotebookViaBackend(notebookSearchKeyword.value, reason)
  }, delay)
}

const focusNotebookSearchMatchAt = (index, { scroll = true } = {}) => {
  const navigation = getNotebookSearchNavigationTargets()
  const matches = navigation.matches
  const total = matches.length
  if (!total) {
    notebookSearchActiveIndex.value = -1
    return
  }

  let nextIndex = Number(index)
  if (!Number.isFinite(nextIndex)) nextIndex = 0
  if (nextIndex < 0) nextIndex = total - 1
  if (nextIndex >= total) nextIndex = 0

  notebookSearchActiveIndex.value = nextIndex
  if (navigation.mode === 'dom') {
    matches.forEach((node) => node.classList.remove('guaxi-notebook__search-hit--active'))
    const activeNode = matches[nextIndex]
    activeNode.classList.add('guaxi-notebook__search-hit--active')
    if (!scroll) return
    activeNode.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' })
    logger.debug('[Notebook] focus DOM search match', {
      index: nextIndex,
      total,
      cellKey: String(activeNode.closest('.guaxi-notebook__cell')?.getAttribute('data-cell-key') || ''),
    })
    return
  }

  if (!scroll) return

  nextTick(() => {
    const active = matches[nextIndex]
    const activeCellKey = String(active?.cell_key || '')
    if (!activeCellKey) return
    const target = notebookRootRef.value?.querySelector?.(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(activeCellKey)}"]`)
    if (target instanceof HTMLElement) {
      target.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'smooth' })
      logger.debug('[Notebook] focus cell search match', {
        index: nextIndex,
        total,
        cellKey: activeCellKey,
      })
    }
  })
}

const openNotebookSearch = async () => {
  notebookSearchVisible.value = true
  logger.info('[Notebook] open search panel')
  await nextTick()
  const input = notebookSearchInputRef.value
  if (input?.focus) input.focus()
  if (input?.select) input.select()
  scheduleNotebookSearchQuery('open-search', 0)
}

const closeNotebookSearch = () => {
  notebookSearchVisible.value = false
  notebookSearchKeyword.value = ''
  logger.info('[Notebook] close search panel')
  clearNotebookSearchHighlights()
}

const focusNextNotebookSearchMatch = () => {
  focusNotebookSearchMatchAt(notebookSearchActiveIndex.value + 1)
}

const focusPrevNotebookSearchMatch = () => {
  focusNotebookSearchMatchAt(notebookSearchActiveIndex.value - 1)
}

const setMarkdownEditorRef = (key, instance) => {
  const normalizedKey = String(key || '')
  if (!normalizedKey) return
  if (instance) {
    markdownEditorRefMap.set(normalizedKey, instance)
    return
  }
  markdownEditorRefMap.delete(normalizedKey)
}

const syncMarkdownEditorValueToCell = (key, reason = 'sync-markdown-editor-value') => {
  const normalizedKey = String(key || '')
  if (!normalizedKey) return false

  const editorRef = markdownEditorRefMap.get(normalizedKey)
  const nextValue = String(editorRef?.getEditorValue?.() || '')
  const currentCell = notebookCells.value.find((cell) => cell?.type === 'markdown' && String(cell.key) === normalizedKey)
  const currentValue = String(currentCell?.content || '')

  if (nextValue === currentValue) {
    logger.debug('[Notebook] skip markdown editor value sync because content is unchanged', {
      key: normalizedKey,
      reason,
      length: nextValue.length,
    })
    return false
  }

  notebookCells.value = notebookCells.value.map((cell) => {
    if (cell?.type !== 'markdown' || String(cell.key) !== normalizedKey) return cell
    return createNotebookMarkdownCell(nextValue, { key: cell.key })
  })

  logger.info('[Notebook] synchronized live Markdown editor value into notebook cell', {
    key: normalizedKey,
    reason,
    previousLength: currentValue.length,
    nextLength: nextValue.length,
  })
  return true
}

const jumpToNotebookOutlineLine = (line, options = {}) => {
  const targetLine = Number(line)
  if (!Number.isFinite(targetLine) || targetLine <= 0) {
    logger.warn('[Notebook] outline jump ignored because line is invalid', {
      line,
    })
    return false
  }

  const outlineEntry = notebookOutlineEntries.value.find((item) => Number(item?.line || 0) === targetLine)
  if (!outlineEntry) {
    logger.warn('[Notebook] outline jump skipped because heading was not found', {
      line: targetLine,
    })
    return false
  }

  const behavior = options?.behavior || 'smooth'
  const outlineId = String(outlineEntry.outlineId || '')
  const cellKey = String(outlineEntry.cellKey || '')

  nextTick(() => {
    const headingTarget = outlineId
      ? notebookRootRef.value?.querySelector?.(`[data-guaxi-outline-id="${CSS.escape(outlineId)}"]`)
      : null
    const fallbackCell = cellKey
      ? notebookRootRef.value?.querySelector?.(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(cellKey)}"]`)
      : null
    const target = headingTarget instanceof HTMLElement
      ? headingTarget
      : fallbackCell instanceof HTMLElement
        ? fallbackCell
        : null

    if (!target) {
      logger.warn('[Notebook] outline jump target missing in DOM', {
        line: targetLine,
        outlineId,
        cellKey,
      })
      return
    }

    target.scrollIntoView({
      block: headingTarget instanceof HTMLElement ? 'start' : 'center',
      inline: 'nearest',
      behavior,
    })

    logger.info('[Notebook] outline jump applied', {
      line: targetLine,
      outlineId,
      cellKey,
    })
  })

  return true
}

const focusMarkdownEditor = (key, reason = 'focus-editor') => {
  nextTick(() => {
    const editorRef = markdownEditorRefMap.get(String(key || ''))
    if (editorRef?.focusEditor) {
      void editorRef.focusEditor({ cursor: 'end' })
      logger.info('[Notebook] focus markdown editor', {
        key: String(key || ''),
        reason,
        mode: 'monaco',
      })
    } else {
      logger.warn('[Notebook] focus markdown editor skipped because editor ref was not found', {
        key: String(key || ''),
        reason,
      })
    }
  })
}

const isMarkdownCellEditing = (cell) => {
  return cell?.type === 'markdown' && String(editingMarkdownCellKey.value || '') === String(cell?.key || '')
}

const enterMarkdownEditMode = (key, reason = 'manual-enter') => {
  const nextKey = String(key || '')
  if (!nextKey) return
  const currentEditingKey = String(editingMarkdownCellKey.value || '')
  if (currentEditingKey && currentEditingKey !== nextKey) {
    syncMarkdownEditorValueToCell(currentEditingKey, 'before-switch-markdown-edit-target')
    flushPendingNotebookMarkdown('switch-markdown-edit-target')
  }
  editingMarkdownCellKey.value = nextKey
  logger.info('[Notebook] enter markdown edit mode', {
    key: nextKey,
    reason,
  })
  focusMarkdownEditor(nextKey, reason)
}

const exitMarkdownEditMode = (key, reason = 'manual-exit') => {
  const targetKey = String(key || '')
  if (String(editingMarkdownCellKey.value || '') !== targetKey) return
  const syncedFromEditor = syncMarkdownEditorValueToCell(targetKey, `before-exit:${reason}`)
  if (syncedFromEditor) {
    flushNotebookMarkdown(`exit-markdown-edit:${reason}:force-sync`)
  } else {
  flushPendingNotebookMarkdown(`exit-markdown-edit:${reason}`)
  }
  editingMarkdownCellKey.value = ''
  logger.info('[Notebook] exit markdown edit mode', {
    key: targetKey,
    reason,
    syncedFromEditor,
  })
}

// 设计说明：
// 1. 直接在每次按键时序列化整个 notebook 并回推父组件，会把探索模式拖得很卡。
// 2. 这里把“本地 cell 状态更新”和“整篇 Markdown 回写”拆开：输入时先只更新本地，再稍后批量 flush。
// 3. 插入、删除、运行写回这类结构性操作仍保持立即同步，避免用户感知到延迟。
const flushNotebookMarkdown = (reason = 'unknown') => {
  const nextMarkdown = serializeNotebookCells(notebookCells.value)
  lastEmittedMarkdown.value = normalizeNotebookMarkdownText(nextMarkdown)
  emit('update:markdown', nextMarkdown)
  logger.info('[Notebook] flush notebook markdown', {
    reason,
    markdownLength: nextMarkdown.length,
    cellCount: notebookCells.value.length,
  })
}

const scheduleFlushNotebookMarkdown = (reason = 'unknown', delay = 140) => {
  if (notebookMarkdownEmitTimer) {
    window.clearTimeout(notebookMarkdownEmitTimer)
  }
  notebookMarkdownEmitTimer = window.setTimeout(() => {
    notebookMarkdownEmitTimer = 0
    flushNotebookMarkdown(reason)
  }, delay)
}

const flushPendingNotebookMarkdown = (reason = 'unknown') => {
  if (!notebookMarkdownEmitTimer) return
  window.clearTimeout(notebookMarkdownEmitTimer)
  notebookMarkdownEmitTimer = 0
  flushNotebookMarkdown(reason)
}

const recordNotebookHistory = (cells, reason = 'unknown') => {
  if (notebookHistoryApplying.value) return
  const snapshot = cloneNotebookCellsForHistory(cells)
  const signature = serializeNotebookCells(snapshot)
  const currentEntry = notebookHistoryEntries.value[notebookHistoryIndex.value] || null
  if (currentEntry?.signature === signature) {
    return
  }
  const nextEntries = notebookHistoryEntries.value.slice(0, notebookHistoryIndex.value + 1)
  nextEntries.push({
    signature,
    cells: snapshot,
  })
  notebookHistoryEntries.value = nextEntries.slice(-80)
  notebookHistoryIndex.value = notebookHistoryEntries.value.length - 1
  logger.info('[Notebook] recorded history snapshot', {
    reason,
    historyLength: notebookHistoryEntries.value.length,
    historyIndex: notebookHistoryIndex.value,
    cellCount: snapshot.length,
  })
}

const applyNotebookHistoryAt = (index, reason = 'unknown') => {
  const target = notebookHistoryEntries.value[index] || null
  if (!target) return
  notebookHistoryApplying.value = true
  try {
    emitCells(target.cells, '', {
      deferMarkdownSync: false,
      recordHistory: false,
    })
    notebookHistoryIndex.value = index
    editingMarkdownCellKey.value = ''
    logger.info('[Notebook] applied history snapshot', {
      reason,
      historyIndex: index,
      cellCount: Array.isArray(target.cells) ? target.cells.length : 0,
    })
  } finally {
    notebookHistoryApplying.value = false
  }
}

const undoNotebookHistory = () => {
  if (!canUndoNotebookHistory.value) return
  applyNotebookHistoryAt(notebookHistoryIndex.value - 1, 'undo')
}

const redoNotebookHistory = () => {
  if (!canRedoNotebookHistory.value) return
  applyNotebookHistoryAt(notebookHistoryIndex.value + 1, 'redo')
}

const requestNotebookSave = () => {
  flushPendingNotebookMarkdown('before-request-save')
  emit('request-save')
  logger.info('[Notebook] requested parent save')
}

const emitCells = (cells, focusKey = '', { deferMarkdownSync = false, recordHistory = true } = {}) => {
  pendingFocusCellKey.value = String(focusKey || '')
  const normalizedCells = decorateNotebookCells(cells)
  notebookCells.value = normalizedCells
  logger.info('[Notebook] emit cells', {
    focusKey: pendingFocusCellKey.value,
    cellCount: normalizedCells.length,
    markdownCellCount: normalizedCells.filter((cell) => cell.type === 'markdown').length,
    pythonCellCount: normalizedCells.filter((cell) => cell.type === 'python').length,
    deferMarkdownSync,
    recordHistory,
  })
  if (recordHistory) {
    recordNotebookHistory(normalizedCells, 'emit-cells')
  }
  if (deferMarkdownSync) {
    scheduleFlushNotebookMarkdown('deferred-cell-edit')
  } else {
    flushNotebookMarkdown('emit-cells')
  }
  schedulePersistNotebookSidecar('emit-cells')
  nextTick(() => {
    if (!pendingFocusCellKey.value) return
    const target = document.querySelector(`[data-cell-key="${CSS.escape(pendingFocusCellKey.value)}"]`)
    if (target instanceof HTMLElement) {
      target.focus()
    }
    pendingFocusCellKey.value = ''
  })
}

const updateMarkdownCell = (key, event) => {
  const value = typeof event === 'string'
    ? event
    : String(event?.target?.value || '')
  emitCells(
    notebookCells.value.map((cell) => {
      if (cell.type !== 'markdown' || String(cell.key) !== String(key)) return cell
      return createNotebookMarkdownCell(value, { key: cell.key })
    }),
    '',
    { deferMarkdownSync: true },
  )
}

// 设计说明：
// 1. Markdown 预览里会嵌套 code block、MathJax、Mermaid SVG 和媒体按钮，这些节点可能自己截获 click，
//    导致外层 preview-shell 的 click 根本到不了，于是用户点了“看起来属于 cell 的区域”却进不了编辑态。
// 2. 这里改成在 pointerdown capture 阶段提前识别“非交互性内容点击”，尽量在预览 DOM 还没被切成 Monaco 之前就切换编辑态，
//    让代码块、公式、图表区域都具备一致的进入编辑体验。
// 3. 对真正的交互控件（链接、按钮、媒体查看/保存按钮等）仍然放行，避免用户本来是想点按钮却被强制拉进编辑态。
const isInteractiveMarkdownPreviewTarget = (event) => {
  const eventPath = typeof event?.composedPath === 'function' ? event.composedPath() : []
  return eventPath.some((node) => {
    if (!(node instanceof Element)) return false
    if (
      node.matches?.(
        'button, a[href], input, textarea, select, option, summary, details, [role="button"], [contenteditable="true"]',
      )
    ) {
      return true
    }
    return node.classList?.contains('media-viewer-button')
      || node.classList?.contains('media-save-button')
      || node.classList?.contains('guaxi-notebook__search-btn')
  })
}

const handleMarkdownPreviewPointerDown = (key, event) => {
  const targetKey = String(key || '')
  if (!targetKey) return

  if (event?.button != null && Number(event.button) !== 0) {
    logger.debug('[Notebook] ignore Markdown preview pointerdown because pointer button is not primary', {
      key: targetKey,
      button: Number(event.button),
    })
    return
  }

  if (isInteractiveMarkdownPreviewTarget(event)) {
    logger.debug('[Notebook] keep Markdown preview pointerdown in preview mode because target is interactive', {
      key: targetKey,
    })
    return
  }

  if (String(editingMarkdownCellKey.value || '') === targetKey) {
    return
  }

  logger.info('[Notebook] enter markdown edit mode from preview pointerdown', {
    key: targetKey,
  })
  enterMarkdownEditMode(targetKey, 'preview-pointerdown')
}

const handleDocumentPointerDownForMarkdownEdit = (event) => {
  const activeKey = String(editingMarkdownCellKey.value || '')
  if (!activeKey) return

  const target = event?.target
  if (!(target instanceof Element)) return

  const eventPath = typeof event?.composedPath === 'function' ? event.composedPath() : []
  const isInsideMonacoOverlay = eventPath.some((node) => {
    return node instanceof Element
      && node.matches?.('.suggest-widget, .monaco-editor-hover, .monaco-hover, .context-view, .monaco-menu-container')
  })

  if (isInsideMonacoOverlay || target.closest('.suggest-widget, .monaco-editor-hover, .monaco-hover, .context-view, .monaco-menu-container')) {
    logger.debug('[Notebook] keep markdown edit mode because pointer is inside Monaco overlay', {
      key: activeKey,
    })
    return
  }

  const currentCell = notebookRootRef.value?.querySelector?.(`.guaxi-notebook__cell[data-cell-key="${CSS.escape(activeKey)}"]`)
  const clickedInsideActiveCell = eventPath.some((node) => {
    if (!(node instanceof Element)) return false
    if (currentCell instanceof Element && currentCell.contains(node)) return true
    return String(node.getAttribute?.('data-cell-key') || '') === activeKey
  })

  if (clickedInsideActiveCell || (currentCell instanceof Element && currentCell.contains(target))) {
    logger.debug('[Notebook] keep markdown edit mode because pointer is still inside active cell', {
      key: activeKey,
      targetTag: String(target.tagName || '').toLowerCase(),
    })
    return
  }

  logger.info('[Notebook] exit markdown edit mode because user clicked outside active cell', {
    key: activeKey,
    targetTag: String(target.tagName || '').toLowerCase(),
  })
  exitMarkdownEditMode(activeKey, 'outside-pointerdown')
}

const updatePythonCell = (key, payload) => {
  if (payload?.target) {
    resizeTextareaToContent(payload.target)
  }
  const value = typeof payload === 'string'
    ? payload
    : String(payload?.target?.value || '')
  emitCells(
    notebookCells.value.map((cell) => {
      if (cell.type !== 'python' || String(cell.key) !== String(key)) return cell
      return createNotebookPythonCell({
        ...cell,
        code: value,
      })
    }),
    '',
    { deferMarkdownSync: true },
  )
}

const insertCell = (index, position = 'below', type = 'markdown') => {
  const next = notebookCells.value.slice()
  const newCell = type === 'python'
    ? createNotebookPythonCell({
      code: '',
    })
    : createNotebookMarkdownCell('')
  const offset = position === 'below' ? 1 : 0
  logger.info('[Notebook] insert cell', {
    index,
    position,
    type,
  })
  if (type === 'markdown') {
    editingMarkdownCellKey.value = newCell.key
  }
  next.splice(Math.max(0, index + offset), 0, newCell)
  emitCells(next, newCell.key)
  if (type === 'markdown') {
    focusMarkdownEditor(newCell.key, 'insert-markdown-cell')
  }
}

const appendCell = (type = 'markdown') => {
  insertCell(notebookCells.value.length - 1, 'below', type)
}

const removeCell = (key) => {
  logger.info('[Notebook] remove cell', { key: String(key || '') })
  if (String(editingMarkdownCellKey.value || '') === String(key || '')) {
    editingMarkdownCellKey.value = ''
  }
  const filtered = notebookCells.value.filter((cell) => String(cell.key) !== String(key))
  emitCells(filtered)
}

const confirmRemoveCell = async (cell) => {
  const targetKey = String(cell?.key || '')
  if (!targetKey) return
  const confirmed = await confirmDialog(`确定删除这个${cell?.type === 'python' ? ' Python' : ' Markdown'} Cell 吗？`, {
    title: '确认删除',
    kind: 'warning',
  })
  logger.info('[Notebook] confirm remove cell result', {
    key: targetKey,
    cellType: String(cell?.type || ''),
    confirmed: !!confirmed,
  })
  if (!confirmed) return
  removeCell(targetKey)
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

const getGuaxiDisplayFromResult = (res) => {
  const value = res?.result_json?.__guaxi_display__
  return value && typeof value === 'object' ? value : null
}

const getGuaxiDataFrameFromResult = (res) => {
  const value = res?.result_json?.__guaxi_dataframe__
  if (!value || typeof value !== 'object') return null
  return {
    columns: Array.isArray(value.columns) ? value.columns : [],
    rows: Array.isArray(value.rows) ? value.rows : [],
    rowCount: Number(value.row_count || 0),
    truncated: !!value.truncated,
  }
}

const getCellSnapshot = (cell) => {
  refreshTick.value
  const snapshot = getPythonCellSnapshot(props.filePath, cell.cellId)
  return {
    outputText: String(snapshot?.outputText || ''),
    display: snapshot?.display || null,
    dataframe: snapshot?.dataframe || null,
    result: snapshot?.result || null,
    sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    meta: {
      ...(cell.meta || {}),
      ...(snapshot?.meta || {}),
      outputCollapsed: snapshot?.meta?.outputCollapsed ?? cell.meta?.outputCollapsed ?? false,
      sessionCollapsed: snapshot?.meta?.sessionCollapsed ?? cell.meta?.sessionCollapsed ?? true,
    },
  }
}

const getCellSessionVars = (cell) => Array.isArray(getCellSnapshot(cell).sessionVars) ? getCellSnapshot(cell).sessionVars : []

const getCellDisplayKind = (cell) => normalizeDisplayType(getCellSnapshot(cell).display)

const getCellDisplayUrl = (cell) => String(resolvedDisplayUrlMap.value[String(cell?.key || '')] || '')

const buildDisplayCacheKey = (display) => {
  const kind = normalizeDisplayType(display)
  const source = getDisplaySourcePath(display)
  const dataUrl = getDisplayDataUrl(display)
  const mime = String(display?.mime || '')
  return [
    kind,
    mime,
    source,
    dataUrl ? computeNotebookDigest(dataUrl) : '',
  ].join('|')
}

const resolveDisplayUrl = async (display) => {
  const dataUrl = getDisplayDataUrl(display)
  if (dataUrl) return dataUrl

  const source = getDisplaySourcePath(display)
  if (!source) return ''
  if (/^(https?:|data:|blob:)/i.test(source)) return source

  try {
    const converted = convertFileSrc(source)
    return typeof converted?.then === 'function' ? await converted : String(converted || '')
  } catch (error) {
    logger.warn('[Notebook] convert display source path failed', {
      source,
      error: String(error?.message || error),
    })
    return source
  }
}

const syncResolvedDisplayUrls = async (cells, reason = 'unknown') => {
  const previousMap = resolvedDisplayUrlMap.value || {}
  const nextMap = {}
  const tasks = []
  let cacheHitCount = 0
  let resolvedCount = 0

  for (const cell of Array.isArray(cells) ? cells : []) {
    if (cell?.type !== 'python') continue
    const display = getCellSnapshot(cell).display
    const kind = normalizeDisplayType(display)
    if (!['image', 'audio', 'video'].includes(kind)) continue
    const cacheKey = buildDisplayCacheKey(display)
    if (cacheKey && notebookDisplayUrlCache.has(cacheKey)) {
      nextMap[String(cell.key || '')] = notebookDisplayUrlCache.get(cacheKey)
      cacheHitCount += 1
      continue
    }
    if (cacheKey && previousMap[String(cell.key || '')]) {
      nextMap[String(cell.key || '')] = previousMap[String(cell.key || '')]
      notebookDisplayUrlCache.set(cacheKey, previousMap[String(cell.key || '')])
      cacheHitCount += 1
      continue
    }
    tasks.push((async () => {
      const resolvedUrl = await resolveDisplayUrl(display)
      nextMap[String(cell.key || '')] = resolvedUrl
      if (cacheKey) {
        notebookDisplayUrlCache.set(cacheKey, resolvedUrl)
      }
      resolvedCount += 1
    })())
  }

  await Promise.all(tasks)
  resolvedDisplayUrlMap.value = nextMap
  logger.info('[Notebook] sync display urls', {
    reason,
    mediaCount: Object.keys(nextMap).length,
    cacheHitCount,
    resolvedCount,
  })
}

const hasVisibleOutput = (cell) => {
  const snapshot = getCellSnapshot(cell)
  return !!(
    snapshot.outputText ||
    snapshot.display ||
    snapshot.dataframe
  )
}

const renderMarkdownHtml = async (content, reason = 'unknown') => {
  const markdownText = String(content || '')
  if (!markdownText.trim()) return ''
  const cacheKey = computeNotebookDigest(markdownText)
  if (notebookMarkdownRenderCache.has(cacheKey)) {
    return notebookMarkdownRenderCache.get(cacheKey)
  }
  try {
    let html = ''
    if (notebookMarkdownWorkerReady.value || notebookMarkdownWorkerRef.value) {
      const workerHtml = await renderMarkdownHtmlWithWorker(markdownText, reason)
      html = await finalizeNotebookRenderedHtml(workerHtml, reason)
      logger.info('[Notebook] rendered markdown via worker', {
        reason,
        markdownLength: markdownText.length,
      })
    } else {
      html = await renderMarkdown(markdownText, buildMarkdownRenderOptions())
      logger.info('[Notebook] rendered markdown via main thread fallback', {
        reason,
        markdownLength: markdownText.length,
      })
    }
    notebookMarkdownRenderCache.set(cacheKey, html)
    return html
  } catch (error) {
    logger.warn('[Notebook] worker markdown render failed, fallback to main thread renderer', {
      reason,
      error: String(error?.message || error),
    })
    try {
      const html = await renderMarkdown(markdownText, buildMarkdownRenderOptions())
      notebookMarkdownRenderCache.set(cacheKey, html)
      return html
    } catch (fallbackError) {
      logger.error('[Notebook] render markdown html failed', {
        reason,
        error: String(fallbackError?.message || fallbackError),
      })
      return markdownText
    }
  }
}

// 设计说明：
// Explorer 模式中的 Markdown 预览需要复用主编辑器的渲染链，
// 这样代码高亮、数学公式与其他增强能力才能和主界面保持一致。
const syncRenderedMarkdownHtml = async (cells, reason = 'unknown') => {
  const renderVersion = markdownRenderVersion.value + 1
  markdownRenderVersion.value = renderVersion

  logger.info('[Notebook] sync rendered markdown start', {
    reason,
    renderVersion,
    cellCount: Array.isArray(cells) ? cells.length : 0,
  })

  const previousMarkdownMap = renderedMarkdownHtmlMap.value || {}
  const previousOutputMap = renderedOutputMarkdownHtmlMap.value || {}
  const nextMarkdownMap = {}
  const nextOutputMap = {}
  const renderTasks = []
  let markdownCacheHitCount = 0
  let outputCacheHitCount = 0
  let markdownRenderCount = 0
  let outputRenderCount = 0

  for (const cell of Array.isArray(cells) ? cells : []) {
    if (cell?.type === 'markdown') {
      const cellKey = String(cell.key || '')
      const markdownText = String(cell.content || '')
      const cacheKey = computeNotebookDigest(markdownText)
      if (previousMarkdownMap[cellKey] && notebookMarkdownRenderCache.has(cacheKey)) {
        nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(previousMarkdownMap[cellKey], cellKey)
        markdownCacheHitCount += 1
        continue
      }
      renderTasks.push((async () => {
        const renderedHtml = await renderMarkdownHtml(
          markdownText,
          `markdown-cell:${cellKey}`,
        )
        nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(renderedHtml, cellKey)
        markdownRenderCount += 1
      })())
      continue
    }

    if (cell?.type === 'python') {
      const snapshot = getCellSnapshot(cell)
      const outputMarkdown = snapshot?.display?.type === 'markdown'
        ? String(snapshot.display.markdown || '')
        : ''
      if (!outputMarkdown.trim()) continue
      const cellKey = String(cell.key || '')
      const cacheKey = computeNotebookDigest(outputMarkdown)
      if (previousOutputMap[cellKey] && notebookMarkdownRenderCache.has(cacheKey)) {
        nextOutputMap[cellKey] = previousOutputMap[cellKey]
        outputCacheHitCount += 1
        continue
      }

      renderTasks.push((async () => {
        nextOutputMap[cellKey] = await renderMarkdownHtml(
          outputMarkdown,
          `python-output:${cellKey}`,
        )
        outputRenderCount += 1
      })())
    }
  }

  await Promise.all(renderTasks)

  if (renderVersion !== markdownRenderVersion.value) {
    logger.info('[Notebook] skip stale rendered markdown result', {
      reason,
      renderVersion,
      latestRenderVersion: markdownRenderVersion.value,
    })
    return
  }

  renderedMarkdownHtmlMap.value = nextMarkdownMap
  renderedOutputMarkdownHtmlMap.value = nextOutputMap

  logger.info('[Notebook] sync rendered markdown finished', {
    reason,
    renderVersion,
    markdownCellCount: Object.keys(nextMarkdownMap).length,
    outputMarkdownCount: Object.keys(nextOutputMap).length,
    markdownCacheHitCount,
    outputCacheHitCount,
    markdownRenderCount,
    outputRenderCount,
  })
}

const getRenderedMarkdownCellHtml = (cell) => String(renderedMarkdownHtmlMap.value[String(cell?.key || '')] || '')

const getRenderedOutputMarkdownHtml = (cell) => String(renderedOutputMarkdownHtmlMap.value[String(cell?.key || '')] || '')

const patchCellMeta = (cell, patch) => {
  const snapshot = getCellSnapshot(cell)
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    meta: {
      ...(snapshot.meta || {}),
      ...(patch || {}),
    },
  })
  bumpRefresh()
  schedulePersistNotebookSidecar('patch-cell-meta')
}

const toggleCellOutput = (cell) => {
  const snapshot = getCellSnapshot(cell)
  logger.info('[Notebook] toggle output panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextCollapsed: !snapshot.meta.outputCollapsed,
  })
  patchCellMeta(cell, {
    outputCollapsed: !snapshot.meta.outputCollapsed,
  })
}

const toggleCellSession = (cell) => {
  const snapshot = getCellSnapshot(cell)
  logger.info('[Notebook] toggle session panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextCollapsed: !snapshot.meta.sessionCollapsed,
  })
  patchCellMeta(cell, {
    sessionCollapsed: !snapshot.meta.sessionCollapsed,
  })
}

const isCellBusy = (cell) => !!busyMap.value[String(cell.key)]

const getPythonCellOrderIndex = (cell) => {
  const targetKey = String(cell?.key || '')
  if (!targetKey) return -1
  return notebookCells.value
    .filter((item) => item?.type === 'python')
    .findIndex((item) => String(item?.key || '') === targetKey)
}

// 设计说明：
// 1. Explorer 模式会先在前端内存里生成稳定 meta.id，随后再把它写回隐藏 Python Cell 源码。
// 2. 老文档第一次进入探索模式时，源码块里可能还没有 meta.id，这时如果只按 cellId 回找很容易失败。
// 3. 因此这里优先用稳定 id，找不到时再兼容旧 cellId，最后按当前 Python Cell 的顺序兜底。
const resolveSourcePythonBlock = (cell, markdown) => {
  const blocks = parsePythonCellBlocks(markdown)
  const stableId = String(cell?.meta?.id || '').trim()
  if (stableId) {
    const stableBlock = blocks.find((block) => String(block?.meta?.id || '').trim() === stableId)
    if (stableBlock) {
      return {
        block: stableBlock,
        blocks,
        matchStrategy: 'meta-id',
      }
    }
  }

  const cellId = String(cell?.cellId || '').trim()
  if (cellId) {
    const directBlock = blocks.find((block) => createPythonCellId(props.filePath, block) === cellId)
    if (directBlock) {
      return {
        block: directBlock,
        blocks,
        matchStrategy: 'cell-id',
      }
    }
  }

  const orderIndex = getPythonCellOrderIndex(cell)
  if (orderIndex >= 0 && blocks[orderIndex]) {
    return {
      block: blocks[orderIndex],
      blocks,
      matchStrategy: 'python-order',
    }
  }

  return {
    block: null,
    blocks,
    matchStrategy: 'missing',
  }
}

const ensureSourcePythonBlockHasStableId = (markdown, cell, resolved) => {
  const stableId = String(cell?.meta?.id || '').trim()
  const targetBlock = resolved?.block || null
  if (!stableId || !targetBlock) {
    return {
      markdown,
      block: targetBlock,
      blocks: resolved?.blocks || [],
      persisted: false,
    }
  }

  if (String(targetBlock?.meta?.id || '').trim() === stableId) {
    return {
      markdown,
      block: targetBlock,
      blocks: resolved?.blocks || [],
      persisted: false,
    }
  }

  const nextMarkdown = replacePythonCellBlock(
    markdown,
    targetBlock,
    String(targetBlock.code || ''),
    {
      ...(targetBlock.meta || {}),
      id: stableId,
    },
  )
  const reparsed = resolveSourcePythonBlock(cell, nextMarkdown)
  logger.info('[Notebook] persist stable python cell id into markdown', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    stableId,
    previousMatchStrategy: String(resolved?.matchStrategy || 'unknown'),
    nextMatchStrategy: String(reparsed?.matchStrategy || 'unknown'),
  })
  return {
    markdown: nextMarkdown,
    block: reparsed.block,
    blocks: reparsed.blocks,
    persisted: true,
  }
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

const buildNotebookAssetTarget = (cellId, extension) => {
  const safeStem = getNotebookDocStem()
  const safeCellId = sanitizeFileSegment(cellId, 'cell')
  const safeExt = String(extension || 'bin').replace(/^\.+/, '') || 'bin'
  const fileName = `${safeStem}-${safeCellId}-output.${safeExt}`
  const absolutePath = getNotebookDocDir()
    ? joinNotebookPath(getNotebookDocDir(), fileName)
    : fileName
  return {
    absolutePath,
    relativePath: encodeURI(getRelativeNotebookPath(absolutePath)),
    fileName,
  }
}

const serializeDisplayAssetForMarkdown = async (display, cellId) => {
  const kind = normalizeDisplayType(display)
  const sourcePath = getDisplaySourcePath(display)
  const mime = String(display?.mime || '').trim()
  const dataUrl = getDisplayDataUrl(display)
  let bytes = null
  let assetMime = mime || 'application/octet-stream'

  if (dataUrl) {
    const matched = dataUrl.match(/^data:([^;]+);base64,(.+)$/i)
    if (!matched) {
      return {
        kind,
        reference: dataUrl,
        assetPath: '',
      }
    }
    assetMime = matched[1] || assetMime
    bytes = base64ToUint8Array(matched[2] || '')
  } else if (sourcePath) {
    bytes = await readBytesFromPath(sourcePath, 'markdown-writeback-source-asset')
  } else {
    return null
  }

  const extension = guessExtensionFromMime(
    assetMime,
    kind === 'image' ? 'png' : kind === 'audio' ? 'mp3' : kind === 'video' ? 'mp4' : 'bin',
  )
  const resourceType = kind || 'file'
  const stored = await registerResourceFile({
    bytes,
    resourceType,
    extension,
    ownerFilePath: props.filePath,
    cellId,
    source: 'python-cell-output',
  })
  return {
    kind,
    reference: stored.url,
    assetPath: stored.absolutePath,
  }
}

const buildMarkdownOutputFromSnapshot = async (cell, snapshot) => {
  if (snapshot?.dataframe) {
    return dataframeToMarkdownTable(snapshot.dataframe)
  }

  const display = snapshot?.display
  const displayType = normalizeDisplayType(display)
  if (displayType === 'markdown' && display?.markdown) return String(display.markdown)
  if (displayType === 'html' && display?.html) return String(display.html)
  if (displayType === 'svg' && display?.svg) return String(display.svg)

  if (['image', 'audio', 'video'].includes(displayType)) {
    const asset = await serializeDisplayAssetForMarkdown(display, String(cell?.cellId || cell?.key || ''))
    if (asset?.reference) {
      if (displayType === 'image') {
        return `![Python Cell 输出](${asset.reference})`
      }
      if (displayType === 'audio') {
        return `<audio controls src="${escapeHtmlAttribute(asset.reference)}"></audio>`
      }
      if (displayType === 'video') {
        return `<video controls src="${escapeHtmlAttribute(asset.reference)}"></video>`
      }
    }
  }

  const text = String(snapshot?.outputText || '').trim()
  if (!text) return ''
  return `\`\`\`text\n${text}\n\`\`\``
}

const buildExportPayloadFromSnapshot = async (cell, snapshot) => {
  if (snapshot?.dataframe) {
    const csvLines = [
      (snapshot.dataframe.columns || []).join(','),
      ...(snapshot.dataframe.rows || []).map((row) => (Array.isArray(row) ? row : []).map((value) => JSON.stringify(value ?? '')).join(',')),
    ].join('\n')
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'csv').fileName,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
      bytes: new TextEncoder().encode(csvLines),
      summary: 'dataframe',
    }
  }

  const display = snapshot?.display
  const displayType = normalizeDisplayType(display)
  if (displayType === 'markdown' && display?.markdown) {
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'md').fileName,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
      bytes: new TextEncoder().encode(String(display.markdown || '')),
      summary: 'markdown',
    }
  }
  if (displayType === 'html' && display?.html) {
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'html').fileName,
      filters: [{ name: 'HTML', extensions: ['html'] }],
      bytes: new TextEncoder().encode(String(display.html || '')),
      summary: 'html',
    }
  }
  if (displayType === 'svg' && display?.svg) {
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'svg').fileName,
      filters: [{ name: 'SVG', extensions: ['svg'] }],
      bytes: new TextEncoder().encode(String(display.svg || '')),
      summary: 'svg',
    }
  }
  if (['image', 'audio', 'video'].includes(displayType)) {
    const dataUrl = getDisplayDataUrl(display)
    if (dataUrl) {
      const matched = dataUrl.match(/^data:([^;]+);base64,(.+)$/i)
      if (matched) {
        const extension = guessExtensionFromMime(
          matched[1] || display?.mime,
          displayType === 'image' ? 'png' : displayType === 'audio' ? 'mp3' : 'mp4',
        )
        return {
          defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), extension).fileName,
          filters: [{ name: String(matched[1] || display?.mime || 'Binary'), extensions: [extension] }],
          bytes: base64ToUint8Array(matched[2] || ''),
          summary: displayType,
        }
      }
    }

    const sourcePath = getDisplaySourcePath(display)
    if (sourcePath) {
      return {
        defaultName: sourcePath.split(/[\\/]/).pop() || buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'bin').fileName,
        filters: undefined,
        bytes: null,
        summary: sourcePath,
      }
    }
  }

  const text = String(snapshot?.outputText || '').trim()
  if (!text) return null
  return {
    defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'txt').fileName,
    filters: [{ name: 'Text', extensions: ['txt'] }],
    bytes: new TextEncoder().encode(text),
    summary: 'text',
  }
}

const writeCellOutputToMarkdown = async (cell) => {
  flushPendingNotebookMarkdown('before-writeback')
  const snapshot = getCellSnapshot(cell)
  const markdownOutput = await buildMarkdownOutputFromSnapshot(cell, snapshot)
  if (!markdownOutput) {
    message.warning('当前输出暂时无法写回到 Markdown')
    return
  }

  const initialMarkdown = normalizeNotebookMarkdownText(lastEmittedMarkdown.value || props.markdown)
  const resolvedBlock = resolveSourcePythonBlock(cell, initialMarkdown)
  const normalized = ensureSourcePythonBlockHasStableId(initialMarkdown, cell, resolvedBlock)
  const currentMarkdown = normalized.markdown
  const targetBlock = normalized.block
  if (!targetBlock) {
    logger.error('[Notebook] failed to resolve source python block for markdown writeback', {
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      stableId: String(cell?.meta?.id || ''),
      matchStrategy: String(resolvedBlock?.matchStrategy || 'missing'),
    })
    message.error('没有找到对应的 Python Cell 源区块')
    return
  }

  const nextMarkdown = writeMarkdownOutputBlock(currentMarkdown, targetBlock, String(cell.cellId), markdownOutput)
  logger.info('[Notebook] write cell output back to markdown', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    stableId: String(cell?.meta?.id || ''),
    matchStrategy: String(resolvedBlock?.matchStrategy || 'unknown'),
    persistedStableId: !!normalized.persisted,
    outputKind: normalizeDisplayType(snapshot?.display) || (snapshot?.dataframe ? 'dataframe' : 'text'),
  })
  emit('update:markdown', nextMarkdown)
  lastEmittedMarkdown.value = normalizeNotebookMarkdownText(nextMarkdown)
  schedulePersistNotebookSidecar('write-cell-output-to-markdown')
  message.success('Python Cell 输出已写回到源 Markdown')
}

const exportCellOutputToFile = async (cell) => {
  const snapshot = getCellSnapshot(cell)
  const payload = await buildExportPayloadFromSnapshot(cell, snapshot)
  if (!payload) {
    message.warning('当前没有可导出的输出内容')
    return
  }

  if (!payload.bytes) {
    const outputPath = await save({
      defaultPath: payload.defaultName,
      filters: payload.filters,
    })
    if (!outputPath) return

    const sourceBytes = await readFile(String(payload.summary || ''))
    await writeBytesToPath(outputPath, sourceBytes, 'export-existing-file')
    logger.info('[Notebook] export existing output file to new path', {
      cellId: String(cell?.cellId || ''),
      sourcePath: payload.summary,
      outputPath,
    })
    message.success('当前输出文件已另存为新文件')
    return
  }

  const outputPath = await save({
    defaultPath: payload.defaultName,
    filters: payload.filters,
  })
  if (!outputPath) return

  await writeBytesToPath(outputPath, payload.bytes, 'export-payload')
  logger.info('[Notebook] export cell output to file', {
    cellId: String(cell?.cellId || ''),
    outputPath,
    summary: payload.summary,
  })
  message.success('当前输出已导出到文件')
}

const importTableToPythonSession = async (cell) => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Tabular Data', extensions: ['csv', 'tsv', 'txt', 'xlsx', 'xls', 'json', 'parquet'] }],
    })
    if (!selected) return

    logger.info('[Notebook] import table file selected', {
      cellId: String(cell?.cellId || ''),
      selected: String(selected),
    })

    const imported = await invoke('python_runtime_import_table_file', { request: { path: String(selected) } })
    if (!imported?.ok) {
      throw new Error(String(imported?.error || '导入失败'))
    }

    const fileName = String(selected).split(/[\\/]/).pop() || 'df'
    const variableName = sanitizeFileSegment(fileName.replace(/\.[^.]+$/, ''), 'df').replace(/-/g, '_')
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
    ].join('\n')

    const sessionId = createPythonSessionId(props.filePath)
    const result = await invoke('python_exec', {
      request: {
        code,
        persist: true,
        expect_json: true,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const importedTable = imported?.table && typeof imported.table === 'object'
      ? {
        columns: Array.isArray(imported.table.columns) ? imported.table.columns : [],
        rows: Array.isArray(imported.table.rows) ? imported.table.rows : [],
        rowCount: Number(imported.table.row_count || 0),
        truncated: !!imported.table.truncated,
      }
      : null

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      // 设计说明：
      // 导入外部表格属于“向共享 Python 会话注入变量”的准备动作，
      // 不应该把导入脚本本身伪装成当前 Cell 的一次运行结果。
      outputText: `已导入表格到会话变量 ${variableName}`,
      display: null,
      dataframe: importedTable,
      result,
      sessionVars: Array.isArray(result?.session_vars) ? result.session_vars : [],
      meta: {
        ...(getCellSnapshot(cell).meta || {}),
        outputCollapsed: !importedTable,
      },
    })
    logger.info('[Notebook] import table injected into python session', {
      cellId: String(cell?.cellId || ''),
      stableId: String(cell?.meta?.id || ''),
      variableName,
      rowCount: Number(importedTable?.rowCount || 0),
      hasPreview: !!importedTable,
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('import-table-to-python-session')
    message.success(`表格已导入到会话变量 ${variableName}`)
  } catch (error) {
    logger.error('[Notebook] import table to python session failed', {
      cellId: String(cell?.cellId || ''),
      error: String(error?.message || error),
    })
    message.error(`导入表格失败：${String(error?.message || error)}`)
  }
}

const runPythonCell = async (cell, { writeBack = false } = {}) => {
  flushPendingNotebookMarkdown('before-run-python-cell')
  if (!cell?.code) {
    message.warning('Python 代码为空')
    return
  }

  busyMap.value = {
    ...busyMap.value,
    [cell.key]: true,
  }

  try {
    const sessionId = createPythonSessionId(props.filePath)
    const res = await invoke('python_exec', {
      request: {
        code: String(cell.code || ''),
        persist: true,
        expect_json: true,
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const outputText = formatPythonExecOutput(res)
    const display = getGuaxiDisplayFromResult(res)
    const dataframe = getGuaxiDataFrameFromResult(res)
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || dataframe)
    const hideOutput = !hasRenderableOutput
    logger.info('[Notebook] python cell executed', {
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      writeBack,
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      hasDataframe: !!dataframe,
      hideOutput,
    })

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      outputText: hideOutput ? '' : outputText,
      display,
      dataframe,
      result: res,
      sessionVars: Array.isArray(res?.session_vars) ? res.session_vars : [],
      meta: {
        ...(getCellSnapshot(cell).meta || {}),
        outputCollapsed: hideOutput,
      },
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('run-python-cell-success')

    if (writeBack && !hideOutput) {
      await writeCellOutputToMarkdown(cell)
    }

    if (res?.ok === false) {
      message.error('Python 执行失败')
      return
    }

    message.success(writeBack ? '运行完成，输出已写回文档' : '运行完成')
  } catch (error) {
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      outputText: `运行失败：${String(error?.message || error)}`,
      display: null,
      dataframe: null,
      result: null,
      sessionVars: [],
      meta: {
        ...(getCellSnapshot(cell).meta || {}),
        outputCollapsed: false,
      },
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('run-python-cell-failed')
    message.error(`运行失败：${String(error?.message || error)}`)
  } finally {
    busyMap.value = {
      ...busyMap.value,
      [cell.key]: false,
    }
  }
}

watch(
  [() => props.markdown, () => props.filePath],
  ([nextMarkdown], oldPair = []) => {
    const prevFilePath = Array.isArray(oldPair) ? oldPair[1] : ''
    const filePathChanged = String(prevFilePath || '') !== String(props.filePath || '')
    if (filePathChanged) {
      notebookSearchIndexReady.value = false
      clearNotebookSearchHighlights()
    }
    syncNotebookCellsFromMarkdown(nextMarkdown, 'props-markdown-or-file-watch', {
      force: filePathChanged,
    })
  },
  { immediate: true },
)

watch(
  notebookCells,
  (cells) => {
    const keys = new Set(cells.map((cell) => String(cell.key || '')))
    if (editingMarkdownCellKey.value && !keys.has(String(editingMarkdownCellKey.value))) {
      editingMarkdownCellKey.value = ''
    }
    syncRenderedMarkdownHtml(cells, 'notebook-cells-watch')
    syncResolvedDisplayUrls(cells, 'notebook-cells-watch')
    scheduleNotebookSearchIndexUpdate('notebook-cells-watch')
    nextTick(() => {
      syncNotebookTextareaHeights('notebook-cells-changed')
      if (notebookSearchVisible.value) {
        scheduleNotebookSearchQuery('notebook-cells-watch-visible', 0)
      }
    })
  },
  { deep: true, immediate: true },
)

watch(
  [() => props.theme, () => props.filePath, refreshTick],
  () => {
    syncRenderedMarkdownHtml(notebookCells.value, 'theme-file-refresh-watch')
    syncResolvedDisplayUrls(notebookCells.value, 'theme-file-refresh-watch')
    scheduleNotebookSearchIndexUpdate('theme-file-refresh-watch')
    if (notebookSearchVisible.value) {
      scheduleNotebookSearchQuery('theme-file-refresh-watch', 0)
    }
  },
)

watch(editingMarkdownCellKey, (nextKey) => {
  if (!nextKey) return
  focusMarkdownEditor(nextKey, 'editing-markdown-cell-key')
})

watch(
  notebookOutlineEntries,
  (entries) => {
    const payload = createNotebookOutlinePayload()
    emit('outline-change', payload)
    logger.info('[Notebook] emit outline change', {
      count: Array.isArray(entries) ? entries.length : 0,
    })
  },
  { immediate: true, deep: true },
)

watch(notebookSearchKeyword, () => {
  if (!String(notebookSearchKeyword.value || '').trim()) {
    clearNotebookSearchHighlights()
    return
  }
  scheduleNotebookSearchQuery('keyword-change')
})

watch(
  [renderedMarkdownHtmlMap, renderedOutputMarkdownHtmlMap],
  () => {
    if (!notebookSearchVisible.value || !String(notebookSearchKeyword.value || '').trim()) return
    nextTick(() => {
      applyNotebookSearchDomHighlights('rendered-html-watch')
    })
  },
  { deep: false },
)

onMounted(() => {
  initNotebookMarkdownWorker()
  document.addEventListener('pointerdown', handleDocumentPointerDownForMarkdownEdit, true)
})

onBeforeUnmount(() => {
  if (notebookSearchIndexSyncTimer) {
    window.clearTimeout(notebookSearchIndexSyncTimer)
    notebookSearchIndexSyncTimer = 0
  }
  if (notebookSearchQueryTimer) {
    window.clearTimeout(notebookSearchQueryTimer)
    notebookSearchQueryTimer = 0
  }
  document.removeEventListener('pointerdown', handleDocumentPointerDownForMarkdownEdit, true)
  disposeNotebookMarkdownWorker('component-unmount')
})

defineExpose({
  insertCell(type = 'python') {
    appendCell(type)
  },
  undo() {
    undoNotebookHistory()
  },
  redo() {
    redoNotebookHistory()
  },
  save() {
    requestNotebookSave()
  },
  canUndo() {
    return canUndoNotebookHistory.value
  },
  canRedo() {
    return canRedoNotebookHistory.value
  },
  openSearch() {
    return openNotebookSearch()
  },
  closeSearch() {
    closeNotebookSearch()
  },
  isSearchOpen() {
    return notebookSearchVisible.value
  },
  getOutline() {
    const outline = createNotebookOutlinePayload()
    logger.info('[Notebook] expose outline snapshot', {
      count: outline.length,
    })
    return outline
  },
  jumpToOutlineLine(line) {
    return jumpToNotebookOutlineLine(line)
  },
})
</script>

<style scoped>
.guaxi-notebook {
  --guaxi-notebook-bg: var(--guaxi-content-bg, #f7f9fc);
  --guaxi-notebook-panel: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 94%, var(--guaxi-content-bg, #f7f9fc) 6%);
  --guaxi-notebook-panel-muted: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 86%, var(--guaxi-content-bg, #f7f9fc) 14%);
  --guaxi-notebook-panel-soft: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 78%, var(--guaxi-content-bg, #f7f9fc) 22%);
  --guaxi-notebook-border: color-mix(in srgb, var(--guaxi-border, rgba(15, 23, 42, 0.1)) 86%, transparent);
  --guaxi-notebook-strong-border: color-mix(in srgb, var(--guaxi-border, rgba(15, 23, 42, 0.1)) 100%, rgba(15, 23, 42, 0.04));
  --guaxi-notebook-text: var(--guaxi-text-primary, #172033);
  --guaxi-notebook-muted: var(--guaxi-text-secondary, rgba(23, 32, 51, 0.72));
  --guaxi-notebook-btn: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 82%, var(--guaxi-content-bg, #f7f9fc) 18%);
  --guaxi-notebook-btn-hover: color-mix(in srgb, var(--guaxi-panel-bg, #ffffff) 70%, var(--guaxi-content-bg, #f7f9fc) 30%);
  --guaxi-notebook-primary: #3b82f6;
  --guaxi-notebook-primary-hover: #2563eb;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 14%, var(--guaxi-panel-bg, #ffffff) 86%);
  --guaxi-notebook-focus-ring: rgba(59, 130, 246, 0.2);
  --guaxi-notebook-shadow: 0 10px 28px rgba(15, 23, 42, 0.06);
  /* 设计说明：
   * 1. 探索模式整体字体统一追随主编辑器，避免 Markdown/Python/变量面板各自使用不同字族。
   * 2. `--editor-font-family` 定义在父级编辑器容器上，这里直接继承；若外层变量缺失，再回退到既有等宽字体。
   */
  --guaxi-notebook-font: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  --guaxi-notebook-mono: var(--editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 100%;
  padding: 18px;
  background: var(--guaxi-notebook-bg, #f6f8fc);
  color: var(--guaxi-notebook-text, #172033);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook.theme-dark {
  --guaxi-notebook-panel: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 97%, #0b1220 3%);
  --guaxi-notebook-panel-muted: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 86%, #334155 14%);
  --guaxi-notebook-panel-soft: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 74%, #334155 26%);
  --guaxi-notebook-border: rgba(148, 163, 184, 0.28);
  --guaxi-notebook-strong-border: rgba(148, 163, 184, 0.38);
  --guaxi-notebook-text: #edf3fb;
  --guaxi-notebook-muted: rgba(221, 230, 241, 0.82);
  --guaxi-notebook-btn: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 68%, #475569 32%);
  --guaxi-notebook-btn-hover: color-mix(in srgb, var(--guaxi-panel-bg, #21252b) 56%, #64748b 44%);
  --guaxi-notebook-primary: #7cc2ff;
  --guaxi-notebook-primary-hover: #4f9dff;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 28%, #334155 72%);
  --guaxi-notebook-focus-ring: rgba(96, 165, 250, 0.24);
  --guaxi-notebook-shadow: 0 14px 34px rgba(2, 6, 23, 0.24);
}

.guaxi-notebook.theme-light {
  --guaxi-notebook-primary: #2563eb;
  --guaxi-notebook-primary-hover: #1d4ed8;
  --guaxi-notebook-danger: color-mix(in srgb, #ef4444 14%, var(--guaxi-panel-bg, #ffffff) 86%);
  --guaxi-notebook-focus-ring: rgba(37, 99, 235, 0.18);
}

.guaxi-notebook__toolbar,
.guaxi-notebook__cell,
.guaxi-notebook__panel {
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 14px;
  background: var(--guaxi-notebook-panel);
  box-shadow: 0 4px 14px rgba(15, 23, 42, 0.04);
}

.guaxi-notebook__toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 16px;
  flex-wrap: wrap;
  padding: 10px 12px;
}

.guaxi-notebook__search-panel-shell {
  position: sticky;
  top: 12px;
  z-index: 24;
  height: 0;
  display: flex;
  justify-content: flex-end;
  pointer-events: none;
}

.guaxi-notebook__search-panel {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  width: fit-content;
  margin-top: 0;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 86%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 96%, transparent);
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.14);
  backdrop-filter: blur(10px);
  pointer-events: auto;
}

.guaxi-notebook__search-input {
  width: 220px;
  height: 26px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  outline: none;
  background: var(--guaxi-notebook-panel-soft);
  color: var(--guaxi-notebook-text);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__search-input:focus {
  border-color: var(--guaxi-notebook-primary);
}

.guaxi-notebook__search-count {
  min-width: 52px;
  text-align: center;
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook__search-btn,
.guaxi-notebook__panel-btn {
  height: 26px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
  color: var(--guaxi-notebook-text);
  cursor: pointer;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__search-btn:hover,
.guaxi-notebook__panel-btn:hover {
  background: var(--guaxi-notebook-btn-hover);
}

.guaxi-notebook__search-btn--danger {
  color: #ef4444;
}

.guaxi-notebook__toolbar-main {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.guaxi-notebook__toolbar-main--solo {
  width: 100%;
  justify-content: flex-end;
}

.guaxi-notebook__cell {
  overflow: hidden;
  transition: box-shadow 0.18s ease, outline-color 0.18s ease, background-color 0.18s ease;
}

.guaxi-notebook__cell.is-search-match {
  outline: 1px solid color-mix(in srgb, var(--guaxi-notebook-primary) 24%, transparent);
  outline-offset: 2px;
  border-radius: 10px;
}

.guaxi-notebook__cell.is-search-active {
  outline: 2px solid color-mix(in srgb, var(--guaxi-notebook-primary) 62%, transparent);
  outline-offset: 2px;
  border-radius: 10px;
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--guaxi-notebook-primary) 16%, transparent);
}

.guaxi-notebook__cell.is-markdown {
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  overflow: visible;
}

.guaxi-notebook__insert-strip {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 12px;
  margin: -2px 0;
}

.guaxi-notebook__insert-line {
  width: 100%;
  height: 1px;
  background: color-mix(in srgb, var(--guaxi-notebook-border) 82%, transparent);
  opacity: 0.34;
  transition: opacity 0.2s ease;
}

.guaxi-notebook__insert-trigger {
  position: absolute;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 72%, transparent);
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 94%, var(--guaxi-notebook-bg) 6%);
  color: var(--guaxi-notebook-muted);
  opacity: 0.48;
  transform: scale(0.96);
  box-shadow: 0 2px 8px rgba(15, 23, 42, 0.05);
  transition: opacity 0.2s ease, transform 0.2s ease, border-color 0.2s ease;
  pointer-events: none;
}

.guaxi-notebook__insert-actions {
  position: absolute;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 74%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 96%, var(--guaxi-notebook-bg) 4%);
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.08);
  opacity: 0;
  transform: translateY(2px) scale(0.96);
  transition: opacity 0.2s ease, transform 0.2s ease;
  pointer-events: none;
}

.guaxi-notebook__insert-strip:hover .guaxi-notebook__insert-line,
.guaxi-notebook__insert-strip:focus-within .guaxi-notebook__insert-line {
  opacity: 0.7;
}

.guaxi-notebook__insert-strip:hover .guaxi-notebook__insert-trigger,
.guaxi-notebook__insert-strip:focus-within .guaxi-notebook__insert-trigger {
  opacity: 0;
  transform: scale(0.82);
}

.guaxi-notebook__insert-strip:hover .guaxi-notebook__insert-actions,
.guaxi-notebook__insert-strip:focus-within .guaxi-notebook__insert-actions {
  opacity: 1;
  transform: translateY(0) scale(1);
  pointer-events: auto;
}

.guaxi-notebook__cell-meta,
.guaxi-notebook__empty,
.guaxi-notebook__var-preview {
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__python-layout {
  display: grid;
  gap: 6px;
  padding: 6px 10px 8px;
}

.guaxi-notebook__markdown-surface {
  position: relative;
  display: grid;
  gap: 6px;
  padding: 0;
  overflow: visible;
}

.guaxi-notebook__markdown-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  width: 100%;
  box-sizing: border-box;
  padding: 0 6px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 70%, transparent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 94%, transparent);
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.08);
  opacity: 0;
  transform: translateY(-2px);
  pointer-events: none;
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.guaxi-notebook__markdown-surface:hover .guaxi-notebook__markdown-actions,
.guaxi-notebook__markdown-surface:focus-within .guaxi-notebook__markdown-actions,
.guaxi-notebook__markdown-surface.is-editing .guaxi-notebook__markdown-actions {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.guaxi-notebook__markdown-edit-shell {
  display: grid;
  gap: 8px;
}

.guaxi-notebook__python-editor {
  width: 100%;
  min-height: 88px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 14px;
  outline: none;
  resize: none;
  padding: 16px 18px;
  color: var(--guaxi-notebook-text);
  background: var(--guaxi-notebook-panel-soft);
  font-size: 14px;
  line-height: 1.7;
  box-sizing: border-box;
  overflow-y: hidden;
}

.guaxi-notebook__python-editor,
.guaxi-notebook__output-text {
  font-family: var(--guaxi-notebook-mono);
}

.guaxi-notebook__python-editor:focus,
.guaxi-notebook__markdown-preview-shell.is-clickable:focus-visible {
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 62%, var(--guaxi-notebook-border) 38%);
  box-shadow: 0 0 0 3px var(--guaxi-notebook-focus-ring);
}

.guaxi-notebook__markdown-preview-shell {
  border: none;
  border-radius: 0;
  background: transparent;
  padding: 6px;
  font-family: var(--guaxi-notebook-font);
  font-size: 17px;
  line-height: 1.5;
}

.guaxi-notebook__markdown-preview-shell.is-clickable {
  cursor: text;
}

.guaxi-notebook__markdown-preview {
  min-height: 0;
  padding: 0;
  overflow: visible;
  font-family: var(--guaxi-notebook-font);
  font-size: 17px;
  line-height: 1.5;
}

.guaxi-notebook__cell-btn,
.guaxi-notebook__toolbar-btn,
.guaxi-notebook__icon-btn {
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 10px;
  background: var(--guaxi-notebook-btn);
  color: var(--guaxi-notebook-text);
  padding: 7px 11px;
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.18s ease;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__cell-btn:hover,
.guaxi-notebook__toolbar-btn:hover,
.guaxi-notebook__icon-btn:hover {
  background: var(--guaxi-notebook-btn-hover);
  transform: translateY(-1px);
}

.guaxi-notebook__cell-btn.primary,
.guaxi-notebook__toolbar-btn.primary,
.guaxi-notebook__icon-btn.primary {
  color: #ffffff;
  border-color: transparent;
  background: linear-gradient(135deg, var(--guaxi-notebook-primary), var(--guaxi-notebook-primary-hover));
}

.guaxi-notebook__cell-btn.ghost,
.guaxi-notebook__toolbar-btn.ghost,
.guaxi-notebook__icon-btn.ghost {
  background: transparent;
}

.guaxi-notebook__cell-btn.danger,
.guaxi-notebook__icon-btn.danger {
  background: var(--guaxi-notebook-danger);
}

.guaxi-notebook__icon-btn {
  width: 30px;
  height: 30px;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 9px;
}

.guaxi-notebook__icon-btn:disabled {
  opacity: 0.48;
  cursor: not-allowed;
  transform: none;
}

.guaxi-notebook__python-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
  min-height: 34px;
  padding: 4px 2px 2px;
}

.guaxi-notebook__python-toolbar-copy {
  min-width: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.guaxi-notebook__python-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 52px;
  height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 82%, var(--guaxi-notebook-panel) 18%);
  border: 1px solid var(--guaxi-notebook-strong-border);
  color: var(--guaxi-notebook-muted);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-notebook__python-title {
  min-width: 0;
  color: var(--guaxi-notebook-muted);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.guaxi-notebook__python-toolbar-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.guaxi-notebook__python-toolbar .guaxi-notebook__icon-btn,
.guaxi-notebook__markdown-actions .guaxi-notebook__icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 8px;
}

.guaxi-notebook__python-toolbar .guaxi-notebook__icon-btn:not(.primary) {
  background: color-mix(in srgb, var(--guaxi-notebook-btn) 82%, transparent);
}

.guaxi-notebook__panel {
  margin: 0;
  padding: 12px 14px;
  border-radius: 12px;
}

.guaxi-notebook__insert-actions .guaxi-notebook__icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 999px;
}

.guaxi-notebook__panel-content {
  display: grid;
  gap: 10px;
}

.guaxi-notebook__panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}

.guaxi-notebook__panel-title {
  font-weight: 700;
  font-family: var(--guaxi-notebook-font);
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.guaxi-notebook__panel-actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.guaxi-notebook__output-text,
.guaxi-notebook__rich,
.guaxi-notebook__dataframe {
  margin: 0;
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
  padding: 12px 14px;
  overflow-x: auto;
  overflow-y: visible;
}

.guaxi-notebook__rich {
  font-size: 17px;
  line-height: 1.5;
}

.guaxi-notebook__output-text {
  white-space: pre-wrap;
}

.guaxi-notebook__dataframe table {
  width: 100%;
  border-collapse: collapse;
}

.guaxi-notebook__dataframe th,
.guaxi-notebook__dataframe td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--guaxi-notebook-border);
  text-align: left;
}

.guaxi-notebook__dataframe th {
  color: var(--guaxi-notebook-text);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 72%, var(--guaxi-notebook-panel) 28%);
}

.guaxi-notebook__image {
  display: block;
  max-width: 100%;
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
}

.guaxi-notebook__media {
  display: block;
  width: 100%;
  max-width: 100%;
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: var(--guaxi-notebook-panel-soft);
}

.guaxi-notebook__media--video {
  max-height: 520px;
}

.guaxi-notebook__empty--dashed {
  min-height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--guaxi-notebook-border);
  border-radius: 12px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 90%, var(--guaxi-notebook-panel-soft) 10%);
}

.guaxi-notebook__vars {
  display: grid;
  gap: 10px;
}

.guaxi-notebook__var {
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--guaxi-notebook-panel-soft);
  border: 1px solid var(--guaxi-notebook-strong-border);
}

.guaxi-notebook__var-main {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.guaxi-notebook__var-name {
  font-weight: 700;
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__var-type {
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook.theme-dark .guaxi-notebook__cell-btn,
.guaxi-notebook.theme-dark .guaxi-notebook__toolbar-btn,
.guaxi-notebook.theme-dark .guaxi-notebook__icon-btn {
  color: #edf3fb;
}

.guaxi-notebook.theme-dark .guaxi-notebook__cell-btn.primary,
.guaxi-notebook.theme-dark .guaxi-notebook__toolbar-btn.primary,
.guaxi-notebook.theme-dark .guaxi-notebook__icon-btn.primary {
  color: #08111f;
  font-weight: 700;
}

.guaxi-notebook.theme-dark .guaxi-notebook__rich,
.guaxi-notebook.theme-dark .guaxi-notebook__output-text,
.guaxi-notebook.theme-dark .guaxi-notebook__dataframe,
.guaxi-notebook.theme-dark .guaxi-notebook__var,
.guaxi-notebook.theme-dark .guaxi-notebook__python-editor {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 88%, #0f172a 12%);
}

.guaxi-notebook__markdown-preview :deep(.md-paragraph),
.guaxi-notebook__rich :deep(.md-paragraph) {
  color: var(--guaxi-notebook-text);
}

.guaxi-notebook__markdown-preview :deep(*),
.guaxi-notebook__rich.markdown-body :deep(*) {
  font-family: var(--guaxi-notebook-font);
}

.guaxi-notebook__markdown-preview :deep(a),
.guaxi-notebook__markdown-preview :deep(a:visited),
.guaxi-notebook__rich.markdown-body :deep(a),
.guaxi-notebook__rich.markdown-body :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(a),
.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(a:visited),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(a),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

:deep(.guaxi-notebook__search-hit) {
  background: rgba(255, 208, 0, 0.38);
  border-radius: 2px;
  padding: 0 1px;
}

:deep(.guaxi-notebook__search-hit--active) {
  background: rgba(255, 145, 0, 0.65);
}

.guaxi-notebook__markdown-preview :deep(code),
.guaxi-notebook__markdown-preview :deep(pre),
.guaxi-notebook__rich :deep(code),
.guaxi-notebook__rich :deep(pre) {
  font-family: var(--guaxi-notebook-mono);
}

.guaxi-notebook__markdown-preview :deep(pre),
.guaxi-notebook__rich :deep(pre) {
  max-width: 100%;
}

.guaxi-notebook__markdown-preview :deep(.hljs),
.guaxi-notebook__rich :deep(.hljs) {
  border-radius: 10px;
  width: 100%;
  box-sizing: border-box;
}

.guaxi-notebook__markdown-preview :deep(mjx-container),
.guaxi-notebook__rich :deep(mjx-container) {
  overflow-x: auto;
  overflow-y: hidden;
}

.guaxi-notebook__markdown-preview :deep(.copy-button),
.guaxi-notebook__rich :deep(.copy-button) {
  opacity: 0.92;
}

@media (max-width: 1100px) {
  .guaxi-notebook__toolbar,
  .guaxi-notebook__python-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .guaxi-notebook__search-panel-shell {
    position: static;
    top: auto;
    height: auto;
    display: block;
    pointer-events: auto;
  }

  .guaxi-notebook__search-panel {
    position: static;
    width: 100%;
    margin-top: -2px;
    box-sizing: border-box;
  }

  .guaxi-notebook__insert-actions {
    padding: 0 6px;
  }
}
</style>
