<template>
  <div class="todo-root">
    <div class="todo-shell">
      <div class="todo-nav">
        <div class="todo-nav-top">
          <div class="todo-brand">
            <div class="todo-brand-title">TO DO</div>
            <div class="todo-brand-sub">本地生产版（SQLite）</div>
            <div class="todo-brand-sub2">DB: {{ dbInfo?.db_path || '-' }}</div>
          </div>

          <n-input v-model:value="searchQuery" size="small" clearable placeholder="搜索任务 / 备注 / 步骤"
            @update:value="onSearchChange" />
        </div>

        <div class="todo-nav-section">
          <div class="nav-title">智能列表</div>

          <div class="nav-item" :class="{ active: activeView === 'myday' }" @click="setActiveView('myday')">
            <div class="nav-item-left">
              <span class="nav-emoji">☀️</span>
              <span class="nav-label">我的一天</span>
            </div>
            <div class="nav-count" v-if="counts.myday">{{ counts.myday }}</div>
          </div>

          <div class="nav-item" :class="{ active: activeView === 'important' }" @click="setActiveView('important')">
            <div class="nav-item-left">
              <span class="nav-emoji">⭐</span>
              <span class="nav-label">重要</span>
            </div>
            <div class="nav-count" v-if="counts.important">{{ counts.important }}</div>
          </div>

          <div class="nav-item" :class="{ active: activeView === 'planned' }" @click="setActiveView('planned')">
            <div class="nav-item-left">
              <span class="nav-emoji">📅</span>
              <span class="nav-label">已计划</span>
            </div>
            <div class="nav-count" v-if="counts.planned">{{ counts.planned }}</div>
          </div>

          <div class="nav-sep"></div>

          <div class="nav-title">列表</div>

          <div class="nav-item" :class="{ active: activeView === 'inbox' }" @click="setActiveView('inbox')">
            <div class="nav-item-left">
              <span class="nav-emoji">🧾</span>
              <span class="nav-label">{{ inboxTitle }}</span>
            </div>
            <div class="nav-count" v-if="counts.inbox">{{ counts.inbox }}</div>
          </div>

          <div v-for="l in customLists" :key="l.id" class="nav-item" :class="{ active: activeView === l.id }">
            <div class="nav-item-left" @click="setActiveView(l.id)">
              <span class="nav-emoji">📌</span>
              <span class="nav-label">{{ l.title }}</span>
            </div>
            <div class="nav-item-right">
              <div class="nav-count" v-if="counts.byList[l.id]">{{ counts.byList[l.id] }}</div>
              <n-dropdown :options="listInlineOptions" trigger="click" @select="(k) => onListInlineSelect(k, l.id)">
                <button class="more-btn" @click.stop>···</button>
              </n-dropdown>
            </div>
          </div>
        </div>

        <div class="todo-nav-footer">
          <n-button size="small" secondary @click="openCreateList">新建列表</n-button>
          <n-button size="small" quaternary @click="reloadAll">刷新</n-button>
        </div>
      </div>

      <div class="todo-list">
        <div class="list-header">
          <div class="list-header-left">
            <div class="list-title">{{ activeTitle }}</div>
            <div class="list-subtitle">{{ activeSubtitle }}</div>
          </div>

          <div class="list-header-right">
            <n-dropdown :options="sortOptions" trigger="click" @select="setSortMode">
              <n-button size="small" quaternary>排序：{{ sortModeLabel }}</n-button>
            </n-dropdown>

            <n-dropdown :options="listMenuOptions" trigger="click" @select="onListMenuSelect">
              <n-button size="small" quaternary>···</n-button>
            </n-dropdown>
          </div>
        </div>

        <div class="task-pane">
          <div v-if="searchHitsMode" class="search-hits">
            <div class="task-group-title">搜索结果 <span class="task-group-count">{{ searchHits.length }}</span></div>

            <div v-if="searchHits.length === 0" class="task-empty">
              <div class="task-empty-title">没有结果</div>
              <div class="task-empty-sub">换个关键词试试</div>
            </div>

            <div v-for="hit in searchHits" :key="hit.task.id" class="task-row"
              :class="{ selected: hit.task.id === selectedTaskId }" @click="selectTask(hit.task.id)">
              <div class="task-row-left">
                <n-checkbox size="small" :checked="!!hit.task.completed"
                  @update:checked="(v) => toggleCompleted(hit.task.id, v)" @click.stop />
                <div class="task-main">
                  <div class="task-title">{{ hit.task.title }}</div>
                  <div class="task-meta">
                    匹配：{{ hit.matched_in }} · {{ taskMetaLine(hit.task) }}
                  </div>
                </div>
              </div>

              <div class="task-row-right">
                <button class="task-star" :class="{ on: !!hit.task.important }"
                  @click.stop="toggleImportant(hit.task.id)">
                  {{ hit.task.important ? '★' : '☆' }}
                </button>
                <button class="task-del" @click.stop="removeTask(hit.task.id)">×</button>
              </div>
            </div>
          </div>

          <template v-else>
            <div class="task-group">
              <div class="task-group-title">
                未完成 <span class="task-group-count">{{ incompleteTasks.length }}</span>
              </div>

              <div v-if="incompleteTasks.length === 0" class="task-empty">
                <div class="task-empty-title">这里还没有任务</div>
                <div class="task-empty-sub">在下方输入框添加一个新任务</div>
              </div>

              <div v-for="t in incompleteTasks" :key="t.id" class="task-row"
                :class="{ selected: t.id === selectedTaskId }" @click="selectTask(t.id)">
                <div class="task-row-left">
                  <n-checkbox size="small" :checked="!!t.completed" @update:checked="(v) => toggleCompleted(t.id, v)"
                    @click.stop />
                  <div class="task-main">
                    <div class="task-title">{{ t.title }}</div>
                    <div class="task-meta" v-if="taskMetaLine(t)">{{ taskMetaLine(t) }}</div>
                  </div>
                </div>

                <div class="task-row-right">
                  <button class="task-star" :class="{ on: !!t.important }" @click.stop="toggleImportant(t.id)">
                    {{ t.important ? '★' : '☆' }}
                  </button>
                  <button class="task-del" @click.stop="removeTask(t.id)">×</button>
                </div>
              </div>
            </div>

            <div class="task-group task-group-completed">
              <div class="task-group-title clickable" @click="toggleShowCompleted">
                已完成 <span class="task-group-count">{{ completedTasks.length }}</span>
                <span class="task-group-tip">{{ showCompleted ? '收起' : '展开' }}</span>
              </div>

              <div v-if="showCompleted">
                <div v-for="t in completedTasks" :key="t.id" class="task-row completed"
                  :class="{ selected: t.id === selectedTaskId }" @click="selectTask(t.id)">
                  <div class="task-row-left">
                    <n-checkbox size="small" :checked="!!t.completed" @update:checked="(v) => toggleCompleted(t.id, v)"
                      @click.stop />
                    <div class="task-main">
                      <div class="task-title">{{ t.title }}</div>
                      <div class="task-meta" v-if="taskMetaLine(t)">{{ taskMetaLine(t) }}</div>
                    </div>
                  </div>

                  <div class="task-row-right">
                    <button class="task-star" :class="{ on: !!t.important }" @click.stop="toggleImportant(t.id)">
                      {{ t.important ? '★' : '☆' }}
                    </button>
                    <button class="task-del" @click.stop="removeTask(t.id)">×</button>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>

        <div class="add-task">
          <div class="add-task-box">
            <span class="add-plus">＋</span>
            <n-input v-model:value="newTaskTitle" placeholder="添加任务" clearable @keyup.enter="addTask" />
          </div>
          <div class="add-task-actions">
            <n-button type="primary" size="small" :loading="busy" @click="addTask">添加</n-button>
          </div>
        </div>
      </div>

      <div class="todo-detail" :class="{ empty: !selectedTask }">
        <div v-if="!selectedTask" class="detail-empty">
          <div class="detail-empty-title">选择一个任务</div>
          <div class="detail-empty-sub">在这里查看并编辑任务详情</div>
        </div>

        <div v-else class="detail-body">
          <div class="detail-top">
            <n-checkbox size="small" :checked="!!selectedTask.completed"
              @update:checked="(v) => toggleCompleted(selectedTask.id, v)" />
            <n-input v-model:value="selectedTaskTitleDraft" class="detail-title-input" placeholder="任务标题"
              @blur="commitSelectedTitle" @keyup.enter="commitSelectedTitle" />
            <button class="detail-star" :class="{ on: !!selectedTask.important }"
              @click="toggleImportant(selectedTask.id)">
              {{ selectedTask.important ? '★' : '☆' }}
            </button>
          </div>

          <div class="detail-actions">
            <div class="detail-action-row">
              <n-button size="small" secondary :loading="busy" @click="toggleMyDay(selectedTask.id)">
                {{ selectedTask.my_day_key === todayKey ? '从“我的一天”移除' : '添加到“我的一天”' }}
              </n-button>

              <n-date-picker v-model:value="duePickerValue" type="date" size="small" clearable placeholder="设置截止日期"
                @update:value="setDueDateFromPicker" />
            </div>

            <div class="detail-action-row">
              <n-select v-model:value="repeatPreset" size="small" clearable placeholder="重复" :options="repeatOptions"
                @update:value="commitRepeatPreset" />
              <n-input v-model:value="repeatDraft" size="small" placeholder="自定义重复（例如：每天/每周/每月/工作日）" clearable
                @blur="commitRepeatDraft" @keyup.enter="commitRepeatDraft" />
            </div>

            <div class="detail-action-row">
              <n-button size="small" quaternary :loading="busy"
                @click="clearDueAndRepeat(selectedTask.id)">清除日期/重复</n-button>
              <n-button size="small" quaternary :loading="busy" @click="duplicateTask(selectedTask.id)">复制任务</n-button>
            </div>
          </div>

          <div class="detail-section">
            <div class="detail-section-title">步骤</div>

            <div v-if="selectedSteps.length === 0" class="detail-muted">还没有步骤</div>

            <div v-for="s in selectedSteps" :key="s.id" class="step-row">
              <n-checkbox size="small" :checked="!!s.completed"
                @update:checked="(v) => toggleStep(selectedTask.id, s.id, v)" />
              <n-input v-model:value="s.title" size="small" placeholder="步骤" @blur="commitStepTitle(selectedTask.id, s)"
                @keyup.enter="commitStepTitle(selectedTask.id, s)" />
              <button class="step-del" @click="removeStep(s.id)">×</button>
            </div>

            <div class="step-add">
              <span class="step-add-plus">＋</span>
              <n-input v-model:value="newStepTitle" size="small" placeholder="添加步骤" clearable
                @keyup.enter="addStep(selectedTask.id)" />
              <n-button size="small" secondary :loading="busy" @click="addStep(selectedTask.id)">添加</n-button>
            </div>
          </div>

          <div class="detail-section">
            <div class="detail-section-title">备注</div>
            <n-input v-model:value="notesDraft" type="textarea" :autosize="{ minRows: 4, maxRows: 10 }"
              placeholder="添加备注" @blur="commitNotes" />
          </div>

          <div class="detail-footer">
            <div class="detail-footer-meta">
              创建于：{{ formatDateTime(selectedTask.created_at) }}
              <span v-if="selectedTask.updated_at"> · 更新于：{{ formatDateTime(selectedTask.updated_at) }}</span>
            </div>

            <div class="detail-footer-actions">
              <n-button size="small" tertiary type="error" :loading="busy"
                @click="removeTask(selectedTask.id)">删除</n-button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <n-modal v-model:show="createListModal">
      <n-card title="新建列表" style="width: 420px">
        <n-input v-model:value="newListTitle" placeholder="列表名称" @keyup.enter="createList" />
        <div style="display:flex; justify-content:flex-end; gap: 8px; margin-top: 12px;">
          <n-button @click="createListModal = false">取消</n-button>
          <n-button type="primary" :loading="busy" @click="createList">创建</n-button>
        </div>
      </n-card>
    </n-modal>

    <n-modal v-model:show="renameListModal">
      <n-card title="重命名列表" style="width: 420px">
        <n-input v-model:value="renameListTitle" placeholder="列表名称" @keyup.enter="confirmRenameList" />
        <div style="display:flex; justify-content:flex-end; gap: 8px; margin-top: 12px;">
          <n-button @click="renameListModal = false">取消</n-button>
          <n-button type="primary" :loading="busy" @click="confirmRenameList">确定</n-button>
        </div>
      </n-card>
    </n-modal>
  </div>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const { message } = createDiscreteApi(['message'])

