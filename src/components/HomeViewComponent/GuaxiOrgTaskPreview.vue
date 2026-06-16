<template>
  <div class="org-task-preview" :class="theme === 'dark' ? 'theme-dark' : 'theme-light'">
    <div class="otp-toolbar">
      <button v-if="canBack" class="otp-btn icon" title="返回" @click="$emit('back')">‹</button>
      <span class="otp-title">{{ titleLabel || '任务预览' }}</span>
      <span class="otp-count">{{ blocks.length }} 项</span>
      <span class="otp-spacer" />
      <button class="otp-btn icon" title="大纲" :class="{ on: showOutline }" @click="showOutline = !showOutline">☰</button>
      <button class="otp-btn icon" title="新建 Bug" @click="$emit('create-block', 'bug')">＋B</button>
      <button class="otp-btn icon" title="新建任务" @click="$emit('create-block', 'task')">＋T</button>
    </div>

    <!-- 搜索栏 -->
    <div class="otp-searchbar">
      <select v-model="searchField" class="otp-sel" title="搜索字段">
        <option value="all">全部字段</option>
        <option value="description">描述</option>
        <option value="id">ID</option>
        <option value="rootCause">根本原因</option>
        <option value="conditionOfOccurrence">复现步骤</option>
        <option value="fixedDetails">修复详情</option>
        <option value="testingSuggestions">测试建议</option>
        <option value="content">详情</option>
        <option value="notes">笔记</option>
      </select>
      <input v-model="search" class="otp-search" placeholder="搜索…" @keydown.enter.prevent="gotoMatch(1)" />
      <span class="otp-search-count">{{ matches.length ? (matchIdx + 1) + '/' + matches.length : '0/0' }}</span>
      <button class="otp-btn icon sm" title="上一个" :disabled="!matches.length" @click="gotoMatch(-1)">↑</button>
      <button class="otp-btn icon sm" title="下一个" :disabled="!matches.length" @click="gotoMatch(1)">↓</button>
    </div>

    <div class="otp-main">
      <!-- 大纲小地图 -->
      <aside v-if="showOutline" class="otp-outline">
        <div class="otp-outline-head">大纲 · {{ blocks.length }}</div>
        <div
          v-for="(b, i) in blocks"
          :key="'o' + (b.internalId || i)"
          class="otp-outline-item"
          :class="{ active: b.blockStartLine === activeBlockLine, done: b.done }"
          @click="$emit('goto-line', b.blockStartLine)"
        >
          <span class="oo-dot" :class="'tt-' + (b.taskType || 'task')" />
          <span class="oo-title">{{ b.description || b.id || '(未命名)' }}</span>
          <span class="oo-st" :class="'st-' + (b.status || 'TODO')">{{ statusLabel(b.status) }}</span>
        </div>
      </aside>

      <div class="otp-body" ref="bodyRef">
        <div v-if="diagnostics.length" class="otp-diags">
          <div class="otp-diags-head">⚠ 语法检查（{{ errorCount }} 错误 / {{ warnCount }} 警告）</div>
          <div v-for="(d, di) in diagnostics" :key="di" class="otp-diag" :class="d.severity" @click="$emit('goto-line', d.line)">
            <span class="otp-diag-line">行 {{ d.line }}</span><span class="otp-diag-msg">{{ d.message }}</span>
          </div>
        </div>

        <div v-if="!blocks.length" class="otp-empty">
          暂无任务块。点击工具栏 <b>＋B / ＋T</b> 新建，或在编辑器输入 <code>##</code> 选择模板。
        </div>

        <div
          v-for="(b, idx) in blocks"
          :key="b.internalId || b.blockStartLine || idx"
          class="otp-card"
          :class="{ done: b.done, active: b.blockStartLine === activeBlockLine }"
          :data-block-line="b.blockStartLine"
          :ref="(el) => registerCardRef(b.blockStartLine, el)"
        >
          <!-- ============ 读视图 ============ -->
          <template v-if="editingLine !== b.blockStartLine">
            <!-- 上方插入 -->
            <div class="otp-ins top">
              <span class="otp-ins-label">↑ 在上方插入</span>
              <button class="otp-ins-btn bug" title="在上方新建 Bug" @click="emit('insert-block', { taskType: 'bug', where: 'above', block: b })">＋Bug</button>
              <button class="otp-ins-btn task" title="在上方新建任务" @click="emit('insert-block', { taskType: 'task', where: 'above', block: b })">＋任务</button>
            </div>

            <div class="otp-card-head">
              <span class="otp-type" :class="'type-' + (b.taskType || 'task')">{{ typeLabel(b.taskType) }}</span>
              <span v-if="b.priority" class="otp-pri" :class="'pri-' + b.priority">#{{ b.priority }}</span>
              <span class="otp-card-title" @click="$emit('goto-line', b.blockStartLine)" v-html="hl(b.description || b.id || '(未命名)')" />
              <button class="otp-mini xs" :class="{ ok: flashed === 'desc:' + b.blockStartLine }" title="复制描述" @click.stop="copyDesc(b)">{{ flashed === 'desc:' + b.blockStartLine ? '✓' : '⧉' }}</button>
              <span class="otp-status" :class="'st-' + (b.status || 'TODO')">{{ statusLabel(b.status) }}</span>
              <span class="otp-actions">
                <button class="otp-mini" title="编辑" @click="startEdit(b)">✎</button>
                <button class="otp-mini" :class="{ ok: flashed === 'copy:' + b.blockStartLine }" title="复制 Bug 信息" @click="copyCard(b)">{{ flashed === 'copy:' + b.blockStartLine ? '✓' : '⧉' }}</button>
                <button class="otp-mini" :class="{ ok: flashed === 'export:' + b.blockStartLine }" title="导出提交信息" @click="exportCard(b)">{{ flashed === 'export:' + b.blockStartLine ? '✓' : '⇪' }}</button>
                <button class="otp-mini danger" title="删除" @click="$emit('delete-block', b)">🗑</button>
              </span>
            </div>

            <div class="otp-props">
              <div v-if="b.id" class="otp-prop"><span class="k">ID</span><span class="v" v-html="hl(b.id)" /></div>
              <div v-if="b.parentId" class="otp-prop"><span class="k">父任务</span><span class="v">{{ b.parentId }}</span></div>
              <div v-if="b.severity" class="otp-prop"><span class="k">严重度</span><span class="v">{{ b.severity }}</span></div>
              <div v-if="b.origin" class="otp-prop"><span class="k">来源</span><span class="v">{{ b.origin }}</span></div>
              <div v-if="b.fixStatus" class="otp-prop"><span class="k">修复</span><span class="v">{{ b.fixStatus }}</span></div>
              <div v-if="b.progress" class="otp-prop"><span class="k">进度</span><span class="v">{{ b.progress }}</span></div>
              <div v-if="b.deadline" class="otp-prop"><span class="k">截止</span><span class="v">{{ b.deadline }}</span></div>
              <div v-if="b.createTime" class="otp-prop"><span class="k">创建</span><span class="v">{{ b.createTime }}</span></div>
              <div v-if="b.completedTime" class="otp-prop done-t"><span class="k">完成</span><span class="v">{{ b.completedTime }}</span></div>
              <div v-if="b.affectedRelease" class="otp-prop"><span class="k">影响版本</span><span class="v">{{ b.affectedRelease }}</span></div>
            </div>

            <!-- 可折叠分区 -->
            <section v-for="sec in sectionsOf(b)" :key="sec.key" class="otp-sec">
              <div class="otp-sec-head" @click="toggleSec(b.blockStartLine, sec.key, sec.defaultCollapsed)">
                <span class="otp-sec-caret">{{ isCollapsed(b.blockStartLine, sec.key, sec.defaultCollapsed) ? '▸' : '▾' }}</span>
                <h4>{{ sec.title }}</h4>
                <span class="otp-sec-actions">
                  <button class="otp-mini xs" :class="{ ok: flashed === 'sec:' + b.blockStartLine + ':' + sec.key }" title="复制本节" @click.stop="copySection(sec, b.blockStartLine)">{{ flashed === 'sec:' + b.blockStartLine + ':' + sec.key ? '✓' : '⧉' }}</button>
                </span>
              </div>
              <div v-show="!isCollapsed(b.blockStartLine, sec.key, sec.defaultCollapsed)" class="otp-sec-body">
                <div v-if="sec.type === 'text'" class="otp-text" v-html="hl(sec.value)" />
                <pre v-else-if="sec.type === 'pre'" class="otp-pre" v-html="hl(sec.value)" />
                <div v-else-if="sec.type === 'md'" class="gmd" v-html="renderMd(sec.value)" />
                <a v-else-if="sec.type === 'link'" class="otp-link" :href="sec.value" target="_blank" rel="noreferrer">{{ sec.value }}</a>
                <template v-else-if="sec.type === 'notes'">
                  <div v-if="sec.value.kind === 'inline'" class="gmd" v-html="renderMd(sec.value.value)" />
                  <a v-else class="otp-notes-file" href="#" @click.prevent="$emit('open-notes', sec.value.value)">📄 {{ sec.value.value }}（点击打开）</a>
                </template>
                <div v-else-if="sec.type === 'extra'" class="otp-extra-list">
                  <pre v-for="(ei, i) in sec.value" :key="i" class="otp-pre">{{ ei }}</pre>
                </div>
              </div>
            </section>

            <!-- 下方插入 -->
            <div class="otp-ins bottom">
              <span class="otp-ins-label">↓ 在下方插入</span>
              <button class="otp-ins-btn bug" title="在下方新建 Bug" @click="emit('insert-block', { taskType: 'bug', where: 'below', block: b })">＋Bug</button>
              <button class="otp-ins-btn task" title="在下方新建任务" @click="emit('insert-block', { taskType: 'task', where: 'below', block: b })">＋任务</button>
            </div>
          </template>

          <!-- ============ 编辑视图 ============ -->
          <template v-else>
            <div class="otp-edit-head">
              <strong>编辑任务</strong>
              <span class="otp-spacer" />
              <button class="otp-btn primary" @click="saveEdit">保存</button>
              <button class="otp-btn" @click="cancelEdit">取消</button>
            </div>
            <div class="otp-form">
              <label>类型<select v-model="draft.taskType"><option v-for="t in typeOptions" :key="t" :value="t">{{ typeLabel(t) }}</option></select></label>
              <label>状态<select v-model="draft.status"><option v-for="s in statusKeys" :key="s" :value="s">{{ statusLabel(s) }}</option></select></label>
              <label>优先级<select v-model="draft.priority"><option value="">未指定</option><option value="A">A</option><option value="B">B</option><option value="C">C</option></select></label>
              <label>严重度<select v-model="draft.severity"><option v-for="s in severityOptions" :key="s" :value="s">{{ s }}</option></select></label>
              <label>来源<select v-model="draft.origin"><option v-for="o in originOptions" :key="o" :value="o">{{ o }}</option></select></label>
              <label>修复状态<select v-model="draft.fixStatus"><option v-for="o in fixOptions" :key="o" :value="o">{{ o }}</option></select></label>
              <label>进度<input v-model="draft.progress" placeholder="50%" /></label>
              <label>ID<input v-model="draft.id" placeholder="bug/task id" /></label>
              <label>父任务<input v-model="draft.parentId" /></label>
              <label>截止
                <div class="otp-date-field">
                  <input v-model="draft.deadline" placeholder="2026年06月20日" />
                  <span class="otp-date-icon" aria-hidden="true">📅</span>
                  <input
                    type="date"
                    class="otp-date-overlay"
                    title="日历选择截止日期"
                    :value="toDateInput(draft.deadline)"
                    @change="onPickDeadline"
                  />
                </div>
              </label>
              <label>创建时间
                <div class="otp-date-field">
                  <input v-model="draft.createTime" placeholder="2026年06月20日 14:30:00" />
                  <span class="otp-date-icon" aria-hidden="true">📅</span>
                  <input
                    type="datetime-local"
                    step="1"
                    class="otp-date-overlay"
                    title="日历选择创建时间"
                    :value="toDateTimeInput(draft.createTime)"
                    @change="onPickCreateTime"
                  />
                </div>
              </label>
              <label>影响版本<input v-model="draft.affectedRelease" /></label>
              <label class="full">描述<input v-model="draft.description" /></label>
              <label class="full">根本原因<textarea v-model="draft.rootCause" class="ta-md" /></label>
              <label class="full">复现步骤<textarea v-model="draft.conditionOfOccurrence" class="ta-lg" /></label>
              <label class="full">修复详情<textarea v-model="draft.fixedDetails" class="ta-lg" /></label>
              <label class="full">测试建议<textarea v-model="draft.testingSuggestions" class="ta-md" /></label>
              <label class="full">详情 (CONTENT)<textarea v-model="draft.content" class="ta-xl" /></label>
              <label class="full">笔记 (NOTES)
                <textarea v-if="draft.notes.kind !== 'file'" v-model="draft.notes.value" class="ta-xl" />
                <input v-else v-model="draft.notes.value" />
              </label>
              <label class="full">Review 链接<input v-model="draft.reviewLink" /></label>
              <div class="full otp-ei-edit">
                <div class="otp-ei-head"><span>Extra Impact</span><button class="otp-mini" @click="draft.extraImpacts.push('')">＋</button></div>
                <div v-for="(ei, i) in draft.extraImpacts" :key="i" class="otp-ei-row">
                  <textarea v-model="draft.extraImpacts[i]" class="ta-md" />
                  <button class="otp-mini" @click="draft.extraImpacts.splice(i, 1)">✕</button>
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, nextTick, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { parseTaskBlocks, validateTaskBlocks, serializeBlock, STATUS_LABELS } from '../../utils/guaxiTaskParser'
import { renderMiniMarkdown } from '../../utils/guaxiMiniMarkdown'

