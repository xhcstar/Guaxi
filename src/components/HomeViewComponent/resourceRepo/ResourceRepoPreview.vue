<template>
  <div class="rrp" :data-kind="kind" :data-state="state">
    <div class="rrp-stage">
      <template v-if="kind === 'image'">
        <img
          v-if="imageSrc"
          class="rrp-image"
          :src="imageSrc"
          :alt="node?.name || ''"
          :style="imageStyle"
          draggable="false"
          @load="onImageLoad"
          @error="onImageError"
        />
      </template>

      <template v-else-if="kind === 'pdf'">
        <img v-if="imageSrc" class="rrp-image" :src="imageSrc" :alt="node?.name || ''" @load="dims = null" />
        <div v-if="pdfNote" class="rrp-badge">{{ pdfNote }}</div>
      </template>

      <template v-else-if="kind === 'audio'">
        <div class="rrp-center">
          <div class="rrp-glyph rrp-glyph-audio">
            <Icon :icon="musicIcon" width="34" height="34" />
          </div>
          <div class="rrp-filename">{{ node?.name }}</div>
          <audio class="rrp-audio" :src="imageSrc" controls preload="metadata" />
        </div>
      </template>

      <template v-else-if="kind === 'video'">
        <video class="rrp-video" :src="imageSrc" controls preload="metadata" />
      </template>

      <template v-else-if="kind === 'table' && !bigFile">
        <div class="rrp-scroll">
          <table class="rrp-table">
            <thead v-if="tableRows.length">
              <tr>
                <th class="rrp-rownum" />
                <th v-for="(cell, i) in tableRows[0]" :key="i">{{ cell }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, r) in tableBody" :key="r">
                <td class="rrp-rownum">{{ r + 2 }}</td>
                <td v-for="(cell, c) in row" :key="c">{{ cell }}</td>
              </tr>
            </tbody>
          </table>
          <div v-if="tableTruncated" class="rrp-note">只显示前 {{ tableBody.length }} 行，完整内容请打开文件。</div>
        </div>
      </template>

      <template v-else-if="kind === 'markdown' && !bigFile">
        <div class="rrp-scroll rrp-doc">
          <!-- 仅渲染资源库里常见的一小撮语法（标题/列表/代码/图片），不引入完整渲染器。 -->
          <div class="rrp-markdown" v-html="markdownHtml" />
          <div v-if="clipped" class="rrp-note">内容较长，预览只显示开头部分。</div>
        </div>
      </template>

      <template v-else-if="kind === 'text' && !bigFile">
        <div class="rrp-scroll">
          <pre class="rrp-pre"><code>{{ textContent }}</code></pre>
          <div v-if="clipped" class="rrp-note">内容较长，预览只显示开头部分。</div>
        </div>
      </template>

      <template v-else-if="kind === 'dir'">
        <div class="rrp-center">
          <div class="rrp-glyph">
            <Icon :icon="folderOpenIcon" width="40" height="40" />
          </div>
          <div class="rrp-filename">{{ node?.name }}</div>
          <div class="rrp-dir-summary">
            <span>{{ dirSummary.files }} 个文件</span>
            <span>{{ dirSummary.dirs }} 个子目录</span>
            <span>{{ humanBytes(dirSummary.size) }}</span>
          </div>
        </div>
      </template>

      <template v-else-if="kind === 'none'">
        <div class="rrp-center">
          <div class="rrp-glyph">
            <Icon :icon="fileIcon" width="36" height="36" />
          </div>
          <div class="rrp-filename">{{ node?.name }}</div>
          <div class="rrp-hint">{{ hintText }}</div>
        </div>
      </template>

      <div v-if="state === 'loading'" class="rrp-veil">
        <span class="rrp-spinner" />
        <span>正在读取…</span>
      </div>
      <div v-else-if="state === 'error'" class="rrp-veil is-error">
        <Icon :icon="alertIcon" width="22" height="22" />
        <span>{{ errorText }}</span>
        <button type="button" class="rrp-link-btn" @click="reload">重试</button>
      </div>
    </div>

    <div v-if="bigFile" class="rrp-big">
      <Icon :icon="alertIcon" width="18" height="18" />
      <div>
        <div class="rrp-big-title">文件较大（{{ humanBytes(node?.size) }}），不在侧栏预览</div>
        <div class="rrp-big-hint">用「打开」或「系统程序」查看完整内容。</div>
      </div>
    </div>

    <div v-if="kind === 'image' && dims" class="rrp-footer">
      <span>{{ dims.width }} × {{ dims.height }}</span>
      <span v-if="megapixels">{{ megapixels }} MP</span>
      <span class="rrp-footer-spacer" />
      <span>{{ extLabel }}</span>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import musicIcon from '@iconify-icons/lucide/music'
