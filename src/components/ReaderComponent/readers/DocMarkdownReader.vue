<template>
  <div class="guaxi-doc-reader" data-reader="markdown">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">{{ kind.toUpperCase() }} → Markdown</span>
      <span class="guaxi-doc-reader__meta">{{ engineLabel }}</span>
      <span class="guaxi-doc-reader__spacer" />
      <ZoomControl v-model="zoom" :ratio="1.12" :disabled="loading || !!errorText" />
      <span class="guaxi-doc-reader__divider" />
      <button
        class="guaxi-doc-reader__icon-btn"
        title="重新转换（用当前引擎重新解析源文件）"
        aria-label="重新转换"
        :disabled="loading"
        @click="reload(true)"
      >
        <Icon :icon="refreshIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="复制 Markdown 源码"
        aria-label="复制 Markdown 源码"
        :disabled="!markdown"
        @click="copyMarkdown"
      >
        <Icon :icon="copyIcon" width="15" height="15" />
      </button>
    </div>

    <div ref="bodyRef" class="guaxi-doc-reader__body">
      <div v-if="loading" class="guaxi-doc-reader__center">
        <div>{{ statusText }}</div>
        <div class="guaxi-doc-reader__meta">旧版 Office / RTF / ODT / 幻灯片通过转换引擎生成只读版式</div>
      </div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
        <button class="guaxi-doc-reader__btn" @click="reload(true)">重试</button>
      </div>
      <div v-else class="gdr-md" :style="{ zoom }" v-html="renderedHtml" />
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useMessage } from 'naive-ui'
import { renderMarkdown, addCopyButtons, addMediaViewerButtons } from '../../GuaxiMarkdownRender/GuaxiMarker'
import { Icon } from '@iconify/vue'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import copyIcon from '@iconify-icons/lucide/copy'
import ZoomControl from './ZoomControl.vue'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocMarkdownReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'doc' },
})

const message = useMessage()

const loading = ref(true)
const errorText = ref('')
const markdown = ref('')
const renderedHtml = ref('')
const zoom = ref(1)
const bodyRef = ref(null)
const engine = ref('')
const statusText = ref('正在转换文档…')

const engineLabel = computed(() => (engine.value ? `引擎：${engine.value}` : ''))

let loadToken = 0

const themeName = () => {
  try {
    const root = document?.documentElement
    const isDark =
      !!root?.classList?.contains?.('dark') ||
      !!root?.classList?.contains?.('dark-mode') ||
      root?.dataset?.guaxiTheme === 'dark'
    return isDark ? 'dark' : 'default'
  } catch {
    return 'default'
  }
}

const zoomBy = (factor) => {
  zoom.value = Math.min(2, Math.max(0.6, +(zoom.value * factor).toFixed(3)))
}

const copyMarkdown = async () => {
  try {
    await navigator.clipboard.writeText(markdown.value || '')
    message.success('已复制 Markdown 源码')
  } catch (error) {
    logger.warn('[DocMarkdownReader] copy failed', { error: String(error?.message || error) })
    message.error('复制失败')
  }
}

const render = async () => {
  const source = markdown.value
  if (!source.trim()) {
    renderedHtml.value = '<p class="guaxi-doc-reader__meta">转换结果为空</p>'
    return
  }
  try {
    // 复用与编辑器预览同源的渲染器：Mermaid / MathJax / 代码高亮全部自动生效，
    // 不需要在这个阅读器里再复制一套渲染逻辑。
    renderedHtml.value = await renderMarkdown(source, {
      preserveOriginal: false,
      basePath: '',
      mermaid: true,
      math: true,
      codeHighlight: true,
      buttons: false,
      theme: themeName(),
    })
    // renderMarkdown 只产出 `<pre class="mermaid">源码</pre>`；真正的 SVG 需要 mermaid.run
    // 在 DOM 挂载之后执行（与编辑器预览、各阅读器的做法一致）。
    await nextTick()
    await renderMermaidInBody()
  } catch (error) {
    logger.warn('[DocMarkdownReader] render failed', { error: String(error?.message || error) })
    renderedHtml.value = `<pre>${String(error?.message || error)}</pre>`
  }
}

/** 把正文里的 mermaid 占位渲染成 SVG，并挂上复制/查看按钮。 */
const renderMermaidInBody = async () => {
  const host = bodyRef.value
  if (!host) return
  try {
    addCopyButtons(host, { theme: themeName() })
    addMediaViewerButtons(host, { theme: themeName() })
  } catch (error) {
    logger.debug('[DocMarkdownReader] decoration skipped', { error: String(error?.message || error) })
  }

  const nodes = Array.from(host.querySelectorAll('.mermaid')).filter(
    (node) => !node.querySelector('svg')
  )
  if (!nodes.length) return
  try {
    // 动态 import：mermaid 只在文档里真的出现图表时才加载，纯文字文档不付这份成本。
    const mod = await import('mermaid')
    const mermaid = mod.default || mod
    const theme = themeName() === 'dark' ? 'dark' : 'default'
    mermaid.initialize({ startOnLoad: false, theme, securityLevel: 'loose' })
    await mermaid.run({ nodes })
  } catch (error) {
    logger.warn('[DocMarkdownReader] mermaid render failed', { error: String(error?.message || error) })
  }
}

const reload = async (force = false) => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''
  statusText.value = force ? '正在重新转换…' : '正在转换文档…'
  if (force) markdown.value = ''

  try {
    const payload = await invoke('convert_document_to_markdown', {
      path: props.path,
      quality: 'fast',
      mediaRoot: null,
    })
    if (token !== loadToken) return
    if (!payload || typeof payload.markdown !== 'string') {
      throw new Error('转换未返回 Markdown')
    }
    markdown.value = payload.markdown
    engine.value = payload.engine || ''
    await render()
    if (token !== loadToken) return
    loading.value = false
    logger.info('[DocMarkdownReader] converted', {
      path: props.path,
      kind: props.kind,
      engine: engine.value,
      length: markdown.value.length,
    })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocMarkdownReader] convert failed', {
      path: props.path,
      kind: props.kind,
      error: String(error?.message || error),
    })
  }
}

onMounted(() => {
  void reload(false)
})

// 正文挂载（renderedHtml 变化）后补渲染 mermaid。
watch(renderedHtml, () => {
  void renderMermaidInBody()
})

onBeforeUnmount(() => {
  loadToken += 1
})
</script>
