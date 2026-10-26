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
                {{ account.protocol === 'pop3' ? 'POP3' : 'IMAP' }}
                {{ account.protocol === 'pop3' ? account.pop3Host : account.imapHost }}:{{
                  account.protocol === 'pop3' ? account.pop3Port : account.imapPort
                }}
                · SMTP {{ account.smtpHost }}:{{ account.smtpPort }}
              </span>
            </div>
            <button class="mail-btn" @click="$emit('select', account.id)">切换</button>
            <button class="mail-btn" @click="startEdit(account)">编辑</button>
            <button class="mail-btn is-danger" @click="remove(account)">删除</button>
          </div>

          <div v-if="!accounts.length" class="mail-hint" style="margin-bottom: 14px">
            还没有账户。选一个常用邮箱会自动填好服务器；公司内部或自建邮件系统请点「自定义 / 公司邮箱」，
            IMAP 与 SMTP 地址、端口和加密方式都可以手填。
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
            <!-- 公司内部邮件系统 / 自建 IMAP/SMTP：清空服务器预填，所有字段手填。 -->
            <button class="mail-chip" @click="startCustom">自定义 / 公司邮箱</button>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">邮箱地址</label>
              <input v-model="form.email" class="mail-input" placeholder="you@example.com" />
              <span v-if="form.email.trim() && !emailValid" class="mail-hint" style="color: var(--mail-danger)">
                请填写完整邮箱地址（例如 name@example.com），缺域名会连不上服务器
              </span>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">显示名称</label>
              <input v-model="form.displayName" class="mail-input" placeholder="张三" />
            </div>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">收信协议</label>
              <div class="mail-protocol-toggle">
                <button
                  v-for="option in PROTOCOL_OPTIONS"
                  :key="option.value"
                  type="button"
                  class="mail-chip"
                  :class="{ 'is-on': form.protocol === option.value }"
                  :title="option.hint"
                  @click="setProtocol(option.value)"
                >
                  {{ option.label }}
                </button>
              </div>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">登录用户名（留空 = 邮箱地址）</label>
              <input v-model="form.username" class="mail-input" placeholder="可选" />
            </div>
          </div>

          <div class="mail-field-row">
            <div class="mail-field">
              <label class="mail-flabel">认证方式</label>
              <select v-model="form.authKind" class="mail-input" :disabled="form.protocol === 'pop3'">
                <option value="password">密码 / 授权码</option>
                <option value="oauth2">OAuth2（Gmail / Outlook 推荐）</option>
              </select>
              <span v-if="form.protocol === 'pop3'" class="mail-hint">
                POP3 只能用密码 / 授权码：服务商没有为它提供 OAuth2 端点。
              </span>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">
                {{ form.protocol === 'pop3' ? '本地保留' : '同步范围' }}
              </label>
              <span class="mail-hint mail-protocol-note">
                {{
                  form.protocol === 'pop3'
                    ? '下载到本机后从本地库读取，已发送 / 草稿 / 删除都只在本机。'
                    : '文件夹、已读与星标都保存在服务器上，多设备一致。'
                }}
              </span>
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
                  <template v-else-if="oauthState === 'waiting'">等待你在浏览器里完成登录…</template>
                  <template v-else-if="oauthState === 'error'">
                    <span class="bad">{{ oauthError }}</span>
                  </template>
                  <template v-else>未授权</template>
                </div>
              </div>
            </div>
            <template v-if="deviceCode">
              <p class="mail-hint" style="margin: 8px 0">
                {{
                  deviceCode.userCode
                    ? '在浏览器打开下方网址，输入代码后回到这里等待完成（通常几秒内）。'
                    : '在浏览器打开下方网址并完成 Microsoft 登录；看到「授权完成」后回到这里，应用会自动继续。'
                }}
              </p>
              <div class="mail-device-code">
                <code v-if="deviceCode.userCode">{{ deviceCode.userCode }}</code>
                <button class="mail-btn" @click="openVerificationUrl">打开授权页</button>
                <button
                  v-if="deviceCode.userCode"
                  class="mail-btn"
                  :disabled="polling"
                  @click="pollAuth"
                >
                  {{ polling ? '等待授权…' : '我已经授权了' }}
                </button>
                <button v-if="oauthState === 'error'" class="mail-btn" @click="restartAuth">
                  重新发起授权
                </button>
              </div>
              <p class="mail-hint" style="margin: 4px 0 0">{{ deviceCode.verificationUrl }}</p>
              <details v-if="!deviceCode.userCode" class="mail-oauth-manual" style="margin-top: 8px">
                <summary class="mail-hint" style="cursor: pointer">浏览器没有自动回到应用？</summary>
                <p class="mail-hint" style="margin: 6px 0">
                  登录完成后浏览器会跳到 http://localhost:端口/?code=… 的地址；如果那个页面打不开，
                  把地址栏里的完整地址粘贴到这里，由应用完成最后一步。
                </p>
                <div style="display: flex; gap: 6px">
                  <input
                    v-model="callbackUrl"
                    class="mail-input"
                    placeholder="http://localhost:.../?code=...&state=..."
                  />
                  <button
                    class="mail-btn"
                    :disabled="!callbackUrl.trim() || completing"
                    @click="completeManual"
                  >
                    {{ completing ? '处理中…' : '完成授权' }}
                  </button>
                </div>
              </details>
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

          <template v-if="form.protocol === 'pop3'">
            <div class="mail-field-row">
              <div class="mail-field">
                <label class="mail-flabel">POP3 服务器</label>
                <input v-model="form.pop3Host" class="mail-input" placeholder="pop.example.com" />
              </div>
              <div class="mail-field">
                <label class="mail-flabel">端口 / 加密</label>
                <div style="display: flex; gap: 6px">
                  <input v-model.number="form.pop3Port" class="mail-input" type="number" style="width: 90px" />
                  <select v-model="form.pop3Security" class="mail-input">
                    <option value="ssl">SSL/TLS（995）</option>
                    <option value="starttls">STARTTLS（110）</option>
                    <option value="none">不加密（110）</option>
                  </select>
                </div>
              </div>
            </div>
            <div class="mail-field">
              <label class="mail-flabel">收信后</label>
              <label class="mail-check">
                <input v-model="form.pop3LeaveOnServer" type="checkbox" />
                在服务器上保留副本
              </label>
              <span class="mail-hint">
                勾选时删除邮件只影响本机；取消勾选后，本机删除的邮件会在下一次同步时从服务器删掉。
                多半只有一台设备用这个邮箱时，取消勾选可以避免服务器空间被占满。
              </span>
            </div>
          </template>

          <div v-else class="mail-field-row">
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
          <button
            class="mail-btn is-primary"
            :disabled="!canSave || saving || checkingAuth"
            @click="save()"
          >
            {{ saving ? '保存中…' : checkingAuth ? '检查授权…' : '保存并同步' }}
          </button>
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
  /** 查询账户当前是否已经拿到 OAuth 令牌（编辑既有账户时决定是否要求重新授权）。 */
  oauthStatusFn: { type: Function, default: null },
  /** 浏览器无法自动回到本机时，用粘贴进来的回调地址完成授权。 */
  oauthCompleteFn: { type: Function, default: null },
  /**
   * 真正落库的保存函数。必须是函数 prop 而不是 emit：设备码流程要在保存后
   * 拿到账户 id，再把令牌挂到这个 id 上；Vue 的 emit 不回传返回值。
   */
  saveFn: { type: Function, default: null },
})
const emit = defineEmits(['close', 'saved', 'deleted', 'select'])

