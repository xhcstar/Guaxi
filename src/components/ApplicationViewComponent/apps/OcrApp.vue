<template>
  <div class="ocr-root">
    <div class="ocr-header">
      <div class="ocr-title">图片文字识别 (OCR)</div>
      <div class="ocr-sub">离线识别 · 中文 / 多语言 · 基于 PP-OCR</div>
    </div>

    <div class="ocr-main">
      <!-- 左侧：图片来源与预览 -->
      <div class="ocr-panel ocr-source">
        <div class="panel-label">图片</div>

        <div
          class="ocr-dropzone"
          :class="{ 'has-image': !!previewSrc, 'dragging': dragging }"
          @click="pickImage"
          @dragover.prevent="dragging = true"
          @dragleave.prevent="dragging = false"
          @drop.prevent="onDrop"
          @paste="onPaste"
          tabindex="0"
        >
          <img v-if="previewSrc" :src="previewSrc" class="ocr-preview" alt="预览" />
          <div v-else class="ocr-dropzone-hint">
            <div class="ocr-dropzone-icon">🖼️</div>
            <div>点击选择图片，或拖拽 / 粘贴到此处</div>
            <div class="ocr-dropzone-formats">支持 PNG / JPG / JPEG / WEBP / BMP</div>
          </div>
        </div>

        <div class="ocr-actions">
          <n-button type="primary" :loading="recognizing" :disabled="!canRecognize" @click="recognize">
            {{ recognizing ? '识别中…' : '开始识别' }}
          </n-button>
          <n-button
            secondary
            :loading="capturing"
            :disabled="recognizing || capturing"
            @click="screenshotRecognize"
          >
            {{ capturing ? '截图中…' : '📷 截图识别' }}
          </n-button>
          <n-button quaternary size="small" :disabled="recognizing || capturing || !previewSrc" @click="clearImage">
            清空
          </n-button>
        </div>
      </div>

      <!-- 右侧：识别结果 -->
      <div class="ocr-panel ocr-result">
        <div class="panel-label-row">
          <span class="panel-label">识别结果</span>
          <div class="ocr-result-tools" v-if="lines.length">
            <span class="ocr-count">{{ lines.length }} 行</span>
            <n-button quaternary size="tiny" @click="copyText">复制</n-button>
          </div>
        </div>

        <n-input
          v-model:value="resultText"
          type="textarea"
          class="ocr-result-text"
          placeholder="识别出的文字会显示在这里，可直接编辑与复制。"
          :autosize="false"
        />

        <div v-if="errorMsg" class="ocr-error">{{ errorMsg }}</div>
        <div v-else-if="recognizing" class="ocr-tip">
          首次识别需加载模型，可能稍慢，请稍候…
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('OcrApp')
const { message } = createDiscreteApi(['message'])

// 图片来源：本地路径优先（体积小、无需 base64），否则用 base64（拖拽/粘贴）。
const imagePath = ref('')
const imageBase64 = ref('')
const previewSrc = ref('')

const recognizing = ref(false)
const capturing = ref(false)
const dragging = ref(false)
const lines = ref([])
const resultText = ref('')
const errorMsg = ref('')

const canRecognize = computed(() => !recognizing.value && !capturing.value && (!!imagePath.value || !!imageBase64.value))

// —— 选择本地图片 ——
const pickImage = async () => {
  if (recognizing.value) return
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }],
    })
    if (!selected) return
    const path = Array.isArray(selected) ? selected[0] : selected
    setFromPath(path)
  } catch (e) {
    logger.error('选择图片失败:', e)
    message.error('选择图片失败')
  }
}

const setFromPath = (path) => {
  imagePath.value = path
  imageBase64.value = ''
  try {
    previewSrc.value = convertFileSrc(path)
  } catch (e) {
    logger.warn('convertFileSrc 失败:', e)
    previewSrc.value = ''
  }
  resetResult()
}

const setFromBase64 = (dataUrl) => {
  imageBase64.value = dataUrl
  imagePath.value = ''
  previewSrc.value = dataUrl
  resetResult()
}

// —— 拖拽 ——
const onDrop = (evt) => {
  dragging.value = false
  const file = evt.dataTransfer?.files?.[0]
  if (file) readFileAsDataUrl(file)
}

// —— 粘贴 ——
const onPaste = (evt) => {
  const items = evt.clipboardData?.items || []
  for (const item of items) {
    if (item.type && item.type.startsWith('image/')) {
      const file = item.getAsFile()
      if (file) {
        readFileAsDataUrl(file)
        evt.preventDefault()
        return
      }
    }
  }
}