const props = defineProps({
  content: { type: String, default: '' },
  category: { type: String, default: '' },
  theme: { type: String, default: 'light' },
  titleLabel: { type: String, default: '任务预览' },
  activeBlockLine: { type: Number, default: 0 },
  canBack: { type: Boolean, default: false },
})
const emit = defineEmits(['rebuild', 'open-notes', 'goto-line', 'apply-block', 'copy-block', 'export-block', 'back', 'create-block', 'delete-block', 'insert-block'])

const blocks = computed(() => parseTaskBlocks(props.content, props.category))
const diagnostics = computed(() => validateTaskBlocks(props.content, props.category))
const errorCount = computed(() => diagnostics.value.filter((d) => d.severity === 'error').length)
const warnCount = computed(() => diagnostics.value.filter((d) => d.severity === 'warning').length)

const statusKeys = ['TODO', 'NEXT', 'DOING', 'WAITING', 'PAUSED', 'HOLD', 'CANCELLED', 'DONE']
const severityOptions = ['critical', 'major', 'normal', 'minor', 'trivial']
const originOptions = ['dev', 'QA', 'customer', 'other']
const fixOptions = ['debug', 'test', 'review', 'approval', 'fixed', 'other']
const typeOptions = computed(() => (props.category === 'life' || props.category === 'study') ? ['task', 'other'] : ['bug', 'task', 'other'])

