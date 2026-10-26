<template>
  <aside class="rail-root">
    <div class="rail-top">
      <button type="button" class="new-task" @click="store.newDraft()">
        <Icon :icon="plusIcon" width="15" height="15" />
        <span>新任务</span>
        <kbd class="new-task-kbd">⌘N</kbd>
      </button>

      <label class="rail-search">
        <Icon :icon="searchIcon" width="13" height="13" class="search-icon" />
        <input
          v-model="search"
          class="search-input"
          type="text"
          placeholder="搜索会话"
          spellcheck="false"
          @input="onSearch"
          @keydown.esc="clearSearch"
        />
        <button v-if="search" type="button" class="search-clear" title="清空" @click="clearSearch">
          <Icon :icon="xIcon" width="11" height="11" />
        </button>
      </label>
    </div>

    <div ref="listRef" class="rail-list" @scroll="onScroll">
      <div v-if="creatingSection" class="section-create">
        <Icon :icon="folderPlusIcon" width="13" height="13" />
        <input
          ref="sectionInput"
          v-model="newSectionName"
          class="section-create-input"
          type="text"
          placeholder="分区名称，Enter 创建"
          @keydown.enter.prevent="submitSection"
          @keydown.esc="cancelSection"
          @blur="cancelSection"
        />
      </div>

      <template v-if="groups.length">
        <div v-for="group in groups" :key="group.id || '__loose__'" class="group">
          <div
            class="group-head"
            :class="{ 'group-head--over': dropSection === group.id, 'group-head--loose': !group.id }"
            @click="toggleGroup(group.id)"
            @dragover.prevent="onDragOver(group.id)"
            @dragleave="onDragLeave(group.id)"
            @drop.prevent="onDrop(group.id)"
          >
            <Icon
              :icon="isGroupOpen(group.id) ? chevronDownIcon : chevronRightIcon"
              width="11"
              height="11"
              class="group-caret"
            />
            <span v-if="group.color" class="group-color" :style="{ background: group.color }" />
            <input
              v-if="renamingSectionId === group.id"
              v-model="sectionNameValue"
              class="group-rename"
              type="text"
              data-section-rename
              @click.stop
              @keydown.enter.prevent="commitSectionRename(group)"
              @keydown.esc.prevent="renamingSectionId = ''"
              @blur="commitSectionRename(group)"
            />
            <span v-else class="group-name">{{ group.name }}</span>
            <span class="group-count">{{ group.threads.length }}</span>
            <button
              v-if="group.id"
              type="button"
              class="group-more"
              title="分区操作"
              @click.stop="openSectionMenu($event, group)"
            >
              <Icon :icon="moreIcon" width="13" height="13" />
            </button>
          </div>

          <div v-if="isGroupOpen(group.id)" class="group-body">
            <div
              v-for="thread in group.threads"
              :key="thread.id"
              class="thread-row"
              :class="{ 'thread-row--active': thread.id === store.currentThreadId }"
              draggable="true"
              @click="pick(thread)"
              @contextmenu.prevent="openThreadMenu($event, thread)"
              @dragstart="onDragStart($event, thread)"
              @dragend="clearDrag"
            >
              <span v-if="isRunning(thread)" class="thread-pulse" aria-hidden="true" />
              <span v-else-if="hasApproval(thread)" class="thread-mark thread-mark--ask" aria-hidden="true" />
              <span v-else-if="isUnread(thread)" class="thread-mark thread-mark--unread" aria-hidden="true" />
              <span v-else class="thread-mark" aria-hidden="true" />

              <div class="thread-main">
                <input
                  v-if="renamingId === thread.id"
                  ref="renameInput"
                  v-model="renameValue"
                  class="thread-rename"
                  type="text"
                  @click.stop
                  @keydown.enter.prevent="commitRename(thread)"
                  @keydown.esc.prevent="cancelRename"
                  @blur="commitRename(thread)"
                />
                <span v-else class="thread-title" :title="thread.name || thread.preview || ''">
                  {{ threadTitle(thread) }}
                </span>
                <span class="thread-sub">
                  <template v-if="thread.cwd">{{ baseName(thread.cwd) }}</template>
                  <template v-else>未选择目录</template>
                  <span v-if="isRunning(thread)" class="thread-sub-state">运行中</span>
                  <span v-else-if="thread.status === 'systemError' || thread.status === 'failed'" class="thread-sub-state thread-sub-state--bad">出错</span>
                </span>
              </div>

              <span class="thread-time">{{ relTime(thread.updatedAt) }}</span>

              <button type="button" class="thread-more" title="更多" @click.stop="openThreadMenu($event, thread)">
                <Icon :icon="moreIcon" width="13" height="13" />
              </button>
            </div>
          </div>
        </div>
      </template>

      <div v-else class="rail-empty">
        <p v-if="search">没有匹配「{{ search }}」的会话</p>
        <p v-else-if="store.view === 'archived'">还没有归档的会话</p>
        <p v-else>还没有会话。发一条消息就会建一个。</p>
      </div>

      <button v-if="canLoadMore" type="button" class="rail-more" @click="loadMore">
        加载更早的会话
      </button>
    </div>

    <div class="rail-foot">
      <button
        type="button"
        class="foot-btn"
        :class="{ 'foot-btn--on': store.view === 'archived' }"
        @click="toggleArchived"
      >
        <Icon :icon="store.view === 'archived' ? archiveRestoreIcon : archiveIcon" width="13" height="13" />
        <span>{{ store.view === 'archived' ? '返回会话' : '已归档' }}</span>
      </button>
      <button type="button" class="foot-btn" title="新建分区" @click="startSection">
        <Icon :icon="folderPlusIcon" width="13" height="13" />
        <span>分区</span>
      </button>

      <button
        type="button"
        class="rail-engine"
        :class="{ 'rail-engine--bad': engineBad, 'rail-engine--ok': engineOk }"
        :title="engineTitle"
        @click="onEngineClick"
      >
        <span class="engine-dot" />
        <span class="engine-text">{{ engineText }}</span>
      </button>
    </div>
    <!-- 自己画右键菜单：才能容下「移动到分区」的二级结构，也更贴近 Codex 的密度。 -->
    <div v-if="menu.open" class="menu-layer" @click="closeMenu" @contextmenu.prevent="closeMenu">
      <div class="rail-menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }" @click.stop>
        <template v-if="menu.kind === 'thread'">
          <template v-if="menu.page === 'main'">
            <button type="button" class="menu-item" @click="menuAction('rename')">
              <Icon :icon="pencilIcon" width="13" height="13" />
              <span>重命名</span>
            </button>
            <button type="button" class="menu-item" @click="menuAction('fork')">
              <Icon :icon="forkIcon" width="13" height="13" />
              <span>分叉出新会话</span>
            </button>
            <button
              v-if="store.view !== 'archived'"
              type="button"
              class="menu-item"
              @click="menu.page = 'sections'"
            >
              <Icon :icon="folderInputIcon" width="13" height="13" />
              <span>移动到分区</span>
              <Icon :icon="chevronRightIcon" width="12" height="12" class="menu-arrow" />
            </button>
            <div class="menu-sep"></div>
            <button
              v-if="store.view !== 'archived'"
              type="button"
              class="menu-item"
              @click="menuAction('archive')"
            >
              <Icon :icon="archiveIcon" width="13" height="13" />
              <span>归档</span>
            </button>
            <button v-else type="button" class="menu-item" @click="menuAction('unarchive')">
              <Icon :icon="archiveRestoreIcon" width="13" height="13" />
              <span>取消归档</span>
            </button>
            <button type="button" class="menu-item menu-item--danger" @click="menuAction('delete')">
              <Icon :icon="trashIcon" width="13" height="13" />
              <span>{{ menu.confirm ? '再点一次确认删除' : '删除会话' }}</span>
            </button>
          </template>

          <template v-else>
            <div class="menu-label">移动到</div>
            <button type="button" class="menu-item" @click="menuAction('move', '')">
              <Icon :icon="folderOpenIcon" width="13" height="13" />
              <span>未分组</span>
              <Icon v-if="!menu.thread?.section?.id" :icon="checkIcon" width="12" height="12" class="menu-check" />
            </button>
            <button
              v-for="section in store.sections"
              :key="section.id"
              type="button"
              class="menu-item"
              @click="menuAction('move', section.id)"
            >
              <span
                class="menu-swatch"
                :style="{ background: section.appearance?.color || 'var(--guaxi-text-secondary)' }"
              />
              <span>{{ section.name }}</span>
              <Icon
                v-if="menu.thread?.section?.id === section.id"
                :icon="checkIcon"
                width="12"
                height="12"
                class="menu-check"
              />
            </button>
            <button type="button" class="menu-item menu-item--quiet" @click="menu.page = 'main'">
              返回
            </button>
          </template>
        </template>

        <template v-else-if="menu.kind === 'section'">
          <button type="button" class="menu-item" @click="menuAction('rename-section')">
            <Icon :icon="pencilIcon" width="13" height="13" />
            <span>重命名分区</span>
          </button>
          <button type="button" class="menu-item" @click="menuAction('new-section')">
            <Icon :icon="folderPlusIcon" width="13" height="13" />
            <span>新建分区</span>
          </button>
          <div class="menu-sep"></div>
          <button type="button" class="menu-item menu-item--danger" @click="menuAction('delete-section')">
            <Icon :icon="trashIcon" width="13" height="13" />
            <span>{{ menu.confirm ? '再点一次确认删除' : '删除分区' }}</span>
          </button>
        </template>
      </div>
    </div>
  </aside>
