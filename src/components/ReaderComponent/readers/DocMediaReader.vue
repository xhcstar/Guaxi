<template>
  <div class="guaxi-doc-reader" data-reader="media">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">{{ isVideo ? 'VIDEO' : 'AUDIO' }}</span>
      <span class="guaxi-doc-reader__spacer" />
      <button class="guaxi-doc-reader__btn" title="在系统播放器中打开" @click="openExternally">系统打开</button>
    </div>

    <div class="guaxi-doc-reader__body" style="overflow: hidden">
      <div class="gdr-media">
        <video
          v-if="isVideo"
          ref="videoRef"
          :src="mediaSrc"
          controls
          preload="metadata"
          @error="onError"
        />
        <audio
          v-else
          ref="audioRef"
          :src="mediaSrc"
          controls
          preload="metadata"
          @error="onError"
        />
      </div>
      <div v-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { openPath } from '@tauri-apps/plugin-opener'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocMediaReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  kind: { type: String, default: 'video' },
})

const videoRef = ref(null)
const audioRef = ref(null)
const errorText = ref('')
const mediaSrc = ref('')

const isVideo = computed(() => String(props.kind || '').toLowerCase() === 'video' ||
  /\.(mp4|mkv|webm|avi|mov|m4v|wmv|flv)$/i.test(props.path))

const resolveSrc = () => {
  try {
    mediaSrc.value = convertFileSrc(props.path)
  } catch (error) {
    logger.warn('[DocMediaReader] convertFileSrc failed', error)
    mediaSrc.value = `file://${String(props.path).replace(/\\/g, '/')}`
  }
}

const onError = () => {
  errorText.value = `无法播放：${props.path}\n（WebView2 内置解码器可能不支持该编码）`
  logger.warn('[DocMediaReader] media element error', { path: props.path })
}

const openExternally = async () => {
  try {
    await openPath(props.path)
  } catch (error) {
    logger.warn('[DocMediaReader] openPath failed', { error: String(error?.message || error) })
    errorText.value = `无法用系统播放器打开：${String(error?.message || error)}`
  }
}

onMounted(() => {
  resolveSrc()
})

onBeforeUnmount(() => {
  // 停止解码，避免切走后仍在后台占用解码器。
  try {
    videoRef.value?.pause?.()
    audioRef.value?.pause?.()
  } catch {
    /* 忽略 */
  }
})
</script>
