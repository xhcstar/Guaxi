<template>
  <div class="tts-book-root" :class="{ 'tts-book-dark': isDarkTheme }">
    <header class="tts-header">
      <div>
        <p class="eyebrow">TTS Studio</p>
        <h1 class="page-title">有声书转换</h1>
      </div>
    </header>

    <section class="tts-layout">
      <n-card title="输入与参数" size="small" class="tts-panel control-panel">
        <n-space vertical :size="16">
          <div class="panel-block">
            <div class="block-head">
              <div>
                <div class="block-label">源文件</div>
                <div class="block-hint">{{ selectedFiles.length ? `已打开 ${selectedFiles.length} 个文件` : '选择单个或多个待转换文件' }}</div>
              </div>
              <n-button circle tertiary @click="pickFiles">
                <template #icon>
                  <n-icon>
                    <FolderOpenOutline />
                  </n-icon>
                </template>
              </n-button>
            </div>
          </div>

          <div class="panel-block">
            <div class="block-label">输出设置</div>
            <div class="toolbar-row">
              <n-input v-model:value="outputDir" placeholder="输出目录，默认使用首个文件所在目录" clearable />
              <n-button tertiary @click="pickOutputDir">选择目录</n-button>
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
                <n-select
                  v-model:value="speakerId"
                  filterable
                  :options="speakerOptions"
                  placeholder="选择说话人"
                />
              </div>

              <div class="option-card">
                <div class="option-title">语速</div>
                <n-input-number
                  v-model:value="ttsSpeed"
                  :min="0.2"
                  :max="3"
                  :step="0.1"
                  :precision="1"
                  style="width: 100%"
                />
              </div>

              <div class="option-card">
                <div class="option-title">分段长度</div>
                <n-input-number
                  v-model:value="maxChunkChars"
                  :min="80"
                  :max="420"
                  :step="10"
                  style="width: 100%"
                />
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
            <n-button type="primary" :loading="running" :disabled="!canStart" @click="startConvertQueue">
              开始转换
            </n-button>
            <n-button :disabled="running" @click="clearFinished">清理完成任务</n-button>
          </n-space>
        </n-space>
      </n-card>

      <n-card title="已打开文件" size="small" class="tts-panel files-panel">
        <template #header-extra>
          <n-button tertiary size="small" @click="pickFiles">
            <template #icon>
              <n-icon>
                <FolderOpenOutline />
              </n-icon>
            </template>
            添加文件
          </n-button>
        </template>

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
            <n-button quaternary circle @click="removeSelected(file)">
              <template #icon>
                <n-icon>
                  <TrashOutline />
                </n-icon>
              </template>
            </n-button>
          </div>
        </div>
      </n-card>

      <aside class="sidebar">
        <n-card title="任务队列" size="small" class="tts-panel queue-panel">
          <template #header-extra>
            <span class="panel-extra">{{ totalTasks }} 项</span>
          </template>

          <n-space vertical :size="14">
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
                    <n-progress
                      v-if="task.status === 'running' || task.progress > 0 || task.status === 'success'"
                      type="line"
                      :percentage="task.progress"
                      :show-indicator="true"
                      :height="8"
                      :status="task.status === 'error' ? 'error' : task.status === 'success' ? 'success' : 'info'"
                    />
                    <div v-if="task.stats" class="task-meta">
                      分段 {{ task.stats.total_chunks }}，中文 {{ task.stats.zh_chunks }}，英文 {{ task.stats.en_chunks }}
                    </div>
                    <div v-if="task.chapterCount > 0" class="task-meta">章节音频 {{ task.chapterCount }} 个</div>
                  </div>

                  <div class="queue-actions">
                    <n-button v-if="task.outputPath" size="tiny" tertiary @click="openOutput(task.outputPath)">音频</n-button>
                    <n-button v-if="task.segmentJsonPath" size="tiny" tertiary @click="openOutput(task.segmentJsonPath)">JSON</n-button>
                    <n-button
                      v-if="task.chapterOutputPaths?.length"
                      size="tiny"
                      tertiary
                      @click="openOutput(task.chapterOutputPaths[0])"
                    >
                      章节
                    </n-button>
                  </div>
                </div>
              </n-list-item>
            </n-list>
          </n-space>
        </n-card>
      </aside>
    </section>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { DocumentTextOutline, FolderOpenOutline, TrashOutline } from '@vicons/ionicons5'
