<template>
  <div class="graph-page">
    <div v-if="leftVisible" class="graph-left" :style="{ width: `${leftWidth}px` }">
      <div class="graph-left-body">
        <n-tree v-if="treeData.length" class="kb-tree" block-line :data="treeData" :expanded-keys="expandedKeysModel"
          :selected-keys="selectedKeysModel" @update:expanded-keys="expandedKeysModel = $event"
          @update:selected-keys="handleSelectFromTree" />
        <div v-else class="empty">请选择一个知识库</div>
      </div>
    </div>

    <div v-if="leftVisible" class="splitter">
      <div class="splitter-hit" @mousedown="startResize('left', $event)"></div>
    </div>

    <div class="graph-center">
      <div class="graph-toolbar">
        <div class="graph-hint">{{ graphHint }}</div>
        <n-space size="small" align="center">
          <n-button size="tiny" :disabled="!treeData.length" @click="zoomOut">-</n-button>
          <div class="graph-zoom">{{ Math.round(graphZoom * 100) }}%</div>
          <n-button size="tiny" :disabled="!treeData.length" @click="zoomIn">+</n-button>
          <n-button size="tiny" :disabled="!treeData.length" @click="resetZoom">重置</n-button>

          <n-tooltip v-if="canExitSubGraph" trigger="hover">
            <template #trigger>
              <n-icon-wrapper
                :size="26"
                :border-radius="8"
                class="toolbar-icon"
                @click="exitSubGraph"
              >
                <n-icon :component="ArrowBackOutline" size="16" />
              </n-icon-wrapper>
            </template>
            返回父图
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper
                :size="26"
                :border-radius="8"
                class="toolbar-icon"
                @click="exportGraphAsSvg"
              >
                <n-icon :component="DownloadOutline" size="16" />
              </n-icon-wrapper>
            </template>
            导出图形
          </n-tooltip>
        </n-space>
      </div>

      <div
        ref="graphCanvasRef"
        class="graph-canvas"
        :class="{ panning: isPanning }"
        @wheel="handleWheel"
        @mousedown="startPan"
        @click="handleCanvasClick"
      >
        <div v-if="graphLayout.nodes.length" class="graph-stage-outer" :style="graphStageOuterStyle">
          <div class="graph-stage-inner" :style="graphStageInnerStyle">
            <svg class="graph-svg" :width="graphLayout.width" :height="graphLayout.height">
              <g class="edges">
                <path v-for="e in graphLayout.edges" :key="`${e.from}->${e.to}`" class="edge" :d="e.d" />
              </g>

              <g class="nodes">
                <g v-for="n in graphLayout.nodes" :key="n.key" class="node"
                  :class="{ selected: n.key === selectedKey, dir: n.kind === 'dir', file: n.kind === 'file', paragraph: n.kind === 'paragraph', expanded: isNodeExpanded(n) }"
                  :transform="`translate(${n.x},${n.y})`" @click.stop="selectGraphNode(n.key)">
                  <rect
                    class="node-rect"
                    :width="nodeRenderSize(n).w"
                    :height="nodeRenderSize(n).h"
                    :rx="nodeCornerRadius(n)"
                    :ry="nodeCornerRadius(n)"
                  />

                  <foreignObject v-if="editingKey === n.key" :x="8" :y="12" :width="nodeRenderSize(n).w - 16" :height="nodeRenderSize(n).h - 24">
                    <div class="inline-edit-wrap">
                      <input
                        class="inline-edit-input"
                        v-model="editingValue"
                        :data-edit-key="n.key"
                        @keydown.enter.prevent.stop="confirmInlineEdit"
                        @keydown.esc.prevent.stop="cancelInlineEdit"
                        @blur="confirmInlineEdit"
                      />
                    </div>
                  </foreignObject>

                  <foreignObject v-else-if="isNodeExpanded(n)" :x="8" :y="10" :width="nodeRenderSize(n).w - 16" :height="nodeRenderSize(n).h - 20">
                    <div class="node-expanded-fo">
                      <div class="node-expanded-title">{{ n.displayLabel }}</div>
                      <div v-if="nodeContentLoading" class="node-expanded-loading">内容加载中...</div>
                      <pre class="node-expanded-pre">{{ expandedNodeText }}</pre>
                    </div>
                  </foreignObject>

                  <text v-else class="node-text" :x="nodeRenderSize(n).w / 2" :y="nodeRenderSize(n).h / 2 + 5" text-anchor="middle">
                    {{ n.displayLabel }}
                  </text>

                  <g v-if="n.key === selectedKey" class="node-actions" :transform="`translate(${nodeRenderSize(n).w - 126}, -11)`">
                    <g class="node-view" transform="translate(104,0)" @click.stop="toggleNodeExpand(n.key)">
                      <title>查看内容</title>
                      <rect class="node-view-rect" width="22" height="22" rx="11" ry="11" />
                      <text class="node-view-text" x="11" y="15">☰</text>
                    </g>

                    <g v-if="n.isDir" class="node-plus" transform="translate(78,0)" @click.stop="openCreateTypeModal">
                      <title>新建（分类 / 知识点）</title>
                      <rect class="node-plus-rect" width="22" height="22" rx="11" ry="11" />
                      <text class="node-plus-text" x="11" y="15">+</text>
                    </g>

                    <g v-if="n.key !== graphRootKey && n.kind !== 'paragraph'" class="node-subgraph" transform="translate(52,0)" @click.stop="enterSubGraph(n.key)">
                      <title>进入子图</title>
                      <rect class="node-subgraph-rect" width="22" height="22" rx="11" ry="11" />
                      <text class="node-subgraph-text" x="11" y="15">⤵</text>
                    </g>

                    <g v-if="n.key !== rootKey && n.kind !== 'paragraph'" class="node-rename" transform="translate(26,0)" @click.stop="startInlineEdit(n.key)">
                      <title>重命名</title>
                      <rect class="node-rename-rect" width="22" height="22" rx="11" ry="11" />
                      <text class="node-rename-text" x="11" y="15">✎</text>
                    </g>

                    <g v-if="n.key !== rootKey && n.kind !== 'paragraph'" class="node-delete" @click.stop="deleteSelectedNode">
                      <title>删除</title>
                      <rect class="node-delete-rect" width="22" height="22" rx="11" ry="11" />
                      <text class="node-delete-text" x="11" y="15">×</text>
                    </g>
                  </g>
                </g>
              </g>
            </svg>
          </div>
        </div>

        <div v-else class="empty">请选择一个知识库</div>
      </div>
    </div>

    <div v-if="rightVisible" class="splitter">
      <div class="splitter-hit" @mousedown="startResize('right', $event)"></div>
    </div>

    <div v-if="rightVisible" class="graph-right" :style="{ width: `${rightWidth}px` }">
      <div class="right-sider-content">
        <n-space vertical :size="12" class="panel">
          <n-card title="节点信息" size="small">
            <n-space v-if="selectedNode" vertical>
              <n-tag :type="getNodeTagType(selectedNodeKind)" size="small">
                {{ kindLabel(selectedNodeKind) }}
              </n-tag>

              <div class="node-content-wrap">
                <div v-if="nodeContentLoading" class="node-content-loading">内容加载中...</div>
                <pre class="node-content-pre">{{ nodeContentFullText || nodeContentText }}</pre>
              </div>

              <n-button-group size="small">
                <n-button @click="focusSelected">聚焦</n-button>
                <n-button v-if="canOpenSelected" @click.stop="openSelectedInHome">打开</n-button>
                <n-button v-if="selectedKey && selectedKey !== graphRootKey" @click="enterSubGraph(selectedKey)">子图</n-button>
                <n-button @click="clearSelected">清除</n-button>
              </n-button-group>
            </n-space>
            <n-text v-else depth="3">点击图谱中的节点查看属性</n-text>
          </n-card>


          <n-card title="搜索" size="small">
            <n-input v-model:value="searchQueryModel" placeholder="搜索节点..." clearable />
          </n-card>

          <n-card title="类型筛选" size="small">
            <n-checkbox-group v-model:value="visibleKindsModel">
              <n-space vertical>
                <n-checkbox v-for="opt in kindOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</n-checkbox>
              </n-space>
            </n-checkbox-group>
          </n-card>

          <n-card title="结果" size="small">
            <n-list v-if="filteredResults.length > 0" class="search-results">
              <n-list-item v-for="n in filteredResults" :key="n.key" @click="focusNodeFromResult(n)">
                <n-thing :title="n.title" :description="n.description" />
              </n-list-item>
            </n-list>
            <n-text v-else depth="3">无匹配结果</n-text>
          </n-card>
        </n-space>
      </div>
    </div>

    <n-modal v-model:show="createTypeVisible" preset="dialog" title="新建" :mask-closable="true">
      <div class="create-type-hint">请选择要创建的类型</div>
      <n-space justify="end" size="small">
        <n-button @click="createTypeVisible = false">取消</n-button>
        <n-button type="primary" :disabled="loading" @click="chooseCreateType('folder')">分类</n-button>
        <n-button type="primary" :disabled="loading" @click="chooseCreateType('file')">知识点</n-button>
      </n-space>
    </n-modal>

    <n-modal v-model:show="graphModalVisible" preset="dialog" :title="graphModalTitle" :mask-closable="false">
      <n-form label-placement="left" label-width="80" :show-require-mark="false">
        <n-form-item label="名称">
          <n-input v-model:value="graphModalName" :placeholder="graphModalPlaceholder" />
        </n-form-item>
      </n-form>
      <n-space justify="end">
        <n-button @click="graphModalVisible = false">取消</n-button>
        <n-button type="primary" :loading="loading" @click="confirmGraphModal">确定</n-button>
      </n-space>
    </n-modal>

  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import {
  NButton,
  NButtonGroup,
  NCard,
  NCheckbox,
  NCheckboxGroup,
  NForm,
  NFormItem,
  NIcon,
  NIconWrapper,
  NInput,
  NList,
  NListItem,
  NModal,
  NSpace,
  NTag,
  NText,
  NThing,
  NTooltip,
  NTree,
  createDiscreteApi
} from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { emitTo } from '@tauri-apps/api/event'
import { confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import { ArrowBackOutline, DownloadOutline } from '@vicons/ionicons5'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useEditorStore } from '../../stores/editorStore'

