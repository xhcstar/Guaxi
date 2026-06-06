<template>
  <div class="preference-wrapper">
    <n-card title="笔记库与资源库">
      <n-space vertical size="large" style="width: 100%">

        <div>
          <div class="section-header">
            <h3>文件库根目录</h3>
            <n-tooltip trigger="hover">
              <template #trigger>
                <n-button text @click="handleOpenFileBrowserForLibraryRoot">
                  <n-icon :component="librariesRoot ? DocumentEdit16Regular : FolderAdd20Regular" :size="22" />
                </n-button>
              </template>
              {{ librariesRoot ? '更改文件库根目录' : '添加文件库根目录' }}
            </n-tooltip>
          </div>
          <p class="description">
            用于存放导入的文件库数据。建议选择一个稳定且有足够空间的磁盘目录。
          </p>

          <n-alert type="info" v-if="librariesRoot" style="max-width: 95%;">
            当前文件库根目录：{{ librariesRoot }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置文件库根目录
          </n-alert>

          <n-list hoverable style="max-width: 95%; margin-top: 8px;">
            <n-list-item v-if="librariesRoot">
              <template #prefix>
                <n-icon :component="Folder20Regular" :size="24" />
              </template>
              <n-text strong>{{ librariesRoot }}</n-text>
            </n-list-item>
          </n-list>

        </div>

        <n-divider />

        <div>
          <h3>知识库根目录</h3>
          <p class="description">
            用于存放知识库数据。未配置时将使用程序同级目录下的 KnowledgeBases 文件夹。
          </p>

          <n-alert type="info" v-if="knowledgeBasesRoot" style="max-width: 95%;">
            当前知识库根目录：{{ knowledgeBasesRoot }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置知识库根目录，将使用默认位置。
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="选择知识库根目录">
              <n-button :disabled="loading" @click="handleKnowledgeBasesRootPick">
                浏览文件夹...
              </n-button>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />
        <div>
          <h3>资源库根目录</h3>
          <p class="description">
            用于存放截图、图片等资源的根目录。未配置时将使用程序所在目录下的 ResourceRepo 文件夹。
          </p>

          <n-alert v-if="resourceRoot" type="info" :show-icon="true" class="tip" style="max-width: 95%;">
            当前资源库根目录：{{ resourceRoot }}
          </n-alert>
          <n-alert v-else type="warning" :show-icon="true" class="tip" style="max-width: 95%;">
            未配置资源库根目录，将使用默认位置。
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="选择资源库根目录">
              <n-button :disabled="loading" @click="handleResourceRootPick">
                浏览文件夹...
              </n-button>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>资源库同步（云存储连接器）</h3>
          <p class="description">
            用于在不同设备间同步 ResourceRepo 目录内容。建议先配置连接器，再手动触发同步。
          </p>

          <n-alert :type="syncUi.alertType" :show-icon="true" class="sync-status-alert" style="max-width: 95%;">
            <n-space align="center" justify="space-between" style="width: 100%">
              <div class="sync-status-main">
                <div class="sync-status-title">{{ syncUi.title }}</div>
                <div class="sync-status-sub">
                  <span v-if="syncState.detail">{{ syncState.detail }}</span>
                  <span v-else>
                    最近操作：{{ syncState.action || '—' }} · {{ syncState.at || '—' }}
                  </span>
                </div>
              </div>
              <n-tag :type="syncUi.tagType" size="small">{{ syncUi.tagText }}</n-tag>
            </n-space>
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="同步后端">
              <n-select v-model:value="syncProvider" :options="syncProviderOptions" :disabled="loading" />
            </n-form-item>

            <n-form-item label="优先拉取">
              <n-switch v-model:value="preferPull" :disabled="loading" />
            </n-form-item>

            <template v-if="syncProvider === 'p2p'">
              <n-form-item label="本机名称">
                <n-input v-model:value="p2pDeviceName" :disabled="loading" placeholder="例如：Laptop / Desktop" />
              </n-form-item>
              <n-form-item label="Ticket 有效期">
                <n-select v-model:value="p2pTicketValidityMode" :options="p2pTicketValidityOptions" :disabled="loading" />
              </n-form-item>
              <n-form-item v-if="p2pTicketValidityMode === 'expires'" label="有效分钟数">
                <n-input-number v-model:value="p2pTicketTtlMinutes" :disabled="loading" :min="1" :step="10" placeholder="10" />
              </n-form-item>
              <n-form-item label="自动同步资源库">
                <n-switch v-model:value="p2pAutoSyncResourceRepo" :disabled="loading" />
              </n-form-item>
              <n-form-item label="同步间隔（分钟）">
                <n-input-number v-model:value="p2pSyncIntervalMinutes" :disabled="loading" :min="30" :step="30" placeholder="720" />
              </n-form-item>
              <n-form-item label="并发传输数">
                <n-input-number v-model:value="p2pTransferConcurrency" :disabled="loading" :min="1" :max="32" :step="1" placeholder="6" />
              </n-form-item>
              <n-form-item label="连接空闲超时（秒）">
                <n-input-number v-model:value="p2pIdleTimeoutSecs" :disabled="loading" :min="15" :step="5" placeholder="30" />
              </n-form-item>
              <n-form-item label="块级增量传输">
                <n-switch v-model:value="p2pEnableBlockDelta" :disabled="loading" />
              </n-form-item>
              <n-form-item label="自动组网（信任传播）">
                <n-switch v-model:value="p2pAutoPropagateTrust" :disabled="loading" />
              </n-form-item>
              <n-form-item label="本机 Peer ID">
                <div class="copy-field">
                  <n-input :value="p2pStatus?.peer_id || ''" readonly placeholder="保存并刷新后生成" />
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button quaternary circle :disabled="!p2pStatus?.peer_id" @click="copyP2PValue(p2pStatus?.peer_id, '本机 Peer ID')">
                        <template #icon>
                          <n-icon :component="Copy20Regular" />
                        </template>
                      </n-button>
                    </template>
                    复制本机 Peer ID
                  </n-tooltip>
                </div>
              </n-form-item>
              <n-form-item label="P2P 状态">
                <n-space vertical style="width: 100%">
                  <n-alert :type="p2pStatusAlertType" :show-icon="false">
                    {{ p2pStatusText }}
                  </n-alert>
                  <n-space>
                    <n-tag :type="p2pStatus?.enabled ? 'success' : 'warning'">
                      {{ p2pStatus?.enabled ? '已启用 P2P' : '未启用 P2P' }}
                    </n-tag>
                    <n-tag type="info">iroh {{ p2pStatus?.iroh_version || '0.98.2' }}</n-tag>
                    <n-tag :type="p2pTicketValidityMode === 'permanent' ? 'warning' : 'success'">
                      Ticket {{ p2pTicketValidityMode === 'permanent' ? '永久有效' : `${p2pTicketTtlMinutes || 10} 分钟有效` }}
                    </n-tag>
                  </n-space>
                </n-space>
              </n-form-item>
              <n-form-item label="配对 Ticket">
                <div class="copy-field copy-field--textarea">
                  <n-input v-model:value="p2pTicketText" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" :disabled="loading" placeholder="点击创建 Ticket，或粘贴另一台设备的 Ticket" />
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button quaternary circle :disabled="!p2pTicketText" @click="copyP2PValue(p2pTicketText, '配对 Ticket')">
                        <template #icon>
                          <n-icon :component="Copy20Regular" />
                        </template>
                      </n-button>
                    </template>
                    复制配对 Ticket
                  </n-tooltip>
                </div>
              </n-form-item>
              <n-form-item label="P2P 操作">
                <n-space>
                  <n-button :disabled="loading" @click="handleResourceRepoSyncSave">保存配置</n-button>
                  <n-button :disabled="loading" @click="handleResourceRepoSyncTest">测试连接</n-button>
                  <n-button :disabled="loading" @click="handleP2PRefreshStatus">刷新状态</n-button>
                  <n-button :disabled="loading" @click="handleP2PProbePeers">检测设备在线</n-button>
                  <n-button :disabled="loading || !p2pCanCreateTicket" @click="handleP2PCreateTicket">创建 Ticket</n-button>
                  <n-button :disabled="loading || !p2pTicketText" @click="handleP2PJoinTicket">加入 Ticket</n-button>
                </n-space>
              </n-form-item>
              <n-form-item label="同步操作">
                <n-space>
                  <n-button :disabled="loading || !p2pReady" type="primary" @click="handleResourceRepoSyncNow">同步资源库</n-button>
                  <n-button :disabled="loading || !p2pReady" @click="handleP2PVerifyResourceRepo">校验资源库</n-button>
                  <n-button :disabled="loading || !p2pReady" type="warning" @click="handleP2PSyncKnowledgeBase">应急同步知识库</n-button>
                  <n-button :disabled="loading || !p2pReady" @click="handleP2PVerifyKnowledgeBase">校验知识库</n-button>
                </n-space>
              </n-form-item>
              <n-form-item label="可信设备">
                <n-space vertical style="width: 100%">
                  <n-alert v-if="!p2pTrustedPeers.length" type="default" :show-icon="false">
                    暂无可信设备
                  </n-alert>
                  <n-list v-else bordered>
                    <n-list-item v-for="peer in p2pTrustedPeers" :key="peer.peer_id">
                      <n-space justify="space-between" align="center" style="width: 100%">
                        <span>{{ peer.device_name }}（{{ shortPeerId(peer.peer_id) }}）</span>
                        <n-space align="center">
                          <n-tag :type="peerStatusTagType(peer.online_status)">
                            {{ peerStatusText(peer.online_status) }}
                          </n-tag>
                          <n-text depth="3">{{ peer.status_detail || '尚未检测' }}</n-text>
                          <n-tooltip trigger="hover">
                            <template #trigger>
                              <n-button size="small" quaternary circle :disabled="!peer.peer_id" @click="copyP2PValue(peer.peer_id, `${peer.device_name || '设备'} Peer ID`)">
                                <template #icon>
                                  <n-icon :component="Copy20Regular" />
                                </template>
                              </n-button>
                            </template>
                            复制设备 Peer ID
                          </n-tooltip>
                        </n-space>
                        <n-button size="small" :disabled="loading" @click="handleP2PRemovePeer(peer.peer_id)">移除</n-button>
                      </n-space>
                    </n-list-item>
                  </n-list>
                </n-space>
              </n-form-item>
            </template>

            <template v-else-if="syncProvider === 'webdav'">
              <n-form-item label="WebDAV URL">
                <n-input v-model:value="webdavUrl" :disabled="loading" placeholder="例如：https://dav.jianguoyun.com/dav" />
              </n-form-item>
              <n-form-item label="用户名">
                <n-input v-model:value="webdavUsername" :disabled="loading" placeholder="坚果云账号" />
              </n-form-item>
              <n-form-item label="密码/应用密码">
                <n-input v-model:value="webdavPassword" :disabled="loading" type="password" show-password-on="click" placeholder="建议使用应用密码" />
              </n-form-item>
              <n-form-item label="远端目录">
                <n-input v-model:value="webdavRemoteBasePath" :disabled="loading" placeholder="例如：Guaxi/ResourceRepo" />
              </n-form-item>
            </template>

            <template v-else-if="syncProvider === 'syncthing'">
              <n-form-item label="API URL">
                <n-input v-model:value="syncthingApiUrl" :disabled="loading" placeholder="例如：http://127.0.0.1:8384" />
              </n-form-item>
              <n-form-item label="API Key">
                <n-input v-model:value="syncthingApiKey" :disabled="loading" type="password" show-password-on="click" placeholder="Syncthing GUI API Key" />
              </n-form-item>
              <n-form-item label="Folder ID">
                <n-input v-model:value="syncthingFolderId" :disabled="loading" placeholder="例如：default" />
              </n-form-item>
            </template>

            <n-form-item v-if="syncProvider !== 'p2p'" label="操作">
              <n-space>
                <n-button :disabled="loading" @click="handleResourceRepoSyncSave">保存</n-button>
                <n-button :disabled="loading" @click="handleResourceRepoSyncTest">测试连接</n-button>
                <n-button :disabled="loading" type="primary" @click="handleResourceRepoSyncNow">立即同步</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>模板库</h3>
          <p class="description">
            用于存放 Markdown 模板文件。未配置时启动会自动在程序同级目录创建 Template 文件夹并写入配置。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="模板库路径">
              <n-input v-model:value="templateRepoPath" :disabled="loading" placeholder="模板库路径" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" @click="handleTemplateRepoPick">浏览文件夹...</n-button>
                <n-button :disabled="loading" type="primary" @click="handleTemplateRepoSave">保存</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>Slidev PPT 源文件根目录</h3>
          <p class="description">
            用于存放 Slidev PPT 源文件。建议每个 PPT 使用一个子目录存放（主题、素材等），避免相互污染。未配置时将使用程序同级目录下的 SlideRepo 文件夹。
          </p>

          <n-alert type="info" v-if="slidevRepoRoot" style="max-width: 95%;">
            当前 Slidev 根目录：{{ slidevRepoRoot }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置 Slidev 根目录，将使用默认位置。
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="Slidev 根目录">
              <n-input v-model:value="slidevRepoRoot" :disabled="loading" placeholder="Slidev 根目录" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" @click="handleSlidevRepoPick">浏览文件夹...</n-button>
                <n-button :disabled="loading" type="primary" @click="handleSlidevRepoSave">保存</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>知识库远程仓库</h3>
          <p class="description">
            配置后将应用到 KnowledgeBases 根目录下的 Git 仓库。push / pull 将直接调用系统 git 命令（类似 VSCode），使用你的本机 Git 配置与凭据环境。
          </p>

          <n-alert type="info" v-if="knowledgeBasesRemoteUrl" style="max-width: 95%;">
            当前远程仓库：{{ knowledgeBasesRemoteUrl }}
          </n-alert>
          <n-alert type="warning" v-else style="max-width: 95%;">
            未配置远程仓库地址
          </n-alert>

          <n-form label-placement="left" label-width="auto" :show-require-mark="false" style="margin-top: 8px">
            <n-form-item label="远程仓库地址">
              <n-input v-model:value="knowledgeBasesRemoteUrl" :disabled="loading" placeholder="https://..." />
            </n-form-item>
            <n-form-item label="Git 用户名">
              <n-input v-model:value="knowledgeBasesGitUsername" :disabled="loading"
                placeholder="用于提交, 例如: Your Name" />
            </n-form-item>
            <n-form-item label="Git Email">
              <n-input v-model:value="knowledgeBasesGitEmail" :disabled="loading"
                placeholder="用于提交, 例如: you@example.com" />
            </n-form-item>
            <n-form-item label="操作">
              <n-space>
                <n-button :disabled="loading" type="primary" @click="handleKnowledgeBasesRemoteSave">保存并应用</n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>
      </n-space>
    </n-card>

  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, ref, onMounted, onUnmounted } from 'vue';
import {
  NCard,
  NList,
  NListItem,
  NText,
  NButton,
  NIcon,
  NInput,
  NInputNumber,
  NSpace,
  NAlert,
  NTooltip,
  NDivider,
  NForm,
  NFormItem,
  NSelect,
  NSwitch,
  NTag,
} from 'naive-ui';
import {
  Copy20Regular,
  FolderAdd20Regular,
  Folder20Regular,
  DocumentEdit16Regular,
} from '@vicons/fluent';
import { repoStore } from '../../stores/repoStore'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('NoteRepoContent')

const store = repoStore();

// 文件库根目录
const librariesRoot = ref('');

// 资源库根目录
const resourceRoot = ref('');

// 知识库根目录
const knowledgeBasesRoot = ref('');

// 模板库路径
const templateRepoPath = ref('');

// Slidev PPT 源文件根目录
const slidevRepoRoot = ref('');

// 资源库同步配置
const syncProvider = ref('none');
const preferPull = ref(false);

const webdavUrl = ref('');
const webdavUsername = ref('');
const webdavPassword = ref('');
const webdavRemoteBasePath = ref('');

const syncthingApiUrl = ref('');
const syncthingApiKey = ref('');
const syncthingFolderId = ref('');

const p2pDeviceName = ref('');
const p2pTicketValidityMode = ref('expires');
const p2pTicketTtlMinutes = ref(10);
const p2pAutoSyncResourceRepo = ref(false);
const p2pSyncIntervalMinutes = ref(720);
const p2pTransferConcurrency = ref(6);
const p2pIdleTimeoutSecs = ref(30);
const p2pEnableBlockDelta = ref(true);
const p2pAutoPropagateTrust = ref(true);
const p2pTicketText = ref('');
const p2pStatus = ref(null);
const p2pTrustedPeers = ref([]);

const syncProviderOptions = [
  { label: '不启用', value: 'none' },
  { label: 'P2P 设备同步', value: 'p2p' },
  { label: '坚果云 WebDAV', value: 'webdav' },
  { label: 'Syncthing API', value: 'syncthing' },
];

const p2pTicketValidityOptions = [
  { label: '有期限', value: 'expires' },
  { label: '永久有效', value: 'permanent' },
];

// 知识库远程仓库配置
const knowledgeBasesRemoteUrl = ref('');
const knowledgeBasesGitUsername = ref('');
const knowledgeBasesGitEmail = ref('');

const loading = ref(false);

const RESOURCE_REPO_SYNC_STATUS_KEY = 'guaxi:resourceRepoSyncStatus:v1'

const nowLabel = () => {
  try {
    return new Date().toLocaleString()
  } catch {
    return ''
  }
}

const formatInvokeError = (e) => {
  const msg = e?.message ?? (typeof e?.toString === 'function' ? e.toString() : String(e))
  const s = typeof msg === 'string' ? msg.trim() : ''
  return s || '操作失败'
}

const readSyncState = () => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY)
    if (!raw) return { status: 'idle', action: '', at: '', detail: '' }
    const v = JSON.parse(raw)
    return {
      status: String(v?.status || 'idle'),
      action: String(v?.action || ''),
      at: String(v?.at || ''),
      detail: String(v?.detail || ''),
    }
  } catch (e) {
    logger.warn('[NoteRepoContent][ResourceRepoSync] read sync state failed', e)
    return { status: 'idle', action: '', at: '', detail: '' }
  }
}

