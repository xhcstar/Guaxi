<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="home" side="left" />
      <div class="logo-text">Guaxi</div>

      <!--
        编辑区上下装饰条的显隐开关（格式工具栏 + 面包屑导航）。

        为什么放在 Header 左侧：这两条属于「中间编辑区」的装饰，而切换按钮要长期可见；
        阅读 PDF / 电子书 / 图片时它们不适用，收起来能把高度让给内容。

        默认显示 = 与改造前完全一致；状态存在 siderStore，按窗口记住。
        图标用「收拢 / 展开」两态：上下双箭头向中线收 = 正在显示，点它把两条收起；
        上下双箭头向外张开 = 已隐藏，点它让两条回来。比原先的「面板」图标更直白。
      -->
      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon
            class="header-icon editor-chrome-toggle"
            :class="{ active: isEditorChromeShown }"
            size="20"
            :component="isEditorChromeShown ? ArrowMinimizeVertical20Filled : ArrowMaximizeVertical20Filled"
            @click="toggleEditorChrome"
          />
        </template>
        {{ headerShortcutTooltip(isEditorChromeShown ? '隐藏工具栏与面包屑' : '显示工具栏与面包屑', 'toggleEditorChrome') }}
      </n-tooltip>
    </div>

    <!-- 中间：搜索框 -->
    <div class="header-center">
      <div ref="searchContainerRef" class="search-container" :class="{ open: isSearchDropdownVisible }">
        <n-input ref="searchInputRef" v-model:value="searchQuery" placeholder="搜索文件或内容，输入 > 打开命令" round clearable
          @keydown="handleSearchKeydown" @focus="showSearchOptions" @blur="handleSearchBlur">
          <template #prefix>
            <n-icon :component="Search32Filled" />
          </template>
          <template #suffix>
            <span class="search-mode-group">
              <span
                v-for="m in MATCH_TOGGLES"
                :key="m.key"
                class="search-mode-btn"
                :class="{ on: matchFlags[m.key] }"
                :title="m.title"
                @mousedown.prevent
                @click="toggleMatchFlag(m.key)"
              >{{ m.glyph }}</span>
            </span>
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
          <n-icon class="header-icon" size="20" data-tour="create-kb" :component="BookAdd20Regular" @click="openCreateKnowledgeBase" />
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
          <n-icon class="header-icon" size="20" @click="openDiffCompare">
            <svg viewBox="0 0 24 24" width="18" height="18">
              <path fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
                stroke-linejoin="round"
                d="M4 4h6v16H4zm10 0h6v16h-6zM6 8h2m-2 3.5h2M16 8h2m-2 3.5h2m-2 3.5h2" />
            </svg>
          </n-icon>
        </template>
        差异对比（任意文件 / 目录）
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

      <!-- 点图标弹菜单，让用户自己选要不要隐藏窗口：隐藏时截到的是窗口**后面**的内容，
           不隐藏才能截取本软件自己的界面。快捷键仍然直接走"隐藏窗口"那一档
           （见 CaptureScreenshot），行为与以前一致。
           嵌套顺序沿用工具栏里的既有约定：n-dropdown 在外、n-tooltip 在内。 -->
      <n-dropdown trigger="click" :options="screenshotMenuOptions" @select="handleScreenshotMenuSelect">
        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-icon class="header-icon" size="20" :component="Screenshot24Regular"></n-icon>
          </template>
          {{ headerShortcutTooltip('截图并插入', 'captureScreenshot') }}
        </n-tooltip>
      </n-dropdown>

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

      <GuaxiSiderToggle view-key="home" side="right" />

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" @click="restartOnboardingTour">
            <svg viewBox="0 0 24 24" width="18" height="18">
              <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="1.7" />
              <path fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"
                d="M9.6 9.3a2.5 2.5 0 1 1 3.3 2.3c-.7.3-1 .9-1 1.6v.4" />
              <circle cx="12" cy="16.7" r="1.05" fill="currentColor" />
            </svg>
          </n-icon>
        </template>
        重新观看新手引导
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
      <!-- 命令面板：输入框以 `>` 开头时接管整个下拉 -->
      <div v-if="isCommandMode" class="search-dd-section">
        <div class="search-dd-section-title">命令</div>
        <div v-if="!matchedCommands.length" class="search-dd-empty">没有匹配的命令</div>
        <div
          v-for="c in matchedCommands"
          :key="`command:${c.id}`"
          class="search-dd-row search-dd-row-command"
          :class="{ 'is-selected': selectedRowKey === `command:${c.id}` }"
          :data-row-key="`command:${c.id}`"
          @click="handleSearchSelect(`command:${c.id}`)"
        >
          <span class="search-dd-icon-wrap">
            <Icon :icon="c.icon" width="16" height="16" class="search-dd-icon" />
          </span>
          <div class="search-dd-cmd-text">
            <span class="search-dd-cmd-label">{{ c.label }}</span>
            <span class="search-dd-cmd-hint">{{ c.hint }}</span>
          </div>
          <span v-if="c.state" class="search-dd-cmd-state">{{ c.state }}</span>
        </div>
        <div class="search-dd-divider" />
        <!-- 记号不写在这儿没人会知道它们存在 -->
        <div class="search-dd-syntax">
          <span class="search-dd-syntax-title">查询语法</span>
          <code>ext:md,txt</code>
          <code>path:src</code>
          <code>tag:待办</code>
          <code>after:7d</code>
          <code>before:2026-01</code>
        </div>
      </div>

      <div v-if="!isCommandMode" class="search-dd-section">
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
      <div v-if="filtersOpen && !isCommandMode" class="search-dd-section search-filters">
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

      <div v-if="historyItems.length && !isCommandMode" class="search-dd-section">
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

      <div v-if="!isCommandMode && !trimmedQuery" class="search-dd-section">
        <div v-if="!historyItems.length" class="search-dd-empty">暂无最近搜索</div>
      </div>

      <div v-else-if="!isCommandMode" class="search-dd-section">
        <!-- 报错和结果要并存：一路失败不该把另两路的结果一起吞掉 -->
        <div v-if="previewError" class="search-dd-error">{{ previewError }}</div>
        <div v-if="previewLoading" class="search-dd-empty">搜索中…</div>
        <div v-else-if="!previewHasAnyResults && !previewError" class="search-dd-empty">无匹配结果</div>
        <template v-else-if="previewHasAnyResults">
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
import { parseSearchQuery, matchesPathFilters } from '../../utils/searchQuerySyntax'
import { computed, ref, h, onMounted, onUnmounted, nextTick, watch } from "vue";
import { NDropdown, NIcon, NTooltip } from 'naive-ui'
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'
import { useOnboardingTour } from '../../composables/useOnboardingTour'
import { GUAXI_SHORTCUTS, shortcutTooltip, useGuaxiHotkeys } from '../../composables/useGuaxiHotkeys'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useEditorStore } from '../../stores/editorStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useDiffCompareStore } from '../../stores/diffCompareStore'
import CreateKnowledgeBaseForm from '../BrainViewComponent/CreateKnowledgeBaseForm.vue'

