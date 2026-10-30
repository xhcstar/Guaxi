import * as monaco from 'monaco-editor'
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../utils/logger'
import { getGuaxiSyntaxClassForCapture } from './guaxiSyntaxTokens'

// Markdown 围栏代码块的上色链路。主编辑器（GuaxiEditor）和探索者的 Markdown cell
// （GuaxiNotebookMarkdownEditor）共用这一份，否则同一段 ```c 在两处会长成不同颜色：
// 主编辑器靠这里的 tree-sitter span + 正则兜底铺 `md-fence-token-*` decoration，
// cell 那边只能吃 Markdown 层的语义 token，整块被染成 string 色。
//
// 这里只负责「给定 model 与块列表 → 一组 decoration」，不持有编辑器状态：
// 调度、可视区裁剪、请求作废判定都留在宿主里。

const logger = createLogger('MarkdownFence')

const MARKDOWN_FENCE_LANG_MAP = {
  js: 'javascript',
  mjs: 'javascript',
  cjs: 'javascript',
  javascript: 'javascript',
  jsx: 'javascript',
  ts: 'typescript',
  mts: 'typescript',
  cts: 'typescript',
  tsx: 'typescript',
  py: 'python',
  python: 'python',
  c: 'c',
  cpp: 'cpp',
  cxx: 'cpp',
  cc: 'cpp',
  h: 'cpp',
  hpp: 'cpp',
  rs: 'rust',
  rust: 'rust',
  sh: 'shell',
  shell: 'shell',
  bash: 'shell',
  zsh: 'shell',
  html: 'html',
  css: 'css',
  json: 'json',
  jsonc: 'json',
  yaml: 'yaml',
  yml: 'yaml',
  lua: 'lua',
  java: 'java',
  tex: 'latex',
  latex: 'latex',
  typ: 'typst',
  org: 'guaxitask',
  mmd: 'mermaid',
  mermaid: 'mermaid',
}

export const resolveFenceLanguageId = (infoRaw) => {
  const info = String(infoRaw || '').trim().toLowerCase()
  if (!info) return 'plaintext'
  const langToken = info.split(/\s+/)[0] || ''
  const mapped = MARKDOWN_FENCE_LANG_MAP[langToken]
  if (mapped) return mapped
  const languages = monaco.languages?.getLanguages?.() || []
  const exists = languages.some((item) => item?.id === langToken)
  if (exists) return langToken
  logger.debug('[MarkdownFence] unknown fence language, fallback to plaintext', { infoRaw })
  return 'plaintext'
}

