// 内嵌图表的纯数据逻辑（便于单测）。
//
// 依赖说明：这里唯一的外部依赖是同目录的 guaxiChartTypes.js（同样是纯函数模块，
// 不碰 DOM / Tauri），因此仍然可以被 Node 测试直接加载。

import {
  CHART_TYPES,
  normalizeChartType,
  supportedChartTypes,
  fieldOf,
  fieldsOf,
  titleOf,
  legendOf,
  tooltipOf,
  gridOf,
  categoryAxis,
  valueAxis,
} from './guaxiChartTypes.js'
//
// 图表类型的具体 option 组装在 guaxiChartTypes.js（按族登记的 builder 表），
// 这里只负责「解析围栏 → 取数据 → 调用对应 builder → 补齐公共外壳」。
//
// 为什么单独一个文件：guaxiChartEmbeds.js 要 import Tauri 的 invoke、logger 和动态
// ECharts，Node 的测试进程无法直接加载它。把「解析围栏 / 解析 CSV / 组装 option」
// 这些纯函数抽出来，测试就能覆盖真正的解析语义，而不是去 mock 一整套运行时。

/** 极简 CSV/TSV 解析（支持引号包裹、引号内逗号/换行、双引号转义）。 */
export function parseDelimited(text, delimiter = ',') {
  const rows = []
  let row = []
  let field = ''
  let inQuotes = false
  const src = String(text || '').replace(/^﻿/, '')

  for (let i = 0; i < src.length; i += 1) {
    const ch = src[i]
    if (inQuotes) {
      if (ch === '"') {
        if (src[i + 1] === '"') {
          field += '"'
          i += 1
        } else {
          inQuotes = false
        }
      } else {
        field += ch
      }
      continue
    }
    if (ch === '"') {
      inQuotes = true
    } else if (ch === delimiter) {
      row.push(field)
      field = ''
    } else if (ch === '\n') {
      row.push(field)
      rows.push(row)
      row = []
      field = ''
    } else if (ch === '\r') {
      // 交给 \n 处理
    } else {
      field += ch
    }
  }
  if (field.length || row.length) {
    row.push(field)
    rows.push(row)
  }
  return rows.filter((r) => r.some((c) => String(c).trim() !== ''))
}

/** 猜测单元格类型：数字就转 number，其余保持字符串。 */
const coerceCell = (value) => {
  const raw = String(value ?? '').trim()
  if (raw === '') return ''
  if (/^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(raw)) return Number(raw)
  return raw
}

/** CSV 文本 → 行对象数组（首行为表头）。 */
export function csvToRecords(text, delimiter = ',') {
  const rows = parseDelimited(text, delimiter)
  if (rows.length < 2) return []
  const header = rows[0].map((h, idx) => String(h).trim() || `col${idx + 1}`)
  return rows.slice(1).map((row) => {
    const record = {}
    header.forEach((key, idx) => {
      record[key] = coerceCell(row[idx])
    })
    return record
  })
}

