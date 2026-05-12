import { commands as vscodeCommands, Position as VscodePosition, Uri as VscodeUri } from 'vscode'
import { createLogger } from '../utils/logger'
import { requestLspCompletionItems } from './lspClient'

const logger = createLogger('PythonCompletionBridge')
const registeredLanguageProvidersByMonaco = new WeakMap()
const activeCompletionRequests = new Set()
const bridgedCompletionCache = new Map()
const scheduledRetriggers = new Map()
const BRIDGED_COMPLETION_CACHE_TTL_MS = 3500
const BRIDGED_COMPLETION_RETRIGGER_GAP_MS = 450
const DIRECT_MEMBER_COMPLETION_TIMEOUT_MS = 1400
const DIRECT_DEFAULT_COMPLETION_TIMEOUT_MS = 260

const VSCODE_COMPLETION_KIND_NAMES = {
  0: 'Text',
  1: 'Method',
  2: 'Function',
  3: 'Constructor',
  4: 'Field',
  5: 'Variable',
  6: 'Class',
  7: 'Interface',
  8: 'Module',
  9: 'Property',
  10: 'Unit',
  11: 'Value',
  12: 'Enum',
  13: 'Keyword',
  14: 'Snippet',
  15: 'Color',
  16: 'File',
  17: 'Reference',
  18: 'Folder',
  19: 'EnumMember',
  20: 'Constant',
  21: 'Struct',
  22: 'Event',
  23: 'Operator',
  24: 'TypeParameter',
  25: 'Issue',
  26: 'User',
}

const LSP_COMPLETION_KIND_NAMES = {
  1: 'Text',
  2: 'Method',
  3: 'Function',
  4: 'Constructor',
  5: 'Field',
  6: 'Variable',
  7: 'Class',
  8: 'Interface',
  9: 'Module',
  10: 'Property',
  11: 'Unit',
  12: 'Value',
  13: 'Enum',
  14: 'Keyword',
  15: 'Snippet',
  16: 'Color',
  17: 'File',
  18: 'Reference',
  19: 'Folder',
  20: 'EnumMember',
  21: 'Constant',
  22: 'Struct',
  23: 'Event',
  24: 'Operator',
  25: 'TypeParameter',
}

const normalizeLanguageId = (value) => String(value || '').trim().toLowerCase()

const getRegisteredLanguageProviders = (monaco) => {
  let providers = registeredLanguageProvidersByMonaco.get(monaco)
  if (!providers) {
    providers = new Map()
    registeredLanguageProvidersByMonaco.set(monaco, providers)
  }
  return providers
}

const normalizeCompletionLabel = (label) => {
  if (typeof label === 'string') return label.trim()
  if (label && typeof label === 'object') {
    if (typeof label.label === 'string') return label.label.trim()
    if (typeof label.value === 'string') return label.value.trim()
  }
  return ''
}

const normalizeCompletionDocumentation = (documentation) => {
  if (!documentation) return ''
  if (typeof documentation === 'string') return documentation
  if (typeof documentation?.value === 'string') return documentation.value
  return ''
}

const buildCompletionSortKey = (item, label, index) => {
  const explicitSortText = String(item?.sortText || '').trim()
  if (explicitSortText) return explicitSortText
  return `${String(index).padStart(6, '0')}:${String(label || '').toLowerCase()}`
}

const PYTHON_IDENTIFIER_COMPLETION_RE = /^[A-Za-z_]\w*$/

const isPythonMemberAccessContext = (languageId, context) => (
  normalizeLanguageId(languageId) === 'python'
  && String(context?.filterPrefix || '').endsWith('.')
)

const isEncodedOrPathLikeCompletionText = (value) => {
  const text = String(value || '').trim()
  if (!text) return false
  return /%[0-9a-f]{2}/i.test(text)
    || /[\\/]/.test(text)
    || /:\/\/|^[.]?[0-9a-f]{2}$/i.test(text)
}

