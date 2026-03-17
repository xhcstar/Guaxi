<template>
  <div class="tts-book-root">
    <n-grid :cols="1" :x-gap="14" :y-gap="14">
      <n-gi>
        <n-card title="有声书 / 配音工坊" size="small" class="tts-card">
          <n-space vertical :size="12">
            <n-alert type="info" :show-icon="false">
              支持 <b>Word(.docx)</b>、<b>PDF</b>、<b>Markdown</b>、<b>TXT</b>、<b>EPUB</b>。
              支持批量排队转换，并可按目录章节切分导出（完整音频 + 章节分轨）。
            </n-alert>

            <n-space align="center">
              <n-input :value="selectedSummary" placeholder="请选择待转换文件（支持多选）" readonly />
              <n-button tertiary @click="pickFiles">选择文件（多选）</n-button>
            </n-space>

            <n-card size="small" embedded :bordered="false" class="selected-files-card">
              <n-empty v-if="!selectedFiles.length" description="未选择文件" />
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

            <n-space align="center">
              <n-input v-model:value="outputDir" placeholder="导出目录（可选，默认文件所在目录）" clearable />
              <n-button tertiary @click="pickOutputDir">导出目录</n-button>
            </n-space>

            <n-space align="center">
              <n-input v-model:value="outputName" placeholder="输出名称（仅单文件时生效）" />
              <n-input-number v-model:value="maxChunkChars" :min="80" :max="420" :step="10" style="width: 180px">
                <template #prefix>分段长度</template>
              </n-input-number>
              <n-switch v-model:value="splitByToc">
                <template #checked>章节分轨</template>
                <template #unchecked>仅完整音频</template>
              </n-switch>
            </n-space>

            <n-space>
              <n-button type="primary" :loading="running" :disabled="!canStart" @click="startConvertQueue">
                开始转换
              </n-button>
              <n-button :disabled="running" @click="clearFinished">清理已完成任务</n-button>
            </n-space>
          </n-space>
        </n-card>
      </n-gi>

      <n-gi>
        <n-card title="转换任务队列" size="small" class="tts-card">
          <n-empty v-if="!tasks.length" description="暂无任务" />

          <n-list v-else bordered>
            <n-list-item v-for="task in tasks" :key="task.id">
              <div class="task-row">
                <div class="task-main">
                  <div class="task-head">
                    <n-tag size="small" :type="tagType(task.status)">{{ task.statusLabel }}</n-tag>
                    <span class="task-name">{{ task.fileName }}</span>
                  </div>
                  <div class="task-meta">{{ task.message }}</div>
                  <div v-if="task.outputPath" class="task-meta">完整音频：{{ task.outputPath }}</div>
                  <div v-if="task.chapterCount > 0" class="task-meta">章节分轨：{{ task.chapterCount }} 个文件</div>
                  <div v-if="task.stats" class="task-meta">
                    分段 {{ task.stats.total_chunks }}（中 {{ task.stats.zh_chunks }} / 英 {{ task.stats.en_chunks }}）
                  </div>
                </div>

                <n-space>
                  <n-button v-if="task.outputPath" size="tiny" tertiary @click="openOutput(task.outputPath)">
                    打开完整音频目录
                  </n-button>
                  <n-button
                    v-if="task.chapterOutputPaths?.length"
                    size="tiny"
                    tertiary
                    @click="openOutput(task.chapterOutputPaths[0])"
                  >
                    打开章节目录
                  </n-button>
                </n-space>
              </div>
            </n-list-item>
          </n-list>
        </n-card>
      </n-gi>
    </n-grid>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { createDiscreteApi } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('TTSBookApp')
const { message } = createDiscreteApi(['message'])

const selectedFiles = ref([])
const outputDir = ref('')
const outputName = ref('')
const maxChunkChars = ref(180)
const splitByToc = ref(true)
const running = ref(false)

const tasks = ref([])

const selectedSummary = computed(() => {
  if (!selectedFiles.value.length) return ''
  if (selectedFiles.value.length === 1) return selectedFiles.value[0]
  return `已选择 ${selectedFiles.value.length} 个文件`
})

const canStart = computed(() => {
  return selectedFiles.value.length > 0 && !running.value
})

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
    message.error('选择导出目录失败')
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
    chapterCount: 0,
    chapterOutputPaths: [],
    stats: null,
  }
}

const startConvertQueue = async () => {
  if (!canStart.value) return

  const queue = selectedFiles.value.map((p) => String(p || '').trim()).filter(Boolean).map((p) => createTask(p))
  if (!queue.length) return

  const targetOutputDir = String(outputDir.value || '').trim() || dirname(queue[0].filePath)
  if (!targetOutputDir) {
    message.error('无法确定导出目录，请手动选择导出目录')
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
      task.outputPath = String(response?.output_path || '')
      task.chapterCount = Number(response?.chapter_count || 0)
      task.chapterOutputPaths = Array.isArray(response?.chapter_output_paths) ? response.chapter_output_paths : []
      task.stats = response || null
      task.message = task.chapterCount > 0 ? '转换完成（含章节分轨）。' : '转换完成。'
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
      task.message = String(e?.message || e || '转换失败')
    }
  }

  running.value = false
  logger.info('startConvertQueue finished', {
    successCount,
    total: queue.length,
  })
  message.success(`转换完成：成功 ${successCount} / ${queue.length}`)
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
  height: 100%;
  min-height: 0;
  padding: 12px;
  box-sizing: border-box;
  overflow: auto;
}

.tts-card {
  border-radius: 14px;
}

.selected-files-card {
  max-height: 220px;
  overflow: auto;
}

.task-row {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 12px;
  align-items: start;
}

.task-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.task-name {
  font-weight: 600;
}

.task-meta {
  margin-top: 6px;
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  word-break: break-all;
}
</style>
