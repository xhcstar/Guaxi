// ============================================================================
// guaxiNotebookMathRuntime.js —— 探索笔记本「数学块」运行时 bootstrap 生成器
// ----------------------------------------------------------------------------
// 设计说明（v2，命令式数学工作区）：
//   1. 数学块的全部计算逻辑现在位于真实、可测试的 Python 引擎模块
//      `src-tauri/scripts/guaximath_engine.py`（符号/数值/交互绘图、命令库等）。
//   2. 本文件不再用 JS 字符串拼接几百行 Python，而是只生成一段极小的 bootstrap：
//        - 优先 `import guaximath_engine`（worker 已把它和自身放进 sys.path，一次加载常驻）；
//        - 失败则 exec 由 Vite `?raw` 内联进来的引擎源码（带版本守卫，同一会话只 exec 一次），
//          保证开发期 / 未物化时仍可用。
//      最终调用 `guaximath_engine.run(source, options, ns=globals())`，把返回的 display
//      写入会话全局 `__guaxi_plotly_display__`，worker 统一转发为 result_json.__guaxi_display__。
//   3. 工作区变量持久化在会话 globals 的 `__GUAXIMATH_WS__` 里，跨多次运行 / 多个数学块复用。
//
// 与旧版的差异：
//   - 入参从「单表达式 + 单 mode」变为「整段 source（多行命令/赋值/裸表达式）+ options」。
//   - 旧的 mode 仍可通过 options.mode 透传（向后兼容工具栏的快捷动作），引擎可据此包裹一层调用。
// ============================================================================

// Vite 原生支持 `?raw`：构建时把引擎源码作为字符串内联进前端包，仅用于「import 失败」兜底。
import GUAXIMATH_ENGINE_SOURCE from '../../../../src-tauri/scripts/guaximath_engine.py?raw'

const normalizeText = (value) => String(value == null ? '' : value).trim()

// 与引擎里的 ENGINE_VERSION 保持一致；版本变化时兜底路径会强制重新 exec。
const GUAXIMATH_ENGINE_VERSION = '1.1.0'

// 把引擎源码安全地内嵌进生成的 Python：用 base64 规避所有转义/缩进问题。
const encodeEngineSourceBase64 = (source) => {
  const text = String(source || '')
  if (typeof TextEncoder !== 'undefined' && typeof btoa === 'function') {
    const bytes = new TextEncoder().encode(text)
    let binary = ''
    for (let i = 0; i < bytes.length; i += 1) binary += String.fromCharCode(bytes[i])
    return btoa(binary)
  }
  // Node / SSR 兜底
  // eslint-disable-next-line no-undef
  return Buffer.from(text, 'utf-8').toString('base64')
}

// 把若干「快捷动作」mode 映射成对应的 source 改写，向后兼容旧工具栏的一键操作。
// 例如选中「求导」时，把整段表达式包成 diff(<expr>)。多行 / 已是命令的内容则原样执行。
const MATH_MODE_WRAPPERS = {
  simplify: (expr) => `simplify(${expr})`,
  derivative: (expr) => `diff(${expr})`,
  integral: (expr) => `integrate(${expr})`,
  solve: (expr) => `solve(${expr})`,
  series: (expr, o) => `taylor(${expr}, x, ${Number(o.seriesPoint) || 0}, ${Number(o.seriesOrder) || 6})`,
  plot2d: (expr) => `plot(${expr})`,
  distribution: (expr) => expr, // 引擎已提供 Normal/Binomial 等构造器，直接执行
  laplace: (expr) => `laplace(${expr})`,
  fourier: (expr) => `fourier(${expr})`,
  dsolve: (expr) => `dsolve(${expr})`,
}

// 把一段任意文本编码为 Python 字符串字面量（JSON 兼容 Python 的字符串语法）。
const toPyStr = (value) => JSON.stringify(String(value == null ? '' : value))

