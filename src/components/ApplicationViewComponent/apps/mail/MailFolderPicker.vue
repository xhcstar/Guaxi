<template>
  <div
    ref="rootRef"
    class="mail-folder-picker"
    @keydown.esc.stop.prevent="close"
  >
    <!-- 触发器看起来像一个下拉框，点开的是悬浮的文件夹树：
         以前用 window.prompt 让用户输入文件夹名，既选不到已有文件夹，
         也和应用里其它地方的选择流程完全不一样。 -->
    <button
      type="button"
      class="mail-folder-trigger"
      :class="{ 'is-open': open }"
      :title="title"
      @click="toggle"
    >
      <span class="mail-folder-trigger-text" :class="{ 'is-empty': !currentLabel }">
        {{ currentLabel || placeholder }}
      </span>
      <span class="mail-folder-trigger-caret" aria-hidden="true">▾</span>
    </button>

    <div
      v-if="open"
      class="mail-folder-panel"
      :class="{ 'is-up': dropUp }"
      role="dialog"
      aria-label="选择文件夹"
    >
      <div class="mail-folder-panel-head">
        <span>{{ accountLabel || '选择文件夹' }}</span>
        <button class="mail-folder-refresh" title="从服务器刷新文件夹" @click="$emit('refresh')">
          ⟳
        </button>
      </div>

      <div class="mail-folder-tree" role="tree">
        <div
          v-if="allowClear"
          class="mail-folder-node"
          :class="{ 'is-on': draft === '' }"
          role="treeitem"
          @click="draft = ''"
          @dblclick="confirm('')"
        >
          <span class="mail-folder-caret is-leaf"></span>
          <span class="mail-folder-name">不移动</span>
        </div>
        <div
          v-for="row in visibleRows"
          :key="row.path"
          class="mail-folder-node"
          :class="{ 'is-on': draft === row.path, 'is-group': row.children.length > 0 }"
          :style="{ paddingLeft: `${6 + row.depth * 15}px` }"
          role="treeitem"
          :aria-expanded="row.children.length ? String(expanded.has(row.path)) : undefined"
          @click="draft = row.path"
          @dblclick="confirm(row.path)"
        >
          <span
            class="mail-folder-caret"
            :class="{ 'is-leaf': !row.children.length }"
            :aria-label="expanded.has(row.path) ? '折叠' : '展开'"
            @click.stop="toggleExpand(row.path)"
          >
            {{ row.children.length ? (expanded.has(row.path) ? '▾' : '▸') : '' }}
          </span>
          <span class="mail-folder-icon" aria-hidden="true">{{ iconFor(row.folder) }}</span>
          <span class="mail-folder-name" :title="row.folder?.name || row.path">{{ row.label }}</span>
          <span v-if="row.unread" class="mail-folder-count">{{ row.unread }}</span>
        </div>
        <div v-if="!visibleRows.length" class="mail-folder-empty">这个账户还没有文件夹</div>
      </div>

      <!-- 新建：面板内联输入，不再弹系统对话框 -->
      <div v-if="creating" class="mail-folder-create">
        <input
          ref="createInputRef"
          v-model="newName"
          class="mail-input"
          placeholder="新文件夹名，回车创建"
          @keydown.enter.prevent="commitCreate"
          @keydown.esc.prevent="cancelCreate"
        />
        <button
          class="mail-btn is-primary"
          :disabled="!newName.trim() || busy"
          @click="commitCreate"
        >
          {{ busy ? '创建中…' : '创建' }}
        </button>
        <button class="mail-btn" @click="cancelCreate">取消</button>
      </div>
      <p v-if="createError" class="mail-folder-create-error">{{ createError }}</p>

      <div class="mail-folder-actions">
        <button class="mail-btn is-primary" :disabled="!canConfirm" @click="confirm(draft)">
          确定
        </button>
        <button class="mail-btn" @click="close">取消</button>
        <button class="mail-btn" :disabled="creating" @click="startCreate">新建(N)…</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  /** 当前账户的文件夹表（由父组件按账户取好，跨账户不混列）。 */
  folders: { type: Array, default: () => [] },
  /** 选中的文件夹名（完整路径）；'' 表示「不移动」（allowClear 时）。 */
  modelValue: { type: String, default: '' },
  accountLabel: { type: String, default: '' },
  placeholder: { type: String, default: '选择文件夹…' },
  title: { type: String, default: '' },
  /** 是否允许「不移动」这一项。 */
  allowClear: { type: Boolean, default: true },
  /** 新建文件夹：async (name) => folder | null。 */
  createFolder: { type: Function, default: null },
})
const emit = defineEmits(['update:modelValue', 'refresh'])

