<template>
  <div class="editor-container guaxi-editor">
    <div class="editor-header" ref="editorHeaderRef">
      <div class="tab-bar" ref="tabBarRef" @wheel="handleWheelScroll">
        <div class="tab-region tab-region-left" ref="tabRegionLeftRef">
          <div v-for="tab in leftTabs" :key="tab.path"
            :class="['tab-item', { 'active': fileTreeStore.activeTabLeft === tab.path, 'dirty': !!tab.dirty }]"
            @click="switchTab(tab.path, 'left')" @mousedown.middle="closeTab(tab.path, 'left')">
            <span class="tab-title">{{ tab.fileName }}</span>
            <span class="tab-affordance">
              <span class="tab-dirty-dot" v-show="tab.dirty" />
              <n-icon class="tab-close" size="14" @click.stop="closeTab(tab.path, 'left')">
                <CloseOutline />
              </n-icon>
            </span>
          </div>
        </div>
        <div v-if="layoutMode !== 'single'" class="tab-region tab-region-right" ref="tabRegionRightRef">
          <div v-for="tab in rightTabs" :key="tab.path"
            :class="['tab-item', { 'active': fileTreeStore.activeTabRight === tab.path, 'dirty': !!tab.dirty }]"
            @click="switchTab(tab.path, 'right')" @mousedown.middle="closeTab(tab.path, 'right')">
            <span class="tab-title">{{ tab.fileName }}</span>
            <span class="tab-affordance">
              <span class="tab-dirty-dot" v-show="tab.dirty" />
              <n-icon class="tab-close" size="14" @click.stop="closeTab(tab.path, 'right')">
                <CloseOutline />
              </n-icon>
            </span>
          </div>
        </div>
      </div>
      <div class="toolbar">
        <div v-if="previewMode !== 'realtime'" class="toolbar-editing-actions">
          <n-dropdown trigger="click" :options="headingOptions" @select="handleHeadingSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text size="tiny">
                  <Icon :icon="headingIcon" width="16" height="16" />
                </n-button>
              </template>
              标题
            </n-tooltip>
          </n-dropdown>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBold">
                <Icon :icon="boldIcon" width="16" height="16" />
              </n-button>
            </template>
            加粗 (Ctrl+B)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleItalic">
                <Icon :icon="italicIcon" width="16" height="16" />
              </n-button>
            </template>
            斜体 (Ctrl+I)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleStrikethrough">
                <Icon :icon="strikethroughIcon" width="16" height="16" />
              </n-button>
            </template>
            删除线 (Ctrl+Shift+X)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertLink">
                <Icon :icon="linkIcon" width="16" height="16" />
              </n-button>
            </template>
            链接 (Ctrl+K)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBlockquote">
                <Icon :icon="quoteIcon" width="16" height="16" />
              </n-button>
            </template>
            引用
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleInlineCode">
                <Icon :icon="codeIcon" width="16" height="16" />
              </n-button>
            </template>
            行内代码
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertCodeBlockFromToolbar">
                <Icon :icon="squareTerminalIcon" width="16" height="16" />
              </n-button>
            </template>
            代码块
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleUnorderedList">
                <Icon :icon="listIcon" width="16" height="16" />
              </n-button>
            </template>
            无序列表
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleOrderedList">
                <Icon :icon="listOrderedIcon" width="16" height="16" />
              </n-button>
            </template>
            有序列表
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleTaskList">
                <Icon :icon="listChecksIcon" width="16" height="16" />
              </n-button>
            </template>
            任务列表
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertTable">
                <Icon :icon="tableIcon" width="16" height="16" />
              </n-button>
            </template>
            表格 (Ctrl+Alt+T)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertImage">
                <Icon :icon="imageIcon" width="16" height="16" />
              </n-button>
            </template>
            图片
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertHorizontalRule">
                <Icon :icon="minusIcon" width="16" height="16" />
              </n-button>
            </template>
            分隔线
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="undo">
                <Icon :icon="undoIcon" width="16" height="16" />
              </n-button>
            </template>
            撤销
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="redo">
                <Icon :icon="redoIcon" width="16" height="16" />
              </n-button>
            </template>
            重做
          </n-tooltip>
        </div>
        <div class="toolbar-spacer"></div>
        <div class="toolbar-actions">
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text @click="toggleVimMode">
                <n-icon :component="vimMode ? LogoVimeo : Edit16Filled" width="16" height="16" />
              </n-button>
            </template>
            {{ vimMode ? '禁用Vim模式' : '启用Vim模式' }}
          </n-tooltip>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text @click="saveContent">
                <n-icon :component="SaveOutline" width="16" height="16" />
              </n-button>
            </template>
            保存 (Ctrl+S)
          </n-tooltip>

          <n-dropdown trigger="click" :options="previewModeOptions" @select="handlePreviewModeSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text>
                  <Icon :icon="previewModeIcon" width="16" height="16" />
                </n-button>
              </template>
              {{ previewModeLabel }} (Ctrl+1/2/3/4)
            </n-tooltip>
          </n-dropdown>

          <n-dropdown trigger="click" :options="exportOptions" @select="handleExportSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text>
                  <n-icon :component="DownloadOutline" width="16" height="16" />
                </n-button>
              </template>
              导出
            </n-tooltip>
          </n-dropdown>
        </div>
      </div>
    </div>

    <div class="editor-content" :class="previewMode">
      <div v-if="!selectedFile" class="editor-empty">请选择一个文件以开始编辑</div>
      <div v-show="selectedFile && previewMode === 'realtime' && canPreview" id="realtime-vditor"
        class="realtime-vditor">
      </div>
      <div v-show="previewMode !== 'realtime' && showPreview" v-html="renderedHtml" class="preview"
        ref="previewContainer" @click="handlePreviewClick"></div>
      <div v-show="previewMode !== 'realtime' && showEditor" class="editor-wrapper" ref="editorWrapperRef">
        <div v-show="tableToolbarVisible" class="floating-table-toolbar" :style="tableToolbarStyle" @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
                <n-button text size="tiny" @click="addTableRowBelow">
                <Icon :icon="tableRowPlusAfter" width="16" height="16" />
              </n-button>
            </template>
            新增行 (Ctrl+Alt+↓)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="deleteTableRow">
                <Icon :icon="tableRowRemove" width="16" height="16" />
              </n-button>
            </template>
            删除行 (Ctrl+Alt+Shift+↓)
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="addTableColumnRight">
                <Icon :icon="tableColumnPlusAfter" width="16" height="16" />
              </n-button>
            </template>
            新增列 (Ctrl+Alt+→)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="deleteTableColumn">
                <Icon :icon="tableColumnRemove" width="16" height="16" />
              </n-button>
            </template>
            删除列 (Ctrl+Alt+Shift+→)
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
                <n-button text size="tiny" @click="setTableColumnAlign('left')">
                <Icon :icon="alignLeftIcon" width="16" height="16" />
              </n-button>
            </template>
            左对齐
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
                <n-button text size="tiny" @click="setTableColumnAlign('center')">
                <Icon :icon="alignCenterIcon" width="16" height="16" />
              </n-button>
            </template>
            居中
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
                <n-button text size="tiny" @click="setTableColumnAlign('right')">
                <Icon :icon="alignRightIcon" width="16" height="16" />
              </n-button>
            </template>
            右对齐
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
                <n-button text size="tiny" @click="formatTable">
                <Icon :icon="sparklesIcon" width="16" height="16" />
              </n-button>
            </template>
            格式化
          </n-tooltip>
        </div>
        <div v-show="floatingToolbarVisible" class="floating-md-toolbar" :style="floatingToolbarStyle"
          @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBold">
                  <Icon :icon="boldIcon" width="16" height="16" />
              </n-button>
            </template>
            加粗 (Ctrl+B)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleItalic">
                  <Icon :icon="italicIcon" width="16" height="16" />
              </n-button>
            </template>
            斜体 (Ctrl+I)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleStrikethrough">
                  <Icon :icon="strikethroughIcon" width="16" height="16" />
              </n-button>
            </template>
            删除线 (Ctrl+Shift+X)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertLink">
                  <Icon :icon="linkIcon" width="16" height="16" />
              </n-button>
            </template>
            链接 (Ctrl+K)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleInlineCode">
                  <Icon :icon="codeIcon" width="16" height="16" />
              </n-button>
            </template>
            行内代码
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBlockquote">
                 <Icon :icon="quoteIcon" width="16" height="16" />
              </n-button>
            </template>
            引用
          </n-tooltip>
        </div>
        <div class="editor-split-wrapper" :class="[layoutMode, { 'vim-enabled': vimMode }]">
          <div ref="editorContainer" class="editor-container" @mousedown="activateRegion('left')">
            <!-- 编辑器内容将在这里渲染 -->
          </div>
          <div v-if="layoutMode !== 'single'" ref="secondaryEditorContainer" class="editor-container"
            @mousedown="activateRegion('right')">
          </div>
        </div>

        <n-dropdown v-model:show="editorContextMenuVisible" :options="editorContextMenuOptions" placement="bottom-start"
          :x="editorContextMenuPosition.x" :y="editorContextMenuPosition.y" @select="handleEditorContextMenuSelect"
          @clickoutside="handleEditorContextMenuClickoutside" />

        <div v-if="vimMode" class="vim-status-bar" ref="vimStatusBar"></div>
        <GuaxiEditorStatusBar class="editor-status-bar" />
      </div>

    </div>
    <!-- 添加 GitPanel 组件 -->
    <!--
    <GitPanel ref="gitPanel" :workspace-path="fileTreeStore.activeRepoPath" />
    -->
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted, nextTick, provide, h } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'
import { useSettingsStore } from '../../stores/settingsStore'
import GuaxiEditorStatusBar from './GuaxiEditorStatusBar.vue'
import { NTooltip, NIcon, useMessage, NDropdown } from 'naive-ui'
import { Icon } from '@iconify/vue'
import { EyeOutline, LogoVimeo, CloseOutline, DownloadOutline, SaveOutline, ArrowBackOutline, ArrowForwardOutline } from '@vicons/ionicons5'
import { DocumentEdit16Regular, Edit16Filled, Image28Regular } from '@vicons/fluent'
// Iconify local icon imports (lucide + mdi) to preserve original visuals without runtime network fetch
import headingIcon from '@iconify-icons/lucide/heading'
import boldIcon from '@iconify-icons/lucide/bold'
import italicIcon from '@iconify-icons/lucide/italic'
import strikethroughIcon from '@iconify-icons/lucide/strikethrough'
import linkIcon from '@iconify-icons/lucide/link'
import quoteIcon from '@iconify-icons/lucide/quote'
import codeIcon from '@iconify-icons/lucide/code'
import squareTerminalIcon from '@iconify-icons/lucide/terminal-square'
import listIcon from '@iconify-icons/lucide/list'
import listOrderedIcon from '@iconify-icons/lucide/list-ordered'
import listChecksIcon from '@iconify-icons/lucide/list-checks'
import tableIcon from '@iconify-icons/lucide/table'
import imageIcon from '@iconify-icons/lucide/image'
import minusIcon from '@iconify-icons/lucide/minus'
import undoIcon from '@iconify-icons/lucide/undo'
import redoIcon from '@iconify-icons/lucide/redo'
import alignLeftIcon from '@iconify-icons/lucide/align-left'
import alignCenterIcon from '@iconify-icons/lucide/align-center'
import alignRightIcon from '@iconify-icons/lucide/align-right'
import sparklesIcon from '@iconify-icons/lucide/sparkles'
import filePenLineIcon from '@iconify-icons/lucide/file-edit'
import eyeIcon from '@iconify-icons/lucide/eye'
import columns2Icon from '@iconify-icons/lucide/columns'

import tableRowPlusAfter from '@iconify-icons/mdi/table-row-plus-after'
import tableRowRemove from '@iconify-icons/mdi/table-row-remove'
import tableColumnPlusAfter from '@iconify-icons/mdi/table-column-plus-after'
import tableColumnRemove from '@iconify-icons/mdi/table-column-remove'
import { dirname, homeDir } from '@tauri-apps/api/path'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import Vditor from 'vditor'
import 'vditor/dist/index.css'
import 'vditor/dist/method'
import { debounce } from 'lodash-es'
import * as monaco from 'monaco-editor'
import { initVimMode } from 'monaco-vim';
import { initTextmate } from '../../monaco/textmateLoader'
import { initLspClient } from '../../monaco/lspClient'
// 更新所有语言导入路径
import 'monaco-editor/esm/vs/basic-languages/python/python.contribution'
import 'monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution'
import 'monaco-editor/esm/vs/basic-languages/rust/rust.contribution'
import 'monaco-editor/esm/vs/basic-languages/shell/shell.contribution'
import 'monaco-editor/esm/vs/basic-languages/html/html.contribution'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/css/css.contribution'
import 'monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'
import 'monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution'
import 'monaco-editor/esm/vs/basic-languages/java/java.contribution'
import 'monaco-editor/esm/vs/basic-languages/lua/lua.contribution'
import 'monaco-editor/esm/vs/basic-languages/ini/ini.contribution'

import 'monaco-editor/esm/vs/editor/contrib/clipboard/browser/clipboard.js'
import 'monaco-editor/esm/vs/editor/standalone/browser/quickAccess/standaloneCommandsQuickAccess.js'

import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import { GitBranchOutline } from '@vicons/ionicons5'
import GitPanel from './GitPanel.vue'
import GitService from './utils/GitService'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { save, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'

const outlineStore = useOutlineStore()
// 添加 GitPanel 组件引用
const gitPanel = ref(null)


// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const settingsStore = useSettingsStore()
const mode = ref('edit');
const previewMode = ref('toggle');
const layoutMode = computed(() => fileTreeStore.layoutMode || 'single');
const renderedHtml = ref('');
const lastPreviewRenderedSource = ref('');
const previewContainer = ref(null);
const editorContainer = ref(null);
const secondaryEditorContainer = ref(null);
const editorWrapperRef = ref(null);
const editorHeaderRef = ref(null);
const floatingToolbarVisible = ref(false);
const floatingToolbarStyle = ref({ left: '0px', top: '0px' });

const editorContextMenuVisible = ref(false);
const editorContextMenuPosition = ref({ x: 0, y: 0 });
const editorContextMenuOptions = ref([]);

const handleEditorContextMenuClickoutside = () => {
  editorContextMenuVisible.value = false;
};

const writeClipboardText = async (text) => {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
};

const readClipboardText = async () => {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return null;
  }
};

const clipboardCopy = async () => {
  if (!editor) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection || selection.isEmpty()) return;
  const text = model.getValueInRange(selection);
  if (!text) return;
  const ok = await writeClipboardText(text);
  if (!ok) message.warning('复制失败：无剪贴板权限');
};

const clipboardCut = async () => {
  if (!editor) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection || selection.isEmpty()) return;

  const text = model.getValueInRange(selection);
  if (!text) return;

  const ok = await writeClipboardText(text);
  if (!ok) {
    message.warning('剪切失败：无剪贴板权限');
    return;
  }

  editor.executeEdits('clipboard-cut', [
    {
      range: selection,
      text: '',
      forceMoveMarkers: true
    }
  ]);
};

const clipboardPaste = async () => {
  if (!editor) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection) return;

  const text = await readClipboardText();
  if (text == null) {
    message.warning('粘贴失败：无剪贴板权限');
    return;
  }

  editor.executeEdits('clipboard-paste', [
    {
      range: selection,
      text,
      forceMoveMarkers: true
    }
  ]);
};

const runEditorAction = async (actionId) => {
  if (!editor) return;
  editor.focus();

  if (actionId === 'editor.action.clipboardCopyAction') {
    const action = editor.getAction?.(actionId);
    if (action) {
      await action.run();
      return;
    }
    await clipboardCopy();
    return;
  }

  if (actionId === 'editor.action.clipboardCutAction') {
    const action = editor.getAction?.(actionId);
    if (action) {
      await action.run();
      return;
    }
    await clipboardCut();
    return;
  }

  if (actionId === 'editor.action.clipboardPasteAction') {
    const action = editor.getAction?.(actionId);
    if (action) {
      await action.run();
      return;
    }
    await clipboardPaste();
    return;
  }

  const action = editor.getAction?.(actionId);
  if (action) {
    await action.run();
    return;
  }

  try {
    editor.trigger('contextmenu', actionId, null);
  } catch {
    message.warning('该命令当前不可用');
  }
};

const buildEditorContextMenuOptions = () => {
  const model = editor?.getModel?.();
  const isMarkdown = (model?.getLanguageId?.() || '') === 'markdown';
  const selection = editor?.getSelection?.();
  const hasSelection = !!selection && !selection.isEmpty();

  return [
    { label: '剪切', key: 'editor.action.clipboardCutAction' },
    { label: '复制', key: 'editor.action.clipboardCopyAction' },
    { label: '粘贴', key: 'editor.action.clipboardPasteAction' },
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
  ];
};

const handleEditorContextMenuSelect = async (key) => {
  editorContextMenuVisible.value = false;

  if (key === 'guaxi.markdown.formatDocument') {
    await formatMarkdownDocument();
    return;
  }

  if (key === 'guaxi.markdown.formatSelection') {
    await formatMarkdownSelection();
    return;
  }

  if (key === 'guaxi.edit.navigateBack') {
    await navigateToPreviousEditLocation();
    return;
  }

  if (key === 'guaxi.edit.navigateForward') {
    await navigateToNextEditLocation();
    return;
  }

  await runEditorAction(key);
};
const tableToolbarVisible = ref(false);
const tableToolbarStyle = ref({ left: '0px', top: '0px' });
const activeTableContext = ref(null);
const scrollRatio = ref(0);
const pendingPreviewScrollRestore = ref(false);
const pendingPreviewScrollLine = ref(null);
const pendingRealtimeScrollRestore = ref(false);
const realtimeVditorId = 'realtime-vditor';
const realtimeVditorReady = ref(false);
const pendingRealtimeVditorValue = ref(null);
let realtimeVditor = null;
let settingRealtimeVditorValue = false;
const syncRealtimeValueToRawContent = debounce((value) => {
  rawContent.value = value;
  const path = fileTreeStore.activeTab;
  if (path) fileTreeStore.updateTabContent(path, value);
}, 120);
let editor = null;
let secondaryEditor = null;
let editorEffectsLeft = null;
let editorEffectsRight = null;

