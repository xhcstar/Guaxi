// 内嵌图表的「类型 → ECharts option」适配层。
//
// 设计目标：把图表类型支持做成**可枚举的登记表**，而不是一串 if-else。
// 新增一类图，只需在这里加一个 builder；上层（guaxiChartData）完全不用改。
//
// 分族：
//   - 笛卡尔系（line / bar / scatter / effectScatter / candlestick / boxplot …
//     / pictorialBar / themeRiver / custom）—— 共用 xAxis / yAxis / grid
//   - 极坐标与独立布局（pie / radar / gauge / funnel / sankey / tree / treemap
//     / sunburst / graph / parallel / map / lines / heatmap）
//   - 扩展包（liquidFill / wordCloud，运行时按需加载）
//
// 所有 builder 的契约一致：`(spec, records, helpers) => ECharts option`
// 其中 helpers 提供字段解析、数值转换、系列样式合并等公共能力。

// ───────────────────────── 公共工具 ─────────────────────────

/** 取字段名（兼容 x / xField 两种写法）。 */
export const fieldOf = (spec, key, alt) => {
  const v = spec?.[key] ?? spec?.[alt]
  return v == null ? '' : String(v)
}

/** 取字段名列表（y 可以是单个字符串或数组）。 */
export const fieldsOf = (spec, key, alt) => {
  const v = spec?.[key] ?? spec?.[alt]
  if (v == null) return []
  return Array.isArray(v) ? v.map(String) : [String(v)]
}

/** 安全取数：转成数字，非数字返回 null（ECharts 会把它当空值断开，比 0 更诚实）。 */
export const num = (value) => {
  if (value === '' || value === null || value === undefined) return null
  const n = Number(value)
  return Number.isFinite(n) ? n : null
}

/** 原样取值（字符串保留）。 */
export const raw = (value) => (value === undefined || value === null ? '' : value)

/** 构造标题配置。 */
export const titleOf = (spec, extra = {}) =>
  spec?.title ? { text: String(spec.title), ...extra } : undefined

/**
 * 图例：多序列或无分类轴时显示。
 * `spec.legend === false` 可显式关掉。
 *
 * `names` 由外层从已生成的 series 里收集：给出时写成显式 data，
 * 这样内部占位系列（水瀑图的 `_placeholder`）不会渗进图例。
 */
export const legendOf = (spec, { seriesCount = 1, names = [], extra = {} } = {}) => {
  if (spec?.legend === false) return undefined
  const want = spec?.legend === true || seriesCount > 1
  if (!want) return undefined
  const data = (Array.isArray(names) ? names : []).filter(Boolean)
  return {
    // 标题默认占据顶部（18px 字号，文字底边约在 38px）。图例贴到 42px 起步，
    // 否则在当前 240px 高的画布里图例与标题会直接贴在一起（截图里“快重合”）。
    top: spec?.title ? 42 : 0,
    type: 'scroll',
    ...(data.length ? { data } : {}),
    ...extra,
  }
}

/** 把 spec 里的样式片段合并进 series（用户可用 seriesStyle 覆盖任意 ECharts 字段）。 */
export const withStyle = (series, spec, extra = {}) => ({
  ...series,
  ...(spec?.seriesStyle && typeof spec.seriesStyle === 'object' ? spec.seriesStyle : {}),
  ...extra,
})

/** 统一网格边距（笛卡尔系共用），随标题/图例高度自适应。 */
export const gridOf = (spec, { hasLegend = false } = {}) => {
  const top = (spec?.title ? 52 : 16) + (hasLegend ? 28 : 0)
  return {
    left: Number(spec?.gridLeft ?? 52),
    right: Number(spec?.gridRight ?? 28),
    top,
    bottom: Number(spec?.gridBottom ?? 44),
    containLabel: true,
  }
}

/** 分类轴：从记录里取 x 字段作为类目。 */
export const categoryAxis = (spec, records, xKey) => {
  const axis = {
    type: spec?.xType === 'time' || spec?.xType === 'value' ? spec.xType : 'category',
    data: axis_type_data(spec, records, xKey),
    axisLabel: { hideOverlap: true },
    boundaryGap: spec?.boundaryGap !== undefined ? !!spec.boundaryGap : undefined,
  }
  if (spec?.xName) axis.name = String(spec.xName)
  if (spec?.xAxisStyle && typeof spec.xAxisStyle === 'object') Object.assign(axis, spec.xAxisStyle)
  return axis
}

const axis_type_data = (spec, records, xKey) => {
  if (xKey) return records.map((r) => raw(r[xKey]))
  return records.map((_, i) => String(i + 1))
}

/** 数值轴。 */
export const valueAxis = (spec, key = 'yAxisStyle') => {
  const axis = { type: 'value' }
  if (spec?.yName) axis.name = String(spec.yName)
  if (spec?.yMin !== undefined) axis.min = spec.yMin
  if (spec?.yMax !== undefined) axis.max = spec.yMax
  if (spec?.[key] && typeof spec[key] === 'object') Object.assign(axis, spec[key])
  return axis
}

/** 通用 tooltip。 */
export const tooltipOf = (spec, trigger = 'axis') => {
  if (spec?.tooltip === false) return undefined
  if (spec?.tooltip && typeof spec.tooltip === 'object') return spec.tooltip
  return { trigger }
}

