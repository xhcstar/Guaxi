<template>
  <div class="guaxi-doc-reader" data-reader="html">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">HTML</span>
      <span class="guaxi-doc-reader__meta">{{ frameNote }}</span>
      <span class="guaxi-doc-reader__spacer" />
      <label
        class="guaxi-doc-reader__meta"
        style="display: inline-flex; align-items: center; gap: 4px"
        title="默认不执行页面脚本；仅在你信任该文件时勾选"
      >
        <input v-model="allowScripts" type="checkbox" aria-label="允许执行页面脚本" />
        允许脚本
      </label>
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

    <div class="guaxi-doc-reader__body" style="overflow: hidden">
      <div v-if="loading" class="guaxi-doc-reader__center">正在读取 HTML…</div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
      <iframe
        v-show="!loading && !errorText"
        ref="frameRef"
        class="gdr-html"
        :sandbox="allowScripts ? 'allow-scripts' : ''"
        referrerpolicy="no-referrer"
      />
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocHtmlReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
})

const frameRef = ref(null)
const loading = ref(true)
const errorText = ref('')
const allowScripts = ref(false)

const isTauriRuntime = (() => {
  try {
    return !!window.__TAURI_INTERNALS__
  } catch {
    return false
  }
})()

const frameNote = computed(() => {
  if (!isTauriRuntime) return '桌面端可完整渲染（当前为浏览器预览）'
  return allowScripts.value ? '脚本已启用（隔离沙箱内执行）' : '脚本已禁用'
})

let blobUrl = ''
let loadToken = 0

const revokeBlob = () => {
  if (blobUrl) {
    try {
      URL.revokeObjectURL(blobUrl)
    } catch {
      /* 忽略 */
    }
    blobUrl = ''
  }
}

/**
 * 用 blob URL 而不是 srcdoc 承载 HTML。
 *
 * 原因与 GuaxiNotebookHtmlOutput 一致：release 版 WebView2 里 srcdoc 会继承主页面 CSP，
 * 导致文档内联样式/图片被拦；blob URL 没有这个限制，且不会因为内容过长而截断。
 *
 * 沙箱策略：默认完全 sandbox（无 scripts），需要交互再勾选「允许脚本」。
 * 始终不开放 allow-same-origin —— 否则 HTML 文档能拿到与主窗口同源的权限。
 */
const load = async () => {
  const token = ++loadToken
  loading.value = true
  errorText.value = ''
  revokeBlob()

  try {
    let html = ''
    if (isTauriRuntime) {
      const bytes = await invoke('read_file_content', { path: props.path })
      html = String(bytes || '')
    } else {
      // 浏览器预览环境没有 IPC：直接按静态资源取。
      const res = await fetch(('file://' + String(props.path)).split(String.fromCharCode(92)).join('/'))
      html = await res.text()
    }
    if (token !== loadToken) return
    if (!html.trim()) throw new Error('HTML 文件内容为空')

    const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
    blobUrl = URL.createObjectURL(blob)
    if (frameRef.value) frameRef.value.src = blobUrl
    loading.value = false
    logger.info('[DocHtmlReader] loaded', { path: props.path, length: html.length })
  } catch (error) {
    if (token !== loadToken) return
    loading.value = false
    errorText.value = String(error?.message || error || error)
    logger.warn('[DocHtmlReader] load failed', {
      path: props.path,
      error: String(error?.message || error),
    })
  }
}

const reload = () => {
  void load()
}

onMounted(() => {
  void load()
})

onBeforeUnmount(() => {
  loadToken += 1
  revokeBlob()
})

watch(
  () => props.path,
  () => {
    void load()
  }
)

// 切换沙箱策略后必须重新导航，浏览器不会对已加载的文档重新计算 sandbox。
watch(allowScripts, () => {
  void load()
})
</script>