const createEditorEffects = (targetEditor) => {
  const host = targetEditor?.getDomNode?.();
  if (!host) return null;

  const overlay = document.createElement('div');
  overlay.className = 'guaxi-editor-effects';

  const canvas = document.createElement('canvas');
  canvas.className = 'guaxi-editor-effects-canvas';

  const cursor = document.createElement('div');
  cursor.className = 'guaxi-editor-smooth-cursor';

  overlay.appendChild(canvas);
  overlay.appendChild(cursor);
  host.appendChild(overlay);

  const ctx = canvas.getContext('2d');
  const palette = [
    'rgba(255, 208, 0, 0.95)',
    'rgba(255, 105, 180, 0.95)',
    'rgba(0, 220, 255, 0.95)',
    'rgba(140, 255, 120, 0.95)',
    'rgba(255, 90, 90, 0.95)',
    'rgba(180, 120, 255, 0.95)'
  ];

  const state = {
    host,
    overlay,
    canvas,
    ctx,
    dpr: 1,
    w: 0,
    h: 0,
    particles: [],
    raf: 0,
    lastTs: 0,
    cx: 0,
    cy: 0,
    tx: 0,
    ty: 0,
    ch: 18,
    tch: 18,
    cursorVisible: false,
    ro: null,
    resizeHandler: null
  };

  const resize = () => {
    const rect = host.getBoundingClientRect();
    const w = Math.max(1, Math.floor(rect.width));
    const h = Math.max(1, Math.floor(rect.height));
    const dpr = window.devicePixelRatio || 1;

    if (w === state.w && h === state.h && dpr === state.dpr) return;

    state.w = w;
    state.h = h;
    state.dpr = dpr;

    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    canvas.width = Math.max(1, Math.floor(w * dpr));
    canvas.height = Math.max(1, Math.floor(h * dpr));

    if (ctx) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
  };

  const step = (ts) => {
    const dt = Math.min(0.034, Math.max(0, (ts - (state.lastTs || ts)) / 1000));
    state.lastTs = ts;

    resize();

    const dx = state.tx - state.cx;
    const dy = state.ty - state.cy;
    const dh = state.tch - state.ch;

    const follow = 1 - Math.pow(1 - 0.35, dt * 60);
    state.cx += dx * follow;
    state.cy += dy * follow;
    state.ch += dh * follow;

    if (state.cursorVisible) {
      cursor.style.opacity = '1';
      cursor.style.height = `${Math.max(8, state.ch)}px`;
      cursor.style.transform = `translate3d(${state.cx}px, ${state.cy}px, 0)`;
    } else {
      cursor.style.opacity = '0';
    }

    const g = 900;
    const drag = Math.pow(0.04, dt);

    if (state.particles.length) {
      for (let i = state.particles.length - 1; i >= 0; i -= 1) {
        const p = state.particles[i];
        p.life -= dt;
        if (p.life <= 0) {
          state.particles.splice(i, 1);
          continue;
        }
        p.vy += g * dt;
        p.vx *= drag;
        p.vy *= drag;
        p.x += p.vx * dt;
        p.y += p.vy * dt;
      }
    }

    if (ctx) {
      ctx.clearRect(0, 0, state.w, state.h);
      if (state.particles.length) {
        ctx.globalCompositeOperation = 'lighter';
        for (let i = 0; i < state.particles.length; i += 1) {
          const p = state.particles[i];
          const a = Math.max(0, Math.min(1, p.life / p.ttl));
          ctx.globalAlpha = a;
          ctx.fillStyle = p.color;
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
          ctx.fill();
        }
        ctx.globalAlpha = 1;
        ctx.globalCompositeOperation = 'source-over';
      }
    }

    // When there's nothing to animate, stop the RAF loop to reduce idle overhead.
    // 1) no particles and cursor not visible -> stop
    // 2) no particles and cursor already very close to target -> stop
    if (!state.particles.length) {
      if (!state.cursorVisible) {
        state.raf = 0;
        return;
      }

      const rdx = state.tx - state.cx;
      const rdy = state.ty - state.cy;
      const rdh = state.tch - state.ch;

      // Pixel-level settle threshold (CSS pixels)
      const settlePosEps = 0.25;
      const settleHeightEps = 0.25;
      const settled =
        Math.abs(rdx) < settlePosEps &&
        Math.abs(rdy) < settlePosEps &&
        Math.abs(rdh) < settleHeightEps;

      if (settled) {
        // Snap to target so the last rendered frame is exact.
        state.cx = state.tx;
        state.cy = state.ty;
        state.ch = state.tch;

        cursor.style.opacity = '1';
        cursor.style.height = `${Math.max(8, state.ch)}px`;
        cursor.style.transform = `translate3d(${state.cx}px, ${state.cy}px, 0)`;

        state.raf = 0;
        return;
      }
    }

    state.raf = requestAnimationFrame(step);
  };

  const ensureLoop = () => {
    if (state.raf) return;
    state.lastTs = performance.now();
    state.raf = requestAnimationFrame(step);
  };

  const updateCursorTarget = () => {
    const model = targetEditor.getModel();
    const pos = targetEditor.getPosition();
    if (!model || !pos) {
      state.cursorVisible = false;
      ensureLoop();
      return;
    }

    const visible = targetEditor.getScrolledVisiblePosition(pos);
    if (!visible) {
      state.cursorVisible = false;
      ensureLoop();
      return;
    }

    state.cursorVisible = true;
    state.tx = visible.left;
    state.ty = visible.top;
    state.tch = visible.height || 18;
    ensureLoop();
  };

  const spawnExplosionAtCursor = (text) => {
    const ch = text ? text[text.length - 1] : '';
    if (!ch || ch === '\n' || ch === '\r' || ch === '\t') return;

    const model = targetEditor.getModel();
    const pos = targetEditor.getPosition();
    if (!model || !pos) return;

    const visible = targetEditor.getScrolledVisiblePosition(pos);
    if (!visible) return;

    const x = visible.left;
    const y = visible.top + (visible.height || 18) * 0.7;

    const n = Math.min(48, 18 + Math.floor(Math.random() * 10));
    for (let i = 0; i < n; i += 1) {
      const a = Math.random() * Math.PI * 2;
      const s = 80 + Math.random() * 340;
      state.particles.push({
        x,
        y,
        vx: Math.cos(a) * s,
        vy: Math.sin(a) * s - (120 + Math.random() * 160),
        size: 0.8 + Math.random() * 2.2,
        ttl: 0.55 + Math.random() * 0.35,
        life: 0.55 + Math.random() * 0.35,
        color: palette[(Math.random() * palette.length) | 0]
      });
    }

    ensureLoop();
  };

  resize();

  if (typeof ResizeObserver !== 'undefined') {
    state.ro = new ResizeObserver(() => resize());
    state.ro.observe(host);
  } else {
    state.resizeHandler = () => resize();
    window.addEventListener('resize', state.resizeHandler);
  }

  // Don't start RAF loop until there's something to animate (cursor move / particles).
  return {
    updateCursorTarget,
    spawnExplosionAtCursor,
    destroy() {
      if (state.raf) cancelAnimationFrame(state.raf);
      state.raf = 0;
      if (state.ro) state.ro.disconnect();
      state.ro = null;
      if (state.resizeHandler) {
        window.removeEventListener('resize', state.resizeHandler);
        state.resizeHandler = null;
      }
      if (state.overlay?.parentNode) state.overlay.parentNode.removeChild(state.overlay);
      state.particles = [];
      state.cursorVisible = false;
    }
  };
};

let initEditorPending = false;

const editLocationBackStack = ref([]);
const editLocationForwardStack = ref([]);
let lastRecordedEditLocationKey = null;
let lastRecordedEditLocationAt = 0;
let suppressEditLocationRecording = false;
const maxEditLocationHistory = 200;

const getCurrentEditLocation = () => {
  if (!editor) return null;
  const path = fileTreeStore.activeTab || selectedFile.value?.path;
  const position = editor.getPosition();
  if (!path || !position) return null;
  return {
    path,
    lineNumber: position.lineNumber,
    column: position.column
  };
};

const isSameEditLocation = (a, b) => {
  if (!a || !b) return false;
  return a.path === b.path && a.lineNumber === b.lineNumber && a.column === b.column;
};

const recordEditLocationFromEditor = () => {
  if (!editor) return;
  if (suppressEditLocationRecording) return;

  const loc = getCurrentEditLocation();
  if (!loc) return;

  const now = Date.now();
  const key = `${loc.path}:${loc.lineNumber}:${loc.column}`;
  if (key === lastRecordedEditLocationKey && now - lastRecordedEditLocationAt < 500) return;

  const back = editLocationBackStack.value;
  const last = back[back.length - 1];
  if (last && isSameEditLocation(last, loc)) {
    lastRecordedEditLocationKey = key;
    lastRecordedEditLocationAt = now;
    return;
  }

  back.push({ ...loc, ts: now });
  if (back.length > maxEditLocationHistory) back.shift();
  editLocationForwardStack.value = [];

  lastRecordedEditLocationKey = key;
  lastRecordedEditLocationAt = now;
};

const goToEditLocation = async (loc) => {
  if (!loc) return false;

  suppressEditLocationRecording = true;
  try {
    if (fileTreeStore.activeTab !== loc.path) {
      await fileTreeStore.openFileInTab(loc.path);
    }

    await nextTick();

    if (!editor) return false;
    const pos = new monaco.Position(loc.lineNumber, loc.column);
    editor.focus();
    editor.setPosition(pos);
    editor.revealPositionInCenterIfOutsideViewport(pos);
    return true;
  } finally {
    suppressEditLocationRecording = false;
  }
};

const navigateToPreviousEditLocation = async () => {
  if (!editor) return;

  const current = getCurrentEditLocation();
  if (!current) return;

  const back = editLocationBackStack.value;
  const forward = editLocationForwardStack.value;

  if (back.length === 0) {
    back.push({ ...current, ts: Date.now() });
    return;
  }

  const last = back[back.length - 1];
  if (!isSameEditLocation(last, current)) {
    forward.push({ ...current, ts: Date.now() });
    await goToEditLocation(last);
    return;
  }

  if (back.length < 2) {
    message.info('没有上一个编辑位置');
    return;
  }

  const popped = back.pop();
  forward.push(popped);
  const target = back[back.length - 1];
  await goToEditLocation(target);
};

const navigateToNextEditLocation = async () => {
  if (!editor) return;

  const forward = editLocationForwardStack.value;
  if (forward.length === 0) {
    return;
  }

  const target = forward.pop();
  const back = editLocationBackStack.value;
  const last = back[back.length - 1];
  if (!last || !isSameEditLocation(last, target)) {
    back.push(target);
    if (back.length > maxEditLocationHistory) back.shift();
  }

  await goToEditLocation(target);
};

const selectedFile = computed(() => fileTreeStore.selectedFile)
// 定义 rawContent 变量
const rawContent = ref(selectedFile.value?.content || '')
// 1. 获取当前激活tab对象和内容
const activeTab = computed(() => fileTreeStore.openTabs.find(tab => tab.path === fileTreeStore.activeTab));
const activeTabContent = computed(() => activeTab.value?.content || '');

const leftTabs = computed(() => {
  if (layoutMode.value === 'single' || fileTreeStore.splitIndex == null) {
    return fileTreeStore.openTabs;
  }
  return fileTreeStore.openTabs.slice(0, fileTreeStore.splitIndex);
});

const rightTabs = computed(() => {
  if (layoutMode.value === 'single' || fileTreeStore.splitIndex == null) {
    return [];
  }
  if (fileTreeStore.openTabs.length === 1) {
    return fileTreeStore.openTabs;
  }
  return fileTreeStore.openTabs.slice(fileTreeStore.splitIndex);
});


const message = useMessage()

let unlistenTemplateInsert = null
let templateInsertListenerUnmounted = false

const safeUnlistenTemplateInsert = () => {
  if (typeof unlistenTemplateInsert !== 'function') return
  try {
    Promise.resolve(unlistenTemplateInsert()).catch((e) => {
      console.debug('反注册模板插入事件失败:', e)
    })
  } catch (e) {
    console.debug('反注册模板插入事件失败:', e)
  }
  unlistenTemplateInsert = null
}

onMounted(async () => {
  templateInsertListenerUnmounted = false
  try {
    const unlisten = await listen('template_insert_event', async (event) => {
      const content = typeof event?.payload?.content === 'string' ? event.payload.content : ''
      if (!content) return

      const text = content.endsWith('\n') ? content : `${content}\n`
      const inserted = await editorStore.insertContent(text)
      if (!inserted) {
        message.warning('插入模板失败：请先打开一个文件')
      }
    })

    if (templateInsertListenerUnmounted) {
      try {
        Promise.resolve(unlisten()).catch((e) => {
          console.debug('反注册模板插入事件失败:', e)
        })
      } catch (e) {
        console.debug('反注册模板插入事件失败:', e)
      }
      return
    }

    unlistenTemplateInsert = unlisten
  } catch (e) {
    console.error('注册模板插入事件失败:', e)
  }
})

onUnmounted(() => {
  templateInsertListenerUnmounted = true
  safeUnlistenTemplateInsert()
})

const headingOptions = [
  { label: '正文', key: 0 },
  { label: 'H1', key: 1 },
  { label: 'H2', key: 2 },
  { label: 'H3', key: 3 },
  { label: 'H4', key: 4 },
  { label: 'H5', key: 5 },
  { label: 'H6', key: 6 }
]

const vimMode = ref(false);
const vimStatusBar = ref(null);
let vimModeInstance = null;

const applyVimOverlayInsets = () => {
  const baseTop = 8;
  const baseBottom = 8;
  const vimH = vimMode.value
    ? Math.ceil(vimStatusBar.value?.getBoundingClientRect?.()?.height || 0)
    : 0;
  const bottom = baseBottom + (vimH ? vimH + 6 : 0);

  const opt = {
    padding: {
      top: baseTop,
      bottom
    }
  };

  if (editor) {
    editor.updateOptions(opt);
    editor.layout();
  }
  if (secondaryEditor) {
    secondaryEditor.updateOptions(opt);
    secondaryEditor.layout();
  }
};

let markdownFeaturesRegistered = false;

let filesystemPathCompletionRegistered = false;
const filesystemPathCompletionCache = new Map();
const filesystemPathCompletionInflight = new Map();
let filesystemPathCompletionDocPath = '';
let filesystemPathCompletionDocDir = '';
let filesystemPathCompletionHomeDir = '';

const normalizeFsPath = (p) => {
  let s = (p || '').replace(/\\/g, '/');
  // Fix Windows drive path like `/C:/...` which may break relative completion like `../`
  if (/^\/[a-zA-Z]:\//.test(s)) {
    s = s.slice(1);
  }
  return s;
};

const joinFsPath = (base, sub) => {
  const a = normalizeFsPath(base).replace(/\/+$/, '');
  const b = normalizeFsPath(sub).replace(/^\/+/, '');
  if (!a) return b;
  if (!b) return a;
  return `${a}/${b}`;
};

const getFilesystemCompletionDocDir = async () => {
  const p = selectedFile.value?.path || '';
  if (!p) return fileTreeStore.activeRepoPath || '';
  if (p === filesystemPathCompletionDocPath && filesystemPathCompletionDocDir) {
    return filesystemPathCompletionDocDir;
  }
  filesystemPathCompletionDocPath = p;
  try {
    filesystemPathCompletionDocDir = await dirname(p);
  } catch {
    filesystemPathCompletionDocDir = fileTreeStore.activeRepoPath || '';
  }
  return filesystemPathCompletionDocDir;
};

const getFilesystemCompletionHomeDir = async () => {
  if (filesystemPathCompletionHomeDir) return filesystemPathCompletionHomeDir;
  try {
    filesystemPathCompletionHomeDir = await homeDir();
  } catch {
    filesystemPathCompletionHomeDir = '';
  }
  return filesystemPathCompletionHomeDir;
};

