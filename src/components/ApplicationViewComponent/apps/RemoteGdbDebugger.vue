<template>
  <div class="gdb-root">
    <div class="gdb-bar">
      <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="选择 SSH 档案" style="width:180px" />
      <n-radio-group v-model:value="mode" size="small" :disabled="!!sessionId">
        <n-radio-button value="launch">启动程序</n-radio-button>
        <n-radio-button value="attach">附加进程</n-radio-button>
      </n-radio-group>
      <n-input v-model:value="program" size="small" :placeholder="mode === 'attach' ? '可执行文件(可空，仅符号)' : '可执行文件路径(可空=仅连gdb)'" style="width:230px" />
      <n-input-number v-if="mode === 'attach'" v-model:value="attachPid" size="small" :min="1" :show-button="false" placeholder="PID" style="width:110px" />
      <n-input v-else v-model:value="argsText" size="small" placeholder="参数(空格分隔)" style="width:150px" />
      <n-button size="small" type="primary" :loading="starting" :disabled="!!sessionId" @click="start">启动 gdb</n-button>
      <n-button size="small" v-if="sessionId" @click="stop">结束</n-button>
      <span class="gdb-status">{{ statusText }}</span>
      <span v-if="error" class="gdb-error">{{ error }}</span>
    </div>

    <div class="gdb-palette" v-if="sessionId">
      <n-button size="tiny" v-for="c in palette" :key="c.label" @click="c.fn ? c.fn() : runCmd(c.cmd)">
        <template #icon><Icon :icon="c.icon" width="13" height="13" /></template>{{ c.label }}
      </n-button>
      <div class="gdb-fontctl" title="控制台字号">
        <button class="gdb-fbtn" @click="setGdbFontSize(gdbFontSize - 1)"><Icon :icon="minusIcon" width="13" height="13" /></button>
        <span class="gdb-fsz">{{ gdbFontSize }}</span>
        <button class="gdb-fbtn" @click="setGdbFontSize(gdbFontSize + 1)"><Icon :icon="plusIcon" width="13" height="13" /></button>
      </div>
    </div>

    <div class="gdb-body" v-if="sessionId">
      <div class="gdb-side" :style="{ width: gdbSideWidth + 'px' }">
        <n-collapse :default-expanded-names="['breaks','callstack','locals','vars','watch']" arrow-placement="right">
          <n-collapse-item title="断点" name="breaks">
            <div class="gdb-actions">
              <n-input v-model:value="bpInput" size="small" placeholder="main / file.c:42 / func" style="flex:1" @keyup.enter="addBreak" />
              <n-button size="tiny" type="primary" @click="addBreak">加</n-button>
              <n-button size="tiny" @click="refreshBreaks">刷新</n-button>
            </div>
            <table class="gdb-table">
              <thead><tr><th>#</th><th>位置</th><th>命中</th><th></th></tr></thead>
              <tbody>
                <tr v-for="b in breaks" :key="b.number">
                  <td>{{ b.number }}</td>
                  <td>{{ b.where }}</td>
                  <td>{{ b.times }}</td>
                  <td><n-button size="tiny" text @click="delBreak(b.number)">删</n-button></td>
                </tr>
                <tr v-if="!breaks.length"><td colspan="4" class="gdb-hint">加断点或点刷新。</td></tr>
              </tbody>
            </table>
          </n-collapse-item>

          <n-collapse-item title="线程" name="threads">
            <div class="gdb-actions"><n-button size="tiny" @click="refreshThreads">刷新线程</n-button></div>
            <div class="gdb-threads">
              <div v-for="t in threads" :key="t.id" class="gdb-thread" :class="{ active: t.id === currentThread }" @click="selectThread(t.id)">
                <div class="gdb-thread-h">线程 {{ t.id }} <span v-if="t.name">({{ t.name }})</span> — {{ t.state }}</div>
                <div class="gdb-frame" v-if="t.frame">#{{ t.frame.level }} {{ t.frame.func }} @ {{ t.frame.file }}:{{ t.frame.line }}</div>
              </div>
              <div v-if="!threads.length" class="gdb-hint">点“刷新线程”，或在停下时自动加载。</div>
            </div>
          </n-collapse-item>

          <n-collapse-item title="调用栈" name="callstack">
            <div class="gdb-actions"><n-button size="tiny" @click="refreshStack">刷新调用栈</n-button></div>
            <div class="gdb-stack">
              <div
                v-for="f in callStack"
                :key="f.level"
                class="gdb-sframe"
                :class="{ active: String(f.level) === String(curFrame) }"
                @click="selectFrame(f.level)"
                :title="f.fullname || f.file"
              >
                <span class="gdb-sframe-lv">#{{ f.level }}</span>
                <span class="gdb-sframe-func">{{ f.func }}</span>
                <span class="gdb-sframe-loc">{{ shortFile(f.file) }}<template v-if="f.line">:{{ f.line }}</template></span>
              </div>
              <div v-if="!callStack.length" class="gdb-hint">停在断点时自动加载，或点“刷新调用栈”。</div>
            </div>
          </n-collapse-item>

          <n-collapse-item title="局部变量" name="locals">
            <div class="gdb-actions"><n-button size="tiny" @click="refreshLocals">刷新</n-button><span class="gdb-hint">当前帧，停下自动刷新</span></div>
            <table class="gdb-table">
              <tbody>
                <tr v-for="l in locals" :key="l.name">
                  <td class="gdb-lname">{{ l.name }}</td>
                  <td class="gdb-val">{{ l.value }}</td>
                </tr>
                <tr v-if="!locals.length"><td colspan="2" class="gdb-hint">停在断点时显示当前帧局部变量。</td></tr>
              </tbody>
            </table>
          </n-collapse-item>

          <n-collapse-item title="变量树" name="vars">
            <div class="gdb-actions">
              <n-input v-model:value="exprInput" size="small" placeholder="*ptr / mystruct / arr" style="flex:1" @keyup.enter="addVar" />
              <n-button size="tiny" type="primary" @click="addVar">展开</n-button>
              <n-button size="tiny" :disabled="!varRoots.length" @click="sendVarTree">→编辑器</n-button>
            </div>
            <div class="gdb-actions">
              <span class="gdb-hint">显示格式</span>
              <n-select v-model:value="varFormat" size="tiny" :options="varFormatOptions" style="width:120px" @update:value="applyVarFormat" />
            </div>
            <div class="gdb-tree">
              <GdbVarNode v-for="n in varRoots" :key="n.name" :node="n" :depth="0" @expand="expandVar" />
              <div v-if="!varRoots.length" class="gdb-hint">输入表达式，把复杂结构体展开成可折叠树。</div>
            </div>
          </n-collapse-item>

          <n-collapse-item title="Watch" name="watch">
            <div class="gdb-actions">
              <n-input v-model:value="watchInput" size="small" placeholder="钉住表达式" style="flex:1" @keyup.enter="addWatch" />
              <n-button size="tiny" type="primary" @click="addWatch">加</n-button>
              <n-button size="tiny" @click="refreshWatch">刷新</n-button>
            </div>
            <table class="gdb-table">
              <tbody>
                <tr v-for="w in watches" :key="w.expr" :class="{ changed: w.changed }">
                  <td>{{ w.expr }}</td>
                  <td class="gdb-val">{{ w.value }}</td>
                  <td><n-button size="tiny" text @click="removeWatch(w.expr)">移除</n-button></td>
                </tr>
                <tr v-if="!watches.length"><td colspan="3" class="gdb-hint">停在断点时自动刷新并高亮变化。</td></tr>
              </tbody>
            </table>
          </n-collapse-item>

          <n-collapse-item title="寄存器" name="regs">
            <div class="gdb-actions"><n-button size="tiny" @click="refreshRegisters">刷新寄存器</n-button></div>
            <table class="gdb-table gdb-regs">
              <thead><tr><th>寄存器</th><th>HEX</th><th>十进制</th></tr></thead>
              <tbody>
                <tr v-for="r in registers" :key="r.name">
                  <td class="gdb-rname">{{ r.name }}</td>
                  <td class="gdb-val">{{ r.hex }}</td>
                  <td class="gdb-val">{{ r.dec }}</td>
                </tr>
                <tr v-if="!registers.length"><td colspan="3" class="gdb-hint">停下后点“刷新寄存器”。</td></tr>
              </tbody>
            </table>
          </n-collapse-item>

          <n-collapse-item title="内存" name="mem">
            <div class="gdb-actions">
              <n-input v-model:value="memAddr" size="small" placeholder="&buf 或 0x..." style="flex:1" @keyup.enter="readMem" />
              <n-input-number v-model:value="memCount" size="small" :min="1" :max="4096" :show-button="false" style="width:80px" />
              <n-button size="tiny" type="primary" @click="readMem">读内存</n-button>
            </div>
            <pre class="gdb-mem">{{ memText || '按十六进制 + ASCII 展示内存块。' }}</pre>
          </n-collapse-item>

          <n-collapse-item title="宏" name="macros">
            <div class="gdb-actions">
              <n-input v-model:value="macroName" size="small" placeholder="宏名" style="width:90px" />
              <n-input v-model:value="macroCmds" size="small" placeholder="命令;命令" style="flex:1" />
              <n-button size="tiny" type="primary" @click="addMacro">存</n-button>
            </div>
            <div class="gdb-actions">
              <n-button size="tiny" @click="saveMacros">导出</n-button>
              <n-button size="tiny" @click="loadMacros">导入</n-button>
            </div>
            <table class="gdb-table">
              <tbody>
                <tr v-for="m in macros" :key="m.name">
                  <td>{{ m.name }}</td>
                  <td><n-button size="tiny" type="primary" @click="runMacro(m)">运行</n-button> <n-button size="tiny" text @click="removeMacro(m.name)">删</n-button></td>
                </tr>
                <tr v-if="!macros.length"><td colspan="2" class="gdb-hint">把常用命令序列存成一键宏。</td></tr>
              </tbody>
            </table>
          </n-collapse-item>
        </n-collapse>
      </div>

      <div class="gdb-sideresize" @mousedown="onGdbResizeDown"></div>

      <div class="gdb-center">
        <div class="gdb-source" :style="{ height: sourceHeight + 'px' }">
          <div class="gdb-source-bar">
            <n-radio-group v-model:value="sourceMode" size="tiny">
              <n-radio-button value="local">本地源码</n-radio-button>
              <n-radio-button value="remote">远端源码</n-radio-button>
            </n-radio-group>
            <n-input v-model:value="srcFromPrefix" size="tiny" placeholder="编译路径前缀 (可空)" style="width:150px" />
            <span class="gdb-arrow">→</span>
            <n-input v-model:value="srcToPrefix" size="tiny" :placeholder="sourceMode === 'local' ? '本地源码根' : '远端路径前缀(可空)'" style="width:160px" />
            <n-button size="tiny" @click="pickLocalRoot" v-if="sourceMode === 'local'">选目录</n-button>
            <n-button size="tiny" type="primary" @click="refreshSource">对照当前行</n-button>
            <span class="gdb-src-loc" :title="sourceFile">{{ shortFile(sourceFile) }}<template v-if="sourceCurLine">:{{ sourceCurLine }}</template></span>
          </div>
          <div class="gdb-source-body" ref="sourceBodyRef" :style="{ fontSize: gdbFontSize + 'px' }">
            <div
              v-for="ln in sourceLines"
              :key="ln.n"
              class="gdb-srcline"
              :class="{ cur: ln.n === sourceCurLine }"
              :ref="el => { if (ln.n === sourceCurLine) curLineEl = el }"
            >
              <span class="gdb-srcno">{{ ln.n }}</span>
              <span class="gdb-srctext">{{ ln.text }}</span>
            </div>
            <div v-if="!sourceLines.length" class="gdb-hint gdb-src-empty">{{ sourceHint }}</div>
          </div>
        </div>
        <div class="gdb-vresize" @mousedown="onSourceResizeDown"></div>
        <div class="gdb-console" ref="consoleRef" :style="{ fontSize: gdbFontSize + 'px' }">
          <pre v-for="(l, i) in consoleLines" :key="i" :class="'gdb-l-' + l.kind">{{ l.text }}</pre>
        </div>
        <div class="gdb-input">
          <n-input v-model:value="cmdInput" size="small" placeholder="输入 MI(-exec-next) 或 CLI(bt) 命令回车执行" @keyup.enter="submitCmd" />
          <n-button size="small" @click="submitCmd">执行</n-button>
        </div>
      </div>
    </div>
    <div v-else class="gdb-empty">选择 SSH 档案与可执行文件，启动远端 gdb -i=mi 开始调试。</div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import { createLogger } from '../../../utils/logger'
