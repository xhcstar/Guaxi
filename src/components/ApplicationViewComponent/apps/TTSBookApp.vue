<template>
  <div class="tts-book-root" :class="{ 'tts-book-dark': isDarkTheme }">
    <header class="tts-header">
      <div>
        <p class="eyebrow">TTS Studio</p>
        <h1 class="page-title">有声书转换</h1>
      </div>
    </header>

    <section class="tts-layout">
      <n-card title="转换参数" size="small" class="tts-panel control-panel">
        <div class="panel-scroll">
          <n-space vertical :size="16">
            <div class="panel-block">
              <div class="block-label">输出设置</div>
              <div class="toolbar-row">
                <n-input v-model:value="outputDir" placeholder="输出目录，默认使用首个文件所在目录" clearable />
                <n-tooltip>
                  <template #trigger>
                    <n-button tertiary @click="pickOutputDir">
                      <template #icon>
                        <n-icon>
                          <FolderOpenOutline />
                        </n-icon>
                      </template>
                      目录
                    </n-button>
                  </template>
                  选择输出目录
                </n-tooltip>
              </div>
              <n-input v-model:value="outputName" placeholder="输出文件名，仅单文件任务生效" />
            </div>

            <div class="panel-block">
              <div class="block-label">合成参数</div>
              <div class="options-grid">
                <div class="option-card option-card-wide">
                  <div class="option-title">模型</div>
                  <n-select v-model:value="modelPreset" :options="modelOptions" />
                  <div class="option-tip">{{ resolvedModelLabel }}</div>
                </div>

                <div class="option-card">
                  <div class="option-title">Speaker ID</div>
                  <n-select v-model:value="speakerId" filterable :options="speakerOptions" placeholder="选择说话人" />
                </div>

                <div class="option-card">
                  <div class="option-title">语速</div>
                  <n-input-number v-model:value="ttsSpeed" :min="0.2" :max="3" :step="0.1" :precision="1"
                    style="width: 100%" />
                </div>

                <div class="option-card">
                  <div class="option-title">分段长度</div>
                  <n-input-number v-model:value="maxChunkChars" :min="80" :max="420" :step="10" style="width: 100%" />
                </div>

                <div class="option-card">
                  <div class="option-title">转换引擎</div>
                  <n-select v-model:value="executionProvider" :options="executionProviderOptions" />
                  <div class="option-tip">切换 sherpa-onnx 的执行 provider</div>
                </div>

                <div class="option-card">
                  <div class="option-title">章节拆分</div>
                  <n-switch v-model:value="splitByToc">
                    <template #checked>开启</template>
                    <template #unchecked>关闭</template>
                  </n-switch>
                </div>
              </div>
            </div>

            <n-space>
              <n-tooltip>
                <template #trigger>
                  <n-button type="primary" :loading="running" :disabled="!canStart" @click="startConvertQueue">
                    <template #icon>
                      <n-icon>
                        <PlayCircleOutline />
                      </n-icon>
                    </template>
                    开始转换
                  </n-button>
                </template>
                开始执行当前任务队列
              </n-tooltip>
              <n-tooltip>
                <template #trigger>
                  <n-button :disabled="!canPauseAll" @click="pauseAllTasks">
                    <template #icon>
                      <n-icon>
                        <PauseOutline />
                      </n-icon>
                    </template>
                    暂停全部
                  </n-button>
                </template>
                暂停当前和等待中的任务，稍后可继续
              </n-tooltip>
              <n-tooltip>
                <template #trigger>
                  <n-button :disabled="!canStopAll" @click="stopAllTasks">
                    <template #icon>
                      <n-icon>
                        <StopOutline />
                      </n-icon>
                    </template>
                    停止全部
                  </n-button>
                </template>
                终止当前和等待中的任务
              </n-tooltip>
              <n-tooltip>
                <template #trigger>
                  <n-button :disabled="running" @click="clearFinished">
                    <template #icon>
                      <n-icon>
                        <TrashOutline />
                      </n-icon>
                    </template>
                    清理完成任务
                  </n-button>
                </template>
                清理已完成和失败任务
              </n-tooltip>
            </n-space>
          </n-space>
        </div>
      </n-card>

      <section class="middle-column">
        <n-card title="已打开文件" size="small" class="tts-panel files-panel">
          <template #header-extra>
            <div class="files-header-actions">
              <span class="panel-extra">{{ selectedFiles.length }} 个文件</span>
              <n-tooltip>
                <template #trigger>
                  <n-button tertiary circle size="small" @click="pickFiles">
                    <template #icon>
                      <n-icon>
                        <AddOutline />
                      </n-icon>
                    </template>
                  </n-button>
                </template>
                添加文件（支持单个或批量添加）
              </n-tooltip>
            </div>
          </template>

          <div class="files-scroll">
            <n-empty v-if="!selectedFiles.length" description="还没有打开文件" />
            <div v-else class="file-grid">
              <div v-for="file in selectedFiles" :key="file" class="file-card">
                <div class="file-icon">
                  <n-icon size="18">
                    <DocumentTextOutline />
                  </n-icon>
                </div>
                <div class="file-info">
                  <div class="file-name">{{ basename(file) }}</div>
                  <div class="file-meta">{{ fileTypeLabel(file) }}</div>
                </div>
                <n-tooltip>
                  <template #trigger>
                    <n-button quaternary circle @click="removeSelected(file)">
                      <template #icon>
                        <n-icon>
                          <TrashOutline />
                        </n-icon>
                      </template>
                    </n-button>
                  </template>
                  移除文件
                </n-tooltip>
              </div>
            </div>
          </div>
        </n-card>

        <n-card title="音频播放器" size="small" class="tts-panel player-panel">
          <template #header-extra>
            <div class="player-header-actions">
              <span class="panel-extra">{{ playerTitle || '等待播放' }}</span>
              <n-tooltip>
                <template #trigger>
                  <n-button circle tertiary size="small" @click="pickPlayerMedia">
                    <template #icon>
                      <n-icon>
                        <FolderOpenOutline />
                      </n-icon>
                    </template>
                  </n-button>
                </template>
                选择音频和字幕 JSON
              </n-tooltip>
            </div>
          </template>

          <div class="player-body">
            <n-empty v-if="!currentAudioSrc" description="点击右侧任务队列中的音频按钮即可在这里播放" />
            <template v-else>
              <audio ref="audioRef" class="audio-element" :src="currentAudioSrc" controls preload="metadata"
                @loadedmetadata="handleAudioMetadata" @timeupdate="handleAudioTimeUpdate" @ended="handleAudioEnded" />

              <div class="subtitle-panel">
                <div class="subtitle-label">当前字幕</div>
                <div class="subtitle-current">{{ currentSubtitleText }}</div>
                <div class="subtitle-list">
                  <button v-for="(item, index) in subtitleTimeline" :key="`${index}-${item.text}`" type="button"
                    class="subtitle-item" :class="{ active: index === currentSubtitleIndex }"
                    @click="seekToSubtitle(index)">
                    <span class="subtitle-index">{{ index + 1 }}</span>
                    <span class="subtitle-text">{{ item.text }}</span>
                  </button>
                </div>
              </div>
            </template>
          </div>
        </n-card>
      </section>

      <aside class="sidebar">
        <n-card title="任务队列" size="small" class="tts-panel queue-panel">
          <template #header-extra>
            <span class="panel-extra">{{ totalTasks }} 项</span>
          </template>

          <div class="queue-layout">
            <div class="stat-grid">
              <div class="stat-card">
                <div class="stat-label">总任务</div>
                <div class="stat-value">{{ totalTasks }}</div>
              </div>
              <div class="stat-card">
                <div class="stat-label">成功</div>
                <div class="stat-value success">{{ successTasks }}</div>
              </div>
              <div class="stat-card">
                <div class="stat-label">失败</div>
                <div class="stat-value error">{{ failedTasks }}</div>
              </div>
            </div>

            <div class="progress-card">
              <div class="block-label">任务完成度</div>
              <n-progress type="line" :percentage="progressPercent" :show-indicator="true" :height="10" />
            </div>

            <div class="task-scroll">
              <n-empty v-if="!tasks.length" description="暂无任务" />
              <n-list v-else bordered class="task-list">
                <n-list-item v-for="task in tasks" :key="task.id">
                  <div class="queue-item">
                    <div class="task-main">
                      <div class="task-head">
                        <n-tag size="small" :type="tagType(task.status)">{{ taskStatusLabel(task) }}</n-tag>
                        <span class="task-name">{{ task.fileName }}</span>
                      </div>
                      <div class="task-meta">{{ task.message }}</div>
                      <n-progress v-if="task.status === 'running' || task.progress > 0 || task.status === 'success'"
                        type="line" :percentage="task.progress" :show-indicator="true" :height="8"
                        :status="task.status === 'error' ? 'error' : task.status === 'success' ? 'success' : 'info'" />
                      <div v-if="task.stats" class="task-meta">
                        分段 {{ task.stats.total_chunks }}，中文 {{ task.stats.zh_chunks }}，英文 {{ task.stats.en_chunks }}
                      </div>
                      <div v-if="task.chapterCount > 0" class="task-meta">章节音频 {{ task.chapterCount }} 个</div>
                    </div>

                    <div class="queue-actions">
                      <n-tooltip v-if="task.status === 'running'">
                        <template #trigger>
                          <n-button quaternary circle size="small" @click="pauseTask(task)">
                            <template #icon>
                              <n-icon>
                                <PauseOutline />
                              </n-icon>
                            </template>
                          </n-button>
                        </template>
                        暂停任务
                      </n-tooltip>
                      <n-tooltip v-else-if="task.status === 'paused' || task.status === 'queued'">
                        <template #trigger>
                          <n-button quaternary circle size="small" :disabled="running" @click="resumeTask(task)">
                            <template #icon>
                              <n-icon>
                                <PlayOutline />
                              </n-icon>
                            </template>
                          </n-button>
                        </template>
                        继续任务
                      </n-tooltip>
                      <n-tooltip v-if="task.status !== 'success' && task.status !== 'stopped'">
                        <template #trigger>
                          <n-button quaternary circle size="small" @click="stopTask(task)">
                            <template #icon>
                              <n-icon>
                                <StopOutline />
                              </n-icon>
                            </template>
                          </n-button>
                        </template>
                        终止任务
                      </n-tooltip>
                      <n-tooltip v-if="task.outputPath">
                        <template #trigger>
                          <n-button size="tiny" tertiary @click="playTaskAudio(task)">
                            <template #icon>
                              <n-icon>
                                <PlayOutline />
                              </n-icon>
                            </template>
                            音频
                          </n-button>
                        </template>
                        在中间播放器中播放
                      </n-tooltip>
                      <n-tooltip v-if="task.outputPath">
                        <template #trigger>
                          <n-button size="tiny" tertiary @click="openOutput(task.outputPath)">
                            <template #icon>
                              <n-icon>
                                <FolderOpenOutline />
                              </n-icon>
                            </template>
                            目录
                          </n-button>
                        </template>
                        打开音频所在位置
                      </n-tooltip>
                      <n-tooltip v-if="task.segmentJsonPath">
                        <template #trigger>
                          <n-button size="tiny" tertiary @click="openOutput(task.segmentJsonPath)">
                            <template #icon>
                              <n-icon>
                                <DocumentTextOutline />
                              </n-icon>
                            </template>
                            JSON
                          </n-button>
                        </template>
                        打开分段 JSON
                      </n-tooltip>
                      <n-tooltip v-if="task.chapterOutputPaths?.length">
                        <template #trigger>
                          <n-button size="tiny" tertiary @click="openOutput(task.chapterOutputPaths[0])">
                            <template #icon>
                              <n-icon>
                                <LibraryOutline />
                              </n-icon>
                            </template>
                            章节
                          </n-button>
                        </template>
                        打开章节音频位置
                      </n-tooltip>
                    </div>
                  </div>
                </n-list-item>
              </n-list>
            </div>
          </div>
        </n-card>
      </aside>
    </section>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { AddOutline, DocumentTextOutline, FolderOpenOutline, LibraryOutline, PauseOutline, PlayCircleOutline, PlayOutline, StopOutline, TrashOutline } from '@vicons/ionicons5'