</template>

<script setup>
/**
 * 会话栏。
 *
 * 一个线程 = 一个 codex thread = 一个工作目录 + 一份沙箱策略。
 * 这里只做「找得到、理得清、收得走」三件事：搜索、分区、归档/删除，
 * 全部直接落在 codex 的线程索引上。
 *
 * 交互取舍：
 * - 重命名用行内输入，不弹对话框——改个名字不该离开当前视线；
 * - 删除要按两次（菜单文案会变成「再点一次确认」），而不是弹确认框；
 * - 拖放只接受落在分区标题上，落在会话上不重排（codex 也不支持手动排序）。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { Icon } from '@iconify/vue'
import plusIcon from '@iconify-icons/lucide/plus'
import searchIcon from '@iconify-icons/lucide/search'
import xIcon from '@iconify-icons/lucide/x'
import moreIcon from '@iconify-icons/lucide/more-horizontal'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import archiveIcon from '@iconify-icons/lucide/archive'
import archiveRestoreIcon from '@iconify-icons/lucide/archive-restore'
import trashIcon from '@iconify-icons/lucide/trash-2'
import forkIcon from '@iconify-icons/lucide/git-fork'
import pencilIcon from '@iconify-icons/lucide/pencil'
import folderPlusIcon from '@iconify-icons/lucide/folder-plus'
import folderOpenIcon from '@iconify-icons/lucide/folder-open'
import folderInputIcon from '@iconify-icons/lucide/folder-input'
import checkIcon from '@iconify-icons/lucide/check'
import { useAgentWorkspaceStore } from '../../stores/agentWorkspaceStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('AgentSessionRail')
const store = useAgentWorkspaceStore()

const search = ref('')
const listRef = ref(null)
const renameInput = ref(null)
const sectionInput = ref(null)

const renamingId = ref('')
const renameValue = ref('')
const renamingSectionId = ref('')
const sectionNameValue = ref('')
const creatingSection = ref(false)
const newSectionName = ref('')

const dropSection = ref(null)
const dragThreadId = ref('')

const collapsedGroups = ref(new Set())

/** 每分钟推进一次，让「3 分钟前」自己变旧。 */
const tick = ref(Date.now())
let tickTimer = null
let searchTimer = null

