<template>
  <div v-if="open" class="import-dialog" @mousedown.self="requestClose">
    <div class="import-dialog__panel" role="dialog" aria-modal="true">
      <header class="import-dialog__header">
        <div class="import-dialog__title-block">
          <button
            v-if="step === 'configure'"
            type="button"
            class="import-dialog__icon-btn"
            title="返回选择数据源"
            @click="backToFormat"
          >
            <Icon :icon="arrowLeftIcon" width="16" height="16" />
          </button>
          <div>
            <h2>{{ step === 'format' ? '选择数据源' : (activeFormat?.label || '配置导入') }}</h2>
            <small>{{ step === 'format' ? '标准结构化 / 半结构化 / 私有格式' : activeCategoryLabel }}</small>
          </div>
        </div>
        <button type="button" class="import-dialog__icon-btn" title="关闭" @click="requestClose">
          <Icon :icon="xIcon" width="16" height="16" />
        </button>
      </header>

      <!-- 第一步：选择类别与格式 -->
      <section v-if="step === 'format'" class="import-dialog__body">
        <div class="import-dialog__category-tabs">
          <button
            v-for="cat in categories"
            :key="cat.id"
            type="button"
            class="import-dialog__category-tab"
            :class="{ 'is-active': cat.id === selectedCategory }"
            @click="selectedCategory = cat.id"
          >
            <Icon :icon="categoryIcon(cat.id)" width="16" height="16" />
            <span>{{ cat.label }}</span>
          </button>
        </div>
        <div class="import-dialog__format-grid">
          <button
            v-for="fmt in formatsForCategory"
            :key="fmt.id"
            type="button"
            class="import-dialog__format-card"
            @click="chooseFormat(fmt)"
          >
            <Icon :icon="categoryIcon(fmt.category)" width="20" height="20" />
            <strong>{{ fmt.label }}</strong>
            <span v-if="fmt.needsProbe" class="import-dialog__format-badge">支持探测预览</span>
          </button>
          <div v-if="!formatsForCategory.length" class="import-dialog__muted">该类别暂无可用格式</div>
        </div>
      </section>

      <!-- 第二步：配置 + 探测预览 -->
      <section v-else class="import-dialog__body import-dialog__body--configure">
        <div class="import-dialog__config">
          <label class="import-dialog__field">
            <span>变量名</span>
            <input
              class="import-dialog__input"
              :value="draft.variableName"
              placeholder="留空则按来源自动命名"
              @input="draft.variableName = $event.target.value"
            />
          </label>

          <template v-for="field in activeFormat?.fields || []" :key="field.key">
            <!-- 文件选择 -->
            <label v-if="field.type === 'file'" class="import-dialog__field">
              <span>{{ field.label }}<em v-if="field.required">*</em></span>
              <div class="import-dialog__file-row">
                <input class="import-dialog__input" :value="getField(field.key)" readonly :placeholder="field.placeholder || '点击选择文件'" />
                <button type="button" class="import-dialog__mini-btn" @click="pickFile(field)">选择</button>
              </div>
            </label>

            <!-- 工作表选择（探测后填充） -->
            <label v-else-if="field.type === 'sheet'" class="import-dialog__field">
              <span>{{ field.label }}</span>
              <select
                v-if="probeResult?.sheets?.length"
                class="import-dialog__input"
                :value="getField(field.key)"
                @change="setField(field.key, $event.target.value)"
              >
                <option value="">（第一张工作表）</option>
                <option v-for="sheet in probeResult.sheets" :key="sheet" :value="sheet">{{ sheet }}</option>
              </select>
              <input
                v-else
                class="import-dialog__input"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                @input="setField(field.key, $event.target.value)"
              />
            </label>

            <!-- 数据库表选择（探测后填充） -->
            <label v-else-if="field.type === 'table'" class="import-dialog__field">
              <span>{{ field.label }}</span>
              <select
                v-if="probeResult?.tables?.length"
                class="import-dialog__input"
                :value="getField(field.key)"
                @change="setField(field.key, $event.target.value)"
              >
                <option value="">（请选择表）</option>
                <option v-for="tbl in probeResult.tables" :key="tbl.name" :value="tbl.name">{{ tbl.name }}</option>
              </select>
              <input
                v-else
                class="import-dialog__input"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                @input="setField(field.key, $event.target.value)"
              />
              <small v-if="selectedTableColumns.length" class="import-dialog__hint">列：{{ selectedTableColumns.join(', ') }}</small>
            </label>

            <!-- 下拉选择 -->
            <label v-else-if="field.type === 'select'" class="import-dialog__field">
              <span>{{ field.label }}</span>
              <select
                class="import-dialog__input"
                :value="getField(field.key)"
                @change="setField(field.key, $event.target.value)"
              >
                <option v-for="opt in field.options || []" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
              </select>
            </label>

            <!-- 键值对（请求头） -->
            <div v-else-if="field.type === 'kv'" class="import-dialog__field">
              <span>{{ field.label }}</span>
              <div class="import-dialog__kv-list">
                <div v-for="(row, idx) in kvState[field.key] || []" :key="`${field.key}-${idx}`" class="import-dialog__kv-row">
                  <input class="import-dialog__input" v-model="row.key" placeholder="名称" />
                  <input class="import-dialog__input" v-model="row.value" placeholder="值" />
                  <button type="button" class="import-dialog__icon-btn" title="删除" @click="removeKvRow(field.key, idx)">
                    <Icon :icon="xIcon" width="14" height="14" />
                  </button>
                </div>
                <button type="button" class="import-dialog__mini-btn" @click="addKvRow(field.key)">
                  <Icon :icon="plusIcon" width="13" height="13" /> 添加
                </button>
              </div>
            </div>

            <!-- 代码编辑器 -->
            <label v-else-if="field.type === 'code'" class="import-dialog__field import-dialog__field--wide">
              <span>{{ field.label }}</span>
              <textarea
                class="import-dialog__input import-dialog__code"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                spellcheck="false"
                rows="8"
                @input="setField(field.key, $event.target.value)"
              />
            </label>

            <!-- 多行文本 -->
            <label v-else-if="field.type === 'textarea'" class="import-dialog__field import-dialog__field--wide">
              <span>
                {{ field.label }}<em v-if="field.required">*</em>
                <button v-if="field.clipboard" type="button" class="import-dialog__link-btn" @click="readClipboardInto(field)">读取剪贴板</button>
              </span>
              <textarea
                class="import-dialog__input"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                rows="5"
                @input="setField(field.key, $event.target.value)"
              />
            </label>

            <!-- 普通文本 -->
            <label v-else class="import-dialog__field">
              <span>{{ field.label }}<em v-if="field.required">*</em></span>
              <input
                class="import-dialog__input"
                :value="getField(field.key)"
                :placeholder="field.placeholder"
                @input="setField(field.key, $event.target.value)"
              />
            </label>
          </template>
        </div>

        <!-- 探测结果预览 -->
        <div v-if="activeFormat?.needsProbe || probeResult || probeError" class="import-dialog__preview">
          <div class="import-dialog__preview-head">
            <strong>探测预览</strong>
            <button
              v-if="activeFormat?.needsProbe"
              type="button"
              class="import-dialog__mini-btn"
              :disabled="probing"
              @click="runProbe"
            >
              <Icon :icon="searchIcon" width="13" height="13" />
              {{ probing ? '探测中…' : '探测' }}
            </button>
          </div>

          <div v-if="probeError" class="import-dialog__preview-error">{{ probeError }}</div>

          <div v-else-if="probeResult" class="import-dialog__preview-body">
            <div v-if="probeMeta.length" class="import-dialog__preview-meta">
              <span v-for="item in probeMeta" :key="item">{{ item }}</span>
            </div>

            <table v-if="previewRows.length" class="import-dialog__preview-table">
              <thead>
                <tr><th v-for="col in previewColumns" :key="col">{{ col }}</th></tr>
              </thead>
              <tbody>
                <tr v-for="(row, rIdx) in previewRows" :key="rIdx">
                  <td v-for="col in previewColumns" :key="col">{{ cellText(row[col]) }}</td>
                </tr>
              </tbody>
            </table>

            <pre v-else-if="probeResult.preview" class="import-dialog__preview-pre">{{ probeResult.preview }}</pre>
            <div v-else-if="probeResult.note" class="import-dialog__muted">{{ probeResult.note }}</div>
          </div>

          <div v-else class="import-dialog__muted">点击“探测”可在导入前确认表结构、工作表或数据库表。</div>
        </div>
      </section>

      <footer v-if="step === 'configure'" class="import-dialog__footer">
        <span v-if="footerError" class="import-dialog__footer-error">{{ footerError }}</span>
        <div class="import-dialog__footer-actions">
          <button type="button" class="import-dialog__mini-btn" @click="backToFormat">上一步</button>
          <button type="button" class="import-dialog__action-btn" @click="confirmImport">
            <Icon :icon="downloadIcon" width="15" height="15" />
            导入
          </button>
        </div>
      </footer>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import arrowLeftIcon from '@iconify-icons/lucide/arrow-left'
