<template>
  <div class="fta-root">
    <div class="fta-header">
      <div class="fta-title">文件传输助手</div>
      <div class="fta-subtitle">
        局域网发现 + 直连互传（无第三方服务器）；支持多任务、断点续传；支持把目录作为 HTTP 根目录对外上传/下载。
      </div>
    </div>

    <div class="fta-body">
      <n-grid :cols="1" :x-gap="14" :y-gap="14" responsive="screen">
      <n-gi>
        <n-card size="small" class="fta-card" title="接收服务（用于互传）">
          <n-space vertical :size="10">
            <n-space align="center" :size="10">
              <n-tag v-if="receiverStatus?.running" type="success">运行中</n-tag>
              <n-tag v-else type="default">未启动</n-tag>
              <n-text depth="3">bind</n-text>
              <n-input v-model:value="receiverBindAddr" size="small" style="width: 140px" />
              <n-text depth="3">port</n-text>
              <n-input-number v-model:value="receiverPort" size="small" style="width: 140px" :min="1" :max="65535" />
            </n-space>

            <n-space align="center" :size="10">
              <n-text depth="3">保存目录</n-text>
              <n-input v-model:value="receiverSaveDir" size="small" placeholder="留空则使用默认 incoming 目录" />
              <n-button size="small" tertiary @click="pickReceiverSaveDir">选择</n-button>
            </n-space>

            <n-space>
              <n-button size="small" type="primary" :disabled="!!receiverStatus?.running" @click="startReceiver">
                启动接收服务
              </n-button>
              <n-button size="small" type="error" tertiary :disabled="!receiverStatus?.running" @click="stopReceiver">
                停止
              </n-button>
              <n-button size="small" tertiary @click="refreshAll">刷新状态</n-button>
            </n-space>

            <n-alert v-if="receiverStatus?.running" type="info" :show-icon="false">
              <div class="fta-kv">
                <div class="k">服务地址</div>
                <div class="v">{{ receiverStatus.baseUrl }}</div>
              </div>
              <div class="fta-kv">
                <div class="k">提示</div>
                <div class="v">
                  bind=0.0.0.0 表示监听所有网卡；对外访问时请用本机局域网 IP（或公网 IP/域名 + 端口转发）。
                </div>
              </div>
            </n-alert>
          </n-space>
        </n-card>
      </n-gi>

      <n-gi>
        <n-card size="small" class="fta-card" title="局域网发现（多台安装本软件互传）">
          <n-space vertical :size="10">
            <n-space align="center">
              <n-tag v-if="discoveryRunning" type="success">发现中</n-tag>
              <n-tag v-else>未开启</n-tag>
              <n-text depth="3">发现端口</n-text>
              <n-input-number v-model:value="discoveryPort" size="small" style="width: 140px" :min="1" :max="65535" />
            </n-space>

            <n-space>
              <n-button size="small" type="primary" :disabled="discoveryRunning" @click="startDiscovery">
                开启发现
              </n-button>
              <n-button size="small" type="error" tertiary :disabled="!discoveryRunning" @click="stopDiscovery">
                关闭发现
              </n-button>
              <n-button size="small" tertiary @click="refreshPeers">刷新列表</n-button>
            </n-space>

            <n-empty v-if="!peers.length" description="暂无发现到的设备（请确保双方都启动了接收服务并开启发现）" />

            <n-list v-else bordered hoverable>
              <n-list-item v-for="p in peers" :key="p.peerId">
                <n-space justify="space-between" align="center" style="width: 100%">
                  <div class="peer-main">
                    <div class="peer-title">
                      <span class="peer-name">{{ p.name }}</span>
                      <n-text depth="3" class="peer-meta">{{ p.addr }}:{{ p.port }}</n-text>
                    </div>
                    <div class="peer-sub">
                      <n-text depth="3">lastSeen: {{ formatLastSeen(p.lastSeenMs) }}</n-text>
                    </div>
                  </div>

                  <n-space>
                    <n-button size="tiny" tertiary @click="sendFilesToPeer(p)">发送文件</n-button>
                    <n-button size="tiny" tertiary @click="copyPeerUrl(p)">复制地址</n-button>
                  </n-space>
                </n-space>
              </n-list-item>
            </n-list>
          </n-space>
        </n-card>
      </n-gi>

      <n-gi>
        <n-card size="small" class="fta-card" title="互联网互传">
          <n-tabs type="segment" size="small" animated>
            <n-tab-pane name="http" tab="HTTP 直连">
              <n-space vertical :size="10">
                <n-alert type="warning" :show-icon="false">
                  HTTP 直连需要你知道对方“可访问的地址”（公网 IP/域名 + 端口转发 或 同一内网）。
                </n-alert>

                <n-space align="center">
                  <n-text depth="3">对方地址</n-text>
                  <n-input v-model:value="customPeerUrl"
                    placeholder="例如：http://203.0.113.10:39190 或 http://myhost.example.com:39190" />
                  <n-button size="small" type="primary" @click="sendFilesToCustomUrl">选择文件并发送</n-button>
                </n-space>
              </n-space>
            </n-tab-pane>

            <n-tab-pane name="webrtc" tab="WebRTC（房间码）">
              <n-space vertical :size="10">
                <n-alert type="info" :show-icon="false">
                  房间码模式需要一个“信令服务”（只中转 WebRTC 协商信息，不传文件内容）。双方输入同一房间码即可自动交换 offer/answer。
                </n-alert>

                <n-space align="center" :size="10">
                  <n-text depth="3">信令服务</n-text>
                  <n-text depth="3">{{ webrtcSignalUrl || '未配置（到 设置 → 外观与系统 配置）' }}</n-text>
                  <n-button size="small" tertiary :disabled="!webrtcSignalUrl" @click="copyText(webrtcSignalUrl)">复制</n-button>
                </n-space>

                <n-space align="center" :size="10">
                  <n-text depth="3">房间码</n-text>
                  <n-input v-model:value="webrtcRoomCode" size="small" style="width: 140px" placeholder="例如：123456" />
                  <n-button size="small" tertiary :disabled="webrtcBusy" @click="webrtcGenerateRoomCode">生成</n-button>
                  <n-button size="small" type="primary" :disabled="webrtcBusy || !webrtcCanJoinRoom" @click="webrtcJoinRoom">加入</n-button>
                  <n-button size="small" tertiary :disabled="webrtcBusy || !webrtcSignalJoined" @click="webrtcLeaveRoom">退出</n-button>
                  <n-tag :type="webrtcSignalTagType">{{ webrtcSignalText }}</n-tag>
                </n-space>

                <n-space align="center" :size="10">
                  <n-text depth="3">我的 ID</n-text>
                  <n-text depth="3">{{ webrtcClientId || '-' }}</n-text>
                  <n-text depth="3">对端</n-text>
                  <n-text depth="3">{{ webrtcTargetPeerId || '-' }}</n-text>
                </n-space>

                <n-space align="center" :size="10">
                  <n-text depth="3">连接状态</n-text>
                  <n-tag :type="webrtcConnStateTagType">{{ webrtcConnStateText }}</n-tag>
                  <n-button size="small" tertiary :disabled="webrtcBusy" @click="webrtcClose">断开</n-button>
                </n-space>

                <n-space vertical :size="8">
                  <n-text depth="3">STUN servers（每行一个 urls，例如：stun:stun.cloudflare.com:3478）</n-text>
                  <n-input v-model:value="webrtcIceServersText" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
                </n-space>

                <n-space align="center" :size="10">
                  <n-button size="small" type="primary" :disabled="!webrtcCanSend" @click="webrtcPickAndSendFiles">选择文件并通过 WebRTC 发送</n-button>
                  <n-text depth="3">发送端分块读取本地文件并通过 DataChannel 发送；接收端落盘，支持断点续传。</n-text>
                </n-space>

                <n-alert v-if="webrtcLastError" type="error" :show-icon="false">
                  {{ webrtcLastError }}
                </n-alert>
              </n-space>
            </n-tab-pane>
          </n-tabs>
        </n-card>
      </n-gi>

      <n-gi>
        <n-card size="small" class="fta-card" title="任务管理（多任务 / 断点续传 / 暂停继续）">
          <n-space vertical :size="10">
            <n-space>
              <n-button size="small" tertiary @click="refreshTasks">刷新任务</n-button>
              <n-text depth="3">任务更新也会通过事件实时推送</n-text>
            </n-space>

            <n-empty v-if="!tasks.length" description="暂无任务" />

            <div v-else class="fta-task-list">
              <n-list bordered>
                <n-list-item v-for="t in tasks" :key="t.id">
                  <div class="task-row">
                    <div class="task-left">
                      <div class="task-title">
                        <n-tag size="small" :type="tagTypeForTask(t)">{{ t.status }}</n-tag>
                        <span class="task-name">{{ t.fileName }}</span>
                        <n-text depth="3" class="task-peer">{{ t.peer }}</n-text>
                      </div>

                      <div class="task-progress">
                        <n-progress type="line" :percentage="percent(t.transferredBytes, t.totalBytes)"
                          :show-indicator="true" :height="10" />
                        <n-text depth="3" class="task-bytes">
                          {{ formatBytes(t.transferredBytes) }} / {{ formatBytes(t.totalBytes) }}
                        </n-text>
                      </div>

                      <div v-if="t.error" class="task-error">
                        <n-text type="error">{{ t.error }}</n-text>
                      </div>
                    </div>

                    <div class="task-actions">
                      <n-space>
                        <n-button size="tiny" tertiary :disabled="t.status !== 'running'"
                          @click="pauseTask(t.id)">暂停</n-button>
                        <n-button size="tiny" tertiary :disabled="t.status !== 'paused'"
                          @click="resumeTask(t.id)">继续</n-button>
                        <n-button size="tiny" tertiary type="error"
                          :disabled="t.status === 'done' || t.status === 'canceled'"
                          @click="cancelTask(t.id)">取消</n-button>
                      </n-space>
                    </div>
                  </div>
                </n-list-item>
              </n-list>
            </div>
          </n-space>
        </n-card>
      </n-gi>

      <n-gi>
        <n-card size="small" class="fta-card" title="目录 HTTP 分享（把目录作为 HTTP 根目录，支持多人上传/下载）">
          <n-space vertical :size="10">
            <n-space align="center">
              <n-tag v-if="httpShareStatus?.running" type="success">运行中</n-tag>
              <n-tag v-else>未启动</n-tag>

              <n-text depth="3">bind</n-text>
              <n-input v-model:value="shareBindAddr" size="small" style="width: 140px" />
              <n-text depth="3">port</n-text>
              <n-input-number v-model:value="sharePort" size="small" style="width: 140px" :min="1" :max="65535" />
            </n-space>

            <n-space align="center">
              <n-text depth="3">目录</n-text>
              <n-input v-model:value="shareRootDir" size="small" placeholder="请选择要分享的目录" />
              <n-button size="small" tertiary @click="pickShareRoot">选择</n-button>
            </n-space>

            <n-space>
              <n-button size="small" type="primary" :disabled="httpShareStatus?.running" @click="startHttpShare">
                启动分享
              </n-button>
              <n-button size="small" type="error" tertiary :disabled="!httpShareStatus?.running" @click="stopHttpShare">
                停止
              </n-button>
            </n-space>

            <n-alert v-if="httpShareStatus?.running" type="info" :show-icon="false">
              <div class="fta-kv">
                <div class="k">访问</div>
                <div class="v">{{ httpShareStatus.baseUrl }}/</div>
              </div>
              <div class="fta-kv">
                <div class="k">下载</div>
                <div class="v">GET {{ httpShareStatus.baseUrl }}/相对路径</div>
              </div>
              <div class="fta-kv">
                <div class="k">上传</div>
                <div class="v">PUT {{ httpShareStatus.baseUrl }}/相对路径 （body 为文件内容）</div>
              </div>
            </n-alert>
          </n-space>
        </n-card>
      </n-gi>
      </n-grid>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'

