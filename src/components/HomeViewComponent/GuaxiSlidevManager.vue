<template>
  <div class="slidev-page">
    <div class="sider-toggle sider-toggle-left" :class="{ collapsed: leftCollapsed }"
      :style="{ left: (leftCollapsed ? 0 : leftSiderWidth) + 'px' }">
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button size="small" quaternary circle @click="toggleLeftCollapsed">
            <template #icon>
              <n-icon :component="leftCollapsed ? ChevronForwardOutline : ChevronBackOutline" />
            </template>
          </n-button>
        </template>
        {{ leftCollapsed ? '展开左侧' : '折叠左侧' }}
      </n-tooltip>
    </div>

    <div class="sider-toggle sider-toggle-right" :class="{ collapsed: rightCollapsed }"
      :style="{ right: (rightCollapsed ? 0 : rightSiderWidth) + 'px' }">
      <n-tooltip trigger="hover">
        <template #trigger>
          <n-button size="small" quaternary circle @click="toggleRightCollapsed">
            <template #icon>
              <n-icon :component="rightCollapsed ? ChevronBackOutline : ChevronForwardOutline" />
            </template>
          </n-button>
        </template>
        {{ rightCollapsed ? '展开右侧' : '折叠右侧' }}
      </n-tooltip>
    </div>

    <n-layout class="slidev-layout" has-sider>
      <n-layout-sider v-model:collapsed="leftCollapsed"
        :class="['slidev-sider', 'slidev-sider-left', { 'is-collapsed': leftCollapsed }]" :width="leftSiderWidth"
        bordered collapse-mode="width" :collapsed-width="0" :native-scrollbar="false"
        content-style="padding: 0; height: 100%; display: flex; flex-direction: column; overflow: hidden;"
        style="height: 100%; position: relative;">
        <div ref="leftPanelsRef" class="left-panels">
          <div class="panel left-panel-top" :style="{ height: `${leftPanelTopHeight}px` }">
            <div class="panel-header">
              <div class="panel-header-row">
                <div class="panel-title">PPT 源文件</div>
                <n-space size="small" align="center">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="tiny" quaternary circle @click="refreshSlidevRoot">
                        <template #icon>
                          <n-icon :component="RefreshOutline" />
                        </template>
                      </n-button>
                    </template>
                    刷新
                  </n-tooltip>

                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="tiny" quaternary circle @click="openCreateModal('folder')">
                        <template #icon>
                          <n-icon :component="AddOutline" />
                        </template>
                      </n-button>
                    </template>
                    新建 PPT 目录
                  </n-tooltip>

                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="tiny" quaternary circle @click="openCreateModal('file')">
                        <template #icon>
                          <n-icon :component="DocumentTextOutline" />
                        </template>
                      </n-button>
                    </template>
                    新建文件
                  </n-tooltip>

                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="tiny" quaternary circle type="error" @click="deleteSelected">
                        <template #icon>
                          <n-icon :component="TrashOutline" />
                        </template>
                      </n-button>
                    </template>
                    删除
                  </n-tooltip>
                </n-space>
              </div>

              <n-input v-model:value="slidevFilter" size="small" clearable placeholder="过滤 PPT 文件...">
                <template #prefix>
                  <n-icon :component="SearchOutline" />
                </template>
              </n-input>
            </div>

            <div class="panel-body">
              <n-spin :show="slidevTreeLoading">
                <div class="tree-host">
                  <GuaxiSlidevTreeView :nodes="filteredSlidevTree" v-model:selected-key="slidevSelectedKey"
                    v-model:expanded-keys="slidevExpandedKeys" :load-children="handleSlidevNodeLoad"
                    empty-text="暂无 PPT 文件" @select="handleSlidevSelectNode"
                    @contextmenu="(payload) => handleTreeContextMenu('slidev', payload)" />
                </div>
              </n-spin>
            </div>

          </div>

          <div class="left-panel-splitter" @mousedown="onMouseDownLeftPanelSplit" />

          <div class="panel left-panel-bottom">
            <div class="panel-header">
              <div class="panel-header-row">
                <div class="panel-title">知识库素材</div>
                <n-space size="small" align="center">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="tiny" quaternary circle @click="loadKbTree">
                        <template #icon>
                          <n-icon :component="RefreshOutline" />
                        </template>
                      </n-button>
                    </template>
                    加载目录树
                  </n-tooltip>

                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="tiny" quaternary circle @click="clearMaterial">
                        <template #icon>
                          <n-icon :component="TrashOutline" />
                        </template>
                      </n-button>
                    </template>
                    清空选中
                  </n-tooltip>
                </n-space>
              </div>

              <n-select v-model:value="selectedKbPath" size="small" :options="kbOptions" filterable
                placeholder="选择知识库..." style="width: 100%;" />
              <n-input v-model:value="kbFilter" size="small" clearable placeholder="过滤文件名...">
                <template #prefix>
                  <n-icon :component="SearchOutline" />
                </template>
              </n-input>
            </div>

            <div class="panel-body panel-body-scroll">
              <n-spin :show="kbSpinLoading">
                <div class="tree-host">
                  <GuaxiSlidevTreeView :nodes="filteredKbTree" v-model:selected-key="kbSelectedKey"
                    v-model:expanded-keys="kbExpandedKeys" empty-text="暂无知识库文件" :show-loading="false"
                    @select="handleKbSelectNode" @contextmenu="(payload) => handleTreeContextMenu('kb', payload)" />
                </div>
              </n-spin>
            </div>
          </div>
        </div>
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSider" />
      </n-layout-sider>

      <n-layout-content class="slidev-center" :native-scrollbar="false"
        content-style="height: 100%; min-height: 0; display: flex; flex-direction: column; overflow: hidden;">
        <div class="editor-header">
          <div class="tab-bar" ref="tabBarRef" @wheel="onTabBarWheel">
            <div v-for="tab in slidevEditorStore.openTabs" :key="tab.path"
              :class="['tab-item', { active: normalizePath(tab.path) === activeEditorPath, dirty: !!tab.dirty }]"
              @click="setActiveTab(tab.path)" @mousedown.middle="closeTab(tab.path)">
              <span class="tab-title">{{ tab.fileName }}</span>
              <span class="tab-affordance">
                <span class="tab-dirty-dot" v-show="tab.dirty" />
                <n-icon class="tab-close" size="14" @click.stop="closeTab(tab.path)">
                  <CloseOutline />
                </n-icon>
              </span>
            </div>
          </div>

          <div class="toolbar">
            <div class="toolbar-left">
              <div class="toolbar-editing-actions">
                <n-dropdown trigger="click" :options="headingOptions" @select="handleHeadingSelect">
                  <n-tooltip placement="top" trigger="hover">
                    <template #trigger>
                      <n-button text size="tiny" :disabled="!canEdit">
                        <Icon :icon="headingIcon" width="16" height="16" />
                      </n-button>
                    </template>
                    标题
                  </n-tooltip>
                </n-dropdown>

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
                    <n-button text size="tiny" :disabled="!canEdit" @click="toggleBlockquote">
                      <Icon :icon="quoteIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  引用
                </n-tooltip>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="toggleInlineCode">
                      <Icon :icon="codeIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  行内代码
                </n-tooltip>
                <n-dropdown trigger="click" :options="codeBlockOptions" @select="handleCodeBlockSelect">
                  <n-tooltip placement="top" trigger="hover">
                    <template #trigger>
                      <n-button text size="tiny" :disabled="!canEdit">
                        <Icon :icon="squareTerminalIcon" width="16" height="16" />
                      </n-button>
                    </template>
                    代码块
                  </n-tooltip>
                </n-dropdown>

                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="toggleUnorderedList">
                      <Icon :icon="listIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  无序列表
                </n-tooltip>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="toggleOrderedList">
                      <Icon :icon="listOrderedIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  有序列表
                </n-tooltip>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="toggleTaskList">
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
                        <n-button text size="tiny" :disabled="!canEdit">
                          <Icon :icon="tableIcon" width="16" height="16" />
                        </n-button>
                      </template>
                      表格 (Ctrl+Alt+T)
                    </n-tooltip>
                  </template>

                  <div class="table-insert-popover" @mousedown.stop>
                    <div class="table-insert-hint">{{ tableInsertHint }}</div>
                    <div class="table-insert-grid" @mouseup="handleTableGridMouseUp"
                      @mouseleave="handleTableGridMouseLeave">
                      <template v-for="r in tableGridMaxRows" :key="`r-${r}`">
                        <template v-for="c in tableGridMaxCols" :key="`c-${r}-${c}`">
                          <div class="table-insert-cell" :class="{ active: r <= tableGridRows && c <= tableGridCols }"
                            @mousedown.prevent="handleTableGridMouseDown(r, c)"
                            @mouseover="handleTableGridMouseOver(r, c)" />
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
                        <n-button size="tiny" type="primary" :disabled="!canEdit"
                          @click="insertTableFromInputs">插入</n-button>
                      </div>
                    </div>
                  </div>
                </n-popover>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="insertImage">
                      <Icon :icon="imageIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  图片
                </n-tooltip>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="insertHorizontalRule">
                      <Icon :icon="minusIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  分隔线
                </n-tooltip>

                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="undo">
                      <Icon :icon="undoIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  撤销
                </n-tooltip>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button text size="tiny" :disabled="!canEdit" @click="redo">
                      <Icon :icon="redoIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  重做
                </n-tooltip>
              </div>

              <div class="toolbar-spacer"></div>

              <n-space size="small" align="center" :wrap="false" class="toolbar-primary-actions">
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle @click="saveActive">
                      <template #icon>
                        <Icon :icon="saveIcon" width="16" height="16" />
                      </template>
                    </n-button>
                  </template>
                  保存
                </n-tooltip>

                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle @click="previewSlidev">
                      <template #icon>
                        <Icon :icon="eyeIcon" width="16" height="16" />
                      </template>
                    </n-button>
                  </template>
                  浏览器预览
                </n-tooltip>

                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle @click="presentSlidev">
                      <template #icon>
                        <Icon :icon="playCircleIcon" width="16" height="16" />
                      </template>
                    </n-button>
                  </template>
                  演示
                </n-tooltip>

                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle @click="exportPptx">
                      <template #icon>
                        <Icon :icon="downloadIcon" width="16" height="16" />
                      </template>
                    </n-button>
                  </template>
                  导出 PPTX
                </n-tooltip>

                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle :type="devRunning ? 'error' : 'default'"
                      :class="{ 'stop-active': devRunning }" @click="stopPreview">
                      <template #icon>
                        <Icon :icon="stopCircleIcon" width="16" height="16" />
                      </template>
                    </n-button>
                  </template>
                  {{ stopActionLabel }}
                </n-tooltip>

                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle :disabled="!outlineAvailable" @click="toggleOutline">
                      <Icon :icon="listIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  大纲
                </n-tooltip>
                <n-tooltip placement="top" trigger="hover">
                  <template #trigger>
                    <n-button size="small" quaternary circle :disabled="!canEdit" @click="captureScreenshot">
                      <Icon :icon="cameraIcon" width="16" height="16" />
                    </n-button>
                  </template>
                  截图
                </n-tooltip>
                <n-dropdown trigger="click" :options="previewModeOptions" @select="handlePreviewModeSelect">
                  <n-tooltip placement="top" trigger="hover">
                    <template #trigger>
                      <n-button size="small" quaternary circle>
                        <Icon :icon="previewModeIcon" width="16" height="16" />
                      </n-button>
                    </template>
                    {{ previewModeLabel }}
                  </n-tooltip>
                </n-dropdown>
              </n-space>
            </div>

            <div class="toolbar-right">
              <n-input v-model:value="devPortText" size="small" placeholder="端口(默认3030)" style="width: 70px" />
            </div>
          </div>
        </div>

        <div class="editor-content" :class="previewMode">
          <div v-if="!activeEditorPath" class="editor-empty">请选择一个 PPT 源文件以开始编辑</div>
          <div v-show="activeEditorPath && showEditor" class="editor-pane">
            <GuaxiSlidevLiteEditor ref="pptEditorRef" :active-path="activeEditorPath" :content="activeEditorContent"
              :unsupported="activeEditorUnsupported" :slidev-root="slidevRoot" @draft-flush="onEditorDraftFlush"
              @save-request="onEditorSaveRequest" />
          </div>
          <div v-show="activeEditorPath && showPreview" class="preview-pane">
            <div v-if="previewLoading" class="preview-loading">正在启动 Slidev 预览...</div>
            <div v-else-if="!previewUrl" class="preview-empty">
              <n-button size="small" @click="startPreviewInApp">启动预览</n-button>
            </div>
            <iframe v-else class="preview-iframe" :src="previewUrl" />
          </div>
        </div>
      </n-layout-content>

      <n-layout-sider v-model:collapsed="rightCollapsed"
        :class="['slidev-sider', 'slidev-sider-right', { 'is-collapsed': rightCollapsed }]" :width="rightSiderWidth"
        bordered position="right" collapse-mode="width" :collapsed-width="0" :native-scrollbar="false"
        content-style="padding: 0; height: 100%; display: flex; flex-direction: column; overflow: hidden;"
        style="height: 100%; position: relative;">
        <div class="panel">
          <div class="panel-header">
            <div class="panel-header-row">
              <div class="panel-title-icons">
                <n-button text size="tiny" :class="['panel-mode-btn', { active: rightPanelMode === 'material' }]"
                  @click="setRightPanelMode('material')">
                  素材
                </n-button>
                <n-button text size="tiny" :disabled="!outlineAvailable"
                  :class="['panel-mode-btn', { active: rightPanelMode === 'outline' }]"
                  @click="setRightPanelMode('outline')">
                  大纲
                </n-button>
              </div>
              <n-space size="small" align="center">
                <n-tooltip v-if="rightPanelMode === 'material'" trigger="hover">
                  <template #trigger>
                    <n-button size="tiny" quaternary circle :disabled="!material.path || material.loading"
                      @click="insertMaterialFile">
                      <template #icon>
                        <Icon :icon="plusIcon" width="16" height="16" />
                      </template>
                    </n-button>
                  </template>
                  插入整篇
                </n-tooltip>
              </n-space>
            </div>
          </div>

          <div class="panel-body" :class="{ 'panel-body-scroll': rightPanelMode === 'material' }">
            <div v-if="rightPanelMode === 'material'">
              <n-spin :show="material.loading">
                <div v-if="materialCards.length === 0" class="panel-empty">
                  {{ material.path ? '素材文件为空' : '请选择一个素材文件' }}
                </div>
                <div v-else class="kb-card-list" ref="materialCardsRef">
                  <n-card v-for="(card, index) in materialCards" :key="card.id" size="small" class="kb-card"
                    :class="{ 'kb-card-active': card.id === activeMaterialCardId }"
                    @click="handleMaterialCardClick(card)">
                    <template #header>
                      <span class="kb-card-header-spacer"></span>
                    </template>
                    <template #header-extra>
                      <n-tooltip trigger="hover">
                        <template #trigger>
                          <n-button size="tiny" quaternary circle @click="insertMaterialCard(card, index)">
                            <template #icon>
                              <n-icon :component="AddOutline" />
                            </template>
                          </n-button>
                        </template>
                        插入到PPT
                      </n-tooltip>
                    </template>
                    <div class="kb-card-content" v-html="card.html"></div>
                  </n-card>
                </div>
              </n-spin>
            </div>
            <div v-else class="outline-panel-host">
              <GuaxiOutline />
            </div>
          </div>
        </div>
        <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSider" />
      </n-layout-sider>
    </n-layout>

    <n-dropdown v-model:show="treeContextMenuVisible" :options="treeContextMenuOptions" placement="bottom-start"
      :x="treeContextMenuPosition.x" :y="treeContextMenuPosition.y" @select="handleTreeContextMenuSelect"
      @clickoutside="closeTreeContextMenu" />

    <n-modal v-model:show="createModalVisible" preset="dialog" :title="createModalTitle" :show-icon="false">
      <n-space vertical size="small">
        <n-input v-model:value="createName" placeholder="名称（文件可带 .md）" />
        <n-text depth="3" class="path-text" :title="createTargetDir">目标目录：{{ createTargetDir }}</n-text>
      </n-space>
      <template #action>
        <n-space>
          <n-button size="small" @click="createModalVisible = false">取消</n-button>
          <n-button size="small" type="primary" :disabled="!createName.trim()" @click="confirmCreate">创建</n-button>
        </n-space>
      </template>
    </n-modal>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, reactive, ref, nextTick, watch, h } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import { confirm as confirmDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import {
  useMessage,
  NLayout,
  NLayoutSider,
  NLayoutContent,
  NSpin,
  NButton,
  NSpace,
  NInput,
  NInputNumber,
  NSelect,
  NDivider,
  NCard,
  NModal,
  NText,
  NIcon,
  NTooltip,
  NDropdown,
  NPopover
} from 'naive-ui'
import { createLogger } from '../../utils/logger'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import mermaid from 'mermaid'
import { useSlidevEditorStore } from '../../stores/slidevEditorStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import {
  AddOutline,
  ChevronBackOutline,
  ChevronForwardOutline,
  CloseOutline,
  DocumentTextOutline,
  FolderOutline,
  RefreshOutline,
  SearchOutline,
  TrashOutline
} from '@vicons/ionicons5'
import { Icon } from '@iconify/vue'
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
import filePenLineIcon from '@iconify-icons/lucide/file-edit'
import fileTextIcon from '@iconify-icons/lucide/file-text'
import eyeIcon from '@iconify-icons/lucide/eye'
import saveIcon from '@iconify-icons/lucide/save'
import playCircleIcon from '@iconify-icons/lucide/play-circle'
import downloadIcon from '@iconify-icons/lucide/download'
import stopCircleIcon from '@iconify-icons/lucide/stop-circle'
import cameraIcon from '@iconify-icons/lucide/camera'
import plusIcon from '@iconify-icons/lucide/plus'
import columns2Icon from '@iconify-icons/lucide/columns'
import GuaxiSlidevLiteEditor from './GuaxiSlidevLiteEditor.vue'
import GuaxiSlidevTreeView from './GuaxiSlidevTreeView.vue'
import GuaxiOutline from './GuaxiOutline.vue'

