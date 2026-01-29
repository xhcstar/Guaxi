<template>
  <div class="preference-wrapper">
    <n-card title="外观与系统">
      <n-space vertical size="large" style="width: 100%">
        <div>
          <h3>外观</h3>
          <p class="description">在此切换应用主题, 暗色或者亮色。</p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="主题">
              <n-select
                v-model:value="themeValue"
                :options="themeOptions"
                :disabled="loading || themeSaving"
                style="width: 240px"
                @update:value="handleThemeChange"
              />
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>垃圾回收配置</h3>
          <p class="description">
            后台会定期清理已删除的数据和临时冲突文件，以减少空间占用并保持库的整洁。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="GC 周期（小时）">
              <n-input-number
                v-model:value="gcCycleHours"
                :min="1"
                :max="72"
                :step="1"
                :precision="0"
                :disabled="loading"
                style="width: 200px"
                @update:value="handleGcCycleChange"
              />
            </n-form-item>
          </n-form>
          <n-alert type="info" :show-icon="true" class="tip">
            修改后将在下一次周期生效，最低间隔为 1 小时。若未配置则默认每 1 小时执行一次垃圾回收。
          </n-alert>
        </div>

        <n-divider />

        <div>
          <h3>LSP 配置</h3>
          <p class="description">
            关闭后将不会初始化 LSP 语法高亮和代码智能，重新打开后新建编辑器才会生效。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="启用 LSP 语法高亮">
              <n-switch
                v-model:value="lspHighlightEnabled"
                :disabled="loading"
                @update:value="handleLspHighlightChange"
              />
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>WebRTC 文件传输</h3>
          <p class="description">
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
        </div>

      </n-space>
    </n-card>
  </div>
</template>

<script setup>
import { ref, onMounted, watch } from 'vue'
import { NCard, NSpace, NDivider, NForm, NFormItem, NInputNumber, NInput, NButton, NAlert, NSwitch, NSelect } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()

const themeOptions = [
  { label: '亮色', value: 'light' },
  { label: '暗色', value: 'dark' },
]

const themeValue = ref('light')
const gcCycleHours = ref(1)
const lspHighlightEnabled = ref(false)
const webrtcSignalUrl = ref('')
const loading = ref(false)
const themeSaving = ref(false)
const webrtcSignalSaving = ref(false)

watch(
  () => settingsStore.appearanceTheme,
  (v) => {
    themeValue.value = v === 'dark' ? 'dark' : 'light'
  },
  { immediate: true }
)

onMounted(async () => {
  try {
    loading.value = true
    await Promise.all([
      settingsStore.loadAppearanceConfig(),
      (async () => {
        const [seconds, enabled, signalUrl] = await Promise.all([
          invoke('get_gc_cycle_seconds'),
          invoke('get_lsp_syntax_highlight_enabled'),
          invoke('get_webrtc_signal_url')
        ])

        if (typeof seconds === 'number' && seconds > 0) {
          gcCycleHours.value = Math.max(1, Math.round(seconds / 3600))
        } else {
          gcCycleHours.value = 1
        }

        lspHighlightEnabled.value = !!enabled
        webrtcSignalUrl.value = typeof signalUrl === 'string' ? signalUrl : ''
      })(),
    ])

    themeValue.value = settingsStore.appearanceTheme
  } catch (error) {
    console.error('加载偏好设置失败:', error)
    gcCycleHours.value = 1
    lspHighlightEnabled.value = false
    themeValue.value = 'light'
  } finally {
    loading.value = false
  }
})

const handleThemeChange = async (value) => {
  if (loading.value || themeSaving.value) return

  const next = value === 'dark' ? 'dark' : 'light'
  themeValue.value = next

  try {
    themeSaving.value = true

    console.log('[Preference] 切换主题:', {
      next,
      current: settingsStore.appearanceTheme,
    })

    await settingsStore.setAppearanceTheme(next)

    try {
      const appearance = await invoke('get_appearance_config')
      console.log('[Preference] 后端返回的 appearance（用于确认是否写入配置文件）:', appearance)
    } catch (e) {
      console.error('[Preference] 获取后端 appearance 失败（无法确认是否写入配置文件）:', e)
    }

    console.log('[Preference] 主题切换完成（已应用并已尝试写入配置文件）:', next)
  } catch (error) {
    console.error('[Preference] 主题切换失败（可能未写入配置文件）:', error)
  } finally {
    themeSaving.value = false
  }
}

const handleGcCycleChange = async (value) => {
  if (!value || value < 1) {
    gcCycleHours.value = 1
  }
  try {
    loading.value = true
    const seconds = gcCycleHours.value * 3600
    await invoke('set_gc_cycle_seconds', { seconds })
  } catch (error) {
    console.error('更新 GC 周期配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleLspHighlightChange = async (value) => {
  try {
    loading.value = true
    await invoke('set_lsp_syntax_highlight_enabled', { enabled: value })
  } catch (error) {
    console.error('更新 LSP 语法高亮配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleWebrtcSignalUrlSave = async () => {
  try {
    webrtcSignalSaving.value = true
    const url = String(webrtcSignalUrl.value || '').trim()

    console.log('[Preference] 保存 WebRTC 信令服务地址:', url)

    await invoke('set_webrtc_signal_url', { url })

    try {
      const saved = await invoke('get_webrtc_signal_url')
      console.log('[Preference] WebRTC 信令服务地址已保存（后端读取确认）:', saved)
      webrtcSignalUrl.value = typeof saved === 'string' ? saved : url
    } catch (e) {
      console.error('[Preference] 读取 WebRTC 信令服务地址失败（无法确认是否写入配置文件）:', e)
    }
  } catch (error) {
    console.error('保存 WebRTC 信令服务地址失败:', error)
  } finally {
    webrtcSignalSaving.value = false
  }
}

</script>

<style scoped>
.preference-wrapper {
  width: 100%;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
}

.description {
  margin-top: 4px;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

.tip {
  margin-top: 8px;
}
</style>