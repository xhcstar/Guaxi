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
  const withBg = ensureSvgBackground(svgText)
  const blob = new Blob([withBg], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(blob)

  try {
    const img = new Image()
    img.decoding = 'sync'
    await new Promise((resolve, reject) => {
      img.onload = resolve
      img.onerror = () => reject(new Error('SVG 解码失败'))
      img.src = url
    })

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
    URL.revokeObjectURL(url)
  }
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

/**
 * 打开图表查看器浮层。
 *
 * 只做图表需要的三件事：按容器宽度自适应缩放、拖拽平移、导出。
 * 与 mermaid 的查看器视觉保持一致（同一个半透明遮罩 + 白色面板）。
 */
export function openChartOverlay({ title, svgText, onExportSvg, onExportPng }) {
  const overlay = document.createElement('div')
  overlay.style.cssText =
    'position:fixed;inset:0;background:rgba(0,0,0,0.55);z-index:99999;display:flex;align-items:center;justify-content:center;'

  const panel = document.createElement('div')
  panel.style.cssText =
    'position:relative;max-width:94vw;max-height:92vh;background:#fff;border-radius:10px;overflow:hidden;box-shadow:0 12px 40px rgba(0,0,0,.35);display:flex;flex-direction:column;'

  const bar = document.createElement('div')
  bar.style.cssText =
    'display:flex;align-items:center;gap:8px;padding:8px 12px;border-bottom:1px solid rgba(0,0,0,.1);font:12px/1.4 system-ui,sans-serif;color:#333;'

  const titleEl = document.createElement('span')
  titleEl.textContent = title || '图表预览'
  titleEl.style.cssText = 'flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;'

  const mkBtn = (text, title2) => {
    const b = document.createElement('button')
    b.type = 'button'
    b.textContent = text
    b.title = title2 || text
    b.style.cssText =
      'height:26px;padding:0 10px;border-radius:6px;border:1px solid rgba(0,0,0,.15);background:#fff;color:#333;font-size:12px;cursor:pointer;'
    return b
  }

  const btnSvg = mkBtn('导出 SVG', '导出为矢量 SVG')
  const btnPng = mkBtn('导出 PNG', '导出为位图 PNG（白底）')
  const btnClose = mkBtn('关闭', '关闭 (Esc)')

  bar.append(titleEl, btnSvg, btnPng, btnClose)

  const stage = document.createElement('div')
  stage.style.cssText =
    'overflow:auto;padding:16px;background:#f5f5f5;max-height:calc(92vh - 44px);'

  const holder = document.createElement('div')
  holder.style.cssText = 'display:inline-block;min-width:100%;text-align:center;'
  holder.innerHTML = String(svgText || '')
  const svgEl = holder.querySelector('svg')
  if (svgEl instanceof SVGElement) {
    // 自适应：撑满面板宽度，同时不超过原始尺寸的 2 倍。
    svgEl.removeAttribute('width')
    svgEl.removeAttribute('height')
    svgEl.style.cssText = 'max-width:100%;height:auto;display:inline-block;'
  }
  stage.appendChild(holder)

  panel.append(bar, stage)
  overlay.appendChild(panel)

  const close = () => {
    document.removeEventListener('keydown', onKey, true)
    overlay.remove()
  }
  const onKey = (event) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      close()
    }
  }
  overlay.addEventListener('click', (event) => {
    if (event.target === overlay) close()
  })
  btnClose.addEventListener('click', close)
  document.addEventListener('keydown', onKey, true)

  if (typeof onExportSvg === 'function') btnSvg.addEventListener('click', () => onExportSvg())
  else btnSvg.disabled = true
  if (typeof onExportPng === 'function') btnPng.addEventListener('click', () => onExportPng())
  else btnPng.disabled = true

  document.body.appendChild(overlay)
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