const busy = ref(false)
const dbInfo = ref(null)

const lists = ref([])
const tasks = ref([])
const steps = ref([])

const activeView = ref('myday')
const selectedTaskId = ref('')
const showCompleted = ref(true)

const searchQuery = ref('')
const searchHits = ref([])
const searchHitsMode = computed(() => String(searchQuery.value || '').trim().length > 0)

const sortMode = ref('smart')
const newTaskTitle = ref('')

const createListModal = ref(false)
const newListTitle = ref('')

const renameListModal = ref(false)
const renameListTitle = ref('')
const renameListId = ref('')

const selectedTaskTitleDraft = ref('')
const notesDraft = ref('')
const newStepTitle = ref('')
const repeatDraft = ref('')
const repeatPreset = ref(null)
const duePickerValue = ref(null)

const todayKey = computed(() => localDateKey(new Date()))

const repeatOptions = [
  { label: '每天', value: '每天' },
  { label: '每周', value: '每周' },
  { label: '每月', value: '每月' },
  { label: '工作日', value: '工作日' },
]

const listInlineOptions = [
  { label: '重命名', key: 'rename' },
  { label: '删除', key: 'delete' },
]

const sortOptions = [
  { label: '智能', key: 'smart' },
  { label: '按截止日期', key: 'due' },
  { label: '按创建时间', key: 'created' },
  { label: '按标题', key: 'title' },
  { label: '按重要', key: 'important' },
]

