// 文本层构造：把后端 `pdf_page_chars` 返回的"字符级框"（左上原点 pt）按行分组，
// 生成每行一个透明 span 的几何数据。配合组件里测量 naturalWidth + scaleX，
// 让透明字形精确叠在栅格图上 —— 这是修复"中英混排选择断裂 / 末行无高亮"的关键：
//   1. 行分组只看垂直中心，与字体/脚本无关（不再像 pdfium segment 那样在中英交界处裂开）；
//   2. 每行都是一个完整高度的真实 span（不会丢掉最后一行）；
//   3. scaleX 把透明文本宽度拉到与渲染图一致，原生选择高亮因此连续不断。

/**
 * 把字符框分组成行。
 * @param {Array<{c:string,x:number,y:number,w:number,h:number,font_size:number}>} chars
 *   每个字符的左上原点 bbox（pt）。
 * @param {{spaceRatio?:number, lineTolRatio?:number}} [opts]
 *   spaceRatio：水平间距超过 fontSize*spaceRatio 时补空格；
 *   lineTolRatio：垂直中心差在 fontSize*lineTolRatio 内算同一行。
 * @returns {Array<{text:string,leftPt:number,topPt:number,widthPt:number,heightPt:number,fontSizePt:number,scaleX:number}>}
 */
export function buildLines(chars, { spaceRatio = 0.25, lineTolRatio = 0.6 } = {}) {
  if (!Array.isArray(chars) || chars.length === 0) return []

  const enriched = chars.map((ch) => ({
    c: ch.c,
    x: ch.x,
    y: ch.y,
    w: ch.w,
    h: ch.h,
    fs: ch.font_size || ch.h || 1,
    right: ch.x + ch.w,
    bottom: ch.y + ch.h,
    cy: ch.y + ch.h / 2,
  }))
  // 先按垂直中心、再按水平位置排序，便于顺序分行。
  enriched.sort((a, b) => a.cy - b.cy || a.x - b.x)

  const groups = []
  let cur = null
  for (const ch of enriched) {
    if (!cur) {
      cur = { chars: [ch], refCy: ch.cy, fsSum: ch.fs, n: 1 }
      continue
    }
    const avgFs = cur.fsSum / cur.n
    const tol = Math.max(2, avgFs * lineTolRatio)
    if (Math.abs(ch.cy - cur.refCy) <= tol) {
      cur.chars.push(ch)
      cur.fsSum += ch.fs
      cur.n += 1
      // 运行平均，避免行内基线轻微抖动累积导致误并/误分。
      cur.refCy = (cur.refCy * (cur.n - 1) + ch.cy) / cur.n
    } else {
      groups.push(cur)
      cur = { chars: [ch], refCy: ch.cy, fsSum: ch.fs, n: 1 }
    }
  }
  if (cur) groups.push(cur)

  return groups.flatMap((group) => {
    // 同一基线（垂直中心相同）不代表同一段文字：双栏排版、表格、目录页里，
    // 左右两栏/两列的每一行都恰好共线。若把它们并进一个 span，选区会跨越中间
    // 的整段空白，浏览器就把这一整行（含两栏之间的空隙）整块涂蓝 —— 表现为
    // 「划一个词，满屏都是蓝色方块」。这里按「横向大间隙」把行再切成若干段，
    // 每一段各自成为一个可独立选中的 span。
    const cs = group.chars.slice().sort((a, b) => a.x - b.x)
    const runs = []
    let run = []
    for (const ch of cs) {
      if (run.length) {
        const prev = run[run.length - 1]
        const gap = ch.x - prev.right
        // 阈值取字号的 1.5 倍：普通词间距（≤0.5em）与两端对齐的宽词距都会保留，
        // 而双栏间距、表格列距通常远大于 1.5em。下限 4pt 防止小字号误切。
        if (gap > Math.max(4, ch.fs * 1.5)) {
          runs.push(run)
          run = []
        }
      }
      run.push(ch)
    }
    if (run.length) runs.push(run)

    return runs.map((runChars) => {
      let text = ''
      let left = Infinity
      let top = Infinity
      let right = -Infinity
      let bottom = -Infinity
      let fsSum = 0
      let prevRight = null
      for (const ch of runChars) {
        fsSum += ch.fs
        if (prevRight != null) {
          const gap = ch.x - prevRight
          if (gap > ch.fs * spaceRatio) {
            // 段内的小间隙补空格即可；上限 3 个，避免把宽间隙拉成一长串空格。
            const spaces = Math.min(3, Math.max(1, Math.round(gap / (ch.fs * 0.5))))
            text += ' '.repeat(spaces)
          }
        }
        text += ch.c
        prevRight = ch.right
        if (ch.x < left) left = ch.x
        if (ch.y < top) top = ch.y
        if (ch.right > right) right = ch.right
        if (ch.bottom > bottom) bottom = ch.bottom
      }
      return {
        text,
        leftPt: left,
        topPt: top,
        widthPt: Math.max(0, right - left),
        heightPt: Math.max(0, bottom - top),
        fontSizePt: fsSum / runChars.length,
        // 测量后回填；scaleX 与缩放无关（分子分母都随 scale 线性变化），测一次即可复用。
        scaleX: 1,
      }
    })
  })
}
