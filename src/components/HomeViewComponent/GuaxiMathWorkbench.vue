<template>
  <main class="math-workbench" :data-mwb-theme="context.theme">
    <header class="math-workbench__header">
      <div class="math-workbench__title-block">
        <h1 class="math-workbench__title">数学工作台</h1>
        <span class="math-workbench__subtitle">{{ carrier?.label || context.command || '—' }}</span>
      </div>
      <div class="math-workbench__actions">
        <button type="button" class="math-workbench__btn" :disabled="loading" @click="reload">刷新</button>
        <button type="button" class="math-workbench__btn primary" :disabled="running || !carrier" @click="rerun">
          {{ running ? '运行中…' : '按参数重新运行' }}
        </button>
      </div>
    </header>

    <section class="math-workbench__body">
      <aside class="math-workbench__sidebar">
        <div class="math-workbench__section-title">参数</div>
        <div v-if="!carrier" class="math-workbench__muted">{{ errorText || '正在载入工作台…' }}</div>
        <template v-else>
          <div v-for="field in fields" :key="field.key" class="math-workbench__field">
            <label :for="`fld-${field.key}`">{{ field.label }}</label>
            <input
              :id="`fld-${field.key}`"
              v-model="form[field.key]"
              :type="field.type === 'text' || field.type === 'list' ? 'text' : 'number'"
              :step="field.step || 'any'"
              class="math-workbench__input"
            />
          </div>
          <div v-if="!fields.length" class="math-workbench__muted">该工作台暂无可调参数。</div>

          <div class="math-workbench__source">
            <div class="math-workbench__section-title">当前命令</div>
            <code>{{ previewSource }}</code>
          </div>

          <template v-if="carrier.kind === 'simulation' && carrier.solutionVar">
            <div class="math-workbench__section-title">仿真回放（轻量）</div>
            <button type="button" class="math-workbench__btn" :disabled="loadingSolution" @click="loadSolution">
              {{ solution ? '重新载入解' : '载入解数组' }}
            </button>
            <template v-if="solution">
              <input
                v-model.number="frame"
                type="range"
                min="0"
                :max="frameMax"
                class="math-workbench__slider"
              />
              <div class="math-workbench__frame-readout">
                <div>t = {{ frameTimeLabel }}</div>
                <div v-for="(name, i) in solution.state" :key="name">
                  {{ name }} = {{ frameValueLabel(i) }}
                </div>
              </div>
            </template>
          </template>
        </template>
      </aside>

      <section class="math-workbench__result">
        <div v-if="errorText" class="math-workbench__error">{{ errorText }}</div>
        <div v-if="!displays.length && !errorText" class="math-workbench__muted math-workbench__placeholder">
          调整左侧参数后点「按参数重新运行」，结果（数值 / 收敛过程 / 交互图）会显示在这里。
        </div>
        <div
          v-for="(disp, idx) in displays"
          :key="idx"
          class="math-workbench__display"
        >
          <iframe
            v-if="disp.type === 'html'"
            class="math-workbench__iframe"
            sandbox="allow-scripts allow-same-origin"
            :srcdoc="disp.html"
          ></iframe>
          <img
            v-else-if="disp.type === 'image'"
            class="math-workbench__image"
            :src="`data:${disp.mime || 'image/png'};base64,${disp.data_base64}`"
            alt="result"
          />
          <pre v-else class="math-workbench__markdown">{{ cleanMarkdown(disp.markdown) }}</pre>
        </div>
      </section>
    </section>
  </main>
</template>

<script setup>
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useMessage } from 'naive-ui'
import { createLogger } from '../../utils/logger'
import {
  parseMathWorkbenchRouteQuery,
  fetchMathCarrier,
  fetchMathSolution,
  buildRerunSource,
  rerunMathCarrier,
} from './utils/guaxiMathWorkbenchClient'

const logger = createLogger('GuaxiMathWorkbench')
const message = useMessage()
const route = useRoute()
const context = reactive(parseMathWorkbenchRouteQuery(route.query || {}))

const carrier = ref(null)
const form = reactive({})
const displays = ref([])
const errorText = ref('')
const loading = ref(false)
const running = ref(false)
const solution = ref(null)
const loadingSolution = ref(false)
const frame = ref(0)