const props = defineProps({
  loading: { type: Boolean, default: false },
  kbOptions: { type: Array, default: () => [] },

  selectedKbName: { type: String, default: '' },
  expandedKeys: { type: Array, default: () => [] },
  selectedKeys: { type: Array, default: () => [] },

  leftVisible: { type: Boolean, default: true },
  rightVisible: { type: Boolean, default: false },

  treeData: { type: Array, default: () => [] },

  kbLocation: { type: String, default: '' },
  rootKey: { type: String, default: '__kb_root__' }
})

const emit = defineEmits([
  'update:selectedKbName',
  'update:expandedKeys',
  'update:selectedKeys',
  'update:leftVisible',
  'update:rightVisible',
  'patch-tree',
  'reload'
])

const { message } = createDiscreteApi(['message'])

const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()

// 左右面板显隐与拖拽调整宽度
const leftVisible = ref(!!props.leftVisible)
const rightVisible = ref(!!props.rightVisible)
const leftWidth = ref(320)
const rightWidth = ref(360)

watch(() => props.leftVisible, (v) => {
  if (typeof v === 'boolean') leftVisible.value = v
})

watch(() => props.rightVisible, (v) => {
  if (typeof v === 'boolean') rightVisible.value = v
})

const resizeState = ref({ dragging: false, side: '', startX: 0, startWidth: 0 })

const clamp = (v, min, max) => Math.min(max, Math.max(min, v))

const onResizeMove = (e) => {
  if (!resizeState.value.dragging) return
  const { side, startX, startWidth } = resizeState.value
  const dx = e.clientX - startX

  if (side === 'left') {
    leftWidth.value = clamp(startWidth + dx, 220, 560)
  } else if (side === 'right') {
    rightWidth.value = clamp(startWidth - dx, 260, 680)
  }
}

const stopResize = () => {
  if (!resizeState.value.dragging) return
  resizeState.value.dragging = false
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onResizeMove)
  window.removeEventListener('mouseup', stopResize)
  console.info('[KBGraphEditor] resize stop', { leftWidth: leftWidth.value, rightWidth: rightWidth.value })
}

const startResize = (side, e) => {
  if (side === 'left' && !leftVisible.value) return
  if (side === 'right' && !rightVisible.value) return

  const startWidth = side === 'left' ? leftWidth.value : rightWidth.value
  resizeState.value = { dragging: true, side, startX: e.clientX, startWidth }
  document.body.style.userSelect = 'none'

  window.addEventListener('mousemove', onResizeMove)
  window.addEventListener('mouseup', stopResize)
  console.info('[KBGraphEditor] resize start', { side, startWidth })
}

const setLeftVisible = (v) => {
  leftVisible.value = !!v
  emit('update:leftVisible', leftVisible.value)
  console.info('[KBGraphEditor] toggle left panel', { visible: leftVisible.value })
}

const setRightVisible = (v) => {
  rightVisible.value = !!v
  emit('update:rightVisible', rightVisible.value)
  console.info('[KBGraphEditor] toggle right panel', { visible: rightVisible.value })
}

const toggleLeftVisible = () => setLeftVisible(!leftVisible.value)
const toggleRightVisible = () => setRightVisible(!rightVisible.value)

onBeforeUnmount(() => {
  stopResize()
  stopPan()
})

// 子图导航状态：viewRootKey 为当前图根节点；stack 用于返回父图时恢复展开/选中/滚动/缩放
const viewRootKey = ref('')
const viewRootStack = ref([])

// 当前视图根节点 key（默认是知识库根）；子图模式下会变为某个目录/文件节点 key
const graphRootKey = computed(() => String(viewRootKey.value || props.rootKey))

// 是否处于子图模式（堆栈非空即意味着可返回）
const canExitSubGraph = computed(() => viewRootStack.value.length > 0)

const expandedKeysModel = computed({
  get: () => Array.isArray(props.expandedKeys) ? props.expandedKeys : [],
  set: (v) => emit('update:expandedKeys', Array.isArray(v) ? v : [])
})

const selectedKeysModel = computed({
  get: () => Array.isArray(props.selectedKeys) ? props.selectedKeys : [],
  set: (v) => emit('update:selectedKeys', Array.isArray(v) ? v : [])
})

// 当前选中的节点 key；允许为空（用于“点击空白取消选中/隐藏节点操作按钮”）
const selectedKey = computed(() => selectedKeysModel.value[0] || '')

const normalizeRel = (p) => String(p || '').replace(/\\/g, '/').replace(/^\/+/, '').trim()
const normalizeBase = (p) => String(p || '').replace(/\\/g, '/').replace(/\/+$/, '').trim()

const toAbsPath = (relPath) => {
  const base = normalizeBase(props.kbLocation)
  const rel = normalizeRel(relPath)
  if (!base) return ''
  if (!rel) return base
  return `${base}/${rel}`
}

const nodeMap = computed(() => {
  const m = new Map()

  const walk = (n, parentKey) => {
    if (!n || typeof n.key !== 'string') return
    const key = String(n.key)
    const isRoot = key === props.rootKey
    const relPath = isRoot ? '' : normalizeRel(key)

    const rawKind = String(n?.kind || n?.nodeKind || n?.type || '').toLowerCase()
    const kind = (rawKind === 'paragraph' || rawKind === 'para' || n?.isParagraph)
      ? 'paragraph'
      : (n?.isDir ? 'dir' : 'file')

    const parentRelPath = parentKey ? (m.get(parentKey)?.relPath || '') : ''
    const effectiveRelPath = kind === 'paragraph' ? parentRelPath : relPath

    m.set(key, {
      key,
      kind,
      name: String(n.label || ''),
      isDir: !!n.isDir,
      isParagraph: kind === 'paragraph',
      parentKey: parentKey || null,
      relPath: effectiveRelPath,
      absPath: toAbsPath(effectiveRelPath),
      stableId: typeof n?.stableId === 'string' ? n.stableId : (typeof n?.stable_id === 'string' ? n.stable_id : ''),
      startLine: n?.startLine ?? n?.start_line,
      endLine: n?.endLine ?? n?.end_line
    })

    const kids = Array.isArray(n.children) ? n.children : []
    for (const ch of kids) walk(ch, key)
  }

  const root = Array.isArray(props.treeData) ? props.treeData[0] : null
  if (root) walk(root, null)
  return m
})

