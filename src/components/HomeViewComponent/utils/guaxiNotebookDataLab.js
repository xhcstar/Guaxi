const NOTEBOOK_DATALAB_EVENT = 'guaxi:notebook-data-lab-action'
const NOTEBOOK_DATALAB_BRIDGE_KEY = '__GUAXI_NOTEBOOK_LAB__'

const normalizeText = (value) => String(value == null ? '' : value).trim()

const TABLE_TYPE_HINTS = ['dataframe', 'series']
const STRUCTURED_TYPE_HINTS = ['dict', 'list', 'tuple', 'set', 'ndarray', 'array', 'json']
const TEXT_TYPE_HINTS = ['str', 'string', 'text']

export const NOTEBOOK_LAB_CHART_TYPES = [
  { value: 'line', label: '折线图' },
  { value: 'bar', label: '柱状图' },
  { value: 'scatter', label: '散点图' },
  { value: 'area', label: '面积图' },
  { value: 'histogram', label: '直方图' },
  { value: 'box', label: '箱线图' },
  { value: 'violin', label: '小提琴图' },
  { value: 'pie', label: '饼图' },
  { value: 'heatmap', label: '热力图' },
  { value: 'corr', label: '相关矩阵' },
  { value: 'density_heatmap', label: '密度热力图' },
  { value: 'density_contour', label: '密度等高线' },
  { value: 'scatter_matrix', label: '散点矩阵' },
  { value: 'parallel', label: '平行坐标' },
  { value: 'radar', label: '雷达图' },
  { value: 'treemap', label: '矩形树图' },
  { value: 'sunburst', label: '旭日图' },
  { value: 'funnel', label: '漏斗图' },
  { value: 'ecdf', label: 'ECDF' },
  { value: 'scatter3d', label: '3D 散点图' },
  { value: 'line3d', label: '3D 折线图' },
  { value: 'surface3d', label: '3D 曲面图' },
  { value: 'candlestick', label: 'K 线图' },
]

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
      columnNames: Array.isArray(item?.column_names) ? item.column_names.map((column) => String(column ?? '')) : [],
      dtypes: item?.dtypes && typeof item.dtypes === 'object' ? { ...item.dtypes } : {},
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
    "if __guaxi_lab_ext in ('.csv', '.tsv', '.xlsx', '.xls', '.parquet', '.feather', '.db', '.sqlite', '.sqlite3', '.duckdb'):",
    "    if __guaxi_lab_ext == '.csv':",
    '        __guaxi_lab_value = pd.read_csv(__guaxi_lab_source_path)',
    "    elif __guaxi_lab_ext == '.tsv':",
    "        __guaxi_lab_value = pd.read_csv(__guaxi_lab_source_path, sep='\\t')",
    "    elif __guaxi_lab_ext in ('.xlsx', '.xls'):",
    '        __guaxi_lab_value = pd.read_excel(__guaxi_lab_source_path, sheet_name=0)',
    "    elif __guaxi_lab_ext == '.parquet':",
    '        __guaxi_lab_value = pd.read_parquet(__guaxi_lab_source_path)',
    "    elif __guaxi_lab_ext == '.feather':",
    '        __guaxi_lab_value = pd.read_feather(__guaxi_lab_source_path)',
    '    else:',
    '        import sqlite3',
    '        __guaxi_lab_conn = sqlite3.connect(__guaxi_lab_source_path)',
    "        __guaxi_lab_tables = pd.read_sql_query(\"select name from sqlite_master where type='table' order by name\", __guaxi_lab_conn)",
    '        if __guaxi_lab_tables.empty:',
    "            raise ValueError('No table found in database file')",
    '        __guaxi_lab_table_name = str(__guaxi_lab_tables.iloc[0, 0])',
    '        __guaxi_lab_value = pd.read_sql_query(f\'select * from "{__guaxi_lab_table_name}"\', __guaxi_lab_conn)',
    '        __guaxi_lab_conn.close()',
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

