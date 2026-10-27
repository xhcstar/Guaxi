<template>
  <div class="address-picker">
    <div class="address-picker-head">
      <input
        ref="searchRef"
        v-model="query"
        class="mail-input"
        placeholder="搜索联系人、通讯组，或直接输入邮箱"
        @keydown.enter.prevent="addTyped"
        @keydown.esc="$emit('close')"
      />
      <button class="mail-icon-btn" title="关闭" @click="$emit('close')">×</button>
    </div>

    <div class="address-picker-body">
      <template v-if="!query.trim()">
        <div class="address-picker-section">通讯组</div>
        <button
          v-for="group in groups"
          :key="group.id"
          class="address-picker-row"
          @click="$emit('pick-group', group.id)"
        >
          <span class="address-picker-kind">组</span>
          <span class="address-picker-name">{{ group.name }}</span>
          <span class="address-picker-meta">{{ groupCount(group) }} 人</span>
        </button>
        <div class="address-picker-section">常用联系人</div>
        <button
          v-for="contact in frequent"
          :key="contact.id"
          class="address-picker-row"
          @click="pickContact(contact)"
        >
          <span class="address-picker-kind">人</span>
          <span class="address-picker-name">{{ contact.displayName || contact.primaryEmail }}</span>
          <span class="address-picker-meta">
            {{ contact.primaryEmail }}
            <template v-if="contact.messageCount"> · {{ contact.messageCount }} 封</template>
          </span>
        </button>
        <div v-if="!groups.length && !frequent.length" class="mail-hint">
          通讯录还是空的。同步收件箱会自动收录联系人，也可以在「通讯录」里导入 vCard。
        </div>
      </template>

      <template v-else>
        <button
          v-for="item in results"
          :key="`${item.kind}-${item.email}`"
          class="address-picker-row"
          @click="pickSuggestion(item)"
        >
          <span class="address-picker-kind">{{ item.kind === 'group' ? '组' : '人' }}</span>
          <span class="address-picker-name">{{ item.display }}</span>
        </button>
        <button v-if="typedAddress" class="address-picker-row is-add" @click="addTyped">
          <span class="address-picker-kind">＋</span>
          <span class="address-picker-name">使用 {{ typedAddress }}</span>
        </button>
        <div v-if="!results.length && !typedAddress" class="mail-hint">没有匹配的联系人。</div>
      </template>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useContactsStore } from '../../../../stores/contactsStore'

const props = defineProps({
  /** 已经在收件人里的地址，选的时候不再重复列出。 */
  exclude: { type: Array, default: () => [] },
  /** 选中后不自动关闭：规则条件这类需要连续挑选多个地址的场景。 */
  keepOpen: { type: Boolean, default: false },
})
const emit = defineEmits(['pick', 'pick-group', 'close'])

const book = useContactsStore()
const query = ref('')
const results = ref([])
const searchRef = ref(null)
let requestId = 0

onMounted(async () => {
  await book.init()
  await nextTick()
  searchRef.value?.focus()
})

const groups = computed(() => book.groups)

/** 组的真实人数：嵌套组展开、地址去重。 */
function groupCount(group) {
  return book.groupCounts?.[group.id] ?? group.members?.length ?? 0
}
const frequent = computed(() =>
  [...book.contacts]
    .filter((c) => c.emails?.length && !isExcluded(c.emails[0].value))
    .sort((a, b) => (b.messageCount || 0) - (a.messageCount || 0))
    .slice(0, 12),
)

/** 输入框里是一整个邮箱时，允许「直接使用」。 */
const typedAddress = computed(() => {
  const text = query.value.trim()
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text) && !isExcluded(text) ? text : ''
})

function isExcluded(address) {
  const norm = (v) => {
    const match = String(v || '').match(/<([^>]+)>/)
    return (match ? match[1] : v).trim().toLowerCase()
  }
  return props.exclude.some((item) => norm(item) === norm(address))
}

// 输入即搜。
let timer = null
function scheduleSearch() {
  clearTimeout(timer)
  timer = setTimeout(async () => {
    const text = query.value.trim()
    if (!text) {
      results.value = []
      return
    }
    const id = ++requestId
    const items = await book.suggest(text)
    if (id !== requestId) return
    results.value = items || []
  }, 140)
}

watch(query, () => scheduleSearch())

function pickContact(contact) {
  const address = contact.emails?.[0]?.value
  if (address) {
    emit('pick', address)
    if (!props.keepOpen) emit('close')
  }
}

function pickSuggestion(item) {
  if (item.kind === 'group') {
    emit('pick-group', item.contactId)
  } else {
    emit('pick', item.email)
  }
  if (!props.keepOpen) emit('close')
}

function addTyped() {
  const address = typedAddress.value
  if (!address) return
  emit('pick', address)
  query.value = ''
  if (!props.keepOpen) emit('close')
}
</script>

<style scoped>
.address-picker {
  border-bottom: 1px solid var(--mail-border);
  background: var(--mail-bg-sunken);
  max-height: 240px;
  display: flex;
  flex-direction: column;
}

.address-picker-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  border-bottom: 1px solid var(--mail-border);
}

.address-picker-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 10px 8px;
}

.address-picker-section {
  padding: 6px 4px 3px;
  font-size: 10.5px;
  color: var(--mail-text-muted);
}

.address-picker-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 5px 4px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.address-picker-row:hover {
  background: var(--mail-panel-hover);
}
.address-picker-row.is-add {
  color: var(--mail-accent);
}

.address-picker-kind {
  flex: 0 0 auto;
  font-size: 10px;
  color: var(--mail-text-muted);
  background: var(--mail-bg);
  border-radius: 3px;
  padding: 0 4px;
}

.address-picker-name {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.address-picker-meta {
  font-size: 10.5px;
  color: var(--mail-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
