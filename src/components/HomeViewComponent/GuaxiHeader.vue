<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon size="20" class="header-icon" :component="PanelLeftContract24Regular" @click="toggleLeftSider"></n-icon>
        </template>
        切换左侧边栏
      </n-tooltip>
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 中间：搜索框 -->
    <div class="header-center">
      <div ref="searchContainerRef" class="search-container" :class="{ open: isSearchDropdownVisible }">
        <n-input ref="searchInputRef" v-model:value="searchQuery" placeholder="搜索文件或内容..." round clearable
          @keydown="handleSearchKeydown" @focus="showSearchOptions" @blur="handleSearchBlur">
          <template #prefix>
            <n-icon :component="Search32Filled" />
          </template>
          <template #suffix>
            <span
              class="search-filter-btn"
              :class="{ active: hasActiveFilters }"
              title="筛选：时间 / 文件类型 / 标签"
              @mousedown.prevent
              @click="toggleFilters"
            >
              <svg viewBox="0 0 24 24" width="15" height="15">
                <path fill="currentColor" d="M3 4.5h18l-7.2 8.4v5.1l-3.6 1.8v-6.9z" />
              </svg>
              <span v-if="activeFilterCount" class="search-filter-badge">{{ activeFilterCount }}</span>
            </span>
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="BookAdd20Regular" @click="openCreateKnowledgeBase" />
        </template>
        {{ headerShortcutTooltip('新建知识库', 'createKnowledgeBase') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="TextBulletListLtr20Filled" @click="openOutlinePanel"></n-icon>
        </template>
        {{ headerShortcutTooltip('大纲', 'openOutline') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" @click="openAssistantPanel">
            <Icon :icon="assistantIcon" width="18" height="18" />
          </n-icon>
        </template>
        {{ headerShortcutTooltip('智能助手', 'openAssistant') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="Box20Regular" @click="openTemplateManager" />
        </template>
        {{ headerShortcutTooltip('模板管理', 'openTemplateManager') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" @click="openSlidevManager">
            <svg viewBox="0 0 24 24" width="18" height="18">
              <path fill="currentColor" d="M4 5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v8.5a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5zm2 0v8.5h12V5H6zm-1 14a1 1 0 0 1 1-1h4v-1h4v1h4a1 1 0 1 1 0 2H6a1 1 0 0 1-1-1z" />
            </svg>
          </n-icon>
        </template>
        PPT 制作与管理
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="Folder20Regular" @click="openResourceRepoManager" />
        </template>
        {{ headerShortcutTooltip('知识库资源管理', 'openResourceRepoManager') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="TaskListSquareLtr20Regular" @click="openTaskManager" />
        </template>
        任务管理面板
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" @click="openLanguageLearningWindow">
            <svg viewBox="0 0 24 24" width="18" height="18">
              <path fill="none" stroke="currentColor" stroke-width="1.8" d="M4 5.5A2.5 2.5 0 0 1 6.5 3H11v16H6.5A2.5 2.5 0 0 0 4 21V5.5zm16 0A2.5 2.5 0 0 0 17.5 3H13v16h4.5A2.5 2.5 0 0 1 20 21V5.5zM8 8h2m-2 3h2m6-3h-2m2 3h-2" />
            </svg>
          </n-icon>
        </template>
        语言学习
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="Screenshot24Regular" @click="CaptureScreenshot"></n-icon>
        </template>
        {{ headerShortcutTooltip('截图并插入', 'captureScreenshot') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" @click="splitVertical">
            <svg viewBox="0 0 16 16" width="16" height="16">
              <rect x="1.5" y="2.5" width="13" height="11" rx="1" ry="1" fill="none" stroke="currentColor" stroke-width="1" />
              <line x1="8" y1="3" x2="8" y2="13" stroke="currentColor" stroke-width="1" />
            </svg>
          </n-icon>
        </template>
        {{ headerShortcutTooltip('纵向分屏', 'splitVertical') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" @click="splitHorizontal">
            <svg viewBox="0 0 16 16" width="16" height="16">
              <rect x="1.5" y="2.5" width="13" height="11" rx="1" ry="1" fill="none" stroke="currentColor" stroke-width="1" />
              <line x1="2" y1="8" x2="14" y2="8" stroke="currentColor" stroke-width="1" />
            </svg>
          </n-icon>
        </template>
        {{ headerShortcutTooltip('横向分屏', 'splitHorizontal') }}
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="PanelRightContract24Regular" @click="toggleRightSider"></n-icon>
        </template>
        切换右侧边栏
      </n-tooltip>

    </div>
  </div>

  <teleport to="body">
    <div
      v-if="isSearchDropdownVisible"
      ref="searchDropdownRef"
      class="search-dd-root"
      :style="dropdownFloatingStyle"
      @mousedown.prevent
    >
      <div class="search-dd-section">
        <div
          v-for="item in typeMenuItems"
          :key="item.key"
          class="search-dd-row search-dd-row-type"
          :class="{ active: item.key === searchType, 'is-selected': selectedRowKey === item.key }"
          :data-row-key="item.key"
          @click="handleSearchSelect(item.key)"
        >
          <span class="search-dd-icon-wrap">
            <n-icon :component="item.icon" />
          </span>
          <span class="search-dd-row-text">{{ item.label }}</span>
        </div>
        <div class="search-dd-divider" />
      </div>

      <!-- 筛选：时间段 / 文件类型(多选) / 标签 -->
      <div v-if="filtersOpen" class="search-dd-section search-filters">
        <div class="filter-group">
          <div class="filter-label">时间段</div>
          <div class="filter-chips">
            <span
              v-for="t in TIME_PRESETS"
              :key="t.key"
              class="filter-chip"
              :class="{ on: filterTimePreset === t.key }"
              @click="setTimePreset(t.key)"
            >{{ t.label }}</span>
          </div>
        </div>
        <div v-if="filterTimePreset === 'custom'" class="filter-dates">
          <input type="date" v-model="filterCustomStart" class="filter-date" @mousedown.stop />
          <span class="filter-dash">~</span>
          <input type="date" v-model="filterCustomEnd" class="filter-date" @mousedown.stop />
        </div>

        <div class="filter-group">
          <div class="filter-label">文件类型</div>
          <div class="filter-chips">
            <span
              v-for="e in COMMON_EXTS"
              :key="e.ext"
              class="filter-chip"
              :class="{ on: filterExts.includes(e.ext) }"
              @click="toggleExt(e.ext)"
            >{{ e.label }}</span>
            <span
              v-for="e in customExts"
              :key="'custom-' + e"
              class="filter-chip on"
              @click="toggleExt(e)"
            >.{{ e }} ✕</span>
            <input
              v-model="customExtInput"
              class="filter-ext-input"
              placeholder="自定义后缀↵"
              @keyup.enter="addCustomExt"
              @mousedown.stop
            />
          </div>
        </div>

        <div class="filter-group">
          <div class="filter-label">标签</div>
          <div class="filter-chips">
            <span v-if="!availableTags.length" class="filter-empty">该知识库暂无标签</span>
            <span
              v-for="t in availableTags.slice(0, 60)"
              :key="t.tag"
              class="filter-chip"
              :class="{ on: filterTags.includes(t.tag) }"
              @click="toggleTag(t.tag)"
            >{{ t.tag }}<i class="filter-chip-count">{{ t.count }}</i></span>
          </div>
          <label v-if="filterTags.length > 1" class="filter-matchall">
            <input type="checkbox" v-model="filterTagMatchAll" @mousedown.stop />
            <span>同时满足所有标签</span>
          </label>
        </div>

        <div class="filter-actions">
          <span class="filter-hint">回车应用筛选</span>
          <span v-if="hasActiveFilters" class="filter-clear" @click="clearFilters">清除筛选</span>
        </div>
        <div class="search-dd-divider" />
      </div>

      <div v-if="historyItems.length" class="search-dd-section">
        <div class="search-dd-section-title">最近搜索</div>
        <div
          v-for="item in historyItems"
          :key="`history:${item._historyKey}`"
          class="search-dd-row"
          :class="{ 'is-selected': selectedRowKey === `historyResult:${item._historyKey}` }"
          :data-row-key="`historyResult:${item._historyKey}`"
          @click="handleSearchSelect(`historyResult:${item._historyKey}`)"
        >
          <div class="search-dd-item">
            <div class="search-dd-main" :title="getItemRel(item)">
              <span class="search-dd-icon-wrap">
                <Icon :icon="getItemIcon(item)" width="16" height="16" class="search-dd-icon" />
              </span>
              <span class="search-dd-filename">{{ getItemFilename(item) }}</span>
              <span class="search-dd-path">{{ getItemRelDir(item) }}</span>
            </div>
            <div v-if="item.preview" class="search-dd-preview">
              {{ String(item.preview).slice(0, 160) }}
            </div>
          </div>
        </div>
        <div v-if="trimmedQuery" class="search-dd-divider" />
      </div>

      <div v-if="!trimmedQuery" class="search-dd-section">
        <div v-if="!historyItems.length" class="search-dd-empty">暂无最近搜索</div>
      </div>

      <div v-else class="search-dd-section">
        <div v-if="previewLoading" class="search-dd-empty">搜索中…</div>
        <div v-else-if="previewError" class="search-dd-empty">{{ previewError }}</div>
        <div v-else-if="!previewHasAnyResults" class="search-dd-empty">无匹配结果</div>
        <template v-else>
          <template v-if="previewResultsByType.file.length">
            <div class="search-dd-section-title">搜索文件</div>
            <div
              v-for="item in previewResultsByType.file.slice(0, 10)"
              :key="makeResultKey(item)"
              class="search-dd-row"
              :class="{ 'is-selected': selectedRowKey === makeResultKey(item) }"
              :data-row-key="makeResultKey(item)"
              @click="handleSearchSelect(makeResultKey(item))"
            >
              <div class="search-dd-item">
                <div class="search-dd-main" :title="getItemRel(item)">
                  <span class="search-dd-icon-wrap">
                    <Icon :icon="getItemIcon(item)" width="16" height="16" class="search-dd-icon" />
                  </span>
                  <span class="search-dd-filename">{{ getItemFilename(item) }}</span>
                  <span class="search-dd-path">{{ getItemRelDir(item) }}</span>
                </div>
              </div>
            </div>
          </template>

          <template v-if="previewResultsByType.note.length">
            <div class="search-dd-section-title">搜索关键字</div>
            <div
              v-for="item in previewResultsByType.note.slice(0, 10)"
              :key="makeResultKey(item)"
              class="search-dd-row"
              :class="{ 'is-selected': selectedRowKey === makeResultKey(item) }"
              :data-row-key="makeResultKey(item)"
              @click="handleSearchSelect(makeResultKey(item))"
            >
              <div class="search-dd-item">
                <div class="search-dd-main" :title="getItemRel(item)">
                  <span class="search-dd-icon-wrap">
                    <Icon :icon="getItemIcon(item)" width="16" height="16" class="search-dd-icon" />
                  </span>
                  <span class="search-dd-filename">{{ getItemFilename(item) }}</span>
                  <span class="search-dd-path">{{ getItemRelDir(item) }}</span>
                </div>
                <div v-if="item.preview" class="search-dd-preview">
                  {{ String(item.preview).slice(0, 160) }}
                </div>
              </div>
            </div>
          </template>

          <template v-if="previewResultsByType.excerpt.length">
            <div class="search-dd-section-title">搜索摘录笔记</div>
            <div
              v-for="item in previewResultsByType.excerpt.slice(0, 10)"
              :key="makeResultKey(item)"
              class="search-dd-row"
              :class="{ 'is-selected': selectedRowKey === makeResultKey(item) }"
              :data-row-key="makeResultKey(item)"
              @click="handleSearchSelect(makeResultKey(item))"
            >
              <div class="search-dd-item">
                <div class="search-dd-main" :title="getItemRel(item)">
                  <span class="search-dd-icon-wrap">
                    <Icon :icon="getItemIcon(item)" width="16" height="16" class="search-dd-icon" />
                  </span>
                  <span class="search-dd-filename">{{ getItemFilename(item) }}</span>
                  <span class="search-dd-path">{{ getItemRelDir(item) }}</span>
                </div>
              </div>
            </div>
          </template>
        </template>
      </div>
    </div>
  </teleport>

  <CreateKnowledgeBaseForm />
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, ref, h, onMounted, onUnmounted, nextTick, watch } from "vue";
import { NIcon, NTooltip } from 'naive-ui'
import { useSiderStore } from '../../stores/useSiderStore'
import { useSiderHotkeys } from '../../composables/useSiderHotkeys'
import { GUAXI_SHORTCUTS, shortcutTooltip, useGuaxiHotkeys } from '../../composables/useGuaxiHotkeys'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useEditorStore } from '../../stores/editorStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import CreateKnowledgeBaseForm from '../BrainViewComponent/CreateKnowledgeBaseForm.vue'

import { useMessage } from 'naive-ui';
import { debounce } from 'lodash-es'
import { Icon } from '@iconify/vue'
import assistantIcon from '@iconify-icons/mdi/robot-excited-outline'
import {
  Screenshot24Regular,
  Search32Filled,
  TextBulletListLtr20Filled,
  DocumentSearch20Filled,
  DocumentSearch24Regular,
  NotebookSection24Regular,
  BookAdd20Regular,
  Box20Regular,
  Folder20Regular,
  TaskListSquareLtr20Regular,
  PanelLeftContract24Regular,
  PanelRightContract24Regular
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

const logger = createLogger('GuaxiHeader')
const rightSiderStore = useRightSiderStore()
const settingsStore = useSettingsStore()
const siderStore = useSiderStore();
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const searchInputRef = ref(null)
const searchContainerRef = ref(null)
const searchDropdownRef = ref(null)
const searchType = ref('file')

const viewKey = 'home'
const message = useMessage();
const appWindow = getCurrentWindow();

const RESOURCE_REPO_SYNC_STATUS_KEY = 'guaxi:resourceRepoSyncStatus:v1'
const RESOURCE_REPO_SYNC_PENDING_KEY = 'guaxi:resourceRepoSyncPending:v1'
const headerShortcutTooltip = (label, shortcutKey) => shortcutTooltip(label, GUAXI_SHORTCUTS[shortcutKey])

const nowLabel = () => {
  try {
    return new Date().toLocaleString()
  } catch {
    return ''
  }
}

const formatInvokeError = (e) => {
  const msg = e?.message ?? (typeof e?.toString === 'function' ? e.toString() : String(e))
  const s = typeof msg === 'string' ? msg.trim() : ''
  return s || '操作失败'
}

const readSyncState = () => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY)
    if (!raw) return { status: 'idle', action: '', at: '', detail: '' }
    const v = JSON.parse(raw)
    return {
      status: String(v?.status || 'idle'),
      action: String(v?.action || ''),
      at: String(v?.at || ''),
      detail: String(v?.detail || ''),
    }
  } catch {
    return { status: 'idle', action: '', at: '', detail: '' }
  }
}

const setSyncState = (patch) => {
  const base = readSyncState()
  const next = { ...base, ...patch }
  try {
    localStorage.setItem(RESOURCE_REPO_SYNC_STATUS_KEY, JSON.stringify(next))
  } catch (e) {
    logger.warn('[GuaxiHeader] write sync state failed', e)
  }

  try {
    window.dispatchEvent(
      new CustomEvent('guaxi:resource-repo-sync-state', {
        detail: next,
      })
    )
  } catch (e) {
    logger.warn('[GuaxiHeader] dispatch sync state event failed', e)
  }

  logger.info('[GuaxiHeader] resource repo sync state', next)
}

const autoSyncBusy = ref(false)

const closeRequested = ref(false)

const runResourceRepoSync = async (reason) => {
  if (autoSyncBusy.value) return
  autoSyncBusy.value = true

  const action = reason === 'shutdown' ? '自动同步（关闭）' : '自动同步（启动）'

  try {
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    logger.info('[GuaxiHeader] resource repo auto sync start', { reason })

    const res = await invoke('resource_repo_sync_now')
    const text = typeof res === 'string' ? res : '同步完成'

    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
    logger.info('[GuaxiHeader] resource repo auto sync success', { reason, text })
  } catch (e) {
    const msg = formatInvokeError(e)
    const skipped = msg.includes('未启用资源库同步后端')

    logger.warn('[GuaxiHeader] resource repo auto sync failed', { reason, msg, raw: e })

    setSyncState({
      status: skipped ? 'skipped' : 'error',
      action,
      at: nowLabel(),
      detail: msg,
    })
  } finally {
    autoSyncBusy.value = false
  }
}

const initResourceRepoAutoSync = async () => {
  try {
    setTimeout(() => {
      if (closeRequested.value) return

      let pending = false
      try {
        pending = localStorage.getItem(RESOURCE_REPO_SYNC_PENDING_KEY) === '1'
      } catch {}

      if (pending) {
        try {
          localStorage.removeItem(RESOURCE_REPO_SYNC_PENDING_KEY)
        } catch {}
      }

      runResourceRepoSync(pending ? 'startup-pending' : 'startup').catch(() => {})
    }, 800)
  } catch {}

}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const waitForWindowMinimized = async (timeoutMs = 1200, settleMs = 260) => {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const minimized = await appWindow.isMinimized();
      if (minimized) {
        logger.debug('[Screenshot] Window minimized, waiting extra settle time before capture');
        await sleep(settleMs);
        logger.debug('[Screenshot] Window minimized and settled, ready to capture');
        return true;
      }
    } catch (e) {
      logger.warn('[Screenshot] Failed to check isMinimized:', e);
      break;
    }
    await sleep(50);
  }
  logger.warn('[Screenshot] Wait for minimized timed out, continue to capture');
  return false;
};

const splitVertical = () => {
  if (!fileTreeStore.activeTab) {
    message.warning('请先打开一个文件');
    return;
  }
  fileTreeStore.splitCurrentTab('vertical');
};

const splitHorizontal = () => {
  if (!fileTreeStore.activeTab) {
    message.warning('请先打开一个文件');
    return;
  }
  fileTreeStore.splitCurrentTab('horizontal');
};

const toggleLeftSider = () => {
  siderStore.toggleLeftSider(viewKey)
}
const toggleRightSider = () => {
  siderStore.toggleRightSider(viewKey)
}

useSiderHotkeys({
  toggleLeft: toggleLeftSider,
  toggleRight: toggleRightSider,
})

function openOutlinePanel() {
  const isCollapsed = siderStore.isRightSiderCollapsed?.[viewKey]
  const isVisible = isCollapsed === false
  const isOutline = rightSiderStore.panel === 'outline'

  if (isVisible && rightSiderStore.isOpen && isOutline) {
    rightSiderStore.close()
    siderStore.setRightSiderCollapsed(viewKey, true)
    return
  }

  rightSiderStore.setPanel('outline')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(viewKey, false)
}

function openAssistantPanel() {
  const isCollapsed = siderStore.isRightSiderCollapsed?.[viewKey]
  const isVisible = isCollapsed === false
  const isAssistant = rightSiderStore.panel === 'assistant'

  if (isVisible && rightSiderStore.isOpen && isAssistant) {
    rightSiderStore.close()
    siderStore.setRightSiderCollapsed(viewKey, true)
    return
  }

  rightSiderStore.setPanel('assistant')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(viewKey, false)
}

const openCreateKnowledgeBase = () => {
  logger.info('打开新建知识库表单')
  knowledgeBaseStore.toggleForm(true)
}

async function openTemplateManager() {
  try {
    await invoke('create_template_manager_window', {
      label: 'template_manager',
      title: '模板管理器',
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('模板管理器窗口已打开')
      return
    }
    logger.error('打开模板管理器失败:', err)
    message.error('打开模板管理器失败')
  }
}

async function openSlidevManager() {
  try {
    await invoke('create_slidev_manager_window', {
      label: 'slidev_manager',
      title: 'PPT 制作与管理',
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('PPT 窗口已打开')
      return
    }
    logger.error('打开 PPT 窗口失败:', err)
    message.error('打开 PPT 窗口失败')
  }
}

async function openResourceRepoManager() {
  try {
    await invoke('create_resource_repo_manager_window', {
      label: 'resource_repo_manager',
      title: '资源库管理',
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('资源库管理窗口已打开')
      return
    }
    logger.error('打开资源库管理失败:', err)
    message.error('打开资源库管理失败')
  }
}

async function openTaskManager() {
  try {
    await invoke('create_task_manager_window', {
      label: 'task_manager',
      title: '任务管理面板',
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('任务管理面板已打开')
      return
    }
    logger.error('打开任务管理面板失败:', err)
    message.error('打开任务管理面板失败')
  }
}

async function openLanguageLearningWindow() {
  try {
    await invoke('create_language_learning_window', {
      label: 'language_learning',
      title: '语言学习中心',
      theme: settingsStore.appearanceTheme,
    })
  } catch (err) {
    const msg = String(err?.message || err || '')
    if (msg.includes('已存在')) {
      message.info('语言学习窗口已打开')
      return
    }
    logger.error('打开语言学习窗口失败:', err)
    message.error('打开语言学习窗口失败')
  }
}

async function CaptureScreenshot() {
  try {
    try {
      logger.debug('[Screenshot] Minimizing window before capture')
      await appWindow.minimize()
      await waitForWindowMinimized()
    } catch (minErr) {
      logger.warn('[Screenshot] Failed to minimize window:', minErr)
    }

    const originalPath = fileTreeStore.selectedFile?.path
    logger.debug('[Screenshot] Triggered', {
      activeTab: fileTreeStore.activeTab,
      selectedPath: originalPath
    })

    if (!originalPath) {
      message.warning('请先打开一个文件')
      logger.warn('[Screenshot] Aborted: no selected file path')
      return
    }

    logger.debug('[Screenshot] Invoking take_screenshot_markdown', {
      ownerFilePath: originalPath,
      alt: 'screenshot'
    })

    const result = await invoke('take_screenshot_markdown', {
      ownerFilePath: originalPath,
      alt: 'screenshot'
    })

    logger.debug('[Screenshot] Backend returned', {
      relative_path: result?.relative_path,
      absolute_path: result?.absolute_path,
      url: result?.url,
      markdown_len: typeof result?.markdown === 'string' ? result.markdown.length : null
    })

    const markdown = result?.markdown
    if (typeof markdown !== 'string' || !markdown) {
      throw new Error('后端未返回 markdown')
    }

    logger.debug('[Screenshot] Inserting markdown into editor')
    const inserted = await editorStore.insertContent(`${markdown}\n`)
    logger.debug('[Screenshot] Insert result', { inserted })

    if (!inserted) {
      throw new Error('插入失败：编辑器未就绪')
    }
  } catch (err) {
    logger.error('[Screenshot] Failed:', err)
    message.error('截图失败')
  } finally {
    try {
      logger.debug('[Screenshot] Restoring window after capture')
      await appWindow.unminimize()
      await appWindow.show()
      await appWindow.setFocus()
    } catch (restoreErr) {
      logger.warn('[Screenshot] Failed to restore window:', restoreErr)
    }
  }
}

// 设计说明：Header 图标快捷键只映射到已有 click 处理函数，不复制业务逻辑。
// 这样工具栏点击与键盘触发始终保持同一条执行路径，后续调整按钮行为时不需要同步两份实现。
useGuaxiHotkeys([
  {
    id: 'createKnowledgeBase',
    label: '新建知识库',
    shortcut: GUAXI_SHORTCUTS.createKnowledgeBase,
    handler: openCreateKnowledgeBase,
  },
  {
    id: 'openAssistant',
    label: '智能助手',
    shortcut: GUAXI_SHORTCUTS.openAssistant,
    handler: openAssistantPanel,
  },
  {
    id: 'splitVertical',
    label: '纵向分屏',
    shortcut: GUAXI_SHORTCUTS.splitVertical,
    handler: splitVertical,
  },
  {
    id: 'splitHorizontal',
    label: '横向分屏',
    shortcut: GUAXI_SHORTCUTS.splitHorizontal,
    handler: splitHorizontal,
  },
  {
    id: 'openOutline',
    label: '大纲',
    shortcut: GUAXI_SHORTCUTS.openOutline,
    handler: openOutlinePanel,
  },
  {
    id: 'captureScreenshot',
    label: '截图并插入',
    shortcut: GUAXI_SHORTCUTS.captureScreenshot,
    handler: CaptureScreenshot,
  },
  {
    id: 'openTemplateManager',
    label: '模板管理',
    shortcut: GUAXI_SHORTCUTS.openTemplateManager,
    handler: openTemplateManager,
  },
  {
    id: 'openResourceRepoManager',
    label: '知识库资源管理',
    shortcut: GUAXI_SHORTCUTS.openResourceRepoManager,
    handler: openResourceRepoManager,
  },
], {
  logger,
  scope: 'GuaxiHeaderShortcuts',
})

const isSearchDropdownVisible = ref(false)
const dropdownPosition = ref({ x: 0, y: 0 })
const searchQuery = ref('')
const dropdownWidth = ref(300)
const dropdownFloatingStyle = computed(() => {
  const w = Math.min(800, Math.max(320, Number(dropdownWidth.value) || 0))
  return {
    position: 'fixed',
    left: `${dropdownPosition.value.x}px`,
    top: `${dropdownPosition.value.y}px`,
    width: `${w}px`,
    zIndex: 3000
  }
})

const trimmedQuery = computed(() => searchQuery.value.trim())

// ───── 搜索筛选：时间段 / 文件类型(多选) / 标签 ─────
const filtersOpen = ref(false)
const filterTimePreset = ref('any')
const filterCustomStart = ref('')
const filterCustomEnd = ref('')
const filterExts = ref([])
const filterTags = ref([])
const filterTagMatchAll = ref(false)
const availableTags = ref([])
const customExtInput = ref('')

const TIME_PRESETS = [
  { key: 'any', label: '全部' },
  { key: 'today', label: '今天' },
  { key: '7d', label: '近 7 天' },
  { key: '30d', label: '近 30 天' },
  { key: '365d', label: '近一年' },
  { key: 'custom', label: '自定义' },
]

const COMMON_EXTS = [
  { ext: 'md', label: 'Markdown' },
  { ext: 'txt', label: '文本' },
  { ext: 'pdf', label: 'PDF' },
  { ext: 'docx', label: 'Word' },
  { ext: 'xlsx', label: 'Excel' },
  { ext: 'pptx', label: 'PPT' },
  { ext: 'png', label: 'PNG' },
  { ext: 'jpg', label: 'JPG' },
  { ext: 'svg', label: 'SVG' },
  { ext: 'json', label: 'JSON' },
  { ext: 'html', label: 'HTML' },
  { ext: 'css', label: 'CSS' },
  { ext: 'tex', label: 'TeX' },
  { ext: 'typ', label: 'Typst' },
]
const COMMON_EXT_SET = new Set(COMMON_EXTS.map((e) => e.ext))
const customExts = computed(() => filterExts.value.filter((e) => !COMMON_EXT_SET.has(e)))

// 搜索根目录：跟随左栏文件树当前模式(知识库/文件/任务)的激活根目录;
// 未设置时回退到知识库激活路径。摘录笔记搜索除外(固定为文件库目录)。
const searchRootPath = computed(
  () => fileTreeStore.sidebarNavigationRootPath || fileTreeStore.activeRepoPath
)
const activeKbName = computed(() => basename(searchRootPath.value))

const timeRange = computed(() => {
  const now = Date.now()
  const day = 86400000
  switch (filterTimePreset.value) {
    case 'today': {
      const d = new Date()
      d.setHours(0, 0, 0, 0)
      return { after: d.getTime(), before: null }
    }
    case '7d': return { after: now - 7 * day, before: null }
    case '30d': return { after: now - 30 * day, before: null }
    case '365d': return { after: now - 365 * day, before: null }
    case 'custom': {
      const a = filterCustomStart.value ? new Date(filterCustomStart.value + 'T00:00:00').getTime() : null
      const b = filterCustomEnd.value ? new Date(filterCustomEnd.value + 'T23:59:59').getTime() : null
      return { after: Number.isFinite(a) ? a : null, before: Number.isFinite(b) ? b : null }
    }
    default: return { after: null, before: null }
  }
})

const extensionsParam = computed(() => (filterExts.value.length ? [...filterExts.value] : null))

const hasActiveFilters = computed(
  () => filterExts.value.length > 0 || filterTags.value.length > 0 || filterTimePreset.value !== 'any'
)
const activeFilterCount = computed(
  () =>
    (filterTimePreset.value !== 'any' ? 1 : 0) +
    (filterExts.value.length ? 1 : 0) +
    (filterTags.value.length ? 1 : 0)
)

const searchFilterArgs = computed(() => ({
  extensions: extensionsParam.value,
  modifiedAfter: timeRange.value.after,
  modifiedBefore: timeRange.value.before,
}))

const scheduleFilterSearch = () => {
  if (!isSearchDropdownVisible.value) return
  updateDropdownPosition()
  debouncedRunPreviewSearch()
}

const toggleFilters = async () => {
  await showSearchOptions()
  filtersOpen.value = !filtersOpen.value
  if (filtersOpen.value) loadAvailableTags()
  nextTick(() => {
    updateDropdownPosition()
    searchInputRef.value?.focus?.()
  })
}

const setTimePreset = (key) => {
  filterTimePreset.value = key
  scheduleFilterSearch()
}
const toggleExt = (ext) => {
  const i = filterExts.value.indexOf(ext)
  if (i >= 0) filterExts.value.splice(i, 1)
  else filterExts.value.push(ext)
  scheduleFilterSearch()
}
const addCustomExt = () => {
  const raw = String(customExtInput.value || '').trim().replace(/^\./, '').toLowerCase()
  customExtInput.value = ''
  if (!raw) return
  if (!filterExts.value.includes(raw)) filterExts.value.push(raw)
  scheduleFilterSearch()
}
const toggleTag = (tag) => {
  const i = filterTags.value.indexOf(tag)
  if (i >= 0) filterTags.value.splice(i, 1)
  else filterTags.value.push(tag)
  scheduleFilterSearch()
}
const clearFilters = () => {
  filterTimePreset.value = 'any'
  filterCustomStart.value = ''
  filterCustomEnd.value = ''
  filterExts.value = []
  filterTags.value = []
  filterTagMatchAll.value = false
  scheduleFilterSearch()
}

const loadAvailableTags = async () => {
  const kb = activeKbName.value
  if (!kb) {
    availableTags.value = []
    return
  }
  try {
    const tags = await invoke('kg_all_tags', { kbName: kb })
    availableTags.value = Array.isArray(tags) ? tags : []
  } catch (e) {
    availableTags.value = []
    logger.warn('[GuaxiHeader] load tags failed', e)
  }
}

const resolveTagPathSet = async () => {
  if (!filterTags.value.length) return null
  const kb = activeKbName.value
  if (!kb) return new Set()
  try {
    const paths = await invoke('kg_files_by_tags', {
      kbName: kb,
      tags: [...filterTags.value],
      matchAll: filterTagMatchAll.value,
    })
    return new Set((Array.isArray(paths) ? paths : []).map((p) => normalizeSlashes(p).toLowerCase()))
  } catch (e) {
    logger.warn('[GuaxiHeader] files by tags failed', e)
    return new Set()
  }
}

const filterByTagPaths = (items, tagSet) => {
  if (!tagSet) return items
  return items.filter((it) => tagSet.has(normalizeSlashes(it.path || '').toLowerCase()))
}

// 监听筛选项变化，自动刷新下拉预览
watch(
  [filterTimePreset, filterCustomStart, filterCustomEnd, filterExts, filterTags, filterTagMatchAll],
  () => scheduleFilterSearch(),
  { deep: true }
)

watch(trimmedQuery, (q) => {
  if (q) return

  // 查询为空但仍有筛选条件时，保持(并刷新)结果，不清空。
  if (hasActiveFilters.value) {
    scheduleFilterSearch()
    return
  }

  if (editorStore.highlightInfo) {
    editorStore.highlightInfo = null
  }

  previewResultsByType.value = { file: [], note: [], excerpt: [] }
  previewResults.value = []
  previewError.value = ''
  previewHasMore.value = false
  selectedRowKey.value = ''

  try {
    /*
     * Design notes:
     * 1. Clearing the top search box must also clear the right search panel.
     * 2. Use the store-level reset instead of only blanking the query string.
     * 3. This keeps file/note/excerpt/error/loading state in sync.
     */
    rightSiderStore.clearSearchResults()
    logger.info('[GuaxiHeader] cleared search query and reset right sider search results')
  } catch (e) {
    logger.warn('[GuaxiHeader] clear right sider search query failed', e)
  }
})

const SEARCH_TYPE_KEYS = ['file', 'note', 'excerpt']
const SEARCH_TYPE_LABELS = {
  file: '搜索文件',
  note: '搜索关键字',
  excerpt: '搜索摘录笔记'
}

const typeMenuItems = computed(() => ([
  { key: 'file', label: SEARCH_TYPE_LABELS.file, icon: DocumentSearch20Filled },
  { key: 'note', label: SEARCH_TYPE_LABELS.note, icon: DocumentSearch24Regular },
  { key: 'excerpt', label: SEARCH_TYPE_LABELS.excerpt, icon: NotebookSection24Regular }
]))

const basename = (p) => String(p || '').split(/[\\/]/).pop() || ''
const dirnameFromPath = (p) => {
  const parts = String(p || '').replace(/\\\\/g, '/').split('/')
  parts.pop()
  return parts.join('/')
}
const normalizeSlashes = (p) => String(p || '').replace(/\\\\/g, '/')
const isCellmetaPath = (p) => normalizeSlashes(p).toLowerCase().endsWith('.cellmeta')
const toRelativePath = (absPath, basePath) => {
  const abs = normalizeSlashes(absPath)
  const base = normalizeSlashes(basePath)
  if (!base) return abs
  if (abs === base) return ''
  if (abs.startsWith(base + '/')) return abs.slice(base.length + 1)
  return abs
}

const HISTORY_STORAGE_KEY = 'guaxi.home.searchResultHistory.v1'
const searchHistory = ref((() => {
  try {
    const raw = localStorage.getItem(HISTORY_STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed) ? parsed : []
  } catch {
    return []
  }
})())

const persistSearchHistory = () => {
  try {
    localStorage.setItem(HISTORY_STORAGE_KEY, JSON.stringify(searchHistory.value.slice(0, 120)))
  } catch {}
}

const historyKeyToEntry = computed(() => {
  const map = new Map()
  for (const e of searchHistory.value) {
    if (e && e.key) map.set(e.key, e)
  }
  return map
})

const historyEntryToItem = (entry) => {
  const t = entry?.type
  if (t === 'excerpt') {
    const filePath = String(entry.file_path || '')
    return {
      file_path: filePath,
      file_name: String(entry.file_name || basename(filePath)),
      expanded: false,
      loading: false,
      _searchType: 'excerpt',
      _historyKey: String(entry.key || '')
    }
  }

  const path = String(entry?.path || '')
  const item = {
    path,
    file_name: String(entry?.file_name || basename(path)),
    preview: entry?.preview ? String(entry.preview) : undefined,
    line_number: Number(entry?.line_number) || 0,
    matches: [],
    _searchType: t === 'note' ? 'note' : 'file',
    _historyKey: String(entry?.key || '')
  }
  return item
}

function getHistorySuggestions(input) {
  const q = String(input || '').trim().toLowerCase()
  const now = Date.now()

  const scored = searchHistory.value
    .filter(e => e && e.key)
    .map(e => {
      const t = e.type
      const absPath = t === 'excerpt' ? String(e.file_path || '') : String(e.path || '')
      const base = t === 'excerpt' ? settingsStore.librariesRoot : searchRootPath.value
      const rel = toRelativePath(absPath, base)
      const hay = `${String(e.file_name || '')} ${rel}`.toLowerCase()

      if (q && !hay.includes(q)) return null

      const hours = Math.floor((now - Number(e.lastUsed || 0)) / 3600000)
      const recency = Math.max(0, 72 - hours)
      const freq = Math.min(50, Number(e.count || 0))
      const score = freq * 10 + recency
      return { ...e, _score: score }
    })
    .filter(Boolean)
    .sort((a, b) => b._score - a._score)

  const uniq = new Map()
  for (const e of scored) {
    if (!uniq.has(e.key)) uniq.set(e.key, e)
  }

  return Array.from(uniq.values()).slice(0, 6).map(historyEntryToItem)
}

const historyItems = computed(() => getHistorySuggestions(trimmedQuery.value))

const selectedRowKey = ref('')
const flashSelectRow = async (key) => {
  selectedRowKey.value = key
  await new Promise(resolve => setTimeout(resolve, 140))
  if (selectedRowKey.value === key) selectedRowKey.value = ''
}

const dropdownSelectableKeys = computed(() => {
  const keys = []
  for (const t of typeMenuItems.value) keys.push(t.key)

  for (const item of historyItems.value) {
    const key = item?._historyKey ? `historyResult:${item._historyKey}` : ''
    if (key) keys.push(key)
  }

  if (!trimmedQuery.value) return keys

  const fileItems = (previewResultsByType.value.file || []).slice(0, 10)
  const noteItems = (previewResultsByType.value.note || []).slice(0, 10)
  const excerptItems = (previewResultsByType.value.excerpt || []).slice(0, 10)

  for (const item of fileItems) keys.push(makeResultKey(item))
  for (const item of noteItems) keys.push(makeResultKey(item))
  for (const item of excerptItems) keys.push(makeResultKey(item))

  return keys
})

const normalizeDropdownSelection = () => {
  if (!isSearchDropdownVisible.value) return
  const keys = dropdownSelectableKeys.value
  if (!keys.length) {
    selectedRowKey.value = ''
    return
  }
  if (!keys.includes(selectedRowKey.value)) selectedRowKey.value = keys[0]
}

const moveDropdownSelection = (delta) => {
  const keys = dropdownSelectableKeys.value
  if (!keys.length) return

  const cur = keys.indexOf(selectedRowKey.value)
  if (cur < 0) {
    selectedRowKey.value = delta >= 0 ? keys[0] : keys[keys.length - 1]
    return
  }

  const next = (cur + delta + keys.length) % keys.length
  selectedRowKey.value = keys[next]
}

const handleSearchKeydown = (e) => {
  if (e?.isComposing) return
  if (!isSearchDropdownVisible.value) return

  const key = e?.key
  if (key === 'ArrowDown') {
    e.preventDefault()
    moveDropdownSelection(1)
    return
  }

  if (key === 'ArrowUp') {
    e.preventDefault()
    moveDropdownSelection(-1)
    return
  }

  if (key === 'Enter') {
    e.preventDefault()
    if (selectedRowKey.value) {
      handleSearchSelect(selectedRowKey.value)
      return
    }
    if (trimmedQuery.value) handleSearchEnter()
  }
}

const scrollDropdownSelectionIntoView = async () => {
  if (!isSearchDropdownVisible.value) return

  const key = selectedRowKey.value
  if (!key) return

  await nextTick()

  const root = searchDropdownRef.value
  if (!root) return

  // 将当前选中行滚动到可视区域内
  const rows = Array.from(root.querySelectorAll('[data-row-key]'))
  const target = rows.find(row => row.getAttribute('data-row-key') === key)
  if (!target) return

  logger.debug('[GuaxiHeader] scroll dropdown selection', { key })
  target.scrollIntoView({ block: 'nearest', inline: 'nearest' })
}

watch([isSearchDropdownVisible, dropdownSelectableKeys], () => {
  normalizeDropdownSelection()
})

watch([selectedRowKey, isSearchDropdownVisible], () => {
  scrollDropdownSelectionIntoView()
})

const getItemAbsPath = (item) => {
  const t = item?._searchType || searchType.value
  return t === 'excerpt' ? item.file_path : item.path
}

const getItemRel = (item) => {
  const t = item?._searchType || searchType.value
  const absPath = getItemAbsPath(item)
  const base = t === 'excerpt' ? settingsStore.librariesRoot : searchRootPath.value
  return toRelativePath(absPath, base)
}

const getItemRelDir = (item) => dirnameFromPath(getItemRel(item))

const getItemFilename = (item) => {
  const t = item?._searchType || searchType.value
  const absPath = getItemAbsPath(item)
  const fileName = t === 'excerpt' ? item.file_name : (item.file_name || basename(absPath))
  const lineSuffix = item.line_number ? `:${item.line_number}` : ''
  return `${fileName}${lineSuffix}`
}

const getItemIcon = (item) => getFileIconName(getItemAbsPath(item))

const previewResultsByType = ref({ file: [], note: [], excerpt: [] })
const previewResults = ref([])
const previewLoading = ref(false)
const previewError = ref('')
const previewHasMore = ref(false)

const previewHasAnyResults = computed(() => {
  const v = previewResultsByType.value
  return (v.file?.length || 0) + (v.note?.length || 0) + (v.excerpt?.length || 0) > 0
})

const FILE_ICON_MAP = {
  folder: 'mdi:folder-outline',
  js: 'vscode-icons:file-type-js-official',
  ts: 'vscode-icons:file-type-typescript-official',
  py: 'vscode-icons:file-type-python',
  java: 'vscode-icons:file-type-java',
  cpp: 'vscode-icons:file-type-cpp',
  cc: 'vscode-icons:file-type-cpp',
  cxx: 'vscode-icons:file-type-cpp',
  hpp: 'vscode-icons:file-type-cpp-header',
  hxx: 'vscode-icons:file-type-cpp-header',
  c: 'vscode-icons:file-type-c',
  h: 'vscode-icons:file-type-c-header',
  cs: 'vscode-icons:file-type-csharp',
  go: 'vscode-icons:file-type-go',
  rs: 'vscode-icons:file-type-rust',
  php: 'vscode-icons:file-type-php',
  sh: 'mdi:bash',
  md: 'vscode-icons:file-type-markdown',
  vue: 'vscode-icons:file-type-vue',
  json: 'vscode-icons:file-type-json',
  html: 'vscode-icons:file-type-html',
  css: 'vscode-icons:file-type-css',
  yaml: 'vscode-icons:file-type-yaml',
  yml: 'vscode-icons:file-type-yaml',
  toml: 'vscode-icons:file-type-toml',
  xml: 'vscode-icons:file-type-xml',
  default: 'mdi:file-outline'
}

const getFileIconName = (p) => {
  const pathStr = String(p || '')
  if (pathStr.endsWith('/')) return FILE_ICON_MAP.folder
  const lower = pathStr.toLowerCase()
  const specialExtMap = {
    'd.ts': 'ts',
    'spec.js': 'js',
    'test.js': 'js'
  }
  const specialExt = Object.keys(specialExtMap).find(e => lower.endsWith('.' + e))
  const ext = specialExt ? specialExtMap[specialExt] : (lower.split('.').pop() || '')
  return FILE_ICON_MAP[ext] || FILE_ICON_MAP.default
}

const recordSearchHistoryResult = (item, query) => {
  const t = item?._searchType || searchType.value
  if (!SEARCH_TYPE_KEYS.includes(t)) return

  const key = makeResultKey(item)
  if (!key) return

  const now = Date.now()
  const q = String(query || '').trim()

  const idx = searchHistory.value.findIndex(e => e && e.key === key)
  if (idx >= 0) {
    const cur = searchHistory.value[idx]
    searchHistory.value.splice(idx, 1)
    searchHistory.value.unshift({
      ...cur,
      type: t,
      query: q || String(cur.query || ''),
      count: Number(cur.count || 0) + 1,
      lastUsed: now,
      path: t === 'excerpt' ? '' : String(item.path || cur.path || ''),
      file_path: t === 'excerpt' ? String(item.file_path || cur.file_path || '') : '',
      file_name: String(item.file_name || cur.file_name || ''),
      line_number: Number(item.line_number) || 0,
      preview: item.preview ? String(item.preview) : String(cur.preview || '')
    })
  } else {
    searchHistory.value.unshift({
      key,
      type: t,
      query: q,
      count: 1,
      lastUsed: now,
      path: t === 'excerpt' ? '' : String(item.path || ''),
      file_path: t === 'excerpt' ? String(item.file_path || '') : '',
      file_name: String(item.file_name || ''),
      line_number: Number(item.line_number) || 0,
      preview: item.preview ? String(item.preview) : ''
    })
  }

  const seen = new Set()
  searchHistory.value = searchHistory.value.filter(e => {
    if (!e || !e.key) return false
    if (seen.has(e.key)) return false
    seen.add(e.key)
    return true
  })

  persistSearchHistory()
}

const normalizeSearchResults = (rawResults, type) => {
  if (!Array.isArray(rawResults)) return []

  if (type === 'file') {
    const normalizedItems = rawResults
      .filter(item => item && typeof item === 'object')
      .map(item => ({
        path: String(item.path || ''),
        file_name: String(item.file_name || basename(item.path)),
        _searchType: 'file'
      }))
      .filter(item => item.path)
    const filteredCellmetaCount = normalizedItems.filter(item => isCellmetaPath(item.path)).length
    const results = normalizedItems
      .filter(item => !isCellmetaPath(item.path))

    logger.info('[GuaxiHeader] normalized file search results', {
      rawCount: rawResults.length,
      resultCount: results.length,
      filteredCellmetaCount,
    })

    return results
  }

  if (type === 'note') {
    const normalizedItems = rawResults
      .filter(item => item && typeof item === 'object')
      .map(item => ({
        path: String(item.path || ''),
        file_name: String(item.file_name || basename(item.path)),
        preview: String(item.preview || item.text || ''),
        line_number: Number(item.line_number) || 0,
        matches: Array.isArray(item.matches) ? item.matches : [],
        _searchType: 'note'
      }))
      .filter(item => item.path && item.preview)
    const filteredCellmetaCount = normalizedItems.filter(item => isCellmetaPath(item.path)).length
    const results = normalizedItems
      .filter(item => !isCellmetaPath(item.path))

    logger.info('[GuaxiHeader] normalized note search results', {
      rawCount: rawResults.length,
      resultCount: results.length,
      filteredCellmetaCount,
    })

    return results
  }

  if (type === 'excerpt') {
    return rawResults
      .filter(item => item && typeof item === 'object')
      .map(item => ({
        file_path: String(item.file_path || item.path || ''),
        file_name: String(item.file_name || basename(item.file_path || item.path)),
        expanded: false,
        loading: false,
        _searchType: 'excerpt'
      }))
      .filter(item => item.file_path)
  }

  return []
}

const makeResultKey = (item) => {
  const t = item?._searchType || searchType.value
  if (t === 'excerpt') return `result:excerpt:${item.file_path}`
  return `result:${t}:${item.path}:${item.line_number || 0}`
}

const resultKeyToItem = computed(() => {
  const map = new Map()
  for (const item of previewResults.value) {
    map.set(makeResultKey(item), item)
  }
  return map
})

const renderResultOption = (item) => {
  const type = item?._searchType || searchType.value
  const absPath = type === 'excerpt' ? item.file_path : item.path
  const base = type === 'excerpt' ? settingsStore.librariesRoot : searchRootPath.value
  const rel = toRelativePath(absPath, base)
  const relDir = dirnameFromPath(rel)

  const fileName = type === 'excerpt' ? item.file_name : (item.file_name || basename(absPath))
  const lineSuffix = item.line_number ? `:${item.line_number}` : ''
  const iconName = getFileIconName(absPath)

  return h('div', { class: 'search-dd-item' }, [
    h('div', { class: 'search-dd-main', title: rel }, [
      h('span', { class: 'search-dd-icon-wrap' }, [
        h(Icon, { icon: iconName, width: 16, height: 16, class: 'search-dd-icon' })
      ]),
      h('div', { class: 'search-dd-filename' }, `${fileName}${lineSuffix}`),
      relDir
        ? h('div', { class: 'search-dd-path-inline', title: rel }, relDir)
        : h('div', { class: 'search-dd-path-inline', title: rel }, '')
    ]),
    type === 'note' && item.preview
      ? h('div', { class: 'search-dd-preview' }, String(item.preview).slice(0, 160))
      : null
  ])
}

const renderHistoryOption = (entry) => {
  return h('div', { class: 'search-dd-item' }, [
    h('div', { class: 'search-dd-main' }, [
      h('span', { class: 'search-dd-icon-wrap' }, [
        h(Icon, { icon: 'mdi:history', width: 16, height: 16, class: 'search-dd-icon search-dd-icon-muted' })
      ]),
      h('div', { class: 'search-dd-filename' }, entry.query),
      h('div', { class: 'search-dd-path-inline' }, '')
    ])
  ])
}

const dropdownOptions = computed(() => {
  const typeOptions = [
    {
      label: searchType.value === 'file' ? `${SEARCH_TYPE_LABELS.file}（当前）` : SEARCH_TYPE_LABELS.file,
      key: 'file',
      icon: () => h(NIcon, null, { default: () => h(DocumentSearch20Filled) })
    },
    {
      label: searchType.value === 'note' ? `${SEARCH_TYPE_LABELS.note}（当前）` : SEARCH_TYPE_LABELS.note,
      key: 'note',
      icon: () => h(NIcon, null, { default: () => h(DocumentSearch24Regular) })
    },
    {
      label: searchType.value === 'excerpt' ? `${SEARCH_TYPE_LABELS.excerpt}（当前）` : SEARCH_TYPE_LABELS.excerpt,
      key: 'excerpt',
      icon: () => h(NIcon, null, { default: () => h(NotebookSection24Regular) })
    }
  ]

  const q = searchQuery.value.trim()
  const historyItems = getHistorySuggestions(q)

  if (!q) {
    const options = []
    if (historyItems.length > 0) {
      options.push({ label: '最近搜索', key: '__recent_title__', disabled: true })
      for (const e of historyItems) {
        options.push({
          key: `history:${e.type}:${encodeURIComponent(e.query)}`,
          label: () => renderHistoryOption(e)
        })
      }
      return options
    }
    return [{ label: '暂无最近搜索', key: '__empty_history__', disabled: true }]
  }

  const options = [...typeOptions, { type: 'divider', key: '__divider__' }]

  if (historyItems.length > 0) {
    options.push({ label: '最近搜索', key: '__recent_title__', disabled: true })
    for (const e of historyItems) {
      options.push({
        key: `history:${e.type}:${encodeURIComponent(e.query)}`,
        label: () => renderHistoryOption(e)
      })
    }
    options.push({ type: 'divider', key: '__divider_recent__' })
  }

  if (previewLoading.value) {
    options.push({ label: '搜索中…', key: '__loading__', disabled: true })
    return options
  }

  if (previewError.value) {
    options.push({ label: previewError.value, key: '__error__', disabled: true })
    return options
  }

  if (previewResults.value.length === 0) {
    options.push({ label: '无匹配结果', key: '__empty__', disabled: true })
    return options
  }

  for (const item of previewResults.value.slice(0, 20)) {
    const key = makeResultKey(item)
    options.push({ key, label: () => renderResultOption(item) })
  }

  return options
})

const updateDropdownPosition = () => {
  const rect = searchContainerRef.value?.getBoundingClientRect()
  if (!rect) return

  dropdownWidth.value = rect.width
  dropdownPosition.value = {
    x: Math.round(rect.left),
    y: Math.round(rect.bottom)
  }
}

const showSearchOptions = async () => {
  await nextTick()
  updateDropdownPosition()
  isSearchDropdownVisible.value = true
}

const handleSearchBlur = (e) => {
  // 焦点移动到下拉内部的筛选控件(日期/自定义后缀输入)时不关闭下拉。
  const rt = e && e.relatedTarget
  if (rt && searchDropdownRef.value && searchDropdownRef.value.contains(rt)) return
  setTimeout(() => {
    isSearchDropdownVisible.value = false
  }, 200)
}

// 点击下拉与搜索框之外的区域时关闭下拉(覆盖焦点落在筛选输入框后的外部点击)。
const handleDocMousedown = (e) => {
  if (!isSearchDropdownVisible.value) return
  const t = e.target
  if (searchContainerRef.value && searchContainerRef.value.contains(t)) return
  if (searchDropdownRef.value && searchDropdownRef.value.contains(t)) return
  isSearchDropdownVisible.value = false
}

const openResultInEditor = async (item) => {
  const t = item?._searchType || searchType.value
  const q = String(searchQuery.value || '').trim()
  const shouldHighlight = t === 'note' && !!q

  logger.debug('[openResultInEditor] open', {
    type: t,
    hasQuery: !!q,
    path: item?.path,
    line_number: item?.line_number
  })

  const result = await fileTreeStore.fileTreeOpenSearchResult({
    ...item,
    searchText: shouldHighlight ? q : ''
  })

  if (shouldHighlight) {
    editorStore.highlightInfo = {
      path: result.path,
      lineNumber: result.lineNumber,
      matches: result.matches,
      text: result.text,
    }
  } else {
    editorStore.highlightInfo = null
  }

  editorStore.setFile({
    path: result.path,
    content: result.content || '',
  })
}

const applyPreviewResultsToRightSider = (lastType) => {
  const t = SEARCH_TYPE_KEYS.includes(lastType) ? lastType : searchType.value
  const nextResults = {
    ...rightSiderStore.searchResults,
    fileResults: previewResultsByType.value.file,
    noteResults: previewResultsByType.value.note,
    excerptResults: previewResultsByType.value.excerpt,
    searchQuery: searchQuery.value,
    lastSearchType: t,
    loading: false,
    error: ''
  }

  rightSiderStore.setSearchResults(nextResults)
  rightSiderStore.setPanel('search')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(viewKey, false)
}

const handleSearchEnter = () => {
  const q = searchQuery.value.trim()
  if (!q && !hasActiveFilters.value) return

  // 仅有筛选条件(无关键字)时，按文件列举筛选结果(内容/摘录搜索需要关键字)。
  if (!q && hasActiveFilters.value) {
    handleSearchFiles()
    return
  }

  if (searchType.value === 'file') {
    handleSearchFiles()
  } else if (searchType.value === 'note') {
    handleSearchNotes()
  } else if (searchType.value === 'excerpt') {
    handleSearchExcerpts()
  }
}

const applySingleItemToRightSider = (item, query) => {
  const t = item?._searchType || searchType.value
  const nextResults = {
    ...rightSiderStore.searchResults,
    fileResults: t === 'file' ? [item] : [],
    noteResults: t === 'note' ? [item] : [],
    excerptResults: t === 'excerpt' ? [item] : [],
    searchQuery: String(query || ''),
    lastSearchType: SEARCH_TYPE_KEYS.includes(t) ? t : searchType.value,
    loading: false,
    error: ''
  }

  rightSiderStore.setSearchResults(nextResults)
  rightSiderStore.setPanel('search')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed(viewKey, false)
}

const handleSearchSelect = async (key) => {
  if (SEARCH_TYPE_KEYS.includes(key)) {
    searchType.value = key
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      lastSearchType: key
    })

    if (searchQuery.value.trim()) {
      await nextTick()
      updateDropdownPosition()
    }

    nextTick(() => {
      searchInputRef.value?.focus()
    })
    return
  }

  if (typeof key === 'string' && key.startsWith('historyResult:')) {
    const resultKey = key.slice('historyResult:'.length)
    const entry = historyKeyToEntry.value.get(resultKey)
    if (!entry) return

    const item = historyEntryToItem(entry)
    const t = item._searchType || searchType.value
    if (SEARCH_TYPE_KEYS.includes(t)) {
      searchType.value = t
    }

    if (entry.query) {
      searchQuery.value = String(entry.query)
      await nextTick()
      updateDropdownPosition()
    }

    recordSearchHistoryResult(item, searchQuery.value)
    applySingleItemToRightSider(item, searchQuery.value)

    await flashSelectRow(key)

    if (item._searchType !== 'excerpt') {
      await openResultInEditor(item)
    }

    isSearchDropdownVisible.value = false
    return
  }

  if (typeof key === 'string' && key.startsWith('result:')) {
    const item = resultKeyToItem.value.get(key)
    if (!item) return

    const t = item._searchType || searchType.value
    if (SEARCH_TYPE_KEYS.includes(t)) {
      searchType.value = t
    }

    recordSearchHistoryResult(item, searchQuery.value)
    applyPreviewResultsToRightSider(t)

    await flashSelectRow(key)

    if (item._searchType !== 'excerpt') {
      await openResultInEditor(item)
    }

    isSearchDropdownVisible.value = false
  }
}

const runPreviewSearchAll = async () => {
  const q = trimmedQuery.value
  // 有关键字、或仅有筛选条件时都执行搜索
  if ((!q && !hasActiveFilters.value) || !isSearchDropdownVisible.value) {
    previewResultsByType.value = { file: [], note: [], excerpt: [] }
    previewResults.value = []
    previewError.value = ''
    previewHasMore.value = false
    return
  }

  previewLoading.value = true
  previewError.value = ''
  previewHasMore.value = false

  try {
    const { extensions, modifiedAfter, modifiedBefore } = searchFilterArgs.value
    const tagSetPromise = resolveTagPathSet()

    // 关键字搜索(rg) / 摘录搜索需要关键字；仅筛选时只列举文件(fd)。
    const [fdResp, rgResp, exResp] = await Promise.allSettled([
      invoke('fd_search', { pattern: q, path: searchRootPath.value, extensions, modifiedAfter, modifiedBefore }),
      q
        ? invoke('rg_search', { pattern: q, path: searchRootPath.value, extensions, modifiedAfter, modifiedBefore })
        : Promise.resolve(null),
      q
        ? invoke('search_excerpt_notes', { pattern: q, path: settingsStore.librariesRoot })
        : Promise.resolve(null)
    ])

    const tagSet = await tagSetPromise
    const next = { file: [], note: [], excerpt: [] }

    if (fdResp.status === 'fulfilled' && fdResp.value) {
      const [rawResults = [], hasMore = false] = Array.isArray(fdResp.value) ? fdResp.value : []
      next.file = filterByTagPaths(normalizeSearchResults(rawResults, 'file'), tagSet)
      previewHasMore.value = previewHasMore.value || Boolean(hasMore)
    }

    if (rgResp.status === 'fulfilled' && rgResp.value) {
      const response = rgResp.value
      const rawResults = (() => {
        if (!response) return []
        if (Array.isArray(response[0])) return response[0]
        if (Array.isArray(response)) return response
        return [response]
      })()
      next.note = filterByTagPaths(normalizeSearchResults(rawResults, 'note'), tagSet)
    }

    if (exResp.status === 'fulfilled' && exResp.value) {
      next.excerpt = normalizeSearchResults(exResp.value, 'excerpt')
    }

    previewResultsByType.value = next
    previewResults.value = [...next.file, ...next.note, ...next.excerpt]

    if (!previewHasAnyResults.value && fdResp.status === 'rejected' && rgResp.status === 'rejected' && exResp.status === 'rejected') {
      previewError.value = '搜索失败'
    }
  } catch (err) {
    previewResultsByType.value = { file: [], note: [], excerpt: [] }
    previewResults.value = []
    previewError.value = err?.message ? `搜索失败：${err.message}` : '搜索失败'
  } finally {
    previewLoading.value = false
  }
}

const debouncedRunPreviewSearch = debounce(runPreviewSearchAll, 200)

watch([searchQuery, isSearchDropdownVisible], () => {
  if (!isSearchDropdownVisible.value) return
  updateDropdownPosition()
  debouncedRunPreviewSearch()
})

const handleWindowReflow = () => {
  if (!isSearchDropdownVisible.value) return
  updateDropdownPosition()
}

const handleGlobalSearchHotkey = (e) => {
  if (e?.defaultPrevented) return

  const key = e?.key
  const isP = key === 'p' || key === 'P'
  const withModifier = !!(e?.ctrlKey || e?.metaKey)

  if (!isP || !withModifier || e?.altKey) return

  e.preventDefault()
  e.stopPropagation()

  Promise.resolve(showSearchOptions())
    .catch(() => {})
    .finally(() => {
      nextTick(() => {
        searchInputRef.value?.focus?.()
      })
    })
}

const handleBeforeUnload = () => {
  closeRequested.value = true
  searchHistory.value = []
  try {
    localStorage.removeItem(HISTORY_STORAGE_KEY)
  } catch {}
}

onMounted(() => {
  window.addEventListener('resize', handleWindowReflow)
  window.addEventListener('scroll', handleWindowReflow, true)
  window.addEventListener('keydown', handleGlobalSearchHotkey, true)
  window.addEventListener('beforeunload', handleBeforeUnload)
  window.addEventListener('mousedown', handleDocMousedown, true)

  Promise.resolve(initResourceRepoAutoSync()).catch((e) => {
    logger.warn('[GuaxiHeader] init resource repo auto sync failed', e)
  })
})

onUnmounted(() => {
  closeRequested.value = true
  window.removeEventListener('resize', handleWindowReflow)
  window.removeEventListener('scroll', handleWindowReflow, true)
  window.removeEventListener('keydown', handleGlobalSearchHotkey, true)
  window.removeEventListener('beforeunload', handleBeforeUnload)
  window.removeEventListener('mousedown', handleDocMousedown, true)
  debouncedRunPreviewSearch.cancel()

})

async function handleSearchFiles() {
  try {
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      fileResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'file',
      loading: true,
      error: ''
    })

    const { extensions, modifiedAfter, modifiedBefore } = searchFilterArgs.value
    const tagSet = await resolveTagPathSet()
    const rawResponse = await invoke('fd_search', {
      pattern: searchQuery.value,
      path: searchRootPath.value,
      extensions,
      modifiedAfter,
      modifiedBefore
    })

    const [rawResults = [], hasMore = false] = Array.isArray(rawResponse) ? rawResponse : []
    const validResults = filterByTagPaths(normalizeSearchResults(rawResults, 'file'), tagSet)

    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      fileResults: validResults,
      searchQuery: searchQuery.value,
      lastSearchType: 'file',
      loading: false,
      error: ''
    })

    rightSiderStore.setPanel('search')
    siderStore.setRightSiderCollapsed(viewKey, false)
  } catch (error) {
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      fileResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'file',
      loading: false,
      error: error?.message || '文件搜索失败'
    })
    message.error(`文件搜索失败: ${error?.message || String(error)}`)
  }
}

async function handleSearchNotes() {
  try {
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      noteResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'note',
      loading: true,
      error: ''
    })

    const { extensions, modifiedAfter, modifiedBefore } = searchFilterArgs.value
    const tagSet = await resolveTagPathSet()
    const response = await invoke('rg_search', {
      pattern: searchQuery.value,
      path: searchRootPath.value,
      extensions,
      modifiedAfter,
      modifiedBefore
    })

    const rawResults = (() => {
      if (!response) return []
      if (Array.isArray(response[0])) return response[0]
      if (Array.isArray(response)) return response
      return [response]
    })()

    const validResults = filterByTagPaths(normalizeSearchResults(rawResults, 'note'), tagSet)

    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      noteResults: validResults,
      searchQuery: searchQuery.value,
      lastSearchType: 'note',
      loading: false,
      error: ''
    })

    rightSiderStore.setPanel('search')
    siderStore.setRightSiderCollapsed(viewKey, false)
  } catch (err) {
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      noteResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'note',
      error: err?.message || '内容搜索失败',
      loading: false
    })
    message.error(`内容搜索失败: ${err?.message || String(err)}`)
  }
}

