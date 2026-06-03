<template>
  <div v-if="open" class="export-dialog" @mousedown.self="requestClose">
    <div class="export-dialog__panel" role="dialog" aria-modal="true">
      <header class="export-dialog__header">
        <div class="export-dialog__title-block">
          <button
            v-if="step === 'configure'"
            type="button"
            class="export-dialog__icon-btn"
            title="返回选择导出格式"
            @click="backToFormat"
          >
            <Icon :icon="arrowLeftIcon" width="16" height="16" />
          </button>
          <div>
            <h2>{{ step === 'format' ? '导出数据' : (activeFormat?.label || '配置导出') }}</h2>
            <small>变量 {{ variableName || '-' }}</small>
          </div>
        </div>
        <button type="button" class="export-dialog__icon-btn" title="关闭" @click="requestClose">
          <Icon :icon="xIcon" width="16" height="16" />
        </button>
      </header>

      <!-- 第一步：选择类别与格式 -->
      <section v-if="step === 'format'" class="export-dialog__body">
        <div class="export-dialog__category-tabs">
          <button
            v-for="cat in categories"
            :key="cat.id"
            type="button"
            class="export-dialog__category-tab"
            :class="{ 'is-active': cat.id === selectedCategory }"
            @click="selectedCategory = cat.id"
          >
            <Icon :icon="categoryIcon(cat.id)" width="16" height="16" />
            <span>{{ cat.label }}</span>
          </button>
        </div>
        <div class="export-dialog__format-grid">
          <button
            v-for="fmt in formatsForCategory"
            :key="fmt.id"
            type="button"
            class="export-dialog__format-card"
            @click="chooseFormat(fmt)"
          >
            <Icon :icon="categoryIcon(fmt.category)" width="20" height="20" />
            <strong>{{ fmt.label }}</strong>
            <span v-if="fmt.ext?.length" class="export-dialog__format-badge">.{{ fmt.ext[0] }}</span>
          </button>
          <div v-if="!formatsForCategory.length" class="export-dialog__muted">该类别暂无可用格式</div>
        </div>
      </section>

      <!-- 第二步：配置 -->
      <section v-else class="export-dialog__body">
        <div class="export-dialog__config">
          <template v-for="field in activeFormat?.fields || []" :key="field.key">
            <!-- 保存路径选择 -->
            <label v-if="field.type === 'save'" class="export-dialog__field">
              <span>{{ field.label }}<em v-if="field.required">*</em></span>
              <div class="export-dialog__file-row">
                <input class="export-dialog__input" :value="getField(field.key)" readonly :placeholder="field.placeholder || '点击选择保存位置'" />
                <button type="button" class="export-dialog__mini-btn" @click="pickSavePath(field)">选择</button>
              </div>
            </label>

            <!-- 脚本文件选择 -->
            <label v-else-if="field.type === 'file'" class="export-dialog__field">
              <span>{{ field.label }}<em v-if="field.required">*</em></span>
              <div class="export-dialog__file-row">
                <input class="export-dialog__input" :value="getField(field.key)" readonly :placeholder="field.placeholder || '点击选择文件'" />
                <button type="button" class="export-dialog__mini-btn" @click="pickFile(field)">选择</button>
              </div>
            </label>

            <!-- 下拉选择 -->
            <label v-else-if="field.type === 'select'" class="export-dialog__field">
              <span>{{ field.label }}</span>
              <select class="export-dialog__input" :value="getField(field.key)" @change="setField(field.key, $event.target.value)">
                <option v-for="opt in field.options || []" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
              </select>
            </label>

            <!-- 数字 -->
            <label v-else-if="field.type === 'number'" class="export-dialog__field">
              <span>{{ field.label }}</span>
              <input class="export-dialog__input" type="number" :value="getField(field.key)" :placeholder="field.placeholder" @input="setField(field.key, $event.target.value)" />
            </label>

            <!-- 代码编辑器 -->
            <label v-else-if="field.type === 'code'" class="export-dialog__field export-dialog__field--wide">
              <span>{{ field.label }}</span>
              <textarea
                class="export-dialog__input export-dialog__code"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                spellcheck="false"
                rows="8"
                @input="setField(field.key, $event.target.value)"
              />
            </label>

            <!-- 普通文本 -->
            <label v-else class="export-dialog__field">
              <span>{{ field.label }}<em v-if="field.required">*</em></span>
              <input
                class="export-dialog__input"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                @input="setField(field.key, $event.target.value)"
              />
            </label>
          </template>
        </div>
      </section>

      <footer v-if="step === 'configure'" class="export-dialog__footer">
        <span v-if="footerError" class="export-dialog__footer-error">{{ footerError }}</span>
        <div class="export-dialog__footer-actions">
          <button type="button" class="export-dialog__mini-btn" @click="backToFormat">上一步</button>
          <button type="button" class="export-dialog__action-btn" :disabled="exporting" @click="confirmExport">
            <Icon :icon="uploadIcon" width="15" height="15" />
            {{ exporting ? '导出中…' : '导出' }}
          </button>
        </div>
      </footer>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { open as openFileDialog, save as saveFileDialog } from '@tauri-apps/plugin-dialog'
