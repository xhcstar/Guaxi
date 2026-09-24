<template>
  <teleport to="body">
    <div
      v-if="visible"
      class="guaxi-codegen-panel"
      :style="panelStyle"
      @mousedown.stop
      @click.stop
    >
      <div class="guaxi-codegen-panel__header">
        <span class="guaxi-codegen-panel__title">AI 生成 Python 代码</span>
        <button class="guaxi-codegen-panel__close" title="关闭" @click="close">×</button>
      </div>

      <textarea
        ref="inputRef"
        v-model="instruction"
        class="guaxi-codegen-panel__input"
        placeholder="用自然语言描述你想实现什么，例如：读取 data.csv，按月份分组统计销售额并画柱状图（Ctrl+Enter 生成）"
        :disabled="loading"
        @keydown.ctrl.enter.prevent="generate"
      />

      <div class="guaxi-codegen-panel__actions">
        <button
          class="guaxi-codegen-panel__btn guaxi-codegen-panel__btn--primary"
          :disabled="loading || !instruction.trim()"
          @click="generate"
        >
          {{ loading ? '生成中…' : '生成' }}
        </button>
        <span v-if="loading" class="guaxi-codegen-panel__hint">本地模型可能较慢，请稍候</span>
      </div>

      <div v-if="error" class="guaxi-codegen-panel__error">{{ error }}</div>
      <div v-if="warning" class="guaxi-codegen-panel__warning">{{ warning }}</div>

      <div v-if="generatedCode" class="guaxi-codegen-panel__result">
        <pre class="guaxi-codegen-panel__code">{{ generatedCode }}</pre>
        <div class="guaxi-codegen-panel__actions">
          <button class="guaxi-codegen-panel__btn guaxi-codegen-panel__btn--primary" @click="emitInsert('insert')">插入到光标处</button>
          <button class="guaxi-codegen-panel__btn" @click="emitInsert('replace')">替换整个 cell</button>
          <button class="guaxi-codegen-panel__btn" :disabled="loading" @click="generate">重新生成</button>
        </div>
      </div>
    </div>
  </teleport>
</template>

<!-- SCRIPT_PLACEHOLDER -->

<script setup>
import { ref, computed, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useChatStore } from '../../stores/chatStore'

const props = defineProps({
  visible: { type: Boolean, default: false },
  anchor: { type: Object, default: () => ({ x: 0, y: 0 }) },
  context: { type: String, default: '' },
  sessionId: { type: String, default: '' },
})
const emit = defineEmits(['update:visible', 'insert'])

const chatStore = useChatStore()
const instruction = ref('')
const loading = ref(false)
const generatedCode = ref('')
const error = ref('')
const warning = ref('')
const inputRef = ref(null)
// 复用对话会话，保持多轮上下文;每次打开面板重置，避免跨 cell 串味。
let codegenSessionId = ''

const panelStyle = computed(() => {
  const x = Math.max(8, Math.min(Number(props.anchor?.x ?? 0), (typeof window !== 'undefined' ? window.innerWidth : 1200) - 400))
  const y = Math.max(8, Math.min(Number(props.anchor?.y ?? 0), (typeof window !== 'undefined' ? window.innerHeight : 800) - 120))
  return { left: `${x}px`, top: `${y}px` }
})

watch(
  () => props.visible,
  (v) => {
    if (v) {
      instruction.value = ''
      generatedCode.value = ''
      error.value = ''
      warning.value = ''
      codegenSessionId = String(props.sessionId || '')
      nextTick(() => inputRef.value?.focus?.())
    }
  }
)

const close = () => emit('update:visible', false)

// 仅编译 + 检查 import 可用性,绝不执行用户代码(安全):把生成代码经 vars 注入为 _codegen_src。
const VERIFY_SNIPPET = `
import ast, importlib.util
_issues = {"syntax_error": None, "missing": []}
_src = globals().get("_codegen_src", "")
try:
    _tree = ast.parse(_src, mode="exec")
except SyntaxError as _e:
    _issues["syntax_error"] = f"{_e.msg} (line {_e.lineno})"
    _tree = None
if _tree is not None:
    _seen = set()
    for _n in ast.walk(_tree):
        _mods = []
        if isinstance(_n, ast.Import):
            _mods = [a.name.split(".")[0] for a in _n.names]
        elif isinstance(_n, ast.ImportFrom) and (_n.level or 0) == 0 and _n.module:
            _mods = [_n.module.split(".")[0]]
        for _m in _mods:
            if _m and _m not in _seen:
                _seen.add(_m)
                try:
                    if importlib.util.find_spec(_m) is None:
                        _issues["missing"].append(_m)
                except Exception:
                    _issues["missing"].append(_m)
_result = _issues
`

const requestCodegen = async (instructionText) => {
  const res = await invoke('python_codegen_from_prompt', {
    request: {
      modelid: Number(chatStore.selectedModel ?? 5),
      sessionid: codegenSessionId,
      instruction: instructionText,
      context: String(props.context || ''),
    },
  })
  codegenSessionId = String(res?.sessionid || codegenSessionId)
  return String(res?.code || '').trim()
}

