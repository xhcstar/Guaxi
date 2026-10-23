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
        <!-- 标准文件夹：按角色固定分区，与 Mailspring / Outlook 的侧栏一致。
             顺序是刻意的——用户最常看的收件箱在最上，草稿与已发送紧跟，
             垃圾箱与存档这类低频入口沉底。 -->
        <div class="mail-section-label">邮箱</div>
        <button
          v-for="entry in standardFolders"
          :key="entry.key"
          class="mail-folder"
          :class="{ 'is-active': isFolderActive(entry) }"
          @click="selectStandard(entry)"
          @contextmenu.prevent="entry.folder && openFolderMenu(entry.folder, $event)"
        >
          <span class="mail-folder-icon">{{ entry.icon }}</span>
          <span class="mail-folder-name">{{ entry.label }}</span>
          <span v-if="entry.unread" class="mail-badge">{{ entry.unread }}</span>
        </button>

        <div class="mail-section-label">快速查看</div>
        <button
          class="mail-folder"
          :class="{ 'is-active': scope === 'unread' }"
          @click="$emit('select-scope', 'unread')"
        >
          <span class="mail-folder-icon">●</span>
          <span class="mail-folder-name">未读</span>
          <span v-if="quickCounts.unread" class="mail-badge">{{ quickCounts.unread }}</span>
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

        <!-- 用户自建的文件夹单独成区，避免与标准文件夹混在一起分不清 -->
        <template v-if="customFolders.length">
          <div class="mail-section-label is-row">
            <span>我的文件夹</span>
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
            v-for="folder in customFolders"
            :key="folder.name"
            class="mail-folder"
            :class="{ 'is-active': scope === 'folder' && activeFolder === folder.name }"
            @click="$emit('select-folder', folder.name)"
            @contextmenu.prevent="openFolderMenu(folder, $event)"
          >
            <span class="mail-folder-icon">▢</span>
            <span class="mail-folder-name" :title="folder.displayName">{{ folder.displayName }}</span>
            <span v-if="folder.unread" class="mail-badge">{{ folder.unread }}</span>
          </button>
        </template>
        <button
          v-else
          class="mail-folder mail-folder-add"
          title="新建文件夹"
          @click="startCreateFolder"
        >
          <span class="mail-folder-icon">＋</span>
          <span class="mail-folder-name">新建文件夹</span>
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

/** 标准文件夹的固定顺序与显示名。服务器没建时后端会补占位项，这里再兜一次。 */
const STANDARD_ROLES = [
  { role: 'inbox', label: '收件箱', icon: '▣' },
  { role: 'drafts', label: '草稿', icon: '✎' },
  { role: 'sent', label: '已发送', icon: '↗' },
  { role: 'archive', label: '存档', icon: '▤' },
  { role: 'junk', label: '垃圾邮件', icon: '⚠' },
  { role: 'trash', label: '已删除', icon: '🗑' },
]

/**
 * 标准文件夹条目。
 *
 * 只要账户存在就展示全部角色——即使服务器上还没有那个文件夹。
 * Mailspring / Outlook 都是这样：位置是固定的，用户不该因为服务商差异
 * 而在侧栏里「找不到草稿箱」。
 */
const standardFolders = computed(() => {
  const byRole = new Map()
  for (const folder of folders.value) {
    // 同角色若出现多个（如「存档」和「存档/2024」），取名字最短的那个当入口。
    const current = byRole.get(folder.role)
    if (!current || folder.name.length < current.name.length) byRole.set(folder.role, folder)
  }
  return STANDARD_ROLES.filter((entry) => entry.role !== 'archive' || byRole.has('archive')).map(
    (entry) => {
      const folder = byRole.get(entry.role) || null
      return {
        key: entry.role,
        role: entry.role,
        label: entry.label,
        icon: entry.icon,
        unread: folder?.unread || 0,
        folder,
      }
    },
  )
})

/** 用户自建文件夹：排除掉已被标准分区占用的角色与名字。 */
const customFolders = computed(() => {
  const standardNames = new Set(
    standardFolders.value.filter((e) => e.folder).map((e) => e.folder.name),
  )
  return folders.value.filter(
    (f) => f.role === 'custom' && !standardNames.has(f.name),
  )
})

/** 快速查看里的未读角标：只统计收件箱，避免和账户徽标重复计数。 */
const quickCounts = computed(() => {
  const inbox = folders.value.find((f) => f.role === 'inbox')
  return { unread: inbox?.unread || 0 }
})

function isFolderActive(entry) {
  // 标准分区用 scope（跨文件夹按角色聚合），而不是锁定某个具体文件夹名。
  if (entry.role === 'inbox') {
    return props.scope === 'folder' && props.activeFolder === entry.folder?.name
  }
  return props.scope === entry.role
}

function selectStandard(entry) {
  if (entry.role === 'inbox' && entry.folder) {
    emit('select-folder', entry.folder.name)
    return
  }
  emit('select-scope', entry.role)
}

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
