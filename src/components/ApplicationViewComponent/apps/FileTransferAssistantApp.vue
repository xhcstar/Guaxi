<template>
  <div class="ftc-root">
    <header class="ftc-top">
      <div class="ftc-title">
        <span class="ftc-mark" aria-hidden="true">⇄</span>
        <div>
          <h1>传输</h1>
          <p>局域网直连 · 断点续传 · 文件夹</p>
        </div>
      </div>
      <div class="ftc-top-actions">
        <button type="button" class="ftc-presence" :class="{ 'is-on': discoverable }" @click="toggleDiscoverable">
          <span class="ftc-dot" />
          {{ discoverable ? '可被发现' : '开启发现' }}
        </button>
        <button type="button" class="ftc-ghost" @click="showAdvanced = true">高级设置</button>
      </div>
    </header>

    <div v-if="transferNotice" class="ftc-notice">
      <span>{{ transferNotice }}</span>
      <button type="button" @click="transferNotice = ''">×</button>
    </div>

    <div class="ftc-layout">
      <aside class="ftc-devices">
        <div class="ftc-section-title">附近设备 <span>{{ peers.length }}</span></div>
        <div class="ftc-device-list">
          <button
            v-for="peer in peers"
            :key="peer.peerId"
            type="button"
            class="ftc-device"
            :class="{ 'is-selected': selectedPeerId === peer.peerId }"
            @click="selectPeer(peer)"
          >
            <span class="ftc-avatar">{{ peerInitials(peer.name) }}</span>
            <span class="ftc-device-info">
              <strong>{{ peer.name }}</strong>
              <small>{{ peer.addr }} · {{ formatLastSeen(peer.lastSeenMs) }}</small>
            </span>
            <span v-if="peer.requiresToken" class="ftc-lock">配对码</span>
            <span v-if="selectedPeerId === peer.peerId" class="ftc-selected" aria-hidden="true">●</span>
          </button>
          <div v-if="!peers.length" class="ftc-empty-mini">
            还没有发现设备
            <small>双方打开「可被发现」即可</small>
          </div>
        </div>

        <div class="ftc-section-title">P2P 已配对 <span>{{ p2pPeers.length }}</span></div>
        <div class="ftc-device-list">
          <div v-for="peer in p2pPeers" :key="peer.peer_id" class="ftc-device is-static" :title="'P2P 用于目录同步，不用于临时文件'">
            <span class="ftc-avatar is-p2p">P2P</span>
            <span class="ftc-device-info">
              <strong>{{ peer.device_name || '未命名设备' }}</strong>
              <small>{{ p2pStatusLabel(peer.online_status) }} · {{ String(peer.peer_id || '').slice(0, 8) }}</small>
            </span>
            <button type="button" class="ftc-mini" :disabled="!!p2pBusyId" @click="p2pSend(peer, false)">发送</button>
            <button type="button" class="ftc-mini" :disabled="!!p2pBusyId" @click="p2pSend(peer, true)">文件夹</button>
            <button type="button" class="ftc-mini" :disabled="!!p2pBusyId" @click="syncP2PResourceRepo(peer)">同步</button>
          </div>
          <div v-if="!p2pPeers.length" class="ftc-empty-mini">还没有 P2P 配对设备</div>
          <div v-if="p2pProgress && p2pProgress.status === 'running'" class="ftc-p2p-progress">
            <span class="ftc-p2p-name">{{ p2pProgress.file || p2pProgress.device_name }}</span>
            <span class="ftc-task-bar">
              <span :style="{ width: `${percent(p2pProgress.bytes_current, p2pProgress.bytes_total)}%` }" />
            </span>
            <small>{{ formatBytes(p2pProgress.bytes_current) }} / {{ formatBytes(p2pProgress.bytes_total) }}</small>
          </div>
        </div>

        <label class="ftc-token">
          <span>对方配对码</span>
          <input v-model="peerToken" class="ftc-input" placeholder="设备需要时填写" />
        </label>

        <div class="ftc-section-title">远程</div>
        <button type="button" class="ftc-device is-remote" @click="showRemote = true">
          <span class="ftc-avatar is-remote">#</span>
          <span class="ftc-device-info">
            <strong>房间码 / 地址</strong>
            <small>{{ webrtcSignalJoined ? `已加入 ${webrtcRoomCode}` : '信令房间或 HTTP 直连' }}</small>
          </span>
        </button>

        <div class="ftc-device-foot">
          <div class="ftc-addr-head">
            <span>本机地址</span>
            <em :class="{ 'is-on': receiverStatus?.running }">
              {{ receiverStatus?.running ? `HTTP 已开启 · ${receiverPortValue}` : 'HTTP 未开启' }}
            </em>
          </div>
          <div v-if="!localIps.length" class="ftc-empty-mini">未发现可用网卡</div>
          <button
            v-for="ip in localIps"
            :key="`${ip.name}-${ip.address}`"
            type="button"
            class="ftc-addr"
            :class="{ 'is-active': selectedLocalIp === ip.address }"
            title="在系统浏览器中打开"
            @click="openLocalIp(ip)"
          >
            <span class="ftc-addr-ip">{{ ip.address }}</span>
            <span class="ftc-addr-name">{{ ip.name }}</span>
          </button>
          <div class="ftc-addr-actions">
            <button type="button" @click="toggleHttpServer">
              {{ receiverStatus?.running ? '停止 HTTP 服务' : '开启 HTTP 服务' }}
            </button>
            <button type="button" :disabled="!selectedLocalUrl" @click="copyMyUrl">复制地址</button>
          </div>
        </div>
      </aside>

      <main class="ftc-main">
        <section class="ftc-drop" :class="{ 'is-active': dropActive, 'has-target': selectedPeer }">
          <div class="ftc-drop-glyph" aria-hidden="true">↓</div>
          <h2>{{ targetLabel }}</h2>
          <p>{{ selectedPeer ? '拖入文件或文件夹，或从本机选择' : '拖入文件或文件夹，然后点右侧设备发送' }}</p>
          <div class="ftc-drop-actions">
            <button type="button" class="ftc-primary" @click="pickAndSend('files')">选择文件</button>
            <button type="button" @click="pickAndSend('folder')">选择文件夹</button>
          </div>
          <small>文件夹会保留目录结构；同名文件自动改名，不覆盖。</small>
          <div v-if="pendingPaths.length" class="ftc-pending">
            已选择 {{ pendingPaths.length }} 项，点右侧设备发送
          </div>
        </section>

        <section class="ftc-queue">
          <div class="ftc-section-head">
            <h3>传输队列</h3>
            <div>
              <span>{{ activeTasks.length }} 进行中</span>
              <button type="button" @click="clearFinished">清除已完成</button>
            </div>
          </div>

          <div v-if="!tasks.length" class="ftc-empty">还没有传输任务</div>

          <div v-for="task in visibleTasks" :key="task.id" class="ftc-task" :data-status="task.status">
            <span class="ftc-task-dir" :data-direction="task.direction">{{ task.direction === 'send' ? '↑' : '↓' }}</span>
            <div class="ftc-task-main">
              <div class="ftc-task-title">
                <span class="ftc-task-name" :title="task.fileName">{{ task.fileName }}</span>
                <span class="ftc-task-status">{{ taskStatusLabel(task.status) }}</span>
              </div>
              <div class="ftc-task-sub">
                {{ task.peer }} · {{ formatBytes(task.transferredBytes) }} / {{ formatBytes(task.totalBytes) }}
                <span v-if="task.error" class="ftc-error"> · {{ task.error }}</span>
              </div>
              <div class="ftc-task-bar">
                <span :style="{ width: `${percent(task.transferredBytes, task.totalBytes)}%` }" />
              </div>
            </div>
            <div class="ftc-task-actions">
              <button v-if="task.status === 'running'" type="button" @click="pauseTask(task.id)">暂停</button>
              <button v-else-if="task.status === 'paused'" type="button" @click="resumeTask(task.id)">继续</button>
              <button
                v-if="['error', 'interrupted', 'canceled'].includes(task.status)"
                type="button"
                @click="retryTask(task.id)"
              >
                重试
              </button>
              <button
                v-if="!['done', 'canceled', 'interrupted'].includes(task.status)"
                type="button"
                class="is-danger"
                @click="cancelTask(task.id)"
              >
                取消
              </button>
            </div>
          </div>
        </section>
      </main>
    </div>

    <n-drawer v-model:show="showAdvanced" :width="440" placement="right">
      <n-drawer-content title="高级设置" closable>
        <div class="ftc-adv">
          <h4>接收服务</h4>
          <div class="ftc-row">
            <label class="ftc-field"><span>监听地址</span><input v-model="receiverBindAddr" class="ftc-input" /></label>
            <label class="ftc-field is-narrow"><span>端口</span><input v-model.number="receiverPort" type="number" class="ftc-input" /></label>
          </div>
          <label class="ftc-field">
            <span>保存目录</span>
            <div class="ftc-pick">
              <input v-model="receiverSaveDir" class="ftc-input" placeholder="留空使用默认 incoming 目录" />
              <button type="button" @click="pickReceiverSaveDir">选择</button>
            </div>
          </label>
          <label class="ftc-field">
            <span>配对码（可选）</span>
            <input v-model="receiverToken" class="ftc-input" placeholder="留空则允许局域网直接发送" />
          </label>
          <div class="ftc-actions">
            <button type="button" :disabled="!!receiverStatus?.running" @click="startReceiver">启动接收</button>
            <button type="button" :disabled="!receiverStatus?.running" @click="stopReceiver">停止</button>
            <span class="ftc-mono">
              {{ receiverStatus?.running ? `监听 ${receiverStatus.bindAddr}:${receiverStatus.port}（所有网卡）` : '未启动' }}
            </span>
          </div>

          <h4>局域网发现</h4>
          <label class="ftc-field is-narrow">
            <span>发现端口</span>
            <input v-model.number="discoveryPort" type="number" class="ftc-input" />
          </label>
          <div class="ftc-actions">
            <button type="button" :disabled="discoveryRunning" @click="startDiscovery">开启发现</button>
            <button type="button" :disabled="!discoveryRunning" @click="stopDiscovery">关闭发现</button>
          </div>

          <h4>HTTP 目录分享</h4>
          <div class="ftc-row">
            <label class="ftc-field"><span>监听地址</span><input v-model="shareBindAddr" class="ftc-input" /></label>
            <label class="ftc-field is-narrow"><span>端口</span><input v-model.number="sharePort" type="number" class="ftc-input" /></label>
          </div>
          <label class="ftc-field">
            <span>分享目录</span>
            <div class="ftc-pick">
              <input v-model="shareRootDir" class="ftc-input" placeholder="选择要作为 HTTP 根目录的文件夹" />
              <button type="button" @click="pickShareRoot">选择</button>
            </div>
          </label>
          <div class="ftc-row">
            <label class="ftc-field">
              <span>上传配对码（可选）</span>
              <input v-model="shareToken" class="ftc-input" placeholder="关闭只读后，PUT 需要携带" />
            </label>
            <label class="ftc-check">
              <n-switch v-model:value="shareReadOnly" size="small" />
              只读
            </label>
          </div>
          <div class="ftc-actions">
            <button type="button" :disabled="!!httpShareStatus?.running" @click="startHttpShare">启动分享</button>
            <button type="button" :disabled="!httpShareStatus?.running" @click="stopHttpShare">停止</button>
            <span class="ftc-mono">{{ httpShareStatus?.baseUrl || '未启动' }}</span>
          </div>

          <h4>WebRTC</h4>
          <label class="ftc-field">
            <span>信令服务</span>
            <span class="ftc-mono">{{ webrtcSignalUrl || '未配置（设置 → 外观与系统）' }}</span>
          </label>
          <label class="ftc-field">
            <span>STUN servers（每行一个）</span>
            <textarea v-model="webrtcIceServersText" class="ftc-input is-area" rows="3" />
          </label>
          <div class="ftc-row">
            <label class="ftc-field"><span>房间码</span><input v-model="webrtcRoomCode" class="ftc-input" /></label>
            <div class="ftc-actions">
              <button type="button" @click="webrtcGenerateRoomCode">生成</button>
              <button type="button" :disabled="!webrtcCanJoinRoom" @click="webrtcJoinRoom">加入</button>
              <button type="button" :disabled="!webrtcSignalJoined" @click="webrtcLeaveRoom">退出</button>
            </div>
          </div>
          <div class="ftc-actions is-muted">
            <span>{{ webrtcSignalText }}</span>
            <span>{{ webrtcConnStateText }}</span>
            <span v-if="webrtcTargetPeerId">对端 {{ webrtcTargetPeerId }}</span>
          </div>
          <div class="ftc-actions">
            <button type="button" :disabled="!webrtcCanSend" @click="webrtcPickAndSendFiles">通过 WebRTC 发送</button>
            <button type="button" :disabled="!webrtcCanSend" @click="webrtcPickAndSendFolder">发送文件夹</button>
            <button type="button" @click="webrtcClose">断开连接</button>
          </div>
          <p v-if="webrtcLastError" class="ftc-error">{{ webrtcLastError }}</p>

          <h4>任务记录</h4>
          <div class="ftc-actions">
            <button type="button" @click="clearFinished">清除已完成 / 已取消</button>
            <span class="ftc-muted">记录最多保留 300 条</span>
          </div>
        </div>
      </n-drawer-content>
    </n-drawer>

    <n-modal v-model:show="showRemote" preset="card" class="ftc-modal" title="远程设备" :bordered="false">
      <div class="ftc-adv">
        <h4>HTTP 直连</h4>
        <label class="ftc-field">
          <span>对方地址</span>
          <input v-model="customPeerUrl" class="ftc-input" placeholder="http://203.0.113.10:39190" />
        </label>
        <label class="ftc-field">
          <span>对方配对码（可选）</span>
          <input v-model="peerToken" class="ftc-input" />
        </label>
        <div class="ftc-actions">
          <button type="button" class="ftc-primary" @click="sendFilesToCustomUrl">选择文件发送</button>
          <button type="button" @click="sendFolderToCustomUrl">选择文件夹发送</button>
        </div>

        <h4>WebRTC 房间码</h4>
        <label class="ftc-field">
          <span>信令</span>
          <span class="ftc-mono">{{ webrtcSignalUrl || '未配置' }}</span>
        </label>
        <div class="ftc-row">
          <label class="ftc-field"><span>房间码</span><input v-model="webrtcRoomCode" class="ftc-input" /></label>
          <div class="ftc-actions">
            <button type="button" @click="webrtcGenerateRoomCode">生成</button>
            <button type="button" :disabled="!webrtcCanJoinRoom" @click="webrtcJoinRoom">加入</button>
            <button type="button" :disabled="!webrtcSignalJoined" @click="webrtcLeaveRoom">退出</button>
          </div>
        </div>
        <div class="ftc-actions is-muted">
          <span>{{ webrtcSignalText }}</span>
          <span>{{ webrtcConnStateText }}</span>
        </div>
        <div class="ftc-actions">
          <button type="button" :disabled="!webrtcCanSend" @click="webrtcPickAndSendFiles">选择文件发送</button>
          <button type="button" :disabled="!webrtcCanSend" @click="webrtcPickAndSendFolder">选择文件夹发送</button>
          <button type="button" @click="webrtcClose">断开</button>
        </div>
        <p v-if="webrtcLastError" class="ftc-error">{{ webrtcLastError }}</p>
      </div>
    </n-modal>
  </div>