import { Icon } from '@iconify/vue'
import playIcon from '@iconify-icons/lucide/play'
import fastForwardIcon from '@iconify-icons/lucide/fast-forward'
import skipForwardIcon from '@iconify-icons/lucide/skip-forward'
import arrowDownIcon from '@iconify-icons/lucide/corner-down-right'
import arrowUpIcon from '@iconify-icons/lucide/corner-up-right'
import pauseIcon from '@iconify-icons/lucide/pause'
import layersIcon from '@iconify-icons/lucide/layers'
import variableIcon from '@iconify-icons/lucide/variable'
import cpuIcon from '@iconify-icons/lucide/cpu'
import minusIcon from '@iconify-icons/lucide/minus'
import plusIcon from '@iconify-icons/lucide/plus'

const logger = createLogger('RemoteGdbDebugger')

import GdbVarNode from './GdbVarNode.vue'

const profiles = ref([])
const profileOptions = computed(() =>
  profiles.value.map(p => ({ label: `${p.name || p.host} (${p.user}@${p.host})`, value: p.id })))
const profileId = ref(null)
const mode = ref('launch')       // 'launch' | 'attach'
const attachPid = ref(null)
const program = ref('')
const argsText = ref('')
const starting = ref(false)
const error = ref('')
const sessionId = ref('')
const statusText = ref('未连接')
const tab = ref('console')

