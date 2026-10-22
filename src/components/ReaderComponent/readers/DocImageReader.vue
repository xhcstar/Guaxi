<template>
  <div class="guaxi-doc-reader" :data-reader="`image-${variant}`">
    <div class="guaxi-doc-reader__bar">
      <span class="guaxi-doc-reader__title" :title="path">{{ name }}</span>
      <span class="guaxi-doc-reader__badge">{{ dimsLabel }}</span>
      <span v-if="mime" class="guaxi-doc-reader__badge">{{ mime }}</span>
      <span class="guaxi-doc-reader__spacer" />
      <ZoomControl v-model="scale" :ratio="1.2" :disabled="loading || errorText" />
      <span class="guaxi-doc-reader__divider" />
      <button
        class="guaxi-doc-reader__icon-btn"
        title="适应窗口（0）"
        aria-label="适应窗口"
        :disabled="loading || errorText"
        @click="fitToWindow"
      >
        <Icon :icon="maximizeIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="原始大小（1）"
        aria-label="原始大小"
        :disabled="loading || errorText"
        @click="actualSize"
      >
        <Icon :icon="scanIcon" width="15" height="15" />
      </button>
      <span class="guaxi-doc-reader__divider" />
      <button
        class="guaxi-doc-reader__icon-btn"
        title="向左旋转（R）"
        aria-label="向左旋转"
        :disabled="loading || errorText"
        @click="rotate(-90)"
      >
        <Icon :icon="rotateLeftIcon" width="15" height="15" />
      </button>
      <button
        class="guaxi-doc-reader__icon-btn"
        title="向右旋转（Shift+R）"
        aria-label="向右旋转"
        :disabled="loading || errorText"
        @click="rotate(90)"
      >
        <Icon :icon="rotateRightIcon" width="15" height="15" />
      </button>
    </div>

    <div
      ref="stageRef"
      class="guaxi-doc-reader__body gdr-image-stage"
      :class="{ 'is-panning': panning }"
      @wheel.prevent="onWheel"
      @mousedown="onPanStart"
      @dblclick="toggleFit"
    >
      <div v-if="loading" class="guaxi-doc-reader__center">正在加载图片…</div>
      <div v-else-if="errorText" class="guaxi-doc-reader__center">
        <div class="guaxi-doc-reader__error">{{ errorText }}</div>
      </div>
      <div v-show="!loading && !errorText" class="gdr-image-canvas" :style="canvasStyle">
        <img
          ref="imgRef"
          :src="imageSrc"
          :alt="name"
          draggable="false"
          @load="onImageLoad"
          @error="onImageError"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import maximizeIcon from '@iconify-icons/lucide/maximize-2'
import scanIcon from '@iconify-icons/lucide/scan'
import rotateLeftIcon from '@iconify-icons/lucide/rotate-ccw'
import rotateRightIcon from '@iconify-icons/lucide/rotate-cw'
import ZoomControl from './ZoomControl.vue'
import { createLogger } from '../../../utils/logger'
import './readerPane.css'

const logger = createLogger('DocImageReader')

const props = defineProps({
  path: { type: String, required: true },
  name: { type: String, default: '' },
  mime: { type: String, default: '' },
  // gallery：来自图片集（可左右切换）。单图打开时为 single。
  variant: { type: String, default: 'single' },
})

const emit = defineEmits(['navigate'])

const stageRef = ref(null)
const imgRef = ref(null)
const loading = ref(true)
const errorText = ref('')
const scale = ref(1)
const rotation = ref(0)
const fitMode = ref('fit')
const natural = ref({ w: 0, h: 0 })
const translate = ref({ x: 0, y: 0 })
const panning = ref(false)

const imageSrc = ref('')
let panState = null

const displayName = computed(() => props.name || String(props.path || '').split(/[\\/]/).pop() || '图片')

const dimsLabel = computed(() => {
  if (!natural.value.w || !natural.value.h) return ''
  const angle = ((rotation.value % 360) + 360) % 360
  const swapped = angle === 90 || angle === 270
  const w = swapped ? natural.value.h : natural.value.w
  const h = swapped ? natural.value.w : natural.value.h
  return `${w} × ${h}`
})

const canvasStyle = computed(() => {
  const angle = rotation.value
  // 旋转用 CSS transform 叠在缩放之上；为了让旋转后仍居中，在 rotate 外层再包一层位移。
  return {
    transform: `translate(${translate.value.x}px, ${translate.value.y}px) scale(${scale.value}) rotate(${angle}deg)`,
  }
})

const loadImage = async () => {
  loading.value = true
  errorText.value = ''
  try {
    imageSrc.value = convertFileSrc(props.path)
  } catch (error) {
    // convertFileSrc 不可用时退回 file://（浏览器预览环境常见）。
    logger.warn('[DocImageReader] convertFileSrc failed, fallback to file://', error)
    imageSrc.value = `file://${String(props.path).replace(/\\/g, '/')}`
  }
}

const onImageLoad = () => {
  const el = imgRef.value
  if (el) {
    natural.value = { w: el.naturalWidth || 0, h: el.naturalHeight || 0 }
  }
  loading.value = false
  nextTick(() => fitToWindow())
}

