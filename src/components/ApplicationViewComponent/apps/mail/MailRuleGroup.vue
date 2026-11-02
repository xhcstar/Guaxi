<template>
  <div class="mail-rule-group" :class="{ 'is-nested': depth > 0 }">
    <div class="mail-rule-group-head">
      <span class="mail-rule-group-label">{{ depth > 0 ? '条件组' : '满足以下条件' }}</span>
      <select v-model="group.mode" class="mail-select" title="同组条件的组合方式">
        <option value="all">全部满足（且）</option>
        <option value="any">任一满足（或）</option>
      </select>
      <label class="mail-check mail-rule-not" title="对整组结果取反">
        <input v-model="group.not" type="checkbox" />
        非
      </label>
      <button
        v-if="depth > 0"
        class="mail-btn mail-rule-group-remove"
        title="移除这个条件组"
        @click="$emit('remove')"
      >
        移除
      </button>
    </div>

    <div class="mail-rule-children">
      <template v-for="(clause, index) in group.clauses" :key="clause.id">
        <span v-if="index > 0" class="mail-rule-joiner">{{ joiner }}</span>
        <div class="mail-rule-clause">
          <select v-model="clause.field" class="mail-select" @change="onFieldChange(clause)">
            <option value="from">发件人</option>
            <option value="to">收件人</option>
            <option value="subject">主题</option>
            <option value="body">正文</option>
            <option value="any">主题 / 正文 / 发件人</option>
            <option value="size">邮件大小</option>
          </select>
          <select v-model="clause.op" class="mail-select">
            <option v-for="op in opsFor(clause.field)" :key="op.value" :value="op.value">
              {{ op.label }}
            </option>
          </select>
          <input
            v-model="clause.value"
            class="mail-input mail-rule-value"
            :placeholder="placeholderFor(clause.field)"
            spellcheck="false"
          />
          <button
            v-if="clause.field === 'from' || clause.field === 'to'"
            class="mail-btn mail-rule-pick-btn"
            :class="{ 'is-on': pickerFor === index }"
            title="从通讯录选择地址"
            @click="togglePicker(index)"
          >
            通讯录
          </button>
          <div v-if="clause.field === 'size'" class="mail-rule-unit">KB</div>
          <button
            class="mail-icon-btn mail-rule-remove"
            title="移除这条条件"
            aria-label="移除这条条件"
            @click="group.clauses.splice(index, 1)"
          >
            ×
          </button>
        </div>

        <div v-if="pickerFor === index" class="mail-rule-picker">
          <AddressPicker
            @pick="(address) => onPickAddress(index, address)"
            @pick-group="(id) => onPickGroup(index, id)"
            @close="pickerFor = -1"
          />
        </div>
      </template>

      <template v-for="(child, index) in group.groups" :key="child.id">
        <span v-if="index > 0 || group.clauses.length" class="mail-rule-joiner">{{ joiner }}</span>
        <MailRuleGroup
          :group="child"
          :depth="depth + 1"
          @remove="group.groups.splice(index, 1)"
        />
      </template>
    </div>

    <div v-if="!group.clauses.length && !group.groups.length" class="mail-hint mail-rule-empty">
      还没有条件。点下面的「＋ 条件」开始；需要混合与或时再加一个条件组。
    </div>

    <div class="mail-rule-actions">
      <button class="mail-btn" @click="addClause">＋ 条件</button>
      <button class="mail-btn" @click="addGroup">＋ 条件组</button>
    </div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import AddressPicker from './AddressPicker.vue'
import { expandGroupMembers, useContactsStore } from '../../../../stores/contactsStore'

defineOptions({ name: 'MailRuleGroup' })

const props = defineProps({
  group: { type: Object, required: true },
  depth: { type: Number, default: 0 },
})
defineEmits(['remove'])

const contacts = useContactsStore()
const pickerFor = ref(-1)

const joiner = computed(() => (props.group.mode === 'any' ? '或' : '且'))

const TEXT_OPS = [
  { value: 'contains', label: '包含' },
  { value: 'notContains', label: '不包含' },
  { value: 'equals', label: '等于' },
  { value: 'notEquals', label: '不等于' },
  { value: 'startsWith', label: '以…开头' },
  { value: 'endsWith', label: '以…结尾' },
  { value: 'regex', label: '正则匹配' },
]
const SIZE_OPS = [
  { value: 'greaterThan', label: '大于' },
  { value: 'lessThan', label: '小于' },
  { value: 'equals', label: '等于' },
  { value: 'notEquals', label: '不等于' },
]

