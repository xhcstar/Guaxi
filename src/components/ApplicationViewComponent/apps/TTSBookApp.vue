<template>
  <div class="tts-book-root" :class="{ 'tts-book-dark': isDarkTheme }">
    <!-- 顶栏承担全局读数与队列操作：原先这块是空的，指标却挤在侧栏里。 -->
    <header class="tts-header">
      <div class="brand">
        <n-icon size="17" class="brand-icon">
          <LibraryOutline />
        </n-icon>
        <h1 class="brand-title">有声书工坊</h1>
      </div>

      <div class="stat-strip">
        <span class="stat-chip">
          <b>{{ totalResources }}</b><em>资源</em>
        </span>
        <span class="stat-chip tone-idle">
          <b>{{ idleCount }}</b><em>待转换</em>
        </span>
        <span class="stat-chip tone-running">
          <b>{{ activeCount }}</b><em>进行中</em>
        </span>
        <span class="stat-chip tone-done">
          <b>{{ successCount }}</b><em>已完成</em>
        </span>
        <span class="stat-chip tone-error">
          <b>{{ failedCount }}</b><em>失败</em>
        </span>
      </div>

      <div class="header-meter">
        <n-progress type="line" :percentage="queuePercent" :height="6" :show-indicator="false" :status="queueStatus" />
        <span class="meter-label">{{ queuePercent }}%</span>
      </div>

      <div class="header-actions">
        <n-tooltip>
          <template #trigger>
            <n-button size="small" secondary @click="pickFiles">
              <template #icon>
                <n-icon>
                  <AddOutline />
                </n-icon>
              </template>
              导入
            </n-button>
          </template>
          导入电子书或文档（可多选）
        </n-tooltip>
        <n-tooltip>
          <template #trigger>
            <n-button size="small" type="primary" :loading="running" :disabled="!canStart" @click="startConvertQueue()">
              <template #icon>
                <n-icon>
                  <PlayCircleOutline />
                </n-icon>
              </template>
              开始转换
            </n-button>
          </template>
          把待转换的资源全部加入队列并开始
        </n-tooltip>
        <n-tooltip>
          <template #trigger>
            <n-button size="small" secondary circle :disabled="!canPauseAll" @click="pauseAllTasks">
              <template #icon>
                <n-icon>
                  <PauseOutline />
                </n-icon>
              </template>
            </n-button>
          </template>
          暂停全部
        </n-tooltip>
        <n-tooltip>
          <template #trigger>
            <n-button size="small" secondary circle :disabled="!canStopAll" @click="stopAllTasks">
              <template #icon>
                <n-icon>
                  <StopOutline />
                </n-icon>
              </template>
            </n-button>
          </template>
          停止全部
        </n-tooltip>
        <n-tooltip>
          <template #trigger>
            <n-button size="small" secondary circle :disabled="running" @click="clearFinished">
              <template #icon>
                <n-icon>
                  <TrashOutline />
                </n-icon>
              </template>
            </n-button>
          </template>
          清理已结束的任务
        </n-tooltip>
        <n-tooltip>
          <template #trigger>
            <n-button size="small" quaternary circle class="rail-toggle"
              :class="{ active: !paramsCollapsed }" @click="toggleParamsRail">
              <template #icon>
                <n-icon>
                  <OptionsOutline />
                </n-icon>
              </template>
            </n-button>
          </template>
          {{ paramsCollapsed ? '展开转换参数' : '收起转换参数' }}
        </n-tooltip>
      </div>
    </header>

    <section ref="layoutRef" class="tts-body" :style="layoutVars">
      <aside v-if="!paramsCollapsed" class="rail params-rail">
        <div class="rail-scroll">
          <div class="param-group">
            <div class="group-title">输出</div>
            <label class="field">
              <span class="field-label">输出目录</span>
              <div class="field-row">
                <n-input size="small" v-model:value="outputDir" placeholder="默认 tts_output" clearable />
                <n-tooltip>
                  <template #trigger>
                    <n-button size="small" tertiary @click="pickOutputDir">
                      <template #icon>
                        <n-icon>
                          <FolderOpenOutline />
                        </n-icon>
                      </template>
                    </n-button>
                  </template>
                  选择输出目录
                </n-tooltip>
              </div>
            </label>
            <label class="field">
              <span class="field-label">输出文件名</span>
              <n-input size="small" v-model:value="outputName" placeholder="仅单文件任务生效" />
            </label>
          </div>

          <div class="param-group">
            <div class="group-title">合成</div>
            <label class="field">
              <span class="field-label">模型</span>
              <n-select size="small" v-model:value="modelPreset" :options="modelOptions" />
              <span class="field-tip">{{ resolvedModelLabel }}</span>
            </label>
            <div class="field-pair">
              <label class="field">
                <span class="field-label">音色</span>
                <n-select size="small" filterable v-model:value="speakerId" :options="speakerOptions"
                  placeholder="选择说话人" />
              </label>
              <label class="field field-narrow">
                <span class="field-label">语速</span>
                <n-input-number size="small" v-model:value="ttsSpeed" :min="0.2" :max="3" :step="0.1"
                  :precision="1" />
              </label>
            </div>
            <div class="field-pair">
              <label class="field">
                <span class="field-label">分段长度</span>
                <n-input-number size="small" v-model:value="maxChunkChars" :min="80" :max="420" :step="10" />
              </label>
              <label class="field">
                <span class="field-label">引擎</span>
                <n-select size="small" v-model:value="executionProvider" :options="executionProviderOptions" />
              </label>
            </div>
            <div class="field-tip push-down">引擎「自动」= 有 CUDA 就用 GPU，否则回退 CPU</div>
            <label class="field field-inline">
              <span class="field-label">章节拆分</span>
              <n-switch size="small" v-model:value="splitByToc">
                <template #checked>开启</template>
                <template #unchecked>关闭</template>
              </n-switch>
            </label>
          </div>
        </div>
      </aside>

      <div v-if="!paramsCollapsed" class="col-resizer" title="拖动调整参数栏宽度"
        @pointerdown="startColumnResize('left', $event)">
        <div class="resizer-dot" />
      </div>

      <!-- 资源库：源文件、队列任务、已完成记录统一成一条生命周期，不再分三处显示。 -->
      <section class="library">
        <div class="lib-toolbar">
          <div class="lib-search">
            <n-input v-model:value="libraryQuery" size="small" clearable placeholder="搜索书名 / 文件名 / 源文件">
              <template #prefix>
                <n-icon>
                  <SearchOutline />
                </n-icon>
              </template>
            </n-input>
          </div>

          <div class="facet-row">
            <button v-for="item in facets" :key="item.value" type="button" class="facet"
              :class="{ on: facet === item.value }" @click="facet = item.value">
              {{ item.label }}
              <span class="facet-count">{{ item.count }}</span>
            </button>
          </div>

          <div class="lib-tools">
            <n-button-group size="small">
              <n-tooltip>
                <template #trigger>
                  <n-button :type="viewMode === 'list' ? 'primary' : 'default'" :secondary="viewMode !== 'list'"
                    @click="viewMode = 'list'">
                    <template #icon>
                      <n-icon>
                        <ListOutline />
                      </n-icon>
                    </template>
                  </n-button>
                </template>
                列表视图
              </n-tooltip>
              <n-tooltip>
                <template #trigger>
                  <n-button :type="viewMode === 'grid' ? 'primary' : 'default'" :secondary="viewMode !== 'grid'"
                    @click="viewMode = 'grid'">
                    <template #icon>
                      <n-icon>
                        <GridOutline />
                      </n-icon>
                    </template>
                  </n-button>
                </template>
                书架视图
              </n-tooltip>
            </n-button-group>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" secondary circle :loading="scanningRecords" @click="scanRecords">
                  <template #icon>
                    <n-icon>
                      <RefreshOutline />
                    </n-icon>
                  </template>
                </n-button>
              </template>
              扫描输出目录，把已有的转换成果导入成记录
            </n-tooltip>

            <n-tooltip>
              <template #trigger>
                <n-button size="small" secondary circle :disabled="!selectedFiles.length" @click="clearSelectedFiles">
                  <template #icon>
                    <n-icon>
                      <TrashOutline />
                    </n-icon>
                  </template>
                </n-button>
              </template>
              清空待转换的导入文件
            </n-tooltip>
          </div>
        </div>

        <div class="lib-scroll">
          <n-empty v-if="!filteredResources.length"
            :description="libraryQuery || facet !== 'all' ? '没有匹配的资源' : '还没有资源，先导入电子书或文档'" />

          <div v-else class="res-collection" :class="`view-${viewMode}`">
            <article v-for="res in filteredResources" :key="res.key" class="res-item" :class="`st-${res.status}`">
              <div class="res-format">
                <span class="format-text">{{ res.ext || '???' }}</span>
                <span v-if="res.chapterCount" class="format-count">{{ res.chapterCount }}章</span>
              </div>

              <div class="res-main">
                <div class="res-head">
                  <span class="res-title" :title="res.title">{{ res.title }}</span>
                  <span class="res-status">{{ statusLabel(res) }}</span>
                </div>
                <div class="res-file" :title="res.filePath">{{ res.fileName }}</div>
                <div v-if="res.meta" class="res-meta">{{ res.meta }}</div>
                <n-progress v-if="res.status === 'running' || res.status === 'paused' || res.status === 'queued'"
                  type="line" :percentage="res.progress" :show-indicator="false" :height="5" class="res-progress"
                  :status="res.status === 'paused' ? 'warning' : 'info'" />
              </div>

              <div class="res-actions">
                <!-- 待转换：还没进队列 -->
                <template v-if="res.status === 'idle'">
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary :disabled="running" @click="startSingle(res)">
                        <template #icon>
                          <n-icon>
                            <PlayOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    转换这一个
                  </n-tooltip>
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="removeSelected(res.filePath)">
                        <template #icon>
                          <n-icon>
                            <TrashOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    移除
                  </n-tooltip>
                </template>

                <!-- 队列中：排队 / 暂停 -->
                <template v-else-if="res.status === 'queued' || res.status === 'paused'">
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary :disabled="running" @click="resumeResource(res)">
                        <template #icon>
                          <n-icon>
                            <PlayOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    继续任务
                  </n-tooltip>
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="stopResource(res)">
                        <template #icon>
                          <n-icon>
                            <StopOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    终止任务
                  </n-tooltip>
                </template>

                <!-- 处理中 -->
                <template v-else-if="res.status === 'running'">
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="pauseResource(res)">
                        <template #icon>
                          <n-icon>
                            <PauseOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    暂停任务
                  </n-tooltip>
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="stopResource(res)">
                        <template #icon>
                          <n-icon>
                            <StopOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    终止任务
                  </n-tooltip>
                </template>

                <!-- 失败 / 已终止：可重试 -->
                <template v-else-if="res.status === 'error' || res.status === 'stopped'">
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary :disabled="running" @click="resumeResource(res)">
                        <template #icon>
                          <n-icon>
                            <RefreshOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    重新转换
                  </n-tooltip>
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary :disabled="running" @click="clearTasksByStatus">
                        <template #icon>
                          <n-icon>
                            <TrashOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    清理已结束的任务
                  </n-tooltip>
                </template>

                <!-- 已完成：播放 / 查看产物 / 改名 / 删除 -->
                <template v-else-if="res.status === 'success'">
                  <n-tooltip>
                    <template #trigger>
                      <n-button size="tiny" tertiary :disabled="!res.outputPath" @click="playResource(res)">
                        <template #icon>
                          <n-icon>
                            <PlayOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    在右侧播放器播放
                  </n-tooltip>
                  <n-tooltip v-if="res.segmentJsonPath">
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="openOutput(res.segmentJsonPath)">
                        <template #icon>
                          <n-icon>
                            <DocumentTextOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    打开分段 JSON
                  </n-tooltip>
                  <n-tooltip v-if="res.chapterOutputPaths.length">
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="openOutput(res.chapterOutputPaths[0])">
                        <template #icon>
                          <n-icon>
                            <AlbumsOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    打开章节音频位置
                  </n-tooltip>
                  <n-tooltip v-if="res.kind === 'record'">
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="renameRecord(res.record)">
                        <template #icon>
                          <n-icon>
                            <CreateOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    重命名
                  </n-tooltip>
                  <n-tooltip v-if="res.kind === 'record'">
                    <template #trigger>
                      <n-button size="tiny" tertiary @click="deleteRecord(res.record)">
                        <template #icon>
                          <n-icon>
                            <TrashOutline />
                          </n-icon>
                        </template>
                      </n-button>
                    </template>
                    删除记录或连同产物
                  </n-tooltip>
                </template>

                <n-tooltip v-if="res.filePath">
                  <template #trigger>
                    <n-button size="tiny" quaternary @click="openOutput(res.filePath)">
                      <template #icon>
                        <n-icon>
                          <FolderOpenOutline />
                        </n-icon>
                      </template>
                    </n-button>
                  </template>
                  在文件管理器中定位
                </n-tooltip>
              </div>
            </article>
          </div>
        </div>
      </section>

      <div class="col-resizer" title="拖动调整播放器宽度" @pointerdown="startColumnResize('right', $event)">
        <div class="resizer-dot" />
      </div>

      <aside class="rail player-rail">
        <header class="rail-head">
          <span class="rail-title">播放器</span>
          <span class="rail-sub">{{ playerTitle || '等待播放' }}</span>
          <n-tooltip>
            <template #trigger>
              <n-button size="tiny" tertiary circle @click="pickPlayerMedia">
                <template #icon>
                  <n-icon>
                    <FolderOpenOutline />
                  </n-icon>
                </template>
              </n-button>
            </template>
            选择音频和字幕 JSON
          </n-tooltip>
        </header>

        <div class="player-body">
          <n-empty v-if="!currentAudioSrc" description="点资源库里的播放按钮，音频会在这里播放" />
          <template v-else>
            <audio ref="audioRef" class="audio-element" :src="currentAudioSrc" controls preload="metadata"
              @loadedmetadata="handleAudioMetadata" @canplay="handleAudioCanPlay" @play="handleAudioPlay"
              @timeupdate="handleAudioTimeUpdate" @ended="handleAudioEnded" @error="handleAudioError" />

            <div class="subtitle-panel">
              <div class="current-subtitle">{{ currentSubtitleText }}</div>
              <div v-if="subtitleTimeline.length" class="subtitle-list">
                <button v-for="(item, index) in subtitleTimeline" :key="`${index}-${item.text}`" type="button"
                  class="subtitle-item" :class="{ active: index === currentSubtitleIndex }" @click="seekToSubtitle(index)">
                  <span class="subtitle-index">{{ index + 1 }}</span>
                  <span class="subtitle-text">{{ item.text }}</span>
                </button>
              </div>
              <div v-else class="subtitle-empty">该音频暂无分段字幕</div>
            </div>
          </template>
        </div>
      </aside>
    </section>
  </div>
