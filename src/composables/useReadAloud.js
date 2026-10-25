import { computed, onBeforeUnmount, ref, shallowRef } from 'vue'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'
import { onReadaloudSegment } from '../utils/readaloudEvents'

const logger = createLogger('useReadAloud')

/** 目标缓冲水位（秒）：稳态下始终保留这么多就绪音频排在播放头之后。 */
const TARGET_BUFFER_SECONDS = 25
/** 起播门槛（秒）：够了才开声，避免第一句念完就卡住；命中缓存时瞬间满足。 */
const START_BUFFER_SECONDS = 3
/** 已排定音频的时间跨度上限（秒）：没必要把整本书都排到时间轴上。 */
const SCHEDULE_HORIZON_SECONDS = 30
/** 排程提前量：给解码与调度留一点余量。 */
const LEAD_SECONDS = 0.08
/** 欠载判定余量（秒）：排定音频快播完而下一段还没就绪时，提前暂停等缓冲。 */
const UNDERRUN_MARGIN_SECONDS = 0.15
/** 驱动高亮与补排程的节拍。10Hz 足够跟上句子切换，又不至于忙。 */
const TICK_MS = 100
/** 预取水位刷新节流（毫秒）。 */
const PREFETCH_REFRESH_MS = 2500
/** 播放头之后保留多少段的解码缓冲，更早的释放掉——AudioBuffer 是实打实的内存。 */
const KEEP_DECODED_BEHIND = 4