import { createLogger } from '../../../utils/logger'
import { useSettingsStore } from '../../../stores/settingsStore'

const logger = createLogger('TTSBookApp')
const { message } = createDiscreteApi(['message'])
const settingsStore = useSettingsStore()

const DEFAULT_TTS_EXE = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
const DEFAULT_MODEL = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
const DEFAULT_TOKENS = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'
const KOKORO_SPEAKER_NAMES = [
  'af_maple', 'af_sol', 'bf_vale', 'zf_001', 'zf_002', 'zf_003', 'zf_004', 'zf_005', 'zf_006', 'zf_007',
  'zf_008', 'zf_017', 'zf_018', 'zf_019', 'zf_021', 'zf_022', 'zf_023', 'zf_024', 'zf_026', 'zf_027',
  'zf_028', 'zf_032', 'zf_036', 'zf_038', 'zf_039', 'zf_040', 'zf_042', 'zf_043', 'zf_044', 'zf_046',
  'zf_047', 'zf_048', 'zf_049', 'zf_051', 'zf_059', 'zf_060', 'zf_067', 'zf_070', 'zf_071', 'zf_072',
  'zf_073', 'zf_074', 'zf_075', 'zf_076', 'zf_077', 'zf_078', 'zf_079', 'zf_083', 'zf_084', 'zf_085',
  'zf_086', 'zf_087', 'zf_088', 'zf_090', 'zf_092', 'zf_093', 'zf_094', 'zf_099', 'zm_009', 'zm_010',
  'zm_011', 'zm_012', 'zm_013', 'zm_014', 'zm_015', 'zm_016', 'zm_020', 'zm_025', 'zm_029', 'zm_030',
  'zm_031', 'zm_033', 'zm_034', 'zm_035', 'zm_037', 'zm_041', 'zm_045', 'zm_050', 'zm_052', 'zm_053',
  'zm_054', 'zm_055', 'zm_056', 'zm_057', 'zm_058', 'zm_061', 'zm_062', 'zm_063', 'zm_064', 'zm_065',
  'zm_066', 'zm_068', 'zm_069', 'zm_080', 'zm_081', 'zm_082', 'zm_089', 'zm_091', 'zm_095', 'zm_096',
  'zm_097', 'zm_098', 'zm_100',
]

const ttsConfig = ref({
  provider: 'sherpa-onnx',
  model: DEFAULT_MODEL,
  tokens: DEFAULT_TOKENS,
  exe: DEFAULT_TTS_EXE,
})

const selectedFiles = ref([])
const outputDir = ref('')
const outputName = ref('')
const maxChunkChars = ref(180)
const splitByToc = ref(true)
const modelPreset = ref('settings')
const speakerId = ref('0')
const ttsSpeed = ref(1)
const running = ref(false)
const tasks = ref([])
let unlistenTtsBookProgress = null

const isDarkTheme = computed(() => settingsStore.appearanceTheme === 'dark')

const totalTasks = computed(() => tasks.value.length)
const successTasks = computed(() => tasks.value.filter((t) => t.status === 'success').length)
const failedTasks = computed(() => tasks.value.filter((t) => t.status === 'error').length)
const progressPercent = computed(() => {
  if (!totalTasks.value) return 0
  const totalProgress = tasks.value.reduce((sum, task) => sum + Number(task.progress || 0), 0)
  return Math.round(totalProgress / totalTasks.value)
})

const canStart = computed(() => selectedFiles.value.length > 0 && !running.value)

