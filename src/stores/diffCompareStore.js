import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'
import { useSettingsStore } from './settingsStore'

const logger = createLogger('diffCompareStore')

const normalize = (p) => String(p || '').replace(/\\/g, '/')
const baseName = (p) => normalize(p).split('/').filter(Boolean).pop() || normalize(p)
const blankSide = () => ({ path: '', label: '', text: null })

const MODES = ['file', 'dir', 'text', 'buffer']

/**
 * 上次对比的两侧路径。存 localStorage 而不是后端配置：
 * 各窗口同源共享，主窗口写、对比窗口读都是同一份，不用再走一次 IPC。
 */
const LAST_KEY = 'guaxi_diff_last_paths'

const readLast = () => {
  try {
    const raw = localStorage.getItem(LAST_KEY)
    if (!raw) return { mode: 'file', leftPath: '', rightPath: '' }
    const v = JSON.parse(raw) || {}
    return {
      mode: v.mode === 'dir' ? 'dir' : 'file',
      leftPath: String(v.leftPath || ''),
      rightPath: String(v.rightPath || ''),
    }
  } catch {
    return { mode: 'file', leftPath: '', rightPath: '' }
  }
}

const writeLast = (mode, leftPath, rightPath) => {
  // 只记路径型对比：text 没有路径，buffer 的两侧是同一个文件的两个版本，记了也没用。
  if (mode !== 'file' && mode !== 'dir') return
  if (!leftPath && !rightPath) return
  try {
    localStorage.setItem(LAST_KEY, JSON.stringify({ mode, leftPath, rightPath }))
  } catch {
    // 存不下就算了，只是少一个便利
  }
}

/**
 * 差异对比的唯一状态源。同一份代码在两种窗口里担两个角色：
 *
 * - 主窗口（host=false）：文件树右键、标签右键、工具栏按钮都只负责"提出请求"，
 *   请求交给 Rust，由它开出/唤醒那个独立的对比窗口。入口散落在三处组件里，
 *   走 store 才不用层层透传事件；"选择以进行比较 → 与已选比较"两步流程也需要
 *   一个跨组件存活的暂存位（pending）。
 * - 对比窗口（host=true，由 GuaxiDiffComparePage 调 becomeHost 打开这个开关）：
 *   请求原地落到 state，面板 watch requestSeq 重新算。
 *
 * Pinia 实例是按 webview 隔离的，两个窗口各有一份 state，所以跨窗口只能显式交接，
 * 不存在"自动同步"这回事。
 */
