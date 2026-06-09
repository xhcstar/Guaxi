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
            <n-button circle size="small" class="bookmark-toggle" :class="{ 'is-on': isCurrentBookmarked }"
              @click="toggleBookmark">
              <n-icon>
                <BookmarkFilledIcon v-if="isCurrentBookmarked" />
                <BookmarkIcon v-else />
              </n-icon>
            </n-button>
          </template>
          {{ isCurrentBookmarked ? '取消收藏' : '收藏文章' }}
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button
              circle
              size="small"
              :disabled="!currentArticle?.url"
              @click="openInBrowser"
            >
              <n-icon>
                <OpenIcon />
              </n-icon>
            </n-button>
          </template>
          在内置浏览器打开原网页
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button
              circle
              size="small"
              :loading="reparsing"
              :disabled="!currentArticle?.url || reparsing"
              @click="reparseArticle"
            >
              <n-icon>
                <RefreshIcon />
              </n-icon>
            </n-button>
          </template>
          重新深解析（重新抓取完整图文）
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

          <h1 class="article-title">
            {{ currentArticle.title }}
            <n-tooltip placement="top" trigger="hover" v-if="currentArticle.url">
              <template #trigger>
                <n-button text class="title-open-btn" @click="openInBrowser">
                  <n-icon size="18">
                    <OpenIcon />
                  </n-icon>
                </n-button>
              </template>
              用浏览器打开原文
            </n-tooltip>
          </h1>

          <div class="article-enrichment"
            v-if="currentArticle.summary || (currentArticle.tags && currentArticle.tags.length) || currentArticle.relevance_score >= 0">
            <div class="enrich-row" v-if="currentArticle.relevance_score >= 0">
              <span class="relevance-chip" :class="relevanceClass">相关度 {{ currentArticle.relevance_score }}</span>
              <span v-for="tag in (currentArticle.tags || [])" :key="tag" class="tag-chip">{{ tag }}</span>
            </div>
            <p class="article-summary" v-if="currentArticle.summary">{{ currentArticle.summary }}</p>
          </div>

          <div class="article-image" v-if="currentArticle.image_url">
            <img :src="currentArticle.image_url" :alt="currentArticle.title" loading="lazy" />
          </div>
        </div>

        <!-- 登录墙提示：抓取命中登录/验证页时引导用户登录一次 -->
        <n-alert v-if="currentArticle.login_required" type="warning" class="login-wall-alert"
          title="该内容需要登录后才能完整抓取">
          <div class="login-wall-body">
            <span>此站点疑似登录墙或反爬限制。登录一次后会话会被保存，之后可正常抓取同站内容。</span>
            <n-button size="small" type="primary" :loading="loginBusy" @click="handleOpenLogin">
              打开登录
            </n-button>
          </div>
        </n-alert>

        <!-- 正文后台加载提示（点击已即时打开，正文异步 hydrate） -->
        <div v-if="isHydratingDetail" class="hydrate-tip">
          <n-spin size="small" />
          <span>正在加载完整正文…</span>
        </div>

        <!-- 文章内容 -->
        <div class="article-body">
          <SelectionNotesPanel
            ref="selectionNotesRef"
            :article-id="currentArticle.id"
            :article-title="currentArticle.title"
            :source-url="currentArticle.url"
            :category="currentArticle.category"
          >
            <div
              class="article-content"
              v-html="formattedContent"
              @mouseup="selectionNotesRef?.handleSelectionMouseUp?.()"
              @dblclick="handleContentDblClick"
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
import { NSpin, useMessage } from 'naive-ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import SelectionNotesPanel from './SelectionNotesPanel.vue'
import {
  BookmarkOutline,
  Bookmark,
  ShareSocialOutline,
  CloseOutline,
  DocumentTextOutline,
  OpenOutline,
  RefreshOutline
} from '@vicons/ionicons5'