const writeSyncState = (next) => {
  try {
    localStorage.setItem(RESOURCE_REPO_SYNC_STATUS_KEY, JSON.stringify(next))
  } catch (e) {
    logger.warn('[NoteRepoContent][ResourceRepoSync] write sync state failed', e)
  }
}

const syncState = ref(readSyncState())
const syncStateRaw = ref('')
let syncPollTimer = null

const applySyncState = (next, source) => {
  if (!next) return
  syncState.value = {
    status: String(next?.status || 'idle'),
    action: String(next?.action || ''),
    at: String(next?.at || ''),
    detail: String(next?.detail || ''),
  }
  logger.info('[NoteRepoContent][ResourceRepoSync] state update', { source, state: syncState.value })
}

const refreshSyncStateFromStorage = (source) => {
  try {
    const raw = localStorage.getItem(RESOURCE_REPO_SYNC_STATUS_KEY) || ''
    if (raw && raw === syncStateRaw.value) return
    syncStateRaw.value = raw
    if (!raw) {
      applySyncState({ status: 'idle', action: '', at: '', detail: '' }, source)
      return
    }
    const v = JSON.parse(raw)
    applySyncState(v, source)
  } catch (e) {
    logger.warn('[NoteRepoContent][ResourceRepoSync] refresh sync state failed', e)
  }
}