import xIcon from '@iconify-icons/lucide/x'
import plusIcon from '@iconify-icons/lucide/plus'
import searchIcon from '@iconify-icons/lucide/scan-search'
import downloadIcon from '@iconify-icons/lucide/download'
import fileIcon from '@iconify-icons/lucide/file'
import databaseIcon from '@iconify-icons/lucide/database'
import globeIcon from '@iconify-icons/lucide/globe'
import clipboardIcon from '@iconify-icons/lucide/clipboard'
import codeIcon from '@iconify-icons/lucide/code-2'
import { createLogger } from '../../utils/logger'
import {
  IMPORT_CATEGORIES,
  getImportFormat,
  getImportFormatsByCategory,
  normalizeImportSpec,
} from './utils/guaxiNotebookDataLab'
import { probeWorkbenchImportSource } from './utils/guaxiDataLabWorkbenchClient'

const logger = createLogger('GuaxiDataImportDialog')

const props = defineProps({
  open: { type: Boolean, default: false },
  sessionId: { type: String, default: '' },
  category: { type: String, default: '' },
})
const emit = defineEmits(['update:open', 'confirm'])

const categories = IMPORT_CATEGORIES
const CATEGORY_ICONS = {
  file: fileIcon,
  database: databaseIcon,
  api: globeIcon,
  clipboard: clipboardIcon,
  script: codeIcon,
}