export const getMarkdownFenceBlocks = (model) => {
  if (!model) return [];
  const blocks = [];
  let inFence = false;
  let fenceToken = '';
  let fenceLang = '';
  let fenceStartLine = 0;

  for (let ln = 1; ln <= model.getLineCount(); ln += 1) {
    const line = model.getLineContent(ln);
    const hit = /^\s*(`{3,}|~{3,})(.*)$/.exec(line);
    if (!hit) continue;

    const token = hit[1];
    if (!inFence) {
      inFence = true;
      fenceToken = token;
      fenceStartLine = ln;
      fenceLang = String(hit[2] || '').trim();
      continue;
    }

    if (token[0] === fenceToken[0] && token.length >= fenceToken.length) {
      const languageId = resolveFenceLanguageId(fenceLang);
      if (languageId) {
        const blockStartLine = fenceStartLine + 1;
        const blockEndLine = Math.max(blockStartLine, ln - 1);
        blocks.push({ languageId, blockStartLine, blockEndLine });
      }
      inFence = false;
      fenceToken = '';
      fenceLang = '';
      fenceStartLine = 0;
    }
  }

  // 未闭合的围栏（用户正在敲 ``` 或误删了收尾行）必须按「延续到文末」处理。
  //
  // 这里曾经直接丢弃这类块，而 getMarkdownFenceContext 却把它判定为「到文末的代码块」。
  // 两份实现对同一份文本给出不同结论，于是出现只在光标附近发生的错乱：
  // 光标在未闭合块里时 context 会把它当代码行去涂色，块的边界一改，
  // 后面的普通正文就跟着被染成代码颜色，直到下一次整篇重扫才恢复。
  // CommonMark 的语义也是「未闭合围栏延续到文末」，这里与之对齐。
  //
  // 没有语言标记时（用户刚敲下 ``` 还没输入语言）仍要列出该块——补全与上下文判定依赖它，
  // 但上色阶段要跳过：把它按 plaintext 铺 md-fence-token-base，会把后面整篇正文染成代码色。
  if (inFence) {
    const languageId = resolveFenceLanguageId(fenceLang);
    if (languageId) {
      const blockStartLine = fenceStartLine + 1;
      const blockEndLine = Math.max(blockStartLine, model.getLineCount());
      if (blockStartLine <= model.getLineCount()) {
        blocks.push({
          languageId,
          blockStartLine,
          blockEndLine,
          unclosed: true,
          fenceHasLanguage: !!String(fenceLang || '').trim(),
        });
      }
    }
  }

  return blocks;
};

export const buildMarkdownFenceBlockText = (model, startLine, endLine) => {
  const lines = [];
  const safeStart = Math.max(1, startLine || 1);
  const safeEnd = Math.max(safeStart, endLine || safeStart);
  for (let ln = safeStart; ln <= safeEnd; ln += 1) {
    lines.push(model.getLineContent(ln));
  }
  return lines.join('\n');
};

export const markdownFenceTokenizeSkippedLanguages = new Set(['python']);
const TREE_SITTER_FENCE_HIGHLIGHT_LANGUAGES = new Set([
  'markdown',
  'javascript',
  'typescript',
  'tsx',
  'python',
  'rust',
  'shell',
  'bash',
  'lua',
  'java',
  'html',
  'c',
  'cpp',
  'latex',
]);

const shouldSkipMarkdownFenceTokenize = (languageId) => {
  const lang = String(languageId || '').trim().toLowerCase();
  if (!lang) return true;
  if (!markdownFenceTokenizeSkippedLanguages.has(lang)) return false;
  // 兼容说明：
  // 当前 Python TextMate grammar 在 Markdown fence 二次 tokenization 时会触发 endless loop 报错，
  // 这里先跳过这类语言的附加高亮，保证编辑器主体与 Python Cell UI 稳定可用。
  return true;
};

export const resolveFenceTokenClass = (tokenType) => (
  // 设计说明：Markdown fence、Notebook cell 和 Monaco semantic token 共用同一个 tree-sitter capture mapper。
  // 这里保留旧函数名作为调用边界，避免散落的 decoration 逻辑继续维护第二份映射表。
  getGuaxiSyntaxClassForCapture(tokenType, { prefix: 'md-fence-token' })
);

export const normalizeTreeSitterFenceHighlightLanguage = (languageId) => {
  const lang = String(languageId || '').trim().toLowerCase();
  if (lang === 'shell' || lang === 'sh' || lang === 'zsh' || lang === 'powershell' || lang === 'ps1') return 'bash';
  if (lang === 'tex') return 'latex';
  return lang;
};

const shouldUseTreeSitterFenceHighlight = (languageId) => {
  const lang = normalizeTreeSitterFenceHighlightLanguage(languageId);
  return !!lang && TREE_SITTER_FENCE_HIGHLIGHT_LANGUAGES.has(lang);
};

export const getEmbeddedBlockLineRanges = (embeddedCtx, model) => {
  if (Array.isArray(embeddedCtx?.lineRanges) && embeddedCtx.lineRanges.length) {
    return embeddedCtx.lineRanges;
  }

  if (!embeddedCtx || !model) return [];

  const blockText = String(embeddedCtx.blockText || '');
  const lineCount = blockText ? blockText.split('\n').length : 1;
  const ranges = [];
  for (let i = 0; i < lineCount; i += 1) {
    const lineNumber = Math.max(1, Number(embeddedCtx.blockStartLine || 1) + i);
    const lineContent = model.getLineContent(lineNumber);
    ranges.push({
      lineNumber,
      startColumn: 1,
      endColumn: Math.max(1, lineContent.length + 1)
    });
  }
  return ranges;
};

const appendFenceBaseLineDecorations = (decorations, block, model, blockText) => {
  if (!decorations || !block || !model) return;
  const lineCount = Math.max(1, String(blockText || '').split('\n').length);
  for (let i = 0; i < lineCount; i += 1) {
    const lineNumber = Number(block.blockStartLine || 1) + i;
    if (lineNumber < 1 || lineNumber > model.getLineCount()) continue;
    const lineContent = model.getLineContent(lineNumber);
    decorations.push({
      range: new monaco.Range(lineNumber, 1, lineNumber, Math.max(1, lineContent.length + 1)),
      options: { inlineClassName: 'md-fence-token-base' }
    });
  }
};

export const appendTreeSitterFenceDecorations = (decorations, block, model, blockText, spans) => {
  if (!decorations || !block || !model || !Array.isArray(spans) || !spans.length) return;
  const lineRanges = getEmbeddedBlockLineRanges({ ...block, blockText }, model);
  if (!lineRanges.length) return;

  for (const span of spans) {
    const startByte = Number(span?.start_byte ?? span?.startByte ?? 0);
    const endByte = Number(span?.end_byte ?? span?.endByte ?? 0);
    if (!Number.isFinite(startByte) || !Number.isFinite(endByte) || endByte <= startByte) continue;

    const className = getGuaxiSyntaxClassForCapture(span?.token_type ?? span?.tokenType ?? '', {
      prefix: 'md-fence-token',
      sourceText: blockText,
      // 这里要的是 JS 字符串下标（token 类型判定会 text[offset - 1] 回看一个字符判断是否点号成员访问），
      // 所以用 UTF-16 偏移而不是字节偏移——块里只要出现过中文注释/字符串，两者就不相等。
      // 纯 ASCII 块下两者相同，因此这个修正对既有观感没有影响，只修正含非 ASCII 的块。
      startOffset: Number(span?.start_utf16 ?? startByte),
    });
    if (!className) continue;

    // 行列由后端一次算好（resolve_token_span_positions），前端不再逐字符建 UTF-8 索引：
    // 那是每个字符一次 String.fromCodePoint + TextEncoder.encode（各分配一个 Uint8Array），
    // 而且是每轮高亮 pass 都要重跑一遍。现在它随 span 一起进缓存，命中时零成本。
    const start = { lineIndex: Number(span?.line ?? 0), column: Number(span?.start_column ?? 0) };
    const end = { lineIndex: Number(span?.end_line ?? 0), column: Number(span?.end_column ?? 0) };
    if (!start.column || !end.column) {
      // 前后端同包发布，正常不会走到这里；真走到了说明后端没给出解析结果，
      // 宁可少装饰一段也不能按 0 列去猜——那会把整行染错。
      logger.warn('[MarkdownFence][TreeSitter] span missing resolved positions, skipped', {
        languageId: block.languageId,
        tokenType: String(span?.token_type ?? ''),
        startByte,
        endByte,
      });
      continue;
    }

    for (let lineIndex = start.lineIndex; lineIndex <= end.lineIndex; lineIndex += 1) {
      const lineRange = lineRanges[lineIndex];
      if (!lineRange) continue;

      const lineWidth = Math.max(0, lineRange.endColumn - lineRange.startColumn);
      const relativeStartColumn = lineIndex === start.lineIndex ? start.column : 1;
      const relativeEndColumn = lineIndex === end.lineIndex ? end.column : lineWidth + 1;
      const startColumn = Math.min(
        Math.max(lineRange.startColumn, lineRange.startColumn + relativeStartColumn - 1),
        lineRange.endColumn
      );
      const endColumn = Math.min(
        Math.max(startColumn, lineRange.startColumn + relativeEndColumn - 1),
        lineRange.endColumn
      );
      if (startColumn === endColumn) continue;

      decorations.push({
        range: new monaco.Range(lineRange.lineNumber, startColumn, lineRange.lineNumber, endColumn),
        options: { inlineClassName: className }
      });
    }
  }
};

const C_LIKE_KEYWORD_RE = /\b(?:alignas|alignof|asm|auto|break|case|class|const|constexpr|continue|decltype|default|delete|do|else|enum|explicit|export|extern|for|friend|goto|if|inline|mutable|namespace|new|noexcept|operator|override|private|protected|public|register|return|sizeof|static|struct|switch|template|this|thread_local|throw|try|typedef|typename|union|using|virtual|volatile|while)\b/g;
const C_LIKE_TYPE_RE = /\b(?:bool|char|char8_t|char16_t|char32_t|complex|double|float|int|long|short|signed|size_t|string|unsigned|void|wchar_t)\b/g;
const C_LIKE_PREPROCESSOR_RE = /^\s*#?\s*(include|define|elif|else|endif|error|if|ifdef|ifndef|line|pragma|undef|warning)\b/g;
const C_LIKE_HEADER_RE = /^\s*#?\s*include\s*(<[^>\n]+>|"[^"\n]+")/g;
const RUST_KEYWORD_RE = /\b(?:as|async|await|break|const|continue|crate|dyn|else|enum|extern|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|ref|return|self|Self|static|struct|super|trait|type|unsafe|use|where|while)\b/g;
const RUST_TYPE_RE = /\b(?:bool|char|f32|f64|i8|i16|i32|i64|i128|isize|str|String|u8|u16|u32|u64|u128|usize|Vec|Option|Result)\b/g;

const appendRegexFenceLineDecorations = (decorations, lineRange, lineText, regex, className, captureIndex = 0, reject = null) => {
  if (!decorations || !lineRange || !lineText || !regex || !className) return 0;
  regex.lastIndex = 0;
  let count = 0;
  let match = null;
  while ((match = regex.exec(lineText)) !== null) {
    const captureText = captureIndex > 0 ? match[captureIndex] : match[0];
    if (!captureText) {
      if (match[0] === '') regex.lastIndex += 1;
      continue;
    }

    const captureOffset = captureIndex > 0
      ? Math.max(0, match[0].indexOf(captureText))
      : 0;
    const startOffset = match.index + captureOffset;
    const endOffset = startOffset + captureText.length;
    if (reject?.(captureText, match, startOffset, endOffset)) continue;

    decorations.push({
      range: new monaco.Range(
        lineRange.lineNumber,
        lineRange.startColumn + startOffset,
        lineRange.lineNumber,
        lineRange.startColumn + endOffset
      ),
      options: { inlineClassName: className }
    });
    count += 1;

    if (match[0] === '') regex.lastIndex += 1;
  }
  return count;
};

export const appendLanguageAwareFenceDecorations = (decorations, block, model, blockText, reason) => {
  const languageId = normalizeTreeSitterFenceHighlightLanguage(block?.languageId || '');
  if (!decorations || !block || !model || !blockText || !languageId) return 0;
  if (!['c', 'cpp', 'rust', 'latex'].includes(languageId)) return 0;

  const lineRanges = getEmbeddedBlockLineRanges({ ...block, blockText }, model);
  const blockLines = String(blockText || '').split('\n');
  let appended = 0;

  for (let i = 0; i < blockLines.length; i += 1) {
    const lineRange = lineRanges[i];
    const lineText = blockLines[i] || '';
    if (!lineRange) continue;

    if (languageId === 'c' || languageId === 'cpp') {
      // 设计思路：
      // 1. C/C++ 预处理指令由 tree-sitter query 捕获不稳定，尤其 Markdown 源码编辑器里
      //    `#include` 有时会被 Markdown 自身语法和围栏装饰叠加影响，表现为 include 仍是 base 色。
      // 2. 这里只在行首补预处理指令，避免把普通函数名/变量名 include 误染为关键字。
      // 3. 头文件路径单独染成 builtin，让 `#include <iostream>` 的指令和导入目标都能清楚区分。
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_PREPROCESSOR_RE, 'md-fence-token-keyword', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_HEADER_RE, 'md-fence-token-builtin', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_KEYWORD_RE, 'md-fence-token-keyword');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, C_LIKE_TYPE_RE, 'md-fence-token-type');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)\s*(?=\()/g, 'md-fence-token-function', 1, (word) => (
        /^(if|for|while|switch|return|sizeof)$/.test(word)
      ));
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)(?=::)/g, 'md-fence-token-namespace', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /::\s*([A-Za-z_]\w*)/g, 'md-fence-token-property', 1);
      continue;
    }

    if (languageId === 'rust') {
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, RUST_KEYWORD_RE, 'md-fence-token-keyword');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, RUST_TYPE_RE, 'md-fence-token-type');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)!(?=\s*[\(\[])/g, 'md-fence-token-macro', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)\s*(?=\()/g, 'md-fence-token-function', 1, (word) => (
        /^(if|for|while|match|loop|return)$/.test(word)
      ));
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b([A-Za-z_]\w*)(?=::)/g, 'md-fence-token-namespace', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /::\s*([A-Za-z_]\w*)/g, 'md-fence-token-property', 1);
      continue;
    }

    if (languageId === 'latex') {
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /(\\[A-Za-z@]+\*?|\\.)/g, 'md-fence-token-keyword', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\\(?:begin|end)\s*\{\s*([A-Za-z*_-][\w*:-]*)\s*\}/g, 'md-fence-token-type', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\\(?:documentclass|usepackage|usetikzlibrary|input|include)\s*(?:\[[^\]]*\])?\s*\{\s*([^}\s,]+)/g, 'md-fence-token-builtin', 1);
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\[[^\]]*\]/g, 'md-fence-token-attribute');
      appended += appendRegexFenceLineDecorations(decorations, lineRange, lineText, /\b\d+(?:\.\d+)?\b/g, 'md-fence-token-number');
    }
  }

  if (appended > 0) {
    logger.debug('[MarkdownFence] language-aware supplemental decorations appended', {
      languageId,
      appended,
      blockStartLine: block.blockStartLine,
      blockEndLine: block.blockEndLine,
      reason,
    });
  }
  return appended;
};

