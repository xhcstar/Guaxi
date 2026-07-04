<template>
  <n-message-provider placement="bottom">
    <div class="resource-repo-manager page">
      <header class="header">
        <div class="header-left">
          <div class="title">资源库管理</div>
          <n-tag :type="syncUi.tagType" size="small">{{ syncUi.tagText }}</n-tag>
        </div>

        <div class="header-actions">
          <div class="header-tool-group">
            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :disabled="loading || !resourceRoot" @click="openCreateModal('folder')">
                  <template #icon><n-icon :component="FolderAdd20Regular" /></template>
                </n-button>
              </template>
              新建文件夹
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :disabled="loading || !resourceRoot" @click="openCreateModal('file')">
                  <template #icon><n-icon :component="DocumentAdd20Regular" /></template>
                </n-button>
              </template>
              新建文件
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :disabled="loading || !selectedPath" @click="openRenameModal">
                  <template #icon><n-icon :component="Edit20Regular" /></template>
                </n-button>
              </template>
              重命名
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :disabled="loading || !selectedPath" @click="openSelectedInExplorer">
                  <template #icon><n-icon :component="Open20Regular" /></template>
                </n-button>
              </template>
              打开所选
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle type="error" :disabled="loading || !selectedPath" @click="requestDeleteSelected">
                  <template #icon><n-icon :component="Delete20Regular" /></template>
                </n-button>
              </template>
              删除所选
            </n-tooltip>
          </div>

          <n-button-group size="small" class="view-switcher">
            <n-button :type="viewMode === 'tree' ? 'primary' : 'default'" circle @click="setViewMode('tree')">
              <template #icon><n-icon :component="TextBulletListTree20Regular" /></template>
            </n-button>
            <n-button :type="viewMode === 'list' ? 'primary' : 'default'" circle @click="setViewMode('list')">
              <template #icon><n-icon :component="List20Regular" /></template>
            </n-button>
            <n-button :type="viewMode === 'grid' ? 'primary' : 'default'" circle @click="setViewMode('grid')">
              <template #icon><n-icon :component="Grid20Regular" /></template>
            </n-button>
          </n-button-group>

          <div class="header-tool-group">
            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :loading="loadingTree" @click="refresh">
                  <template #icon><n-icon :component="ArrowSync20Regular" /></template>
                </n-button>
              </template>
              刷新
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :disabled="loading || !resourceRoot" @click="openInExplorer">
                  <template #icon><n-icon :component="FolderOpen20Regular" /></template>
                </n-button>
              </template>
              打开资源库目录
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle :disabled="loading" @click="testConnection">
                  <template #icon><n-icon :component="PlugConnected20Regular" /></template>
                </n-button>
              </template>
              测试连接
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" circle type="primary" :disabled="loading" @click="syncNow">
                  <template #icon><n-icon :component="CloudArrowUp20Regular" /></template>
                </n-button>
              </template>
              立即同步
            </n-tooltip>
          </div>
        </div>
      </header>

      <main class="main">
        <section class="workspace">
          <div class="workspace-toolbar">
            <div class="toolbar-top">
              <n-input
                v-model:value="searchKeyword"
                class="search"
                size="medium"
                clearable
                placeholder="搜索已加载的资源"
              >
                <template #prefix>
                  <n-icon :component="Search20Regular" />
                </template>
              </n-input>
            </div>
          </div>

          <div class="browser">
            <div v-if="viewMode !== 'tree'" class="browser-head">
              <n-button size="small" circle :disabled="!canGoUp" @click="goUpDirectory">
                <template #icon><n-icon :component="ArrowUp20Regular" /></template>
              </n-button>
              <div class="current-dir">
                <n-icon :component="FolderOpen20Regular" />
                <span>{{ currentDirLabel }}</span>
              </div>
              <span class="browser-count">{{ visibleItems.length }} 项</span>
            </div>

            <n-spin :show="loadingTree">
              <n-empty v-if="!resourceRoot" description="资源库根目录未配置" />
              <n-empty v-else-if="flatItems.length === 0" description="资源库为空" />
              <n-empty v-else-if="browserIsEmpty" :description="browserEmptyDescription" />

              <n-tree
                v-else-if="viewMode === 'tree'"
                class="resource-tree"
                :data="filteredTreeData"
                key-field="key"
                label-field="label"
                children-field="children"
                :default-expand-all="false"
                :expanded-keys="expandedKeys"
                :selected-keys="selectedKeys"
                :render-label="renderLabel"
                :render-prefix="renderPrefix"
                block-line
                @update:expanded-keys="handleExpandedKeysUpdate"
                @update:selected-keys="handleSelectedKeysUpdate"
              />

              <div v-else-if="viewMode === 'list'" class="list-view">
                <button
                  v-for="item in visibleItems"
                  :key="item.key"
                  class="list-row"
                  :class="{ selected: selectedPath === item.key }"
                  type="button"
                  @dblclick="handleItemOpen(item)"
                  @click="handleItemClick(item)"
                >
                  <n-icon class="row-icon" :component="item.isDir ? Folder20Regular : Document20Regular" />
                  <span class="row-name" :style="{ paddingLeft: `${item.depth * 10}px` }">{{ item.label }}</span>
                  <span class="row-kind">{{ item.isDir ? '目录' : fileExtensionLabel(item.label) }}</span>
                  <span class="row-path">{{ relativePath(item.key) }}</span>
                </button>
              </div>

              <div v-else class="grid-view">
                <button
                  v-for="item in visibleItems"
                  :key="item.key"
                  class="grid-cell"
                  :class="{ selected: selectedPath === item.key }"
                  type="button"
                  @dblclick="handleItemOpen(item)"
                  @click="handleItemClick(item)"
                >
                  <n-icon class="grid-icon" :component="item.isDir ? Folder20Regular : Document20Regular" />
                  <span class="grid-name">{{ item.label }}</span>
                  <span class="grid-meta">{{ item.isDir ? '目录' : fileExtensionLabel(item.label) }}</span>
                </button>
              </div>
            </n-spin>
          </div>
        </section>

        <aside class="side">
          <section class="side-section">
            <div class="side-title">
              <n-icon :component="Info20Regular" />
              <span>详情</span>
            </div>
            <div class="detail-line">
              <span class="detail-label">选中</span>
              <span class="detail-value">{{ selectedNode?.label || '未选择' }}</span>
            </div>
            <div class="detail-line">
              <span class="detail-label">类型</span>
              <span class="detail-value">{{ selectedNode ? (selectedNode.isDir ? '目录' : '文件') : '-' }}</span>
            </div>
            <div class="detail-line">
              <span class="detail-label">位置</span>
              <span class="detail-value">{{ selectedPath ? relativePath(selectedPath) : '-' }}</span>
            </div>
            <div class="detail-line">
              <span class="detail-label">新建位置</span>
              <span class="detail-value">{{ createTargetDir ? relativePath(createTargetDir) || '根目录' : '-' }}</span>
            </div>
          </section>

          <section class="side-section">
            <div class="side-title">
              <n-icon :component="DataBarVertical20Regular" />
              <span>统计</span>
            </div>
            <div class="side-stats">
              <div class="side-stat">
                <span class="side-stat-value">{{ repoStats.total }}</span>
                <span class="side-stat-label">全部</span>
              </div>
              <div class="side-stat">
                <span class="side-stat-value">{{ repoStats.dirs }}</span>
                <span class="side-stat-label">目录</span>
              </div>
              <div class="side-stat">
                <span class="side-stat-value">{{ repoStats.files }}</span>
                <span class="side-stat-label">文件</span>
              </div>
              <div class="side-stat">
                <span class="side-stat-value">{{ flatFilteredItems.length }}</span>
                <span class="side-stat-label">匹配</span>
              </div>
              <div class="side-stat wide">
                <span class="side-stat-value">{{ displayedItemCount }}</span>
                <span class="side-stat-label">当前视图</span>
              </div>
            </div>
          </section>

          <section class="side-section">
            <div class="side-title">
              <n-icon :component="ArrowSync20Regular" />
              <span>同步</span>
            </div>
            <div class="sync-state-summary">
              <div class="sync-state-title">{{ syncUi.title }}</div>
              <div class="sync-state-detail">{{ syncState.detail || `最近操作：${syncState.action || '-'} · ${syncState.at || '-'}` }}</div>
            </div>
            <div class="sync-head">
              <n-tag :type="syncProgress.provider === 'p2p' ? 'success' : 'default'" size="small">
                {{ syncProgress.provider === 'p2p' ? 'P2P' : '通用' }}
              </n-tag>
              <span>{{ syncProgress.phase || 'idle' }}</span>
            </div>
            <n-progress
              type="line"
              :percentage="syncProgressPercent"
              :processing="syncProgress.status === 'running'"
              :status="syncProgressBarStatus"
            />
            <div class="sync-progress-meta">
              <div>{{ syncProgress.message || '尚未收到后端同步进度事件' }}</div>
              <div>文件：{{ syncProgress.current || 0 }}/{{ syncProgress.total || 0 }}</div>
              <div>数据：{{ formatBytes(syncProgress.bytes_current || 0) }}/{{ formatBytes(syncProgress.bytes_total || 0) }}</div>
              <div v-if="syncProgress.file_path">当前：{{ syncProgress.file_path }}</div>
              <div v-if="syncProgress.at">时间：{{ syncProgress.at }}</div>
            </div>
          </section>
        </aside>
      </main>

      <n-modal v-model:show="createModalVisible" preset="card" class="resource-modal" :title="createMode === 'folder' ? '新建文件夹' : '新建文件'">
        <div class="modal-body">
          <div class="modal-hint">创建到：{{ relativePath(createTargetDir) || '根目录' }}</div>
          <n-input
            v-model:value="createName"
            autofocus
            clearable
            :placeholder="createMode === 'folder' ? '文件夹名称' : '文件名称'"
            @keyup.enter="confirmCreate"
          />
        </div>
        <template #footer>
          <div class="modal-actions">
            <n-button size="small" @click="createModalVisible = false">取消</n-button>
            <n-button size="small" type="primary" :loading="loading" @click="confirmCreate">创建</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="renameModalVisible" preset="card" class="resource-modal" title="重命名">
        <div class="modal-body">
          <div class="modal-hint">当前：{{ selectedNode?.label || '-' }}</div>
          <n-input v-model:value="renameName" autofocus clearable placeholder="新名称" @keyup.enter="confirmRename" />
        </div>
        <template #footer>
          <div class="modal-actions">
            <n-button size="small" @click="renameModalVisible = false">取消</n-button>
            <n-button size="small" type="primary" :loading="loading" @click="confirmRename">保存</n-button>
          </div>
        </template>
      </n-modal>

      <n-modal v-model:show="deleteModalVisible" preset="card" class="resource-modal danger-modal" title="删除资源">
        <div class="modal-body">
          <div class="danger-title">确定删除「{{ selectedNode?.label || selectedPath }}」吗？</div>
          <div class="modal-hint">目录会连同内部资源一起删除，此操作不可从应用内撤销。</div>
        </div>
        <template #footer>
          <div class="modal-actions">
            <n-button size="small" @click="deleteModalVisible = false">取消</n-button>
            <n-button size="small" type="error" :loading="loading" @click="confirmDeleteSelected">删除</n-button>
          </div>
        </template>
      </n-modal>
    </div>
  </n-message-provider>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, h, onMounted, onUnmounted, ref } from 'vue'
