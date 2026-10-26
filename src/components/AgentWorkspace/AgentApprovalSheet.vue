<template>
  <div v-if="current" class="approval-root" @click.self="onBackdrop">
    <div class="approval-card" role="dialog" aria-modal="true" :aria-label="headline">
      <header class="approval-head">
        <span class="approval-kind">{{ kindLabel }}</span>
        <span v-if="serverName" class="approval-server">{{ serverName }}</span>
        <span v-if="queue.length > 1" class="approval-progress">{{ index + 1 }} / {{ queue.length }}</span>
        <button type="button" class="approval-close" title="拒绝并关闭" @click="deny">
          <Icon :icon="xIcon" width="13" height="13" />
        </button>
      </header>

      <p class="approval-title">{{ headline }}</p>
      <p v-if="subline" class="approval-sub">{{ subline }}</p>

      <!-- 命令：原文、工作目录、代理给的理由，排在按钮前面。 -->
      <template v-if="kind === 'command'">
        <pre class="approval-pre">{{ commandText || '（没有给出命令内容）' }}</pre>
        <div v-if="cwdText" class="approval-meta">
          <span class="meta-key">目录</span>
          <span class="meta-val mono">{{ cwdText }}</span>
        </div>
      </template>

      <!-- 文件改动：逐文件增删，按需展开补丁。 -->
      <template v-else-if="kind === 'file'">
        <div v-if="changes.length" class="approval-changes">
          <div v-for="(c, i) in changes" :key="c.path || i" class="approval-change">
            <button type="button" class="change-row" @click="togglePatch(i)">
              <span class="change-kind">{{ kindOfChange(c) }}</span>
              <span class="change-path" :title="c.path">{{ c.path }}</span>
              <span class="change-nums">
                <span v-if="changeStats(c).added" class="num-add">+{{ changeStats(c).added }}</span>
                <span v-if="changeStats(c).removed" class="num-del">−{{ changeStats(c).removed }}</span>
              </span>
              <Icon :icon="chevronIcon" width="11" height="11" class="change-caret" />
            </button>
            <pre v-if="expanded[i]" class="change-patch">{{ c.diff || '（没有补丁内容）' }}</pre>
          </div>
        </div>
        <div v-else class="approval-empty">这次改动没有附带文件清单</div>
      </template>

      <!-- 额外权限：把请求的权限摊平成「名字 = 值」。 -->
      <template v-else-if="kind === 'permission'">
        <dl v-if="permissionEntries.length" class="approval-perms">
          <div v-for="(entry, i) in permissionEntries" :key="i" class="approval-perm">
            <dt class="mono">{{ entry.key }}</dt>
            <dd class="mono">{{ entry.value }}</dd>
          </div>
        </dl>
        <div v-else class="approval-empty">没有列出具体权限</div>
      </template>

      <!-- 用户输入请求：每个问题一组选项，或一个输入框。 -->
      <template v-else-if="isInput">
        <div class="approval-questions">
          <div v-for="(q, qi) in questions" :key="q.id || qi" class="approval-question">
            <div class="question-head">
              <span v-if="q.header" class="question-header">{{ q.header }}</span>
              <span class="question-text">{{ q.question || '需要补充信息' }}</span>
            </div>
            <div v-if="q.options?.length" class="question-options">
              <button
                v-for="(opt, oi) in q.options"
                :key="oi"
                type="button"
                class="question-option"
                :class="{ 'question-option--on': isChosen(q, qi, opt) }"
                :title="opt.description || ''"
                @click="chooseOption(q, qi, opt)"
              >
                {{ opt.label || opt.value }}
              </button>
            </div>
            <input
              v-model="answers[questionKey(q, qi)]"
              class="question-input"
              type="text"
              :placeholder="q.options?.length ? '其他答案（可选）' : '输入回答'"
              @focus="clearOptions(qi)"
            />
          </div>
        </div>
      </template>

      <!-- MCP elicitation：按服务端给的 schema 生成表单。 -->
      <template v-else-if="isElicitation">
        <div v-if="elicitationFields.length" class="approval-fields">
          <div v-for="field in elicitationFields" :key="field.key" class="approval-field">
            <label class="field-label">
              {{ field.title }}
              <span v-if="field.required" class="field-req">必填</span>
            </label>
            <p v-if="field.description" class="field-desc">{{ field.description }}</p>
            <div v-if="field.options.length" class="question-options">
              <button
                v-for="opt in field.options"
                :key="opt"
                type="button"
                class="question-option"
                :class="{ 'question-option--on': String(elicitation[field.key]) === String(opt) }"
                @click="elicitation[field.key] = opt"
              >
                {{ opt }}
              </button>
            </div>
            <input
              v-else
              v-model="elicitation[field.key]"
              class="question-input"
              type="text"
              :placeholder="field.type === 'number' ? '数字' : '输入'"
            />
          </div>
        </div>
        <pre v-else class="approval-pre">{{ schemaText }}</pre>
        <details class="approval-raw">
          <summary>原始请求</summary>
          <pre>{{ jsonText }}</pre>
        </details>
      </template>

      <!-- 其余类型：至少把 params 原样摊开，不能只剩一个「允许」。 -->
      <template v-else>
        <pre class="approval-pre">{{ jsonText }}</pre>
      </template>

      <div v-if="errorText" class="approval-error">{{ errorText }}</div>

      <footer class="approval-actions">
        <button type="button" class="approval-btn approval-btn--ghost" @click="deny">
          {{ isInput ? '取消' : '拒绝' }}
        </button>
        <template v-if="isInput">
          <button type="button" class="approval-btn approval-btn--primary" @click="submitAnswers">
            提交回答
          </button>
        </template>
        <template v-else-if="isElicitation">
          <button type="button" class="approval-btn approval-btn--primary" @click="acceptElicitation">
            接受
          </button>
        </template>
        <template v-else-if="kind === 'permission'">
          <button type="button" class="approval-btn approval-btn--ghost" @click="acceptPermission('turn')">
            仅本轮
          </button>
          <button type="button" class="approval-btn approval-btn--primary" @click="acceptPermission('session')">
            本会话允许
          </button>
        </template>
        <template v-else>
          <button
            v-if="sessionAllowed"
            type="button"
            class="approval-btn approval-btn--ghost"
            title="本次会话内不再询问同类操作"
            @click="accept('acceptForSession')"
          >
            本会话允许
          </button>
          <button type="button" class="approval-btn approval-btn--primary" @click="accept('accept')">
            允许一次
          </button>
        </template>
      </footer>
    </div>
  </div>
