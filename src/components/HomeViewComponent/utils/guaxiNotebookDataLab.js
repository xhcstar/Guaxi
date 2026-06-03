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

// ============================================================================
// 统一数据导出抽象（Export Spec）—— 与导入对称：声明式注册表驱动 UI + Python 代码生成。
// 导出直接在工作台会话执行（不经 Notebook），向导确认即落盘/写库。
// ============================================================================

export const EXPORT_CATEGORIES = [
  { id: 'file', label: '文件', icon: 'file', summary: 'CSV / Excel / Parquet / JSON / Markdown / HTML' },
  { id: 'database', label: '数据库', icon: 'database', summary: '写入 SQL 数据表' },
  { id: 'script', label: '自定义脚本', icon: 'code', summary: '私有协议 / 二进制 / 自定义序列化' },
]

const EXPORT_INDEX_FIELD = {
  key: 'options.index',
  label: '行索引',
  type: 'select',
  default: 'false',
  options: [{ value: 'false', label: '不含' }, { value: 'true', label: '包含' }],
}
const EXPORT_ORIENT_OPTIONS = ['records', 'columns', 'index', 'split', 'values', 'table'].map((value) => ({ value, label: value }))
const EXPORT_IF_EXISTS_OPTIONS = [
  { value: 'fail', label: '已存在则报错' },
  { value: 'replace', label: '覆盖重建' },
  { value: 'append', label: '追加' },
]
const exportSaveField = (ext) => ({ key: 'target.path', label: '导出到', type: 'save', ext, required: true })

export const EXPORT_FORMATS = [
  {
    id: 'csv', category: 'file', label: 'CSV', ext: ['csv'],
    fields: [
      exportSaveField(['csv']),
      { key: 'options.encoding', label: '编码', type: 'text', placeholder: '默认 utf-8-sig' },
      { key: 'options.sep', label: '分隔符', type: 'text', placeholder: '默认逗号；\\t 表示制表符' },
      EXPORT_INDEX_FIELD,
    ],
  },
  { id: 'tsv', category: 'file', label: 'TSV', ext: ['tsv'], fields: [exportSaveField(['tsv']), EXPORT_INDEX_FIELD] },
  {
    id: 'excel', category: 'file', label: 'Excel', ext: ['xlsx'],
    fields: [
      exportSaveField(['xlsx']),
      { key: 'options.sheet', label: '工作表名', type: 'text', default: 'Sheet1' },
      EXPORT_INDEX_FIELD,
    ],
  },
  { id: 'parquet', category: 'file', label: 'Parquet', ext: ['parquet'], fields: [exportSaveField(['parquet'])] },
  { id: 'feather', category: 'file', label: 'Feather', ext: ['feather'], fields: [exportSaveField(['feather'])] },
  {
    id: 'json', category: 'file', label: 'JSON', ext: ['json'],
    fields: [
      exportSaveField(['json']),
      { key: 'options.orient', label: '结构(orient)', type: 'select', default: 'records', options: EXPORT_ORIENT_OPTIONS },
      { key: 'options.indent', label: '缩进', type: 'number', default: 2 },
    ],
  },
  { id: 'jsonl', category: 'file', label: 'JSON Lines', ext: ['jsonl'], fields: [exportSaveField(['jsonl'])] },
  { id: 'markdown', category: 'file', label: 'Markdown 表格', ext: ['md'], fields: [exportSaveField(['md']), EXPORT_INDEX_FIELD] },
  { id: 'html', category: 'file', label: 'HTML 表格', ext: ['html'], fields: [exportSaveField(['html']), EXPORT_INDEX_FIELD] },
  {
    id: 'database', category: 'database', label: '数据库表',
    fields: [
      { key: 'target.connStr', label: '连接串', type: 'text', required: true, placeholder: 'sqlite:///路径 或 postgresql+psycopg2://user:pwd@host/db' },
      { key: 'target.table', label: '目标表名', type: 'text', required: true },
      { key: 'options.ifExists', label: '已存在', type: 'select', default: 'fail', options: EXPORT_IF_EXISTS_OPTIONS },
      EXPORT_INDEX_FIELD,
    ],
  },
  {
    id: 'custom', category: 'script', label: '自定义脚本',
    fields: [
      { key: 'target.scriptText', label: '脚本（内联）', type: 'code', placeholder: 'def dump(value, target):\n    # 写出私有 / 二进制格式\n    target.write_bytes(serialize(value))' },
      { key: 'target.scriptPath', label: '或脚本文件', type: 'file', accept: ['py'] },
      { key: 'target.entry', label: '入口函数', type: 'text', default: 'dump' },
      { key: 'target.path', label: '输出路径（可选）', type: 'save', ext: [] },
    ],
  },
]

export const getExportFormatsByCategory = (category) => {
  const id = normalizeText(category)
  return EXPORT_FORMATS.filter((item) => item.category === id)
}

export const getExportFormat = (formatId) => {
  const id = normalizeText(formatId)
  return EXPORT_FORMATS.find((item) => item.id === id) || null
}

export const normalizeExportSpec = (spec = {}) => {
  const input = spec && typeof spec === 'object' ? spec : {}
  const target = input.target && typeof input.target === 'object' ? { ...input.target } : {}
  const options = input.options && typeof input.options === 'object' ? { ...input.options } : {}
  const format = normalizeText(input.format) || 'csv'
  const variableName = normalizeText(input.variableName)
  return { format, variableName, target, options }
}

