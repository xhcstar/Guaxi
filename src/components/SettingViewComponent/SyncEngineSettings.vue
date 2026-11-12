<template>
  <div class="sync-engine">
    <div class="se-head">
      <div>
        <h3>同步目标</h3>
        <p class="description">
          一个目标 = 本地目录 + 一个连接。资源库、知识库、文件库都可以各自选择 WebDAV、S3 或 SFTP，互不影响。
        </p>
      </div>
      <n-space :size="8">
        <n-button size="small" :disabled="loading" @click="openTarget()">
          <template #icon><n-icon :component="Add20Regular" /></template>
          添加目标
        </n-button>
        <n-button size="small" type="primary" :disabled="loading || !enabledTargets.length" @click="runAll">
          <template #icon><n-icon :component="ArrowSync20Regular" /></template>
          全部同步
        </n-button>
      </n-space>
    </div>

    <n-alert v-if="notice.text" :type="notice.type" :show-icon="true" closable class="se-notice" @close="notice.text = ''">
      {{ notice.text }}
    </n-alert>

    <n-empty v-if="!config.targets.length" description="还没有同步目标。先添加一个连接，再添加目标。" class="se-empty">
      <template #extra>
        <n-button size="small" @click="openConnection()">添加连接</n-button>
      </template>
    </n-empty>

    <n-list v-else hoverable class="se-targets">
      <n-list-item v-for="target in config.targets" :key="target.id">
        <div class="se-target">
          <div class="se-target-main">
            <div class="se-target-title">
              <n-icon :component="Folder20Regular" :size="18" />
              <strong>{{ target.name || scopeLabel(target.scope) }}</strong>
              <n-tag size="tiny" :bordered="false" type="info">{{ connectionLabel(target.connection_id) }}</n-tag>
              <n-tag size="tiny" :bordered="false">{{ directionLabel(target.direction) }}</n-tag>
              <n-tag v-if="target.delete_missing" size="tiny" :bordered="false" type="warning">传播删除</n-tag>
            </div>
            <div class="se-target-meta">
              <span>{{ target.local_path || scopeLabel(target.scope) + ' 根目录' }}</span>
              <span class="se-dot">·</span>
              <span>远端 {{ target.remote_path || '/' }}</span>
              <span class="se-dot">·</span>
              <span>{{ target.interval_minutes || 30 }} 分钟</span>
            </div>
            <div v-if="statusOf(target)" class="se-progress">
              <n-progress
                type="line"
                :height="6"
                :percentage="percentOf(target)"
                :status="statusOf(target).status === 'error' ? 'error' : statusOf(target).status === 'success' ? 'success' : 'default'"
                :show-indicator="false"
              />
              <span class="se-progress-text">
                {{ statusOf(target).message || statusOf(target).phase }}
                <template v-if="statusOf(target).total">
                  · {{ statusOf(target).current }}/{{ statusOf(target).total }}
                </template>
                <template v-if="statusOf(target).speed_bps">
                  · {{ formatSpeed(statusOf(target).speed_bps) }}
                </template>
              </span>
              <span v-if="statusOf(target).file" class="se-progress-file">{{ statusOf(target).file }}</span>
            </div>
            <div v-else-if="target.last_message" class="se-last" :class="{ 'is-error': target.last_status === 'error' }">
              {{ target.last_message }}
            </div>
          </div>

          <n-space :size="6" align="center" class="se-target-actions">
            <n-switch
              :value="target.enabled"
              size="small"
              :disabled="loading"
              @update:value="(value) => toggleTarget(target, value)"
            />
            <n-button size="tiny" :disabled="loading || isRunning(target)" @click="runTarget(target)">
              <template #icon><n-icon :component="ArrowSync20Regular" :size="14" /></template>
              同步
            </n-button>
            <n-button size="tiny" quaternary @click="openTarget(target)">编辑</n-button>
            <n-popconfirm @positive-click="removeTarget(target)">
              <template #trigger>
                <n-button size="tiny" quaternary type="error">删除</n-button>
              </template>
              删除这个同步目标？远端文件不会被删除。
            </n-popconfirm>
          </n-space>
        </div>
      </n-list-item>
    </n-list>

    <div class="se-head is-connections">
      <div>
        <h3>连接</h3>
        <p class="description">连接只保存地址与凭据，可被多个目标复用。</p>
      </div>
      <n-button size="small" tertiary @click="openConnection()">
        <template #icon><n-icon :component="Add20Regular" /></template>
        添加连接
      </n-button>
    </div>

    <div v-if="!config.connections.length" class="se-empty-line">还没有连接</div>
    <div v-else class="se-connections">
      <div v-for="connection in config.connections" :key="connection.id" class="se-connection">
        <div class="se-connection-main">
          <strong>{{ connection.name || connection.kind }}</strong>
          <n-tag size="tiny" :bordered="false">{{ kindLabel(connection.kind) }}</n-tag>
          <span class="se-connection-addr">{{ connectionAddress(connection) }}</span>
        </div>
        <n-space :size="4" align="center">
          <n-button size="tiny" quaternary :disabled="testingId === connection.id" @click="testConnection(connection)">
            {{ testingId === connection.id ? '测试中…' : '测试' }}
          </n-button>
          <n-button size="tiny" quaternary @click="openConnection(connection)">编辑</n-button>
          <n-popconfirm @positive-click="removeConnection(connection)">
            <template #trigger>
              <n-button size="tiny" quaternary type="error">删除</n-button>
            </template>
            删除连接？使用它的目标会失效。
          </n-popconfirm>
        </n-space>
      </div>
    </div>

    <!-- 连接编辑 -->
    <n-modal v-model:show="showConnectionModal" preset="card" class="se-modal" :title="connectionDraft.isNew ? '添加连接' : '编辑连接'" :bordered="false">
      <div class="se-form">
        <label>名称<input v-model="connectionDraft.name" class="se-input" placeholder="例如：坚果云 / R2 / 家里 NAS" /></label>
        <label>类型
          <n-select v-model:value="connectionDraft.kind" :options="kindOptions" size="small" />
        </label>

        <template v-if="connectionDraft.kind === 'webdav'">
          <label>WebDAV 地址<input v-model="connectionDraft.webdav.url" class="se-input" placeholder="https://dav.jianguoyun.com/dav/你的目录" /></label>
          <label>用户名<input v-model="connectionDraft.webdav.username" class="se-input" autocomplete="off" /></label>
          <label>密码 / 应用密码<input v-model="connectionDraft.webdav.password" class="se-input" type="password" autocomplete="new-password" /></label>
        </template>

        <template v-else-if="connectionDraft.kind === 's3'">
          <label>Endpoint<input v-model="connectionDraft.s3.endpoint" class="se-input" placeholder="https://<account>.r2.cloudflarestorage.com" /></label>
          <div class="se-row">
            <label>Bucket<input v-model="connectionDraft.s3.bucket" class="se-input" placeholder="guaxi" /></label>
            <label>Region<input v-model="connectionDraft.s3.region" class="se-input" placeholder="auto" /></label>
          </div>
          <label>Access Key<input v-model="connectionDraft.s3.access_key" class="se-input" autocomplete="off" /></label>
          <label>Secret Key<input v-model="connectionDraft.s3.secret_key" class="se-input" type="password" autocomplete="new-password" /></label>
          <label class="se-check">
            <n-switch v-model:value="connectionDraft.s3.path_style" size="small" />
            路径风格（MinIO / R2 / 大多数兼容服务）
          </label>
        </template>

        <template v-else-if="connectionDraft.kind === 'local'">
          <label>本地 / 同步盘目录
            <div class="se-pick">
              <input v-model="connectionDraft.local.root" class="se-input" placeholder="例如：D:\\OneDrive\\Guaxi 或 NAS 挂载点" />
              <n-button size="small" @click="pickConnectionDir">浏览</n-button>
            </div>
          </label>
          <p class="se-hint">指向 OneDrive、Dropbox、坚果云同步盘或 NAS 映射盘即可，不需要厂商 API。</p>
        </template>

        <template v-else-if="connectionDraft.kind === 'cloud'">
          <label>网盘
            <n-select v-model:value="connectionDraft.cloud.provider" :options="cloudProviderOptions" size="small" />
          </label>

          <template v-if="connectionDraft.cloud.provider === 'baidu'">
            <label>应用目录名（AppName）
              <input v-model="connectionDraft.cloud.app_name" class="se-input" placeholder="例如：Guaxi；远端根目录为 /apps/Guaxi" />
            </label>
            <label>Access Token
              <input v-model="connectionDraft.cloud.access_token" class="se-input" placeholder="可留空，用下面的 Refresh Token 自动换取" />
            </label>
            <div class="se-row">
              <label>Refresh Token<input v-model="connectionDraft.cloud.refresh_token" class="se-input" /></label>
              <label>Secret Key<input v-model="connectionDraft.cloud.client_secret" class="se-input" type="password" /></label>
            </div>
            <label>AppKey（Client ID）<input v-model="connectionDraft.cloud.client_id" class="se-input" /></label>
            <p class="se-hint">百度开放平台的应用数据默认在 /apps/应用名；Access Token 30 天有效，填 Refresh Token 可自动续期。</p>
          </template>

          <template v-else-if="connectionDraft.cloud.provider === 'aliyun'">
            <label>Access Token
              <input v-model="connectionDraft.cloud.access_token" class="se-input" placeholder="可留空，用 Refresh Token 自动换取" />
            </label>
            <div class="se-row">
              <label>Refresh Token<input v-model="connectionDraft.cloud.refresh_token" class="se-input" /></label>
              <label>Client Secret<input v-model="connectionDraft.cloud.client_secret" class="se-input" type="password" /></label>
            </div>
            <div class="se-row">
              <label>Client ID<input v-model="connectionDraft.cloud.client_id" class="se-input" /></label>
              <label>Drive ID<input v-model="connectionDraft.cloud.drive_id" class="se-input" placeholder="留空自动获取" /></label>
            </div>
            <p class="se-hint">阿里云盘开放平台 Client ID / Secret 需在控制台申请；远端目录填文件夹 ID，根目录填 root。</p>
          </template>

          <template v-else>
            <label>夸克 Cookie
              <textarea v-model="connectionDraft.cloud.cookie" class="se-input is-area" rows="3" placeholder="登录 pan.quark.cn 后从开发者工具复制 Cookie" />
            </label>
            <p class="se-hint">夸克官方 API 尚未公开，当前是 Cookie 模式，支持浏览/下载/删除/建目录；上传会提示改用 WebDAV 或同步盘。</p>
          </template>
        </template>

        <template v-else>
          <label>SSH 档案
            <n-select v-model:value="connectionDraft.sftp.profile_id" :options="profileOptions" size="small" filterable placeholder="从网络工具箱的 SSH 档案中选择" />
          </label>
          <label>远端起始目录<input v-model="connectionDraft.sftp.root" class="se-input" placeholder="留空使用登录后的家目录" /></label>
          <p class="se-hint">密码与私钥沿用 SSH 档案，不会在同步配置里重复保存。</p>
        </template>
      </div>
      <template #footer>
        <div class="se-modal-actions">
          <n-button size="small" @click="testDraftConnection">测试连接</n-button>
          <span class="se-spacer" />
          <n-button size="small" @click="showConnectionModal = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveConnection">保存</n-button>
        </div>
      </template>
    </n-modal>

    <!-- 目标编辑 -->
    <n-modal v-model:show="showTargetModal" preset="card" class="se-modal" :title="targetDraft.isNew ? '添加同步目标' : '编辑同步目标'" :bordered="false">
      <div class="se-form">
        <label>名称<input v-model="targetDraft.name" class="se-input" placeholder="例如：资源库" /></label>
        <label>目录类型
          <n-select v-model:value="targetDraft.scope" :options="scopeOptions" size="small" />
        </label>
        <label>本地目录
          <div class="se-pick">
            <input v-model="targetDraft.local_path" class="se-input" placeholder="留空则使用该库的根目录" />
            <n-button size="small" @click="pickTargetDir">浏览</n-button>
          </div>
        </label>
        <label>连接
          <n-select v-model:value="targetDraft.connection_id" :options="connectionOptions" size="small" placeholder="先添加一个连接" />
        </label>
        <label>远端目录<input v-model="targetDraft.remote_path" class="se-input" :placeholder="remotePathPlaceholder" /></label>
        <div class="se-row">
          <label>方向
            <n-select v-model:value="targetDraft.direction" :options="directionOptions" size="small" />
          </label>
          <label>冲突
            <n-select v-model:value="targetDraft.conflict_policy" :options="conflictOptions" size="small" />
          </label>
        </div>
        <div class="se-row">
          <label>自动同步间隔（分钟）
            <n-input-number v-model:value="targetDraft.interval_minutes" size="small" :min="5" :step="5" />
          </label>
          <label class="se-check">
            <n-switch v-model:value="targetDraft.delete_missing" size="small" />
            传播删除
          </label>
        </div>
        <p class="se-hint">首次同步会保留冲突副本；「传播删除」默认关闭，避免误删另一台设备的文件。</p>
      </div>
      <template #footer>
        <div class="se-modal-actions">
          <span class="se-spacer" />
          <n-button size="small" @click="showTargetModal = false">取消</n-button>
          <n-button size="small" type="primary" @click="saveTarget">保存</n-button>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import {
  NAlert,
  NButton,
  NEmpty,
  NIcon,
  NInputNumber,
  NList,
  NListItem,
  NModal,
  NPopconfirm,
  NProgress,
  NSelect,
  NSpace,
  NSwitch,
  NTag,
} from 'naive-ui'
import { Add20Regular, ArrowSync20Regular, Folder20Regular } from '@vicons/fluent'
import { createLogger } from '../../utils/logger'