const readFileAsDataUrl = (file) => {
  const reader = new FileReader()
  reader.onload = () => setFromBase64(String(reader.result || ''))
  reader.onerror = () => message.error('读取图片失败')
  reader.readAsDataURL(file)
}

// —— 识别 ——
const recognize = async () => {
  if (!canRecognize.value) return
  recognizing.value = true
  errorMsg.value = ''
  try {
    const res = await invoke('ocr_recognize', {
      imagePath: imagePath.value || null,
      imageBase64: imageBase64.value || null,
    })
    lines.value = Array.isArray(res?.lines) ? res.lines : []
    resultText.value = res?.text || ''
    if (!lines.value.length) {
      message.info('未识别到文字')
    }
  } catch (e) {
    logger.error('OCR 识别失败:', e)
    errorMsg.value = typeof e === 'string' ? e : (e?.message || '识别失败')
    message.error('识别失败')
  } finally {
    recognizing.value = false
  }
}

// —— 截图识别：一键框选屏幕并直接识别 ——
const screenshotRecognize = async () => {
  if (capturing.value || recognizing.value) return
  capturing.value = true
  errorMsg.value = ''
  try {
    const res = await invoke('screenshot_ocr')
    if (res?.cancelled) {
      message.info('已取消截图')
      return
    }
    // 截图以 base64 回传，用于预览；同时清掉本地路径来源
    if (res?.image_base64) {
      imageBase64.value = res.image_base64
      imagePath.value = ''
      previewSrc.value = res.image_base64
    }
    lines.value = Array.isArray(res?.lines) ? res.lines : []
    resultText.value = res?.text || ''
    if (!lines.value.length) {
      message.info('未识别到文字')
    }
  } catch (e) {
    logger.error('截图识别失败:', e)
    errorMsg.value = typeof e === 'string' ? e : (e?.message || '截图识别失败')
    message.error('截图识别失败')
  } finally {
    capturing.value = false
  }
}

const copyText = async () => {
  if (!resultText.value) return
  try {
    await navigator.clipboard.writeText(resultText.value)
    message.success('已复制')
  } catch (e) {
    logger.warn('复制失败:', e)
    message.error('复制失败')
  }
}

const resetResult = () => {
  lines.value = []
  resultText.value = ''
  errorMsg.value = ''
}

const clearImage = () => {
  imagePath.value = ''
  imageBase64.value = ''
  previewSrc.value = ''
  resetResult()
}
</script>

<style scoped>
.ocr-root {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 16px 18px;
  box-sizing: border-box;
}

.ocr-header {
  margin-bottom: 14px;
}

.ocr-title {
  font-size: 18px;
  font-weight: 600;
}

.ocr-sub {
  font-size: 12px;
  color: #6b7280;
  margin-top: 2px;
}

.ocr-main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.ocr-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  gap: 8px;
}

.panel-label {
  font-size: 12px;
  color: #6b7280;
}

.panel-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ocr-result-tools {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ocr-count {
  font-size: 12px;
  color: #9ca3af;
}

/* 拖拽区 */
.ocr-dropzone {
  flex: 1;
  min-height: 0;
  border: 1.5px dashed var(--guaxi-border-color, #d1d5db);
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  overflow: hidden;
  transition: border-color 0.15s, background 0.15s;
  background: var(--guaxi-app-bg, #fafafa);
}

.ocr-dropzone:hover,
.ocr-dropzone.dragging {
  border-color: #3b82f6;
  background: rgba(59, 130, 246, 0.05);
}

.ocr-dropzone.has-image {
  border-style: solid;
}

.ocr-preview {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.ocr-dropzone-hint {
  text-align: center;
  color: #9ca3af;
  font-size: 13px;
  padding: 20px;
}

.ocr-dropzone-icon {
  font-size: 32px;
  margin-bottom: 8px;
}

.ocr-dropzone-formats {
  font-size: 11px;
  color: #b8bfc9;
  margin-top: 6px;
}

.ocr-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.ocr-result-text {
  flex: 1;
  min-height: 0;
}

.ocr-result-text :deep(.n-input__textarea-el) {
  height: 100% !important;
  resize: none;
}

.ocr-error {
  font-size: 12px;
  color: #ef4444;
  white-space: pre-wrap;
  max-height: 80px;
  overflow: auto;
}

.ocr-tip {
  font-size: 12px;
  color: #9ca3af;
}
</style>
