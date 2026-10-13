<template>
  <div class="cd-root">
    <!-- ============ 顶栏：目标设备 + 配置来源 ============ -->
    <div class="cd-bar">
      <span class="cd-lbl">目标设备</span>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="profileId" size="small" :options="profileOptions" placeholder="选择 SSH 档案（或用 YAML 里的 global）" clearable style="width:230px" />
      </template>优先用库内 SSH 档案；留空则用配置里 global 段的 ip/username/password</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <n-input v-model:value="enablePassword" type="password" size="small" placeholder="enable 密码(可选)" show-password-on="click" style="width:160px" />
      </template>设备 CLI 进特权模式 `enable` 的口令。只有勾了档案才能存进 keychain</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" :disabled="!profileId || !enablePassword" @click="saveEnable"><Icon :icon="keyIcon" width="15" height="15" /></button>
      </template>把 enable 密码存入系统 keychain（对应所选档案）</n-tooltip>
      <span class="cd-sep"></span>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib" @click="importYaml"><Icon :icon="folderIcon" width="15" height="15" /></button>
      </template>导入现有 config.yaml（global/targets/tasks 全兼容）</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <n-select v-model:value="scenarioId" size="small" :options="scenarioOptions" placeholder="内置场景模板" style="width:230px" />
      </template>选一个内置高级流程模板，填参数后一键生成整套任务</n-tooltip>
      <n-tooltip trigger="hover"><template #trigger>
        <button class="tb-ib primary" :disabled="!scenarioId" @click="openScenario"><Icon :icon="wandIcon" width="15" height="15" /></button>
      </template>配置该场景的参数并生成任务</n-tooltip>
      <n-tooltip v-if="scenarioId" trigger="hover"><template #trigger>
        <Icon :icon="helpIcon" width="14" height="14" class="tb-help" />
      </template>{{ scenarioSummary }}</n-tooltip>
      <span v-if="notice" class="cd-notice">{{ notice }}</span>
      <span v-if="error" class="cd-err">{{ error }}</span>
    </div>

    <div class="cd-body">
      <!-- ============ 左：可视化任务表 ============ -->
      <div class="cd-left" :style="{ width: leftWidth + 'px' }">
        <div class="cd-head">
          <Icon :icon="listIcon" width="13" height="13" class="cd-head-ico" />
          <span>任务表</span>
          <span class="cd-n">{{ tasks.length }} 条</span>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="addTask"><Icon :icon="plusIcon" width="14" height="14" /></button>
          </template>新增一条任务</n-tooltip>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" @click="toggleYaml"><Icon :icon="fileIcon" width="14" height="14" :class="{ 'tb-ok': showYaml }" /></button>
          </template>显示/隐藏 YAML 原文（与任务表双向同步）</n-tooltip>
        </div>
        <div class="cd-scroll">
          <div class="cd-targets">
            <n-tooltip trigger="hover"><template #trigger>
              <n-input :value="hostnamesText" size="tiny" placeholder="目标主机名，逗号分隔" style="width:100%" @update:value="setHostnames" />
            </template>targets.hostnames：SSL 主机名等逻辑目标，逗号分隔</n-tooltip>
            <n-tooltip trigger="hover"><template #trigger>
              <n-input :value="domainsText" size="tiny" placeholder="域名，逗号分隔（可空 = 只做 host 级）" style="width:100%" @update:value="setDomains" />
            </template>targets.domains：域名列表；可留空（只跑 host 级任务），或含空项表示同时跑 host 与 SNI</n-tooltip>
          </div>
          <table class="cd-table">
            <thead><tr><th>id</th><th>命令</th><th>依赖</th><th>条件</th><th>PEM / index</th><th></th></tr></thead>
            <tbody>
              <tr v-for="(t, i) in tasks" :key="i">
                <td><n-input v-model:value="t.id" size="tiny" placeholder="id" style="width:96px" @update:value="syncYaml" /></td>
                <td><n-input v-model:value="t.cmd" size="tiny" placeholder="命令（支持 {hostname}/{domain}/{index}）" style="width:100%" @update:value="syncYaml" /></td>
                <td><n-input :value="(t.depends_on || []).join(',')" size="tiny" placeholder="依赖 id" style="width:120px" @update:value="v => setDeps(t, v)" /></td>
                <td class="cd-cond">
                  <n-tooltip trigger="hover"><template #trigger><n-checkbox :checked="!!t.only_if_no_domain" size="small" @update:checked="v => { t.only_if_no_domain = v; if (v) t.only_if_domain = false; syncYaml() }">host</n-checkbox></template>只在 host 级（无域名）执行</n-tooltip>
                  <n-tooltip trigger="hover"><template #trigger><n-checkbox :checked="!!t.only_if_domain" size="small" @update:checked="v => { t.only_if_domain = v; if (v) t.only_if_no_domain = false; syncYaml() }">sni</n-checkbox></template>只在关联了域名时执行（SNI 流程）</n-tooltip>
                </td>
                <td>
                  <n-input v-model:value="t.pem_file" size="tiny" placeholder="PEM 文件(可空)" style="width:130px" @update:value="syncYaml" />
                  <n-input v-model:value="t.index" size="tiny" placeholder="index" style="width:56px;margin-top:2px" @update:value="syncYaml" />
                </td>
                <td class="cd-ops">
                  <n-tooltip trigger="hover"><template #trigger>
                    <button class="tb-ib" @click="openCatalogFor(t)"><Icon :icon="bookIcon" width="14" height="14" /></button>
                  </template>从命令目录给这条选个命令模板</n-tooltip>
                  <n-tooltip trigger="hover"><template #trigger>
                    <button class="tb-ib danger" @click="removeTask(i)"><Icon :icon="trashIcon" width="14" height="14" /></button>
                  </template>删除该任务</n-tooltip>
                </td>
              </tr>
              <tr v-if="!tasks.length"><td colspan="6" class="cd-hint">还没有任务。可「导入 config.yaml」或从「内置场景模板」生成。</td></tr>
            </tbody>
          </table>
          <div class="cd-yaml" v-if="showYaml">
            <n-input v-model:value="yamlText" type="textarea" :rows="14" placeholder="config.yaml 原文" @update:value="onYamlEdited" />
            <div class="cd-row">
              <n-tooltip trigger="hover"><template #trigger>
                <button class="tb-ib" @click="loadYaml"><Icon :icon="uploadIcon" width="15" height="15" /></button>
              </template>把上面的 YAML 重新解析回任务表（会覆盖当前任务表）</n-tooltip>
              <n-tooltip trigger="hover"><template #trigger>
                <button class="tb-ib" @click="exportYaml"><Icon :icon="saveIcon" width="15" height="15" /></button>
              </template>导出 config.yaml 到文件</n-tooltip>
            </div>
          </div>
        </div>
      </div>

      <div class="cd-vresize" @mousedown="onLeftResize"></div>

      <!-- ============ 右：执行 ============ -->
      <div class="cd-right">
        <div class="cd-head">
          <Icon :icon="playIcon" width="13" height="13" class="cd-head-ico" />
          <span>执行</span>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib" :disabled="running" @click="doPlan"><Icon :icon="listIcon" width="14" height="14" /></button>
          </template>预览：展开 targets 并做依赖排序，列出将要逐条执行的命令（不下发）</n-tooltip>
          <n-tooltip trigger="hover"><template #trigger>
            <button class="tb-ib primary" :disabled="running || !tasks.length" @click="doRun"><Icon :icon="playIcon" width="15" height="15" /></button>
          </template>开始下发（失败即停，除非配置里 continue_on_error）</n-tooltip>
          <n-tooltip v-if="running" trigger="hover"><template #trigger>
            <button class="tb-ib danger" @click="doCancel"><Icon :icon="stopIcon" width="15" height="15" /></button>
          </template>取消本次下发</n-tooltip>
          <span class="cd-stat" v-if="runStat">{{ runStat }}</span>
          <span v-if="notice" class="cd-notice">{{ notice }}</span>
          <span v-if="error" class="cd-err">{{ error }}</span>
        </div>
        <div class="cd-scroll">
          <table class="cd-table" v-if="steps.length">
            <thead><tr><th>#</th><th>状态</th><th>命令</th><th>说明</th></tr></thead>
            <tbody>
              <tr v-for="(s, i) in steps" :key="s.key" :class="'cd-' + s.status">
                <td>{{ i + 1 }}</td>
                <td class="cd-st">{{ statusLabel(s.status) }}</td>
                <td class="cd-cmd" :title="s.cmd">{{ s.cmd }}</td>
                <td class="cd-reason" :title="s.reason">{{ s.reason }}</td>
              </tr>
            </tbody>
          </table>
          <div v-else class="cd-hint">点右侧「预览」看展开后的执行计划，或直接「运行」。</div>
          <div class="cd-warns" v-if="warnings.length">
            <div v-for="(w, i) in warnings" :key="i">⚠ {{ w }}</div>
          </div>
          <div class="cd-out" v-if="lastOutput">
            <div class="cd-out-head">最近一次命令输出</div>
            <pre class="cd-pre">{{ lastOutput }}</pre>
          </div>
        </div>
      </div>
    </div>

    <!-- 场景参数弹窗 -->
    <n-modal v-model:show="showScenario" :mask-closable="false" style="width:680px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="wandIcon" width="16" height="16" class="cd-head-ico" />
          <span>{{ currentScenarioName }}</span>
          <button class="tb-ib" @click="showScenario = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b">
          <div class="cd-hint">{{ scenarioSummary }}</div>
          <div v-for="f in scenarioParams" :key="f.name" class="cd-field">
            <label>{{ f.name }}<span v-if="f.required" class="cd-req">*</span></label>
            <n-input v-model:value="scenarioValues[f.name]" size="small" :placeholder="f.example" />
            <span class="cd-doc">{{ f.doc }}</span>
          </div>
        </div>
        <div class="cd-modal-f">
          <n-button size="small" @click="showScenario = false">取消</n-button>
          <n-button size="small" type="primary" @click="renderScenario">生成任务</n-button>
        </div>
      </div>
    </n-modal>

    <!-- 命令目录弹窗 -->
    <n-modal v-model:show="showCatalog" :mask-closable="false" style="width:860px;max-width:94vw">
      <div class="cd-modal">
        <div class="cd-modal-h">
          <Icon :icon="bookIcon" width="16" height="16" class="cd-head-ico" />
          <span>命令目录（SSL / SLB）</span>
          <n-input v-model:value="catalogQuery" size="small" placeholder="搜索命令或说明" clearable style="width:220px" />
          <button class="tb-ib" @click="showCatalog = false"><Icon :icon="xIcon" width="16" height="16" /></button>
        </div>
        <div class="cd-modal-b cd-cat">
          <div class="cd-cat-list">
            <template v-for="g in catalogGroups" :key="g.group">
              <div class="cd-cat-g">{{ g.group }}</div>
              <div v-for="c in g.items" :key="c.id" class="cd-cat-item" :class="{ active: picked && picked.id === c.id }" @click="pick(c)">
                <span class="cd-cat-name">{{ c.name }}</span>
                <span class="cd-cat-syn">{{ c.syntax }}</span>
              </div>
            </template>
          </div>
          <div class="cd-cat-detail" v-if="picked">
            <div class="cd-cat-title">{{ picked.name }}
              <span v-if="picked.pem" class="cd-tag">需要 PEM</span>
            </div>
            <div class="cd-cat-sum">{{ picked.summary }}</div>
            <div class="cd-cat-syntax">{{ picked.syntax }}</div>
            <div v-for="(pp, i) in picked.params" :key="i" class="cd-field">
              <label :class="{ 'cd-req-l': pp.required }">{{ pp.name }}</label>
              <n-input v-model:value="pickedValues[pp.name]" size="small" :placeholder="pp.example" />
              <span class="cd-doc">{{ pp.doc }}</span>
            </div>
            <div class="cd-row">
              <n-button size="small" @click="insertCatalog(false)">追加到任务表</n-button>
              <n-button size="small" type="primary" @click="insertCatalog(true)">替换选中任务</n-button>
            </div>
            <div class="cd-hint">生成的命令会把填的参数带进去；占位符 {{ '{hostname}' }} / {{ '{domain}' }} / {{ '{index}' }} 保留给 targets 展开时替换。</div>
          </div>
        </div>
      </div>
    </n-modal>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import { Icon } from '@iconify/vue'