export const useDiffCompareStore = defineStore('diffCompare', {
  state: () => ({
    /** 当前 webview 是否就是那个独立对比窗口 */
    host: false,
    /** 'file' | 'dir' | 'text' | 'buffer' */
    mode: 'file',
    /** { path, label, text } —— text 只在 text / buffer 模式下有意义 */
    left: blankSide(),
    right: blankSide(),
    /** 第一步"选择以进行比较"暂存的一侧：{ path, isDir } */
    pending: null,
    /** 每次发起对比自增，面板 watch 它决定是否重新计算 */
    requestSeq: 0,
    /** 这次请求要不要自动跑。预填路径的请求为 false，让用户自己点"对比" */
    autoRun: true,
  }),

  getters: {
    hasPending: (state) => !!state.pending?.path,
    pendingLabel: (state) => (state.pending?.path ? baseName(state.pending.path) : ''),
    /** 菜单项文案里带上已选文件名，避免用户忘了自己选了哪个 */
    compareWithLabel: (state) =>
      state.pending?.path ? `与"${baseName(state.pending.path)}"比较` : '与已选比较',
    leftLabel: (state) => state.left.label || baseName(state.left.path),
    rightLabel: (state) => state.right.label || baseName(state.right.path),
    /** 目录/文件两种路径模式之外（text、buffer）不允许在面板里切换类型 */
    isPathMode: (state) => state.mode === 'file' || state.mode === 'dir',
  },

  actions: {
    /** 由对比窗口的宿主页面调用，之后所有请求原地生效。 */
    becomeHost() {
      this.host = true
    },

    /** 把一次交接过来的请求落进 state（只有宿主窗口会走到这里）。 */
    applyRequest(req) {
      if (!req) return false
      const mode = String(req.mode || '')
      this.mode = MODES.includes(mode) ? mode : 'file'
      this.left = {
        path: String(req.leftPath || ''),
        label: String(req.leftLabel || ''),
        text: req.leftText ?? null,
      }
      this.right = {
        path: String(req.rightPath || ''),
        label: String(req.rightLabel || ''),
        text: req.rightText ?? null,
      }
      this.pending = null
      this.autoRun = req.autoRun !== false
      this.requestSeq += 1
      writeLast(this.mode, this.left.path, this.right.path)
      logger.info('[Diff] apply request', {
        mode: this.mode,
        left: this.left.path || this.left.label,
        right: this.right.path || this.right.label,
      })
      return true
    },

    /**
     * 所有对比入口的统一出口。
     * @param {object|null} request 见 Rust 侧 DiffWindowRequest；null 表示只把窗口叫到前台
     * @param {boolean} onlyIfNew true 时请求只作为新窗口的初始值，不覆盖已经开着的窗口内容
     */
    dispatch(request, onlyIfNew = false) {
      if (this.host) {
        if (request) this.applyRequest(request)
        return
      }
      let theme = ''
      try {
        theme = useSettingsStore().appearanceTheme || ''
      } catch (err) {
        logger.warn('[Diff] read theme failed', err)
      }
      invoke('open_diff_compare_window', {
        request: request || null,
        theme,
        onlyIfNew: !!onlyIfNew,
      }).catch((err) => logger.error('[Diff] open compare window failed', err))
    },

    /** 第一步：把某个文件/目录标记为待比较的左侧。 */
    markForCompare(path, isDir = false) {
      if (!path) return
      this.pending = { path: String(path), isDir: !!isDir }
      logger.info('[Diff] marked for compare', { path, isDir })
    },

    clearPending() {
      this.pending = null
    },

    /**
     * 第二步：用暂存的一侧作左边，当前项作右边。
     * 两边类型必须一致（文件对文件、目录对目录），否则返回错误说明由调用方提示。
     */
    compareWithPending(path, isDir = false) {
      if (!this.pending?.path) return { ok: false, error: '还没有选择用于比较的一方' }
      if (!path) return { ok: false, error: '目标路径无效' }
      if (!!isDir !== !!this.pending.isDir) {
        return { ok: false, error: '只能文件对文件、目录对目录地比较' }
      }
      const leftPath = this.pending.path
      this.pending = null
      if (isDir) this.compareDirs(leftPath, path)
      else this.compareFiles(leftPath, path)
      return { ok: true }
    },

    compareFiles(leftPath, rightPath) {
      this.dispatch({
        mode: 'file',
        leftPath: String(leftPath || ''),
        rightPath: String(rightPath || ''),
      })
    },

    compareDirs(leftRoot, rightRoot) {
      this.dispatch({
        mode: 'dir',
        leftPath: String(leftRoot || ''),
        rightPath: String(rightRoot || ''),
      })
    },

    /** 未落盘的内容（编辑器缓冲、选区、剪贴板）走文本对比。 */
    compareTexts(leftText, rightText, leftLabel = '左侧', rightLabel = '右侧') {
      this.dispatch({
        mode: 'text',
        leftLabel,
        rightLabel,
        leftText: String(leftText ?? ''),
        rightText: String(rightText ?? ''),
      })
    },

    /**
     * 编辑器缓冲区 vs 磁盘内容。
     * 单独一个 mode 而不是复用 'text'：磁盘那一侧交给 Rust 直接读，
     * 大文件不必先整份塞进 JS 再传回去。左边是磁盘，右边是缓冲区（改动方在右，和文件对比一致）。
     */
    compareBufferWithDisk(path, bufferText) {
      const name = baseName(path)
      this.dispatch({
        mode: 'buffer',
        leftPath: String(path || ''),
        rightPath: String(path || ''),
        leftLabel: `${name}（磁盘）`,
        rightLabel: `${name}（编辑器）`,
        rightText: String(bufferText ?? ''),
      })
    },

    /** 只填一侧就打开窗口，另一侧留给用户在窗口里挑。 */
    openWith(path, isDir = false, side = 'left') {
      const p = String(path || '')
      this.dispatch(
        {
          mode: isDir ? 'dir' : 'file',
          leftPath: side === 'right' ? '' : p,
          rightPath: side === 'right' ? p : '',
        },
        true,
      )
    },

    /** 两侧全空地打开窗口，纯靠用户自己挑文件或目录。 */
    openEmpty(kind = 'file') {
      this.dispatch({ mode: kind === 'dir' ? 'dir' : 'file' }, true)
    },

    /**
     * Header 里那个入口：开一个"什么都能挑"的对比窗口，两侧预填上次比过的路径。
     *
     * 预填之前先问一遍磁盘：上次的路径可能已经删了、改名了，或者上次比的是目录
     * 这次却剩一个文件——类型对不上就当没有，不能把坏路径塞进去让用户莫名报错。
     * 预填不自动跑（目录扫描太贵），文件对比两侧都还在时才顺手算出来。
     */
    async openLastUsed() {
      const last = readLast()
      const wanted = last.mode === 'dir' ? 'dir' : 'file'
      const paths = [last.leftPath, last.rightPath].filter(Boolean)
      let leftPath = ''
      let rightPath = ''
      if (paths.length) {
        try {
          const got = await invoke('diff_probe_paths', { paths })
          const alive = new Set(
            (got || []).filter((it) => it?.kind === wanted).map((it) => it.path),
          )
          if (alive.has(last.leftPath)) leftPath = last.leftPath
          if (alive.has(last.rightPath)) rightPath = last.rightPath
        } catch (err) {
          logger.warn('[Diff] probe last paths failed', err)
        }
      }
      this.dispatch(
        {
          mode: wanted,
          leftPath,
          rightPath,
          autoRun: wanted === 'file' && !!(leftPath && rightPath),
        },
        true,
      )
    },

    /**
     * 面板里切换"文件 / 目录"。已填的两侧类型对不上就得丢掉，
     * 顺带清掉 text / buffer 模式残留的文本，否则会拿旧文本去跑路径对比。
     */
    setMode(kind, { keepSides = false } = {}) {
      const next = kind === 'dir' ? 'dir' : 'file'
      const wasTextLike = this.mode === 'text' || this.mode === 'buffer'
      this.mode = next
      if (wasTextLike || !keepSides) {
        this.left = blankSide()
        this.right = blankSide()
      } else {
        this.left = { ...this.left, text: null }
        this.right = { ...this.right, text: null }
      }
    },

    swap() {
      const l = this.left
      this.left = this.right
      this.right = l
      writeLast(this.mode, this.left.path, this.right.path)
      if (this.left.path && this.right.path) this.requestSeq += 1
    },

    setSide(side, path) {
      const slot = { path: String(path || ''), label: '', text: null }
      if (side === 'right') this.right = slot
      else this.left = slot
      // 用户在窗口里挑的路径同样要记住，下次开窗直接接上。
      writeLast(this.mode, this.left.path, this.right.path)
    },

    rerun() {
      this.requestSeq += 1
    },
  },
})

export const diffPathBaseName = baseName
