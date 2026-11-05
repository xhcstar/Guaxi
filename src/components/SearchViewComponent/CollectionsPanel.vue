<template>
  <div class="collections-panel" :class="{ 'collections-panel-visible': visible }"
    :style="{ width: panelWidth + 'px' }">
    <!-- 左侧拖拽把手：拖动改变面板宽度 -->
    <div class="panel-resize-handle" @mousedown="startResize"></div>
    <div class="collections-header">
      <h3>{{ activeTab === 'favorites' ? '收藏夹' : '我的收集' }}</h3>
      <n-tooltip trigger="hover" :z-index="10050">
        <template #trigger>
          <n-button circle size="small" tertiary @click="$emit('close')">
            <n-icon :component="CloseIcon" />
          </n-button>
        </template>
        关闭面板
      </n-tooltip>
    </div>

    <div class="collections-tabs">
      <n-button
        size="tiny"
        :type="activeTab === 'all' ? 'primary' : 'default'"
        :tertiary="activeTab !== 'all'"
        @click="activeTab = 'all'"
      >全部笔记</n-button>
      <n-button
        size="tiny"
        :type="activeTab === 'favorites' ? 'primary' : 'default'"
        :tertiary="activeTab !== 'favorites'"
        @click="activeTab = 'favorites'"
      >收藏夹 ({{ favoriteCount }})</n-button>
    </div>

    <div class="collections-search">
      <n-input
        v-model:value="collections.searchQuery.value"
        round
        clearable
        size="small"
        placeholder="搜索标题 / 正文 / 来源 / 分类..."
      >
        <template #prefix>
          <n-icon :component="SearchIcon" />
        </template>
      </n-input>
    </div>

    <n-scrollbar class="collections-body">
      <div v-if="!entries.length" class="collections-empty">
        <n-empty size="small" :description="activeTab === 'favorites' ? '收藏夹还是空的' : '还没有收集任何笔记'">
          <template #extra>
            <span class="empty-tip">{{ activeTab === 'favorites' ? '在笔记卡片上点 ☆ 收藏，长久保留在这里' : '在文章里选中文字 → “+ 笔记”，即可收集到这里' }}</span>
          </template>
        </n-empty>
      </div>

      <n-card
        v-for="entry in entries"
        :key="entry.id"
        size="small"
        class="collection-card"
        :title="entry.title"
        :bordered="true"
      >
        <template #header-extra>
          <div class="collection-header-extra">
            <span class="collection-cat" v-if="entry.category">{{ entry.category }}</span>
            <n-tooltip trigger="hover" :z-index="10050">
              <template #trigger>
                <n-button text size="small" @click="toggleFavorite(entry)">
                  <span :class="['fav-star', { 'fav-on': entry.favorite }]">{{ entry.favorite ? '★' : '☆' }}</span>
                </n-button>
              </template>
              {{ entry.favorite ? '取消收藏' : '加入收藏夹' }}
            </n-tooltip>
          </div>
        </template>

        <div class="collection-content" @click="openEntry(entry)" title="点击在正文区查看原文">
          <img v-if="entryImage(entry)" :src="entryImage(entry)" class="collection-media-img" loading="lazy" />
          <div v-else-if="entryVideo(entry)" class="collection-media-video">
            <n-icon :component="PlayIcon" size="16" /> 视频内容（点击查看原文）
          </div>
          <span v-else>{{ truncate(entry.content) }}</span>
        </div>

        <div class="collection-source" v-if="entry.sourceTitle || entry.sourceUrl">
          <n-icon :component="LinkIcon" size="13" />
          <a v-if="entry.sourceUrl" :href="entry.sourceUrl" target="_blank" rel="noopener noreferrer">
            {{ entry.sourceTitle || entry.sourceUrl }}
          </a>
          <span v-else>{{ entry.sourceTitle }}</span>
        </div>

        <template #footer>
          <div class="collection-actions">
            <n-text depth="3" class="collection-time">{{ formatTime(entry.timestamp) }}</n-text>
            <div class="collection-btns">
              <n-tooltip trigger="hover" :z-index="10050">
                <template #trigger>
                  <n-button size="small" circle tertiary @click="copyMarkdown(entry)">
                    <n-icon :component="CopyIcon" />
                  </n-button>
                </template>
                复制 Markdown
              </n-tooltip>
              <n-tooltip trigger="hover" :z-index="10050">
                <template #trigger>
                  <n-button size="small" circle type="primary" @click="insertToEditor(entry)">
                    <n-icon :component="InsertIcon" />
                  </n-button>
                </template>
                插入主页文档
              </n-tooltip>
              <n-tooltip trigger="hover" :z-index="10050">
                <template #trigger>
                  <n-button size="small" circle type="error" tertiary @click="remove(entry)">
                    <n-icon :component="TrashIcon" />
                  </n-button>
                </template>
                删除
              </n-tooltip>
            </div>
          </div>
        </template>
      </n-card>
    </n-scrollbar>
  </div>
</template>