import keyIcon from '@iconify-icons/lucide/key-round'
import folderIcon from '@iconify-icons/lucide/folder-open'
import wandIcon from '@iconify-icons/lucide/wand-2'
import helpIcon from '@iconify-icons/lucide/help-circle'
import listIcon from '@iconify-icons/lucide/list'
import plusIcon from '@iconify-icons/lucide/plus'
import fileIcon from '@iconify-icons/lucide/file-code'
import bookIcon from '@iconify-icons/lucide/book-open'
import trashIcon from '@iconify-icons/lucide/trash-2'
import playIcon from '@iconify-icons/lucide/play'
import stopIcon from '@iconify-icons/lucide/square'
import uploadIcon from '@iconify-icons/lucide/upload'
import saveIcon from '@iconify-icons/lucide/save'
import xIcon from '@iconify-icons/lucide/x'

// ============ 状态 ============
const profiles = ref([])
const profileOptions = computed(() => profiles.value.map(p => ({ label: `${p.name || p.host}`, value: p.id })))
const profileId = ref(null)
const enablePassword = ref('')

const cfg = ref({ global: {}, targets: { hostnames: [], domains: [] }, tasks: [], continue_on_error: false })
const yamlText = ref('')
const showYaml = ref(false)
const baseDir = ref('')