const consoleLines = ref([])
const consoleRef = ref(null)
const cmdInput = ref('')
const exprInput = ref('')
const varRoots = ref([])
const varFormat = ref('natural')
const varFormatOptions = [
  { label: '自然', value: 'natural' },
  { label: '十六进制', value: 'hexadecimal' },
  { label: '十进制', value: 'decimal' },
  { label: '二进制', value: 'binary' },
]
const watchInput = ref('')
const watches = ref([])
const threads = ref([])
const currentThread = ref(null)
const bpInput = ref('')
const breaks = ref([])
const memAddr = ref('')
const memCount = ref(64)
const memText = ref('')
const macroName = ref('')
const macroCmds = ref('')
const macros = ref([])

// 调用栈 / 局部变量 / 寄存器（结构化面板）
const callStack = ref([])
const curFrame = ref(0)
const locals = ref([])
const registers = ref([])

// 跨机源码对照
const sourceMode = ref('local')  // 'local' | 'remote'
const srcFromPrefix = ref('')
const srcToPrefix = ref('')
const sourceLines = ref([])
const sourceCurLine = ref(0)
const sourceFile = ref('')
const sourceHint = ref('停在断点时自动对照当前行；也可点“对照当前行”。')
const sourceBodyRef = ref(null)
let curLineEl = null
const sourceHeight = ref(220)