/** 堆叠配置：spec.stack 为 true/'total'/字符串时启用。 */
export const stackOf = (spec) => {
  if (!spec?.stack) return undefined
  return spec.stack === true ? 'total' : String(spec.stack)
}

// ───────────────────────── 笛卡尔系 ─────────────────────────

/** line / area：折线，可选平滑、阶梯、面积填充。 */
function buildLine(spec, records, xKey, yKeys) {
  const area = String(spec.type || '').toLowerCase() === 'area'
  const smooth = !!spec.smooth
  const step = spec.step ? (spec.step === true ? 'start' : String(spec.step)) : undefined
  const series = yKeys.map((key) =>
    withStyle(
      {
        name: key,
        type: 'line',
        data: records.map((r) => num(r[key])),
        smooth,
        ...(step ? { step } : {}),
        ...(area || spec.areaStyle ? { areaStyle: {} } : {}),
        ...(spec.connectNulls ? { connectNulls: true } : {}),
        ...(spec.symbol !== undefined ? { symbol: spec.symbol } : {}),
        ...(spec.lineWidth ? { lineStyle: { width: Number(spec.lineWidth) } } : {}),
      },
      spec
    )
  )
  return series
}

/** bar：柱状，支持分组（默认）、堆叠、正负、背景条、动态排序。 */
function buildBar(spec, records, xKey, yKeys) {
  const stack = stackOf(spec)
  const horizontal = spec.orient === 'horizontal' || spec.horizontal === true
  const series = yKeys.map((key) =>
    withStyle(
      {
        name: key,
        type: 'bar',
        data: records.map((r) => num(r[key])),
        ...(stack ? { stack } : {}),
        ...(spec.showBackground ? { showBackground: true, backgroundStyle: { color: 'rgba(180,180,180,0.12)' } } : {}),
        ...(spec.barWidth ? { barWidth: spec.barWidth } : {}),
        ...(spec.barGap ? { barGap: spec.barGap } : {}),
        ...(spec.barCategoryGap ? { barCategoryGap: spec.barCategoryGap } : {}),
        ...(spec.roundCap ? { itemStyle: { borderRadius: [4, 4, 0, 0] } } : {}),
      },
      spec
    )
  )
  return { series, horizontal }
}

/**
 * 瀑布图：由「增量」推出累计值。
 *
 * 记法：数据里每行一个增量字段，positive / negative 分别着色；
 * 第一根柱子从 0 起，最后一根可选显示合计（spec.total: true）。
 */
function buildWaterfall(spec, records, xKey, yKeys) {
  const key = yKeys[0]
  const values = records.map((r) => num(r[key]) ?? 0)
  const names = records.map((r) => String(raw(r[xKey])))
  const placeholder = []
  const rises = []
  const falls = []
  let acc = 0
  values.forEach((v) => {
    if (v >= 0) {
      placeholder.push(acc)
      rises.push(v)
      falls.push(0)
    } else {
      placeholder.push(acc + v)
      rises.push(0)
      falls.push(-v)
    }
    acc += v
  })
  if (spec.total) {
    names.push('合计')
    placeholder.push(0)
    rises.push(acc)
    falls.push(0)
  }
  const common = { type: 'bar', stack: 'waterfall', barWidth: spec.barWidth || '45%' }
  return {
    series: [
      withStyle({ ...common, name: '_placeholder', itemStyle: { color: 'transparent' }, emphasis: { itemStyle: { color: 'transparent' } }, data: placeholder }, spec),
      withStyle({ ...common, name: '增加', itemStyle: { color: spec.positiveColor || '#3ba272' }, data: rises }, spec),
      withStyle({ ...common, name: '减少', itemStyle: { color: spec.negativeColor || '#ee6666' }, data: falls }, spec),
    ],
    extra: { xAxisData: names },
  }
}

/** 数值轴（散点等以数据本身为坐标的图用）。 */
const numericAxis = (spec, { nameKey, minKey, maxKey, styleKey }) => {
  const axis = { type: 'value' }
  if (spec?.[nameKey]) axis.name = String(spec[nameKey])
  if (spec?.[minKey] !== undefined) axis.min = spec[minKey]
  if (spec?.[maxKey] !== undefined) axis.max = spec[maxKey]
  if (spec?.[styleKey] && typeof spec[styleKey] === 'object') Object.assign(axis, spec[styleKey])
  return axis
}

/** scatter / effectScatter：二维散点，第三维可映射到点大小。 */
function buildScatter(spec, records, xKey, yKeys, { effect = false } = {}) {
  const sizeKey = spec.size ? String(spec.size) : ''
  const data = records.map((r) => {
    const point = [num(r[xKey]) ?? num(r[yKeys[0]]), num(r[yKeys[1]])]
    if (sizeKey) point.push(num(r[sizeKey]) ?? 0)
    return point
  })
  const maxSize = Number(spec.symbolSize || 20)
  const series = withStyle(
    {
      name: spec.title ? String(spec.title) : 'scatter',
      type: effect ? 'effectScatter' : 'scatter',
      data,
      symbolSize: sizeKey
        ? (val) => {
            const v = Array.isArray(val) ? Number(val[2]) : 0
            const max = Math.max(...data.map((d) => Number(d[2]) || 0), 1)
            return Math.max(6, (v / max) * maxSize)
          }
        : Number(spec.symbolSize || 12),
      ...(effect ? { showEffectOn: 'render', rippleEffect: { brushType: spec.rippleType || 'stroke', scale: spec.rippleScale || 3 } } : {}),
    },
    spec
  )
  // 设计说明（关键 · 对应“散点图的点全消失”）：
  // 散点的 x 维来自数据本身（y 给两列就是坐标对），必须用数值轴。
  // 交给外层默认的 category 轴时，160 这样的坐标会被当成“第 160 个类目”，
  // 点全部落在绘图区之外——预览与 pandoc 导出里都只剩坐标轴。
  return {
    series: [series],
    xAxis: numericAxis(spec, { nameKey: 'xName', minKey: 'xMin', maxKey: 'xMax', styleKey: 'xAxisStyle' }),
    yAxis: valueAxis(spec),
  }
}

