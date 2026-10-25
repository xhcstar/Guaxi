<template>
  <div class="group-members-view">
    <div class="group-head">
      <h3>{{ group?.name || '通讯组' }}</h3>
      <span class="mail-hint">{{ members.length }} 位成员</span>
      <button class="mail-btn" style="margin-left: auto" @click="$emit('edit')">编辑</button>
      <button class="mail-btn is-primary" @click="composeAll">写信给全组</button>
    </div>
    <p v-if="group?.description" class="mail-hint">{{ group.description }}</p>

    <div v-if="loading" class="mail-hint">正在展开成员…</div>
    <div v-else-if="!members.length" class="mail-hint">这个组还没有成员。</div>
    <div v-for="(member, index) in members" :key="`${member.email}-${index}`" class="group-members-row">
      <span class="group-members-name">{{ memberName(member) }}</span>
      <button class="contacts-link" @click="$emit('compose-to', memberEmail(member))">
        {{ memberEmail(member) }}
      </button>
      <span v-if="member.viaGroup" class="group-members-via" :title="`经由 ${member.viaGroup}`">
        经 {{ member.viaGroup }}
      </span>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue'
import { useContactsStore } from '../../../../stores/contactsStore'

const props = defineProps({
  group: { type: Object, default: null },
})
const emit = defineEmits(['compose-to', 'compose-group', 'edit'])

const book = useContactsStore()
const members = ref([])
const loading = ref(false)

// 详情页要能看到「所有人」：嵌套组在这里已经递归展开、地址去重；
// 后端展开失败（例如组里有已删除的成员）时退回原始成员行，至少不空白。
// 监听整个 group 对象而不是 id：编辑保存后 store 会用新对象替换旧对象，
// 只盯 id 的话人数和成员列表不会刷新。
watch(
  () => props.group,
  async (group) => {
    members.value = group?.members || []
    if (!group?.id) return
    loading.value = true
    try {
      const resolved = await book.expandGroup(group.id)
      if (Array.isArray(resolved) && resolved.length) members.value = resolved
    } catch {
      /* 保留原始成员行 */
    } finally {
      loading.value = false
    }
  },
  { immediate: true },
)

function memberEmail(member) {
  return member.email || ''
}

function memberName(member) {
  return member.name || member.email || '（联系人已删除）'
}

/** 地址栏只放组名芯片，真正展开留给发送时做。 */
function composeAll() {
  if (props.group?.id) emit('compose-group', props.group)
}
</script>

<style scoped>
.group-members-view {
  padding: 14px 16px;
}

.group-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 8px;
}
.group-head h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 650;
}

.group-members-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 5px 0;
  border-bottom: 1px solid var(--mail-border);
}
.group-members-name {
  flex: 0 0 34%;
  font-size: 12.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-link {
  border: 0;
  background: transparent;
  color: var(--mail-accent);
  font: inherit;
  font-size: 12px;
  padding: 0;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-link:hover {
  text-decoration: underline;
}

.group-members-via {
  margin-left: auto;
  flex: 0 0 auto;
  font-size: 10px;
  color: var(--mail-text-muted);
  border: 1px solid var(--mail-border);
  border-radius: 999px;
  padding: 0 6px;
}
</style>