const palette = [
  { label: '运行', cmd: '-exec-run', icon: playIcon },
  { label: '继续', cmd: '-exec-continue', icon: fastForwardIcon },
  { label: '下一步', cmd: '-exec-next', icon: skipForwardIcon },
  { label: '步入', cmd: '-exec-step', icon: arrowDownIcon },
  { label: '步出', cmd: '-exec-finish', icon: arrowUpIcon },
  { label: '中断', cmd: '-exec-interrupt', icon: pauseIcon },
  { label: '调用栈', fn: () => refreshStack(), icon: layersIcon },
  { label: '局部变量', fn: () => refreshLocals(), icon: variableIcon },
  { label: '寄存器', fn: () => refreshRegisters(), icon: cpuIcon },
]
const gdbFontSize = ref(14)
function setGdbFontSize(v) { gdbFontSize.value = Math.min(28, Math.max(9, v)) }
const gdbSideWidth = ref(320)
function onGdbResizeDown(e) {
  const startX = e.clientX; const startW = gdbSideWidth.value
  let resizing = true
  const move = ev => { if (resizing) gdbSideWidth.value = Math.min(600, Math.max(220, startW + ev.clientX - startX)) }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}

function miQuote(s) {
  return '"' + String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"'
}
// __BODY2__
let unlisteners = []
onMounted(async () => {
  try { profiles.value = await invoke('ssh_list_profiles') } catch (e) { logger.error('list profiles', e) }
  unlisteners.push(await listen('gdb-mi-output', ev => {
    if (ev.payload?.session_id === sessionId.value) appendConsole(ev.payload.kind, ev.payload.text)
  }))
  unlisteners.push(await listen('gdb-mi-async', ev => {
    if (ev.payload?.session_id === sessionId.value) onAsync(ev.payload)
  }))
  unlisteners.push(await listen('gdb-mi-exit', ev => {
    if (ev.payload?.session_id === sessionId.value) { statusText.value = '已退出'; sessionId.value = '' }
  }))
})
onBeforeUnmount(() => {
  unlisteners.forEach(u => u())
  if (sessionId.value) invoke('gdb_mi_close', { sessionId: sessionId.value }).catch(() => {})
})

function appendConsole(kind, text) {
  consoleLines.value.push({ kind, text })
  if (consoleLines.value.length > 5000) consoleLines.value.splice(0, consoleLines.value.length - 5000)
  nextTick(() => { const el = consoleRef.value; if (el) el.scrollTop = el.scrollHeight })
}

function onAsync(p) {
  if (p.class === 'stopped') {
    statusText.value = '已停下: ' + (p.payload?.reason || '')
    curFrame.value = 0
    refreshThreads(); refreshWatch(); refreshStack(); refreshLocals(); refreshSource()
  } else if (p.class === 'running') {
    statusText.value = '运行中'
  }
}

async function start() {
  if (!profileId.value) { error.value = '请选择 SSH 档案'; return }
  if (mode.value === 'attach' && !attachPid.value) { error.value = '附加模式需要填 PID'; return }
  starting.value = true; error.value = ''
  try {
    const args = argsText.value.trim() ? argsText.value.trim().split(/\s+/) : []
    sessionId.value = await invoke('gdb_mi_start', {
      profileId: profileId.value,
      program: program.value || null,
      args,
      attachPid: mode.value === 'attach' ? Number(attachPid.value) : null,
    })
    statusText.value = '已连接'
    appendConsole('log', mode.value === 'attach' ? `[gdb 已启动，正在附加 PID ${attachPid.value}]` : '[gdb 已启动]')
    if (mode.value === 'attach') {
      // 用 MI 命令显式附加：能拿到结构化结果与 *stopped 事件；attach 停住后要手动“继续”才会跑到断点。
      const r = await runCmd(`-target-attach ${Number(attachPid.value)}`)
      if (r?.class === 'error') {
        error.value = '附加失败: ' + (r?.payload?.msg || '可能无权限(需与目标同用户/root)或 PID 不存在')
      } else {
        statusText.value = `已附加 PID ${attachPid.value}（已停住，加断点后点"继续"）`
        await refreshThreads(); await refreshStack(); await refreshLocals(); await refreshSource()
      }
    }
  } catch (e) { error.value = '启动失败: ' + (e?.message || e) } finally { starting.value = false }
}

async function stop() {
  if (sessionId.value) { await invoke('gdb_mi_close', { sessionId: sessionId.value }).catch(() => {}); sessionId.value = ''; statusText.value = '已结束' }
}

async function runCmd(cmd) {
  if (!sessionId.value) return null
  appendConsole('cmd', '» ' + cmd)
  try { return await invoke('gdb_mi_command', { sessionId: sessionId.value, command: cmd }) }
  catch (e) { error.value = '命令失败: ' + (e?.message || e); return null }
}