const typeLabel = (t) => ({ bug: 'Bug', task: '任务', other: '其他' }[t] || (t || '任务'))
const statusLabel = (s) => STATUS_LABELS[s] || s || '待办'
const renderMd = (text) => renderMiniMarkdown(text, convertFileSrc)

// ── 分区（含顺序：Review 倒数第二、Extra Impact 最后且默认折叠）──
const sectionsOf = (b) => {
  const out = []
  if (b.rootCause) out.push({ key: 'rootCause', title: '根本原因', type: 'text', value: b.rootCause })
  if (b.conditionOfOccurrence) out.push({ key: 'condition', title: '复现步骤', type: 'pre', value: b.conditionOfOccurrence })
  if (b.fixedDetails) out.push({ key: 'fixed', title: '修复详情', type: 'pre', value: b.fixedDetails })
  if (b.testingSuggestions) out.push({ key: 'testing', title: '测试建议', type: 'pre', value: b.testingSuggestions })
  if (b.content) out.push({ key: 'content', title: '详情 (CONTENT)', type: 'md', value: b.content })
  if (b.notes && b.notes.kind !== 'none') out.push({ key: 'notes', title: '笔记 (NOTES)', type: 'notes', value: b.notes })
  if (b.reviewLink) out.push({ key: 'review', title: 'Review', type: 'link', value: b.reviewLink })
  if (b.extraImpacts && b.extraImpacts.length) out.push({ key: 'extra', title: 'Extra Impact', type: 'extra', value: b.extraImpacts, defaultCollapsed: true })
  return out
}
const secState = reactive({})
const secKey = (line, key) => `${line}:${key}`
const isCollapsed = (line, key, dft) => {
  const k = secKey(line, key)
  return k in secState ? secState[k] : !!dft
}
const toggleSec = (line, key, dft) => {
  const k = secKey(line, key)
  secState[k] = !(k in secState ? secState[k] : !!dft)
}
const copySection = async (sec, line) => {
  let text = ''
  if (sec.type === 'extra') text = (sec.value || []).join('\n\n')
  else if (sec.type === 'notes') text = sec.value?.value || ''
  else text = String(sec.value || '')
  try { await navigator.clipboard.writeText(text); doFlash(`sec:${line}:${sec.key}`) } catch {}
}
// 复制成功的短暂反馈（icon → ✓）
const flashed = ref('')
let flashTimer = null
const doFlash = (key) => {
  flashed.value = key
  if (flashTimer) clearTimeout(flashTimer)
  flashTimer = setTimeout(() => { flashed.value = '' }, 1200)
}
const copyCard = (b) => { emit('copy-block', b); doFlash(`copy:${b.blockStartLine}`) }
const exportCard = (b) => { emit('export-block', b); doFlash(`export:${b.blockStartLine}`) }
const copyDesc = async (b) => {
  try { await navigator.clipboard.writeText(b.description || ''); doFlash(`desc:${b.blockStartLine}`) } catch {}
}

