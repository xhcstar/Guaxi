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
          <n-button :type="view === 'kanban' ? 'primary' : 'default'" title="看板" @click="view = 'kanban'">
            <template #icon><n-icon :component="Board20Regular" /></template>
          </n-button>
          <n-button :type="view === 'calendar' ? 'primary' : 'default'" title="日历" @click="view = 'calendar'">
            <template #icon><n-icon :component="CalendarLtr20Regular" /></template>
          </n-button>
          <n-button :type="view === 'stats' ? 'primary' : 'default'" title="统计" @click="view = 'stats'">
            <template #icon><n-icon :component="DataPie20Regular" /></template>
          </n-button>
        </n-button-group>
        <n-button size="small" circle title="导入任务" @click="showImport = true">
          <template #icon><n-icon :component="ArrowImport20Regular" /></template>
        </n-button>
        <n-button size="small" circle title="导出报表" @click="openExport">
          <template #icon><n-icon :component="ArrowExportLtr20Regular" /></template>
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
                <div class="tm-card" :class="{ overdue: isOverdue(element) }">
                  <div class="tm-card-top">
                    <span v-if="element.priority" class="tm-pri" :class="'pri-' + element.priority">{{ element.priority }}</span>
                    <span class="tm-card-title">{{ element.title || '(无标题)' }}</span>
                  </div>
                  <div class="tm-card-meta">
                    <n-tag size="tiny" :bordered="false" :color="catTagColor(element.category)">{{ catLabel(element.category) }}</n-tag>
                    <span class="tm-type" :class="'tt-' + element.taskType">{{ typeLabel(element.taskType) }}</span>
                    <span v-if="element.deadline" class="tm-date deadline"><n-icon :component="Flag16Regular" />{{ element.deadline }}</span>
                    <span v-if="element.displayId" class="tm-did">{{ element.displayId }}</span>
                  </div>
                </div>
              </template>
            </draggable>
            <div v-if="!grouped[col.key].length" class="tm-empty-col">暂无任务</div>
          </div>
        </section>

        <!-- 日历 -->
        <section v-if="view === 'calendar'" class="tm-calendar">
          <FullCalendar ref="calRef" :options="calendarOptions" />
        </section>

        <!-- 日历悬浮详情卡片 -->
        <Teleport to="body">
          <div
            v-if="hover.visible && hover.item"
            class="tm-cal-tip"
            :class="isDark ? 'dark' : 'light'"
            :style="{ left: hover.x + 'px', top: hover.y + 'px' }"
          >
            <div class="tip-head">
              <span class="tip-type" :class="'tt-' + hover.item.taskType">{{ typeLabel(hover.item.taskType) }}</span>
              <span v-if="hover.item.priority" class="tip-pri" :class="'pri-' + hover.item.priority">#{{ hover.item.priority }}</span>
              <span class="tip-title">{{ hover.item.title }}</span>
            </div>
            <div class="tip-row"><span class="tip-k">状态</span><span class="tip-v">{{ statusZh(hover.item.status) }}</span></div>
            <div v-if="hover.item.displayId" class="tip-row"><span class="tip-k">ID</span><span class="tip-v">{{ hover.item.displayId }}</span></div>
            <div class="tip-row"><span class="tip-k">分类</span><span class="tip-v">{{ catLabel(hover.item.category) }}</span></div>
            <div v-if="hover.item.severity" class="tip-row"><span class="tip-k">严重度</span><span class="tip-v">{{ hover.item.severity }}</span></div>
            <div v-if="hover.item.deadline" class="tip-row"><span class="tip-k">截止</span><span class="tip-v">{{ hover.item.deadline }}</span></div>
            <div v-if="hover.item.createTime" class="tip-row"><span class="tip-k">创建</span><span class="tip-v">{{ hover.item.createTime }}</span></div>
          </div>
        </Teleport>

        <!-- 统计 -->
        <section v-if="view === 'stats'" class="tm-stats">
          <div class="tm-stats-bar">
            <span class="tm-stats-title">统计概览</span>
            <span class="tm-spacer2" />
            <n-button size="small" type="primary" ghost @click="openFieldExport">
              <template #icon><n-icon :component="ArrowExportLtr20Regular" /></template>
              按字段筛选导出
            </n-button>
          </div>
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
            <div v-if="showBugStat" class="tm-stat-card">
              <div class="tm-stat-num" style="color:#d03050">{{ bugCount }}</div>
              <div class="tm-stat-label">Bug 数</div>
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
    <n-drawer v-model:show="showExport" :width="640" placement="right">
      <n-drawer-content title="导出报表" closable>
        <n-form label-placement="left" label-width="auto" size="small" class="tm-export-form">
          <n-form-item label="报表类型">
            <n-select v-model:value="exportForm.kind" :options="reportKindOptions" style="width:160px" @update:value="genPreview" />
          </n-form-item>
          <n-form-item label="开始日期">
            <n-date-picker v-model:formatted-value="exportForm.from" value-format="yyyy-MM-dd" type="date" clearable style="width:200px" placeholder="默认按类型回溯" @update:formatted-value="genPreview" />
          </n-form-item>
          <n-form-item label="结束日期">
            <n-date-picker v-model:formatted-value="exportForm.to" value-format="yyyy-MM-dd" type="date" clearable style="width:200px" placeholder="默认今天" @update:formatted-value="genPreview" />
          </n-form-item>
        </n-form>
        <div class="tm-export-preview-wrap">
          <div class="tm-export-preview-label">报表内容（纯文本，可编辑）</div>
          <textarea v-model="exportPreview" class="tm-export-textarea" placeholder="点击「生成」后在此编辑纯文本报表…" spellcheck="false" />
        </div>
        <template #footer>
          <n-space>
            <n-button @click="genPreview" :loading="exporting">生成</n-button>
            <n-button type="primary" :disabled="!exportPreview" @click="saveReport">保存到文件…</n-button>
            <n-button :disabled="!exportPreview" @click="copyReport">复制</n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>

    <!-- 按字段筛选导出 -->
    <n-drawer v-model:show="showFieldExport" :width="760" placement="right">
      <n-drawer-content title="按字段筛选导出" closable>
        <div class="tm-fx">
          <div class="tm-fx-filters">
            <div class="tm-fx-fields-head"><span>快捷筛选</span></div>
            <div class="tm-fx-row">
              <label>分类
                <n-select v-model:value="fxFilter.category" size="small" :options="fxCategoryOptions" @update:value="recomputeFx" />
              </label>
              <label>类型
                <n-select v-model:value="fxFilter.taskType" size="small" :options="fxTypeOptions" @update:value="recomputeFx" />
              </label>
              <label>状态
                <n-select v-model:value="fxFilter.status" size="small" :options="fxStatusOptions" @update:value="recomputeFx" />
              </label>
              <label>严重度
                <n-select v-model:value="fxFilter.severity" size="small" :options="fxSeverityOptions" @update:value="recomputeFx" />
              </label>
              <label>优先级
                <n-select v-model:value="fxFilter.priority" size="small" :options="fxPriorityOptions" @update:value="recomputeFx" />
              </label>
            </div>
            <div class="tm-fx-row">
              <label>创建起<n-date-picker v-model:formatted-value="fxFilter.createFrom" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label>创建止<n-date-picker v-model:formatted-value="fxFilter.createTo" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label>完成起<n-date-picker v-model:formatted-value="fxFilter.completedFrom" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label>完成止<n-date-picker v-model:formatted-value="fxFilter.completedTo" value-format="yyyy-MM-dd" type="date" size="small" clearable @update:formatted-value="recomputeFx" /></label>
              <label class="grow">描述含<n-input v-model:value="fxFilter.keyword" size="small" clearable placeholder="关键字" @update:value="recomputeFx" /></label>
            </div>

            <!-- 字段条件：覆盖全部默认字段与自定义字段（来源/修复状态/进度/根本原因/影响版本…） -->
            <div class="tm-fx-conds">
              <div class="tm-fx-fields-head">
                <span>字段条件</span>
                <span class="tm-spacer2" />
                <a @click="addCond">+ 添加条件</a>
                <template v-if="fxConds.length"> · <a @click="clearConds">清空</a></template>
              </div>
              <div v-for="(c, i) in fxConds" :key="c.uid" class="tm-fx-cond">
                <n-select v-model:value="c.field" size="small" class="fx-cond-field" :options="fxFieldOptions" placeholder="字段" @update:value="(v) => onCondFieldChange(c, v)" />
                <n-select v-model:value="c.op" size="small" class="fx-cond-op" :options="condOpOptions(c)" @update:value="recomputeFx" />
                <template v-if="!condNoValue(c)">
                  <n-select v-if="condMulti(c)" v-model:value="c.values" size="small" multiple class="fx-cond-val" :options="condEnumOptions(c)" placeholder="多个值" @update:value="recomputeFx" />
                  <n-select v-else-if="condEnumSingle(c)" v-model:value="c.value" size="small" clearable class="fx-cond-val" :options="condEnumOptions(c)" placeholder="值" @update:value="recomputeFx" />
                  <template v-else-if="condIsDate(c)">
                    <n-date-picker v-model:formatted-value="c.value" value-format="yyyy-MM-dd" type="date" size="small" clearable class="fx-cond-date" placeholder="日期" @update:formatted-value="recomputeFx" />
                    <n-date-picker v-if="c.op === 'range'" v-model:formatted-value="c.value2" value-format="yyyy-MM-dd" type="date" size="small" clearable class="fx-cond-date" placeholder="至" @update:formatted-value="recomputeFx" />
                  </template>
                  <n-input v-else v-model:value="c.value" size="small" clearable class="fx-cond-val" placeholder="值" @update:value="recomputeFx" />
                </template>
                <n-button size="tiny" quaternary title="删除条件" @click="removeCond(i)">✕</n-button>
              </div>
              <div v-if="!fxConds.length" class="tm-fx-hint">
                可对任意默认字段（来源 / 修复状态 / 进度 / 根本原因 / 影响版本 / 复现步骤…）与自定义字段添加条件。
              </div>
            </div>
          </div>

          <div class="tm-fx-stat">
            匹配 <b>{{ fxMatched.length }}</b> 条 · Bug {{ fxCount('bug') }} / 任务 {{ fxCount('task') }} / 其他 {{ fxCount('other') }} · 已完成 {{ fxDoneCount }} / 未完成 {{ fxMatched.length - fxDoneCount }}
          </div>

          <div class="tm-fx-fields">
            <div class="tm-fx-fields-head">
              <span>导出字段</span>
              <span class="tm-fx-fields-count">默认 {{ FIELD_DEFS.length }} 项 · 自定义 {{ customFieldKeys.length }} 项 · 已选 {{ fxFields.length }}</span>
              <span class="tm-spacer2" />
              <a @click="fxSelectAll(true)">全选</a> · <a @click="fxSelectAll(false)">清空</a>
            </div>
            <label v-for="f in allExportFields" :key="f.key" class="tm-fx-chk">
              <input type="checkbox" :value="f.key" v-model="fxFields" @change="buildFxText" /> {{ f.label }}
            </label>
          </div>

          <div class="tm-fx-opts">
            <label class="tm-fx-chk"><input type="checkbox" v-model="fxOneLine" @change="buildFxText" /> 单行紧凑(字段用 " | " 分隔)</label>
          </div>

          <div class="tm-fx-preview-wrap">
            <div class="tm-export-preview-label">导出预览（可编辑）</div>
            <textarea v-model="fxText" class="tm-export-textarea" spellcheck="false" />
          </div>
        </div>
        <template #footer>
          <n-space>
            <n-button @click="buildFxText">重新生成</n-button>
            <n-button type="primary" :disabled="!fxText" @click="fxSaveFile">保存到文件…</n-button>
            <n-button :disabled="!fxText" @click="fxCopy">复制</n-button>
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
  Flag16Regular,
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
const typeLabel = (t) => ({ bug: 'Bug', task: '任务', other: '其他' }[t] || '任务')
const STATUS_ZH = { TODO: '待办', NEXT: '下一步', DOING: '进行中', WAITING: '等待', PAUSED: '暂停', HOLD: '搁置', CANCELLED: '已取消', DONE: '已完成' }
const statusZh = (s) => STATUS_ZH[s] || s || '待办'
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

