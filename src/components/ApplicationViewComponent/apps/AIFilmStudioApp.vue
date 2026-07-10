<template>
  <div class="film-studio">
    <header class="studio-header">
      <div>
        <div class="eyebrow">GUAXI STORY PIPELINE · BLENDER 5.2 LTS</div>
        <h1>AI 影像工坊</h1>
        <p>从 Markdown、史料、地理数据或小说出发，编译分镜并交给 Blender 制作预演。</p>
      </div>
      <div class="header-actions">
        <n-tag :type="studio.blenderStatus.compatible ? 'success' : 'warning'" round>
          {{ studio.blenderStatus.compatible ? studio.blenderStatus.detectedVersion : 'Blender 未就绪' }}
        </n-tag>
        <n-button size="small" :loading="studio.checkingBlender" @click="refreshBlender">
          检测环境
        </n-button>
        <n-button size="small" @click="openPluginDirectory">插件目录</n-button>
      </div>
    </header>

    <main class="studio-layout">
      <aside class="source-panel panel-scroll">
        <section class="panel-section">
          <div class="section-heading">
            <span class="step-number">01</span>
            <div>
              <h2>文稿与项目</h2>
              <small>知识库是默认入口，不是数据边界</small>
            </div>
          </div>
          <div class="button-grid">
            <n-button type="primary" :loading="studio.loadingSource" @click="pickMarkdown(true)">
              从知识库选择
            </n-button>
            <n-button :loading="studio.loadingSource" @click="pickMarkdown(false)">任意位置</n-button>
          </div>
          <n-button block size="small" secondary @click="pickExistingProject">打开已有影像项目</n-button>
          <div v-if="studio.sourcePath" class="path-card">
            <strong>{{ studio.sourceName }}</strong>
            <span :title="studio.sourcePath">{{ studio.sourcePath }}</span>
            <small>{{ formatBytes(studio.sourceByteLength) }}</small>
          </div>
          <n-input v-model:value="studio.projectName" size="small" placeholder="项目名称" />
          <div class="path-row">
            <n-input v-model:value="studio.outputRoot" size="small" placeholder="项目输出目录" />
            <n-button size="small" @click="pickOutputRoot">选择</n-button>
          </div>
        </section>

        <section class="panel-section">
          <div class="section-heading">
            <span class="step-number">02</span>
            <div>
              <h2>故事编译模型</h2>
              <small>复用 Guaxi 云端、本地和自定义模型</small>
            </div>
          </div>
          <n-form label-placement="top" size="small">
            <n-form-item label="模型">
              <n-select v-model:value="studio.modelId" :options="modelOptions" filterable />
            </n-form-item>
            <div class="form-grid">
              <n-form-item label="内容类型">
                <n-select v-model:value="studio.genre" :options="genreOptions" />
              </n-form-item>
              <n-form-item label="目标时长（秒）">
                <n-input-number v-model:value="studio.targetDurationSeconds" :min="5" :max="10800" />
              </n-form-item>
            </div>
            <n-form-item label="视觉风格">
              <n-select v-model:value="studio.visualStyle" :options="styleOptions" filterable tag />
            </n-form-item>
            <n-form-item label="制作模式">
              <n-select v-model:value="studio.productionMode" :options="productionModeOptions" />
            </n-form-item>
            <n-form-item label="导演补充要求">
              <n-input
                v-model:value="studio.additionalDirection"
                type="textarea"
                :autosize="{ minRows: 2, maxRows: 5 }"
                placeholder="例如：地图必须标注比例尺；人物服饰遵循汉代考据；避免生成写实战争伤口。"
              />
            </n-form-item>
          </n-form>
          <n-button
            type="primary"
            block
            :disabled="!studio.hasSource"
            :loading="studio.compiling"
            @click="compileStory"
          >
            编译 Story IR 与分镜
          </n-button>
        </section>

        <section class="panel-section">
          <div class="section-heading">
            <span class="step-number">03</span>
            <div>
              <h2>Blender 5.2</h2>
              <small>开发期允许 RC，生产期锁定同一补丁版</small>
            </div>
          </div>
          <div class="path-row">
            <n-input v-model:value="studio.blenderPath" size="small" placeholder="blender.exe 路径" />
            <n-button size="small" @click="pickBlenderExecutable">选择</n-button>
          </div>
          <div class="path-row">
            <n-input v-model:value="studio.ffmpegPath" size="small" placeholder="ffmpeg.exe（用于 MP4 编码）" />
            <n-button size="small" @click="pickFfmpegExecutable">选择</n-button>
          </div>
          <n-tag size="small" :type="studio.mediaToolsStatus.ffmpegAvailable ? 'success' : 'warning'">
            {{ studio.mediaToolsStatus.ffmpegAvailable ? 'FFmpeg 可用' : 'PNG 序列模式' }}
          </n-tag>
          <div class="bridge-box">
            <div class="bridge-title">
              <span>交互式 Bridge</span>
              <n-tag size="small" :type="studio.bridgeConnected ? 'success' : 'default'">
                {{ studio.bridgeConnected ? '已连接' : '未连接' }}
              </n-tag>
            </div>
            <div class="bridge-fields">
              <n-input-number v-model:value="studio.bridgePort" size="small" :min="1024" :max="65535" />
              <n-input
                v-model:value="studio.bridgeToken"
                size="small"
                type="password"
                show-password-on="click"
                placeholder="插件访问令牌"
              />
              <n-button size="small" :loading="studio.probingBridge" @click="probeBridge">连接</n-button>
            </div>
            <n-button
              v-if="studio.bridgeConnected && studio.project"
              size="small"
              block
              :disabled="!studio.bridgeToken"
              @click="sendToInteractiveBlender"
            >
              发送 Story IR 到当前 Blender
            </n-button>
          </div>
          <n-alert
            v-if="studio.blenderStatus.warnings?.length"
            type="warning"
            :show-icon="false"
            class="compact-alert"
          >
            <div v-for="warning in studio.blenderStatus.warnings" :key="warning">{{ warning }}</div>
          </n-alert>
          <n-button
            block
            :disabled="!studio.canCreateProject"
            :loading="studio.creatingProject"
            @click="createProject"
          >
            创建标准制作项目
          </n-button>
          <div v-if="studio.project" class="project-actions">
            <n-button size="small" @click="openProjectDirectory">打开项目目录</n-button>
            <n-button
              size="small"
              type="primary"
              :disabled="!studio.canStartPreview"
              :loading="studio.startingPreview"
              @click="startPreview(true)"
            >
              生成 MP4 预演
            </n-button>
            <n-button
              size="small"
              :disabled="!studio.canStartPreview"
              @click="startPreview(false)"
            >
              仅构建 .blend
            </n-button>
          </div>
          <div v-if="studio.jobStatus !== 'idle'" class="job-card">
            <div class="job-line">
              <strong>{{ jobStatusLabel }}</strong>
              <span>{{ studio.jobProgress }}%</span>
            </div>
            <n-progress
              type="line"
              :percentage="studio.jobProgress"
              :status="studio.jobStatus === 'failed' ? 'error' : undefined"
              :show-indicator="false"
            />
            <small>{{ studio.jobMessage || studio.jobStage }}</small>
            <n-button
              v-if="studio.jobStatus === 'running'"
              size="tiny"
              type="error"
              ghost
              @click="cancelJob"
            >
              取消任务
            </n-button>
          </div>
        </section>
      </aside>

      <section class="workspace-panel">
        <n-tabs v-model:value="activeTab" type="line" animated class="studio-tabs">
          <n-tab-pane name="source" tab="文稿工作台">
            <div class="source-editor-wrap">
              <div v-if="!studio.hasSource" class="empty-state">
                <div class="empty-icon">🎬</div>
                <h2>选择一篇 Markdown 开始制作</h2>
                <p>可从知识库默认目录进入，也可加载磁盘任意位置的 UTF-8 文稿。</p>
                <n-button type="primary" @click="pickMarkdown(true)">选择文稿</n-button>
              </div>
              <template v-else>
                <div class="editor-toolbar">
                  <span>{{ studio.sourceName }}</span>
                  <small>此处修改会用于本次编译；创建项目时保存 UTF-8 副本</small>
                </div>
                <n-input
                  v-model:value="studio.sourceContent"
                  type="textarea"
                  class="source-editor"
                  placeholder="Markdown 文稿"
                />
              </template>
            </div>
          </n-tab-pane>

          <n-tab-pane name="storyboard" tab="分镜与场景">
            <div v-if="!studio.hasStoryIr" class="empty-state compact">
              <div class="empty-icon">🧭</div>
              <h2>尚未生成 Story IR</h2>
              <p>模型会先提取事实、人物、地点、资产需求，再生成场景和镜头。</p>
            </div>
            <div v-else class="storyboard-content panel-scroll">
              <div class="metric-grid">
                <div class="metric-card"><strong>{{ studio.sceneCount }}</strong><span>场景</span></div>
                <div class="metric-card"><strong>{{ studio.shotCount }}</strong><span>镜头</span></div>
                <div class="metric-card"><strong>{{ studio.storyIr.metadata?.targetDurationSeconds || '-' }}</strong><span>目标秒数</span></div>
                <div class="metric-card"><strong>{{ assetCount }}</strong><span>资产需求</span></div>
              </div>
              <n-alert v-for="warning in studio.compileWarnings" :key="warning" type="warning" :show-icon="false">
                {{ warning }}
              </n-alert>
              <article
                v-for="(scene, sceneIndex) in studio.storyIr.scenes"
                :key="scene.id || sceneIndex"
                class="scene-card"
              >
                <div class="scene-header">
                  <div>
                    <span class="scene-index">SCENE {{ String(sceneIndex + 1).padStart(2, '0') }}</span>
                    <h3>{{ scene.title || scene.id }}</h3>
                  </div>
                  <n-tag size="small" round>{{ scene.durationSeconds || '?' }} 秒</n-tag>
                </div>
                <p>{{ scene.summary }}</p>
                <div class="shot-list">
                  <div v-for="(shot, shotIndex) in scene.shots || []" :key="shot.id || shotIndex" class="shot-row">
                    <span class="shot-number">{{ shotIndex + 1 }}</span>
                    <div>
                      <strong>{{ shot.shotType || 'shot' }} · {{ shot.cameraMovement || 'static' }}</strong>
                      <p>{{ shot.description || shot.action }}</p>
                      <small v-if="shot.narration">旁白：{{ shot.narration }}</small>
                    </div>
                    <span>{{ shot.durationSeconds || '?' }}s</span>
                  </div>
                </div>
              </article>
            </div>
          </n-tab-pane>

          <n-tab-pane name="json" tab="Story IR">
            <div class="json-editor-wrap">
              <div class="editor-toolbar">
                <span>可审计的 Blender 中间协议</span>
                <n-button size="tiny" :disabled="!storyIrText" @click="applyStoryIrJson">应用 JSON 修改</n-button>
              </div>
              <n-input
                v-model:value="storyIrText"
                type="textarea"
                class="json-editor"
                placeholder="生成 Story IR 后可在这里检查和调整"
              />
            </div>
          </n-tab-pane>

          <n-tab-pane name="pipeline" tab="全流程">
            <div class="pipeline-board panel-scroll">
              <div v-for="(stage, index) in pipelineStages" :key="stage.key" class="pipeline-stage">
                <div class="stage-index">{{ String(index + 1).padStart(2, '0') }}</div>
                <div>
                  <h3>{{ stage.title }}</h3>
                  <p>{{ stage.description }}</p>
                  <div class="stage-tags">
                    <n-tag v-for="tag in stage.tags" :key="tag" size="small" :bordered="false">{{ tag }}</n-tag>
                  </div>
                </div>
              </div>
            </div>
          </n-tab-pane>

          <n-tab-pane name="logs" tab="任务日志">
            <div class="log-list panel-scroll">
              <div v-if="!studio.logs.length" class="empty-state compact"><p>暂无任务日志</p></div>
              <div v-for="log in studio.logs" :key="log.id" class="log-row" :class="`is-${log.level}`">
                <span>{{ log.time }}</span>
                <strong>{{ log.level.toUpperCase() }}</strong>
                <div>
                  <p>{{ log.message }}</p>
                  <small v-if="log.details">{{ stringifyDetails(log.details) }}</small>
                </div>
              </div>
            </div>
          </n-tab-pane>
        </n-tabs>
      </section>

      <aside class="inspector-panel panel-scroll">
        <section>
          <div class="inspector-title">制作原则</div>
          <div class="principle-card">
            <strong>Guaxi 负责“为什么这样拍”</strong>
            <p>文稿、史料证据、地理语义、角色关系、跨章节连续性与审核。</p>
          </div>
          <div class="principle-card accent">
            <strong>Blender 负责“画面如何实现”</strong>
            <p>资产、布局、相机、动画、灯光、VSE 时间线、合成和渲染。</p>
          </div>
        </section>

        <section>
          <div class="inspector-title">Pallaidium 对照策略</div>
          <ul class="check-list">
            <li>借鉴 VSE 中图片、视频、语音、字幕统一成为 Strip</li>
            <li>借鉴可发现的模型插件和异步 Render Queue</li>
            <li>兼容本地、ComfyUI 和 OpenAI /v1 风格媒体后端</li>
            <li>保留 seed、prompt、模型、许可证和来源元数据</li>
            <li>支持从成片反向提取字幕与剧本，形成修改闭环</li>
          </ul>
        </section>

        <section v-if="studio.hasStoryIr">
          <div class="inspector-title">当前项目</div>
          <dl class="project-facts">
            <div><dt>标题</dt><dd>{{ studio.storyIr.metadata?.title || studio.projectName }}</dd></div>
            <div><dt>类型</dt><dd>{{ studio.storyIr.metadata?.genre || studio.genre }}</dd></div>
            <div><dt>风格</dt><dd>{{ studio.storyIr.metadata?.visualStyle || studio.visualStyle }}</dd></div>
            <div><dt>帧率</dt><dd>{{ studio.storyIr.metadata?.fps || 24 }} fps</dd></div>
            <div><dt>事实缺口</dt><dd>{{ researchGapCount }}</dd></div>
          </dl>
        </section>
      </aside>
    </main>
  </div>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { openPath } from '@tauri-apps/plugin-opener'
