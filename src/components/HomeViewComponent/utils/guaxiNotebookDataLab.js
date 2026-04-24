const NOTEBOOK_DATALAB_EVENT = 'guaxi:notebook-data-lab-action'
const NOTEBOOK_DATALAB_BRIDGE_KEY = '__GUAXI_NOTEBOOK_LAB__'

const normalizeText = (value) => String(value == null ? '' : value).trim()

const TABLE_TYPE_HINTS = ['dataframe', 'series']
const STRUCTURED_TYPE_HINTS = ['dict', 'list', 'tuple', 'set', 'ndarray', 'array', 'json']
const TEXT_TYPE_HINTS = ['str', 'string', 'text']

const clampVariableName = (value, fallback = 'data_item') => {
  const normalized = normalizeText(value)
    .replace(/[^\w\u4e00-\u9fa5]+/g, '_')
    .replace(/^_+|_+$/g, '')
  return normalized || fallback
}

const buildImportVariableName = (sourceLabel = '', fallback = 'data_item') => {
  const base = normalizeText(sourceLabel).replace(/\.[^.]+$/, '')
  return clampVariableName(base, fallback).replace(/-/g, '_')
}

export const classifyNotebookLabVariable = (item = {}) => {
  const valueType = normalizeText(item?.value_type || item?.type).toLowerCase()
  const hasTableShape = Number.isFinite(Number(item?.rows)) || Number.isFinite(Number(item?.columns))

  if (hasTableShape || TABLE_TYPE_HINTS.some((hint) => valueType.includes(hint))) {
    return 'table'
  }
  if (STRUCTURED_TYPE_HINTS.some((hint) => valueType.includes(hint))) {
    return 'structured'
  }
  if (TEXT_TYPE_HINTS.some((hint) => valueType.includes(hint))) {
    return 'text'
  }
  return 'value'
}

export const upsertNotebookLabImportRecord = (imports = [], record = null) => {
  const normalizedRecord = record && typeof record === 'object'
    ? {
      variableName: normalizeText(record.variableName || record.name),
      sourceType: normalizeText(record.sourceType || 'unknown'),
      sourceLabel: normalizeText(record.sourceLabel || ''),
      dataKind: normalizeText(record.dataKind || 'value'),
      importedAt: normalizeText(record.importedAt || new Date().toISOString()),
    }
    : null
  if (!normalizedRecord?.variableName) {
    return Array.isArray(imports) ? [...imports] : []
  }

  const next = Array.isArray(imports) ? [...imports] : []
  const index = next.findIndex((item) => normalizeText(item?.variableName) === normalizedRecord.variableName)
  if (index >= 0) {
    next.splice(index, 1, {
      ...next[index],
      ...normalizedRecord,
    })
    return next
  }
  return [normalizedRecord, ...next]
}

export const buildNotebookLabDatasetDescriptors = (sessionVars = [], imports = []) => {
  const importMap = new Map()
  ;(Array.isArray(imports) ? imports : []).forEach((item) => {
    const variableName = normalizeText(item?.variableName)
    if (!variableName) return
    importMap.set(variableName, item)
  })

  return (Array.isArray(sessionVars) ? sessionVars : []).map((item) => {
    const variableName = normalizeText(item?.name)
    const importMeta = importMap.get(variableName) || null
    return {
      variableName,
      valueType: normalizeText(item?.value_type || item?.type || 'value'),
      dataKind: classifyNotebookLabVariable(item),
      preview: String(item?.preview || ''),
      shape: Array.isArray(item?.shape) ? [...item.shape] : [],
      length: item?.length ?? null,
      rows: item?.rows ?? null,
      columns: item?.columns ?? null,
      sourceType: normalizeText(importMeta?.sourceType || ''),
      sourceLabel: normalizeText(importMeta?.sourceLabel || ''),
      importedAt: normalizeText(importMeta?.importedAt || ''),
    }
  })
}