const todayStr = () => {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}
// 任意日期 → ISO "YYYY-MM-DD"：支持 ISO(2026-06-20)、斜杠(2026/6/20)、中文(2026年06月20日)，无法解析返回 ''
const cnDateToIso = (s) => {
  const str = String(s || '')
  const pad = (n) => String(n).padStart(2, '0')
  let m = /(\d{4})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/.exec(str)
  if (m) return `${m[1]}-${pad(m[2])}-${pad(m[3])}`
  return ''
}
const isOverdue = (it) => it.deadlineIso && !it.done && it.deadlineIso < todayStr()

// TaskBlock(原始) → 看板使用的归一化项
const normalizeBlock = (b) => ({
  id: b.internalId || '',          // 主键（更新/拖拽用）
  displayId: b.id || '',
  title: b.description || b.id || '(未命名)',
  status: (b.status || 'TODO').toUpperCase(),
  done: !!b.done,
  priority: b.priority || '',
  category: b.manageType || '',
  taskType: b.taskType || 'task',
  severity: b.severity || '',
  deadline: b.deadline || '',
  deadlineIso: cnDateToIso(b.deadline),
  createTime: b.createTime || '',
  createTimeIso: cnDateToIso(b.createTime),
  content: b.content || '',
  line: b.blockStartLine || 0,
})

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
    items.value = (Array.isArray(res) ? res : []).map(normalizeBlock)
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
    const fresh = Array.isArray(updated) ? updated.find((x) => x.internalId === el.id) : null
    el.status = fresh ? (fresh.status || newStatus) : newStatus
    el.done = fresh ? !!fresh.done : (newStatus === 'DONE' || newStatus === 'CANCELLED')
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
const editForm = reactive({ status: '', title: '', deadline: '', notes: '' })
const openEditor = (it) => {
  editing.value = it
  editForm.status = it.status || 'TODO'
  editForm.title = it.title || ''
  editForm.deadline = it.deadline || ''
  editForm.notes = it.content || ''
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

// ── 导出报表（日/周/月/半年/年，纯文本可编辑）──
const showExport = ref(false)
const exporting = ref(false)
const exportPreview = ref('')
const exportForm = reactive({ kind: 'weekly', from: null, to: null })
const reportKindOptions = [
  { label: '日报', value: 'daily' },
  { label: '周报', value: 'weekly' },
  { label: '月报', value: 'monthly' },
  { label: '半年报', value: 'halfyear' },
  { label: '年报', value: 'yearly' },
]
const reportKindLabel = () => (reportKindOptions.find((o) => o.value === exportForm.kind)?.label || '报表')
// 打开导出抽屉并立即生成一次预览
const openExport = () => {
  showExport.value = true
  genPreview()
}
const genPreview = async () => {
  exporting.value = true
  try {
    // 导出前先重扫描，确保数据库与源文件一致（编辑器保存后立刻反映）
    await invoke('task_scan_and_index').catch(() => {})
    exportPreview.value = await invoke('task_export_report', {
      input: { kind: exportForm.kind, date: '', from: exportForm.from || null, to: exportForm.to || null, format: 'text' },
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
    const path = await saveDialog({ defaultPath: `${reportKindLabel()}-${todayStr()}.txt`, filters: [{ name: 'Text', extensions: ['txt'] }] })
    if (!path) return
    await invoke('task_write_report_file', { path, content: exportPreview.value })
    message.success('报表已保存')
  } catch (e) {
    logger.error('保存报表失败:', e)
    message.error('保存失败：' + (e?.message || e))
  }
}
const copyReport = async () => {
  try { await navigator.clipboard.writeText(exportPreview.value); message.success('已复制周报') }
  catch { message.error('复制失败') }
}

// ── 按字段筛选导出 ──
const showFieldExport = ref(false)
const fxData = ref([])
const fxText = ref('')
const fxOneLine = ref(false)
const fxFilter = reactive({
  category: 'all', taskType: 'all', status: 'all', severity: 'all', priority: 'all',
  createFrom: null, createTo: null, completedFrom: null, completedTo: null, keyword: '',
})
const fxCategoryOptions = [
  { label: '全部分类', value: 'all' }, { label: '工作', value: 'work' }, { label: '生活', value: 'life' }, { label: '学习', value: 'study' },
]
const fxTypeOptions = [
  { label: '全部类型', value: 'all' }, { label: 'Bug', value: 'bug' }, { label: '任务', value: 'task' }, { label: '其他', value: 'other' },
]
const fxStatusOptions = [
  { label: '全部状态', value: 'all' }, { label: '已完成', value: 'done' }, { label: '未完成', value: 'undone' },
  { label: '待办 TODO', value: 'st:TODO' }, { label: '进行中 DOING', value: 'st:DOING' }, { label: '下一步 NEXT', value: 'st:NEXT' },
  { label: '等待 WAITING', value: 'st:WAITING' }, { label: '暂停 PAUSED', value: 'st:PAUSED' }, { label: '搁置 HOLD', value: 'st:HOLD' },
  { label: '已取消 CANCELLED', value: 'st:CANCELLED' }, { label: '已完成 DONE', value: 'st:DONE' },
]
const fxSeverityOptions = [
  { label: '全部', value: 'all' }, { label: 'critical', value: 'critical' }, { label: 'major', value: 'major' },
  { label: 'normal', value: 'normal' }, { label: 'minor', value: 'minor' }, { label: 'trivial', value: 'trivial' },
]
const fxPriorityOptions = [
  { label: '全部', value: 'all' }, { label: 'A', value: 'A' }, { label: 'B', value: 'B' }, { label: 'C', value: 'C' }, { label: '无', value: 'none' },
]
// 字段注册表：任务块 DSL 的全部默认字段（顺序与后端 KNOWN_FIELDS / 模板一致）。
// 同时驱动「导出字段」勾选表与「字段条件」筛选器，新增默认字段只需改这一处。
// type: enum（有固定取值，下拉选择）/ date（中文或 ISO 日期，按时间比较）/ text（其余，按文本匹配）
const enumOptionsOf = (pairs) => pairs.map(([value, label]) => ({ value, label }))
const FIELD_DEFS = [
  { key: 'manageType', label: '分类', type: 'enum', options: enumOptionsOf([['work', '工作'], ['life', '生活'], ['study', '学习']]) },
  { key: 'internalId', label: '内部 ID', type: 'text' },
  { key: 'id', label: 'ID', type: 'text' },
  { key: 'parentId', label: '父任务', type: 'text' },
  { key: 'taskType', label: '类型', type: 'enum', options: enumOptionsOf([['bug', 'Bug'], ['task', '任务'], ['other', '其他']]) },
  { key: 'deadline', label: '截止', type: 'date' },
  { key: 'createTime', label: '创建时间', type: 'date' },
  { key: 'completedTime', label: '完成时间', type: 'date' },
  { key: 'origin', label: '来源', type: 'enum', options: enumOptionsOf([['customer', 'customer'], ['qa', 'qa'], ['dev', 'dev'], ['other', 'other']]) },
  { key: 'severity', label: '严重度', type: 'enum', options: enumOptionsOf(['critical', 'major', 'normal', 'minor', 'trivial'].map((v) => [v, v])) },
  { key: 'priority', label: '优先级', type: 'enum', options: enumOptionsOf([['A', 'A'], ['B', 'B'], ['C', 'C']]) },
  { key: 'status', label: '状态', type: 'enum', options: enumOptionsOf(Object.entries(STATUS_ZH).map(([code, zh]) => [code, `${zh} ${code}`])) },
  { key: 'progress', label: '进度', type: 'text' },
  { key: 'fixStatus', label: '修复状态', type: 'enum', options: enumOptionsOf(['debug', 'test', 'review', 'approval', 'fixed', 'other'].map((v) => [v, v])) },
  { key: 'description', label: '描述', type: 'text' },
  { key: 'rootCause', label: '根本原因', type: 'text' },
  { key: 'conditionOfOccurrence', label: '复现步骤', type: 'text' },
  { key: 'fixedDetails', label: '修复详情', type: 'text' },
  { key: 'testingSuggestions', label: '测试建议', type: 'text' },
  { key: 'affectedRelease', label: '影响版本', type: 'text' },
  { key: 'reviewLink', label: 'Review', type: 'text' },
  { key: 'extraImpacts', label: 'Extra Impact', type: 'text' },
  { key: 'content', label: '详情', type: 'text' },
  { key: 'notes', label: '笔记', type: 'text' },
]
const fxFields = ref(['id', 'taskType', 'status', 'description', 'createTime', 'completedTime'])
// 自定义字段：汇总当前数据里出现过的所有自定义字段名，动态并入字段列表（筛选与导出共用）。
const customFieldKeys = computed(() => {
  const names = new Set()
  for (const b of fxData.value) {
    for (const cf of (b.customFields || [])) {
      if (cf && cf.name) names.add(cf.name)
    }
  }
  return [...names].map((n) => ({ key: 'cf:' + n, label: n, type: 'text' }))
})
const allExportFields = computed(() => FIELD_DEFS.concat(customFieldKeys.value))
const fieldDef = (key) => allExportFields.value.find((f) => f.key === key)
const fxFieldOptions = computed(() => allExportFields.value.map((f) => ({ label: f.label, value: f.key })))

// 字段条件：任意默认/自定义字段 + 比较操作符。日期类字段用 cnDateToIso 归一化后比较，
// 因此 "2026年06月20日" 与 "2026-6-20" 视作同一天。
const OPS_BY_TYPE = {
  enum: [['eq', '等于'], ['ne', '不等于'], ['in', '属于其中之一'], ['empty', '为空'], ['nempty', '不为空']],
  date: [['gte', '不早于'], ['lte', '不晚于'], ['range', '区间'], ['empty', '为空'], ['nempty', '不为空']],
  text: [['contains', '包含'], ['ncontains', '不包含'], ['eq', '等于'], ['ne', '不等于'], ['empty', '为空'], ['nempty', '不为空']],
}
const fxConds = ref([])
let condUid = 0
const condType = (c) => fieldDef(c?.field)?.type || 'text'
const condEnumOptions = (c) => fieldDef(c?.field)?.options || []
const condOpOptions = (c) => OPS_BY_TYPE[condType(c)].map(([value, label]) => ({ value, label }))
const condNoValue = (c) => c?.op === 'empty' || c?.op === 'nempty'
const condMulti = (c) => !!condEnumOptions(c).length && c?.op === 'in'
const condEnumSingle = (c) => !!condEnumOptions(c).length && !condNoValue(c) && !condMulti(c)
const condIsDate = (c) => condType(c) === 'date' && !condNoValue(c)
const addCond = () => {
  fxConds.value.push({ uid: ++condUid, field: 'origin', op: 'eq', value: null, value2: null, values: [] })
  recomputeFx()
}
const removeCond = (i) => { fxConds.value.splice(i, 1); recomputeFx() }
const clearConds = () => { fxConds.value = []; recomputeFx() }
// 换字段后旧操作符可能不适用（如 text→date 的「包含」），重置为该项类型的首个操作符并清值。
// key 由事件显式传入，不依赖 v-model 是否已写回 c.field。
const onCondFieldChange = (c, key) => {
  const type = fieldDef(key || c.field)?.type || 'text'
  c.op = OPS_BY_TYPE[type][0][0]
  c.value = null; c.value2 = null; c.values = []
  recomputeFx()
}
// 筛选用的原始值：枚举字段用英文规范值（与 options 的 value 对齐），日期字段用原文（比较时再归一化）。
const condRawValue = (b, key) => {
  if (key.startsWith('cf:')) {
    const name = key.slice(3)
    const cf = (b.customFields || []).find((x) => x && x.name === name)
    return cf ? String(cf.value ?? '') : ''
  }
  if (key === 'extraImpacts') return (Array.isArray(b.extraImpacts) ? b.extraImpacts : []).join('\n')
  if (key === 'notes') {
    const n = b.notes
    if (!n || n.kind === 'none') return ''
    return String(n.value || '')
  }
  if (key === 'status') return String(b.status || '').toUpperCase()
  return String(b[key] ?? '')
}
const matchCond = (b, c) => {
  if (!c || !c.field || !c.op) return true
  const s = condRawValue(b, c.field)
  const t = s.trim()
  switch (c.op) {
    case 'empty': return !t
    case 'nempty': return !!t
    case 'eq': return t === String(c.value ?? '').trim()
    case 'ne': return t !== String(c.value ?? '').trim()
    case 'in': return (c.values || []).includes(t)
    case 'contains': return t.toLowerCase().includes(String(c.value ?? '').toLowerCase())
    case 'ncontains': return !t.toLowerCase().includes(String(c.value ?? '').toLowerCase())
    case 'gte': { const iso = cnDateToIso(s); return !!iso && !!c.value && iso >= c.value }
    case 'lte': { const iso = cnDateToIso(s); return !!iso && !!c.value && iso <= c.value }
    case 'range': { const iso = cnDateToIso(s); return !!iso && (!c.value || iso >= c.value) && (!c.value2 || iso <= c.value2) }
    default: return true
  }
}

const fxMatched = computed(() => fxData.value.filter((b) => {
  if (fxFilter.category !== 'all' && (b.manageType || '') !== fxFilter.category) return false
  if (fxFilter.taskType !== 'all' && (b.taskType || '') !== fxFilter.taskType) return false
  const st = String(b.status || '').toUpperCase()
  if (fxFilter.status === 'done' && st !== 'DONE') return false
  if (fxFilter.status === 'undone' && st === 'DONE') return false
  if (fxFilter.status.startsWith('st:') && st !== fxFilter.status.slice(3)) return false
  if (fxFilter.severity !== 'all' && !String(b.severity || '').toLowerCase().includes(fxFilter.severity)) return false
  if (fxFilter.priority !== 'all') {
    if (fxFilter.priority === 'none') { if (b.priority) return false }
    else if ((b.priority || '') !== fxFilter.priority) return false
  }
  const cIso = cnDateToIso(b.createTime)
  if (fxFilter.createFrom && (!cIso || cIso < fxFilter.createFrom)) return false
  if (fxFilter.createTo && (!cIso || cIso > fxFilter.createTo)) return false
  const dIso = cnDateToIso(b.completedTime)
  if (fxFilter.completedFrom && (!dIso || dIso < fxFilter.completedFrom)) return false
  if (fxFilter.completedTo && (!dIso || dIso > fxFilter.completedTo)) return false
  if (fxFilter.keyword) {
    const k = fxFilter.keyword.toLowerCase()
    if (!String(b.description || '').toLowerCase().includes(k) && !String(b.id || '').toLowerCase().includes(k)) return false
  }
  for (const c of fxConds.value) if (!matchCond(b, c)) return false
  return true
}))
const fxCount = (t) => fxMatched.value.filter((b) => (b.taskType || '') === t).length
const fxDoneCount = computed(() => fxMatched.value.filter((b) => String(b.status || '').toUpperCase() === 'DONE').length)

const fxFieldValue = (b, key) => {
  if (key === 'taskType') return typeLabel(b.taskType)
  if (key === 'status') return statusZh(String(b.status || '').toUpperCase())
  if (key === 'manageType') return catLabel(b.manageType)
  if (key === 'extraImpacts') return (Array.isArray(b.extraImpacts) ? b.extraImpacts : []).join('\n\n')
  if (key === 'notes') {
    const n = b.notes
    if (!n || n.kind === 'none') return ''
    return n.kind === 'file' ? `（外链）${n.value || ''}` : String(n.value || '')
  }
  return condRawValue(b, key)
}
const buildFxText = () => {
  const fields = allExportFields.value.filter((f) => fxFields.value.includes(f.key))
  const entries = fxMatched.value.map((b) => {
    if (fxOneLine.value) return fields.map((f) => `${f.label}: ${fxFieldValue(b, f.key).replace(/\n/g, ' ')}`).join(' | ')
    return fields.map((f) => {
      const v = fxFieldValue(b, f.key)
      return v.includes('\n') ? `${f.label}:\n${v}` : `${f.label}: ${v}`
    }).join('\n')
  })
  fxText.value = entries.join(fxOneLine.value ? '\n' : '\n----------------------------------------\n')
}
const recomputeFx = () => nextTick(buildFxText)
const fxSelectAll = (all) => { fxFields.value = all ? allExportFields.value.map((f) => f.key) : []; buildFxText() }
const openFieldExport = async () => {
  showFieldExport.value = true
  try {
    await invoke('task_scan_and_index').catch(() => {})
    fxData.value = (await invoke('task_query', { query: {} })) || []
  } catch (e) {
    logger.error('加载任务失败:', e); fxData.value = []
  }
  buildFxText()
}
const fxSaveFile = async () => {
  if (!fxText.value) return
  try {
    const path = await saveDialog({ defaultPath: `任务导出-${todayStr()}.txt`, filters: [{ name: 'Text', extensions: ['txt'] }] })
    if (!path) return
    await invoke('task_write_report_file', { path, content: fxText.value })
    message.success('已导出到文件')
  } catch (e) { logger.error('导出失败:', e); message.error('导出失败：' + (e?.message || e)) }
}
const fxCopy = async () => {
  try { await navigator.clipboard.writeText(fxText.value); message.success('已复制导出内容') }
  catch { message.error('复制失败') }
}

// ── 统计 ──
const doneCount = computed(() => items.value.filter((i) => i.done).length)
const doingCount = computed(() => items.value.filter((i) => ['DOING', 'NEXT'].includes(String(i.status).toUpperCase())).length)
const bugCount = computed(() => items.value.filter((i) => i.taskType === 'bug').length)
// Bug 统计仅在工作/全部下显示（生活·学习无 bug 概念）
const showBugStat = computed(() => category.value !== 'life' && category.value !== 'study')
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
  // 类型/严重度分布
  const types = ['bug', 'task', 'other']
  const typeLabels = { bug: 'Bug', task: '任务', other: '其他' }
  return {
    backgroundColor: 'transparent',
    title: { text: '类型分布', left: 'center', textStyle: { color: axisColor.value, fontSize: 14 } },
    tooltip: { trigger: 'item' },
    legend: { bottom: 0, textStyle: { color: axisColor.value } },
    series: [{
      type: 'pie', radius: ['40%', '66%'], center: ['50%', '48%'],
      label: { color: axisColor.value },
      data: types.map((t, idx) => ({
        name: typeLabels[t],
        value: items.value.filter((i) => i.taskType === t).length,
        itemStyle: { color: ['#d03050', '#2080f0', '#8a8f99'][idx] },
      })),
    }],
  }
})

// ── 日历（按截止日期 ISO） ──
const calRef = ref(null)
const calendarEvents = computed(() => {
  const evs = []
  for (const it of items.value) {
    if (it.deadlineIso) {
      evs.push({ id: it.id + ':d', title: '⏰ ' + it.title, start: it.deadlineIso, color: it.done ? '#18a058' : '#d03050', extendedProps: { item: it } })
    } else if (it.createTimeIso) {
      // 无截止日期时，用创建时间作为日历标记，保证任务仍可见
      evs.push({ id: it.id + ':c', title: it.title, start: it.createTimeIso, color: '#2080f0', extendedProps: { item: it } })
    }
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
  eventMouseEnter: (info) => {
    const it = info.event.extendedProps?.item
    if (!it) return
    const r = info.el.getBoundingClientRect()
    hover.value = { visible: true, x: r.left + r.width / 2, y: r.top, item: it }
  },
  eventMouseLeave: () => { hover.value.visible = false },
}))

// 日历悬浮详情卡片
const hover = ref({ visible: false, x: 0, y: 0, item: null })

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

// 任务数据变化时，把事件推给已挂载的日历（v-if 重挂载会自带初始事件，此处覆盖实时更新）
watch(calendarEvents, async (evs) => {
  await nextTick()
  const api = calRef.value?.getApi?.()
  if (api) {
    try { api.removeAllEvents(); api.addEventSource(evs) } catch (e) { logger.warn('日历事件更新失败', e) }
  }
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
.tm-type { font-size: 11px; padding: 0 6px; border-radius: 8px; color: #fff; }
.tm-type.tt-bug { background: #d03050; } .tm-type.tt-task { background: #2080f0; } .tm-type.tt-other { background: #8a8f99; }
.tm-did { font-size: 11px; color: var(--guaxi-text-secondary); opacity: 0.85; }

/* 日历 */
.tm-calendar { flex: 1 1 auto; min-height: 0; padding: 12px; box-sizing: border-box; }
.tm-calendar :deep(.fc) {
  height: 100%;
  /* 统一用应用主题变量驱动 FullCalendar，修复暗色下亮色边框/列表项 */
  --fc-border-color: var(--guaxi-border);
  --fc-page-bg-color: var(--guaxi-app-bg);
  --fc-neutral-bg-color: var(--guaxi-panel-bg);
  --fc-neutral-text-color: var(--guaxi-text-secondary);
  --fc-today-bg-color: rgba(32, 128, 240, 0.10);
  --fc-list-event-hover-bg-color: var(--guaxi-panel-bg);
  --fc-highlight-color: rgba(32, 128, 240, 0.12);
  color: var(--guaxi-text-primary);
}
.tm-calendar :deep(.fc .fc-toolbar-title) { font-size: 16px; }
/* 上一月/下一月箭头：不依赖 FullCalendar 自带的 fcicons 字体。
   那个字体是以 data: URI 内嵌在运行时注入的 CSS 里的，而 release 版本的 Tauri CSP
   只有 default-src 'self'（没有 font-src），data: 字体会被拦掉，按钮就变成两个方块豆腐。
   CSP 已经补上 font-src data:，这里再用 SVG mask 把箭头画出来做双保险：
   用 currentColor 上色，亮/暗主题都跟着走，也不再受字体加载影响。 */
.tm-calendar :deep(.fc .fc-icon-chevron-left::before),
.tm-calendar :deep(.fc .fc-icon-chevron-right::before),
.tm-calendar :deep(.fc .fc-icon-chevrons-left::before),
.tm-calendar :deep(.fc .fc-icon-chevrons-right::before) {
  content: '';
  display: block;
  width: 1em;
  height: 1em;
  background-color: currentColor;
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: 68% 68%;
  mask-size: 68% 68%;
}
.tm-calendar :deep(.fc .fc-icon-chevron-left::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M15 5 8 12l7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M15 5 8 12l7 7'/></svg>");
}
.tm-calendar :deep(.fc .fc-icon-chevron-right::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M9 5l7 7-7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M9 5l7 7-7 7'/></svg>");
}
.tm-calendar :deep(.fc .fc-icon-chevrons-left::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M18 5l-7 7 7 7M11 5l-7 7 7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M18 5l-7 7 7 7M11 5l-7 7 7 7'/></svg>");
}
.tm-calendar :deep(.fc .fc-icon-chevrons-right::before) {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M6 5l7 7-7 7M13 5l7 7-7 7'/></svg>");
  mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24'><path fill='none' stroke='black' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round' d='M6 5l7 7-7 7M13 5l7 7-7 7'/></svg>");
}
/* 周/日视图：弱化半小时虚线分隔，避免刺眼 */
.tm-calendar :deep(.fc .fc-timegrid-slot-minor) { border-top-style: dotted; border-top-color: color-mix(in srgb, var(--guaxi-border) 45%, transparent); }
.tm-calendar :deep(.fc .fc-timegrid-slot) { border-color: color-mix(in srgb, var(--guaxi-border) 60%, transparent); }
.tm-calendar :deep(.fc .fc-timegrid-now-indicator-line) { border-color: #d03050; }
/* 列表(年)视图：暗色下文字/表头颜色 */
.tm-calendar :deep(.fc .fc-list-day-cushion) { background: var(--guaxi-panel-bg); }
.tm-calendar :deep(.fc .fc-list-day-text),
.tm-calendar :deep(.fc .fc-list-day-side-text),
.tm-calendar :deep(.fc .fc-list-event-title),
.tm-calendar :deep(.fc .fc-list-event-time) { color: var(--guaxi-text-primary); }
.tm-calendar :deep(.fc .fc-list-empty) { background: var(--guaxi-app-bg); color: var(--guaxi-text-secondary); }
.tm-calendar :deep(.fc .fc-col-header-cell-cushion),
.tm-calendar :deep(.fc .fc-daygrid-day-number) { color: var(--guaxi-text-primary); }
.tm-calendar :deep(.fc .fc-day-other .fc-daygrid-day-number) { color: var(--guaxi-text-secondary); opacity: 0.6; }

/* 日历悬浮详情卡片 */
.tm-cal-tip {
  position: fixed; z-index: 9999; transform: translate(-50%, calc(-100% - 10px));
  min-width: 220px; max-width: 320px; padding: 10px 12px; border-radius: 10px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, 'Microsoft YaHei', monospace;
  font-size: 13px; pointer-events: none; box-shadow: 0 8px 28px rgba(0,0,0,0.28);
  border: 1px solid; line-height: 1.6;
}
.tm-cal-tip.light { background: #ffffff; color: #1f2328; border-color: #e3e6eb; }
.tm-cal-tip.dark { background: #282c34; color: #abb2bf; border-color: #3e4451; }
.tm-cal-tip .tip-head { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; flex-wrap: wrap; }
.tm-cal-tip .tip-title { font-weight: 700; font-size: 14px; flex: 1; min-width: 0; }
.tm-cal-tip .tip-type { font-size: 11px; padding: 1px 7px; border-radius: 9px; color: #fff; }
.tm-cal-tip .tt-bug { background: #d03050; } .tm-cal-tip .tt-task { background: #2080f0; } .tm-cal-tip .tt-other { background: #8a8f99; }
.tm-cal-tip .tip-pri { font-size: 11px; font-weight: 700; padding: 1px 6px; border-radius: 4px; color: #fff; }
.tm-cal-tip .pri-A { background: #d03050; } .tm-cal-tip .pri-B { background: #f0a020; } .tm-cal-tip .pri-C { background: #909399; }
.tm-cal-tip .tip-row { display: flex; gap: 8px; }
.tm-cal-tip .tip-k { color: var(--guaxi-text-secondary, #8b94a3); flex: 0 0 44px; }
.tm-cal-tip .tip-v { word-break: break-word; }

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
.tm-export-form { margin-bottom: 10px; }
.tm-export-preview-wrap { display: flex; flex-direction: column; height: calc(100% - 60px); min-height: 360px; }
.tm-export-preview-label { font-size: 12px; color: var(--guaxi-text-secondary); margin-bottom: 6px; }
.tm-export-textarea {
  flex: 1 1 auto; width: 100%; box-sizing: border-box; resize: vertical; min-height: 340px;
  background: var(--guaxi-app-bg); color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 12px 14px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  font-size: 13.5px; line-height: 1.7; white-space: pre; overflow: auto;
}
.tm-export-textarea:focus { outline: none; border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.18); }
.tm-edit-info { margin-top: 4px; }

/* 统计顶栏 + 按字段筛选导出 */
.tm-stats-bar { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; }
.tm-stats-title { font-weight: 700; font-size: 15px; }
.tm-spacer2 { flex: 1; }
.tm-fx { display: flex; flex-direction: column; gap: 12px; height: 100%; }
.tm-fx-filters { display: flex; flex-direction: column; gap: 8px; }
.tm-fx-row { display: flex; flex-wrap: wrap; gap: 10px; }
.tm-fx-row label { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--guaxi-text-secondary); min-width: 120px; }
.tm-fx-row label.grow { flex: 1; min-width: 180px; }
.tm-fx-stat { background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 8px 12px; font-size: 13px; }
.tm-fx-stat b { color: #2080f0; font-size: 15px; }
.tm-fx-fields { border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 10px 12px; }
.tm-fx-fields-head { display: flex; align-items: center; font-size: 12px; color: var(--guaxi-text-secondary); margin-bottom: 8px; }
.tm-fx-fields-head a { color: #2080f0; cursor: pointer; }
.tm-fx-fields-count { margin-left: 8px; opacity: 0.75; }

/* 字段条件（任意默认/自定义字段 + 操作符） */
.tm-fx-conds { border: 1px solid var(--guaxi-border); border-radius: 8px; padding: 10px 12px; }
.tm-fx-cond { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
.tm-fx-cond .fx-cond-field { flex: 0 0 150px; }
.tm-fx-cond .fx-cond-op { flex: 0 0 110px; }
.tm-fx-cond .fx-cond-val { flex: 1 1 auto; min-width: 120px; }
.tm-fx-cond .fx-cond-date { flex: 0 0 150px; }
.tm-fx-hint { font-size: 12px; color: var(--guaxi-text-secondary); opacity: 0.85; line-height: 1.6; }
.tm-fx-chk { display: inline-flex; align-items: center; gap: 4px; font-size: 13px; margin: 3px 12px 3px 0; cursor: pointer; }
.tm-fx-opts { font-size: 13px; }
.tm-fx-preview-wrap { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 220px; }
</style>
