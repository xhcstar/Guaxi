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
          重新抓取并解析（重新请求原站）
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button
              circle
              size="small"
              :loading="replaying"
              :disabled="!currentArticle?.url || replaying"
              @click="replayExtraction"
            >
              <n-icon>
                <ReloadIcon />
              </n-icon>
            </n-button>
          </template>
          从本地原文重放抽取（不重新请求站点）
        </n-tooltip>

        <n-tooltip placement="bottom" trigger="hover">
          <template #trigger>
            <n-button circle size="small" :type="bilingual.enabled.value ? 'primary' : 'default'"
              :loading="bilingual.busy.value" @click="toggleBilingual">
              <n-icon>
                <TranslateIcon />
              </n-icon>
            </n-button>
          </template>
          {{ bilingual.enabled.value ? '关闭全文翻译' : '全文翻译（双语对照）' }}
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
              @mouseup="onContentMouseUp"
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
              <p>{{ plainPreview(article.description) }}</p>
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

    <!-- 划词翻译面板：与主界面编辑器同款富面板（多引擎/词典/语言切换/固定/拖拽）。 -->
    <GuaxiSelectionTranslatePanel
      v-if="selTranslate.visible.value"
      :pos="selTranslate.pos.value"
      :raw="selTranslate.raw.value"
      :result="selTranslate.result.value"
      :error="selTranslate.error.value"
      :busy="selTranslate.busy.value"
      :pinned="selTranslate.pinned.value"
      :items="selTranslate.items.value"
      :word-meta="selTranslate.wordMeta.value"
      :from-lang="selTranslate.fromLang.value"
      :to-lang="selTranslate.toLang.value"
      :lang-options="selTranslate.langOptions"
      @update:from-lang="(v) => (selTranslate.fromLang.value = v)"
      @update:to-lang="(v) => (selTranslate.toLang.value = v)"
      @update:pos="(p) => (selTranslate.pos.value = p)"
      @translate="() => selTranslate.translate()"
      @copy="selTranslate.copyResult"
      @swap-langs="selTranslate.swapLangs"
      @toggle-pin="selTranslate.togglePin"
      @close="selTranslate.close"
    />
  </div>
</template>

<script setup>
import { computed, ref, watch, nextTick } from 'vue'
import { NSpin, useMessage } from 'naive-ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { createLogger } from '../../utils/logger'
import { useFlipboardStore } from '../../stores/flipboardStore.js'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker.js'
import SelectionNotesPanel from './SelectionNotesPanel.vue'

const logger = createLogger('FlipboardDetailView')
import {
  BookmarkOutline,
  Bookmark,
  ShareSocialOutline,
  CloseOutline,
  DocumentTextOutline,
  OpenOutline,
  RefreshOutline,
  SyncOutline,
  LanguageOutline
} from '@vicons/ionicons5'
import { useBilingualPreview } from '../../composables/useBilingualPreview.js'
import { useSelectionTranslate } from '../../composables/useSelectionTranslate.js'
import GuaxiSelectionTranslatePanel from '../HomeViewComponent/GuaxiSelectionTranslatePanel.vue'

const BookmarkIcon = BookmarkOutline
const BookmarkFilledIcon = Bookmark
const ShareIcon = ShareSocialOutline
const CloseIcon = CloseOutline
const DocumentTextIcon = DocumentTextOutline
const OpenIcon = OpenOutline
const RefreshIcon = RefreshOutline
const ReloadIcon = SyncOutline
const TranslateIcon = LanguageOutline

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

// 估算一段 HTML 去标签后的正文文字量（用于判断 clean_content_html 是不是"只有图/没正文"）。
const htmlTextLen = (html) => String(html || '')
  .replace(/<[^>]*>/g, ' ')
  .replace(/\s+/g, '')
  .trim()
  .length

// 相关文章卡片的摘要：全文 RSS 的 description 常带 HTML 标签，去标签压成纯文本预览。
const plainPreview = (html) => String(html || '')
  .replace(/<[^>]*>/g, ' ')
  .replace(/\s+/g, ' ')
  .trim()
  .slice(0, 120)

