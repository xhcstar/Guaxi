// ============================================================================
// guaxiMathWorkbenchClient.js —— 「数学工作台」独立窗口的会话客户端
// ----------------------------------------------------------------------------
// 设计说明：
//   1. 本文件服务于数学块承载层的最外层（独立窗口 /math-workbench），
//      与 Python Cell 的 guaxiDataLabWorkbenchClient.js 完全独立——不 import、不共享，
//      仅同样通过通用的 invoke('python_exec') 连接同一持久 Python 会话。
//   2. 重型结果（carrier）登记在会话 globals 的 __GUAXIMATH_CARRIERS__ 里；本客户端
//      负责：读取 carrier 参数、按新参数重建命令源码并复跑、读取仿真解数组用于回放。
//   3. 命令源码复用数学块自己的 bootstrap 生成器 buildNotebookLabMathCode（同属数学模块）。
// ============================================================================
import { invoke } from '@tauri-apps/api/core'
import { buildNotebookLabMathCode } from './guaxiNotebookMathRuntime'

const normalizeText = (value) => String(value == null ? '' : value).trim()

export const parseMathWorkbenchRouteQuery = (query = {}) => ({
  sessionId: normalizeText(query.sessionId),
  carrierKey: normalizeText(query.carrierKey),
  command: normalizeText(query.command),
  filePath: normalizeText(query.filePath),
  cellId: normalizeText(query.cellId),
  cellKey: normalizeText(query.cellKey),
  stableId: normalizeText(query.stableId),
  theme: normalizeText(query.theme) === 'dark' ? 'dark' : 'light',
})

// 在指定持久会话里跑一段代码（无 dataframe 概念，纯 math 用途）。
const runSessionCode = async (sessionId, code, { expectJson = false, filePath = '', cellId = '' } = {}) => {
  if (!normalizeText(sessionId)) throw new Error('sessionId 不能为空')
  const response = await invoke('python_exec', {
    request: {
      code: String(code || ''),
      persist: true,
      expect_json: !!expectJson,
      include_session_vars: false,
      vars: {
        _guaxi_session: String(sessionId || ''),
        _doc_path: String(filePath || ''),
        _py_cell_id: String(cellId || ''),
      },
    },
  })
  return response
}

const parseSentinelJson = (stdout, sentinel) => {
  const text = String(stdout || '')
  const at = text.indexOf(sentinel)
  if (at < 0) return null
  const line = text.slice(at + sentinel.length).split('\n')[0]
  try {
    return JSON.parse(line)
  } catch (error) {
    return null
  }
}

// —— 读取单个 carrier 的元数据（参数等）—— //
const buildReadCarrierCode = (carrierKey) => [
  'import json as __mwb_json',
  `__mwb_key = ${JSON.stringify(String(carrierKey || ''))}`,
  '__mwb_c = (globals().get("__GUAXIMATH_CARRIERS__", {}) or {}).get(__mwb_key, None)',
  'print("__GUAXI_MWB__" + __mwb_json.dumps(__mwb_c, ensure_ascii=False, default=str))',
  'for __mwb_k in ("__mwb_json", "__mwb_key", "__mwb_c", "__mwb_k"):',
  '    globals().pop(__mwb_k, None)',
].join('\n')

export const fetchMathCarrier = async (sessionId, carrierKey, ctx = {}) => {
  const response = await runSessionCode(sessionId, buildReadCarrierCode(carrierKey), {
    expectJson: false, filePath: ctx.filePath, cellId: ctx.cellId,
  })
  if (!response?.ok) throw new Error(String(response?.stderr || '读取工作台参数失败'))
  return parseSentinelJson(response?.stdout, '__GUAXI_MWB__')
}

// —— 读取仿真解数组（轻量回放骨架用）—— //
const buildReadSolutionCode = (solutionVar) => [
  'import json as __mwb_json',
  `__mwb_var = ${JSON.stringify(String(solutionVar || ''))}`,
  '__mwb_sol = globals().get(__mwb_var, None)',
  'print("__GUAXI_MWBSOL__" + __mwb_json.dumps(__mwb_sol, ensure_ascii=False, default=str))',
  'for __mwb_k in ("__mwb_json", "__mwb_var", "__mwb_sol", "__mwb_k"):',
  '    globals().pop(__mwb_k, None)',
].join('\n')