const onImageError = () => {
  loading.value = false
  errorText.value = `无法加载图片：${props.path}`
  logger.warn('[DocImageReader] image load failed', { path: props.path, src: imageSrc.value })
}

/** 适应窗口：等比缩放到完整可见，并居中。 */
const fitToWindow = () => {
  const stage = stageRef.value
  if (!stage || !natural.value.w || !natural.value.h) return
  const padding = 24
  const rect = stage.getBoundingClientRect()
  const availW = Math.max(40, rect.width - padding * 2)
  const availH = Math.max(40, rect.height - padding * 2)
  const angle = ((rotation.value % 360) + 360) % 360
  const swapped = angle === 90 || angle === 270
  const w = swapped ? natural.value.h : natural.value.w
  const h = swapped ? natural.value.w : natural.value.h
  fitMode.value = 'fit'
  scale.value = Math.min(1, availW / w, availH / h) || 1
  translate.value = { x: 0, y: 0 }
}

const actualSize = () => {
  fitMode.value = 'actual'
  scale.value = 1
  translate.value = { x: 0, y: 0 }
}

const toggleFit = () => {
  if (fitMode.value === 'fit') actualSize()
  else fitToWindow()
}

/** 以视口中心为锚点缩放。 */
const zoomBy = (factor) => {
  const next = Math.min(16, Math.max(0.05, scale.value * factor))
  if (next === scale.value) return
  const stage = stageRef.value
  const anchorX = (stage?.clientWidth || 0) / 2
  const anchorY = (stage?.clientHeight || 0) / 2
  applyZoomAt(next, anchorX, anchorY)
  fitMode.value = 'manual'
}

const applyZoomAt = (nextScale, anchorX, anchorY) => {
  const ratio = nextScale / scale.value
  translate.value = {
    x: anchorX - (anchorX - translate.value.x) * ratio,
    y: anchorY - (anchorY - translate.value.y) * ratio,
  }
  scale.value = nextScale
}

const onWheel = (event) => {
  if (loading.value || errorText.value) return
  const stage = stageRef.value
  if (!stage) return
  const rect = stage.getBoundingClientRect()
  const anchorX = event.clientX - rect.left
  const anchorY = event.clientY - rect.top
  // 触控板 pinch 会给出 ctrlKey=true 的小 delta，直接按比例缩放更跟手。
  const factor = event.ctrlKey ? Math.exp(-event.deltaY * 0.01) : event.deltaY < 0 ? 1.1 : 1 / 1.1
  const next = Math.min(16, Math.max(0.05, scale.value * factor))
  applyZoomAt(next, anchorX, anchorY)
  fitMode.value = 'manual'
}

const rotate = (delta) => {
  rotation.value = (rotation.value + delta + 360) % 360
  nextTick(() => {
    if (fitMode.value === 'fit') fitToWindow()
  })
}

const onPanStart = (event) => {
  if (event.button !== 0 || loading.value || errorText.value) return
  panState = {
    startX: event.clientX,
    startY: event.clientY,
    originX: translate.value.x,
    originY: translate.value.y,
  }
  panning.value = true
  window.addEventListener('mousemove', onPanMove)
  window.addEventListener('mouseup', onPanEnd)
}

const onPanMove = (event) => {
  if (!panState) return
  translate.value = {
    x: panState.originX + (event.clientX - panState.startX),
    y: panState.originY + (event.clientY - panState.startY),
  }
}

const onPanEnd = () => {
  panState = null
  panning.value = false
  window.removeEventListener('mousemove', onPanMove)
  window.removeEventListener('mouseup', onPanEnd)
}

const onKeydown = (event) => {
  if (event.target && /input|textarea/i.test(event.target.tagName || '')) return
  const key = String(event.key || '')
  if (key === '+' || key === '=') {
    event.preventDefault()
    zoomBy(1.2)
  } else if (key === '-' || key === '_') {
    event.preventDefault()
    zoomBy(1 / 1.2)
  } else if (key === '0') {
    event.preventDefault()
    fitToWindow()
  } else if (key === '1') {
    event.preventDefault()
    actualSize()
  } else if (key.toLowerCase() === 'r') {
    event.preventDefault()
    rotate(event.shiftKey ? 90 : -90)
  } else if (key === 'ArrowLeft') {
    emit('navigate', -1)
  } else if (key === 'ArrowRight') {
    emit('navigate', 1)
  }
}

let resizeObserver = null

onMounted(async () => {
  await loadImage()
  window.addEventListener('keydown', onKeydown)
  if (typeof ResizeObserver !== 'undefined' && stageRef.value) {
    resizeObserver = new ResizeObserver(() => {
      if (fitMode.value === 'fit') fitToWindow()
    })
    resizeObserver.observe(stageRef.value)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  onPanEnd()
  resizeObserver?.disconnect()
  resizeObserver = null
})

watch(
  () => props.path,
  async () => {
    natural.value = { w: 0, h: 0 }
    rotation.value = 0
    translate.value = { x: 0, y: 0 }
    fitMode.value = 'fit'
    await loadImage()
  }
)
</script>