const catalog = ref([])
const catalogQuery = ref('')
const showCatalog = ref(false)
const picked = ref(null)
const pickedValues = ref({})
const catalogTarget = ref(-1)   // 替换哪条任务；-1 = 追加

const scenarios = ref([])
const scenarioId = ref(null)
const showScenario = ref(false)
const scenarioValues = ref({})
const scenarioTarget = ref(null)

const steps = ref([])
const warnings = ref([])
const running = ref(false)
const runId = ref('')
const runStat = ref('')
const lastOutput = ref('')
let unlisteners = []

const leftWidth = ref(560)
const notice = ref(''); let noticeTimer = null
const error = ref('')
function note(m) { notice.value = m; if (noticeTimer) clearTimeout(noticeTimer); noticeTimer = setTimeout(() => { notice.value = '' }, 12000) }

const tasks = computed(() => cfg.value.tasks || [])
const hostnamesText = computed(() => (cfg.value.targets?.hostnames || []).join(','))
const domainsText = computed(() => (cfg.value.targets?.domains || []).join(','))
function setHostnames(v) { cfg.value.targets.hostnames = v.split(',').map(s => s.trim()).filter(Boolean); syncYaml() }
// 域名列表只放真实域名即可：host 级任务靠 only_if_no_domain 强制 domain=""，不依赖这里有没有空项。
function setDomains(v) { cfg.value.targets.domains = v.split(',').map(s => s.trim()).filter(Boolean); syncYaml() }
function setDeps(t, v) { t.depends_on = String(v).split(',').map(s => s.trim()).filter(Boolean); syncYaml() }

