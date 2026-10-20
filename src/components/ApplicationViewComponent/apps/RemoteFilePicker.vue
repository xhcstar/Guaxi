<template>
  <n-modal :show="show" :mask-closable="false" style="width:780px;max-width:94vw" @update:show="v => emit('update:show', v)">
    <div class="rfp">
      <div class="rfp-head">
        <Icon :icon="folderIcon" width="17" height="17" class="rfp-ico" />
        <span class="rfp-title">{{ title }}</span>
        <span class="rfp-mode">{{ mode === 'dir' ? '选择目录' : '选择文件' }}</span>
        <button class="rfp-ib" title="关闭" @click="emit('update:show', false)"><Icon :icon="xIcon" width="16" height="16" /></button>
      </div>

      <div class="rfp-bar">
        <button class="rfp-ib" title="上一级" :disabled="!sessionId || cwd === '/'" @click="goUp"><Icon :icon="upIcon" width="16" height="16" /></button>
        <button class="rfp-ib" title="主目录" :disabled="!sessionId" @click="goHome"><Icon :icon="homeIcon" width="16" height="16" /></button>
        <button class="rfp-ib" title="刷新" :disabled="!sessionId || loading" @click="go(cwd, false)"><Icon :icon="refreshIcon" width="16" height="16" /></button>
        <n-input v-model:value="pathInput" size="small" placeholder="远端路径" :disabled="!sessionId" @keyup.enter="go(pathInput, true)" />
        <n-button size="small" :disabled="!sessionId" @click="go(pathInput, true)">前往</n-button>
      </div>

      <div class="rfp-body">
        <div v-if="error" class="rfp-err">{{ error }}</div>
        <div v-else-if="!profileId" class="rfp-hint">先在左侧选择一个 SSH 档案。</div>
        <div v-else-if="loading" class="rfp-hint">读取远端目录…</div>
        <div v-else-if="!entries.length" class="rfp-hint">这个目录是空的。</div>
        <div v-else class="rfp-list">
          <div
            v-for="e in entries"
            :key="e.name"
            class="rfp-row"
            :class="{ dir: e.is_dir, sel: !e.is_dir && selected && selected.name === e.name }"
            @click="onRow(e)"
            @dblclick="onDouble(e)"
          >
            <Icon :icon="e.is_dir ? folderIcon : fileIcon" width="16" height="16" class="rfp-row-ico" />
            <span class="rfp-name">{{ e.name }}</span>
            <span class="rfp-size">{{ e.is_dir ? '' : fmtSize(e.size) }}</span>
          </div>
        </div>
      </div>

      <div class="rfp-foot">
        <span class="rfp-cur" :title="cwd">{{ mode === 'dir' ? '当前目录' : '已选' }}：{{ mode === 'dir' ? cwd : selectedPath || '（未选择文件）' }}</span>
        <span class="rfp-spacer"></span>
        <n-button size="small" @click="emit('update:show', false)">取消</n-button>
        <n-button size="small" type="primary" :disabled="!canConfirm" @click="confirm">
          {{ mode === 'dir' ? '选择此目录' : '选择此文件' }}
        </n-button>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '@iconify/vue'
import folderIcon from '@iconify-icons/lucide/folder'
import fileIcon from '@iconify-icons/lucide/file'
import upIcon from '@iconify-icons/lucide/arrow-up'
import homeIcon from '@iconify-icons/lucide/home'
import refreshIcon from '@iconify-icons/lucide/refresh-cw'
import xIcon from '@iconify-icons/lucide/x'

const props = defineProps({
  show: { type: Boolean, default: false },
  profileId: { type: String, default: '' },
  mode: { type: String, default: 'file' }, // file | dir
  title: { type: String, default: '选择远端文件' },
  initialPath: { type: String, default: '' },
})
const emit = defineEmits(['update:show', 'pick'])

const sessionId = ref('')
const home = ref('')
const cwd = ref('')
const pathInput = ref('')
const entries = ref([])
const selected = ref(null)
const loading = ref(false)
const error = ref('')

const selectedPath = computed(() => selected.value ? join(cwd.value, selected.value.name) : '')
const canConfirm = computed(() => props.mode === 'dir' ? !!sessionId.value && !!cwd.value : !!selectedPath.value)

function join(dir, name) {
  if (!dir || dir === '/') return '/' + name
  return dir.replace(/\/+$/, '') + '/' + name
}
function parentOf(p) {
  const s = String(p || '/').replace(/\/+$/, '')
  if (!s || s === '/') return '/'
  const i = s.lastIndexOf('/')
  return i <= 0 ? '/' : s.slice(0, i)
}
function fmtSize(n) {
  const v = Number(n) || 0
  if (v < 1024) return v + ' B'
  if (v < 1024 * 1024) return (v / 1024).toFixed(1) + ' KB'
  if (v < 1024 * 1024 * 1024) return (v / 1024 / 1024).toFixed(1) + ' MB'
  return (v / 1024 / 1024 / 1024).toFixed(1) + ' GB'
}

let sessionGen = 0

