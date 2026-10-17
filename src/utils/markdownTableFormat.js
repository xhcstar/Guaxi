// Markdown 表格源码对齐：把表格块按「显示列宽」补齐空格，让 | 在等宽字体下对齐。
//
// 这些函数原先内联在 GuaxiEditor.vue 里，现在抽出来给主编辑器、探索者 cell、
// 模板管理器和 Slidev 编辑器共用同一套对齐算法，避免各入口格式化结果不一致。
//
// 设计说明：
// 1. 宽度按「显示列宽」计算（CJK/emoji 记 2 列），不是 JS string.length。
// 2. 只在真实 Markdown 表格块上生效，代码围栏与自定义 cell 由调用方提前跳过。
// 3. 分隔行也按列宽补齐 `-`，否则表格在源码编辑态看起来对不齐。
import { createLogger } from './logger'

const logger = createLogger('MarkdownTableFormat')

const isPotentialTableLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  const pipeCount = getMarkdownTablePipeCount(trimmed);
  return pipeCount >= 2;
};

const getMarkdownTablePipeCount = (line) => {
  const text = String(line || '');
  let count = 0;
  let escaped = false;
  let codeTickLen = 0;

  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === '\\') {
      escaped = true;
      continue;
    }
    if (ch === '`') {
      let j = i;
      while (j < text.length && text[j] === '`') j += 1;
      const len = j - i;
      if (!codeTickLen) {
        codeTickLen = len;
      } else if (len >= codeTickLen) {
        codeTickLen = 0;
      }
      i = j - 1;
      continue;
    }
    if (ch === '|' && !codeTickLen) count += 1;
  }

  return count;
};

const splitMarkdownTableRowCells = (line) => {
  const raw = String(line || '').trim();
  let text = raw;
  if (!text.startsWith('|')) text = `| ${text}`;
  if (!text.endsWith('|')) text = `${text} |`;

  const parts = [];
  let current = '';
  let escaped = false;
  let codeTickLen = 0;

  // 设计说明：
  // 1. Markdown 表格允许用 `\|` 表示单元格内的竖线，也允许在代码 span 中出现 `|`。
  // 2. 旧版直接 split('|') 会把这些内容误拆成多列，导致“某行分隔符数量不一致”时越格式化越歪。
  // 3. 这里只把非转义、非代码 span 中的 `|` 当作列分隔符。
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (escaped) {
      current += ch;
      escaped = false;
      continue;
    }
    if (ch === '\\') {
      current += ch;
      escaped = true;
      continue;
    }
    if (ch === '`') {
      let j = i;
      while (j < text.length && text[j] === '`') j += 1;
      const ticks = text.slice(i, j);
      current += ticks;
      const len = ticks.length;
      if (!codeTickLen) {
        codeTickLen = len;
      } else if (len >= codeTickLen) {
        codeTickLen = 0;
      }
      i = j - 1;
      continue;
    }
    if (ch === '|' && !codeTickLen) {
      parts.push(current);
      current = '';
      continue;
    }
    current += ch;
  }
  parts.push(current);

  if (parts.length && !parts[0].trim()) parts.shift();
  if (parts.length && !parts[parts.length - 1].trim()) parts.pop();
  return parts.map((c) => c.trim());
};

const isTableSeparatorLine = (line) => {
  const trimmed = (line || '').trim();
  if (!trimmed) return false;
  const cells = splitMarkdownTableRowCells(trimmed);
  return cells.length >= 2 && cells.every((cell) => /^\s*:?-{3,}:?\s*$/.test(cell));
};

const normalizeTableRowCells = (line) => {
  return splitMarkdownTableRowCells(line);
};

const buildTableRow = (cells) => {
  return `| ${cells.map((c) => (c ?? '').trim()).join(' | ')} |`;
};

const detectSeparatorAlign = (cell) => {
  const v = String(cell || '').trim();
  const left = v.startsWith(':');
  const right = v.endsWith(':');
  if (left && right) return 'center';
  if (right) return 'right';
  if (left) return 'left';
  return null;
};

