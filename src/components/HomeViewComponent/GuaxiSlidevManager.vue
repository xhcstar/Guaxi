<template>
  <div class="slidev-page">
    <div class="sider-toggle sider-toggle-left" :class="{ collapsed: leftCollapsed }" :style="{ left: (leftCollapsed ? 0 : leftSiderWidth) + 'px' }">
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

    <div class="sider-toggle sider-toggle-right" :class="{ collapsed: rightCollapsed }" :style="{ right: (rightCollapsed ? 0 : rightSiderWidth) + 'px' }">
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
      <n-layout-sider
        v-model:collapsed="leftCollapsed"
        :class="['slidev-sider', 'slidev-sider-left', { 'is-collapsed': leftCollapsed }]"
        :width="leftSiderWidth"
        bordered
        collapse-mode="width"
        :collapsed-width="0"
        :native-scrollbar="false"
        content-style="padding: 0; height: 100%; display: flex; flex-direction: column; overflow: hidden;"
        style="height: 100%; position: relative;"
      >
      <div class="panel">
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

              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-button size="tiny" quaternary circle @click="toggleLeftCollapsed">
                    <template #icon>
                      <n-icon :component="leftCollapsed ? ChevronForwardOutline : ChevronBackOutline" />
                    </template>
                  </n-button>
                </template>
                {{ leftCollapsed ? '展开左侧' : '折叠左侧' }}
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
              <GuaxiSlidevTreeView
                :nodes="filteredSlidevTree"
                v-model:selected-key="slidevSelectedKey"
                v-model:expanded-keys="slidevExpandedKeys"
                :load-children="handleSlidevNodeLoad"
                empty-text="暂无 PPT 文件"
                @select="handleSlidevSelectNode"
              />
            </div>
          </n-spin>
        </div>

        <div class="panel-footer">
          <n-text depth="3" class="path-text" :title="slidevRoot">{{ slidevRoot || 'Slidev 根目录未加载' }}</n-text>
        </div>
      </div>
      <div class="sider-resize-handle" @mousedown="onMouseDownLeftSider" />
    </n-layout-sider>

    <n-layout-content
      class="slidev-center"
      :native-scrollbar="false"
      content-style="height: 100%; min-height: 0; display: flex; flex-direction: column; overflow: hidden;"
    >
      <div class="editor-toolbar">
        <n-space size="small" align="center">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" quaternary circle @click="saveActive">
                <template #icon>
                  <n-icon :component="SaveOutline" />
                </template>
              </n-button>
            </template>
            保存
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" quaternary circle @click="previewSlidev">
                <template #icon>
                  <n-icon :component="EyeOutline" />
                </template>
              </n-button>
            </template>
            预览
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" quaternary circle @click="presentSlidev">
                <template #icon>
                  <n-icon :component="PlayCircleOutline" />
                </template>
              </n-button>
            </template>
            演示
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" quaternary circle @click="exportPptx">
                <template #icon>
                  <n-icon :component="DownloadOutline" />
                </template>
              </n-button>
            </template>
            导出 PPTX
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="small" quaternary circle @click="stopPreview">
                <template #icon>
                  <n-icon :component="StopCircleOutline" />
                </template>
              </n-button>
            </template>
            停止预览
          </n-tooltip>
        </n-space>

        <div class="toolbar-right">
          <n-input v-model:value="devPortText" size="small" placeholder="端口(默认3030)" style="width: 140px" />
        </div>
      </div>

      <div class="editor-host">
        <GuaxiSlidevLiteEditor
          ref="pptEditorRef"
          :active-path="activeEditorPath"
          :content="activeEditorContent"
          :unsupported="activeEditorUnsupported"
          @draft-flush="onEditorDraftFlush"
        />
      </div>
    </n-layout-content>

    <n-layout-sider
      v-model:collapsed="rightCollapsed"
      :class="['slidev-sider', 'slidev-sider-right', { 'is-collapsed': rightCollapsed }]"
      :width="rightSiderWidth"
      bordered
      position="right"
      collapse-mode="width"
      :collapsed-width="0"
      :native-scrollbar="false"
      content-style="padding: 0; height: 100%; display: flex; flex-direction: column; overflow: hidden;"
      style="height: 100%; position: relative;"
    >
      <div class="panel">
        <div class="panel-header" style="gap: 8px;">
          <div class="panel-header-row">
            <div class="panel-title">知识库素材</div>
            <n-space size="small" align="center">
              <n-tooltip trigger="hover">
                <template #trigger>
                  <n-button size="tiny" quaternary circle @click="toggleRightCollapsed">
                    <template #icon>
                      <n-icon :component="rightCollapsed ? ChevronBackOutline : ChevronForwardOutline" />
                    </template>
                  </n-button>
                </template>
                {{ rightCollapsed ? '展开右侧' : '折叠右侧' }}
              </n-tooltip>
            </n-space>
          </div>

          <n-select v-model:value="selectedKbPath" size="small" :options="kbOptions" filterable placeholder="选择知识库..."
            style="width: 100%;" />
          <n-input v-model:value="kbFilter" size="small" clearable placeholder="过滤文件名...">
            <template #prefix>
              <n-icon :component="SearchOutline" />
            </template>
          </n-input>
          <n-space size="small" justify="space-between">
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

        <div class="panel-body">
          <n-spin :show="kbLoading">
            <div class="tree-host">
              <GuaxiSlidevTreeView
                :nodes="filteredKbTree"
                v-model:selected-key="kbSelectedKey"
                v-model:expanded-keys="kbExpandedKeys"
                empty-text="暂无知识库文件"
                @select="handleKbSelectNode"
              />
            </div>
          </n-spin>

          <n-divider />

          <n-card v-if="material.path" size="small" title="素材预览">
            <n-space vertical size="small">
              <n-text depth="3" class="path-text" :title="material.path">{{ material.path }}</n-text>
              <n-input :value="materialPreview" type="textarea" :rows="10" readonly />
              <n-space size="small">
                <n-button size="small" type="primary" :disabled="material.loading" @click="insertMaterial">插入到
                  PPT</n-button>
                <n-button size="small" quaternary :disabled="material.loading" @click="reloadMaterial">重新读取</n-button>
              </n-space>
            </n-space>
          </n-card>
        </div>
      </div>
      <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSider" />
    </n-layout-sider>
  </n-layout>

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
import { computed, onBeforeUnmount, onMounted, reactive, ref, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
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
  NSelect,
  NDivider,
  NCard,
  NModal,
  NText,
  NIcon,
  NTooltip
} from 'naive-ui'
import { createLogger } from '../../utils/logger'
import { useSlidevEditorStore } from '../../stores/slidevEditorStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import {
  AddOutline,
  ChevronBackOutline,
  ChevronForwardOutline,
  DocumentTextOutline,
  DownloadOutline,
  EyeOutline,
  PlayCircleOutline,
  RefreshOutline,
  SearchOutline,
  SaveOutline,
  StopCircleOutline,
  TrashOutline
} from '@vicons/ionicons5'
import GuaxiSlidevLiteEditor from './GuaxiSlidevLiteEditor.vue'
import GuaxiSlidevTreeView from './GuaxiSlidevTreeView.vue'

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