const logger = createLogger('GuaxiSlidevManager')
const message = useMessage()

let lastCenterLayoutLogAt = 0
const logCenterLayout = (stage, extra = {}) => {
  const now = Date.now()
  if (now - lastCenterLayoutLogAt < 250) return
  lastCenterLayoutLogAt = now

  try {
    const centerEl = document.querySelector('.slidev-center')
    const hostEl = document.querySelector('.editor-host')
    const centerRect = centerEl?.getBoundingClientRect?.() || { width: -1, height: -1, top: -1 }
    const hostRect = hostEl?.getBoundingClientRect?.() || { width: -1, height: -1, top: -1 }

    const centerStyle = centerEl ? window.getComputedStyle(centerEl) : null
    const hostStyle = hostEl ? window.getComputedStyle(hostEl) : null

    logger.info('[layout-center]', {
      stage,
      centerRect: { w: Math.round(centerRect.width), h: Math.round(centerRect.height), top: Math.round(centerRect.top) },
      centerCss: centerStyle ? { height: centerStyle.height, display: centerStyle.display, flex: centerStyle.flex, minHeight: centerStyle.minHeight, overflow: centerStyle.overflow } : null,
      hostRect: { w: Math.round(hostRect.width), h: Math.round(hostRect.height), top: Math.round(hostRect.top) },
      hostCss: hostStyle ? { height: hostStyle.height, display: hostStyle.display, flex: hostStyle.flex, minHeight: hostStyle.minHeight, overflow: hostStyle.overflow } : null,
      viewport: { w: window.innerWidth, h: window.innerHeight },
      ...extra,
    })
  } catch (e) {
    logger.warn('[layout-center] log failed', e)
  }
}

