<template>
  <div class="mail-modal-backdrop" @click.self="$emit('close')">
    <div class="mail-modal is-wide" role="dialog" aria-label="账户管理">
      <div class="mail-modal-head">
        <h2 class="mail-modal-title">{{ editing ? '编辑账户' : '添加邮箱账户' }}</h2>
        <button class="mail-modal-close" @click="$emit('close')">×</button>
      </div>

      <div class="mail-modal-body">
        <!-- 已有账户 -->
        <template v-if="!editing">
          <div v-for="account in accounts" :key="account.id" class="mail-account-row">
            <span class="mail-account-dot" :style="{ background: account.accent }"></span>
            <div class="meta">
              <span class="email">{{ account.email }}</span>
              <span class="servers">
                IMAP {{ account.imapHost }}:{{ account.imapPort }} · SMTP
                {{ account.smtpHost }}:{{ account.smtpPort }}
              </span>
            </div>
            <button class="mail-btn" @click="$emit('select', account.id)">切换</button>
            <button class="mail-btn" @click="startEdit(account)">编辑</button>
            <button class="mail-btn is-danger" @click="remove(account)">删除</button>
          </div>

          <div v-if="!accounts.length" class="mail-hint" style="margin-bottom: 14px">
            还没有账户。下面选一个常用邮箱，其余字段会自动填好。
          </div>

          <button class="mail-btn is-primary" @click="startNew">＋ 添加账户</button>
        </template>

        <!-- 编辑表单 -->
        <template v-else>
          <div class="mail-preset">
            <button
              v-for="preset in presets"
              :key="preset.label"
              class="mail-chip"
              @click="applyPreset(preset)"
            >
              {{ preset.label }}
            </button>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">邮箱地址</label>
              <input v-model="form.email" class="mail-input" placeholder="you@example.com" />
            </div>
            <div class="mail-field">
              <label class="mail-flabel">显示名称</label>
              <input v-model="form.displayName" class="mail-input" placeholder="张三" />
            </div>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">登录用户名（留空 = 邮箱地址）</label>
              <input v-model="form.username" class="mail-input" placeholder="可选" />
            </div>
            <div class="mail-field">
              <label class="mail-flabel">认证方式</label>
              <select v-model="form.authKind" class="mail-input">
                <option value="password">密码 / 授权码</option>
                <option value="oauth2">OAuth2（Gmail / Outlook 推荐）</option>
              </select>
            </div>
          </div>

          <!-- OAuth2：设备码流程 -->
          <div v-if="form.authKind === 'oauth2'" class="mail-oauth-box">
            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">服务商</label>
                <select v-model="form.oauthProvider" class="mail-input">
                  <option value="gmail">Google / Gmail</option>
                  <option value="outlook">Microsoft / Outlook</option>
                </select>
              </div>
              <div class="mail-field">
                <label class="mail-flabel">授权状态</label>
                <div class="mail-oauth-state">
                  <template v-if="oauthState === 'done'">✓ 已授权</template>
                  <template v-else-if="oauthState === 'waiting'">等待你在浏览器里输入代码…</template>
                  <template v-else-if="oauthState === 'error'" class="bad">{{ oauthError }}</template>
                  <template v-else>未授权</template>
                </div>
              </div>
            </div>
            <template v-if="deviceCode">
              <p class="mail-hint" style="margin: 8px 0">
                在浏览器打开下方网址，输入代码后回到这里等待完成（通常几秒内）。
              </p>
              <div class="mail-device-code">
                <code>{{ deviceCode.userCode }}</code>
                <button class="mail-btn" @click="openVerificationUrl">打开授权页</button>
                <button class="mail-btn" :disabled="oauthState === 'waiting'" @click="pollAuth">
                  我已经授权了
                </button>
              </div>
              <p class="mail-hint" style="margin: 4px 0 0">{{ deviceCode.verificationUrl }}</p>
            </template>
            <button
              v-else
              class="mail-btn is-primary"
              style="margin-top: 8px"
              :disabled="oauthState === 'starting'"
              @click="startAuth"
            >
              {{ oauthState === 'starting' ? '正在请求…' : '开始授权' }}
            </button>
            <p class="mail-hint" style="margin: 8px 0 0">
              令牌只存在系统凭据库，不会写进本地数据库；备份邮件库不会带走它。
            </p>
          </div>

          <div v-else class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">
                {{ secretStored ? '密码 / 授权码（已保存，留空表示不改）' : '密码 / 授权码' }}
              </label>
              <input
                v-model="form.secret"
                class="mail-input"
                type="password"
                :placeholder="secretStored ? '已保存' : 'QQ/163 需要授权码，不是登录密码'"
              />
            </div>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">IMAP 服务器</label>
              <input v-model="form.imapHost" class="mail-input" placeholder="imap.example.com" />
            </div>
            <div class="mail-field">
              <label class="mail-flabel">端口 / 加密</label>
              <div style="display: flex; gap: 6px">
                <input v-model.number="form.imapPort" class="mail-input" type="number" style="width: 90px" />
                <select v-model="form.imapSecurity" class="mail-input">
                  <option value="ssl">SSL/TLS</option>
                  <option value="starttls">STARTTLS</option>
                  <option value="none">不加密</option>
                </select>
              </div>
            </div>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">SMTP 服务器</label>
              <input v-model="form.smtpHost" class="mail-input" placeholder="smtp.example.com" />
            </div>
            <div class="mail-field">
              <label class="mail-flabel">端口 / 加密</label>
              <div style="display: flex; gap: 6px">
                <input v-model.number="form.smtpPort" class="mail-input" type="number" style="width: 90px" />
                <select v-model="form.smtpSecurity" class="mail-input">
                  <option value="ssl">SSL/TLS</option>
                  <option value="starttls">STARTTLS</option>
                  <option value="none">不加密</option>
                </select>
              </div>
            </div>
          </div>

          <div class="mail-field">
            <label class="mail-flabel">签名</label>
            <textarea v-model="form.signature" class="mail-textarea" rows="3" placeholder="署名 / 联系方式，会附在每封邮件末尾"></textarea>
          </div>

          <!-- 共享邮箱 / 委托访问 -->
          <div class="mail-field">
            <label class="mail-flabel">共享邮箱 / 委托访问（团队收件箱）</label>
            <p class="mail-hint" style="margin: 0 0 6px">
              用你的凭据登录，直接读取被委托的邮箱（Exchange / Office 365 / Dovecot
              支持；QQ / 163 不支持）。填了之后，侧栏里会多出这些邮箱的文件夹。
            </p>
            <div v-for="(item, index) in form.delegate" :key="index" class="mail-delegate-row">
              <input v-model="item.email" class="mail-input" placeholder="team@corp.com" />
              <input v-model="item.displayName" class="mail-input" placeholder="显示名（可选）" />
              <button class="mail-btn is-danger" @click="form.delegate.splice(index, 1)">移除</button>
            </div>
            <button class="mail-btn" style="margin-top: 6px" @click="addDelegate">
              ＋ 添加共享邮箱
            </button>
          </div>

          <!-- 服务器端规则（Sieve） -->
          <div class="mail-field">
            <label class="mail-flabel">服务器端规则（Sieve，可选）</label>
            <div style="display: flex; gap: 6px; align-items: center; flex-wrap: wrap">
              <select v-model="form.sieve.mode" class="mail-input" style="width: 140px">
                <option value="off">不使用</option>
                <option value="managesieve">ManageSieve</option>
              </select>
              <input
                v-if="form.sieve.mode !== 'off'"
                v-model="form.sieve.host"
                class="mail-input"
                placeholder="留空 = 与 IMAP 同主机"
              />
              <input
                v-if="form.sieve.mode !== 'off'"
                v-model.number="form.sieve.port"
                class="mail-input"
                type="number"
                style="width: 90px"
                placeholder="4190"
              />
            </div>
            <span class="mail-hint">
              只有支持 ManageSieve 的服务器（Fastmail、Dovecot、部分企业邮）可用；
              QQ / 163 / Exchange 不支持，选了也连不上。
            </span>
          </div>

          <!-- 代理：账户级覆盖全局 -->
          <div class="mail-field">
            <label class="mail-flabel">
              <input
                v-model="overrideProxy"
                type="checkbox"
                style="vertical-align: -1px; margin-right: 6px"
              />
              为这个账户单独设置代理
            </label>
            <template v-if="overrideProxy">
              <div style="display: flex; gap: 6px; margin-top: 6px">
                <select v-model="form.proxy.mode" class="mail-input" style="width: 140px">
                  <option value="none">直连</option>
                  <option value="system">跟随系统</option>
                  <option value="custom">自定义</option>
                </select>
                <input
                  v-if="form.proxy.mode === 'custom'"
                  v-model="form.proxy.url"
                  class="mail-input"
                  placeholder="socks5://127.0.0.1:1080 或 http://127.0.0.1:7890"
                />
              </div>
              <input
                v-model="bypassText"
                class="mail-input"
                style="margin-top: 6px"
                placeholder="不走代理的主机，逗号分隔（如 corp.local）"
              />
            </template>
            <span v-else class="mail-hint">
              默认沿用「外观与设置」里的全局代理。这里的设置只影响本账户的收信与发信。
            </span>
          </div>

          <div v-if="probeResult" class="mail-probe">
            <div v-for="(probe, index) in probeResult" :key="index">
              <span :class="probe.ok ? 'ok' : 'bad'">
                {{ index === 0 ? 'IMAP' : 'SMTP' }}{{ probe.ok ? ' 正常' : ' 失败' }}
              </span>
              — {{ probe.detail }}
            </div>
          </div>
        </template>
      </div>

      <div class="mail-modal-foot">
        <template v-if="editing">
          <button class="mail-btn" :disabled="testing" @click="test">
            {{ testing ? '诊断中…' : '测试连接' }}
          </button>
          <span class="spacer"></span>
          <button class="mail-btn" @click="cancel">取消</button>
          <button class="mail-btn is-primary" :disabled="!canSave" @click="save">保存并同步</button>
        </template>
        <template v-else>
          <span class="spacer"></span>
          <button class="mail-btn" @click="$emit('close')">关闭</button>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'

