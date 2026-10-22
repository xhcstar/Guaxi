<template>
  <div v-if="current" class="approval-root">
    <div class="approval-card" role="dialog" aria-modal="true" :aria-label="headline">
      <header class="approval-head">
        <span class="approval-kind">{{ kindLabel }}</span>
        <span class="approval-progress" v-if="queue.length > 1">
          {{ index + 1 }} / {{ queue.length }}
        </span>
      </header>

      <p class="approval-title">{{ headline }}</p>
      <p v-if="subline" class="approval-sub">{{ subline }}</p>

      <!-- 命令：完整命令 + 工作目录。这是审批里最需要看清的东西。 -->
      <pre v-if="commandText" class="approval-pre">{{ commandText }}</pre>
      <div v-if="cwdText" class="approval-meta">
        <span class="meta-key">目录</span>
        <span class="meta-val mono">{{ cwdText }}</span>
      </div>

      <!-- 文件改动：逐文件列增删，展开看补丁 -->
      <div v-if="changes.length" class="approval-changes">
        <div v-for="(c, i) in changes" :key="i" class="approval-change" :title="c.path">
          <span class="change-kind">{{ kindOfChange(c) }}</span>
          <span class="change-path">{{ c.path }}</span>
          <span class="change-nums">
            <span v-if="c.addedLines" class="num-add">+{{ c.addedLines }}</span>
            <span v-if="c.removedLines" class="num-del">−{{ c.removedLines }}</span>
          </span>
        </div>
      </div>

      <!-- 用户输入请求：给一组选项或一个文本框 -->
      <div v-if="questions.length" class="approval-questions">
        <div v-for="(q, qi) in questions" :key="qi" class="approval-question">
          <div class="question-text">{{ q.question || q.header || '需要补充信息' }}</div>
          <div v-if="q.options?.length" class="question-options">
            <button
              v-for="(opt, oi) in q.options"
              :key="oi"
              type="button"
              class="question-option"
              :class="{ 'question-option--on': answers[qi] === (opt.label || opt.value) }"
              @click="answers[qi] = opt.label || opt.value"
            >
              {{ opt.label || opt.value }}
            </button>
          </div>
          <input
            v-else
            v-model="answers[qi]"
            class="question-input"
            type="text"
            placeholder="输入回答"
          />
        </div>
      </div>

      <div v-if="errorText" class="approval-error">{{ errorText }}</div>

      <footer class="approval-actions">
        <button type="button" class="approval-btn approval-btn--ghost" @click="deny">拒绝</button>
        <button
          v-if="sessionOption"
          type="button"
          class="approval-btn approval-btn--ghost"
          :title="sessionOption.title"
          @click="approve(true)"
        >
          {{ sessionOption.label }}
        </button>
        <button type="button" class="approval-btn approval-btn--primary" @click="approve(false)">
          允许
        </button>
      </footer>
    </div>
  </div>
</template>

<script setup>
/**
 * 审批浮层。
 *
 * Harness 与聊天框最大的区别就在这块：模型要做的事先给你看一眼，你决定放不放。
 * 三条原则：
 * 1. **先给证据**：命令原文、工作目录、逐文件增删，排在按钮前面。
 * 2. **拒绝永远比允许好点**：默认焦点在"拒绝"，且拒绝不需要二次确认。
 * 3. **一次只问一件事**：多条审批排队逐个处理，不做"全部允许"。
 *    （批量放行会把审批变成形式主义，那比没有审批更糟。）
 */
import { computed, ref, watch } from 'vue'

const props = defineProps({
  queue: { type: Array, default: () => [] },
  index: { type: Number, default: 0 },
})

const emit = defineEmits(['decide'])

const answers = ref([])
const errorText = ref('')

const current = computed(() => props.queue[props.index] || null)

const params = computed(() => current.value?.params || {})

const kindLabel = computed(() => ({
  command: '执行命令',
  file: '修改文件',
  permission: '额外权限',
  input: '需要信息',
  other: '请求',
}[current.value?.kind] || '请求'))

const headline = computed(() => {
  const p = params.value
  switch (current.value?.kind) {
    case 'command':
      return p.reason || p.justification || '模型请求执行一条命令'
    case 'file':
      return p.reason || p.justification || '模型请求修改文件'
    case 'permission':
      return p.reason || '模型请求额外权限'
    case 'input':
      return '模型需要你补充信息才能继续'
    default:
      return '模型请求执行一个受限操作'
  }
})

const subline = computed(() => {
  const p = params.value
  if (current.value?.kind === 'permission') {
    const perms = p.permissions || p.requestedPermissions
    if (Array.isArray(perms) && perms.length) {
      return `请求：${perms.map((x) => (typeof x === 'string' ? x : x?.type || JSON.stringify(x))).join('、')}`
    }
  }
  return ''
})

const commandText = computed(() => {
  const p = params.value
  if (Array.isArray(p.command)) return p.command.join(' ')
  return String(p.command || p.cmd || '')
})

const cwdText = computed(() => String(params.value.cwd || ''))

