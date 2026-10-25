<template>
  <div class="address-row mail-compose-line">
      <span class="key">{{ label }}</span>
      <div class="address-field">
        <!-- 已选地址渲染成胶囊：点整块可删，点邮箱回改 -->
        <span
          v-for="(token, index) in tokens"
          :key="`${token}-${index}`"
          class="address-chip"
          :class="{ 'is-group': groupOf(token) }"
          :title="chipTitle(token)"
        >
          <template v-if="groupOf(token)">
            <span class="address-chip-badge">组</span>
            <span class="address-chip-name">{{ groupOf(token).name || '通讯组' }}</span>
            <span v-if="groupCount(token)" class="address-chip-count">{{ groupCount(token) }} 人</span>
          </template>
          <template v-else>{{ token }}</template>
          <button class="address-chip-remove" title="移除" @click="removeToken(index)">×</button>
        </span>
        <input
          ref="inputRef"
          v-model="query"
          :placeholder="tokens.length ? '' : placeholder"
          spellcheck="false"
          @input="onInput"
          @keydown.enter.prevent="commitTyped"
          @keydown.,.prevent="commitTyped"
          @keydown.tab.prevent="tabComplete"
          @keydown.esc="closeSuggestions"
          @keydown.backspace="onBackspace"
          @keydown.down.prevent="move(1)"
          @keydown.up.prevent="move(-1)"
          @focus="onInput"
          @blur="onBlur"
        />
      </div>
      <slot name="actions" />
      <div
        v-if="suggestions.length"
        class="address-suggest"
        :style="popupStyle"
        @mousedown.prevent
      >
        <button
          v-for="(item, index) in suggestions"
          :key="`${item.email}-${index}`"
          class="address-suggest-item"
          :class="{ 'is-active': index === activeIndex }"
          @mousedown.prevent="pick(item)"
          @mouseenter="activeIndex = index"
        >
          <span class="address-suggest-kind">{{ kindLabel(item.kind) }}</span>
          <span class="address-suggest-main">
            <span class="address-suggest-name">{{ item.display }}</span>
            <span v-if="item.memberCount" class="address-suggest-meta">{{ item.memberCount }} 人</span>
          </span>
        </button>
      </div>
    </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  expandGroupMembers,
  groupRecipientToken,
  parseGroupToken,
} from '../../../../stores/contactsStore'

const props = defineProps({
  modelValue: { type: Array, default: () => [] },
  label: { type: String, default: '收件人' },
  placeholder: { type: String, default: '输入姓名或邮箱，回车确认' },
  /** 由父组件提供的补全函数，返回 ContactSuggestion[]。 */
  suggest: { type: Function, default: null },
  /** 通讯组清单（contacts.groups），用来显示组名与人数。 */
  groups: { type: Array, default: () => [] },
})
const emit = defineEmits(['update:modelValue'])

const query = ref('')
/** 下拉层位置：跟随输入框，滚动/改尺寸时也会重算（见 onScrollCapture）。 */
const popupStyle = ref({})
const inputRef = ref(null)
const suggestions = ref([])
const activeIndex = ref(0)
let requestId = 0

const tokens = computed(() => props.modelValue || [])

async function onInput() {
  const text = query.value.trim()
  if (!text || !props.suggest) {
    suggestions.value = []
    return
  }
  const id = ++requestId
  try {
    const items = await props.suggest(text)
    // 迟到的响应不能覆盖更新的输入。
    if (id !== requestId) return
    suggestions.value = items || []
    activeIndex.value = 0
    await nextTick()
    positionPopup()
  } catch {
    suggestions.value = []
  }
}

function onBlur() {
  // 失焦后延迟关闭：给 mousedown 选中留时间。
  setTimeout(() => {
    closeSuggestions()
    // 失焦时把没确认的地址也收进去，避免用户填了却「消失」。
    commitTyped({ silent: true })
  }, 120)
}

function closeSuggestions() {
  suggestions.value = []
  activeIndex.value = 0
}

function pick(item) {
  if (item.kind === 'group') {
    // 组只放一个芯片：地址栏里不铺开上百个地址，发送时由 Rust 侧展开。
    const id = item.contactId || String(item.email || '').replace(/^group:/, '')
    const group = (props.groups || []).find((g) => g.id === id)
    addToken(groupRecipientToken(group || { id, name: item.display }))
  } else {
    addToken(item.email)
  }
  query.value = ''
  closeSuggestions()
  nextTick(() => inputRef.value?.focus())
}

function commitTyped(options = {}) {
  const text = query.value.trim()
  if (!text) return
  // 回车时若正有高亮候选，优先选它——与主流客户端一致。
  if (!options.silent && suggestions.value.length) {
    pick(suggestions.value[activeIndex.value] || suggestions.value[0])
    return
  }
  for (const part of text.split(/[,;，；]/)) {
    const value = part.trim()
    if (value) addToken(value)
  }
  query.value = ''
  closeSuggestions()
}