// 每种命令暴露的可调参数（轻量、按需扩展：新命令在此加一条 schema 即可）。
const FIELD_SCHEMAS = {
  nsolve: [
    { key: 'x0', label: '初值 x0', from: (p) => p.x0 },
    { key: 'tol', label: '容差 tol', from: (p) => p.tol },
    { key: 'maxiter', label: '最大迭代', from: (p) => p.maxiter },
  ],
  minimize: [
    { key: 'x0', label: '初值 x0（逗号分隔）', type: 'list', from: (p) => (p.x0 || []).join(', ') },
    { key: 'tol', label: '容差 tol', from: (p) => p.tol },
    { key: 'maxiter', label: '最大迭代', from: (p) => p.maxiter },
    { key: 'method', label: '方法', type: 'text', from: (p) => p.method || 'BFGS' },
  ],
  odeint: [
    { key: 't0', label: '起始 t', from: (p) => p.trange?.[1] },
    { key: 't1', label: '终止 t', from: (p) => p.trange?.[2] },
    { key: 'n', label: '采样点 n', from: (p) => p.n },
    { key: 'y0', label: '初值 y0（逗号分隔）', type: 'list', from: (p) => (p.y0 || []).join(', ') },
  ],
  phase_portrait: [
    { key: 'density', label: '密度', from: (p) => p.density },
  ],
}

const fields = computed(() => FIELD_SCHEMAS[String(carrier.value?.command || '')] || [])

const parseList = (text) => String(text || '')
  .split(',')
  .map((s) => Number(String(s).trim()))
  .filter((n) => Number.isFinite(n))

// form → buildRerunSource 的 overrides（按命令拼回参数结构）。
const buildOverrides = () => {
  const cmd = String(carrier.value?.command || '')
  const p = carrier.value?.params || {}
  if (cmd === 'nsolve') {
    return { x0: Number(form.x0), tol: Number(form.tol), maxiter: Number(form.maxiter) }
  }
  if (cmd === 'minimize') {
    return { x0: parseList(form.x0), tol: Number(form.tol), maxiter: Number(form.maxiter), method: String(form.method || 'BFGS') }
  }
  if (cmd === 'odeint') {
    const t = p.trange || ['t', 0, 1]
    return { trange: [t[0], Number(form.t0), Number(form.t1)], n: Number(form.n), y0: parseList(form.y0) }
  }
  if (cmd === 'phase_portrait') {
    return { density: Number(form.density) }
  }
  return {}
}

const previewSource = computed(() => {
  if (!carrier.value) return ''
  try {
    return buildRerunSource(carrier.value, buildOverrides())
  } catch (error) {
    return String(carrier.value?.command || '')
  }
})

const cleanMarkdown = (md) => String(md || '').replace(/\$\$/g, '').trim()

const resetForm = () => {
  Object.keys(form).forEach((k) => delete form[k])
  const p = carrier.value?.params || {}
  for (const field of fields.value) {
    form[field.key] = field.from ? field.from(p) : ''
  }
}

const reload = async () => {
  if (!context.sessionId || !context.carrierKey) {
    errorText.value = '缺少会话或工作台标识。'
    return
  }
  loading.value = true
  errorText.value = ''
  try {
    const data = await fetchMathCarrier(context.sessionId, context.carrierKey, context)
    if (!data) {
      errorText.value = '未找到该工作台（会话可能已重置）。'
      carrier.value = null
      return
    }
    carrier.value = data
    resetForm()
    logger.info('[MathWorkbench] loaded carrier', { carrierKey: context.carrierKey, command: data.command })
  } catch (error) {
    errorText.value = String(error?.message || error)
    logger.error('[MathWorkbench] load carrier failed', { error: String(error?.message || error) })
  } finally {
    loading.value = false
  }
}

const rerun = async () => {
  if (!carrier.value) return
  running.value = true
  errorText.value = ''
  try {
    const { displays: out, source } = await rerunMathCarrier(
      context.sessionId, carrier.value, buildOverrides(), context,
    )
    displays.value = out
    logger.info('[MathWorkbench] rerun done', { source, displayCount: out.length })
    if (!out.length) message.warning('本次运行没有可显示的结果')
  } catch (error) {
    errorText.value = String(error?.message || error)
    message.error(`运行失败：${String(error?.message || error)}`)
  } finally {
    running.value = false
  }
}

const loadSolution = async () => {
  const solVar = String(carrier.value?.solutionVar || '')
  if (!solVar) return
  loadingSolution.value = true
  try {
    const sol = await fetchMathSolution(context.sessionId, solVar, context)
    solution.value = sol && Array.isArray(sol.t) ? sol : null
    frame.value = 0
    if (!solution.value) message.warning('解数组不可用（会话可能已重置）')
  } catch (error) {
    message.error(`载入解失败：${String(error?.message || error)}`)
  } finally {
    loadingSolution.value = false
  }
}