const handleSyncStateEvent = (e) => {
  applySyncState(e?.detail, 'custom-event')
}

const handleStorageEvent = (e) => {
  if (!e) return
  if (e.key !== RESOURCE_REPO_SYNC_STATUS_KEY) return
  refreshSyncStateFromStorage('storage-event')
}

const setSyncState = (patch) => {
  const next = { ...syncState.value, ...patch }
  syncState.value = next
  writeSyncState(next)

  try {
    syncStateRaw.value = JSON.stringify(next)
  } catch {}

  try {
    window.dispatchEvent(
      new CustomEvent('guaxi:resource-repo-sync-state', {
        detail: next,
      })
    )
  } catch (e) {
    logger.warn('[NoteRepoContent][ResourceRepoSync] dispatch sync state event failed', e)
  }

  logger.info('[NoteRepoContent][ResourceRepoSync] state', next)
}

const syncUi = computed(() => {
  const s = String(syncState.value?.status || 'idle')
  if (s === 'running') {
    return { alertType: 'info', tagType: 'info', tagText: '进行中', title: '资源库同步进行中…' }
  }
  if (s === 'success') {
    return { alertType: 'success', tagType: 'success', tagText: '成功', title: '资源库同步状态正常' }
  }
  if (s === 'error') {
    return { alertType: 'error', tagType: 'error', tagText: '失败', title: '资源库同步失败' }
  }
  if (s === 'skipped') {
    return { alertType: 'warning', tagType: 'warning', tagText: '跳过', title: '资源库同步已跳过' }
  }
  return { alertType: 'default', tagType: 'default', tagText: '未运行', title: '资源库同步未运行' }
})