const slidevEditorStore = useSlidevEditorStore()
const knowledgeBaseStore = useKnowledgeBaseStore()

const pptEditorRef = ref(null)
const tabBarRef = ref(null)

const previewMode = ref('edit')
const previewUrl = ref('')
const previewLoading = ref(false)
const previewEntryPath = ref('')
const previewPort = ref(0)

const devRunning = ref(false)
const devMode = ref('')

const stopActionLabel = computed(() => {
  if (!devRunning.value) return '停止预览'
  return devMode.value === 'present' ? '停止演示' : '停止预览'
})

const previewModeOptions = [
  { label: '编辑模式', key: 'edit', icon: () => h(Icon, { icon: filePenLineIcon, width: 16, height: 16 }) },
  { label: '分栏预览', key: 'split', icon: () => h(Icon, { icon: columns2Icon, width: 16, height: 16 }) },
  { label: '预览模式', key: 'preview', icon: () => h(Icon, { icon: eyeIcon, width: 16, height: 16 }) }
]

const previewModeLabel = computed(() => {
  if (previewMode.value === 'split') return '分栏预览'
  if (previewMode.value === 'preview') return '预览模式'
  return '编辑模式'
})

const previewModeIcon = computed(() => {
  if (previewMode.value === 'split') return columns2Icon
  if (previewMode.value === 'preview') return eyeIcon
  return filePenLineIcon
})

const handlePreviewModeSelect = (key) => {
  const mode = String(key || '')
  if (!mode) return
  setPreviewMode(mode)
}

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

const normalizePath = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$/g, '')

const isEditableTarget = (el) => {
  if (!el || !(el instanceof HTMLElement)) return false
  const tag = el.tagName ? el.tagName.toLowerCase() : ''
  if (tag === 'input' || tag === 'textarea' || tag === 'select') return true
  return !!el.isContentEditable
}

const getTabByPath = (path) => {
  const p = normalizePath(path)
  if (!p) return null
  return (Array.isArray(slidevEditorStore.openTabs) ? slidevEditorStore.openTabs : []).find(
    (t) => normalizePath(t?.path) === p
  ) || null
}

const activeEditorPath = computed(() => {
  const p = slidevEditorStore.activeTab || slidevEditorStore.selectedFile?.path || ''
  return normalizePath(p)
})

const activeEditorContent = computed(() => {
  const p = activeEditorPath.value
  if (!p) return ''
  const tab = getTabByPath(p)
  return String(tab?.content ?? slidevEditorStore.selectedFile?.content ?? '')
})

const activeEditorUnsupported = computed(() => {
  const p = activeEditorPath.value
  if (!p) return false
  const tab = getTabByPath(p)
  return !!tab?.unsupported || !!slidevEditorStore.selectedFile?.unsupported
})

const canEdit = computed(() => !!activeEditorPath.value && !activeEditorUnsupported.value)

const outlineAvailable = computed(() => {
  const v = pptEditorRef.value?.outlineHasContent
  if (typeof v === 'boolean') return v
  if (v && typeof v.value !== 'undefined') return !!v.value
  return false
})

const showEditor = computed(() => previewMode.value !== 'preview')
const showPreview = computed(() => previewMode.value !== 'edit')

const onEditorDraftFlush = (payload) => {
  const path = normalizePath(payload?.path)
  if (!path) return
  const content = String(payload?.content ?? '')
  slidevEditorStore.applyDraft(path, content)
  logger.info('[Slidev] draft flush', { path, len: content.length, tag: payload?.tag })
}

const onEditorSaveRequest = () => {
  logger.info('[Slidev] save request from editor')
  saveActive()
}

const setRightPanelMode = (mode) => {
  const next = mode === 'outline' ? 'outline' : 'material'
  rightPanelMode.value = next
  if (rightCollapsed.value) {
    rightCollapsed.value = false
    logger.info('[UI] auto expand right sider', { mode: next })
  }
}

const toggleOutline = () => {
  if (!outlineAvailable.value) {
    message.warning('当前文件无大纲')
    return
  }
  setRightPanelMode('outline')
}

const captureScreenshot = () => runEditorMethod('captureScreenshot')

const runEditorMethod = (method, ...args) => {
  if (!canEdit.value) {
    message.warning('请先打开一个 PPT 源文件')
    return
  }
  const api = pptEditorRef.value
  const fn = api?.[method]
  if (typeof fn !== 'function') {
    logger.warn('[toolbar] editor method missing', { method })
    return
  }
  fn(...args)
  logger.info('[toolbar] action', { method, args })
}

const handleHeadingSelect = (key) => {
  const level = Number(key)
  if (Number.isNaN(level)) return
  runEditorMethod('toggleHeading', level)
}

const toggleBold = () => runEditorMethod('toggleBold')
const toggleItalic = () => runEditorMethod('toggleItalic')
const toggleStrikethrough = () => runEditorMethod('toggleStrikethrough')
const toggleInlineCode = () => runEditorMethod('toggleInlineCode')
const insertLink = () => runEditorMethod('insertLink')
const toggleBlockquote = () => runEditorMethod('toggleBlockquote')
const toggleUnorderedList = () => runEditorMethod('toggleUnorderedList')
const toggleOrderedList = () => runEditorMethod('toggleOrderedList')
const toggleTaskList = () => runEditorMethod('toggleTaskList')
const insertImage = () => runEditorMethod('insertImage')
const insertHorizontalRule = () => runEditorMethod('insertHorizontalRule')
const undo = () => runEditorMethod('undo')
const redo = () => runEditorMethod('redo')

const handleCodeBlockSelect = (key) => {
  const k = String(key || '').trim()
  const lang = k === 'plain' ? '' : k
  runEditorMethod('insertCodeBlock', lang)
}

const clampInt = (value, min, max) => {
  const n = Number(value)
  if (!Number.isFinite(n)) return min
  return Math.max(min, Math.min(max, Math.floor(n)))
}

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

const insertTableWithSize = (rows, cols) => {
  const r = clampInt(rows, 2, tableInsertMaxRows)
  const c = clampInt(cols, 1, tableInsertMaxCols)
  runEditorMethod('insertTable', r, c)
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
  insertTableWithSize(tableInputRows.value, tableInputCols.value)
  tableInsertPopoverVisible.value = false
}

const setActiveTab = (path) => {
  slidevEditorStore.setActiveTab(path)
}

const closeTab = (path) => {
  const p = normalizePath(path)
  const tabs = Array.isArray(slidevEditorStore.openTabs) ? slidevEditorStore.openTabs : []
  const idx = tabs.findIndex((t) => normalizePath(t?.path) === p)
  if (idx < 0) return

  const wasActive = normalizePath(slidevEditorStore.activeTab) === p
  tabs.splice(idx, 1)

  if (wasActive) {
    const next = tabs[idx] || tabs[idx - 1]
    slidevEditorStore.setActiveTab(next?.path || '')
  }

  logger.info('[Slidev] tab closed', { path: p, remain: tabs.length })
}

const onTabBarWheel = (e) => {
  const el = tabBarRef.value
  if (!el) return
  el.scrollLeft += e.deltaY
}

const setPreviewMode = (mode) => {
  previewMode.value = mode
  logger.info('[Slidev] preview mode', { mode })
  if (mode === 'edit') return

  window.setTimeout(() => {
    if (previewMode.value === 'edit') return
    void ensurePreview()
  }, 0)
}

const ensurePreview = async () => {
  const { path } = getActivePathAndContent()
  if (!path) return

  const port = parsePort()
  if (previewUrl.value && previewEntryPath.value === path && previewPort.value === port) return
  if (previewLoading.value) return

  previewLoading.value = true
  try {
    await saveActive()
    const info = await invoke('slidev_start_dev', { entryPath: path, port })
    const url = String(info?.url || '')
    if (!url) {
      message.error('未获取到预览地址')
      return
    }
    previewUrl.value = url
    previewEntryPath.value = path
    previewPort.value = port
    devRunning.value = true
    devMode.value = 'preview'
    logger.info('[Slidev] preview ready', { path, url, port, mode: devMode.value })
  } catch (e) {
    logger.error('[Slidev] preview init failed', e)
    message.error('启动预览失败（需要本机可用的 Node.js / npx）')
  } finally {
    previewLoading.value = false
  }
}

const isHiddenName = (name) => {
  const s = String(name || '').trim()
  return !s || s.startsWith('.')
}

const leftCollapsed = ref(false)
const rightCollapsed = ref(true)
const rightPanelMode = ref('material')

const leftPanelsRef = ref(null)
const leftPanelTopHeight = ref(360)

