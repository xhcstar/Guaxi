<template>
  <n-drawer :show="show" :width="520" placement="right" @update:show="(v) => emit('update:show', v)">
    <n-drawer-content v-if="form" closable>
      <template #header>
        <div class="td-head">
          <span class="td-kind" :class="'k-' + form.kind">{{ kindLabel(form.kind) }}</span>
          <span class="td-title">{{ form.title || '(无标题)' }}</span>
        </div>
      </template>

      <div class="td-body">
        <n-form label-placement="top" size="small" :show-feedback="false" class="td-form">
          <n-form-item label="标题">
            <n-input v-model:value="form.title" placeholder="任务标题" />
          </n-form-item>

          <div class="td-grid">
            <n-form-item label="形态">
              <n-select v-model:value="form.kind" :options="kindOptions" />
            </n-form-item>
            <n-form-item label="状态">
              <n-select v-model:value="form.status" :options="statusOptions" />
            </n-form-item>
            <n-form-item label="优先级">
              <n-select v-model:value="form.priority" :options="priorityOptions" />
            </n-form-item>
            <n-form-item label="截止">
              <n-date-picker v-model:formatted-value="form.deadline" value-format="yyyy-MM-dd" type="date" clearable placeholder="不设截止" />
            </n-form-item>
            <n-form-item label="置顶到今日">
              <n-date-picker v-model:formatted-value="form.focus" value-format="yyyy-MM-dd" type="date" clearable placeholder="不置顶" />
            </n-form-item>
            <n-form-item label="投入（分钟）">
              <n-input-number v-model:value="form.spent" :min="0" :step="5" placeholder="0" style="width:100%" />
            </n-form-item>
            <n-form-item label="重复">
              <n-select v-model:value="form.repeat" :options="repeatOptions" clearable placeholder="不重复" />
            </n-form-item>
            <n-form-item label="父任务 / 目标">
              <n-input v-model:value="form.parentId" placeholder="ID 或内部 ID" />
              <n-select
                v-if="goalOptions.length"
                v-model:value="form.parentId"
                size="small"
                clearable
                :options="goalOptions"
                placeholder="或直接选择一个目标"
                style="margin-top:6px"
              />
            </n-form-item>
          </div>

          <div v-if="isWork" class="td-grid">
            <n-form-item label="严重度">
              <n-select v-model:value="form.severity" :options="severityOptions" clearable />
            </n-form-item>
            <n-form-item label="来源">
              <n-select v-model:value="form.origin" :options="originOptions" clearable />
            </n-form-item>
            <n-form-item label="修复状态">
              <n-select v-model:value="form.fixStatus" :options="fixOptions" clearable />
            </n-form-item>
            <n-form-item label="影响版本">
              <n-input v-model:value="form.affectedRelease" placeholder="rel_app_..." />
            </n-form-item>
          </div>

          <n-form-item label="正文 / 详情">
            <n-input v-model:value="form.content" type="textarea" :autosize="{ minRows: 5, maxRows: 14 }" placeholder="Markdown 文本" />
          </n-form-item>

          <n-form-item label="笔记">
            <n-input v-model:value="form.notes" type="textarea" :autosize="{ minRows: 3, maxRows: 8 }" placeholder="补充说明" :disabled="notesIsFile" />
          </n-form-item>

          <div v-if="form.kind === 'habit'" class="td-habit">
            <div class="td-habit-head">
              <span>打卡记录 · {{ form.checkins.length }} 次</span>
              <n-button size="small" :type="checkedToday ? 'primary' : 'default'" @click="toggleToday">
                {{ checkedToday ? '取消今日打卡' : '今日打卡' }}
              </n-button>
            </div>
            <div class="td-checkins">
              <span v-for="d in form.checkins" :key="d" class="td-checkin">{{ d }}</span>
              <span v-if="!form.checkins.length" class="td-muted">还没有打卡</span>
            </div>
          </div>
        </n-form>

        <div class="td-source">
          <span class="td-muted">{{ fileName }}</span>
          <n-button text size="tiny" @click="openSource">打开源文件</n-button>
        </div>
      </div>

      <template #footer>
        <div class="td-footer">
          <n-button size="small" :type="deleteArmed ? 'error' : 'default'" :loading="deleting" @click="onDelete">
            {{ deleteArmed ? '确认删除' : '删除' }}
          </n-button>
          <span class="td-spacer" />
          <n-button size="small" @click="emit('update:show', false)">取消</n-button>
          <n-button size="small" type="primary" :loading="saving" @click="save">保存到 .org</n-button>
        </div>
      </template>
    </n-drawer-content>
  </n-drawer>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import {
  NDrawer, NDrawerContent, NForm, NFormItem, NInput, NInputNumber, NSelect,
  NDatePicker, NButton, useMessage,
} from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps({
  show: { type: Boolean, default: false },
  task: { type: Object, default: null },
  goals: { type: Array, default: () => [] },
})
const emit = defineEmits(['update:show', 'saved', 'deleted', 'open-source'])
const message = useMessage()