const isFilesystemPathContext = (beforeCursor) => {
  const s = beforeCursor || '';
  if (/\]\(\s*$/.test(s)) return true;
  if (/!\[[^\]]*\]\(\s*$/.test(s)) return true;
  if (/(src|href)=['"]\s*$/.test(s)) return true;
  if (/\(\s*$/.test(s)) return true;
  return false;
};

const extractPathToken = (beforeCursor) => {
  if (!beforeCursor) return '';
  let i = beforeCursor.length - 1;
  while (i >= 0) {
    const ch = beforeCursor[i];
    if (
      /\s/.test(ch) ||
      ch === '"' ||
      ch === "'" ||
      ch === ')' ||
      ch === '(' ||
      ch === '[' ||
      ch === ']' ||
      ch === '<' ||
      ch === '>' ||
      ch === '`' ||
      ch === '{' ||
      ch === '}'
    ) {
      break;
    }
    i -= 1;
  }
  return beforeCursor.slice(i + 1);
};

const splitTokenDirAndPartial = (token) => {
  if (!token) return { dirPrefix: '', partial: '' };
  if (token === '~') return { dirPrefix: '~/', partial: '' };

  const lastSlash = Math.max(token.lastIndexOf('/'), token.lastIndexOf('\\'));
  if (lastSlash >= 0) {
    return {
      dirPrefix: token.slice(0, lastSlash + 1),
      partial: token.slice(lastSlash + 1)
    };
  }
  if (/^[a-zA-Z]:$/.test(token)) {
    return { dirPrefix: `${token}/`, partial: '' };
  }
  return { dirPrefix: '', partial: token };
};

const listDirCached = async (dirPath) => {
  const key = normalizeFsPath(dirPath);
  const now = Date.now();
  const cached = filesystemPathCompletionCache.get(key);
  if (cached && now - cached.ts < 2000) return cached.nodes;

  const inflight = filesystemPathCompletionInflight.get(key);
  if (inflight) return inflight;

  const p = invoke('get_file_tree', { path: key })
    .then((nodes) => {
      const safe = Array.isArray(nodes) ? nodes : [];
      filesystemPathCompletionCache.set(key, { ts: Date.now(), nodes: safe });
      return safe;
    })
    .catch(() => [])
    .finally(() => {
      filesystemPathCompletionInflight.delete(key);
    });

  filesystemPathCompletionInflight.set(key, p);
  return p;
};

const registerFilesystemPathCompletionFeatures = () => {
  if (filesystemPathCompletionRegistered) return;
  filesystemPathCompletionRegistered = true;

  const languages = [
    'markdown',
    'javascript',
    'typescript',
    'python',
    'cpp',
    'rust',
    'shell',
    'html',
    'css',
    'yaml',
    'java',
    'lua',
    'ini',
    'json'
  ];

  languages.forEach((language) => {
    monaco.languages.registerCompletionItemProvider(language, {
      triggerCharacters: ['/', '\\', '.', ':'],
      async provideCompletionItems(model, position) {
        const lineContent = model.getLineContent(position.lineNumber);
        const beforeCursor = lineContent.slice(0, position.column - 1);
        const token = extractPathToken(beforeCursor);
        const pathContext = isFilesystemPathContext(beforeCursor);

        const looksLikePath =
          (!!token &&
            (/[\\/]/.test(token) ||
              token.startsWith('.') ||
              token.startsWith('~') ||
              /^[a-zA-Z]:/.test(token))) ||
          pathContext;
        if (!looksLikePath) return { suggestions: [] };

        const replaceRange = new monaco.Range(
          position.lineNumber,
          Math.max(1, position.column - (token ? token.length : 0)),
          position.lineNumber,
          position.column
        );

        // Align with nvim-cmp: typing '.' / '..' should suggest './' / '../'
        if (token === '.' || token === '..') {
          const suggestions = [];
          if (token === '.') {
            suggestions.push(
              {
                label: './',
                kind: monaco.languages.CompletionItemKind.Folder,
                insertText: './',
                range: replaceRange,
                detail: '当前目录'
              },
              {
                label: '../',
                kind: monaco.languages.CompletionItemKind.Folder,
                insertText: '../',
                range: replaceRange,
                detail: '父目录'
              }
            );
          } else {
            suggestions.push({
              label: '../',
              kind: monaco.languages.CompletionItemKind.Folder,
              insertText: '../',
              range: replaceRange,
              detail: '父目录'
            });
          }
          return { suggestions };
        }

        const { dirPrefix, partial } = splitTokenDirAndPartial(token);

        const tokenStartsAbsolute =
          /^[a-zA-Z]:[\\/]/.test(token) || token.startsWith('/');
        const prefixIsAbsolute =
          /^[a-zA-Z]:[\\/]/.test(dirPrefix) || dirPrefix.startsWith('/');
        const isHomePrefix = dirPrefix.startsWith('~/') || dirPrefix.startsWith('~\\');

        let dirPath = dirPrefix;
        if (!dirPath) {
          dirPath = tokenStartsAbsolute ? '' : await getFilesystemCompletionDocDir();
        } else if (isHomePrefix) {
          const base = await getFilesystemCompletionHomeDir();
          if (!base) return { suggestions: [] };
          dirPath = joinFsPath(base, dirPrefix.slice(2));
          if (!dirPrefix.endsWith('/') && !dirPrefix.endsWith('\\')) {
            dirPath = `${dirPath}/`;
          }
        } else if (!prefixIsAbsolute) {
          const base = await getFilesystemCompletionDocDir();
          dirPath = joinFsPath(base, dirPath);
        }

        dirPath = normalizeFsPath(dirPath);
        if (!dirPath) return { suggestions: [] };

        if (tokenStartsAbsolute && !dirPrefix && token === '/') {
          dirPath = '/';
        }

        const nodes = await listDirCached(dirPath);
        if (!nodes.length) return { suggestions: [] };

        const lowerPartial = (partial || '').toLowerCase();
        const matching = nodes
          .filter((n) => {
            const name = String(n?.name || '');
            if (!name) return false;
            if (!lowerPartial) return true;
            return name.toLowerCase().startsWith(lowerPartial);
          })
          .slice(0, 50);

        if (!matching.length) {
          console.debug('[PathCompletion] no match', {
            token,
            dirPrefix,
            partial,
            dirPath,
            nodeCount: nodes.length
          });
          return { suggestions: [] };
        }

        const partialLen = (partial || '').length;
        const replaceRangeForPartial = new monaco.Range(
          position.lineNumber,
          Math.max(1, position.column - partialLen),
          position.lineNumber,
          position.column
        );

        console.debug('[PathCompletion] suggest', {
          token,
          dirPrefix,
          partial,
          dirPath,
          nodeCount: nodes.length,
          matchCount: matching.length
        });

        const suggestions = matching.map((n) => {
          const name = String(n?.name || '');
          const isDir = !!n?.is_directory;
          const suffix = isDir ? '/' : '';

          return {
            label: `${name}${suffix}`,
            kind: isDir
              ? monaco.languages.CompletionItemKind.Folder
              : monaco.languages.CompletionItemKind.File,
            insertText: `${name}${suffix}`,
            range: replaceRangeForPartial,
            detail: isDir ? '目录' : '文件',
            sortText: isDir ? `0_${name}` : `1_${name}`
          };
        });

        return { suggestions };
      }
    });
  });
};

const registerMarkdownLanguageFeatures = () => {
  if (markdownFeaturesRegistered) return;
  markdownFeaturesRegistered = true;

  const createSnippet = (label, insertText, detail) => ({
    label,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules:
      monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail
  });

  const markdownSnippets = [
    createSnippet('h1', '# ${1:标题}\n\n', '插入一级标题'),
    createSnippet('h2', '## ${1:标题}\n\n', '插入二级标题'),
    createSnippet('h3', '### ${1:标题}\n\n', '插入三级标题'),
    createSnippet('h4', '#### ${1:标题}\n\n', '插入四级标题'),
    createSnippet('h5', '##### ${1:标题}\n\n', '插入五级标题'),
    createSnippet('h6', '###### ${1:标题}\n\n', '插入六级标题'),
    createSnippet('ul', '- ${1:列表项}\n- ${2:列表项}\n', '插入无序列表'),
    createSnippet('ol', '1. ${1:第一项}\n2. ${2:第二项}\n', '插入有序列表'),
    createSnippet('taskl', '- [ ] ${1:任务}\n- [ ] ${2:任务}\n', '插入任务列表'),
    createSnippet('table', '| ${1:表头1} | ${2:表头2} |\n| --- | --- |\n| ${3:内容1} | ${4:内容2} |\n', '插入表格'),
    createSnippet('code', '```${1:language}\n${2:代码}\n```\n', '插入代码块'),
    createSnippet('toc', '${1:目录}\n\n${2:目录内容}\n', '插入目录'),
    createSnippet('bold', '**${1:文本}**', '加粗文本'),
    createSnippet('italic', '*${1:文本}*', '斜体文本'),
    createSnippet('strikeout', '~~${1:文本}~~', '删除线'),
    createSnippet('inlineCode', '`${1:code}`', '内联代码'),
    createSnippet('link', '[${1:描述}](${2:链接})', '插入链接'),
    createSnippet('image', '![${1:描述}](${2:路径})', '插入图片'),
    createSnippet('quote', '> ${1:引用内容}\n', '插入引用'),
    createSnippet('hr', '\n---\n', '插入分隔线'),
    createSnippet('table2', '| 列1 | 列2 |\n| --- | --- |\n| ${1:值1} | ${2:值2} |\n', '插入表格'),
    createSnippet('table3', '| 列1 | 列2 | 列3 |\n| --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} |\n', '插入三列表格'),
    createSnippet('table4', '| 列1 | 列2 | 列3 | 列4 |\n| --- | --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} | ${4:值4} |\n', '插入四列表格'),
    createSnippet('table5', '| 列1 | 列2 | 列3 | 列4 | 列5 |\n| --- | --- | --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} | ${4:值4} | ${5:值5} |\n', '插入五列表格'),
    createSnippet('table6', '| 列1 | 列2 | 列3 | 列4 | 列5 | 列6 |\n| --- | --- | --- | --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} | ${4:值4} | ${5:值5} | ${6:值6} |\n', '插入六列表格'),
    createSnippet('table7', '| 列1 | 列2 | 列3 | 列4 | 列5 | 列6 | 列7 |\n| --- | --- | --- | --- | --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} | ${4:值4} | ${5:值5} | ${6:值6} | ${7:值7} |\n', '插入七列表格'),
    createSnippet('table8', '| 列1 | 列2 | 列3 | 列4 | 列5 | 列6 | 列7 | 列8 |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} | ${4:值4} | ${5:值5} | ${6:值6} | ${7:值7} | ${8:值8} |\n', '插入八列表格'),
    createSnippet('table9', '| 列1 | 列2 | 列3 | 列4 | 列5 | 列6 | 列7 | 列8 | 列9 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n| ${1:值1} | ${2:值2} | ${3:值3} | ${4:值4} | ${5:值5} | ${6:值6} | ${7:值7} | ${8:值8} | ${9:值9} |\n', '插入九列表格'),
    createSnippet('red', '<font color="red">${1:红色文字}</font>', '设置红色文本'),
    createSnippet('blue', '<font color="blue">${1:蓝色文字}</font>', '设置蓝色文本'),
    createSnippet('green', '<font color="green">${1:绿色文字}</font>', '设置绿色文本'),
    createSnippet('yellow', '<font color="yellow">${1:黄色文字}</font>', '设置黄色文本'),
    createSnippet('orange', '<font color="orange">${1:橙色文字}</font>', '设置橙色文本'),
    createSnippet('purple', '<font color="purple">${1:紫色文字}</font>', '设置紫色文本'),
    createSnippet('cyan', '<font color="cyan">${1:青色文字}</font>', '设置青色文本'),
    createSnippet('purple', '<font color="purple">${1:紫色文字}</font>', '设置紫色文本'),
  ];

  const createLatexSnippet = (key, insertText, detail, label) => ({
    key: String(key || '').toLowerCase(),
    label: label || `\\${key}`,
    kind: monaco.languages.CompletionItemKind.Snippet,
    insertText,
    insertTextRules:
      monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
    detail
  });

  const latexSnippets = [
    createLatexSnippet('frac', '\\frac{${1:a}}{${2:b}}', '分式'),
    createLatexSnippet('sqrt', '\\sqrt{${1:x}}', '平方根'),
    createLatexSnippet('sqrtn', '\\sqrt[${1:n}]{${2:x}}', 'n 次方根', '\\sqrt[n]{x}'),
    createLatexSnippet('sum', '\\sum_{${1:i=1}}^{${2:n}} ${3:expr}', '求和'),
    createLatexSnippet('prod', '\\prod_{${1:i=1}}^{${2:n}} ${3:expr}', '连乘'),
    createLatexSnippet('int', '\\int_{${1:a}}^{${2:b}} ${3:f(x)}\\,d${4:x}', '积分'),
    createLatexSnippet('lim', '\\lim_{${1:n}\\to\\infty} ${2:expr}', '极限'),
    createLatexSnippet('cdot', '\\cdot', '点乘'),
    createLatexSnippet('times', '\\times', '乘号'),
    createLatexSnippet('pm', '\\pm', '正负号'),
    createLatexSnippet('to', '\\to', '趋于'),
    createLatexSnippet('infty', '\\infty', '无穷'),

    createLatexSnippet('left', '\\left(${1:expr}\\right)', '自动伸缩括号'),
    createLatexSnippet('text', '\\text{${1:text}}', '文本'),
    createLatexSnippet('mathbf', '\\mathbf{${1:x}}', '粗体'),
    createLatexSnippet('mathbb', '\\mathbb{${1:R}}', '黑板粗体'),
    createLatexSnippet('mathcal', '\\mathcal{${1:L}}', '花体'),

    createLatexSnippet('begin', '\\begin{${1:env}}\n  ${0}\n\\end{${1:env}}', '环境'),
    createLatexSnippet('aligned', '\\begin{aligned}\n  ${0}\n\\end{aligned}', 'aligned 环境', '\\begin{aligned}...\\end{aligned}'),
    createLatexSnippet('align', '\\begin{align}\n  ${0}\n\\end{align}', 'align 环境', '\\begin{align}...\\end{align}'),
    createLatexSnippet('cases', '\\begin{cases}\n  ${1:expr} & ${2:cond}\\\\\n  ${3:expr} & ${4:cond}\n\\end{cases}', 'cases 环境', '\\begin{cases}...\\end{cases}'),

    createLatexSnippet('alpha', '\\alpha', '希腊字母 α'),
    createLatexSnippet('beta', '\\beta', '希腊字母 β'),
    createLatexSnippet('gamma', '\\gamma', '希腊字母 γ'),
    createLatexSnippet('delta', '\\delta', '希腊字母 δ'),
    createLatexSnippet('epsilon', '\\epsilon', '希腊字母 ε'),
    createLatexSnippet('theta', '\\theta', '希腊字母 θ'),
    createLatexSnippet('lambda', '\\lambda', '希腊字母 λ'),
    createLatexSnippet('mu', '\\mu', '希腊字母 μ'),
    createLatexSnippet('pi', '\\pi', '希腊字母 π'),
    createLatexSnippet('sigma', '\\sigma', '希腊字母 σ'),
    createLatexSnippet('phi', '\\phi', '希腊字母 φ'),
    createLatexSnippet('omega', '\\omega', '希腊字母 ω'),

    createLatexSnippet('leq', '\\leq', '≤'),
    createLatexSnippet('geq', '\\geq', '≥'),
    createLatexSnippet('neq', '\\neq', '≠'),
    createLatexSnippet('approx', '\\approx', '≈'),
    createLatexSnippet('equiv', '\\equiv', '≡'),
    createLatexSnippet('subseteq', '\\subseteq', '⊆'),
    createLatexSnippet('supseteq', '\\supseteq', '⊇'),
    createLatexSnippet('in', '\\in', '∈'),
    createLatexSnippet('notin', '\\notin', '∉')
  ];

  const isInMarkdownMath = (model, position) => {
    const maxScanLines = 250;
    const startLine = Math.max(1, position.lineNumber - maxScanLines + 1);

    let inFence = false;
    let fenceChar = '';
    let fenceLen = 0;

    let inDollar = false;
    let inDisplay = false;
    let inParen = false;
    let inBracket = false;

    for (let lineNumber = startLine; lineNumber <= position.lineNumber; lineNumber += 1) {
      let text = model.getLineContent(lineNumber);
      if (lineNumber === position.lineNumber) {
        text = text.slice(0, position.column - 1);
      }

      const fence = /^\s*(`{3,}|~{3,})/.exec(text);
      if (fence) {
        const mark = fence[1];
        if (!inFence) {
          inFence = true;
          fenceChar = mark[0];
          fenceLen = mark.length;
        } else if (mark[0] === fenceChar && mark.length >= fenceLen) {
          inFence = false;
          fenceChar = '';
          fenceLen = 0;
        }
        continue;
      }
      if (inFence) continue;

      for (let i = 0; i < text.length;) {
        const ch = text[i];

        if (ch === '\\') {
          const next = text[i + 1];
          if (next === '(') {
            inParen = true;
            i += 2;
            continue;
          }
          if (next === ')') {
            inParen = false;
            i += 2;
            continue;
          }
          if (next === '[') {
            inBracket = true;
            i += 2;
            continue;
          }
          if (next === ']') {
            inBracket = false;
            i += 2;
            continue;
          }
          i += 1;
          continue;
        }

        if (ch === '$') {
          const prev = i > 0 ? text[i - 1] : '';
          if (prev === '\\') {
            i += 1;
            continue;
          }

          const next = text[i + 1];
          if (next === '$') {
            inDisplay = !inDisplay;
            i += 2;
            continue;
          }

          if (!inDisplay) {
            inDollar = !inDollar;
          }
          i += 1;
          continue;
        }

        i += 1;
      }
    }

    return inDisplay || inDollar || inParen || inBracket;
  };

  monaco.languages.registerCompletionItemProvider('markdown', {
    provideCompletionItems(model, position) {
      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.slice(0, position.column - 1);

      const token = extractPathToken(beforeCursor);
      const pathContext = isFilesystemPathContext(beforeCursor);
      const inPathLike =
        pathContext ||
        (!!token &&
          (token.includes('/') ||
            token.includes('\\') ||
            token.startsWith('.') ||
            token.startsWith('~') ||
            /^[a-zA-Z]:/.test(token)));
      if (inPathLike) return { suggestions: [] };

      const word = model.getWordUntilPosition(position)?.word || '';
      if (!word) return { suggestions: [] };

      const lower = word.toLowerCase();
      const suggestions = markdownSnippets
        .filter((s) => String(s.label || '').toLowerCase().startsWith(lower))
        .slice(0, 50);

      return { suggestions };
    }
  });

  monaco.languages.registerCompletionItemProvider('markdown', {
    triggerCharacters: ['\\'],
    provideCompletionItems(model, position) {
      if (!isInMarkdownMath(model, position)) return { suggestions: [] };

      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.slice(0, position.column - 1);

      const match = /\\[A-Za-z]*$/.exec(beforeCursor);
      if (!match) return { suggestions: [] };

      const typed = match[0].slice(1);
      const prefix = typed.toLowerCase();

      const startColumn = Math.max(1, position.column - match[0].length);
      const range = new monaco.Range(
        position.lineNumber,
        startColumn,
        position.lineNumber,
        position.column
      );

      const suggestions = latexSnippets
        .filter((s) => !prefix || String(s.key || '').startsWith(prefix))
        .slice(0, 80)
        .map((s, index) => ({
          ...s,
          range,
          sortText: `0_${String(index).padStart(3, '0')}_${String(s.key || '')}`
        }));

      return { suggestions };
    }
  });
};

const tabBarRef = ref(null)
const tabRegionLeftRef = ref(null)
const tabRegionRightRef = ref(null)

const languagePairingConfigured = new Set()

const ensureLanguagePairingConfig = (languageId) => {
  const id = languageId || 'plaintext'
  if (languagePairingConfigured.has(id)) return
  if (id !== 'markdown' && id !== 'plaintext') return

  try {
    monaco.languages.setLanguageConfiguration(id, {
      brackets: [
        ['{', '}'],
        ['[', ']'],
        ['(', ')']
      ],
      autoClosingPairs: [
        { open: '{', close: '}' },
        { open: '[', close: ']' },
        { open: '(', close: ')' },
        { open: '"', close: '"' },
        { open: "'", close: "'" },
        { open: '`', close: '`' },
        { open: '“', close: '”' },
        { open: '‘', close: '’' }
      ],
      surroundingPairs: [
        { open: '{', close: '}' },
        { open: '[', close: ']' },
        { open: '(', close: ')' },
        { open: '"', close: '"' },
        { open: "'", close: "'" },
        { open: '`', close: '`' },
        { open: '“', close: '”' },
        { open: '‘', close: '’' }
      ]
    })

    languagePairingConfigured.add(id)
    console.debug('[Editor] language pairing configured', { languageId: id })
  } catch (e) {
    console.warn('[Editor] failed to configure language pairing', { languageId: id, error: e })
  }
}

// 初始化编辑器
const initEditor = () => {
  console.debug('[Editor] initEditor called', {
    hasContainer: !!editorContainer.value,
    selectedPath: selectedFile.value?.path,
    activeTab: fileTreeStore.activeTab
  })

  if (!selectedFile.value) return

  if (!editorContainer.value) {
    if (!initEditorPending) {
      initEditorPending = true;
      nextTick(() => {
        initEditorPending = false;
        if (!editor && editorContainer.value) {
          initEditor();
        }
      });
    }
    console.debug('[Editor] editor container not ready');
    return;
  }

  // 获取文件扩展名并确定语言
  const fileExt = selectedFile.value?.path?.split('.').pop() || 'markdown';
  const languageMap = {
    'js': 'javascript',
    'ts': 'typescript',
    'css': 'css',
    'html': 'html',
    'htm': 'html',
    'md': 'markdown',
    'py': 'python',
    'cpp': 'cpp',
    'c': 'cpp',
    'h': 'cpp',
    'hpp': 'cpp',
    'rs': 'rust',
    'sh': 'shell',
    'jsx': 'javascript',
    'tsx': 'javascript',
    'json': 'json',
    'yaml': 'yaml',
    'yml': 'yaml',
    'lua': 'lua',
    'java': 'java',
    'toml': 'ini',
  };
  const language = languageMap[fileExt] || 'markdown';

  ensureLanguagePairingConfig(language)

  const isDark = document.documentElement.dataset.guaxiTheme === 'dark';

  monaco.editor.defineTheme('guaxi-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: 'a0a1a7' },           // Comment
      { token: 'keyword', foreground: 'a626a4' },           // Keyword
      { token: 'string', foreground: '50a14f' },           // String
      { token: 'number', foreground: '986801' },           // Number
      { token: 'regexp', foreground: '0184bc' },           // RegExp → 用 Operator 相近色
      { token: 'type', foreground: 'c18401' },           // Type
      { token: 'delimiter', foreground: '1f2328' },           // Delimiter → Main text color
      { token: 'variable', foreground: 'e45649' },           // Variable
      { token: 'parameter', foreground: '4078f2' },           // Parameter → Function color
      { token: 'function', foreground: '4078f2' },           // Function
      { token: 'constant', foreground: '986801' },           // Constant → Number color
      { token: 'class', foreground: 'c18401' },           // Class → Type color
      { token: 'identifier', foreground: '1f2328' },           // Identifier → Main text color
      { token: 'tag', foreground: 'e45649' },           // Tag → Variable color
      { token: 'attribute.name', foreground: 'c18401' },           // Attribute name → Type color
      { token: 'meta.embedded', foreground: 'a0a1a7' }            // Meta embedded → Comment color
    ],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#1f2328',
      'editorLineNumber.foreground': '#a0a1a7',
      'editorLineNumber.activeForeground': '#1f2328',
      'editorCursor.foreground': '#4078f2',
      'editor.selectionBackground': '#d7dae0',
      'editor.inactiveSelectionBackground': '#e3f2fd',
      'editor.lineHighlightBackground': '#f0f0f0',
      'editorBracketMatch.background': '#e1f5fe',
      'editorBracketMatch.border': '#90caf9',
      'editorBracketHighlight.foreground1': '#d73a49',
      'editorBracketHighlight.foreground2': '#f66a0a',
      'editorBracketHighlight.foreground3': '#b08800',
      'editorBracketHighlight.foreground4': '#22863a',
      'editorBracketHighlight.foreground5': '#0366d6',
      'editorBracketHighlight.foreground6': '#6f42c1',
      'editorBracketHighlight.unexpectedBracket.foreground': '#d73a49',
      'editorBracketPairGuide.background1': '#d73a4933',
      'editorBracketPairGuide.background2': '#f66a0a33',
      'editorBracketPairGuide.background3': '#b0880033',
      'editorBracketPairGuide.background4': '#22863a33',
      'editorBracketPairGuide.background5': '#0366d633',
      'editorBracketPairGuide.background6': '#6f42c133',
      'editorBracketPairGuide.activeBackground1': '#d73a4966',
      'editorBracketPairGuide.activeBackground2': '#f66a0a66',
      'editorBracketPairGuide.activeBackground3': '#b0880066',
      'editorBracketPairGuide.activeBackground4': '#22863a66',
      'editorBracketPairGuide.activeBackground5': '#0366d666',
      'editorBracketPairGuide.activeBackground6': '#6f42c166',
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
      'scrollbar.shadow': '#00000033'
    }
  });

  monaco.editor.defineTheme('guaxi-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '5c6370' },           // Comment
      { token: 'keyword', foreground: 'c678dd' },           // Keyword
      { token: 'string', foreground: '98c379' },           // String
      { token: 'number', foreground: 'd19a66' },           // Number
      { token: 'regexp', foreground: '56b6c2' },           // RegExp → Operator color
      { token: 'type', foreground: 'e5c07b' },           // Type
      { token: 'delimiter', foreground: 'abb2bf' },           // Delimiter → Main text color
      { token: 'variable', foreground: 'e06c75' },           // Variable
      { token: 'parameter', foreground: '61afef' },           // Parameter → Function color
      { token: 'function', foreground: '61afef' },           // Function
      { token: 'constant', foreground: 'd19a66' },           // Constant → Number color
      { token: 'class', foreground: 'e5c07b' },           // Class → Type color
      { token: 'identifier', foreground: 'abb2bf' },           // Identifier → Main text color
      { token: 'tag', foreground: 'e06c75' },           // Tag → Variable color
      { token: 'attribute.name', foreground: 'e5c07b' },           // Attribute name → Type color
      { token: 'meta.embedded', foreground: '5c6370' }            // Meta embedded → Comment color
    ],
    colors: {
      'editor.background': '#282c34',
      'editor.foreground': '#abb2bf',
      'editorLineNumber.foreground': '#5c6370',
      'editorLineNumber.activeForeground': '#abb2bf',
      'editorCursor.foreground': '#61afef',
      'editor.selectionBackground': '#3e4451',
      'editor.inactiveSelectionBackground': '#323842',
      'editor.lineHighlightBackground': '#2c313a',
      'editorBracketMatch.background': '#3b4048',
      'editorBracketMatch.border': '#528bff',
      'editorBracketHighlight.foreground1': '#e06c75',
      'editorBracketHighlight.foreground2': '#d19a66',
      'editorBracketHighlight.foreground3': '#e5c07b',
      'editorBracketHighlight.foreground4': '#98c379',
      'editorBracketHighlight.foreground5': '#56b6c2',
      'editorBracketHighlight.foreground6': '#c678dd',
      'editorBracketHighlight.unexpectedBracket.foreground': '#e06c75',
      'editorBracketPairGuide.background1': '#e06c7533',
      'editorBracketPairGuide.background2': '#d19a6633',
      'editorBracketPairGuide.background3': '#e5c07b33',
      'editorBracketPairGuide.background4': '#98c37933',
      'editorBracketPairGuide.background5': '#56b6c233',
      'editorBracketPairGuide.background6': '#c678dd33',
      'editorBracketPairGuide.activeBackground1': '#e06c7566',
      'editorBracketPairGuide.activeBackground2': '#d19a6666',
      'editorBracketPairGuide.activeBackground3': '#e5c07b66',
      'editorBracketPairGuide.activeBackground4': '#98c37966',
      'editorBracketPairGuide.activeBackground5': '#56b6c266',
      'editorBracketPairGuide.activeBackground6': '#c678dd66',
      'editor.selectionHighlightBackground': '#3e445155',
      'editor.foldBackground': '#2c313a',
      'editorWhitespace.foreground': '#3b4048',
      'editorIndentGuide.background': '#3b4048',
      'editorIndentGuide.activeBackground': '#4b5263',
      'editorHoverWidget.background': '#282c34',
      'editorHoverWidget.border': '#3b4048',
      'editorSuggestWidget.background': '#282c34',
      'editorSuggestWidget.border': '#3b4048',
      'editorSuggestWidget.foreground': '#abb2bf',
      'editorSuggestWidget.selectedBackground': '#2c313a',
      'editorWidget.background': '#282c34',
      'editorWidget.border': '#3b4048',
      'scrollbar.shadow': '#00000066'
    }
  });

  const themeName = isDark ? 'guaxi-dark' : 'guaxi-light';

  editor = monaco.editor.create(editorContainer.value, {
    value: rawContent.value,
    language: language,
    theme: themeName,
    readOnly: !!activeTab.value?.unsupported,
    autoClosingBrackets: 'always',
    autoClosingQuotes: 'always',
    autoClosingDelete: 'auto',
    autoClosingOvertype: 'auto',
    autoSurround: 'languageDefined',
    automaticLayout: true,  // 自动布局
    scrollBeyondLastColumn: 40, // 允许水平滚动超出最后一列
    fixedOverflowWidgets: true, // 确保浮动部件可以超出编辑器边界
    wordWrap: 'on',
    renderValidationDecorations: 'on',
    minimap: {
      enabled: false,  // 启用迷你地图
      showSlider: 'always',
      size: 'fit'
    },
    glyphMargin: true, // 启用行号左侧边栏
    lineDecorationsWidth: 6,
    scrollbar: {
      vertical: 'auto',
      horizontal: 'auto',
      useShadows: true,
      verticalScrollbarSize: 10,
      horizontalScrollbarSize: 10,
      arrowSize: 10,
      alwaysConsumeMouseWheel: false,
      verticalHasArrows: false,
      horizontalHasArrows: false,
      handleMouseWheel: true,
      scrollByPage: false,
      theme: {
        scrollbarShadow: '#00000022',
        scrollbarSliderBackground: '#64646466',
        scrollbarSliderHoverBackground: '#64646499',
        scrollbarSliderActiveBackground: '#646464CC'
      }
    },
    vim: {
      enabled: vimMode.value,
      status: true,
      useSystemClipboard: true,
      handleKeys: {
        '<C-s>': false,
        '<C-c>': false,
        '<C-v>': false,
        '<C-a>': false
      }
    },
    contextmenu: {
      enabled: true,
      addDefaultActions: true,
      addNavigationActions: true
    },
    lineNumbers: 'on',
    lineNumbersMinChars: 3,  // 最小行号宽度
    scrollBeyondLastLine: false,
    fontSize: 18,  // 调整为更合适的字号
    fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
    fontLigatures: true,
    // wrappingIndent: 'indent',  // 换行缩进
    renderWhitespace: 'selection',  // 选中时显示空白字符
    renderLineHighlight: 'all',  // 高亮当前行
    lineHeight: 24,  // 行高
    padding: {
      top: 8,  // 顶部内边距
      bottom: 8  // 底部内边距
    },
    suggest: {
      snippetsPreventQuickSuggestions: true  // 允许代码片段建议
    },
    quickSuggestions: {
      other: true,
      comments: false,
      strings: true
    },
    parameterHints: {
      enabled: true  // 启用参数提示
    },
    bracketPairColorization: {
      enabled: true  // 括号配对着色
    },
    guides: {
      bracketPairs: true,  // 显示括号配对引导线
      bracketPairsHorizontal: 'active'  // 水平引导线
    },
    mouseWheelZoom: true  // 启用鼠标滚轮缩放
  });

  invoke('get_lsp_syntax_highlight_enabled')
    .then((result) => {
      const enableLspSyntaxHighlight = !!result
      if (enableLspSyntaxHighlight) {
        initTextmate(editor, language);
        initLspClient(editor, language, selectedFile.value?.path || '');
      }
    })
    .catch((error) => {
      console.error('加载 LSP 语法高亮配置失败:', error)
    });

  // 注册编辑器实例到store
  editorStore.setEditorInstance(editor);
  editorStore.setLanguage(language);
  console.debug('[Editor] Monaco editor instance registered')

  if (editorEffectsLeft) {
    editorEffectsLeft.destroy();
    editorEffectsLeft = null;
  }
  editorEffectsLeft = createEditorEffects(editor);
  editorEffectsLeft?.updateCursorTarget();

  registerMarkdownLanguageFeatures();
  registerFilesystemPathCompletionFeatures();

  if (vimMode.value) {
    nextTick(() => {
      if (editor && vimStatusBar.value) {
        vimModeInstance = initVimMode(editor, vimStatusBar.value);
        editor.updateOptions({
          vim: {
            enabled: true,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
              '<C-s>': false,
              '<C-c>': false,
              '<C-v>': false
            }
          }
        });
      }
      applyVimOverlayInsets();
    });
  }

  // 注册自定义命令
  const registerCommands = () => {
    // Markdown模板命令
    editor.addAction({
      id: 'insert.md.template',
      label: '插入Markdown模板',
      run: () => {
        const selection = editor.getSelection();
        editor.executeEdits('', [{
          range: selection,
          text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
        }]);
      }
    });
  };

  // 注册自定义命令和菜单项
  const registerContextMenu = () => {
    // 1. 注册Markdown模板命令
    editor.addAction({
      id: 'insert.md.template',
      label: '插入Markdown模板',
      contextMenuGroupId: '2_custom',
      contextMenuOrder: 1,
      run: () => {
        const selection = editor.getSelection();
        editor.executeEdits('', [{
          range: selection,
          text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
        }]);
      }
    });

    editor.addAction({
      id: 'guaxi.markdown.formatDocument',
      label: 'Markdown 格式化（全文）',
      run: () => formatMarkdownDocument()
    });

    editor.addAction({
      id: 'guaxi.markdown.formatSelection',
      label: 'Markdown 格式化（选中）',
      run: () => formatMarkdownSelection()
    });

    editor.addAction({
      id: 'guaxi.edit.navigateBack',
      label: '跳转到上次编辑位置',
      keybindings: [monaco.KeyMod.Alt | monaco.KeyCode.LeftArrow],
      run: () => navigateToPreviousEditLocation()
    });

    editor.addAction({
      id: 'guaxi.edit.navigateForward',
      label: '跳转到下一编辑位置',
      keybindings: [monaco.KeyMod.Alt | monaco.KeyCode.RightArrow],
      run: () => navigateToNextEditLocation()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleBold',
      label: '加粗',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyB],
      run: () => toggleBold()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleItalic',
      label: '斜体',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyI],
      run: () => toggleItalic()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleStrikethrough',
      label: '删除线',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyX],
      run: () => toggleStrikethrough()
    });

    editor.addAction({
      id: 'guaxi.markdown.toggleInlineCode',
      label: '内联代码',
      run: () => toggleInlineCode()
    });

    editor.addAction({
      id: 'guaxi.markdown.insertLink',
      label: '插入链接',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyK],
      run: () => insertLink()
    });

    editor.addAction({
      id: 'guaxi.markdown.insertTable',
      label: '插入表格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyT],
      run: () => insertTable()
    });

    editor.addAction({
      id: 'guaxi.table.addRowBelow',
      label: '表格：下方插入行',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.DownArrow],
      run: () => addTableRowBelow()
    });

    editor.addAction({
      id: 'guaxi.table.deleteRow',
      label: '表格：删除当前行',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyMod.Shift | monaco.KeyCode.DownArrow],
      run: () => deleteTableRow()
    });

    editor.addAction({
      id: 'guaxi.table.addColumnRight',
      label: '表格：右侧插入列',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.RightArrow],
      run: () => addTableColumnRight()
    });

    editor.addAction({
      id: 'guaxi.table.deleteColumn',
      label: '表格：删除当前列',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyMod.Shift | monaco.KeyCode.RightArrow],
      run: () => deleteTableColumn()
    });

    editor.onContextMenu((e) => {
      e.event.preventDefault();
      const { posx, posy } = e.event;

      editor.focus();
      editorContextMenuPosition.value = { x: posx, y: posy };
      editorContextMenuOptions.value = buildEditorContextMenuOptions();
      editorContextMenuVisible.value = true;
    });

    outlineStore.setJumpToLine(jumpToLine)
  };

  // 初始化上下文菜单
  registerContextMenu();
  setupEditorListeners();
};

const ensureSecondaryEditor = () => {
  if (!secondaryEditorContainer.value || secondaryEditor) return;

  const rightPath = fileTreeStore.activeTabRight;
  const rightTab = rightPath ? fileTreeStore.openTabs.find((t) => t.path === rightPath) : null;

  const fileExt = rightPath?.split('.').pop() || 'markdown';
  const languageMap = {
    'js': 'javascript',
    'ts': 'typescript',
    'css': 'css',
    'html': 'html',
    'htm': 'html',
    'md': 'markdown',
    'py': 'python',
    'cpp': 'cpp',
    'c': 'cpp',
    'h': 'cpp',
    'hpp': 'cpp',
    'rs': 'rust',
    'sh': 'shell',
    'jsx': 'javascript',
    'tsx': 'javascript',
    'json': 'json',
    'yaml': 'yaml',
    'yml': 'yaml',
    'lua': 'lua',
    'java': 'java',
    'toml': 'ini',
  };
  const language = languageMap[fileExt] || 'markdown';

  const baseOptions = editor?.getRawOptions ? editor.getRawOptions() : {};

  secondaryEditor = monaco.editor.create(secondaryEditorContainer.value, {
    ...baseOptions,
    value: rightTab?.content || '',
    language,
    readOnly: !!rightTab?.unsupported,
    autoClosingBrackets: 'always',
    autoClosingQuotes: 'always',
    autoClosingDelete: 'auto',
    autoClosingOvertype: 'auto',
    autoSurround: 'languageDefined',
    bracketPairColorization: { enabled: true },
    guides: {
      bracketPairs: true,
      bracketPairsHorizontal: 'active'
    },
    automaticLayout: true,
  });

  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }
  editorEffectsRight = createEditorEffects(secondaryEditor);
  editorEffectsRight?.updateCursorTarget();

  secondaryEditor.onDidFocusEditorText(() => {
    if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.activeTabRight) {
      fileTreeStore.setActiveTab(fileTreeStore.activeTabRight, 'right');
    }
  });

  secondaryEditor.onDidChangeCursorPosition(() => {
    editorEffectsRight?.updateCursorTarget();
  });

  secondaryEditor.onDidScrollChange(() => {
    editorEffectsRight?.updateCursorTarget();
  });

  secondaryEditor.onDidType((text) => {
    editorEffectsRight?.spawnExplosionAtCursor(text);
    editorEffectsRight?.updateCursorTarget();
  });

  const model = secondaryEditor.getModel();
  if (model) {
    model.onDidChangeContent(
      debounce(() => {
        if (fileTreeStore.activeRegion !== 'right') return;
        const path = fileTreeStore.activeTab;
        if (!path) return;
        const content = model.getValue();
        rawContent.value = content;
        fileTreeStore.updateTabContent(path, content);
        updateGitDecorations();
      }, 120)
    );
  }
};

const disposeSecondaryEditor = () => {
  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }
  if (secondaryEditor) {
    secondaryEditor.dispose();
    secondaryEditor = null;
  }
};

watch(
  () => fileTreeStore.layoutMode,
  (mode) => {
    if (mode === 'single') {
      disposeSecondaryEditor();
    } else {
      nextTick(() => {
        ensureSecondaryEditor();
      });
    }
  },
  { immediate: true }
);


// Git 服务
const gitService = ref(null)

let gitDiffEditor = null
let gitDiffContainerEl = null
let gitDiffOriginalModel = null
let gitDiffModifiedModel = null
let gitDecorationsSeq = 0

const ensureGitDiffEditor = (languageId) => {
  const lang = languageId || 'plaintext'

  if (gitDiffEditor && gitDiffOriginalModel && gitDiffModifiedModel) {
    monaco.editor.setModelLanguage(gitDiffOriginalModel, lang)
    monaco.editor.setModelLanguage(gitDiffModifiedModel, lang)
    return
  }

  gitDiffContainerEl = document.createElement('div')
  gitDiffContainerEl.style.position = 'absolute'
  gitDiffContainerEl.style.left = '-99999px'
  gitDiffContainerEl.style.top = '-99999px'
  gitDiffContainerEl.style.width = '1px'
  gitDiffContainerEl.style.height = '1px'
  gitDiffContainerEl.style.overflow = 'hidden'
  document.body.appendChild(gitDiffContainerEl)

  gitDiffEditor = monaco.editor.createDiffEditor(gitDiffContainerEl, {
    renderSideBySide: false,
    readOnly: true,
    renderIndicators: false,
    automaticLayout: false,
    scrollBeyondLastLine: false,
    diffAlgorithm: 'advanced'
  })

  gitDiffOriginalModel = monaco.editor.createModel('', lang)
  gitDiffModifiedModel = monaco.editor.createModel('', lang)

  gitDiffEditor.setModel({
    original: gitDiffOriginalModel,
    modified: gitDiffModifiedModel
  })
}

const computeUnsavedLineChanges = () => {
  const filePath = selectedFile.value?.path
  if (!filePath) return null

  const tab = fileTreeStore.openTabs.find((t) => t?.path === filePath)
  if (!tab) return null

  const original = tab.savedContent ?? ''
  const modified = tab.content ?? ''
  if (original === modified) return { added: [], modified: [], deleted: [] }

  const languageId = editor?.getModel?.()?.getLanguageId?.() || 'plaintext'
  ensureGitDiffEditor(languageId)

  gitDiffOriginalModel.setValue(original)
  gitDiffModifiedModel.setValue(modified)

  const lineChanges = gitDiffEditor.getLineChanges() || []
  const modifiedLineCount = gitDiffModifiedModel.getLineCount()

  const added = new Set()
  const changed = new Set()
  const deleted = new Set()

  const toLen = (start, end) => (end === 0 ? 0 : Math.max(0, end - start + 1))
  const clampLine = (n) => {
    const nn = Number(n) || 1
    if (modifiedLineCount <= 0) return 1
    return Math.min(Math.max(1, nn), modifiedLineCount)
  }

  for (const ch of lineChanges) {
    const oLen = toLen(ch.originalStartLineNumber, ch.originalEndLineNumber)
    const mLen = toLen(ch.modifiedStartLineNumber, ch.modifiedEndLineNumber)

    if (oLen === 0 && mLen > 0) {
      for (let ln = ch.modifiedStartLineNumber; ln <= ch.modifiedEndLineNumber; ln += 1) {
        added.add(clampLine(ln))
      }
      continue
    }

    if (mLen === 0 && oLen > 0) {
      deleted.add(clampLine(ch.modifiedStartLineNumber))
      continue
    }

    if (mLen > 0) {
      for (let ln = ch.modifiedStartLineNumber; ln <= ch.modifiedEndLineNumber; ln += 1) {
        changed.add(clampLine(ln))
      }
    }
  }

  return {
    added: Array.from(added).sort((a, b) => a - b),
    modified: Array.from(changed).sort((a, b) => a - b),
    deleted: Array.from(deleted).sort((a, b) => a - b)
  }
}

// 初始化 Git 服务
const initGitService = async () => {
  const filePath = selectedFile.value?.path
  if (filePath) {
    const dir = await dirname(filePath)
    console.debug('[Git] initGitService', { filePath, dir })
    gitService.value = new GitService(dir)
    await updateGitStatus()
    return
  }

  console.debug('[Git] initGitService skipped (no selected file)')
}

// 更新 Git 状态
const updateGitStatus = async () => {
  const filePath = selectedFile.value?.path
  const workspacePath = gitService.value?.workspacePath

  if (!gitService.value || !filePath) {
    console.debug('[Git] updateGitStatus skipped', {
      hasGitService: !!gitService.value,
      filePath
    })
    return
  }

  try {
    console.debug('[Git] updateGitStatus start', { filePath, workspacePath })

    const branch = await gitService.value.getCurrentBranch()
    editorStore.setGitBranch(branch)

    const status = await gitService.value.getFileStatus(filePath)
    const counts = {
      added: status.filter(s => s.status === 'A').length,
      modified: status.filter(s => s.status === 'M').length,
      deleted: status.filter(s => s.status === 'D').length
    }

    console.debug('[Git] fileStatus', {
      filePath,
      statusCount: Array.isArray(status) ? status.length : null,
      counts,
      sample: Array.isArray(status) ? status.slice(0, 3) : status
    })

    editorStore.setGitChanges(counts)

    await updateGitDecorations()
    console.debug('[Git] updateGitStatus done', {
      gitDecorationsCount: editorStore.gitDecorations?.length || 0,
      gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
    })
  } catch (error) {
    console.error('[Git] updateGitStatus failed:', error)
  }
}

// 更新 Git 装饰器
const updateGitDecorations = async () => {
  const filePath = selectedFile.value?.path
  const seq = ++gitDecorationsSeq

  const apply = (changes) => {
    if (seq !== gitDecorationsSeq) return

    if (!changes) {
      editorStore.clearGitDecorations()
      return
    }

    editorStore.updateGitLineDecorations({
      added: changes.added || [],
      modified: changes.modified || [],
      deleted: changes.deleted || []
    })
  }

  if (!editor || !filePath || !gitService.value) {
    console.debug('[Git] updateGitDecorations skipped', {
      hasEditor: !!editor,
      filePath,
      hasGitService: !!gitService.value
    })
    apply(null)
    return
  }

  const tab = fileTreeStore.openTabs.find((t) => t?.path === filePath)
  const preferUnsaved = !!tab?.dirty

  try {
    console.debug('[Git] updateGitDecorations start', {
      filePath,
      hasEditorStoreInstance: !!editorStore.editorInstance,
      preferUnsaved
    })

    if (preferUnsaved) {
      const changes = computeUnsavedLineChanges()
      apply(changes)
      console.debug('[Git] updateGitDecorations done (unsaved)', {
        gitDecorationsCount: editorStore.gitDecorations?.length || 0,
        gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
      })
      return
    }

    const changes = await gitService.value.getLineChanges(filePath)
    apply(changes)

    console.debug('[Git] updateGitDecorations done', {
      gitDecorationsCount: editorStore.gitDecorations?.length || 0,
      gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
    })
  } catch (error) {
    console.error('[Git] updateGitDecorations failed:', error)
    apply(null)
  }
}

// 切换 Git 面板
const toggleGitPanel = async () => {
  const windowLabel = `git-${Date.now()}` // 唯一窗口标识
  const windowTitle = `${fileTreeStore.activeRepoPath || '未命名文件'}` // 使用不带后缀的文件名

  console.log('打开 Git 窗口:', windowLabel, windowTitle)
  console.log('workspace_path:', fileTreeStore.activeRepoPath)
  try {
    await invoke('create_git_window', {
      label: windowLabel,
      title: windowTitle,
      workspacePath: fileTreeStore.activeRepoPath,
      theme: settingsStore.appearanceTheme,
    })
  } catch (error) {
    console.error('创建窗口失败:', error)
    alert(`无法打开文件: ${error.message || '未知错误'}`)
  }
}

const handleWheelScroll = (e) => {
  const target = e?.target;
  const inRight = !!(target && target.closest && target.closest('.tab-region-right'));
  const container =
    fileTreeStore.layoutMode !== 'single' && inRight
      ? tabRegionRightRef.value
      : tabRegionLeftRef.value;

  if (!container) return;

  // 阻止默认的垂直滚动行为
  e.preventDefault()

  // 水平滚动（乘以一个系数可以调整滚动速度）
  container.scrollLeft += e.deltaY * 0.5
}

const setupEditorListeners = () => {
  if (!editor) return;

  editor.onDidFocusEditorText(() => {
    activateRegion('left');
  });

  editor.onDidChangeCursorPosition((e) => {
    editorStore.setCursorPosition(e.position);
    updateTableToolbar();
    editorEffectsLeft?.updateCursorTarget();
  });

  const model = editor.getModel();
  if (model) {
    model.onDidChangeContent(
      debounce(() => {
        const content = model.getValue();
        const languageId = model.getLanguageId() || 'plaintext';
        const problems = analyzeContentForProblems(content, model, languageId);
        editorStore.setProblems(problems);
        rawContent.value = content;
        const path = fileTreeStore.activeTab;
        if (path) fileTreeStore.updateTabContent(path, content);
        recordEditLocationFromEditor();
        updateTableToolbar();
        scheduleAutoFormatActiveTable();
        updateGitDecorations();
      }, 300)
    );
  }

  const syncRealtimePreviewScroll = debounce(() => {
    if (!editor) return;
    if (!showPreview.value) return;
    if (previewMode.value !== 'realtime') return;

    const ranges = editor.getVisibleRanges();
    const topLine = ranges?.[0]?.startLineNumber || editor.getPosition()?.lineNumber || 1;
    scrollPreviewToLine(topLine);
  }, 50);

  editor.onDidScrollChange(() => {
    if (tableToolbarVisible.value) {
      updateTableToolbar();
    }
    editorEffectsLeft?.updateCursorTarget();
    syncRealtimePreviewScroll();
  });

  editor.onDidChangeModelLanguage(() => {
    const language = editor.getModel()?.getLanguageId() || 'plaintext';
    editorStore.setLanguage(language);
    updateTableToolbar();
  });

  editor.onDidType((text) => {
    if (!editor) return;
    const ch = text ? text[text.length - 1] : '';
    if (!ch) return;

    editorEffectsLeft?.spawnExplosionAtCursor(text);
    editorEffectsLeft?.updateCursorTarget();

    const model = editor.getModel();
    const pos = editor.getPosition();
    if (!model || !pos) return;

    const lineContent = model.getLineContent(pos.lineNumber);
    const beforeCursor = lineContent.slice(0, pos.column - 1);
    const token = extractPathToken(beforeCursor);
    const pathContext = isFilesystemPathContext(beforeCursor);

    const isPathLike =
      pathContext ||
      (!!token &&
        (token.includes('/') ||
          token.includes('\\') ||
          token.startsWith('.') ||
          token.startsWith('~') ||
          /^[a-zA-Z]:/.test(token)));

    if (!isPathLike) return;

    if (!/[0-9a-zA-Z._\-~\/\\:]/.test(ch)) return;

    console.debug('[PathCompletion] triggerSuggest', {
      ch,
      token,
      pathContext
    });
    editor.trigger('guaxi-path', 'editor.action.triggerSuggest', {});
  });

  const isSuggestWidgetVisible = () => {
    const dom = editor?.getDomNode?.();
    if (!dom) return false;
    return !!dom.querySelector('.suggest-widget.visible');
  };

  editor.onKeyDown((e) => {
    if (e.keyCode === monaco.KeyCode.Enter) {
      const handled = handleMarkdownListEnter(e);
      if (handled) return;
    }

    if (e.keyCode !== monaco.KeyCode.Tab) return;
    if (!isSuggestWidgetVisible()) return;

    e.preventDefault();
    e.stopPropagation();

    editor.trigger(
      'guaxi-suggest',
      e.shiftKey ? 'selectPrevSuggestion' : 'selectNextSuggestion',
      null
    );
  });

  editor.onDidBlurEditorWidget(() => {
    floatingToolbarVisible.value = false;
    tableToolbarVisible.value = false;
  });

  editor.onDidChangeCursorSelection((e) => {
    const model = editor.getModel();
    if (!model) return;

    const selectedText = model.getValueInRange(e.selection);
    const status = selectedText
      ? `Selected ${countLines(selectedText)} lines, ${selectedText.length} chars`
      : null;
    editorStore.setSelectionStatus(status);

    if (model.getLanguageId() !== 'markdown') {
      floatingToolbarVisible.value = false;
      tableToolbarVisible.value = false;
      return;
    }

    const isEmpty = e.selection.isEmpty();
    if (isEmpty || !selectedText.trim()) {
      floatingToolbarVisible.value = false;
      return;
    }

    const wrapperEl = editorWrapperRef.value;
    const editorEl = editorContainer.value;
    if (!wrapperEl || !editorEl) {
      floatingToolbarVisible.value = false;
      return;
    }

    const startPos = e.selection.getStartPosition();
    const endPos = e.selection.getEndPosition();
    const startVisible = editor.getScrolledVisiblePosition(startPos);
    const endVisible = editor.getScrolledVisiblePosition(endPos);
    if (!startVisible || !endVisible) {
      floatingToolbarVisible.value = false;
      return;
    }

    const wrapperRect = wrapperEl.getBoundingClientRect();
    const editorRect = editorEl.getBoundingClientRect();

    const leftBase = editorRect.left - wrapperRect.left;
    const topBase = editorRect.top - wrapperRect.top;

    const selTop = Math.min(startVisible.top, endVisible.top);
    const selBottom = Math.max(startVisible.top + startVisible.height, endVisible.top + endVisible.height);
    const selLeft = Math.min(startVisible.left, endVisible.left);

    const headerEl = editorHeaderRef.value;
    const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0;
    const minTop = headerHeight + 8;

    const toolbarH = 36;
    const gap = 8;

    const preferTop = topBase + selTop - toolbarH - gap;
    const fallbackTop = topBase + selBottom + gap;

    const top = preferTop >= minTop ? preferTop : fallbackTop;
    const clampedTop = Math.min(Math.max(minTop, top), Math.max(minTop, wrapperRect.height - toolbarH - 8));

    const left = leftBase + selLeft;
    const clampedLeft = Math.min(Math.max(8, left), Math.max(8, wrapperRect.width - 260));

    floatingToolbarStyle.value = {
      left: `${clampedLeft}px`,
      top: `${clampedTop}px`
    };
    floatingToolbarVisible.value = true;
  });
};

// 内容问题分析
function analyzeContentForProblems(content, model, languageId) {
  if (!content) {
    if (model) {
      monaco.editor.setModelMarkers(model, 'markdown-lint', []);
    }
    return { errors: 0, warnings: 0 };
  }

  if (languageId === 'markdown') {
    return analyzeMarkdown(content, model);
  }

  let errors = 0;
  let warnings = 0;

  const lines = content.split('\n');
  lines.forEach((line) => {
    const openBraces = (line.match(/\(/g) || []).length;
    const closeBraces = (line.match(/\)/g) || []).length;
    if (openBraces !== closeBraces) errors++;

    if (line.length > 120) warnings++;
  });

  if (model) {
    monaco.editor.setModelMarkers(model, 'markdown-lint', []);
  }

  return { errors, warnings };
}

function analyzeMarkdown(content, model) {
  let errors = 0;
  let warnings = 0;
  const markers = [];
  const lines = content.split('\n');

  const pushMarker = (lineNumber, message, severity) => {
    if (!model) return;
    const maxColumn = model.getLineMaxColumn(lineNumber) || 1;
    markers.push({
      severity:
        severity === 'error'
          ? monaco.MarkerSeverity.Error
          : monaco.MarkerSeverity.Warning,
      message,
      startLineNumber: lineNumber,
      startColumn: 1,
      endLineNumber: lineNumber,
      endColumn: maxColumn
    });
    if (severity === 'error') {
      errors++;
    } else {
      warnings++;
    }
  };

  let lastHeadingLevel = 0;
  const headingTexts = new Map();
  const tokens = md.parse(content, {});

  tokens.forEach((token, idx) => {
    if (token.type !== 'heading_open') return;

    const level = Number(token.tag.slice(1));
    const nextToken = tokens[idx + 1];
    const lineNumber = token.map ? token.map[0] + 1 : 1;

    if (!nextToken || !nextToken.content || !nextToken.content.trim()) {
      pushMarker(lineNumber, '标题内容为空', 'warning');
    }

    if (lastHeadingLevel && level > lastHeadingLevel + 1) {
      pushMarker(
        lineNumber,
        `标题级别从 H${lastHeadingLevel} 跳到 H${level}`,
        'warning'
      );
    }

    const text = nextToken?.content?.trim();
    if (text) {
      const existed = headingTexts.get(text);
      if (existed) {
        pushMarker(
          lineNumber,
          `标题与第 ${existed} 行重复`,
          'warning'
        );
      } else {
        headingTexts.set(text, lineNumber);
      }
    }

    lastHeadingLevel = level;
  });

  const fenceLines = [];
  lines.forEach((line, index) => {
    const trimmed = line.trim();
    if (trimmed.startsWith('```')) {
      fenceLines.push(index + 1);
    }

    const openBracket = (line.match(/\[/g) || []).length;
    const linkStart = (line.match(/\]\(/g) || []).length;
    const closeParen = (line.match(/\)/g) || []).length;
    if (openBracket && (linkStart === 0 || closeParen < linkStart)) {
      pushMarker(index + 1, '可能存在不完整的链接语法', 'warning');
    }
  });

  if (fenceLines.length % 2 !== 0) {
    const lineNumber = fenceLines[fenceLines.length - 1];
    pushMarker(lineNumber, '代码块未正确闭合', 'error');
  }

  if (model) {
    monaco.editor.setModelMarkers(model, 'markdown-lint', markers);
  }

  return { errors, warnings };
}

// 计算行数
function countLines(text) {
  return text.split(/\r\n|\n|\r/).length;
}

const ensureMarkdownEditor = () => {
  if (!editor) {
    message.warning('没有可编辑的文档');
    return false;
  }
  const model = editor.getModel();
  if (!model) return false;
  const languageId = model.getLanguageId();
  if (languageId !== 'markdown') {
    message.warning('当前工具栏功能仅在 Markdown 文件中可用');
    return false;
  }
  return true;
};

const invokeMarkdownFormat = async (content) => {
  const response = await invoke('markdown_format', {
    request: {
      content,
      options: {
        max_line_width: 0,
        indent_width: 2,
        list_indent: 2,
        format_tables: true,
        format_lists: true,
        preserve_code_blocks: true,
        remove_extra_blank_lines: true,
        fix_extra_whitespace: false
      }
    }
  });

  const success = response?.success;
  const formatted = response?.formatted_content ?? response?.formattedContent;
  const error = response?.error;

  if (!success || typeof formatted !== 'string') {
    throw new Error(typeof error === 'string' ? error : 'Markdown 格式化失败');
  }

  return formatted;
};

const formatMarkdownDocument = async () => {
  if (!ensureMarkdownEditor()) return;

  const model = editor.getModel();
  if (!model) return;

  const original = model.getValue();

  try {
    suppressEditLocationRecording = true;
    const formatted = await invokeMarkdownFormat(original);

    if (formatted === original) {
      message.info('无需格式化');
      return;
    }

    editor.executeEdits('markdown-format', [
      {
        range: model.getFullModelRange(),
        text: formatted,
        forceMoveMarkers: true
      }
    ]);

    editor.focus();
  } catch (e) {
    console.error('Markdown 格式化失败:', e);
    message.error(e instanceof Error ? e.message : 'Markdown 格式化失败');
  } finally {
    suppressEditLocationRecording = false;
  }
};

const formatMarkdownSelection = async () => {
  if (!ensureMarkdownEditor()) return;

  const model = editor.getModel();
  if (!model) return;

  const selection = editor.getSelection();
  if (!selection || selection.isEmpty()) {
    message.info('请先选择需要格式化的内容');
    return;
  }

  const selectedText = model.getValueInRange(selection);
  if (!selectedText.trim()) {
    message.info('请先选择需要格式化的内容');
    return;
  }

  try {
    suppressEditLocationRecording = true;

    let formatted = await invokeMarkdownFormat(selectedText);
    if (!selectedText.endsWith('\n')) {
      formatted = formatted.replace(/\n$/, '');
    }

    editor.executeEdits('markdown-format-selection', [
      {
        range: selection,
        text: formatted,
        forceMoveMarkers: true
      }
    ]);

    editor.focus();
  } catch (e) {
    console.error('Markdown 格式化失败:', e);
    message.error(e instanceof Error ? e.message : 'Markdown 格式化失败');
  } finally {
    suppressEditLocationRecording = false;
  }
};

const wrapSelection = (prefix, suffix, placeholder = '') => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  const selectedText = model.getValueInRange(selection);
  const finalText = prefix + (selectedText || placeholder) + suffix;

  editor.executeEdits('markdown-toolbar', [
    {
      range: selection,
      text: finalText,
      forceMoveMarkers: true
    }
  ]);

  if (!selectedText && placeholder) {
    const startLineNumber = selection.startLineNumber;
    const startColumn = selection.startColumn + prefix.length;
    const endColumn = startColumn + placeholder.length;
    editor.setSelection(
      new monaco.Selection(
        startLineNumber,
        startColumn,
        startLineNumber,
        endColumn
      )
    );
  }

  editor.focus();
};

const insertBlockSnippet = (snippet) => {
  if (!ensureMarkdownEditor()) return;
  const position = editor.getPosition();
  const range = new monaco.Range(
    position.lineNumber,
    1,
    position.lineNumber,
    1
  );

  editor.executeEdits('markdown-toolbar', [
    {
      range,
      text: snippet,
      forceMoveMarkers: true
    }
  ]);

  editor.focus();
};

const toggleHeading = (level) => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection) return;

  const startLine = selection.startLineNumber;
  const endLine = selection.endLineNumber;
  const edits = [];

  for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
    const text = model.getLineContent(lineNumber);
    const content = text.replace(/^#{1,6}\s+/, '');

    let next = content;
    if (level > 0) {
      const sameLevel = new RegExp(`^#{${level}}\\s+`).test(text);
      next = sameLevel ? content : `${'#'.repeat(level)} ${content || '标题'}`;
    }

    edits.push({
      range: new monaco.Range(lineNumber, 1, lineNumber, model.getLineMaxColumn(lineNumber)),
      text: next,
      forceMoveMarkers: true
    });
  }

  editor.executeEdits('markdown-toolbar', edits);
  editor.focus();
};