</template>

<script setup>
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { createDiscreteApi, NInput } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { AddOutline, AlbumsOutline, CreateOutline, DocumentTextOutline, FolderOpenOutline, GridOutline, LibraryOutline, ListOutline, OptionsOutline, PauseOutline, PlayCircleOutline, PlayOutline, RefreshOutline, SearchOutline, StopOutline, TrashOutline } from '@vicons/ionicons5'
import { createLogger } from '../../../utils/logger'
import { useSettingsStore } from '../../../stores/settingsStore'
import { buildSpeakerOptions } from '../../../utils/ttsSpeakers'
import { readUiState, persistUiState, oneOf, within } from '../../../utils/uiState'
import { useTtsBookFiles } from '../../../composables/useTtsBookFiles'

const logger = createLogger('TTSBookApp')
const { message, dialog } = createDiscreteApi(['message', 'dialog'])
const settingsStore = useSettingsStore()

const DEFAULT_TTS_EXE = 'utils/sherpa-onnx/sherpa-onnx-offline-tts.exe'
const DEFAULT_MODEL = 'utils/Models/tts/kokoro_multi_lang/model.onnx'
const DEFAULT_TOKENS = 'utils/Models/tts/kokoro_multi_lang/tokens.txt'

// 本页的「上次选择」按项目既有惯例存 localStorage（见 utils/uiState.js）。
// 这些都是每次进来都得重填的参数，不记住很烦人；键名沿用 `guaxi_<页面>_<字段>`。
const UI = {
  outputName: 'guaxi_ttsbook_output_name',
  maxChunkChars: 'guaxi_ttsbook_max_chunk_chars',
  splitByToc: 'guaxi_ttsbook_split_by_toc',
  modelPreset: 'guaxi_ttsbook_model_preset',
  executionProvider: 'guaxi_ttsbook_execution_provider',
  speakerId: 'guaxi_ttsbook_speaker_id',
  speed: 'guaxi_ttsbook_speed',
  leftWidth: 'guaxi_ttsbook_left_width',
  rightWidth: 'guaxi_ttsbook_right_width',
  paramsCollapsed: 'guaxi_ttsbook_params_collapsed',
  facet: 'guaxi_ttsbook_facet',
  viewMode: 'guaxi_ttsbook_view_mode',
}