// 说明：与主窗口一致，左右侧栏支持拖拽调宽；这里不使用 CSS resize，避免不同平台表现不一致
const leftSiderWidth = ref(320)
const rightSiderWidth = ref(380)

const MIN_LEFT_SIDER_WIDTH = 260
const MAX_LEFT_SIDER_WIDTH = 560
const MIN_RIGHT_SIDER_WIDTH = 300
const MAX_RIGHT_SIDER_WIDTH = 760

const MIN_LEFT_PANEL_TOP = 220
const MIN_LEFT_PANEL_BOTTOM = 220

const clamp = (v, min, max) => Math.max(min, Math.min(max, v))

const withResizingCursor = (fn) => {
  const prevCursor = document.body.style.cursor
  const prevSelect = document.body.style.userSelect
  document.body.style.cursor = 'ew-resize'
  document.body.style.userSelect = 'none'

  try {
    fn()
  } finally {
    document.body.style.cursor = prevCursor
    document.body.style.userSelect = prevSelect
  }
}

const withResizingCursorVertical = (fn) => {
  const prevCursor = document.body.style.cursor
  const prevSelect = document.body.style.userSelect
  document.body.style.cursor = 'ns-resize'
  document.body.style.userSelect = 'none'

  try {
    fn()
  } finally {
    document.body.style.cursor = prevCursor
    document.body.style.userSelect = prevSelect
  }
}

const onMouseDownLeftSider = (event) => {
  if (leftCollapsed.value) return
  event.preventDefault()

  const startX = event.clientX
  const startWidth = leftSiderWidth.value

  let raf = 0
  const onMove = (e) => {
    const dx = e.clientX - startX
    const next = clamp(startWidth + dx, MIN_LEFT_SIDER_WIDTH, MAX_LEFT_SIDER_WIDTH)
    if (raf) cancelAnimationFrame(raf)
    raf = requestAnimationFrame(() => {
      leftSiderWidth.value = next
    })
  }

  const onUp = () => {
    if (raf) cancelAnimationFrame(raf)
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    logger.info('[UI] left sider resized', { width: leftSiderWidth.value })
  }

  withResizingCursor(() => {
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  })
}

const onMouseDownLeftPanelSplit = (event) => {
  event.preventDefault()

  const container = leftPanelsRef.value
  if (!container) return

  const rect = container.getBoundingClientRect()
  const startY = event.clientY
  const startHeight = leftPanelTopHeight.value || rect.height * 0.6
  const maxTop = Math.max(MIN_LEFT_PANEL_TOP, rect.height - MIN_LEFT_PANEL_BOTTOM)

  let raf = 0
  const onMove = (e) => {
    const dy = e.clientY - startY
    const next = clamp(startHeight + dy, MIN_LEFT_PANEL_TOP, maxTop)
    if (raf) cancelAnimationFrame(raf)
    raf = requestAnimationFrame(() => {
      leftPanelTopHeight.value = next
    })
  }

  const onUp = () => {
    if (raf) cancelAnimationFrame(raf)
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    logger.info('[UI] left panel split resized', { topHeight: leftPanelTopHeight.value })
  }

  withResizingCursorVertical(() => {
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  })
}

const onMouseDownRightSider = (event) => {
  if (rightCollapsed.value) return
  event.preventDefault()

  const startX = event.clientX
  const startWidth = rightSiderWidth.value

  let raf = 0
  const onMove = (e) => {
    const dx = startX - e.clientX
    const next = clamp(startWidth + dx, MIN_RIGHT_SIDER_WIDTH, MAX_RIGHT_SIDER_WIDTH)
    if (raf) cancelAnimationFrame(raf)
    raf = requestAnimationFrame(() => {
      rightSiderWidth.value = next
    })
  }

  const onUp = () => {
    if (raf) cancelAnimationFrame(raf)
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    logger.info('[UI] right sider resized', { width: rightSiderWidth.value })
  }

  withResizingCursor(() => {
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  })
}

onBeforeUnmount(() => {
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  stopKbPolling()
})

const toggleLeftCollapsed = async () => {
  leftCollapsed.value = !leftCollapsed.value
  logger.info('[UI] toggle left collapsed', { collapsed: leftCollapsed.value })

  if (!leftCollapsed.value) {
    await nextTick()
  }
}

const toggleRightCollapsed = async () => {
  rightCollapsed.value = !rightCollapsed.value
  logger.info('[UI] toggle right collapsed', { collapsed: rightCollapsed.value })

  if (!rightCollapsed.value) {
    await nextTick()
  }
}

const slidevFilter = ref('')

const slidevSelectedKey = computed({
  get: () => String(slidevSelectedKeys.value?.[0] || ''),
  set: (v) => {
    const k = String(v || '')
    slidevSelectedKeys.value = k ? [k] : []
  }
})

const kbExpandedKeys = ref([])
const kbSelectedKey = computed({
  get: () => String(kbSelectedKeys.value?.[0] || ''),
  set: (v) => {
    const k = String(v || '')
    kbSelectedKeys.value = k ? [k] : []
  }
})

const treeContextMenuVisible = ref(false)
const treeContextMenuPosition = ref({ x: 0, y: 0 })
const treeContextMenuOptions = ref([])
const treeContextMenuTarget = ref({ type: '', node: null })

const buildTreeContextMenuOptions = (type, node) => {
  const opts = []
  const isSlidev = type === 'slidev'
  const isRoot = isSlidev && normalizePath(node?.key) === normalizePath(slidevRoot.value)

  if (isSlidev) {
    opts.push({
      label: '新建文件',
      key: 'createFile',
      icon: () => h(NIcon, null, { default: () => h(DocumentTextOutline) })
    })
    opts.push({
      label: '新建文件夹',
      key: 'createFolder',
      icon: () => h(NIcon, null, { default: () => h(AddOutline) })
    })
    opts.push({ type: 'divider' })
    opts.push({
      label: '删除',
      key: 'delete',
      disabled: isRoot,
      icon: () => h(NIcon, null, { default: () => h(TrashOutline) })
    })
    opts.push({ type: 'divider' })
  }

  opts.push({
    label: '在文件资源管理器中打开',
    key: 'openInExplorer',
    icon: () => h(NIcon, null, { default: () => h(FolderOutline) })
  })

  return opts
}

const resolveExplorerPath = (node) => {
  const nodePath = normalizePath(node?.key)
  if (!nodePath) return ''
  if (node?.isDir) return nodePath
  const parts = nodePath.split('/')
  parts.pop()
  return parts.join('/')
}

const handleTreeContextMenu = (type, payload) => {
  const node = payload?.item
  if (!node) return

  treeContextMenuTarget.value = { type, node }
  treeContextMenuOptions.value = buildTreeContextMenuOptions(type, node)
  treeContextMenuPosition.value = { x: payload.x, y: payload.y }
  treeContextMenuVisible.value = true
  logger.info('[TreeContextMenu] open', { type, key: payload.key, isDir: payload.isDir })
}

const closeTreeContextMenu = () => {
  treeContextMenuVisible.value = false
}

const handleTreeContextMenuSelect = async (key) => {
  const { type, node } = treeContextMenuTarget.value || {}
  treeContextMenuVisible.value = false
  if (!node) return

  try {
    switch (key) {
      case 'createFile':
        if (type !== 'slidev') return
        openCreateModal('file')
        logger.info('[TreeContextMenu] create file')
        break
      case 'createFolder':
        if (type !== 'slidev') return
        openCreateModal('folder')
        logger.info('[TreeContextMenu] create folder')
        break
      case 'delete':
        if (type !== 'slidev') return
        await deleteSelected()
        break
      case 'openInExplorer':
        {
          const target = resolveExplorerPath(node)
          if (!target) return
          await invoke('open_in_file_explorer', { path: target })
          logger.info('[TreeContextMenu] open in explorer', { path: target })
        }
        break
    }
  } catch (e) {
    logger.error('[TreeContextMenu] action failed', { key, err: e })
    message.error('操作失败')
  }
}

const slidevRoot = ref('')
const slidevTreeLoading = ref(false)
const slidevTree = ref([])
const slidevExpandedKeys = ref([])
const slidevSelectedKeys = ref([])
const slidevOpeningKey = ref('')
const slidevLoadedDirKeys = ref(new Set())

const devPortText = ref('3030')

const createModalVisible = ref(false)
const createMode = ref('folder') // folder | file
const createName = ref('')

const createModalTitle = computed(() => (createMode.value === 'folder' ? '新建 PPT 目录' : '新建文件'))

const findTreeNode = (nodes, key) => {
  const list = Array.isArray(nodes) ? nodes : []
  for (const n of list) {
    if (n?.key === key) return n
    const hit = findTreeNode(n?.children, key)
    if (hit) return hit
  }
  return null
}

const getSelectedSlidevNode = () => {
  const key = slidevSelectedKeys.value?.[0]
  if (!key) return null
  return findTreeNode(slidevTree.value, key)
}

const ensureDirExists = async (dir) => {
  const p = normalizePath(dir)
  if (!p) return
  try {
    await invoke('create_directory', { path: p })
  } catch (e) {
    logger.warn('[Slidev] ensureDirExists failed', e)
  }
}

const mapFileNodes = (nodes) => {
  const list = Array.isArray(nodes) ? nodes : []
  return list
    .filter((n) => {
      const name = String(n?.name || '').trim()
      if (!name) return false
      if (isHiddenName(name)) return false
      if (name.toLowerCase() === 'node_modules') return false
      return true
    })
    .map((n) => {
      const key = normalizePath(n?.id)
      const isDir = !!n?.is_directory
      return {
        key,
        label: String(n?.name || ''),
        isDir,
        children: isDir ? [] : undefined,
        isLeaf: !isDir,
      }
    })
}