import {
  NButton,
  NButtonGroup,
  NEmpty,
  NIcon,
  NInput,
  NMessageProvider,
  NModal,
  NProgress,
  NSpin,
  NTag,
  NTooltip,
  NTree,
  useMessage,
} from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  ArrowUp20Regular,
  ArrowSync20Regular,
  CloudArrowUp20Regular,
  DataBarVertical20Regular,
  Delete20Regular,
  Document20Regular,
  DocumentAdd20Regular,
  Edit20Regular,
  Folder20Regular,
  FolderAdd20Regular,
  FolderOpen20Regular,
  Grid20Regular,
  Info20Regular,
  List20Regular,
  Open20Regular,
  PlugConnected20Regular,
  Search20Regular,
  TextBulletListTree20Regular,
} from '@vicons/fluent'

const logger = createLogger('GuaxiResourceRepoManager')
const message = useMessage()

const RESOURCE_REPO_SYNC_STATUS_KEY = 'guaxi:resourceRepoSyncStatus:v1'

const nowLabel = () => {
  try {
    return new Date().toLocaleString()
  } catch {
    return ''
  }
}

const formatInvokeError = (e) => {
  const msg = e?.message ?? (typeof e?.toString === 'function' ? e.toString() : String(e))
  const s = typeof msg === 'string' ? msg.trim() : ''
  return s || '操作失败'
}