const p2pReady = computed(() => {
  return syncProvider.value === 'p2p' && Boolean(p2pStatus.value?.peer_id)
})

const p2pCanCreateTicket = computed(() => {
  return syncProvider.value === 'p2p'
})

const p2pStatusAlertType = computed(() => {
  if (syncProvider.value !== 'p2p') return 'warning'
  if (!p2pStatus.value?.peer_id) return 'warning'
  return 'success'
})

const p2pStatusText = computed(() => {
  if (syncProvider.value !== 'p2p') return '当前同步后端不是 P2P，请先选择 P2P 设备同步并保存配置。'
  if (!p2pStatus.value?.peer_id) return '尚未生成本机 P2P 身份，请保存配置或刷新状态。'
  const peers = p2pTrustedPeers.value.length
  return `本机 P2P 身份已就绪，可信设备 ${peers} 个。在线检测需要对端使用新版 Ticket 并保持 P2P 服务在线。`
})

const shortPeerId = (peerId) => {
  const s = String(peerId || '')
  if (s.length <= 18) return s
  return `${s.slice(0, 8)}…${s.slice(-8)}`
}

const peerStatusTagType = (status) => {
  const s = String(status || 'unknown')
  if (s === 'online') return 'success'
  if (s === 'offline') return 'error'
  return 'warning'
}