// ============ YAML 双向同步（YAML 只由后端解析，前端不引 YAML 库） ============
let syncing = false
async function syncYaml() {
  if (syncing) return
  syncing = true
  try { yamlText.value = await invoke('cmd_dispatch_to_yaml', { config: cfg.value }) }
  catch (e) { error.value = '生成 YAML 失败: ' + (e?.message || e) }
  finally { syncing = false }
}
async function loadYaml() {
  try {
    const c = await invoke('cmd_dispatch_parse', { configYaml: yamlText.value })
    cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
    note('已从 YAML 载入任务表')
  } catch (e) { error.value = 'YAML 解析失败: ' + (e?.message || e) }
}
function onYamlEdited() { /* 等用户点"载入"再解析，避免边打字边报错刷屏 */ }

async function importYaml() {
  error.value = ''
  const sel = await open({ multiple: false, directory: false, title: '选择 config.yaml', filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }] })
  if (!sel) return
  const path = typeof sel === 'string' ? sel : sel?.path
  try {
    yamlText.value = await readTextFile(path)
    // PEM 相对路径以该配置文件所在目录为基准。
    baseDir.value = String(path).replace(/\\/g, '/').split('/').slice(0, -1).join('/')
    const c = await invoke('cmd_dispatch_parse', { configYaml: yamlText.value })
    cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
    if (cfg.value.global?.ip && !profileId.value) note(`配置里带了 global.ip=${cfg.value.global.ip}，未选档案时将用它直连`)
    note(`已导入 ${String(path).replace(/\\/g, '/').split('/').pop()}`)
  } catch (e) { error.value = '导入失败: ' + (e?.message || e) }
}
async function exportYaml() {
  try {
    await syncYaml()
    const p = await save({ defaultPath: 'config.yaml', filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }] })
    if (p) { await invoke('write_file_content', { path: p, content: yamlText.value }); note('已导出: ' + p) }
  } catch (e) { error.value = '导出失败: ' + (e?.message || e) }
}
function toggleYaml() { showYaml.value = !showYaml.value; if (showYaml.value) syncYaml() }