export const fetchTreeSitterFenceTokenSpans = async (languageId, code, context = {}) => {
  const normalized = normalizeTreeSitterFenceHighlightLanguage(languageId);
  if (!normalized) return null;

  const startedAt = performance.now();
  try {
    const response = await invoke('syntax_highlight_tokens', {
      request: {
        code: String(code || ''),
        language: normalized,
      },
    });

    if (!response?.supported) {
      logger.debug('[MarkdownFence][TreeSitter] backend returned unsupported language', {
        languageId,
        normalized,
        reason: context.reason || '',
        blockStartLine: context.blockStartLine || 0,
        blockEndLine: context.blockEndLine || 0,
        frontendElapsedMs: Math.round(performance.now() - startedAt),
      });
      return null;
    }

    const spans = Array.isArray(response?.spans) ? response.spans : [];
    logger.debug('[MarkdownFence][TreeSitter] token spans received', {
      languageId,
      normalized,
      backendDebugId: response?.debug_id,
      spanCount: spans.length,
      backendElapsedMs: response?.elapsed_ms,
      frontendElapsedMs: Math.round(performance.now() - startedAt),
      reason: context.reason || '',
      blockStartLine: context.blockStartLine || 0,
      blockEndLine: context.blockEndLine || 0,
    });
    return spans;
  } catch (error) {
    logger.warn('[MarkdownFence][TreeSitter] token request failed, fallback to Monaco tokenizer', {
      languageId,
      normalized,
      reason: context.reason || '',
      blockStartLine: context.blockStartLine || 0,
      blockEndLine: context.blockEndLine || 0,
      frontendElapsedMs: Math.round(performance.now() - startedAt),
      error,
    });
    return null;
  }
};