const rootRef = ref(null)
const createInputRef = ref(null)
const open = ref(false)
/** 面板朝上还是朝下开：字段靠近弹层底部时向下会被裁掉一截。 */
const dropUp = ref(false)
const draft = ref(props.modelValue || '')
const creating = ref(false)
const newName = ref('')
const busy = ref(false)
/** 建夹失败就地显示，不弹系统对话框（重名 / 服务器拒绝时用户要能直接改名重试）。 */
const createError = ref('')
const expanded = ref(new Set())

const ROLE_LABELS = {
  inbox: '收件箱',
  drafts: '草稿',
  sent: '已发送',
  archive: '存档',
  junk: '垃圾邮件',
  trash: '已删除',
}
const ROLE_ICONS = {
  inbox: '▣',
  drafts: '✎',
  sent: '↗',
  archive: '▤',
  junk: '⚠',
  trash: '🗑',
  outbox: '◷',
}

function iconFor(folder) {
  return ROLE_ICONS[folder?.role] || '▢'
}

/** 显示名：标准角色给中文，自建文件夹给层级里的最后一段。 */
function labelFor(folder, fallback) {
  if (!folder) return fallback
  const role = ROLE_LABELS[folder.role]
  const name = folder.displayName || folder.name || fallback
  if (!role || role === name) return name
  return `${role}（${name}）`
}

// ---------------- 文件夹树 ----------------

const ROLE_ORDER = ['inbox', 'archive', 'sent', 'drafts', 'junk', 'trash', 'custom']

function sortNodes(nodes) {
  nodes.sort((a, b) => {
    const ai = a.folder ? ROLE_ORDER.indexOf(a.folder.role) : 90
    const bi = b.folder ? ROLE_ORDER.indexOf(b.folder.role) : 90
    if (ai !== bi) return (ai < 0 ? 99 : ai) - (bi < 0 ? 99 : bi)
    return String(a.label).localeCompare(String(b.label), 'zh-CN')
  })
  for (const node of nodes) sortNodes(node.children)
}

const tree = computed(() => {
  const byPath = new Map()
  const roots = []
  for (const folder of props.folders) {
    const delimiter = folder.delimiter || '/'
    const parts = String(folder.name || '')
      .split(delimiter)
      .map((part) => part.trim())
      .filter(Boolean)
    if (!parts.length) continue
    let siblings = roots
    let path = ''
    parts.forEach((part, index) => {
      path = index ? `${path}${delimiter}${part}` : part
      let node = byPath.get(path)
      if (!node) {
        node = { path, label: part, folder: null, children: [] }
        byPath.set(path, node)
        siblings.push(node)
      }
      if (index === parts.length - 1) {
        node.folder = folder
        node.label = labelFor(folder, part)
      }
      siblings = node.children
    })
  }
  sortNodes(roots)
  return roots
})

/** 展开状态：默认全部展开（层级通常只有一层，收起反而要多点几下）。 */
const flatRows = computed(() => {
  const out = []
  const walk = (nodes, depth) => {
    for (const node of nodes) {
      out.push({
        path: node.path,
        label: node.label,
        folder: node.folder,
        depth,
        unread: Number(node.folder?.unread) || 0,
        children: node.children,
      })
      if (node.children.length && expanded.value.has(node.path)) walk(node.children, depth + 1)
    }
  }
  walk(tree.value, 0)
  return out
})

const visibleRows = computed(() => flatRows.value)

/** 没选任何文件夹时不给「确定」（除非允许「不移动」）。 */
const canConfirm = computed(() => props.allowClear || Boolean(draft.value))

const selectedRow = computed(() =>
  flatRows.value.find((row) => row.path === props.modelValue) || null,
)
const currentLabel = computed(() => {
  if (!props.modelValue) return ''
  return selectedRow.value?.label || props.modelValue
})

function toggleExpand(path) {
  const next = new Set(expanded.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  expanded.value = next
}

function openPanel() {
  open.value = true
  draft.value = props.modelValue || ''
  // 默认展开所有有子节点的层，选中项的祖先也要展开（否则看不到高亮）。
  const next = new Set(expanded.value)
  const walk = (nodes) => {
    for (const node of nodes) {
      if (node.children.length) next.add(node.path)
      walk(node.children)
    }
  }
  walk(tree.value)
  expanded.value = next
  // 字段离弹层底部太近时朝上开，否则面板会被滚动容器裁掉半截。
  const box = rootRef.value?.getBoundingClientRect?.()
  const host = rootRef.value?.closest?.('.mail-modal-body, .mail-modal')
  const hostBox = host?.getBoundingClientRect?.()
  dropUp.value = Boolean(box && hostBox && hostBox.bottom - box.bottom < 340)
  emit('refresh')
}

function toggle() {
  if (open.value) close()
  else openPanel()
}

function close() {
  open.value = false
  creating.value = false
  newName.value = ''
  createError.value = ''
}

function confirm(path) {
  const value = path === null || path === undefined ? '' : path
  if (value !== '' || props.allowClear) {
    emit('update:modelValue', value)
  }
  close()
}

async function startCreate() {
  creating.value = true
  newName.value = ''
  createError.value = ''
  await nextTick()
  createInputRef.value?.focus()
}

function cancelCreate() {
  creating.value = false
  newName.value = ''
  createError.value = ''
}

async function commitCreate() {
  const name = newName.value.trim()
  if (!name || busy.value) return
  busy.value = true
  createError.value = ''
  try {
    const created = await props.createFolder?.(name)
    const path = created?.name || name
    draft.value = path
    emit('update:modelValue', path)
    emit('refresh')
    cancelCreate()
    close()
  } catch (error) {
    // 建不出来（重名 / 服务器拒绝）：留在面板里让用户改个名字，不要静默失败。
    createError.value = String(error?.message || error)
  } finally {
    busy.value = false
  }
}

function onDocumentMouseDown(event) {
  if (!open.value) return
  if (rootRef.value?.contains?.(event.target)) return
  close()
}

onMounted(() => document.addEventListener('mousedown', onDocumentMouseDown))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocumentMouseDown))

