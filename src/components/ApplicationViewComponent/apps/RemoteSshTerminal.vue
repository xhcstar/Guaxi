<template>
  <div class="ssh-root">
    <div class="ssh-collapsed-rail" v-if="sideCollapsed" @click="sideCollapsed = false" title="展开连接面板">
      <Icon :icon="chevronRightIcon" width="16" height="16" />
      <span class="ssh-rail-text">连接面板</span>
    </div>
    <div class="ssh-side" v-show="!sideCollapsed" :style="{ width: sideWidth + 'px' }">
      <div class="ssh-side-head">
        <span class="ssh-sec-toggle" @click="connCollapsed = !connCollapsed">
          <Icon :icon="connCollapsed ? chevronRightIcon : chevronDownIcon" width="14" height="14" />连接会话
        </span>
        <span class="ssh-head-actions">
          <n-button size="tiny" type="primary" @click="openNewConn" title="新建连接">
            <template #icon><Icon :icon="plusIcon" width="15" height="15" /></template>新建
          </n-button>
          <n-button size="tiny" quaternary @click="sideCollapsed = true" title="折叠面板"><Icon :icon="chevronLeftIcon" width="15" height="15" /></n-button>
        </span>
      </div>
      <div class="ssh-profiles" v-show="!connCollapsed">
        <div v-for="grp in groupedProfiles" :key="grp.group" class="ssh-folder">
          <div class="ssh-folder-head" @click="toggleGroup(grp.group)">
            <Icon :icon="groupOpen[grp.group] === false ? chevronRightIcon : chevronDownIcon" width="13" height="13" />
            <Icon :icon="folderIcon" width="13" height="13" class="ssh-folder-ico" />
            <span class="ssh-folder-name">{{ grp.group }}</span>
            <span class="ssh-folder-count">{{ grp.items.length }}</span>
          </div>
          <div v-show="groupOpen[grp.group] !== false" class="ssh-folder-items">
            <div
              v-for="p in grp.items"
              :key="p.id"
              class="ssh-profile"
              @click="connect(p)"
              :title="'连接 ' + (p.name || p.host || p.serial_path)"
            >
              <div class="ssh-profile-ico"><Icon :icon="kindIcon(p.kind)" width="16" height="16" /></div>
              <div class="ssh-profile-main">
                <div class="ssh-profile-name">{{ p.name || p.host || p.serial_path }}</div>
                <div class="ssh-profile-sub">{{ profileSub(p) }}</div>
              </div>
              <div class="ssh-profile-badge">{{ kindBadge(p) }}</div>
              <div class="ssh-profile-actions">
                <button class="ssh-ico-btn primary" @click.stop="connect(p)" title="连接"><Icon :icon="plugIcon" width="14" height="14" /></button>
                <button class="ssh-ico-btn" @click.stop="openEditConn(p)" title="编辑"><Icon :icon="editIcon" width="14" height="14" /></button>
                <button class="ssh-ico-btn danger" @click.stop="remove(p)" title="删除"><Icon :icon="trashIcon" width="14" height="14" /></button>
              </div>
            </div>
          </div>
        </div>
        <div v-if="!profiles.length" class="ssh-empty-side">还没有连接，点右上角“新建”添加 SSH / 串口 / Telnet。</div>
      </div>

      <SftpBrowser class="ssh-sftp" :profiles="profiles" :active-profile-id="activeProfileId" />
    </div>

    <n-modal v-model:show="showConnModal" :mask-closable="false">
      <div class="ssh-modal">
        <div class="ssh-modal-head">
          <span>{{ editing.id ? '编辑连接' : '新建连接' }}</span>
          <Icon :icon="xIcon" width="18" height="18" class="ssh-modal-close" @click="showConnModal = false" />
        </div>
        <div class="ssh-modal-tabs">
          <button
            v-for="t in connTabs"
            :key="t.key"
            class="ssh-mtab"
            :class="{ active: connTab === t.key }"
            @click="connTab = t.key"
          >
            <Icon :icon="t.icon" width="15" height="15" />{{ t.label }}
          </button>
        </div>
        <div class="ssh-modal-body">
          <template v-if="connTab === 'ssh'">
            <div class="ssh-field"><label>名称</label><n-input v-model:value="editing.name" size="small" placeholder="给这个连接起个名字" /></div>
            <div class="ssh-field"><label>分组 / 文件夹</label><n-input v-model:value="editing.group" size="small" placeholder="如 生产 / 测试（空=未分组）" /></div>
            <div class="ssh-field">
              <label>主机 / IP</label>
              <div class="ssh-row">
                <n-input v-model:value="editing.host" size="small" placeholder="192.168.1.10 或 域名" style="flex:1" />
                <n-input-number v-model:value="editing.port" size="small" :min="1" :max="65535" :show-button="false" style="width:96px" />
                <span class="ssh-port-tag">端口</span>
              </div>
            </div>
            <div class="ssh-field"><label>用户名</label><n-input v-model:value="editing.user" size="small" placeholder="root" /></div>
            <div class="ssh-field"><label>认证方式</label><n-select v-model:value="editing.auth" size="small" :options="authOptions" :to="false" /></div>
            <div class="ssh-field" v-if="editing.auth === 'key'"><label>私钥路径</label><n-input v-model:value="editing.key_path" size="small" placeholder="~/.ssh/id_ed25519" /></div>
            <template v-if="editing.auth === 'password'">
              <div class="ssh-field"><label>密码</label><n-input v-model:value="password" type="password" size="small" placeholder="存入系统 keychain" show-password-on="click" /></div>
              <div class="ssh-hint">密码存入系统 keychain，连接时自动用于认证，不会明文落盘。</div>
            </template>
            <span v-if="error" class="ssh-error">{{ error }}</span>
            <div class="ssh-modal-foot">
              <n-button size="small" @click="showConnModal = false">取消</n-button>
              <n-button size="small" @click="saveProfile()"><template #icon><Icon :icon="saveIcon" width="14" height="14" /></template>保存</n-button>
              <n-button size="small" type="primary" @click="saveAndConnect"><template #icon><Icon :icon="plugIcon" width="14" height="14" /></template>保存并连接</n-button>
            </div>
          </template>

          <template v-else-if="connTab === 'serial'">
            <div class="ssh-field"><label>名称</label><n-input v-model:value="editing.name" size="small" placeholder="给这个串口连接起个名字" /></div>
            <div class="ssh-field"><label>分组 / 文件夹</label><n-input v-model:value="editing.group" size="small" placeholder="空=未分组" /></div>
            <div class="ssh-field">
              <label>串口</label>
              <div class="ssh-row">
                <n-select v-model:value="serialPort" size="small" :options="serialPorts.map(p => ({ label: `${p.name} (${p.kind})`, value: p.name }))" placeholder="选择串口" style="flex:1" />
                <n-button size="small" @click="loadSerialPorts">刷新</n-button>
              </div>
            </div>
            <div class="ssh-field"><label>波特率</label><n-input-number v-model:value="serialBaud" size="small" :min="300" :show-button="false" style="width:140px" /></div>
            <span v-if="error" class="ssh-error">{{ error }}</span>
            <div class="ssh-modal-foot">
              <n-button size="small" @click="showConnModal = false">取消</n-button>
              <n-button size="small" type="primary" @click="connectSerial"><template #icon><Icon :icon="usbIcon" width="14" height="14" /></template>连接串口</n-button>
            </div>
          </template>

          <template v-else>
            <div class="ssh-field"><label>名称</label><n-input v-model:value="editing.name" size="small" placeholder="给这个 Telnet 连接起个名字" /></div>
            <div class="ssh-field"><label>分组 / 文件夹</label><n-input v-model:value="editing.group" size="small" placeholder="空=未分组" /></div>
            <div class="ssh-field">
              <label>Telnet 主机</label>
              <div class="ssh-row">
                <n-input v-model:value="telnetHost" size="small" placeholder="主机 / IP" style="flex:1" />
                <n-input-number v-model:value="telnetPort" size="small" :min="1" :max="65535" :show-button="false" style="width:96px" />
                <span class="ssh-port-tag">端口</span>
              </div>
            </div>
            <span v-if="error" class="ssh-error">{{ error }}</span>
            <div class="ssh-modal-foot">
              <n-button size="small" @click="showConnModal = false">取消</n-button>
              <n-button size="small" type="primary" @click="connectTelnet"><template #icon><Icon :icon="networkIcon" width="14" height="14" /></template>连接 Telnet</n-button>
            </div>
          </template>
        </div>
      </div>
    </n-modal>

    <div class="ssh-resize" v-show="!sideCollapsed" @mousedown="onSideResizeDown"></div>

    <div class="ssh-main">
      <div class="ssh-tabbar">
        <div class="ssh-tabs-list">
          <div
            v-for="s in sessions"
            :key="s.session_id"
            class="ssh-tab"
            :class="{ active: s.session_id === activeSession }"
            @click="activeSession = s.session_id"
          >
            <Icon :icon="s.kind === 'serial' ? usbIcon : (s.kind === 'telnet' ? networkIcon : serverIcon)" width="13" height="13" />
            {{ s.host }}
            <span class="ssh-tab-close" @click.stop="closeSession(s.session_id)"><Icon :icon="xIcon" width="12" height="12" /></span>
          </div>
        </div>
        <div class="ssh-fontctl" v-if="sessions.length" title="终端字号">
          <button class="ssh-fbtn" @click="setTermFontSize(termFontSize - 1)"><Icon :icon="minusIcon" width="13" height="13" /></button>
          <span class="ssh-fsz">{{ termFontSize }}</span>
          <button class="ssh-fbtn" @click="setTermFontSize(termFontSize + 1)"><Icon :icon="plusIcon" width="13" height="13" /></button>
        </div>
      </div>
      <div class="ssh-toolbar" v-if="sessions.length">
        <n-input v-model:value="searchQuery" size="tiny" placeholder="查找终端内容" style="width:180px" @keyup.enter="doSearch(1)" />
        <n-button size="tiny" quaternary @click="doSearch(-1)" title="上一个"><Icon :icon="chevronUpIcon" width="14" height="14" /></n-button>
        <n-button size="tiny" quaternary @click="doSearch(1)" title="下一个"><Icon :icon="chevronDownIcon" width="14" height="14" /></n-button>
        <span class="ssh-tb-sep"></span>
        <n-button size="tiny" @click="doCopy"><Icon :icon="copyIcon" width="13" height="13" />复制</n-button>
        <n-button size="tiny" @click="doPaste"><Icon :icon="pasteIcon" width="13" height="13" />粘贴</n-button>
        <n-button size="tiny" @click="doSave"><Icon :icon="saveIcon" width="13" height="13" />保存</n-button>
        <n-checkbox v-model:checked="copyOnSelect" size="small">选中即复制</n-checkbox>
        <span v-if="searchInfo" class="ssh-tb-info">{{ searchInfo }}</span>
      </div>
      <div class="ssh-term-hosts">
        <div
          v-for="s in sessions"
          :key="s.session_id"
          class="ssh-term-host"
          :ref="el => setTermEl(s.session_id, el)"
          v-show="s.session_id === activeSession"
        ></div>
        <div v-if="!sessions.length" class="ssh-term-empty">选择左侧档案点“连接”，开一个 SSH 终端。</div>
      </div>

      <div class="ssh-macrobar" v-if="sessions.length">
        <n-button v-if="!recording" size="tiny" type="error" ghost @click="startRecord"><Icon :icon="recordIcon" width="13" height="13" />录制宏</n-button>
        <template v-else>
          <n-button size="tiny" type="error" @click="stopRecord"><Icon :icon="stopIcon" width="13" height="13" />停止 ({{ recordBuf.length }} 字符)</n-button>
        </template>
        <n-input v-model:value="macroName" size="tiny" placeholder="宏名" style="width:110px" />
        <span class="ssh-macro-list">
          <span v-for="m in macros" :key="m.name" class="ssh-macro-chip">
            <span @click="replayMacro(m)" :title="'重放: ' + m.name">▶ {{ m.name }}</span>
            <span class="ssh-macro-del" @click="macros = macros.filter(x => x !== m)">×</span>
          </span>
        </span>
        <span v-if="!macros.length" class="ssh-tb-info">录制你敲的命令序列，停止时命名保存，点▶重放到当前会话。</span>
      </div>
    </div>

    <!-- 右侧栏：会话录制（从左栏移出，避免左侧拥挤），可折叠 + 拖拽改宽 -->
    <div class="ssh-recright-rail" v-if="recCollapsed" @click="recCollapsed = false" title="展开会话录制">
      <Icon :icon="chevronLeftIcon" width="16" height="16" />
      <span class="ssh-rail-text">会话录制</span>
    </div>
    <template v-else>
      <div class="ssh-recresize" @mousedown="onRecResizeDown"></div>
      <div class="ssh-side-right" :style="{ width: recWidth + 'px' }">
        <div class="ssh-side-head">
          <span>会话录制</span>
          <span class="ssh-head-actions">
            <n-button size="tiny" @click="loadRecordings">刷新</n-button>
            <n-button size="tiny" quaternary @click="recCollapsed = true" title="折叠"><Icon :icon="chevronRightIcon" width="15" height="15" /></n-button>
          </span>
        </div>
        <div class="ssh-rec-list">
          <div v-for="r in recordings" :key="r.path" class="ssh-rec" @click="$emit('open-in-analysis', r.path)" :title="r.path">
            <div class="ssh-rec-name">{{ r.name }}</div>
            <div class="ssh-rec-sub">{{ formatBytes(r.size) }} · 点开在日志分析中回放</div>
          </div>
          <div v-if="!recordings.length" class="ssh-empty-side">连接后自动录制，回放在这里打开。</div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup>