const changes = computed(() => {
  const p = params.value
  const list = p.changes || p.fileChanges || p.patch?.changes
  return Array.isArray(list) ? list : []
})

const kindOfChange = (c) => {
  const k = String(c?.kind || c?.type || 'update')
  return { add: '新增', delete: '删除', update: '修改', move: '移动' }[k] || k
}

const questions = computed(() => {
  if (current.value?.kind !== 'input') return []
  const qs = params.value.questions
  if (Array.isArray(qs) && qs.length) return qs
  return [{ question: params.value.message || params.value.prompt || '' }]
})

/**
 * 「本会话内允许」只对命令类有意义——codex 的审批响应支持 session 作用域。
 * 文件与权限类不给这个口子：放开一次改文件的授权，后面就很难收回了。
 */
const sessionOption = computed(() => {
  if (current.value?.kind !== 'command') return null
  return { label: '本会话允许', title: '本次会话内不再询问同类命令' }
})

watch(
  () => current.value?.requestId,
  () => {
    answers.value = []
    errorText.value = ''
  },
)

/** codex 的审批回包结构：{ decision }。不同方法的字段略有差异，这里统一成三种。 */
const decide = (decision, extra = {}) => {
  if (!current.value) return
  emit('decide', {
    requestId: current.value.requestId,
    decision: { decision, ...extra },
  })
}

const approve = (forSession) => {
  if (questions.value.length) {
    const filled = questions.value.map((q, i) => ({
      questionId: q.id || String(i),
      answer: answers.value[i] || '',
    }))
    if (filled.some((f) => !f.answer)) {
      errorText.value = '请先回答全部问题'
      return
    }
    decide('accept', { answers: filled })
    return
  }
  decide(forSession ? 'acceptForSession' : 'accept')
}

const deny = () => decide('decline')
</script>

<style scoped>
.approval-root {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(10, 12, 15, 0.42);
}

.approval-card {
  width: min(560px, 100%);
  max-height: min(76vh, 680px);
  overflow-y: auto;
  padding: 16px 18px 14px;
  border-radius: 9px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.32);
}

.approval-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 8px;
}

.approval-kind {
  font-size: 11.5px;
  color: #b47d1a;
}

.approval-progress {
  margin-left: auto;
  font-size: 11px;
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
}

.approval-title {
  margin: 0 0 4px;
  font-size: 14.5px;
  line-height: 1.5;
  color: var(--guaxi-text-primary);
}

.approval-sub {
  margin: 0 0 8px;
  font-size: 12.5px;
  color: var(--guaxi-text-secondary);
}

.approval-pre {
  margin: 8px 0;
  padding: 9px 11px;
  max-height: 220px;
  overflow: auto;
  border-radius: 5px;
  background: var(--guaxi-header-bg);
  border: 1px solid var(--guaxi-border);
  color: var(--guaxi-text-primary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-all;
}

.approval-meta {
  display: flex;
  gap: 8px;
  font-size: 12px;
  margin-bottom: 6px;
}

.meta-key { color: var(--guaxi-text-secondary); }
.meta-val { color: var(--guaxi-text-primary); }
.mono { font-family: var(--guaxi-editor-font-family); font-size: 11.5px; }

.approval-changes {
  border: 1px solid var(--guaxi-border);
  border-radius: 5px;
  overflow: hidden;
  margin: 8px 0;
}

.approval-change {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 4px 9px;
  font-size: 12px;
}

.approval-change + .approval-change {
  border-top: 1px solid var(--guaxi-border);
}

.change-kind {
  flex: 0 0 auto;
  color: var(--guaxi-text-secondary);
  font-size: 11px;
}

.change-path {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11.5px;
}

.change-nums {
  display: flex;
  gap: 5px;
  font-family: var(--guaxi-editor-font-family);
  font-size: 11px;
}

.num-add { color: #4f8a5b; }
.num-del { color: #b42318; }

.approval-questions {
  margin: 8px 0;
}

.approval-question + .approval-question {
  margin-top: 10px;
}

.question-text {
  font-size: 12.5px;
  color: var(--guaxi-text-primary);
  margin-bottom: 5px;
}

.question-options {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.question-option {
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  cursor: pointer;
}

.question-option--on {
  border-color: var(--guaxi-text-secondary);
  color: var(--guaxi-text-primary);
}

.question-input {
  width: 100%;
  height: 28px;
  padding: 0 9px;
  border-radius: 5px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font: inherit;
  font-size: 12.5px;
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
  height: 28px;
  padding: 0 14px;
  border-radius: 5px;
  font-size: 12.5px;
  cursor: pointer;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-primary);
}

.approval-btn--ghost:hover {
  background: var(--guaxi-hover-bg);
}

.approval-btn--primary {
  border-color: rgba(180, 125, 26, 0.6);
  background: rgba(180, 125, 26, 0.12);
  color: #b47d1a;
}

.approval-btn--primary:hover {
  background: rgba(180, 125, 26, 0.2);
}
</style>
