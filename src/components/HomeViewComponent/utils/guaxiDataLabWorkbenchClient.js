import { invoke } from '@tauri-apps/api/core'
import { emit, emitTo } from '@tauri-apps/api/event'
import { save } from '@tauri-apps/plugin-dialog'
import { createLogger } from '../../../utils/logger'
import {
  NOTEBOOK_LAB_CHART_TYPES,
  buildNotebookLabPlotCode,
  buildImportProbeCode,
  buildExportCode,
  normalizeImportSpec,
  normalizeExportSpec,
  normalizeAnalysisSpec,
  getExportFormat,
} from './guaxiNotebookDataLab'

const logger = createLogger('GuaxiDataLabWorkbenchClient')

const normalizeText = (value) => String(value == null ? '' : value).trim()
export const DATA_LAB_SESSION_VARS_UPDATED_EVENT = 'guaxi:data-lab-session-vars-updated'

const WORKBENCH_INTERNAL_VARIABLE_NAMES = new Set([
  '__builtins__',
  '_doc_path',
  '_guaxi_data_workbench',
  '_guaxi_session',
  '_py_cell_id',
  '_result',
])

export const WORKBENCH_SUPPORTED_EXPORTS = [
  { label: 'CSV', name: 'CSV', value: 'csv', extensions: ['csv'] },
  { label: 'Excel', name: 'Excel', value: 'xlsx', extensions: ['xlsx'] },
  { label: 'JSON', name: 'JSON', value: 'json', extensions: ['json'] },
]

export const WORKBENCH_CHART_TYPES = NOTEBOOK_LAB_CHART_TYPES

export const isWorkbenchInternalVariableName = (name) => {
  const value = normalizeText(name)
  if (!value) return true

  // 设计说明：
  // 1. 数据工作台和 Notebook 会向 Python 会话注入少量 `_guaxi_*` / `__guaxi_*` 控制变量，用于路由、快照和导出。
  // 2. 这些变量不是用户主动创建的数据对象，默认展示会挤占变量面板并造成误解，因此统一在会话变量边界过滤。
  // 3. 不直接屏蔽所有单下划线变量，保留用户自己命名 `_cache`、`_tmp` 之类变量的可见性。
  return WORKBENCH_INTERNAL_VARIABLE_NAMES.has(value)
    || value.startsWith('_guaxi_')
    || value.startsWith('__guaxi_')
}

export const filterVisibleWorkbenchSessionVariables = (items = []) => {
  const rawItems = Array.isArray(items) ? items : []
  return rawItems.filter((item) => !isWorkbenchInternalVariableName(item?.name || item?.variableName))
}

export const parseWorkbenchRouteQuery = (query = {}) => ({
  sessionId: normalizeText(query.sessionId),
  variableName: normalizeText(query.variableName),
  dataKind: normalizeText(query.dataKind || 'value'),
  filePath: normalizeText(query.filePath),
  cellId: normalizeText(query.cellId),
  cellKey: normalizeText(query.cellKey),
  stableId: normalizeText(query.stableId),
  theme: normalizeText(query.theme),
})

export const fetchWorkbenchSessionVariables = async ({
  sessionId = '',
  variableName = '',
} = {}) => {
  const name = normalizeText(variableName)
  const response = await runWorkbenchPython(sessionId, 'print("[GuaxiDataWorkbench] refreshed session variables")', {
    sessionVarNames: name ? [name] : undefined,
    vars: {
      _guaxi_data_workbench: 'session-vars',
    },
  })
  const variables = Array.isArray(response?.session_vars) ? response.session_vars : []
  const visibleVariables = filterVisibleWorkbenchSessionVariables(variables)
  logger.info('[DataLabWorkbenchClient] fetched session variables', {
    sessionId,
    variableName: name,
    variableCount: visibleVariables.length,
    hiddenInternalVariableCount: Math.max(0, variables.length - visibleVariables.length),
  })
  return visibleVariables
}

const buildPythonRequest = (sessionId, code, options = {}) => ({
  code: String(code || ''),
  persist: options.persist !== false,
  expect_json: options.expectJson !== false,
  include_session_vars: options.includeSessionVars !== false,
  session_var_names: Array.isArray(options.sessionVarNames) ? options.sessionVarNames : undefined,
  vars: {
    _guaxi_session: String(sessionId || ''),
    ...(options.vars || {}),
  },
})

