<template>
  <div class="guaxi-doc-reader" data-reader="slides">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">PPTX</span>
      <span v-if="slideCount" class="guaxi-doc-reader__meta">共 {{ slideCount }} 页</span>
      <span class="guaxi-doc-reader__spacer" />
      <CraftEditButton :path="path" :kind="kind" />
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': viewMode === 'slides' }"
        title="按版式预览"
        aria-label="按版式预览"
        :aria-pressed="viewMode === 'slides'"
        :disabled="loading || !!errorText"
        @click="setMode('slides')"
      >
        <Icon :icon="layoutIcon" width="15" height="15" />
      </button>
      <button
        v-if="slideTitles.length && viewMode === 'slides'"
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': slideOutlineVisible }"
        title="显示/隐藏幻灯片列表"
        aria-label="显示或隐藏幻灯片列表"
        :aria-pressed="slideOutlineVisible"
        @click="slideOutlineVisible = !slideOutlineVisible"
      >
        <Icon :icon="panelIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        :class="{ 'is-on': viewMode === 'outline' }"
        title="只看大纲（无版式时可用）"
        aria-label="只看大纲"
        :aria-pressed="viewMode === 'outline'"
        :disabled="loading || !!errorText"
        @click="setMode('outline')"
      >
        <Icon :icon="outlineIcon" width="15" height="15" />
      </button>
      <span class="guaxi-doc-reader__divider" />
      <ZoomControl v-model="zoom" :disabled="loading || !!errorText || viewMode !== 'slides'" />
      <button
        class="guaxi-doc-reader__icon-btn"
        title="重新加载"
        aria-label="重新加载"
        :disabled="loading"
        @click="reload"
      >
        <Icon :icon="refreshIcon" width="15" height="15" />
      </button>
    </div>

    <div ref="bodyRef" class="guaxi-doc-reader__body gdr-pptx-layout">
      <div v-if="loading" class="guaxi-doc-reader__center">
        <div>正在解析演示文稿…</div>
        <div class="guaxi-doc-reader__meta">首次加载需要引入渲染内核，稍等片刻</div>
      </div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
        <button class="guaxi-doc-reader__btn" @click="setMode('outline')">改用大纲视图</button>
      </div>

      <!--
        幻灯片导航侧栏。

        为什么需要：500 页的 PPT 光靠滚动几乎没法用 —— 想跳到第 300 页只能一直拖。
        这里按 @vue-office/pptx 渲染出的 slide 元素给出一页一条的列表，
        点击即滚动定位。不做缩略图：那需要把每页再栅格化一遍，
        500 页会带来明显的启动延迟与内存占用，编号 + 标题已经够导航。
      -->
      <aside v-if="slideOutlineVisible && slideTitles.length" class="gdr-pptx-outline">
        <div class="gdr-pptx-outline__head">
          <span>幻灯片</span>
          <button
            class="guaxi-doc-reader__icon-btn"
            title="隐藏幻灯片列表"
            aria-label="隐藏幻灯片列表"
            @click="slideOutlineVisible = false"
          >
            <Icon :icon="closeIcon" width="15" height="15" />
          </button>
        </div>
        <div class="gdr-pptx-outline__body">
          <button
            v-for="(item, idx) in slideTitles"
            :key="idx"
            class="gdr-pptx-outline__item"
            :class="{ 'is-active': currentSlide === idx }"
            :title="item || `第 ${idx + 1} 页`"
            @click="goToSlide(idx)"
          >
            <span class="gdr-pptx-outline__num">{{ idx + 1 }}</span>
            <span class="gdr-pptx-outline__title">{{ item || '（无标题）' }}</span>
          </button>
        </div>
      </aside>

      <!-- 版式视图：@vue-office/pptx 自带渲染内核，离线可用。 -->
      <div
        v-show="!loading && !errorText && viewMode === 'slides'"
        ref="pptxHostRef"
        class="gdr-pptx-host"
        :style="zoomStyle"
      />

      <!-- 大纲视图：渲染失败或用户主动切换时的兜底，走已有的转换链路。 -->
      <div v-if="!loading && viewMode === 'outline'" ref="outlineRef" class="gdr-md" v-html="outlineHtml" />
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { renderMarkdown, addCopyButtons } from '../../GuaxiMarkdownRender/GuaxiMarker'
import { createLogger } from '../../../utils/logger'
import CraftEditButton from './CraftEditButton.vue'
import ZoomControl from './ZoomControl.vue'
import { Icon } from '@iconify/vue'
import layoutIcon from '@iconify-icons/lucide/layout-grid'
import outlineIcon from '@iconify-icons/lucide/list-tree'
import closeIcon from '@iconify-icons/lucide/x'
import panelIcon from '@iconify-icons/lucide/panel-left'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import './readerPane.css'