const readSyncState = () => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY)
    if (!raw) return { status: 'idle', action: '', at: '', detail: '' }
    const v = JSON.parse(raw)
    return {
      status: String(v?.status || 'idle'),
      action: String(v?.action || ''),
      at: String(v?.at || ''),
      detail: String(v?.detail || ''),
    }
  } catch (e) {
    logger.warn('[ResourceRepoManager] read sync state failed', e)
    return { status: 'idle', action: '', at: '', detail: '' }
  }
}

const writeSyncState = (next) => {
  try {
    localStorage.setItem(RESOURCE_REPO_SYNC_STATUS_KEY, JSON.stringify(next))
  } catch (e) {
    logger.warn('[ResourceRepoManager] write sync state failed', e)
  }
}

const syncState = ref(readSyncState())
const syncStateRaw = ref('')
const syncProgress = ref({
  run_id: '',
  provider: '',
  target: '',
  phase: '',
  status: 'idle',
  message: '',
  current: 0,
  total: 0,
  bytes_current: 0,
  bytes_total: 0,
  file_path: '',
  at: '',
})
let syncPollTimer = null
let syncProgressUnlisten = null

const applySyncState = (next, source) => {
  if (!next) return
  syncState.value = {
    status: String(next?.status || 'idle'),
    action: String(next?.action || ''),
    at: String(next?.at || ''),
    detail: String(next?.detail || ''),
  }
  logger.info('[ResourceRepoManager] sync state update', { source, state: syncState.value })
}

const refreshSyncStateFromStorage = (source) => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY) || ''
    if (raw && raw === syncStateRaw.value) return
    syncStateRaw.value = raw
    if (!raw) {
      applySyncState({ status: 'idle', action: '', at: '', detail: '' }, source)
      return
    }
    const v = JSON.parse(raw)
    applySyncState(v, source)
  } catch (e) {
    logger.warn('[ResourceRepoManager] refresh sync state failed', e)
  }
}

const handleSyncStateEvent = (e) => {
  applySyncState(e?.detail, 'custom-event')
}

const handleStorageEvent = (e) => {
  if (!e) return
  if (e.key !== RESOURCE_REPO_SYNC_STATUS_KEY) return
  refreshSyncStateFromStorage('storage-event')
}

const setSyncState = (patch) => {
  const next = { ...syncState.value, ...patch }
  syncState.value = next
  writeSyncState(next)

  try {
    syncStateRaw.value = JSON.stringify(next)
  } catch {}

  try {
    window.dispatchEvent(
      new CustomEvent('guaxi:resource-repo-sync-state', {
        detail: next,
      })
    )
  } catch (e) {
    logger.warn('[ResourceRepoManager] dispatch sync state event failed', e)
  }

  logger.info('[ResourceRepoManager] sync state', next)
}

const syncUi = computed(() => {
  const s = String(syncState.value?.status || 'idle')
  if (s === 'running') {
    return { tagType: 'info', tagText: '进行中', title: '资源库同步进行中…' }
  }
  if (s === 'success') {
    return { tagType: 'success', tagText: '成功', title: '资源库同步状态正常' }
  }
  if (s === 'error') {
    return { tagType: 'error', tagText: '失败', title: '资源库同步失败' }
  }
  if (s === 'skipped') {
    return { tagType: 'warning', tagText: '跳过', title: '资源库同步已跳过' }
  }
  return { tagType: 'default', tagText: '未运行', title: '资源库同步未运行' }
})

const syncProgressPercent = computed(() => {
  const total = Number(syncProgress.value?.total || 0)
  const current = Number(syncProgress.value?.current || 0)
  if (total > 0) return Math.max(0, Math.min(100, Math.round((current / total) * 100)))
  const bytesTotal = Number(syncProgress.value?.bytes_total || 0)
  const bytesCurrent = Number(syncProgress.value?.bytes_current || 0)
  if (bytesTotal > 0) return Math.max(0, Math.min(100, Math.round((bytesCurrent / bytesTotal) * 100)))
  return syncProgress.value?.status === 'success' ? 100 : 0
})

const syncProgressBarStatus = computed(() => {
  const s = String(syncProgress.value?.status || 'idle')
  if (s === 'error') return 'error'
  if (s === 'success') return 'success'
  if (s === 'waiting') return 'warning'
  return 'default'
})

const syncProgressTagType = computed(() => {
  const s = String(syncProgress.value?.status || 'idle')
  if (s === 'success') return 'success'
  if (s === 'error') return 'error'
  if (s === 'waiting') return 'warning'
  if (s === 'running') return 'info'
  return 'default'
})

const syncProgressStatusText = computed(() => {
  const s = String(syncProgress.value?.status || 'idle')
  if (s === 'success') return '完成'
  if (s === 'error') return '失败'
  if (s === 'waiting') return '等待'
  if (s === 'running') return '进行中'
  return '未开始'
})