const extractGuaxiResult = (response) => {
  const resultJson = response?.result_json
  if (resultJson && typeof resultJson === 'object' && '__guaxi_result__' in resultJson) {
    return resultJson.__guaxi_result__
  }
  return resultJson
}

export const runWorkbenchPython = async (sessionId, code, options = {}) => {
  if (!normalizeText(sessionId)) {
    throw new Error('sessionId 不能为空')
  }
  const response = await invoke('python_exec', {
    request: buildPythonRequest(sessionId, code, options),
  })
  if (!response?.ok) {
    throw new Error(String(response?.stderr || response?.stdout || 'Python 执行失败'))
  }
  return response
}

export const fetchWorkbenchVariableSnapshot = async ({
  sessionId = '',
  variableName = '',
  offset = 0,
  limit = 200,
} = {}) => {
  const name = normalizeText(variableName)
  if (!name) throw new Error('变量名不能为空')

  const code = [
    'import json',
    'import math',
    'import datetime',
    'try:',
    '    import pandas as pd',
    'except Exception:',
    '    pd = None',
    'try:',
    '    import numpy as np',
    'except Exception:',
    '    np = None',
    `__guaxi_name = ${JSON.stringify(name)}`,
    `__guaxi_offset = max(0, int(${Number(offset) || 0}))`,
    `__guaxi_limit = max(1, min(1000, int(${Number(limit) || 200})))`,
    'if __guaxi_name not in globals():',
    '    raise KeyError(f"变量不存在: {__guaxi_name}")',
    '__guaxi_value = globals()[__guaxi_name]',
    'def __guaxi_basic(v):',
    '    if v is None or isinstance(v, (bool, int, float, str)):',
    '        return v',
    '    if isinstance(v, (datetime.date, datetime.datetime, datetime.time)):',
    '        return v.isoformat()',
    '    if np is not None:',
    '        try:',
    '            if isinstance(v, np.generic):',
    '                return __guaxi_basic(v.item())',
    '        except Exception:',
    '            pass',
    '    return str(v)',
    'def __guaxi_jsonable(v, depth=0):',
    '    if depth > 5:',
    '        return repr(v)',
    '    if v is None or isinstance(v, (bool, int, float, str)):',
    '        return v',
    '    if isinstance(v, (datetime.date, datetime.datetime, datetime.time)):',
    '        return v.isoformat()',
    '    if np is not None:',
    '        try:',
    '            if isinstance(v, np.generic):',
    '                return __guaxi_jsonable(v.item(), depth + 1)',
    '        except Exception:',
    '            pass',
    '    if isinstance(v, dict):',
    '        return {str(k): __guaxi_jsonable(val, depth + 1) for k, val in list(v.items())[:500]}',
    '    if isinstance(v, (list, tuple, set)):',
    '        return [__guaxi_jsonable(item, depth + 1) for item in list(v)[:500]]',
    '    return repr(v)',
    'def __guaxi_table_from_df(df):',
    '    page = df.iloc[__guaxi_offset:__guaxi_offset + __guaxi_limit]',
    '    safe = page.astype(object).where(pd.notnull(page), None)',
    '    return {',
    '        "columns": [str(c) for c in df.columns.tolist()],',
    '        "rows": [[__guaxi_basic(x) for x in row] for row in safe.values.tolist()],',
    '        "rowCount": int(len(df.index)),',
    '        "offset": __guaxi_offset,',
    '        "limit": __guaxi_limit,',
    '        "dtypes": {str(k): str(v) for k, v in df.dtypes.astype(str).to_dict().items()},',
    '    }',
    'def __guaxi_sequence_page(seq):',
    '    raw = list(seq)',
    '    page = raw[__guaxi_offset:__guaxi_offset + __guaxi_limit]',
    '    return {',
    '        "columns": ["index", "value"],',
    '        "rows": [[__guaxi_offset + idx, __guaxi_jsonable(item)] for idx, item in enumerate(page)],',
    '        "rowCount": len(raw),',
    '        "offset": __guaxi_offset,',
    '        "limit": __guaxi_limit,',
    '        "dtypes": {"index": "int", "value": "object"},',
    '    }',
    'def __guaxi_dict_page(obj):',
    '    pairs = list(obj.items())',
    '    page = pairs[__guaxi_offset:__guaxi_offset + __guaxi_limit]',
    '    return {',
    '        "columns": ["key", "value"],',
    '        "rows": [[str(k), __guaxi_jsonable(v)] for k, v in page],',
    '        "rowCount": len(pairs),',
    '        "offset": __guaxi_offset,',
    '        "limit": __guaxi_limit,',
    '        "dtypes": {"key": "str", "value": "object"},',
    '    }',
    '__guaxi_kind = type(__guaxi_value).__name__',
    '__guaxi_table = None',
    '__guaxi_structure = None',
    'if pd is not None and isinstance(__guaxi_value, pd.Series):',
    '    __guaxi_kind = "series"',
    '    __guaxi_table = __guaxi_table_from_df(__guaxi_value.to_frame(name=str(__guaxi_value.name or "value")).reset_index())',
    'elif pd is not None and isinstance(__guaxi_value, pd.DataFrame):',
    '    __guaxi_kind = "dataframe"',
    '    __guaxi_table = __guaxi_table_from_df(__guaxi_value)',
    'elif np is not None and isinstance(__guaxi_value, np.ndarray):',
    '    __guaxi_kind = "ndarray"',
    '    if __guaxi_value.ndim <= 2:',
    '        __guaxi_arr_df = pd.DataFrame(__guaxi_value) if pd is not None else None',
    '        __guaxi_table = __guaxi_table_from_df(__guaxi_arr_df) if __guaxi_arr_df is not None else __guaxi_sequence_page(__guaxi_value.tolist())',
    '    else:',
    '        __guaxi_structure = __guaxi_jsonable(__guaxi_value.tolist())',
    'elif isinstance(__guaxi_value, dict):',
    '    __guaxi_kind = "dict"',
    '    __guaxi_table = __guaxi_dict_page(__guaxi_value)',
    '    __guaxi_structure = __guaxi_jsonable(__guaxi_value)',
    'elif isinstance(__guaxi_value, list):',
    '    __guaxi_kind = "list"',
    '    __guaxi_table = __guaxi_sequence_page(__guaxi_value)',
    '    __guaxi_structure = __guaxi_jsonable(__guaxi_value)',
    'elif isinstance(__guaxi_value, tuple):',
    '    __guaxi_kind = "tuple"',
    '    __guaxi_table = __guaxi_sequence_page(__guaxi_value)',
    '    __guaxi_structure = __guaxi_jsonable(__guaxi_value)',
    'elif isinstance(__guaxi_value, str):',
    '    __guaxi_kind = "text"',
    '    __guaxi_structure = __guaxi_value',
    'else:',
    '    __guaxi_structure = __guaxi_jsonable(__guaxi_value)',
    'try:',
    '    __guaxi_shape = list(getattr(__guaxi_value, "shape", []))',
    'except Exception:',
    '    __guaxi_shape = []',
    'try:',
    '    __guaxi_length = len(__guaxi_value) if not isinstance(__guaxi_value, (str, bytes, bytearray)) else len(str(__guaxi_value))',
    'except Exception:',
    '    __guaxi_length = None',
    '_result = {',
    '    "name": __guaxi_name,',
    '    "kind": __guaxi_kind,',
    '    "type": type(__guaxi_value).__name__,',
    '    "shape": __guaxi_shape,',
    '    "length": __guaxi_length,',
    '    "preview": repr(__guaxi_value)[:1200],',
    '    "table": __guaxi_table,',
    '    "structure": __guaxi_structure,',
    '}',
  ].join('\n')

  const response = await runWorkbenchPython(sessionId, code, {
    sessionVarNames: [name],
    vars: {
      _guaxi_data_workbench: 'snapshot',
    },
  })
  const result = extractGuaxiResult(response)
  logger.info('[DataLabWorkbenchClient] fetched variable snapshot', {
    sessionId,
    variableName: name,
    kind: String(result?.kind || ''),
    offset,
    limit,
  })
  return {
    snapshot: result && typeof result === 'object' ? result : null,
    sessionVars: filterVisibleWorkbenchSessionVariables(response?.session_vars),
  }
}