const { message } = createDiscreteApi(['message'])

const receiverBindAddr = ref('0.0.0.0')
const receiverPort = ref(39190)
const receiverSaveDir = ref('')

const discoveryPort = ref(39191)
const customPeerUrl = ref('')

const shareBindAddr = ref('0.0.0.0')
const sharePort = ref(39200)
const shareRootDir = ref('')

const receiverStatus = ref(null)
const httpShareStatus = ref(null)
const discoveryRunning = ref(false)

const peers = ref([])
const tasks = ref([])

const webrtcIceServersText = ref('stun:stun.cloudflare.com:3478\nstun:stun.l.google.com:19302')
const webrtcLocalSdp = ref('')
const webrtcRemoteSdp = ref('')
const webrtcConnState = ref('new')
const webrtcBusy = ref(false)
const webrtcLastError = ref('')

const webrtcSignalUrl = ref('')
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
  await Promise.allSettled([refreshStatus(), refreshPeers(), refreshTasks()])
}

const refreshStatus = async () => {
  try {
    const st = await invoke('filetransfer_get_status')
    receiverStatus.value = st?.receiver || null
    httpShareStatus.value = st?.httpShare || null
    discoveryRunning.value = !!st?.discoveryRunning
  } catch (e) {
    console.error('[FTA] refreshStatus failed:', e)
  }
}