const handleHeadingSelect = (key) => {
  const level = Number(key);
  if (Number.isNaN(level)) return;
  toggleHeading(level);
};

const toggleBold = () => {
  wrapSelection('**', '**', '粗体文本');
};

const toggleItalic = () => {
  wrapSelection('*', '*', '斜体文本');
};

const toggleStrikethrough = () => {
  wrapSelection('~~', '~~', '删除线');
};

const toggleInlineCode = () => {
  wrapSelection('`', '`', 'code');
};

const insertLink = () => {
  wrapSelection('[', '](https://)', '链接文字');
};

const toggleLinePrefix = (prefix, pattern, buildLine) => {
  if (!ensureMarkdownEditor()) return;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection) return;

  const startLine = selection.startLineNumber;
  const endLine = selection.endLineNumber;
  const lines = [];
  let allMatch = true;

  for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
    const text = model.getLineContent(lineNumber);
    lines.push(text);
    if (!pattern.test(text)) allMatch = false;
  }

  const shouldMoveCursor = selection.isEmpty() && startLine === endLine && (!lines[0].trim() || selection.startColumn === 1);

  const edits = lines.map((text, index) => {
    const lineNumber = startLine + index;
    const next = allMatch
      ? text.replace(pattern, '')
      : (buildLine ? buildLine(text, index) : `${prefix}${text}`);

    return {
      range: new monaco.Range(lineNumber, 1, lineNumber, model.getLineMaxColumn(lineNumber)),
      text: next,
      forceMoveMarkers: true
    };
  });

  editor.executeEdits('markdown-toolbar', edits);

  if (shouldMoveCursor && !allMatch) {
    const nextLine = model.getLineContent(startLine);
    const m = nextLine.match(pattern);
    const nextColumn = m ? Math.max(1, m[0].length + 1) : 1;
    editor.setPosition(new monaco.Position(startLine, nextColumn));
  }

  editor.focus();
};