const KIND_OPTIONS = [
  { label: '任务', value: 'task' }, { label: '习惯', value: 'habit' },
  { label: '目标', value: 'goal' }, { label: '复盘', value: 'review' },
]
const kindOptions = KIND_OPTIONS
const kindLabel = (k) => KIND_OPTIONS.find((o) => o.value === k)?.label || '任务'
const statusOptions = [
  { label: '待办 TODO', value: 'TODO' }, { label: '下一步 NEXT', value: 'NEXT' },
  { label: '进行中 DOING', value: 'DOING' }, { label: '等待 WAITING', value: 'WAITING' },
  { label: '暂停 PAUSED', value: 'PAUSED' }, { label: '搁置 HOLD', value: 'HOLD' },
  { label: '已完成 DONE', value: 'DONE' }, { label: '已取消 CANCELLED', value: 'CANCELLED' },
]
const priorityOptions = [
  { label: '无', value: '' }, { label: 'A', value: 'A' }, { label: 'B', value: 'B' }, { label: 'C', value: 'C' },
]
const repeatOptions = [
  { label: '每天', value: '每天' }, { label: '工作日', value: '工作日' },
  { label: '每周', value: '每周' }, { label: '每月', value: '每月' },
]
const severityOptions = ['critical', 'major', 'normal', 'minor', 'trivial'].map((v) => ({ label: v, value: v }))
const originOptions = ['dev', 'QA', 'customer', 'other'].map((v) => ({ label: v, value: v }))
const fixOptions = ['debug', 'test', 'review', 'approval', 'fixed', 'wontfix', 'doc', 'other'].map((v) => ({ label: v, value: v }))

const form = reactive({
  id: '', title: '', kind: 'task', status: 'TODO', priority: '', deadline: null, focus: null,
  repeat: '', spent: 0, parentId: '', severity: '', origin: '', fixStatus: '', affectedRelease: '',
  content: '', notes: '', checkins: [],
})
const notesIsFile = ref(false)
const saving = ref(false)
const deleting = ref(false)
const deleteArmed = ref(false)

const pad = (n) => String(n).padStart(2, '0')
const isoToCn = (iso) => {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''))
  return m ? `${m[1]}年${m[2]}月${m[3]}日` : ''
}
const cnToIso = (s) => {
  const m = /(\d{4})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/.exec(String(s || ''))
  return m ? `${m[1]}-${pad(m[2])}-${pad(m[3])}` : ''
}
const todayIso = () => {
  const d = new Date()
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}
const isWork = computed(() => props.task?.manageType === 'work' || props.task?.taskType === 'bug')
const checkedToday = computed(() => form.checkins.some((d) => cnToIso(d) === todayIso()))
const fileName = computed(() => {
  const p = String(props.task?.filePath || '')
  return p ? p.split(/[\\/]/).pop() : ''
})
const goalOptions = computed(() => props.goals
  .filter((g) => g && g.id && g.id !== props.task?.internalId)
  .map((g) => ({ label: g.title, value: g.id })))

