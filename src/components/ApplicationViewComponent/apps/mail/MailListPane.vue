<template>
  <section class="mail-pane mail-list-pane">
    <div class="mail-list-head">
      <div class="mail-list-title-row">
        <h2 class="mail-list-title">{{ title }}</h2>
        <span class="mail-list-meta">
          {{ visible.length }} 封<template v-if="unreadCount"> · {{ unreadCount }} 未读</template>
        </span>
        <label class="mail-selectall">
          <input
            type="checkbox"
            :checked="allChecked"
            :indeterminate="someChecked && !allChecked"
            @change="$emit('select-all', $event.target.checked)"
          />
        </label>
        <span v-if="loading" class="mail-spin" style="margin-left: auto"></span>
      </div>

      <div class="mail-search">
        <span aria-hidden="true">{{ fieldIcon }}</span>
        <input
          :value="searchQuery"
          :placeholder="fieldPlaceholder"
          spellcheck="false"
          @input="$emit('update:search-query', $event.target.value)"
          @keyup.enter="$emit('search')"
        />
        <button v-if="searchQuery" class="mail-search-clear" title="清空" @click="$emit('clear-search')">
          ×
        </button>
      </div>

      <div v-if="selectedKeys.length" class="mail-batch-bar">
        <span class="mail-batch-count">已选 {{ selectedKeys.length }} 封</span>
        <button class="mail-chip" @click="$emit('batch', 'read')">标为已读</button>
        <button class="mail-chip" @click="$emit('batch', 'unread')">标为未读</button>
        <button class="mail-chip" @click="$emit('batch', 'flag')">加星标</button>
        <button class="mail-chip" @click="$emit('batch', 'unflag')">取消星标</button>
        <button class="mail-chip" @click="$emit('batch', 'junk')">标为垃圾邮件</button>
        <select class="mail-select" @change="onBatchMove($event)">
          <option value="">移到…</option>
          <option v-for="f in moveTargets" :key="f.name" :value="f.name">{{ f.displayName }}</option>
        </select>
        <button class="mail-chip is-danger" @click="$emit('batch', 'delete')">删除</button>
        <button class="mail-chip" @click="$emit('clear-selection')">取消选择</button>
      </div>

      <div class="mail-search-row">
        <select
          class="mail-select"
          :value="searchField"
          @change="$emit('update:search-field', $event.target.value)"
        >
          <option value="all">全部字段</option>
          <option value="subject">主题</option>
          <option value="from">发件人</option>
          <option value="to">收件人</option>
          <option value="body">正文</option>
        </select>
        <button
          class="mail-chip"
          :class="{ 'is-on': filters.unreadOnly }"
          @click="$emit('toggle-filter', 'unreadOnly')"
        >
          未读
        </button>
        <button
          class="mail-chip"
          :class="{ 'is-on': filters.starredOnly }"
          @click="$emit('toggle-filter', 'starredOnly')"
        >
          星标
        </button>
        <button
          class="mail-chip"
          :class="{ 'is-on': filters.hasAttachment }"
          @click="$emit('toggle-filter', 'hasAttachment')"
        >
          附件
        </button>
      </div>
    </div>

    <div v-if="searchError" class="mail-search-error">{{ searchError }}</div>

    <div
      ref="scrollRef"
      class="mail-list-scroll"
      role="list"
      @scroll.passive="onScroll"
    >
      <div v-if="!visible.length && !loading" class="mail-empty">
        <div class="mail-empty-title">这里没有邮件</div>
        <div v-if="searchQuery">换个关键词，或者清空搜索条件再试。</div>
        <div v-else>同步一下，或者切到别的文件夹看看。</div>
      </div>

      <!-- 虚拟滚动：只渲染视口附近的若干行。一万封邮件时若全量渲染，
           光是 DOM 节点就有十几万个，切换文件夹会卡住一秒以上。 -->
      <div v-if="virtual" class="mail-list-spacer" :style="{ height: `${totalHeight}px` }">
        <article
          v-for="item in windowRows"
          :key="rowKey(item.row)"
          class="mail-row"
          :style="{ transform: `translateY(${item.top}px)` }"
          :class="{
            'is-unread': !item.row.seen,
            'is-selected': isSelected(item.row),
            'is-active': activeKey === rowKey(item.row),
            thread: item.row.threadCount > 1,
          }"
          role="listitem"
          tabindex="0"
          :draggable="true"
          @click="onRowClick(item.row, $event)"
          @keydown.enter="$emit('open', item.row)"
          @contextmenu.prevent="onRowContextMenu(item.row, $event)"
          @dragstart="onDragStart(item.row, $event)"
          @dragend="$emit('drag-end')"
        >
          <MailRowContent
            :row="item.row"
            :checked="isChecked(item.row)"
            @toggle="(v) => $emit('toggle-select', item.row, v)"
            @star="(v) => $emit('toggle-star', item.row, v)"
            @delete="$emit('delete', item.row)"
          />
        </article>
      </div>

      <template v-else>
        <article
          v-for="row in visible"
          :key="rowKey(row)"
          class="mail-row"
          :class="{
            'is-unread': !row.seen,
            'is-selected': isSelected(row),
            'is-active': activeKey === rowKey(row),
            thread: row.threadCount > 1,
          }"
          role="listitem"
          tabindex="0"
          :draggable="true"
          @click="onRowClick(row, $event)"
          @keydown.enter="$emit('open', row)"
          @contextmenu.prevent="onRowContextMenu(row, $event)"
          @dragstart="onDragStart(row, $event)"
          @dragend="$emit('drag-end')"
        >
          <MailRowContent
            :row="row"
            :checked="isChecked(row)"
            @toggle="(v) => $emit('toggle-select', row, v)"
            @star="(v) => $emit('toggle-star', row, v)"
            @delete="$emit('delete', row)"
          />
        </article>
      </template>

      <div v-if="hasMore" class="mail-load-more">
        <button class="mail-btn" :disabled="loadingMore" @click="$emit('load-more')">
          {{ loadingMore ? '加载中…' : '加载更多' }}
        </button>
      </div>
    </div>
  </section>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import MailRowContent from './MailRowContent.vue'