async function handleSearchExcerpts() {
  try {
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      excerptResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'excerpt',
      loading: true,
      error: ''
    })

    const results = await invoke('search_excerpt_notes', {
      pattern: searchQuery.value,
      path: settingsStore.librariesRoot
    })

    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      excerptResults: normalizeSearchResults(results, 'excerpt'),
      searchQuery: searchQuery.value,
      lastSearchType: 'excerpt',
      loading: false,
      error: ''
    })

    rightSiderStore.setPanel('search')
    siderStore.setRightSiderCollapsed(viewKey, false)
  } catch (err) {
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      excerptResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'excerpt',
      loading: false,
      error: err?.message || '摘录搜索失败'
    })
    message.error('摘录搜索失败: ' + (err?.message || String(err)))
  }
}

onMounted(async () => {
  settingsStore.get_librariesRoot()
})
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  /* 固定高度 */
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  padding: 0 8px;
  box-sizing: border-box;
  /* 确保padding不增加总高度 */
}

/* 重置所有内部div的盒模型 */
.guaxi-header>div {
  margin: 0;
  padding: 0;
  height: 100%;
}

/* 左侧样式 */
.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
}

.logo-text {
  font-weight: bold;
  font-size: 16px;
  white-space: nowrap;
  line-height: 1;
}

