<template>
  <div class="tts-book-root" :class="{ 'tts-book-dark': isDarkTheme }">
    <section class="tts-hero">
      <div class="hero-copy">
        <p class="eyebrow">TTS Studio</p>
        <h1 class="hero-title">有声书转换</h1>
        <p class="hero-subtitle">从 PDF、Word、Markdown 到整本音频，用更顺手的三列工作台完成转换。</p>
      </div>
      <div class="hero-badges">
        <n-tag :bordered="false" round type="success">Rust PDF 解析</n-tag>
        <n-tag :bordered="false" round type="info">{{ running ? '处理中' : '待命' }}</n-tag>
      </div>
    </section>

    <section class="tts-layout">
      <n-card title="输入与参数" size="small" class="tts-panel control-panel">
        <n-space vertical :size="14">
          <n-alert type="info" :show-icon="false">
            支持 <b>PDF</b>、<b>Word(.docx)</b>、<b>Markdown</b>、<b>TXT</b>、<b>EPUB</b>，
            可按目录拆分章节音频。
          </n-alert>

          <div class="panel-block">
            <div class="block-label">源文件</div>
            <div class="toolbar-row">
              <n-input :value="selectedSummary" placeholder="请选择待转换文档，支持多选" readonly />
              <n-button tertiary @click="pickFiles">选择文件</n-button>
            </div>
          </div>

          <n-card size="small" embedded :bordered="false" class="selected-files-card">
            <n-empty v-if="!selectedFiles.length" description="还没有选择文件" />
            <n-list v-else hoverable clickable>
              <n-list-item v-for="file in selectedFiles" :key="file">
                <div class="task-row">
                  <div class="task-main">
                    <div class="task-head">
                      <span class="task-name">{{ basename(file) }}</span>
                    </div>
                    <div class="task-meta">{{ file }}</div>
                  </div>
                  <n-button size="tiny" quaternary @click="removeSelected(file)">移除</n-button>
                </div>
              </n-list-item>
            </n-list>
          </n-card>

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
              <div class="option-card">
                <div class="option-title">分段长度</div>
                <n-input-number v-model:value="maxChunkChars" :min="80" :max="420" :step="10" style="width: 100%" />
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

      <n-card title="任务队列" size="small" class="tts-panel queue-panel">
        <template #header-extra>
          <span class="panel-extra">{{ totalTasks }} 项</span>
        </template>

        <n-empty v-if="!tasks.length" description="暂无任务" />
        <n-list v-else bordered>
          <n-list-item v-for="task in tasks" :key="task.id">
            <div class="queue-item">
              <div class="task-main">
                <div class="task-head">
                  <n-tag size="small" :type="tagType(task.status)">{{ task.statusLabel }}</n-tag>
                  <span class="task-name">{{ task.fileName }}</span>
                </div>
                <div class="task-meta">{{ task.message }}</div>
                <n-progress
                  v-if="task.status === 'running' || task.progress > 0"
                  type="line"
                  :percentage="task.progress"
                  :show-indicator="true"
                  :height="8"
                  :status="task.status === 'error' ? 'error' : task.status === 'success' ? 'success' : 'info'"
                />
                <div v-if="task.outputPath" class="task-meta">完整音频：{{ task.outputPath }}</div>
                <div v-if="task.segmentJsonPath" class="task-meta">分段 JSON：{{ task.segmentJsonPath }}</div>
                <div v-if="task.chapterCount > 0" class="task-meta">章节文件：{{ task.chapterCount }} 个</div>
                <div v-if="task.stats" class="task-meta">
                  分段 {{ task.stats.total_chunks }}，中文 {{ task.stats.zh_chunks }}，英文 {{ task.stats.en_chunks }}
                </div>
              </div>

              <div class="queue-actions">
                <n-button v-if="task.outputPath" size="tiny" tertiary @click="openOutput(task.outputPath)">
                  打开音频
                </n-button>
                <n-button v-if="task.segmentJsonPath" size="tiny" tertiary @click="openOutput(task.segmentJsonPath)">
                  打开 JSON
                </n-button>
                <n-button
                  v-if="task.chapterOutputPaths?.length"
                  size="tiny"
                  tertiary
                  @click="openOutput(task.chapterOutputPaths[0])"
                >
                  打开章节
                </n-button>
              </div>
            </div>
          </n-list-item>
        </n-list>
      </n-card>

      <n-card title="进度与输出" size="small" class="tts-panel stats-panel">
        <n-space vertical :size="16">
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

          <div class="output-card">
            <div class="block-label">最近输出</div>
            <n-empty v-if="!latestOutputs.length" description="还没有输出文件" />
            <n-list v-else bordered>
              <n-list-item v-for="path in latestOutputs" :key="path">
                <div class="task-row">
                  <div class="task-main">
                    <div class="task-name">{{ basename(path) }}</div>
                    <div class="task-meta">{{ path }}</div>
                  </div>
                  <n-button size="tiny" tertiary @click="openOutput(path)">打开</n-button>
                </div>
              </n-list-item>
            </n-list>
          </div>
        </n-space>
      </n-card>
    </section>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createLogger } from '../../../utils/logger'
