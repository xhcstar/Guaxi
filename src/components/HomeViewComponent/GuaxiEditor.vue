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
          <n-dropdown trigger="click" :options="codeBlockOptions" @select="handleCodeBlockSelect">
            <n-tooltip placement="top" trigger="hover">
              <template #trigger>
                <n-button text size="tiny">
                  <Icon :icon="squareTerminalIcon" width="16" height="16" />
                </n-button>
              </template>
              代码块
            </n-tooltip>
          </n-dropdown>

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

          <n-popover trigger="click" placement="bottom-start" :show="tableInsertPopoverVisible"
            @update:show="handleTableInsertPopoverUpdateShow">
            <template #trigger>
              <n-tooltip placement="top" trigger="hover">
                <template #trigger>
                  <n-button text size="tiny">
                    <Icon :icon="tableIcon" width="16" height="16" />
                  </n-button>
                </template>
                表格 (Ctrl+Alt+T)
              </n-tooltip>
            </template>

            <div class="table-insert-popover" @mousedown.stop>
              <div class="table-insert-hint">{{ tableInsertHint }}</div>
              <div class="table-insert-grid" @mouseup="handleTableGridMouseUp" @mouseleave="handleTableGridMouseLeave">
                <template v-for="r in tableGridMaxRows" :key="`r-${r}`">
                  <template v-for="c in tableGridMaxCols" :key="`c-${r}-${c}`">
                    <div class="table-insert-cell" :class="{ active: r <= tableGridRows && c <= tableGridCols }"
                      @mousedown.prevent="handleTableGridMouseDown(r, c)" @mouseover="handleTableGridMouseOver(r, c)" />
                  </template>
                </template>
              </div>

              <div class="table-insert-controls">
                <div class="table-insert-inputs">
                  <n-input-number v-model:value="tableInputRows" size="tiny" :min="2" :max="tableInsertMaxRows"
                    :show-button="false" placeholder="行" />
                  <span class="table-insert-mul">×</span>
                  <n-input-number v-model:value="tableInputCols" size="tiny" :min="1" :max="tableInsertMaxCols"
                    :show-button="false" placeholder="列" />
                  <n-button size="tiny" type="primary" @click="insertTableFromInputs">插入</n-button>
                </div>
              </div>
            </div>
          </n-popover>
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
                <n-icon :component="SaveOutline" size="18" />
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
                  <n-icon :component="ExitOutline" size="18" />
                </n-button>
              </template>
              导出
            </n-tooltip>
          </n-dropdown>

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text @click="openMarkdownManual">
                <Icon :icon="bookOpenIcon" width="16" height="16" />
              </n-button>
            </template>
            Markdown语法查询手册
          </n-tooltip>
        </div>
      </div>
    </div>

    <div class="editor-content" :class="previewMode">
      <div v-if="!selectedFile" class="editor-empty">请选择一个文件以开始编辑</div>
      <div v-show="selectedFile && previewMode === 'realtime' && canPreview" id="realtime-vditor"
        class="realtime-vditor">
      </div>
      <div v-show="previewMode !== 'realtime' && showPreview" class="preview-shell" ref="previewShellRef">
        <div v-html="renderedHtml" class="preview" ref="previewContainer" @click="handlePreviewClick"
          @mouseup="handlePreviewSelectionMouseUp"></div>
        <div v-show="previewFloatingToolbarVisible" class="floating-md-toolbar" :style="previewFloatingToolbarStyle"
          @mousedown.prevent>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" :disabled="selectionTranslateBusy" @click="openSelectionTranslatePanel">
                <Icon :icon="languagesIcon" width="16" height="16" />
              </n-button>
            </template>
            翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="askSelectionWithAssistant">
                <Icon :icon="messageCircleIcon" width="16" height="16" />
              </n-button>
            </template>
            问AI
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="openAssistantTranslateDialog">
                <Icon :icon="botIcon" width="16" height="16" />
              </n-button>
            </template>
            用AI翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" :disabled="selectionReadAloudBusy" @click="readSelectionAloud">
                <Icon :icon="volume2Icon" width="16" height="16" />
              </n-button>
            </template>
            朗读
          </n-tooltip>
        </div>
      </div>
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

          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" :disabled="selectionTranslateBusy" @click="openSelectionTranslatePanel">
                <Icon :icon="languagesIcon" width="16" height="16" />
              </n-button>
            </template>
            翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="askSelectionWithAssistant">
                <Icon :icon="messageCircleIcon" width="16" height="16" />
              </n-button>
            </template>
            问AI
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" @click="openAssistantTranslateDialog">
                <Icon :icon="botIcon" width="16" height="16" />
              </n-button>
            </template>
            用AI翻译
          </n-tooltip>
          <n-tooltip placement="top" trigger="hover">
            <template #trigger>
              <n-button text size="tiny" :disabled="selectionReadAloudBusy" @click="readSelectionAloud">
                <Icon :icon="volume2Icon" width="16" height="16" />
              </n-button>
            </template>
            朗读
          </n-tooltip>
        </div>

        <div class="breadcrumb-bar">
          <div ref="breadcrumbScrollRef" class="breadcrumb-scroll" @wheel.prevent="handleBreadcrumbWheel">
            <n-breadcrumb separator="›">
              <n-breadcrumb-item v-for="it in breadcrumbItems" :key="it.key">
                <span class="breadcrumb-item" :class="{ disabled: !!it.disabled }" :title="it.title"
                  @click.stop="openBreadcrumbPicker(it, $event)">
                  <Icon v-if="it.icon" class="breadcrumb-icon" :icon="it.icon" width="14" height="14" />
                  <span class="breadcrumb-label">{{ it.label }}</span>
                </span>
              </n-breadcrumb-item>
            </n-breadcrumb>
          </div>

          <n-dropdown v-model:show="breadcrumbPickerVisible" trigger="manual" placement="bottom-start"
            :x="breadcrumbPickerPosition.x" :y="breadcrumbPickerPosition.y" :options="breadcrumbPickerOptions"
            @select="handleBreadcrumbPickerSelect" @clickoutside="closeBreadcrumbPicker" />
        </div>

        <n-modal v-model:show="assistantTranslateDialogVisible" preset="card" title="用AI翻译" style="width: 420px">
          <div class="assistant-translate-dialog-row">
            <Icon :icon="languagesIcon" width="16" height="16" />
            <span>选择目标语言</span>
          </div>
          <n-select
            v-model:value="assistantTranslateTargetLang"
            size="small"
            :options="assistantTranslateLangOptions"
          />
          <div class="assistant-translate-dialog-actions">
            <n-button size="small" @click="assistantTranslateDialogVisible = false">取消</n-button>
            <n-button size="small" type="primary" @click="sendSelectionAsAiTranslate">发送并翻译</n-button>
          </div>
        </n-modal>

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

      <GuaxiSelectionTranslatePanel v-if="selectionTranslatePanelVisible" :pos="selectionTranslatePanelPos"
        :raw="selectionTranslateRaw" :result="selectionTranslateResult" :error="selectionTranslateError"
        :busy="selectionTranslateBusy" :pinned="selectionTranslatePinned" :items="selectionTranslateItems"
        :word-meta="selectionTranslateWordMeta" :from-lang="selectionTranslateFrom" :to-lang="selectionTranslateTo"
        :lang-options="selectionTranslateLangOptions"
        @update:from-lang="(v) => (selectionTranslateFrom = String(v || 'auto'))"
        @update:to-lang="(v) => (selectionTranslateTo = String(v || 'zh'))" @swap-langs="swapSelectionTranslateLangs"
        @translate="translateSelectionText" @copy="copySelectionTranslateResult"
        @toggle-pin="toggleSelectionTranslatePinned" @update:pos="handleSelectionTranslatePanelUpdatePos"
        @close="closeSelectionTranslatePanel" />

    </div>
    <!-- 添加 GitPanel 组件 -->
    <!--
    <GitPanel ref="gitPanel" :workspace-path="fileTreeStore.activeRepoPath" />
    -->
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, watch, onMounted, onUnmounted, onActivated, nextTick, provide, h } from 'vue'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiEditorStatusBar from './GuaxiEditorStatusBar.vue'
import GuaxiSelectionTranslatePanel from './GuaxiSelectionTranslatePanel.vue'
import { NTooltip, NIcon, useMessage, NDropdown, NPopover, NInputNumber, NBreadcrumb, NBreadcrumbItem } from 'naive-ui'
import { Icon } from '@iconify/vue'
import { EyeOutline, LogoVimeo, CloseOutline, ExitOutline, SaveOutline, ArrowBackOutline, ArrowForwardOutline } from '@vicons/ionicons5'
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
import bookOpenIcon from '@iconify-icons/lucide/book-open'
import languagesIcon from '@iconify-icons/lucide/languages'
import messageCircleIcon from '@iconify-icons/lucide/message-circle'
import botIcon from '@iconify-icons/lucide/bot'
import volume2Icon from '@iconify-icons/lucide/volume-2'
import folderIcon from '@iconify-icons/lucide/folder'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import hashIcon from '@iconify-icons/lucide/hash'

