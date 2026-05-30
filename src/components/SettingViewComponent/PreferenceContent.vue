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
            LSP 能力统一由下方语言扩展提供；安装对应扩展后，新建或重新打开编辑器即可获得语法高亮和语义补全。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="启用 LSP 扩展能力">
              <n-switch
                v-model:value="lspHighlightEnabled"
                :disabled="loading"
                @update:value="handleLspHighlightChange"
              />
            </n-form-item>
          </n-form>
          <LspExtensionPanel />
        </div>

        <n-divider />

        <div>
          <h3>TTS 配置</h3>
          <p class="description">
            配置朗读服务提供方与本地 sherpa-onnx 路径。设置保存到 guaxi_config.json，不再使用 localStorage。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="Provider">
              <n-select
                v-model:value="ttsProvider"
                :options="ttsProviderOptions"
                :disabled="loading || ttsSaving"
                style="width: 240px"
              />
            </n-form-item>
            <n-form-item label="Model 路径">
              <n-input
                v-model:value="ttsModel"
                placeholder="utils/Models/tts/kokoro_multi_lang/model.onnx"
                :disabled="loading || ttsSaving"
              />
            </n-form-item>
            <n-form-item v-if="ttsProvider === 'sherpa-onnx'" label="Tokens 路径">
              <n-input
                v-model:value="ttsTokens"
                placeholder="utils/Models/tts/kokoro_multi_lang/tokens.txt"
                :disabled="loading || ttsSaving"
              />
            </n-form-item>
            <n-form-item v-if="ttsProvider === 'sherpa-onnx'" label="Exe 路径">
              <n-input
                v-model:value="ttsExe"
                placeholder="utils/sherpa-onnx/sherpa-onnx-offline-tts.exe"
                :disabled="loading || ttsSaving"
              />
            </n-form-item>
            <n-form-item label="测试文本">
              <n-input
                v-model:value="ttsTestText"
                placeholder="输入一行用于试听的文本"
                :disabled="loading || ttsSaving || ttsTesting"
              />
            </n-form-item>
            <n-form-item>
              <n-space>
                <n-button
                  type="primary"
                  size="small"
                  :disabled="loading || ttsSaving"
                  @click="handleTtsConfigSave"
                >
                  保存
                </n-button>
                <n-button
                  size="small"
                  :disabled="loading || ttsSaving"
                  @click="handleTtsConfigReset"
                >
                  恢复默认
                </n-button>
                <n-button
                  secondary
                  size="small"
                  :loading="ttsTesting"
                  :disabled="loading || ttsSaving"
                  @click="handleTtsTestReadAloud"
                >
                  {{ ttsTesting ? '朗读中...' : '测试朗读' }}
                </n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>ASR 配置</h3>
          <p class="description">
            配置语音转文字提供方与本地 sherpa-onnx 路径。设置保存到 guaxi_config.json。
          </p>
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="Provider">
              <n-select
                v-model:value="asrProvider"
                :options="asrProviderOptions"
                :disabled="loading || asrSaving"
                style="width: 240px"
              />
            </n-form-item>
            <n-form-item label="Model 路径">
              <n-input
                v-model:value="asrModel"
                placeholder="可填模型目录或模型文件路径"
                :disabled="loading || asrSaving"
              />
            </n-form-item>
            <n-form-item v-if="asrProvider === 'sherpa-onnx'" label="Exe 路径">
              <n-input
                v-model:value="asrExe"
                placeholder="utils/sherpa-onnx/sherpa-onnx-offline.exe"
                :disabled="loading || asrSaving"
              />
            </n-form-item>
            <n-form-item label="测试音频路径">
              <n-input
                v-model:value="asrTestFilePath"
                placeholder="填写本地音频文件绝对路径（wav/mp3/m4a/flac）"
                :disabled="loading || asrSaving || asrTesting"
              />
            </n-form-item>
            <n-form-item label="转写结果">
              <n-input
                v-model:value="asrTestResult"
                type="textarea"
                :autosize="{ minRows: 3, maxRows: 6 }"
                placeholder="点击“测试转写”后显示结果"
                :disabled="true"
              />
            </n-form-item>
            <n-form-item>
              <n-space>
                <n-button
                  type="primary"
                  size="small"
                  :disabled="loading || asrSaving"
                  @click="handleAsrConfigSave"
                >
                  保存
                </n-button>
                <n-button
                  size="small"
                  :disabled="loading || asrSaving"
                  @click="handleAsrConfigReset"
                >
                  恢复默认
                </n-button>
                <n-button
                  secondary
                  size="small"
                  :loading="asrTesting"
                  :disabled="loading || asrSaving"
                  @click="handleAsrTestTranscribe"
                >
                  {{ asrTesting ? '转写中...' : '测试转写' }}
                </n-button>
              </n-space>
            </n-form-item>
          </n-form>
        </div>

        <n-divider />

        <div>
          <h3>终端配置</h3>
          <p class="description">
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
import { createLogger } from '../../utils/logger'
import { ref, onBeforeUnmount, onMounted, watch } from 'vue'
import { NCard, NSpace, NDivider, NForm, NFormItem, NInputNumber, NInput, NButton, NAlert, NSwitch, NSelect } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '../../stores/settingsStore'
import LspExtensionPanel from './LspExtensionPanel.vue'

