<template>
  <!--
    预览根节点。

    为什么要多包一层：划词浮条要相对「预览窗格」定位（absolute + 选区矩形换算），
    而 GuaxiPdfViewer 的根节点是它自己的布局单元。外层只负责定位上下文与尺寸传递，
    不改变工具条/正文的任何既有排布 —— class/style 仍然透传到这一层，原来给
    GuaxiPdfViewer 的 `previewPaneStyle`（flex 宽度）照旧落在同一个元素上。
  -->
  <div
    ref="rootEl"
    class="gxlp"
    @mouseup="onRootMouseUp"
    @mousedown="hideBar"
    @scroll.capture="hideBar"
  >
    <GuaxiPdfViewer
      ref="viewerRef"
      class="gxlp__viewer"
      :path="pdfPath"
      fit-mode="width"
      :source-path="sourcePath"
      :building="building"
      :error-text="errorText"
      :theme="theme"
      :engine="engineKind"
      :title-label="`${titleLabel} 预览`"
      empty-text="暂无预览，开始编辑或点击 ⟳ 构建"
      :features="features"
      @export="$emit('export')"
      @rebuild="$emit('rebuild')"
      @ready="(p) => $emit('ready', p)"
      @reverse-jump="(p) => $emit('reverse-jump', p)"
      @scroll="$emit('scroll')"
    />

    <!--
      划词浮条：与 PDF 阅读器（ReaderPane）同一套动作，独独不含高亮。

      高亮落在 PDF 的 annotation 里，而预览 PDF 是每次构建覆盖同一个临时文件的产物，
      编辑一次就重建 —— 高亮会跟着旧版本一起消失。与其给一个「存不住」的按钮，
      不如让用户回编辑器里改源文件。
    -->
    <div
      v-if="barVisible"
      class="gxlp-bar"
      :style="{ left: `${anchor.left}px`, top: `${anchor.top}px` }"
      @mousedown.stop.prevent
      @mouseup.stop
    >
      <span class="gxlp-bar__hint">{{ previewText }}</span>
      <button class="gxlp-bar__icon" title="翻译选中的文字" aria-label="翻译" @click="translateSelection">
        <Icon :icon="translateIcon" width="15" height="15" />
      </button>
      <button
        class="gxlp-bar__icon"
        :title="readAloudPlaying ? '停止朗读' : '朗读选中的文字'"
        aria-label="朗读"
        @click="speakSelection"
      >
        <Icon :icon="volumeIcon" width="15" height="15" />
      </button>
      <span class="gxlp-bar__sep" />
      <button class="gxlp-bar__icon" title="查询选中的文字" aria-label="查询" @click="querySelection">
        <Icon :icon="searchQueryIcon" width="15" height="15" />
      </button>
      <button class="gxlp-bar__icon" title="用 AI 翻译选中的文字" aria-label="用 AI 翻译" @click="aiTranslateSelection">
        <Icon :icon="wandIcon" width="15" height="15" />
      </button>
      <button class="gxlp-bar__icon" title="把选中的文字发给 AI 提问" aria-label="问 AI" @click="askSelectionWithAssistant">
        <Icon :icon="botIcon" width="15" height="15" />
      </button>
    </div>

    <!-- 划词译文面板：与阅读器 / 电子书同一套组件，交互一致。 -->
    <GuaxiSelectionTranslatePanel
      v-if="translate.visible.value"
      :pos="translate.pos.value"
      :raw="translate.raw.value"
      :result="translate.result.value"
      :error="translate.error.value"
      :busy="translate.busy.value"
      :pinned="translate.pinned.value"
      :items="translate.items.value"
      :from-lang="translate.fromLang.value"
      :to-lang="translate.toLang.value"
      :lang-options="translate.langOptions"
      :word-meta="translate.wordMeta.value"
      @translate="translate.translateText(translate.raw.value, translate.pos.value)"
      @copy="copyTranslated"
      @swap-langs="swapTranslateLangs"
      @toggle-pin="translate.pinned.value = !translate.pinned.value"
      @update:pos="(v) => { translate.pos.value = v }"
      @update:fromLang="(v) => { translate.fromLang.value = v }"
      @update:toLang="(v) => { translate.toLang.value = v }"
      @close="translate.close()"
    />
  </div>
</template>

