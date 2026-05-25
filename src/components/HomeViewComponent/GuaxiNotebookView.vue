<template>
  <div
    ref="notebookRootRef"
    class="guaxi-notebook"
    :class="[
      themeClass,
      {
        'is-top-toolbar-visible': isNotebookTopToolbarVisible,
        'has-active-python-cell': !!activeNotebookPythonCell,
      },
    ]"
    @mousemove="handleNotebookRootMouseMove"
    @mouseleave="handleNotebookRootMouseLeave"
    @click="handleNotebookRenderedClick"
    @mouseup="handleNotebookSelectionMouseUp"
  >
    <div
      class="guaxi-notebook__toolbar-shell"
      :class="{ 'is-visible': isNotebookTopToolbarVisible }"
      @mouseenter="revealNotebookTopToolbar('toolbar-shell-enter')"
      @mouseleave="scheduleHideNotebookTopToolbar('toolbar-shell-leave')"
      @focusin="revealNotebookTopToolbar('toolbar-focusin')"
      @focusout="handleNotebookToolbarFocusOut"
    >
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
            class="guaxi-notebook__icon-btn"
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
          <template v-if="activeNotebookPythonCell">
            <span class="guaxi-notebook__toolbar-divider" aria-hidden="true"></span>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :disabled="isActiveNotebookPythonSessionBusy"
              :title="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，请稍候再导入数据' : '从外部文件导入数据到当前选中的 Python Cell 实验室会话'"
              :aria-label="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，请稍候再导入数据' : '从外部文件导入数据到当前选中的 Python Cell 实验室会话'"
              @click="handleActivePythonToolbarAction('import-table')"
            >
              <Icon :icon="uploadIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn primary"
              :class="{ 'is-busy': isActiveNotebookPythonSessionBusy }"
              :title="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行当前选中的 Python Cell' : '运行当前选中的 Python Cell'"
              :aria-label="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行当前选中的 Python Cell' : '运行当前选中的 Python Cell'"
              @click="handleActivePythonToolbarAction('run')"
            >
              <Icon :icon="playIcon" width="16" height="16" :class="{ 'guaxi-notebook__busy-spin': isActiveNotebookPythonSessionBusy }" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :title="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行并写回当前选中的 Python Cell' : '运行并将当前选中的 Python Cell 输出写回文档'"
              :aria-label="isActiveNotebookPythonSessionBusy ? '当前文档的 Python 会话正在运行，点击后会在空闲时自动运行并写回当前选中的 Python Cell' : '运行并将当前选中的 Python Cell 输出写回文档'"
              @click="handleActivePythonToolbarAction('run-writeback')"
            >
              <Icon :icon="checkCircleIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :disabled="!hasVisibleOutput(activeNotebookPythonCell)"
              :title="activeNotebookPythonCellSnapshot.meta.outputCollapsed ? '显示当前选中 Python Cell 输出' : '隐藏当前选中 Python Cell 输出'"
              :aria-label="activeNotebookPythonCellSnapshot.meta.outputCollapsed ? '显示当前选中 Python Cell 输出' : '隐藏当前选中 Python Cell 输出'"
              @click="handleActivePythonToolbarAction('toggle-output')"
            >
              <Icon :icon="activeNotebookPythonCellSnapshot.meta.outputCollapsed ? eyeIcon : eyeOffIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              :title="isNotebookPythonSessionPanelVisible ? '隐藏当前选中 Python Cell 数据实验室' : '显示当前选中 Python Cell 数据实验室'"
              :aria-label="isNotebookPythonSessionPanelVisible ? '隐藏当前选中 Python Cell 数据实验室' : '显示当前选中 Python Cell 数据实验室'"
              @click="handleActivePythonToolbarAction('toggle-session')"
            >
              <Icon :icon="flaskConicalIcon" width="16" height="16" />
            </button>
            <button
              type="button"
              class="guaxi-notebook__icon-btn"
              title="打开 Python 控制台"
              aria-label="打开 Python 控制台"
              @click="handleActivePythonToolbarAction('open-console')"
            >
              <Icon :icon="terminalIcon" width="16" height="16" />
            </button>
          </template>
        </div>
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

    <div
      v-show="notebookSelectionToolbarVisible"
      class="guaxi-notebook__selection-toolbar"
      :style="notebookSelectionToolbarStyle"
      @mousedown.prevent
      @click.stop
    >
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        :disabled="selectionTranslateBusy"
        title="翻译选中文本"
        aria-label="翻译选中文本"
        @click="openNotebookSelectionTranslatePanel"
      >
        <Icon :icon="languagesIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        title="发送到助手"
        aria-label="发送到助手"
        @click="askNotebookSelectionWithAssistant"
      >
        <Icon :icon="messageCircleIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        title="用 AI 翻译"
        aria-label="用 AI 翻译"
        @click="sendNotebookSelectionAsAiTranslate"
      >
        <Icon :icon="botIcon" width="16" height="16" />
      </button>
      <button
        type="button"
        class="guaxi-notebook__selection-tool-btn"
        :disabled="selectionReadAloudBusy"
        :title="selectionReadAloudBusy ? '正在朗读' : '朗读选中文本'"
        :aria-label="selectionReadAloudBusy ? '正在朗读' : '朗读选中文本'"
        @click="readNotebookSelectionAloud"
      >
        <Icon :icon="volume2Icon" width="16" height="16" />
      </button>
    </div>

    <template v-for="(cell, index) in notebookCells" :key="cell.key">
      <div
        class="guaxi-notebook__cell"
        :class="[
          `is-${cell.type}`,
          {
            'is-search-match': isNotebookSearchMatchedCell(cell),
            'is-search-active': isNotebookSearchActiveCell(cell),
            'is-active-python-cell': isActiveNotebookPythonCell(cell),
          },
        ]"
        :data-cell-key="cell.key"
        @mousedown="handleNotebookCellPointerDown(cell, $event)"
      >
        <button
          v-if="cell.type === 'markdown'"
          type="button"
          class="guaxi-notebook__cell-delete"
          title="删除当前 Cell"
          aria-label="删除当前 Cell"
          @click.stop="confirmRemoveCell(cell)"
        >
          <Icon :icon="trash2Icon" width="16" height="16" />
        </button>
      <template v-if="cell.type === 'markdown'">
        <div
          class="guaxi-notebook__markdown-surface"
          :class="{ 'is-editing': isMarkdownCellEditing(cell) }"
        >
          <div v-if="isMarkdownCellEditing(cell)" class="guaxi-notebook__markdown-edit-shell">
            <GuaxiNotebookMarkdownEditor
              :ref="(instance) => setMarkdownEditorRef(cell.key, instance)"
              :cell-key="cell.key"
              :document-path="filePath"
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
            @dblclick="enterMarkdownEditMode(cell.key, 'preview-dblclick')"
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
              <span class="guaxi-notebook__python-badge" :class="{ 'is-busy': isCellBusy(cell) }">
                <Icon
                  :icon="pythonIcon"
                  width="14"
                  height="14"
                  class="guaxi-notebook__python-badge-icon"
                  :class="{ 'guaxi-notebook__busy-spin': isCellBusy(cell) }"
                />
                <span>{{ isCellBusy(cell) ? '运行中' : 'Python' }}</span>
              </span>
              <span v-if="String(cell.meta?.title || '').trim()" class="guaxi-notebook__python-title">{{ cell.meta?.title }}</span>
            </div>
            <div class="guaxi-notebook__python-toolbar-actions">
              <button
                type="button"
                class="guaxi-notebook__icon-btn danger"
                title="删除当前 Python Cell"
                aria-label="删除当前 Python Cell"
                @click="confirmRemoveCell(cell)"
              >
                <Icon :icon="trash2Icon" width="16" height="16" />
              </button>
            </div>
          </div>

          <div v-if="getCellSnapshot(cell).meta.staleVariables?.length" class="guaxi-notebook__stale-banner">
            上游变量已更新：{{ getCellSnapshot(cell).meta.staleVariables.join(', ') }}。建议重新运行当前 Cell。
          </div>

          <GuaxiNotebookPythonEditor
            :cell-key="cell.key"
            :document-path="filePath"
            :model-value="cell.code"
            :theme="theme"
            @update:model-value="updatePythonCell(cell.key, $event)"
            @focus="handleNotebookPythonEditorFocus(cell)"
          />

          <div v-if="(hasVisibleOutput(cell) || isCellBusy(cell)) && !getCellSnapshot(cell).meta.outputCollapsed" class="guaxi-notebook__panel">
            <div class="guaxi-notebook__panel-head">
              <div class="guaxi-notebook__panel-title">输出 · {{ getCellOutputModules(cell).length }}</div>
              <div class="guaxi-notebook__panel-actions">
                <button
                  v-if="getLatestTextOutputModule(cell)"
                  type="button"
                  class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                  title="复制文本输出"
                  aria-label="复制文本输出"
                  @click="copyPythonCellTextOutput(cell)"
                >
                  <Icon :icon="copyIcon" width="14" height="14" />
                </button>
                <button type="button" class="guaxi-notebook__panel-btn" @click="writeCellOutputToMarkdown(cell)">写回 Markdown</button>
                <button type="button" class="guaxi-notebook__panel-btn" @click="exportCellOutputToFile(cell)">导出文件</button>
              </div>
            </div>
            <div class="guaxi-notebook__panel-content">
              <div v-if="isCellBusy(cell)" class="guaxi-notebook__running-banner">Python 正在运行，请稍候...</div>
              <div
                v-for="outputModule in getCellOutputModules(cell)"
                :key="`${cell.key}-${outputModule.id}`"
                class="guaxi-notebook__output-module"
                :class="{ 'is-collapsed': outputModule.collapsed }"
              >
                <div class="guaxi-notebook__output-module-head">
                  <div class="guaxi-notebook__output-module-copy">
                    <span class="guaxi-notebook__output-module-title">{{ getNotebookOutputModuleTitle(outputModule) }}</span>
                    <span v-if="getNotebookOutputModuleSummary(outputModule)" class="guaxi-notebook__output-module-summary">
                      {{ getNotebookOutputModuleSummary(outputModule) }}
                    </span>
                  </div>
                  <div class="guaxi-notebook__output-module-actions">
                    <button
                      v-if="outputModule.kind === 'text'"
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                      title="复制当前模块文本"
                      aria-label="复制当前模块文本"
                      @click="copyPythonCellTextOutput(cell, outputModule.id)"
                    >
                      <Icon :icon="copyIcon" width="14" height="14" />
                    </button>
                    <button type="button" class="guaxi-notebook__panel-btn" @click="writeCellOutputToMarkdown(cell, outputModule.id)">写回 Markdown</button>
                    <button type="button" class="guaxi-notebook__panel-btn" @click="exportCellOutputToFile(cell, outputModule.id)">导出文件</button>
                    <button
                      type="button"
                      class="guaxi-notebook__icon-btn guaxi-notebook__panel-icon-btn"
                      :title="outputModule.collapsed ? '显示当前输出模块' : '隐藏当前输出模块'"
                      :aria-label="outputModule.collapsed ? '显示当前输出模块' : '隐藏当前输出模块'"
                      @click="toggleCellOutputModule(cell, outputModule.id)"
                    >
                      <Icon :icon="outputModule.collapsed ? eyeIcon : eyeOffIcon" width="14" height="14" />
                    </button>
                  </div>
                </div>
                <div v-if="!outputModule.collapsed" class="guaxi-notebook__output-module-body">
                  <div v-if="outputModule.kind === 'html'" class="guaxi-notebook__rich guaxi-notebook__rich--html">
                    <GuaxiNotebookHtmlOutput
                      :html="String(outputModule.display?.html || '')"
                      :theme="theme"
                      :title="`Python Cell HTML Output ${cell.cellId || cell.key || ''}`"
                    />
                  </div>
                  <div v-else-if="outputModule.kind === 'svg'" class="guaxi-notebook__rich" v-html="String(outputModule.display?.svg || '')"></div>
                  <div
                    v-else-if="outputModule.kind === 'markdown'"
                    class="guaxi-notebook__rich markdown-body"
                    v-html="getRenderedOutputMarkdownHtml(cell, outputModule)"
                  ></div>
                  <img
                    v-else-if="outputModule.kind === 'image' && getCellDisplayUrl(cell, outputModule)"
                    class="guaxi-notebook__image"
                    :src="getCellDisplayUrl(cell, outputModule)"
                    alt="Python Cell Output"
                  />
                  <audio
                    v-else-if="outputModule.kind === 'audio' && getCellDisplayUrl(cell, outputModule)"
                    class="guaxi-notebook__media"
                    :src="getCellDisplayUrl(cell, outputModule)"
                    controls
                    preload="metadata"
                  ></audio>
                  <video
                    v-else-if="outputModule.kind === 'video' && getCellDisplayUrl(cell, outputModule)"
                    class="guaxi-notebook__media guaxi-notebook__media--video"
                    :src="getCellDisplayUrl(cell, outputModule)"
                    controls
                    preload="metadata"
                  ></video>
                  <div v-else-if="outputModule.kind === 'table' && outputModule.table" class="guaxi-notebook__dataframe">
                    <GuaxiNotebookTableOutput :table="outputModule.table" />
                  </div>
                  <pre v-else-if="outputModule.kind === 'text'" class="guaxi-notebook__output-text">{{ outputModule.text }}</pre>
                </div>
              </div>
              <div v-if="!getCellOutputModules(cell).length && !isCellBusy(cell)" class="guaxi-notebook__empty guaxi-notebook__empty--dashed">
                当前还没有输出结果
              </div>
            </div>
          </div>
        </div>
      </template>
      </div>

      <div v-if="index < notebookCells.length - 1" class="guaxi-notebook__insert-strip">
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

    <GuaxiSelectionTranslatePanel
      v-if="selectionTranslatePanelVisible"
      :pos="selectionTranslatePanelPos"
      :raw="selectionTranslateRaw"
      :result="selectionTranslateResult"
      :error="selectionTranslateError"
      :busy="selectionTranslateBusy"
      :pinned="selectionTranslatePinned"
      :items="selectionTranslateItems"
      :word-meta="selectionTranslateWordMeta"
      :from-lang="selectionTranslateFrom"
      :to-lang="selectionTranslateTo"
      :lang-options="selectionTranslateLangOptions"
      @update:from-lang="(value) => (selectionTranslateFrom = String(value || 'auto'))"
      @update:to-lang="(value) => (selectionTranslateTo = String(value || 'zh'))"
      @swap-langs="swapNotebookSelectionTranslateLangs"
      @translate="translateNotebookSelectionText"
      @copy="copyNotebookSelectionTranslateResult"
      @toggle-pin="toggleNotebookSelectionTranslatePinned"
      @update:pos="handleNotebookSelectionTranslatePanelUpdatePos"
      @close="closeNotebookSelectionTranslatePanel"
    />
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { listen } from '@tauri-apps/api/event'
import { useMessage } from 'naive-ui'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { confirm as confirmDialog, open, save } from '@tauri-apps/plugin-dialog'
import { readFile, writeFile } from '@tauri-apps/plugin-fs'
import mermaid from 'mermaid'
import MarkdownIt from 'markdown-it'
import { debounce } from 'lodash-es'
import filePlusIcon from '@iconify-icons/lucide/file-plus'
import terminalSquareIcon from '@iconify-icons/lucide/terminal-square'
import playIcon from '@iconify-icons/lucide/play'
import checkCircleIcon from '@iconify-icons/lucide/check-circle-2'
import plusIcon from '@iconify-icons/lucide/plus'
import searchIcon from '@iconify-icons/lucide/search'
import saveIcon from '@iconify-icons/lucide/save'
import uploadIcon from '@iconify-icons/lucide/upload'
import eyeIcon from '@iconify-icons/lucide/eye'
import eyeOffIcon from '@iconify-icons/lucide/eye-off'
import flaskConicalIcon from '@iconify-icons/lucide/flask-conical'
import terminalIcon from '@iconify-icons/lucide/terminal'
import trash2Icon from '@iconify-icons/lucide/trash-2'
import undoIcon from '@iconify-icons/lucide/undo'
import redoIcon from '@iconify-icons/lucide/redo'
import copyIcon from '@iconify-icons/lucide/copy'
import languagesIcon from '@iconify-icons/lucide/languages'
import messageCircleIcon from '@iconify-icons/lucide/message-circle'
import botIcon from '@iconify-icons/lucide/bot'
import volume2Icon from '@iconify-icons/lucide/volume-2'
import pythonIcon from '@iconify-icons/vscode-icons/file-type-python'
import { createLogger } from '../../utils/logger'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiNotebookPythonEditor from './GuaxiNotebookPythonEditor.vue'
import GuaxiNotebookMarkdownEditor from './GuaxiNotebookMarkdownEditor.vue'
import GuaxiNotebookHtmlOutput from './GuaxiNotebookHtmlOutput.vue'
import GuaxiNotebookTableOutput from './GuaxiNotebookTableOutput.vue'
import GuaxiSelectionTranslatePanel from './GuaxiSelectionTranslatePanel.vue'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import {
  createPythonCellId,
  createPythonSessionId,
  dataframeToMarkdownTable,
  maskPythonCellBlocksForMarkdown,
  parsePythonCellBlocks,
  replacePythonCellBlock,
} from './utils/guaxiPythonCellSyntax'
import {
  appendPythonCellOutputModules,
  buildPythonCellOutputModules,
  getLatestPythonCellOutputModule,
  getLatestPythonCellOutputModuleByKind,
  getPythonCellOutputDisplayDataUrl,
  getPythonCellOutputDisplaySourcePath,
  isPythonCellDisplayOutputKind,
  normalizePythonCellOutputDisplayType,
  normalizePythonCellOutputModules,
  togglePythonCellOutputModuleCollapsed,
} from './utils/guaxiPythonCellOutputs'
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
import {
  NOTEBOOK_DATALAB_EVENT,
  buildNotebookLabDatasetDescriptors,
  buildNotebookLabImportPlan,
  buildNotebookLabMaterializeCode,
  buildNotebookLabMathCode,
  buildNotebookLabPlotCode,
  buildNotebookLabReportCode,
  buildNotebookLabVariableOperationCode,
  decorateNotebookLabOutputModules,
  installNotebookDataLabBridge,
  upsertNotebookLabImportRecord,
} from './utils/guaxiNotebookDataLab'

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
const fileTreeStore = useFileTreeStore()
const rightSiderStore = useRightSiderStore()
const siderStore = useSiderStore()
const NOTEBOOK_RIGHT_SIDER_VIEW_KEY = 'home'

const refreshTick = ref(0)
const pendingFocusCellKey = ref('')
const busyMap = ref({})
const notebookPendingPythonImport = ref(null)
const notebookQueuedPythonAction = ref(null)
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
const notebookSelectionToolbarVisible = ref(false)
const notebookSelectionToolbarStyle = ref({ left: '0px', top: '0px' })
const notebookSelectionText = ref('')
const notebookTopToolbarRevealed = ref(false)
const activeNotebookPythonCellKey = ref('')
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
const notebookLinkCardMetaCache = new Map()
const selectionTranslatePanelVisible = ref(false)
const selectionTranslatePinned = ref(false)
const selectionTranslatePanelPos = ref({ left: 8, top: 8 })
const selectionTranslateFrom = ref('auto')
const selectionTranslateTo = ref('zh')
const selectionTranslateBusy = ref(false)
const selectionTranslateRaw = ref('')
const selectionTranslateResult = ref('')
const selectionTranslateError = ref('')
const selectionTranslateItems = ref([])
const selectionTranslateWordMeta = ref(null)
const selectionReadAloudBusy = ref(false)
let notebookSidecarPersistTimer = 0
let notebookMarkdownEmitTimer = 0
let notebookSearchIndexSyncTimer = 0
let notebookSearchQueryTimer = 0
let notebookMarkdownWorkerTaskId = 0
let notebookMermaidRenderCounter = 0
let notebookTopToolbarHideTimer = 0
let selectionAudioElement = null
let selectionAudioObjectUrl = ''
let selectionTranslateMultiNotFoundWarned = false
let disposeNotebookDataLabBridge = null
let notebookDataLabRpcUnlisten = null

const themeClass = computed(() => (String(props.theme || '').toLowerCase() === 'dark' ? 'theme-dark' : 'theme-light'))
const isNotebookTopToolbarVisible = computed(() => notebookTopToolbarRevealed.value || notebookSearchVisible.value)
const activeNotebookPythonCell = computed(() => notebookCells.value.find((cell) => (
  cell?.type === 'python' && String(cell?.key || '') === String(activeNotebookPythonCellKey.value || '')
)) || null)
const selectionTranslateMultiAvailable = ref(true)
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
  { label: '泰语', value: 'th' },
]
const getSelectionTranslateLangLabel = (value) => {
  const item = selectionTranslateLangOptions.find((entry) => entry.value === value)
  return item ? String(item.label || '') : '中文'
}

const activeNotebookPythonCellSnapshot = computed(() => {
  if (!activeNotebookPythonCell.value) {
    return {
      meta: {
        outputCollapsed: false,
        sessionCollapsed: true,
      },
    }
  }
  return getCellSnapshot(activeNotebookPythonCell.value)
})
const isNotebookPythonSessionPanelVisible = computed(() => (
  rightSiderStore.isOpen
  && rightSiderStore.panel === 'python-session'
  && siderStore.isRightSiderCollapsed?.[NOTEBOOK_RIGHT_SIDER_VIEW_KEY] === false
))

const bumpRefresh = () => {
  refreshTick.value += 1
  emit('state-change')
}