/* 中间样式 */
.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  max-width: 1000px;
  margin: 0 10px;
}

.search-container {
  width: 100%;
  max-width: 800px;
  display: flex;
  align-items: center;
}

/* 加强输入框样式优先级 */
.search-container {
  height: 100%;
}

.search-container :deep(.n-input) {
  height: 28px !important;
  min-height: 28px !important;
  margin: 0 !important;
}

.search-container :deep(.n-input .n-input-wrapper) {
  background-color: var(--guaxi-panel-bg) !important;
}

.search-container :deep(.n-input .n-input__border),
.search-container :deep(.n-input .n-input__state-border) {
  border-color: var(--guaxi-border) !important;
}

.search-container :deep(.n-input__input-el) {
  height: 28px !important;
  line-height: 28px !important;
  color: var(--guaxi-text-primary) !important;
}

.search-container :deep(.n-input__prefix),
.search-container :deep(.n-input__suffix) {
  height: 28px !important;
  color: var(--guaxi-text-secondary) !important;
}

/* 右侧样式 */
.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
  justify-content: flex-end;
}

/* 图标通用样式 */
.header-icon {
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: rgba(0, 0, 0, 0.05);
}

/* 搜索下拉菜单（自绘，避免 Naive Dropdown label 布局干扰） */
.search-container.open :deep(.n-input .n-input__border),
.search-container.open :deep(.n-input .n-input__state-border) {
  border-bottom-left-radius: 0 !important;
  border-bottom-right-radius: 0 !important;
}