const modelOptions = computed(() => [
  { label: '跟随全局设置', value: 'settings' },
  { label: 'Kokoro Multi Lang', value: 'kokoro_multi_lang' },
  { label: 'VITS Melo ZH/EN', value: 'vits_melo_zh_en' },
])

const speakerOptions = computed(() => {
  const resolved = resolveModelConfig()
  const isKokoro = String(resolved.modeHint || '').toLowerCase() === 'kokoro'

  if (isKokoro) {
    const groups = {
      chineseFemale: [],
      chineseMale: [],
      americanFemale: [],
      britishFemale: [],
      other: [],
    }

    KOKORO_SPEAKER_NAMES.forEach((name, index) => {
      const option = {
        label: `${index} · ${name}`,
        value: String(index),
      }

      if (name.startsWith('zf_')) {
        groups.chineseFemale.push(option)
      } else if (name.startsWith('zm_')) {
        groups.chineseMale.push(option)
      } else if (name.startsWith('af_')) {
        groups.americanFemale.push(option)
      } else if (name.startsWith('bf_')) {
        groups.britishFemale.push(option)
      } else {
        groups.other.push(option)
      }
    })

    return [
      {
        type: 'group',
        label: '中文女声',
        key: 'chinese-female',
        children: groups.chineseFemale,
      },
      {
        type: 'group',
        label: '中文男声',
        key: 'chinese-male',
        children: groups.chineseMale,
      },
      {
        type: 'group',
        label: '美式女声',
        key: 'american-female',
        children: groups.americanFemale,
      },
      {
        type: 'group',
        label: '英式女声',
        key: 'british-female',
        children: groups.britishFemale,
      },
      {
        type: 'group',
        label: '其他',
        key: 'other',
        children: groups.other,
      },
    ].filter((group) => group.children.length > 0)
  }

  return Array.from({ length: 16 }, (_, index) => ({
    label: `Speaker ${index}`,
    value: String(index),
  }))
})

const basename = (p) => String(p || '').split(/[\\/]/).pop() || 'unknown'

const dirname = (p) => {
  const normalized = String(p || '').trim().replace(/\\/g, '/')
  const i = normalized.lastIndexOf('/')
  return i > 0 ? normalized.slice(0, i) : ''
}

const inferModeHint = (model) => {
  const value = String(model || '').toLowerCase()
  if (value.includes('kokoro')) return 'kokoro'
  if (value.includes('vits')) return 'vits'
  return ''
}

const resolveModelConfig = () => {
  if (modelPreset.value === 'vits_melo_zh_en') {
    return {
      model: 'utils/Models/tts/vits-melo-tts-zh-en/model.onnx',
      tokens: 'utils/Models/tts/vits-melo-tts-zh-en/tokens.txt',
      modeHint: 'vits',
      label: 'VITS Melo ZH/EN',
    }
  }

  if (modelPreset.value === 'kokoro_multi_lang') {
    return {
      model: 'utils/Models/tts/kokoro_multi_lang/model.onnx',
      tokens: 'utils/Models/tts/kokoro_multi_lang/tokens.txt',
      modeHint: 'kokoro',
      label: 'Kokoro Multi Lang',
    }
  }

  return {
    model: String(ttsConfig.value.model || DEFAULT_MODEL),
    tokens: String(ttsConfig.value.tokens || DEFAULT_TOKENS),
    modeHint: inferModeHint(ttsConfig.value.model),
    label: '跟随全局设置',
  }
}

const resolvedModelLabel = computed(() => {
  const resolved = resolveModelConfig()
  const modelName = basename(resolved.model)
  return `${resolved.label} · ${modelName}`
})

const fileTypeLabel = (file) => {
  const ext = basename(file).split('.').pop()?.toLowerCase() || ''
  if (!ext) return '未知类型'
  return ext.toUpperCase()
}

const tagType = (status) => {
  if (status === 'queued') return 'default'
  if (status === 'running') return 'info'
  if (status === 'success') return 'success'
  if (status === 'error') return 'error'
  return 'default'
}