</template>

<script setup>
/**
 * 审批浮层。
 *
 * Harness 与聊天框最大的区别就在这块：模型要做的事先给你看一眼，你决定放不放。
 * 四条原则：
 * 1. 先给证据：命令原文、工作目录、逐文件增删，排在按钮前面。
 * 2. 拒绝永远比允许好点：拒绝不需要二次确认，关闭按钮也是拒绝。
 * 3. 一次只问一件事：多条审批排队逐个处理，不做「全部允许」。
 * 4. 不同请求不同形状：命令/文件是 decision，权限是 scope，用户输入是 answers，
 *    MCP elicitation 是 schema 表单——把它们压成同一个按钮组只会让回包出错。
 */
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
import xIcon from '@iconify-icons/lucide/x'
import chevronIcon from '@iconify-icons/lucide/chevron-right'

const props = defineProps({
  queue: { type: Array, default: () => [] },
  index: { type: Number, default: 0 },
})

const emit = defineEmits(['decide'])

const answers = reactive({})
const chosen = reactive({})
const elicitation = reactive({})
const expanded = reactive({})
const errorText = ref('')

const current = computed(() => props.queue[props.index] || null)
const params = computed(() => current.value?.params || {})
const method = computed(() => String(current.value?.method || ''))

const isElicitation = computed(() =>
  current.value?.kind === 'elicitation' || method.value === 'mcpServer/elicitation/request',
)
const isInput = computed(() =>
  !isElicitation.value
  && (current.value?.kind === 'input' || method.value.includes('requestUserInput')),
)

const kind = computed(() => {
  if (isInput.value) return 'input'
  if (isElicitation.value) return 'elicitation'
  const k = String(current.value?.kind || '')
  if (k) return k
  if (params.value.command) return 'command'
  if (params.value.changes) return 'file'
  if (params.value.permissions) return 'permission'
  return 'other'
})

