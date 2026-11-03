<template>
  <div class="file-tree-container" ref="fileTreeContainerRef">
    <!-- 工具栏 -->
    <div class="list-header">
      <div class="title-container">
        <n-text class="repo-title-header">{{ modeTitle }}</n-text>
      </div>
      <div class="header-actions">
        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper
              :size="18"
              :border-radius="6"
              class="browser-icon"
              :class="{ active: browserPreviewEnabled }"
              @click="toggleBrowserPreview"
            >
              <n-icon :component="GlobeOutline" size="16" />
            </n-icon-wrapper>
          </template>
          {{ browserPreviewEnabled ? '浏览器预览：已开启' : '浏览器预览：已关闭' }}
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper :size="18" :border-radius="6" class="git-icon" @click="toggleGitPanel">
              <n-icon :component="GitBranchOutline" size="16" />
            </n-icon-wrapper>
          </template>
          Git 操作
        </n-tooltip>

        <n-dropdown trigger="click" :options="openMenuOptions" class="file-tree-context-menu" @select="handleOpenMenuSelect">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="18" :border-radius="6" class="open-icon">
                <n-icon :component="OpenIcon" size="16" />
              </n-icon-wrapper>
            </template>
            打开文件 / 目录
          </n-tooltip>
        </n-dropdown>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper
              :size="18"
              :border-radius="6"
              class="mode-toggle-icon"
              :class="{ active: viewMode !== 'kb' }"
              @click="toggleViewMode"
            >
              <n-icon :component="modeToggleIcon" size="16" />
            </n-icon-wrapper>
          </template>
          {{ modeToggleTooltip }}
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper :size="18" :border-radius="6" class="refresh-icon" @click="refreshAll">
              <n-icon :component="RefreshIcon" size="16" />
            </n-icon-wrapper>
          </template>
          刷新所有目录
        </n-tooltip>

        <n-tooltip trigger="hover">
          <template #trigger>
            <n-icon-wrapper
              :size="18"
              :border-radius="6"
              class="kb-manager-icon"
              @click="openKnowledgeBaseManager"
            >
              <n-icon :component="SettingsOutline" size="16" />
            </n-icon-wrapper>
          </template>
          知识库管理器
        </n-tooltip>
      </div>
    </div>

    <div v-if="viewMode === 'kb'" class="repo-body" :class="{ 'active-filled': isActiveRepoExpanded }">
      <div v-if="activeRepoPathItem" class="active-repo-wrapper"
        :class="{ expanded: fileTreeStore.isRepoExpanded(activeRepoPathItem) }">
        <div class="repo-item" @click="toggleRepo(activeRepoPathItem)">
          <div class="repo-title">
            <n-icon size="16" class="repo-icon">
              <FolderIcon />
            </n-icon>
            {{ getRepoName(activeRepoPathItem) }}
          </div>
          <div class="repo-actions">
            <n-dropdown trigger="click" :options="importMenuOptions" placement="bottom-start" @click.stop @select="(key) => handleImportMarkdownFile(activeRepoPathItem, key)">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop>
                    <n-icon :component="DownloadOutline" size="16" />
                  </n-icon-wrapper>
                </template>
                {{ fileTreeShortcutTooltip('导入文件', 'fileTreeImportFile') }}
              </n-tooltip>
            </n-dropdown>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" data-tour="new-file" @click.stop="handleAddFile(activeRepoPathItem)">
                  <n-icon :component="FileAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              {{ fileTreeShortcutTooltip('新建文件', 'fileTreeNewFile') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(activeRepoPathItem)">
                  <n-icon :component="FolderAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              {{ fileTreeShortcutTooltip('新建文件夹', 'fileTreeNewFolder') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="openRootPathInNeovim(activeRepoPathItem, 'knowledge-root-active')">
                  <Icon :icon="neovimIcon" width="16" height="16" class="neovim-action-icon" />
                </n-icon-wrapper>
              </template>
              用 Neovim 打开目录
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleDeleteKnowledgeBase(activeRepoPathItem)">
                  <n-icon :component="TrashOutline" size="16" />
                </n-icon-wrapper>
              </template>
              删除知识库
            </n-tooltip>
          </div>
        </div>

        <div v-if="fileTreeStore.isRepoExpanded(activeRepoPathItem)" class="active-tree-container" @wheel.stop>
          <GuaxiVirtualFileTree
            ref="treeRef"
            class="tree-component"
            :nodes="fileTree"
            v-model:expanded-keys="expandedKeys"
            v-model:selected-keys="selectedKeys"
            :editing-id="editingId"
            v-model:edit-name="editName"
            :root-id="fileTreeStore.activeRepoPath"
            @open-file="handleOpenFile"
            @load-children="handleLoadChildren"
            @contextmenu="handleNodeContextMenu"
            @blank-contextmenu="handleBlankContextMenu"
            @rename-commit="handleBlur"
            @rename-cancel="cancelEditing"
            @move="handleDropMove"
            @delete-node="(node) => handleVirtualDeleteNode('kb', node)"
          />
        </div>
      </div>

      <div v-show="!isActiveRepoExpanded" class="repo-scroll-wrapper">
        <n-scrollbar ref="repoScrollbarRef" class="repo-scroll-container">
          <n-list class="repo-list" :show-divider="false">
          <n-list-item v-for="path in inactiveRepoPaths" :key="path" class="repo-list-item">
            <div class="repo-item" @click="toggleRepo(path)">
              <div class="repo-title">
                <n-icon size="16" class="repo-icon">
                  <FolderIcon />
                </n-icon>
                {{ getRepoName(path) }}
              </div>
              <div class="repo-actions">
                <n-dropdown trigger="click" :options="importMenuOptions" placement="bottom-start" @click.stop @select="(key) => handleImportMarkdownFile(path, key)">
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop>
                        <n-icon :component="DownloadOutline" size="16" />
                      </n-icon-wrapper>
                    </template>
                    {{ fileTreeShortcutTooltip('导入文件', 'fileTreeImportFile') }}
                  </n-tooltip>
                </n-dropdown>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFile(path)">
                      <n-icon :component="FileAddIcon" size="16" />
                    </n-icon-wrapper>
                  </template>
                  {{ fileTreeShortcutTooltip('新建文件', 'fileTreeNewFile') }}
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(path)">
                      <n-icon :component="FolderAddIcon" size="16" />
                    </n-icon-wrapper>
                  </template>
                  {{ fileTreeShortcutTooltip('新建文件夹', 'fileTreeNewFolder') }}
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="openRootPathInNeovim(path, 'knowledge-root-inactive')">
                      <Icon :icon="neovimIcon" width="16" height="16" class="neovim-action-icon" />
                    </n-icon-wrapper>
                  </template>
                  用 Neovim 打开目录
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleDeleteKnowledgeBase(path)">
                      <n-icon :component="TrashOutline" size="16" />
                    </n-icon-wrapper>
                  </template>
                  删除知识库
                </n-tooltip>
              </div>
            </div>
          </n-list-item>
          </n-list>
        </n-scrollbar>
      </div>
    </div>

    <!-- 任务编辑模式：与知识库一致——激活的分类占满整列，收起后其它分类以列表显示在其后 -->
    <div v-else-if="viewMode === 'task'" class="repo-body" :class="{ 'active-filled': taskActiveExpanded }">
      <div class="active-repo-wrapper" :class="{ expanded: taskActiveExpanded }">
        <div class="repo-item" @click="toggleActiveTaskCat">
          <div class="repo-title" :title="catPathOf(activeTaskCat)">
            <n-icon size="16" class="repo-icon"><FolderIcon /></n-icon>
            {{ catZhOf(activeTaskCat) }}
          </div>
          <div class="repo-actions">
            <n-dropdown trigger="click" :options="importMenuOptions" placement="bottom-start" @click.stop @select="(key) => taskCatImport(activeTaskCat, key)">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop>
                    <n-icon :component="DownloadOutline" size="16" />
                  </n-icon-wrapper>
                </template>
                {{ fileTreeShortcutTooltip('导入文件', 'fileTreeImportFile') }}
              </n-tooltip>
            </n-dropdown>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="taskCatAddFile(activeTaskCat)">
                  <n-icon :component="FileAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              {{ fileTreeShortcutTooltip('新建文件', 'fileTreeNewFile') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="taskCatAddFolder(activeTaskCat)">
                  <n-icon :component="FolderAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              {{ fileTreeShortcutTooltip('新建文件夹', 'fileTreeNewFolder') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="openRootPathInNeovim(catPathOf(activeTaskCat), 'task-root-active')">
                  <Icon :icon="neovimIcon" width="16" height="16" class="neovim-action-icon" />
                </n-icon-wrapper>
              </template>
              用 Neovim 打开目录
            </n-tooltip>
          </div>
        </div>

        <div v-if="taskActiveExpanded" class="active-tree-container" @wheel.stop>
          <GuaxiVirtualFileTree
            ref="taskTreeRef"
            class="tree-component"
            :nodes="taskCatTrees[activeTaskCat]"
            v-model:expanded-keys="taskCatExpandedKeys[activeTaskCat]"
            v-model:selected-keys="taskCatSelectedKeys[activeTaskCat]"
            :editing-id="editingId"
            v-model:edit-name="editName"
            :root-id="catPathOf(activeTaskCat)"
            @open-file="handleTaskOpenFile"
            @load-children="handleTaskLoadChildren"
            @contextmenu="(p) => openGenContextMenu('task', p)"
            @blank-contextmenu="(p) => openGenBlankMenu('task', p, catPathOf(activeTaskCat))"
            @rename-commit="handleInlineCommit"
            @rename-cancel="cancelInlineCreate"
            @delete-node="(node) => handleVirtualDeleteNode('task', node)"
          />
        </div>
      </div>

      <div v-show="!taskActiveExpanded" class="repo-scroll-wrapper">
        <n-scrollbar class="repo-scroll-container">
          <n-list class="repo-list" :show-divider="false">
            <n-list-item v-for="cat in inactiveTaskCats" :key="cat.key" class="repo-list-item">
              <div class="repo-item" @click="activateTaskCat(cat.key)">
                <div class="repo-title" :title="catPathOf(cat.key)">
                  <n-icon size="16" class="repo-icon"><FolderIcon /></n-icon>
                  {{ cat.zh }}
                </div>
                <div class="repo-actions">
                  <n-dropdown trigger="click" :options="importMenuOptions" placement="bottom-start" @click.stop @select="(key) => taskCatImport(cat.key, key)">
                    <n-tooltip trigger="hover">
                      <template #trigger>
                        <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop>
                          <n-icon :component="DownloadOutline" size="16" />
                        </n-icon-wrapper>
                      </template>
                      {{ fileTreeShortcutTooltip('导入文件', 'fileTreeImportFile') }}
                    </n-tooltip>
                  </n-dropdown>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="taskCatAddFile(cat.key)">
                        <n-icon :component="FileAddIcon" size="16" />
                      </n-icon-wrapper>
                    </template>
                    {{ fileTreeShortcutTooltip('新建文件', 'fileTreeNewFile') }}
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="taskCatAddFolder(cat.key)">
                        <n-icon :component="FolderAddIcon" size="16" />
                      </n-icon-wrapper>
                    </template>
                    {{ fileTreeShortcutTooltip('新建文件夹', 'fileTreeNewFolder') }}
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="openRootPathInNeovim(catPathOf(cat.key), 'task-root-inactive')">
                        <Icon :icon="neovimIcon" width="16" height="16" class="neovim-action-icon" />
                      </n-icon-wrapper>
                    </template>
                    用 Neovim 打开目录
                  </n-tooltip>
                </div>
              </div>
            </n-list-item>
          </n-list>
        </n-scrollbar>
      </div>
    </div>

    <!-- 普通文件编辑模式：与知识库一致——激活目录占满整列，收起后打开历史列表显示在其后 -->
    <div v-else class="repo-body plain-body" :class="{ 'active-filled': plainActiveExpanded }">
      <!-- 已激活某个外部目录：根条目可展开收缩 + 操作图标（与知识库模式一致） -->
      <div v-if="externalDir" class="active-repo-wrapper" :class="{ expanded: plainRootExpanded }">
        <div class="repo-item" @click="plainRootExpanded = !plainRootExpanded">
          <div class="repo-title" :title="externalDir">
            <n-icon size="16" class="repo-icon"><FolderIcon /></n-icon>
            {{ getDirDisplayName(externalDir) }}
          </div>
          <div class="repo-actions">
            <n-dropdown trigger="click" :options="importMenuOptions" placement="bottom-start" @click.stop @select="(key) => plainHeaderImport(key)">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop>
                    <n-icon :component="DownloadOutline" size="16" />
                  </n-icon-wrapper>
                </template>
                {{ fileTreeShortcutTooltip('导入文件', 'fileTreeImportFile') }}
              </n-tooltip>
            </n-dropdown>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="plainHeaderAddFile">
                  <n-icon :component="FileAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              {{ fileTreeShortcutTooltip('新建文件', 'fileTreeNewFile') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="plainHeaderAddFolder">
                  <n-icon :component="FolderAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              {{ fileTreeShortcutTooltip('新建文件夹', 'fileTreeNewFolder') }}
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="openRootPathInNeovim(externalDir, 'plain-root-active')">
                  <Icon :icon="neovimIcon" width="16" height="16" class="neovim-action-icon" />
                </n-icon-wrapper>
              </template>
              用 Neovim 打开目录
            </n-tooltip>
          </div>
        </div>
        <div v-if="plainRootExpanded" class="active-tree-container" @wheel.stop>
          <GuaxiVirtualFileTree
            ref="plainTreeRef"
            class="tree-component"
            :nodes="externalDirTree"
            v-model:expanded-keys="externalExpandedKeys"
            v-model:selected-keys="externalSelectedKeys"
            :editing-id="editingId"
            v-model:edit-name="editName"
            :root-id="externalDir"
            @open-file="handleExternalOpenFile"
            @load-children="handleExternalLoadChildren"
            @contextmenu="(p) => openGenContextMenu('plain', p)"
            @blank-contextmenu="(p) => openGenBlankMenu('plain', p)"
            @rename-commit="handleInlineCommit"
            @rename-cancel="cancelInlineCreate"
            @delete-node="(node) => handleVirtualDeleteNode('plain', node)"
          />
        </div>
      </div>

      <!-- 收起或未激活外部目录：显示打开历史列表（点击历史条目激活，等价知识库切换仓库） -->
      <div v-show="!plainActiveExpanded" class="history-wrapper">
        <div class="history-header">
          <span class="history-title">打开历史</span>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="18" :border-radius="6" class="action-icon" @click="clearOpenHistory">
                <n-icon :component="TrashOutline" size="16" />
              </n-icon-wrapper>
            </template>
            清空历史记录
          </n-tooltip>
        </div>
        <n-scrollbar class="repo-scroll-container">
          <div v-if="!openHistory.length" class="history-empty">
            暂无历史，点击上方“打开”选择一个目录
          </div>
          <div
            v-for="path in openHistory"
            :key="path"
            class="repo-list-item history-item"
            @click="handleHistoryItemClick(path)"
          >
            <div class="repo-item">
              <div class="repo-title" :title="path">
                <n-icon size="16" class="repo-icon"><FolderIcon /></n-icon>
                <span class="history-name">{{ getDirDisplayName(path) }}</span>
              </div>
            </div>
            <div class="history-path">{{ path }}</div>
          </div>
        </n-scrollbar>
      </div>
    </div>

    <!-- 右键菜单 -->
    <n-dropdown v-model:show="isContextMenuVisible" :options="activeContextMenuOptions" placement="bottom-start"
      class="file-tree-context-menu"
      :x="contextMenuPosition.x" :y="contextMenuPosition.y" @select="handleContextMenuSelect"
      @clickoutside="handleClickoutside" />

    <!-- 文件编辑 / 任务模式 通用右键菜单 -->
    <n-dropdown v-model:show="genMenuVisible" :options="genMenuOptions" placement="bottom-start"
      class="file-tree-context-menu"
      :x="genMenuPos.x" :y="genMenuPos.y" @select="handleGenMenuSelect"
      @clickoutside="closeGenMenu" />

    <!-- 通用命名弹窗（新建/重命名） -->
    <n-modal v-model:show="nameModal.visible" preset="dialog" :title="nameModal.title" :mask-closable="false"
      positive-text="确定" negative-text="取消" @positive-click="confirmNameModal" @negative-click="cancelNameModal">
      <n-input v-model:value="nameModal.value" :placeholder="nameModal.placeholder" @keyup.enter="confirmNameModal" autofocus />
    </n-modal>


    <n-modal v-model:show="importTemplateModalVisible" preset="dialog" title="导入Markdown文件" :mask-closable="false">
      <div class="import-template-modal-content">
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <n-form-item label="模板名称">
            <n-input v-model:value="importTemplateName" placeholder="例如：会议纪要" />
          </n-form-item>
          <n-form-item label="模板类型">
            <n-input v-model:value="importTemplateType" placeholder="例如：会议 / 日记 / TODO" />
          </n-form-item>
          <n-form-item label="来源文件">
            <n-input :value="importTemplateSourcePath" disabled />
          </n-form-item>
        </n-form>
        <n-space justify="end">
          <n-button @click="importTemplateModalVisible = false">取消</n-button>
          <n-button type="primary" :loading="importTemplateLoading" @click="confirmImportAsTemplate">导入</n-button>
        </n-space>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { NIcon, useMessage, NModal, NForm, NFormItem, NInput, NButton, NSpace } from 'naive-ui'