// 按「语言 + 块内容」记账的结果表。块内容没变时结果必然相同。
// 两个用途：
//   markdownFenceSpanMemo —— 后端 tree-sitter 的 token span。打字过程中只有正在编辑的那一两个块
//     需要重新请求后端；后端也有 LRU，但一道道请求本身（把块文本序列化上行）仍然要走 IPC。
//   markdownFenceTokenLineMemo —— 非 tree-sitter 语言（css/json/yaml…）走 Monaco 自带 tokenizer 的结果，
//     那是在**主线程**上跑的，整篇重涂时按块全部重跑一遍很浪费。
const MARKDOWN_FENCE_SPAN_MEMO_LIMIT = 300;
// tokenLines 是「数组的数组」，单条比 span 大得多，所以单独给一个更小的上限；
// 它是纯提速缓存，被淘汰只是多跑一次 tokenizer，不影响正确性。
const MARKDOWN_FENCE_TOKENLINE_MEMO_LIMIT = 150;
// 记账表的键里带着整段块文本，所以超大块不进表：否则单条就可能留住几百 KB 的字符串。
const MARKDOWN_FENCE_SPAN_MEMO_MAX_BLOCK_CHARS = 65536;
const markdownFenceSpanMemo = new Map();
const markdownFenceTokenLineMemo = new Map();