// 统一导出执行：读取会话变量，按 Export Spec 落盘 / 写库 / 跑自定义脚本；包在函数里避免泄漏。
const EXPORT_RUNTIME_PY = String.raw`
def __guaxi_run_export():
    import json
    spec = __guaxi_export_spec
    fmt = str(spec.get('format') or 'csv')
    var = str(spec.get('variableName') or '')
    tgt = spec.get('target') or {}
    opt = spec.get('options') or {}
    if not var or var not in globals():
        raise KeyError('变量不存在: ' + var)
    value = globals()[var]
    try:
        import pandas as pd
    except Exception:
        pd = None

    def truthy(x, default=False):
        if x is None or x == '':
            return default
        return str(x).strip().lower() in ('1', 'true', 'yes', 't', 'y', '是')

    def as_frame(v):
        if pd is None:
            return None
        if isinstance(v, pd.DataFrame):
            return v
        if isinstance(v, pd.Series):
            return v.to_frame(name=str(v.name or 'value'))
        try:
            return pd.DataFrame(v)
        except Exception:
            return None

    def need_frame():
        df = as_frame(value)
        if df is None:
            raise TypeError('当前变量无法导出为表格，请改用 JSON 或自定义脚本')
        return df

    path = str(tgt.get('path') or '')
    if path:
        from pathlib import Path
        Path(path).parent.mkdir(parents=True, exist_ok=True)
    index = truthy(opt.get('index'), False)
    enc = str(opt.get('encoding') or 'utf-8-sig')
    out = path

    if fmt in ('csv', 'tsv'):
        if not path:
            raise ValueError('请提供导出路径')
        sep = opt.get('sep')
        if not sep:
            sep = '\t' if fmt == 'tsv' else ','
        elif sep == '\\t':
            sep = '\t'
        need_frame().to_csv(path, index=index, sep=sep, encoding=enc)
    elif fmt == 'excel':
        if not path:
            raise ValueError('请提供导出路径')
        need_frame().to_excel(path, index=index, sheet_name=str(opt.get('sheet') or 'Sheet1'))
    elif fmt == 'parquet':
        if not path:
            raise ValueError('请提供导出路径')
        need_frame().to_parquet(path)
    elif fmt == 'feather':
        if not path:
            raise ValueError('请提供导出路径')
        need_frame().reset_index(drop=True).to_feather(path)
    elif fmt == 'json':
        if not path:
            raise ValueError('请提供导出路径')
        try:
            indent = int(opt.get('indent') or 2)
        except Exception:
            indent = 2
        df = as_frame(value)
        if df is not None and not isinstance(value, (dict, list)):
            df.to_json(path, orient=str(opt.get('orient') or 'records'), force_ascii=False, indent=indent)
        else:
            with open(path, 'w', encoding='utf-8') as f:
                json.dump(value, f, ensure_ascii=False, indent=indent, default=str)
    elif fmt == 'jsonl':
        if not path:
            raise ValueError('请提供导出路径')
        need_frame().to_json(path, orient='records', lines=True, force_ascii=False)
    elif fmt == 'markdown':
        if not path:
            raise ValueError('请提供导出路径')
        df = need_frame()
        try:
            text = df.to_markdown(index=index)
        except Exception:
            cols = [str(c) for c in df.columns]
            lines = ['| ' + ' | '.join(cols) + ' |', '| ' + ' | '.join(['---'] * len(cols)) + ' |']
            for _, row in df.iterrows():
                lines.append('| ' + ' | '.join(str(x) for x in row.tolist()) + ' |')
            text = '\n'.join(lines)
        with open(path, 'w', encoding='utf-8') as f:
            f.write(text)
    elif fmt == 'html':
        if not path:
            raise ValueError('请提供导出路径')
        need_frame().to_html(path, index=index)
    elif fmt == 'database':
        conn_str = str(tgt.get('connStr') or '')
        table = str(tgt.get('table') or '')
        if not conn_str or not table:
            raise ValueError('请提供数据库连接串与目标表名')
        from sqlalchemy import create_engine
        eng = create_engine(conn_str)
        try:
            need_frame().to_sql(table, eng, if_exists=str(opt.get('ifExists') or 'fail'), index=index)
        finally:
            eng.dispose()
        out = conn_str + ' :: ' + table
    elif fmt == 'custom':
        code = str(tgt.get('scriptText') or '')
        spath = str(tgt.get('scriptPath') or '')
        if not code.strip() and spath:
            from pathlib import Path
            code = Path(spath).read_text(encoding='utf-8')
        if not code.strip():
            raise ValueError('请提供导出脚本内容或脚本文件')
        entry = str(tgt.get('entry') or 'dump') or 'dump'

        class TargetCtx:
            def __init__(self):
                self.path = path
                self.options = opt
                self.target = tgt

            def write_bytes(self, b):
                from pathlib import Path
                if not self.path:
                    raise ValueError('请提供输出路径')
                Path(self.path).parent.mkdir(parents=True, exist_ok=True)
                Path(self.path).write_bytes(b)
                return self.path

            def write_text(self, t, encoding='utf-8'):
                from pathlib import Path
                if not self.path:
                    raise ValueError('请提供输出路径')
                Path(self.path).parent.mkdir(parents=True, exist_ok=True)
                Path(self.path).write_text(t, encoding=encoding)
                return self.path

        ns = {}
        exec(code, ns)
        fn = ns.get(entry)
        if not callable(fn):
            raise ValueError('脚本中未找到入口函数 ' + entry + '(value, target)')
        ret = fn(value, TargetCtx())
        out = str(ret) if ret else (path or 'custom')
    else:
        raise ValueError('Unsupported export format: ' + fmt)
    return out


__guaxi_export_out = __guaxi_run_export()
print('[GuaxiNotebookLab] exported var=' + str(__guaxi_export_spec.get('variableName')) + ' format=' + str(__guaxi_export_spec.get('format')) + ' target=' + str(__guaxi_export_out))
_result = {'outputPath': __guaxi_export_out, 'format': __guaxi_export_spec.get('format')}
`