const BookmarkIcon = BookmarkOutline
const BookmarkFilledIcon = Bookmark
const ShareIcon = ShareSocialOutline
const CloseIcon = CloseOutline
const DocumentTextIcon = DocumentTextOutline
const OpenIcon = OpenOutline
const RefreshIcon = RefreshOutline

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
const message = useMessage()
const selectionNotesRef = ref(null)
const loginBusy = ref(false)
const isHydratingDetail = computed(() => store.isHydratingDetail.value)

const handleOpenLogin = async () => {
  const article = props.currentArticle
  if (!article?.url) return
  loginBusy.value = true
  try {
    // profile 用文章 host，与后端 render_profile_for_url 对齐，登录态同站复用。
    let profile = 'default'
    try {
      profile = new URL(article.url).host || 'default'
    } catch {
      profile = 'default'
    }
    await store.openLoginSession(profile, article.url)
    message.info('已打开登录浏览器，请在其中完成登录（扫码 / 账号密码均可），登录后回到这里重新打开该文章即可抓取正文', {
      duration: 6000
    })
  } catch (error) {
    message.error(`打开登录失败：${error?.message || error}`, { duration: 3000 })
  } finally {
    loginBusy.value = false
  }
}

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

const escapeHtml = (value) => String(value || '')
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const linkifyText = (value) => {
  const escaped = escapeHtml(value)
  return escaped.replace(/(https?:\/\/[^\s<]+)/g, (m) => {
    const href = m.replace(/"/g, '&quot;')
    return `<a href="${href}" target="_blank" rel="noopener noreferrer">${m}</a>`
  })
}

const isImageUrl = (url) => /\.(png|jpe?g|gif|webp|svg)(\?.*)?$/i.test(url)
const isMediaUrl = (url) => /(youtube\.com|youtu\.be|bilibili\.com|vimeo\.com|\.mp4(\?|$)|\.webm(\?|$)|\.m3u8(\?|$))/i.test(url)
const toEmbedUrl = (url) => {
  if (/youtube\.com\/watch\?v=([^&]+)/i.test(url)) {
    return `https://www.youtube.com/embed/${url.match(/youtube\.com\/watch\?v=([^&]+)/i)?.[1] || ''}`
  }
  if (/youtu\.be\/([^?&]+)/i.test(url)) {
    return `https://www.youtube.com/embed/${url.match(/youtu\.be\/([^?&]+)/i)?.[1] || ''}`
  }
  if (/bilibili\.com\/video\/(BV[0-9A-Za-z]+)/i.test(url)) {
    return `https://player.bilibili.com/player.html?bvid=${url.match(/bilibili\.com\/video\/(BV[0-9A-Za-z]+)/i)?.[1] || ''}&page=1`
  }
  return url
}

const formattedContent = computed(() => {
  // 工业级抽取产出的干净 HTML（内联图片）优先；回退到旧的 content 纯文本。
  const cleanHtml = String(props.currentArticle?.clean_content_html || '').trim()
  if (cleanHtml) return cleanHtml
  const raw = String(props.currentArticle?.content || '').trim()
  if (!raw) return ''
  if (/<(p|img|h1|h2|h3|ul|ol|li|blockquote|a)(\s|>)/i.test(raw)) {
    return raw
  }
  return raw
    .split('\n')
    .map(line => line.trim())
    .filter(Boolean)
    .map(line => {
      if (/^-\s+https?:\/\//i.test(line)) {
        const url = line.replace(/^-+\s*/, '')
        const escapedUrl = escapeHtml(url)
        if (isMediaUrl(url)) {
          const embed = escapeHtml(toEmbedUrl(url))
          return `<p>• <a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p><div class="media-placeholder"><iframe src="${embed}" loading="lazy" referrerpolicy="no-referrer" allowfullscreen></iframe></div>`
        }
        if (isImageUrl(url)) {
          return `<p>• <a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p><p><img src="${escapedUrl}" loading="lazy" alt="image" /></p>`
        }
        return `<p>• <a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p>`
      }
      if (/^https?:\/\//i.test(line)) {
        const escapedUrl = escapeHtml(line)
        if (isMediaUrl(line)) {
          const embed = escapeHtml(toEmbedUrl(line))
          return `<p><a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p><div class="media-placeholder"><iframe src="${embed}" loading="lazy" referrerpolicy="no-referrer" allowfullscreen></iframe></div>`
        }
        if (isImageUrl(line)) {
          return `<p><a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p><p><img src="${escapedUrl}" loading="lazy" alt="image" /></p>`
        }
        return `<p><a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p>`
      }
      return `<p>${linkifyText(line)}</p>`
    })
    .join('')
})

const toggleBookmark = () => {
  if (props.currentArticle) {
    store.toggleBookmark(props.currentArticle)
  }
}

const relevanceClass = computed(() => {
  const s = Number(props.currentArticle?.relevance_score)
  if (!Number.isFinite(s) || s < 0) return ''
  if (s >= 70) return 'rel-high'
  if (s >= 40) return 'rel-mid'
  return 'rel-low'
})

// 双击正文中的句子/段落：直接把该块文本插入笔记，无需手动选择。
const handleContentDblClick = (event) => {
  // 若用户实际是在拖选文字（有选区），走原有选择逻辑，不抢双击。
  const sel = window.getSelection?.()
  const selectedText = sel ? String(sel.toString() || '').trim() : ''
  if (selectedText) {
    selectionNotesRef.value?.handleSelectionMouseUp?.()
    return
  }

  // 直接双击图片：收集该图片。
  const target = event.target
  if (target && target.tagName === 'IMG' && target.src) {
    selectionNotesRef.value?.insertBlockText?.({
      type: 'image',
      content: target.src,
      originalMarkdown: `![${target.alt || '图片'}](${target.src})`
    })
    return
  }

  // 向上找到最近的块级元素（p/h1-h4/li/blockquote/pre/figure/div）。
  let el = event.target
  const blockTags = new Set(['P', 'H1', 'H2', 'H3', 'H4', 'LI', 'BLOCKQUOTE', 'PRE', 'FIGURE', 'DIV'])
  while (el && el !== event.currentTarget && !blockTags.has(el.tagName)) {
    el = el.parentElement
  }
  if (!el || el === event.currentTarget) return

  const text = String(el.innerText || el.textContent || '').trim()

  // 块内含视频（iframe / video）：收集视频链接。
  const media = el.querySelector?.('iframe[src], video source[src], video[src]')
  const mediaUrl = media?.src || media?.getAttribute?.('src') || ''
  if (mediaUrl) {
    selectionNotesRef.value?.insertBlockText?.({
      type: 'video',
      content: mediaUrl,
      originalMarkdown: `[▶ 视频](${mediaUrl})`
    })
    return
  }

  // 块内主要是图片（文字很少）：收集图片。
  const img = el.querySelector?.('img[src]')
  if (img?.src && text.length < 12) {
    selectionNotesRef.value?.insertBlockText?.({
      type: 'image',
      content: img.src,
      originalMarkdown: `![${img.alt || '图片'}](${img.src})`
    })
    return
  }

  if (text) {
    selectionNotesRef.value?.insertBlockText?.(text)
  }
}

const openInBrowser = async () => {
  const url = String(props.currentArticle?.url || '').trim()
  if (!url) return
  try {
    // 优先在内置浏览器窗口打开真实网页（人类友好浏览）。
    await store.openArticleInBrowser(url)
  } catch (error) {
    // 退回系统浏览器。
    try {
      await openUrl(url)
    } catch (e2) {
      message.error(`打开链接失败：${e2?.message || e2}`, { duration: 2500 })
    }
  }
}

const reparsing = ref(false)
const reparseArticle = async () => {
  if (!props.currentArticle) return
  reparsing.value = true
  try {
    await store.reparseArticle(props.currentArticle)
    message.success('已重新深解析', { duration: 1500 })
  } catch (error) {
    message.error(`深解析失败：${error?.message || error}`, { duration: 2500 })
  } finally {
    reparsing.value = false
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
  justify-content: flex-end;
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

/* 收藏激活态：金色高亮，点击后状态变化清晰可见 */
.reader-actions .n-button.bookmark-toggle.is-on,
.reader-actions .n-button.bookmark-toggle.is-on:hover {
  background: rgba(240, 160, 32, 0.14);
  color: #f0a020;
  border-color: #f0a020;
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

.article-enrichment {
  margin-bottom: 20px;
}

.enrich-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}

.relevance-chip {
  font-size: 12px;
  padding: 2px 10px;
  border-radius: 12px;
  font-weight: 600;
  color: #fff;
  background: var(--guaxi-text-secondary);
}

.relevance-chip.rel-high { background: #18a058; }
.relevance-chip.rel-mid { background: #f0a020; }
.relevance-chip.rel-low { background: #909399; }

.tag-chip {
  font-size: 12px;
  padding: 2px 10px;
  border-radius: 12px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}

.article-summary {
  font-size: 14px;
  line-height: 1.7;
  color: var(--guaxi-text-secondary);
  padding: 12px 16px;
  background: var(--guaxi-panel-bg);
  border-left: 3px solid var(--guaxi-link, #4c8bf5);
  border-radius: 6px;
  margin: 0;
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

.hydrate-tip {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 12px;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #64748b);
}

.login-wall-alert {
  margin: 0 0 16px;
}

.login-wall-body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.article-content {
  max-width: none;
}

.title-open-btn {
  margin-left: 8px;
  vertical-align: middle;
  color: var(--guaxi-text-secondary, #64748b);
}

.title-open-btn:hover {
  color: var(--guaxi-primary, #2563eb);
}

/* 双击插入：句子/段落块悬停时柔和高亮 + 左侧强调条，提示可双击收集 */
.article-content :deep(p),
.article-content :deep(li),
.article-content :deep(h2),
.article-content :deep(h3),
.article-content :deep(blockquote),
.article-content :deep(pre) {
  border-radius: 8px;
  padding: 4px 10px;
  margin-left: -10px;
  margin-right: -10px;
  box-shadow: inset 3px 0 0 transparent;
  transition: background 0.18s ease, box-shadow 0.18s ease;
}

.article-content :deep(p:hover),
.article-content :deep(li:hover),
.article-content :deep(h2:hover),
.article-content :deep(h3:hover),
.article-content :deep(blockquote:hover),
.article-content :deep(pre:hover) {
  background: rgba(37, 99, 235, 0.05);
  box-shadow: inset 3px 0 0 rgba(37, 99, 235, 0.55);
}

/* 选中文字的高亮色，统一品牌色 */
.article-content :deep(::selection) {
  background: rgba(37, 99, 235, 0.22);
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

.article-content :deep(img) {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 12px auto 20px;
  border-radius: 8px;
}

.article-content :deep(blockquote) {
  margin: 1em 0;
  padding: 0.5em 1em;
  border-left: 3px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}

.article-content :deep(pre) {
  background: var(--guaxi-panel-bg);
  padding: 12px;
  border-radius: 8px;
  overflow-x: auto;
}

.article-content :deep(table) {
  width: 100%;
  border-collapse: collapse;
  margin: 1em 0;
}

.article-content :deep(td),
.article-content :deep(th) {
  border: 1px solid var(--guaxi-border);
  padding: 6px 10px;
}

.article-content :deep(.media-placeholder) {
  width: 100%;
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  overflow: hidden;
  margin: 8px 0 20px;
  background: var(--guaxi-panel-bg);
}

.article-content :deep(.media-placeholder iframe) {
  width: 100%;
  aspect-ratio: 16 / 9;
  border: 0;
  display: block;
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