const selectedNode = computed(() => nodeMap.value.get(selectedKey.value) || null)

const inspectorDraft = ref({ name: '' })

const stripMdExt = (name) => {
  const s = String(name || '')
  return s.toLowerCase().endsWith('.md') ? s.slice(0, -3) : s
}

watch(selectedNode, (n) => {
  if (!n) {
    inspectorDraft.value = { name: '' }
    return
  }

  const draftName = n.isDir ? String(n.name || '') : stripMdExt(n.name)
  inspectorDraft.value = { name: draftName }
}, { immediate: true })

const fileContentCache = new Map()

const nodeContentLoading = ref(false)
const nodeContentText = ref('')
const nodeContentFullText = ref('')

const expandedNodeKey = ref('')

const expandedNodeText = computed(() => {
  return String(nodeContentFullText.value || nodeContentText.value || '')
})

const expandedNodeHeight = computed(() => {
  const t = String(expandedNodeText.value || '')
  if (!t) return 220
  const lines = t.split('\n')
  const lineCount = Math.max(1, lines.length)

  // 只膨胀“高度”，宽度保持与普通节点一致，避免横向遮挡；内容使用滚动查看
  const h = 120 + Math.min(320, Math.floor(lineCount * 18))
  return clamp(h, 160, 520)
})

const isNodeExpanded = (layoutNode) => {
  const k = String(layoutNode?.key || '')
  return k && k === expandedNodeKey.value && k === String(selectedKey.value || '')
}

const nodeRenderSize = (layoutNode) => {
  if (!isNodeExpanded(layoutNode)) return graphNodeSize
  return { w: graphNodeSize.w, h: expandedNodeHeight.value }
}

const toggleNodeExpand = async (key) => {
  const k = String(key || '')
  if (!k) return

  if (expandedNodeKey.value === k) {
    expandedNodeKey.value = ''
    console.info('[KBGraphEditor] collapse node', { key: k })
    return
  }

  expandedNodeKey.value = k
  console.info('[KBGraphEditor] expand node', { key: k })

  const n = nodeMap.value.get(k)
  if (n) {
    await loadTextForNode(n)
  }
}

const canOpenSelected = computed(() => {
  const n = selectedNode.value
  if (!n) return false
  return ['file', 'paragraph'].includes(String(n.kind || '')) && !!String(n.absPath || '')
})

const normalizeNewline = (s) => String(s || '').replace(/\r\n/g, '\n')

const extractParagraph = (content, startLine, endLine) => {
  const text = normalizeNewline(content)
  const lines = text.split('\n')

  const start = Math.max(1, Number(startLine || 1))
  const end = endLine != null
    ? Math.max(start, Number(endLine || start))
    : lines.length

  return lines.slice(start - 1, Math.min(end, lines.length)).join('\n')
}

const applyNodeContent = (text, meta = {}) => {
  const v = String(text || '')
  nodeContentText.value = v
  nodeContentFullText.value = v
  console.info('[KBGraphEditor] node content loaded', { key: selectedKey.value, len: v.length, ...meta })
}

const loadTextForNode = async (n) => {
  if (!n) {
    nodeContentText.value = ''
    nodeContentFullText.value = ''
    return
  }

  const kind = String(n.kind || '')
  if (kind === 'dir') {
    applyNodeContent(String(n.name || ''), { kind })
    return
  }

  const absPath = String(n.absPath || '')
  if (!absPath) {
    applyNodeContent(String(n.name || ''), { kind, absPath: '' })
    return
  }

  nodeContentLoading.value = true
  try {
    let content = fileContentCache.get(absPath)
    if (typeof content !== 'string') {
      console.info('[KBGraphEditor] load node content', { absPath, kind })
      const res = await fileTreeStore.fetchFileContent(absPath)
      if (!res?.ok) {
        throw new Error('读取文件内容失败')
      }
      content = String(res.content || '')
      fileContentCache.set(absPath, content)
    }

    if (kind === 'paragraph') {
      const para = extractParagraph(content, n.startLine, n.endLine)
      applyNodeContent(para, { kind, absPath, startLine: n.startLine, endLine: n.endLine })
      return
    }

    applyNodeContent(content, { kind, absPath })
  } catch (e) {
    console.error('[KBGraphEditor] load node content failed:', e)
    applyNodeContent(String(n.name || ''), { kind, absPath, fallback: true })
  } finally {
    nodeContentLoading.value = false
  }
}

watch(selectedNode, (n) => {
  loadTextForNode(n)
}, { immediate: true })

watch(selectedKey, (k) => {
  const key = String(k || '')
  if (expandedNodeKey.value && expandedNodeKey.value !== key) {
    console.info('[KBGraphEditor] collapse expanded node (selection changed)', {
      expanded: expandedNodeKey.value,
      selected: key,
    })
    expandedNodeKey.value = ''
  }
})

const OPEN_FILE_IN_HOME_EVENT = 'open_file_in_home'

const openSelectedInHome = async () => {
  const n = selectedNode.value
  if (!n) return

  const kind = String(n.kind || '')
  if (!['file', 'paragraph'].includes(kind)) return

  const path = String(n.absPath || '')
  if (!path) return

  const lineNumber = kind === 'paragraph' ? Math.max(1, Number(n.startLine || 1)) : 1

  const payload = { path, kind, lineNumber }

  try {
    console.info('[KBGraphEditor] emit open_file_in_home', { target: 'main', payload })
    await emitTo('main', OPEN_FILE_IN_HOME_EVENT, payload)
    message.success('已通知主窗口打开')
  } catch (e) {
    console.warn('[KBGraphEditor] emit open_file_in_home failed, fallback to local open:', e)

    try {
      const res = await fileTreeStore.fetchFileContent(path)
      if (!res?.ok) {
        message.error('打开失败：读取文件内容失败')
        return
      }

      await fileTreeStore.openFileInTab(path, res.content, { unsupported: !!res.unsupported })

      editorStore.setFile({
        path,
        content: String(res.content || ''),
        highlightInfo: {
          lineNumber,
          matches: [{ start: 0, end: 1000000 }],
          text: ''
        }
      })
    } catch (err) {
      console.error('[KBGraphEditor] fallback local open failed:', err)
      message.error(String(err?.message || '打开失败'))
    }
  }
}

const searchQuery = ref('')
const visibleKinds = ref(['dir', 'file', 'paragraph'])

const kindOptions = [
  { label: '目录', value: 'dir' },
  { label: '文件', value: 'file' },
  { label: '段落', value: 'paragraph' }
]

const searchQueryModel = computed({
  get: () => searchQuery.value,
  set: (v) => { searchQuery.value = String(v || '') }
})

const visibleKindsModel = computed({
  get: () => visibleKinds.value,
  set: (v) => { visibleKinds.value = Array.isArray(v) ? v : [] }
})

const selectedNodeKind = computed(() => {
  const n = selectedNode.value
  if (!n) return ''
  return String(n.kind || (n.isParagraph ? 'paragraph' : (n.isDir ? 'dir' : 'file')))
})

const kindLabel = (k) => {
  const hit = kindOptions.find(x => x.value === k)
  return hit ? hit.label : String(k || '')
}

const getNodeTagType = (k) => {
  switch (k) {
    case 'dir': return 'info'
    case 'file': return 'success'
    case 'paragraph': return 'default'
    default: return 'default'
  }
}

const normalize = (v) => String(v || '').toLowerCase()

