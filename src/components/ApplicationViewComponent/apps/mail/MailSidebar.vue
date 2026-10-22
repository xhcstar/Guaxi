<template>
  <aside class="mail-pane mail-sidebar">
    <div class="mail-brand">
      <span class="mail-brand-mark">✉</span>
      <span class="mail-brand-name">邮局</span>
    </div>

    <button class="mail-compose-btn" @click="$emit('compose')">
      <span aria-hidden="true">✎</span> 写邮件
    </button>

    <div class="mail-sidebar-scroll">
      <div class="mail-section-label">账户</div>
      <button
        class="mail-account"
        :class="{ 'is-active': activeAccountId === null }"
        @click="$emit('select-account', null)"
      >
        <span class="mail-account-dot" style="background: var(--mail-text-muted)"></span>
        <span class="mail-account-email">全部账户</span>
        <span v-if="totalUnread" class="mail-badge">{{ totalUnread }}</span>
      </button>

      <button
        v-for="account in accounts"
        :key="account.id"
        class="mail-account"
        :class="{ 'is-active': activeAccountId === account.id }"
        @click="$emit('select-account', account.id)"
      >
        <span class="mail-account-dot" :style="{ background: account.accent }"></span>
        <span class="mail-account-email" :title="account.email">{{ account.email }}</span>
        <span v-if="unread[account.id]" class="mail-badge">{{ unread[account.id] }}</span>
      </button>

      <button class="mail-account is-add" @click="$emit('add-account')">＋ 添加账户</button>

      <!-- 聚合视图下不显示文件夹树：不同账户的同名文件夹混在一起没有意义 -->
      <template v-if="activeAccountId">
        <div class="mail-section-label">快速查看</div>
        <button
          class="mail-folder"
          :class="{ 'is-active': scope === 'unread' }"
          @click="$emit('select-scope', 'unread')"
        >
          <span class="mail-folder-icon">●</span>
          <span class="mail-folder-name">未读</span>
        </button>
        <button
          class="mail-folder"
          :class="{ 'is-active': scope === 'flagged' }"
          @click="$emit('select-scope', 'flagged')"
        >
          <span class="mail-folder-icon">★</span>
          <span class="mail-folder-name">已加星标</span>
        </button>
        <button
          class="mail-folder"
          :class="{ 'is-active': scope === 'attachments' }"
          @click="$emit('select-scope', 'attachments')"
        >
          <span class="mail-folder-icon">📎</span>
          <span class="mail-folder-name">带附件</span>
        </button>
        <button
          class="mail-folder"
          :class="{ 'is-active': scope === 'all' }"
          @click="$emit('select-scope', 'all')"
        >
          <span class="mail-folder-icon">▤</span>
          <span class="mail-folder-name">全部邮件</span>
        </button>

        <div class="mail-section-label">文件夹</div>
        <button
          v-for="folder in folders"
          :key="folder.name"
          class="mail-folder"
          :class="{ 'is-active': scope === 'folder' && activeFolder === folder.name }"
          @click="$emit('select-folder', folder.name)"
        >
          <span class="mail-folder-icon">{{ folderIcon(folder.role) }}</span>
          <span class="mail-folder-name" :title="folder.displayName">{{ folder.displayName }}</span>
          <span v-if="folder.unread" class="mail-badge">{{ folder.unread }}</span>
        </button>
      </template>

      <template v-if="labels.length">
        <div class="mail-section-label">分类</div>
        <button
          v-for="[label, count] in labels"
          :key="label"
          class="mail-folder"
          :class="{ 'is-active': activeLabel === label }"
          @click="$emit('select-scope', 'all', label)"
        >
          <span class="mail-folder-icon">◇</span>
          <span class="mail-folder-name" :title="label">{{ label }}</span>
          <span class="mail-badge">{{ count }}</span>
        </button>
      </template>
    </div>
  </aside>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  accounts: { type: Array, default: () => [] },
  foldersByAccount: { type: Object, default: () => ({}) },
  activeAccountId: { type: String, default: null },
  activeFolder: { type: String, default: 'INBOX' },
  activeLabel: { type: String, default: '' },
  labels: { type: Array, default: () => [] },
  scope: { type: String, default: 'all' },
  unread: { type: Object, default: () => ({}) },
})
defineEmits(['select-account', 'select-folder', 'select-scope', 'compose', 'add-account'])

const folders = computed(() => {
  const list = props.foldersByAccount[props.activeAccountId] || []
  return list.filter((f) => f.selectable !== false)
})

const totalUnread = computed(() =>
  Object.values(props.unread).reduce((sum, n) => sum + (Number(n) || 0), 0),
)

function folderIcon(role) {
  return (
    {
      inbox: '▣',
      sent: '↗',
      drafts: '✎',
      trash: '🗑',
      junk: '⚠',
      archive: '▤',
    }[role] || '▢'
  )
}
</script>

<style scoped>
.mail-account.is-add {
  color: var(--mail-text-muted);
  font-size: 12px;
}
.mail-account.is-add:hover {
  color: var(--mail-accent);
}
</style>