import { createLogger } from '../../../utils/logger'
import { useSettingsStore } from '../../../stores/settingsStore'

const logger = createLogger('TTSBookApp')
const { message } = createDiscreteApi(['message'])
const settingsStore = useSettingsStore()

const DEFAULT_TTS_EXE = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
const DEFAULT_MODEL = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
const DEFAULT_TOKENS = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
const KOKORO_SPEAKER_NAMES = ['af_maple', 'af_sol', 'bf_vale', 'zf_001', 'zf_002', 'zf_003', 'zf_004', 'zf_005', 'zf_006', 'zf_007', 'zf_008', 'zf_017', 'zf_018', 'zf_019', 'zf_021', 'zf_022', 'zf_023', 'zf_024', 'zf_026', 'zf_027', 'zf_028', 'zf_032', 'zf_036', 'zf_038', 'zf_039', 'zf_040', 'zf_042', 'zf_043', 'zf_044', 'zf_046', 'zf_047', 'zf_048', 'zf_049', 'zf_051', 'zf_059', 'zf_060', 'zf_067', 'zf_070', 'zf_071', 'zf_072', 'zf_073', 'zf_074', 'zf_075', 'zf_076', 'zf_077', 'zf_078', 'zf_079', 'zf_083', 'zf_084', 'zf_085', 'zf_086', 'zf_087', 'zf_088', 'zf_090', 'zf_092', 'zf_093', 'zf_094', 'zf_099', 'zm_009', 'zm_010', 'zm_011', 'zm_012', 'zm_013', 'zm_014', 'zm_015', 'zm_016', 'zm_020', 'zm_025', 'zm_029', 'zm_030', 'zm_031', 'zm_033', 'zm_034', 'zm_035', 'zm_037', 'zm_041', 'zm_045', 'zm_050', 'zm_052', 'zm_053', 'zm_054', 'zm_055', 'zm_056', 'zm_057', 'zm_058', 'zm_061', 'zm_062', 'zm_063', 'zm_064', 'zm_065', 'zm_066', 'zm_068', 'zm_069', 'zm_080', 'zm_081', 'zm_082', 'zm_089', 'zm_091', 'zm_095', 'zm_096', 'zm_097', 'zm_098', 'zm_100']

