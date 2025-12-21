<template>
  <div class="editor-container guaxi-editor">
    <div class="editor-header" ref="editorHeaderRef">
      <div class="tab-bar" ref="tabBarRef" @wheel="handleWheelScroll">
        <div class="tab-region tab-region-left">
          <div v-for="tab in leftTabs" :key="tab.path"
            :class="['tab-item', { 'active': fileTreeStore.activeTab === tab.path, 'dirty': !!tab.dirty }]" @click="switchTab(tab.path)"
            @mousedown.middle="closeTab(tab.path, 'left')">
            <span class="tab-title">{{ tab.fileName }}</span>
            <span class="tab-affordance">
              <span class="tab-dirty-dot" v-show="tab.dirty" />
              <n-icon class="tab-close" size="14" @click.stop="closeTab(tab.path, 'left')">
                <CloseOutline />
              </n-icon>
            </span>
          </div>
        </div>
        <div v-if="layoutMode !== 'single'" class="tab-region tab-region-right">
          <div v-for="tab in rightTabs" :key="tab.path"
            :class="['tab-item', { 'active': fileTreeStore.activeTab === tab.path, 'dirty': !!tab.dirty }]" @click="switchTab(tab.path)"
            @mousedown.middle="closeTab(tab.path, 'right')">
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
                <Icon icon="lucide:heading" width="16" height="16" />
              </n-button>
            </template>
            标题
          </n-tooltip>
        </n-dropdown>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleBold">
              <Icon icon="lucide:bold" width="16" height="16" />
            </n-button>
          </template>
          加粗 (Ctrl+B)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleItalic">
              <Icon icon="lucide:italic" width="16" height="16" />
            </n-button>
          </template>
          斜体 (Ctrl+I)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleStrikethrough">
              <Icon icon="lucide:strikethrough" width="16" height="16" />
            </n-button>
          </template>
          删除线 (Ctrl+Shift+X)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertLink">
              <Icon icon="lucide:link" width="16" height="16" />
            </n-button>
          </template>
          链接 (Ctrl+K)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleBlockquote">
              <Icon icon="lucide:quote" width="16" height="16" />
            </n-button>
          </template>
          引用
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleInlineCode">
              <Icon icon="lucide:code" width="16" height="16" />
            </n-button>
          </template>
          行内代码
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertCodeBlockFromToolbar">
              <Icon icon="lucide:square-terminal" width="16" height="16" />
            </n-button>
          </template>
          代码块
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleUnorderedList">
              <Icon icon="lucide:list" width="16" height="16" />
            </n-button>
          </template>
          无序列表
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleOrderedList">
              <Icon icon="lucide:list-ordered" width="16" height="16" />
            </n-button>
          </template>
          有序列表
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="toggleTaskList">
              <Icon icon="lucide:list-checks" width="16" height="16" />
            </n-button>
          </template>
          任务列表
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertTable">
              <Icon icon="lucide:table" width="16" height="16" />
            </n-button>
          </template>
          表格 (Ctrl+Alt+T)
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertImage">
              <Icon icon="lucide:image" width="16" height="16" />
            </n-button>
          </template>
          图片
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="insertHorizontalRule">
              <Icon icon="lucide:minus" width="16" height="16" />
            </n-button>
          </template>
          分隔线
        </n-tooltip>

        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="undo">
              <Icon icon="lucide:undo" width="16" height="16" />
            </n-button>
          </template>
          撤销
        </n-tooltip>
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text size="tiny" @click="redo">
              <Icon icon="lucide:redo" width="16" height="16" />
            </n-button>
          </template>
          重做
        </n-tooltip>
      </div>
      <div class="toolbar-spacer"></div>
      <div class="toolbar-actions">
        <!-- 添加 Git 操作按钮 -->
        <n-tooltip placement="top" trigger="hover">
          <template #trigger>
            <n-button text @click="toggleGitPanel">
              <n-icon width="16" height="16">
                <GitBranchOutline />
              </n-icon>
            </n-button>
          </template>
          Git 操作
        </n-tooltip>

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

    <div v-if="selectedFile" class="editor-content" :class="previewMode">
      <div v-show="previewMode === 'realtime' && canPreview" id="realtime-vditor" class="realtime-vditor"></div>
      <div v-show="previewMode !== 'realtime' && showPreview" v-html="renderedHtml" class="preview"
        ref="previewContainer" @click="handlePreviewClick"></div>
      <div v-show="previewMode !== 'realtime' && showEditor" class="editor-wrapper" ref="editorWrapperRef">
        <div v-show="tableToolbarVisible" class="floating-table-toolbar" :style="tableToolbarStyle" @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="addTableRowBelow">
                <Icon icon="mdi:table-row-plus-after" width="16" height="16" />
              </n-button>
            </template>
            新增行 (Ctrl+Alt+↓)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="deleteTableRow">
                <Icon icon="mdi:table-row-remove" width="16" height="16" />
              </n-button>
            </template>
            删除行 (Ctrl+Alt+Shift+↓)
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="addTableColumnRight">
                <Icon icon="mdi:table-column-plus-after" width="16" height="16" />
              </n-button>
            </template>
            新增列 (Ctrl+Alt+→)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="deleteTableColumn">
                <Icon icon="mdi:table-column-remove" width="16" height="16" />
              </n-button>
            </template>
            删除列 (Ctrl+Alt+Shift+→)
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="setTableColumnAlign('left')">
                <Icon icon="lucide:align-left" width="16" height="16" />
              </n-button>
            </template>
            左对齐
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="setTableColumnAlign('center')">
                <Icon icon="lucide:align-center" width="16" height="16" />
              </n-button>
            </template>
            居中
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="setTableColumnAlign('right')">
                <Icon icon="lucide:align-right" width="16" height="16" />
              </n-button>
            </template>
            右对齐
          </n-tooltip>
          <div class="floating-table-toolbar-divider"></div>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="formatTable">
                <Icon icon="lucide:sparkles" width="16" height="16" />
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
                <Icon icon="lucide:bold" width="16" height="16" />
              </n-button>
            </template>
            加粗 (Ctrl+B)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleItalic">
                <Icon icon="lucide:italic" width="16" height="16" />
              </n-button>
            </template>
            斜体 (Ctrl+I)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleStrikethrough">
                <Icon icon="lucide:strikethrough" width="16" height="16" />
              </n-button>
            </template>
            删除线 (Ctrl+Shift+X)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="insertLink">
                <Icon icon="lucide:link" width="16" height="16" />
              </n-button>
            </template>
            链接 (Ctrl+K)
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleInlineCode">
                <Icon icon="lucide:code" width="16" height="16" />
              </n-button>
            </template>
            行内代码
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="toggleBlockquote">
                <Icon icon="lucide:quote" width="16" height="16" />
              </n-button>
            </template>
            引用
          </n-tooltip>
        </div>
        <div class="editor-split-wrapper" :class="layoutMode">
          <div ref="editorContainer" class="editor-container">
            <!-- 编辑器内容将在这里渲染 -->
          </div>
          <div v-if="layoutMode !== 'single'" ref="secondaryEditorContainer" class="editor-container">
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
import GuaxiEditorStatusBar from './GuaxiEditorStatusBar.vue'
import { NTooltip, NIcon, useMessage, NDropdown } from 'naive-ui'
import { Icon } from '@iconify/vue'
import { EyeOutline, LogoVimeo, CloseOutline, DownloadOutline, SaveOutline } from '@vicons/ionicons5'
import { DocumentEdit16Regular, Edit16Filled } from '@vicons/fluent'
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
import { save } from '@tauri-apps/plugin-dialog'