/** 把数据规整为「行对象数组」：支持 JSON 数组、{data: []}、CSV/TSV 文本。 */
export function normalizeDataset(raw, format = '') {
  const text = String(raw || '').trim()
  if (!text) return []

  const looksJson = /^[[{]/.test(text)
  const fmt = String(format || '').toLowerCase()

  if (looksJson || fmt === 'json') {
    try {
      const parsed = JSON.parse(text)
      if (Array.isArray(parsed)) return parsed
      if (parsed && Array.isArray(parsed.data)) return parsed.data
      if (parsed && Array.isArray(parsed.rows)) return parsed.rows
      if (parsed && typeof parsed === 'object') return [parsed]
      return []
    } catch (error) {
      if (looksJson) throw new Error(`数据不是合法 JSON：${error.message}`)
    }
  }

  return csvToRecords(text, fmt === 'tsv' ? '\t' : ',')
}

/**
 * 极简的 YAML 风格 spec 解析：`key: value` 一行一个键；值可以是
 * JSON 字面量（[a, b] / {"k":1}）、数字、布尔、带引号的字符串，或裸字符串。
 * 同时支持块状列表：
 *   y:
 *     - revenue
 *     - cost
 *
 * 为什么不用 YAML 库：围栏里的 spec 是**受控的小结构**（图表类型 + 几个字段名），
 * 引一个 YAML 解析器只为这点语法不划算，也会给主渲染链路增加依赖。
 */
function parseSpecLines(specText) {
  const root = {}
  const lines = String(specText || '').split(/\r?\n/)
  let currentKey = ''
  let collectingList = false

  const parseScalar = (raw) => {
    const value = String(raw || '').trim()
    if (!value) return ''
    if (value === 'true') return true
    if (value === 'false') return false
    if (value === 'null') return null
    if (/^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(value)) return Number(value)
    if (/^[[{]/.test(value)) {
      try {
        return JSON.parse(value)
      } catch {
        // 允许 [a, b] 这种不带引号的数组
        if (value.startsWith('[') && value.endsWith(']')) {
          return value
            .slice(1, -1)
            .split(',')
            .map((v) => v.trim().replace(/^['"]|['"]$/g, ''))
            .filter(Boolean)
        }
        return value
      }
    }
    if (/^(['"]).*\1$/.test(value)) return value.slice(1, -1)
    return value
  }

  for (const line of lines) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue

    // 块状列表项
    const itemMatch = trimmed.match(/^-\s+(.*)$/)
    if (itemMatch && currentKey) {
      if (!collectingList) {
        root[currentKey] = []
        collectingList = true
      }
      if (Array.isArray(root[currentKey])) root[currentKey].push(parseScalar(itemMatch[1]))
      continue
    }

    const kv = trimmed.match(/^([A-Za-z_][\w-]*)\s*:\s*(.*)$/)
    if (!kv) continue
    const [, key, rest] = kv
    const value = rest.trim()
    if (!value) {
      // 值在后续的块状列表里
      currentKey = key
      collectingList = false
      root[key] = root[key] ?? null
      continue
    }
    currentKey = ''
    collectingList = false
    root[key] = parseScalar(value)
  }

  // 只声明了 key、没有列表项的，收成空数组
  for (const [key, value] of Object.entries(root)) {
    if (value === null) root[key] = []
  }

  return root
}

/**
 * 解析 ```chart 围栏内容。
 *
 *   spec:            # JSON（可省略外层花括号，写成缩进的键值对）
 *     type: bar
 *     x: month
 *     y: [revenue, cost]
 *   data: ./x.csv    # 或 `data: |` 接内联文本
 *
 * 解析失败时返回 spec = null（调用方显示错误提示，不抛异常）。
 */
export function parseChartFence(source) {
  const text = String(source || '')
  const result = { spec: null, specRaw: '', data: '', dataFormat: '', dataInline: false }

  const trimmed = text.trim()
  if (/^\s*\{/.test(trimmed)) {
    try {
      result.spec = JSON.parse(trimmed)
      result.specRaw = trimmed
      return result
    } catch {
      /* 落到键值解析 */
    }
  }

  const lines = text.split(/\r?\n/)
  let mode = ''
  const specLines = []
  const dataLines = []
  let dataRef = ''

  for (const line of lines) {
    const trimmedLine = line.trim()
    if (!trimmedLine) {
      if (mode === 'data') dataLines.push('')
      else if (mode === 'spec') specLines.push('')
      continue
    }
    const keyMatch = trimmedLine.match(/^(spec|data)\s*:\s*(.*)$/i)
    if (keyMatch && !/^\s/.test(line)) {
      const [, key, rest] = keyMatch
      mode = key.toLowerCase()
      if (mode === 'data') {
        const value = rest.trim()
        if (value && value !== '|' && value !== '>') {
          const looksPath =
            /^resource:/i.test(value) ||
            /^bundled:/i.test(value) ||
            /^\.{0,2}\//.test(value) ||
            /^[a-zA-Z]:[\\/]/.test(value) ||
            /\.(csv|tsv|json)$/i.test(value)
          if (looksPath) {
            dataRef = value
          } else {
            dataLines.push(value)
            result.dataInline = true
          }
        }
      }
      continue
    }
    if (mode === 'data') {
      dataLines.push(line)
      result.dataInline = true
    } else if (mode === 'spec') {
      specLines.push(line)
    }
  }

  const specText = specLines.join('\n').trim()
  if (specText) {
    result.specRaw = specText
    try {
      result.spec = JSON.parse(specText)
    } catch {
      try {
        result.spec = JSON.parse(`{${specText}}`)
      } catch {
        // 最常见写法：缩进的 `key: value` 行（YAML 风格）
        const yamlLike = parseSpecLines(specText)
        result.spec = Object.keys(yamlLike).length ? yamlLike : null
      }
    }
  }

  if (dataRef) {
    result.data = dataRef
  } else if (dataLines.length) {
    result.data = dataLines.join('\n').trim()
  }

  if (result.data && !result.dataInline) {
    result.dataFormat = 'auto'
  } else if (result.data) {
    result.dataFormat = /^[[{]/.test(result.data) ? 'json' : (result.data.includes('\t') ? 'tsv' : 'csv')
  }

  return result
}

/**
 * 从 Markdown 文本里切出指定类型的代码围栏块。
 *
 * 设计说明（关键 · 对应导出时“部分图表没被替换成图片”）：
 * 围栏必须**从行首开始**。旧实现允许围栏出现在行中间，于是标题里的字面量
 * （例如本项目的示例文档就写成「## 10A. 内嵌图表（```chart）」）会被当成围栏开头，
 * 把紧随其后的第一个真正的代码块吞掉一半 —— 被吞掉的那块永远拿不到预渲染，
 * 导出成品里就仍然是源码。
 *
 * @param {string} markdown
 * @param {string|string[]} kinds 围栏语言（chart / excalidraw / mermaid …）
 * @returns {{full:string, lead:string, fence:string, kind:string, body:string}[]}
 *   `lead` 是匹配到的换行，替换整块时需原样补回（否则相邻段落会被粘在一起）。
 */
export function collectFenceBlocks(markdown, kinds) {
  const text = String(markdown || '')
  const list = (Array.isArray(kinds) ? kinds : [kinds])
    .map((kind) => String(kind || '').trim())
    .filter(Boolean)
    .map((kind) => kind.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
  if (!text || !list.length) return []

  const re = new RegExp(
    `(^|\\n)[ \\t]*(\`{3,}|~{3,})[ \\t]*(${list.join('|')})[^\\n]*\\n([\\s\\S]*?)\\n[ \\t]*\\2[ \\t]*(?=\\n|$)`,
    'g',
  )
  const blocks = []
  let match
  while ((match = re.exec(text)) !== null) {
    blocks.push({
      full: match[0],
      lead: match[1],
      fence: match[2],
      kind: match[3].toLowerCase(),
      body: match[4],
    })
  }
  return blocks
}

/** 依据 spec.type + 数据列，构建一个可直接喂给 ECharts 的 option。 */
export function buildChartOption(spec, records) {
  if (!spec || typeof spec !== 'object') {
    throw new Error('缺少 spec（示例：spec: {"type":"bar","x":"month","y":["revenue"]}）')
  }

  // 完整 ECharts option：包一层 option 显式给出，或直接贴 option 本体。
  // 手册承诺「可以直接贴整个 option」，那就必须真的按 option 处理，
  // 否则它会被当成 spec 走默认分支，画出一张空图。
  const looksLikeOption =
    (spec.option && typeof spec.option === 'object')
    || Array.isArray(spec.series)
    || spec.xAxis !== undefined
    || spec.yAxis !== undefined

  if (looksLikeOption) {
    const option = spec.option && typeof spec.option === 'object' ? { ...spec.option } : { ...spec }
    if (!option.dataset && records.length) option.dataset = { source: records }
    return option
  }

  const typeKey = normalizeChartType(spec.type)
  if (!typeKey) {
    throw new Error(
      `不支持的图表类型「${spec.type}」。可用类型：${supportedChartTypes().join(' / ')}`
    )
  }
  const entry = CHART_TYPES[typeKey]

  const xKey = fieldOf(spec, 'x', 'xField')
  const yKeys = fieldsOf(spec, 'y', 'yField')
  const title = titleOf(spec)

  // 交给类型 builder 生成 series / 片段。builder 的契约统一，因此这里不需要
  // 为每种图写 if 分支 —— 新增类型只在 guaxiChartTypes 里登记即可。
  const built = entry.builder(spec, records, xKey, yKeys)

  // builder 可以返回数组（纯 series）或对象（自带 axis / visualMap 等）。
  // builder 的返回形态有两种：
  //   1) 纯 series 数组，或单个 series 对象（heatmap / parallel 这类只产出一条系列的图）；
  //   2) 片段对象 { series, xAxis?, yAxis?, ... }。
  // 区分依据：片段一定带 `series` 字段，而且不是 ECharts 的 series 本体
  // （series 本体必有 type）。
  let series
  let fragment = null
  if (Array.isArray(built)) {
    series = built
  } else if (built && typeof built === 'object' && built.series !== undefined) {
    fragment = built
    series = Array.isArray(built.series) ? built.series : [built.series]
  } else if (built && typeof built === 'object') {
    series = [built]
  } else {
    throw new Error('图表构造器没有返回任何 series')
  }

  const seriesList = Array.isArray(series) ? series : [series]
  const seriesCount = seriesList.length
  // 图例只列真正的序列：下划线开头的是内部占位（瀑布图垫底的透明柱），
  // 让 ECharts 自动收集会把它也画成一个图例项，看起来像多了一条莫名其妙的系列。
  const legendNames = seriesList
    .map((s) => (s && typeof s === 'object' ? String(s.name || '').trim() : ''))
    .filter((name) => name && !name.startsWith('_'))
  const legend = legendOf(spec, { seriesCount, names: legendNames })
  const tooltip = tooltipOf(spec, entry.family === 'cartesian' ? 'axis' : 'item')

  // 独立布局：图表自己管坐标，不拼 grid / axis。
  if (entry.family === 'independent' || entry.family === 'extension') {
    const option = {
      title,
      tooltip,
      legend,
      series,
      ...(fragment ? { ...fragment, series } : {}),
    }
    return option
  }

  // 自定义族：builder 自带坐标轴（甘特图等）。
  if (entry.family === 'custom') {
    return {
      title,
      tooltip,
      legend,
      series,
      ...(fragment || {}),
      ...(fragment?.xAxis ? {} : { xAxis: categoryAxis(spec, records, xKey), yAxis: valueAxis(spec) }),
      ...(fragment?.grid ? {} : { grid: gridOf(spec, { hasLegend: !!legend }) }),
    }
  }

  // 笛卡尔系：拼装 grid + 双轴。
  // 横向柱状由 buildBar 通过 horizontal 标记；其余类型按 x 分类轴 / y 数值轴的默认布局。
  const horizontal = fragment?.horizontal === true
    || spec.orient === 'horizontal'
    || spec.horizontal === true
  const xAxis = fragment?.xAxis
    ?? (horizontal ? valueAxis(spec) : categoryAxis(spec, records, xKey))
  const yAxis = fragment?.yAxis
    ?? (horizontal ? categoryAxis(spec, records, xKey) : valueAxis(spec))

  return {
    title,
    tooltip,
    legend,
    grid: fragment?.grid ?? gridOf(spec, { hasLegend: !!legend }),
    xAxis,
    yAxis,
    ...(fragment?.visualMap ? { visualMap: fragment.visualMap } : {}),
    series,
  }
}

/**
 * 把 option 里「跟随主题」的部分改写成浅色取值。
 *
 * 用户写的 spec 通常不指定颜色，ECharts 会取当前主题的默认值；如果页面是深色，
 * 导出的图就是深底浅字。这里只调整**没有显式指定**的项，显式指定的配色一律保留。
 */
export function buildLightOption(option) {
  if (!option || typeof option !== 'object') return option
  const out = { ...option }

  // 背景固定白（spec 里显式写了 backgroundColor 就尊重它）
  if (!out.backgroundColor) out.backgroundColor = '#ffffff'

  // 标题与图例文字在没有显式颜色时改成深灰
  if (out.title && typeof out.title === 'object' && !out.title.textStyle?.color) {
    out.title = { ...out.title, textStyle: { color: '#1f2328', ...(out.title.textStyle || {}) } }
  }
  if (out.legend && typeof out.legend === 'object' && !out.legend.textStyle?.color) {
    out.legend = { ...out.legend, textStyle: { color: '#4a4f57', ...(out.legend.textStyle || {}) } }
  }

  // 坐标轴：轴线/刻度/文字统一浅色主题
  const lightenAxis = (axis) => {
    if (!axis || typeof axis !== 'object') return axis
    const next = { ...axis }
    next.axisLabel = { color: '#4a4f57', ...(next.axisLabel || {}) }
    next.axisLine = {
      ...(next.axisLine || {}),
      lineStyle: { color: '#c9ced6', ...(next.axisLine?.lineStyle || {}) },
    }
    if (next.splitLine) {
      next.splitLine = {
        ...next.splitLine,
        lineStyle: { color: '#eef0f3', ...(next.splitLine?.lineStyle || {}) },
      }
    }
    if (next.name && !next.nameTextStyle?.color) {
      next.nameTextStyle = { color: '#4a4f57', ...(next.nameTextStyle || {}) }
    }
    return next
  }
  if (Array.isArray(out.xAxis)) out.xAxis = out.xAxis.map(lightenAxis)
  else if (out.xAxis) out.xAxis = lightenAxis(out.xAxis)
  if (Array.isArray(out.yAxis)) out.yAxis = out.yAxis.map(lightenAxis)
  else if (out.yAxis) out.yAxis = lightenAxis(out.yAxis)

  return out
}
