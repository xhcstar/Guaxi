<template>
  <div class="group-members-view">
    <div class="group-head">
      <h3>{{ group?.name || '通讯组' }}</h3>
      <span class="mail-hint">{{ group?.members?.length || 0 }} 位成员</span>
      <button class="mail-btn" style="margin-left: auto" @click="$emit('edit')">编辑</button>
      <button class="mail-btn is-primary" @click="composeAll">写信给全组</button>
    </div>
    <p v-if="group?.description" class="mail-hint">{{ group.description }}</p>

    <div v-if="!group?.members?.length" class="mail-hint">这个组还没有成员。</div>
    <div
      v-for="(member, index) in group?.members || []"
      :key="index"
      class="group-members-row"
    >
      <span class="group-members-name">{{ memberName(member) }}</span>
      <button class="contacts-link" @click="$emit('compose-to', memberEmail(member))">
        {{ memberEmail(member) }}
      </button>
    </div>
  </div>
</template>

<script setup>
const props = defineProps({
  group: { type: Object, default: null },
})
const emit = defineEmits(['compose-to', 'edit'])

function memberEmail(member) {
  return member.email || ''
}

function memberName(member) {
  return member.name || member.email || '（联系人已删除）'
}

function composeAll() {
  const addresses = (props.group?.members || [])
    .map((m) => m.email)
    .filter(Boolean)
  if (addresses.length) emit('compose-to', addresses.join(', '))
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
  flex: 0 0 40%;
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
</style>
