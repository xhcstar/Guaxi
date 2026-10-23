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

        <div class="mail-section-label is-row">
          <span>文件夹</span>
          <button
            class="mail-section-action"
            title="新建文件夹"
            aria-label="新建文件夹"
            @click="startCreateFolder"
          >
            ＋
          </button>
        </div>
        <button
          v-for="folder in folders"
          :key="folder.name"
          class="mail-folder"
          :class="{ 'is-active': scope === 'folder' && activeFolder === folder.name }"
          @click="$emit('select-folder', folder.name)"
          @contextmenu.prevent="openFolderMenu(folder, $event)"
        >
          <span class="mail-folder-icon">{{ folderIcon(folder.role) }}</span>
          <span class="mail-folder-name" :title="folder.displayName">{{ folder.displayName }}</span>
          <span v-if="folder.unread" class="mail-badge">{{ folder.unread }}</span>
        </button>
        <div v-if="creating" class="mail-folder-create">
          <input
            ref="createInput"
            v-model="newFolderName"
            class="mail-input"
            placeholder="文件夹名，回车创建"
            @keydown.enter.prevent="commitCreateFolder"
            @keydown.esc="cancelCreateFolder"
            @blur="cancelCreateFolder"
          />
        </div>
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

    <!-- 底部：设置入口。Outlook / Thunderbird / Mailspring 都把它放在左下角，
         顶栏留给高频动作，设置属于低频全局配置。 -->
    <div class="mail-sidebar-footer">
      <button class="mail-footer-btn" title="外观与设置" @click="$emit('settings')">
        <span class="mail-footer-icon" aria-hidden="true">⚙</span>
        <span class="mail-footer-label">设置</span>
      </button>
    </div>

    <!-- 文件夹右键菜单：自建文件夹可删除；系统文件夹只给「标记已读」这类安全动作。 -->
    <div
      v-if="folderMenu"
      class="mail-context"
      :style="{ left: `${folderMenu.x}px`, top: `${folderMenu.y}px` }"
      @click.stop
    >
      <button class="mail-context-item" @click="menuAction('select')">打开</button>
      <button class="mail-context-item" @click="menuAction('create')">新建文件夹…</button>
      <div class="mail-context-sep"></div>
      <button
        class="mail-context-item is-danger"
        :disabled="folderMenu.folder.role !== 'custom'"
        :title="folderMenu.folder.role !== 'custom' ? '系统文件夹不能删除' : ''"
        @click="menuAction('delete')"
      >
        删除文件夹
      </button>
    </div>
  </aside>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'

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
const emit = defineEmits([
  'select-account',
  'select-folder',
  'select-scope',
  'compose',
  'add-account',
  'create-folder',
  'delete-folder',
  'settings',
  'drop-on-folder',
])

const creating = ref(false)
const newFolderName = ref('')
const createInput = ref(null)
const folderMenu = ref(null)
/** 拖拽悬停中的文件夹（用于高亮可放区域）。 */
const dropTarget = ref('')

function onDragOver(folder) {
  dropTarget.value = folder.name
}

function onDragLeave(folder) {
  if (dropTarget.value === folder.name) dropTarget.value = ''
}

function onDrop(folder) {
  dropTarget.value = ''
  emit('drop-on-folder', folder.name)
}

async function startCreateFolder() {
  creating.value = true
  newFolderName.value = ''
  await nextTick()
  createInput.value?.focus()
}

function commitCreateFolder() {
  const name = newFolderName.value.trim()
  creating.value = false
  newFolderName.value = ''
  if (name) emit('create-folder', name)
}

function cancelCreateFolder() {
  creating.value = false
  newFolderName.value = ''
}

function openFolderMenu(folder, event) {
  const x = Math.min(event.clientX, window.innerWidth - 200)
  const y = Math.min(event.clientY, window.innerHeight - 160)
  folderMenu.value = { folder, x, y }
  setTimeout(() => document.addEventListener('click', closeFolderMenu, { once: true }), 0)
}

function closeFolderMenu() {
  folderMenu.value = null
}

async function menuAction(action) {
  const folder = folderMenu.value?.folder
  closeFolderMenu()
  if (!folder) return
  if (action === 'select') emit('select-folder', folder.name)
  if (action === 'create') await startCreateFolder()
  if (action === 'delete') emit('delete-folder', folder)
}

onBeforeUnmount(() => document.removeEventListener('click', closeFolderMenu))

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