const ttsConfig = ref({ provider: 'sherpa-onnx', model: DEFAULT_MODEL, tokens: DEFAULT_TOKENS, exe: DEFAULT_TTS_EXE, outputDir: '' })
// 已打开文件走「本次运行内保留」（模块作用域），不要跨重启——见 useTtsBookFiles 的说明。
const selectedFiles = useTtsBookFiles()
const outputDir = ref('')
const outputName = ref(readUiState(UI.outputName, ''))
const maxChunkChars = ref(readUiState(UI.maxChunkChars, 180, within(80, 420)))
const splitByToc = ref(readUiState(UI.splitByToc, true))
// 模型/引擎用白名单校验：选项以后改过时，旧值不该被还原成一个已不存在的选项。
const modelPreset = ref(readUiState(UI.modelPreset, 'settings', oneOf('settings', 'kokoro_multi_lang', 'vits_melo_zh_en')))
const executionProvider = ref(readUiState(UI.executionProvider, 'auto', oneOf('auto', 'cpu', 'cuda')))
// null 表示本页还没记住过，由 loadTtsConfig 用全局默认补上。
const speakerId = ref(readUiState(UI.speakerId, null))
const ttsSpeed = ref(readUiState(UI.speed, 1, within(0.2, 3)))
const running = ref(false)
const tasks = ref([])
const records = ref([])
const scanningRecords = ref(false)
const audioRef = ref(null)
const currentAudioSrc = ref('')
const currentPlayingTaskId = ref('')
const currentSubtitleJsonPath = ref('')
const playerTitle = ref('')
const currentSubtitleIndex = ref(-1)
const subtitleTimeline = ref([])
const layoutRef = ref(null)
const leftColWidth = ref(readUiState(UI.leftWidth, 300, within(260, 620)))
const rightColWidth = ref(readUiState(UI.rightWidth, 380, within(280, 560)))
const paramsCollapsed = ref(readUiState(UI.paramsCollapsed, false))
const facet = ref(readUiState(UI.facet, 'all', oneOf('all', 'idle', 'running', 'success', 'error')))
const viewMode = ref(readUiState(UI.viewMode, 'list', oneOf('list', 'grid')))
const libraryQuery = ref('')
const dragState = ref(null)
let unlistenTtsBookProgress = null
let currentAudioObjectUrl = ''
let currentAudioPath = ''
let currentAudioSourceMode = ''
let currentAudioFallbackTried = false
let currentAudioFallbackInFlight = false

const isDarkTheme = computed(() => settingsStore.appearanceTheme === 'dark')

const modelOptions = computed(() => [
  { label: '跟随全局设置', value: 'settings' },
  { label: 'Kokoro Multi Lang', value: 'kokoro_multi_lang' },
  { label: 'VITS Melo ZH/EN', value: 'vits_melo_zh_en' },
])
const executionProviderOptions = [
  { label: '自动（有 GPU 就用 GPU）', value: 'auto' },
  { label: 'CPU', value: 'cpu' },
  { label: 'GPU / CUDA', value: 'cuda' },
]

// ───────────────────────── 统一资源模型 ─────────────────────────
// 源文件（待转换）、队列任务（进行中）、完成记录（已完成）在语义上是同一条
// 生命周期的三个阶段，之前被拆成三个列表，右栏「进行中 / 已完成」的分页
// 也因此重复。这里合流成一份 resources，列表只管渲染，视图切换交给 facet。
const extOf = (name) => String(name || '').split(/[\\/]/).pop()?.split('.').pop()?.toUpperCase() || ''

const recordResource = (record) => ({
  key: `record-${record.id}`,
  kind: 'record',
  record,
  taskId: String(record.id || ''),
  title: String(record.title || record.file_name || '未命名'),
  fileName: String(record.file_name || ''),
  filePath: String(record.source_file || ''),
  ext: extOf(record.file_name || record.source_file),
  status: 'success',
  progress: 100,
  outputPath: String(record.output_path || ''),
  segmentJsonPath: String(record.segment_json_path || ''),
  chapterOutputPaths: Array.isArray(record.chapter_output_paths) ? record.chapter_output_paths : [],
  chapterCount: Array.isArray(record.chapter_output_paths) ? record.chapter_output_paths.length : 0,
  meta: describeRecord(record),
})

const taskResource = (task) => ({
  key: `task-${task.taskId || task.id}`,
  kind: 'task',
  task,
  taskId: String(task.taskId || task.id),
  title: String(task.title || task.fileName || '未命名'),
  fileName: String(task.fileName || ''),
  filePath: String(task.filePath || ''),
  ext: extOf(task.fileName || task.filePath),
  status: task.status,
  progress: Number(task.progress || 0),
  outputPath: String(task.outputPath || ''),
  segmentJsonPath: String(task.segmentJsonPath || ''),
  chapterOutputPaths: Array.isArray(task.chapterOutputPaths) ? task.chapterOutputPaths : [],
  chapterCount: Number(task.chapterCount || 0),
  meta: describeTask(task),
})

const sourceResource = (file) => ({
  key: `source-${file}`,
  kind: 'source',
  taskId: '',
  title: basename(file),
  fileName: basename(file),
  filePath: String(file || ''),
  ext: extOf(file),
  status: 'idle',
  progress: 0,
  outputPath: '',
  segmentJsonPath: '',
  chapterOutputPaths: [],
  chapterCount: 0,
  meta: '待加入队列',
})

// 成功任务的展示交给完成记录（后端索引才是权威），但记录万一没读出来时
// 不能让结果凭空消失，所以按 taskId 去重后仍保留没有对应记录的那几条。
const resources = computed(() => {
  const list = []
  const indexedRecords = new Set(records.value.map((record) => String(record.id)))
  for (const task of tasks.value) {
    if (task.status === 'success' && indexedRecords.has(String(task.taskId || task.id))) continue
    list.push(taskResource(task))
  }
  for (const record of records.value) list.push(recordResource(record))
  for (const file of selectedFiles.value) list.push(sourceResource(file))
  const rank = { running: 0, queued: 1, paused: 2, error: 3, idle: 4, stopped: 5, success: 6 }
  return list.sort((a, b) => (rank[a.status] ?? 9) - (rank[b.status] ?? 9))
})