async function submitCmd() { const c = cmdInput.value.trim(); if (!c) return; cmdInput.value = ''; await runCmd(c) }
// __BODY3__
async function addVar() {
  const expr = exprInput.value.trim(); if (!expr || !sessionId.value) return
  const r = await runCmd(`-var-create - * ${expr}`)
  const p = r?.payload
  if (p && p.name) {
    if (varFormat.value !== 'natural') {
      await runCmd(`-var-set-format ${p.name} ${varFormat.value}`)
      const er = await runCmd(`-var-evaluate-expression ${p.name}`)
      if (er?.payload?.value != null) p.value = er.payload.value
    }
    varRoots.value.push({ name: p.name, expr, value: p.value || '', type: p.type || '', numchild: parseInt(p.numchild || '0', 10), children: [], expanded: false })
  }
}
// 变量树显示格式切换：对所有已创建的 var 对象（含子节点）设格式并重取值，便于按 hex/dec/bin 看复杂结构。
async function applyVarFormat() {
  const fmt = varFormat.value
  const walk = async nodes => {
    for (const n of nodes) {
      await runCmd(`-var-set-format ${n.name} ${fmt}`)
      const er = await runCmd(`-var-evaluate-expression ${n.name}`)
      if (er?.payload?.value != null) n.value = er.payload.value
      if (n.children.length) await walk(n.children)
    }
  }
  await walk(varRoots.value)
}
async function expandVar(node) {
  if (node.expanded) { node.expanded = false; return }
  if (!node.children.length && node.numchild > 0) {
    const r = await runCmd(`-var-list-children --all-values ${node.name}`)
    const kids = r?.payload?.children || []
    node.children = kids.map(c => ({ name: c.name, expr: c.exp || '', value: c.value || '', type: c.type || '', numchild: parseInt(c.numchild || '0', 10), children: [], expanded: false }))
  }
  node.expanded = true
}

async function addWatch() { const e = watchInput.value.trim(); if (!e) return; watchInput.value = ''; watches.value.push({ expr: e, value: '', changed: false }); await refreshWatch() }
function removeWatch(e) { watches.value = watches.value.filter(w => w.expr !== e) }
async function refreshWatch() {
  for (const w of watches.value) {
    const r = await runCmd(`-data-evaluate-expression ${miQuote(w.expr)}`)
    const v = r?.payload?.value ?? ''
    w.changed = (w.value !== '' && w.value !== v)
    w.value = v
  }
}

async function refreshThreads() {
  const r = await runCmd('-thread-info'); const p = r?.payload
  if (p?.['current-thread-id']) currentThread.value = parseInt(p['current-thread-id'], 10)
  threads.value = (p?.threads || []).map(t => ({
    id: parseInt(t.id, 10), name: t['target-id'] || t.name || '', state: t.state || '',
    frame: t.frame ? { level: t.frame.level, func: t.frame.func, file: t.frame.file || t.frame.fullname || '', line: t.frame.line || '' } : null,
  }))
}
function selectThread(id) { currentThread.value = id; runCmd(`-thread-select ${id}`).then(() => refreshWatch()) }

async function refreshBreaks() {
  const r = await runCmd('-break-list')
  const body = r?.payload?.BreakpointTable?.body || []
  breaks.value = body.map(b => ({ number: b.number, where: b.func ? `${b.func} @ ${b.file || ''}:${b.line || ''}` : (b['original-location'] || b.addr || ''), times: b.times || '0' }))
}
async function addBreak() { const loc = bpInput.value.trim(); if (!loc) return; bpInput.value = ''; await runCmd(`-break-insert ${loc}`); await refreshBreaks() }
async function delBreak(n) { await runCmd(`-break-delete ${n}`); await refreshBreaks() }

async function readMem() {
  if (!memAddr.value) return
  const r = await runCmd(`-data-read-memory-bytes ${memAddr.value} ${memCount.value}`)
  const mem = r?.payload?.memory || []
  // 把各块的连续 hex 串格式化成经典 hexdump：地址 | 16 字节 hex | ASCII。
  const rows = []
  for (const m of mem) {
    const base = parseInt(m.begin || m.addr || '0', 16)
    const hex = (m.contents || '').replace(/[^0-9a-fA-F]/g, '')
    const bytes = []
    for (let i = 0; i + 1 < hex.length; i += 2) bytes.push(parseInt(hex.slice(i, i + 2), 16))
    for (let off = 0; off < bytes.length; off += 16) {
      const chunk = bytes.slice(off, off + 16)
      const addr = (base + off).toString(16).padStart(16, '0')
      const hexCells = chunk.map(b => b.toString(16).padStart(2, '0'))
      while (hexCells.length < 16) hexCells.push('  ')
      const hexStr = hexCells.slice(0, 8).join(' ') + '  ' + hexCells.slice(8).join(' ')
      const ascii = chunk.map(b => (b >= 32 && b < 127) ? String.fromCharCode(b) : '.').join('')
      rows.push(`0x${addr}  ${hexStr}  |${ascii}|`)
    }
  }
  memText.value = rows.join('\n') || '(无数据)'
}

