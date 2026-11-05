<template>
  <div ref="rootEl" class="guaxi-org-md">
    <div v-if="html" ref="bodyRef" class="guaxi-org-md__body" v-html="html" />
    <pre v-else class="guaxi-org-md__raw">{{ content }}</pre>
  </div>
</template>

<script setup>
/**
 * 任务条目字段的 Markdown 渲染。
 *
 * 复用编辑器同一套 `renderMarkdown`（表格 / 代码高亮 / 公式 / Mermaid / 图片代理），
 * 不再维护第二套「够用就好」的解析器。代价是渲染变成异步的，因此缓存、并发队列与
 * 视口调度都放在 guaxiOrgFieldRender 模块里跨卡片共享：
 *   1. 只在内容进入视口附近时才渲染，打开预览不会把几十张卡片的全文一次性送进管线；
 *   2. 结果按「内容 + 主题 + 基准路径」缓存，搜索、切过滤器、重排都不重复渲染；
 *   3. 全局并发上限 3，避免滚动时同时向本地后端发起上百个图片预处理请求。
 * 渲染没完成前显示原始文本，保证卡片内容不闪空。
 */
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  applyQueryHighlight,
  decorateOrgFieldContainer,
  observeOnceVisible,
  orgFieldCacheKey,
  renderMermaidIn,
  renderOrgFieldMarkdown,
} from './utils/guaxiOrgFieldRender'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  content: { type: String, default: '' },
  basePath: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  query: { type: String, default: '' },
  /** 大窗预览等「一出现就要完整内容」的场景跳过视口延迟。 */
  immediate: { type: Boolean, default: false },
})

const logger = createLogger('GuaxiOrgMarkdown')

const rootEl = ref(null)
const bodyRef = ref(null)
const html = ref('')

let renderSeq = 0
let unobserve = null
let started = false
let highlightTimer = null
let renderTimer = null

const themeName = () => (props.theme === 'dark' ? 'dark' : 'default')

const currentCacheKey = () => orgFieldCacheKey({
  content: props.content,
  theme: themeName(),
  basePath: props.basePath,
})

const applyHighlight = () => {
  applyQueryHighlight(bodyRef.value, props.query)
}

const render = async () => {
  const source = String(props.content || '')
  if (!source.trim()) {
    renderSeq += 1
    html.value = ''
    return
  }

  const seq = ++renderSeq
  const key = currentCacheKey()
  try {
    const result = await renderOrgFieldMarkdown({
      content: source,
      theme: themeName(),
      basePath: props.basePath,
    })
    if (seq !== renderSeq) return
    html.value = result
    await nextTick()
    if (seq !== renderSeq) return
    // v-html 重建了整棵子树，按钮与高亮都要挂在新节点上。
    const container = bodyRef.value
    if (!container) return
    decorateOrgFieldContainer(container, themeName())
    applyHighlight()
    // 图只有在这段内容仍是「当前内容」时才值得排版。
    if (key === currentCacheKey()) {
      await renderMermaidIn(container)
    }
  } catch (error) {
    if (seq !== renderSeq) return
    logger.warn('[OrgMarkdown] render failed, keep raw text', {
      error: String(error?.message || error),
    })
    html.value = ''
  }
}

const startRendering = () => {
  if (started) return
  started = true
  void render()
}

/**
 * 内容变化后的重绘去抖。
 * 并排视图里正文每敲一个字都会回流到这里，完整渲染管线（含后端图片预处理）
 * 不该跟着逐键跑；停顿 250ms 再渲染，人眼看不出差别，队列也不会被塞满。
 */
const scheduleRender = () => {
  if (renderTimer) clearTimeout(renderTimer)
  renderTimer = setTimeout(() => {
    renderTimer = null
    void render()
  }, 250)
}

onMounted(() => {
  if (props.immediate) {
    startRendering()
    return
  }
  unobserve = observeOnceVisible(rootEl.value, startRendering)
})

onBeforeUnmount(() => {
  // 让在途渲染的回调失效，避免组件卸载后继续写 DOM。
  renderSeq += 1
  unobserve?.()
  unobserve = null
  if (highlightTimer) {
    clearTimeout(highlightTimer)
    highlightTimer = null
  }
  if (renderTimer) {
    clearTimeout(renderTimer)
    renderTimer = null
  }
})

watch(
  () => [props.content, props.basePath, props.theme],
  () => {
    // 还没进入视口的卡片不必提前渲染；视口命中后会读到最新内容。
    if (started) scheduleRender()
  },
)

watch(
  () => props.query,
  () => {
    // 高亮只改 DOM、不重跑渲染管线；去抖是为了连续输入时不必每键一次全文走一遍。
    if (highlightTimer) clearTimeout(highlightTimer)
    highlightTimer = setTimeout(() => {
      highlightTimer = null
      applyHighlight()
    }, 120)
  },
)
</script>

<style scoped>
.guaxi-org-md {
  min-width: 0;
}

/* 渲染完成前的原始文本：只负责占位与换行，不引入第二套排版。 */
.guaxi-org-md__raw {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font: inherit;
  color: inherit;
}
</style>