const props = defineProps({
  accounts: { type: Array, default: () => [] },
  settings: { type: Object, default: () => ({}) },
  activeAccountId: { type: String, default: null },
  // Vue 的 emit 不回传处理函数的返回值，所以「测试连接」「是否已存口令」这两件
  // 需要拿结果的事情用函数 props 传进来。
  testFn: { type: Function, default: null },
  hasSecretFn: { type: Function, default: null },
  /** OAuth 三步：发起设备码 → 轮询换令牌 → 查状态。由 store 提供。 */
  oauthStartFn: { type: Function, default: null },
  oauthPollFn: { type: Function, default: null },
})
const emit = defineEmits(['close', 'saved', 'deleted', 'select'])

const presets = [
  { label: 'QQ 邮箱', email: '', imapHost: 'imap.qq.com', imapPort: 993, smtpHost: 'smtp.qq.com', smtpPort: 465, hint: '需要 QQ 邮箱设置里生成的授权码' },
  { label: '163 邮箱', email: '', imapHost: 'imap.163.com', imapPort: 993, smtpHost: 'smtp.163.com', smtpPort: 465, hint: '需要客户端授权密码' },
  { label: '126 邮箱', email: '', imapHost: 'imap.126.com', imapPort: 993, smtpHost: 'smtp.126.com', smtpPort: 465, hint: '需要客户端授权密码' },
  { label: 'Gmail', email: '', imapHost: 'imap.gmail.com', imapPort: 993, smtpHost: 'smtp.gmail.com', smtpPort: 465, hint: '需要开启两步验证并使用应用专用密码' },
  { label: 'Outlook', email: '', imapHost: 'outlook.office365.com', imapPort: 993, smtpHost: 'smtp.office365.com', smtpPort: 587, smtpSecurity: 'starttls', hint: '微软账户需要在安全设置里开启 IMAP' },
  { label: '腾讯企业邮', email: '', imapHost: 'imap.exmail.qq.com', imapPort: 993, smtpHost: 'smtp.exmail.qq.com', smtpPort: 465, hint: '登录名可能是完整邮箱' },
  { label: '阿里企业邮', email: '', imapHost: 'imap.qiye.aliyun.com', imapPort: 993, smtpHost: 'smtp.qiye.aliyun.com', smtpPort: 465, hint: '' },
]