import folderOpenIcon from '@iconify-icons/lucide/folder-open'
import fileIcon from '@iconify-icons/lucide/file'
import alertIcon from '@iconify-icons/lucide/alert-triangle'
import { renderMiniMarkdown } from '../../../utils/guaxiMiniMarkdown'
import { createLogger } from '../../../utils/logger'
import {
  PREVIEWABLE_IMAGE_EXTS,
  clipText,
  extOf,
  humanBytes,
  joinPath,
  normalizeRepoPath,
  parseDelimited,
} from './resourceRepoModel'

const logger = createLogger('ResourceRepoPreview')

const props = defineProps({
  node: { type: Object, default: null },
  /** 绝对路径；树里只有库内相对路径。 */
  absolutePath: { type: String, default: '' },
  /** 资源库根目录绝对路径，用于把文档里的相对引用解析回来。 */
  rootPath: { type: String, default: '' },
  /** 目录节点直接给出汇总，避免在预览里重新遍历。 */
  dirSummary: { type: Object, default: () => ({ files: 0, dirs: 0, size: 0 }) },
})

const emit = defineEmits(['dimensions'])

const state = ref('idle')
const bigFile = ref(false)
const errorText = ref('')
const imageSrc = ref('')
const textContent = ref('')
const clipped = ref(false)
const dims = ref(null)
const tableRows = ref([])
const tableTruncated = ref(false)
const pdfNote = ref('')
const markdownHtml = ref('')
const imageStyle = ref({})

let loadToken = 0

/** 内嵌预览的体积上限（字节）。 */
const MAX_INLINE_BYTES = 12 * 1024 * 1024

const ext = computed(() => extOf(props.node?.name || ''))
const extLabel = computed(() => (ext.value ? ext.value.toUpperCase() : '文件'))
const megapixels = computed(() => {
  if (!dims.value) return ''
  const mp = (dims.value.width * dims.value.height) / 1_000_000
  return mp >= 0.1 ? mp.toFixed(mp >= 10 ? 0 : 1) : ''
})

const kind = computed(() => {
  const node = props.node
  if (!node) return 'none'
  if (node.isDir) return 'dir'
  const e = ext.value
  if (e === 'svg' || PREVIEWABLE_IMAGE_EXTS.has(e)) return 'image'
  if (e === 'csv' || e === 'tsv') return 'table'
  if (e === 'md' || e === 'markdown' || e === 'mdx') return 'markdown'
  if (e === 'pdf') return 'pdf'
  if (['mp3', 'wav', 'flac', 'ogg', 'm4a', 'aac', 'opus', 'wma'].includes(e)) return 'audio'
  if (['mp4', 'mkv', 'webm', 'avi', 'mov', 'm4v', 'wmv'].includes(e)) return 'video'
  if (
    [
      'txt', 'text', 'log', 'json', 'jsonl', 'ndjson', 'yaml', 'yml', 'toml', 'ini', 'conf', 'cfg',
      'xml', 'html', 'htm', 'css', 'scss', 'less', 'js', 'mjs', 'cjs', 'ts', 'tsx', 'jsx', 'vue',
      'py', 'rs', 'go', 'java', 'c', 'h', 'cpp', 'hpp', 'cs', 'sh', 'bash', 'ps1', 'bat', 'sql',
      'tex', 'bib', 'org', 'rst', 'mmd', 'mermaid', 'puml', 'dot', 'diff', 'patch', 'srt', 'vtt',
    ].includes(e)
  ) {
    return 'text'
  }
  return 'none'
})