.search-dd-root {
  background: var(--guaxi-panel-bg);
  border: 1px solid rgba(0, 0, 0, 0.12);
  border-top: none;
  border-top-left-radius: 0;
  border-top-right-radius: 0;
  border-bottom-left-radius: 6px;
  border-bottom-right-radius: 6px;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.16);
  max-height: 60vh;
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  padding: 4px 0;
  box-sizing: border-box;
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.26) transparent;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
}

.search-dd-root::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.search-dd-root::-webkit-scrollbar-thumb {
  background-color: rgba(0, 0, 0, 0.22);
  border-radius: 3px;
}

.search-dd-root::-webkit-scrollbar-thumb:hover {
  background-color: rgba(0, 0, 0, 0.34);
}

.search-dd-root::-webkit-scrollbar-track {
  background-color: transparent;
}

.search-dd-section {
  display: block;
}

.search-dd-section-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding: 6px 10px;
}

.search-dd-divider {
  height: 1px;
  background: var(--guaxi-border);
  margin: 4px 0;
}

.search-dd-row {
  padding: 6px 10px;
  margin: 2px 6px;
  border-radius: 6px;
  cursor: pointer;
  user-select: none;
  transition: transform 0.2s ease, box-shadow 0.2s ease, background 0.2s ease;
}

