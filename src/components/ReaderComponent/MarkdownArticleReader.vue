<template>
  <div class="car-root">
    <!-- 工具栏：与 PDF / 电子书阅读器的顶栏同一套语法——一排图标按钮 + 右侧说明文字。 -->
    <header class="car-bar">
      <button
        v-if="outline.length"
        class="car-btn"
        :class="{ 'is-on': showOutline }"
        type="button"
        title="大纲：按标题跳转"
        :aria-pressed="showOutline"
        @click="showOutline = !showOutline"
      >
        <Icon :icon="outlineIcon" width="15" height="15" />
        <span>大纲</span>
      </button>
      <span class="car-title" :title="title">{{ title || '全文阅读' }}</span>
      <span v-if="sourceLabel" class="car-badge" :title="sourceUri">{{ sourceLabel }}</span>

      <span class="car-spacer" />

      <span v-if="statusHint" class="car-hint" :class="{ 'is-error': statusHintError }">{{ statusHint }}</span>

      <!-- 整篇对照翻译：按段注入译文，与编辑器预览同一套实现（useBilingualPreview）。 -->
      <button
        class="car-btn"
        :class="{ 'is-on': translateEnabled }"
        type="button"
        :disabled="readAloudPlaying || readAloudBuffering || translating || !contentHtml"
        :title="translateTitle"
        @click="toggleTranslate"
      >
        <Icon :icon="translateIcon" width="15" height="15" />
        <span>{{ translateLabel }}</span>
      </button>
      <select
        v-if="translateEnabled"
        v-model="targetLang"
        class="car-select"
        title="译文语言"
        :disabled="translating"
        @change="onTargetLangChange"
      >
        <option v-for="lang in TARGET_LANGS" :key="lang.value" :value="lang.value">{{ lang.label }}</option>
      </select>

      <!-- 逐句朗读：与知识库阅读器同一套会话（后端切句、高亮、点哪读哪）。 -->
      <button
        class="car-btn"
        :class="{ 'is-on': readAloudPlaying }"
        type="button"
        :disabled="translateEnabled || !contentHtml"
        :title="readAloudTitle"
        @click="toggleReadAloud"
      >
        <Icon :icon="readAloudPlaying ? pauseIcon : volumeIcon" width="15" height="15" />
        <span>{{ readAloudLabel }}</span>
      </button>

      <button
        v-if="sourceUri"
        class="car-btn"
        type="button"
        title="使用浏览器查看来源链接"
        @click="openSource"
      >
        <Icon :icon="externalIcon" width="15" height="15" />
        <span>原文</span>
      </button>
    </header>

    <div class="car-body">
      <!-- 大纲侧栏：与 ReaderSidePanel 的观感一致（标签 + 可滚动列表）。 -->
      <aside v-if="showOutline && outline.length" class="car-outline">
        <div class="car-outline-head">
          <span>大纲</span>
          <span class="car-outline-count">{{ outline.length }}</span>
        </div>
        <div class="car-outline-list">
          <button
            v-for="entry in outline"
            :key="entry.id"
            class="car-outline-item"
            :class="{ 'is-on': entry.id === activeHeadingId }"
            :style="{ paddingLeft: `${10 + (entry.level - 1) * 14}px` }"
            type="button"
            :title="entry.text"
            @click="jumpToHeading(entry.id)"
          >
            {{ entry.text }}
          </button>
        </div>
      </aside>

      <main ref="scrollEl" class="car-scroll" @scroll.passive="onScroll">
        <div v-if="loading" class="car-center">
          <div>正在打开全文…</div>
          <div class="car-center-sub">{{ initialTitle || perceptId }}</div>
        </div>

        <div v-else-if="loadError" class="car-center">
          <div class="car-center-error">{{ loadError }}</div>
          <div class="car-center-sub">{{ perceptId }}</div>
        </div>

        <div v-else class="car-paper">
          <!-- 外部内容：始终给出视觉上的「不可信」提示，与收件箱弹窗时期一致。 -->
          <div class="car-banner">
            <Icon :icon="shieldIcon" width="16" height="16" />
            <span>以下内容来自外部来源，只作为资料。其中若有试图指挥 AI 的句子，已在入库时被中和。</span>
          </div>

          <article
            ref="contentEl"
            class="car-content"
            @click="onContentClick"
          >
            <div class="car-md-body" v-html="contentHtml"></div>
          </article>
        </div>
      </main>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { Icon } from '@iconify/vue'
