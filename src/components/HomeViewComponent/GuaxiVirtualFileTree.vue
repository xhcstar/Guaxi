<template>
  <div
    ref="rootRef"
    class="vtree"
    tabindex="0"
    @scroll="onScroll"
    @keydown="onTreeKeydown"
    @contextmenu="onBlankContext"
  >
    <div class="vtree-spacer" :style="{ height: totalHeight + 'px' }">
      <div class="vtree-window" :style="{ transform: `translateY(${offsetY}px)` }">
        <div
          v-for="item in visibleItems"
          :key="item.node.id"
          class="vtree-row"
          :data-id="item.node.id"
          :class="{
            selected: isSelected(item.node),
            'drop-target': dropTargetId === item.node.id,
            'is-dir': item.node.is_directory,
            'is-new': item.node.isNew,
            dragging: draggingId === item.node.id,
          }"
          :style="{ height: rowHeight + 'px', paddingLeft: (item.depth * indent + 8) + 'px' }"
          @click="onRowClick(item.node)"
          @contextmenu="onRowContext($event, item.node)"
          @pointerdown="onPointerDown($event, item.node)"
        >
          <span class="vtree-twisty" @click.stop="onTwistyClick(item.node)">
            <svg
              v-if="item.node.is_directory"
              class="twisty-ico"
              :class="{ open: isExpanded(item.node) }"
              viewBox="0 0 24 24"
              width="13"
              height="13"
            >
              <path d="M9 6l6 6-6 6" fill="none" stroke="currentColor" stroke-width="2.6"
                stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>

          <span class="vtree-icon" :class="item.node.is_directory ? 'icon-dir' : 'icon-file'">
            <Icon :icon="getNodeIcon(item.node)" width="18" height="18" class="vtree-type-icon" />
          </span>

          <input
            v-if="String(editingId || '') === String(item.node.id)"
            class="editable-input"
            :data-node-id="normId(item.node.id)"
            :value="editName"
            spellcheck="false"
            @input="$emit('update:editName', $event.target.value)"
            @blur="$emit('rename-commit', item.node)"
            @keydown="onEditKeydown($event)"
            @click.stop
            @pointerdown.stop
          />
          <span v-else class="vtree-label" :title="item.node.name">{{ item.node.name }}</span>
        </div>
      </div>
    </div>

    <div v-if="!flat.length" class="vtree-empty">空目录</div>

    <teleport to="body">
      <div
        v-if="dragging"
        class="vtree-ghost"
        :style="{ left: ghost.x + 14 + 'px', top: ghost.y + 10 + 'px' }"
      >
        <span class="ghost-ico">{{ ghost.isDir ? '📁' : '📄' }}</span>
        <span class="ghost-text">{{ ghost.text }}</span>
      </div>
    </teleport>
  </div>
</template>

<script setup>
import { computed, onMounted, onActivated, onBeforeUnmount, nextTick, reactive, ref } from 'vue'
import { Icon } from '@iconify/vue'
import { getFileTreeIcon } from './fileTreeIcons'

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  expandedKeys: { type: Array, default: () => [] },
  selectedKeys: { type: Array, default: () => [] },
  editingId: { type: [String, null], default: null },
  editName: { type: String, default: '' },
  rowHeight: { type: Number, default: 28 },
  indent: { type: Number, default: 15 },
  rootId: { type: String, default: '' },
})

const emit = defineEmits([
  'update:expandedKeys', 'update:selectedKeys', 'update:editName',
  'open-file', 'load-children', 'contextmenu', 'blank-contextmenu', 'rename-commit', 'rename-cancel', 'move', 'delete-node',
])

const OVERSCAN = 8
const DRAG_THRESHOLD = 5

const rootRef = ref(null)
const scrollTop = ref(0)
const viewportH = ref(0)
let resizeObs = null

const normId = (p) => {
  if (!p) return ''
  let s = String(p).replace(/\\/g, '/')
  if (s.length > 1 && s.endsWith('/')) s = s.slice(0, -1)
  return s
}
const parentOf = (id) => normId(id).split('/').slice(0, -1).join('/')
// 节点图标解析委托给共享模块；目录传入展开态以切换开/合图标。
const getNodeIcon = (node) =>
  getFileTreeIcon(
    node?.name || normId(node?.id).split('/').pop() || '',
    !!node?.is_directory,
    !!node?.is_directory && isExpanded(node),
  )