const matchesFacet = (res, value) => {
  if (value === 'all') return true
  if (value === 'idle') return res.status === 'idle'
  if (value === 'running') return ['running', 'queued', 'paused'].includes(res.status)
  if (value === 'success') return res.status === 'success'
  return ['error', 'stopped'].includes(res.status)
}

const filteredResources = computed(() => {
  const query = libraryQuery.value.trim().toLowerCase()
  return resources.value.filter((res) => {
    if (!matchesFacet(res, facet.value)) return false
    if (!query) return true
    return [res.title, res.fileName, res.filePath].some((field) => String(field || '').toLowerCase().includes(query))
  })
})

const countOf = (value) => resources.value.filter((res) => matchesFacet(res, value)).length
const facets = computed(() => [
  { value: 'all', label: '全部', count: resources.value.length },
  { value: 'idle', label: '待转换', count: countOf('idle') },
  { value: 'running', label: '进行中', count: countOf('running') },
  { value: 'success', label: '已完成', count: countOf('success') },
  { value: 'error', label: '失败', count: countOf('error') },
])

const totalResources = computed(() => resources.value.length)
const idleCount = computed(() => countOf('idle'))
const successCount = computed(() => countOf('success'))
const failedCount = computed(() => countOf('error'))
const activeCount = computed(() => countOf('running'))
// 进度只对「已入队」的资源算：刚导入还没开始的文件不该把完成度拉低。
const queuePercent = computed(() => {
  const pending = activeCount.value + failedCount.value + tasks.value.filter((t) => t.status === 'stopped').length
  const done = successCount.value
  const total = done + pending
  return total ? Math.round((done / total) * 100) : 0
})

const queueStatus = computed(() => {
  if (running.value) return 'info'
  if (failedCount.value) return 'error'
  return successCount.value && queuePercent.value === 100 ? 'success' : 'default'
})

const hasPendingTasks = computed(() => tasks.value.some((task) => ['queued', 'paused'].includes(task.status)))
const canStart = computed(() => !running.value && (selectedFiles.value.length > 0 || hasPendingTasks.value))
const canPauseAll = computed(() => tasks.value.some((task) => task.status === 'running' || task.status === 'queued'))
const canStopAll = computed(() => tasks.value.some((task) => ['running', 'queued', 'paused', 'error'].includes(task.status)))

const currentSubtitleText = computed(() => currentSubtitleIndex.value >= 0 ? (subtitleTimeline.value[currentSubtitleIndex.value]?.text || '等待字幕...') : '等待字幕...')

// 参数栏收起时把整条轨道从网格里摘掉，而不是留一列 0 宽度的空位。
const layoutVars = computed(() => ({
  '--left-col-width': `${Math.round(leftColWidth.value)}px`,
  '--right-col-width': `${Math.round(rightColWidth.value)}px`,
  '--body-columns': paramsCollapsed.value
    ? `minmax(520px, 1fr) 8px var(--right-col-width)`
    : `var(--left-col-width) 8px minmax(520px, 1fr) 8px var(--right-col-width)`,
}))

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

const speakerOptions = computed(() => buildSpeakerOptions(resolveModelConfig().model))

const basename = (p) => String(p || '').split(/[\\/]/).pop() || 'unknown'
const dirname = (p) => {
  const normalized = String(p || '').trim().replace(/\\/g, '/')
  const i = normalized.lastIndexOf('/')
  return i > 0 ? normalized.slice(0, i) : ''
}

const resolvedModelLabel = computed(() => `${resolveModelConfig().label} · ${basename(resolveModelConfig().model)}`)

const statusLabel = (res) => {
  if (res.status === 'idle') return '待转换'
  if (res.status === 'queued') return '排队中'
  if (res.status === 'running') return res.task?.resumed ? '继续处理中' : `处理中 ${res.progress}%`
  if (res.status === 'paused') return `已暂停 ${res.progress}%`
  if (res.status === 'stopped') return '已终止'
  if (res.status === 'success') return '已完成'
  if (res.status === 'error') return '失败'
  return '未知'
}

const updateTask = (taskId, patch) => {
  const index = tasks.value.findIndex((item) => item.id === taskId || item.taskId === taskId || item.filePath === taskId)
  if (index < 0) return
  tasks.value.splice(index, 1, { ...tasks.value[index], ...patch })
}

const findTask = (res) => tasks.value.find((item) => (item.taskId || item.id) === res.taskId)

const clearFinished = () => {
  tasks.value = tasks.value.filter((t) => ['running', 'queued', 'paused'].includes(t.status))
}

const clearTasksByStatus = clearFinished

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

const saveTtsOutputDirConfig = async (nextOutputDir) => {
  try {
    await invoke('set_tts_config', {
      ttsConfig: {
        provider: String(ttsConfig.value.provider || 'sherpa-onnx'),
        model: String(ttsConfig.value.model || DEFAULT_MODEL),
        tokens: String(ttsConfig.value.tokens || DEFAULT_TOKENS),
        exe: String(ttsConfig.value.exe || DEFAULT_TTS_EXE),
        output_dir: String(nextOutputDir || '').trim(),
        speaker_id: String(ttsConfig.value.speakerId || '0').trim() || '0',
      },
    })
  } catch (e) {
    logger.error('saveTtsOutputDirConfig failed:', e)
  }
}

const loadTtsConfig = async () => {
  try {
    const saved = await invoke('get_tts_config')
    const savedOutputDir = String(saved?.output_dir || '').trim()
    ttsConfig.value = {
      provider: String(saved?.provider || 'sherpa-onnx'),
      model: String(saved?.model || DEFAULT_MODEL),
      tokens: String(saved?.tokens || DEFAULT_TOKENS),
      exe: String(saved?.exe || DEFAULT_TTS_EXE),
      outputDir: savedOutputDir,
      // 记住全局音色：本页保存配置时会回写整份 ttsConfig，
      // 若不带这个字段就会把设置页选的音色覆盖成默认值。
      speakerId: String(saved?.speaker_id || '0').trim() || '0',
    }
    if (!String(outputDir.value || '').trim()) {
      outputDir.value = savedOutputDir
    }
    // 本页没记住过音色时才跟随设置页的全局默认；一旦在本页选过就以本页为准。
    if (speakerId.value === null) {
      speakerId.value = String(saved?.speaker_id || '').trim() || '0'
    }
  } catch (e) {
    logger.error('loadTtsConfig failed:', e)
  }
}

const readJsonFile = async (path) => {
  const content = await invoke('read_file_content', { path: String(path || '').trim() })
  return JSON.parse(String(content || ''))
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
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [
        { name: '电子书与文档', extensions: ['pdf', 'epub', 'mobi', 'azw3', 'docx', 'doc', 'md', 'markdown', 'txt'] },
        { name: '所有文件', extensions: ['*'] },
      ],
    })
    const incoming = (typeof picked === 'string' ? [picked] : Array.isArray(picked) ? picked : [])
      .map((p) => String(p || '').trim())
      .filter(Boolean)
    if (!incoming.length) return
    // 追加而不是替换：资源库是台账，「导入」就该往里加，清空另有入口。
    selectedFiles.value = [...new Set([...selectedFiles.value, ...incoming])]
    facet.value = 'idle'
  } catch (e) {
    logger.error('pickFiles failed:', e)
    message.error('选择文件失败')
  }
}

const removeSelected = (file) => {
  selectedFiles.value = selectedFiles.value.filter((p) => p !== file)
}