const tableBody = computed(() => tableRows.value.slice(1))

const hintText = computed(() => {
  const e = ext.value
  if (!e) return '这个文件没有可内嵌的预览。'
  if (['doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'epub', 'zip', 'rar', '7z'].includes(e)) {
    return '这类文件需要在专用阅读器里打开。'
  }
  return `${e.toUpperCase()} 文件没有内嵌预览，可以用系统程序打开。`
})

const safeConvert = (path) => {
  try {
    return convertFileSrc(path)
  } catch (e) {
    logger.warn('[Preview] convertFileSrc failed', e)
    return ''
  }
}

const reset = () => {
  state.value = 'idle'
  errorText.value = ''
  imageSrc.value = ''
  textContent.value = ''
  markdownHtml.value = ''
  tableRows.value = []
  tableTruncated.value = false
  pdfNote.value = ''
  dims.value = null
  clipped.value = false
  bigFile.value = false
  imageStyle.value = {}
}

const textExts = new Set([
  'txt', 'text', 'log', 'json', 'jsonl', 'ndjson', 'yaml', 'yml', 'toml', 'ini', 'conf', 'cfg',
  'xml', 'html', 'htm', 'css', 'scss', 'less', 'js', 'mjs', 'cjs', 'ts', 'tsx', 'jsx', 'vue',
  'py', 'rs', 'go', 'java', 'c', 'h', 'cpp', 'hpp', 'cs', 'sh', 'bash', 'ps1', 'bat', 'sql',
  'tex', 'bib', 'org', 'rst', 'mmd', 'mermaid', 'puml', 'dot', 'diff', 'patch', 'srt', 'vtt',
])

const resolveRelative = (raw) => {
  const value = String(raw || '').trim()
  if (!value) return ''
  if (/^(https?:|data:|blob:|asset:)/i.test(value)) return value
  if (/^[a-zA-Z]:[\\/]/.test(value) || value.startsWith('\\\\')) return safeConvert(value)
  const dir = props.node ? normalizeRepoPath(props.node.path).split('/').slice(0, -1).join('/') : ''
  const rel = value.startsWith('/') ? value.slice(1) : joinPath(dir, value)
  return safeConvert(joinPath(props.rootPath, rel))
}

const populateMarkdown = (text) => {
  const { text: body, clipped: wasClipped } = clipText(text, 120_000)
  clipped.value = wasClipped
  markdownHtml.value = renderMiniMarkdown(body, resolveRelative)
}

const load = async () => {
  const token = ++loadToken
  reset()
  const node = props.node
  if (!node) return

  if (node.isDir) {
    state.value = 'ready'
    return
  }
  const e = ext.value
  const abs = props.absolutePath

  if (!abs) {
    state.value = 'error'
    errorText.value = '缺少文件路径'
    return
  }

  if (kind.value === 'image') {
    imageSrc.value = safeConvert(abs)
    if (!imageSrc.value) {
      state.value = 'error'
      errorText.value = '无法生成图片地址'
      return
    }
    state.value = 'loading'
    return
  }

  if (kind.value === 'audio' || kind.value === 'video') {
    imageSrc.value = safeConvert(abs)
    state.value = imageSrc.value ? 'ready' : 'error'
    if (!imageSrc.value) errorText.value = '无法生成媒体地址'
    return
  }

  // 几十 MB 的日志/CSV 读进内存再截断会卡住整个窗口；超过阈值就只给提示，
  // 让用户用「打开」或系统程序查看完整内容。
  const size = Number(node.size) || 0
  if (size > MAX_INLINE_BYTES && (kind.value === 'text' || kind.value === 'markdown' || kind.value === 'table')) {
    state.value = 'ready'
    textContent.value = ''
    errorText.value = ''
    bigFile.value = true
    return
  }
  bigFile.value = false

  state.value = 'loading'
  try {
    if (kind.value === 'pdf') {
      const resp = await invoke('pdf_render_page', {
        request: { path: abs, page: 1, dpi: 110 },
      })
      if (token !== loadToken) return
      imageSrc.value = safeConvert(resp.image_path)
      pdfNote.value = '首页预览'
      state.value = 'ready'
      return
    }

    if (kind.value === 'table') {
      const raw = await invoke('read_text_file', { path: abs })
      if (token !== loadToken) return
      const delimiter = e === 'tsv' ? '\t' : ','
      const rows = parseDelimited(raw, delimiter, 201)
      tableTruncated.value = rows.length > 200
      tableRows.value = rows.slice(0, 200)
      state.value = 'ready'
      return
    }

    if (kind.value === 'markdown') {
      const raw = await invoke('read_text_file', { path: abs })
      if (token !== loadToken) return
      populateMarkdown(raw)
      state.value = 'ready'
      return
    }

    if (kind.value === 'text' && textExts.has(e)) {
      const raw = await invoke('read_text_file', { path: abs })
      if (token !== loadToken) return
      const body = e === 'json' || e === 'jsonl' ? prettyJson(raw) : raw
      const clippedResult = clipText(body, 80_000)
      textContent.value = clippedResult.text
      clipped.value = clippedResult.clipped
      state.value = 'ready'
      return
    }

    state.value = 'ready'
  } catch (error) {
    if (token !== loadToken) return
    logger.warn('[Preview] load failed', { path: abs, kind: kind.value, error: String(error?.message || error) })
    state.value = 'error'
    errorText.value = String(error?.message || error || '读取失败')
  }
}

const prettyJson = (raw) => {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

const reload = () => load()

const onImageLoad = (event) => {
  const el = event?.target
  const width = Number(el?.naturalWidth || 0)
  const height = Number(el?.naturalHeight || 0)
  state.value = 'ready'
  if (!width || !height) return
  dims.value = { width, height }
  // 竖图在窄侧栏里按高度适配，横图按宽度适配；等比不变形。
  const ratio = width / height
  imageStyle.value = ratio >= 1.15
    ? { width: '100%', height: 'auto' }
    : ratio <= 0.87
      ? { height: '100%', width: 'auto', maxWidth: '100%' }
      : { maxWidth: '100%', maxHeight: '100%' }
  emit('dimensions', dims.value)
}

const onImageError = () => {
  state.value = 'error'
  errorText.value = '图片无法解码，可能已损坏'
}

watch(() => [props.node?.path, props.absolutePath], load, { immediate: true })

onBeforeUnmount(() => {
  loadToken += 1
})

defineExpose({ reload })
</script>

<style scoped>
.rrp {
  --rrp-stage-bg: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 4%, transparent);
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.rrp-stage {
  position: relative;
  flex: 1 1 auto;
  min-height: 72px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 10px;
  overflow: hidden;
  background: var(--rrp-stage-bg);
  border-radius: 10px;
  border: 1px solid color-mix(in srgb, var(--guaxi-border, #d0d7de) 60%, transparent);
}

.rrp-image {
  display: block;
  object-fit: contain;
  border-radius: 6px;
  max-width: 100%;
  max-height: 100%;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.16);
}

.rrp-video {
  width: 100%;
  max-height: 100%;
  border-radius: 8px;
  background: #000;
}

.rrp-audio {
  width: 100%;
  margin-top: 12px;
}

.rrp-center {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  text-align: center;
  padding: 18px;
  max-width: 100%;
}

.rrp-glyph {
  width: 64px;
  height: 64px;
  display: grid;
  place-items: center;
  border-radius: 16px;
  color: var(--guaxi-text-secondary, #57606a);
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 6%, transparent);
}

.rrp-glyph-audio {
  color: #56b6c2;
}

.rrp-filename {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.4;
  word-break: break-all;
  max-width: 100%;
}

.rrp-dir-summary {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 6px 10px;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #57606a);
}

.rrp-hint {
  font-size: 12px;
  line-height: 1.5;
  color: var(--guaxi-text-secondary, #57606a);
}

.rrp-scroll {
  align-self: stretch;
  flex: 1;
  overflow: auto;
  margin: -10px;
  padding: 10px;
}

.rrp-pre {
  margin: 0;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--guaxi-text-primary, #1f2328);
  font-family: inherit;
}

.rrp-doc {
  background: var(--guaxi-panel-bg, #fff);
  border-radius: 8px;
}

.rrp-markdown {
  font-size: 13px;
  line-height: 1.7;
  word-break: break-word;
}

.rrp-markdown :deep(.gmd-p) {
  margin: 0 0 8px;
}

.rrp-markdown :deep(.gmd-img) {
  max-width: 100%;
  border-radius: 6px;
}

.rrp-markdown :deep(.gmd-pre) {
  padding: 8px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 6%, transparent);
  overflow: auto;
  font-size: 12px;
}

.rrp-markdown :deep(.gmd-code-inline) {
  padding: 1px 4px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 8%, transparent);
}

.rrp-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.rrp-table th,
.rrp-table td {
  padding: 5px 8px;
  border-bottom: 1px solid color-mix(in srgb, var(--guaxi-border, #d0d7de) 70%, transparent);
  text-align: left;
  white-space: nowrap;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rrp-table thead th {
  position: sticky;
  top: 0;
  background: var(--guaxi-panel-bg, #fff);
  font-weight: 600;
  color: var(--guaxi-text-secondary, #57606a);
  z-index: 1;
}

.rrp-table tbody tr:hover td {
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 4%, transparent);
}

.rrp-rownum {
  width: 28px;
  color: var(--guaxi-text-secondary, #57606a);
  text-align: right;
  user-select: none;
}

.rrp-note {
  margin-top: 8px;
  font-size: 11px;
  line-height: 1.5;
  color: var(--guaxi-text-secondary, #57606a);
}

.rrp-badge {
  position: absolute;
  right: 8px;
  bottom: 8px;
  padding: 2px 6px;
  border-radius: 5px;
  font-size: 11px;
  color: var(--guaxi-text-secondary, #57606a);
  background: var(--guaxi-panel-bg, #fff);
  border: 1px solid var(--guaxi-border, #d0d7de);
}

.rrp-veil {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 16px;
  text-align: center;
  font-size: 12px;
  color: var(--guaxi-text-secondary, #57606a);
  background: color-mix(in srgb, var(--guaxi-panel-bg, #fff) 86%, transparent);
  backdrop-filter: blur(2px);
}

.rrp-veil.is-error {
  flex-direction: column;
  color: var(--n-error-color, #e06c75);
}

.rrp-spinner {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid color-mix(in srgb, var(--guaxi-text-secondary, #57606a) 30%, transparent);
  border-top-color: var(--n-primary-color, #61afef);
  animation: rrp-spin 0.7s linear infinite;
}

@keyframes rrp-spin {
  to {
    transform: rotate(360deg);
  }
}

.rrp-link-btn {
  border: 0;
  background: none;
  padding: 0;
  color: var(--n-primary-color, #61afef);
  font: inherit;
  cursor: pointer;
  text-decoration: underline;
}

.rrp-big {
  flex: 0 0 auto;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-top: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  color: var(--guaxi-text-secondary, #57606a);
  background: color-mix(in srgb, var(--guaxi-text-primary, #1f2328) 5%, transparent);
  font-size: 11.5px;
  line-height: 1.5;
}

.rrp-big-title {
  color: var(--guaxi-text-primary, #1f2328);
  font-weight: 600;
}

.rrp-big-hint {
  margin-top: 2px;
}

.rrp-footer {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 10px;
  padding-top: 8px;
  font-size: 11px;
  color: var(--guaxi-text-secondary, #57606a);
  font-variant-numeric: tabular-nums;
}

.rrp-footer-spacer {
  flex: 1;
}

@media (prefers-reduced-motion: reduce) {
  .rrp-spinner {
    animation-duration: 2.4s;
  }
}
</style>
