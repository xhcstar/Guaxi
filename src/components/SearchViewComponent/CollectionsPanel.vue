<template>
  <div class="collections-panel" :class="{ 'collections-panel-visible': visible }">
    <div class="collections-header">
      <h3>{{ activeTab === 'favorites' ? '收藏夹' : '我的收集' }}</h3>
      <n-button text size="small" @click="$emit('close')">关闭</n-button>
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
            <n-button
              text
              size="small"
              :title="entry.favorite ? '取消收藏' : '加入收藏夹'"
              @click="toggleFavorite(entry)"
            >
              <span :class="['fav-star', { 'fav-on': entry.favorite }]">{{ entry.favorite ? '★' : '☆' }}</span>
            </n-button>
          </div>
        </template>

        <div class="collection-content">{{ truncate(entry.content) }}</div>

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
              <n-button size="tiny" tertiary @click="copyMarkdown(entry)">复制 MD</n-button>
              <n-button size="tiny" type="primary" @click="insertToEditor(entry)">插入文档</n-button>
              <n-button size="tiny" type="error" tertiary @click="remove(entry)">删除</n-button>
            </div>
          </div>
        </template>
      </n-card>
    </n-scrollbar>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { NInput, NIcon, NScrollbar, NCard, NButton, NText, NEmpty, useMessage } from 'naive-ui'
import { SearchOutline as SearchIcon, LinkOutline as LinkIcon } from '@vicons/ionicons5'
import { useFlipboardCollectionsStore } from '../../stores/flipboardCollectionsStore.js'
import { useEditorStore } from '../../stores/editorStore'
import { createLogger } from '../../utils/logger'

const logger = createLogger('CollectionsPanel')

defineProps({
  visible: {
    type: Boolean,
    default: false
  }
})
defineEmits(['close'])

const collections = useFlipboardCollectionsStore()
const editorStore = useEditorStore()
const message = useMessage()

const activeTab = ref('all') // 'all' | 'favorites'
const entries = computed(() =>
  activeTab.value === 'favorites'
    ? collections.favoriteEntries.value
    : collections.filteredEntries.value
)
const favoriteCount = computed(() => collections.favoriteEntries.value.length)

const toggleFavorite = async (entry) => {
  const on = await collections.toggleFavorite(entry.id)
  message.success(on ? '已加入收藏夹' : '已移出收藏夹', { duration: 1000 })
}

const truncate = (text) => {
  const s = String(text || '')
  return s.length > 220 ? s.slice(0, 220) + '…' : s
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
  await collections.removeEntry(entry.id)
  message.success('已删除', { duration: 1000 })
}

onMounted(() => {
  collections.load()
})
</script>

<style scoped>
.collections-panel {
  position: fixed;
  top: 64px;
  right: -380px;
  width: 340px;
  height: calc(100% - 64px);
  background: var(--guaxi-panel-bg, #fff);
  border-left: 1px solid var(--guaxi-border, #e5e7eb);
  box-shadow: -4px 0 24px rgba(15, 23, 42, 0.12);
  display: flex;
  flex-direction: column;
  transition: right 0.25s ease;
  z-index: 9997;
}

.collections-panel-visible {
  right: 0;
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
  font-size: 14px;
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
}

.collection-cat {
  font-size: 11px;
  padding: 1px 8px;
  border-radius: 8px;
  background: var(--guaxi-header-bg, #eef2ff);
  color: var(--guaxi-text-secondary, #64748b);
}

.collection-content {
  font-size: 13px;
  line-height: 1.6;
  color: var(--guaxi-text-primary, #0f172a);
  white-space: pre-wrap;
}

.collection-source {
  margin-top: 8px;
  font-size: 12px;
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
  font-size: 11px;
}

.collection-btns {
  display: flex;
  gap: 6px;
}
</style>