const loadSlidevChildren = async (dirPath) => {
  const p = normalizePath(dirPath)
  const raw = await invoke('get_file_tree', { path: p })
  return mapFileNodes(raw)
}

const withTimeout = async (p, ms, label) => {
  let timer
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`timeout:${label}`)), ms)
  })
  try {
    return await Promise.race([p, timeout])
  } finally {
    clearTimeout(timer)
  }
}

const refreshSlidevRoot = async () => {
  if (slidevTreeLoading.value) {
    logger.info('[Slidev] refresh skipped: already loading')
    return
  }

  const prevExpanded = Array.isArray(slidevExpandedKeys.value) ? slidevExpandedKeys.value : []
  const prevExpandedSet = new Set(prevExpanded.map(normalizePath).filter(Boolean))

  slidevTreeLoading.value = true
  try {
    const root = await withTimeout(invoke('get_slidev_repo_root'), 12000, 'get_slidev_repo_root')
    slidevRoot.value = normalizePath(root)
    await ensureDirExists(slidevRoot.value)

    const children = await withTimeout(loadSlidevChildren(slidevRoot.value), 12000, 'loadSlidevChildren')
    slidevTree.value = [
      {
        key: slidevRoot.value,
        label: slidevRoot.value.split('/').pop() || slidevRoot.value,
        isDir: true,
        children,
      },
    ]

    const nextExpanded = new Set([slidevRoot.value])
    prevExpandedSet.forEach((key) => {
      if (key && key.startsWith(slidevRoot.value)) nextExpanded.add(key)
    })

    slidevExpandedKeys.value = Array.from(nextExpanded)
    slidevLoadedDirKeys.value = new Set([slidevRoot.value])
    await restoreExpandedNodes(slidevExpandedKeys.value)

    logger.info('[Slidev] loaded root', {
      root: slidevRoot.value,
      count: children.length,
      expanded: slidevExpandedKeys.value.length
    })
  } catch (e) {
    logger.error('[Slidev] refresh failed', e)
    message.error('加载 PPT 源文件目录失败')
  } finally {
    slidevTreeLoading.value = false
  }
}

const restoreExpandedNodes = async (keys) => {
  const list = Array.isArray(keys) ? keys : []
  const normalized = list.map(normalizePath).filter(Boolean)
  const uniq = Array.from(new Set(normalized))
  if (uniq.length === 0) return

  const sorted = uniq.sort((a, b) => a.split('/').length - b.split('/').length)
  for (const key of sorted) {
    const node = findTreeNode(slidevTree.value, key)
    if (!node || !node.isDir) continue
    await handleSlidevNodeLoad(node)
  }

  logger.info('[Slidev] restore expanded', { count: sorted.length })
}

const handleSlidevNodeLoad = async (node) => {
  try {
    if (!node?.isDir) {
      logger.debug('[Slidev] onLoad skipped (not dir)', { key: node?.key })
      return Promise.resolve()
    }

    if (slidevLoadedDirKeys.value.has(node.key) && Array.isArray(node.children) && node.children.length > 0) {
      logger.debug('[Slidev] onLoad skipped (already loaded)', { key: node?.key })
      return Promise.resolve()
    }

    const children = await withTimeout(loadSlidevChildren(node.key), 12000, 'loadSlidevChildren')
    node.children = children
    slidevLoadedDirKeys.value.add(node.key)
    logger.info('[Slidev] loaded children', { dir: node.key, count: children.length })
    return Promise.resolve()
  } catch (e) {
    logger.error('[Slidev] load children failed', e)
    return Promise.resolve()
  }
}

const handleSlidevSelectNode = async (node) => {
  const key = normalizePath(node?.key)
  if (!key) return

  if (slidevOpeningKey.value === key) {
    logger.debug('[Slidev] select skipped: opening same key', { key })
    return
  }

  if (node?.isDir) return

  if (normalizePath(slidevEditorStore.activeTab) === key) {
    logger.debug('[Slidev] select skipped: already active', { key })
    return
  }

  const traceId = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`
  slidevOpeningKey.value = key

  const t0 = performance.now()
  logCenterLayout('select-start', { traceId, key })
  const label = String(node?.label ?? '')

  let watchdogTimer = 0
  try {
    logger.info('[Slidev] select start', { traceId, key, label })

    requestAnimationFrame(() => {
      logger.info('[Slidev] trace raf after click', { traceId, key, costMs: Math.round(performance.now() - t0) })
    })

    watchdogTimer = window.setTimeout(() => {
      logger.warn('[Slidev] trace watchdog (UI may be blocked)', {
        traceId,
        key,
        costMs: Math.round(performance.now() - t0),
      })
    }, 1500)

    await nextTick()
    await new Promise((resolve) => requestAnimationFrame(resolve))

    const tRead0 = performance.now()
    logger.info('[Slidev] read start', { traceId, key })
    logCenterLayout('read-start', { traceId, key })
    const res = await slidevEditorStore.fetchFileContent(key)
    const readCost = Math.round(performance.now() - tRead0)

    const len = typeof res?.content === 'string' ? res.content.length : -1
    logger.info('[Slidev] read done', {
      traceId,
      key,
      ok: !!res?.ok,
      unsupported: !!res?.unsupported,
      len,
      readCostMs: readCost,
      costMs: Math.round(performance.now() - t0)
    })

    if (len > 2 * 1024 * 1024) {
      logger.warn('[Slidev] large file detected', { traceId, key, len })
    }

    if (!res?.ok) {
      message.error('读取文件失败')
      return
    }

    const tOpen0 = performance.now()
    logger.info('[Slidev] openTab start', { traceId, key })
    logCenterLayout('openTab-start', { traceId, key })
    await slidevEditorStore.openFileInTab(key, res.content, { unsupported: !!res.unsupported })
    logger.info('[Slidev] openTab done', {
      traceId,
      key,
      openCostMs: Math.round(performance.now() - tOpen0),
      costMs: Math.round(performance.now() - t0)
    })
    logCenterLayout('openTab-done', { traceId, key })

    requestAnimationFrame(() => {
      logCenterLayout('raf-after-openTab', { traceId, key })
    })

    requestAnimationFrame(() => {
      logger.info('[Slidev] trace raf after openTab', { traceId, key, costMs: Math.round(performance.now() - t0) })
    })

    window.setTimeout(() => {
      logger.info('[Slidev] trace timeout after openTab', { traceId, key, costMs: Math.round(performance.now() - t0) })
    }, 0)

    if (res.unsupported) message.warning('该文件可能为二进制或非 UTF-8，无法编辑')
  } catch (e) {
    logger.error('[Slidev] open file failed', { traceId, key, err: e })
    message.error('打开文件失败')
  } finally {
    if (watchdogTimer) window.clearTimeout(watchdogTimer)
    slidevOpeningKey.value = ''
    logger.info('[Slidev] trace end', { traceId, key, costMs: Math.round(performance.now() - t0) })
  }
}

const createTargetDir = computed(() => {
  const root = slidevRoot.value
  const sel = getSelectedSlidevNode()
  if (!sel) return root
  if (sel.isDir) return sel.key
  const parts = normalizePath(sel.key).split('/')
  parts.pop()
  return parts.join('/') || root
})

const openCreateModal = (mode) => {
  createMode.value = mode === 'file' ? 'file' : 'folder'
  createName.value = createMode.value === 'file' ? 'slides.md' : ''
  createModalVisible.value = true
}

const defaultSlidevMarkdown = () => {
  return [
    '---',
    'title: New Slidev',
    '---',
    '',
    '# 标题',
    '',
    '---',
    '',
    '## 第二页',
    '',
  ].join('\n')
}

const makeUniqueName = async (dirPath, desired) => {
  const dir = normalizePath(dirPath)
  const want = String(desired || '').trim()
  if (!want) return want
  try {
    const raw = await invoke('get_file_tree', { path: dir })
    const names = new Set((Array.isArray(raw) ? raw : []).map((n) => String(n?.name || '').toLowerCase()))
    if (!names.has(want.toLowerCase())) return want

    const dot = want.lastIndexOf('.')
    const base = dot > 0 ? want.slice(0, dot) : want
    const ext = dot > 0 ? want.slice(dot) : ''
    for (let i = 2; i < 2000; i += 1) {
      const next = `${base} (${i})${ext}`
      if (!names.has(next.toLowerCase())) return next
    }
    return `${base} (${Date.now()})${ext}`
  } catch (e) {
    logger.warn('[Slidev] makeUniqueName failed', e)
    return want
  }
}

const confirmCreate = async () => {
  const dir = createTargetDir.value
  const name = createName.value.trim()
  if (!dir || !name) return

  try {
    if (createMode.value === 'folder') {
      const target = normalizePath(`${dir}/${name}`)
      logger.info('[Slidev] create folder', { target })
      await invoke('create_directory', { path: target })
    } else {
      const finalName = await makeUniqueName(dir, name)
      const target = normalizePath(`${dir}/${finalName}`)
      const content = finalName.toLowerCase().endsWith('.md') ? defaultSlidevMarkdown() : ''
      logger.info('[Slidev] create file', { target, len: content.length })
      await invoke('create_file_with_content', { path: target, content })
    }

    createModalVisible.value = false
    await refreshSlidevRoot()
    message.success('创建成功')
  } catch (e) {
    logger.error('[Slidev] create failed', e)
    message.error('创建失败')
  }
}

// 删除文件/目录后关闭其下已打开的编辑器标签：
// 不关闭的话，标签仍可保存，会把刚删掉的文件重新写回磁盘。
const closeEditorTabsUnderPath = (deletedPath) => {
  const root = normalizePath(deletedPath)
  if (!root) return
  const tabs = Array.isArray(slidevEditorStore.openTabs) ? [...slidevEditorStore.openTabs] : []
  const toClose = tabs.filter((tab) => {
    const p = normalizePath(tab?.path)
    return !!p && (p === root || p.startsWith(`${root}/`))
  })
  for (const tab of toClose) closeTab(tab.path)
  if (toClose.length) {
    logger.info('[Slidev] closed editor tabs for deleted path', {
      deletedPath: root,
      closed: toClose.length,
    })
  }
}

const deleteSelected = async () => {
  const sel = getSelectedSlidevNode()
  if (!sel) {
    message.warning('请先选择要删除的文件/目录')
    return
  }
  if (normalizePath(sel.key) === normalizePath(slidevRoot.value)) {
    message.warning('不允许删除根目录')
    return
  }

  const ok = await confirmDialog(`确定要删除「${sel.label}」吗？`, { title: '删除', kind: 'warning' })
  if (!ok) return

  try {
    await invoke('delete_path', { path: sel.key })
    closeEditorTabsUnderPath(sel.key)
    await refreshSlidevRoot()
    message.success('删除成功')
  } catch (e) {
    logger.error('[Slidev] delete failed', e)
    message.error('删除失败')
  }
}

const getActivePathAndContent = () => {
  const active = slidevEditorStore.activeTab || slidevEditorStore.selectedFile?.path
  const path = normalizePath(active)
  if (!path) return { path: '', content: '' }

  const tab = (Array.isArray(slidevEditorStore.openTabs) ? slidevEditorStore.openTabs : []).find(
    (t) => normalizePath(t?.path) === path
  )
  const content = String(tab?.content ?? slidevEditorStore.selectedFile?.content ?? '')
  const unsupported = !!tab?.unsupported || !!slidevEditorStore.selectedFile?.unsupported
  return { path, content, unsupported }
}

const saveActive = async () => {
  const { path, content, unsupported } = getActivePathAndContent()
  if (!path) {
    message.warning('请先打开一个 PPT 源文件')
    return
  }
  if (unsupported) {
    message.warning('不支持保存该文件类型')
    return
  }

  try {
    let finalContent = content
    let source = 'store'
    const editorPayload = pptEditorRef.value?.getContent?.()
    if (editorPayload?.ok && normalizePath(editorPayload.path) === path) {
      finalContent = String(editorPayload.content ?? '')
      source = 'editor'
    }

    logger.info('[Slidev] save', { path, len: finalContent.length, source })
    await slidevEditorStore.updateFileContent(path, finalContent)

    // 说明：保存成功后重置编辑器脏状态
    pptEditorRef.value?.markSaved?.(path)

    message.success('已保存')
  } catch (e) {
    logger.error('[Slidev] save failed', e)
    message.error('保存失败')
  }
}

const parsePort = () => {
  const n = Number(devPortText.value || 0)
  if (!Number.isFinite(n) || n <= 0) return 3030
  return Math.floor(n)
}

const startPreviewInApp = async () => {
  const { path } = getActivePathAndContent()
  if (!path) {
    message.warning('请先打开 slides.md（或任意 Slidev 源文件）')
    return
  }

  logger.info('[Slidev] start preview in app', { path })
  await ensurePreview()
}

const previewSlidev = async () => {
  const { path } = getActivePathAndContent()
  if (!path) {
    message.warning('请先打开 slides.md（或任意 Slidev 源文件）')
    return
  }

  const traceId = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`
  logger.info('[Slidev] open preview in browser', { traceId, path })

  await ensurePreview()
  const url = String(previewUrl.value || '')
  if (!url) {
    message.error('未获取到预览地址')
    logger.warn('[Slidev] open preview aborted: url empty', { traceId })
    return
  }

  try {
    logger.info('[Slidev] openUrl start', { traceId, url })
    await openUrl(url)
    logger.info('[Slidev] openUrl done', { traceId, url })
  } catch (err) {
    logger.error('[Slidev] openUrl failed', { traceId, url, err })
    message.error('打开浏览器失败')
  }
}

