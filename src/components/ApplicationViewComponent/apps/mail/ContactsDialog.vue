<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal is-wide contacts-modal" role="dialog" aria-label="通讯录">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">通讯录</h2>
        <span class="contacts-count">{{ book.stats.total }} 位联系人</span>
        <div class="contacts-head-actions">
          <button class="mail-btn" @click="rebuild">从邮件重建</button>
          <button class="mail-btn" @click="importFile">导入 vCard</button>
          <button class="mail-btn" @click="exportAll">导出 vCard</button>
          <button class="mail-btn is-primary" @click="startNewContact">＋ 新建联系人</button>
          <button class="mail-modal-close" @click="$emit('close')">×</button>
        </div>
      </div>

      <div class="contacts-body">
        <!-- 左：分组 -->
        <aside class="contacts-groups">
          <button
            class="mail-folder"
            :class="{ 'is-active': !book.activeGroupId }"
            @click="openAll()"
          >
            <span class="mail-folder-icon">◉</span>
            <span class="mail-folder-name">全部联系人</span>
            <span class="mail-badge">{{ book.stats.total }}</span>
          </button>
          <button
            v-for="group in book.groups"
            :key="group.id"
            class="mail-folder"
            :class="{ 'is-active': book.activeGroupId === group.id }"
            @click="openGroup(group.id)"
          >
            <span class="mail-folder-icon">▣</span>
            <span class="mail-folder-name" :title="group.name">{{ group.name }}</span>
            <span class="mail-badge">{{ book.groupCounts[group.id] ?? group.members.length }}</span>
          </button>
          <button class="mail-folder contacts-add-group" @click="startNewGroup">＋ 新建通讯组</button>

          <div class="contacts-filters">
            <button
              class="mail-chip"
              :class="{ 'is-on': book.query.starredOnly }"
              @click="book.toggleStarFilter(!book.query.starredOnly)"
            >
              星标
            </button>
            <button
              class="mail-chip"
              :class="{ 'is-on': book.query.frequentOnly }"
              @click="book.toggleFrequent(!book.query.frequentOnly)"
            >
              常联系
            </button>
            <button
              class="mail-chip"
              :class="{ 'is-on': book.query.source === 'auto' }"
              @click="toggleSource('auto')"
            >
              自动收录
            </button>
          </div>
        </aside>

        <!-- 中：联系人列表 -->
        <section class="contacts-list">
          <div class="contacts-search">
            <input
              :value="book.query.text"
              placeholder="搜索姓名、邮箱、公司、电话…"
              @input="onSearch($event.target.value)"
            />
            <button v-if="book.query.text" class="mail-search-clear" @click="clearSearch">×</button>
          </div>

          <div class="contacts-scroll">
            <div v-if="!book.contacts.length && !book.loading" class="mail-empty">
              <div class="mail-empty-title">没有匹配的联系人</div>
              <div>换个关键词，或者从邮件重建通讯录。</div>
            </div>
            <template v-for="section in book.sections" :key="section.letter">
              <div class="contacts-section">{{ section.letter }}</div>
              <button
                v-for="contact in section.items"
                :key="contact.id"
                class="contacts-row"
                @click="book.open(contact.id)"
              >
                <span
                  class="mail-avatar"
                  :style="{ background: `hsl(${hueOf(contact)} 42% 48%)` }"
                >
                  {{ initialOf(contact) }}
                </span>
                <span class="contacts-row-main">
                  <span class="contacts-row-name">
                    {{ displayOf(contact) }}
                    <span v-if="contact.starred" class="contacts-star">★</span>
                  </span>
                  <span class="contacts-row-sub">
                    {{ contact.emails?.[0]?.value || '（无邮箱）' }}
                    <template v-if="contact.organization"> · {{ contact.organization }}</template>
                  </span>
                </span>
                <span class="contacts-row-meta">
                  <span v-if="contact.unreadCount" class="mail-badge">{{ contact.unreadCount }}</span>
                  <span v-else-if="contact.messageCount" class="contacts-count-mini">
                    {{ contact.messageCount }}
                  </span>
                </span>
              </button>
            </template>
          </div>
        </section>

        <!-- 右：详情 -->
        <section class="contacts-detail">
          <div v-if="editing" class="contacts-detail-scroll">
            <ContactEditor
              :contact="draftContact"
              :groups="book.groups"
              :saving="saving"
              @save="saveContact"
              @cancel="editing = false"
            />
          </div>

          <div v-else-if="editingGroup" class="contacts-detail-scroll">
            <GroupEditor
              :group="draftGroup"
              :contacts="book.contacts"
              :groups="book.groups"
              :saving="saving"
              @save="saveGroup"
              @cancel="editingGroup = false"
            />
          </div>

          <template v-else-if="book.active">
            <div class="contacts-detail-head">
              <span
                class="mail-avatar contacts-detail-avatar"
                :style="{ background: `hsl(${hueOf(book.active)} 42% 48%)` }"
              >
                {{ initialOf(book.active) }}
              </span>
              <div class="contacts-detail-title">
                <h3>{{ displayOf(book.active) }}</h3>
                <p>
                  {{ book.active.organization || '—' }}
                  <template v-if="book.active.title"> · {{ book.active.title }}</template>
                </p>
              </div>
              <div class="contacts-detail-actions">
                <button
                  class="mail-icon-btn"
                  :class="{ 'is-on': book.active.starred }"
                  :title="book.active.starred ? '取消星标' : '加星标'"
                  @click="toggleStar(book.active)"
                >
                  ★
                </button>
                <button class="mail-btn" @click="startEditContact(book.active)">编辑</button>
                <button class="mail-btn is-danger" @click="removeContact(book.active)">删除</button>
              </div>
            </div>

            <div class="contacts-detail-scroll">
              <dl class="contacts-fields">
                <div v-for="(email, index) in book.active.emails" :key="`e${index}`">
                  <dt>{{ labelText(email.label, '邮箱') }}</dt>
                  <dd>
                    <button class="contacts-link" @click="$emit('compose-to', email.value)">
                      {{ email.value }}
                    </button>
                    <button
                      class="contacts-copy"
                      title="复制"
                      @click="copy(email.value)"
                    >
                      复制
                    </button>
                  </dd>
                </div>
                <div v-for="(phone, index) in book.active.phones" :key="`p${index}`">
                  <dt>{{ labelText(phone.label, '电话') }}</dt>
                  <dd>{{ phone.value }}</dd>
                </div>
                <div v-if="book.active.address">
                  <dt>地址</dt>
                  <dd>{{ book.active.address }}</dd>
                </div>
                <div v-if="book.active.birthday">
                  <dt>生日</dt>
                  <dd>{{ book.active.birthday }}</dd>
                </div>
                <div v-if="book.active.website">
                  <dt>网址</dt>
                  <dd>{{ book.active.website }}</dd>
                </div>
                <div v-if="book.active.note">
                  <dt>备注</dt>
                  <dd class="contacts-note">{{ book.active.note }}</dd>
                </div>
              </dl>

              <div class="contacts-activity">
                <div class="contacts-activity-head">
                  <span>邮件往来</span>
                  <span class="contacts-count-mini">
                    {{ book.active.messageCount || 0 }} 封
                    <template v-if="book.active.unreadCount">
                      · {{ book.active.unreadCount }} 未读
                    </template>
                  </span>
                </div>
                <div v-if="book.activeLoading" class="mail-spin"></div>
                <div v-else-if="!book.activeMessages.length" class="mail-hint">
                  还没有往来邮件。同步收件箱后这里会显示。
                </div>
                <button
                  v-for="message in book.activeMessages"
                  :key="`${message.accountId}:${message.folder}:${message.uid}`"
                  class="contacts-message"
                  @click="$emit('open-message', message)"
                >
                  <span class="contacts-message-dir" :class="message.direction">
                    {{ message.direction === 'in' ? '收' : '发' }}
                  </span>
                  <span class="contacts-message-main">
                    <span class="contacts-message-subject">{{ message.subject || '（无主题）' }}</span>
                    <span class="contacts-message-snippet">{{ message.snippet }}</span>
                  </span>
                  <span class="contacts-message-date">{{ dateLabel(message.date) }}</span>
                </button>
              </div>
            </div>
          </template>

          <template v-else-if="groupEditingView">
            <div class="contacts-detail-scroll">
              <GroupMembers
                :group="book.groupsById[book.activeGroupId]"
                @compose-to="(email) => $emit('compose-to', email)"
                @compose-group="(group) => $emit('compose-group', group)"
                @edit="startEditGroup"
              />
            </div>
          </template>

          <div v-else class="mail-reader-placeholder contacts-placeholder">
            从左侧选一个联系人查看详情，<br />或者新建一位。
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { useContactsStore, blankContact, blankGroup } from '../../../../stores/contactsStore'
import ContactEditor from './ContactEditor.vue'
import GroupEditor from './GroupEditor.vue'
import GroupMembers from './GroupMembers.vue'