export const buildApplyEditsCode = ({
  variableName = '',
  edits = [],
  saveAs = '',
} = {}) => {
  const name = normalizeText(variableName)
  const targetName = normalizeText(saveAs)
  const safeEdits = Array.isArray(edits) ? edits : []
  return [
    'import copy',
    'import json',
    'try:',
    '    import pandas as pd',
    'except Exception:',
    '    pd = None',
    'try:',
    '    import numpy as np',
    'except Exception:',
    '    np = None',
    `__guaxi_name = ${JSON.stringify(name)}`,
    `__guaxi_target_name = ${JSON.stringify(targetName)}`,
    // 设计说明：
    // 1. JavaScript JSON 里的 null/true/false 不能直接作为 Python 字面量执行，否则会触发 NameError。
    // 2. 这里把编辑队列作为 JSON 字符串交给 Python 的 json.loads 解析，保证 null -> None、true/false -> True/False。
    // 3. 后续单元格值仍走 __guaxi_parse_value，避免把普通字符串误当成 Python 表达式执行。
    `__guaxi_edits = json.loads(${JSON.stringify(JSON.stringify(safeEdits))})`,
    'if __guaxi_name not in globals():',
    '    raise KeyError(f"变量不存在: {__guaxi_name}")',
    '__guaxi_value = globals()[__guaxi_name]',
    '__guaxi_next = copy.deepcopy(__guaxi_value)',
    'def __guaxi_parse_value(raw):',
    '    if isinstance(raw, str):',
    '        text = raw.strip()',
    '        if text == "":',
    '            return ""',
    '        try:',
    '            return json.loads(text)',
    '        except Exception:',
    '            return raw',
    '    return raw',
    'for __guaxi_edit in __guaxi_edits:',
    '    __guaxi_row = int(__guaxi_edit.get("rowIndex", 0))',
    '    __guaxi_col = str(__guaxi_edit.get("column", ""))',
    '    __guaxi_value_next = __guaxi_parse_value(__guaxi_edit.get("value"))',
    '    if pd is not None and isinstance(__guaxi_next, pd.DataFrame):',
    '        if __guaxi_col not in __guaxi_next.columns:',
    '            raise KeyError(f"列不存在: {__guaxi_col}")',
    '        __guaxi_next.iat[__guaxi_row, list(__guaxi_next.columns).index(__guaxi_col)] = __guaxi_value_next',
    '    elif pd is not None and isinstance(__guaxi_next, pd.Series):',
    '        __guaxi_next.iloc[__guaxi_row] = __guaxi_value_next',
    '    elif np is not None and isinstance(__guaxi_next, np.ndarray):',
    '        __guaxi_next[__guaxi_row, int(__guaxi_col)] = __guaxi_value_next',
    '    elif isinstance(__guaxi_next, list):',
    '        __guaxi_next[__guaxi_row] = __guaxi_value_next',
    '    elif isinstance(__guaxi_next, dict):',
    '        __guaxi_key = __guaxi_edit.get("key")',
    '        if __guaxi_key is None:',
    '            __guaxi_key = list(__guaxi_next.keys())[__guaxi_row]',
    '        __guaxi_next[__guaxi_key] = __guaxi_value_next',
    '    else:',
    '        raise TypeError(f"当前类型不支持表格化编辑: {type(__guaxi_next).__name__}")',
    'globals()[__guaxi_target_name or __guaxi_name] = __guaxi_next',
    'print(f"[GuaxiDataWorkbench] applied edits variable={__guaxi_name} target={__guaxi_target_name or __guaxi_name} count={len(__guaxi_edits)}")',
    '_result = None',
  ].join('\n')
}