<script setup>
// LaTeX / Typst 预览：现在是统一基础组件 GuaxiPdfViewer 的薄包装。
// 对外保持原有的 props / emits / 暴露的 forwardSync 不变，因此 GuaxiEditor 无需改动。
// 划词浮条由本层提供（与 PDF 阅读器一致），基础组件只负责渲染与选区上报。
import { computed, defineAsyncComponent, ref } from 'vue'
import { Icon } from '@iconify/vue'
import translateIcon from '@iconify-icons/lucide/languages'
import volumeIcon from '@iconify-icons/lucide/volume-2'
import searchQueryIcon from '@iconify-icons/lucide/search'
import wandIcon from '@iconify-icons/lucide/wand-2'
import botIcon from '@iconify-icons/lucide/bot'
import { openUrl } from '@tauri-apps/plugin-opener'
import GuaxiPdfViewer from '../PdfViewer/GuaxiPdfViewer.vue'
import { usePageTranslate } from '../../composables/usePageTranslate'
import { useSelectionReadAloud } from '../../composables/useSelectionReadAloud'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { useSiderStore } from '../../stores/useSiderStore'
import { createLogger } from '../../utils/logger'

// 译文面板是这里唯一依赖 naive-ui 的组件：只在真正翻译时加载。
const GuaxiSelectionTranslatePanel = defineAsyncComponent(
  () => import('./GuaxiSelectionTranslatePanel.vue')
)

const props = defineProps({
  // 已构建好的 PDF 临时文件路径
  pdfPath: { type: String, default: '' },
  // 源文件路径（.tex / .typ）：双向定位需要
  sourcePath: { type: String, default: '' },
  building: { type: Boolean, default: false },
  errorText: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  // 'LaTeX' | 'Typst'
  titleLabel: { type: String, default: 'LaTeX' },
})

defineEmits(['export', 'rebuild', 'ready', 'reverse-jump', 'scroll'])

const logger = createLogger('GuaxiLatexPreview')
const viewerRef = ref(null)
const rootEl = ref(null)

const rightSiderStore = useRightSiderStore()
const siderStore = useSiderStore()

const engineKind = computed(() => (/typst/i.test(props.titleLabel) ? 'typst' : 'latex'))

const features = {
  search: true,
  reverseJump: true,
  outline: false,
  export: true,
  rebuild: true,
}

// ───────────────────────── 划词 ─────────────────────────
// 预览就是 GuaxiPdfViewer，正文是带透明文本层的真实页面，所以选区就在主文档里：
// 与 ReaderPane 一样直接用 window.getSelection()，不需要 iframe 转发那一套。
const translate = usePageTranslate({ getContainer: () => rootEl.value || null })
const readAloud = useSelectionReadAloud({})
const readAloudPlaying = readAloud.busy

const barVisible = ref(false)
const selectionText = ref('')
const anchor = ref({ left: 0, top: 0 })

/** 浮条上只放一段摘要：整段原文会把浮条撑到盖住正文。 */
const previewText = computed(() => {
  const t = String(selectionText.value || '').trim().replace(/\s+/g, ' ')
  if (!t) return ''
  return t.length > 20 ? `${t.slice(0, 20)}…` : t
})

/**
 * 把选区矩形换算成相对根节点的坐标。
 *
 * 用选区矩形而不是鼠标位置：滚动、缩放之后鼠标早就离开那段文字了，
 * 而矩形始终贴着它。浮条放在选区下方，下面放不下就翻到上方。
 */
const placeBar = () => {
  const sel = window.getSelection?.()
  if (!sel || !sel.rangeCount) return false
  const rect = sel.getRangeAt(0).getBoundingClientRect()
  if (!rect || (!rect.width && !rect.height)) return false
  const root = rootEl.value?.getBoundingClientRect?.()
  if (!root) return false
  const BAR_H = 40
  const left = Math.min(
    Math.max(8, rect.left - root.left + rect.width / 2),
    Math.max(8, root.width - 8)
  )
  const below = rect.bottom - root.top + 8
  const top = below + BAR_H <= root.height ? below : Math.max(8, rect.top - root.top - BAR_H - 8)
  anchor.value = { left: Math.round(left), top: Math.round(top) }
  return true
}

const hideBar = () => {
  barVisible.value = false
  selectionText.value = ''
}

const onRootMouseUp = () => {
  const sel = window.getSelection?.()
  const text = String(sel || '').trim()
  // 选区必须落在本预览窗格内：编辑器正文里选字不该在这里弹条。
  if (!text || !sel?.anchorNode || !rootEl.value?.contains(sel.anchorNode)) {
    hideBar()
    return
  }
  selectionText.value = text
  if (placeBar()) barVisible.value = true
}

const clearSelection = () => {
  window.getSelection?.()?.removeAllRanges()
  hideBar()
}

const translateSelection = () => {
  const text = String(selectionText.value || '').trim()
  if (!text) return
  translate.translateText(text, anchor.value)
  hideBar()
}