import { DocumentTextOutline, FolderOutline, AddOutline, TrashOutline, CreateOutline, MoveOutline, FolderOpenOutline, GlobeOutline, SettingsOutline, GitBranchOutline, DownloadOutline, FolderOpenOutline as OpenIcon, DocumentOutline, SwapHorizontalOutline, LibraryOutline, TimeOutline, ListOutline, CheckboxOutline, CopyOutline, DuplicateOutline, BrowsersOutline, GitCompareOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
// 阅读器标签登记表（纯增量）：非文本文件打开时登记，文本链路不查它。
import { useReaderTabStore } from '../../stores/readerTabStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import GuaxiVirtualFileTree from './GuaxiVirtualFileTree.vue';
import { useSettingsStore } from '../../stores/settingsStore'
import { h, ref, reactive, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { listen } from '@tauri-apps/api/event'
import { NList, NListItem, NText, NTooltip, NIconWrapper } from 'naive-ui'
import { Icon } from '@iconify/vue'
import { RefreshOutline as RefreshIcon, ChevronDownOutline as ChevronDownIcon, ChevronForwardOutline as ChevronForwardIcon } from '@vicons/ionicons5'
import { Folder20Regular as FolderIcon, ChannelAdd20Regular as FileAddIcon, FolderAdd20Regular as FolderAddIcon } from '@vicons/fluent'
import { useEditorBridgeStore } from '../../stores/editorBridgeStore'
import { useDiffCompareStore, diffPathBaseName } from '../../stores/diffCompareStore'
import { GUAXI_SHORTCUTS, shortcutTooltip, useGuaxiHotkeys } from '../../composables/useGuaxiHotkeys'
import { updateMarkdownLinksAfterRename } from '../../utils/markdownLinkRename'
import { runFilePathMutationGuards } from '../../utils/filePathMutation'

const logger = createLogger('GuaxiFileTree')
const message = useMessage();
const fileTreeShortcutTooltip = (label, shortcutKey) => shortcutTooltip(label, GUAXI_SHORTCUTS[shortcutKey])

// 编辑器面包屑联动：用于从编辑器定位到文件树节点
const EVENT_REVEAL_IN_FILE_TREE = 'guaxi_editor_reveal_in_file_tree'
const breadcrumbRevealUnlisten = ref(null)

// 处理文件树容器的右键点击事件
const treeRef = ref(null)

const BROWSER_PREVIEW_URL = 'http://localhost:8888/browser/knowledge-bases'
const browserPreviewEnabled = ref(false)
const neovimIcon = {
  width: 24,
  height: 24,
  body: '<path fill="#57a143" d="M3 4.7 8.9 2v20L3 19.3V4.7Z"/><path fill="#0f7d3a" d="M21 4.7 15.1 2v20l5.9-2.7V4.7Z"/><path fill="#4b8bbe" d="M8.9 2 15.1 22V2h-3.2v11.9L8.9 2Z"/>',
}
const renderNeovimIcon = () => h(Icon, {
  icon: neovimIcon,
  width: 16,
  height: 16,
  class: 'neovim-action-icon',
})
// 设计说明：
// 1. `.cellmeta` 会跟随 Notebook/探索者模式频繁落盘，不能按文件变化实时提交，否则会污染 Git 日志并影响编辑体验。
// 2. 文件关闭是更自然的保存边界：用户完成一轮编辑后再合并提交一次，能收集更多内部元数据变化。
// 3. 仍保留低频兜底检查，避免异常关闭或事件丢失时 `.cellmeta` 长时间停留在未提交状态。
const EVENT_CELLMETA_FILE_CLOSED = 'guaxi_cellmeta_file_closed'
const CELLMETA_AUTO_COMMIT_INTERVAL_MS = 60 * 60 * 1000
const CELLMETA_AUTO_COMMIT_CLOSE_DELAY_MS = 1500

let cellmetaAutoCommitIntervalId = null
let cellmetaAutoCommitTimerId = null
let cellmetaAutoCommitRunning = false
let cellmetaAutoCommitPending = false
let cellmetaFileClosedHandler = null

const toggleBrowserPreview = async () => {
  const next = !browserPreviewEnabled.value
  browserPreviewEnabled.value = next

  if (!next) return

  try {
    await openUrl(BROWSER_PREVIEW_URL)
  } catch (e) {
    browserPreviewEnabled.value = false
    message.error('打开浏览器失败，请确认已启用 opener 插件')
  }
}

// 设计说明：Git 操作应针对“当前可见模式”的根目录，而不是固定的知识库根目录。
// 三种模式各自维护独立根目录（与 syncSidebarNavigationContext 一致）：
//   kb    → fileTreeStore.activeRepoPath（当前激活的知识库）
//   plain → externalDir（文件编辑模式打开的外部目录）
//   task  → 当前激活任务分类子目录
// 之前一律用 settingsStore.knowledgeBasesRoot，导致切换模式后 Git 面板仍指向知识库根仓库，
// 既不会随模式切换，也会把“另一个仓库”的改动错当成当前文件的改动（看似没改却显示有改动、退不掉）。
const resolveCurrentModeRepoPath = () => {
  if (viewMode.value === 'plain') {
    return externalDir.value || ''
  }
  if (viewMode.value === 'task') {
    return catPathOf(activeTaskCat.value) || ''
  }
  return fileTreeStore.activeRepoPath || ''
}

const toggleGitPanel = async () => {
  let workspacePath = resolveCurrentModeRepoPath()

  // 知识库模式下若尚无激活仓库，回退到知识库根目录。
  if (!workspacePath && viewMode.value === 'kb') {
    if (!settingsStore.knowledgeBasesRoot) {
      await settingsStore.get_knowledgeBasesRoot()
    }
    workspacePath = settingsStore.knowledgeBasesRoot
  }

  if (!workspacePath) {
    message.error('当前模式没有可用于 Git 操作的目录')
    return
  }

  const windowLabel = `git-${Date.now()}`
  const windowTitle = workspacePath

  try {
    await invoke('create_git_window', {
      label: windowLabel,
      title: windowTitle,
      workspacePath,
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('Git 窗口已打开')
      return
    }
    logger.error('创建 Git 窗口失败:', err)
    message.error('打开 Git 窗口失败')
  }
}

const openKnowledgeBaseManager = async () => {
  try {
    await invoke('create_knowledge_base_manager_window', {
      label: 'knowledge_base_manager',
      title: '知识库管理器',
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('知识库管理器窗口已打开')
      return
    }
    logger.error('打开知识库管理器失败:', err)
    message.error('打开知识库管理器失败')
  }
}

const isMarkdownFilePath = (p) => {
  const v = String(p || '').toLowerCase()
  return v.endsWith('.md') || v.endsWith('.markdown')
}

// 「在浏览器中查看」三种模式共用：预览端点本身只认知识库根，plain/task 的外部目录
// 需要先让后端登记这次显式请求，否则会 403。
const openMarkdownInBrowser = async (filePath) => {
  const nodePath = normalizePath(filePath)
  if (!nodePath) return
  if (!isMarkdownFilePath(nodePath)) {
    message.warning('仅支持 Markdown 文件')
    return
  }
  try {
    await invoke('remember_browser_preview_path', { path: nodePath })
  } catch (error) {
    logger.warn('[OpenInBrowser] register preview path failed', {
      path: nodePath,
      error: String(error?.message || error),
    })
    message.error('无法在浏览器中打开该文件')
    return
  }
  const url = `http://localhost:8888/browser/preview?path=${encodeURIComponent(nodePath)}`
  await openUrl(url)
}

// pandoc 只能“导出”PDF、不能把 PDF/图片作为输入解析（会报 “Unknown input format pdf”）。
// 侧边栏快速导入仅覆盖 pandoc 能读取的文本类文档；PDF、扫描件、图片等需走仓库导入的
// marker/OCR 流程，这里提前拦截并给出明确提示，避免把 pandoc 的失败堆栈直接抛给用户。
const PANDOC_UNSUPPORTED_IMPORT_EXTS = new Set([
  'pdf',
  'png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'tif', 'tiff', 'svg',
])
const isPandocUnsupportedImport = (p) => {
  const ext = String(p || '').toLowerCase().split('.').pop()
  return PANDOC_UNSUPPORTED_IMPORT_EXTS.has(ext)
}

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const readerTabStore = useReaderTabStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const settingsStore = useSettingsStore()
const editorBridgeStore = useEditorBridgeStore()
const diffStore = useDiffCompareStore()
const lastSelectedKey = ref(null);

const fileTreeContainerRef = ref(null)
const repoScrollbarRef = ref(null)
const activeRepoMaxHeight = ref(360)
const activeRepoResizeObserver = ref(null)

// ───── 视图模式：'kb' 知识库模式（默认） / 'plain' 普通文件编辑模式 / 'task' 任务编辑模式 ─────
const viewMode = ref('kb')
// plain 模式下当前激活的外部目录（为空则显示历史列表）
const externalDir = ref(null)
const externalDirTree = ref([])
const externalExpandedKeys = ref([])
const externalSelectedKeys = ref([])
// 激活的外部目录根条目是否展开（与知识库模式一致，可点击收缩）
const plainRootExpanded = ref(true)
// 是否处于「激活目录已展开占满」态：用于控制历史列表的显隐（与知识库 isActiveRepoExpanded 一致）
const plainActiveExpanded = computed(() => !!externalDir.value && plainRootExpanded.value)

// ───── 任务编辑模式：三类根（工作/生活/学习）映射到 work/life/study 子目录 ─────
// 与知识库模式一致：每个分类渲染为一个可展开/收缩、带操作图标的「仓库」条目。
const TASK_CATEGORIES = [
  { key: 'work', zh: '工作' },
  { key: 'life', zh: '生活' },
  { key: 'study', zh: '学习' },
]
const taskRootPath = ref('')
const taskCategoryPaths = ref({ work: '', life: '', study: '' })
// 与知识库模式一致：同一时刻只有一个「激活」分类，激活后占满整列；收起时其它分类以列表形式显示在其后。
const activeTaskCat = ref('work')
const taskActiveExpanded = ref(true)
// 任务模式是否已完成首次初始化：仅首次进入时加载三类目录，之后切换模式保留已展开/已加载状态，
// 避免每次切换都把文件树与展开条目重置回默认态。
const taskModeInitialized = ref(false)
// 注意：这里必须是 reactive，不能换成 shallowReactive。
// 虚拟树把 `taskCatTrees[activeTaskCat]` 直接当 props.nodes，目录展开时
// handleTaskLoadChildren / refreshModeDir 会**就地**给 node.children 赋值；
// shallowReactive 只代理顶层，这类修改不会触发虚拟树重算，表现为「展开目录后子节点不显示」。
// 模式切换的耗时问题由下面 watch 去掉 deep 解决，不靠这里。
const taskCatTrees = reactive({ work: [], life: [], study: [] })
const taskCatExpandedKeys = reactive({ work: [], life: [], study: [] })
const taskCatSelectedKeys = reactive({ work: [], life: [], study: [] })
const catPathOf = (key) => normalizePath(taskCategoryPaths.value[key] || '')
const catZhOf = (key) => (TASK_CATEGORIES.find((c) => c.key === key)?.zh) || key
const inactiveTaskCats = computed(() => TASK_CATEGORIES.filter((c) => c.key !== activeTaskCat.value))

// 三种模式各有一个虚拟树实例（同一时刻只渲染一个），滚动定位时按模式取对应实例。
const plainTreeRef = ref(null)
const taskTreeRef = ref(null)
const treeRefForMode = (mode) => {
  if (mode === 'plain') return plainTreeRef.value
  if (mode === 'task') return taskTreeRef.value
  return treeRef.value
}
const activeTreeRef = computed(() => treeRefForMode(viewMode.value))

// 打开历史（持久化在 localStorage）
const OPEN_HISTORY_KEY = 'guaxi_filetree_open_dir_history'
const openHistory = ref([])

const loadOpenHistory = () => {
  try {
    const raw = localStorage.getItem(OPEN_HISTORY_KEY)
    const arr = raw ? JSON.parse(raw) : []
    openHistory.value = Array.isArray(arr) ? arr.filter(Boolean) : []
  } catch {
    openHistory.value = []
  }
}

const persistOpenHistory = () => {
  try {
    localStorage.setItem(OPEN_HISTORY_KEY, JSON.stringify(openHistory.value))
  } catch (e) {
    logger.warn('保存打开历史失败', e)
  }
}

const pushOpenHistory = (dirPath) => {
  const norm = normalizePath(dirPath)
  if (!norm) return
  openHistory.value = [norm, ...openHistory.value.filter(p => normalizePath(p) !== norm)].slice(0, 50)
  persistOpenHistory()
}

const clearOpenHistory = () => {
  openHistory.value = []
  persistOpenHistory()
}

// "打开"下拉菜单
const openMenuOptions = [
  { label: '打开文件', key: 'openFile', icon: () => h(NIcon, null, { default: () => h(DocumentOutline) }) },
  { label: '打开目录', key: 'openDir', icon: () => h(NIcon, null, { default: () => h(FolderOpenOutline) }) },
]

/**
 * 「导入文件」下拉：两种用户意图。
 *
 * - 原样导入：与改造前行为一致（直接拷贝原始文件；Markdown 直接入库）；
 * - 转换为 Markdown：交给后端引擎路由（PDF/OCR/旧版 Office/电子书/表格都能处理），
 *   产物是同名 .md，原始文件不会被改动。
 *
 * 入口图标与快捷键不变，只是把一次点击变成一次选择；默认项仍是「原样导入」。
 */
const IMPORT_MENU_OPTIONS = [
  {
    label: '原样导入（保留原格式）',
    key: 'importFileCopy',
    icon: () => h(NIcon, null, { default: () => h(DownloadOutline) }),
  },
  {
    label: '转换为 Markdown 后导入',
    key: 'importFileConvert',
    icon: () => h(NIcon, null, { default: () => h(DocumentTextOutline) }),
  },
]
const importMenuOptions = IMPORT_MENU_OPTIONS

const activeRepoPathItem = computed(() => {
  const list = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  const active = fileTreeStore.activeRepoPath
  if (!active) return null

  const activeNorm = normalizePath(active)
  return list.find(p => normalizePath(p) === activeNorm) || null
})

const inactiveRepoPaths = computed(() => {
  const list = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  const active = activeRepoPathItem.value
  if (!active) return list

  const activeNorm = normalizePath(active)
  return list.filter(p => normalizePath(p) !== activeNorm)
})

// 激活的知识库是否展开：展开时让其文件树占满剩余高度，未激活条目退到底部待用户激活。
const isActiveRepoExpanded = computed(() =>
  !!activeRepoPathItem.value && fileTreeStore.isRepoExpanded(activeRepoPathItem.value)
)

const updateActiveRepoMaxHeight = () => {
  const el = fileTreeContainerRef.value
  if (!el) return

  const headerEl = el.querySelector('.list-header')
  const totalHeight = el.getBoundingClientRect().height
  const headerHeight = headerEl ? headerEl.getBoundingClientRect().height : 0
  const scrollAreaHeight = totalHeight - headerHeight

  const reservedForInactive = 120
  activeRepoMaxHeight.value = Math.max(180, Math.floor(scrollAreaHeight - reservedForInactive))
}

watch(
  () => fileTreeStore.activeRepoPath,
  async () => {
    await nextTick()
    updateActiveRepoMaxHeight()
    repoScrollbarRef.value?.scrollTo?.({ top: 0 })
  }
)

// 文件树数据
const fileTree = ref([])
const isTreeLoading = ref(false)

// 设计说明：
// 1. 左侧边栏的三种模式分别维护独立根目录和树数据，GuaxiEditor 不能通过 activeRepoPath 推断 plain/task 模式。
// 2. 这里把当前可见模式的“根路径 + 文件树”同步到共享 store，面包屑即可使用与左栏完全相同的浏览上下文。
// 3. 【不要加 deep】这个 watch 曾经是 `deep: true`，每切换一次模式都要把整棵响应式树完整走一遍：
//    实测 1.26 万节点 88ms、4.2 万节点 269ms，而且是同步执行——用户点下模式切换图标后要干等这么久，
//    节点越多越明显。树本身是不可变替换的（loadFileTree / fetchDirChildren 都赋新数组），
//    引用变化就够了；懒加载的原地 children 赋值由 refreshSidebarNavigationContext() 显式通知（见下）。
// 4. 此处使用局部归一化函数，因为通用 normalizePath 在组件后半段声明；immediate 监听不能提前访问它。
const normalizeSidebarNavigationPath = (path) => String(path || '').replace(/\\/g, '/').replace(/\/+$/, '')

// 当前模式的根路径与树（只读引用，不遍历内容）。
const currentSidebarNavigationSource = () => {
  if (viewMode.value === 'plain') {
    return { rootPath: normalizeSidebarNavigationPath(externalDir.value), tree: externalDirTree.value }
  }
  if (viewMode.value === 'task') {
    return {
      rootPath: catPathOf(activeTaskCat.value),
      tree: taskCatTrees[activeTaskCat.value] || [],
    }
  }
  return {
    rootPath: normalizeSidebarNavigationPath(fileTreeStore.activeRepoPath),
    tree: fileTree.value,
  }
}

const syncSidebarNavigationContext = () => {
  const { rootPath, tree } = currentSidebarNavigationSource()
  fileTreeStore.setSidebarNavigationContext({
    mode: viewMode.value,
    rootPath,
    tree,
  })
}

/**
 * 通知面包屑「树内容变了」，但不做深度监听。
 *
 * 懒加载会把 children 直接赋到已有节点上（`node.children = ...`），树的引用不变，
 * 上面的浅监听看不到这类变化，面包屑的同级候选就会停在加载前的旧树。
 * 这类操作只发生在用户展开/刷新目录时，次数有限，由调用点显式通知即可。
 *
 * 用 requestAnimationFrame 合并同一轮的多次变更（一次展开可能连着改几层）。
 */
let sidebarContextSyncFrame = 0
const refreshSidebarNavigationContext = () => {
  if (sidebarContextSyncFrame) return
  const raf = typeof requestAnimationFrame === 'function' ? requestAnimationFrame : (cb) => setTimeout(cb, 16)
  sidebarContextSyncFrame = raf(() => {
    sidebarContextSyncFrame = 0
    syncSidebarNavigationContext()
  })
}

// 【源一律写成 getter】这是本组件最容易踩的坑，改动前请读完：
//
// 1. watch 的源如果直接传 `reactive` 对象（例如旧写法里的 `taskCatTrees` /
//    `taskCategoryPaths`），Vue 会**隐式按 deep 订阅**——不是我们显式写的 `deep: true`，
//    而是传对象本身就等于 deep。`taskCatTrees` 里是整棵任务树，于是每次切换模式、
//    每次读到它们，都要把树完整走一遍。实测 2.1 万节点约 150ms、4.2 万节点约 270ms，
//    全部同步阻塞在点击回调里 —— 这就是「切回知识库特别慢」的真正原因。
// 2. 改成 getter 后，Vue 只比较返回值：`() => taskCatTrees[activeTaskCat.value]` 返回数组引用，
//    任务树整体替换（`taskCatTrees[key] = ...`）会触发，节点内部变化不会——正是我们要的粒度。
// 3. 懒加载对 children 的就地赋值仍由 refreshSidebarNavigationContext() 覆盖，见下方说明。
watch(
  [
    viewMode,
    externalDir,
    () => externalDirTree.value,
    activeTaskCat,
    () => taskCategoryPaths.value,
    () => taskCatTrees[activeTaskCat.value],
    () => fileTreeStore.activeRepoPath,
    () => fileTree.value,
  ],
  syncSidebarNavigationContext,
  { immediate: true }
)

// 右键菜单状态
const isContextMenuVisible = ref(false)
const contextMenuPosition = ref({ x: 0, y: 0 })
const selectedNode = ref(null) // 当前右键点击的节点
const menuMode = ref('node') // 'node' = 右键某节点；'blank' = 右键空白处

// 已展开的目录
const expandedKeys = ref([])
const unlistenRef = ref(null)

const selectedKeys = ref([]); // 新增：存储当前选中的节点keys

watch(
  () => fileTreeStore.openTabs.length,
  (len) => {
    if (len !== 0) return;
    lastSelectedKey.value = null;
    selectedKeys.value = [];
  }
)

const ensureExpandedForDir = (dirPath) => {
  const active = normalizePath(fileTreeStore.activeRepoPath)
  const target = normalizePath(dirPath)
  if (!target) return

  if (!active || !target.startsWith(active)) {
    if (!expandedKeys.value.includes(target)) expandedKeys.value.push(target)
    return
  }

  const rel = target.slice(active.length).replace(/^\/+/, '')
  if (!rel) return

  const parts = rel.split('/').filter(Boolean)
  let acc = active
  for (const part of parts) {
    acc = `${acc}/${part}`
    if (!expandedKeys.value.includes(acc)) expandedKeys.value.push(acc)
  }
}

const scrollToKey = async (key, align = 'center') => {
  await nextTick()
  await new Promise((r) => requestAnimationFrame(r))

  const tree = activeTreeRef.value
  if (!tree || typeof tree.scrollTo !== 'function') return

  try {
    await Promise.resolve(tree.scrollTo({ key, align }))
  } catch {
    return
  }
}

const safeCssEscape = (v) => {
  const s = String(v ?? '')
  try {
    if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(s)
  } catch {
    return s.replace(/"/g, '\\"')
  }
  return s.replace(/"/g, '\\"')
}

const focusEditingInput = async (nodeId) => {
  const id = normalizePath(nodeId)
  for (let i = 0; i < 8; i += 1) {
    await nextTick()
    await new Promise((r) => requestAnimationFrame(r))

    const selector = `.editable-input[data-node-id="${safeCssEscape(id)}"]`
    const root = fileTreeContainerRef.value
    const input = (root && root.querySelector(selector)) || document.querySelector(selector)
    if (input) {
      input.focus()
      input.select()
      return true
    }
  }
  return false
}

const importTemplateModalVisible = ref(false)
const importTemplateLoading = ref(false)
const importTemplateName = ref('')
const importTemplateType = ref('未分类')
const importTemplateSourcePath = ref('')

const basename = (p) => String(p || '').split(/[\\/]/).pop() || ''
const stemname = (p) => {
  const b = basename(p)
  const idx = b.lastIndexOf('.')
  return idx > 0 ? b.slice(0, idx) : b
}

const openImportTemplateModal = async () => {
  const node = selectedNode.value
  if (!node || node.is_directory) return

  const nodePath = normalizePath(node.id)
  if (!isMarkdownFilePath(nodePath)) {
    message.warning('仅支持 Markdown 文件')
    return
  }

  importTemplateSourcePath.value = nodePath
  importTemplateName.value = stemname(nodePath)
  importTemplateType.value = '未分类'
  importTemplateModalVisible.value = true
}

const confirmImportAsTemplate = async () => {
  const sourcePath = importTemplateSourcePath.value
  const name = importTemplateName.value.trim()
  const templateType = importTemplateType.value.trim() || '未分类'

  if (!sourcePath) return
  if (!name) {
    message.warning('请输入模板名称')
    return
  }

  try {
    importTemplateLoading.value = true
    await invoke('import_template_from_file', {
      sourcePath,
      name,
      templateType
    })
    message.success('已导入到模板库')
    importTemplateModalVisible.value = false
  } catch (error) {
    logger.error('导入模板失败:', error)
    message.error('导入模板失败')
  } finally {
    importTemplateLoading.value = false
  }
}

// 右键菜单选项
const contextMenuOptions = ref([
  {
    label: '新建文件',
    key: 'addFile',
    icon: () => h(NIcon, null, { default: () => h(AddOutline) })
  },
  {
    label: '新建文件夹',
    key: 'addFolder',
    icon: () => h(NIcon, null, { default: () => h(FolderOpenOutline) })
  },
  {
    label: '导入Markdown文件',
    key: 'importAsTemplate',
    disabled: true,
    icon: () => h(NIcon, null, { default: () => h(DocumentTextOutline) })
  },
  {
    label: '在一侧打开',
    key: 'openToSide',
    icon: () => h(NIcon, null, { default: () => h(BrowsersOutline) })
  },
  {
    label: '选择以进行比较',
    key: 'markForCompare',
    icon: () => h(NIcon, null, { default: () => h(GitCompareOutline) })
  },
  {
    // 标签文案在 handleNodeContextMenu 里按已选的那一方动态改写；没选过就整项隐藏
    label: '与已选比较',
    key: 'compareWithPending',
    show: false,
    icon: () => h(NIcon, null, { default: () => h(SwapHorizontalOutline) })
  },
  {
    label: '复制路径',
    key: 'copyPath',
    icon: () => h(NIcon, null, { default: () => h(CopyOutline) })
  },
  {
    label: '复制相对路径',
    key: 'copyRelativePath',
    icon: () => h(NIcon, null, { default: () => h(DuplicateOutline) })
  },
  {
    label: '重命名',
    key: 'rename',
    icon: () => h(NIcon, null, { default: () => h(CreateOutline) })
  },
  {
    label: '删除',
    key: 'delete',
    icon: () => h(NIcon, null, { default: () => h(TrashOutline) })
  },
  {
    label: '移动到',
    key: 'move',
    icon: () => h(NIcon, null, { default: () => h(MoveOutline) })
  },
  {
    label: '在文件浏览器中打开',
    key: 'openInExplorer',
    icon: () => h(NIcon, null, { default: () => h(FolderOutline) })
  },
  {
    label: '用 Neovim 打开',
    key: 'openInNeovim',
    icon: renderNeovimIcon
  },
  {
    label: '在浏览器中查看',
    key: 'openInBrowser',
    disabled: true,
    icon: () => h(NIcon, null, { default: () => h(GlobeOutline) })
  },
])

// 空白处右键只保留"新建文件/新建文件夹"
const activeContextMenuOptions = computed(() => {
  if (menuMode.value === 'blank') {
    return contextMenuOptions.value.filter(o => o.key === 'addFile' || o.key === 'addFolder')
  }
  return contextMenuOptions.value
})

const isHiddenPath = (path) => {
  if (!path) return false;
  const normalized = normalizePath(path);
  // 设计说明：
  // 1. 点目录仍然作为内部目录隐藏，维持原有文件树洁净度。
  // 2. `.cellmeta` 是 Notebook/探索者模式的侧车元数据文件，需要保留在磁盘和 Git 中，
  //    但不应作为普通文档出现在左侧文件树里。
  return isCellmetaPath(normalized) || normalized.split('/').some(part => part.startsWith('.'));
};

const isCellmetaPath = (path) => normalizePath(path).toLowerCase().endsWith('.cellmeta')

const getCellmetaAutoCommitRepos = () => {
  const repos = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  const active = fileTreeStore.activeRepoPath ? [fileTreeStore.activeRepoPath] : []
  return Array.from(new Set([...active, ...repos].map(p => normalizePath(p)).filter(Boolean)))
}

const autoCommitCellmetaChanges = async (reason = 'manual') => {
  if (cellmetaAutoCommitRunning) {
    logger.debug('[CellmetaAutoCommit] skip overlapping check', { reason })
    return
  }

  const repos = getCellmetaAutoCommitRepos()
  if (!repos.length) {
    logger.debug('[CellmetaAutoCommit] no repository available', { reason })
    return
  }

  cellmetaAutoCommitRunning = true
  logger.info('[CellmetaAutoCommit] start checking metadata sidecars', { reason, repoCount: repos.length })
  let hasError = false
  try {
    for (const repoPath of repos) {
      try {
        const result = await invoke('auto_commit_cellmeta_changes', { path: repoPath })
        if (result?.committed) {
          logger.info('[CellmetaAutoCommit] committed metadata sidecars', {
            repoPath,
            pathCount: result.path_count,
            reason
          })
        } else {
          logger.debug('[CellmetaAutoCommit] no metadata commit needed', {
            repoPath,
            pathCount: result?.path_count ?? 0,
            reason
          })
        }
      } catch (error) {
        logger.warn('[CellmetaAutoCommit] failed to auto commit metadata sidecars', {
          repoPath,
          reason,
          error: String(error?.message || error || '')
        })
        hasError = true
      }
    }
  } finally {
    cellmetaAutoCommitRunning = false
    if (!hasError) {
      cellmetaAutoCommitPending = false
    }
  }
}

const scheduleCellmetaAutoCommit = (reason = 'scheduled', delay = CELLMETA_AUTO_COMMIT_CLOSE_DELAY_MS) => {
  if (cellmetaAutoCommitTimerId) {
    clearTimeout(cellmetaAutoCommitTimerId)
    cellmetaAutoCommitTimerId = null
  }

  logger.debug('[CellmetaAutoCommit] scheduled metadata check', { reason, delay })
  cellmetaAutoCommitTimerId = setTimeout(() => {
    cellmetaAutoCommitTimerId = null
    autoCommitCellmetaChanges(reason).catch((error) => {
      logger.warn('[CellmetaAutoCommit] scheduled check failed', {
        reason,
        error: String(error?.message || error || '')
      })
    })
  }, delay)
}

const scheduleCellmetaAutoCommitAfterFileClose = (payload = {}) => {
  cellmetaAutoCommitPending = true
  logger.info('[CellmetaAutoCommit] file closed; queue async metadata commit', {
    path: payload?.path || '',
    fileName: payload?.fileName || '',
    dirty: !!payload?.dirty,
    delay: CELLMETA_AUTO_COMMIT_CLOSE_DELAY_MS
  })
  scheduleCellmetaAutoCommit('file-close', CELLMETA_AUTO_COMMIT_CLOSE_DELAY_MS)
}

const startCellmetaAutoCommitScheduler = () => {
  if (cellmetaAutoCommitIntervalId) clearInterval(cellmetaAutoCommitIntervalId)
  logger.info('[CellmetaAutoCommit] start periodic metadata checker', {
    intervalMs: CELLMETA_AUTO_COMMIT_INTERVAL_MS,
    closeDelayMs: CELLMETA_AUTO_COMMIT_CLOSE_DELAY_MS
  })
  cellmetaAutoCommitIntervalId = setInterval(() => {
    scheduleCellmetaAutoCommit('periodic-check', 0)
  }, CELLMETA_AUTO_COMMIT_INTERVAL_MS)
}

const normalizeNewMarkdownFileName = (rawName) => {
  const name = String(rawName || '').trim();
  if (!name) return '';

  const lower = name.toLowerCase();
  if (lower.endsWith('.md')) return name;

  if (lower.endsWith('.markdown')) {
    const next = name.slice(0, -'.markdown'.length) + '.md';
    logger.info('[NewFile] normalize .markdown -> .md', { input: name, output: next });
    return next;
  }

  const lastDot = name.lastIndexOf('.');
  if (lastDot > 0 && lastDot < name.length - 1) {
    const base = name.slice(0, lastDot);
    const next = `${base}.md`;
    logger.info('[NewFile] replace extension -> .md', { input: name, output: next });
    return next;
  }

  return `${name}.md`;
};

const makeUniqueFileName = (existingNames, desiredName) => {
  // 设计说明：文件树创建/导入文件时必须区分大小写，允许 README.md 与 readme.md 这类名称共存。
  // 这里保留原始大小写做精确匹配；真正能否落盘由底层文件系统的大小写敏感能力决定。
  const set = new Set((existingNames || []).map((x) => String(x || '')));
  const original = String(desiredName || '').trim();
  if (!original) return '';

  if (!set.has(original)) return original;

  const dot = original.lastIndexOf('.');
  const hasExt = dot > 0 && dot < original.length - 1;
  const base = hasExt ? original.slice(0, dot) : original;
  const ext = hasExt ? original.slice(dot) : '';

  for (let i = 2; i < 2000; i += 1) {
    const next = `${base} (${i})${ext}`;
    if (!set.has(next)) return next;
  }

  return `${base} (${Date.now()})${ext}`;
};

const guessImportTargetDir = (repoPath) => {
  let parentPath = repoPath;

  if (selectedKeys.value.length > 0) {
    const selectedKey = selectedKeys.value[0];
    const selectedNode = findNodeById(fileTree.value, selectedKey);
    if (selectedNode?.is_directory) {
      parentPath = selectedNode.id;
    }
  }

  return normalizePath(parentPath);
};

/**
 * 知识库导入。
 *
 * @param {string} repoPath 知识库根（用于定位目标目录）
 * @param {'copy'|'convert'} action
 *   copy —— 原样导入（Markdown 直接入库；其它二进制原样拷贝）；
 *   convert —— 转换为 Markdown 后入库（引擎路由在 Rust 侧完成）。
 *
 * copy 分支保持与改造前一致的行为，新能力只落在 convert 分支，
 * 因此既有用户的导入结果不会发生变化。
 */
const handleImportMarkdownFile = async (repoPath, action = 'copy') => {
  const targetDir = guessImportTargetDir(repoPath);
  if (!targetDir) {
    message.warning('未找到导入目标目录');
    return;
  }

  try {
    // 不再限制文件类型：Markdown 直接入库，其它格式先经 pandoc 转成 Markdown 再入库。
    const picked = await open({ multiple: false });

    if (!picked) return;

    const srcPath = normalizePath(picked);
    const fileName = srcPath.split('/').pop();
    if (!fileName) {
      message.error('导入失败：无法解析文件名');
      return;
    }

    const dirNodes = await invoke('get_file_tree', { path: targetDir });
    const existingNames = Array.isArray(dirNodes) ? dirNodes.map((n) => n?.name).filter(Boolean) : [];

    if (action === 'convert') {
      await importAsConvertedMarkdown(srcPath, fileName, targetDir, existingNames);
      return;
    }

    if (action === 'copy' && !isMarkdownFilePath(srcPath)) {
      // 原样导入：直接拷贝原始文件（二进制/文档保持原格式，不改内容）。
      const uniqueName = makeUniqueFileName(existingNames, fileName);
      const dstPath = normalizePath(`${targetDir}/${uniqueName}`);
      await invoke('copy_path', { sourcePath: srcPath, destinationPath: dstPath });
      await refreshDirectory(targetDir);
      logger.info('[ImportFile] raw copy import success', { srcPath, dstPath });
      message.success('已原样导入');
      return;
    }

    if (isMarkdownFilePath(srcPath)) {
      // Markdown 文件：保持原有“读内容 → 写入目标目录”的直接入库行为。
      const uniqueName = makeUniqueFileName(existingNames, fileName);
      const dstPath = normalizePath(`${targetDir}/${uniqueName}`);

      logger.info('[ImportFile] markdown direct import', {
        repoPath: normalizePath(repoPath),
        targetDir,
        srcPath,
        dstPath
      });

      const content = await invoke('read_file_content', { path: srcPath });
      await invoke('write_file_content', { path: dstPath, content: String(content ?? '') });

      await refreshDirectory(targetDir);

      logger.info('[ImportFile] markdown import success', { dstPath, len: String(content ?? '').length });
      message.success('Markdown 文件已导入');
    } else {
      // 非 Markdown 文件：与导出链路共用同一 pandoc，转换为 Markdown 后再入库。
      // 先拦截 pandoc 无法读取的输入（PDF/图片等），给出明确提示而非抛出 pandoc 失败。
      if (isPandocUnsupportedImport(srcPath)) {
        logger.info('[ImportFile] skip pandoc-unsupported input', { srcPath });
        message.warning('暂不支持在此导入 PDF / 图片，请使用仓库导入功能（OCR 转换）');
        return;
      }

      const desiredMdName = `${stemname(fileName)}.md`;
      const uniqueMdName = makeUniqueFileName(existingNames, desiredMdName);
      const dstPath = normalizePath(`${targetDir}/${uniqueMdName}`);

      logger.info('[ImportFile] convert via pandoc', {
        repoPath: normalizePath(repoPath),
        targetDir,
        srcPath,
        dstPath
      });

      const loading = message.loading('正在转换为 Markdown…', { duration: 0 });
      try {
        await invoke('convert_file_to_markdown_pandoc', { inputPath: srcPath, outputPath: dstPath });
      } finally {
        loading?.destroy?.();
      }

      await refreshDirectory(targetDir);

      logger.info('[ImportFile] pandoc import success', { dstPath });
      message.success('已转换为 Markdown 并导入');
    }
  } catch (error) {
    logger.error('[ImportFile] failed:', error);
    message.error(`导入失败: ${error.message || error}`);
    await refreshDirectory(targetDir);
  }
};

/**
 * 「转换为 Markdown 后导入」。
 *
 * 统一走 `convert_document_to_markdown`，覆盖 pandoc 读不了的格式
 * （PDF、图片 OCR、旧版 Office、电子书、表格）；引擎路由在 Rust 侧完成。
 */
const importAsConvertedMarkdown = async (srcPath, fileName, targetDir, existingNames) => {
  const desiredMdName = `${stemname(fileName)}.md`;
  const uniqueMdName = makeUniqueFileName(existingNames, desiredMdName);
  const dstPath = normalizePath(`${targetDir}/${uniqueMdName}`);
  const loading = message.loading('正在转换为 Markdown…', { duration: 0 });

  try {
    const result = await invoke('convert_document_to_markdown', {
      path: srcPath,
      quality: 'fast',
      mediaRoot: null,
    });
    const markdown = String(result?.markdown || '');
    if (!markdown.trim()) throw new Error('转换结果为空');

    await invoke('write_file_content', { path: dstPath, content: markdown });

    // 转换引擎若抽出了图片，整体搬到 md 同级的 `<stem>_assets`，保证相对引用可用。
    const mediaDir = String(result?.mediaDir || '');
    if (mediaDir) {
      const dstMediaDir = normalizePath(`${targetDir}/${stemname(uniqueMdName)}_assets`);
      try {
        await invoke('copy_path', { sourcePath: mediaDir, destinationPath: dstMediaDir });
      } catch (mediaError) {
        logger.warn('[ImportFile] copy converted media failed', {
          mediaDir,
          dstMediaDir,
          error: String(mediaError?.message || mediaError),
        });
      }
    }

    await refreshDirectory(targetDir);
    logger.info('[ImportFile] convert import success', {
      srcPath,
      dstPath,
      engine: result?.engine || '',
    });
    message.success(`已转换为 Markdown 并导入${result?.engine ? `（${result.engine}）` : ''}`);
  } finally {
    loading?.destroy?.();
  }
};

// 修改后的handleAddFile函数
const handleAddFile = async (repoPath) => {
  try {
    const parentPath = guessImportTargetDir(repoPath); // 与导入一致：优先用选中的目录
    await addNode(parentPath, false);
  } catch (error) {
    message.error(`添加文件失败: ${error.message}`);
  }
};

// 修改后的handleAddFolder函数
const handleAddFolder = async (repoPath) => {
  try {
    let parentPath = repoPath; // 默认使用仓库路径

    // 如果有选中的节点，获取其路径
    if (selectedKeys.value.length > 0) {
      const selectedKey = selectedKeys.value[0];
      const selectedNode = findNodeById(fileTree.value, selectedKey);

      // 如果选中的是目录，则使用该目录路径
      if (selectedNode?.is_directory) {
        parentPath = selectedNode.id;
      }
    }

    await addNode(parentPath, true);
  } catch (error) {
    message.error(`添加文件夹失败: ${error.message}`);
  }
};

const closeTabsUnderRepo = (repoPath) => {
  const prefix = normalizePath(repoPath)
  if (!prefix) return
  const tabs = Array.isArray(fileTreeStore.openTabs) ? [...fileTreeStore.openTabs] : []
  for (const tab of tabs) {
    const tabPath = normalizePath(tab?.path)
    if (!tabPath) continue
    if (tabPath === prefix || tabPath.startsWith(`${prefix}/`)) {
      fileTreeStore.closeTab(tab.path)
    }
  }
}

const handleDeleteKnowledgeBase = async (repoPath) => {
  const kbName = getRepoName(repoPath)
  if (!kbName) return

  const ok = await confirmDialog(
    `确定将知识库 “${kbName}” 移入回收站吗？\n\n将把该知识库目录（以及对应元数据）移动到回收站目录，避免误删后无法找回。`,
    {
      title: '移入回收站',
      kind: 'warning',
      okLabel: '移入回收站',
      cancelLabel: '取消'
    }
  )

  if (!ok) return

  try {
    const deletedNorm = normalizePath(repoPath)
    const activeNorm = normalizePath(fileTreeStore.activeRepoPath)

    closeTabsUnderRepo(repoPath)
    logger.info('[KnowledgeBase] move to trash start', { kbName, repoPath })
    await invoke('delete_knowledge_base', {
      input: { knowledge_base_name: kbName }
    })
    logger.info('[KnowledgeBase] move to trash success', { kbName })

    let refreshed = []
    try {
      refreshed = await knowledgeBaseStore.fetchKnowledgeBases()
    } catch {
      refreshed = knowledgeBaseStore.knowledgeBases
    }

    const paths = Array.isArray(refreshed) ? refreshed.map(kb => kb.location) : []
    fileTreeStore.setRepopathlist(paths)

    const activeStillExists = paths.some(p => normalizePath(p) === activeNorm)
    const needResetActive = activeNorm === deletedNorm || !activeStillExists

    if (needResetActive) {
      const nextRepo = paths[0] || null
      fileTreeStore.setActiveRepoPath(nextRepo)

      if (nextRepo) {
        fileTreeStore.expandedRepos.set(nextRepo, true)
        await loadRepoTree(nextRepo)
      } else {
        fileTree.value = []
        expandedKeys.value = []
        selectedKeys.value = []
      }
    }

    message.success('知识库已移入回收站')
  } catch (error) {
    logger.error('移入回收站失败:', error)
    message.error('移入回收站失败')
  }
}

// 获取仓库显示名称
const getRepoName = (path) => {
  const name = path.split(/[\\/]/).pop() || path
  // logger.info('[Util] 获取仓库名称:', { path, name })
  return name
}

const toggleRepo = async (path) => {
  // logger.info('目标路径:', path)
  // logger.info('当前激活路径:', fileTreeStore.activeRepoPath)

  if (fileTreeStore.activeRepoPath === path) {
    // 如果点击的是当前已激活的仓库，则切换展开状态
    fileTreeStore.toggleRepoExpand(path)
  } else {
    // 如果是新仓库，则设置激活并展开
    fileTreeStore.setActiveRepoPath(path)
    fileTreeStore.expandedRepos.set(path, true) // 确保新仓库展开
    await loadRepoTree(path)
  }
}

const loadRepoTree = async (path) => {
  // logger.info('[API] 加载仓库树')
  try {
    // 切换/加载仓库时清空上一个仓库残留的展开/选中状态，
    // 否则虚拟树会按旧 key 把目录显示成"已展开箭头"，但其子节点并未加载。
    expandedKeys.value = []
    selectedKeys.value = []
    // 无论数据是否为空都重新加载
    await loadFileTree()
    // 确保仓库在加载后保持展开状态
    // fileTreeStore.expandedRepos.set(path, false)
  } catch (error) {
    logger.error('[API] 加载仓库树失败:', error)
  }
}

const refreshAll = async () => {
  const activePath = fileTreeStore.activeRepoPath
  if (!activePath) return
  logger.info('[Refresh] 刷新仓库:', activePath)
  await loadRepoTree(activePath)
}

const normalizeTreeIds = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];
  return tree.map(item => {
    const newItem = { ...item, id: normalizePath(item.id) };
    if (item.children) {
      newItem.children = normalizeTreeIds(item.children);
    }
    return newItem;
  });
};

// 加载文件树数据
const loadFileTree = async (options = {}) => {
  const { force = false } = options || {}
  logger.info('[API] 加载文件树');
  isTreeLoading.value = true
  try {
    logger.info('当前仓库路径:', fileTreeStore.activeRepoPath);
    await fileTreeStore.fetchFileTree(fileTreeStore.activeRepoPath, { force });

    const filteredTree = filterHiddenTree(fileTreeStore.fileTree);

    fileTree.value = [...sortFileTree(normalizeTreeIds(filteredTree))];
  } catch (error) {
    logger.error('获取文件树数据失败', error);
    fileTree.value = [];
  } finally {
    isTreeLoading.value = false
  }
};

const filterHiddenTree = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];
  return tree
    .filter(item => !isHiddenPath(item.id))
    .map(item => {
      const next = { ...item };
      if (item.children) {
        next.children = filterHiddenTree(item.children);
      }
      return next;
    });
};

const treeNameCollator = (() => {
  try {
    if (typeof Intl !== 'undefined' && typeof Intl.Collator === 'function') {
      return new Intl.Collator('zh-u-co-pinyin', { numeric: true, sensitivity: 'base' })
    }
  } catch {
    return null
  }
  return null
})()

// 对文件树数据进行排序（目录在上，文件在下；英文按字母，中文按拼音）
const sortFileTree = (tree) => {
  if (!tree || !Array.isArray(tree)) return [];

  const compareName = (aName, bName) => {
    const aStr = String(aName || '')
    const bStr = String(bName || '')
    if (treeNameCollator) return treeNameCollator.compare(aStr, bStr)
    return aStr.localeCompare(bStr)
  }

  return tree
    .map(item => {
      if (item.children) {
        item.children = sortFileTree(item.children);
      }
      return item;
    })
    .sort((a, b) => {
      const aIsDir = !!a.is_directory || a.type === 'directory'
      const bIsDir = !!b.is_directory || b.type === 'directory'
      if (aIsDir !== bIsDir) return aIsDir ? -1 : 1
      return compareName(a.name, b.name)
    });
};

// ─────────── 虚拟文件树事件处理 ───────────

// 打开文件(虚拟树 open-file 事件):等价旧 handleSelect 的文件分支
/**
 * 统一的「打开一个文件」入口（纯增量改造）。
 *
 * 对**文本文件**，这里走的是与改造前完全相同的路径（fetchFileContent → openFileInTab），
 * 一次额外的 IPC 都不会发生 —— openTargetKind 先做纯前端的扩展名判定，
 * 只有非文本才进入阅读器分支。
 *
 * 非文本文件（PDF / Office / 电子书 / 表格 / 图片 / 音视频 / HTML / 二进制）：
 *   1. 在 readerTabStore 登记为一个「阅读器标签」；
 *   2. 仍用 openFileInTab 建标签（内容是占位文本 + unsupported 标记）；
 *   3. GuaxiEditor 里的 ReaderPane 检测到登记后，覆盖在编辑区上显示真实内容。
 *
 * 为什么仍然建标签：这样标签栏、分屏、关闭、拖动排序等既有交互全部照旧，
 * 用户不会遇到「打开了文件却没有标签」的新状态。
 */
/**
 * 打开一个文档。
 *
 * 二进制文档（PDF / 电子书 / Office / 表格 / 图片 / 音视频 / HTML）现在打开在**独立阅读器窗口**里：
 * 阅读器要长出搜索、书签、批注、Office 编辑这些能力，继续挤在编辑器标签页里每加一个功能
 * 都得先证明不碰 Monaco 和自动保存链路；独立窗口有自己的组件树，改它不影响主界面。
 *
 * 保留「在标签页中打开」：`inline` 为 true 时走原来的覆盖层路径。
 * 这是退路而不是主路径 —— 用户从右键菜单显式选择时才走。
 */
const openFileByKind = async (path, { region = '', onMissing = null, inline = false } = {}) => {
  const normalized = normalizePath(path)
  if (!normalized) return false

  let target = null
  try {
    target = await readerTabStore.openReader(normalized, 'auto')
  } catch (error) {
    logger.warn('[OpenFile] reader routing failed, fallback to text path', {
      path: normalized,
      error: String(error?.message || error),
    })
    target = null
  }

  if (target && target.reader !== 'text') {
    // 独立窗口：主编辑器不建标签、不挂覆盖层，完全让位。
    if (!inline) {
      try {
        const fileName = normalized.replace(/\\/g, '/').split('/').filter(Boolean).pop() || ''
        await invoke('create_doc_reader_window', {
          path: normalized,
          name: fileName,
          kind: String(target.kind || ''),
          reader: String(target.reader || ''),
          theme: settingsStore.appearanceTheme,
        })
        logger.info('[OpenFile] opened in doc reader window', {
          path: normalized,
          reader: target.reader,
          kind: target.kind,
        })
        return true
      } catch (error) {
        // 窗口建不起来（极少见）时退回标签页覆盖层，不能让文件打不开。
        logger.warn('[OpenFile] doc reader window failed, fallback to inline reader', {
          path: normalized,
          error: String(error?.message || error),
        })
      }
    }

    const placeholder = `该文件由内嵌阅读器显示（${String(target.reader)}）。

路径：${normalized}`
    await fileTreeStore.openFileInTab(normalized, placeholder, {
      unsupported: true,
      ...(region ? { region } : {}),
    })
    logger.info('[OpenFile] opened via inline reader', {
      path: normalized,
      reader: target.reader,
      kind: target.kind,
    })
    return true
  }

  // ── 文本路径：与原实现逐行一致 ──
  const res = await fileTreeStore.fetchFileContent(normalized)
  if (!res?.ok) {
    message.warning('文件不存在或已被删除')
    if (typeof onMissing === 'function') await onMissing(normalized)
    return false
  }
  await fileTreeStore.openFileInTab(normalized, res.content, {
    unsupported: !!res.unsupported,
    ...(region ? { region } : {}),
  })
  return true
}

const handleOpenFile = async (node) => {
  if (!node) return
  const normalizedKey = normalizePath(node.id)
  if (isHiddenPath(normalizedKey)) return

  selectedKeys.value = [node.id]

  const existingTab = fileTreeStore.openTabs.find(tab => normalizePath(tab.path) === normalizedKey)
  if (existingTab) {
    fileTreeStore.setActiveTab(existingTab.path)
    lastSelectedKey.value = node.id
    return
  }
  lastSelectedKey.value = node.id

  if (editingNode.value || node.isNew) return

  try {
    // 文本文件不存在时沿用旧行为：顺手把已消失的节点从树上摘掉。
    await openFileByKind(node.id, { onMissing: (p) => handleFileRemoved(p) })
  } catch (error) {
    logger.error('处理文件选中时出错:', error)
  }
}

// 目录展开懒加载(虚拟树 load-children 事件):等价旧 handleSelect 的目录分支
const handleLoadChildren = async (node) => {
  if (!node?.is_directory) return
  selectedKeys.value = [node.id]
  if (editingNode.value || node.isNew) return
  try {
    const dirId = normalizePath(node.id)
    await fileTreeStore.fetchFileTree(dirId, { force: true })
    const filteredChildren = filterHiddenTree(fileTreeStore.fileTree)
    node.children = sortFileTree(normalizeTreeIds(filteredChildren))
    refreshSidebarNavigationContext()
    await scrollToKey(dirId)
  } catch (error) {
    logger.error('加载子目录内容失败:', error)
  }
}

// ───── "打开"菜单 / 模式切换 / 外部目录树 ─────
const handleOpenMenuSelect = async (key) => {
  if (key === 'openFile') await openExternalFile()
  else if (key === 'openDir') await openExternalDir()
}

// 打开文件：直接在主编辑区打开用户通过文件浏览器选择的文件
const openExternalFile = async () => {
  try {
    const picked = await open({ directory: false, multiple: false, title: '打开文件' })
    if (!picked) return
    const filePath = Array.isArray(picked) ? picked[0] : picked
    if (!filePath) return
    // 按真实类型分流：文本进 Monaco，其余进内嵌阅读器。
    await openFileByKind(filePath)
  } catch (error) {
    logger.error('打开文件失败:', error)
    message.error('打开文件失败')
  }
}

// 打开目录：把知识编辑区切换为该外部目录对应的文件树，并记入历史
const openExternalDir = async () => {
  try {
    const picked = await open({ directory: true, multiple: false, title: '打开目录' })
    if (!picked) return
    const dirPath = Array.isArray(picked) ? picked[0] : picked
    if (!dirPath) return
    pushOpenHistory(dirPath)
    await activateExternalDir(dirPath)
  } catch (error) {
    logger.error('打开目录失败:', error)
    message.error('打开目录失败')
  }
}

// 激活某个外部目录的文件树（进入 plain 模式并展示其目录树）
const activateExternalDir = async (dirPath) => {
  const norm = normalizePath(dirPath)
  viewMode.value = 'plain'
  externalDir.value = norm
  plainRootExpanded.value = true
  externalExpandedKeys.value = []
  externalSelectedKeys.value = []
  externalDirTree.value = []
  repointFileWatcher(norm)
  try {
    await fileTreeStore.fetchFileTree(norm, { force: true })
    const filtered = filterHiddenTree(fileTreeStore.fileTree)
    externalDirTree.value = sortFileTree(normalizeTreeIds(filtered))
  } catch (error) {
    logger.error('加载外部目录树失败:', error)
    message.error('加载目录失败')
  }
}

// 切换视图模式：kb → plain → task → kb（三态轮转）
// 设计说明：三种模式各自维护独立的树/展开状态（module 级 ref），组件不会随切换卸载，
// 因此切换时不应清空对方状态。之前每次进入 task 都重跑 activateTaskMode 全量重置、
// 且离开 plain 时把 externalDir 置空，导致已展开的目录/文件树被打回默认态。
// 现在：task 仅首次进入时初始化，plain 的激活目录跨模式保留；切换只重定向文件监控。
const toggleViewMode = () => {
  if (viewMode.value === 'kb') {
    viewMode.value = 'plain'
    // 回到文件编辑模式时，把文件监控重新指向其激活的外部目录（若曾打开过）。
    if (externalDir.value) repointFileWatcher(externalDir.value)
  } else if (viewMode.value === 'plain') {
    viewMode.value = 'task'
    if (!taskModeInitialized.value) {
      activateTaskMode()
    } else {
      // 已初始化过：保留已加载/展开的任务树，仅重定向监控并触发一次后台重索引。
      if (taskRootPath.value) repointFileWatcher(taskRootPath.value)
      invoke('task_scan_and_index').catch((e) => logger.warn('任务扫描失败', e))
    }
  } else {
    viewMode.value = 'kb'
    if (fileTreeStore.activeRepoPath) repointFileWatcher(fileTreeStore.activeRepoPath)
  }
}

// 模式切换图标 / 提示 / 标题（三态轮转）
const modeToggleIcon = computed(() => {
  if (viewMode.value === 'kb') return SwapHorizontalOutline   // kb → plain
  if (viewMode.value === 'plain') return CheckboxOutline      // plain → task
  return LibraryOutline                                       // task → kb
})
const modeToggleTooltip = computed(() => {
  if (viewMode.value === 'kb') return '切换到文件编辑模式'
  if (viewMode.value === 'plain') return '切换到任务编辑模式'
  return '切换回知识库模式'
})
const modeTitle = computed(() => {
  if (viewMode.value === 'plain') return '文件编辑'
  if (viewMode.value === 'task') return '任务编辑'
  return '知识编辑'
})

// 进入任务编辑模式：加载三类目录路径，默认展开「工作」并加载其 .org 文件树
const activateTaskMode = async () => {
  try {
    const dirs = await invoke('task_ensure_dirs')
    taskRootPath.value = dirs?.root || ''
    const cats = dirs?.categories || {}
    taskCategoryPaths.value = {
      work: cats.work || '',
      life: cats.life || '',
      study: cats.study || '',
    }
  } catch (error) {
    logger.error('初始化任务目录失败:', error)
    message.error('初始化任务目录失败')
    return
  }
  // 触发一次后端扫描索引（看板/统计数据源）
  invoke('task_scan_and_index').catch((e) => logger.warn('任务扫描失败', e))
  // 监控任务根目录，文件变动时刷新任务树
  repointFileWatcher(taskRootPath.value)

  // 重置三类树，默认激活「工作」并展开
  for (const c of TASK_CATEGORIES) {
    taskCatTrees[c.key] = []
    taskCatExpandedKeys[c.key] = []
    taskCatSelectedKeys[c.key] = []
  }
  activeTaskCat.value = 'work'
  taskActiveExpanded.value = true
  await loadTaskCat('work')
  taskModeInitialized.value = true
}

// 加载某分类根目录下的 .org 文件树
const loadTaskCat = async (key) => {
  const dir = catPathOf(key)
  if (!dir) return
  try {
    taskCatTrees[key] = await fetchDirChildren(dir, true)
  } catch (error) {
    logger.error('加载任务分类失败:', error)
  }
}

// 激活某分类（占满整列并展开，首次加载其文件树）——等价知识库模式激活一个仓库
const activateTaskCat = async (key) => {
  activeTaskCat.value = key
  taskActiveExpanded.value = true
  if (!taskCatTrees[key] || !taskCatTrees[key].length) {
    await loadTaskCat(key)
  }
}

// 点击激活分类标题：展开/收缩（收缩后其它分类以列表显示在其后）
const toggleActiveTaskCat = () => {
  taskActiveExpanded.value = !taskActiveExpanded.value
}

// 过滤出目录与 .org 文件（目录保留以便逐层展开）
const filterOrgTree = (nodes) => {
  if (!Array.isArray(nodes)) return []
  const out = []
  for (const n of nodes) {
    if (n.is_directory) {
      out.push({ ...n, children: filterOrgTree(n.children || []) })
    } else if (String(n.name || n.id || '').toLowerCase().endsWith('.org')) {
      out.push(n)
    }
  }
  return out
}

// 任务模式：懒加载子目录（更新该节点 children）
const handleTaskLoadChildren = async (node) => {
  if (!node?.is_directory) return
  try {
    node.children = await fetchDirChildren(node.id, true)
    refreshSidebarNavigationContext()
  } catch (error) {
    logger.error('加载任务子目录失败:', error)
  }
}

// 任务模式：在主编辑区打开 .org 文件
const handleTaskOpenFile = async (node) => {
  if (!node || node.is_directory) return
  try {
    await openFileByKind(node.id)
  } catch (error) {
    logger.error('打开任务文件失败:', error)
  }
}

// plain 模式下点击历史条目，激活其文件树
const handleHistoryItemClick = async (path) => {
  pushOpenHistory(path)
  await activateExternalDir(path)
}

// plain 模式：返回历史列表（清掉当前激活的外部目录）
const backToHistory = () => {
  externalDir.value = null
  externalDirTree.value = []
}

// ───── 文件编辑模式 / 任务模式：通用文件操作（新建/导入/重命名/删除）+ 右键菜单 ─────
// 设计：plain/task 两种模式的文件树不复用 kb 的内联编辑机制（其深度绑定 fileTree/activeRepoPath），
// 改用轻量的命名弹窗 + 同一套后端命令（create_file_with_content / create_directory / move_path /
// delete_path / copy_path / open_in_file_explorer），并在操作后增量刷新对应子树。

const parentDirOf = (id) => normalizePath(id).split('/').slice(0, -1).join('/')

// 命名输入弹窗
const nameModal = ref({ visible: false, title: '', value: '', placeholder: '', resolve: null })
const promptName = (title, initial = '', placeholder = '') => {
  return new Promise((resolve) => {
    nameModal.value = { visible: true, title, value: initial, placeholder, resolve }
  })
}
const confirmNameModal = () => {
  const r = nameModal.value.resolve
  const v = (nameModal.value.value || '').trim()
  nameModal.value.visible = false
  if (r) r(v || null)
}
const cancelNameModal = () => {
  const r = nameModal.value.resolve
  nameModal.value.visible = false
  if (r) r(null)
}

// 抓取某目录的子节点（可选只保留目录与 .org）
const fetchDirChildren = async (dirPath, orgOnly = false) => {
  await fileTreeStore.fetchFileTree(normalizePath(dirPath), { force: true })
  let nodes = sortFileTree(normalizeTreeIds(filterHiddenTree(fileTreeStore.fileTree)))
  if (orgOnly) nodes = filterOrgTree(nodes)
  return nodes
}

// 合并刷新某目录的直接子项：保留仍存在目录节点上已懒加载的 children，避免
// 整树替换把展开态打回默认（与 kb 模式 refreshDirectory 的复用策略一致）。
// - prevChildren: 旧的直接子项数组（可能含已加载 children 的目录节点）
// - freshChildren: 重新拉取的直接子项（目录 children 为空，需懒加载）
// 返回合并后的新数组：逐项复用旧目录节点（含其 children），仅更新顶层条目。
const mergeDirChildren = (prevChildren, freshChildren) => {
  const prevList = Array.isArray(prevChildren) ? prevChildren : []
  const prevById = new Map()
  for (const n of prevList) {
    const k = normalizePath(n?.id)
    if (k) prevById.set(k, n)
  }
  return freshChildren.map((fresh) => {
    const prev = prevById.get(normalizePath(fresh.id))
    if (!prev) return fresh
    // 目录节点：保留旧 children（已懒加载的子树），仅刷新自身存在性；
    // 文件节点没有 children，直接复用旧引用以避免编辑态/选中态丢失。
    const next = { ...prev, ...fresh }
    if (prev.is_directory && Array.isArray(prev.children)) {
      next.children = prev.children
    }
    return next
  })
}

// 刷新指定模式下某目录的子树（合并式，保留展开态与已加载子树）
// options.expand：刷新后是否顺带把目标目录展开。
// 文件系统事件（保存 / 外部改动）走 expand=false——刷新目录内容不应改变用户的展开/收起状态；
// 新建、导入、重命名、删除这些用户显式操作仍按默认 true，保持原有行为。
const refreshModeDir = async (mode, dirPath, options = {}) => {
  const { expand = true } = options
  const target = normalizePath(dirPath)
  if (mode === 'plain') {
    const root = normalizePath(externalDir.value)
    if (!root) return
    if (!target || target === root) {
      // 根目录：合并刷新顶层，保留已展开子目录的懒加载 children
      const fresh = await fetchDirChildren(root)
      externalDirTree.value = mergeDirChildren(externalDirTree.value, fresh)
      return
    }
    const node = findNodeById(externalDirTree.value, target)
    if (node) {
      const fresh = await fetchDirChildren(target)
      node.children = mergeDirChildren(node.children, fresh)
      refreshSidebarNavigationContext()
      if (expand && !externalExpandedKeys.value.includes(target)) {
        externalExpandedKeys.value = [...externalExpandedKeys.value, target]
      }
    } else {
      const fresh = await fetchDirChildren(root)
      externalDirTree.value = mergeDirChildren(externalDirTree.value, fresh)
    }
  } else {
    // 任务模式：定位到所属分类树
    const catKey = taskCatOfPath(target)
    if (catKey) {
      const catPath = catPathOf(catKey)
      if (!catPath) return
      if (target === catPath) {
        const fresh = await fetchDirChildren(catPath, true)
        taskCatTrees[catKey] = mergeDirChildren(taskCatTrees[catKey], fresh)
      } else {
        const node = findNodeById(taskCatTrees[catKey], target)
        if (node) {
          const fresh = await fetchDirChildren(target, true)
          node.children = mergeDirChildren(node.children, fresh)
          refreshSidebarNavigationContext()
          if (expand && !taskCatExpandedKeys[catKey].includes(target)) {
            taskCatExpandedKeys[catKey] = [...taskCatExpandedKeys[catKey], target]
          }
        } else {
          const fresh = await fetchDirChildren(catPath, true)
          taskCatTrees[catKey] = mergeDirChildren(taskCatTrees[catKey], fresh)
        }
      }
    }
    // 任务数据变更 → 后端重索引，刷新看板/统计
    invoke('task_scan_and_index').catch((e) => logger.warn('任务重索引失败', e))
  }
}

// 判断路径属于哪个任务分类
const taskCatOfPath = (p) => {
  const n = normalizePath(p)
  for (const c of TASK_CATEGORIES) {
    const cp = catPathOf(c.key)
    if (cp && (n === cp || n.startsWith(`${cp}/`))) return c.key
  }
  return null
}

// === 行内新建（与知识库模式一致：在树中插入可编辑临时节点，而非弹窗）===
// plain / task 模式共用 editingNode / editName / editingId（同一时刻只有一个行内编辑），
// 通过 inlineCreateCtx 记录“在哪个模式/父目录下新建”，提交时据此落盘并刷新对应树。
const inlineCreateCtx = ref(null)
// 行内重命名上下文（与新建互斥）：记录“在哪个模式下重命名哪个已存在节点”。
const inlineRenameCtx = ref(null)

const pruneNodeById = (arr, id) =>
  (Array.isArray(arr) ? arr : []).filter((n) => {
    if (normalizePath(n.id) === id) return false
    if (n.children) n.children = pruneNodeById(n.children, id)
    return true
  })

const insertInlineTempNode = (mode, parentDir, isDirectory) => {
  const tempName = isDirectory ? '新建文件夹' : '新建文件'
  const baseDir = normalizePath(parentDir)
  const node = {
    id: `${baseDir}/${tempName}`,
    name: tempName,
    is_directory: isDirectory,
    children: isDirectory ? [] : undefined,
    isNew: true,
  }
  if (mode === 'plain') {
    const root = normalizePath(externalDir.value)
    if (baseDir === root) {
      externalDirTree.value = [node, ...externalDirTree.value]
    } else {
      const parent = findNodeById(externalDirTree.value, baseDir)
      if (parent) {
        parent.children = [node, ...(parent.children || [])]
        if (!externalExpandedKeys.value.includes(baseDir)) externalExpandedKeys.value = [...externalExpandedKeys.value, baseDir]
      } else {
        externalDirTree.value = [node, ...externalDirTree.value]
      }
    }
  } else {
    const cat = taskCatOfPath(baseDir) || activeTaskCat.value
    const root = catPathOf(cat)
    if (baseDir === root) {
      taskCatTrees[cat] = [node, ...(taskCatTrees[cat] || [])]
    } else {
      const parent = findNodeById(taskCatTrees[cat], baseDir)
      if (parent) {
        parent.children = [node, ...(parent.children || [])]
        if (!taskCatExpandedKeys[cat].includes(baseDir)) taskCatExpandedKeys[cat] = [...taskCatExpandedKeys[cat], baseDir]
      } else {
        taskCatTrees[cat] = [node, ...(taskCatTrees[cat] || [])]
      }
    }
  }
  refreshSidebarNavigationContext()
  return node
}

const removeInlineTempNode = (mode, node) => {
  const id = normalizePath(node?.id || '')
  if (!id) return
  if (mode === 'plain') {
    externalDirTree.value = pruneNodeById(externalDirTree.value, id)
  } else {
    const cat = taskCatOfPath(id) || activeTaskCat.value
    taskCatTrees[cat] = pruneNodeById(taskCatTrees[cat] || [], id)
  }
}

// 选中某模式下的节点（只作用于该模式已渲染的树，不切换分类/仓库）
const selectInMode = (mode, rawPath) => {
  const target = normalizePath(rawPath)
  if (!target) return
  if (mode === 'plain') {
    externalSelectedKeys.value = [target]
    return
  }
  const cat = taskCatOfPath(target) || activeTaskCat.value
  if (taskCatSelectedKeys[cat]) taskCatSelectedKeys[cat] = [target]
}

// 行内新建后先把新行滚入视口：虚拟列表只渲染可视区附近的行，节点在视口外时
// input 根本没被渲染，focusEditingInput 找不到元素就无从聚焦，用户只能自己滚动过去。
const scrollModeToKey = async (mode, key) => {
  const tree = treeRefForMode(mode)
  if (!tree || typeof tree.scrollTo !== 'function') return

  await nextTick()
  await new Promise((r) => requestAnimationFrame(r))

  try {
    await Promise.resolve(tree.scrollTo({ key: normalizePath(key), align: 'nearest' }))
  } catch (e) {
    logger.debug('[TreeScroll] scrollTo failed', e)
  }
}

const beginInlineCreate = async ({ mode, parentDir, isDirectory }) => {
  if (inlineCreateCtx.value) cancelInlineCreate()
  // 先把目标所在的树显示出来：分类收起或未激活时，行内新建的节点建在一棵没渲染的树里，
  // 既看不见也编辑不了（task 的未激活分类还会因为是空数组而只加载出这一个新节点）。
  if (mode === 'plain') {
    plainRootExpanded.value = true
  } else if (mode === 'task') {
    const cat = taskCatOfPath(parentDir) || activeTaskCat.value
    if (activeTaskCat.value !== cat) await activateTaskCat(cat)
    taskActiveExpanded.value = true
  }
  const node = insertInlineTempNode(mode, parentDir, isDirectory)
  inlineCreateCtx.value = { mode, parentDir: normalizePath(parentDir), isDirectory, node }
  startEditing(node)
  selectInMode(mode, node.id)
  await scrollModeToKey(mode, node.id)
  await focusEditingInput(node.id)
}

const handleInlineCommit = async () => {
  // 重命名优先：当前若处于行内重命名态，走改名提交路径。
  if (inlineRenameCtx.value) { await commitInlineRename(); return }
  const ctx = inlineCreateCtx.value
  if (!ctx) { editingNode.value = null; editName.value = ''; return }
  const name = (editName.value || '').trim()
  if (!name || name === '新建文件' || name === '新建文件夹') {
    cancelInlineCreate()
    return
  }
  const { mode, parentDir, isDirectory } = ctx
  const path = normalizePath(`${parentDir}/${name}`)
  try {
    if (isDirectory) await invoke('create_directory', { path })
    else await invoke('create_file_with_content', { path, content: '' })
    inlineCreateCtx.value = null
    editingNode.value = null
    editName.value = ''
    await refreshModeDir(mode, parentDir)
    // 刷新合并后行会重建：把选中态与视口落到真实节点上，光标即停在刚建出的文件/目录上。
    selectInMode(mode, path)
    await scrollModeToKey(mode, path)
    if (!isDirectory) {
      try {
        const res = await fileTreeStore.fetchFileContent(path)
        if (res?.ok) await fileTreeStore.openFileInTab(path, res.content, { unsupported: !!res.unsupported })
      } catch {}
    }
    message.success(isDirectory ? '已新建文件夹' : '已新建文件')
  } catch (e) {
    logger.error('新建失败:', e)
    message.error('新建失败: ' + (e?.message || e))
    cancelInlineCreate()
  }
}

const cancelInlineCreate = () => {
  // 重命名取消：仅退出编辑态，不动节点本身。
  if (inlineRenameCtx.value) {
    inlineRenameCtx.value = null
    editingNode.value = null
    editName.value = ''
    return
  }
  const ctx = inlineCreateCtx.value
  if (ctx) removeInlineTempNode(ctx.mode, ctx.node)
  inlineCreateCtx.value = null
  editingNode.value = null
  editName.value = ''
}

const genAddFile = async (mode, dir) => {
  await beginInlineCreate({ mode, parentDir: dir, isDirectory: false })
}

const genAddFolder = async (mode, dir) => {
  await beginInlineCreate({ mode, parentDir: dir, isDirectory: true })
}

/**
 * 文件 / 任务模式的导入。
 *
 * @param {'copy'|'convert'} action
 *   copy（默认）—— 原样拷贝，行为与改造前完全一致；
 *   convert —— 逐个转换为 Markdown 后入库（PDF/OCR/旧版 Office/电子书/表格都能处理）。
 *
 * 默认值是 'copy'，所以右键菜单与快捷键等旧入口不传 action 时仍走原路径。
 */
const genImportFile = async (mode, dir, action = 'copy') => {
  const picked = await open({ multiple: true, title: '导入文件' })
  if (!picked) return
  const list = Array.isArray(picked) ? picked : [picked]

  if (action === 'convert') {
    let ok = 0
    let failed = 0
    const loading = message.loading('正在转换为 Markdown…', { duration: 0 })
    try {
      const dirNodes = await invoke('get_file_tree', { path: dir }).catch(() => [])
      const existing = new Set(
        (Array.isArray(dirNodes) ? dirNodes : []).map((n) => String(n?.name || '')).filter(Boolean)
      )
      for (const src of list) {
        const name = String(src).split(/[\\/]/).filter(Boolean).pop()
        if (!name) continue
        try {
          const result = await invoke('convert_document_to_markdown', {
            path: src,
            quality: 'fast',
            mediaRoot: null,
          })
          const markdown = String(result?.markdown || '')
          if (!markdown.trim()) throw new Error('转换结果为空')
          const base = /\.(md|markdown)$/i.test(name) ? name : `${stemname(name)}.md`
          const unique = makeUniqueFileName(Array.from(existing), base)
          existing.add(unique)
          const dst = normalizePath(`${dir}/${unique}`)
          await invoke('write_file_content', { path: dst, content: markdown })
          ok += 1
        } catch (e) {
          failed += 1
          logger.error('转换导入失败:', { src, error: String(e?.message || e) })
        }
      }
    } finally {
      loading?.destroy?.()
    }
    await refreshModeDir(mode, dir)
    if (failed) message.warning(`已转换导入 ${ok} 个文件，${failed} 个失败`)
    else message.success(`已转换导入 ${ok} 个文件`)
    return
  }

  // ── 原样导入：与改造前逐行一致 ──
  let ok = 0
  for (const src of list) {
    const name = String(src).split(/[\\/]/).filter(Boolean).pop()
    if (!name) continue
    const dest = normalizePath(`${dir}/${name}`)
    try {
      await invoke('copy_path', { sourcePath: src, destinationPath: dest })
      ok++
    } catch (e) {
      logger.error('导入文件失败:', e)
    }
  }
  await refreshModeDir(mode, dir)
  message.success(`已导入 ${ok} 个文件`)
}

// 文件/任务模式重命名：与知识库模式一致，选中节点直接进入行内编辑态，
// 回车提交（commitInlineRename）、Esc 取消（cancelInlineCreate），不再弹出对话框。
const genRename = async (mode, node) => {
  if (inlineCreateCtx.value) cancelInlineCreate()
  inlineRenameCtx.value = { mode, node, oldId: normalizePath(node.id) }
  startEditing(node)
  await nextTick()
  await focusEditingInput(node.id)
}

// ===== 文件路径变更（重命名/移动/删除）的统一处理 =====
// 这部分逻辑以前分散在多个入口里，且只更新 openTabs、不更新分屏路径列表，
// 也从不暂停写盘：重命名后 in-flight 的自动保存会按旧路径把文件写回来。
// 现在统一走下面三个工具函数。

/** 执行一次路径变更：先让编辑器落地防抖编辑，再暂停写盘，完成（成功或失败）后恢复。 */
const runPathMutation = async (mutation, operation) => {
  await runFilePathMutationGuards(mutation)
  // 暂停新写入并等待在途写盘完成，再执行真正的移动/删除。
  await fileTreeStore.beginPathMutation()
  try {
    return await operation()
  } finally {
    fileTreeStore.endPathMutation()
  }
}

/**
 * 重命名/移动后同步所有打开标签的路径。
 * 同时处理：openTabs、左右分屏的路径列表、activeTab（以及其子文件）。
 */
const remapOpenTabsForPathChange = (oldPath, newPath, nextName = '') => {
  const source = normalizePath(oldPath)
  const dest = normalizePath(newPath)
  if (!source || !dest || source === dest) return

  const remapPath = (raw) => {
    const path = normalizePath(raw)
    if (path === source) return dest
    if (path.startsWith(`${source}/`)) return dest + path.slice(source.length)
    return raw
  }

  fileTreeStore.openTabs.forEach((tab) => {
    const nextPath = remapPath(tab?.path)
    if (nextPath === tab?.path) return
    tab.path = nextPath
    if (nextName && normalizePath(tab.path) === dest) tab.fileName = nextName
  })
  fileTreeStore.leftTabPaths = (fileTreeStore.leftTabPaths || []).map(remapPath)
  fileTreeStore.rightTabPaths = (fileTreeStore.rightTabPaths || []).map(remapPath)

  const active = normalizePath(fileTreeStore.activeTab)
  if (active === source || active.startsWith(`${source}/`)) {
    fileTreeStore.setActiveTab(remapPath(active))
  } else {
    // 活跃文件未受影响时也要刷新一次，让 selectedFile 与（可能已更新的）标签列表一致。
    fileTreeStore.setActiveTab(fileTreeStore.activeTab)
  }

  // 阅读器登记表也要跟着搬，否则重命名后标签仍指向旧路径、覆盖层就消失了。
  readerTabStore.remapPath(source, dest)
}

/** 删除后关闭其下所有标签，避免自动保存把已删除的文件重新写回磁盘。 */
const dropTabsAfterDelete = (path) => {
  const target = normalizePath(path)
  if (!target) return
  fileTreeStore.dropTabsUnderPath(target)
  // 同步清理阅读器登记，避免路径复用后旧登记串到新文件上。
  readerTabStore.closeUnderPath(target)
}

const commitInlineRename = async () => {
  const ctx = inlineRenameCtx.value
  if (!ctx) { editingNode.value = null; editName.value = ''; return }
  const { mode, node, oldId } = ctx
  const name = (editName.value || '').trim()
  // 名称为空或未变化：放弃改名，直接退出编辑态。
  if (!name || name === node.name) {
    inlineRenameCtx.value = null
    editingNode.value = null
    editName.value = ''
    return
  }
  const parent = parentDirOf(oldId)
  const dest = normalizePath(`${parent}/${name}`)
  inlineRenameCtx.value = null
  try {
    // 先让编辑器落地防抖编辑，再暂停写盘，避免重命名瞬间的自动保存写回旧路径。
    await runPathMutation({ type: 'rename', oldPath: oldId, newPath: dest }, async () => {
      await invoke('move_path', { sourcePath: oldId, destinationPath: dest })
      remapOpenTabsForPathChange(oldId, dest, name)
    })
    editingNode.value = null
    editName.value = ''
    await refreshModeDir(mode, parent)

    // 重命名后自动更新全库指向它的链接（wiki 链接 + 相对路径链接）。
    // 整段包在 try/catch 里：链接更新出任何问题，都不能影响「重命名本身已经成功」这个事实。
    let linkUpdate = null
    if (isMarkdownFilePath(oldId) && isMarkdownFilePath(dest)) {
      try {
        linkUpdate = await updateMarkdownLinksAfterRename({
          repoRoot: fileTreeStore.activeRepoPath || '',
          oldPath: oldId,
          newPath: dest,
          // 打开着的文件跳过：它们可能有未保存的编辑，读盘再写回会丢掉用户正在改的内容。
          openPaths: (fileTreeStore.openTabs || []).map((tab) => normalizePath(tab.path)),
        })
      } catch (error) {
        logger.warn('[Rename] link update threw, rename itself succeeded', { oldId, dest }, error)
      }
    }

    message.success(
      linkUpdate && linkUpdate.linksChanged
        ? `已重命名，并更新了 ${linkUpdate.linksChanged} 处链接（${linkUpdate.filesChanged} 个文件）`
        : '已重命名'
    )
  } catch (e) {
    editingNode.value = null
    editName.value = ''
    logger.error('重命名失败:', e)
    message.error('重命名失败: ' + (e?.message || e))
    await refreshModeDir(mode, parent)
  }
}

const genDelete = async (mode, node) => {
  const yes = await confirmDialog(`确定删除「${node.name}」吗？此操作不可撤销。`, { title: '删除确认', kind: 'warning', okLabel: '删除', cancelLabel: '取消' })
  if (!yes) return false
  const deletedPath = normalizePath(node.id)
  await runPathMutation({ type: 'delete', oldPath: deletedPath }, async () => {
    await invoke('delete_path', { path: deletedPath })
    // 删除后立刻关闭相关标签，避免自动保存把文件重新创建出来。
    dropTabsAfterDelete(deletedPath)
  })
  await refreshModeDir(mode, parentDirOf(node.id))
  message.success('已删除')
  return true
}

// 设计说明：虚拟文件树只负责键盘命令识别，真正的删除仍复用各模式原有确认弹窗和后端命令。
// 这样 Delete 快捷键与右键菜单行为保持一致，也避免绕过删除确认。
const handleVirtualDeleteNode = async (mode, node) => {
  if (!node?.id || editingNode.value) return
  const path = normalizePath(node.id)
  logger.info('[FileTreeKeyboard] delete shortcut requested', {
    mode,
    path,
    isDirectory: !!node.is_directory
  })

  if (mode === 'kb') {
    selectedNode.value = node
    await deleteNode(path)
    return
  }

  const deleted = await genDelete(mode, node)
  if (!deleted) return
  if (mode === 'task') {
    taskCatSelectedKeys[activeTaskCat.value] = []
  } else if (mode === 'plain') {
    externalSelectedKeys.value = []
  }
}

const genOpenExplorer = async (node) => {
  const p = normalizePath(node.id)
  const target = node.is_directory ? p : parentDirOf(p)
  if (target) await invoke('open_in_file_explorer', { path: target })
}

const openNodeInNeovim = async (node, reason = 'file-tree-context-menu') => {
  if (!node?.id) return
  const nodePath = normalizePath(node.id)
  const isDirectory = !!node.is_directory

  logger.info('[EditorBridge] open path in Neovim from file tree', {
    path: nodePath,
    isDirectory,
    reason,
  })

  if (!isDirectory) {
    const openTab = fileTreeStore.openTabs.find((tab) => normalizePath(tab?.path) === nodePath)
    if (openTab?.dirty) {
      logger.info('[EditorBridge] save dirty open tab before file-tree Neovim handoff', {
        path: nodePath,
        reason,
      })
      await fileTreeStore.updateFileContent(nodePath, openTab.content)
    }
  }

  await invoke('neovim_open_path', { path: nodePath, isDirectory })
  if (isDirectory) {
    editorBridgeStore.setNeovimRoot(nodePath, reason)
  } else {
    await editorBridgeStore.trackFile(nodePath, fileTreeStore)
    editorBridgeStore.setOwner(nodePath, 'neovim', reason)
  }
  message.info(isDirectory ? '已用 Neovim 打开目录' : '已交给 Neovim 编辑，Guaxi 将保持旁观同步')
}

const openRootPathInNeovim = async (path, reason = 'file-tree-root-action') => {
  const rootPath = normalizePath(path)
  if (!rootPath) {
    message.warning('根目录路径为空，无法用 Neovim 打开')
    return
  }

  logger.info('[EditorBridge] open root path in Neovim', {
    path: rootPath,
    reason,
  })
  await openNodeInNeovim({ id: rootPath, name: getDirDisplayName(rootPath), is_directory: true }, reason)
}

// 通用右键菜单
const genMenuVisible = ref(false)
const genMenuPos = ref({ x: 0, y: 0 })
const genMenuContext = ref({ mode: 'plain', node: null, isBlank: false })
const genMenuOptions = computed(() => {
  const { node, isBlank } = genMenuContext.value
  const base = [
    { label: '新建文件', key: 'addFile', icon: () => h(NIcon, null, { default: () => h(AddOutline) }) },
    { label: '新建文件夹', key: 'addFolder', icon: () => h(NIcon, null, { default: () => h(FolderOpenOutline) }) },
    {
      label: '导入文件',
      key: 'importFile',
      icon: () => h(NIcon, null, { default: () => h(DownloadOutline) }),
      children: IMPORT_MENU_OPTIONS,
    },
  ]
  if (node && !isBlank) {
    base.push(
      { label: '重命名', key: 'rename', icon: () => h(NIcon, null, { default: () => h(CreateOutline) }) },
      { label: '删除', key: 'delete', icon: () => h(NIcon, null, { default: () => h(TrashOutline) }) },
      { label: '在文件浏览器中打开', key: 'openInExplorer', icon: () => h(NIcon, null, { default: () => h(FolderOutline) }) },
      { label: '用 Neovim 打开', key: 'openInNeovim', icon: renderNeovimIcon },
    )
    // 浏览器预览只对 Markdown 文件有意义，目录与非 Markdown 文件不显示这一项。
    if (!node.is_directory && isMarkdownFilePath(node.id)) {
      base.push({ label: '在浏览器中查看', key: 'openInBrowser', icon: () => h(NIcon, null, { default: () => h(GlobeOutline) }) })
    }
    // 文档默认开在独立阅读器窗口；这里给一条「仍在标签页里看」的退路，
    // 便于读者一边读、一边在同一窗口对照编辑其它笔记。
    if (!node.is_directory && readerTabStore.isReaderPath(node.id)) {
      base.push({ label: '在标签页中打开', key: 'openInTab', icon: () => h(NIcon, null, { default: () => h(DocumentOutline) }) })
    }
  }
  return base
})
const actionDirForContext = () => {
  const { mode, node, rootOverride } = genMenuContext.value
  if (node) return node.is_directory ? normalizePath(node.id) : parentDirOf(node.id)
  if (rootOverride) return normalizePath(rootOverride)
  return mode === 'plain' ? normalizePath(externalDir.value) : catPathOf('work')
}
const openGenContextMenu = (mode, { node, x, y }) => {
  genMenuContext.value = { mode, node, isBlank: false, rootOverride: '' }
  genMenuPos.value = { x, y }
  genMenuVisible.value = true
}
const openGenBlankMenu = (mode, { x, y }, rootOverride = '') => {
  genMenuContext.value = { mode, node: null, isBlank: true, rootOverride }
  genMenuPos.value = { x, y }
  genMenuVisible.value = true
}
const closeGenMenu = () => { genMenuVisible.value = false }
const handleGenMenuSelect = async (key) => {
  genMenuVisible.value = false
  const { mode, node } = genMenuContext.value
  const dir = actionDirForContext()
  try {
    if (key === 'addFile') await genAddFile(mode, dir)
    else if (key === 'addFolder') await genAddFolder(mode, dir)
    else if (key === 'importFile') await genImportFile(mode, dir, 'copy')
    else if (key === 'importFileCopy') await genImportFile(mode, dir, 'copy')
    else if (key === 'importFileConvert') await genImportFile(mode, dir, 'convert')
    else if (key === 'rename' && node) await genRename(mode, node)
    else if (key === 'delete' && node) await genDelete(mode, node)
    else if (key === 'openInExplorer' && node) await genOpenExplorer(node)
    else if (key === 'openInNeovim' && node) await openNodeInNeovim(node, `file-tree-${mode}`)
    else if (key === 'openInBrowser' && node) await openMarkdownInBrowser(node.id)
    else if (key === 'openInTab' && node) await openFileByKind(node.id, { inline: true })
  } catch (e) {
    logger.error('文件操作失败:', e)
    message.error('操作失败: ' + (e?.message || e))
  }
}

// 头部动作图标：plain 作用于外部目录根；task 每个分类作用于该分类根目录
const plainHeaderAddFile = () => genAddFile('plain', normalizePath(externalDir.value))
const plainHeaderAddFolder = () => genAddFolder('plain', normalizePath(externalDir.value))
const plainHeaderImport = (action = 'copy') => genImportFile('plain', normalizePath(externalDir.value), action)
const taskCatAddFile = (key) => genAddFile('task', catPathOf(key))
const taskCatAddFolder = (key) => genAddFolder('task', catPathOf(key))
const taskCatImport = (key, action = 'copy') => genImportFile('task', catPathOf(key), action)

const runFileTreeToolbarShortcut = async (action) => {
  logger.info('[FileTreeShortcut] toolbar shortcut requested', {
    action,
    viewMode: viewMode.value,
    activeRepoPath: activeRepoPathItem.value,
    externalDir: externalDir.value,
    activeTaskCat: activeTaskCat.value,
  })

  if (viewMode.value === 'kb') {
    const repoPath = activeRepoPathItem.value
    if (!repoPath) {
      message.warning('请先选择一个知识库')
      return
    }
    if (action === 'new-file') return handleAddFile(repoPath)
    if (action === 'new-folder') return handleAddFolder(repoPath)
    return handleImportMarkdownFile(repoPath)
  }

  if (viewMode.value === 'task') {
    const cat = activeTaskCat.value
    if (!catPathOf(cat)) {
      message.warning('请先选择一个任务分类')
      return
    }
    if (action === 'new-file') return taskCatAddFile(cat)
    if (action === 'new-folder') return taskCatAddFolder(cat)
    return taskCatImport(cat)
  }

  if (!externalDir.value) {
    message.warning('请先打开一个外部目录')
    return
  }
  if (action === 'new-file') return plainHeaderAddFile()
  if (action === 'new-folder') return plainHeaderAddFolder()
  return plainHeaderImport()
}

// 设计说明：文件树工具条图标在三种模式下共用同一组快捷键；
// 快捷键只做路由，最终仍调用原按钮函数，避免新建/导入行为和鼠标点击出现分叉。
useGuaxiHotkeys([
  {
    id: 'fileTreeNewFile',
    label: '新建文件',
    shortcut: GUAXI_SHORTCUTS.fileTreeNewFile,
    handler: () => runFileTreeToolbarShortcut('new-file'),
  },
  {
    id: 'fileTreeNewFolder',
    label: '新建文件夹',
    shortcut: GUAXI_SHORTCUTS.fileTreeNewFolder,
    handler: () => runFileTreeToolbarShortcut('new-folder'),
  },
  {
    id: 'fileTreeImportFile',
    label: '导入文件',
    shortcut: GUAXI_SHORTCUTS.fileTreeImportFile,
    handler: () => runFileTreeToolbarShortcut('import-file'),
  },
], {
  logger,
  scope: 'GuaxiFileTreeShortcuts',
})


// 外部目录树：打开文件
/**
 * 文件编辑模式：打开外部目录里的文件。
 *
 * 与知识库模式共用同一条分流逻辑（openFileByKind）：文本进 Monaco，
 * 其余交给内嵌阅读器。此前这里只走 fetchFileContent，于是 .docx / .epub / .pptx
 * 在「文件编辑」模式下只会落进「不支持该文件类型」的占位文本。
 */
const handleExternalOpenFile = async (node) => {
  if (!node || node.is_directory) return
  externalSelectedKeys.value = [node.id]
  try {
    await openFileByKind(node.id)
  } catch (error) {
    logger.error('打开外部文件失败:', error)
  }
}

// 外部目录树：懒加载子目录
const handleExternalLoadChildren = async (node) => {
  if (!node?.is_directory) return
  externalSelectedKeys.value = [node.id]
  try {
    const dirId = normalizePath(node.id)
    await fileTreeStore.fetchFileTree(dirId, { force: true })
    const filtered = filterHiddenTree(fileTreeStore.fileTree)
    node.children = sortFileTree(normalizeTreeIds(filtered))
    refreshSidebarNavigationContext()
  } catch (error) {
    logger.error('加载外部子目录失败:', error)
  }
}

const getDirDisplayName = (path) => {
  const p = String(path || '')
  return p.split(/[\\/]/).filter(Boolean).pop() || p
}

// 右键菜单(虚拟树 contextmenu 事件):等价旧 nodeProps 的逻辑
const handleNodeContextMenu = ({ node, x, y }) => {
  selectedNode.value = node

  const nodePath = normalizePath(node.id)
  const canOpenInBrowser = !node.is_directory && isMarkdownFilePath(nodePath)
  const openInBrowserOpt = contextMenuOptions.value.find(o => o.key === 'openInBrowser')
  if (openInBrowserOpt) openInBrowserOpt.disabled = !canOpenInBrowser

  const canImportAsTemplate = !node.is_directory && isMarkdownFilePath(nodePath)
  const importOpt = contextMenuOptions.value.find(o => o.key === 'importAsTemplate')
  if (importOpt) importOpt.disabled = !canImportAsTemplate

  // "在一侧打开"只对文件有意义，目录置灰。
  const openToSideOpt = contextMenuOptions.value.find(o => o.key === 'openToSide')
  if (openToSideOpt) openToSideOpt.disabled = !!node.is_directory

  // 差异对比第二步只在已标记过一方、且不是自己跟自己比时出现，文案带上已选的名字。
  const compareOpt = contextMenuOptions.value.find(o => o.key === 'compareWithPending')
  if (compareOpt) {
    const pending = diffStore.pending
    compareOpt.show = !!pending?.path && normalizePath(pending.path) !== nodePath
    compareOpt.label = diffStore.compareWithLabel
  }

  isContextMenuVisible.value = true
  contextMenuPosition.value = { x, y }
  menuMode.value = 'node'
}

// 文件树空白处右键：未选中任何节点，新建的文件/目录落在当前仓库根目录下
const handleBlankContextMenu = ({ x, y }) => {
  selectedNode.value = null
  menuMode.value = 'blank'
  isContextMenuVisible.value = true
  contextMenuPosition.value = { x, y }
}

// 取消内联编辑(虚拟树 rename-cancel 事件):等价旧 handleKeydown 的 Escape 分支
const cancelEditing = async () => {
  if (editingNode.value && editingNode.value.isNew) {
    const node = editingNode.value
    const parentNode = findParentNode(fileTree.value, node.id)
    let parentPath = parentNode ? parentNode.id : null

    if (!parentNode) {
      parentPath = normalizePath(node.id).split('/').slice(0, -1).join('/')
      if (!parentPath || parentPath === normalizePath(fileTreeStore.activeRepoPath)) {
        await loadFileTree()
      } else {
        await refreshDirectory(parentPath)
      }
    } else {
      parentNode.children = parentNode.children.filter(n => n.id !== node.id)
      refreshSidebarNavigationContext()
      if (normalizePath(parentPath) === normalizePath(fileTreeStore.activeRepoPath)) {
        await loadFileTree()
      } else {
        await refreshDirectory(parentPath)
      }
    }
  }
  editingNode.value = null
  editName.value = ''
}

// 拖拽移动(虚拟树 move 事件):把 sourceId 移动到 targetDirId
const handleDropMove = async (sourceId, targetDirId) => {
  const sourcePath = normalizePath(sourceId)
  const targetDir = normalizePath(targetDirId)
  if (!sourcePath || !targetDir) return
  if (isHiddenPath(sourcePath)) return
  if (sourcePath === targetDir || targetDir === sourcePath || targetDir.startsWith(`${sourcePath}/`)) {
    message.warning('不能移动到自身或其子目录')
    return
  }

  const name = sourcePath.split('/').pop()
  const destinationPath = normalizePath(`${targetDir}/${name}`)
  const sourceParent = sourcePath.split('/').slice(0, -1).join('/') || normalizePath(fileTreeStore.activeRepoPath)
  if (destinationPath === sourcePath) return
  if (normalizePath(sourceParent) === targetDir) return // 已在目标目录

  try {
    await runPathMutation({ type: 'move', oldPath: sourcePath, newPath: destinationPath }, async () => {
      await invoke('move_path', { sourcePath, destinationPath })
      // 文件本身或目录下的子文件，路径统一重映射。
      remapOpenTabsForPathChange(sourcePath, destinationPath, name)
    })

    if (!expandedKeys.value.includes(targetDir)) expandedKeys.value.push(targetDir)
    await refreshDirectory(sourceParent)
    await refreshDirectory(targetDir)

    selectedKeys.value = [destinationPath]
    lastSelectedKey.value = destinationPath
    logger.info('[DnDMove] success', { sourcePath, destinationPath })
    message.success('移动成功')
  } catch (error) {
    logger.error('拖拽移动失败:', error)
    message.error(`移动失败: ${error.message || error}`)
    await refreshDirectory(sourceParent)
    await refreshDirectory(targetDir)
  }
}

// 递归查找节点
const findNodeById = (tree, targetId) => {
  const normalizedTargetId = normalizePath(targetId);
  logger.info('查找节点:', normalizedTargetId);

  // 跳过隐藏目录
  if (isHiddenPath(normalizedTargetId)) {
    logger.info('跳过隐藏目录:', normalizedTargetId);
    return null;
  }

  // 深度优先搜索
  const dfs = (nodes) => {
    for (const node of nodes) {
      const nodePath = normalizePath(node.id);
      
      // 跳过隐藏目录
      if (isHiddenPath(nodePath)) {
        continue;
      }

      if (nodePath === normalizedTargetId) {
        return node;
      }

      if (node.children) {
        const found = dfs(node.children);
        if (found) return found;
      }
    }
    return null;
  };

  return dfs(tree);
};


// 处理右键菜单选择
// ───── 复制路径 / 在一侧打开 ─────
// 说明：优先用浏览器剪贴板；WebView 里被策略拦掉时回退到 Tauri 剪贴板插件（和编辑器一致）。
const writeClipboardText = async (text) => {
  const value = String(text ?? '')
  try {
    await navigator.clipboard.writeText(value)
    return true
  } catch (error) {
    logger.debug('[Clipboard] navigator write failed, trying Tauri clipboard manager', {
      error: String(error?.message || error || ''),
    })
  }
  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (error) {
    logger.warn('[Clipboard] write failed', { error: String(error?.message || error || '') })
    return false
  }
}

// 绝对路径：按当前系统的分隔符输出，方便直接粘到终端/资源管理器里用。
const toNativePath = (p) => {
  const normalized = normalizePath(p)
  if (!normalized) return ''
  const isWindowsPath = /^[a-zA-Z]:\//.test(normalized) || normalized.startsWith('//')
  return isWindowsPath ? normalized.replace(/\//g, '\\') : normalized
}

// 相对路径：相对文件树根节点（当前笔记库）。不在根下时退化成绝对路径。
const toRepoRelativePath = (p) => {
  const target = normalizePath(p)
  const root = normalizePath(fileTreeStore.activeRepoPath).replace(/\/+$/, '')
  if (!target) return ''
  if (!root) return target
  if (target === root) return ''
  const prefix = `${root}/`
  if (target.toLowerCase().startsWith(prefix.toLowerCase())) {
    return target.slice(prefix.length)
  }
  return target
}

const copyNodePath = async (node, { relative = false } = {}) => {
  if (!node?.id) return
  const text = relative ? toRepoRelativePath(node.id) : toNativePath(node.id)
  if (!text) {
    message.warning(relative ? '该节点就是文件树根目录，没有相对路径' : '无法获取路径')
    return
  }
  const ok = await writeClipboardText(text)
  if (ok) message.success(`已复制${relative ? '相对' : ''}路径: ${text}`)
  else message.error('复制失败')
}

// 在右侧分屏打开：交给 store 强制落到 right pane（单屏时会自动进入左右分屏）。
const openNodeToSide = async (node) => {
  if (!node?.id || node.is_directory) return
  try {
    // 与主打开路径同源：非文本文件在右侧 pane 里同样交给 ReaderPane。
    await openFileByKind(node.id, { region: 'right' })
  } catch (error) {
    logger.error('在一侧打开文件失败:', error)
    message.error('在一侧打开失败')
  }
}

const handleContextMenuSelect = async (key) => {
  isContextMenuVisible.value = false;
  try {
    switch (key) {
      case 'openToSide':
        await openNodeToSide(selectedNode.value);
        break;
      case 'markForCompare':
        if (selectedNode.value) {
          const target = normalizePath(selectedNode.value.id);
          diffStore.markForCompare(target, !!selectedNode.value.is_directory);
          message.info(`已选择"${diffPathBaseName(target)}"，再右键另一项即可比较`);
        }
        break;
      case 'compareWithPending':
        if (selectedNode.value) {
          const res = diffStore.compareWithPending(
            normalizePath(selectedNode.value.id),
            !!selectedNode.value.is_directory,
          );
          if (!res.ok) message.warning(res.error);
        }
        break;
      case 'copyPath':
        await copyNodePath(selectedNode.value);
        break;
      case 'copyRelativePath':
        await copyNodePath(selectedNode.value, { relative: true });
        break;
      case 'addFile':
        await addNode(selectedNode.value?.id, false);
        break;
      case 'addFolder':
        await addNode(selectedNode.value?.id, true);
        break;
      case 'importAsTemplate':
        await openImportTemplateModal();
        break;
      case 'rename':
        if (selectedNode.value) {
          renameNode(selectedNode.value.id);
        }
        break;
      case 'delete':
        if (selectedNode.value) {
          deleteNode(selectedNode.value.id);
        }
        break;
      case 'move':
        if (selectedNode.value) {
          moveNode(selectedNode.value);
        }
        break;
      case 'openInExplorer':
        if (selectedNode.value) {
          const nodePath = normalizePath(selectedNode.value.id);
          const targetPath = selectedNode.value.is_directory
            ? nodePath
            : nodePath.split('/').slice(0, -1).join('/');
          if (targetPath) {
            await invoke('open_in_file_explorer', { path: targetPath });
          }
        }
        break;
      case 'openInNeovim':
        if (selectedNode.value) {
          await openNodeInNeovim(selectedNode.value, 'knowledge-tree-context-menu');
        }
        break;
      case 'openInBrowser':
        if (selectedNode.value && !selectedNode.value.is_directory) {
          await openMarkdownInBrowser(selectedNode.value.id)
        }
        break;
    }
  } catch (error) {
    message.error(`操作失败: ${error.message}`);
  }
};

// 处理点击外部关闭菜单
const handleClickoutside = () => {
  isContextMenuVisible.value = false
}

// 添加一个 ref 来存储当前正在编辑的节点
const editingNode = ref(null)
// 添加一个 ref 来存储编辑中的名称
const editName = ref('')
// 传给虚拟文件树:当前处于编辑态的节点 id
const editingId = computed(() => editingNode.value?.id ?? null)

// 修改后的 addNode 函数
const addNode = async (parentId, isDirectory = false) => {
  parentId = normalizePath(parentId);

  const activeRepo = normalizePath(fileTreeStore.activeRepoPath)
  const isRoot = !parentId || parentId === activeRepo

  let parentNode = null
  if (!isRoot) {
    parentNode = findNodeById(fileTree.value, parentId);
    if (!parentNode) {
      parentId = activeRepo
      parentNode = null
    }
  }

  if (!isRoot && parentNode?.is_directory) {
    ensureExpandedForDir(parentNode.id)
    if (!Array.isArray(parentNode.children) || parentNode.children.length === 0) {
      await fileTreeStore.fetchFileTree(parentNode.id)
      const filteredChildren = filterHiddenTree(fileTreeStore.fileTree)
      parentNode.children = sortFileTree(normalizeTreeIds(filteredChildren))
      refreshSidebarNavigationContext()
    }
  }

  const tempName = isDirectory ? '新建文件夹' : '新建文件';
  const baseDir = isRoot ? activeRepo : normalizePath(parentNode?.id)
  const tempId = `${baseDir}/${tempName}`;

  const newNode = {
    id: tempId,
    name: tempName,
    is_directory: isDirectory,
    children: isDirectory ? [] : undefined,
    isNew: true
  };

  if (isRoot) {
    fileTree.value = [newNode, ...fileTree.value];
  } else {
    parentNode.children = [newNode, ...(parentNode.children || [])];
    refreshSidebarNavigationContext()
  }

  if (!isRoot && parentNode?.id) {
    ensureExpandedForDir(parentNode.id)
  }

  // 仓库整体收起时文件树没有渲染，新建的节点会建在看不见的地方：先把它展开。
  if (fileTreeStore.activeRepoPath) {
    fileTreeStore.expandedRepos.set(fileTreeStore.activeRepoPath, true)
  }

  startEditing(newNode);

  // 新建行进入选中态：编辑中的临时节点也要有高亮条，否则用户得自己在一大片节点里找它。
  selectedKeys.value = [newNode.id];
  lastSelectedKey.value = newNode.id;

  await nextTick()
  await scrollToKey(newNode.id)
  await focusEditingInput(newNode.id)

  const created = await new Promise((resolve) => {
    const unwatch = watch(editingNode, (newVal) => {
      if (!newVal) {
        unwatch();
        handleEditComplete(newNode, parentNode, isDirectory, isRoot, resolve);
      }
    });
  });

  // 提交后树被刷新，临时节点已被真实节点替换：把选中态与视口落到真实 id 上。
  if (created?.id) {
    selectedKeys.value = [created.id]
    lastSelectedKey.value = created.id
    await scrollToKey(created.id, 'nearest')
  }

  return created;
};

// 处理编辑完成的函数
const handleEditComplete = async (node, parentNode, isDirectory, isRoot, resolve) => {
  const removeTempNode = () => {
    if (isRoot) {
      fileTree.value = fileTree.value.filter(n => n.id !== node.id)
      return
    }

    if (parentNode?.children) {
      parentNode.children = parentNode.children.filter(n => n.id !== node.id)
      refreshSidebarNavigationContext()
    }
  }

  if (node.name === '新建文件夹' || node.name === '新建文件' || !node.name.trim()) {
    removeTempNode()
    resolve(null);
    return;
  }

  try {
    const targetPath = normalizePath(node.id)
    node.id = targetPath

    if (isDirectory) {
      await invoke('create_directory', { path: targetPath })
    } else {
      await invoke('create_file_with_content', {
        path: targetPath,
        content: ''
      })
    }

    delete node.isNew

    const parentDir = isRoot
      ? fileTreeStore.activeRepoPath
      : parentNode?.id

    if (parentDir) {
      await refreshDirectory(parentDir)
    }

    resolve(node);
  } catch (error) {
    const errorMessage = String(error?.message || error || '未知错误')
    logger.error('创建失败:', { error: errorMessage, nodeId: node.id, isDirectory });
    removeTempNode()
    message.error(`创建失败: ${errorMessage}`);
    resolve(null);
  } finally {
    selectedKeys.value = [];
  }
};

// 修改后的 handleBlur 函数
const handleBlur = async () => {
  if (!editingNode.value) return;

  const node = editingNode.value;
  const oldId = node.id;
  const parentNode = findParentNode(fileTree.value, oldId);
  let parentPath = parentNode ? parentNode.id : fileTreeStore.activeRepoPath;

  if (!editName.value.trim() ||
    editName.value === '新建文件' ||
    editName.value === '新建文件夹') {
    // 名称为空，如果是新节点则删除
    if (node.isNew) {
      if (!parentNode) {
        // logger.warn('未找到父节点，无法删除新节点');
        // 根目录下直接从 fileTree.value 里移除
        fileTree.value = fileTree.value.filter(n => n.id !== oldId);
        // 新增：刷新根目录
        parentPath = normalizePath(node.id).split('/').slice(0, -1).join('/');
        if (!parentPath || parentPath === normalizePath(fileTreeStore.activeRepoPath)) {
          await loadFileTree();
        } else {
          await refreshDirectory(parentPath);
        }
      } else {
        parentNode.children = parentNode.children.filter(n => n.id !== oldId);
        refreshSidebarNavigationContext()
        if (normalizePath(parentPath) === normalizePath(fileTreeStore.activeRepoPath)) {
          await loadFileTree();
        } else {
          await refreshDirectory(parentPath);
        }
      }
    }
    editingNode.value = null;
    editName.value = '';
    return;
  }

  if (node?.isNew && !node?.is_directory) {
    const normalized = normalizeNewMarkdownFileName(editName.value);
    if (normalized && normalized !== editName.value) {
      logger.info('[NewFile] auto append .md', { input: editName.value, output: normalized });
      editName.value = normalized;
    }
  }

  // 构建新路径（统一规范化，避免 Windows 路径分隔符导致创建/读取不一致）
  const basePath = normalizePath(parentPath)
  const newId = normalizePath(`${basePath}/${editName.value}`)

  // 更新节点信息
  node.name = editName.value
  node.id = newId

  // 如果是新节点，结束编辑（实际文件会在handleEditComplete中创建）
  if (node.isNew) {
    editingNode.value = null;
    editName.value = '';
    return;
  }

  // 以下是原有重命名逻辑
  try {
    await runPathMutation({ type: 'rename', oldPath: oldId, newPath: newId }, async () => {
      if (node.is_directory) {
        await invoke('rename_directory', { oldPath: oldId, newPath: newId });
      } else {
        await invoke('rename_path', { oldPath: oldId, newPath: newId });
      }
      // 目录重命名时，其下已打开的子文件也必须一起改路径；写盘在此期间被暂停。
      remapOpenTabsForPathChange(oldId, newId, editName.value)
    })
  } catch (error) {
    logger.error('重命名操作失败:', error);
    node.name = oldId.split(/[\\/]/).pop();
    node.id = oldId;
    message.error(`重命名失败: ${error.message}`);
  } finally {
    editingNode.value = null;
    editName.value = '';
    // 根目录下刷新整个树，否则刷新父目录
    if (normalizePath(parentPath) === normalizePath(fileTreeStore.activeRepoPath)) {
      await loadFileTree({ force: true });
    } else {
      await refreshDirectory(parentPath);
    }
  }
};

// 删除文件或目录
const deleteNode = async (nodeId) => {
  const normalizedId = normalizePath(nodeId)
  const node = findNodeById(fileTree.value, normalizedId)
  if (!node) return

  const ok = await confirmDialog(`确定要删除 ${node.name} 吗？`, {
    title: '删除',
    kind: 'warning'
  })
  if (!ok) return

  try {
    await runPathMutation({ type: 'delete', oldPath: normalizedId }, async () => {
      await invoke('delete_path', { path: normalizedId })
      // 先关闭相关标签，再刷新树：避免自动保存把已删除的文件重新写回。
      dropTabsAfterDelete(normalizedId)
      await handleFileRemoved(normalizedId)
    })
  } catch (error) {
    message.error(`删除失败: ${error.message}`)
    const parentPath = normalizedId.split('/').slice(0, -1).join('/') || normalizePath(fileTreeStore.activeRepoPath)
    await refreshDirectory(parentPath)
  } finally {
    selectedKeys.value = []
    lastSelectedKey.value = null
  }
}

// 重命名文件或目录
const renameNode = async (nodeId) => {
  const node = findNodeById(fileTree.value, nodeId)
  if (node) {
    // 设置节点为编辑状态
    startEditing(node)
  }
}

// 修改后的 findParentNode 函数
const findParentNode = (tree, targetId) => {
  const normalizedTargetId = normalizePath(targetId);
  logger.info('查找父节点:', normalizedTargetId);

  // 深度优先搜索
  const dfs = (nodes, parent) => {
    for (const node of nodes) {
      const nodePath = normalizePath(node.id);

      if (nodePath === normalizedTargetId) {
        return parent;
      }

      if (node.children) {
        const found = dfs(node.children, node);
        if (found) return found;
      }
    }
    return null;
  };

  return dfs(tree, null);
};

// 开始编辑节点
const startEditing = (node) => {
  logger.info('startEditing 开始编辑节点:', node.name);
  editingNode.value = node
  editName.value = node.name
  logger.info('startEditing ==> ', node);
}

watch(editingNode, async (val) => {
  if (!val) return
  await focusEditingInput(val.id)
})

const moveNode = async (node) => {
  const targetDirectoryPath = await selectTargetPath();
  if (!targetDirectoryPath) return;

  const sourcePath = normalizePath(node.id);
  const targetDir = normalizePath(targetDirectoryPath);
  const destinationPath = normalizePath(`${targetDir}/${node.name}`);
  const sourceParentPath = sourcePath.split('/').slice(0, -1).join('/') || normalizePath(fileTreeStore.activeRepoPath);

  try {
    logger.info('源路径:', sourcePath);
    logger.info('目标路径:', destinationPath);

    await runPathMutation({ type: 'move', oldPath: sourcePath, newPath: destinationPath }, async () => {
      await invoke('move_path', { sourcePath, destinationPath });
      remapOpenTabsForPathChange(sourcePath, destinationPath, destinationPath.split('/').pop())
    });

    const parentNode = findParentNode(fileTree.value, sourcePath);
    if (parentNode?.children?.length) {
      parentNode.children = parentNode.children.filter(
        (child) => normalizePath(child.id) !== sourcePath
      );
    } else {
      fileTree.value = fileTree.value.filter((n) => normalizePath(n.id) !== sourcePath);
    }

    const targetNode = findNodeById(fileTree.value, targetDir);
    if (targetNode && targetNode.is_directory) {
      if (!Array.isArray(targetNode.children)) targetNode.children = [];
      node.id = destinationPath;
      targetNode.children.push(node);
    } else {
      node.id = destinationPath;
      await loadFileTree({ force: true });
    }

    if (selectedKeys.value?.length && normalizePath(selectedKeys.value[0]) === sourcePath) {
      selectedKeys.value = [destinationPath];
      lastSelectedKey.value = destinationPath;
    }

    await refreshDirectory(sourceParentPath);
    await refreshDirectory(targetDir);

    logger.info('移动成功');
  } catch (error) {
    logger.error('移动失败:', error);
    await refreshDirectory(sourceParentPath);
    await refreshDirectory(targetDir);
  }
};

const selectTargetPath = async () => {
  const result = await open({
    directory: true,
    multiple: false,
    title: '选择目标路径',
  });

  if (!result) return null;

  const pickedPath = Array.isArray(result) ? result[0] : result;
  return pickedPath || null;
};

const refreshDirectory = async (directoryId) => {
  const activeRepo = normalizePath(fileTreeStore.activeRepoPath)
  const normalizedId = normalizePath(directoryId)
  logger.info('刷新目录:', normalizedId)

  if (isHiddenPath(normalizedId)) {
    logger.info('跳过隐藏目录刷新:', normalizedId)
    return
  }

  if (!normalizedId || !activeRepo || normalizedId === activeRepo) {
    await loadFileTree({ force: true })
    return
  }

  let targetDir = normalizedId

  const maybeNode = findNodeById(fileTree.value, targetDir)
  if (maybeNode && !maybeNode.is_directory) {
    targetDir = targetDir.split('/').slice(0, -1).join('/') || activeRepo
  }

  if (!targetDir || targetDir === activeRepo) {
    await loadFileTree({ force: true })
    return
  }

  const directoryNode = findNodeById(fileTree.value, targetDir)
  if (!directoryNode || !directoryNode.is_directory) {
    await loadFileTree({ force: true })
    return
  }

  try {
    const wasExpanded = expandedKeys.value.includes(targetDir)

    await fileTreeStore.fetchFileTree(targetDir, { force: true })

    const filteredTree = filterHiddenTree(fileTreeStore.fileTree)
    const sortedChildren = sortFileTree(normalizeTreeIds(filteredTree))

    const newChildren = sortedChildren.map(newNode => {
      const existingNode = findNodeById([directoryNode], newNode.id)
      return existingNode || newNode
    })

    directoryNode.children = [...newChildren]
    refreshSidebarNavigationContext()

    if (wasExpanded && !expandedKeys.value.includes(targetDir)) {
      expandedKeys.value.push(targetDir)
    }

    logger.info('目录局部刷新成功:', directoryNode)
  } catch (error) {
    logger.error('目录刷新失败:', error)
  }
};

// 更健壮的路径规范化函数
const normalizePath = (path) => {
  if (!path) return '';

  // 替换所有反斜杠为正斜杠
  let normalized = path.replace(/\\/g, '/');

  // 处理Windows盘符 (如 C:/)
  if (normalized.match(/^[a-zA-Z]:\//)) {
    normalized = normalized[0].toUpperCase() + normalized.slice(1);
  }

  // 移除末尾斜杠（目录除外）
  if (normalized.endsWith('/') && normalized.length > 1) {
    normalized = normalized.slice(0, -1);
  }

  return normalized;
};

// 从路径推断所属知识库根目录（选择“最长前缀匹配”的仓库路径）
const pickRepoRootForPath = (absPath) => {
  const target = normalizePath(absPath)
  if (!target) return ''

  const repos = Array.isArray(fileTreeStore.repopathlist) ? fileTreeStore.repopathlist : []
  let best = ''
  for (const raw of repos) {
    const repo = normalizePath(raw)
    if (!repo) continue
    if (target === repo || target.startsWith(`${repo}/`)) {
      if (repo.length > best.length) best = repo
    }
  }
  return best
}

// 构造从 repoRoot 到 targetPath 的“目录祖先链”，用于展开
const buildAncestorDirs = (repoRoot, targetPath) => {
  const root = normalizePath(repoRoot)
  const target = normalizePath(targetPath)
  if (!root || !target) return []
  if (!(target === root || target.startsWith(`${root}/`))) return []

  const rel = target === root ? '' : target.slice(root.length + 1)
  const parts = rel ? rel.split('/').filter(Boolean) : []
  if (!parts.length) return []

  const dirs = []
  for (let i = 0; i < parts.length - 1; i++) {
    const p = `${root}/${parts.slice(0, i + 1).join('/')}`
    dirs.push(p)
  }
  return dirs
}

// ───── 树内定位（VSCode 式 autoReveal）：展开祖先 → 懒加载子目录 → 选中 → 滚动 ─────
// 设计说明：虚拟树只渲染「展开链路上的已加载节点」。深层路径若只是塞进 expandedKeys 而不把
// 每一级目录的 children 补齐，目标行根本不在 flat 里，选中态和滚动都会静默失效——这正是
// 「打开文件后左栏不跟随」「面包屑点了没反应」的根因。
const loadModeDirChildren = async (mode, node) => {
  if (!node?.is_directory) return false
  if (Array.isArray(node.children) && node.children.length) return true

  try {
    if (mode === 'kb') {
      await fileTreeStore.fetchFileTree(normalizePath(node.id), { force: true })
      node.children = sortFileTree(normalizeTreeIds(filterHiddenTree(fileTreeStore.fileTree)))
    } else {
      node.children = await fetchDirChildren(node.id, mode === 'task')
    }
    refreshSidebarNavigationContext()
    return true
  } catch (error) {
    logger.warn('[TreeReveal] 加载子目录失败', {
      mode,
      id: node.id,
      error: String(error?.message || error || ''),
    })
    return false
  }
}

const revealInModeTree = async (mode, rawPath, options = {}) => {
  const { align = 'nearest' } = options
  const target = normalizePath(rawPath)
  if (!target || isHiddenPath(target)) return false

  let root = ''
  let tree = null
  let catKey = null

  if (mode === 'plain') {
    root = normalizePath(externalDir.value)
    tree = externalDirTree.value
  } else if (mode === 'task') {
    catKey = taskCatOfPath(target)
    if (!catKey) return false
    if (activeTaskCat.value !== catKey) await activateTaskCat(catKey)
    root = catPathOf(catKey)
    tree = taskCatTrees[catKey] || []
  } else {
    root = normalizePath(fileTreeStore.activeRepoPath)
    tree = fileTree.value
  }

  // 目标不在当前模式的根目录下就不动：宁可不定位，也不要把左栏切到无关目录。
  if (!root || !(target === root || target.startsWith(`${root}/`))) return false

  for (const dir of buildAncestorDirs(root, target)) {
    const node = findNodeById(tree, dir)
    if (!node) return false
    await loadModeDirChildren(mode, node)
    if (mode === 'plain') {
      if (!externalExpandedKeys.value.includes(dir)) externalExpandedKeys.value = [...externalExpandedKeys.value, dir]
    } else if (mode === 'task') {
      if (!taskCatExpandedKeys[catKey].includes(dir)) taskCatExpandedKeys[catKey] = [...taskCatExpandedKeys[catKey], dir]
    } else if (!expandedKeys.value.includes(dir)) {
      expandedKeys.value = [...expandedKeys.value, dir]
    }
  }

  // 设计说明（对应"面包屑里点目录展开不了、看不到里面的文件"）：
  // buildAncestorDirs 只给到目标的**祖先链**（循环到 parts.length - 1），所以目标本身若是目录，
  // 上面那一轮只会把它所在的那一行选中并滚过去，它自己仍是收起的——于是"定位到目录"看起来毫无反应。
  // 这里补上：目标是目录时把它也展开（并懒加载其子节点）。
  //
  // **刻意做成 opt-in**：只有面包屑那条调用会传 expandTargetDir，'打开文件自动跟随标签页'那条不传。
  // 这样文件树原有行为（跟随 activeTab 时只展开祖先、不改目标节点状态）一字不变，
  // 新行为的影响面严格限制在"从面包屑点目录"这一个动作上。
  const targetNode = findNodeById(tree, target)
  if (options.expandTargetDir === true && targetNode?.is_directory) {
    await loadModeDirChildren(mode, targetNode)
    if (mode === 'plain') {
      if (!externalExpandedKeys.value.includes(target)) externalExpandedKeys.value = [...externalExpandedKeys.value, target]
    } else if (mode === 'task') {
      if (!taskCatExpandedKeys[catKey].includes(target)) taskCatExpandedKeys[catKey] = [...taskCatExpandedKeys[catKey], target]
    } else if (!expandedKeys.value.includes(target)) {
      expandedKeys.value = [...expandedKeys.value, target]
    }
  }

  if (mode === 'plain') {
    externalSelectedKeys.value = [target]
  } else if (mode === 'task') {
    taskCatSelectedKeys[catKey] = [target]
  } else {
    selectedKeys.value = [target]
    lastSelectedKey.value = target
  }

  await nextTick()
  try {
    await Promise.resolve(treeRefForMode(mode)?.scrollTo?.({ key: target, align }))
  } catch (e) {
    logger.debug('[TreeReveal] scrollTo failed', e)
  }
  return true
}

// 由编辑器面包屑触发：切换仓库、展开并选中目标节点
const revealPathInTree = async (absPath, options = {}) => {
  const target = normalizePath(absPath)
  if (!target) return false

  if (isHiddenPath(target)) {
    logger.info('[BreadcrumbReveal] ignored hidden path', { target })
    return false
  }

  const repoRoot = pickRepoRootForPath(target) || normalizePath(fileTreeStore.activeRepoPath)
  const activeRepo = normalizePath(fileTreeStore.activeRepoPath)

  try {
    // 目标仓库即使已经是当前仓库，也要确保它是展开态：
    // 否则树体被收起，展开目录/选中节点都发生在看不见的地方。
    if (repoRoot) {
      fileTreeStore.expandedRepos.set(repoRoot, true)
    }
    if (repoRoot && repoRoot !== activeRepo) {
      logger.info('[BreadcrumbReveal] switch repo', { from: activeRepo, to: repoRoot })
      fileTreeStore.setActiveRepoPath(repoRoot)
      fileTreeStore.expandedRepos.set(repoRoot, true)
      await loadRepoTree(repoRoot)
    } else if (repoRoot && (!Array.isArray(fileTree.value) || fileTree.value.length === 0)) {
      await loadRepoTree(repoRoot)
    }

    const ok = await revealInModeTree('kb', target, { align: 'center', ...options })
    logger.info('[BreadcrumbReveal] done', { repoRoot, target, ok })
    return ok
  } catch (e) {
    logger.error('[BreadcrumbReveal] failed', e)
    return false
  }
};

// 定位统一入口：左侧栏有三种模式，各自一棵树，按当前模式分派（面包屑在 plain/task 模式下
// 传的就是该模式的路径，不能再按知识库逻辑处理）。
const revealPathInSidebar = async (path, options = {}) => {
  if (viewMode.value === 'kb') return revealPathInTree(path, options)
  return revealInModeTree(viewMode.value, path, options)
}

// 打开文件后自动跟随（等价 VSCode 的 explorer.autoReveal）：点标签页、面包屑选文件、
// 程序化 openFileInTab 都会切换 activeTab，据此展开并滚动左栏树到该文件。
watch(
  () => fileTreeStore.activeTab,
  async (path) => {
    if (!path) return
    const ok = await revealPathInSidebar(path, { align: 'nearest' })
    logger.debug('[TreeReveal] follow active tab', { path, ok })
  }
)

// 处理文件创建事件
const handleFileCreated = async (filePath) => {
  const normalizedPath = normalizePath(filePath);
  const parentPath = normalizedPath.split('/').slice(0, -1).join('/');

  // 检查是否是隐藏路径
  if (isHiddenPath(normalizedPath) || isHiddenPath(parentPath)) {
    logger.info('跳过隐藏目录的文件创建事件:', normalizedPath);
    return;
  }

  await refreshDirectory(parentPath || fileTreeStore.activeRepoPath);
};

// 处理文件重命名事件
const handleFileRenamed = async (oldPath, newPath) => {
  if (!newPath) return;

  try {
    const normalizedNewPath = normalizePath(newPath);
    const normalizedOldPath = oldPath ? normalizePath(oldPath) : null;

    // 检查是否是隐藏路径
    if (isHiddenPath(normalizedNewPath) || (normalizedOldPath && isHiddenPath(normalizedOldPath))) {
      logger.info('跳过隐藏目录的文件重命名事件:', normalizedOldPath, '->', normalizedNewPath);
      return;
    }

    // 1. 更新树中的节点
    if (normalizedOldPath) {
      const nodeToUpdate = findNodeById(fileTree.value, normalizedOldPath);
      if (nodeToUpdate) {
        nodeToUpdate.id = normalizedNewPath;
        nodeToUpdate.name = newPath.split(/[\\/]/).pop();
      }
    }

    // 2. 更新标签页
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === normalizedOldPath);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].path = normalizedNewPath;
      fileTreeStore.openTabs[tabIndex].fileName = newPath.split(/[\\/]/).pop();

      if (fileTreeStore.activeTab === normalizedOldPath) {
        fileTreeStore.setActiveTab(normalizedNewPath);
      }
    }

    // 3. 刷新父目录
    const parentPath = normalizedOldPath
      ? normalizedOldPath.split('/').slice(0, -1).join('/')
      : normalizedNewPath.split('/').slice(0, -1).join('/');

    await refreshDirectory(parentPath || fileTreeStore.activeRepoPath);
  } catch (error) {
    logger.error('处理文件重命名时出错:', error);
  }
};

// 文件监控函数
const handleExternalFileModified = async (changedPath) => {
  const normalizedPath = normalizePath(changedPath)
  if (!normalizedPath || isHiddenPath(normalizedPath)) return

  try {
    const result = await editorBridgeStore.absorbExternalChange(normalizedPath, fileTreeStore)
    if (result?.status === 'synced') {
      logger.info('[EditorBridge] synchronized external file modification into open tab', {
        path: normalizedPath,
        result,
      })
    } else if (result?.status === 'conflict') {
      message.warning(result.message || '检测到外部修改冲突，已阻止自动覆盖')
    }
  } catch (error) {
    logger.warn('[EditorBridge] failed to absorb external file modification', {
      path: normalizedPath,
      error,
    })
  }
}

const setupFileWatcher = async () => {
  try {
    logger.info('开始监控目录:', fileTreeStore.activeRepoPath)

    // 启动后端监控
    await invoke('start_file_watching', {
      path: fileTreeStore.activeRepoPath
    })

    // 监听文件变化事件
    if (unlistenRef.value) {
      // 先移除旧监听器，避免重复
      await unlistenRef.value();
      unlistenRef.value = null;
    }

    // 监听文件变化事件
    const unlisten = await listen('file_change', (event) => {
      const payload = event.payload
      logger.info('文件系统变化事件:', payload)

      const changedPath = normalizePath(payload?.path)
      const oldChangedPath = normalizePath(payload?.old_path)

      if (isCellmetaPath(changedPath) || isCellmetaPath(oldChangedPath)) {
        cellmetaAutoCommitPending = true
        logger.info('[CellmetaAutoCommit] metadata sidecar changed; skip tree refresh and wait for file-close commit', {
          kind: payload?.kind,
          path: changedPath,
          oldPath: oldChangedPath,
          pending: cellmetaAutoCommitPending
        })
        return
      }

      if (payload?.kind === 'Modify') {
        void handleExternalFileModified(changedPath)
      }

      // 文件编辑模式 / 任务模式：刷新对应的树（不走 kb 逻辑）
      // Rename 事件需同时刷新旧父目录，传 old_path 让刷新逻辑覆盖原父目录残留节点。
      if (viewMode.value === 'plain' || viewMode.value === 'task') {
        const oldForRefresh = payload?.kind === 'Rename' ? oldChangedPath : ''
        scheduleNonKbTreeRefresh(changedPath, oldForRefresh)
        return
      }

      switch (payload.kind) {
        case 'Create':
          // logger.info('文件创建事件:', payload.path)
          handleFileCreated(payload.path)
          break
        case 'Remove':
          // logger.info('文件删除事件:', payload.path)
          handleFileRemoved(payload.path)
          break
        case 'Rename':
          // 处理old_path可能为null的情况
          const oldPath = payload.old_path || extractOldPathFromEvent(payload)
          // logger.info('文件重命名事件:', oldPath, '->', payload.path)
          handleFileRenamed(oldPath, payload.path)
          break
      }
    })
    unlistenRef.value = unlisten; // ★ 保存监听器

    logger.info('文件监控已启动')
  } catch (error) {
    logger.error('启动文件监控失败:', error)
  }
}

// 把全局文件监控重定向到指定目录（kb/plain/task 模式切换时调用）
const repointFileWatcher = async (path) => {
  const p = normalizePath(path)
  if (!p) return
  try { await invoke('start_file_watching', { path: p }) }
  catch (e) { logger.warn('重定向文件监控失败', e) }
}

// 文件编辑/任务模式下的树刷新（去抖）
// 设计说明：
// 1. 之前这里一律刷新到「模式根目录」（plain 的 externalDir / task 的分类根），
//    经 refreshModeDir 整树替换后会把已懒加载的 children 清空，导致打开文件触发的
//    file_change 把所有已展开目录一并收起（与 kb 模式 refreshDirectory 的局部刷新不一致）。
// 2. 现在改为刷新「变化文件的真实父目录」：单个文件变化只影响其所在父目录，
//    其它已展开目录既不会被整树替换、children 也不会被清空，展开态得以保留。
// 3. Rename 事件需要同时刷新新旧父目录，否则旧父目录里会残留已移走的节点。
// 4. 仅刷新在当前模式树范围内的目录；落在根以外或未加载分类的事件降级为刷新根。
let nonKbRefreshTimer = null
let nonKbRefreshPending = []
const scheduleNonKbTreeRefresh = (changedPath, oldPath = '') => {
  const next = normalizePath(changedPath)
  const old = normalizePath(oldPath)
  // 合并本轮待刷新目录：去重后只在去抖结束后刷一次，避免同一目录被多次拉取
  const candidates = new Set()
  if (next) candidates.add(parentDirOf(next))
  if (old && old !== next) candidates.add(parentDirOf(old))
  nonKbRefreshPending = Array.from(new Set([...nonKbRefreshPending, ...candidates]))

  if (nonKbRefreshTimer) clearTimeout(nonKbRefreshTimer)
  nonKbRefreshTimer = setTimeout(async () => {
    nonKbRefreshTimer = null
    const targets = nonKbRefreshPending
    nonKbRefreshPending = []
    try {
      if (viewMode.value === 'plain' && externalDir.value) {
        const root = normalizePath(externalDir.value)
        if (!targets.length) {
          await refreshModeDir('plain', root)
          return
        }
        for (const t of targets) {
          // 仅刷新落在当前外部目录树范围内的目录，避免误触其它路径
          // expand:false —— 保存 / 外部改动只刷新内容，不改变用户的展开状态
          if (t === root || t.startsWith(`${root}/`)) {
            await refreshModeDir('plain', t, { expand: false })
          }
        }
      } else if (viewMode.value === 'task') {
        const handledCats = new Set()
        for (const t of targets) {
          const cat = taskCatOfPath(t)
          if (!cat) continue
          handledCats.add(cat)
          // 落在已加载分类范围内的目录：刷新该父目录（expand:false，不改变展开状态）
          const catPath = catPathOf(cat)
          if (t === catPath || t.startsWith(`${catPath}/`)) {
            await refreshModeDir('task', t, { expand: false })
          }
        }
        // 事件落在未加载分类时降级刷新已加载分类根（维持原兜底行为）
        if (!handledCats.size) {
          for (const c of TASK_CATEGORIES) {
            if (taskCatTrees[c.key] && taskCatTrees[c.key].length) await loadTaskCat(c.key)
          }
        }
      }
    } catch (e) {
      logger.warn('非知识库树刷新失败', e)
    }
  }, 300)
}

// 辅助函数：从事件中提取旧路径（如果需要）
const extractOldPathFromEvent = (event) => {
  // 根据实际情况实现从事件中提取旧路径的逻辑
  return null // 如果无法提取则返回null
}

// 优化处理文件删除事件的函数
const handleFileRemoved = async (filePath) => {
  const removedPath = normalizePath(filePath)
  const parentPath = removedPath.split('/').slice(0, -1).join('/')

  if (isHiddenPath(removedPath) || isHiddenPath(parentPath)) {
    logger.info('跳过隐藏目录的文件删除事件:', removedPath)
    return
  }

  const removedNode = findNodeById(fileTree.value, removedPath)
  const isDir =
    !!removedNode?.is_directory ||
    fileTreeStore.openTabs.some((tab) => normalizePath(tab.path).startsWith(`${removedPath}/`))

  const removeFromTree = (nodes) => {
    if (!Array.isArray(nodes)) return []
    return nodes
      .filter((node) => normalizePath(node.id) !== removedPath)
      .map((node) => {
        if (node.children) node.children = removeFromTree(node.children)
        return node
      })
  }

  fileTree.value = removeFromTree(fileTree.value)

  if (selectedKeys.value?.length && normalizePath(selectedKeys.value[0]) === removedPath) {
    selectedKeys.value = []
    lastSelectedKey.value = null
  }

  if (expandedKeys.value?.length) {
    expandedKeys.value = expandedKeys.value.filter((key) => {
      const k = normalizePath(key)
      if (k === removedPath) return false
      if (isDir && k.startsWith(`${removedPath}/`)) return false
      return true
    })
  }

  const tabPathsToClose = fileTreeStore.openTabs
    .map((tab) => tab.path)
    .filter((p) => {
      const np = normalizePath(p)
      if (np === removedPath) return true
      return isDir && np.startsWith(`${removedPath}/`)
    })

  for (const p of tabPathsToClose) {
    fileTreeStore.closeTab(p)
  }

  // 仅做就地移除,不再全量刷新父目录:避免重建子节点导致已展开目录被收起。
  // 磁盘删除已完成,乐观移除即可保持树与展开状态一致。
};

// 清理函数
const cleanupFileWatcher = async () => {
  try {
    await invoke('stop_file_watching')

    if (unlistenRef.value && typeof unlistenRef.value === 'function') {
      await unlistenRef.value()
      unlistenRef.value = null
    }
    logger.info('文件监控已停止')
  } catch (error) {
    if (error && error.message && error.message.includes('unregisterListener')) {
      logger.warn('文件监控已经停止，无需重复停止')
      return
    }
    logger.error('停止文件监控失败:', error)
  }
}

const syncRepoPathsFromKnowledgeBases = (knowledgeBases) => {
  const list = Array.isArray(knowledgeBases) ? knowledgeBases : []
  const paths = list.map(kb => kb.location)
  fileTreeStore.setRepopathlist(paths)

  const currentActive = fileTreeStore.activeRepoPath
  const defaultActive = paths.length > 0 ? paths[0] : null

  if (!currentActive && defaultActive) {
    fileTreeStore.setActiveRepoPath(defaultActive)
  } else if (currentActive && !paths.includes(currentActive)) {
    fileTreeStore.setActiveRepoPath(defaultActive)
  }

  return paths
}

// 新增初始化函数
const initializeRepositories = async () => {
  logger.info('[Init] 开始初始化知识库')

  try {
    let knowledgeBases = knowledgeBaseStore.knowledgeBases
    if (!knowledgeBases || knowledgeBases.length === 0) {
      knowledgeBases = await knowledgeBaseStore.fetchKnowledgeBases()
    }
    logger.info('[Load] 获取知识库列表:', knowledgeBases)

    const paths = syncRepoPathsFromKnowledgeBases(knowledgeBases)

    if (!fileTreeStore.activeRepoPath && paths.length > 0) {
      logger.info('[Set] 设置默认激活路径:', paths[0])
    }

    return paths
  } catch (error) {
    logger.error('[Error] 初始化知识库失败:', error)
    throw error
  } finally {
    if (fileTreeStore.activeRepoPath) {
      await setupFileWatcher()
    }
  }
}

watch(
  () => knowledgeBaseStore.knowledgeBases,
  (list) => {
    syncRepoPathsFromKnowledgeBases(list)
  },
  { deep: true }
)

// 初始化时获取文件树数据
onMounted(async () => {
  // 0. 载入打开历史
  loadOpenHistory()

  // 1. 初始化知识库
  await initializeRepositories()

  // 2. 加载当前激活仓库的文件树
  if (fileTreeStore.activeRepoPath) {
    await loadRepoTree(fileTreeStore.activeRepoPath)
  }

  await nextTick()
  updateActiveRepoMaxHeight()

  // 从大脑等页面先 openFileInTab、再跳回主界面时，activeTab 在树挂载前就已变化，
  // watch(activeTab) 收不到事件。这里按当前激活标签补一次展开 + 选中。
  const initialRevealPath = fileTreeStore.activeTab
  if (initialRevealPath) {
    try {
      const ok = await revealPathInSidebar(initialRevealPath, { align: 'nearest' })
      logger.info('[TreeReveal] initial reveal from active tab', {
        path: initialRevealPath,
        ok,
      })
    } catch (e) {
      logger.debug('[TreeReveal] initial reveal failed', e)
    }
  }

  window.addEventListener('resize', updateActiveRepoMaxHeight)

  if (typeof ResizeObserver !== 'undefined' && fileTreeContainerRef.value) {
    const ro = new ResizeObserver(() => updateActiveRepoMaxHeight())
    ro.observe(fileTreeContainerRef.value)

    const headerEl = fileTreeContainerRef.value.querySelector('.list-header')
    if (headerEl) ro.observe(headerEl)

    activeRepoResizeObserver.value = ro
  }

  // 监听编辑器面包屑导航事件，用于联动定位
  try {
    const unlisten = await listen(EVENT_REVEAL_IN_FILE_TREE, async (event) => {
      const payload = event?.payload
      const path = typeof payload === 'string' ? payload : String(payload?.path || '')
      if (!path) return
      // 从面包屑来的定位：允许展开目标目录（其余调用方不传这个标志，行为不变）。
      const expandTargetDir = typeof payload === 'object' && payload?.expandTargetDir === true
      logger.info('[BreadcrumbReveal] event received', { path, expandTargetDir })
      await revealPathInSidebar(path, { expandTargetDir })
    })
    breadcrumbRevealUnlisten.value = unlisten
  } catch (e) {
    logger.error('[BreadcrumbReveal] listen failed', e)
  }

  // 添加文件监控
  // setupFileWatcher()
  cellmetaFileClosedHandler = (event) => {
    // 关闭文件只是触发后台提交边界，不等待 Git 命令完成，避免影响编辑器关闭标签的手感。
    scheduleCellmetaAutoCommitAfterFileClose(event?.detail || {})
  }
  window.addEventListener(EVENT_CELLMETA_FILE_CLOSED, cellmetaFileClosedHandler)
  logger.info('[CellmetaAutoCommit] listen file-close commit boundary', {
    event: EVENT_CELLMETA_FILE_CLOSED
  })
  startCellmetaAutoCommitScheduler()
})


onUnmounted(() => {
  window.removeEventListener('resize', updateActiveRepoMaxHeight)
  activeRepoResizeObserver.value?.disconnect?.()
  activeRepoResizeObserver.value = null
  if (cellmetaAutoCommitTimerId) {
    clearTimeout(cellmetaAutoCommitTimerId)
    cellmetaAutoCommitTimerId = null
  }
  if (cellmetaAutoCommitIntervalId) {
    clearInterval(cellmetaAutoCommitIntervalId)
    cellmetaAutoCommitIntervalId = null
  }
  if (cellmetaFileClosedHandler) {
    window.removeEventListener(EVENT_CELLMETA_FILE_CLOSED, cellmetaFileClosedHandler)
    cellmetaFileClosedHandler = null
  }

  // 清理编辑器面包屑联动监听
  try {
    Promise.resolve(breadcrumbRevealUnlisten.value?.()).catch((e) => {
      logger.debug('[BreadcrumbReveal] unlisten failed', e)
    })
  } catch (e) {
    logger.debug('[BreadcrumbReveal] unlisten failed', e)
  }
  breadcrumbRevealUnlisten.value = null

  // 清理所有状态
  editingNode.value = null
  editName.value = ''
  selectedNode.value = null
  expandedKeys.value = []
  lastSelectedKey.value = null

  // 清理文件监控 - 不使用 await
  cleanupFileWatcher().catch(error => {
    logger.error('清理文件监控时出错:', error)
  })
})

</script>

<style scoped>
/* 刷新图标样式 */
.refresh-icon {
  color: var(--n-text-color-secondary);
  cursor: pointer;
  transition: all 0.3s ease;
  padding: 4px;
}

.refresh-icon:hover {
  color: var(--n-color-primary);
  background-color: var(--n-color-primary-subtle);
  transform: rotate(90deg);
}

/* 暗色模式适配 */
.dark-mode .refresh-icon:hover {
  background-color: rgba(65, 105, 225, 0.2);
}

/* 新增的标题栏样式
 * 设计思路：顶部工具按钮数量固定，窄宽度下优先截断“知识编辑”标题，而不是让按钮换到第二行。
 * 主布局已提高左侧栏最小宽度，这里作为样式兜底，避免其它入口调整宽度时再次挤压。
 */
.list-header {
  flex: 0 0 auto;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  margin-bottom: 0;
  background-color: var(--guaxi-header-bg);
  border-radius: 0;
  border-bottom: 1px solid var(--guaxi-border);
  min-width: 0;
}

.header-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 8px;
}