watch(
  () => props.modelValue,
  (value) => {
    if (!open.value) draft.value = value || ''
  },
)
</script>

<style scoped>
.mail-folder-picker {
  position: relative;
  flex: 1 1 auto;
  min-width: 0;
}

.mail-folder-trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  height: 31px;
  padding: 7px 10px;
  border: 1px solid var(--mail-border);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-bg-sunken);
  color: var(--mail-text);
  font: inherit;
  font-size: 12.5px;
  text-align: left;
  cursor: pointer;
}
.mail-folder-trigger:hover,
.mail-folder-trigger.is-open {
  border-color: var(--mail-accent);
}
.mail-folder-trigger-text {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mail-folder-trigger-text.is-empty {
  color: var(--mail-text-muted);
}
.mail-folder-trigger-caret {
  flex: 0 0 auto;
  color: var(--mail-text-muted);
  font-size: 10px;
  transition: transform 0.12s ease;
}
.mail-folder-trigger.is-open .mail-folder-trigger-caret {
  transform: rotate(180deg);
}

/* 悬浮面板：贴在触发框下方，宽度至少和字段一样宽。 */
.mail-folder-panel {
  position: absolute;
  left: 0;
  top: calc(100% + 4px);
  z-index: 60;
  display: flex;
  flex-direction: column;
  width: min(320px, 92vw);
  min-width: 100%;
  border: 1px solid var(--mail-border-strong);
  border-radius: 10px;
  background: var(--mail-panel);
  box-shadow: var(--mail-shadow);
  overflow: hidden;
}

.mail-folder-panel.is-up {
  top: auto;
  bottom: calc(100% + 4px);
}

.mail-folder-panel-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 8px 7px 12px;
  border-bottom: 1px solid var(--mail-border);
  color: var(--mail-text-secondary);
  font-size: 11.5px;
}
.mail-folder-panel-head span {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mail-folder-refresh {
  flex: 0 0 auto;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--mail-text-muted);
  font: inherit;
  cursor: pointer;
}
.mail-folder-refresh:hover {
  background: var(--mail-panel-hover);
  color: var(--mail-text);
}

.mail-folder-tree {
  max-height: 260px;
  padding: 4px;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
}
.mail-folder-node {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 4px 6px;
  border-radius: 5px;
  color: var(--mail-text-secondary);
  font-size: 12.5px;
  cursor: pointer;
}
.mail-folder-node:hover {
  background: var(--mail-panel-hover);
  color: var(--mail-text);
}
.mail-folder-node.is-on {
  background: var(--mail-accent-soft);
  color: var(--mail-accent);
  font-weight: 600;
}
.mail-folder-caret {
  flex: 0 0 12px;
  color: var(--mail-text-muted);
  font-size: 9px;
  text-align: center;
}
.mail-folder-caret.is-leaf {
  /* 叶子节点也占位，名称才能对齐成一条线。 */
  cursor: default;
}
.mail-folder-icon {
  flex: 0 0 auto;
  color: var(--mail-text-muted);
  font-size: 11px;
}
.mail-folder-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mail-folder-count {
  flex: 0 0 auto;
  padding: 0 5px;
  border-radius: 999px;
  background: var(--mail-bg-sunken);
  color: var(--mail-text-muted);
  font-size: 10.5px;
}
.mail-folder-empty {
  padding: 10px 8px;
  color: var(--mail-text-muted);
  font-size: 11.5px;
}

.mail-folder-create {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border-top: 1px solid var(--mail-border);
}
.mail-folder-create .mail-input {
  flex: 1 1 auto;
  min-width: 0;
}
.mail-folder-create-error {
  margin: 0;
  padding: 0 10px 8px;
  color: var(--mail-danger);
  font-size: 12px;
}

.mail-folder-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  padding: 8px 10px;
  border-top: 1px solid var(--mail-border);
  background: var(--mail-bg-elevated);
}
</style>