/** candlestick：K 线，接受 [open, close, low, high] 四列。 */
function buildCandlestick(spec, records, xKey, yKeys) {
  // 约定字段顺序：open / close / low / high；也接受 spec.fields 显式指定。
  const keys = Array.isArray(spec.fields) && spec.fields.length >= 4
    ? spec.fields.map(String)
    : yKeys.length >= 4
      ? yKeys.slice(0, 4)
      : ['open', 'close', 'low', 'high']
  const data = records.map((r) => keys.map((k) => num(r[k]) ?? 0))
  return [
    withStyle(
      {
        name: spec.title ? String(spec.title) : 'K线',
        type: 'candlestick',
        data,
        itemStyle: {
          color: spec.riseColor || '#ef5350',
          color0: spec.fallColor || '#26a69a',
          borderColor: spec.riseColor || '#ef5350',
          borderColor0: spec.fallColor || '#26a69a',
        },
      },
      spec
    ),
  ]
}

/** boxplot：箱线图。字段顺序 [min, Q1, median, Q3, max]，或由原始值自动计算。 */
function buildBoxplot(spec, records, xKey, yKeys) {
  const keys = Array.isArray(spec.fields) && spec.fields.length >= 5 ? spec.fields.map(String) : yKeys
  let data
  if (keys.length >= 5) {
    data = records.map((r) => keys.slice(0, 5).map((k) => num(r[k]) ?? 0))
  } else {
    // 单字段：把一行里的多个数值当作一组样本，现场算五数概括。
    data = records.map((r) => {
      const values = Object.values(r).map(num).filter((v) => v !== null).sort((a, b) => a - b)
      if (!values.length) return [0, 0, 0, 0, 0]
      const q = (p) => values[Math.min(values.length - 1, Math.floor((values.length - 1) * p))]
      return [values[0], q(0.25), q(0.5), q(0.75), values[values.length - 1]]
    })
  }
  return [withStyle({ name: spec.title ? String(spec.title) : '箱线', type: 'boxplot', data }, spec)]
}

/** heatmap（笛卡尔）：[xIndex, yIndex, value]。 */
function buildHeatmapCartesian(spec, records, xKey, yKeys) {
  const xCats = [...new Set(records.map((r) => String(raw(r[xKey]))))]
  const yCats = yKeys.length >= 2
    ? [...new Set(records.map((r) => String(raw(r[yKeys[0]]))))]
    : [String(spec.yLabel || '值')]
  const data = records.map((r) => {
    const x = xCats.indexOf(String(raw(r[xKey])))
    const y = yKeys.length >= 2 ? yCats.indexOf(String(raw(r[yKeys[0]]))) : 0
    const v = num(r[yKeys.length >= 2 ? yKeys[1] : yKeys[0]]) ?? 0
    return [x, y, v]
  })
  const values = data.map((d) => d[2])
  return {
    series: withStyle({ name: spec.title ? String(spec.title) : '热力', type: 'heatmap', data }, spec),
    xAxis: { type: 'category', data: xCats, splitArea: { show: true } },
    yAxis: { type: 'category', data: yCats, splitArea: { show: true } },
    visualMap: {
      min: Math.min(...values, 0),
      max: Math.max(...values, 1),
      calculable: true,
      orient: 'horizontal',
      left: 'center',
      bottom: 0,
      inRange: { color: spec.colorRange || ['#e0f3ff', '#0f6fd6'] },
    },
  }
}

/** pictorialBar：象形柱图（用符号替换柱子）。 */
function buildPictorialBar(spec, records, xKey, yKeys) {
  const symbol = spec.symbol || 'circle'
  return yKeys.map((key) =>
    withStyle(
      {
        name: key,
        type: 'pictorialBar',
        symbol,
        symbolRepeat: spec.symbolRepeat !== false,
        symbolSize: spec.symbolSize || [16, 8],
        symbolMargin: spec.symbolMargin || '10%',
        symbolClip: true,
        data: records.map((r) => num(r[key])),
      },
      spec
    )
  )
}

/** themeRiver：主题河流图，需要三列 [时间, 值, 主题]。 */
function buildThemeRiver(spec, records, xKey, yKeys) {
  const nameKey = spec.name ? String(spec.name) : yKeys[1] || ''
  const valueKey = yKeys[0]
  const data = records.map((r) => [String(raw(r[xKey])), num(r[valueKey]) ?? 0, String(raw(r[nameKey]))])
  return [
    withStyle(
      {
        name: spec.title ? String(spec.title) : '河流',
        type: 'themeRiver',
        data,
        label: { show: !!spec.showLabel },
      },
      spec
    ),
  ]
}