const presets = [
  { label: 'QQ 邮箱', email: '', imapHost: 'imap.qq.com', imapPort: 993, pop3Host: 'pop.qq.com', smtpHost: 'smtp.qq.com', smtpPort: 465, hint: '需要 QQ 邮箱设置里生成的授权码' },
  { label: '163 邮箱', email: '', imapHost: 'imap.163.com', imapPort: 993, pop3Host: 'pop.163.com', smtpHost: 'smtp.163.com', smtpPort: 465, hint: '需要客户端授权密码' },
  { label: '126 邮箱', email: '', imapHost: 'imap.126.com', imapPort: 993, pop3Host: 'pop.126.com', smtpHost: 'smtp.126.com', smtpPort: 465, hint: '需要客户端授权密码' },
  { label: 'Gmail', email: '', imapHost: 'imap.gmail.com', imapPort: 993, smtpHost: 'smtp.gmail.com', smtpPort: 465, oauthProvider: 'gmail', hint: '需要开启两步验证并使用应用专用密码' },
  { label: 'Outlook', email: '', imapHost: 'outlook.office365.com', imapPort: 993, smtpHost: 'smtp.office365.com', smtpPort: 587, smtpSecurity: 'starttls', oauthProvider: 'outlook', authKind: 'oauth2', hint: '微软账户需要在安全设置里开启 IMAP' },
  { label: '腾讯企业邮', email: '', imapHost: 'imap.exmail.qq.com', imapPort: 993, smtpHost: 'smtp.exmail.qq.com', smtpPort: 465, hint: '登录名可能是完整邮箱' },
  { label: '阿里企业邮', email: '', imapHost: 'imap.qiye.aliyun.com', imapPort: 993, smtpHost: 'smtp.qiye.aliyun.com', smtpPort: 465, hint: '' },
]

