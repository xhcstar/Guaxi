<template>
  <div class="guaxi-doc-reader" data-reader="slides">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">PPTX</span>
      <span v-if="slideCount" class="guaxi-doc-reader__meta">共 {{ slideCount }} 页</span>
      <span class="guaxi-doc-reader__spacer" />
      <button
        class="guaxi-doc-reader__btn"
        :class="{ 'is-on': viewMode === 'slides' }"
        title="按版式预览"
        :disabled="loading || !!errorText"
        @click="setMode('slides')"
      >版式</button>
      <button
        class="guaxi-doc-reader__btn"
        :class="{ 'is-on': viewMode === 'outline' }"
        title="只看大纲（无版式时可用）"
        :disabled="loading || !!errorText"
        @click="setMode('outline')"
      >大纲</button>
      <button class="guaxi-doc-reader__btn" title="重新加载" :disabled="loading" @click="reload">重新加载</button>
    </div>

    <div ref="bodyRef" class="guaxi-doc-reader__body">
      <div v-if="loading" class="guaxi-doc-reader__center">
        <div>正在解析演示文稿…</div>
        <div class="guaxi-doc-reader__meta">首次加载需要引入渲染内核，稍等片刻</div>
      </div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
        <button class="guaxi-doc-reader__btn" @click="setMode('outline')">改用大纲视图</button>
      </div>

      <!-- 版式视图：@vue-office/pptx 自带渲染内核，离线可用。 -->
      <div v-show="!loading && !errorText && viewMode === 'slides'" ref="pptxHostRef" class="gdr-pptx-host" />

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
const slideCount = ref(0)
const outlineHtml = ref('')
let outlineMarkdown = ''

let loadToken = 0
let pptxInstance = null
let pptxModPromise = null

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
        } catch {
          slideCount.value = 0
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

const reload = () => {
  void load(false)
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