const refreshPeers = async () => {
  try {
    const list = await invoke('filetransfer_get_peers')
    peers.value = Array.isArray(list) ? list : []
  } catch (e) {
    console.error('[FTA] refreshPeers failed:', e)
  }
}

const refreshTasks = async () => {
  try {
    const list = await invoke('filetransfer_list_tasks')
    tasks.value = Array.isArray(list) ? list : []
  } catch (e) {
    console.error('[FTA] refreshTasks failed:', e)
  }
}

const pickReceiverSaveDir = async () => {
  try {
    const p = await open({ directory: true, multiple: false })
    if (typeof p === 'string' && p.trim()) receiverSaveDir.value = p
  } catch (e) {
    console.error('[FTA] pickReceiverSaveDir failed:', e)
  }
}

const startReceiver = async () => {
  try {
    await invoke('filetransfer_start_receiver', {
      bindAddr: receiverBindAddr.value,
      port: receiverPort.value,
      saveDir: receiverSaveDir.value || null,
    })
    message.success('接收服务已启动')
    await refreshStatus()
  } catch (e) {
    console.error('[FTA] startReceiver failed:', e)
    message.error(String(e?.message || e || '启动失败'))
  }
}

const stopReceiver = async () => {
  try {
    await invoke('filetransfer_stop_receiver')
    message.success('接收服务已停止')
    await refreshStatus()
  } catch (e) {
    console.error('[FTA] stopReceiver failed:', e)
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
    console.error('[FTA] startDiscovery failed:', e)
    message.error(String(e?.message || e || '开启失败'))
  }
}