const rememberBoundedMemoEntry = (memo, limit, key, value) => {
  memo.delete(key);
  memo.set(key, value);
  while (memo.size > limit) {
    const oldest = memo.keys().next().value;
    if (oldest === undefined) break;
    memo.delete(oldest);
  }
};

const buildFenceMemoKey = (languageId, blockText) => `${languageId}\u0000${blockText}`;
const isFenceMemoizable = (blockText) =>
  String(blockText || '').length <= MARKDOWN_FENCE_SPAN_MEMO_MAX_BLOCK_CHARS;

// 一次批量取回若干围栏代码块的 token span，返回值按「blocks 下标 → spans（不支持时为 null）」。
// 设计说明：以前是每个块一次 invoke 且串行 await，一篇几十个块的笔记，每次输入停顿都要排队等几十次 IPC。
const fetchFenceTokenSpansForBlocks = async (blocks, blockTexts, indexes, reason) => {
  const spansByIndex = new Map();
  const pending = [];
  const requestBlocks = [];

  for (const index of indexes) {
    const languageId = normalizeTreeSitterFenceHighlightLanguage(blocks[index].languageId);
    const blockText = blockTexts[index];
    const memoizable = isFenceMemoizable(blockText);
    const key = buildFenceMemoKey(languageId, blockText);
    const cached = memoizable ? markdownFenceSpanMemo.get(key) : undefined;
    if (cached) {
      spansByIndex.set(index, cached.supported ? cached.spans : null);
      continue;
    }
    pending.push({ index, key, memoizable });
    requestBlocks.push({ code: blockText, language: languageId });
  }

  if (!requestBlocks.length) return spansByIndex;

  try {
    const response = await invoke('syntax_highlight_tokens_batch', {
      request: { blocks: requestBlocks },
    });
    const results = Array.isArray(response?.blocks) ? response.blocks : [];

    for (let i = 0; i < pending.length; i += 1) {
      const target = pending[i];
      const result = results[i];
      if (!result) continue;
      const supported = !!result.supported;
      const spans = Array.isArray(result.spans) ? result.spans : [];
      if (target.memoizable) {
        rememberBoundedMemoEntry(markdownFenceSpanMemo, MARKDOWN_FENCE_SPAN_MEMO_LIMIT, target.key, { supported, spans });
      }
      spansByIndex.set(target.index, supported ? spans : null);
    }

    logger.debug('[MarkdownFence][TreeSitter] token batch resolved', {
      reason,
      requestedCount: requestBlocks.length,
      reusedCount: indexes.length - requestBlocks.length,
      backendElapsedMs: response?.elapsed_ms,
    });
  } catch (error) {
    // 失败不写记忆表：下一轮换语言/换内容时还有机会重新拿到。
    logger.warn('[MarkdownFence][TreeSitter] token batch request failed, fallback to Monaco tokenizer', {
      reason,
      requestedCount: requestBlocks.length,
      error,
    });
  }

  return spansByIndex;
};