const formatBytes = (value) => {
  const n = Number(value || 0)
  if (!Number.isFinite(n) || n <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = n
  let idx = 0
  while (size >= 1024 && idx < units.length - 1) {
    size /= 1024
    idx += 1
  }
  return `${size.toFixed(idx === 0 ? 0 : 1)} ${units[idx]}`
}

const applySyncProgress = (payload, source) => {
  if (!payload) return
  const next = {
    run_id: String(payload.run_id || ''),
    provider: String(payload.provider || ''),
    target: String(payload.target || ''),
    phase: String(payload.phase || ''),
    status: String(payload.status || 'idle'),
    message: String(payload.message || ''),
    current: Number(payload.current || 0),
    total: Number(payload.total || 0),
    bytes_current: Number(payload.bytes_current || 0),
    bytes_total: Number(payload.bytes_total || 0),
    file_path: String(payload.file_path || ''),
    at: String(payload.at || ''),
  }
  syncProgress.value = next
  logger.info('[ResourceRepoManager] sync progress update', { source, progress: next })
}

const loading = ref(false)
const loadingTree = ref(false)

const resourceRoot = ref('')
const treeData = ref([])

const expandedKeys = ref([])
const selectedKeys = ref([])
const selectedPath = ref('')
const searchKeyword = ref('')
const viewMode = ref('tree')
const currentDir = ref('')

const createModalVisible = ref(false)
const createMode = ref('folder')
const createName = ref('')
const renameModalVisible = ref(false)
const renameName = ref('')
const deleteModalVisible = ref(false)

const normalizePath = (path) => String(path || '').replace(/\\/g, '/').replace(/\/+/g, '/')

const trimPathSegment = (name) => String(name || '').trim()

const isValidPathSegment = (name) => {
  const value = trimPathSegment(name)
  return !!value && !/[\\/]/.test(value) && value !== '.' && value !== '..'
}

const pathJoin = (dir, name) => {
  const base = normalizePath(dir).replace(/\/+$/g, '')
  const child = String(name || '').replace(/^\/+/g, '')
  return base ? `${base}/${child}` : child
}

const dirname = (path) => {
  const normalized = normalizePath(path)
  const idx = normalized.lastIndexOf('/')
  return idx > 0 ? normalized.slice(0, idx) : ''
}

const basename = (path) => {
  const normalized = normalizePath(path).replace(/\/+$/g, '')
  const idx = normalized.lastIndexOf('/')
  return idx >= 0 ? normalized.slice(idx + 1) : normalized
}

const relativePath = (path) => {
  const root = normalizePath(resourceRoot.value).replace(/\/+$/g, '')
  const normalized = normalizePath(path).replace(/\/+$/g, '')
  if (!normalized) return ''
  if (root && normalized === root) return ''
  if (root && normalized.toLowerCase().startsWith(`${root.toLowerCase()}/`)) {
    return normalized.slice(root.length + 1)
  }
  return normalized
}

const fileExtensionLabel = (name) => {
  const value = String(name || '')
  const idx = value.lastIndexOf('.')
  if (idx <= 0 || idx === value.length - 1) return '文件'
  return value.slice(idx + 1).toUpperCase()
}

const toTreeNode = (n) => {
  const isDir = Boolean(n?.is_directory)
  const key = normalizePath(n?.id || '')
  return {
    key,
    label: String(n?.name || ''),
    isDir,
    loaded: !isDir,
    children: isDir ? [] : undefined,
  }
}

const loadRoot = async () => {
  const root = await invoke('get_image_bed_root')
  resourceRoot.value = typeof root === 'string' ? normalizePath(root) : ''
}

const sortTreeNodes = (nodes) => {
  return [...(nodes || [])].sort((a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
    return String(a.label || '').localeCompare(String(b.label || ''), 'zh-Hans-CN')
  })
}

const loadChildren = async (dirPath) => {
  const nodes = await invoke('get_file_tree', { path: dirPath })
  if (!Array.isArray(nodes)) return []
  const mapped = sortTreeNodes(nodes.map(toTreeNode))
  logger.info('[ResourceRepoManager] loaded children', { dirPath, count: mapped.length })
  return mapped
}

const refresh = async () => {
  try {
    loadingTree.value = true
    await loadRoot()
    if (!resourceRoot.value) {
      treeData.value = []
      selectedKeys.value = []
      selectedPath.value = ''
      logger.warn('[ResourceRepoManager] refresh skipped: resource root is empty')
      return
    }
    const children = await loadChildren(resourceRoot.value)
    treeData.value = children
    currentDir.value = resourceRoot.value
    expandedKeys.value = []
    selectedKeys.value = []
    selectedPath.value = ''
    logger.info('[ResourceRepoManager] refreshed', {
      root: resourceRoot.value,
      count: treeData.value.length,
    })
  } catch (e) {
    logger.error('[ResourceRepoManager] refresh failed', e)
    message.error('刷新失败')
  } finally {
    loadingTree.value = false
  }
}

const findNodeByKey = (nodes, key) => {
  const normalizedKey = normalizePath(key)
  for (const n of nodes || []) {
    if (normalizePath(n?.key) === normalizedKey) return n
    const hit = findNodeByKey(n?.children, normalizedKey)
    if (hit) return hit
  }
  return null
}

const renderPrefix = ({ option }) => {
  const icon = option?.isDir ? Folder20Regular : Document20Regular
  return h(NIcon, { component: icon, size: 16 })
}

const renderLabel = ({ option }) => {
  return h(
    'span',
    {
      class: 'tree-label',
      title: option?.label || '',
      onClick: (event) => {
        event?.stopPropagation?.()
        handleTreeNodeClick(option)
      },
    },
    option?.label || ''
  )
}

// 设计说明：
// treeData 是资源浏览的唯一前端数据源，树/列表/网格/统计都从它派生。
// 这样可以避免多份状态互相漂移；后端仍然按需读取目录，保证大资源库不会一次性阻塞窗口。
const flattenNodes = (nodes, depth = 0, bucket = []) => {
  for (const node of nodes || []) {
    bucket.push({ ...node, depth })
    if (Array.isArray(node.children) && node.children.length > 0) {
      flattenNodes(node.children, depth + 1, bucket)
    }
  }
  return bucket
}

const flatItems = computed(() => flattenNodes(treeData.value))

const selectedNode = computed(() => {
  if (!selectedPath.value) return null
  return findNodeByKey(treeData.value, selectedPath.value)
})

const currentDirNode = computed(() => {
  if (!currentDir.value || normalizePath(currentDir.value) === normalizePath(resourceRoot.value)) return null
  return findNodeByKey(treeData.value, currentDir.value)
})

const createTargetDir = computed(() => {
  if (!resourceRoot.value) return ''
  const node = selectedNode.value
  if (!node) return resourceRoot.value
  return node.isDir ? node.key : dirname(node.key) || resourceRoot.value
})

const normalizedSearchKeyword = computed(() => searchKeyword.value.trim().toLowerCase())

const nodeMatchesKeyword = (node, keyword) => {
  if (!keyword) return true
  const haystack = `${node?.label || ''} ${relativePath(node?.key || '')}`.toLowerCase()
  return haystack.includes(keyword)
}

const filterTreeNodes = (nodes, keyword) => {
  if (!keyword) return nodes
  const filtered = []
  for (const node of nodes || []) {
    const children = Array.isArray(node.children) ? filterTreeNodes(node.children, keyword) : undefined
    if (nodeMatchesKeyword(node, keyword) || (children && children.length > 0)) {
      filtered.push({ ...node, children })
    }
  }
  return filtered
}

const filteredTreeData = computed(() => filterTreeNodes(treeData.value, normalizedSearchKeyword.value))

const flatFilteredItems = computed(() => {
  const keyword = normalizedSearchKeyword.value
  if (!keyword) return flatItems.value
  return flatItems.value.filter((item) => nodeMatchesKeyword(item, keyword))
})

const currentDirChildren = computed(() => {
  if (!resourceRoot.value || normalizePath(currentDir.value) === normalizePath(resourceRoot.value)) {
    return treeData.value
  }
  const node = currentDirNode.value
  return Array.isArray(node?.children) ? node.children : []
})

const visibleItems = computed(() => {
  const keyword = normalizedSearchKeyword.value
  if (keyword) return flatFilteredItems.value
  return currentDirChildren.value.map((item) => ({ ...item, depth: 0 }))
})

const currentDirLabel = computed(() => {
  if (!resourceRoot.value || normalizePath(currentDir.value) === normalizePath(resourceRoot.value)) return '根目录'
  return relativePath(currentDir.value) || basename(currentDir.value) || '根目录'
})

const canGoUp = computed(() => {
  return !!resourceRoot.value && normalizePath(currentDir.value) !== normalizePath(resourceRoot.value)
})

const browserEmptyDescription = computed(() => {
  if (viewMode.value === 'tree') return normalizedSearchKeyword.value ? '没有匹配的资源' : '资源库为空'
  return normalizedSearchKeyword.value ? '没有匹配的资源' : '当前目录为空'
})

const browserIsEmpty = computed(() => {
  if (viewMode.value === 'tree') return filteredTreeData.value.length === 0
  return visibleItems.value.length === 0
})

const displayedItemCount = computed(() => {
  if (viewMode.value === 'tree') return filteredTreeData.value.length
  return visibleItems.value.length
})

const repoStats = computed(() => {
  const total = flatItems.value.length
  const dirs = flatItems.value.filter((item) => item.isDir).length
  return {
    total,
    dirs,
    files: total - dirs,
  }
})

const handleSelectedKeysUpdate = (keys) => {
  selectedKeys.value = keys
  selectedPath.value = keys?.[0] ? normalizePath(keys[0]) : ''
  logger.info('[ResourceRepoManager] selected path changed', { selectedPath: selectedPath.value })

  const node = selectedNode.value
  if (node?.isDir) {
    currentDir.value = node.key
    toggleDirectoryExpansion(node).catch((e) => {
      logger.error('[ResourceRepoManager] toggle directory from tree click failed', e)
      message.error('展开目录失败')
    })
  }
}

const handleTreeNodeClick = (option) => {
  const key = normalizePath(option?.key || '')
  if (!key) return

  const node = findNodeByKey(treeData.value, key) || option
  selectedKeys.value = [key]
  selectedPath.value = key

  if (node?.isDir) {
    currentDir.value = key
    toggleDirectoryExpansion(node).catch((e) => {
      logger.error('[ResourceRepoManager] toggle directory from label click failed', e)
      message.error('展开目录失败')
    })
  }

  logger.info('[ResourceRepoManager] tree label clicked', { path: key, isDir: Boolean(node?.isDir) })
}

const handleExpandedKeysUpdate = async (keys) => {
  expandedKeys.value = keys

  const newest = keys?.[keys.length - 1]
  if (!newest) return

  const node = findNodeByKey(treeData.value, String(newest))
  if (!node || !node.isDir) return
  if (node.loaded) return

  try {
    loadingTree.value = true
    logger.info('[ResourceRepoManager] expanding directory', { path: node.key })
    const children = await loadChildren(node.key)
    node.children = children
    node.loaded = true
  } catch (e) {
    logger.error('[ResourceRepoManager] load children failed', e)
    message.error('加载目录失败')
  } finally {
    loadingTree.value = false
  }
}

const ensureDirectoryLoaded = async (node) => {
  if (!node?.isDir || node.loaded) return
  logger.info('[ResourceRepoManager] ensure directory loaded', { path: node.key })
  const children = await loadChildren(node.key)
  node.children = children
  node.loaded = true
}

const getSourceNode = (item) => {
  const key = normalizePath(item?.key || '')
  if (!key) return null
  return findNodeByKey(treeData.value, key) || item
}

const toggleDirectoryExpansion = async (node) => {
  const sourceNode = getSourceNode(node)
  if (!sourceNode?.isDir) return
  const normalizedKey = normalizePath(sourceNode.key)
  const expanded = new Set(expandedKeys.value.map((key) => normalizePath(key)))
  if (expanded.has(normalizedKey)) {
    expanded.delete(normalizedKey)
    expandedKeys.value = [...expanded]
    logger.info('[ResourceRepoManager] collapse directory from row click', { path: sourceNode.key })
    return
  }

  try {
    loadingTree.value = true
    await ensureDirectoryLoaded(sourceNode)
    expanded.add(normalizedKey)
    expandedKeys.value = [...expanded]
    logger.info('[ResourceRepoManager] expand directory from row click', { path: sourceNode.key })
  } finally {
    loadingTree.value = false
  }
}

const setViewMode = async (mode) => {
  viewMode.value = mode
  if (mode !== 'tree') {
    await ensureCurrentDirLoaded('switch-view')
  }
  logger.info('[ResourceRepoManager] view mode changed', { mode })
}

const ensureCurrentDirLoaded = async (source) => {
  if (!currentDir.value || normalizePath(currentDir.value) === normalizePath(resourceRoot.value)) return
  const node = currentDirNode.value
  if (!node?.isDir || node.loaded) return
  try {
    loadingTree.value = true
    await ensureDirectoryLoaded(node)
    logger.info('[ResourceRepoManager] current directory loaded', { source, path: node.key })
  } catch (e) {
    logger.error('[ResourceRepoManager] load current directory failed', { source, path: currentDir.value }, e)
    message.error('加载当前目录失败')
  } finally {
    loadingTree.value = false
  }
}

const selectItem = (item) => {
  selectedKeys.value = item?.key ? [item.key] : []
  selectedPath.value = item?.key || ''
  logger.info('[ResourceRepoManager] item selected', { path: selectedPath.value, viewMode: viewMode.value })
}

const enterDirectory = async (item, source) => {
  const node = getSourceNode(item)
  if (!node?.isDir) return
  try {
    loadingTree.value = true
    await ensureDirectoryLoaded(node)
    currentDir.value = node.key
    selectedKeys.value = [node.key]
    selectedPath.value = node.key

    const expanded = new Set(expandedKeys.value.map((key) => normalizePath(key)))
    expanded.add(normalizePath(node.key))
    expandedKeys.value = [...expanded]

    logger.info('[ResourceRepoManager] enter directory', {
      path: node.key,
      source,
      viewMode: viewMode.value,
    })
  } catch (e) {
    logger.error('[ResourceRepoManager] enter directory failed', e)
    message.error('进入目录失败')
  } finally {
    loadingTree.value = false
  }
}

const handleItemClick = async (item) => {
  if (!item) return
  selectItem(item)
  if (item.isDir && viewMode.value !== 'tree') {
    await enterDirectory(item, 'single-click')
  }
}

const handleItemOpen = async (item) => {
  if (!item) return
  if (item.isDir) {
    await enterDirectory(item, 'double-click')
    return
  }
  await openPathInExplorer(item.key)
}

const goUpDirectory = () => {
  if (!canGoUp.value) return
  const parent = dirname(currentDir.value)
  currentDir.value = parent && normalizePath(parent).toLowerCase().startsWith(normalizePath(resourceRoot.value).toLowerCase())
    ? parent
    : resourceRoot.value
  selectedKeys.value = currentDir.value ? [currentDir.value] : []
  selectedPath.value = currentDir.value
  logger.info('[ResourceRepoManager] go up directory', { currentDir: currentDir.value })
}

const openPathInExplorer = async (path) => {
  try {
    if (!path) return
    logger.info('[ResourceRepoManager] open in explorer', { path })
    await invoke('open_in_file_explorer', { path })
  } catch (e) {
    logger.error('[ResourceRepoManager] open explorer failed', e)
    message.error('打开失败')
  }
}

const openInExplorer = async () => {
  await openPathInExplorer(resourceRoot.value)
}

const openSelectedInExplorer = async () => {
  await openPathInExplorer(selectedPath.value)
}

const makeUniqueName = async (dirPath, desired) => {
  const want = trimPathSegment(desired)
  if (!want) return want
  try {
    const raw = await invoke('get_file_tree', { path: dirPath })
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
    logger.warn('[ResourceRepoManager] make unique name failed, fallback to desired name', e)
    return want
  }
}

const openCreateModal = (mode) => {
  createMode.value = mode === 'file' ? 'file' : 'folder'
  createName.value = createMode.value === 'file' ? 'new-file.md' : '新建文件夹'
  createModalVisible.value = true
  logger.info('[ResourceRepoManager] open create modal', {
    mode: createMode.value,
    targetDir: createTargetDir.value,
  })
}

const confirmCreate = async () => {
  const dir = createTargetDir.value
  const name = trimPathSegment(createName.value)
  if (!dir || !isValidPathSegment(name)) {
    message.warning('请输入有效名称，名称不能包含路径分隔符')
    return
  }

  try {
    loading.value = true
    const finalName = await makeUniqueName(dir, name)
    const target = pathJoin(dir, finalName)
    if (createMode.value === 'folder') {
      logger.info('[ResourceRepoManager] create directory', { target })
      await invoke('create_directory', { path: target })
    } else {
      logger.info('[ResourceRepoManager] create file', { target })
      await invoke('create_file_with_content', { path: target, content: '' })
    }
    createModalVisible.value = false
    message.success('创建成功')
    await refresh()
  } catch (e) {
    logger.error('[ResourceRepoManager] create failed', e)
    message.error(formatInvokeError(e))
  } finally {
    loading.value = false
  }
}

const openRenameModal = () => {
  if (!selectedNode.value) {
    message.warning('请先选择要重命名的资源')
    return
  }
  renameName.value = selectedNode.value.label
  renameModalVisible.value = true
  logger.info('[ResourceRepoManager] open rename modal', { path: selectedPath.value })
}

const confirmRename = async () => {
  const sourcePath = selectedPath.value
  const name = trimPathSegment(renameName.value)
  if (!sourcePath || !selectedNode.value) return
  if (!isValidPathSegment(name)) {
    message.warning('请输入有效名称，名称不能包含路径分隔符')
    return
  }
  if (name === selectedNode.value.label) {
    renameModalVisible.value = false
    return
  }

  try {
    loading.value = true
    const destinationPath = pathJoin(dirname(sourcePath), name)
    logger.info('[ResourceRepoManager] rename path', { sourcePath, destinationPath })
    await invoke('move_path', { sourcePath, destinationPath })
    renameModalVisible.value = false
    message.success('重命名成功')
    await refresh()
  } catch (e) {
    logger.error('[ResourceRepoManager] rename failed', e)
    message.error(formatInvokeError(e))
  } finally {
    loading.value = false
  }
}

const requestDeleteSelected = () => {
  if (!selectedPath.value) {
    message.warning('请先选择要删除的资源')
    return
  }
  deleteModalVisible.value = true
  logger.info('[ResourceRepoManager] request delete confirmation', { path: selectedPath.value })
}

const confirmDeleteSelected = async () => {
  try {
    if (!selectedPath.value) return
    loading.value = true
    logger.info('[ResourceRepoManager] delete path', { path: selectedPath.value })
    await invoke('delete_path', { path: selectedPath.value })
    deleteModalVisible.value = false
    message.success('已删除')
    await refresh()
  } catch (e) {
    logger.error('[ResourceRepoManager] delete failed', e)
    message.error(formatInvokeError(e))
  } finally {
    loading.value = false
  }
}

const testConnection = async () => {
  const action = '测试连接'
  try {
    loading.value = true
    logger.info('[ResourceRepoManager] test connection start')
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    applySyncProgress({
      provider: 'unknown',
      phase: 'connect',
      status: 'running',
      message: '正在测试同步连接',
      at: nowLabel(),
    }, 'local-test')

    const res = await invoke('resource_repo_test_connection')
    const text = typeof res === 'string' ? res : '连接正常'
    applySyncProgress({
      provider: text.includes('P2P') ? 'p2p' : 'other',
      phase: 'connect',
      status: 'success',
      message: text,
      current: 1,
      total: 1,
      at: nowLabel(),
    }, 'local-test')
    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
    logger.info('[ResourceRepoManager] test connection success', { text })
    message.success(text)
  } catch (e) {
    const msg = formatInvokeError(e)
    logger.error('[ResourceRepoManager] test connection failed', e)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
    message.error(msg)
  } finally {
    loading.value = false
  }
}

const syncNow = async () => {
  const action = '立即同步'
  try {
    loading.value = true
    logger.info('[ResourceRepoManager] sync start')
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    applySyncProgress({
      provider: 'unknown',
      phase: 'start',
      status: 'running',
      message: '正在启动资源库同步',
      at: nowLabel(),
    }, 'local-sync')

    const res = await invoke('resource_repo_sync_now')
    const text = typeof res === 'string' ? res : '同步完成'
    if (syncProgress.value.provider !== 'p2p') {
      applySyncProgress({
        provider: 'other',
        phase: 'complete',
        status: 'success',
        message: text,
        current: 1,
        total: 1,
        at: nowLabel(),
      }, 'local-sync')
    }
    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
    logger.info('[ResourceRepoManager] sync success', { text })
    message.success(text)
    await refresh()
  } catch (e) {
    const msg = formatInvokeError(e)
    logger.error('[ResourceRepoManager] sync failed', e)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
    message.error(msg)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  window.addEventListener('guaxi:resource-repo-sync-state', handleSyncStateEvent)
  window.addEventListener('storage', handleStorageEvent)

  refreshSyncStateFromStorage('mounted')

  try {
    syncProgressUnlisten = await listen('resource_repo_sync_progress', (event) => {
      applySyncProgress(event?.payload, 'tauri-event')
    })
    logger.info('[ResourceRepoManager] resource_repo_sync_progress listener registered')
  } catch (e) {
    logger.warn('[ResourceRepoManager] register sync progress listener failed', e)
  }

  try {
    syncPollTimer = setInterval(() => {
      refreshSyncStateFromStorage('poll')
    }, 800)
  } catch (e) {
    logger.warn('[ResourceRepoManager] start sync state poll failed', e)
  }

  await refresh()
})

onUnmounted(() => {
  window.removeEventListener('guaxi:resource-repo-sync-state', handleSyncStateEvent)
  window.removeEventListener('storage', handleStorageEvent)

  try {
    if (syncProgressUnlisten) {
      syncProgressUnlisten()
      syncProgressUnlisten = null
    }
  } catch (e) {
    logger.warn('[ResourceRepoManager] stop sync progress listener failed', e)
  }

  try {
    if (syncPollTimer) {
      clearInterval(syncPollTimer)
      syncPollTimer = null
    }
  } catch (e) {
    logger.warn('[ResourceRepoManager] stop sync state poll failed', e)
  }
})
</script>

<style scoped>
.page {
  height: 100vh;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--guaxi-app-bg, var(--n-color, #20242b));
  color: var(--guaxi-text-primary, var(--n-text-color, #dfe4ea));
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  font-size: 15px;
}

.header {
  height: 52px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 12px 0 16px;
  border-bottom: 1px solid var(--guaxi-border, var(--n-divider-color, rgba(255,255,255,0.12)));
  background: var(--guaxi-header-bg, rgba(255,255,255,0.025));
}

.header-left,
.header-actions,
.header-tool-group,
.sync-head,
.side-title {
  display: flex;
  align-items: center;
}

.header-left {
  flex: 1 1 auto;
  min-width: 0;
  gap: 10px;
}

.header-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.header-tool-group {
  gap: 8px;
  padding-left: 10px;
  border-left: 1px solid rgba(255,255,255,0.1);
}

.header-tool-group:first-child {
  padding-left: 0;
  border-left: 0;
}

.title {
  flex: 0 0 auto;
  font-size: 19px;
  font-weight: 700;
}

.main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 370px;
  gap: 0;
}

.workspace {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--guaxi-border, var(--n-divider-color, rgba(255,255,255,0.12)));
}

.workspace-toolbar {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  padding: 10px 14px;
  border-bottom: 1px solid var(--guaxi-border, var(--n-divider-color, rgba(255,255,255,0.12)));
  background: rgba(255,255,255,0.018);
}

.toolbar-top {
  display: flex;
  align-items: center;
  min-width: 0;
}

.toolbar-top {
  gap: 0;
}

.search {
  flex: 1 1 auto;
  max-width: 720px;
  min-width: 220px;
}

.view-switcher {
  flex: 0 0 auto;
}

.browser {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 14px 16px 18px;
  background: rgba(0,0,0,0.035);
}

.browser-head {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  margin-bottom: 12px;
  padding: 0 2px;
}

.current-dir {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--guaxi-text-primary, #dfe4ea);
  font-size: 16px;
  font-weight: 700;
}

.current-dir span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.browser-count {
  flex: 0 0 auto;
  margin-left: auto;
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.64));
  font-size: 14px;
}