/** custom：用户自己给 renderItem（字符串形式在沙箱里禁止，约定用内置快捷方式）。 */
function buildCustom(spec, records, xKey, yKeys) {
  // 工业场景常见的几种「缺的图」用内置快捷方式表达，避免让用户在笔记里写任意 JS。
  const variant = String(spec.variant || 'gantt').toLowerCase()
  const key = yKeys[0]
  if (variant === 'gantt') {
    // 甘特图：[开始, 结束] 两列，或 spec.start / spec.end 指定字段
    const startKey = spec.start ? String(spec.start) : yKeys[0]
    const endKey = spec.end ? String(spec.end) : yKeys[1] || yKeys[0]
    const cats = records.map((r) => String(raw(r[xKey])))
    const seriesData = records.map((r, i) => ({
      name: cats[i],
      value: [i, num(r[startKey]) ?? 0, num(r[endKey]) ?? 0],
      itemStyle: { color: spec.barColor || '#5470c6' },
    }))
    return {
      series: [withStyle({ type: 'custom', renderItem: ganttRenderItem, encode: { x: [1, 2], y: 0 }, data: seriesData }, spec)],
      xAxis: { type: 'value', min: spec.axisMin, max: spec.axisMax },
      yAxis: { type: 'category', data: cats, inverse: true },
    }
  }
  if (variant === 'bullet' || variant === 'progress') {
    // 子弹图 / 进度条：用 bar 叠加实现，比 custom 更稳。
    const goal = spec.goal != null ? Number(spec.goal) : 100
    return yKeys.map((k) =>
      withStyle({ name: k, type: 'bar', data: records.map((r) => num(r[k])), barWidth: spec.barWidth || 14 }, spec)
    ).concat([
      withStyle({ name: '目标', type: 'bar', data: records.map(() => goal), barWidth: spec.barWidth || 14, barGap: '-100%', itemStyle: { color: 'rgba(0,0,0,0.08)' } }, spec),
    ])
  }
  // 兜底：把 custom 当普通柱图（至少不空图）。
  return buildBar(spec, records, xKey, yKeys).series
}

/** 甘特图的 renderItem：把 [类别索引, 开始, 结束] 画成一根横条。 */
function ganttRenderItem(params, api) {
  const categoryIndex = api.value(0)
  const start = api.coord([api.value(1), categoryIndex])
  const end = api.coord([api.value(2), categoryIndex])
  const height = api.size([0, 1])[1] * 0.5
  const rectShape = { x: start[0], y: start[1] - height / 2, width: Math.max(2, end[0] - start[0]), height }
  // api.style() 在 ECharts 5 已弃用，会在控制台刷 DEPRECATED 警告；
  // 颜色来自数据项的 itemStyle，这里直接读出来写字面量 style。
  return {
    type: 'rect',
    shape: rectShape,
    style: { fill: params?.style?.fill ?? params?.itemStyle?.color ?? '#5470c6' },
    emphasis: { style: { stroke: '#000' } },
  }
}

/** 平行坐标系（parallel）：多字段多维对比。 */
function buildParallel(spec, records, xKey, yKeys) {
  const dims = spec.parallelFields ? spec.parallelFields.map(String) : yKeys
  return {
    series: withStyle({
      name: spec.title ? String(spec.title) : '平行坐标',
      type: 'parallel',
      data: records.map((r) => dims.map((k) => num(r[k]) ?? 0)),
      lineStyle: { width: 1.5, opacity: 0.55 },
    }, spec),
    parallelAxis: dims.map((name, i) => ({
      dim: i,
      name,
      ...(Array.isArray(spec.parallelRanges) && spec.parallelRanges[i]
        ? { min: spec.parallelRanges[i][0], max: spec.parallelRanges[i][1] }
        : {}),
    })),
  }
}

// ───────────────────────── 极坐标 / 独立布局 ─────────────────────────

/** pie / doughnut / ring：饼图、环形、南丁格尔玫瑰。 */
function buildPie(spec, records, xKey, yKeys) {
  const nameKey = xKey || Object.keys(records[0] || {})[0]
  const valueKey = yKeys[0] || Object.keys(records[0] || {})[1]
  const type = String(spec.type || '').toLowerCase()
  const isRose = spec.roseType === true || spec.roseType === 'angle' || spec.roseType === 'radius'
  const radius = spec.radius
    ?? (type === 'pie' && !isRose ? '62%' : ['42%', '68%'])
  const series = withStyle(
    {
      type: 'pie',
      radius,
      center: spec.center || ['50%', '55%'],
      ...(isRose ? { roseType: spec.roseType === true ? 'radius' : String(spec.roseType) } : {}),
      data: records.map((r) => ({
        name: String(raw(r[nameKey] ?? '')),
        value: Number(num(r[valueKey])) || 0,
      })),
      label: { formatter: spec.labelFormatter || '{b}: {d}%', ...(spec.showLabel === false ? { show: false } : {}) },
      ...(spec.donutTitle ? { emphasis: { scale: true } } : {}),
    },
    spec
  )
  return [series]
}