const buildFileImportCode = ({ sourcePath = '', variableName = '', sourceLabel = '' } = {}) => {
  return [
    'from pathlib import Path',
    'import json',
    'import pandas as pd',
    `__guaxi_lab_source_path = ${JSON.stringify(String(sourcePath || ''))}`,
    `__guaxi_lab_var_name = ${JSON.stringify(String(variableName || 'data_item'))}`,
    `__guaxi_lab_source_label = ${JSON.stringify(String(sourceLabel || sourcePath || 'file'))}`,
    "__guaxi_lab_kind = 'value'",
    '__guaxi_lab_path = Path(__guaxi_lab_source_path)',
    '__guaxi_lab_ext = __guaxi_lab_path.suffix.lower()',
    "if __guaxi_lab_ext in ('.csv', '.tsv', '.xlsx', '.xls', '.parquet'):",
    "    if __guaxi_lab_ext == '.csv':",
    '        __guaxi_lab_value = pd.read_csv(__guaxi_lab_source_path)',
    "    elif __guaxi_lab_ext == '.tsv':",
    "        __guaxi_lab_value = pd.read_csv(__guaxi_lab_source_path, sep='\\t')",
    "    elif __guaxi_lab_ext in ('.xlsx', '.xls'):",
    '        __guaxi_lab_value = pd.read_excel(__guaxi_lab_source_path, sheet_name=0)',
    '    else:',
    '        __guaxi_lab_value = pd.read_parquet(__guaxi_lab_source_path)',
    "    __guaxi_lab_kind = 'table'",
    "elif __guaxi_lab_ext in ('.json',):",
    "    __guaxi_lab_text = __guaxi_lab_path.read_text(encoding='utf-8')",
    '    __guaxi_lab_value = json.loads(__guaxi_lab_text)',
    "    __guaxi_lab_kind = 'structured'",
    '    try:',
    '        __guaxi_lab_df = pd.DataFrame(__guaxi_lab_value)',
    '        if len(__guaxi_lab_df.columns):',
    '            __guaxi_lab_value = __guaxi_lab_df',
    "            __guaxi_lab_kind = 'table'",
    '    except Exception:',
    '        pass',
    "elif __guaxi_lab_ext in ('.md', '.markdown', '.yaml', '.yml', '.log', '.py', '.js', '.ts', '.txt'):",
    "    __guaxi_lab_value = __guaxi_lab_path.read_text(encoding='utf-8')",
    "    __guaxi_lab_kind = 'text'",
    'else:',
    "    raise ValueError(f'Unsupported external data format: {__guaxi_lab_ext or __guaxi_lab_source_path}')",
    'globals()[__guaxi_lab_var_name] = __guaxi_lab_value',
    'print(',
    "    f'[GuaxiNotebookLab] imported source={__guaxi_lab_source_label} '",
    "    f'var={__guaxi_lab_var_name} kind={__guaxi_lab_kind}'",
    ')',
    '_result = None',
  ].join('\n')
}

const buildInlineImportCode = ({
  variableName = '',
  sourceLabel = '',
  text = '',
  sourceType = 'inline',
} = {}) => {
  return [
    'import json',
    'import pandas as pd',
    `__guaxi_lab_var_name = ${JSON.stringify(String(variableName || 'data_item'))}`,
    `__guaxi_lab_source_label = ${JSON.stringify(String(sourceLabel || sourceType || 'inline'))}`,
    `__guaxi_lab_raw_text = ${JSON.stringify(String(text || ''))}`,
    "__guaxi_lab_kind = 'text'",
    '__guaxi_lab_value = __guaxi_lab_raw_text',
    'try:',
    '    __guaxi_lab_json = json.loads(__guaxi_lab_raw_text)',
    '    __guaxi_lab_value = __guaxi_lab_json',
    "    __guaxi_lab_kind = 'structured'",
    '    try:',
    '        __guaxi_lab_df = pd.DataFrame(__guaxi_lab_json)',
    '        if len(__guaxi_lab_df.columns):',
    '            __guaxi_lab_value = __guaxi_lab_df',
    "            __guaxi_lab_kind = 'table'",
    '    except Exception:',
    '        pass',
    'except Exception:',
    '    pass',
    'globals()[__guaxi_lab_var_name] = __guaxi_lab_value',
    'print(',
    "    f'[GuaxiNotebookLab] imported source={__guaxi_lab_source_label} '",
    "    f'var={__guaxi_lab_var_name} kind={__guaxi_lab_kind}'",
    ')',
    '_result = None',
  ].join('\n')
}

export const buildNotebookLabImportPlan = ({
  sourceType = 'file',
  sourcePath = '',
  sourceLabel = '',
  variableName = '',
  text = '',
} = {}) => {
  const normalizedSourceType = normalizeText(sourceType || 'file').toLowerCase()
  const normalizedSourceLabel = normalizeText(sourceLabel || sourcePath || normalizedSourceType)
  const nextVariableName = clampVariableName(
    variableName || buildImportVariableName(normalizedSourceLabel, normalizedSourceType === 'clipboard' ? 'clipboard_data' : 'data_item'),
    'data_item',
  )

  if (normalizedSourceType === 'file') {
    return {
      sourceType: 'file',
      sourceLabel: normalizedSourceLabel,
      variableName: nextVariableName,
      code: buildFileImportCode({
        sourcePath,
        sourceLabel: normalizedSourceLabel,
        variableName: nextVariableName,
      }),
    }
  }

  return {
    sourceType: normalizedSourceType || 'inline',
    sourceLabel: normalizedSourceLabel,
    variableName: nextVariableName,
    code: buildInlineImportCode({
      variableName: nextVariableName,
      sourceLabel: normalizedSourceLabel,
      text,
      sourceType: normalizedSourceType || 'inline',
    }),
  }
}