.resource-tree {
  min-width: 420px;
  font-size: 15px;
}

.tree-label {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  font-size: 15px;
  cursor: pointer;
}

.list-view {
  display: grid;
  gap: 5px;
  min-width: 560px;
}

.list-row {
  width: 100%;
  min-height: 48px;
  display: grid;
  grid-template-columns: 26px minmax(160px, 1fr) 76px minmax(180px, 0.9fr);
  align-items: center;
  gap: 10px;
  border: 1px solid rgba(255,255,255,0.055);
  border-radius: 8px;
  padding: 0 12px;
  color: inherit;
  background: rgba(255,255,255,0.022);
  text-align: left;
  cursor: pointer;
}

.list-row:hover,
.grid-cell:hover {
  background: var(--guaxi-hover-bg, rgba(255,255,255,0.07));
}

.list-row.selected,
.grid-cell.selected {
  background: var(--guaxi-active-bg, rgba(74, 144, 226, 0.22));
  outline: 1px solid var(--guaxi-primary, rgba(74, 144, 226, 0.55));
}

.row-icon,
.grid-icon {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.72));
}

.row-name,
.row-path,
.grid-name,
.grid-meta,
.detail-value {
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-name,
.grid-name,
.grid-meta {
  white-space: nowrap;
}

.row-kind {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.68));
  font-size: 14px;
}