import { useMessage } from 'naive-ui';
import { debounce } from 'lodash-es'
import { Icon } from '@iconify/vue'
import assistantIcon from '@iconify-icons/mdi/robot-excited-outline'
// 命令面板 / 历史记录的图标。和上面的 assistantIcon 一样 import 图标数据对象，
// 而不是给 <Icon> 传 'mdi:xxx' 名字：名字得先在 src/main.js 里 addIcon 注册，
// 漏注册的名字会让 <Icon> 去 api.iconify.design 现取——release 包在断网、内网、
// CSP 收紧的机器上就是一排空白图标。传对象则被 Vite 打进 bundle，不碰网络，
// 名字写错也在构建期就报"模块找不到"，不会留到运行时。
import mdiFileSearchOutline from '@iconify-icons/mdi/file-search-outline'
import mdiTextSearch from '@iconify-icons/mdi/text-search'
import mdiFilterOffOutline from '@iconify-icons/mdi/filter-off-outline'
import mdiRegex from '@iconify-icons/mdi/regex'
import mdiFormatLetterMatches from '@iconify-icons/mdi/format-letter-matches'
import mdiFormatLetterCase from '@iconify-icons/mdi/format-letter-case'
import mdiHistory from '@iconify-icons/mdi/history'
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
  ArrowMinimizeVertical20Filled,
  ArrowMaximizeVertical20Filled
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
const diffCompareStore = useDiffCompareStore()
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

/**
 * 编辑区装饰条（格式工具栏 + 面包屑）当前是否显示。
 * 未初始化时返回 true —— 老用户升级后界面不变。
 */
const isEditorChromeShown = computed(() => siderStore.isEditorChromeShown(viewKey))

const toggleEditorChrome = () => {
  siderStore.toggleEditorChrome(viewKey)
}

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