const kindLabel = computed(() => ({
  command: '执行命令',
  file: '修改文件',
  permission: '额外权限',
  input: '需要信息',
  elicitation: '连接器请求',
  other: '请求',
}[kind.value] || '请求'))

const serverName = computed(() => String(params.value.serverName || params.value.server || ''))

const headline = computed(() => {
  const p = params.value
  switch (kind.value) {
    case 'command':
      return p.reason || p.justification || '模型请求执行一条命令'
    case 'file':
      return p.reason || p.justification || '模型请求修改文件'
    case 'permission':
      return p.reason || p.justification || '模型请求额外权限'
    case 'input':
      return '模型需要你补充信息才能继续'
    case 'elicitation':
      return p.message || '一个连接器请求你提供信息'
    default:
      return '模型请求执行一个受限操作'
  }
})

const subline = computed(() => {
  if (isInput.value) return '回答会原样交给模型，作为这一步的输入。'
  if (isElicitation.value) return '这是 MCP 连接器发起的请求，内容由它自己定义。'
  return ''
})

const commandText = computed(() => {
  const command = params.value.command
  if (Array.isArray(command)) return command.join(' ')
  return String(command || params.value.cmd || '')
})

const cwdText = computed(() => String(params.value.cwd || ''))

const changes = computed(() => {
  const list = params.value.changes || params.value.fileChanges || params.value.patch?.changes
  return Array.isArray(list) ? list : []
})

const kindOfChange = (c) => {
  const value = String(c?.kind || c?.type || 'update')
  return { add: '新增', delete: '删除', update: '修改', move: '移动' }[value] || value
}

function countPatch(patch) {
  let added = 0
  let removed = 0
  for (const line of String(patch || '').split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) added += 1
    else if (line.startsWith('-') && !line.startsWith('---')) removed += 1
  }
  return { added, removed }
}

const changeStats = (c) => {
  if (typeof c?.addedLines === 'number' || typeof c?.removedLines === 'number') {
    return { added: Number(c.addedLines || 0), removed: Number(c.removedLines || 0) }
  }
  return countPatch(c?.diff)
}

const togglePatch = (index) => {
  expanded[index] = !expanded[index]
}

const permissionEntries = computed(() => {
  const raw = params.value.permissions || params.value.requestedPermissions
  const rows = []
  const walk = (value, prefix) => {
    if (value === null || value === undefined) return
    if (Array.isArray(value)) {
      rows.push({ key: prefix || '权限', value: value.join('、') || '（空）' })
      return
    }
    if (typeof value === 'object') {
      for (const [key, inner] of Object.entries(value)) {
        walk(inner, prefix ? `${prefix}.${key}` : key)
      }
      return
    }
    rows.push({ key: prefix || '权限', value: String(value) })
  }
  if (Array.isArray(raw)) {
    for (const item of raw) {
      if (typeof item === 'string') rows.push({ key: item, value: '允许' })
      else walk(item, '')
    }
  } else {
    walk(raw, '')
  }
  return rows
})

const questions = computed(() => {
  if (!isInput.value) return []
  const list = params.value.questions
  if (Array.isArray(list) && list.length) return list
  return [{ id: 'answer', question: params.value.message || params.value.prompt || '' }]
})

const questionKey = (q, index) => String(q?.id || index)

const isChosen = (q, index, opt) =>
  chosen[index] === (opt?.label ?? opt?.value ?? String(opt))

const chooseOption = (q, index, opt) => {
  const value = String(opt?.label ?? opt?.value ?? opt)
  chosen[index] = value
  answers[questionKey(q, index)] = value
}

const clearOptions = (index) => {
  chosen[index] = ''
}

const schemaProps = computed(() => params.value.requestedSchema?.properties || {})
const schemaRequired = computed(() => params.value.requestedSchema?.required || [])

const elicitationFields = computed(() =>
  Object.entries(schemaProps.value).map(([key, prop]) => ({
    key,
    title: String(prop?.title || key),
    description: String(prop?.description || ''),
    type: String(prop?.type || 'string'),
    required: schemaRequired.value.includes(key),
    options: Array.isArray(prop?.enum) ? prop.enum : [],
  })),
)

