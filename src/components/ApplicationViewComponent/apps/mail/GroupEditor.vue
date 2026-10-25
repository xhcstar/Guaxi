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
          placeholder="搜索联系人、通讯组，或直接输入邮箱后回车"
          @keydown.enter.prevent="addTypedEmail"
        />
        <div v-if="candidates.length" class="group-candidates">
          <button
            v-for="candidate in candidates"
            :key="candidate.key"
            class="group-candidate"
            @click="addCandidate(candidate)"
          >
            <span class="group-candidate-kind">{{ candidate.kind === 'group' ? '组' : '人' }}</span>
            <span class="group-candidate-name">{{ candidate.name }}</span>
            <span class="group-candidate-email">{{ candidate.email }}</span>
          </button>
        </div>
      </div>

      <div class="group-members">
        <div v-for="(member, index) in form.members" :key="memberKey(member, index)" class="group-member">
          <span class="group-member-kind">{{ kindLabel(member.kind) }}</span>
          <span class="group-member-name">{{ memberLabel(member) }}</span>

          <!-- 联系人可能有好几个邮箱（公司 + 个人），成员上要能选发给哪一个。 -->
          <select
            v-if="emailChoices(member).length > 1"
            class="group-member-select"
            :value="memberEmail(member)"
            :title="'选择成员邮箱'"
            @change="changeMemberEmail(member, $event.target.value)"
          >
            <option v-for="email in emailChoices(member)" :key="email" :value="email">
              {{ email }}
            </option>
          </select>
          <span v-else-if="member.kind === 'group'" class="group-member-email">
            通讯组 · {{ groupCount(member.groupId) }} 人
          </span>
          <span v-else class="group-member-email">{{ memberEmail(member) }}</span>

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
import { expandGroupMembers } from '../../../../stores/contactsStore'

const props = defineProps({
  group: { type: Object, required: true },
  contacts: { type: Array, default: () => [] },
  /** 全部通讯组：用来把一个组加成另一个组的成员（嵌套组）。 */
  groups: { type: Array, default: () => [] },
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

  // 1) 联系人（每个联系人一个候选；多邮箱的人可以逐个添加）
  for (const contact of props.contacts) {
    const name = contact.displayName || ''
    for (const email of contact.emails || []) {
      if (!email.value) continue
      if (name.toLowerCase().includes(q) || email.value.toLowerCase().includes(q)) {
        if (hasContactMember(contact.id, email.value)) continue
        if (hasContactMember(contact.id)) continue
        out.push({
          key: `c:${contact.id}:${email.value}`,
          kind: 'contact',
          name: name || email.value,
          email: email.value,
          contactId: contact.id,
        })
      }
    }
    if (out.length >= 12) break
  }

  // 2) 通讯组（嵌套）；不能把当前组或「包含当前组」的祖先进来，避免环。
  for (const group of props.groups) {
    if (!group.id || group.id === form.id) continue
    if (!String(group.name || '').toLowerCase().includes(q)) continue
    if (form.members.some((m) => m.kind === 'group' && m.groupId === group.id)) continue
    if (groupContains(group.id, form.id)) continue
    out.push({
      key: `g:${group.id}`,
      kind: 'group',
      name: group.name,
      email: `通讯组 · ${groupCount(group.id)} 人`,
      groupId: group.id,
    })
    if (out.length >= 16) break
  }
  return out
})

function hasContactMember(contactId, email = '') {
  return form.members.some(
    (m) =>
      m.kind === 'contact' &&
      m.contactId === contactId &&
      (!email || String(m.email || '').toLowerCase() === email.toLowerCase()),
  )
}

/** 候选组是否（递归地）包含 `targetId`：是的话加进来会形成循环。 */
function groupContains(rootId, targetId, seen = []) {
  if (!rootId || !targetId || seen.includes(rootId)) return false
  const group = (props.groups || []).find((g) => g.id === rootId)
  if (!group) return false
  const path = [...seen, rootId]
  for (const member of group.members || []) {
    if (member.kind !== 'group') continue
    if (member.groupId === targetId) return true
    if (groupContains(member.groupId, targetId, path)) return true
  }
  return false
}

