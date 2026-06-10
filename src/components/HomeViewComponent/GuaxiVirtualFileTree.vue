<template>
  <div ref="rootRef" class="vtree" @scroll="onScroll">
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
            <svg v-if="item.node.is_directory" viewBox="0 0 24 24" width="16" height="16">
              <path
                d="M3.5 6.4c0-.95.77-1.72 1.72-1.72h3.4c.5 0 .98.22 1.3.6l.96 1.12h8c.95 0 1.72.77 1.72 1.72v8.86c0 .95-.77 1.72-1.72 1.72H5.22c-.95 0-1.72-.77-1.72-1.72V6.4z"
                fill="currentColor" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="16" height="16">
              <path
                d="M6.2 3.6h6.7L18.5 9v10.2c0 .95-.77 1.72-1.72 1.72H6.2c-.95 0-1.72-.77-1.72-1.72V5.32c0-.95.77-1.72 1.72-1.72z"
                fill="currentColor" opacity="0.92" />
              <path d="M12.7 3.8V9.2h5.2" fill="none"
                stroke="var(--vtree-file-fold, rgba(255,255,255,.5))" stroke-width="1.3" />
            </svg>
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
  </div>

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
</template>

<script setup>
import { computed, onMounted, onBeforeUnmount, nextTick, reactive, ref } from 'vue'

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
  'open-file', 'load-children', 'contextmenu', 'rename-commit', 'rename-cancel', 'move',
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
function measure() { if (rootRef.value) viewportH.value = rootRef.value.clientHeight }

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

function toggleDir(node) {
  const willExpand = !isExpanded(node)
  emitToggleExpand(node, willExpand)
  if (willExpand) emit('load-children', node)
}

function onRowClick(node) {
  if (suppressClick) return
  emit('update:selectedKeys', [node.id])
  if (node.is_directory) toggleDir(node)
  else emit('open-file', node)
}

function onTwistyClick(node) {
  if (!node.is_directory || suppressClick) return
  emit('update:selectedKeys', [node.id])
  toggleDir(node)
}

function onRowContext(e, node) {
  e.preventDefault()
  e.stopPropagation()
  emit('update:selectedKeys', [node.id])
  emit('contextmenu', { node, x: e.clientX, y: e.clientY })
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
  const target = idx * props.rowHeight - (viewportH.value / 2 - props.rowHeight / 2)
  rootRef.value.scrollTop = Math.max(0, target)
}
defineExpose({ scrollTo })

onMounted(() => {
  measure()
  if (typeof ResizeObserver !== 'undefined' && rootRef.value) {
    resizeObs = new ResizeObserver(() => measure())
    resizeObs.observe(rootRef.value)
  }
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
  padding: 4px 6px 8px 4px;
  box-sizing: border-box;
  /* Apple 风格细滚动条 */
  scrollbar-width: thin;
  scrollbar-color: rgba(140, 140, 150, 0.4) transparent;
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
  margin: 1px 2px;
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

.vtree-icon { flex: 0 0 18px; display: inline-flex; align-items: center; justify-content: center; }
.icon-dir { color: var(--n-color-primary, #3b82f6); }
.icon-file { color: var(--guaxi-text-secondary, rgba(150, 158, 170, 0.95)); }

.vtree-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  line-height: 1.4;
  letter-spacing: 0.1px;
}

.editable-input {
  flex: 1;
  min-width: 0;
  border: 1px solid var(--n-color-primary, #0a84ff);
  border-radius: 6px;
  padding: 1px 6px;
  font-size: 13px;
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