<script setup>
import { computed, onMounted, onBeforeUnmount, ref } from 'vue'
import { NInput, NIcon, NScrollbar, NCard, NButton, NText, NEmpty, NTooltip, useMessage } from 'naive-ui'
import {
  SearchOutline as SearchIcon,
  LinkOutline as LinkIcon,
  CopyOutline as CopyIcon,
  AddOutline as InsertIcon,
  TrashOutline as TrashIcon,
  CloseOutline as CloseIcon,
  PlayCircleOutline as PlayIcon
} from '@vicons/ionicons5'
import { useFlipboardCollectionsStore } from '../../stores/flipboardCollectionsStore.js'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import { useEditorStore } from '../../stores/editorStore'
import { createLogger } from '../../utils/logger'
import { proxiedImageUrl } from '../../utils/imageProxy.js'

const logger = createLogger('CollectionsPanel')

defineProps({
  visible: {
    type: Boolean,
    default: false
  }
})
defineEmits(['close'])

const collections = useFlipboardCollectionsStore()
const flipboard = useFlipboardStore()
const editorStore = useEditorStore()
const message = useMessage()

// 面板宽度（可拖拽，持久化）
const panelWidth = ref(Number(localStorage.getItem('flipboard_collections_width')) || 340)
let resizing = false
let resizeStartX = 0
let resizeStartWidth = 0

const onResizeMove = (e) => {
  if (!resizing) return
  // 把手在左侧：向左拖动增大宽度。
  const delta = resizeStartX - e.clientX
  panelWidth.value = Math.max(280, Math.min(720, resizeStartWidth + delta))
}

const stopResize = () => {
  if (!resizing) return
  resizing = false
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
  localStorage.setItem('flipboard_collections_width', String(panelWidth.value))
}

const startResize = (e) => {
  resizing = true
  resizeStartX = e.clientX
  resizeStartWidth = panelWidth.value
  document.addEventListener('mousemove', onResizeMove)
  document.addEventListener('mouseup', stopResize)
  e.preventDefault()
}

const activeTab = ref('all') // 'all' | 'favorites'
const entries = computed(() =>
  activeTab.value === 'favorites'
    ? collections.favoriteEntries.value
    : collections.noteEntries.value
)
const favoriteCount = computed(() => collections.favoriteEntries.value.length)

const toggleFavorite = async (entry) => {
  // 卡片收藏（整篇文章）：点星即取消收藏，与卡片 icon 状态联动。
  if (entry.type === 'bookmark') {
    flipboard.removeBookmark(entry.articleId)
    message.success('已取消收藏', { duration: 1000 })
    return
  }
  const on = await collections.toggleFavorite(entry.id)
  message.success(on ? '已加入收藏夹' : '已移出收藏夹', { duration: 1000 })
}

const truncate = (text) => {
  const s = String(text || '')
  return s.length > 220 ? s.slice(0, 220) + '…' : s
}

// 从笔记内容里识别图片 / 视频 markdown，用于在收集面板里直接预览复杂格式。
const IMG_RE = /!\[[^\]]*\]\(([^)]+)\)/
const VIDEO_RE = /\[(?:▶\s*)?视频\]\(([^)]+)\)/
const entryImage = (e) => {
  const m = IMG_RE.exec(String(e?.content || ''))
  if (!m) return ''
  return proxiedImageUrl(m[1])
}
const entryVideo = (e) => {
  const m = VIDEO_RE.exec(String(e?.content || ''))
  return m ? m[1] : ''
}

// 点击条目：在正文阅读区重新打开原文（类似浏览器收藏夹）。
const openEntry = (entry) => {
  let articleRef = null
  if (entry.type === 'bookmark') {
    const list = Array.isArray(flipboard.bookmarks.value) ? flipboard.bookmarks.value : []
    const full = list.find(b => String(b?.id) === String(entry.articleId))
    articleRef = full || { id: entry.articleId, url: entry.sourceUrl, title: entry.sourceTitle }
  } else if (entry.articleId || entry.sourceUrl) {
    articleRef = { id: entry.articleId, url: entry.sourceUrl, title: entry.sourceTitle }
  }
  if (!articleRef || (!articleRef.id && !articleRef.url)) {
    message.warning('该条目没有可打开的来源', { duration: 1500 })
    return
  }
  flipboard.openReaderForArticle(articleRef)
  flipboard.collectionsPanelVisible.value = false
}

const formatTime = (t) => {
  try {
    return new Date(t).toLocaleString()
  } catch {
    return ''
  }
}

const copyMarkdown = async (entry) => {
  const md = collections.buildMarkdown(entry)
  try {
    await navigator.clipboard.writeText(md)
    message.success('已复制 Markdown', { duration: 1200 })
  } catch (e) {
    logger.error('copy markdown failed', e)
    message.error('复制失败', { duration: 1500 })
  }
}

const insertToEditor = async (entry) => {
  const md = collections.buildMarkdown(entry)
  if (!editorStore.editorInstance?.getModel()) {
    // 编辑器未挂载（不在主页编辑视图）：退回复制，提示用户。
    await copyMarkdown(entry)
    message.info('编辑器未打开，已复制到剪贴板，可在主页粘贴', { duration: 2500 })
    return
  }
  const ok = await editorStore.insertContent(`\n${md}\n`)
  if (ok) {
    message.success('已插入文档', { duration: 1200 })
  } else {
    await copyMarkdown(entry)
    message.info('插入失败，已复制到剪贴板', { duration: 2500 })
  }
}

