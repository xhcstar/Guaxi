<template>
  <div class="nt-root">
    <n-tabs v-model:value="activeNetTab" type="line" size="small" class="nt-tabs" pane-class="nt-pane">
      <n-tab-pane name="parse" tab="报文解析">
        <template #tab><span class="tw-tab"><Icon :icon="searchIcon" width="15" height="15" />报文解析</span></template>
        <div class="nt-parse">
          <div class="nt-toolbar">
            <n-radio-group v-model:value="parseLayer" size="small">
              <n-radio-button value="ip">从 IP 层</n-radio-button>
              <n-radio-button value="eth">从以太网帧</n-radio-button>
            </n-radio-group>
            <n-button size="small" type="primary" :loading="parseLoading" @click="runParse">解析</n-button>
            <n-button size="small" @click="parseInput = sampleTcp">示例(TCP)</n-button>
            <span v-if="error" class="nt-err">{{ error }}</span>
          </div>
          <n-input v-model:value="parseInput" type="textarea" :rows="4" size="small" placeholder="粘贴 hex（可含空格/冒号）或 base64 报文" />
          <div class="nt-parse-body" v-if="parseResult">
            <div class="nt-layers">
              <div v-for="(l, i) in parseResult.layers" :key="i" class="nt-layer">
                <div class="nt-layer-name">{{ l.name }}</div>
                <table class="nt-kv">
                  <tbody>
                    <tr v-for="(kv, j) in l.fields" :key="j"><td>{{ kv[0] }}</td><td>{{ kv[1] }}</td></tr>
                  </tbody>
                </table>
              </div>
              <div class="nt-layer" v-if="parseResult.payload_hex">
                <div class="nt-layer-name">载荷 ({{ parseResult.payload_hex.length / 2 }} 字节)</div>
                <pre class="nt-hex">{{ formatHex(parseResult.payload_hex) }}</pre>
              </div>
            </div>
          </div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="build" tab="报文构造">
        <template #tab><span class="tw-tab"><Icon :icon="hammerIcon" width="15" height="15" />报文构造</span></template>
        <div class="nt-build">
          <div class="nt-form">
            <div class="nt-frow"><label>传输层</label>
              <n-select v-model:value="b.transport" size="small" :options="transportOptions" style="width:160px" />
            </div>
            <div class="nt-frow"><label>源 IP</label><n-input v-model:value="b.ip_src" size="small" placeholder="10.0.0.1" style="width:170px" />
              <label>目的 IP</label><n-input v-model:value="b.ip_dst" size="small" placeholder="10.0.0.2" style="width:170px" />
              <label>TTL</label><n-input-number v-model:value="b.ttl" size="small" :min="1" :max="255" :show-button="false" style="width:80px" />
            </div>
            <div class="nt-frow" v-if="b.transport === 'tcp' || b.transport === 'udp'">
              <label>源端口</label><n-input-number v-model:value="b.src_port" size="small" :min="0" :max="65535" :show-button="false" style="width:100px" />
              <label>目的端口</label><n-input-number v-model:value="b.dst_port" size="small" :min="0" :max="65535" :show-button="false" style="width:100px" />
              <template v-if="b.transport === 'tcp'">
                <label>序列号</label><n-input-number v-model:value="b.seq" size="small" :min="0" :show-button="false" style="width:130px" />
                <label>窗口</label><n-input-number v-model:value="b.window" size="small" :min="0" :max="65535" :show-button="false" style="width:100px" />
              </template>
            </div>
            <div class="nt-frow" v-if="b.transport === 'icmp-echo'">
              <label>ICMP id</label><n-input-number v-model:value="b.icmp_id" size="small" :min="0" :max="65535" :show-button="false" style="width:100px" />
              <label>seq</label><n-input-number v-model:value="b.icmp_seq" size="small" :min="0" :max="65535" :show-button="false" style="width:100px" />
            </div>
            <div class="nt-frow"><label>载荷</label>
              <n-checkbox v-model:checked="b.payload_is_hex" size="small">按 hex</n-checkbox>
              <n-input v-model:value="b.payload" size="small" :placeholder="b.payload_is_hex ? '48656c6c6f' : '任意文本'" style="flex:1;min-width:220px" />
            </div>
            <div class="nt-frow">
              <n-button size="small" type="primary" :loading="buildLoading" @click="runBuild">构造(自动算校验和)</n-button>
              <n-button size="small" :disabled="!buildResult" @click="sendToEditor('```\\n' + buildResult.hex + '\\n```\\n')">→ 编辑器</n-button>
              <span v-if="buildResult" class="nt-res">{{ buildResult.len }} 字节</span>
              <span v-if="error" class="nt-err">{{ error }}</span>
            </div>
          </div>
          <pre class="nt-hex" v-if="buildResult">{{ formatHex(buildResult.hex) }}</pre>
        </div>
      </n-tab-pane>

      <n-tab-pane name="client" tab="客户端">
        <template #tab><span class="tw-tab"><Icon :icon="sendIcon" width="15" height="15" />客户端</span></template>
        <div class="nt-form">
          <div class="nt-frow">
            <n-select v-model:value="cliProto" size="small" :options="cliProtoOptions" style="width:120px" />
            <template v-if="cliProto !== 'http'">
              <label>主机</label><n-input v-model:value="cliHost" size="small" placeholder="127.0.0.1" style="width:180px" />
              <label>端口</label><n-input-number v-model:value="cliPort" size="small" :min="1" :max="65535" :show-button="false" style="width:100px" />
              <template v-if="cliProto === 'tls'">
                <label>SNI</label><n-input v-model:value="cliSni" size="small" placeholder="留空=用主机名" style="width:150px" />
              </template>
              <n-checkbox v-model:checked="cliHex" size="small">按 hex</n-checkbox>
            </template>
            <template v-else>
              <n-select v-model:value="httpMethod" size="small" :options="httpMethodOptions" style="width:100px" />
              <n-input v-model:value="httpUrl" size="small" placeholder="https://example.com/path" style="flex:1;min-width:260px" />
            </template>
            <n-button size="small" type="primary" :loading="cliLoading" @click="runClient">发送</n-button>
          </div>

          <template v-if="cliProto !== 'http'">
            <n-input v-model:value="cliData" type="textarea" :rows="3" size="small" :placeholder="cliHex ? '48656c6c6f' : (cliProto === 'tls' ? 'GET / HTTP/1.1\\r\\nHost: example.com\\r\\n\\r\\n' : '任意字节/文本')" />
            <div v-if="cliResult" class="nt-frow"><span class="nt-res">发送 {{ cliResult.sent }} 字节 · 收到 {{ cliResult.recv_len }} 字节</span></div>
            <pre class="nt-hex" v-if="cliResult">{{ cliResult.recv_text }}</pre>
          </template>

          <template v-else>
            <div class="nt-frow">
              <span class="nt-res">请求头</span>
              <n-button size="tiny" @click="httpHeaders.push(['',''])">加一行</n-button>
            </div>
            <div v-for="(h, i) in httpHeaders" :key="i" class="nt-frow">
              <n-input v-model:value="h[0]" size="small" placeholder="Header" style="width:200px" />
              <n-input v-model:value="h[1]" size="small" placeholder="Value" style="flex:1" />
              <n-button size="tiny" text @click="httpHeaders.splice(i,1)">×</n-button>
            </div>
            <n-input v-model:value="httpBody" type="textarea" :rows="3" size="small" placeholder="请求体(可空)" />
            <div v-if="httpResp" class="nt-frow">
              <span class="nt-res" :style="{ color: httpResp.status < 400 ? 'var(--tw-accent)' : '#e5484d' }">{{ httpResp.status }} {{ httpResp.status_text }}</span>
              <span class="nt-res">{{ httpResp.body_len }} 字节 · {{ httpResp.elapsed_ms }}ms</span>
            </div>
            <pre class="nt-hex" v-if="httpResp && httpResp.headers.length">{{ httpResp.headers.map(h => h[0] + ': ' + h[1]).join('\n') }}</pre>
            <pre class="nt-hex" v-if="httpResp">{{ httpResp.body.slice(0, 20000) }}</pre>
          </template>
          <span v-if="error" class="nt-err">{{ error }}</span>
        </div>
      </n-tab-pane>

      <n-tab-pane name="server" tab="服务端">
        <template #tab><span class="tw-tab"><Icon :icon="serverIcon" width="15" height="15" />服务端</span></template>
        <div class="nt-form">
          <div class="nt-frow">
            <n-select v-model:value="srvProto" size="small" :options="srvProtoOptions" style="width:120px" :disabled="!!srvId" />
            <label>端口</label><n-input-number v-model:value="srvPort" size="small" :min="1" :max="65535" :show-button="false" style="width:100px" :disabled="!!srvId" />
            <n-checkbox v-if="srvProto !== 'http'" v-model:checked="srvEcho" size="small" :disabled="!!srvId">回显</n-checkbox>
            <template v-if="srvProto === 'http'">
              <label>状态码</label><n-input-number v-model:value="srvHttpStatus" size="small" :min="100" :max="599" :show-button="false" style="width:90px" :disabled="!!srvId" />
            </template>
            <n-button v-if="!srvId" size="small" type="primary" @click="startServer">启动监听</n-button>
            <n-button v-else size="small" @click="stopServer">停止</n-button>
            <n-button size="small" :disabled="!srvLog.length" @click="srvLog = []">清空</n-button>
            <span v-if="srvId" class="nt-res">监听中 :{{ srvPort }}</span>
            <span v-if="error" class="nt-err">{{ error }}</span>
          </div>
          <n-input v-if="srvProto === 'http'" v-model:value="srvHttpBody" type="textarea" :rows="2" size="small" placeholder="HTTP 响应体" :disabled="!!srvId" />
          <div class="nt-srvlog">
            <div v-for="(e, i) in srvLog" :key="i" class="nt-srvitem">
              <span class="nt-srvpeer">{{ e.peer }}</span>
              <span class="nt-srvtext">{{ e.data_text }}</span>
            </div>
            <div v-if="!srvLog.length" class="nt-hint">启动监听后，收到的数据会显示在这里。</div>
          </div>
        </div>
      </n-tab-pane>

      <n-tab-pane name="dns" tab="DNS">
        <template #tab><span class="tw-tab"><Icon :icon="globeIcon" width="15" height="15" />DNS</span></template>
        <div class="nt-form">
          <div class="nt-frow">
            <label>DNS 服务器</label><n-input v-model:value="dnsServer" size="small" placeholder="8.8.8.8" style="width:160px" />
            <label>域名</label><n-input v-model:value="dnsName" size="small" placeholder="example.com" style="width:220px" />
            <n-select v-model:value="dnsType" size="small" :options="dnsTypeOptions" style="width:110px" />
            <n-button size="small" type="primary" :loading="dnsLoading" @click="runDns">查询</n-button>
          </div>
          <table class="nt-kv" v-if="dnsResult && dnsResult.answers.length">
            <thead><tr><th>名称</th><th>类型</th><th>TTL</th><th>数据</th></tr></thead>
            <tbody>
              <tr v-for="(r, i) in dnsResult.answers" :key="i"><td>{{ r.name }}</td><td>{{ r.rtype }}</td><td>{{ r.ttl }}</td><td>{{ r.data }}</td></tr>
            </tbody>
          </table>
          <div v-else-if="dnsResult" class="nt-hint">无应答记录。</div>
          <span v-if="error" class="nt-err">{{ error }}</span>
        </div>
      </n-tab-pane>

      <n-tab-pane name="tls" tab="TLS">
        <template #tab><span class="tw-tab"><Icon :icon="lockIcon" width="15" height="15" />TLS</span></template>
        <div class="nt-form">
          <div class="nt-frow">
            <label>主机</label><n-input v-model:value="tlsHost" size="small" placeholder="example.com" style="width:220px" />
            <label>端口</label><n-input-number v-model:value="tlsPort" size="small" :min="1" :max="65535" :show-button="false" style="width:100px" />
            <label>SNI</label><n-input v-model:value="tlsSni" size="small" placeholder="留空=用主机名" style="width:180px" />
            <n-button size="small" type="primary" :loading="tlsLoading" @click="runTls">握手探测</n-button>
          </div>
          <table class="nt-kv" v-if="tlsReport">
            <tbody>
              <tr><td>协议版本</td><td>{{ tlsReport.version }}</td></tr>
              <tr><td>密码套件</td><td>{{ tlsReport.cipher }}</td></tr>
              <tr><td>ALPN</td><td>{{ tlsReport.alpn || '—' }}</td></tr>
            </tbody>
          </table>
          <div class="nt-layer" v-for="(c, i) in (tlsReport ? tlsReport.certs : [])" :key="i" style="margin-top:8px">
            <div class="nt-layer-name">证书 #{{ i + 1 }}</div>
            <table class="nt-kv"><tbody>
              <tr><td>主题</td><td>{{ c.subject }}</td></tr>
              <tr><td>颁发者</td><td>{{ c.issuer }}</td></tr>
              <tr><td>有效期</td><td>{{ c.not_before }} ~ {{ c.not_after }}</td></tr>
            </tbody></table>
          </div>
          <span v-if="error" class="nt-err">{{ error }}</span>
        </div>
      </n-tab-pane>

      <n-tab-pane name="capture" tab="抓包">
        <template #tab><span class="tw-tab"><Icon :icon="radarIcon" width="15" height="15" />抓包</span></template>
        <div class="nt-form">
          <div v-if="pcapStatus && !pcapStatus.installed" class="nt-pcap-guide">
            <Icon :icon="alertIcon" width="18" height="18" />
            <div>
              <div class="nt-pcap-title">未检测到抓包驱动 Npcap</div>
              <div class="nt-pcap-msg">{{ pcapStatus.message }}</div>
              <div class="nt-pcap-hint">安装 <a href="https://npcap.com/#download" target="_blank">Npcap</a>(或安装 Wireshark 会自带),安装时勾选 “WinPcap API-compatible mode”,重启软件后即可抓包。其它网络功能(客户端/服务端/DNS/TLS/解析/构造)不受影响。</div>
              <n-button size="small" @click="refreshPcap" style="margin-top:6px">重新检测</n-button>
            </div>
          </div>
          <div class="nt-frow">
            <n-button size="small" @click="loadIfaces" :disabled="pcapStatus && !pcapStatus.installed">刷新网卡</n-button>
            <n-select v-model:value="capDev" size="small" :options="ifaceOptions" placeholder="选择网卡" style="min-width:260px" :disabled="!!capId || (pcapStatus && !pcapStatus.installed)" />
            <n-input v-model:value="capFilter" size="small" placeholder="BPF 过滤(可空)，如 tcp port 80" style="width:220px" :disabled="!!capId" />
            <n-button v-if="!capId" size="small" type="primary" :disabled="!capDev" @click="startCapture">开始抓包</n-button>
            <n-button v-else size="small" @click="stopCapture">停止</n-button>
            <n-button size="small" :disabled="!capLog.length" @click="capLog = []">清空</n-button>
            <span v-if="error" class="nt-err">{{ error }}</span>
          </div>
          <div class="nt-hint">点某个包可跳到"报文解析"查看分层。</div>
          <div class="nt-caplog">
            <div v-for="p in capLog" :key="p.index" class="nt-capitem" @click="parseCaptured(p.hex)">
              <span class="nt-capidx">#{{ p.index }}</span>
              <span class="nt-caplen">{{ p.len }}B</span>
              <span class="nt-caphex">{{ p.hex.slice(0, 96) }}{{ p.hex.length > 96 ? '…' : '' }}</span>
            </div>
            <div v-if="!capLog.length" class="nt-hint">开始抓包后,数据包列表显示在这里。</div>
          </div>
        </div>
      </n-tab-pane>
    </n-tabs>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Icon } from '@iconify/vue'