</template>

<script setup>

import { createLogger } from '../../../utils/logger'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { openUrl } from '@tauri-apps/plugin-opener'
import { getCurrentWindow } from '@tauri-apps/api/window'

const logger = createLogger('FileTransferAssistantApp')

const { message } = createDiscreteApi(['message'])

const TRANSFER_PREFS_KEY = 'guaxi:filetransfer:prefs:v1'

const receiverBindAddr = ref('0.0.0.0')
const receiverPort = ref(39190)
const receiverSaveDir = ref('')
const receiverToken = ref('')
const peerToken = ref('')

const discoveryPort = ref(39191)
const customPeerUrl = ref('')

const shareBindAddr = ref('0.0.0.0')
const sharePort = ref(39200)
const shareRootDir = ref('')
const shareToken = ref('')
const shareReadOnly = ref(true)

const receiverStatus = ref(null)
const httpShareStatus = ref(null)
const discoveryRunning = ref(false)
const localIps = ref([])
const selectedLocalIp = ref('')

const peers = ref([])
const tasks = ref([])
const p2pPeers = ref([])
const p2pBusyId = ref('')
const p2pProgress = ref(null)

const selectedPeerId = ref('')
const pendingPaths = ref([])
const dropActive = ref(false)
const showAdvanced = ref(false)
const showRemote = ref(false)
const transferNotice = ref('')

const selectedPeer = computed(() => peers.value.find((peer) => peer.peerId === selectedPeerId.value) || null)
const discoverable = computed(() => Boolean(receiverStatus.value?.running) && discoveryRunning.value)
const activeTasks = computed(() => tasks.value.filter((task) => task.status === 'running' || task.status === 'paused'))
const visibleTasks = computed(() => tasks.value.slice(0, 60))
const targetLabel = computed(() => (selectedPeer.value ? `发送到 ${selectedPeer.value.name}` : '先选一个设备'))
const receiverPortValue = computed(() => receiverStatus.value?.port || receiverPort.value)
const selectedLocalUrl = computed(() => {
  const ip = localIps.value.find((item) => item.address === selectedLocalIp.value)
  if (!ip) return ''
  const host = ip.family === 'ipv6' ? `[${ip.address}]` : ip.address
  return `http://${host}:${receiverPortValue.value}`
})

const webrtcIceServersText = ref('stun:stun.cloudflare.com:3478\nstun:stun.l.google.com:19302')
const webrtcLocalSdp = ref('')
const webrtcRemoteSdp = ref('')
const webrtcConnState = ref('new')
const webrtcBusy = ref(false)
const webrtcLastError = ref('')

const webrtcSignalUrl = ref('')
const webrtcSignalToken = ref('')
const webrtcRoomCode = ref('')
const webrtcClientId = ref('')
const webrtcTargetPeerId = ref('')
const webrtcSignalJoined = ref(false)
const webrtcSignalConn = ref('idle')

let webrtcSignalWs = null
let webrtcSignalRoom = ''

let webrtcPc = null
let webrtcDc = null

let unlistenTaskUpdate = null
let unlistenPeersUpdate = null
let unlistenDrop = null
let unlistenP2pProgress = null

const percent = (done, total) => {
  const d = Number(done || 0)
  const t = Number(total || 0)
  if (!t) return 0
  return Math.max(0, Math.min(100, Math.round((d / t) * 1000) / 10))
}