const peerStatusText = (status) => {
  const s = String(status || 'unknown')
  if (s === 'online') return '在线'
  if (s === 'offline') return '离线'
  return '未知'
}

const copyP2PValue = async (value, label) => {
  const text = String(value || '')
  if (!text) return
  const action = `复制${label}`
  try {
    await navigator.clipboard.writeText(text)
    setSyncState({ status: 'success', action, at: nowLabel(), detail: '已复制到剪贴板' })
    logger.info('[NoteRepoContent][P2PSync] copied value', { label, length: text.length })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('[NoteRepoContent][P2PSync] copy value failed:', error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg || '复制失败' })
  }
}

// 打开文件浏览器选择文件库根目录
const handleOpenFileBrowserForLibraryRoot = async () => {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });

    if (selectedPath) {
      await invoke('set_libraries_root', { path: selectedPath });
      librariesRoot.value = selectedPath;
      store.set_librariesRoot(selectedPath);
      logger.info('文件库根目录已更新为:', store.librariesRoot);
    }
  } catch (error) {
    logger.error('打开文件浏览器失败:', error);
  }
};

// 加载配置
onMounted(async () => {
  window.addEventListener('guaxi:resource-repo-sync-state', handleSyncStateEvent)
  window.addEventListener('storage', handleStorageEvent)

  refreshSyncStateFromStorage('mounted')

  try {
    syncPollTimer = setInterval(() => {
      refreshSyncStateFromStorage('poll')
    }, 800)
  } catch (e) {
    logger.warn('[NoteRepoContent][ResourceRepoSync] start sync state poll failed', e)
  }

  librariesRoot.value = await invoke('get_libraries_root');

  try {
    knowledgeBasesRoot.value = await invoke('get_knowledge_bases_root');
  } catch (error) {
    logger.error('加载知识库根目录失败:', error);
    knowledgeBasesRoot.value = '';
  }

  try {
    const remoteConfig = await invoke('get_knowledge_bases_remote_config');
    const remoteUrl = remoteConfig?.remoteUrl ?? remoteConfig?.remote_url;
    const gitUsername = remoteConfig?.gitUsername ?? remoteConfig?.git_username;
    const gitEmail = remoteConfig?.gitEmail ?? remoteConfig?.git_email;

    logger.info('[NoteRepoContent][KB Remote] loaded', {
      remoteUrl: remoteUrl ? String(remoteUrl) : '',
      gitUsername: gitUsername ? String(gitUsername) : '',
      gitEmail: gitEmail ? String(gitEmail) : '',
    });

    knowledgeBasesRemoteUrl.value = remoteUrl ? String(remoteUrl) : '';
    knowledgeBasesGitUsername.value = gitUsername ? String(gitUsername) : '';
    knowledgeBasesGitEmail.value = gitEmail ? String(gitEmail) : '';
  } catch (error) {
    logger.error('加载知识库远程仓库配置失败:', error);
    knowledgeBasesRemoteUrl.value = '';
    knowledgeBasesGitUsername.value = '';
    knowledgeBasesGitEmail.value = '';
  }

  try {
    const tplPath = await invoke('get_template_repo_path');
    templateRepoPath.value = typeof tplPath === 'string' ? tplPath : '';
  } catch (error) {
    logger.error('加载模板库路径失败:', error);
    templateRepoPath.value = '';
  }

  try {
    const slideRoot = await invoke('get_slidev_repo_root');
    slidevRepoRoot.value = typeof slideRoot === 'string' ? slideRoot : '';
  } catch (error) {
    logger.error('加载 Slidev 根目录失败:', error);
    slidevRepoRoot.value = '';
  }

  try {
    loading.value = true;
    const imageBedRoot = await invoke('get_image_bed_root');
    if (typeof imageBedRoot === 'string') {
      resourceRoot.value = imageBedRoot;
    } else {
      resourceRoot.value = '';
    }
  } catch (error) {
    logger.error('加载资源库根目录失败:', error);
    resourceRoot.value = '';
  } finally {
    loading.value = false;
  }

  try {
    const cfg = await invoke('get_resource_repo_sync_config');

    syncProvider.value = cfg?.provider ? String(cfg.provider) : 'none';
    preferPull.value = Boolean(cfg?.prefer_pull);

    webdavUrl.value = cfg?.webdav?.url ? String(cfg.webdav.url) : '';
    webdavUsername.value = cfg?.webdav?.username ? String(cfg.webdav.username) : '';
    webdavPassword.value = cfg?.webdav?.password ? String(cfg.webdav.password) : '';
    webdavRemoteBasePath.value = cfg?.webdav?.remote_base_path ? String(cfg.webdav.remote_base_path) : '';

    syncthingApiUrl.value = cfg?.syncthing?.api_url ? String(cfg.syncthing.api_url) : '';
    syncthingApiKey.value = cfg?.syncthing?.api_key ? String(cfg.syncthing.api_key) : '';
    syncthingFolderId.value = cfg?.syncthing?.folder_id ? String(cfg.syncthing.folder_id) : '';

    p2pDeviceName.value = cfg?.p2p?.device_name ? String(cfg.p2p.device_name) : '';
    p2pTicketValidityMode.value = cfg?.p2p?.ticket_validity_mode ? String(cfg.p2p.ticket_validity_mode) : 'expires';
    p2pTicketTtlMinutes.value = Number(cfg?.p2p?.ticket_ttl_minutes || 10);
    p2pAutoSyncResourceRepo.value = Boolean(cfg?.p2p?.auto_sync_resource_repo);
    p2pSyncIntervalMinutes.value = Number(cfg?.p2p?.sync_interval_minutes || 720);
    p2pTransferConcurrency.value = Number(cfg?.p2p?.transfer_concurrency || 6);
    p2pIdleTimeoutSecs.value = Number(cfg?.p2p?.idle_timeout_secs || 30);
    p2pEnableBlockDelta.value = cfg?.p2p?.enable_block_delta !== false;
    p2pAutoPropagateTrust.value = cfg?.p2p?.auto_propagate_trust !== false;
    if (syncProvider.value === 'p2p') {
      await handleP2PRefreshStatus(false);
    }

    logger.info('[NoteRepoContent][ResourceRepoSync] loaded', {
      provider: syncProvider.value,
      preferPull: preferPull.value,
      webdavUrl: webdavUrl.value,
      webdavRemoteBasePath: webdavRemoteBasePath.value,
      syncthingApiUrl: syncthingApiUrl.value,
      syncthingFolderId: syncthingFolderId.value,
      p2pDeviceName: p2pDeviceName.value,
      p2pTicketValidityMode: p2pTicketValidityMode.value,
    });
  } catch (error) {
    logger.error('加载资源库同步配置失败:', error);
  }
});

