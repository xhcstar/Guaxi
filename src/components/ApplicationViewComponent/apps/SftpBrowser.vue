<template>
  <div class="sftp-root">
    <div class="sftp-head">
      <span>SFTP 文件</span>
      <n-button v-if="!sftpId" size="tiny" type="primary" :loading="loading" :disabled="!profileId" @click="connect">连接</n-button>
      <n-button v-else size="tiny" @click="disconnect">断开</n-button>
    </div>
    <div class="sftp-bar" v-if="!sftpId">
      <n-select v-model:value="profileId" size="tiny" :options="profileOptions" placeholder="选择档案" />
    </div>
    <template v-else>
      <div class="sftp-path">
        <n-button size="tiny" quaternary @click="up" title="上级"><Icon :icon="upIcon" width="14" height="14" /></n-button>
        <n-input v-model:value="cwd" size="tiny" @keyup.enter="list(cwd)" />
        <n-button size="tiny" quaternary @click="list(cwd)" title="刷新"><Icon :icon="refreshIcon" width="14" height="14" /></n-button>
      </div>
      <div class="sftp-tools">
        <n-button size="tiny" @click="upload"><Icon :icon="uploadIcon" width="13" height="13" />上传</n-button>
        <n-button size="tiny" @click="mkdir"><Icon :icon="folderPlusIcon" width="13" height="13" />新建夹</n-button>
      </div>
      <div class="sftp-list">
        <div v-for="e in entries" :key="e.name" class="sftp-item" @dblclick="e.is_dir ? enterDir(e.name) : download(e)">
          <Icon :icon="e.is_dir ? folderIcon : fileIcon" width="14" height="14" :class="e.is_dir ? 'sftp-dir' : 'sftp-file'" />
          <span class="sftp-name" :title="e.name">{{ e.name }}</span>
          <span class="sftp-size">{{ e.is_dir ? '' : fmtSize(e.size) }}</span>
          <span class="sftp-actions">
            <Icon v-if="!e.is_dir" :icon="downloadIcon" width="13" height="13" title="下载" @click.stop="download(e)" />
            <Icon :icon="editIcon" width="13" height="13" title="重命名" @click.stop="rename(e)" />
            <Icon :icon="trashIcon" width="13" height="13" title="删除" @click.stop="remove(e)" />
          </span>
        </div>
        <div v-if="!entries.length" class="sftp-empty">空目录</div>
      </div>
    </template>
    <span v-if="error" class="sftp-err">{{ error }}</span>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { Icon } from '@iconify/vue'
import upIcon from '@iconify-icons/lucide/arrow-up'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import uploadIcon from '@iconify-icons/lucide/upload'
import downloadIcon from '@iconify-icons/lucide/download'
import folderPlusIcon from '@iconify-icons/lucide/folder-plus'
import folderIcon from '@iconify-icons/lucide/folder'
import fileIcon from '@iconify-icons/lucide/file'
import editIcon from '@iconify-icons/lucide/pencil'
import trashIcon from '@iconify-icons/lucide/trash-2'

const props = defineProps({
  profiles: { type: Array, default: () => [] },
  // 当前激活 SSH 会话的档案 id：有值即自动连到同主机的 SFTP（见下方 watch）。
  activeProfileId: { type: [String, null], default: null },
})
const profileId = ref(null)
const profileOptions = computed(() => props.profiles.map(p => ({ label: p.name || p.host, value: p.id })))
const sftpId = ref('')
const connectedProfileId = ref('')
const cwd = ref('/')
const entries = ref([])
const error = ref('')
const loading = ref(false)

function joinPath(dir, name) {
  const base = dir.replace(/\/+$/, '')
  return (base === '' ? '' : base) + '/' + name
}
function parentPath(dir) {
  const b = dir.replace(/\/+$/, '')
  const i = b.lastIndexOf('/')
  return i <= 0 ? '/' : b.slice(0, i)
}
function fmtSize(n) {
  if (n < 1024) return n + 'B'
  if (n < 1048576) return (n / 1024).toFixed(1) + 'K'
  return (n / 1048576).toFixed(1) + 'M'
}

async function connect() {
  error.value = ''; loading.value = true
  try {
    const r = await invoke('sftp_connect', { profileId: profileId.value })
    sftpId.value = r.session_id
    connectedProfileId.value = profileId.value
    cwd.value = r.home || '/'
    await list(cwd.value)
  } catch (e) { error.value = '连接失败: ' + (e?.message || e) } finally { loading.value = false }
}
async function disconnect() {
  if (sftpId.value) { await invoke('sftp_close', { sessionId: sftpId.value }).catch(() => {}); sftpId.value = ''; connectedProfileId.value = ''; entries.value = [] }
}

