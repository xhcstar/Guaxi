import { defineStore } from 'pinia'
import { createLogger } from '../utils/logger'
import { READER_KIND, resolveOpenTarget } from '../utils/openTargetKind'

const logger = createLogger('readerTabStore')

const normalizeKey = (path) => String(path || '').replace(/\\/g, '/')

/**
 * 阅读器标签登记表。
 *
 * 为什么单独建一个 store 而不是往 fileTreeStore 的 tab 上塞字段：
 * - `fileTreeStore.openTabs` 的每一项都会被 Monaco 绑定、自动保存、脏标记、导出等一整套
 *   既有逻辑消费。往里加一个 `readerTarget` 字段，等于让所有那些代码都要先判断「这是不是
 *   阅读器」—— 一处漏判就是回归。
 * - 这里用一张独立的 `path → target` 映射，消费方只有 ReaderPane 与几个打开入口。
 *   文本文件永远不会出现在这张表里，既有链路连一次查表都不会发生。
 *
 * 生命周期：
 * - `openReader(...)`：用户打开一个非文本文件时登记；
 * - `closeReader(path)`：标签关闭 / 不再是激活标签且被移除时清理；
 * - `has(path)`：ReaderPane 与打开入口用它做 O(1) 判定。
 */
export const useReaderTabStore = defineStore('readerTabStore', {
  state: () => ({
    /** @type {Record<string, {kind:string, reader:string, mime:string, size:number, container:boolean, forced:boolean}>} */
    targets: {},
    /** 进行中的判定请求，避免同一路径被连点多次时重复嗅探。 */
    pending: {},
    /**
     * 被用户显式要求「用编辑器打开」的路径。
     *
     * 为什么放在这个 store 而不是编辑器组件内部：再次从文件树打开同一文件时，
     * openReader 要能把这个标记清掉，否则阅读器永远不会再出现。
     */
    editorMode: {},
  }),

  getters: {
    /** 该路径是否登记为阅读器标签。 */
    has: (state) => (path) => !!state.targets[normalizeKey(path)],
    /** 取阅读器描述，未登记返回 null。 */
    targetFor: (state) => (path) => {
      const key = normalizeKey(path)
      return state.targets[key] || null
    },
    /** 该路径是否被用户强制切到了编辑器视图。 */
    isEditorMode: (state) => (path) => !!state.editorMode[normalizeKey(path)],
    /** 该路径是否需要外部阅读器（而非 Monaco）。 */
    isReaderPath: (state) => (path) => {
      const target = state.targets[normalizeKey(path)]
      return !!target && target.reader !== READER_KIND.TEXT
    },
    /** 当前登记的全部路径（调试/清理用）。 */
    allPaths: (state) => Object.keys(state.targets),
  },

  actions: {
    /**
     * 判定一个文件的打开方式并登记（如果需要阅读器）。
     *
     * @param {string} path 绝对路径
     * @param {'auto'|'editor'|'reader'} mode 用户意图
     * @returns {Promise<object>} resolveOpenTarget 的结果（reader === 'text' 表示应交给 Monaco）
     */
    async openReader(path, mode = 'auto') {
      const key = normalizeKey(path)
      if (!key) return null

      // 已登记：直接复用，不重复嗅探。
      if (this.targets[key]) return this.targets[key]

      // 同一路径正在判定：复用同一个 Promise。
      if (this.pending[key]) return this.pending[key]

      const task = (async () => {
        try {
          const target = await resolveOpenTarget(path, mode)
          if (target && target.reader !== READER_KIND.TEXT) {
            // 用户重新打开了这个文件：强制编辑器视图应当失效，否则覆盖层不会出现。
            if (this.editorMode[key]) delete this.editorMode[key]
            this.targets[key] = target
            logger.info('[readerTab] registered', {
              path: key,
              reader: target.reader,
              kind: target.kind,
              mode,
            })
          }
          return target
        } finally {
          delete this.pending[key]
        }
      })()

      this.pending[key] = task
      return task
    },

    /** 让某个阅读器标签临时切换到编辑器视图（登记保留，便于再次打开时恢复）。 */
    setEditorMode(path) {
      const key = normalizeKey(path)
      if (!key) return
      this.editorMode[key] = true
      logger.info('[readerTab] switched to editor view', { path: key })
    },

    /** 把已登记路径改判为文本（彻底不再当阅读器处理）。 */
    demoteToText(path) {
      const key = normalizeKey(path)
      if (this.targets[key]) {
        delete this.targets[key]
        logger.info('[readerTab] demoted to text', { path: key })
      }
      delete this.editorMode[key]
    },

    /** 移除登记。标签关闭、路径变更、文件删除时调用。 */
    closeReader(path) {
      const key = normalizeKey(path)
      if (!this.targets[key] && !this.editorMode[key]) return
      delete this.targets[key]
      delete this.editorMode[key]
      logger.debug('[readerTab] closed', { path: key })
    },

    /** 目录被整体删除/重命名时，批量清理其下所有登记。 */
    closeUnderPath(rootPath) {
      const root = normalizeKey(rootPath)
      if (!root) return
      let removed = 0
      for (const key of Object.keys(this.targets)) {
        if (key === root || key.startsWith(`${root}/`)) {
          delete this.targets[key]
          delete this.editorMode[key]
          removed += 1
        }
      }
      for (const key of Object.keys(this.editorMode)) {
        if (key === root || key.startsWith(`${root}/`)) delete this.editorMode[key]
      }
      if (removed) logger.info('[readerTab] closed under path', { root, removed })
    },

    /** 路径被重命名/移动后，把登记搬到新路径。 */
    remapPath(oldPath, newPath) {
      const from = normalizeKey(oldPath)
      const to = normalizeKey(newPath)
      if (!from || !to || from === to) return

      if (this.targets[from]) {
        this.targets[to] = this.targets[from]
        delete this.targets[from]
        logger.info('[readerTab] remapped', { from, to })
      }
      if (this.editorMode[from]) {
        this.editorMode[to] = true
        delete this.editorMode[from]
      }
      // 目录重命名：其下所有登记一起搬。
      for (const key of Object.keys(this.targets)) {
        if (!key.startsWith(`${from}/`)) continue
        const next = to + key.slice(from.length)
        this.targets[next] = this.targets[key]
        delete this.targets[key]
      }
      for (const key of Object.keys(this.editorMode)) {
        if (!key.startsWith(`${from}/`)) continue
        const next = to + key.slice(from.length)
        this.editorMode[next] = true
        delete this.editorMode[key]
      }
    },

    /** 清空（切换知识库/退出时用）。 */
    reset() {
      this.targets = {}
      this.pending = {}
      this.editorMode = {}
    },
  },
})