const outlineStore = useOutlineStore()
// 添加 GitPanel 组件引用
const gitPanel = ref(null)


// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const mode = ref('edit');
const previewMode = ref('toggle');
const layoutMode = computed(() => fileTreeStore.layoutMode || 'single');
const renderedHtml = ref('');
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

onMounted(async () => {
  try {
    unlistenTemplateInsert = await listen('template_insert_event', async (event) => {
      const content = typeof event?.payload?.content === 'string' ? event.payload.content : ''
      if (!content) return

      const text = content.endsWith('\n') ? content : `${content}\n`
      const inserted = await editorStore.insertContent(text)
      if (!inserted) {
        message.warning('插入模板失败：请先打开一个文件')
      }
    })
  } catch (e) {
    console.error('注册模板插入事件失败:', e)
  }
})

onUnmounted(() => {
  if (typeof unlistenTemplateInsert === 'function') {
    unlistenTemplateInsert()
    unlistenTemplateInsert = null
  }
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
    createSnippet('hr', '\n---\n', '插入分隔线')
  ];

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
};

const tabBarRef = ref(null)
// 初始化编辑器
const initEditor = () => {
  console.debug('[Editor] initEditor called', {
    hasContainer: !!editorContainer.value,
    selectedPath: selectedFile.value?.path,
    activeTab: fileTreeStore.activeTab
  })

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
    console.error('[Editor] 编辑器容器未找到');
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

  monaco.editor.defineTheme('one-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '9e9e9e' },
      { token: 'keyword', foreground: '9c27b0' },
      { token: 'string', foreground: '2e7d32' },
      { token: 'number', foreground: 'ef6c00' },
      { token: 'regexp', foreground: '00897b' },
      { token: 'type', foreground: '6a1b9a' },
      { token: 'delimiter', foreground: '2f3337' },
      { token: 'variable', foreground: 'e53935' },
      { token: 'parameter', foreground: '3949ab' },
      { token: 'function', foreground: '1565c0' },
      { token: 'constant', foreground: '00838f' },
      { token: 'class', foreground: '283593' },
      { token: 'identifier', foreground: '2f3337' },
      { token: 'tag', foreground: 'd32f2f' },
      { token: 'attribute.name', foreground: '6d4c41' },
      { token: 'meta.embedded', foreground: '455a64' }
    ],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#2f3337',
      'editorLineNumber.foreground': '#b0b3ba',
      'editorLineNumber.activeForeground': '#2f3337',
      'editorCursor.foreground': '#1565c0',
      'editor.selectionBackground': '#cfe8ff',
      'editor.inactiveSelectionBackground': '#e3f2fd',
      'editor.lineHighlightBackground': '#f2f5fb',
      'editorBracketMatch.background': '#e1f5fe',
      'editorBracketMatch.border': '#90caf9',
      'editor.selectionHighlightBackground': '#ffe0b2',
      'editor.foldBackground': '#f7f9fc',
      'editorWhitespace.foreground': '#d0d0d5',
      'editorIndentGuide.background': '#e0e0e0',
      'editorIndentGuide.activeBackground': '#b0bec5',
      'editorHoverWidget.background': '#ffffff',
      'editorHoverWidget.border': '#e0e0e0',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#e0e0e0',
      'editorSuggestWidget.foreground': '#2f3337',
      'editorSuggestWidget.selectedBackground': '#e3f2fd',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#e0e0e0',
      'scrollbar.shadow': '#00000033'
    }
  });

  editor = monaco.editor.create(editorContainer.value, {
    value: rawContent.value,
    language: language,
    theme: 'one-light',
    readOnly: !!activeTab.value?.unsupported,
    automaticLayout: true,  // 自动布局
    scrollBeyondLastColumn: 40, // 允许水平滚动超出最后一列
    fixedOverflowWidgets: true, // 确保浮动部件可以超出编辑器边界
    wordWrap: 'off',
    renderValidationDecorations: 'on',
    minimap: {
      enabled: false,  // 启用迷你地图
      showSlider: 'always',
      size: 'fit'
    },
    glyphMargin: true, // 启用行号左侧边栏
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
  if (!secondaryEditorContainer.value || !editor || secondaryEditor) return;
  const model = editor.getModel();
  if (!model) return;
  const baseOptions = editor.getRawOptions ? editor.getRawOptions() : {};
  secondaryEditor = monaco.editor.create(secondaryEditorContainer.value, {
    ...baseOptions,
    model
  });
};

const disposeSecondaryEditor = () => {
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

// 初始化 Git 服务
const initGitService = async () => {
  if (selectedFile.value?.path) {
    const dir = await dirname(selectedFile.value.path)
    gitService.value = new GitService(dir)
    await updateGitStatus()
  }
}

// 更新 Git 状态
const updateGitStatus = async () => {
  if (!gitService.value || !selectedFile.value?.path) return

  try {
    const branch = await gitService.value.getCurrentBranch()
    editorStore.setGitBranch(branch)

    const status = await gitService.value.getFileStatus(selectedFile.value.path)
    editorStore.setGitChanges({
      added: status.filter(s => s.status === 'A').length,
      modified: status.filter(s => s.status === 'M').length,
      deleted: status.filter(s => s.status === 'D').length
    })

    await updateGitDecorations()
  } catch (error) {
    console.error('更新 Git 状态失败:', error)
  }
}

// 更新 Git 装饰器
const updateGitDecorations = async () => {
  if (!editor || !selectedFile.value?.path || !gitService.value) return

  try {
    const changes = await gitService.value.getLineChanges(selectedFile.value.path)
    editorStore.updateGitLineDecorations({
      added: changes.added,
      modified: changes.modified,
      deleted: changes.deleted
    })
  } catch (error) {
    console.error('更新 Git 装饰器失败:', error)
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
    })
  } catch (error) {
    console.error('创建窗口失败:', error)
    alert(`无法打开文件: ${error.message || '未知错误'}`)
  }
}