const normalizePythonMemberCandidate = (value) => (
  String(value || '')
    .trim()
    .replace(/\s.*$/, '')
    .replace(/\(.*$/, '')
)

const shouldKeepBridgedCompletionItem = (languageId, item, rawLabel, rawInsertText, context) => {
  if (!isPythonMemberAccessContext(languageId, context)) return true

  const labelCandidate = normalizePythonMemberCandidate(rawLabel)
  const filterText = String(item?.filterText || '').trim()
  const insertText = String(rawInsertText || '').trim()

  // Design note:
  // Python member completion after `obj.` must be an attribute name. Logs showed
  // candidates such as `.2f`, which come from URI/path fragments and become
  // `sys..2f` after the embedded prefix rewrite. Monaco then filters the whole
  // list to "No suggestions" even though the LSP server returned items.
  if (!PYTHON_IDENTIFIER_COMPLETION_RE.test(labelCandidate)) return false
  if (isEncodedOrPathLikeCompletionText(rawLabel)) return false
  if (isEncodedOrPathLikeCompletionText(filterText)) return false
  if (isEncodedOrPathLikeCompletionText(insertText)) return false
  return true
}

const shouldDropNoisyPythonMemberCandidate = (languageId, item, completionSource, context) => {
  if (!isPythonMemberAccessContext(languageId, context)) return false
  const rawKind = Number(item?.kind)
  if (!Number.isFinite(rawKind)) return false
  const kindName = completionSource === 'direct-lsp'
    ? LSP_COMPLETION_KIND_NAMES[rawKind]
    : VSCODE_COMPLETION_KIND_NAMES[rawKind]

  // 设计说明：
  // 1. 这里只丢弃明显属于模板/关键字的候选，不再因为 kind=Text 就整批过滤。
  // 2. basedpyright 在部分 Notebook 虚拟文档场景会把真实成员候选标成 Text；旧逻辑会在 `obj.` 后
  //    把这些真实候选全部丢掉，最终 Monaco 只显示 No suggestions。
  // 3. URI/path 噪声已经由 shouldKeepBridgedCompletionItem 过滤；剩余 Text 候选会在 kind 映射阶段兜底成 Property。
  return kindName === 'Keyword' || kindName === 'Snippet'
}

const resolveBridgedCompletionKind = (monaco, item, completionSource, context) => {
  const rawKind = Number(item?.kind)
  const kindNames = completionSource === 'direct-lsp'
    ? LSP_COMPLETION_KIND_NAMES
    : VSCODE_COMPLETION_KIND_NAMES
  const kindName = Number.isFinite(rawKind) ? kindNames[rawKind] : ''
  const memberAccess = isPythonMemberAccessContext(context?.languageId, context)
  const pythonLanguage = normalizeLanguageId(context?.languageId) === 'python'
  const effectiveKindName = memberAccess && (!kindName || kindName === 'Text')
    ? 'Property'
    : pythonLanguage && (!kindName || kindName === 'Text')
      ? 'Variable'
      : (kindName || 'Text')
  const resolvedKind = monaco?.languages?.CompletionItemKind?.[effectiveKindName]

  // 设计说明：
  // 1. `vscode.executeCompletionItemProvider` 返回的是 VSCode API 的 0 基准 kind，
  //    直接 LSP 返回的是协议层 1 基准 kind，两者不能共用一张表。
  // 2. Notebook/Python 编辑器关闭了 `showWords`，如果语义候选被错误兜底成 Text(18)，
  //    Monaco 会在进入 completion model 前把它们全部过滤掉，界面就只剩 `No suggestions`。
  // 3. Python 的未知/Text 候选统一兜底成 Variable；成员访问里兜底成 Property，
  //    既保留 basedpyright 的真实候选，又避免重新打开 Monaco 文档词补全噪声。
  return Number.isFinite(resolvedKind)
    ? resolvedKind
    : monaco.languages.CompletionItemKind.Text
}

const dedupeCompletionSuggestions = (suggestions = []) => {
  const seen = new Set()
  const output = []
  for (const item of Array.isArray(suggestions) ? suggestions : []) {
    if (!item) continue
    const key = [
      String(normalizeCompletionLabel(item.label) || ''),
      String(item.insertText || ''),
      String(item.filterText || ''),
      String(item.range?.startLineNumber || ''),
      String(item.range?.startColumn || ''),
      String(item.range?.endLineNumber || ''),
      String(item.range?.endColumn || ''),
    ].join('::')
    if (seen.has(key)) continue
    seen.add(key)
    output.push(item)
  }
  return output
}

const buildBridgedCompletionCacheKey = (languageId, uriString, position, context) => [
  normalizeLanguageId(languageId),
  String(uriString || ''),
  Number(position?.lineNumber || 0),
  Number(position?.column || 0),
  String(context?.hostToken || ''),
  String(context?.typedWord || ''),
  String(context?.filterPrefix || ''),
].join('::')

const isMarkdownFencedCodePosition = (model, position) => {
  if (normalizeLanguageId(model?.getLanguageId?.()) !== 'markdown') return false
  const lineNumber = Number(position?.lineNumber || 1)
  let openFence = ''

  for (let line = 1; line <= lineNumber; line += 1) {
    const text = String(model?.getLineContent?.(line) || '')
    const match = text.match(/^\s*(```+|~~~+)/)
    if (!match) continue
    const marker = match[1][0]
    if (!openFence) {
      openFence = marker
    } else if (openFence === marker) {
      openFence = ''
    }
  }

  return !!openFence
}

const readBridgedCompletionCache = (cacheKey) => {
  const cached = bridgedCompletionCache.get(cacheKey)
  if (!cached) return []
  if (Date.now() - Number(cached.createdAt || 0) > BRIDGED_COMPLETION_CACHE_TTL_MS) {
    bridgedCompletionCache.delete(cacheKey)
    return []
  }
  return Array.isArray(cached.suggestions) ? cached.suggestions : []
}

const writeBridgedCompletionCache = (cacheKey, suggestions) => {
  if (!Array.isArray(suggestions) || !suggestions.length) return
  bridgedCompletionCache.set(cacheKey, {
    createdAt: Date.now(),
    suggestions,
  })
}

const findFocusedEditorForModel = (monaco, model) => {
  const editors = typeof monaco?.editor?.getEditors === 'function'
    ? monaco.editor.getEditors()
    : []
  return (Array.isArray(editors) ? editors : [])
    .find((candidate) => candidate?.hasTextFocus?.() && candidate?.getModel?.() === model) || null
}

const scheduleBridgedCompletionRetrigger = (monaco, model, position, cacheKey, reason) => {
  const now = Date.now()
  const lastAt = Number(scheduledRetriggers.get(cacheKey) || 0)
  if (now - lastAt < BRIDGED_COMPLETION_RETRIGGER_GAP_MS) return
  scheduledRetriggers.set(cacheKey, now)

  window.setTimeout(() => {
    const targetEditor = findFocusedEditorForModel(monaco, model)
    if (!targetEditor) {
      logger.debug('[LspCompletionBridge] retrigger skipped because focused editor was not found', {
        reason,
        cacheKey,
      })
      return
    }

    try {
      targetEditor.trigger('guaxi-lsp-completion-bridge', 'hideSuggestWidget', null)
    } catch {
    }

    logger.info('[LspCompletionBridge] retrigger suggest from cached bridge results', {
      reason,
      cacheKey,
      lineNumber: Number(position?.lineNumber || 0),
      column: Number(position?.column || 0),
    })
    targetEditor.trigger('guaxi-lsp-completion-bridge', 'editor.action.triggerSuggest', {})
  }, 20)
}

const resolveCompletionContext = (monaco, model, position) => {
  const lineNumber = Number(position?.lineNumber || 1)
  const column = Number(position?.column || 1)
  const lineContent = String(model?.getLineContent?.(lineNumber) || '')
  const beforeCursor = lineContent.slice(0, Math.max(0, column - 1))
  const hostToken = String(beforeCursor.match(/[^\s()[\]{}"'`,;]+$/)?.[0] || '')
  const fallbackWord = model?.getWordUntilPosition?.(position)
  const typedWord = String(hostToken.match(/([A-Za-z0-9_$]*)$/)?.[1] || '')
  const filterPrefix = hostToken.slice(0, Math.max(0, hostToken.length - typedWord.length))
  const wordStart = Number(fallbackWord?.startColumn || column)
  const wordEnd = Number(fallbackWord?.endColumn || column)

  // 设计说明：
  // 1. Notebook Python Cell 自己就是 `python` Monaco model，不是 Markdown 宿主里的嵌入语言。
  // 2. 对 `sys.` / `sys.pa`，Monaco 的合法 completion range 应该只覆盖点号后的当前单词：
  //    `sys.` 时是 5:5 空 range，`sys.pa` 时是 `pa` 的 range。
  // 3. 之前把整段 `sys.` 当 range，并把 label/insertText 改成 `sys.argv`，这是 Markdown fence 的特殊绕过策略；
  //    放到原生 Python model 会让 suggest model 的前缀过滤和 range 校验互相打架，表现为 LSP 已返回但 UI 仍 No suggestions。
  return {
    hostToken,
    typedWord: String(fallbackWord?.word || typedWord || ''),
    filterPrefix,
    replaceRange: new monaco.Range(
      lineNumber,
      Math.max(1, wordStart),
      lineNumber,
      Math.max(wordStart, wordEnd),
    ),
  }
}

const buildBridgedDisplayLabel = (label, context) => {
  const baseLabel = normalizeCompletionLabel(label)
  if (!baseLabel) return ''
  if (isPythonMemberAccessContext(context?.languageId, context)) {
    // 设计说明：`obj.` 后的候选应保持为成员名本身。把 `obj.`
    // 拼进 label 会让空替换区间和过滤文本不一致，Monaco 可能直接把列表判空。
    return baseLabel
  }
  return baseLabel
}

const buildBridgedFilterText = (filterText, rawLabel, context) => {
  const baseText = String(filterText || rawLabel || '').trim()
  if (isPythonMemberAccessContext(context?.languageId, context)) {
    // 设计说明：filterText 要和点号后的 range 对齐，让 `pa` 匹配 `path`，
    // 而不是拿 `pa` 去匹配 `sys.path`。
    return baseText
  }
  return baseText
}

const buildBridgedInsertText = (insertText, rawLabel, context) => {
  const baseText = String(insertText || rawLabel || '').trim()
  return baseText
}

const getCompletionInsertText = (item, rawLabel) => {
  if (typeof item?.insertText === 'string') return item.insertText
  if (typeof item?.insertText?.value === 'string') return item.insertText.value
  if (typeof item?.textEdit?.newText === 'string') return item.textEdit.newText
  return rawLabel
}

const createLspCompletionBridgeProvider = (monaco, languageId) => ({
  triggerCharacters: ['.', ':', '>', '/', '\\', '"', "'", '@', '#', '<'],
  async provideCompletionItems(model, position, context) {
    if (normalizeLanguageId(model?.getLanguageId?.()) !== languageId) {
      return { suggestions: [] }
    }
    if (languageId === 'markdown' && isMarkdownFencedCodePosition(model, position)) {
      logger.debug('[LspCompletionBridge] markdown bridge skipped inside fenced code block', {
        uriString: String(model?.uri?.toString?.() || ''),
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
      })
      return { suggestions: [] }
    }

    const completionContext = {
      ...resolveCompletionContext(monaco, model, position),
      languageId,
    }

    const uriString = String(model?.uri?.toString?.() || '').trim()
    if (!uriString) {
      return { suggestions: [] }
    }

    const cacheKey = buildBridgedCompletionCacheKey(languageId, uriString, position, completionContext)
    const cachedSuggestions = readBridgedCompletionCache(cacheKey)
    if (cachedSuggestions.length) {
      logger.info('[LspCompletionBridge] completion resolved from bridge cache', {
        languageId,
        uriString,
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
        hostToken: completionContext.hostToken,
        filterPrefix: completionContext.filterPrefix,
        suggestionCount: cachedSuggestions.length,
        firstSuggestionLabel: normalizeCompletionLabel(cachedSuggestions[0]?.label || ''),
        firstSuggestionFilterText: String(cachedSuggestions[0]?.filterText || ''),
        firstSuggestionInsertText: String(cachedSuggestions[0]?.insertText || ''),
        firstSuggestionKind: Number(cachedSuggestions[0]?.kind ?? -1),
        firstSuggestionRange: cachedSuggestions[0]?.range
          ? `${cachedSuggestions[0].range.startLineNumber}:${cachedSuggestions[0].range.startColumn}-${cachedSuggestions[0].range.endLineNumber}:${cachedSuggestions[0].range.endColumn}`
          : '',
      })
      return {
        suggestions: cachedSuggestions,
        incomplete: true,
      }
    }

    const requestKey = [
      languageId,
      uriString,
      Number(position?.lineNumber || 0),
      Number(position?.column || 0),
      String(context?.triggerCharacter || ''),
    ].join('::')
    if (activeCompletionRequests.has(requestKey)) {
      logger.debug('[PythonCompletionBridge] skip recursive completion request', {
        languageId,
        uriString,
        requestKey,
      })
      return { suggestions: [] }
    }

    activeCompletionRequests.add(requestKey)
    try {
      const triggerCharacter = context?.triggerKind === monaco.languages.CompletionTriggerKind.TriggerCharacter
        ? String(context?.triggerCharacter || '')
        : undefined
      // 设计说明：
      // 1. 这条日志放在真正调用 VSCode completion command 前，用来确认 Monaco provider 已被触发。
      // 2. 如果后续仍然显示“没有建议”，可以直接对照后端 `textDocument/completion` 日志判断是桥接未触发、
      //    VSCode provider 未返回，还是 LSP server 本身返回 0 个候选。
      // 3. 这里同时打印 VSCode API 对象类型，避免 import/初始化问题再次被吞成空建议。
      logger.info('[LspCompletionBridge] requesting vscode completion items', {
        languageId,
        uriString,
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
        triggerCharacter: triggerCharacter || '',
        hostToken: completionContext.hostToken,
        linePrefix: String(model?.getLineContent?.(Number(position?.lineNumber || 1)) || '')
          .slice(0, Math.max(0, Number(position?.column || 1) - 1))
          .slice(-80),
        hasVscodeCommandApi: typeof vscodeCommands?.executeCommand === 'function',
        hasVscodeUriApi: typeof VscodeUri?.parse === 'function',
        hasVscodePositionApi: typeof VscodePosition === 'function',
      })
      const directResult = await requestLspCompletionItems(
        languageId,
        uriString,
        Number(position?.lineNumber || 1),
        Number(position?.column || 1),
        triggerCharacter || undefined,
        isPythonMemberAccessContext(languageId, completionContext)
          ? DIRECT_MEMBER_COMPLETION_TIMEOUT_MS
          : DIRECT_DEFAULT_COMPLETION_TIMEOUT_MS,
      )
      const result = directResult?.items?.length
        ? directResult
        : await vscodeCommands.executeCommand(
          'vscode.executeCompletionItemProvider',
          VscodeUri.parse(uriString),
          new VscodePosition(
            Math.max(0, Number(position?.lineNumber || 1) - 1),
            Math.max(0, Number(position?.column || 1) - 1),
          ),
          triggerCharacter || undefined,
          0,
        )
      const items = Array.isArray(result?.items) ? result.items : []
      const completionSource = directResult?.items?.length ? 'direct-lsp' : 'vscode-command'
      if (!items.length) {
        logger.debug('[PythonCompletionBridge] vscode completion returned no items', {
          languageId,
          uriString,
          lineNumber: Number(position?.lineNumber || 0),
          column: Number(position?.column || 0),
          hostToken: completionContext.hostToken,
          completionSource,
          resultType: Object.prototype.toString.call(result),
          hasItemsArray: Array.isArray(result?.items),
        })
        return { suggestions: [] }
      }

      const suggestions = []
      let filteredOutCount = 0
      let firstFilteredOutLabel = ''
      let noisyKindFilteredCount = 0
      let textKindSuggestionCount = 0
      let propertyFallbackKindCount = 0
      let firstRawKind = ''
      let firstMappedKind = ''
      for (let index = 0; index < items.length; index += 1) {
        const item = items[index]
        const rawLabel = normalizeCompletionLabel(item?.label)
        if (!rawLabel) continue
        const rawInsertText = getCompletionInsertText(item, rawLabel)
        if (!shouldKeepBridgedCompletionItem(languageId, item, rawLabel, rawInsertText, completionContext)) {
          filteredOutCount += 1
          if (!firstFilteredOutLabel) firstFilteredOutLabel = rawLabel
          continue
        }
        if (shouldDropNoisyPythonMemberCandidate(languageId, item, completionSource, completionContext)) {
          noisyKindFilteredCount += 1
          if (!firstFilteredOutLabel) firstFilteredOutLabel = rawLabel
          continue
        }
        const mappedKind = resolveBridgedCompletionKind(monaco, item, completionSource, completionContext)
        if (!firstRawKind) firstRawKind = String(item?.kind ?? '')
        if (!firstMappedKind) firstMappedKind = String(mappedKind)
        if (mappedKind === monaco.languages.CompletionItemKind.Text) {
          textKindSuggestionCount += 1
        }
        if (
          isPythonMemberAccessContext(languageId, completionContext)
          && mappedKind === monaco.languages.CompletionItemKind.Property
          && (!Number.isFinite(Number(item?.kind)) || Number(item?.kind) === 0 || Number(item?.kind) === 1)
        ) {
          propertyFallbackKindCount += 1
        }
        const mapped = {
          label: buildBridgedDisplayLabel(rawLabel, completionContext),
          kind: mappedKind,
          detail: String(item?.detail || ''),
          documentation: normalizeCompletionDocumentation(item?.documentation),
          filterText: buildBridgedFilterText(item?.filterText, rawLabel, completionContext),
          sortText: buildCompletionSortKey(item, rawLabel, index),
          insertText: buildBridgedInsertText(rawInsertText, rawLabel, completionContext),
          insertTextRules: (item?.insertText && typeof item.insertText === 'object') || Number(item?.insertTextFormat || 0) === 2
            ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
            : undefined,
          range: completionContext.replaceRange,
        }
        suggestions.push(mapped)
      }

      const dedupedSuggestions = dedupeCompletionSuggestions(suggestions)
      writeBridgedCompletionCache(cacheKey, dedupedSuggestions)
      logger.info('[LspCompletionBridge] bridged vscode completion items for Monaco', {
        languageId,
        uriString,
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
        triggerCharacter: triggerCharacter || '',
        hostToken: completionContext.hostToken,
        typedWord: completionContext.typedWord,
        filterPrefix: completionContext.filterPrefix,
        replaceStartColumn: completionContext.replaceRange?.startColumn || 0,
        replaceEndColumn: completionContext.replaceRange?.endColumn || 0,
        completionSource,
        itemCount: items.length,
        filteredOutCount,
        noisyKindFilteredCount,
        firstFilteredOutLabel,
        suggestionCount: dedupedSuggestions.length,
        firstSuggestionLabel: normalizeCompletionLabel(dedupedSuggestions[0]?.label || ''),
        firstSuggestionFilterText: String(dedupedSuggestions[0]?.filterText || ''),
        firstSuggestionInsertText: String(dedupedSuggestions[0]?.insertText || ''),
        firstRawKind,
        firstMappedKind,
        textKindSuggestionCount,
        propertyFallbackKindCount,
        firstSuggestionRange: dedupedSuggestions[0]?.range
          ? `${dedupedSuggestions[0].range.startLineNumber}:${dedupedSuggestions[0].range.startColumn}-${dedupedSuggestions[0].range.endLineNumber}:${dedupedSuggestions[0].range.endColumn}`
          : '',
      })
      if (dedupedSuggestions.length) {
        scheduleBridgedCompletionRetrigger(
          monaco,
          model,
          position,
          cacheKey,
          completionSource,
        )
      }
      return {
        suggestions: dedupedSuggestions,
        incomplete: !!result?.isIncomplete,
      }
    } catch (error) {
      logger.warn('[LspCompletionBridge] vscode completion bridge failed', {
        languageId,
        uriString,
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
        error: String(error?.message || error),
      })
      return { suggestions: [] }
    } finally {
      activeCompletionRequests.delete(requestKey)
    }
  },
})

// 设计说明：
// 1. 统一 LSP 扩展体系后，Monaco 和 VSCode completion provider 之间仍需要一层薄桥接。
// 2. 语言服务器实际返回了候选时，Standalone Monaco 在成员访问上下文里可能按 `sys.` / `foo::`
//    这种宿主 token 过滤，最终把真实语义候选筛成 `No suggestions`。
// 3. 这里对所有扩展启用的 LSP 语言注册桥接 provider，主动把 VSCode completion item 映射成 Monaco
//    可接受的 label/filterText/range，并用请求 key 防止递归调用自身。
export const registerLspCompletionBridge = (monaco, languageIds = ['python']) => {
  if (!monaco?.languages?.registerCompletionItemProvider) return
  const registeredLanguageProviders = getRegisteredLanguageProviders(monaco)
  for (const rawLanguageId of Array.isArray(languageIds) ? languageIds : []) {
    const languageId = normalizeLanguageId(rawLanguageId)
    if (!languageId || registeredLanguageProviders.has(languageId)) continue
    const disposable = monaco.languages.registerCompletionItemProvider(
      languageId,
      createLspCompletionBridgeProvider(monaco, languageId),
    )
    registeredLanguageProviders.set(languageId, disposable)
    logger.info('[LspCompletionBridge] registered completion bridge', {
      languageId,
      registeredCountForMonaco: registeredLanguageProviders.size,
    })
  }
}

export const registerPythonCompletionBridge = registerLspCompletionBridge