const logger = createLogger('SyncEngineSettings')
const loading = ref(false)
const testingId = ref('')
const config = reactive({ connections: [], targets: [] })
const statuses = reactive({})
const notice = reactive({ type: 'info', text: '' })

const showConnectionModal = ref(false)
const showTargetModal = ref(false)
const connectionDraft = reactive(emptyConnection())
const targetDraft = reactive(emptyTarget())
const profileOptions = ref([])

const kindOptions = [
  { label: 'WebDAV（坚果云 / Nextcloud / 群晖）', value: 'webdav' },
  { label: 'S3 兼容（R2 / B2 / MinIO / OSS）', value: 's3' },
  { label: 'SFTP（复用 SSH 档案）', value: 'sftp' },
  { label: '本地 / 同步盘目录', value: 'local' },
  { label: '网盘直连（百度 / 阿里云盘 / 夸克）', value: 'cloud' },
]

const cloudProviderOptions = [
  { label: '百度网盘', value: 'baidu' },
  { label: '阿里云盘', value: 'aliyun' },
  { label: '夸克（Cookie 模式）', value: 'quark' },
]

const scopeOptions = [
  { label: '资源库', value: 'resource_repo' },
  { label: '知识库', value: 'knowledge_base' },
  { label: '文件库', value: 'libraries' },
  { label: '模板库', value: 'template' },
  { label: 'Slidev 源文件', value: 'slidev' },
  { label: '自定义目录', value: 'custom' },
]

