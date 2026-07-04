import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('useSpeechDictation')

const DICTATION_WORKLET_PROCESSOR_NAME = 'guaxi-dictation-processor'
const DICTATION_WORKLET_SOURCE = `
class GuaxiDictationProcessor extends AudioWorkletProcessor {
  process(inputs) {
    const input = inputs[0]
    const channel = input && input[0]
    if (channel && channel.length) {
      this.port.postMessage(channel)
    }
    return true
  }
}

registerProcessor('${DICTATION_WORKLET_PROCESSOR_NAME}', GuaxiDictationProcessor)
`

const TARGET_SAMPLE_RATE = 16000 // sherpa-onnx / 多数 ASR 期望 16k 单声道

/**
 * 实时语音听写：麦克风采集 → VAD 静音分句 → 16k 单声道 WAV → audio_speech_to_text。
 * 跟随设置页的 ASR 配置（get_asr_config）选择 provider/model。
 *
 * @param {(text: string) => void} onTranscript 每转写出一句调用一次，把文本插入光标处。
 */
export function useSpeechDictation(onTranscript) {
  const recording = ref(false)
  // 'idle' | 'listening' | 'speaking' | 'transcribing' | 'error'
  const status = ref('idle')
  const errorText = ref('')
  const devices = ref([]) // [{ deviceId, label }]
  const selectedDeviceId = ref('')
  // VAD 灵敏度：阈值越低越灵敏（更容易判定为"在说话"）
  const silenceThreshold = ref(0.012)
  const pendingCount = ref(0) // 正在转写的分句数

  let audioContext = null
  let mediaStream = null
  let sourceNode = null
  let processorNode = null
  let muteNode = null

  // 采集缓冲（源采样率下的 Float32 PCM 累积）
  let captureBuffers = []
  let captureLength = 0
  let sourceSampleRate = TARGET_SAMPLE_RATE

  // VAD 状态
  let speaking = false
  let silenceMs = 0
  let voicedMs = 0
  const SILENCE_HANG_MS = 520 // 静音超过此值判定一句结束，短暂停顿即可尽快输出
  const MIN_UTTERANCE_MS = 300 // 太短的片段忽略（避免噪声触发）
  const STREAMING_CHUNK_MS = 5200 // 连续讲话时也定时切片，避免一直等到停止录音

  // 转写请求是异步的，短片段连续送 ASR 后可能后发先至。
  // 这里用序号缓存结果，只按采集顺序回调到编辑器，保证会议记录不会前后颠倒。
  let transcriptSeq = 0
  let nextEmitSeq = 1
  let activeRunId = 0
  const transcriptQueue = new Map()

  const refreshDevices = async () => {
    try {
      // label 仅在已授权后可见，先尝试枚举
      const list = await navigator.mediaDevices.enumerateDevices()
      const inputs = list
        .filter((d) => d.kind === 'audioinput')
        .map((d, i) => ({
          deviceId: d.deviceId,
          label: d.label || `麦克风 ${i + 1}`,
        }))
      devices.value = inputs
      if (!selectedDeviceId.value && inputs.length) {
        selectedDeviceId.value = inputs[0].deviceId
      }
      return inputs
    } catch (e) {
      logger.warn('枚举音频设备失败', { error: String(e?.message || e) })
      return []
    }
  }

  // 把任意采样率的 Float32 PCM 线性重采样到 16k
  const resampleTo16k = (input, inputRate) => {
    if (inputRate === TARGET_SAMPLE_RATE) return input
    const ratio = inputRate / TARGET_SAMPLE_RATE
    const outLength = Math.floor(input.length / ratio)
    const out = new Float32Array(outLength)
    for (let i = 0; i < outLength; i += 1) {
      const srcPos = i * ratio
      const i0 = Math.floor(srcPos)
      const i1 = Math.min(i0 + 1, input.length - 1)
      const frac = srcPos - i0
      out[i] = input[i0] * (1 - frac) + input[i1] * frac
    }
    return out
  }

  // Float32 [-1,1] PCM → 16-bit WAV(单声道) ArrayBuffer
  const encodeWav16kMono = (float32) => {
    const pcm16 = new Int16Array(float32.length)
    for (let i = 0; i < float32.length; i += 1) {
      let s = Math.max(-1, Math.min(1, float32[i]))
      pcm16[i] = s < 0 ? s * 0x8000 : s * 0x7fff
    }
    const dataBytes = pcm16.length * 2
    const buffer = new ArrayBuffer(44 + dataBytes)
    const view = new DataView(buffer)
    const writeStr = (off, str) => {
      for (let i = 0; i < str.length; i += 1) view.setUint8(off + i, str.charCodeAt(i))
    }
    writeStr(0, 'RIFF')
    view.setUint32(4, 36 + dataBytes, true)
    writeStr(8, 'WAVE')
    writeStr(12, 'fmt ')
    view.setUint32(16, 16, true) // PCM chunk size
    view.setUint16(20, 1, true) // PCM format
    view.setUint16(22, 1, true) // mono
    view.setUint32(24, TARGET_SAMPLE_RATE, true)
    view.setUint32(28, TARGET_SAMPLE_RATE * 2, true) // byte rate
    view.setUint16(32, 2, true) // block align
    view.setUint16(34, 16, true) // bits per sample
    writeStr(36, 'data')
    view.setUint32(40, dataBytes, true)
    let off = 44
    for (let i = 0; i < pcm16.length; i += 1, off += 2) view.setInt16(off, pcm16[i], true)
    return buffer
  }

  const arrayBufferToBase64 = (buffer) => {
    const bytes = new Uint8Array(buffer)
    let binary = ''
    const chunk = 0x8000
    for (let i = 0; i < bytes.length; i += chunk) {
      binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk))
    }
    return window.btoa(binary)
  }

  const resolveAsrRequestCommon = async () => {
    const cfg = await invoke('get_asr_config')
    const provider = String(cfg?.provider || 'sherpa-onnx').trim() || 'sherpa-onnx'
    const model = String(cfg?.model || '').trim()
    // sherpa-onnx：endpoint 字段被后端用作 exe 路径
    const endpoint = provider === 'sherpa-onnx' ? String(cfg?.exe || '').trim() : ''
    return { provider, model, endpoint }
  }

  // 设计思路：
  // 1. 部分 ASR 程序会把结构化识别结果直接写入 text 字段，形如一个 JSON 对象，甚至多个 JSON 对象连续输出。
  // 2. 编辑器听写只需要最终可读文本，因此这里统一提取结构化结果中的 text 字段。
  // 3. 如果返回本来就是普通文本，保持原样；这样兼容其他 provider，也避免把用户正常说出的花括号内容误删。
  const joinTranscriptPieces = (pieces) => {
    return pieces.reduce((acc, piece) => {
      const current = String(piece || '').trim()
      if (!current) return acc
      if (!acc) return current
      const prevLast = acc.slice(-1)
      const nextFirst = current[0]
      const shouldSeparate = /[A-Za-z0-9]$/.test(prevLast) && /^[A-Za-z0-9]/.test(nextFirst)
      return `${acc}${shouldSeparate ? ' ' : ''}${current}`
    }, '')
  }

  const extractJsonObjectTexts = (rawText) => {
    const source = String(rawText || '').trim()
    if (!source.includes('{') || !source.includes('}')) return []

    const objects = []
    let start = -1
    let depth = 0
    let inString = false
    let escaped = false

    for (let i = 0; i < source.length; i += 1) {
      const ch = source[i]
      if (inString) {
        if (escaped) {
          escaped = false
        } else if (ch === '\\') {
          escaped = true
        } else if (ch === '"') {
          inString = false
        }
        continue
      }

      if (ch === '"') {
        inString = true
        continue
      }
      if (ch === '{') {
        if (depth === 0) start = i
        depth += 1
        continue
      }
      if (ch === '}' && depth > 0) {
        depth -= 1
        if (depth === 0 && start >= 0) {
          objects.push(source.slice(start, i + 1))
          start = -1
        }
      }
    }

    return objects
      .map((item) => {
        try {
          const parsed = JSON.parse(item)
          return typeof parsed?.text === 'string' ? parsed.text : ''
        } catch (e) {
          logger.warn('ASR 结构化片段解析失败', {
            error: String(e?.message || e),
            fragmentLength: item.length,
          })
          return ''
        }
      })
      .filter((text) => String(text || '').trim())
  }

  const normalizeAsrTranscriptText = (payload) => {
    if (payload && typeof payload === 'object' && !Array.isArray(payload)) {
      return normalizeAsrTranscriptText(payload.text)
    }

    const rawText = String(payload || '').trim()
    if (!rawText) return ''

    try {
      const parsed = JSON.parse(rawText)
      if (Array.isArray(parsed)) {
        const pieces = parsed.map((item) => (typeof item?.text === 'string' ? item.text : ''))
        return joinTranscriptPieces(pieces)
      }
      if (parsed && typeof parsed === 'object' && typeof parsed.text === 'string') {
        return String(parsed.text || '').trim()
      }
    } catch {
      // 普通文本或多个 JSON 对象连续输出时会进入这里，下面继续做兼容解析。
    }

    const jsonTexts = extractJsonObjectTexts(rawText)
    if (jsonTexts.length) {
      const normalized = joinTranscriptPieces(jsonTexts)
      logger.info('ASR 结构化返回已提取 text 字段', {
        segmentCount: jsonTexts.length,
        rawLength: rawText.length,
        normalizedLength: normalized.length,
      })
      return normalized
    }

    return rawText
  }

  const emitTranscriptInOrder = (runId, seq, text) => {
    if (runId !== activeRunId) {
      logger.warn('丢弃过期语音听写片段', {
        runId,
        activeRunId,
        seq,
      })
      return
    }
    transcriptQueue.set(seq, String(text || '').trim())
    while (transcriptQueue.has(nextEmitSeq)) {
      const queuedText = transcriptQueue.get(nextEmitSeq)
      transcriptQueue.delete(nextEmitSeq)
      if (queuedText) {
        logger.info('语音听写片段按序输出', {
          seq: nextEmitSeq,
          textLength: queuedText.length,
        })
        onTranscript?.(queuedText)
      }
      nextEmitSeq += 1
    }
  }

  // 把一段累积的源 PCM 切出来，转写并回调。
  // 设计思路：flush 只负责取走当前缓冲，随后立即允许新的音频帧继续累积；
  // ASR 请求在后台完成，结果再通过 emitTranscriptInOrder 保序插入。
  const flushUtterance = async (reason = 'unknown') => {
    if (!captureLength) return
    const merged = new Float32Array(captureLength)
    let offset = 0
    for (const b of captureBuffers) {
      merged.set(b, offset)
      offset += b.length
    }
    captureBuffers = []
    captureLength = 0

    const durationMs = (merged.length / sourceSampleRate) * 1000
    if (durationMs < MIN_UTTERANCE_MS) return // 噪声片段，丢弃

    const seq = ++transcriptSeq
    const runId = activeRunId
    const pcm16k = resampleTo16k(merged, sourceSampleRate)
    const wav = encodeWav16kMono(pcm16k)
    const base64 = arrayBufferToBase64(wav)

    pendingCount.value += 1
    status.value = 'transcribing'
    logger.info('语音听写片段开始转写', {
      seq,
      reason,
      durationMs: Math.round(durationMs),
      sourceSampleRate,
      pendingCount: pendingCount.value,
    })
    try {
      const common = await resolveAsrRequestCommon()
      const resp = await invoke('audio_speech_to_text', {
        request: {
          common,
          file_base64: base64,
          file_name: 'dictation.wav',
          mime_type: 'audio/wav',
        },
      })
      const rawText = resp?.text ?? ''
      const text = normalizeAsrTranscriptText(rawText)
      if (String(rawText || '').trim() && text !== String(rawText || '').trim()) {
        logger.info('ASR 转写结果已规整为纯文本', {
          rawLength: String(rawText || '').trim().length,
          textLength: text.length,
        })
      }
      if (text) {
        emitTranscriptInOrder(runId, seq, text)
      } else {
        emitTranscriptInOrder(runId, seq, '')
      }
    } catch (e) {
      errorText.value = String(e?.message || e || '转写失败')
      logger.error('转写失败', { seq, reason, error: errorText.value })
      emitTranscriptInOrder(runId, seq, '')
    } finally {
      pendingCount.value = Math.max(0, pendingCount.value - 1)
      if (recording.value) {
        status.value = speaking ? 'speaking' : 'listening'
      }
    }
  }

  const processAudioSamples = (input) => {
    if (!recording.value) return
    if (!input?.length) return
    // 计算 RMS 音量做 VAD
    let sum = 0
    for (let i = 0; i < input.length; i += 1) sum += input[i] * input[i]
    const rms = Math.sqrt(sum / input.length)
    const frameMs = (input.length / sourceSampleRate) * 1000

    const isVoice = rms >= silenceThreshold.value

    if (isVoice) {
      if (!speaking) {
        speaking = true
        status.value = 'speaking'
      }
      silenceMs = 0
      voicedMs += frameMs
      // 累积语音帧（拷贝，因为底层 buffer 会被复用）
      captureBuffers.push(new Float32Array(input))
      captureLength += input.length
      // 连续讲话时也按固定时长切片，让长段录音可以边说边出文字。
      if (voicedMs >= STREAMING_CHUNK_MS) {
        logger.info('语音听写触发连续讲话切片', {
          voicedMs: Math.round(voicedMs),
          captureLength,
        })
        voicedMs = 0
        speaking = false
        silenceMs = 0
        void flushUtterance('streaming-chunk')
      }
    } else if (speaking) {
      // 语音中遇到静音：仍然把静音附上（保留句末气口），并累计静音时长
      silenceMs += frameMs
      captureBuffers.push(new Float32Array(input))
      captureLength += input.length
      if (silenceMs >= SILENCE_HANG_MS) {
        speaking = false
        silenceMs = 0
        voicedMs = 0
        status.value = 'transcribing'
        void flushUtterance('silence')
      }
    }
    // 非说话且没在 speaking：丢弃静音帧，不累积
  }

  const createAudioWorkletProcessor = async () => {
    if (!audioContext?.audioWorklet || !window.AudioWorkletNode || !window.Blob || !window.URL) {
      return null
    }

    let moduleUrl = ''
    try {
      const blob = new Blob([DICTATION_WORKLET_SOURCE], { type: 'application/javascript' })
      moduleUrl = URL.createObjectURL(blob)
      await audioContext.audioWorklet.addModule(moduleUrl)
      const node = new window.AudioWorkletNode(audioContext, DICTATION_WORKLET_PROCESSOR_NAME, {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        outputChannelCount: [1],
      })
      node.port.onmessage = (event) => processAudioSamples(event.data)
      logger.info('语音听写已启用 AudioWorklet 音频采集', { sourceSampleRate })
      return node
    } catch (e) {
      logger.warn('AudioWorklet 初始化失败，将回退到 ScriptProcessor', { error: String(e?.message || e) })
      return null
    } finally {
      if (moduleUrl) URL.revokeObjectURL(moduleUrl)
    }
  }

  const createLegacyScriptProcessor = () => {
    // 兜底说明：只有在当前 WebView 不支持 AudioWorklet 时才使用旧接口。
    // 现代环境会优先走 AudioWorklet，从而避免 ScriptProcessorNode 的弃用警告。
    const node = audioContext.createScriptProcessor(2048, 1, 1)
    node.onaudioprocess = (event) => processAudioSamples(event.inputBuffer.getChannelData(0))
    logger.warn('AudioWorklet 不可用，语音听写回退到 ScriptProcessor 兼容采集')
    return node
  }

  const start = async () => {
    if (recording.value) return
    errorText.value = ''
    try {
      const constraints = {
        audio: selectedDeviceId.value
          ? {
              deviceId: { exact: selectedDeviceId.value },
              echoCancellation: true,
              noiseSuppression: true,
              autoGainControl: true,
            }
          : { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      }
      mediaStream = await navigator.mediaDevices.getUserMedia(constraints)
      // 授权后再刷新一次设备列表，拿到真实 label
      await refreshDevices()

      audioContext = new (window.AudioContext || window.webkitAudioContext)()
      if (audioContext.state === 'suspended') {
        try { await audioContext.resume() } catch { /* noop */ }
      }
      sourceSampleRate = audioContext.sampleRate || TARGET_SAMPLE_RATE
      sourceNode = audioContext.createMediaStreamSource(mediaStream)
      processorNode = await createAudioWorkletProcessor()
      if (!processorNode) {
        processorNode = createLegacyScriptProcessor()
      }
      sourceNode.connect(processorNode)
      // 必须接到 destination 才会持续触发音频图处理；用 0 增益避免把麦克风声音回放出来。
      muteNode = audioContext.createGain()
      muteNode.gain.value = 0
      processorNode.connect(muteNode)
      muteNode.connect(audioContext.destination)

      captureBuffers = []
      captureLength = 0
      speaking = false
      silenceMs = 0
      voicedMs = 0
      transcriptSeq = 0
      nextEmitSeq = 1
      activeRunId += 1
      transcriptQueue.clear()
      recording.value = true
      status.value = 'listening'
      logger.info('开始语音听写', { sourceSampleRate, deviceId: selectedDeviceId.value })
    } catch (e) {
      errorText.value = String(e?.message || e || '无法访问麦克风')
      status.value = 'error'
      logger.error('启动语音听写失败', { error: errorText.value })
      await stop()
      throw e
    }
  }

  const stop = async () => {
    const wasRecording = recording.value
    recording.value = false
    // 把残留的语音片段刷出去
    if (wasRecording && captureLength > 0 && voicedMs >= 0) {
      try { await flushUtterance('stop') } catch { /* noop */ }
    }
    try { processorNode && (processorNode.onaudioprocess = null) } catch { /* noop */ }
    try { processorNode?.port && (processorNode.port.onmessage = null) } catch { /* noop */ }
    try { sourceNode?.disconnect() } catch { /* noop */ }
    try { processorNode?.disconnect() } catch { /* noop */ }
    try { muteNode?.disconnect() } catch { /* noop */ }
    try {
      if (mediaStream) mediaStream.getTracks().forEach((t) => t.stop())
    } catch { /* noop */ }
    try { await audioContext?.close() } catch { /* noop */ }
    audioContext = null
    mediaStream = null
    sourceNode = null
    processorNode = null
    muteNode = null
    captureBuffers = []
    captureLength = 0
    speaking = false
    if (status.value !== 'error') status.value = 'idle'
  }

  const toggle = async () => {
    if (recording.value) await stop()
    else await start()
  }

  return {
    recording,
    status,
    errorText,
    devices,
    selectedDeviceId,
    silenceThreshold,
    pendingCount,
    refreshDevices,
    start,
    stop,
    toggle,
  }
}

export default useSpeechDictation
