<template>
  <div class="tb-root">
    <n-tabs type="line" size="small" class="tb-tabs" pane-class="tb-pane">
      <n-tab-pane name="calc" tab="计算器">
        <template #tab><span class="tw-tab"><Icon :icon="calcIcon" width="15" height="15" />计算器</span></template>
        <div class="tb-sec">
          <div class="tb-h">进制 / 编码转换</div>
          <n-input v-model:value="numInput" size="small" placeholder="输入 0x1F / 0b1010 / 31 / 0o37" @input="convertNum" style="max-width:320px" />
          <table class="tb-kv" v-if="numOut">
            <tbody>
              <tr><td>HEX</td><td>{{ numOut.hex }}</td></tr>
              <tr><td>DEC</td><td>{{ numOut.dec }}</td></tr>
              <tr><td>OCT</td><td>{{ numOut.oct }}</td></tr>
              <tr><td>BIN</td><td>{{ numOut.bin }}</td></tr>
              <tr><td>ASCII</td><td>{{ numOut.ascii }}</td></tr>
            </tbody>
          </table>
        </div>
        <div class="tb-sec">
          <div class="tb-h">程序员计算器（位运算）</div>
          <div class="tb-row">
            <n-input v-model:value="opA" size="small" placeholder="操作数 A" style="width:150px" />
            <n-select v-model:value="op" size="small" :options="opOptions" style="width:90px" />
            <n-input v-model:value="opB" size="small" placeholder="操作数 B" style="width:150px" />
            <n-button size="small" type="primary" @click="calcOp">=</n-button>
            <span class="tb-res" v-if="opResult">{{ opResult }}</span>
          </div>
        </div>
        <div class="tb-sec">
          <div class="tb-h">时间戳互转</div>
          <div class="tb-row">
            <n-input v-model:value="epochInput" size="small" placeholder="epoch 秒或毫秒" style="width:180px" @input="epochToDate" />
            <span class="tb-res">{{ epochOut }}</span>
          </div>
          <div class="tb-row">
            <n-input v-model:value="dateInput" size="small" placeholder="2026-09-23 12:00:00" style="width:220px" @input="dateToEpoch" />
            <span class="tb-res">{{ dateOut }}</span>
          </div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="bin" tab="二进制分析">
        <template #tab><span class="tw-tab"><Icon :icon="binaryIcon" width="15" height="15" />二进制分析</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-button size="small" type="primary" :loading="binLoading" @click="pickBinary">选择 ELF/PE 文件</n-button>
            <span class="tb-res" v-if="binReport">{{ binReport.format }} · {{ binReport.arch }} · 总熵 {{ binReport.overall_entropy.toFixed(2) }}</span>
            <span v-if="error" class="tb-err">{{ error }}</span>
          </div>
          <table class="tb-table" v-if="binReport && binReport.sections.length">
            <thead><tr><th>段</th><th>大小</th><th>虚拟地址</th><th>熵(0-8)</th></tr></thead>
            <tbody>
              <tr v-for="s in binReport.sections" :key="s.name + s.vaddr">
                <td>{{ s.name }}</td>
                <td>{{ s.size.toLocaleString() }}</td>
                <td>{{ s.vaddr }}</td>
                <td>
                  <div class="tb-ent"><div class="tb-ent-bar" :style="{ width: (s.entropy/8*100)+'%', background: s.entropy>7?'#e5484d':(s.entropy>5?'#f5a524':'#30a46c') }"></div><span>{{ s.entropy.toFixed(2) }}</span></div>
                </td>
              </tr>
            </tbody>
          </table>
          <div class="tb-hint">熵 &gt;7 多为加密/压缩段，&lt;5 多为代码/数据。</div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="core" tab="Coredump">
        <template #tab><span class="tw-tab"><Icon :icon="skullIcon" width="15" height="15" />Coredump</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="SSH 档案" style="width:180px" />
            <n-input v-model:value="coreProgram" size="small" placeholder="可执行文件路径" style="width:200px" />
            <n-input v-model:value="coreFile" size="small" placeholder="core 文件路径" style="width:200px" />
            <n-button size="small" type="primary" :loading="coreLoading" @click="analyzeCore">分析</n-button>
            <n-button size="small" :disabled="!coreOut" @click="sendToEditor(coreOut)">→ 编辑器</n-button>
          </div>
          <pre class="tb-out">{{ coreOut }}</pre>
        </div>
      </n-tab-pane>

      <n-tab-pane name="exec" tab="远程执行">
        <template #tab><span class="tw-tab"><Icon :icon="zapIcon" width="15" height="15" />远程执行</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="SSH 档案" style="width:180px" />
            <n-input v-model:value="execCmd" size="small" placeholder="远程命令，如 dmesg | tail -100" style="flex:1;min-width:240px" @keyup.enter="runExec" />
            <n-button size="small" type="primary" :loading="execLoading" @click="runExec">执行</n-button>
            <n-button size="small" :disabled="!execSaved" @click="importCapture">导入分析</n-button>
          </div>
          <div class="tb-row">
            <span class="tb-h">定时采集</span>
            <n-input-number v-model:value="periodSecs" size="small" :min="1" style="width:110px" />
            <n-button size="small" v-if="!collectorId" @click="startPeriodic">开始</n-button>
            <n-button size="small" v-else @click="stopPeriodic">停止采集</n-button>
            <span class="tb-res" v-if="collectorId">采集中 → {{ collectorPath }}</span>
          </div>
          <pre class="tb-out">{{ execOut }}</pre>
        </div>
      </n-tab-pane>
      <n-tab-pane name="multi" tab="多机对齐">
        <template #tab><span class="tw-tab"><Icon :icon="gitCompareIcon" width="15" height="15" />多机对齐</span></template>
        <div class="tb-sec">
          <div class="tb-row">
            <n-button size="small" type="primary" @click="addMultiLog">添加日志</n-button>
            <n-button size="small" :disabled="!multiLogs.length" @click="clearMultiLogs">清空</n-button>
            <span class="tb-res">{{ multiLogs.length }} 台机器/日志，按时间戳对齐到同一时间线</span>
          </div>
          <VChart v-if="multiChartOption" class="tb-multi-chart" :option="multiChartOption" autoresize />
          <div v-else class="tb-hint">导入多份日志（如 client 与 server），每份的快照会排到同一条时间轴上，方便对因果。</div>
        </div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { ScatterChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, LegendComponent, DataZoomComponent } from 'echarts/components'