const normalizeSeparatorCell = (cell, align) => {
  const base = (cell || '').trim();
  const hasMin = base.replace(/:/g, '').replace(/-/g, '').length === 0;
  const detected = align || detectSeparatorAlign(base);

  if (!hasMin) {
    if (detected === 'left') return ':---';
    if (detected === 'center') return ':---:';
    if (detected === 'right') return '---:';
    return '---';
  }

  if (detected === 'left') return ':---';
  if (detected === 'center') return ':---:';
  if (detected === 'right') return '---:';
  return '---';
};

// 判断 Unicode 组合附加符号（Combining Marks）。
// 这类字符通常叠加在前一个字符上显示，因此在“可视宽度”计算中按 0 宽度处理。
const isCombiningCodePoint = (cp) => {
  return (
    (cp >= 0x0300 && cp <= 0x036f) ||
    (cp >= 0x1ab0 && cp <= 0x1aff) ||
    (cp >= 0x1dc0 && cp <= 0x1dff) ||
    (cp >= 0x20d0 && cp <= 0x20ff) ||
    (cp >= 0xfe20 && cp <= 0xfe2f)
  );
};

const isZeroWidthCodePoint = (cp) => {
  return (
    isCombiningCodePoint(cp) ||
    cp === 0x200d ||
    cp === 0xfe0e ||
    cp === 0xfe0f ||
    (cp >= 0x1f3fb && cp <= 0x1f3ff) ||
    (cp >= 0xe0100 && cp <= 0xe01ef)
  );
};

const isEmojiLikeCodePoint = (cp) => {
  return (
    (cp >= 0x2300 && cp <= 0x23ff) ||
    (cp >= 0x2600 && cp <= 0x27bf) ||
    (cp >= 0x1f000 && cp <= 0x1faff)
  );
};

// 判断 Unicode 码点是否为“全宽”字符。
// 用于近似估算等宽字体下的显示宽度：CJK/全角标点/emoji 等通常按 2 列宽处理。
const isFullWidthCodePoint = (cp) => {
  if (cp >= 0x1100 && (
    cp <= 0x115f ||
    cp === 0x2329 ||
    cp === 0x232a ||
    (cp >= 0x2e80 && cp <= 0xa4cf && cp !== 0x303f) ||
    (cp >= 0xac00 && cp <= 0xd7a3) ||
    (cp >= 0xf900 && cp <= 0xfaff) ||
    (cp >= 0xfe10 && cp <= 0xfe19) ||
    (cp >= 0xfe30 && cp <= 0xfe6f) ||
    (cp >= 0xff00 && cp <= 0xff60) ||
    (cp >= 0xffe0 && cp <= 0xffe6) ||
    (cp >= 0x1f300 && cp <= 0x1f64f) ||
    (cp >= 0x1f680 && cp <= 0x1f6ff) ||
    (cp >= 0x1f900 && cp <= 0x1f9ff) ||
    (cp >= 0x1fa70 && cp <= 0x1faff) ||
    (cp >= 0x20000 && cp <= 0x3fffd)
  )) return true;

  return false;
};

const segmentDisplayGraphemes = (input) => {
  const s = String(input ?? '');
  try {
    if (typeof Intl !== 'undefined' && typeof Intl.Segmenter === 'function') {
      const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
      return Array.from(segmenter.segment(s), (item) => item.segment);
    }
  } catch {
  }
  return Array.from(s);
};

// 计算字符串的“可视宽度”（用于 Markdown 表格源码对齐）。
// 规则：控制字符忽略；组合字符按 0；全宽字符按 2；其它按 1。
// 说明：这是为了提升中英文混排时 | 的对齐观感，而不是严格的排版引擎实现。
const getDisplayWidth = (input) => {
  const s = String(input ?? '');
  let width = 0;

  // 设计说明：
  // 1. 表格源码对齐看的是 Monaco 等宽字体中的“显示列宽”，不是 JS string.length。
  // 2. ✅ / ⏳ / 🚫 等 emoji 由单个或多个 code point 组成，通常显示为 2 列。
  // 3. 优先按 grapheme cluster 计宽，避免组合音标、变体选择符、ZWJ emoji 序列导致列宽漂移。
  for (const cluster of segmentDisplayGraphemes(s)) {
    const cps = Array.from(cluster).map((ch) => ch.codePointAt(0) || 0).filter(Boolean);
    if (!cps.length) continue;
    if (cps.every((cp) => cp < 32 || cp === 0x7f || isZeroWidthCodePoint(cp))) continue;
    if (cps.some((cp) => isEmojiLikeCodePoint(cp) || isFullWidthCodePoint(cp))) {
      width += 2;
      continue;
    }
    for (const cp of cps) {
      if (cp < 32 || cp === 0x7f || isZeroWidthCodePoint(cp)) continue;
      width += isFullWidthCodePoint(cp) ? 2 : 1;
    }
  }

  return width;
};

