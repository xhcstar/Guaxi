<template>
  <aside
    v-if="visible"
    class="guaxi-usage-assistant"
    :class="themeClass"
    :style="panelPositionStyle"
    @mousedown.stop
    @click.stop
  >
    <header class="guaxi-usage-assistant__head">
      <div>
        <div class="guaxi-usage-assistant__title">离线文档助手</div>
        <div class="guaxi-usage-assistant__meta">{{ sourceMetaText }}</div>
      </div>
      <button
        type="button"
        class="guaxi-usage-assistant__icon-btn"
        title="关闭"
        aria-label="关闭离线文档助手"
        @click="emit('close')"
      >
        <Icon :icon="xIcon" width="16" height="16" />
      </button>
    </header>

    <div class="guaxi-usage-assistant__tabs" role="status">
      <span class="guaxi-usage-assistant__tab is-active">
        {{ activeChannelLabel }}
      </span>
    </div>

    <div class="guaxi-usage-assistant__search-row">
      <Icon :icon="searchIcon" width="15" height="15" />
      <input
        v-model="query"
        class="guaxi-usage-assistant__search"
        type="text"
        placeholder="搜索语法、接口、参数或示例"
        @keydown.enter.prevent="runSearch('enter')"
      />
      <button
        type="button"
        class="guaxi-usage-assistant__icon-btn"
        title="搜索"
        aria-label="搜索离线文档"
        @click="runSearch('button')"
      >
        <Icon :icon="arrowRightIcon" width="15" height="15" />
      </button>
    </div>

    <div v-if="loading" class="guaxi-usage-assistant__state">正在读取离线索引...</div>
    <div v-else-if="loadError" class="guaxi-usage-assistant__state is-warning">
      <div>{{ loadError }}</div>
      <code>npm run docs:python</code>
    </div>
    <div v-else class="guaxi-usage-assistant__body">
      <article
        v-for="entry in results"
        :key="entry.id"
        class="guaxi-usage-assistant__entry"
      >
        <div class="guaxi-usage-assistant__entry-head">
          <span class="guaxi-usage-assistant__kind">{{ entry.kind || entry.group || '文档' }}</span>
          <button
            v-if="entry.localUrl || entry.sourceUrl"
            type="button"
            class="guaxi-usage-assistant__doc-link"
            title="打开完整文档"
            aria-label="打开完整文档"
            @click="openDocument(entry)"
          >
            <Icon :icon="externalLinkIcon" width="13" height="13" />
          </button>
        </div>
        <h3>{{ entry.title }}</h3>
        <p>{{ entry.excerpt || entry.summary }}</p>
        <div v-if="Array.isArray(entry.headings) && entry.headings.length" class="guaxi-usage-assistant__headings">
          <span v-for="heading in entry.headings.slice(0, 4)" :key="heading">{{ heading }}</span>
        </div>
        <div v-if="Array.isArray(entry.examples) && entry.examples.length" class="guaxi-usage-assistant__examples">
          <button
            v-for="example in entry.examples"
            :key="example.label + example.code"
            type="button"
            class="guaxi-usage-assistant__example-btn"
            @click="insertExample(entry, example)"
          >
            <Icon :icon="plusIcon" width="13" height="13" />
            <span>{{ example.label || '插入示例' }}</span>
          </button>
        </div>
        <div class="guaxi-usage-assistant__entry-actions">
          <button
            type="button"
            class="guaxi-usage-assistant__text-btn"
            @click="askAssistant(entry)"
          >
            <Icon :icon="messageCircleIcon" width="13" height="13" />
            <span>问智能助手</span>
          </button>
        </div>
      </article>

      <div v-if="!results.length" class="guaxi-usage-assistant__state">
        没有匹配结果，换个关键词试试。
      </div>
    </div>
  </aside>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import searchIcon from '@iconify-icons/lucide/search'