// ── 搜索（高亮 + 上一个/下一个，按字段）──
const search = ref('')
const searchField = ref('all')
const matchFields = ['description', 'id', 'rootCause', 'conditionOfOccurrence', 'fixedDetails', 'testingSuggestions', 'content']
const blockMatches = (b) => {
  const q = search.value.trim().toLowerCase()
  if (!q) return false
  const fields = searchField.value === 'all' ? matchFields.concat('notes') : [searchField.value]
  return fields.some((f) => {
    if (f === 'notes') return String(b.notes?.value || '').toLowerCase().includes(q)
    return String(b[f] || '').toLowerCase().includes(q)
  })
}
const matches = computed(() => (search.value.trim() ? blocks.value.filter(blockMatches).map((b) => b.blockStartLine) : []))
const matchIdx = ref(0)
watch(matches, () => { matchIdx.value = 0 })
const gotoMatch = (dir) => {
  if (!matches.value.length) return
  matchIdx.value = (matchIdx.value + dir + matches.value.length) % matches.value.length
  emit('goto-line', matches.value[matchIdx.value])
  const el = cardRefs.get(matches.value[matchIdx.value])
  if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' })
}
// 高亮工具：转义 + <mark> 包裹匹配（用于 v-html 字段）
const escapeHtml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
const hl = (text) => {
  const raw = String(text ?? '')
  const q = search.value.trim()
  if (!q) return escapeHtml(raw)
  const esc = escapeHtml(raw)
  const qe = escapeHtml(q).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return esc.replace(new RegExp(qe, 'gi'), (m) => `<mark class="otp-hit">${m}</mark>`)
}

