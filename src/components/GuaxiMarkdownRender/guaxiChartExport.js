// 内嵌图表（```chart / ```excalidraw）的「查看 / 导出」能力。
//
// 为什么单独一个文件：GuaxiMarker 里的那套 addMediaViewerButtons 是个大闭包
// （依赖 overlay、OCR、缩放拖拽等一堆内部状态），图表模块无法复用其中的
// openOverlay / saveBlob。与其把闭包整体提出来（会动到 mermaid / 图片那两条
// 已经稳定的链路），不如在这里独立实现——只做图表需要的最小子集。

import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'
import { buildLightOption } from './guaxiChartData'

const logger = createLogger('ChartExport')

// ───────────────────────── 通用：写文件 ─────────────────────────

/**
 * 把二进制写到用户选定的路径。
 *
 * 先用后端命令（与既有图片/Mermaid 导出一致的通道），失败再退 fs 插件。
 * 走后端的原因：它在 release 包里的权限配置已经验证过，能写到任意用户选择的位置。
 */
async function writeBytesToPath(path, bytes) {
  try {
    let binary = ''
    const CHUNK = 0x8000
    for (let i = 0; i < bytes.length; i += CHUNK) {
      binary += String.fromCharCode.apply(null, bytes.subarray(i, i + CHUNK))
    }
    await invoke('write_file_bytes_base64', { path, base64: btoa(binary) })
    return true
  } catch (error) {
    logger.warn('[ChartExport] backend write failed, fallback to fs plugin', {
      error: String(error?.message || error),
    })
  }
  const { writeFile } = await import('@tauri-apps/plugin-fs')
  await writeFile(path, bytes)
  return true
}

async function saveBlob(blob, defaultName, filters) {
  const { save } = await import('@tauri-apps/plugin-dialog')
  const path = await save({ defaultPath: defaultName, filters })
  if (!path) return false
  const buf = new Uint8Array(await blob.arrayBuffer())
  await writeBytesToPath(path, buf)
  logger.info('[ChartExport] saved', { path, bytes: buf.length })
  return true
}