import arrowRightIcon from '@iconify-icons/lucide/arrow-right'
import externalLinkIcon from '@iconify-icons/lucide/external-link'
import messageCircleIcon from '@iconify-icons/lucide/message-circle'
import plusIcon from '@iconify-icons/lucide/plus'
import xIcon from '@iconify-icons/lucide/x'
import { createLogger } from '../../utils/logger'
import {
  NOTEBOOK_USAGE_GUIDE_CHANNELS,
  buildNotebookUsageAssistantPrompt,
  inferNotebookUsageGuideQuery,
  queryNotebookUsageGuide,
  queryPythonOfficialDocs,
} from './utils/guaxiNotebookUsageGuide'

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  channel: {
    type: String,
    default: 'python',
  },
  contextContent: {
    type: String,
    default: '',
  },
  anchorPosition: {
    type: Object,
    default: () => ({}),
  },
  theme: {
    type: String,
    default: 'light',
  },
})

const emit = defineEmits(['close', 'insert-example', 'ask-assistant', 'update:channel'])

const logger = createLogger('GuaxiNotebookUsageAssistant')
const activeChannel = ref(props.channel === 'math' ? 'math' : 'python')
const query = ref('')
const results = ref([])
const loading = ref(false)
const loadError = ref('')
const pythonManifest = ref(null)
let searchRequestSeq = 0

const themeClass = computed(() => (String(props.theme || '').toLowerCase() === 'dark' ? 'theme-dark' : 'theme-light'))
const panelPositionStyle = computed(() => {
  const left = Number(props.anchorPosition?.left)
  const top = Number(props.anchorPosition?.top)
  const width = Number(props.anchorPosition?.width)
  const maxHeight = Number(props.anchorPosition?.maxHeight)
  if (!Number.isFinite(left) || !Number.isFinite(top)) return {}

  return {
    left: `${Math.max(8, left)}px`,
    top: `${Math.max(8, top)}px`,
    right: 'auto',
    width: Number.isFinite(width) ? `${Math.max(280, width)}px` : undefined,
    maxHeight: Number.isFinite(maxHeight) ? `${Math.max(220, maxHeight)}px` : undefined,
  }
})
const sourceMetaText = computed(() => {
  if (activeChannel.value !== 'python') return 'LaTeX / SymPy / 数学块本地索引'
  const manifest = pythonManifest.value
  if (!manifest) return 'Python 官方完整文档索引'
  const lang = manifest.language === 'zh-cn' ? '简体中文' : '英文'
  return `${manifest.title || 'Python 官方完整文档'} · ${lang} · ${manifest.totalEntries || 0} 页`
})
const activeChannelLabel = computed(() => (
  NOTEBOOK_USAGE_GUIDE_CHANNELS.find((item) => item.key === activeChannel.value)?.label || 'Python'
))

const normalizeChannel = (value) => (value === 'math' ? 'math' : 'python')

const primeQueryFromContext = () => {
  query.value = inferNotebookUsageGuideQuery({
    channel: activeChannel.value,
    content: props.contextContent,
  })
}

const openDocument = async (entry) => {
  const rawUrl = String(entry?.localUrl || entry?.sourceUrl || '').trim()
  if (!rawUrl) return
  const resolvedUrl = new URL(rawUrl, window.location.href).toString()
  logger.info('[NotebookUsageAssistant] open official document', {
    channel: activeChannel.value,
    entryId: String(entry?.id || ''),
    url: resolvedUrl,
  })
  try {
    await openUrl(resolvedUrl)
  } catch (error) {
    logger.warn('[NotebookUsageAssistant] opener failed, fallback to window.open', {
      url: resolvedUrl,
      error: String(error?.message || error),
    })
    window.open(resolvedUrl, '_blank', 'noopener,noreferrer')
  }
}

const runSearch = async (reason = 'manual') => {
  const channel = activeChannel.value
  const requestId = ++searchRequestSeq
  loading.value = true
  loadError.value = ''
  results.value = []
  logger.info('[NotebookUsageAssistant] search offline docs', {
    channel,
    reason,
    query: query.value,
    requestId,
  })
  try {
    if (channel === 'python') {
      const outcome = await queryPythonOfficialDocs({
        query: query.value,
        limit: 24,
      })
      if (requestId !== searchRequestSeq || channel !== activeChannel.value) return
      pythonManifest.value = outcome.manifest || null
      results.value = outcome.entries || []
    } else {
      const nextResults = queryNotebookUsageGuide({
        channel,
        query: query.value,
        limit: 18,
      })
      if (requestId !== searchRequestSeq || channel !== activeChannel.value) return
      results.value = nextResults
    }
  } catch (error) {
    if (requestId !== searchRequestSeq || channel !== activeChannel.value) return
    results.value = []
    loadError.value = channel === 'python'
      ? '还没有导入 Python 官方完整文档。请先运行下方命令生成本地索引。'
      : `读取本地文档失败：${String(error?.message || error)}`
    logger.warn('[NotebookUsageAssistant] search failed', {
      channel,
      error: String(error?.message || error),
      requestId,
    })
  } finally {
    if (requestId === searchRequestSeq && channel === activeChannel.value) {
      loading.value = false
    }
  }
}

