/**
 * 内容搜索命中行的预览裁剪。
 *
 * 要解决的是"搜到了，但看不见搜到了什么"：grep 回来的 `preview` 是整行原文，
 * 代码、表格、压成一行的 JSON 动辄几百上千字符，命中往往在很后面。
 * 结果面板一行只有一行的高度，所以得把窗口平移到命中附近，而不是永远从行首开始截。
 *
 * 单独放在 utils 里是为了能自检：仓库没有前端测试框架，
 * 自检脚本是 tools/check_search_preview.mjs（`npm run check:search-preview`）。
 *
 * 坐标系约定：后端（src-tauri/src/commands/searcher.rs 的 byte_to_char_indices）给的
 * start/end 是 Unicode 标量下标，不是 UTF-16 码元下标。所以这里整行按 Array.from
 * 拆成码点数组再切片——汉字两者一致，emoji 这类代理对不这么切就会错位半个字符。
 */

/** 命中前保留几个字符的上文：够看出命中在什么语境里，又不至于把命中挤出可见区。 */
export const PREVIEW_LEAD = 12

/** 单行最多取多少字符。一行本来也放不下更多，而超长行能有几万字符，全切成片段纯属白费。 */
export const PREVIEW_MAX = 240

/** 悬停 tooltip 里给整行，但整行也得有个上限。 */
export const PREVIEW_TITLE_MAX = 400

/** grep 给的行带着行尾换行符，直接渲染会多出一行空白。 */
export const stripLineEnd = (text) => String(text ?? '').replace(/[\r\n]+$/, '')

/**
 * 整理后端给的高亮区间：按起点排序、丢掉空的和越界的、合并重叠的。
 * 不合并的话后面切片会切出交叉片段，渲染成嵌套或错位的高亮。
 *
 * @param {Array<{start: number, end: number}>} matches
 * @param {number} length 整行的字符数（码点数）
 * @returns {Array<{start: number, end: number}>}
 */
export const normalizeMatchRanges = (matches, length) => {
  const ranges = (Array.isArray(matches) ? matches : [])
    .filter((m) => m && Number.isFinite(m.start) && Number.isFinite(m.end) && m.end > m.start)
    .map((m) => ({ start: Math.max(0, m.start), end: Math.min(length, m.end) }))
    .filter((r) => r.end > r.start)
    .sort((a, b) => a.start - b.start)

  const merged = []
  for (const range of ranges) {
    const last = merged[merged.length - 1]
    if (last && range.start <= last.end) last.end = Math.max(last.end, range.end)
    else merged.push(range)
  }
  return merged
}

/**
 * 把命中行裁成"一行看得完"的窗口，并保证第一处命中一定落在窗口里。
 *
 * 顺序是固定的：先掐行首缩进（代码里几十个空格能把内容整体推出可见区），
 * 再按第一处命中右移窗口，最后按 max 截尾。leadCut / tailCut 告诉调用方
 * 该不该在两端补省略号——这个记号不能混进 segments，样式上要和原文区分开。
 *
 * leadCut 只在窗口越过了真正的内容时才为真：掐掉的纯缩进是排版噪声，
 * 补个"…"反而让每一行都顶着省略号，看不出哪行是真的从中间截的。
 *
 * @param {{preview?: string, matches?: Array}} item 一条命中行
 * @param {{lead?: number, max?: number, titleMax?: number}} [options]
 * @returns {{segments: Array<{text: string, hit: boolean}>, leadCut: boolean, tailCut: boolean, full: string}}
 */
export const buildPreviewSegments = (item, options = {}) => {
  const lead = Number.isFinite(options.lead) ? options.lead : PREVIEW_LEAD
  const max = Number.isFinite(options.max) ? options.max : PREVIEW_MAX
  const titleMax = Number.isFinite(options.titleMax) ? options.titleMax : PREVIEW_TITLE_MAX

  const raw = stripLineEnd(item?.preview)
  const chars = Array.from(raw)
  const ranges = normalizeMatchRanges(item?.matches, chars.length)

  let from = 0
  while (from < chars.length && (chars[from] === ' ' || chars[from] === '\t')) from += 1
  // 搜的就是空白时命中会落在缩进里，这种情况一个字都不能掐
  if (ranges.length && ranges[0].start < from) from = 0
  const indentEnd = from

  const firstHit = ranges.length ? ranges[0].start : from
  if (firstHit - from > lead) from = firstHit - lead

  const to = Math.min(chars.length, from + max)
  const slice = (start, end) => chars.slice(start, end).join('')

  const segments = []
  let cursor = from
  for (const range of ranges) {
    const start = Math.max(range.start, from)
    const end = Math.min(range.end, to)
    if (end <= start) continue
    if (start > cursor) segments.push({ text: slice(cursor, start), hit: false })
    segments.push({ text: slice(start, end), hit: true })
    cursor = end
  }
  if (cursor < to) segments.push({ text: slice(cursor, to), hit: false })

  const fullChars = Array.from(raw)
  return {
    segments,
    leadCut: from > indentEnd,
    tailCut: to < chars.length,
    full: fullChars.length > titleMax ? `${fullChars.slice(0, titleMax).join('')}…` : raw
  }
}