// 批量移除的入口：单个资源的删除仍在每张卡片上。
const clearSelectedFiles = () => {
  if (!selectedFiles.value.length) return
  dialog.warning({
    title: '清空待转换文件',
    content: `移除全部 ${selectedFiles.value.length} 个待转换文件？只清空这个列表，不会动磁盘上的文件。`,
    positiveText: '清空',
    negativeText: '取消',
    onPositiveClick: () => {
      selectedFiles.value = []
    },
  })
}

const pickOutputDir = async () => {
  try {
    const picked = await open({ multiple: false, directory: true })
    if (typeof picked === 'string' && picked.trim()) {
      outputDir.value = picked
      ttsConfig.value.outputDir = picked
      await saveTtsOutputDirConfig(picked)
    }
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

const base64ToUint8Array = (b64) => {
  const binary = atob(String(b64 || ''))
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

const revokeCurrentAudioObjectUrl = () => {
  if (!currentAudioObjectUrl) return
  try {
    URL.revokeObjectURL(currentAudioObjectUrl)
  } catch (e) {
    logger.warn('[TTSBookApp] revoke audio object url failed:', e)
  }
  currentAudioObjectUrl = ''
}

const getAudioElementState = () => ({
  networkState: audioRef.value?.networkState || 0,
  readyState: audioRef.value?.readyState || 0,
  currentSrcProtocol: String(audioRef.value?.currentSrc || '').split(':')[0],
  sourceMode: currentAudioSourceMode,
})

const guessAudioMimeType = (audioPath) => {
  const ext = String(audioPath || '').split(/[\\/]/).pop()?.split('.').pop()?.toLowerCase() || ''
  if (ext === 'mp3') return 'audio/mpeg'
  if (ext === 'flac') return 'audio/flac'
  if (ext === 'ogg') return 'audio/ogg'
  if (ext === 'm4a') return 'audio/mp4'
  return 'audio/wav'
}

const buildAudioBlobUrlFromPath = async (audioPath, reason = 'fallback') => {
  const base64 = await invoke('read_file_bytes_base64', { path: audioPath })
  const bytes = base64ToUint8Array(base64)
  const contentType = guessAudioMimeType(audioPath)
  const blob = new Blob([bytes], { type: contentType })
  const objectUrl = URL.createObjectURL(blob)
  logger.info('[TTSBookApp] 本地音频已转换为 Blob 播放源', {
    reason,
    audioPath,
    contentType,
    audioBytes: bytes.byteLength,
    objectUrlProtocol: objectUrl.split(':')[0],
  })
  return objectUrl
}

const fallbackToBlobAudioSource = async (reason = 'unknown', rawError = null) => {
  if (currentAudioFallbackTried || currentAudioFallbackInFlight || !currentAudioPath) return false
  currentAudioFallbackTried = true
  currentAudioFallbackInFlight = true
  logger.warn('[TTSBookApp] asset 音频播放失败，尝试 Blob fallback', {
    reason,
    audioPath: currentAudioPath,
    error: String(rawError?.message || rawError || ''),
    ...getAudioElementState(),
  })
  try {
    const objectUrl = await buildAudioBlobUrlFromPath(currentAudioPath, reason)
    revokeCurrentAudioObjectUrl()
    currentAudioObjectUrl = objectUrl
    currentAudioSourceMode = 'blob'
    currentAudioSrc.value = objectUrl
    await nextTick()
    if (audioRef.value) {
      audioRef.value.load()
      await audioRef.value.play()
    }
    return true
  } catch (e) {
    logger.error('[TTSBookApp] Blob fallback 播放失败', {
      reason,
      audioPath: currentAudioPath,
      error: String(e?.message || e || ''),
    })
    return false
  } finally {
    currentAudioFallbackInFlight = false
  }
}

const handleAudioMetadata = async () => {
  if (!audioRef.value) return
  logger.info('[TTSBookApp] 音频元数据已加载', {
    duration: Number(audioRef.value.duration || 0),
    audioPath: currentAudioPath,
    ...getAudioElementState(),
  })
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

const handleAudioCanPlay = () => {
  logger.info('[TTSBookApp] 音频可播放', {
    audioPath: currentAudioPath,
    ...getAudioElementState(),
  })
}

const handleAudioPlay = () => {
  logger.info('[TTSBookApp] 音频开始播放', {
    audioPath: currentAudioPath,
    ...getAudioElementState(),
  })
}

const handleAudioError = () => {
  const mediaError = audioRef.value?.error
  logger.error('[TTSBookApp] 音频元素加载失败', {
    audioPath: currentAudioPath,
    mediaErrorCode: mediaError?.code || 0,
    mediaErrorMessage: mediaError?.message || '',
    ...getAudioElementState(),
  })
  void fallbackToBlobAudioSource('audio-element-error')
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

const playResource = async (res) => {
  if (!res?.outputPath) return
  await loadPlayerSources({
    audioPath: res.outputPath,
    jsonPath: res.segmentJsonPath || '',
    title: res.title || basename(res.outputPath),
    taskId: res.taskId,
  })
}

const loadPlayerSources = async ({ audioPath, jsonPath = '', title = '', taskId = '' }) => {
  try {
    // 设计说明：TTSBook 的成品音频可能很大，优先使用 Tauri asset URL 交给 WebView 流式读取；
    // release 下若 CSP 或 asset scope 拒绝加载，再由播放器错误处理切到 Blob fallback。
    revokeCurrentAudioObjectUrl()
    currentAudioPath = String(audioPath || '').trim()
    currentAudioSourceMode = 'asset'
    currentAudioFallbackTried = false
    currentAudioFallbackInFlight = false
    const assetSrc = convertFileSrc(audioPath)
    currentAudioSrc.value = assetSrc
    currentPlayingTaskId.value = taskId
    currentSubtitleJsonPath.value = jsonPath
    playerTitle.value = title || basename(audioPath)
    currentSubtitleIndex.value = -1
    subtitleTimeline.value = []
    logger.info('[TTSBookApp] 准备加载播放器音频', {
      audioPath,
      jsonPath,
      taskId,
      sourceMode: currentAudioSourceMode,
      assetSrcProtocol: String(assetSrc || '').split(':')[0],
    })
    await nextTick()
    if (audioRef.value) {
      audioRef.value.load()
    }

    const candidateJsonPaths = jsonPath ? [jsonPath] : findSiblingJsonPath(audioPath)
    for (const candidate of candidateJsonPaths) {
      try {
        const manifest = await readJsonFile(candidate)
        subtitleTimeline.value = buildSubtitleTimeline(manifest?.chunks || [], Number(audioRef.value?.duration || 0))
        logger.info('[TTSBookApp] 字幕 JSON 已加载', {
          candidate,
          chunks: Array.isArray(manifest?.chunks) ? manifest.chunks.length : 0,
          timelineItems: subtitleTimeline.value.length,
          duration: Number(audioRef.value?.duration || 0),
        })
        if (subtitleTimeline.value.length) {
          currentSubtitleIndex.value = 0
          break
        }
      } catch (e) {
        logger.warn('loadPlayerSources skip json:', candidate, e)
      }
    }

    if (audioRef.value) {
      try {
        await audioRef.value.play()
      } catch (playError) {
        const recovered = await fallbackToBlobAudioSource('asset-play-rejected', playError)
        if (!recovered) throw playError
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

const clamp = (value, min, max) => Math.min(Math.max(value, min), max)

const stopDragging = () => {
  window.removeEventListener('pointermove', handleGlobalPointerMove)
  window.removeEventListener('pointerup', stopDragging)
  dragState.value = null
}

const handleGlobalPointerMove = (event) => {
  const state = dragState.value
  if (!state) return
  const containerWidth = Number(layoutRef.value?.clientWidth || 0)
  if (containerWidth <= 0) return
  const splitterWidth = 16
  const middleMin = 520
  const sideMin = 280
  const sideMax = 560
  if (state.type === 'left') {
    const rawLeft = state.startLeft + (event.clientX - state.startX)
    const maxByMiddle = containerWidth - rightColWidth.value - middleMin - splitterWidth * 2
    leftColWidth.value = clamp(rawLeft, sideMin, Math.max(sideMin, Math.min(sideMax, maxByMiddle)))
    return
  }
  const rawRight = state.startRight - (event.clientX - state.startX)
  const maxByMiddle = containerWidth - leftColWidth.value - middleMin - splitterWidth * 2
  rightColWidth.value = clamp(rawRight, sideMin, Math.max(sideMin, Math.min(sideMax, maxByMiddle)))
}

const startColumnResize = (type, event) => {
  event.preventDefault()
  dragState.value = {
    type,
    startX: event.clientX,
    startLeft: leftColWidth.value,
    startRight: rightColWidth.value,
  }
  window.addEventListener('pointermove', handleGlobalPointerMove)
  window.addEventListener('pointerup', stopDragging)
}

const toggleParamsRail = () => {
  paramsCollapsed.value = !paramsCollapsed.value
}

// 选择变化即落盘。放在所有被持久化的 ref 声明**之后**，否则会命中 TDZ。
for (const [key, source] of [
  [UI.outputName, outputName],
  [UI.maxChunkChars, maxChunkChars],
  [UI.splitByToc, splitByToc],
  [UI.modelPreset, modelPreset],
  [UI.executionProvider, executionProvider],
  [UI.speakerId, speakerId],
  [UI.speed, ttsSpeed],
  [UI.leftWidth, leftColWidth],
  [UI.rightColWidth, rightColWidth],
  [UI.paramsCollapsed, paramsCollapsed],
  [UI.facet, facet],
  [UI.viewMode, viewMode],
]) {
  watch(source, (value) => persistUiState(key, value))
}

onMounted(async () => {
  await loadTtsConfig()
  await loadSavedTasks()
  await loadRecords()
  unlistenTtsBookProgress = await listen('tts_book_progress', (event) => applyTaskProgress(event.payload || {}))
})

onBeforeUnmount(() => {
  stopDragging()
  revokeCurrentAudioObjectUrl()
  if (typeof unlistenTtsBookProgress === 'function') {
    unlistenTtsBookProgress()
    unlistenTtsBookProgress = null
  }
})

const enqueueSources = (files) => {
  const wanted = [...new Set(files.map((file) => String(file || '').trim()).filter(Boolean))]
  const incoming = wanted
    .filter((file) => !tasks.value.some((task) => task.filePath === file && ['queued', 'running', 'paused'].includes(task.status)))
    .map((file) => createTask(file))
  if (incoming.length) {
    tasks.value = [...incoming, ...tasks.value]
  }
  const taken = new Set(wanted)
  selectedFiles.value = selectedFiles.value.filter((file) => !taken.has(file))
}

const buildTaskRequest = (task, totalCount) => {
  const targetOutputDir = String(task.outputDir || '').trim() || String(ttsConfig.value.outputDir || '').trim() || dirname(task.filePath)
  if (!targetOutputDir) {
    throw new Error('无法确定输出目录，请手动选择输出目录')
  }
  return {
    task_id: task.taskId || task.id,
    file_path: task.filePath,
    output_dir: targetOutputDir,
    output_name: String(task.outputName || '').trim(),
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

// onlyFile 只给单条资源的「转换这一个」用；不传则把库里全部待转换文件一起入队。
const startConvertQueue = async (onlyFile = '') => {
  if (!running.value) {
    const pool = onlyFile ? [onlyFile] : selectedFiles.value
    if (pool.length) {
      enqueueSources(pool)
    }
  }
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
      // 该任务随后归入完成记录，刷新后资源库里立刻可见。
      void loadRecords()
    } catch (e) {
      const text = String(e?.message || e || '转换失败')
      if (text.includes('__TTS_BOOK_PAUSED__')) {
        logger.info('startConvertQueue paused by user')
        updateTask(task.id, { status: 'paused', message: '任务已暂停，可稍后继续' })
        pausedCount += 1
      } else if (text.includes('__TTS_BOOK_STOPPED__')) {
        logger.info('startConvertQueue stopped by user')
        updateTask(task.id, { status: 'stopped', message: '任务已终止' })
        stoppedCount += 1
      } else {
        logger.error('startConvertQueue failed:', e)
        updateTask(task.id, { status: 'error', progress: Math.max(Number(task.progress || 0), 0), message: text })
        failedCount += 1
      }
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

const startSingle = async (res) => {
  await startConvertQueue(res.filePath)
}

const pauseResource = async (res) => {
  const task = findTask(res)
  if (!task) return
  if (task.status === 'running') {
    try {
      await invoke('tts_book_pause_task', { taskId: task.taskId || task.id })
      updateTask(task.id, { message: '已请求暂停，当前分段完成后会暂停' })
    } catch (e) {
      logger.error('pauseResource failed:', e)
      message.error('暂停任务失败')
    }
    return
  }
  if (task.status === 'queued') {
    updateTask(task.id, { status: 'paused', message: '任务已暂停，等待继续' })
  }
}

const stopResource = async (res) => {
  const task = findTask(res)
  if (!task) return
  if (task.status === 'running') {
    try {
      await invoke('tts_book_stop_task', { taskId: task.taskId || task.id })
      updateTask(task.id, { message: '已请求终止，当前分段完成后会停止' })
    } catch (e) {
      logger.error('stopResource failed:', e)
      message.error('终止任务失败')
    }
    return
  }
  updateTask(task.id, { status: 'stopped', message: '任务已终止' })
}

// 暂停/失败/终止后重新入队都走这一条：后端会用同一份 resume_state 接着做。
const resumeResource = async (res) => {
  const task = findTask(res)
  if (!task) return
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

// ───────────────────────── 已完成记录 ─────────────────────────

const loadRecords = async () => {
  try {
    records.value = await invoke('tts_book_list_records')
  } catch (e) {
    logger.error('loadRecords failed:', e)
  }
}

const scanRecords = async () => {
  scanningRecords.value = true
  try {
    const imported = await invoke('tts_book_scan_records')
    await loadRecords()
    message.success(imported > 0 ? `已导入 ${imported} 条记录` : '没有发现可导入的产物')
  } catch (e) {
    logger.error('scanRecords failed:', e)
    message.error(String(e?.message || e || '扫描失败'))
  } finally {
    scanningRecords.value = false
  }
}

const renameRecord = (record) => {
  let next = record.title || record.file_name || ''
  dialog.info({
    title: '重命名记录',
    content: () =>
      h(NInput, {
        defaultValue: next,
        placeholder: '新的标题（只改显示名，不动磁盘文件）',
        onUpdateValue: (value) => {
          next = value
        },
      }),
    positiveText: '保存',
    negativeText: '取消',
    onPositiveClick: async () => {
      const title = String(next || '').trim()
      if (!title) {
        message.warning('标题不能为空')
        return false
      }
      try {
        await invoke('tts_book_rename_record', { recordId: record.id, title })
        await loadRecords()
        message.success('已重命名')
        return true
      } catch (e) {
        logger.error('renameRecord failed:', e)
        message.error(String(e?.message || e || '重命名失败'))
        return false
      }
    },
  })
}

// 两个按钮都真的会删，所以把「连产物一起删」标成红色避免误点。
// 与 ResourceHunterApp 的「删除记录 / 删除记录和文件」是同一套交互。
const deleteRecord = (record) => {
  dialog.warning({
    title: '删除记录',
    content: `「${record.title || record.file_name}」：可以只移除记录，或连同音频、分段 JSON、章节与分段缓存一起删掉（后者不可恢复）。`,
    positiveText: '仅移除记录',
    negativeText: '同时删除产物文件',
    negativeButtonProps: { type: 'error', tertiary: true },
    onPositiveClick: () => applyDeleteRecord(record, false),
    onNegativeClick: () => applyDeleteRecord(record, true),
  })
}

const applyDeleteRecord = async (record, removeFiles) => {
  try {
    await invoke('tts_book_delete_record', { recordId: record.id, removeFiles })
    await loadRecords()
    message.success(removeFiles ? '记录与产物已删除' : '记录已移除')
  } catch (e) {
    logger.error('deleteRecord failed:', e)
    message.error(String(e?.message || e || '删除失败'))
  }
}

const formatBytes = (bytes) => {
  const value = Number(bytes || 0)
  if (!value) return ''
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(0)} KB`
  if (value < 1024 * 1024 * 1024) return `${(value / 1024 / 1024).toFixed(1)} MB`
  return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`
}

// 完成记录的说明行：完成时间 · 段数 · 字数 · 体积 · 音色。
// 时间放最前面：这一行会被截断，放在末尾等于永远看不见。
const describeRecord = (record) => {
  const parts = []
  const finished = formatFinishedAt(record.finished_at)
  if (finished) parts.push(finished)
  if (record.total_chunks) parts.push(`${record.total_chunks} 段`)
  if (record.total_chars) parts.push(`${Number(record.total_chars).toLocaleString('zh-CN')} 字`)
  if (record.audio_bytes) parts.push(formatBytes(record.audio_bytes))
  if (record.speaker_id !== undefined && record.speaker_id !== null && String(record.speaker_id) !== '') {
    parts.push(`Speaker ${record.speaker_id}`)
  }
  return parts.join(' · ')
}

const describeTask = (task) => {
  const parts = []
  const total = Number(task.stats?.total_chunks || 0)
  if (total) {
    const zh = Number(task.stats?.zh_chunks || 0)
    const en = Number(task.stats?.en_chunks || 0)
    // 续转任务只带总段数，中英拆分为 0 时显示出来只会让人以为统计坏了。
    parts.push(zh || en ? `分段 ${total}（中 ${zh} / 英 ${en}）` : `分段 ${total}`)
  }
  if (task.chapterCount > 0) parts.push(`章节音频 ${task.chapterCount} 个`)
  if (task.message) parts.push(task.message)
  return parts.join(' · ')
}

const formatFinishedAt = (value) => {
  const text = String(value || '').trim()
  if (!text) return ''
  const date = new Date(text)
  if (Number.isNaN(date.getTime())) return text
  return date.toLocaleString('zh-CN', { hour12: false })
}
</script>

<style scoped>
.tts-book-root {
  --tts-bg-1: #eef4fb;
  --tts-bg-2: #f7fbf8;
  --tts-panel: rgba(255, 255, 255, 0.86);
  --tts-panel-solid: rgba(255, 255, 255, 0.95);
  --tts-rail: rgba(250, 253, 255, 0.7);
  --tts-border: rgba(31, 71, 109, 0.13);
  --tts-hairline: rgba(31, 71, 109, 0.07);
  --tts-title: #12334f;
  --tts-text: #2b4a66;
  --tts-muted: #71879b;
  --tts-accent: #2f6fd0;
  --tts-st-idle: #7c8fa3;
  --tts-st-queued: #4b7fd6;
  --tts-st-running: #1091b8;
  --tts-st-paused: #c08a1e;
  --tts-st-error: #c8443f;
  --tts-st-stopped: #8a8f98;
  --tts-st-done: #1f9d63;
  --tts-resizer: rgba(76, 124, 168, 0.2);
  --tts-resizer-hover: rgba(76, 124, 168, 0.45);
  --tts-header-h: 46px;
  --tts-font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Microsoft YaHei', 'Courier New', monospace;
  height: 100%;
  min-height: 0;
  padding: 0;
  box-sizing: border-box;
  overflow: hidden;
  font-family: var(--tts-font-family);
  color: var(--tts-text);
  background:
    radial-gradient(circle at 0% 0%, rgba(123, 196, 255, 0.2), transparent 32%),
    radial-gradient(circle at 100% 0%, rgba(128, 224, 170, 0.16), transparent 28%),
    linear-gradient(135deg, var(--tts-bg-1), var(--tts-bg-2));
}

/* ───────── 顶栏：标题 + 全局读数 + 队列进度 + 操作 ───────── */
.tts-header {
  height: var(--tts-header-h);
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 0 12px;
  box-sizing: border-box;
  border-bottom: 1px solid var(--tts-border);
  background: var(--tts-panel-solid);
  backdrop-filter: blur(12px);
}

.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.brand-icon {
  color: var(--tts-accent);
}

.brand-title {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: var(--tts-title);
  white-space: nowrap;
}

.stat-strip {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.stat-chip {
  display: inline-flex;
  align-items: baseline;
  gap: 5px;
  padding: 3px 9px;
  border-radius: 999px;
  border: 1px solid var(--tts-hairline);
  background: var(--tts-rail);
  font-size: 11px;
  color: var(--tts-muted);
}

.stat-chip b {
  font-size: 13px;
  font-weight: 700;
  color: var(--tts-title);
  font-variant-numeric: tabular-nums;
}

.stat-chip em {
  font-style: normal;
}

.stat-chip.tone-idle b { color: var(--tts-st-idle); }
.stat-chip.tone-running b { color: var(--tts-st-running); }
.stat-chip.tone-done b { color: var(--tts-st-done); }
.stat-chip.tone-error b { color: var(--tts-st-error); }

.header-meter {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 120px;
  max-width: 420px;
}

.meter-label {
  font-size: 11px;
  color: var(--tts-muted);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
  flex-shrink: 0;
}

.rail-toggle.active {
  color: var(--tts-accent);
}

/* ───────── 主体三轨 ───────── */
.tts-body {
  display: grid;
  grid-template-columns: var(--body-columns);
  gap: 0;
  align-items: stretch;
  min-width: 980px;
  width: 100%;
  height: calc(100% - var(--tts-header-h));
  min-height: 0;
}

.rail {
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--tts-rail);
  backdrop-filter: blur(10px);
}

.params-rail {
  border-right: 1px solid var(--tts-border);
}

.rail-head {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--tts-hairline);
  flex-shrink: 0;
}

.rail-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--tts-title);
}

.rail-sub {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 11px;
  color: var(--tts-muted);
  text-align: right;
}

.rail-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* ───────── 参数（紧凑表单，取代原来的气泡卡片阵） ───────── */
.param-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.group-title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.16em;
  color: var(--tts-muted);
  text-transform: uppercase;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--tts-hairline);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
}

.field-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--tts-text);
}

.field-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 6px;
  align-items: center;
}

.field-pair {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.field-pair .field-narrow {
  max-width: 120px;
}

.field-inline {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
}

.field-tip {
  font-size: 11px;
  color: var(--tts-muted);
  overflow-wrap: anywhere;
}

.field-tip.push-down {
  margin-top: -4px;
}

/* ───────── 资源库 ───────── */
.library {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  min-height: 0;
  min-width: 0;
  background: var(--tts-panel);
  container-type: inline-size;
}

.lib-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 320px) minmax(0, 1fr) auto;
  grid-template-areas: 'search facets tools';
  align-items: center;
  gap: 8px 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--tts-hairline);
}

.lib-search {
  grid-area: search;
  min-width: 0;
  max-width: 360px;
}

.facet-row {
  grid-area: facets;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.facet-row::-webkit-scrollbar {
  display: none;
}

/* 窄容器：筛选条整行下沉，搜索框与视图工具留在首行。
   单行模式需要约 880px（搜索 320 + 筛选 384 + 工具 148 + 间距），留出余量后取 940。 */
@container (max-width: 939px) {
  .lib-toolbar {
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      'search tools'
      'facets facets';
  }
}

.facet {
  display: inline-flex;
  align-items: center;
  flex: 0 0 auto;
  gap: 6px;
  padding: 4px 10px;
  white-space: nowrap;
  border-radius: 999px;
  border: 1px solid var(--tts-hairline);
  background: transparent;
  color: var(--tts-muted);
  font-family: inherit;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
}

.facet:hover {
  color: var(--tts-title);
  border-color: var(--tts-border);
  background: rgba(47, 111, 208, 0.06);
}

.facet.on {
  color: #fff;
  border-color: transparent;
  background: var(--tts-accent);
}

.facet-count {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

.facet.on .facet-count {
  opacity: 0.9;
}

.lib-tools {
  grid-area: tools;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.lib-scroll {
  min-height: 0;
  overflow: auto;
  padding: 12px;
}

.res-collection.view-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.res-collection.view-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 12px;
  align-content: start;
}

/* 状态色只落在左边那条「书脊」上，列表整体保持安静。 */
.res-item {
  --st-color: var(--tts-st-idle);
  position: relative;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 10px 12px 10px 16px;
  border: 1px solid var(--tts-border);
  border-radius: 10px;
  background: var(--tts-panel-solid);
  transition: border-color 0.15s ease, box-shadow 0.15s ease, transform 0.15s ease;
}

.res-item:hover {
  border-color: rgba(47, 111, 208, 0.3);
  box-shadow: 0 6px 18px -12px rgba(18, 51, 79, 0.5);
}

.res-item::before {
  content: '';
  position: absolute;
  left: 6px;
  top: 9px;
  bottom: 9px;
  width: 3px;
  border-radius: 999px;
  background: var(--st-color);
}

.res-item.st-queued { --st-color: var(--tts-st-queued); }
.res-item.st-running { --st-color: var(--tts-st-running); }
.res-item.st-paused { --st-color: var(--tts-st-paused); }
.res-item.st-error { --st-color: var(--tts-st-error); }
.res-item.st-stopped { --st-color: var(--tts-st-stopped); }
.res-item.st-success { --st-color: var(--tts-st-done); }

.res-item.st-running::before {
  animation: spine-pulse 1.6s ease-in-out infinite;
}

@keyframes spine-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

.res-format {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  width: 46px;
  height: 46px;
  border-radius: 8px;
  border: 1px solid var(--tts-hairline);
  background: linear-gradient(150deg, rgba(47, 111, 208, 0.1), rgba(47, 111, 208, 0.03));
  flex-shrink: 0;
}

.format-text {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  color: var(--tts-accent);
}

.format-count {
  font-size: 9px;
  color: var(--tts-muted);
}

.res-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.res-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.res-title {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 13px;
  font-weight: 600;
  color: var(--tts-title);
}

.res-status {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--st-color);
  white-space: nowrap;
}

.res-file,
.res-meta {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 11px;
  color: var(--tts-muted);
}

.res-progress {
  margin-top: 4px;
}

.res-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 4px;
  flex-shrink: 0;
}

/* 书架视图：同样的标记换一套排布，不再写第二份模板。 */
.res-collection.view-grid .res-item {
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto minmax(0, 1fr) auto;
  align-items: stretch;
  gap: 10px;
  padding: 14px 12px 10px 16px;
}

.res-collection.view-grid .res-format {
  width: 100%;
  height: 56px;
  flex-direction: row;
  gap: 6px;
}

.res-collection.view-grid .format-text {
  font-size: 13px;
}

.res-collection.view-grid .res-head {
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
}

.res-collection.view-grid .res-title {
  white-space: normal;
  overflow-wrap: anywhere;
  line-height: 1.4;
}

.res-collection.view-grid .res-actions {
  justify-content: flex-start;
  border-top: 1px solid var(--tts-hairline);
  padding-top: 8px;
}

/* ───────── 分栏拖拽 ───────── */
.col-resizer {
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: col-resize;
  user-select: none;
  touch-action: none;
  background: linear-gradient(180deg, transparent, var(--tts-resizer), transparent);
}

.col-resizer:hover {
  background: linear-gradient(180deg, transparent, var(--tts-resizer-hover), transparent);
}

.resizer-dot {
  width: 3px;
  height: 34px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.6);
}

/* ───────── 播放器 ───────── */
.player-rail {
  border-left: 1px solid var(--tts-border);
}

.player-body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 12px;
  padding: 12px;
  overflow: hidden;
}