import searchIcon from '@iconify-icons/lucide/scan-search'
import hammerIcon from '@iconify-icons/lucide/hammer'
import sendIcon from '@iconify-icons/lucide/send'
import serverIcon from '@iconify-icons/lucide/server'
import globeIcon from '@iconify-icons/lucide/globe'
import lockIcon from '@iconify-icons/lucide/lock'
import radarIcon from '@iconify-icons/lucide/radar'
import alertIcon from '@iconify-icons/lucide/alert-triangle'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('RemoteNetworkToolbox')
const error = ref('')
const activeNetTab = ref('parse')

const parseLayer = ref('ip')
const parseInput = ref('')
const parseResult = ref(null)
const parseLoading = ref(false)
const sampleTcp = '4500003400004000400600000a0000010a000002' + 'd431005000000000000000008002faf000000000020405b40103030801010402'

async function runParse() {
  error.value = ''; parseResult.value = null
  if (!parseInput.value.trim()) return
  parseLoading.value = true
  try {
    parseResult.value = await invoke('net_parse_packet', { input: parseInput.value, hasEthernet: parseLayer.value === 'eth' })
  } catch (e) { error.value = '解析失败: ' + (e?.message || e) } finally { parseLoading.value = false }
}

const transportOptions = [
  { label: 'TCP', value: 'tcp' },
  { label: 'UDP', value: 'udp' },
  { label: 'ICMP Echo 请求', value: 'icmp-echo' },
]
const b = reactive({ transport: 'tcp', ip_src: '10.0.0.1', ip_dst: '10.0.0.2', ttl: 64, src_port: 12345, dst_port: 80, seq: 0, window: 65535, icmp_id: 1, icmp_seq: 1, payload: '', payload_is_hex: false })
const buildResult = ref(null)
const buildLoading = ref(false)