const filteredResults = computed(() => {
  const all = Array.from(nodeMap.value.values())
  const allowed = new Set(Array.isArray(visibleKinds.value) ? visibleKinds.value : [])
  const q = normalize(searchQuery.value).trim()

  const out = all.filter((n) => {
    if (!n) return false
    const k = String(n.kind || (n.isParagraph ? 'paragraph' : (n.isDir ? 'dir' : 'file')))
    if (allowed.size && !allowed.has(k)) return false

    if (!q) return true
    const hay = `${n.name || ''} ${n.relPath || ''} ${n.absPath || ''} ${n.stableId || ''}`
    return normalize(hay).includes(q)
  }).map((n) => {
    const k = String(n.kind || (n.isParagraph ? 'paragraph' : (n.isDir ? 'dir' : 'file')))
    const title = String(n.name || '')
    const descParts = [kindLabel(k)]
    if (n.relPath) descParts.push(String(n.relPath))
    if (n.stableId) descParts.push(String(n.stableId))
    if (n.startLine != null || n.endLine != null) descParts.push(`L${n.startLine ?? ''}-${n.endLine ?? ''}`)
    return { key: n.key, kind: k, parentKey: n.parentKey, title, description: descParts.join(' | ') }
  })

  out.sort((a, b) => {
    if (a.kind !== b.kind) return String(a.kind).localeCompare(String(b.kind), 'zh')
    return String(a.title).localeCompare(String(b.title), 'zh')
  })

  return out.slice(0, 200)
})

const focusSelected = () => {
  const k = String(selectedKey.value || '')
  if (!k) return
  centerOnKey(k)
  console.info('[KBGraphEditor] right panel focus selected', { key: k })
}

const clearSelected = () => {
  cancelInlineEdit()
  selectedKeysModel.value = []
  console.info('[KBGraphEditor] right panel clear selected')
}

const focusNodeFromResult = (n) => {
  const key = String(n?.key || '')
  if (!key) return

  if (String(n?.kind) === 'paragraph' && n?.parentKey) {
    ensureExpandedToKey(String(n.parentKey))
  }

  selectGraphNode(key)
  console.info('[KBGraphEditor] right panel focus node', { key })
}

const graphCanvasRef = ref(null)

const graphNodeSize = { w: 220, h: 54 }
const graphZoom = ref(1)

// 画布拖拽平移：按住空白区域拖动即可滚动查看全图（不影响节点点击/编辑）
// 画布拖拽平移：按住空白区域拖动即可滚动查看全图（不影响节点点击/编辑）
// 画布拖拽平移：按住空白区域拖动即可滚动查看全图（不影响节点点击/编辑）
const isPanning = ref(false)
const lastPanMovedAt = ref(0)
const panState = ref({ dragging: false, moved: false, startX: 0, startY: 0, startLeft: 0, startTop: 0 })

const onPanMove = (e) => {
  if (!panState.value.dragging) return
  const el = graphCanvasRef.value
  if (!el) return

  const dx = e.clientX - panState.value.startX
  const dy = e.clientY - panState.value.startY

  if (!panState.value.moved && (Math.abs(dx) + Math.abs(dy) > 2)) {
    panState.value.moved = true
  }

  el.scrollLeft = panState.value.startLeft - dx
  el.scrollTop = panState.value.startTop - dy
}

const stopPan = () => {
  if (!panState.value.dragging) return
  panState.value.dragging = false
  isPanning.value = false
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onPanMove)
  window.removeEventListener('mouseup', stopPan)

  if (panState.value.moved) lastPanMovedAt.value = Date.now()
  console.info('[KBGraphEditor] pan stop', { moved: panState.value.moved })
}

const startPan = (e) => {
  if (!e || e.button !== 0) return
  const el = graphCanvasRef.value
  if (!el) return

  const target = e.target
  if (target?.closest?.('.node')) return
  if (target?.closest?.('.inline-edit-input')) return

  panState.value = {
    dragging: true,
    moved: false,
    startX: e.clientX,
    startY: e.clientY,
    startLeft: el.scrollLeft,
    startTop: el.scrollTop
  }
  isPanning.value = true

  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onPanMove)
  window.addEventListener('mouseup', stopPan)

  console.info('[KBGraphEditor] pan start', { left: el.scrollLeft, top: el.scrollTop })
}

const kindForTreeNode = (n) => {
  const raw = String(n?.kind || n?.nodeKind || n?.type || '').toLowerCase()
  if (raw === 'paragraph' || raw === 'para') return 'paragraph'
  if (n?.isParagraph) return 'paragraph'
  return n?.isDir ? 'dir' : 'file'
}

const nodeCornerRadius = (layoutNode) => {
  const kind = String(layoutNode?.kind || '')
  if (kind === 'dir') return 14
  if (kind === 'file') return 10
  if (kind === 'paragraph') return 6
  return 10
}

const setZoom = (v) => {
  const next = Math.round((Number(v) || 1) * 10) / 10
  graphZoom.value = clamp(next, 0.5, 2)
}

const zoomIn = () => setZoom(graphZoom.value + 0.1)
const zoomOut = () => setZoom(graphZoom.value - 0.1)
const resetZoom = () => setZoom(1)

// Ctrl + 滚轮缩放（避免系统/浏览器缩放干扰）
const handleWheel = (e) => {
  if (!e?.ctrlKey) return
  e.preventDefault()
  if (e.deltaY > 0) zoomOut()
  else zoomIn()
  console.info('[KBGraphEditor] zoom wheel', { zoom: graphZoom.value })
}

// 导出：使用保存对话框写入文件（避免外部 CSS 丢失，所以会把关键样式写进 SVG）
const sanitizeFileName = (name) => {
  const s = String(name || '').trim() || 'graph'
  return s.replace(/[\\/:*?"<>|]/g, '_')
}

const readCssVar = (name, fallback) => {
  try {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name)
    return String(v || '').trim() || fallback
  } catch (e) {
    console.warn('[KBGraphEditor] readCssVar failed', { name, e })
    return fallback
  }
}

// 导出时读取主题色：优先从当前图形容器取值，其次 root/body（避免某些情况下根节点变量未生效导致导出背景变白）
const readCssVarFrom = (el, name, fallback) => {
  const candidates = [el, document.documentElement, document.body].filter(Boolean)
  for (const c of candidates) {
    try {
      const v = getComputedStyle(c).getPropertyValue(name)
      const out = String(v || '').trim()
      if (out) return out
    } catch (e) {
      console.warn('[KBGraphEditor] readCssVarFrom failed', { name, target: c === el ? 'host' : (c === document.body ? 'body' : 'root'), e })
    }
  }
  return fallback
}

const saveBlobWithDialog = async (blob, defaultName, filters) => {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { writeFile } = await import('@tauri-apps/plugin-fs')

    const path = await save({ defaultPath: defaultName, filters })
    if (!path) {
      console.info('[KBGraphEditor] export canceled')
      return false
    }

    const buf = new Uint8Array(await blob.arrayBuffer())
    await writeFile(path, buf)

    console.info('[KBGraphEditor] export saved', { path, bytes: buf.byteLength })
    return true
  } catch (e) {
    console.error('[KBGraphEditor] export save failed:', e)
    message.error(String(e?.message || '保存失败'))
    return false
  }
}

