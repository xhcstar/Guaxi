<template>
  <div class="task-manager-page">
    <!-- 顶部工具栏 -->
    <header class="tm-toolbar">
      <div class="tm-toolbar-left">
        <n-icon size="22" class="tm-logo"><ClipboardTaskListLtr20Filled /></n-icon>
        <span class="tm-title">任务管理</span>
        <n-tabs v-model:value="category" type="segment" size="small" class="tm-cat-tabs" @update:value="reload">
          <n-tab name="all">全部</n-tab>
          <n-tab name="work">工作</n-tab>
          <n-tab name="life">生活</n-tab>
          <n-tab name="study">学习</n-tab>
        </n-tabs>
      </div>
      <div class="tm-toolbar-right">
        <n-input v-model:value="keyword" placeholder="搜索任务…" clearable size="small" class="tm-search" @update:value="onSearchInput">
          <template #prefix><n-icon :component="Search20Regular" /></template>
        </n-input>
        <n-button-group size="small" class="tm-view-switch">
          <n-button :type="view === 'kanban' ? 'primary' : 'default'" @click="view = 'kanban'">
            <template #icon><n-icon :component="Board20Regular" /></template>看板
          </n-button>
          <n-button :type="view === 'calendar' ? 'primary' : 'default'" @click="view = 'calendar'">
            <template #icon><n-icon :component="CalendarLtr20Regular" /></template>日历
          </n-button>
          <n-button :type="view === 'stats' ? 'primary' : 'default'" @click="view = 'stats'">
            <template #icon><n-icon :component="DataPie20Regular" /></template>统计
          </n-button>
        </n-button-group>
        <n-button size="small" @click="showImport = true">
          <template #icon><n-icon :component="ArrowImport20Regular" /></template>导入
        </n-button>
        <n-button size="small" @click="showExport = true">
          <template #icon><n-icon :component="ArrowExportLtr20Regular" /></template>导出报表
        </n-button>
        <n-button size="small" tertiary @click="reload">
          <template #icon><n-icon :component="ArrowSync20Regular" /></template>刷新
        </n-button>
      </div>
    </header>

    <main class="tm-body">
        <!-- 看板 -->
        <section v-show="view === 'kanban'" class="tm-kanban">
          <div v-for="col in columns" :key="col.key" class="tm-column">
            <div class="tm-column-head" :style="{ borderColor: col.color }">
              <span class="tm-col-dot" :style="{ background: col.color }" />
              <span class="tm-col-name">{{ col.label }}</span>
              <n-tag size="tiny" round :bordered="false" class="tm-col-count">{{ grouped[col.key].length }}</n-tag>
            </div>
            <draggable
              :list="grouped[col.key]"
              group="tasks"
              item-key="id"
              class="tm-cards"
              :animation="160"
              @change="(e) => onDragChange(col.key, e)"
            >
              <template #item="{ element }">
                <div class="tm-card" :class="{ overdue: isOverdue(element) }" @click="openEditor(element)">
                  <div class="tm-card-top">
                    <span v-if="element.priority" class="tm-pri" :class="'pri-' + element.priority">{{ element.priority }}</span>
                    <span class="tm-card-title">{{ element.title || '(无标题)' }}</span>
                  </div>
                  <div class="tm-card-meta">
                    <n-tag size="tiny" :bordered="false" :color="catTagColor(element.category)">{{ catLabel(element.category) }}</n-tag>
                    <span v-if="element.deadline" class="tm-date deadline"><n-icon :component="Flag16Regular" />{{ element.deadline }}</span>
                    <span v-else-if="element.scheduled" class="tm-date"><n-icon :component="CalendarClock16Regular" />{{ element.scheduled }}</span>
                    <span v-if="element.clock_minutes > 0" class="tm-clock">{{ fmtClock(element.clock_minutes) }}</span>
                  </div>
                  <div v-if="element.tags && element.tags.length" class="tm-tags">
                    <span v-for="t in element.tags" :key="t" class="tm-tag">#{{ t }}</span>
                  </div>
                </div>
              </template>
            </draggable>
            <div v-if="!grouped[col.key].length" class="tm-empty-col">暂无任务</div>
          </div>
        </section>

        <!-- 日历 -->
        <section v-show="view === 'calendar'" class="tm-calendar">
          <FullCalendar ref="calRef" :options="calendarOptions" />
        </section>

        <!-- 统计 -->
        <section v-show="view === 'stats'" class="tm-stats">
          <div class="tm-stat-cards">
            <div class="tm-stat-card">
              <div class="tm-stat-num">{{ items.length }}</div>
              <div class="tm-stat-label">任务总数</div>
            </div>
            <div class="tm-stat-card">
              <div class="tm-stat-num" style="color:#18a058">{{ doneCount }}</div>
              <div class="tm-stat-label">已完成</div>
            </div>
            <div class="tm-stat-card">
              <div class="tm-stat-num" style="color:#2080f0">{{ doingCount }}</div>
              <div class="tm-stat-label">进行中</div>
            </div>
            <div class="tm-stat-card">
              <div class="tm-stat-num">{{ completionRate }}%</div>
              <div class="tm-stat-label">完成率</div>
            </div>
            <div class="tm-stat-card">
              <div class="tm-stat-num">{{ fmtClock(totalClock) }}</div>
              <div class="tm-stat-label">累计工时</div>
            </div>
          </div>
          <div class="tm-charts">
            <div class="tm-chart-box"><VChart :option="statusChart" autoresize /></div>
            <div class="tm-chart-box"><VChart :option="categoryChart" autoresize /></div>
            <div class="tm-chart-box tm-chart-wide"><VChart :option="trendChart" autoresize /></div>
          </div>
        </section>

        <div v-if="loading" class="tm-loading-overlay"><n-spin :size="28" /></div>
    </main>

    <!-- 任务编辑抽屉 -->
    <n-drawer v-model:show="showEditor" :width="420" placement="right">
      <n-drawer-content title="编辑任务" closable>
        <n-form v-if="editing" label-placement="top" size="small">
          <n-form-item label="状态">
            <n-select v-model:value="editForm.status" :options="statusOptions" />
          </n-form-item>
          <n-form-item label="标题">
            <n-input v-model:value="editForm.title" type="textarea" :autosize="{ minRows: 1, maxRows: 3 }" />
          </n-form-item>
          <n-form-item label="计划日期 (SCHEDULED)">
            <n-date-picker v-model:formatted-value="editForm.scheduled" value-format="yyyy-MM-dd" type="date" clearable style="width:100%" />
          </n-form-item>
          <n-form-item label="截止日期 (DEADLINE)">
            <n-date-picker v-model:formatted-value="editForm.deadline" value-format="yyyy-MM-dd" type="date" clearable style="width:100%" />
          </n-form-item>
          <n-form-item label="备注 / 正文">
            <n-input v-model:value="editForm.notes" type="textarea" :autosize="{ minRows: 3, maxRows: 10 }" placeholder="任务说明、备注…" />
          </n-form-item>
          <div class="tm-edit-info">
            <n-text depth="3" style="font-size:12px;">
              {{ editing.file_path }} · 第 {{ editing.line }} 行
            </n-text>
          </div>
        </n-form>
        <template #footer>
          <n-space>
            <n-button @click="showEditor = false">取消</n-button>
            <n-button type="primary" :loading="saving" @click="saveEditor">保存到 .org</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>

    <!-- 导入抽屉 -->
    <n-drawer v-model:show="showImport" :width="460" placement="right">
      <n-drawer-content title="导入任务" closable>
        <n-form label-placement="top" size="small">
          <n-form-item label="目标分类">
            <n-select v-model:value="importForm.category" :options="categoryOptions" />
          </n-form-item>
          <n-form-item label="目标文件名">
            <n-input v-model:value="importForm.target_file_name" placeholder="inbox.org" />
          </n-form-item>
          <n-form-item label="源格式">
            <n-radio-group v-model:value="importForm.source_kind">
              <n-radio value="org">Org 文本</n-radio>
              <n-radio value="json">JSON (TaskItem[])</n-radio>
            </n-radio-group>
          </n-form-item>
          <n-form-item label="内容">
            <n-input v-model:value="importForm.content" type="textarea" :autosize="{ minRows: 6, maxRows: 16 }" placeholder="粘贴 .org 文本或 JSON…" />
          </n-form-item>
          <n-button size="small" tertiary @click="pickImportFile">从文件读取…</n-button>
        </n-form>
        <template #footer>
          <n-space>
            <n-button @click="showImport = false">取消</n-button>
            <n-button type="primary" :loading="importing" @click="doImport">导入并扫描</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>

    <!-- 导出抽屉 -->
    <n-drawer v-model:show="showExport" :width="420" placement="right">
      <n-drawer-content title="导出报表" closable>
        <n-form label-placement="top" size="small">
          <n-form-item label="报表类型">
            <n-select v-model:value="exportForm.kind" :options="reportKindOptions" />
          </n-form-item>
          <n-form-item label="锚点日期">
            <n-date-picker v-model:formatted-value="exportForm.date" value-format="yyyy-MM-dd" type="date" style="width:100%" />
          </n-form-item>
          <template v-if="exportForm.kind === 'custom'">
            <n-form-item label="开始日期">
              <n-date-picker v-model:formatted-value="exportForm.from" value-format="yyyy-MM-dd" type="date" style="width:100%" />
            </n-form-item>
            <n-form-item label="结束日期">
              <n-date-picker v-model:formatted-value="exportForm.to" value-format="yyyy-MM-dd" type="date" style="width:100%" />
            </n-form-item>
          </template>
          <n-form-item label="格式">
            <n-radio-group v-model:value="exportForm.format">
              <n-radio value="md">Markdown</n-radio>
              <n-radio value="json">JSON</n-radio>
            </n-radio-group>
          </n-form-item>
        </n-form>
        <n-card v-if="exportPreview" size="small" title="预览" class="tm-export-preview">
          <pre>{{ exportPreview.slice(0, 2000) }}</pre>
        </n-card>
        <template #footer>
          <n-space>
            <n-button @click="genPreview" :loading="exporting">生成预览</n-button>
            <n-button type="primary" :disabled="!exportPreview" @click="saveReport">保存到文件…</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>
  </div>