async function runBuild() {
  error.value = ''; buildResult.value = null
  buildLoading.value = true
  try {
    buildResult.value = await invoke('net_build_packet', { spec: { ...b } })
  } catch (e) { error.value = '构造失败: ' + (e?.message || e) } finally { buildLoading.value = false }
}

function formatHex(h) {
  const bytes = h.match(/.{1,2}/g) || []
  const lines = []
  for (let i = 0; i < bytes.length; i += 16) {
    const chunk = bytes.slice(i, i + 16)
    const off = i.toString(16).padStart(4, '0')
    const hexPart = chunk.join(' ')
    const ascii = chunk.map(x => { const n = parseInt(x, 16); return n >= 32 && n < 127 ? String.fromCharCode(n) : '.' }).join('')
    lines.push(`${off}  ${hexPart.padEnd(48)}  ${ascii}`)
  }
  return lines.join('\n')
}

async function sendToEditor(content) {
  if (!content) return
  try { await invoke('emit_template_insert', { content }) }
  catch (e) { error.value = '发送失败: ' + (e?.message || e) }
}

// 客户端
const cliProto = ref('tcp'); const cliHost = ref('127.0.0.1'); const cliPort = ref(80); const cliSni = ref('')
const cliData = ref(''); const cliHex = ref(false); const cliResult = ref(null); const cliLoading = ref(false)
const cliProtoOptions = [
  { label: 'TCP', value: 'tcp' }, { label: 'UDP', value: 'udp' },
  { label: 'TLS', value: 'tls' }, { label: 'HTTP(S)', value: 'http' },
]
const httpMethod = ref('GET'); const httpUrl = ref('https://'); const httpHeaders = ref([]); const httpBody = ref(''); const httpResp = ref(null)
const httpMethodOptions = ['GET', 'POST', 'PUT', 'DELETE', 'HEAD', 'PATCH', 'OPTIONS'].map(v => ({ label: v, value: v }))
async function runClient() {
  error.value = ''; cliLoading.value = true
  try {
    if (cliProto.value === 'http') {
      httpResp.value = null
      httpResp.value = await invoke('net_http_request', {
        req: {
          method: httpMethod.value, url: httpUrl.value,
          headers: httpHeaders.value.filter(h => h[0].trim()),
          body: httpBody.value || null, timeout_ms: 10000,
        },
      })
    } else if (cliProto.value === 'tls') {
      cliResult.value = null
      cliResult.value = await invoke('net_tls_send', { host: cliHost.value, port: cliPort.value, sni: cliSni.value || null, data: cliData.value, isHex: cliHex.value, timeoutMs: 5000 })
    } else {
      cliResult.value = null
      const cmd = cliProto.value === 'udp' ? 'net_udp_send' : 'net_tcp_send'
      cliResult.value = await invoke(cmd, { host: cliHost.value, port: cliPort.value, data: cliData.value, isHex: cliHex.value, timeoutMs: 3000 })
    }
  } catch (e) { error.value = '发送失败: ' + (e?.message || e) } finally { cliLoading.value = false }
}

