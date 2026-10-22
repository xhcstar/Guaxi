<template>
  <div v-if="streaming" class="agent-md agent-md--plain">{{ text }}</div>
  <div v-else class="agent-md" v-html="html" />
</template>

<script setup>
/**
 * 工作台正文的 Markdown 渲染。
 *
 * 复用项目既有的 `renderMarkdown`（mermaid / 公式 / 代码高亮 / 图表内嵌都在里面），
 * 不另引一套解析器——工作台里的回答和编辑器里的文档必须长得一样，否则同一段内容
 * 在两个地方排版不同，用户会以为哪里出错了。
 *
 * 流式期间不渲染：每个 delta 都跑一遍完整管线（含对 8888 后端的请求）代价太高，
 * 而且未闭合的 ``` 围栏会让整段闪烁。流式时显示纯文本，结束后一次性渲染成 HTML。
 */
import { ref, watch } from 'vue'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'
import { createLogger } from '../../utils/logger'

const props = defineProps({
  text: { type: String, default: '' },
  streaming: { type: Boolean, default: false },
})

const logger = createLogger('AgentMarkdown')
const html = ref('')

let renderSeq = 0

const render = async () => {
  const source = props.text || ''
  if (!source || props.streaming) {
    html.value = ''
    return
  }
  const seq = ++renderSeq
  try {
    const out = await renderMarkdown(source, {
      basePath: '',
      mermaid: true,
      math: true,
      codeHighlight: true,
      chatPreprocess: true,
      includeNotes: false,
      deferMathUntilComplete: false,
      smartInlineMathFromCode: true,
    })
    // 渲染是异步的：期间文本又变了就丢弃这次结果，避免旧内容覆盖新内容。
    if (seq === renderSeq) html.value = String(out || '')
  } catch (err) {
    logger.warn('[agent] markdown render failed', { error: String(err) })
    if (seq === renderSeq) html.value = ''
  }
}

watch(() => [props.text, props.streaming], render, { immediate: true })
</script>

<style scoped>
.agent-md {
  font-size: 14.5px;
  line-height: 1.72;
  color: var(--guaxi-text-primary);
  word-break: break-word;
}

.agent-md--plain {
  white-space: pre-wrap;
  font-family: inherit;
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