// ---- 调用栈 / 局部变量 / 寄存器 结构化面板 ----
function shortFile(f) {
  if (!f) return ''
  const parts = String(f).replace(/\\/g, '/').split('/')
  return parts[parts.length - 1] || f
}
async function refreshStack() {
  if (!sessionId.value) return
  const r = await runCmd('-stack-list-frames')
  const frames = r?.payload?.stack || []
  callStack.value = frames.map(f => ({
    level: f.level, func: f.func || '??',
    file: f.file || f.fullname || '', fullname: f.fullname || f.file || '', line: f.line || '', addr: f.addr || '',
  }))
}
async function selectFrame(level) {
  if (!sessionId.value) return
  curFrame.value = level
  await runCmd(`-stack-select-frame ${level}`)
  await refreshLocals(); await refreshSource(); await refreshWatch()
}
async function refreshLocals() {
  if (!sessionId.value) return
  const r = await runCmd('-stack-list-variables --all-values')
  const vars = r?.payload?.variables || []
  locals.value = vars.map(v => ({ name: v.name, value: v.value != null ? String(v.value) : '' }))
}
async function refreshRegisters() {
  if (!sessionId.value) return
  const nr = await runCmd('-data-list-register-names')
  const names = nr?.payload?.['register-names'] || []
  const vr = await runCmd('-data-list-register-values x')
  const vals = vr?.payload?.['register-values'] || []
  const hexByNum = {}
  for (const rv of vals) hexByNum[rv.number] = rv.value
  const out = []
  names.forEach((nm, i) => {
    if (!nm) return
    const hex = hexByNum[String(i)] || hexByNum[i]
    if (hex == null) return
    let dec = ''
    try { dec = BigInt(hex).toString(10) } catch (_) { dec = '' }
    out.push({ name: nm, hex, dec })
  })
  registers.value = out
}