const toggleBlockquote = () => {
  toggleLinePrefix('> ', /^>\s+/, null);
};

const toggleUnorderedList = () => {
  toggleLinePrefix('- ', /^-\s+/, null);
};

const toggleOrderedList = () => {
  toggleLinePrefix(
    '1. ',
    /^\d+\.\s+/,
    (text, index) => `${index + 1}. ${text}`
  );
};

const toggleTaskList = () => {
  toggleLinePrefix('- [ ] ', /^-\s\[( |x|X)\]\s+/, null);
};

const isMarkdownListDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_md_list') || '').trim().toLowerCase();
    return v === '1' || v === 'true' || v === 'yes' || v === 'on';
  } catch {
    return false;
  }
};

const parseMarkdownListPrefix = (line) => {
  const text = String(line || '');

  const task = /^([\t ]*)([-+*])\s+\[( |x|X)\]\s+/.exec(text);
  if (task) {
    return {
      kind: 'task',
      indent: task[1] || '',
      marker: task[2] || '-',
      checked: (task[3] || '').toLowerCase() === 'x',
      prefix: task[0]
    };
  }

  const ordered = /^([\t ]*)(\d+)\.\s+/.exec(text);
  if (ordered) {
    return {
      kind: 'ordered',
      indent: ordered[1] || '',
      number: Number(ordered[2] || 1) || 1,
      prefix: ordered[0]
    };
  }

  const unordered = /^([\t ]*)([-+*])\s+/.exec(text);
  if (unordered) {
    return {
      kind: 'unordered',
      indent: unordered[1] || '',
      marker: unordered[2] || '-',
      prefix: unordered[0]
    };
  }

  return null;
};

const handleMarkdownListEnter = (e) => {
  if (!editor) return false;

  const model = editor.getModel();
  const pos = editor.getPosition();
  const selection = editor.getSelection();
  if (!model || !pos || !selection || !selection.isEmpty()) return false;

  if (model.getLanguageId() !== 'markdown') return false;

  const lineText = model.getLineContent(pos.lineNumber);
  const parsed = parseMarkdownListPrefix(lineText);
  if (!parsed) return false;

  if (e.shiftKey || e.altKey || e.ctrlKey || e.metaKey) return false;

  const afterPrefix = lineText.slice(parsed.prefix.length);
  const isEmptyItem = afterPrefix.trim().length === 0 && pos.column >= parsed.prefix.length + 1;

  e.preventDefault();
  e.stopPropagation();

  if (isMarkdownListDebug()) {
    console.debug('[MarkdownList] enter', {
      lineNumber: pos.lineNumber,
      kind: parsed.kind,
      isEmptyItem
    });
  }

  if (isEmptyItem) {
    const fullRange = new monaco.Range(pos.lineNumber, 1, pos.lineNumber, model.getLineMaxColumn(pos.lineNumber));
    editor.executeEdits('md-list-enter-exit', [
      {
        range: fullRange,
        text: `${parsed.indent}\n${parsed.indent}`,
        forceMoveMarkers: true
      }
    ]);
    editor.setPosition(new monaco.Position(pos.lineNumber + 1, parsed.indent.length + 1));
    editor.focus();
    return true;
  }

  let nextPrefix = '';
  if (parsed.kind === 'ordered') {
    nextPrefix = `${parsed.indent}${parsed.number + 1}. `;
  } else if (parsed.kind === 'task') {
    nextPrefix = `${parsed.indent}${parsed.marker} [ ] `;
  } else {
    nextPrefix = `${parsed.indent}${parsed.marker} `;
  }

  editor.executeEdits('md-list-enter-continue', [
    {
      range: new monaco.Range(pos.lineNumber, pos.column, pos.lineNumber, pos.column),
      text: `\n${nextPrefix}`,
      forceMoveMarkers: true
    }
  ]);

  editor.setPosition(new monaco.Position(pos.lineNumber + 1, nextPrefix.length + 1));
  editor.focus();
  return true;
};

const isPotentialTableLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  const pipeCount = (trimmed.match(/\|/g) || []).length;
  return pipeCount >= 2;
};

const isTableSeparatorLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  return /^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?$/.test(trimmed);
};

const normalizeTableRowCells = (line) => {
  const raw = (line || '').trim();
  let text = raw;
  if (!text.startsWith('|')) text = `| ${text}`;
  if (!text.endsWith('|')) text = `${text} |`;
  const parts = text.split('|');
  return parts.slice(1, -1).map((c) => c.trim());
};

const buildTableRow = (cells) => {
  return `| ${cells.map((c) => (c ?? '').trim()).join(' | ')} |`;
};