/**
 * 给一批已确定的围栏块铺 `md-fence-token-*` decoration。
 * 不做可视区裁剪、不做请求作废判定 —— 那些依赖编辑器状态，由宿主负责。
 */
export const buildMarkdownFenceDecorations = async ({ model, blocks, reason }) => {
  const decorations = [];
  if (!model || !Array.isArray(blocks) || !blocks.length) return decorations;

  const blockTexts = blocks.map((block) => (
    block.blockText ?? buildMarkdownFenceBlockText(model, block.blockStartLine, block.blockEndLine)
  ));
  const treeSitterBlockIndexes = [];
  for (let i = 0; i < blocks.length; i += 1) {
    if (shouldUseTreeSitterFenceHighlight(blocks[i].languageId)) treeSitterBlockIndexes.push(i);
  }
  const spansByIndex = await fetchFenceTokenSpansForBlocks(blocks, blockTexts, treeSitterBlockIndexes, reason);

  for (let blockIndex = 0; blockIndex < blocks.length; blockIndex += 1) {
    const block = blocks[blockIndex];
    const blockText = blockTexts[blockIndex];
    let appliedTreeSitterTokens = false;

    if (spansByIndex.has(blockIndex)) {
      const spans = spansByIndex.get(blockIndex);
      if (spans?.length) {
        appendFenceBaseLineDecorations(decorations, block, model, blockText);
        appendTreeSitterFenceDecorations(decorations, block, model, blockText, spans);
        appendLanguageAwareFenceDecorations(decorations, block, model, blockText, reason);
        appliedTreeSitterTokens = true;
        logger.debug('[MarkdownFence][TreeSitter] decorations appended', {
          languageId: block.languageId,
          spanCount: spans.length,
          blockStartLine: block.blockStartLine,
          blockEndLine: block.blockEndLine,
          reason,
        });
      } else if (spans) {
        logger.debug('[MarkdownFence][TreeSitter] empty token spans, fallback to Monaco tokenizer', {
          languageId: block.languageId,
          blockStartLine: block.blockStartLine,
          blockEndLine: block.blockEndLine,
          reason,
        });
      }
    }

    if (!appliedTreeSitterTokens && shouldSkipMarkdownFenceTokenize(block.languageId)) {
      logger.debug('[MarkdownFence] skip extra tokenize for compatibility', {
        languageId: block.languageId,
        reason,
      });
      appendFenceBaseLineDecorations(decorations, block, model, blockText);
      appendLanguageAwareFenceDecorations(decorations, block, model, blockText, reason);
      continue;
    }

    if (!appliedTreeSitterTokens) {
      // 非 tree-sitter 语言走 Monaco 自带的 tokenizer，那是在主线程上跑的；块内容没变时结果必然相同，
      // 整篇重涂时按块全部重跑一遍是纯浪费，所以和 span 一样记账。
      // 键用**原始** languageId：tokenize 用的就是它，而 Monaco 对 sh/bash 这类写法可能有各自的定义，
      // 归一化后再做键会把两者的结果混在一起。
      const tokenLineKey = buildFenceMemoKey(block.languageId, blockText);
      const tokenLinesMemoizable = isFenceMemoizable(blockText);
      const cachedTokenLines = tokenLinesMemoizable ? markdownFenceTokenLineMemo.get(tokenLineKey) : undefined;

      let tokenLines = [];
      if (cachedTokenLines) {
        tokenLines = cachedTokenLines;
      } else {
        try {
          tokenLines = monaco.editor.tokenize(blockText || '', block.languageId) || [];
        } catch (error) {
          logger.warn('[MarkdownFence] tokenize failed', { language: block.languageId }, error);
          appendFenceBaseLineDecorations(decorations, block, model, blockText);
          continue;
        }
        if (tokenLinesMemoizable) {
          rememberBoundedMemoEntry(
            markdownFenceTokenLineMemo,
            MARKDOWN_FENCE_TOKENLINE_MEMO_LIMIT,
            tokenLineKey,
            tokenLines
          );
        }
      }

      // 设计思路：
      // 1. tree-sitter 覆盖 Python/LaTeX/主流代码块，Monaco tokenizer 继续作为 css/json/yaml 等语言兜底。
      // 2. 无论 token 细分是否可用，都先铺一层 base 装饰，保证围栏代码块在暗色/亮色主题下保持可读。
      // 3. 后端失败时这里会留下日志并回退到原生 tokenizer，不影响用户继续编辑。
      appendFenceBaseLineDecorations(decorations, block, model, blockText);

      for (let i = 0; i < tokenLines.length; i += 1) {
        const tokens = tokenLines[i] || [];
        if (!tokens.length) continue;
        const lineNumber = block.blockStartLine + i;
        const lineContent = model.getLineContent(lineNumber);
        const lineLen = lineContent.length;

        for (let j = 0; j < tokens.length; j += 1) {
          const token = tokens[j];
          const className = resolveFenceTokenClass(token?.type || '');
          if (!className) continue;

          const startOffset = token?.offset || 0;
          const nextOffset = tokens[j + 1] ? tokens[j + 1].offset : lineLen;
          const startColumn = Math.min(Math.max(1, startOffset + 1), lineLen + 1);
          const endColumn = Math.min(Math.max(startColumn, nextOffset + 1), lineLen + 1);
          if (startColumn === endColumn) continue;

          decorations.push({
            range: new monaco.Range(lineNumber, startColumn, lineNumber, endColumn),
            options: { inlineClassName: className }
          });
        }
      }
      appendLanguageAwareFenceDecorations(decorations, block, model, blockText, reason);
    }
  }

  return decorations;
};