// ---- 跨机源码对照（本地 readTextFile / 远端 ssh_read_text_file） ----
function mapSourcePath(fullpath) {
  const from = srcFromPrefix.value.trim()
  const to = srcToPrefix.value.trim()
  let p = fullpath
  if (from && p.startsWith(from)) p = to.replace(/[\\/]+$/, '') + p.slice(from.length)
  else if (!from && to) {
    // 无前缀时：把编译路径的文件名拼到目标根下（最朴素映射）。
    p = to.replace(/[\\/]+$/, '') + '/' + shortFile(p)
  }
  return p
}
async function pickLocalRoot() {
  try {
    const sel = await open({ directory: true, multiple: false, title: '选择本地源码根目录' })
    if (sel) srcToPrefix.value = typeof sel === 'string' ? sel : sel?.path
  } catch (_) {}
}
async function refreshSource() {
  if (!sessionId.value) return
  const r = await runCmd('-stack-info-frame')
  const fr = r?.payload?.frame
  if (!fr) { sourceHint.value = '当前无栈帧（先运行到断点）。'; return }
  const full = fr.fullname || fr.file || ''
  const line = parseInt(fr.line || '0', 10)
  sourceFile.value = full
  sourceCurLine.value = line
  if (!full) { sourceLines.value = []; sourceHint.value = '当前帧无源码信息。'; return }
  const target = mapSourcePath(full)
  try {
    let text = ''
    if (sourceMode.value === 'local') {
      text = await readTextFile(target)
    } else {
      // 远端：同时把映射告诉 gdb，使其内部 list/断点源码定位也一致。
      if (srcFromPrefix.value.trim() && srcToPrefix.value.trim()) {
        await runCmd(`-interpreter-exec console ${miQuote(`set substitute-path ${srcFromPrefix.value.trim()} ${srcToPrefix.value.trim()}`)}`)
      }
      text = await invoke('ssh_read_text_file', { profileId: profileId.value, path: target, maxBytes: 1048576 })
    }
    sourceLines.value = text.split(/\r?\n/).map((t, i) => ({ n: i + 1, text: t }))
    await nextTick()
    if (curLineEl && curLineEl.scrollIntoView) curLineEl.scrollIntoView({ block: 'center' })
  } catch (e) {
    sourceLines.value = []
    sourceHint.value = `读取源码失败（${shortFile(target)}）：${e?.message || e}`
  }
}
let sourceResizing = false
function onSourceResizeDown(e) {
  sourceResizing = true
  const startY = e.clientY; const startH = sourceHeight.value
  const move = ev => { if (sourceResizing) sourceHeight.value = Math.min(640, Math.max(80, startH + ev.clientY - startY)) }
  const up = () => { sourceResizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}

function addMacro() {
  const n = macroName.value.trim(); if (!n || !macroCmds.value.trim()) return
  const cmds = macroCmds.value.split(';').map(s => s.trim()).filter(Boolean)
  const i = macros.value.findIndex(m => m.name === n)
  if (i >= 0) macros.value[i] = { name: n, commands: cmds }; else macros.value.push({ name: n, commands: cmds })
  macroName.value = ''; macroCmds.value = ''
}
function removeMacro(n) { macros.value = macros.value.filter(m => m.name !== n) }
async function runMacro(m) { for (const c of m.commands) await runCmd(c) }

// L：发送到主界面编辑器（复用 template_insert_event 通道）
async function sendToEditor(content) {
  if (!content) return
  try { await invoke('emit_template_insert', { content }) }
  catch (e) { error.value = '发送到编辑器失败: ' + (e?.message || e) }
}
function flattenVar(node, depth, out) {
  out.push('  '.repeat(depth) + `${node.expr || node.name}${node.type ? ': ' + node.type : ''} = ${node.value}`)
  if (node.expanded) for (const c of node.children) flattenVar(c, depth + 1, out)
}
function sendVarTree() {
  const out = []
  for (const n of varRoots.value) flattenVar(n, 0, out)
  if (out.length) sendToEditor('```\n' + out.join('\n') + '\n```\n')
}
function sendWatch() {
  if (!watches.value.length) return
  const text = watches.value.map(w => `${w.expr} = ${w.value}`).join('\n')
  sendToEditor('```\n' + text + '\n```\n')
}
async function saveMacros() {
  try {
    const path = await save({ title: '导出 gdb 宏', defaultPath: 'gdb-macros.json', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!path) return
    await invoke('gdb_save_macros', { path, macros: macros.value })
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}
async function loadMacros() {
  try {
    const sel = await open({ multiple: false, directory: false, filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!sel) return
    const path = typeof sel === 'string' ? sel : sel?.path
    macros.value = await invoke('gdb_load_macros', { path })
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
</script>

<style scoped>
.gdb-root { height: 100%; display: flex; flex-direction: column; min-height: 0; background: var(--guaxi-content-bg); }
.gdb-bar { display: flex; gap: 8px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; }
.gdb-status { font-size: 12px; color: var(--guaxi-text-secondary, #6b7280); }
.gdb-error { font-size: 12px; color: #e5484d; }
.gdb-palette { display: flex; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; }
.gdb-body { flex: 1 1 auto; min-height: 0; display: flex; }
.gdb-tabs { height: 100%; width: 100%; display: flex; flex-direction: column; }
.gdb-tabs :deep(.gdb-pane) { height: 100%; padding: 8px 12px; min-height: 0; overflow: auto; }
.gdb-console { height: calc(100% - 44px); overflow: auto; background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; padding: 8px; border-radius: 6px; }
.gdb-console pre { margin: 0; white-space: pre-wrap; word-break: break-all; }
.gdb-l-log { color: #9aa0a6; }
.gdb-l-target { color: #79c0ff; }
.gdb-l-cmd { color: #7ee787; }
.gdb-input { display: flex; gap: 8px; margin-top: 8px; }
.gdb-actions { display: flex; gap: 8px; align-items: center; margin-bottom: 8px; flex-wrap: wrap; }
.gdb-hint { color: var(--guaxi-text-secondary, #9ca3af); font-size: 12px; padding: 12px; }
.gdb-tree { font-family: var(--guaxi-mono-font, monospace); font-size: 12px; }
.gdb-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.gdb-table th, .gdb-table td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); }
.gdb-val { font-family: var(--guaxi-mono-font, monospace); white-space: pre-wrap; word-break: break-all; }
.gdb-table tr.changed { background: rgba(255,196,0,0.16); }
.gdb-threads { font-size: 12px; }
.gdb-thread { padding: 6px 8px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); cursor: pointer; }
.gdb-thread.active { background: var(--guaxi-active-bg, rgba(59,130,246,0.12)); }
.gdb-thread-h { font-weight: 500; }
.gdb-frame { color: var(--guaxi-text-secondary, #6b7280); font-family: var(--guaxi-mono-font, monospace); }
.gdb-mem { background: #1e1e1e; color: #d4d4d4; font-family: var(--guaxi-mono-font, monospace); font-size: 12px; padding: 8px; border-radius: 6px; white-space: pre-wrap; word-break: break-all; min-height: 120px; }
.gdb-empty { flex: 1 1 auto; display: flex; align-items: center; justify-content: center; color: var(--guaxi-text-secondary, #6b7280); font-size: 13px; }

/* ---- 美化层 ---- */
.gdb-root { font-family: var(--tw-ui-font); }
.gdb-bar { gap: 8px; padding: 10px 14px; background: color-mix(in srgb, var(--guaxi-panel-bg) 55%, transparent); }
.gdb-palette { gap: 5px; padding: 8px 14px; align-items: center; background: color-mix(in srgb, var(--guaxi-panel-bg) 35%, transparent); }
.gdb-palette :deep(.n-button) { border-radius: var(--tw-radius-sm); }
.gdb-console { background: var(--tw-code-bg) !important; color: var(--tw-code-fg) !important; font-family: var(--tw-mono-font) !important; border-radius: var(--tw-radius-sm); line-height: 1.55; }
.gdb-mem { background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); border-radius: var(--tw-radius-sm); }
.gdb-l-cmd { color: var(--tw-accent); font-weight: 600; }
.gdb-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.gdb-tabs :deep(.n-tabs-tab) { font-family: var(--tw-ui-font); }
.gdb-tree, .gdb-val, .gdb-frame { font-family: var(--tw-mono-font); }
.gdb-table th { text-transform: uppercase; letter-spacing: .04em; font-size: 11px; color: var(--guaxi-text-secondary); }
.gdb-thread.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.gdb-fontctl { display: inline-flex; align-items: center; gap: 4px; margin-left: auto; padding: 2px 4px; border: 1px solid var(--guaxi-border); border-radius: 999px; }
.gdb-fbtn { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: 50%; }
.gdb-fbtn:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.gdb-fsz { min-width: 20px; text-align: center; font-size: 12px; font-family: var(--tw-mono-font); color: var(--tw-accent); }

/* 面板填满高度 + 亮色适配 */
.gdb-tabs :deep(.gdb-pane) { height: 100%; display: flex; flex-direction: column; min-height: 0; background: var(--guaxi-content-bg); }
.gdb-console { flex: 1 1 auto; height: auto !important; min-height: 0; }

/* 两列布局：左侧调试面板(accordion) + 中间控制台 */
.gdb-body { flex: 1 1 auto; min-height: 0; display: flex; }
.gdb-side { flex: 0 0 auto; overflow-y: auto; overflow-x: hidden; border-right: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 35%, transparent); }
.gdb-side :deep(.n-collapse-item__header) { padding: 8px 12px !important; font-weight: 600; font-size: 12px; }
.gdb-side :deep(.n-collapse-item) { border-top: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); margin-top: 0 !important; }
.gdb-side :deep(.n-collapse-item__content-inner) { padding: 6px 10px 10px !important; }
.gdb-sideresize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; transition: opacity .15s, background .15s; }
.gdb-sideresize:hover { opacity: 1; background: var(--tw-accent); }
.gdb-center { flex: 1 1 auto; min-width: 0; min-height: 0; display: flex; flex-direction: column; padding: 8px; }
.gdb-center .gdb-console { flex: 1 1 auto; min-height: 0; }
.gdb-actions { flex-wrap: wrap; gap: 6px; margin-bottom: 6px; }
.gdb-tree { max-height: 260px; overflow: auto; }
.gdb-threads { max-height: 200px; overflow: auto; }
.gdb-mem { max-height: 220px; overflow: auto; }

/* 1c：调试显示区统一用与终端(xterm)一致的等宽字体栈，避免落到非等宽的编辑器字体。 */
.gdb-root { --gdb-mono: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, 'Courier New', monospace; }
.gdb-console, .gdb-mem, .gdb-tree, .gdb-val, .gdb-frame, .gdb-source-body { font-family: var(--gdb-mono) !important; }

/* 调用栈 / 局部变量 / 寄存器面板 */
.gdb-stack { font-size: 12px; max-height: 220px; overflow: auto; }
.gdb-sframe { display: flex; gap: 6px; align-items: baseline; padding: 4px 6px; cursor: pointer; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); white-space: nowrap; }
.gdb-sframe:hover { background: var(--guaxi-hover-bg, rgba(0,0,0,0.04)); }
.gdb-sframe.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.gdb-sframe-lv { color: var(--guaxi-text-secondary); font-family: var(--gdb-mono); flex: 0 0 auto; }
.gdb-sframe-func { color: var(--tw-accent); overflow: hidden; text-overflow: ellipsis; }
.gdb-sframe-loc { color: var(--guaxi-text-secondary); font-family: var(--gdb-mono); margin-left: auto; flex: 0 0 auto; }
.gdb-lname, .gdb-rname { color: #79c0ff; font-family: var(--gdb-mono); }
.gdb-regs { max-height: 260px; }
.gdb-regs .gdb-val { white-space: nowrap; }

/* 源码对照面板（居中列顶部，可上下拖拽） */
.gdb-source { flex: 0 0 auto; min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius-sm); overflow: hidden; background: var(--tw-code-bg); }
.gdb-source-bar { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 5px 8px; border-bottom: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); flex: 0 0 auto; }
.gdb-arrow { color: var(--guaxi-text-secondary); }
.gdb-src-loc { font-size: 11px; color: var(--tw-accent); font-family: var(--gdb-mono); margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 220px; }
.gdb-source-body { flex: 1 1 auto; min-height: 0; overflow: auto; background: var(--tw-code-bg); color: var(--tw-code-fg); padding: 4px 0; }
.gdb-srcline { display: flex; white-space: pre; line-height: 1.5; }
.gdb-srcline.cur { background: rgba(255,196,0,0.16); box-shadow: inset 3px 0 0 #f5a524; }
.gdb-srcno { flex: 0 0 auto; width: 52px; text-align: right; padding-right: 12px; color: var(--guaxi-text-secondary); opacity: .7; user-select: none; }
.gdb-srctext { flex: 1 1 auto; }
.gdb-src-empty { padding: 12px; }
.gdb-vresize { flex: 0 0 auto; height: 5px; cursor: row-resize; background: var(--guaxi-border); opacity: .5; margin: 4px 0; border-radius: 3px; transition: opacity .15s, background .15s; }
.gdb-vresize:hover { opacity: 1; background: var(--tw-accent); }
</style>