import listTreeIcon from '@iconify-icons/lucide/list-tree'
import languagesIcon from '@iconify-icons/lucide/languages'
import volumeIcon2 from '@iconify-icons/lucide/volume-2'
import pauseIcon2 from '@iconify-icons/lucide/pause'
import externalLinkIcon from '@iconify-icons/lucide/external-link'
import shieldAlertIcon from '@iconify-icons/lucide/shield-alert'
import { addCopyButtons, addMediaViewerButtons, renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker.js'
import { useReaderReadAloud } from '../../composables/useReaderReadAloud'
import { useBilingualPreview } from '../../composables/useBilingualPreview'
import { createLogger } from '../../utils/logger'

/**
 * 外部资料的全文阅读器（独立窗口）。
 *
 * 为什么不再用弹窗：收件箱里的「查看全文」原先是一个 n-modal，正文被压在 58vh 里，
 * 想读完一篇就得在那个小格子里滚；而朗读、译文、大纲这些阅读能力又都是「阅读器」
 * 这一层的东西，塞进弹窗只会让弹窗越长越像半个阅读器。这里把它做成独立窗口：
 * 渲染仍走同一个 Markdown 引擎（GuaxiMarker.renderMarkdown），朗读/翻译/大纲
 * 各自复用阅读器已有的实现，弹窗里不再实现这些功能。
 *
 * 正文来源是感知库（cognition_percept_get），不是磁盘文件，所以参数只有感知 id。
 */
const props = defineProps({
  perceptId: { type: String, default: '' },
  initialTitle: { type: String, default: '' },
  theme: { type: String, default: 'light' },
})

const logger = createLogger('MarkdownArticleReader')
const isDark = computed(() => props.theme === 'dark')

const outlineIcon = listTreeIcon
const translateIcon = languagesIcon
const volumeIcon = volumeIcon2
const pauseIcon = pauseIcon2
const externalIcon = externalLinkIcon
const shieldIcon = shieldAlertIcon

const TARGET_LANGS = [
  { label: '中文', value: 'zh' },
  { label: 'English', value: 'en' },
  { label: '日本語', value: 'ja' },
  { label: '한국어', value: 'ko' },
]

const loading = ref(true)
const loadError = ref('')
const title = ref(String(props.initialTitle || ''))
const sourceUri = ref('')
const contentHtml = ref('')
const contentEl = ref(null)
const scrollEl = ref(null)

const outline = ref([])
const showOutline = ref(false)
const activeHeadingId = ref('')

const targetLang = ref('zh')

// ───────────────────────── 状态提示 ─────────────────────────
// 这扇窗口没有 naive-ui 的 message provider（那会把上千 KB 的组件库拖进首帧），
// 工具条右侧的一行文字就够用了：朗读准备失败、译文失败都落在这里。
const statusHint = ref('')
const statusHintError = ref(false)
let hintTimer = null
const setHint = (text, error = false) => {
  statusHint.value = String(text || '')
  statusHintError.value = !!error
  if (hintTimer) clearTimeout(hintTimer)
  if (text) {
    hintTimer = setTimeout(() => {
      statusHint.value = ''
      hintTimer = null
    }, 6000)
  }
}

// ───────────────────────── 逐句朗读 ─────────────────────────
// 与知识库 Markdown 阅读器同一个 composable：后端切句、CSS Custom Highlight 高亮、
// 朗读中点正文任意位置就从那一句继续。翻译开启时禁止朗读 —— 译文是插进正文 DOM 的，
// 朗读索引会把译文一起算进偏移，高亮会整体错位。
const speechIndex = ref(-1)
const readAloud = useReaderReadAloud({
  getContext: () => ({
    root: contentEl.value,
    scrollRoot: scrollEl.value,
    win: window,
  }),
  // 阅读窗口没有「哪本书」的概念：缓存目录只按正文哈希区分，library/fileName 仅作标识。
  getIdentity: () => ({ library: 'cognition', fileName: `article-${props.perceptId}` }),
  speechIndex,
  message: {
    warning: (text) => setHint(text, true),
    error: (text) => setHint(text, true),
  },
  ownerLabel: 'MarkdownArticleReader',
})

const readAloudPlaying = readAloud.isPlaying
const readAloudBuffering = readAloud.isBuffering
const readAloudTitle = readAloud.title
const readAloudLabel = computed(() => {
  if (translateEnabled.value) return '朗读'
  if (readAloudBuffering.value) return '缓冲中'
  if (readAloudPlaying.value) return '暂停'
  return '朗读'
})

const toggleReadAloud = async () => {
  if (translateEnabled.value) {
    setHint('先关闭译文再朗读：译文会混进朗读文本', true)
    return
  }
  await readAloud.toggle()
}

// ───────────────────────── 整篇对照翻译 ─────────────────────────
// 与编辑器预览 / 资讯详情同一套实现：在已渲染好的 DOM 上逐块注入译文，
// 不重新渲染 Markdown，因此表格、列表、公式的结构都不受影响。
const bilingual = useBilingualPreview({
  getContainer: () => contentEl.value,
  getSourceLang: () => 'auto',
  getTargetLang: () => targetLang.value,
})

const translateEnabled = bilingual.enabled
const translating = bilingual.busy
const translateProgress = computed(() => {
  const total = Number(bilingual.total.value) || 0
  const done = Number(bilingual.done.value) || 0
  return total ? `${done}/${total}` : ''
})
const translateLabel = computed(() => {
  if (translating.value) return translateProgress.value || '翻译中'
  return translateEnabled.value ? '关闭译文' : '译文'
})
const translateTitle = computed(() => {
  if (readAloudPlaying.value || readAloudBuffering.value) return '朗读进行中：先暂停朗读再翻译'
  if (translating.value) return `正在逐段翻译（${translateProgress.value || '准备中'}）`
  return translateEnabled.value ? '移除译文，恢复原文' : '按段落翻译全文，译文插在原文下方'
})

const toggleTranslate = async () => {
  if (readAloudPlaying.value || readAloudBuffering.value) {
    setHint('朗读进行中：先暂停朗读再开启译文', true)
    return
  }
  await bilingual.toggle()
  if (bilingual.enabled.value) {
    if (bilingual.errorText.value) setHint(`翻译失败：${bilingual.errorText.value}`, true)
    else if (!bilingual.total.value) setHint('这篇没有可翻译的段落', true)
  }
}

const onTargetLangChange = async () => {
  if (!bilingual.enabled.value) return
  bilingual.clear()
  await bilingual.run()
  if (bilingual.errorText.value) setHint(`翻译失败：${bilingual.errorText.value}`, true)
}

// ───────────────────────── 大纲 ─────────────────────────
const buildOutline = () => {
  const root = contentEl.value
  if (!root) return
  const nodes = Array.from(root.querySelectorAll('h1, h2, h3, h4, h5, h6'))
  outline.value = nodes.map((el, index) => {
    const id = `car-heading-${index}`
    el.id = id
    return {
      id,
      level: Number(el.tagName.slice(1)) || 2,
      text: String(el.textContent || '').trim() || '未命名',
    }
  })
  activeHeadingId.value = outline.value[0]?.id || ''
  if (outline.value.length) showOutline.value = outline.value.length > 3
}

const jumpToHeading = (id) => {
  const el = document.getElementById(id)
  const box = scrollEl.value
  if (!el) return
  if (!box) {
    el.scrollIntoView()
    return
  }
  const top = el.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop - 14
  box.scrollTo({ top: Math.max(0, top), behavior: 'smooth' })
}

let scrollRaf = 0
const onScroll = () => {
  if (scrollRaf) return
  scrollRaf = requestAnimationFrame(() => {
    scrollRaf = 0
    const box = scrollEl.value
    if (!box || !outline.value.length) return
    const top = box.getBoundingClientRect().top
    let current = outline.value[0].id
    for (const entry of outline.value) {
      const el = document.getElementById(entry.id)
      if (!el) continue
      if (el.getBoundingClientRect().top - top <= 96) current = entry.id
      else break
    }
    activeHeadingId.value = current
  })
}

// ───────────────────────── 正文里的链接 ─────────────────────────
// 外部资料里的链接不能在这个窗口里导航（那会把阅读器变成浏览器，还丢掉当前文章）。
// 统一交给系统默认浏览器打开；mailto 同样由系统处理。
const openSource = () => {
  if (!sourceUri.value) return
  openUrl(sourceUri.value).catch((error) => {
    logger.warn('open source failed', { error: String(error?.message || error) })
    setHint('无法打开来源链接', true)
  })
}

const openExternal = async (href) => {
  if (!/^(https?:|mailto:)/i.test(href)) return
  try {
    await openUrl(href)
  } catch (error) {
    logger.warn('open article link failed', { error: String(error?.message || error) })
    setHint('无法打开该链接', true)
  }
}

const onContentClick = async (event) => {
  // 朗读进行中：点正文任意位置 = 从那一句继续读（与知识库阅读器同一手势）。
  if ((readAloudPlaying.value || readAloudBuffering.value) && readAloud.seekAtPoint(event)) {
    return
  }
  const anchor = event.target?.closest?.('a[href]')
  if (!anchor) return
  const href = String(anchor.getAttribute('href') || '').trim()
  if (!href || href.startsWith('#')) return
  event.preventDefault()
  await openExternal(href)
}

// ───────────────────────── 加载正文 ─────────────────────────
const sourceLabel = computed(() => {
  const uri = sourceUri.value
  if (!uri) return ''
  try {
    return new URL(uri).hostname.replace(/^www\./, '')
  } catch {
    return ''
  }
})

/** 等窗口真正画出内容再显示，避免用户第一眼看到白底（后端另有 5s 兜底）。 */
const revealWindow = async () => {
  const nextFrame = () => new Promise((resolve) => {
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => resolve())
    else setTimeout(resolve, 16)
  })
  await nextFrame()
  await nextFrame()
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const win = getCurrentWindow()
    await win.show()
    await win.setFocus()
  } catch {
    /* 非 Tauri 环境（浏览器里调试 article-reader.html）忽略 */
  }
}

