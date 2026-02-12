<template>
  <div class="tree-view" @contextmenu.prevent>
    <div v-if="visibleItems.length === 0" class="empty">
      {{ emptyText }}
    </div>

    <div v-else class="list">
      <div v-for="item in visibleItems" :key="item.key" class="row"
        :class="{ selected: item.key === selectedKey, dir: item.isDir }"
        :style="{ paddingLeft: `${basePadding + item.depth * indent}px` }" @click="onRowClick(item)">
        <span class="twisty" @click.stop="onTwistyClick(item)">
          <n-icon v-if="item.isDir" :component="item.expanded ? ChevronDownOutline : ChevronForwardOutline" />
          <span v-else class="twisty-placeholder"></span>
        </span>

        <span class="icon">
          <n-icon :component="item.isDir ? FolderOutline : DocumentTextOutline" />
        </span>

        <span class="label" :title="item.label">{{ item.label }}</span>

        <span v-if="showAction && !item.isDir" class="action">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-button size="tiny" quaternary circle @click.stop="onActionClick(item)">
                <template #icon>
                  <n-icon :component="actionIcon" />
                </template>
              </n-button>
            </template>
            {{ actionTitle }}
          </n-tooltip>
        </span>

        <span v-if="loadingKeys.has(item.key)" class="loading" title="加载中">
          <span class="dot"></span>
          <span class="dot"></span>
          <span class="dot"></span>
        </span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { NButton, NIcon, NTooltip } from 'naive-ui'
import {
  AddOutline,
  ChevronDownOutline,
  ChevronForwardOutline,
  DocumentTextOutline,
  FolderOutline
} from '@vicons/ionicons5'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiSlidevTreeView')

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  selectedKey: { type: String, default: '' },
  expandedKeys: { type: Array, default: () => [] },
  indent: { type: Number, default: 14 },
  basePadding: { type: Number, default: 10 },
  emptyText: { type: String, default: '暂无数据' },
  loadChildren: { type: Function, default: null },
  showAction: { type: Boolean, default: false },
  actionTitle: { type: String, default: '插入到PPT' },
  actionIcon: { type: Object, default: () => AddOutline },
})

const emit = defineEmits(['update:selectedKey', 'update:expandedKeys', 'select', 'action'])

const normalizeKey = (k) => String(k || '')

const expandedSet = computed(() => new Set((Array.isArray(props.expandedKeys) ? props.expandedKeys : []).map(normalizeKey)))

const setExpanded = (key, on) => {
  const k = normalizeKey(key)
  const next = new Set(expandedSet.value)
  if (on) next.add(k)
  else next.delete(k)
  emit('update:expandedKeys', Array.from(next))
}

const loadingKeys = ref(new Set())

const visibleItems = computed(() => {
  const out = []
  const walk = (list, depth) => {
    const arr = Array.isArray(list) ? list : []
    for (const n of arr) {
      if (!n) continue
      const key = normalizeKey(n.key)
      const label = String(n.label ?? '')
      const isDir = !!n.isDir
      const expanded = isDir ? expandedSet.value.has(key) : false

      out.push({
        key,
        label,
        depth,
        isDir,
        expanded,
        node: n,
      })

      if (isDir && expanded) {
        walk(n.children, depth + 1)
      }
    }
  }
  walk(props.nodes, 0)
  return out
})

const selectKey = (key, node) => {
  const k = normalizeKey(key)
  emit('update:selectedKey', k)
  emit('select', node)
  logger.info('[select]', { key: k, isDir: !!node?.isDir })
}

const ensureChildrenLoaded = async (n) => {
  if (!n?.isDir) return
  const k = normalizeKey(n.key)
  const hasChildren = Array.isArray(n.children) && n.children.length > 0
  if (hasChildren) return
  if (typeof props.loadChildren !== 'function') return
  if (loadingKeys.value.has(k)) return

  const next = new Set(loadingKeys.value)
  next.add(k)
  loadingKeys.value = next

  const start = performance.now()
  try {
    logger.info('[load] start', { key: k })
    await props.loadChildren(n)
    logger.info('[load] done', { key: k, costMs: Math.round(performance.now() - start) })
  } catch (e) {
    logger.error('[load] failed', { key: k, err: e })
  } finally {
    const endSet = new Set(loadingKeys.value)
    endSet.delete(k)
    loadingKeys.value = endSet
  }
}

const toggleDir = async (n) => {
  if (!n?.isDir) return
  const k = normalizeKey(n.key)
  const next = !expandedSet.value.has(k)
  setExpanded(k, next)
  if (next) await ensureChildrenLoaded(n)
}

const onRowClick = async (item) => {
  selectKey(item.key, item.node)
  if (item.isDir) {
    await toggleDir(item.node)
  }
}

const onTwistyClick = async (item) => {
  if (!item.isDir) return
  await toggleDir(item.node)
}

const onActionClick = (item) => {
  emit('action', item.node)
  logger.info('[action]', { key: item?.key })
}
</script>

<style scoped>
.tree-view {
  height: 100%;
  width: 100%;
  overflow: auto;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
  font-size: 13px;
}

.empty {
  height: 100%;
  min-height: 120px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary, #8a8f98);
  user-select: none;
}

.list {
  padding: 6px 0;
}

.row {
  height: 28px;
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
}

.row:hover {
  background: rgba(128, 128, 128, 0.08);
}

.row.selected {
  background: rgba(24, 144, 255, 0.16);
}

.twisty {
  width: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
}

.twisty-placeholder {
  width: 16px;
  height: 16px;
  display: inline-block;
}

.icon {
  width: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary, #8a8f98);
}

.label {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.action {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  margin-right: 6px;
}

.loading {
  display: inline-flex;
  gap: 4px;
  padding-right: 10px;
}

.loading .dot {
  width: 4px;
  height: 4px;
  border-radius: 999px;
  background: var(--guaxi-text-secondary, #8a8f98);
  opacity: 0.7;
  animation: blink 1.05s infinite;
}

.loading .dot:nth-child(2) {
  animation-delay: 0.15s;
}

.loading .dot:nth-child(3) {
  animation-delay: 0.3s;
}

@keyframes blink {

  0%,
  80%,
  100% {
    transform: translateY(0);
    opacity: 0.35;
  }

  40% {
    transform: translateY(-2px);
    opacity: 0.95;
  }
}
</style>