// 服务端
const srvProto = ref('tcp'); const srvPort = ref(9000); const srvEcho = ref(true)
const srvHttpStatus = ref(200); const srvHttpBody = ref('Hello from Guaxi net toolbox')
const srvProtoOptions = [
  { label: 'TCP', value: 'tcp' }, { label: 'UDP', value: 'udp' }, { label: 'HTTP', value: 'http' },
]
const srvId = ref(''); const srvLog = ref([])
let srvUnlisten = null
async function startServer() {
  error.value = ''
  try {
    if (srvProto.value === 'http') {
      srvId.value = await invoke('net_http_listen', { port: srvPort.value, status: srvHttpStatus.value, body: srvHttpBody.value })
    } else {
      const cmd = srvProto.value === 'udp' ? 'net_udp_listen' : 'net_tcp_listen'
      srvId.value = await invoke(cmd, { port: srvPort.value, echo: srvEcho.value })
    }
    if (!srvUnlisten) {
      srvUnlisten = await listen('net-server-event', ev => {
        if (ev.payload?.session_id === srvId.value) {
          srvLog.value.push(ev.payload)
          if (srvLog.value.length > 1000) srvLog.value.splice(0, srvLog.value.length - 1000)
        }
      })
    }
  } catch (e) { error.value = '启动失败: ' + (e?.message || e) }
}
async function stopServer() {
  if (srvId.value) { await invoke('net_server_stop', { sessionId: srvId.value }).catch(() => {}); srvId.value = '' }
}