const directionOptions = [
  { label: '推送（本地 → 远端）', value: 'upload' },
  { label: '拉取（远端 → 本地）', value: 'download' },
  { label: '双向', value: 'two_way' },
]

const conflictOptions = [
  { label: '保留冲突副本', value: 'newer' },
  { label: '本地优先', value: 'local' },
  { label: '远端优先', value: 'remote' },
]

const enabledTargets = computed(() => config.targets.filter((t) => t.enabled))
const remotePathPlaceholder = computed(() => {
  const connection = config.connections.find((c) => c.id === targetDraft.connection_id)
  if (!connection) return '例如：Guaxi/ResourceRepo'
  if (connection.kind === 'local') return '本地目录下的子目录，留空表示根'
  if (connection.kind === 'cloud') {
    if (connection.cloud?.provider === 'baidu') return '留空使用 /apps/应用名，或填 /apps/应用名/子目录'
    if (connection.cloud?.provider === 'aliyun') return '文件夹 ID，根目录填 root'
    if (connection.cloud?.provider === 'quark') return '夸克固定从根目录开始，可留空'
  }
  return '例如：Guaxi/ResourceRepo'
})

const connectionOptions = computed(() =>
  config.connections.map((c) => ({ label: c.name || c.kind, value: c.id }))
)