function addCandidate(candidate) {
  if (candidate.kind === 'group') {
    form.members.push({ kind: 'group', groupId: candidate.groupId, name: candidate.name })
  } else {
    form.members.push({
      kind: 'contact',
      contactId: candidate.contactId,
      email: candidate.email,
      name: candidate.name,
    })
  }
  search.value = ''
}

/** 直接输入一个不在通讯录里的地址。 */
function addTypedEmail() {
  const raw = search.value.trim()
  if (!raw.includes('@')) return
  const match = raw.match(/^(.*?)\s*<([^>]+)>$/)
  const name = match ? match[1].trim() : ''
  const email = (match ? match[2] : raw).trim()
  if (form.members.some((m) => String(m.email || '').toLowerCase() === email.toLowerCase())) {
    search.value = ''
    return
  }
  form.members.push({ kind: 'email', contactId: '', email, name })
  search.value = ''
}

function memberEmail(member) {
  if (member.kind === 'email') return member.email
  const contact = props.contacts.find((c) => c.id === member.contactId)
  return member.email || contact?.emails?.[0]?.value || ''
}

/** 联系人的全部邮箱：多于一个时成员行上给出下拉选择。 */
function emailChoices(member) {
  if (member.kind !== 'contact') return []
  const contact = props.contacts.find((c) => c.id === member.contactId)
  const list = (contact?.emails || []).map((e) => e.value).filter(Boolean)
  if (member.email && !list.includes(member.email)) list.unshift(member.email)
  return list
}

function changeMemberEmail(member, email) {
  member.email = email
}

function memberLabel(member) {
  if (member.kind === 'group') {
    const group = props.groups.find((g) => g.id === member.groupId)
    return group?.name || member.name || '（通讯组已删除）'
  }
  if (member.kind === 'email') return member.name || member.email
  const contact = props.contacts.find((c) => c.id === member.contactId)
  return contact?.displayName || member.name || '未命名'
}

function groupCount(id) {
  return expandGroupMembers(props.groups, id).length
}

function kindLabel(kind) {
  return { group: '组', email: '邮箱', contact: '人' }[kind] || '人'
}

function memberKey(member, index) {
  return `${member.kind}:${member.contactId || member.groupId || member.email}:${index}`
}

function save() {
  emit('save', {
    ...form,
    name: form.name.trim(),
    members: form.members.map((member) => ({ ...member })),
  })
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
  align-items: baseline;
  gap: 8px;
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
.group-candidate-kind {
  flex: 0 0 auto;
  font-size: 9px;
  line-height: 14px;
  padding: 0 4px;
  border-radius: 3px;
  background: var(--mail-bg-sunken);
  color: var(--mail-text-muted);
}
.group-candidate-name {
  font-size: 12px;
  flex: 0 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-candidate-email {
  font-size: 10.5px;
  color: var(--mail-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-members {
  border: 1px solid var(--mail-border);
  border-radius: var(--mail-radius-sm);
  padding: 6px;
  max-height: 280px;
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
.group-member-kind {
  flex: 0 0 auto;
  font-size: 9px;
  line-height: 14px;
  padding: 0 4px;
  border-radius: 3px;
  background: var(--mail-bg-sunken);
  color: var(--mail-text-muted);
}
.group-member-name {
  font-size: 12px;
  flex: 0 1 auto;
  max-width: 34%;
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
.group-member-select {
  flex: 1 1 auto;
  min-width: 0;
  max-width: 100%;
  border: 1px solid var(--mail-border);
  border-radius: 4px;
  background: var(--mail-panel);
  color: var(--mail-text);
  font: inherit;
  font-size: 10.5px;
  padding: 1px 4px;
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