const props = defineProps({
  /** 打开时是否直接进入「新建联系人」。 */
  startNew: { type: Boolean, default: false },
})
const emit = defineEmits(['close', 'compose-to', 'compose-group', 'open-message'])

const book = useContactsStore()
const editing = ref(false)
const editingGroup = ref(false)
const saving = ref(false)
const draftContact = reactive(blankContact())
const draftGroup = reactive(blankGroup())

// 选中某个通讯组、且没有打开具体联系人时，右栏显示组成员。
// 直接读 store 字段，别用 book.activeGroupId 这种「非响应式解构」的写法。
const groupEditingView = computed(
  () => !editing.value && !editingGroup.value && Boolean(book.activeGroupId) && !book.active,
)

onMounted(async () => {
  await book.init()
  if (props.startNew) startNewContact()
  else if (book.activeGroupId) await book.reload()
})

function displayOf(contact) {
  return (
    contact.displayName ||
    [contact.familyName, contact.givenName].filter(Boolean).join('') ||
    contact.emails?.[0]?.value ||
    '（未命名）'
  )
}

function initialOf(contact) {
  const name = displayOf(contact)
  return (name[0] || '?').toUpperCase()
}

function hueOf(contact) {
  const source = contact.id || displayOf(contact)
  let hash = 0
  for (let i = 0; i < source.length; i += 1) hash = (hash * 31 + source.charCodeAt(i)) % 360
  return hash
}