import { useSettingsStore } from '../../../stores/settingsStore'

const logger = createLogger('TTSBookApp')
const { message } = createDiscreteApi(['message'])
const settingsStore = useSettingsStore()

const selectedFiles = ref([])
const outputDir = ref('')
const outputName = ref('')
const maxChunkChars = ref(180)
const splitByToc = ref(true)
const running = ref(false)
const tasks = ref([])
let unlistenTtsBookProgress = null

const isDarkTheme = computed(() => settingsStore.appearanceTheme === 'dark')

const selectedSummary = computed(() => {
  if (!selectedFiles.value.length) return ''
  if (selectedFiles.value.length === 1) return selectedFiles.value[0]
  return `已选择 ${selectedFiles.value.length} 个文件`
})

const totalTasks = computed(() => tasks.value.length)
const successTasks = computed(() => tasks.value.filter((t) => t.status === 'success').length)
const failedTasks = computed(() => tasks.value.filter((t) => t.status === 'error').length)
const progressPercent = computed(() => {
  if (!totalTasks.value) return 0
  const totalProgress = tasks.value.reduce((sum, task) => sum + Number(task.progress || 0), 0)
  return Math.round(totalProgress / totalTasks.value)
})

const latestOutputs = computed(() => {
  return tasks.value
    .filter((t) => t.status === 'success')
    .flatMap((t) => [t.outputPath, t.segmentJsonPath, ...(t.chapterOutputPaths || [])].filter(Boolean))
    .slice(0, 6)
})

const canStart = computed(() => selectedFiles.value.length > 0 && !running.value)

const basename = (p) => String(p || '').split(/[\\/]/).pop() || 'unknown'