const speakSelection = () => {
  const text = String(selectionText.value || '').trim()
  if (!text) return
  if (readAloudPlaying.value) {
    readAloud.stop()
    return
  }
  void readAloud.toggle(text)
  hideBar()
}

const querySelection = async () => {
  const text = String(selectionText.value || '').trim()
  if (!text) return
  const url = `https://www.bing.com/search?q=${encodeURIComponent(text)}`
  clearSelection()
  try {
    await openUrl(url)
  } catch (error) {
    logger.warn('[GuaxiLatexPreview] open search url failed', {
      error: String(error?.message || error),
    })
  }
}

/**
 * 打开右侧智能助手。预览永远在编辑器里（/home），所以走全局右栏；
 * 只调 open() 看不见 —— 全局右栏默认折叠，必须同时展开。
 */
const sendSelectionToAssistant = (text, options = {}) => {
  const t = String(text || '').trim()
  if (!t) return false
  rightSiderStore.setPanel('assistant')
  rightSiderStore.open()
  siderStore.setRightSiderCollapsed('home', false)
  rightSiderStore.appendAssistantDraft(t, {
    autoSend: options.autoSend !== false,
    disableWebSearch: !!options.disableWebSearch,
  })
  return true
}

const askSelectionWithAssistant = () => {
  const text = String(selectionText.value || '').trim()
  if (!text) return
  sendSelectionToAssistant(text, { autoSend: true })
  clearSelection()
}

const aiTranslateSelection = () => {
  const text = String(selectionText.value || '').trim()
  if (!text) return
  const lang = translate.toLang?.value || 'zh'
  const label = translate.langOptions?.find((x) => x.value === lang)?.label || lang
  const prompt = `请将以下内容翻译成${label}，保持原意并保留术语与格式，只输出译文：\n\n${text}`
  if (sendSelectionToAssistant(prompt, { autoSend: true, disableWebSearch: true })) clearSelection()
}

const copyTranslated = async (payload) => {
  const t = typeof payload === 'string' ? payload : payload?.text
  const text = String(t || translate.result.value || '')
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
  } catch (error) {
    logger.warn('[GuaxiLatexPreview] copy translation failed', {
      error: String(error?.message || error),
    })
  }
}

/** 交换语言后重新翻一次：只换两个下拉框的值，用户会以为按钮坏了。 */
const swapTranslateLangs = () => {
  const from = translate.fromLang.value
  const to = translate.toLang.value
  if (from === 'auto') return
  translate.fromLang.value = to
  translate.toLang.value = from
  const raw = String(translate.raw.value || '').trim()
  if (raw) translate.translateText(raw, translate.pos.value)
}

// 供 GuaxiEditor 调用的正向定位，转发给基础组件。
const forwardSync = (payload) => viewerRef.value?.forwardSync(payload)
// 预留给宿主的大纲/书签页码跳转能力；不改变当前正向定位链路，只把 PdfViewer 已有能力透出。
const goToPage = (page, options) => viewerRef.value?.goToPage(page, options)
// 设计说明：GuaxiEditor 会按源文件记住 PDF 阅读位置；这里继续转发基础阅读器的比例滚动 API，
// 避免编辑器层需要知道 PDF 阅读器内部 DOM 结构。
// null 必须原样透传（容器不可测量），不能兜底成 0：宿主会把它当成“用户就在文档开头”存下来。
const scrollRatio = () => viewerRef.value?.scrollRatio?.() ?? null
const restoreScroll = (ratio) => viewerRef.value?.restoreScroll?.(ratio)
// 跨帧重试到布局稳定的落位：切换文件后恢复阅读位置时用，避免文档刚打开、容器还没有
// 可滚动高度时只落一帧被 clamp 到顶部。
const restoreScrollStable = (ratio) => viewerRef.value?.restoreScrollStable?.(ratio)

defineExpose({ forwardSync, goToPage, scrollRatio, restoreScroll, restoreScrollStable })
</script>

<style scoped>
/* 定位上下文 + 把尺寸透给基础组件：原来宿主给的是 flex 宽度，这里原样承接。 */
.gxlp {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.gxlp__viewer {
  flex: 1;
  min-height: 0;
}

.gxlp-bar {
  position: absolute;
  z-index: 20;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 10px;
  background: rgba(24, 26, 30, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.14);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
  color: rgba(255, 255, 255, 0.9);
  font-size: 12px;
  white-space: nowrap;
  user-select: none;
}
.gxlp-bar__hint {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  opacity: 0.72;
}
.gxlp-bar__sep {
  width: 1px;
  height: 14px;
  background: rgba(255, 255, 255, 0.18);
}
.gxlp-bar__icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  color: inherit;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}
.gxlp-bar__icon:hover { background: rgba(255, 255, 255, 0.12); }
</style>