const exportGraphAsSvg = async () => {
  if (!graphLayout.value?.nodes?.length) {
    message.warning('暂无可导出的图形')
    return
  }

  const host = graphCanvasRef.value
  const svg = host?.querySelector?.('svg.graph-svg')
  if (!svg) {
    console.warn('[KBGraphEditor] export svg missed: svg not found')
    message.error('导出失败：未找到图形')
    return
  }

  const theme = String(document?.documentElement?.dataset?.guaxiTheme || '').toLowerCase()

  const panelBg = readCssVarFrom(host, '--guaxi-panel-bg', theme === 'dark' ? '#21252b' : '#ffffff')
  const contentBg = readCssVarFrom(host, '--guaxi-content-bg', theme === 'dark' ? '#282c34' : '#ffffff')
  const border = readCssVarFrom(host, '--guaxi-border', theme === 'dark' ? '#3e4451' : '#d0d7de')
  const textPrimary = readCssVarFrom(host, '--guaxi-text-primary', theme === 'dark' ? '#abb2bf' : '#1f2328')

  console.info('[KBGraphEditor] export theme vars', { theme, panelBg, contentBg, border, textPrimary })

  const nodeW = graphNodeSize.w
  const nodeH = graphNodeSize.h
  const pad = 26

  let viewX = 0
  let viewY = 0
  let viewW = 0
  let viewH = 0

  // 优先使用 SVG 实际内容 bbox（更稳健，避免布局尺寸偏大导致右侧空白）
  try {
    const bbox = svg.getBBox()
    viewX = bbox.x - pad
    viewY = bbox.y - pad
    viewW = bbox.width + pad * 2
    viewH = bbox.height + pad * 2
    console.info('[KBGraphEditor] export bbox', { x: bbox.x, y: bbox.y, w: bbox.width, h: bbox.height })
  } catch (e) {
    console.warn('[KBGraphEditor] export getBBox failed, fallback to layout bounds', e)

    const xs = graphLayout.value.nodes.map(n => Number(n.x || 0))
    const ys = graphLayout.value.nodes.map(n => Number(n.y || 0))
    const minX = Math.min(...xs)
    const minY = Math.min(...ys)
    const maxX = Math.max(...xs) + nodeW
    const maxY = Math.max(...ys) + nodeH

    viewX = minX - pad
    viewY = minY - pad
    viewW = (maxX - minX) + pad * 2
    viewH = (maxY - minY) + pad * 2
  }

  const clone = svg.cloneNode(true)
  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
  clone.setAttribute('width', String(Math.ceil(viewW)))
  clone.setAttribute('height', String(Math.ceil(viewH)))
  clone.setAttribute('viewBox', `${viewX} ${viewY} ${viewW} ${viewH}`)

  const styleEl = document.createElementNS('http://www.w3.org/2000/svg', 'style')
  styleEl.textContent = `
svg{--guaxi-panel-bg:${panelBg};--guaxi-content-bg:${contentBg};--guaxi-border:${border};--guaxi-text-primary:${textPrimary};}
.edge{fill:none;stroke:rgba(33,189,236,.45);stroke-width:2;}
.node-rect{fill:${panelBg};stroke:rgba(33,189,236,.34);stroke-width:1.5;}
.node.dir .node-rect{stroke:rgba(24,160,88,.46);}
.node.paragraph .node-rect{stroke:rgba(179,127,235,.58);stroke-dasharray:4 3;}
.node.selected .node-rect{stroke:rgba(33,189,236,.90);stroke-width:2.5;}
.node-text{font-size:14px;fill:${textPrimary};opacity:.9;font-family:'Maple Mono NF CN','JetBrainsMono Nerd Font Mono',Consolas,monospace,'Microsoft YaHei',system-ui;}
.node-view-rect{fill:${panelBg};stroke:rgba(33,189,236,.68);stroke-width:1.5;}
.node-view-text{font-size:13px;text-anchor:middle;fill:${textPrimary};}
.node-plus-rect{fill:${panelBg};stroke:rgba(33,189,236,.68);stroke-width:1.5;}
.node-plus-text{font-size:14px;text-anchor:middle;fill:${textPrimary};}
.node-subgraph-rect{fill:${panelBg};stroke:rgba(179,127,235,.64);stroke-width:1.5;}
.node-subgraph-text{font-size:12px;text-anchor:middle;fill:${textPrimary};}
.node-rename-rect{fill:${panelBg};stroke:color-mix(in srgb, ${textPrimary} 40%, transparent);stroke-width:1.5;}
.node-rename-text{font-size:14px;text-anchor:middle;fill:${textPrimary};}
.node-delete-rect{fill:${panelBg};stroke:rgba(255,107,107,.70);stroke-width:1.5;}
.node-delete-text{font-size:14px;text-anchor:middle;fill:#ff6b6b;}
`

  const bg = document.createElementNS('http://www.w3.org/2000/svg', 'rect')
  bg.setAttribute('x', String(viewX))
  bg.setAttribute('y', String(viewY))
  bg.setAttribute('width', String(viewW))
  bg.setAttribute('height', String(viewH))
  bg.setAttribute('fill', contentBg)

  clone.insertBefore(styleEl, clone.firstChild)
  clone.insertBefore(bg, clone.firstChild)

  const xml = new XMLSerializer().serializeToString(clone)
  const svgText = `<?xml version="1.0" encoding="UTF-8"?>\n${xml}`

  const kb = sanitizeFileName(props.selectedKbName)
  const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
  const defaultName = `${kb || 'kb'}_graph_${ts}.svg`

  console.info('[KBGraphEditor] export svg', { viewW, viewH, defaultName })

  const ok = await saveBlobWithDialog(
    new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' }),
    defaultName,
    [{ name: 'SVG', extensions: ['svg'] }]
  )

  if (ok) message.success('已导出 SVG')
}

const graphHint = computed(() => {
  if (!props.selectedKbName) return '请选择一个知识库'
  if (!props.kbLocation) return '知识库路径为空，无法编辑文件系统'
  if (canExitSubGraph.value) return `子图模式：${graphRootKey.value}（点击返回图标可回到父图）`
  return '点击图形节点/左侧树选择；在右侧进行新增/重命名/删除（会同步到文件系统）'
})

const layoutTreeGraph = (root, activeRootKey) => {
  const nodes = []
  const edges = []

  const nodeW = graphNodeSize.w
  const nodeHForKey = (k) => {
    const key = String(k || '')
    if (!key) return graphNodeSize.h
    return key === expandedNodeKey.value ? Number(expandedNodeHeight.value || graphNodeSize.h) : graphNodeSize.h
  }
  const colGap = 56
  const colStep = nodeW + colGap
  const rowGap = 14
  const pad = 26

  const expanded = new Set(expandedKeysModel.value)

  const visibleChildrenOf = (n) => {
    const key = String(n?.key || '')
    const kids = Array.isArray(n?.children) ? n.children : []
    if (!key || !kids.length) return []
    return expanded.has(key) ? kids : []
  }

  const heightMemo = new Map()

  const measureHeight = (n) => {
    const key = String(n?.key || '')
    const selfH = nodeHForKey(key)
    if (!key) return selfH

    const cached = heightMemo.get(key)
    if (typeof cached === 'number') return cached

    const kids = visibleChildrenOf(n)
    if (!kids.length) {
      heightMemo.set(key, selfH)
      return selfH
    }

    const sumKids = kids.reduce((acc, ch) => acc + measureHeight(ch), 0)
    const total = sumKids + rowGap * Math.max(kids.length - 1, 0)
    const h = Math.max(selfH, total)
    heightMemo.set(key, h)
    return h
  }

  const putNode = (n, depth, topY, parentKey) => {
    const key = String(n?.key || '')
    if (!key) return

    const subtreeH = measureHeight(n)
    const selfH = nodeHForKey(key)
    const x = depth * colStep
    const y = topY + (subtreeH - selfH) / 2

    const kind = kindForTreeNode(n)

    nodes.push({
      key,
      x,
      y,
      w: nodeW,
      h: selfH,
      kind,
      isDir: !!n?.isDir,
      displayLabel: (() => {
        const raw = String(n?.label || '')
        const cleaned = n?.isDir ? raw : stripMdExt(raw)
        return cleaned.length > 16 ? `${cleaned.slice(0, 15)}…` : cleaned
      })()
    })

    if (parentKey) edges.push({ from: parentKey, to: key })

    const kids = visibleChildrenOf(n)
    let cursor = topY
    for (const ch of kids) {
      putNode(ch, depth + 1, cursor, key)
      cursor += measureHeight(ch) + rowGap
    }
  }

  if (root) putNode(root, 0, 0, null)

  const applyShift = (dx, dy) => {
    if (!dx && !dy) return
    for (const n of nodes) {
      n.x += dx
      n.y += dy
    }
  }

  const measureBounds = () => {
    if (!nodes.length) return { left: 0, top: 0, right: 0, bottom: 0 }

    const minX = Math.min(...nodes.map(n => n.x))
    const minY = Math.min(...nodes.map(n => n.y))
    const maxR = Math.max(...nodes.map(n => n.x + Number(n.w || nodeW)))
    const maxB = Math.max(...nodes.map(n => n.y + Number(n.h || graphNodeSize.h)))

    return {
      left: minX,
      top: minY,
      right: maxR,
      bottom: maxB
    }
  }

  const rootKey = String(activeRootKey || props.rootKey)
  const rootNode = nodes.find(n => n.key === rootKey)

  if (rootNode) {
    const b0 = measureBounds()
    const bboxCx = (b0.left + b0.right) / 2
    const bboxCy = (b0.top + b0.bottom) / 2

    const rootW = Number(rootNode.w || nodeW)
    const rootH = Number(rootNode.h || graphNodeSize.h)

    const rootCx = rootNode.x + rootW / 2
    const rootCy = rootNode.y + rootH / 2

    applyShift(bboxCx - rootCx, bboxCy - rootCy)

    const b1 = measureBounds()
    applyShift(pad - b1.left, pad - b1.top)
  } else {
    const b1 = measureBounds()
    applyShift(pad - b1.left, pad - b1.top)
  }

  const b = measureBounds()
  const width = Math.max(b.right - b.left + pad * 2, 520)
  const height = Math.max(b.bottom - b.top + pad * 2, 320)

  const pos = new Map(nodes.map(n => [n.key, n]))
  const outEdges = edges.map(e => {
    const a = pos.get(e.from)
    const b = pos.get(e.to)
    if (!a || !b) return null

    const aw = Number(a.w || nodeW)
    const ah = Number(a.h || graphNodeSize.h)
    const bh = Number(b.h || graphNodeSize.h)

    const x1 = a.x + aw
    const y1 = a.y + ah / 2
    const x2 = b.x
    const y2 = b.y + bh / 2

    const dx = Math.max(54, Math.min(110, (x2 - x1) / 2))
    const c1x = x1 + dx
    const c1y = y1
    const c2x = x2 - dx
    const c2y = y2

    return { ...e, d: `M ${x1} ${y1} C ${c1x} ${c1y}, ${c2x} ${c2y}, ${x2} ${y2}` }
  }).filter(Boolean)

  return { nodes, edges: outEdges, width, height }
}