/**
 * 给一个只用来「看源码」的 Markdown 编辑器挂上围栏上色（探索者的 Markdown cell）。
 *
 * 主编辑器那边有自己的可视区裁剪、光标块预热和 LSP 请求作废判定，所以不共用这一层；
 * 这里只做「内容变了 → 重算 → 落地」，保证同样的 ```c 在两处铺同一套 md-fence-token-* 颜色。
 */
export const createMarkdownFenceHighlighter = (editor) => {
  if (!editor || typeof editor.getModel !== 'function') {
    return { refresh: () => {}, dispose: () => {} };
  }

  let collection = null;
  let timer = 0;
  let requestSeq = 0;
  let disposed = false;

  const run = async (reason) => {
    if (disposed) return;
    const model = editor.getModel();
    if (!model || model.getLanguageId?.() !== 'markdown') return;

    const seq = (requestSeq += 1);
    // 未闭合且还没写语言的围栏只参与「它是什么块」的判定，不上色——
    // 按 plaintext 铺一层 base 会把后面整篇正文染成代码色。
    const blocks = getMarkdownFenceBlocks(model)
      .filter((item) => !(item?.unclosed && item?.fenceHasLanguage === false));
    const decorations = await buildMarkdownFenceDecorations({ model, blocks, reason });
    if (disposed || seq !== requestSeq) return;

    if (!collection) collection = editor.createDecorationsCollection([]);
    collection.set(decorations);
  };

  const schedule = (reason = 'cell') => {
    if (disposed) return;
    window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      timer = 0;
      run(reason).catch((error) => {
        logger.warn('[MarkdownFence] cell highlight failed', { reason }, error);
      });
    }, 60);
  };

  const contentListener = editor.onDidChangeModelContent?.(() => schedule('cell-debounced'));
  schedule('cell-init');

  return {
    refresh: schedule,
    dispose: () => {
      disposed = true;
      window.clearTimeout(timer);
      contentListener?.dispose?.();
      collection?.clear?.();
      collection = null;
    },
  };
};
