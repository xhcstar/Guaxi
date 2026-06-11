<template>
  <div class="file-tree-container" ref="fileTreeContainerRef">
    <!-- 工具栏 -->
    <div class="list-header">
      <div class="title-container">
        <n-text class="repo-title-header">知识编辑</n-text>
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
              :class="{ active: viewMode === 'plain' }"
              @click="toggleViewMode"
            >
              <n-icon :component="viewMode === 'plain' ? LibraryOutline : SwapHorizontalOutline" size="16" />
            </n-icon-wrapper>
          </template>
          {{ viewMode === 'plain' ? '切换回知识库模式' : '切换到文件编辑模式' }}
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
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleImportMarkdownFile(activeRepoPathItem)">
                  <n-icon :component="DownloadOutline" size="16" />
                </n-icon-wrapper>
              </template>
              导入 Markdown 文件
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFile(activeRepoPathItem)">
                  <n-icon :component="FileAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              新建文件
            </n-tooltip>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(activeRepoPathItem)">
                  <n-icon :component="FolderAddIcon" size="16" />
                </n-icon-wrapper>
              </template>
              新建文件夹
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
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleImportMarkdownFile(path)">
                      <n-icon :component="DownloadOutline" size="16" />
                    </n-icon-wrapper>
                  </template>
                  导入 Markdown 文件
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFile(path)">
                      <n-icon :component="FileAddIcon" size="16" />
                    </n-icon-wrapper>
                  </template>
                  新建文件
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click.stop="handleAddFolder(path)">
                      <n-icon :component="FolderAddIcon" size="16" />
                    </n-icon-wrapper>
                  </template>
                  新建文件夹
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

    <!-- 普通文件编辑模式：外部目录树 / 打开历史列表 -->
    <div v-else class="repo-body plain-body">
      <!-- 已激活某个外部目录：显示其文件树 -->
      <div v-if="externalDir" class="external-dir-wrapper">
        <div class="external-dir-header">
          <div class="repo-title" :title="externalDir">
            <n-icon size="16" class="repo-icon"><FolderIcon /></n-icon>
            {{ getDirDisplayName(externalDir) }}
          </div>
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="22" :border-radius="4" class="action-icon" @click="backToHistory">
                <n-icon :component="TimeOutline" size="16" />
              </n-icon-wrapper>
            </template>
            返回历史列表
          </n-tooltip>
        </div>
        <div class="active-tree-container" @wheel.stop>
          <GuaxiVirtualFileTree
            class="tree-component"
            :nodes="externalDirTree"
            v-model:expanded-keys="externalExpandedKeys"
            v-model:selected-keys="externalSelectedKeys"
            :root-id="externalDir"
            @open-file="handleExternalOpenFile"
            @load-children="handleExternalLoadChildren"
          />
        </div>
      </div>

      <!-- 未激活外部目录：显示打开历史列表 -->
      <div v-else class="history-wrapper">
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
import { DocumentTextOutline, FolderOutline, AddOutline, TrashOutline, CreateOutline, MoveOutline, FolderOpenOutline, GlobeOutline, SettingsOutline, GitBranchOutline, DownloadOutline, FolderOpenOutline as OpenIcon, DocumentOutline, SwapHorizontalOutline, LibraryOutline, TimeOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import GuaxiVirtualFileTree from './GuaxiVirtualFileTree.vue';
import { useSettingsStore } from '../../stores/settingsStore'
import { h, ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { listen } from '@tauri-apps/api/event'
import { NList, NListItem, NText, NTooltip, NIconWrapper } from 'naive-ui'
import { RefreshOutline as RefreshIcon, ChevronDownOutline as ChevronDownIcon, ChevronForwardOutline as ChevronForwardIcon } from '@vicons/ionicons5'
import { Folder20Regular as FolderIcon, ChannelAdd20Regular as FileAddIcon, FolderAdd20Regular as FolderAddIcon } from '@vicons/fluent'

const logger = createLogger('GuaxiFileTree')
const message = useMessage();

// 编辑器面包屑联动：用于从编辑器定位到文件树节点
const EVENT_REVEAL_IN_FILE_TREE = 'guaxi_editor_reveal_in_file_tree'
const breadcrumbRevealUnlisten = ref(null)

// 处理文件树容器的右键点击事件
const treeRef = ref(null)

const BROWSER_PREVIEW_URL = 'http://localhost:8888/browser/knowledge-bases'
const browserPreviewEnabled = ref(false)
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

const toggleGitPanel = async () => {
  if (!settingsStore.knowledgeBasesRoot) {
    await settingsStore.get_knowledgeBasesRoot()
  }

  const workspacePath = settingsStore.knowledgeBasesRoot
  if (!workspacePath) {
    message.error('未配置知识库根目录')
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

// 引入 Pinia store
const fileTreeStore = useFileTreeStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const settingsStore = useSettingsStore()
const lastSelectedKey = ref(null);

const fileTreeContainerRef = ref(null)
const repoScrollbarRef = ref(null)
const activeRepoMaxHeight = ref(360)
const activeRepoResizeObserver = ref(null)

// ───── 视图模式：'kb' 知识库模式（默认） / 'plain' 普通文件编辑模式 ─────
const viewMode = ref('kb')
// plain 模式下当前激活的外部目录（为空则显示历史列表）
const externalDir = ref(null)
const externalDirTree = ref([])
const externalExpandedKeys = ref([])
const externalSelectedKeys = ref([])

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

const scrollToKey = async (key) => {
  await nextTick()
  await new Promise((r) => requestAnimationFrame(r))

  const tree = treeRef.value
  if (!tree || typeof tree.scrollTo !== 'function') return

  try {
    await Promise.resolve(tree.scrollTo({ key }))
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
  const set = new Set((existingNames || []).map((x) => String(x || '').toLowerCase()));
  const original = String(desiredName || '').trim();
  if (!original) return '';

  if (!set.has(original.toLowerCase())) return original;

  const dot = original.lastIndexOf('.');
  const hasExt = dot > 0 && dot < original.length - 1;
  const base = hasExt ? original.slice(0, dot) : original;
  const ext = hasExt ? original.slice(dot) : '';

  for (let i = 2; i < 2000; i += 1) {
    const next = `${base} (${i})${ext}`;
    if (!set.has(next.toLowerCase())) return next;
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

const handleImportMarkdownFile = async (repoPath) => {
  const targetDir = guessImportTargetDir(repoPath);
  if (!targetDir) {
    message.warning('未找到导入目标目录');
    return;
  }

  try {
    const picked = await open({
      multiple: false,
      filters: [{ name: 'Markdown', extensions: ['md', 'markdown'] }]
    });

    if (!picked) return;

    const srcPath = normalizePath(picked);
    const fileName = srcPath.split('/').pop();
    if (!fileName) {
      message.error('导入失败：无法解析文件名');
      return;
    }

    const dirNodes = await invoke('get_file_tree', { path: targetDir });
    const existingNames = Array.isArray(dirNodes) ? dirNodes.map((n) => n?.name).filter(Boolean) : [];

    const uniqueName = makeUniqueFileName(existingNames, fileName);
    const dstPath = normalizePath(`${targetDir}/${uniqueName}`);

    logger.info('[ImportMarkdown] start', {
      repoPath: normalizePath(repoPath),
      targetDir,
      srcPath,
      dstPath
    });

    const content = await invoke('read_file_content', { path: srcPath });
    await invoke('write_file_content', { path: dstPath, content: String(content ?? '') });

    await refreshDirectory(targetDir);

    logger.info('[ImportMarkdown] success', { dstPath, len: String(content ?? '').length });
    message.success('Markdown 文件已导入');
  } catch (error) {
    logger.error('[ImportMarkdown] failed:', error);
    message.error(`导入失败: ${error.message || error}`);
    await refreshDirectory(targetDir);
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
    const res = await fileTreeStore.fetchFileContent(node.id)
    if (res?.ok) {
      await fileTreeStore.openFileInTab(node.id, res.content, { unsupported: !!res.unsupported })
      if (res.unsupported) {
        message.warning('不支持打开该文件类型（仅支持文本/Markdown）')
      }
    } else {
      message.warning('文件不存在或已被删除')
      await handleFileRemoved(node.id)
    }
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
    const res = await fileTreeStore.fetchFileContent(filePath)
    if (res?.ok) {
      await fileTreeStore.openFileInTab(filePath, res.content, { unsupported: !!res.unsupported })
      if (res.unsupported) message.warning('不支持打开该文件类型（仅支持文本/Markdown）')
    } else {
      message.error('无法打开该文件')
    }
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
  externalExpandedKeys.value = []
  externalSelectedKeys.value = []
  externalDirTree.value = []
  try {
    await fileTreeStore.fetchFileTree(norm, { force: true })
    const filtered = filterHiddenTree(fileTreeStore.fileTree)
    externalDirTree.value = sortFileTree(normalizeTreeIds(filtered))
  } catch (error) {
    logger.error('加载外部目录树失败:', error)
    message.error('加载目录失败')
  }
}

// 切换视图模式：kb ⇄ plain
const toggleViewMode = () => {
  if (viewMode.value === 'kb') {
    viewMode.value = 'plain'
    // 进入 plain 模式：若没有已激活的外部目录则展示历史列表
  } else {
    viewMode.value = 'kb'
    externalDir.value = null
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

// 外部目录树：打开文件
const handleExternalOpenFile = async (node) => {
  if (!node || node.is_directory) return
  externalSelectedKeys.value = [node.id]
  try {
    const res = await fileTreeStore.fetchFileContent(node.id)
    if (res?.ok) {
      await fileTreeStore.openFileInTab(node.id, res.content, { unsupported: !!res.unsupported })
      if (res.unsupported) message.warning('不支持打开该文件类型（仅支持文本/Markdown）')
    } else {
      message.warning('文件不存在或已被删除')
    }
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
    await invoke('move_path', { sourcePath, destinationPath })

    // 更新打开的标签页路径(文件本身或目录前缀)
    fileTreeStore.openTabs.forEach((tab) => {
      const tp = normalizePath(tab.path)
      if (tp === sourcePath) {
        const wasActive = normalizePath(fileTreeStore.activeTab) === tp
        tab.path = destinationPath
        tab.fileName = name
        if (wasActive) fileTreeStore.setActiveTab(destinationPath)
      } else if (tp.startsWith(`${sourcePath}/`)) {
        const np = destinationPath + tp.slice(sourcePath.length)
        const wasActive = normalizePath(fileTreeStore.activeTab) === tp
        tab.path = np
        if (wasActive) fileTreeStore.setActiveTab(np)
      }
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
const handleContextMenuSelect = async (key) => {
  isContextMenuVisible.value = false;
  try {
    switch (key) {
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
      case 'openInBrowser':
        if (selectedNode.value && !selectedNode.value.is_directory) {
          const nodePath = normalizePath(selectedNode.value.id)
          if (!isMarkdownFilePath(nodePath)) {
            message.warning('仅支持 Markdown 文件')
            break
          }
          const url = `http://localhost:8888/browser/preview?path=${encodeURIComponent(nodePath)}`
          await openUrl(url)
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
  }

  if (!isRoot && parentNode?.id) {
    ensureExpandedForDir(parentNode.id)
  }

  startEditing(newNode);

  await nextTick()
  await scrollToKey(newNode.id)
  await focusEditingInput(newNode.id)

  selectedKeys.value = [];

  return new Promise((resolve) => {
    const unwatch = watch(editingNode, (newVal) => {
      if (!newVal) {
        unwatch();
        handleEditComplete(newNode, parentNode, isDirectory, isRoot, resolve);
      }
    });
  });
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
    logger.error('创建失败:', error);
    removeTempNode()
    message.error(`创建失败: ${error.message}`);
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
    if (node.is_directory) {
      await invoke('rename_directory', { oldPath: oldId, newPath: newId });
    } else {
      await invoke('rename_path', { oldPath: oldId, newPath: newId });
    }

    // 更新标签页
    const tabIndex = fileTreeStore.openTabs.findIndex(tab => tab.path === oldId);
    if (tabIndex !== -1) {
      fileTreeStore.openTabs[tabIndex].path = newId;
      fileTreeStore.openTabs[tabIndex].fileName = editName.value;
      // 同步内容（如果内容有变化）
      fileTreeStore.openTabs[tabIndex].content = node.content || fileTreeStore.openTabs[tabIndex].content;
      if (fileTreeStore.activeTab === oldId) {
        fileTreeStore.setActiveTab(newId);
      }
    }
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
    await invoke('delete_path', { path: normalizedId })
    await handleFileRemoved(normalizedId)
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

    await invoke('move_path', { sourcePath, destinationPath });

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

    const tabIndex = fileTreeStore.openTabs.findIndex((tab) => normalizePath(tab.path) === sourcePath);
    if (tabIndex !== -1) {
      const tab = fileTreeStore.openTabs[tabIndex];
      tab.path = destinationPath;
      tab.fileName = destinationPath.split('/').pop();
      if (fileTreeStore.activeTab && normalizePath(fileTreeStore.activeTab) === sourcePath) {
        fileTreeStore.setActiveTab(destinationPath);
      }
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

// 由编辑器面包屑触发：切换仓库、展开并选中目标节点
const revealPathInTree = async (absPath) => {
  const target = normalizePath(absPath)
  if (!target) return

  if (isHiddenPath(target)) {
    logger.info('[BreadcrumbReveal] ignored hidden path', { target })
    return
  }

  const repoRoot = pickRepoRootForPath(target) || normalizePath(fileTreeStore.activeRepoPath)
  const activeRepo = normalizePath(fileTreeStore.activeRepoPath)

  try {
    if (repoRoot && repoRoot !== activeRepo) {
      logger.info('[BreadcrumbReveal] switch repo', { from: activeRepo, to: repoRoot })
      fileTreeStore.setActiveRepoPath(repoRoot)
      fileTreeStore.expandedRepos.set(repoRoot, true)
      await loadRepoTree(repoRoot)
    } else if (repoRoot && (!Array.isArray(fileTree.value) || fileTree.value.length === 0)) {
      await loadRepoTree(repoRoot)
    }

    const ancestors = buildAncestorDirs(repoRoot || activeRepo, target)
    if (ancestors.length) {
      expandedKeys.value = Array.from(new Set([...(expandedKeys.value || []), ...ancestors]))
    }

    selectedKeys.value = [target]
    lastSelectedKey.value = target

    await nextTick()

    try {
      treeRef.value?.scrollTo?.({ key: target })
    } catch (e) {
      logger.debug('[BreadcrumbReveal] tree scrollTo not available or failed', e)
    }

    logger.info('[BreadcrumbReveal] done', { repoRoot, target, expanded: ancestors.length })
  } catch (e) {
    logger.error('[BreadcrumbReveal] failed', e)
  }
};

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
      logger.info('[BreadcrumbReveal] event received', { path })
      await revealPathInTree(path)
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

/* 新增的标题栏样式 */
.list-header {
  flex: 0 0 auto;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 14px;
  margin-bottom: 0;
  background-color: var(--guaxi-header-bg);
  border-radius: 0;
  border-bottom: 1px solid var(--guaxi-border);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.title-container {
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
