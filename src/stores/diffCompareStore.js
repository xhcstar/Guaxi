import { defineStore } from 'pinia'
import { createLogger } from '../utils/logger'

const logger = createLogger('diffCompareStore')

const normalize = (p) => String(p || '').replace(/\\/g, '/')
const baseName = (p) => normalize(p).split('/').filter(Boolean).pop() || normalize(p)

/**
 * 差异对比面板的唯一状态源。
 *
 * 之所以单独开一个 store 而不是把状态塞进 GuaxiEditor：入口散落在文件树右键、
 * 标签右键、编辑器工具栏三处不同组件里，走 store 才不用层层透传事件。
 * "选择以进行比较 → 与已选比较" 的两步流程也需要一个跨组件存活的暂存位。
 */
export const useDiffCompareStore = defineStore('diffCompare', {
  state: () => ({
    visible: false,
    /** 'file' | 'dir' | 'text' | 'buffer' */
    mode: 'file',
    /** { path, label, text } —— text 只在 mode==='text' 时有意义 */
    left: { path: '', label: '', text: null },
    right: { path: '', label: '', text: null },
    /** 第一步"选择以进行比较"暂存的一侧：{ path, isDir } */
    pending: null,
    /** 每次发起对比自增，面板 watch 它决定是否重新计算 */
    requestSeq: 0,
    /** 面板高度由编辑器侧持久化，这里只存一个共享值 */
    panelHeight: 420,
  }),

  getters: {
    hasPending: (state) => !!state.pending?.path,
    pendingLabel: (state) => (state.pending?.path ? baseName(state.pending.path) : ''),
    /** 菜单项文案里带上已选文件名，避免用户忘了自己选了哪个 */
    compareWithLabel: (state) =>
      state.pending?.path ? `与"${baseName(state.pending.path)}"比较` : '与已选比较',
    leftLabel: (state) => state.left.label || baseName(state.left.path),
    rightLabel: (state) => state.right.label || baseName(state.right.path),
  },

  actions: {
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
      this.mode = 'file'
      this.left = { path: String(leftPath || ''), label: '', text: null }
      this.right = { path: String(rightPath || ''), label: '', text: null }
      this.visible = true
      this.requestSeq += 1
      logger.info('[Diff] compare files', { leftPath, rightPath })
    },

    compareDirs(leftRoot, rightRoot) {
      this.mode = 'dir'
      this.left = { path: String(leftRoot || ''), label: '', text: null }
      this.right = { path: String(rightRoot || ''), label: '', text: null }
      this.visible = true
      this.requestSeq += 1
      logger.info('[Diff] compare dirs', { leftRoot, rightRoot })
    },

    /** 未落盘的内容（编辑器缓冲、选区、剪贴板）走文本对比。 */
    compareTexts(leftText, rightText, leftLabel = '左侧', rightLabel = '右侧') {
      this.mode = 'text'
      this.left = { path: '', label: leftLabel, text: String(leftText ?? '') }
      this.right = { path: '', label: rightLabel, text: String(rightText ?? '') }
      this.visible = true
      this.requestSeq += 1
    },

    /**
     * 编辑器缓冲区 vs 磁盘内容。
     * 单独一个 mode 而不是复用 'text'：磁盘那一侧交给 Rust 直接读，
     * 大文件不必先整份塞进 JS 再传回去。左边是磁盘，右边是缓冲区（改动方在右，和文件对比一致）。
     */
    compareBufferWithDisk(path, bufferText) {
      const name = baseName(path)
      this.mode = 'buffer'
      this.left = { path: String(path || ''), label: `${name}（磁盘）`, text: null }
      this.right = { path: String(path || ''), label: `${name}（编辑器）`, text: String(bufferText ?? '') }
      this.visible = true
      this.requestSeq += 1
      logger.info('[Diff] compare buffer vs disk', { path })
    },

    /** 只填一侧就打开面板，另一侧留给用户在面板里挑。 */
    openWith(path, isDir = false, side = 'left') {
      this.mode = isDir ? 'dir' : 'file'
      const slot = { path: String(path || ''), label: '', text: null }
      const blank = { path: '', label: '', text: null }
      if (side === 'right') {
        this.left = blank
        this.right = slot
      } else {
        this.left = slot
        this.right = blank
      }
      this.visible = true
    },

    swap() {
      const l = this.left
      this.left = this.right
      this.right = l
      if (this.left.path && this.right.path) this.requestSeq += 1
    },

    setSide(side, path) {
      const slot = { path: String(path || ''), label: '', text: null }
      if (side === 'right') this.right = slot
      else this.left = slot
    },

    rerun() {
      this.requestSeq += 1
    },

    open() {
      this.visible = true
    },

    close() {
      this.visible = false
    },

    toggle() {
      this.visible = !this.visible
    },

    setPanelHeight(h) {
      const next = Number(h)
      if (Number.isFinite(next)) this.panelHeight = Math.round(next)
    },
  },
})

export const diffPathBaseName = baseName