const stopDiscovery = async () => {
  try {
    await invoke('filetransfer_stop_lan_discovery')
    message.success('局域网发现已关闭')
    await refreshStatus()
  } catch (e) {
    console.error('[FTA] stopDiscovery failed:', e)
    message.error(String(e?.message || e || '关闭失败'))
  }
}

const pickShareRoot = async () => {
  try {
    const p = await open({ directory: true, multiple: false })
    if (typeof p === 'string' && p.trim()) shareRootDir.value = p
  } catch (e) {
    console.error('[FTA] pickShareRoot failed:', e)
  }
}

const startHttpShare = async () => {
  try {
    await invoke('filetransfer_start_http_share', {
      bindAddr: shareBindAddr.value,
      port: sharePort.value,
      rootDir: shareRootDir.value,
    })
    message.success('HTTP 分享已启动')
    await refreshStatus()
  } catch (e) {
    console.error('[FTA] startHttpShare failed:', e)
    message.error(String(e?.message || e || '启动失败'))
  }
}

const stopHttpShare = async () => {
  try {
    await invoke('filetransfer_stop_http_share')
    message.success('HTTP 分享已停止')
    await refreshStatus()
  } catch (e) {
    console.error('[FTA] stopHttpShare failed:', e)
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
    console.error('[FTA] sendFilesToPeer failed:', e)
    message.error(String(e?.message || e || '发送失败'))
  }
}