const base64ToArrayBuffer = (base64) => {
  const binary = atob(String(base64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
  return bytes.buffer
}

/**
 * 逐句朗读的会话与播放调度。
 *
 * 三件事决定了听感是否"连续"，都在这里：
 *
 * 1. **Web Audio 时间轴串排**，不是"每句一个 audio 元素、ended 再播下一个"——后者每句
 *    之间天然有 50–300ms 静音，听起来就是断断续续。
 * 2. **时长水位驱动预取**：告诉后端"播放头在第几段、要保留多少秒"，后端自己算合成到哪。
 * 3. **欠载时暂停而不是放静音**：排定音频快放完而下一段还没合成好，就暂停等缓冲；
 *    宁可短暂停顿并提示"缓冲中"，也不糊一段静音过去。
 *
 * @param {{ onSegmentChange?: (index: number) => void }} [options]
 */
export function useReadAloud(options = {}) {
  const segments = ref([])
  const currentIndex = ref(-1)
  const isPlaying = ref(false)
  const isBuffering = ref(false)
  const error = ref('')
  const cachedMs = ref(0)
  const readyCount = ref(0)
  /** 'streaming' | 'complete'：供 UI 区分"边合成边播"与"播放已留存的整本语音"。 */
  const modeRef = ref('streaming')

  const audioContext = shallowRef(null)
  let sessionId = ''
  let docKey = ''
  // null = 没显式指定，由后端使用设置页「朗读声音」里的全局语速。
  let speed = null
  let speakerId = null
  let unlisten = null
  let ticker = null
  let lastPrefetchAt = 0

  /** index -> { buffer, durationMs, pauseAfterMs }：已解码、可播的段。 */
  const ready = new Map()
  /** index -> wav 路径：已合成到磁盘的段（含缓存命中）。比解码缓冲便宜得多，可以长期保留。 */
  const paths = new Map()
  /** 已排到时间轴上的段（按开始时间升序）。 */
  let timeline = []
  const sources = []
  let scheduledUntil = -1
  let lastEndTime = 0
  /** 用户已按播放但还在等起播缓冲。 */
  let waitingToStart = false

  /** `streaming` = 逐段合成边播边取；`complete` = 播放这本书留存的整本语音（重听零合成）。 */
  let mode = 'streaming'
  /** complete 模式：整本语音的媒体元素与清单时间轴（按 startMs 升序，二分查找当前句）。 */
  let completeAudio = null
  let completePath = ''
  let completeTimeline = []

  const currentText = computed(() => segments.value[currentIndex.value]?.text || '')
  /** 进度按段序号算：段的字数差异会让时长口径失真，而"读到第几段"对这个条来说够用且单调。 */
  const progress = computed(() =>
    segments.value.length ? Math.min(1, (currentIndex.value + 1) / segments.value.length) : 0
  )

  const clampIndex = (value) => Math.max(0, Math.min(segments.value.length - 1, Number(value) || 0))

  // ───────────────────────── 后端交互 ─────────────────────────

  const prepare = async (text, opts = {}) => {
    reset()
    const plain = String(text || '')
    if (!plain.trim()) return false

    docKey = String(opts.docKey || '')
    speed = Number(opts.speed) > 0 ? Number(opts.speed) : null
    speakerId = opts.speakerId ?? null
    sessionId = `readaloud-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`

    const response = await invoke('readaloud_prepare', {
      request: {
        sessionId,
        text: plain,
        library: String(opts.library || ''),
        fileName: String(opts.fileName || ''),
        speed,
        speakerId: speakerId === null ? null : Number(speakerId),
      },
    })

    segments.value = (response?.segments || []).map((segment) => ({
      index: Number(segment.index),
      text: String(segment.text || ''),
      startOffset: Number(segment.startOffset || 0),
      endOffset: Number(segment.endOffset || 0),
      pauseAfterMs: Number(segment.pauseAfterMs || 0),
    }))
    cachedMs.value = Number(response?.cachedMs || 0)
    mode = String(response?.mode || 'streaming')
    modeRef.value = mode

    if (mode === 'complete') {
      // 这本书已经留存了整本语音：播一个文件，按清单时间轴跳句与高亮。
      completePath = String(response?.audioPath || '')
      completeTimeline = (response?.segments || [])
        .map((segment) => ({
          index: Number(segment.index),
          startMs: Number(segment.startMs || 0),
          endMs: Number(segment.endMs || 0),
        }))
        .filter((entry) => Number.isFinite(entry.startMs) && entry.endMs > entry.startMs)
        .sort((a, b) => a.startMs - b.startMs)
      if (!completePath || !completeTimeline.length) {
        mode = 'streaming'
      } else {
        logger.info('命中留存的整本语音，重听无需合成', {
          path: completePath,
          totalMs: Number(response?.totalMs || 0),
        })
        return segments.value.length > 0
      }
    }

    // 缓存命中的段：路径来自 prepare，不必等事件；真正解码推迟到要用的时候。
    for (const segment of response?.segments || []) {
      const cached = segment?.cached
      if (cached?.path) {
        paths.set(Number(segment.index), String(cached.path))
      }
    }
    readyCount.value = 0

    unlisten = await onReadaloudSegment(sessionId, handleSegmentEvent)
    logger.info('朗读会话就绪', {
      segments: segments.value.length,
      cached: paths.size,
      cachedMs: cachedMs.value,
    })
    return segments.value.length > 0
  }

  const handleSegmentEvent = async (event) => {
    if (event.index < 0) return
    if (!event.ok) {
      // 单段失败不终止整场朗读：当成零长度跳过，继续往下读。
      logger.warn('朗读片段合成失败，跳过', event.index, event.error)
      ready.set(event.index, { buffer: null, durationMs: 0, pauseAfterMs: 0, failed: true })
      pump()
      return
    }
    paths.set(event.index, event.path)
    await decodeSegment(event.index)
    pump()
  }

  /** 取回并解码一段音频。失败时标成跳过，避免一个坏文件卡住整条时间轴。 */
  const decodeSegment = async (index) => {
    const path = paths.get(index)
    if (!path || !audioContext.value) return false
    if (ready.has(index)) return true
    try {
      const bytes = await fetchSegmentBytes(path)
      const buffer = await audioContext.value.decodeAudioData(bytes)
      ready.set(index, {
        buffer,
        durationMs: Math.round(buffer.duration * 1000),
        pauseAfterMs: segments.value[index]?.pauseAfterMs || 0,
      })
      readyCount.value += 1
      return true
    } catch (e) {
      logger.warn('朗读片段解码失败，跳过该段', index, e)
      ready.set(index, { buffer: null, durationMs: 0, pauseAfterMs: 0, failed: true })
      return false
    }
  }

  /** 优先走 asset 协议（WebView 直接流式读盘），失败再退到 base64——既有音频播放的稳妥做法。 */
  const fetchSegmentBytes = async (path) => {
    try {
      const response = await fetch(convertFileSrc(path))
      if (response.ok) return await response.arrayBuffer()
    } catch (e) {
      logger.warn('asset 协议取朗读音频失败，回退 base64', e)
    }
    const base64 = await invoke('read_file_bytes_base64', { path })
    return base64ToArrayBuffer(base64)
  }

  /** 把"播放头 + 目标水位"告诉后端；后端据此决定合成到第几段。 */
  const refreshPrefetch = (force = false) => {
    if (!sessionId || !segments.value.length) return
    const now = Date.now()
    if (!force && now - lastPrefetchAt < PREFETCH_REFRESH_MS) return
    lastPrefetchAt = now
    const playhead = clampIndex(currentIndex.value < 0 ? 0 : currentIndex.value)
    invoke('readaloud_prefetch', {
      sessionId,
      playhead,
      targetSeconds: TARGET_BUFFER_SECONDS,
    }).catch((e) => logger.warn('刷新朗读预取失败', e))
  }

  // ───────────────────────── 播放调度 ─────────────────────────

  const ensureContext = async () => {
    if (!audioContext.value) {
      const Ctor = window.AudioContext || window.webkitAudioContext
      if (!Ctor) throw new Error('当前环境不支持 Web Audio，无法朗读')
      audioContext.value = new Ctor()
    }
    if (audioContext.value.state === 'suspended' && !isBuffering.value) {
      await audioContext.value.resume()
    }
    return audioContext.value
  }

  /** 从 `from` 起第一段还没解码好的序号（= 后端还没合成到这儿，或本地还没解码）。 */
  const nextMissing = (from) => {
    for (let i = from; i < segments.value.length; i += 1) {
      if (!ready.has(i)) return i
    }
    return segments.value.length
  }

  /** 流水线：把已就绪的段补排到时间轴上；该等缓冲就等。 */
  const pump = () => {
    if (!isPlaying.value || !audioContext.value) return

    if (waitingToStart) {
      if (bufferedAheadSeconds() >= START_BUFFER_SECONDS) {
        waitingToStart = false
        isBuffering.value = false
        isPlaying.value = true
        schedule()
        startTicker()
        audioContext.value.resume().catch(() => {})
      } else {
        isBuffering.value = true
      }
      return
    }

    if (isBuffering.value) {
      // 还在等真正缺的那一段（不是为了别的段到位就急着恢复，免得"恢复—又欠载"反复抖）。
      const from = scheduledUntil < 0 ? currentIndex.value : scheduledUntil
      const missing = nextMissing(from)
      if (missing < segments.value.length && !ready.has(missing)) return
      isBuffering.value = false
      audioContext.value.resume().catch(() => {})
      schedule()
      return
    }

    schedule()
  }

  /** 把 ready 里连续可播的段按时间轴串起来（样本级首尾相接，句间只留标点对应的停顿）。 */
  const schedule = () => {
    const context = audioContext.value
    if (!context || scheduledUntil < 0) return
    const horizon = context.currentTime + SCHEDULE_HORIZON_SECONDS
    if (lastEndTime < context.currentTime) {
      lastEndTime = context.currentTime + LEAD_SECONDS
    }

    while (scheduledUntil < segments.value.length && lastEndTime < horizon) {
      const index = scheduledUntil
      const entry = ready.get(index)
      if (!entry) break
      scheduledUntil += 1
      if (entry.failed || !entry.buffer) continue

      const source = context.createBufferSource()
      source.buffer = entry.buffer
      source.connect(context.destination)
      const start = lastEndTime
      source.start(start)
      const end = start + entry.buffer.duration
      sources.push(source)
      timeline.push({ index, start, end })
      lastEndTime = end + Math.max(0, entry.pauseAfterMs || 0) / 1000
    }
  }

  /** 播放头之后已就绪的连续音频总时长（秒）。 */
  const bufferedAheadSeconds = () => {
    let total = 0
    const from = currentIndex.value < 0 ? 0 : currentIndex.value
    for (let i = from; i < segments.value.length; i += 1) {
      const entry = ready.get(i)
      if (!entry) break
      if (entry.buffer) total += entry.buffer.duration
    }
    return total
  }

  const startTicker = () => {
    if (ticker) return
    ticker = setInterval(() => (mode === 'complete' ? completeTick() : tick()), TICK_MS)
  }

  const stopTicker = () => {
    if (ticker) {
      clearInterval(ticker)
      ticker = null
    }
  }

  /** 心跳：跟着音频时钟更新当前句（驱动高亮/滚动）、补排程、判断欠载、回收旧缓冲。 */
  const tick = () => {
    const context = audioContext.value
    if (!context || !isPlaying.value) return
    const now = context.currentTime

    const playing = timeline.find((item) => now >= item.start && now < item.end)
    if (playing && playing.index !== currentIndex.value) {
      currentIndex.value = playing.index
      options.onSegmentChange?.(playing.index)
      refreshPrefetch()
    }
    while (timeline.length && timeline[0].end <= now) timeline.shift()

    // 回收已播过的解码缓冲：长听时只进不出会把内存撑爆（留几段供回退重取）。
    const floor = currentIndex.value - KEEP_DECODED_BEHIND
    for (let i = 0; i < floor; i += 1) {
      if (!ready.has(i)) break
      ready.delete(i)
    }

    if (!isBuffering.value) {
      const missing = nextMissing(scheduledUntil)
      const willRunDry = lastEndTime - now < UNDERRUN_MARGIN_SECONDS
      if (missing < segments.value.length && !ready.has(missing) && willRunDry) {
        // 欠载：暂停等缓冲，而不是放静音。恢复由段就绪事件驱动。
        isBuffering.value = true
        context.suspend().catch(() => {})
        logger.debug('朗读缓冲不足，暂停等待', { missing, remaining: lastEndTime - now })
        return
      }
      schedule()
      refreshPrefetch()
    }
  }

  // ─────────────────── 整本留存模式（重听） ───────────────────
  // 整本 WAV 可能上 GB，`decodeAudioData` 要整块进内存，必然爆；媒体元素走 asset 协议
  // 由 Chromium 按 Range 流式读取，随便 seek——这才是"再听一遍要快"的正解。

  const ensureCompleteAudio = () => {
    if (completeAudio || !completePath) return completeAudio
    const audio = new Audio(convertFileSrc(completePath))
    audio.preload = 'auto'
    // 拉 Range 时的等待映射成"缓冲中"：重听也可能撞上磁盘忙。
    audio.addEventListener('waiting', () => {
      isBuffering.value = true
    })
    audio.addEventListener('playing', () => {
      isBuffering.value = false
    })
    audio.addEventListener('ended', () => {
      isPlaying.value = false
      isBuffering.value = false
      stopTicker()
    })
    audio.addEventListener('error', () => {
      error.value = `整本语音播放失败：${completePath}`
      isPlaying.value = false
      isBuffering.value = false
      stopTicker()
      logger.error('整本语音播放失败', completePath)
    })
    completeAudio = audio
    return audio
  }

  /** 播放时间 → 段序号（清单时间轴按 startMs 升序，二分）。 */
  const completeIndexAt = (ms) => {
    let low = 0
    let high = completeTimeline.length - 1
    while (low <= high) {
      const mid = (low + high) >> 1
      const entry = completeTimeline[mid]
      if (ms < entry.startMs) {
        high = mid - 1
      } else if (ms >= entry.endMs) {
        low = mid + 1
      } else {
        return entry.index
      }
    }
    return -1
  }

  const completeTick = () => {
    const audio = completeAudio
    if (!audio || !isPlaying.value) return
    const index = completeIndexAt(audio.currentTime * 1000)
    if (index >= 0 && index !== currentIndex.value) {
      currentIndex.value = index
      options.onSegmentChange?.(index)
    }
  }

  const startCompleteAt = async (index) => {
    const audio = ensureCompleteAudio()
    if (!audio) return
    const entry = completeTimeline.find((item) => item.index === index) || completeTimeline[0]
    if (!entry) return
    currentIndex.value = entry.index
    isPlaying.value = true
    try {
      audio.currentTime = entry.startMs / 1000
      await audio.play()
      startTicker()
    } catch (e) {
      error.value = `整本语音播放失败：${String(e?.message || e || '')}`
      isPlaying.value = false
      logger.error('整本语音起播失败', e)
    }
    options.onSegmentChange?.(entry.index)
  }

  const releaseCompleteAudio = () => {
    if (!completeAudio) return
    try {
      completeAudio.pause()
      completeAudio.src = ''
    } catch {
      /* noop */
    }
    completeAudio = null
  }

  // ───────────────────────── 对外动作 ─────────────────────────

  const startAt = async (index) => {
    if (mode === 'complete') {
      await startCompleteAt(clampIndex(index))
      return
    }
    // 先清缓冲态：它会让 ensureContext 认为"暂停是刻意的"而不去恢复音频时钟。
    isBuffering.value = false
    const context = await ensureContext()
    stopSources()
    const from = clampIndex(index)
    currentIndex.value = from
    timeline = []
    scheduledUntil = from
    lastEndTime = context.currentTime + LEAD_SECONDS
    // 起播位置可能命中磁盘缓存：先把这几段解码出来，否则时间轴上没东西可排。
    await ensureDecoded(from, 6)
    waitingToStart = true
    isPlaying.value = true
    startTicker()
    refreshPrefetch(true)
    pump()
    options.onSegmentChange?.(currentIndex.value)
  }

  /** 把 paths 里已有、但尚未解码的段按顺序解码出来（回退重播时用得上）。 */
  const ensureDecoded = async (from, count = 6) => {
    for (let i = from; i < Math.min(segments.value.length, from + count); i += 1) {
      if (ready.has(i)) continue
      if (!paths.has(i)) break
      // eslint-disable-next-line no-await-in-loop
      await decodeSegment(i)
    }
  }

  const play = async (opts = {}) => {
    if (!segments.value.length) return
    const target = opts.fromIndex === undefined ? currentIndex.value : opts.fromIndex
    await startAt(target < 0 ? 0 : target)
  }

  const pause = () => {
    if (!isPlaying.value) return
    if (mode === 'complete') {
      completeAudio?.pause()
      isPlaying.value = false
      return
    }
    if (!audioContext.value) return
    isPlaying.value = false
    audioContext.value.suspend().catch(() => {})
    if (sessionId) {
      invoke('readaloud_stop', { sessionId }).catch((e) => logger.warn('停止朗读合成失败', e))
    }
  }

  const resume = () => {
    if (!segments.value.length) return
    if (currentIndex.value < 0) {
      play({ fromIndex: 0 })
      return
    }
    if (mode === 'complete') {
      const audio = ensureCompleteAudio()
      if (!audio) return
      isPlaying.value = true
      audio
        .play()
        .then(() => startTicker())
        .catch((e) => {
          isPlaying.value = false
          error.value = `整本语音播放失败：${String(e?.message || e || '')}`
          logger.error('整本语音恢复播放失败', e)
        })
      return
    }
    isPlaying.value = true
    ensureContext()
      .then(() => {
        pump()
        startTicker()
        refreshPrefetch(true)
      })
      .catch((e) => logger.warn('恢复朗读失败', e))
  }

  const toggle = () => (isPlaying.value ? pause() : resume())

  const seek = async (index) => {
    const wasPlaying = isPlaying.value
    await startAt(index)
    if (!wasPlaying) {
      pause()
    }
  }

  const next = () => seek(clampIndex(currentIndex.value + 1))
  const prev = () => seek(clampIndex(currentIndex.value - 1))

  const stop = () => {
    if (mode === 'complete') {
      completeAudio?.pause()
    }
    stopSources()
    isPlaying.value = false
    isBuffering.value = false
    waitingToStart = false
    timeline = []
    scheduledUntil = -1
    stopTicker()
    if (sessionId) {
      invoke('readaloud_stop', { sessionId }).catch(() => {})
    }
  }

  const stopSources = () => {
    while (sources.length) {
      const source = sources.pop()
      try {
        source.stop()
      } catch {
        /* 已经播完 */
      }
      try {
        source.disconnect()
      } catch {
        /* noop */
      }
    }
  }

  /** 只重置本地状态，不动后端（后端会话由 dispose/drop 收尾）。 */
  const reset = () => {
    stop()
    releaseCompleteAudio()
    completePath = ''
    completeTimeline = []
    mode = 'streaming'
    modeRef.value = 'streaming'
    ready.clear()
    paths.clear()
    segments.value = []
    currentIndex.value = -1
    error.value = ''
    cachedMs.value = 0
    readyCount.value = 0
  }

  /** 彻底收尾：停播、退订、通知后端释放会话与专用引擎。 */
  const dispose = async () => {
    const id = sessionId
    reset()
    sessionId = ''
    if (unlisten) {
      try {
        unlisten()
      } catch {
        /* noop */
      }
      unlisten = null
    }
    if (id) {
      try {
        await invoke('readaloud_drop', { sessionId: id })
      } catch (e) {
        logger.warn('释放朗读会话失败', e)
      }
    }
    if (audioContext.value) {
      try {
        await audioContext.value.close()
      } catch {
        /* noop */
      }
      audioContext.value = null
    }
  }

  onBeforeUnmount(() => {
    dispose()
  })

  return {
    // 状态
    segments,
    currentIndex,
    isPlaying,
    isBuffering,
    error,
    progress,
    currentText,
    cachedMs,
    readyCount,
    mode: modeRef,
    // 动作
    prepare,
    play,
    pause,
    resume,
    toggle,
    next,
    prev,
    seek,
    stop,
    dispose,
  }
}