// 「重新观看新手引导」。引导要定位的是首页的顶栏/文件树/编辑器，所以入口只能挂在这里——
// 放设置页的话，那些控件根本不在 DOM 里，每一步都会退化成居中卡片。
const restartOnboardingTour = () => {
  const { start } = useOnboardingTour()
  start({ force: true })
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

/**
 * 唤醒独立的差异对比窗口，两侧预填上次比过的路径（存在且类型没变才填）。
 * 路径都由后端自己的命令用 std::fs 读，不走 fs 插件，所以 release 下的权限和 dev 一致；
 * 窗口里"选文件"用的 dialog 插件权限已经在 capabilities 里覆盖了 diff-compare 窗口。
 */
async function openDiffCompare() {
  try {
    await diffCompareStore.openLastUsed()
  } catch (err) {
    logger.error('打开差异对比窗口失败:', err)
    message.error('打开差异对比窗口失败')
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

/**
 * 截图并插入。
 * - `hideWindow: true`：先把窗口最小化再截 —— 截到的是窗口**后面**的内容（原来的行为）。
 * - `hideWindow: false`：保持窗口可见，用于截取本软件自己的界面。
 * 只有真的隐藏过才还原：否则会把用户自己的窗口状态改掉（比如把已最小化的窗口弹出来抢焦点）。
 */
async function runScreenshotCapture({ hideWindow = true } = {}) {
  try {
    if (hideWindow) {
      try {
        logger.debug('[Screenshot] Minimizing window before capture')
        await appWindow.minimize()
        await waitForWindowMinimized()
      } catch (minErr) {
        logger.warn('[Screenshot] Failed to minimize window:', minErr)
      }
    }

    const originalPath = fileTreeStore.selectedFile?.path
    logger.debug('[Screenshot] Triggered', {
      activeTab: fileTreeStore.activeTab,
      selectedPath: originalPath,
      hideWindow
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
    if (hideWindow) {
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
}

/** 快捷键走这一档：与改动前完全一致的"隐藏窗口再截"，行为不变。 */
function CaptureScreenshot() {
  return runScreenshotCapture({ hideWindow: true })
}

// 点图标弹出的两个选项；key 只用来区分是否隐藏窗口。
const screenshotMenuOptions = [
  { label: '隐藏当前窗口截图', key: 'hide' },
  { label: '不隐藏当前窗口截图', key: 'keep' },
]

const handleScreenshotMenuSelect = (key) => {
  void runScreenshotCapture({ hideWindow: key !== 'keep' })
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
  {
    id: 'toggleEditorChrome',
    label: '显示/隐藏工具栏与面包屑',
    shortcut: GUAXI_SHORTCUTS.toggleEditorChrome,
    handler: toggleEditorChrome,
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

/**
 * 输入框里的查询语法（`ext:md`、`after:7d`、`path:src`、`tag:待办`、`>命令`）。
 *
 * 解析放在这一层而不是后端：筛选条件本来就要作为独立参数传下去，
 * 后端收到的 pattern 必须是剥干净的搜索词——否则 `ext:md 配置`
 * 会被当成字面量拿去 grep，一条都搜不到。
 */
const parsedQuery = computed(() => parseSearchQuery(searchQuery.value))

/** 命令模式（输入以 `>` 开头）下的命令查询词；非命令模式为 null。 */
const commandQuery = computed(() => parsedQuery.value.command)
const isCommandMode = computed(() => commandQuery.value !== null)

// 真正要搜的词：记号已经剥掉。命令模式下不搜任何东西
const trimmedQuery = computed(() => (isCommandMode.value ? '' : parsedQuery.value.text.trim()))

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

// ───── 匹配方式：区分大小写 / 全词 / 正则 ─────
// 后端默认按字面量 + 智能大小写，这三个开关是给需要精确控制的人用的。
// 记进 localStorage：习惯用正则的人不该每次开窗都重新点一遍。
const MATCH_FLAGS_KEY = 'guaxi.home.searchMatchFlags.v1'
const MATCH_TOGGLES = [
  { key: 'caseSensitive', glyph: 'Aa', title: '区分大小写（关闭时：查询里出现大写才区分）' },
  { key: 'wholeWord', glyph: 'ab|', title: '全词匹配（中文按分词判断词边界）' },
  { key: 'regex', glyph: '.*', title: '按正则表达式搜索（关闭时按字面量，标点不会被当通配符）' },
]
const matchFlags = ref((() => {
  const base = { caseSensitive: false, wholeWord: false, regex: false }
  try {
    const saved = JSON.parse(localStorage.getItem(MATCH_FLAGS_KEY) || '{}')
    for (const k of Object.keys(base)) base[k] = !!saved[k]
  } catch {}
  return base
})())

const toggleMatchFlag = (key) => {
  matchFlags.value = { ...matchFlags.value, [key]: !matchFlags.value[key] }
  try {
    localStorage.setItem(MATCH_FLAGS_KEY, JSON.stringify(matchFlags.value))
  } catch {}
  scheduleFilterSearch()
  nextTick(() => searchInputRef.value?.focus?.())
}

/**
 * 传给 fd_search / rg_search 的 `options`。
 * caseSensitive 关掉时传 null 而不是 false —— null 让后端走智能大小写，
 * false 会强制忽略大小写，反而比默认更钝。
 */
const searchMatchOptions = computed(() => ({
  regex: matchFlags.value.regex,
  wholeWord: matchFlags.value.wholeWord,
  caseSensitive: matchFlags.value.caseSensitive ? true : null,
}))

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

/**
 * 面板上的筛选 + 输入框里的记号，合成最终生效的筛选条件。
 *
 * 同一个维度上记号优先：用户刚敲下 `ext:rs`，却还拿到面板上勾着的 .md，
 * 会觉得"我明明写了 rs"。面板没勾的维度自然由记号补上。
 */
const effectiveExts = computed(() =>
  parsedQuery.value.exts.length ? parsedQuery.value.exts : filterExts.value
)
const effectiveTags = computed(() =>
  parsedQuery.value.tags.length ? parsedQuery.value.tags : filterTags.value
)
const effectiveTimeRange = computed(() => {
  const { after, before } = parsedQuery.value
  if (after !== null || before !== null) return { after, before }
  return timeRange.value
})

const extensionsParam = computed(() => (effectiveExts.value.length ? [...effectiveExts.value] : null))

const hasActiveFilters = computed(
  () =>
    effectiveExts.value.length > 0 ||
    effectiveTags.value.length > 0 ||
    effectiveTimeRange.value.after !== null ||
    effectiveTimeRange.value.before !== null ||
    parsedQuery.value.paths.length > 0
)
const activeFilterCount = computed(
  () =>
    (effectiveTimeRange.value.after !== null || effectiveTimeRange.value.before !== null ? 1 : 0) +
    (effectiveExts.value.length ? 1 : 0) +
    (effectiveTags.value.length ? 1 : 0) +
    (parsedQuery.value.paths.length ? 1 : 0)
)

const searchFilterArgs = computed(() => ({
  extensions: extensionsParam.value,
  modifiedAfter: effectiveTimeRange.value.after,
  modifiedBefore: effectiveTimeRange.value.before,
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
  if (!effectiveTags.value.length) return null
  const kb = activeKbName.value
  if (!kb) return new Set()
  try {
    const paths = await invoke('kg_files_by_tags', {
      kbName: kb,
      tags: [...effectiveTags.value],
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

/**
 * 按输入框里的 `path:` 记号筛结果。
 *
 * 后端没有"路径前缀"这个参数，只能拿回结果之后在这里筛。代价是被筛掉的那些
 * 白占了后端的名额，所以带 path: 时会把 limit 放大几倍垫着（见 previewLimit）。
 */
const filterByPathTokens = (items) => {
  const paths = parsedQuery.value.paths
  if (!paths.length) return items
  return items.filter((it) => matchesPathFilters(it.rel_path || it.path, paths))
}

// 监听筛选项变化，自动刷新下拉预览
watch(
  [filterTimePreset, filterCustomStart, filterCustomEnd, filterExts, filterTags, filterTagMatchAll],
  () => scheduleFilterSearch(),
  { deep: true }
)

watch(trimmedQuery, (q) => {
  if (q) return

  // 命令模式下 trimmedQuery 必然是空的，但那只是"暂时不在搜东西"，
  // 不代表用户清空了搜索框——这时候把右侧面板清掉，敲个 `>` 就等于丢结果
  if (isCommandMode.value) return

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
// 设计说明：下拉框只是即时预览，固定限制条数；用户确认搜索后使用 limit=0 请求完整结果。
// 这样输入过程保持轻量，右侧正式结果面板则不会因默认 50 条分页而漏掉任何未忽略命中项。
const SEARCH_PREVIEW_LIMIT = 50
const SEARCH_ALL_RESULTS_LIMIT = 0
/**
 * 预览这一路实际要向后端要多少条。
 *
 * `path:` 是拿回结果之后在前端筛的，被筛掉的会白占名额——只要一条 path: 记号，
 * 就多要几倍垫着，否则用户会看到"明明有文件在那个目录下却一条都不出"。
 */
const previewLimit = computed(() =>
  parsedQuery.value.paths.length ? SEARCH_PREVIEW_LIMIT * 6 : SEARCH_PREVIEW_LIMIT
)
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

/**
 * 命令面板。输入框里打 `>` 就切过来。
 *
 * 为什么塞进搜索框而不是另开一个入口：这些操作都是"搜不到想要的东西时"才想起来要做的
 * ——索引旧了、筛选忘了清、正则开关还开着。手已经在搜索框里了，不该再去菜单里翻。
 */
const COMMAND_ITEMS = [
  {
    id: 'rebuild-file-index',
    label: '重建文件名索引',
    hint: '文件名搜索漏文件或明显变慢时用',
    keywords: 'rebuild file index 索引 文件名 chongjian',
    icon: mdiFileSearchOutline,
  },
  {
    id: 'rebuild-fulltext-index',
    label: '重建正文全文索引',
    hint: '正文搜索结果对不上磁盘内容时用',
    keywords: 'rebuild fulltext index 全文 正文 索引 chongjian',
    icon: mdiTextSearch,
  },
  {
    id: 'clear-filters',
    label: '清除全部筛选',
    hint: '时间段、文件类型、标签一起清掉',
    keywords: 'clear filter 清除 筛选 qingchu',
    icon: mdiFilterOffOutline,
  },
  {
    id: 'toggle-regex',
    label: '切换正则匹配',
    hint: '关闭时按字面量搜索，标点不当通配符',
    keywords: 'regex 正则 zhengze',
    icon: mdiRegex,
  },
  {
    id: 'toggle-whole-word',
    label: '切换全词匹配',
    hint: '开启后只认整词，中文的词边界按分词判断',
    keywords: 'whole word 全词 quanci',
    icon: mdiFormatLetterMatches,
  },
  {
    id: 'toggle-case',
    label: '切换区分大小写',
    hint: '关闭时按智能大小写：查询里有大写才区分',
    keywords: 'case sensitive 大小写 daxiaoxie',
    icon: mdiFormatLetterCase,
  },
]

/**
 * 命令的当前状态，显示在右侧。
 * 三个开关是 on/off 的，光看"切换正则匹配"不知道现在是开还是关。
 */
const commandStateText = (id) => {
  if (id === 'toggle-regex') return matchFlags.value.regex ? '已开启' : '已关闭'
  if (id === 'toggle-whole-word') return matchFlags.value.wholeWord ? '已开启' : '已关闭'
  if (id === 'toggle-case') return matchFlags.value.caseSensitive ? '已开启' : '已关闭'
  if (id === 'clear-filters') return activeFilterCount.value ? `${activeFilterCount.value} 项` : '无'
  return ''
}

/** 子串匹配，label / hint / keywords 一起找。`>` 后面还没敲字就列出全部。 */
const matchedCommands = computed(() => {
  const q = (commandQuery.value || '').trim().toLowerCase()
  const items = q
    ? COMMAND_ITEMS.filter((c) => `${c.label} ${c.hint} ${c.keywords}`.toLowerCase().includes(q))
    : COMMAND_ITEMS
  return items.map((c) => ({ ...c, state: commandStateText(c.id) }))
})

const selectedRowKey = ref('')
const flashSelectRow = async (key) => {
  selectedRowKey.value = key
  await new Promise(resolve => setTimeout(resolve, 140))
  if (selectedRowKey.value === key) selectedRowKey.value = ''
}

const dropdownSelectableKeys = computed(() => {
  const keys = []

  // 命令模式下只有命令能选：类型菜单、历史、结果这时都不显示
  if (isCommandMode.value) {
    for (const c of matchedCommands.value) keys.push(`command:${c.id}`)
    return keys
  }

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

/**
 * 一"路"搜索的代际与取消管理。
 *
 * 原来每次输入都直接 invoke：既不传 search_id（后端那套 SEARCH_MANAGER 取消机制
 * 因此形同虚设），也不校验返回顺序——慢的旧请求后返回时，下拉里显示的是上一个词的结果。
 * 这里给每一轮搜索发一个代号：只认最新一代的响应，并把上一代的 id 送去后端取消，
 * 让 rayon 的 par_iter 立刻短路，不再白烧 CPU 扫完整个目录。
 *
 * 预览和正式搜索分成两路：取消会让结果静默截断，所以预览那一路的自动重跑
 * 绝不能去掐用户按回车发起的正式搜索。
 */
const createSearchLane = (name) => {
  let generation = 0
  let activeIds = []
  const cancelActive = () => {
    const ids = activeIds
    activeIds = []
    for (const id of ids) {
      invoke('cancel_search', { searchId: id }).catch(() => {})
    }
  }
  return {
    /**
     * 开一轮新搜索。每路必须用不同的 id：后端命令跑完会自己摘掉登记，
     * 共用一个 id 会让还在跑的那路被误判成"已取消"，结果凭空少一半。
     */
    begin(kinds) {
      cancelActive()
      const gen = ++generation
      activeIds = kinds.map((k) => `${name}-${k}-${gen}`)
      return { gen, ids: activeIds }
    },
    isStale: (gen) => gen !== generation,
    finish(gen) {
      // 正常跑完的请求后端已自行摘登记，不必再发一次取消
      if (gen === generation) activeIds = []
    },
    cancelActive,
  }
}

const previewLane = createSearchLane('preview')
const fullLane = createSearchLane('full')

/**
 * 让后端把文件名索引对到当前搜索根上。
 *
 * 搜索本身不需要等它——`file_index_search` 在索引没建好时会自己退回目录遍历，
 * 所以这里只是"提前打个招呼"，让用户敲第一个字符时就已经是快的那条路。
 */
const ensureFileIndex = async () => {
  const root = searchRootPath.value
  if (!root) return
  try {
    const status = await invoke('file_index_ensure', { path: root })
    logger.info('[GuaxiHeader] file index', status)
  } catch (err) {
    // 索引只是加速手段，建不起来不该影响搜索
    logger.warn('[GuaxiHeader] ensure file index failed', err)
  }
}

/**
 * 让后端把正文全文索引对到当前搜索根上。
 *
 * 和文件名索引分开发起：正文索引要读文件内容，第一次建会明显慢一些，
 * 不该把文件名那一路也拖住。同样不用等——`fulltext_search` 在索引没建好时
 * 会自己退回 `rg_search`。
 */
const ensureFulltextIndex = async () => {
  const root = searchRootPath.value
  if (!root) return
  try {
    const status = await invoke('fulltext_ensure', { path: root })
    logger.info('[GuaxiHeader] fulltext index', status)
  } catch (err) {
    logger.warn('[GuaxiHeader] ensure fulltext index failed', err)
  }
}

// 换库/换知识库就得换索引根
watch(searchRootPath, () => {
  ensureFileIndex()
  ensureFulltextIndex()
})

const rebuildingCommand = ref('')

/**
 * 执行命令面板里选中的命令。
 *
 * 重建索引传 `force: true`：走这条路的人已经认定索引不对了，
 * `ifNeeded` 那套"看起来不用重建就跳过"的判断正是他要绕开的东西。
 */
const runSearchCommand = async (id) => {
  const item = COMMAND_ITEMS.find((c) => c.id === id)
  if (!item) return

  if (id === 'clear-filters') {
    clearFilters()
    searchQuery.value = ''
    message.success('已清除全部筛选')
    nextTick(() => searchInputRef.value?.focus?.())
    return
  }

  if (id === 'toggle-regex' || id === 'toggle-whole-word' || id === 'toggle-case') {
    const flag = id === 'toggle-regex' ? 'regex' : id === 'toggle-whole-word' ? 'wholeWord' : 'caseSensitive'
    toggleMatchFlag(flag)
    searchQuery.value = ''
    message.success(`${item.label.replace('切换', '')}${matchFlags.value[flag] ? '已开启' : '已关闭'}`)
    return
  }

  const root = searchRootPath.value
  if (!root) {
    message.warning('还没有打开的目录，无法重建索引')
    return
  }
  if (rebuildingCommand.value) {
    message.info('索引正在重建，稍等一下')
    return
  }

  const command = id === 'rebuild-file-index' ? 'file_index_ensure' : 'fulltext_ensure'
  rebuildingCommand.value = id
  searchQuery.value = ''
  try {
    const status = await invoke(command, { path: root, force: true })
    logger.info('[GuaxiHeader] rebuild index', { command, status })
    // 后端只是把重建排进后台就返回了，这里不能报"完成"——
    // 拿到的 file_count 还是旧索引的数字
    message.success(`${item.label}已开始，建好后自动生效`)
  } catch (err) {
    logger.warn('[GuaxiHeader] rebuild index failed', { command, err })
    message.error(`${item.label}失败：${err}`)
  } finally {
    rebuildingCommand.value = ''
    nextTick(() => searchInputRef.value?.focus?.())
  }
}

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
        // 后端现在会按整条相对路径匹配，命中可能来自目录名，
        // 所以相对路径和文件名高亮区间都得带到结果面板去，不然用户不知道为什么这个文件会中。
        rel_path: item.rel_path ? String(item.rel_path) : '',
        name_matches: Array.isArray(item.name_matches) ? item.name_matches : [],
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
        // 结果面板把命中按文件分组，组头要显示相对目录（同名文件太多，光文件名认不出是哪个）。
        // 顺带修好 path: 记号：filterByPathTokens 用 rel_path || path，以前这里没带 rel_path，
        // 内容搜索只能拿绝对路径去比，`path:` 写搜索根里的任意一段都能全中。
        rel_path: item.rel_path ? String(item.rel_path) : '',
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

/**
 * 统一解析后端 `(results, has_more)` 搜索响应，同时兼容旧版直接返回数组的格式。
 * @param {unknown} response Tauri 搜索命令返回值
 * @returns {{ results: Array, hasMore: boolean }} 标准化分页结果
 */
const unpackPagedSearchResponse = (response) => {
  if (Array.isArray(response) && Array.isArray(response[0])) {
    return {
      results: response[0],
      hasMore: Boolean(response[1]),
    }
  }
  if (Array.isArray(response)) return { results: response, hasMore: false }
  if (response && typeof response === 'object') return { results: [response], hasMore: false }
  return { results: [], hasMore: false }
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
        h(Icon, { icon: mdiHistory, width: 16, height: 16, class: 'search-dd-icon search-dd-icon-muted' })
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

/**
 * 根据结果类型执行正式完整搜索。
 * 下拉项和历史项只能作为导航入口，不能把有限条数的预览误当成最终搜索结果。
 * @param {'file'|'note'|'excerpt'} type 搜索结果类型
 */
async function runCompleteSearchForType(type) {
  logger.info('[GuaxiHeader] run complete search from result selection', {
    type,
    query: searchQuery.value,
  })
  if (type === 'note') return handleSearchNotes()
  if (type === 'excerpt') return handleSearchExcerpts()
  return handleSearchFiles()
}

const handleSearchSelect = async (key) => {
  if (typeof key === 'string' && key.startsWith('command:')) {
    await runSearchCommand(key.slice('command:'.length))
    return
  }

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
    await runCompleteSearchForType(t)

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
    await runCompleteSearchForType(t)

    await flashSelectRow(key)

    if (item._searchType !== 'excerpt') {
      await openResultInEditor(item)
    }

    isSearchDropdownVisible.value = false
  }
}

const runPreviewSearchAll = async () => {
  // 命令模式：下拉里显示的是命令列表，这时候搜索既没结果可显示也没必要发起。
  // 直接返回而不是清空——退出命令模式后原来的结果还在，不用重搜一遍
  if (isCommandMode.value) return

  const q = trimmedQuery.value
  // 有关键字、或仅有筛选条件时都执行搜索
  if ((!q && !hasActiveFilters.value) || !isSearchDropdownVisible.value) {
    previewLane.cancelActive()
    previewResultsByType.value = { file: [], note: [], excerpt: [] }
    previewResults.value = []
    previewError.value = ''
    previewHasMore.value = false
    return
  }

  const { gen, ids: [fdId, rgId] } = previewLane.begin(['fd', 'rg'])
  previewLoading.value = true
  previewError.value = ''
  previewHasMore.value = false

  try {
    const { extensions, modifiedAfter, modifiedBefore } = searchFilterArgs.value
    const options = searchMatchOptions.value
    const tagSetPromise = resolveTagPathSet()

    // 关键字搜索(全文索引) / 摘录搜索需要关键字；仅筛选时只列举文件(fd)。
    const [fdResp, rgResp, exResp] = await Promise.allSettled([
      // 逐字符触发的这一路走索引：参数和 fd_search 完全一致，
      // 索引没建好时后端会自己退回 fd_search，这里不用判断。
      invoke('file_index_search', {
        pattern: q,
        path: searchRootPath.value,
        limit: previewLimit.value,
        offset: 0,
        searchId: fdId,
        extensions,
        modifiedAfter,
        modifiedBefore,
        options,
      }),
      q
        ? invoke('fulltext_search', {
          // 正文这一路走全文索引：BM25 排序 + 中文分词，能命中分散在同一篇里的两个词。
          // 参数与 rg_search 完全一致，索引不可用（还没建好、开了正则/全词）时后端自己退回 rg_search。
          pattern: q,
          path: searchRootPath.value,
          limit: previewLimit.value,
          offset: 0,
          searchId: rgId,
          extensions,
          modifiedAfter,
          modifiedBefore,
          options,
        })
        : Promise.resolve(null),
      q
        ? invoke('search_excerpt_notes', { pattern: q, path: settingsStore.librariesRoot })
        : Promise.resolve(null)
    ])

    // 用户还在打字，这一轮已经不是最新的：结果直接丢掉，不能盖住新一轮
    if (previewLane.isStale(gen)) return

    const tagSet = await tagSetPromise
    if (previewLane.isStale(gen)) return
    const next = { file: [], note: [], excerpt: [] }

    if (fdResp.status === 'fulfilled' && fdResp.value) {
      const { results, hasMore } = unpackPagedSearchResponse(fdResp.value)
      next.file = filterByPathTokens(
        filterByTagPaths(normalizeSearchResults(results, 'file'), tagSet)
      ).slice(0, SEARCH_PREVIEW_LIMIT)
      previewHasMore.value = previewHasMore.value || Boolean(hasMore)
    }

    if (rgResp.status === 'fulfilled' && rgResp.value) {
      const { results, hasMore } = unpackPagedSearchResponse(rgResp.value)
      next.note = filterByPathTokens(
        filterByTagPaths(normalizeSearchResults(results, 'note'), tagSet)
      ).slice(0, SEARCH_PREVIEW_LIMIT)
      previewHasMore.value = previewHasMore.value || Boolean(hasMore)
    }

    if (exResp.status === 'fulfilled' && exResp.value) {
      next.excerpt = normalizeSearchResults(exResp.value, 'excerpt')
    }

    previewResultsByType.value = next
    previewResults.value = [...next.file, ...next.note, ...next.excerpt]

    logger.info('[GuaxiHeader] preview search completed', {
      query: q,
      root: searchRootPath.value,
      fileCount: next.file.length,
      noteCount: next.note.length,
      excerptCount: next.excerpt.length,
      hasMore: previewHasMore.value,
    })

    /*
     * 逐路上报失败。原来只有三路全挂才提示"搜索失败"，于是最常见的一种失败
     * ——正则写错了，只有 rg 那一路报错——会被文件名结果盖住，用户看到的是
     * "有结果啊，只是没找到我要的那行"，完全不知道内容搜索根本没跑成。
     */
    const failures = []
    if (fdResp.status === 'rejected') failures.push(`文件名（${formatInvokeError(fdResp.reason)}）`)
    if (rgResp.status === 'rejected') failures.push(`内容（${formatInvokeError(rgResp.reason)}）`)
    if (exResp.status === 'rejected') failures.push(`摘录（${formatInvokeError(exResp.reason)}）`)
    previewError.value = failures.length ? `搜索未全部完成：${failures.join('；')}` : ''
    if (failures.length) {
      logger.warn('[GuaxiHeader] preview search partial failure', { query: q, failures })
    }
  } catch (err) {
    if (previewLane.isStale(gen)) return
    previewResultsByType.value = { file: [], note: [], excerpt: [] }
    previewResults.value = []
    previewError.value = `搜索失败：${formatInvokeError(err)}`
  } finally {
    // 已被新一轮接管时不要动 loading，否则会把新一轮的"搜索中"提前熄掉
    if (!previewLane.isStale(gen)) {
      previewLoading.value = false
      previewLane.finish(gen)
    }
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

  ensureFileIndex()
  ensureFulltextIndex()
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
  // 组件都没了还让后端接着扫目录是纯浪费
  previewLane.cancelActive()
  fullLane.cancelActive()

})

async function handleSearchFiles() {
  const { gen, ids: [fdId] } = fullLane.begin(['fd'])
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
      pattern: trimmedQuery.value,
      path: searchRootPath.value,
      // limit=0 是后端约定的完整结果模式，正式搜索不得沿用 50 条预览上限。
      limit: SEARCH_ALL_RESULTS_LIMIT,
      offset: 0,
      searchId: fdId,
      extensions,
      modifiedAfter,
      modifiedBefore,
      options: searchMatchOptions.value
    })

    // 已有更新的一次正式搜索发出，这次的结果作废
    if (fullLane.isStale(gen)) return

    const { results, hasMore } = unpackPagedSearchResponse(rawResponse)
    const validResults = filterByPathTokens(
      filterByTagPaths(normalizeSearchResults(results, 'file'), tagSet)
    )

    logger.info('[GuaxiHeader] complete file search received', {
      query: searchQuery.value,
      root: searchRootPath.value,
      resultCount: validResults.length,
      hasMore,
    })

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
    if (fullLane.isStale(gen)) return
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      fileResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'file',
      loading: false,
      error: formatInvokeError(error) || '文件搜索失败'
    })
    message.error(`文件搜索失败: ${formatInvokeError(error)}`)
  } finally {
    fullLane.finish(gen)
  }
}

async function handleSearchNotes() {
  const { gen, ids: [rgId] } = fullLane.begin(['rg'])
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
      pattern: trimmedQuery.value,
      path: searchRootPath.value,
      // limit=0 请求全部内容命中；结果面板使用虚拟列表，不会一次渲染全部 DOM。
      limit: SEARCH_ALL_RESULTS_LIMIT,
      offset: 0,
      searchId: rgId,
      extensions,
      modifiedAfter,
      modifiedBefore,
      options: searchMatchOptions.value
    })

    if (fullLane.isStale(gen)) return

    const { results, hasMore } = unpackPagedSearchResponse(response)
    const validResults = filterByPathTokens(
      filterByTagPaths(normalizeSearchResults(results, 'note'), tagSet)
    )
    const uniqueFileCount = new Set(validResults.map(item => normalizeSlashes(item.path).toLowerCase())).size

    logger.info('[GuaxiHeader] complete content search received', {
      query: searchQuery.value,
      root: searchRootPath.value,
      resultCount: validResults.length,
      uniqueFileCount,
      hasMore,
    })

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
    if (fullLane.isStale(gen)) return
    rightSiderStore.setSearchResults({
      ...rightSiderStore.searchResults,
      noteResults: [],
      searchQuery: searchQuery.value,
      lastSearchType: 'note',
      error: formatInvokeError(err) || '内容搜索失败',
      loading: false
    })
    message.error(`内容搜索失败: ${formatInvokeError(err)}`)
  } finally {
    fullLane.finish(gen)
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
      pattern: trimmedQuery.value,
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

/* 编辑区装饰条开关：显示时给一点强调色，和侧栏开关的「已开启」观感一致。 */
.editor-chrome-toggle.active {
  color: var(--guaxi-accent, #2f6fdf);
  background-color: rgba(47, 111, 223, 0.12);
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

/* ───── 命令面板 ───── */
.search-dd-row-command {
  display: flex;
  align-items: center;
  gap: 8px;
}

.search-dd-cmd-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.search-dd-cmd-label {
  font-size: 13px;
  color: var(--guaxi-text-primary);
}

.search-dd-cmd-hint {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.search-dd-cmd-state {
  flex: none;
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  color: var(--guaxi-text-secondary);
  background: rgba(255, 255, 255, 0.06);
}

.search-dd-syntax {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  padding: 4px 10px 8px;
}

.search-dd-syntax-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.search-dd-syntax code {
  font-size: 11px;
  padding: 1px 5px;
  border-radius: 4px;
  color: var(--guaxi-text-secondary);
  background: rgba(255, 255, 255, 0.06);
}

.search-dd-empty {
  padding: 8px 10px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

/* 部分搜索失败的提示：和结果并存，所以要能一眼看出是警告 */
.search-dd-error {
  margin: 6px 8px;
  padding: 6px 8px;
  border-radius: 6px;
  border-left: 3px solid var(--n-color-warning, #f0a020);
  background: rgba(240, 160, 32, 0.1);
  font-size: 12px;
  line-height: 1.5;
  color: var(--guaxi-text-primary);
  word-break: break-all;
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

/* 搜索框内的匹配方式开关：区分大小写 / 全词 / 正则 */
.search-mode-group {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  margin-right: 4px;
}
.search-mode-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 20px;
  padding: 0 3px;
  border-radius: 5px;
  cursor: pointer;
  user-select: none;
  font-size: 11px;
  line-height: 1;
  font-family: var(--guaxi-font-mono, ui-monospace, Consolas, monospace);
  color: var(--guaxi-text-secondary);
  transition: color 0.15s, background-color 0.15s;
}
.search-mode-btn:hover {
  color: var(--n-color-primary, #1890ff);
  background-color: rgba(24, 144, 255, 0.12);
}
.search-mode-btn.on {
  color: #fff;
  background-color: var(--n-color-primary, #1890ff);
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
