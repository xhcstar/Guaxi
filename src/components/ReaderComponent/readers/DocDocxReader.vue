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

    <div ref="bodyRef" class="guaxi-doc-reader__body">
      <div v-if="loading" class="guaxi-doc-reader__center">正在解析文档…</div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
      <div v-show="!loading && !errorText" class="gdr-docx" :style="zoomStyle">
        <div ref="hostRef" />
      </div>
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

const hostRef = ref(null)
const bodyRef = ref(null)
const loading = ref(true)
const errorText = ref('')
const zoom = ref(1)

const zoomStyle = computed(() => ({
  transform: `scale(${zoom.value})`,
  transformOrigin: 'top center',
  width: `${100 / zoom.value}%`,
}))

let loadToken = 0

const zoomBy = (factor) => {
  zoom.value = Math.min(3, Math.max(0.4, +(zoom.value * factor).toFixed(3)))
}

/**
 * docx-preview 的 renderAsync 需要 ArrayBuffer / Uint8Array / Blob。
 * 后端 `read_word_file` 返回的是字节数组（既有命令，未改动）。
 *
 * 这里只支持 .docx（OOXML）。.doc / .rtf / .odt 由 ReaderPane 走「转换后只读 Markdown」分支，
 * 不会进到本组件。
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
    const host = hostRef.value
    if (!host) throw new Error('预览容器尚未就绪')
    host.innerHTML = ''

    await renderAsync(new Uint8Array(bytes), host, undefined, {
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
    logger.info('[DocDocxReader] rendered', { path: props.path, nodes: host.childElementCount })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocDocxReader] render failed', { path: props.path, error: String(error?.message || error) })
  }
}

onMounted(loadDocument)

onBeforeUnmount(() => {
  // 递增 token 让在途渲染的结果被丢弃，避免卸载后写 DOM。
  loadToken += 1
})

watch(() => props.path, loadDocument)
</script>
