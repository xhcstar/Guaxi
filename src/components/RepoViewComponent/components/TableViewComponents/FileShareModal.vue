<template>
  <n-modal :show="show" @update:show="(v) => emit('update:show', v)" :mask-closable="true" :auto-focus="false">
    <div class="fsm-card">
      <div class="fsm-head">
        <span class="fsm-title">分享 · {{ row?.importFileName || '文件' }}</span>
        <button class="fsm-close" @click="emit('update:show', false)">×</button>
      </div>

      <div class="fsm-body">
        <div v-if="loading" class="fsm-hint">正在开启局域网分享…</div>
        <div v-else-if="error" class="fsm-error">{{ error }}</div>

        <template v-else-if="links">
          <div class="fsm-qr" v-html="qrSvg"></div>
          <p class="fsm-scan">用微信 / QQ「扫一扫」即可在手机浏览器打开此文件</p>

          <div class="fsm-field">
            <span class="fsm-label">浏览链接</span>
            <div class="fsm-row">
              <input class="fsm-input" :value="links.view_url" readonly @focus="selectAll" />
              <n-button size="small" @click="copy(links.view_url)">复制</n-button>
            </div>
          </div>
          <div class="fsm-field">
            <span class="fsm-label">下载链接</span>
            <div class="fsm-row">
              <input class="fsm-input" :value="links.download_url" readonly @focus="selectAll" />
              <n-button size="small" @click="copy(links.download_url)">复制</n-button>
            </div>
          </div>

          <p class="fsm-foot">
            同一局域网内的设备可扫码 / 打开链接浏览、下载。
            <a class="fsm-link" @click="copy(links.site_url)">复制整库浏览链接</a>
            · 关闭分享后链接立即失效。
          </p>
          <div class="fsm-ops">
            <n-button size="small" type="error" ghost :loading="stopping" @click="stopShare">关闭对外分享</n-button>
          </div>
        </template>
      </div>
    </div>
  </n-modal>
</template>

<script setup>
import { ref, watch } from 'vue'
import { NModal, NButton, useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const props = defineProps({
  show: { type: Boolean, default: false },
  row: { type: Object, default: null },
})
const emit = defineEmits(['update:show'])
const message = useMessage()

const loading = ref(false)
const stopping = ref(false)
const error = ref('')
const links = ref(null)
const qrSvg = ref('')

const load = async () => {
  if (!props.row) return
  loading.value = true
  error.value = ''
  links.value = null
  qrSvg.value = ''
  try {
    const res = await invoke('library_site_file_share_link', {
      library: props.row.library || '',
      fileName: props.row.importFileName || '',
      fileType: props.row.type || '',
    })
    links.value = res
    try {
      qrSvg.value = await invoke('library_site_qr_svg', { text: res.view_url })
    } catch (e) {
      qrSvg.value = ''
    }
  } catch (e) {
    error.value = '开启分享失败:' + String(e?.message || e)
  } finally {
    loading.value = false
  }
}

const stopShare = async () => {
  stopping.value = true
  try {
    await invoke('library_site_stop_share')
    message.success('已关闭对外分享,此前链接已失效')
    emit('update:show', false)
  } catch (e) {
    message.error('关闭失败:' + String(e?.message || e))
  } finally {
    stopping.value = false
  }
}

const copy = async (text) => {
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
    message.success('已复制')
  } catch (e) {
    message.error('复制失败')
  }
}

const selectAll = (e) => e.target.select()

watch(
  () => props.show,
  (v) => {
    if (v) load()
  }
)
</script>

<style scoped>
.fsm-card {
  width: 420px;
  max-width: 92vw;
  /* 与主界面统一字体 */
  font-family: var(--guaxi-editor-font-family, 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New');
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}
.fsm-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--guaxi-border);
}
.fsm-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.fsm-close {
  border: none;
  background: none;
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
  color: var(--guaxi-text-secondary);
}
.fsm-close:hover { color: var(--guaxi-text-primary); }
.fsm-body { padding: 18px; }
.fsm-hint,
.fsm-error { font-size: 13px; color: var(--guaxi-text-secondary); padding: 16px 0; text-align: center; }
.fsm-error { color: var(--guaxi-error, #d03050); }
.fsm-qr {
  display: grid;
  place-items: center;
  padding: 14px;
  border: 1px solid var(--guaxi-border);
  border-radius: 12px;
  background: #ffffff;
}
.fsm-qr :deep(svg) { width: 200px; height: 200px; }
.fsm-scan { font-size: 12px; color: var(--guaxi-text-secondary); text-align: center; margin: 10px 0 4px; }
.fsm-field { margin-top: 12px; }
.fsm-label { font-size: 12px; color: var(--guaxi-text-secondary); }
.fsm-row { display: flex; gap: 8px; margin-top: 4px; }
.fsm-input {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  padding: 6px 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 7px;
  background: var(--guaxi-app-bg);
  color: var(--guaxi-text-primary);
}
.fsm-foot { font-size: 12px; color: var(--guaxi-text-secondary); line-height: 1.6; margin-top: 14px; }
.fsm-link { color: #1890ff; cursor: pointer; }
.fsm-ops { margin-top: 14px; display: flex; justify-content: flex-end; }
</style>