const verifyCode = async (code) => {
  try {
    const res = await invoke('python_exec', {
      request: {
        code: VERIFY_SNIPPET,
        persist: false,
        expect_json: true,
        include_session_vars: false,
        vars: { _codegen_src: String(code || '') },
      },
    })
    const rj = res?.result_json || {}
    const missing = Array.isArray(rj.missing) ? rj.missing : []
    return { ok: !rj.syntax_error && missing.length === 0, syntaxError: rj.syntax_error || null, missing }
  } catch (e) {
    // 校验本身失败(worker 未就绪等)不阻断,直接放行生成结果。
    return { ok: true, syntaxError: null, missing: [], skipped: true }
  }
}

// VERIFY_HELPERS_PLACEHOLDER

const describeIssues = (check) => {
  const parts = []
  if (check.syntaxError) parts.push(`语法错误:${check.syntaxError}`)
  if (check.missing?.length) parts.push(`用到了未安装的库:${check.missing.join(', ')}`)
  return parts.join(';')
}

const generate = async () => {
  const text = String(instruction.value || '').trim()
  if (!text || loading.value) return
  loading.value = true
  error.value = ''
  warning.value = ''
  try {
    let code = await requestCodegen(text)
    let check = await verifyCode(code)
    if (!check.ok && code) {
      // 一轮自动修正:把编译/依赖检查发现的问题反馈给模型,重生成一次(会话保留上下文)。
      const feedback = describeIssues(check)
      const fixInstruction = `${text}\n\n上次生成的代码存在问题:${feedback}。请修正(优先改用运行环境已预装的库,或去掉这些依赖),只输出修正后的代码。`
      const fixed = await requestCodegen(fixInstruction)
      if (fixed) {
        code = fixed
        check = await verifyCode(fixed)
      }
    }
    generatedCode.value = String(code || '').trim()
    if (!generatedCode.value) {
      error.value = '模型没有返回可用代码，请调整描述后重试。'
    } else if (!check.ok) {
      warning.value = `注意:${describeIssues(check)}。已保留生成结果,请自行核对后再运行。`
    }
  } catch (e) {
    error.value = '生成失败：' + String(e?.message || e)
  } finally {
    loading.value = false
  }
}


const emitInsert = (mode) => {
  if (!generatedCode.value) return
  emit('insert', { code: generatedCode.value, mode })
  close()
}
</script>

<!-- STYLE_PLACEHOLDER -->

<style scoped>
.guaxi-codegen-panel {
  position: fixed;
  z-index: 3000;
  width: 380px;
  max-width: calc(100vw - 16px);
  background: var(--guaxi-content-bg, #fff);
  color: var(--guaxi-text-color, #1f2329);
  border: 1px solid var(--guaxi-border-color, #e0e0e6);
  border-radius: 8px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.18);
  padding: 10px;
  font-size: 13px;
}
.guaxi-codegen-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}
.guaxi-codegen-panel__title {
  font-weight: 600;
}
.guaxi-codegen-panel__close {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 18px;
  line-height: 1;
  color: inherit;
  opacity: 0.6;
}
.guaxi-codegen-panel__close:hover {
  opacity: 1;
}
.guaxi-codegen-panel__input {
  width: 100%;
  min-height: 64px;
  resize: vertical;
  box-sizing: border-box;
  padding: 6px 8px;
  border: 1px solid var(--guaxi-border-color, #e0e0e6);
  border-radius: 6px;
  background: var(--guaxi-input-bg, #fafafa);
  color: inherit;
  font-family: inherit;
}
.guaxi-codegen-panel__actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  flex-wrap: wrap;
}
.guaxi-codegen-panel__btn {
  padding: 4px 12px;
  border: 1px solid var(--guaxi-border-color, #d0d0d6);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  cursor: pointer;
}
.guaxi-codegen-panel__btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.guaxi-codegen-panel__btn--primary {
  background: var(--guaxi-primary-color, #2f6fed);
  border-color: var(--guaxi-primary-color, #2f6fed);
  color: #fff;
}
.guaxi-codegen-panel__hint {
  opacity: 0.6;
  font-size: 12px;
}
.guaxi-codegen-panel__error {
  margin-top: 8px;
  color: #d03050;
  font-size: 12px;
  word-break: break-all;
}
.guaxi-codegen-panel__warning {
  margin-top: 8px;
  color: #d98a00;
  font-size: 12px;
  word-break: break-all;
}
.guaxi-codegen-panel__result {
  margin-top: 10px;
}
.guaxi-codegen-panel__code {
  max-height: 260px;
  overflow: auto;
  margin: 0;
  padding: 8px;
  border-radius: 6px;
  background: var(--guaxi-code-bg, #f5f5f7);
  font-family: 'Consolas', 'Menlo', monospace;
  font-size: 12px;
  white-space: pre;
}
</style>