// ============ 任务表 ============
function addTask() {
  cfg.value.tasks.push({ id: 'task' + (tasks.value.length + 1), cmd: '', pem_file: '', index: '', depends_on: [], only_if_domain: false, only_if_no_domain: false })
  syncYaml()
}
function removeTask(i) { cfg.value.tasks.splice(i, 1); syncYaml() }

// ============ 命令目录 ============
const catalogGroups = computed(() => {
  const q = catalogQuery.value.trim().toLowerCase()
  const hit = catalog.value.filter(c => !q || c.name.toLowerCase().includes(q) || c.summary.toLowerCase().includes(q) || c.group.toLowerCase().includes(q))
  const m = new Map()
  for (const c of hit) { if (!m.has(c.group)) m.set(c.group, []); m.get(c.group).push(c) }
  return Array.from(m.entries()).map(([group, items]) => ({ group, items }))
})
function openCatalogFor(t) {
  catalogTarget.value = cfg.value.tasks.indexOf(t)
  showCatalog.value = true
  if (t.cmd) { /* 保留当前内容，用户选完再替换 */ }
}
function pick(c) {
  picked.value = c
  pickedValues.value = {}
  for (const pp of c.params) if (pp.example && !/^\d+$/.test(pp.example)) pickedValues.value[pp.name] = ''
}
function buildFromCatalog(c, vals) {
  // 用模板 + 用户填的参数拼出命令；占位符保留。
  let cmd = c.cmd_template
  for (const [k, v] of Object.entries(vals)) {
    if (!v) continue
    const re = new RegExp('\\{' + k + '\\}', 'g')
    cmd = cmd.replace(re, v)
  }
  return cmd
}
function insertCatalog(replace) {
  const c = picked.value
  if (!c) return
  const cmd = buildFromCatalog(c, pickedValues.value)
  const deps = []
  if (replace && catalogTarget.value >= 0) {
    const t = cfg.value.tasks[catalogTarget.value]
    t.id = c.id
    t.cmd = cmd
    if (c.pem) t.pem_file = t.pem_file || './cfgfiles/'
    if (c.id.includes('activate')) t.index = t.index || '1'
    syncYaml()
  } else {
    cfg.value.tasks.push({ id: c.id, cmd, pem_file: c.pem ? './cfgfiles/' : '', index: '', depends_on: deps, only_if_domain: false, only_if_no_domain: false })
    syncYaml()
  }
  showCatalog.value = false
}