import arrowLeftIcon from '@iconify-icons/lucide/arrow-left'
import xIcon from '@iconify-icons/lucide/x'
import uploadIcon from '@iconify-icons/lucide/upload'
import fileIcon from '@iconify-icons/lucide/file'
import databaseIcon from '@iconify-icons/lucide/database'
import codeIcon from '@iconify-icons/lucide/code-2'
import { createLogger } from '../../utils/logger'
import {
  EXPORT_CATEGORIES,
  getExportFormat,
  getExportFormatsByCategory,
  normalizeExportSpec,
} from './utils/guaxiNotebookDataLab'

const logger = createLogger('GuaxiDataExportDialog')

const props = defineProps({
  open: { type: Boolean, default: false },
  sessionId: { type: String, default: '' },
  variableName: { type: String, default: '' },
  category: { type: String, default: 'file' },
})
const emit = defineEmits(['update:open', 'confirm'])

const categories = EXPORT_CATEGORIES
const CATEGORY_ICONS = { file: fileIcon, database: databaseIcon, script: codeIcon }
const categoryIcon = (id) => CATEGORY_ICONS[id] || fileIcon

const step = ref('format')
const selectedCategory = ref('file')
const selectedFormatId = ref('')
const draft = reactive({ target: {}, options: {} })
const exporting = ref(false)
const footerError = ref('')

const formatsForCategory = computed(() => getExportFormatsByCategory(selectedCategory.value))
const activeFormat = computed(() => getExportFormat(selectedFormatId.value))

watch(() => props.open, (value) => {
  if (value) resetForCategory(props.category)
})

const resetDraft = () => {
  draft.target = {}
  draft.options = {}
}

const resetForCategory = (category) => {
  step.value = 'format'
  selectedCategory.value = categories.some((c) => c.id === category) ? category : 'file'
  selectedFormatId.value = ''
  footerError.value = ''
  resetDraft()
}

const getField = (key) => {
  const [group, name] = String(key).split('.')
  return draft[group]?.[name] ?? ''
}
const setField = (key, value) => {
  const [group, name] = String(key).split('.')
  if (!draft[group]) draft[group] = {}
  draft[group][name] = value
}

const chooseFormat = (fmt) => {
  selectedFormatId.value = fmt.id
  resetDraft()
  footerError.value = ''
  ;(fmt.fields || []).forEach((field) => {
    if (field.default != null) setField(field.key, field.default)
  })
  step.value = 'configure'
  logger.info('[ExportDialog] chose format', { category: selectedCategory.value, format: fmt.id })
}

const backToFormat = () => {
  step.value = 'format'
}

const defaultFileName = (ext) => `${props.variableName || 'guaxi_data'}.${ext || 'dat'}`

const pickSavePath = async (field) => {
  try {
    const ext = field.ext?.[0] || ''
    const selected = await saveFileDialog({
      defaultPath: defaultFileName(ext),
      filters: field.ext?.length ? [{ name: String(activeFormat.value?.label || ext).toUpperCase(), extensions: field.ext }] : undefined,
    })
    if (selected) setField(field.key, String(selected))
  } catch (error) {
    logger.warn('[ExportDialog] pick save path failed', { error: String(error?.message || error) })
  }
}

const pickFile = async (field) => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: field.accept?.length ? [{ name: 'Script', extensions: field.accept }] : undefined,
    })
    if (selected) setField(field.key, String(selected))
  } catch (error) {
    logger.warn('[ExportDialog] pick file failed', { error: String(error?.message || error) })
  }
}

const buildSpec = () => normalizeExportSpec({
  format: selectedFormatId.value,
  variableName: props.variableName,
  target: { ...draft.target },
  options: { ...draft.options },
})