const schemaText = computed(() => JSON.stringify(params.value.requestedSchema || {}, null, 2))
const jsonText = computed(() => JSON.stringify(params.value || {}, null, 2))

const sessionAllowed = computed(() => kind.value === 'command' || kind.value === 'file')

const reset = () => {
  for (const key of Object.keys(answers)) delete answers[key]
  for (const key of Object.keys(chosen)) delete chosen[key]
  for (const key of Object.keys(elicitation)) delete elicitation[key]
  for (const key of Object.keys(expanded)) delete expanded[key]
  errorText.value = ''

  for (const [index, q] of questions.value.entries()) {
    answers[questionKey(q, index)] = ''
  }
  for (const field of elicitationFields.value) {
    elicitation[field.key] = field.options.length ? field.options[0] : ''
  }
}

watch(
  () => current.value?.requestId,
  () => reset(),
  { immediate: true },
)

const emitDecide = (action, options = {}) => {
  if (!current.value) return
  emit('decide', {
    requestId: current.value.requestId,
    action,
    options,
  })
}

const accept = (action) => emitDecide(action)

const deny = () => emitDecide('decline')

const acceptPermission = (scope) => {
  const permissions = params.value.permissions || params.value.requestedPermissions || {}
  emitDecide(scope === 'session' ? 'acceptForSession' : 'accept', { permissions, scope })
}

const submitAnswers = () => {
  const payload = {}
  for (const [index, q] of questions.value.entries()) {
    const value = String(answers[questionKey(q, index)] || '').trim()
    if (!value) {
      errorText.value = '请先回答全部问题'
      return
    }
    payload[questionKey(q, index)] = { answers: [value] }
  }
  emitDecide('accept', { answers: payload })
}

const acceptElicitation = () => {
  const content = {}
  for (const field of elicitationFields.value) {
    const raw = elicitation[field.key]
    if ((raw === '' || raw === undefined || raw === null) && field.required) {
      errorText.value = `「${field.title}」是必填项`
      return
    }
    if (raw === '' || raw === undefined || raw === null) continue
    if (field.type === 'number' || field.type === 'integer') content[field.key] = Number(raw)
    else if (field.type === 'boolean') content[field.key] = raw === true || raw === 'true'
    else content[field.key] = raw
  }
  emitDecide('accept', { content })
}

const onBackdrop = () => {
  // 点背景不当作拒绝：审批应该由明确的按钮决定。
}

const onKeydown = (event) => {
  if (event.key === 'Escape' && current.value) {
    event.preventDefault()
    deny()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<style scoped>
.approval-root {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: 24px 24px 22px;
  background: rgba(10, 12, 15, 0.22);
  backdrop-filter: blur(2px);
}

.approval-card {
  width: min(620px, 100%);
  max-height: min(78vh, 720px);
  overflow-y: auto;
  padding: 15px 17px 14px;
  border-radius: 12px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.3);
  animation: approval-in 160ms cubic-bezier(0.22, 1, 0.36, 1);
}

@keyframes approval-in {
  from { opacity: 0; transform: translateY(14px) scale(0.99); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.approval-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 8px;
}

.approval-kind {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: #b47d1a;
}

.approval-server {
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  color: var(--guaxi-text-secondary);
  opacity: 0.7;
}

.approval-progress {
  margin-left: auto;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  color: var(--guaxi-text-secondary);
}

.approval-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  margin-left: 6px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
}

.approval-close:hover {
  background: var(--guaxi-hover-bg);
  color: var(--guaxi-text-primary);
}

.approval-title {
  margin: 0 0 3px;
  font-size: 14.5px;
  line-height: 1.5;
  color: var(--guaxi-text-primary);
}

.approval-sub {
  margin: 0 0 9px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
}

.approval-pre {
  margin: 8px 0;
  padding: 9px 11px;
  max-height: 240px;
  overflow: auto;
  border-radius: 6px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}

.approval-meta {
  display: flex;
  gap: 8px;
  font-size: 11.5px;
  margin-bottom: 6px;
}

.meta-key { color: var(--guaxi-text-secondary); }
.meta-val { color: var(--guaxi-text-primary); }
.mono { font-family: var(--guaxi-editor-font-family); font-size: 11px; }

.approval-changes {
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  overflow: hidden;
  margin: 8px 0;
}

.approval-change + .approval-change {
  border-top: 1px solid var(--guaxi-border);
}

.change-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 5px 9px;
  border: none;
  background: transparent;
  color: var(--guaxi-text-primary);
  text-align: left;
  cursor: pointer;
}