import VChart from 'vue-echarts'
import { createLogger } from '../../../utils/logger'
import { Icon } from '@iconify/vue'
import calcIcon from '@iconify-icons/lucide/calculator'
import binaryIcon from '@iconify-icons/lucide/binary'
import skullIcon from '@iconify-icons/lucide/skull'
import zapIcon from '@iconify-icons/lucide/zap'
import gitCompareIcon from '@iconify-icons/lucide/git-compare'

use([CanvasRenderer, ScatterChart, GridComponent, TooltipComponent, LegendComponent, DataZoomComponent])

const emit = defineEmits(['import-file'])
const logger = createLogger('RemoteToolbox')
const error = ref('')

// __TB_BODY__
const profiles = ref([])
const profileOptions = computed(() => profiles.value.map(p => ({ label: `${p.name || p.host}`, value: p.id })))
const profileId = ref(null)

const numInput = ref('')
const numOut = ref(null)
function bigintToAscii(v) {
  if (v < 0n) return '—'
  let bytes = []; let x = v
  while (x > 0n) { bytes.unshift(Number(x & 0xffn)); x >>= 8n }
  if (!bytes.length) bytes = [0]
  return bytes.map(b => (b >= 32 && b < 127) ? String.fromCharCode(b) : '.').join('')
}
function convertNum() {
  const s = numInput.value.trim()
  if (!s) { numOut.value = null; return }
  let v
  try {
    if (/^0x[0-9a-f]+$/i.test(s) || /^0b[01]+$/i.test(s) || /^0o[0-7]+$/i.test(s) || /^-?\d+$/.test(s)) v = BigInt(s)
    else { numOut.value = null; return }
  } catch (_) { numOut.value = null; return }
  numOut.value = {
    hex: '0x' + (v < 0n ? '-' + (-v).toString(16) : v.toString(16)),
    dec: v.toString(10),
    oct: '0o' + (v < 0n ? '-' + (-v).toString(8) : v.toString(8)),
    bin: '0b' + (v < 0n ? '-' + (-v).toString(2) : v.toString(2)),
    ascii: bigintToAscii(v),
  }
}

