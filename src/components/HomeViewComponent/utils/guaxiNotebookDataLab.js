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
  // 设计说明：导入记录现在承载完整 Dataset Descriptor（source/format/schema/options/spec），
  // 既兼容旧的 sourceType/sourceLabel/dataKind 扁平字段，也支持后续“重新导入”与结构分析。
  const normalizedRecord = record && typeof record === 'object'
    ? {
      variableName: normalizeText(record.variableName || record.name),
      sourceType: normalizeText(record.sourceType || record?.source?.category || 'unknown'),
      sourceLabel: normalizeText(record.sourceLabel || record?.source?.label || ''),
      dataKind: normalizeText(record.dataKind || 'value'),
      importedAt: normalizeText(record.importedAt || new Date().toISOString()),
      ...(record.source && typeof record.source === 'object' ? { source: { ...record.source } } : {}),
      ...(record.schema && typeof record.schema === 'object' ? { schema: { ...record.schema } } : {}),
      ...(record.options && typeof record.options === 'object' ? { options: { ...record.options } } : {}),
      ...(record.spec && typeof record.spec === 'object' ? { spec: { ...record.spec } } : {}),
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

// ============================================================================
// 统一数据导入抽象层（Import Spec + Dataset Descriptor）
// ----------------------------------------------------------------------------
// 设计说明：
// 1. 所有导入方式（文件 / 数据库 / API / 剪贴板 / 自定义脚本）都被抽象成一份可
//    序列化的 Import Spec，描述“从哪取数 + 怎么解析”；它既能走 RPC，又能存档复跑。
// 2. IMPORT_FORMATS 是声明式注册表，同时驱动导入向导的动态表单与 Python 代码生成，
//    新增格式 = 往注册表加一条 + 在运行时加一个分支。
// 3. 真正解析逻辑放在 Python 运行时（IMPORT_RUNTIME_PY / IMPORT_PROBE_RUNTIME_PY），
//    整体包在 `__guaxi_*` 前缀的函数里，局部变量不会泄漏成会话变量。
// ============================================================================

export const IMPORT_CATEGORIES = [
  { id: 'file', label: '文件', icon: 'file', summary: 'CSV / Excel / Parquet / JSON / 文本' },
  { id: 'database', label: '数据库', icon: 'database', summary: 'SQLite / Postgres / MySQL / DuckDB' },
  { id: 'api', label: 'API & URL', icon: 'globe', summary: 'HTTP 接口 / 远程文件' },
  { id: 'clipboard', label: '剪贴板', icon: 'clipboard', summary: '表格 / JSON / 文本' },
  { id: 'script', label: '自定义脚本', icon: 'code', summary: '私有协议 / 二进制 / 混合文本' },
]

const HEADER_FIELD_OPTIONS = [
  { value: 'infer', label: '第一行作表头' },
  { value: 'none', label: '无表头' },
]
const HTTP_METHOD_OPTIONS = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'].map((value) => ({ value, label: value }))

// 每个格式的 fields 用点号路径（source.* / options.*）声明，向导据此读写嵌套 Spec。
// type: text | textarea | number | select | file | code | kv | sheet | table | clipboard
export const IMPORT_FORMATS = [
  {
    id: 'csv', category: 'file', label: 'CSV / 分隔文本', needsProbe: true,
    accept: ['csv', 'tsv', 'txt'],
    fields: [
      { key: 'source.path', label: '数据文件', type: 'file', accept: ['csv', 'tsv', 'txt'], required: true },
      { key: 'options.encoding', label: '编码', type: 'text', placeholder: '留空自动（utf-8 / gbk …）' },
      { key: 'options.sep', label: '分隔符', type: 'text', placeholder: '留空自动；\\t 表示制表符' },
      { key: 'options.header', label: '表头', type: 'select', default: 'infer', options: HEADER_FIELD_OPTIONS },
    ],
  },
  {
    id: 'excel', category: 'file', label: 'Excel 工作簿', needsProbe: true,
    accept: ['xlsx', 'xls'],
    fields: [
      { key: 'source.path', label: '数据文件', type: 'file', accept: ['xlsx', 'xls'], required: true },
      { key: 'options.sheet', label: '工作表', type: 'sheet', placeholder: '点“探测”后选择，留空取第一张' },
    ],
  },
  {
    id: 'parquet', category: 'file', label: 'Parquet / Feather',
    accept: ['parquet', 'feather'],
    fields: [
      { key: 'source.path', label: '数据文件', type: 'file', accept: ['parquet', 'feather'], required: true },
    ],
  },
  {
    id: 'json', category: 'file', label: 'JSON / JSONL', needsProbe: true,
    accept: ['json', 'jsonl'],
    fields: [
      { key: 'source.path', label: '数据文件', type: 'file', accept: ['json', 'jsonl'], required: true },
      { key: 'options.jsonPath', label: 'JSON 路径', type: 'text', placeholder: '例如 data.items（留空取根）' },
    ],
  },
  {
    id: 'database', category: 'database', label: '数据库连接', needsProbe: true,
    fields: [
      { key: 'source.connStr', label: '连接串', type: 'text', placeholder: 'sqlite:///路径 或 postgresql+psycopg2://user:pwd@host/db' },
      { key: 'source.path', label: '或 SQLite/DuckDB 文件', type: 'file', accept: ['db', 'sqlite', 'sqlite3', 'duckdb'] },
      { key: 'source.table', label: '表名', type: 'table', placeholder: '点“探测”后选择' },
      { key: 'source.query', label: '自定义 SQL', type: 'textarea', placeholder: '留空则 select * from 选定表' },
    ],
  },
  {
    id: 'api-json', category: 'api', label: 'API（JSON 响应）', needsProbe: true,
    fields: [
      { key: 'source.url', label: 'URL', type: 'text', required: true, placeholder: 'https://example.com/api/data' },
      { key: 'source.method', label: '方法', type: 'select', default: 'GET', options: HTTP_METHOD_OPTIONS },
      { key: 'source.headers', label: '请求头', type: 'kv' },
      { key: 'source.auth', label: 'Token', type: 'text', placeholder: '可选，自动加 Authorization: Bearer …' },
      { key: 'source.body', label: '请求体', type: 'textarea', placeholder: 'POST/PUT 时的 JSON 文本' },
      { key: 'source.jsonPath', label: 'JSON 路径', type: 'text', placeholder: '例如 data.items（留空取根）' },
    ],
  },
  {
    id: 'api-csv', category: 'api', label: '远程 CSV',
    fields: [
      { key: 'source.url', label: 'URL', type: 'text', required: true },
      { key: 'source.method', label: '方法', type: 'select', default: 'GET', options: HTTP_METHOD_OPTIONS },
      { key: 'source.headers', label: '请求头', type: 'kv' },
      { key: 'source.auth', label: 'Token', type: 'text', placeholder: '可选' },
    ],
  },
  {
    id: 'api-text', category: 'api', label: '远程文本 / 原始响应',
    fields: [
      { key: 'source.url', label: 'URL', type: 'text', required: true },
      { key: 'source.method', label: '方法', type: 'select', default: 'GET', options: HTTP_METHOD_OPTIONS },
      { key: 'source.headers', label: '请求头', type: 'kv' },
      { key: 'source.auth', label: 'Token', type: 'text', placeholder: '可选' },
    ],
  },
  {
    id: 'logline', category: 'api', label: '日志（正则逐行解析）', needsProbe: true,
    fields: [
      { key: 'source.path', label: '日志文件', type: 'file', accept: ['log', 'txt'] },
      { key: 'source.text', label: '或粘贴日志', type: 'textarea', placeholder: '直接粘贴日志文本', clipboard: true },
      { key: 'options.logPattern', label: '解析正则', type: 'text', required: true, placeholder: '带命名分组，如 (?P<ts>\\S+) (?P<level>\\w+) (?P<msg>.*)' },
    ],
  },
  {
    id: 'clipboard', category: 'clipboard', label: '剪贴板内容', needsProbe: true,
    fields: [
      { key: 'source.text', label: '内容', type: 'textarea', placeholder: '粘贴表格（Excel/CSV）、JSON 或文本', clipboard: true, required: true },
    ],
  },
  {
    id: 'custom', category: 'script', label: '自定义脚本', needsProbe: true,
    fields: [
      { key: 'source.scriptText', label: '脚本（内联）', type: 'code', placeholder: 'def load(source):\n    raw = source.read_bytes()\n    # 解析私有协议 / 二进制 / 混合文本\n    return value' },
      { key: 'source.scriptPath', label: '或脚本文件', type: 'file', accept: ['py'] },
      { key: 'source.entry', label: '入口函数', type: 'text', default: 'load' },
      { key: 'source.path', label: '数据文件 → source.path', type: 'file' },
      { key: 'source.url', label: '数据 URL → source.url', type: 'text' },
    ],
  },
]

export const getImportFormatsByCategory = (category) => {
  const id = normalizeText(category)
  return IMPORT_FORMATS.filter((item) => item.category === id)
}

export const getImportFormat = (formatId) => {
  const id = normalizeText(formatId)
  return IMPORT_FORMATS.find((item) => item.id === id) || null
}

const DEFAULT_VAR_BY_CATEGORY = {
  file: 'data_item',
  database: 'db_table',
  api: 'api_data',
  clipboard: 'clipboard_data',
  script: 'custom_data',
}

const deriveImportLabel = (source = {}, category = '') => {
  const path = normalizeText(source?.path)
  if (path) return path.split(/[\\/]/).pop() || path
  const url = normalizeText(source?.url)
  if (url) return url
  const connStr = normalizeText(source?.connStr)
  if (connStr) return connStr
  return normalizeText(category) || 'data'
}

// 把任意输入规整成标准 Import Spec，保证 category/format/variableName/sourceLabel 齐全，
// source/options 为对象。format 缺省留 'auto'，由 Python 运行时按扩展名/内容推断。
export const normalizeImportSpec = (spec = {}) => {
  const input = spec && typeof spec === 'object' ? spec : {}
  const source = input.source && typeof input.source === 'object' ? { ...input.source } : {}
  const options = input.options && typeof input.options === 'object' ? { ...input.options } : {}
  const category = normalizeText(input.category) || 'file'
  const format = normalizeText(input.format) || 'auto'
  const sourceLabel = normalizeText(input.sourceLabel) || deriveImportLabel(source, category)
  const fallbackVar = DEFAULT_VAR_BY_CATEGORY[category] || 'data_item'
  const variableName = clampVariableName(
    input.variableName || buildImportVariableName(sourceLabel, fallbackVar),
    fallbackVar,
  )
  return { category, format, variableName, sourceLabel, source, options }
}

// 只读探测：列 Excel sheet / DB 表 / 嗅探 CSV 编码分隔符 / 预览 JSON·日志·脚本结果。
// 绝不写入 globals()，结果以 JSON 返回。
const IMPORT_PROBE_RUNTIME_PY = String.raw`
def __guaxi_probe():
    import json, io
    spec = __guaxi_spec
    cat = str(spec.get('category') or 'file')
    fmt = str(spec.get('format') or '')
    src = spec.get('source') or {}
    opt = spec.get('options') or {}
    N = 20
    try:
        import pandas as pd
    except Exception:
        pd = None
    out = {'category': cat, 'format': fmt, 'ok': True}

    def http():
        import urllib.request
        url = str(src.get('url') or '')
        if not url:
            raise ValueError('请提供请求 URL')
        method = str(src.get('method') or 'GET').upper()
        headers = src.get('headers') or {}
        body = src.get('body')
        auth = src.get('auth')
        data = None
        if body not in (None, ''):
            data = json.dumps(body).encode('utf-8') if isinstance(body, (dict, list)) else str(body).encode('utf-8')
        req = urllib.request.Request(url, data=data, method=method)
        if isinstance(headers, dict):
            for hk, hv in headers.items():
                if str(hk).strip():
                    req.add_header(str(hk), str(hv))
        if auth not in (None, ''):
            token = str(auth)
            req.add_header('Authorization', token if token.lower().startswith(('bearer ', 'basic ')) else 'Bearer ' + token)
        with urllib.request.urlopen(req) as resp:
            return resp.read()

    def jpath(obj, path):
        import re
        path = str(path or '').strip()
        if not path:
            return obj
        cur = obj
        for tok in re.findall(r'[^.\[\]]+', path):
            if cur is None:
                break
            if isinstance(cur, dict):
                cur = cur.get(tok)
            elif isinstance(cur, (list, tuple)):
                try:
                    cur = cur[int(tok)]
                except Exception:
                    cur = None
            else:
                cur = None
        return cur

    try:
        if fmt == 'excel':
            xl = pd.ExcelFile(str(src.get('path') or ''))
            out['sheets'] = [str(s) for s in xl.sheet_names]
        elif cat == 'database' or fmt in ('database', 'sqlite', 'sql'):
            conn_str = str(src.get('connStr') or '').strip()
            path = str(src.get('path') or '').strip()
            tables = []
            if conn_str:
                from sqlalchemy import create_engine, inspect
                eng = create_engine(conn_str)
                try:
                    insp = inspect(eng)
                    for t in insp.get_table_names():
                        try:
                            cols = [str(c['name']) for c in insp.get_columns(t)]
                        except Exception:
                            cols = []
                        tables.append({'name': str(t), 'columns': cols})
                finally:
                    eng.dispose()
            elif path:
                import sqlite3
                conn = sqlite3.connect(path)
                try:
                    rows = conn.execute("select name from sqlite_master where type='table' order by name").fetchall()
                    for (name,) in rows:
                        try:
                            info = conn.execute('PRAGMA table_info("' + str(name) + '")').fetchall()
                            cols = [str(c[1]) for c in info]
                        except Exception:
                            cols = []
                        tables.append({'name': str(name), 'columns': cols})
                finally:
                    conn.close()
            else:
                raise ValueError('请提供连接串或 SQLite 文件')
            out['tables'] = tables
        elif fmt in ('csv', 'tsv'):
            from pathlib import Path
            path = str(src.get('path') or '')
            raw = Path(path).read_bytes()[:65536] if path else str(src.get('text') or '').encode('utf-8')
            enc = str(opt.get('encoding') or '')
            tried = [enc] if enc else ['utf-8', 'gbk', 'latin-1']
            text = None
            used = None
            for e in tried:
                try:
                    text = raw.decode(e)
                    used = e
                    break
                except Exception:
                    continue
            if text is None:
                text = raw.decode('utf-8', errors='replace')
                used = 'utf-8'
            sep = opt.get('sep')
            if not sep:
                if fmt == 'tsv':
                    sep = '\t'
                else:
                    import csv as _csv
                    try:
                        sep = _csv.Sniffer().sniff(text[:4096], delimiters=',;\t|').delimiter
                    except Exception:
                        sep = ','
            elif sep == '\\t':
                sep = '\t'
            df = pd.read_csv(io.StringIO(text), sep=sep, nrows=N)
            out['encoding'] = used
            out['sep'] = '\\t' if sep == '\t' else sep
            out['columns'] = [str(c) for c in df.columns]
            out['sample'] = json.loads(df.head(N).to_json(orient='records', force_ascii=False))
        elif fmt in ('json', 'jsonl') or fmt.startswith('api') or cat in ('api', 'clipboard'):
            if cat == 'api' or fmt.startswith('api'):
                raw_text = http().decode(str(opt.get('encoding') or 'utf-8'), errors='replace')
            elif src.get('path'):
                from pathlib import Path
                raw_text = Path(str(src.get('path'))).read_text(encoding=str(opt.get('encoding') or 'utf-8'))
            else:
                raw_text = str(src.get('text') or '')
            data = None
            if fmt == 'jsonl':
                data = [json.loads(ln) for ln in raw_text.splitlines() if ln.strip()]
            else:
                try:
                    data = json.loads(raw_text)
                except Exception:
                    out['topType'] = 'text'
                    out['preview'] = raw_text[:2000]
                    return out
            data = jpath(data, src.get('jsonPath') or opt.get('jsonPath'))
            out['topType'] = type(data).__name__
            if isinstance(data, dict):
                out['keys'] = [str(k) for k in list(data.keys())[:100]]
            if isinstance(data, (list, dict)):
                try:
                    out['length'] = len(data)
                except Exception:
                    pass
            if pd is not None:
                try:
                    df = pd.DataFrame(data)
                    if len(df.columns):
                        out['columns'] = [str(c) for c in df.columns]
                        out['sample'] = json.loads(df.head(N).to_json(orient='records', force_ascii=False))
                except Exception:
                    pass
            out['preview'] = json.dumps(data, ensure_ascii=False, indent=2, default=str)[:2000]
        elif fmt == 'logline':
            import re
            pattern = str(opt.get('logPattern') or '')
            if not pattern:
                raise ValueError('请提供日志解析正则 logPattern')
            rx = re.compile(pattern)
            path = str(src.get('path') or '')
            if path:
                from pathlib import Path
                lines = Path(path).read_text(encoding=str(opt.get('encoding') or 'utf-8'), errors='replace').splitlines()
            else:
                lines = str(src.get('text') or '').splitlines()
            rows = []
            matched = 0
            for ln in lines:
                m = rx.search(ln)
                if not m:
                    continue
                matched += 1
                gd = m.groupdict()
                rows.append(gd if gd else {('g%d' % (i + 1)): g for i, g in enumerate(m.groups())})
                if len(rows) >= N:
                    pass
            out['total'] = len(lines)
            out['matched'] = matched
            out['rows'] = rows[:N]
            if rows:
                out['columns'] = [str(c) for c in rows[0].keys()]
        elif cat == 'script' or fmt == 'custom':
            value, kind = __guaxi_eval_custom(src, opt, http)
            out['kind'] = kind
            if pd is not None and isinstance(value, (pd.DataFrame, pd.Series)):
                df = value.to_frame() if isinstance(value, pd.Series) else value
                out['columns'] = [str(c) for c in df.columns]
                out['sample'] = json.loads(df.head(N).to_json(orient='records', force_ascii=False))
                out['length'] = int(len(df.index))
            else:
                out['preview'] = json.dumps(value, ensure_ascii=False, indent=2, default=str)[:2000] if isinstance(value, (dict, list)) else str(value)[:2000]
        else:
            out['note'] = '该格式无需探测，可直接导入'
    except Exception as e:
        out['ok'] = False
        out['error'] = type(e).__name__ + ': ' + str(e)
    return out


def __guaxi_eval_custom(src, opt, http):
    code = str(src.get('scriptText') or '')
    spath = str(src.get('scriptPath') or '')
    if not code.strip() and spath:
        from pathlib import Path
        code = Path(spath).read_text(encoding='utf-8')
    if not code.strip():
        raise ValueError('请提供脚本内容或脚本文件')
    entry = str(src.get('entry') or 'load') or 'load'

    class SourceCtx:
        def __init__(self):
            self.path = str(src.get('path') or '')
            self.url = str(src.get('url') or '')
            self.text = str(src.get('text') or '')
            self.options = opt
            self.source = src

        def read_bytes(self):
            from pathlib import Path
            if self.path:
                return Path(self.path).read_bytes()
            if self.url:
                return http()
            return self.text.encode('utf-8')

        def read_text(self, encoding='utf-8'):
            if self.text:
                return self.text
            return self.read_bytes().decode(encoding, errors='replace')

    ns = {}
    exec(code, ns)
    fn = ns.get(entry)
    if not callable(fn):
        raise ValueError('脚本中未找到入口函数 ' + entry + '(source)')
    value = fn(SourceCtx())
    try:
        import pandas as pd
    except Exception:
        pd = None
    if pd is not None and isinstance(value, (pd.DataFrame, pd.Series)):
        kind = 'table'
    elif isinstance(value, str):
        kind = 'text'
    elif isinstance(value, (dict, list, tuple, set)):
        kind = 'structured'
    else:
        kind = 'value'
    return value, kind


_result = __guaxi_probe()
`

// 统一导入执行：按 format 分发解析，写入会话变量并打印日志；整体包在函数里避免局部泄漏。
const IMPORT_RUNTIME_PY = String.raw`
def __guaxi_run_import():
    import json, io
    spec = __guaxi_spec
    cat = str(spec.get('category') or 'file')
    fmt = str(spec.get('format') or '')
    src = spec.get('source') or {}
    opt = spec.get('options') or {}
    try:
        import pandas as pd
    except Exception:
        pd = None

    def need_pd():
        if pd is None:
            raise RuntimeError('当前 Python 环境缺少 pandas，无法解析表格数据')

    def infer_kind(v):
        if pd is not None and isinstance(v, (pd.DataFrame, pd.Series)):
            return 'table'
        if isinstance(v, str):
            return 'text'
        if isinstance(v, (dict, list, tuple, set)):
            return 'structured'
        return 'value'

    def to_table(v):
        if pd is not None and isinstance(v, (pd.DataFrame, pd.Series)):
            return v, 'table'
        if pd is not None:
            try:
                df = pd.DataFrame(v)
                if len(df.columns):
                    return df, 'table'
            except Exception:
                pass
        return v, infer_kind(v)

    def jpath(obj, path):
        import re
        path = str(path or '').strip()
        if not path:
            return obj
        cur = obj
        for tok in re.findall(r'[^.\[\]]+', path):
            if cur is None:
                break
            if isinstance(cur, dict):
                cur = cur.get(tok)
            elif isinstance(cur, (list, tuple)):
                try:
                    cur = cur[int(tok)]
                except Exception:
                    cur = None
            else:
                cur = None
        return cur

    def http():
        import urllib.request
        url = str(src.get('url') or '')
        if not url:
            raise ValueError('请提供请求 URL')
        method = str(src.get('method') or 'GET').upper()
        headers = src.get('headers') or {}
        body = src.get('body')
        auth = src.get('auth')
        data = None
        if body not in (None, ''):
            data = json.dumps(body).encode('utf-8') if isinstance(body, (dict, list)) else str(body).encode('utf-8')
        req = urllib.request.Request(url, data=data, method=method)
        if isinstance(headers, dict):
            for hk, hv in headers.items():
                if str(hk).strip():
                    req.add_header(str(hk), str(hv))
        if auth not in (None, ''):
            token = str(auth)
            req.add_header('Authorization', token if token.lower().startswith(('bearer ', 'basic ')) else 'Bearer ' + token)
        with urllib.request.urlopen(req) as resp:
            return resp.read()

    def read_text(path):
        from pathlib import Path
        return Path(path).read_text(encoding=str(opt.get('encoding') or 'utf-8'))

    class SourceCtx:
        def __init__(self):
            self.path = str(src.get('path') or '')
            self.url = str(src.get('url') or '')
            self.text = str(src.get('text') or '')
            self.options = opt
            self.source = src

        def read_bytes(self):
            from pathlib import Path
            if self.path:
                return Path(self.path).read_bytes()
            if self.url:
                return http()
            return self.text.encode('utf-8')

        def read_text(self, encoding='utf-8'):
            if self.text:
                return self.text
            return self.read_bytes().decode(encoding, errors='replace')

    path = str(src.get('path') or '')

    if fmt in ('', 'auto'):
        target = path or str(src.get('url') or '')
        head = target.split('?')[0].split('#')[0].lower()
        ext = head.rsplit('.', 1)[-1] if '.' in head else ''
        mapping = {
            'csv': 'csv', 'tsv': 'tsv', 'txt': 'text',
            'xlsx': 'excel', 'xls': 'excel',
            'parquet': 'parquet', 'feather': 'feather',
            'json': 'json', 'jsonl': 'jsonl',
            'db': 'database', 'sqlite': 'database', 'sqlite3': 'database', 'duckdb': 'database',
            'md': 'text', 'markdown': 'text', 'yaml': 'text', 'yml': 'text',
            'log': 'text', 'py': 'text', 'js': 'text', 'ts': 'text',
        }
        fmt = mapping.get(ext, '')
        if not fmt:
            if src.get('text') not in (None, ''):
                fmt = 'clipboard'
            elif src.get('url'):
                fmt = 'api-text'
            else:
                fmt = 'text'

    if fmt in ('csv', 'tsv'):
        need_pd()
        sep = opt.get('sep')
        if not sep:
            sep = '\t' if fmt == 'tsv' else ','
        elif sep == '\\t':
            sep = '\t'
        enc = str(opt.get('encoding') or 'utf-8')
        header = 0 if str(opt.get('header') or 'infer') != 'none' else None
        if path:
            return pd.read_csv(path, sep=sep, encoding=enc, header=header), 'table'
        return pd.read_csv(io.StringIO(str(src.get('text') or '')), sep=sep, header=header), 'table'

    if fmt == 'excel':
        need_pd()
        sheet = opt.get('sheet')
        if sheet in (None, ''):
            sheet = 0
        elif str(sheet).isdigit():
            sheet = int(sheet)
        return pd.read_excel(path, sheet_name=sheet), 'table'

    if fmt == 'parquet':
        need_pd()
        return pd.read_parquet(path), 'table'

    if fmt == 'feather':
        need_pd()
        return pd.read_feather(path), 'table'

    if fmt == 'jsonl':
        need_pd()
        if path:
            return pd.read_json(path, lines=True), 'table'
        return pd.read_json(io.StringIO(str(src.get('text') or '')), lines=True), 'table'

    if fmt == 'json':
        raw = read_text(path) if path else str(src.get('text') or '')
        data = jpath(json.loads(raw), opt.get('jsonPath') or src.get('jsonPath'))
        value, kind = to_table(data)
        if kind != 'table':
            return data, 'structured'
        return value, kind

    if cat == 'database' or fmt in ('database', 'sqlite', 'sql'):
        need_pd()
        conn_str = str(src.get('connStr') or '').strip()
        db_path = str(src.get('path') or '').strip()
        table = str(src.get('table') or '').strip()
        query = str(src.get('query') or '').strip()
        if conn_str:
            from sqlalchemy import create_engine
            eng = create_engine(conn_str)
            try:
                if not query:
                    if not table:
                        from sqlalchemy import inspect
                        names = inspect(eng).get_table_names()
                        if not names:
                            raise ValueError('数据库中没有可用的表')
                        table = names[0]
                    query = 'select * from "' + table + '"'
                return pd.read_sql(query, eng), 'table'
            finally:
                eng.dispose()
        if db_path:
            import sqlite3
            conn = sqlite3.connect(db_path)
            try:
                if not query:
                    if not table:
                        rows = conn.execute("select name from sqlite_master where type='table' order by name").fetchall()
                        if not rows:
                            raise ValueError('数据库文件中没有可用的表')
                        table = str(rows[0][0])
                    query = 'select * from "' + table + '"'
                return pd.read_sql_query(query, conn), 'table'
            finally:
                conn.close()
        raise ValueError('请提供数据库连接串或 SQLite 文件路径')

    if fmt.startswith('api'):
        raw = http()
        enc = str(opt.get('encoding') or 'utf-8')
        if fmt == 'api-csv':
            need_pd()
            return pd.read_csv(io.StringIO(raw.decode(enc, errors='replace'))), 'table'
        if fmt == 'api-text':
            return raw.decode(enc, errors='replace'), 'text'
        data = jpath(json.loads(raw.decode(enc, errors='replace')), src.get('jsonPath') or opt.get('jsonPath'))
        value, kind = to_table(data)
        if kind != 'table':
            return data, 'structured'
        return value, kind

    if fmt == 'logline':
        need_pd()
        import re
        pattern = str(opt.get('logPattern') or '')
        if not pattern:
            raise ValueError('请提供日志解析正则 logPattern')
        rx = re.compile(pattern)
        if path:
            lines = read_text(path).splitlines()
        else:
            lines = str(src.get('text') or '').splitlines()
        rows = []
        for ln in lines:
            m = rx.search(ln)
            if not m:
                continue
            gd = m.groupdict()
            rows.append(gd if gd else {('g%d' % (i + 1)): g for i, g in enumerate(m.groups())})
        return pd.DataFrame(rows), 'table'

    if cat == 'clipboard' or fmt == 'clipboard':
        text = str(src.get('text') or '')
        try:
            data = json.loads(text)
            value, kind = to_table(data)
            return (value, kind) if kind == 'table' else (data, 'structured')
        except Exception:
            pass
        if pd is not None and '\n' in text.strip():
            try:
                first = text.strip().splitlines()[0]
                sep = '\t' if '\t' in first else (';' if ';' in first and ',' not in first else ',')
                df = pd.read_csv(io.StringIO(text), sep=sep)
                if len(df.columns) > 1 or len(df.index) > 0:
                    return df, 'table'
            except Exception:
                pass
        return text, 'text'

    if cat == 'script' or fmt == 'custom':
        code = str(src.get('scriptText') or '')
        spath = str(src.get('scriptPath') or '')
        if not code.strip() and spath:
            from pathlib import Path
            code = Path(spath).read_text(encoding='utf-8')
        if not code.strip():
            raise ValueError('请提供脚本内容或脚本文件')
        entry = str(src.get('entry') or 'load') or 'load'
        ns = {}
        exec(code, ns)
        fn = ns.get(entry)
        if not callable(fn):
            raise ValueError('脚本中未找到入口函数 ' + entry + '(source)')
        value = fn(SourceCtx())
        return value, infer_kind(value)

    if fmt == 'text':
        if path:
            return read_text(path), 'text'
        if src.get('url'):
            return http().decode(str(opt.get('encoding') or 'utf-8'), errors='replace'), 'text'
        return str(src.get('text') or ''), 'text'

    raise ValueError('Unsupported import format: ' + (fmt or '(empty)'))


__guaxi_value, __guaxi_kind = __guaxi_run_import()
globals()[__guaxi_spec['variableName']] = __guaxi_value
print('[GuaxiNotebookLab] imported source=' + str(__guaxi_spec.get('sourceLabel')) + ' var=' + str(__guaxi_spec['variableName']) + ' kind=' + str(__guaxi_kind))
_result = None
`

const buildSpecHeaderPy = (spec) => {
  const normalized = normalizeImportSpec(spec)
  return [
    'import json',
    `__guaxi_spec = json.loads(${JSON.stringify(JSON.stringify(normalized))})`,
  ].join('\n')
}

// 生成只读探测代码：列 sheet/表、嗅探 CSV、预览 JSON/日志/脚本结果。
export const buildImportProbeCode = (spec = {}) => `${buildSpecHeaderPy(spec)}\n${IMPORT_PROBE_RUNTIME_PY}`

// 生成统一导入代码：按 Import Spec 解析并写入会话变量。
export const buildImportCode = (spec = {}) => `${buildSpecHeaderPy(spec)}\n${IMPORT_RUNTIME_PY}`

export const buildNotebookLabImportPlan = ({
  sourceType = 'file',
  sourcePath = '',
  sourceLabel = '',
  variableName = '',
  text = '',
  sourceUrl = '',
} = {}) => {
  const normalizedSourceType = normalizeText(sourceType || 'file').toLowerCase()
  const categoryByType = { file: 'file', url: 'api', clipboard: 'clipboard', inline: 'clipboard' }
  const category = categoryByType[normalizedSourceType] || 'clipboard'
  const source = {}
  if (category === 'file') source.path = sourcePath
  else if (category === 'api') source.url = sourceUrl
  else source.text = text
  const spec = normalizeImportSpec({
    category,
    format: category === 'clipboard' ? 'clipboard' : 'auto',
    variableName,
    sourceLabel: sourceLabel || sourcePath || sourceUrl || normalizedSourceType,
    source,
  })
  return {
    sourceType: normalizedSourceType || 'inline',
    sourceLabel: spec.sourceLabel,
    variableName: spec.variableName,
    code: buildImportCode(spec),
    spec,
  }
}

// 直接从完整 Import Spec 构造导入计划（供导入向导/RPC 使用）。
export const buildNotebookLabImportPlanFromSpec = (spec = {}) => {
  const normalized = normalizeImportSpec(spec)
  return {
    sourceType: normalized.category,
    sourceLabel: normalized.sourceLabel,
    variableName: normalized.variableName,
    code: buildImportCode(normalized),
    spec: normalized,
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