.audio-element {
  width: 100%;
  min-height: 44px;
  flex-shrink: 0;
}

.subtitle-panel {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 10px;
  min-height: 0;
  overflow: hidden;
}

.current-subtitle {
  padding: 12px;
  border-radius: 10px;
  border: 1px solid var(--tts-border);
  background: var(--tts-panel-solid);
  color: var(--tts-title);
  font-size: 15px;
  line-height: 1.7;
}

.subtitle-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow: auto;
  min-height: 0;
}

.subtitle-item {
  display: grid;
  grid-template-columns: 30px minmax(0, 1fr);
  gap: 8px;
  align-items: start;
  width: 100%;
  padding: 7px 10px;
  border: 1px solid transparent;
  border-radius: 8px;
  background: transparent;
  color: var(--tts-text);
  text-align: left;
  cursor: pointer;
  font-family: inherit;
  font-size: 13px;
  line-height: 1.65;
  transition: background 0.15s ease;
}

.subtitle-item:hover {
  background: rgba(47, 111, 208, 0.06);
}

.subtitle-item.active {
  background: rgba(47, 111, 208, 0.12);
  border-color: rgba(47, 111, 208, 0.26);
}

.subtitle-index {
  font-size: 11px;
  color: var(--tts-muted);
  font-variant-numeric: tabular-nums;
}