const opA = ref(''); const opB = ref(''); const op = ref('&'); const opResult = ref('')
const opOptions = [
  { label: 'AND', value: '&' }, { label: 'OR', value: '|' }, { label: 'XOR', value: '^' },
  { label: '<<', value: '<<' }, { label: '>>', value: '>>' },
  { label: '+', value: '+' }, { label: '-', value: '-' }, { label: '*', value: '*' },
]
function pnum(s) { s = (s || '').trim(); try { return BigInt(s) } catch (_) { return null } }
function calcOp() {
  const a = pnum(opA.value), b = pnum(opB.value)
  if (a == null || b == null) { opResult.value = '输入无效'; return }
  let r
  switch (op.value) {
    case '&': r = a & b; break; case '|': r = a | b; break; case '^': r = a ^ b; break
    case '<<': r = a << b; break; case '>>': r = a >> b; break
    case '+': r = a + b; break; case '-': r = a - b; break; case '*': r = a * b; break; default: r = 0n
  }
  const hx = r < 0n ? '-0x' + (-r).toString(16) : '0x' + r.toString(16)
  opResult.value = `${r.toString(10)}  =  ${hx}`
}

const epochInput = ref(''); const epochOut = ref(''); const dateInput = ref(''); const dateOut = ref('')
function epochToDate() {
  const s = epochInput.value.trim(); if (!s) { epochOut.value = ''; return }
  const n = Number(s); if (!isFinite(n)) { epochOut.value = '无效'; return }
  const ms = s.replace('-', '').length > 10 ? n : n * 1000
  const d = new Date(ms); epochOut.value = isNaN(d.getTime()) ? '无效' : d.toLocaleString() + '  /  ' + d.toISOString()
}
function dateToEpoch() {
  const s = dateInput.value.trim(); if (!s) { dateOut.value = ''; return }
  const d = new Date(s.replace(' ', 'T')); if (isNaN(d.getTime())) { dateOut.value = '无效'; return }
  dateOut.value = `epoch ${Math.floor(d.getTime() / 1000)} s  /  ${d.getTime()} ms`
}
// __TB_BODY2__
const binReport = ref(null); const binLoading = ref(false)
async function pickBinary() {
  error.value = ''
  const sel = await open({ multiple: false, directory: false }); if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  binLoading.value = true
  try { binReport.value = await invoke('binary_analyze', { path }) }
  catch (e) { error.value = '分析失败: ' + (e?.message || e) } finally { binLoading.value = false }
}

const coreProgram = ref(''); const coreFile = ref(''); const coreOut = ref(''); const coreLoading = ref(false)
async function analyzeCore() {
  if (!profileId.value || !coreProgram.value || !coreFile.value) { error.value = '需要档案 + 程序 + core'; return }
  coreLoading.value = true; error.value = ''
  try { const r = await invoke('gdb_coredump_analyze', { profileId: profileId.value, program: coreProgram.value, core: coreFile.value }); coreOut.value = r.output }
  catch (e) { error.value = '分析失败: ' + (e?.message || e) } finally { coreLoading.value = false }
}

const execCmd = ref(''); const execOut = ref(''); const execLoading = ref(false); const execSaved = ref('')
async function runExec() {
  if (!profileId.value || !execCmd.value) { error.value = '需要档案 + 命令'; return }
  execLoading.value = true; error.value = ''
  try { const r = await invoke('ssh_exec_capture', { profileId: profileId.value, command: execCmd.value }); execOut.value = r.output; execSaved.value = r.saved_path }
  catch (e) { error.value = '执行失败: ' + (e?.message || e) } finally { execLoading.value = false }
}
function importCapture() { if (execSaved.value) emit('import-file', execSaved.value) }

const periodSecs = ref(10); const collectorId = ref(''); const collectorPath = ref('')
async function startPeriodic() {
  if (!profileId.value || !execCmd.value) { error.value = '需要档案 + 命令'; return }
  try {
    const r = await invoke('ssh_start_periodic', { profileId: profileId.value, command: execCmd.value, intervalSecs: periodSecs.value, label: 'periodic' })
    collectorId.value = r.id; collectorPath.value = r.path
  } catch (e) { error.value = '启动采集失败: ' + (e?.message || e) }
}
async function stopPeriodic() {
  if (collectorId.value) {
    await invoke('ssh_stop_periodic', { id: collectorId.value }).catch(() => {})
    const p = collectorPath.value; collectorId.value = ''
    if (p) emit('import-file', p)
  }
}

async function sendToEditor(content) {
  if (!content) return
  try { await invoke('emit_template_insert', { content }) }
  catch (e) { error.value = '发送失败: ' + (e?.message || e) }
}

onMounted(async () => { try { profiles.value = await invoke('ssh_list_profiles') } catch (e) { logger.error('profiles', e) } })