const expandedSet = computed(() => new Set((props.expandedKeys || []).map(normId)))
const selectedSet = computed(() => new Set((props.selectedKeys || []).map(normId)))
const isExpanded = (node) => expandedSet.value.has(normId(node.id))
const isSelected = (node) => selectedSet.value.has(normId(node.id))

// 扁平化可见节点(仅展开链路);防御性跳过点开头隐藏项。
const flat = computed(() => {
  const out = []
  const walk = (list, depth) => {
    if (!Array.isArray(list)) return
    for (const node of list) {
      if (!node) continue
      const name = String(node.name || '')
      if (name.startsWith('.')) continue
      out.push({ node, depth })
      if (node.is_directory && expandedSet.value.has(normId(node.id)) && Array.isArray(node.children)) {
        walk(node.children, depth + 1)
      }
    }
  }
  walk(props.nodes, 0)
  return out
})

const totalHeight = computed(() => flat.value.length * props.rowHeight)
const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / props.rowHeight) - OVERSCAN))
const endIndex = computed(() =>
  Math.min(flat.value.length, Math.ceil((scrollTop.value + viewportH.value) / props.rowHeight) + OVERSCAN),
)
const visibleItems = computed(() => flat.value.slice(startIndex.value, endIndex.value))
const offsetY = computed(() => startIndex.value * props.rowHeight)

function onScroll(e) { scrollTop.value = e.target.scrollTop }
function measure() {
  if (!rootRef.value) return
  viewportH.value = rootRef.value.clientHeight
  // keep-alive 重新激活或容器尺寸变化后，DOM 的 scrollTop 会被重置为 0，而 scrollTop ref
  // 仍保留切走前的旧值，导致虚拟窗口按旧 offsetY 下移、顶部出现一块空白（滚一下才复原）。
  // 这里把 ref 对齐到真实 DOM，使可视区间立即按当前滚动位置重新计算。
  scrollTop.value = rootRef.value.scrollTop
}

const findNodeById = (id) => {
  const k = normId(id)
  const it = flat.value.find((x) => normId(x.node.id) === k)
  return it ? it.node : null
}

// ───── 展开 / 选中 / 打开 ─────
function emitToggleExpand(node, willExpand) {
  const id = normId(node.id)
  const set = new Set((props.expandedKeys || []).map(normId))
  if (willExpand) set.add(id)
  else set.delete(id)
  emit('update:expandedKeys', Array.from(set))
}

function focusTree() {
  if (!rootRef.value || String(props.editingId || '')) return
  rootRef.value.focus({ preventScroll: true })
}

function scrollIndexIntoView(index, align = 'nearest') {
  if (!rootRef.value || index < 0) return
  const maxTop = Math.max(0, rootRef.value.scrollHeight - rootRef.value.clientHeight)
  const rowTop = index * props.rowHeight
  const rowBottom = rowTop + props.rowHeight
  const viewTop = rootRef.value.scrollTop
  const viewBottom = viewTop + viewportH.value

  let nextTop = viewTop
  if (align === 'center') {
    nextTop = rowTop - (viewportH.value / 2 - props.rowHeight / 2)
  } else if (rowTop < viewTop) {
    nextTop = rowTop
  } else if (rowBottom > viewBottom) {
    nextTop = rowBottom - viewportH.value
  }

  const clamped = Math.max(0, Math.min(maxTop, Math.floor(nextTop)))
  rootRef.value.scrollTop = clamped
  scrollTop.value = clamped
}

async function selectFlatIndex(index, reason = 'keyboard') {
  const item = flat.value[index]
  if (!item?.node) return
  emit('update:selectedKeys', [item.node.id])
  scrollIndexIntoView(index, 'nearest')
  await nextTick()
  scrollIndexIntoView(index, 'nearest')
}

const selectedFlatIndex = () => {
  const selected = normId((props.selectedKeys || [])[0])
  if (!selected) return -1
  return flat.value.findIndex((it) => normId(it.node.id) === selected)
}

function parentVisibleIndex(index) {
  const current = flat.value[index]
  if (!current || current.depth <= 0) return -1
  for (let i = index - 1; i >= 0; i -= 1) {
    if (flat.value[i]?.depth === current.depth - 1) return i
  }
  return -1
}