.search-dd-row:hover {
  background: rgba(255, 255, 255, 0.03);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.18), 0 2px 12px 0 rgba(7, 118, 161, 0.1);
  transform: scale(1.01);
}

.search-dd-row.is-selected {
  background: rgba(255, 255, 255, 0.06);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.26), 0 2px 12px 0 rgba(7, 118, 161, 0.14);
  animation: search-dd-select-pop 160ms ease-out;
}

@keyframes search-dd-select-pop {
  from {
    transform: scale(1);
  }
  60% {
    transform: scale(1.02);
  }
  to {
    transform: scale(1.01);
  }
}

.search-dd-row.active {
  background-color: rgba(0, 0, 0, 0.06);
}

.search-dd-row-type {
  display: flex;
  align-items: center;
  gap: 8px;
}

.search-dd-row-text {
  font-size: 13px;
  color: var(--guaxi-text-primary);
}

.search-dd-empty {
  padding: 8px 10px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.search-dd-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.search-dd-main {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  width: 100%;
}

.search-dd-icon-wrap {
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.search-dd-icon {
  opacity: 0.95;
  display: block;
}

.search-dd-icon-muted {
  opacity: 0.65;
}

.search-dd-filename {
  font-size: 13px;
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 0 1 auto;
  max-width: 55%;
}

.search-dd-path {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1 1 auto;
}

.search-dd-preview {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding-left: 28px;
}

/* 搜索框内的筛选按钮 */
.search-filter-btn {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
  transition: color 0.15s, background-color 0.15s;
}
.search-filter-btn:hover {
  color: var(--n-color-primary, #1890ff);
  background-color: rgba(24, 144, 255, 0.12);
}
.search-filter-btn.active {
  color: var(--n-color-primary, #1890ff);
}
.search-filter-badge {
  position: absolute;
  top: -3px;
  right: -3px;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  border-radius: 7px;
  background: var(--n-color-primary, #1890ff);
  color: #fff;
  font-size: 10px;
  line-height: 14px;
  text-align: center;
  box-sizing: border-box;
}

/* 筛选面板 */
.search-filters {
  padding: 6px 10px 2px;
}
.filter-group {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 6px 0;
}
.filter-label {
  flex: 0 0 52px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding-top: 4px;
}
.filter-chips {
  flex: 1 1 auto;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  min-width: 0;
}
.filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 9px;
  font-size: 12px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  background: transparent;
  cursor: pointer;
  user-select: none;
  transition: all 0.15s ease;
}
.filter-chip:hover {
  border-color: var(--n-color-primary, #1890ff);
}
.filter-chip.on {
  background: rgba(24, 144, 255, 0.16);
  border-color: var(--n-color-primary, #1890ff);
  color: var(--n-color-primary, #1890ff);
}
.filter-chip-count {
  font-size: 10px;
  opacity: 0.6;
  font-style: normal;
}
.filter-dates {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 2px 0 6px 60px;
}
.filter-date {
  font-size: 12px;
  padding: 2px 6px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  outline: none;
}
.filter-dash {
  color: var(--guaxi-text-secondary);
}
.filter-ext-input {
  width: 96px;
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 999px;
  border: 1px dashed var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-primary);
  outline: none;
}
.filter-ext-input:focus {
  border-color: var(--n-color-primary, #1890ff);
  border-style: solid;
}
.filter-empty {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}
.filter-matchall {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin: 6px 0 0 60px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
}
.filter-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
}
.filter-hint {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}
.filter-clear {
  font-size: 12px;
  color: var(--n-color-primary, #1890ff);
  cursor: pointer;
}
.filter-clear:hover {
  text-decoration: underline;
}
</style>
