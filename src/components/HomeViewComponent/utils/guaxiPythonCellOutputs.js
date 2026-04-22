const DISPLAY_OUTPUT_KINDS = new Set(['html', 'svg', 'markdown', 'image', 'audio', 'video']);

const createOutputId = () => `pyout-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;

const normalizeText = (value) => String(value == null ? '' : value).trim();

const normalizeTablePayload = (table = {}) => {
  if (!table || typeof table !== 'object') return null;
  const columns = Array.isArray(table.columns) ? table.columns.map((item) => String(item ?? '')) : [];
  const rows = Array.isArray(table.rows) ? table.rows.map((row) => (Array.isArray(row) ? [...row] : [])) : [];
  const rowCount = Number(table.rowCount ?? table.row_count ?? rows.length ?? 0);
  const resultHandle = normalizeText(table.resultHandle ?? table.result_handle ?? table.handle ?? '');
  const manifestPath = normalizeText(table.manifestPath ?? table.manifest_path ?? '');
  const artifactPath = normalizeText(table.artifactPath ?? table.artifact_path ?? '');
  const tableKey = normalizeText(table.tableKey ?? table.table_key ?? '');
  const tableName = normalizeText(table.tableName ?? table.table_name ?? '');
  const tableKind = normalizeText(table.tableKind ?? table.table_kind ?? 'table');
  const tableCount = Number(table.tableCount ?? table.table_count ?? 1);
  const remote = !!table.remote || (!!resultHandle && rows.length === 0);
  if (!columns.length && !rows.length && !rowCount && !resultHandle) return null;
  return {
    columns,
    rows,
    rowCount,
    truncated: !!table.truncated,
    dtypes: table.dtypes && typeof table.dtypes === 'object' ? { ...table.dtypes } : {},
    sourcePath: normalizeText(table.sourcePath ?? table.source_path ?? ''),
    resultHandle,
    manifestPath,
    artifactPath,
    tableKey,
    tableName,
    tableKind,
    tableCount,
    remote,
  };
};

export const normalizePythonCellOutputDisplayType = (display) => {
  const explicitType = normalizeText(display?.type).toLowerCase();
  const mime = normalizeText(display?.mime).toLowerCase();
  if (explicitType) return explicitType;
  if (mime.startsWith('image/')) return 'image';
  if (mime.startsWith('audio/')) return 'audio';
  if (mime.startsWith('video/')) return 'video';
  if (mime.includes('markdown')) return 'markdown';
  if (mime.includes('html')) return 'html';
  if (mime.includes('svg')) return 'svg';
  return '';
};

export const getPythonCellOutputDisplaySourcePath = (display) => normalizeText(
  display?.path ||
  display?.file_path ||
  display?.src ||
  display?.source_path ||
  display?.url ||
  '',
);

export const getPythonCellOutputDisplayDataUrl = (display) => {
  const directDataUrl = normalizeText(display?.dataUrl || display?.data_url || '');
  if (directDataUrl.startsWith('data:')) return directDataUrl;
  const base64 = normalizeText(display?.data_base64 || display?.base64 || '');
  if (!base64) return '';
  return `data:${normalizeText(display?.mime || 'application/octet-stream')};base64,${base64}`;
};

export const isPythonCellDisplayOutputKind = (kind) => DISPLAY_OUTPUT_KINDS.has(normalizeText(kind).toLowerCase());

const buildDefaultOutputTitle = (kind, source = 'execution', table = null) => {
  const normalizedKind = normalizeText(kind).toLowerCase();
  const normalizedSource = normalizeText(source).toLowerCase();

  if (normalizedKind === 'table') {
    const tableName = normalizeText(table?.tableName ?? table?.table_name ?? '');
    if (tableName) return tableName;
    return normalizedSource === 'import-table' ? '导入表格' : '表格输出';
  }
  if (normalizedKind === 'text') {
    return normalizedSource === 'import-table' ? '导入说明' : '文本输出';
  }
  if (normalizedKind === 'html') return 'HTML 输出';
  if (normalizedKind === 'svg') return 'SVG 输出';
  if (normalizedKind === 'markdown') return 'Markdown 输出';
  if (normalizedKind === 'image') return '图像输出';
  if (normalizedKind === 'audio') return '音频输出';
  if (normalizedKind === 'video') return '视频输出';
  return '输出结果';
};

const buildDefaultOutputSummary = (kind, { table = null, display = null, text = '' } = {}) => {
  const normalizedKind = normalizeText(kind).toLowerCase();
  if (normalizedKind === 'table' && table) {
    const summaryParts = [`${Number(table.rowCount || 0)} 行`, `${Array.isArray(table.columns) ? table.columns.length : 0} 列`];
    if (table.tableCount > 1) summaryParts.push(`共 ${Number(table.tableCount || 1)} 个表`);
    if (table.sourcePath) summaryParts.push(String(table.sourcePath).split(/[\\/]/).pop() || table.sourcePath);
    if (table.remote) summaryParts.push('按需加载');
    return summaryParts.join(' · ');
  }
  if (normalizedKind === 'text') {
    const lineCount = Math.max(1, normalizeText(text).split(/\r\n?|\n/).length);
    return `${lineCount} 行文本`;
  }
  if (normalizedKind === 'markdown') {
    return `${Math.max(1, normalizeText(display?.markdown).split(/\r\n?|\n/).length)} 行 Markdown`;
  }
  if (normalizedKind === 'html' || normalizedKind === 'svg') {
    return normalizeText(display?.mime || '');
  }
  if (['image', 'audio', 'video'].includes(normalizedKind)) {
    return normalizeText(display?.mime || getPythonCellOutputDisplaySourcePath(display) || normalizedKind);
  }
  return '';
};

export const createPythonCellOutputModule = ({
  id = '',
  kind = '',
  source = 'execution',
  title = '',
  summary = '',
  collapsed = false,
  text = '',
  table = null,
  display = null,
  createdAt = '',
} = {}) => {
  const normalizedKind = normalizeText(kind || normalizePythonCellOutputDisplayType(display)).toLowerCase();
  if (!normalizedKind) return null;

  if (normalizedKind === 'text') {
    const normalizedText = String(text == null ? '' : text);
    if (!normalizeText(normalizedText)) return null;
    return {
      id: normalizeText(id) || createOutputId(),
      kind: 'text',
      source: normalizeText(source) || 'execution',
      title: normalizeText(title) || buildDefaultOutputTitle('text', source),
      summary: normalizeText(summary) || buildDefaultOutputSummary('text', { text: normalizedText }),
      collapsed: !!collapsed,
      createdAt: normalizeText(createdAt) || new Date().toISOString(),
      text: normalizedText,
      table: null,
      display: null,
    };
  }

  if (normalizedKind === 'table') {
    const normalizedTable = normalizeTablePayload(table);
    if (!normalizedTable) return null;
    return {
      id: normalizeText(id) || createOutputId(),
      kind: 'table',
      source: normalizeText(source) || 'execution',
      title: normalizeText(title) || buildDefaultOutputTitle('table', source, normalizedTable),
      summary: normalizeText(summary) || buildDefaultOutputSummary('table', { table: normalizedTable }),
      collapsed: !!collapsed,
      createdAt: normalizeText(createdAt) || new Date().toISOString(),
      text: '',
      table: normalizedTable,
      display: null,
    };
  }

  if (!isPythonCellDisplayOutputKind(normalizedKind)) return null;
  const normalizedDisplay = display && typeof display === 'object' ? { ...display, type: normalizedKind } : null;
  if (!normalizedDisplay) return null;

  return {
    id: normalizeText(id) || createOutputId(),
    kind: normalizedKind,
    source: normalizeText(source) || 'execution',
    title: normalizeText(title) || buildDefaultOutputTitle(normalizedKind, source),
    summary: normalizeText(summary) || buildDefaultOutputSummary(normalizedKind, { display: normalizedDisplay }),
    collapsed: !!collapsed,
    createdAt: normalizeText(createdAt) || new Date().toISOString(),
    text: '',
    table: null,
    display: normalizedDisplay,
  };
};

export const buildPythonCellOutputModules = ({
  outputText = '',
  display = null,
  dataframe = null,
  tables = [],
  source = 'execution',
  idPrefix = '',
} = {}) => {
  // 设计说明：
  // 1. 一个 Python Cell 现在会积累多个独立输出模块，而不是只保留“最后一份 display/dataframe/text”。
  // 2. 这里统一把一次执行/一次导入拆成稳定的模块数组，供 Notebook 视图做逐模块折叠、导出与写回。
  // 3. 当调用方提供 `idPrefix` 时，模块 id 会保持稳定，避免旧快照迁移阶段的折叠状态抖动。
  const modules = [];
  let itemIndex = 0;
  const normalizedTables = Array.isArray(tables) && tables.length
    ? tables.map((item) => normalizeTablePayload(item)).filter(Boolean)
    : [normalizeTablePayload(dataframe)].filter(Boolean);

  normalizedTables.forEach((table) => {
    const tableModule = createPythonCellOutputModule({
      id: idPrefix ? `${idPrefix}-table-${itemIndex}` : '',
      kind: 'table',
      source,
      table,
    });
    if (tableModule) {
      modules.push(tableModule);
      itemIndex += 1;
    }
  });

  const displayModule = createPythonCellOutputModule({
    id: idPrefix ? `${idPrefix}-${normalizePythonCellOutputDisplayType(display) || 'display'}-${itemIndex}` : '',
    kind: normalizePythonCellOutputDisplayType(display),
    source,
    display,
  });
  if (displayModule) {
    modules.push(displayModule);
    itemIndex += 1;
  }

  const textModule = createPythonCellOutputModule({
    id: idPrefix ? `${idPrefix}-text-${itemIndex}` : '',
    kind: 'text',
    source,
    text: outputText,
  });
  if (textModule) modules.push(textModule);

  return modules;
};

export const normalizePythonCellOutputModules = (outputs = [], legacy = {}) => {
  const normalized = Array.isArray(outputs)
    ? outputs
      .map((item) => createPythonCellOutputModule(item))
      .filter(Boolean)
    : [];

  if (normalized.length) return normalized;
  return buildPythonCellOutputModules(legacy);
};

export const appendPythonCellOutputModules = (existingOutputs = [], nextOutputs = [], { prepend = true } = {}) => {
  const normalizedExisting = normalizePythonCellOutputModules(existingOutputs);
  const normalizedNext = Array.isArray(nextOutputs)
    ? nextOutputs.map((item) => createPythonCellOutputModule(item)).filter(Boolean)
    : [];
  if (!normalizedNext.length) return normalizedExisting;
  return prepend
    ? [...normalizedNext, ...normalizedExisting]
    : [...normalizedExisting, ...normalizedNext];
};

export const getLatestPythonCellOutputModule = (outputs = [], { includeCollapsed = true } = {}) => {
  const normalized = normalizePythonCellOutputModules(outputs);
  return normalized.find((item) => includeCollapsed || !item.collapsed) || null;
};

export const getLatestPythonCellOutputModuleByKind = (outputs = [], kind = '') => {
  const normalizedKind = normalizeText(kind).toLowerCase();
  if (!normalizedKind) return null;
  return normalizePythonCellOutputModules(outputs).find((item) => item.kind === normalizedKind) || null;
};

export const togglePythonCellOutputModuleCollapsed = (outputs = [], moduleId = '', collapsed = null) => {
  const normalizedId = normalizeText(moduleId);
  return normalizePythonCellOutputModules(outputs).map((item) => {
    if (!normalizedId || item.id !== normalizedId) return item;
    return {
      ...item,
      collapsed: collapsed == null ? !item.collapsed : !!collapsed,
    };
  });
};
