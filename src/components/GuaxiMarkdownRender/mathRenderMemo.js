// 公式渲染结果的记忆层（主线程与 Worker 共用，各自持有独立实例）。
//
// 为什么要有这一层：
//   markdown-it-mathjax3 每次渲染都会把每条公式现场编译成 SVG，实测约 1.1ms/条；
//   一篇 240 条公式的笔记，一次渲染就有约 250ms 花在这里（占整篇渲染耗时的 ~97%）。
//   而 SVG 只由 LaTeX 源码决定，与上下文无关，所以按源码记忆化即可。
//
// 本模块只做**纯内存**的事，不碰 IPC、不碰 DOM：
//   - 主线程侧由调用方把后端持久缓存的结果 `primeMathMemo()` 灌进来；
//   - Worker 侧由主线程随渲染请求把预热好的条目传进来；
//   - 新算出来的条目通过 `drainMathBackfill()` 取出，交给各自那一侧回填后端。
// 这样两个 realm 复用同一套键与同一套上限，而 IPC 只出现在有 invoke 的那一侧。

/** 记忆条目上限。公式种类有限，超出后丢最旧的，避免长会话里无限增长。 */
export const MATH_RENDER_CACHE_LIMIT = 4000

/**
 * 引擎版本，参与后端缓存主键。
 * **MathJax / markdown-it-mathjax3 升级、或改动了 TeX 宏表时必须改这里**，
 * 否则会把旧引擎产出的 SVG（例如 `\bm` 被 noundefined 画成红色的版本）当成新结果继续用。
 */
export const MATH_ENGINE_VERSION = 'mathjax-full@3.2.2|markdown-it-mathjax3@4.3.2|macros-bm-v1'

const mathRenderCache = new Map()
const pendingBackfill = new Map()

/**
 * 载荷：参与后端缓存主键的唯一标识。
 * 用 `\u0000` 分隔，避免 `inline` 与 `block` 的同名公式互相串。
 * 注意：`latex` 必须是 markdown-it-mathjax3 给出的 **token.content 原文**（不做 trim），
 * 否则键对不上、缓存命中不了。
 */
export const mathPayload = (kind, latex) => `${kind}\u0000${latex}`

/** 把一批「载荷 -> SVG」灌进记忆层（来自后端持久缓存或主线程预热）。已存在的不覆盖。 */
export const primeMathMemo = (entries) => {
  if (!entries) return 0
  const pairs = entries instanceof Map ? entries.entries() : Object.entries(entries)
  let primed = 0
  for (const [key, value] of pairs) {
    if (typeof value !== 'string' || !value) continue
    if (mathRenderCache.has(key)) continue
    mathRenderCache.set(key, value)
    primed += 1
  }
  return primed
}

/**
 * 取（必要时计算）一条公式的最终 HTML。命中时返回的就是「没缓存时那次调用本来会返回的同一个字符串」。
 * `produce` 里再抛错由调用方的 try/catch 兜住（见 GuaxiMarker / worker 里的降级处理）。
 */
export const memoizeMathRender = (kind, latex, produce) => {
  const key = mathPayload(kind, latex)
  const cached = mathRenderCache.get(key)
  if (cached !== undefined) return cached

  const value = produce()
  if (mathRenderCache.size >= MATH_RENDER_CACHE_LIMIT) {
    const oldest = mathRenderCache.keys().next().value
    if (oldest !== undefined) mathRenderCache.delete(oldest)
  }
  mathRenderCache.set(key, value)
  // 只把「本次真正算出来的」记下来回填后端；命中或预热的都不需要再写一遍。
  pendingBackfill.set(key, value)
  return value
}

/** 取出并清空待回填条目（`[payload, svg]`）。调用方拿它去写后端持久缓存。 */
export const drainMathBackfill = () => {
  if (!pendingBackfill.size) return []
  const out = [...pendingBackfill.entries()]
  pendingBackfill.clear()
  return out
}

/** 仅供测试/诊断：当前记忆条目数与待回填条目数。 */
export const mathMemoStats = () => ({
  cached: mathRenderCache.size,
  pendingBackfill: pendingBackfill.size,
})

/** 该载荷是否已在记忆层里（用于避免对已知公式再做一次后端查询）。 */
export const isMathMemoized = (payload) => mathRenderCache.has(payload)

/**
 * 从 markdown-it 的 token 流里**精确**取出所有公式载荷。
 * `math_inline` / `math_block` 既可能出现在顶层（块级），也可能挂在 `inline` token 的
 * `children` 里（行内），所以两边都要走。
 *
 * 这条路径拿到的 `token.content` 就是 markdown-it-mathjax3 写进去的原文，
 * 与渲染器规则里的键**必然一致**——所以主线程自己渲染时用它做预热是精确的。
 */
export const collectMathPayloadsFromTokens = (tokens) => {
  const out = []
  const visit = (list) => {
    for (const token of list || []) {
      if (token.type === 'math_inline') out.push(mathPayload('inline', token.content))
      else if (token.type === 'math_block') out.push(mathPayload('block', token.content))
      if (token.children && token.children.length) visit(token.children)
    }
  }
  visit(tokens)
  return out
}

/**
 * 从 Markdown 源码里**粗略**挑出可能的行内公式，生成候选载荷。
 *
 * 这只作为优化提示：内容与 markdown-it-mathjax3 的 `math_inline` 一致取的是
 * **不 trim 的原文**（见其 index.js:101 `state.src.slice(start, match)`），
 * 所以常见写法能精确对上；对不上（转义 `\$`、跨行、`$` 紧邻数字等边界）只是少命中几次，
 * 退化成现在「现场渲染」的行为——**永远不会渲染错**，因为渲染器规则仍按精确键取值。
 */
export const collectInlineMathCandidates = (content) => {
  const text = String(content || '')
  const out = []
  if (!text.includes('$')) return out

  // 先挖掉围栏代码块，避免把代码里的 $ 当成公式。
  const withoutFences = text.replace(/^```[\s\S]*?^```/gm, '').replace(/^~~~[\s\S]*?^~~~/gm, '')
  const inlineRe = /\$([^$\n]+)\$/g
  let m
  while ((m = inlineRe.exec(withoutFences)) !== null) {
    const latex = m[1]
    if (latex) out.push(mathPayload('inline', latex))
  }

  // 块级公式 `$$ ... $$`：内容由多行拼装，这里只取单行形式做候选，够覆盖大多数写法。
  const blockRe = /\$\$[ \t]*([^$\n]+?)[ \t]*\$\$/g
  while ((m = blockRe.exec(withoutFences)) !== null) {
    const latex = m[1]
    if (latex) out.push(mathPayload('block', latex))
  }
  return out
}