import { ref, reactive, computed, watch, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { save } from '@tauri-apps/plugin-dialog'
import { writeText, readText } from '@tauri-apps/plugin-clipboard-manager'
import { useSettingsStore } from '../../../stores/settingsStore'
import { Terminal } from 'xterm'
import { FitAddon } from '@xterm/addon-fit'
import 'xterm/css/xterm.css'
import { createLogger } from '../../../utils/logger'
import { useSshProfiles } from '../../../composables/useSshProfiles'
import { Icon } from '@iconify/vue'
import serverIcon from '@iconify-icons/lucide/server'
import usbIcon from '@iconify-icons/lucide/usb'
import networkIcon from '@iconify-icons/lucide/network'
import xIcon from '@iconify-icons/lucide/x'
import plusIcon from '@iconify-icons/lucide/plus'
import minusIcon from '@iconify-icons/lucide/minus'
import plugIcon from '@iconify-icons/lucide/plug'
import trashIcon from '@iconify-icons/lucide/trash-2'
import editIcon from '@iconify-icons/lucide/pencil'
import saveIcon from '@iconify-icons/lucide/save'
import chevronLeftIcon from '@iconify-icons/lucide/chevron-left'
import chevronRightIcon from '@iconify-icons/lucide/chevron-right'
import chevronUpIcon from '@iconify-icons/lucide/chevron-up'
import chevronDownIcon from '@iconify-icons/lucide/chevron-down'
import folderIcon from '@iconify-icons/lucide/folder'
import copyIcon from '@iconify-icons/lucide/copy'
import pasteIcon from '@iconify-icons/lucide/clipboard-paste'
import recordIcon from '@iconify-icons/lucide/circle'
import stopIcon from '@iconify-icons/lucide/square'
import SftpBrowser from './SftpBrowser.vue'

const emit = defineEmits(['open-in-analysis'])
const logger = createLogger('RemoteSshTerminal')
const settingsStore = useSettingsStore()
const isDark = computed(() => settingsStore.appearanceTheme === 'dark')
// 终端配色跟随主题：亮色给浅底深字，暗色维持原样。
function termTheme() {
  return isDark.value
    ? { background: '#1e1e1e', foreground: '#d4d4d4', cursor: '#d4d4d4' }
    : { background: '#f6f8fa', foreground: '#1f2328', cursor: '#1f2328', selectionBackground: '#b3d4fc' }
}

const authOptions = [
  { label: '默认密钥 (~/.ssh)', value: 'agent' },
  { label: '指定私钥文件', value: 'key' },
  { label: '密码', value: 'password' },
]

const { profiles, reload: loadProfiles } = useSshProfiles(logger)
const recordings = ref([])
const editing = reactive(blankProfile())
const password = ref('')
const error = ref('')

const sessions = ref([]) // { session_id, host, kind }
const termFontSize = ref(16)
const sideWidth = ref(340)
const sideCollapsed = ref(false)
// 左栏分区折叠；右栏（会话录制）宽度/折叠。
const connCollapsed = ref(false)
const sftpCollapsed = ref(false)
const recCollapsed = ref(false)
const recWidth = ref(300)
const groupOpen = reactive({})
function toggleGroup(g) { groupOpen[g] = groupOpen[g] === false }
// 把档案按 group 文件夹归类（无 group 归"未分组"），用于 Xshell/MobaXterm 式的分组列表。
const groupedProfiles = computed(() => {
  const map = new Map()
  for (const p of profiles.value) {
    const g = (p.group && p.group.trim()) ? p.group.trim() : '未分组'
    if (!map.has(g)) map.set(g, [])
    map.get(g).push(p)
  }
  return Array.from(map.entries()).map(([group, items]) => ({ group, items }))
})
function onRecResizeDown(e) {
  const startX = e.clientX; const startW = recWidth.value
  let resizing = true
  const move = ev => { if (resizing) recWidth.value = Math.min(560, Math.max(220, startW - (ev.clientX - startX))) }
  const up = () => { resizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
}

// 连接向导弹窗：把 SSH 档案 / 串口 / Telnet 的表单从常驻左栏挪进弹窗，左栏只留会话列表。
const showConnModal = ref(false)
const connTab = ref('ssh')
const connTabs = [
  { key: 'ssh', label: 'SSH', icon: serverIcon },
  { key: 'serial', label: '串口', icon: usbIcon },
  { key: 'telnet', label: 'Telnet', icon: networkIcon },
]
function authLabel(a) {
  return a === 'password' ? '密码' : a === 'key' ? '密钥' : '默认密钥'
}
function kindIcon(k) { return k === 'serial' ? usbIcon : (k === 'telnet' ? networkIcon : serverIcon) }
function kindBadge(p) { return p.kind === 'serial' ? '串口' : (p.kind === 'telnet' ? 'Telnet' : authLabel(p.auth)) }
function profileSub(p) {
  if (p.kind === 'serial') return `${p.serial_path || ''} @ ${p.serial_baud || ''}`
  if (p.kind === 'telnet') return `${p.host}:${p.port}`
  return `${p.user}@${p.host}:${p.port}`
}
function openNewConn() {
  newProfile()
  connTab.value = 'ssh'
  showConnModal.value = true
}
function openEditConn(p) {
  edit(p)
  connTab.value = p.kind || 'ssh'
  if (p.kind === 'serial') { serialPort.value = p.serial_path || null; serialBaud.value = p.serial_baud || 115200 }
  else if (p.kind === 'telnet') { telnetHost.value = p.host || ''; telnetPort.value = p.port || 23 }
  showConnModal.value = true
}
async function saveAndConnect() {
  const saved = await saveProfile()
  if (!saved) return
  showConnModal.value = false
  await connect(saved)
}

// M2 终端复制/粘贴/查找/保存
const searchQuery = ref(''); const searchInfo = ref(''); const copyOnSelect = ref(true)
// M3 宏录制
const recording = ref(false); const recordBuf = ref(''); const macros = ref([]); const macroName = ref('')
function activeTerm() { const r = termMap.get(activeSession.value); return r ? r.term : null }
async function doCopy() {
  const t = activeTerm(); if (!t) return
  const sel = t.getSelection()
  if (sel) { await writeText(sel).catch(() => {}); searchInfo.value = '已复制' }
}
async function doPaste() {
  const t = activeTerm(); if (!t) return
  try { const txt = await readText(); if (txt) t.paste(txt) } catch (_) {}
}
function doSearch(dir) {
  const t = activeTerm(); const q = searchQuery.value.trim()
  if (!t || !q) return
  const buf = t.buffer.active; const total = buf.length
  const start = buf.viewportY || 0
  for (let step = 1; step <= total; step++) {
    const i = (((start + dir * step) % total) + total) % total
    const line = buf.getLine(i)
    if (line && line.translateToString(true).toLowerCase().includes(q.toLowerCase())) {
      t.scrollToLine(i); searchInfo.value = `行 ${i + 1}`; return
    }
  }
  searchInfo.value = '未找到'
}
async function doSave() {
  const t = activeTerm(); if (!t) return
  const buf = t.buffer.active; const lines = []
  for (let i = 0; i < buf.length; i++) { const l = buf.getLine(i); lines.push(l ? l.translateToString(true) : '') }
  const path = await save({ title: '保存终端内容', defaultPath: 'terminal.log' }).catch(() => null)
  if (!path) return
  try { await invoke('write_file_content', { path, content: lines.join('\n') }); searchInfo.value = '已保存' } catch (_) { searchInfo.value = '保存失败' }
}
function startRecord() { recordBuf.value = ''; recording.value = true }
function stopRecord() {
  recording.value = false
  const name = macroName.value.trim() || ('宏' + (macros.value.length + 1))
  if (recordBuf.value) { macros.value.push({ name, data: recordBuf.value }); macroName.value = '' }
}
function replayMacro(m) {
  const rec = termMap.get(activeSession.value)
  const w = WIRE[rec?.kind || 'ssh'] || WIRE.ssh
  if (activeSession.value) invoke(w.write, { sessionId: activeSession.value, data: m.data }).catch(() => {})
}
function setTermFontSize(v) {
  termFontSize.value = Math.min(28, Math.max(9, v))
  for (const [sid, rec] of termMap) {
    try { rec.term.options.fontSize = termFontSize.value; rec.fit?.fit() } catch (_) {}
    if (rec.kind === 'ssh') invoke('ssh_resize_session', { sessionId: sid, cols: rec.term.cols, rows: rec.term.rows }).catch(() => {})
  }
}
let sideResizing = false
function onSideResizeDown(e) {
  sideResizing = true
  const startX = e.clientX
  const startW = sideWidth.value
  const move = ev => { if (sideResizing) sideWidth.value = Math.min(560, Math.max(240, startW + ev.clientX - startX)) }
  const up = () => { sideResizing = false; document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up) }
  document.addEventListener('mousemove', move)
  document.addEventListener('mouseup', up)
}
const activeSession = ref('')
// 当前激活的 SSH 会话对应的档案 id——驱动 SFTP 面板自动连到同一主机（串口/telnet 无 SFTP）。
const activeProfileId = computed(() => {
  const s = sessions.value.find(x => x.session_id === activeSession.value)
  return s && s.kind === 'ssh' ? (s.profile_id || null) : null
})
const serialPorts = ref([])
const serialPort = ref(null)
const serialBaud = ref(115200)
const telnetHost = ref('')
const telnetPort = ref(23)
const termMap = new Map() // session_id -> { term, fit, el, unlisten, ro }
const termEls = new Map()

function blankProfile() {
  return { id: '', name: '', host: '', port: 22, user: '', auth: 'agent', key_path: '', kind: 'ssh', group: '', serial_path: '', serial_baud: 115200 }
}

function newProfile() {
  Object.assign(editing, blankProfile())
  password.value = ''
  error.value = ''
}

function edit(p) {
  Object.assign(editing, { ...blankProfile(), ...p })
  password.value = ''
}


async function saveProfile() {
  error.value = ''
  if (!editing.host.trim()) { error.value = '主机不能为空'; return null }
  try {
    const saved = await invoke('ssh_save_profile', { profile: { ...editing } })
    if (editing.auth === 'password' && password.value) {
      await invoke('ssh_set_secret', { profileId: saved.id, secret: password.value })
    }
    await loadProfiles()
    Object.assign(editing, saved)
    password.value = ''
    return saved
  } catch (e) {
    error.value = '保存失败: ' + (e?.message || e)
    return null
  }
}

async function remove(p) {
  try { await invoke('ssh_delete_profile', { id: p.id }); await loadProfiles(); if (editing.id === p.id) newProfile() }
  catch (e) { error.value = '删除失败: ' + (e?.message || e) }
}

async function loadRecordings() {
  try { recordings.value = await invoke('ssh_list_recordings') } catch (e) { logger.error('list recordings', e) }
}

function setTermEl(sid, el) {
  if (el) termEls.set(sid, el)
}

async function connect(p) {
  error.value = ''
  try {
    if (p.kind === 'serial') {
      const sid = await invoke('serial_open', { path: p.serial_path, baud: p.serial_baud || 115200 })
      sessions.value.push({ session_id: sid, host: p.name || p.serial_path, kind: 'serial', profile_id: p.id })
      activeSession.value = sid; await nextTick(); await mountTerminal(sid, 'serial')
    } else if (p.kind === 'telnet') {
      const sid = await invoke('telnet_open', { host: p.host, port: p.port || 23 })
      sessions.value.push({ session_id: sid, host: `${p.host}:${p.port}`, kind: 'telnet', profile_id: p.id })
      activeSession.value = sid; await nextTick(); await mountTerminal(sid, 'telnet')
    } else {
      const snap = await invoke('ssh_create_session', { profileId: p.id, cols: 120, rows: 32 })
      sessions.value.push({ session_id: snap.session_id, host: snap.host, kind: 'ssh', profile_id: snap.profile_id || p.id })
      activeSession.value = snap.session_id; await nextTick(); await mountTerminal(snap.session_id, 'ssh')
    }
    loadRecordings()
  } catch (e) {
    error.value = '连接失败: ' + (e?.message || e)
    logger.error('connect failed', e)
  }
}

const WIRE = {
  ssh: { out: 'ssh-session-output', exit: 'ssh-session-exit', write: 'ssh_write_session', close: 'ssh_close_session' },
  serial: { out: 'serial-output', exit: 'serial-exit', write: 'serial_write', close: 'serial_close' },
  telnet: { out: 'telnet-output', exit: 'telnet-exit', write: 'telnet_write', close: 'telnet_close' },
}

// 串口/telnet 也落档到"连接会话"（像 Xshell/MobaXterm 一样可复用、可归类），再按档案连接。
async function saveConnProfile(profile) {
  try {
    const saved = await invoke('ssh_save_profile', { profile })
    await loadProfiles()
    return saved
  } catch (e) { error.value = '保存失败: ' + (e?.message || e); return null }
}

async function connectSerial() {
  error.value = ''
  if (!serialPort.value) { error.value = '请选择串口'; return }
  const prof = { ...blankProfile(), id: editing.id || '', name: editing.name || serialPort.value, kind: 'serial', serial_path: serialPort.value, serial_baud: serialBaud.value, group: editing.group || '' }
  const saved = await saveConnProfile(prof)
  if (!saved) return
  showConnModal.value = false
  await connect(saved)
}

async function connectTelnet() {
  error.value = ''
  if (!telnetHost.value) { error.value = '请输入主机'; return }
  const prof = { ...blankProfile(), id: editing.id || '', name: editing.name || `${telnetHost.value}:${telnetPort.value}`, kind: 'telnet', host: telnetHost.value, port: telnetPort.value, group: editing.group || '' }
  const saved = await saveConnProfile(prof)
  if (!saved) return
  showConnModal.value = false
  await connect(saved)
}

async function loadSerialPorts() {
  try { serialPorts.value = await invoke('serial_list_ports') } catch (e) { error.value = '枚举串口失败: ' + (e?.message || e) }
}

async function mountTerminal(sid, kind) {
  const w = WIRE[kind] || WIRE.ssh
  const el = termEls.get(sid)
  if (!el) return
  const term = new Terminal({
    fontFamily: "'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, 'Courier New', monospace",
    fontSize: termFontSize.value,
    cursorBlink: true,
    theme: termTheme(),
    scrollback: 5000,
  })
  const fit = new FitAddon()
  term.loadAddon(fit)
  term.open(el)
  try { fit.fit() } catch (_) {}

  term.onData(d => {
    invoke(w.write, { sessionId: sid, data: d }).catch(() => {})
    if (recording.value) recordBuf.value += d
  })

  // 选中即复制(MobaXterm 习惯)
  term.onSelectionChange(() => {
    if (!copyOnSelect.value) return
    const sel = term.getSelection()
    if (sel) writeText(sel).catch(() => {})
  })
  // 右键粘贴(走 xterm.paste，自动处理 bracketed-paste，粘进对端 vim 不乱缩进)
  el.addEventListener('contextmenu', ev => { ev.preventDefault(); doPaste() })

  const unlistenOut = await listen(w.out, ev => {
    if (ev.payload?.session_id === sid) term.write(ev.payload.data)
  })
  const unlistenExit = await listen(w.exit, ev => {
    if (ev.payload?.session_id === sid) term.write('\r\n\x1b[33m[连接已关闭]\x1b[0m\r\n')
  })

  let ro = null
  if (kind === 'ssh') {
    ro = new ResizeObserver(() => {
      try {
        fit.fit()
        invoke('ssh_resize_session', { sessionId: sid, cols: term.cols, rows: term.rows }).catch(() => {})
      } catch (_) {}
    })
    ro.observe(el)
    invoke('ssh_resize_session', { sessionId: sid, cols: term.cols, rows: term.rows }).catch(() => {})
  }

  termMap.set(sid, { term, fit, el, ro, kind, unlisten: () => { unlistenOut(); unlistenExit() } })
}

async function closeSession(sid) {
  const rec = termMap.get(sid)
  const w = WIRE[rec?.kind || 'ssh'] || WIRE.ssh
  await invoke(w.close, { sessionId: sid }).catch(() => {})
  if (rec) { rec.ro?.disconnect(); rec.unlisten?.(); rec.term?.dispose(); termMap.delete(sid) }
  termEls.delete(sid)
  sessions.value = sessions.value.filter(s => s.session_id !== sid)
  if (activeSession.value === sid) activeSession.value = sessions.value[0]?.session_id || ''
}

function formatBytes(n) {
  if (n < 1024) return n + ' B'
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB'
  return (n / 1024 / 1024).toFixed(1) + ' MB'
}

// 主题切换时，刷新所有已开终端的配色。
watch(isDark, () => {
  const theme = termTheme()
  for (const [, rec] of termMap) {
    try { rec.term.options.theme = theme } catch (_) {}
  }
})

onMounted(() => { loadRecordings(); loadSerialPorts() })
onBeforeUnmount(() => {
  for (const [sid, rec] of termMap) {
    const w = WIRE[rec?.kind || 'ssh'] || WIRE.ssh
    invoke(w.close, { sessionId: sid }).catch(() => {})
    rec.ro?.disconnect(); rec.unlisten?.(); rec.term?.dispose()
  }
  termMap.clear()
})
</script>

<style scoped>
.ssh-root { height: 100%; display: flex; min-height: 0; }
.ssh-side { width: 320px; flex: 0 0 auto; border-right: 1px solid var(--guaxi-border); display: flex; flex-direction: column; min-height: 0; overflow-y: auto; }
.ssh-side-head { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; font-size: 15px; font-weight: 600; border-bottom: 1px solid var(--guaxi-border); }
.ssh-profiles { flex: 0 0 auto; max-height: 46%; overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 6px; }
.ssh-profile { display: flex; align-items: center; gap: 10px; padding: 9px 10px; border-radius: var(--tw-radius-sm, 7px); border: 1px solid transparent; background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); cursor: pointer; position: relative; transition: background .14s, border-color .14s, box-shadow .14s; }
.ssh-profile:hover { background: var(--tw-accent-soft, rgba(12,166,192,0.12)); border-color: color-mix(in srgb, var(--tw-accent, #0ca6c0) 40%, transparent); box-shadow: 0 1px 6px rgba(2,12,20,0.08); }
.ssh-profile-ico { flex: 0 0 auto; width: 32px; height: 32px; border-radius: 9px; display: flex; align-items: center; justify-content: center; background: linear-gradient(135deg, var(--tw-accent, #0ca6c0), var(--tw-accent-2, #1fbfd6)); color: #fff; }
.ssh-profile-main { flex: 1 1 auto; min-width: 0; }
.ssh-profile-name { font-size: 16px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ssh-profile-sub { font-size: 14px; color: var(--guaxi-text-secondary, #6b7280); font-family: var(--tw-mono-font); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ssh-profile-badge { flex: 0 0 auto; font-size: 13.5px; padding: 2px 8px; border-radius: 999px; background: var(--tw-accent-soft, rgba(12,166,192,0.12)); color: var(--tw-accent, #0ca6c0); }
.ssh-profile-actions { position: absolute; right: 8px; top: 50%; transform: translateY(-50%); display: none; gap: 3px; padding: 3px 4px; border-radius: 8px; background: color-mix(in srgb, var(--guaxi-panel-bg) 88%, transparent); box-shadow: 0 1px 6px rgba(2,12,20,0.16); }
.ssh-profile:hover .ssh-profile-actions { display: inline-flex; }
.ssh-profile:hover .ssh-profile-badge { visibility: hidden; }
.ssh-ico-btn { width: 24px; height: 24px; border: none; border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; display: inline-flex; align-items: center; justify-content: center; transition: background .12s, color .12s; }
.ssh-ico-btn:hover { background: var(--tw-accent-soft, rgba(12,166,192,0.12)); color: var(--tw-accent, #0ca6c0); }
.ssh-ico-btn.primary { color: var(--tw-accent, #0ca6c0); }
.ssh-ico-btn.danger:hover { background: rgba(229,72,77,0.14); color: #e5484d; }
.ssh-empty-side { padding: 12px; font-size: 15px; color: var(--guaxi-text-secondary, #9ca3af); }

/* 连接向导弹窗 */
.ssh-modal { width: 460px; max-width: 92vw; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius, 10px); box-shadow: var(--tw-elev, 0 8px 30px rgba(2,12,20,0.18)); overflow: visible; font-family: var(--tw-ui-font, system-ui, sans-serif); }
.ssh-modal-head { display: flex; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.ssh-modal-close { cursor: pointer; color: var(--guaxi-text-secondary); }
.ssh-modal-close:hover { color: var(--tw-accent, #0ca6c0); }
.ssh-modal-tabs { display: flex; gap: 2px; padding: 10px 14px 0; }
.ssh-mtab { display: inline-flex; align-items: center; gap: 6px; padding: 8px 16px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: var(--tw-radius-sm, 7px) var(--tw-radius-sm, 7px) 0 0; font-size: 15.5px; font-family: var(--tw-ui-font, system-ui, sans-serif); border-bottom: 2px solid transparent; transition: color .14s, border-color .14s; }
.ssh-mtab:hover { color: var(--tw-accent, #0ca6c0); }
.ssh-mtab.active { color: var(--tw-accent, #0ca6c0); border-bottom-color: var(--tw-accent, #0ca6c0); font-weight: 600; }
.ssh-modal-body { padding: 16px 18px; display: flex; flex-direction: column; gap: 12px; border-top: 1px solid var(--guaxi-border); }
.ssh-field { display: flex; flex-direction: column; gap: 5px; }
.ssh-field > label { font-size: 15px; color: var(--guaxi-text-secondary); }
.ssh-modal-foot { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }
.ssh-form { padding: 10px 12px; display: flex; flex-direction: column; gap: 6px; border-top: 1px solid var(--guaxi-border); border-bottom: 1px solid var(--guaxi-border); }
.ssh-form-title { font-size: 15px; font-weight: 600; }
.ssh-row { display: flex; gap: 6px; }
.ssh-hint { font-size: 14px; color: var(--guaxi-text-secondary, #6b7280); }
.ssh-error { font-size: 15px; color: #e5484d; }
.ssh-form-actions { display: flex; gap: 8px; margin-top: 4px; }
.ssh-recordings { flex: 1 1 auto; display: flex; flex-direction: column; min-height: 0; }
.ssh-rec-list { flex: 1 1 auto; overflow-y: auto; }
.ssh-rec { padding: 6px 12px; cursor: pointer; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.05)); }
.ssh-rec:hover { background: var(--guaxi-hover-bg, rgba(0,0,0,0.04)); }
.ssh-rec-name { font-size: 15px; }
.ssh-rec-sub { font-size: 14px; color: var(--guaxi-text-secondary, #6b7280); }

.ssh-main { flex: 1 1 auto; min-width: 0; display: flex; flex-direction: column; min-height: 0; }
.ssh-tabbar { display: flex; gap: 2px; padding: 6px 8px 0; flex: 0 0 auto; overflow-x: auto; }

/* ---- 美化层 ---- */
.ssh-root { font-family: var(--tw-ui-font); }
.ssh-side { overflow-y: auto; scrollbar-width: thin; }
.ssh-resize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; transition: opacity .15s, background .15s; }
.ssh-resize:hover { opacity: 1; background: var(--tw-accent); }
.ssh-side-head { text-transform: none; font-size: 15px; background: color-mix(in srgb, var(--guaxi-panel-bg) 55%, transparent); }
.ssh-profile.active { box-shadow: inset 3px 0 0 var(--tw-accent); background: var(--tw-accent-soft); }
.ssh-form-title { color: var(--tw-accent); letter-spacing: .02em; }
.ssh-port-tag { font-size: 14px; color: var(--guaxi-text-secondary); align-self: center; }
.ssh-tabbar { align-items: center; gap: 8px; border-bottom: 1px solid var(--guaxi-border); padding: 6px 10px; }
.ssh-tabs-list { display: flex; gap: 4px; overflow-x: auto; flex: 1 1 auto; }
.ssh-tab { display: inline-flex; align-items: center; gap: 6px; border-radius: var(--tw-radius-sm) var(--tw-radius-sm) 0 0; border: 1px solid var(--guaxi-border); border-bottom: none; font-family: var(--tw-ui-font); }
.ssh-tab.active { background: var(--tw-code-bg); color: var(--tw-accent); border-color: var(--tw-accent); }
.ssh-tab-close { display: inline-flex; align-items: center; }
.ssh-fontctl { display: inline-flex; align-items: center; gap: 4px; flex: 0 0 auto; padding: 2px 4px; border: 1px solid var(--guaxi-border); border-radius: 999px; }
.ssh-fbtn { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: none; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; border-radius: 50%; }
.ssh-fbtn:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.ssh-fsz { min-width: 20px; text-align: center; font-size: 15px; font-family: var(--tw-mono-font); color: var(--tw-accent); }
.ssh-rec-name { font-family: var(--tw-mono-font); }
.ssh-recordings { flex: 0 0 auto !important; max-height: 26%; }
.ssh-sftp { flex: 1 1 auto; min-height: 120px; border-top: 1px solid var(--guaxi-border); }
.ssh-toolbar { display: flex; align-items: center; gap: 6px; padding: 5px 10px; border-bottom: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); flex: 0 0 auto; }
.ssh-tb-sep { width: 1px; height: 16px; background: var(--guaxi-border); margin: 0 2px; }
.ssh-tb-info { font-size: 15px; color: var(--tw-accent); font-family: var(--tw-mono-font); }
.ssh-macrobar { display: flex; align-items: center; gap: 8px; padding: 5px 10px; border-top: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 45%, transparent); flex: 0 0 auto; overflow-x: auto; }
.ssh-macro-list { display: inline-flex; gap: 6px; }
.ssh-macro-chip { display: inline-flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); font-size: 15px; cursor: pointer; white-space: nowrap; }
.ssh-macro-del { opacity: .6; }
.ssh-macro-del:hover { opacity: 1; color: #e5484d; }
.ssh-tab { padding: 5px 12px; font-size: 15px; border: 1px solid var(--guaxi-border); border-bottom: none; border-radius: 6px 6px 0 0; cursor: pointer; background: var(--guaxi-panel-bg, #f5f5f5); white-space: nowrap; }
.ssh-tab.active { background: var(--tw-code-bg); color: var(--tw-accent); }
.ssh-tab-close { margin-left: 8px; opacity: 0.6; }
.ssh-tab-close:hover { opacity: 1; }
.ssh-term-hosts { flex: 1 1 auto; min-height: 0; position: relative; background: #1e1e1e; }
.ssh-term-host { position: absolute; inset: 0; padding: 4px; }
.ssh-term-empty { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; color: #9ca3af; font-size: 16px; }

/* 折叠 + 亮色适配 */
.ssh-term-hosts { background: var(--tw-code-bg); }
.ssh-head-actions { display: inline-flex; gap: 6px; align-items: center; }
.ssh-collapsed-rail { flex: 0 0 auto; width: 26px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding-top: 12px; border-right: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 55%, transparent); cursor: pointer; color: var(--guaxi-text-secondary); }
.ssh-collapsed-rail:hover { color: var(--tw-accent); background: var(--tw-accent-soft); }
.ssh-rail-text { writing-mode: vertical-rl; font-size: 15px; letter-spacing: 2px; }

/* 连接会话：文件夹分组 + 垂直滚动（Xshell/MobaXterm 风格） */
.ssh-profiles { flex: 1 1 auto !important; max-height: none !important; min-height: 60px; overflow-y: auto; }
.ssh-sec-toggle { display: inline-flex; align-items: center; gap: 4px; cursor: pointer; user-select: none; }
.ssh-sec-toggle:hover { color: var(--tw-accent); }
.ssh-folder { margin-bottom: 4px; }
.ssh-folder-head { display: flex; align-items: center; gap: 5px; padding: 4px 6px; font-size: 15px; cursor: pointer; color: var(--guaxi-text-secondary); border-radius: 6px; }
.ssh-folder-head:hover { background: var(--tw-accent-soft); color: var(--tw-accent); }
.ssh-folder-ico { color: #e0a53f; }
.ssh-folder-name { flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ssh-folder-count { flex: 0 0 auto; font-size: 13.5px; padding: 0 6px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); }
.ssh-folder-items { display: flex; flex-direction: column; gap: 6px; padding: 2px 0 2px 8px; }

/* 右侧栏：会话录制 */
.ssh-side-right { flex: 0 0 auto; display: flex; flex-direction: column; min-height: 0; border-left: 1px solid var(--guaxi-border); overflow: hidden; background: color-mix(in srgb, var(--guaxi-panel-bg) 30%, transparent); }
.ssh-side-right .ssh-rec-list { flex: 1 1 auto; overflow-y: auto; min-height: 0; }
.ssh-recresize { flex: 0 0 auto; width: 5px; cursor: col-resize; background: var(--guaxi-border); opacity: .5; transition: opacity .15s, background .15s; }
.ssh-recresize:hover { opacity: 1; background: var(--tw-accent); }
.ssh-recright-rail { flex: 0 0 auto; width: 26px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding-top: 12px; border-left: 1px solid var(--guaxi-border); background: color-mix(in srgb, var(--guaxi-panel-bg) 55%, transparent); cursor: pointer; color: var(--guaxi-text-secondary); }
.ssh-recright-rail:hover { color: var(--tw-accent); background: var(--tw-accent-soft); }
</style>