const buildExportHeaderPy = (spec) => [
  'import json',
  `__guaxi_export_spec = json.loads(${JSON.stringify(JSON.stringify(normalizeExportSpec(spec)))})`,
].join('\n')

// 生成统一导出代码：按 Export Spec 把会话变量落盘 / 写库 / 跑脚本。
export const buildExportCode = (spec = {}) => `${buildExportHeaderPy(spec)}\n${EXPORT_RUNTIME_PY}`

// ============================================================================
// 关系/规律发现 · 分析工作台（Analysis Spec + 方法注册表）
// ----------------------------------------------------------------------------
// 设计说明：
// 1. 把「手动选坐标轴绘图」升级为「选分析意图 → 选一组特征(高维) + 角色 → 出交互图 + 结论」。
// 2. ANALYSIS_METHODS 声明式注册表同时驱动分析面板 UI 与 Python 代码生成。
// 3. 每个方法产出 plotly 图 + 结论摘要，合成单个 HTML display（复用 worker 的 __guaxi_display__ 管线）。
// 4. 高级方法按需 import（sklearn/statsmodels/scipy），缺库给中文提示并降级，不强制依赖。
// ============================================================================

export const ANALYSIS_GROUPS = [
  { id: 'relation', label: '相关性 / 关系', summary: '相关矩阵、散点矩阵、互信息、滞后相关' },
  { id: 'structure', label: '高维结构', summary: '降维投影、聚类、平行坐标、特征重要性' },
  { id: 'stats', label: '统计规律 / 回归', summary: '分布拟合、回归、假设检验、描述画像' },
  { id: 'causal', label: '因果 / 时间序列', summary: 'Granger 因果、自相关' },
  { id: 'insight', label: '一键洞察', summary: '自动排出最强关系与候选规律' },
  { id: 'chart', label: '基础图表', summary: '手动指定坐标轴的 23 种图表' },
]

const CORR_METHOD_FIELD = {
  key: 'options.corrMethod', label: '相关系数', type: 'select', default: 'pearson',
  options: [
    { value: 'pearson', label: 'Pearson（线性）' },
    { value: 'spearman', label: 'Spearman（秩）' },
    { value: 'kendall', label: 'Kendall' },
  ],
}
const N_COMPONENTS_FIELD = {
  key: 'options.nComponents', label: '投影维度', type: 'select', default: '2',
  options: [{ value: '2', label: '2D' }, { value: '3', label: '3D' }],
}

// needs：声明该方法需要的角色，驱动 UI 显示哪些角色输入；
// minFeatures：特征集最少列数；lib：依赖库（缺失时运行时给中文提示）。
export const ANALYSIS_METHODS = [
  // —— 相关性 / 关系 ——
  {
    id: 'corr_matrix', group: 'relation', label: '相关矩阵热力图', lib: 'base', minFeatures: 2,
    needs: ['features'], fields: [CORR_METHOD_FIELD],
    desc: '计算特征两两相关系数并以热力图呈现，附最强相关对排名。',
  },
  {
    id: 'scatter_matrix', group: 'relation', label: '散点矩阵 SPLOM', lib: 'base', minFeatures: 2,
    needs: ['features', 'color'], fields: [],
    desc: '特征两两散点 + 对角分布，可按颜色分组观察聚集与关系。',
  },
  {
    id: 'mutual_info', group: 'relation', label: '互信息（非线性关联）', lib: 'sklearn', minFeatures: 1,
    needs: ['features', 'target'], fields: [],
    desc: '衡量每个特征与目标的非线性依赖强度，捕捉相关系数漏掉的关系。',
  },
  {
    id: 'lag_corr', group: 'relation', label: '滞后互相关', lib: 'base', minFeatures: 2,
    needs: ['features', 'timeIndex'], fields: [{ key: 'options.maxLag', label: '最大滞后', type: 'number', default: 10 }],
    desc: '在不同时间滞后下计算两序列相关，找领先/滞后关系。',
  },
  // —— 高维结构 ——
  {
    id: 'pca', group: 'structure', label: 'PCA 主成分投影', lib: 'sklearn', minFeatures: 3,
    needs: ['features', 'color'], fields: [N_COMPONENTS_FIELD],
    desc: '线性降维到 2D/3D 散点，附各主成分方差解释率。',
  },
  {
    id: 'tsne', group: 'structure', label: 't-SNE 流形投影', lib: 'sklearn', minFeatures: 3,
    needs: ['features', 'color'], fields: [N_COMPONENTS_FIELD, { key: 'options.perplexity', label: 'perplexity', type: 'number', default: 30 }],
    desc: '非线性降维，擅长显示局部聚集结构（计算较慢）。',
  },
  {
    id: 'cluster_kmeans', group: 'structure', label: 'KMeans 聚类', lib: 'sklearn', minFeatures: 2,
    needs: ['features'], fields: [{ key: 'options.k', label: '簇数 k', type: 'number', default: 3 }],
    desc: '无监督聚类并用 PCA 投影着色展示，附轮廓系数评估。',
  },
  {
    id: 'parallel', group: 'structure', label: '平行坐标', lib: 'base', minFeatures: 2,
    needs: ['features', 'color'], fields: [],
    desc: '每条折线一个样本，跨多维观察趋势与分组差异。',
  },
  {
    id: 'feature_importance', group: 'structure', label: '特征重要性', lib: 'sklearn', minFeatures: 1,
    needs: ['features', 'target'], fields: [],
    desc: '随机森林评估各特征对目标的预测贡献并排名。',
  },
  // —— 统计规律 / 回归 ——
  {
    id: 'dist_fit', group: 'stats', label: '分布拟合 / QQ', lib: 'scipy', minFeatures: 1,
    needs: ['features'], fields: [{
      key: 'options.dist', label: '分布', type: 'select', default: 'norm',
      options: ['norm', 'lognorm', 'expon', 'gamma', 'uniform'].map((v) => ({ value: v, label: v })),
    }],
    desc: '直方图 + 拟合 PDF + QQ 图，并给拟合优度（KS 检验）。',
  },
  {
    id: 'regression', group: 'stats', label: '回归拟合', lib: 'base', minFeatures: 1,
    needs: ['features', 'target'], fields: [{ key: 'options.polyDegree', label: '多项式次数', type: 'number', default: 1 }],
    desc: '对目标做 OLS / 多项式回归，给 R²、系数与残差图。',
  },
  {
    id: 'hypothesis', group: 'stats', label: '假设检验', lib: 'scipy', minFeatures: 1,
    needs: ['features', 'group'], fields: [{
      key: 'options.test', label: '检验', type: 'select', default: 'ttest',
      options: [
        { value: 'ttest', label: '独立样本 t 检验' },
        { value: 'anova', label: '单因素 ANOVA' },
        { value: 'chi2', label: '卡方独立性' },
      ],
    }],
    desc: '按分组比较特征是否有显著差异，给统计量与 p 值。',
  },
  {
    id: 'profile', group: 'stats', label: '描述性画像', lib: 'base', minFeatures: 1,
    needs: ['features'], fields: [],
    desc: '每个特征的均值/分位/偏度/峰度/缺失率/唯一值，快速体检。',
  },
  // —— 因果 / 时间序列 ——
  {
    id: 'granger', group: 'causal', label: 'Granger 因果矩阵', lib: 'statsmodels', minFeatures: 2,
    needs: ['features', 'timeIndex'], fields: [{ key: 'options.maxLag', label: '最大滞后', type: 'number', default: 5 }],
    desc: '检验一个序列是否有助于预测另一个（格兰杰因果），输出 p 值矩阵。',
  },
  {
    id: 'acf', group: 'causal', label: '自相关 ACF / PACF', lib: 'statsmodels', minFeatures: 1,
    needs: ['features', 'timeIndex'], fields: [{ key: 'options.maxLag', label: '最大滞后', type: 'number', default: 20 }],
    desc: '序列与自身滞后的相关，识别周期性与记忆长度。',
  },
  // —— 一键洞察 ——
  {
    id: 'auto_insight', group: 'insight', label: '自动洞察', lib: 'base', minFeatures: 2,
    needs: ['features'], fields: [{ key: 'options.topN', label: '关系数', type: 'number', default: 10 }],
    desc: '自动跑相关矩阵 + 排出最强关系、标常量/高缺失列、给候选规律排名。',
  },
]