const logger = createLogger('DocPptxReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'pptx' },
})

const bodyRef = ref(null)
const pptxHostRef = ref(null)
const outlineRef = ref(null)

const loading = ref(true)
const errorText = ref('')
const viewMode = ref('slides')
/** 版式视图的缩放倍数。@vue-office/pptx 按容器宽度自适应，这里用 zoom 叠加用户缩放。 */
const zoom = ref(1)
const slideCount = ref(0)
/** 每页标题（供导航列表）；缺标题的页为空串。 */
const slideTitles = ref([])
/** 当前可见的幻灯片索引。 */
const currentSlide = ref(0)
const slideOutlineVisible = ref(true)
const outlineHtml = ref('')
let outlineMarkdown = ''

let loadToken = 0
let pptxInstance = null
let pptxModPromise = null

const zoomStyle = computed(() => ({ zoom: zoom.value }))

const zoomBy = (factor) => {
  zoom.value = Math.min(3, Math.max(0.5, +(zoom.value * factor).toFixed(3)))
}

const themeName = () => {
  try {
    const root = document?.documentElement
    const isDark = !!root?.classList?.contains?.('dark') ||
      !!root?.classList?.contains?.('dark-mode') ||
      root?.dataset?.guaxiTheme === 'dark'
    return isDark ? 'dark' : 'default'
  } catch {
    return 'default'
  }
}

/**
 * 动态加载 @vue-office/pptx。
 *
 * 为什么按需加载：这个包约 1.4MB（含自带的 JSZip），只在用户真的打开 .pptx 时才需要。
 * 静态 import 会把它塞进主包，拖慢每次启动——与「构建 4 分钟、主包已很大」的现状冲突。
 */
const loadPptx = () => {
  if (!pptxModPromise) {
    pptxModPromise = import('@vue-office/pptx').then((mod) => mod.default || mod)
  }
  return pptxModPromise
}

const setMode = (mode) => {
  viewMode.value = mode
  if (mode === 'outline') void ensureOutline()
}

/** 大纲兜底：用已有的转换命令把 pptx 转成 Markdown，再复用同源渲染器。 */
const ensureOutline = async () => {
  if (outlineHtml.value) return
  const token = loadToken
  try {
    const result = await invoke('convert_document_to_markdown', {
      path: props.path,
      quality: 'fast',
      mediaRoot: null,
    })
    if (token !== loadToken) return
    outlineMarkdown = String(result?.markdown || '')
    outlineHtml.value = await renderMarkdown(outlineMarkdown, {
      preserveOriginal: false,
      basePath: '',
      mermaid: true,
      math: true,
      codeHighlight: true,
      buttons: false,
      theme: themeName(),
    })
    await nextTick()
    if (outlineRef.value) addCopyButtons(outlineRef.value, { theme: themeName() })
  } catch (error) {
    if (token !== loadToken) return
    outlineHtml.value = `<p class="guaxi-doc-reader__error">大纲生成失败：${String(error?.message || error)}</p>`
  }
}

/**
 * 挂载版式预览。
 *
 * @vue-office/pptx 是 UMD 组件（带 install），这里用 h() 手动创建，
 * 避免为它注册全局组件、也避免它影响既有组件解析。
 */