function labelText(label, fallback) {
  return { work: '工作', home: '私人', phone: '手机', other: fallback }[label] || fallback
}

function dateLabel(unix) {
  if (!unix) return ''
  const date = new Date(unix * 1000)
  const now = new Date()
  if (date.toDateString() === now.toDateString()) {
    return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })
  }
  return date.toLocaleDateString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit' })
}

function onSearch(value) {
  clearTimeout(onSearch.timer)
  onSearch.timer = setTimeout(() => book.setSearch(value), 160)
}

function clearSearch() {
  book.setSearch('')
}

function toggleSource(source) {
  book.query.source = book.query.source === source ? null : source
  book.reload()
}

// ---------------- 联系人编辑 ----------------

/** 切到某个通讯组：清掉联系人详情，右栏改为显示组成员。 */
async function openGroup(id) {
  book.closeActive()
  editing.value = false
  editingGroup.value = false
  await book.selectGroup(id)
}

function openAll() {
  book.closeActive()
  editing.value = false
  editingGroup.value = false
  book.selectGroup('')
}

function startNewContact() {
  Object.assign(draftContact, blankContact())
  editing.value = true
  editingGroup.value = false
}

function startEditContact(contact) {
  Object.assign(draftContact, JSON.parse(JSON.stringify(contact)))
  editing.value = true
  editingGroup.value = false
}