const clone = (value) => JSON.parse(JSON.stringify(value))

const uid = () => {
  try {
    return crypto.randomUUID()
  } catch {
    return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
  }
}

function emptyConnection() {
  return {
    id: '',
    name: '',
    kind: 'webdav',
    webdav: { url: '', username: '', password: '' },
    s3: { endpoint: '', region: 'auto', bucket: '', access_key: '', secret_key: '', path_style: true },
    sftp: { profile_id: '', root: '' },
    local: { root: '' },
    cloud: {
      provider: 'baidu',
      access_token: '',
      refresh_token: '',
      client_id: '',
      client_secret: '',
      drive_id: '',
      app_name: '',
      cookie: '',
    },
    isNew: true,
  }
}

function emptyTarget() {
  return {
    id: '',
    name: '',
    scope: 'resource_repo',
    local_path: '',
    connection_id: '',
    remote_path: '',
    direction: 'upload',
    conflict_policy: 'newer',
    delete_missing: false,
    enabled: true,
    interval_minutes: 30,
    last_run_ms: 0,
    last_status: '',
    last_message: '',
    isNew: true,
  }
}

const scopeLabel = (scope) => scopeOptions.find((o) => o.value === scope)?.label || scope
const kindLabel = (kind) => {
  if (kind === 'local') return '本地'
  if (kind === 'cloud') return '网盘'
  return kindOptions.find((o) => o.value === kind)?.label?.split('（')[0] || kind
}
const directionLabel = (value) => directionOptions.find((o) => o.value === value)?.label?.split('（')[0] || '推送'
const connectionLabel = (id) => {
  const conn = config.connections.find((c) => c.id === id)
  return conn ? `${kindLabel(conn.kind)} · ${conn.name || '未命名'}` : '连接缺失'
}
const connectionAddress = (connection) => {
  if (connection.kind === 'webdav') return connection.webdav.url || '未填写地址'
  if (connection.kind === 's3') return `${connection.s3.bucket || '未填写 bucket'} @ ${connection.s3.endpoint || '未填写 endpoint'}`
  if (connection.kind === 'local') return connection.local?.root || '未填写目录'
  if (connection.kind === 'cloud') {
    const provider = cloudProviderOptions.find((o) => o.value === connection.cloud?.provider)?.label || '网盘'
    if (connection.cloud?.provider === 'quark') return `${provider} · Cookie`
    return `${provider} · ${connection.cloud?.app_name || connection.cloud?.drive_id || '待授权'}`
  }
  return connection.sftp.profile_id ? `SSH 档案 ${connection.sftp.profile_id}` : '未选择 SSH 档案'
}