const dirname = (p) => {
  const normalized = String(p || '').trim().replace(/\\/g, '/')
  const i = normalized.lastIndexOf('/')
  return i > 0 ? normalized.slice(0, i) : ''
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

const tagType = (status) => {
  if (status === 'queued') return 'default'
  if (status === 'running') return 'info'
  if (status === 'success') return 'success'
  if (status === 'error') return 'error'
  return 'default'
}

const clearFinished = () => {
  tasks.value = tasks.value.filter((t) => t.status === 'running' || t.status === 'queued')
}

const createTask = (filePath) => {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`
  return {
    id,
    filePath,
    fileName: basename(filePath),
    status: 'queued',
    statusLabel: '排队中',
    message: '等待处理...',
    outputPath: '',
    segmentJsonPath: '',
    chapterCount: 0,
    chapterOutputPaths: [],
    progress: 0,
    resumed: false,
    taskId: id,
    stats: null,
  }
}

const applyTaskProgress = (payload) => {
  const task = tasks.value.find((item) => item.id === payload?.task_id || item.taskId === payload?.task_id || item.filePath === payload?.file_path)
  if (!task) return

  task.taskId = String(payload?.task_id || task.taskId || task.id)
  task.progress = Number(payload?.progress || 0)
  task.message = String(payload?.message || task.message || '')
  task.resumed = Boolean(payload?.resumed || task.resumed)
  task.outputPath = String(payload?.output_path || task.outputPath || '')
  task.segmentJsonPath = String(payload?.segment_json_path || task.segmentJsonPath || '')
  if (task.status !== 'success' && task.status !== 'error') {
    task.status = 'running'
    task.statusLabel = task.resumed ? '继续处理中' : '处理中'
  }
}

onMounted(async () => {
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

  const queue = selectedFiles.value.map((p) => String(p || '').trim()).filter(Boolean).map((p) => createTask(p))
  if (!queue.length) return

  const targetOutputDir = String(outputDir.value || '').trim() || dirname(queue[0].filePath)
  if (!targetOutputDir) {
    message.error('无法确定输出目录，请手动选择输出目录')
    return
  }

  tasks.value = [...queue, ...tasks.value]
  running.value = true

  logger.info('startConvertQueue begin', {
    fileCount: queue.length,
    outputDir: targetOutputDir,
    splitByToc: !!splitByToc.value,
    maxChunkChars: Number(maxChunkChars.value || 180),
  })

  let successCount = 0
  let failedCount = 0
  for (let i = 0; i < queue.length; i += 1) {
    const task = queue[i]
    task.status = 'running'
    task.statusLabel = '处理中'
    task.message = `正在处理（${i + 1}/${queue.length}）：解析文档并进行语音合成...`

    const taskStartAt = Date.now()
    logger.info('startConvertQueue file start', {
      index: i + 1,
      total: queue.length,
      filePath: task.filePath,
    })

    try {
      const response = await invoke('tts_book_convert', {
        request: {
          task_id: task.id,
          file_path: task.filePath,
          output_dir: targetOutputDir,
          output_name: queue.length === 1 ? String(outputName.value || '').trim() : '',
          max_chunk_chars: Number(maxChunkChars.value || 180),
          split_by_toc: !!splitByToc.value,
        },
      })

      logger.info('startConvertQueue file done', {
        index: i + 1,
        total: queue.length,
        filePath: task.filePath,
        costMs: Date.now() - taskStartAt,
        outputPath: response?.output_path,
        totalChunks: response?.total_chunks,
        zhChunks: response?.zh_chunks,
        enChunks: response?.en_chunks,
        chapterCount: response?.chapter_count,
      })

      task.status = 'success'
      task.statusLabel = '已完成'
      task.taskId = String(response?.task_id || task.id)
      task.progress = 100
      task.outputPath = String(response?.output_path || '')
      task.segmentJsonPath = String(response?.segment_json_path || '')
      task.chapterCount = Number(response?.chapter_count || 0)
      task.chapterOutputPaths = Array.isArray(response?.chapter_output_paths) ? response.chapter_output_paths : []
      task.stats = response || null
      task.message = task.chapterCount > 0 ? '转换完成，已生成章节拆分音频' : '转换完成'
      successCount += 1
    } catch (e) {
      logger.error('startConvertQueue failed:', {
        index: i + 1,
        total: queue.length,
        filePath: task.filePath,
        costMs: Date.now() - taskStartAt,
        error: e,
      })
      task.status = 'error'
      task.statusLabel = '失败'
      task.progress = Math.max(Number(task.progress || 0), 0)
      task.message = String(e?.message || e || '转换失败')
      failedCount += 1
    }
  }

  running.value = false
  logger.info('startConvertQueue finished', {
    successCount,
    total: queue.length,
  })
  if (successCount === queue.length) {
    message.success(`转换完成：成功 ${successCount} / ${queue.length}`)
  } else if (successCount === 0) {
    message.error(`转换失败：0 / ${queue.length} 成功`)
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
  --tts-bg-1: #eff6ff;
  --tts-bg-2: #f8fff8;
  --tts-panel: rgba(255, 255, 255, 0.9);
  --tts-panel-strong: rgba(255, 255, 255, 0.96);
  --tts-border: rgba(39, 83, 122, 0.12);
  --tts-title: #183b60;
  --tts-text: #26445f;
  --tts-muted: #6b859d;
  --tts-stat: #ffffff;
  --tts-success: #198754;
  --tts-error: #c0384c;
  height: 100%;
  min-height: 0;
  padding: 14px;
  box-sizing: border-box;
  overflow: auto;
  background:
    radial-gradient(circle at 0% 0%, rgba(123, 196, 255, 0.2), transparent 30%),
    radial-gradient(circle at 100% 0%, rgba(128, 224, 170, 0.18), transparent 28%),
    linear-gradient(135deg, var(--tts-bg-1), var(--tts-bg-2));
}

.tts-hero {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  align-items: flex-start;
  margin-bottom: 14px;
  padding: 18px 20px;
  border: 1px solid var(--tts-border);
  border-radius: 18px;
  background: linear-gradient(145deg, rgba(255, 255, 255, 0.92), rgba(245, 251, 255, 0.86));
  backdrop-filter: blur(8px);
}

.hero-copy {
  max-width: 720px;
}

.eyebrow {
  margin: 0 0 6px;
  font-size: 11px;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--tts-muted);
}

.hero-title {
  margin: 0;
  font-size: 28px;
  line-height: 1.1;
  color: var(--tts-title);
  font-weight: 700;
}

.hero-subtitle {
  margin: 8px 0 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--tts-text);
}

.hero-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: flex-end;
}

.tts-layout {
  display: grid;
  grid-template-columns: minmax(320px, 1.05fr) minmax(380px, 1.3fr) minmax(300px, 0.95fr);
  gap: 14px;
  align-items: start;
  min-width: 1240px;
}

.tts-panel {
  border-radius: 18px;
  border: 1px solid var(--tts-border);
  background: var(--tts-panel);
  box-shadow: 0 18px 40px rgba(35, 66, 102, 0.08);
}

.control-panel,
.queue-panel,
.stats-panel {
  min-width: 0;
}

.panel-extra,
.block-label,
.task-meta,
.stat-label {
  color: var(--tts-muted);
}

.panel-block {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.toolbar-row {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 10px;
}

.selected-files-card {
  max-height: 260px;
  overflow: auto;
  border-radius: 14px;
  background: var(--tts-panel-strong);
}

.options-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(160px, 0.9fr);
  gap: 10px;
}

.option-card,
.progress-card,
.output-card,
.stat-card {
  border: 1px solid var(--tts-border);
  border-radius: 14px;
  background: var(--tts-panel-strong);
}

.option-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}

.option-title {
  color: var(--tts-text);
  font-size: 13px;
  font-weight: 600;
}

.queue-item,
.task-row {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 12px;
  align-items: start;
}

.queue-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-main {
  min-width: 0;
}

.task-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.task-name {
  color: var(--tts-title);
  font-weight: 600;
  word-break: break-word;
}

.task-meta {
  margin-top: 6px;
  font-size: 12px;
  word-break: break-all;
}

.stat-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.stat-card {
  padding: 14px;
  background: linear-gradient(160deg, var(--tts-stat), rgba(245, 251, 255, 0.96));
}

.stat-value {
  margin-top: 6px;
  font-size: 24px;
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

.progress-card,
.output-card {
  padding: 14px;
}

@media (max-width: 900px) {
  .tts-layout {
    min-width: 0;
    grid-template-columns: 1fr;
  }
}

@media (max-width: 760px) {
  .tts-hero {
    flex-direction: column;
  }

  .toolbar-row,
  .options-grid,
  .queue-item,
  .task-row,
  .stat-grid {
    grid-template-columns: 1fr;
  }

  .queue-actions {
    flex-direction: row;
    flex-wrap: wrap;
  }
}

.tts-book-root.tts-book-dark {
  --tts-bg-1: #0f1823;
  --tts-bg-2: #121f1b;
  --tts-panel: rgba(15, 23, 34, 0.88);
  --tts-panel-strong: rgba(20, 30, 44, 0.96);
  --tts-border: rgba(126, 165, 205, 0.18);
  --tts-title: #eef6ff;
  --tts-text: #cedced;
  --tts-muted: #92a8be;
  --tts-stat: rgba(18, 27, 39, 0.98);
  --tts-success: #5ad28e;
  --tts-error: #ff758c;
}

.tts-book-root.tts-book-dark .tts-hero {
  background: linear-gradient(145deg, rgba(18, 28, 42, 0.95), rgba(16, 40, 37, 0.84));
}

.tts-book-root.tts-book-dark .tts-panel {
  box-shadow: 0 18px 40px rgba(0, 0, 0, 0.22);
}
</style>