const menu = reactive({
  open: false,
  x: 0,
  y: 0,
  kind: 'thread',
  page: 'main',
  thread: null,
  section: null,
  confirm: false,
})

const groups = computed(() => store.threadGroups)
const canLoadMore = computed(() => Boolean(
  store.view === 'archived' ? store.archivedCursor : store.nextCursor,
))

const engineBad = computed(() => Boolean(store.engineError) || (store.engine && !store.engine.binary_exists))
const engineOk = computed(() => Boolean(store.engine?.running) && !store.engineError)
const engineText = computed(() => {
  if (store.engineError) return '引擎不可用'
  const e = store.engine
  if (!e) return '正在检测引擎'
  if (!e.binary_exists) return '未找到 codex'
  return e.running ? `codex ${e.binary_version || ''}`.trim() : 'codex 待启动'
})
const engineTitle = computed(() => {
  if (store.engineError) return `${store.engineError}\n点击重试连接`
  return store.engine?.binary_path || store.engine?.hint || ''
})

const baseName = (p) => String(p || '').split(/[\\/]/).filter(Boolean).pop() || ''
const threadTitle = (t) => String(t?.name || t?.preview || '').trim() || '新任务'

function relTime(ms) {
  // 读一下 tick，模板里的「几分钟前」才会随时间重算。
  const stamp = tick.value
  const value = Number(ms || 0)
  if (!value) return ''
  const diff = Math.max(0, stamp - value)
  if (diff < 60_000) return '刚刚'
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 时`
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)} 天`
  return new Date(value).toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' })
}