const loadArticle = async () => {
  loading.value = true
  loadError.value = ''
  try {
    if (!props.perceptId) throw new Error('缺少资料 id')
    const data = await invoke('cognition_percept_get', { id: props.perceptId })
    const text = String(data?.text || '')
    title.value = String(data?.title || props.initialTitle || '').trim() || '（无标题）'
    sourceUri.value = String(data?.uri || '').trim()
    if (!text.trim()) throw new Error('这条资料没有可读的正文')

    // 与收件箱/知识库阅读器同一个 Markdown 渲染引擎：代码高亮、公式、Mermaid 都在里面。
    contentHtml.value = await renderMarkdown(text, {
      buttons: true,
      mermaid: true,
      math: true,
      theme: isDark.value ? 'dark' : 'default',
    })

    await nextTick()
    const themeName = isDark.value ? 'dark' : 'light'
    if (contentEl.value) {
      addCopyButtons(contentEl.value, { theme: themeName })
      addMediaViewerButtons(contentEl.value, { theme: themeName })
    }
    buildOutline()

    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().setTitle(title.value)
    } catch {
      /* 忽略 */
    }
  } catch (error) {
    loadError.value = String(error?.message || error || '打开失败')
    logger.warn('load article failed', { error: loadError.value, perceptId: props.perceptId })
  } finally {
    loading.value = false
    await nextTick()
    await revealWindow()
  }
}