const sendFilesToCustomUrl = async () => {
  try {
    const baseUrl = normalizeBaseUrl(customPeerUrl.value)
    if (!baseUrl) {
      message.warning('请先填写对方地址')
      return
    }
    const picked = await open({ multiple: true, directory: false })
    const paths = Array.isArray(picked) ? picked : (typeof picked === 'string' ? [picked] : [])
    const list = paths.map(x => String(x || '').trim()).filter(Boolean)
    if (!list.length) return

    await invoke('filetransfer_send_files', { peerUrl: baseUrl, filePaths: list })
    message.success('已创建发送任务')
    await refreshTasks()
  } catch (e) {
    console.error('[FTA] sendFilesToCustomUrl failed:', e)
    message.error(String(e?.message || e || '发送失败'))
  }
}

const copyPeerUrl = async (peer) => {
  try {
    const url = `http://${peer.addr}:${peer.port}`
    await navigator.clipboard.writeText(url)
    message.success('已复制')
  } catch (e) {
    console.error('[FTA] copyPeerUrl failed:', e)
    message.error('复制失败')
  }
}

const pauseTask = async (taskId) => {
  try {
    await invoke('filetransfer_pause_task', { taskId })
  } catch (e) {
    console.error('[FTA] pauseTask failed:', e)
  }
}

const resumeTask = async (taskId) => {
  try {
    await invoke('filetransfer_resume_task', { taskId })
  } catch (e) {
    console.error('[FTA] resumeTask failed:', e)
  }
}

const cancelTask = async (taskId) => {
  try {
    await invoke('filetransfer_cancel_task', { taskId })
  } catch (e) {
    console.error('[FTA] cancelTask failed:', e)
  }
}