const validate = () => {
  const fmt = activeFormat.value
  if (!fmt) return '请选择导出格式'
  for (const field of fmt.fields || []) {
    if (field.required && !String(getField(field.key) || '').trim()) {
      return `请填写「${field.label}」`
    }
  }
  return ''
}

const confirmExport = () => {
  footerError.value = validate()
  if (footerError.value) return
  const spec = buildSpec()
  logger.info('[ExportDialog] confirm export', { format: spec.format, variableName: spec.variableName })
  emit('confirm', spec)
  emit('update:open', false)
}

const requestClose = () => {
  emit('update:open', false)
}
</script>

<style scoped>
.export-dialog {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(15, 23, 42, 0.42);
}

.export-dialog__panel {
  width: min(720px, 96vw);
  max-height: min(88vh, 720px);
  display: flex;
  flex-direction: column;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  border-radius: 12px;
  background: var(--dl-panel, #fff);
  color: var(--dl-text, #1d2430);
  box-shadow: 0 26px 60px rgba(15, 23, 42, 0.34);
  overflow: hidden;
}

.export-dialog__header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  background: var(--dl-panel-2, #eef3f7);
}

.export-dialog__title-block {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.export-dialog__title-block h2 {
  margin: 0;
  font-size: 16px;
  line-height: 1.2;
}

.export-dialog__title-block small {
  color: var(--dl-muted, #667085);
  font-size: 12px;
}

.export-dialog__body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 16px;
}

.export-dialog__category-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 14px;
}

.export-dialog__category-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 34px;
  padding: 0 12px;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  border-radius: 999px;
  background: var(--dl-panel-2, #eef3f7);
  color: var(--dl-muted, #667085);
  cursor: pointer;
}

.export-dialog__category-tab.is-active {
  color: var(--dl-text, #1d2430);
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 55%, var(--dl-border, rgba(31, 41, 55, 0.13)));
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
}

.export-dialog__format-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 12px;
}

.export-dialog__format-card {
  min-height: 100px;
  display: grid;
  place-items: center;
  align-content: center;
  gap: 6px;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  border-radius: 10px;
  padding: 12px;
  background: var(--dl-panel, #fff);
  color: var(--dl-text, #1d2430);
  cursor: pointer;
}

.export-dialog__format-card:hover {
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 42%, var(--dl-border, rgba(31, 41, 55, 0.13)));
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
}

.export-dialog__format-badge {
  border-radius: 999px;
  padding: 1px 8px;
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
  color: var(--dl-muted, #667085);
  font-size: 11px;
}

.export-dialog__config {
  display: grid;
  align-content: start;
  gap: 12px;
  min-width: 0;
}

.export-dialog__field {
  display: grid;
  gap: 5px;
  font-size: 12px;
  color: var(--dl-muted, #667085);
  min-width: 0;
}

.export-dialog__field > span {
  display: flex;
  align-items: center;
  gap: 8px;
}

.export-dialog__field em {
  color: #d92d20;
  font-style: normal;
}

.export-dialog__input {
  width: 100%;
  min-height: 34px;
  box-sizing: border-box;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  border-radius: 8px;
  padding: 7px 10px;
  background: var(--dl-panel, #fff);
  color: var(--dl-text, #1d2430);
  font: inherit;
  outline: none;
}

.export-dialog__input:focus {
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 55%, var(--dl-border, rgba(31, 41, 55, 0.13)));
}

textarea.export-dialog__input {
  resize: vertical;
  line-height: 1.45;
}

.export-dialog__code {
  font-family: var(--editor-font-family, Consolas, monospace);
  font-size: 12.5px;
  white-space: pre;
}

.export-dialog__file-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
}

.export-dialog__footer {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-top: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  background: var(--dl-panel-2, #eef3f7);
}

.export-dialog__footer-error {
  color: #d92d20;
  font-size: 12px;
}

.export-dialog__footer-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}

.export-dialog__icon-btn,
.export-dialog__mini-btn,
.export-dialog__action-btn {
  cursor: pointer;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  background: var(--dl-panel, #fff);
  color: var(--dl-text, #1d2430);
}

.export-dialog__icon-btn {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.export-dialog__mini-btn {
  min-height: 30px;
  border-radius: 8px;
  padding: 0 12px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
}

.export-dialog__action-btn {
  min-height: 32px;
  border-radius: 8px;
  padding: 0 16px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 45%, var(--dl-border, rgba(31, 41, 55, 0.13)));
}

.export-dialog__action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.export-dialog__muted {
  color: var(--dl-muted, #667085);
  font-size: 12px;
}
</style>