const buildUrlImportCode = ({ sourceUrl = '', variableName = '', sourceLabel = '' } = {}) => {
  return [
    'import json',
    'import pandas as pd',
    `__guaxi_lab_source_url = ${JSON.stringify(String(sourceUrl || ''))}`,
    `__guaxi_lab_var_name = ${JSON.stringify(String(variableName || 'remote_data'))}`,
    `__guaxi_lab_source_label = ${JSON.stringify(String(sourceLabel || sourceUrl || 'url'))}`,
    "__guaxi_lab_ext = __guaxi_lab_source_url.split('?')[0].split('#')[0].lower().rsplit('.', 1)[-1] if '.' in __guaxi_lab_source_url.split('?')[0] else ''",
    "if __guaxi_lab_ext in ('csv', 'tsv'):",
    "    __guaxi_lab_value = pd.read_csv(__guaxi_lab_source_url, sep='\\t' if __guaxi_lab_ext == 'tsv' else ',')",
    "    __guaxi_lab_kind = 'table'",
    "elif __guaxi_lab_ext in ('xlsx', 'xls'):",
    '    __guaxi_lab_value = pd.read_excel(__guaxi_lab_source_url, sheet_name=0)',
    "    __guaxi_lab_kind = 'table'",
    "elif __guaxi_lab_ext in ('json', 'jsonl'):",
    "    if __guaxi_lab_ext == 'jsonl':",
    '        __guaxi_lab_value = pd.read_json(__guaxi_lab_source_url, lines=True)',
    "        __guaxi_lab_kind = 'table'",
    '    else:',
    '        try:',
    '            __guaxi_lab_value = pd.read_json(__guaxi_lab_source_url)',
    "            __guaxi_lab_kind = 'table'",
    '        except Exception:',
    '            import urllib.request',
    '            with urllib.request.urlopen(__guaxi_lab_source_url) as __guaxi_lab_resp:',
    "                __guaxi_lab_value = json.loads(__guaxi_lab_resp.read().decode('utf-8'))",
    "            __guaxi_lab_kind = 'structured'",
    'else:',
    '    import urllib.request',
    '    with urllib.request.urlopen(__guaxi_lab_source_url) as __guaxi_lab_resp:',
    "        __guaxi_lab_value = __guaxi_lab_resp.read().decode('utf-8')",
    "    __guaxi_lab_kind = 'text'",
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
  sourceUrl = '',
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

  if (normalizedSourceType === 'url') {
    return {
      sourceType: 'url',
      sourceLabel: normalizedSourceLabel || sourceUrl,
      variableName: nextVariableName,
      code: buildUrlImportCode({
        sourceUrl,
        sourceLabel: normalizedSourceLabel || sourceUrl,
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

export const buildNotebookLabPlotCode = ({
  variableName = '',
  chartType = 'line',
  x = '',
  y = '',
  ySeries = [],
  z = '',
  color = '',
  title = '',
} = {}) => {
  const normalizedVariableName = normalizeText(variableName)
  if (!normalizedVariableName) return ''
  const safeYSeries = (Array.isArray(ySeries) ? ySeries : [])
    .map((item) => ({
      source: normalizeText(item?.source || item),
      label: normalizeText(item?.label),
    }))
    .filter((item) => item.source)
  return [
    'import pandas as pd',
    'import numpy as np',
    `__guaxi_plot_var_name = ${JSON.stringify(normalizedVariableName)}`,
    `__guaxi_plot_chart_type = ${JSON.stringify(normalizeText(chartType || 'line'))}`,
    `__guaxi_plot_x = ${JSON.stringify(normalizeText(x))}`,
    `__guaxi_plot_y = ${JSON.stringify(normalizeText(y))}`,
    `__guaxi_plot_y_series = ${JSON.stringify(safeYSeries)}`,
    `__guaxi_plot_z = ${JSON.stringify(normalizeText(z))}`,
    `__guaxi_plot_color = ${JSON.stringify(normalizeText(color))}`,
    `__guaxi_plot_title = ${JSON.stringify(normalizeText(title) || `${normalizedVariableName} 图表`)}`,
    "if __guaxi_plot_var_name not in globals(): raise KeyError(f'Variable not found: {__guaxi_plot_var_name}')",
    '__guaxi_plot_value = globals()[__guaxi_plot_var_name]',
    'if isinstance(__guaxi_plot_value, pd.Series):',
    '    __guaxi_plot_df = __guaxi_plot_value.to_frame(name=__guaxi_plot_value.name or "value").reset_index()',
    'elif isinstance(__guaxi_plot_value, pd.DataFrame):',
    '    __guaxi_plot_df = __guaxi_plot_value.copy()',
    'else:',
    '    try:',
    '        __guaxi_plot_df = pd.DataFrame(__guaxi_plot_value)',
    '    except Exception:',
    '        __guaxi_plot_df = pd.DataFrame({"value": list(__guaxi_plot_value) if hasattr(__guaxi_plot_value, "__iter__") and not isinstance(__guaxi_plot_value, (str, bytes)) else [__guaxi_plot_value]})',
    'def __guaxi_to_series(value, label="value"):',
    '    if isinstance(value, pd.DataFrame):',
    '        if len(value.columns) == 1:',
    '            return value.iloc[:, 0].reset_index(drop=True)',
    '        return value.reset_index(drop=True)',
    '    if isinstance(value, pd.Series):',
    '        return value.reset_index(drop=True)',
    '    if isinstance(value, np.ndarray):',
    '        if value.ndim == 1:',
    '            return pd.Series(value, name=label)',
    '        return pd.DataFrame(value).reset_index(drop=True)',
    '    if isinstance(value, (list, tuple)):',
    '        return pd.Series(list(value), name=label)',
    '    return pd.Series([value], name=label)',
    'def __guaxi_resolve_source(expr, fallback_df=None):',
    '    expr = str(expr or "").strip()',
    '    if not expr:',
    '        return None',
    '    if fallback_df is not None and expr in fallback_df.columns:',
    '        return fallback_df[expr]',
    '    if "." in expr:',
    '        var_name, field = expr.split(".", 1)',
    '        source = globals().get(var_name)',
    '        if source is not None:',
    '            if isinstance(source, pd.DataFrame) and field in source.columns:',
    '                return source[field]',
    '            if isinstance(source, pd.Series):',
    '                if field in ("value", str(source.name or "")):',
    '                    return source',
    '            if isinstance(source, dict) and field in source:',
    '                return __guaxi_to_series(source[field], field)',
    '            try:',
    '                return __guaxi_to_series(getattr(source, field), field)',
    '            except Exception:',
    '                pass',
    '    if expr in globals():',
    '        return __guaxi_to_series(globals()[expr], expr)',
    '    return None',
    'def __guaxi_add_source_column(target_df, expr, column_name):',
    '    source = __guaxi_resolve_source(expr, __guaxi_plot_df)',
    '    if source is None:',
    '        return False',
    '    if isinstance(source, pd.DataFrame):',
    '        for col in source.columns:',
    '            target_df[f"{column_name}.{col}"] = source[col].reset_index(drop=True)',
    '        return True',
    '    target_df[column_name] = pd.Series(source).reset_index(drop=True)',
    '    return True',
    '__guaxi_plot_custom_df = pd.DataFrame(index=range(len(__guaxi_plot_df.index)))',
    'if __guaxi_plot_x:',
    '    __guaxi_add_source_column(__guaxi_plot_custom_df, __guaxi_plot_x, "__x")',
    'if __guaxi_plot_z:',
    '    __guaxi_add_source_column(__guaxi_plot_custom_df, __guaxi_plot_z, "__z")',
    'if __guaxi_plot_color:',
    '    __guaxi_add_source_column(__guaxi_plot_custom_df, __guaxi_plot_color, "__color")',
    'if __guaxi_plot_y_series:',
    '    for __guaxi_idx, __guaxi_spec in enumerate(__guaxi_plot_y_series):',
    '        __guaxi_expr = str(__guaxi_spec.get("source") or "").strip()',
    '        __guaxi_label = str(__guaxi_spec.get("label") or __guaxi_expr or f"series_{__guaxi_idx + 1}").strip()',
    '        __guaxi_add_source_column(__guaxi_plot_custom_df, __guaxi_expr, __guaxi_label)',
    '    if len(__guaxi_plot_custom_df.columns):',
    '        __guaxi_plot_df = __guaxi_plot_custom_df',
    '        if "__x" in __guaxi_plot_df.columns:',
    '            __guaxi_plot_x = "__x"',
    '        __guaxi_plot_y = [c for c in __guaxi_plot_df.columns if c not in ("__x", "__z", "__color")]',
    '        __guaxi_plot_z = "__z" if "__z" in __guaxi_plot_df.columns else ""',
    '        __guaxi_plot_color = "__color" if "__color" in __guaxi_plot_df.columns else ""',
    '__guaxi_plot_cols = [str(c) for c in __guaxi_plot_df.columns]',
    'def __guaxi_pick_col(name, fallback_index=0):',
    '    if isinstance(name, (list, tuple)): return [item for item in name if item in __guaxi_plot_df.columns]',
    '    if name and name in __guaxi_plot_df.columns: return name',
    '    if name and str(name).isdigit() and int(name) < len(__guaxi_plot_df.columns): return __guaxi_plot_df.columns[int(name)]',
    '    return __guaxi_plot_df.columns[min(fallback_index, len(__guaxi_plot_df.columns) - 1)] if len(__guaxi_plot_df.columns) else None',
    '__guaxi_x = __guaxi_pick_col(__guaxi_plot_x, 0)',
    '__guaxi_y = __guaxi_pick_col(__guaxi_plot_y, 1 if len(__guaxi_plot_df.columns) > 1 else 0)',
    'if isinstance(__guaxi_y, list) and not __guaxi_y:',
    '    __guaxi_y = __guaxi_pick_col("", 1 if len(__guaxi_plot_df.columns) > 1 else 0)',
    '__guaxi_z = __guaxi_pick_col(__guaxi_plot_z, 2 if len(__guaxi_plot_df.columns) > 2 else 0)',
    '__guaxi_color = __guaxi_plot_color if __guaxi_plot_color in __guaxi_plot_df.columns else None',
    'if isinstance(__guaxi_y, list) and len(__guaxi_y) > 1:',
    '    __guaxi_color = None',
    'try:',
    '    import plotly.express as px',
    '    import plotly.graph_objects as go',
    '    __guaxi_kind = __guaxi_plot_chart_type',
    '    if __guaxi_kind == "line":',
    '        fig = px.line(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "bar":',
    '        fig = px.bar(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "scatter":',
    '        fig = px.scatter(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "area":',
    '        fig = px.area(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "histogram":',
    '        fig = px.histogram(__guaxi_plot_df, x=__guaxi_x, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "box":',
    '        fig = px.box(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "violin":',
    '        fig = px.violin(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, box=True, points="all", title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "pie":',
    '        fig = px.pie(__guaxi_plot_df, names=__guaxi_x, values=__guaxi_y, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "heatmap":',
    '        fig = px.density_heatmap(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "corr":',
    '        fig = px.imshow(__guaxi_plot_df.select_dtypes(include="number").corr(), text_auto=True, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "density_heatmap":',
    '        fig = px.density_heatmap(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "density_contour":',
    '        fig = px.density_contour(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "scatter_matrix":',
    '        fig = px.scatter_matrix(__guaxi_plot_df.select_dtypes(include="number"), title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "parallel":',
    '        fig = px.parallel_coordinates(__guaxi_plot_df.select_dtypes(include="number"), title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "radar":',
    '        fig = px.line_polar(__guaxi_plot_df, r=__guaxi_y, theta=__guaxi_x, color=__guaxi_color, line_close=True, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "treemap":',
    '        fig = px.treemap(__guaxi_plot_df, path=[__guaxi_x], values=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "sunburst":',
    '        fig = px.sunburst(__guaxi_plot_df, path=[__guaxi_x], values=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "funnel":',
    '        fig = px.funnel(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "ecdf":',
    '        fig = px.ecdf(__guaxi_plot_df, x=__guaxi_x, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "scatter3d":',
    '        fig = px.scatter_3d(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, z=__guaxi_z, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "line3d":',
    '        fig = px.line_3d(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, z=__guaxi_z, color=__guaxi_color, title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "surface3d":',
    '        nums = __guaxi_plot_df.select_dtypes(include="number")',
    '        fig = go.Figure(data=[go.Surface(z=nums.to_numpy())])',
    '        fig.update_layout(title=__guaxi_plot_title)',
    '    elif __guaxi_kind == "candlestick":',
    '        fig = go.Figure(data=[go.Candlestick(x=__guaxi_plot_df[__guaxi_x], open=__guaxi_plot_df.get("open"), high=__guaxi_plot_df.get("high"), low=__guaxi_plot_df.get("low"), close=__guaxi_plot_df.get("close"))])',
    '        fig.update_layout(title=__guaxi_plot_title)',
    '    else:',
    '        fig = px.line(__guaxi_plot_df, x=__guaxi_x, y=__guaxi_y, color=__guaxi_color, title=__guaxi_plot_title)',
    '    _result = fig',
    '    print(f"[GuaxiNotebookLab] plotted var={__guaxi_plot_var_name} chart={__guaxi_kind} x={__guaxi_x} y={__guaxi_y}")',
    'except Exception as __guaxi_plotly_error:',
    '    import matplotlib.pyplot as plt',
    '    __guaxi_kind = __guaxi_plot_chart_type',
    '    ax = __guaxi_plot_df.plot(kind="scatter" if __guaxi_kind == "scatter" else "hist" if __guaxi_kind == "histogram" else "box" if __guaxi_kind == "box" else "bar" if __guaxi_kind == "bar" else "line", x=__guaxi_x if __guaxi_kind not in ("histogram", "box") else None, y=__guaxi_y if __guaxi_kind not in ("histogram", "box") else None, title=__guaxi_plot_title)',
    '    plt.tight_layout()',
    '    _result = None',
    '    print(f"[GuaxiNotebookLab] plotted with matplotlib fallback var={__guaxi_plot_var_name} chart={__guaxi_kind}; plotly_error={__guaxi_plotly_error}")',
  ].join('\n')
}

// buildNotebookLabMathCode 已迁移到独立模块 guaxiNotebookMathRuntime.js（模块化数学工具代码生成器），
// 此处重新导出以保持既有 import 路径不变。新增数学工具请在该模块里扩展对应 section/mode。
export { buildNotebookLabMathCode } from './guaxiNotebookMathRuntime'

export const buildNotebookLabReportCode = ({ title = '探索实验室报告' } = {}) => [
  'import datetime',
  `__guaxi_report_title = ${JSON.stringify(normalizeText(title) || '探索实验室报告')}`,
  '__guaxi_report_lines = [f"# {__guaxi_report_title}", "", f"生成时间：{datetime.datetime.now().isoformat(timespec=\'seconds\')}", ""]',
  '__guaxi_report_lines.append("## 会话变量")',
  '__guaxi_report_lines.append("")',
  'for __guaxi_name in sorted([k for k in globals().keys() if not k.startswith("_") and not k.startswith("__")]):',
  '    __guaxi_value = globals()[__guaxi_name]',
  '    if callable(__guaxi_value) or isinstance(__guaxi_value, type):',
  '        continue',
  '    __guaxi_report_lines.append(f"- `{__guaxi_name}`：`{type(__guaxi_value).__name__}`")',
  '    try:',
  '        if hasattr(__guaxi_value, "shape"):',
  '            __guaxi_report_lines[-1] += f"，shape `{getattr(__guaxi_value, \'shape\')}`"',
  '    except Exception:',
  '        pass',
  '__guaxi_report_lines.append("")',
  '__guaxi_report_lines.append("## 结论记录")',
  '__guaxi_report_lines.append("")',
  "__guaxi_report_lines.append('> 可以继续从图表、变量输出和 Markdown 片段中补充分析结论。')",
  '__guaxi_report = "\\n".join(__guaxi_report_lines)',
  'print("[GuaxiNotebookReport] generated report markdown")',
  '_result = {"__guaxi_display__": {"type": "markdown", "mime": "text/markdown", "markdown": __guaxi_report}}',
].join('\n')

export const buildNotebookLabVariableOperationCode = ({
  action = '',
  variableName = '',
  targetName = '',
  expression = '',
} = {}) => {
  const normalizedAction = normalizeText(action)
  const source = normalizeText(variableName)
  const target = normalizeText(targetName)
  if (!normalizedAction || !source) return ''
  return [
    `__guaxi_var_action = ${JSON.stringify(normalizedAction)}`,
    `__guaxi_var_name = ${JSON.stringify(source)}`,
    `__guaxi_target_name = ${JSON.stringify(target)}`,
    `__guaxi_expression = ${JSON.stringify(String(expression || ''))}`,
    "if __guaxi_var_name not in globals(): raise KeyError(f'Variable not found: {__guaxi_var_name}')",
    'if __guaxi_var_action == "delete":',
    '    del globals()[__guaxi_var_name]',
    '    print(f"[GuaxiNotebookLab] deleted variable {__guaxi_var_name}")',
    'elif __guaxi_var_action == "rename":',
    '    if not __guaxi_target_name: raise ValueError("targetName is required")',
    '    globals()[__guaxi_target_name] = globals().pop(__guaxi_var_name)',
    '    print(f"[GuaxiNotebookLab] renamed variable {__guaxi_var_name} -> {__guaxi_target_name}")',
    'elif __guaxi_var_action == "copy":',
    '    import copy',
    '    if not __guaxi_target_name: raise ValueError("targetName is required")',
    '    globals()[__guaxi_target_name] = copy.deepcopy(globals()[__guaxi_var_name])',
    '    print(f"[GuaxiNotebookLab] copied variable {__guaxi_var_name} -> {__guaxi_target_name}")',
    'elif __guaxi_var_action == "derive":',
    '    if not __guaxi_target_name: raise ValueError("targetName is required")',
    '    value = globals()[__guaxi_var_name]',
    '    globals()[__guaxi_target_name] = eval(__guaxi_expression, globals(), {"value": value}) if __guaxi_expression else value',
    '    print(f"[GuaxiNotebookLab] derived variable {__guaxi_target_name} from {__guaxi_var_name}")',
    'else:',
    '    raise ValueError(f"Unsupported variable action: {__guaxi_var_action}")',
    '_result = None',
  ].join('\n')
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