// 设计说明：
// 1. 探索者模式顶部工具栏现在需要知道“当前选中的 Python Cell”是谁，才能把运行/导入/数据实验室面板等操作代理过去。
// 2. 这里把选中态收敛成一个单独的 key，而不是把整份 cell 对象长期缓存下来，避免 cell 重建后引用失效。
// 3. Markdown Cell 被点击时会清空这份选中态，保证顶部工具栏只在真正选中 Python Cell 时才影响它。
const setActiveNotebookPythonCellKey = (cellKey, reason = 'unknown') => {
  const normalizedKey = String(cellKey || '').trim()
  if (String(activeNotebookPythonCellKey.value || '') === normalizedKey) return
  activeNotebookPythonCellKey.value = normalizedKey
  logger.info('[NotebookToolbar] active python cell changed', {
    cellKey: normalizedKey,
    reason,
  })
}

const clearActiveNotebookPythonCell = (reason = 'unknown') => {
  if (!String(activeNotebookPythonCellKey.value || '').trim()) return
  setActiveNotebookPythonCellKey('', reason)
  rightSiderStore.clearPythonSessionInspector()
  logger.info('[NotebookToolbar] cleared python session inspector because active cell was cleared', {
    reason,
  })
}

const activateNotebookCell = (cell, reason = 'unknown') => {
  if (cell?.type === 'python') {
    setActiveNotebookPythonCellKey(String(cell?.key || ''), reason)
    revealNotebookTopToolbar(`python-cell-selected:${reason}`)
    return
  }
  clearActiveNotebookPythonCell(`non-python-cell:${reason}`)
}

const isNotebookInteractiveTarget = (target) => (
  target instanceof HTMLElement
  && !!target.closest('button, input, textarea, select, option, a, [contenteditable="true"]')
)

const handleNotebookCellPointerDown = (cell, event) => {
  if (cell?.type === 'python') {
    activateNotebookCell(cell, isNotebookInteractiveTarget(event?.target) ? 'python-interactive-pointerdown' : 'python-cell-pointerdown')
    return
  }
  activateNotebookCell(cell, 'cell-pointerdown')
}

// 设计说明：
// 1. Monaco 编辑器经常会在内部吞掉 pointer 事件，导致“已经在当前 Python Cell 中编辑/运行”，
//    但探索模式外层并不知道该 Cell 已经成为当前上下文。
// 2. 右侧“数据实验室”完全依赖 active cell 来同步会话变量，因此这里在编辑器 focus 时显式补一次激活。
// 3. 如果右侧实验室已经打开，则顺手同步最新 inspector，避免用户运行后还停留在空变量面板。
const handleNotebookPythonEditorFocus = (cell) => {
  if (cell?.type !== 'python') return
  activateNotebookCell(cell, 'python-editor-focus')
  if (isNotebookPythonSessionPanelVisible.value) {
    syncNotebookPythonSessionInspector(cell, 'python-editor-focus')
  }
  logger.debug('[Notebook] python editor focused and synced active cell context', {
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    sessionPanelVisible: isNotebookPythonSessionPanelVisible.value,
  })
}

const clearNotebookTopToolbarHideTimer = () => {
  if (!notebookTopToolbarHideTimer) return
  window.clearTimeout(notebookTopToolbarHideTimer)
  notebookTopToolbarHideTimer = 0
}

const NOTEBOOK_TOP_TOOLBAR_TRIGGER_DISTANCE = 56
const NOTEBOOK_TOP_TOOLBAR_HIDE_DISTANCE = 120

// 设计说明：
// 1. 探索模式顶部工具条默认压缩成极薄的吸顶感应层，只在鼠标靠近顶部或键盘聚焦时展开。
// 2. 搜索面板打开时强制保持工具条可见，避免搜索条和工具条的 sticky 叠层在滚动时来回跳动。
const setNotebookTopToolbarRevealed = (visible, reason = 'unknown') => {
  if (notebookTopToolbarRevealed.value === visible) return
  notebookTopToolbarRevealed.value = visible
  logger.info('[NotebookToolbar] top toolbar visibility changed', { visible, reason })
}

const revealNotebookTopToolbar = (reason = 'unknown') => {
  clearNotebookTopToolbarHideTimer()
  setNotebookTopToolbarRevealed(true, reason)
}

const hideNotebookTopToolbar = (reason = 'unknown') => {
  if (notebookSearchVisible.value) {
    logger.debug('[NotebookToolbar] keep toolbar visible because search panel is active', { reason })
    return
  }
  setNotebookTopToolbarRevealed(false, reason)
}

const scheduleHideNotebookTopToolbar = (reason = 'unknown') => {
  clearNotebookTopToolbarHideTimer()
  notebookTopToolbarHideTimer = window.setTimeout(() => {
    hideNotebookTopToolbar(reason)
  }, 220)
}

const handleNotebookToolbarFocusOut = (event) => {
  const nextTarget = event?.relatedTarget
  if (nextTarget instanceof Node && notebookRootRef.value?.contains?.(nextTarget)) return
  scheduleHideNotebookTopToolbar('toolbar-focusout')
}

const resolveNotebookToolbarTriggerRect = () => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return null
  const scrollHost = root.parentElement
  if (scrollHost instanceof HTMLElement) {
    return scrollHost.getBoundingClientRect()
  }
  return root.getBoundingClientRect()
}

// 设计说明：
// 1. 触发区域改成“探索者滚动视口的顶部一段距离”，而不是隐藏后的 8px 细条，避免工具条实际上不可达。
// 2. 这里使用 notebook 的滚动宿主边界作为基准，这样文档滚动很深时，鼠标靠近当前可视区顶部依然能唤起工具条。
const handleNotebookRootMouseMove = (event) => {
  const triggerRect = resolveNotebookToolbarTriggerRect()
  if (!triggerRect) return
  const offsetFromTop = Number(event?.clientY || 0) - triggerRect.top
  if (offsetFromTop <= NOTEBOOK_TOP_TOOLBAR_TRIGGER_DISTANCE) {
    revealNotebookTopToolbar('root-mouse-near-top')
    return
  }
  if (offsetFromTop >= NOTEBOOK_TOP_TOOLBAR_HIDE_DISTANCE) {
    scheduleHideNotebookTopToolbar('root-mouse-away-from-top')
  }
}

const handleNotebookRootMouseLeave = () => {
  scheduleHideNotebookTopToolbar('root-mouseleave')
}

const normalizeNotebookMarkdownText = (markdown = '') => String(markdown || '').replace(/\r\n?/g, '\n')
const escapeRegExp = (raw) => String(raw || '').replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
const NOTEBOOK_SELECTION_SURFACE_SELECTOR = '.guaxi-notebook__markdown-preview, .guaxi-notebook__rich, .guaxi-notebook__rich--html'
const NOTEBOOK_SELECTION_IGNORE_SELECTOR = '.guaxi-notebook__toolbar, .guaxi-notebook__search-panel, .guaxi-notebook__selection-toolbar, .pot-selection-translate-panel, textarea, input, select, button, [contenteditable="true"]'
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
// 设计说明：
// 1. Explorer 模式里的 wikilink、卡片元信息和点击打开都会复用这个路径归一化函数。
// 2. 旧实现先把 `E:/foo/bar.md` 拆成 `['E:', 'foo', 'bar.md']`，再额外补一次 `E:/` 前缀，
//    最终生成 `E:/E:/foo/bar.md`，这正是你日志里看到的重复盘符问题。
// 3. 这里改成和主编辑器同一思路：先识别锚定前缀（盘符/UNC/根目录），再只归一化后续路径片段，
//    这样绝对路径、相对路径、`..` 回退都能稳定工作。
const normalizeNotebookFsPath = (rawPath = '') => {
  let normalized = String(rawPath || '').trim().replace(/\\/g, '/')
  if (!normalized) return ''

  if (/^\/[A-Za-z]:\//.test(normalized)) {
    normalized = normalized.slice(1)
  }

  let anchored = false
  let prefix = ''
  let parts = []

  if (/^[A-Za-z]:\//.test(normalized)) {
    anchored = true
    prefix = normalized.slice(0, 3)
    parts = normalized.slice(3).split('/')
  } else if (normalized.startsWith('//')) {
    const uncParts = normalized.slice(2).split('/').filter(Boolean)
    if (uncParts.length >= 2) {
      anchored = true
      prefix = `//${uncParts[0]}/${uncParts[1]}`
      parts = uncParts.slice(2)
    } else {
      return normalized
    }
  } else if (normalized.startsWith('/')) {
    anchored = true
    prefix = '/'
    parts = normalized.slice(1).split('/')
  } else {
    parts = normalized.split('/')
  }

  const output = []
  for (const part0 of parts) {
    const part = String(part0 || '').trim()
    if (!part || part === '.') continue
    if (part === '..') {
      if (output.length && output[output.length - 1] !== '..') {
        output.pop()
      } else if (!anchored) {
        output.push(part)
      }
      continue
    }
    output.push(part)
  }

  if (!output.length) return prefix
  if (!prefix) return output.join('/')
  return prefix.endsWith('/') ? `${prefix}${output.join('/')}` : `${prefix}/${output.join('/')}`
}

