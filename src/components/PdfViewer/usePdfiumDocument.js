// pdfium 文档数据层：文档生命周期 + 页模型 + 渲染/文本/大纲/搜索/定位 + 增量重建 diff。
// 所有后端调用收口于此；GuaxiPdfViewer.vue 负责 DOM/滚动/虚拟化，按需调用这里的方法。
// 注：高亮批注（写回 .pdf）在后续阶段加入，这里先提供阅读/预览所需的只读能力。

import { ref, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { buildLines } from './usePdfTextLayer'
import { createLogger } from '../../utils/logger'

const logger = createLogger('usePdfiumDocument')

export default function usePdfiumDocument() {
  // 页模型：num/尺寸 + 渲染缓存(image,dpi) + 文本(chars,lines) + 增量指纹(renderHash)。
  const pages = reactive([])
  const totalPages = ref(0)
  const hasPdf = ref(false)
  const status = ref('等待加载')
  const statusTone = ref('')

  let activePath = ''
  let loadToken = 0

  const makePage = (num, w, h) => ({
    num,
    wPt: w,
    hPt: h,
    image: '',
    dpi: 0,
    rendering: false,
    renderToken: 0,
    chars: null,
    lines: null,
    textLoading: false,
    scaled: false,
    renderHash: '',
    highlights: null,
    highlightsLoading: false,
  })

  const resetState = () => {
    pages.splice(0, pages.length)
    totalPages.value = 0
    hasPdf.value = false
    activePath = ''
  }

  // 增量对账：页数不变时，只对"已渲染过的页"做指纹 diff，未变页保留 image/lines 不闪。
  const incrementalReconcile = async (path, sizes, token) => {
    // 先按新尺寸更新；尺寸变化的页直接失效（必然要重渲）。
    for (const page of pages) {
      const sz = sizes[page.num - 1]
      if (!sz) continue
      if (Math.abs(sz.w_pt - page.wPt) > 0.5 || Math.abs(sz.h_pt - page.hPt) > 0.5) {
        page.wPt = sz.w_pt
        page.hPt = sz.h_pt
        page.image = ''
        page.dpi = 0
        page.chars = null
        page.lines = null
        page.scaled = false
        page.renderHash = ''
      }
    }
    // 只检查持有图片的页（可见/近期）——离屏页本就没图，懒渲即可，省掉 5000 次哈希调用。
    const loaded = pages.filter((p) => p.image)
    await Promise.all(
      loaded.map(async (page) => {
        try {
          const hash = await invoke('pdf_page_render_hash', { path, page: page.num })
          if (token !== loadToken) return
          if (!(page.renderHash && page.renderHash === hash)) {
            // 内容变化：失效，等可见时重渲。
            page.image = ''
            page.dpi = 0
            page.chars = null
            page.lines = null
            page.scaled = false
          }
          page.renderHash = hash
        } catch {
          /* 哈希失败则保持原样，下次可见时按需重渲 */
        }
      })
    )
  }

  /**
   * 打开/重建文档。
   * @param {string} fp 文档路径（可带 ?ts= 查询，会被去掉）。
   * @param {{keepExisting?:boolean}} [opts] keepExisting 且页数一致时走增量对账。
   */
  const openDocument = async (fp, { keepExisting = true } = {}) => {
    const path = String(fp || '').split('?')[0]
    const token = ++loadToken
    if (!path) {
      resetState()
      status.value = '等待加载'
      statusTone.value = ''
      return { ok: false }
    }
    status.value = '加载预览…'
    statusTone.value = ''
    try {
      const resp = await invoke('pdf_session_open', { path })
      if (token !== loadToken) return { ok: false, stale: true }
      const sizes = resp.pages || []
      const sameCount = keepExisting && pages.length > 0 && pages.length === sizes.length
      activePath = path
      if (sameCount) {
        await incrementalReconcile(path, sizes, token)
        if (token !== loadToken) return { ok: false, stale: true }
      } else {
        pages.splice(0, pages.length)
        sizes.forEach((p, i) => pages.push(makePage(i + 1, p.w_pt, p.h_pt)))
      }
      totalPages.value = resp.total_pages
      hasPdf.value = resp.total_pages > 0
      status.value = `就绪 · ${resp.total_pages} 页`
      statusTone.value = 'ok'
      return { ok: true, totalPages: resp.total_pages }
    } catch (e) {
      if (token !== loadToken) return { ok: false, stale: true }
      logger.error('打开 PDF 失败', { error: String(e?.message || e) })
      status.value = '加载失败'
      statusTone.value = 'error'
      return { ok: false, error: e }
    }
  }

  const closeDocument = async () => {
    loadToken += 1
    const path = activePath
    resetState()
    status.value = '等待加载'
    statusTone.value = ''
    if (path) {
      try {
        await invoke('pdf_session_close', { path })
      } catch {
        /* ignore */
      }
    }
  }

  // 渲染单页（按 dpi）。token 取消：快速滚动/缩放时丢弃过期结果。
  const renderPage = async (page, dpi) => {
    if (!activePath) return
    if (page.rendering && page.dpi === dpi) return
    if (page.image && page.dpi === dpi) return
    page.rendering = true
    const myToken = ++page.renderToken
    try {
      const resp = await invoke('pdf_render_page', {
        request: { path: activePath, page: page.num, dpi },
      })
      if (myToken !== page.renderToken) return
      page.image = `data:image/png;base64,${resp.image_data}`
      page.dpi = resp.dpi
    } catch (e) {
      logger.error('渲染页失败', { page: page.num, error: String(e?.message || e) })
    } finally {
      if (myToken === page.renderToken) page.rendering = false
    }
  }

  // 加载单页字符并分行（供文本层）。
  const loadText = async (page) => {
    if (!activePath) return
    if (page.lines || page.textLoading) return
    page.textLoading = true
    try {
      const resp = await invoke('pdf_page_chars', { path: activePath, page: page.num })
      page.chars = resp.chars || []
      page.lines = buildLines(page.chars)
    } catch (e) {
      page.chars = []
      page.lines = []
    } finally {
      page.textLoading = false
    }
  }

  const freePage = (page) => {
    page.image = ''
    page.dpi = 0
  }

  const getOutline = async () => {
    if (!activePath) return []
    try {
      return (await invoke('pdf_outline', { path: activePath })) || []
    } catch (e) {
      logger.warn('获取大纲失败', { error: String(e?.message || e) })
      return []
    }
  }

  const search = async (query) => {
    const q = String(query || '').trim()
    if (!q || !activePath) return { query: q, total: 0, hits: [] }
    try {
      return await invoke('pdf_search', { path: activePath, query: q })
    } catch (e) {
      logger.error('搜索失败', { error: String(e?.message || e) })
      return { query: q, total: 0, hits: [] }
    }
  }

  const locatePoint = async (page, x, y) => {
    if (!activePath) return null
    try {
      return await invoke('pdf_locate_point', { path: activePath, page, x, y })
    } catch {
      return null
    }
  }

  // ───────── 高亮批注（写回 .pdf）─────────
  const loadHighlights = async (page, { force = false } = {}) => {
    if (!activePath) return
    if (page.highlightsLoading) return
    if (page.highlights && !force) return
    page.highlightsLoading = true
    try {
      page.highlights = (await invoke('pdf_page_highlights', { path: activePath, page: page.num })) || []
    } catch (e) {
      page.highlights = []
    } finally {
      page.highlightsLoading = false
    }
  }

  const addHighlight = async ({ page, rects, color, note } = {}) => {
    if (!activePath || !page || !Array.isArray(rects) || !rects.length) return null
    try {
      const resp = await invoke('pdf_add_highlight', {
        request: { path: activePath, page, rects, color: color || null, note: note || null },
      })
      const target = pages[page - 1]
      if (target) await loadHighlights(target, { force: true })
      return resp?.annot_id || null
    } catch (e) {
      logger.error('创建高亮失败', { error: String(e?.message || e) })
      return null
    }
  }

  const deleteHighlight = async (page, annotId) => {
    if (!activePath || !page || !annotId) return false
    try {
      await invoke('pdf_delete_highlight', { path: activePath, page, annotId })
      const target = pages[page - 1]
      if (target) await loadHighlights(target, { force: true })
      return true
    } catch (e) {
      logger.error('删除高亮失败', { error: String(e?.message || e) })
      return false
    }
  }

  const save = async (destPath = null) => {
    if (!activePath) return false
    try {
      await invoke('pdf_save', { path: activePath, destPath })
      return true
    } catch (e) {
      logger.error('保存 PDF 失败', { error: String(e?.message || e) })
      return false
    }
  }

  const getActivePath = () => activePath

  return {
    pages,
    totalPages,
    hasPdf,
    status,
    statusTone,
    openDocument,
    closeDocument,
    renderPage,
    loadText,
    freePage,
    getOutline,
    search,
    locatePoint,
    loadHighlights,
    addHighlight,
    deleteHighlight,
    save,
    getActivePath,
  }
}