/** radar：雷达图。y 为多个维度字段，x 用作记录名（可选）。 */
function buildRadar(spec, records, xKey, yKeys) {
  const indicators = yKeys.map((k) => {
    const values = records.map((r) => num(r[k]) ?? 0)
    const max = spec.max != null ? Number(spec.max) : Math.max(...values, 1) * 1.15
    return { name: k, max }
  })
  const series = withStyle(
    {
      type: 'radar',
      data: records.map((r) => ({
        name: xKey ? String(raw(r[xKey])) : '',
        value: yKeys.map((k) => num(r[k]) ?? 0),
      })),
      ...(spec.areaStyle !== false ? { areaStyle: { opacity: 0.18 } } : {}),
    },
    spec
  )
  return {
    series: [series],
    radar: {
      indicator: indicators,
      ...(spec.radarRadius ? { radius: spec.radarRadius } : {}),
      splitArea: { areaStyle: { color: ['rgba(250,250,250,0.1)', 'rgba(200,200,200,0.08)'] } },
    },
  }
}

/** gauge：仪表盘，支持多环（每条记录一个环）。 */
function buildGauge(spec, records, xKey, yKeys) {
  const valueKey = yKeys[0]
  const max = spec.max != null ? Number(spec.max) : Math.max(...records.map((r) => num(r[valueKey]) ?? 0), 100)
  if (records.length === 1 && !spec.multi) {
    const detailFontSize = Number(spec.detailFontSize || 22)
    return [withStyle({
      type: 'gauge',
      min: spec.min != null ? Number(spec.min) : 0,
      max,
      progress: spec.progress !== false ? { show: true, width: Number(spec.progressWidth || 12) } : undefined,
      axisLine: { lineStyle: { width: Number(spec.axisLineWidth || 12) } },
      pointer: spec.pointer === false ? { show: false } : undefined,
      detail: {
        formatter: spec.formatter || '{value}',
        fontSize: detailFontSize,
        // ECharts 的 detail 默认 lineHeight 固定为 30px：字号调小时文字框并不会跟着变矮，
        // 会向下压到标题上。按字号显式给行高，让「值」和「名字」的垂直占位可预测。
        lineHeight: Math.round(detailFontSize * 1.25),
        // 值在上、名字在下，两者按半径的百分比分开：
        // 旧值 72% / 92% 只差 20% 半径（240px 高的画布里约 18px），
        // 而值的行高就有 20~30px，名字必然叠进数值里，且 92% 已经贴到圆环内缘。
        offsetCenter: [0, '44%'],
      },
      title: { offsetCenter: [0, '78%'], fontSize: 12 },
      data: [{
        value: num(records[0][valueKey]) ?? 0,
        name: xKey ? String(raw(records[0][xKey])) : '',
      }],
    }, spec)]
  }
  // 多环：每个环一个 series，半径依次外扩。
  const showLegend = spec.legend === true || (spec.legend !== false && records.length > 1)
  const ringBase = Number(spec.baseRadius || 45)
  const ringGap = Number(spec.ringGap || 12)
  // 底部图例大约占 20px（240px 高的画布上约 8% 半径），外圈半径收在 72% 以内才不会压到图例。
  // 环数少时（默认 3 环）间距不变；环数多时按需压缩，避免最外圈顶到图例上。
  const ringStep = showLegend && records.length > 1
    ? Math.min(ringGap, Math.max(5, (72 - ringBase) / (records.length - 1)))
    : ringGap
  const series = records.map((r, i) => withStyle({
    type: 'gauge',
    // 名字放在系列名上：图例与 tooltip 靠它区分各环。
    name: xKey ? String(raw(r[xKey])) : '',
    min: spec.min != null ? Number(spec.min) : 0,
    max,
    radius: `${Math.round(ringBase + i * ringStep)}%`,
    startAngle: 90,
    endAngle: -270,
    pointer: { show: false },
    progress: { show: true, overlap: false, roundCap: true, clip: false },
    axisLine: { lineStyle: { width: 10 } },
    splitLine: { show: false },
    axisTick: { show: false },
    axisLabel: { show: false },
    // 多环共用同一个圆心，ECharts 的 title 默认 show:true、offsetCenter [0,'20%']：
    // 三个环的名字会画在同一像素附近互相压住（截图里“文字糊成一团”的成因）。
    // 名字改由图例承载，这里关掉环内文本。
    title: { show: false },
    detail: { show: false },
    data: [{ value: num(r[valueKey]) ?? 0, name: xKey ? String(raw(r[xKey])) : '' }],
  }, spec))

  // 图例固定放底部：圆环是满圆，顶部既要留给标题又要留给环，底部留白才放得下。
  return {
    series,
    legend: showLegend ? { bottom: 0, type: 'scroll' } : undefined,
  }
}

/** funnel：漏斗图。 */
function buildFunnel(spec, records, xKey, yKeys) {
  const nameKey = xKey || Object.keys(records[0] || {})[0]
  const valueKey = yKeys[0] || Object.keys(records[0] || {})[1]
  const sorted = spec.sort === false
    ? records
    : [...records].sort((a, b) => (num(b[valueKey]) ?? 0) - (num(a[valueKey]) ?? 0))
  return [withStyle({
    type: 'funnel',
    left: spec.left || '12%',
    right: spec.right || '12%',
    top: spec.top || 40,
    bottom: spec.bottom || 24,
    sort: spec.sort || 'descending',
    gap: Number(spec.gap ?? 2),
    label: { show: true, position: 'inside', formatter: spec.labelFormatter || '{b}: {c}' },
    data: sorted.map((r) => ({
      name: String(raw(r[nameKey] ?? '')),
      value: Number(num(r[valueKey])) || 0,
    })),
  }, spec)]
}

