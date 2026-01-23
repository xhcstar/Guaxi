<template>
  <div class="reader-panel">
    <div class="reader-header">
      <div class="reader-actions">
        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button circle size="small" @click="closeReader" class="close-btn">
              <n-icon>
                <CloseIcon />
              </n-icon>
            </n-button>
          </template>
          关闭阅读
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button circle size="small" @click="toggleBookmark" :type="isCurrentBookmarked ? 'primary' : 'default'">
              <n-icon>
                <BookmarkIcon />
              </n-icon>
            </n-button>
          </template>
          收藏/取消收藏
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button
              circle
              size="small"
              :disabled="!currentArticle"
              @click="selectionNotesRef?.toggleNotesPanel?.()"
            >
              <n-icon>
                <DocumentTextIcon />
              </n-icon>
            </n-button>
          </template>
          笔记
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button circle size="small">
              <n-icon>
                <ShareIcon />
              </n-icon>
            </n-button>
          </template>
          分享
        </n-tooltip>
      </div>
    </div>

    <n-scrollbar class="reader-content">
      <div class="article-reader" v-if="currentArticle">
        <!-- 文章头部 -->
        <div class="article-header">
          <div class="article-meta">
            <span class="category-badge">{{ currentArticle.category }}</span>
            <div class="meta-info">
              <span class="source">{{ currentArticle.source }}</span>
              <span class="time">{{ formatTime(currentArticle.published_at) }}</span>
            </div>
          </div>

          <h1 class="article-title">{{ currentArticle.title }}</h1>

          <div class="article-image" v-if="currentArticle.image_url">

          </div>
        </div>

        <!-- 文章内容 -->
        <div class="article-body">
          <SelectionNotesPanel
            ref="selectionNotesRef"
            :article-id="currentArticle.id"
            :article-title="currentArticle.title"
          >
            <div
              class="article-content"
              v-html="formattedContent"
              @mouseup="selectionNotesRef?.handleSelectionMouseUp?.()"
              @dblclick="selectionNotesRef?.toggleNotesPanel?.()"
            ></div>
          </SelectionNotesPanel>
        </div>

        <!-- 相关文章 -->
        <div class="related-articles" v-if="relatedArticles.length > 0">
          <h3 class="related-title">相关文章</h3>
          <div class="related-list">
            <div v-for="article in relatedArticles" :key="article.id" class="related-card"
              @click="selectRelatedArticle(article)">
              <h4>{{ article.title }}</h4>
              <p>{{ article.description }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-else class="reader-empty">
        <n-empty size="large" description="选择一篇文章开始阅读">
          <template #icon>
            <n-icon size="64">
              <DocumentTextIcon />
            </n-icon>
          </template>
          <template #extra>
            <p class="empty-tip">点击左侧的文章卡片查看详情</p>
          </template>
        </n-empty>
      </div>
    </n-scrollbar>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import SelectionNotesPanel from './SelectionNotesPanel.vue'
import {
  BookmarkOutline,
  ShareSocialOutline,
  CloseOutline,
  DocumentTextOutline
} from '@vicons/ionicons5'

const BookmarkIcon = BookmarkOutline
const ShareIcon = ShareSocialOutline
const CloseIcon = CloseOutline
const DocumentTextIcon = DocumentTextOutline

const props = defineProps({
  currentArticle: {
    type: Object,
    default: null
  },
  isLoading: {
    type: Boolean,
    default: false
  }
})

const emit = defineEmits(['close', 'article-select'])

const store = useFlipboardStore()
const selectionNotesRef = ref(null)

const isCurrentBookmarked = computed(() => {
  return props.currentArticle ? store.isBookmarked(props.currentArticle.id) : false
})

const relatedArticles = computed(() => {
  if (!props.currentArticle) return []
  return store.articles.value
    .filter(article =>
      article.id !== props.currentArticle.id &&
      article.category === props.currentArticle.category
    )
    .slice(0, 3)
})

const formattedContent = computed(() => {
  if (!props.currentArticle?.content) return ''
  // 简单的格式化，将换行转换为段落
  return props.currentArticle.content
    .split('\n')
    .filter(p => p.trim())
    .map(p => `<p>${p}</p>`)
    .join('')
})

const toggleBookmark = () => {
  if (props.currentArticle) {
    store.toggleBookmark(props.currentArticle)
  }
}

const closeReader = () => {
  emit('close')
}

const selectRelatedArticle = (article) => {
  emit('article-select', article)
}

const formatTime = (timeString) => {
  if (!timeString) return ''
  const date = new Date(timeString)
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  })
}
</script>

<style scoped>
.reader-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-content-bg);
}

.reader-header {
  padding: 16px 24px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.reader-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.reader-actions .n-button {
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-secondary);
  border: 1px solid var(--guaxi-border);
  transition: all 0.2s ease;
}

.reader-actions .n-button:hover {
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
}

.close-btn {
  color: var(--guaxi-text-secondary);
}

.reader-content {
  flex: 1;
  padding: 0;
}

.article-reader {
  max-width: 800px;
  margin: 0 auto;
  padding: 40px 24px;
}

.article-header {
  margin-bottom: 40px;
}

.article-meta {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 20px;
}

.category-badge {
  background: linear-gradient(135deg, #667eea, #764ba2);
  color: white;
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
}

.meta-info {
  display: flex;
  gap: 16px;
  color: var(--guaxi-text-secondary);
  font-size: 14px;
}

.article-title {
  font-size: 2.5rem;
  font-weight: 700;
  line-height: 1.2;
  color: var(--guaxi-text-primary);
  margin-bottom: 24px;
}

.article-image {
  margin: 32px 0;
}

.article-image img {
  width: 100%;
  max-height: 400px;
  object-fit: cover;
  border-radius: 12px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
}

.article-body {
  line-height: 1.8;
  font-size: 1.125rem;
  color: var(--guaxi-text-primary);
}

.article-content {
  max-width: none;
}

.article-content :deep(p) {
  margin-bottom: 1.5em;
  text-align: justify;
  color: var(--guaxi-text-primary);
}

.article-content :deep(h2) {
  font-size: 1.5rem;
  font-weight: 600;
  margin: 2em 0 1em;
  color: var(--guaxi-text-primary);
}

.article-content :deep(h3) {
  font-size: 1.25rem;
  font-weight: 600;
  margin: 1.5em 0 0.5em;
  color: var(--guaxi-text-primary);
}

.related-articles {
  margin-top: 60px;
  padding-top: 40px;
  border-top: 1px solid var(--guaxi-border);
}

.related-title {
  font-size: 1.25rem;
  font-weight: 600;
  margin-bottom: 24px;
  color: var(--guaxi-text-primary);
}

.related-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.related-card {
  padding: 16px;
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
  cursor: pointer;
  transition: all 0.2s ease;
  border: 1px solid var(--guaxi-border);
}

.related-card:hover {
  background: var(--guaxi-header-bg);
  transform: translateX(4px);
}

.related-card h4 {
  font-size: 1rem;
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--guaxi-text-primary);
}

.related-card p {
  font-size: 0.875rem;
  color: var(--guaxi-text-secondary);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.reader-empty {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  padding: 40px;
}

.empty-tip {
  color: var(--guaxi-text-secondary);
  font-size: 14px;
  margin-top: 8px;
}
</style>