.title-container {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  align-items: center;
  height: auto;
  /* 确保高度自适应 */
  gap: 6px;
}

.kb-manager-icon {
}

.repo-title-header {
  font-size: 18px !important;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  letter-spacing: 0.5px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin: 0;
  padding: 0;
  line-height: 1.2;
}

.repo-title-header::before {
  content: "";
  display: inline-block;
  width: 4px;
  height: 16px;
  background-color: var(--guaxi-text-primary);
  margin-right: 8px;
  border-radius: 2px;
}

.repo-title {
  font-size: 15px !important;
  font-weight: 500;
  color: var(--guaxi-text-primary);
  display: flex;
  align-items: center;
  margin: 0;
  padding: 0;
  line-height: 1.2;
}

/* 刷新按钮样式调整 */
.refresh-btn {
  background-color: transparent;
  color: var(--guaxi-text-secondary);
  border: none;
  transition: all 0.3s ease;
}

.refresh-btn:hover {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  transform: rotate(90deg);
}

.file-tree-container {
  height: 100%;
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-top-left-radius: 0;
  border-top-right-radius: 0;
  border-bottom-left-radius: 0px;
  border-bottom-right-radius: 0px;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05), 0 10px 28px rgba(0, 0, 0, 0.06);
  font-family: 'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.import-template-modal-content {
  width: 420px;
  max-width: 100%;
  box-sizing: border-box;
}