/** sankey：桑基图。需要 [源, 目标, 值] 三列（或 spec.source / target / value 指定）。 */
function buildSankey(spec, records, xKey, yKeys) {
  const sourceKey = spec.source ? String(spec.source) : xKey
  const targetKey = spec.target ? String(spec.target) : yKeys[0]
  const valueKey = spec.value ? String(spec.value) : yKeys[1] || yKeys[0]
  const nodes = new Set()
  const links = records.map((r) => {
    const s = String(raw(r[sourceKey]))
    const t = String(raw(r[targetKey]))
    if (s) nodes.add(s)
    if (t) nodes.add(t)
    return { source: s, target: t, value: num(r[valueKey]) ?? 0 }
  })
  return [withStyle({
    type: 'sankey',
    data: [...nodes].map((name) => ({ name })),
    links,
    emphasis: { focus: 'adjacency' },
    lineStyle: { curveness: Number(spec.curveness ?? 0.5), opacity: Number(spec.linkOpacity ?? 0.4) },
    label: { fontSize: 12 },
    nodeGap: Number(spec.nodeGap ?? 12),
    nodeWidth: Number(spec.nodeWidth ?? 16),
  }, spec)]
}

/** tree：树图。数据用 path 列（如 "A/B/C"）或 parent/id 两列。 */
function buildTree(spec, records, xKey, yKeys) {
  const root = buildTreeData(spec, records, xKey, yKeys)
  return [withStyle({
    type: 'tree',
    data: [root],
    layout: spec.layout || 'orthogonal',
    orient: spec.orient || 'LR',
    symbolSize: Number(spec.symbolSize || 7),
    initialTreeDepth: Number(spec.depth ?? 3),
    label: { position: 'left', verticalAlign: 'middle', align: 'right', fontSize: 12 },
    leaves: { label: { position: 'right', align: 'left' } },
    expandAndCollapse: spec.collapsible !== false,
    roam: spec.roam === true,
    top: spec.top || '4%',
    bottom: spec.bottom || '4%',
  }, spec)]
}

/** 由 path 列或 parent/id 构建树。 */
function buildTreeData(spec, records, xKey, yKeys) {
  const rootName = spec.rootName ? String(spec.rootName) : 'root'
  const root = { name: rootName, children: [] }
  const nodeMap = new Map([[rootName, root]])

  const ensurePath = (segments, value) => {
    let cursor = root
    for (const seg of segments) {
      if (!seg) continue
      let node = (cursor.children || []).find((c) => c.name === seg)
      if (!node) {
        node = { name: seg, children: [] }
        cursor.children = cursor.children || []
        cursor.children.push(node)
      }
      cursor = node
    }
    if (value != null) cursor.value = value
  }

  const pathKey = spec.path ? String(spec.path) : ''
  const parentKey = spec.parent ? String(spec.parent) : ''
  const idKey = spec.id ? String(spec.id) : ''
  const valueKey = yKeys[0]

  for (const r of records) {
    if (pathKey) {
      ensurePath(String(raw(r[pathKey])).split('/').map((s) => s.trim()), num(r[valueKey]))
    } else if (parentKey && idKey) {
      const parent = String(raw(r[parentKey])) || rootName
      const id = String(raw(r[idKey]))
      if (!nodeMap.has(parent)) ensurePath([parent], null)
      ensurePath([parent === rootName ? '' : parent, id].filter(Boolean), num(r[valueKey]))
    } else if (xKey) {
      // 退化成两层：x 作为父、y 作为子
      ensurePath([String(raw(r[xKey]))], num(r[valueKey]))
    }
  }
  // 清理空 children，避免 ECharts 画出空节点
  const prune = (node) => {
    if (node.children && node.children.length === 0) delete node.children
    else node.children?.forEach(prune)
    return node
  }
  return prune(root)
}

/** treemap：矩形树图。 */
function buildTreemap(spec, records, xKey, yKeys) {
  const tree = buildTreeData(spec, records, xKey, yKeys)
  return [withStyle({
    type: 'treemap',
    data: tree.children || [],
    leafDepth: spec.leafDepth != null ? Number(spec.leafDepth) : undefined,
    roam: spec.roam === true,
    label: { show: true, formatter: '{b}' },
    upperLabel: { show: true, height: 20 },
    itemStyle: { borderColor: '#fff', borderWidth: 1, gapWidth: 1 },
    levels: spec.levels || undefined,
  }, spec)]
}

/** sunburst：旭日图。 */
function buildSunburst(spec, records, xKey, yKeys) {
  const tree = buildTreeData(spec, records, xKey, yKeys)
  return [withStyle({
    type: 'sunburst',
    data: tree.children || [],
    radius: spec.radius || [0, '88%'],
    label: { rotate: 'radial', fontSize: 11 },
    itemStyle: { borderColor: '#fff', borderWidth: 1 },
    emphasis: { focus: 'ancestor' },
  }, spec)]
}

