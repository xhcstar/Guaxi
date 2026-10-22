<template>
  <div class="group-editor">
    <h3 class="group-editor-title">{{ group.id ? '编辑通讯组' : '新建通讯组' }}</h3>

    <div class="mail-field">
      <label class="mail-flabel">组名</label>
      <input v-model="form.name" class="mail-input" placeholder="例如：财务组、项目 A 相关人" />
    </div>
    <div class="mail-field">
      <label class="mail-flabel">说明</label>
      <input v-model="form.description" class="mail-input" placeholder="可选" />
    </div>

    <div class="mail-field">
      <label class="mail-flabel">成员（{{ form.members.length }}）</label>

      <div class="group-picker">
        <input
          v-model="search"
          class="mail-input"
          placeholder="搜索联系人或直接输入邮箱后回车"
          @keydown.enter.prevent="addTypedEmail"
        />
        <div v-if="candidates.length" class="group-candidates">
          <button
            v-for="candidate in candidates"
            :key="candidate.key"
            class="group-candidate"
            @click="addCandidate(candidate)"
          >
            <span class="group-candidate-name">{{ candidate.name }}</span>
            <span class="group-candidate-email">{{ candidate.email }}</span>
          </button>
        </div>
      </div>

      <div class="group-members">
        <div v-for="(member, index) in form.members" :key="index" class="group-member">
          <span class="group-member-name">{{ memberLabel(member) }}</span>
          <span class="group-member-email">{{ memberEmail(member) }}</span>
          <button class="mail-icon-btn" title="移出" @click="form.members.splice(index, 1)">×</button>
        </div>
        <div v-if="!form.members.length" class="mail-hint">还没有成员。</div>
      </div>
    </div>

    <div class="group-editor-foot">
      <button class="mail-btn" @click="$emit('cancel')">取消</button>
      <button class="mail-btn is-primary" :disabled="saving || !form.name.trim()" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'

const props = defineProps({
  group: { type: Object, required: true },
  contacts: { type: Array, default: () => [] },
  saving: { type: Boolean, default: false },
})
const emit = defineEmits(['save', 'cancel'])

const form = reactive(JSON.parse(JSON.stringify(props.group)))
const search = ref('')

watch(
  () => props.group,
  (next) => Object.assign(form, JSON.parse(JSON.stringify(next))),
)

const candidates = computed(() => {
  const q = search.value.trim().toLowerCase()
  if (!q) return []
  const out = []
  for (const contact of props.contacts) {
    const name = contact.displayName || ''
    for (const email of contact.emails || []) {
      if (!email.value) continue
      if (
        name.toLowerCase().includes(q) ||
        email.value.toLowerCase().includes(q)
      ) {
        if (form.members.some((m) => m.contactId === contact.id)) continue
        out.push({
          key: `${contact.id}:${email.value}`,
          name: name || email.value,
          email: email.value,
          contactId: contact.id,
        })
      }
    }
    if (out.length >= 12) break
  }
  return out
})

function addCandidate(candidate) {
  form.members.push({
    kind: 'contact',
    contactId: candidate.contactId,
    email: candidate.email,
    name: candidate.name,
  })
  search.value = ''
}

/** 直接输入一个不在通讯录里的地址。 */
function addTypedEmail() {
  const raw = search.value.trim()
  if (!raw.includes('@')) return
  const match = raw.match(/^(.*?)\s*<([^>]+)>$/)
  const name = match ? match[1].trim() : ''
  const email = (match ? match[2] : raw).trim()
  if (form.members.some((m) => m.email?.toLowerCase() === email.toLowerCase())) {
    search.value = ''
    return
  }
  form.members.push({ kind: 'email', contactId: '', email, name })
  search.value = ''
}

function memberEmail(member) {
  if (member.kind === 'email') return member.email
  const contact = props.contacts.find((c) => c.id === member.contactId)
  return contact?.emails?.[0]?.value || member.email || '（联系人已删除）'
}

function memberLabel(member) {
  if (member.kind === 'email') return member.name || member.email
  const contact = props.contacts.find((c) => c.id === member.contactId)
  return contact?.displayName || member.name || '未命名'
}

function save() {
  emit('save', { ...form, name: form.name.trim() })
}
</script>

<style scoped>
.group-editor {
  display: flex;
  flex-direction: column;
  padding: 14px 16px 24px;
  overflow-y: auto;
}

.group-editor-title {
  margin: 0 0 14px;
  font-size: 14px;
  font-weight: 650;
}

.group-picker {
  position: relative;
  margin-bottom: 8px;
}

.group-candidates {
  position: absolute;
  left: 0;
  right: 0;
  top: 100%;
  z-index: 5;
  margin-top: 2px;
  border: 1px solid var(--mail-border-strong);
  border-radius: var(--mail-radius-sm);
  background: var(--mail-panel);
  box-shadow: var(--mail-shadow);
  max-height: 200px;
  overflow-y: auto;
}

.group-candidate {
  display: flex;
  flex-direction: column;
  width: 100%;
  padding: 6px 10px;
  border: 0;
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.group-candidate:hover {
  background: var(--mail-panel-hover);
}
.group-candidate-name {
  font-size: 12px;
}
.group-candidate-email {
  font-size: 10.5px;
  color: var(--mail-text-muted);
}

.group-members {
  border: 1px solid var(--mail-border);
  border-radius: var(--mail-radius-sm);
  padding: 6px;
  max-height: 240px;
  overflow-y: auto;
}

.group-member {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 4px;
  border-bottom: 1px solid var(--mail-border);
}
.group-member:last-child {
  border-bottom: 0;
}
.group-member-name {
  font-size: 12px;
  flex: 0 0 auto;
  max-width: 40%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-member-email {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 10.5px;
  color: var(--mail-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-editor-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--mail-border);
}
</style>