const isRunning = (t) => store.runningThreadIds.includes(t.id) || t.status === 'active'
const isUnread = (t) => Boolean(store.threadStates[t.id]?.unread) && t.id !== store.currentThreadId
const hasApproval = (t) => store.approvals.some((a) => a.threadId === t.id)

const isGroupOpen = (id) => !collapsedGroups.value.has(id || '__loose__')
const toggleGroup = (id) => {
  const key = id || '__loose__'
  const next = new Set(collapsedGroups.value)
  if (next.has(key)) next.delete(key)
  else next.add(key)
  collapsedGroups.value = next
}

// ───────────── 搜索 / 分页

const onSearch = () => {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => {
    store.searchThreads(search.value).catch((err) => {
      logger.warn('[rail] search failed', { error: String(err?.message || err) })
    })
  }, 180)
}

const clearSearch = () => {
  search.value = ''
  store.searchThreads('').catch(() => {})
}

const onScroll = () => {
  const el = listRef.value
  if (!el) return
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - 60 && canLoadMore.value && !store.loadingMore) {
    store.loadingMore = true
    store.loadMoreThreads()
      .catch((err) => logger.warn('[rail] load more failed', { error: String(err?.message || err) }))
      .finally(() => { store.loadingMore = false })
  }
}

const loadMore = () => {
  store.loadMoreThreads().catch((err) => {
    logger.warn('[rail] load more failed', { error: String(err?.message || err) })
  })
}

const toggleArchived = () => {
  store.setView(store.view === 'archived' ? 'active' : 'archived')
}

const onEngineClick = async () => {
  if (!engineBad.value) return
  try {
    await store.retryEngine()
  } catch (err) {
    logger.warn('[rail] engine retry failed', { error: String(err?.message || err) })
  }
}

const pick = (thread) => {
  if (!thread?.id) return
  store.openThread(thread.id).catch((err) => {
    logger.warn('[rail] open thread failed', { id: thread.id, error: String(err?.message || err) })
  })
}

// ───────────── 重命名

const startRename = (thread) => {
  renamingId.value = thread.id
  renameValue.value = threadTitle(thread)
  nextTick(() => {
    const el = Array.isArray(renameInput.value) ? renameInput.value[0] : renameInput.value
    el?.focus()
    el?.select?.()
  })
}

const commitRename = async (thread) => {
  if (renamingId.value !== thread.id) return
  const value = renameValue.value.trim()
  renamingId.value = ''
  if (!value || value === threadTitle(thread)) return
  try {
    await store.renameThread(thread.id, value)
  } catch (err) {
    logger.warn('[rail] rename failed', { id: thread.id, error: String(err?.message || err) })
  }
}

const cancelRename = () => {
  renamingId.value = ''
  renameValue.value = ''
}

// ───────────── 分区

const startSection = () => {
  creatingSection.value = true
  newSectionName.value = ''
  nextTick(() => sectionInput.value?.focus())
}

const cancelSection = () => {
  creatingSection.value = false
  newSectionName.value = ''
}

