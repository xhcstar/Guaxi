<template>
  <section class="mail-pane mail-list-pane">
    <div class="mail-list-head">
      <div class="mail-list-title-row">
        <h2 class="mail-list-title">{{ title }}</h2>
        <span class="mail-list-meta">{{ visible.length }} 封</span>
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

    <div class="mail-list-scroll" role="list">
      <div v-if="!visible.length && !loading" class="mail-empty">
        <div class="mail-empty-title">这里没有邮件</div>
        <div v-if="searchQuery">换个关键词，或者清空搜索条件再试。</div>
        <div v-else>同步一下，或者切到别的文件夹看看。</div>
      </div>

      <article
        v-for="row in visible"
        :key="rowKey(row)"
        class="mail-row"
        :class="{
          'is-unread': !row.seen,
          'is-selected': isSelected(row),
          thread: row.threadCount > 1,
        }"
        role="listitem"
        tabindex="0"
        @click="$emit('open', row)"
        @keydown.enter="$emit('open', row)"
        @contextmenu.prevent="$emit('contextmenu', { row, event: $event })"
      >
        <span class="mail-avatar" :style="avatarStyle(row)">{{ avatarLetter(row) }}</span>

        <div class="mail-row-line">
          <span class="mail-row-from">{{ fromLabel(row) }}</span>
          <span class="mail-row-date">{{ dateLabel(row.date) }}</span>
        </div>

        <div class="mail-row-line">
          <span
            class="mail-row-subject"
            :data-thread-count="row.threadCount > 1 ? row.threadCount : null"
            :title="row.subject || '（无主题）'"
          >
            {{ row.subject || '（无主题）' }}
          </span>
          <span class="mail-row-markers">
            <span v-for="label in rowLabels(row)" :key="label" class="mail-label">{{ label }}</span>
            <span v-if="row.hasAttachments" title="含附件">📎</span>
            <button
              class="mail-star"
              :class="{ 'is-on': row.flagged }"
              :title="row.flagged ? '取消星标' : '加星标'"
              @click.stop="$emit('toggle-star', row, !row.flagged)"
            >
              {{ row.flagged ? '★' : '☆' }}
            </button>
          </span>
        </div>

        <div class="mail-row-snippet">{{ row.snippet || '（无正文预览）' }}</div>
      </article>
    </div>
  </section>
</template>

<script setup>
import { computed } from 'vue'
import { avatarSeed } from './theme'

const props = defineProps({
  messages: { type: Array, default: () => [] },
  selected: { type: Object, default: null },
  title: { type: String, default: '收件箱' },
  loading: { type: Boolean, default: false },
  searchQuery: { type: String, default: '' },
  searchField: { type: String, default: 'all' },
  filters: {
    type: Object,
    default: () => ({ unreadOnly: false, starredOnly: false, hasAttachment: false }),
  },
})
defineEmits([
  'open',
  'toggle-star',
  'contextmenu',
  'update:search-query',
  'update:search-field',
  'search',
  'clear-search',
  'toggle-filter',
])

const visible = computed(() => props.messages || [])

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

function avatarLetter(row) {
  return avatarSeed(row.fromName, row.fromAddr).letter
}

function avatarStyle(row) {
  const { hue } = avatarSeed(row.fromName, row.fromAddr)
  return {
    background: `hsl(${hue} 42% 48%)`,
  }
}

function fromLabel(row) {
  if (row.fromName && row.fromName.trim()) return row.fromName
  return row.fromAddr || '（未知发件人）'
}

function rowLabels(row) {
  return String(row.labels || '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(0, 2)
}

function dateLabel(unix) {
  if (!unix) return ''
  const date = new Date(unix * 1000)
  const now = new Date()
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate()
  if (sameDay) {
    return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })
  }
  if (date.getFullYear() === now.getFullYear()) {
    return date.toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })
  }
  return date.toLocaleDateString('zh-CN', { year: '2-digit', month: 'numeric', day: 'numeric' })
}
</script>