// ============ 场景模板 ============
const scenarioOptions = computed(() => scenarios.value.map(s => ({ label: s.name, value: s.id })))
const currentScenario = computed(() => scenarios.value.find(s => s.id === scenarioId.value) || null)
const currentScenarioName = computed(() => currentScenario.value?.name || '场景参数')
const scenarioSummary = computed(() => currentScenario.value?.summary || '')
const scenarioParams = computed(() => currentScenario.value?.params || [])
function openScenario() {
  scenarioValues.value = {}
  for (const pp of scenarioParams.value) scenarioValues.value[pp.name] = pp.example && !/^\d+$/.test(pp.example) && !pp.example.startsWith('http') ? pp.example : (pp.example || '')
  showScenario.value = true
}
async function renderScenario() {
  try {
    const y = await invoke('cmd_scenario_render', {
      id: scenarioId.value,
      opts: {
        ip: cfg.value.global?.ip || null,
        port: cfg.value.global?.port || 22,
        username: cfg.value.global?.username || null,
        password: cfg.value.global?.password || null,
        enablePassword: enablePassword.value || null,
        params: scenarioValues.value,
      },
    })
    yamlText.value = y
    const c = await invoke('cmd_dispatch_parse', { configYaml: y })
    cfg.value = { global: c.global || {}, targets: c.targets || { hostnames: [], domains: [] }, tasks: c.tasks || [], continue_on_error: !!c.continue_on_error }
    showScenario.value = false
    note(`已按场景「${currentScenarioName.value}」生成 ${tasks.value.length} 条任务`)
  } catch (e) { error.value = '生成场景失败: ' + (e?.message || e) }
}

// ============ 执行 ============
function statusLabel(s) {
  return { pending: '待执行', running: '执行中', ok: '成功', 'ok-idem': '幂等', fail: '失败', skipped: '跳过' }[s] || s
}
function resetSteps(planSteps) {
  steps.value = planSteps.map(t => ({ key: t.task_key || t.taskKey, cmd: t.cmd, status: 'pending', reason: '' }))
}
async function doPlan() {
  error.value = ''
  try {
    await syncYaml()
    const plan = await invoke('cmd_dispatch_plan', { configYaml: yamlText.value })
    resetSteps(plan.steps)
    warnings.value = plan.warnings || []
    runStat.value = `共 ${plan.task_count} 条（${plan.target_count} 个目标）`
  } catch (e) { error.value = '预览失败: ' + (e?.message || e) }
}
async function doRun() {
  error.value = ''
  await syncYaml()
  try {
    const plan = await invoke('cmd_dispatch_plan', { configYaml: yamlText.value })
    resetSteps(plan.steps)
    warnings.value = plan.warnings || []
  } catch (e) { error.value = '配置有问题: ' + (e?.message || e); return }
  runId.value = 'run-' + Date.now()
  lastOutput.value = ''
  runStat.value = `执行中 0/${steps.value.length}`
  running.value = true
  try {
    // base_dir 让 PEM 相对路径以导入的 config.yaml 所在目录为基准。
    const r = await invoke('cmd_dispatch_run', {
      runId: runId.value,
      profileId: profileId.value || null,
      configYaml: yamlText.value,
      opts: { baseDir: baseDir.value || null, continueOnError: null },
    })
    runStat.value = `执行中 0/${r.plan.task_count}`
  } catch (e) { error.value = '下发失败: ' + (e?.message || e); running.value = false }
}
async function doCancel() {
  if (runId.value) { await invoke('cmd_dispatch_cancel', { runId: runId.value }).catch(() => {}); note('已请求取消') }
}
function onTask(ev) {
  const p = ev.payload || {}
  if (p.runId !== runId.value) return
  const i = p.index
  if (steps.value[i]) {
    steps.value[i].status = p.status
    if (p.reason) steps.value[i].reason = p.reason
    if (!steps.value[i].cmd && p.cmd) steps.value[i].cmd = p.cmd
  }
  if (p.output) lastOutput.value = p.output
  const done = steps.value.filter(s => s.status !== 'pending' && s.status !== 'running').length
  runStat.value = `执行中 ${done}/${steps.value.length}`
}
function onDone(ev) {
  const p = ev.payload || {}
  if (p.runId !== runId.value) return
  running.value = false
  if (p.error) { error.value = '连接/执行失败: ' + p.error; runStat.value = ''; return }
  runStat.value = `${p.ok ? '全部完成' : '结束'}：成功 ${p.done - p.failed}/${p.total}${p.failed ? `，失败 ${p.failed}` : ''}${p.cancelled ? '（已取消）' : ''}`
}