</template>

<script setup>
import { createLogger } from '../utils/logger'
import { ref, reactive, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import {
  NTabs, NTab, NInput, NButton, NButtonGroup, NIcon, NTag, NSpin, NDrawer, NDrawerContent,
  NForm, NFormItem, NSelect, NDatePicker, NSpace, NText, NRadioGroup, NRadio, NCard, useMessage,
} from 'naive-ui'
import {
  ClipboardTaskListLtr20Filled, Search20Regular, Board20Regular, CalendarLtr20Regular,
  DataPie20Regular, ArrowImport20Regular, ArrowExportLtr20Regular, ArrowSync20Regular,
  Flag16Regular, CalendarClock16Regular,
} from '@vicons/fluent'
import { invoke } from '@tauri-apps/api/core'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import draggable from 'vuedraggable'
import FullCalendar from '@fullcalendar/vue3'
import dayGridPlugin from '@fullcalendar/daygrid'
import timeGridPlugin from '@fullcalendar/timegrid'
import listPlugin from '@fullcalendar/list'
import interactionPlugin from '@fullcalendar/interaction'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { PieChart, BarChart, LineChart } from 'echarts/charts'
import { TitleComponent, TooltipComponent, LegendComponent, GridComponent } from 'echarts/components'
import VChart from 'vue-echarts'

use([CanvasRenderer, PieChart, BarChart, LineChart, TitleComponent, TooltipComponent, LegendComponent, GridComponent])

const logger = createLogger('TaskManagerPage')
const message = useMessage()
const route = useRoute()

const isDark = ref(false)
const applyTheme = () => {
  const t = String(route.query.theme || '').toLowerCase()
  const theme = t === 'dark' || t === 'light' ? t : (document.documentElement.dataset.guaxiTheme || 'light')
  isDark.value = theme === 'dark'
  document.documentElement.dataset.guaxiTheme = theme
  document.documentElement.classList.toggle('dark-mode', theme === 'dark')
  document.body?.setAttribute?.('data-guaxi-theme', theme)
}

// ── 状态 ──
const view = ref('kanban')
const category = ref('all')
const keyword = ref('')
const items = ref([])
const loading = ref(false)

const CAT_MAP = { work: '工作', life: '生活', study: '学习' }
const catLabel = (c) => CAT_MAP[c] || c || '未分类'
const catTagColor = (c) => {
  const m = { work: { color: 'rgba(124,58,237,0.14)', textColor: '#7c3aed' }, life: { color: 'rgba(5,150,105,0.14)', textColor: '#059669' }, study: { color: 'rgba(37,99,235,0.14)', textColor: '#2563eb' } }
  return m[c] || undefined
}
const categoryOptions = [
  { label: '工作', value: 'work' }, { label: '生活', value: 'life' }, { label: '学习', value: 'study' },
]

// 看板列定义：状态关键字 → 列
const columns = [
  { key: 'todo', label: '待办', color: '#909399', statuses: ['TODO', ''], canonical: 'TODO' },
  { key: 'doing', label: '进行中', color: '#2080f0', statuses: ['DOING', 'NEXT'], canonical: 'DOING' },
  { key: 'paused', label: '暂停', color: '#f0a020', statuses: ['PAUSED', 'WAITING', 'HOLD'], canonical: 'PAUSED' },
  { key: 'done', label: '已完成', color: '#18a058', statuses: ['DONE', 'CANCELLED'], canonical: 'DONE' },
]
const colOf = (status) => {
  const s = String(status || '').toUpperCase()
  for (const c of columns) if (c.statuses.includes(s)) return c.key
  return 'todo'
}

const grouped = reactive({ todo: [], doing: [], paused: [], done: [] })
const regroup = () => {
  grouped.todo = []; grouped.doing = []; grouped.paused = []; grouped.done = []
  for (const it of items.value) grouped[colOf(it.status)].push(it)
}

const statusOptions = [
  { label: 'TODO 待办', value: 'TODO' },
  { label: 'NEXT 下一步', value: 'NEXT' },
  { label: 'DOING 进行中', value: 'DOING' },
  { label: 'WAITING 等待', value: 'WAITING' },
  { label: 'PAUSED 暂停', value: 'PAUSED' },
  { label: 'HOLD 搁置', value: 'HOLD' },
  { label: 'DONE 已完成', value: 'DONE' },
  { label: 'CANCELLED 已取消', value: 'CANCELLED' },
]

const fmtClock = (min) => {
  const m = Number(min) || 0
  if (m < 60) return `${m}m`
  const h = Math.floor(m / 60), r = m % 60
  return r ? `${h}h${r}m` : `${h}h`
}
const todayStr = () => {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}
const isOverdue = (it) => it.deadline && !it.done && it.deadline < todayStr()

// ── 数据加载 ──
let searchTimer = null
const onSearchInput = () => {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(reload, 300)
}

const reload = async () => {
  loading.value = true
  try {
    const query = {
      category: category.value === 'all' ? null : category.value,
      keyword: keyword.value || null,
    }
    const res = await invoke('task_query', { query })
    items.value = Array.isArray(res) ? res : []
    regroup()
  } catch (e) {
    logger.error('加载任务失败:', e)
    message.error('加载任务失败：' + (e?.message || e))
  } finally {
    loading.value = false
  }
}

// ── 拖拽改状态 → 回写 .org ──
const onDragChange = async (colKey, evt) => {
  const added = evt?.added
  if (!added) return
  const el = added.element
  const target = columns.find((c) => c.key === colKey)
  if (!target || !el) return
  const newStatus = target.canonical
  if (String(el.status || '').toUpperCase() === newStatus) return
  try {
    const updated = await invoke('task_update_item', { input: { id: el.id, status: newStatus } })
    // 后端返回该文件最新 items；用其同步本地该任务状态
    const fresh = Array.isArray(updated) ? updated.find((x) => x.id === el.id) : null
    el.status = fresh ? fresh.status : newStatus
    el.done = fresh ? fresh.done : (newStatus === 'DONE')
    message.success('已更新任务状态')
  } catch (e) {
    logger.error('更新状态失败:', e)
    message.error('更新失败：' + (e?.message || e))
    reload()
  }
}

// ── 编辑抽屉 ──
const showEditor = ref(false)
const editing = ref(null)
const saving = ref(false)
const editForm = reactive({ status: '', title: '', scheduled: null, deadline: null, notes: '' })
const openEditor = (it) => {
  editing.value = it
  editForm.status = it.status || 'TODO'
  editForm.title = it.title || ''
  editForm.scheduled = it.scheduled || null
  editForm.deadline = it.deadline || null
  editForm.notes = it.raw_body || ''
  showEditor.value = true
}
const saveEditor = async () => {
  if (!editing.value) return
  saving.value = true
  try {
    const input = {
      id: editing.value.id,
      status: editForm.status,
      title: editForm.title,
      scheduled: editForm.scheduled || '',
      deadline: editForm.deadline || '',
      notes: editForm.notes ?? '',
    }
    await invoke('task_update_item', { input })
    message.success('已保存到 .org')
    showEditor.value = false
    await reload()
  } catch (e) {
    logger.error('保存任务失败:', e)
    message.error('保存失败：' + (e?.message || e))
  } finally {
    saving.value = false
  }
}

// ── 导入 ──
const showImport = ref(false)
const importing = ref(false)
const importForm = reactive({ category: 'work', target_file_name: 'inbox.org', source_kind: 'org', content: '' })
const pickImportFile = async () => {
  try {
    const picked = await openDialog({ multiple: false, filters: [{ name: 'Org/JSON', extensions: ['org', 'json', 'txt'] }] })
    const path = Array.isArray(picked) ? picked[0] : picked
    if (!path) return
    importForm.content = await invoke('task_read_file', { path })
    if (path.toLowerCase().endsWith('.json')) importForm.source_kind = 'json'
  } catch (e) {
    logger.error('读取导入文件失败:', e)
    message.error('读取文件失败')
  }
}
const doImport = async () => {
  importing.value = true
  try {
    const count = await invoke('task_import', {
      input: {
        category: importForm.category,
        target_file_name: importForm.target_file_name,
        source_kind: importForm.source_kind,
        content: importForm.content,
      },
    })
    message.success(`导入完成，共索引 ${count} 个任务`)
    showImport.value = false
    await reload()
  } catch (e) {
    logger.error('导入失败:', e)
    message.error('导入失败：' + (e?.message || e))
  } finally {
    importing.value = false
  }
}

// ── 导出报表 ──
const showExport = ref(false)
const exporting = ref(false)
const exportPreview = ref('')
const exportForm = reactive({ kind: 'weekly', date: todayStr(), from: todayStr(), to: todayStr(), format: 'md' })
const reportKindOptions = [
  { label: '日报', value: 'daily' }, { label: '周报', value: 'weekly' },
  { label: '半年报', value: 'halfyear' }, { label: '年报', value: 'yearly' },
  { label: '自定义区间', value: 'custom' },
]
const genPreview = async () => {
  exporting.value = true
  try {
    exportPreview.value = await invoke('task_export_report', {
      input: { kind: exportForm.kind, date: exportForm.date || todayStr(), from: exportForm.from, to: exportForm.to, format: exportForm.format },
    })
  } catch (e) {
    logger.error('生成报表失败:', e)
    message.error('生成失败：' + (e?.message || e))
  } finally {
    exporting.value = false
  }
}
const saveReport = async () => {
  if (!exportPreview.value) return
  try {
    const ext = exportForm.format === 'json' ? 'json' : 'md'
    const path = await saveDialog({ defaultPath: `task-report-${exportForm.kind}-${exportForm.date}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] })
    if (!path) return
    await invoke('task_write_report_file', { path, content: exportPreview.value })
    message.success('报表已保存')
  } catch (e) {
    logger.error('保存报表失败:', e)
    message.error('保存失败：' + (e?.message || e))
  }
}

// ── 统计 ──
const doneCount = computed(() => items.value.filter((i) => i.done).length)
const doingCount = computed(() => items.value.filter((i) => ['DOING', 'NEXT'].includes(String(i.status).toUpperCase())).length)
const totalClock = computed(() => items.value.reduce((s, i) => s + (Number(i.clock_minutes) || 0), 0))
const completionRate = computed(() => items.value.length ? Math.round((doneCount.value / items.value.length) * 100) : 0)

const axisColor = computed(() => (isDark.value ? '#9da5b4' : '#57606a'))
const statusChart = computed(() => ({
  backgroundColor: 'transparent',
  title: { text: '状态分布', left: 'center', textStyle: { color: axisColor.value, fontSize: 14 } },
  tooltip: { trigger: 'item' },
  legend: { bottom: 0, textStyle: { color: axisColor.value } },
  series: [{
    type: 'pie', radius: ['40%', '66%'], center: ['50%', '48%'], avoidLabelOverlap: true,
    label: { color: axisColor.value },
    data: columns.map((c) => ({ name: c.label, value: grouped[c.key].length, itemStyle: { color: c.color } })),
  }],
}))
const categoryChart = computed(() => {
  const cats = ['work', 'life', 'study']
  return {
    backgroundColor: 'transparent',
    title: { text: '分类完成情况', left: 'center', textStyle: { color: axisColor.value, fontSize: 14 } },
    tooltip: { trigger: 'axis' },
    legend: { bottom: 0, data: ['已完成', '未完成'], textStyle: { color: axisColor.value } },
    grid: { left: 40, right: 16, top: 40, bottom: 40 },
    xAxis: { type: 'category', data: cats.map(catLabel), axisLabel: { color: axisColor.value } },
    yAxis: { type: 'value', axisLabel: { color: axisColor.value } },
    series: [
      { name: '已完成', type: 'bar', stack: 't', color: '#18a058', data: cats.map((c) => items.value.filter((i) => i.category === c && i.done).length) },
      { name: '未完成', type: 'bar', stack: 't', color: '#f0a020', data: cats.map((c) => items.value.filter((i) => i.category === c && !i.done).length) },
    ],
  }
})
const trendChart = computed(() => {
  // 近 14 天按 closed/deadline 完成数
  const days = []
  const base = new Date()
  for (let i = 13; i >= 0; i--) {
    const d = new Date(base); d.setDate(base.getDate() - i)
    days.push(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`)
  }
  const doneByDay = days.map((day) => items.value.filter((i) => i.closed === day).length)
  const dueByDay = days.map((day) => items.value.filter((i) => i.deadline === day).length)
  return {
    backgroundColor: 'transparent',
    title: { text: '近 14 天趋势', left: 'center', textStyle: { color: axisColor.value, fontSize: 14 } },
    tooltip: { trigger: 'axis' },
    legend: { bottom: 0, data: ['完成', '截止'], textStyle: { color: axisColor.value } },
    grid: { left: 40, right: 16, top: 40, bottom: 40 },
    xAxis: { type: 'category', data: days.map((d) => d.slice(5)), axisLabel: { color: axisColor.value } },
    yAxis: { type: 'value', axisLabel: { color: axisColor.value } },
    series: [
      { name: '完成', type: 'line', smooth: true, color: '#18a058', data: doneByDay },
      { name: '截止', type: 'line', smooth: true, color: '#d03050', data: dueByDay },
    ],
  }
})

// ── 日历 ──
const calRef = ref(null)
const calendarEvents = computed(() => {
  const evs = []
  for (const it of items.value) {
    if (it.scheduled) evs.push({ id: it.id + ':s', title: it.title, start: it.scheduled, color: '#2080f0', extendedProps: { item: it } })
    if (it.deadline) evs.push({ id: it.id + ':d', title: '⏰ ' + it.title, start: it.deadline, color: it.done ? '#18a058' : '#d03050', extendedProps: { item: it } })
  }
  return evs
})
const calendarOptions = computed(() => ({
  plugins: [dayGridPlugin, timeGridPlugin, listPlugin, interactionPlugin],
  initialView: 'dayGridMonth',
  headerToolbar: { left: 'prev,next today', center: 'title', right: 'dayGridMonth,timeGridWeek,timeGridDay,listYear' },
  buttonText: { today: '今天', month: '月', week: '周', day: '日', list: '年' },
  locale: 'zh-cn',
  firstDay: 1,
  height: '100%',
  events: calendarEvents.value,
  eventClick: (info) => {
    const it = info.event.extendedProps?.item
    if (it) openEditor(it)
  },
}))

// 看板/日历/统计都放在同一容器内用 v-show 切换：FullCalendar 与 ECharts 在隐藏(display:none)时
// 无法测量尺寸，会塌缩成一条缝。切到对应视图后强制重新测量。
watch(view, async (v) => {
  await nextTick()
  if (v === 'calendar') {
    try { calRef.value?.getApi()?.updateSize() } catch (e) { logger.warn('日历重测量失败', e) }
  }
  // 触发 ECharts(autoresize) 与 FullCalendar(windowResize) 重新计算尺寸
  try { window.dispatchEvent(new Event('resize')) } catch {}
})

let unlisten = null
onMounted(async () => {
  applyTheme()
  await reload()
  try {
    await invoke('task_start_watching')
  } catch (e) {
    logger.warn('启动任务文件监听失败', e)
  }
  try {
    unlisten = await listen('guaxi:task-files-changed', () => reload())
  } catch (e) {
    logger.warn('监听任务文件变更失败', e)
  }
})

onUnmounted(() => {
  try { unlisten?.() } catch {}
  if (searchTimer) clearTimeout(searchTimer)
})
</script>

<style scoped>
.task-manager-page {
  /* 独立窗口根：用 100vh 而非 100% —— 与本项目其它独立窗口一致。
     #app 与 naive 的 config/message provider 之间的包裹层会破坏 height:100% 的百分比链，
     导致整页按内容撑高、看板列无法约束高度而不滚动。 */
  display: flex; flex-direction: column; height: 100vh; max-height: 100vh;
  background: var(--guaxi-app-bg); color: var(--guaxi-text-primary);
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  font-size: 14px;
}
/* 让 naive-ui / FullCalendar 等子组件统一继承应用字体 */
.task-manager-page :deep(.fc),
.task-manager-page :deep(.n-button),
.task-manager-page :deep(.n-tabs),
.task-manager-page :deep(.n-input),
.task-manager-page :deep(.n-drawer),
.task-manager-page :deep(.n-card) {
  font-family: inherit;
}
.tm-toolbar {
  display: flex; align-items: center; justify-content: space-between; gap: 12px;
  padding: 8px 14px; border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg); flex: 0 0 auto;
}
.tm-toolbar-left { display: flex; align-items: center; gap: 12px; min-width: 0; }
.tm-logo { color: var(--n-color-primary, #2080f0); }
.tm-title { font-weight: 600; font-size: 15px; white-space: nowrap; }
.tm-cat-tabs { width: 280px; }
.tm-toolbar-right { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
.tm-search { width: 180px; }
.tm-body {
  flex: 1 1 auto; min-height: 0; overflow: hidden; position: relative;
  display: flex; flex-direction: column;
}
.tm-loading-overlay {
  position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;
  background: color-mix(in srgb, var(--guaxi-app-bg) 60%, transparent); z-index: 20;
}

/* 看板 */
.tm-kanban {
  display: grid; grid-template-columns: repeat(4, 1fr);
  /* 单行高度绑定到栅格高度（minmax(0,1fr)），列才能被约束高度并触发列内滚动，
     否则默认 auto 行会随卡片增多而撑高，导致不滚动只增高。 */
  grid-template-rows: minmax(0, 1fr);
  gap: 12px;
  flex: 1 1 auto; min-height: 0;
  padding: 14px; box-sizing: border-box; overflow: hidden;
}
.tm-column {
  display: flex; flex-direction: column; min-height: 0;
  background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border);
  border-radius: 10px; overflow: hidden;
}
.tm-column-head {
  display: flex; align-items: center; gap: 8px; padding: 10px 12px;
  border-bottom: 2px solid; font-weight: 600; flex: 0 0 auto; font-size: 15px;
}
.tm-col-dot { width: 9px; height: 9px; border-radius: 50%; }
.tm-col-name { flex: 1; }
.tm-col-count { background: var(--guaxi-header-bg); }
.tm-cards { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 8px; }
.tm-empty-col { padding: 16px; text-align: center; color: var(--guaxi-text-secondary); font-size: 13px; }
.tm-card {
  background: var(--guaxi-app-bg); border: 1px solid var(--guaxi-border); border-radius: 8px;
  padding: 10px; cursor: pointer; transition: box-shadow .15s, transform .05s;
}
.tm-card:hover { box-shadow: 0 3px 10px rgba(0,0,0,.12); }
.tm-card:active { transform: scale(.99); }
.tm-card.overdue { border-color: #d03050; }
.tm-card-top { display: flex; align-items: flex-start; gap: 6px; }
.tm-card-title { font-size: 15px; line-height: 1.5; word-break: break-word; }
.tm-pri { font-size: 11px; font-weight: 700; padding: 1px 6px; border-radius: 4px; color: #fff; flex: 0 0 auto; }
.pri-A { background: #d03050; } .pri-B { background: #f0a020; } .pri-C { background: #909399; }
.tm-card-meta { display: flex; align-items: center; gap: 8px; margin-top: 9px; flex-wrap: wrap; font-size: 13px; color: var(--guaxi-text-secondary); }
.tm-date { display: inline-flex; align-items: center; gap: 2px; }
.tm-date.deadline { color: #d03050; }
.tm-clock { padding: 1px 6px; border-radius: 4px; background: var(--guaxi-header-bg); }
.tm-tags { margin-top: 7px; display: flex; flex-wrap: wrap; gap: 5px; }
.tm-tag { font-size: 12px; color: var(--n-color-primary, #2080f0); }

/* 日历 */
.tm-calendar { flex: 1 1 auto; min-height: 0; padding: 12px; box-sizing: border-box; }
.tm-calendar :deep(.fc) { height: 100%; }
.tm-calendar :deep(.fc .fc-toolbar-title) { font-size: 16px; }

/* 统计 */
.tm-stats { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 16px; box-sizing: border-box; }
.tm-stat-cards { display: grid; grid-template-columns: repeat(5, 1fr); gap: 12px; margin-bottom: 16px; }
.tm-stat-card {
  background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: 10px;
  padding: 16px; text-align: center;
}
.tm-stat-num { font-size: 28px; font-weight: 700; }
.tm-stat-label { font-size: 13px; color: var(--guaxi-text-secondary); margin-top: 4px; }
.tm-charts { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.tm-chart-box { background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: 10px; height: 300px; padding: 8px; }
.tm-chart-wide { grid-column: 1 / -1; }
.tm-export-preview { margin-top: 12px; max-height: 280px; overflow: auto; }
.tm-export-preview pre { white-space: pre-wrap; word-break: break-word; font-size: 12px; margin: 0; }
.tm-edit-info { margin-top: 4px; }
</style>
