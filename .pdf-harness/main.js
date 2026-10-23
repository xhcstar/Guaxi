import { createApp, h, nextTick } from 'vue'
import GuaxiPdfViewer from '../src/components/PdfViewer/GuaxiPdfViewer.vue'

// ── mock Tauri IPC ────────────────────────────────────────────────────────────
// 只 mock PDF 阅读器真正会调的几条命令；页面用一张真实 A4 尺寸的占位图代替 pdfium 输出。
const PAGE_W_PT = 595.276
const PAGE_H_PT = 841.89
const TOTAL = 40

const params = new URLSearchParams(location.search)
const aside = params.get('aside') === '1'
const startZoom = Number(params.get('zoom') || 1.3)
const fit = params.get('fit') || 'width'

window.__TAURI_INTERNALS__ = {
  invoke: async (cmd) => {
    if (cmd === 'pdf_session_open' || cmd === 'pdf_open') {
      return {
        total_pages: TOTAL,
        pages: Array.from({ length: TOTAL }, () => ({ w_pt: PAGE_W_PT, h_pt: PAGE_H_PT })),
      }
    }
    if (cmd === 'pdf_render_page') {
      const dpi = 144
      return { page: 1, dpi, image_path: `C:/fake/page.png` }
    }
    if (cmd === 'pdf_page_chars') return { page: 1, w_pt: PAGE_W_PT, h_pt: PAGE_H_PT, chars: [] }
    if (cmd === 'pdf_outline') return []
    if (cmd === 'pdf_page_highlights') return []
    if (cmd === 'pdf_list_highlights') return []
    if (cmd === 'pdf_page_render_hash') return 'x'
    return null
  },
  convertFileSrc: (p) => `/page.png`,
  transformCallback: (cb) => cb,
}

// ── 页面 ─────────────────────────────────────────────────────────────────────
// 复刻 ReaderPane 的 PDF 分支：绝对定位容器 + 可选 260px 侧栏 + GuaxiPdfViewer。
const hostStyle = {
  position: 'relative',
  width: '100%',
  height: '100%',
  overflow: 'hidden',
}

createApp({
  setup() {
    return () =>
      h('div', { style: hostStyle, id: 'stage' }, [
        aside ? h('aside', { style: { position: 'absolute', left: 0, top: 0, bottom: 0, width: '260px', background: '#2b2d31', zIndex: 9 } }) : null,
        h(GuaxiPdfViewer, {
          id: 'viewer',
          style: aside
            ? { position: 'absolute', top: 0, bottom: 0, left: '260px', right: 0 }
            : { position: 'absolute', inset: 0 },
          path: 'C:/fake/book.pdf',
          theme: 'dark',
          titleLabel: 'book.pdf',
          initialZoom: startZoom,
          fitMode: fit,
          features: { toolbar: true, search: true, pagination: true, outline: !!aside, annotations: true },
        }),
      ])
  },
}).mount('#app')

// 等页面渲染稳定后暴露测量结果，供 harness 脚本读取。
window.__layoutReport = () => {
  const body = document.querySelector('.gpv__body')
  const page = document.querySelector('.gpv__page')
  if (!body || !page) return { error: 'not ready', body: !!body, page: !!page }
  const br = body.getBoundingClientRect()
  const pr = page.getBoundingClientRect()
  const inner = document.querySelector('.gpv__page-inner')
  return {
    bodyRect: { left: br.left, right: br.right, width: br.width },
    pageRect: { left: pr.left, right: pr.right, width: pr.width },
    pageCenterOffset: (pr.left + pr.right) / 2 - (br.left + br.right) / 2,
    overflowRight: pr.right - br.right,
    overflowLeft: br.left - pr.left,
    scrollWidth: body.scrollWidth,
    clientWidth: body.clientWidth,
    scrollHeight: body.scrollHeight,
    clientHeight: body.clientHeight,
    innerWidth: inner ? inner.getBoundingClientRect().width : null,
    zoomLabel: document.querySelector('.gpv__zoom')?.textContent,
  }
}
