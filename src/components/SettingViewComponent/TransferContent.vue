<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="文件传输">
      <h3>WebRTC 文件传输</h3>
      <p class="setting-page-desc">
        房间码模式需要一个“信令服务”（只中转 WebRTC 协商信息，不传文件内容）。此处配置后，文件传输助手将不再需要手动填写信令地址。
      </p>
      <n-form label-placement="left" label-width="auto" :show-require-mark="false">
        <n-form-item label="信令服务地址">
          <n-input
            v-model:value="webrtcSignalUrl"
            placeholder="例如：wss://your-signal.example.com/ft/signal"
            :disabled="loading || webrtcSignalSaving"
          />
        </n-form-item>
        <n-form-item>
          <n-button
            type="primary"
            size="small"
            :disabled="loading || webrtcSignalSaving"
            @click="handleWebrtcSignalUrlSave"
          >
            保存
          </n-button>
        </n-form-item>
      </n-form>
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, onMounted } from 'vue'
import { NCard, NForm, NFormItem, NInput, NButton } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('TransferContent')

const loading = ref(false)
const webrtcSignalSaving = ref(false)
const webrtcSignalUrl = ref('')

onMounted(async () => {
  try {
    loading.value = true
    const signalUrl = await invoke('get_webrtc_signal_url')
    webrtcSignalUrl.value = typeof signalUrl === 'string' ? signalUrl : ''
  } catch (error) {
    logger.error('加载文件传输设置失败:', error)
    webrtcSignalUrl.value = ''
  } finally {
    loading.value = false
  }
})

const handleWebrtcSignalUrlSave = async () => {
  try {
    webrtcSignalSaving.value = true
    const url = String(webrtcSignalUrl.value || '').trim()

    logger.info('[Transfer] 保存 WebRTC 信令服务地址:', url)

    await invoke('set_webrtc_signal_url', { url })

    try {
      const saved = await invoke('get_webrtc_signal_url')
      logger.info('[Transfer] WebRTC 信令服务地址已保存（后端读取确认）:', saved)
      webrtcSignalUrl.value = typeof saved === 'string' ? saved : url
    } catch (e) {
      logger.error('[Transfer] 读取 WebRTC 信令服务地址失败（无法确认是否写入配置文件）:', e)
    }
  } catch (error) {
    logger.error('保存 WebRTC 信令服务地址失败:', error)
  } finally {
    webrtcSignalSaving.value = false
  }
}
</script>