.row-path {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.58));
  font-size: 14px;
  white-space: nowrap;
}

.grid-view {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(164px, 1fr));
  gap: 12px;
}

.grid-cell {
  height: 138px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  border: 1px solid var(--guaxi-border, var(--n-divider-color, rgba(255,255,255,0.12)));
  border-radius: 8px;
  padding: 12px;
  color: inherit;
  background: rgba(255,255,255,0.025);
  cursor: pointer;
}

.grid-icon {
  font-size: 34px;
}

.grid-name {
  width: 100%;
  text-align: center;
  font-size: 15px;
  font-weight: 700;
}

.grid-meta {
  max-width: 100%;
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.62));
  font-size: 14px;
}

.side {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: auto;
  background: var(--guaxi-panel-bg, rgba(255,255,255,0.018));
}

.side-section {
  padding: 16px 18px;
  border-bottom: 1px solid var(--guaxi-border, var(--n-divider-color, rgba(255,255,255,0.12)));
}

.side-title {
  gap: 8px;
  margin-bottom: 14px;
  font-size: 17px;
  font-weight: 700;
}

.detail-line {
  display: grid;
  grid-template-columns: 86px minmax(0, 1fr);
  gap: 12px;
  min-height: 36px;
  align-items: center;
  font-size: 15px;
}