async function closeSession() {
  sessionGen += 1
  const id = sessionId.value
  sessionId.value = ''
  if (id) { try { await invoke('sftp_close', { sessionId: id }) } catch (_) {} }
  entries.value = []
  selected.value = null
  cwd.value = ''
  home.value = ''
  pathInput.value = ''
}

async function openSession() {
  await closeSession()
  const gen = sessionGen
  error.value = ''
  if (!props.profileId) return
  loading.value = true
  try {
    const r = await invoke('sftp_connect', { profileId: props.profileId })
    // 连接期间用户可能关了弹窗或换了档案：丢弃这次结果。
    if (gen !== sessionGen) {
      try { await invoke('sftp_close', { sessionId: r.session_id }) } catch (_) {}
      return
    }
    sessionId.value = r.session_id
    home.value = r.home || '/'
    await go(props.initialPath?.trim() || home.value, true)
  } catch (e) {
    if (gen === sessionGen) error.value = '连接 SFTP 失败: ' + (e?.message || e)
  } finally {
    if (gen === sessionGen) loading.value = false
  }
}

async function go(path, canonical) {
  const id = sessionId.value
  if (!id) return
  loading.value = true
  error.value = ''
  selected.value = null
  try {
    let p = String(path || home.value || '/').trim() || '/'
    if (canonical) p = await invoke('sftp_canonicalize', { sessionId: id, path: p })
    // 打开会话期间可能已关闭/切换档案，回来时要丢掉过期结果。
    if (sessionId.value !== id) return
    cwd.value = p
    pathInput.value = p
    entries.value = await invoke('sftp_list', { sessionId: id, path: p })
  } catch (e) {
    if (sessionId.value === id) error.value = '读取目录失败: ' + (e?.message || e)
  } finally {
    if (sessionId.value === id) loading.value = false
  }
}

function goUp() { go(parentOf(cwd.value), false) }
function goHome() { if (home.value) go(home.value, false) }
function onRow(e) {
  if (e.is_dir) go(join(cwd.value, e.name), false)
  else selected.value = e
}
function onDouble(e) {
  if (e.is_dir) { go(join(cwd.value, e.name), false); return }
  selected.value = e
  confirm()
}
function confirm() {
  if (!canConfirm.value) return
  emit('pick', props.mode === 'dir' ? cwd.value : selectedPath.value)
  emit('update:show', false)
}

watch(() => props.show, (v) => { if (v) openSession(); else closeSession() })
watch(() => props.profileId, () => { if (props.show) openSession() })
</script>

<style scoped>
.rfp { display: flex; flex-direction: column; max-height: 84vh; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); border-radius: var(--tw-radius); box-shadow: var(--tw-elev); overflow: hidden; }
.rfp-head { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--guaxi-border); font-weight: 600; }
.rfp-ico { color: var(--tw-accent); }
.rfp-title { font-size: 16px; }
.rfp-mode { font-size: 13px; padding: 1px 8px; border-radius: 999px; background: var(--tw-accent-soft); color: var(--tw-accent); }
.rfp-ib { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; flex: 0 0 auto; padding: 0; border: none; border-radius: 6px; background: transparent; color: var(--guaxi-text-secondary); cursor: pointer; }
.rfp-ib:hover:not(:disabled) { background: var(--tw-accent-soft); color: var(--tw-accent); }
.rfp-ib:disabled { opacity: .35; cursor: not-allowed; }
.rfp-head .rfp-ib { margin-left: auto; }
.rfp-bar { display: flex; align-items: center; gap: 6px; padding: 10px 14px; border-bottom: 1px solid var(--guaxi-border-weak, rgba(0,0,0,.06)); }
.rfp-body { flex: 1 1 auto; min-height: 280px; max-height: 52vh; overflow: auto; padding: 6px 8px; }
.rfp-list { display: flex; flex-direction: column; }
.rfp-row { display: flex; align-items: center; gap: 8px; padding: 7px 10px; border-radius: 6px; cursor: pointer; font-size: 15px; }
.rfp-row:hover { background: var(--tw-accent-soft); }
.rfp-row.sel { background: var(--tw-accent-soft); box-shadow: inset 3px 0 0 var(--tw-accent); color: var(--tw-accent); }
.rfp-row-ico { flex: 0 0 auto; color: var(--guaxi-text-secondary); }
.rfp-row.dir .rfp-row-ico { color: var(--tw-accent); }
.rfp-name { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--tw-mono-font); }
.rfp-size { flex: 0 0 auto; font-size: 13.5px; color: var(--guaxi-text-secondary); }
.rfp-hint { padding: 20px 10px; font-size: 15px; color: var(--guaxi-text-secondary); }
.rfp-err { padding: 12px 10px; font-size: 15px; color: #e5484d; white-space: pre-wrap; }
.rfp-foot { display: flex; align-items: center; gap: 8px; padding: 10px 14px; border-top: 1px solid var(--guaxi-border); }
.rfp-cur { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; color: var(--guaxi-text-secondary); font-family: var(--tw-mono-font); }
.rfp-spacer { flex: 1 1 auto; }
</style>