// DNS
const dnsServer = ref('8.8.8.8'); const dnsName = ref('example.com'); const dnsType = ref('A')
const dnsResult = ref(null); const dnsLoading = ref(false)
const dnsTypeOptions = ['A', 'AAAA', 'CNAME', 'MX', 'TXT', 'NS', 'SOA', 'PTR', 'SRV', 'CAA'].map(v => ({ label: v, value: v }))
async function runDns() {
  error.value = ''; dnsResult.value = null; dnsLoading.value = true
  try { dnsResult.value = await invoke('net_dns_query', { server: dnsServer.value, name: dnsName.value, qtype: dnsType.value }) }
  catch (e) { error.value = 'DNS 查询失败: ' + (e?.message || e) } finally { dnsLoading.value = false }
}

// TLS
const tlsHost = ref('example.com'); const tlsPort = ref(443); const tlsSni = ref('')
const tlsReport = ref(null); const tlsLoading = ref(false)
async function runTls() {
  error.value = ''; tlsReport.value = null; tlsLoading.value = true
  try { tlsReport.value = await invoke('net_tls_inspect', { host: tlsHost.value, port: tlsPort.value, sni: tlsSni.value || null }) }
  catch (e) { error.value = 'TLS 探测失败: ' + (e?.message || e) } finally { tlsLoading.value = false }
}