const ttsConfig = ref({ provider: 'sherpa-onnx', model: DEFAULT_MODEL, tokens: DEFAULT_TOKENS, exe: DEFAULT_TTS_EXE })
const selectedFiles = ref([])
const outputDir = ref('')
const outputName = ref('')
const maxChunkChars = ref(180)
const splitByToc = ref(true)
const modelPreset = ref('settings')
const executionProvider = ref('cpu')
const speakerId = ref('0')
const ttsSpeed = ref(1)
const running = ref(false)
const tasks = ref([])
const currentRunningTaskId = ref('')
const audioRef = ref(null)
const currentAudioSrc = ref('')
const currentPlayingTaskId = ref('')
const currentSubtitleJsonPath = ref('')
const playerTitle = ref('')
const currentSubtitleIndex = ref(-1)
const subtitleTimeline = ref([])
let unlistenTtsBookProgress = null

const isDarkTheme = computed(() => settingsStore.appearanceTheme === 'dark')
const totalTasks = computed(() => tasks.value.length)
const successTasks = computed(() => tasks.value.filter((t) => t.status === 'success').length)
const failedTasks = computed(() => tasks.value.filter((t) => t.status === 'error').length)
const progressPercent = computed(() => totalTasks.value ? Math.round(tasks.value.reduce((sum, task) => sum + Number(task.progress || 0), 0) / totalTasks.value) : 0)
const hasPendingTasks = computed(() => tasks.value.some((task) => ['queued', 'paused'].includes(task.status)))
const canStart = computed(() => !running.value && (selectedFiles.value.length > 0 || hasPendingTasks.value))
const canPauseAll = computed(() => tasks.value.some((task) => task.status === 'running' || task.status === 'queued'))
const canStopAll = computed(() => tasks.value.some((task) => ['running', 'queued', 'paused', 'error'].includes(task.status)))
const currentSubtitleText = computed(() => currentSubtitleIndex.value >= 0 ? (subtitleTimeline.value[currentSubtitleIndex.value]?.text || '等待字幕...') : '等待字幕...')

const modelOptions = computed(() => [
  { label: '跟随全局设置', value: 'settings' },
  { label: 'Kokoro Multi Lang', value: 'kokoro_multi_lang' },
  { label: 'VITS Melo ZH/EN', value: 'vits_melo_zh_en' },
])
const executionProviderOptions = [
  { label: 'CPU', value: 'cpu' },
  { label: 'GPU / CUDA', value: 'cuda' },
]

const inferModeHint = (model) => {
  const value = String(model || '').toLowerCase()
  if (value.includes('kokoro')) return 'kokoro'
  if (value.includes('vits')) return 'vits'
  return ''
}

const resolveModelConfig = () => {
  if (modelPreset.value === 'vits_melo_zh_en') return { model: 'utils/Models/tts/vits-melo-tts-zh-en/model.onnx', tokens: 'utils/Models/tts/vits-melo-tts-zh-en/tokens.txt', modeHint: 'vits', label: 'VITS Melo ZH/EN' }
  if (modelPreset.value === 'kokoro_multi_lang') return { model: 'utils/Models/tts/kokoro_multi_lang/model.onnx', tokens: 'utils/Models/tts/kokoro_multi_lang/tokens.txt', modeHint: 'kokoro', label: 'Kokoro Multi Lang' }
  return { model: String(ttsConfig.value.model || DEFAULT_MODEL), tokens: String(ttsConfig.value.tokens || DEFAULT_TOKENS), modeHint: inferModeHint(ttsConfig.value.model), label: '跟随全局设置' }
}