/**
 * 常见服务商的服务器参数。选了已知服务商就自动填好，不该逼用户手抄
 * IMAP/SMTP 地址——「开始授权」更不该因为服务器栏空着而拒绝。
 */
const KNOWN_SERVERS = {
  outlook: {
    imapHost: 'outlook.office365.com', imapPort: 993, imapSecurity: 'ssl',
    pop3Host: 'outlook.office365.com',
    smtpHost: 'smtp.office365.com', smtpPort: 587, smtpSecurity: 'starttls',
  },
  gmail: {
    imapHost: 'imap.gmail.com', imapPort: 993, imapSecurity: 'ssl',
    pop3Host: 'pop.gmail.com',
    smtpHost: 'smtp.gmail.com', smtpPort: 465, smtpSecurity: 'ssl',
  },
}

const PROTOCOL_OPTIONS = [
  { value: 'imap', label: 'IMAP（推荐）', hint: '多设备同步，文件夹与已读状态都在服务器上' },
  { value: 'pop3', label: 'POP3', hint: '只收件箱，下载到本机；部分老邮箱只支持这个' },
]

const DOMAIN_SERVERS = {
  'outlook.com': KNOWN_SERVERS.outlook,
  'hotmail.com': KNOWN_SERVERS.outlook,
  'live.com': KNOWN_SERVERS.outlook,
  'msn.com': KNOWN_SERVERS.outlook,
  'gmail.com': KNOWN_SERVERS.gmail,
  'googlemail.com': KNOWN_SERVERS.gmail,
  'qq.com': { imapHost: 'imap.qq.com', imapPort: 993, imapSecurity: 'ssl', pop3Host: 'pop.qq.com', smtpHost: 'smtp.qq.com', smtpPort: 465, smtpSecurity: 'ssl' },
  'foxmail.com': { imapHost: 'imap.qq.com', imapPort: 993, imapSecurity: 'ssl', pop3Host: 'pop.qq.com', smtpHost: 'smtp.qq.com', smtpPort: 465, smtpSecurity: 'ssl' },
  '163.com': { imapHost: 'imap.163.com', imapPort: 993, imapSecurity: 'ssl', pop3Host: 'pop.163.com', smtpHost: 'smtp.163.com', smtpPort: 465, smtpSecurity: 'ssl' },
  '126.com': { imapHost: 'imap.126.com', imapPort: 993, imapSecurity: 'ssl', pop3Host: 'pop.126.com', smtpHost: 'smtp.126.com', smtpPort: 465, smtpSecurity: 'ssl' },
  'exmail.qq.com': { imapHost: 'imap.exmail.qq.com', imapPort: 993, imapSecurity: 'ssl', smtpHost: 'smtp.exmail.qq.com', smtpPort: 465, smtpSecurity: 'ssl' },
}

