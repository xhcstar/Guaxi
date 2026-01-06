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
          @keyup.enter="handleSearchEnter" @focus="showSearchOptions" @blur="handleSearchBlur">
          <template #prefix>
            <n-icon :component="Search32Filled" />
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
        新建知识库
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="TextBulletListLtr20Filled" @click="openOutlinePanel"></n-icon>
        </template>
        大纲
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="Box20Regular" @click="openTemplateManager" />
        </template>
        模板管理
      </n-tooltip>

      <n-tooltip placement="bottom" trigger="hover">
        <template #trigger>
          <n-icon class="header-icon" size="20" :component="Screenshot24Regular" @click="CaptureScreenshot"></n-icon>
        </template>
        截图并插入
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
        纵向分屏
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
        横向分屏
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
      class="search-dd-root"
      :style="dropdownFloatingStyle"
      @mousedown.prevent
    >
      <div class="search-dd-section">
        <div
          v-for="item in typeMenuItems"
          :key="item.key"
          class="search-dd-row search-dd-row-type"
          :class="{ active: item.key === searchType }"
          @click="handleSearchSelect(item.key)"
        >
          <span class="search-dd-icon-wrap">
            <n-icon :component="item.icon" />
          </span>
          <span class="search-dd-row-text">{{ item.label }}</span>
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
import { computed, ref, h, onMounted, onUnmounted, nextTick, watch } from "vue";
import { NIcon, NTooltip } from 'naive-ui'
import { useSiderStore } from '../../stores/useSiderStore'
import { useFileTreeStore } from '../../stores/fileTreeStore'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useEditorStore } from '../../stores/editorStore'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import CreateKnowledgeBaseForm from '../BrainViewComponent/CreateKnowledgeBaseForm.vue'

import { useMessage } from 'naive-ui';
import { debounce } from 'lodash-es'
import { Icon } from '@iconify/vue'
import {
  Screenshot24Regular,
  Search32Filled,
  TextBulletListLtr20Filled,
  DocumentSearch20Filled,
  DocumentSearch24Regular,
  NotebookSection24Regular,
  BookAdd20Regular,
  Box20Regular,
  PanelLeftContract24Regular,
  PanelRightContract24Regular
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'


const rightSiderStore = useRightSiderStore()
const settingsStore = useSettingsStore()
const siderStore = useSiderStore();
const fileTreeStore = useFileTreeStore()
const editorStore = useEditorStore()
const knowledgeBaseStore = useKnowledgeBaseStore()
const searchInputRef = ref(null)
const searchContainerRef = ref(null)
const searchType = ref('file')

const viewKey = 'home'
const message = useMessage();

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

const openCreateKnowledgeBase = () => {
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
    console.error('打开模板管理器失败:', err)
    message.error('打开模板管理器失败')
  }
}

async function CaptureScreenshot() {
  try {
    const originalPath = fileTreeStore.selectedFile?.path
    console.debug('[Screenshot] Triggered', {
      activeTab: fileTreeStore.activeTab,
      selectedPath: originalPath
    })

    if (!originalPath) {
      message.warning('请先打开一个文件')
      console.warn('[Screenshot] Aborted: no selected file path')
      return
    }

    console.debug('[Screenshot] Invoking take_screenshot_markdown', {
      ownerFilePath: originalPath,
      alt: 'screenshot'
    })

    const result = await invoke('take_screenshot_markdown', {
      ownerFilePath: originalPath,
      alt: 'screenshot'
    })

    console.debug('[Screenshot] Backend returned', {
      relative_path: result?.relative_path,
      absolute_path: result?.absolute_path,
      url: result?.url,
      markdown_len: typeof result?.markdown === 'string' ? result.markdown.length : null
    })

    const markdown = result?.markdown
    if (typeof markdown !== 'string' || !markdown) {
      throw new Error('后端未返回 markdown')
    }

    console.debug('[Screenshot] Inserting markdown into editor')
    const inserted = await editorStore.insertContent(`${markdown}\n`)
    console.debug('[Screenshot] Insert result', { inserted })

    if (!inserted) {
      throw new Error('插入失败：编辑器未就绪')
    }
  } catch (err) {
    console.error('[Screenshot] Failed:', err)
    message.error('截图失败')
  }
}

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

const typeMenuItems = computed(() => ([
  { key: 'file', label: SEARCH_TYPE_LABELS.file, icon: DocumentSearch20Filled },
  { key: 'note', label: SEARCH_TYPE_LABELS.note, icon: DocumentSearch24Regular },
  { key: 'excerpt', label: SEARCH_TYPE_LABELS.excerpt, icon: NotebookSection24Regular }
]))

const historyItems = computed(() => getHistorySuggestions(trimmedQuery.value))

const selectedRowKey = ref('')
const flashSelectRow = async (key) => {
  selectedRowKey.value = key
  await new Promise(resolve => setTimeout(resolve, 140))
  if (selectedRowKey.value === key) selectedRowKey.value = ''
}