const taskStatusLabel = (task) => {
  if (task.status === 'queued') return '排队中'
  if (task.status === 'running') return task.resumed ? '继续处理中' : '处理中'
  if (task.status === 'success') return '已完成'
  if (task.status === 'error') return '失败'
  return '未知'
}

const updateTask = (taskId, patch) => {
  const index = tasks.value.findIndex(
    (item) => item.id === taskId || item.taskId === taskId || item.filePath === taskId,
  )
  if (index < 0) return
  tasks.value.splice(index, 1, {
    ...tasks.value[index],
    ...patch,
  })
}

const clearFinished = () => {
  tasks.value = tasks.value.filter((t) => t.status === 'running' || t.status === 'queued')
}

const createTask = (filePath) => {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`
  return {
    id,
    taskId: id,
    filePath,
    fileName: basename(filePath),
    status: 'queued',
    message: '等待处理...',
    outputPath: '',
    segmentJsonPath: '',
    chapterCount: 0,
    chapterOutputPaths: [],
    progress: 0,
    resumed: false,
    stats: null,
  }
}

const loadTtsConfig = async () => {
  try {
    const saved = await invoke('get_tts_config')
    ttsConfig.value = {
      provider: String(saved?.provider || 'sherpa-onnx'),
      model: String(saved?.model || DEFAULT_MODEL),
      tokens: String(saved?.tokens || DEFAULT_TOKENS),
      exe: String(saved?.exe || DEFAULT_TTS_EXE),
    }
  } catch (e) {
    logger.error('loadTtsConfig failed:', e)
  }
}

const pickFiles = async () => {
  try {
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [{ name: 'Book Files', extensions: ['pdf', 'docx', 'md', 'markdown', 'txt', 'epub'] }],
    })

    if (typeof picked === 'string' && picked.trim()) {
      selectedFiles.value = [picked]
      return
    }

    if (Array.isArray(picked)) {
      const normalized = picked.map((p) => String(p || '').trim()).filter(Boolean)
      selectedFiles.value = [...new Set(normalized)]
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
    if (typeof picked === 'string' && picked.trim()) {
      outputDir.value = picked
    }
  } catch (e) {
    logger.error('pickOutputDir failed:', e)
    message.error('选择输出目录失败')
  }
}

const applyTaskProgress = (payload) => {
  const task = tasks.value.find(
    (item) => item.id === payload?.task_id || item.taskId === payload?.task_id || item.filePath === payload?.file_path,
  )
  if (!task) return

  const nextStatus = payload?.failed ? 'error' : payload?.stage === 'completed' ? 'success' : 'running'
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

onMounted(async () => {
  await loadTtsConfig()
  unlistenTtsBookProgress = await listen('tts_book_progress', (event) => {
    applyTaskProgress(event.payload || {})
  })
})

onBeforeUnmount(() => {
  if (typeof unlistenTtsBookProgress === 'function') {
    unlistenTtsBookProgress()
    unlistenTtsBookProgress = null
  }
})

const startConvertQueue = async () => {
  if (!canStart.value) return

  const queue = selectedFiles.value
    .map((p) => String(p || '').trim())
    .filter(Boolean)
    .map((p) => createTask(p))
  if (!queue.length) return

  const targetOutputDir = String(outputDir.value || '').trim() || dirname(queue[0].filePath)
  if (!targetOutputDir) {
    message.error('无法确定输出目录，请手动选择输出目录')
    return
  }

  const resolvedModel = resolveModelConfig()
  const normalizedSpeakerId = String(speakerId.value || '').trim() || '0'
  const normalizedSpeed = Number(ttsSpeed.value || 1)

  tasks.value = [...queue, ...tasks.value]
  running.value = true

  logger.info('startConvertQueue begin', {
    fileCount: queue.length,
    outputDir: targetOutputDir,
    splitByToc: !!splitByToc.value,
    maxChunkChars: Number(maxChunkChars.value || 180),
    model: resolvedModel.model,
    speakerId: normalizedSpeakerId,
    speed: normalizedSpeed,
  })

  let successCount = 0
  let failedCount = 0

  for (let i = 0; i < queue.length; i += 1) {
    const task = queue[i]
    updateTask(task.id, {
      status: 'running',
      message: `正在处理（${i + 1}/${queue.length}）：解析文档并进行语音合成...`,
    })

    const taskStartAt = Date.now()

    try {
      const response = await invoke('tts_book_convert', {
        request: {
          task_id: task.id,
          file_path: task.filePath,
          output_dir: targetOutputDir,
          output_name: queue.length === 1 ? String(outputName.value || '').trim() : '',
          max_chunk_chars: Number(maxChunkChars.value || 180),
          split_by_toc: !!splitByToc.value,
          tts_model: resolvedModel.model,
          tts_tokens: resolvedModel.tokens,
          tts_mode_hint: resolvedModel.modeHint,
          speaker_id: normalizedSpeakerId,
          speed: normalizedSpeed,
        },
      })

      logger.info('startConvertQueue file done', {
        index: i + 1,
        total: queue.length,
        filePath: task.filePath,
        costMs: Date.now() - taskStartAt,
        outputPath: response?.output_path,
        totalChunks: response?.total_chunks,
        chapterCount: response?.chapter_count,
      })

      updateTask(task.id, {
        status: 'success',
        taskId: String(response?.task_id || task.id),
        progress: 100,
        outputPath: String(response?.output_path || ''),
        segmentJsonPath: String(response?.segment_json_path || ''),
        chapterCount: Number(response?.chapter_count || 0),
        chapterOutputPaths: Array.isArray(response?.chapter_output_paths) ? response.chapter_output_paths : [],
        stats: response || null,
        message: Number(response?.chapter_count || 0) > 0 ? '转换完成，已输出音频、分段 JSON 和章节文件' : '转换完成，已输出音频和分段 JSON',
      })
      successCount += 1
    } catch (e) {
      logger.error('startConvertQueue failed:', {
        index: i + 1,
        total: queue.length,
        filePath: task.filePath,
        costMs: Date.now() - taskStartAt,
        error: e,
      })

      updateTask(task.id, {
        status: 'error',
        progress: Math.max(Number(task.progress || 0), 0),
        message: String(e?.message || e || '转换失败'),
      })
      failedCount += 1
    }
  }

  running.value = false

  if (successCount === queue.length) {
    message.success(`转换完成：成功 ${successCount} / ${queue.length}`)
  } else if (successCount === 0) {
    message.error(`转换失败：成功 0 / ${queue.length}`)
  } else {
    message.warning(`部分任务失败：成功 ${successCount} / ${queue.length}，失败 ${failedCount}`)
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
  height: 100%;
  min-height: 0;
  padding: 14px;
  box-sizing: border-box;
  overflow: auto;
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
  grid-template-columns: minmax(320px, 360px) minmax(460px, 1fr) minmax(320px, 360px);
  gap: 14px;
  align-items: start;
  min-width: 1160px;
}

.tts-panel {
  background: var(--tts-panel);
  border: 1px solid var(--tts-border);
  backdrop-filter: blur(10px);
}

.control-panel,
.files-panel,
.queue-panel {
  min-height: 100%;
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 14px;
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

.file-info {
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

.task-list {
  max-height: calc(100vh - 270px);
  overflow: auto;
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
  min-width: 0;
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
  background: var(--tts-panel);
}

.tts-book-root :deep(.n-card > .n-card__content) {
  background: transparent;
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

.tts-book-root.tts-book-dark .file-card {
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

@media (max-width: 1360px) {
  .tts-layout {
    grid-template-columns: minmax(300px, 340px) minmax(420px, 1fr) minmax(300px, 340px);
    min-width: 1080px;
  }
}
</style>