const normalizePath = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$/g, '')

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

const onEditorDraftFlush = (payload) => {
  const path = normalizePath(payload?.path)
  if (!path) return
  const content = String(payload?.content ?? '')
  slidevEditorStore.applyDraft(path, content)
  logger.info('[Slidev] draft flush', { path, len: content.length, tag: payload?.tag })
}

const isHiddenName = (name) => {
  const s = String(name || '').trim()
  return !s || s.startsWith('.')
}

const leftCollapsed = ref(false)
const rightCollapsed = ref(false)

// 说明：与主窗口一致，左右侧栏支持拖拽调宽；这里不使用 CSS resize，避免不同平台表现不一致
const leftSiderWidth = ref(320)
const rightSiderWidth = ref(380)

const MIN_LEFT_SIDER_WIDTH = 260
const MAX_LEFT_SIDER_WIDTH = 560
const MIN_RIGHT_SIDER_WIDTH = 300
const MAX_RIGHT_SIDER_WIDTH = 760

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
    .filter((n) => !isHiddenName(n?.name))
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

    slidevExpandedKeys.value = [slidevRoot.value]
    slidevLoadedDirKeys.value = new Set([slidevRoot.value])
    logger.info('[Slidev] loaded root', { root: slidevRoot.value, count: children.length })
  } catch (e) {
    logger.error('[Slidev] refresh failed', e)
    message.error('加载 PPT 源文件目录失败')
  } finally {
    slidevTreeLoading.value = false
  }
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