/** graph：关系图。需要 [源, 目标] 两列，可选权重。 */
function buildGraph(spec, records, xKey, yKeys) {
  const sourceKey = spec.source ? String(spec.source) : xKey
  const targetKey = spec.target ? String(spec.target) : yKeys[0]
  const weightKey = spec.value ? String(spec.value) : yKeys[1]
  const nodes = new Map()
  const links = []
  for (const r of records) {
    const s = String(raw(r[sourceKey]))
    const t = String(raw(r[targetKey]))
    if (s && !nodes.has(s)) nodes.set(s, { name: s, value: 0 })
    if (t && !nodes.has(t)) nodes.set(t, { name: t, value: 0 })
    if (s && t) {
      const w = weightKey ? num(r[weightKey]) ?? 1 : 1
      links.push({ source: s, target: t, value: w })
      if (nodes.has(s)) nodes.get(s).value += 1
      if (nodes.has(t)) nodes.get(t).value += 1
    }
  }
  return [withStyle({
    type: 'graph',
    layout: spec.layout || 'force',
    data: [...nodes.values()],
    links,
    force: { repulsion: Number(spec.repulsion || 180), edgeLength: Number(spec.edgeLength || 90), gravity: 0.1 },
    roam: spec.roam !== false,
    label: { show: true, fontSize: 11 },
    lineStyle: { color: 'source', curveness: Number(spec.curveness ?? 0.12), opacity: 0.55 },
    emphasis: { focus: 'adjacency', lineStyle: { width: 3 } },
    categories: spec.categories || undefined,
    symbolSize: Number(spec.symbolSize || 14),
  }, spec)]
}

/** lines：路径图（迁徙线）。需要 [坐标对] 或经纬度四列。 */
function buildLines(spec, records, xKey, yKeys) {
  // 约定四列：[x1, y1, x2, y2]；若在 geo 坐标系上则是经纬度。
  const keys = Array.isArray(spec.fields) && spec.fields.length >= 4 ? spec.fields.map(String) : yKeys
  const data = records.map((r) => {
    const coords = (keys.length >= 4 ? keys.slice(0, 4) : Object.keys(r).slice(0, 4))
      .map((k) => num(r[k]) ?? 0)
    return { coords: [[coords[0], coords[1]], [coords[2], coords[3]]] }
  })
  return [withStyle({
    type: 'lines',
    coordinateSystem: spec.coordinateSystem || 'cartesian2d',
    polyline: spec.polyline === true,
    data,
    effect: spec.effect === false ? undefined : { show: true, period: Number(spec.period || 4), trailLength: 0.2, symbolSize: 4 },
    lineStyle: { curveness: Number(spec.curveness ?? 0.3), width: Number(spec.lineWidth || 1.2), opacity: 0.7 },
  }, spec)]
}

/** map：地理地图。需要用户注册 GeoJSON（不支持自动下载）。 */
function buildMap(spec, records, xKey, yKeys) {
  const mapName = spec.map ? String(spec.map) : ''
  if (!mapName) {
    throw new Error(
      'map 类型需要在 spec.map 里指定已注册的地图名。离线环境不内置地理数据，' +
      '请在笔记同目录放一份 GeoJSON，或改用 province/city 名称 + type: bar 的替代方案。'
    )
  }
  const valueKey = yKeys[0]
  const nameKey = xKey || 'name'
  return [withStyle({
    type: 'map',
    map: mapName,
    roam: spec.roam !== false,
    data: records.map((r) => ({
      name: String(raw(r[nameKey])),
      value: num(r[valueKey]) ?? 0,
    })),
    label: { show: !!spec.showLabel, fontSize: 10 },
    emphasis: { label: { show: true } },
  }, spec)]
}

// ───────────────────────── 扩展包 ─────────────────────────

/** liquidFill：水球图（来自 echarts-liquidfill）。 */
function buildLiquidFill(spec, records, xKey, yKeys) {
  const valueKey = yKeys[0]
  const value = num(records[0]?.[valueKey]) ?? 0
  const ratio = spec.max ? value / Number(spec.max) : value > 1 ? value / 100 : value
  return [withStyle({
    type: 'liquidFill',
    data: [Number(ratio.toFixed(4)), Number((ratio * 0.82).toFixed(4))],
    radius: spec.radius || '72%',
    outline: { show: spec.outline !== false },
    label: { formatter: spec.formatter || `${Math.round(ratio * 100)}%`, fontSize: Number(spec.detailFontSize || 26) },
    color: spec.colors || ['#3a8ee6', '#64a9ec'],
    backgroundStyle: { color: 'transparent' },
  }, spec)]
}

/** wordCloud：词云（来自 echarts-wordcloud）。用 [词, 权重] 两列。 */
function buildWordCloud(spec, records, xKey, yKeys) {
  const wordKey = xKey || Object.keys(records[0] || {})[0]
  const weightKey = yKeys[0] || Object.keys(records[0] || {})[1]
  return [withStyle({
    type: 'wordCloud',
    shape: spec.shape || 'circle',
    left: 'center',
    top: 'center',
    width: spec.width || '92%',
    height: spec.height || '88%',
    sizeRange: spec.sizeRange || [14, 64],
    rotationRange: spec.rotationRange || [-45, 45],
    gridSize: Number(spec.gridSize || 10),
    drawOutOfBound: false,
    textStyle: { fontFamily: 'sans-serif', fontWeight: 'bold', color: undefined },
    data: records.map((r) => ({
      name: String(raw(r[wordKey])),
      value: num(r[weightKey]) ?? 1,
    })),
  }, spec)]
}