const emit = defineEmits([
  'open',
  'toggle-star',
  'contextmenu',
  'update:search-query',
  'update:search-field',
  'search',
  'clear-search',
  'toggle-filter',
  'toggle-select',
  'select-all',
  'clear-selection',
  'batch',
  'drag-start',
  'drag-end',
  'load-more',
  'active-change',
  'delete',
])

const props = defineProps({
  messages: { type: Array, default: () => [] },
  selected: { type: Object, default: null },
  /** 键盘导航的「当前行」——与「已选中的批量勾选」是两件事。 */
  activeKey: { type: String, default: '' },
  title: { type: String, default: '收件箱' },
  loading: { type: Boolean, default: false },
  searchQuery: { type: String, default: '' },
  searchField: { type: String, default: 'all' },
  filters: {
    type: Object,
    default: () => ({ unreadOnly: false, starredOnly: false, hasAttachment: false }),
  },
  selectedKeys: { type: Array, default: () => [] },
  searchError: { type: String, default: '' },
  /** 拖拽时可以放下的目标文件夹。 */
  moveTargets: { type: Array, default: () => [] },
  /** 是否还有下一页。 */
  hasMore: { type: Boolean, default: false },
  loadingMore: { type: Boolean, default: false },
  /** 虚拟滚动开关（设置里可关掉，老机器或调试时有用）。 */
  virtual: { type: Boolean, default: true },
  /** 行高（px），虚拟滚动按它计算偏移。与 mail.css 里的 --mail-row-height 保持一致。 */
  rowHeight: { type: Number, default: 76 },
})
const visible = computed(() => props.messages || [])
/** 已加载邮件里的未读数：和左侧「未读」角标对照着看，避免总数/未读数被误读成同一个指标。 */
const unreadCount = computed(() => visible.value.filter((m) => !m.seen).length)

// ---------------- 虚拟滚动 ----------------

const scrollRef = ref(null)
const scrollTop = ref(0)
const viewportHeight = ref(600)
/** 视口上下各多渲染几屏，快速滚动时不会看到空白。 */
const OVERSCAN = 6

const totalHeight = computed(() => visible.value.length * props.rowHeight)