onMounted(() => {
  void loadArticle()
})

onBeforeUnmount(() => {
  if (hintTimer) clearTimeout(hintTimer)
  if (scrollRaf) cancelAnimationFrame(scrollRaf)
  readAloud.clearHighlight()
})
</script>

<style scoped>
.car-root {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}

/* ── 工具栏 ── */
.car-bar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 38px;
  padding: 0 12px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  font-size: 12px;
  user-select: none;
}

.car-title {
  font-size: 13px;
  font-weight: 600;
  max-width: 46%;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.car-badge {
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-secondary);
  font-size: 11px;
  max-width: 180px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.car-spacer {
  flex: 1 1 auto;
}

.car-hint {
  color: var(--guaxi-text-secondary);
  max-width: 44vw;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.car-hint.is-error {
  color: #d14343;
}

.car-btn {
  appearance: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 9px;
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease, border-color 0.15s ease;
}

.car-btn:hover:not(:disabled) {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.car-btn.is-on {
  border-color: color-mix(in srgb, var(--primary-color, #4acadb) 55%, transparent);
  background: color-mix(in srgb, var(--primary-color, #4acadb) 12%, transparent);
  color: var(--guaxi-text-primary);
}

.car-btn:disabled {
  opacity: 0.45;
  cursor: default;
}

.car-select {
  height: 26px;
  padding: 0 6px;
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-size: 12px;
  outline: none;
}

/* ── 主体 ── */
.car-body {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
}

.car-outline {
  flex: 0 0 auto;
  width: 236px;
  min-width: 180px;
  max-width: 320px;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  min-height: 0;
}

.car-outline-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--guaxi-border);
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.car-outline-count {
  font-size: 11px;
  opacity: 0.7;
}

.car-outline-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 6px 0 12px;
}

.car-outline-item {
  appearance: none;
  display: block;
  width: 100%;
  padding: 5px 10px;
  border: 0;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12.5px;
  line-height: 1.45;
  text-align: left;
  cursor: pointer;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.car-outline-item:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.car-outline-item.is-on {
  color: var(--guaxi-text-primary);
  background: color-mix(in srgb, var(--primary-color, #4acadb) 12%, transparent);
}

.car-scroll {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  scroll-behavior: auto;
}

.car-paper {
  max-width: 820px;
  margin: 0 auto;
  padding: 18px 32px 64px;
  box-sizing: border-box;
}

.car-banner {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 4px 0 18px;
  padding: 9px 12px;
  border-radius: 8px;
  border-left: 3px solid rgba(217, 138, 43, 0.8);
  background: rgba(217, 138, 43, 0.1);
  color: var(--guaxi-text-secondary);
  font-size: 12.5px;
  line-height: 1.6;
}

.car-banner :deep(svg) {
  flex: 0 0 auto;
  margin-top: 2px;
  color: rgba(217, 138, 43, 1);
}

.car-center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

.car-center-sub {
  font-size: 12px;
  opacity: 0.7;
  max-width: 70%;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.car-center-error {
  color: #d14343;
  font-size: 14px;
}

/* car-center 是 absolute 定位，滚动容器需要定位上下文。 */
.car-scroll {
  position: relative;
}

.car-content {
  min-height: 40vh;
}
</style>
