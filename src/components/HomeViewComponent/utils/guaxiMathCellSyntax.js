// 设计说明：
// 1. Math Cell 统一使用 Markdown HTML 注释哨兵：<!--math--> ... <!--/math-->。
// 2. 带状态的 Cell 使用同一套开始标记承载 JSON：<!--math {"id":"..."}-->。
// 3. 普通 $$...$$ / \[...\] / ```math fenced code block 保持 Markdown 原生行为，
//    不再被编辑态 Math Cell 接管，这样 SymPy 命令不会被错误渲染为 LaTeX 公式。
// 4. 预览阶段会剥离隐藏 Cell 源码，只保留用户后续写回的 Markdown 输出。
// 5. 与 Python Cell 的 <!--py--> 语法对称设计，探索者模式下大纲、保存、加载流程保持一致。

const MATH_CELL_START_RE = /^<!--\s*math(?:\s+(\{.*\}))?\s*-->$/;
const MATH_CELL_END_RE = /^<!--\s*\/math\s*-->$/;

const defaultMathCellMeta = () => ({
  title: '',
  outputCollapsed: false,
});

export const normalizeMathCellMeta = (meta = {}) => {
  const next = {
    ...defaultMathCellMeta(),
    ...(meta && typeof meta === 'object' ? meta : {}),
  };

  next.title = String(next.title || '').trim();
  next.storage = String(next.storage || '').trim();
  next.outputCollapsed = !!next.outputCollapsed;
  return next;
};

export const tryParseMathCellMeta = (rawMeta) => {
  const text = String(rawMeta || '').trim();
  if (!text) return defaultMathCellMeta();
  try {
    return normalizeMathCellMeta(JSON.parse(text));
  } catch {
    return defaultMathCellMeta();
  }
};

export const serializeMathCellMeta = (meta = {}) => {
  const normalized = normalizeMathCellMeta(meta);
  const defaultKeys = Object.keys(defaultMathCellMeta());
  const hasExtraMeta = Object.keys(normalized).some((key) => {
    if (defaultKeys.includes(key)) return false;
    const value = normalized[key];
    if (typeof value === 'string') return value.trim();
    return value !== undefined && value !== null && value !== false;
  });
  const hasCustomMeta =
    normalized.title ||
    normalized.storage ||
    normalized.outputCollapsed ||
    hasExtraMeta;
  if (!hasCustomMeta) {
    return '<!--math-->';
  }
  const defaults = defaultMathCellMeta();
  const compactMeta = {};
  Object.keys(normalized).forEach((key) => {
    const value = normalized[key];
    if (Object.prototype.hasOwnProperty.call(defaults, key) && defaults[key] === value) return;
    if (typeof value === 'string' && !value.trim()) return;
    if (value === undefined || value === null) return;
    compactMeta[key] = value;
  });
  const json = JSON.stringify(compactMeta);
  return `<!--math ${json}-->`;
};

export const buildMathCellBlock = ({
  expression = '',
  meta = {},
} = {}) => {
  const normalizedMeta = normalizeMathCellMeta(meta);
  const expressionText = String(expression || '').replace(/\r\n?/g, '\n');
  return `${serializeMathCellMeta(normalizedMeta)}\n${expressionText}\n<!--/math-->`;
};

// 设计说明：
// 1. 探索者模式下 Math Cell 表达式保存在 `.cellmeta` 的 cells[].expression 中，
//    源 Markdown 只保留轻量引用块。
// 2. 引用块保留稳定 id 与 storage 标记，用来维持 cell 在文档中的位置、顺序和跨次加载匹配关系。
// 3. 旧文档中的完整 <!--math-->...<!--/math--> 块仍由 parseMathCellBlocks 读取；
//    保存时再迁移为引用块，避免破坏兼容性。
export const buildMathCellReferenceBlock = ({
  meta = {},
} = {}) => {
  const normalizedMeta = normalizeMathCellMeta({
    ...(meta || {}),
    storage: 'cellmeta',
  });
  return `${serializeMathCellMeta(normalizedMeta)}\n<!--/math-->`;
};