const treeIndex = computed(() => {
  const m = new Map()
  const walk = (n) => {
    if (!n || typeof n.key !== 'string') return
    const key = String(n.key)
    m.set(key, n)
    const kids = Array.isArray(n.children) ? n.children : []
    for (const ch of kids) walk(ch)
  }
  const root = Array.isArray(props.treeData) ? props.treeData[0] : null
  if (root) walk(root)
  return m
})

const graphLayout = computed(() => {
  const root = Array.isArray(props.treeData) ? props.treeData[0] : null
  const viewRoot = treeIndex.value.get(graphRootKey.value) || root
  return layoutTreeGraph(viewRoot, graphRootKey.value)
})

const graphStageOuterStyle = computed(() => {
  return {
    width: `${graphLayout.value.width * graphZoom.value}px`,
    height: `${graphLayout.value.height * graphZoom.value}px`
  }
})

const graphStageInnerStyle = computed(() => {
  return {
    width: `${graphLayout.value.width}px`,
    height: `${graphLayout.value.height}px`,
    transform: `scale(${graphZoom.value})`,
    transformOrigin: '0 0'
  }
})

const ensureExpandedToKey = (key) => {
  const k = String(key || graphRootKey.value)
  const n = nodeMap.value.get(k)
  if (!n) return

  const rootK = graphRootKey.value
  const next = new Set(expandedKeysModel.value)
  next.add(rootK)

  const treeNode = treeIndex.value.get(k)
  const hasChildren = !!(treeNode && Array.isArray(treeNode.children) && treeNode.children.length)
  if ((n.isDir || hasChildren) && k !== rootK) next.add(k)

  let cur = n
  while (cur?.parentKey) {
    next.add(cur.parentKey)
    cur = nodeMap.value.get(cur.parentKey) || null
  }

  expandedKeysModel.value = Array.from(next)
}

const centerOnKey = (key) => {
  const el = graphCanvasRef.value
  if (!el) return

  const k = String(key || graphRootKey.value)
  const node = graphLayout.value.nodes.find(x => x.key === k)
  if (!node) return

  const zoom = graphZoom.value
  const pad = 12

  const nodeW = Number(node.w || graphNodeSize.w)
  const nodeH = Number(node.h || graphNodeSize.h)

  const cx = (node.x + nodeW / 2) * zoom
  const cy = (node.y + nodeH / 2) * zoom

  const nextLeft = Math.max(0, pad + cx - el.clientWidth / 2)
  const nextTop = Math.max(0, pad + cy - el.clientHeight / 2)

  try {
    el.scrollTo({ left: nextLeft, top: nextTop, behavior: 'smooth' })
  } catch {
    el.scrollLeft = nextLeft
    el.scrollTop = nextTop
  }

  console.info('[KBGraphEditor] focus node', { key: k, left: nextLeft, top: nextTop, zoom })
}

const collapseDirKey = (dirKey) => {
  const k = String(dirKey || graphRootKey.value)
  const next = new Set(expandedKeysModel.value)

  const rootK = graphRootKey.value
  if (k === rootK) {
    expandedKeysModel.value = []
    console.info('[KBGraphEditor] collapse root', { key: k })
    return
  }

  next.add(rootK)
  next.delete(k)

  for (const [key, item] of nodeMap.value.entries()) {
    let cur = item
    while (cur?.parentKey) {
      if (cur.parentKey === k) {
        next.delete(key)
        break
      }
      cur = nodeMap.value.get(cur.parentKey) || null
    }
  }

  expandedKeysModel.value = Array.from(next)
  console.info('[KBGraphEditor] collapse dir', { key: k })
}

const toggleDirExpanded = (dirKey) => {
  const k = String(dirKey || graphRootKey.value)
  const expandedSet = new Set(expandedKeysModel.value)
  if (expandedSet.has(k)) collapseDirKey(k)
  else {
    ensureExpandedToKey(k)
    console.info('[KBGraphEditor] expand dir', { key: k })
  }
}

// 进入子图：把当前节点作为“新的根”重绘，并把当前视图状态压栈以支持返回
// 进入子图：把当前节点作为“新的根”重绘，并把当前视图状态压栈以支持返回
const enterSubGraph = (key) => {
  const k = String(key || '')
  if (!k) return
  if (!treeIndex.value.get(k)) return

  const el = graphCanvasRef.value
  viewRootStack.value.push({
    rootKey: graphRootKey.value,
    expandedKeys: [...expandedKeysModel.value],
    selectedKeys: [...selectedKeysModel.value],
    scrollLeft: el?.scrollLeft || 0,
    scrollTop: el?.scrollTop || 0,
    zoom: graphZoom.value
  })

  viewRootKey.value = k
  selectedKeysModel.value = [k]

  const n = nodeMap.value.get(k)
  const treeNode = treeIndex.value.get(k)
  const expandable = !!(treeNode && Array.isArray(treeNode.children) && treeNode.children.length)
  expandedKeysModel.value = (n?.isDir || expandable) ? [k] : []

  nextTick(() => {
    centerOnKey(k)
    console.info('[KBGraphEditor] enter subgraph', { key: k, stack: viewRootStack.value.length })
  })
}

const exitSubGraph = () => {
  const prev = viewRootStack.value.pop()
  if (!prev) return

  viewRootKey.value = prev.rootKey === String(props.rootKey) ? '' : String(prev.rootKey || '')
  expandedKeysModel.value = Array.isArray(prev.expandedKeys) ? prev.expandedKeys : []
  selectedKeysModel.value = Array.isArray(prev.selectedKeys) ? prev.selectedKeys : [graphRootKey.value]
  graphZoom.value = Number(prev.zoom || 1)

  nextTick(() => {
    const el = graphCanvasRef.value
    if (el) {
      el.scrollLeft = Number(prev.scrollLeft || 0)
      el.scrollTop = Number(prev.scrollTop || 0)
    }
    console.info('[KBGraphEditor] exit subgraph', { rootKey: graphRootKey.value, stack: viewRootStack.value.length })
  })
}

const selectGraphNode = (key) => {
  const k = String(key || graphRootKey.value)
  const prevSelected = selectedKey.value
  selectedKeysModel.value = [k]

  const n = nodeMap.value.get(k)
  const treeNode = treeIndex.value.get(k)
  const expandable = !!(treeNode && Array.isArray(treeNode.children) && treeNode.children.length)

  if (n && n.kind !== 'paragraph' && (n.isDir || expandable)) {
    if (prevSelected === k) toggleDirExpanded(k)
    else ensureExpandedToKey(k)
  }

  nextTick(() => centerOnKey(k))
}