/** 文件名净化：去掉各平台非法字符，过长则截断。 */
function slugify(raw, fallback = 'chart') {
  const s = String(raw || '')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/[\\/:*?"<>|]/g, '')
    .slice(0, 60)
  return s || fallback
}

/** 从图表块的上下文里取一个像样的默认文件名（最近的标题 + 序号）。 */
function buildBaseName(block, index) {
  try {
    const heading = block
      ?.closest?.('.gdr-md, .preview, .markdown-content')
      ?.querySelector?.('h1, h2, h3, h4, h5, h6')
    const title = heading?.textContent?.trim()
    if (title) return `${slugify(title)}-${index}`
  } catch {
    /* 忽略 */
  }
  return `chart-${index}`
}

// ───────────────────────── ECharts 导出 ─────────────────────────


/**
 * 生成**静态** SVG 前先把动画彻底关掉。
 *
 * 为什么必须关：ECharts 的 SVG 渲染器会给折线/柱状等系列生成一段进场动画，
 * 动画的 `clipPath` 在 0% 时是一个零宽矩形（`d="M78 79l0 0l0 378..."`），
 * 100% 才是完整区域。SVG 一旦离开浏览器（<img> 栅格化、打印成 PDF、被嵌进
 * 静态预览页），CSS 动画不会运行，图形就永远停在 0% —— 于是导出结果里
 * 只剩背景和坐标轴，折线、树图连线、标签全部被裁掉。
 *
 * 关掉动画后 ECharts 不再输出 `@keyframes`，clipPath 直接是最终几何。
 * series 上也可能挂动画（用户可以直接贴整段 ECharts option），所以逐条覆盖。
 */
function freezeChartAnimation(option) {
  if (!option || typeof option !== 'object') return option
  const out = {
    ...option,
    animation: false,
    animationDuration: 0,
    animationDurationUpdate: 0,
  }
  if (Array.isArray(out.series)) {
    out.series = out.series.map((s) => (s && typeof s === 'object' ? { ...s, animation: false } : s))
  } else if (out.series && typeof out.series === 'object') {
    out.series = { ...out.series, animation: false }
  }
  return out
}

const hasWordCloudSeries = (option) => {
  const list = Array.isArray(option?.series) ? option.series : (option?.series ? [option.series] : [])
  return list.some((s) => String(s?.type || '').toLowerCase() === 'wordcloud')
}

/**
 * 等第一次真正绘制完成再取 SVG。
 *
 * 词云（echarts-wordcloud）的布局是**异步**的：它在离屏 canvas 上逐轮尝试摆放文字，
 * `setOption` 返回时画面里还是空的。原先在这里立刻 `getDataURL`，导出的词云就是
 * 一张 200 多字节的空 SVG —— 这正是「词云内容为空」的直接原因。
 * 其余类型都是同步绘制，一次 refresh 就够，不做任何等待。
 */
async function waitForWordCloudLayout(instance, timeoutMs = 2500, intervalMs = 16) {
  const deadline = Date.now() + timeoutMs
  const painted = () => {
    try {
      instance.getZr().refreshImmediately()
      const svg = instance.getDom()?.querySelector('svg')
      return !!svg?.querySelector('text')
    } catch {
      return false
    }
  }
  while (Date.now() < deadline) {
    if (painted()) return true
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
  return painted()
}

/**
 * 把 ECharts 实例导出成 SVG 字符串。
 *
 * 为什么用 SVG 而不是 canvas：ECharts 的 canvas 渲染器拿不到矢量结果，
 * 而 SVG 既能直接查看，也能按需栅格化成 PNG。
 * 为了拿到 SVG，需要临时按 SVG 渲染器重建一次图表（官方没有"canvas 转 svg"的接口）。
 */
export async function renderChartSvg(option, { width = 900, height = 520 } = {}) {
  const mod = await import('echarts')
  const echarts = mod.default || mod

  const host = document.createElement('div')
  host.style.cssText = `position:absolute;left:-99999px;top:0;width:${width}px;height:${height}px;background:#fff;`
  // 导出容器显式声明浅色：否则会继承页面的深色主题变量，
  // 导出的图变成深底浅字，贴进 Word/PDF 后难以阅读。
  host.setAttribute('data-guaxi-export-theme', 'light')
  document.body.appendChild(host)

  let instance = null
  try {
    // 第三个参数（theme）固定传 null 而不是 'dark'：导出必须与当前界面主题解耦。
    // 图表本身的配色若来自 spec，仍会原样保留。
    instance = echarts.init(host, null, { renderer: 'svg', width, height })
    // buildLightOption 会给 option 补上 backgroundColor，ECharts 会据此输出背板矩形：
    // 少了它，SVG 离开深色页面后就是透明底 + 深灰字，看着像「内容丢了」。
    instance.setOption(freezeChartAnimation(buildLightOption(option)), true)
    // 扩展包（词云）的布局是异步的，必须等它真正落笔后再序列化。
    if (hasWordCloudSeries(option)) await waitForWordCloudLayout(instance)
    // SVG 渲染器下 getDataURL 返回 data:image/svg+xml;charset=UTF-8,...
    const dataUrl = instance.getDataURL({ type: 'svg' })
    const comma = String(dataUrl).indexOf(',')
    if (comma < 0) throw new Error('ECharts 未返回 SVG')
    return decodeURIComponent(String(dataUrl).slice(comma + 1))
  } finally {
    try {
      instance?.dispose()
    } catch {
      /* 已销毁 */
    }
    host.remove()
  }
}

/**
 * 把 SVG 字符串栅格化成 PNG blob。
 *
 * 关键在主题：图表要在**浅色底**上导出。ECharts 的 svg 导出本身不带背板，
 * 直接转 PNG 会得到透明底 —— 贴到深色文档里文字就看不见了。
 * 这里显式铺一层白底再栅格化。
 */
export async function svgToPngBlob(svgText, { scale = 2 } = {}) {
  const withBg = ensureSvgBackground(normalizeSvgNamespaces(svgText))

  // 与 mermaid 导出的栅格化同源：优先 base64 data URL。
  // blob: URL 在 Tauri 的 WebView 里偶尔解码失败（表现为整块导出变成空白/源码），
  // 而 data URL 不依赖 Blob 生命周期；只有它失败时才退回 blob。
  const loadImage = (src) => new Promise((resolve, reject) => {
    const image = new Image()
    image.decoding = 'sync'
    image.onload = () => resolve(image)
    image.onerror = () => reject(new Error('SVG 解码失败'))
    image.src = src
  })

  let objectUrl = ''
  try {
    let img
    try {
      const dataUrl = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(withBg)))}`
      img = await loadImage(dataUrl)
    } catch (dataUrlError) {
      logger.debug('[ChartExport] data-url decode failed, fallback to blob url', {
        error: String(dataUrlError?.message || dataUrlError),
      })
      const blob = new Blob([withBg], { type: 'image/svg+xml;charset=utf-8' })
      objectUrl = URL.createObjectURL(blob)
      img = await loadImage(objectUrl)
    }

    const w = Math.max(1, Math.round((img.naturalWidth || 900) * scale))
    const h = Math.max(1, Math.round((img.naturalHeight || 520) * scale))
    const canvas = document.createElement('canvas')
    canvas.width = w
    canvas.height = h
    const ctx = canvas.getContext('2d')
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, w, h)
    ctx.drawImage(img, 0, 0, w, h)

    return await new Promise((resolve, reject) => {
      canvas.toBlob((out) => (out ? resolve(out) : reject(new Error('PNG 编码失败'))), 'image/png')
    })
  } finally {
    if (objectUrl) URL.revokeObjectURL(objectUrl)
  }
}

/**
 * 补齐 XML 命名空间。
 *
 * 浏览器把 SVG 当图片解码时要求根元素声明 xmlns；ECharts 会带上，
 * 但 Excalidraw 的 `outerHTML` 未必。缺失时 <img> 会直接 onerror，
 * 整块导出失败、最终退回源码 —— 也就是“白板图表没有正常导出”。
 */
function normalizeSvgNamespaces(svgText) {
  let svg = String(svgText || '')
  if (!svg.trim()) return ''
  if (!/xmlns=/.test(svg)) {
    svg = svg.replace(/<svg\b/i, '<svg xmlns="http://www.w3.org/2000/svg"')
  }
  if (!/xmlns:xlink=/.test(svg)) {
    svg = svg.replace(/<svg\b/i, '<svg xmlns:xlink="http://www.w3.org/1999/xlink"')
  }
  return svg
}

/** 给 SVG 补一层白色背板（没有 <rect> 背板时）。 */
function ensureSvgBackground(svgText) {
  const svg = String(svgText || '')
  if (/<rect[^>]*fill=["']#?fff/i.test(svg)) return svg
  // 取 viewBox 决定背板尺寸；没有就退 width/height。
  const vb = svg.match(/viewBox=["']([^"']+)["']/i)?.[1]?.trim().split(/[\s,]+/)
  const w = vb?.[2] || svg.match(/width=["']([\d.]+)/i)?.[1] || '900'
  const h = vb?.[3] || svg.match(/height=["']([\d.]+)/i)?.[1] || '520'
  const rect = `<rect x="0" y="0" width="${w}" height="${h}" fill="#ffffff"/>`
  // 插到根 <svg ...> 之后（若有 <defs> 则在 defs 之后更稳妥，这里简化为紧跟根标签）。
  return svg.replace(/(<svg[^>]*>)/i, `$1${rect}`)
}

// ───────────────────────── 查看器 ─────────────────────────

/* 与 mermaid / 图片查看器同一套图标，保证三个查看器的按钮观感一致。 */
const OVERLAY_ICONS = {
  close: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M18.3 5.71 12 12l6.3 6.29-1.41 1.42L12 13.41l-6.29 6.3-1.42-1.41L10.59 12 4.29 5.71 5.7 4.29 12 10.59l6.29-6.3z"/></svg>',
  plus: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M19 11H13V5h-2v6H5v2h6v6h2v-6h6z"/></svg>',
  minus: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M19 13H5v-2h14v2z"/></svg>',
  view: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 5c-7 0-11 7-11 7s4 7 11 7 11-7 11-7-4-7-11-7zm0 12a5 5 0 1 1 0-10 5 5 0 0 1 0 10z"/></svg>',
  save: '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M5 20h14v-2H5v2zm7-18-5.5 5.5 1.42 1.42L11 6.84V16h2V6.84l3.08 3.08 1.42-1.42L12 2z"/></svg>',
}

/**
 * 打开图表查看器浮层。
 *
 * 之前这里是个「按内容撑开的白色小面板」：面板宽度取决于 SVG 的原始宽度，
 * 于是同一篇文档里 mermaid 能铺满 95% 屏幕、图表却挤在中间一小块，观感完全不一致。
 * 现在与 mermaid / 图片查看器共用同一套尺寸与交互：
 *   - 95vw × 92vh 全屏面板，白色底；
 *   - 右上角「缩小 / 放大 / 1:1 / 关闭」；
 *   - 滚轮按光标定点缩放，按住拖拽平移，双击还原，Esc 关闭；
 *   - 左上角提示条。
 * 导出按钮保留在工具栏里（图表特有的能力，不因为对齐而丢掉）。
 */
export function openChartOverlay({ title, svgText, onExportSvg, onExportPng }) {
  const overlay = document.createElement('div')
  overlay.className = 'guaxi-media-overlay'
  overlay.style.cssText =
    'position:fixed;inset:0;background:rgba(0,0,0,0.55);z-index:99999;display:flex;align-items:center;justify-content:center;'

  // 面板是「标题栏 / 画布 / 提示条」三段纵向布局。此前三者全部绝对定位浮在画布上：
  // 图表自带的标题（ECharts title 画在 SVG 里、居中偏上）正好被居中的「图表预览」
  // 胶囊压住，右上角工具栏也盖在图上；导出尺寸一变，底部提示条又会压住刻度文字。
  // 现在控件各占一行，画布是它们之间独立的区域，任何缩放下都不会互相遮挡。
  const panel = document.createElement('div')
  panel.className = 'guaxi-media-panel'
  panel.style.cssText =
    'display:flex;flex-direction:column;width:95vw;height:92vh;max-width:95vw;max-height:92vh;overflow:hidden;box-sizing:border-box;'

  const header = document.createElement('div')
  header.style.cssText =
    'display:flex;align-items:center;gap:12px;flex:0 0 auto;padding:8px 10px;border-bottom:1px solid var(--guaxi-border, #d0d7de);'

  const toolbar = document.createElement('div')
  toolbar.style.cssText = 'display:flex;gap:6px;flex:0 0 auto;'

  const legend = document.createElement('div')
  // 提示条放在面板底部的独立一行（画布之外）。
  // 配色走 .guaxi-media-legend，暗色主题下不会留白块。
  legend.className = 'guaxi-media-legend'
  legend.style.cssText =
    'flex:0 0 auto;align-self:center;display:flex;flex-wrap:wrap;justify-content:center;gap:6px 12px;align-items:center;margin:8px auto 10px;padding:6px 10px;border-radius:8px;line-height:1.3;max-width:calc(95vw - 24px);box-sizing:border-box;'

  const mkLegendItem = (iconSvg, text) => {
    const item = document.createElement('span')
    item.style.cssText = 'display:inline-flex;align-items:center;gap:6px;white-space:nowrap;'
    const icon = document.createElement('span')
    icon.style.cssText = 'display:inline-flex;align-items:center;justify-content:center;line-height:0;flex:0 0 auto;'
    icon.innerHTML = iconSvg
    const label = document.createElement('span')
    label.textContent = text
    item.append(icon, label)
    return item
  }

  const mkTool = (iconSvg, titleText, fallbackText) => {
    const b = document.createElement('button')
    b.type = 'button'
    b.title = titleText
    b.setAttribute('aria-label', titleText)
    b.className = 'guaxi-media-tool'
    b.style.cssText =
      'height:30px;min-width:30px;padding:0 8px;border-radius:8px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;line-height:0;'
    if (fallbackText) {
      b.textContent = fallbackText
      b.style.fontSize = '12px'
      b.style.lineHeight = '1'
    } else {
      const icon = document.createElement('span')
      icon.style.cssText = 'display:inline-flex;align-items:center;justify-content:center;line-height:0;'
      icon.innerHTML = iconSvg
      b.appendChild(icon)
    }
    return b
  }

  const btnMinus = mkTool(OVERLAY_ICONS.minus, '缩小（滚轮/按钮）')
  const btnPlus = mkTool(OVERLAY_ICONS.plus, '放大（滚轮/按钮）')
  const btnReset = mkTool('', '还原（双击画布也可以）', '1:1')
  const btnSvg = mkTool(OVERLAY_ICONS.save, '导出为矢量 SVG')
  const btnPng = mkTool(OVERLAY_ICONS.save, '导出为位图 PNG（白底）')
  const btnClose = mkTool(OVERLAY_ICONS.close, '关闭 (Esc)')

  // 导出按钮带文字：图表的导出有 SVG / PNG 两种，纯图标分不清。
  const withLabel = (btn, text) => {
    const span = document.createElement('span')
    span.textContent = text
    span.style.cssText = 'margin-left:6px;font-size:12px;line-height:1;'
    btn.appendChild(span)
    btn.style.minWidth = '0'
    btn.style.padding = '0 10px'
    return btn
  }
  withLabel(btnSvg, 'SVG')
  withLabel(btnPng, 'PNG')

  toolbar.append(btnMinus, btnPlus, btnReset, btnSvg, btnPng, btnClose)

  // 设计同 mermaid：transform: scale() 不参与布局，只加 scale 时放大后既滚不到也拖不动，
  // 所以这里自己维护 translate + scale，滚轮按光标定点缩放，按住拖动平移。
  const stage = document.createElement('div')
  stage.className = 'guaxi-media-stage'
  stage.style.cssText =
    'flex:1 1 auto;min-height:0;display:flex;align-items:center;justify-content:center;overflow:hidden;padding:16px 24px;box-sizing:border-box;cursor:grab;touch-action:none;'

  const contentWrap = document.createElement('div')
  contentWrap.style.cssText = 'transform-origin:center center;will-change:transform;'
  stage.appendChild(contentWrap)

  // 标题放在顶栏左侧，与工具栏同一行。
  const titleBar = document.createElement('div')
  titleBar.className = 'guaxi-media-title'
  titleBar.style.cssText =
    'flex:1 1 auto;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;'
  titleBar.textContent = title || '图表预览'

  header.append(titleBar, toolbar)

  legend.appendChild(mkLegendItem(OVERLAY_ICONS.plus, '滚轮缩放（按光标定点）'))
  legend.appendChild(mkLegendItem(OVERLAY_ICONS.view, '按住拖动查看细节'))
  legend.appendChild(mkLegendItem(OVERLAY_ICONS.close, '双击还原 · Esc 关闭'))

  const holder = document.createElement('div')
  holder.style.cssText = 'display:block;'
  holder.innerHTML = String(svgText || '')
  const svgEl = holder.querySelector('svg')
  if (svgEl instanceof SVGElement) {
    // 保留原始尺寸参与缩放计算；铺满屏幕的适配交给下面的 fitToStage。
    svgEl.style.display = 'block'
    svgEl.style.maxWidth = 'none'
    svgEl.style.maxHeight = 'none'
  }
  contentWrap.appendChild(holder)

  const ZOOM_MIN = 0.1
  const ZOOM_MAX = 8
  let zoom = 1
  let panX = 0
  let panY = 0
  let fittedScale = 1

  const applyTransform = () => {
    contentWrap.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom * fittedScale})`
  }

  /**
   * 初始适配：让图表完整落在可视区里。
   *
   * 图表 SVG 固定 900×520 输出，直接按原始像素显示会在小窗口里溢出；
   * 这里按「可视区 / 内容尺寸」算一个基准缩放，用户后续的缩放叠加在它之上。
   */
  const fitToStage = () => {
    const rect = stage.getBoundingClientRect()
    const contentRect = holder.getBoundingClientRect()
    const availW = Math.max(1, rect.width - 48)
    const availH = Math.max(1, rect.height - 32)
    const contentW = Math.max(1, contentRect.width / Math.max(fittedScale, 0.0001))
    const contentH = Math.max(1, contentRect.height / Math.max(fittedScale, 0.0001))
    fittedScale = Math.min(availW / contentW, availH / contentH, 4)
    if (!(fittedScale > 0) || !Number.isFinite(fittedScale)) fittedScale = 1
    applyTransform()
  }

  const zoomAt = (nextZoom, anchorX, anchorY) => {
    const target = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, nextZoom))
    if (target === zoom) return
    const rect = contentWrap.getBoundingClientRect()
    const centerX = rect.left + rect.width / 2 - panX
    const centerY = rect.top + rect.height / 2 - panY
    const dx = anchorX - centerX
    const dy = anchorY - centerY
    const ratio = target / zoom
    panX = dx - ratio * (dx - panX)
    panY = dy - ratio * (dy - panY)
    zoom = target
    applyTransform()
  }

  const zoomAtStageCenter = (nextZoom) => {
    const rect = stage.getBoundingClientRect()
    zoomAt(nextZoom, rect.left + rect.width / 2, rect.top + rect.height / 2)
  }

  const resetView = () => {
    zoom = 1
    panX = 0
    panY = 0
    applyTransform()
  }

  let dragging = false
  let dragStartX = 0
  let dragStartY = 0
  let dragOriginX = 0
  let dragOriginY = 0
  let dragMoved = false

  const onDragMove = (e) => {
    if (!dragging) return
    const dx = e.clientX - dragStartX
    const dy = e.clientY - dragStartY
    if (!dragMoved && Math.abs(dx) + Math.abs(dy) > 2) dragMoved = true
    panX = dragOriginX + dx
    panY = dragOriginY + dy
    applyTransform()
  }

  const endDrag = () => {
    if (!dragging) return
    dragging = false
    stage.style.cursor = 'grab'
    window.removeEventListener('mousemove', onDragMove, true)
    window.removeEventListener('mouseup', endDrag, true)
  }

  const close = () => {
    try {
      endDrag()
      document.removeEventListener('keydown', onKey, true)
      if (typeof resizeObserver !== 'undefined') resizeObserver?.disconnect?.()
      overlay.remove()
    } catch {
      /* 已移除 */
    }
  }

  const onKey = (event) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      close()
    }
  }

  btnClose.addEventListener('click', close)
  btnPlus.addEventListener('click', () => zoomAtStageCenter(zoom * 1.1))
  btnMinus.addEventListener('click', () => zoomAtStageCenter(zoom / 1.1))
  btnReset.addEventListener('click', resetView)

  if (typeof onExportSvg === 'function') {
    btnSvg.addEventListener('click', (event) => { event.stopPropagation(); onExportSvg() })
  } else {
    btnSvg.disabled = true
  }
  if (typeof onExportPng === 'function') {
    btnPng.addEventListener('click', (event) => { event.stopPropagation(); onExportPng() })
  } else {
    btnPng.disabled = true
  }

  overlay.addEventListener('click', (event) => {
    // 拖拽结束时鼠标可能正好落在遮罩上，这一下不该被当成「点空白关闭」。
    if (event.target === overlay && !dragMoved) close()
    dragMoved = false
  })

  stage.addEventListener('mousedown', (event) => {
    if (event.button !== 0) return
    event.preventDefault()
    dragging = true
    dragMoved = false
    dragStartX = event.clientX
    dragStartY = event.clientY
    dragOriginX = panX
    dragOriginY = panY
    stage.style.cursor = 'grabbing'
    window.addEventListener('mousemove', onDragMove, true)
    window.addEventListener('mouseup', endDrag, true)
  })

  stage.addEventListener('dblclick', (event) => {
    event.preventDefault()
    resetView()
  })

  stage.addEventListener(
    'wheel',
    (event) => {
      event.preventDefault()
      const delta = event.deltaY > 0 ? 0.9 : 1.1
      zoomAt(zoom * delta, event.clientX, event.clientY)
    },
    { passive: false },
  )

  document.addEventListener('keydown', onKey, true)

  panel.appendChild(header)
  panel.appendChild(stage)
  panel.appendChild(legend)
  overlay.appendChild(panel)
  document.body.appendChild(overlay)

  // 首帧后按可视区适配；窗口尺寸变化时重新适配（用户已手动缩放则保持不动）。
  //
  // 刻意不依赖 requestAnimationFrame：浮层刚 append 时 stage 已经有布局尺寸
  // （宽度由 CSS 的 95vw 决定，不依赖内容），直接量即可。rAF 在窗口最小化 /
  // 被遮挡时不会触发，用了它就会出现「打开查看器后图没有适配」的空窗。
  let resizeObserver = null
  const runFit = () => {
    try {
      fitToStage()
    } catch {
      /* 容器已移除 */
    }
  }
  runFit()
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(() => {
      if (zoom === 1 && panX === 0 && panY === 0) runFit()
    })
    resizeObserver.observe(stage)
  }

  return { close }
}