.detail-label {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.62));
}

.detail-value {
  white-space: nowrap;
  font-weight: 650;
}

.sync-state-summary {
  display: grid;
  gap: 5px;
  margin-bottom: 14px;
}

.sync-state-title {
  color: var(--guaxi-text-primary, #dfe4ea);
  font-size: 15px;
  font-weight: 750;
}

.sync-state-detail {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.72));
  font-size: 14px;
  line-height: 1.45;
  word-break: break-word;
}

.side-stats {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.side-stat {
  min-height: 58px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
  padding: 10px 12px;
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 8px;
  background: rgba(255,255,255,0.028);
}

.side-stat.wide {
  grid-column: 1 / -1;
}

.side-stat-value {
  color: var(--guaxi-text-primary, #dfe4ea);
  font-size: 20px;
  font-weight: 800;
  line-height: 1;
}

.side-stat-label {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.68));
  font-size: 14px;
}

.sync-head {
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.72));
  font-size: 15px;
}

.sync-progress-meta {
  display: grid;
  gap: 5px;
  margin-top: 10px;
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.72));
  font-size: 15px;
  line-height: 1.45;
  word-break: break-all;
}

.resource-modal {
  width: min(460px, calc(100vw - 40px));
}

.modal-body {
  display: grid;
  gap: 12px;
}

