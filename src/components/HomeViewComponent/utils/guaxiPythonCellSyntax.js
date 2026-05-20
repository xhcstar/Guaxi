// 设计说明：
// 1. Python Cell 统一使用 Markdown HTML 注释哨兵：<!--py--> ... <!--/py-->。
// 2. 带状态的 Cell 使用同一套开始标记承载 JSON：<!--py {"id":"..."}-->，不再引入第二种旧标记。
// 3. 普通 ```python fenced code block 保持 Markdown 原生行为，不再被编辑态 Cell 接管。
// 4. 预览阶段会剥离隐藏 Cell 源码，只保留用户后续写回的 Markdown 输出。

const PY_CELL_START_RE = /^<!--\s*py(?:\s+(\{.*\}))?\s*-->$/;
const PY_CELL_END_RE = /^<!--\s*\/py\s*-->$/;

const defaultCellMeta = () => ({
  title: '',
  codeCollapsed: false,
  outputCollapsed: false,
  sessionCollapsed: true,
  consoleCollapsed: false,
});

export const normalizePythonCellMeta = (meta = {}) => {
  const next = {
    ...defaultCellMeta(),
    ...(meta && typeof meta === 'object' ? meta : {}),
  };

  next.title = String(next.title || '').trim();
  next.codeCollapsed = !!next.codeCollapsed;
  next.outputCollapsed = !!next.outputCollapsed;
  next.sessionCollapsed = next.sessionCollapsed !== false;
  next.consoleCollapsed = !!next.consoleCollapsed;
  return next;
};

export const tryParsePythonCellMeta = (rawMeta) => {
  const text = String(rawMeta || '').trim();
  if (!text) return defaultCellMeta();
  try {
    return normalizePythonCellMeta(JSON.parse(text));
  } catch {
    return defaultCellMeta();
  }
};

export const serializePythonCellMeta = (meta = {}) => {
  const normalized = normalizePythonCellMeta(meta);
  const hasCustomMeta =
    normalized.title ||
    normalized.codeCollapsed ||
    normalized.outputCollapsed ||
    !normalized.sessionCollapsed ||
    normalized.consoleCollapsed;
  if (!hasCustomMeta) {
    return '<!--py-->';
  }
  const json = JSON.stringify(normalized);
  return `<!--py ${json}-->`;
};

export const buildPythonCellBlock = ({
  code = '',
  meta = {},
} = {}) => {
  const normalizedMeta = normalizePythonCellMeta(meta);
  const codeText = String(code || '').replace(/\r\n?/g, '\n');
  return `${serializePythonCellMeta(normalizedMeta)}\n${codeText}\n<!--/py-->`;
};

export const createPythonCellStableId = () => `py-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;

export const parsePythonCellBlocks = (markdown = '') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const blocks = [];

  for (let i = 0; i < lines.length; i += 1) {
    // 设计说明：
    // 1. 编辑态 Python Cell 只识别显式隐藏语法，避免误伤普通 ```python Markdown 代码块。
    // 2. 这样普通 fenced code block 仍按 Markdown 原生方式显示，高亮与预览行为也保持稳定。
    const startMatch = String(lines[i] || '').match(PY_CELL_START_RE);
    if (startMatch) {
      let endIndex = -1;
      for (let j = i + 1; j < lines.length; j += 1) {
        if (PY_CELL_END_RE.test(String(lines[j] || ''))) {
          endIndex = j;
          break;
        }
      }
      if (endIndex < 0) continue;

      const meta = tryParsePythonCellMeta(startMatch[1] || '');
      const codeLines = lines.slice(i + 1, endIndex);
      const code = codeLines.join('\n');
      const blockLines = lines.slice(i, endIndex + 1);

      blocks.push({
        index: blocks.length,
        startLine: i + 1,
        endLine: endIndex + 1,
        codeStartLine: i + 2,
        codeEndLine: endIndex,
        code,
        meta,
        raw: blockLines.join('\n'),
        storage: 'hidden',
      });

      i = endIndex;
    }
  }

  return blocks;
};

export const stripPythonCellBlocksFromMarkdown = (markdown = '') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const kept = [];
  let insidePyCell = false;

  for (const line of lines) {
    if (PY_CELL_START_RE.test(String(line || ''))) {
      insidePyCell = true;
      continue;
    }
    if (insidePyCell && PY_CELL_END_RE.test(String(line || ''))) {
      insidePyCell = false;
      continue;
    }
    if (!insidePyCell) {
      kept.push(line);
    }
  }

  return kept.join('\n');
};