// 抓包 (Npcap, 运行时探测)
const pcapStatus = ref(null)
async function refreshPcap() {
  try { pcapStatus.value = await invoke('net_pcap_status') }
  catch (e) { pcapStatus.value = { installed: false, message: '' + (e?.message || e) } }
}
const ifaceOptions = ref([]); const capDev = ref(null); const capFilter = ref('')
const capId = ref(''); const capLog = ref([])
let capUnlisten = null
async function loadIfaces() {
  error.value = ''
  try {
    const list = await invoke('net_list_interfaces')
    ifaceOptions.value = list.map(i => ({ label: `${i.desc || i.name}`, value: i.name }))
  } catch (e) { error.value = '' + (e?.message || e) }
}
async function startCapture() {
  error.value = ''
  if (!capDev.value) return
  try {
    capId.value = await invoke('net_capture_start', { device: capDev.value, filter: capFilter.value || null, maxPackets: 5000 })
    if (!capUnlisten) {
      capUnlisten = await listen('net-capture-packet', ev => {
        if (ev.payload?.session_id === capId.value) {
          capLog.value.push(ev.payload)
          if (capLog.value.length > 5000) capLog.value.splice(0, capLog.value.length - 5000)
        }
      })
    }
  } catch (e) { error.value = '' + (e?.message || e); capId.value = '' }
}
async function stopCapture() {
  if (capId.value) { await invoke('net_capture_stop', { sessionId: capId.value }).catch(() => {}); capId.value = '' }
}
function parseCaptured(hexStr) {
  parseInput.value = hexStr
  parseLayer.value = 'eth'
  activeNetTab.value = 'parse'
  runParse()
}

onMounted(() => { refreshPcap() })