import tableRowPlusAfter from '@iconify-icons/mdi/table-row-plus-after'
import tableRowRemove from '@iconify-icons/mdi/table-row-remove'
import tableColumnPlusAfter from '@iconify-icons/mdi/table-column-plus-after'
import tableColumnRemove from '@iconify-icons/mdi/table-column-remove'
import { dirname, homeDir } from '@tauri-apps/api/path'
import { openUrl } from '@tauri-apps/plugin-opener'
import { renderMarkdown, addCopyButtons, addMediaViewerButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import Vditor from 'vditor'
import 'vditor/dist/index.css'
import 'vditor/dist/method'
import { debounce } from 'lodash-es'
import * as monaco from 'monaco-editor'
import { initVimMode } from 'monaco-vim';
import { initTextmate } from '../../monaco/textmateLoader'
import { initLspClient, disposeAllLspClients, getLspConnectionState, resetLspDisabled } from '../../monaco/lspClient'
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
import { listen, emit } from '@tauri-apps/api/event'
import { save, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'

const logger = createLogger('GuaxiEditor')
const outlineStore = useOutlineStore()

const ensureLatexLanguage = () => {
  const exists = monaco.languages.getLanguages().some((lang) => lang.id === 'latex')
  if (exists) return
  monaco.languages.register({
    id: 'latex',
    extensions: ['.tex', '.ltx', '.latex', '.cls', '.sty'],
    aliases: ['LaTeX', 'TeX', 'latex', 'tex'],
    mimetypes: ['text/x-tex', 'text/x-latex']
  })
  logger.info('[Editor] language registered', { languageId: 'latex' })
}

ensureLatexLanguage()
// 添加 GitPanel 组件引用
const gitPanel = ref(null)


// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const settingsStore = useSettingsStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const rightSiderStore = useRightSiderStore()
const siderStore = useSiderStore()
const mode = ref('edit');
const previewMode = ref('toggle');
const layoutMode = computed(() => fileTreeStore.layoutMode || 'single');
const renderedHtml = ref('');
const lastPreviewRenderedSource = ref('');
const previewContainer = ref(null);

const openMarkdownManual = async () => {
  const windowLabel = 'markdown-manual'
  const windowTitle = 'Markdown语法查询手册'

  try {
    await invoke('create_markdown_manual_window', {
      label: windowLabel,
      title: windowTitle,
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('Markdown 语法手册窗口已打开')
      return
    }
    logger.error('[MarkdownManual] open window failed:', err)
    message.error('打开 Markdown 语法手册失败')
  }
}

const editorContainer = ref(null);
const secondaryEditorContainer = ref(null);
const editorWrapperRef = ref(null);
const editorHeaderRef = ref(null);
const previewShellRef = ref(null);
const floatingToolbarVisible = ref(false);
const floatingToolbarStyle = ref({ left: '0px', top: '0px' });
const previewFloatingToolbarVisible = ref(false)
const previewFloatingToolbarStyle = ref({ left: '0px', top: '0px' })
const previewSelectionText = ref('')
const selectionSurface = ref('editor')

const selectionTranslatePanelVisible = ref(false);
const selectionTranslatePinned = ref(false);
const selectionTranslatePanelPos = ref({ left: 8, top: 8 });

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

const selectionTranslateBusy = ref(false);
const selectionTranslateRaw = ref('');
const selectionTranslateResult = ref('');
const selectionTranslateError = ref('');
const selectionTranslateItems = ref([]);
const selectionTranslateWordMeta = ref(null)
const selectionReadAloudBusy = ref(false)

let selectionAudioElement = null
let selectionAudioObjectUrl = ''

const selectionTranslateMultiAvailable = ref(true)
let selectionTranslateMultiNotFoundWarned = false

const scheduleSelectionTranslate = debounce(() => {
  if (!selectionTranslatePanelVisible.value) return
  if (selectionTranslateBusy.value) return

  const raw = String(selectionTranslateRaw.value || '').trim()
  if (!raw) return

  translateSelectionText(raw)
}, 250)

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

const stopSelectionAudioPlayback = () => {
  if (selectionAudioElement) {
    try {
      selectionAudioElement.pause()
      selectionAudioElement.src = ''
    } catch { }
    selectionAudioElement = null
  }
  if (selectionAudioObjectUrl) {
    try {
      URL.revokeObjectURL(selectionAudioObjectUrl)
    } catch { }
    selectionAudioObjectUrl = ''
  }
}

const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

const readSelectionAloud = async () => {
  if (selectionReadAloudBusy.value) return
  const text = normalizeSelectionTextForLanguageTasks(getSelectedTextForTranslate())
  if (!text) {
    message.warning('请先框选要朗读的文本')
    return
  }

  selectionReadAloudBusy.value = true
  try {
    const response = await invoke('audio_text_to_speech', {
      request: {
        common: {
          provider: 'siliconflow',
          model: 'FunAudioLLM/CosyVoice2-0.5B',
        },
        input: text,
        voice: 'FunAudioLLM/CosyVoice2-0.5B:alex',
        response_format: 'mp3',
        stream: false,
      },
    })
    const audioBase64 = String(response?.audio_base64 || '')
    if (!audioBase64) {
      throw new Error('语音数据为空')
    }
    const contentType = String(response?.content_type || 'audio/mpeg')
    const bytes = base64ToUint8Array(audioBase64)
    const blob = new Blob([bytes], { type: contentType })
    stopSelectionAudioPlayback()
    selectionAudioObjectUrl = URL.createObjectURL(blob)
    const audio = new Audio(selectionAudioObjectUrl)
    selectionAudioElement = audio
    audio.onended = () => {
      stopSelectionAudioPlayback()
    }
    await audio.play()
  } catch (err) {
    logger.error('朗读失败:', err)
    message.error(`朗读失败: ${String(err?.message || err || '')}`)
  } finally {
    selectionReadAloudBusy.value = false
  }
}

watch([selectionTranslateFrom, selectionTranslateTo], ([from, to], [prevFrom, prevTo]) => {
  if (!selectionTranslatePanelVisible.value) return
  if (from === prevFrom && to === prevTo) return
  scheduleSelectionTranslate()
})

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
let tableInsertAutoFormatBusy = false;
let lastTableToolbarInsertAt = 0;
const scrollRatio = ref(0);
const pendingPreviewScrollRestore = ref(false);
const pendingPreviewScrollLine = ref(null);
const pendingRealtimeScrollRestore = ref(false);
const realtimeVditorId = 'realtime-vditor';
const realtimeVditorReady = ref(false);
const pendingRealtimeVditorValue = ref(null);
let realtimeVditor = null;
let settingRealtimeVditorValue = false;
let realtimeVditorLinkInterceptorCleanup = null;

const syncRealtimeValueToRawContent = debounce((value) => {
  rawContent.value = value;
  const path = fileTreeStore.activeTab;
  if (path) fileTreeStore.updateTabContent(path, value);
}, 120);

let editor = null;
let secondaryEditor = null;
let monacoCtrlClickDisposable = null;
let secondaryMonacoCtrlClickDisposable = null;
let monacoLinkLikeDecorDisposable = null;
let secondaryMonacoLinkLikeDecorDisposable = null;
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

const previewLocationBackStack = ref([]);
const previewLocationForwardStack = ref([]);
let lastRecordedPreviewLocationKey = null;
let lastRecordedPreviewLocationAt = 0;
let suppressPreviewLocationRecording = false;
const maxPreviewLocationHistory = 200;

const getCurrentPreviewLocation = () => {
  const path = fileTreeStore.activeTab || selectedFile.value?.path;
  const container = previewContainer.value;
  if (!path || !(container instanceof HTMLElement)) return null;

  const scrollTop = container.scrollTop;
  const height = container.scrollHeight;
  const scrollRatio = height ? scrollTop / height : 0;

  const containerRect = container.getBoundingClientRect();
  const els = Array.from(container.querySelectorAll('[data-md-start-line]') || []);
  if (!els.length) return { path, lineNumber: 1, scrollTop, scrollRatio };

  let bestLine = 1;
  let bestScore = Number.POSITIVE_INFINITY;

  for (const el of els) {
    const start = Number(el?.dataset?.mdStartLine || 0);
    if (!Number.isFinite(start) || start <= 0) continue;

    const r = el.getBoundingClientRect();
    const visible = r.bottom > containerRect.top + 8 && r.top < containerRect.bottom - 8;
    const score = visible ? Math.abs(r.top - containerRect.top) : (Math.abs(r.top - containerRect.top) + 100000);
    if (score < bestScore) {
      bestScore = score;
      bestLine = start;
    }
  }

  return { path, lineNumber: bestLine || 1, scrollTop, scrollRatio };
};

const isSamePreviewLocation = (a, b) => {
  if (!a || !b) return false;
  if (a.path !== b.path) return false;

  const at = Number(a.scrollTop);
  const bt = Number(b.scrollTop);
  if (Number.isFinite(at) && Number.isFinite(bt)) {
    return Math.round(at / 8) === Math.round(bt / 8);
  }

  if (
    Number(a.lineNumber) > 0 &&
    Number(b.lineNumber) > 0 &&
    Number(a.lineNumber) === Number(b.lineNumber)
  ) {
    const ai = a.hitIndex;
    const bi = b.hitIndex;
    if (ai == null || bi == null) return true;
    return Number(ai) === Number(bi);
  }

  return false;
};

const recordPreviewLocation = (reason, overrideLoc = null) => {
  if (suppressPreviewLocationRecording) return;

  const loc = overrideLoc || getCurrentPreviewLocation();
  if (!loc) return;

  const now = Date.now();
  const st = Number(loc.scrollTop);
  const stKey = Number.isFinite(st) ? Math.round(st / 8) : 0;
  const key = `${loc.path}:${stKey}`;
  if (key === lastRecordedPreviewLocationKey && now - lastRecordedPreviewLocationAt < 350) return;

  const back = previewLocationBackStack.value;
  const last = back[back.length - 1];
  if (last && isSamePreviewLocation(last, loc)) {
    lastRecordedPreviewLocationKey = key;
    lastRecordedPreviewLocationAt = now;
    return;
  }

  back.push({
    ...loc,
    scrollTop: Number.isFinite(st) ? st : 0,
    scrollTopKey: stKey,
    ts: now,
    reason: String(reason || '')
  });
  if (back.length > maxPreviewLocationHistory) back.shift();
  previewLocationForwardStack.value = [];

  lastRecordedPreviewLocationKey = key;
  lastRecordedPreviewLocationAt = now;

  logger.debug('[PreviewNav] record', { reason, path: loc.path, line: loc.lineNumber, back: back.length });
};

const goToPreviewLocation = async (loc, { source = 'shortcut' } = {}) => {
  if (!loc) return false;
  const path = String(loc.path || '');
  const st = Number(loc.scrollTop);
  const ratio = Number(loc.scrollRatio);
  const line = Number(loc.lineNumber || 0);
  if (!path) return false;

  suppressPreviewLocationRecording = true;
  try {
    if (fileTreeStore.activeTab !== path) {
      await fileTreeStore.openFileInTab(path);
      await nextTick();
    }

    await nextTick();

    const container = previewContainer.value;
    if (container && container instanceof HTMLElement) {
      const maxTop = Math.max(0, container.scrollHeight - container.clientHeight);
      let nextTop = null;

      if (Number.isFinite(st)) {
        nextTop = Math.min(Math.max(0, st), maxTop);
      } else if (Number.isFinite(ratio) && ratio > 0) {
        nextTop = Math.min(Math.max(0, ratio * container.scrollHeight), maxTop);
      }

      if (nextTop != null) {
        container.scrollTop = nextTop;
      } else if (Number.isFinite(line) && line > 0) {
        scrollPreviewToLine(line);
      }
    } else if (Number.isFinite(line) && line > 0) {
      scrollPreviewToLine(line);
    }

    logger.info('[PreviewNav] go', {
      source,
      path,
      scrollTop: Number.isFinite(st) ? st : 'na',
      line: Number.isFinite(line) ? line : 'na'
    });
    return true;
  } finally {
    window.setTimeout(() => {
      suppressPreviewLocationRecording = false;
    }, 0);
  }
};

const navigateToPreviousPreviewLocation = async () => {
  const current = getCurrentPreviewLocation();
  if (!current) return;

  const back = previewLocationBackStack.value;
  const forward = previewLocationForwardStack.value;

  const last = back[back.length - 1];
  if (!last || !isSamePreviewLocation(last, current)) {
    back.push({ ...current, ts: Date.now(), reason: 'implicit-current' });
    if (back.length > maxPreviewLocationHistory) back.shift();
  }

  if (back.length < 2) {
    message.info('没有上一个预览位置');
    return;
  }

  const from = back.pop();
  forward.push(from);
  const target = back[back.length - 1];
  await goToPreviewLocation(target, { source: 'alt-left' });
};

const navigateToNextPreviewLocation = async () => {
  const forward = previewLocationForwardStack.value;
  if (forward.length === 0) {
    message.info('没有下一个预览位置');
    return;
  }

  const target = forward.pop();
  const back = previewLocationBackStack.value;
  const last = back[back.length - 1];
  if (!last || !isSamePreviewLocation(last, target)) {
    back.push(target);
    if (back.length > maxPreviewLocationHistory) back.shift();
  }

  await goToPreviewLocation(target, { source: 'alt-right' });
};

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

// 面包屑导航：点击后联动文件树定位/展开
const EVENT_REVEAL_IN_FILE_TREE = 'guaxi_editor_reveal_in_file_tree'

// 统一路径分隔符（用于面包屑展示与前缀判断）
const normalizeSlashes = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$|\s+$/g, '').trim()

const cursorLineNumber = computed(() => {
  const n = Number(editorStore.cursorPosition?.lineNumber || 1)
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

// 从 repopathlist 中找出“最匹配”的知识库根路径
const findRepoRootForPath = (absPath) => {
  const target = normalizeSlashes(absPath)
  if (!target) return ''

  const repos = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  let best = ''
  for (const raw of repos) {
    const repo = normalizeSlashes(raw)
    if (!repo) continue
    if (target === repo || target.startsWith(`${repo}/`)) {
      if (repo.length > best.length) best = repo
    }
  }

  // 兜底：如果当前 activeRepoPath 能匹配，也返回
  const active = normalizeSlashes(fileTreeStore.activeRepoPath)
  if (!best && active && (target === active || target.startsWith(`${active}/`))) {
    best = active
  }

  return best
}

// 折叠过长的路径面包屑，避免把“标题/函数”等大纲面包屑挤出可视区（行为参考 VSCode：保留头部与尾部，中间用“…”折叠）
const collapsePathBreadcrumbItems = (items, maxVisible = 4) => {
  const arr = Array.isArray(items) ? items : []
  if (arr.length <= maxVisible) return arr

  const head = arr[0]
  const tailCount = Math.max(1, maxVisible - 2)
  const tail = arr.slice(-tailCount)

  const full = arr.map((it) => String(it?.title || it?.label || '')).filter(Boolean).join(' › ')

  return [
    head,
    {
      key: `ellipsis:${head?.key || 'path'}`,
      label: '…',
      title: full || '路径已折叠',
      kind: 'ellipsis',
      disabled: true
    },
    ...tail
  ]
}

const buildBreadcrumbItems = (absPath) => {
  const target = normalizeSlashes(absPath)
  if (!target) {
    return [{ key: 'placeholder', label: '未打开文件', title: '未打开文件', icon: fileTextIcon, kind: 'placeholder', disabled: true }]
  }

  const repoRoot = findRepoRootForPath(target)
  const items = []

  if (repoRoot) {
    const repoName = repoRoot.split('/').pop() || repoRoot
    items.push({ key: `repo:${repoRoot}`, label: repoName, title: repoRoot, icon: bookOpenIcon, kind: 'repo', path: repoRoot })

    const rel = target === repoRoot ? '' : target.slice(repoRoot.length + 1)
    const parts = rel ? rel.split('/').filter(Boolean) : []

    let cursor = repoRoot
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      cursor = `${cursor}/${part}`
      const isLast = i === parts.length - 1
      items.push({
        key: `path:${cursor}`,
        label: part,
        title: cursor,
        icon: isLast ? fileTextIcon : folderIcon,
        kind: isLast ? 'file' : 'dir',
        path: cursor
      })
    }

    return collapsePathBreadcrumbItems(items)
  }

  const leaf = target.split('/').pop() || target
  return [{ key: `leaf:${target}`, label: leaf, title: target, icon: fileTextIcon, kind: 'leaf', path: target }]
}

const breadcrumbItems = computed(() => {
  const absPath = activeTab.value?.path || selectedFile.value?.path || ''
  const pathItems = buildBreadcrumbItems(absPath)
  const headingItems = buildHeadingBreadcrumbItems(safeOutlineFlat.value, cursorLineNumber.value)
  return [...pathItems, ...headingItems]
})

const handleBreadcrumbClick = async (it) => {
  if (!it || it.disabled) return

  if (it.kind === 'heading' && Number.isFinite(Number(it.line))) {
    const ln = Number(it.line)
    logger.info('[Breadcrumb] jump to heading', { line: ln, label: it.label })
    try {
      jumpToLine(ln)
    } catch (e) {
      logger.warn('[Breadcrumb] jump failed:', e)
    }
    return
  }

  const path = it?.path ? String(it.path) : ''
  if (!path) return

  logger.info('[Breadcrumb] reveal in file tree', { label: it?.label, path })
  try {
    await emit(EVENT_REVEAL_IN_FILE_TREE, { path })
  } catch (e) {
    logger.warn('[Breadcrumb] emit reveal event failed:', e)
  }
}

const breadcrumbScrollRef = ref(null)

const handleBreadcrumbWheel = (e) => {
  const el = breadcrumbScrollRef.value
  if (!el) return

  const dx = Number(e?.deltaX || 0)
  const dy = Number(e?.deltaY || 0)
  const delta = Math.abs(dy) >= Math.abs(dx) ? dy : dx

  if (!Number.isFinite(delta) || delta === 0) return

  el.scrollLeft += delta
}

const breadcrumbPickerVisible = ref(false)
const breadcrumbPickerOptions = ref([])
const breadcrumbPickerPosition = ref({ x: 0, y: 0 })
const breadcrumbPickerPayloadByKey = ref(new Map())

const closeBreadcrumbPicker = () => {
  breadcrumbPickerVisible.value = false
  breadcrumbPickerOptions.value = []
  breadcrumbPickerPayloadByKey.value = new Map()
}

const normalizeBreadcrumbPath = (path) => {
  const raw = normalizeSlashes(path)
  if (!raw) return ''
  if (raw.match(/^[a-zA-Z]:\//)) return raw[0].toUpperCase() + raw.slice(1)
  return raw
}

const isHiddenPathForBreadcrumb = (path) => {
  const p = normalizeBreadcrumbPath(path)
  if (!p) return false
  return p.split('/').some((part) => String(part || '').startsWith('.'))
}

const getFileTreeIndex = (() => {
  let cachedTreeRef = null
  let cachedRootId = ''
  let cached = null

  const walk = (nodes, parentId, parentById, childrenByParent, nodeById) => {
    if (!Array.isArray(nodes)) return
    for (const node of nodes) {
      const id = normalizeBreadcrumbPath(node?.id)
      if (!id) continue

      nodeById.set(id, node)
      if (parentId) parentById.set(id, parentId)

      const list = childrenByParent.get(parentId) || []
      list.push(node)
      childrenByParent.set(parentId, list)

      if (Array.isArray(node?.children) && node.children.length) {
        walk(node.children, id, parentById, childrenByParent, nodeById)
      }
    }
  }

  return (rootId) => {
    const treeRef = fileTreeStore.fileTree
    const rid = normalizeBreadcrumbPath(rootId)
    if (treeRef && treeRef === cachedTreeRef && rid && rid === cachedRootId && cached) return cached

    const parentById = new Map()
    const childrenByParent = new Map()
    const nodeById = new Map()

    walk(treeRef, rid, parentById, childrenByParent, nodeById)

    cachedTreeRef = treeRef
    cachedRootId = rid
    cached = { parentById, childrenByParent, nodeById, rootId: rid }
    return cached
  }
})()

const buildOutlineTreeWithParents = (flatOutline) => {
  const stack = []
  const root = []
  const byKey = new Map()

  const arr = Array.isArray(flatOutline) ? flatOutline : []
  arr.forEach((item, idx) => {
    const level = Number(item?.level || 0)
    const text = String(item?.text || '').trim()
    const line = Number(item?.line || 0)
    if (!text || !Number.isFinite(level) || level <= 0 || !Number.isFinite(line) || line <= 0) return

    const key = `heading:${line}:${level}:${text}`
    const node = { key, level, text, line, parent: null, children: [], _idx: idx }

    while (stack.length && stack[stack.length - 1].level >= node.level) stack.pop()

    if (stack.length === 0) {
      root.push(node)
    } else {
      node.parent = stack[stack.length - 1]
      node.parent.children.push(node)
    }

    stack.push(node)
    byKey.set(key, node)
  })

  return { root, byKey }
}

const makeBreadcrumbPickerOption = (payload, isActive) => {
  const key = payload.key
  return {
    key,
    label: () => h('div', { class: ['breadcrumb-picker-item', isActive ? 'active' : ''] }, [
      payload.icon ? h(Icon, { icon: payload.icon, width: 14, height: 14, class: 'breadcrumb-picker-icon' }) : null,
      h('span', { class: 'breadcrumb-picker-label', title: payload.title || payload.label }, payload.label),
    ])
  }
}

const openBreadcrumbPicker = (it, ev) => {
  if (!it || it.disabled) return

  const x = Number(ev?.clientX || 0)
  const y = Number(ev?.clientY || 0)
  breadcrumbPickerPosition.value = { x, y }

  const payloadByKey = new Map()
  let payloads = []

  if (it.kind === 'heading') {
    const tree = buildOutlineTreeWithParents(safeOutlineFlat.value)
    const node = tree.byKey.get(String(it.key || ''))
    const siblings = node?.parent ? node.parent.children : tree.root

    payloads = (siblings || []).map((n) => ({
      key: `H:${n.key}`,
      label: n.text,
      title: `Ln ${n.line}: ${n.text}`,
      icon: hashIcon,
      kind: 'heading',
      line: n.line,
    }))
  } else {
    if (it.kind === 'repo') {
      payloads = (Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []).map((p) => {
        const path = normalizeBreadcrumbPath(p)
        const name = path.split('/').pop() || path
        return { key: `P:${path}`, label: name, title: path, icon: bookOpenIcon, kind: 'repo', path, isFile: false }
      })
    } else {
      const target = normalizeBreadcrumbPath(it.path)
      const repoRoot = normalizeBreadcrumbPath(findRepoRootForPath(target) || fileTreeStore.activeRepoPath || '')
      const index = getFileTreeIndex(repoRoot)

      const parentId = index?.parentById?.get(target) || repoRoot
      const siblingsRaw = index?.childrenByParent?.get(parentId) || []

      payloads = siblingsRaw
        .map((n) => {
          const path = normalizeBreadcrumbPath(n?.id)
          if (!path) return null
          if (isHiddenPathForBreadcrumb(path)) return null

          const label = String(n?.label || path.split('/').pop() || path)
          const isDir = Array.isArray(n?.children) && n.children.length > 0
          return {
            key: `P:${path}`,
            label,
            title: path,
            icon: isDir ? folderIcon : fileTextIcon,
            kind: isDir ? 'dir' : 'file',
            path,
            isFile: !isDir
          }
        })
        .filter(Boolean)
        .sort((a, b) => String(a.label || '').localeCompare(String(b.label || ''), 'zh-CN'))
    }
  }

  if (!payloads.length) {
    logger.info('[BreadcrumbPicker] no siblings, fallback click', { key: it.key, kind: it.kind })
    handleBreadcrumbClick(it)
    return
  }

  for (const p of payloads) payloadByKey.set(p.key, p)

  breadcrumbPickerPayloadByKey.value = payloadByKey
  breadcrumbPickerOptions.value = payloads.map((p) => {
    const isActive = it.kind === 'heading'
      ? String(it.key || '') === String(p.key || '').replace(/^H:/, '')
      : normalizeBreadcrumbPath(it.path) === normalizeBreadcrumbPath(p.path)

    return makeBreadcrumbPickerOption(p, isActive)
  })

  breadcrumbPickerVisible.value = true
  logger.info('[BreadcrumbPicker] opened', { kind: it.kind, key: it.key, count: payloads.length })
}

const handleBreadcrumbPickerSelect = async (key) => {
  const payload = breadcrumbPickerPayloadByKey.value?.get?.(key)
  closeBreadcrumbPicker()
  if (!payload) return

  if (payload.kind === 'heading') {
    logger.info('[BreadcrumbPicker] select heading', { line: payload.line, label: payload.label })
    handleBreadcrumbClick({ kind: 'heading', line: payload.line, label: payload.label })
    return
  }

  const path = payload.path
  if (!path) return

  logger.info('[BreadcrumbPicker] select path', { path, kind: payload.kind, isFile: !!payload.isFile })

  try {
    if (payload.isFile) {
      await fileTreeStore.openFileInTab(path)
    }
    await emit(EVENT_REVEAL_IN_FILE_TREE, { path })
  } catch (e) {
    logger.warn('[BreadcrumbPicker] select failed:', e)
  }
}

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
const PENDING_TEMPLATE_INSERT_KEY = 'guaxi.pending_template_insert'
const TEMPLATE_INSERT_DEDUP_MS = 1500
let lastTemplateInsertPayload = ''
let lastTemplateInsertAt = 0

const isDuplicateTemplateInsert = (raw) => {
  const text = String(raw || '')
  if (!text) return false
  const now = Date.now()
  return text === lastTemplateInsertPayload && (now - lastTemplateInsertAt) < TEMPLATE_INSERT_DEDUP_MS
}

const markTemplateInsertApplied = (raw) => {
  lastTemplateInsertPayload = String(raw || '')
  lastTemplateInsertAt = Date.now()
}

const applyTemplateInsert = async (content) => {
  const raw = typeof content === 'string' ? content : ''
  if (!raw) return false
  if (isDuplicateTemplateInsert(raw)) return true
  const text = raw.endsWith('\n') ? raw : `${raw}\n`
  const inserted = !!(await editorStore.insertContent(text))
  if (inserted) {
    markTemplateInsertApplied(raw)
  }
  return inserted
}

const consumePendingTemplateInsert = async () => {
  try {
    const pending = localStorage.getItem(PENDING_TEMPLATE_INSERT_KEY) || ''
    if (!pending) return
    const inserted = await applyTemplateInsert(pending)
    if (inserted) {
      localStorage.removeItem(PENDING_TEMPLATE_INSERT_KEY)
    }
  } catch (e) {
    logger.debug('处理待插入模板内容失败:', e)
  }
}
const safeUnlistenTemplateInsert = () => {
  if (typeof unlistenTemplateInsert !== 'function') return
  try {
    Promise.resolve(unlistenTemplateInsert()).catch((e) => {
      logger.debug('反注册模板插入事件失败:', e)
    })
  } catch (e) {
    logger.debug('反注册模板插入事件失败:', e)
  }
  unlistenTemplateInsert = null
}

onMounted(async () => {
  templateInsertListenerUnmounted = false
  try {
    const unlisten = await listen('template_insert_event', async (event) => {
      const content = typeof event?.payload?.content === 'string' ? event.payload.content : ''
      if (!content) return

      const inserted = await applyTemplateInsert(content)
      if (!inserted) {
        localStorage.setItem(PENDING_TEMPLATE_INSERT_KEY, content)
        message.warning('插入模板失败：请先打开一个文件')
      } else {
        localStorage.removeItem(PENDING_TEMPLATE_INSERT_KEY)
      }
    })

    if (templateInsertListenerUnmounted) {
      try {
        Promise.resolve(unlisten()).catch((e) => {
          logger.debug('反注册模板插入事件失败:', e)
        })
      } catch (e) {
        logger.debug('反注册模板插入事件失败:', e)
      }
      return
    }

    unlistenTemplateInsert = unlisten
    await consumePendingTemplateInsert()
  } catch (e) {
    logger.error('注册模板插入事件失败:', e)
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

const codeBlockOptions = [
  { label: '纯代码块', key: 'plain' },
  { label: 'C', key: 'c' },
  { label: 'C++', key: 'cpp' },
  { label: 'Python', key: 'python' },
  { label: 'Shell', key: 'bash' },
  { label: 'Mermaid', key: 'mermaid' },
  { label: 'Rust', key: 'rust' },
  { label: 'JavaScript', key: 'javascript' },
  { label: 'TypeScript', key: 'typescript' },
  { label: 'Lua', key: 'lua' },
  { label: 'HTML', key: 'html' },
  { label: 'Perl', key: 'perl' },
  { label: 'JSON', key: 'json' },
  { label: 'XML', key: 'xml' },
  { label: 'TeX', key: 'tex' },
  { label: 'C#', key: 'csharp' },
  { label: 'Lisp', key: 'lisp' },
  { label: 'TOML', key: 'toml' },
  { label: 'PowerShell', key: 'powershell' }
]

const tableInsertPopoverVisible = ref(false)
const tableGridMaxRows = 10
const tableGridMaxCols = 10
const tableGridRows = ref(2)
const tableGridCols = ref(2)
const tableGridDragging = ref(false)

const tableInsertMaxRows = 120
const tableInsertMaxCols = 40
const tableInputRows = ref(2)
const tableInputCols = ref(2)

const tableInsertHint = computed(() => `${tableGridRows.value} 行 × ${tableGridCols.value} 列`)

const handleTableInsertPopoverUpdateShow = (val) => {
  tableInsertPopoverVisible.value = val
  if (val) {
    tableGridRows.value = 2
    tableGridCols.value = 2
    tableGridDragging.value = false
    tableInputRows.value = 2
    tableInputCols.value = 2
  } else {
    tableGridDragging.value = false
  }
}

const clampInt = (value, min, max) => {
  const n = Number(value)
  if (!Number.isFinite(n)) return min
  return Math.max(min, Math.min(max, Math.floor(n)))
}

const buildMarkdownTable = (rows, cols) => {
  const r = clampInt(rows, 2, tableInsertMaxRows)
  const c = clampInt(cols, 1, tableInsertMaxCols)

  const header = Array.from({ length: c }, (_, i) => `列${i + 1}`)
  const sep = Array.from({ length: c }, () => '---')

  const lines = []
  lines.push(`| ${header.join(' | ')} |`)
  lines.push(`| ${sep.join(' | ')} |`)

  const bodyRows = Math.max(1, r - 1)
  for (let i = 0; i < bodyRows; i += 1) {
    const cells = Array.from({ length: c }, (_, idx) => (i === 0 ? `内容${idx + 1}` : ''))
    lines.push(`| ${cells.join(' | ')} |`)
  }

  return `${lines.join('\n')}\n`
}

const insertTableWithSize = (rows, cols) => {
  if (!ensureMarkdownEditor()) return

  const model = editor.getModel()
  const pos = editor.getPosition()
  if (!model || !pos) return

  lastTableToolbarInsertAt = Date.now();

  const snippet = buildMarkdownTable(rows, cols)
  insertBlockSnippet(snippet)

  nextTick(() => {
    const line = pos.lineNumber + 2
    editor.setPosition(new monaco.Position(line, 3))
    editor.focus()
    updateTableToolbar()
    formatTable()
  })
}

const handleTableGridMouseDown = (r, c) => {
  tableGridDragging.value = true
  tableGridRows.value = clampInt(r, 1, tableGridMaxRows)
  tableGridCols.value = clampInt(c, 1, tableGridMaxCols)
}

const handleTableGridMouseOver = (r, c) => {
  if (!tableGridDragging.value) {
    tableGridRows.value = clampInt(r, 1, tableGridMaxRows)
    tableGridCols.value = clampInt(c, 1, tableGridMaxCols)
    return
  }
  tableGridRows.value = clampInt(r, 1, tableGridMaxRows)
  tableGridCols.value = clampInt(c, 1, tableGridMaxCols)
}

const handleTableGridMouseUp = () => {
  if (!tableInsertPopoverVisible.value) return
  if (!tableGridDragging.value) return
  tableGridDragging.value = false

  insertTableWithSize(tableGridRows.value, tableGridCols.value)
  tableInsertPopoverVisible.value = false
}

const handleTableGridMouseLeave = () => {
  if (!tableGridDragging.value) return
  tableGridDragging.value = false
}

const insertTableFromInputs = () => {
  const r = clampInt(tableInputRows.value, 2, tableInsertMaxRows)
  const c = clampInt(tableInputCols.value, 1, tableInsertMaxCols)
  insertTableWithSize(r, c)
  tableInsertPopoverVisible.value = false
}

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

const stripLinkBoundaryPunct = (raw) => {
  let s = String(raw || '').trim();
  if (!s) return '';
  s = s.replace(/^(["'`<({\[])+/, '');
  s = s.replace(/(["'`>)}\].,;!?])+$/, '');
  return s;
};

const isHttpUrl = (raw) => {
  const s = String(raw || '').trim().toLowerCase();
  return s.startsWith('http://') || s.startsWith('https://');
};

// 从 `path.md#标题` / `path.md#^blockId` 中拆出 anchor（仅用于非 http 链接）
const splitFsLinkAndAnchor = (rawLink) => {
  const raw = stripLinkBoundaryPunct(rawLink);
  if (!raw) return { link: '', anchor: '' };
  if (isHttpUrl(raw)) return { link: raw, anchor: '' };

  const idx = raw.indexOf('#');
  if (idx <= 0) return { link: raw, anchor: '' };
  return { link: raw.slice(0, idx), anchor: raw.slice(idx + 1) };
};

// 说明：为避免误判普通文本，仅对“明显路径特征”放行；另外允许 `xxx.md` 这种相对文件名。
const isPathLikeToken = (raw) => {
  const s0 = String(raw || '').trim();
  if (!s0) return false;
  const s = stripLinkBoundaryPunct(s0);
  if (!s) return false;

  const lower = s.toLowerCase();
  if (lower.startsWith('file://')) return true;
  if (s.startsWith('~/') || s.startsWith('./') || s.startsWith('../')) return true;
  if (/^[a-zA-Z]:[\\/]/.test(s)) return true;
  if (s.includes('/') || s.includes('\\')) return true;
  return /\.(md|markdown)$/i.test(s);
};

// Markdown 链接目标提取：当光标位于 `](` 与 `)` 之间时，返回括号内真实目标
const extractMarkdownLinkTargetAtLineColumn = (line, idx0) => {
  const i = Math.max(0, Math.min(line.length, idx0));

  const left = line.lastIndexOf('](', i);
  if (left >= 0) {
    const open = left + 1; // '(' index
    const close = line.indexOf(')', open + 1);
    if (close > open + 1 && i >= open + 1 && i <= close - 1) {
      return line.slice(open + 1, close).trim();
    }
  }

  const lt = line.lastIndexOf('<', i);
  if (lt >= 0) {
    const gt = line.indexOf('>', lt + 1);
    if (gt > lt + 1 && i >= lt + 1 && i <= gt - 1) {
      return line.slice(lt + 1, gt).trim();
    }
  }

  return '';
};

// 统一生成内部 wikilink href（给 Monaco Ctrl+Click / Preview / Realtime 共用）
const buildGuaxiWikilinkHref = (filePart, anchorPart) => {
  const file = String(filePart || '').trim();
  const anchor = String(anchorPart || '').trim();
  return `guaxi://wikilink?file=${encodeURIComponent(file)}&anchor=${encodeURIComponent(anchor)}`;
};

// Obsidian 双链提取：当光标位于 `[[...]]` 内时，返回内部可点击 href
// 支持：[[file]] / [[file#heading]] / [[file#^blockId]] / [[#heading]] / [[file|alias]]
const extractObsidianWikilinkHrefAtLineColumn = (line, idx0) => {
  const i = Math.max(0, Math.min(line.length, idx0));
  const left = line.lastIndexOf('[[', i);
  if (left < 0) return '';
  const right = line.indexOf(']]', left + 2);
  if (right < 0) return '';
  if (i < left + 2 || i > right - 1) return '';

  const inner = String(line.slice(left + 2, right) || '').trim();
  if (!inner) return '';

  const parts = inner.split('|');
  const target = String(parts[0] || '').trim();
  if (!target) return '';

  let filePart = target;
  let anchorPart = '';
  const hashIdx = target.indexOf('#');
  if (hashIdx >= 0) {
    filePart = target.slice(0, hashIdx).trim();
    anchorPart = target.slice(hashIdx + 1).trim();
  }

  return buildGuaxiWikilinkHref(filePart, anchorPart);
};

// Monaco 行内 token 提取：优先走 Obsidian/Markdown 语法提取，避免把 `xxx](` 左侧吞进来
const extractClickableTokenAtLineColumn = (lineContent, column) => {
  const line = String(lineContent || '');
  const idx0 = Math.max(0, Math.min(line.length, Number(column || 1) - 1));
  if (!line) return '';

  const wikiHref = extractObsidianWikilinkHrefAtLineColumn(line, idx0);
  if (wikiHref) return wikiHref;

  const mdTarget = extractMarkdownLinkTargetAtLineColumn(line, idx0);
  if (mdTarget) return mdTarget;

  // 注意：这里刻意不把 `[` `]` 当作 token 字符，避免得到类似 `path](../a.md` 的 token
  const isLinkChar = (ch) => /[0-9A-Za-z\-._~:/?#@!$&'()*+,;=%\\]/.test(ch);

  let l = idx0;
  while (l > 0 && isLinkChar(line[l - 1])) l -= 1;

  let r = idx0;
  while (r < line.length && isLinkChar(line[r])) r += 1;

  if (r <= l) return '';
  return line.slice(l, r);
};

const isMarkdownFilePath = (p) => {
  const v = String(p || '').toLowerCase();
  return v.endsWith('.md') || v.endsWith('.markdown');
};

const ensureMarkdownExt = (p) => {
  const s = String(p || '').trim();
  if (!s) return '';
  if (isMarkdownFilePath(s)) return s;
  const lastSlash = Math.max(s.lastIndexOf('/'), s.lastIndexOf('\\'));
  const lastDot = s.lastIndexOf('.');
  const hasExt = lastDot > lastSlash;
  if (hasExt) return s;
  return `${s}.md`;
};

const ensureKnowledgeBasesLoaded = async () => {
  const list = knowledgeBaseStore?.knowledgeBases;
  if (Array.isArray(list) && list.length) return list;
  try {
    const next = await knowledgeBaseStore.fetchKnowledgeBases();
    logger.debug('[LinkOpen] knowledge bases loaded', { count: Array.isArray(next) ? next.length : 0 });
    return next;
  } catch (e) {
    logger.warn('[LinkOpen] fetchKnowledgeBases failed', e);
    return [];
  }
};

const isPathInKnowledgeBase = async (absPath) => {
  const p = normalizeFsPath(absPath);
  if (!p) return false;
  const list = await ensureKnowledgeBasesLoaded();
  for (const kb of Array.isArray(list) ? list : []) {
    const root = normalizeFsPath(kb?.location);
    if (root && (p === root || p.startsWith(`${root}/`))) return true;
  }
  return false;
};

const decodeFileUrlToPath = (href) => {
  const s = String(href || '').trim();
  if (!s.toLowerCase().startsWith('file://')) return '';
  try {
    let rest = s.replace(/^file:\/\//i, '');
    if (rest.startsWith('/')) rest = rest.slice(1);
    return decodeURIComponent(rest);
  } catch {
    return '';
  }
};

// 将 path 规范化（处理 `.`/`..`，并尽量保留 Windows 盘符/UNC 前缀）
const normalizeResolvedFsPath = (rawPath) => {
  let s = normalizeFsPath(rawPath);
  if (!s) return '';

  // 兼容 `/C:/...`
  if (/^\/[a-zA-Z]:\//.test(s)) s = s.slice(1);

  let anchored = false;
  let prefix = '';
  let parts = [];

  if (/^[a-zA-Z]:\//.test(s)) {
    anchored = true;
    prefix = s.slice(0, 3); // `C:/`
    parts = s.slice(3).split('/');
  } else if (s.startsWith('//')) {
    const p = s.slice(2).split('/').filter(Boolean);
    if (p.length >= 2) {
      anchored = true;
      prefix = `//${p[0]}/${p[1]}`;
      parts = p.slice(2);
    } else {
      return s;
    }
  } else if (s.startsWith('/')) {
    anchored = true;
    prefix = '/';
    parts = s.slice(1).split('/');
  } else {
    parts = s.split('/');
  }

  const out = [];
  for (const seg0 of parts) {
    const seg = String(seg0 || '').trim();
    if (!seg || seg === '.') continue;
    if (seg === '..') {
      if (out.length && out[out.length - 1] !== '..') {
        out.pop();
      } else if (!anchored) {
        out.push('..');
      }
      continue;
    }
    out.push(seg);
  }

  if (!out.length) return prefix;
  if (!prefix) return out.join('/');
  return prefix.endsWith('/') ? `${prefix}${out.join('/')}` : `${prefix}/${out.join('/')}`;
};

const resolveFsLinkToAbsolute = async (rawLink, { baseDocPath } = {}) => {
  const raw = stripLinkBoundaryPunct(rawLink);
  if (!raw) return '';

  const fileUrlDecoded = decodeFileUrlToPath(raw);
  const link = normalizeFsPath(fileUrlDecoded || raw);
  if (!link) return '';

  const isAbs = /^[a-zA-Z]:\//.test(link) || link.startsWith('/') || link.startsWith('\\\\');
  if (isAbs) return normalizeResolvedFsPath(link);

  if (link.startsWith('~/')) {
    try {
      const base = await getFilesystemCompletionHomeDir();
      return normalizeResolvedFsPath(joinFsPath(base, link.slice(2)));
    } catch {
      return '';
    }
  }

  let baseDir = '';
  if (baseDocPath) {
    try {
      baseDir = await dirname(baseDocPath);
    } catch (e) {
      logger.warn('[LinkOpen] dirname(baseDocPath) failed, fallback to getFilesystemCompletionDocDir', { baseDocPath }, e);
      baseDir = '';
    }
  }
  if (!baseDir) {
    baseDir = await getFilesystemCompletionDocDir();
  }

  return normalizeResolvedFsPath(joinFsPath(baseDir, link));
};

const findMarkdownAnchorLine = (content, anchorRaw) => {
  const anchor = String(anchorRaw || '').trim();
  if (!anchor) return 1;

  const lines = String(content || '').replace(/\r/g, '').split('\n');

  if (anchor.startsWith('^')) {
    const id = anchor.slice(1);
    if (!id) return 1;
    const re = new RegExp(`\\^${id.replace(/[.*+?^${}()|[\\]\\\\]/g, '\\$&')}\\b`);
    for (let i = 0; i < lines.length; i += 1) {
      if (re.test(lines[i])) return i + 1;
    }
    return 1;
  }

  const norm = (s) => String(s || '').trim().toLowerCase().replace(/\s+/g, ' ');
  const want = norm(anchor.replace(/_/g, ' '));

  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(/^\s{0,3}#{1,6}\s+(.*)$/);
    if (!m) continue;
    const got = norm(m[1].replace(/#+\s*$/, ''));
    if (got === want) return i + 1;
  }

  return 1;
};

const findFootnoteDefinitionLine = (content, numRaw) => {
  const num = String(numRaw || '').trim();
  if (!num) return 0;

  const lines = String(content || '').replace(/\r/g, '').split('\n');
  const esc = num.replace(/[.*+?^${}()|[\\]\\\\]/g, '\\$&');
  const re = new RegExp(`^\\s*\\[\\s*${esc}\\s*\\]\\s*\\(@ref\\)\\s*:`, 'i');

  for (let i = 0; i < lines.length; i += 1) {
    if (re.test(lines[i])) return i + 1;
  }
  return 0;
};

const jumpToFootnoteDefinition = (num, { source = 'preview' } = {}) => {
  const content = editor?.getModel?.()?.getValue?.() ?? rawContent.value ?? '';
  const line = findFootnoteDefinitionLine(content, num);
  if (!line) {
    logger.debug('[Footnote] definition not found', { num, source });
    return false;
  }

  recordPreviewLocation('footnote');
  scrollPreviewToLine(line);
  if (showEditor.value) {
    syncEditorToLine(line, { focus: false });
  }

  logger.info('[Footnote] jumped to definition', { num, line, source });
  return true;
};

// ==================== Markdown 任务列表（Task List）交互 ====================
// 目标：预览模式点击 checklist 时：
// 1) 回写源 Markdown：- [ ] <-> - [x]
// 2) 不触发整篇 Markdown 的重渲染（大文件会卡顿）；只更新被点击条目的 DOM
// 注意：
// - 回写优先使用 Monaco 的 executeEdits 做最小范围替换，保留 undo/redo 以及光标状态。
// - 为避免 watch(rawContent) 触发 renderDebounced() 重渲染，使用一次性“抑制自动渲染”标记。
let suppressPreviewAutoRenderState = { until: 0, reason: '' };

const suppressPreviewAutoRenderOnce = (reason, ttlMs = 900) => {
  const ttl = Math.max(120, Number(ttlMs || 0) || 0);
  suppressPreviewAutoRenderState = {
    until: Date.now() + ttl,
    reason: String(reason || 'unknown')
  };
  logger.debug('[TaskList] suppress preview auto render', { reason: suppressPreviewAutoRenderState.reason, ttlMs: ttl });
};

const consumeSuppressPreviewAutoRender = (from) => {
  const now = Date.now();
  if (!suppressPreviewAutoRenderState?.until) return false;
  if (now > suppressPreviewAutoRenderState.until) {
    suppressPreviewAutoRenderState = { until: 0, reason: '' };
    return false;
  }

  logger.debug('[TaskList] preview auto render suppressed', {
    from: String(from || ''),
    reason: suppressPreviewAutoRenderState.reason,
    remainMs: suppressPreviewAutoRenderState.until - now
  });
  suppressPreviewAutoRenderState = { until: 0, reason: '' };
  return true;
};

const toggleMarkdownTaskAtLine = (lineNumber, checked, { source = 'preview' } = {}) => {
  const ln = Number(lineNumber || 0);
  if (!ln || ln < 1) return false;

  const desired = checked ? 'x' : ' ';
  const taskRe = /^(\s*(?:[-+*]|\d+[.)])\s+)\[( |x|X)\](\s+)/;

  // 预览点击触发的“回写源 Markdown”属于局部更新：抑制一次自动渲染，避免整篇重渲染。
  // 备注：抑制逻辑由 watch(rawContent) 处消费。
  suppressPreviewAutoRenderOnce(`task-toggle:${source}`, 1200);

  const targetEditor = editor;
  const model = targetEditor?.getModel?.();
  const lineText = model ? model.getLineContent(ln) : (String(rawContent.value || '').replace(/\r/g, '').split('\n')[ln - 1] || '');

  const m = taskRe.exec(String(lineText || ''));
  if (!m) {
    logger.debug('[TaskList] no task marker on line', { ln, source, lineText: String(lineText || '').slice(0, 120) });
    return false;
  }

  const current = String(m[2] || ' ');
  const nextChar = desired;
  if (current.toLowerCase() === nextChar) {
    logger.debug('[TaskList] already desired', { ln, checked, source });
    return true;
  }

  // '[' 的 0-based index = m[1].length；括号内字符的位置 = index + 1
  const insideIdx0 = String(m[1] || '').length + 1;

  if (model && targetEditor) {
    try {
      targetEditor.executeEdits('md-task-toggle', [
        {
          range: new monaco.Range(ln, insideIdx0 + 1, ln, insideIdx0 + 2),
          text: nextChar,
          forceMoveMarkers: true
        }
      ]);
      logger.info('[TaskList] toggled', { ln, checked, source });
      return true;
    } catch (e) {
      logger.warn('[TaskList] executeEdits failed, fallback to full replace', { ln, checked, source }, e);
    }
  }

  try {
    const lines = String(rawContent.value || '').replace(/\r/g, '').split('\n');
    if (ln > lines.length) return false;
    const src = String(lines[ln - 1] || '');
    const m2 = taskRe.exec(src);
    if (!m2) return false;

    const head = src.slice(0, m2[1].length + 1);
    const tail = src.slice(m2[1].length + 2);
    lines[ln - 1] = `${head}${nextChar}${tail}`;
    updateEditorContent(lines.join('\n'));

    logger.info('[TaskList] toggled via fallback replace', { ln, checked, source });
    return true;
  } catch (e) {
    logger.error('[TaskList] fallback replace failed', { ln, checked, source }, e);
    return false;
  }
};

const openPathInExplorer = async (absPath) => {
  const p = normalizeFsPath(absPath);
  if (!p) return;
  try {
    await invoke('open_in_file_explorer', { path: p });
  } catch (e) {
    logger.error('[LinkOpen] open_in_file_explorer failed', { path: p }, e);
    message.error('打开文件资源管理器失败');
  }
};

const openMarkdownInEditor = async (absPath, anchor, { silent = false, focus = true } = {}) => {
  const p = normalizeFsPath(absPath);
  if (!p) return false;

  try {
    // 先读取内容确认文件可打开（并避免 openFileInTab 内部失败但不抛错导致无法做 fallback）。
    const res = await fileTreeStore.fetchFileContent(p);
    if (!res?.ok) {
      logger.warn('[LinkOpen] openMarkdownInEditor: fetchFileContent failed', { path: p, anchor, code: res?.code, error: res?.error });
      if (!silent) message.error('打开文件失败');
      return false;
    }

    // 用已读取的内容打开/激活标签页，避免重复读取。
    await fileTreeStore.openFileInTab(p, res.content, { unsupported: !!res.unsupported });
    await nextTick();

    const content = String(res.content || fileTreeStore.selectedFile?.content || '');
    const line = findMarkdownAnchorLine(content, anchor);
    syncEditorToLine(line, { focus });
    return true;
  } catch (e) {
    logger.error('[LinkOpen] openMarkdownInEditor failed', { path: p, anchor }, e);
    if (!silent) message.error('打开文件失败');
    return false;
  }
};

// Obsidian 风格：[[Note]] 这种不带路径的文件名，允许在知识库内按 basename 做一次兜底查找。
// 说明：前端没有轻量 exists API，这里通过 get_file_tree 做“限量 BFS”，只在相对解析失败时触发。
const isBareWikilinkFileName = (filePart) => {
  const s = String(filePart || '').trim();
  if (!s) return false;
  if (s.startsWith('./') || s.startsWith('../') || s.startsWith('~/')) return false;
  if (s.includes('/') || s.includes('\\')) return false;
  if (/^[a-zA-Z]:[\\/]/.test(s)) return false;
  if (s.toLowerCase().startsWith('file://')) return false;
  return true;
};

const findMarkdownByBasenameInRepo = async (filePart, { repoRoot, maxDirs = 300 } = {}) => {
  const root = normalizeFsPath(repoRoot);
  if (!root) return '';

  const wantRaw = String(filePart || '').trim();
  if (!wantRaw) return '';

  const wantWithExt = ensureMarkdownExt(wantRaw);
  const wantNames = new Set([String(wantRaw).toLowerCase(), String(wantWithExt).toLowerCase()]);

  const queue = [root];
  const visited = new Set();
  let walked = 0;

  while (queue.length) {
    const dir0 = queue.shift();
    const dir = normalizeFsPath(dir0);
    if (!dir || visited.has(dir)) continue;
    visited.add(dir);

    walked += 1;
    if (walked > maxDirs) {
      logger.warn('[LinkOpen] wikilink basename search aborted: too many dirs', { repoRoot: root, want: wantRaw, maxDirs });
      return '';
    }

    let children = [];
    try {
      children = await invoke('get_file_tree', { path: dir });
    } catch (e) {
      logger.warn('[LinkOpen] wikilink basename search: get_file_tree failed', { dir }, e);
      continue;
    }

    for (const node of Array.isArray(children) ? children : []) {
      const id = normalizeFsPath(node?.id);
      const name = String(node?.name || '').trim();

      if (node?.is_directory) {
        if (id) queue.push(id);
        continue;
      }

      if (!id || !name) continue;
      if (!/\.(md|markdown)$/i.test(name)) continue;

      if (wantNames.has(name.toLowerCase())) {
        logger.info('[LinkOpen] wikilink basename search hit', { want: wantRaw, path: id, walked });
        return id;
      }
    }
  }

  logger.info('[LinkOpen] wikilink basename search miss', { want: wantRaw, repoRoot: root, walked });
  return '';
};

const openWikilinkFromHref = async (href, { baseDocPath, source } = {}) => {
  const src = String(source || 'unknown');
  const baseDoc = normalizeFsPath(baseDocPath || '');

  try {
    const u = new URL(String(href || '').trim());
    const file = stripLinkBoundaryPunct(u.searchParams.get('file') || '');
    const anchor = stripLinkBoundaryPunct(u.searchParams.get('anchor') || '');

    logger.info('[LinkOpen] open wikilink', { source: src, file, anchor, baseDocPath: baseDoc });

    // [[#heading]]：不带 file 时视为当前文件。
    if (!file) {
      if (baseDoc) return await openMarkdownInEditor(baseDoc, anchor);
      return false;
    }

    // 1) 优先按“当前文档所在目录”解析（Obsidian 默认行为）。
    const primaryAbs = await resolveFsLinkToAbsolute(ensureMarkdownExt(file), { baseDocPath: baseDoc });
    if (await openMarkdownInEditor(primaryAbs, anchor, { silent: true })) return true;

    // 2) 若是纯文件名（不含路径），在知识库内做一次限量 basename 搜索兜底。
    if (isBareWikilinkFileName(file)) {
      let repoRoot = normalizeFsPath(fileTreeStore.activeRepoPath || '');
      if (!repoRoot && baseDoc) {
        const list = await ensureKnowledgeBasesLoaded();
        for (const kb of Array.isArray(list) ? list : []) {
          const root = normalizeFsPath(kb?.location);
          if (root && (baseDoc === root || baseDoc.startsWith(`${root}/`))) {
            repoRoot = root;
            break;
          }
        }
      }

      const hit = await findMarkdownByBasenameInRepo(file, { repoRoot });
      if (hit && (await openMarkdownInEditor(hit, anchor, { silent: true }))) return true;
    }

    // 3) 最终失败：给用户一次明确反馈。
    logger.warn('[LinkOpen] wikilink target not found', { source: src, file, anchor, baseDocPath: baseDoc, primaryAbs });
    message.error('未找到目标文件');
    return false;
  } catch (err) {
    logger.warn('[LinkOpen] parse/open wikilink failed', { href, source: src, baseDocPath: baseDoc }, err);
    return false;
  }
};

const openFsLinkSmart = async (rawLink, { anchor, viaCtrl = false, baseDocPath } = {}) => {
  const abs = await resolveFsLinkToAbsolute(rawLink, { baseDocPath });
  if (!abs) return;

  const isMd = isMarkdownFilePath(abs) || isMarkdownFilePath(ensureMarkdownExt(abs));
  const mdPath = isMd ? ensureMarkdownExt(abs) : abs;

  if (isMd && (await isPathInKnowledgeBase(mdPath))) {
    await openMarkdownInEditor(mdPath, anchor);
    return;
  }

  if (viaCtrl) {
    await openPathInExplorer(abs);
  }
};

// Monaco：编辑模式 Ctrl+Click 打开 URL/路径
const installMonacoCtrlClickOpen = (targetEditor, { tag = 'left' } = {}) => {
  if (!targetEditor) return null;

  const disposable = targetEditor.onMouseDown((ev) => {
    const ctrl = !!(ev?.event?.ctrlKey || ev?.event?.metaKey);
    if (!ctrl) return;

    const pos = ev?.target?.position;
    const model = targetEditor.getModel?.();
    if (!pos || !model) return;

    const lineContent = model.getLineContent(pos.lineNumber);
    const token0 = extractClickableTokenAtLineColumn(lineContent, pos.column);
    const token = stripLinkBoundaryPunct(token0);
    if (!token) return;

    const lower = String(token || '').trim().toLowerCase();
    const isWikilink = lower.startsWith('guaxi://wikilink');
    const isUrl = isHttpUrl(token);
    const isPath = !isUrl && !isWikilink && isPathLikeToken(token);
    if (!isUrl && !isPath && !isWikilink) return;

    try {
      ev.event?.preventDefault?.();
      ev.event?.stopPropagation?.();
      ev.event?.browserEvent?.preventDefault?.();
      ev.event?.browserEvent?.stopPropagation?.();
    } catch {
      // ignore
    }

    const baseDocPath = fileTreeStore.activeTab || selectedFile.value?.path || '';
    logger.info('[LinkOpen] monaco ctrl+click', {
      tag,
      token,
      baseDocPath,
      line: pos.lineNumber,
      column: pos.column
    });

    (async () => {
      if (isWikilink) {
        await openWikilinkFromHref(token, { baseDocPath, source: `monaco:${tag}` });
        return;
      }

      if (isUrl) {
        try {
          await openUrl(token);
        } catch (err) {
          logger.error('[LinkOpen] openUrl failed', { href: token }, err);
          message.error('打开浏览器失败');
        }
        return;
      }

      const { link, anchor } = splitFsLinkAndAnchor(token);
      if (!link) return;
      await openFsLinkSmart(link, { anchor, viaCtrl: true, baseDocPath });
    })();
  });

  logger.debug('[LinkOpen] monaco ctrl+click handler installed', { tag });
  return disposable;
};
const installMonacoLinkLikeDecorations = (targetEditor, { tag = 'left' } = {}) => {
  if (!targetEditor) return null;

  const classLinkLike = 'guaxi-link-like';
  const classMdBracket = 'guaxi-mdlink-bracket';
  const classMdAngle = 'guaxi-mdlink-angle';
  const classMdText = 'guaxi-mdlink-text';
  const classMdParen = 'guaxi-mdlink-paren';
  const classMdTarget = 'guaxi-mdlink-target';
  const classMdRefTarget = 'guaxi-mdref-target';
  const classWikiBracket = 'guaxi-wikilink-bracket';
  const classWikiText = 'guaxi-wikilink-text';

  let decorationIds = [];
  let timerId = null;

  const wikiRe = /\[\[[^\]]+?\]\]/g;
  const mdInlineLinkRe = /\[([^\]\n]+?)\]\(\s*([^\s)]+)(?:\s+"[^"]*")?\s*\)/g;
  const mdRefLinkRe = /\[([^\]\n]+?)\]\s*\[([^\]\n]*?)\]/g;
  const mdAutoLinkUrlRe = /<((?:https?|ftp):\/\/[^>\s]+)>/g;
  const mdAutoLinkEmailRe = /<((?:mailto:)?[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*)>/g;
  const mdFootnoteRefRe = /\[(\d+)\]\s*\(@ref\)/g;
  const urlRe = /(?:https?:\/\/|file:\/\/)[0-9A-Za-z\-._~:\/?#@!$&'()*+,;=%\\]+/g;
  const pathRe = /(?:\.\.\/|\.\/|~\/|[a-zA-Z]:[\\/]|\\\\)[0-9A-Za-z\-._~:\/?#@!$&'()*+,;=%\\]+/g;

  const isFence = (text) => /^\s*(```+|~~~+)/.test(String(text || ''));

  const update = (reason) => {
    const model = targetEditor.getModel?.();
    if (!model || model.getLanguageId?.() !== 'markdown') {
      if (decorationIds.length) decorationIds = targetEditor.deltaDecorations(decorationIds, []);
      return;
    }

    const ranges = targetEditor.getVisibleRanges?.() || [];
    if (!ranges.length) return;

    let startLine = Math.max(1, Math.min(...ranges.map((r) => r.startLineNumber)) - 80);
    let endLine = Math.min(model.getLineCount(), Math.max(...ranges.map((r) => r.endLineNumber)) + 80);

    const scanBack = 500;
    const scanStart = Math.max(1, startLine - scanBack);

    let inFence = false;
    let fenceToken = '';

    for (let ln = scanStart; ln < startLine; ln += 1) {
      const t = model.getLineContent(ln);
      const m = /^\s*(```+|~~~+)/.exec(t);
      if (!m) continue;
      const token = m[1];
      if (!inFence) {
        inFence = true;
        fenceToken = token;
      } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
        inFence = false;
        fenceToken = '';
      }
    }

    const decs = [];

    for (let ln = startLine; ln <= endLine; ln += 1) {
      const line = model.getLineContent(ln);
      const fenceHit = /^\s*(```+|~~~+)/.exec(line);
      if (fenceHit) {
        const token = fenceHit[1];
        if (!inFence) {
          inFence = true;
          fenceToken = token;
        } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
          inFence = false;
          fenceToken = '';
        }
        continue;
      }
      if (inFence) continue;

      wikiRe.lastIndex = 0;
      for (const m of line.matchAll(wikiRe)) {
        const raw = String(m[0] || '');
        if (!raw) continue;
        const start = Number(m.index || 0);
        const end = start + raw.length;
        if (raw.length < 4) continue;

        // Obsidian 双链：[[...]]
        // 目标：括号用次要色，内部文本用链接色，避免 "[[" 左侧第二个括号颜色割裂。
        decs.push({
          range: new monaco.Range(ln, start + 1, ln, start + 2 + 1),
          options: {
            inlineClassName: classWikiBracket,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
        decs.push({
          range: new monaco.Range(ln, end - 2 + 1, ln, end + 1),
          options: {
            inlineClassName: classWikiBracket,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
        if (end - start > 4) {
          decs.push({
            range: new monaco.Range(ln, start + 2 + 1, ln, end - 2 + 1),
            options: {
              inlineClassName: classWikiText,
              stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
            }
          });
        }
      }

      // Markdown 内联链接：[text](target "title")
      // 目标：括号符号易区分（#f35f4c），链接文本用独立色，URL/路径目标用链接色且仅目标下划线。
      mdInlineLinkRe.lastIndex = 0;
      for (const m of line.matchAll(mdInlineLinkRe)) {
        const rawText = String(m[1] || '');
        const rawTarget = String(m[2] || '');
        if (!rawTarget) continue;

        const token = stripLinkBoundaryPunct(rawTarget);
        if (!token) continue;
        if (!isPathLikeToken(token) && !isHttpUrl(token)) continue;

        const whole = String(m[0] || '');
        const startWhole = Number(m.index || 0);

        const closeBracketRel = whole.indexOf(']');
        if (closeBracketRel < 1) continue;

        const openParenRel = whole.indexOf('(', closeBracketRel);
        const closeParenRel = whole.lastIndexOf(')');
        if (openParenRel < 0 || closeParenRel <= openParenRel) continue;

        // '[' text ']'
        decs.push({ range: new monaco.Range(ln, startWhole + 1, ln, startWhole + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (rawText) decs.push({ range: new monaco.Range(ln, startWhole + 2, ln, startWhole + closeBracketRel + 1), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + closeBracketRel + 1, ln, startWhole + closeBracketRel + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        // '(' target ')'（括号不下划线）
        decs.push({ range: new monaco.Range(ln, startWhole + openParenRel + 1, ln, startWhole + openParenRel + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        const targetRel = whole.indexOf(rawTarget, openParenRel + 1);
        if (targetRel >= 0) decs.push({ range: new monaco.Range(ln, startWhole + targetRel + 1, ln, startWhole + targetRel + rawTarget.length + 1), options: { inlineClassName: classMdTarget, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + closeParenRel + 1, ln, startWhole + closeParenRel + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // Markdown 引用链接：[text][id] / [text][]
      mdRefLinkRe.lastIndex = 0;
      for (const m of line.matchAll(mdRefLinkRe)) {
        const rawText = String(m[1] || '');
        const rawId = String(m[2] || '');
        const whole = String(m[0] || '');
        const startWhole = Number(m.index || 0);

        const firstClose = whole.indexOf(']');
        const secondOpen = whole.indexOf('[', firstClose + 1);
        const secondClose = whole.lastIndexOf(']');
        if (firstClose < 1 || secondOpen < 0 || secondClose <= secondOpen) continue;

        // '[' text ']'
        decs.push({ range: new monaco.Range(ln, startWhole + 1, ln, startWhole + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (rawText) decs.push({ range: new monaco.Range(ln, startWhole + 2, ln, startWhole + firstClose + 1), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + firstClose + 1, ln, startWhole + firstClose + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        // '[' id ']'
        decs.push({ range: new monaco.Range(ln, startWhole + secondOpen + 1, ln, startWhole + secondOpen + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (rawId) decs.push({ range: new monaco.Range(ln, startWhole + secondOpen + 2, ln, startWhole + secondClose + 1), options: { inlineClassName: classMdRefTarget, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, startWhole + secondClose + 1, ln, startWhole + secondClose + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // Markdown 自动链接：<https://...> / <contact@example.com>
      const decorateAngle = (start, innerLen) => {
        decs.push({ range: new monaco.Range(ln, start + 1, ln, start + 2), options: { inlineClassName: classMdAngle, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, start + innerLen + 2, ln, start + innerLen + 3), options: { inlineClassName: classMdAngle, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      };

      mdAutoLinkUrlRe.lastIndex = 0;
      for (const m of line.matchAll(mdAutoLinkUrlRe)) {
        const inner = String(m[1] || '');
        if (!inner) continue;
        const start = Number(m.index || 0);
        decorateAngle(start, inner.length);
        decs.push({ range: new monaco.Range(ln, start + 2, ln, start + 2 + inner.length), options: { inlineClassName: classLinkLike, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      mdAutoLinkEmailRe.lastIndex = 0;
      for (const m of line.matchAll(mdAutoLinkEmailRe)) {
        const inner = String(m[1] || '');
        if (!inner) continue;
        const start = Number(m.index || 0);
        decorateAngle(start, inner.length);
        decs.push({ range: new monaco.Range(ln, start + 2, ln, start + 2 + inner.length), options: { inlineClassName: classLinkLike, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // 脚注引用（当前工程语法）：[1](@ref)
      mdFootnoteRefRe.lastIndex = 0;
      for (const m of line.matchAll(mdFootnoteRefRe)) {
        const num = String(m[1] || '');
        const start = Number(m.index || 0);
        // '[' num ']'
        decs.push({ range: new monaco.Range(ln, start + 1, ln, start + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (num) decs.push({ range: new monaco.Range(ln, start + 2, ln, start + 2 + num.length), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        decs.push({ range: new monaco.Range(ln, start + 2 + num.length, ln, start + 3 + num.length), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        // '(@ref)'
        const open = line.indexOf('(', start);
        const close = line.indexOf(')', start);
        if (open >= 0) decs.push({ range: new monaco.Range(ln, open + 1, ln, open + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (close > open + 1) decs.push({ range: new monaco.Range(ln, open + 2, ln, close + 1), options: { inlineClassName: classMdRefTarget, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (close >= 0) decs.push({ range: new monaco.Range(ln, close + 1, ln, close + 2), options: { inlineClassName: classMdParen, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
      }

      // 引用链接定义：[id]: url "title"（仅 URL/路径下划线）
      const def = /^\s*\[([^\]]+?)\]\s*:\s*(.*)$/.exec(line);
      if (def) {
        const id = String(def[1] || '');
        const after = String(def[2] || '');
        const openIdx = line.indexOf('[');
        const closeIdx = line.indexOf(']', openIdx + 1);
        const colonIdx = line.indexOf(':', closeIdx + 1);

        if (openIdx >= 0) decs.push({ range: new monaco.Range(ln, openIdx + 1, ln, openIdx + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (id && closeIdx > openIdx + 1) decs.push({ range: new monaco.Range(ln, openIdx + 2, ln, closeIdx + 1), options: { inlineClassName: classMdText, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        if (closeIdx >= 0) decs.push({ range: new monaco.Range(ln, closeIdx + 1, ln, closeIdx + 2), options: { inlineClassName: classMdBracket, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });

        const base = colonIdx >= 0 ? colonIdx + 1 : closeIdx + 1;
        const raw = after.trimStart();
        const pad = after.length - raw.length;
        const rawOffset = base + pad;

        let tok = '';
        let tokStart = 0;
        let tokEnd = 0;
        if (raw.startsWith('`')) {
          const endTick = raw.indexOf('`', 1);
          if (endTick > 1) {
            tok = raw.slice(1, endTick);
            tokStart = 1;
            tokEnd = endTick;
          }
        } else if (raw.startsWith('<')) {
          const gt = raw.indexOf('>', 1);
          if (gt > 1) {
            tok = raw.slice(1, gt);
            tokStart = 1;
            tokEnd = gt;
          }
        } else {
          const m0 = /^\S+/.exec(raw);
          if (m0) {
            tok = m0[0];
            tokStart = 0;
            tokEnd = tok.length;
          }
        }

        const cleaned = stripLinkBoundaryPunct(tok);
        if (cleaned && (isHttpUrl(cleaned) || isPathLikeToken(cleaned))) {
          decs.push({ range: new monaco.Range(ln, rawOffset + tokStart + 1, ln, rawOffset + tokEnd + 1), options: { inlineClassName: classLinkLike, stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges } });
        }
      }

      urlRe.lastIndex = 0;
      for (const m of line.matchAll(urlRe)) {
        const raw = String(m[0] || '');
        const token = stripLinkBoundaryPunct(raw);
        if (!token || !isHttpUrl(token)) continue;
        const start = Number(m.index || 0);
        const end = start + raw.length;
        decs.push({
          range: new monaco.Range(ln, start + 1, ln, end + 1),
          options: {
            inlineClassName: classLinkLike,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
      }

      pathRe.lastIndex = 0;
      for (const m of line.matchAll(pathRe)) {
        const raw = String(m[0] || '');
        const token = stripLinkBoundaryPunct(raw);
        if (!token || !isPathLikeToken(token)) continue;
        const start = Number(m.index || 0);
        const end = start + raw.length;
        decs.push({
          range: new monaco.Range(ln, start + 1, ln, end + 1),
          options: {
            inlineClassName: classLinkLike,
            stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
          }
        });
      }
    }

    decorationIds = targetEditor.deltaDecorations(decorationIds, decs);
    logger.debug('[LinkDecor] updated', { tag, reason, startLine, endLine, count: decs.length });
  };

  const schedule = (reason) => {
    if (timerId) window.clearTimeout(timerId);
    timerId = window.setTimeout(() => update(reason), 120);
  };

  const d1 = targetEditor.onDidScrollChange(() => schedule('scroll'));
  const d2 = targetEditor.onDidChangeModel(() => schedule('model'));
  const d3 = targetEditor.onDidChangeModelContent(() => schedule('content'));

  schedule('init');

  return {
    dispose() {
      try {
        d1?.dispose?.();
        d2?.dispose?.();
        d3?.dispose?.();
      } catch {
        // ignore
      }
      if (timerId) {
        window.clearTimeout(timerId);
        timerId = null;
      }
      if (decorationIds.length) {
        decorationIds = targetEditor.deltaDecorations(decorationIds, []);
      }
    }
  };
};

// Vditor realtime：点击链接时拦截为“系统浏览器/资源管理器”，避免内部跳转
const installRealtimeVditorLinkInterceptor = () => {
  const root = document.getElementById(realtimeVditorId);
  if (!root) {
    logger.warn('[LinkOpen] realtime vditor root not found, skip interceptor');
    return;
  }

  if (typeof realtimeVditorLinkInterceptorCleanup === 'function') {
    realtimeVditorLinkInterceptorCleanup();
    realtimeVditorLinkInterceptorCleanup = null;
  }

  const handler = (e) => {
    const base = e?.target instanceof Element ? e.target : e?.target?.parentElement;
    if (!base) return;

    const a = base.closest?.('a');
    if (!a || !root.contains(a)) return;

    const href0 = a.getAttribute('href') || '';
    const href = String(href0 || '').trim();
    if (!href) return;

    if (href.toLowerCase().startsWith('guaxi://wikilink')) {
      e.preventDefault();
      e.stopPropagation();

      const baseDocPath = fileTreeStore.activeTab || selectedFile.value?.path || '';
      logger.info('[LinkOpen] realtime wikilink', { href, baseDocPath });

      void (async () => {
        await openWikilinkFromHref(href, { baseDocPath, source: 'realtime' });
      })();
      return;
    }

    if (isHttpUrl(href)) {
      e.preventDefault();
      e.stopPropagation();

      logger.info('[LinkOpen] realtime openUrl', { href });
      void (async () => {
        try {
          await openUrl(href);
        } catch (err) {
          logger.error('[LinkOpen] openUrl failed', { href }, err);
          message.error('打开浏览器失败');
        }
      })();
      return;
    }

    if (!isPathLikeToken(href)) return;

    e.preventDefault();
    e.stopPropagation();

    const { link, anchor } = splitFsLinkAndAnchor(href);
    if (!link) return;

    const baseDocPath = fileTreeStore.activeTab || selectedFile.value?.path || '';
    logger.info('[LinkOpen] realtime openFsLink', { link, anchor, baseDocPath });
    void (async () => {
      await openFsLinkSmart(link, { anchor, viaCtrl: true, baseDocPath });
    })();
  };

  root.addEventListener('click', handler, true);
  realtimeVditorLinkInterceptorCleanup = () => {
    root.removeEventListener('click', handler, true);
  };

  logger.debug('[LinkOpen] realtime vditor link interceptor installed');
};

const preprocessObsidianWikilinks = (content) => {
  const text = String(content || '');
  if (!text.includes('[[')) return text;

  const lines = text.replace(/\r/g, '').split('\n');
  let inFence = false;
  let fenceToken = '';

  const out = lines.map((line) => {
    const fence = line.match(/^\s*(```+|~~~+)/);
    if (fence) {
      const token = fence[1];
      if (!inFence) {
        inFence = true;
        fenceToken = token;
      } else if (token.startsWith(fenceToken)) {
        inFence = false;
        fenceToken = '';
      }
      return line;
    }

    if (inFence || !line.includes('[[')) return line;

    return line.replace(/\[\[([^\]]+?)\]\]/g, (_m, innerRaw) => {
      const inner = String(innerRaw || '').trim();
      if (!inner) return _m;

      const parts = inner.split('|');
      const target = String(parts[0] || '').trim();
      const alias = String(parts[1] || '').trim();

      let filePart = target;
      let anchorPart = '';
      const hashIdx = target.indexOf('#');
      if (hashIdx >= 0) {
        filePart = target.slice(0, hashIdx).trim();
        anchorPart = target.slice(hashIdx + 1).trim();
      }

      const display = alias || anchorPart || filePart || target;
      const safeText = display.replace(/([\[\]\\])/g, '\\$1');

      const href = `guaxi://wikilink?file=${encodeURIComponent(filePart)}&anchor=${encodeURIComponent(anchorPart)}`;
      return `[${safeText}](${href})`;
    });
  });

  return out.join('\n');
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
          logger.debug('[PathCompletion] no match', {
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

        logger.debug('[PathCompletion] suggest', {
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

  const clampInt = (value, min, max) => {
    const num = Number.parseInt(String(value || ''), 10);
    if (!Number.isFinite(num)) return null;
    if (num < min) return min;
    if (num > max) return max;
    return num;
  };

  const buildMarkdownTableSnippet = (rows, cols) => {
    const safeRows = clampInt(rows, 1, 30);
    const safeCols = clampInt(cols, 1, 30);
    if (!safeRows || !safeCols) return '';

    const headerCells = Array.from(
      { length: safeCols },
      (_, i) => ` 列${i + 1} `
    ).join('|');
    const sepCells = Array.from({ length: safeCols }, () => ' --- ').join('|');

    let tabStop = 1;
    const bodyLines = [];
    for (let r = 0; r < safeRows; r += 1) {
      const rowCells = [];
      for (let c = 0; c < safeCols; c += 1) {
        const label = `值${r + 1}-${c + 1}`;
        rowCells.push(' ${' + String(tabStop) + ':' + label + '} ');
        tabStop += 1;
      }
      bodyLines.push(`|${rowCells.join('|')}|`);
    }

    return `|${headerCells}|\n|${sepCells}|\n${bodyLines.join('\n')}\n`;
  };

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
    async provideCompletionItems(model, position) {
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

      const fenceLspSuggestions = await provideMarkdownFenceLspSuggestions(model, position);
      if (fenceLspSuggestions.length) {
        return { suggestions: fenceLspSuggestions };
      }

      const wordInfo = model.getWordUntilPosition(position);
      const word = wordInfo?.word || '';
      if (!word) return { suggestions: [] };

      const lower = word.toLowerCase();
      const tableMatch = /^table(\d{1,2})(?:x|_)(\d{1,2})$/.exec(lower);
      if (tableMatch && wordInfo?.startColumn && wordInfo?.endColumn) {
        const rows = clampInt(tableMatch[1], 1, 30);
        const cols = clampInt(tableMatch[2], 1, 30);
        const insertText = buildMarkdownTableSnippet(rows, cols);

        if (insertText) {
          logger.debug('[MarkdownSnippet] table param', { word, rows, cols });

          const range = new monaco.Range(
            position.lineNumber,
            wordInfo.startColumn,
            position.lineNumber,
            wordInfo.endColumn
          );

          return {
            suggestions: [
              {
                label: `table${rows}x${cols}`,
                kind: monaco.languages.CompletionItemKind.Snippet,
                insertText,
                insertTextRules:
                  monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                detail: `插入 ${rows}×${cols} 表格`,
                range,
                sortText: '0_table_param'
              }
            ]
          };
        }

        logger.warn('[MarkdownSnippet] invalid table param', { word, rows, cols });
        return { suggestions: [] };
      }

      const range = new monaco.Range(
        position.lineNumber,
        wordInfo.startColumn,
        position.lineNumber,
        wordInfo.endColumn
      );

      const suggestions = markdownSnippets
        .filter((s) => String(s.label || '').toLowerCase().startsWith(lower))
        .slice(0, 50)
        .map((s, index) => ({
          ...s,
          range,
          sortText: s.sortText || `1_${String(index).padStart(3, '0')}_${String(s.label || '')}`
        }));

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
const lspEnabled = ref(false)
const markdownFenceCompletionModels = new Map()

// Markdown 围栏高亮与补全的共享逻辑
const getMarkdownFenceContext = (model, position) => {
  if (!model || !position) return null;
  if (model.getLanguageId?.() !== 'markdown') return null;

  let inFence = false;
  let fenceToken = '';
  let fenceLang = '';
  let fenceStartLine = 0;

  for (let ln = 1; ln <= position.lineNumber; ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})(.*)$/.exec(line);
    if (!hit) continue;

    const token = hit[1];
    if (!inFence) {
      inFence = true;
      fenceToken = token;
      fenceStartLine = ln;
      fenceLang = String(hit[2] || '').trim();
    } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
      inFence = false;
      fenceToken = '';
      fenceLang = '';
      fenceStartLine = 0;
    }
  }

  if (!inFence || position.lineNumber <= fenceStartLine) return null;
  const languageId = resolveFenceLanguageId(fenceLang);
  if (!languageId) return null;

  let fenceEndLine = model.getLineCount() + 1;
  for (let ln = position.lineNumber + 1; ln <= model.getLineCount(); ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})/.exec(line);
    if (hit && hit[1][0] === fenceToken[0] && hit[1].length >= fenceToken.length) {
      fenceEndLine = ln;
      break;
    }
  }

  const blockStartLine = fenceStartLine + 1;
  const blockEndLine = Math.max(blockStartLine, fenceEndLine - 1);
  const blockLines = [];
  for (let ln = blockStartLine; ln <= blockEndLine; ln += 1) {
    blockLines.push(model.getLineContent(ln));
  }

  return {
    languageId,
    blockText: blockLines.join('\n'),
    positionInBlock: new monaco.Position(
      Math.max(1, position.lineNumber - blockStartLine + 1),
      Math.max(1, position.column)
    ),
    blockStartLine,
    blockEndLine
  };
};

const getMarkdownFenceBlocks = (model) => {
  if (!model) return [];
  const blocks = [];
  let inFence = false;
  let fenceToken = '';
  let fenceLang = '';
  let fenceStartLine = 0;

  for (let ln = 1; ln <= model.getLineCount(); ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})(.*)$/.exec(line);
    if (!hit) continue;

    const token = hit[1];
    if (!inFence) {
      inFence = true;
      fenceToken = token;
      fenceStartLine = ln;
      fenceLang = String(hit[2] || '').trim();
      continue;
    }

    if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
      const languageId = resolveFenceLanguageId(fenceLang);
      if (languageId) {
        const blockStartLine = fenceStartLine + 1;
        const blockEndLine = Math.max(blockStartLine, ln - 1);
        blocks.push({ languageId, blockStartLine, blockEndLine });
      }
      inFence = false;
      fenceToken = '';
      fenceLang = '';
      fenceStartLine = 0;
    }
  }

  return blocks;
};

const buildMarkdownFenceBlockText = (model, startLine, endLine) => {
  const lines = [];
  const safeStart = Math.max(1, startLine || 1);
  const safeEnd = Math.max(safeStart, endLine || safeStart);
  for (let ln = safeStart; ln <= safeEnd; ln += 1) {
    lines.push(model.getLineContent(ln));
  }
  return lines.join('\n');
};

const markdownFenceHighlightStates = new WeakMap();

const getMarkdownFenceHighlightState = (targetEditor) => {
  let state = markdownFenceHighlightStates.get(targetEditor);
  if (!state) {
    state = { decorations: [], timer: 0, lastKey: '' };
    markdownFenceHighlightStates.set(targetEditor, state);
  }
  return state;
};

const clearMarkdownFenceSyntaxHighlight = (targetEditor) => {
  if (!targetEditor) return;
  const state = getMarkdownFenceHighlightState(targetEditor);
  if (!state.decorations?.length) return;
  state.decorations = targetEditor.deltaDecorations(state.decorations, []);
};

const resolveFenceTokenClass = (tokenType) => {
  const type = String(tokenType || '').toLowerCase();
  if (!type) return '';
  if (type.includes('comment')) return 'md-fence-token-comment';
  if (type.includes('string')) return 'md-fence-token-string';
  if (type.includes('regexp')) return 'md-fence-token-regexp';
  if (type.includes('keyword') || type.includes('storage')) return 'md-fence-token-keyword';
  if (type.includes('number') || type.includes('constant.numeric')) return 'md-fence-token-number';
  if (type.includes('boolean')) return 'md-fence-token-boolean';
  if (type.includes('type') || type.includes('interface') || type.includes('enum') || type.includes('struct')) return 'md-fence-token-type';
  if (type.includes('class')) return 'md-fence-token-class';
  if (type.includes('method')) return 'md-fence-token-method';
  if (type.includes('function')) return 'md-fence-token-function';
  if (type.includes('parameter') || type.includes('argument')) return 'md-fence-token-parameter';
  if (type.includes('property') || type.includes('field')) return 'md-fence-token-property';
  if (type.includes('namespace') || type.includes('module') || type.includes('package')) return 'md-fence-token-namespace';
  if (type.includes('tag')) return 'md-fence-token-tag';
  if (type.includes('attribute')) return 'md-fence-token-attribute';
  if (type.includes('builtin') || type.includes('support')) return 'md-fence-token-builtin';
  if (type.includes('decorator') || type.includes('annotation')) return 'md-fence-token-decorator';
  if (type.includes('variable') || type.includes('identifier')) return 'md-fence-token-variable';
  if (type.includes('constant')) return 'md-fence-token-constant';
  if (type.includes('operator')) return 'md-fence-token-operator';
  if (type.includes('delimiter') || type.includes('punctuation') || type.includes('bracket')) return 'md-fence-token-delimiter';
  return '';
};

const updateMarkdownFenceSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const model = targetEditor.getModel?.();
  if (!model || model.getLanguageId?.() !== 'markdown') {
    clearMarkdownFenceSyntaxHighlight(targetEditor);
    return;
  }

  if (lspEnabled.value) {
    clearMarkdownFenceSyntaxHighlight(targetEditor);
    return;
  }

  const position = targetEditor.getPosition?.();
  const ctx = getMarkdownFenceContext(model, position);
  const visibleRanges = typeof targetEditor.getVisibleRanges === 'function'
    ? targetEditor.getVisibleRanges()
    : [];
  let blocks = [];
  if (ctx) {
    blocks = [ctx];
  } else {
    const allBlocks = getMarkdownFenceBlocks(model);
    if (visibleRanges.length) {
      blocks = allBlocks.filter((item) => visibleRanges.some((range) => (
        item.blockEndLine >= range.startLineNumber && item.blockStartLine <= range.endLineNumber
      )));
    } else {
      blocks = allBlocks;
    }
  }
  if (!blocks.length) {
    clearMarkdownFenceSyntaxHighlight(targetEditor);
    return;
  }

  const state = getMarkdownFenceHighlightState(targetEditor);
  const versionId = typeof model.getVersionId === 'function'
    ? model.getVersionId()
    : model.getAlternativeVersionId?.() || 0;
  const rangeKey = visibleRanges
    .map((range) => `${range.startLineNumber}-${range.endLineNumber}`)
    .join('|');
  const key = ctx
    ? `${ctx.languageId}:${ctx.blockStartLine}:${ctx.blockEndLine}:${versionId}`
    : `${versionId}:${rangeKey}`;
  if (state.lastKey === key && reason === 'cursor') return;
  state.lastKey = key;

  const decorations = [];
  for (const block of blocks) {
    const blockText = block.blockText ?? buildMarkdownFenceBlockText(model, block.blockStartLine, block.blockEndLine);
    let tokenLines = [];
    try {
      tokenLines = monaco.editor.tokenize(blockText || '', block.languageId) || [];
    } catch (error) {
      logger.warn('[MarkdownFence] tokenize failed', { language: block.languageId }, error);
      continue;
    }

    for (let i = 0; i < tokenLines.length; i += 1) {
      const tokens = tokenLines[i] || [];
      if (!tokens.length) continue;
      const lineNumber = block.blockStartLine + i;
      const lineContent = model.getLineContent(lineNumber);
      const lineLen = lineContent.length;

      decorations.push({
        range: new monaco.Range(lineNumber, 1, lineNumber, Math.max(1, lineLen + 1)),
        options: { inlineClassName: 'md-fence-token-base' }
      });

      for (let j = 0; j < tokens.length; j += 1) {
        const token = tokens[j];
        const className = resolveFenceTokenClass(token?.type || '');
        if (!className) continue;

        const startOffset = token?.offset || 0;
        const nextOffset = tokens[j + 1] ? tokens[j + 1].offset : lineLen;
        const startColumn = Math.min(Math.max(1, startOffset + 1), lineLen + 1);
        const endColumn = Math.min(Math.max(startColumn, nextOffset + 1), lineLen + 1);
        if (startColumn === endColumn) continue;

        decorations.push({
          range: new monaco.Range(lineNumber, startColumn, lineNumber, endColumn),
          options: { inlineClassName: className }
        });
      }
    }
  }

  state.decorations = targetEditor.deltaDecorations(state.decorations || [], decorations);
};

const scheduleMarkdownFenceSyntaxHighlight = (targetEditor, reason) => {
  if (!targetEditor) return;
  const state = getMarkdownFenceHighlightState(targetEditor);
  if (state.timer) {
    window.clearTimeout(state.timer);
  }
  state.timer = window.setTimeout(() => {
    state.timer = 0;
    updateMarkdownFenceSyntaxHighlight(targetEditor, reason);
  }, 60);
};

const getOrCreateMarkdownFenceModel = (languageId) => {
  const key = String(languageId || '').trim().toLowerCase();
  if (!key) return null;

  let model = markdownFenceCompletionModels.get(key);
  if (model && !model.isDisposed?.()) {
    if (model.getLanguageId?.() !== key) {
      monaco.editor.setModelLanguage(model, key);
    }
    return model;
  }

  const uri = monaco.Uri.parse(`inmemory://guaxi/markdown-fence-${key}.tmp`);
  model = monaco.editor.getModel(uri) || monaco.editor.createModel('', key, uri);
  markdownFenceCompletionModels.set(key, model);
  return model;
};

const provideMarkdownFenceLspSuggestions = async (model, position) => {
  const ctx = getMarkdownFenceContext(model, position);
  if (!ctx) return [];
  if (!LSP_SUPPORTED_LANGUAGES.has(ctx.languageId)) return [];
  if (!lspEnabled.value) return [];
  if (!editor) return [];

  const lspState = getLspConnectionState(ctx.languageId);
  if (lspState?.state === 'disabled') return [];

  const filePath = String(selectedFile.value?.path || '').trim();
  initLspClient(editor, ctx.languageId, filePath);

  const virtualModel = getOrCreateMarkdownFenceModel(ctx.languageId);
  if (!virtualModel) return [];
  virtualModel.setValue(ctx.blockText || '');

  const providers = (monaco.languages).CompletionProviderRegistry?.all?.(virtualModel) || [];
  if (!providers.length) return [];

  const wordInfo = model.getWordUntilPosition(position);
  const replaceRange = new monaco.Range(
    position.lineNumber,
    wordInfo?.startColumn || position.column,
    position.lineNumber,
    wordInfo?.endColumn || position.column
  );

  const fallbackToken = {
    isCancellationRequested: false,
    onCancellationRequested: () => ({ dispose() { } })
  };

  const suggestions = [];
  const seen = new Set();
  for (const provider of providers) {
    if (!provider?.provideCompletionItems) continue;
    try {
      const result = await provider.provideCompletionItems(
        virtualModel,
        ctx.positionInBlock,
        { triggerKind: monaco.languages.CompletionTriggerKind.Invoke },
        fallbackToken
      );
      const items = Array.isArray(result?.suggestions) ? result.suggestions : [];
      for (const item of items) {
        const label = typeof item?.label === 'string'
          ? item.label
          : String(item?.label?.label || '');
        const insertText = String(item?.insertText || item?.textEdit?.text || '');
        const dedupeKey = `${label}::${insertText}`;
        if (!label || seen.has(dedupeKey)) continue;
        seen.add(dedupeKey);

        suggestions.push({
          ...item,
          insertText,
          range: replaceRange,
          textEdit: undefined,
          additionalTextEdits: undefined,
        });
      }
    } catch {
    }
  }

  return suggestions;
};

const EXT_TO_LANGUAGE = {
  js: 'javascript',
  ts: 'typescript',
  css: 'css',
  html: 'html',
  htm: 'html',
  md: 'markdown',
  py: 'python',
  cpp: 'cpp',
  c: 'cpp',
  h: 'cpp',
  hpp: 'cpp',
  rs: 'rust',
  sh: 'shell',
  jsx: 'javascript',
  tsx: 'javascript',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  lua: 'lua',
  java: 'java',
  tex: 'latex',
  ltx: 'latex',
  latex: 'latex',
  cls: 'latex',
  sty: 'latex',
  toml: 'ini',
}

const MARKDOWN_FENCE_LANG_MAP = {
  js: 'javascript',
  javascript: 'javascript',
  jsx: 'javascript',
  ts: 'typescript',
  tsx: 'typescript',
  py: 'python',
  python: 'python',
  c: 'c',
  cpp: 'cpp',
  cxx: 'cpp',
  cc: 'cpp',
  h: 'cpp',
  hpp: 'cpp',
  rs: 'rust',
  rust: 'rust',
  sh: 'shell',
  shell: 'shell',
  bash: 'shell',
  zsh: 'shell',
  html: 'html',
  css: 'css',
  json: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  lua: 'lua',
  java: 'java',
  tex: 'latex',
  latex: 'latex',
}

const LSP_SUPPORTED_LANGUAGES = new Set([
  'cpp',
  'c',
  'rust',
  'python',
  'shell',
  'lua',
  'java',
  'typescript',
  'javascript',
  'html',
  'markdown',
  'latex',
])

const resolveMonacoLanguageByPath = (path) => {
  const ext = String(path || '').split('.').pop()?.toLowerCase() || 'markdown'
  return EXT_TO_LANGUAGE[ext] || 'markdown'
}

const resolveFenceLanguageId = (infoRaw) => {
  const info = String(infoRaw || '').trim().toLowerCase()
  if (!info) return ''
  const langToken = info.split(/\s+/)[0] || ''
  const mapped = MARKDOWN_FENCE_LANG_MAP[langToken]
  if (mapped) return mapped
  const languages = monaco.languages?.getLanguages?.() || []
  const exists = languages.some((item) => item?.id === langToken)
  return exists ? langToken : ''
}

const refreshLspEnabledSetting = async () => {
  try {
    const enabled = await invoke('get_lsp_syntax_highlight_enabled')
    lspEnabled.value = !!enabled
    applyMonacoThemeByLsp(lspEnabled.value)
  } catch (error) {
    logger.error('加载 LSP 配置失败:', error)
    lspEnabled.value = false
    applyMonacoThemeByLsp(false)
  }
}

// LSP 开关驱动 Monaco 主题，确保高亮使用 One Dark
const applyMonacoThemeByLsp = (enabled) => {
  const isDark = settingsStore.appearanceTheme === 'dark'
  const themeName = enabled ? 'guaxi-dark' : (isDark ? 'guaxi-dark' : 'guaxi-light')
  try {
    monaco.editor.setTheme(themeName)
    logger.info('[theme] apply by lsp', { enabled, themeName })
  } catch (error) {
    logger.warn('[theme] apply by lsp failed', { enabled, themeName, error })
  }
}

const enableLspForEditorLanguage = (targetEditor, languageId, filePath = '') => {
  if (!targetEditor) return
  const lang = String(languageId || '').trim().toLowerCase()
  if (!lang) return

  initTextmate(targetEditor, lang).catch((e) => {
    logger.warn('[Editor] initTextmate failed, ignored', { language: lang }, e)
  })

  if (!lspEnabled.value) return
  if (!LSP_SUPPORTED_LANGUAGES.has(lang)) return
  initLspClient(targetEditor, lang, filePath || '')
}

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
    logger.debug('[Editor] language pairing configured', { languageId: id })
  } catch (e) {
    logger.warn('[Editor] failed to configure language pairing', { languageId: id, error: e })
  }
}

// 初始化编辑器
const initEditor = () => {
  logger.debug('[Editor] initEditor called', {
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
    logger.debug('[Editor] editor container not ready');
    return;
  }

  // 获取文件扩展名并确定语言
  const language = resolveMonacoLanguageByPath(selectedFile.value?.path || '');

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
      { token: 'delimiter', foreground: '383a42' },           // Delimiter → Main text color
      { token: 'variable', foreground: 'e45649' },           // Variable
      { token: 'parameter', foreground: '4078f2' },           // Parameter → Function color
      { token: 'function', foreground: '4078f2' },           // Function
      { token: 'constant', foreground: '986801' },           // Constant → Number color
      { token: 'class', foreground: 'c18401' },           // Class → Type color
      { token: 'identifier', foreground: '383a42' },           // Identifier → Main text color
      { token: 'tag', foreground: 'e45649' },           // Tag → Variable color
      { token: 'attribute.name', foreground: 'c18401' },           // Attribute name → Type color
      { token: 'meta.embedded', foreground: 'a0a1a7' }            // Meta embedded → Comment color
    ],
    colors: {
      'editor.background': '#fafafa',
      'editor.foreground': '#383a42',
      'editorLineNumber.foreground': '#a0a1a7',
      'editorLineNumber.activeForeground': '#383a42',
      'editorCursor.foreground': '#4078f2',
      'editor.selectionBackground': '#e5e5e6',
      'editor.inactiveSelectionBackground': '#e5e5e6',
      'editor.lineHighlightBackground': '#f0f0f0',
      'editorBracketMatch.background': '#d7dae0',
      'editorBracketMatch.border': '#c5c8ce',
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
      'editor.selectionHighlightBackground': '#d7dae0',
      'editor.foldBackground': '#f0f0f0',
      'editorWhitespace.foreground': '#d0d0d0',
      'editorIndentGuide.background': '#d0d0d0',
      'editorIndentGuide.activeBackground': '#c5c8ce',
      'editorHoverWidget.background': '#fafafa',
      'editorHoverWidget.border': '#dcdfe4',
      'editorSuggestWidget.background': '#ffffff',
      'editorSuggestWidget.border': '#dcdfe4',
      'editorSuggestWidget.foreground': '#383a42',
      'editorSuggestWidget.selectedBackground': '#e5e5e6',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#dcdfe4',
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
      { token: 'meta.embedded', foreground: '5c6370' },           // Meta embedded → Comment color
      { token: 'operator', foreground: '56b6c2' },
      { token: 'punctuation', foreground: 'abb2bf' },
      { token: 'punctuation.definition', foreground: 'abb2bf' },
      { token: 'punctuation.bracket', foreground: 'abb2bf' },
      { token: 'property', foreground: '61afef' },
      { token: 'method', foreground: '61afef' },
      { token: 'function.declaration', foreground: '61afef' },
      { token: 'function.defaultLibrary', foreground: '61afef' },
      { token: 'parameter.declaration', foreground: '61afef' },
      { token: 'variable.readonly', foreground: 'd19a66' },
      { token: 'variable.defaultLibrary', foreground: 'e06c75' },
      { token: 'type.defaultLibrary', foreground: 'e5c07b' },
      { token: 'class.defaultLibrary', foreground: 'e5c07b' },
      { token: 'interface', foreground: 'e5c07b' },
      { token: 'enum', foreground: 'e5c07b' },
      { token: 'enumMember', foreground: 'd19a66' },
      { token: 'namespace', foreground: 'e5c07b' },
      { token: 'typeParameter', foreground: 'e5c07b' },
      { token: 'struct', foreground: 'e5c07b' },
      { token: 'macro', foreground: 'c678dd' },
      { token: 'decorator', foreground: 'c678dd' },
      { token: 'label', foreground: 'c678dd' },
      { token: 'keyword.control', foreground: 'c678dd' },
      { token: 'keyword.defaultLibrary', foreground: 'c678dd' },
      { token: 'string.escape', foreground: '56b6c2' },
      { token: 'number.float', foreground: 'd19a66' },
      { token: 'boolean', foreground: 'd19a66' }
    ],
    semanticHighlighting: true,
    semanticTokenColors: {
      comment: '#5c6370',
      string: '#98c379',
      keyword: '#c678dd',
      number: '#d19a66',
      regexp: '#56b6c2',
      operator: '#abb2bf',
      punctuation: '#abb2bf',
      'punctuation.definition': '#abb2bf',
      'punctuation.bracket': '#abb2bf',
      class: '#e5c07b',
      enum: '#e5c07b',
      interface: '#e5c07b',
      type: '#e5c07b',
      typeParameter: '#e5c07b',
      namespace: '#e5c07b',
      struct: '#e5c07b',
      function: '#61afef',
      method: '#61afef',
      property: '#61afef',
      parameter: '#61afef',
      'function.declaration': '#61afef',
      'function.defaultLibrary': '#61afef',
      'parameter.declaration': '#61afef',
      variable: '#e06c75',
      'variable.readonly': '#d19a66',
      'variable.defaultLibrary': '#e06c75',
      enumMember: '#d19a66',
      boolean: '#d19a66',
      macro: '#c678dd',
      decorator: '#c678dd',
      label: '#c678dd',
      'string.escape': '#56b6c2'
    },
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
      'editorBracketMatch.border': '#61afef',
      'editorBracketHighlight.foreground1': '#e06c75',
      'editorBracketHighlight.foreground2': '#d19a66',
      'editorBracketHighlight.foreground3': '#e5c07b',
      'editorBracketHighlight.foreground4': '#98c379',
      'editorBracketHighlight.foreground5': '#56b6c2',
      'editorBracketHighlight.foreground6': '#c678dd',
      'editorBracketHighlight.unexpectedBracket.foreground': '#e06c75',
      'editorBracketPairGuide.background1': '#e06c7540',
      'editorBracketPairGuide.background2': '#d19a6640',
      'editorBracketPairGuide.background3': '#e5c07b40',
      'editorBracketPairGuide.background4': '#98c37940',
      'editorBracketPairGuide.background5': '#56b6c240',
      'editorBracketPairGuide.background6': '#c678dd40',
      'editorBracketPairGuide.activeBackground1': '#e06c7580',
      'editorBracketPairGuide.activeBackground2': '#d19a6680',
      'editorBracketPairGuide.activeBackground3': '#e5c07b80',
      'editorBracketPairGuide.activeBackground4': '#98c37980',
      'editorBracketPairGuide.activeBackground5': '#56b6c280',
      'editorBracketPairGuide.activeBackground6': '#c678dd80',
      'editor.selectionHighlightBackground': '#3e4451',
      'editor.foldBackground': '#21252b',
      'editorWhitespace.foreground': '#4b5263',
      'editorIndentGuide.background': '#4b5263',
      'editorIndentGuide.activeBackground': '#5c6370',
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
    renderLineHighlight: 'line',  // 高亮当前行
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
    find: {
      addExtraSpaceOnTop: false
    },
    mouseWheelZoom: true  // 启用鼠标滚轮缩放
  });

  void refreshLspEnabledSetting().then(() => {
    enableLspForEditorLanguage(editor, language, selectedFile.value?.path || '')
  })

  // 注册编辑器实例到store
  editorStore.setEditorInstance(editor);
  editorStore.setLanguage(language);
  logger.debug('[Editor] Monaco editor instance registered')

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

    editor.addAction({
      id: 'guaxi.table.moveCellLeft',
      label: '表格：移动到左侧单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyH],
      run: () => moveTableCell(-1, 0)
    });

    editor.addAction({
      id: 'guaxi.table.moveCellDown',
      label: '表格：移动到下方单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyJ],
      run: () => moveTableCell(0, 1)
    });

    editor.addAction({
      id: 'guaxi.table.moveCellUp',
      label: '表格：移动到上方单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyK],
      run: () => moveTableCell(0, -1)
    });

    editor.addAction({
      id: 'guaxi.table.moveCellRight',
      label: '表格：移动到右侧单元格',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyMod.Alt | monaco.KeyCode.KeyL],
      run: () => moveTableCell(1, 0)
    });

    editor.addAction({
      id: 'guaxi.file.find',
      label: '文件内搜索',
      keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyF],
      run: () => runEditorAction('actions.find')
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

  const language = resolveMonacoLanguageByPath(rightPath || '');

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

  enableLspForEditorLanguage(secondaryEditor, language, rightPath || '');
  scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'init');

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
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'cursor');
  });

  secondaryEditor.onDidScrollChange(() => {
    editorEffectsRight?.updateCursorTarget();
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'scroll');
  });

  secondaryEditor.onDidType((text) => {
    editorEffectsRight?.spawnExplosionAtCursor(text);
    editorEffectsRight?.updateCursorTarget();
  });

  secondaryEditor.onDidChangeModelLanguage(() => {
    const languageId = secondaryEditor?.getModel?.()?.getLanguageId?.() || 'plaintext';
    enableLspForEditorLanguage(secondaryEditor, languageId, rightPath || '');
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'model');
  });

  if (secondaryMonacoCtrlClickDisposable) {
    secondaryMonacoCtrlClickDisposable.dispose();
    secondaryMonacoCtrlClickDisposable = null;
  }
  secondaryMonacoCtrlClickDisposable = installMonacoCtrlClickOpen(secondaryEditor, { tag: 'right' });

  if (secondaryMonacoLinkLikeDecorDisposable) {
    secondaryMonacoLinkLikeDecorDisposable.dispose();
    secondaryMonacoLinkLikeDecorDisposable = null;
  }
  secondaryMonacoLinkLikeDecorDisposable = installMonacoLinkLikeDecorations(secondaryEditor, { tag: 'right' });

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
        if (model.getLanguageId() === 'markdown') {
          scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'debounced');
        }
      }, 120)
    );
  }
};