function opsFor(field) {
  return field === 'size' ? SIZE_OPS : TEXT_OPS
}

function placeholderFor(field) {
  if (field === 'size') return '例如 1024'
  if (field === 'subject') return '主题里的词'
  if (field === 'body') return '正文里的词'
  if (field === 'any') return '任意字段里的词或正则'
  return '地址、显示名或域名'
}

let seq = 0
function nextId() {
  seq += 1
  return `c${Date.now().toString(36)}${seq}`
}

function newClause() {
  return { id: nextId(), field: 'from', op: 'contains', value: '' }
}

function newGroup() {
  return { id: nextId(), mode: 'all', not: false, clauses: [newClause()], groups: [] }
}

function addClause() {
  props.group.clauses.push(newClause())
}

function addGroup() {
  props.group.groups.push(newGroup())
}

function onFieldChange(clause) {
  pickerFor.value = -1
  if (clause.field === 'size') {
    clause.op = 'greaterThan'
    if (!clause.value) clause.value = '1024'
  } else if (clause.op === 'greaterThan' || clause.op === 'lessThan') {
    clause.op = 'contains'
  }
}

function togglePicker(index) {
  pickerFor.value = pickerFor.value === index ? -1 : index
}

function onPickAddress(index, address) {
  const clause = props.group.clauses[index]
  if (clause) clause.value = address
}

/**
 * 通讯组：把组成员展开成「任一地址」子组。
 *
 * 直接塞进当前组会受当前组的「且」影响——那会变成「同时来自组里所有人」，
 * 永远不可能命中。单独建一个 any 子组，语义才与用户选「一个组」一致。
 */
function onPickGroup(index, groupId) {
  const clause = props.group.clauses[index]
  if (!clause) return
  const members = expandGroupMembers(contacts.groups, groupId)
    .map((member) => member.email)
    .filter(Boolean)
  if (!members.length) return
  if (members.length === 1) {
    clause.value = members[0]
    pickerFor.value = -1
    return
  }
  const values = [...new Set([clause.value, ...members])].filter(Boolean)
  props.group.clauses.splice(index, 1)
  const negative = clause.op === 'notContains' || clause.op === 'notEquals'
  props.group.groups.push({
    id: nextId(),
    mode: 'any',
    // 原来的「不包含这个组」= 非(包含其中任意一个)。
    not: negative,
    clauses: values.map((value) => ({
      id: nextId(),
      field: clause.field,
      // 组里每个成员都是「来自/发给这个地址」，用包含最直观；原来的「不包含」
      // 在展开成多个地址后会变成「同时不包含所有人」，语义是错的。
      op: 'contains',
      value,
    })),
    groups: [],
  })
  pickerFor.value = -1
}
</script>

<style scoped>
.mail-rule-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.mail-rule-group.is-nested {
  padding: 10px 12px;
  border: 1px solid var(--mail-border);
  border-left: 2px solid var(--mail-accent);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-bg-sunken);
}

.mail-rule-group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.mail-rule-group-label {
  font-size: 12px;
  color: var(--mail-text-muted);
}
.mail-rule-not {
  margin: 0;
}
.mail-rule-group-remove {
  margin-left: auto;
}

.mail-rule-children {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mail-rule-joiner {
  align-self: flex-start;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--mail-accent-soft);
  color: var(--mail-accent);
  font-size: 10.5px;
  font-weight: 600;
  line-height: 16px;
}

.mail-rule-clause {
  display: flex;
  align-items: center;
  gap: 6px;
}
.mail-rule-clause .mail-select {
  flex: 0 0 auto;
}
.mail-rule-value {
  flex: 1 1 auto;
  min-width: 0;
}
.mail-rule-pick-btn {
  flex: 0 0 auto;
  height: 28px;
}
.mail-rule-unit {
  flex: 0 0 auto;
  color: var(--mail-text-muted);
  font-size: 11px;
}
.mail-rule-remove {
  flex: 0 0 auto;
  width: 26px;
  height: 26px;
  font-size: 15px;
}

.mail-rule-picker {
  margin: 2px 0 4px 18px;
  border: 1px solid var(--mail-border-strong);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-panel);
  overflow: hidden;
}
.mail-rule-picker :deep(.address-picker) {
  background: var(--mail-panel);
  border-bottom: 0;
}

.mail-rule-empty {
  margin: 0;
}
.mail-rule-actions {
  display: flex;
  gap: 6px;
}

@media (max-width: 720px) {
  .mail-rule-clause {
    flex-wrap: wrap;
  }
  .mail-rule-value {
    flex-basis: 100%;
  }
}
</style>