export const applyWorkbenchEdits = async ({
  sessionId = '',
  variableName = '',
  edits = [],
  saveAs = '',
} = {}) => {
  const code = buildApplyEditsCode({ variableName, edits, saveAs })
  const response = await runWorkbenchPython(sessionId, code, {
    sessionVarNames: [normalizeText(saveAs) || normalizeText(variableName)],
    vars: {
      _guaxi_data_workbench: 'apply-edits',
    },
  })
  logger.info('[DataLabWorkbenchClient] applied variable edits', {
    sessionId,
    variableName,
    saveAs,
    editCount: Array.isArray(edits) ? edits.length : 0,
  })
  return response
}

export const emitWorkbenchVariableUpdated = async ({
  sessionId = '',
  variableName = '',
  filePath = '',
  cellId = '',
  source = 'data-workbench',
} = {}) => {
  const payload = {
    sessionId: normalizeText(sessionId),
    variableName: normalizeText(variableName),
    filePath: normalizeText(filePath),
    cellId: normalizeText(cellId),
    source: normalizeText(source) || 'data-workbench',
    updatedAt: new Date().toISOString(),
  }
  if (!payload.sessionId || !payload.variableName) return

  await emit(DATA_LAB_SESSION_VARS_UPDATED_EVENT, payload)
  logger.info('[DataLabWorkbenchClient] emitted variable updated event', payload)
}