import { useMessage } from 'naive-ui'
import { useChatStore } from '../../../stores/chatStore'
import { useAIFilmStudioStore } from '../../../stores/aiFilmStudioStore'
import { createLogger } from '../../../utils/logger'

const logger = createLogger('AIFilmStudioApp')
const message = useMessage()
const chatStore = useChatStore()
const studio = useAIFilmStudioStore()
const activeTab = ref('source')
const storyIrText = ref('')

const modelOptions = computed(() => {
  const remote = Array.isArray(chatStore.LLMlist?.remoteModel) ? chatStore.LLMlist.remoteModel : []
  const local = Array.isArray(chatStore.LLMlist?.localModel) ? chatStore.LLMlist.localModel : []
  return [...remote, ...local]
    .map((item) => ({ label: String(item?.name || '未命名模型'), value: Number(item?.selectedModel) }))
    // 网页模型没有稳定的结构化 API，Story IR 编译只展示后端可调用的模型。
    .filter((item) => Number.isFinite(item.value) && item.value !== 7 && item.value !== 8)
})

const genreOptions = [
  { label: '史书 / 历史', value: 'history' },
  { label: '地理 / 地图叙事', value: 'geography' },
  { label: '小说 / 故事电影', value: 'novel' },
  { label: '纪录片', value: 'documentary' },
  { label: '知识讲解', value: 'education' },
  { label: '其他', value: 'other' },
]
const styleOptions = [
  { label: '电影写实', value: 'cinematic-realism' },
  { label: '历史纪录片', value: 'historical-documentary' },
  { label: '水墨与三维融合', value: 'ink-painting-3d' },
  { label: '风格化低多边形', value: 'stylized-low-poly' },
  { label: '地图信息可视化', value: 'cartographic-visualization' },
  { label: '二维动画', value: '2d-animation' },
]
const productionModeOptions = [
  { label: '混合制作：3D + AI 媒体', value: 'hybrid-3d-ai' },
  { label: 'Blender 原生 3D', value: 'native-3d' },
  { label: 'AI 镜头 + VSE 剪辑', value: 'ai-media-vse' },
  { label: '分镜预演', value: 'storyboard-only' },
]