const submitSection = async () => {
  const name = newSectionName.value.trim()
  if (!name) {
    cancelSection()
    return
  }
  try {
    await store.createSection(name)
    creatingSection.value = false
    newSectionName.value = ''
  } catch (err) {
    logger.warn('[rail] create section failed', { error: String(err?.message || err) })
  }
}

const startSectionRename = (section) => {
  renamingSectionId.value = section.id
  sectionNameValue.value = section.name
  nextTick(() => {
    const el = document.querySelector('[data-section-rename]')
    if (el instanceof HTMLInputElement) {
      el.focus()
      el.select()
    }
  })
}

const commitSectionRename = async (section) => {
  if (renamingSectionId.value !== section.id) return
  const value = sectionNameValue.value.trim()
  renamingSectionId.value = ''
  if (!value || value === section.name) return
  await store.renameSection(section.id, value).catch((err) => {
    logger.warn('[rail] rename section failed', { error: String(err?.message || err) })
  })
}

// ───────────── 拖放

const onDragStart = (event, thread) => {
  dragThreadId.value = thread.id
  event.dataTransfer.effectAllowed = 'move'
  event.dataTransfer.setData('text/plain', thread.id)
}

const clearDrag = () => {
  dragThreadId.value = ''
  dropSection.value = null
}

const onDragOver = (sectionId) => {
  if (!dragThreadId.value) return
  dropSection.value = sectionId || ''
}

const onDragLeave = (sectionId) => {
  if (dropSection.value === (sectionId || '')) dropSection.value = null
}

const onDrop = async (sectionId) => {
  const threadId = dragThreadId.value
  const target = sectionId || ''
  clearDrag()
  if (!threadId) return
  try {
    await store.moveThreadToSection(threadId, target)
  } catch (err) {
    logger.warn('[rail] move thread failed', { threadId, sectionId: target, error: String(err?.message || err) })
  }
}

// ───────────── 菜单

const openThreadMenu = (event, thread) => {
  const width = 196
  const height = 236
  menu.open = true
  menu.kind = 'thread'
  menu.page = 'main'
  menu.thread = thread
  menu.section = null
  menu.confirm = false
  menu.x = Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8))
  menu.y = Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8))
}

const openSectionMenu = (event, group) => {
  const width = 180
  const height = 140
  menu.open = true
  menu.kind = 'section'
  menu.page = 'main'
  menu.section = { id: group.id, name: group.name }
  menu.thread = null
  menu.confirm = false
  const rect = event.currentTarget.getBoundingClientRect()
  menu.x = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8))
  menu.y = Math.max(8, Math.min(rect.bottom + 4, window.innerHeight - height - 8))
}

const closeMenu = () => {
  menu.open = false
  menu.confirm = false
  menu.page = 'main'
}

const menuAction = async (action, arg = '') => {
  const thread = menu.thread
  const section = menu.section
  const guard = async (fn, label) => {
    try {
      await fn()
      closeMenu()
    } catch (err) {
      logger.warn(`[rail] ${label} failed`, { error: String(err?.message || err) })
    }
  }

  switch (action) {
    case 'rename':
      closeMenu()
      startRename(thread)
      break
    case 'fork':
      await guard(() => store.forkThread(thread.id), 'fork')
      break
    case 'move':
      await guard(() => store.moveThreadToSection(thread.id, arg), 'move thread')
      break
    case 'archive':
      await guard(() => store.archiveThread(thread.id), 'archive')
      break
    case 'unarchive':
      await guard(() => store.unarchiveThread(thread.id), 'unarchive')
      break
    case 'delete':
      if (!menu.confirm) {
        menu.confirm = true
        return
      }
      await guard(() => store.deleteThread(thread.id), 'delete')
      break
    case 'rename-section':
      closeMenu()
      startSectionRename(section)
      break
    case 'new-section':
      closeMenu()
      startSection()
      break
    case 'delete-section':
      if (!menu.confirm) {
        menu.confirm = true
        return
      }
      await guard(() => store.deleteSection(section.id), 'delete section')
      break
    default:
      closeMenu()
  }
}

// ───────────── 生命周期

const onKeydown = (event) => {
  if (menu.open && event.key === 'Escape') {
    closeMenu()
    return
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'n') {
    event.preventDefault()
    store.newDraft()
  }
}