const writeClipboardText = async (text) => {
  const value = String(text || '')
  if (!value) return false

  try {
    await navigator.clipboard.writeText(value)
    return true
  } catch (error) {
    logger.warn('[NotebookSelection] navigator clipboard write failed', {
      error: String(error?.message || error),
    })
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (error) {
    logger.error('[NotebookSelection] clipboard write failed', {
      error: String(error?.message || error),
    })
    return false
  }
}

const readClipboardText = async () => {
  if (navigator?.clipboard?.readText) {
    try {
      return String(await navigator.clipboard.readText())
    } catch (error) {
      logger.warn('[NotebookDataLab] navigator clipboard read failed', {
        error: String(error?.message || error),
      })
    }
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    return String(await mod.readText())
  } catch (error) {
    logger.error('[NotebookDataLab] clipboard read failed', {
      error: String(error?.message || error),
    })
    throw new Error('读取剪贴板失败')
  }
}

const normalizeSelectionTextForLanguageTasks = (raw) => {
  const text = String(raw || '').replace(/\r\n?/g, '\n')
  if (!text.trim()) return ''
  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/([^\n，。！？?!;；:：])\n(?!\n)/g, '$1 ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

const isWordSelection = (raw) => {
  const text = String(raw || '').trim()
  if (!text || /\s/.test(text)) return false
  return /^[A-Za-z][A-Za-z'-]{0,47}$/.test(text)
}

const hideNotebookSelectionToolbar = () => {
  notebookSelectionToolbarVisible.value = false
}

const clearNotebookSelectionState = (reason = 'unknown') => {
  notebookSelectionText.value = ''
  hideNotebookSelectionToolbar()
  logger.info('[NotebookSelection] cleared selection state', { reason })
}

const stopSelectionAudioPlayback = () => {
  if (selectionAudioElement) {
    try {
      selectionAudioElement.pause()
      selectionAudioElement.src = ''
    } catch {}
    selectionAudioElement = null
  }
  if (selectionAudioObjectUrl) {
    try {
      URL.revokeObjectURL(selectionAudioObjectUrl)
    } catch {}
    selectionAudioObjectUrl = ''
  }
}

// 设计说明：
// 1. Explorer 模式里存在多个渲染面：Markdown 预览、输出面板、HTML 输出容器。
// 2. 这里统一从 `window.getSelection()` 反查最近的合法渲染面，避免把工具条错误地挂到搜索框、
//    Cell 编辑器或工具按钮上。
const resolveNotebookSelectionSurface = (selection, fallbackTarget = null) => {
  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement) || !selection) return null

  const candidates = [
    selection.anchorNode,
    selection.focusNode,
    selection.rangeCount > 0 ? selection.getRangeAt(0).commonAncestorContainer : null,
    fallbackTarget,
  ]

  for (const node of candidates) {
    const element = node instanceof Element ? node : node?.parentElement
    if (!(element instanceof Element)) continue
    const surface = element.closest(NOTEBOOK_SELECTION_SURFACE_SELECTOR)
    if (surface instanceof HTMLElement && root.contains(surface)) {
      return surface
    }
  }

  return null
}

const getNotebookSelectionTextFromWindow = (fallbackTarget = null) => {
  const selection = window.getSelection?.()
  const root = notebookRootRef.value
  if (!selection || !(root instanceof HTMLElement) || selection.rangeCount === 0) {
    return { text: '', surface: null }
  }

  const text = normalizeSelectionTextForLanguageTasks(selection.toString())
  if (!text) return { text: '', surface: null }

  const surface = resolveNotebookSelectionSurface(selection, fallbackTarget)
  if (!(surface instanceof HTMLElement)) {
    return { text: '', surface: null }
  }

  return { text, surface }
}

const getNotebookSelectionTextForLanguageTasks = () => {
  const text = normalizeSelectionTextForLanguageTasks(
    notebookSelectionText.value || getNotebookSelectionTextFromWindow().text,
  )
  return text
}

const scheduleSelectionTranslate = debounce(() => {
  if (!selectionTranslatePanelVisible.value) return
  if (selectionTranslateBusy.value) return

  const raw = String(selectionTranslateRaw.value || '').trim()
  if (!raw) return

  translateNotebookSelectionText(raw)
}, 250)
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
const isPythonCellSnapshotFresh = (cell, snapshot) => {
  const currentCodeDigest = getNotebookCellContentDigest(cell)
  const snapshotCodeDigest = String(snapshot?.codeDigest || '').trim()
  return !!currentCodeDigest && !!snapshotCodeDigest && currentCodeDigest === snapshotCodeDigest
}
const buildStalePythonCellSnapshot = (cell, snapshot = {}, reason = 'unknown') => {
  const nextSnapshot = {
    codeDigest: getNotebookCellContentDigest(cell),
    outputText: '',
    display: null,
    dataframe: null,
    outputs: [],
    result: null,
    // 设计说明：
    // 1. Python Notebook 这里使用的是“文件级持久会话”，代码被编辑后，旧输出确实应该失效；
    // 2. 但会话里的变量并不会因为用户改了一行代码就自动从 Python 进程里消失，
    //    否则就会出现“cell 中还能 print 出来，右侧实验室却显示 0 个变量”的割裂体验；
    // 3. 因此 stale snapshot 只清空依赖当前代码版本的输出，不再顺手清空 sessionVars。
    sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    lab: snapshot?.lab && typeof snapshot.lab === 'object'
      ? {
        ...snapshot.lab,
        imports: Array.isArray(snapshot?.lab?.imports) ? [...snapshot.lab.imports] : [],
      }
      : {
        imports: [],
      },
    meta: snapshot?.meta && typeof snapshot.meta === 'object' ? { ...snapshot.meta } : {},
  }
  logger.info('[Notebook] invalidate stale python snapshot', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    stableId: getNotebookCellStableId(cell),
    reason,
    previousCodeDigest: String(snapshot?.codeDigest || ''),
    nextCodeDigest: nextSnapshot.codeDigest,
    preservedSessionVarCount: nextSnapshot.sessionVars.length,
  })
  return nextSnapshot
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
      outputs: Array.isArray(snapshot?.outputs) ? snapshot.outputs : [],
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      lab: snapshot?.lab && typeof snapshot.lab === 'object'
        ? {
          ...snapshot.lab,
          imports: Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
        }
        : {
          imports: [],
        },
      meta: snapshot?.meta || {},
    }
  })
  return snapshots
}
const applyNotebookSidecarSnapshots = (cells = [], snapshots = {}) => {
  const nextSnapshots = {}
  let restoredCount = 0
  let skippedCount = 0
  normalizeNotebookCells(cells).forEach((cell) => {
    if (cell?.type !== 'python') return
    const cellId = String(cell?.cellId || '')
    const stableId = getNotebookCellStableId(cell)
    if (!cellId && !stableId) return
    const persisted = snapshots && typeof snapshots === 'object'
      ? snapshots[stableId] || snapshots[cellId] || null
      : null
    if (!persisted || typeof persisted !== 'object') return
    const snapshotCodeDigest = String(persisted.codeDigest || '').trim()
    const currentCodeDigest = getNotebookCellContentDigest(cell)
    if (!snapshotCodeDigest || snapshotCodeDigest !== currentCodeDigest) {
      skippedCount += 1
      logger.info('[Notebook] skip stale sidecar snapshot', {
        cellId,
        stableId,
        snapshotCodeDigest,
        currentCodeDigest,
      })
      return
    }
    nextSnapshots[cellId] = {
      codeDigest: snapshotCodeDigest,
      outputText: String(persisted.outputText || ''),
      display: persisted.display || null,
      dataframe: persisted.dataframe || null,
      outputs: Array.isArray(persisted.outputs) ? persisted.outputs : [],
      sessionVars: Array.isArray(persisted.sessionVars) ? persisted.sessionVars : [],
      lab: persisted?.lab && typeof persisted.lab === 'object'
        ? {
          ...persisted.lab,
          imports: Array.isArray(persisted?.lab?.imports) ? persisted.lab.imports : [],
        }
        : {
          imports: [],
        },
      meta: persisted.meta && typeof persisted.meta === 'object' ? persisted.meta : {},
    }
    restoredCount += 1
  })
  replacePythonCellSnapshots(props.filePath, nextSnapshots)
  logger.info('[Notebook] applied sidecar python snapshots', {
    cellCount: normalizeNotebookCells(cells).filter((cell) => cell?.type === 'python').length,
    restoredCount,
    skippedCount,
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
    const currentStorage = String(cell?.meta?.storage || '').trim()
    const currentCode = String(cell?.code || '')
    const persistedCode = String(persistedCell?.code || '')
    const shouldRestoreSidecarCode =
      cell?.type === 'python' &&
      currentStorage === 'cellmeta' &&
      !currentCode &&
      !!persistedCode
    if (matchStrategy === 'stable-id') {
      stableIdMatchedCount += 1
    } else if (matchStrategy === 'code-digest') {
      codeDigestMatchedCount += 1
    }
    if (shouldRestoreSidecarCode) {
      logger.info('[Notebook] restored python cell code from sidecar reference block', {
        stableId,
        codeLength: persistedCode.length,
        matchStrategy,
      })
    }
    return createNotebookPythonCell({
      ...cell,
      code: shouldRestoreSidecarCode ? persistedCode : String(cell.code || ''),
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
  return normalizePythonCellOutputDisplayType(display)
}
const getDisplaySourcePath = (display) => getPythonCellOutputDisplaySourcePath(display)
const getDisplayDataUrl = (display) => getPythonCellOutputDisplayDataUrl(display)

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
    version: 4,
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
    const outlineSource = maskPythonCellBlocksForMarkdown(content)
    const tokens = notebookOutlineMarkdownParser.parse(outlineSource, {})
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

// 设计说明：
// 1. 探索者模式是按 Cell 分段渲染的，markdown-it 的 TOC 插件拿不到“整篇文档”的标题上下文，
//    因此不能直接复用单 Cell 的目录输出。
// 2. 这里显式识别 [toc] / [[toc]] / [目录] 这类占位标记，再用 notebookOutlineEntries 汇总出的整篇标题树生成目录 HTML。
// 3. 目录中的锚点直接指向我们自己写入 heading 的 id，保证探索者模式里的目录跳转和右侧大纲共用同一批标题节点。
const NOTEBOOK_TOC_MARKER_RE = /^\s*(?:\[\[\s*(?:toc|目录)\s*\]\]|\[\s*(?:toc|目录)\s*\])\s*$/im

const hasNotebookTocMarker = (markdown = '') => NOTEBOOK_TOC_MARKER_RE.test(String(markdown || ''))

const isNotebookTocMarkerLine = (line = '') => /^\s*(?:\[\[\s*(?:toc|目录)\s*\]\]|\[\s*(?:toc|目录)\s*\])\s*$/i.test(String(line || ''))

const splitNotebookMarkdownByTocMarkers = (markdown = '') => {
  const lines = String(markdown || '').replace(/\r\n?/g, '\n').split('\n')
  const parts = []
  let chunkStart = 0
  let fenceState = { active: false, char: '', length: 0 }

  const pushMarkdownChunk = (startIndex, endExclusive) => {
    const chunk = lines.slice(startIndex, endExclusive).join('\n')
    if (chunk.trim()) {
      parts.push({
        type: 'markdown',
        content: chunk,
      })
    }
  }

  lines.forEach((line, index) => {
    const textLine = String(line || '')
    if (!fenceState.active && isNotebookTocMarkerLine(textLine)) {
      pushMarkdownChunk(chunkStart, index)
      parts.push({
        type: 'toc',
        content: textLine,
      })
      chunkStart = index + 1
      return
    }

    const fenceMatch = textLine.match(/^\s*(`{3,}|~{3,})/)
    if (!fenceMatch) return

    const fenceText = String(fenceMatch[1] || '')
    const fenceChar = fenceText[0] || ''
    const fenceLength = fenceText.length
    if (!fenceState.active) {
      fenceState = { active: true, char: fenceChar, length: fenceLength }
      return
    }
    if (fenceState.char === fenceChar && fenceLength >= fenceState.length) {
      fenceState = { active: false, char: '', length: 0 }
    }
  })

  pushMarkdownChunk(chunkStart, lines.length)
  return parts
}

// 设计说明：
// 1. `[TOC]` 是一个行级占位符，不应该吞掉同一个 Markdown Cell 中的其它正文。
// 2. 因此这里按行切分，只把真正的 TOC 标记行替换成探索者模式的整篇目录 HTML。
// 3. 切分时显式跳过 fenced code block，避免示例代码里的 `[TOC]` 被误当作目录占位。
const renderNotebookMarkdownHtmlWithSyntheticToc = async (markdownText, cellKey, reason = 'unknown') => {
  const parts = splitNotebookMarkdownByTocMarkers(markdownText)
  if (!parts.length) return ''

  const htmlParts = []
  let tocCount = 0
  let markdownChunkCount = 0
  for (const part of parts) {
    if (part.type === 'toc') {
      tocCount += 1
      htmlParts.push(buildNotebookTocHtml({ currentCellKey: cellKey }))
      continue
    }
    markdownChunkCount += 1
    htmlParts.push(await renderMarkdownHtml(part.content, `${reason}:toc-chunk:${markdownChunkCount}`))
  }

  logger.info('[Notebook] rendered markdown cell with inline synthetic toc', {
    reason,
    cellKey,
    tocCount,
    markdownChunkCount,
    filePath: props.filePath,
  })
  return htmlParts.join('\n')
}

// 设计说明：
// 1. 探索者模式此前直接把 markdown 原文送进 worker，`[[note]]` 这类 Obsidian wikilink 不会被 markdown-it 识别成 <a>。
// 2. 文件卡片提升依赖最终 HTML 中已经存在链接节点，因此这里先把 wikilink 统一预处理成内部链接协议。
// 3. 代码块内的内容必须保持原样，避免把示例代码中的 `[[...]]` 误转成真实链接。
const preprocessNotebookObsidianWikilinks = (content = '') => {
  const text = String(content || '')
  if (!text.includes('[[')) return text

  const lines = text.replace(/\r/g, '').split('\n')
  let inFence = false
  let fenceToken = ''

  const out = lines.map((line) => {
    const fence = line.match(/^\s*(```+|~~~+)/)
    if (fence) {
      const token = String(fence[1] || '')
      if (!inFence) {
        inFence = true
        fenceToken = token
      } else if (token.startsWith(fenceToken)) {
        inFence = false
        fenceToken = ''
      }
      return line
    }

    if (inFence || !line.includes('[[')) return line

    return line.replace(/\[\[([^\]]+?)\]\]/g, (matched, innerRaw) => {
      const inner = String(innerRaw || '').trim()
      if (!inner || /^(toc|目录)$/i.test(inner)) return matched

      const parts = inner.split('|')
      const target = String(parts[0] || '').trim()
      const alias = String(parts[1] || '').trim()

      let filePart = target
      let anchorPart = ''
      const hashIndex = target.indexOf('#')
      if (hashIndex >= 0) {
        filePart = target.slice(0, hashIndex).trim()
        anchorPart = target.slice(hashIndex + 1).trim()
      }

      const display = alias || anchorPart || filePart || target
      const safeText = display.replace(/([\[\]\\])/g, '\\$1')
      const href = `guaxi://wikilink?file=${encodeURIComponent(filePart)}&anchor=${encodeURIComponent(anchorPart)}`
      return `[${safeText}](${href})`
    })
  })

  return out.join('\n')
}

const buildNotebookMarkdownCacheKey = (content, options = {}) => {
  const markdownText = String(content || '')
  const theme = String(options.theme || props.theme || '')
  const filePath = normalizeNotebookFsPath(options.filePath || props.filePath || '')
  const basePath = normalizeNotebookFsPath(options.basePath || getNotebookBasePath())
  return `${theme}::${filePath}::${basePath}::${markdownText}`
}

const escapeNotebookHtml = (value) => String(value ?? '')
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const buildNotebookTocHtml = ({ currentCellKey = '' } = {}) => {
  const entries = Array.isArray(notebookOutlineEntries.value) ? notebookOutlineEntries.value : []
  const tocEntries = entries.filter((entry) => {
    const outlineId = String(entry?.outlineId || '').trim()
    if (!outlineId) return false
    // 设计说明：
    // 如果目录占位本身写在一个带标题的 markdown Cell 里，目录中不再回指当前占位 Cell 的标题，避免自引用。
    if (currentCellKey && String(entry?.cellKey || '') === String(currentCellKey || '')) return false
    return true
  })

  if (!tocEntries.length) {
    logger.info('[Notebook] build toc html skipped because no outline entries were found', {
      currentCellKey,
    })
    return `
      <nav class="guaxi-notebook__toc" data-guaxi-notebook-toc="true">
        <div class="guaxi-notebook__toc-title">目录</div>
        <div class="guaxi-notebook__toc-empty">当前文档还没有可用标题</div>
      </nav>
    `
  }

  const itemsHtml = tocEntries.map((entry) => {
    const level = Math.max(1, Number(entry?.level || 1))
    const outlineId = String(entry?.outlineId || '').trim()
    const text = escapeNotebookHtml(String(entry?.text || '').trim() || '未命名标题')
    const line = Math.max(1, Number(entry?.line || 1))
    return `
      <li class="guaxi-notebook__toc-item level-${level}">
        <a
          class="guaxi-notebook__toc-link"
          href="#${outlineId}"
          data-guaxi-outline-id="${outlineId}"
          data-guaxi-outline-line="${line}"
        >${text}</a>
      </li>
    `
  }).join('')

  logger.info('[Notebook] build toc html', {
    currentCellKey,
    headingCount: tocEntries.length,
  })

  return `
    <nav class="guaxi-notebook__toc" data-guaxi-notebook-toc="true">
      <div class="guaxi-notebook__toc-title">目录</div>
      <ol class="guaxi-notebook__toc-list">
        ${itemsHtml}
      </ol>
    </nav>
  `
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
    const outlineId = String(entry.outlineId || '')
    headingNode.setAttribute('data-guaxi-outline-id', outlineId)
    headingNode.setAttribute('data-guaxi-outline-line', String(entry.line || ''))
    if (outlineId) {
      headingNode.id = outlineId
    }
  })

  return container.innerHTML
}

// 设计说明：
// 1. 目录 HTML 里的 `<a data-guaxi-outline-id="...">` 和真正的标题节点共享同一个 outlineId。
// 2. 如果直接用 `[data-guaxi-outline-id="..."]` 查询，命中的往往是目录链接本身，
//    结果滚动会落到目录区域，而不是对应 markdown Cell 内的标题。
// 3. 这里统一优先查找带同名 id / data 属性的 heading，必要时再回退到非目录容器中的任意元素。
const findNotebookHeadingTargetByOutlineId = (outlineId) => {
  const root = notebookRootRef.value
  const normalizedOutlineId = String(outlineId || '').trim()
  if (!(root instanceof HTMLElement) || !normalizedOutlineId) return null

  const escapedId = typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
    ? CSS.escape(normalizedOutlineId)
    : normalizedOutlineId.replace(/["\\]/g, '\\$&')
  const escapedAttrValue = normalizedOutlineId.replace(/"/g, '&quot;')

  const headingById = root.querySelector(`h1#${escapedId}, h2#${escapedId}, h3#${escapedId}, h4#${escapedId}, h5#${escapedId}, h6#${escapedId}`)
  if (headingById instanceof HTMLElement) return headingById

  const headingByData = root.querySelector(
    `h1[data-guaxi-outline-id="${escapedAttrValue}"], h2[data-guaxi-outline-id="${escapedAttrValue}"], h3[data-guaxi-outline-id="${escapedAttrValue}"], h4[data-guaxi-outline-id="${escapedAttrValue}"], h5[data-guaxi-outline-id="${escapedAttrValue}"], h6[data-guaxi-outline-id="${escapedAttrValue}"]`,
  )
  if (headingByData instanceof HTMLElement) return headingByData

  const nonTocTarget = Array.from(root.querySelectorAll(`[data-guaxi-outline-id="${escapedAttrValue}"]`)).find((node) => {
    return node instanceof HTMLElement && !node.closest('[data-guaxi-notebook-toc="true"]')
  })
  return nonTocTarget instanceof HTMLElement ? nonTocTarget : null
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
      const outputTexts = getCellOutputModules(cell)
        .map((outputModule) => buildOutputModuleSearchText(outputModule))
        .filter(Boolean)
      const sessionText = Array.isArray(snapshot?.sessionVars)
        ? snapshot.sessionVars
          .map((item) => `${item?.name || ''} ${item?.value_type || item?.type || ''} ${item?.preview || ''}`.trim())
          .join('\n')
        : ''
      const parts = [
        String(cell?.meta?.title || ''),
        String(cell?.code || ''),
        ...outputTexts,
        sessionText,
      ].filter(Boolean)
      const searchText = parts.join('\n\n')
      return {
        cell_key: String(cell?.key || ''),
        cell_type: 'python',
        digest: computeNotebookDigest(searchText),
        search_text: searchText,
        summary_text: [String(cell?.meta?.title || ''), ...outputTexts.slice(0, 2)]
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
  revealNotebookTopToolbar('open-search')
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
  scheduleHideNotebookTopToolbar('close-search')
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
      ? findNotebookHeadingTargetByOutlineId(outlineId)
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

const updatePythonCell = (key, payload) => {
  if (payload?.target) {
    resizeTextareaToContent(payload.target)
  }
  const value = typeof payload === 'string'
    ? payload
    : String(payload?.target?.value || '')
  const targetCell = notebookCells.value.find((cell) => cell?.type === 'python' && String(cell?.key || '') === String(key))
  if (targetCell) {
    const currentDigest = getNotebookCellContentDigest(targetCell)
    const nextDigest = computeNotebookDigest(value)
    if (currentDigest !== nextDigest) {
      const currentSnapshot = getPythonCellSnapshot(props.filePath, targetCell.cellId)
      patchPythonCellSnapshot(
        props.filePath,
        targetCell.cellId,
        buildStalePythonCellSnapshot(targetCell, currentSnapshot, 'code-edited'),
      )
      bumpRefresh()
    }
  }
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
    clearActiveNotebookPythonCell('insert-markdown-cell')
  } else {
    setActiveNotebookPythonCellKey(String(newCell.key || ''), 'insert-python-cell')
    revealNotebookTopToolbar('insert-python-cell')
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

watch(notebookCells, (cells) => {
  const activeKey = String(activeNotebookPythonCellKey.value || '')
  if (!activeKey) return
  const stillExists = cells.some((cell) => cell?.type === 'python' && String(cell?.key || '') === activeKey)
  if (!stillExists) {
    clearActiveNotebookPythonCell('active-cell-removed')
  }
}, { deep: false })

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
  // 设计说明：
  // Python 代码块“正常执行但没有显式输出”时，最后一个表达式可能只是 `None`。
  // 这种值对探索模式用户来说属于噪音，不应该伪装成一次可见输出。
  if (resultRepr && resultRepr !== 'None' && !stdout.includes(resultRepr)) chunks.push(resultRepr)
  return chunks.join('\n\n').trim()
}
const getNotebookRepoRoot = () => {
  const normalizedFilePath = normalizeNotebookFsPath(props.filePath)
  if (!normalizedFilePath) return ''
  const normalizedRepoCandidates = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  let best = ''
  for (const candidate of normalizedRepoCandidates) {
    const normalizedCandidate = normalizeNotebookFsPath(candidate)
    if (!normalizedCandidate) continue
    if (normalizedFilePath === normalizedCandidate || normalizedFilePath.startsWith(`${normalizedCandidate}/`)) {
      if (normalizedCandidate.length > best.length) best = normalizedCandidate
    }
  }
  return best
}
const splitNotebookFsLinkAndAnchor = (href = '') => {
  const raw = String(href || '').trim()
  if (!raw) return { link: '', anchor: '' }
  const hashIndex = raw.indexOf('#')
  if (hashIndex < 0) return { link: raw, anchor: '' }
  return {
    link: raw.slice(0, hashIndex),
    anchor: raw.slice(hashIndex + 1),
  }
}
const ensureNotebookMarkdownExt = (raw = '') => {
  const normalized = String(raw || '').trim()
  if (!normalized) return ''
  if (/\.(md|markdown)$/i.test(normalized)) return normalized
  if (/[.][^/\\]+$/.test(normalized)) return normalized
  return `${normalized}.md`
}
const isNotebookHttpUrl = (href = '') => /^https?:\/\//i.test(String(href || '').trim())
const isNotebookMarkdownFilePath = (filePath = '') => /\.(md|markdown)$/i.test(String(filePath || '').trim())
const isNotebookWikilinkHref = (href = '') => String(href || '').trim().toLowerCase().startsWith('guaxi://wikilink')
const isNotebookPathLikeToken = (href = '') => {
  const raw = String(href || '').trim()
  if (!raw) return false
  if (raw.startsWith('#') || isNotebookHttpUrl(raw) || /^mailto:/i.test(raw)) return false
  return isNotebookWikilinkHref(raw) || raw.startsWith('.') || raw.startsWith('/') || /^[A-Za-z]:[\\/]/.test(raw) || raw.includes('/') || raw.includes('\\')
}
const resolveNotebookRelativePath = (target, baseDir) => {
  const normalizedTarget = String(target || '').trim()
  if (!normalizedTarget) return ''
  if (/^[A-Za-z]:[\\/]/.test(normalizedTarget)) return normalizeNotebookFsPath(normalizedTarget)
  if (normalizedTarget.startsWith('/')) return normalizeNotebookFsPath(normalizedTarget)
  return normalizeNotebookFsPath(joinNotebookPath(baseDir || '', normalizedTarget))
}
const buildNotebookLinkCardCacheKey = ({ targetPath = '', repoRoot = '', mtimeMs = null }) => {
  const normalizedTargetPath = normalizeNotebookFsPath(targetPath)
  const normalizedRepoRoot = normalizeNotebookFsPath(repoRoot)
  const normalizedMtime = Number.isFinite(Number(mtimeMs)) ? String(Number(mtimeMs)) : 'unknown'
  return `${normalizedRepoRoot}::${normalizedTargetPath}::${normalizedMtime}`
}
const extractNotebookLinkCardExcerpt = (content = '') => {
  const lines = String(content || '')
    .replace(/\r/g, '')
    .split('\n')
    .map((line) => String(line || '').trim())
    .filter(Boolean)
  for (const line of lines) {
    if (/^\s{0,3}#{1,6}\s+/.test(line)) continue
    if (/^\s*>/.test(line)) continue
    if (/^\s*[-*+]\s+/.test(line)) continue
    if (/^\s*```/.test(line)) continue
    const cleaned = line
      .replace(/!\[[^\]]*\]\([^)]+\)/g, '')
      .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
      .replace(/\[\[([^\]]+)\]\]/g, '$1')
      .replace(/[`*_>#-]/g, ' ')
      .replace(/\s+/g, ' ')
      .trim()
    if (cleaned) return cleaned.slice(0, 96)
  }
  return '打开关联内容'
}
const buildNotebookLinkCardTitle = (content, absPath, fallbackText = '') => {
  const heading = maskPythonCellBlocksForMarkdown(content).match(/^\s{0,3}#{1,6}\s+(.+)$/m)
  if (heading?.[1]) {
    return String(heading[1] || '').replace(/\s+#+\s*$/, '').trim()
  }
  const normalizedPath = normalizeNotebookFsPath(absPath)
  const fileName = normalizedPath.split('/').pop() || String(fallbackText || '').trim() || normalizedPath
  return fileName.replace(/\.(md|markdown)$/i, '') || '未命名笔记'
}
const buildNotebookLinkCardPathLabel = (absPath, repoRoot = '') => {
  const normalizedPath = normalizeNotebookFsPath(absPath)
  const normalizedRepoRoot = normalizeNotebookFsPath(repoRoot)
  if (normalizedRepoRoot && normalizedPath.startsWith(`${normalizedRepoRoot}/`)) {
    return normalizedPath.slice(normalizedRepoRoot.length + 1)
  }
  return normalizedPath
}
const resolveNotebookLinkTargetPath = async (href, { baseDocPath } = {}) => {
  const rawHref = String(href || '').trim()
  if (!rawHref) return ''
  const normalizedBaseDocPath = normalizeNotebookFsPath(baseDocPath || '')
  const baseDir = normalizedBaseDocPath
    ? normalizedBaseDocPath.replace(/\/[^/]*$/, '')
    : getNotebookDocDir()
  if (rawHref.toLowerCase().startsWith('guaxi://wikilink')) {
    try {
      const url = new URL(rawHref)
      const file = String(url.searchParams.get('file') || '').trim()
      if (!file) return ''
      return resolveNotebookRelativePath(ensureNotebookMarkdownExt(file), baseDir)
    } catch (error) {
      logger.warn('[NotebookLinkCard] resolve wikilink target failed', {
        href: rawHref,
        baseDocPath,
        error: String(error?.message || error),
      })
      return ''
    }
  }
  if (!isNotebookPathLikeToken(rawHref)) return ''
  const { link } = splitNotebookFsLinkAndAnchor(rawHref)
  const resolvedPath = resolveNotebookRelativePath(link, baseDir)
  logger.info('[NotebookLinkCard] resolved notebook link target path', {
    href: rawHref,
    baseDocPath: normalizedBaseDocPath,
    baseDir,
    resolvedPath,
  })
  return resolvedPath
}
const canPromoteNotebookLinkToCard = (anchor) => {
  if (!(anchor instanceof HTMLAnchorElement)) return false
  const href = String(anchor.getAttribute('href') || '').trim()
  if (!href || href.startsWith('#') || isNotebookHttpUrl(href)) return false
  const container = anchor.closest('p, li, div')
  if (!container) return false
  const allAnchors = container.querySelectorAll('a')
  if (allAnchors.length !== 1 || allAnchors[0] !== anchor) return false
  const normalizeStandaloneLinkText = (value = '') => String(value || '')
    .replace(/\[\[|\]\]|\[|\]|\(|\)|【|】/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  const blockText = normalizeStandaloneLinkText(container.textContent || '')
  const anchorText = normalizeStandaloneLinkText(anchor.textContent || '')
  if (!!anchorText && blockText === anchorText) return true

  const onlyHasAnchorAndWhitespace = Array.from(container.childNodes || []).every((node) => {
    if (node === anchor) return true
    if (node.nodeType === Node.TEXT_NODE) {
      return !String(node.textContent || '').trim()
    }
    if (node.nodeType === Node.ELEMENT_NODE) {
      const element = node
      if (element instanceof HTMLBRElement) return true
      return element.contains(anchor) && normalizeStandaloneLinkText(element.textContent || '') === anchorText
    }
    return false
  })
  return !!anchorText && onlyHasAnchorAndWhitespace
}
const loadNotebookLinkCardMeta = async (targetPath, { repoRoot = '' } = {}) => {
  const normalizedTargetPath = normalizeNotebookFsPath(targetPath)
  if (!normalizedTargetPath) return null
  try {
    const fileMeta = await invoke('get_file_meta', { path: normalizedTargetPath })
    const cacheKey = buildNotebookLinkCardCacheKey({
      targetPath: normalizedTargetPath,
      repoRoot,
      mtimeMs: fileMeta?.mtime_ms,
    })
    const cached = notebookLinkCardMetaCache.get(cacheKey)
    if (cached) {
      logger.info('[NotebookLinkCard] meta cache hit', {
        path: normalizedTargetPath,
        repoRoot,
        mtimeMs: fileMeta?.mtime_ms ?? null,
      })
      return cached
    }
    const isDirectory = !!fileMeta?.is_directory
    let content = ''
    if (!isDirectory && isNotebookMarkdownFilePath(normalizedTargetPath)) {
      content = String(await invoke('read_file_content', { path: normalizedTargetPath }) || '')
    }
    const meta = {
      path: normalizedTargetPath,
      repoRoot: normalizeNotebookFsPath(repoRoot),
      mtimeMs: fileMeta?.mtime_ms ?? null,
      kind: isDirectory ? 'directory' : 'file',
      title: isDirectory
        ? (normalizedTargetPath.split('/').pop() || '目录')
        : buildNotebookLinkCardTitle(content, normalizedTargetPath),
      excerpt: isDirectory
        ? '打开目录并继续浏览其中的笔记与资源'
        : extractNotebookLinkCardExcerpt(content),
      pathLabel: buildNotebookLinkCardPathLabel(normalizedTargetPath, repoRoot),
    }
    notebookLinkCardMetaCache.set(cacheKey, meta)
    logger.info('[NotebookLinkCard] meta loaded', {
      path: normalizedTargetPath,
      kind: meta.kind,
      repoRoot,
      mtimeMs: meta.mtimeMs,
    })
    return meta
  } catch (error) {
    logger.warn('[NotebookLinkCard] load meta failed', {
      path: normalizedTargetPath,
      repoRoot,
      error: String(error?.message || error),
    })
    return null
  }
}
const enhanceNotebookHtmlWithLinkCards = async (html, { baseDocPath } = {}) => {
  const rawHtml = String(html || '')
  if (!rawHtml.includes('<a')) return rawHtml
  const root = document.createElement('div')
  root.innerHTML = rawHtml
  const anchors = Array.from(root.querySelectorAll('a[href]'))
  if (!anchors.length) return rawHtml
  const repoRoot = getNotebookRepoRoot()
  for (const anchor of anchors) {
    try {
      if (!canPromoteNotebookLinkToCard(anchor)) continue
      const href = String(anchor.getAttribute('href') || '').trim()
      const targetPath = await resolveNotebookLinkTargetPath(href, { baseDocPath })
      if (!targetPath) continue
      const meta = await loadNotebookLinkCardMeta(targetPath, { repoRoot })
      if (!meta) continue
      const sourceBlock = anchor.closest('p, li')
      if (!sourceBlock) continue
      const card = document.createElement('div')
      card.className = `guaxi-preview-file-card guaxi-notebook__link-card is-${meta.kind}`
      card.setAttribute('data-guaxi-preview-card', meta.kind === 'directory' ? 'directory-link' : 'file-link')
      card.setAttribute('data-guaxi-preview-card-path', meta.path)
      const cardLink = document.createElement('a')
      cardLink.className = 'guaxi-preview-file-card__link'
      cardLink.setAttribute('href', href)
      cardLink.setAttribute('title', meta.path)
      cardLink.innerHTML = `
        <span class="guaxi-preview-file-card__eyebrow">${meta.kind === 'directory' ? '关联目录' : '关联笔记'}</span>
        <span class="guaxi-preview-file-card__title">${escapeHtmlAttribute(meta.title)}</span>
        <span class="guaxi-preview-file-card__excerpt">${escapeHtmlAttribute(meta.excerpt)}</span>
        <span class="guaxi-preview-file-card__meta">${escapeHtmlAttribute(meta.pathLabel)}</span>
      `
      card.appendChild(cardLink)
      sourceBlock.replaceWith(card)
      logger.info('[NotebookLinkCard] promoted markdown link to card', {
        href,
        targetPath: meta.path,
        kind: meta.kind,
      })
    } catch (error) {
      logger.warn('[NotebookLinkCard] promote link to card failed', {
        error: String(error?.message || error),
      })
    }
  }
  return root.innerHTML
}

// 设计说明：
// 1. Explorer 模式里的 Markdown 图片和主预览一样，可能以“同一个段落里多张图片”的形式出现。
// 2. 旧样式只把单张图片单独居中，没有给图片组容器建立布局，
//    结果就是多图时整体不居中，或者一张一行导致排版松散。
// 3. 这里把“纯图片段落”统一标记为媒体组，再交给样式层做更稳定的居中与间距控制。
const enhanceNotebookHtmlWithMediaGroups = (html) => {
  const rawHtml = String(html || '')
  if (!rawHtml.includes('<img')) return rawHtml

  const root = document.createElement('div')
  root.innerHTML = rawHtml
  const blocks = Array.from(root.querySelectorAll('p'))
  let mediaGroupCount = 0
  let mediaItemCount = 0
  let preservedAuthoredHtmlBlockCount = 0

  const isWhitespaceNode = (node) => {
    if (!node) return true
    if (node.nodeType === Node.TEXT_NODE) return !String(node.textContent || '').trim()
    if (node.nodeType === Node.ELEMENT_NODE && node instanceof HTMLBRElement) return true
    return false
  }

  const shouldPreserveAuthoredHtmlLayout = (block) => {
    if (!(block instanceof HTMLParagraphElement)) return false
    return block.hasAttribute('align') || block.hasAttribute('style')
  }

  const resolveMediaItem = (node) => {
    if (!(node instanceof Element)) return null
    if (node instanceof HTMLImageElement) {
      return {
        host: node,
        image: node,
      }
    }
    if (node instanceof HTMLAnchorElement) {
      const childElements = Array.from(node.children || [])
      if (childElements.length !== 1 || !(childElements[0] instanceof HTMLImageElement)) return null
      const hasNonMediaChild = Array.from(node.childNodes || []).some((child) => {
        if (child === childElements[0]) return false
        return !isWhitespaceNode(child)
      })
      if (hasNonMediaChild) return null
      return {
        host: node,
        image: childElements[0],
      }
    }
    return null
  }

  for (const block of blocks) {
    // 设计说明：
    // 1. Explorer 模式同样会遇到 Markdown 中嵌入的原生 HTML，最常见的布局声明也是 `align` / `style`。
    // 2. 上一版判定条件过宽，导致标准 Markdown 图片段落也被误判。
    // 3. 这里改成只跳过明确声明过布局的 HTML 段落，保证 Markdown 图片语法继续正常增强。
    if (shouldPreserveAuthoredHtmlLayout(block)) {
      preservedAuthoredHtmlBlockCount += 1
      continue
    }

    const meaningfulNodes = Array.from(block.childNodes || []).filter((node) => !isWhitespaceNode(node))
    if (!meaningfulNodes.length) continue

    const mediaItems = meaningfulNodes.map(resolveMediaItem)
    if (mediaItems.some((item) => !item)) continue

    block.classList.add('guaxi-media-group')
    block.classList.toggle('is-single', mediaItems.length === 1)
    block.classList.toggle('is-gallery', mediaItems.length > 1)
    block.setAttribute('data-guaxi-media-count', String(mediaItems.length))
    block.style.setProperty('--guaxi-media-count', String(mediaItems.length))

    for (const item of mediaItems) {
      // 设计说明：
      // 1. 裸 `<img>` 本身不适合作为画廊单元，无法稳定承载留白、阴影和响应式宽度。
      // 2. 这里在渲染后补一个轻量容器，让 Explorer 模式与主预览都能复用同一套媒体项布局策略。
      if (item.host === item.image) {
        const wrapper = document.createElement('span')
        wrapper.setAttribute('data-guaxi-media-item', 'image')
        item.image.replaceWith(wrapper)
        wrapper.appendChild(item.image)
        item.host = wrapper
      } else {
        item.host.setAttribute('data-guaxi-media-item', 'linked-image')
      }
      item.host.classList.add('guaxi-media-group__item')
      item.image.classList.add('guaxi-media-group__image')
    }

    mediaGroupCount += 1
    mediaItemCount += mediaItems.length
  }

  logger.info('[Notebook] enhanced media groups', {
    filePath: props.filePath,
    mediaGroupCount,
    mediaItemCount,
    preservedAuthoredHtmlBlockCount,
  })
  return root.innerHTML
}

const getGuaxiDisplayFromResult = (res) => {
  const value = res?.result_json?.__guaxi_display__
  return value && typeof value === 'object' ? value : null
}

const getGuaxiDataflowFromResult = (res) => {
  const value = res?.result_json?.__guaxi_dataflow__
  return value && typeof value === 'object' ? value : null
}

const normalizeGuaxiTablePayloadFromResult = (value) => {
  if (!value || typeof value !== 'object') return null
  return {
    columns: Array.isArray(value.columns) ? value.columns.map((item) => String(item ?? '')) : [],
    rows: Array.isArray(value.rows) ? value.rows : [],
    rowCount: Number(value.row_count ?? (Array.isArray(value.rows) ? value.rows.length : 0)),
    truncated: !!value.truncated,
    dtypes: value.dtypes && typeof value.dtypes === 'object' ? value.dtypes : {},
    sourcePath: String(value.source_path || '').trim(),
    resultHandle: String(value.result_handle || '').trim(),
    sessionId: String(value.session_id || '').trim(),
    manifestPath: String(value.manifest_path || '').trim(),
    artifactPath: String(value.artifact_path || '').trim(),
    tableKey: String(value.table_key || '').trim(),
    tableName: String(value.table_name || '').trim(),
    tableKind: String(value.table_kind || '').trim(),
    tableCount: Number(value.table_count || 1),
    remote: !!value.remote,
  }
}

const getGuaxiTablesFromResult = (res) => {
  const explicitTables = Array.isArray(res?.result_json?.__guaxi_tables__)
    ? res.result_json.__guaxi_tables__.map((item) => normalizeGuaxiTablePayloadFromResult(item)).filter(Boolean)
    : []
  if (explicitTables.length) return explicitTables

  const fallback = normalizeGuaxiTablePayloadFromResult(res?.result_json?.__guaxi_dataframe__)
  return fallback ? [fallback] : []
}

const getGuaxiDataFrameFromResult = (res) => getGuaxiTablesFromResult(res)[0] || null

const getCellOutputModules = (cell) => {
  const snapshot = getCellSnapshot(cell)
  return Array.isArray(snapshot?.outputs) ? snapshot.outputs : []
}

const getCellOutputModule = (cell, moduleId = '') => {
  const modules = getCellOutputModules(cell)
  const normalizedId = String(moduleId || '').trim()
  if (normalizedId) {
    return modules.find((item) => String(item?.id || '') === normalizedId) || null
  }
  return getLatestPythonCellOutputModule(modules) || null
}

const getLatestTextOutputModule = (cell) => getLatestPythonCellOutputModuleByKind(getCellOutputModules(cell), 'text')

const getNotebookOutputModuleTitle = (outputModule) => String(outputModule?.title || '输出结果')

const getNotebookOutputModuleSummary = (outputModule) => String(outputModule?.summary || '')

const buildOutputModuleMarkdownText = (outputModule) => {
  if (!outputModule) return ''
  if (outputModule.kind === 'table' && outputModule.table) {
    return dataframeToMarkdownTable(outputModule.table)
  }
  if (outputModule.kind === 'markdown') {
    return String(outputModule.display?.markdown || '')
  }
  if (outputModule.kind === 'html') {
    return String(outputModule.display?.html || '')
  }
  if (outputModule.kind === 'svg') {
    return String(outputModule.display?.svg || '')
  }
  if (outputModule.kind === 'text') {
    return String(outputModule.text || '')
  }
  return ''
}

const buildOutputModuleSearchText = (outputModule) => {
  if (!outputModule) return ''
  if (outputModule.kind === 'table') return buildOutputModuleMarkdownText(outputModule)
  if (outputModule.kind === 'markdown') return String(outputModule.display?.markdown || '')
  if (outputModule.kind === 'html') return stripHtmlTags(String(outputModule.display?.html || ''))
  if (outputModule.kind === 'svg') return stripHtmlTags(String(outputModule.display?.svg || ''))
  if (outputModule.kind === 'text') return String(outputModule.text || '')
  return String(outputModule.summary || '')
}

const getCellSnapshot = (cell) => {
  refreshTick.value
  const persistedSnapshot = getPythonCellSnapshot(props.filePath, cell.cellId)
  const snapshot = isPythonCellSnapshotFresh(cell, persistedSnapshot)
    ? persistedSnapshot
    : buildStalePythonCellSnapshot(cell, persistedSnapshot, 'snapshot-read')
  // 设计说明：
  // 1. 旧快照里可能还只有 `outputText/display/dataframe` 三槽结构，这里读取时即时迁移成 `outputs[]`。
  // 2. Notebook 的新渲染层始终围绕模块列表工作，这样导入表格、运行结果、错误输出才能同时保留。
  // 3. 同时继续回填一份“最新文本/最新表格/最新富输出”给旧逻辑复用，降低这次改造对其余功能的侵入面。
  const outputs = normalizePythonCellOutputModules(snapshot?.outputs, {
    outputText: String(snapshot?.outputText || ''),
    display: snapshot?.display || null,
    dataframe: snapshot?.dataframe || null,
    source: 'legacy-snapshot',
    idPrefix: `legacy-${String(cell?.cellId || cell?.key || 'cell')}`,
  })
  const latestTextOutput = getLatestPythonCellOutputModuleByKind(outputs, 'text')
  const latestTableOutput = getLatestPythonCellOutputModuleByKind(outputs, 'table')
  const latestDisplayOutput = outputs.find((item) => isPythonCellDisplayOutputKind(item?.kind))
  return {
    codeDigest: String(snapshot?.codeDigest || getNotebookCellContentDigest(cell)),
    outputText: latestTextOutput?.text || String(snapshot?.outputText || ''),
    display: latestDisplayOutput?.display || snapshot?.display || null,
    dataframe: latestTableOutput?.table || snapshot?.dataframe || null,
    outputs,
    result: snapshot?.result || null,
    sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    lab: snapshot?.lab && typeof snapshot.lab === 'object'
      ? {
        ...snapshot.lab,
        imports: Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
      }
      : {
        imports: [],
      },
    meta: {
      ...(cell.meta || {}),
      ...(snapshot?.meta || {}),
      outputCollapsed: snapshot?.meta?.outputCollapsed ?? cell.meta?.outputCollapsed ?? false,
      sessionCollapsed: snapshot?.meta?.sessionCollapsed ?? cell.meta?.sessionCollapsed ?? true,
    },
  }
}

const getCellLabImports = (cell) => {
  const snapshot = getCellSnapshot(cell)
  return Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : []
}

const patchCellSnapshotLab = (cell, patch = {}) => {
  const snapshot = getCellSnapshot(cell)
  const nextLab = {
    ...(snapshot?.lab && typeof snapshot.lab === 'object' ? snapshot.lab : {}),
    ...(patch && typeof patch === 'object' ? patch : {}),
  }
  if (!Array.isArray(nextLab.imports)) {
    nextLab.imports = []
  }
  return nextLab
}

const updateNotebookCellDataflowSnapshot = (cell, dataflow = null, reason = 'unknown') => {
  if (!cell?.cellId || !dataflow || typeof dataflow !== 'object') return null
  const snapshot = getCellSnapshot(cell)
  const nextLab = patchCellSnapshotLab(cell, {
    dataflow: {
      ...dataflow,
      updatedAt: new Date().toISOString(),
      reason,
    },
  })
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    lab: nextLab,
    meta: {
      ...(snapshot?.meta || {}),
      staleVariables: [],
      staleReason: '',
    },
  })
  logger.info('[NotebookDataflow] cell dataflow snapshot updated', {
    reason,
    cellId: String(cell?.cellId || ''),
    used: Array.isArray(dataflow.used) ? dataflow.used : [],
    touched: Array.isArray(dataflow.touched) ? dataflow.touched : [],
  })
  return nextLab
}

const markDependentNotebookCellsStale = (sourceCell, dataflow = null, reason = 'unknown') => {
  if (!sourceCell?.cellId || !dataflow || typeof dataflow !== 'object') return []
  const touched = new Set(Array.isArray(dataflow.touched) ? dataflow.touched.map((item) => String(item || '').trim()).filter(Boolean) : [])
  if (!touched.size) return []

  const sourceIndex = notebookCells.value.findIndex((item) => String(item?.cellId || '') === String(sourceCell.cellId || ''))
  const affected = []
  notebookCells.value.forEach((cell, index) => {
    if (cell?.type !== 'python') return
    if (sourceIndex >= 0 && index <= sourceIndex) return
    const snapshot = getCellSnapshot(cell)
    const used = Array.isArray(snapshot?.lab?.dataflow?.used) ? snapshot.lab.dataflow.used : []
    const staleVariables = used.map((item) => String(item || '').trim()).filter((item) => touched.has(item))
    if (!staleVariables.length) return
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      meta: {
        ...(snapshot?.meta || {}),
        staleVariables,
        staleReason: `上游 Cell 更新了 ${staleVariables.join(', ')}`,
      },
    })
    affected.push({
      cellId: String(cell.cellId || ''),
      cellKey: String(cell.key || ''),
      staleVariables,
    })
  })

  if (affected.length) {
    logger.info('[NotebookDataflow] downstream cells marked stale', {
      reason,
      sourceCellId: String(sourceCell?.cellId || ''),
      touchedVariables: Array.from(touched),
      affectedCellCount: affected.length,
      affectedCellIds: affected.map((item) => item.cellId),
    })
  }
  return affected
}

const mergeNotebookSessionVarSummaries = (currentItems = [], updatedItems = []) => {
  const map = new Map()

  // 设计说明：
  // 1. 数据实验室导入现在走“仅回传目标变量摘要”的快路径，不能再用返回值直接覆盖整份会话变量列表。
  // 2. 这里把已有快照与增量更新按变量名合并，既保留旧变量，又让新导入的数据集可以立即出现在右侧实验室面板里。
  // 3. 统一在前端按名称排序，保证侧栏顺序稳定，避免用户误以为变量丢失或重复。
  ;(Array.isArray(currentItems) ? currentItems : []).forEach((item) => {
    const name = String(item?.name || '').trim()
    if (!name) return
    map.set(name, {
      ...item,
      name,
    })
  })

  ;(Array.isArray(updatedItems) ? updatedItems : []).forEach((item) => {
    const name = String(item?.name || '').trim()
    if (!name) return
    map.set(name, {
      ...(map.get(name) || {}),
      ...item,
      name,
    })
  })

  return Array.from(map.values()).sort((left, right) => {
    const leftName = String(left?.name || '')
    const rightName = String(right?.name || '')
    return leftName.localeCompare(rightName, 'zh-Hans-CN')
  })
}

// 设计说明：
// 1. Python worker 的正常执行链路应该返回 `session_vars`，但历史兼容路径或异常恢复路径可能缺失这个字段。
// 2. 旧逻辑一旦遇到“字段缺失”就把前端快照清空，右侧实验室会立刻显示成 0 个变量，形成“变量明明还能 print 却面板为空”的错觉。
// 3. 这里改成“字段存在就覆盖，字段缺失就保留上一次快照”，只在后端明确返回数组时才认定为最新会话变量列表。
const resolveNotebookSessionVarsFromExecResult = (result, previousSessionVars = []) => {
  if (Array.isArray(result?.session_vars)) {
    return result.session_vars
  }
  logger.warn('[Notebook] python execution response missing session_vars, keep previous snapshot variables', {
    hasResult: !!result,
    resultKeys: result && typeof result === 'object' ? Object.keys(result) : [],
    previousSessionVarCount: Array.isArray(previousSessionVars) ? previousSessionVars.length : 0,
  })
  return Array.isArray(previousSessionVars) ? previousSessionVars : []
}

const buildNotebookPythonSessionInspectorPayload = (cell) => {
  if (cell?.type !== 'python') {
    return {
      filePath: String(props.filePath || ''),
      cellId: '',
      cellKey: '',
      stableId: '',
      cellTitle: '',
      sessionId: createPythonSessionId(props.filePath),
      hasActiveCell: false,
      datasets: [],
      variables: [],
    }
  }

  const snapshot = getCellSnapshot(cell)
  return {
    filePath: String(props.filePath || ''),
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    stableId: String(cell?.meta?.id || ''),
    cellTitle: String(cell?.meta?.title || ''),
    sessionId: createPythonSessionId(props.filePath),
    hasActiveCell: true,
    datasets: buildNotebookLabDatasetDescriptors(
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
    ),
    variables: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
  }
}

const syncNotebookPythonSessionInspector = (cell, reason = 'unknown') => {
  const payload = buildNotebookPythonSessionInspectorPayload(cell)
  rightSiderStore.setPythonSessionInspector(payload)
  logger.debug('[Notebook] sync python session inspector payload', {
    reason,
    cellId: payload.cellId,
    hasActiveCell: payload.hasActiveCell,
    variableCount: payload.variables.length,
  })
  return payload
}

watch(
  [
    () => activeNotebookPythonCellKey.value,
    () => refreshTick.value,
    () => props.filePath,
  ],
  () => {
    const targetCell = activeNotebookPythonCell.value
    if (targetCell?.type === 'python') {
      syncNotebookPythonSessionInspector(targetCell, 'active-cell-or-refresh-watch')
      return
    }
    rightSiderStore.clearPythonSessionInspector()
    logger.debug('[Notebook] cleared python session inspector from watch because no active python cell exists')
  },
  { immediate: true },
)

const isActiveNotebookPythonCell = (cell) => {
  if (cell?.type !== 'python') return false
  return String(cell?.key || '') === String(activeNotebookPythonCellKey.value || '')
}

const buildCellOutputModuleCacheKey = (cell, outputModule) => [
  String(cell?.key || ''),
  String(outputModule?.id || ''),
].join('::')

const getCellDisplayKind = (cell, outputModule = null) => {
  const targetModule = outputModule || getCellOutputModule(cell)
  return String(targetModule?.kind || '').trim()
}

const getCellDisplayUrl = (cell, outputModule = null) => {
  const targetModule = outputModule || getCellOutputModule(cell)
  if (!targetModule) return ''
  return String(resolvedDisplayUrlMap.value[buildCellOutputModuleCacheKey(cell, targetModule)] || '')
}

const buildDisplayCacheKey = (display) => {
  const kind = normalizePythonCellOutputDisplayType(display)
  const source = getPythonCellOutputDisplaySourcePath(display)
  const dataUrl = getPythonCellOutputDisplayDataUrl(display)
  const mime = String(display?.mime || '')
  return [
    kind,
    mime,
    source,
    dataUrl ? computeNotebookDigest(dataUrl) : '',
  ].join('|')
}

const resolveDisplayUrl = async (display) => {
  const dataUrl = getPythonCellOutputDisplayDataUrl(display)
  if (dataUrl) return dataUrl

  const source = getPythonCellOutputDisplaySourcePath(display)
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
    for (const outputModule of getCellOutputModules(cell)) {
      if (!['image', 'audio', 'video'].includes(String(outputModule?.kind || ''))) continue
      const display = outputModule?.display || null
      const mapKey = buildCellOutputModuleCacheKey(cell, outputModule)
      const cacheKey = buildDisplayCacheKey(display)
      if (cacheKey && notebookDisplayUrlCache.has(cacheKey)) {
        nextMap[mapKey] = notebookDisplayUrlCache.get(cacheKey)
        cacheHitCount += 1
        continue
      }
      if (cacheKey && previousMap[mapKey]) {
        nextMap[mapKey] = previousMap[mapKey]
        notebookDisplayUrlCache.set(cacheKey, previousMap[mapKey])
        cacheHitCount += 1
        continue
      }
      tasks.push((async () => {
        const resolvedUrl = await resolveDisplayUrl(display)
        nextMap[mapKey] = resolvedUrl
        if (cacheKey) {
          notebookDisplayUrlCache.set(cacheKey, resolvedUrl)
        }
        resolvedCount += 1
      })())
    }
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
  return getCellOutputModules(cell).length > 0
}

const renderMarkdownHtml = async (content, reason = 'unknown') => {
  const markdownText = preprocessNotebookObsidianWikilinks(String(content || ''))
  if (!markdownText.trim()) return ''
  const cacheKey = buildNotebookMarkdownCacheKey(markdownText, {
    theme: props.theme,
    filePath: props.filePath,
    basePath: getNotebookBasePath(),
  })
  if (notebookMarkdownRenderCache.has(cacheKey)) {
    logger.info('[Notebook] markdown render cache hit', {
      reason,
      cacheKey,
      filePath: props.filePath,
    })
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
    html = await enhanceNotebookHtmlWithLinkCards(html, {
      baseDocPath: props.filePath,
    })
    html = enhanceNotebookHtmlWithMediaGroups(html)
    notebookMarkdownRenderCache.set(cacheKey, html)
    logger.info('[Notebook] markdown render cache store', {
      reason,
      cacheKey,
      filePath: props.filePath,
    })
    return html
  } catch (error) {
    logger.warn('[Notebook] worker markdown render failed, fallback to main thread renderer', {
      reason,
      error: String(error?.message || error),
    })
    try {
      let html = await renderMarkdown(markdownText, buildMarkdownRenderOptions())
      html = await enhanceNotebookHtmlWithLinkCards(html, {
        baseDocPath: props.filePath,
      })
      html = enhanceNotebookHtmlWithMediaGroups(html)
      notebookMarkdownRenderCache.set(cacheKey, html)
      logger.info('[Notebook] markdown render cache store after fallback', {
        reason,
        cacheKey,
        filePath: props.filePath,
      })
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
      const cacheKey = buildNotebookMarkdownCacheKey(markdownText, {
        theme: props.theme,
        filePath: props.filePath,
        basePath: getNotebookBasePath(),
      })
      if (hasNotebookTocMarker(markdownText)) {
        renderTasks.push((async () => {
          const renderedHtml = await renderNotebookMarkdownHtmlWithSyntheticToc(
            markdownText,
            cellKey,
            `markdown-cell:${cellKey}`,
          )
          nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(renderedHtml, cellKey)
          markdownRenderCount += 1
        })())
        continue
      }
      const cachedMarkdownHtml = notebookMarkdownRenderCache.get(cacheKey)
      if (typeof cachedMarkdownHtml === 'string' && cachedMarkdownHtml) {
        // 设计说明：
        // 1. 旧实现只要“当前 cellKey 在上一次渲染 map 中存在”且“cacheKey 已命中”，
        //    就直接复用 previousMarkdownMap[cellKey]。
        // 2. 但切换不同文件时，cellKey 很容易重复（例如都叫 markdown-1），
        //    这样会把“上一份文件”的 HTML 错误复用到“当前文件”上。
        // 3. 这里改成严格按本次 cacheKey 读取缓存内容，确保命中的永远是当前文件对应的渲染结果。
        nextMarkdownMap[cellKey] = decorateNotebookHeadingAnchors(cachedMarkdownHtml, cellKey)
        markdownCacheHitCount += 1
        logger.info('[Notebook] reuse cached markdown html for cell', {
          reason,
          cellKey,
          cacheKey,
          filePath: props.filePath,
        })
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
      for (const outputModule of getCellOutputModules(cell)) {
        if (outputModule.kind !== 'markdown') continue
        const outputMarkdown = String(outputModule.display?.markdown || '')
        if (!outputMarkdown.trim()) continue
        const cellKey = String(cell.key || '')
        const outputMapKey = buildCellOutputModuleCacheKey(cell, outputModule)
        const cacheKey = buildNotebookMarkdownCacheKey(outputMarkdown, {
          theme: props.theme,
          filePath: props.filePath,
          basePath: getNotebookBasePath(),
        })
        const cachedOutputHtml = notebookMarkdownRenderCache.get(cacheKey)
        if (typeof cachedOutputHtml === 'string' && cachedOutputHtml) {
          nextOutputMap[outputMapKey] = cachedOutputHtml
          outputCacheHitCount += 1
          logger.info('[Notebook] reuse cached markdown output html for python cell module', {
            reason,
            cellKey,
            outputModuleId: String(outputModule.id || ''),
            cacheKey,
            filePath: props.filePath,
          })
          continue
        }

        renderTasks.push((async () => {
          nextOutputMap[outputMapKey] = await renderMarkdownHtml(
            outputMarkdown,
            `python-output:${cellKey}:${String(outputModule.id || '')}`,
          )
          outputRenderCount += 1
        })())
      }
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

const getRenderedOutputMarkdownHtml = (cell, outputModule = null) => {
  const targetModule = outputModule || getCellOutputModule(cell)
  if (!targetModule) return ''
  return String(renderedOutputMarkdownHtmlMap.value[buildCellOutputModuleCacheKey(cell, targetModule)] || '')
}

const handleNotebookSelectionMouseUp = (event) => {
  const target = event?.target instanceof Element ? event.target : null
  if (target?.closest(NOTEBOOK_SELECTION_IGNORE_SELECTOR)) return

  const root = notebookRootRef.value
  if (!(root instanceof HTMLElement)) return

  const { text, surface } = getNotebookSelectionTextFromWindow(target)
  if (!text || !(surface instanceof HTMLElement)) {
    clearNotebookSelectionState('mouseup-empty-selection')
    return
  }

  const rootRect = root.getBoundingClientRect()
  const x = Number(event?.clientX || rootRect.left)
  const y = Number(event?.clientY || rootRect.top)
  const toolbarWidth = 196
  const toolbarHeight = 38
  const pad = 8
  const gap = 10

  let left = x - rootRect.left
  let top = y - rootRect.top - toolbarHeight - gap
  if (top < pad) {
    top = y - rootRect.top + gap
  }

  const maxLeft = Math.max(pad, rootRect.width - toolbarWidth - pad)
  const maxTop = Math.max(pad, rootRect.height - toolbarHeight - pad)

  notebookSelectionText.value = text
  notebookSelectionToolbarStyle.value = {
    left: `${Math.min(Math.max(pad, left), maxLeft)}px`,
    top: `${Math.min(Math.max(pad, top), maxTop)}px`,
  }
  notebookSelectionToolbarVisible.value = true

  logger.info('[NotebookSelection] show toolbar from rendered selection', {
    textLength: text.length,
    tagName: surface.tagName,
  })
}

const handleNotebookGlobalSelectionChange = () => {
  if (!notebookSelectionToolbarVisible.value) return

  const { text } = getNotebookSelectionTextFromWindow()
  if (!text) {
    clearNotebookSelectionState('global-selection-change-empty')
    return
  }

  notebookSelectionText.value = text
}

const handleNotebookGlobalMouseDown = (event) => {
  if (!notebookSelectionToolbarVisible.value) return

  const target = event?.target instanceof Element ? event.target : null
  if (!target) return
  if (target.closest('.guaxi-notebook__selection-toolbar')) return
  if (target.closest('.pot-selection-translate-panel')) return
  if (target.closest(NOTEBOOK_SELECTION_SURFACE_SELECTOR)) return

  clearNotebookSelectionState('global-mousedown-outside')
}

const handleNotebookSelectionTranslatePanelUpdatePos = (pos) => {
  if (!pos) return

  const left = Number(pos.left)
  const top = Number(pos.top)
  if (!Number.isFinite(left) || !Number.isFinite(top)) return

  const current = selectionTranslatePanelPos.value
  if (Number(current.left) === left && Number(current.top) === top) return

  selectionTranslatePanelPos.value = { left, top }
  logger.info('[NotebookSelection] translate panel moved', { left, top })
}

const toggleNotebookSelectionTranslatePinned = () => {
  selectionTranslatePinned.value = !selectionTranslatePinned.value
  logger.info('[NotebookSelection] translate panel pin toggled', {
    pinned: selectionTranslatePinned.value,
  })
}

const swapNotebookSelectionTranslateLangs = () => {
  const from = selectionTranslateFrom.value
  selectionTranslateFrom.value = selectionTranslateTo.value
  selectionTranslateTo.value = from
  logger.info('[NotebookSelection] swap translate languages', {
    from: selectionTranslateFrom.value,
    to: selectionTranslateTo.value,
  })

  if (selectionTranslatePanelVisible.value && !selectionTranslateBusy.value) {
    translateNotebookSelectionText(selectionTranslateRaw.value)
  }
}

const closeNotebookSelectionTranslatePanel = () => {
  selectionTranslatePanelVisible.value = false
  selectionTranslatePinned.value = false
  selectionTranslateWordMeta.value = null
  logger.info('[NotebookSelection] close translate panel')
}

const openNotebookSelectionTranslatePanel = async () => {
  const text = getNotebookSelectionTextForLanguageTasks()
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }

  // 设计说明：
  // 1. 翻译面板继续定位在 Notebook 根容器内部，和 Explorer 的滚动容器保持同一坐标系。
  // 2. 先复用划词工具条的锚点，再做一次边界钳制，能避免长文档滚动后面板跑出可视区。
  const root = notebookRootRef.value
  const toolbarStyle = notebookSelectionToolbarStyle.value
  const left = Number.parseFloat(String(toolbarStyle.left || ''))
  const top = Number.parseFloat(String(toolbarStyle.top || ''))
  const width = root?.clientWidth ?? window.innerWidth
  const height = root?.clientHeight ?? window.innerHeight
  const panelWidth = 360
  const panelHeight = 260
  const pad = 8

  selectionTranslatePanelPos.value = {
    left: Math.min(Math.max(pad, Number.isFinite(left) ? left : pad), Math.max(pad, width - panelWidth - pad)),
    top: Math.min(Math.max(pad, (Number.isFinite(top) ? top : pad) + 44), Math.max(pad, height - panelHeight - pad)),
  }
  selectionTranslatePanelVisible.value = true
  hideNotebookSelectionToolbar()

  logger.info('[NotebookSelection] open translate panel', {
    textLength: text.length,
    left: selectionTranslatePanelPos.value.left,
    top: selectionTranslatePanelPos.value.top,
  })

  await translateNotebookSelectionText(text)
}

const sendNotebookSelectionToAssistant = (overrideText, options = {}) => {
  const text = String(overrideText ?? getNotebookSelectionTextForLanguageTasks()).trim()
  if (!text) {
    message.warning('请先框选要发送到助手的文本')
    return false
  }

  const autoSend = options.autoSend !== false
  const disableWebSearch = !!options.disableWebSearch
  rightSiderStore.setPanel('assistant')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed('home', false)
  rightSiderStore.appendAssistantDraft(text, { autoSend, disableWebSearch })
  message.success(options.successMessage || (autoSend ? '已发送到助手并开始提问' : '已插入到助手输入框'))

  logger.info('[NotebookSelection] send selection to assistant', {
    textLength: text.length,
    autoSend,
    disableWebSearch,
  })
  return true
}

const askNotebookSelectionWithAssistant = () => {
  sendNotebookSelectionToAssistant(undefined, {
    autoSend: true,
    successMessage: '已发送到助手并开始提问',
  })
}

const sendNotebookSelectionAsAiTranslate = () => {
  const text = getNotebookSelectionTextForLanguageTasks()
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }

  const targetLang = String(selectionTranslateTo.value || 'zh')
  const normalizedTargetLang = targetLang === 'auto' ? 'zh' : targetLang
  const targetLangLabel = getSelectionTranslateLangLabel(normalizedTargetLang)
  const prompt = `请将以下内容翻译成${targetLangLabel}，保持原意，并保留术语、代码、链接与原有格式，只输出译文：\n\n${text}`

  sendNotebookSelectionToAssistant(prompt, {
    autoSend: true,
    disableWebSearch: true,
    successMessage: `已发送到助手并翻译为${targetLangLabel}`,
  })
}

const readNotebookSelectionAloud = async () => {
  if (selectionReadAloudBusy.value) return

  const text = getNotebookSelectionTextForLanguageTasks()
  if (!text) {
    message.warning('请先框选要朗读的文本')
    return
  }

  selectionReadAloudBusy.value = true
  try {
    const tts = await invoke('get_tts_config')
    const provider = String(tts?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    const model = String(tts?.model || '').trim() || (provider === 'sherpa-onnx'
      ? 'utils/Models/tts/kokoro_multi_lang/model.onnx'
      : 'FunAudioLLM/CosyVoice2-0.5B')
    const tokens = String(tts?.tokens || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(tts?.exe || '').trim() : ''
    const voice = provider === 'sherpa-onnx' ? '0' : 'alex'
    const responseFormat = provider === 'sherpa-onnx' ? 'wav' : 'mp3'

    logger.info('[NotebookSelection] start text to speech', {
      provider,
      model,
      responseFormat,
      textLength: text.length,
    })

    const response = await invoke('audio_text_to_speech', {
      request: {
        common: {
          provider,
          model,
          endpoint,
          tokens,
        },
        input: text,
        voice,
        response_format: responseFormat,
        stream: false,
      },
    })

    const audioBase64 = String(response?.audio_base64 || '')
    if (!audioBase64) {
      throw new Error('语音数据为空')
    }

    const contentType = String(response?.content_type || 'audio/mpeg')
    const blob = new Blob([base64ToUint8Array(audioBase64)], { type: contentType })
    stopSelectionAudioPlayback()
    selectionAudioObjectUrl = URL.createObjectURL(blob)

    const audio = new Audio(selectionAudioObjectUrl)
    selectionAudioElement = audio
    const waitForPlaybackEnd = new Promise((resolve, reject) => {
      audio.onended = () => {
        stopSelectionAudioPlayback()
        resolve()
      }
      audio.onerror = () => {
        stopSelectionAudioPlayback()
        reject(new Error('音频播放失败'))
      }
    })

    await audio.play()
    await waitForPlaybackEnd
  } catch (error) {
    logger.error('[NotebookSelection] read aloud failed', { error })
    message.error(`朗读失败：${String(error?.message || error || '')}`)
  } finally {
    selectionReadAloudBusy.value = false
  }
}

const translateNotebookSelectionText = async (overrideText) => {
  const text = normalizeSelectionTextForLanguageTasks(overrideText ?? getNotebookSelectionTextForLanguageTasks())
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }
  if (selectionTranslateBusy.value) return

  selectionTranslateRaw.value = text
  selectionTranslateResult.value = ''
  selectionTranslateError.value = ''
  selectionTranslateItems.value = []
  selectionTranslateWordMeta.value = null
  selectionTranslateBusy.value = true

  logger.info('[NotebookSelection] start translate', {
    textLength: text.length,
    from: selectionTranslateFrom.value,
    to: selectionTranslateTo.value,
  })

  try {
    let hasUsableMultiResult = false
    if (selectionTranslateMultiAvailable.value) {
      try {
        const response = await invoke('selection_translate_free_multi', {
          text,
          from: selectionTranslateFrom.value,
          to: selectionTranslateTo.value,
        })

        const items = Array.isArray(response?.items) ? response.items : []
        selectionTranslateItems.value = items.map((item) => ({
          engine: String(item?.engine || ''),
          success: Boolean(item?.success),
          text: String(item?.text || ''),
          error: item?.error ? String(item.error) : '',
        }))

        const primary = selectionTranslateItems.value.find((item) => item.success && String(item.text || '').trim())
        if (primary) {
          selectionTranslateResult.value = String(primary.text || '')
          hasUsableMultiResult = true
          logger.info('[NotebookSelection] multi translate success', {
            engines: selectionTranslateItems.value.length,
          })
        } else {
          throw new Error(String(response?.error || '翻译结果为空'))
        }
      } catch (error) {
        const messageText = String(error?.message || error || '')
        if (messageText.includes('selection_translate_free_multi') && messageText.toLowerCase().includes('not found')) {
          selectionTranslateMultiAvailable.value = false
          if (!selectionTranslateMultiNotFoundWarned) {
            selectionTranslateMultiNotFoundWarned = true
            logger.warn('[NotebookSelection] multi translate command not found, fallback to single path')
          }
        } else {
          logger.warn('[NotebookSelection] multi translate failed, fallback to single path', {
            error,
          })
        }
      }
    }

    if (!hasUsableMultiResult) {
      const response = await invoke('translate_text_free', {
        texts: [text],
        from: selectionTranslateFrom.value,
        to: selectionTranslateTo.value,
      })
      const translated = String(response?.translated_texts?.[0] || '')
      if (!translated.trim() || translated.includes('[Translation Failed]')) {
        throw new Error(String(response?.error || '翻译结果为空'))
      }

      selectionTranslateItems.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
      selectionTranslateResult.value = translated
      logger.info('[NotebookSelection] single translate success', {
        translatedLength: translated.length,
      })
    }

    if (isWordSelection(text)) {
      try {
        const basic = await invoke('language_learning_enrich_word_basic', {
          word: text,
          sourceLang: selectionTranslateFrom.value,
          targetLang: selectionTranslateTo.value,
        })
        selectionTranslateWordMeta.value = {
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
          examples: [],
          example_translations: [],
        }

        const details = await invoke('language_learning_enrich_word_details', {
          word: text,
          sourceLang: selectionTranslateFrom.value,
          targetLang: selectionTranslateTo.value,
        })
        selectionTranslateWordMeta.value = {
          ...selectionTranslateWordMeta.value,
          synonyms: Array.isArray(details?.synonyms) ? details.synonyms : [],
          antonyms: Array.isArray(details?.antonyms) ? details.antonyms : [],
          associates: Array.isArray(details?.associates) ? details.associates : [],
          synonyms_translations: Array.isArray(details?.synonyms_translations) ? details.synonyms_translations : [],
          antonyms_translations: Array.isArray(details?.antonyms_translations) ? details.antonyms_translations : [],
          associates_translations: Array.isArray(details?.associates_translations) ? details.associates_translations : [],
          phrases: Array.isArray(details?.phrases) ? details.phrases : [],
          phrase_translations: Array.isArray(details?.phrase_translations) ? details.phrase_translations : [],
          examples: Array.isArray(details?.examples) ? details.examples : [],
          example_translations: Array.isArray(details?.example_translations) ? details.example_translations : [],
        }
        logger.info('[NotebookSelection] enriched word translation metadata', {
          word: text,
        })
      } catch (error) {
        logger.warn('[NotebookSelection] enrich word failed', { error, word: text })
      }
    }

    const finalTranslated = String(selectionTranslateResult.value || '').trim()
    if (finalTranslated) {
      try {
        const primaryItem = selectionTranslateItems.value.find((item) => item.success && String(item.text || '').trim())
        await invoke('language_learning_record_translation', {
          input: {
            source_text: text,
            translated_text: finalTranslated,
            source_lang: selectionTranslateFrom.value,
            target_lang: selectionTranslateTo.value,
            engine: String(primaryItem?.engine || 'Free'),
            record_type: isWordSelection(text) ? 'word' : undefined,
            word_meta: selectionTranslateWordMeta.value || undefined,
          },
        })
      } catch (error) {
        logger.warn('[NotebookSelection] record translation failed', { error })
      }
    }
  } catch (error) {
    const messageText = String(error?.message || error || '翻译失败')
    selectionTranslateError.value = messageText
    selectionTranslateItems.value = [{ engine: 'Free', success: false, text: '', error: messageText }]
    logger.error('[NotebookSelection] translate failed', { error })
    message.error(`翻译失败：${messageText}`)
  } finally {
    selectionTranslateBusy.value = false
  }
}

const copyNotebookSelectionTranslateResult = async (payload) => {
  const text = String((typeof payload === 'string' ? payload : payload?.text) || selectionTranslateResult.value || '')
  if (!text) return

  const ok = await writeClipboardText(text)
  if (ok) {
    message.success('已复制译文', { duration: 1200 })
    logger.info('[NotebookSelection] copied translated text', {
      textLength: text.length,
    })
    return
  }

  message.warning('复制失败：无法访问剪贴板')
}

const handleNotebookRenderedClick = async (event) => {
  const target = event?.target
  const anchor = target instanceof Element ? target.closest('a[href]') : null
  if (!(anchor instanceof HTMLAnchorElement)) return

  const href = String(anchor.getAttribute('href') || '').trim()
  if (!href) return
  if (href.startsWith('#')) {
    event.preventDefault()
    event.stopPropagation()

    const outlineId = decodeURIComponent(href.slice(1))
    const root = notebookRootRef.value
    if (!(root instanceof HTMLElement) || !outlineId) return

    const escapedId = typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
      ? CSS.escape(outlineId)
      : outlineId.replace(/["\\]/g, '\\$&')
    const heading = findNotebookHeadingTargetByOutlineId(outlineId) || root.querySelector(`#${escapedId}`)
    if (heading instanceof HTMLElement) {
      heading.scrollIntoView({ behavior: 'smooth', block: 'center' })
      logger.info('[Notebook] toc anchor navigated', {
        outlineId,
        filePath: props.filePath,
      })
    }
    return
  }
  if (isNotebookHttpUrl(href)) return

  // 设计说明：
  // 1. `guaxi://wikilink` 是应用内协议，浏览器层并没有注册 handler。
  // 2. 如果等到异步解析路径之后再 preventDefault，浏览器会先尝试打开该 scheme 并报错。
  // 3. 因此这里对所有 notebook 内部链接都提前拦截，再交给本地文件跳转逻辑处理。
  event.preventDefault()
  event.stopPropagation()
  logger.info('[NotebookLinkCard] intercept rendered notebook link before browser navigation', {
    href,
    filePath: props.filePath,
  })

  const targetPath = await resolveNotebookLinkTargetPath(href, {
    baseDocPath: props.filePath,
  })
  if (!targetPath) return

  try {
    const meta = await invoke('get_file_meta', { path: targetPath })

    if (meta?.is_directory) {
      await invoke('open_in_file_explorer', { path: targetPath })
      logger.info('[NotebookLinkCard] open directory from rendered notebook link', {
        href,
        path: targetPath,
      })
      return
    }

    if (isNotebookMarkdownFilePath(targetPath)) {
      await fileTreeStore.openFileInTab(targetPath)
      logger.info('[NotebookLinkCard] open markdown file from rendered notebook link', {
        href,
        path: targetPath,
      })
      return
    }

    await invoke('open_in_file_explorer', { path: targetPath })
    logger.info('[NotebookLinkCard] reveal file from rendered notebook link', {
      href,
      path: targetPath,
    })
  } catch (error) {
    logger.warn('[NotebookLinkCard] handle rendered link click failed', {
      href,
      path: targetPath,
      error: String(error?.message || error),
    })
  }
}

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

const toggleCellOutputModule = (cell, moduleId) => {
  const snapshot = getCellSnapshot(cell)
  const nextOutputs = togglePythonCellOutputModuleCollapsed(snapshot?.outputs || [], moduleId)
  patchPythonCellSnapshot(props.filePath, cell.cellId, {
    outputs: nextOutputs,
  })
  logger.info('[Notebook] toggle output module panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    outputModuleId: String(moduleId || ''),
    outputModuleCount: nextOutputs.length,
  })
  bumpRefresh()
  schedulePersistNotebookSidecar('toggle-output-module')
}

// Design notes:
// 1. 数据实验室现在挂在共享右侧栏里，而不是 Python Cell 下方的内联变量卡片。
// 2. 这里仍然把开关状态镜像回 cell meta，保证 notebook sidecar 能稳定记录“该 cell 的实验室面板是否展开”。
const openNotebookPythonSessionPanel = (cell, reason = 'unknown') => {
  const payload = syncNotebookPythonSessionInspector(cell, `open-session-panel:${reason}`)
  rightSiderStore.setPanel('python-session')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(NOTEBOOK_RIGHT_SIDER_VIEW_KEY, false)
  patchCellMeta(cell, {
    sessionCollapsed: false,
  })
  logger.info('[Notebook] open data lab panel', {
    reason,
    cellId: payload.cellId,
    datasetCount: Array.isArray(payload.datasets) ? payload.datasets.length : 0,
    variableCount: payload.variables.length,
  })
}

const closeNotebookPythonSessionPanel = (cell, reason = 'unknown') => {
  if (rightSiderStore.panel === 'python-session') {
    rightSiderStore.close()
    siderStore.setRightSiderCollapsed(NOTEBOOK_RIGHT_SIDER_VIEW_KEY, true)
  }
  if (cell?.type === 'python') {
    patchCellMeta(cell, {
      sessionCollapsed: true,
    })
  }
  logger.info('[Notebook] close data lab panel', {
    reason,
    cellId: String(cell?.cellId || ''),
  })
}

const toggleCellSession = (cell) => {
  const nextVisible = !isNotebookPythonSessionPanelVisible.value
  logger.info('[Notebook] toggle data lab panel', {
    key: String(cell?.key || ''),
    cellId: String(cell?.cellId || ''),
    nextVisible,
  })
  if (nextVisible) {
    openNotebookPythonSessionPanel(cell, 'toolbar-toggle')
    return
  }
  closeNotebookPythonSessionPanel(cell, 'toolbar-toggle')
}

const NOTEBOOK_PYTHON_SESSION_BUSY_PREFIX = 'python-session::'
const createNotebookActionRequestId = (prefix = 'nb-task') => [
  prefix,
  Date.now().toString(36),
  Math.random().toString(36).slice(2, 10),
].join('-')
const buildNotebookPythonSessionBusyKey = (filePath = props.filePath) => {
  const sessionId = String(createPythonSessionId(filePath) || '').trim()
  if (!sessionId) return ''
  return `${NOTEBOOK_PYTHON_SESSION_BUSY_PREFIX}${sessionId}`
}
const getNotebookPythonCellIdentity = (cell) => ({
  cellId: String(cell?.cellId || '').trim(),
  cellKey: String(cell?.key || '').trim(),
  stableId: getNotebookCellStableId(cell),
})
const resolveNotebookPythonCellByIdentity = (identity = {}) => {
  const targetCellId = String(identity?.cellId || '').trim()
  const targetCellKey = String(identity?.cellKey || '').trim()
  const targetStableId = String(identity?.stableId || '').trim()
  return notebookCells.value.find((entry) => {
    if (entry?.type !== 'python') return false
    if (targetCellId && String(entry?.cellId || '').trim() === targetCellId) return true
    if (targetCellKey && String(entry?.key || '').trim() === targetCellKey) return true
    return !!targetStableId && getNotebookCellStableId(entry) === targetStableId
  }) || null
}
const getNotebookBusyKeys = (cell) => {
  const cellKey = String(cell?.key || '').trim()
  return cellKey ? [cellKey] : []
}
const getNotebookPythonSessionBusyKeys = (cell) => {
  if (cell?.type !== 'python') return []
  const sessionBusyKey = buildNotebookPythonSessionBusyKey(props.filePath)
  return sessionBusyKey ? [sessionBusyKey] : []
}
// 设计说明：
// 1. 同一份 Notebook 的 Python Cell 共享会话，所以“导入表格/运行代码”的后端占用不能只挂在单个 cell 上。
// 2. 这里同时写入 cell 级 busy 标记和 session 级 busy 标记，这样用户切换到别的 Python Cell 时，UI 仍然知道共享会话正在忙。
// 3. 这样既能避免误判空闲导致的串行请求叠加，也方便排查“切换选中 cell 后结果串写”的问题。
const setNotebookBusyState = (cell, nextBusy, reason = 'unknown') => {
  const busyKeys = [
    ...getNotebookBusyKeys(cell),
    ...getNotebookPythonSessionBusyKeys(cell),
  ]
  if (!busyKeys.length) return
  const nextMap = {
    ...busyMap.value,
  }
  busyKeys.forEach((key) => {
    nextMap[key] = !!nextBusy
  })
  busyMap.value = nextMap
  logger.debug('[Notebook] updated busy state', {
    reason,
    nextBusy: !!nextBusy,
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    busyKeys,
  })
}
const isCellBusy = (cell) => getNotebookBusyKeys(cell).some((key) => !!busyMap.value[key])
const isNotebookPythonSessionBusy = (cell) => getNotebookPythonSessionBusyKeys(cell).some((key) => !!busyMap.value[key])
const isActiveNotebookPythonSessionBusy = computed(() => isNotebookPythonSessionBusy(activeNotebookPythonCell.value))
const queueNotebookPythonAction = (cell, action, reason = 'unknown') => {
  const targetIdentity = getNotebookPythonCellIdentity(cell)
  const queuedRequestId = createNotebookActionRequestId('nb-queued-python')
  notebookQueuedPythonAction.value = {
    requestId: queuedRequestId,
    action: String(action || ''),
    targetIdentity,
    queuedAt: Date.now(),
    reason: String(reason || 'unknown'),
  }
  logger.info('[Notebook] queued python action', {
    queuedRequestId,
    action: String(action || ''),
    reason: String(reason || 'unknown'),
    cellId: String(cell?.cellId || ''),
    cellKey: String(cell?.key || ''),
    targetCellId: targetIdentity.cellId,
    targetCellKey: targetIdentity.cellKey,
    activeImportRequestId: String(notebookPendingPythonImport.value?.requestId || ''),
  })
}

const flushQueuedNotebookPythonActionIfIdle = async (reason = 'unknown') => {
  const queuedAction = notebookQueuedPythonAction.value
  if (!queuedAction?.action) return
  const targetCell = resolveNotebookPythonCellByIdentity(queuedAction.targetIdentity)
  if (!targetCell) {
    logger.warn('[Notebook] drop queued python action because target cell no longer exists', {
      queuedRequestId: String(queuedAction?.requestId || ''),
      action: String(queuedAction?.action || ''),
      reason: String(reason || 'unknown'),
      targetCellId: String(queuedAction?.targetIdentity?.cellId || ''),
      targetCellKey: String(queuedAction?.targetIdentity?.cellKey || ''),
    })
    notebookQueuedPythonAction.value = null
    return
  }
  if (isNotebookPythonSessionBusy(targetCell)) {
    logger.debug('[Notebook] queued python action still waiting for idle session', {
      queuedRequestId: String(queuedAction?.requestId || ''),
      action: String(queuedAction?.action || ''),
      reason: String(reason || 'unknown'),
      cellId: String(targetCell?.cellId || ''),
      cellKey: String(targetCell?.key || ''),
    })
    return
  }

  notebookQueuedPythonAction.value = null
  logger.info('[Notebook] flush queued python action', {
    queuedRequestId: String(queuedAction?.requestId || ''),
    action: String(queuedAction?.action || ''),
    reason: String(reason || 'unknown'),
    cellId: String(targetCell?.cellId || ''),
    cellKey: String(targetCell?.key || ''),
  })

  if (queuedAction.action === 'run') {
    await runPythonCell(targetCell, { writeBack: false })
    return
  }
  if (queuedAction.action === 'run-writeback') {
    await runPythonCell(targetCell, { writeBack: true })
  }
}

const handleActivePythonToolbarAction = async (action) => {
  const targetCell = activeNotebookPythonCell.value
  if (!targetCell) {
    message.warning('请先选中一个 Python Cell')
    return
  }

  activateNotebookCell(targetCell, `toolbar-action:${action}`)

  if ((action === 'run' || action === 'run-writeback') && isNotebookPythonSessionBusy(targetCell)) {
    queueNotebookPythonAction(targetCell, action, `toolbar:${action}:session-busy`)
    message.info(action === 'run'
      ? '当前 Python 会话正在忙，已在空闲后自动运行当前 Cell'
      : '当前 Python 会话正在忙，已在空闲后自动运行并写回当前 Cell')
    return
  }

  if (action === 'import-table') {
    await importTableToPythonSession(targetCell)
    return
  }
  if (action === 'run') {
    await runPythonCell(targetCell, { writeBack: false })
    return
  }
  if (action === 'run-writeback') {
    await runPythonCell(targetCell, { writeBack: true })
    return
  }
  if (action === 'toggle-output') {
    toggleCellOutput(targetCell)
    return
  }
  if (action === 'toggle-session') {
    toggleCellSession(targetCell)
    return
  }
  if (action === 'open-console') {
    emit('open-console')
    return
  }

  logger.warn('[NotebookToolbar] unknown active python action', {
    action,
    cellKey: String(targetCell?.key || ''),
  })
}

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

const ensureNotebookTableRowsMaterialized = async (table = null) => {
  if (!table || typeof table !== 'object') return null
  const expectedRowCount = Math.max(0, Number(table.rowCount || 0))
  const existingRows = Array.isArray(table.rows) ? table.rows : []
  if (!String(table.resultHandle || '').trim() || existingRows.length >= expectedRowCount) {
    return table
  }

  logger.info('[Notebook] materialize full table rows for downstream action', {
    tableKey: String(table.tableKey || ''),
    tableName: String(table.tableName || ''),
    expectedRowCount,
  })

  const rows = []
  const pageSize = 1000
  for (let offset = 0; offset < expectedRowCount; offset += pageSize) {
    const response = await invoke('python_runtime_get_table_rows', {
      request: {
        handle: String(table.resultHandle || ''),
        sessionId: String(table.sessionId || ''),
        tableKey: String(table.tableKey || ''),
        manifestPath: String(table.manifestPath || ''),
        offset,
        limit: Math.min(pageSize, expectedRowCount - offset),
      },
    })
    if (!response?.ok) {
      throw new Error(String(response?.error || '读取完整表格数据失败'))
    }
    const nextRows = Array.isArray(response?.rows) ? response.rows : []
    rows.push(...nextRows)
  }

  return {
    ...table,
    rows,
  }
}

const buildMarkdownOutputFromModule = async (cell, outputModule = null, snapshot = null) => {
  const resolvedSnapshot = snapshot || getCellSnapshot(cell)
  const targetModule = outputModule || getCellOutputModule(cell)

  if (targetModule?.kind === 'table' && targetModule.table) {
    const materializedTable = await ensureNotebookTableRowsMaterialized(targetModule.table)
    return dataframeToMarkdownTable(materializedTable)
  }

  const display = targetModule?.display || resolvedSnapshot?.display || null
  const displayType = targetModule?.kind || normalizeDisplayType(display)
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

  const text = String(targetModule?.text || resolvedSnapshot?.outputText || '').trim()
  if (!text) return ''
  return `\`\`\`text\n${text}\n\`\`\``
}

const getTableExportFilters = (table = {}, cell = null) => {
  const totalTables = Number(table?.tableCount || 1)
  const baseStem = buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'xlsx').fileName.replace(/\.[^.]+$/, '')
  const preferredSheetName = String(table?.tableName || table?.tableKey || 'table').trim().replace(/[^\w.-]+/g, '_')
  const defaultBase = preferredSheetName ? `${baseStem}-${preferredSheetName}` : baseStem
  const filters = []

  if (totalTables > 1) {
    filters.push({ name: 'Excel 工作簿', extensions: ['xlsx'] })
    filters.push({ name: 'JSON 工作簿', extensions: ['json'] })
  }
  filters.push({ name: 'CSV', extensions: ['csv'] })
  filters.push({ name: 'TSV', extensions: ['tsv'] })
  filters.push({ name: 'JSON', extensions: ['json'] })
  filters.push({ name: 'Excel', extensions: ['xlsx'] })
  filters.push({ name: 'Parquet', extensions: ['parquet'] })
  filters.push({ name: 'SQLite 数据库', extensions: ['sqlite', 'db'] })
  filters.push({ name: 'DuckDB 数据库', extensions: ['duckdb'] })

  return {
    defaultBase,
    filters,
  }
}

const buildExportPayloadFromModule = async (cell, outputModule = null, snapshot = null) => {
  const resolvedSnapshot = snapshot || getCellSnapshot(cell)
  const targetModule = outputModule || getCellOutputModule(cell)

  if (targetModule?.kind === 'table' && targetModule.table) {
    if (String(targetModule.table.resultHandle || '').trim()) {
      const { defaultBase, filters } = getTableExportFilters(targetModule.table, cell)
      return {
        kind: 'python-table-handle',
        defaultName: `${defaultBase}.xlsx`,
        filters,
        bytes: null,
        summary: 'python-table-handle',
        table: targetModule.table,
      }
    }

    const csvLines = [
      (targetModule.table.columns || []).join(','),
      ...(targetModule.table.rows || []).map((row) => (Array.isArray(row) ? row : []).map((value) => JSON.stringify(value ?? '')).join(',')),
    ].join('\n')
    return {
      defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'csv').fileName,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
      bytes: new TextEncoder().encode(csvLines),
      summary: 'dataframe',
    }
  }

  const display = targetModule?.display || resolvedSnapshot?.display || null
  const displayType = targetModule?.kind || normalizeDisplayType(display)
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

  const text = String(targetModule?.text || resolvedSnapshot?.outputText || '').trim()
  if (!text) return null
  return {
    defaultName: buildNotebookAssetTarget(String(cell?.cellId || cell?.key || 'export'), 'txt').fileName,
    filters: [{ name: 'Text', extensions: ['txt'] }],
    bytes: new TextEncoder().encode(text),
    summary: 'text',
  }
}

const copyPythonCellTextOutput = async (cell, moduleId = '') => {
  const outputModule = moduleId ? getCellOutputModule(cell, moduleId) : getLatestTextOutputModule(cell)
  const text = String(outputModule?.text || '')
  if (!text) {
    message.warning('当前没有可复制的文本输出')
    return
  }

  const copied = await writeClipboardText(text)
  if (!copied) {
    message.error('复制文本输出失败')
    return
  }

  logger.info('[Notebook] copied python cell text output', {
    cellId: String(cell?.cellId || ''),
    outputModuleId: String(outputModule?.id || ''),
    textLength: text.length,
  })
  message.success('文本输出已复制')
}

const writeCellOutputToMarkdown = async (cell, moduleId = '') => {
  flushPendingNotebookMarkdown('before-writeback')
  const snapshot = getCellSnapshot(cell)
  const outputModule = getCellOutputModule(cell, moduleId)
  const markdownOutput = await buildMarkdownOutputFromModule(cell, outputModule, snapshot)
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
    outputModuleId: String(outputModule?.id || ''),
    matchStrategy: String(resolvedBlock?.matchStrategy || 'unknown'),
    persistedStableId: !!normalized.persisted,
    outputKind: String(outputModule?.kind || normalizeDisplayType(snapshot?.display) || (snapshot?.dataframe ? 'dataframe' : 'text')),
  })
  emit('update:markdown', nextMarkdown)
  lastEmittedMarkdown.value = normalizeNotebookMarkdownText(nextMarkdown)
  schedulePersistNotebookSidecar('write-cell-output-to-markdown')
  message.success('Python Cell 输出已写回到源 Markdown')
}

const exportCellOutputToFile = async (cell, moduleId = '') => {
  try {
    const snapshot = getCellSnapshot(cell)
    const outputModule = getCellOutputModule(cell, moduleId)
    const payload = await buildExportPayloadFromModule(cell, outputModule, snapshot)
    if (!payload) {
      message.warning('当前没有可导出的输出内容')
      return
    }

    if (payload.kind === 'python-table-handle' && payload.table) {
      const outputPath = await save({
        defaultPath: payload.defaultName,
        filters: payload.filters,
      })
      if (!outputPath) return

      const extension = String(outputPath).split('.').pop()?.trim().toLowerCase() || 'xlsx'
      const exportScope = Number(payload.table?.tableCount || 1) > 1 && ['xlsx', 'json'].includes(extension)
        ? 'workbook'
        : 'table'

      logger.info('[Notebook] start export handled python table result', {
        cellId: String(cell?.cellId || ''),
        outputModuleId: String(outputModule?.id || ''),
        tableKey: String(payload.table?.tableKey || ''),
        tableCount: Number(payload.table?.tableCount || 1),
        format: extension,
        exportScope,
        outputPath,
      })

      const response = await invoke('python_runtime_export_table_result', {
        request: {
          handle: String(payload.table?.resultHandle || ''),
          sessionId: String(payload.table?.sessionId || ''),
          tableKey: String(payload.table?.tableKey || ''),
          manifestPath: String(payload.table?.manifestPath || ''),
          outputPath: String(outputPath),
          format: extension,
          scope: exportScope,
        },
      })
      if (!response?.ok) {
        throw new Error(String(response?.error || '导出表格失败'))
      }

      logger.info('[Notebook] export handled python table result completed', {
        cellId: String(cell?.cellId || ''),
        outputModuleId: String(outputModule?.id || ''),
        tableKey: String(payload.table?.tableKey || ''),
        format: extension,
        exportScope,
        outputPath,
      })
      message.success('当前表格输出已导出到文件')
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
  } catch (error) {
    logger.error('[Notebook] export cell output to file failed', {
      cellId: String(cell?.cellId || ''),
      outputModuleId: String(moduleId || ''),
      error: String(error?.message || error),
    })
    message.error(`导出输出失败：${String(error?.message || error)}`)
  }
}

const importExternalDataToPythonSession = async (cell, {
  sourceType = 'file',
  inlineText = '',
  sourceLabel = '',
  variableName = '',
  sourceUrl = '',
} = {}) => {
  if (!cell?.key) return
  const requestId = createNotebookActionRequestId('nb-import')
  const normalizedSourceType = String(sourceType || 'file').trim().toLowerCase()
  const targetIdentity = getNotebookPythonCellIdentity(cell)
  const activeCellKeyAtStart = String(activeNotebookPythonCellKey.value || '')
  notebookPendingPythonImport.value = {
    requestId,
    ...targetIdentity,
    filePath: String(props.filePath || ''),
  }
  setNotebookBusyState(cell, true, `import-data:start:${requestId}`)

  try {
    let selectedPath = ''
    let effectiveText = String(inlineText || '')
    let effectiveLabel = String(sourceLabel || '')
    let effectiveUrl = String(sourceUrl || '').trim()

    if (normalizedSourceType === 'file') {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'External Data',
          extensions: ['csv', 'tsv', 'txt', 'xlsx', 'xls', 'json', 'jsonl', 'parquet', 'feather', 'db', 'sqlite', 'sqlite3', 'duckdb', 'md', 'markdown', 'yaml', 'yml', 'log', 'py', 'js', 'ts'],
        }],
      })
      if (!selected) return
      selectedPath = String(selected)
      effectiveLabel = effectiveLabel || (selectedPath.split(/[\\/]/).pop() || selectedPath)
    } else if (normalizedSourceType === 'url') {
      if (!effectiveUrl) {
        effectiveUrl = String(window.prompt?.('输入要导入的 CSV / JSON / Excel / 文本 URL', '') || '').trim()
      }
      if (!effectiveUrl) return
      effectiveLabel = effectiveLabel || effectiveUrl
    } else if (normalizedSourceType === 'clipboard') {
      effectiveText = String(await readClipboardText())
      if (!String(effectiveText || '').trim()) {
        message.warning('剪贴板里没有可导入的文本数据')
        return
      }
      effectiveLabel = effectiveLabel || 'clipboard'
    } else if (!String(effectiveText || '').trim()) {
      throw new Error('缺少可导入的数据内容')
    }

    const importPlan = buildNotebookLabImportPlan({
      sourceType: normalizedSourceType,
      sourcePath: selectedPath,
      sourceLabel: effectiveLabel,
      variableName,
      text: effectiveText,
      sourceUrl: effectiveUrl,
    })
    logger.info('[NotebookDataLab] prepared import plan', {
      requestId,
      sourceType: normalizedSourceType,
      sourceLabel: importPlan.sourceLabel,
      variableName: importPlan.variableName,
      cellId: String(cell?.cellId || ''),
      targetCellId: targetIdentity.cellId,
      targetStableId: targetIdentity.stableId,
    })

    const sessionId = createPythonSessionId(props.filePath)
    const result = await invoke('python_exec', {
      request: {
        code: importPlan.code,
        persist: true,
        expect_json: false,
        include_session_vars: true,
        session_var_names: [importPlan.variableName],
        vars: {
          _guaxi_session: sessionId,
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    if (result?.ok === false) {
      throw new Error(String(result?.stderr || result?.stdout || '导入失败'))
    }

    const targetCell = resolveNotebookPythonCellByIdentity(targetIdentity)
    if (!targetCell) {
      throw new Error('导入目标 Python Cell 已不存在，请重新选择后再试')
    }

    const snapshot = getCellSnapshot(targetCell)
    const nextSessionVars = mergeNotebookSessionVarSummaries(
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      Array.isArray(result?.session_vars) ? result.session_vars : [],
    )
    const dataflow = getGuaxiDataflowFromResult(result)
    const previousOutputCount = Array.isArray(snapshot?.outputs) ? snapshot.outputs.length : 0
    const outputText = normalizedSourceType === 'clipboard'
      ? `已将剪贴板数据导入到会话变量 ${importPlan.variableName}，可以在实验室面板中继续生成输出、写回 Markdown 或导出。`
      : `已将外部文件 ${importPlan.sourceLabel} 导入到会话变量 ${importPlan.variableName}，可以在实验室面板中继续生成输出、写回 Markdown 或导出。`
    const nextOutputs = appendPythonCellOutputModules(
      snapshot?.outputs || [],
      buildPythonCellOutputModules({
        outputText,
        display: null,
        dataframe: null,
        tables: [],
        source: 'data-lab-import',
      }),
    )
    const nextImports = upsertNotebookLabImportRecord(getCellLabImports(targetCell), {
      variableName: importPlan.variableName,
      sourceType: importPlan.sourceType,
      sourceLabel: importPlan.sourceLabel,
      dataKind: 'value',
      importedAt: new Date().toISOString(),
    })

    patchPythonCellSnapshot(props.filePath, targetCell.cellId, {
      codeDigest: getNotebookCellContentDigest(targetCell),
      outputText,
      display: snapshot?.display || null,
      dataframe: snapshot?.dataframe || null,
      outputs: nextOutputs,
      result: snapshot?.result || null,
      sessionVars: nextSessionVars,
      lab: patchCellSnapshotLab(targetCell, {
        imports: nextImports,
        lastImportSourceType: importPlan.sourceType,
        lastImportAt: new Date().toISOString(),
        ...(dataflow
          ? {
            dataflow: {
              ...dataflow,
              updatedAt: new Date().toISOString(),
              reason: 'import-data-to-python-session',
            },
          }
          : {}),
      }),
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: false,
      },
    })
    logger.info('[NotebookDataLab] imported external data into python session', {
      requestId,
      cellId: String(targetCell?.cellId || ''),
      cellKey: String(targetCell?.key || ''),
      stableId: String(targetCell?.meta?.id || ''),
      variableName: importPlan.variableName,
      sourceType: importPlan.sourceType,
      sourceLabel: importPlan.sourceLabel,
      previousOutputCount,
      updatedSessionVarCount: Array.isArray(result?.session_vars) ? result.session_vars.length : 0,
      sessionVarCount: nextSessionVars.length,
      outputModuleCount: nextOutputs.length,
      activeCellChangedDuringImport: activeCellKeyAtStart !== String(activeNotebookPythonCellKey.value || ''),
      activeCellKeyAtFinish: String(activeNotebookPythonCellKey.value || ''),
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('import-data-to-python-session')
    message.success(`已导入到会话变量 ${importPlan.variableName}`)
  } catch (error) {
    logger.error('[NotebookDataLab] import external data to python session failed', {
      requestId,
      cellId: String(cell?.cellId || ''),
      cellKey: String(cell?.key || ''),
      targetCellId: targetIdentity.cellId,
      targetCellKey: targetIdentity.cellKey,
      sourceType: normalizedSourceType,
      error: String(error?.message || error),
    })
    message.error(`导入数据失败：${String(error?.message || error)}`)
  } finally {
    if (String(notebookPendingPythonImport.value?.requestId || '') === requestId) {
      notebookPendingPythonImport.value = null
    }
    const resolvedTargetCell = resolveNotebookPythonCellByIdentity(targetIdentity) || cell
    setNotebookBusyState(resolvedTargetCell, false, `import-data:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`import-data:finish:${requestId}`)
  }
}

const importTableToPythonSession = async (cell) => {
  await importExternalDataToPythonSession(cell, { sourceType: 'file' })
}

const materializeSessionVariableOutput = async (cell, dataset = {}, {
  writeBack = false,
  exportAfter = false,
} = {}) => {
  if (!cell?.key) return null
  const variableName = String(dataset?.variableName || '').trim()
  if (!variableName) {
    message.warning('缺少变量名，无法生成实验室输出')
    return null
  }

  const requestId = createNotebookActionRequestId('nb-lab')
  setNotebookBusyState(cell, true, `data-lab-materialize:start:${requestId}`)

  try {
    const code = buildNotebookLabMaterializeCode(dataset)
    if (!code) {
      throw new Error('缺少实验室输出脚本')
    }

    const res = await invoke('python_exec', {
      request: {
        code,
        persist: true,
        expect_json: true,
        include_session_vars: false,
        vars: {
          _guaxi_session: createPythonSessionId(props.filePath),
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const outputText = formatPythonExecOutput(res)
    const display = getGuaxiDisplayFromResult(res)
    const tables = getGuaxiTablesFromResult(res)
    const dataframe = tables[0] || null
    const runOutputs = decorateNotebookLabOutputModules(
      buildPythonCellOutputModules({
        outputText,
        display,
        dataframe,
        tables,
        source: 'data-lab-materialize',
      }),
      dataset,
      'data-lab-materialize',
    )
    const snapshot = getCellSnapshot(cell)
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || tables.length)
    const nextOutputs = hasRenderableOutput
      ? appendPythonCellOutputModules(snapshot?.outputs || [], runOutputs)
      : (snapshot?.outputs || [])
    const outputModuleId = String(runOutputs?.[0]?.id || '')
    const hideOutput = !nextOutputs.length

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: hideOutput ? '' : outputText,
      display,
      dataframe,
      outputs: nextOutputs,
      result: res,
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      lab: patchCellSnapshotLab(cell, {
        lastMaterializedVariable: variableName,
        lastMaterializedAt: new Date().toISOString(),
      }),
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: hideOutput,
      },
    })
    logger.info('[NotebookDataLab] materialized session variable output', {
      requestId,
      cellId: String(cell?.cellId || ''),
      variableName,
      dataKind: String(dataset?.dataKind || ''),
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      tableCount: tables.length,
      outputModuleId,
      totalOutputModuleCount: nextOutputs.length,
      writeBack,
      exportAfter,
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('data-lab-materialize-success')

    if (writeBack && outputModuleId) {
      await writeCellOutputToMarkdown(cell, outputModuleId)
    }
    if (exportAfter && outputModuleId) {
      await exportCellOutputToFile(cell, outputModuleId)
    }

    if (res?.ok === false) {
      message.error(`变量 ${variableName} 生成输出失败`)
      return {
        outputModuleId,
        ok: false,
      }
    }

    if (!writeBack && !exportAfter) {
      message.success(`变量 ${variableName} 已生成输出`)
    }
    return {
      outputModuleId,
      ok: true,
    }
  } catch (error) {
    const snapshot = getCellSnapshot(cell)
    const errorOutputs = appendPythonCellOutputModules(
      snapshot?.outputs || [],
      decorateNotebookLabOutputModules(
        buildPythonCellOutputModules({
          outputText: `变量 ${variableName} 生成失败：${String(error?.message || error)}`,
          display: null,
          dataframe: null,
          source: 'data-lab-error',
        }),
        dataset,
        'data-lab-error',
      ),
    )
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: `变量 ${variableName} 生成失败：${String(error?.message || error)}`,
      display: null,
      dataframe: null,
      outputs: errorOutputs,
      result: null,
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      lab: snapshot?.lab || { imports: [] },
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: false,
      },
    })
    logger.error('[NotebookDataLab] materialize session variable failed', {
      requestId,
      cellId: String(cell?.cellId || ''),
      variableName,
      error: String(error?.message || error),
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('data-lab-materialize-failed')
    message.error(`生成变量输出失败：${String(error?.message || error)}`)
    return null
  } finally {
    setNotebookBusyState(cell, false, `data-lab-materialize:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`data-lab-materialize:finish:${requestId}`)
  }
}

const runNotebookLabGeneratedCode = async (cell, {
  code = '',
  source = 'data-lab-action',
  dataset = {},
  writeBack = false,
  exportAfter = false,
  successMessage = '',
  labPatch = {},
} = {}) => {
  if (!cell?.key || !String(code || '').trim()) return null
  const requestId = createNotebookActionRequestId('nb-lab-action')
  setNotebookBusyState(cell, true, `${source}:start:${requestId}`)

  try {
    const res = await invoke('python_exec', {
      request: {
        code: String(code || ''),
        persist: true,
        expect_json: true,
        include_session_vars: true,
        vars: {
          _guaxi_session: createPythonSessionId(props.filePath),
          _doc_path: String(props.filePath || ''),
          _py_cell_id: String(cell.cellId || ''),
        },
      },
    })

    const outputText = formatPythonExecOutput(res)
    const display = getGuaxiDisplayFromResult(res)
    const tables = getGuaxiTablesFromResult(res)
    const dataframe = tables[0] || null
    const dataflow = getGuaxiDataflowFromResult(res)
    const snapshot = getCellSnapshot(cell)
    const runOutputs = decorateNotebookLabOutputModules(
      buildPythonCellOutputModules({
        outputText,
        display,
        dataframe,
        tables,
        source,
      }),
      dataset,
      source,
    )
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || tables.length)
    const nextOutputs = hasRenderableOutput
      ? appendPythonCellOutputModules(snapshot?.outputs || [], runOutputs)
      : (snapshot?.outputs || [])
    const outputModuleId = String(runOutputs?.[0]?.id || '')
    const nextSessionVars = resolveNotebookSessionVarsFromExecResult(
      res,
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    )

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: outputText || snapshot?.outputText || '',
      display: display || snapshot?.display || null,
      dataframe: dataframe || snapshot?.dataframe || null,
      outputs: nextOutputs,
      result: res,
      sessionVars: nextSessionVars,
      lab: patchCellSnapshotLab(cell, {
        ...(labPatch && typeof labPatch === 'object' ? labPatch : {}),
        ...(dataflow
          ? {
            dataflow: {
              ...dataflow,
              updatedAt: new Date().toISOString(),
              reason: source,
            },
          }
          : {}),
      }),
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: !nextOutputs.length,
        staleVariables: [],
        staleReason: '',
      },
    })

    if (dataflow) {
      markDependentNotebookCellsStale(cell, dataflow, `${source}:success`)
    }
    if (isActiveNotebookPythonCell(cell) || isNotebookPythonSessionPanelVisible.value) {
      syncNotebookPythonSessionInspector(cell, `${source}:success`)
    }

    logger.info('[NotebookDataLab] generated lab action completed', {
      requestId,
      source,
      cellId: String(cell?.cellId || ''),
      variableName: String(dataset?.variableName || ''),
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      tableCount: tables.length,
      sessionVarCount: nextSessionVars.length,
      touchedVariables: Array.isArray(dataflow?.touched) ? dataflow.touched : [],
      outputModuleCount: nextOutputs.length,
      touchedVariables: Array.isArray(dataflow?.touched) ? dataflow.touched : [],
      writeBack,
      exportAfter,
    })
    if (dataflow) {
      markDependentNotebookCellsStale(targetCell, dataflow, 'import-data-to-python-session')
    }
    if (isActiveNotebookPythonCell(targetCell) || isNotebookPythonSessionPanelVisible.value) {
      syncNotebookPythonSessionInspector(targetCell, 'import-data-to-python-session')
    }

    bumpRefresh()
    schedulePersistNotebookSidecar(`${source}-success`)

    if (writeBack && outputModuleId) {
      await writeCellOutputToMarkdown(cell, outputModuleId)
    }
    if (exportAfter && outputModuleId) {
      await exportCellOutputToFile(cell, outputModuleId)
    }

    if (res?.ok === false) {
      message.error('实验室动作执行失败')
      return { ok: false, outputModuleId }
    }
    if (successMessage) message.success(successMessage)
    return { ok: true, outputModuleId }
  } catch (error) {
    logger.error('[NotebookDataLab] generated lab action failed', {
      requestId,
      source,
      cellId: String(cell?.cellId || ''),
      variableName: String(dataset?.variableName || ''),
      error: String(error?.message || error),
    })
    message.error(`实验室动作失败：${String(error?.message || error)}`)
    return null
  } finally {
    setNotebookBusyState(cell, false, `${source}:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`${source}:finish:${requestId}`)
  }
}

const handleNotebookDataLabActionEvent = async (event) => {
  const detail = event?.detail && typeof event.detail === 'object' ? event.detail : {}
  const targetFilePath = String(detail?.filePath || '').trim()
  if (targetFilePath && targetFilePath !== String(props.filePath || '').trim()) return

  const targetCell = resolveNotebookPythonCellByIdentity(detail) || activeNotebookPythonCell.value
  if (!targetCell) {
    logger.warn('[NotebookDataLab] ignored lab action because target cell is missing', {
      action: String(detail?.action || ''),
      cellId: String(detail?.cellId || ''),
      stableId: String(detail?.stableId || ''),
    })
    return
  }

  activateNotebookCell(targetCell, `data-lab:${String(detail?.action || 'unknown')}`)
  const dataset = {
    variableName: String(detail?.variableName || ''),
    dataKind: String(detail?.dataKind || ''),
    chartType: String(detail?.chartType || ''),
  }

  if (detail.action === 'import-file') {
    await importExternalDataToPythonSession(targetCell, { sourceType: 'file' })
    return
  }
  if (detail.action === 'import-clipboard') {
    await importExternalDataToPythonSession(targetCell, { sourceType: 'clipboard' })
    return
  }
  if (detail.action === 'import-url') {
    await importExternalDataToPythonSession(targetCell, {
      sourceType: 'url',
      sourceUrl: String(detail?.sourceUrl || ''),
      sourceLabel: String(detail?.sourceLabel || ''),
      variableName: String(detail?.variableName || ''),
    })
    return
  }
  if (detail.action === 'import-inline-data') {
    await importExternalDataToPythonSession(targetCell, {
      sourceType: String(detail?.sourceType || 'inline'),
      inlineText: String(detail?.text || ''),
      sourceLabel: String(detail?.sourceLabel || ''),
      variableName: String(detail?.variableName || ''),
    })
    return
  }
  if (detail.action === 'plot-variable' || detail.action === 'plot-variable-writeback' || detail.action === 'plot-variable-export') {
    const code = buildNotebookLabPlotCode({
      variableName: String(detail?.variableName || ''),
      chartType: String(detail?.chartType || 'line'),
      x: String(detail?.x || ''),
      y: String(detail?.y || ''),
      z: String(detail?.z || ''),
      color: String(detail?.color || ''),
      title: String(detail?.title || ''),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-plot',
      dataset,
      writeBack: detail.action === 'plot-variable-writeback',
      exportAfter: detail.action === 'plot-variable-export',
      successMessage: '图表已生成',
      labPatch: {
        lastPlotVariable: String(detail?.variableName || ''),
        lastPlotType: String(detail?.chartType || 'line'),
        lastPlotAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'math-evaluate' || detail.action === 'math-writeback') {
    const code = buildNotebookLabMathCode({
      expression: String(detail?.expression || ''),
      mode: String(detail?.mode || 'plot2d'),
      variable: String(detail?.variable || 'x'),
      rangeMin: Number(detail?.rangeMin ?? -10),
      rangeMax: Number(detail?.rangeMax ?? 10),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-math',
      dataset: {
        variableName: 'math',
        dataKind: 'math',
      },
      writeBack: detail.action === 'math-writeback',
      successMessage: '数学结果已生成',
      labPatch: {
        lastMathExpression: String(detail?.expression || ''),
        lastMathMode: String(detail?.mode || 'plot2d'),
        lastMathAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'generate-report' || detail.action === 'writeback-report') {
    const code = buildNotebookLabReportCode({
      title: String(detail?.title || '探索实验室报告'),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-report',
      dataset: {
        variableName: 'report',
        dataKind: 'markdown',
      },
      writeBack: detail.action === 'writeback-report',
      successMessage: '报告草稿已生成',
      labPatch: {
        lastReportAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'variable-delete' || detail.action === 'variable-rename' || detail.action === 'variable-copy' || detail.action === 'variable-derive') {
    const variableAction = String(detail.action || '').replace(/^variable-/, '')
    const code = buildNotebookLabVariableOperationCode({
      action: variableAction,
      variableName: String(detail?.variableName || ''),
      targetName: String(detail?.targetName || ''),
      expression: String(detail?.expression || ''),
    })
    await runNotebookLabGeneratedCode(targetCell, {
      code,
      source: 'data-lab-variable-op',
      dataset,
      successMessage: '变量操作已完成',
      labPatch: {
        lastVariableAction: variableAction,
        lastVariableActionAt: new Date().toISOString(),
      },
    })
    return
  }
  if (detail.action === 'materialize-variable') {
    await materializeSessionVariableOutput(targetCell, dataset)
    return
  }
  if (detail.action === 'writeback-variable') {
    await materializeSessionVariableOutput(targetCell, dataset, { writeBack: true })
    return
  }
  if (detail.action === 'export-variable') {
    await materializeSessionVariableOutput(targetCell, dataset, { exportAfter: true })
    return
  }

  logger.warn('[NotebookDataLab] unknown lab action received', {
    action: String(detail?.action || ''),
    cellId: String(targetCell?.cellId || ''),
  })
}

const runPythonCell = async (cell, { writeBack = false } = {}) => {
  flushPendingNotebookMarkdown('before-run-python-cell')
  if (!cell?.code) {
    message.warning('Python 代码为空')
    return
  }

  const requestId = createNotebookActionRequestId('nb-run')
  setNotebookBusyState(cell, true, `run-python-cell:start:${requestId}`)

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
    const tables = getGuaxiTablesFromResult(res)
    const dataframe = tables[0] || null
    const hasRenderableOutput = !!(String(outputText || '').trim() || display || tables.length)
    const snapshot = getCellSnapshot(cell)
    const runOutputs = buildPythonCellOutputModules({
      outputText,
      display,
      dataframe,
      tables,
      source: 'execution',
    })
    const nextSessionVars = resolveNotebookSessionVarsFromExecResult(
      res,
      Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
    )
    const dataflow = getGuaxiDataflowFromResult(res)
    const nextOutputs = hasRenderableOutput
      ? appendPythonCellOutputModules(snapshot?.outputs || [], runOutputs)
      : (snapshot?.outputs || [])
    const hideOutput = !nextOutputs.length
    logger.info('[Notebook] python cell executed', {
      requestId,
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      writeBack,
      hasTextOutput: !!String(outputText || '').trim(),
      hasDisplay: !!display,
      hasDataframe: !!dataframe,
      tableCount: tables.length,
      sessionVarCount: nextSessionVars.length,
      touchedVariables: Array.isArray(dataflow?.touched) ? dataflow.touched : [],
      hideOutput,
      appendedOutputModuleCount: runOutputs.length,
      totalOutputModuleCount: nextOutputs.length,
    })

    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: hideOutput ? '' : outputText,
      display,
      dataframe,
      outputs: nextOutputs,
      result: res,
      sessionVars: nextSessionVars,
      lab: dataflow
        ? patchCellSnapshotLab(cell, {
          dataflow: {
            ...dataflow,
            updatedAt: new Date().toISOString(),
            reason: 'run-python-cell',
          },
        })
        : snapshot?.lab || { imports: [] },
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: hideOutput,
        staleVariables: [],
        staleReason: '',
      },
    })
    if (dataflow) {
      markDependentNotebookCellsStale(cell, dataflow, 'run-python-cell-success')
    }
    if (isActiveNotebookPythonCell(cell) || isNotebookPythonSessionPanelVisible.value) {
      syncNotebookPythonSessionInspector(cell, 'run-python-cell-success')
    }
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
    const snapshot = getCellSnapshot(cell)
    const errorOutputs = appendPythonCellOutputModules(
      snapshot?.outputs || [],
      buildPythonCellOutputModules({
        outputText: `运行失败：${String(error?.message || error)}`,
        display: null,
        dataframe: null,
        source: 'execution-error',
      }),
    )
    patchPythonCellSnapshot(props.filePath, cell.cellId, {
      codeDigest: getNotebookCellContentDigest(cell),
      outputText: `运行失败：${String(error?.message || error)}`,
      display: null,
      dataframe: null,
      outputs: errorOutputs,
      result: null,
      // 设计说明：
      // 1. 运行失败并不等于 Python 持久会话被重置，之前已有的变量往往仍然存在；
      // 2. 保留上一份变量快照，可以避免用户一次失败后右侧实验室瞬间“清空”，
      //    与同一会话里后续仍能访问旧变量的行为不一致；
      // 3. 真正需要清空变量时，应由 `%reset` / drop session / 后端明确返回空变量列表来决定。
      sessionVars: Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
      meta: {
        ...(snapshot?.meta || {}),
        outputCollapsed: false,
      },
    })
    logger.error('[Notebook] python cell execution failed', {
      requestId,
      key: String(cell?.key || ''),
      cellId: String(cell?.cellId || ''),
      outputModuleCount: errorOutputs.length,
      error: String(error?.message || error),
    })
    bumpRefresh()
    schedulePersistNotebookSidecar('run-python-cell-failed')
    message.error(`运行失败：${String(error?.message || error)}`)
  } finally {
    setNotebookBusyState(cell, false, `run-python-cell:finish:${requestId}`)
    await flushQueuedNotebookPythonActionIfIdle(`run-python-cell:finish:${requestId}`)
  }
}

watch(
  [() => props.markdown, () => props.filePath],
  ([nextMarkdown], oldPair = []) => {
    const prevFilePath = Array.isArray(oldPair) ? oldPair[1] : ''
    const filePathChanged = String(prevFilePath || '') !== String(props.filePath || '')
    if (filePathChanged) {
      // 设计说明：
      // 1. Notebook 组件在 tab 间复用时，文件切换与新内容渲染之间存在一个异步窗口。
      // 2. 如果不先清空旧文件的渲染结果，模板会短暂继续显示上一个文件的 HTML，
      //    在 cellKey 重复时甚至可能被错误复用。
      // 3. 因此在 filePath 切换的第一时间先重置渲染 map，确保界面只等待当前文件的新结果。
      renderedMarkdownHtmlMap.value = {}
      renderedOutputMarkdownHtmlMap.value = {}
      resolvedDisplayUrlMap.value = {}
      notebookSearchIndexReady.value = false
      clearNotebookSearchHighlights()
      logger.info('[Notebook] cleared rendered html maps for file switch', {
        previousFilePath: String(prevFilePath || ''),
        nextFilePath: String(props.filePath || ''),
      })
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

watch([selectionTranslateFrom, selectionTranslateTo], ([from, to], [prevFrom, prevTo]) => {
  if (!selectionTranslatePanelVisible.value) return
  if (from === prevFrom && to === prevTo) return

  logger.info('[NotebookSelection] translate language changed', { from, to })
  scheduleSelectionTranslate()
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
  document.addEventListener('selectionchange', handleNotebookGlobalSelectionChange, true)
  document.addEventListener('mousedown', handleNotebookGlobalMouseDown, true)
  window.addEventListener(NOTEBOOK_DATALAB_EVENT, handleNotebookDataLabActionEvent)
  void listen('notebook-data-lab-rpc', (event) => {
    logger.info('[NotebookDataLab] received backend rpc event', {
      requestId: String(event?.payload?.requestId || ''),
      action: String(event?.payload?.action || ''),
      filePath: String(event?.payload?.filePath || ''),
    })
    return handleNotebookDataLabActionEvent({
      detail: event?.payload && typeof event.payload === 'object' ? event.payload : {},
    })
  }).then((unlisten) => {
    notebookDataLabRpcUnlisten = unlisten
  }).catch((error) => {
    logger.warn('[NotebookDataLab] failed to bind backend rpc listener', {
      error: String(error?.message || error),
    })
  })
  disposeNotebookDataLabBridge = installNotebookDataLabBridge({
    getContext: () => {
      const targetCell = activeNotebookPythonCell.value
      const snapshot = targetCell ? getCellSnapshot(targetCell) : null
      return {
        filePath: String(props.filePath || ''),
        sessionId: createPythonSessionId(props.filePath),
        activeCell: targetCell
          ? {
            cellId: String(targetCell?.cellId || ''),
            cellKey: String(targetCell?.key || ''),
            stableId: String(targetCell?.meta?.id || ''),
            title: String(targetCell?.meta?.title || ''),
          }
          : null,
        datasets: targetCell
          ? buildNotebookLabDatasetDescriptors(
            Array.isArray(snapshot?.sessionVars) ? snapshot.sessionVars : [],
            Array.isArray(snapshot?.lab?.imports) ? snapshot.lab.imports : [],
          )
          : [],
      }
    },
  })
  logger.info('[NotebookDataLab] mounted event bridge', {
    filePath: String(props.filePath || ''),
  })
  logger.info('[NotebookSelection] mounted selection listeners')
})

onBeforeUnmount(() => {
  clearNotebookTopToolbarHideTimer()
  if (notebookSearchIndexSyncTimer) {
    window.clearTimeout(notebookSearchIndexSyncTimer)
    notebookSearchIndexSyncTimer = 0
  }
  if (notebookSearchQueryTimer) {
    window.clearTimeout(notebookSearchQueryTimer)
    notebookSearchQueryTimer = 0
  }
  stopSelectionAudioPlayback()
  closeNotebookSelectionTranslatePanel()
  clearNotebookSelectionState('component-unmount')
  document.removeEventListener('selectionchange', handleNotebookGlobalSelectionChange, true)
  document.removeEventListener('mousedown', handleNotebookGlobalMouseDown, true)
  window.removeEventListener(NOTEBOOK_DATALAB_EVENT, handleNotebookDataLabActionEvent)
  if (typeof notebookDataLabRpcUnlisten === 'function') {
    notebookDataLabRpcUnlisten()
    notebookDataLabRpcUnlisten = null
  }
  if (typeof disposeNotebookDataLabBridge === 'function') {
    disposeNotebookDataLabBridge()
    disposeNotebookDataLabBridge = null
  }
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
  --guaxi-notebook-sticky-top: 6px;
  --guaxi-notebook-toolbar-shell-height: 8px;
  --guaxi-notebook-toolbar-max-width: min(100%, 520px);
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

.guaxi-notebook.is-top-toolbar-visible {
  --guaxi-notebook-toolbar-shell-height: 50px;
}

.guaxi-notebook.has-active-python-cell {
  --guaxi-notebook-toolbar-max-width: min(100%, 760px);
}

.guaxi-notebook__toolbar-shell {
  position: sticky;
  top: var(--guaxi-notebook-sticky-top);
  z-index: 26;
  display: flex;
  justify-content: center;
  height: var(--guaxi-notebook-toolbar-shell-height);
  min-height: var(--guaxi-notebook-toolbar-shell-height);
  margin: 0 auto 2px;
  transition: height 0.18s ease, min-height 0.18s ease;
  overflow: visible;
}

.guaxi-notebook__toolbar {
  position: absolute;
  inset: 0 auto auto 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* 设计说明：
   * 1. 绝对定位元素搭配 `fit-content` 时，浏览器会走 shrink-to-fit 计算，视觉上容易出现底板宽度比按钮行更短。
   * 2. 这里改成按内容宽度强制撑开，再给一个可视区上限，保证顶部工具条既能完整包裹 icon，也不会在窄窗口冲出容器。
   */
  width: max-content;
  min-width: max-content;
  max-width: min(var(--guaxi-notebook-toolbar-max-width), calc(100% - 24px));
  margin: 0;
  /* 设计说明：
   * 1. 顶部工具条需要明确留出上下呼吸感，否则 30px 的 icon 按钮会显得比背景条更高。
   * 2. 这里同步提高内边距和最小高度，让“底板”稳定包住整排入口，避免出现按钮悬在条外的观感。
   */
  min-height: 42px;
  padding: 6px 12px;
  box-sizing: border-box;
  backdrop-filter: blur(10px);
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, calc(-100% + 8px));
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.guaxi-notebook__toolbar-shell.is-visible .guaxi-notebook__toolbar {
  opacity: 1;
  pointer-events: auto;
  transform: translate(-50%, 0);
}

.guaxi-notebook__search-panel-shell {
  position: sticky;
  top: calc(var(--guaxi-notebook-sticky-top) + var(--guaxi-notebook-toolbar-shell-height));
  z-index: 24;
  min-height: 42px;
  display: flex;
  align-items: flex-start;
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
  min-height: 40px;
  margin-top: 2px;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 86%, transparent);
  border-radius: 12px;
  /* 保持搜索面板是一个完整的实体卡片，避免透明混色让中间看起来像“空了一块”。 */
  background: var(--guaxi-notebook-panel);
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.14);
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

.guaxi-notebook__panel-icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 8px;
}

.guaxi-notebook__search-btn--danger {
  color: #ef4444;
}

.guaxi-notebook__selection-toolbar {
  position: absolute;
  z-index: 25;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 86%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 94%, transparent);
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.14);
  backdrop-filter: blur(10px);
}

.guaxi-notebook__selection-tool-btn {
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--guaxi-notebook-strong-border);
  border-radius: 9px;
  background: var(--guaxi-notebook-panel-soft);
  color: var(--guaxi-notebook-text);
  cursor: pointer;
  transition: transform 0.16s ease, background-color 0.16s ease, border-color 0.16s ease;
}

.guaxi-notebook__selection-tool-btn:hover:not(:disabled) {
  background: var(--guaxi-notebook-btn-hover);
  border-color: color-mix(in srgb, var(--guaxi-notebook-primary) 36%, var(--guaxi-notebook-strong-border));
  transform: translateY(-1px);
}

.guaxi-notebook__selection-tool-btn:disabled {
  opacity: 0.56;
  cursor: not-allowed;
}

.guaxi-notebook__toolbar-main {
  display: flex;
  align-items: center;
  justify-content: center;
  width: max-content;
  min-width: max-content;
  flex-wrap: nowrap;
  gap: 8px;
}

.guaxi-notebook__toolbar-divider {
  width: 1px;
  height: 18px;
  background: color-mix(in srgb, var(--guaxi-notebook-strong-border) 82%, transparent);
  opacity: 0.8;
}

.guaxi-notebook__toolbar-main--solo {
  width: auto;
  justify-content: center;
}

.guaxi-notebook__cell {
  position: relative;
  overflow: visible;
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

.guaxi-notebook__cell.is-active-python-cell {
  outline: 2px solid color-mix(in srgb, var(--guaxi-notebook-primary) 72%, transparent);
  outline-offset: 2px;
  border-radius: 14px;
  box-shadow: 0 0 0 5px color-mix(in srgb, var(--guaxi-notebook-primary) 14%, transparent);
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
  min-height: 24px;
  margin: 2px 0 4px;
  z-index: 3;
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
  z-index: 1;
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
  z-index: 2;
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
  padding: 0px 0px 0px;
}

.guaxi-notebook__markdown-surface {
  position: relative;
  display: grid;
  gap: 6px;
  padding: 0;
  overflow: visible;
}

.guaxi-notebook__cell-delete {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 4;
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 78%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 95%, transparent);
  color: #dc2626;
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.08);
  opacity: 0;
  transform: translateY(-4px);
  pointer-events: none;
  transition: opacity 0.18s ease, transform 0.18s ease, background-color 0.18s ease, border-color 0.18s ease;
}

.guaxi-notebook__cell:hover .guaxi-notebook__cell-delete,
.guaxi-notebook__cell:focus-within .guaxi-notebook__cell-delete {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.guaxi-notebook__cell-delete:hover {
  background: color-mix(in srgb, #ef4444 12%, var(--guaxi-notebook-panel) 88%);
  border-color: color-mix(in srgb, #ef4444 28%, var(--guaxi-notebook-strong-border));
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

.guaxi-notebook__python-badge.is-busy {
  color: #1d4ed8;
  border-color: color-mix(in srgb, #60a5fa 48%, var(--guaxi-notebook-strong-border));
  box-shadow: 0 10px 24px rgba(37, 99, 235, 0.14);
}

.guaxi-notebook__python-badge-icon {
  flex: 0 0 auto;
}

.guaxi-notebook__busy-spin {
  animation: guaxi-notebook-busy-spin 0.9s linear infinite;
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
  justify-content: flex-end;
}

.guaxi-notebook__python-toolbar .guaxi-notebook__icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 8px;
}

.guaxi-notebook__python-toolbar .guaxi-notebook__icon-btn:not(.primary) {
  background: color-mix(in srgb, var(--guaxi-notebook-btn) 82%, transparent);
}

.guaxi-notebook__stale-banner {
  margin: 4px 0 8px;
  padding: 8px 10px;
  border: 1px solid color-mix(in srgb, #d97706 34%, var(--guaxi-notebook-border));
  border-radius: 8px;
  background: color-mix(in srgb, #f59e0b 12%, var(--guaxi-notebook-panel) 88%);
  color: color-mix(in srgb, #92400e 82%, var(--guaxi-notebook-text) 18%);
  font-size: 12px;
  line-height: 1.45;
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
  gap: 8px;
}

.guaxi-notebook__output-module {
  border-radius: 12px;
  border: 1px solid var(--guaxi-notebook-strong-border);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 86%, var(--guaxi-notebook-panel) 14%);
  overflow: hidden;
}

.guaxi-notebook__output-module.is-collapsed {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 74%, var(--guaxi-notebook-panel) 26%);
}

.guaxi-notebook__output-module-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
  padding: 10px 12px;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 74%, transparent);
}

.guaxi-notebook__output-module.is-collapsed .guaxi-notebook__output-module-head {
  border-bottom: none;
}

.guaxi-notebook__output-module-copy {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.guaxi-notebook__output-module-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--guaxi-notebook-text);
}

.guaxi-notebook__output-module-summary {
  font-size: 12px;
  color: var(--guaxi-notebook-muted);
  word-break: break-word;
}

.guaxi-notebook__output-module-actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.guaxi-notebook__output-module-body {
  display: grid;
  gap: 8px;
  padding: 0 12px 10px;
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
  border: none;
  border-radius: 0;
  background: transparent;
  padding: 0;
  max-height: min(62vh, 720px);
  overflow-x: auto;
  overflow-y: auto;
}

.guaxi-notebook__rich {
  font-size: 17px;
  line-height: 1.5;
  padding-top: 2px;
}

.guaxi-notebook__output-text {
  white-space: pre-wrap;
  word-break: break-word;
  padding-top: 2px;
}

.guaxi-notebook__dataframe table {
  width: max-content;
  min-width: 100%;
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
.guaxi-notebook.theme-dark .guaxi-notebook__output-module,
.guaxi-notebook.theme-dark .guaxi-notebook__var,
.guaxi-notebook.theme-dark .guaxi-notebook__python-editor {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 88%, #0f172a 12%);
}

@keyframes guaxi-notebook-busy-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.guaxi-notebook__markdown-preview :deep(.md-paragraph),
.guaxi-notebook__rich :deep(.md-paragraph) {
  color: var(--guaxi-notebook-text);
}

/* 设计说明：
 * 1. Explorer 模式下也需要区分“Markdown 渲染器生成的普通段落”和“作者手写的 HTML 段落”。
 * 2. 对于显式带 `align` 的 HTML 段落，这里恢复其原始对齐语义，避免被后续基础样式吞掉。
 */
.guaxi-notebook__markdown-preview :deep(p[align='center']),
.guaxi-notebook__rich.markdown-body :deep(p[align='center']) {
  text-align: center !important;
}

.guaxi-notebook__markdown-preview :deep(p[align='right']),
.guaxi-notebook__rich.markdown-body :deep(p[align='right']) {
  text-align: right !important;
}

.guaxi-notebook__markdown-preview :deep(p[align='left']),
.guaxi-notebook__rich.markdown-body :deep(p[align='left']) {
  text-align: left !important;
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

/* 设计说明：
 * 1. 这里的自动居中只服务于 Markdown 语法生成的图片与媒体组。
 * 2. 手写 HTML 中的图片布局需要交给作者自己的段落 / 行内结构控制，不能再被统一 `margin:auto`
 *    覆盖，否则徽章组、Logo 组会被错误地堆叠并居中。
 */
.guaxi-notebook__markdown-preview :deep(.md-paragraph img),
.guaxi-notebook__rich.markdown-body :deep(.md-paragraph img),
.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__image),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__image) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 0 auto;
}

.guaxi-notebook__markdown-preview :deep(p > img:only-child),
.guaxi-notebook__markdown-preview :deep(p > a:only-child > img:only-child),
.guaxi-notebook__rich.markdown-body :deep(p > img:only-child),
.guaxi-notebook__rich.markdown-body :deep(p > a:only-child > img:only-child) {
  display: block;
  margin-left: auto;
  margin-right: auto;
}

/* 设计说明：
 * 1. 对于作者直接写进 Markdown 的 HTML 图片块，需要保留其 inline / badge 排版能力。
 * 2. 因此带 `align` / `style="text-align"` 的段落里，把图片恢复为 inline-block，
 *    由段落自身的对齐属性来控制最终布局。
 */
.guaxi-notebook__markdown-preview :deep(p[align] > img),
.guaxi-notebook__markdown-preview :deep(p[align] > a > img),
.guaxi-notebook__markdown-preview :deep(p[style*='text-align'] > img),
.guaxi-notebook__markdown-preview :deep(p[style*='text-align'] > a > img),
.guaxi-notebook__rich.markdown-body :deep(p[align] > img),
.guaxi-notebook__rich.markdown-body :deep(p[align] > a > img),
.guaxi-notebook__rich.markdown-body :deep(p[style*='text-align'] > img),
.guaxi-notebook__rich.markdown-body :deep(p[style*='text-align'] > a > img) {
  display: inline-block;
  max-width: 100%;
  height: auto;
  margin: 0;
  vertical-align: middle;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group) {
  --guaxi-media-gap: clamp(14px, 2vw, 24px);
  display: flex !important;
  flex-wrap: wrap;
  justify-content: center !important;
  align-items: stretch;
  align-content: flex-start;
  gap: var(--guaxi-media-gap);
  width: 100% !important;
  max-width: 1000px;
  margin: 22px auto !important;
  padding: 6px 0;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-single),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-single) {
  display: block !important;
  width: min(100%, 760px) !important;
  gap: 0;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-gallery),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-gallery) {
  display: grid !important;
  grid-template-columns: repeat(auto-fit, minmax(220px, 320px));
  justify-content: center !important;
  justify-items: center;
  align-items: start;
  padding-left: 6px;
  padding-right: 6px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item) {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  width: fit-content;
  max-width: 100%;
  box-sizing: border-box;
  padding: 0;
  border: none;
  background: transparent;
  box-shadow: none;
  overflow: visible;
  text-decoration: none;
  transition: opacity 0.18s ease;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item:hover),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item:hover) {
  opacity: 0.98;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-single > .guaxi-media-group__item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-single > .guaxi-media-group__item) {
  display: flex;
  flex: 0 1 auto;
  min-width: 0;
  width: fit-content;
  max-width: min(100%, 760px);
  margin-left: auto;
  margin-right: auto;
  padding: 0;
  border: none;
  background: transparent;
  box-shadow: none;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group__image),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__image) {
  display: block;
  width: 100%;
  max-width: 100%;
  max-height: min(56vh, 420px);
  height: auto;
  margin: 0;
  border-radius: 14px;
  object-fit: contain;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-single .guaxi-media-group__image),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-single .guaxi-media-group__image) {
  width: auto;
  max-width: min(100%, 760px);
  max-height: min(72vh, 640px);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item) {
  background: transparent;
  box-shadow: none;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item:hover),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item:hover) {
  opacity: 0.98;
}

@media (max-width: 760px) {
  .guaxi-notebook__markdown-preview :deep(.guaxi-media-group),
  .guaxi-notebook__markdown-preview :deep(.guaxi-media-group.is-gallery),
  .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group),
  .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group.is-gallery) {
    width: 100% !important;
  }

  .guaxi-notebook__markdown-preview :deep(.guaxi-media-group__item),
  .guaxi-notebook__rich.markdown-body :deep(.guaxi-media-group__item) {
    max-width: 100%;
    width: 100%;
  }
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(a),
.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(a:visited),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(a),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card) {
  display: inline-flex;
  max-width: min(360px, 100%);
  vertical-align: top;
  margin: 6px 0;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 75%, transparent 25%);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 86%, var(--guaxi-notebook-panel) 14%);
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.08);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__link),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__link) {
  display: inline-flex;
  flex-direction: column;
  gap: 3px;
  padding: 8px 12px;
  min-width: 0;
  text-decoration: none;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__link:hover),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 92%, var(--guaxi-notebook-panel) 8%);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__eyebrow),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__eyebrow) {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__title),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__title) {
  font-size: 13px;
  font-weight: 700;
  color: var(--guaxi-notebook-text);
  line-height: 1.35;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__excerpt),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__excerpt) {
  font-size: 12px;
  line-height: 1.45;
  color: var(--guaxi-notebook-muted);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__meta),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__meta) {
  font-size: 11px;
  color: var(--guaxi-notebook-muted);
  opacity: 0.9;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 90%, var(--guaxi-notebook-panel) 10%);
  border-color: rgba(148, 163, 184, 0.22);
  box-shadow: 0 12px 28px rgba(2, 6, 23, 0.35);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-preview-file-card__link:hover),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-preview-file-card__link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 96%, var(--guaxi-notebook-panel) 4%);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc) {
  display: grid;
  gap: 10px;
  margin: 6px 0;
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid color-mix(in srgb, var(--guaxi-notebook-strong-border) 78%, transparent 22%);
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 88%, var(--guaxi-notebook-panel) 12%) !important;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-title),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-title) {
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-list),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-list) {
  margin: 0;
  padding-left: 0;
  list-style: none;
  display: grid;
  gap: 6px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item) {
  margin: 0;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-2),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-2) {
  padding-left: 18px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-3),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-3) {
  padding-left: 36px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-4),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-4) {
  padding-left: 54px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-5),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-5) {
  padding-left: 72px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-item.level-6),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-item.level-6) {
  padding-left: 90px;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link) {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 4px 8px;
  border-radius: 9px;
  color: var(--guaxi-notebook-text);
  text-decoration: none;
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 82%, var(--guaxi-notebook-panel-soft) 18%);
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link:hover),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 92%, var(--guaxi-notebook-panel) 8%);
  border-color: color-mix(in srgb, var(--guaxi-notebook-accent, #38bdf8) 28%, transparent 72%);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--guaxi-notebook-accent, #38bdf8) 16%, transparent 84%) inset;
}

.guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-empty),
.guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-empty) {
  font-size: 13px;
  color: var(--guaxi-notebook-muted);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 90%, var(--guaxi-notebook-panel) 10%) !important;
  border-color: rgba(148, 163, 184, 0.24);
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel) 82%, var(--guaxi-notebook-panel-soft) 18%);
  color: #e2e8f0;
}

.guaxi-notebook.theme-dark .guaxi-notebook__markdown-preview :deep(.guaxi-notebook__toc-link:hover),
.guaxi-notebook.theme-dark .guaxi-notebook__rich.markdown-body :deep(.guaxi-notebook__toc-link:hover) {
  background: color-mix(in srgb, var(--guaxi-notebook-panel-soft) 96%, var(--guaxi-notebook-panel) 4%);
  border-color: rgba(74, 202, 219, 0.28);
  box-shadow: 0 0 0 1px rgba(74, 202, 219, 0.16) inset;
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
  .guaxi-notebook__toolbar-shell,
  .guaxi-notebook.is-top-toolbar-visible {
    --guaxi-notebook-toolbar-shell-height: 50px;
  }

  /* 设计说明：
   * 1. 顶部中间工具条承担运行、保存等高频操作，窄视口下仍需要保持横向一排，避免入口被挤成纵向难以扫视。
   * 2. Python Cell 工具条仍保留纵向折叠能力，因为它内容更长，窄屏时改成列布局更容易容纳。
   */
  .guaxi-notebook__toolbar {
    position: relative;
    inset: auto;
    opacity: 1;
    pointer-events: auto;
    transform: none;
    align-items: center;
    flex-direction: row;
  }

  .guaxi-notebook__toolbar-main {
    flex-wrap: nowrap;
    justify-content: center;
    gap: 8px;
  }

  .guaxi-notebook__python-toolbar {
    position: relative;
    inset: auto;
    opacity: 1;
    pointer-events: auto;
    transform: none;
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