/** 扩展包登记：type → { builder, requires }。requires 供渲染层按需 import。 */
export const EXTENSION_BUILDERS = {
  liquidfill: { builder: buildLiquidFill, requires: 'echarts-liquidfill' },
  wordcloud: { builder: buildWordCloud, requires: 'echarts-wordcloud' },
}

// ───────────────────────── 类型登记表 ─────────────────────────

/**
 * 图表类型登记表。
 *
 * `family` 决定外层如何拼装 axis / grid：
 *   - 'cartesian'：需要 xAxis + yAxis + grid
 *   - 'independent'：图表自带布局，不拼轴
 *   - 'extension'：需要先加载扩展包
 *
 * `builder` 返回 series 数组或完整片段（{ series, xAxis, yAxis, ... }）。
 */
export const CHART_TYPES = {
  // ── 趋势 ──
  line: { family: 'cartesian', builder: (s, r, x, y) => buildLine(s, r, x, y) },
  area: { family: 'cartesian', builder: (s, r, x, y) => buildLine(s, r, x, y) },
  bar: { family: 'cartesian', builder: (s, r, x, y) => buildBar(s, r, x, y).series },
  waterfall: { family: 'cartesian', builder: (s, r, x, y) => buildWaterfall(s, r, x, y) },
  // ── 分布 ──
  scatter: { family: 'cartesian', builder: (s, r, x, y) => buildScatter(s, r, x, y) },
  effectscatter: { family: 'cartesian', builder: (s, r, x, y) => buildScatter(s, r, x, y, { effect: true }) },
  boxplot: { family: 'cartesian', builder: (s, r, x, y) => buildBoxplot(s, r, x, y) },
  candlestick: { family: 'cartesian', builder: (s, r, x, y) => buildCandlestick(s, r, x, y) },
  heatmap: { family: 'independent', builder: (s, r, x, y) => buildHeatmapCartesian(s, r, x, y) },
  pictorialbar: { family: 'cartesian', builder: (s, r, x, y) => buildPictorialBar(s, r, x, y) },
  themeriver: { family: 'cartesian', builder: (s, r, x, y) => buildThemeRiver(s, r, x, y) },
  custom: { family: 'custom', builder: (s, r, x, y) => buildCustom(s, r, x, y) },
  // ── 占比 / 多维 ──
  pie: { family: 'independent', builder: (s, r, x, y) => buildPie(s, r, x, y) },
  doughnut: { family: 'independent', builder: (s, r, x, y) => buildPie(s, r, x, y) },
  ring: { family: 'independent', builder: (s, r, x, y) => buildPie(s, r, x, y) },
  radar: { family: 'independent', builder: (s, r, x, y) => buildRadar(s, r, x, y) },
  parallel: { family: 'independent', builder: (s, r, x, y) => buildParallel(s, r, x, y) },
  // ── 流向 / 转化 / 指标 ──
  sankey: { family: 'independent', builder: (s, r, x, y) => buildSankey(s, r, x, y) },
  funnel: { family: 'independent', builder: (s, r, x, y) => buildFunnel(s, r, x, y) },
  gauge: { family: 'independent', builder: (s, r, x, y) => buildGauge(s, r, x, y) },
  // ── 层级 ──
  tree: { family: 'independent', builder: (s, r, x, y) => buildTree(s, r, x, y) },
  treemap: { family: 'independent', builder: (s, r, x, y) => buildTreemap(s, r, x, y) },
  sunburst: { family: 'independent', builder: (s, r, x, y) => buildSunburst(s, r, x, y) },
  // ── 关系 / 地理 ──
  graph: { family: 'independent', builder: (s, r, x, y) => buildGraph(s, r, x, y) },
  map: { family: 'independent', builder: (s, r, x, y) => buildMap(s, r, x, y) },
  lines: { family: 'independent', builder: (s, r, x, y) => buildLines(s, r, x, y) },
  // ── 扩展包 ──
  liquidfill: { family: 'extension', builder: (s, r, x, y) => buildLiquidFill(s, r, x, y) },
  wordcloud: { family: 'extension', builder: (s, r, x, y) => buildWordCloud(s, r, x, y) },
}

/** 别名：让笔记里的写法更自然（如 violin → boxplot、kline → candlestick）。 */
const TYPE_ALIASES = {
  kline: 'candlestick',
  candlestick: 'candlestick',
  violin: 'boxplot',
  waterful: 'waterfall',
  waterfallchart: 'waterfall',
  donut: 'doughnut',
  circle: 'pie',
  rosetype: 'pie',
  bubble: 'scatter',
  scatter3d: 'scatter',
  force: 'graph',
  network: 'graph',
  wordcloud: 'wordcloud',
  liquid: 'liquidfill',
  liquidfill: 'liquidfill',
}

/** 把用户写的 type 规范化到登记表的 key。 */
export function normalizeChartType(type) {
  const t = String(type || 'line').trim().toLowerCase().replace(/[\s_-]/g, '')
  if (CHART_TYPES[t]) return t
  if (TYPE_ALIASES[t] && CHART_TYPES[TYPE_ALIASES[t]]) return TYPE_ALIASES[t]
  return ''
}

/** 支持的全部类型（供文档与错误提示用）。 */
export function supportedChartTypes() {
  return Object.keys(CHART_TYPES).sort()
}