const showOutline = ref(false)

// ── 编辑模式 ──
const editingLine = ref(0)
const draft = reactive({})
const startEdit = (b) => {
  Object.assign(draft, JSON.parse(JSON.stringify(b)))
  if (!Array.isArray(draft.extraImpacts)) draft.extraImpacts = []
  if (!draft.notes || !draft.notes.kind) draft.notes = { kind: 'inline', value: '' }
  editingLine.value = b.blockStartLine
}
const cancelEdit = () => { editingLine.value = 0 }
const saveEdit = () => {
  draft.done = draft.status === 'DONE' || draft.status === 'CANCELLED'
  emit('apply-block', { startLine: draft.blockStartLine, endLine: draft.blockEndLine, text: serializeBlock(draft) })
  editingLine.value = 0
}

// ── 日历选择截止/创建时间 ──
// 设计：保留原文本输入（兼容手输与粘贴的多种格式），同时提供原生日历控件。
// 选择后统一写回与全局一致的中文格式（截止：2026年06月20日；创建：2026年06月20日 14:30:00）。
const pad2 = (n) => String(n).padStart(2, '0')
// 从已有字段值解析出年月日时分秒，兼容 ISO(2026-06-20)、斜杠(2026/6/20)、中文(2026年06月20日)及可选时间
const parseDateParts = (str) => {
  const s = String(str || '')
  const dm = s.match(/(\d{4})\s*[-/年]\s*(\d{1,2})\s*[-/月]\s*(\d{1,2})/)
  if (!dm) return null
  const tm = s.match(/(\d{1,2}):(\d{2})(?::(\d{2}))?/)
  return {
    y: +dm[1], mo: +dm[2], d: +dm[3],
    hh: tm ? +tm[1] : 0, mi: tm ? +tm[2] : 0, ss: tm ? +(tm[3] || 0) : 0,
  }
}
// 供原生 <input type="date"> 回显：yyyy-MM-dd
const toDateInput = (str) => {
  const p = parseDateParts(str)
  return p ? `${p.y}-${pad2(p.mo)}-${pad2(p.d)}` : ''
}
// 供原生 <input type="datetime-local" step="1"> 回显：yyyy-MM-ddTHH:mm:ss
const toDateTimeInput = (str) => {
  const p = parseDateParts(str)
  return p ? `${p.y}-${pad2(p.mo)}-${pad2(p.d)}T${pad2(p.hh)}:${pad2(p.mi)}:${pad2(p.ss)}` : ''
}
const onPickDeadline = (e) => {
  const m = String(e?.target?.value || '').match(/(\d{4})-(\d{2})-(\d{2})/)
  if (m) draft.deadline = `${m[1]}年${m[2]}月${m[3]}日`
}
const onPickCreateTime = (e) => {
  const m = String(e?.target?.value || '').match(/(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?/)
  if (m) draft.createTime = `${m[1]}年${m[2]}月${m[3]}日 ${m[4]}:${m[5]}:${m[6] || '00'}`
}

// ── 光标同步：滚动到激活块 ──
const bodyRef = ref(null)
const cardRefs = new Map()
const registerCardRef = (line, el) => { if (el) cardRefs.set(line, el) }
watch(() => props.activeBlockLine, async (line) => {
  if (!line) return
  await nextTick()
  const el = cardRefs.get(line)
  if (el && bodyRef.value) el.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
})
</script>

<script>
export default { name: 'GuaxiOrgTaskPreview' }
</script>

<style scoped>
.org-task-preview {
  /* 设计说明：
   * 1. 任务预览与主编辑器展示的是同一份 .org 文档内容，阅读态和编辑态应使用完全一致的字体栈。
   * 2. 原生 button/select/input/textarea 可能使用浏览器系统 UI 字体，不能只依赖根容器的普通继承。
   * 3. 使用组件级变量统一管理，后续调整主编辑器字体时可以直接同步这一处。
   */
  --otp-editor-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  display: flex; flex-direction: column; height: 100%; min-width: 0;
  font-family: var(--otp-editor-font-family);
  font-size: 14px;
}

/* Bug/Task 的所有条目、操作控件和编辑区统一继承主编辑器字体。
 * `button` 与表单元素需要显式指定，避免浏览器 user-agent 样式重新切回系统字体。
 */
.org-task-preview :where(
  button,
  input,
  select,
  option,
  textarea,
  label,
  pre,
  code
) {
  font-family: var(--otp-editor-font-family) !important;
}

/* renderMiniMarkdown 生成的节点位于 v-html 内容中，需要通过 :deep 明确覆盖其段落、代码及链接字体。 */
.org-task-preview :deep(.gmd),
.org-task-preview :deep(.gmd *) {
  font-family: var(--otp-editor-font-family) !important;
}

.theme-light { --otp-bg:#f7f8fa; --otp-card:#ffffff; --otp-border:#e3e6eb; --otp-text:#1f2328; --otp-sub:#57606a; --otp-chip:#eef1f5; --otp-hit:#ffe08a; }
.theme-dark { --otp-bg:#21252b; --otp-card:#282c34; --otp-border:#3e4451; --otp-text:#abb2bf; --otp-sub:#8b94a3; --otp-chip:#2f343d; --otp-hit:#7a6420; }
.org-task-preview { background: var(--otp-bg); color: var(--otp-text); }

.otp-toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--otp-border); flex: 0 0 auto; }
.otp-title { font-weight: 700; font-size: 15px; }
.otp-count { font-size: 12px; color: var(--otp-sub); }
.otp-spacer { flex: 1; }
.otp-btn { background: transparent; border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 7px; cursor: pointer; padding: 3px 10px; font-size: 13px; }
.otp-btn.icon { padding: 3px 9px; font-weight: 700; }
.otp-btn.sm { padding: 2px 8px; }
.otp-btn.on { background: #2080f0; color: #fff; border-color: #2080f0; }
.otp-btn.primary { background: #2080f0; color: #fff; border-color: #2080f0; }
.otp-btn:hover:not(:disabled) { opacity: 0.85; }
.otp-btn:disabled { opacity: 0.4; cursor: default; }

.otp-searchbar { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--otp-border); flex: 0 0 auto; }
.otp-sel, .otp-search { background: var(--otp-card); border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 6px; padding: 4px 8px; font-size: 12px; font-family: inherit; }
.otp-search { flex: 1; }
.otp-search-count { font-size: 12px; color: var(--otp-sub); min-width: 42px; text-align: center; }

.otp-main { flex: 1 1 auto; min-height: 0; display: flex; }
.otp-outline { flex: 0 0 200px; border-right: 1px solid var(--otp-border); overflow-y: auto; background: var(--otp-card); }
.otp-outline-head { padding: 8px 10px; font-size: 12px; color: var(--otp-sub); font-weight: 700; border-bottom: 1px solid var(--otp-border); position: sticky; top: 0; background: var(--otp-card); }
.otp-outline-item { display: flex; align-items: center; gap: 6px; padding: 7px 10px; cursor: pointer; border-bottom: 1px solid var(--otp-border); }
.otp-outline-item:hover { background: var(--otp-chip); }
.otp-outline-item.active { background: rgba(32,128,240,0.14); }
.otp-outline-item.done { opacity: 0.6; }
.oo-dot { width: 8px; height: 8px; border-radius: 50%; flex: 0 0 8px; }
.oo-dot.tt-bug { background: #d03050; } .oo-dot.tt-task { background: #2080f0; } .oo-dot.tt-other { background: #8a8f99; }
.oo-title { flex: 1; font-size: 12.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.oo-st { font-size: 10px; color: var(--otp-sub); }

.otp-body { flex: 1 1 auto; min-width: 0; overflow-y: auto; padding: 12px; display: flex; flex-direction: column; gap: 12px; }
.otp-empty { color: var(--otp-sub); padding: 24px; text-align: center; }
.otp-empty code, .otp-empty b { background: var(--otp-chip); padding: 1px 5px; border-radius: 4px; }

.otp-diags { border: 1px solid var(--otp-border); border-radius: 8px; overflow: hidden; }
.otp-diags-head { padding: 6px 10px; background: var(--otp-chip); font-size: 12px; font-weight: 700; }
.otp-diag { display: flex; gap: 8px; padding: 5px 10px; font-size: 12px; cursor: pointer; border-top: 1px solid var(--otp-border); }
.otp-diag:hover { background: var(--otp-chip); }
.otp-diag.error { color: #e45649; } .otp-diag.warning { color: #c18401; }
.otp-diag-line { flex: 0 0 auto; opacity: 0.8; }

.otp-card { background: var(--otp-card); border: 1px solid var(--otp-border); border-radius: 12px; padding: 14px 16px; box-shadow: 0 2px 8px rgba(0,0,0,0.07); }
.otp-card.done { opacity: 0.75; }
.otp-card.active { border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.25); }
.otp-ins { display: flex; align-items: center; gap: 6px; opacity: 0; transition: opacity .15s; margin: 2px 0; }
.otp-ins.bottom { margin-top: 10px; justify-content: flex-start; }
.otp-card:hover .otp-ins { opacity: 1; }
.otp-ins-label { font-size: 11px; color: var(--otp-sub); }
.otp-ins-btn { font-size: 11px; padding: 1px 8px; border-radius: 10px; border: 1px dashed var(--otp-border); background: transparent; color: var(--otp-sub); cursor: pointer; }
.otp-ins-btn.bug:hover { border-color: #d03050; color: #d03050; }
.otp-ins-btn.task:hover { border-color: #2080f0; color: #2080f0; }
.otp-card-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.otp-card-title { font-size: 16px; font-weight: 700; flex: 1; min-width: 120px; word-break: break-word; cursor: pointer; line-height: 1.5; }
.otp-card-title:hover { color: #2080f0; }
.otp-type { font-size: 12px; padding: 2px 9px; border-radius: 10px; color: #fff; font-weight: 600; }
.type-bug { background: #d03050; } .type-task { background: #2080f0; } .type-other { background: #8a8f99; }
.otp-pri { font-size: 12px; font-weight: 700; padding: 2px 8px; border-radius: 5px; color: #fff; }
.pri-A { background: #d03050; } .pri-B { background: #f0a020; } .pri-C { background: #909399; }
.otp-status { font-size: 13px; padding: 3px 11px; border-radius: 10px; background: var(--otp-chip); color: var(--otp-text); font-weight: 600; }
.st-DONE { background: rgba(24,160,88,0.18); color: #18a058; }
.st-DOING, .st-NEXT { background: rgba(32,128,240,0.18); color: #2080f0; }
.st-PAUSED, .st-WAITING, .st-HOLD { background: rgba(240,160,32,0.18); color: #f0a020; }
.st-CANCELLED { background: rgba(144,147,153,0.18); color: #909399; }
.otp-actions { display: inline-flex; gap: 4px; }
.otp-mini { background: var(--otp-chip); border: 1px solid var(--otp-border); color: var(--otp-text); border-radius: 6px; cursor: pointer; width: 28px; height: 28px; line-height: 1; font-size: 14px; }
.otp-mini.xs { width: 22px; height: 22px; font-size: 12px; }
.otp-mini:hover { background: var(--otp-border); }
.otp-mini.ok { background: rgba(24,160,88,0.2); color: #18a058; border-color: #18a058; }
.otp-mini.danger:hover { background: rgba(208,48,80,0.18); color: #d03050; border-color: #d03050; }

.otp-props { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 12px; }
.otp-prop { display: inline-flex; align-items: center; gap: 5px; background: var(--otp-chip); border-radius: 6px; padding: 3px 9px; font-size: 13px; }
.otp-prop .k { color: var(--otp-sub); }
.otp-prop.done-t .v { color: #18a058; }

.otp-sec { margin-top: 14px; }
.otp-sec-head { display: flex; align-items: center; gap: 6px; cursor: pointer; user-select: none; padding: 3px 0; border-bottom: 1px solid var(--otp-border); }
.otp-sec-caret { color: var(--otp-sub); font-size: 12px; width: 14px; }
.otp-sec-head h4 { margin: 0; font-size: 15px; font-weight: 700; flex: 1; letter-spacing: 0.01em; }
.otp-sec-actions { opacity: 0; transition: opacity .15s; }
.otp-sec-head:hover .otp-sec-actions { opacity: 1; }
.otp-sec-body { margin-top: 7px; }
.otp-text { font-size: 14px; line-height: 1.75; white-space: pre-wrap; word-break: break-word; }
.otp-pre { margin: 0 0 6px; font-size: 13.5px; line-height: 1.6; white-space: pre-wrap; word-break: break-word; background: var(--otp-chip); border-radius: 8px; padding: 10px 12px; }
.otp-extra-list { display: flex; flex-direction: column; gap: 8px; }
.otp-link, .otp-notes-file { color: #2080f0; word-break: break-all; font-size: 14px; }
:deep(mark.otp-hit) { background: var(--otp-hit); color: inherit; border-radius: 3px; padding: 0 1px; }

/* 编辑表单：大字段全宽 + 高文本域 */
.otp-edit-head { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; font-size: 15px; }
.otp-form { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 12px; }
.otp-form label { display: flex; flex-direction: column; gap: 4px; font-size: 13px; color: var(--otp-sub); font-weight: 600; }
.otp-form .full { grid-column: 1 / -1; }
.otp-form input, .otp-form select, .otp-form textarea {
  background: var(--otp-bg); border: 1px solid var(--otp-border); color: var(--otp-text);
  border-radius: 8px; padding: 8px 10px; font-size: 14px; font-family: inherit; resize: vertical; line-height: 1.6;
}
.otp-form textarea.ta-md { min-height: 110px; }
.otp-form textarea.ta-lg { min-height: 170px; }
.otp-form textarea.ta-xl { min-height: 240px; }
.otp-form input:focus, .otp-form select:focus, .otp-form textarea:focus { outline: none; border-color: #2080f0; box-shadow: 0 0 0 2px rgba(32,128,240,0.18); }
.otp-ei-edit { display: flex; flex-direction: column; gap: 6px; }
.otp-ei-head { display: flex; align-items: center; justify-content: space-between; font-size: 13px; color: var(--otp-sub); font-weight: 600; }
.otp-ei-row { display: flex; gap: 6px; }
.otp-ei-row textarea { flex: 1; }

/* 截止/创建时间：单输入框 + 右侧日历图标。
 * 原生日期控件以透明层叠加在图标上方，真实点击落在“日历指示器”上即可弹出系统日历，
 * 无需依赖 showPicker()（WebView2 下更稳）。 */
.otp-date-field { position: relative; }
.otp-date-field > input:not(.otp-date-overlay) { width: 100%; box-sizing: border-box; padding-right: 36px; }
.otp-date-field .otp-date-icon {
  position: absolute; right: 6px; top: 50%; transform: translateY(-50%);
  width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center;
  font-size: 15px; line-height: 1; pointer-events: none; opacity: 0.85;
}
/* 透明原生控件覆盖在图标处，承接点击 */
.otp-date-field .otp-date-overlay {
  position: absolute; right: 4px; top: 50%; transform: translateY(-50%);
  width: 28px; height: 28px; padding: 0; margin: 0; border: none; background: transparent;
  opacity: 0; cursor: pointer;
}
.theme-light .otp-date-field .otp-date-overlay { color-scheme: light; }
.theme-dark .otp-date-field .otp-date-overlay { color-scheme: dark; }
/* 让整块透明控件都成为可点击的日历指示器热区 */
.otp-date-field .otp-date-overlay::-webkit-calendar-picker-indicator {
  position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; padding: 0;
  opacity: 0; cursor: pointer;
}

.gmd :deep(.gmd-p) { margin: 4px 0; font-size: 14px; line-height: 1.7; word-break: break-word; }
.gmd :deep(.gmd-pre) { background: var(--otp-chip); border-radius: 8px; padding: 10px 12px; overflow-x: auto; font-size: 13.5px; line-height: 1.55; }
.gmd :deep(.gmd-code-inline) { background: var(--otp-chip); border-radius: 4px; padding: 1px 5px; font-size: 13.5px; }
.gmd :deep(.gmd-hr) { border: none; border-top: 1px solid var(--otp-border); margin: 10px 0; }
.gmd :deep(.gmd-img), .gmd :deep(.gmd-img-wrap img) { max-width: 100%; border-radius: 6px; display: block; margin: 6px 0; }
.gmd :deep(.gmd-link) { color: #2080f0; word-break: break-all; }

.otp-body::-webkit-scrollbar, .otp-outline::-webkit-scrollbar { width: 9px; }
.otp-body::-webkit-scrollbar-thumb, .otp-outline::-webkit-scrollbar-thumb { background: rgba(140,140,150,0.38); border-radius: 10px; border: 2.5px solid transparent; background-clip: padding-box; }
</style>