onUnmounted(() => {
  window.removeEventListener('guaxi:resource-repo-sync-state', handleSyncStateEvent)
  window.removeEventListener('storage', handleStorageEvent)

  try {
    if (syncPollTimer) {
      clearInterval(syncPollTimer)
      syncPollTimer = null
    }
  } catch (e) {
    logger.warn('[NoteRepoContent][ResourceRepoSync] stop sync state poll failed', e)
  }
})

const handleKnowledgeBasesRootPick = async () => {
  try {
    loading.value = true;
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });
    if (selectedPath) {
      await invoke('set_knowledge_bases_root', { path: selectedPath });
      knowledgeBasesRoot.value = selectedPath;
    }
  } catch (error) {
    logger.error('更新知识库根目录失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleTemplateRepoPick = async () => {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });

    if (!selectedPath) return;

    loading.value = true;
    await invoke('set_template_repo_path', { path: selectedPath });
    templateRepoPath.value = selectedPath;
  } catch (error) {
    logger.error('更新模板库路径失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleTemplateRepoSave = async () => {
  try {
    loading.value = true;
    await invoke('set_template_repo_path', { path: templateRepoPath.value });
  } catch (error) {
    logger.error('更新模板库路径失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleSlidevRepoPick = async () => {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });

    if (!selectedPath) return;

    loading.value = true;
    await invoke('set_slidev_repo_root', { path: selectedPath });
    slidevRepoRoot.value = selectedPath;
  } catch (error) {
    logger.error('更新 Slidev 根目录失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleSlidevRepoSave = async () => {
  try {
    loading.value = true;
    await invoke('set_slidev_repo_root', { path: slidevRepoRoot.value });
  } catch (error) {
    logger.error('更新 Slidev 根目录失败:', error);
  } finally {
    loading.value = false;
  }
};

const handleKnowledgeBasesRemoteSave = async () => {
  try {
    loading.value = true;

    const remoteUrl = String(knowledgeBasesRemoteUrl.value || '').trim();

    logger.info('[NoteRepoContent][KB Remote] saving', {
      remoteUrl: remoteUrl,
      gitUsername: String(knowledgeBasesGitUsername.value || '').trim(),
      gitEmail: String(knowledgeBasesGitEmail.value || '').trim(),
    });

    await invoke('set_knowledge_bases_remote_config', {
      remoteUrl: knowledgeBasesRemoteUrl.value,
      username: '',
      token: '',
      gitUsername: knowledgeBasesGitUsername.value,
      gitEmail: knowledgeBasesGitEmail.value,
    });

    try {
      const remoteConfig = await invoke('get_knowledge_bases_remote_config');
      logger.info('[NoteRepoContent][KB Remote] saved, backend says', {
        remoteUrl: remoteConfig?.remote_url ?? remoteConfig?.remoteUrl,
      });
    } catch (e) {
      logger.warn('[NoteRepoContent][KB Remote] verify after save failed', e);
    }
  } catch (error) {
    logger.error('保存知识库远程仓库配置失败:', error);
  } finally {
    loading.value = false;
  }
};

// 打开文件浏览器选择资源库根目录
const handleResourceRootPick = async () => {
  try {
    loading.value = true;
    const selectedPath = await open({
      directory: true,
      multiple: false,
    });
    if (selectedPath) {
      await invoke('set_image_bed_root', { path: selectedPath });
      resourceRoot.value = selectedPath;
    }
  } catch (error) {
    logger.error('更新资源库根目录失败:', error);
  } finally {
    loading.value = false;
  }
};

const sanitizeWebdavText = (v) => {
  const s = String(v || '').trim()
  return s.replace(/^`+|`+$/g, '').replace(/^"+|"+$/g, '').replace(/^'+|'+$/g, '').trim()
}

const buildResourceRepoSyncConfig = () => {
  return {
    provider: String(syncProvider.value || 'none'),
    prefer_pull: Boolean(preferPull.value),
    webdav: {
      url: sanitizeWebdavText(webdavUrl.value),
      username: String(webdavUsername.value || '').trim(),
      password: String(webdavPassword.value || ''),
      remote_base_path: sanitizeWebdavText(webdavRemoteBasePath.value),
    },
    syncthing: {
      api_url: String(syncthingApiUrl.value || '').trim(),
      api_key: String(syncthingApiKey.value || ''),
      folder_id: String(syncthingFolderId.value || '').trim(),
    },
    p2p: {
      device_name: String(p2pDeviceName.value || '').trim(),
      ticket_validity_mode: String(p2pTicketValidityMode.value || 'expires'),
      ticket_ttl_minutes: Math.max(1, Number(p2pTicketTtlMinutes.value || 10)),
      auto_sync_resource_repo: Boolean(p2pAutoSyncResourceRepo.value),
      sync_interval_minutes: Math.max(30, Number(p2pSyncIntervalMinutes.value || 720)),
      transfer_concurrency: Math.min(32, Math.max(1, Number(p2pTransferConcurrency.value || 6))),
      idle_timeout_secs: Math.max(15, Number(p2pIdleTimeoutSecs.value || 30)),
      enable_block_delta: Boolean(p2pEnableBlockDelta.value),
      auto_propagate_trust: Boolean(p2pAutoPropagateTrust.value),
    },
  }
}

const handleP2PRefreshStatus = async (withUi = true) => {
  const action = '刷新 P2P 状态'
  try {
    if (withUi) {
      loading.value = true
      setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    }
    const status = await invoke('p2p_sync_status')
    p2pStatus.value = status || null
    p2pTrustedPeers.value = Array.isArray(status?.trusted_peers) ? status.trusted_peers : []
    if (withUi) {
      setSyncState({ status: 'success', action, at: nowLabel(), detail: `可信设备 ${p2pTrustedPeers.value.length} 个` })
    }
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('[NoteRepoContent][P2PSync] refresh status failed:', error)
    if (withUi) {
      setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
    }
  } finally {
    if (withUi) loading.value = false
  }
}

const handleP2PCreateTicket = async () => {
  const action = '创建 P2P Ticket'
  try {
    loading.value = true
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    await invoke('set_resource_repo_sync_config', { configInput: buildResourceRepoSyncConfig() })
    const res = await invoke('p2p_sync_create_ticket')
    p2pTicketText.value = String(res?.ticket || '')
    await handleP2PRefreshStatus(false)
    const expiry = res?.expires_at ? `有效期至 ${res.expires_at}` : '永久有效'
    setSyncState({ status: 'success', action, at: nowLabel(), detail: expiry })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('[NoteRepoContent][P2PSync] create ticket failed:', error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false
  }
}

const handleP2PJoinTicket = async () => {
  const action = '加入 P2P Ticket'
  try {
    loading.value = true
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    const res = await invoke('p2p_sync_join_ticket', { ticket: String(p2pTicketText.value || '') })
    await handleP2PRefreshStatus(false)
    setSyncState({ status: 'success', action, at: nowLabel(), detail: typeof res === 'string' ? res : '已加入可信设备' })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('[NoteRepoContent][P2PSync] join ticket failed:', error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false
  }
}

const handleP2PProbePeers = async () => {
  const action = '检测 P2P 设备在线'
  try {
    loading.value = true
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    const peers = await invoke('p2p_sync_probe_peers')
    p2pTrustedPeers.value = Array.isArray(peers) ? peers : []
    await handleP2PRefreshStatus(false)
    const online = p2pTrustedPeers.value.filter((p) => String(p?.online_status || '') === 'online').length
    const unknown = p2pTrustedPeers.value.filter((p) => String(p?.online_status || 'unknown') === 'unknown').length
    setSyncState({
      status: unknown > 0 ? 'skipped' : 'success',
      action,
      at: nowLabel(),
      detail: `在线 ${online}，未知 ${unknown}，共 ${p2pTrustedPeers.value.length} 个设备`,
    })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('[NoteRepoContent][P2PSync] probe peers failed:', error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false
  }
}

const handleP2PRemovePeer = async (peerId) => {
  const action = '移除 P2P 设备'
  try {
    loading.value = true
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    const res = await invoke('p2p_sync_remove_peer', { peerId })
    await handleP2PRefreshStatus(false)
    setSyncState({ status: 'success', action, at: nowLabel(), detail: typeof res === 'string' ? res : '已移除' })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('[NoteRepoContent][P2PSync] remove peer failed:', error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false
  }
}

const runP2PAction = async (action, command) => {
  try {
    loading.value = true
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })
    const res = await invoke(command)
    setSyncState({ status: 'success', action, at: nowLabel(), detail: typeof res === 'string' ? res : '完成' })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error(`[NoteRepoContent][P2PSync] ${action} failed:`, error)
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false
  }
}

const handleP2PVerifyResourceRepo = () => runP2PAction('P2P 校验资源库', 'p2p_sync_verify_resource_repo')
const handleP2PSyncKnowledgeBase = () => runP2PAction('P2P 应急同步知识库', 'p2p_sync_knowledge_base_now')
const handleP2PVerifyKnowledgeBase = () => runP2PAction('P2P 校验知识库', 'p2p_sync_verify_knowledge_base')

const handleResourceRepoSyncSave = async () => {
  const action = '保存配置'
  try {
    loading.value = true;
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })

    await invoke('set_resource_repo_sync_config', {
      configInput: buildResourceRepoSyncConfig(),
    });
    if (syncProvider.value === 'p2p') {
      await handleP2PRefreshStatus(false)
    }

    setSyncState({ status: 'success', action, at: nowLabel(), detail: '配置已保存' })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('保存资源库同步配置失败:', error);
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false;
  }
}

const handleResourceRepoSyncTest = async () => {
  const action = '测试连接'
  try {
    loading.value = true;
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })

    if (syncProvider.value === 'p2p') {
      await invoke('set_resource_repo_sync_config', {
        configInput: buildResourceRepoSyncConfig(),
      });
    }

    const res = await invoke('resource_repo_test_connection');
    const text = typeof res === 'string' ? res : '连接正常'
    if (syncProvider.value === 'p2p') {
      await handleP2PRefreshStatus(false)
    }
    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('测试资源库同步连接失败:', error);
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false;
  }
}

const handleResourceRepoSyncNow = async () => {
  const action = '立即同步'
  try {
    loading.value = true;
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })

    const res = await invoke('resource_repo_sync_now');
    const text = typeof res === 'string' ? res : '同步完成'
    setSyncState({ status: 'success', action, at: nowLabel(), detail: text })
  } catch (error) {
    const msg = formatInvokeError(error)
    logger.error('资源库同步失败:', error);
    setSyncState({ status: 'error', action, at: nowLabel(), detail: msg })
  } finally {
    loading.value = false;
  }
}

</script>

<style scoped>
.preference-wrapper {
  width: 100%;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  max-width: 95%;
}

.section-header h3 {
  margin: 0;
}

.description {
  margin-top: 4px;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

.copy-field {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32px;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.copy-field--textarea {
  align-items: start;
}

.copy-field--textarea :deep(.n-button) {
  margin-top: 2px;
}

.tip {
  margin-top: 8px;
}

.n-list-item {
  padding: 12px 16px;
}

.n-list-item__suffix {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sync-status-alert {
  margin-top: 8px;
}

.sync-status-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sync-status-title {
  font-size: 13px;
  font-weight: 600;
}

.sync-status-sub {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  line-height: 1.25;
}
</style>