const handleWheelScroll = (e) => {
  if (!tabBarRef.value) return

  // 阻止默认的垂直滚动行为
  e.preventDefault()

  // 水平滚动（乘以一个系数可以调整滚动速度）
  tabBarRef.value.scrollLeft += e.deltaY * 0.5
}

const setupEditorListeners = () => {
  if (!editor) return;

  editor.onDidChangeCursorPosition((e) => {
    editorStore.setCursorPosition(e.position);
    updateTableToolbar();
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

    const pos = e.selection.getStartPosition();
    const visiblePos = editor.getScrolledVisiblePosition(pos);
    if (!visiblePos) {
      floatingToolbarVisible.value = false;
      return;
    }

    const wrapperRect = wrapperEl.getBoundingClientRect();
    const editorRect = editorEl.getBoundingClientRect();

    const left = editorRect.left - wrapperRect.left + visiblePos.left;
    const top = editorRect.top - wrapperRect.top + visiblePos.top - 36;

    const headerEl = editorHeaderRef.value;
    const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0;
    const minTop = headerHeight + 8;

    floatingToolbarStyle.value = {
      left: `${Math.max(8, left)}px`,
      top: `${Math.max(minTop, top)}px`
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

const normalizeSeparatorCell = (cell, align) => {
  const base = (cell || '').trim();
  const hasMin = base.replace(/:/g, '').replace(/-/g, '').length === 0;
  if (!hasMin) return '---';

  if (align === 'left') return ':---';
  if (align === 'center') return ':---:';
  if (align === 'right') return '---:';
  return '---';
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

const applyTableEdits = (nextRows, nextActiveRowIndex, nextActiveColIndex) => {
  if (!editor) return;
  const model = editor.getModel();
  const ctx = activeTableContext.value;
  if (!model || !ctx) return;

  const textLines = nextRows.map((cells, idx) => {
    if (idx === 1) {
      const normalized = cells.map((c) => normalizeSeparatorCell(c, null));
      return buildTableRow(normalized);
    }
    return buildTableRow(cells);
  });

  const range = new monaco.Range(ctx.startLine, 1, ctx.endLine, model.getLineMaxColumn(ctx.endLine));
  editor.executeEdits('md-table-toolbar', [
    {
      range,
      text: textLines.join('\n'),
      forceMoveMarkers: true
    }
  ]);

  const row = Math.max(2, Math.min(textLines.length - 1, nextActiveRowIndex));
  const col = Math.max(0, Math.min((nextRows[0]?.length || 1) - 1, nextActiveColIndex));
  const targetLine = ctx.startLine + row;
  editor.setPosition(new monaco.Position(targetLine, 3));
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

// 更新编辑器内容
const updateEditorContent = (content) => {
  const nextContent = content ?? '';
  rawContent.value = nextContent;

  if (!editor) {
    initEditor();
  }

  if (!editor) return;

  // 当内容为空时，确保完全重置编辑器
  if (!nextContent) {
    editor.setValue('');
    const model = editor.getModel();
    if (model) {
      model.setValue('');
    }
  } else {
    editor.setValue(nextContent);
  }

  editor.updateOptions({
    readOnly: !!activeTab.value?.unsupported,
  });
};

// 保存内容
const saveContent = async () => {
  if (!selectedFile.value) return;

  if (activeTab.value?.unsupported) {
    message.warning('不支持编辑/保存该文件类型');
    return;
  }

  const content =
    previewMode.value === 'realtime' && realtimeVditor
      ? realtimeVditor.getValue()
      : editor
        ? editor.getValue()
        : rawContent.value;

  await fileTreeStore.updateFileContent(selectedFile.value.path, content);
  message.info('修改已保存');
  if (vimMode.value) {
    message.info('修改已保存');
  }

  rawContent.value = content;

  if (previewMode.value !== 'realtime' && editor) {
    updateEditorContent(content);
  }

  if (previewMode.value === 'split' && canPreview.value) {
    renderDebounced(content);
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
    return;
  }

  try {
    const basePath = selectedFile.value.path ? await dirname(selectedFile.value.path) : '';
    renderedHtml.value = await renderMarkdown(content, {
      basePath: basePath,
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: 'default'
    });

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

// 监听文件选择
watch(selectedFile, async (newFile) => {
  if (newFile) {
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

    if (editor) {
      const model = editor.getModel();
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

    await updateGitDecorations()
  } else {
    if (editor) {
      editor.setValue('');
      editor.getModel().setValue('');
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
    return mode.value === 'edit' ? 'lucide:file-pen-line' : 'lucide:eye';
  }
  return previewMode.value === 'split' ? 'lucide:columns-2' : 'lucide:sparkles';
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

const switchTab = (path) => {
  console.debug('[Editor] switchTab', {
    from: fileTreeStore.activeTab,
    to: path,
    hasEditor: !!editor
  })

  const currentPath = fileTreeStore.activeTab;
  if (currentPath) {
    const currentContent =
      previewMode.value === 'realtime' && realtimeVditor
        ? realtimeVditor.getValue()
        : editor
          ? editor.getValue()
          : rawContent.value;
    fileTreeStore.updateTabContent(currentPath, currentContent);
  }

  fileTreeStore.setActiveTab(path);
  const tab = fileTreeStore.openTabs.find((t) => t.path === path);
  if (tab) {
    rawContent.value = tab.content;
    if (previewMode.value === 'realtime' && realtimeVditor) {
      initRealtimeVditor();
    }
    if (editor) {
      updateEditorContent(tab.content);
    }
  }
};

const closeTab = (path, region) => {
  if (fileTreeStore.layoutMode !== 'single' && fileTreeStore.openTabs.length === 1) {
    if (editor && fileTreeStore.activeTab === path) {
      const content = editor.getValue();
      fileTreeStore.updateTabContent(path, content);
    }

    fileTreeStore.setLayoutMode('single');
    if (fileTreeStore.activeTab !== path) {
      fileTreeStore.setActiveTab(path);
    }
    return;
  }

  if (editor && fileTreeStore.activeTab === path) {
    const content = editor.getValue();
    fileTreeStore.updateTabContent(path, content);
  }

  fileTreeStore.closeTab(path);
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
    renderedHtml.value = '';
  }
};

const applyHighlights = () => {
  if (!editor || !editorStore.highlightInfo) return;

  const model = editor.getModel(); // 只声明一次
  if (!model) return;

  const { lineNumber, matches = [], text } = editorStore.highlightInfo;

  console.log("WWW>>> applyHighlights", { lineNumber, matches, text });
  // 清除旧的高亮
  if (editorStore.decorations) {
    editor.deltaDecorations(editorStore.decorations, []);
    editorStore.decorations = [];
  }

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
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  destroyRealtimeVditor();
  if (secondaryEditor) {
    secondaryEditor.dispose();
    secondaryEditor = null;
  }
  editor?.dispose();
  editor = null;
  editorStore.setEditorInstance(null); // 清除store中的引用
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
  background-color: var(--toolbar-bg, #f8f8f8);
  border-bottom: 1px solid var(--divider-color, #e0e0e0);
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
  background-color: var(--toolbar-bg, #f8f8f8);
  border: 1px solid var(--divider-color, #e0e0e0);
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
  background-color: var(--toolbar-bg, #f8f8f8);
  border: 1px solid var(--divider-color, #e0e0e0);
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
  background-color: var(--divider-color, #e0e0e0);
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
  background-color: rgba(0, 0, 0, 0.04);
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
  background-color: var(--hover-color, #f0f0f0);
}

.editor-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  overflow: hidden;
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
  height: 30px;
  min-height: 30px;
  flex: 0 0 30px;
  flex-shrink: 0;
  align-items: center;
  background: #f5f5f5;
  border-bottom: 1px solid #ddd;
  overflow-x: auto;
  overflow-y: hidden;
}

.tab-region {
  display: flex;
  flex: 1;
}

.tab-region-right {
  border-left: 1px solid #ddd;
}

.tab-bar::-webkit-scrollbar {
  width: 3px;
  height: 4px;
}

.tab-bar::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

.tab-bar::-webkit-scrollbar-thumb:hover {
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
  color: rgba(0, 0, 0, 0.85);
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
  background: #fff;
  border-bottom: 2px solid var(--primary-color, #18a058);
  color: rgba(0, 0, 0, 0.95);
}

.tab-item:hover {
  background: rgba(74, 163, 255, 0.08);
}

/* Git 行变更装饰器样式 */
.git-line-added {
  background: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 6"><circle cx="3" cy="3" r="3" fill="%2361af61"/></svg>') no-repeat;
  background-position: 0 center;
  margin-left: 3px;
}

.git-line-modified {
  background: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 6"><circle cx="3" cy="3" r="3" fill="%23e5c07b"/></svg>') no-repeat;
  background-position: 0 center;
  margin-left: 3px;
}

.git-line-deleted {
  background: url('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 6"><circle cx="3" cy="3" r="3" fill="%23e06c75"/></svg>') no-repeat;
  background-position: 0 center;
  margin-left: 3px;
}

/* 确保装饰器区域可见 */
.monaco-editor .glyph-margin {
  background-color: var(--vscode-editorGutter-background);
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
  background-color: white;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

.dark .mermaid {
  background-color: #2d2d2d;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
}

/* 代码块样式 */
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

/* 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
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