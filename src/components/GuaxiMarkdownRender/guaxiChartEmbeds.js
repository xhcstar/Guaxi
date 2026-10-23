// 笔记内嵌图表的运行时增强。
//
// 负责两件事：
//   1. ```chart 围栏 → 用 ECharts 画图（spec 可内联，也可从同目录 CSV/JSON 读数据）；
//   2. ```excalidraw 围栏 → 读取 .excalidraw 场景 JSON，用官方 exportToSvg 渲染为只读 SVG。
//
// 设计约束（与零回归直接相关）：
// - 渲染器本身只输出一个**占位 div**（带 data-* 属性），所有重活都在这里、在 DOM 挂载后异步做。
//   因此 Markdown 渲染器不引入 echarts/excalidraw 到主渲染路径，纯文本文档零额外开销。
// - 这个模块由编辑器预览与阅读器共用，调用失败只影响对应那一块，绝不抛出到渲染链路。
// - 围栏不出现时，本模块的所有函数都是空操作。

import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'
import {
  attachChartButtons,
  renderChartSvg,
  buildBaseName,
} from './guaxiChartExport'
import { CHART_TYPES, EXTENSION_BUILDERS, normalizeChartType } from './guaxiChartTypes'

const logger = createLogger('ChartEmbeds')

/** 已挂载的图表实例：容器 → 实例，卸载时统一 dispose，避免内存泄漏。 */
const chartInstances = new WeakMap()
/** 图表容器 → 已解析好的 option + 数据源，供「查看 / 导出」重建 SVG 用。 */
const chartOptions = new WeakMap()
/** 标记已处理过的容器，防止 MutationObserver 反复重渲染。 */
const processed = new WeakSet()

const safeDecode = (raw) => {
  if (!raw) return ''
  try {
    return decodeURIComponent(String(raw))
  } catch {
    return String(raw)
  }
}

// 解析 / 组装逻辑在 guaxiChartData.js（零依赖，可被 Node 单测直接加载）。
// 这里 re-export，调用方仍然只从本模块取。
export {
  parseDelimited,
  csvToRecords,
  normalizeDataset,
  parseChartFence,
  buildChartOption,
} from './guaxiChartData'

import {
  normalizeDataset,
  parseChartFence,
  buildChartOption,
} from './guaxiChartData'

// ───────────────────────── 数据加载 ─────────────────────────

const dirnameOf = (p) => {
  const normalized = String(p || '').replace(/\\/g, '/')
  const idx = normalized.lastIndexOf('/')
  return idx >= 0 ? normalized.slice(0, idx) : ''
}

