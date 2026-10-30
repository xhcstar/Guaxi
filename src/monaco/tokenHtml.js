import { mapTreeSitterCaptureToGuaxiToken } from './guaxiSyntaxTokens.js'

// 把后端 token span 拼成 HTML。
//
// 单独拆成纯函数（不依赖 Tauri / DOM）是为了能直接跑单测：
// span 用的是「0 基行 + 1 基 UTF-16 列」，换算错一位，含中文注释的代码块就会整片偏色。

const escapeHtml = (value) => String(value == null ? '' : value)
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

const buildLineOffsets = (code) => {
  const offsets = [0]
  for (let index = 0; index < code.length; index += 1) {
    if (code.charCodeAt(index) === 10) offsets.push(index + 1)
  }
  return offsets
}

/**
 * 把后端 span 拼回 HTML。
 *
 * span 给的是「0 基行 + 1 基 UTF-16 列」，必须先换算成字符串下标再切片：
 * 代码里有中文注释或 emoji 时，直接按字节偏移切会把整段颜色切歪。
 */
export const buildTokenHtml = (code, spans) => {
  const lineOffsets = buildLineOffsets(code)
  const offsetOf = (line, column) => {
    const start = lineOffsets[line]
    if (start == null) return code.length
    return Math.min(code.length, start + Math.max(0, Number(column || 1) - 1))
  }

  let html = ''
  let cursor = 0
  for (const span of spans || []) {
    const start = offsetOf(Number(span?.line ?? 0), Number(span?.start_column ?? 1))
    const end = offsetOf(Number(span?.end_line ?? 0), Number(span?.end_column ?? 1))
    if (!(end > start) || start < cursor) continue
    if (start > cursor) html += escapeHtml(code.slice(cursor, start))
    const bucket = mapTreeSitterCaptureToGuaxiToken(span?.token_type || '', {
      sourceText: code,
      startOffset: start,
    })
    const text = escapeHtml(code.slice(start, end))
    html += bucket ? `<span class="guaxi-tok-${bucket}">${text}</span>` : text
    cursor = end
  }
  if (cursor < code.length) html += escapeHtml(code.slice(cursor))
  return html
}

