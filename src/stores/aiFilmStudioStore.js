import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createLogger } from '../utils/logger'

const logger = createLogger('AIFilmStudioStore')
let unlistenJobEvent = null

const emptyBlenderStatus = () => ({
  installed: false,
  compatible: false,
  targetVersion: '5.2 LTS',
  executable: '',
  detectedVersion: '',
  isPrerelease: false,
  pluginDirectory: '',
  runnerPath: '',
  warnings: [],
})

const emptyMediaToolsStatus = () => ({
  ffmpegAvailable: false,
  ffmpegPath: '',
  ffmpegVersion: '',
  warnings: [],
})

/**
 * AI 影像工坊状态中心。
 *
 * 设计说明：独立窗口重新挂载时，Pinia 会保持同一窗口内的制作状态；持久化项目则交给
 * Tauri 后端写入 guaxi-film-project.json、source.md 和 story_ir.json，避免把长篇文稿塞入 localStorage。
 */
export const useAIFilmStudioStore = defineStore('aiFilmStudio', {
  state: () => ({
    initialized: false,
    knowledgeRoot: '',
    sourcePath: '',
    sourceName: '',
    sourceContent: '',
    sourceByteLength: 0,
    outputRoot: '',
    projectName: '',
    modelId: 9,
    sessionId: '',
    genre: 'history',
    visualStyle: 'cinematic-realism',
    productionMode: 'hybrid-3d-ai',
    targetDurationSeconds: 60,
    additionalDirection: '',
    storyIr: null,
    compileWarnings: [],
    blenderPath: '',
    blenderStatus: emptyBlenderStatus(),
    ffmpegPath: '',
    assetLibraryRoot: '',
    mediaToolsStatus: emptyMediaToolsStatus(),
    checkingMediaTools: false,
    bridgePort: 19152,
    bridgeToken: '',
    bridgeConnected: false,
    bridgeInfo: null,
    probingBridge: false,
    project: null,
    currentJobId: '',
    jobStatus: 'idle',
    jobStage: '',
    jobProgress: 0,
    jobMessage: '',
    logs: [],
    loadingSource: false,
    compiling: false,
    creatingProject: false,
    checkingBlender: false,
    startingPreview: false,
  }),

  getters: {
    hasSource: (state) => Boolean(state.sourceContent.trim()),
    hasStoryIr: (state) => Boolean(state.storyIr && typeof state.storyIr === 'object'),
    canCreateProject() {
      return this.hasSource && this.hasStoryIr && !this.creatingProject
    },
    canStartPreview: (state) => Boolean(
      state.project?.projectDir
      && state.blenderStatus.compatible
      && !['running', 'cancelling'].includes(state.jobStatus),
    ),
    sceneCount: (state) => Array.isArray(state.storyIr?.scenes) ? state.storyIr.scenes.length : 0,
    shotCount: (state) => Array.isArray(state.storyIr?.scenes)
      ? state.storyIr.scenes.reduce((sum, scene) => sum + (Array.isArray(scene?.shots) ? scene.shots.length : 0), 0)
      : 0,
  },

  actions: {
    appendLog(level, message, details = null) {
      this.logs.unshift({
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        time: new Date().toLocaleTimeString(),
        level,
        message,
        details,
      })
      if (this.logs.length > 200) this.logs.length = 200
    },

    async initialize() {
      if (this.initialized) return
      this.initialized = true
      // 先恢复持久化的可执行文件路径与资产库，再据此探测环境。
      await this.loadFilmStudioConfig()
      try {
        this.knowledgeRoot = String(await invoke('get_knowledge_bases_root') || '')
        this.outputRoot = this.knowledgeRoot
        this.appendLog('info', '已将知识库设为默认文稿选择位置', this.knowledgeRoot)
      } catch (error) {
        logger.warn('[FilmStudio] knowledge root unavailable', error)
        this.appendLog('warn', '无法读取知识库根目录，仍可从任意位置选择 Markdown')
      }
      await this.setupJobListener()
      await Promise.all([this.refreshBlenderStatus(), this.refreshMediaToolsStatus()])
    },

    async loadFilmStudioConfig() {
      try {
        const config = await invoke('get_film_studio_config')
        this.blenderPath = String(config?.blenderPath || '')
        this.ffmpegPath = String(config?.ffmpegPath || '')
        this.assetLibraryRoot = String(config?.assetLibraryRoot || '')
        if (this.blenderPath || this.ffmpegPath || this.assetLibraryRoot) {
          this.appendLog('info', '已恢复影像工坊持久化配置', {
            blenderPath: this.blenderPath,
            ffmpegPath: this.ffmpegPath,
            assetLibraryRoot: this.assetLibraryRoot,
          })
        }
      } catch (error) {
        logger.warn('[FilmStudio] film studio config unavailable', error)
      }
    },

    async persistFilmStudioConfig() {
      // 每次修改 Blender/FFmpeg/资产库路径后调用，写入 guaxi_config.json，重启后保留。
      try {
        await invoke('set_film_studio_config', {
          blenderPath: this.blenderPath || '',
          ffmpegPath: this.ffmpegPath || '',
          assetLibraryRoot: this.assetLibraryRoot || '',
        })
      } catch (error) {
        logger.error('[FilmStudio] persist film studio config failed', error)
        this.appendLog('warn', '保存影像工坊配置失败，重启后可能需要重新选择路径', String(error))
      }
    },

    async setupJobListener() {
      if (unlistenJobEvent) return
      unlistenJobEvent = await listen('story-studio-job', ({ payload }) => {
        const event = payload || {}
        if (this.currentJobId && event.jobId !== this.currentJobId) return
        this.currentJobId = String(event.jobId || this.currentJobId || '')
        this.jobStatus = String(event.status || this.jobStatus || 'idle')
        this.jobStage = String(event.stage || '')
        this.jobProgress = Math.max(0, Math.min(100, Number(event.progress || 0)))
        this.jobMessage = String(event.message || '')
        if (event.previewPath && this.project) this.project.previewPath = String(event.previewPath)
        this.appendLog(
          this.jobStatus === 'failed' ? 'error' : this.jobStatus === 'cancelled' ? 'warn' : 'info',
          this.jobMessage || `Blender 状态：${this.jobStatus}`,
          { stage: this.jobStage, progress: this.jobProgress },
        )
        logger.info('[FilmStudio] blender job event', event)
      })
    },

    async loadMarkdown(path) {
      this.loadingSource = true
      try {
        const source = await invoke('story_studio_read_markdown', { path })
        this.sourcePath = String(source?.path || path)
        this.sourceName = String(source?.fileName || '未命名文稿.md')
        this.sourceContent = String(source?.content || '')
        this.sourceByteLength = Number(source?.byteLength || 0)
        this.projectName = this.sourceName.replace(/\.(md|markdown|mdown)$/i, '') || 'Guaxi影像项目'
        this.storyIr = null
        this.project = null
        this.compileWarnings = []
        this.appendLog('info', 'Markdown 文稿加载完成', {
          path: this.sourcePath,
          bytes: this.sourceByteLength,
        })
        logger.info('[FilmStudio] markdown loaded', { path: this.sourcePath, bytes: this.sourceByteLength })
        return source
      } catch (error) {
        this.appendLog('error', '加载 Markdown 失败', String(error))
        logger.error('[FilmStudio] markdown load failed', error)
        throw error
      } finally {
        this.loadingSource = false
      }
    },

    async openProject(path) {
      try {
        const opened = await invoke('story_studio_open_project', { path })
        this.project = opened?.project || null
        this.sourcePath = String(opened?.sourcePath || opened?.project?.sourceCopyPath || '')
        this.sourceName = this.sourcePath.split(/[\\/]/).pop() || `${opened?.project?.projectName || '项目'}.md`
        this.sourceContent = String(opened?.sourceContent || '')
        this.sourceByteLength = new TextEncoder().encode(this.sourceContent).length
        this.storyIr = opened?.storyIr || null
        this.projectName = String(opened?.project?.projectName || 'Guaxi影像项目')
        this.compileWarnings = []
        this.jobStatus = 'idle'
        this.currentJobId = ''
        this.appendLog('info', '已恢复现有影像项目', opened?.project?.projectDir)
        logger.info('[FilmStudio] existing project opened', opened?.project)
        return opened
      } catch (error) {
        this.appendLog('error', '打开影像项目失败', String(error))
        throw error
      }
    },

    async compileStoryIr() {
      if (!this.hasSource) throw new Error('请先加载 Markdown 文稿')
      this.compiling = true
      this.compileWarnings = []
      try {
        this.appendLog('info', '开始调用所选模型编译 Story IR', { modelId: Number(this.modelId) })
        const response = await invoke('story_studio_compile_markdown', {
          request: {
            modelid: Number(this.modelId),
            sessionid: this.sessionId,
            sourceName: this.sourceName,
            markdown: this.sourceContent,
            genre: this.genre,
            visualStyle: this.visualStyle,
            productionMode: this.productionMode,
            targetDurationSeconds: Number(this.targetDurationSeconds || 60),
            additionalDirection: this.additionalDirection,
          },
        })
        this.storyIr = response?.storyIr || null
        this.sessionId = String(response?.sessionid || '')
        this.compileWarnings = Array.isArray(response?.warnings) ? response.warnings : []
        this.project = null
        this.appendLog('info', 'Story IR 编译完成', {
          scenes: this.sceneCount,
          shots: this.shotCount,
          warnings: this.compileWarnings.length,
        })
        logger.info('[FilmStudio] Story IR compiled', { scenes: this.sceneCount, shots: this.shotCount })
        return response
      } catch (error) {
        this.appendLog('error', 'Story IR 编译失败', String(error))
        logger.error('[FilmStudio] Story IR compilation failed', error)
        throw error
      } finally {
        this.compiling = false
      }
    },

    replaceStoryIr(value) {
      if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw new Error('Story IR 根节点必须是 JSON 对象')
      }
      this.storyIr = value
      this.project = null
      this.appendLog('info', '已应用手工编辑后的 Story IR')
    },

    async refreshBlenderStatus() {
      this.checkingBlender = true
      try {
        const status = await invoke('story_studio_get_blender_status', {
          blenderPath: this.blenderPath || null,
        })
        this.blenderStatus = { ...emptyBlenderStatus(), ...(status || {}) }
        if (status?.executable) {
          this.blenderPath = String(status.executable)
          // 探测到的可执行文件也持久化，避免下次重启重新手动选择。
          await this.persistFilmStudioConfig()
        }
        this.appendLog(status?.compatible ? 'info' : 'warn', status?.compatible
          ? `Blender 环境可用：${status.detectedVersion}`
          : 'Blender 5.2 LTS 环境尚未就绪', status?.warnings || [])
        logger.info('[FilmStudio] blender status refreshed', status)
        return status
      } catch (error) {
        this.blenderStatus = emptyBlenderStatus()
        this.appendLog('error', '检测 Blender 环境失败', String(error))
        logger.error('[FilmStudio] blender status failed', error)
        throw error
      } finally {
        this.checkingBlender = false
      }
    },

    async refreshMediaToolsStatus() {
      this.checkingMediaTools = true
      try {
        const status = await invoke('story_studio_get_media_tools_status', {
          ffmpegPath: this.ffmpegPath || null,
        })
        this.mediaToolsStatus = { ...emptyMediaToolsStatus(), ...(status || {}) }
        if (status?.ffmpegPath) {
          this.ffmpegPath = String(status.ffmpegPath)
          await this.persistFilmStudioConfig()
        }
        this.appendLog(status?.ffmpegAvailable ? 'info' : 'warn', status?.ffmpegAvailable
          ? `FFmpeg 编码器可用：${status.ffmpegVersion}`
          : '未检测到独立 FFmpeg，将保留 PNG 序列等待编码', status?.warnings || [])
        return status
      } catch (error) {
        this.mediaToolsStatus = emptyMediaToolsStatus()
        this.appendLog('error', '检测 FFmpeg 失败', String(error))
        throw error
      } finally {
        this.checkingMediaTools = false
      }
    },

    async probeBridge() {
      this.probingBridge = true
      try {
        const info = await invoke('story_studio_probe_blender_bridge', { port: Number(this.bridgePort) })
        this.bridgeConnected = Boolean(info?.ok)
        this.bridgeInfo = info || null
        this.appendLog('info', `已连接交互式 Blender Bridge：${info?.blenderVersion || '5.2'}`)
        logger.info('[FilmStudio] interactive bridge connected', info)
        return info
      } catch (error) {
        this.bridgeConnected = false
        this.bridgeInfo = null
        this.appendLog('warn', '未连接到交互式 Blender Bridge', String(error))
        throw error
      } finally {
        this.probingBridge = false
      }
    },

    async sendStoryToBridge() {
      if (!this.project?.projectDir || !this.hasStoryIr) {
        throw new Error('请先创建标准制作项目')
      }
      const requestId = globalThis.crypto?.randomUUID?.() || `guaxi-${Date.now()}`
      const response = await invoke('story_studio_send_blender_commands', {
        request: {
          port: Number(this.bridgePort),
          token: this.bridgeToken,
          payload: {
            protocolVersion: '1.0',
            requestId,
            operations: [{
              // 资产化路径：解析真实资产 + 冻结 Build Plan（§7.2/§7.3）。
              type: 'build_story_assets',
              storyIr: this.storyIr,
              projectDir: this.project.projectDir,
              renderAnimation: false,
            }],
          },
        },
      })
      this.appendLog('info', 'Story IR 已发送到当前打开的 Blender', { requestId })
      logger.info('[FilmStudio] story sent to interactive bridge', { requestId })
      return response
    },

    async createProject() {
      if (!this.canCreateProject) throw new Error('请先加载文稿并生成 Story IR')
      this.creatingProject = true
      try {
        const project = await invoke('story_studio_create_project', {
          request: {
            projectName: this.projectName,
            sourcePath: this.sourcePath,
            sourceContent: this.sourceContent,
            storyIr: this.storyIr,
            outputRoot: this.outputRoot,
          },
        })
        this.project = project
        this.appendLog('info', '影像项目目录创建完成', project?.projectDir)
        logger.info('[FilmStudio] project created', project)
        return project
      } catch (error) {
        this.appendLog('error', '创建影像项目失败', String(error))
        logger.error('[FilmStudio] project creation failed', error)
        throw error
      } finally {
        this.creatingProject = false
      }
    },

    async startPreview(renderAnimation = true, builder = 'assets') {
      if (!this.canStartPreview) throw new Error('项目或 Blender 5.2 LTS 环境尚未就绪')
      this.startingPreview = true
      this.jobStatus = 'launching'
      this.jobProgress = 0
      try {
        // builder：assets（真实资产 + 冻结 Build Plan）、greybox（旧立方体预演）、
        // replay（重放已冻结的 build_plan.json，确定性重渲染）。
        const jobId = await invoke('story_studio_start_preview', {
          request: {
            projectDir: this.project.projectDir,
            blenderPath: this.blenderPath,
            ffmpegPath: this.ffmpegPath,
            assetLibraryRoot: this.assetLibraryRoot,
            renderAnimation,
            builder,
          },
        })
        this.currentJobId = String(jobId || '')
        this.jobStatus = 'running'
        this.appendLog('info', renderAnimation ? '已提交 Blender MP4 预演任务' : '已提交 Blender 工程构建任务', {
          jobId: this.currentJobId,
          builder,
        })
        return jobId
      } catch (error) {
        this.jobStatus = 'failed'
        this.appendLog('error', '启动 Blender 任务失败', String(error))
        logger.error('[FilmStudio] start preview failed', error)
        throw error
      } finally {
        this.startingPreview = false
      }
    },

    async cancelJob() {
      if (!this.currentJobId || this.jobStatus !== 'running') return
      this.jobStatus = 'cancelling'
      try {
        await invoke('story_studio_cancel_job', { jobId: this.currentJobId })
        this.appendLog('warn', '已请求取消 Blender 任务', { jobId: this.currentJobId })
      } catch (error) {
        this.jobStatus = 'running'
        this.appendLog('error', '取消 Blender 任务失败', String(error))
        throw error
      }
    },
  },
})
