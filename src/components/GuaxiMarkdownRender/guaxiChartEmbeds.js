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
 * 读取外部数据文件。
 *
 * 支持三种写法，覆盖「数据来自任意位置」的需求：
 *   1. 相对路径（`./data/x.csv`、`../shared/x.csv`）—— 相对**当前笔记所在目录**；
 *   2. 绝对路径（`D:/data/x.csv`、`/home/u/x.csv`）；
 *   3. 用户主目录（`~/datasets/x.csv`）。
 *
 * 读取失败时抛出带路径的明确错误，而不是让图表静默变空。
 */
async function loadExternalData(ref, baseDir) {
  const raw = String(ref || '').trim()
  if (!raw) throw new Error('data 为空')

  let abs
  if (/^~[\\/]/.test(raw)) {
    const { homeDir, join } = await import('@tauri-apps/api/path')
    abs = await join(await homeDir(), raw.replace(/^~[\\/]+/, ''))
  } else if (/^[a-zA-Z]:[\\/]/.test(raw) || raw.startsWith('/')) {
    abs = raw.replace(/\\/g, '/')
  } else {
    if (!baseDir) {
      throw new Error(
        `无法解析相对路径「${raw}」：当前文档没有可用的基准目录。请改用绝对路径（如 D:/data/x.csv）或 ~/ 开头的路径。`
      )
    }
    abs = joinPath(baseDir, raw)
  }

  const ext = String(abs.split('.').pop() || '').toLowerCase()
  const format = ext === 'json' ? 'json' : ext === 'tsv' ? 'tsv' : ext === 'csv' ? 'csv' : 'auto'

  try {
    const text = await invoke('read_file_content', { path: abs })
    logger.info('[ChartEmbed] loaded external data', { ref: raw, abs, format })
    return { text: String(text || ''), format }
  } catch (error) {
    const hint =
      format === 'auto'
        ? '支持 .csv / .tsv / .json 三种数据文件'
        : '请确认路径正确、文件是 UTF-8 文本'
    throw new Error(`读取数据文件失败：${abs}
${hint}`)
  }
}

// ───────────────────────── 渲染入口 ─────────────────────────

let echartsModulePromise = null
const loadECharts = () => {
  if (!echartsModulePromise) {
    // 动态 import：只有文档里真的出现 ```chart 时才加载 ECharts 全量包。
    echartsModulePromise = import('echarts').then((mod) => mod.default || mod)
  }
  return echartsModulePromise
}

const isDarkTheme = () => {
  try {
    const root = document?.documentElement
    return (
      !!root?.classList?.contains?.('dark') ||
      !!root?.classList?.contains?.('dark-mode') ||
      root?.dataset?.guaxiTheme === 'dark'
    )
  } catch {
    return false
  }
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
    const option = buildChartOption(parsed.spec, records)

    const echarts = await loadECharts()
    const instance = echarts.init(el, isDarkTheme() ? 'dark' : null, { renderer: 'canvas' })
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
    const isAbsolute = /^[a-zA-Z]:[\\/]/.test(scenePath) || scenePath.startsWith('/')
    const abs = isAbsolute ? scenePath : joinPath(dirnameOf(baseDocPath), scenePath)
    const raw = await invoke('read_file_content', { path: abs })

    let scene
    try {
      scene = JSON.parse(String(raw || '{}'))
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
