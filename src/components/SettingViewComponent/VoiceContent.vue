<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="语音能力">
      <div class="voice-block">
        <h3>朗读声音</h3>
        <p class="setting-page-desc">
          划词朗读、逐句朗读、PDF 朗读与有声书默认都使用这里的音色与语速；设置保存到 guaxi_config.json。
        </p>
        <n-form label-placement="left" label-width="auto" :show-require-mark="false">
          <template v-if="ttsProvider === 'sherpa-onnx'">
            <n-form-item label="语言">
              <n-select
                v-model:value="ttsLanguage"
                :options="ttsLanguageOptions"
                :disabled="loading || ttsSaving"
                style="width: 240px"
              />
            </n-form-item>
            <n-form-item label="性别">
              <n-select
                v-model:value="ttsGender"
                :options="ttsGenderOptions"
                :disabled="loading || ttsSaving"
                style="width: 240px"
              />
            </n-form-item>
            <n-form-item label="音色">
              <n-space vertical :size="4">
                <n-select
                  v-model:value="ttsSpeakerId"
                  :options="ttsSpeakerOptions"
                  filterable
                  tag
                  placeholder="选择朗读音色"
                  :disabled="loading || ttsSaving"
                  style="width: 320px"
                />
                <span class="setting-page-desc">{{ ttsSpeakerHint }}</span>
                <span v-if="!ttsSpeakerOptions.length" class="setting-page-desc">
                  当前语言/性别组合下没有可用音色（Kokoro 英文音色只有女声），换一个组合试试。
                </span>
              </n-space>
            </n-form-item>
          </template>
          <n-form-item label="语速">
            <n-space align="center" :size="12">
              <n-slider
                v-model:value="ttsSpeed"
                :min="0.5"
                :max="2"
                :step="0.05"
                :disabled="loading || ttsSaving"
                style="width: 220px"
              />
              <span class="setting-page-desc">{{ Number(ttsSpeed).toFixed(2) }}×</span>
              <n-button size="tiny" quaternary :disabled="loading || ttsSaving" @click="ttsSpeed = 1">
                原速
              </n-button>
            </n-space>
          </n-form-item>
          <n-form-item label="试听文本">
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
          <n-collapse>
            <n-collapse-item title="高级：朗读服务与模型路径" name="advanced">
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
            </n-collapse-item>
          </n-collapse>
        </n-form>
      </div>

      <n-divider />

      <div class="voice-block">
        <h3>ASR 语音转写配置</h3>
        <p class="setting-page-desc">
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
          <n-form-item label="测试音频路径">
            <n-input
              v-model:value="asrTestFilePath"
              placeholder="填写本地音频文件绝对路径（仅支持 wav）"
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
    </n-card>
  </div>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { buildSpeakerOptions, describeSpeaker, speakerMeta } from '../../utils/ttsSpeakers'
import { NCard, NSpace, NDivider, NForm, NFormItem, NInput, NButton, NSelect, NSlider, NCollapse, NCollapseItem } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const logger = createLogger('VoiceContent')

const ttsProvider = ref('sherpa-onnx')
const ttsModel = ref('')
const ttsTokens = ref('')
const ttsExe = ref('')
/// 默认音色（Speaker ID）。各朗读入口在未显式指定时都会用它，
/// 所以这里是"全局音色"的唯一入口。3 号 = zf_001（中文女声）。
const ttsSpeakerId = ref('3')
/// 全局朗读语速：0.5–2.0，1.0 为原速。划词/逐句朗读没显式传语速时由后端取它。
const ttsSpeed = ref(1)
/// 设置页的筛选条件：只影响下拉列表，不单独保存；音色本身仍以 speaker id 为准。
const ttsLanguage = ref('auto')
const ttsGender = ref('auto')
const ttsLanguageOptions = [
  { label: '不限', value: 'auto' },
  { label: '中文', value: 'zh' },
  { label: '英文', value: 'en' },
]
const ttsGenderOptions = [
  { label: '不限', value: 'auto' },
  { label: '女声', value: 'female' },
  { label: '男声', value: 'male' },
]
const ttsSpeakerOptions = computed(() =>
  buildSpeakerOptions(ttsModel.value, { language: ttsLanguage.value, gender: ttsGender.value })
)
const ttsSpeakerHint = computed(() => describeSpeaker(ttsModel.value, ttsSpeakerId.value))

const speakerValuesOf = (options) =>
  (Array.isArray(options) ? options : []).flatMap((option) =>
    option?.type === 'group' ? (option.children || []).map((child) => child.value) : [option?.value]
  )

/** 当前音色不在筛选结果里时，自动落到第一个匹配项，避免下拉显示空值。 */
watch([ttsLanguage, ttsGender], () => {
  const values = speakerValuesOf(ttsSpeakerOptions.value)
  if (!values.length) return
  if (!values.includes(String(ttsSpeakerId.value ?? ''))) {
    ttsSpeakerId.value = values[0]
  }
})