// ============ 生命周期 ============
async function saveEnable() {
  if (!profileId.value || !enablePassword.value) return
  try { await invoke('ssh_set_enable_secret', { profileId: profileId.value, secret: enablePassword.value }); note('enable 密码已存入 keychain') }
  catch (e) { error.value = '保存失败: ' + (e?.message || e) }
}
function onLeftResize(e) {
  const startX = e.clientX; const startW = leftWidth.value
  let on = true
  const mv = ev => { if (on) leftWidth.value = Math.min(1000, Math.max(380, startW + ev.clientX - startX)) }
  const up = () => { on = false; document.removeEventListener('mousemove', mv); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', mv); document.addEventListener('mouseup', up)
}
onMounted(async () => {
  try { profiles.value = await invoke('ssh_list_profiles') } catch (_) {}
  try { catalog.value = await invoke('cmd_catalog_list') } catch (_) {}
  try { scenarios.value = await invoke('cmd_scenario_list') } catch (_) {}
  unlisteners.push(await listen('cmd-dispatch-task', onTask))
  unlisteners.push(await listen('cmd-dispatch-done', onDone))
  await syncYaml()
})
onBeforeUnmount(() => {
  unlisteners.forEach(u => u())
  if (noticeTimer) clearTimeout(noticeTimer)
})
</script>

<style scoped>
.cd-root { height: 100%; display: flex; flex-direction: column; min-height: 0; font-family: var(--tw-ui-font); }
.cd-bar { display: flex; align-items: center; gap: 8px; padding: 8px 14px; border-bottom: 1px solid var(--guaxi-border); flex-wrap: wrap; flex: 0 0 auto; background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); }
.cd-lbl { font-size: 12px; color: var(--guaxi-text-secondary); }
.cd-sep { width: 1px; height: 18px; background: var(--guaxi-border); margin: 0 4px; }
.cd-notice { font-size: 12px; color: #f5a524; }
.cd-err { font-size: 12px; color: #e5484d; }
.cd-stat { font-size: 12px; color: var(--tw-accent); font-family: var(--tw-mono-font); }

.cd-body { flex: 1 1 auto; min-height: 0; display: flex; }
.cd-left { flex: 0 0 auto; display: flex; flex-direction: column; min-height: 0; border-right: 1px solid var(--guaxi-border); }
.cd-right { flex: 1 1 auto; min-width: 0; display: flex; flex-direction: column; min-height: 0; }
.cd-vresize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; }
.cd-vresize:hover { opacity: 1; background: var(--tw-accent); }

.cd-head { flex: 0 0 auto; display: flex; align-items: center; gap: 6px; padding: 6px 10px; font-size: 12px; font-weight: 600; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.08)); color: var(--guaxi-text-secondary); background: color-mix(in srgb, var(--guaxi-panel-bg) 60%, transparent); }
.cd-head-ico { color: var(--tw-accent); }
.cd-n { font-size: 10px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); }
.cd-scroll { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 8px 10px; }