const speakerOptions = computed(() => {
  if (String(resolveModelConfig().modeHint || '').toLowerCase() !== 'kokoro') {
    return Array.from({ length: 16 }, (_, index) => ({ label: `Speaker ${index}`, value: String(index) }))
  }
  const groups = { chineseFemale: [], chineseMale: [], americanFemale: [], britishFemale: [], other: [] }
  KOKORO_SPEAKER_NAMES.forEach((name, index) => {
    const option = { label: `${index} · ${name}`, value: String(index) }
    if (name.startsWith('zf_')) groups.chineseFemale.push(option)
    else if (name.startsWith('zm_')) groups.chineseMale.push(option)
    else if (name.startsWith('af_')) groups.americanFemale.push(option)
    else if (name.startsWith('bf_')) groups.britishFemale.push(option)
    else groups.other.push(option)
  })
  return [
    { type: 'group', label: '美式女声', key: 'american-female', children: groups.americanFemale },
    { type: 'group', label: '英式女声', key: 'british-female', children: groups.britishFemale },
    { type: 'group', label: '中文女声', key: 'chinese-female', children: groups.chineseFemale },
    { type: 'group', label: '中文男声', key: 'chinese-male', children: groups.chineseMale },
    { type: 'group', label: '其他', key: 'other', children: groups.other },
  ].filter((group) => group.children.length > 0)
})

const basename = (p) => String(p || '').split(/[\\/]/).pop() || 'unknown'
const dirname = (p) => {
  const normalized = String(p || '').trim().replace(/\\/g, '/')
  const i = normalized.lastIndexOf('/')
  return i > 0 ? normalized.slice(0, i) : ''
}

const resolvedModelLabel = computed(() => `${resolveModelConfig().label} · ${basename(resolveModelConfig().model)}`)
const fileTypeLabel = (file) => {
  const ext = basename(file).split('.').pop()?.toLowerCase() || ''
  return ext ? ext.toUpperCase() : '未知类型'
}

const tagType = (status) => {
  if (status === 'queued') return 'default'
  if (status === 'running') return 'info'
  if (status === 'paused') return 'warning'
  if (status === 'success') return 'success'
  if (status === 'error' || status === 'stopped') return 'error'
  return 'default'
}
const taskStatusLabel = (task) => {
  if (task.status === 'queued') return '排队中'
  if (task.status === 'running') return task.resumed ? '继续处理中' : '处理中'
  if (task.status === 'paused') return '已暂停'
  if (task.status === 'stopped') return '已终止'
  if (task.status === 'success') return '已完成'
  if (task.status === 'error') return '失败'
  return '未知'
}

const updateTask = (taskId, patch) => {
  const index = tasks.value.findIndex((item) => item.id === taskId || item.taskId === taskId || item.filePath === taskId)
  if (index < 0) return
  tasks.value.splice(index, 1, { ...tasks.value[index], ...patch })
}

const clearFinished = () => {
  tasks.value = tasks.value.filter((t) => ['running', 'queued', 'paused'].includes(t.status))
}

const snapshotCurrentTaskConfig = () => {
  const resolvedModel = resolveModelConfig()
  return {
    outputDir: String(outputDir.value || '').trim(),
    outputName: String(outputName.value || '').trim(),
    maxChunkChars: Number(maxChunkChars.value || 180),
    splitByToc: !!splitByToc.value,
    ttsModel: resolvedModel.model,
    ttsTokens: resolvedModel.tokens,
    ttsModeHint: resolvedModel.modeHint,
    speakerId: String(speakerId.value || '').trim() || '0',
    speed: Number(ttsSpeed.value || 1),
    executionProvider: String(executionProvider.value || 'cpu').trim() || 'cpu',
  }
}