const detectSeparatorAlign = (cell) => {
  const v = String(cell || '').trim();
  const left = v.startsWith(':');
  const right = v.endsWith(':');
  if (left && right) return 'center';
  if (right) return 'right';
  if (left) return 'left';
  return null;
};

const normalizeSeparatorCell = (cell, align) => {
  const base = (cell || '').trim();
  const hasMin = base.replace(/:/g, '').replace(/-/g, '').length === 0;
  const detected = align || detectSeparatorAlign(base);

  if (!hasMin) {
    if (detected === 'left') return ':---';
    if (detected === 'center') return ':---:';
    if (detected === 'right') return '---:';
    return '---';
  }

  if (detected === 'left') return ':---';
  if (detected === 'center') return ':---:';
  if (detected === 'right') return '---:';
  return '---';
};

// 判断 Unicode 组合附加符号（Combining Marks）。
// 这类字符通常叠加在前一个字符上显示，因此在“可视宽度”计算中按 0 宽度处理。
const isCombiningCodePoint = (cp) => {
  return (
    (cp >= 0x0300 && cp <= 0x036f) ||
    (cp >= 0x1ab0 && cp <= 0x1aff) ||
    (cp >= 0x1dc0 && cp <= 0x1dff) ||
    (cp >= 0x20d0 && cp <= 0x20ff) ||
    (cp >= 0xfe20 && cp <= 0xfe2f)
  );
};

// 判断 Unicode 码点是否为“全宽”字符。
// 用于近似估算等宽字体下的显示宽度：CJK/全角标点/emoji 等通常按 2 列宽处理。
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
  )) return true;

  return false;
};

// 计算字符串的“可视宽度”（用于 Markdown 表格源码对齐）。
// 规则：控制字符忽略；组合字符按 0；全宽字符按 2；其它按 1。
// 说明：这是为了提升中英文混排时 | 的对齐观感，而不是严格的排版引擎实现。
const getDisplayWidth = (input) => {
  const s = String(input ?? '');
  let width = 0;

  for (const ch of s) {
    const cp = ch.codePointAt(0) || 0;

    if (cp === 0) continue;
    if (cp < 32 || cp === 0x7f) continue;
    if (isCombiningCodePoint(cp)) continue;

    width += isFullWidthCodePoint(cp) ? 2 : 1;
  }

  return width;
};

// 按列宽对齐单元格：用“可视宽度”补齐空格，而不是用 string.length。
// 这样在 Monaco 编辑器的等宽字体下，中英文混排时各行的竖线更容易对齐。
const padTableCell = (text, width, align) => {
  const raw = String(text ?? '').trim();
  const current = getDisplayWidth(raw);
  const w = Math.max(0, Number(width) || 0);
  if (current >= w) return raw;

  const delta = w - current;
  if (align === 'right') return `${' '.repeat(delta)}${raw}`;
  if (align === 'center') {
    const left = Math.floor(delta / 2);
    const right = delta - left;
    return `${' '.repeat(left)}${raw}${' '.repeat(right)}`;
  }
  return `${raw}${' '.repeat(delta)}`;
};

// 将表格的二维 cells 渲染为 Markdown 行，并根据列宽补空格。
// 目标：在源码编辑态也能保持每列 | 对齐，提升可读性。
const buildAlignedTableLines = (rows) => {
  const safeRows = Array.isArray(rows) ? rows : [];
  if (!safeRows.length) return [];

  const colCount = safeRows.reduce((m, r) => Math.max(m, Array.isArray(r) ? r.length : 0), 0) || 1;
  const sepRow = safeRows[1] || [];
  const aligns = Array.from({ length: colCount }, (_, i) => detectSeparatorAlign(sepRow[i]));

  const widths = Array.from({ length: colCount }, () => 3);
  safeRows.forEach((row, rowIndex) => {
    if (!Array.isArray(row) || rowIndex === 1) return;
    for (let i = 0; i < colCount; i += 1) {
      const v = String(row[i] ?? '').trim();
      widths[i] = Math.max(widths[i], getDisplayWidth(v));
    }
  });

  return safeRows.map((row, rowIndex) => {
    const cells = Array.from({ length: colCount }, (_, i) => (Array.isArray(row) ? row[i] : ''));

    if (rowIndex === 1) {
      const sepCells = cells.map((c, i) => {
        const a = aligns[i];
        const w = Math.max(3, widths[i]);
        const dashes = '-'.repeat(w);
        if (a === 'left') return `:${dashes}`;
        if (a === 'center') return `:${dashes}:`;
        if (a === 'right') return `${dashes}:`;
        return dashes;
      });
      return `| ${sepCells.join(' | ')} |`;
    }

    const padded = cells.map((c, i) => padTableCell(c, widths[i], aligns[i]));
    return `| ${padded.join(' | ')} |`;
  });
};

const getColumnIndexFromCursor = (line, column, colCount) => {
  const text = line || '';
  const positions = [];
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '|') positions.push(i);
  }
  if (positions.length < 2) return 0;

  const cursor = Math.max(0, Math.min(text.length, (column || 1) - 1));

  if (cursor <= positions[0]) return 0;
  if (cursor > positions[positions.length - 1]) return Math.max(0, (colCount || positions.length - 1) - 1);

  for (let i = 0; i < positions.length - 1; i += 1) {
    if (cursor > positions[i] && cursor <= positions[i + 1]) {
      return Math.max(0, Math.min((colCount || positions.length - 1) - 1, i));
    }
  }

  return 0;
};

const detectTableContext = () => {
  if (!editor) return null;
  const model = editor.getModel();
  if (!model) return null;
  if (model.getLanguageId() !== 'markdown') return null;

  const pos = editor.getPosition();
  if (!pos) return null;

  const lineNumber = pos.lineNumber;
  const lineText = model.getLineContent(lineNumber);
  if (!isPotentialTableLine(lineText)) return null;

  const total = model.getLineCount();
  let start = lineNumber;
  while (start > 1 && isPotentialTableLine(model.getLineContent(start - 1))) {
    start -= 1;
  }

  let end = lineNumber;
  while (end < total && isPotentialTableLine(model.getLineContent(end + 1))) {
    end += 1;
  }

  let separatorLine = -1;
  for (let i = start; i <= end; i += 1) {
    if (isTableSeparatorLine(model.getLineContent(i))) {
      separatorLine = i;
      break;
    }
  }

  if (separatorLine === -1 || separatorLine === start) return null;

  const tableStart = separatorLine - 1;

  const rawLines = [];
  for (let i = tableStart; i <= end; i += 1) {
    rawLines.push(model.getLineContent(i));
  }

  const rows = rawLines.map((l) => normalizeTableRowCells(l));
  const colCount = rows.reduce((m, r) => Math.max(m, r.length), 0) || 1;
  const padded = rows.map((r) => {
    const next = [...r];
    while (next.length < colCount) next.push('');
    return next;
  });

  const activeRowIndex = lineNumber - tableStart;
  const cursorColIndex = getColumnIndexFromCursor(rawLines[Math.max(0, Math.min(rawLines.length - 1, activeRowIndex))], pos.column, colCount);

  return {
    startLine: tableStart,
    endLine: end,
    separatorLine,
    colCount,
    rows: padded,
    activeRowIndex,
    activeColIndex: cursorColIndex
  };
};

let tableAutoFormatBusy = false;

const isTableAutoFormatDebug = () => {
  try {
    const v = String(window?.localStorage?.getItem('guaxi_debug_md_table') || '').trim().toLowerCase();
    return v === '1' || v === 'true' || v === 'yes' || v === 'on';
  } catch {
    return false;
  }
};

const scheduleAutoFormatActiveTable = debounce(() => {
  if (!editor) return;
  if (tableAutoFormatBusy) return;

  const model = editor.getModel();
  const selection = editor.getSelection();
  const pos = editor.getPosition();
  if (!model || !pos || !selection || !selection.isEmpty()) return;
  if (model.getLanguageId() !== 'markdown') return;

  const ctx = detectTableContext();
  if (!ctx) return;

  const rawLines = [];
  for (let i = ctx.startLine; i <= ctx.endLine; i += 1) {
    rawLines.push(model.getLineContent(i));
  }

  const nextRows = ctx.rows.map((row, idx) => {
    if (idx !== 1) return row;
    return row.map((c) => normalizeSeparatorCell(c, null));
  });

  const nextLines = buildAlignedTableLines(nextRows);
  const currentText = rawLines.join('\n');
  const nextText = nextLines.join('\n');
  if (currentText === nextText) return;

  const activeRowIndex = Math.max(0, Math.min(rawLines.length - 1, pos.lineNumber - ctx.startLine));
  const activeColIndex = getColumnIndexFromCursor(rawLines[activeRowIndex] || '', pos.column, ctx.colCount);

  const currentCellColumn = getCursorColumnForTableCell(rawLines[activeRowIndex] || '', activeColIndex, 0);
  const offset = Math.max(0, (pos.column || 1) - currentCellColumn);

  if (isTableAutoFormatDebug()) {
    console.debug('[MarkdownTable] auto-format', {
      startLine: ctx.startLine,
      endLine: ctx.endLine,
      activeRowIndex,
      activeColIndex,
      offset
    });
  }

  tableAutoFormatBusy = true;
  try {
    const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine));
    editor.executeEdits('md-table-auto-format', [
      {
        range,
        text: nextText,
        forceMoveMarkers: true
      }
    ]);

    const targetText = nextLines[activeRowIndex] || '';
    const targetCol = getCursorColumnForTableCell(targetText, activeColIndex, offset);
    editor.setPosition(new monaco.Position(ctx.startLine + activeRowIndex, targetCol));
  } finally {
    nextTick(() => {
      tableAutoFormatBusy = false;
      updateTableToolbar();
    });
  }
}, 220);

const updateTableToolbar = () => {
  if (!editor) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const model = editor.getModel();
  if (!model || model.getLanguageId() !== 'markdown') {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const ctx = detectTableContext();
  if (!ctx) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const wrapperEl = editorWrapperRef.value;
  const editorEl = editorContainer.value;
  if (!wrapperEl || !editorEl) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const wrapperRect = wrapperEl.getBoundingClientRect();
  const editorRect = editorEl.getBoundingClientRect();

  const anchorPos = new monaco.Position(ctx.startLine, 1);
  const visiblePos = editor.getScrolledVisiblePosition(anchorPos);
  if (!visiblePos) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const headerEl = editorHeaderRef.value;
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0;
  const minTop = headerHeight + 8;

  tableToolbarStyle.value = {
    left: `${Math.max(8, editorRect.left - wrapperRect.left + 8)}px`,
    top: `${Math.max(minTop, editorRect.top - wrapperRect.top + visiblePos.top - 36)}px`
  };

  activeTableContext.value = ctx;
  tableToolbarVisible.value = true;
};

const getPipePositions = (line) => {
  const text = String(line || '');
  const positions = [];
  for (let i = 0; i < text.length; i += 1) {
    if (text[i] === '|') positions.push(i);
  }
  return positions;
};

const getCursorColumnForTableCell = (line, colIndex, offset = 0) => {
  const text = String(line || '');
  const pipes = getPipePositions(text);
  if (pipes.length < 2) return 1;

  const col = Math.max(0, Math.min(pipes.length - 2, Number(colIndex) || 0));
  const left = pipes[col];
  const right = pipes[col + 1];
  const cellStart = Math.min(text.length, left + 2);
  const cellEnd = Math.max(cellStart, right - 1);
  const off = Math.max(0, Math.min(cellEnd - cellStart, Number(offset) || 0));

  return Math.max(1, Math.min(text.length + 1, cellStart + off + 1));
};

const applyTableEdits = (nextRows, nextActiveRowIndex, nextActiveColIndex) => {
  if (!editor) return;
  const model = editor.getModel();
  const ctx = activeTableContext.value;
  if (!model || !ctx) return;

  const normalizedRows = nextRows.map((row, idx) => {
    if (idx !== 1) return row;
    return row.map((c) => normalizeSeparatorCell(c, null));
  });

  const textLines = buildAlignedTableLines(normalizedRows);

  const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine));
  editor.executeEdits('md-table-toolbar', [
    {
      range,
      text: textLines.join('\n'),
      forceMoveMarkers: true
    }
  ]);

  const row = Math.max(2, Math.min(textLines.length - 1, nextActiveRowIndex));
  const col = Math.max(0, Math.min((normalizedRows[0]?.length || 1) - 1, nextActiveColIndex));
  const targetLine = ctx.startLine + row;
  const targetText = textLines[row] || '';
  const targetCol = getCursorColumnForTableCell(targetText, col, 0);

  editor.setPosition(new monaco.Position(targetLine, targetCol));
  editor.focus();

  nextTick(() => {
    updateTableToolbar();
  });
};

const addTableRowBelow = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const insertAt = ctx.activeRowIndex <= 1 ? 2 : Math.min(ctx.rows.length, ctx.activeRowIndex + 1);
  const empty = Array.from({ length: ctx.colCount }, () => '');

  const nextRows = [...ctx.rows];
  nextRows.splice(insertAt, 0, empty);

  applyTableEdits(nextRows, insertAt, ctx.activeColIndex);
};

const deleteTableRow = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  if (ctx.activeRowIndex <= 1) {
    message.warning('无法删除表头或分隔行');
    return;
  }

  const nextRows = [...ctx.rows];
  if (nextRows.length <= 2) return;

  nextRows.splice(ctx.activeRowIndex, 1);
  const nextRow = Math.min(nextRows.length - 1, ctx.activeRowIndex);

  applyTableEdits(nextRows, nextRow, ctx.activeColIndex);
};

const addTableColumnRight = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const insertAt = Math.min(ctx.colCount, ctx.activeColIndex + 1);
  const nextRows = ctx.rows.map((row, rowIndex) => {
    const next = [...row];
    if (rowIndex === 1) {
      next.splice(insertAt, 0, '---');
    } else {
      next.splice(insertAt, 0, '');
    }
    return next;
  });

  const nextCol = insertAt;
  applyTableEdits(nextRows, ctx.activeRowIndex, nextCol);
};

const deleteTableColumn = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  if (ctx.colCount <= 1) {
    message.warning('至少保留 1 列');
    return;
  }

  const removeAt = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex));
  const nextRows = ctx.rows.map((row) => {
    const next = [...row];
    next.splice(removeAt, 1);
    return next;
  });

  const nextCol = Math.max(0, Math.min(ctx.colCount - 2, removeAt));
  applyTableEdits(nextRows, ctx.activeRowIndex, nextCol);
};

const setTableColumnAlign = (align) => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const col = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex));
  const nextRows = ctx.rows.map((row, rowIndex) => {
    if (rowIndex !== 1) return row;
    const next = [...row];
    next[col] = normalizeSeparatorCell(next[col], align);
    return next;
  });

  applyTableEdits(nextRows, ctx.activeRowIndex, ctx.activeColIndex);
};

const formatTable = () => {
  const ctx = activeTableContext.value;
  if (!ctx || !ctx.rows?.length) return;

  const nextRows = ctx.rows.map((row, rowIndex) => {
    if (rowIndex === 1) {
      return row.map((c) => normalizeSeparatorCell(c, null));
    }
    return row.map((c) => (c ?? '').trim());
  });

  applyTableEdits(nextRows, ctx.activeRowIndex, ctx.activeColIndex);
};

const insertTable = () => {
  if (!ensureMarkdownEditor()) return;

  const pos = editor.getPosition();
  insertBlockSnippet('| 列1 | 列2 |\n| --- | --- |\n| 内容1 | 内容2 |\n');

  if (pos) {
    nextTick(() => {
      editor.setPosition(new monaco.Position(pos.lineNumber + 2, 3));
      editor.focus();
      updateTableToolbar();
    });
  }
};

const insertImage = () => {
  insertBlockSnippet('![图片描述](路径或URL)\n');
};

const insertCodeBlockFromToolbar = () => {
  wrapSelection('```\n', '\n```\n', '代码');
};

const insertHorizontalRule = () => {
  insertBlockSnippet('---\n');
};

const undo = () => {
  if (!editor) return;
  editor.focus();
  editor.trigger('toolbar', 'undo', null);
};

const redo = () => {
  if (!editor) return;
  editor.focus();
  editor.trigger('toolbar', 'redo', null);
};

const getActiveEditor = () => {
  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.activeRegion === 'right') {
    return secondaryEditor;
  }
  return editor;
};

const activateRegion = (region) => {
  if (fileTreeStore.layoutMode === 'single') return;

  const desired = region === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft;
  if (desired) {
    fileTreeStore.setActiveTab(desired, region);
    return;
  }

  fileTreeStore.setActiveRegion(region);
};

// 更新编辑器内容
const updateEditorContent = (content) => {
  const nextContent = content ?? '';
  rawContent.value = nextContent;

  if (!editor) {
    initEditor();
  }

  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.activeRegion === 'right') {
    ensureSecondaryEditor();
  }

  const targetEditor = getActiveEditor();
  if (!targetEditor) return;

  targetEditor.updateOptions({
    readOnly: !!activeTab.value?.unsupported,
  });

  const model = targetEditor.getModel();
  const currentValue = model ? model.getValue() : targetEditor.getValue();
  if (currentValue === nextContent) return;

  if (model) {
    model.setValue(nextContent);
  } else {
    targetEditor.setValue(nextContent);
  }
};

const AUTO_SAVE_FALLBACK_INTERVAL_MS = 5 * 60 * 1000
let autoSaveTimerId = null
const autoSaving = ref(false)

const autoSaveDirtyTabs = async () => {
  if (autoSaving.value) return
  if (editorStore.autoSaveEnabled === false) return
  if (!fileTreeStore.openTabs.length) return

  autoSaving.value = true
  try {
    const activePath = fileTreeStore.activeTab
    if (activePath) {
      const targetEditor = getActiveEditor()
      const activeContent =
        previewMode.value === 'realtime' && realtimeVditor
          ? realtimeVditor.getValue()
          : targetEditor
            ? targetEditor.getValue()
            : rawContent.value
      fileTreeStore.updateTabContent(activePath, activeContent)
    }

    const dirtyTabs = fileTreeStore.openTabs.filter((tab) => tab?.dirty)
    if (!dirtyTabs.length) return

    for (const tab of dirtyTabs) {
      if (!tab?.path) continue
      if (tab.unsupported) continue
      await fileTreeStore.updateFileContent(tab.path, tab.content)
    }

    await updateGitStatus();
    editorStore.setLastAutoSaveAt(Date.now())
  } catch (e) {
    console.error('自动保存失败:', e)
  } finally {
    autoSaving.value = false
  }
}

// 保存内容
const saveContent = async () => {
  if (!selectedFile.value) return;

  if (activeTab.value?.unsupported) {
    message.warning('不支持编辑/保存该文件类型');
    return;
  }

  const targetEditor = getActiveEditor();
  const content =
    previewMode.value === 'realtime' && realtimeVditor
      ? realtimeVditor.getValue()
      : targetEditor
        ? targetEditor.getValue()
        : rawContent.value;

  await fileTreeStore.updateFileContent(selectedFile.value.path, content);
  await updateGitStatus();
  message.info('修改已保存');

  if (rawContent.value !== content) {
    rawContent.value = content;
  }

  if (previewMode.value === 'split' && canPreview.value) {
    if (lastPreviewRenderedSource.value !== content) {
      renderDebounced(content);
    }
  }
};

// 快捷键处理
const handleKeyDown = (e) => {
  if (e.ctrlKey && e.key === 's') {
    e.preventDefault();
    saveContent();
    return;
  }

  if (e.ctrlKey && !e.altKey && !e.metaKey) {
    if (e.key === '1') {
      e.preventDefault();
      handlePreviewModeSelect('edit');
      return;
    }

    if (e.key === '2') {
      e.preventDefault();
      handlePreviewModeSelect('preview');
      return;
    }

    if (e.key === '3') {
      e.preventDefault();
      handlePreviewModeSelect('split');
      return;
    }

    if (e.key === '4') {
      e.preventDefault();
      handlePreviewModeSelect('realtime');
      return;
    }
  }

  if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
    const target = e.target;
    const tagName = (target?.tagName || '').toLowerCase();
    const isEditable =
      tagName === 'input' ||
      tagName === 'textarea' ||
      target?.isContentEditable;

    if (isEditable) return;

    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      navigateToPreviousEditLocation();
      return;
    }

    if (e.key === 'ArrowRight') {
      e.preventDefault();
      navigateToNextEditLocation();
      return;
    }
  }
};

const getRealtimeScrollElement = () => {
  const root = document.getElementById(realtimeVditorId);
  if (!root) return null;
  return root.querySelector('.vditor-content') || root;
};

