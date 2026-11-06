<template>
  <n-modal
    :show="show"
    preset="card"
    :title="title"
    class="prd-modal"
    :bordered="true"
    @update:show="onShowChange"
  >
    <div class="prd-body">
      <p v-if="description" class="prd-desc">{{ description }}</p>

      <div v-for="f in safeFields" :key="f.key" class="prd-row">
        <div class="prd-label">
          {{ f.label || f.key }}
          <span v-if="f.required" class="prd-req">*</span>
        </div>
        <div class="prd-control">
          <n-switch v-if="f.type === 'bool'" v-model:value="values[f.key]" size="small" />
          <n-input-number
            v-else-if="f.type === 'number'"
            v-model:value="values[f.key]"
            size="small"
            class="prd-number"
          />
          <n-select
            v-else-if="f.type === 'select'"
            v-model:value="values[f.key]"
            size="small"
            :options="f.options"
            class="prd-select"
          />
          <n-input
            v-else-if="f.type === 'textarea'"
            v-model:value="values[f.key]"
            type="textarea"
            size="small"
            :autosize="{ minRows: 3, maxRows: 8 }"
            :placeholder="f.placeholder || ''"
          />
          <div v-else-if="f.type === 'file' || f.type === 'folder'" class="prd-path">
            <n-input
              v-model:value="values[f.key]"
              size="small"
              :placeholder="f.placeholder || (f.type === 'folder' ? '选择或填写目录路径' : '选择或填写文件路径')"
            />
            <n-button size="small" @click="pick(f)">选择</n-button>
          </div>
          <n-input
            v-else
            v-model:value="values[f.key]"
            size="small"
            :placeholder="f.placeholder || ''"
          />
          <div v-if="f.hint" class="prd-hint">{{ f.hint }}</div>
        </div>
      </div>

      <div v-if="!safeFields.length" class="prd-empty">这个插件没有需要填写的参数。</div>

      <div v-if="error" class="prd-error">{{ error }}</div>

      <div v-if="canRemember && safeFields.length" class="prd-remember">
        <n-checkbox v-model:checked="remember">记住这些值,下次默认使用</n-checkbox>
      </div>
    </div>

    <template #footer>
      <div class="prd-footer">
        <n-button size="small" @click="cancel">取消</n-button>
        <n-button size="small" type="primary" @click="submit">{{ submitLabel || '运行' }}</n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'

const props = defineProps({
  show: { type: Boolean, default: false },
  title: { type: String, default: '插件参数' },
  description: { type: String, default: '' },
  fields: { type: Array, default: () => [] },
  initial: { type: Object, default: () => ({}) },
  submitLabel: { type: String, default: '运行' },
  canRemember: { type: Boolean, default: false },
})

const emit = defineEmits(['update:show', 'submit', 'cancel'])

const KNOWN_TYPES = new Set(['text', 'number', 'bool', 'select', 'textarea', 'file', 'folder'])

const values = ref({})
const remember = ref(true)
const error = ref('')

const safeFields = computed(() =>
  (Array.isArray(props.fields) ? props.fields : [])
    .filter((f) => f && f.key)
    .map((f) => {
      const type = KNOWN_TYPES.has(f.type) ? f.type : 'text'
      const options = (Array.isArray(f.options) ? f.options : [])
        .map((o) => (o && typeof o === 'object'
          ? { label: String(o.label ?? o.value ?? ''), value: o.value ?? o.label }
          : { label: String(o), value: o }))
        .filter((o) => o.label !== '')
      return {
        key: String(f.key),
        label: String(f.label || f.key),
        type,
        options,
        placeholder: String(f.placeholder || ''),
        hint: String(f.hint || ''),
        required: !!f.required,
        accept: f.accept,
        default: f.default,
      }
    })
)

const buildValues = () => {
  const out = {}
  for (const f of safeFields.value) {
    const saved = props.initial?.[f.key]
    if (saved !== undefined && saved !== null) {
      out[f.key] = saved
    } else if (f.default !== undefined && f.default !== null) {
      out[f.key] = f.default
    } else if (f.type === 'bool') {
      out[f.key] = false
    } else if (f.type === 'number') {
      out[f.key] = null
    } else {
      out[f.key] = ''
    }
  }
  return out
}

watch(
  () => props.show,
  (visible) => {
    if (!visible) return
    values.value = buildValues()
    remember.value = true
    error.value = ''
  },
  { immediate: true }
)

const extensionsOf = (accept) => {
  if (Array.isArray(accept)) return accept.map((x) => String(x).trim()).filter(Boolean)
  if (typeof accept === 'string') return accept.split(/[,;]/).map((x) => x.trim()).filter(Boolean)
  return []
}

const pick = async (f) => {
  try {
    const exts = extensionsOf(f.accept)
    const picked = await open({
      multiple: false,
      directory: f.type === 'folder',
      ...(exts.length ? { filters: [{ name: '文件', extensions: exts }] } : {}),
    })
    const path = Array.isArray(picked) ? picked[0] : picked
    if (path) values.value[f.key] = String(path)
  } catch (e) {
    error.value = '选择失败:' + String(e?.message || e)
  }
}

const submit = () => {
  for (const f of safeFields.value) {
    if (!f.required) continue
    const v = values.value[f.key]
    if (v === undefined || v === null || String(v).trim() === '') {
      error.value = `请填写「${f.label}」`
      return
    }
  }
  error.value = ''
  emit('submit', { values: { ...values.value }, remember: !!remember.value })
}

const cancel = () => {
  error.value = ''
  emit('cancel')
}

const onShowChange = (visible) => {
  if (visible) {
    emit('update:show', true)
    return
  }
  cancel()
}
</script>

<style scoped>
.prd-modal {
  width: min(580px, 92vw);
}
.prd-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.prd-desc {
  margin: 0 0 4px;
  font-size: 13px;
  opacity: 0.7;
  white-space: pre-wrap;
}
.prd-row {
  display: flex;
  align-items: flex-start;
  gap: 12px;
}
.prd-label {
  flex: 0 0 150px;
  padding-top: 4px;
  font-size: 13px;
  opacity: 0.85;
}
.prd-req {
  color: var(--n-error-color, #e88080);
  margin-left: 2px;
}
.prd-control {
  flex: 1 1 auto;
  min-width: 0;
}
.prd-number {
  width: 150px;
}
.prd-select {
  width: 220px;
}
.prd-path {
  display: flex;
  align-items: center;
  gap: 8px;
}
.prd-path .n-input {
  flex: 1 1 auto;
}
.prd-hint {
  margin-top: 4px;
  font-size: 12px;
  opacity: 0.55;
}
.prd-remember {
  margin-top: 2px;
}
.prd-error {
  font-size: 12px;
  color: var(--n-error-color, #e88080);
}
.prd-empty {
  font-size: 13px;
  opacity: 0.6;
}
.prd-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
