// 内嵌图表的纯数据逻辑（零依赖、无副作用，便于单测）。
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

/** 依据 spec.type + 数据列，构建一个可直接喂给 ECharts 的 option。 */
export function buildChartOption(spec, records) {
  if (!spec || typeof spec !== 'object') {
    throw new Error('缺少 spec（示例：spec: {"type":"bar","x":"month","y":["revenue"]}）')
  }
  // 完整 ECharts option：包一层 option 显式给出，或直接贴 option 本体（有 series/xAxis/yAxis）。
  // 两种写法都要支持 —— 手册里承诺"可以直接贴整个 option"，那就必须真的按 option 处理，
  // 否则它会被当成 spec 走默认 line 分支，画出一张空图。
  const looksLikeOption =
    spec.option && typeof spec.option === 'object'
    || Array.isArray(spec.series)
    || spec.xAxis !== undefined
    || spec.yAxis !== undefined

  if (looksLikeOption) {
    const option = spec.option && typeof spec.option === 'object' ? { ...spec.option } : { ...spec }
    if (!option.dataset && records.length) option.dataset = { source: records }
    return option
  }

  const type = String(spec.type || 'line').toLowerCase()
  const xKey = String(spec.x || spec.xField || '')
  const yRaw = spec.y ?? spec.yField
  const yKeys = Array.isArray(yRaw) ? yRaw.map(String) : (yRaw ? [String(yRaw)] : [])
  const title = spec.title ? { text: String(spec.title) } : undefined
  const legend = yKeys.length > 1 ? { top: spec.title ? 32 : 0 } : undefined

  const categories = xKey ? records.map((r) => r[xKey]) : records.map((_, i) => String(i + 1))
  const xAxis = { type: 'category', data: categories, axisLabel: { hideOverlap: true } }
  if (spec.xType === 'time' || spec.xType === 'value') xAxis.type = spec.xType
  if (spec.xName) xAxis.name = String(spec.xName)

  if (type === 'pie' || type === 'doughnut' || type === 'ring') {
    const valueKey = yKeys[0] || Object.keys(records[0] || {})[1]
    const nameKey = xKey || Object.keys(records[0] || {})[0]
    return {
      title,
      tooltip: { trigger: 'item' },
      legend,
      series: [
        {
          type: 'pie',
          radius: type === 'pie' ? '62%' : ['42%', '68%'],
          data: records.map((r) => ({
            name: String(r[nameKey] ?? ''),
            value: Number(r[valueKey]) || 0,
          })),
          label: { formatter: '{b}: {d}%' },
        },
      ],
    }
  }

  const series = yKeys.map((key) => ({
    name: key,
    type: type === 'area' ? 'line' : type,
    data: records.map((r) => r[key]),
    ...(type === 'area' ? { areaStyle: {} } : {}),
    ...(type === 'line' ? { smooth: !!spec.smooth } : {}),
  }))

  const option = {
    title,
    tooltip: { trigger: type === 'scatter' ? 'item' : 'axis' },
    legend,
    grid: { left: 48, right: 24, top: title ? 60 : 28, bottom: 40, containLabel: true },
    xAxis,
    yAxis: { type: 'value', name: spec.yName ? String(spec.yName) : undefined },
    series,
  }

  if (type === 'scatter' && yKeys.length >= 2) {
    option.xAxis = { type: 'value' }
    option.series = [{
      type: 'scatter',
      name: spec.title ? String(spec.title) : 'scatter',
      data: records.map((r) => [Number(r[yKeys[0]]) || 0, Number(r[yKeys[1]]) || 0]),
      symbolSize: Number(spec.symbolSize || 10),
    }]
  }

  return option
}
