<template>
  <div :class="['article-card', viewMode]">
    <div class="card-image" v-if="article.image_url && viewMode !== 'list'">

      <div class="image-overlay">
        <div class="category-badge">{{ article.category }}</div>
        <button class="bookmark-btn" :class="{ bookmarked: isBookmarked }" @click.stop="toggleBookmark">
          <n-icon size="16">
            <BookmarkIcon />
          </n-icon>
        </button>
      </div>
    </div>

    <div class="card-content">
      <div class="card-header">
        <div class="category-tag" v-if="viewMode === 'list'">
          {{ article.category }}
        </div>
        <div class="meta-info">
          <span class="source">{{ article.source }}</span>
          <span class="time">{{ formatTime(article.published_at) }}</span>
        </div>
      </div>

      <h3 class="card-title">{{ article.title }}</h3>

      <p class="card-summary" v-if="viewMode !== 'grid'">
        {{ article.description || article.summary }}
      </p>

      <div class="card-footer">
        <div class="actions">
          <button class="action-btn" @click.stop="toggleBookmark">
            <n-icon size="14" :class="{ bookmarked: isBookmarked }">
              <BookmarkIcon />
            </n-icon>
            {{ isBookmarked ? '已收藏' : '收藏' }}
          </button>
          <button class="action-btn">
            <n-icon size="14">
              <ShareIcon />
            </n-icon>
            分享
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import { BookmarkOutline, ShareSocialOutline } from '@vicons/ionicons5'

const BookmarkIcon = BookmarkOutline
const ShareIcon = ShareSocialOutline

const props = defineProps({
  article: {
    type: Object,
    required: true
  },
  viewMode: {
    type: String,
    default: 'magazine'
  }
})

const store = useFlipboardStore()

const isBookmarked = computed(() => {
  return store.isBookmarked(props.article.id)
})

const toggleBookmark = () => {
  store.toggleBookmark(props.article)
}

const formatTime = (timeString) => {
  if (!timeString) return ''
  const date = new Date(timeString)
  const now = new Date()
  const diff = now - date

  const minutes = Math.floor(diff / 60000)
  const hours = Math.floor(diff / 3600000)
  const days = Math.floor(diff / 86400000)

  if (minutes < 60) return `${minutes}分钟前`
  if (hours < 24) return `${hours}小时前`
  if (days < 7) return `${days}天前`

  return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}
</script>

<style scoped>
.article-card {
  background: white;
  border-radius: 16px;
  overflow: hidden;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.08);
  transition: all 0.3s ease;
  cursor: pointer;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.article-card:hover {
  transform: translateY(-4px);
  box-shadow: 0 8px 40px rgba(0, 0, 0, 0.15);
}

.article-card.list {
  flex-direction: row;
  align-items: center;
  padding: 16px;
  gap: 16px;
}

.card-image {
  position: relative;
  height: 200px;
  overflow: hidden;
}

.article-card.list .card-image {
  width: 120px;
  height: 80px;
  flex-shrink: 0;
  border-radius: 8px;
}

.card-image img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.3s ease;
}

.article-card:hover .card-image img {
  transform: scale(1.05);
}

.image-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(to bottom, rgba(0, 0, 0, 0.1), rgba(0, 0, 0, 0.3));
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 12px;
}

.category-badge {
  background: var(--guaxi-panel-bg);
  padding: 4px 12px;
  border-radius: 12px;
  font-size: 12px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.bookmark-btn {
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 50%;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s ease;
  color: var(--guaxi-text-secondary);
}

.bookmark-btn:hover {
  background: var(--guaxi-content-bg);
  color: #ff6b6b;
}

.bookmark-btn.bookmarked {
  color: #ff6b6b;
}

.card-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
}

.article-card.list .card-content {
  padding: 0;
  flex: 1;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.category-tag {
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
}

.meta-info {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.card-title {
  font-size: 18px;
  font-weight: 700;
  line-height: 1.4;
  color: var(--guaxi-text-primary);
  margin-bottom: 8px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.article-card.list .card-title {
  font-size: 16px;
  -webkit-line-clamp: 1;
}

.card-summary {
  color: var(--guaxi-text-secondary);
  line-height: 1.6;
  font-size: 14px;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  flex: 1;
}

.article-card.list .card-summary {
  -webkit-line-clamp: 2;
  margin-bottom: 8px;
}

.card-footer {
  margin-top: auto;
}

.actions {
  display: flex;
  gap: 16px;
}

.action-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  background: none;
  border: none;
  color: #666;
  font-size: 12px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
  transition: all 0.2s ease;
}

.action-btn:hover {
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
}

.action-btn .bookmarked {
  color: #ff6b6b;
}
</style>