const joinPath = (base, rel) => {
  const b = String(base || '').replace(/\\/g, '/').replace(/\/+$/, '')
  const r = String(rel || '').replace(/\\/g, '/').replace(/^\.\//, '').replace(/^\/+/, '')
  if (!b) return r
  if (!r) return b
  // 处理 ../ 回退
  const parts = `${b}/${r}`.split('/')
  const out = []
  for (const part of parts) {
    if (!part || part === '.') continue
    if (part === '..') out.pop()
    else out.push(part)
  }
  const prefix = /^[a-zA-Z]:/.test(b) ? '' : '/'
  return prefix + out.join('/')
}

/**
 * 把围栏里写的本地引用解析成绝对路径。
 *
 * 三种写法统一在这里处理，chart 的 `data` 与 excalidraw 的场景路径共用：
 *   1. `~/...`  -> 用户主目录（交给 Tauri 的 path API 拼）； 
 *   2. 绝对路径 -> 原样（统一成正斜杠）；
 *   3. 相对路径 -> 相对当前文档所在目录。
 */
async function resolveLocalRef(ref, baseDir) {
  const value = String(ref || '').trim()
  if (!value) throw new Error('路径为空')

  if (/^~[/\\]/.test(value)) {
    const { homeDir, join } = await import('@tauri-apps/api/path')
    return await join(await homeDir(), value.replace(/^~[/\\]+/, ''))
  }
  if (/^[a-zA-Z]:[/\\]/.test(value) || value.startsWith('/')) {
    return value.replace(/\\/g, '/')
  }
  if (!baseDir) {
    throw new Error(
      `无法解析相对路径「${value}」：当前文档没有可用的基准目录。请改用绝对路径（如 D:/data/x.csv）或 ~/ 开头的路径。`
    )
  }
  return joinPath(baseDir, value)
}

/**
 * 读取外部数据文件。
 *
 * 引用顺序（这是「换机器/挪文件后图表还能画出来」的关键）：
 *   1. `resource:charts/sales.csv` —— 显式指向资源库内副本，最稳；
 *   2. `./data/x.csv`、`../shared/x.csv` —— 相对当前笔记所在目录；
 *   3. `D:/data/x.csv`、`/home/u/x.csv` —— 绝对路径；
 *   4. `~/datasets/x.csv` —— 用户主目录。
 *
 * 第 2/3 步读不到时再回退到资源库同名相对路径：笔记里的写法不用改，
 * 只要数据文件被纳入资源库，原文件被挪走或换机器也能继续渲染。
 */
async function loadExternalData(ref, baseDir) {
  const raw = String(ref || '').trim()
  if (!raw) throw new Error('data 为空')

  const extOf = (p) => String(p.split('.').pop() || '').toLowerCase()
  const formatOf = (p) => {
    const ext = extOf(p)
    return ext === 'json' ? 'json' : ext === 'tsv' ? 'tsv' : ext === 'csv' ? 'csv' : 'auto'
  }

  const ATTEMPTS = []

  // 1) 显式资源库引用
  if (/^resource:/i.test(raw)) {
    const rel = raw.replace(/^resource:/i, '').replace(/^[/\\]+/, '')
    const abs = await invoke('resolve_resource_path', { relativePath: rel })
    ATTEMPTS.push({ label: '资源库', abs })
  } else {
    try {
      const abs = await resolveLocalRef(raw, baseDir)
      ATTEMPTS.push({ label: '文档目录', abs })
    } catch (error) {
      // 相对路径没有基准目录时，仍然值得试一次资源库。
      logger.info('[ChartEmbed] local ref unresolved, falling back to resource library', {
        ref: raw,
        reason: String(error?.message || error),
      })
    }
    // 无论上面成功与否，都追加资源库回退 —— 文档目录那份可能已经不在了。
    if (!/^([a-zA-Z]:[/\\]|\/|~[/\\])/.test(raw)) {
      try {
        const rel = raw.replace(/^\.\//, '').replace(/^[/\\]+/, '')
        const abs = await invoke('resolve_resource_path', { relativePath: rel })
        ATTEMPTS.push({ label: '资源库', abs })
      } catch {
        /* 资源库未配置时不构成故障 */
      }
    }
  }

  if (!ATTEMPTS.length) {
    throw new Error(
      `无法解析数据引用「${raw}」：当前文档没有可用的基准目录。请改用绝对路径（如 D:/data/x.csv）、~/ 开头，或 resource: 开头的资源库路径。`
    )
  }

  const tried = []
  for (const { label, abs } of ATTEMPTS) {
    try {
      const text = await invoke('read_file_content', { path: abs })
      logger.info('[ChartEmbed] loaded external data', {
        ref: raw,
        abs,
        via: label,
        format: formatOf(abs),
      })
      return { text: String(text || ''), format: formatOf(abs) }
    } catch (error) {
      tried.push(`${label} ${abs}: ${String(error?.message || error)}`)
    }
  }

  throw new Error(`读取数据文件失败：${raw}
已尝试：
${tried.map((t) => `  - ${t}`).join('\n')}
支持 .csv / .tsv / .json 三种数据文件。也可以把数据文件放进资源库后用 resource:相对路径 引用。`)
}

// ───────────────────────── 渲染入口 ─────────────────────────

let echartsModulePromise = null
/**
 * 已加载的扩展包（liquidfill / wordcloud）。
 *
 * 扩展包必须在使用前 import 一次 —— 它们靠副作用把系列类型注册进 ECharts。
 * 按需加载而不是静态 import：只有文档里真的用了水球图/词云才付这份体积。
 */
const loadedExtensions = new Set()

async function ensureExtension(name) {
  if (loadedExtensions.has(name)) return
  try {
    if (name === 'echarts-liquidfill') {
      await import('echarts-liquidfill')
    } else if (name === 'echarts-wordcloud') {
      await import('echarts-wordcloud')
    }
    loadedExtensions.add(name)
    logger.info('[ChartEmbed] extension loaded', { name })
  } catch (error) {
    throw new Error(
      `图表扩展包「${name}」加载失败：${String(error?.message || error)}`
    )
  }
}

const loadECharts = () => {
  if (!echartsModulePromise) {
    // 动态 import：只有文档里真的出现 ```chart 时才加载 ECharts 全量包。
    echartsModulePromise = import('echarts').then((mod) => mod.default || mod)
  }
  return echartsModulePromise
}

/**
 * 图表**始终**用浅色渲染，与界面深浅色无关。
 *
 * 之前的实现会跟随 documentElement 的暗色标记并用 ECharts 内置 dark 主题，
 * 结果是暗色界面下图表变成深底浅字：在界面里勉强能看，但一旦离开这个页面
 * （导出 SVG/PNG、打印成 PDF、贴进其它文档）就只剩一片深色，内容等于丢失。
 * 导出的那条链路本来就被强制成浅色（guaxiChartExport.renderChartSvg 用
 * buildLightOption + 白底），两边不一致正是「预览里看得见、导出里看不见」的来源。
 *
 * 现在统一浅色：画布由 guaxiChartEmbeds.css 铺白底，界面深浅色只影响周围的卡片外观。
 */
const CHART_THEME = null

/**
 * 把一个 chart 围栏解析成可直接喂给 ECharts 的 option。
 *
 * 实时渲染与导出静态渲染共用这一份逻辑：两边各写一遍必然漂移，
 * 出现「预览能画、导出画不出来」这类只在某一侧复现的问题。
 *
 * @param {string} source 围栏正文
 * @param {string} baseDocPath 当前文档路径（解析相对数据文件用）
 */
async function buildOptionFromFence(source, baseDocPath) {
  const parsed = parseChartFence(source)
  if (!parsed.spec) {
    throw new Error('无法解析 chart 围栏（spec 需要是 JSON）')
  }

  let rawData = parsed.data
  let format = parsed.dataFormat
  if (parsed.data && !parsed.dataInline && !parsed.data.includes('\n')) {
    const loaded = await loadExternalData(parsed.data, dirnameOf(baseDocPath))
    rawData = loaded.text
    format = loaded.format
  }

  const records = normalizeDataset(rawData, format)
  const typeKey = normalizeChartType(parsed.spec?.type)
  const extensionRequires = CHART_TYPES[typeKey]?.family === 'extension'
    ? EXTENSION_BUILDERS[typeKey]?.requires
    : null
  const option = buildChartOption(parsed.spec, records)

  // 扩展包类型（liquidFill / wordCloud）必须先注册系列，否则 setOption 会静默画空白。
  if (extensionRequires) await ensureExtension(extensionRequires)

  return { option, records, parsed, typeKey }
}

/**
 * 渲染单个 chart 容器。
 * @param {HTMLElement} el 渲染器产出的 .guaxi-chart-block__canvas
 */
async function renderChartBlock(el) {
  if (!(el instanceof HTMLElement) || processed.has(el)) return
  processed.add(el)

  const block = el.closest('.guaxi-chart-block') || el
  const source = safeDecode(el.getAttribute('data-chart-source') || '')
  const baseDocPath = safeDecode(el.getAttribute('data-chart-base') || '')

  try {
    const { option, records, parsed } = await buildOptionFromFence(source, baseDocPath)

    const echarts = await loadECharts()
    const instance = echarts.init(el, CHART_THEME, { renderer: 'canvas' })
    instance.setOption(option, true)
    chartInstances.set(el, instance)

    // 容器宽度变化（分屏、窗口缩放、侧栏展开）时重算，否则图会被压扁。
    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(() => {
        try {
          instance.resize()
        } catch {
          /* 实例可能已销毁 */
        }
      })
      observer.observe(el)
      el.__guaxiChartObserver = observer
    }

    el.removeAttribute('data-chart-pending')
    chartOptions.set(el, { option })

    // 「查看 / 导出」按钮：与 mermaid、图片的媒体按钮同位置同观感。
    // 导出时按 SVG 渲染器重建一次图表（ECharts 没有 canvas→svg 的接口）。
    attachChartButtons(block, {
      getSvg: () => renderChartSvg(option),
      getBaseName: () => buildBaseName(block, 1),
    })

    logger.info('[ChartEmbed] rendered', {
      records: records.length,
      type: String(parsed.spec.type || ''),
      hasExternalData: !!(parsed.data && !parsed.dataInline),
    })
  } catch (error) {
    processed.delete(el)
    const message = String(error?.message || error)
    block.innerHTML = `<div class="guaxi-chart-block__error">图表渲染失败：${message.replace(/[<>&]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c]))}</div>`
    logger.warn('[ChartEmbed] render failed', { error: message })
  }
}

/**
 * 读取嵌入块引用的文本文件（excalidraw 场景 JSON 等）。
 *
 * 与 loadExternalData 同一套回退顺序：文档目录 → 资源库。
 * 白板场景文件比数据文件更容易「画完随手另存到别处」，资源库回退在这里同样必要。
 */
async function readEmbedFile(ref, baseDir) {
  const raw = String(ref || '').trim()
  if (!raw) throw new Error('路径为空')

  if (/^resource:/i.test(raw)) {
    const rel = raw.replace(/^resource:/i, '').replace(/^[/\\]+/, '')
    const abs = await invoke('resolve_resource_path', { relativePath: rel })
    return { abs, text: String(await invoke('read_file_content', { path: abs }) || '') }
  }

  try {
    const abs = await resolveLocalRef(raw, baseDir)
    const text = String(await invoke('read_file_content', { path: abs }) || '')
    return { abs, text }
  } catch (error) {
    // 绝对路径读不到时不再猜；相对路径才回退到资源库同名位置。
    if (/^([a-zA-Z]:[/\\]|\/|~[/\\])/.test(raw)) throw error
    const rel = raw.replace(/^\.\//, '').replace(/^[/\\]+/, '')
    const abs = await invoke('resolve_resource_path', { relativePath: rel })
    logger.info('[ChartEmbed] embed file fell back to resource library', { ref: raw, abs })
    return { abs, text: String(await invoke('read_file_content', { path: abs }) || '') }
  }
}

/**
 * 渲染单个 excalidraw 容器。
 *
 * 优先用官方 exportToSvg 生成静态 SVG（无 React、无交互，导出到 PDF 也安全）；
 * 场景文件读取失败时给出明确错误，不影响文档其余部分。
 */
async function renderExcalidrawBlock(el) {
  if (!(el instanceof HTMLElement) || processed.has(el)) return
  processed.add(el)

  const wrap = el.closest('.guaxi-excalidraw-embed') || el
  const scenePath = safeDecode(el.getAttribute('data-excalidraw-path') || '')
  const baseDocPath = safeDecode(el.getAttribute('data-chart-base') || '')

  try {
    if (!scenePath) throw new Error('缺少场景文件路径')
    // 与 chart 的 data 保持同一套路径规则：相对（笔记目录）/ 绝对 / ~ 开头 / resource: 资源库。
    const { abs, text } = await readEmbedFile(scenePath, dirnameOf(baseDocPath))

    let scene
    try {
      scene = JSON.parse(String(text || '{}'))
    } catch (error) {
      throw new Error(`场景文件不是合法 JSON：${error.message}`)
    }

    const elements = Array.isArray(scene.elements) ? scene.elements : []
    const appState = scene.appState && typeof scene.appState === 'object' ? scene.appState : {}
    if (!elements.length) throw new Error('场景里没有图形元素')

    const mod = await import('@excalidraw/excalidraw')
    const exportToSvg = mod.exportToSvg || mod.default?.exportToSvg
    if (typeof exportToSvg !== 'function') throw new Error('Excalidraw 导出接口不可用')

    const svg = await exportToSvg({
      elements,
      appState: {
        ...appState,
        exportBackground: true,
        viewBackgroundColor: appState.viewBackgroundColor || '#ffffff',
      },
      files: scene.files || {},
    })

    el.innerHTML = ''
    el.appendChild(svg)
    el.removeAttribute('data-excalidraw-pending')
    logger.info('[ExcalidrawEmbed] rendered', {
      path: abs,
      elements: elements.length,
    })
  } catch (error) {
    const message = String(error?.message || error)
    wrap.innerHTML = `<div class="guaxi-excalidraw-embed__error">白板渲染失败：${message.replace(/[<>&]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c]))}</div>`
    logger.warn('[ExcalidrawEmbed] render failed', { error: message })
  }
}

/**
 * 把一段 chart 围栏直接渲染成 SVG 字符串（**不碰 DOM、不建实例**）。
 *
 * 供导出链路使用：导出的 HTML 字符串在第 3 行就已经定型，之后不会有任何 DOM 挂载，
 * `renderChartEmbeds` 那套「挂载后再画」的机制根本不会被触发 —— 这就是导出成品里
 * 只剩「正在渲染图表…」占位文字的原因。这里在导出前同步把图变成内联 SVG。
 *
 * 失败时返回 null 而不是抛错：一张图坏掉不应该让整篇文档导出失败。
 */
export async function renderChartFenceToSvg(source, baseDocPath) {
  try {
    const { option } = await buildOptionFromFence(String(source || ''), String(baseDocPath || ''))
    return await renderChartSvg(option)
  } catch (error) {
    logger.warn('[ChartEmbed] export svg render failed', {
      error: String(error?.message || error),
    })
    return null
  }
}

/**
 * 把一段 excalidraw 围栏渲染成 SVG 字符串。
 *
 * 与实时渲染的区别只有一处：实时版把 SVG 塞进页面元素，这里只要字符串。
 * 导出必须走这条，理由同 renderChartFenceToSvg。
 */
export async function renderExcalidrawFenceToSvg(scenePath, baseDocPath) {
  try {
    const { text } = await readEmbedFile(String(scenePath || ''), dirnameOf(String(baseDocPath || '')))
    const scene = JSON.parse(String(text || '{}'))
    const elements = Array.isArray(scene.elements) ? scene.elements : []
    const appState = scene.appState && typeof scene.appState === 'object' ? scene.appState : {}
    if (!elements.length) throw new Error('场景里没有图形元素')

    const mod = await import('@excalidraw/excalidraw')
    const exportToSvg = mod.exportToSvg || mod.default?.exportToSvg
    if (typeof exportToSvg !== 'function') throw new Error('Excalidraw 导出接口不可用')

    const svg = await exportToSvg({
      elements,
      appState: {
        ...appState,
        exportBackground: true,
        viewBackgroundColor: appState.viewBackgroundColor || '#ffffff',
      },
      files: scene.files || {},
    })
    return svg?.outerHTML || ''
  } catch (error) {
    logger.warn('[ExcalidrawEmbed] export svg render failed', {
      error: String(error?.message || error),
    })
    return null
  }
}

/**
 * 把渲染器产出的占位块就地替换成静态 SVG（导出前调用）。
 *
 * 只处理仍带 pending 标记的块；已经由 renderChartEmbeds 画好的块保留原样 ——
 * 实时渲染的结果本来就是带按钮的合法 DOM，替换反而会丢掉交互。
 */
export async function materializeChartEmbedsForExport(root) {
  if (!(root instanceof HTMLElement)) return { charts: 0, boards: 0, failed: 0 }
  let charts = 0
  let boards = 0
  let failed = 0

  const chartNodes = Array.from(
    root.querySelectorAll('.guaxi-chart-block__canvas[data-chart-pending]')
  )
  for (const el of chartNodes) {
    const source = safeDecode(el.getAttribute('data-chart-source') || '')
    const baseDocPath = safeDecode(el.getAttribute('data-chart-base') || '')
    const svg = await renderChartFenceToSvg(source, baseDocPath)
    if (!svg) {
      failed += 1
      el.innerHTML = '<div class="guaxi-chart-block__error">图表导出渲染失败</div>'
      continue
    }
    el.innerHTML = svg
    el.removeAttribute('data-chart-pending')
    const svgEl = el.querySelector('svg')
    if (svgEl instanceof SVGElement) {
      // 导出页要按纸宽排布，固定像素宽会溢出或留下大片空白。
      svgEl.removeAttribute('width')
      svgEl.removeAttribute('height')
      svgEl.style.cssText = 'max-width:100%;height:auto;'
    }
    charts += 1
  }

  const boardNodes = Array.from(
    root.querySelectorAll('.guaxi-excalidraw-embed__stage[data-excalidraw-pending]')
  )
  for (const el of boardNodes) {
    const scenePath = safeDecode(el.getAttribute('data-excalidraw-path') || '')
    const baseDocPath = safeDecode(el.getAttribute('data-chart-base') || '')
    const svg = await renderExcalidrawFenceToSvg(scenePath, baseDocPath)
    if (!svg) {
      failed += 1
      el.innerHTML = '<div class="guaxi-excalidraw-embed__error">白板导出渲染失败</div>'
      continue
    }
    el.innerHTML = svg
    el.removeAttribute('data-excalidraw-pending')
    const svgEl = el.querySelector('svg')
    if (svgEl instanceof SVGElement) {
      svgEl.removeAttribute('width')
      svgEl.removeAttribute('height')
      svgEl.style.cssText = 'max-width:100%;height:auto;'
    }
    boards += 1
  }

  logger.info('[ChartEmbed] materialized for export', { charts, boards, failed })
  return { charts, boards, failed }
}

/** 销毁容器内的图表实例（容器被替换/组件卸载时调用）。 */
export function disposeChartInstances(root) {
  if (!(root instanceof HTMLElement)) return
  root.querySelectorAll('.guaxi-chart-block__canvas').forEach((el) => {
    const instance = chartInstances.get(el)
    if (instance) {
      try {
        instance.dispose()
      } catch {
        /* 已销毁 */
      }
      chartInstances.delete(el)
    }
    el.__guaxiChartObserver?.disconnect?.()
    el.__guaxiChartObserver = null
    chartOptions.delete(el)
    processed.delete(el)
  })
  root.querySelectorAll('.guaxi-excalidraw-embed__stage').forEach((el) => {
    processed.delete(el)
  })
}

/**
 * 扫描容器，渲染其中所有尚未渲染的 chart / excalidraw 块。
 * 幂等：已渲染的块会被 processed 跳过，可以安全地在每次 DOM 更新后调用。
 */
export function renderChartEmbeds(root) {
  if (!(root instanceof HTMLElement)) return
  const charts = root.querySelectorAll('.guaxi-chart-block__canvas[data-chart-pending]')
  charts.forEach((el) => {
    void renderChartBlock(el)
  })
  const boards = root.querySelectorAll('.guaxi-excalidraw-embed__stage[data-excalidraw-pending]')
  boards.forEach((el) => {
    void renderExcalidrawBlock(el)
  })
}

/**
 * 绑定「在其他窗口打开」：白板嵌入块里的按钮跳到白板页。
 *
 * 用一次性的容器级委托，而不是给每个按钮挂监听 —— 预览会频繁重渲染，
 * 逐节点绑定会不断累积监听器。已绑定过的容器用 data 标记跳过。
 */
export function bindExcalidrawOpenButtons(root, { onOpen } = {}) {
  if (!(root instanceof HTMLElement)) return
  if (root.dataset.guaxiExcalidrawBound === '1') return
  root.dataset.guaxiExcalidrawBound = '1'
  root.addEventListener('click', (event) => {
    const btn = event.target?.closest?.('[data-excalidraw-open]')
    if (!btn) return
    event.preventDefault()
    event.stopPropagation()
    const ref = safeDecode(btn.getAttribute('data-excalidraw-open') || '')
    if (typeof onOpen === 'function') onOpen(ref)
    else {
      // 兜底：跳白板页（场景文件仍由用户在那边手动打开）。
      try {
        window.location.hash = '/excalidraw'
      } catch {
        /* 忽略 */
      }
    }
  })
}