// ───────────────────────── 按钮挂载 ─────────────────────────

/**
 * 给图表块挂上「查看 / 导出」按钮（与 mermaid / 图片的媒体按钮同一位置与观感）。
 *
 * @param {HTMLElement} block  .guaxi-chart-block
 * @param {{ getSvg: () => Promise<string>, getBaseName: () => string }} api
 */
export function attachChartButtons(block, api) {
  if (!(block instanceof HTMLElement)) return
  if (block.dataset.guaxiChartButtons === '1') return
  block.dataset.guaxiChartButtons = '1'

  const tools = document.createElement('div')
  tools.className = 'guaxi-chart-block__tools'

  const mkTool = (text, title) => {
    const b = document.createElement('button')
    b.type = 'button'
    b.className = 'guaxi-chart-block__tool'
    b.textContent = text
    b.title = title
    return b
  }

  const btnView = mkTool('查看', '放大查看图表')
  const btnSvg = mkTool('SVG', '导出为矢量 SVG')
  const btnPng = mkTool('PNG', '导出为位图 PNG（白底）')

  const runExport = async (kind) => {
    const [btn, orig] = kind === 'png' ? [btnPng, btnPng.textContent] : [btnSvg, btnSvg.textContent]
    btn.disabled = true
    btn.textContent = '…'
    try {
      const svgText = await api.getSvg()
      const base = api.getBaseName()
      if (kind === 'svg') {
        const blob = new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' })
        await saveBlob(blob, `${base}.svg`, [{ name: 'SVG', extensions: ['svg'] }])
      } else {
        const pngBlob = await svgToPngBlob(svgText)
        await saveBlob(pngBlob, `${base}.png`, [{ name: 'PNG', extensions: ['png'] }])
      }
    } catch (error) {
      logger.warn('[ChartExport] export failed', {
        kind,
        error: String(error?.message || error),
      })
      window.alert?.(`导出失败：${String(error?.message || error)}`)
    } finally {
      btn.disabled = false
      btn.textContent = orig
    }
  }

  btnView.addEventListener('click', async (event) => {
    event.preventDefault()
    event.stopPropagation()
    try {
      const svgText = await api.getSvg()
      openChartOverlay({
        title: block.querySelector('.guaxi-chart-block__canvas')?.getAttribute('data-chart-title') || '图表预览',
        svgText,
        onExportSvg: () => runExport('svg'),
        onExportPng: () => runExport('png'),
      })
    } catch (error) {
      logger.warn('[ChartExport] view failed', { error: String(error?.message || error) })
      window.alert?.(`无法预览图表：${String(error?.message || error)}`)
    }
  })
  btnSvg.addEventListener('click', (event) => {
    event.preventDefault()
    event.stopPropagation()
    void runExport('svg')
  })
  btnPng.addEventListener('click', (event) => {
    event.preventDefault()
    event.stopPropagation()
    void runExport('png')
  })

  tools.append(btnView, btnSvg, btnPng)
  block.appendChild(tools)
}

export { slugify, buildBaseName }