const formatBytes = (v) => {
  const n = Number(v || 0)
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let x = n
  let i = 0
  while (x >= 1024 && i < units.length - 1) {
    x /= 1024
    i += 1
  }
  return `${x.toFixed(i === 0 ? 0 : 2)} ${units[i]}`
}

const formatLastSeen = (ms) => {
  const n = Number(ms || 0)
  if (!n) return '-'
  const diff = Date.now() - n
  if (diff < 1000) return '刚刚'
  if (diff < 60_000) return `${Math.floor(diff / 1000)}s`
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m`
  return `${Math.floor(diff / 3_600_000)}h`
}

const tagTypeForTask = (t) => {
  const s = String(t?.status || '')
  if (s === 'done') return 'success'
  if (s === 'error') return 'error'
  if (s === 'paused') return 'warning'
  if (s === 'running') return 'info'
  return 'default'
}

const refreshAll = async () => {
  await Promise.allSettled([refreshStatus(), refreshPeers(), refreshTasks(), refreshP2PPeers(), loadLocalIps()])
}

const loadLocalIps = async () => {
  try {
    const list = await invoke('filetransfer_list_local_ips')
    localIps.value = Array.isArray(list) ? list : []
    if (!localIps.value.some((item) => item.address === selectedLocalIp.value)) {
      selectedLocalIp.value = (
        localIps.value.find((item) => !item.isLoopback) || localIps.value[0]
      )?.address || ''
    }
  } catch (error) {
    logger.warn('[FTA] load local ips failed:', error)
    localIps.value = []
  }
}

const refreshP2PPeers = async () => {
  try {
    const list = await invoke('p2p_sync_list_peers')
    p2pPeers.value = Array.isArray(list) ? list : []
  } catch (error) {
    logger.warn('[FTA] load p2p peers failed:', error)
  }
}

const p2pSend = async (peer, folder = false) => {
  if (p2pBusyId.value) return
  try {
    const picked = await open({ multiple: !folder, directory: folder })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (!list.length) return
    const entries = await invoke('filetransfer_collect_entries', { paths: list })
    if (!entries.length) return
    p2pBusyId.value = peer.peer_id
    p2pProgress.value = null
    transferNotice.value = `正在通过 P2P 发送 ${entries.length} 个文件…`
    const message = await invoke('p2p_send_files', {
      peerId: peer.peer_id,
      files: entries.map((entry) => ({ path: entry.path, relative_path: entry.relative_path })),
    })
    transferNotice.value = String(message || 'P2P 发送完成')
  } catch (error) {
    logger.error('[FTA] p2p send failed:', error)
    transferNotice.value = String(error?.message || error || 'P2P 发送失败')
  } finally {
    p2pBusyId.value = ''
  }
}

const syncP2PResourceRepo = async (peer) => {
  const id = peer?.peer_id || 'all'
  if (p2pBusyId.value) return
  p2pBusyId.value = id
  transferNotice.value = `正在向已配对设备同步资源库…`
  try {
    const text = await invoke('p2p_sync_resource_repo_now')
    transferNotice.value = String(text || 'P2P 资源库同步完成')
    await refreshP2PPeers()
  } catch (error) {
    logger.error('[FTA] p2p sync failed:', error)
    transferNotice.value = String(error?.message || error || 'P2P 同步失败')
  } finally {
    p2pBusyId.value = ''
  }
}

const refreshStatus = async () => {
  try {
    const st = await invoke('filetransfer_get_status')
    receiverStatus.value = st?.receiver || null
    httpShareStatus.value = st?.httpShare || null
    discoveryRunning.value = !!st?.discoveryRunning
  } catch (e) {
    logger.error('[FTA] refreshStatus failed:', e)
  }
}

const refreshPeers = async () => {
  try {
    const list = await invoke('filetransfer_get_peers')
    peers.value = Array.isArray(list) ? list : []
  } catch (e) {
    logger.error('[FTA] refreshPeers failed:', e)
  }
}

const refreshTasks = async () => {
  try {
    const list = await invoke('filetransfer_list_tasks')
    tasks.value = Array.isArray(list) ? list : []
  } catch (e) {
    logger.error('[FTA] refreshTasks failed:', e)
  }
}

const loadTransferPrefs = () => {
  try {
    const raw = localStorage.getItem(TRANSFER_PREFS_KEY)
    if (!raw) return
    const value = JSON.parse(raw)
    if (typeof value?.receiverBindAddr === 'string') receiverBindAddr.value = value.receiverBindAddr
    if (Number(value?.receiverPort) > 0) receiverPort.value = Number(value.receiverPort)
    if (typeof value?.receiverSaveDir === 'string') receiverSaveDir.value = value.receiverSaveDir
    if (Number(value?.discoveryPort) > 0) discoveryPort.value = Number(value.discoveryPort)
    if (typeof value?.shareBindAddr === 'string') shareBindAddr.value = value.shareBindAddr
    if (Number(value?.sharePort) > 0) sharePort.value = Number(value.sharePort)
    if (typeof value?.shareRootDir === 'string') shareRootDir.value = value.shareRootDir
    if (typeof value?.shareReadOnly === 'boolean') shareReadOnly.value = value.shareReadOnly
    if (typeof value?.selectedLocalIp === 'string') selectedLocalIp.value = value.selectedLocalIp
  } catch (error) {
    logger.warn('[FTA] load prefs failed:', error)
  }
}

let savePrefsTimer = null
const scheduleSaveTransferPrefs = () => {
  clearTimeout(savePrefsTimer)
  savePrefsTimer = setTimeout(() => {
    try {
      localStorage.setItem(TRANSFER_PREFS_KEY, JSON.stringify({
        receiverBindAddr: receiverBindAddr.value,
        receiverPort: receiverPort.value,
        receiverSaveDir: receiverSaveDir.value,
        discoveryPort: discoveryPort.value,
        shareBindAddr: shareBindAddr.value,
        sharePort: sharePort.value,
        shareRootDir: shareRootDir.value,
        shareReadOnly: shareReadOnly.value,
        selectedLocalIp: selectedLocalIp.value,
      }))
    } catch (error) {
      logger.warn('[FTA] save prefs failed:', error)
    }
  }, 400)
}

watch(
  [receiverBindAddr, receiverPort, receiverSaveDir, discoveryPort, shareBindAddr, sharePort, shareRootDir, shareReadOnly, selectedLocalIp],
  scheduleSaveTransferPrefs
)

const peerBaseUrl = (peer) => normalizeBaseUrl(`http://${peer.addr}:${peer.port}`)
const peerInitials = (name) => String(name || '?').replace(/\s+/g, '').slice(0, 2).toUpperCase()
const p2pStatusLabel = (status) => ({
  online: '在线',
  offline: '离线',
  unknown: '未知',
}[String(status || 'unknown')] || '未知')

const taskStatusLabel = (status) => ({
  running: '传输中',
  paused: '已暂停',
  done: '已完成',
  error: '失败',
  canceled: '已取消',
  interrupted: '已中断',
}[status] || status)

const sendPathsTo = async (peer, paths) => {
  if (!peer) {
    transferNotice.value = '先选择一个接收设备'
    return
  }
  const list = (paths || []).filter(Boolean)
  if (!list.length) {
    transferNotice.value = '没有可发送的文件'
    return
  }
  try {
    const entries = await invoke('filetransfer_collect_entries', { paths: list })
    if (!Array.isArray(entries) || !entries.length) {
      transferNotice.value = '没有可发送的文件'
      return
    }
    await invoke('filetransfer_send_entries', {
      peerUrl: peerBaseUrl(peer),
      entries,
      accessToken: peerToken.value.trim() || null,
    })
    pendingPaths.value = []
    transferNotice.value = `已加入队列：${entries.length} 个文件 → ${peer.name}`
    await refreshTasks()
  } catch (error) {
    logger.error('[FTA] send paths failed:', error)
    transferNotice.value = String(error?.message || error || '发送失败')
  }
}

const selectPeer = async (peer) => {
  selectedPeerId.value = peer.peerId
  if (pendingPaths.value.length) {
    await sendPathsTo(peer, pendingPaths.value)
  }
}

const pickAndSend = async (kind) => {
  try {
    const folder = kind === 'folder'
    const picked = await open({ multiple: !folder, directory: folder })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (!list.length) return
    if (!selectedPeer.value) {
      pendingPaths.value = list
      transferNotice.value = `已选择 ${list.length} 项，点右侧设备发送`
      return
    }
    await sendPathsTo(selectedPeer.value, list)
  } catch (error) {
    logger.error('[FTA] pick and send failed:', error)
    transferNotice.value = String(error?.message || error || '选择文件失败')
  }
}

const toggleDiscoverable = async () => {
  if (discoverable.value) {
    await stopDiscovery()
    await stopReceiver()
    await refreshAll()
    return
  }
  if (!receiverStatus.value?.running) await startReceiver()
  if (!discoveryRunning.value) await startDiscovery()
  await refreshAll()
}

const clearFinished = async () => {
  try {
    await invoke('filetransfer_clear_finished')
    await refreshTasks()
    transferNotice.value = '已清除完成和取消的记录'
  } catch (error) {
    logger.error('[FTA] clear finished failed:', error)
  }
}

const toggleHttpServer = async () => {
  if (receiverStatus.value?.running) {
    await stopReceiver()
  } else {
    await startReceiver()
  }
  await refreshAll()
}

const openLocalIp = async (ip) => {
  selectedLocalIp.value = ip.address
  if (!receiverStatus.value?.running) {
    transferNotice.value = '先开启 HTTP 服务，再在浏览器打开'
    return
  }
  const url = selectedLocalUrl.value
  if (!url) {
    transferNotice.value = '没有可用的本机地址'
    return
  }
  try {
    await openUrl(url)
  } catch (error) {
    logger.error('[FTA] open local url failed:', error)
    transferNotice.value = String(error?.message || error || '打开浏览器失败')
  }
}

const copyMyUrl = async () => {
  if (!receiverStatus.value?.running) {
    transferNotice.value = '先开启 HTTP 服务，再复制地址'
    return
  }
  if (!selectedLocalUrl.value) {
    transferNotice.value = '没有可用的本机地址'
    return
  }
  await copyText(selectedLocalUrl.value)
}

const sendToCustomUrl = async (folder) => {
  try {
    const baseUrl = normalizeBaseUrl(customPeerUrl.value)
    if (!baseUrl) {
      message.warning('请先填写对方地址')
      return
    }
    const picked = await open({ multiple: !folder, directory: folder })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (!list.length) return
    const entries = await invoke('filetransfer_collect_entries', { paths: list })
    await invoke('filetransfer_send_entries', {
      peerUrl: baseUrl,
      entries,
      accessToken: peerToken.value.trim() || null,
    })
    transferNotice.value = `已加入队列：${entries.length} 个文件 → ${baseUrl}`
    await refreshTasks()
  } catch (error) {
    logger.error('[FTA] send to custom url failed:', error)
    message.error(String(error?.message || error || '发送失败'))
  }
}

const sendFolderToCustomUrl = () => sendToCustomUrl(true)

const setupDropTarget = async () => {
  try {
    unlistenDrop = await getCurrentWindow().onDragDropEvent(async (event) => {
      const payload = event?.payload || {}
      if (payload.type === 'enter' || payload.type === 'over') {
        dropActive.value = true
        return
      }
      if (payload.type === 'leave') {
        dropActive.value = false
        return
      }
      if (payload.type !== 'drop') return
      dropActive.value = false
      const paths = Array.isArray(payload.paths) ? payload.paths : []
      if (!paths.length) return
      if (selectedPeer.value) await sendPathsTo(selectedPeer.value, paths)
      else {
        pendingPaths.value = paths
        transferNotice.value = `已选择 ${paths.length} 项，点右侧设备发送`
      }
    })
  } catch (error) {
    logger.warn('[FTA] drag drop listener failed:', error)
  }
}

const pickReceiverSaveDir = async () => {
  try {
    const p = await open({ directory: true, multiple: false })
    if (typeof p === 'string' && p.trim()) receiverSaveDir.value = p
  } catch (e) {
    logger.error('[FTA] pickReceiverSaveDir failed:', e)
  }
}

const startReceiver = async () => {
  try {
    await invoke('filetransfer_start_receiver', {
      bindAddr: receiverBindAddr.value,
      port: receiverPort.value,
      saveDir: receiverSaveDir.value || null,
      accessToken: receiverToken.value.trim() || null,
    })
    message.success('接收服务已启动')
    await refreshStatus()
  } catch (e) {
    logger.error('[FTA] startReceiver failed:', e)
    message.error(String(e?.message || e || '启动失败'))
  }
}

const stopReceiver = async () => {
  try {
    await invoke('filetransfer_stop_receiver')
    message.success('接收服务已停止')
    await refreshStatus()
  } catch (e) {
    logger.error('[FTA] stopReceiver failed:', e)
    message.error(String(e?.message || e || '停止失败'))
  }
}

const startDiscovery = async () => {
  try {
    await invoke('filetransfer_start_lan_discovery', { discoveryPort: discoveryPort.value })
    message.success('局域网发现已开启')
    await refreshStatus()
    await refreshPeers()
  } catch (e) {
    logger.error('[FTA] startDiscovery failed:', e)
    message.error(String(e?.message || e || '开启失败'))
  }
}

const stopDiscovery = async () => {
  try {
    await invoke('filetransfer_stop_lan_discovery')
    message.success('局域网发现已关闭')
    await refreshStatus()
  } catch (e) {
    logger.error('[FTA] stopDiscovery failed:', e)
    message.error(String(e?.message || e || '关闭失败'))
  }
}

const pickShareRoot = async () => {
  try {
    const p = await open({ directory: true, multiple: false })
    if (typeof p === 'string' && p.trim()) shareRootDir.value = p
  } catch (e) {
    logger.error('[FTA] pickShareRoot failed:', e)
  }
}

const startHttpShare = async () => {
  try {
    await invoke('filetransfer_start_http_share', {
      bindAddr: shareBindAddr.value,
      port: sharePort.value,
      rootDir: shareRootDir.value,
      accessToken: shareToken.value.trim() || null,
      readOnly: shareReadOnly.value,
    })
    message.success('HTTP 分享已启动')
    await refreshStatus()
  } catch (e) {
    logger.error('[FTA] startHttpShare failed:', e)
    message.error(String(e?.message || e || '启动失败'))
  }
}

const stopHttpShare = async () => {
  try {
    await invoke('filetransfer_stop_http_share')
    message.success('HTTP 分享已停止')
    await refreshStatus()
  } catch (e) {
    logger.error('[FTA] stopHttpShare failed:', e)
    message.error(String(e?.message || e || '停止失败'))
  }
}

const normalizeBaseUrl = (s) => {
  const raw = String(s || '').trim()
  if (!raw) return ''
  if (raw.startsWith('http://') || raw.startsWith('https://')) return raw.replace(/\/+$/, '')
  return `http://${raw}`.replace(/\/+$/, '')
}

const sendFilesToPeer = async (peer) => {
  try {
    const baseUrl = normalizeBaseUrl(`http://${peer.addr}:${peer.port}`)
    const picked = await open({ multiple: true, directory: false })
    const paths = Array.isArray(picked) ? picked : (typeof picked === 'string' ? [picked] : [])
    const list = paths.map(x => String(x || '').trim()).filter(Boolean)
    if (!list.length) return

    await invoke('filetransfer_send_files', { peerUrl: baseUrl, filePaths: list })
    message.success('已创建发送任务')
    await refreshTasks()
  } catch (e) {
    logger.error('[FTA] sendFilesToPeer failed:', e)
    message.error(String(e?.message || e || '发送失败'))
  }
}

const sendFilesToCustomUrl = () => sendToCustomUrl(false)

const retryTask = async (taskId) => {
  try {
    await invoke('filetransfer_retry_task', {
      taskId,
      accessToken: peerToken.value.trim() || null,
    })
    await refreshTasks()
    transferNotice.value = '已重新加入传输队列'
  } catch (error) {
    logger.error('[FTA] retry task failed:', error)
    transferNotice.value = String(error?.message || error || '重试失败')
  }
}

const copyPeerUrl = async (peer) => {
  try {
    const url = `http://${peer.addr}:${peer.port}`
    await navigator.clipboard.writeText(url)
    message.success('已复制')
  } catch (e) {
    logger.error('[FTA] copyPeerUrl failed:', e)
    message.error('复制失败')
  }
}

const pauseTask = async (taskId) => {
  try {
    await invoke('filetransfer_pause_task', { taskId })
  } catch (e) {
    logger.error('[FTA] pauseTask failed:', e)
  }
}

const resumeTask = async (taskId) => {
  try {
    await invoke('filetransfer_resume_task', { taskId })
  } catch (e) {
    logger.error('[FTA] resumeTask failed:', e)
  }
}

const cancelTask = async (taskId) => {
  try {
    await invoke('filetransfer_cancel_task', { taskId })
  } catch (e) {
    logger.error('[FTA] cancelTask failed:', e)
  }
}

const copyText = async (text) => {
  try {
    const s = String(text || '')
    if (!s) return
    await navigator.clipboard.writeText(s)
    message.success('已复制')
  } catch (e) {
    logger.error('[FTA] copyText failed:', e)
    message.error('复制失败')
  }
}

const normalizeSignalUrl = (raw) => {
  let s = String(raw || '').trim().replace(/\/+$/, '')
  if (!s) return ''

  if (s.startsWith('http://')) s = `ws://${s.slice('http://'.length)}`
  if (s.startsWith('https://')) s = `wss://${s.slice('https://'.length)}`
  if (!/^wss?:\/\//.test(s)) s = `ws://${s}`

  if (!/\/ft\/signal($|\?)/.test(s)) s = `${s.replace(/\/+$/, '')}/ft/signal`

  const token = String(webrtcSignalToken.value || '').trim()
  if (token && !/[?&]token=/.test(s)) {
    s += `${s.includes('?') ? '&' : '?'}token=${encodeURIComponent(token)}`
  }

  return s
}

const closeSignalWs = async () => {
  try {
    if (webrtcSignalWs) {
      try { webrtcSignalWs.onopen = null } catch { }
      try { webrtcSignalWs.onclose = null } catch { }
      try { webrtcSignalWs.onerror = null } catch { }
      try { webrtcSignalWs.onmessage = null } catch { }
      try { webrtcSignalWs.close() } catch { }
    }
  } finally {
    webrtcSignalWs = null
    webrtcSignalConn.value = 'closed'
    webrtcSignalJoined.value = false
    webrtcClientId.value = ''
    webrtcTargetPeerId.value = ''
    webrtcSignalRoom = ''
  }
}

const webrtcSignalText = computed(() => {
  const s = String(webrtcSignalConn.value || 'idle')
  if (!webrtcSignalUrl.value) return '未配置'
  if (s === 'connecting') return '连接中'
  if (s === 'open' && webrtcSignalJoined.value) return '已加入'
  if (s === 'open') return '已连接'
  if (s === 'error') return '错误'
  if (s === 'closed') return '已断开'
  return '未连接'
})

const webrtcSignalTagType = computed(() => {
  const s = String(webrtcSignalConn.value || 'idle')
  if (!webrtcSignalUrl.value) return 'warning'
  if (s === 'open' && webrtcSignalJoined.value) return 'success'
  if (s === 'open') return 'info'
  if (s === 'connecting') return 'info'
  if (s === 'error') return 'error'
  if (s === 'closed') return 'default'
  return 'default'
})

const webrtcCanJoinRoom = computed(() => {
  return !!normalizeSignalUrl(webrtcSignalUrl.value) && !!String(webrtcRoomCode.value || '').trim() && !webrtcBusy.value && !webrtcSignalJoined.value
})

const webrtcGenerateRoomCode = () => {
  const n = Math.floor(100000 + Math.random() * 900000)
  webrtcRoomCode.value = String(n)
}

const webrtcSignalSend = (obj) => {
  const ws = webrtcSignalWs
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    throw new Error('信令连接未建立')
  }
  ws.send(JSON.stringify(obj))
}

const maybeStartOffer = async () => {
  const me = String(webrtcClientId.value || '')
  const peer = String(webrtcTargetPeerId.value || '')
  if (!me || !peer) return

  const shouldOffer = me < peer
  if (!shouldOffer) return

  logger.info('[FTA][WebRTC][Signal] start offer, me=', me, 'peer=', peer)

  webrtcBusy.value = true
  webrtcLastError.value = ''

  try {
    const pc = await ensureWebrtcPc('offer')

    if (!webrtcDc) {
      webrtcDc = pc.createDataChannel('guaxi-ft')
      bindDataChannel(webrtcDc)
    }

    const offer = await pc.createOffer()
    await pc.setLocalDescription(offer)
    await waitIceGatheringComplete(pc)

    const d = pc.localDescription
    if (!d?.type || !d?.sdp) throw new Error('localDescription is empty')

    const payload = { type: d.type, sdp: d.sdp }
    webrtcLocalSdp.value = JSON.stringify(payload)

    webrtcSignalSend({
      t: 'signal',
      room: webrtcSignalRoom,
      to: peer,
      data: { t: 'sdp', sdp: payload },
    })
  } catch (e) {
    logger.error('[FTA][WebRTC][Signal] start offer failed:', e)
    webrtcLastError.value = String(e?.message || e || '创建 Offer 失败')
  } finally {
    webrtcBusy.value = false
    updateWebrtcState()
  }
}

const handleSignalMsg = async (msg) => {
  const t = String(msg?.t || '')

  if (t === 'joined') {
    webrtcSignalJoined.value = true
    webrtcSignalConn.value = 'open'
    webrtcSignalRoom = String(msg?.room || '')
    webrtcClientId.value = String(msg?.clientId || '')

    const peers = Array.isArray(msg?.peers) ? msg.peers.map(x => String(x || '')).filter(Boolean) : []
    webrtcTargetPeerId.value = peers[0] || ''

    logger.info('[FTA][WebRTC][Signal] joined room=', webrtcSignalRoom, 'clientId=', webrtcClientId.value, 'peers=', peers)

    await maybeStartOffer()
    return
  }

  if (t === 'peer_joined') {
    const pid = String(msg?.clientId || '')
    if (pid && !webrtcTargetPeerId.value) {
      webrtcTargetPeerId.value = pid
      logger.info('[FTA][WebRTC][Signal] peer joined, set target=', pid)
      await maybeStartOffer()
    }
    return
  }

  if (t === 'peer_left') {
    const pid = String(msg?.clientId || '')
    if (pid && pid === webrtcTargetPeerId.value) {
      logger.info('[FTA][WebRTC][Signal] target left:', pid)
      webrtcTargetPeerId.value = ''
    }
    return
  }

  if (t === 'signal') {
    const from = String(msg?.from || '')
    const data = msg?.data || null

    if (!from) return
    if (webrtcTargetPeerId.value && from !== webrtcTargetPeerId.value) {
      logger.warn('[FTA][WebRTC][Signal] ignore signal from unexpected peer:', from)
      return
    }
    if (!webrtcTargetPeerId.value) webrtcTargetPeerId.value = from

    const dt = String(data?.t || '')
    if (dt === 'sdp') {
      const sdp = data?.sdp
      const type = String(sdp?.type || '')
      const sdpText = String(sdp?.sdp || '')

      if (!type || !sdpText) return

      if (type === 'offer') {
        logger.info('[FTA][WebRTC][Signal] recv offer from=', from)

        webrtcBusy.value = true
        webrtcLastError.value = ''

        try {
          await webrtcClose()
          const pc = await ensureWebrtcPc('answer')
          await pc.setRemoteDescription({ type: 'offer', sdp: sdpText })

          const answer = await pc.createAnswer()
          await pc.setLocalDescription(answer)
          await waitIceGatheringComplete(pc)

          const d = pc.localDescription
          if (!d?.type || !d?.sdp) throw new Error('localDescription is empty')

          const payload = { type: d.type, sdp: d.sdp }
          webrtcLocalSdp.value = JSON.stringify(payload)

          webrtcSignalSend({
            t: 'signal',
            room: webrtcSignalRoom,
            to: from,
            data: { t: 'sdp', sdp: payload },
          })
        } catch (e) {
          logger.error('[FTA][WebRTC][Signal] answer failed:', e)
          webrtcLastError.value = String(e?.message || e || '创建 Answer 失败')
        } finally {
          webrtcBusy.value = false
          updateWebrtcState()
        }
        return
      }

      if (type === 'answer') {
        logger.info('[FTA][WebRTC][Signal] recv answer from=', from)

        webrtcBusy.value = true
        webrtcLastError.value = ''

        try {
          const pc = await ensureWebrtcPc('apply')
          await pc.setRemoteDescription({ type: 'answer', sdp: sdpText })
        } catch (e) {
          logger.error('[FTA][WebRTC][Signal] apply answer failed:', e)
          webrtcLastError.value = String(e?.message || e || '设置远端描述失败')
        } finally {
          webrtcBusy.value = false
          updateWebrtcState()
        }
        return
      }
    }

    return
  }

  if (t === 'left') {
    logger.info('[FTA][WebRTC][Signal] left')
    return
  }

  if (t === 'error') {
    const err = String(msg?.error || '')
    logger.warn('[FTA][WebRTC][Signal] error:', err)
    if (err) webrtcLastError.value = err
    return
  }
}

const webrtcJoinRoom = async () => {
  if (webrtcBusy.value) return

  const room = String(webrtcRoomCode.value || '').trim()
  if (!room) {
    message.warning('请先填写房间码')
    return
  }

  const url = normalizeSignalUrl(webrtcSignalUrl.value)
  if (!url) {
    message.warning('请先到“设置 → 外观与系统”配置 WebRTC 信令服务地址')
    return
  }

  await closeSignalWs()

  webrtcBusy.value = true
  webrtcLastError.value = ''

  try {
    logger.info('[FTA][WebRTC][Signal] connect url=', url, 'room=', room)
    webrtcSignalConn.value = 'connecting'

    const ws = new WebSocket(url)
    webrtcSignalWs = ws

    ws.onopen = () => {
      logger.info('[FTA][WebRTC][Signal] ws open')
      webrtcSignalConn.value = 'open'
      try {
        ws.send(JSON.stringify({ t: 'join', room }))
      } catch (e) {
        logger.error('[FTA][WebRTC][Signal] send join failed:', e)
      }
    }

    ws.onmessage = async (ev) => {
      try {
        const raw = String(ev?.data || '')
        const msg = JSON.parse(raw)
        await handleSignalMsg(msg)
      } catch (e) {
        logger.error('[FTA][WebRTC][Signal] onmessage parse failed:', e)
      }
    }

    ws.onerror = (e) => {
      logger.error('[FTA][WebRTC][Signal] ws error:', e)
      webrtcSignalConn.value = 'error'
    }

    ws.onclose = () => {
      logger.info('[FTA][WebRTC][Signal] ws close')
      webrtcSignalConn.value = 'closed'
      webrtcSignalJoined.value = false
      webrtcClientId.value = ''
      webrtcTargetPeerId.value = ''
      webrtcSignalRoom = ''
    }

    await new Promise((resolve) => setTimeout(resolve, 150))
  } catch (e) {
    logger.error('[FTA][WebRTC][Signal] join failed:', e)
    webrtcSignalConn.value = 'error'
    webrtcLastError.value = String(e?.message || e || '加入房间失败')
  } finally {
    webrtcBusy.value = false
  }
}

const webrtcLeaveRoom = async () => {
  try {
    if (webrtcSignalWs && webrtcSignalWs.readyState === WebSocket.OPEN && webrtcSignalRoom) {
      try {
        webrtcSignalWs.send(JSON.stringify({ t: 'leave' }))
      } catch (e) {
        logger.error('[FTA][WebRTC][Signal] send leave failed:', e)
      }
    }
  } finally {
    await closeSignalWs()
  }
}

const parseIceServers = () => {
  const urls = String(webrtcIceServersText.value || '')
    .split(/\r?\n/)
    .map(x => String(x || '').trim())
    .filter(Boolean)

  return urls.map(u => ({ urls: u }))
}

const updateWebrtcState = () => {
  try {
    const s = String(webrtcPc?.connectionState || 'new')
    webrtcConnState.value = s
  } catch {
    webrtcConnState.value = 'new'
  }
}

const waitIceGatheringComplete = async (pc, timeoutMs = 15000) => {
  if (!pc) return
  if (pc.iceGatheringState === 'complete') return

  await new Promise((resolve) => {
    let done = false
    const t = setTimeout(() => {
      if (done) return
      done = true
      try { pc.onicegatheringstatechange = null } catch { }
      resolve()
    }, timeoutMs)

    pc.onicegatheringstatechange = () => {
      if (done) return
      if (pc.iceGatheringState === 'complete') {
        done = true
        clearTimeout(t)
        resolve()
      }
    }
  })
}

const webrtcConnStateText = computed(() => {
  const s = String(webrtcConnState.value || 'new')
  if (s === 'connected') return '已连接'
  if (s === 'connecting') return '连接中'
  if (s === 'failed') return '失败'
  if (s === 'disconnected') return '已断开'
  if (s === 'closed') return '已关闭'
  return '未连接'
})

const webrtcConnStateTagType = computed(() => {
  const s = String(webrtcConnState.value || 'new')
  if (s === 'connected') return 'success'
  if (s === 'connecting') return 'info'
  if (s === 'failed') return 'error'
  if (s === 'disconnected') return 'warning'
  return 'default'
})

const webrtcCanSend = computed(() => {
  return !!webrtcDc && webrtcDc?.readyState === 'open' && String(webrtcConnState.value) === 'connected' && !webrtcBusy.value
})

const bindDataChannel = (dc) => {
  if (!dc) return

  dc.onopen = () => logger.info('[FTA][WebRTC] dc open')
  dc.onclose = () => logger.info('[FTA][WebRTC] dc close')
  dc.onerror = (e) => logger.error('[FTA][WebRTC] dc error', e)

  dc.onmessage = async (ev) => {
    try {
      const raw = ev?.data
      if (typeof raw !== 'string') return
      const msg = JSON.parse(raw)
      const t = String(msg?.t || '')

      if (t === 'init') {
        logger.info('[FTA][WebRTC] recv init uploadId=', msg?.uploadId)
        const resp = await invoke('filetransfer_webrtc_recv_init', {
          saveDir: null,
          uploadId: msg.uploadId,
          fileName: msg.fileName,
          fileSize: msg.fileSize,
          mtimeMs: msg.mtimeMs,
          peerLabel: msg.peerLabel || null,
        })
        dc.send(JSON.stringify({ t: 'init_ack', uploadId: resp.uploadId, receivedBytes: resp.receivedBytes }))
        return
      }

      if (t === 'chunk') {
        await invoke('filetransfer_webrtc_recv_chunk_base64', {
          uploadId: msg.uploadId,
          offset: msg.offset,
          dataBase64: msg.dataBase64,
        })
        return
      }

      if (t === 'complete') {
        const savedPath = await invoke('filetransfer_webrtc_recv_complete', { uploadId: msg.uploadId })
        dc.send(JSON.stringify({ t: 'complete_ack', uploadId: msg.uploadId, savedPath }))
        message.success('WebRTC 接收完成')
        return
      }
    } catch (e) {
      logger.error('[FTA][WebRTC] onmessage failed:', e)
      webrtcLastError.value = String(e?.message || e || 'WebRTC 消息处理失败')
    }
  }
}

const ensureWebrtcPc = async (role) => {
  if (webrtcPc) return webrtcPc

  webrtcLastError.value = ''
  const iceServers = parseIceServers()
  logger.info('[FTA][WebRTC] create pc role=', role, 'iceServers=', iceServers)

  const pc = new RTCPeerConnection({ iceServers })

  pc.onconnectionstatechange = () => {
    logger.info('[FTA][WebRTC] connectionState=', pc.connectionState)
    updateWebrtcState()
  }

  pc.onicecandidateerror = (ev) => logger.warn('[FTA][WebRTC] icecandidateerror:', ev)

  pc.ondatachannel = (ev) => {
    logger.info('[FTA][WebRTC] ondatachannel label=', ev?.channel?.label)
    webrtcDc = ev.channel
    bindDataChannel(webrtcDc)
  }

  webrtcPc = pc
  updateWebrtcState()
  return pc
}

const webrtcCreateOffer = async () => {
  webrtcBusy.value = true
  webrtcLastError.value = ''

  try {
    const pc = await ensureWebrtcPc('offer')

    if (!webrtcDc) {
      webrtcDc = pc.createDataChannel('guaxi-ft')
      bindDataChannel(webrtcDc)
    }

    const offer = await pc.createOffer()
    await pc.setLocalDescription(offer)
    await waitIceGatheringComplete(pc)

    const d = pc.localDescription
    webrtcLocalSdp.value = JSON.stringify({ type: d?.type, sdp: d?.sdp })
  } catch (e) {
    logger.error('[FTA][WebRTC] create offer failed:', e)
    webrtcLastError.value = String(e?.message || e || '创建 Offer 失败')
  } finally {
    webrtcBusy.value = false
    updateWebrtcState()
  }
}

const webrtcCreateAnswer = async () => {
  webrtcBusy.value = true
  webrtcLastError.value = ''

  try {
    const pc = await ensureWebrtcPc('answer')

    const raw = String(webrtcRemoteSdp.value || '').trim()
    if (!raw) {
      message.warning('请先粘贴对方 Offer')
      return
    }

    const obj = JSON.parse(raw)
    if (obj?.type !== 'offer' || !obj?.sdp) {
      throw new Error('远端内容不是 offer JSON')
    }

    await pc.setRemoteDescription({ type: 'offer', sdp: obj.sdp })

    const answer = await pc.createAnswer()
    await pc.setLocalDescription(answer)
    await waitIceGatheringComplete(pc)

    const d = pc.localDescription
    webrtcLocalSdp.value = JSON.stringify({ type: d?.type, sdp: d?.sdp })
  } catch (e) {
    logger.error('[FTA][WebRTC] create answer failed:', e)
    webrtcLastError.value = String(e?.message || e || '创建 Answer 失败')
  } finally {
    webrtcBusy.value = false
    updateWebrtcState()
  }
}

const webrtcApplyRemote = async () => {
  webrtcBusy.value = true
  webrtcLastError.value = ''

  try {
    const pc = await ensureWebrtcPc('apply')

    const raw = String(webrtcRemoteSdp.value || '').trim()
    if (!raw) {
      message.warning('请先粘贴对方内容')
      return
    }

    const obj = JSON.parse(raw)
    if (!obj?.type || !obj?.sdp) {
      throw new Error('远端内容不是 JSON（需要 {type,sdp}）')
    }

    await pc.setRemoteDescription({ type: obj.type, sdp: obj.sdp })
    logger.info('[FTA][WebRTC] remote applied type=', obj.type)
  } catch (e) {
    logger.error('[FTA][WebRTC] apply remote failed:', e)
    webrtcLastError.value = String(e?.message || e || '设置远端描述失败')
  } finally {
    webrtcBusy.value = false
    updateWebrtcState()
  }
}

const webrtcClose = async () => {
  try {
    logger.info('[FTA][WebRTC] close')
    try { webrtcDc?.close?.() } catch { }
    try { webrtcPc?.close?.() } catch { }

    webrtcDc = null
    webrtcPc = null

    webrtcLocalSdp.value = ''
    webrtcRemoteSdp.value = ''
    webrtcLastError.value = ''
    webrtcConnState.value = 'closed'
  } catch (e) {
    logger.error('[FTA][WebRTC] close failed:', e)
  }
}

const sendDcJson = async (obj) => {
  if (!webrtcDc || webrtcDc.readyState !== 'open') {
    throw new Error('DataChannel 未连接')
  }
  webrtcDc.send(JSON.stringify(obj))
}

const throttleDc = async (threshold = 8 * 1024 * 1024) => {
  if (!webrtcDc) return
  while (webrtcDc.bufferedAmount > threshold) {
    await new Promise(r => setTimeout(r, 20))
  }
}

const webrtcPickAndSendFiles = async () => {
  webrtcBusy.value = true
  webrtcLastError.value = ''

  try {
    const picked = await open({ multiple: true, directory: false })
    const paths = Array.isArray(picked) ? picked : (typeof picked === 'string' ? [picked] : [])
    const list = paths.map(x => String(x || '').trim()).filter(Boolean)
    if (!list.length) return

    for (const filePath of list) {
      await webrtcSendOneFile(filePath)
    }

    message.success('WebRTC 发送完成')
  } catch (e) {
    logger.error('[FTA][WebRTC] send files failed:', e)
    webrtcLastError.value = String(e?.message || e || 'WebRTC 发送失败')
  } finally {
    webrtcBusy.value = false
  }
}

const webrtcPickAndSendFolder = async () => {
  webrtcBusy.value = true
  webrtcLastError.value = ''
  try {
    const picked = await open({ multiple: false, directory: true })
    const list = Array.isArray(picked) ? picked : picked ? [picked] : []
    if (!list.length) return
    const entries = await invoke('filetransfer_collect_entries', { paths: list })
    for (const entry of entries) {
      await webrtcSendOneFile(entry.path, entry.relative_path)
    }
    message.success('WebRTC 文件夹发送完成')
  } catch (e) {
    logger.error('[FTA][WebRTC] send folder failed:', e)
    webrtcLastError.value = String(e?.message || e || 'WebRTC 发送失败')
  } finally {
    webrtcBusy.value = false
  }
}

const webrtcSendOneFile = async (filePath, relativePath = '') => {
  const st = await invoke('filetransfer_stat_file', { filePath })
  const uploadId = await invoke('filetransfer_compute_upload_id', { filePath })

  const total = Number(st?.totalBytes || 0)
  logger.info('[FTA][WebRTC] send init file=', st?.fileName, 'bytes=', total, 'uploadId=', uploadId)

  await sendDcJson({
    t: 'init',
    uploadId,
    fileName: relativePath || st?.fileName,
    fileSize: total,
    mtimeMs: st?.mtimeMs,
    peerLabel: 'guaxi',
  })

  let receivedBytes = 0

  await new Promise((resolve) => {
    const dc = webrtcDc
    if (!dc) return resolve()

    const onMsg = (ev) => {
      try {
        const raw = ev?.data
        if (typeof raw !== 'string') return
        const msg = JSON.parse(raw)
        if (msg?.t !== 'init_ack' || msg?.uploadId !== uploadId) return
        receivedBytes = Number(msg?.receivedBytes || 0)
        dc.removeEventListener('message', onMsg)
        resolve()
      } catch {
        dc.removeEventListener('message', onMsg)
        resolve()
      }
    }

    dc.addEventListener('message', onMsg)
    setTimeout(() => {
      try { dc.removeEventListener('message', onMsg) } catch { }
      resolve()
    }, 3000)
  })

  let offset = Math.max(0, Math.min(total, receivedBytes))
  const chunkSize = 128 * 1024

  while (offset < total) {
    await throttleDc()

    const dataBase64 = await invoke('filetransfer_read_file_chunk_base64', {
      filePath,
      offset,
      length: chunkSize,
    })

    if (!dataBase64) break

    await sendDcJson({ t: 'chunk', uploadId, offset, dataBase64 })

    const b64 = String(dataBase64)
    const pad = b64.endsWith('==') ? 2 : (b64.endsWith('=') ? 1 : 0)
    const rawBytes = Math.max(0, Math.floor((b64.length * 3) / 4) - pad)
    offset += rawBytes
  }

  await sendDcJson({ t: 'complete', uploadId })
}

onMounted(async () => {
  logger.info('[FTA] mounted')
  loadTransferPrefs()

  try {
    const [signalUrl, signalToken] = await Promise.all([
      invoke('get_webrtc_signal_url'),
      invoke('get_webrtc_signal_token'),
    ])
    webrtcSignalUrl.value = typeof signalUrl === 'string' ? signalUrl : ''
    webrtcSignalToken.value = typeof signalToken === 'string' ? signalToken : ''
    logger.info('[FTA][WebRTC][Signal] loaded signal config:', webrtcSignalUrl.value)
  } catch (e) {
    logger.error('[FTA][WebRTC][Signal] load signal url failed:', e)
    webrtcSignalUrl.value = ''
    webrtcSignalToken.value = ''
  }

  await refreshAll()
  await setupDropTarget()

  unlistenTaskUpdate = await listen('filetransfer_task_update', (ev) => {
    const t = ev?.payload
    if (!t?.id) return
    const idx = tasks.value.findIndex(x => x?.id === t.id)
    if (idx >= 0) {
      tasks.value.splice(idx, 1, t)
    } else {
      tasks.value.unshift(t)
    }
  })

  unlistenPeersUpdate = await listen('filetransfer_peers_update', (ev) => {
    const list = ev?.payload
    if (Array.isArray(list)) peers.value = list
  })

  unlistenP2pProgress = await listen('p2p-file-progress', (ev) => {
    const payload = ev?.payload
    if (!payload) return
    p2pProgress.value = payload
    if (payload.status === 'done') {
      transferNotice.value = payload.message || 'P2P 发送完成'
      setTimeout(() => { p2pProgress.value = null }, 1200)
    }
  })
})

onUnmounted(async () => {
  logger.info('[FTA] unmounted')
  try { if (typeof unlistenTaskUpdate === 'function') await unlistenTaskUpdate() } catch { }
  try { if (typeof unlistenPeersUpdate === 'function') await unlistenPeersUpdate() } catch { }
  try { if (typeof unlistenDrop === 'function') await unlistenDrop() } catch { }
  try { if (typeof unlistenP2pProgress === 'function') await unlistenP2pProgress() } catch { }
  await webrtcLeaveRoom()
  await webrtcClose()
})
</script>

<style scoped>
.ftc-root {
  --ftc-accent: #5fb3a1;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
  font-size: 13px;
}

.ftc-top {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 13px 18px;
  border-bottom: 1px solid var(--guaxi-border-weak);
  background: var(--guaxi-header-bg);
}

.ftc-title { display: flex; align-items: center; gap: 11px; }
.ftc-title h1 { margin: 0; font-size: 16px; font-weight: 650; letter-spacing: 0.01em; }
.ftc-title p { margin: 2px 0 0; font-size: 10.5px; color: var(--guaxi-text-secondary); letter-spacing: 0.02em; }
.ftc-mark {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: 9px;
  background: rgba(95, 179, 161, 0.14);
  border: 1px solid rgba(95, 179, 161, 0.34);
  color: var(--ftc-accent);
  font-size: 17px;
  font-weight: 700;
}

.ftc-top-actions { display: flex; align-items: center; gap: 8px; }
.ftc-presence {
  display: flex;
  align-items: center;
  gap: 7px;
  height: 28px;
  padding: 0 11px;
  border-radius: 999px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 11.5px;
  cursor: pointer;
}
.ftc-presence.is-on {
  color: var(--guaxi-text-primary);
  border-color: rgba(95, 179, 161, 0.45);
  background: rgba(95, 179, 161, 0.1);
}
.ftc-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--guaxi-text-secondary); }
.ftc-presence.is-on .ftc-dot { background: var(--ftc-accent); box-shadow: 0 0 0 3px rgba(95, 179, 161, 0.16); }
.ftc-ghost {
  height: 28px;
  padding: 0 10px;
  border-radius: 7px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 11.5px;
  cursor: pointer;
}
.ftc-ghost:hover { background: var(--guaxi-hover-bg); color: var(--guaxi-text-primary); }