const statusOf = (target) => statuses[target.id] || null
const isRunning = (target) => {
  const status = statusOf(target)
  return status && status.phase !== 'done'
}
const percentOf = (target) => {
  const status = statusOf(target)
  if (!status) return 0
  if (status.status === 'success' || status.phase === 'done') return 100
  if (!status.total) return status.status === 'running' ? 10 : 0
  return Math.max(0, Math.min(100, Math.round((status.current / status.total) * 100)))
}
const formatSpeed = (bps) => {
  const value = Number(bps || 0)
  if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB/s`
  if (value >= 1024) return `${(value / 1024).toFixed(0)} KB/s`
  return `${value} B/s`
}

const loadConfig = async () => {
  loading.value = true
  try {
    const value = await invoke('sync_engine_get_config')
    config.connections = Array.isArray(value?.connections) ? value.connections : []
    config.targets = Array.isArray(value?.targets) ? value.targets : []
  } catch (error) {
    logger.error('load sync config failed', error)
    notice.type = 'error'
    notice.text = String(error?.message || error)
  } finally {
    loading.value = false
  }
}

const loadProfiles = async () => {
  try {
    const list = await invoke('ssh_list_profiles')
    profileOptions.value = (Array.isArray(list) ? list : []).map((p) => ({
      label: `${p.name || p.host}（${p.user || 'user'}@${p.host || 'host'}）`,
      value: p.id,
    }))
  } catch (error) {
    logger.warn('load ssh profiles failed', error)
  }
}

const saveConfig = async () => {
  await invoke('sync_engine_set_config', {
    configInput: {
      connections: config.connections.map(stripDraft),
      targets: config.targets.map(stripDraft),
    },
  })
}

const stripDraft = (value) => {
  const { isNew, ...rest } = value
  return rest
}

const openConnection = (connection) => {
  Object.assign(connectionDraft, connection ? clone(connection) : emptyConnection())
  connectionDraft.isNew = !connection
  if (!connection) connectionDraft.id = uid()
  showConnectionModal.value = true
}

const saveConnection = async () => {
  if (!connectionDraft.name.trim()) {
    notice.type = 'warning'
    notice.text = '给连接起个名字，方便区分。'
    return
  }
  if (connectionDraft.kind === 'local' && !connectionDraft.local.root.trim()) {
    notice.type = 'warning'
    notice.text = '本地 / 同步盘连接需要填写目录。'
    return
  }
  if (connectionDraft.kind === 'cloud') {
    const cloud = connectionDraft.cloud
    if (cloud.provider === 'quark' && !cloud.cookie.trim()) {
      notice.type = 'warning'
      notice.text = '夸克连接需要填写 Cookie。'
      return
    }
    if (cloud.provider !== 'quark' && !cloud.access_token.trim() && (!cloud.refresh_token.trim() || !cloud.client_id.trim() || !cloud.client_secret)) {
      notice.type = 'warning'
      notice.text = '请填写 Access Token，或补齐 Refresh Token / Client ID / Client Secret。'
      return
    }
  }
  const payload = stripDraft(clone(connectionDraft))
  const index = config.connections.findIndex((c) => c.id === payload.id)
  if (index >= 0) config.connections[index] = payload
  else config.connections.push(payload)
  try {
    await saveConfig()
    showConnectionModal.value = false
    notice.type = 'success'
    notice.text = '连接已保存'
  } catch (error) {
    notice.type = 'error'
    notice.text = String(error?.message || error)
  }
}

const testDraftConnection = async () => {
  testingId.value = connectionDraft.id || 'draft'
  notice.text = ''
  try {
    const text = await invoke('sync_engine_test_connection', {
      connectionId: null,
      connection: stripDraft(clone(connectionDraft)),
    })
    notice.type = 'success'
    notice.text = text || '连接正常'
  } catch (error) {
    notice.type = 'error'
    notice.text = String(error?.message || error)
  } finally {
    testingId.value = ''
  }
}

const testConnection = async (connection) => {
  testingId.value = connection.id
  notice.text = ''
  try {
    const text = await invoke('sync_engine_test_connection', { connectionId: connection.id })
    notice.type = 'success'
    notice.text = text || '连接正常'
  } catch (error) {
    notice.type = 'error'
    notice.text = String(error?.message || error)
  } finally {
    testingId.value = ''
  }
}

const removeConnection = async (connection) => {
  const used = config.targets.filter((t) => t.connection_id === connection.id)
  if (used.length) {
    notice.type = 'warning'
    notice.text = `还有 ${used.length} 个目标在使用这个连接，先改掉它们。`
    return
  }
  config.connections = config.connections.filter((c) => c.id !== connection.id)
  await saveConfig()
}

const openTarget = (target) => {
  Object.assign(targetDraft, target ? clone(target) : emptyTarget())
  targetDraft.isNew = !target
  if (!target) {
    targetDraft.id = uid()
    targetDraft.connection_id = config.connections[0]?.id || ''
  }
  showTargetModal.value = true
}

const pickConnectionDir = async () => {
  try {
    const picked = await open({ directory: true, multiple: false })
    if (typeof picked === 'string') connectionDraft.local.root = picked
  } catch (error) {
    logger.warn('pick connection dir failed', error)
  }
}

const pickTargetDir = async () => {
  try {
    const picked = await open({ directory: true, multiple: false })
    if (typeof picked === 'string') targetDraft.local_path = picked
  } catch (error) {
    logger.warn('pick target dir failed', error)
  }
}

const saveTarget = async () => {
  if (!targetDraft.connection_id) {
    notice.type = 'warning'
    notice.text = '先添加并选择一个连接。'
    return
  }
  const payload = stripDraft(clone(targetDraft))
  const index = config.targets.findIndex((t) => t.id === payload.id)
  if (index >= 0) config.targets[index] = payload
  else config.targets.push(payload)
  try {
    await saveConfig()
    showTargetModal.value = false
    notice.type = 'success'
    notice.text = '同步目标已保存'
  } catch (error) {
    notice.type = 'error'
    notice.text = String(error?.message || error)
  }
}

const removeTarget = async (target) => {
  config.targets = config.targets.filter((t) => t.id !== target.id)
  await saveConfig()
}

const toggleTarget = async (target, enabled) => {
  target.enabled = enabled
  await saveConfig()
}

const runTarget = async (target) => {
  notice.text = ''
  try {
    await invoke('sync_engine_run_target', { targetId: target.id })
  } catch (error) {
    notice.type = 'error'
    notice.text = String(error?.message || error)
  }
}

const runAll = async () => {
  notice.text = ''
  try {
    await invoke('sync_engine_run_all')
  } catch (error) {
    notice.type = 'error'
    notice.text = String(error?.message || error)
  }
}

let unlistenProgress = null
let unlistenDone = null

onMounted(async () => {
  await Promise.all([loadConfig(), loadProfiles()])
  try {
    const current = await invoke('sync_engine_statuses')
    for (const status of Array.isArray(current) ? current : []) statuses[status.target_id] = status
  } catch (error) {
    logger.warn('load sync statuses failed', error)
  }
  unlistenProgress = await listen('sync:progress', (event) => {
    const payload = event?.payload
    if (payload?.target_id) statuses[payload.target_id] = payload
  })
  unlistenDone = await listen('sync:done', (event) => {
    const payload = event?.payload
    if (payload?.target_id) statuses[payload.target_id] = payload
    setTimeout(loadConfig, 300)
  })
})

onUnmounted(() => {
  unlistenProgress?.()
  unlistenDone?.()
})
</script>

<style scoped>
.sync-engine { width: 100%; }
.se-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 10px; }
.se-head h3 { margin: 0 0 4px; font-size: 15px; }
.se-head.is-connections { margin-top: 24px; }
.description { margin: 0; color: var(--n-text-color-3); font-size: 13px; line-height: 1.7; }
.se-notice { margin: 8px 0 12px; max-width: 95%; }
.se-empty { padding: 18px 0; }
.se-targets { border: 1px solid var(--n-border-color); border-radius: 8px; }
.se-target { display: flex; align-items: center; justify-content: space-between; gap: 16px; width: 100%; }
.se-target-main { min-width: 0; flex: 1; }
.se-target-title { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; font-size: 13.5px; }
.se-target-meta { margin-top: 4px; color: var(--n-text-color-3); font-size: 12px; word-break: break-all; }
.se-dot { margin: 0 6px; opacity: .5; }
.se-progress { margin-top: 8px; display: grid; grid-template-columns: 1fr auto; gap: 2px 10px; align-items: center; }
.se-progress :deep(.n-progress) { grid-column: 1; }
.se-progress-text { grid-column: 1 / -1; color: var(--n-text-color-3); font-size: 12px; font-variant-numeric: tabular-nums; }
.se-progress-file { grid-column: 1 / -1; color: var(--n-text-color-3); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.se-last { margin-top: 6px; color: var(--n-text-color-3); font-size: 12px; }
.se-last.is-error { color: #d03050; }
.se-target-actions { flex: 0 0 auto; }
.se-connections { display: flex; flex-direction: column; border: 1px solid var(--n-border-color); border-radius: 8px; }
.se-connection { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 9px 12px; border-bottom: 1px solid var(--n-border-color); }
.se-connection:last-child { border-bottom: 0; }
.se-connection-main { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: 13px; }
.se-connection-addr { color: var(--n-text-color-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.se-empty-line { color: var(--n-text-color-3); font-size: 13px; padding: 8px 0; }
.se-modal { width: min(560px, 92vw); }
.se-form { display: flex; flex-direction: column; gap: 12px; }
.se-form label { display: flex; flex-direction: column; gap: 5px; font-size: 13px; color: var(--n-text-color-2); }
.se-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.se-check { flex-direction: row !important; align-items: center; gap: 8px !important; }
.se-input { height: 30px; padding: 0 9px; border-radius: 5px; border: 1px solid var(--n-border-color); background: var(--n-color); color: var(--n-text-color); font-size: 13px; outline: none; }
.se-input:focus { border-color: var(--n-primary-color); }
.se-pick { display: flex; gap: 8px; }
.se-pick .se-input { flex: 1; }
.se-hint { margin: 0; color: var(--n-text-color-3); font-size: 12px; line-height: 1.65; }
.se-modal-actions { display: flex; align-items: center; gap: 8px; }
.se-spacer { flex: 1; }
</style>