const editing = ref(false)
const testing = ref(false)
const probeResult = ref(null)
const secretStored = ref(false)
const customMode = ref(false)
const overrideProxy = ref(false)
const bypassText = ref('')
// OAuth2 授权流程的界面状态
const oauthState = ref('idle')
const oauthError = ref('')
const deviceCode = ref(null)

const blank = () => ({
  id: '',
  email: '',
  displayName: '',
  imapHost: '',
  imapPort: 993,
  imapSecurity: 'ssl',
  smtpHost: '',
  smtpPort: 465,
  smtpSecurity: 'ssl',
  username: '',
  authKind: 'password',
  oauthProvider: 'gmail',
  oauthExpires: 0,
  accent: pickAccent(),
  signature: '',
  folderMap: {},
  createdAt: 0,
  lastSyncAt: 0,
  proxy: { mode: 'system', url: '', bypass: [] },
  delegate: [],
  sieve: { mode: 'off', host: '', port: 4190, security: 'starttls', script: 'guaxi', lastSyncAt: 0, lastError: '' },
  secret: '',
})

const form = reactive(blank())

watch(overrideProxy, (on) => {
  if (!on) return
  const global = props.settings?.proxy || {}
  form.proxy = {
    mode: global.mode || 'system',
    url: global.url || '',
    bypass: [...(global.bypass || [])],
  }
  bypassText.value = (form.proxy.bypass || []).join(', ')
})

watch(bypassText, (text) => {
  if (!overrideProxy.value) return
  form.proxy.bypass = String(text || '')
    .split(/[,;，；]/)
    .map((s) => s.trim())
    .filter(Boolean)
})

watch(
  () => form.id,
  async (id) => {
    if (!editing.value || !id) {
      secretStored.value = false
      return
    }
    secretStored.value = await hasSecret(id)
  },
)