const logger = createLogger('PreferenceContent')

const settingsStore = useSettingsStore()

const themeOptions = [
  { label: '亮色', value: 'light' },
  { label: '暗色', value: 'dark' },
]

const themeValue = ref('light')
const gcCycleHours = ref(1)
const lspHighlightEnabled = ref(false)
const webrtcSignalUrl = ref('')
const terminalDefaultProfile = ref('powershell')
const terminalGitBashPath = ref('')
const ttsProvider = ref('sherpa-onnx')
const ttsModel = ref('')
const ttsTokens = ref('')
const ttsExe = ref('')
const ttsTestText = ref('你好，这是测试朗读。')
const asrProvider = ref('sherpa-onnx')
const asrModel = ref('')
const asrExe = ref('')
const asrTestFilePath = ref('')
const asrTestResult = ref('')
const loading = ref(false)
const themeSaving = ref(false)
const webrtcSignalSaving = ref(false)
const ttsSaving = ref(false)
const ttsTesting = ref(false)
const asrSaving = ref(false)
const asrTesting = ref(false)
const terminalSaving = ref(false)
const ttsProviderOptions = [
  { label: 'sherpa-onnx', value: 'sherpa-onnx' },
  { label: 'siliconflow', value: 'siliconflow' },
]
const asrProviderOptions = [
  { label: 'sherpa-onnx', value: 'sherpa-onnx' },
  { label: 'siliconflow', value: 'siliconflow' },
]
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
let ttsTestAudioObjectUrl = ''
let ttsTestAudioElement = null

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
        const [seconds, enabled, signalUrl, ttsConfig, asrConfig, terminalConfig] = await Promise.all([
          invoke('get_gc_cycle_seconds'),
          invoke('get_lsp_syntax_highlight_enabled'),
          invoke('get_webrtc_signal_url'),
          invoke('get_tts_config'),
          invoke('get_asr_config'),
          invoke('get_terminal_config'),
        ])

        if (typeof seconds === 'number' && seconds > 0) {
          gcCycleHours.value = Math.max(1, Math.round(seconds / 3600))
        } else {
          gcCycleHours.value = 1
        }

        lspHighlightEnabled.value = !!enabled
        webrtcSignalUrl.value = typeof signalUrl === 'string' ? signalUrl : ''
        ttsProvider.value = String(ttsConfig?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
        ttsModel.value = String(ttsConfig?.model || '').trim()
        ttsTokens.value = String(ttsConfig?.tokens || '').trim()
        ttsExe.value = String(ttsConfig?.exe || '').trim()
        asrProvider.value = String(asrConfig?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
        asrModel.value = String(asrConfig?.model || '').trim()
        asrExe.value = String(asrConfig?.exe || '').trim()
        terminalDefaultProfile.value = String(terminalConfig?.default_profile || 'powershell').trim() || 'powershell'
        terminalGitBashPath.value = String(terminalConfig?.git_bash_path || '').trim()
      })(),
    ])

    themeValue.value = settingsStore.appearanceTheme
  } catch (error) {
    logger.error('加载偏好设置失败:', error)
    gcCycleHours.value = 1
    lspHighlightEnabled.value = false
    themeValue.value = 'light'
    ttsProvider.value = 'sherpa-onnx'
    ttsModel.value = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
    ttsTokens.value = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
    ttsExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
    asrProvider.value = 'sherpa-onnx'
    asrModel.value = ''
    asrExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline.exe'
    terminalDefaultProfile.value = 'powershell'
    terminalGitBashPath.value = ''
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

    logger.info('[Preference] 切换主题:', {
      next,
      current: settingsStore.appearanceTheme,
    })

    await settingsStore.setAppearanceTheme(next)

    try {
      const appearance = await invoke('get_appearance_config')
      logger.info('[Preference] 后端返回的 appearance（用于确认是否写入配置文件）:', appearance)
    } catch (e) {
      logger.error('[Preference] 获取后端 appearance 失败（无法确认是否写入配置文件）:', e)
    }

    logger.info('[Preference] 主题切换完成（已应用并已尝试写入配置文件）:', next)
  } catch (error) {
    logger.error('[Preference] 主题切换失败（可能未写入配置文件）:', error)
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
    logger.error('更新 GC 周期配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleLspHighlightChange = async (value) => {
  try {
    loading.value = true
    await invoke('set_lsp_syntax_highlight_enabled', { enabled: value })
    window.dispatchEvent(
      new CustomEvent('guaxi:lsp-setting-changed', {
        detail: { enabled: !!value }
      })
    )
  } catch (error) {
    logger.error('更新 LSP 语法高亮配置失败:', error)
  } finally {
    loading.value = false
  }
}

const handleWebrtcSignalUrlSave = async () => {
  try {
    webrtcSignalSaving.value = true
    const url = String(webrtcSignalUrl.value || '').trim()

    logger.info('[Preference] 保存 WebRTC 信令服务地址:', url)

    await invoke('set_webrtc_signal_url', { url })

    try {
      const saved = await invoke('get_webrtc_signal_url')
      logger.info('[Preference] WebRTC 信令服务地址已保存（后端读取确认）:', saved)
      webrtcSignalUrl.value = typeof saved === 'string' ? saved : url
    } catch (e) {
      logger.error('[Preference] 读取 WebRTC 信令服务地址失败（无法确认是否写入配置文件）:', e)
    }
  } catch (error) {
    logger.error('保存 WebRTC 信令服务地址失败:', error)
  } finally {
    webrtcSignalSaving.value = false
  }
}

const handleTerminalConfigSave = async () => {
  try {
    terminalSaving.value = true
    const payload = {
      default_profile: String(terminalDefaultProfile.value || '').trim() || 'powershell',
      git_bash_path: String(terminalGitBashPath.value || '').trim(),
    }
    logger.info('[Preference] 保存终端配置', payload)
    await invoke('set_terminal_config', {
      terminalConfig: payload,
    })
    const saved = await invoke('get_terminal_config')
    terminalDefaultProfile.value = String(saved?.default_profile || payload.default_profile).trim()
      || payload.default_profile
    terminalGitBashPath.value = String(saved?.git_bash_path || payload.git_bash_path).trim()
    logger.info('[Preference] 终端配置已保存', {
      defaultProfile: terminalDefaultProfile.value,
      gitBashPath: terminalGitBashPath.value,
    })
  } catch (error) {
    logger.error('[Preference] 保存终端配置失败:', error)
  } finally {
    terminalSaving.value = false
  }
}

const handleTtsConfigSave = async () => {
  try {
    ttsSaving.value = true
    const current = await invoke('get_tts_config')
    await invoke('set_tts_config', {
      ttsConfig: {
        provider: String(ttsProvider.value || '').trim(),
        model: String(ttsModel.value || '').trim(),
        tokens: String(ttsTokens.value || '').trim(),
        exe: String(ttsExe.value || '').trim(),
        output_dir: String(current?.output_dir || '').trim(),
      }
    })
    const saved = await invoke('get_tts_config')
    ttsProvider.value = String(saved?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    ttsModel.value = String(saved?.model || '').trim()
    ttsTokens.value = String(saved?.tokens || '').trim()
    ttsExe.value = String(saved?.exe || '').trim()
  } catch (error) {
    logger.error('保存 TTS 配置失败:', error)
  } finally {
    ttsSaving.value = false
  }
}

const handleTtsConfigReset = async () => {
  ttsProvider.value = 'sherpa-onnx'
  ttsModel.value = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
  ttsTokens.value = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
  ttsExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
  await handleTtsConfigSave()
}

const handleAsrConfigSave = async () => {
  try {
    asrSaving.value = true
    await invoke('set_asr_config', {
      asrConfig: {
        provider: String(asrProvider.value || '').trim(),
        model: String(asrModel.value || '').trim(),
        exe: String(asrExe.value || '').trim(),
      }
    })
    const saved = await invoke('get_asr_config')
    asrProvider.value = String(saved?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    asrModel.value = String(saved?.model || '').trim()
    asrExe.value = String(saved?.exe || '').trim()
    logger.info('保存 ASR 配置完成', {
      provider: asrProvider.value,
      model: asrModel.value,
      exe: asrExe.value,
    })
  } catch (error) {
    logger.error('保存 ASR 配置失败:', error)
  } finally {
    asrSaving.value = false
  }
}

const handleAsrConfigReset = async () => {
  asrProvider.value = 'sherpa-onnx'
  asrModel.value = ''
  asrExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline.exe'
  await handleAsrConfigSave()
}

const handleAsrTestTranscribe = async () => {
  const filePath = String(asrTestFilePath.value || '').trim()
  if (!filePath || asrTesting.value) return
  try {
    asrTesting.value = true
    asrTestResult.value = ''
    const provider = String(asrProvider.value || '').trim() || 'sherpa-onnx'
    const model = String(asrModel.value || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(asrExe.value || '').trim() : ''
    logger.info('开始测试 ASR 转写', {
      provider,
      model,
      endpoint,
      filePath,
    })
    const response = await invoke('audio_speech_to_text', {
      request: {
        common: {
          provider,
          model,
          endpoint,
        },
        file_path: filePath,
        file_name: filePath.split(/[\\/]/).pop() || 'audio.wav',
        mime_type: 'application/octet-stream',
      }
    })
    asrTestResult.value = String(response?.text || '').trim()
    logger.info('ASR 测试完成', {
      textLength: asrTestResult.value.length,
      traceId: String(response?.trace_id || ''),
      model: String(response?.model || ''),
    })
  } catch (error) {
    logger.error('测试 ASR 转写失败:', error)
    asrTestResult.value = ''
  } finally {
    asrTesting.value = false
  }
}

const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

const stopTtsTestAudioPlayback = () => {
  if (ttsTestAudioElement) {
    ttsTestAudioElement.pause()
    ttsTestAudioElement.src = ''
    ttsTestAudioElement.onended = null
    ttsTestAudioElement = null
  }
  if (ttsTestAudioObjectUrl) {
    URL.revokeObjectURL(ttsTestAudioObjectUrl)
    ttsTestAudioObjectUrl = ''
  }
}

const handleTtsTestReadAloud = async () => {
  const text = String(ttsTestText.value || '').trim()
  if (!text || ttsTesting.value) return
  try {
    ttsTesting.value = true
    const provider = String(ttsProvider.value || '').trim() || 'sherpa-onnx'
    const model = String(ttsModel.value || '').trim()
    const endpoint = provider === 'sherpa-onnx' ? String(ttsExe.value || '').trim() : ''
    const tokens = provider === 'sherpa-onnx' ? String(ttsTokens.value || '').trim() : ''
    const responseFormat = provider === 'sherpa-onnx' ? 'wav' : 'mp3'
    logger.info('[PreferenceContent] 测试朗读配置', {
      provider,
      model,
      endpoint,
      responseFormat,
      isSherpaOnnx: provider === 'sherpa-onnx',
    })
    const response = await invoke('audio_text_to_speech', {
      request: {
        common: {
          provider,
          model,
          endpoint,
          tokens,
        },
        input: text,
        voice: provider === 'sherpa-onnx' ? '0' : 'alex',
        response_format: responseFormat,
        stream: false,
      },
    })
    const audioBase64 = String(response?.audio_base64 || '')
    if (!audioBase64) {
      throw new Error('语音数据为空')
    }
    const contentType = String(response?.content_type || (responseFormat === 'wav' ? 'audio/wav' : 'audio/mpeg'))
    const bytes = base64ToUint8Array(audioBase64)
    const blob = new Blob([bytes], { type: contentType })
    stopTtsTestAudioPlayback()
    ttsTestAudioObjectUrl = URL.createObjectURL(blob)
    // 设计说明：release 版 WebView 会严格执行 CSP；这里记录 Blob 音频元信息，
    // 便于区分“后端合成异常”和“前端 media-src/解码层拒绝加载”。
    logger.info('[PreferenceContent] 测试朗读音频已准备', {
      contentType,
      audioBytes: bytes.byteLength,
      responseFormat,
      objectUrlProtocol: ttsTestAudioObjectUrl.split(':')[0],
    })
    const audio = new Audio(ttsTestAudioObjectUrl)
    ttsTestAudioElement = audio
    const waitForPlaybackEnd = new Promise((resolve, reject) => {
      audio.onended = () => {
        stopTtsTestAudioPlayback()
        resolve()
      }
      audio.onerror = () => {
        const mediaError = audio.error
        logger.error('[PreferenceContent] 测试朗读音频元素加载失败', {
          contentType,
          audioBytes: bytes.byteLength,
          mediaErrorCode: mediaError?.code || 0,
          mediaErrorMessage: mediaError?.message || '',
          networkState: audio.networkState,
          readyState: audio.readyState,
          currentSrcProtocol: String(audio.currentSrc || '').split(':')[0],
        })
        stopTtsTestAudioPlayback()
        reject(new Error('音频播放失败'))
      }
    })
    logger.info('[PreferenceContent] 开始播放测试朗读音频', {
      contentType,
      audioBytes: bytes.byteLength,
    })
    await audio.play()
    await waitForPlaybackEnd
  } catch (error) {
    logger.error('测试朗读失败:', error)
  } finally {
    ttsTesting.value = false
  }
}

onBeforeUnmount(() => {
  stopTtsTestAudioPlayback()
})

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

.lsp-status-panel {
  margin-top: 12px;
  padding: 12px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-panel-bg);
}

.lsp-status-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.lsp-status-title {
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.lsp-status-loading,
.lsp-status-empty {
  color: var(--guaxi-text-secondary);
  font-size: 13px;
}

.lsp-status-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.lsp-status-item {
  padding: 10px;
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  background: var(--guaxi-content-bg);
}

.lsp-status-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.lsp-status-name {
  font-weight: 500;
  color: var(--guaxi-text-primary);
}

.lsp-status-chip {
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 12px;
  background: var(--guaxi-border);
  color: var(--guaxi-text-secondary);
}

.lsp-status-chip.ok {
  background: rgba(25, 113, 194, 0.12);
  color: #2b6cb0;
}

.lsp-status-chip.warn {
  background: rgba(245, 159, 0, 0.15);
  color: #c77d00;
}

.lsp-status-chip.info {
  background: rgba(132, 94, 194, 0.15);
  color: #6b46c1;
}

.lsp-status-chip.loading {
  background: rgba(0, 0, 0, 0.08);
  color: var(--guaxi-text-secondary);
}

.lsp-status-path {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  word-break: break-all;
}

.lsp-status-message {
  margin-top: 4px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
}

.lsp-status-actions {
  margin-top: 6px;
}

.tip {
  margin-top: 8px;
}
</style>
