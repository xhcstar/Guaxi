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

            <template v-if="syncProvider === 'webdav'">
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

            <n-form-item label="操作">
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

const syncProviderOptions = [
  { label: '不启用', value: 'none' },
  { label: '坚果云 WebDAV', value: 'webdav' },
  { label: 'Syncthing API', value: 'syncthing' },
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

    logger.info('[NoteRepoContent][ResourceRepoSync] loaded', {
      provider: syncProvider.value,
      preferPull: preferPull.value,
      webdavUrl: webdavUrl.value,
      webdavRemoteBasePath: webdavRemoteBasePath.value,
      syncthingApiUrl: syncthingApiUrl.value,
      syncthingFolderId: syncthingFolderId.value,
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
  }
}

const handleResourceRepoSyncSave = async () => {
  const action = '保存配置'
  try {
    loading.value = true;
    setSyncState({ status: 'running', action, at: nowLabel(), detail: '' })

    await invoke('set_resource_repo_sync_config', {
      configInput: buildResourceRepoSyncConfig(),
    });

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

    const res = await invoke('resource_repo_test_connection');
    const text = typeof res === 'string' ? res : '连接正常'
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