const insertExample = (entry, example) => {
  const code = String(example?.code || '')
  if (!code) return
  logger.info('[NotebookUsageAssistant] insert offline guide example', {
    channel: activeChannel.value,
    entryId: String(entry?.id || ''),
    label: String(example?.label || ''),
  })
  emit('insert-example', {
    channel: activeChannel.value,
    entry,
    example,
  })
}

const askAssistant = (entry) => {
  const prompt = buildNotebookUsageAssistantPrompt({
    channel: activeChannel.value,
    content: props.contextContent,
    guideEntry: entry,
  })
  logger.info('[NotebookUsageAssistant] send offline guide context to assistant', {
    channel: activeChannel.value,
    entryId: String(entry?.id || ''),
    promptLength: prompt.length,
  })
  emit('ask-assistant', {
    channel: activeChannel.value,
    entry,
    prompt,
  })
}

watch(() => props.channel, (value) => {
  activeChannel.value = normalizeChannel(value)
  results.value = []
  loadError.value = ''
})

watch(() => props.visible, (visible) => {
  if (!visible) return
  primeQueryFromContext()
  void runSearch('open')
  logger.info('[NotebookUsageAssistant] open assistant', {
    channel: activeChannel.value,
    contextLength: String(props.contextContent || '').length,
  })
})

watch(() => props.contextContent, () => {
  if (!props.visible) return
  primeQueryFromContext()
  void runSearch('context-change')
})

onMounted(() => {
  if (props.visible) {
    primeQueryFromContext()
    void runSearch('mounted')
  }
})
</script>