const disposeSecondaryEditor = () => {
  if (editorEffectsRight) {
    editorEffectsRight.destroy();
    editorEffectsRight = null;
  }

  if (secondaryMonacoCtrlClickDisposable) {
    secondaryMonacoCtrlClickDisposable.dispose();
    secondaryMonacoCtrlClickDisposable = null;
  }

  if (secondaryMonacoLinkLikeDecorDisposable) {
    secondaryMonacoLinkLikeDecorDisposable.dispose();
    secondaryMonacoLinkLikeDecorDisposable = null;
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
    logger.debug('[Git] initGitService', { filePath, dir })
    gitService.value = new GitService(dir)
    await updateGitStatus()
    return
  }

  logger.debug('[Git] initGitService skipped (no selected file)')
}

// 更新 Git 状态
const updateGitStatus = async () => {
  const filePath = selectedFile.value?.path
  const workspacePath = gitService.value?.workspacePath

  if (!gitService.value || !filePath) {
    logger.debug('[Git] updateGitStatus skipped', {
      hasGitService: !!gitService.value,
      filePath
    })
    return
  }

  try {
    logger.debug('[Git] updateGitStatus start', { filePath, workspacePath })

    const branch = await gitService.value.getCurrentBranch()
    editorStore.setGitBranch(branch)

    const status = await gitService.value.getFileStatus(filePath)
    const counts = {
      added: status.filter(s => s.status === 'A').length,
      modified: status.filter(s => s.status === 'M').length,
      deleted: status.filter(s => s.status === 'D').length
    }

    logger.debug('[Git] fileStatus', {
      filePath,
      statusCount: Array.isArray(status) ? status.length : null,
      counts,
      sample: Array.isArray(status) ? status.slice(0, 3) : status
    })

    editorStore.setGitChanges(counts)

    await updateGitDecorations()
    logger.debug('[Git] updateGitStatus done', {
      gitDecorationsCount: editorStore.gitDecorations?.length || 0,
      gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
    })
  } catch (error) {
    logger.error('[Git] updateGitStatus failed:', error)
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
    logger.debug('[Git] updateGitDecorations skipped', {
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
    logger.debug('[Git] updateGitDecorations start', {
      filePath,
      hasEditorStoreInstance: !!editorStore.editorInstance,
      preferUnsaved
    })

    if (preferUnsaved) {
      const changes = computeUnsavedLineChanges()
      apply(changes)
      logger.debug('[Git] updateGitDecorations done (unsaved)', {
        gitDecorationsCount: editorStore.gitDecorations?.length || 0,
        gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
      })
      return
    }

    const changes = await gitService.value.getLineChanges(filePath)
    apply(changes)

    logger.debug('[Git] updateGitDecorations done', {
      gitDecorationsCount: editorStore.gitDecorations?.length || 0,
      gitDecorationIdsCount: editorStore.gitDecorationIds?.length || 0
    })
  } catch (error) {
    logger.error('[Git] updateGitDecorations failed:', error)
    apply(null)
  }
}

// 切换 Git 面板
const toggleGitPanel = async () => {
  const windowLabel = `git-${Date.now()}` // 唯一窗口标识
  const windowTitle = `${fileTreeStore.activeRepoPath || '未命名文件'}` // 使用不带后缀的文件名

  logger.info('打开 Git 窗口:', windowLabel, windowTitle)
  logger.info('workspace_path:', fileTreeStore.activeRepoPath)
  try {
    await invoke('create_git_window', {
      label: windowLabel,
      title: windowTitle,
      workspacePath: fileTreeStore.activeRepoPath,
      theme: settingsStore.appearanceTheme,
    })
  } catch (error) {
    logger.error('创建窗口失败:', error)
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

  if (monacoCtrlClickDisposable) {
    monacoCtrlClickDisposable.dispose();
    monacoCtrlClickDisposable = null;
  }
  monacoCtrlClickDisposable = installMonacoCtrlClickOpen(editor, { tag: 'left' });

  if (monacoLinkLikeDecorDisposable) {
    monacoLinkLikeDecorDisposable.dispose();
    monacoLinkLikeDecorDisposable = null;
  }
  monacoLinkLikeDecorDisposable = installMonacoLinkLikeDecorations(editor, { tag: 'left' });

  scheduleMarkdownFenceSyntaxHighlight(editor, 'init');

  editor.onDidChangeCursorPosition((e) => {
    editorStore.setCursorPosition(e.position);
    updateTableToolbar();
    scheduleMarkdownFenceSyntaxHighlight(editor, 'cursor');
    editorEffectsLeft?.updateCursorTarget();
  });

  const model = editor.getModel();
  if (model) {
    model.onDidChangeContent((ev) => {
      if (!editor) return;
      if (tableInsertAutoFormatBusy) return;
      if (tableAutoFormatBusy) return;
      if (Date.now() - lastTableToolbarInsertAt < 450) return;
      if (model.getLanguageId() !== 'markdown') return;
      const selection = editor.getSelection();
      if (!selection) return;
      if (!ev?.changes?.length) return;

      const tableChange = ev.changes.find((c) => {
        if (!c?.text) return false;
        if (c.range?.startColumn !== 1) return false;
        const t = String(c.text).replace(/\r/g, '');
        if (!t.includes('\n')) return false;
        if (!/^\s*\|/.test(t)) return false;
        return /\n\|\s*-{3,}/.test(t);
      });

      if (!tableChange) return;

      tableInsertAutoFormatBusy = true;

      window.setTimeout(() => {
        if (!editor) {
          tableInsertAutoFormatBusy = false;
          return;
        }

        const ctx = detectTableContext();
        if (!ctx) {
          tableInsertAutoFormatBusy = false;
          return;
        }

        const targetLine = Math.min(model.getLineCount(), ctx.startLine + 2);
        editor.setPosition(new monaco.Position(targetLine, 3));
        updateTableToolbar();
        formatTable();

        nextTick(() => {
          tableInsertAutoFormatBusy = false;
        });
      }, 0);
    });

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
        scheduleMarkdownFenceSyntaxHighlight(editor, 'debounced');
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
    scheduleMarkdownFenceSyntaxHighlight(editor, 'scroll');
    syncRealtimePreviewScroll();
  });

  editor.onDidChangeModelLanguage(() => {
    const language = editor.getModel()?.getLanguageId() || 'plaintext';
    editorStore.setLanguage(language);
    enableLspForEditorLanguage(editor, language, selectedFile.value?.path || '');
    updateTableToolbar();
    scheduleMarkdownFenceSyntaxHighlight(editor, 'model');
  });

  const isSuggestWidgetVisible = () => {
    const dom = editor?.getDomNode?.();
    if (!dom) return false;
    return !!dom.querySelector('.suggest-widget.visible');
  };

  let lastSnipasteSuggestAt = 0;

  const forceTriggerSuggest = (source, meta) => {
    if (!editor) return;

    const now = Date.now();
    if (now - lastSnipasteSuggestAt < 120) {
      logger.debug('[KeywordSuggest] triggerSuggest skipped (too frequent)', {
        ...meta,
        dt: now - lastSnipasteSuggestAt
      });
      return;
    }
    lastSnipasteSuggestAt = now;

    const visibleBefore = isSuggestWidgetVisible();

    try {
      editor.trigger(source, 'hideSuggestWidget', null);
    } catch {
    }

    logger.debug('[KeywordSuggest] triggerSuggest (force)', {
      ...meta,
      visibleBefore
    });

    editor.trigger(source, 'editor.action.triggerSuggest', {});

    window.setTimeout(() => {
      if (!editor) return;
      const visibleAfter = isSuggestWidgetVisible();
      logger.debug('[KeywordSuggest] triggerSuggest check', { ...meta, visibleAfter });
      if (visibleAfter) return;
      editor.trigger(source, 'editor.action.triggerSuggest', {});
    }, 80);
  };

  editor.onDidType((text) => {
    if (!editor) return;
    const ch = text ? text[text.length - 1] : '';
    if (!ch) return;

    editorEffectsLeft?.spawnExplosionAtCursor(text);
    editorEffectsLeft?.updateCursorTarget();

    const model = editor.getModel();
    const pos = editor.getPosition();
    if (!model || !pos) return;

    const word = model.getWordUntilPosition(pos)?.word || '';
    const lowerWord = String(word || '').trim().toLowerCase();
    if (lowerWord === 'snipaste') {
      forceTriggerSuggest('guaxi-keyword', { word: lowerWord, line: pos.lineNumber, col: pos.column });
    }

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

    logger.debug('[PathCompletion] triggerSuggest', {
      ch,
      token,
      pathContext
    });
    editor.trigger('guaxi-path', 'editor.action.triggerSuggest', {});
  });

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
      hidePreviewFloatingToolbar()
      return;
    }

    const isEmpty = e.selection.isEmpty();
    if (isEmpty || !selectedText.trim()) {
      floatingToolbarVisible.value = false;
      return;
    }

    if (e.source !== 'mouse') {
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
    const clampedLeft = Math.min(Math.max(8, left), Math.max(8, wrapperRect.width - 360));

    floatingToolbarStyle.value = {
      left: `${clampedLeft}px`,
      top: `${clampedTop}px`
    };
    selectionSurface.value = 'editor'
    hidePreviewFloatingToolbar()
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
  let inFence = false;
  let fenceToken = '';

  lines.forEach((line, index) => {
    const trimmed = line.trim();

    // 1) fenced code block（```/~~~）：记录行号用于“未闭合”错误，同时在代码块内部跳过链接语法检查。
    const fence = /^(```+|~~~+)/.exec(trimmed);
    if (fence) {
      const token = fence[1];
      fenceLines.push(index + 1);

      if (!inFence) {
        inFence = true;
        fenceToken = token;
      } else if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
        inFence = false;
        fenceToken = '';
      }
      return;
    }

    if (inFence) return;

    // 2) 任务列表 `- [ ] xxx` / `- [x] xxx`：方括号是合法语法，不参与“不完整链接”检查。
    if (/^\s*(?:[-*+]|\d+\.)\s+\[(?: |x|X)\]\s+/.test(line)) {
      return;
    }

    // 2.5) 目录标记：[TOC] / [toc] / [目录] / [[toc]]
    if (/^\s*(?:\[\[\s*(?:toc|\u76ee\u5f55)\s*\]\]|\[\s*(?:toc|\u76ee\u5f55)\s*\])\s*$/i.test(trimmed)) {
      return;
    }

    // 3) 剔除 Obsidian 双链与行内代码，降低误报。
    const lintLine = line
      .replace(/\[\[[\s\S]*?\]\]/g, '')
      .replace(/`[^`]*`/g, '')
      .replace(/\\[\[\]\(\)]/g, '');

    const openBracket = (lintLine.match(/\[/g) || []).length;
    const linkStart = (lintLine.match(/\]\s*\(/g) || []).length;
    const closeParen = (lintLine.match(/\)/g) || []).length;
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
    logger.error('Markdown 格式化失败:', e);
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
    logger.error('Markdown 格式化失败:', e);
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

const isWordSelection = (raw) => {
  const text = String(raw || '').trim()
  if (!text || /\s/.test(text)) return false
  return /^[A-Za-z][A-Za-z'-]{0,47}$/.test(text)
}

const hidePreviewFloatingToolbar = () => {
  previewFloatingToolbarVisible.value = false
}

const clearPreviewSelectionState = () => {
  previewSelectionText.value = ''
  hidePreviewFloatingToolbar()
}

const updatePreviewSelectionFromWindow = () => {
  const selection = window.getSelection?.()
  const container = previewContainer.value
  if (!selection || !container || selection.rangeCount === 0) return ''
  const text = String(selection.toString() || '')
  if (!text.trim()) return ''
  const range = selection.getRangeAt(0)
  const target = range.commonAncestorContainer
  const el = target instanceof Element ? target : target?.parentElement
  if (!el || !container.contains(el)) return ''
  return text
}

const getSelectedTextForTranslate = () => {
  if (selectionSurface.value === 'preview') {
    const text = normalizeSelectionTextForLanguageTasks(previewSelectionText.value || updatePreviewSelectionFromWindow())
    if (text) return text
  }

  if (editor) {
    const model = editor.getModel();
    const selection = editor.getSelection();
    if (model && selection && !selection.isEmpty()) {
      const text = normalizeSelectionTextForLanguageTasks(model.getValueInRange(selection) || '')
      if (text) {
        selectionSurface.value = 'editor'
        return text
      }
    }
  }

  const previewText = normalizeSelectionTextForLanguageTasks(previewSelectionText.value || updatePreviewSelectionFromWindow())
  if (previewText) {
    selectionSurface.value = 'preview'
    return previewText
  }

  return '';
};

const handlePreviewSelectionMouseUp = (e) => {
  if (!showPreview.value || previewMode.value === 'realtime') return
  const container = previewContainer.value
  const shell = previewShellRef.value
  if (!container || !shell) return

  const text = normalizeSelectionTextForLanguageTasks(updatePreviewSelectionFromWindow())
  if (!text) {
    clearPreviewSelectionState()
    return
  }

  previewSelectionText.value = text
  selectionSurface.value = 'preview'

  const shellRect = shell.getBoundingClientRect()
  const x = Number(e?.clientX || shellRect.left)
  const y = Number(e?.clientY || shellRect.top)
  const toolbarW = 232
  const toolbarH = 34
  const pad = 8
  const gap = 10

  let left = x - shellRect.left
  let top = y - shellRect.top - toolbarH - gap
  if (top < pad) {
    top = y - shellRect.top + gap
  }
  const maxLeft = Math.max(pad, shellRect.width - toolbarW - pad)
  const maxTop = Math.max(pad, shellRect.height - toolbarH - pad)

  previewFloatingToolbarStyle.value = {
    left: `${Math.min(Math.max(pad, left), maxLeft)}px`,
    top: `${Math.min(Math.max(pad, top), maxTop)}px`,
  }
  previewFloatingToolbarVisible.value = true
}

const handleGlobalSelectionChange = () => {
  if (selectionSurface.value !== 'preview') return
  const text = normalizeSelectionTextForLanguageTasks(updatePreviewSelectionFromWindow())
  if (!text) {
    clearPreviewSelectionState()
  } else {
    previewSelectionText.value = text
  }
}

const handleGlobalMouseDownForPreviewToolbar = (event) => {
  if (!previewFloatingToolbarVisible.value) return
  const target = event?.target instanceof Element ? event.target : null
  if (!target) return
  if (target.closest('.floating-md-toolbar')) return
  if (target.closest('.pot-selection-translate-panel')) return
  if (target.closest('.preview')) return
  clearPreviewSelectionState()
}

const handleSelectionTranslatePanelUpdatePos = (pos) => {
  if (!pos) return;
  const left = Number(pos.left);
  const top = Number(pos.top);
  if (!Number.isFinite(left) || !Number.isFinite(top)) return;

  const cur = selectionTranslatePanelPos.value
  if (cur && Number(cur.left) === left && Number(cur.top) === top) return

  selectionTranslatePanelPos.value = { left, top };
};

const toggleSelectionTranslatePinned = () => {
  selectionTranslatePinned.value = !selectionTranslatePinned.value;
  logger.info('[SelectionTranslate] pin:', { pinned: selectionTranslatePinned.value });
};

const swapSelectionTranslateLangs = () => {
  const tmp = selectionTranslateFrom.value
  selectionTranslateFrom.value = selectionTranslateTo.value
  selectionTranslateTo.value = tmp
  logger.info('[SelectionTranslate] swap langs', { from: selectionTranslateFrom.value, to: selectionTranslateTo.value })

  if (selectionTranslatePanelVisible.value && !selectionTranslateBusy.value) {
    translateSelectionText(selectionTranslateRaw.value)
  }
}

const closeSelectionTranslatePanel = () => {
  selectionTranslatePanelVisible.value = false;
  selectionTranslatePinned.value = false;
  selectionTranslateWordMeta.value = null
};

const openSelectionTranslatePanel = async () => {
  const text = normalizeSelectionTextForLanguageTasks(getSelectedTextForTranslate());
  if (!text) {
    message.warning('请先框选要翻译的文本');
    return;
  }

  const usePreviewAnchor = selectionSurface.value === 'preview' && previewFloatingToolbarVisible.value
  const anchorStyle = usePreviewAnchor ? previewFloatingToolbarStyle.value : floatingToolbarStyle.value
  const left = Number.parseFloat(String(anchorStyle?.left || ''));
  const top = Number.parseFloat(String(anchorStyle?.top || ''));

  const wrapperEl = usePreviewAnchor ? previewShellRef.value : editorWrapperRef.value
  const pw = wrapperEl?.clientWidth ?? window.innerWidth
  const ph = wrapperEl?.clientHeight ?? window.innerHeight

  const pad = 8
  const panelW = 360
  const panelH = 260
  const maxLeft = Math.max(pad, pw - panelW - pad)
  const maxTop = Math.max(pad, ph - panelH - pad)

  const nextLeft = Number.isFinite(left) ? left : pad
  const nextTop = Number.isFinite(top) ? top + 44 : 52

  selectionTranslatePanelPos.value = {
    left: Math.min(Math.max(pad, nextLeft), maxLeft),
    top: Math.min(Math.max(pad, nextTop), maxTop)
  };
  selectionTranslatePanelVisible.value = true;
  if (usePreviewAnchor) {
    hidePreviewFloatingToolbar()
  }

  await translateSelectionText(text);
};

const sendSelectionToAssistant = (overrideText, options = {}) => {
  const text = String(overrideText ?? getSelectedTextForTranslate()).trim()
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
  return true
}

const askSelectionWithAssistant = () => {
  sendSelectionToAssistant(undefined, { autoSend: true, successMessage: '已发送到助手并开始提问' })
}

const getTranslateLangLabel = (value) => {
  const item = assistantTranslateLangOptions.find((x) => x.value === value)
  return item ? String(item.label || '') : '目标语言'
}

const openAssistantTranslateDialog = () => {
  const text = normalizeSelectionTextForLanguageTasks(getSelectedTextForTranslate())
  if (!text) {
    message.warning('请先框选要翻译的文本')
    return
  }
  assistantTranslateSourceText.value = text
  assistantTranslateDialogVisible.value = true
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

const translateSelectionText = async (overrideText) => {
  const text = normalizeSelectionTextForLanguageTasks(overrideText ?? getSelectedTextForTranslate());
  if (!text) {
    message.warning('请先框选要翻译的文本');
    return;
  }

  if (selectionTranslateBusy.value) return;

  selectionTranslateRaw.value = text;
  selectionTranslateResult.value = '';
  selectionTranslateError.value = '';
  selectionTranslateItems.value = [];
  selectionTranslateWordMeta.value = null
  selectionTranslateBusy.value = true;

  logger.info('[SelectionTranslate] start', { len: text.length, from: selectionTranslateFrom.value, to: selectionTranslateTo.value });

  try {
    if (selectionTranslateMultiAvailable.value) {
      try {
        const response = await invoke('selection_translate_free_multi', {
          text,
          from: selectionTranslateFrom.value,
          to: selectionTranslateTo.value
        });

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
          logger.info('[SelectionTranslate] multi success', { engines: selectionTranslateItems.value.length })
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
            logger.warn('[SelectionTranslate] multi command not found, disabled multi path. Please restart tauri backend.')
          }
        } else {
          logger.warn('[SelectionTranslate] multi failed, fallback to single:', err)
        }
      }
    }

    const response = await invoke('translate_text_free', {
      texts: [text],
      from: selectionTranslateFrom.value,
      to: selectionTranslateTo.value
    });
    const translated = (response?.translated_texts?.[0] || '').toString();

    if (!translated.trim() || translated.includes('[Translation Failed]')) {
      const err = String(response?.error || '翻译结果为空');
      throw new Error(err);
    }

    if (!selectionTranslateResult.value) {
      selectionTranslateItems.value = [{ engine: 'Free', success: true, text: translated, error: '' }]
      selectionTranslateResult.value = translated;
      logger.info('[SelectionTranslate] single success', { len: translated.length });
    }

    const finalTranslated = String(selectionTranslateResult.value || '').trim()
    if (isWordSelection(text)) {
      try {
        const enriched = await invoke('language_learning_enrich_word', {
          word: text,
          sourceLang: selectionTranslateFrom.value,
          targetLang: selectionTranslateTo.value
        })
        selectionTranslateWordMeta.value = {
          word: String(enriched?.word || text).trim(),
          phonetic: String(enriched?.phonetic || '').trim(),
          part_of_speech: String(enriched?.part_of_speech || '').trim(),
          meanings: Array.isArray(enriched?.meanings) ? enriched.meanings : [],
          synonyms: Array.isArray(enriched?.synonyms) ? enriched.synonyms : [],
          antonyms: Array.isArray(enriched?.antonyms) ? enriched.antonyms : [],
          examples: Array.isArray(enriched?.examples) ? enriched.examples : []
        }
      } catch (err) {
        logger.warn('[SelectionTranslate] enrich word failed:', err)
      }
    }

    if (finalTranslated) {
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
        logger.warn('[SelectionTranslate] record save failed:', err)
      }
    }
  } catch (err) {
    const msg = String(err?.message || err || '翻译失败');
    selectionTranslateError.value = msg;
    selectionTranslateItems.value = [{ engine: 'Free', success: false, text: '', error: msg }]
    logger.error('[SelectionTranslate] failed:', err);
    message.error(`翻译失败: ${msg}`);
  } finally {
    selectionTranslateBusy.value = false;
  }
};

const copySelectionTranslateResult = async (payload) => {
  const t = typeof payload === 'string' ? payload : payload?.text
  const text = String(t || selectionTranslateResult.value || '')
  if (!text) return;

  const ok = await writeClipboardText(text);
  if (ok) {
    message.success('已复制译文', { duration: 1200 });
  } else {
    message.warning('复制失败：无剪贴板权限');
  }
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
    logger.debug('[MarkdownList] enter', {
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
    logger.debug('[MarkdownTable] auto-format', {
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

  const leftBase = editorRect.left - wrapperRect.left;
  const topBase = editorRect.top - wrapperRect.top;

  const cursorPos = editor.getPosition();
  const anchorPos = cursorPos
    ? new monaco.Position(cursorPos.lineNumber, cursorPos.column)
    : new monaco.Position(ctx.startLine, 1);

  const visiblePos = editor.getScrolledVisiblePosition(anchorPos);
  if (!visiblePos) {
    tableToolbarVisible.value = false;
    activeTableContext.value = null;
    return;
  }

  const headerEl = editorHeaderRef.value;
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0;
  const minTop = headerHeight + 8;

  const toolbarH = 36;
  const toolbarW = 320;
  const gap = 8;

  const preferTop = topBase + visiblePos.top - toolbarH - gap;
  const fallbackTop = topBase + visiblePos.top + visiblePos.height + gap;
  const top = preferTop >= minTop ? preferTop : fallbackTop;
  const clampedTop = Math.min(
    Math.max(minTop, top),
    Math.max(minTop, wrapperRect.height - toolbarH - 8)
  );

  const layout = editor.getLayoutInfo ? editor.getLayoutInfo() : null;
  const contentLeft = layout && Number.isFinite(layout.contentLeft) ? layout.contentLeft : 0;
  const minLeft = Math.max(8, leftBase + contentLeft);

  const left = leftBase + visiblePos.left;
  const clampedLeft = Math.min(
    Math.max(minLeft, left),
    Math.max(minLeft, wrapperRect.width - toolbarW - 8)
  );

  tableToolbarStyle.value = {
    left: `${clampedLeft}px`,
    top: `${clampedTop}px`
  };

  if (isTableAutoFormatDebug()) {
    logger.debug('[MarkdownTable] toolbar', {
      left: clampedLeft,
      top: clampedTop,
      contentLeft,
      anchor: { lineNumber: anchorPos.lineNumber, column: anchorPos.column },
    });
  }

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

const moveTableCell = (dx, dy) => {
  if (!editor) return;

  const model = editor.getModel();
  const pos = editor.getPosition();
  const selection = editor.getSelection();
  if (!model || !pos || !selection) return;
  if (model.getLanguageId() !== 'markdown') return;
  if (!selection.isEmpty()) return;

  const ctx = detectTableContext();
  if (!ctx) return;

  const maxRow = Math.max(0, ctx.rows.length - 1);
  const nextCol = Math.max(0, Math.min(ctx.colCount - 1, ctx.activeColIndex + (Number(dx) || 0)));

  let nextRow = Math.max(0, Math.min(maxRow, ctx.activeRowIndex + (Number(dy) || 0)));
  if (nextRow === 1 && dy < 0) nextRow = 0;
  if (nextRow === 1 && dy > 0) nextRow = Math.min(maxRow, 2);

  const currentLine = model.getLineContent(ctx.startLine + ctx.activeRowIndex);
  const currentCellCol = getCursorColumnForTableCell(currentLine, ctx.activeColIndex, 0);
  const offset = Math.max(0, (pos.column || 1) - currentCellCol);

  const targetLineNumber = ctx.startLine + nextRow;
  const targetLine = model.getLineContent(targetLineNumber);
  const targetCol = getCursorColumnForTableCell(targetLine, nextCol, offset);

  editor.setPosition(new monaco.Position(targetLineNumber, targetCol));
  editor.focus();
  updateTableToolbar();
};

const insertTable = () => {
  insertTableWithSize(2, 2)
};

const insertImage = () => {
  insertBlockSnippet('![图片描述](路径或URL)\n');
};

const insertCodeBlock = (lang) => {
  const v = String(lang || '').trim();
  const prefix = v ? `\`\`\`${v}\n` : '```\n';
  wrapSelection(prefix, '\n```\n', '代码');
};

const handleCodeBlockSelect = (key) => {
  const k = String(key || '').trim();
  const lang = k === 'plain' ? '' : k;
  insertCodeBlock(lang);
};

const insertCodeBlockFromToolbar = () => {
  insertCodeBlock('');
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
    logger.error('自动保存失败:', e)
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

    const k = String(e.key || '');
    const c = String(e.code || '');
    const isAltLeft = k === 'ArrowLeft' || k === 'Left' || k === 'BrowserBack' || c === 'ArrowLeft';
    const isAltRight = k === 'ArrowRight' || k === 'Right' || k === 'BrowserForward' || c === 'ArrowRight';

    const previewOnly = mode.value === 'preview' && showPreview.value && !showEditor.value;

    if (isAltLeft) {
      e.preventDefault();
      e.stopPropagation();
      if (previewOnly) {
        navigateToPreviousPreviewLocation();
      } else {
        navigateToPreviousEditLocation();
      }
      return;
    }

    if (isAltRight) {
      e.preventDefault();
      e.stopPropagation();
      if (previewOnly) {
        navigateToNextPreviewLocation();
      } else {
        navigateToNextEditLocation();
      }
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

const handlePreviewClick = async (e) => {
  const base = e?.target instanceof Element ? e.target : e?.target?.parentElement;
  if (!base) return;

  // 纯预览模式：任意点击都记录一次当前位置（优先使用被点击块的 data-md-start-line）。
  try {
    const previewOnly = mode.value === 'preview' && showPreview.value && !showEditor.value
    const container = previewContainer.value
    if (previewOnly && container && container.contains(base)) {
      const hit = base.closest?.('[data-md-start-line]')
      const ln = Number(hit?.dataset?.mdStartLine || 0)
      const path = fileTreeStore.activeTab || selectedFile.value?.path
      const st = container.scrollTop
      const h = container.scrollHeight
      const ratio = h ? st / h : 0

      if (path) {
        let hitIndex = null
        try {
          if (hit && Number.isFinite(ln) && ln > 0) {
            const sameLine = Array.from(container.querySelectorAll(`[data-md-start-line="${ln}"]`) || [])
            const idx = sameLine.indexOf(hit)
            hitIndex = idx >= 0 ? idx : null
          }
        } catch {
          hitIndex = null
        }

        recordPreviewLocation('click', {
          path,
          scrollTop: st,
          scrollRatio: ratio,
          lineNumber: Number.isFinite(ln) && ln > 0 ? ln : undefined,
          hitIndex
        })
      } else {
        recordPreviewLocation('click')
      }
    }
  } catch (err) {
    logger.warn('[PreviewNav] record click failed', err)
  }

  const guaxiSafeDecode = (v) => {
    try {
      return decodeURIComponent(String(v || ''))
    } catch {
      return String(v || '')
    }
  }

  const formatPythonExecOutput = (res) => {
    const ok = res?.ok
    const stdout = String(res?.stdout || '')
    const stderr = String(res?.stderr || '')

    const hasStdout = stdout.trim().length > 0
    const hasStderr = stderr.trim().length > 0

    if (ok === false) {
      if (hasStderr) return stderr.trimEnd()
      if (hasStdout) return stdout.trimEnd()
      return '(执行失败：无错误输出)'
    }

    const parts = []
    if (hasStdout) parts.push(stdout.trimEnd())
    if (hasStderr) parts.push(stderr.trimEnd())

    return parts.join('\n\n')
  }

  const getPdfArtifactFromCodeCell = (res) => {
    const artifacts = Array.isArray(res?.artifacts) ? res.artifacts : []
    if (!artifacts.length) return null

    const hit = artifacts.find((a) => {
      const kind = String(a?.kind || '').trim().toLowerCase()
      const mime = String(a?.mime || '').trim().toLowerCase()
      return kind === 'pdf' || mime === 'application/pdf'
    })

    if (!hit) return null

    const dataBase64 = String(hit?.data_base64 || hit?.dataBase64 || '').trim()
    const name = String(hit?.name || hit?.filename || hit?.fileName || '').trim() || 'output.pdf'
    if (!dataBase64) return null

    return { name, mime: 'application/pdf', dataBase64 }
  }

  const isLatexKind = (kind) => {
    const k = String(kind || '').trim().toLowerCase()
    return k === 'tex' || k === 'latex'
  }

  const formatCodeCellExecOutput = (res, kind) => {
    const ok = res?.ok
    const stdout = String(res?.stdout || '')
    const stderr = String(res?.stderr || '')

    const hasStdout = stdout.trim().length > 0
    const hasStderr = stderr.trim().length > 0

    const pdf = getPdfArtifactFromCodeCell(res)

    if (ok === false) {
      if (hasStderr) return stderr.trimEnd()
      if (hasStdout) return stdout.trimEnd()
      return '(执行失败：无错误输出)'
    }

    if (isLatexKind(kind) && pdf) {
      return ''
    }

    const parts = []
    if (hasStdout) parts.push(stdout.trimEnd())
    if (hasStderr) parts.push(stderr.trimEnd())

    return parts.join('\n\n')
  }

  const base64ToUint8Array = (b64) => {
    const bin = atob(String(b64 || ''))
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i)
    return bytes
  }

  const ensureCodeCellMediaContainer = (cell, afterEl) => {
    if (!(cell instanceof Element)) return null

    let media = cell.querySelector('.guaxi-codecell-media')
    if (media) return media

    media = document.createElement('div')
    media.className = 'guaxi-codecell-media'
    media.style.marginTop = '8px'
    media.style.display = 'none'
    media.style.flexDirection = 'column'
    media.style.alignItems = 'center'
    media.style.justifyContent = 'center'
    media.style.position = 'relative'

    const border = getComputedStyle(document.documentElement).getPropertyValue('--guaxi-border') || '#e1e4e8'
    const panelBg = getComputedStyle(document.documentElement).getPropertyValue('--guaxi-panel-bg') || '#fff'

    media.style.border = `1px solid ${String(border).trim() || '#e1e4e8'}`
    media.style.borderRadius = '8px'
    media.style.padding = '10px'
    media.style.background = String(panelBg).trim() || '#fff'

    if (afterEl && afterEl.parentNode) {
      afterEl.parentNode.insertBefore(media, afterEl.nextSibling)
    } else {
      cell.appendChild(media)
    }

    return media
  }

  const renderCodeCellArtifacts = async (cell, outputPre, res, outputText, kind) => {
    const pdf = getPdfArtifactFromCodeCell(res)
    const shouldHideText = String(outputText || '').trim().length === 0

    if (!pdf) {
      const media = cell?.querySelector?.('.guaxi-codecell-media')
      if (media) {
        const oldUrl = String(media.getAttribute('data-blob-url') || '').trim()
        if (oldUrl) {
          try { URL.revokeObjectURL(oldUrl) } catch { }
          media.removeAttribute('data-blob-url')
        }

        media.innerHTML = ''
        media.style.display = 'none'
      }
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    const media = ensureCodeCellMediaContainer(cell, outputPre)
    if (!media) return

    const oldUrl = String(media.getAttribute('data-blob-url') || '').trim()
    if (oldUrl) {
      try { URL.revokeObjectURL(oldUrl) } catch { }
      media.removeAttribute('data-blob-url')
    }

    const bytes = base64ToUint8Array(pdf.dataBase64)
    const blob = new Blob([bytes], { type: pdf.mime })
    const blobUrl = URL.createObjectURL(blob)
    media.setAttribute('data-blob-url', blobUrl)

    media.style.padding = '0'
    media.style.alignItems = 'stretch'
    media.style.justifyContent = 'stretch'
    media.style.overflow = 'hidden'

    const calcPdfHeightPx = () => {
      const vh = Number(window?.innerHeight || 0)
      const ideal = Math.floor(vh > 0 ? vh * 0.78 : 820)
      return Math.max(780, Math.min(1200, ideal || 820))
    }

    const wrap = document.createElement('div')
    wrap.style.width = '100%'
    wrap.style.flex = '1 1 auto'

    const iframe = document.createElement('iframe')
    // 让 PDF 默认尽量“适应页面/容器”，减少空白浪费
    // 注意：不同内置 PDF 阅读器对 hash 参数支持不完全一致，但大多支持 zoom=page-fit/page-width 等。
    iframe.src = `${blobUrl}#zoom=page-fit`
    iframe.style.display = 'block'
    iframe.style.width = '100%'
    iframe.style.height = `${calcPdfHeightPx()}px`
    iframe.style.border = '0'
    iframe.style.borderRadius = '8px'

    wrap.appendChild(iframe)

    media.innerHTML = ''
    media.appendChild(wrap)
    media.style.display = 'flex'

    try {
      message.info('PDF 已渲染，可使用预览区工具栏下载/保存')
    } catch {
    }

    if (outputPre && shouldHideText) {
      outputPre.style.display = 'none'
    }

    logger.info('[CodeCell] pdf rendered in output area', { kind: String(kind || ''), name: pdf.name, bytes: bytes.length })
  }

  const getGuaxiDisplayFromResult = (res) => {
    const rj = res?.result_json
    if (!rj || typeof rj !== 'object') return null

    const display = rj?.__guaxi_display__ || rj?.guaxi_display || null
    if (!display || typeof display !== 'object') return null

    const t = String(display?.type || '').trim().toLowerCase()
    if (t !== 'image') return null

    const mime = String(display?.mime || '').trim() || 'image/png'
    const dataUrl = String(display?.data_url || '').trim()
    const dataBase64 = String(display?.data_base64 || '').trim()
    const data = display?.data

    return {
      type: 'image',
      mime,
      dataUrl,
      dataBase64,
      data: typeof data === 'string' ? data : null,
    }
  }

  const ensurePyCellMediaContainer = (cell, afterEl) => {
    if (!(cell instanceof Element)) return null

    let media = cell.querySelector('.guaxi-pycell-media')
    if (media) return media

    media = document.createElement('div')
    media.className = 'guaxi-pycell-media'
    media.style.marginTop = '8px'
    media.style.display = 'none'
    media.style.flexDirection = 'column'
    media.style.alignItems = 'center'
    media.style.justifyContent = 'center'

    const border = getComputedStyle(document.documentElement).getPropertyValue('--guaxi-border') || '#e1e4e8'
    const panelBg = getComputedStyle(document.documentElement).getPropertyValue('--guaxi-panel-bg') || '#fff'

    media.style.border = `1px solid ${String(border).trim() || '#e1e4e8'}`
    media.style.borderRadius = '8px'
    media.style.padding = '10px'
    media.style.background = String(panelBg).trim() || '#fff'

    if (afterEl && afterEl.parentNode) {
      afterEl.parentNode.insertBefore(media, afterEl.nextSibling)
    } else {
      cell.appendChild(media)
    }

    return media
  }

  const renderPyCellDisplay = (cell, outputPre, res, outputText) => {
    const display = getGuaxiDisplayFromResult(res)
    const shouldHideText = String(outputText || '').trim().length === 0

    if (!display) {
      const media = cell?.querySelector?.('.guaxi-pycell-media')
      if (media) {
        media.innerHTML = ''
        media.style.display = 'none'
      }
      if (outputPre && shouldHideText) outputPre.style.display = 'none'
      return
    }

    const media = ensurePyCellMediaContainer(cell, outputPre)
    if (!media) return

    const img = document.createElement('img')
    img.style.maxWidth = '100%'
    img.style.height = 'auto'
    img.style.display = 'block'
    img.style.marginLeft = 'auto'
    img.style.marginRight = 'auto'

    let src = ''
    if (display.dataUrl && display.dataUrl.startsWith('data:')) {
      src = display.dataUrl
    } else if (display.dataBase64) {
      src = `data:${display.mime};base64,${display.dataBase64}`
    } else if (display.data != null) {
      src = `data:${display.mime},${encodeURIComponent(display.data)}`
    }

    if (!src) {
      media.innerHTML = ''
      media.style.display = 'none'
      return
    }

    img.src = src
    media.innerHTML = ''
    media.appendChild(img)
    media.style.display = 'flex'

    if (outputPre && shouldHideText) {
      outputPre.style.display = 'none'
    }
  }

  const upsertPythonOutputBlock = (markdown, startLine, cellId, outputText) => {
    const marker = `<!-- guaxi:pyout:${cellId} -->`
    const outputFenceStart = '```text'
    const outputFenceEnd = '```'

    const lines = String(markdown || '').split(/\r?\n|\r/)

    const findOpenFenceFrom = (idx) => {
      for (let i = Math.max(0, idx); i < lines.length; i += 1) {
        const t = String(lines[i] || '').trim()
        if (/^```\s*(python|py)(\s|$)/i.test(t)) return i
      }
      return -1
    }

    const openIdx = Number.isFinite(startLine)
      ? findOpenFenceFrom(Math.max(0, startLine - 1))
      : findOpenFenceFrom(0)

    if (openIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    let closeIdx = -1
    for (let i = openIdx + 1; i < lines.length; i += 1) {
      if (String(lines[i] || '').trim() === '```') {
        closeIdx = i
        break
      }
    }

    if (closeIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    const markerIdx = lines.findIndex((l) => String(l || '').trim() === marker)
    if (markerIdx >= 0) {
      let fenceStartIdx = -1
      for (let i = markerIdx + 1; i < lines.length; i += 1) {
        if (String(lines[i] || '').trim().startsWith('```')) {
          fenceStartIdx = i
          break
        }
      }
      if (fenceStartIdx >= 0) {
        let fenceEndIdx = -1
        for (let i = fenceStartIdx + 1; i < lines.length; i += 1) {
          if (String(lines[i] || '').trim() === '```') {
            fenceEndIdx = i
            break
          }
        }
        if (fenceEndIdx >= 0) {
          lines.splice(markerIdx, fenceEndIdx - markerIdx + 1, marker, outputFenceStart, outputText, outputFenceEnd)
          return `${lines.join('\n')}\n`
        }
      }
    }

    lines.splice(closeIdx + 1, 0, '', marker, outputFenceStart, outputText, outputFenceEnd)
    return `${lines.join('\n')}\n`
  }

  const upsertCodeCellOutputBlock = (markdown, startLine, cellId, kind, outputText) => {
    const marker = `<!-- guaxi:cellout:${cellId} -->`
    const outputFenceStart = '```text'
    const outputFenceEnd = '```'

    const lines = String(markdown || '').split(/\r?\n|\r/)

    const kindToFenceRe = (k) => {
      const t = String(k || '').trim().toLowerCase()
      if (t === 'javascript') return /^(javascript|js)(\s|$)/i
      if (t === 'latex') return /^(latex|tex)(\s|$)/i
      if (t === 'c') return /^c(\s|$)/i
      if (t === 'cpp') return /^(cpp|c\+\+)(\s|$)/i
      if (t === 'rust') return /^(rust|rs)(\s|$)/i
      return null
    }

    const fenceRe = kindToFenceRe(kind)

    const findOpenFenceFrom = (idx) => {
      for (let i = Math.max(0, idx); i < lines.length; i += 1) {
        const t = String(lines[i] || '').trim()
        if (!t.startsWith('```')) continue
        const info = t.replace(/^```\s*/, '')
        if (fenceRe ? fenceRe.test(info) : true) return i
      }
      return -1
    }

    const openIdx = Number.isFinite(startLine)
      ? findOpenFenceFrom(Math.max(0, startLine - 1))
      : findOpenFenceFrom(0)

    if (openIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    let closeIdx = -1
    for (let i = openIdx + 1; i < lines.length; i += 1) {
      if (String(lines[i] || '').trim() === '```') {
        closeIdx = i
        break
      }
    }

    if (closeIdx < 0) {
      return `${String(markdown || '').trim()}\n\n${marker}\n${outputFenceStart}\n${outputText}\n${outputFenceEnd}\n`
    }

    const markerIdx = lines.findIndex((l) => String(l || '').trim() === marker)
    if (markerIdx >= 0) {
      let fenceStartIdx = -1
      for (let i = markerIdx + 1; i < lines.length; i += 1) {
        if (String(lines[i] || '').trim().startsWith('```')) {
          fenceStartIdx = i
          break
        }
      }
      if (fenceStartIdx >= 0) {
        let fenceEndIdx = -1
        for (let i = fenceStartIdx + 1; i < lines.length; i += 1) {
          if (String(lines[i] || '').trim() === '```') {
            fenceEndIdx = i
            break
          }
        }
        if (fenceEndIdx >= 0) {
          lines.splice(markerIdx, fenceEndIdx - markerIdx + 1, marker, outputFenceStart, outputText, outputFenceEnd)
          return `${lines.join('\n')}\n`
        }
      }
    }

    lines.splice(closeIdx + 1, 0, '', marker, outputFenceStart, outputText, outputFenceEnd)
    return `${lines.join('\n')}\n`
  }

  const applyMarkdownReplaceFromPreview = (nextMarkdown, { action = 'pycell-run-save' } = {}) => {
    const targetEditor = editor
    const model = targetEditor?.getModel?.()

    if (targetEditor && model) {
      try {
        const selection = targetEditor.getSelection?.()
        targetEditor.executeEdits(action, [
          {
            range: model.getFullModelRange(),
            text: String(nextMarkdown || ''),
            forceMoveMarkers: true
          }
        ])
        if (selection) targetEditor.setSelection(selection)
        return true
      } catch (err) {
        logger.warn('[PyCell] executeEdits failed, fallback to updateEditorContent', err)
      }
    }

    updateEditorContent(String(nextMarkdown || ''))
    return true
  }

  // 可执行 Python 代码块：预览点击“运行/运行并写回/显示结果”
  const pyBtn = base.closest?.('[data-guaxi-py-action]')
  if (pyBtn && previewContainer.value && previewContainer.value.contains(pyBtn)) {
    const action = String(pyBtn.getAttribute('data-guaxi-py-action') || '').trim()

    if (action === 'toggle-output') {
      const cell = pyBtn.closest?.('.guaxi-pycell')
      if (!cell) return

      const outputEl = cell.querySelector?.('[data-guaxi-py-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const media = cell.querySelector?.('.guaxi-pycell-media')

      if (!outputPre && !media) {
        message.warning('未找到运行结果区域')
        e.stopPropagation()
        return
      }

      const hasText = String(outputEl?.textContent || '').trim().length > 0
      const hasImage = !!media?.querySelector?.('img')

      const isVisible = (el) => {
        if (!el) return false
        return String(el.style.display || '').trim() !== 'none'
      }

      const currentlyVisible = isVisible(outputPre) || isVisible(media)
      const nextShow = !currentlyVisible

      if (outputPre) {
        outputPre.style.display = nextShow && hasText ? '' : 'none'
      }
      if (media) {
        media.style.display = nextShow && hasImage ? 'flex' : 'none'
      }

      const anyVisibleAfter = isVisible(outputPre) || isVisible(media)
      pyBtn.textContent = anyVisibleAfter ? '隐藏结果' : '显示结果'

      logger.info('[PyCell] toggle output', {
        nextShow,
        hasText,
        hasImage,
        textVisible: isVisible(outputPre),
        imageVisible: isVisible(media),
      })

      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'run' || action === 'run-save') {
      const cell = pyBtn.closest?.('.guaxi-pycell')
      if (!cell) return

      const code = guaxiSafeDecode(cell.getAttribute('data-guaxi-py-code'))
      const cellId = guaxiSafeDecode(cell.getAttribute('data-guaxi-py-cell'))
      const startLine = Number(cell.getAttribute('data-guaxi-md-start-line'))

      if (!code) {
        message.warning('Python 代码为空')
        e.stopPropagation()
        return
      }

      const outputEl = cell.querySelector?.('[data-guaxi-py-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const toggleBtn = cell.querySelector?.('[data-guaxi-py-action="toggle-output"]')
      const syncToggleText = () => {
        if (!toggleBtn) return
        const media = cell.querySelector?.('.guaxi-pycell-media')
        const textVisible = outputPre ? String(outputPre.style.display || '').trim() !== 'none' : false
        const imageVisible = media ? String(media.style.display || '').trim() !== 'none' : false
        toggleBtn.textContent = textVisible || imageVisible ? '隐藏结果' : '显示结果'
      }

      // 说明：输出容器默认隐藏（避免未运行时占位）；运行时再展示。
      // 运行前清理上次的展示（例如 __guaxi_display__ 生成的图片），避免旧内容闪现。
      const media = cell.querySelector?.('.guaxi-pycell-media')
      if (media) {
        media.innerHTML = ''
        media.style.display = 'none'
      }

      if (outputPre) {
        outputPre.style.display = ''
      }

      syncToggleText()
      if (outputEl) outputEl.textContent = '运行中...'

      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-py-action]') || [])
      relatedBtns.forEach((b) => {
        try {
          b.setAttribute('disabled', 'true')
          b.style.opacity = '0.6'
          b.style.cursor = 'not-allowed'
        } catch {
        }
      })

      try {
        const docPath = selectedFile.value?.path || ''
        const sessionId = docPath ? String(docPath) : 'guaxi:pycell:global'

        logger.info('[PyCell] python_exec', {
          action,
          cellId,
          codeLen: code.length,
          docPath,
          sessionId,
        })

        const res = await invoke('python_exec', {
          request: {
            code,
            persist: true,
            expect_json: true,
            vars: {
              _guaxi_session: sessionId,
              _doc_path: String(docPath),
              _py_cell_id: String(cellId || ''),
            },
          },
        })

        logger.info('[PyCell] python_exec result', {
          ok: res?.ok,
          stdoutLen: String(res?.stdout || '').length,
          stderrLen: String(res?.stderr || '').length,
          hasResultRepr: !!res?.result_repr,
          hasResultJson: res?.result_json != null,
        })

        const outputText = formatPythonExecOutput(res)

        const hideText = String(outputText || '').trim().length === 0

        if (outputPre) {
          outputPre.style.display = hideText ? 'none' : ''
        }
        syncToggleText()

        if (outputEl) {
          outputEl.textContent = hideText ? '' : outputText
        }

        renderPyCellDisplay(cell, outputPre, res, outputText)

        if (res?.ok === false) {
          message.error('Python 执行失败')
        }

        if (action === 'run-save') {
          const hasTextOutput = String(outputText || '').trim().length > 0
          if (!hasTextOutput) {
            logger.info('[PyCell] run-save skipped: empty text output', { cellId })
            message.success('运行完成（无文字输出，未写回）')
          } else {
            suppressPreviewAutoRenderOnce(`pycell:${action}`, 1500)

            const current = String(editor?.getModel?.()?.getValue?.() || rawContent.value || '')
            const nextMarkdown = upsertPythonOutputBlock(
              current,
              Number.isFinite(startLine) ? startLine : null,
              cellId || 'pycell',
              outputText
            )

            applyMarkdownReplaceFromPreview(nextMarkdown, { action: 'pycell-run-save' })
            message.success('已写回到文档（未自动保存）')
          }
        } else {
          message.success('运行完成')
        }
      } catch (err) {
        logger.error('[PyCell] run failed', err)
        if (outputPre) {
          outputPre.style.display = ''
        }
        syncToggleText()
        if (outputEl) outputEl.textContent = `运行失败：${String(err?.message || err)}`
        message.error('运行失败')
      } finally {
        relatedBtns.forEach((b) => {
          try {
            b.removeAttribute('disabled')
            b.style.opacity = ''
            b.style.cursor = ''
          } catch {
          }
        })
      }

      e.preventDefault()
      e.stopPropagation()
      return
    }
  }

  // 可执行通用代码块：预览点击“运行/运行并写回/显示结果”
  const cellBtn = base.closest?.('[data-guaxi-cell-action]')
  if (cellBtn && previewContainer.value && previewContainer.value.contains(cellBtn)) {
    const action = String(cellBtn.getAttribute('data-guaxi-cell-action') || '').trim()

    if (action === 'toggle-output') {
      const cell = cellBtn.closest?.('.guaxi-codecell')
      if (!cell) return

      const outputEl = cell.querySelector?.('[data-guaxi-cell-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const media = cell.querySelector?.('.guaxi-codecell-media')

      if (!outputPre && !media) {
        message.warning('未找到运行结果区域')
        e.stopPropagation()
        return
      }

      const hasText = String(outputEl?.textContent || '').trim().length > 0
      const hasMedia = !!media && media.childNodes && media.childNodes.length > 0

      const isVisible = (el) => {
        if (!el) return false
        return String(el.style.display || '').trim() !== 'none'
      }

      const currentlyVisible = isVisible(outputPre) || isVisible(media)
      const nextShow = !currentlyVisible

      if (outputPre) {
        outputPre.style.display = nextShow && hasText ? '' : 'none'
      }
      if (media) {
        media.style.display = nextShow && hasMedia ? 'flex' : 'none'
      }

      const anyVisibleAfter = isVisible(outputPre) || isVisible(media)
      cellBtn.textContent = anyVisibleAfter ? '隐藏结果' : '显示结果'

      logger.info('[CodeCell] toggle output', {
        nextShow,
        hasText,
        hasMedia,
        textVisible: isVisible(outputPre),
        mediaVisible: isVisible(media),
      })

      e.preventDefault()
      e.stopPropagation()
      return
    }

    if (action === 'run' || action === 'run-save') {
      const cell = cellBtn.closest?.('.guaxi-codecell')
      if (!cell) return

      const kind = guaxiSafeDecode(cell.getAttribute('data-guaxi-cell-kind'))
      const code = guaxiSafeDecode(cell.getAttribute('data-guaxi-cell-code'))
      const cellId = guaxiSafeDecode(cell.getAttribute('data-guaxi-cell-id'))
      const startLine = Number(cell.getAttribute('data-guaxi-md-start-line'))

      if (!code) {
        message.warning('代码为空')
        e.stopPropagation()
        return
      }

      const outputEl = cell.querySelector?.('[data-guaxi-cell-output]')
      const outputPre = outputEl ? outputEl.closest?.('pre') : null
      const toggleBtn = cell.querySelector?.('[data-guaxi-cell-action="toggle-output"]')
      const syncToggleText = () => {
        if (!toggleBtn) return
        const media = cell.querySelector?.('.guaxi-codecell-media')
        const textVisible = outputPre ? String(outputPre.style.display || '').trim() !== 'none' : false
        const mediaVisible = media ? String(media.style.display || '').trim() !== 'none' : false
        toggleBtn.textContent = textVisible || mediaVisible ? '隐藏结果' : '显示结果'
      }

      if (outputPre) {
        outputPre.style.display = ''
      }
      syncToggleText()
      if (outputEl) outputEl.textContent = '运行中...'

      const relatedBtns = Array.from(cell.querySelectorAll?.('[data-guaxi-cell-action]') || [])
      relatedBtns.forEach((b) => {
        try {
          b.setAttribute('disabled', 'true')
          b.style.opacity = '0.6'
          b.style.cursor = 'not-allowed'
        } catch {
        }
      })

      try {
        const docPath = selectedFile.value?.path || ''

        logger.info('[CodeCell] code_cell_exec', {
          action,
          kind,
          cellId,
          codeLen: code.length,
          docPath,
        })

        const res = await invoke('code_cell_exec', {
          request: {
            kind: String(kind || '').trim(),
            code,
            vars: {
              _doc_path: String(docPath),
              _cell_id: String(cellId || ''),
            },
          }
        })

        logger.info('[CodeCell] code_cell_exec result', {
          ok: res?.ok,
          stdoutLen: String(res?.stdout || '').length,
          stderrLen: String(res?.stderr || '').length,
          hasArtifacts: Array.isArray(res?.artifacts) && res.artifacts.length > 0,
        })

        const outputText = formatCodeCellExecOutput(res, kind)
        const hideText = String(outputText || '').trim().length === 0

        if (outputPre) {
          outputPre.style.display = hideText ? 'none' : ''
        }
        syncToggleText()

        if (outputEl) {
          outputEl.textContent = hideText ? '' : outputText
        }

        await renderCodeCellArtifacts(cell, outputPre, res, outputText, kind)
        syncToggleText()

        if (res?.ok === false) {
          message.error('执行失败')
        }

        if (action === 'run-save') {
          suppressPreviewAutoRenderOnce(`codecell:${action}`, 1500)

          const current = String(editor?.getModel?.()?.getValue?.() || rawContent.value || '')
          const nextMarkdown = upsertCodeCellOutputBlock(
            current,
            Number.isFinite(startLine) ? startLine : null,
            cellId || `cell-${String(kind || 'code')}`,
            kind,
            outputText
          )

          applyMarkdownReplaceFromPreview(nextMarkdown, { action: 'codecell-run-save' })
          message.success('已写回到文档（未自动保存）')
        } else {
          message.success('运行完成')
        }
      } catch (err) {
        logger.error('[CodeCell] run failed', err)
        if (outputPre) {
          outputPre.style.display = ''
        }
        syncToggleText()
        if (outputEl) outputEl.textContent = `运行失败：${String(err?.message || err)}`
        message.error('运行失败')
      } finally {
        relatedBtns.forEach((b) => {
          try {
            b.removeAttribute('disabled')
            b.style.opacity = ''
            b.style.cursor = ''
          } catch {
          }
        })
      }

      e.preventDefault()
      e.stopPropagation()
      return
    }
  }

  // 任务列表：点击 checkbox 时，直接更新 Markdown 源内容（并仅更新当前条目 DOM）
  const checkbox = base.closest?.('input.task-list-item-checkbox');
  if (checkbox && previewContainer.value && previewContainer.value.contains(checkbox)) {
    const lineFromAttr = Number(checkbox.getAttribute('data-md-task-line') || 0);
    const lineFromParent = Number(
      checkbox.closest?.('[data-md-start-line]')?.getAttribute?.('data-md-start-line') || 0
    );
    const ln = lineFromAttr || lineFromParent || 0;
    const checked = !!checkbox.checked;

    if (!ln) {
      logger.warn('[TaskList] preview toggle ignored: missing line', { checked });
      return;
    }

    const ok = toggleMarkdownTaskAtLine(ln, checked, { source: 'preview' });

    // 仅更新被点击的条目：不依赖整篇 Markdown 重渲染
    const li = checkbox.closest?.('li.task-list-item');
    if (li) {
      li.classList.toggle('task-list-item-checked', checked);
      li.setAttribute('data-md-task', checked ? '1' : '0');
    }

    if (!ok) {
      logger.warn('[TaskList] preview toggle failed', { ln, checked });
    } else {
      logger.info('[TaskList] preview toggled (dom patched)', { ln, checked });
    }

    e.stopPropagation();
    return;
  }

  const a = base.closest('a');
  if (a && previewContainer.value && previewContainer.value.contains(a)) {
    const href0 = a.getAttribute('href') || '';
    const href = String(href0 || '').trim();
    const ctrl = !!(e?.ctrlKey || e?.metaKey);

    if (href) {
      if (href.toLowerCase() === '@ref') {
        e.preventDefault();
        e.stopPropagation();

        const text = String(a.textContent || '').trim();
        const m = text.match(/\d+/);
        const num = m ? m[0] : '';
        if (!num) {
          logger.debug('[Footnote] invalid ref click: missing number', { href, text });
          return;
        }

        jumpToFootnoteDefinition(num, { source: 'preview' });
        return;
      }

      if (href.toLowerCase().startsWith('guaxi://wikilink')) {
        e.preventDefault();
        e.stopPropagation();

        const baseDocPath = selectedFile.value?.path || fileTreeStore.activeTab || '';
        await openWikilinkFromHref(href, { baseDocPath, source: 'preview' });
        return;
      }

      if (href.startsWith('#')) {
        e.preventDefault();
        e.stopPropagation();

        recordPreviewLocation('anchor');

        const id = decodeURIComponent(href.slice(1));
        const container = previewContainer.value;
        if (container) {
          const target = container.querySelector(`#${CSS.escape(id)}`);
          target?.scrollIntoView?.({ behavior: 'smooth', block: 'start' });
        }
        return;
      }

      if (isHttpUrl(href)) {
        e.preventDefault();
        e.stopPropagation();

        try {
          await openUrl(href);
        } catch (err) {
          logger.error('[LinkOpen] openUrl failed', { href }, err);
          message.error('打开浏览器失败');
        }
        return;
      }

      e.preventDefault();
      e.stopPropagation();

      const { link, anchor } = splitFsLinkAndAnchor(href);
      if (!link) return;
      const baseDocPath = selectedFile.value?.path || fileTreeStore.activeTab || '';

      logger.debug('[LinkOpen] preview openFsLink', { link, anchor, ctrl, baseDocPath });
      await openFsLinkSmart(link, { anchor, viaCtrl: ctrl, baseDocPath });
      return;
    }
  }

  const hit = base.closest('[data-md-start-line]');
  if (!hit) return;

  const line = Number(hit.dataset.mdStartLine || 1);
  if (showEditor.value) {
    const shouldFocus = showEditor.value;
    syncEditorToLine(line, { focus: shouldFocus });
  }
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
    const processed = preprocessObsidianWikilinks(content);
    renderedHtml.value = await renderMarkdown(processed, {
      basePath: basePath,
      mermaid: true,
      math: true,
      codeHighlight: true,
      pythonCells: true,
      codeCells: true,
      theme: settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default'
    });

    lastPreviewRenderedSource.value = content;

    // 确保DOM更新后添加按钮
    nextTick(() => {
      if (previewContainer.value) {
        addCopyButtons(previewContainer.value, {
          theme: settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default'
        });
        addMediaViewerButtons(previewContainer.value, {
          theme: settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default'
        });
      }
    });
  } catch (error) {
    logger.error('渲染错误:', error);
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

    const language = resolveMonacoLanguageByPath(newFile.path || '');

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
      enableLspForEditorLanguage(targetEditor, language, newFile.path || '');
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
  { label: '编辑模式 (Ctrl+1)', key: 'edit', icon: () => h(Icon, { icon: filePenLineIcon, width: 16, height: 16 }) },
  { label: '预览模式 (Ctrl+2)', key: 'preview', icon: () => h(Icon, { icon: eyeIcon, width: 16, height: 16 }) },
  { label: '分屏预览 (Ctrl+3)', key: 'split', icon: () => h(Icon, { icon: columns2Icon, width: 16, height: 16 }) },
  { label: '实时预览 (Ctrl+4)', key: 'realtime', icon: () => h(Icon, { icon: sparklesIcon, width: 16, height: 16 }) }
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

        installRealtimeVditorLinkInterceptor();

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

  if (typeof realtimeVditorLinkInterceptorCleanup === 'function') {
    realtimeVditorLinkInterceptorCleanup();
    realtimeVditorLinkInterceptorCleanup = null;
  }

  if (!realtimeVditor) return;
  try {
    realtimeVditor.destroy();
  } finally {
    realtimeVditor = null;
    realtimeVditorReady.value = false;
    pendingRealtimeVditorValue.value = null;
  }
};

const normalizeHomePreviewModeKey = (key) => {
  const k = String(key || '').trim().toLowerCase();
  return ['edit', 'preview', 'split', 'realtime'].includes(k) ? k : 'edit';
};

const getHomePreviewModeKeyFromState = () => {
  if (previewMode.value === 'toggle') {
    return mode.value === 'preview' ? 'preview' : 'edit';
  }
  return normalizeHomePreviewModeKey(previewMode.value);
};

const setHomePreviewModeKeySafe = (key) => {
  const next = normalizeHomePreviewModeKey(key);

  const setter = editorStore?.setHomePreviewModeKey;
  if (typeof setter === 'function') {
    try {
      setter.call(editorStore, next);
      return;
    } catch (e) {
      logger.warn('[Editor] setHomePreviewModeKey failed, fallback to direct write', e);
    }
  } else {
    logger.warn('[Editor] editorStore.setHomePreviewModeKey is not a function, fallback to direct write', {
      type: typeof setter,
      key: next,
    });
  }

  try {
    editorStore.homePreviewModeKey = next;
  } catch (e) {
    logger.error('[Editor] fallback write homePreviewModeKey failed', e);
  }
};

const handlePreviewModeSelect = async (key) => {
  const desiredKey = normalizeHomePreviewModeKey(key);

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

  if (desiredKey === 'edit') {
    setHomePreviewModeKeySafe('edit');
    previewMode.value = 'toggle';
    await setMainMode('edit');
    return;
  }

  if (desiredKey === 'preview') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      setHomePreviewModeKeySafe('edit');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      return;
    }
    setHomePreviewModeKeySafe('preview');
    previewMode.value = 'toggle';
    await setMainMode('preview');
    return;
  }

  if (desiredKey === 'split') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      setHomePreviewModeKeySafe('edit');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      renderedHtml.value = '';
      return;
    }

    setHomePreviewModeKeySafe('split');
    previewMode.value = 'split';
    await setMainMode('edit');
    await nextTick();
    refreshPreview();
    return;
  }

  if (desiredKey === 'realtime') {
    if (!canPreview.value) {
      message.warning('仅 Markdown 文件支持预览');
      setHomePreviewModeKeySafe('edit');
      previewMode.value = 'toggle';
      await setMainMode('edit');
      renderedHtml.value = '';
      return;
    }

    setHomePreviewModeKeySafe('realtime');
    captureScrollPosition();
    pendingRealtimeScrollRestore.value = true;
    previewMode.value = 'realtime';
    await setMainMode('edit');
  }
};

const restoreHomePreviewMode = async (from = 'mounted') => {
  const desiredKey = normalizeHomePreviewModeKey(editorStore.homePreviewModeKey);
  const currentKey = getHomePreviewModeKeyFromState();
  if (currentKey === desiredKey) return;

  logger.debug('[Editor] restoreHomePreviewMode', { from, desiredKey, currentKey });
  await handlePreviewModeSelect(desiredKey);
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
  if (!shouldAutoRenderPreview.value) return;

  // 任务列表点击会回写 rawContent，但预览仅做局部 DOM 更新；此处抑制整篇重渲染。
  if (consumeSuppressPreviewAutoRender('watch:rawContent')) return;

  renderDebounced(rawContent.value);
});

// 监听内容变化
watch(renderedHtml, () => {
  nextTick(() => {
    if (previewContainer.value) {
      addCopyButtons(previewContainer.value, {
        theme: settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default'
      });
      addMediaViewerButtons(previewContainer.value, {
        theme: settingsStore.appearanceTheme === 'dark' ? 'dark' : 'default'
      });
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
    message.warning('当前没有选中的文件');
    return;
  }

  const path = selectedFile.value.path || '';
  const ext = path.split('.').pop()?.toLowerCase() || '';

  if (mode.value === 'edit' && editor) {
    saveContent();
  }

  // 说明：.tex 文件导出 PDF 走 tectonic（不走 pandoc）
  const isTexFile = ext === 'tex'
  const isMarkdownFile = ext === 'md'

  if (!isMarkdownFile && !(isTexFile && format === 'pdf')) {
    if (isTexFile) {
      message.warning('当前为 .tex 文件，仅支持导出为 PDF')
    } else {
      message.warning('仅支持对 Markdown(.md) 文件进行导出')
    }
    return
  }

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

    if (isTexFile && format === 'pdf') {
      await invoke('export_tex_pdf_tectonic', {
        inputPath: path,
        outputPath
      })
    } else if (format === 'pdf') {
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
    logger.error('导出 Markdown 失败:', error);
    message.error('导出 Markdown 失败');
  }
};

const switchTab = (path, region) => {
  const targetRegion = region || 'left';

  logger.debug('[Editor] switchTab', {
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

  logger.info("WWW>>> applyHighlights", { lineNumber, matches, text });

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
  } else if (typeof text === 'string' && text.trim()) {
    const query = text.trim();
    const foundMatches = model.findMatches(query, false, false, true, null, true);

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
    logger.info("WWW>>> editorStore.currentFile", file)
    editor.setValue(file.content);
    nextTick(applyHighlights);
  }
});

watch(() => editorStore.highlightInfo, () => {
  if (editor) {
    logger.info("WWW>>> editorStore.highlightInfo", editorStore.highlightInfo)
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
    recordPreviewLocation('outline');
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

const handleLspSettingChanged = (evt) => {
  const enabled = !!evt?.detail?.enabled
  lspEnabled.value = enabled
  applyMonacoThemeByLsp(enabled)

  if (enabled) {
    clearMarkdownFenceSyntaxHighlight(editor)
    clearMarkdownFenceSyntaxHighlight(secondaryEditor)
  } else {
    scheduleMarkdownFenceSyntaxHighlight(editor, 'setting')
    scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'setting')
  }

  if (!enabled) {
    disposeAllLspClients()
    return
  }

  resetLspDisabled()

  const mainModel = editor?.getModel?.()
  if (mainModel) {
    enableLspForEditorLanguage(editor, mainModel.getLanguageId?.() || 'plaintext', selectedFile.value?.path || '')
  }

  const rightPath = fileTreeStore.activeTabRight || ''
  const rightModel = secondaryEditor?.getModel?.()
  if (rightModel) {
    enableLspForEditorLanguage(secondaryEditor, rightModel.getLanguageId?.() || 'plaintext', rightPath)
  }
}

watch(lspEnabled, (enabled) => {
  applyMonacoThemeByLsp(enabled)
  if (enabled) {
    clearMarkdownFenceSyntaxHighlight(editor)
    clearMarkdownFenceSyntaxHighlight(secondaryEditor)
    return
  }
  scheduleMarkdownFenceSyntaxHighlight(editor, 'setting')
  scheduleMarkdownFenceSyntaxHighlight(secondaryEditor, 'setting')
})

// 生命周期
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('guaxi:lsp-setting-changed', handleLspSettingChanged);
  document.addEventListener('selectionchange', handleGlobalSelectionChange, true)
  document.addEventListener('mousedown', handleGlobalMouseDownForPreviewToolbar, true)
  outlineStore.setJumpToLine(jumpToLine)
  initGitService()
  void refreshLspEnabledSetting()

  restoreHomePreviewMode('mounted')

  const intervalMs = Number(editorStore.autoSaveIntervalMs) || AUTO_SAVE_FALLBACK_INTERVAL_MS
  if (editorStore.autoSaveEnabled !== false && intervalMs > 0) {
    autoSaveTimerId = window.setInterval(autoSaveDirtyTabs, intervalMs)
  }
});

onActivated(() => {
  restoreHomePreviewMode('activated')
})

onUnmounted(() => {
  stopSelectionAudioPlayback()
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('guaxi:lsp-setting-changed', handleLspSettingChanged);
  document.removeEventListener('selectionchange', handleGlobalSelectionChange, true)
  document.removeEventListener('mousedown', handleGlobalMouseDownForPreviewToolbar, true)
  clearPreviewSelectionState()
  destroyRealtimeVditor();

  if (monacoCtrlClickDisposable) {
    monacoCtrlClickDisposable.dispose();
    monacoCtrlClickDisposable = null;
  }
  if (secondaryMonacoCtrlClickDisposable) {
    secondaryMonacoCtrlClickDisposable.dispose();
    secondaryMonacoCtrlClickDisposable = null;
  }
  if (monacoLinkLikeDecorDisposable) {
    monacoLinkLikeDecorDisposable.dispose();
    monacoLinkLikeDecorDisposable = null;
  }
  if (secondaryMonacoLinkLikeDecorDisposable) {
    secondaryMonacoLinkLikeDecorDisposable.dispose();
    secondaryMonacoLinkLikeDecorDisposable = null;
  }

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

  markdownFenceCompletionModels.forEach((m) => {
    try {
      m.dispose?.()
    } catch {
    }
  })
  markdownFenceCompletionModels.clear()
  disposeAllLspClients()
});

</script>


<style scoped>
.guaxi-editor {
  --editor-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.editor-header {
  position: sticky;
  top: 0;
  z-index: 50;
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
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
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

.breadcrumb-picker-item {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 220px;
  max-width: 420px;
}

.breadcrumb-picker-icon {
  flex: 0 0 auto;
  opacity: 0.9;
}

.breadcrumb-picker-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.breadcrumb-picker-item.active .breadcrumb-picker-label {
  font-weight: 650;
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

.assistant-translate-dialog-row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
  color: var(--guaxi-text-secondary);
}

.assistant-translate-dialog-actions {
  margin-top: 12px;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
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

.table-insert-popover {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  width: fit-content;
  max-width: min(240px, calc(100vw - 32px));
  padding: 6px;
}

.table-insert-hint {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.9;
}

.table-insert-grid {
  display: inline-grid;
  width: fit-content;
  grid-template-columns: repeat(10, 16px);
  gap: 4px;
  padding: 6px;
  border-radius: 10px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  user-select: none;
}

.table-insert-cell {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 75%, transparent);
}

.table-insert-cell.active {
  background: rgba(74, 202, 219, 0.65);
  border-color: rgba(74, 202, 219, 0.9);
}

.table-insert-controls {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.table-insert-inputs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.table-insert-inputs :deep(.n-input-number) {
  width: 74px;
}

.table-insert-mul {
  opacity: 0.7;
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

.editor-content.split > .preview-shell,
.editor-content.split > .editor-wrapper {
  flex: 1 1 0;
  min-width: 0;
  width: 0;
}

.editor-content.split > .editor-wrapper {
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

.editor-wrapper :deep(.monaco-scrollable-element) {
  scrollbar-width: thin;
  scrollbar-color: rgba(120, 120, 120, 0.45) transparent;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar) {
  width: 8px;
  height: 8px;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-track) {
  background: transparent;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(120, 120, 120, 0.45);
  border-radius: 999px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(120, 120, 120, 0.65);
}

:global(:root[data-guaxi-theme='dark']) .editor-wrapper :deep(.monaco-scrollable-element) {
  scrollbar-color: rgba(255, 255, 255, 0.28) transparent;
}

:global(:root[data-guaxi-theme='dark']) .editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb) {
  background-color: rgba(255, 255, 255, 0.28);
}

:global(:root[data-guaxi-theme='dark']) .editor-wrapper :deep(.monaco-scrollable-element::-webkit-scrollbar-thumb:hover) {
  background-color: rgba(255, 255, 255, 0.45);
}

.guaxi-editor :deep(.md-fence-token-base) { color: #abb2bf; }
.guaxi-editor :deep(.md-fence-token-keyword) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-string) { color: #98c379; }
.guaxi-editor :deep(.md-fence-token-number) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-boolean) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-comment) { color: #5c6370; }
.guaxi-editor :deep(.md-fence-token-type) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-class) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-function) { color: #61afef; }
.guaxi-editor :deep(.md-fence-token-method) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-parameter) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-variable) { color: #e06c75; }
.guaxi-editor :deep(.md-fence-token-constant) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-regexp) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-property) { color: #98c379; }
.guaxi-editor :deep(.md-fence-token-namespace) { color: #e5c07b; }
.guaxi-editor :deep(.md-fence-token-tag) { color: #e06c75; }
.guaxi-editor :deep(.md-fence-token-attribute) { color: #d19a66; }
.guaxi-editor :deep(.md-fence-token-builtin) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-decorator) { color: #c678dd; }
.guaxi-editor :deep(.md-fence-token-operator) { color: #56b6c2; }
.guaxi-editor :deep(.md-fence-token-delimiter) { color: #abb2bf; }

:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-base) { color: #abb2bf; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-keyword) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-string) { color: #98c379; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-number) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-boolean) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-comment) { color: #5c6370; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-type) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-class) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-function) { color: #61afef; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-method) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-parameter) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-variable) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-constant) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-regexp) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-property) { color: #98c379; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-namespace) { color: #e5c07b; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-tag) { color: #e06c75; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-attribute) { color: #d19a66; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-builtin) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-decorator) { color: #c678dd; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-operator) { color: #56b6c2; }
:global(:root[data-guaxi-theme='dark']) .guaxi-editor :deep(.md-fence-token-delimiter) { color: #abb2bf; }

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

.preview-shell {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  width: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
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
  position: relative;
}

.preview :deep(a),
.preview :deep(a:visited) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da));
  text-decoration: underline;
  text-underline-offset: 2px;
}

.dark-mode .preview :deep(a),
.dark-mode .preview :deep(a:visited),
.dark .preview :deep(a),
.dark .preview :deep(a:visited) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

.preview :deep(a:hover) {
  opacity: 0.92;
}

/* ==================== Markdown 任务列表（Task List）样式 ==================== */
.preview :deep(li.task-list-item) {
  list-style: none;
}

.preview :deep(input.task-list-item-checkbox) {
  width: 16px;
  height: 16px;
  margin-right: 8px;
  vertical-align: -2px;
  cursor: pointer;

  /* 圆圈样式：替代默认方形 checkbox */
  appearance: none;
  -webkit-appearance: none;
  outline: none;
  border-radius: 999px;
  border: 2px solid var(--primary-color, #4acadb);
  background: transparent;
  display: inline-grid;
  place-content: center;
  transition: background-color 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
}

.preview :deep(input.task-list-item-checkbox:hover) {
  transform: scale(1.05);
}

.preview :deep(input.task-list-item-checkbox::after) {
  content: '';
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: #ffffff;
  transform: scale(0);
  transition: transform 0.12s ease;
}

.preview :deep(input.task-list-item-checkbox:checked) {
  background: var(--primary-color, #4acadb);
  border-color: var(--primary-color, #4acadb);
}

.preview :deep(input.task-list-item-checkbox:checked::after) {
  transform: scale(1);
}

.dark-mode .preview :deep(input.task-list-item-checkbox),
.dark .preview :deep(input.task-list-item-checkbox) {
  border-color: var(--guaxi-text-link-dark, #4acadb);
}

.dark-mode .preview :deep(input.task-list-item-checkbox:checked),
.dark .preview :deep(input.task-list-item-checkbox:checked) {
  background: var(--guaxi-text-link-dark, #4acadb);
  border-color: var(--guaxi-text-link-dark, #4acadb);
}

.preview :deep(li.task-list-item.task-list-item-checked .task-list-item-text) {
  text-decoration: line-through;
  opacity: 0.78;
}

.preview :deep(.md-toc) {
  border: 1px solid var(--guaxi-border, #e1e4e8);
  border-radius: 10px;
  padding: 10px 12px;
  margin: 12px 0;
  background: rgba(175, 184, 193, 0.12);
}

.preview :deep(.md-toc .md-toc-list) {
  list-style: none;
  padding-left: 0;
  margin: 0;
}

.preview :deep(.md-toc .md-toc-list .md-toc-list) {
  padding-left: 16px;
  margin-top: 6px;
}

.preview :deep(.md-toc-item) {
  margin: 6px 0;
  line-height: 1.35;
}

.preview :deep(a.md-toc-link) {
  color: var(--guaxi-text-link, var(--primary-color, #4acadb));
  text-decoration: none;
  cursor: pointer;
}

.preview :deep(a.md-toc-link:hover) {
  text-decoration: underline;
}

.dark-mode .preview :deep(.md-toc),
.dark .preview :deep(.md-toc) {
  border-color: #30363d;
  background: rgba(110, 118, 129, 0.12);
}

.dark-mode .preview :deep(a.md-toc-link),
.dark .preview :deep(a.md-toc-link) {
  color: var(--guaxi-text-link-dark, #4acadb);
}

:deep(.monaco-editor .guaxi-link-like),
:deep(.monaco-editor .guaxi-mdlink-target) {
  color: var(--guaxi-text-link, var(--primary-color, #4acadb)) !important;
  text-decoration: underline !important;
  text-underline-offset: 2px;
  cursor: pointer;
}

:deep(.monaco-editor .guaxi-mdlink-text),
:deep(.monaco-editor .guaxi-wikilink-text) {
  color: var(--guaxi-mdlink-text-color, #c99016) !important;
  text-decoration: none !important;
}

:deep(.monaco-editor .guaxi-mdlink-bracket),
:deep(.monaco-editor .guaxi-mdlink-paren),
:deep(.monaco-editor .guaxi-mdlink-angle),
:deep(.monaco-editor .guaxi-wikilink-bracket) {
  color: #f35f4c !important;
  text-decoration: none !important;
}

:deep(.monaco-editor .guaxi-mdref-target) {
  color: var(--guaxi-text-link, var(--primary-color, #0969da)) !important;
  text-decoration: none !important;
}

:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-mdref-target) {
  color: var(--guaxi-text-link-dark, #4acadb) !important;
}

:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-link-like),
:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-mdlink-target) {
  color: var(--guaxi-text-link-dark, #4acadb) !important;
}

:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-mdlink-text),
:root[data-guaxi-theme='dark'] :deep(.monaco-editor .guaxi-wikilink-text) {
  color: var(--guaxi-mdlink-text-color-dark, #c678dd) !important;
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

.dark .tab-bar {
  background: var(--dark-toolbar-bg, #1e1e1e);
  border-bottom-color: var(--dark-divider-color, #333);
}

.dark .tab-region-right {
  border-left-color: var(--dark-divider-color, #333);
}

.dark .tab-item {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.08);
}

.dark .tab-item:hover {
  background: rgba(74, 163, 255, 0.18);
  border-color: rgba(74, 163, 255, 0.3);
  box-shadow: 0 1px 6px rgba(0, 0, 0, 0.35);
  transform: translateY(-1px);
}

.dark .tab-item.active {
  background: rgba(24, 144, 255, 0.28);
  border-color: rgba(74, 163, 255, 0.45);
  box-shadow: 0 1px 8px rgba(0, 0, 0, 0.4);
}

.dark .tab-item.active:hover {
  background: rgba(24, 144, 255, 0.32);
  border-color: rgba(74, 163, 255, 0.55);
}

.tab-bar {
  display: flex;
  height: 38px;
  min-height: 38px;
  flex: 0 0 38px;
  flex-shrink: 0;
  align-items: center;
  background: var(--guaxi-header-bg);
  border-bottom: 1px solid var(--guaxi-border);
  overflow: hidden;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.tab-region {
  display: flex;
  flex: 1 1 0;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
  padding: 4px 8px;
  gap: 6px;
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
  height: 28px;
  padding: 0 12px;
  border-radius: 8px;
  border: 1px solid rgba(0, 0, 0, 0.08);
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
  flex-shrink: 0;
  color: var(--guaxi-text-primary);
  background: rgba(128, 128, 128, 0.08);
  transition: background 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease, transform 0.15s ease;
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
  border-radius: 6px;
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
  background: rgba(24, 144, 255, 0.18);
  border-color: rgba(24, 144, 255, 0.35);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  color: var(--guaxi-text-primary);
}

.tab-item:hover {
  background: rgba(127, 127, 127, 0.12);
  border-color: rgba(0, 0, 0, 0.14);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  transform: translateY(-1px);
}

.tab-item.active:hover {
  background: rgba(24, 144, 255, 0.22);
  border-color: rgba(24, 144, 255, 0.45);
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

.n-dropdown-menu {
  max-height: 320px;
  max-height: min(320px, calc(100vh - 140px));
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(140, 140, 140, 0.45) transparent;
}

.n-dropdown-menu::-webkit-scrollbar {
  width: 8px;
}

.n-dropdown-menu::-webkit-scrollbar-track {
  background: transparent;
}

.n-dropdown-menu::-webkit-scrollbar-thumb {
  background: rgba(140, 140, 140, 0.35);
  border-radius: 8px;
  border: 2px solid transparent;
  background-clip: content-box;
}

.n-dropdown-menu::-webkit-scrollbar-thumb:hover {
  background: rgba(140, 140, 140, 0.55);
  background-clip: content-box;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: #ffffff;
  border: 1px solid var(--guaxi-border, #e1e4e8);
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  overflow: auto;
  color: #1f2328;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
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

.preview pre:not(.hljs)>code {
  display: block;
  border-radius: 6px;
  border: 1px solid #e1e4e8;
  background: #ffffff;
  color: #24292f;
  padding: 1.5em 1em;
  overflow-x: auto;
}

.dark-mode .preview pre:not(.hljs)>code {
  border-color: #30363d;
  background: #1f242d;
  color: #c9d1d9;
}

.preview :not(pre)>code:not(.hljs) {
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

.dark-mode .preview :not(pre)>code:not(.hljs) {
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