const previewSlidev = async () => {
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
    logger.info('[Slidev] preview start', { traceId, entryPath: path, port })

    const info = await invoke('slidev_start_dev', { entryPath: path, port })
    const url = String(info?.url || '')
    logger.info('[Slidev] preview start_dev ok', { traceId, info, url })

    if (!url) {
      message.error('未获取到预览地址')
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
  } catch (e) {
    logger.error('[Slidev] preview failed', { traceId, err: e })
    message.error('启动预览失败（需要本机可用的 Node.js / npx）')
  } finally {
    logger.info('[Slidev] preview end', { traceId, costMs: Math.round(performance.now() - t0) })
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
    logger.info('[Slidev] presenter start_dev ok', { traceId, info, url })

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
    message.success('已停止预览进程')
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
const kbTree = ref([])
const kbFilter = ref('')
const selectedKbPath = ref('')
const kbSelectedKeys = ref([])

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

const loadKbTree = async () => {
  const rootPath = normalizePath(selectedKbPath.value)
  if (!rootPath) {
    message.warning('请先选择一个知识库')
    return
  }

  kbLoading.value = true
  try {
    const root = await invoke('get_knowledge_base_tree', { rootPath })
    const node = toKbTreeNode(root)
    kbTree.value = node ? [node] : []
    logger.info('[KB] tree loaded', { rootPath })
  } catch (e) {
    logger.error('[KB] load tree failed', e)
    message.error('加载知识库目录树失败')
  } finally {
    kbLoading.value = false
  }
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
  loading: false,
})

const materialPreview = computed(() => {
  const s = String(material.content || '')
  if (!s) return ''
  return s.length > 4000 ? `${s.slice(0, 4000)}\n\n...（已截断）` : s
})

const clearMaterial = () => {
  material.path = ''
  material.content = ''
  material.loading = false
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
  await readMaterial(key)
}

const insertMaterial = async () => {
  const content = String(material.content || '')
  if (!material.path || !content) {
    message.warning('请先选择一个素材文件')
    return
  }

  try {
    const text = content.endsWith('\n') ? content : `${content}\n`
    const ok = await pptEditorRef.value?.insertContent?.(`\n${text}`)
    logger.info('[KB] insert to editor', { ok: !!ok, path: material.path, len: text.length })
    if (!ok) {
      message.warning('插入失败：请先打开一个 PPT 源文件')
      return
    }
    message.success('已插入到 PPT 源文件')
  } catch (e) {
    logger.error('[KB] insert failed', e)
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
}

.sider-toggle-left {
  margin-left: -14px;
}

.sider-toggle-left.collapsed {
  margin-left: 12px;
}

.sider-toggle-right {
  margin-right: -14px;
}

.sider-toggle-right.collapsed {
  margin-right: 12px;
}

.slidev-center {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}

.editor-toolbar {
  padding: 10px 12px;
  border-bottom: 1px solid var(--n-border-color);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.editor-host {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

.editor-host :deep(.slidev-lite-editor) {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  height: 100%;
  width: 100%;
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

.tree-label {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.panel-body {
  flex: 1;
  min-height: 0;
  padding: 10px;
  overflow: hidden;
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
</style>