const frameMax = computed(() => Math.max(0, (solution.value?.t?.length || 1) - 1))
const frameTimeLabel = computed(() => {
  const t = solution.value?.t?.[frame.value]
  return Number.isFinite(t) ? Number(t).toFixed(4) : '—'
})
const frameValueLabel = (i) => {
  const v = solution.value?.y?.[i]?.[frame.value]
  return Number.isFinite(v) ? Number(v).toFixed(4) : '—'
}

onMounted(reload)
</script>

<style scoped>
.math-workbench {
  --mwb-bg: #f6f8fa;
  --mwb-panel: #ffffff;
  --mwb-text: #1f2328;
  --mwb-muted: #57606a;
  --mwb-border: #d0d7de;
  --mwb-accent: #0891b2;
  position: fixed;
  inset: 0;
  display: flex;
  flex-direction: column;
  background: var(--mwb-bg);
  color: var(--mwb-text);
  font-family: var(--guaxi-notebook-font, system-ui, sans-serif);
}

.math-workbench[data-mwb-theme='dark'] {
  --mwb-bg: #1b1f27;
  --mwb-panel: #242a33;
  --mwb-text: #e8edf6;
  --mwb-muted: #aeb8ca;
  --mwb-border: #39414d;
}

.math-workbench__header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 20px;
  border-bottom: 1px solid var(--mwb-border);
  background: var(--mwb-panel);
}

.math-workbench__title {
  margin: 0;
  font-size: 18px;
}

.math-workbench__subtitle {
  margin-left: 12px;
  font-size: 13px;
  color: var(--mwb-muted);
}

.math-workbench__actions {
  display: flex;
  gap: 8px;
}

.math-workbench__btn {
  padding: 7px 14px;
  border: 1px solid var(--mwb-border);
  border-radius: 8px;
  background: transparent;
  color: var(--mwb-text);
  cursor: pointer;
  font-size: 13px;
}

.math-workbench__btn:hover:not(:disabled) {
  border-color: var(--mwb-accent);
  color: var(--mwb-accent);
}

.math-workbench__btn.primary {
  background: var(--mwb-accent);
  border-color: var(--mwb-accent);
  color: #fff;
}

.math-workbench__btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.math-workbench__body {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 300px minmax(0, 1fr);
}

.math-workbench__sidebar {
  border-right: 1px solid var(--mwb-border);
  background: var(--mwb-panel);
  padding: 16px;
  overflow: auto;
}

.math-workbench__section-title {
  margin: 16px 0 8px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.02em;
  color: var(--mwb-muted);
}
.math-workbench__section-title:first-child {
  margin-top: 0;
}

.math-workbench__field {
  display: grid;
  gap: 4px;
  margin-bottom: 10px;
}

.math-workbench__field label {
  font-size: 12px;
  color: var(--mwb-muted);
}

.math-workbench__input {
  width: 100%;
  box-sizing: border-box;
  padding: 6px 8px;
  border: 1px solid var(--mwb-border);
  border-radius: 6px;
  background: var(--mwb-bg);
  color: var(--mwb-text);
  font: inherit;
}

.math-workbench__source code {
  display: block;
  padding: 8px;
  border-radius: 6px;
  background: var(--mwb-bg);
  border: 1px solid var(--mwb-border);
  font-family: var(--guaxi-notebook-mono-font, monospace);
  font-size: 12px;
  word-break: break-all;
}

.math-workbench__slider {
  width: 100%;
  margin: 8px 0;
}

.math-workbench__frame-readout {
  font-family: var(--guaxi-notebook-mono-font, monospace);
  font-size: 12px;
  line-height: 1.6;
}

.math-workbench__result {
  min-width: 0;
  overflow: auto;
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.math-workbench__muted {
  color: var(--mwb-muted);
  font-size: 13px;
  line-height: 1.7;
}

.math-workbench__placeholder {
  margin-top: 24px;
}

.math-workbench__error {
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, #d92d20 40%, var(--mwb-border));
  background: color-mix(in srgb, #d92d20 8%, transparent);
  color: #b42318;
  font-size: 13px;
}

.math-workbench__display {
  border: 1px solid var(--mwb-border);
  border-radius: 10px;
  background: var(--mwb-panel);
  overflow: hidden;
}

.math-workbench__iframe {
  width: 100%;
  height: 480px;
  border: 0;
  background: #fff;
}

.math-workbench__image {
  display: block;
  max-width: 100%;
}

.math-workbench__markdown {
  margin: 0;
  padding: 14px 16px;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--guaxi-notebook-mono-font, monospace);
  font-size: 13px;
  line-height: 1.6;
  color: var(--mwb-text);
}
</style>