// SSH 连上后自动把 SFTP 连到同一主机；切到别的主机 Tab 时自动重连；该主机会话全关则不动已连接。
watch(() => props.activeProfileId, async (pid) => {
  if (!pid || pid === connectedProfileId.value || loading.value) return
  if (sftpId.value) await disconnect()
  profileId.value = pid
  await connect()
}, { immediate: true })
async function list(path) {
  if (!sftpId.value) return
  error.value = ''
  try {
    entries.value = await invoke('sftp_list', { sessionId: sftpId.value, path })
    cwd.value = path
  } catch (e) { error.value = '列目录失败: ' + (e?.message || e) }
}
function enterDir(name) { list(joinPath(cwd.value, name)) }
function up() { list(parentPath(cwd.value)) }

async function download(e) {
  try {
    const local = await save({ title: '保存到', defaultPath: e.name })
    if (!local) return
    const n = await invoke('sftp_download', { sessionId: sftpId.value, remotePath: joinPath(cwd.value, e.name), localPath: local })
    error.value = `已下载 ${fmtSize(n)}`
  } catch (err) { error.value = '下载失败: ' + (err?.message || err) }
}
async function upload() {
  try {
    const sel = await open({ multiple: false, directory: false, title: '选择要上传的文件' })
    if (!sel) return
    const local = typeof sel === 'string' ? sel : sel?.path
    const name = local.replace(/\\/g, '/').split('/').pop()
    await invoke('sftp_upload', { sessionId: sftpId.value, localPath: local, remotePath: joinPath(cwd.value, name) })
    await list(cwd.value)
  } catch (err) { error.value = '上传失败: ' + (err?.message || err) }
}
async function mkdir() {
  const name = window.prompt('新建目录名')
  if (!name) return
  try { await invoke('sftp_mkdir', { sessionId: sftpId.value, path: joinPath(cwd.value, name) }); await list(cwd.value) }
  catch (e) { error.value = '新建失败: ' + (e?.message || e) }
}
async function remove(e) {
  if (!window.confirm(`删除 ${e.name}?`)) return
  try { await invoke('sftp_remove', { sessionId: sftpId.value, path: joinPath(cwd.value, e.name), isDir: e.is_dir }); await list(cwd.value) }
  catch (err) { error.value = '删除失败: ' + (err?.message || err) }
}
async function rename(e) {
  const to = window.prompt('重命名为', e.name)
  if (!to || to === e.name) return
  try { await invoke('sftp_rename', { sessionId: sftpId.value, from: joinPath(cwd.value, e.name), to: joinPath(cwd.value, to) }); await list(cwd.value) }
  catch (err) { error.value = '重命名失败: ' + (err?.message || err) }
}
</script>

<style scoped>
.sftp-root { display: flex; flex-direction: column; min-height: 0; font-family: var(--tw-ui-font); }
.sftp-head { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; font-size: 12px; font-weight: 600; border-bottom: 1px solid var(--guaxi-border); }
.sftp-bar { padding: 8px 12px; }
.sftp-path { display: flex; gap: 4px; align-items: center; padding: 6px 8px; }
.sftp-tools { display: flex; gap: 6px; padding: 0 8px 6px; }
.sftp-list { flex: 1 1 auto; overflow: auto; min-height: 0; font-size: 12px; }
.sftp-item { display: flex; align-items: center; gap: 6px; padding: 4px 10px; cursor: default; }
.sftp-item:hover { background: var(--tw-accent-soft, rgba(12,166,192,0.12)); }
.sftp-item:hover .sftp-actions { opacity: 1; }
.sftp-dir { color: #e0a53f; }
.sftp-file { color: var(--guaxi-text-secondary); }
.sftp-name { flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sftp-size { color: var(--guaxi-text-secondary); font-family: var(--tw-mono-font); flex: 0 0 auto; }
.sftp-actions { display: inline-flex; gap: 6px; opacity: 0; color: var(--guaxi-text-secondary); }
.sftp-actions :hover { color: var(--tw-accent); }
.sftp-empty { padding: 12px; color: var(--guaxi-text-secondary); }
.sftp-err { padding: 6px 12px; font-size: 12px; color: #e5484d; }
</style>