/** 从已保存的 speaker id 反推语言/性别筛选，打开设置页时展示对应的那一组。 */
const syncSpeakerFiltersFromId = () => {
  const meta = speakerMeta(ttsModel.value, ttsSpeakerId.value)
  ttsLanguage.value = meta.language || 'auto'
  ttsGender.value = meta.gender || 'auto'
}

// 直接在下拉里换了一个音色时，筛选条件跟着它走（否则会出现「筛选是中文、
// 选中的却是英文音色」的错位）。筛选 watch 可能回写 id，两边的终态一致，不会循环。
watch(ttsSpeakerId, () => {
  const meta = speakerMeta(ttsModel.value, ttsSpeakerId.value)
  if (meta.language && meta.language !== ttsLanguage.value) ttsLanguage.value = meta.language
  if (meta.gender && meta.gender !== ttsGender.value) ttsGender.value = meta.gender
})
const ttsTestText = ref('你好，这是测试朗读。')
const asrProvider = ref('sherpa-onnx')
const asrModel = ref('')
const asrExe = ref('')
const asrTestFilePath = ref('')
const asrTestResult = ref('')
const loading = ref(false)
const ttsSaving = ref(false)
const ttsTesting = ref(false)
const asrSaving = ref(false)
const asrTesting = ref(false)
const ttsProviderOptions = [
  { label: 'sherpa-onnx', value: 'sherpa-onnx' },
  { label: 'siliconflow', value: 'siliconflow' },
]
const asrProviderOptions = [
  { label: 'sherpa-onnx', value: 'sherpa-onnx' },
  { label: 'siliconflow', value: 'siliconflow' },
]
let ttsTestAudioObjectUrl = ''
let ttsTestAudioElement = null

onMounted(async () => {
  try {
    loading.value = true
    const [ttsConfig, asrConfig] = await Promise.all([
      invoke('get_tts_config'),
      invoke('get_asr_config'),
    ])

    ttsProvider.value = String(ttsConfig?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    ttsModel.value = String(ttsConfig?.model || '').trim()
    ttsTokens.value = String(ttsConfig?.tokens || '').trim()
    ttsExe.value = String(ttsConfig?.exe || '').trim()
    ttsSpeakerId.value = String(ttsConfig?.speaker_id || '3').trim() || '3'
    ttsSpeed.value = Number(ttsConfig?.speed) > 0 ? Number(ttsConfig.speed) : 1
    syncSpeakerFiltersFromId()
    asrProvider.value = String(asrConfig?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    asrModel.value = String(asrConfig?.model || '').trim()
    asrExe.value = String(asrConfig?.exe || '').trim()
  } catch (error) {
    logger.error('加载语音设置失败:', error)
    ttsProvider.value = 'sherpa-onnx'
    ttsModel.value = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
    ttsTokens.value = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
    ttsExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
    asrProvider.value = 'sherpa-onnx'
    asrModel.value = 'utils/Models/asr/sense-voice'
    asrExe.value = 'utils/sherpa-onnx/sherpa-onnx-offline.exe'
  } finally {
    loading.value = false
  }
})

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
        speaker_id: String(ttsSpeakerId.value || '3').trim() || '3',
        speed: Number(ttsSpeed.value) || 1,
        output_dir: String(current?.output_dir || '').trim(),
      }
    })
    const saved = await invoke('get_tts_config')
    ttsProvider.value = String(saved?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    ttsModel.value = String(saved?.model || '').trim()
    ttsTokens.value = String(saved?.tokens || '').trim()
    ttsExe.value = String(saved?.exe || '').trim()
    ttsSpeakerId.value = String(saved?.speaker_id || '3').trim() || '3'
    ttsSpeed.value = Number(saved?.speed) > 0 ? Number(saved.speed) : 1
    syncSpeakerFiltersFromId()
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
  ttsSpeakerId.value = '3'
  ttsSpeed.value = 1
  syncSpeakerFiltersFromId()
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
  asrModel.value = 'utils/Models/asr/sense-voice'
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
    logger.info('[VoiceContent] 测试朗读配置', {
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
        // 试听直接用当前选中的音色与语速，这样不用先保存就能比较。
        voice: provider === 'sherpa-onnx' ? String(ttsSpeakerId.value || '3').trim() : 'alex',
        speed: Number(ttsSpeed.value) || 1,
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
    logger.info('[VoiceContent] 测试朗读音频已准备', {
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
        logger.error('[VoiceContent] 测试朗读音频元素加载失败', {
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
    logger.info('[VoiceContent] 开始播放测试朗读音频', {
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