const presentSlidev = async () => {
  const { path } = getActivePathAndContent()
  if (!path) {
    message.warning('请先打开 slides.md（或任意 Slidev 源文件）')
    return
  }

  const traceId = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`
  const t0 = performance.now()

  try {
    await saveActive()
    const port = parsePort()
    logger.info('[Slidev] presenter start', { traceId, entryPath: path, port })

    const info = await invoke('slidev_start_dev', { entryPath: path, port })
    const base = String(info?.url || '')
    if (!base) {
      message.error('未获取到预览地址')
      return
    }

    const url = `${base}presenter`
    devRunning.value = true
    devMode.value = 'present'
    logger.info('[Slidev] presenter start_dev ok', { traceId, info, url, mode: devMode.value })

    try {
      logger.info('[Slidev] openUrl start', { traceId, url })
      await openUrl(url)
      logger.info('[Slidev] openUrl done', { traceId, url })
    } catch (err) {
      logger.error('[Slidev] openUrl failed', { traceId, url, err })
      message.error('打开浏览器失败')
    }
  } catch (e) {
    logger.error('[Slidev] presenter failed', { traceId, err: e })
    message.error('启动演示失败')
  } finally {
    logger.info('[Slidev] presenter end', { traceId, costMs: Math.round(performance.now() - t0) })
  }
}

const stopPreview = async () => {
  try {
    await invoke('slidev_stop_dev')
    previewUrl.value = ''
    previewEntryPath.value = ''
    previewPort.value = 0
    devRunning.value = false
    const stoppedMode = devMode.value
    devMode.value = ''
    if (previewMode.value === 'preview') previewMode.value = 'edit'
    const tip = stoppedMode === 'present' ? '已停止演示进程' : '已停止预览进程'
    message.success(tip)
    logger.info('[Slidev] preview cleared', { mode: stoppedMode })
  } catch (e) {
    logger.error('[Slidev] stop preview failed', e)
    message.error('停止失败')
  }
}

const exportPptx = async () => {
  const { path } = getActivePathAndContent()
  if (!path) {
    message.warning('请先打开 slides.md（或任意 Slidev 源文件）')
    return
  }

  try {
    await saveActive()

    const suggestedName = 'slides.pptx'
    const output = await saveDialog({
      title: '导出为 PPTX',
      defaultPath: suggestedName,
      filters: [{ name: 'PPTX', extensions: ['pptx'] }],
    })
    if (!output) return

    logger.info('[Slidev] export pptx', { entry: path, output })
    await invoke('slidev_export_pptx', { entryPath: path, outputPath: String(output) })
    message.success('导出成功')
  } catch (e) {
    logger.error('[Slidev] export failed', e)
    message.error('导出失败（需要本机可用的 Node.js / npx）')
  }
}

const kbLoading = ref(false)
const kbSpinLoading = ref(false)
const kbTree = ref([])
const kbFilter = ref('')
const selectedKbPath = ref('')
const kbSelectedKeys = ref([])
const kbWatchActive = ref(false)
let kbWatchTimer = 0

const kbOptions = computed(() => {
  const list = Array.isArray(knowledgeBaseStore.knowledgeBases) ? knowledgeBaseStore.knowledgeBases : []
  return list.map((kb) => ({
    label: kb.knowledge_base_name || kb.knowledgeBaseName || kb.location,
    value: normalizePath(kb.location),
  }))
})

const toKbTreeNode = (node) => {
  if (!node) return null
  const name = String(node.name || '')
  const path = normalizePath(node.path)
  if (!path || isHiddenName(name) || name === '.meta' || path.includes('/.meta/')) return null

  const isDir = !!node.is_dir
  const children = Array.isArray(node.children) ? node.children : []
  const mappedChildren = children.map(toKbTreeNode).filter(Boolean)

  if (isDir) {
    return { key: path, label: name, isDir: true, children: mappedChildren }
  }
  return { key: path, label: name, isDir: false }
}

const collectTreeKeys = (nodes) => {
  const out = new Set()
  const walk = (list) => {
    const arr = Array.isArray(list) ? list : []
    for (const n of arr) {
      const key = normalizePath(n?.key)
      if (key) out.add(key)
      if (n?.children?.length) walk(n.children)
    }
  }
  walk(nodes)
  return out
}

const loadKbTree = async (options = {}) => {
  const { silent = false, keepState = false, reason = 'manual' } = options
  const rootPath = normalizePath(selectedKbPath.value)
  if (!rootPath) {
    if (!silent) message.warning('请先选择一个知识库')
    return
  }

  if (kbLoading.value) {
    logger.info('[KB] load skipped', { rootPath, reason })
    return
  }

  kbLoading.value = true
  if (!silent) kbSpinLoading.value = true
  try {
    const root = await invoke('get_knowledge_base_tree', { rootPath })
    const node = toKbTreeNode(root)
    const nodes = node ? [node] : []
    kbTree.value = nodes

    if (keepState) {
      const keySet = collectTreeKeys(nodes)
      const nextExpanded = (Array.isArray(kbExpandedKeys.value) ? kbExpandedKeys.value : [])
        .map(normalizePath)
        .filter((k) => keySet.has(k))
      const nextSelected = (Array.isArray(kbSelectedKeys.value) ? kbSelectedKeys.value : [])
        .map(normalizePath)
        .filter((k) => keySet.has(k))

      kbExpandedKeys.value = nextExpanded.length > 0 ? nextExpanded : node ? [node.key] : []
      kbSelectedKeys.value = nextSelected
    } else {
      kbExpandedKeys.value = node ? [node.key] : []
    }

    logger.info('[KB] tree loaded', { rootPath, reason, keepState })
  } catch (e) {
    logger.error('[KB] load tree failed', e)
    message.error('加载知识库目录树失败')
  } finally {
    kbLoading.value = false
    if (!silent) kbSpinLoading.value = false
  }
}

watch(
  () => selectedKbPath.value,
  async (next, prev) => {
    const n = normalizePath(next)
    const p = normalizePath(prev)
    if (n === p) return

    stopKbPolling()
    clearMaterial()
    kbTree.value = []
    kbExpandedKeys.value = []
    kbSelectedKeys.value = []

    if (!n) return

    logger.info('[KB] selected', { rootPath: n })
    await loadKbTree({ reason: 'select' })
    startKbPolling()
  }
)


const startKbPolling = () => {
  const rootPath = normalizePath(selectedKbPath.value)
  if (!rootPath) {
    stopKbPolling()
    return
  }

  if (kbWatchActive.value && kbWatchTimer) return

  stopKbPolling()
  kbWatchTimer = window.setInterval(async () => {
    if (!selectedKbPath.value || kbLoading.value) return
    await loadKbTree({ silent: true, keepState: true, reason: 'poll' })
  }, 10 * 60 * 1000)
  kbWatchActive.value = true
  logger.info('[KB] poll start', { rootPath })
}

const stopKbPolling = () => {
  if (kbWatchTimer) {
    clearInterval(kbWatchTimer)
    kbWatchTimer = 0
  }
  if (kbWatchActive.value) {
    logger.info('[KB] poll stop')
  }
  kbWatchActive.value = false
}

const filterTree = (nodes, q) => {
  const query = String(q || '').trim().toLowerCase()
  if (!query) return nodes

  const visit = (n) => {
    const label = String(n?.label || '').toLowerCase()
    const children = Array.isArray(n?.children) ? n.children : []
    const nextChildren = children.map(visit).filter(Boolean)

    if (label.includes(query) || nextChildren.length > 0) {
      return { ...n, children: nextChildren.length > 0 ? nextChildren : n.children }
    }
    return null
  }

  return (Array.isArray(nodes) ? nodes : []).map(visit).filter(Boolean)
}

const filteredKbTree = computed(() => filterTree(kbTree.value, kbFilter.value))
const filteredSlidevTree = computed(() => filterTree(slidevTree.value, slidevFilter.value))

const material = reactive({
  path: '',
  content: '',
  blocks: [],
  loading: false,
})

const activeMaterialCardId = ref('')
const materialCardsRef = ref(null)

const splitMaterialFallback = (raw) => {
  const text = String(raw || '')
  if (!text) return []
  return text
    .split(/\n\s*\n+/g)
    .map((item) => String(item ?? '').trim())
    .filter((item) => item)
}

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

const renderMaterialBlocks = async (blocks) => {
  const list = Array.isArray(blocks) ? blocks : []
  const rendered = []
  const theme = resolveMarkdownTheme()

  for (const item of list) {
    const raw = String(item ?? '').trim()
    if (!raw) continue
    const html = await renderMarkdown(raw, {
      preserveOriginal: true,
      basePath: '',
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme
    })
    rendered.push({ raw, html: String(html ?? '') })
  }

  return rendered
}

const renderMaterialMermaid = async () => {
  try {
    await nextTick()

    const host = materialCardsRef.value
    if (!(host instanceof HTMLElement)) return

    const nodes = Array.from(host.querySelectorAll('.mermaid') || [])
    if (!nodes.length) return

    const theme = 'default'
    mermaid.initialize({
      startOnLoad: false,
      theme,
      securityLevel: 'loose'
    })

    nodes.forEach((el) => {
      try {
        el.removeAttribute('data-processed')
        const content = String(el.textContent || '')
          .replace(/&gt;/g, '>')
          .replace(/&lt;/g, '<')
          .replace(/&amp;/g, '&')
        el.innerHTML = content
      } catch (err) {
        logger.error('[KB] mermaid content failed', err)
      }
    })

    await nextTick()
    requestAnimationFrame(() => {
      Promise.resolve(mermaid.run({ nodes }))
        .then(() => {
          logger.info('[KB] mermaid rendered', { count: nodes.length })
        })
        .catch((err) => {
          logger.error('[KB] mermaid render failed', err)
        })
    })
  } catch (e) {
    logger.error('[KB] mermaid render failed', e)
  }
}

const parseMaterialContent = async (raw) => {
  const text = String(raw || '')
  if (!text) {
    material.blocks = []
    return
  }

  try {
    const blocks = await invoke('slidev_split_markdown_paragraphs', { content: text })
    if (Array.isArray(blocks)) {
      material.blocks = await renderMaterialBlocks(blocks)
      await renderMaterialMermaid()
      logger.info('[KB] material parsed', { path: material.path, count: material.blocks.length })
      return
    }
    material.blocks = []
    logger.info('[KB] material parsed empty', { path: material.path })
  } catch (e) {
    logger.error('[KB] material parse failed', e)
    const fallbackBlocks = splitMaterialFallback(text)
    material.blocks = await renderMaterialBlocks(fallbackBlocks)
    await renderMaterialMermaid()
  }
}

const materialCards = computed(() => {
  const blocks = Array.isArray(material.blocks) ? material.blocks : []

  return blocks
    .map((item, index) => ({
      id: `${material.path || 'material'}-${index}`,
      raw: String(item?.raw ?? '').trim(),
      html: String(item?.html ?? '')
    }))
    .filter((item) => item.raw)
})

const materialCardCount = computed(() => materialCards.value.length)

const handleMaterialCardClick = (card) => {
  const id = String(card?.id || '')
  if (!id) return
  activeMaterialCardId.value = id
  logger.info('[KB] material card selected', { id })
}

const clearMaterial = () => {
  material.path = ''
  material.content = ''
  material.blocks = []
  material.loading = false
  activeMaterialCardId.value = ''
  kbSelectedKeys.value = []
}

const readMaterial = async (path) => {
  const p = normalizePath(path)
  if (!p) return

  material.loading = true
  try {
    const content = await invoke('read_file_content', { path: p })
    material.path = p
    material.content = String(content ?? '')
    await parseMaterialContent(material.content)
    activeMaterialCardId.value = ''
    logger.info('[KB] material loaded', { path: p, len: material.content.length })
  } catch (e) {
    logger.error('[KB] material read failed', e)
    message.error('读取素材失败（可能为非 UTF-8 或二进制文件）')
  } finally {
    material.loading = false
  }
}

const reloadMaterial = async () => {
  if (!material.path) return
  await readMaterial(material.path)
}

const handleKbSelectNode = async (node) => {
  const key = normalizePath(node?.key)
  if (!key) return
  if (node?.isDir) return

  rightPanelMode.value = 'material'
  if (rightCollapsed.value) {
    rightCollapsed.value = false
    logger.info('[UI] auto expand right sider on kb select', { key })
    await nextTick()
  }

  await readMaterial(key)
}

const insertMaterialFile = async () => {
  const path = normalizePath(material.path)
  if (!path) {
    message.warning('请选择一个素材文件')
    return
  }

  const content = String(material.content || '')
  if (!content) {
    message.warning('素材为空')
    return
  }

  try {
    const text = content.endsWith('\n') ? content : `${content}\n`
    const ok = await pptEditorRef.value?.insertContent?.(`\n${text}`)
    logger.info('[KB] insert file', { ok: !!ok, path, len: text.length })
    if (!ok) {
      message.warning('插入失败：请先打开一个 PPT 源文件')
      return
    }
    message.success('已插入到 PPT 源文件')
  } catch (e) {
    logger.error('[KB] insert file failed', e)
    message.error('插入失败')
  }
}

const insertMaterialCard = async (card, index) => {
  const content = String(card?.raw || '')
  if (!material.path || !content) {
    message.warning('请选择一个素材段落')
    return
  }

  try {
    const text = content.endsWith('\n') ? content : `${content}\n`
    const ok = await pptEditorRef.value?.insertContent?.(`\n${text}`)
    logger.info('[KB] insert card', { ok: !!ok, path: material.path, index, len: text.length })
    if (!ok) {
      message.warning('插入失败：请先打开一个 PPT 源文件')
      return
    }
    message.success('已插入到 PPT 源文件')
  } catch (e) {
    logger.error('[KB] insert card failed', e)
    message.error('插入失败')
  }
}

const handleKbInsertAction = async (node) => {
  const key = normalizePath(node?.key)
  if (!key || node?.isDir) return

  try {
    if (material.path !== key) {
      await readMaterial(key)
    }

    const content = String(material.content || '')
    if (!content) {
      message.warning('素材为空')
      return
    }

    const text = content.endsWith('\n') ? content : `${content}\n`
    const ok = await pptEditorRef.value?.insertContent?.(`\n${text}`)
    logger.info('[KB] insert by action', { ok: !!ok, path: key, len: text.length })
    if (!ok) {
      message.warning('插入失败：请先打开一个 PPT 源文件')
      return
    }
    message.success('已插入到 PPT 源文件')
  } catch (e) {
    logger.error('[KB] insert action failed', e)
    message.error('插入失败')
  }
}


onMounted(async () => {
  // 说明：Slidev 窗口使用独立 store，避免与主窗口共享 tab/编辑状态
  slidevEditorStore.reset()

  await nextTick()
  logCenterLayout('mounted')

  try {
    await knowledgeBaseStore.fetchKnowledgeBases()
    if (!selectedKbPath.value && kbOptions.value.length > 0) {
      selectedKbPath.value = kbOptions.value[0].value
    }
  } catch (e) {
    logger.warn('[KB] fetch list failed', e)
  }

  await refreshSlidevRoot()
})
</script>

<style scoped>
.slidev-page {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1 1 auto;
  height: 100vh;
  min-height: 0;
  width: 100%;
  overflow: hidden;
  background: var(--guaxi-app-bg);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

.slidev-layout {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  background: var(--guaxi-app-bg);
}

.slidev-page :deep(.n-layout) {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
}

.slidev-page :deep(.n-layout-content) {
  min-height: 0;
}

.slidev-page :deep(.n-layout-sider) {
  min-height: 0;
}

.slidev-sider {
  height: 100%;
}

.slidev-sider-left,
.slidev-sider-right {
  overflow: hidden;
}

.slidev-sider-left.is-collapsed,
.slidev-sider-right.is-collapsed {
  min-width: 0;
}

.sider-toggle {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  z-index: 60;
  pointer-events: auto;
  transition: left 0.2s ease, right 0.2s ease, opacity 0.2s ease;
}

.sider-toggle-left {
  transform: translate(-50%, -50%);
}

.sider-toggle-right {
  transform: translate(50%, -50%);
}

.sider-toggle :deep(.n-button) {
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  transition: background 0.2s ease, border-color 0.2s ease, box-shadow 0.2s ease, color 0.2s ease;
}

.sider-toggle :deep(.n-button:hover) {
  background: rgba(24, 144, 255, 0.18);
  border-color: rgba(24, 144, 255, 0.6);
  box-shadow: 0 0 0 1px rgba(24, 144, 255, 0.22);
}

.sider-toggle.collapsed :deep(.n-button) {
  background: rgba(24, 144, 255, 0.22);
  border-color: rgba(24, 144, 255, 0.7);
  box-shadow: 0 4px 10px rgba(24, 144, 255, 0.2);
}

.left-panels {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.left-panel-top {
  flex: 0 0 auto;
  min-height: 0;
  overflow: hidden;
}

.left-panel-bottom {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

.left-panel-splitter {
  height: 6px;
  cursor: ns-resize;
  background: transparent;
}

.left-panel-splitter:hover {
  background: rgba(0, 0, 0, 0.12);
}

.slidev-center {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

.editor-header {
  display: flex;
  flex-direction: column;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
}

.tab-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
}

.tab-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 6px;
  cursor: pointer;
  background: rgba(128, 128, 128, 0.08);
  border: 1px solid rgba(0, 0, 0, 0.08);
  color: var(--guaxi-text-primary);
  user-select: none;
  flex: 0 0 auto;
  transition: background 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease, transform 0.15s ease;
}

.tab-item:hover {
  background: rgba(127, 127, 127, 0.12);
  border-color: rgba(0, 0, 0, 0.14);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
  transform: translateY(-1px);
}

.tab-item.active {
  background: rgba(24, 144, 255, 0.18);
  border-color: rgba(24, 144, 255, 0.35);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.12);
}

.tab-item.active:hover {
  background: rgba(24, 144, 255, 0.22);
  border-color: rgba(24, 144, 255, 0.45);
}

:global(:root[data-guaxi-theme='dark']) .tab-item {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.08);
}

:global(:root[data-guaxi-theme='dark']) .tab-item:hover {
  background: rgba(74, 163, 255, 0.18);
  border-color: rgba(74, 163, 255, 0.3);
  box-shadow: 0 1px 6px rgba(0, 0, 0, 0.35);
}

:global(:root[data-guaxi-theme='dark']) .tab-item.active {
  background: rgba(24, 144, 255, 0.28);
  border-color: rgba(74, 163, 255, 0.45);
  box-shadow: 0 1px 8px rgba(0, 0, 0, 0.4);
}

:global(:root[data-guaxi-theme='dark']) .tab-item.active:hover {
  background: rgba(24, 144, 255, 0.32);
  border-color: rgba(74, 163, 255, 0.55);
}

.tab-title {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tab-affordance {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.tab-dirty-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: #d97706;
}

.tab-close {
  opacity: 0.7;
  transition: opacity 0.15s ease, color 0.15s ease;
}

.tab-item:hover .tab-close {
  opacity: 1;
}

.toolbar {
  padding: 8px 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: nowrap;
  min-width: 0;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}

.toolbar-primary-actions {
  flex: 0 1 auto;
  min-width: 0;
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

.toolbar-editing-actions :deep(.n-button) {
  padding: 0 6px;
  height: 24px;
  border-radius: 6px;
}

.toolbar-editing-actions :deep(.n-button:hover) {
  background-color: var(--guaxi-panel-bg);
}

.toolbar-spacer {
  flex-grow: 1;
  min-width: 16px;
  flex-shrink: 1;
}

.stop-active :deep(.n-icon),
.stop-active :deep(.n-button__icon) {
  color: var(--n-error-color);
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
  font-size: 12px;
  color: var(--guaxi-text-primary);
}

.editor-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.editor-pane,
.preview-pane {
  flex: 1 1 0;
  min-width: 0;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.editor-pane :deep(.slidev-lite-editor) {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  height: 100%;
  width: 100%;
}

.editor-empty {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--n-text-color-3);
  font-size: 13px;
}

.preview-pane {
  border-left: 1px solid var(--n-border-color);
  background: var(--guaxi-content-bg, #111);
}

.preview-iframe {
  flex: 1 1 auto;
  width: 100%;
  height: 100%;
  border: none;
}

.preview-empty,
.preview-loading {
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--n-text-color-3);
  font-size: 13px;
}

.editor-content.edit .preview-pane {
  display: none;
}

.editor-content.preview .editor-pane {
  display: none;
}

.tree-scroll {
  height: 100%;
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 4px;
  height: 100%;
  cursor: ew-resize;
  background: transparent;
}

.sider-resize-handle-right {
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  cursor: ew-resize;
  background: transparent;
}

.sider-resize-handle:hover,
.sider-resize-handle-right:hover {
  background: rgba(0, 0, 0, 0.12);
}

.tree-host {
  height: 100%;
  min-height: 0;
}

.panel-body :deep(.n-spin-container) {
  height: 100%;
}

.panel-body :deep(.n-spin-content) {
  height: 100%;
}

.panel-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}

.panel-mode-btn {
  border-radius: 8px;
  padding: 2px 10px;
  min-height: 24px;
  font-size: 13px;
  transition: background 0.2s ease, color 0.2s ease, box-shadow 0.2s ease, transform 0.2s ease;
}

.panel-mode-btn:hover {
  background: rgba(24, 144, 255, 0.12);
  color: var(--n-color-primary, #1890ff);
  transform: translateY(-1px);
}

.panel-mode-btn.active {
  color: var(--n-color-primary, #1890ff);
  background: rgba(24, 144, 255, 0.18);
  box-shadow: 0 2px 8px rgba(24, 144, 255, 0.2);
}

.outline-panel-host {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.tree-label {
  display: inline-block;
  max-width: 100%;
  overflow: visible;
  text-overflow: clip;
  white-space: normal;
  word-break: break-word;
  line-height: 1.4;
}

.panel {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.panel-header {
  padding: 10px 10px 8px;
  border-bottom: 1px solid var(--n-border-color);
  display: flex;
  flex-direction: column;
}

.panel-title {
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.panel-title-icons {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.panel-title-icons .panel-mode-btn.active {
  color: var(--n-color-primary, #1890ff);
}

.panel-body {
  flex: 1;
  min-height: 0;
  padding: 10px 6px 10px 10px;
  overflow: hidden;
}

.panel-body-scroll {
  overflow: auto;
}

.panel-empty {
  height: 100%;
  min-height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, #8a8f98);
  font-size: 13px;
  user-select: none;
}

.kb-card-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.kb-card {
  border: 1px solid var(--n-border-color);
  transition: box-shadow 0.2s ease, transform 0.2s ease, border-color 0.2s ease;
}

.kb-card:hover {
  border-color: rgba(24, 144, 255, 0.35);
  box-shadow: 0 6px 16px rgba(24, 144, 255, 0.12);
  transform: translateY(-1px);
}

.kb-card.kb-card-active {
  border-color: rgba(24, 144, 255, 0.7);
  box-shadow: 0 8px 20px rgba(24, 144, 255, 0.18);
}

.kb-card :deep(.n-card__header) {
  padding: 6px 12px 0;
  min-height: 0;
}

.kb-card :deep(.n-card__content) {
  padding-top: 4px;
}

.kb-card-header-spacer {
  display: block;
  height: 0;
}

.kb-card-content {
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.5;
  font-size: 14px;
  color: var(--n-text-color);
}

.panel-body :deep(.n-tree) {
  height: 100%;
}

.panel-footer {
  padding: 8px 10px;
  border-top: 1px solid var(--n-border-color);
}

.path-text {
  display: block;
  width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.material-preview-textarea {
  width: 100%;
  min-height: 180px;
  border: 1px solid var(--n-border-color);
  border-radius: 6px;
  padding: 8px 10px;
  background: var(--n-color);
  color: var(--n-text-color);
  font-size: 12px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  resize: vertical;
  box-sizing: border-box;
}

.select-tip {
  color: var(--n-text-color-3);
  font-size: 12px;
}
</style>
