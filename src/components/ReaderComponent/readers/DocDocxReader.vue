<template>
  <div class="guaxi-doc-reader" data-reader="word">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">{{ kind.toUpperCase() }}</span>
      <span class="guaxi-doc-reader__spacer" />
      <button class="guaxi-doc-reader__btn" title="缩小" :disabled="loading || !!errorText" @click="zoomBy(1 / 1.15)">−</button>
      <span class="guaxi-doc-reader__meta" style="min-width: 46px; text-align: center">{{ Math.round(zoom * 100) }}%</span>
      <button class="guaxi-doc-reader__btn" title="放大" :disabled="loading || !!errorText" @click="zoomBy(1.15)">＋</button>
      <button class="guaxi-doc-reader__btn" title="重置缩放" :disabled="loading || !!errorText" @click="zoom = 1">100%</button>
    </div>

    <div ref="bodyRef" class="guaxi-doc-reader__body gdr-docx-body">
      <div v-if="loading" class="guaxi-doc-reader__center">正在解析文档…</div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
      <!--
        正文与样式都放进 Shadow DOM。

        docx-preview 生成的是**无前缀的裸选择器**：`p.MsoNormal`、`.docx table td {}`，
        甚至 `p { margin: 0 }` 这类全局标签规则。它们一旦注入页面 <head>，就会把整个
        应用改形 —— 实测表现就是打开 Word 后主界面被一个巨大黑块覆盖。

        Shadow DOM 让文档样式只能作用于宿主内部：既解决了「污染全局」，也解决了
        「样式被外层布局裁剪导致没有格式」——正文与样式在同一棵影子树里。
      -->
      <div
        ref="shadowHostRef"
        v-show="!loading && !errorText"
        class="gdr-docx"
        :style="zoomStyle"
      />
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { renderAsync } from 'docx-preview'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocDocxReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'docx' },
})

const bodyRef = ref(null)
const shadowHostRef = ref(null)
const loading = ref(true)
const errorText = ref('')
const zoom = ref(1)

/**
 * 缩放样式。
 *
 * 用 CSS `zoom` 而不是 `transform: scale`：
 * - `transform` 不参与布局，缩放后可视区域与滚动条不对应（放大后右侧内容滚不到）；
 * - 早先还额外写了 `width: 100/zoom` 去补偿，但 transform 根本不改宽度，
 *   这层补偿只会让内容在放大时横向溢出、缩小时留下空白 —— 正是「缩放状态不对」的表现。
 * - `zoom` 直接改布局尺寸，滚动、命中、文本换行都跟着一起缩放。
 *
 * WebView2（Chromium）与 Safari 都支持；Firefox 也已从 126 起支持。
 */
const zoomStyle = computed(() => ({ zoom: zoom.value }))

/** 当前挂载的影子根；换文档/卸载时整体丢弃，不做增量清理。 */
let shadowRoot = null
let loadToken = 0

const zoomBy = (factor) => {
  zoom.value = Math.min(3, Math.max(0.4, +(zoom.value * factor).toFixed(3)))
}

/**
 * 准备一个干净的 Shadow DOM 容器。
 *
 * `mode: 'open'` 只影响能否从外部脚本访问影子树，样式隔离效果与 closed 相同。
 * 选 open 是为了将来做文本选择/搜索时还能访问到内容。
 */
const ensureShadowRoot = () => {
  const host = shadowHostRef.value
  if (!host) return null
  // 同一个宿主不能 attachShadow 两次：换文档时先清空旧影子树的内容。
  if (!shadowRoot) {
    shadowRoot = host.attachShadow({ mode: 'open' })
  }
  shadowRoot.innerHTML = ''

  // 影子树里的基础排版兜底：docx-preview 默认样式假设宿主已提供白底与页面居中。
  const base = document.createElement('style')
  base.textContent = `
    :host { display: block; }
    .docx-wrapper { background: transparent; padding: 16px; }
    .docx-wrapper > section.docx { margin: 0 auto 16px; }
  `
  shadowRoot.appendChild(base)
  return shadowRoot
}

/**
 * 加载并渲染 .docx。
 *
 * 只支持 OOXML（.doc / .rtf / .odt 由 ReaderPane 走「转换后只读 Markdown」分支）。
 */
const loadDocument = async () => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''
  zoom.value = 1

  try {
    if (props.kind !== 'docx') {
      throw new Error(`当前组件只直接渲染 .docx；.${props.kind} 请使用「转换为 Markdown」查看`)
    }
    const bytes = await invoke('read_word_file', { path: props.path })
    if (token !== loadToken) return
    if (!bytes || !bytes.length) throw new Error('文件内容为空')

    await nextTick()
    const root = ensureShadowRoot()
    if (!root) throw new Error('预览容器尚未就绪')

    // 正文容器与样式容器都落在影子根里：任何一条样式都出不了这个边界。
    const body = document.createElement('div')
    const styleHost = document.createElement('div')
    root.appendChild(styleHost)
    root.appendChild(body)

    await renderAsync(new Uint8Array(bytes), body, styleHost, {
      inWrapper: true,
      ignoreWidth: false,
      ignoreHeight: false,
      breakPages: false,
      ignoreFonts: false,
      renderHeaders: true,
      renderFooters: true,
      renderFootnotes: true,
      renderEndnotes: true,
      experimental: true,
      useBase64URL: true,
    })
    if (token !== loadToken) return
    loading.value = false
    logger.info('[DocDocxReader] rendered in shadow root', {
      path: props.path,
      nodes: body.childElementCount,
    })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocDocxReader] render failed', {
      path: props.path,
      error: String(error?.message || error),
    })
  }
}

onMounted(loadDocument)

onBeforeUnmount(() => {
  // 递增 token 让在途渲染的结果被丢弃，避免卸载后写 DOM。
  loadToken += 1
  shadowRoot = null
})

watch(() => props.path, loadDocument)
</script>