// 按列宽对齐单元格：用“可视宽度”补齐空格，而不是用 string.length。
// 这样在 Monaco 编辑器的等宽字体下，中英文混排时各行的竖线更容易对齐。
const padTableCell = (text, width, align) => {
  const raw = String(text ?? '').trim();
  const current = getDisplayWidth(raw);
  const w = Math.max(0, Number(width) || 0);
  if (current >= w) return raw;

  const delta = w - current;
  if (align === 'right') return `${' '.repeat(delta)}${raw}`;
  if (align === 'center') {
    const left = Math.floor(delta / 2);
    const right = delta - left;
    return `${' '.repeat(left)}${raw}${' '.repeat(right)}`;
  }
  return `${raw}${' '.repeat(delta)}`;
};

// 将表格的二维 cells 渲染为 Markdown 行，并根据列宽补空格。
// 目标：在源码编辑态也能保持每列 | 对齐，提升可读性。
const buildAlignedTableLines = (rows) => {
  const safeRows = Array.isArray(rows) ? rows : [];
  if (!safeRows.length) return [];

  const colCount = safeRows.reduce((m, r) => Math.max(m, Array.isArray(r) ? r.length : 0), 0) || 1;
  const sepRow = safeRows[1] || [];
  const aligns = Array.from({ length: colCount }, (_, i) => detectSeparatorAlign(sepRow[i]));

  const widths = Array.from({ length: colCount }, (_, i) => {
    const align = aligns[i];
    // 设计说明：
    // 1. Markdown 分隔行的 `:` 本身也占据源码列宽，不能只把宽度理解成 `-` 的数量。
    // 2. 左/右对齐至少需要 `:---` / `---:` 4 列，居中至少需要 `:---:` 5 列。
    // 3. 先把列宽提升到“分隔单元格总宽度”，后续正文补空格与分隔行渲染才能让 `|` 完全对齐。
    if (align === 'center') return 5;
    if (align === 'left' || align === 'right') return 4;
    return 3;
  });
  safeRows.forEach((row, rowIndex) => {
    if (!Array.isArray(row) || rowIndex === 1) return;
    for (let i = 0; i < colCount; i += 1) {
      const v = String(row[i] ?? '').trim();
      widths[i] = Math.max(widths[i], getDisplayWidth(v));
    }
  });

  return safeRows.map((row, rowIndex) => {
    const cells = Array.from({ length: colCount }, (_, i) => (Array.isArray(row) ? row[i] : ''));

    if (rowIndex === 1) {
      const sepCells = cells.map((c, i) => {
        const a = aligns[i];
        const colonCount = a === 'center' ? 2 : (a === 'left' || a === 'right' ? 1 : 0);
        const w = Math.max(3 + colonCount, widths[i]);
        const dashes = '-'.repeat(Math.max(3, w - colonCount));
        if (a === 'left') return `:${dashes}`;
        if (a === 'center') return `:${dashes}:`;
        if (a === 'right') return `${dashes}:`;
        return dashes;
      });
      return `| ${sepCells.join(' | ')} |`;
    }

    const padded = cells.map((c, i) => padTableCell(c, widths[i], aligns[i]));
    return `| ${padded.join(' | ')} |`;
  });
};