const step = ref('format')
const selectedCategory = ref('file')
const selectedFormatId = ref('')
const draft = reactive({ source: {}, options: {}, variableName: '', sourceLabel: '' })
const kvState = reactive({})
const probing = ref(false)
const probeError = ref('')
const probeResult = ref(null)
const footerError = ref('')

const formatsForCategory = computed(() => getImportFormatsByCategory(selectedCategory.value))
const activeFormat = computed(() => getImportFormat(selectedFormatId.value))
const activeCategoryLabel = computed(() => categories.find((c) => c.id === selectedCategory.value)?.label || '')

const categoryIcon = (id) => CATEGORY_ICONS[id] || fileIcon

const selectedTableColumns = computed(() => {
  const tables = probeResult.value?.tables
  if (!Array.isArray(tables)) return []
  const current = String(draft.source.table || '')
  const matched = tables.find((t) => String(t?.name) === current)
  return Array.isArray(matched?.columns) ? matched.columns.map((c) => String(c)) : []
})

const previewColumns = computed(() => {
  if (Array.isArray(probeResult.value?.columns) && probeResult.value.columns.length) {
    return probeResult.value.columns.map((c) => String(c))
  }
  const rows = previewRows.value
  return rows.length ? Object.keys(rows[0]) : []
})
const previewRows = computed(() => {
  const sample = probeResult.value?.sample
  if (Array.isArray(sample)) return sample.slice(0, 20)
  const rows = probeResult.value?.rows
  if (Array.isArray(rows)) return rows.slice(0, 20)
  return []
})
const probeMeta = computed(() => {
  const r = probeResult.value
  if (!r) return []
  const meta = []
  if (r.encoding) meta.push(`编码 ${r.encoding}`)
  if (r.sep) meta.push(`分隔符 ${r.sep}`)
  if (Array.isArray(r.sheets)) meta.push(`${r.sheets.length} 个工作表`)
  if (Array.isArray(r.tables)) meta.push(`${r.tables.length} 张表`)
  if (r.topType) meta.push(`类型 ${r.topType}`)
  if (r.length != null) meta.push(`长度 ${r.length}`)
  if (r.matched != null && r.total != null) meta.push(`匹配 ${r.matched}/${r.total} 行`)
  if (r.kind) meta.push(`kind ${r.kind}`)
  return meta
})

watch(() => props.open, (value) => {
  if (value) resetForCategory(props.category)
})

const resetDraft = () => {
  draft.source = {}
  draft.options = {}
  draft.variableName = ''
  draft.sourceLabel = ''
  Object.keys(kvState).forEach((key) => delete kvState[key])
}

const resetForCategory = (category) => {
  step.value = 'format'
  selectedCategory.value = categories.some((c) => c.id === category) ? category : 'file'
  selectedFormatId.value = ''
  probeResult.value = null
  probeError.value = ''
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
  probeResult.value = null
  probeError.value = ''
  footerError.value = ''
  ;(fmt.fields || []).forEach((field) => {
    if (field.default != null) setField(field.key, field.default)
    if (field.type === 'kv') kvState[field.key] = [{ key: '', value: '' }]
  })
  step.value = 'configure'
  logger.info('[ImportDialog] chose format', { category: selectedCategory.value, format: fmt.id })
}