const getItemAbsPath = (item) => {
  const t = item?._searchType || searchType.value
  return t === 'excerpt' ? item.file_path : item.path
}

const getItemRel = (item) => {
  const t = item?._searchType || searchType.value
  const absPath = getItemAbsPath(item)
  const base = t === 'excerpt' ? settingsStore.librariesRoot : fileTreeStore.activeRepoPath
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

const basename = (p) => String(p || '').split(/[\\/]/).pop() || ''
const dirnameFromPath = (p) => {
  const parts = String(p || '').replace(/\\\\/g, '/').split('/')
  parts.pop()
  return parts.join('/')
}
const normalizeSlashes = (p) => String(p || '').replace(/\\\\/g, '/')
const toRelativePath = (absPath, basePath) => {
  const abs = normalizeSlashes(absPath)
  const base = normalizeSlashes(basePath)
  if (!base) return abs
  if (abs === base) return ''
  if (abs.startsWith(base + '/')) return abs.slice(base.length + 1)
  return abs
}

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

const getHistorySuggestions = (input) => {
  const q = String(input || '').trim().toLowerCase()
  const now = Date.now()

  const scored = searchHistory.value
    .filter(e => e && e.key)
    .map(e => {
      const t = e.type
      const absPath = t === 'excerpt' ? String(e.file_path || '') : String(e.path || '')
      const base = t === 'excerpt' ? settingsStore.librariesRoot : fileTreeStore.activeRepoPath
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

const normalizeSearchResults = (rawResults, type) => {
  if (!Array.isArray(rawResults)) return []

  if (type === 'file') {
    return rawResults
      .filter(item => item && typeof item === 'object')
      .map(item => ({
        path: String(item.path || ''),
        file_name: String(item.file_name || basename(item.path)),
        _searchType: 'file'
      }))
      .filter(item => item.path)
  }

  if (type === 'note') {
    return rawResults
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

const SEARCH_TYPE_KEYS = ['file', 'note', 'excerpt']
const SEARCH_TYPE_LABELS = {
  file: '搜索文件',
  note: '搜索关键字',
  excerpt: '搜索摘录笔记'
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
  const base = type === 'excerpt' ? settingsStore.librariesRoot : fileTreeStore.activeRepoPath
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

const handleSearchBlur = () => {
  setTimeout(() => {
    isSearchDropdownVisible.value = false
  }, 200)
}

const openResultInEditor = async (item) => {
  const result = await fileTreeStore.fileTreeOpenSearchResult({
    ...item,
    searchText: searchQuery.value
  })

  editorStore.highlightInfo = {
    path: result.path,
    lineNumber: result.lineNumber,
    matches: result.matches,
    text: result.text,
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
  if (!q) return

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
  if (!q || !isSearchDropdownVisible.value) {
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
    const [fdResp, rgResp, exResp] = await Promise.allSettled([
      invoke('fd_search', { pattern: q, path: fileTreeStore.activeRepoPath }),
      invoke('rg_search', { pattern: q, path: fileTreeStore.activeRepoPath }),
      invoke('search_excerpt_notes', { pattern: q, path: settingsStore.librariesRoot })
    ])

    const next = { file: [], note: [], excerpt: [] }

    if (fdResp.status === 'fulfilled') {
      const [rawResults = [], hasMore = false] = Array.isArray(fdResp.value) ? fdResp.value : []
      next.file = normalizeSearchResults(rawResults, 'file')
      previewHasMore.value = previewHasMore.value || Boolean(hasMore)
    }

    if (rgResp.status === 'fulfilled') {
      const response = rgResp.value
      const rawResults = (() => {
        if (!response) return []
        if (Array.isArray(response[0])) return response[0]
        if (Array.isArray(response)) return response
        return [response]
      })()
      next.note = normalizeSearchResults(rawResults, 'note')
    }

    if (exResp.status === 'fulfilled') {
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

onMounted(() => {
  window.addEventListener('resize', handleWindowReflow)
  window.addEventListener('scroll', handleWindowReflow, true)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleWindowReflow)
  window.removeEventListener('scroll', handleWindowReflow, true)
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

    const rawResponse = await invoke('fd_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    })

    const [rawResults = [], hasMore = false] = Array.isArray(rawResponse) ? rawResponse : []
    const validResults = normalizeSearchResults(rawResults, 'file')

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

    const response = await invoke('rg_search', {
      pattern: searchQuery.value,
      path: fileTreeStore.activeRepoPath
    })

    const rawResults = (() => {
      if (!response) return []
      if (Array.isArray(response[0])) return response[0]
      if (Array.isArray(response)) return response
      return [response]
    })()

    const validResults = normalizeSearchResults(rawResults, 'note')

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
  color: rgba(0, 0, 0, 0.65);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding-left: 28px;
}
</style>