const copyText = async (text) => {
  try {
    const s = String(text || '')
    if (!s) return
    await navigator.clipboard.writeText(s)
    message.success('已复制')
  } catch (e) {
    console.error('[FTA] copyText failed:', e)
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

  console.log('[FTA][WebRTC][Signal] start offer, me=', me, 'peer=', peer)

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
    console.error('[FTA][WebRTC][Signal] start offer failed:', e)
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

    console.log('[FTA][WebRTC][Signal] joined room=', webrtcSignalRoom, 'clientId=', webrtcClientId.value, 'peers=', peers)

    await maybeStartOffer()
    return
  }

  if (t === 'peer_joined') {
    const pid = String(msg?.clientId || '')
    if (pid && !webrtcTargetPeerId.value) {
      webrtcTargetPeerId.value = pid
      console.log('[FTA][WebRTC][Signal] peer joined, set target=', pid)
      await maybeStartOffer()
    }
    return
  }

  if (t === 'peer_left') {
    const pid = String(msg?.clientId || '')
    if (pid && pid === webrtcTargetPeerId.value) {
      console.log('[FTA][WebRTC][Signal] target left:', pid)
      webrtcTargetPeerId.value = ''
    }
    return
  }

  if (t === 'signal') {
    const from = String(msg?.from || '')
    const data = msg?.data || null

    if (!from) return
    if (webrtcTargetPeerId.value && from !== webrtcTargetPeerId.value) {
      console.warn('[FTA][WebRTC][Signal] ignore signal from unexpected peer:', from)
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
        console.log('[FTA][WebRTC][Signal] recv offer from=', from)

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
          console.error('[FTA][WebRTC][Signal] answer failed:', e)
          webrtcLastError.value = String(e?.message || e || '创建 Answer 失败')
        } finally {
          webrtcBusy.value = false
          updateWebrtcState()
        }
        return
      }

      if (type === 'answer') {
        console.log('[FTA][WebRTC][Signal] recv answer from=', from)

        webrtcBusy.value = true
        webrtcLastError.value = ''

        try {
          const pc = await ensureWebrtcPc('apply')
          await pc.setRemoteDescription({ type: 'answer', sdp: sdpText })
        } catch (e) {
          console.error('[FTA][WebRTC][Signal] apply answer failed:', e)
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
    console.log('[FTA][WebRTC][Signal] left')
    return
  }

  if (t === 'error') {
    const err = String(msg?.error || '')
    console.warn('[FTA][WebRTC][Signal] error:', err)
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
    console.log('[FTA][WebRTC][Signal] connect url=', url, 'room=', room)
    webrtcSignalConn.value = 'connecting'

    const ws = new WebSocket(url)
    webrtcSignalWs = ws

    ws.onopen = () => {
      console.log('[FTA][WebRTC][Signal] ws open')
      webrtcSignalConn.value = 'open'
      try {
        ws.send(JSON.stringify({ t: 'join', room }))
      } catch (e) {
        console.error('[FTA][WebRTC][Signal] send join failed:', e)
      }
    }

    ws.onmessage = async (ev) => {
      try {
        const raw = String(ev?.data || '')
        const msg = JSON.parse(raw)
        await handleSignalMsg(msg)
      } catch (e) {
        console.error('[FTA][WebRTC][Signal] onmessage parse failed:', e)
      }
    }

    ws.onerror = (e) => {
      console.error('[FTA][WebRTC][Signal] ws error:', e)
      webrtcSignalConn.value = 'error'
    }

    ws.onclose = () => {
      console.log('[FTA][WebRTC][Signal] ws close')
      webrtcSignalConn.value = 'closed'
      webrtcSignalJoined.value = false
      webrtcClientId.value = ''
      webrtcTargetPeerId.value = ''
      webrtcSignalRoom = ''
    }

    await new Promise((resolve) => setTimeout(resolve, 150))
  } catch (e) {
    console.error('[FTA][WebRTC][Signal] join failed:', e)
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
        console.error('[FTA][WebRTC][Signal] send leave failed:', e)
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

  dc.onopen = () => console.log('[FTA][WebRTC] dc open')
  dc.onclose = () => console.log('[FTA][WebRTC] dc close')
  dc.onerror = (e) => console.error('[FTA][WebRTC] dc error', e)

  dc.onmessage = async (ev) => {
    try {
      const raw = ev?.data
      if (typeof raw !== 'string') return
      const msg = JSON.parse(raw)
      const t = String(msg?.t || '')

      if (t === 'init') {
        console.log('[FTA][WebRTC] recv init uploadId=', msg?.uploadId)
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
      console.error('[FTA][WebRTC] onmessage failed:', e)
      webrtcLastError.value = String(e?.message || e || 'WebRTC 消息处理失败')
    }
  }
}

const ensureWebrtcPc = async (role) => {
  if (webrtcPc) return webrtcPc

  webrtcLastError.value = ''
  const iceServers = parseIceServers()
  console.log('[FTA][WebRTC] create pc role=', role, 'iceServers=', iceServers)

  const pc = new RTCPeerConnection({ iceServers })

  pc.onconnectionstatechange = () => {
    console.log('[FTA][WebRTC] connectionState=', pc.connectionState)
    updateWebrtcState()
  }

  pc.onicecandidateerror = (ev) => console.warn('[FTA][WebRTC] icecandidateerror:', ev)

  pc.ondatachannel = (ev) => {
    console.log('[FTA][WebRTC] ondatachannel label=', ev?.channel?.label)
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
    console.error('[FTA][WebRTC] create offer failed:', e)
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
    console.error('[FTA][WebRTC] create answer failed:', e)
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
    console.log('[FTA][WebRTC] remote applied type=', obj.type)
  } catch (e) {
    console.error('[FTA][WebRTC] apply remote failed:', e)
    webrtcLastError.value = String(e?.message || e || '设置远端描述失败')
  } finally {
    webrtcBusy.value = false
    updateWebrtcState()
  }
}

const webrtcClose = async () => {
  try {
    console.log('[FTA][WebRTC] close')
    try { webrtcDc?.close?.() } catch { }
    try { webrtcPc?.close?.() } catch { }

    webrtcDc = null
    webrtcPc = null

    webrtcLocalSdp.value = ''
    webrtcRemoteSdp.value = ''
    webrtcLastError.value = ''
    webrtcConnState.value = 'closed'
  } catch (e) {
    console.error('[FTA][WebRTC] close failed:', e)
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
    console.error('[FTA][WebRTC] send files failed:', e)
    webrtcLastError.value = String(e?.message || e || 'WebRTC 发送失败')
  } finally {
    webrtcBusy.value = false
  }
}

const webrtcSendOneFile = async (filePath) => {
  const st = await invoke('filetransfer_stat_file', { filePath })
  const uploadId = await invoke('filetransfer_compute_upload_id', { filePath })

  const total = Number(st?.totalBytes || 0)
  console.log('[FTA][WebRTC] send init file=', st?.fileName, 'bytes=', total, 'uploadId=', uploadId)

  await sendDcJson({
    t: 'init',
    uploadId,
    fileName: st?.fileName,
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
  console.log('[FTA] mounted')

  try {
    const signalUrl = await invoke('get_webrtc_signal_url')
    webrtcSignalUrl.value = typeof signalUrl === 'string' ? signalUrl : ''
    console.log('[FTA][WebRTC][Signal] loaded signal url from config:', webrtcSignalUrl.value)
  } catch (e) {
    console.error('[FTA][WebRTC][Signal] load signal url failed:', e)
    webrtcSignalUrl.value = ''
  }

  await refreshAll()

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
})

onUnmounted(async () => {
  console.log('[FTA] unmounted')
  try { if (typeof unlistenTaskUpdate === 'function') await unlistenTaskUpdate() } catch { }
  try { if (typeof unlistenPeersUpdate === 'function') await unlistenPeersUpdate() } catch { }
  await webrtcLeaveRoom()
  await webrtcClose()
})
</script>

<style scoped>
.fta-root {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.fta-header {
  padding: 8px 4px 2px;
}

.fta-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding-bottom: 10px;
}

.fta-task-list {
  max-height: 45vh;
  overflow: auto;
  padding-right: 6px;
}

.fta-title {
  font-size: 16px;
  font-weight: 700;
}

.fta-subtitle {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  line-height: 1.5;
}

.fta-card {
  border-radius: 14px;
}

.fta-kv {
  display: grid;
  grid-template-columns: 80px 1fr;
  gap: 10px;
  margin-top: 6px;
}

.fta-kv .k {
  color: var(--guaxi-text-secondary);
}

.peer-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.peer-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.peer-name {
  font-weight: 600;
}

.peer-meta {
  font-size: 12px;
}

.task-row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 12px;
  align-items: center;
}

.task-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.task-name {
  font-weight: 600;
}

.task-peer {
  font-size: 12px;
}

.task-progress {
  margin-top: 8px;
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 10px;
  align-items: center;
}

.task-bytes {
  font-size: 12px;
  white-space: nowrap;
}

.task-error {
  margin-top: 8px;
}
</style>