const windowRows = computed(() => {
  if (!props.virtual) return []
  const start = Math.max(0, Math.floor(scrollTop.value / props.rowHeight) - OVERSCAN)
  const count = Math.ceil(viewportHeight.value / props.rowHeight) + OVERSCAN * 2
  const end = Math.min(visible.value.length, start + count)
  const out = []
  for (let i = start; i < end; i += 1) {
    out.push({ row: visible.value[i], top: i * props.rowHeight, index: i })
  }
  return out
})

function onScroll(event) {
  scrollTop.value = event.target.scrollTop
}

let resizeObserver = null

onMounted(() => {
  if (!scrollRef.value) return
  viewportHeight.value = scrollRef.value.clientHeight || 600
  // ResizeObserver 比监听 window.resize 更准：侧栏折叠 / 拖拽调宽都会改变高度，
  // 而窗口尺寸并不变。
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver((entries) => {
      const height = entries[0]?.contentRect?.height
      if (height) viewportHeight.value = height
    })
    resizeObserver.observe(scrollRef.value)
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
})

/** 列表切换（换文件夹 / 换账户 / 搜索）时回到顶部。 */
watch(
  () => [props.title, props.searchQuery, props.messages.length && props.messages[0]?.uid],
  async () => {
    await nextTick()
    if (scrollRef.value) scrollRef.value.scrollTop = 0
    scrollTop.value = 0
  },
)

/** 键盘导航：把「当前行」滚进视口。老机器上平滑滚动会掉帧，这里用自动。 */
function ensureVisible(key) {
  if (!scrollRef.value || !key) return
  const index = visible.value.findIndex((m) => rowKey(m) === key)
  if (index < 0) return
  const top = index * props.rowHeight
  const el = scrollRef.value
  if (top < el.scrollTop) {
    el.scrollTop = top
  } else if (top + props.rowHeight > el.scrollTop + el.clientHeight) {
    el.scrollTop = top + props.rowHeight - el.clientHeight
  }
}

defineExpose({ ensureVisible })

const keyOf = (row) => `${row.accountId}:${row.folder}:${row.uid}`
const allChecked = computed(
  () => visible.value.length > 0 && props.selectedKeys.length >= visible.value.length,
)
const someChecked = computed(() => props.selectedKeys.length > 0)

const isChecked = (row) => props.selectedKeys.includes(keyOf(row))

/** 点行：按住 Ctrl/Cmd 或 Shift 时切换勾选，否则打开。与 Outlook 习惯一致。 */
function onRowClick(row, event) {
  if (event.ctrlKey || event.metaKey || event.shiftKey || props.selectedKeys.length) {
    emit('toggle-select', row, !isChecked(row))
    return
  }
  emit('open', row)
}

function onRowContextMenu(row, event) {
  // 右键只负责弹出菜单，不改变勾选状态——「右键一下突然出现一堆复选框」
  // 比「右键菜单只作用于点中的那一封」更让人意外。批量目标由父组件决定。
  emit('contextmenu', { row, event })
}

function onDragStart(row, event) {
  event.dataTransfer.effectAllowed = 'move'
  // 拖动已选中的行时整批一起走；否则只走这一行。
  const payload = isChecked(row) ? [...props.selectedKeys] : [keyOf(row)]
  event.dataTransfer.setData('text/x-guaxi-mail', JSON.stringify(payload))
  emit('drag-start', row)
}

function onBatchMove(event) {
  const target = event.target.value
  event.target.value = ''
  if (target) emit('batch', 'move', target)
}

const fieldPlaceholder = computed(
  () =>
    ({
      subject: '搜索主题…',
      from: '搜索发件人…',
      to: '搜索收件人…',
      body: '搜索正文…',
    })[props.searchField] || '搜索邮件、发件人、正文…',
)

const fieldIcon = computed(
  () =>
    ({
      subject: '≡',
      from: '⇤',
      to: '⇥',
      body: '¶',
    })[props.searchField] || '⌕',
)

function rowKey(row) {
  return `${row.accountId}:${row.folder}:${row.uid}`
}

function isSelected(row) {
  const sel = props.selected
  return Boolean(sel && sel.accountId === row.accountId && sel.folder === row.folder && sel.uid === row.uid)
}





</script>
