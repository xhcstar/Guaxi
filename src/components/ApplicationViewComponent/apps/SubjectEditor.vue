<template>
  <div class="subj-ed">
    <div class="subj-head">
      <span class="subj-title">{{ title }}</span>
      <span class="subj-hint">留空的项目不会写入证书</span>
      <span class="subj-spacer"></span>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="subj-ib" @click="resetDefaults"><Icon :icon="resetIcon" width="16" height="16" /></button>
      </template>恢复软件默认主题信息</n-tooltip>
    </div>
    <div class="subj-paste">
      <n-input v-model:value="pasteText" size="small" placeholder="可选：粘贴 openssl 主题串，如 /C=CN/ST=Shannxi/L=xi'an/O=XA/OU=Dev/CN=Root CA" />
      <n-tooltip trigger="hover"><template #trigger>
        <button class="subj-ib" @click="parseSubject"><Icon :icon="wandIcon" width="16" height="16" /></button>
      </template>解析并填入下面的字段（会覆盖同名字段）</n-tooltip>
    </div>
    <div class="subj-grid">
      <label v-for="f in fields" :key="f.key" class="subj-field">
        <span class="subj-label">{{ f.label }}<span v-if="f.required" class="subj-req">*</span></span>
        <n-input v-model:value="local[f.key]" size="small" :placeholder="f.placeholder" />
      </label>
    </div>
  </div>
</template>

<script setup>
import { reactive, ref, watch, nextTick } from 'vue'
import { Icon } from '@iconify/vue'
import resetIcon from '@iconify-icons/lucide/rotate-ccw'
import wandIcon from '@iconify-icons/lucide/wand-2'

const props = defineProps({
  modelValue: { type: Object, default: () => ({}) },
  defaults: { type: Object, default: () => ({}) },
  title: { type: String, default: '主题信息' },
})
const emit = defineEmits(['update:modelValue'])

const fields = [
  { key: 'commonName', label: '通用名 CN', required: true, placeholder: 'server.example.com' },
  { key: 'country', label: '国家/地区 C', placeholder: 'CN' },
  { key: 'state', label: '省/州 ST', placeholder: 'Shannxi' },
  { key: 'locality', label: '城市 L', placeholder: "xi'an" },
  { key: 'org', label: '组织 O', placeholder: 'XA' },
  { key: 'orgUnit', label: '部门 OU', placeholder: 'Dev' },
  { key: 'email', label: '邮箱 emailAddress', placeholder: 'admin@example.com' },
  { key: 'serialNumber', label: '序列号 serialNumber', placeholder: '' },
  { key: 'street', label: '街道 street', placeholder: '' },
  { key: 'postalCode', label: '邮编 postalCode', placeholder: '' },
  { key: 'title', label: '头衔 title', placeholder: '' },
  { key: 'givenName', label: '名 givenName', placeholder: '' },
  { key: 'surname', label: '姓 surname', placeholder: '' },
  { key: 'initials', label: '缩写 initials', placeholder: '' },
  { key: 'pseudonym', label: '笔名 pseudonym', placeholder: '' },
  { key: 'generationQualifier', label: '后缀 generationQualifier', placeholder: '' },
  { key: 'dnQualifier', label: 'DN 限定 dnQualifier', placeholder: '' },
  { key: 'organizationIdentifier', label: '组织标识 organizationIdentifier', placeholder: '' },
]
const pasteText = ref('')
const local = reactive({ ...props.defaults, ...props.modelValue })
let syncing = false

watch(local, () => {
  if (syncing) return
  emit('update:modelValue', { ...local })
}, { deep: true })

watch(() => props.modelValue, (v) => {
  if (!v || syncing) return
  syncing = true
  Object.assign(local, v)
  nextTick(() => { syncing = false })
}, { deep: true })

function resetDefaults() {
  for (const k of Object.keys(local)) delete local[k]
  Object.assign(local, props.defaults)
}

const ALIASES = {
  C: 'country', ST: 'state', L: 'locality', O: 'org', OU: 'orgUnit', CN: 'commonName',
  emailAddress: 'email', email: 'email', serialNumber: 'serialNumber', street: 'street',
  postalCode: 'postalCode', title: 'title', GN: 'givenName', givenName: 'givenName',
  SN: 'surname', surname: 'surname', initials: 'initials', pseudonym: 'pseudonym',
  generationQualifier: 'generationQualifier', dnQualifier: 'dnQualifier',
  organizationIdentifier: 'organizationIdentifier',
}
function parseSubject() {
  const s = String(pasteText.value || '').trim()
  if (!s) return
  const parts = s.split('/').filter(Boolean)
  for (const part of parts) {
    const i = part.indexOf('=')
    if (i <= 0) continue
    const key = part.slice(0, i).trim()
    const val = part.slice(i + 1).trim().replace(/^"(.*)"$/, '$1')
    const field = ALIASES[key]
    if (field) local[field] = val
  }
}
</script>

<style scoped>
.subj-ed { margin: 8px 0 12px; padding: 10px 12px; border: 1px solid var(--guaxi-border-weak, rgba(0,0,0,.08)); border-radius: var(--tw-radius-sm); background: color-mix(in srgb, var(--guaxi-panel-bg) 35%, transparent); }
.subj-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.subj-title { font-size: 15px; font-weight: 600; color: var(--tw-accent); }
.subj-hint { font-size: 13px; color: var(--guaxi-text-secondary); }
.subj-spacer { flex: 1 1 auto; }
.subj-ib { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; flex: 0 0 auto; padding: 0; border: none; border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; }
.subj-ib:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.subj-paste { display: flex; gap: 6px; align-items: center; margin-bottom: 8px; }
.subj-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px 12px; }
.subj-field { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.subj-label { font-size: 13px; color: var(--guaxi-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.subj-req { color: #e5484d; margin-left: 2px; }
</style>
