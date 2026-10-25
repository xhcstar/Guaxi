<template>
  <label class="mail-row-check" @click.stop @mousedown.stop @dragstart.prevent>
    <input type="checkbox" :checked="checked" @change="$emit('toggle', $event.target.checked)" />
  </label>
  <span class="mail-avatar" :style="avatarStyle">{{ avatarLetter }}</span>

  <div class="mail-row-line">
    <span class="mail-row-from">{{ fromLabel }}</span>
    <span class="mail-row-date">{{ dateLabel }}</span>
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
      <span v-for="label in labels" :key="label" class="mail-label">{{ label }}</span>
      <span v-if="row.hasAttachments" title="含附件">📎</span>
      <button
        class="mail-star"
        :class="{ 'is-on': row.flagged }"
        :title="row.flagged ? '取消星标' : '加星标'"
        @click.stop="$emit('star', !row.flagged)"
      >
        {{ row.flagged ? '★' : '☆' }}
      </button>
    </span>
  </div>

  <div class="mail-row-snippet">{{ row.snippet || '（无正文预览）' }}</div>

  <!-- 行内删除：悬停出现，省得每次都先选中再右键/点工具栏。 -->
  <span class="mail-row-actions">
    <button
      class="mail-row-delete"
      title="删除"
      aria-label="删除这封邮件"
      @click.stop="$emit('delete')"
    >
      <MailIcon name="trash" :size="14" />
    </button>
  </span>
</template>

<script setup>
// 抽成子组件是为了虚拟滚动：五行结构在「一屏 10 行」和「一万行」下完全一样，
// 但父组件的模板里多一层嵌套会让 Vue 的补丁算法在滚动时做更多对比工作。
// 这里把计算（头像色、日期格式）也一并放进来，父组件只负责定位。
import { computed } from 'vue'
import { avatarSeed } from './theme'
import MailIcon from './MailIcon.vue'

const props = defineProps({
  row: { type: Object, required: true },
  checked: { type: Boolean, default: false },
})

defineEmits(['toggle', 'star', 'delete'])

const avatar = computed(() => avatarSeed(props.row.fromName, props.row.fromAddr))
const avatarLetter = computed(() => avatar.value.letter)
const avatarStyle = computed(() => ({ background: `hsl(${avatar.value.hue} 42% 48%)` }))

const fromLabel = computed(() => {
  const { fromName, fromAddr } = props.row
  if (fromName && fromName.trim()) return fromName
  return fromAddr || '（未知发件人）'
})

const labels = computed(() =>
  String(props.row.labels || '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(0, 2),
)

const dateLabel = computed(() => {
  const unix = props.row.date
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
})
</script>