:deep(.import-template-modal-content .n-form),
:deep(.import-template-modal-content .n-form-item) {
  max-width: 100%;
}

:deep(.import-template-modal-content .n-form-item-blank) {
  flex: 1 1 0;
  min-width: 0;
}

:deep(.import-template-modal-content .n-input),
:deep(.import-template-modal-content .n-input-wrapper) {
  width: 100%;
  box-sizing: border-box;
}

:deep(.import-template-modal-content .n-input__input-el) {
  min-width: 0;
}

.toolbar {
  padding: 12px 16px;
  background-color: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title {
  font-size: 16px;
  font-weight: 600;
  color: var(--n-text-color);
}

.repo-body {
  flex: 1 1 auto;
  min-height: 0;
  height: auto;
  display: flex;
  flex-direction: column;
  position: relative;
}

.active-repo-wrapper {
  background-color: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  margin-top: 4px;
  margin-bottom: 0px;
  margin-left: 2px;
  margin-right: 1px;
  box-shadow: 0 1px 6px rgba(7, 118, 161, 0.06);
  overflow: hidden;
  box-sizing: border-box;
  /* 收起态：按内容自然高度，绝不被下方列表挤压变矮 */
  flex: 0 0 auto;
}

/* 激活的知识库展开后占满剩余高度。
   repo-body 已是确定高度(925px)的定位锚点，这里用绝对定位铺满，
   彻底绕开 flex-grow 在某些渲染帧下不生效、塌成内容高度的问题。 */
.repo-body.active-filled .active-repo-wrapper.expanded {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
}

/* 非占满态（激活条目收起时不会展开，此分支基本用于兜底）保持流式布局 */
.active-repo-wrapper.expanded {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}


.active-repo-wrapper .repo-actions {
  opacity: 1;
}

.active-repo-wrapper .repo-title,
.active-repo-wrapper .repo-icon,
.active-repo-wrapper .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

/* 激活条目展开时头部用大一点的内边距 */
.active-repo-wrapper.expanded .repo-item {
  padding: 12px 12px;
}

/* 激活条目收起时，补齐与未激活条目（list-item 5px + repo-item 4px）一致的高度 */
.active-repo-wrapper:not(.expanded) .repo-item {
  padding: 9px 12px;
}

.active-tree-container {
  flex: 1 1 auto;
  min-height: 0;
  padding: 2px 2px 6px 2px;
  overflow: hidden;
  overscroll-behavior: contain;
}

.active-tree-container .tree-component {
  height: 100%;
}

/* 未激活条目列表：
   - 激活条目已展开时，通过 v-show 整体隐藏（n-scrollbar 根节点非元素，v-show 必须挂在外层 div 上）；
   - 激活条目收起（或无激活条目）时，立即从底部顶上来填满剩余空间，排在激活条目之后待用户激活。 */
.repo-scroll-wrapper {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  /* 顶部淡出遮罩：未激活条目向上滚动越过顶边时平滑淡出，
     避免在激活条目下方出现生硬的半截被裁切的条目。 */
  -webkit-mask-image: linear-gradient(to bottom, transparent 0, #000 14px);
  mask-image: linear-gradient(to bottom, transparent 0, #000 14px);
}

.repo-scroll-container {
  flex: 1 1 auto;
  min-height: 0;
  background-color: transparent;
}

:deep(.repo-scroll-container .n-scrollbar-rail) {
  width: 7px;
  right: 2px;
}

:deep(.repo-scroll-container .n-scrollbar-rail__scrollbar) {
  border-radius: 8px;
  background-color: rgba(140, 140, 150, 0.4);
}

:deep(.repo-scroll-container .n-scrollbar-rail__scrollbar:hover) {
  background-color: rgba(120, 120, 130, 0.6);
}

.repo-list {
  border-radius: 0;
  background-color: transparent;
  margin-right: 8px;
}

/* 去掉 naive n-list-item__main 内部默认内边距，避免内容（尤其右侧图标）被推出卡片圆角外 */
:deep(.repo-list-item .n-list-item__main) {
  padding: 0;
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
}

/* 未激活条目沿用激活条目的圆润卡片风格 */
.repo-list-item {
  padding: 5px 12px;
  transition: background-color 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  border-radius: 12px;
  border: 1px solid var(--guaxi-border);
  background-color: var(--guaxi-panel-bg);
  box-shadow: 0 1px 6px rgba(7, 118, 161, 0.06);
  position: relative;
  margin: 4px;
  overflow: hidden;
  box-sizing: border-box;
}

.repo-list-item:last-child {
  border-bottom: 1px solid var(--guaxi-border);
}

/* 鼠标悬停效果 - 添加发光边框和背景 */
.repo-list-item:hover {
  background-color: var(--guaxi-header-bg);
  box-shadow: 0 1px 6px rgba(7, 118, 161, 0.06), inset 0 0 0 1px rgba(0, 0, 0, 0.08);
}

/* 鼠标悬停时的文字和图标效果 */
.repo-list-item:hover .repo-title {
  color: #1890ff;
}

.repo-list-item:hover .repo-icon {
  color: #1890ff;
}

.repo-list-item:hover .expand-icon {
  color: #1890ff;
}

/* 选中状态效果*/
.repo-list-item.active {
  background-color: var(--guaxi-header-bg);
  border-left: 3px solid var(--n-color-primary, #1890ff);
  box-shadow: inset 0 0 8px rgba(0, 0, 0, 0.08);
  position: sticky;
  top: 0;
  z-index: 3;
}


/* 暗色模式下选中状态更明显 */
.dark-mode .repo-list-item.active {
  background-color: rgba(255, 255, 255, 0.06);
  border-left: 3px solid #8ab4f8;
  box-shadow: inset 0 0 8px rgba(255, 255, 255, 0.08);
}

/* 选中状态下的文字和图标 */
.repo-list-item.active .repo-title,
.repo-list-item.active .repo-icon,
.repo-list-item.active .expand-icon {
  color: #1890ff;
  font-weight: 500;
}

.repo-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  cursor: pointer;
  padding: 4px 0;
}

.repo-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: var(--n-text-color);
  transition: color 0.2s ease;
}

.repo-icon {
  color: var(--n-color-primary);
  margin-right: 8px;
  transition: color 0.2s ease;
}

.expand-icon {
  transition: all 0.2s ease;
  color: var(--n-text-color);
}

.expand-icon.rotated {
  transform: rotate(90deg);
}

.tree-container {
  position: relative;
  width: 100%;
  padding: 8px 0 8px 24px;
  border-left: 1px dashed var(--n-border-color);
}

.tree-component {
  width: 100%;
  height: 100%;
}

/* 编辑输入框样式 */
.editable-input {
  border: 1px solid var(--n-border-color);
  border-radius: 4px;
  padding: 2px 4px;
  font-size: 14px;
  outline: none;
  width: 100%;
}

.editable-input:focus {
  border-color: var(--n-color-primary);
  box-shadow: 0 0 0 2px var(--n-color-primary-hover);
}

/* 刷新按钮动画 */
.refresh-btn {
  transition: transform 0.3s ease;
}

.refresh-btn:hover {
  transform: rotate(90deg);
}

/* 暗色模式适配 */
.dark-mode .file-tree-container {
  background-color: var(--guaxi-panel-bg);
  border-color: var(--guaxi-border);
}

.dark-mode .repo-scroll-container {
  background-color: transparent;
}

/* 选中状态下的 action/refresh 图标 */
.repo-list-item.active .action-icon,
.repo-list-item.active .refresh-icon,
.repo-list-item.active .browser-icon {
  color: var(--n-color-primary, #1890ff);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

/* 按下状态下的 action/refresh 图标 */
.action-icon:active,
.refresh-icon:active {
  color: var(--n-color-primary-hover, #1765ad);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

/* 暗色模式下 action/refresh 图标 */
.dark-mode .action-icon,
.dark-mode .refresh-icon,
.dark-mode .browser-icon,
.dark-mode .git-icon,
.dark-mode .kb-manager-icon {
  color: #b8c7ff;
  background-color: transparent;
}

.dark-mode .action-icon:hover,
.dark-mode .action-icon.active,
.dark-mode .refresh-icon:hover,
.dark-mode .refresh-icon.active,
.dark-mode .browser-icon:hover,
.dark-mode .browser-icon.active,
.dark-mode .git-icon:hover,
.dark-mode .git-icon.active,
.dark-mode .kb-manager-icon:hover,
.dark-mode .kb-manager-icon.active,
.dark-mode .repo-list-item.active .action-icon,
.dark-mode .repo-list-item.active .refresh-icon,
.dark-mode .repo-list-item.active .browser-icon {
  color: #8ab4f8;
  background-color: rgba(65, 105, 225, 0.13);
}

/* 新增的样式 */
.repo-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.repo-list-item:hover .repo-actions {
  opacity: 1;
}

/* Action/refresh 图标基础样式（合并） */
.action-icon,
.refresh-icon,
.browser-icon,
.git-icon,
.open-icon,
.mode-toggle-icon,
.kb-manager-icon {
  color: var(--guaxi-text-secondary);
  background-color: transparent;
  transition: color 0.2s, background-color 0.2s;
  border-radius: 4px;
  padding: 2px;
  cursor: pointer;
}

.action-icon:hover,
.action-icon.active,
.refresh-icon:hover,
.refresh-icon.active,
.browser-icon:hover,
.browser-icon.active,
.git-icon:hover,
.git-icon.active,
.open-icon:hover,
.open-icon.active,
.mode-toggle-icon:hover,
.mode-toggle-icon.active,
.kb-manager-icon:hover,
.kb-manager-icon.active {
  color: var(--n-color-primary, #1890ff);
  background-color: var(--n-color-primary-suppl, #e6f7ff);
}

.neovim-action-icon {
  display: block;
  flex: 0 0 auto;
}

/* ───── 普通文件编辑模式：外部目录树 / 历史列表 ───── */
.plain-body {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.external-dir-wrapper {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  margin: 6px;
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  background-color: var(--guaxi-panel-bg);
  box-shadow: 0 1px 6px rgba(7, 118, 161, 0.06);
  overflow: hidden;
  box-sizing: border-box;
}

.external-dir-header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.external-dir-header .repo-title {
  color: #1890ff;
  font-weight: 500;
  min-width: 0;
  overflow: hidden;
}

.external-dir-header .repo-title span,
.history-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-wrapper {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.history-header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  color: var(--guaxi-text-secondary);
}

.history-title {
  font-size: 13px;
  font-weight: 500;
}

.history-empty {
  padding: 24px 16px;
  text-align: center;
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}

.history-item {
  flex-direction: column;
  align-items: stretch;
  cursor: pointer;
}

.history-item .repo-item {
  padding: 2px 0;
}

.history-path {
  margin-top: 2px;
  padding-left: 24px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

<!-- 右键菜单 teleport 到 body，scoped 样式无法命中，这里用全局样式统一字体与文件树一致 -->
<style>
.file-tree-context-menu.n-dropdown,
.file-tree-context-menu .n-dropdown-option,
.file-tree-context-menu .n-dropdown-option-body,
.file-tree-context-menu .n-dropdown-option-body__label {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}
</style>