.cd-targets { display: flex; gap: 6px; margin-bottom: 8px; }
.cd-table { width: 100%; border-collapse: collapse; font-size: 12px; }
.cd-table th, .cd-table td { text-align: left; padding: 4px 6px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.06)); vertical-align: top; }
.cd-table th { font-size: 11px; text-transform: uppercase; letter-spacing: .04em; color: var(--guaxi-text-secondary); position: sticky; top: 0; background: var(--guaxi-panel-bg); z-index: 1; }
.cd-cond { white-space: nowrap; }
.cd-ops { white-space: nowrap; }
.cd-cmd { font-family: var(--tw-mono-font); max-width: 420px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-reason { font-size: 11px; color: var(--guaxi-text-secondary); max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-st { white-space: nowrap; }
.cd-ok td { color: #30a46c; }
.cd-ok-idem td { color: #3b82f6; }
.cd-fail td { color: #e5484d; }
.cd-running td { color: var(--tw-accent); }
.cd-skipped td { color: var(--guaxi-text-secondary); opacity: .7; }
.cd-hint { font-size: 12px; color: var(--guaxi-text-secondary); padding: 8px 2px; }
.cd-warns { margin-top: 8px; font-size: 12px; color: #f5a524; }
.cd-yaml { margin-top: 8px; }
.cd-row { display: flex; gap: 6px; margin-top: 6px; }
.cd-out { margin-top: 10px; }
.cd-out-head { font-size: 12px; font-weight: 600; color: var(--tw-accent); margin-bottom: 4px; }
.cd-pre { margin: 0; padding: 8px; border-radius: var(--tw-radius-sm); background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); font-size: 12px; white-space: pre-wrap; word-break: break-all; max-height: 320px; overflow: auto; }

.cd-modal { display: flex; flex-direction: column; max-height: 84vh; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; }
.cd-modal-h { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.cd-modal-b { flex: 1 1 auto; min-height: 0; overflow: auto; padding: 12px 14px; }
.cd-modal-f { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--guaxi-border); }
.cd-field { display: grid; grid-template-columns: 130px 1fr; gap: 4px 10px; align-items: center; margin-bottom: 8px; }
.cd-field label { font-size: 12px; color: var(--guaxi-text-secondary); font-family: var(--tw-mono-font); }
.cd-field .cd-doc { grid-column: 2; font-size: 11px; color: var(--guaxi-text-secondary); }
.cd-req { color: #e5484d; margin-left: 2px; }
.cd-req-l::after { content: ' *'; color: #e5484d; }

.cd-cat { display: flex; gap: 10px; min-height: 420px; }
.cd-cat-list { flex: 0 0 320px; overflow: auto; border-right: 1px solid var(--guaxi-border); padding-right: 8px; }
.cd-cat-g { font-size: 11px; font-weight: 700; color: var(--tw-accent); margin: 8px 0 4px; }
.cd-cat-item { padding: 5px 8px; border-radius: 6px; cursor: pointer; }
.cd-cat-item:hover { background: var(--tw-accent-soft); }
.cd-cat-item.active { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); }
.cd-cat-name { display: block; font-size: 12px; font-family: var(--tw-mono-font); }
.cd-cat-syn { display: block; font-size: 11px; color: var(--guaxi-text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-cat-detail { flex: 1 1 auto; min-width: 0; overflow: auto; }
.cd-cat-title { font-size: 13px; font-weight: 600; margin-bottom: 6px; }
.cd-tag { font-size: 10px; padding: 1px 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); margin-left: 6px; }
.cd-cat-sum { font-size: 12px; color: var(--guaxi-text-primary); margin-bottom: 8px; line-height: 1.5; }
.cd-cat-syntax { font-family: var(--tw-mono-font); font-size: 12px; padding: 6px 8px; border-radius: 6px; background: var(--tw-code-bg); color: var(--tw-code-fg); margin-bottom: 10px; word-break: break-all; }
</style>