// J：多机时间轴对齐——导入多份日志，把各自快照按时间戳排到同一条时间线。
const multiLogs = ref([]) // { name, sessionId, points: [[ts, laneIndex], ...] }
async function addMultiLog() {
  error.value = ''
  const sel = await open({ multiple: false, directory: false, title: '选择日志文件' })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    const res = await invoke('log_import_file', { path })
    const lane = multiLogs.value.length
    const name = path.replace(/\\/g, '/').split('/').pop() || path
    const points = (res.frames || [])
      .filter(f => f.timestamp_ms != null)
      .map(f => [f.timestamp_ms, lane])
    multiLogs.value.push({ name, sessionId: res.session_id, points })
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
function clearMultiLogs() { multiLogs.value = [] }
const multiChartOption = computed(() => {
  if (!multiLogs.value.length) return null
  return {
    animation: false,
    grid: { left: 140, right: 20, top: 20, bottom: 50 },
    tooltip: { trigger: 'item', formatter: p => new Date(p.value[0]).toLocaleString() },
    legend: { show: false },
    xAxis: { type: 'time', name: '时间' },
    yAxis: {
      type: 'category',
      data: multiLogs.value.map(m => m.name),
    },
    dataZoom: [{ type: 'inside' }, { type: 'slider', height: 18, bottom: 12 }],
    series: multiLogs.value.map((m, i) => ({
      name: m.name, type: 'scatter', symbolSize: 6,
      data: m.points.map(pt => [pt[0], i]),
    })),
  }
})
</script>

<style scoped>
.tb-root { height: 100%; display: flex; flex-direction: column; min-height: 0; }
.tb-tabs { height: 100%; display: flex; flex-direction: column; }
.tb-tabs :deep(.tb-pane) { height: 100%; overflow: auto; padding: 12px; }
.tb-sec { margin-bottom: 18px; }
.tb-h { font-size: 12px; font-weight: 600; margin-bottom: 8px; }
.tb-row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-bottom: 8px; }
.tb-res { font-size: 12px; font-family: var(--guaxi-mono-font, monospace); color: var(--guaxi-text-secondary, #6b7280); }
.tb-err { font-size: 12px; color: #e5484d; }
.tb-kv { border-collapse: collapse; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; margin-top: 8px; }
.tb-kv td { padding: 3px 12px 3px 0; }
.tb-kv td:first-child { color: var(--guaxi-text-secondary, #9ca3af); }
.tb-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.tb-table th, .tb-table td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); }
.tb-ent { position: relative; background: var(--guaxi-border-weak, rgba(0,0,0,0.08)); border-radius: 3px; height: 16px; min-width: 120px; }
.tb-ent-bar { height: 100%; border-radius: 3px; }
.tb-ent span { position: absolute; left: 6px; top: 0; font-size: 11px; line-height: 16px; }
.tb-out { background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; padding: 8px; border-radius: 6px; white-space: pre-wrap; word-break: break-all; min-height: 120px; max-height: 50vh; overflow: auto; }
.tb-hint { font-size: 11px; color: var(--guaxi-text-secondary, #9ca3af); margin-top: 6px; }
.tb-multi-chart { width: 100%; height: 360px; }

/* ---- 美化层 ---- */
.tw-tab { display: inline-flex; align-items: center; gap: 7px; font-weight: 500; }
.tb-root { font-family: var(--tw-ui-font); }
.tb-tabs :deep(.n-tabs-tab) { font-family: var(--tw-ui-font); font-weight: 500; }
.tb-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.tb-tabs :deep(.n-tabs-bar) { background: var(--tw-accent) !important; height: 2.5px; border-radius: 2px; }
.tb-h { color: var(--tw-accent); letter-spacing: .02em; display: inline-flex; align-items: center; gap: 6px; }
.tb-sec { padding: 14px 16px; margin-bottom: 8px; border: 1px solid color-mix(in srgb, var(--guaxi-border) 60%, transparent); border-radius: var(--tw-radius); background: color-mix(in srgb, var(--guaxi-panel-bg) 40%, transparent); }
.tb-kv { font-family: var(--tw-mono-font); }
.tb-kv td:first-child { color: var(--tw-accent); font-weight: 600; }
.tb-res { font-family: var(--tw-mono-font); }
.tb-out { background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); border-radius: var(--tw-radius-sm); line-height: 1.55; }
.tb-table th { text-transform: uppercase; letter-spacing: .04em; font-size: 11px; color: var(--guaxi-text-secondary); }
</style>