// 按完整 Export Spec 在当前会话直接执行导出（落盘 / 写库 / 自定义脚本）。
// target.path 等已由导出向导（含 save 对话框）解析好，这里只负责执行并回传结果。
export const runWorkbenchExportSpec = async ({ sessionId = '', spec = {} } = {}) => {
  const normalizedSpec = normalizeExportSpec(spec)
  const code = buildExportCode(normalizedSpec)
  const response = await runWorkbenchPython(sessionId, code, {
    includeSessionVars: false,
    vars: {
      _guaxi_data_workbench: 'export',
    },
  })
  const result = extractGuaxiResult(response)
  logger.info('[DataLabWorkbenchClient] exported variable via spec', {
    sessionId,
    variableName: normalizedSpec.variableName,
    format: normalizedSpec.format,
    outputPath: result?.outputPath || '',
  })
  return result && typeof result === 'object' ? result : { outputPath: normalizedSpec.target.path || '', format: normalizedSpec.format }
}

// 旧的一键导出（CSV/Excel/JSON）：保留入口，先弹保存对话框再委托统一执行器。
export const exportWorkbenchVariable = async ({
  sessionId = '',
  variableName = '',
  format = 'csv',
} = {}) => {
  const normalizedFormat = normalizeText(format || 'csv').toLowerCase()
  const formatSpec = getExportFormat(normalizedFormat === 'xlsx' ? 'excel' : normalizedFormat)
  const ext = formatSpec?.ext?.[0] || (normalizedFormat === 'xlsx' ? 'xlsx' : normalizedFormat)
  const outputPath = await save({
    defaultPath: `${normalizeText(variableName) || 'guaxi_data'}.${ext}`,
    filters: [{ name: String(formatSpec?.label || ext).toUpperCase(), extensions: [ext] }],
  })
  if (!outputPath) return null

  await runWorkbenchExportSpec({
    sessionId,
    spec: {
      format: formatSpec?.id || (normalizedFormat === 'xlsx' ? 'excel' : normalizedFormat),
      variableName: normalizeText(variableName),
      target: { path: String(outputPath || '') },
    },
  })
  logger.info('[DataLabWorkbenchClient] exported variable', { sessionId, variableName, format: normalizedFormat, outputPath })
  return outputPath
}

export const writeWorkbenchChartToNotebook = async ({
  context = {},
  chart = {},
  writeBack = false,
} = {}) => {
  const variableName = normalizeText(context.variableName)
  const code = buildNotebookLabPlotCode({
    variableName,
    chartType: normalizeText(chart.chartType || 'line'),
    x: normalizeText(chart.x),
    y: normalizeText(chart.y),
    ySeries: Array.isArray(chart.ySeries) ? chart.ySeries : [],
    z: normalizeText(chart.z),
    color: normalizeText(chart.color),
    title: normalizeText(chart.title) || `${variableName} 图表`,
  })
  await emitTo('main', 'notebook-data-lab-rpc', {
    action: writeBack ? 'plot-variable-writeback' : 'plot-variable',
    filePath: normalizeText(context.filePath),
    cellId: normalizeText(context.cellId),
    cellKey: normalizeText(context.cellKey),
    stableId: normalizeText(context.stableId),
    variableName,
    dataKind: normalizeText(context.dataKind),
    chartType: normalizeText(chart.chartType || 'line'),
    x: normalizeText(chart.x),
    y: normalizeText(chart.y),
    ySeries: Array.isArray(chart.ySeries) ? chart.ySeries : [],
    z: normalizeText(chart.z),
    color: normalizeText(chart.color),
    title: normalizeText(chart.title),
    code,
  })
  logger.info('[DataLabWorkbenchClient] emitted chart action to notebook', {
    variableName,
    chartType: normalizeText(chart.chartType || 'line'),
    writeBack,
  })
}