async function onTreeKeydown(e) {
  if (String(props.editingId || '')) return
  const target = e.target
  if (target && ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)) return
  if (!flat.value.length) return

  const currentIndex = selectedFlatIndex()
  const currentNode = currentIndex >= 0 ? flat.value[currentIndex]?.node : null

  if (e.key === 'ArrowDown') {
    e.preventDefault()
    await selectFlatIndex(currentIndex < 0 ? 0 : Math.min(flat.value.length - 1, currentIndex + 1), 'arrow-down')
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    await selectFlatIndex(currentIndex < 0 ? flat.value.length - 1 : Math.max(0, currentIndex - 1), 'arrow-up')
  } else if (e.key === 'Home') {
    e.preventDefault()
    await selectFlatIndex(0, 'home')
  } else if (e.key === 'End') {
    e.preventDefault()
    await selectFlatIndex(flat.value.length - 1, 'end')
  } else if (e.key === 'PageDown') {
    e.preventDefault()
    const step = Math.max(1, Math.floor(viewportH.value / props.rowHeight) - 1)
    await selectFlatIndex(Math.min(flat.value.length - 1, (currentIndex < 0 ? 0 : currentIndex) + step), 'page-down')
  } else if (e.key === 'PageUp') {
    e.preventDefault()
    const step = Math.max(1, Math.floor(viewportH.value / props.rowHeight) - 1)
    await selectFlatIndex(Math.max(0, (currentIndex < 0 ? flat.value.length - 1 : currentIndex) - step), 'page-up')
  } else if (e.key === 'ArrowRight' && currentNode?.is_directory) {
    e.preventDefault()
    if (!isExpanded(currentNode)) {
      emitToggleExpand(currentNode, true)
      emit('load-children', currentNode)
    } else if (currentIndex + 1 < flat.value.length && flat.value[currentIndex + 1]?.depth > flat.value[currentIndex]?.depth) {
      await selectFlatIndex(currentIndex + 1, 'arrow-right-child')
    }
  } else if (e.key === 'ArrowLeft' && currentNode) {
    e.preventDefault()
    if (currentNode.is_directory && isExpanded(currentNode)) {
      emitToggleExpand(currentNode, false)
    } else {
      const parentIndex = parentVisibleIndex(currentIndex)
      if (parentIndex >= 0) await selectFlatIndex(parentIndex, 'arrow-left-parent')
    }
  } else if (e.key === 'Enter' && currentNode) {
    e.preventDefault()
    if (currentNode.is_directory) toggleDir(currentNode)
    else emit('open-file', currentNode)
  } else if (e.key === 'Delete' && currentNode) {
    e.preventDefault()
    emit('delete-node', currentNode)
  }
}

function toggleDir(node) {
  const willExpand = !isExpanded(node)
  emitToggleExpand(node, willExpand)
  if (willExpand) emit('load-children', node)
}

function onRowClick(node) {
  if (suppressClick) return
  focusTree()
  emit('update:selectedKeys', [node.id])
  if (node.is_directory) toggleDir(node)
  else emit('open-file', node)
}

function onTwistyClick(node) {
  if (!node.is_directory || suppressClick) return
  focusTree()
  emit('update:selectedKeys', [node.id])
  toggleDir(node)
}

function onRowContext(e, node) {
  e.preventDefault()
  e.stopPropagation()
  emit('update:selectedKeys', [node.id])
  emit('contextmenu', { node, x: e.clientX, y: e.clientY })
}

// 文件树空白处右键：未命中任何行，按"当前仓库根目录"上下文处理
function onBlankContext(e) {
  if (e.target && e.target.closest && e.target.closest('.vtree-row')) return
  e.preventDefault()
  emit('update:selectedKeys', [])
  emit('blank-contextmenu', { x: e.clientX, y: e.clientY })
}

function onEditKeydown(e) {
  if (e.key === 'Enter') { e.preventDefault(); e.target.blur() }
  else if (e.key === 'Escape') { e.preventDefault(); emit('rename-cancel') }
}

// ───── 指针拖拽(Tauri 下原生 HTML5 DnD 会被 webview 拦截,改用 pointer 实现)─────
const dragging = ref(false)
const draggingId = ref(null)
const dropTargetId = ref(null)
const ghost = reactive({ x: 0, y: 0, text: '', isDir: false })

let startX = 0
let startY = 0
let pending = null
let moveHandler = null
let upHandler = null
let suppressClick = false