onBeforeUnmount(() => {
  if (srvId.value) invoke('net_server_stop', { sessionId: srvId.value }).catch(() => {})
  if (srvUnlisten) srvUnlisten()
  if (capId.value) invoke('net_capture_stop', { sessionId: capId.value }).catch(() => {})
  if (capUnlisten) capUnlisten()
})
</script>

<style scoped>
.nt-root { height: 100%; display: flex; flex-direction: column; min-height: 0; font-family: var(--tw-ui-font); }
.nt-tabs { height: 100%; display: flex; flex-direction: column; }
.nt-tabs :deep(.nt-pane) { height: 100%; overflow: auto; padding: 12px 14px; }
.nt-tabs :deep(.n-tabs-tab--active) { color: var(--tw-accent) !important; }
.nt-tabs :deep(.n-tabs-bar) { background: var(--tw-accent) !important; }
.tw-tab { display: inline-flex; align-items: center; gap: 7px; font-weight: 500; }
.nt-toolbar { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; flex-wrap: wrap; }
.nt-err { color: #e5484d; font-size: 15px; }
.nt-res { color: var(--tw-accent); font-size: 15px; font-family: var(--tw-mono-font); }
.nt-parse-body { margin-top: 10px; }
.nt-layers { display: flex; flex-direction: column; gap: 10px; }
.nt-layer { border: 1px solid color-mix(in srgb, var(--guaxi-border) 65%, transparent); border-radius: var(--tw-radius-sm); overflow: hidden; }
.nt-layer-name { background: var(--tw-accent-soft); color: var(--tw-accent); font-weight: 600; font-size: 15px; padding: 6px 12px; }
.nt-kv { width: 100%; border-collapse: collapse; font-size: 15px; font-family: var(--tw-mono-font); }
.nt-kv td { padding: 4px 12px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.05)); }
.nt-kv td:first-child { color: var(--guaxi-text-secondary); width: 140px; }
.nt-hex { background: var(--tw-code-bg); color: var(--tw-code-fg); font-family: var(--tw-mono-font); font-size: 15px; padding: 10px; border-radius: var(--tw-radius-sm); white-space: pre; overflow: auto; margin: 8px 0 0; }
.nt-form { display: flex; flex-direction: column; gap: 10px; }
.nt-frow { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.nt-frow > label { font-size: 15px; color: var(--guaxi-text-secondary); }
.nt-kv th { text-align: left; padding: 4px 12px; font-size: 14px; text-transform: uppercase; color: var(--guaxi-text-secondary); border-bottom: 1px solid var(--guaxi-border); }
.nt-srvlog { margin-top: 8px; border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius-sm); max-height: 50vh; overflow: auto; font-family: var(--tw-mono-font); font-size: 15px; }
.nt-srvitem { display: flex; gap: 10px; padding: 5px 10px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.05)); }
.nt-srvpeer { color: var(--tw-accent); flex: 0 0 auto; }
.nt-srvtext { white-space: pre-wrap; word-break: break-all; }
.nt-hint { color: var(--guaxi-text-secondary); font-size: 15px; padding: 12px; }
.nt-caplog { margin-top: 8px; border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius-sm); max-height: 55vh; overflow: auto; font-family: var(--tw-mono-font); font-size: 15px; }
.nt-capitem { display: flex; gap: 10px; padding: 4px 10px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,0.05)); cursor: pointer; }
.nt-capitem:hover { background: var(--tw-accent-soft); }
.nt-capidx { color: var(--tw-accent); flex: 0 0 auto; width: 48px; }
.nt-caplen { color: var(--guaxi-text-secondary); flex: 0 0 auto; width: 56px; }
.nt-caphex { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.nt-pcap-guide { display: flex; gap: 10px; padding: 12px 14px; margin-bottom: 10px; border: 1px solid #f0a020; border-radius: var(--tw-radius-sm); background: rgba(240,160,32,0.10); color: var(--guaxi-text-primary); }
.nt-pcap-guide > :first-child { color: #f0a020; flex: 0 0 auto; margin-top: 2px; }
.nt-pcap-title { font-weight: 600; margin-bottom: 3px; }
.nt-pcap-msg { font-size: 15px; color: var(--guaxi-text-secondary); }
.nt-pcap-hint { font-size: 15px; margin-top: 4px; line-height: 1.5; }
.nt-pcap-hint a { color: var(--tw-accent); }
</style>