onMounted(() => {
  tickTimer = setInterval(() => { tick.value = Date.now() }, 60_000)
  window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
  if (tickTimer) clearInterval(tickTimer)
  if (searchTimer) clearTimeout(searchTimer)
  window.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
.rail-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  border-right: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
}

.rail-top {
  flex: 0 0 auto;
  padding: 10px 10px 6px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.new-task {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 32px;
  padding: 0 9px;
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-size: 12.5px;
  font-family: inherit;
  cursor: pointer;
  transition: border-color 120ms ease;
}

.new-task:hover {
  border-color: var(--guaxi-text-secondary);
}

.new-task-kbd {
  margin-left: auto;
  padding: 1px 5px;
  border: 1px solid var(--guaxi-border);
  border-radius: 4px;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  line-height: 1.4;
  opacity: 0.7;
}

.rail-search {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: var(--guaxi-hover-bg);
  transition: border-color 120ms ease, background 120ms ease;
}

.rail-search:focus-within {
  border-color: var(--guaxi-border);
  background: var(--guaxi-content-bg);
}

.search-icon {
  flex: 0 0 auto;
  opacity: 0.55;
}

.search-input {
  flex: 1 1 auto;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-family: inherit;
  font-size: 12px;
}

.search-input::placeholder {
  color: var(--guaxi-text-secondary);
  opacity: 0.65;
}

.search-clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
}

.search-clear:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.rail-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 6px 12px;
  scrollbar-width: thin;
}

.group + .group {
  margin-top: 2px;
}

.group-head {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 6px;
  border-radius: 5px;
  cursor: pointer;
  user-select: none;
  transition: background 120ms ease, box-shadow 120ms ease;
}

.group-head:hover {
  background: var(--guaxi-hover-bg);
}

.group-head--over {
  background: rgba(180, 125, 26, 0.1);
  box-shadow: inset 0 0 0 1px rgba(180, 125, 26, 0.4);
}

.group-caret {
  flex: 0 0 auto;
  opacity: 0.5;
}

.group-color {
  flex: 0 0 auto;
  width: 6px;
  height: 6px;
  border-radius: 2px;
}

.group-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  opacity: 0.72;
}

.group-head--loose .group-name {
  text-transform: none;
  letter-spacing: 0.02em;
  font-size: 11px;
}

.group-rename {
  flex: 1 1 auto;
  min-width: 0;
  height: 18px;
  padding: 0 4px;
  border: 1px solid #b47d1a;
  border-radius: 3px;
  outline: none;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
}

.group-count {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  opacity: 0.4;
}

.group-more {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  opacity: 0;
  cursor: pointer;
  transition: opacity 120ms ease, background 120ms ease;
}

.group-head:hover .group-more {
  opacity: 0.8;
}

.group-more:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.group-body {
  padding: 1px 0 2px;
}

.thread-row {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 7px;
  padding: 5px 6px 5px 7px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 120ms ease;
}

.thread-row:hover,
.thread-row--active {
  background: var(--guaxi-hover-bg);
}

.thread-row--active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 2px;
  border-radius: 1px;
  background: #b47d1a;
}

.thread-mark {
  flex: 0 0 auto;
  width: 5px;
  height: 5px;
  margin-top: 7px;
  border-radius: 50%;
  background: var(--guaxi-text-secondary);
  opacity: 0.18;
}

.thread-mark--unread {
  background: #b47d1a;
  opacity: 0.9;
}

.thread-mark--ask {
  width: 6px;
  height: 6px;
  margin-top: 6px;
  border-radius: 2px;
  background: #b47d1a;
  opacity: 1;
  animation: rail-ask 2s ease-in-out infinite;
}

.thread-pulse {
  flex: 0 0 auto;
  width: 6px;
  height: 6px;
  margin-top: 6px;
  border-radius: 50%;
  background: #b47d1a;
  animation: rail-pulse 1.6s ease-out infinite;
}

@keyframes rail-pulse {
  0% { box-shadow: 0 0 0 0 rgba(180, 125, 26, 0.4); }
  70% { box-shadow: 0 0 0 5px rgba(180, 125, 26, 0); }
  100% { box-shadow: 0 0 0 0 rgba(180, 125, 26, 0); }
}

