import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'

const logger = createLogger('slidevEditorStore')

const classifyReadFileError = (error) => {
  const msg = String(error?.message || error || '')
  const lower = msg.toLowerCase()

  if (
    lower.includes('stream did not contain valid utf-8') ||
    lower.includes('invalid utf') ||
    lower.includes('utf-8') ||
    lower.includes('the data is invalid')
  ) {
    return 'UNSUPPORTED'
  }

  if (
    lower.includes('os error 2') ||
    lower.includes('os error 3') ||
    lower.includes('not found') ||
    lower.includes('no such file') ||
    lower.includes('cannot find the file') ||
    lower.includes('系统找不到') ||
    lower.includes('文件不存在')
  ) {
    return 'NOT_FOUND'
  }

  return 'READ_FAILED'
}

const normalizeTabPath = (p) => String(p || '').replace(/\\/g, '/').toLowerCase()

const fileNameOf = (p) => String(p || '').split(/[\\/]/).pop() || String(p || '')

const generatePreview = (content) => {
  const s = String(content ?? '')
  return s.length > 80 ? `${s.slice(0, 80)}...` : s
}

export const useSlidevEditorStore = defineStore('slidevEditorStore', {
  state: () => ({
    openTabs: [],
    activeTab: null,
    selectedFile: null,
  }),

  actions: {
    reset() {
      logger.info('[reset]')
      this.openTabs = []
      this.activeTab = null
      this.selectedFile = null
    },

    setActiveTab(path) {
      const desiredKey = normalizeTabPath(path)
      const resolved = path
        ? (Array.isArray(this.openTabs) ? this.openTabs : []).find((t) => normalizeTabPath(t?.path) === desiredKey)
        : null

      const finalPath = resolved?.path || path || null
      this.activeTab = finalPath

      if (!finalPath) {
        this.selectedFile = null
        return
      }

      const tab = (Array.isArray(this.openTabs) ? this.openTabs : []).find(
        (t) => normalizeTabPath(t?.path) === normalizeTabPath(finalPath)
      )

      if (tab) {
        this.selectedFile = {
          path: tab.path,
          content: tab.content,
          unsupported: !!tab.unsupported,
        }
      } else if (this.selectedFile?.path !== finalPath) {
        this.selectedFile = { path: finalPath, content: '', unsupported: false }
      }

      logger.debug('[active] set', { path: finalPath })
    },

    async fetchFileContent(path) {
      const p = String(path || '')
      if (!p) return { ok: false, code: 'EMPTY_PATH', error: 'empty path' }

      try {
        const content = await invoke('read_file_content', { path: p })
        const text = String(content ?? '')
        this.selectedFile = { path: p, content: text, unsupported: false }
        return { ok: true, content: text, unsupported: false }
      } catch (error) {
        const code = classifyReadFileError(error)
        const errorMessage = String(error?.message || error || '')

        if (code === 'UNSUPPORTED') {
          const placeholder =
            `该文件为二进制文件或包含非 UTF-8 内容，当前不支持在编辑器中打开。\n\n路径：${p}\n\n你仍可在文件浏览器中打开它进行查看。`
          this.selectedFile = {
            path: p,
            content: placeholder,
            unsupported: true,
            error: errorMessage,
          }
          return { ok: true, content: placeholder, unsupported: true }
        }

        logger.error('[read] failed', { path: p, code, errorMessage })
        return { ok: false, code, error: errorMessage }
      }
    },

    async openFileInTab(path, content, options = {}) {
      const p = String(path || '')
      if (!p) return

      const desiredKey = normalizeTabPath(p)
      const existing = (Array.isArray(this.openTabs) ? this.openTabs : []).find(
        (t) => normalizeTabPath(t?.path) === desiredKey
      )

      if (existing) {
        this.setActiveTab(existing.path)
        return
      }

      let fileContent = content
      let unsupported = !!options.unsupported

      if (fileContent === undefined || fileContent === null) {
        const res = await this.fetchFileContent(p)
        if (!res?.ok) return
        fileContent = res.content
        unsupported = !!res.unsupported
      }

      const text = String(fileContent ?? '')
      const tab = {
        path: p,
        fileName: fileNameOf(p),
        content: text,
        savedContent: text,
        dirty: false,
        preview: generatePreview(text),
        unsupported,
      }

      this.openTabs.push(tab)
      this.setActiveTab(p)

      logger.info('[tab] opened', { path: p, unsupported: !!unsupported, len: text.length })
    },

    applyDraft(path, content) {
      const p = normalizeTabPath(path)
      if (!p) return

      const tab = (Array.isArray(this.openTabs) ? this.openTabs : []).find((t) => normalizeTabPath(t?.path) === p)
      if (!tab) return

      const next = String(content ?? '')
      tab.content = next
      tab.preview = generatePreview(next)
      tab.dirty = String(tab.savedContent ?? '') !== next

      if (this.selectedFile && normalizeTabPath(this.selectedFile.path) === p) {
        this.selectedFile.content = next
      }
    },

    markClean(path) {
      const p = normalizeTabPath(path)
      if (!p) return

      const tab = (Array.isArray(this.openTabs) ? this.openTabs : []).find((t) => normalizeTabPath(t?.path) === p)
      if (!tab) return

      tab.savedContent = String(tab.content ?? '')
      tab.dirty = false
    },

    async updateFileContent(path, content) {
      const p = String(path || '')
      if (!p) return

      // 路径已不在打开列表（被删除/改名）：不要再写盘，否则会把旧文件重新创建出来。
      const key = normalizeTabPath(p)
      const tracked = (Array.isArray(this.openTabs) ? this.openTabs : [])
        .some((t) => normalizeTabPath(t?.path) === key)
      if (!tracked) {
        logger.warn('[save] skip write for untracked path', { path: p })
        return
      }

      const next = String(content ?? '')
      logger.info('[save] write', { path: p, len: next.length })

      try {
        await invoke('write_file_content', { path: p, content: next })

        const tab = (Array.isArray(this.openTabs) ? this.openTabs : []).find((t) => normalizeTabPath(t?.path) === key)
        if (tab) {
          tab.content = next
          tab.savedContent = next
          tab.dirty = false
          tab.preview = generatePreview(next)
        }

        if (this.selectedFile && normalizeTabPath(this.selectedFile.path) === key) {
          this.selectedFile.content = next
          this.selectedFile.unsupported = false
        }
      } catch (error) {
        const errorMessage = String(error?.message || error || '')
        logger.error('[save] failed', { path: p, errorMessage })
        throw error
      }
    },
  },
})