watch(() => props.task, (t) => {
  if (!t) return
  deleteArmed.value = false
  Object.assign(form, {
    id: t.internalId || '',
    title: t.description || '',
    kind: t.kind || 'task',
    status: String(t.status || 'TODO').toUpperCase(),
    priority: t.priority || '',
    deadline: cnToIso(t.deadline) || null,
    focus: cnToIso(t.focus) || null,
    repeat: t.repeat || '',
    spent: Number(t.spent) || 0,
    parentId: t.parentId || '',
    severity: t.severity || '',
    origin: t.origin || '',
    fixStatus: t.fixStatus || '',
    affectedRelease: t.affectedRelease || '',
    content: t.content || '',
    notes: t.notes?.kind === 'inline' ? (t.notes.value || '') : '',
    checkins: Array.isArray(t.checkins) ? t.checkins.slice() : [],
  })
  notesIsFile.value = t.notes?.kind === 'file'
}, { immediate: true })

watch(() => props.show, (v) => { if (v) deleteArmed.value = false })

const toggleToday = () => {
  const today = todayIso()
  const rest = form.checkins.filter((d) => cnToIso(d) !== today)
  form.checkins = checkedToday.value ? rest : [...rest, today]
}

const save = async () => {
  if (!form.id) return
  saving.value = true
  try {
    const fields = {
      description: form.title.trim(),
      kind: form.kind,
      status: String(form.status || 'TODO').toUpperCase(),
      priority: form.priority || '',
      deadline: form.deadline ? isoToCn(form.deadline) : '',
      focus: form.focus || '',
      repeat: form.repeat || '',
      spent: Number(form.spent) || 0,
      parent_id: form.parentId || '',
      severity: form.severity || '',
      origin: form.origin || '',
      fix_status: form.fixStatus || '',
      affected_release: form.affectedRelease || '',
      content: form.content || '',
      checkins: form.checkins,
    }
    if (!notesIsFile.value) fields.notes = form.notes || ''
    await invoke('task_update_item', { input: { id: form.id, fields } })
    message.success('已保存到 .org')
    emit('saved')
    emit('update:show', false)
  } catch (e) {
    message.error('保存失败：' + (e?.message || e))
  } finally {
    saving.value = false
  }
}

const onDelete = async () => {
  if (!form.id) return
  if (!deleteArmed.value) {
    deleteArmed.value = true
    return
  }
  deleting.value = true
  try {
    await invoke('task_delete_item', { id: form.id })
    message.success('任务已删除')
    emit('deleted')
    emit('update:show', false)
  } catch (e) {
    message.error('删除失败：' + (e?.message || e))
  } finally {
    deleting.value = false
    deleteArmed.value = false
  }
}

const openSource = async () => {
  if (!form.id) return
  try {
    await invoke('task_open_source', { id: form.id })
    emit('open-source')
  } catch (e) {
    message.warning('打不开源文件：' + (e?.message || e))
  }
}
</script>

<style scoped>
.td-head { display: flex; align-items: center; gap: 10px; min-width: 0; }
.td-kind { font-size: 11px; padding: 2px 8px; border-radius: 10px; background: var(--guaxi-header-bg); color: var(--guaxi-text-secondary); }
.td-kind.k-habit { color: #b07a12; background: rgba(240, 160, 32, 0.14); }
.td-kind.k-goal { color: #3f6ea8; background: rgba(63, 110, 168, 0.14); }
.td-kind.k-review { color: #7a5aa8; background: rgba(122, 90, 168, 0.14); }
.td-title { font-size: 16px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.td-body { display: flex; flex-direction: column; gap: 14px; }
.td-form { display: flex; flex-direction: column; gap: 4px; }
.td-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 12px; }
.td-habit { border: 1px solid var(--guaxi-border); border-radius: 10px; padding: 10px 12px; background: var(--guaxi-panel-bg); }
.td-habit-head { display: flex; align-items: center; justify-content: space-between; font-size: 13px; margin-bottom: 8px; }
.td-checkins { display: flex; flex-wrap: wrap; gap: 6px; max-height: 132px; overflow: auto; }
.td-checkin { font-size: 12px; padding: 2px 8px; border-radius: 8px; background: var(--guaxi-header-bg); color: var(--guaxi-text-secondary); }
.td-muted { font-size: 12px; color: var(--guaxi-text-secondary); }
.td-source { display: flex; align-items: center; justify-content: space-between; border-top: 1px dashed var(--guaxi-border); padding-top: 10px; }
.td-footer { display: flex; align-items: center; gap: 8px; }
.td-spacer { flex: 1; }
</style>