const pipelineStages = [
  { key: 'source', title: '文稿与研究', description: '加载 Markdown、知识库条目、史料引文、地名和时间线；记录证据来源与不确定性。', tags: ['Markdown', 'RAG', '史料校验'] },
  { key: 'story', title: 'Story IR', description: '将长文拆为事实、实体、场景、镜头、资产、音频和发布要求，稳定 ID 贯穿全项目。', tags: ['JSON Schema', '版本化', '可审计'] },
  { key: 'storyboard', title: '分镜与预演', description: '生成构图参考、镜头时长、相机运动和低模角色走位，先验证叙事再追求画质。', tags: ['Animatic', 'Camera', 'Timing'] },
  { key: 'asset', title: '资产与世界构建', description: '匹配本地 Blender 资产、GIS 地形、历史服饰、道具；缺失资产再交给程序化或生成模型。', tags: ['Asset Browser', 'GIS', 'Geometry Nodes'] },
  { key: 'generation', title: '媒体生成后端', description: '采用 Pallaidium 式 provider 插件：本地模型、ComfyUI、OpenAI /v1 风格服务或云 API 都进入统一队列。', tags: ['Provider', 'Queue', 'ComfyUI'] },
  { key: 'animation', title: '角色、动画与特效', description: '动作重定向、口型、群集、布料、烟火与地图路线动画；关键镜头允许人工精修。', tags: ['Rigging', 'Lip Sync', 'Simulation'] },
  { key: 'audio', title: '配音与声音', description: '复用 Guaxi 配音能力，生成角色对白、旁白、环境音、拟音、音乐和时间对齐字幕。', tags: ['TTS', 'SFX', 'SRT'] },
  { key: 'edit', title: 'VSE 剪辑与回改', description: '所有镜头媒体成为带模型、seed、prompt 和来源元数据的 Strip，支持重做、替换和反向重建剧本。', tags: ['VSE', 'Metadata', 'Redo'] },
  { key: 'render', title: '渲染与合成', description: '预演走 Eevee，最终镜头按需走 Cycles；图片序列优先，失败可从单镜头恢复。', tags: ['Eevee', 'Cycles', 'Color'] },
  { key: 'publish', title: '审核与发布', description: '事实、肖像、素材许可证、音乐版权和平台比例检查后导出成片、字幕、封面及制作清单。', tags: ['Rights', 'QC', '16:9 / 9:16'] },
]