.modal-hint {
  color: var(--guaxi-text-secondary, rgba(223,228,234,0.68));
  font-size: 12px;
  word-break: break-all;
}

.danger-title {
  font-weight: 700;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

:deep(.n-spin-container),
:deep(.n-spin-content) {
  min-height: 100%;
}

@media (max-width: 860px) {
  .main {
    grid-template-columns: 1fr;
  }

  .side {
    display: none;
  }

  .workspace-toolbar {
    gap: 10px;
  }

  .toolbar-top {
    align-items: stretch;
    flex-direction: column;
  }

  .search {
    max-width: none;
  }

}
</style>

<style>
.resource-repo-manager {
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  font-size: 15px;
}

.resource-repo-manager .header {
  height: 52px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 12px 0 16px;
  box-sizing: border-box;
}

.resource-repo-manager .header-left,
.resource-repo-manager .header-actions,
.resource-repo-manager .header-tool-group,
.resource-repo-manager .toolbar-top,
.resource-repo-manager .sync-head,
.resource-repo-manager .side-title {
  display: flex;
  align-items: center;
}

.resource-repo-manager .header-left {
  flex: 1 1 auto;
  min-width: 0;
  gap: 10px;
}

.resource-repo-manager .header-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.resource-repo-manager .header-tool-group {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-left: 10px;
  border-left: 1px solid rgba(255,255,255,0.1);
}

.resource-repo-manager .header-tool-group:first-child {
  padding-left: 0;
  border-left: 0;
}

.resource-repo-manager .main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 370px;
}

.resource-repo-manager .workspace {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.resource-repo-manager .workspace-toolbar {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  padding: 10px 14px;
  box-sizing: border-box;
}

.resource-repo-manager .toolbar-top {
  min-width: 0;
}

.resource-repo-manager .search {
  flex: 1 1 auto;
  max-width: 720px;
  min-width: 220px;
}

.resource-repo-manager .view-switcher {
  flex: 0 0 auto;
}

.resource-repo-manager .row-name,
.resource-repo-manager .row-path,
.resource-repo-manager .grid-name,
.resource-repo-manager .grid-meta,
.resource-repo-manager .detail-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.resource-repo-manager .browser {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 14px 16px 18px;
  box-sizing: border-box;
}

.resource-repo-manager .tree-label {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  font-size: 15px;
  cursor: pointer;
}

.resource-repo-manager .browser-head {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  margin-bottom: 12px;
}

.resource-repo-manager .current-dir {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  font-weight: 700;
}

.resource-repo-manager .browser-count {
  flex: 0 0 auto;
  margin-left: auto;
  font-size: 14px;
}

.resource-repo-manager .list-view {
  display: grid;
  gap: 5px;
  min-width: 560px;
}

.resource-repo-manager .list-row {
  width: 100%;
  min-height: 48px;
  display: grid;
  grid-template-columns: 26px minmax(160px, 1fr) 76px minmax(180px, 0.9fr);
  align-items: center;
  gap: 10px;
  border-radius: 8px;
  padding: 0 12px;
  box-sizing: border-box;
  text-align: left;
}

.resource-repo-manager .grid-view {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(164px, 1fr));
  gap: 12px;
}

.resource-repo-manager .grid-cell {
  height: 138px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  border-radius: 8px;
  padding: 12px;
  box-sizing: border-box;
  text-align: center;
}

.resource-repo-manager .side {
  min-width: 0;
  min-height: 0;
  overflow: auto;
}

.resource-repo-manager .side-section {
  padding: 16px 18px;
}

.resource-repo-manager .side-title {
  font-size: 17px;
}

.resource-repo-manager .detail-line {
  min-height: 36px;
  grid-template-columns: 86px minmax(0, 1fr);
  font-size: 15px;
}

.resource-repo-manager .side-stats {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.resource-repo-manager .side-stat {
  min-height: 58px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
  padding: 10px 12px;
  box-sizing: border-box;
  border-radius: 8px;
}

.resource-repo-manager .side-stat.wide {
  grid-column: 1 / -1;
}

.resource-repo-manager .side-stat-value {
  font-size: 20px;
  font-weight: 800;
  line-height: 1;
}

.resource-repo-manager .side-stat-label,
.resource-repo-manager .sync-state-title,
.resource-repo-manager .sync-state-detail,
.resource-repo-manager .sync-head,
.resource-repo-manager .sync-progress-meta {
  font-size: 15px;
}

.resource-repo-manager .sync-state-summary {
  display: grid;
  gap: 5px;
  margin-bottom: 14px;
}

.resource-repo-manager .sync-state-title {
  font-weight: 750;
}

.resource-repo-manager .sync-state-detail {
  line-height: 1.45;
  word-break: break-word;
}

@media (max-width: 860px) {
  .resource-repo-manager .main {
    grid-template-columns: minmax(0, 1fr);
  }

  .resource-repo-manager .side {
    display: none;
  }

  .resource-repo-manager .toolbar-top {
    align-items: stretch;
    flex-direction: column;
  }

  .resource-repo-manager .search {
    max-width: none;
  }
}
</style>
