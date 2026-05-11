import { commands as vscodeCommands, Position as VscodePosition, Uri as VscodeUri } from 'vscode'
import { createLogger } from '../utils/logger'

const logger = createLogger('PythonCompletionBridge')
const registeredLanguageProviders = new Map()
const activeCompletionRequests = new Set()

const VSCODE_TO_MONACO_COMPLETION_KIND = {
  1: 1,
  2: 2,
  3: 3,
  4: 4,
  5: 5,
  6: 6,
  7: 7,
  8: 8,
  9: 9,
  10: 10,
  11: 11,
  12: 12,
  13: 13,
  14: 14,
  15: 15,
  16: 16,
  17: 17,
  18: 18,
  19: 19,
  20: 20,
  21: 21,
  22: 22,
  23: 23,
  24: 24,
  25: 25,
  26: 26,
  27: 27,
  28: 28,
}

const normalizeLanguageId = (value) => String(value || '').trim().toLowerCase()

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

const resolveCompletionContext = (monaco, model, position) => {
  const lineNumber = Number(position?.lineNumber || 1)
  const column = Number(position?.column || 1)
  const lineContent = String(model?.getLineContent?.(lineNumber) || '')
  const beforeCursor = lineContent.slice(0, Math.max(0, column - 1))
  const hostToken = String(beforeCursor.match(/[^\s()[\]{}"'`,;]+$/)?.[0] || '')
  const fallbackWord = model?.getWordUntilPosition?.(position)

  const typedWord = String(hostToken.match(/([A-Za-z0-9_$]*)$/)?.[1] || '')
  const filterPrefix = hostToken.slice(0, Math.max(0, hostToken.length - typedWord.length))
  const isMemberAccess = !!hostToken
    && /(\.|::|->)$/.test(filterPrefix)
    && /^(?:[A-Za-z_$][\w$]*)(?:(?:\.|::|->)[A-Za-z_$][\w$]*)*(?:\.|::|->)?[A-Za-z0-9_$]*$/.test(hostToken)

  if (isMemberAccess) {
    return {
      hostToken,
      typedWord,
      filterPrefix,
      replaceRange: new monaco.Range(
        lineNumber,
        Math.max(1, column - hostToken.length),
        lineNumber,
        column,
      ),
    }
  }

  const wordStart = Number(fallbackWord?.startColumn || column)
  const wordEnd = Number(fallbackWord?.endColumn || column)
  return {
    hostToken: String(fallbackWord?.word || typedWord || ''),
    typedWord: String(fallbackWord?.word || typedWord || ''),
    filterPrefix: '',
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
  if (!baseLabel || !context) return baseLabel
  if (baseLabel.startsWith(context.filterPrefix)) return baseLabel
  return `${context.filterPrefix}${baseLabel}`
}

const buildBridgedFilterText = (filterText, rawLabel, context) => {
  const baseText = String(filterText || rawLabel || '').trim()
  if (!baseText || !context) return baseText
  if (baseText.startsWith(context.filterPrefix)) return baseText
  return `${context.filterPrefix}${baseText}`
}

const buildBridgedInsertText = (insertText, rawLabel, context) => {
  const baseText = String(insertText || rawLabel || '').trim()
  if (!baseText || !context) return baseText
  if (baseText.startsWith(context.filterPrefix)) return baseText
  return `${context.filterPrefix}${baseText}`
}

const createLspCompletionBridgeProvider = (monaco, languageId) => ({
  triggerCharacters: ['.', ':', '>', '/', '\\', '"', "'", '@', '#', '<'],
  async provideCompletionItems(model, position, context) {
    if (normalizeLanguageId(model?.getLanguageId?.()) !== languageId) {
      return { suggestions: [] }
    }

    const completionContext = resolveCompletionContext(monaco, model, position)

    const uriString = String(model?.uri?.toString?.() || '').trim()
    if (!uriString) {
      return { suggestions: [] }
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
      const result = await vscodeCommands.executeCommand(
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
      if (!items.length) {
        logger.debug('[PythonCompletionBridge] vscode completion returned no items', {
        languageId,
        uriString,
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
        hostToken: completionContext.hostToken,
      })
      return { suggestions: [] }
    }

      const suggestions = []
      for (let index = 0; index < items.length; index += 1) {
        const item = items[index]
        const rawLabel = normalizeCompletionLabel(item?.label)
        if (!rawLabel) continue
        const rawInsertText = typeof item?.insertText === 'string'
          ? item.insertText
          : String(item?.insertText?.value || rawLabel)
        const mapped = {
          label: buildBridgedDisplayLabel(rawLabel, completionContext),
          kind: VSCODE_TO_MONACO_COMPLETION_KIND[item?.kind] || monaco.languages.CompletionItemKind.Text,
          detail: String(item?.detail || ''),
          documentation: normalizeCompletionDocumentation(item?.documentation),
          filterText: buildBridgedFilterText(item?.filterText, rawLabel, completionContext),
          sortText: buildCompletionSortKey(item, rawLabel, index),
          insertText: buildBridgedInsertText(rawInsertText, rawLabel, completionContext),
          insertTextRules: item?.insertText && typeof item.insertText === 'object'
            ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
            : undefined,
          range: completionContext.replaceRange,
        }
        suggestions.push(mapped)
      }

      const dedupedSuggestions = dedupeCompletionSuggestions(suggestions)
      logger.info('[LspCompletionBridge] bridged vscode completion items for Monaco', {
        languageId,
        uriString,
        lineNumber: Number(position?.lineNumber || 0),
        column: Number(position?.column || 0),
        triggerCharacter: triggerCharacter || '',
        hostToken: completionContext.hostToken,
        typedWord: completionContext.typedWord,
        filterPrefix: completionContext.filterPrefix,
        itemCount: items.length,
        suggestionCount: dedupedSuggestions.length,
        firstSuggestionLabel: normalizeCompletionLabel(dedupedSuggestions[0]?.label || ''),
      })
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
    })
  }
}

export const registerPythonCompletionBridge = registerLspCompletionBridge