export const getAnalysisMethodsByGroup = (group) => {
  const id = normalizeText(group)
  return ANALYSIS_METHODS.filter((item) => item.group === id)
}

export const getAnalysisMethod = (methodId) => {
  const id = normalizeText(methodId)
  return ANALYSIS_METHODS.find((item) => item.id === id) || null
}

export const normalizeAnalysisSpec = (spec = {}) => {
  const input = spec && typeof spec === 'object' ? spec : {}
  const features = (Array.isArray(input.features) ? input.features : [])
    .map((item) => normalizeText(item?.source || item))
    .filter(Boolean)
  return {
    method: normalizeText(input.method) || 'corr_matrix',
    features,
    target: normalizeText(input.target),
    group: normalizeText(input.group),
    color: normalizeText(input.color),
    timeIndex: normalizeText(input.timeIndex),
    primaryVariable: normalizeText(input.primaryVariable || input.variableName),
    title: normalizeText(input.title),
    options: input.options && typeof input.options === 'object' ? { ...input.options } : {},
  }
}

// 共享前导：跨变量解析 + 按角色构造对齐后的特征矩阵 __guaxi_an_df（沿用绘图器的 resolve 思路）。
const ANALYSIS_PRELUDE_PY = String.raw`
import json
import numpy as np
import pandas as pd

def __guaxi_an_to_series(value, label='value'):
    if isinstance(value, pd.DataFrame):
        return value.iloc[:, 0].reset_index(drop=True) if len(value.columns) == 1 else value.reset_index(drop=True)
    if isinstance(value, pd.Series):
        return value.reset_index(drop=True)
    if isinstance(value, np.ndarray):
        return pd.Series(value, name=label) if value.ndim == 1 else pd.DataFrame(value).reset_index(drop=True)
    if isinstance(value, (list, tuple)):
        return pd.Series(list(value), name=label)
    return pd.Series([value], name=label)

def __guaxi_an_primary_df():
    name = __guaxi_an_spec.get('primaryVariable') or ''
    if name and name in globals():
        v = globals()[name]
        if isinstance(v, pd.DataFrame):
            return v.copy()
        if isinstance(v, pd.Series):
            return v.to_frame(name=str(v.name or 'value'))
        try:
            return pd.DataFrame(v)
        except Exception:
            return None
    return None

def __guaxi_an_resolve(expr):
    expr = str(expr or '').strip()
    if not expr:
        return None
    fb = __guaxi_an_fallback
    if fb is not None and expr in fb.columns:
        return fb[expr].reset_index(drop=True)
    if '.' in expr:
        var_name, field = expr.split('.', 1)
        src = globals().get(var_name)
        if src is not None:
            if isinstance(src, pd.DataFrame) and field in src.columns:
                return src[field].reset_index(drop=True)
            if isinstance(src, pd.Series) and field in ('value', str(src.name or '')):
                return src.reset_index(drop=True)
            if isinstance(src, dict) and field in src:
                return __guaxi_an_to_series(src[field], field)
            try:
                return __guaxi_an_to_series(getattr(src, field), field)
            except Exception:
                pass
    if expr in globals():
        return __guaxi_an_to_series(globals()[expr], expr)
    if fb is not None and expr in fb.columns:
        return fb[expr].reset_index(drop=True)
    return None

__guaxi_an_fallback = __guaxi_an_primary_df()

def __guaxi_an_build_features():
    cols = {}
    for expr in (__guaxi_an_spec.get('features') or []):
        s = __guaxi_an_resolve(expr)
        if s is not None:
            cols[str(expr)] = pd.Series(s).reset_index(drop=True)
    if not cols and __guaxi_an_fallback is not None:
        return __guaxi_an_fallback.select_dtypes(include='number').reset_index(drop=True)
    return pd.DataFrame(cols)

def __guaxi_an_role(role):
    expr = __guaxi_an_spec.get(role) or ''
    s = __guaxi_an_resolve(expr)
    return pd.Series(s).reset_index(drop=True) if s is not None else None

def __guaxi_an_numeric(df):
    return df.apply(pd.to_numeric, errors='coerce')

def __guaxi_an_opt(key, default=None):
    v = (__guaxi_an_spec.get('options') or {}).get(key)
    return default if v in (None, '') else v

def __guaxi_an_int(key, default):
    try:
        return int(float(__guaxi_an_opt(key, default)))
    except Exception:
        return default

def __guaxi_an_title(fallback):
    return str(__guaxi_an_spec.get('title') or fallback)

def __guaxi_an_html(fig, summary_html=''):
    try:
        import plotly.io as pio
        fig_html = pio.to_html(fig, include_plotlyjs=True, full_html=False, config={'responsive': True})
        return {'__guaxi_display__': {'type': 'html', 'mime': 'text/html', 'html': summary_html + fig_html}}
    except Exception:
        return fig

def __guaxi_an_table_html(title, rows, headers):
    th = ''.join('<th style="text-align:left;padding:4px 10px;border-bottom:1px solid #ddd">' + str(h) + '</th>' for h in headers)
    trs = ''
    for r in rows:
        tds = ''.join('<td style="padding:4px 10px;border-bottom:1px solid #eee">' + str(c) + '</td>' for c in r)
        trs += '<tr>' + tds + '</tr>'
    return ('<div style="font-family:system-ui,Microsoft YaHei,sans-serif;margin:0 0 10px">'
            '<div style="font-weight:700;margin:4px 0 6px">' + str(title) + '</div>'
            '<table style="border-collapse:collapse;font-size:13px">'
            '<thead><tr>' + th + '</tr></thead><tbody>' + trs + '</tbody></table></div>')

def __guaxi_an_need(mod, label):
    try:
        return __import__(mod)
    except Exception:
        raise RuntimeError('该分析方法需要 ' + label + '，当前 Python 运行时未安装。请在运行时安装后重试。')
`