function isValidDropTarget(src, targetDir) {
  const s = normId(src)
  const t = normId(targetDir)
  if (!s || !t) return false
  if (s === t) return false
  if (t === s || t.startsWith(s + '/')) return false // 不能移入自身或子目录
  if (parentOf(s) === t) return false // 已在目标目录
  return true
}

function resolveTargetDir(clientX, clientY) {
  const el = document.elementFromPoint(clientX, clientY)
  const rowEl = el && el.closest ? el.closest('.vtree-row') : null
  if (rowEl) {
    const tn = findNodeById(rowEl.getAttribute('data-id'))
    if (!tn) return null
    return tn.is_directory ? tn.id : parentOf(tn.id)
  }
  // 落在树空白区域 → 仓库根
  if (rootRef.value && el && rootRef.value.contains(el)) return props.rootId
  return null
}

function onPointerDown(e, node) {
  if (e.button !== 0) return
  if (String(props.editingId || '') === String(node.id)) return
  pending = node
  startX = e.clientX
  startY = e.clientY
  moveHandler = onPointerMove
  upHandler = onPointerUp
  window.addEventListener('pointermove', moveHandler)
  window.addEventListener('pointerup', upHandler)
}

function onPointerMove(e) {
  if (!pending) return
  if (!dragging.value) {
    if (Math.abs(e.clientX - startX) < DRAG_THRESHOLD && Math.abs(e.clientY - startY) < DRAG_THRESHOLD) return
    dragging.value = true
    draggingId.value = pending.id
    ghost.text = pending.name
    ghost.isDir = !!pending.is_directory
    document.body.style.cursor = 'grabbing'
  }
  ghost.x = e.clientX
  ghost.y = e.clientY
  const targetDir = resolveTargetDir(e.clientX, e.clientY)
  if (targetDir && isValidDropTarget(pending.id, targetDir)) {
    // 高亮目标目录所在行(目录行本身,或文件所在的父目录行)
    const el = document.elementFromPoint(e.clientX, e.clientY)
    const rowEl = el && el.closest ? el.closest('.vtree-row') : null
    const tn = rowEl ? findNodeById(rowEl.getAttribute('data-id')) : null
    dropTargetId.value = tn ? (tn.is_directory ? tn.id : null) : null
  } else {
    dropTargetId.value = null
  }
}

function onPointerUp(e) {
  window.removeEventListener('pointermove', moveHandler)
  window.removeEventListener('pointerup', upHandler)
  if (dragging.value && pending) {
    const targetDir = resolveTargetDir(e.clientX, e.clientY)
    if (targetDir && isValidDropTarget(pending.id, targetDir)) {
      emit('move', pending.id, targetDir)
    }
    suppressClick = true
    setTimeout(() => { suppressClick = false }, 50)
  }
  dragging.value = false
  draggingId.value = null
  dropTargetId.value = null
  pending = null
  document.body.style.cursor = ''
}

// ───── 暴露:滚动到指定 key ─────
async function scrollTo(opts) {
  const key = normId(opts && opts.key)
  if (!key) return
  await nextTick()
  const idx = flat.value.findIndex((it) => normId(it.node.id) === key)
  if (idx < 0 || !rootRef.value) return
  scrollIndexIntoView(idx, 'center')
}
defineExpose({ scrollTo })

onMounted(() => {
  measure()
  if (typeof ResizeObserver !== 'undefined' && rootRef.value) {
    resizeObs = new ResizeObserver(() => measure())
    resizeObs.observe(rootRef.value)
  }
})

// 本组件常被包在 keep-alive 里（首页在应用页之间来回切换时被缓存）。重新激活时 DOM
// 会重新插入、原生 scrollTop 归零，需要再测一次视口并把滚动位置对齐，否则顶部会留白。
onActivated(() => {
  nextTick(() => measure())
})

onBeforeUnmount(() => {
  resizeObs?.disconnect?.()
  resizeObs = null
  if (moveHandler) window.removeEventListener('pointermove', moveHandler)
  if (upHandler) window.removeEventListener('pointerup', upHandler)
  document.body.style.cursor = ''
})
</script>