const remove = async (entry) => {
  if (entry.type === 'bookmark') {
    // 整篇收藏：删除即取消卡片收藏（联动 localStorage + 收集库）。
    flipboard.removeBookmark(entry.articleId)
  } else {
    await collections.removeEntry(entry.id)
  }
  message.success('已删除', { duration: 1000 })
}

let unlistenExternal = null
onMounted(async () => {
  collections.load()
  // 监听"浏览器打开原文"窗口双击采集的内容，写入收集库。
  try {
    const { listen } = await import('@tauri-apps/api/event')
    unlistenExternal = await listen('flipboard-external-note', async (e) => {
      await collections.addCapturedNote(e?.payload || {})
      message.success('已从网页采集到「我的收集」', { duration: 1500 })
    })
  } catch (err) {
    logger.warn('listen flipboard-external-note failed', err)
  }
})

onBeforeUnmount(() => {
  if (unlistenExternal) { try { unlistenExternal() } catch {} }
  document.removeEventListener('mousemove', onResizeMove)
  document.removeEventListener('mouseup', stopResize)
})
</script>

<style scoped>
.collections-panel {
  position: fixed;
  top: 64px;
  right: 0;
  width: 340px;
  height: calc(100% - 64px);
  background: var(--guaxi-panel-bg, #fff);
  border-left: 1px solid var(--guaxi-border, #e5e7eb);
  box-shadow: -4px 0 24px rgba(15, 23, 42, 0.12);
  display: flex;
  flex-direction: column;
  transform: translateX(100%);
  transition: transform 0.25s ease;
  z-index: 9997;
}

.collections-panel-visible {
  transform: translateX(0);
}

.panel-resize-handle {
  position: absolute;
  top: 0;
  left: 0;
  width: 5px;
  height: 100%;
  cursor: ew-resize;
  z-index: 2;
  background: transparent;
  transition: background 0.15s ease;
}

.panel-resize-handle:hover {
  background: var(--guaxi-primary, #2563eb);
  opacity: 0.4;
}

.collections-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--guaxi-border, #e5e7eb);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.collections-header h3 {
  margin: 0;
  font-size: 19px;
  font-weight: 700;
}

/* 卡片标题（naive n-card 头部）：作为小标题，明显大于正文。 */
.collection-card :deep(.n-card-header__main) {
  font-size: 16px;
  font-weight: 600;
  line-height: 1.4;
}

.collections-search {
  padding: 10px 12px;
  border-bottom: 1px solid var(--guaxi-border, #e5e7eb);
}

.collections-tabs {
  display: flex;
  gap: 8px;
  padding: 10px 12px 0;
}

.collection-header-extra {
  display: flex;
  align-items: center;
  gap: 8px;
}

.fav-star {
  font-size: 16px;
  color: var(--guaxi-text-secondary, #94a3b8);
  line-height: 1;
}

.fav-star.fav-on {
  color: #f0a020;
}

.collections-body {
  flex: 1;
  padding: 12px;
}

.collections-empty {
  padding: 32px 8px;
}

.empty-tip {
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}

.collection-card {
  margin-bottom: 12px;
  border-radius: 10px;
  transition: box-shadow 0.18s ease, transform 0.18s ease;
}

.collection-card:hover {
  box-shadow: 0 6px 18px rgba(15, 23, 42, 0.1);
  transform: translateY(-1px);
}

.collection-cat {
  font-size: 12px;
  padding: 2px 9px;
  border-radius: 8px;
  background: var(--guaxi-header-bg, #eef2ff);
  color: var(--guaxi-text-secondary, #64748b);
}

.collection-content {
  /* 正文：放大到 15px，与字体族对齐中间正文，层次上小于卡片标题。 */
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  font-size: 15px;
  line-height: 1.7;
  color: var(--guaxi-text-primary, #0f172a);
  white-space: pre-wrap;
  cursor: pointer;
}

.collection-content:hover {
  color: var(--guaxi-primary, #2563eb);
}

.collection-media-img {
  max-width: 100%;
  max-height: 160px;
  border-radius: 8px;
  object-fit: cover;
  display: block;
}

.collection-media-video {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--guaxi-text-secondary, #64748b);
  padding: 8px 10px;
  border: 1px dashed var(--guaxi-border, #e5e7eb);
  border-radius: 8px;
}

.collection-source {
  margin-top: 8px;
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--guaxi-text-secondary, #64748b);
  overflow: hidden;
}

.collection-source a {
  color: var(--guaxi-primary, #2563eb);
  text-decoration: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.collection-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
}

.collection-time {
  font-size: 12px;
}

.empty-tip {
  font-size: 13px;
}

/* 视频占位与媒体说明也同步放大一点。 */
.collection-media-video {
  font-size: 14px;
}

.collection-btns {
  display: flex;
  gap: 6px;
}
</style>