export const parseMathCellBlocks = (markdown = '') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const blocks = [];

  for (let i = 0; i < lines.length; i += 1) {
    const startMatch = String(lines[i] || '').match(MATH_CELL_START_RE);
    if (startMatch) {
      let endIndex = -1;
      for (let j = i + 1; j < lines.length; j += 1) {
        if (MATH_CELL_END_RE.test(String(lines[j] || ''))) {
          endIndex = j;
          break;
        }
      }
      if (endIndex < 0) continue;

      const meta = tryParseMathCellMeta(startMatch[1] || '');
      const expressionLines = lines.slice(i + 1, endIndex);
      const expression = expressionLines.join('\n');
      const blockLines = lines.slice(i, endIndex + 1);

      blocks.push({
        index: blocks.length,
        startLine: i + 1,
        endLine: endIndex + 1,
        expressionStartLine: i + 2,
        expressionEndLine: endIndex,
        expression,
        meta,
        raw: blockLines.join('\n'),
        storage: 'hidden',
      });

      i = endIndex;
    }
  }

  return blocks;
};

export const stripMathCellBlocksFromMarkdown = (markdown = '') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const kept = [];
  let insideMathCell = false;

  for (const line of lines) {
    if (MATH_CELL_START_RE.test(String(line || ''))) {
      insideMathCell = true;
      continue;
    }
    if (insideMathCell && MATH_CELL_END_RE.test(String(line || ''))) {
      insideMathCell = false;
      continue;
    }
    if (!insideMathCell) {
      kept.push(line);
    }
  }

  return kept.join('\n');
};

export const maskMathCellBlocksForMarkdown = (markdown = '') => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const masked = [];
  let insideMathCell = false;
  let maskedLineCount = 0;

  for (const line of lines) {
    const textLine = String(line || '');
    const trimmed = textLine.trim();

    // 设计说明：
    // 1. Math Cell 的真实存储格式是 Markdown HTML 注释哨兵，内部内容仍是数学/SymPy 表达式。
    // 2. 大纲、lint、目录等 Markdown 解析器不能直接读取这些表达式，否则 `# 注释` 会被识别成 Markdown 标题。
    // 3. 这里用空行替换 Cell 区域而不是直接删除，保留原始行号，保证右侧大纲和面包屑跳转仍然准确。
    if (!insideMathCell && MATH_CELL_START_RE.test(trimmed)) {
      insideMathCell = true;
      masked.push('');
      maskedLineCount += 1;
      continue;
    }

    if (insideMathCell) {
      masked.push('');
      maskedLineCount += 1;
      if (MATH_CELL_END_RE.test(trimmed)) {
        insideMathCell = false;
      }
      continue;
    }

    masked.push(textLine);
  }

  if (maskedLineCount > 0) {
    console.info('[MathCellSyntax] masked math cells for markdown parser', {
      maskedLineCount,
      totalLineCount: lines.length,
    });
  }

  return masked.join('\n');
};

export const transformMathCellBlocksForPreview = (markdown = '', { filePath = '' } = {}) => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  const lines = text.split('\n');
  const output = [];

  for (let i = 0; i < lines.length; i += 1) {
    const startMatch = String(lines[i] || '').match(MATH_CELL_START_RE);
    if (!startMatch) {
      output.push(lines[i]);
      continue;
    }

    let endIndex = -1;
    for (let j = i + 1; j < lines.length; j += 1) {
      if (MATH_CELL_END_RE.test(String(lines[j] || ''))) {
        endIndex = j;
        break;
      }
    }

    if (endIndex < 0) {
      output.push(lines[i]);
      continue;
    }

    const meta = tryParseMathCellMeta(startMatch[1] || '');
    const block = {
      startLine: i + 1,
      endLine: endIndex + 1,
    };
    const expression = lines.slice(i + 1, endIndex).join('\n');
    const cellId = createMathCellId(filePath, block);

    if (meta.title) {
      output.push(`> Math Cell: ${meta.title}`);
    }
    output.push(`\`\`\`math guaxi-cell=${cellId}`);
    output.push(expression);
    output.push('```');

    i = endIndex;
  }

  return output.join('\n');
};

export const replaceMathCellBlock = (markdown = '', block, nextExpression, nextMeta) => {
  const text = String(markdown || '').replace(/\r\n?/g, '\n');
  if (!block?.startLine || !block?.endLine) return text;

  const lines = text.split('\n');
  const replacementText = buildMathCellBlock({
    expression: nextExpression,
    meta: nextMeta ?? block.meta ?? {},
  });
  const replacement = replacementText.split('\n');

  lines.splice(block.startLine - 1, block.endLine - block.startLine + 1, ...replacement);
  return lines.join('\n');
};

export const createMathCellId = (filePath, block) => {
  const fileKey = String(filePath || 'untitled').replace(/[^\w.-]+/g, '_');
  const stableId = String(block?.meta?.id || '').trim();
  if (stableId) {
    return `mathcell-${fileKey}-${stableId}`;
  }
  return `mathcell-${fileKey}-${block?.startLine || 0}-${block?.endLine || 0}`;
};