// 把分析工作台产出的完整 Analysis Spec 发往主窗口 Notebook 执行（analyze-variable 动作）。
// notebook 侧据 spec 调 buildAnalysisCode 重新生成代码并执行，产出交互图 + 结论摘要。
export const writeWorkbenchAnalysisToNotebook = async ({
  context = {},
  analysis = {},
  writeBack = false,
} = {}) => {
  const spec = normalizeAnalysisSpec({ ...analysis, primaryVariable: analysis.primaryVariable || context.variableName })
  await emitTo('main', 'notebook-data-lab-rpc', {
    action: writeBack ? 'analyze-variable-writeback' : 'analyze-variable',
    filePath: normalizeText(context.filePath),
    cellId: normalizeText(context.cellId),
    cellKey: normalizeText(context.cellKey),
    stableId: normalizeText(context.stableId),
    variableName: normalizeText(context.variableName),
    dataKind: 'analysis',
    method: spec.method,
    features: spec.features,
    target: spec.target,
    group: spec.group,
    color: spec.color,
    timeIndex: spec.timeIndex,
    primaryVariable: spec.primaryVariable,
    title: spec.title,
    options: spec.options,
  })
  logger.info('[DataLabWorkbenchClient] emitted analysis action to notebook', {
    method: spec.method,
    featureCount: spec.features.length,
    writeBack,
  })
  return spec
}

export const emitWorkbenchNotebookAction = async ({
  context = {},
  action = '',
  variableName = '',
  dataKind = '',
  extra = {},
} = {}) => {
  const payload = {
    action: normalizeText(action),
    filePath: normalizeText(context.filePath),
    cellId: normalizeText(context.cellId),
    cellKey: normalizeText(context.cellKey),
    stableId: normalizeText(context.stableId),
    variableName: normalizeText(variableName || context.variableName),
    dataKind: normalizeText(dataKind || context.dataKind),
    ...(extra && typeof extra === 'object' ? extra : {}),
  }
  await emitTo('main', 'notebook-data-lab-rpc', payload)
  logger.info('[DataLabWorkbenchClient] emitted notebook action', {
    action: payload.action,
    variableName: payload.variableName,
    dataKind: payload.dataKind,
  })
}

// 只读探测导入源：在当前会话直接执行探测代码，列 sheet/表、嗅探 CSV、预览 JSON/日志/脚本，
// 不写入任何会话变量，结果直接返回给导入向导用于配置确认。
export const probeWorkbenchImportSource = async ({ sessionId = '', spec = {} } = {}) => {
  const normalizedSpec = normalizeImportSpec(spec)
  const code = buildImportProbeCode(normalizedSpec)
  const response = await runWorkbenchPython(sessionId, code, {
    includeSessionVars: false,
    vars: {
      _guaxi_data_workbench: 'import-probe',
    },
  })
  const result = extractGuaxiResult(response)
  logger.info('[DataLabWorkbenchClient] probed import source', {
    sessionId,
    category: normalizedSpec.category,
    format: normalizedSpec.format,
    ok: result?.ok !== false,
  })
  if (result && result.ok === false) {
    throw new Error(String(result.error || '探测失败'))
  }
  return result && typeof result === 'object' ? result : {}
}

// 把导入向导产出的完整 Import Spec 发往主窗口 Notebook 执行与记账（统一 import-source 动作）。
export const emitWorkbenchImportSource = async ({ context = {}, spec = {} } = {}) => {
  const normalizedSpec = normalizeImportSpec(spec)
  await emitTo('main', 'notebook-data-lab-rpc', {
    action: 'import-source',
    filePath: normalizeText(context.filePath),
    cellId: normalizeText(context.cellId),
    cellKey: normalizeText(context.cellKey),
    stableId: normalizeText(context.stableId),
    variableName: normalizedSpec.variableName,
    sourceType: normalizedSpec.category,
    sourceLabel: normalizedSpec.sourceLabel,
    spec: normalizedSpec,
  })
  logger.info('[DataLabWorkbenchClient] emitted import-source action', {
    category: normalizedSpec.category,
    format: normalizedSpec.format,
    variableName: normalizedSpec.variableName,
  })
  return normalizedSpec
}

export const requestNotebookInspectorRefresh = async (context = {}) => {
  await emitTo('main', 'notebook-data-lab-rpc', {
    action: 'refresh-session-inspector',
    filePath: normalizeText(context.filePath),
    cellId: normalizeText(context.cellId),
    cellKey: normalizeText(context.cellKey),
    stableId: normalizeText(context.stableId),
    variableName: normalizeText(context.variableName),
    dataKind: normalizeText(context.dataKind),
  })
  logger.info('[DataLabWorkbenchClient] requested notebook inspector refresh', {
    variableName: normalizeText(context.variableName),
  })
}