// 各分析方法主体：按 method 分发，产出 _result（HTML display 或 fig）。
const ANALYSIS_RUNTIME_PY = String.raw`
def __guaxi_run_analysis():
    import plotly.express as px
    import plotly.graph_objects as go
    method = str(__guaxi_an_spec.get('method') or 'corr_matrix')
    df = __guaxi_an_build_features()
    if df is None or df.shape[1] == 0:
        raise ValueError('没有可用的特征列，请先选择特征或确认变量可表格化')
    title = __guaxi_an_title(method)

    if method == 'corr_matrix':
        num = __guaxi_an_numeric(df).dropna(how='all', axis=1)
        cm = str(__guaxi_an_opt('corrMethod', 'pearson'))
        corr = num.corr(method=cm)
        fig = px.imshow(corr, text_auto='.2f', color_continuous_scale='RdBu', zmin=-1, zmax=1, title=title + '（' + cm + '）')
        pairs = []
        cols = list(corr.columns)
        for i in range(len(cols)):
            for j in range(i + 1, len(cols)):
                v = corr.iloc[i, j]
                if pd.notna(v):
                    pairs.append((cols[i], cols[j], float(v)))
        pairs.sort(key=lambda t: abs(t[2]), reverse=True)
        rows = [(a, b, round(v, 4)) for a, b, v in pairs[:10]]
        summary = __guaxi_an_table_html('最强相关对', rows, ['特征 A', '特征 B', '相关系数'])
        return __guaxi_an_html(fig, summary)

    if method == 'scatter_matrix':
        num = __guaxi_an_numeric(df)
        color = __guaxi_an_role('color')
        plot_df = num.copy()
        color_col = None
        if color is not None:
            plot_df['__color'] = color.values
            color_col = '__color'
        fig = px.scatter_matrix(plot_df, dimensions=list(num.columns), color=color_col, title=title)
        fig.update_traces(diagonal_visible=False, showupperhalf=False)
        return __guaxi_an_html(fig)

    if method == 'mutual_info':
        __guaxi_an_need('sklearn', 'scikit-learn')
        from sklearn.feature_selection import mutual_info_regression, mutual_info_classif
        target = __guaxi_an_role('target')
        if target is None:
            raise ValueError('互信息需要选择目标(target)')
        X = __guaxi_an_numeric(df).fillna(0.0)
        y = pd.Series(target).reset_index(drop=True)
        idx = X.index[:len(y)]
        X = X.loc[idx]
        y = y.iloc[:len(X)]
        if y.dtype == object or y.nunique() <= max(2, int(len(y) * 0.05)):
            yc = y.astype('category').cat.codes
            mi = mutual_info_classif(X, yc, random_state=0)
        else:
            mi = mutual_info_regression(X, pd.to_numeric(y, errors='coerce').fillna(0.0), random_state=0)
        order = np.argsort(mi)[::-1]
        cols = [list(X.columns)[i] for i in order]
        vals = [float(mi[i]) for i in order]
        fig = px.bar(x=vals, y=cols, orientation='h', title=title + '（互信息）', labels={'x': '互信息', 'y': '特征'})
        fig.update_yaxes(autorange='reversed')
        rows = [(c, round(v, 4)) for c, v in zip(cols, vals)]
        return __guaxi_an_html(fig, __guaxi_an_table_html('互信息排名', rows, ['特征', '互信息']))

    if method == 'lag_corr':
        num = __guaxi_an_numeric(df)
        if num.shape[1] < 2:
            raise ValueError('滞后互相关需要至少 2 个特征')
        a = num.iloc[:, 0]
        b = num.iloc[:, 1]
        max_lag = __guaxi_an_int('maxLag', 10)
        lags = list(range(-max_lag, max_lag + 1))
        vals = []
        for lag in lags:
            vals.append(float(a.corr(b.shift(lag))))
        fig = px.bar(x=lags, y=vals, title=title + '：' + str(num.columns[0]) + ' vs ' + str(num.columns[1]),
                     labels={'x': '滞后(b 相对 a)', 'y': '相关系数'})
        best = max(range(len(lags)), key=lambda i: abs(vals[i]) if not np.isnan(vals[i]) else -1)
        summary = __guaxi_an_table_html('最强滞后', [(lags[best], round(vals[best], 4))], ['滞后', '相关系数'])
        return __guaxi_an_html(fig, summary)

    if method in ('pca', 'tsne'):
        __guaxi_an_need('sklearn', 'scikit-learn')
        from sklearn.preprocessing import StandardScaler
        X = __guaxi_an_numeric(df).dropna()
        if X.shape[1] < 2:
            raise ValueError('降维需要至少 2 个数值特征')
        Xs = StandardScaler().fit_transform(X)
        n = 3 if str(__guaxi_an_opt('nComponents', '2')) == '3' else 2
        color = __guaxi_an_role('color')
        cser = pd.Series(color).reset_index(drop=True).loc[X.index] if color is not None else None
        summary = ''
        if method == 'pca':
            from sklearn.decomposition import PCA
            comp = PCA(n_components=n).fit(Xs)
            emb = comp.transform(Xs)
            ev = comp.explained_variance_ratio_
            summary = __guaxi_an_table_html('方差解释率',
                [('PC' + str(i + 1), round(float(ev[i]), 4)) for i in range(n)] + [('累计', round(float(ev.sum()), 4))],
                ['主成分', '解释率'])
        else:
            from sklearn.manifold import TSNE
            perp = min(__guaxi_an_int('perplexity', 30), max(5, len(Xs) - 1))
            emb = TSNE(n_components=n, perplexity=perp, init='pca', random_state=0).fit_transform(Xs)
        axis = ['维度1', '维度2', '维度3'][:n]
        edf = pd.DataFrame(emb, columns=axis)
        if cser is not None:
            edf['分组'] = cser.values
        cc = '分组' if cser is not None else None
        if n == 3:
            fig = px.scatter_3d(edf, x=axis[0], y=axis[1], z=axis[2], color=cc, title=title)
        else:
            fig = px.scatter(edf, x=axis[0], y=axis[1], color=cc, title=title)
        return __guaxi_an_html(fig, summary)

    if method == 'cluster_kmeans':
        __guaxi_an_need('sklearn', 'scikit-learn')
        from sklearn.preprocessing import StandardScaler
        from sklearn.cluster import KMeans
        from sklearn.decomposition import PCA
        from sklearn.metrics import silhouette_score
        X = __guaxi_an_numeric(df).dropna()
        k = max(2, __guaxi_an_int('k', 3))
        Xs = StandardScaler().fit_transform(X)
        labels = KMeans(n_clusters=k, n_init=10, random_state=0).fit_predict(Xs)
        try:
            sil = silhouette_score(Xs, labels)
        except Exception:
            sil = float('nan')
        emb = PCA(n_components=2).fit_transform(Xs) if Xs.shape[1] > 2 else Xs
        edf = pd.DataFrame(emb[:, :2], columns=['维度1', '维度2'])
        edf['簇'] = [str(l) for l in labels]
        fig = px.scatter(edf, x='维度1', y='维度2', color='簇', title=title + '（k=' + str(k) + '）')
        rows = [('簇数', k), ('轮廓系数', round(float(sil), 4) if sil == sil else 'N/A')]
        return __guaxi_an_html(fig, __guaxi_an_table_html('聚类评估', rows, ['指标', '值']))

    if method == 'parallel':
        num = __guaxi_an_numeric(df)
        color = __guaxi_an_role('color')
        plot_df = num.copy()
        cc = None
        if color is not None:
            cseries = pd.Series(color).reset_index(drop=True)
            if cseries.dtype == object:
                cseries = cseries.astype('category').cat.codes
            plot_df['__color'] = pd.to_numeric(cseries, errors='coerce').values
            cc = '__color'
        fig = px.parallel_coordinates(plot_df, dimensions=list(num.columns), color=cc, title=title)
        return __guaxi_an_html(fig)

    if method == 'feature_importance':
        __guaxi_an_need('sklearn', 'scikit-learn')
        from sklearn.ensemble import RandomForestRegressor, RandomForestClassifier
        target = __guaxi_an_role('target')
        if target is None:
            raise ValueError('特征重要性需要选择目标(target)')
        X = __guaxi_an_numeric(df).fillna(0.0)
        y = pd.Series(target).reset_index(drop=True).iloc[:len(X)]
        X = X.iloc[:len(y)]
        if y.dtype == object or y.nunique() <= max(2, int(len(y) * 0.05)):
            model = RandomForestClassifier(n_estimators=200, random_state=0).fit(X, y.astype('category').cat.codes)
        else:
            model = RandomForestRegressor(n_estimators=200, random_state=0).fit(X, pd.to_numeric(y, errors='coerce').fillna(0.0))
        imp = model.feature_importances_
        order = np.argsort(imp)[::-1]
        cols = [list(X.columns)[i] for i in order]
        vals = [float(imp[i]) for i in order]
        fig = px.bar(x=vals, y=cols, orientation='h', title=title + '（随机森林）', labels={'x': '重要性', 'y': '特征'})
        fig.update_yaxes(autorange='reversed')
        rows = [(c, round(v, 4)) for c, v in zip(cols, vals)]
        return __guaxi_an_html(fig, __guaxi_an_table_html('特征重要性', rows, ['特征', '重要性']))

    if method == 'dist_fit':
        __guaxi_an_need('scipy', 'scipy')
        from scipy import stats as st
        num = __guaxi_an_numeric(df)
        s = num.iloc[:, 0].dropna()
        dname = str(__guaxi_an_opt('dist', 'norm'))
        dist = getattr(st, dname)
        params = dist.fit(s)
        ks = st.kstest(s, dname, args=params)
        xs = np.linspace(float(s.min()), float(s.max()), 200)
        pdf = dist.pdf(xs, *params)
        fig = go.Figure()
        fig.add_trace(go.Histogram(x=s, histnorm='probability density', name='样本', opacity=0.6))
        fig.add_trace(go.Scatter(x=xs, y=pdf, name=dname + ' 拟合', mode='lines'))
        fig.update_layout(title=title + '：' + str(num.columns[0]) + ' ~ ' + dname)
        rows = [('KS 统计量', round(float(ks.statistic), 4)), ('p 值', round(float(ks.pvalue), 4)),
                ('结论', '不能拒绝该分布' if ks.pvalue > 0.05 else '与该分布有显著差异')]
        return __guaxi_an_html(fig, __guaxi_an_table_html('拟合优度（KS 检验）', rows, ['指标', '值']))

    if method == 'regression':
        target = __guaxi_an_role('target')
        if target is None:
            raise ValueError('回归需要选择目标(target)')
        num = __guaxi_an_numeric(df)
        x = num.iloc[:, 0]
        y = pd.to_numeric(pd.Series(target).reset_index(drop=True), errors='coerce')
        m = pd.concat([x.rename('x'), y.rename('y')], axis=1).dropna()
        deg = max(1, __guaxi_an_int('polyDegree', 1))
        coef = np.polyfit(m['x'], m['y'], deg)
        pred = np.polyval(coef, m['x'])
        ss_res = float(np.sum((m['y'] - pred) ** 2))
        ss_tot = float(np.sum((m['y'] - m['y'].mean()) ** 2))
        r2 = 1 - ss_res / ss_tot if ss_tot else float('nan')
        order = np.argsort(m['x'].values)
        fig = go.Figure()
        fig.add_trace(go.Scatter(x=m['x'], y=m['y'], mode='markers', name='样本'))
        fig.add_trace(go.Scatter(x=m['x'].values[order], y=pred.values[order] if hasattr(pred, 'values') else np.asarray(pred)[order], mode='lines', name='拟合'))
        fig.update_layout(title=title + '：' + str(num.columns[0]) + ' → target（' + str(deg) + ' 次）')
        crows = [('R²', round(float(r2), 4))] + [('系数 x^' + str(deg - i), round(float(c), 4)) for i, c in enumerate(coef)]
        return __guaxi_an_html(fig, __guaxi_an_table_html('回归结果', crows, ['项', '值']))

    if method == 'hypothesis':
        __guaxi_an_need('scipy', 'scipy')
        from scipy import stats as st
        group = __guaxi_an_role('group')
        if group is None:
            raise ValueError('假设检验需要选择分组(group)')
        num = __guaxi_an_numeric(df)
        col = num.iloc[:, 0]
        g = pd.Series(group).reset_index(drop=True).iloc[:len(col)]
        test = str(__guaxi_an_opt('test', 'ttest'))
        groups = [col[g == lvl].dropna() for lvl in pd.unique(g.dropna())]
        if test == 'chi2':
            ct = pd.crosstab(col, g)
            chi2, p, dof, _ = st.chi2_contingency(ct)
            stat, name = chi2, '卡方'
            fig = px.imshow(ct, text_auto=True, title=title + '：列联表')
        elif test == 'anova':
            stat, p = st.f_oneway(*[grp for grp in groups if len(grp) > 1])
            name = 'F'
            fig = px.box(pd.DataFrame({'值': col, '分组': g}), x='分组', y='值', title=title + '：ANOVA')
        else:
            stat, p = st.ttest_ind(groups[0], groups[1], equal_var=False)
            name = 't'
            fig = px.box(pd.DataFrame({'值': col, '分组': g}), x='分组', y='值', title=title + '：t 检验')
        rows = [(name + ' 统计量', round(float(stat), 4)), ('p 值', round(float(p), 4)),
                ('结论', '存在显著差异' if p < 0.05 else '无显著差异')]
        return __guaxi_an_html(fig, __guaxi_an_table_html('假设检验', rows, ['指标', '值']))

    if method == 'profile':
        num = __guaxi_an_numeric(df)
        rows = []
        for c in df.columns:
            s = num[c] if c in num.columns else pd.to_numeric(df[c], errors='coerce')
            rows.append((c, round(float(s.mean()), 3) if s.notna().any() else 'N/A',
                         round(float(s.std()), 3) if s.notna().any() else 'N/A',
                         round(float(s.skew()), 3) if s.notna().sum() > 2 else 'N/A',
                         round(float(s.kurt()), 3) if s.notna().sum() > 3 else 'N/A',
                         round(float(df[c].isna().mean()), 3), int(df[c].nunique())))
        miss = pd.DataFrame({'特征': list(df.columns), '缺失率': [float(df[c].isna().mean()) for c in df.columns]})
        fig = px.bar(miss, x='特征', y='缺失率', title=title + '：缺失率')
        summary = __guaxi_an_table_html('描述性画像', rows, ['特征', '均值', '标准差', '偏度', '峰度', '缺失率', '唯一值'])
        return __guaxi_an_html(fig, summary)

    if method == 'granger':
        __guaxi_an_need('statsmodels', 'statsmodels')
        from statsmodels.tsa.stattools import grangercausalitytests
        num = __guaxi_an_numeric(df).dropna()
        cols = list(num.columns)
        if len(cols) < 2:
            raise ValueError('Granger 因果需要至少 2 个序列')
        max_lag = __guaxi_an_int('maxLag', 5)
        mat = pd.DataFrame(np.ones((len(cols), len(cols))), index=cols, columns=cols)
        for i, ci in enumerate(cols):
            for j, cj in enumerate(cols):
                if i == j:
                    mat.iloc[i, j] = np.nan
                    continue
                try:
                    res = grangercausalitytests(num[[ci, cj]], maxlag=max_lag, verbose=False)
                    pmin = min(res[l][0]['ssr_ftest'][1] for l in res)
                    mat.iloc[i, j] = float(pmin)
                except Exception:
                    mat.iloc[i, j] = np.nan
        fig = px.imshow(mat, text_auto='.3f', color_continuous_scale='Blues_r', title=title + '：行→列 因果 p 值（越小越显著）')
        sig = []
        for i, ci in enumerate(cols):
            for j, cj in enumerate(cols):
                v = mat.iloc[i, j]
                if i != j and pd.notna(v) and v < 0.05:
                    sig.append((ci + ' → ' + cj, round(float(v), 4)))
        sig.sort(key=lambda t: t[1])
        return __guaxi_an_html(fig, __guaxi_an_table_html('显著因果关系（p<0.05）', sig[:10] or [('无', '-')], ['关系', 'p 值']))

    if method == 'acf':
        __guaxi_an_need('statsmodels', 'statsmodels')
        from statsmodels.tsa.stattools import acf, pacf
        num = __guaxi_an_numeric(df)
        s = num.iloc[:, 0].dropna()
        max_lag = min(__guaxi_an_int('maxLag', 20), max(1, len(s) // 2 - 1))
        a = acf(s, nlags=max_lag)
        try:
            p = pacf(s, nlags=max_lag)
        except Exception:
            p = a
        lags = list(range(len(a)))
        fig = go.Figure()
        fig.add_trace(go.Bar(x=lags, y=a, name='ACF'))
        fig.add_trace(go.Bar(x=list(range(len(p))), y=p, name='PACF'))
        fig.update_layout(title=title + '：' + str(num.columns[0]) + ' 自相关', barmode='group')
        return __guaxi_an_html(fig)

    if method == 'auto_insight':
        num = __guaxi_an_numeric(df).dropna(how='all', axis=1)
        corr = num.corr()
        cols = list(corr.columns)
        pairs = []
        for i in range(len(cols)):
            for j in range(i + 1, len(cols)):
                v = corr.iloc[i, j]
                if pd.notna(v):
                    pairs.append((cols[i], cols[j], float(v)))
        pairs.sort(key=lambda t: abs(t[2]), reverse=True)
        topn = __guaxi_an_int('topN', 10)
        const_cols = [c for c in df.columns if df[c].nunique(dropna=True) <= 1]
        miss_cols = [(c, round(float(df[c].isna().mean()), 3)) for c in df.columns if df[c].isna().mean() > 0.3]
        fig = px.imshow(corr, text_auto='.2f', color_continuous_scale='RdBu', zmin=-1, zmax=1, title=__guaxi_an_title('自动洞察：相关概览'))
        parts = [__guaxi_an_table_html('最强关系排名', [(a, b, round(v, 4)) for a, b, v in pairs[:topn]] or [('无', '-', '-')], ['特征 A', '特征 B', '相关系数'])]
        if const_cols:
            parts.append(__guaxi_an_table_html('常量列（无信息）', [(c,) for c in const_cols], ['特征']))
        if miss_cols:
            parts.append(__guaxi_an_table_html('高缺失列（>30%）', miss_cols, ['特征', '缺失率']))
        return __guaxi_an_html(fig, ''.join(parts))

    raise ValueError('Unsupported analysis method: ' + method)


_result = __guaxi_run_analysis()
print('[GuaxiNotebookLab] analyzed method=' + str(__guaxi_an_spec.get('method')) + ' features=' + str(len(__guaxi_an_spec.get('features') or [])))
`

const buildAnalysisHeaderPy = (spec) => [
  'import json',
  `__guaxi_an_spec = json.loads(${JSON.stringify(JSON.stringify(normalizeAnalysisSpec(spec)))})`,
].join('\n')

// 生成分析代码：按 Analysis Spec 构造特征矩阵并按方法产出交互图 + 结论摘要。
export const buildAnalysisCode = (spec = {}) => [
  buildAnalysisHeaderPy(spec),
  ANALYSIS_PRELUDE_PY,
  ANALYSIS_RUNTIME_PY,
].join('\n')

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