const handleSelectFromTree = (keys) => {
  const k = Array.isArray(keys) ? String(keys[0] || props.rootKey) : props.rootKey
  selectedKeysModel.value = [k]
  ensureExpandedToKey(k)
  nextTick(() => centerOnKey(k))
}

// 点击空白：取消选中（隐藏节点操作按钮/右侧节点信息）；拖拽平移结束后短时间内的 click 会被忽略
const handleCanvasClick = () => {
  if (!props.treeData.length) return

  const now = Date.now()
  if (now - Number(lastPanMovedAt.value || 0) < 120) {
    console.info('[KBGraphEditor] canvas click ignored (just panned)')
    return
  }

  if (!selectedKeysModel.value?.length) return

  cancelInlineEdit()
  selectedKeysModel.value = []
  console.info('[KBGraphEditor] canvas click clear selection')
}

watch(() => selectedKey.value, (k) => {
  if (!k) return
  nextTick(() => centerOnKey(k))
})

const canOperate = computed(() => !!props.selectedKbName && !!normalizeBase(props.kbLocation))
const canAdd = computed(() => canOperate.value && !!selectedNode.value && selectedNode.value.kind === 'dir')
const canRename = computed(() => canOperate.value && !!selectedNode.value && selectedKey.value !== props.rootKey && selectedNode.value.kind !== 'paragraph')
const canDelete = computed(() => canOperate.value && !!selectedNode.value && selectedKey.value !== props.rootKey && selectedNode.value.kind !== 'paragraph')

const createTypeVisible = ref(false)

const openCreateTypeModal = () => {
  if (!canAdd.value) return
  createTypeVisible.value = true
  console.info('[KBGraphEditor] open create type modal', { selectedKey: selectedKey.value })
}

const chooseCreateType = (kind) => {
  createTypeVisible.value = false
  openAddModal(kind)
}

const graphModalVisible = ref(false)
const graphModalKind = ref('') // 'file' | 'folder'
const graphModalName = ref('')

const graphModalTitle = computed(() => {
  return graphModalKind.value === 'folder' ? '新建分类' : '新建知识点'
})

const graphModalPlaceholder = computed(() => {
  return graphModalKind.value === 'folder' ? '例如：分类名' : '例如：知识点'
})

const openAddModal = (kind) => {
  if (!canAdd.value) return
  graphModalKind.value = kind
  graphModalName.value = ''
  graphModalVisible.value = true
}

const parentRelForAdd = computed(() => {
  const n = selectedNode.value
  if (!n) return ''
  if (selectedKey.value === props.rootKey) return ''
  if (n.isDir) return n.relPath
  const parentKey = n.parentKey
  const parent = parentKey ? nodeMap.value.get(parentKey) : null
  return String(parent?.relPath || '')
})

const safeName = (v) => {
  const s = String(v || '').trim()
  if (!s) return ''
  if (s.includes('/') || s.includes('\\')) return ''
  return s
}

const normalizeKnowledgePointFileName = (raw) => {
  const name = safeName(raw)
  if (!name) return { ok: false, msg: '名称不合法（不能为空，且不能包含 / 或 \\）' }

  const lower = name.toLowerCase()
  if (lower.endsWith('.md')) return { ok: true, name }

  if (name.includes('.')) {
    return { ok: false, msg: '知识点仅支持 .md 后缀；请输入不带后缀的名称，程序会自动补齐 .md' }
  }

  return { ok: true, name: `${name}.md` }
}

const confirmGraphModal = async () => {
  let name = safeName(graphModalName.value)

  if (graphModalKind.value === 'file') {
    const r = normalizeKnowledgePointFileName(name)
    if (!r.ok) {
      message.error(r.msg)
      return
    }
    name = r.name
  }

  if (!name) {
    message.error('名称不合法（不能为空，且不能包含 / 或 \\）')
    return
  }

  const parentRel = parentRelForAdd.value
  const nextRel = parentRel ? `${parentRel}/${name}` : name
  const nextAbs = toAbsPath(nextRel)

  if (!nextAbs) {
    message.error('知识库路径为空，无法创建')
    return
  }

  try {
    console.info('[KBGraphEditor] create', { kind: graphModalKind.value, nextRel, nextAbs })
    if (graphModalKind.value === 'folder') {
      await invoke('create_directory', { path: nextAbs })
    } else {
      await invoke('create_file_with_content', { path: nextAbs, content: '' })
    }

    message.success('已创建')
    graphModalVisible.value = false
    emit('reload', { selectKey: nextRel })
  } catch (e) {
    console.error('[KBGraphEditor] create failed:', e)
    message.error(String(e?.message || '创建失败'))
  }
}

const applyRename = async () => {
  if (!canRename.value) return false

  const n = selectedNode.value
  let rawName = safeName(inspectorDraft.value?.name)
  if (!rawName) {
    message.error('名称不合法（不能为空，且不能包含 / 或 \\）')
    return false
  }

  const oldRel = n.relPath
  const oldAbs = n.absPath
  let nextName = rawName

  if (!n.isDir) {
    const r = normalizeKnowledgePointFileName(nextName)
    if (!r.ok) {
      message.error(r.msg)
      return false
    }
    nextName = r.name
  }

  const parentRel = n.parentKey ? String(nodeMap.value.get(n.parentKey)?.relPath || '') : ''
  const nextRel = parentRel ? `${parentRel}/${nextName}` : nextName
  const nextAbs = toAbsPath(nextRel)

  try {
    console.info('[KBGraphEditor] rename', { oldRel, nextRel, oldAbs, nextAbs })
    await invoke('rename_path', { oldPath: oldAbs, newPath: nextAbs })
    message.success('已重命名')
    emit('patch-tree', { oldKey: oldRel, newKey: nextRel, isDir: n.isDir })
    return true
  } catch (e) {
    console.error('[KBGraphEditor] rename failed:', e)
    message.error(String(e?.message || '重命名失败'))
    return false
  }
}

const editingKey = ref('')
const editingValue = ref('')

const startInlineEdit = (key) => {
  const k = String(key || selectedKey.value || props.rootKey)
  if (k === props.rootKey) return

  const n = nodeMap.value.get(k)
  if (!n) return
  if (!canOperate.value) return

  selectedKeysModel.value = [k]
  editingKey.value = k
  editingValue.value = n.isDir ? String(n.name || '') : stripMdExt(n.name)

  console.info('[KBGraphEditor] inline edit start', { key: k })
  nextTick(() => {
    try {
      const el = document.querySelector(`[data-edit-key="${CSS.escape(k)}"]`)
      el?.focus?.()
      el?.select?.()
    } catch (e) {
      console.warn('[KBGraphEditor] inline edit focus failed', e)
    }
  })
}

const confirmInlineEdit = async () => {
  const k = String(editingKey.value || '')
  if (!k) return

  const v = String(editingValue.value || '').trim()
  inspectorDraft.value = { ...inspectorDraft.value, name: v }
  selectedKeysModel.value = [k]

  const ok = await applyRename()
  if (ok) {
    console.info('[KBGraphEditor] inline edit ok', { key: k })
    editingKey.value = ''
    editingValue.value = ''
  }
}

const cancelInlineEdit = () => {
  if (!editingKey.value) return
  console.info('[KBGraphEditor] inline edit cancel', { key: editingKey.value })
  editingKey.value = ''
  editingValue.value = ''
}

watch(() => selectedKey.value, (k) => {
  if (editingKey.value && editingKey.value !== String(k || '')) cancelInlineEdit()
})

const deleteSelectedNode = async () => {
  if (!canDelete.value) return
  const n = selectedNode.value
  const ok = await confirmDialog(
    `确定删除${n.isDir ? '目录' : '文件'} “${n.name}” 吗？\n\n将同步删除文件系统中的对应路径，此操作不可恢复。`,
    { title: '删除', kind: 'warning', okLabel: '删除', cancelLabel: '取消' }
  )
  if (!ok) return

  try {
    console.info('[KBGraphEditor] delete', { rel: n.relPath, abs: n.absPath })
    await invoke('delete_path', { path: n.absPath })
    message.success('已删除')

    const parentKey = n.parentKey || props.rootKey
    const parent = nodeMap.value.get(parentKey)
    const nextKey = parent?.key || props.rootKey

    emit('reload', { selectKey: nextKey === props.rootKey ? props.rootKey : parent.relPath })
  } catch (e) {
    console.error('[KBGraphEditor] delete failed:', e)
    message.error(String(e?.message || '删除失败'))
  }
}
</script>