export const buildNotebookLabMaterializeCode = (dataset = {}) => {
  const variableName = normalizeText(dataset?.variableName || dataset?.name)
  const dataKind = normalizeText(dataset?.dataKind || dataset?.kind).toLowerCase()

  if (!variableName) {
    return ''
  }

  if (dataKind === 'table') {
    return [
      'import pandas as pd',
      `__guaxi_lab_var_name = ${JSON.stringify(variableName)}`,
      "if __guaxi_lab_var_name not in globals(): raise KeyError(f'Variable not found: {__guaxi_lab_var_name}')",
      'print(f"[GuaxiNotebookLab] materialize table var={__guaxi_lab_var_name}")',
      '__guaxi_lab_value = globals()[__guaxi_lab_var_name]',
      'if isinstance(__guaxi_lab_value, pd.Series):',
      '    __guaxi_lab_value = __guaxi_lab_value.to_frame()',
      'elif not isinstance(__guaxi_lab_value, pd.DataFrame):',
      '    try:',
      '        __guaxi_lab_value = pd.DataFrame(__guaxi_lab_value)',
      '    except Exception:',
      '        pass',
      '_result = __guaxi_lab_value',
    ].join('\n')
  }

  if (dataKind === 'structured') {
    return [
      'import json',
      `__guaxi_lab_var_name = ${JSON.stringify(variableName)}`,
      "if __guaxi_lab_var_name not in globals(): raise KeyError(f'Variable not found: {__guaxi_lab_var_name}')",
      'print(f"[GuaxiNotebookLab] materialize structured var={__guaxi_lab_var_name}")',
      '__guaxi_lab_value = globals()[__guaxi_lab_var_name]',
      'try:',
      '    print(json.dumps(__guaxi_lab_value, ensure_ascii=False, indent=2, default=str))',
      'except Exception:',
      '    print(repr(__guaxi_lab_value))',
      '_result = None',
    ].join('\n')
  }

  if (dataKind === 'text') {
    return [
      `__guaxi_lab_var_name = ${JSON.stringify(variableName)}`,
      "if __guaxi_lab_var_name not in globals(): raise KeyError(f'Variable not found: {__guaxi_lab_var_name}')",
      'print(f"[GuaxiNotebookLab] materialize text var={__guaxi_lab_var_name}")',
      '__guaxi_lab_value = globals()[__guaxi_lab_var_name]',
      'print(str(__guaxi_lab_value))',
      '_result = None',
    ].join('\n')
  }

  return [
    `__guaxi_lab_var_name = ${JSON.stringify(variableName)}`,
    "if __guaxi_lab_var_name not in globals(): raise KeyError(f'Variable not found: {__guaxi_lab_var_name}')",
    'print(f"[GuaxiNotebookLab] materialize value var={__guaxi_lab_var_name}")',
    '__guaxi_lab_value = globals()[__guaxi_lab_var_name]',
    'print(repr(__guaxi_lab_value))',
    '_result = None',
  ].join('\n')
}

export const decorateNotebookLabOutputModules = (modules = [], dataset = {}, source = 'data-lab') => {
  const variableName = normalizeText(dataset?.variableName || dataset?.name)
  const dataKind = normalizeText(dataset?.dataKind || dataset?.kind)
  return (Array.isArray(modules) ? modules : []).map((module) => ({
    ...module,
    source,
    title: variableName || module.title,
    summary: [module.summary, dataKind ? `实验室 ${dataKind}` : '实验室输出'].filter(Boolean).join(' · '),
  }))
}

export const emitNotebookDataLabAction = (detail = {}) => {
  if (typeof window === 'undefined' || typeof window.dispatchEvent !== 'function') return false
  window.dispatchEvent(new CustomEvent(NOTEBOOK_DATALAB_EVENT, {
    detail: detail && typeof detail === 'object' ? { ...detail } : {},
  }))
  return true
}

export const installNotebookDataLabBridge = ({ getContext } = {}) => {
  if (typeof window === 'undefined') {
    return () => {}
  }

  const bridge = {
    version: 1,
    eventName: NOTEBOOK_DATALAB_EVENT,
    getContext() {
      return typeof getContext === 'function' ? (getContext() || {}) : {}
    },
    dispatch(detail = {}) {
      return emitNotebookDataLabAction(detail)
    },
  }

  window[NOTEBOOK_DATALAB_BRIDGE_KEY] = bridge
  return () => {
    if (window[NOTEBOOK_DATALAB_BRIDGE_KEY] === bridge) {
      delete window[NOTEBOOK_DATALAB_BRIDGE_KEY]
    }
  }
}

export {
  NOTEBOOK_DATALAB_BRIDGE_KEY,
  NOTEBOOK_DATALAB_EVENT,
}