const backToFormat = () => {
  step.value = 'format'
}

const addKvRow = (key) => {
  if (!Array.isArray(kvState[key])) kvState[key] = []
  kvState[key].push({ key: '', value: '' })
}
const removeKvRow = (key, index) => {
  if (Array.isArray(kvState[key])) kvState[key].splice(index, 1)
}

const pickFile = async (field) => {
  try {
    const selected = await openFileDialog({
      multiple: false,
      filters: field.accept?.length ? [{ name: 'Data', extensions: field.accept }] : undefined,
    })
    if (!selected) return
    const path = String(selected)
    setField(field.key, path)
    if (!draft.sourceLabel) draft.sourceLabel = path.split(/[\\/]/).pop() || path
  } catch (error) {
    logger.warn('[ImportDialog] pick file failed', { error: String(error?.message || error) })
  }
}

const readClipboardInto = async (field) => {
  let text = ''
  try {
    text = await navigator?.clipboard?.readText?.()
  } catch {
    text = ''
  }
  if (!text) {
    try {
      const mod = await import('@tauri-apps/plugin-clipboard-manager')
      text = await mod.readText()
    } catch (error) {
      logger.warn('[ImportDialog] read clipboard failed', { error: String(error?.message || error) })
    }
  }
  if (text) setField(field.key, String(text))
}

const syncKvToDraft = () => {
  Object.entries(kvState).forEach(([key, rows]) => {
    const obj = {}
    ;(rows || []).forEach((row) => {
      if (String(row?.key || '').trim()) obj[row.key] = row.value
    })
    setField(key, obj)
  })
}

const buildSpec = () => {
  syncKvToDraft()
  return normalizeImportSpec({
    category: selectedCategory.value,
    format: selectedFormatId.value,
    variableName: draft.variableName,
    sourceLabel: draft.sourceLabel,
    source: { ...draft.source },
    options: { ...draft.options },
  })
}

const hasSourceContent = () => Object.values(draft.source || {}).some((value) => String(value ?? '').trim())

const runProbe = async () => {
  if (!hasSourceContent()) {
    probeError.value = '请先填写数据源（文件 / 连接串 / URL / 文本 / 脚本）'
    return
  }
  probing.value = true
  probeError.value = ''
  try {
    const spec = buildSpec()
    const result = await probeWorkbenchImportSource({ sessionId: props.sessionId, spec })
    probeResult.value = result
    // 探测嗅探出的编码/分隔符在用户未填时回填，省去手动确认。
    if (result?.encoding && !String(draft.options.encoding || '').trim()) draft.options.encoding = result.encoding
    if (result?.sep && !String(draft.options.sep || '').trim()) draft.options.sep = result.sep
    logger.info('[ImportDialog] probe done', { format: selectedFormatId.value })
  } catch (error) {
    probeError.value = String(error?.message || error)
  } finally {
    probing.value = false
  }
}

const confirmImport = () => {
  footerError.value = ''
  if (!hasSourceContent()) {
    footerError.value = '请先填写数据源后再导入'
    return
  }
  const spec = buildSpec()
  logger.info('[ImportDialog] confirm import', {
    category: spec.category,
    format: spec.format,
    variableName: spec.variableName,
  })
  emit('confirm', spec)
  emit('update:open', false)
}

const requestClose = () => {
  emit('update:open', false)
}

const cellText = (value) => {
  if (value == null) return ''
  if (typeof value === 'object') {
    try {
      return JSON.stringify(value)
    } catch {
      return String(value)
    }
  }
  return String(value)
}
</script>

<style scoped>
.import-dialog {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(15, 23, 42, 0.42);
}