export const fetchMathSolution = async (sessionId, solutionVar, ctx = {}) => {
  if (!normalizeText(solutionVar)) return null
  const response = await runSessionCode(sessionId, buildReadSolutionCode(solutionVar), {
    expectJson: false, filePath: ctx.filePath, cellId: ctx.cellId,
  })
  if (!response?.ok) throw new Error(String(response?.stderr || '读取仿真解失败'))
  return parseSentinelJson(response?.stdout, '__GUAXI_MWBSOL__')
}

// —— 按（覆盖后的）参数重建命令源码 —— //
const fmtNum = (v) => {
  const n = Number(v)
  return Number.isFinite(n) ? String(n) : '0'
}
const fmtList = (arr) => `[${(Array.isArray(arr) ? arr : [arr]).map(fmtNum).join(', ')}]`
const fmtRange = (r) => `(${r?.[0]}, ${fmtNum(r?.[1])}, ${fmtNum(r?.[2])})`

export const buildRerunSource = (carrier, overrides = {}) => {
  const command = String(carrier?.command || '')
  const p = { ...(carrier?.params || {}), ...(overrides || {}) }
  switch (command) {
    case 'nsolve':
      return `nsolve(${p.expr}, ${fmtNum(p.x0)}, tol=${fmtNum(p.tol)}, maxiter=${fmtNum(p.maxiter)})`
    case 'minimize':
      return `minimize(${p.expr}, ${fmtList(p.x0)}, method='${p.method || 'BFGS'}', tol=${fmtNum(p.tol)}, maxiter=${fmtNum(p.maxiter)})`
    case 'odeint': {
      const rhsList = Array.isArray(p.rhs) ? p.rhs : [p.rhs]
      const rhs = rhsList.length === 1 ? rhsList[0] : `[${rhsList.join(', ')}]`
      const y0List = Array.isArray(p.y0) ? p.y0 : [p.y0]
      const y0 = y0List.length === 1 ? fmtNum(y0List[0]) : fmtList(y0List)
      const vars = Array.isArray(p.state) && p.state.length > 1 ? `, vars=[${p.state.join(', ')}]` : ''
      return `odeint(${rhs}, ${y0}, ${fmtRange(p.trange)}, n=${fmtNum(p.n)}${vars})`
    }
    case 'phase_portrait': {
      const rhs = `[${(p.rhs || []).join(', ')}]`
      const vars = Array.isArray(p.state) && p.state.length ? `, vars=[${p.state.join(', ')}]` : ''
      return `phase_portrait(${rhs}, ${fmtRange(p.xr)}, ${fmtRange(p.yr)}, density=${fmtNum(p.density)}${vars})`
    }
    default:
      // 未知命令：尽力用 summary 里的源码或直接回放 command 名（不报错）。
      return String(p.source || command || '')
  }
}

// 取出 worker 转发的 display（单个或数组），统一成数组。
export const extractMathDisplays = (response) => {
  const display = response?.result_json?.__guaxi_display__
    ?? response?.result?.result_json?.__guaxi_display__
    ?? null
  if (Array.isArray(display)) return display.filter((d) => d && typeof d === 'object')
  if (display && typeof display === 'object') return [display]
  return []
}

// 复跑 carrier：按新参数重建源码 → 生成 bootstrap → 执行 → 返回 display 数组 + 原始响应。
export const rerunMathCarrier = async (sessionId, carrier, overrides = {}, ctx = {}) => {
  const source = buildRerunSource(carrier, overrides)
  if (!normalizeText(source)) throw new Error('无法重建命令源码')
  const code = buildNotebookLabMathCode({ source, theme: ctx.theme || 'light' })
  const response = await runSessionCode(sessionId, code, {
    expectJson: true, filePath: ctx.filePath, cellId: ctx.cellId,
  })
  if (!response?.ok && response?.ok !== undefined) {
    // 引擎层错误信息通常已包在 display 里，这里只在硬失败时抛。
    if (!response?.result_json) throw new Error(String(response?.stderr || '复跑失败'))
  }
  return { source, displays: extractMathDisplays(response), response }
}

export { normalizeText }