@keyframes rail-ask {
  0%, 100% { opacity: 0.55; }
  50% { opacity: 1; }
}

.thread-main {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.thread-title {
  font-size: 12.5px;
  line-height: 1.4;
  color: var(--guaxi-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thread-row:hover .thread-title,
.thread-row--active .thread-title {
  color: var(--guaxi-text-primary);
}

.thread-sub {
  display: flex;
  align-items: center;
  gap: 6px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  line-height: 1.4;
  color: var(--guaxi-text-secondary);
  opacity: 0.55;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thread-sub-state {
  color: #b47d1a;
  opacity: 1;
}

.thread-sub-state--bad {
  color: #b42318;
}

.thread-time {
  flex: 0 0 auto;
  margin-top: 1px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.45;
}

.thread-row:hover .thread-time {
  opacity: 0;
}

.thread-more {
  position: absolute;
  right: 4px;
  top: 4px;
  display: none;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  cursor: pointer;
}

.thread-row:hover .thread-more {
  display: inline-flex;
}

.thread-more:hover {
  color: var(--guaxi-text-primary);
}

.thread-rename {
  width: 100%;
  height: 20px;
  padding: 0 4px;
  border: 1px solid #b47d1a;
  border-radius: 4px;
  outline: none;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-family: inherit;
  font-size: 12.5px;
}

.section-create {
  display: flex;
  align-items: center;
  gap: 7px;
  height: 28px;
  margin: 2px 6px 4px;
  padding: 0 8px;
  border: 1px dashed var(--guaxi-border);
  border-radius: 6px;
  color: var(--guaxi-text-secondary);
}

.section-create-input {
  flex: 1 1 auto;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-family: inherit;
  font-size: 12px;
}

.rail-empty {
  padding: 26px 12px;
  text-align: center;
}

.rail-empty p {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary);
  opacity: 0.65;
}

.rail-more {
  display: block;
  width: calc(100% - 12px);
  margin: 8px 6px 0;
  padding: 6px 0;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
}

.rail-more:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.rail-foot {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 7px 8px;
  border-top: 1px solid var(--guaxi-border);
}

.foot-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 7px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 11px;
  font-family: inherit;
  cursor: pointer;
  transition: background 120ms ease, color 120ms ease;
}

.foot-btn:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.foot-btn--on {
  color: #b47d1a;
  background: rgba(180, 125, 26, 0.08);
}

.rail-engine {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
  min-width: 0;
  height: 24px;
  padding: 0 6px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  cursor: default;
}

.rail-engine--bad {
  color: #b42318;
  cursor: pointer;
}

.engine-dot {
  flex: 0 0 auto;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--guaxi-text-secondary);
  opacity: 0.4;
}

.rail-engine--ok .engine-dot {
  background: #4f8a5b;
  opacity: 1;
}

.rail-engine--bad .engine-dot {
  background: #b42318;
  opacity: 1;
}

.engine-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu-layer {
  position: fixed;
  inset: 0;
  z-index: 900;
}

.rail-menu {
  position: fixed;
  width: 196px;
  padding: 4px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.22);
  animation: menu-in 110ms ease-out;
}

@keyframes menu-in {
  from { opacity: 0; transform: translateY(-3px) scale(0.985); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 28px;
  padding: 0 8px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-size: 12px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
}

.menu-item:hover {
  background: var(--guaxi-hover-bg);
}

.menu-item--danger {
  color: #b42318;
}

.menu-item--quiet {
  margin-top: 2px;
  color: var(--guaxi-text-secondary);
  font-size: 11px;
}

.menu-arrow,
.menu-check {
  margin-left: auto;
  opacity: 0.6;
}

.menu-swatch {
  width: 7px;
  height: 7px;
  border-radius: 2px;
  opacity: 0.8;
}

.menu-sep {
  height: 1px;
  margin: 4px 6px;
  background: var(--guaxi-border);
}

.menu-label {
  padding: 4px 8px 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
  letter-spacing: 0.07em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
}

@media (prefers-reduced-motion: reduce) {
  .thread-pulse,
  .thread-mark--ask,
  .rail-menu {
    animation: none;
  }
}
</style>