<style scoped>
.graph-page {
  height: 100%;
  min-height: 0;
  width: 100%;
  display: flex;
  gap: 0;
  overflow: hidden;
}

.splitter {
  position: relative;
  width: 6px;
  flex: 0 0 6px;
  background: transparent;
}

.splitter-hit {
  position: absolute;
  inset: 0;
  cursor: col-resize;
  background: var(--guaxi-border);
  opacity: 0.55;
}

.splitter-hit:hover {
  opacity: 0.9;
}

.graph-left,
.graph-right,
.graph-center {
  min-height: 0;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
  overflow: hidden;
}

.graph-left {
  flex: 0 0 auto;
  min-width: 220px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  box-sizing: border-box;
}

.graph-left-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 2px 0;
}

.graph-center {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.graph-toolbar {
  flex: 0 0 44px;
  height: 44px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 0 12px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  overflow: hidden;
}

.graph-hint {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 12px;
  opacity: 0.8;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.graph-toolbar :deep(.n-space) {
  flex-wrap: nowrap;
  overflow: hidden;
}

.graph-zoom {
  font-size: 12px;
  opacity: 0.8;
  width: 48px;
  text-align: center;
}

.toolbar-icon {
  cursor: pointer;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  transition: transform 0.12s ease, box-shadow 0.18s ease, background 0.18s ease, color 0.18s ease;
}

.toolbar-icon:hover {
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--guaxi-header-bg) 88%, #21bdec 12%);
  box-shadow: 0 6px 16px rgba(7, 118, 161, 0.14);
  transform: translateY(-1px);
}

.graph-canvas {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: var(--guaxi-content-bg);
  padding: 12px;
  box-sizing: border-box;
  cursor: grab;
}

.graph-canvas.panning {
  cursor: grabbing;
}

.graph-stage-outer {
  position: relative;
}

.graph-stage-inner {
  position: absolute;
  left: 0;
  top: 0;
}

.graph-svg {
  display: block;
}

.edge {
  fill: none;
  stroke: color-mix(in srgb, rgba(33, 189, 236, 0.40) 70%, var(--guaxi-border) 30%);
  stroke-width: 2;
}

.node {
  cursor: pointer;
}

.node-rect {
  fill: color-mix(in srgb, var(--guaxi-panel-bg) 94%, #21bdec 6%);
  stroke: rgba(33, 189, 236, 0.34);
  stroke-width: 1.5;
  transition: fill 140ms ease, stroke 140ms ease, filter 160ms ease;
}

.node:hover .node-rect {
  stroke: rgba(33, 189, 236, 0.78);
  filter: drop-shadow(0 10px 20px rgba(7, 118, 161, 0.14));
}

.node:hover .node-text {
  opacity: 1;
}

.node.dir .node-rect {
  fill: color-mix(in srgb, var(--guaxi-panel-bg) 90%, #18a058 10%);
  stroke: rgba(24, 160, 88, 0.46);
}

.node.file .node-rect {
  fill: color-mix(in srgb, var(--guaxi-panel-bg) 92%, #21bdec 8%);
  stroke: rgba(33, 189, 236, 0.38);
}

.node.paragraph .node-rect {
  fill: color-mix(in srgb, var(--guaxi-panel-bg) 90%, #b37feb 10%);
  stroke: rgba(179, 127, 235, 0.58);
  stroke-dasharray: 4 3;
}

.node.selected .node-rect {
  stroke: rgba(33, 189, 236, 0.90);
  stroke-width: 2.5;
  filter: drop-shadow(0 6px 14px rgba(7, 118, 161, 0.16));
}

.node-text {
  font-size: 14px;
  fill: var(--guaxi-text-primary);
  opacity: 0.9;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

.inline-edit-wrap {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
}

.inline-edit-input {
  width: 100%;
  height: 28px;
  box-sizing: border-box;
  border-radius: 8px;
  padding: 0 10px;
  background: var(--guaxi-content-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  outline: none;
  font-size: 13px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

.inline-edit-input:focus {
  border-color: rgba(33, 189, 236, 0.85);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.18);
}

.create-type-hint {
  font-size: 12px;
  color: var(--guaxi-text-primary);
  opacity: 0.85;
  padding: 2px 0 10px;
}

.node-view,
.node-plus,
.node-rename,
.node-delete {
  cursor: pointer;
}

.node-view-rect {
  fill: var(--guaxi-panel-bg);
  stroke: rgba(33, 189, 236, 0.68);
  stroke-width: 1.5;
}

.node-view:hover .node-view-rect {
  filter: drop-shadow(0 6px 14px rgba(7, 118, 161, 0.16));
}

.node-view-text {
  font-size: 13px;
  text-anchor: middle;
  fill: var(--guaxi-text-primary);
}

.node-plus-rect {
  fill: var(--guaxi-panel-bg);
  stroke: rgba(33, 189, 236, 0.68);
  stroke-width: 1.5;
}

.node-plus:hover .node-plus-rect {
  filter: drop-shadow(0 6px 14px rgba(7, 118, 161, 0.16));
}

.node-plus-text {
  font-size: 14px;
  text-anchor: middle;
  fill: var(--guaxi-text-primary);
}

.node-subgraph-rect {
  fill: var(--guaxi-panel-bg);
  stroke: rgba(179, 127, 235, 0.64);
  stroke-width: 1.5;
}

.node-subgraph:hover .node-subgraph-rect {
  filter: drop-shadow(0 6px 14px rgba(179, 127, 235, 0.14));
}

.node-subgraph-text {
  font-size: 12px;
  text-anchor: middle;
  fill: var(--guaxi-text-primary);
}

.node-rename-rect {
  fill: var(--guaxi-panel-bg);
  stroke: color-mix(in srgb, var(--guaxi-text-primary) 40%, transparent);
  stroke-width: 1.5;
}

.node-rename:hover .node-rename-rect {
  filter: drop-shadow(0 6px 14px rgba(0, 0, 0, 0.12));
}

.node-rename-text {
  font-size: 14px;
  text-anchor: middle;
  fill: var(--guaxi-text-primary);
}

.node-delete-rect {
  fill: var(--guaxi-panel-bg);
  stroke: rgba(255, 107, 107, 0.70);
  stroke-width: 1.5;
}

.node-delete:hover .node-delete-rect {
  filter: drop-shadow(0 6px 14px rgba(255, 107, 107, 0.16));
}

.node-delete-text {
  font-size: 14px;
  text-anchor: middle;
  fill: #ff6b6b;
}

.graph-right {
  flex: 0 0 auto;
  min-width: 260px;
  display: flex;
  flex-direction: column;
}

.right-sider-content {
  flex: 1 1 auto;
  height: 100%;
  width: 100%;
  overflow: auto;
}

.panel {
  padding: 8px;
}

.search-results {
  max-height: 380px;
  overflow: auto;
}

.node-content-wrap {
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: var(--guaxi-content-bg);
  max-height: 220px;
  overflow: auto;
}

.node-content-loading {
  font-size: 14px;
  opacity: 0.75;
}

.node-content-pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 14px;
  line-height: 1.7;
  color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

.node-expanded-fo {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
}

.node-expanded-title {
  font-size: 13px;
  font-weight: 700;
  opacity: 0.9;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.node-expanded-loading {
  font-size: 14px;
  opacity: 0.75;
}

.node-expanded-pre {
  margin: 0;
  flex: 1 1 auto;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 14px;
  line-height: 1.7;
  color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

.empty {
  font-size: 12px;
  opacity: 0.75;
  padding: 10px 10px;
  border-radius: 10px;
  border: 1px dashed var(--guaxi-border);
  background: var(--guaxi-header-bg);
}
</style>