async function saveContact(contact) {
  saving.value = true
  try {
    await book.save(contact)
    editing.value = false
  } finally {
    saving.value = false
  }
}

async function toggleStar(contact) {
  await book.save({ ...contact, starred: !contact.starred })
}

async function removeContact(contact) {
  if (!window.confirm(`删除联系人「${displayOf(contact)}」？`)) return
  await book.remove(contact.id)
}

// ---------------- 通讯组 ----------------

function startNewGroup() {
  Object.assign(draftGroup, blankGroup())
  editingGroup.value = true
  editing.value = false
}

function startEditGroup() {
  const group = book.groupsById[book.activeGroupId]
  if (!group) return
  Object.assign(draftGroup, JSON.parse(JSON.stringify(group)))
  editingGroup.value = true
  editing.value = false
}

async function saveGroup(group) {
  saving.value = true
  try {
    const saved = await book.saveGroup(group)
    editingGroup.value = false
    await book.selectGroup(saved.id)
  } catch (error) {
    // 例如组之间形成了循环引用，后端会拒绝；要让用户看到原因而不是静默失败。
    window.alert(String(error?.message || error))
  } finally {
    saving.value = false
  }
}

// ---------------- 导入导出 ----------------

async function importFile() {
  try {
    const report = await book.importFromFile()
    if (!report) return
    const parts = [`导入 ${report.imported} 位`, `合并 ${report.merged} 位`]
    if (report.groups) parts.push(`通讯组 ${report.groups} 个`)
    if (report.skipped) parts.push(`跳过 ${report.skipped} 条`)
    window.alert(parts.join('，') + (report.errors?.length ? `\n\n${report.errors.join('\n')}` : ''))
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

async function exportAll() {
  try {
    const count = await book.exportToFile(book.activeGroupId || null)
    window.alert(count ? `已导出 ${count} 位联系人` : '已导出通讯组（成员为直接邮箱）')
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

async function rebuild() {
  if (!window.confirm('从已有邮件重新收录联系人对已有联系人没有影响，确定继续？')) return
  try {
    const touched = await book.rebuildFromMail()
    window.alert(`已扫描 ${touched} 封邮件`)
  } catch (error) {
    window.alert(String(error?.message || error))
  }
}

async function copy(text) {
  try {
    const { writeText } = await import('@tauri-apps/plugin-clipboard-manager')
    await writeText(text)
  } catch {
    navigator.clipboard?.writeText?.(text)
  }
}
</script>

<style scoped>
.contacts-modal {
  width: min(1120px, 95vw);
  max-height: 90vh;
}

.contacts-count {
  font-size: 11.5px;
  color: var(--mail-text-muted);
}

.contacts-head-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}

.contacts-body {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 168px minmax(220px, 300px) minmax(0, 1fr);
}

.contacts-groups {
  border-right: 1px solid var(--mail-border);
  padding: 8px 6px;
  overflow-y: auto;
  background: var(--mail-sidebar);
}

.contacts-add-group {
  color: var(--mail-text-muted);
  font-size: 12px;
}
.contacts-add-group:hover {
  color: var(--mail-accent);
}

.contacts-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 10px 6px 4px;
  border-top: 1px solid var(--mail-border);
  margin-top: 10px;
}
.contacts-filters .mail-chip {
  font-size: 11px;
  padding: 1px 8px;
}

.contacts-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-right: 1px solid var(--mail-border);
}

.contacts-search {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--mail-border);
}
.contacts-search input {
  flex: 1 1 auto;
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  font-size: 12.5px;
}

.contacts-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}

.contacts-section {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 4px 10px;
  font-size: 11px;
  font-weight: 600;
  color: var(--mail-text-muted);
  background: var(--mail-panel);
  border-bottom: 1px solid var(--mail-border);
}