const mountPptx = async (bytes) => {
  const Pptx = await loadPptx()
  await nextTick()
  const host = pptxHostRef.value
  if (!host) throw new Error('预览容器尚未就绪')

  // 组件内部按 100% 宽高铺开，容器必须有确定尺寸。
  host.innerHTML = ''
  const { createApp, h } = await import('vue')
  pptxInstance = createApp({
    render: () => h(Pptx, {
      src: bytes,
      // 组件对外提供的是这两个事件，比轮询 DOM 可靠：
      // 渲染完成时才可能数到页数，出错时也能把真实原因带给用户。
      onRendered: () => {
        try {
          const slides = host.querySelectorAll('.pptx-preview-slide-wrapper, .vue-office-pptx section')
          slideCount.value = slides.length
          captureSlideTitles(host, slides)
        } catch {
          slideCount.value = 0
          slideTitles.value = []
        }
        logger.info('[DocPptxReader] rendered event', { slides: slideCount.value })
      },
      onError: (err) => {
        logger.warn('[DocPptxReader] error event', { error: String(err?.message || err) })
      },
    }),
  })
  pptxInstance.mount(host)
}

/**
 * 从渲染后的 DOM 里提取每页标题，供导航列表显示。
 *
 * 取每个 slide 里第一个标题级元素的文本；没有标题就留空，
 * 列表里退化成「（无标题）」+ 页码。只读一次 DOM，不引入额外解析。
 */
const captureSlideTitles = (host, slides) => {
  const titles = []
  slides.forEach((slide) => {
    let title = ''
    try {
      const heading = slide.querySelector('h1, h2, h3, h4, h5, h6, [class*="title" i]')
      title = String(heading?.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 60)
    } catch {
      title = ''
    }
    titles.push(title)
  })
  slideTitles.value = titles
  currentSlide.value = 0
  observeSlides(host)
  logger.info('[DocPptxReader] slide titles captured', {
    total: titles.length,
    withTitle: titles.filter(Boolean).length,
  })
}

/** 跳到第 idx 页：滚动到对应的 slide 元素。 */
const goToSlide = (idx) => {
  const host = pptxHostRef.value
  if (!host) return
  const slides = host.querySelectorAll('.pptx-preview-slide-wrapper, .vue-office-pptx section')
  const target = slides[idx]
  if (!target) return
  currentSlide.value = idx
  try {
    target.scrollIntoView({ block: 'start', behavior: 'auto' })
  } catch (error) {
    logger.debug('[DocPptxReader] scrollIntoView failed', {
      error: String(error?.message || error),
    })
  }
}

/** 滚动时同步高亮当前页：用 IntersectionObserver 比监听 scroll 更省。 */
let slideObserver = null

const observeSlides = (host) => {
  slideObserver?.disconnect?.()
  slideObserver = null
  if (typeof IntersectionObserver === 'undefined') return
  const slides = host.querySelectorAll('.pptx-preview-slide-wrapper, .vue-office-pptx section')
  if (!slides.length) return

  const body = bodyRef.value
  slideObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue
        const idx = Array.prototype.indexOf.call(slides, entry.target)
        if (idx >= 0) currentSlide.value = idx
      }
    },
    { root: body || null, threshold: 0.55 }
  )
  slides.forEach((slide) => slideObserver.observe(slide))
}

const load = async (tryOutlineOnFail = true) => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''

  try {
    if (pptxInstance) {
      try { pptxInstance.unmount() } catch { /* 已卸载 */ }
      pptxInstance = null
    }

    // 既有命令：返回原始字节数组。
    const bytes = await invoke('read_file_bytes_base64', { path: props.path })
    if (token !== loadToken) return
    if (!bytes) throw new Error('文件内容为空')

    const binary = atob(String(bytes))
    const buf = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i += 1) buf[i] = binary.charCodeAt(i)

    await mountPptx(buf.buffer)
    if (token !== loadToken) return
    loading.value = false
    viewMode.value = 'slides'
    logger.info('[DocPptxReader] rendered', { path: props.path, bytes: buf.length })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error)
    logger.warn('[DocPptxReader] render failed', {
      path: props.path,
      error: String(error?.message || error),
    })
    // 版式渲染失败时自动退回大纲，保证「至少能看到内容」。
    if (tryOutlineOnFail) {
      viewMode.value = 'outline'
      void ensureOutline()
    }
  }
}

onMounted(() => {
  void load()
})

onBeforeUnmount(() => {
  loadToken += 1
  if (pptxInstance) {
    try { pptxInstance.unmount() } catch { /* 已卸载 */ }
    pptxInstance = null
  }
})

watch(() => props.path, () => {
  outlineHtml.value = ''
  slideCount.value = 0
  void load()
})
</script>