const canSave = computed(
  () => form.email.trim() && form.imapHost.trim() && form.smtpHost.trim(),
)

function pickAccent() {
  const palette = ['#5b7cfa', '#d97757', '#2f9e6e', '#b46bd6', '#c98a18', '#2f8fbf']
  return palette[props.accounts.length % palette.length]
}

function addDelegate() {
  form.delegate.push({ email: '', username: '', displayName: '', linkedAccountId: '' })
}

function startNew() {
  Object.assign(form, blank())
  editing.value = true
  oauthState.value = 'idle'
  deviceCode.value = null
  probeResult.value = null
  secretStored.value = false
  overrideProxy.value = false
  bypassText.value = ''
}

function startEdit(account) {
  customMode.value = false
  Object.assign(form, blank(), {
    ...account,
    proxy: account.proxy ? { ...account.proxy } : { mode: 'system', url: '', bypass: [] },
    // 老账户没有这些字段：统一补成合法结构，避免模板里访问 undefined。
    delegate: (account.delegate || []).map((d) => ({ ...d })),
    sieve: {
      mode: 'off',
      host: '',
      port: 4190,
      security: 'starttls',
      script: 'guaxi',
      lastSyncAt: 0,
      lastError: '',
      ...(account.sieve || {}),
    },
    secret: '',
  })
  editing.value = true
  probeResult.value = null
  overrideProxy.value = Boolean(account.proxy)
  bypassText.value = (account.proxy?.bypass || []).join(', ')
  hasSecret(account.id).then((v) => (secretStored.value = v))
}

function cancel() {
  editing.value = false
  probeResult.value = null
}

/** 自定义 / 内网邮箱：清空预填，全部字段由用户填。 */
function startCustom() {
  customMode.value = true
  form.imapHost = ''
  form.imapPort = 993
  form.imapSecurity = 'ssl'
  form.smtpHost = ''
  form.smtpPort = 465
  form.smtpSecurity = 'ssl'
}

function applyPreset(preset) {
  customMode.value = false
  form.imapHost = preset.imapHost
  form.imapPort = preset.imapPort
  form.imapSecurity = 'ssl'
  form.smtpHost = preset.smtpHost
  form.smtpPort = preset.smtpPort
  form.smtpSecurity = preset.smtpSecurity || 'ssl'
}

async function startAuth() {
  if (!props.oauthStartFn) return
  oauthState.value = 'starting'
  oauthError.value = ''
  try {
    deviceCode.value = await props.oauthStartFn(form.oauthProvider)
    oauthState.value = 'waiting'
    // 自动轮询：用户不用手动点「我已经授权了」，但保留按钮作为兜底。
    pollAuth().catch(() => {})
  } catch (error) {
    oauthState.value = 'error'
    oauthError.value = String(error?.message || error)
  }
}

async function pollAuth() {
  if (!deviceCode.value || !props.oauthPollFn) return
  oauthState.value = 'waiting'
  oauthError.value = ''
  try {
    // 先确保账户已经存在于库里：令牌要挂在账户 id 上。
    if (!form.id) {
      await save(false)
    }
    await props.oauthPollFn(form.id, form.oauthProvider, deviceCode.value)
    oauthState.value = 'done'
  } catch (error) {
    oauthState.value = 'error'
    oauthError.value = String(error?.message || error)
  }
}

async function openVerificationUrl() {
  const url = deviceCode.value?.verificationUrl
  if (!url) return
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(url)
  } catch {
    window.open(url, '_blank')
  }
}

async function hasSecret(id) {
  if (!id || !props.hasSecretFn) return false
  try {
    return Boolean(await props.hasSecretFn(id))
  } catch {
    return false
  }
}

async function test() {
  if (!props.testFn) return
  testing.value = true
  probeResult.value = null
  try {
    // 表单里刚填的口令还没写进 keychain：先存一次再测，否则诊断一定失败。
    // 用临时 id 存会在凭据库里留下垃圾条目，所以只在「已有账户」时这么做；
    // 新账户直接带着明文口令去探测（testFn 内部会用表单里的口令）。
    const account = { ...form, proxy: overrideProxy.value ? { ...form.proxy } : null }
    probeResult.value = await props.testFn(account, form.secret)
  } catch (error) {
    probeResult.value = [{ ok: false, detail: String(error?.message || error), stage: 'error' }]
  } finally {
    testing.value = false
  }
}

async function save(closeAfter = true) {
  const account = {
    ...form,
    proxy: overrideProxy.value ? { ...form.proxy } : null,
  }
  delete account.secret
  emit('saved', { account, secret: form.secret, closeAfter })
}

function remove(account) {
  if (!window.confirm(`删除账户 ${account.email}？本地缓存的邮件也会一并清掉（服务器上的不受影响）。`)) {
    return
  }
  emit('deleted', account.id)
}
</script>
