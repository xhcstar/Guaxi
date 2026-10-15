import { onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('useSelectionReadAloud')

/**
 * 划词朗读（朗读选中的那一段）。
 *
 * 走现成的一次性合成接口 `audio_text_to_speech`，和编辑器的划词朗读同一套——
 * 一小段文本不值得开一个朗读会话。它用的是**独立的引擎实例**（见
 * `TtsEngineConfig::instance_tag`），所以与逐句朗读互不阻塞。
 *
 * 与编辑器唯一的差异：播放中再点一次即停止。编辑器那边播放中按钮是禁用的，
 * 但阅读器里一段选文可能很长、浮动条又随时会消失，没有停止入口会很难受。
 *
 * @param {{ visible?: import('vue').Ref<boolean>|null, message?: { warning: Function, error: Function }|null }} [options]
 */
export function useSelectionReadAloud({ visible = null, message = null } = {}) {
  const busy = ref(false)
  let audio = null
  let objectUrl = ''
  /** 单调令牌：停止/新一轮朗读都会让它自增，用来丢弃在途请求的结果。 */
  let token = 0

  const stop = () => {
    token += 1
    if (audio) {
      try {
        audio.pause()
        audio.onended = null
        audio.onerror = null
        audio.src = ''
      } catch {
        /* noop */
      }
      audio = null
    }
    if (objectUrl) {
      try {
        URL.revokeObjectURL(objectUrl)
      } catch {
        /* noop */
      }
      objectUrl = ''
    }
    busy.value = false
  }

  const speak = async (rawText) => {
    const text = String(rawText || '').trim()
    if (!text) {
      message?.warning?.('请先框选要朗读的文本')
      return
    }
    if (busy.value) return
    busy.value = true
    const myToken = ++token
    try {
      const tts = await invoke('get_tts_config')
      const provider = String(tts?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
      const model =
        String(tts?.model || '').trim() ||
        (provider === 'sherpa-onnx'
          ? 'utils/Models/tts/kokoro_multi_lang/model.onnx'
          : 'FunAudioLLM/CosyVoice2-0.5B')
      const tokens = String(tts?.tokens || '').trim()
      const endpoint = provider === 'sherpa-onnx' ? String(tts?.exe || '').trim() : ''
      const responseFormat = provider === 'sherpa-onnx' ? 'wav' : 'mp3'
      const response = await invoke('audio_text_to_speech', {
        request: {
          common: { provider, model, endpoint, tokens },
          input: text,
          // 留空即由后端回落到设置里配的默认音色（不要在这里硬编码编号，
          // 否则用户在设置页选的音色在划词朗读处会被忽略）。
          voice: provider === 'sherpa-onnx' ? '' : 'alex',
          response_format: responseFormat,
          stream: false,
        },
      })
      if (myToken !== token) return

      const base64 = String(response?.audio_base64 || '')
      if (!base64) throw new Error('语音数据为空')
      const binary = atob(base64)
      const bytes = new Uint8Array(binary.length)
      for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
      objectUrl = URL.createObjectURL(
        new Blob([bytes], { type: String(response?.content_type || 'audio/wav') })
      )

      const player = new Audio(objectUrl)
      audio = player
      const finished = new Promise((resolve, reject) => {
        player.onended = () => {
          if (myToken === token) stop()
          resolve()
        }
        player.onerror = () => {
          if (myToken === token) stop()
          reject(new Error('音频播放失败'))
        }
      })
      await player.play()
      await finished
    } catch (e) {
      if (myToken === token) stop()
      logger.error('划词朗读失败', e)
      message?.error?.(`朗读失败：${String(e?.message || e || '')}`)
    }
  }

  /** 播放中再点即停。 */
  const toggle = (text) => (busy.value ? stop() : speak(text))

  // 浮动条一消失就停止：否则工具栏关了、声音还在响，而用户已经没有停止入口。
  if (visible) {
    watch(visible, (value) => {
      if (!value) stop()
    })
  }
  onBeforeUnmount(stop)

  return { busy, speak, stop, toggle }
}
