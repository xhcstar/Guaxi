<template>
  <!--
    独立文档阅读器窗口。

    为什么单独一个窗口，而不是继续挂在编辑器里：
    - 阅读器要长出搜索、书签、批注、Office 编辑这些能力，继续挤在编辑器标签页里，
      每加一个功能都得先证明「不会碰到 Monaco 与自动保存链路」；
    - 独立窗口有自己的进程与组件树，阅读器的任何改动都不可能影响主编辑器。

    本页只做三件事：解析窗口参数 → 判定文件类型 → 交给既有的 ReaderPane。
    ReaderPane 内部的分发逻辑完全复用，因此「哪个类型用哪个阅读器」只有一处定义。
  -->
  <div class="doc-reader-window" :data-theme="normalizedTheme">
    <div v-if="loading" class="doc-reader-window__center">
      <div>正在打开文档…</div>
      <div class="doc-reader-window__hint">{{ fileName || path }}</div>
    </div>

    <div v-else-if="errorText" class="doc-reader-window__center">
      <div class="doc-reader-window__error">{{ errorText }}</div>
      <div class="doc-reader-window__hint">{{ path }}</div>
    </div>

    <!-- ReaderPane 用 position:absolute; inset:0 铺满定位父容器，这里必须给它一个。 -->
    <div v-else class="doc-reader-window__stage">
      <ReaderPane
        :target="target"
        :path="path"
        :file-name="fileName"
        :theme="normalizedTheme"
        @force-editor="onForceEditor"
        @convert-to-markdown="onConvertToMarkdown"
      />
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'
import { resolveOpenTarget, READER_KIND } from '../../utils/openTargetKind'
import { useSettingsStore } from '../../stores/settingsStore'
import ReaderPane from '../HomeViewComponent/ReaderPane.vue'

const logger = createLogger('DocReaderWindow')

const props = defineProps({
  path: { type: String, default: '' },
  name: { type: String, default: '' },
  kind: { type: String, default: '' },
  reader: { type: String, default: '' },
  theme: { type: String, default: '' },
})

const settingsStore = useSettingsStore()

const normalizedTheme = computed(() => {
  const fromProps = String(props.theme || '').trim().toLowerCase()
  if (fromProps === 'dark' || fromProps === 'light') return fromProps
  const fromStore = String(settingsStore.appearanceTheme || '').trim().toLowerCase()
  if (fromStore === 'dark' || fromStore === 'light') return fromStore
  return 'light'
})

const loading = ref(true)
const errorText = ref('')
const target = ref(null)

const path = computed(() => String(props.path || '').trim())
const fileName = computed(() => {
  const fromProps = String(props.name || '').trim()
  if (fromProps) return fromProps
  const normalized = path.value.replace(/\\/g, '/')
  return normalized.split('/').filter(Boolean).pop() || ''
})

/**
 * 解析阅读器描述。
 *
 * 优先用窗口参数里带过来的 kind/reader（主编辑器已经嗅探过一次，省一次后端往返）；
 * 参数缺失时再走一次 resolveOpenTarget —— 这样直接手输 URL 打开窗口也能工作。
 */
const loadTarget = async () => {
  const filePath = path.value
  if (!filePath) {
    errorText.value = '缺少文件路径'
    loading.value = false
    return
  }

  try {
    if (props.reader && props.kind) {
      target.value = {
        kind: props.kind,
        reader: props.reader,
        mime: '',
        size: 0,
        container: false,
      }
    } else {
      const resolved = await resolveOpenTarget(filePath, 'auto')
      if (!resolved || resolved.reader === READER_KIND.TEXT) {
        throw new Error('该文件不是可预览的文档类型')
      }
      target.value = resolved
    }

    // 窗口标题跟随文档名，用户在任务栏里也能分辨。
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      await getCurrentWindow().setTitle(fileName.value || '文档阅读器')
    } catch {
      /* 非 Tauri 环境（浏览器调试）忽略 */
    }

    logger.info('[DocReaderWindow] target resolved', {
      path: filePath,
      kind: target.value.kind,
      reader: target.value.reader,
    })
  } catch (error) {
    errorText.value = String(error?.message || error)
    logger.warn('[DocReaderWindow] resolve failed', {
      path: filePath,
      error: String(error?.message || error),
    })
  } finally {
    loading.value = false
  }
}

/**
 * 「用编辑器打开」在这个窗口里没有编辑器可切。
 *
 * 与其做一个假的本地切换（那会变成第二个编辑器），不如把用户送回主界面 ——
 * 主编辑器有完整能力，且这条路径本来就有（force-editor 的原语义）。
 */
const onForceEditor = async () => {
  logger.info('[DocReaderWindow] force editor requested, delegating to main window', { path: path.value })
  try {
    const { emit } = await import('@tauri-apps/api/event')
    await emit('doc-reader:open-in-editor', { path: path.value })
  } catch (error) {
    logger.warn('[DocReaderWindow] emit open-in-editor failed', {
      error: String(error?.message || error),
    })
  }
}

/** 「转换为 Markdown」同理：转换与打开标签都是主编辑器的职责。 */
const onConvertToMarkdown = async () => {
  logger.info('[DocReaderWindow] convert requested, delegating to main window', { path: path.value })
  try {
    const { emit } = await import('@tauri-apps/api/event')
    await emit('doc-reader:convert-to-markdown', { path: path.value })
  } catch (error) {
    logger.warn('[DocReaderWindow] emit convert failed', {
      error: String(error?.message || error),
    })
  }
}

const onKeydown = (event) => {
  if (event.key === 'Escape') {
    event.preventDefault()
    // Esc 关闭窗口：独立阅读器的常见预期。
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().close())
      .catch(() => {})
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown)
  try {
    await settingsStore.loadAppearanceConfig()
  } catch (error) {
    logger.warn('[DocReaderWindow] loadAppearanceConfig failed', {
      error: String(error?.message || error),
    })
  }
  await loadTarget()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
.doc-reader-window {
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}

.doc-reader-window__stage {
  /* ReaderPane 以 absolute inset:0 铺满，父级必须有定位上下文与确定尺寸。 */
  position: relative;
  flex: 1;
  min-height: 0;
}

.doc-reader-window__center {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px;
  text-align: center;
}

.doc-reader-window__hint {
  font-size: 12px;
  opacity: 0.65;
  word-break: break-all;
}

.doc-reader-window__error {
  color: #e06c75;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