const captureScrollPosition = () => {
  if (previewMode.value === 'realtime') {
    const el = getRealtimeScrollElement();
    if (el) {
      const height = el.scrollHeight;
      scrollRatio.value = height ? el.scrollTop / height : 0;
      return;
    }
  }

  if (mode.value === 'edit' && editor) {
    const top = editor.getScrollTop();
    const height = editor.getScrollHeight();
    scrollRatio.value = height ? top / height : 0;
  } else if (mode.value === 'preview' && previewContainer.value) {
    const el = previewContainer.value;
    const height = el.scrollHeight;
    scrollRatio.value = height ? el.scrollTop / height : 0;
  } else {
    scrollRatio.value = 0;
  }
};

const restoreRealtimeScroll = () => {
  const el = getRealtimeScrollElement();
  if (!el) {
    pendingRealtimeScrollRestore.value = true;
    return;
  }
  const height = el.scrollHeight;
  el.scrollTop = height * scrollRatio.value;
  pendingRealtimeScrollRestore.value = false;
};

const restoreScrollPosition = () => {
  if (previewMode.value === 'realtime') {
    restoreRealtimeScroll();
    return;
  }

  if (mode.value === 'preview') {
    pendingPreviewScrollRestore.value = true;
  } else if (mode.value === 'edit' && editor) {
    nextTick(() => {
      const height = editor.getScrollHeight();
      const targetTop = height * scrollRatio.value;
      editor.setScrollTop(targetTop);
    });
  }
};

const scrollPreviewToLine = (lineNumber) => {
  if (!previewContainer.value || !lineNumber) return;

  const container = previewContainer.value;
  const elements = Array.from(
    container.querySelectorAll('[data-md-start-line]')
  );

  if (!elements.length) return;

  let target = elements[0];
  let minDelta = Infinity;

  elements.forEach((el) => {
    const start = Number(el.dataset.mdStartLine || 0);
    const end = Number(el.dataset.mdEndLine || start);
    if (!start && !end) return;

    let delta = 0;
    if (lineNumber < start) {
      delta = start - lineNumber;
    } else if (lineNumber > end) {
      delta = lineNumber - end;
    } else {
      delta = 0;
    }

    if (delta < minDelta) {
      minDelta = delta;
      target = el;
    }
  });

  if (!target) return;

  const containerRect = container.getBoundingClientRect();
  const targetRect = target.getBoundingClientRect();
  const offset = targetRect.top - containerRect.top;
  container.scrollTop += offset - container.clientHeight * 0.2;
};

const syncEditorToLine = (lineNumber, { focus = true } = {}) => {
  if (!lineNumber) return;
  if (!editor) {
    initEditor();
  }
  if (!editor) return;

  editor.revealLineInCenter(lineNumber);
  editor.setPosition({ lineNumber, column: 1 });
  if (focus) {
    editor.focus();
  }
};

const handlePreviewClick = (e) => {
  const base = e?.target instanceof Element ? e.target : e?.target?.parentElement;
  if (!base) return;
  const hit = base.closest('[data-md-start-line]');
  if (!hit) return;

  const line = Number(hit.dataset.mdStartLine || 1);
  const shouldFocus = showEditor.value;
  syncEditorToLine(line, { focus: shouldFocus });
};

const setMainMode = async (nextMode) => {
  if (nextMode !== 'edit' && nextMode !== 'preview') return;
  if (mode.value === nextMode) return;

  if (mode.value === 'edit' && nextMode === 'preview' && editor) {
    const pos = editor.getPosition();
    pendingPreviewScrollLine.value =
      (pos && pos.lineNumber) || editorStore.cursorPosition.lineNumber || 1;
  }

  captureScrollPosition();
  mode.value = nextMode;

  await nextTick();

  if (mode.value === 'edit') {
    if (!editor) {
      initEditor();
    }
    if (editor) {
      editor.layout();
    }
    restoreScrollPosition();
    return;
  }

  renderDebounced(rawContent.value);
  restoreScrollPosition();
};

// 防抖渲染
const renderDebounced = debounce(async (content) => {
  if (!selectedFile.value || !mode.value || !content || !canPreview.value) {
    renderedHtml.value = '';
    lastPreviewRenderedSource.value = '';
    return;
  }

  try {
    const basePath = selectedFile.value.path ? await dirname(selectedFile.value.path) : '';
    renderedHtml.value = await renderMarkdown(content, {
      basePath: basePath,
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default'
    });

    lastPreviewRenderedSource.value = content;

    // 确保DOM更新后添加按钮
    nextTick(() => {
      if (previewContainer.value) {
        addCopyButtons(previewContainer.value);
      }
    });
  } catch (error) {
    console.error('渲染错误:', error);
    renderedHtml.value = `
      <div class="render-error">
        <p>渲染错误: ${error instanceof Error ? error.message : String(error)}</p>
        <button @click="retryRender">重试</button>
      </div>
    `;
  }
}, 300);

const canPreview = computed(() => {
  const path = selectedFile.value?.path || '';
  const ext = path.split('.').pop()?.toLowerCase();
  if (ext === 'md' || ext === 'markdown') return true;
  return editorStore.language === 'markdown';
});

const lastSelectedFilePath = ref(null)

// 监听文件选择
watch(selectedFile, async (newFile) => {
  if (newFile) {
    const nextPath = newFile.path || '';
    const prevPath = lastSelectedFilePath.value || '';

    const isSameFile = !!nextPath && !!prevPath && nextPath === prevPath;
    if (isSameFile) {
      const nextContent = newFile.content || '';
      const targetEditor = getActiveEditor();
      const currentContent =
        previewMode.value === 'realtime' && realtimeVditor
          ? realtimeVditor.getValue()
          : targetEditor
            ? targetEditor.getValue()
            : rawContent.value;

      if (currentContent === nextContent) {
        if (rawContent.value !== nextContent) {
          rawContent.value = nextContent;
        }
        return;
      }
    }

    lastSelectedFilePath.value = nextPath;

    const fileExt = newFile.path?.split('.').pop() || 'markdown';
    const languageMap = {
      'js': 'javascript',
      'ts': 'typescript',
      'css': 'css',
      'html': 'html',
      'htm': 'html',
      'md': 'markdown',
      'py': 'python',
      'cpp': 'cpp',
      'c': 'cpp',
      'h': 'cpp',
      'hpp': 'cpp',
      'rs': 'rust',
      'sh': 'shell',
      'jsx': 'javascript',
      'tsx': 'javascript',
      'json': 'json',
      'yaml': 'yaml',
      'yml': 'yaml',
      'lua': 'lua',
      'java': 'java',
      'toml': 'ini',
    };

    const language = languageMap[fileExt] || 'markdown';

    // 更新内容
    rawContent.value = newFile.content || '';

    if (previewMode.value === 'realtime' && realtimeVditor) {
      const nextValue = rawContent.value || '';
      if (!realtimeVditorReady.value) {
        pendingRealtimeVditorValue.value = nextValue;
      } else {
        settingRealtimeVditorValue = true;
        try {
          realtimeVditor.setValue(nextValue);
        } finally {
          settingRealtimeVditorValue = false;
        }
      }
    }

    updateEditorContent(rawContent.value);

    const targetEditor = getActiveEditor();
    if (targetEditor) {
      const model = targetEditor.getModel();
      if (model) {
        monaco.editor.setModelLanguage(model, language);
      }
    }

    if (!selectedFile.value || !mode.value) {
      renderedHtml.value = ''
      return
    }

    if (canPreview.value && (mode.value === 'preview' || previewMode.value !== 'toggle')) {
      renderDebounced(newFile.content)
    }

    await initGitService()
  } else {
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue('');
    }
    if (secondaryEditor) {
      secondaryEditor.setValue('');
      secondaryEditor.getModel().setValue('');
    }
    renderedHtml.value = '';
    return;
  }
}, { immediate: true });

const previewModeOptions = [
  { label: '编辑模式 (Ctrl+1)', key: 'edit' },
  { label: '预览模式 (Ctrl+2)', key: 'preview' },
  { label: '分屏预览 (Ctrl+3)', key: 'split' },
  { label: '实时预览 (Ctrl+4)', key: 'realtime' }
];

const previewModeLabel = computed(() => {
  if (previewMode.value === 'toggle') {
    return mode.value === 'edit' ? '编辑模式' : '预览模式';
  }
  return previewMode.value === 'split' ? '分屏预览' : '实时预览';
});

const previewModeIcon = computed(() => {
  if (previewMode.value === 'toggle') {
    return mode.value === 'edit' ? filePenLineIcon : eyeIcon;
  }
  return previewMode.value === 'split' ? columns2Icon : sparklesIcon;
});

const showEditor = computed(() => {
  if (!selectedFile.value) return false;
  if (previewMode.value === 'realtime') return false;
  return previewMode.value === 'toggle' ? mode.value === 'edit' : true;
});

const showPreview = computed(() => {
  if (!selectedFile.value || !canPreview.value) return false;
  if (previewMode.value === 'realtime') return false;
  return previewMode.value === 'toggle' ? mode.value === 'preview' : true;
});

watch(
  () => editorStore.cursorPosition?.lineNumber,
  (line) => {
    if (!line) return;
    if (showPreview.value) {
      scrollPreviewToLine(line);
    }
  },
  { flush: 'post' }
);

const shouldAutoRenderPreview = computed(() => {
  if (!selectedFile.value || !canPreview.value) return false;
  return previewMode.value === 'toggle' && mode.value === 'preview';
});

const refreshPreview = () => {
  renderDebounced(rawContent.value);
};

const initRealtimeVditor = async () => {
  if (!selectedFile.value || !canPreview.value) return;
  if (previewMode.value !== 'realtime') return;

  await nextTick();

  const desiredValue = rawContent.value || '';

  if (!realtimeVditor) {
    realtimeVditorReady.value = false;
    pendingRealtimeVditorValue.value = null;

    realtimeVditor = new Vditor(realtimeVditorId, {
      mode: 'ir',
      cache: { enable: false },
      preview: {
        delay: 200,
        hljs: { enable: false }
      },
      toolbarConfig: { hide: false },
      toolbar: [
        'headings',
        'bold',
        'italic',
        'strike',
        'link',
        '|',
        'list',
        'ordered-list',
        'check',
        '|',
        'quote',
        'line',
        'code',
        'inline-code',
        '|',
        'table',
        '|',
        'undo',
        'redo'
      ],
      value: desiredValue,
      input: (value) => {
        if (settingRealtimeVditorValue) return;
        syncRealtimeValueToRawContent(value);
      },
      after: () => {
        realtimeVditorReady.value = true;

        if (pendingRealtimeVditorValue.value != null && realtimeVditor) {
          const v = pendingRealtimeVditorValue.value;
          pendingRealtimeVditorValue.value = null;
          settingRealtimeVditorValue = true;
          try {
            realtimeVditor.setValue(v);
          } finally {
            settingRealtimeVditorValue = false;
          }
        }

        if (pendingRealtimeScrollRestore.value) {
          restoreScrollPosition();
        }
      }
    });

    return;
  }

  if (!realtimeVditorReady.value) {
    pendingRealtimeVditorValue.value = desiredValue;
    return;
  }

  settingRealtimeVditorValue = true;
  try {
    realtimeVditor.setValue(desiredValue);
  } finally {
    settingRealtimeVditorValue = false;
  }

  if (pendingRealtimeScrollRestore.value) {
    restoreScrollPosition();
  }
};

const destroyRealtimeVditor = () => {
  syncRealtimeValueToRawContent.cancel();
  if (!realtimeVditor) return;
  try {
    realtimeVditor.destroy();
  } finally {
    realtimeVditor = null;
    realtimeVditorReady.value = false;
    pendingRealtimeVditorValue.value = null;
  }
};

const handlePreviewModeSelect = async (key) => {
  if (previewMode.value === 'realtime') {
    captureScrollPosition();
    if (realtimeVditor) {
      const content = realtimeVditor.getValue();
      rawContent.value = content;
      if (editor) {
        updateEditorContent(content);
      }
    }
  }

  if (key === 'edit') {
    previewMode.value = 'toggle';
    await setMainMode('edit');
    return;
  }

  if (key === 'preview') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      return;
    }
    previewMode.value = 'toggle';
    await setMainMode('preview');
    return;
  }

  if (key === 'split') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      renderedHtml.value = '';
      return;
    }

    previewMode.value = 'split';
    await setMainMode('edit');
    await nextTick();
    refreshPreview();
    return;
  }

  if (key === 'realtime') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      renderedHtml.value = '';
      return;
    }

    captureScrollPosition();
    pendingRealtimeScrollRestore.value = true;
    previewMode.value = 'realtime';
    await setMainMode('edit');
  }
};

watch(previewMode, async (nextMode, prevMode) => {
  if (prevMode === 'realtime' && nextMode !== 'realtime') {
    destroyRealtimeVditor();
  }

  if (nextMode === 'toggle') return;

  if (!canPreview.value) {
    message.warning('仅 Markdown 文件支持预览');
    previewMode.value = 'toggle';
    await setMainMode('edit');
    renderedHtml.value = '';
    return;
  }

  if (nextMode === 'realtime') {
    pendingRealtimeScrollRestore.value = true;
    await initRealtimeVditor();
    return;
  }

  await nextTick();
  if (!editor) {
    initEditor();
  }

  refreshPreview();
});

watch([rawContent, shouldAutoRenderPreview], () => {
  if (shouldAutoRenderPreview.value) {
    renderDebounced(rawContent.value);
  }
});

// 监听内容变化
watch(renderedHtml, () => {
  nextTick(() => {
    if (previewContainer.value) {
      addCopyButtons(previewContainer.value);
      if (pendingPreviewScrollRestore.value) {
        const el = previewContainer.value;
        const height = el.scrollHeight;
        el.scrollTop = height * scrollRatio.value;
        pendingPreviewScrollRestore.value = false;
      }
      if (pendingPreviewScrollLine.value) {
        scrollPreviewToLine(pendingPreviewScrollLine.value);
        pendingPreviewScrollLine.value = null;
      }
    }
  });
});

// 添加Vim模式切换函数
const toggleVimMode = () => {
  if (mode.value === 'preview') {
    message.warning('阅读模式下不能启用Vim编辑模式');
    return;
  }

  vimMode.value = !vimMode.value;

  nextTick(() => {
    if (vimMode.value) {
      if (editor && vimStatusBar.value) {
        vimModeInstance = initVimMode(editor, vimStatusBar.value);
        editor.updateOptions({
          vim: {
            enabled: true,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
              '<C-s>': false,
              '<C-c>': false,
              '<C-v>': false
            }
          }
        });
      }
    } else {
      if (vimModeInstance) {
        vimModeInstance.dispose();
        vimModeInstance = null;
      }
      if (editor) {
        editor.updateOptions({
          vim: {
            enabled: false
          }
        });
      }
    }

    applyVimOverlayInsets();
  });
};