const createTask = (filePath, overrides = {}) => {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`
  return {
    id,
    taskId: id,
    filePath,
    fileName: basename(filePath),
    title: '',
    status: 'queued',
    message: '等待处理...',
    outputPath: '',
    segmentJsonPath: '',
    chapterCount: 0,
    chapterOutputPaths: [],
    progress: 0,
    resumed: false,
    stats: null,
    ...snapshotCurrentTaskConfig(),
    ...overrides,
  }
}

const loadTtsConfig = async () => {
  try {
    const saved = await invoke('get_tts_config')
    ttsConfig.value = { provider: String(saved?.provider || 'sherpa-onnx'), model: String(saved?.model || DEFAULT_MODEL), tokens: String(saved?.tokens || DEFAULT_TOKENS), exe: String(saved?.exe || DEFAULT_TTS_EXE) }
  } catch (e) {
    logger.error('loadTtsConfig failed:', e)
  }
}

const readJsonFile = async (path) => {
  const { readFile } = await import('@tauri-apps/plugin-fs')
  const bytes = await readFile(path)
  return JSON.parse(new TextDecoder().decode(bytes))
}

const buildSubtitleTimeline = (chunks, durationSeconds) => {
  if (!Array.isArray(chunks) || !chunks.length) return []
  const hasExactTimeline = chunks.every((chunk) => Number.isFinite(Number(chunk?.start_ms)) && Number.isFinite(Number(chunk?.end_ms)))
  if (hasExactTimeline) {
    return chunks
      .map((chunk) => ({
        text: String(chunk?.text || '').trim(),
        start: Number(chunk?.start_ms || 0) / 1000,
        end: Number(chunk?.end_ms || 0) / 1000,
      }))
      .filter((item) => item.text && item.end > item.start)
  }
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return []
  const weighted = chunks
    .map((chunk) => ({
      text: String(chunk?.text || '').trim(),
      weight: Math.max(Number(chunk?.char_count || String(chunk?.text || '').length || 1), 1) + Number(chunk?.pause_after_ms || 0) / 120,
    }))
    .filter((item) => item.text)
  const totalWeight = weighted.reduce((sum, item) => sum + item.weight, 0) || 1
  let cursor = 0
  return weighted.map((item) => {
    const length = (item.weight / totalWeight) * durationSeconds
    const entry = { text: item.text, start: cursor, end: cursor + length }
    cursor += length
    return entry
  })
}

const findSiblingJsonPath = (audioPath) => {
  const normalized = String(audioPath || '').trim().replace(/\\/g, '/')
  const stem = normalized.replace(/\.[^/.]+$/, '')
  return [`${stem}_segments.json`, `${stem}.json`]
}

const pickFiles = async () => {
  try {
    const picked = await open({ multiple: true, directory: false, filters: [{ name: 'Book Files', extensions: ['pdf', 'docx', 'md', 'markdown', 'txt', 'epub'] }] })
    if (typeof picked === 'string' && picked.trim()) {
      selectedFiles.value = [picked]
      return
    }
    if (Array.isArray(picked)) {
      selectedFiles.value = [...new Set(picked.map((p) => String(p || '').trim()).filter(Boolean))]
    }
  } catch (e) {
    logger.error('pickFiles failed:', e)
    message.error('选择文件失败')
  }
}

const removeSelected = (file) => {
  selectedFiles.value = selectedFiles.value.filter((p) => p !== file)
}

const pickOutputDir = async () => {
  try {
    const picked = await open({ multiple: false, directory: true })
    if (typeof picked === 'string' && picked.trim()) outputDir.value = picked
  } catch (e) {
    logger.error('pickOutputDir failed:', e)
    message.error('选择输出目录失败')
  }
}

const applyTaskProgress = (payload) => {
  const task = tasks.value.find((item) => item.id === payload?.task_id || item.taskId === payload?.task_id || item.filePath === payload?.file_path)
  if (!task) return
  const nextStatus = payload?.failed
    ? 'error'
    : payload?.stage === 'completed'
      ? 'success'
      : payload?.stage === 'paused'
        ? 'paused'
        : payload?.stage === 'stopped'
          ? 'stopped'
          : 'running'
  updateTask(task.id, {
    taskId: String(payload?.task_id || task.taskId || task.id),
    progress: Number(payload?.progress || 0),
    message: String(payload?.message || task.message || ''),
    resumed: Boolean(payload?.resumed || task.resumed),
    outputPath: String(payload?.output_path || task.outputPath || ''),
    segmentJsonPath: String(payload?.segment_json_path || task.segmentJsonPath || ''),
    status: nextStatus,
  })
}

const mergeLoadedTasks = (savedTasks) => {
  const existing = new Map(tasks.value.map((task) => [task.taskId || task.id, task]))
  const loaded = savedTasks.map((task, index) => {
    const current = existing.get(task.task_id)
    return {
      ...(current || createTask(task.file_path)),
      id: current?.id || task.task_id || `saved-${index}`,
      taskId: task.task_id,
      filePath: task.file_path,
      fileName: task.file_name || basename(task.file_path),
      title: task.title || '',
      outputDir: task.output_dir || '',
      outputName: task.output_name || '',
      ttsModel: task.tts_model || DEFAULT_MODEL,
      ttsTokens: task.tts_tokens || DEFAULT_TOKENS,
      ttsModeHint: task.tts_mode_hint || inferModeHint(task.tts_model || ''),
      speakerId: String(task.speaker_id || '0'),
      speed: Number(task.speed || 1),
      executionProvider: task.execution_provider || 'cpu',
      splitByToc: !!task.split_by_toc,
      maxChunkChars: Number(task.max_chunk_chars || 180),
      status: task.status || 'paused',
      message: task.message || '检测到未完成任务，可继续转换',
      outputPath: task.output_path || '',
      segmentJsonPath: task.segment_json_path || '',
      progress: Number(task.progress || 0),
      resumed: true,
      chapterCount: Number(task.chapter_count || 0),
      chapterOutputPaths: Array.isArray(task.chapter_output_paths) ? task.chapter_output_paths : [],
      stats: {
        total_chunks: Number(task.total_chunks || 0),
        completed_chunks: Number(task.completed_chunks || 0),
      },
    }
  })
  const loadedIds = new Set(loaded.map((task) => task.taskId || task.id))
  const rest = tasks.value.filter((task) => !loadedIds.has(task.taskId || task.id))
  tasks.value = [...loaded, ...rest]
}

const loadSavedTasks = async () => {
  try {
    const saved = await invoke('tts_book_list_saved_tasks')
    if (Array.isArray(saved) && saved.length) {
      mergeLoadedTasks(saved)
    }
  } catch (e) {
    logger.error('loadSavedTasks failed:', e)
  }
}

const handleAudioMetadata = async () => {
  if (!audioRef.value) return
  let subtitleJsonPath = currentSubtitleJsonPath.value
  if (!subtitleJsonPath && currentPlayingTaskId.value) {
    const task = tasks.value.find((item) => item.id === currentPlayingTaskId.value || item.taskId === currentPlayingTaskId.value)
    subtitleJsonPath = task?.segmentJsonPath || ''
  }
  if (!subtitleJsonPath) return
  try {
    const manifest = await readJsonFile(subtitleJsonPath)
    subtitleTimeline.value = buildSubtitleTimeline(manifest?.chunks || [], Number(audioRef.value.duration || 0))
    currentSubtitleIndex.value = subtitleTimeline.value.length ? 0 : -1
  } catch (e) {
    logger.warn('handleAudioMetadata subtitles failed:', e)
  }
}

const handleAudioTimeUpdate = () => {
  const currentTime = Number(audioRef.value?.currentTime || 0)
  const index = subtitleTimeline.value.findIndex((item) => currentTime >= item.start && currentTime < item.end)
  if (index >= 0) currentSubtitleIndex.value = index
}

const handleAudioEnded = () => {
  if (subtitleTimeline.value.length) currentSubtitleIndex.value = subtitleTimeline.value.length - 1
}

const seekToSubtitle = (index) => {
  const item = subtitleTimeline.value[index]
  if (!item || !audioRef.value) return
  audioRef.value.currentTime = item.start
  currentSubtitleIndex.value = index
  audioRef.value.play().catch((e) => logger.warn('seekToSubtitle play failed:', e))
}

const playTaskAudio = async (task) => {
  if (!task?.outputPath) return
  await loadPlayerSources({
    audioPath: task.outputPath,
    jsonPath: task.segmentJsonPath || '',
    title: task.fileName || basename(task.outputPath),
    taskId: task.id,
  })
}

const loadPlayerSources = async ({ audioPath, jsonPath = '', title = '', taskId = '' }) => {
  try {
    currentAudioSrc.value = convertFileSrc(audioPath)
    currentPlayingTaskId.value = taskId
    currentSubtitleJsonPath.value = jsonPath
    playerTitle.value = title || basename(audioPath)
    currentSubtitleIndex.value = -1
    subtitleTimeline.value = []
    await nextTick()
    if (audioRef.value) {
      audioRef.value.load()
      await audioRef.value.play()
    }

    const candidateJsonPaths = jsonPath ? [jsonPath] : findSiblingJsonPath(audioPath)
    for (const candidate of candidateJsonPaths) {
      try {
        const manifest = await readJsonFile(candidate)
        subtitleTimeline.value = buildSubtitleTimeline(manifest?.chunks || [], Number(audioRef.value?.duration || 0))
        if (subtitleTimeline.value.length) break
      } catch (e) {
        logger.warn('loadPlayerSources skip json:', candidate, e)
      }
    }
  } catch (e) {
    logger.error('loadPlayerSources failed:', e)
    message.error('加载音频失败')
  }
}

const pickPlayerMedia = async () => {
  try {
    const pickedAudio = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Audio Files', extensions: ['wav', 'mp3', 'flac', 'ogg', 'm4a'] }],
    })
    if (typeof pickedAudio !== 'string' || !pickedAudio.trim()) return

    const pickedJson = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'JSON Files', extensions: ['json'] }],
    })

    await loadPlayerSources({
      audioPath: pickedAudio,
      jsonPath: typeof pickedJson === 'string' ? pickedJson.trim() : '',
      title: basename(pickedAudio),
      taskId: '',
    })
  } catch (e) {
    logger.error('pickPlayerMedia failed:', e)
    message.error('选择音频或字幕文件失败')
  }
}

onMounted(async () => {
  await loadTtsConfig()
  await loadSavedTasks()
  unlistenTtsBookProgress = await listen('tts_book_progress', (event) => applyTaskProgress(event.payload || {}))
})

onBeforeUnmount(() => {
  if (typeof unlistenTtsBookProgress === 'function') {
    unlistenTtsBookProgress()
    unlistenTtsBookProgress = null
  }
})

const enqueueSelectedFiles = () => {
  const incoming = selectedFiles.value
    .map((file) => String(file || '').trim())
    .filter(Boolean)
    .filter((file) => !tasks.value.some((task) => task.filePath === file && ['queued', 'running', 'paused'].includes(task.status)))
    .map((file) => createTask(file))
  if (incoming.length) {
    tasks.value = [...incoming, ...tasks.value]
  }
  selectedFiles.value = []
}

const buildTaskRequest = (task, totalCount) => {
  const targetOutputDir = String(task.outputDir || '').trim() || dirname(task.filePath)
  if (!targetOutputDir) {
    throw new Error('无法确定输出目录，请手动选择输出目录')
  }
  return {
    task_id: task.taskId || task.id,
    file_path: task.filePath,
    output_dir: targetOutputDir,
    output_name: totalCount === 1 ? String(task.outputName || '').trim() : String(task.outputName || '').trim(),
    max_chunk_chars: Number(task.maxChunkChars || 180),
    split_by_toc: !!task.splitByToc,
    tts_model: task.ttsModel || DEFAULT_MODEL,
    tts_tokens: task.ttsTokens || DEFAULT_TOKENS,
    tts_mode_hint: task.ttsModeHint || inferModeHint(task.ttsModel || DEFAULT_MODEL),
    speaker_id: String(task.speakerId || '0'),
    speed: Number(task.speed || 1),
    execution_provider: String(task.executionProvider || 'cpu'),
  }
}

const startConvertQueue = async () => {
  if (!canStart.value) return
  enqueueSelectedFiles()
  const queue = tasks.value.filter((task) => ['queued', 'paused'].includes(task.status))
  if (!queue.length) return

  running.value = true
  let successCount = 0
  let failedCount = 0
  let pausedCount = 0
  let stoppedCount = 0

  for (let i = 0; i < queue.length; i += 1) {
    const task = tasks.value.find((item) => (item.taskId || item.id) === (queue[i].taskId || queue[i].id))
    if (!task || !['queued', 'paused'].includes(task.status)) continue
    currentRunningTaskId.value = task.taskId || task.id
    updateTask(task.id, {
      status: 'running',
      resumed: task.resumed || task.status === 'paused' || Number(task.progress || 0) > 0,
      message: `正在处理（${i + 1}/${queue.length}）：解析文档并进行语音合成...`,
    })

    try {
      const response = await invoke('tts_book_convert', {
        request: buildTaskRequest(task, queue.length),
      })
      updateTask(task.id, {
        status: 'success',
        taskId: String(response?.task_id || task.taskId || task.id),
        progress: 100,
        title: String(response?.title || task.title || ''),
        outputPath: String(response?.output_path || ''),
        segmentJsonPath: String(response?.segment_json_path || ''),
        chapterCount: Number(response?.chapter_count || 0),
        chapterOutputPaths: Array.isArray(response?.chapter_output_paths) ? response.chapter_output_paths : [],
        stats: response || null,
        message: Number(response?.chapter_count || 0) > 0 ? '转换完成，已输出音频、分段 JSON 和章节文件' : '转换完成，已输出音频和分段 JSON',
      })
      successCount += 1
    } catch (e) {
      const text = String(e?.message || e || '转换失败')
      logger.error('startConvertQueue failed:', e)
      if (text.includes('__TTS_BOOK_PAUSED__')) {
        updateTask(task.id, { status: 'paused', message: '任务已暂停，可稍后继续' })
        pausedCount += 1
      } else if (text.includes('__TTS_BOOK_STOPPED__')) {
        updateTask(task.id, { status: 'stopped', message: '任务已终止' })
        stoppedCount += 1
      } else {
        updateTask(task.id, { status: 'error', progress: Math.max(Number(task.progress || 0), 0), message: text })
        failedCount += 1
      }
    } finally {
      currentRunningTaskId.value = ''
    }
  }

  running.value = false
  if (successCount && !failedCount && !pausedCount && !stoppedCount) {
    message.success(`转换完成：成功 ${successCount} / ${queue.length}`)
  } else if (!successCount && failedCount && !pausedCount && !stoppedCount) {
    message.error(`转换失败：成功 0 / ${queue.length}`)
  } else {
    message.warning(`队列已处理：成功 ${successCount}，失败 ${failedCount}，暂停 ${pausedCount}，终止 ${stoppedCount}`)
  }
}

const pauseTask = async (task) => {
  if (task.status === 'running') {
    try {
      await invoke('tts_book_pause_task', { taskId: task.taskId || task.id })
      updateTask(task.id, { message: '已请求暂停，当前分段完成后会暂停' })
    } catch (e) {
      logger.error('pauseTask failed:', e)
      message.error('暂停任务失败')
    }
    return
  }
  if (task.status === 'queued') {
    updateTask(task.id, { status: 'paused', message: '任务已暂停，等待继续' })
  }
}

const stopTask = async (task) => {
  if (task.status === 'running') {
    try {
      await invoke('tts_book_stop_task', { taskId: task.taskId || task.id })
      updateTask(task.id, { message: '已请求终止，当前分段完成后会停止' })
    } catch (e) {
      logger.error('stopTask failed:', e)
      message.error('终止任务失败')
    }
    return
  }
  updateTask(task.id, { status: 'stopped', message: '任务已终止' })
}

const resumeTask = async (task) => {
  updateTask(task.id, { status: 'queued', message: '等待继续处理...' })
  if (!running.value) {
    await startConvertQueue()
  }
}

const pauseAllTasks = async () => {
  const activeIds = tasks.value
    .filter((task) => task.status === 'running')
    .map((task) => task.taskId || task.id)
  const queuedIds = tasks.value.filter((task) => task.status === 'queued').map((task) => task.id)
  queuedIds.forEach((id) => updateTask(id, { status: 'paused', message: '任务已暂停，等待继续' }))
  if (!activeIds.length) return
  try {
    await invoke('tts_book_pause_all_tasks', { taskIds: activeIds })
    activeIds.forEach((id) => updateTask(id, { message: '已请求暂停，当前分段完成后会暂停' }))
  } catch (e) {
    logger.error('pauseAllTasks failed:', e)
    message.error('暂停全部任务失败')
  }
}

const stopAllTasks = async () => {
  const activeIds = tasks.value
    .filter((task) => task.status === 'running')
    .map((task) => task.taskId || task.id)
  tasks.value
    .filter((task) => ['queued', 'paused', 'error'].includes(task.status))
    .forEach((task) => updateTask(task.id, { status: 'stopped', message: '任务已终止' }))
  if (!activeIds.length) return
  try {
    await invoke('tts_book_stop_all_tasks', { taskIds: activeIds })
    activeIds.forEach((id) => updateTask(id, { message: '已请求终止，当前分段完成后会停止' }))
  } catch (e) {
    logger.error('stopAllTasks failed:', e)
    message.error('停止全部任务失败')
  }
}

const openOutput = async (path) => {
  try {
    await invoke('open_in_file_explorer', { path })
  } catch (e) {
    logger.error('openOutput failed:', e)
    message.error('打开目录失败')
  }
}
</script>

<style scoped>
.tts-book-root {
  --tts-bg-1: #edf6ff;
  --tts-bg-2: #f5fbf6;
  --tts-panel: rgba(255, 255, 255, 0.9);
  --tts-panel-strong: rgba(255, 255, 255, 0.96);
  --tts-border: rgba(39, 83, 122, 0.12);
  --tts-title: #173857;
  --tts-text: #2a4760;
  --tts-muted: #6d8496;
  --tts-stat: rgba(255, 255, 255, 0.72);
  --tts-success: #198754;
  --tts-error: #c0384c;
  --tts-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Microsoft YaHei', 'Courier New', monospace;
  height: 100%;
  min-height: 0;
  padding: 14px;
  box-sizing: border-box;
  overflow: hidden;
  font-family: var(--tts-font-family);
  background:
    radial-gradient(circle at 0% 0%, rgba(123, 196, 255, 0.2), transparent 32%),
    radial-gradient(circle at 100% 0%, rgba(128, 224, 170, 0.16), transparent 28%),
    linear-gradient(135deg, var(--tts-bg-1), var(--tts-bg-2));
}

.tts-header {
  margin-bottom: 14px;
  padding: 12px 4px 4px;
}

.eyebrow {
  margin: 0 0 6px;
  font-size: 11px;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--tts-muted);
}

.page-title {
  margin: 0;
  font-size: 28px;
  line-height: 1.12;
  color: var(--tts-title);
  font-weight: 700;
}

.tts-layout {
  display: grid;
  grid-template-columns: minmax(320px, 360px) minmax(520px, 1fr) minmax(320px, 360px);
  gap: 14px;
  align-items: stretch;
  min-width: 1200px;
  height: calc(100% - 70px);
  min-height: 0;
}

.middle-column {
  display: grid;
  grid-template-rows: minmax(260px, 1fr) minmax(280px, 0.95fr);
  gap: 14px;
  min-height: 0;
}

.sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.tts-panel {
  min-height: 0;
  height: 100%;
  border: 1px solid var(--tts-border);
  background: var(--tts-panel);
  backdrop-filter: blur(10px);
}

.panel-scroll,
.files-scroll,
.task-scroll,
.player-body {
  height: 100%;
  min-height: 0;
  overflow: auto;
}

.queue-layout {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  gap: 14px;
  height: 100%;
  min-height: 0;
}

.panel-block {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.block-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--tts-title);
}

.block-hint,
.option-tip,
.panel-extra,
.task-meta,
.file-meta {
  font-size: 12px;
  color: var(--tts-muted);
}

.toolbar-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
}

.options-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.option-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: 14px;
  border: 1px solid var(--tts-border);
  background: var(--tts-panel-strong);
}

.option-card-wide {
  grid-column: 1 / -1;
}

.option-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--tts-text);
}

.file-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
}

.file-card {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 12px;
  align-items: center;
  padding: 14px;
  border-radius: 16px;
  border: 1px solid var(--tts-border);
  background: linear-gradient(145deg, rgba(255, 255, 255, 0.92), rgba(244, 249, 255, 0.88));
}

.file-icon {
  width: 36px;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  color: #3e7ad8;
  background: rgba(92, 149, 233, 0.12);
}

.file-info,
.task-main {
  min-width: 0;
}

.file-name,
.task-name {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--tts-title);
  font-weight: 600;
}

.files-header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.player-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.player-header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.audio-element {
  width: 100%;
}

.subtitle-panel {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  gap: 12px;
  min-height: 0;
}

.subtitle-label {
  font-size: 12px;
  color: var(--tts-muted);
}

.subtitle-current {
  padding: 14px;
  border-radius: 14px;
  border: 1px solid var(--tts-border);
  background: var(--tts-panel-strong);
  color: var(--tts-title);
  /* 字体设置移至统一类 */
}

.subtitle-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: auto;
}

.subtitle-item {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr);
  gap: 10px;
  align-items: start;
  width: 100%;
  padding: 10px 12px;
  border: 1px solid var(--tts-border);
  border-radius: 12px;
  background: transparent;
  color: var(--tts-text);
  text-align: left;
  cursor: pointer;
  /* 添加统一字体设置 */
  font-family: var(--tts-font-family);
  font-size: 15px;
  line-height: 1.7;
}

.subtitle-item.active {
  background: rgba(77, 136, 229, 0.12);
  border-color: rgba(77, 136, 229, 0.28);
}

.subtitle-index {
  font-size: 12px;
  color: var(--tts-muted);
}

.subtitle-text {
  line-height: 1.6;
  /* 继承父元素的字体设置 */
  font-family: inherit;
  font-size: inherit;
  color: inherit;
}

/* 为.subtitle-current添加相同的字体设置 */
.subtitle-current {
  font-family: var(--tts-font-family);
  font-size: 15px;
  line-height: 1.7;
}

.stat-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.stat-card,
.progress-card {
  padding: 12px;
  border-radius: 14px;
  border: 1px solid var(--tts-border);
  background: var(--tts-stat);
}

.stat-label {
  font-size: 12px;
  color: var(--tts-muted);
}

.stat-value {
  margin-top: 6px;
  font-size: 28px;
  line-height: 1;
  font-weight: 700;
  color: var(--tts-title);
}

.stat-value.success {
  color: var(--tts-success);
}

.stat-value.error {
  color: var(--tts-error);
}

.queue-item {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.task-main {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-head {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.queue-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.tts-book-root :deep(.n-card) {
  border-radius: 18px;
  height: 100%;
  background: var(--tts-panel);
}

.tts-book-root :deep(.n-card > .n-card__content) {
  height: calc(100% - 52px);
  min-height: 0;
  background: transparent;
}

.tts-book-root :deep(.n-card-header__main),
.tts-book-root :deep(.n-button),
.tts-book-root :deep(.n-base-selection),
.tts-book-root :deep(.n-input),
.tts-book-root :deep(.n-input-number),
.tts-book-root :deep(.n-tag),
.tts-book-root :deep(.n-empty),
.tts-book-root :deep(.n-progress),
.tts-book-root :deep(.n-list),
.tts-book-root :deep(.n-list-item) {
  font-family: inherit;
}

.tts-book-root :deep(.n-input .n-input-wrapper),
.tts-book-root :deep(.n-input-number .n-input-wrapper),
.tts-book-root :deep(.n-base-selection) {
  background: rgba(255, 255, 255, 0.92);
}

.tts-book-root.tts-book-dark {
  --tts-bg-1: #101924;
  --tts-bg-2: #0e141d;
  --tts-panel: rgba(18, 27, 39, 0.9);
  --tts-panel-strong: rgba(18, 27, 39, 0.96);
  --tts-border: rgba(126, 158, 189, 0.18);
  --tts-title: #e7f0fb;
  --tts-text: #c7d5e5;
  --tts-muted: #8ea2b8;
  --tts-stat: rgba(14, 20, 29, 0.72);
}

.tts-book-root.tts-book-dark .file-card,
.tts-book-root.tts-book-dark .subtitle-current,
.tts-book-root.tts-book-dark .subtitle-item {
  background: linear-gradient(145deg, rgba(19, 30, 43, 0.98), rgba(14, 22, 32, 0.92));
}

.tts-book-root.tts-book-dark :deep(.n-card),
.tts-book-root.tts-book-dark :deep(.n-list),
.tts-book-root.tts-book-dark :deep(.n-list-item),
.tts-book-root.tts-book-dark :deep(.n-empty),
.tts-book-root.tts-book-dark :deep(.n-input .n-input-wrapper),
.tts-book-root.tts-book-dark :deep(.n-input-number .n-input-wrapper),
.tts-book-root.tts-book-dark :deep(.n-base-selection) {
  background: rgba(18, 27, 39, 0.94);
  color: #d7e5f5;
}

.tts-book-root.tts-book-dark :deep(.n-card-header__main),
.tts-book-root.tts-book-dark :deep(.n-empty__description),
.tts-book-root.tts-book-dark :deep(.n-input__input-el),
.tts-book-root.tts-book-dark :deep(.n-base-selection-label) {
  color: #d7e5f5;
}

.tts-book-root.tts-book-dark :deep(.n-input__placeholder),
.tts-book-root.tts-book-dark :deep(.n-base-selection-placeholder) {
  color: #8ea2b8;
}
</style>