.ftc-notice {
  margin: 10px 18px 0;
  padding: 8px 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border-radius: 8px;
  border: 1px solid rgba(95, 179, 161, 0.28);
  background: rgba(95, 179, 161, 0.08);
  font-size: 11.5px;
}
.ftc-notice button { border: 0; background: transparent; color: inherit; cursor: pointer; font-size: 14px; line-height: 1; opacity: 0.65; }

.ftc-layout {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: 232px minmax(0, 1fr);
}

.ftc-devices {
  min-height: 0;
  overflow: auto;
  padding: 14px 10px;
  border-right: 1px solid var(--guaxi-border-weak);
}
.ftc-section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 4px 6px 8px;
  color: var(--guaxi-text-secondary);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}
.ftc-section-title span { font-variant-numeric: tabular-nums; letter-spacing: 0; }

.ftc-device-list { display: flex; flex-direction: column; gap: 2px; }
.ftc-device {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 8px;
  border: 1px solid transparent;
  border-radius: 9px;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}
.ftc-device:hover { background: var(--guaxi-hover-bg); }
.ftc-device.is-selected {
  border-color: rgba(95, 179, 161, 0.42);
  background: rgba(95, 179, 161, 0.09);
}
.ftc-avatar {
  flex: 0 0 auto;
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border-radius: 8px;
  background: var(--guaxi-border-weak);
  color: var(--guaxi-text-primary);
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.02em;
}
.ftc-avatar.is-remote { color: var(--ftc-accent); background: rgba(95, 179, 161, 0.1); }
.ftc-avatar.is-p2p { color: #7f9fe0; background: rgba(109, 143, 209, 0.14); font-size: 8.5px; letter-spacing: 0; }
.ftc-device.is-static { cursor: default; }
.ftc-device.is-static:hover { background: transparent; }
.ftc-mini {
  flex: 0 0 auto;
  height: 22px;
  padding: 0 7px;
  border: 1px solid var(--guaxi-border);
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 9.5px;
  cursor: pointer;
}
.ftc-mini:hover:not(:disabled) { color: var(--guaxi-text-primary); background: var(--guaxi-hover-bg); }
.ftc-mini:disabled { opacity: 0.5; cursor: default; }
.ftc-p2p-progress {
  display: flex;
  flex-direction: column;
  gap: 3px;
  margin: 4px 6px 2px;
  padding: 6px 8px;
  border-radius: 6px;
  background: var(--guaxi-hover-bg);
  font-size: 9.5px;
  color: var(--guaxi-text-secondary);
}
.ftc-p2p-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ftc-p2p-progress .ftc-task-bar { margin-top: 1px; }
.ftc-p2p-progress small { font-variant-numeric: tabular-nums; }
.ftc-device-info { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 2px; }
.ftc-device-info strong {
  font-size: 12px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ftc-device-info small {
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ftc-selected { color: var(--ftc-accent); font-size: 9px; }
.ftc-lock {
  flex: 0 0 auto;
  padding: 1px 4px;
  border: 1px solid rgba(216, 166, 87, 0.4);
  border-radius: 4px;
  color: #d8a657;
  font-size: 9px;
  white-space: nowrap;
}
.ftc-token {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 12px 4px 2px;
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
}
.ftc-token .ftc-input { height: 26px; font-size: 10.5px; }
.ftc-empty-mini {
  padding: 16px 8px;
  color: var(--guaxi-text-secondary);
  font-size: 11px;
  line-height: 1.7;
  text-align: center;
}
.ftc-empty-mini small { display: block; font-size: 10px; opacity: 0.75; }

.ftc-device-foot {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 14px 4px 0;
  padding-top: 12px;
  border-top: 1px solid var(--guaxi-border-weak);
}
.ftc-device-foot button {
  border: 0;
  background: transparent;
  color: var(--guaxi-text-secondary);
  text-align: left;
  font-size: 10.5px;
  padding: 3px 4px;
  border-radius: 4px;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ftc-device-foot button:hover { color: var(--guaxi-text-primary); background: var(--guaxi-hover-bg); }

.ftc-addr-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 0 4px 2px;
  color: var(--guaxi-text-secondary);
  font-size: 9.5px;
}
.ftc-addr-head em { font-style: normal; opacity: 0.8; white-space: nowrap; }
.ftc-addr-head em.is-on { color: var(--ftc-accent); opacity: 1; }
.ftc-addr {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 5px 6px !important;
  border: 1px solid transparent !important;
  border-radius: 6px;
  text-align: left;
}
.ftc-addr.is-active {
  border-color: rgba(95, 179, 161, 0.42) !important;
  background: rgba(95, 179, 161, 0.09);
}
.ftc-addr-ip {
  flex: 0 0 auto;
  color: var(--guaxi-text-primary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10px;
}
.ftc-addr-name {
  min-width: 0;
  color: var(--guaxi-text-secondary);
  font-size: 9.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ftc-addr-actions { display: flex; gap: 5px; margin-top: 3px; }
.ftc-addr-actions button {
  flex: 1;
  min-width: 0;
  padding: 5px 4px !important;
  border: 1px solid var(--guaxi-border) !important;
  border-radius: 5px;
  text-align: center;
  white-space: nowrap;
}
.ftc-addr-actions button:disabled { opacity: 0.5; cursor: default; }

.ftc-main {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 16px 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.ftc-drop {
  flex: 0 0 auto;
  padding: 26px 22px 22px;
  text-align: center;
  border: 1px dashed var(--guaxi-border);
  border-radius: 14px;
  background:
    radial-gradient(circle at 50% 0%, rgba(95, 179, 161, 0.07), transparent 62%),
    var(--guaxi-panel-bg);
  transition: border-color 0.18s ease, background 0.18s ease, transform 0.18s ease;
}
.ftc-drop.has-target { border-color: rgba(95, 179, 161, 0.32); }
.ftc-drop.is-active {
  border-color: var(--ftc-accent);
  background: rgba(95, 179, 161, 0.1);
  transform: scale(1.004);
}
.ftc-drop-glyph {
  width: 40px;
  height: 40px;
  margin: 0 auto 10px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  border: 1px solid rgba(95, 179, 161, 0.34);
  color: var(--ftc-accent);
  font-size: 20px;
  font-weight: 300;
}
.ftc-drop h2 { margin: 0; font-size: 15px; font-weight: 600; }
.ftc-drop p { margin: 6px 0 0; color: var(--guaxi-text-secondary); font-size: 11.5px; }
.ftc-drop small { display: block; margin-top: 12px; color: var(--guaxi-text-secondary); font-size: 10.5px; opacity: 0.8; }
.ftc-drop-actions { display: flex; justify-content: center; gap: 8px; margin-top: 14px; }
.ftc-drop-actions button {
  height: 30px;
  padding: 0 14px;
  border-radius: 7px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-primary);
  font-size: 12px;
  cursor: pointer;
}
.ftc-drop-actions button:hover { background: var(--guaxi-hover-bg); }
.ftc-drop-actions .ftc-primary {
  border-color: transparent;
  background: var(--ftc-accent);
  color: #0d1512;
  font-weight: 600;
}
.ftc-drop-actions .ftc-primary:hover { filter: brightness(1.06); }
.ftc-pending {
  margin: 12px auto 0;
  width: fit-content;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(95, 179, 161, 0.12);
  color: var(--ftc-accent);
  font-size: 10.5px;
}

.ftc-queue {
  border: 1px solid var(--guaxi-border-weak);
  border-radius: 12px;
  background: var(--guaxi-panel-bg);
  overflow: hidden;
}
.ftc-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--guaxi-border-weak);
}
.ftc-section-head h3 { margin: 0; font-size: 12.5px; font-weight: 600; }
.ftc-section-head > div { display: flex; align-items: center; gap: 10px; color: var(--guaxi-text-secondary); font-size: 10.5px; }
.ftc-section-head button {
  border: 0;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
}
.ftc-section-head button:hover { color: var(--guaxi-text-primary); background: var(--guaxi-hover-bg); }

.ftc-empty { padding: 24px 14px; text-align: center; color: var(--guaxi-text-secondary); font-size: 11.5px; }

.ftc-task {
  display: grid;
  grid-template-columns: 26px minmax(0, 1fr) auto;
  gap: 10px;
  align-items: center;
  padding: 10px 14px;
  border-bottom: 1px solid var(--guaxi-border-weak);
}
.ftc-task:last-child { border-bottom: 0; }
.ftc-task-dir {
  width: 24px;
  height: 24px;
  display: grid;
  place-items: center;
  border-radius: 7px;
  background: rgba(95, 179, 161, 0.12);
  color: var(--ftc-accent);
  font-size: 12px;
  font-weight: 700;
}
.ftc-task-dir[data-direction='recv'] { background: rgba(109, 143, 209, 0.14); color: #7f9fe0; }
.ftc-task-main { min-width: 0; }
.ftc-task-title { display: flex; align-items: baseline; gap: 8px; }
.ftc-task-name {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 550;
}
.ftc-task-status { color: var(--guaxi-text-secondary); font-size: 10px; white-space: nowrap; }
.ftc-task-sub {
  margin-top: 3px;
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.ftc-task-bar { height: 3px; margin-top: 6px; border-radius: 2px; background: var(--guaxi-border-weak); overflow: hidden; }
.ftc-task-bar span { display: block; height: 100%; background: var(--ftc-accent); transition: width 0.22s ease; }
.ftc-task[data-status='error'] .ftc-task-bar span { background: #e05b5b; }
.ftc-task[data-status='done'] .ftc-task-bar span { background: #63b37a; }
.ftc-task[data-status='interrupted'] .ftc-task-bar span,
.ftc-task[data-status='canceled'] .ftc-task-bar span { background: var(--guaxi-border); }
.ftc-task-actions { display: flex; gap: 4px; }
.ftc-task-actions button {
  height: 24px;
  padding: 0 8px;
  border-radius: 5px;
  border: 1px solid var(--guaxi-border);
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
  cursor: pointer;
}
.ftc-task-actions button:hover { color: var(--guaxi-text-primary); background: var(--guaxi-hover-bg); }
.ftc-task-actions button.is-danger:hover { color: #e05b5b; border-color: rgba(224, 91, 91, 0.4); }

/* 抽屉/弹窗被 teleport 到 body，继承不到 .ftc-root 上的 --ftc-accent，
   主按钮会退化成「透明底 + 深色字」，暗色模式下几乎看不见。 */
.ftc-adv { --ftc-accent: #5fb3a1; display: flex; flex-direction: column; gap: 8px; font-size: 12px; }
.ftc-adv h4 {
  margin: 16px 0 2px;
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.ftc-adv h4:first-child { margin-top: 2px; }
.ftc-row { display: flex; gap: 10px; align-items: flex-end; }
.ftc-field { display: flex; flex: 1; min-width: 0; flex-direction: column; gap: 5px; color: var(--guaxi-text-secondary); font-size: 10.5px; }
.ftc-field.is-narrow { flex: 0 0 104px; }
.ftc-input {
  width: 100%;
  height: 30px;
  box-sizing: border-box;
  padding: 0 9px;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  font-size: 11.5px;
  font-family: inherit;
}
.ftc-input:focus { outline: none; border-color: var(--ftc-accent); }
.ftc-input.is-area { height: auto; padding: 7px 9px; resize: vertical; font-family: var(--guaxi-editor-font-family); }
.ftc-pick { display: flex; gap: 6px; }
.ftc-pick .ftc-input { flex: 1; }
.ftc-pick button,
.ftc-actions button {
  height: 27px;
  padding: 0 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-primary);
  font-size: 11px;
  cursor: pointer;
  white-space: nowrap;
}
.ftc-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; margin-top: 6px; }
.ftc-actions button:hover:not(:disabled) { background: var(--guaxi-hover-bg); }
.ftc-actions button:disabled { opacity: 0.45; cursor: default; }
.ftc-actions button.ftc-primary { border-color: transparent; background: var(--ftc-accent); color: #0d1512; font-weight: 600; }
.ftc-actions.is-muted { color: var(--guaxi-text-secondary); font-size: 10.5px; }
.ftc-mono {
  color: var(--guaxi-text-secondary);
  font-family: var(--guaxi-editor-font-family);
  font-size: 10.5px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ftc-error { color: #e05b5b; font-size: 10.5px; }
.ftc-muted { color: var(--guaxi-text-secondary); font-size: 10.5px; }
.ftc-check {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-bottom: 6px;
  color: var(--guaxi-text-secondary);
  font-size: 10.5px;
  white-space: nowrap;
}
.ftc-modal { width: min(520px, 92vw); }

@media (max-width: 820px) {
  .ftc-layout { grid-template-columns: 196px minmax(0, 1fr); }
  .ftc-main { padding: 12px; }
  .ftc-drop { padding: 20px 14px 18px; }
}
</style>