.subtitle-empty {
  font-size: 12px;
  color: var(--tts-muted);
  padding: 4px 2px;
}

/* ───────── naive-ui 外观对齐 ───────── */
.tts-book-root :deep(.n-button),
.tts-book-root :deep(.n-base-selection),
.tts-book-root :deep(.n-input),
.tts-book-root :deep(.n-input-number),
.tts-book-root :deep(.n-tag),
.tts-book-root :deep(.n-empty),
.tts-book-root :deep(.n-progress),
.tts-book-root :deep(.n-switch) {
  font-family: inherit;
}

.tts-book-root :deep(.n-input .n-input-wrapper),
.tts-book-root :deep(.n-input-number .n-input-wrapper),
.tts-book-root :deep(.n-base-selection) {
  background: rgba(255, 255, 255, 0.92);
}

/* ───────── 深色 ───────── */
.tts-book-root.tts-book-dark {
  --tts-bg-1: #101924;
  --tts-bg-2: #0e141d;
  --tts-panel: rgba(18, 27, 39, 0.88);
  --tts-panel-solid: rgba(19, 28, 40, 0.94);
  --tts-rail: rgba(15, 23, 34, 0.7);
  --tts-border: rgba(126, 158, 189, 0.18);
  --tts-hairline: rgba(126, 158, 189, 0.1);
  --tts-title: #e7f0fb;
  --tts-text: #c7d5e5;
  --tts-muted: #8ea2b8;
  --tts-accent: #5f9bea;
  --tts-st-idle: #8ea2b8;
  --tts-st-queued: #6ea3f0;
  --tts-st-running: #3fc0e0;
  --tts-st-paused: #e0b44e;
  --tts-st-error: #ec7a72;
  --tts-st-stopped: #98a0aa;
  --tts-st-done: #4fc98c;
  --tts-resizer: rgba(142, 162, 184, 0.24);
  --tts-resizer-hover: rgba(142, 162, 184, 0.48);
}

.tts-book-root.tts-book-dark :deep(.n-input .n-input-wrapper),
.tts-book-root.tts-book-dark :deep(.n-input-number .n-input-wrapper),
.tts-book-root.tts-book-dark :deep(.n-base-selection) {
  background: rgba(18, 27, 39, 0.94);
  color: #d7e5f5;
}

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