const sortModeLabel = computed(() => sortOptions.find((o) => o.key === sortMode.value)?.label || '智能')

const localDateKey = (d) => {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

const reloadAll = async () => {
  busy.value = true
  try {
    const info = await invoke('todo_init')
    dbInfo.value = info
    const snap = await invoke('todo_get_snapshot')
    lists.value = snap.lists || []
    tasks.value = snap.tasks || []
    steps.value = snap.steps || []
    dbInfo.value = snap.db_info || info
    console.info('[TODO] snapshot loaded', { lists: lists.value.length, tasks: tasks.value.length, steps: steps.value.length })
  } catch (e) {
    console.error('[TODO] reload failed', e)
    message.error(`加载 TODO 数据失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const inboxTitle = computed(() => (lists.value.find((l) => l.id === 'inbox')?.title || '任务'))

const customLists = computed(() =>
  lists.value
    .filter((l) => l.kind === 'custom')
    .slice()
    .sort((a, b) => (a.order_index ?? 0) - (b.order_index ?? 0)),
)

const selectedTask = computed(() => tasks.value.find((t) => t.id === selectedTaskId.value) || null)

const selectedSteps = computed(() => {
  if (!selectedTask.value) return []
  return steps.value
    .filter((s) => s.task_id === selectedTask.value.id)
    .slice()
    .sort((a, b) => (a.order_index ?? 0) - (b.order_index ?? 0))
})

watch(selectedTask, (t) => {
  if (!t) {
    selectedTaskTitleDraft.value = ''
    notesDraft.value = ''
    newStepTitle.value = ''
    repeatDraft.value = ''
    repeatPreset.value = null
    duePickerValue.value = null
    return
  }
  selectedTaskTitleDraft.value = t.title || ''
  notesDraft.value = t.notes || ''
  repeatDraft.value = t.repeat_rule || ''
  repeatPreset.value = repeatOptions.some((x) => x.value === t.repeat_rule) ? t.repeat_rule : null
  duePickerValue.value = t.due_date ? Date.parse(`${t.due_date}T00:00:00Z`) : null
})

const setActiveView = (v) => {
  activeView.value = v
  selectedTaskId.value = ''
  if (searchHitsMode.value) {
    onSearchChange()
  }
}

const activeTitle = computed(() => {
  if (activeView.value === 'myday') return '我的一天'
  if (activeView.value === 'important') return '重要'
  if (activeView.value === 'planned') return '已计划'
  const l = lists.value.find((x) => x.id === activeView.value)
  return l?.title || '任务'
})

const activeSubtitle = computed(() => {
  if (activeView.value === 'myday') return `今天 · ${todayKey.value}`
  if (activeView.value === 'important') return '带星标的任务'
  if (activeView.value === 'planned') return '设置了截止日期的任务'
  return ''
})

const viewTasks = computed(() => {
  const all = tasks.value

  if (activeView.value === 'myday') return all.filter((t) => t.my_day_key === todayKey.value)
  if (activeView.value === 'important') return all.filter((t) => !!t.important)
  if (activeView.value === 'planned') return all.filter((t) => !!t.due_date)

  return all.filter((t) => t.list_id === activeView.value)
})

const compareTask = (a, b) => {
  const ca = a?.created_at ? Date.parse(a.created_at) : 0
  const cb = b?.created_at ? Date.parse(b.created_at) : 0
  const da = a?.due_date ? Date.parse(`${a.due_date}T00:00:00Z`) : Number.POSITIVE_INFINITY
  const db = b?.due_date ? Date.parse(`${b.due_date}T00:00:00Z`) : Number.POSITIVE_INFINITY

  if (sortMode.value === 'due') {
    if (da !== db) return da - db
    return cb - ca
  }
  if (sortMode.value === 'title') return String(a.title || '').localeCompare(String(b.title || ''), 'zh-CN')
  if (sortMode.value === 'created') return cb - ca
  if (sortMode.value === 'important') {
    if (!!a.important !== !!b.important) return a.important ? -1 : 1
    if (!!a.due_date !== !!b.due_date) return a.due_date ? -1 : 1
    if (da !== db) return da - db
    return cb - ca
  }

  if (!!a.important !== !!b.important) return a.important ? -1 : 1
  if (!!a.due_date !== !!b.due_date) return a.due_date ? -1 : 1
  if (da !== db) return da - db
  return cb - ca
}

const sortedTasks = computed(() => viewTasks.value.slice().sort(compareTask))
const incompleteTasks = computed(() => sortedTasks.value.filter((t) => !t.completed))
const completedTasks = computed(() => sortedTasks.value.filter((t) => !!t.completed))

const counts = computed(() => {
  const byList = {}
  for (const l of lists.value) byList[l.id] = 0

  let myday = 0
  let important = 0
  let planned = 0

  for (const t of tasks.value) {
    if (!t.completed) {
      if (t.my_day_key === todayKey.value) myday += 1
      if (t.important) important += 1
      if (t.due_date) planned += 1
      if (t.list_id && Object.prototype.hasOwnProperty.call(byList, t.list_id)) byList[t.list_id] += 1
    }
  }

  return { myday, important, planned, inbox: byList.inbox || 0, byList }
})

const formatDateTime = (iso) => {
  const ms = Date.parse(String(iso || ''))
  if (!Number.isFinite(ms)) return '-'
  return new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(ms))
}

const formatDue = (yyyyMMdd) => {
  if (!yyyyMMdd) return ''
  const ms = Date.parse(`${yyyyMMdd}T00:00:00Z`)
  if (!Number.isFinite(ms)) return ''
  const d = new Date(ms)
  return new Intl.DateTimeFormat('zh-CN', { month: '2-digit', day: '2-digit' }).format(d)
}

const taskMetaLine = (t) => {
  const bits = []
  if (t.my_day_key === todayKey.value) bits.push('我的一天')
  if (t.due_date) bits.push(`截止 ${formatDue(t.due_date)}`)
  if (t.repeat_rule) bits.push(`重复 ${t.repeat_rule}`)
  const ss = steps.value.filter((x) => x.task_id === t.id)
  if (ss.length) {
    const done = ss.filter((x) => !!x.completed).length
    bits.push(`步骤 ${done}/${ss.length}`)
  }
  return bits.join(' · ')
}

const selectTask = (id) => {
  selectedTaskId.value = id
}

const setSortMode = (k) => {
  sortMode.value = k
}

const onSearchChange = async () => {
  const q = String(searchQuery.value || '').trim()
  if (!q) {
    searchHits.value = []
    return
  }
  try {
    const res = await invoke('todo_search_tasks', { query: q, limit: 300 })
    searchHits.value = Array.isArray(res) ? res : []
  } catch (e) {
    console.warn('[TODO] search failed', e)
    searchHits.value = []
  }
}

const addTask = async () => {
  const title = String(newTaskTitle.value || '').trim()
  if (!title) return

  busy.value = true
  try {
    const payload = {
      listId: resolveNewTaskListId(),
      title,
      completed: false,
      important: activeView.value === 'important',
      myDayKey: activeView.value === 'myday' ? todayKey.value : '',
      dueDate: '',
      repeatRule: '',
      notes: '',
    }

    const task = await invoke('todo_upsert_task', {
      input: {
        id: null,
        list_id: payload.listId,
        title: payload.title,
        completed: payload.completed,
        important: payload.important,
        my_day_key: payload.myDayKey,
        due_date: payload.dueDate,
        repeat_rule: payload.repeatRule,
        notes: payload.notes,
      },
    })

    tasks.value.unshift(task)
    newTaskTitle.value = ''
    selectedTaskId.value = task.id
    console.info('[TODO] task created', task.id)
  } catch (e) {
    console.error('[TODO] addTask failed', e)
    message.error(`添加任务失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const resolveNewTaskListId = () => {
  if (activeView.value === 'inbox') return 'inbox'
  if (activeView.value === 'myday') return 'inbox'
  if (activeView.value === 'important') return 'inbox'
  if (activeView.value === 'planned') return 'inbox'
  const l = lists.value.find((x) => x.id === activeView.value)
  return l ? l.id : 'inbox'
}

const patchTaskLocal = (t) => {
  const idx = tasks.value.findIndex((x) => x.id === t.id)
  if (idx >= 0) tasks.value[idx] = t
  else tasks.value.unshift(t)
}

const toggleCompleted = async (id, v) => {
  busy.value = true
  try {
    const t = await invoke('todo_set_task_completed', { taskId: id, completed: !!v })
    patchTaskLocal(t)
  } catch (e) {
    console.error('[TODO] toggleCompleted failed', e)
    message.error(`更新完成状态失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const toggleImportant = async (id) => {
  busy.value = true
  try {
    const t = await invoke('todo_toggle_task_important', { taskId: id })
    patchTaskLocal(t)
  } catch (e) {
    console.error('[TODO] toggleImportant failed', e)
    message.error(`更新重要状态失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const removeTask = async (id) => {
  busy.value = true
  try {
    await invoke('todo_delete_task', { taskId: id })
    tasks.value = tasks.value.filter((x) => x.id !== id)
    steps.value = steps.value.filter((s) => s.task_id !== id)
    if (selectedTaskId.value === id) selectedTaskId.value = ''
    console.info('[TODO] task deleted', id)
  } catch (e) {
    console.error('[TODO] removeTask failed', e)
    message.error(`删除任务失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const commitSelectedTitle = async () => {
  if (!selectedTask.value) return
  const title = String(selectedTaskTitleDraft.value || '').trim()
  if (!title) {
    selectedTaskTitleDraft.value = selectedTask.value.title
    return
  }
  if (title === selectedTask.value.title) return

  busy.value = true
  try {
    const t = await invoke('todo_upsert_task', {
      input: {
        id: selectedTask.value.id,
        list_id: selectedTask.value.list_id,
        title,
      },
    })
    patchTaskLocal(t)
  } catch (e) {
    console.error('[TODO] commit title failed', e)
    message.error(`保存标题失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const commitNotes = async () => {
  if (!selectedTask.value) return
  const notes = String(notesDraft.value || '')
  if (notes === selectedTask.value.notes) return

  busy.value = true
  try {
    const t = await invoke('todo_set_task_notes', { taskId: selectedTask.value.id, notes })
    patchTaskLocal(t)
  } catch (e) {
    console.error('[TODO] commit notes failed', e)
    message.error(`保存备注失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const toggleMyDay = async (id) => {
  const t = tasks.value.find((x) => x.id === id)
  if (!t) return
  const next = t.my_day_key === todayKey.value ? '' : todayKey.value

  busy.value = true
  try {
    const nt = await invoke('todo_set_task_myday', { taskId: id, myDayKey: next })
    patchTaskLocal(nt)
  } catch (e) {
    console.error('[TODO] toggleMyDay failed', e)
    message.error(`更新“我的一天”失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const setDueDateFromPicker = async (ms) => {
  if (!selectedTask.value) return
  const due = ms ? localDateKey(new Date(ms)) : ''

  busy.value = true
  try {
    const nt = await invoke('todo_set_task_due_date', { taskId: selectedTask.value.id, dueDate: due })
    patchTaskLocal(nt)
  } catch (e) {
    console.error('[TODO] setDueDate failed', e)
    message.error(`设置截止日期失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const commitRepeatPreset = async (v) => {
  if (!selectedTask.value) return
  if (!v) return
  repeatDraft.value = ''
  busy.value = true
  try {
    const nt = await invoke('todo_set_task_repeat_rule', { taskId: selectedTask.value.id, repeatRule: String(v) })
    patchTaskLocal(nt)
  } catch (e) {
    console.error('[TODO] commitRepeatPreset failed', e)
    message.error(`设置重复失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const commitRepeatDraft = async () => {
  if (!selectedTask.value) return
  const rep = String(repeatDraft.value || '').trim()
  if (!rep) return
  repeatPreset.value = null

  busy.value = true
  try {
    const nt = await invoke('todo_set_task_repeat_rule', { taskId: selectedTask.value.id, repeatRule: rep })
    patchTaskLocal(nt)
  } catch (e) {
    console.error('[TODO] commitRepeatDraft failed', e)
    message.error(`设置重复失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const clearDueAndRepeat = async (id) => {
  busy.value = true
  try {
    const t1 = await invoke('todo_set_task_due_date', { taskId: id, dueDate: '' })
    patchTaskLocal(t1)
    const t2 = await invoke('todo_set_task_repeat_rule', { taskId: id, repeatRule: '' })
    patchTaskLocal(t2)
    duePickerValue.value = null
    repeatDraft.value = ''
    repeatPreset.value = null
  } catch (e) {
    console.error('[TODO] clearDueAndRepeat failed', e)
    message.error(`清除失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const duplicateTask = async (id) => {
  const t = tasks.value.find((x) => x.id === id)
  if (!t) return

  busy.value = true
  try {
    const nt = await invoke('todo_upsert_task', {
      input: {
        id: null,
        list_id: t.list_id,
        title: `${t.title}（副本）`,
        completed: false,
        important: t.important,
        my_day_key: '',
        due_date: t.due_date || '',
        repeat_rule: t.repeat_rule || '',
        notes: t.notes || '',
      },
    })
    tasks.value.unshift(nt)
    selectedTaskId.value = nt.id
  } catch (e) {
    console.error('[TODO] duplicateTask failed', e)
    message.error(`复制失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const toggleShowCompleted = () => {
  showCompleted.value = !showCompleted.value
}

const addStep = async (taskId) => {
  const title = String(newStepTitle.value || '').trim()
  if (!title) return

  busy.value = true
  try {
    const s = await invoke('todo_upsert_step', { input: { id: null, task_id: taskId, title, completed: false } })
    steps.value.push(s)
    newStepTitle.value = ''
  } catch (e) {
    console.error('[TODO] addStep failed', e)
    message.error(`添加步骤失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const toggleStep = async (taskId, stepId, v) => {
  const s0 = steps.value.find((x) => x.id === stepId)
  if (!s0) return

  busy.value = true
  try {
    const s = await invoke('todo_upsert_step', { input: { id: stepId, task_id: taskId, title: s0.title, completed: !!v } })
    const idx = steps.value.findIndex((x) => x.id === s.id)
    if (idx >= 0) steps.value[idx] = s
  } catch (e) {
    console.error('[TODO] toggleStep failed', e)
    message.error(`更新步骤失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const commitStepTitle = async (taskId, s) => {
  const title = String(s.title || '').trim()
  if (!title) return

  busy.value = true
  try {
    const ns = await invoke('todo_upsert_step', { input: { id: s.id, task_id: taskId, title, completed: !!s.completed } })
    const idx = steps.value.findIndex((x) => x.id === ns.id)
    if (idx >= 0) steps.value[idx] = ns
  } catch (e) {
    console.error('[TODO] commitStepTitle failed', e)
    message.error(`保存步骤失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const removeStep = async (stepId) => {
  busy.value = true
  try {
    await invoke('todo_delete_step', { stepId })
    steps.value = steps.value.filter((x) => x.id !== stepId)
  } catch (e) {
    console.error('[TODO] removeStep failed', e)
    message.error(`删除步骤失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const openCreateList = () => {
  newListTitle.value = ''
  createListModal.value = true
}

const createList = async () => {
  const title = String(newListTitle.value || '').trim()
  if (!title) return

  busy.value = true
  try {
    const l = await invoke('todo_upsert_list', { input: { id: null, title, kind: 'custom' } })
    lists.value.push(l)
    createListModal.value = false
    setActiveView(l.id)
  } catch (e) {
    console.error('[TODO] createList failed', e)
    message.error(`新建列表失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const onListInlineSelect = (k, listId) => {
  if (k === 'rename') {
    const l = lists.value.find((x) => x.id === listId)
    if (!l) return
    renameListId.value = l.id
    renameListTitle.value = l.title
    renameListModal.value = true
    return
  }
  if (k === 'delete') {
    deleteList(listId)
  }
}

const confirmRenameList = async () => {
  const id = String(renameListId.value || '').trim()
  const title = String(renameListTitle.value || '').trim()
  if (!id || !title) return

  busy.value = true
  try {
    const l = await invoke('todo_upsert_list', { input: { id, title, kind: 'custom' } })
    const idx = lists.value.findIndex((x) => x.id === id)
    if (idx >= 0) lists.value[idx] = l
    renameListModal.value = false
  } catch (e) {
    console.error('[TODO] rename list failed', e)
    message.error(`重命名失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const deleteList = async (listId) => {
  busy.value = true
  try {
    await invoke('todo_delete_list', { listId })
    lists.value = lists.value.filter((x) => x.id !== listId)
    tasks.value = tasks.value.filter((t) => t.list_id !== listId)
    steps.value = steps.value.filter((s) => tasks.value.some((t) => t.id === s.task_id))
    if (activeView.value === listId) setActiveView('inbox')
  } catch (e) {
    console.error('[TODO] delete list failed', e)
    message.error(`删除列表失败：${String(e?.message || e || '')}`)
  } finally {
    busy.value = false
  }
}

const listMenuOptions = computed(() => {
  const base = [
    { label: showCompleted.value ? '隐藏已完成' : '显示已完成', key: 'toggle-completed' },
    { label: '刷新', key: 'reload' },
  ]
  return base
})

const onListMenuSelect = (k) => {
  if (k === 'toggle-completed') toggleShowCompleted()
  if (k === 'reload') reloadAll()
}

onMounted(async () => {
  await reloadAll()
})
</script>

<style scoped>
.todo-root {
  height: 100%;
  width: 100%;
  box-sizing: border-box;
}

.todo-shell {
  height: 100%;
  width: 100%;
  display: grid;
  grid-template-columns: 260px minmax(360px, 1fr) minmax(360px, 520px);
  gap: 12px;
  box-sizing: border-box;
}

.todo-nav,
.todo-list,
.todo-detail {
  height: 100%;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  border-radius: 14px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.todo-nav-top {
  padding: 14px 14px 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  border-bottom: 1px solid var(--guaxi-border);
}

.todo-brand {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.todo-brand-title {
  font-size: 16px;
  font-weight: 700;
  letter-spacing: 0.5px;
}

.todo-brand-sub {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.todo-brand-sub2 {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  opacity: 0.8;
  word-break: break-all;
}

.todo-nav-section {
  padding: 10px;
  overflow: auto;
}

.nav-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding: 8px 10px 6px;
}

.nav-sep {
  height: 1px;
  margin: 8px 10px;
  background: var(--guaxi-border);
}

.nav-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 10px;
  border-radius: 10px;
  cursor: pointer;
  color: var(--guaxi-text1);
  transition: background 0.15s ease, transform 0.1s ease;
}

.nav-item:hover {
  background: var(--guaxi-header-bg);
  transform: translateX(1px);
}

.nav-item.active {
  background: linear-gradient(135deg, rgba(79, 172, 254, 0.24), rgba(0, 242, 254, 0.18));
  border: 1px solid rgba(79, 172, 254, 0.25);
}

.nav-item-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}

.nav-emoji {
  width: 20px;
  text-align: center;
}

.nav-label {
  font-size: 13px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nav-item-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.more-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
  padding: 2px 6px;
  border-radius: 8px;
}

.more-btn:hover {
  background: var(--guaxi-header-bg);
}

.nav-count {
  font-size: 12px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
  background: var(--guaxi-header-bg);
}

.todo-nav-footer {
  padding: 10px;
  display: flex;
  gap: 8px;
  border-top: 1px solid var(--guaxi-border);
}

.list-header {
  padding: 14px 14px 10px;
  border-bottom: 1px solid var(--guaxi-border);
  display: flex;
  justify-content: space-between;
  gap: 10px;
}

.list-title {
  font-size: 18px;
  font-weight: 700;
}

.list-subtitle {
  margin-top: 3px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.list-header-right {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.task-pane {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 10px 10px 0;
}

.task-group {
  margin-bottom: 14px;
}

.task-group-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding: 0 6px 8px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.task-group-title.clickable {
  cursor: pointer;
  user-select: none;
}

.task-group-count {
  font-size: 11px;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}

.task-group-tip {
  margin-left: auto;
  font-size: 11px;
  opacity: 0.8;
}

.task-empty {
  margin: 8px 6px 10px;
  padding: 14px;
  border-radius: 12px;
  background: var(--guaxi-header-bg);
  border: 1px dashed var(--guaxi-border);
}

.task-empty-title {
  font-size: 13px;
  font-weight: 600;
}

.task-empty-sub {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.task-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 10px 10px;
  border-radius: 12px;
  border: 1px solid transparent;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease;
  margin-bottom: 8px;
}

.task-row:hover {
  background: var(--guaxi-header-bg);
}

.task-row.selected {
  background: rgba(79, 172, 254, 0.12);
  border-color: rgba(79, 172, 254, 0.22);
}

.task-row.completed .task-title {
  text-decoration: line-through;
  color: var(--guaxi-text-secondary);
}

.task-row-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}

.task-main {
  min-width: 0;
  flex: 1;
}

.task-title {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.task-meta {
  margin-top: 2px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.task-row-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.task-star,
.task-del,
.detail-star,
.step-del {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  padding: 2px 6px;
  border-radius: 8px;
  color: var(--guaxi-text-secondary);
}

.task-star.on,
.detail-star.on {
  color: #f6c343;
}

.task-del:hover,
.step-del:hover {
  background: rgba(224, 108, 117, 0.12);
  color: #e06c75;
}

.task-star:hover,
.detail-star:hover {
  background: rgba(246, 195, 67, 0.12);
}

.add-task {
  padding: 10px;
  border-top: 1px solid var(--guaxi-border);
  display: flex;
  align-items: center;
  gap: 10px;
}

.add-task-box {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  padding: 8px 10px;
}

.add-plus {
  color: var(--guaxi-text-secondary);
  font-size: 16px;
}

.todo-detail.empty {
  display: flex;
  align-items: center;
  justify-content: center;
}

.detail-empty {
  text-align: center;
  padding: 18px;
}

.detail-empty-title {
  font-size: 14px;
  font-weight: 700;
}

.detail-empty-sub {
  margin-top: 6px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.detail-body {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 14px;
  box-sizing: border-box;
  overflow: auto;
}

.detail-top {
  display: flex;
  align-items: center;
  gap: 10px;
}

.detail-title-input {
  flex: 1;
}

.detail-actions {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--guaxi-border);
}

.detail-action-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.detail-section {
  margin-top: 12px;
}

.detail-section-title {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-bottom: 8px;
}

.detail-muted {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  padding: 6px 2px;
}

.step-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.step-add {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
  padding: 8px 10px;
  border-radius: 12px;
  background: var(--guaxi-header-bg);
  border: 1px dashed var(--guaxi-border);
}

.step-add-plus {
  color: var(--guaxi-text-secondary);
  font-size: 16px;
}

.detail-footer {
  margin-top: auto;
  padding-top: 12px;
  border-top: 1px solid var(--guaxi-border);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.detail-footer-meta {
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.detail-footer-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

@media (max-width: 1100px) {
  .todo-shell {
    grid-template-columns: 240px 1fr;
  }

  .todo-detail {
    display: none;
  }
}
</style>