const formatMarkdownTableBlockLines = (lines) => {
  const rawLines = Array.isArray(lines) ? lines : [];
  if (rawLines.length < 2) return rawLines;
  const rows = rawLines.map((line) => normalizeTableRowCells(line));
  const colCount = rows.reduce((m, r) => Math.max(m, Array.isArray(r) ? r.length : 0), 0) || 1;
  const paddedRows = rows.map((row, rowIndex) => {
    const next = [...row];
    while (next.length < colCount) next.push('');
    if (rowIndex === 1) {
      return next.map((cell) => normalizeSeparatorCell(cell, null));
    }
    return next.map((cell) => String(cell ?? '').trim());
  });
  return buildAlignedTableLines(paddedRows);
};

const formatMarkdownTablesInText = (content, source = 'manual') => {
  const text = String(content ?? '');
  if (!text.includes('|')) return text;

  const hasTrailingNewline = /\n$/.test(text);
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  if (hasTrailingNewline && lines.length && lines[lines.length - 1] === '') {
    lines.pop();
  }

  const result = [];
  let formattedCount = 0;
  let inFence = false;
  let fenceChar = '';
  let fenceLen = 0;
  let inOpaqueCell = false;

  // 设计说明：
  // 1. 后端 Markdown 格式化器目前只保留表格块，不负责源码列对齐。
  // 2. 这里在前端二次处理真实 Markdown 表格，跳过 fenced code block，避免误改代码中的 `|`。
  // 3. 分隔行可能不在当前扫描块第一行，取“分隔行上一行”作为表头，兼容表格前后紧贴其它含 `|` 的行。
  // 4. 探索者/Notebook 的自定义 cell 整块跳过：里面的 `|`（Python 位运算、LaTeX）不是表格。
  for (let i = 0; i < lines.length;) {
    const line = lines[i] || '';

    if (inOpaqueCell) {
      result.push(line);
      if (isNotebookCellEndLine(line)) inOpaqueCell = false;
      i += 1;
      continue;
    }
    if (isNotebookCellStartLine(line)) {
      result.push(line);
      inOpaqueCell = true;
      i += 1;
      continue;
    }

    const fence = /^\s*(`{3,}|~{3,})/.exec(line);
    if (fence) {
      const mark = fence[1];
      if (!inFence) {
        inFence = true;
        fenceChar = mark[0];
        fenceLen = mark.length;
      } else if (mark[0] === fenceChar && mark.length >= fenceLen) {
        inFence = false;
        fenceChar = '';
        fenceLen = 0;
      }
      result.push(line);
      i += 1;
      continue;
    }

    if (inFence || !isPotentialTableLine(line)) {
      result.push(line);
      i += 1;
      continue;
    }

    let end = i;
    while (end < lines.length && isPotentialTableLine(lines[end] || '')) {
      end += 1;
    }

    let separatorIndex = -1;
    for (let idx = i; idx < end; idx += 1) {
      if (isTableSeparatorLine(lines[idx] || '')) {
        separatorIndex = idx;
        break;
      }
    }

    if (separatorIndex <= i) {
      result.push(line);
      i += 1;
      continue;
    }

    const tableStart = separatorIndex - 1;
    for (let idx = i; idx < tableStart; idx += 1) {
      result.push(lines[idx]);
    }

    const tableLines = lines.slice(tableStart, end);
    const nextTableLines = formatMarkdownTableBlockLines(tableLines);
    result.push(...nextTableLines);
    formattedCount += 1;
    i = end;
  }

  const nextText = `${result.join('\n')}${hasTrailingNewline ? '\n' : ''}`;
  if (formattedCount > 0 && nextText !== text) {
    logger.info('[MarkdownTable] formatted markdown table blocks', {
      source,
      tableCount: formattedCount
    });
  }
  return nextText;
};


export {
  isPotentialTableLine,
  getMarkdownTablePipeCount,
  splitMarkdownTableRowCells,
  isTableSeparatorLine,
  normalizeTableRowCells,
  buildTableRow,
  detectSeparatorAlign,
  normalizeSeparatorCell,
  getDisplayWidth,
  padTableCell,
  buildAlignedTableLines,
  formatMarkdownTableBlockLines,
  formatMarkdownTablesInText,
}