.contacts-row {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 7px 10px;
  border: 0;
  border-bottom: 1px solid var(--mail-border);
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.contacts-row:hover {
  background: var(--mail-panel-hover);
}

.contacts-row-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1 1 auto;
  gap: 1px;
}
.contacts-row-name {
  font-size: 12.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-row-sub {
  font-size: 11px;
  color: var(--mail-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-star {
  color: var(--mail-star);
  font-size: 10px;
}
.contacts-row-meta {
  flex: 0 0 auto;
}
.contacts-count-mini {
  font-size: 10.5px;
  color: var(--mail-text-muted);
  font-variant-numeric: tabular-nums;
}

.contacts-detail {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

/* 编辑器 / 组编辑器是详情栏的唯一子节点：撑满并自己滚动，行动按钮才会留在可见区。 */
.contacts-detail > .contacts-detail-scroll {
  flex: 1 1 auto;
  min-height: 0;
}

.contacts-detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--mail-border);
}
.contacts-detail-avatar {
  width: 36px;
  height: 36px;
  font-size: 14px;
}
.contacts-detail-title {
  min-width: 0;
  flex: 1 1 auto;
}
.contacts-detail-title h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-detail-title p {
  margin: 2px 0 0;
  font-size: 11.5px;
  color: var(--mail-text-muted);
}
.contacts-detail-actions {
  display: flex;
  gap: 4px;
  align-items: center;
}

.contacts-detail-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 14px 16px 24px;
}

.contacts-fields {
  margin: 0 0 18px;
}
.contacts-fields > div {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  gap: 10px;
  padding: 5px 0;
  border-bottom: 1px solid var(--mail-border);
}
.contacts-fields dt {
  font-size: 11px;
  color: var(--mail-text-muted);
  padding-top: 1px;
}
.contacts-fields dd {
  margin: 0;
  font-size: 12.5px;
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}
.contacts-note {
  white-space: pre-wrap;
  line-height: 1.65;
}

.contacts-link {
  border: 0;
  background: transparent;
  color: var(--mail-accent);
  font: inherit;
  font-size: 12.5px;
  padding: 0;
  cursor: pointer;
  text-align: left;
  word-break: break-all;
}
.contacts-link:hover {
  text-decoration: underline;
}

.contacts-copy {
  border: 0;
  background: transparent;
  color: var(--mail-text-muted);
  font: inherit;
  font-size: 10.5px;
  cursor: pointer;
  padding: 0;
}
.contacts-copy:hover {
  color: var(--mail-text);
}

.contacts-activity {
  border-top: 1px solid var(--mail-border);
  padding-top: 12px;
}
.contacts-activity-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 12px;
  font-weight: 600;
  margin-bottom: 6px;
}

.contacts-message {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  padding: 6px 2px;
  border: 0;
  border-bottom: 1px solid var(--mail-border);
  background: transparent;
  color: var(--mail-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.contacts-message:hover {
  background: var(--mail-panel-hover);
}
.contacts-message-dir {
  flex: 0 0 auto;
  width: 18px;
  height: 18px;
  border-radius: 4px;
  display: grid;
  place-items: center;
  font-size: 10px;
  background: var(--mail-bg-sunken);
  color: var(--mail-text-muted);
}
.contacts-message-dir.in {
  background: var(--mail-accent-soft);
  color: var(--mail-accent);
}
.contacts-message-main {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.contacts-message-subject {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-message-snippet {
  font-size: 10.5px;
  color: var(--mail-text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-message-date {
  flex: 0 0 auto;
  font-size: 10.5px;
  color: var(--mail-text-muted);
  font-variant-numeric: tabular-nums;
}

.contacts-placeholder {
  margin: auto;
  text-align: center;
}

@media (max-width: 1000px) {
  .contacts-body {
    grid-template-columns: 140px minmax(0, 1fr);
  }
  .contacts-detail {
    display: none;
  }
}
</style>