.change-row:hover {
  background: var(--guaxi-hover-bg);
}

.change-kind {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
  font-size: 10px;
}

.change-path {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
}

.change-nums {
  display: flex;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 9.5px;
}

.change-caret {
  flex: 0 0 auto;
  opacity: 0.4;
}

.change-patch {
  margin: 0;
  padding: 8px 11px;
  max-height: 220px;
  overflow: auto;
  border-top: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-all;
}

.num-add { color: #4f8a5b; }
.num-del { color: #b42318; }

.approval-perms {
  margin: 8px 0;
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  overflow: hidden;
}

.approval-perm {
  display: flex;
  gap: 10px;
  padding: 5px 9px;
  font-size: 11px;
}

.approval-perm + .approval-perm {
  border-top: 1px solid var(--guaxi-border);
}

.approval-perm dt {
  flex: 0 0 46%;
  color: var(--guaxi-text-secondary);
  word-break: break-all;
}

.approval-perm dd {
  margin: 0;
  min-width: 0;
  color: var(--guaxi-text-primary);
  word-break: break-all;
}

.approval-questions {
  margin: 8px 0;
}

.approval-question + .approval-question {
  margin-top: 12px;
}

.question-head {
  display: flex;
  align-items: baseline;
  gap: 7px;
  margin-bottom: 5px;
}

.question-header {
  font-family: var(--guaxi-editor-font-family);
  font-size: 9px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
  opacity: 0.65;
}

.question-text {
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--guaxi-text-primary);
}

.question-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 6px;
}

.question-option {
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 11.5px;
  font-family: inherit;
  cursor: pointer;
  transition: border-color 120ms ease, color 120ms ease, background 120ms ease;
}

.question-option:hover {
  border-color: var(--guaxi-text-secondary);
  color: var(--guaxi-text-primary);
}

.question-option--on {
  border-color: rgba(180, 125, 26, 0.6);
  background: rgba(180, 125, 26, 0.08);
  color: #b47d1a;
}

.question-input {
  width: 100%;
  height: 29px;
  padding: 0 9px;
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
  outline: none;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font: inherit;
  font-size: 12px;
}

.question-input:focus {
  border-color: rgba(180, 125, 26, 0.55);
}

.approval-fields {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 8px 0;
}

.field-label {
  display: flex;
  align-items: baseline;
  gap: 7px;
  font-size: 12px;
  color: var(--guaxi-text-primary);
}

.field-req {
  font-family: var(--guaxi-editor-font-family);
  font-size: 9px;
  color: #b47d1a;
}

.field-desc {
  margin: 2px 0 5px;
  font-size: 11px;
  line-height: 1.55;
  color: var(--guaxi-text-secondary);
}

.approval-raw {
  margin-top: 6px;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
}

.approval-raw summary {
  cursor: pointer;
  opacity: 0.7;
}

.approval-raw pre {
  margin: 6px 0 0;
  padding: 8px 10px;
  max-height: 200px;
  overflow: auto;
  border-radius: 5px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
}

.approval-empty {
  margin: 8px 0;
  font-size: 11.5px;
  color: var(--guaxi-text-secondary);
  opacity: 0.6;
}

.approval-error {
  margin: 6px 0;
  font-size: 12px;
  color: #b42318;
}

.approval-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}

.approval-btn {
  height: 29px;
  padding: 0 14px;
  border-radius: 6px;
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-primary);
  transition: background 120ms ease, border-color 120ms ease;
}

.approval-btn--ghost:hover {
  background: var(--guaxi-hover-bg);
}

.approval-btn--primary {
  border-color: #b47d1a;
  background: #b47d1a;
  color: #fff;
}

.approval-btn--primary:hover {
  background: #a06d14;
  border-color: #a06d14;
}

@media (max-width: 720px) {
  .approval-root {
    padding: 12px;
  }

  .approval-card {
    max-height: 86vh;
  }
}

@media (prefers-reduced-motion: reduce) {
  .approval-card {
    animation: none;
  }
}
</style>