<style scoped>
.vtree {
  position: relative;
  height: 100%;
  width: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  padding: 0 6px 0 4px;
  box-sizing: border-box;
  /* Apple 风格细滚动条 */
  scrollbar-width: thin;
  scrollbar-color: rgba(140, 140, 150, 0.4) transparent;
}
.vtree:focus {
  outline: none;
}
.vtree::-webkit-scrollbar { width: 9px; height: 9px; }
.vtree::-webkit-scrollbar-track { background: transparent; }
.vtree::-webkit-scrollbar-thumb {
  background: rgba(140, 140, 150, 0.38);
  border-radius: 10px;
  border: 2.5px solid transparent;
  background-clip: padding-box;
}
.vtree::-webkit-scrollbar-thumb:hover {
  background: rgba(120, 120, 130, 0.6);
  background-clip: padding-box;
}

.vtree-spacer { position: relative; width: 100%; }
.vtree-window { position: absolute; top: 0; left: 0; right: 0; will-change: transform; }

.vtree-row {
  display: flex;
  align-items: center;
  gap: 5px;
  padding-right: 8px;
  margin: 0 2px;
  border-radius: 8px;
  cursor: default;
  user-select: none;
  color: var(--guaxi-text-primary, var(--n-text-color));
  transition: background-color 0.14s ease, box-shadow 0.14s ease;
  box-sizing: border-box;
}
.vtree-row:hover { background: rgba(120, 120, 128, 0.12); }
.vtree-row.selected {
  background: rgba(0, 122, 255, 0.14);
  box-shadow: inset 0 0 0 1px rgba(0, 122, 255, 0.28);
}
.vtree-row.drop-target {
  background: rgba(0, 122, 255, 0.16);
  box-shadow: inset 0 0 0 2px rgba(0, 122, 255, 0.55);
}
.vtree-row.dragging { opacity: 0.45; }
.vtree-row.is-new { font-style: italic; }

.vtree-twisty {
  width: 15px;
  height: 15px;
  flex: 0 0 15px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, rgba(150, 150, 160, 0.85));
  cursor: pointer;
}
.twisty-ico { transition: transform 0.18s cubic-bezier(0.4, 0, 0.2, 1); }
.twisty-ico.open { transform: rotate(90deg); }

.vtree-icon {
  flex: 0 0 20px;
  width: 20px;
  height: 20px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.vtree-type-icon {
  display: block;
  flex: 0 0 auto;
  pointer-events: none;
  /* 柔和投影 + 轻微提亮，让彩色图标在深浅底上都通透有质感 */
  filter: drop-shadow(0 1px 1.5px rgba(0, 0, 0, 0.18)) saturate(1.04);
  transition: transform 0.14s ease, filter 0.14s ease;
}
.vtree-row:hover .vtree-type-icon {
  transform: scale(1.06);
  filter: drop-shadow(0 2px 3px rgba(0, 0, 0, 0.22)) saturate(1.12) brightness(1.04);
}

/* 文件夹:单色描线图标,统一染成淡蓝高级色(与彩色文件图标和谐共处) */
.vtree-icon.icon-dir .vtree-type-icon {
  color: #5b9bd5;
  filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.10));
}
.vtree-row:hover .vtree-icon.icon-dir .vtree-type-icon {
  filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.14)) brightness(1.06);
}
:global(:root[data-guaxi-theme='dark']) .vtree-icon.icon-dir .vtree-type-icon {
  color: #7fb2e8;
}

.vtree-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
  line-height: 1.4;
  letter-spacing: 0.1px;
}

.editable-input {
  flex: 1;
  min-width: 0;
  border: 1px solid var(--n-color-primary, #0a84ff);
  border-radius: 6px;
  padding: 1px 6px;
  font-size: 14px;
  outline: none;
  background: var(--guaxi-panel-bg, #fff);
  color: var(--guaxi-text-primary, inherit);
  font-family: inherit;
}
.editable-input:focus { box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.25); }

.vtree-empty {
  position: absolute;
  top: 16px;
  left: 0;
  right: 0;
  text-align: center;
  font-size: 12px;
  opacity: 0.4;
  pointer-events: none;
}
</style>

<style>
/* 拖拽浮层(teleport 到 body,非 scoped)*/
.vtree-ghost {
  position: fixed;
  z-index: 99999;
  pointer-events: none;
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 240px;
  padding: 4px 10px;
  font-size: 12.5px;
  border-radius: 8px;
  color: #fff;
  background: rgba(10, 132, 255, 0.92);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(6px);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei';
}
.vtree-ghost .ghost-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
