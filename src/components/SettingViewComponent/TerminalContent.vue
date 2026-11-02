<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="终端">
      <p class="setting-page-desc">
        配置全局终端面板的默认终端类型，以及 Windows 下 Git Bash 的可执行文件路径。
      </p>
      <n-form label-placement="left" label-width="auto" :show-require-mark="false">
        <n-form-item label="默认终端">
          <n-select
            v-model:value="terminalDefaultProfile"
            :options="terminalProfileOptions"
            :disabled="loading || terminalSaving"
            style="width: 240px"
          />
        </n-form-item>
        <n-form-item label="Git Bash 路径">
          <n-input
            v-model:value="terminalGitBashPath"
            placeholder="例如：D:\\Git\\bin\\bash.exe -i -l"
            :disabled="loading || terminalSaving"
          />
        </n-form-item>
        <n-form-item>
          <n-button
            type="primary"
            size="small"
            :disabled="loading || terminalSaving"
            @click="handleTerminalConfigSave"
          >
            保存终端配置
          </n-button>
        </n-form-item>
      </n-form>
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, onMounted } from 'vue'
import { NCard, NForm, NFormItem, NInput, NButton, NSelect } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('TerminalContent')

const loading = ref(false)
const terminalSaving = ref(false)
const terminalDefaultProfile = ref('powershell')
const terminalGitBashPath = ref('')
const terminalProfileOptions = [
  { label: 'IPython', value: 'ipython' },
  { label: 'PowerShell', value: 'powershell' },
  { label: 'Git Bash', value: 'git-bash' },
  { label: 'CMD', value: 'cmd' },
  ...(typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
    ? []
    : [
        { label: 'Bash', value: 'bash' },
        { label: 'Zsh', value: 'zsh' },
      ]),
]

onMounted(async () => {
  try {
    loading.value = true
    const terminalConfig = await invoke('get_terminal_config')
    terminalDefaultProfile.value = String(terminalConfig?.default_profile || 'powershell').trim() || 'powershell'
    terminalGitBashPath.value = String(terminalConfig?.git_bash_path || '').trim()
  } catch (error) {
    logger.error('加载终端设置失败:', error)
    terminalDefaultProfile.value = 'powershell'
    terminalGitBashPath.value = ''
  } finally {
    loading.value = false
  }
})

const handleTerminalConfigSave = async () => {
  try {
    terminalSaving.value = true
    const payload = {
      default_profile: String(terminalDefaultProfile.value || '').trim() || 'powershell',
      git_bash_path: String(terminalGitBashPath.value || '').trim(),
    }
    logger.info('[Terminal] 保存终端配置', payload)
    await invoke('set_terminal_config', {
      terminalConfig: payload,
    })
    const saved = await invoke('get_terminal_config')
    terminalDefaultProfile.value = String(saved?.default_profile || payload.default_profile).trim()
      || payload.default_profile
    terminalGitBashPath.value = String(saved?.git_bash_path || payload.git_bash_path).trim()
    logger.info('[Terminal] 终端配置已保存', {
      defaultProfile: terminalDefaultProfile.value,
      gitBashPath: terminalGitBashPath.value,
    })
  } catch (error) {
    logger.error('[Terminal] 保存终端配置失败:', error)
  } finally {
    terminalSaving.value = false
  }
}
</script>