// 是否看起来像 Markdown 源码（需要走 Markdown 渲染引擎）。
const looksLikeMarkdown = (text) => {
  const s = String(text || '')
  return /(^|\n)#{1,6}\s/.test(s)          // 标题
    || /!\[[^\]]*\]\([^)]+\)/.test(s)       // 图片
    || /(^|\n)[-*+]\s+\S/.test(s)           // 无序列表
    || /(^|\n)\d+\.\s+\S/.test(s)           // 有序列表
    || /(^|\n)>\s/.test(s)                  // 引用
    || /```/.test(s)                        // 代码块
    || /\[[^\]]+\]\([^)]+\)/.test(s)        // 链接
}

// 选出用于渲染的原始内容。
//
// 数据侧契约（Phase 3 之后）：`clean_content_markdown` 是正文真源，
// 由抽取器产出并经过块级清洗；`clean_content_html` 是由它渲染出来的 HTML。
// 所以这里**优先 Markdown**——它是唯一经过清洗的形态。
//
// 保留 HTML / 纯文本两条退路，仅为兼容旧缓存行（Phase 3 之前入库的数据
// 可能只有 HTML 或纯 content）。新数据不会再走到它们。
const contentSource = computed(() => {
  const md = String(props.currentArticle?.clean_content_markdown || '').trim()
  if (md) return md
  const cleanHtml = String(props.currentArticle?.clean_content_html || '').trim()
  const raw = String(props.currentArticle?.content || '').trim()
  if (cleanHtml && !(htmlTextLen(cleanHtml) < 40 && htmlTextLen(raw) > htmlTextLen(cleanHtml))) {
    return cleanHtml
  }
  return raw || cleanHtml || ''
})

// 最终渲染到正文区的 HTML。Markdown 源码交给项目统一的渲染引擎（GuaxiMarker）处理，
// 已是 HTML 的直接用。异步渲染，用 ref 承载。
const formattedContent = ref('')

const renderContent = async (src) => {
  const text = String(src || '').trim()
  if (!text) { formattedContent.value = ''; return }
  // 已经是 HTML（带常见块级标签）则不走 Markdown，直接渲染。
  const isHtml = /<(p|div|img|h[1-6]|ul|ol|li|blockquote|figure|section|article|table)(\s|>)/i.test(text)
  if (isHtml) { formattedContent.value = text; return }
  if (looksLikeMarkdown(text)) {
    try {
      formattedContent.value = await renderMarkdown(text, {
        mermaid: true,
        math: true,
        codeHighlight: true,
        buttons: false,
        theme: 'default',
      })
      return
    } catch (e) {
      logger.warn('[FlipboardDetailView] markdown render failed, fallback to plain', e)
    }
  }
  // 纯文本：按行包段，链接/图片/媒体行做基本处理（保留原有轻量逻辑）。
  formattedContent.value = text
    .split('\n')
    .map(line => line.trim())
    .filter(Boolean)
    .map(line => {
      if (/^https?:\/\//i.test(line)) {
        const escapedUrl = escapeHtml(line)
        if (isMediaUrl(line)) {
          const embed = escapeHtml(toEmbedUrl(line))
          return `<p><a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p><div class="media-placeholder"><iframe src="${embed}" loading="lazy" referrerpolicy="no-referrer" allowfullscreen></iframe></div>`
        }
        if (isImageUrl(line)) {
          return `<p><img src="${escapedUrl}" loading="lazy" alt="image" /></p>`
        }
        return `<p><a href="${escapedUrl}" target="_blank" rel="noopener noreferrer">${escapedUrl}</a></p>`
      }
      return `<p>${linkifyText(line)}</p>`
    })
    .join('')
}

watch(contentSource, (src) => { renderContent(src) }, { immediate: true })

// 全文翻译（双语对照）：复用编辑器预览的同一套引擎，就地在正文 DOM 注入译文块。
const bilingual = useBilingualPreview({
  getContainer: () => document.querySelector('.article-reader .article-content'),
  getSourceLang: () => 'auto',
  getTargetLang: () => 'zh',
})

const toggleBilingual = () => { bilingual.toggle() }

// 划词翻译：选中正文文字即弹富面板（与编辑器同款）。面板相对 .reader-panel 定位。
const selTranslate = useSelectionTranslate({
  getContainer: () => document.querySelector('.reader-panel'),
})

// 正文 mouseup：Ctrl+划词 → 翻译；普通划词 → 摘录。两者用 Ctrl 区分，不再同时触发。
const onContentMouseUp = (event) => {
  if (event?.ctrlKey) {
    selTranslate.handleMouseUp(event)
  } else {
    selectionNotesRef.value?.handleSelectionMouseUp?.()
  }
}

// 正文重渲染后（切换文章 / hydrate 完成），若双语开着则重新注入译文。
watch(formattedContent, async () => {
  if (bilingual.enabled.value) {
    await nextTick()
    bilingual.refresh()
  }
})

// 切换文章时关掉双语，避免把上一篇的译文状态带过来。
watch(() => props.currentArticle?.id, () => {
  if (bilingual.enabled.value) bilingual.clear()
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
    message.success('已重新抓取并解析', { duration: 1500 })
  } catch (error) {
    message.error(`重新解析失败：${error?.message || error}`, { duration: 2500 })
  } finally {
    reparsing.value = false
  }
}

// 从本地 raw HTML 重放抽取：抽取器升级或规则调整后，用来验证新规则，
// 不需要重新请求站点（原站可能已不可达）。
const replaying = ref(false)
const replayExtraction = async () => {
  const url = props.currentArticle?.url
  if (!url) return
  replaying.value = true
  try {
    await store.replayExtraction(url)
    message.success('已从本地原文重新抽取', { duration: 1500 })
    // 重新拉取详情以显示新结果
    await store.fetchArticleDetail(props.currentArticle)
  } catch (error) {
    message.error(`重放失败：${error?.message || error}`, { duration: 3000 })
  } finally {
    replaying.value = false
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
  /* 充分利用阅读区宽度：上限放宽到 1080px，并按容器宽度自适应（宽屏更宽、窄屏不溢出），
     同时保留左右留白保证长文行宽舒适。 */
  max-width: min(1080px, 100%);
  margin: 0 auto;
  padding: 40px clamp(24px, 6%, 72px);
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
  /* 字体族与编辑器 Markdown 预览一致；正文字号调大到 19px、行高放宽，长文更易读。 */
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', Courier, monospace;
  font-size: 19px;
  line-height: 1.75;
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

/* 嵌套块不重复高亮。
 *
 * markdown-it 的"松散列表"（项之间有空行）会渲染成 <li><p>…</p></li>，
 * 于是鼠标悬停时 li:hover 和 p:hover 同时命中：背景 alpha 从 5% 叠成 9.75%、
 * 左侧强调条也画两次，看起来比相邻段落明显更重（实测截图里一眼能看出）。
 *
 * 处理方式是让内层块透明、把 hover 交给外层：
 * - 列表项里的段落放弃自己的高亮与内边距（li 已经提供了）
 * - 引用块里的段落同理
 * 这样无论渲染成紧凑还是松散列表，视觉都是"一个块亮一次"。
 */
.article-content :deep(li > p),
.article-content :deep(blockquote > p) {
  padding: 0;
  margin-left: 0;
  margin-right: 0;
  border-radius: 0;
  box-shadow: none;
}

.article-content :deep(li > p:hover),
.article-content :deep(blockquote > p:hover) {
  background: transparent;
  box-shadow: none;
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

/* 全文翻译注入的译文块（useBilingualPreview 产物），与编辑器预览保持一致的视觉。 */
.article-content :deep(.guaxi-bilingual-line) {
  margin: 4px 0 10px;
  padding: 6px 10px;
  border-left: 3px solid color-mix(in srgb, var(--guaxi-primary, #4acadb) 65%, transparent);
  border-radius: 0 8px 8px 0;
  background: color-mix(in srgb, var(--guaxi-primary, #4acadb) 8%, transparent);
  color: color-mix(in srgb, var(--guaxi-primary, #4acadb) 32%, var(--guaxi-text-primary, #333));
  font-size: 0.95em;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.article-content :deep(li > .guaxi-bilingual-line),
.article-content :deep(td > .guaxi-bilingual-line),
.article-content :deep(th > .guaxi-bilingual-line) {
  margin: 2px 0 0;
  border-radius: 6px;
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
