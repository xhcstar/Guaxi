<template>
  <div class="agent-md" v-html="html" />
</template>

<script setup>
/**
 * 工作台正文的 Markdown 渲染。
 *
 * 复用项目既有的 `renderMarkdown`（mermaid / 公式 / 代码高亮 / 图表内嵌都在里面），
 * 不另引一套解析器——工作台里的回答和编辑器里的文档必须长得一样，否则同一段内容
 * 在两个地方排版不同，用户会以为哪里出错了。
 *
 * 流式期间也渲染，但做了三层收敛：
 * 1. 按固定间隔重绘（120ms），不是每个 delta 都跑一遍完整管线；
 * 2. 上一次渲染还没结束就不再起新的，期间的变化只记一个「待重绘」标记；
 * 3. 流式阶段打开 `deferMathUntilComplete`，公式没闭合时先不交给 MathJax，
 *    否则会出现一半渲染好、一半还是 LaTeX 源码的混合状态。
 * 流式结束后再跑一次完整管线（公式、mermaid 都在这一次落定）。
 */
import { onBeforeUnmount, ref, watch } from 'vue'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  text: { type: String, default: '' },
  streaming: { type: Boolean, default: false },
})

const logger = createLogger('AgentMarkdown')
const html = ref('')

/** 流式重绘的最小间隔：再快人眼也读不出来，只会白烧解析和公式排版。 */
const STREAM_INTERVAL_MS = 120

let renderSeq = 0
let timer = null
let inFlight = false
let trailing = false
let lastRenderAt = 0

const escapeHtml = (s) =>
  String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')

const startRender = async () => {
  if (inFlight) {
    trailing = true
    return
  }
  const source = props.text || ''
  if (!source) {
    html.value = ''
    lastRenderAt = Date.now()
    return
  }

  const streaming = props.streaming
  const seq = ++renderSeq
  inFlight = true
  lastRenderAt = Date.now()
  try {
    const out = await renderMarkdown(source, {
      basePath: '',
      // 流式阶段先不跑 mermaid：图只有围栏闭合后才有意义，而这期间每 120ms
      // 重新排一次图既慢又会闪，留给流式结束后的完整渲染。
      mermaid: !streaming,
      math: true,
      codeHighlight: true,
      chatPreprocess: true,
      includeNotes: false,
      deferMathUntilComplete: streaming,
      smartInlineMathFromCode: true,
    })
    if (seq === renderSeq) html.value = String(out || '')
  } catch (err) {
    logger.warn('[agent] markdown render failed', { error: String(err) })
    // 渲染失败也不能让用户对着空白等：退回转义后的原文。
    if (seq === renderSeq) html.value = `<pre class="agent-md-fallback">${escapeHtml(source)}</pre>`
  } finally {
    inFlight = false
    if (trailing) {
      trailing = false
      requestRender()
    }
  }
}

const requestRender = () => {
  if (inFlight) {
    trailing = true
    return
  }
  const since = Date.now() - lastRenderAt
  if (since >= STREAM_INTERVAL_MS) {
    startRender()
    return
  }
  if (!timer) {
    timer = setTimeout(() => {
      timer = null
      startRender()
    }, STREAM_INTERVAL_MS - since)
  }
}

watch(
  () => [props.text, props.streaming],
  () => {
    if (!props.streaming) {
      // 收尾必须立刻渲染：这一次决定公式和图表最终长什么样，不能等节流窗口。
      if (timer) {
        clearTimeout(timer)
        timer = null
      }
      startRender()
      return
    }
    requestRender()
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (timer) {
    clearTimeout(timer)
    timer = null
  }
  // 让还在路上的渲染结果作废，避免卸载后写进已销毁的 ref。
  renderSeq += 1
})
</script>

<style scoped>
.agent-md {
  font-size: 14.5px;
  line-height: 1.72;
  color: var(--guaxi-text-primary);
  word-break: break-word;
}

.agent-md :deep(.agent-md-fallback) {
  margin: 0;
  white-space: pre-wrap;
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
}

.agent-md :deep(p) {
  margin: 0 0 0.7em;
}

.agent-md :deep(p:last-child) {
  margin-bottom: 0;
}

.agent-md :deep(pre) {
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  padding: 10px 12px;
  overflow-x: auto;
  font-family: var(--guaxi-editor-font-family);
  font-size: 13px;
}

.agent-md :deep(code) {
  font-family: var(--guaxi-editor-font-family);
  font-size: 0.92em;
}

.agent-md :deep(:not(pre) > code) {
  background: var(--guaxi-hover-bg);
  border-radius: 4px;
  padding: 1px 5px;
}

.agent-md :deep(ul),
.agent-md :deep(ol) {
  padding-left: 1.4em;
  margin: 0.5em 0 0.7em;
}

.agent-md :deep(a) {
  color: var(--guaxi-syntax-function);
}

.agent-md :deep(table) {
  border-collapse: collapse;
  margin: 0.6em 0;
}

.agent-md :deep(th),
.agent-md :deep(td) {
  border: 1px solid var(--guaxi-border);
  padding: 5px 9px;
}
</style>