const buildPrintHtml = (bodyHtml, title) => {
  const now = new Date();
  const dateStr = now.toLocaleString();

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<title>${title}</title>
<style>
  @page {
    size: A4;
    margin: 16mm;
  }

  html, body {
    margin: 0;
    padding: 0;
    background: #ffffff;
  }

  body {
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
    color: #000000;
  }

  .export-root {
    box-sizing: border-box;
    padding: 12px 8px 24px 8px;
  }

  .export-header {
    margin-bottom: 24px;
    padding-bottom: 12px;
    border-bottom: 1px solid #e0e0e0;
  }

  .export-title {
    font-size: 22px;
    font-weight: 600;
    margin: 0 0 4px 0;
    word-break: break-all;
  }

  .export-meta {
    font-size: 12px;
    color: #666666;
  }

  .export-content {
    font-size: 16px;
    line-height: 1.6;
  }

  .export-content code,
  .export-content pre {
    font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  }

  .export-content pre {
    background: #f5f5f5;
    padding: 10px 12px;
    border-radius: 4px;
    overflow-x: auto;
  }

  .export-content table {
    border-collapse: collapse;
    width: 100%;
  }

  .export-content table th,
  .export-content table td {
    border: 1px solid #ddd;
    padding: 6px 8px;
  }

  .export-content img {
    max-width: 100%;
    height: auto;
  }

  @media print {
    body {
      -webkit-print-color-adjust: exact;
      print-color-adjust: exact;
    }
  }
</style>
</head>
<body>
  <div class="export-root">
    <div class="export-header">
      <div class="export-title">${title}</div>
      <div class="export-meta">导出时间：${dateStr}</div>
    </div>
    <div class="export-content">${bodyHtml}</div>
  </div>
</body>
</html>`;
};

const createPrintIframeAndPrint = (html) => {
  const iframe = document.createElement('iframe');
  iframe.style.position = 'fixed';
  iframe.style.right = '0';
  iframe.style.bottom = '0';
  iframe.style.width = '0';
  iframe.style.height = '0';
  iframe.style.border = '0';
  iframe.style.visibility = 'hidden';
  iframe.srcdoc = html;
  document.body.appendChild(iframe);

  const cleanup = () => {
    if (iframe.parentNode) {
      iframe.parentNode.removeChild(iframe);
    }
  };

  iframe.onload = () => {
    try {
      if (iframe.contentWindow) {
        iframe.contentWindow.focus();
        iframe.contentWindow.print();
      }
    } finally {
      setTimeout(cleanup, 1000);
    }
  };
};

const exportOptions = [
  { label: '导出为 PDF', key: 'pdf' },
  { label: '导出为 Word (.docx)', key: 'docx' },
  { label: '导出为 HTML (.html)', key: 'html' },
  { label: '导出为 LaTeX (.tex)', key: 'tex' },
  { label: '导出为 EPUB (.epub)', key: 'epub' },
  { label: '导出为 ODT (.odt)', key: 'odt' },
  { label: '导出为 纯文本 (.txt)', key: 'txt' }
];

const handleExportSelect = async (format) => {
  if (!selectedFile.value || !selectedFile.value.path) {
    message.warning('当前没有选中的 Markdown 文件');
    return;
  }

  const ext = selectedFile.value.path.split('.').pop()?.toLowerCase() || '';
  if (ext !== 'md') {
    message.warning('仅支持对 Markdown 文件进行导出');
    return;
  }

  if (mode.value === 'edit' && editor) {
    saveContent();
  }

  const path = selectedFile.value.path || '';

  const formatExtMap = {
    pdf: 'pdf',
    docx: 'docx',
    html: 'html',
    tex: 'tex',
    epub: 'epub',
    odt: 'odt',
    txt: 'txt'
  };

  const extName = formatExtMap[format] || format;

  let filterName = '';
  switch (format) {
    case 'pdf':
      filterName = 'PDF 文件';
      break;
    case 'docx':
      filterName = 'Word 文件';
      break;
    case 'html':
      filterName = 'HTML 文件';
      break;
    case 'tex':
      filterName = 'LaTeX 文件';
      break;
    case 'epub':
      filterName = 'EPUB 文件';
      break;
    case 'odt':
      filterName = 'ODT 文件';
      break;
    case 'txt':
      filterName = '文本文件';
      break;
    default:
      filterName = '导出文件';
  }

  try {
    let defaultPath = path;
    if (path.toLowerCase().endsWith('.md')) {
      defaultPath = path.slice(0, -3) + '.' + extName;
    } else {
      defaultPath = path + '.' + extName;
    }

    const outputPath = await save({
      defaultPath,
      filters: [
        {
          name: filterName,
          extensions: [extName]
        }
      ]
    });

    if (!outputPath) {
      message.info('已取消导出');
      return;
    }

    if (format === 'pdf') {
      await invoke('export_markdown_pdf_pandoc', {
        inputPath: path,
        outputPath
      });
    } else {
      await invoke('export_markdown_pandoc', {
        inputPath: path,
        outputPath,
        format
      });
    }

    message.success(`已导出为 ${filterName}`);
  } catch (error) {
    console.error('导出 Markdown 失败:', error);
    message.error('导出 Markdown 失败');
  }
};

const switchTab = (path, region) => {
  const targetRegion = region || 'left';

  console.debug('[Editor] switchTab', {
    from: targetRegion === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft,
    to: path,
    region: targetRegion,
    hasEditor: !!editor,
    hasSecondary: !!secondaryEditor,
  })

  const currentPath = targetRegion === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft;
  if (currentPath) {
    const currentEditor = targetRegion === 'right' ? secondaryEditor : editor;
    const currentContent =
      targetRegion === fileTreeStore.activeRegion && previewMode.value === 'realtime' && realtimeVditor
        ? realtimeVditor.getValue()
        : currentEditor
          ? currentEditor.getValue()
          : rawContent.value;
    fileTreeStore.updateTabContent(currentPath, currentContent);
  }

  fileTreeStore.setActiveTab(path, targetRegion);
  const tab = fileTreeStore.openTabs.find((t) => t.path === path);
  if (!tab) return;

  rawContent.value = tab.content;

  if (previewMode.value === 'realtime' && realtimeVditor) {
    initRealtimeVditor();
  }

  if (fileTreeStore.layoutMode !== 'single' && targetRegion === 'right') {
    ensureSecondaryEditor();
  }

  updateEditorContent(tab.content);
};

const closeTab = async (path, region) => {
  const targetRegion = region || 'left';
  const regionActivePath = targetRegion === 'right' ? fileTreeStore.activeTabRight : fileTreeStore.activeTabLeft;
  const regionEditor = targetRegion === 'right' ? secondaryEditor : editor;

  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.openTabs.length === 1) {
    if (regionEditor && regionActivePath === path) {
      const content = regionEditor.getValue();
      fileTreeStore.updateTabContent(path, content);
    }

    fileTreeStore.setLayoutMode('single');
    if (fileTreeStore.activeTab !== path) {
      fileTreeStore.setActiveTab(path, 'left');
    }
    return;
  }

  if (regionEditor && regionActivePath === path) {
    const content = regionEditor.getValue();
    fileTreeStore.updateTabContent(path, content);
  }

  const closingTab = fileTreeStore.openTabs.find((t) => t.path === path)
  if (closingTab?.dirty) {
    const ok = await confirmDialog(`文件「${closingTab.fileName || path}」尚未保存，确定要关闭并丢弃更改吗？`, {
      title: '未保存的更改',
      kind: 'warning'
    })
    if (!ok) return
  }

  fileTreeStore.closeTab(path, targetRegion);
  if (fileTreeStore.openTabs.length > 0) {
    const tab = fileTreeStore.openTabs.find(t => t.path === fileTreeStore.activeTab);
    if (tab) {
      updateEditorContent(tab.content);
    }
  } else {
    fileTreeStore.setActiveTab(null);
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue('');
    }
    if (secondaryEditor) {
      secondaryEditor.setValue('');
      secondaryEditor.getModel().setValue('');
    }
    renderedHtml.value = '';
  }
};

const applyHighlights = () => {
  if (!editor) return;

  if (editorStore.decorations?.length) {
    editor.deltaDecorations(editorStore.decorations, []);
    editorStore.decorations = [];
  }

  if (!editorStore.highlightInfo) return;

  const model = editor.getModel();
  if (!model) return;

  const { lineNumber, matches = [], text } = editorStore.highlightInfo;

  console.log("WWW>>> applyHighlights", { lineNumber, matches, text });

  let newDecorations = [];

  // 检查行号合法性
  const totalLines = model.getLineCount();
  const validLineNumber = typeof lineNumber === 'number' && lineNumber >= 1 && lineNumber <= totalLines;

  if (matches && matches.length > 0 && validLineNumber) {
    const lineContent = model.getLineContent(lineNumber);
    const lineLength = lineContent.length;

    newDecorations = matches
      .map(match => {
        let start = Math.max(0, Math.min(match.start, lineLength));
        let end = Math.max(start, Math.min(match.end, lineLength));
        if (start >= end) return null;
        return {
          range: new monaco.Range(
            lineNumber,
            start + 1,
            lineNumber,
            end + 1
          ),
          options: {
            inlineClassName: 'search-highlight',
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        };
      })
      .filter(Boolean);

    editorStore.decorations = editor.deltaDecorations([], newDecorations);

    if (matches.length > 0) {
      let first = matches[0];
      let start = Math.max(0, Math.min(first.start, lineLength));
      editor.revealPositionInCenter({
        lineNumber,
        column: start + 1
      });
    } else {
      editor.revealLineInCenter(lineNumber);
    }
  } else if (text) {
    const foundMatches = model.findMatches(text, false, false, true, null, true);

    newDecorations = foundMatches.map(match => ({
      range: match.range,
      options: {
        inlineClassName: 'search-highlight',
        stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
      }
    }));

    editorStore.decorations = editor.deltaDecorations([], newDecorations);

    if (foundMatches.length > 0) {
      editor.revealRangeInCenter(foundMatches[0].range);
    }
  }
};

// 监听编辑器store的变化
watch(() => editorStore.currentFile, (file) => {
  if (file && editor) {
    console.log("WWW>>> editorStore.currentFile", file)
    editor.setValue(file.content);
    nextTick(applyHighlights);
  }
});

watch(() => editorStore.highlightInfo, () => {
  if (editor) {
    console.log("WWW>>> editorStore.highlightInfo", editorStore.highlightInfo)
    applyHighlights();
  }
}, { deep: true });


const md = new MarkdownIt()
function generateOutline(content) {
  const file = activeTab.value;
  const ext = file?.path?.split('.').pop()?.toLowerCase();
  if (ext !== 'md' && ext !== 'markdown') {
    outlineStore.setOutline([]);
    return;
  }
  const tokens = md.parse(content, {})
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

function jumpToLine(line) {
  if (mode.value === 'preview') {
    scrollPreviewToLine(line)
    return
  }
  if (editor) {
    editor.revealLineInCenter(line)
    editor.setPosition({ lineNumber: line, column: 1 })
    editor.focus()
  }
}

// 监听 activeTab 和内容变化，生成大纲
watch([() => fileTreeStore.activeTab, activeTabContent], ([tabPath, content]) => {
  generateOutlineDebounced(content)
}, { immediate: true })


// 生命周期
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
  outlineStore.setJumpToLine(jumpToLine)
  initGitService()

  const intervalMs = Number(editorStore.autoSaveIntervalMs) || AUTO_SAVE_FALLBACK_INTERVAL_MS
  if (editorStore.autoSaveEnabled !== false && intervalMs > 0) {
    autoSaveTimerId = window.setInterval(autoSaveDirtyTabs, intervalMs)
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  destroyRealtimeVditor();

  if (autoSaveTimerId) {
    window.clearInterval(autoSaveTimerId)
    autoSaveTimerId = null
  }

  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }
  if (secondaryEditor) {
    secondaryEditor.dispose();
    secondaryEditor = null;
  }

  if (editorEffectsLeft) {
    editorEffectsLeft.destroy();
    editorEffectsLeft = null;
  }
  editor?.dispose();
  editor = null;
  editorStore.setEditorInstance(null); // 清除store中的引用

  if (gitDiffEditor) {
    gitDiffEditor.dispose()
    gitDiffEditor = null
  }
  if (gitDiffOriginalModel) {
    gitDiffOriginalModel.dispose()
    gitDiffOriginalModel = null
  }
  if (gitDiffModifiedModel) {
    gitDiffModifiedModel.dispose()
    gitDiffModifiedModel = null
  }
  if (gitDiffContainerEl) {
    gitDiffContainerEl.remove()
    gitDiffContainerEl = null
  }

  if (vimModeInstance) {
    vimModeInstance.dispose();
  }
});

</script>


<style scoped>
.editor-header {
  position: sticky;
  top: 0;
  z-index: 50;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 4px 8px;
  height: 30px;
  min-height: 30px;
  background-color: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  position: relative;
  width: 100%;
  box-sizing: border-box;
  min-width: 0;
  flex-wrap: nowrap;
  overflow: hidden;
}

.toolbar-editing-actions {
  display: flex;
  gap: 4px;
  align-items: center;
  flex: 1 1 auto;
  min-width: 0;
  flex-wrap: nowrap;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}

.toolbar-editing-actions::-webkit-scrollbar {
  height: 0;
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

.toolbar-editing-actions :deep(.n-button) {
  padding: 0 4px;
  height: 22px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.toolbar-editing-actions :deep(.n-button:hover) {
  background-color: var(--guaxi-panel-bg);
}

.toolbar-spacer {
  flex-grow: 1;
  min-width: 16px;
  /* 添加最小间距 */
  flex-shrink: 1;
  /* 关键修复4: 允许间距缩小 */
}

.toolbar-actions {
  display: flex;
  gap: 12px;
  /* 新增按钮间距 */
  align-items: center;
  min-width: 0;
  /* 关键修复5: 允许操作区域缩小 */
  flex-shrink: 0;
  /* 关键修复6: 防止按钮被压缩 */
}

.toolbar-actions :deep(.n-button) {
  flex-shrink: 0;
}

.mode-toggle {
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  transition: background-color 0.2s;
  position: relative;
  z-index: 1;
  white-space: nowrap;
  flex-shrink: 0;
  /* 防止按钮缩小 */
  box-sizing: border-box;
  /* 包含padding和border */
}

/* 响应式处理 */
@media (max-width: 600px) {
  .toolbar {
    padding: 4px;
    height: 30px;
    flex-wrap: nowrap;
    justify-content: space-between;
    overflow: hidden;
    contain: paint;
  }

  .toolbar-spacer {
    display: none;
  }

  .toolbar-editing-actions {
    flex: 1 1 auto;
    min-width: 0;
    flex-wrap: nowrap;
    overflow-x: auto;
    overflow-y: hidden;
    contain: paint;
    -webkit-mask-image: linear-gradient(to right, #000 0%, #000 85%, transparent 100%);
    mask-image: linear-gradient(to right, #000 0%, #000 85%, transparent 100%);
  }

  .toolbar-actions {
    flex: 0 0 auto;
    margin-left: 8px;
  }

  .mode-toggle {
    padding: 2px 4px;
  }
}

.mode-toggle:hover {
  background-color: var(--guaxi-panel-bg);
}

.editor-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.editor-empty {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, #8a8f98);
  font-size: 14px;
  user-select: none;
}

.editor-content.toggle {
  flex-direction: column;
}

.editor-content.split {
  flex-direction: row;
}

.editor-content.split .preview,
.editor-content.split .editor-wrapper {
  flex: 1 1 0;
  min-width: 0;
  width: 0;
}

.editor-content.split .editor-wrapper {
  border-left: 1px solid var(--divider-color, #e0e0e0);
}

.editor-content.realtime {
  flex-direction: column;
}

.realtime-vditor {
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  overflow: hidden;
}

.realtime-vditor :deep(.vditor) {
  height: 100%;
  border: none;
}

.realtime-vditor :deep(.vditor-content) {
  height: 100%;
}

.realtime-vditor :deep(.vditor-toolbar) {
  display: flex;
  flex-direction: row !important;
  flex-wrap: nowrap;
  overflow-x: auto;
  overflow-y: hidden;
  background-color: var(--guaxi-header-bg) !important;
  color: var(--guaxi-text-primary) !important;
  border-bottom: 1px solid var(--guaxi-border) !important;
}

.realtime-vditor :deep(.vditor-toolbar svg),
.realtime-vditor :deep(.vditor-toolbar svg *) {
  fill: currentColor !important;
  stroke: currentColor !important;
}

.realtime-vditor :deep(.vditor-reset) {
  background-color: var(--guaxi-content-bg) !important;
  color: var(--guaxi-text-primary) !important;
}

.realtime-vditor :deep(.vditor-toolbar > *) {
  flex: 0 0 auto;
}

.realtime-vditor :deep(.vditor-toolbar::-webkit-scrollbar) {
  height: 4px;
}

.realtime-vditor :deep(.vditor-toolbar::-webkit-scrollbar-thumb) {
  background-color: rgba(0, 0, 0, 0.25);
  border-radius: 999px;
}

.editor-wrapper {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-split-wrapper {
  flex: 1;
  display: flex;
  width: 100%;
  height: 100%;
}

.editor-split-wrapper.vim-enabled {
  box-sizing: border-box;
  padding-bottom: 0;
}

.editor-split-wrapper.single,
.editor-split-wrapper.vertical {
  flex-direction: row;
}

.editor-split-wrapper.horizontal {
  flex-direction: column;
}

.editor-container {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  width: 100%;
  box-sizing: border-box;
  position: relative;
  isolation: isolate;
}

:deep(.guaxi-editor-effects) {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 20;
  overflow: hidden;
}

:deep(.guaxi-editor-effects-canvas) {
  width: 100%;
  height: 100%;
  display: block;
}

:deep(.guaxi-editor-smooth-cursor) {
  position: absolute;
  top: 0;
  left: 0;
  width: 2px;
  border-radius: 2px;
  background: rgba(0, 173, 255, 0.85);
  box-shadow:
    0 0 10px rgba(0, 173, 255, 0.55),
    0 0 20px rgba(255, 105, 180, 0.25);
  opacity: 0;
  will-change: transform, height, opacity;
}

.vim-status-bar {
  position: absolute;
  bottom: 22px;
  left: 0;
  right: 0;
  height: 20px;
  background-color: var(--toolbar-bg, #f8f8f8);
  border-top: 1px solid var(--divider-color, #e0e0e0);
  padding: 2px 8px;
  font-size: 12px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  z-index: 1;
}

.editor-status-bar {
  height: 22px;
  background-color: var(--statusBar-background);
  border-top: 1px solid var(--statusBar-border);
  flex-shrink: 0;
  /* 防止状态栏被压缩 */
  z-index: 2;
  /* 确保在编辑器上方 */
}

.preview {
  flex: 1 1 auto;
  min-height: 0;
  padding: 12px;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  line-height: 1.5;
  overflow-y: auto;
  overflow-x: hidden;
  word-wrap: break-word;
  box-sizing: border-box;
}

.preview code {
  font-family: inherit !important;
}

.render-error {
  color: var(--error-color, #d32f2f);
  background-color: var(--error-bg, #ffebee);
  padding: 16px;
  border-radius: 4px;
  margin: 16px;
}

.render-error button {
  margin-top: 8px;
  padding: 4px 8px;
  background-color: var(--primary-color, #1976d2);
  color: white;
  border: none;
  border-radius: 2px;
  cursor: pointer;
}

/* 暗色模式适配 */
.dark .toolbar {
  background-color: var(--dark-toolbar-bg, #1e1e1e);
  border-bottom-color: var(--dark-divider-color, #333);
}

.dark .mode-toggle:hover {
  background-color: var(--dark-hover-color, #333);
}

.tab-bar {
  display: flex;
  height: 35px;
  min-height: 35px;
  flex: 0 0 35px;
  flex-shrink: 0;
  align-items: center;
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  overflow: hidden;
}

.tab-region {
  display: flex;
  flex: 1 1 0;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
}

.tab-region-right {
  border-left: 1px solid var(--guaxi-border);
}

.tab-region::-webkit-scrollbar {
  width: 4px;
  height: 4px;
}

.tab-region::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

.tab-region::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.4);
}

.tab-item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 10px;
  border-right: 1px solid #ddd;
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
  flex-shrink: 0;
  color: var(--guaxi-text-primary);
}

.tab-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  max-width: 240px;
}

.tab-affordance {
  position: relative;
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}

.tab-close {
  position: absolute;
  inset: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
}

.tab-dirty-dot {
  position: absolute;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: rgba(74, 163, 255, 0.7);
}

.tab-item:hover .tab-close,
.tab-item.active:not(.dirty) .tab-close {
  opacity: 1;
}

.tab-item.dirty:hover .tab-dirty-dot {
  opacity: 0;
}

.tab-item:hover .tab-affordance {
  background: rgba(74, 163, 255, 0.12);
}

.tab-item:hover .tab-close {
  color: rgba(74, 163, 255, 0.95);
}

.tab-item.active {
  background: var(--guaxi-panel-bg);
  border-bottom: 2px solid var(--primary-color, #18a058);
  color: var(--guaxi-text-primary);
}

.tab-item:hover {
  background: rgba(74, 163, 255, 0.08);
}

/* Git 行变更装饰器样式 */
:deep(.monaco-editor .margin-view-overlays .git-line-added),
:deep(.monaco-editor .margin-view-overlays .git-line-modified),
:deep(.monaco-editor .margin-view-overlays .git-line-deleted) {
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  opacity: 1 !important;
}

:deep(.monaco-editor .margin-view-overlays .git-line-added) {
  box-shadow: inset 4px 0 0 rgba(46, 160, 67, 0.95) !important;
}

:deep(.monaco-editor .margin-view-overlays .git-line-modified) {
  box-shadow: inset 4px 0 0 rgba(46, 160, 67, 0.75) !important;
}

:deep(.monaco-editor .margin-view-overlays .git-line-deleted) {
  box-shadow: inset 4px 0 0 rgba(46, 160, 67, 0.55) !important;
}

/* 确保装饰器区域可见 */
:deep(.monaco-editor .glyph-margin) {
  background-color: var(--guaxi-header-bg) !important;
}
</style>

<style>
/* 数学公式样式 */
.math-inline {
  display: inline;
  vertical-align: middle;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: var(--guaxi-panel-bg, #ffffff);
  border: 1px solid var(--guaxi-border, #e1e4e8);
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  overflow: auto;
  color: var(--guaxi-text-primary, #1f2328);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.dark .mermaid {
  background-color: #1f242d;
  border-color: #30363d;
  color: #c9d1d9;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.dark .mermaid svg text,
.dark .mermaid svg .label text {
  fill: #c9d1d9 !important;
}

.dark .mermaid svg .edgePath path {
  stroke: #8b949e !important;
}

.dark .mermaid svg .arrowheadPath {
  fill: #8b949e !important;
  stroke: #8b949e !important;
}

.dark .mermaid svg .node rect,
.dark .mermaid svg .node polygon,
.dark .mermaid svg .node circle,
.dark .mermaid svg .cluster rect {
  fill: #2c313a !important;
  stroke: #6e7681 !important;
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #ffffff !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
  color: #24292f;
}

.hljs code {
  font-family: inherit !important;
  background: transparent !important;
  border: 0 !important;
  padding: 0 !important;
}

.preview pre:not(.hljs) > code {
  display: block;
  border-radius: 6px;
  border: 1px solid #e1e4e8;
  background: #ffffff;
  color: #24292f;
  padding: 1.5em 1em;
  overflow-x: auto;
}

.dark-mode .preview pre:not(.hljs) > code {
  border-color: #30363d;
  background: #1f242d;
  color: #c9d1d9;
}

.preview :not(pre) > code:not(.hljs) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
  font-size: 0.9em;
  padding: 0.1em 0.35em;
  border-radius: 4px;
  background: rgba(175, 184, 193, 0.2);
}

/* 暗色模式代码块 */
.dark-mode .hljs {
  background: #1f242d !important;
  border-color: #30363d;
  color: #c9d1d9;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark-mode .hljs code {
  font-family: inherit !important;
}

.dark-mode .preview :not(pre) > code:not(.hljs) {
  background: rgba(110, 118, 129, 0.25);
  color: #c9d1d9;
}

.dark-mode .hljs-comment,
.dark-mode .hljs-quote {
  color: #8b949e;
}

.dark-mode .hljs-keyword,
.dark-mode .hljs-selector-tag,
.dark-mode .hljs-literal,
.dark-mode .hljs-built_in,
.dark-mode .hljs-type {
  color: #ff7b72;
}

.dark-mode .hljs-string,
.dark-mode .hljs-doctag,
.dark-mode .hljs-regexp {
  color: #a5d6ff;
}

.dark-mode .hljs-number,
.dark-mode .hljs-symbol,
.dark-mode .hljs-bullet {
  color: #79c0ff;
}

.dark-mode .hljs-title,
.dark-mode .hljs-section,
.dark-mode .hljs-function .hljs-title {
  color: #d2a8ff;
}

.dark-mode .hljs-attr,
.dark-mode .hljs-attribute,
.dark-mode .hljs-variable,
.dark-mode .hljs-template-variable,
.dark-mode .hljs-property,
.dark-mode .hljs-params {
  color: #ffa657 !important;
}

.dark-mode .hljs-subst,
.dark-mode .hljs-operator,
.dark-mode .hljs-punctuation {
  color: #c9d1d9 !important;
}

.dark-mode .hljs-meta,
.dark-mode .hljs-tag,
.dark-mode .hljs-name {
  color: #7ee787;
}

/* 修改为以下样式： */
.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}

table {
  width: 100%;
  border-collapse: collapse;
  border: 1px solid #ddd;
}

th,
td {
  border: 1px solid #ddd;
  padding: 8px;
  text-align: center;
}

th {
  background-color: #f4f4f4;
  font-weight: bold;
}

tr:nth-child(even) {
  background-color: #f9f9f9;
}

img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 0 auto;
}

.search-highlight {
  background-color: rgb(241, 168, 10) !important;
  color: #000 !important;
  border-radius: 2px;
  padding: 0 2px;
}

.dark .search-highlight {
  background-color: rgb(255, 208, 0) !important;
  color: #000 !important;
}
</style>