.import-dialog__panel {
  width: min(880px, 96vw);
  max-height: min(88vh, 760px);
  display: flex;
  flex-direction: column;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  border-radius: 12px;
  background: var(--dl-panel, #fff);
  color: var(--dl-text, #1d2430);
  box-shadow: 0 26px 60px rgba(15, 23, 42, 0.34);
  overflow: hidden;
}

.import-dialog__header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  background: var(--dl-panel-2, #eef3f7);
}

.import-dialog__title-block {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.import-dialog__title-block h2 {
  margin: 0;
  font-size: 16px;
  line-height: 1.2;
}

.import-dialog__title-block small {
  color: var(--dl-muted, #667085);
  font-size: 12px;
}

.import-dialog__body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 16px;
}

.import-dialog__body--configure {
  display: grid;
  grid-template-columns: minmax(0, 1.05fr) minmax(0, 1fr);
  gap: 16px;
}

.import-dialog__category-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 14px;
}

.import-dialog__category-tab {
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

.import-dialog__category-tab.is-active {
  color: var(--dl-text, #1d2430);
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 55%, var(--dl-border, rgba(31, 41, 55, 0.13)));
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
}

.import-dialog__format-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

.import-dialog__format-card {
  min-height: 104px;
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

.import-dialog__format-card:hover {
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 42%, var(--dl-border, rgba(31, 41, 55, 0.13)));
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
}

.import-dialog__format-badge {
  border-radius: 999px;
  padding: 1px 8px;
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
  color: var(--dl-muted, #667085);
  font-size: 11px;
}

.import-dialog__config {
  display: grid;
  align-content: start;
  gap: 12px;
  min-width: 0;
}

.import-dialog__field {
  display: grid;
  gap: 5px;
  font-size: 12px;
  color: var(--dl-muted, #667085);
  min-width: 0;
}

.import-dialog__field > span {
  display: flex;
  align-items: center;
  gap: 8px;
}

.import-dialog__field em {
  color: #d92d20;
  font-style: normal;
}

.import-dialog__input {
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

.import-dialog__input:focus {
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 55%, var(--dl-border, rgba(31, 41, 55, 0.13)));
}

textarea.import-dialog__input {
  resize: vertical;
  line-height: 1.45;
}

.import-dialog__code {
  font-family: var(--editor-font-family, Consolas, monospace);
  font-size: 12.5px;
  white-space: pre;
}

.import-dialog__file-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
}

.import-dialog__kv-list {
  display: grid;
  gap: 6px;
}

.import-dialog__kv-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto;
  gap: 6px;
  align-items: center;
}

.import-dialog__hint {
  color: var(--dl-muted, #667085);
}

.import-dialog__link-btn {
  border: 0;
  background: transparent;
  color: var(--dl-accent, #2f6f55);
  cursor: pointer;
  font: inherit;
  padding: 0;
}

.import-dialog__preview {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  border: 1px dashed var(--dl-border, rgba(31, 41, 55, 0.13));
  border-radius: 10px;
  padding: 12px;
  background: var(--dl-panel-2, #eef3f7);
}

.import-dialog__preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.import-dialog__preview-error {
  color: #d92d20;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-word;
}

.import-dialog__preview-body {
  display: grid;
  gap: 8px;
  min-width: 0;
}

.import-dialog__preview-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.import-dialog__preview-meta span {
  border-radius: 999px;
  padding: 2px 8px;
  background: var(--dl-panel, #fff);
  color: var(--dl-muted, #667085);
  font-size: 11px;
}

.import-dialog__preview-table {
  display: block;
  max-height: 280px;
  overflow: auto;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 12px;
}

.import-dialog__preview-table th,
.import-dialog__preview-table td {
  border-right: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  border-bottom: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  padding: 5px 8px;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.import-dialog__preview-table th {
  position: sticky;
  top: 0;
  background: var(--dl-panel, #fff);
}

.import-dialog__preview-pre {
  margin: 0;
  max-height: 280px;
  overflow: auto;
  border-radius: 8px;
  padding: 10px;
  background: var(--dl-panel, #fff);
  font-family: var(--editor-font-family, Consolas, monospace);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.import-dialog__footer {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-top: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  background: var(--dl-panel-2, #eef3f7);
}

.import-dialog__footer-error {
  color: #d92d20;
  font-size: 12px;
}

.import-dialog__footer-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}

.import-dialog__icon-btn,
.import-dialog__mini-btn,
.import-dialog__action-btn {
  cursor: pointer;
  border: 1px solid var(--dl-border, rgba(31, 41, 55, 0.13));
  background: var(--dl-panel, #fff);
  color: var(--dl-text, #1d2430);
}

.import-dialog__icon-btn {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.import-dialog__mini-btn {
  min-height: 30px;
  border-radius: 8px;
  padding: 0 12px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
}

.import-dialog__action-btn {
  min-height: 32px;
  border-radius: 8px;
  padding: 0 16px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--dl-accent-soft, rgba(47, 111, 85, 0.13));
  border-color: color-mix(in srgb, var(--dl-accent, #2f6f55) 45%, var(--dl-border, rgba(31, 41, 55, 0.13)));
}

.import-dialog__mini-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.import-dialog__muted {
  color: var(--dl-muted, #667085);
  font-size: 12px;
}

@media (max-width: 760px) {
  .import-dialog__body--configure {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