function positionPopup() {
  const anchor = inputRef.value || null
  if (!anchor) return
  const rect = anchor.getBoundingClientRect()
  if (!rect.width) return
  popupStyle.value = {
    position: 'fixed',
    left: `${Math.max(8, rect.left - 44)}px`,
    top: `${Math.round(rect.bottom + 2)}px`,
    width: `${Math.round(rect.width + 44)}px`,
    right: 'auto',
  }
}

function move(delta) {
  if (!suggestions.value.length) return
  activeIndex.value =
    (activeIndex.value + delta + suggestions.value.length) % suggestions.value.length
}

function tabComplete() {
  if (suggestions.value.length) {
    pick(suggestions.value[activeIndex.value] || suggestions.value[0])
  } else {
    commitTyped()
  }
}

function addToken(value) {
  const cleaned = value.trim()
  if (!cleaned) return
  const next = [...tokens.value]
  if (!next.some((t) => normalize(t) === normalize(cleaned))) next.push(cleaned)
  emit('update:modelValue', next)
}

function removeToken(index) {
  const next = [...tokens.value]
  next.splice(index, 1)
  emit('update:modelValue', next)
}

function onBackspace(event) {
  if (query.value || !tokens.value.length) return
  event.preventDefault()
  removeToken(tokens.value.length - 1)
}

/** 比较地址是否重复：`Name <a@b>` 与 `a@b` 视为同一个。 */
function normalize(value) {
  const match = String(value || '').match(/<([^>]+)>/)
  return (match ? match[1] : value).trim().toLowerCase()
}

/** 解析通讯组芯片；普通地址返回 null。 */
function groupOf(token) {
  const parsed = parseGroupToken(token)
  if (!parsed) return null
  const group = (props.groups || []).find((g) => g.id === parsed.id)
  return { ...parsed, name: group?.name || parsed.name, group }
}

/** 组芯片上的人数：嵌套组展开、去重后的真实收件人数。 */
function groupCount(token) {
  const info = groupOf(token)
  return info ? expandGroupMembers(props.groups, info.id).length : 0
}

function chipTitle(token) {
  const info = groupOf(token)
  if (!info) return token
  const count = groupCount(token)
  return count ? `${info.name}（通讯组，${count} 人）` : `${info.name}（通讯组）`
}

function kindLabel(kind) {
  return { contact: '联系人', history: '历史', group: '通讯组' }[kind] || ''
}

function onViewportChange() {
  if (suggestions.value.length) positionPopup()
}

onMounted(() => {
  window.addEventListener('resize', onViewportChange)
  window.addEventListener('scroll', onViewportChange, true)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', onViewportChange)
  window.removeEventListener('scroll', onViewportChange, true)
})

defineExpose({
  focus: () => inputRef.value?.focus(),
})
</script>

<style scoped>
.address-field {
  flex: 1 1 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  min-width: 0;
}

.address-field input {
  flex: 1 1 120px;
  min-width: 120px;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  font-size: 12.5px;
  padding: 2px 0;
}

.address-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 1px 4px 1px 7px;
  border-radius: 999px;
  background: var(--mail-accent-soft);
  color: var(--mail-text);
  font-size: 11.5px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.address-chip.is-group {
  gap: 5px;
  border: 1px solid var(--mail-accent);
  background: var(--mail-accent-soft);
}

.address-chip-badge {
  flex: 0 0 auto;
  padding: 0 4px;
  border-radius: 3px;
  background: var(--mail-accent);
  color: var(--mail-accent-contrast, #fff);
  font-size: 9px;
  line-height: 14px;
  letter-spacing: 0.5px;
}

.address-chip-name {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.address-chip-count {
  flex: 0 0 auto;
  color: var(--mail-text-muted);
  font-size: 10px;
}

.address-chip-remove {
  border: 0;
  background: transparent;
  color: var(--mail-text-muted);
  cursor: pointer;
  font-size: 13px;
  line-height: 1;
  padding: 0 2px;
}
.address-chip-remove:hover {
  color: var(--mail-danger);
}

.address-suggest {
  /* 位置由 popupStyle 以内联样式给出（fixed，跟随输入框）。 */
  z-index: 60;
  border: 1px solid var(--mail-border-strong);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-panel);
  box-shadow: var(--mail-shadow);
  max-height: 220px;
  overflow-y: auto;
}

.address-suggest-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 5px 10px;
  border: 0;
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.address-suggest-item:hover,
.address-suggest-item.is-active {
  background: var(--mail-panel-hover);
}

.address-suggest-kind {
  flex: 0 0 auto;
  font-size: 10px;
  color: var(--mail-text-muted);
  background: var(--mail-bg-sunken);
  border-radius: 3px;
  padding: 0 4px;
}

.address-suggest-main {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
}

.address-suggest-name {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.address-suggest-meta {
  font-size: 10px;
  color: var(--mail-text-muted);
}
</style>