export const maskPythonCellBlocksForMarkdown = (markdown = '') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const masked = [];
  let insidePyCell = false;
  let maskedLineCount = 0;

  for (const line of lines) {
    const textLine = String(line || '');
    const trimmed = textLine.trim();

    // 设计说明：
    // 1. Python Cell 的真实存储格式是 Markdown HTML 注释哨兵，内部内容仍是 Python 源码。
    // 2. 大纲、lint、目录等 Markdown 解析器不能直接读取这些源码，否则 `# 注释` 会被识别成 Markdown 标题。
    // 3. 这里用空行替换 Cell 区域而不是直接删除，保留原始行号，保证右侧大纲和面包屑跳转仍然准确。
    if (!insidePyCell && PY_CELL_START_RE.test(trimmed)) {
      insidePyCell = true;
      masked.push('');
      maskedLineCount += 1;
      continue;
    }

    if (insidePyCell) {
      masked.push('');
      maskedLineCount += 1;
      if (PY_CELL_END_RE.test(trimmed)) {
        insidePyCell = false;
      }
      continue;
    }

    masked.push(textLine);
  }

  if (maskedLineCount > 0) {
    console.info('[PythonCellSyntax] masked python cells for markdown parser', {
      maskedLineCount,
      totalLineCount: lines.length,
    });
  }

  return masked.join('\n');
};

export const transformPythonCellBlocksForPreview = (markdown = '', { filePath = '' } = {}) => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const output = [];

  for (let i = 0; i < lines.length; i += 1) {
    const startMatch = String(lines[i] || '').match(PY_CELL_START_RE);
    if (!startMatch) {
      output.push(lines[i]);
      continue;
    }

    let endIndex = -1;
    for (let j = i + 1; j < lines.length; j += 1) {
      if (PY_CELL_END_RE.test(String(lines[j] || ''))) {
        endIndex = j;
        break;
      }
    }

    if (endIndex < 0) {
      output.push(lines[i]);
      continue;
    }

    const meta = tryParsePythonCellMeta(startMatch[1] || '');
    const block = {
      startLine: i + 1,
      endLine: endIndex + 1,
    };
    const code = lines.slice(i + 1, endIndex).join('\n');
    const cellId = createPythonCellId(filePath, block);

    if (meta.title) {
      output.push(`> Python Cell: ${meta.title}`);
    }
    output.push(`\`\`\`python guaxi-cell=${cellId}`);
    output.push(code);
    output.push('```');

    i = endIndex;
  }

  return output.join('\n');
};

export const replacePythonCellBlock = (markdown = '', block, nextCode, nextMeta) => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  if (!block?.startLine || !block?.endLine) return text;

  const lines = text.split('\n');
  const replacementText = buildPythonCellBlock({
    code: nextCode,
    meta: nextMeta ?? block.meta ?? {},
  });
  const replacement = replacementText.split('\n');

  lines.splice(block.startLine - 1, block.endLine - block.startLine + 1, ...replacement);
  return lines.join('\n');
};

export const createPythonCellId = (filePath, block) => {
  const fileKey = String(filePath || 'untitled').replace(/[^\w.-]+/g, '_');
  const stableId = String(block?.meta?.id || '').trim();
  if (stableId) {
    return `pycell-${fileKey}-${stableId}`;
  }
  return `pycell-${fileKey}-${block?.startLine || 0}-${block?.endLine || 0}`;
};

export const createPythonSessionId = (filePath) => {
  return String(filePath || '').trim() || '__guaxi_global__';
};

const escapeMarkdownTableValue = (value) => {
  return String(value == null ? '' : value)
    .replace(/\r\n?/g, ' ')
    .replace(/\n/g, ' ')
    .replace(/\|/g, '\\|');
};

export const dataframeToMarkdownTable = (table = {}) => {
  const columns = Array.isArray(table.columns) ? table.columns.map((item) => String(item ?? '')) : [];
  if (!columns.length) return '';

  const header = `| ${columns.map(escapeMarkdownTableValue).join(' | ')} |`;
  const divider = `| ${columns.map(() => '---').join(' | ')} |`;
  const rows = Array.isArray(table.rows) ? table.rows : [];
  const body = rows.map((row) => {
    const values = columns.map((_, index) => escapeMarkdownTableValue(Array.isArray(row) ? row[index] : ''));
    return `| ${values.join(' | ')} |`;
  });
  return [header, divider, ...body].join('\n');
};