const editing = ref(false)
const testing = ref(false)
const saving = ref(false)
const polling = ref(false)
/** 编辑 OAuth 账户时正在查询令牌状态：查完再允许保存，避免误判成「未授权」。 */
const checkingAuth = ref(false)
const probeResult = ref(null)
const secretStored = ref(false)
const customMode = ref(false)
const overrideProxy = ref(false)
const bypassText = ref('')
// OAuth2 授权流程的界面状态
const oauthState = ref('idle')
const oauthError = ref('')
const deviceCode = ref(null)
/** 手动粘贴的回调地址（浏览器没能自动回到本机时使用）。 */
const callbackUrl = ref('')
const completing = ref(false)
/** 这个账户还没有可用的令牌：保存并关闭前必须先完成授权。 */
const awaitingAuth = ref(false)
/** 落库时的服务商：改了服务商就必须重新授权，旧令牌对新服务商无效。 */
const savedProvider = ref('')

const blank = () => ({
  id: '',
  email: '',
  displayName: '',
  protocol: 'imap',
  imapHost: '',
  imapPort: 993,
  imapSecurity: 'ssl',
  pop3Host: '',
  pop3Port: 995,
  pop3Security: 'ssl',
  pop3LeaveOnServer: true,
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
/** 上一次自动填写的服务器；用户手动改过之后就不再覆盖。 */
const autoServers = ref(null)

function applyServers(defaults) {
  form.imapHost = defaults.imapHost
  form.imapPort = defaults.imapPort
  form.imapSecurity = defaults.imapSecurity || 'ssl'
  form.pop3Host = defaults.pop3Host || derivePop3Host(defaults.imapHost)
  form.pop3Port = defaults.pop3Port || 995
  form.pop3Security = defaults.pop3Security || 'ssl'
  form.smtpHost = defaults.smtpHost
  form.smtpPort = defaults.smtpPort
  form.smtpSecurity = defaults.smtpSecurity || 'ssl'
  autoServers.value = {
    imapHost: defaults.imapHost,
    pop3Host: form.pop3Host,
    smtpHost: defaults.smtpHost,
  }
}

/**
 * 从 IMAP 主机名推一个 POP3 主机名。常见写法是 `imap.example.com` → `pop.example.com`；
 * 已知服务商都在预设表里单独写好了，这里只是自建邮箱的兜底，用户可以改。
 */
function derivePop3Host(imapHost) {
  const host = String(imapHost || '').trim()
  if (!host) return ''
  if (/^imap\./i.test(host)) return host.replace(/^imap\./i, 'pop.')
  return host
}

/** 按服务商 / 邮箱域名补服务器：已有服务器或用户手改过就不动。 */
function fillKnownServers() {
  const domain = String(form.email.split('@')[1] || '').trim().toLowerCase()
  const defaults =
    (form.authKind === 'oauth2' ? KNOWN_SERVERS[form.oauthProvider] : null) ||
    DOMAIN_SERVERS[domain]
  if (!defaults) return
  const untouched =
    (!form.imapHost.trim() && !form.smtpHost.trim()) ||
    (autoServers.value &&
      form.imapHost === autoServers.value.imapHost &&
      form.smtpHost === autoServers.value.smtpHost)
  if (untouched) applyServers(defaults)
  // POP3 账户只有 POP3 服务器在界面上；它单独填，与 IMAP 的自动补全互不影响。
  if (form.protocol === 'pop3' && !form.pop3Host.trim() && defaults.pop3Host) {
    form.pop3Host = defaults.pop3Host
  }
}

/** 切换收信协议：只切换界面字段，不动用户已经填好的另一套服务器。 */
function setProtocol(next) {
  if (form.protocol === next) return
  form.protocol = next
  if (next === 'pop3') {
    // POP3 不能用 OAuth2：服务商不提供它的授权端点。
    form.authKind = 'password'
    if (!form.pop3Host.trim()) {
      // 已知服务商用精确的 POP3 主机名（pop.qq.com 这类），其余按 imap.→pop. 推。
      const domain = String(form.email.split('@')[1] || '').trim().toLowerCase()
      form.pop3Host = DOMAIN_SERVERS[domain]?.pop3Host || derivePop3Host(form.imapHost)
    }
  }
}

/** 还缺哪些必填字段：报错时直接说清楚，别只给一句笼统提示。 */
function missingFields() {
  const missing = []
  // OAuth 可以先不填邮箱：授权结果里的 id_token 会带回来真实地址。
  const canOmitEmail = form.authKind === 'oauth2' && !form.email.trim()
  if (!emailValid.value && !canOmitEmail) missing.push('完整邮箱地址')
  if (form.protocol === 'pop3') {
    if (!form.pop3Host.trim()) missing.push('POP3 服务器')
  } else if (!form.imapHost.trim()) {
    missing.push('IMAP 服务器')
  }
  if (!form.smtpHost.trim()) missing.push('SMTP 服务器')
  return missing
}

const OUTLOOK_DOMAINS = new Set(['outlook.com', 'hotmail.com', 'live.com', 'msn.com'])
const GMAIL_DOMAINS = new Set(['gmail.com', 'googlemail.com'])

/** 邮箱域名已经说明了服务商：OAuth 下自动选中对应项，避免 Outlook 邮箱配到 Gmail 端点。 */
function syncProviderFromDomain() {
  if (form.authKind !== 'oauth2') return
  const domain = String(form.email.split('@')[1] || '').trim().toLowerCase()
  if (OUTLOOK_DOMAINS.has(domain) && form.oauthProvider !== 'outlook') {
    form.oauthProvider = 'outlook'
  } else if (GMAIL_DOMAINS.has(domain) && form.oauthProvider !== 'gmail') {
    form.oauthProvider = 'gmail'
  }
}

watch(
  [() => form.email, () => form.authKind, () => form.oauthProvider],
  () => {
    syncProviderFromDomain()
    fillKnownServers()
  },
)

watch(
  () => form.oauthProvider,
  (provider) => {
    if (form.id && provider !== savedProvider.value) awaitingAuth.value = true
    // 换服务商后旧的设备码不再有效，否则「我已经授权了」会拿旧码去新端点轮询。
    if (deviceCode.value) {
      deviceCode.value = null
      callbackUrl.value = ''
      oauthState.value = 'idle'
      oauthError.value = ''
    }
  },
)

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

// 只要求「本地部分@域名.后缀」：不追求 RFC 完整性，但一定要挡住
// `name@` 这种缺域名的半截地址——它会让保存下来的账户永远连不上服务器。
const emailValid = computed(() => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email.trim()))
const canSave = computed(
  () =>
    // OAuth 允许先空着邮箱，授权完成后再由服务商返回的地址补上。
    (emailValid.value || (form.authKind === 'oauth2' && !form.email.trim())) &&
    // 收信服务器按协议二选一：POP3 账户没有 IMAP 服务器，反之亦然。
    (form.protocol === 'pop3' ? form.pop3Host.trim() : form.imapHost.trim()) &&
    form.smtpHost.trim(),
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
  oauthError.value = ''
  awaitingAuth.value = false
  savedProvider.value = ''
  deviceCode.value = null
  callbackUrl.value = ''
  autoServers.value = null
  polling.value = false
  checkingAuth.value = false
  probeResult.value = null
  secretStored.value = false
  overrideProxy.value = false
  bypassText.value = ''
}

function startEdit(account) {
  customMode.value = false
  checkingAuth.value = false
  autoServers.value = null
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
  // OAuth 账户打开时先问一下令牌还在不在：没有令牌就要求先授权再保存，
  // 否则保存出去的又是一个「永远提示未授权」的账户。
  // 先按「需要授权」处理，查到令牌有效再放开——宁可多拦一次，也别放过。
  awaitingAuth.value = account.authKind === 'oauth2'
  savedProvider.value = account.oauthProvider || ''
  oauthState.value = 'idle'
  oauthError.value = ''
  deviceCode.value = null
  callbackUrl.value = ''
  if (account.authKind === 'oauth2' && props.oauthStatusFn) {
    checkingAuth.value = true
    props
      .oauthStatusFn(account.id)
      .then((status) => {
        if (form.id !== account.id) return
        awaitingAuth.value = !status?.connected
        // 令牌还在就把状态直接显示成「已授权」，避免每次打开都像要重新登录。
        if (status?.connected) oauthState.value = 'done'
      })
      .catch(() => {})
      .finally(() => {
        checkingAuth.value = false
      })
  }
}

function cancel() {
  editing.value = false
  probeResult.value = null
}

/** 自定义 / 内网邮箱：清空预填，全部字段由用户填。 */
function startCustom() {
  customMode.value = true
  // 自建/公司邮件系统通常用密码或授权码；OAuth2 只对内置的两家服务商有端点。
  form.authKind = 'password'
  autoServers.value = null
  form.imapHost = ''
  form.imapPort = 993
  form.imapSecurity = 'ssl'
  form.pop3Host = ''
  form.pop3Port = 995
  form.pop3Security = 'ssl'
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
  // 服务商自带 OAuth 端点时顺手选中，避免服务器和认证方式对不上。
  if (preset.oauthProvider) form.oauthProvider = preset.oauthProvider
  if (preset.authKind) form.authKind = preset.authKind
  autoServers.value = { imapHost: preset.imapHost, smtpHost: preset.smtpHost }
}

async function startAuth() {
  if (!props.oauthStartFn) return
  if (oauthState.value === 'starting' || polling.value) return
  // 已知服务商先把服务器补好：Outlook/Gmail 的地址是固定的，不该让用户手填。
  syncProviderFromDomain()
  fillKnownServers()
  const missing = missingFields()
  if (missing.length) {
    oauthState.value = 'error'
    oauthError.value = `请先填写${missing.join('、')}，再开始授权`
    return
  }
  oauthState.value = 'starting'
  oauthError.value = ''
  try {
    // Outlook 的回调线程要把令牌挂到账户上，所以先落库拿到 id。
    awaitingAuth.value = true
    if (!form.id) await save(false)
    if (!form.id) throw new Error('账户还没有保存完成，无法开始授权')
    deviceCode.value = await props.oauthStartFn(form.oauthProvider, form.id)
    oauthState.value = 'waiting'
    // 自动轮询：用户不用手动点「我已经授权了」，但保留按钮作为兜底。
    pollAuth().catch(() => {})
  } catch (error) {
    oauthState.value = 'error'
    oauthError.value = String(error?.message || error)
  }
}

/** 设备码过期 / 授权出错后重新发起：必须换一个新的 device code。 */
function restartAuth() {
  deviceCode.value = null
  oauthState.value = 'idle'
  oauthError.value = ''
  callbackUrl.value = ''
}

/**
 * 手动完成授权：把浏览器地址栏里的 `http://localhost:端口/?code=…&state=…`
 * 交给后端换取令牌。`redirect_uri` 取自本次授权地址，保证与微软侧完全一致。
 */
async function completeManual() {
  if (!props.oauthCompleteFn || !form.id || !deviceCode.value) return
  oauthError.value = ''
  let parsed
  try {
    parsed = new URL(callbackUrl.value.trim())
  } catch {
    oauthState.value = 'error'
    oauthError.value = '请粘贴完整的回调地址（以 http://localhost: 开头）'
    return
  }
  const code = parsed.searchParams.get('code')
  const state = parsed.searchParams.get('state')
  if (!code) {
    oauthState.value = 'error'
    oauthError.value = '地址里没有 code 参数，请确认复制的是登录完成后的地址'
    return
  }
  if (state && state !== deviceCode.value.deviceCode) {
    oauthState.value = 'error'
    oauthError.value = '回调地址与本次授权不匹配，请重新发起授权'
    return
  }
  let redirectUri = ''
  try {
    redirectUri =
      new URL(deviceCode.value.verificationUrl).searchParams.get('redirect_uri') || ''
  } catch {
    redirectUri = ''
  }
  if (!redirectUri) {
    oauthState.value = 'error'
    oauthError.value = '授权地址缺少回调参数，请重新发起授权'
    return
  }
  completing.value = true
  try {
    const email = await props.oauthCompleteFn(
      form.id,
      deviceCode.value.deviceCode,
      form.oauthProvider,
      redirectUri,
      code,
    )
    if (typeof email === 'string' && email.trim()) form.email = email.trim()
    awaitingAuth.value = false
    callbackUrl.value = ''
    if (!emailValid.value) {
      oauthState.value = 'error'
      oauthError.value = '授权已完成，但服务商没有返回邮箱地址；请手动填写后保存'
      return
    }
    oauthState.value = 'done'
  } catch (error) {
    oauthState.value = 'error'
    oauthError.value = String(error?.message || error)
  } finally {
    completing.value = false
  }
}

async function pollAuth() {
  if (!deviceCode.value || !props.oauthPollFn || polling.value) return
  polling.value = true
  oauthState.value = 'waiting'
  oauthError.value = ''
  try {
    // 令牌要挂在账户 id 上，所以先确保账户已经在库里。
    // save() 会把返回的 id 写回 form.id——这是重试时不再重复建账户的关键。
    // 先把 awaitingAuth 置位：保存时据此跳过「拉文件夹」，避免没令牌就报未授权。
    awaitingAuth.value = true
    if (!form.id) await save(false)
    if (!form.id) throw new Error('账户还没有保存完成，无法写入授权令牌')
    const email = await props.oauthPollFn(form.id, form.oauthProvider, deviceCode.value)
    if (typeof email === 'string' && email.trim()) form.email = email.trim()
    awaitingAuth.value = false
    if (!emailValid.value) {
      oauthState.value = 'error'
      oauthError.value = '授权已完成，但服务商没有返回邮箱地址；请手动填写后保存'
      return
    }
    oauthState.value = 'done'
  } catch (error) {
    oauthState.value = 'error'
    oauthError.value = String(error?.message || error)
  } finally {
    polling.value = false
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
  if (saving.value) return null
  if (!canSave.value) {
    if (!closeAfter) {
      throw new Error(
        form.protocol === 'pop3'
          ? '请先填写邮箱地址、POP3 与 SMTP 服务器'
          : '请先填写邮箱地址、IMAP 与 SMTP 服务器',
      )
    }
    return null
  }
  // OAuth2 账户还没有可用令牌时，保存只会得到一个「永远提示未授权」的账户。
  // 授权已经完成（oauthState === 'done'）时放行；编辑一个令牌仍然有效的
  // 既有账户时 awaitingAuth 为 false，也不会被拦。
  if (
    closeAfter &&
    form.authKind === 'oauth2' &&
    (awaitingAuth.value || !form.id) &&
    oauthState.value !== 'done'
  ) {
    oauthState.value = 'error'
    oauthError.value = '还没有完成授权：请先输入设备码完成授权，或把认证方式改回密码'
    return null
  }
  saving.value = true
  try {
    const account = {
      ...form,
      email: form.email.trim(),
      imapHost: form.imapHost.trim(),
      smtpHost: form.smtpHost.trim(),
      proxy: overrideProxy.value ? { ...form.proxy } : null,
    }
    delete account.secret
    if (!props.saveFn) {
      emit('saved', { account, secret: form.secret, closeAfter })
      return null
    }
    const saved = await props.saveFn(account, form.secret, {
      closeAfter,
      oauthPending: form.authKind === 'oauth2' && (awaitingAuth.value || !form.id),
    })
    // 记住服务端分配的 id：设备码轮询重试、后续再点保存都用它，不会再建新账户。
    if (saved?.id) form.id = saved.id
    savedProvider.value = form.oauthProvider || ''
    if (closeAfter) emit('close')
    return saved
  } finally {
    saving.value = false
  }
}

function remove(account) {
  if (!window.confirm(`删除账户 ${account.email}？本地缓存的邮件也会一并清掉（服务器上的不受影响）。`)) {
    return
  }
  emit('deleted', account.id)
}
</script>