<style scoped>
.guaxi-usage-assistant {
  /* 设计说明：
   * 1. 离线文档助手由 Notebook 中任意 Cell 的帮助 icon 触发，absolute 会随 Notebook 根节点回到文档顶部坐标。
   * 2. 改为 fixed 后，父组件可以直接传入触发按钮的视口坐标，让面板始终贴近当前点击位置。
   */
  position: fixed;
  right: 16px;
  top: 56px;
  z-index: 70;
  width: min(520px, calc(100% - 32px));
  max-height: min(760px, calc(100vh - 120px));
  display: flex;
  flex-direction: column;
  border: 1px solid var(--guaxi-notebook-strong-border, rgba(148, 163, 184, 0.32));
  border-radius: 8px;
  background: var(--guaxi-notebook-panel, #ffffff);
  color: var(--guaxi-notebook-text, #0f172a);
  box-shadow: 0 24px 60px rgba(15, 23, 42, 0.22);
  overflow: hidden;
  font-family: var(--guaxi-notebook-font, 'Microsoft YaHei', sans-serif);
}

.guaxi-usage-assistant.theme-dark {
  background: var(--guaxi-notebook-panel, #111827);
  color: #e2e8f0;
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
}

.guaxi-usage-assistant__head,
.guaxi-usage-assistant__entry-head,
.guaxi-usage-assistant__entry-actions,
.guaxi-usage-assistant__search-row,
.guaxi-usage-assistant__tabs {
  display: flex;
  align-items: center;
}

.guaxi-usage-assistant__head {
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
}

.guaxi-usage-assistant__title {
  font-size: 15px;
  font-weight: 800;
}

.guaxi-usage-assistant__meta {
  margin-top: 3px;
  font-size: 12px;
  color: var(--guaxi-notebook-muted, #64748b);
}

.guaxi-usage-assistant__tabs {
  gap: 6px;
  padding: 10px 12px 0;
}

.guaxi-usage-assistant__tab,
.guaxi-usage-assistant__icon-btn,
.guaxi-usage-assistant__text-btn,
.guaxi-usage-assistant__example-btn {
  border: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
  background: transparent;
  color: inherit;
  cursor: pointer;
  transition: background 0.16s ease, border-color 0.16s ease;
}

.guaxi-usage-assistant__tab {
  display: inline-flex;
  align-items: center;
  min-height: 30px;
  padding: 5px 10px;
  border-radius: 8px;
  font-size: 13px;
  cursor: default;
}

.guaxi-usage-assistant__tab.is-active {
  background: color-mix(in srgb, #0891b2 14%, transparent);
  border-color: color-mix(in srgb, #0891b2 42%, var(--guaxi-notebook-border, #cbd5e1));
}

.guaxi-usage-assistant__search-row {
  gap: 8px;
  margin: 10px 12px 12px;
  padding: 7px 9px;
  border: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.24));
  border-radius: 8px;
  background: var(--guaxi-notebook-panel-soft, rgba(148, 163, 184, 0.08));
}

.guaxi-usage-assistant__search {
  flex: 1 1 auto;
  min-width: 0;
  border: 0;
  outline: none;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 13px;
}

.guaxi-usage-assistant__icon-btn {
  width: 30px;
  height: 30px;
  display: inline-grid;
  place-items: center;
  border-radius: 8px;
}

.guaxi-usage-assistant__body {
  min-height: 0;
  overflow: auto;
  padding: 0 12px 14px;
}

.guaxi-usage-assistant__entry {
  padding: 12px 0;
  border-top: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.18));
}

.guaxi-usage-assistant__entry:first-child {
  border-top: 0;
}

.guaxi-usage-assistant__entry-head {
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 6px;
}

.guaxi-usage-assistant__kind {
  font-size: 11px;
  color: var(--guaxi-notebook-muted, #64748b);
}

.guaxi-usage-assistant__doc-link {
  border: 1px solid var(--guaxi-notebook-border, rgba(148, 163, 184, 0.22));
  border-radius: 7px;
  width: 26px;
  height: 26px;
  display: inline-grid;
  place-items: center;
  background: transparent;
  color: var(--guaxi-notebook-muted, #64748b);
  cursor: pointer;
}

.guaxi-usage-assistant__entry h3 {
  margin: 0 0 6px;
  font-size: 14px;
  line-height: 1.35;
}

.guaxi-usage-assistant__entry p {
  margin: 0;
  color: var(--guaxi-notebook-muted, #64748b);
  font-size: 12px;
  line-height: 1.55;
}

.guaxi-usage-assistant__headings,
.guaxi-usage-assistant__examples,
.guaxi-usage-assistant__entry-actions {
  margin-top: 9px;
  gap: 6px;
  flex-wrap: wrap;
}

.guaxi-usage-assistant__headings span {
  max-width: 100%;
  padding: 2px 7px;
  border-radius: 8px;
  background: var(--guaxi-notebook-panel-soft, rgba(148, 163, 184, 0.09));
  color: var(--guaxi-notebook-muted, #64748b);
  font-size: 11px;
}

.guaxi-usage-assistant__example-btn,
.guaxi-usage-assistant__text-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 28px;
  padding: 4px 8px;
  border-radius: 8px;
  font-size: 12px;
}

.guaxi-usage-assistant__tab:hover,
.guaxi-usage-assistant__icon-btn:hover,
.guaxi-usage-assistant__text-btn:hover,
.guaxi-usage-assistant__example-btn:hover {
  background: var(--guaxi-notebook-panel-soft, rgba(148, 163, 184, 0.12));
}

.guaxi-usage-assistant__state {
  margin: 0 12px 14px;
  padding: 12px;
  border-radius: 8px;
  color: var(--guaxi-notebook-muted, #64748b);
  background: var(--guaxi-notebook-panel-soft, rgba(148, 163, 184, 0.09));
  font-size: 13px;
}

.guaxi-usage-assistant__state.is-warning {
  color: #92400e;
  background: rgba(251, 191, 36, 0.13);
}

.guaxi-usage-assistant__state code {
  display: inline-block;
  margin-top: 8px;
  padding: 3px 6px;
  border-radius: 6px;
  background: rgba(15, 23, 42, 0.08);
  font-family: var(--guaxi-notebook-mono, Consolas, monospace);
}
</style>