const assetCount = computed(() => Array.isArray(studio.storyIr?.assetPlan) ? studio.storyIr.assetPlan.length : 0)
const researchGapCount = computed(() => Array.isArray(studio.storyIr?.research?.researchGaps)
  ? studio.storyIr.research.researchGaps.length
  : 0)
const jobStatusLabel = computed(() => ({
  launching: '正在启动', running: 'Blender 处理中', cancelling: '正在取消',
  completed: '任务完成', failed: '任务失败', cancelled: '已取消',
}[studio.jobStatus] || studio.jobStatus))

watch(
  () => studio.storyIr,
  (value) => {
    storyIrText.value = value ? JSON.stringify(value, null, 2) : ''
  },
  { deep: true, immediate: true },
)

onMounted(async () => {
  try {
    await studio.initialize()
  } catch (error) {
    logger.error('[FilmStudio] initialization failed', error)
    message.error(`影像工坊初始化失败：${String(error)}`)
  }
})

const formatBytes = (bytes) => {
  const value = Number(bytes || 0)
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`
  return `${(value / 1024 / 1024).toFixed(2)} MiB`
}
const stringifyDetails = (details) => {
  if (typeof details === 'string') return details
  try { return JSON.stringify(details) } catch { return String(details) }
}

async function pickMarkdown(preferKnowledgeBase) {
  try {
    const path = await open({
      multiple: false,
      defaultPath: preferKnowledgeBase ? studio.knowledgeRoot || undefined : undefined,
      filters: [{ name: 'Markdown 文稿', extensions: ['md', 'markdown', 'mdown'] }],
    })
    if (!path) return
    await studio.loadMarkdown(path)
    activeTab.value = 'source'
    message.success('Markdown 文稿已加载')
  } catch (error) {
    message.error(String(error))
  }
}

async function pickExistingProject() {
  try {
    const path = await open({
      multiple: false,
      defaultPath: studio.outputRoot || studio.knowledgeRoot || undefined,
      filters: [{ name: 'Guaxi 影像项目', extensions: ['json'] }],
    })
    if (!path) return
    await studio.openProject(path)
    activeTab.value = 'storyboard'
    message.success('影像项目已恢复')
  } catch (error) {
    message.error(String(error))
  }
}

async function pickOutputRoot() {
  const path = await open({ directory: true, multiple: false, defaultPath: studio.outputRoot || studio.knowledgeRoot || undefined })
  if (path) studio.outputRoot = String(path)
}

async function pickBlenderExecutable() {
  const path = await open({
    multiple: false,
    filters: [{ name: 'Blender', extensions: navigator.userAgent.includes('Windows') ? ['exe'] : ['*'] }],
  })
  if (!path) return
  studio.blenderPath = String(path)
  await refreshBlender()
}

async function pickFfmpegExecutable() {
  const path = await open({
    multiple: false,
    filters: [{ name: 'FFmpeg', extensions: navigator.userAgent.includes('Windows') ? ['exe'] : ['*'] }],
  })
  if (!path) return
  studio.ffmpegPath = String(path)
  try {
    await studio.refreshMediaToolsStatus()
    message[studio.mediaToolsStatus.ffmpegAvailable ? 'success' : 'warning'](
      studio.mediaToolsStatus.ffmpegAvailable ? 'FFmpeg 编码器可用' : '所选 FFmpeg 不可用',
    )
  } catch (error) {
    message.error(String(error))
  }
}

async function compileStory() {
  try {
    await studio.compileStoryIr()
    activeTab.value = 'storyboard'
    message.success(`Story IR 已生成：${studio.sceneCount} 个场景，${studio.shotCount} 个镜头`)
  } catch (error) {
    message.error(String(error))
  }
}

function applyStoryIrJson() {
  try {
    studio.replaceStoryIr(JSON.parse(storyIrText.value))
    message.success('Story IR 修改已应用')
  } catch (error) {
    message.error(`JSON 无效：${String(error)}`)
  }
}

async function refreshBlender() {
  try {
    await studio.refreshBlenderStatus()
    message[studio.blenderStatus.compatible ? 'success' : 'warning'](
      studio.blenderStatus.compatible ? 'Blender 5.2 环境可用' : 'Blender 5.2 环境尚未就绪',
    )
  } catch (error) {
    message.error(String(error))
  }
}

async function probeBridge() {
  try {
    await studio.probeBridge()
    message.success('已连接 Blender 交互式插件')
  } catch (error) {
    message.warning(String(error))
  }
}

async function sendToInteractiveBlender() {
  try {
    await studio.sendStoryToBridge()
    message.success('Story IR 已在当前 Blender 中构建')
  } catch (error) {
    message.error(String(error))
  }
}

async function createProject() {
  try {
    await studio.createProject()
    message.success('标准制作项目已创建')
  } catch (error) {
    message.error(String(error))
  }
}

async function startPreview(renderAnimation) {
  try {
    await studio.startPreview(renderAnimation)
    activeTab.value = 'logs'
    message.success(renderAnimation ? 'MP4 预演任务已启动' : 'Blender 工程构建任务已启动')
  } catch (error) {
    message.error(String(error))
  }
}

async function cancelJob() {
  try { await studio.cancelJob() } catch (error) { message.error(String(error)) }
}

async function openPluginDirectory() {
  const path = studio.blenderStatus.pluginDirectory
  if (!path) return message.warning('尚未定位 Blender 插件目录')
  try { await openPath(path) } catch (error) { message.error(String(error)) }
}

async function openProjectDirectory() {
  if (!studio.project?.projectDir) return
  try { await openPath(studio.project.projectDir) } catch (error) { message.error(String(error)) }
}
</script>

<style scoped>
.film-studio {
  height: 100vh;
  display: flex;
  flex-direction: column;
  color: var(--guaxi-text-color);
  background:
    radial-gradient(circle at 20% -10%, rgba(95, 125, 255, 0.13), transparent 35%),
    var(--guaxi-app-bg);
  overflow: hidden;
}

.studio-header {
  min-height: 88px;
  padding: 16px 22px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  border-bottom: 1px solid var(--guaxi-border);
  background: color-mix(in srgb, var(--guaxi-panel-bg) 88%, transparent);
}

.studio-header h1 { margin: 2px 0; font-size: 24px; letter-spacing: -0.02em; }
.studio-header p { margin: 0; color: var(--guaxi-text-secondary); font-size: 12px; }
.eyebrow { color: #6f87ff; font-size: 10px; letter-spacing: 0.16em; font-weight: 700; }
.header-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }

.studio-layout {
  min-height: 0;
  flex: 1;
  display: grid;
  grid-template-columns: minmax(300px, 340px) minmax(520px, 1fr) minmax(260px, 310px);
}

.source-panel, .inspector-panel { background: var(--guaxi-panel-bg); }
.source-panel { border-right: 1px solid var(--guaxi-border); padding: 14px; }
.inspector-panel { border-left: 1px solid var(--guaxi-border); padding: 16px; }
.panel-scroll { overflow: auto; min-height: 0; }
.workspace-panel { min-width: 0; min-height: 0; overflow: hidden; }
.studio-tabs { height: 100%; display: flex; flex-direction: column; }
.studio-tabs :deep(.n-tabs-nav) { padding: 0 16px; margin: 0; background: var(--guaxi-panel-bg); }
.studio-tabs :deep(.n-tabs-pane-wrapper), .studio-tabs :deep(.n-tab-pane) { min-height: 0; height: 100%; }

.panel-section { padding: 14px; margin-bottom: 12px; border: 1px solid var(--guaxi-border); border-radius: 14px; background: var(--guaxi-content-bg); }
.section-heading { display: flex; gap: 10px; align-items: center; margin-bottom: 14px; }
.section-heading h2 { font-size: 14px; margin: 0; }
.section-heading small { font-size: 10px; color: var(--guaxi-text-secondary); }
.step-number { width: 30px; height: 30px; border-radius: 9px; display: grid; place-items: center; background: rgba(102,126,234,.14); color: #7187fb; font-size: 11px; font-weight: 700; }
.button-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-bottom: 10px; }
.path-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 6px; margin: 8px 0; }
.path-card { display: flex; flex-direction: column; gap: 3px; padding: 9px; margin-bottom: 9px; border-radius: 9px; background: var(--guaxi-header-bg); border: 1px solid var(--guaxi-border); }
.path-card strong { font-size: 12px; }
.path-card span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--guaxi-text-secondary); font-size: 10px; }
.path-card small { font-size: 9px; opacity: .7; }
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.form-grid :deep(.n-input-number) { width: 100%; }
.panel-section :deep(.n-form-item) { margin-bottom: 8px; }
.compact-alert { margin: 8px 0; font-size: 10px; }
.bridge-box { padding: 9px; margin: 8px 0; border-radius: 9px; border: 1px solid var(--guaxi-border); background: var(--guaxi-header-bg); }
.bridge-title { display: flex; align-items: center; justify-content: space-between; margin-bottom: 7px; font-size: 11px; font-weight: 600; }
.bridge-fields { display: grid; grid-template-columns: 82px minmax(0, 1fr) auto; gap: 5px; margin-bottom: 6px; }
.bridge-fields :deep(.n-input-number) { width: 100%; }
.project-actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
.job-card { margin-top: 10px; padding: 10px; border-radius: 10px; background: var(--guaxi-header-bg); border: 1px solid var(--guaxi-border); display: flex; flex-direction: column; gap: 7px; }
.job-line { display: flex; justify-content: space-between; font-size: 11px; }
.job-card small { color: var(--guaxi-text-secondary); }

.source-editor-wrap, .json-editor-wrap { height: 100%; display: flex; flex-direction: column; padding: 14px; box-sizing: border-box; }
.editor-toolbar { min-height: 34px; display: flex; align-items: center; justify-content: space-between; gap: 16px; font-size: 12px; }
.editor-toolbar small { color: var(--guaxi-text-secondary); }
.source-editor, .json-editor { flex: 1; min-height: 0; }
.source-editor :deep(textarea), .json-editor :deep(textarea) { height: 100% !important; resize: none; line-height: 1.65; font-family: "Cascadia Code", "Microsoft YaHei UI", monospace; }
.json-editor :deep(textarea) { font-size: 12px; }

.empty-state { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; color: var(--guaxi-text-secondary); }
.empty-state.compact { min-height: 260px; }
.empty-state h2 { margin: 8px 0 4px; color: var(--guaxi-text-color); font-size: 17px; }
.empty-state p { max-width: 460px; font-size: 12px; }
.empty-icon { font-size: 44px; filter: grayscale(.2); }

.storyboard-content, .pipeline-board, .log-list { height: 100%; box-sizing: border-box; padding: 16px; }
.metric-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin-bottom: 14px; }
.metric-card { padding: 14px; border-radius: 12px; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); display: flex; flex-direction: column; }
.metric-card strong { font-size: 22px; color: #758aff; }
.metric-card span { color: var(--guaxi-text-secondary); font-size: 10px; }
.scene-card { margin-top: 12px; padding: 16px; border-radius: 14px; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); }
.scene-header { display: flex; justify-content: space-between; gap: 12px; align-items: center; }
.scene-header h3 { margin: 2px 0; font-size: 16px; }
.scene-index { font-size: 9px; color: #758aff; letter-spacing: .12em; }
.scene-card > p { color: var(--guaxi-text-secondary); font-size: 12px; }
.shot-list { display: flex; flex-direction: column; gap: 6px; }
.shot-row { display: grid; grid-template-columns: 24px 1fr auto; gap: 10px; padding: 9px; align-items: start; border-radius: 9px; background: var(--guaxi-content-bg); }
.shot-number { width: 22px; height: 22px; border-radius: 7px; display: grid; place-items: center; background: rgba(102,126,234,.13); font-size: 10px; }
.shot-row strong { font-size: 11px; text-transform: uppercase; }
.shot-row p { margin: 2px 0; font-size: 11px; }
.shot-row small { color: var(--guaxi-text-secondary); font-size: 10px; }

.pipeline-board { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.pipeline-stage { display: grid; grid-template-columns: 40px 1fr; gap: 12px; padding: 14px; border-radius: 13px; background: var(--guaxi-panel-bg); border: 1px solid var(--guaxi-border); }
.stage-index { color: #758aff; font: 700 18px/1 monospace; }
.pipeline-stage h3 { margin: 0; font-size: 13px; }
.pipeline-stage p { color: var(--guaxi-text-secondary); font-size: 11px; line-height: 1.55; }
.stage-tags { display: flex; gap: 4px; flex-wrap: wrap; }

.inspector-panel section { margin-bottom: 22px; }
.inspector-title { margin-bottom: 9px; font-size: 11px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; color: var(--guaxi-text-secondary); }
.principle-card { padding: 12px; margin-bottom: 8px; border-radius: 11px; border: 1px solid var(--guaxi-border); background: var(--guaxi-content-bg); }
.principle-card.accent { border-color: rgba(102,126,234,.4); background: rgba(102,126,234,.08); }
.principle-card strong { font-size: 11px; }
.principle-card p { margin: 5px 0 0; color: var(--guaxi-text-secondary); font-size: 10px; line-height: 1.55; }
.check-list { margin: 0; padding-left: 18px; color: var(--guaxi-text-secondary); font-size: 10px; line-height: 1.75; }
.project-facts { margin: 0; }
.project-facts div { display: grid; grid-template-columns: 68px 1fr; gap: 8px; padding: 7px 0; border-bottom: 1px solid var(--guaxi-border); font-size: 10px; }
.project-facts dt { color: var(--guaxi-text-secondary); }
.project-facts dd { margin: 0; text-align: right; overflow-wrap: anywhere; }

.log-list { display: flex; flex-direction: column; gap: 6px; }
.log-row { display: grid; grid-template-columns: 72px 54px 1fr; gap: 8px; padding: 9px; border-radius: 8px; background: var(--guaxi-panel-bg); border-left: 3px solid #7891ff; font-size: 10px; }
.log-row.is-error { border-left-color: #e25757; }
.log-row.is-warn { border-left-color: #d5a139; }
.log-row span, .log-row small { color: var(--guaxi-text-secondary); }
.log-row p { margin: 0 0 3px; }

@media (max-width: 1180px) {
  .studio-layout { grid-template-columns: 310px minmax(500px, 1fr); }
  .inspector-panel { display: none; }
}
</style>