// ---------------------------------------------------------------------------
// 主入口：生成数学块运行时 bootstrap（极小，真正逻辑在 guaximath_engine.py）
// ---------------------------------------------------------------------------
//   入参：
//     source   —— 数学块整段文本（命令式工作区源码）。
//     mode     —— 可选，工具栏快捷动作；提供时把单表达式包一层（见 MATH_MODE_WRAPPERS）。
//     variable / rangeMin / rangeMax / seriesPoint / seriesOrder —— 绘图 / 级数等选项。
//   兼容：仍接受旧字段 `expression`（等价于 source）。
export const buildNotebookLabMathCode = ({
  source = '',
  expression = '',
  mode = '',
  variable = 'x',
  rangeMin = -10,
  rangeMax = 10,
  seriesPoint = 0,
  seriesOrder = 6,
  theme = 'light',
} = {}) => {
  const rawSource = normalizeText(source || expression)
  if (!rawSource) return ''

  const options = {
    variable: normalizeText(variable || 'x'),
    rangeMin: Number.isFinite(Number(rangeMin)) ? Number(rangeMin) : -10,
    rangeMax: Number.isFinite(Number(rangeMax)) ? Number(rangeMax) : 10,
    seriesPoint: Number.isFinite(Number(seriesPoint)) ? Number(seriesPoint) : 0,
    seriesOrder: Math.max(1, Math.min(30, Number.parseInt(seriesOrder, 10) || 6)),
    // 主题透传：暗色模式下交互图用 plotly_dark，与 App 主题协调（符号结果走 markdown 自动跟随主题）。
    theme: String(theme || 'light').toLowerCase() === 'dark' ? 'dark' : 'light',
  }

  // 工具栏快捷动作：仅当 source 是「单条、不含换行、且不像已写好的命令」时才包裹，
  // 避免破坏用户在工作区里写的多行程序。
  const wrapper = MATH_MODE_WRAPPERS[normalizeText(mode)]
  const looksLikeProgram = /\n/.test(rawSource) || /[A-Za-z_]\w*\s*\(/.test(rawSource) || /=/.test(rawSource)
  const effectiveSource = (wrapper && !looksLikeProgram) ? wrapper(rawSource, options) : rawSource

  const engineBase64 = encodeEngineSourceBase64(GUAXIMATH_ENGINE_SOURCE)
  const optionsJson = JSON.stringify(options)

  // 生成的 Python：受控字面量写入，杜绝注入。
  return [
    '# === Guaxi 数学工作区 bootstrap（自动生成，逻辑在 guaximath_engine.py）===',
    'import json as __gx_json',
    `__gx_engine_version = ${toPyStr(GUAXIMATH_ENGINE_VERSION)}`,
    `__gx_source = ${toPyStr(effectiveSource)}`,
    `__gx_options = __gx_json.loads(${toPyStr(optionsJson)})`,
    '',
    '# 1) 优先用 worker 已物化、随 import 常驻的真实引擎模块',
    '__gx_engine = None',
    'try:',
    '    import guaximath_engine as __gx_engine',
    '    if getattr(__gx_engine, "ENGINE_VERSION", None) != __gx_engine_version:',
    '        __gx_engine = None  # 版本不符则走内联兜底，避免新旧逻辑串台',
    'except Exception:',
    '    __gx_engine = None',
    '',
    '# 2) 兜底：exec 由前端 ?raw 内联进来的引擎源码（同一会话只 exec 一次，版本守卫）',
    'if __gx_engine is None:',
    '    if globals().get("__GUAXIMATH_INLINE_VERSION__") != __gx_engine_version:',
    '        import base64 as __gx_b64',
    `        __gx_src = __gx_b64.b64decode(${toPyStr(engineBase64)}).decode("utf-8")`,
    '        __gx_mod_ns = {"__name__": "guaximath_engine_inline"}',
    '        exec(compile(__gx_src, "<guaximath_engine_inline>", "exec"), __gx_mod_ns)',
    '        globals()["__GUAXIMATH_INLINE__"] = __gx_mod_ns',
    '        globals()["__GUAXIMATH_INLINE_VERSION__"] = __gx_engine_version',
    '    __gx_inline = globals().get("__GUAXIMATH_INLINE__") or {}',
    '    class __GxInlineEngine:  # 轻量包装，统一 .run 接口',
    '        run = staticmethod(__gx_inline.get("run"))',
    '    __gx_engine = __GxInlineEngine',
    '',
    '# 3) 执行并把 display 交给 worker 转发',
    '__guaxi_plotly_display__ = __gx_engine.run(__gx_source, __gx_options, globals())',
    '_result = None',
  ].join('\n')
}

export { normalizeText }
