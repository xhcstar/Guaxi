import { invoke } from '@tauri-apps/api/core'
import lspExtensionCatalog from '../../shared/lsp_extensions.json'
import { createLogger } from '../utils/logger'

export type LspLanguageEntry = {
  language: string
  server: string
  bin_name: string
  args: string[]
  auto_download: boolean
  extension_id?: string
  source?: string
}

type ExtensionCatalogEntry = {
  id: string
  languages?: string[]
  lsp?: {
    server_id?: string
  }
}

type RuntimeExtensionState = {
  id?: string
  languages?: string[]
  installed?: boolean
  compatibility_level?: string
  runtime_status?: {
    ready?: boolean
  }
}

const logger = createLogger('lspCapabilities')

const LSP_LANGUAGE_ALIASES: Record<string, string> = {
  ra_syntax_tree: 'rust',
  javascriptreact: 'javascript',
  typescriptreact: 'typescript',
  shellscript: 'shell',
  sh: 'shell',
  tex: 'latex',
}

const normalizeLanguage = (languageId: string) =>
  LSP_LANGUAGE_ALIASES[String(languageId || '').trim().toLowerCase()]
  || String(languageId || '').trim().toLowerCase()

export const normalizeLspLanguageId = (languageId: string) => normalizeLanguage(languageId)

const normalizeCatalogEntries = (
  Array.isArray((lspExtensionCatalog as { extensions?: unknown[] })?.extensions)
    ? (lspExtensionCatalog as { extensions: ExtensionCatalogEntry[] }).extensions
    : []
)
  .flatMap((entry) => {
    const extensionId = String(entry?.id || '').trim().toLowerCase()
    const serverId = String(entry?.lsp?.server_id || '').trim().toLowerCase()
    const languages = Array.isArray(entry?.languages)
      ? entry.languages.map((item) => normalizeLanguage(item)).filter(Boolean)
      : []
    if (!extensionId || !serverId || !languages.length) return []

    return languages.map((language) => ({
      language,
      server: serverId,
      bin_name: '',
      args: [],
      auto_download: false,
      extension_id: extensionId,
      source: 'catalog',
    }))
  })

const LSP_LANGUAGE_ENTRIES_INTERNAL: LspLanguageEntry[] = []
const LSP_SUPPORTED_LANGUAGES_INTERNAL = new Set<string>()
const entryByLanguage = new Map<string, LspLanguageEntry>()
const runtimeExtensionByLanguage = new Map<string, RuntimeExtensionState>()

const rebuildLanguageRegistry = () => {
  LSP_LANGUAGE_ENTRIES_INTERNAL.splice(0, LSP_LANGUAGE_ENTRIES_INTERNAL.length)
  LSP_SUPPORTED_LANGUAGES_INTERNAL.clear()
  entryByLanguage.clear()

  // 设计说明：
  // 1. LSP 能力表现在只从“已安装且运行时就绪”的扩展快照生成，避免旧的 shared/lsp_languages.json
  //    把未安装扩展的语言也暴露成可用 LSP。
  // 2. catalog 仍负责描述语言和 server 映射，但必须由运行时 registry 确认扩展可用后才加入编辑器能力表。
  // 3. 这样编辑器、Notebook、状态栏都会看到同一套 VSCode-like 扩展能力，不再和独立 LSP 状态分叉。
  normalizeCatalogEntries.forEach((entry) => {
    const normalized = normalizeLanguage(entry.language)
    const runtimeEntry = runtimeExtensionByLanguage.get(normalized)
    const runtimeReady = runtimeEntry?.runtime_status?.ready !== false
    if (!normalized || !runtimeEntry?.installed || !runtimeReady) return

    const effectiveEntry = {
      ...entry,
      server: String(
        normalizeCatalogEntries.find((item) => item.extension_id === normalizeLanguage(runtimeEntry.id || '') && item.language === normalized)?.server
          || entry.server
      ).trim(),
      extension_id: String(runtimeEntry.id || entry.extension_id || '').trim().toLowerCase() || entry.extension_id,
      source: 'runtime',
    }

    entryByLanguage.set(normalized, effectiveEntry)
    LSP_LANGUAGE_ENTRIES_INTERNAL.push(effectiveEntry)
    LSP_SUPPORTED_LANGUAGES_INTERNAL.add(normalized)
  })

  runtimeExtensionByLanguage.forEach((runtimeEntry, language) => {
    const normalized = normalizeLanguage(language)
    if (!normalized || entryByLanguage.has(normalized)) return
    const runtimeReady = runtimeEntry?.runtime_status?.ready !== false
    if (!runtimeEntry?.installed || !runtimeReady) return

    const catalogEntry = normalizeCatalogEntries.find((item) => item.language === normalized)
    const effectiveEntry = catalogEntry
      ? {
          ...catalogEntry,
          extension_id: String(runtimeEntry.id || catalogEntry.extension_id || '').trim().toLowerCase() || catalogEntry.extension_id,
          source: 'runtime',
        }
      : {
          language: normalized,
          server: normalized,
          bin_name: '',
          args: [],
          auto_download: false,
          extension_id: String(runtimeEntry.id || '').trim().toLowerCase() || undefined,
          source: 'runtime',
        }

    entryByLanguage.set(normalized, effectiveEntry)
    LSP_LANGUAGE_ENTRIES_INTERNAL.push(effectiveEntry)
    LSP_SUPPORTED_LANGUAGES_INTERNAL.add(normalized)
  })
}

rebuildLanguageRegistry()

export const LSP_LANGUAGE_ENTRIES: LspLanguageEntry[] = LSP_LANGUAGE_ENTRIES_INTERNAL
export const LSP_SUPPORTED_LANGUAGES = LSP_SUPPORTED_LANGUAGES_INTERNAL

export const setLspRuntimeExtensionSnapshot = (snapshot: RuntimeExtensionState[]) => {
  runtimeExtensionByLanguage.clear()
  ;(Array.isArray(snapshot) ? snapshot : []).forEach((item) => {
    if (!item?.installed) return
    const languages = Array.isArray(item.languages) ? item.languages : []
    languages
      .map((language) => normalizeLanguage(language))
      .filter(Boolean)
      .forEach((language) => {
        runtimeExtensionByLanguage.set(language, item)
      })
  })
  rebuildLanguageRegistry()
  logger.info('[LSP] runtime extension snapshot applied', {
    installedExtensionCount: Array.isArray(snapshot) ? snapshot.filter((item) => item?.installed).length : 0,
    runtimeLanguageCount: runtimeExtensionByLanguage.size,
  })
}

export const refreshLspRuntimeExtensionSnapshot = async () => {
  try {
    const snapshot = await invoke('list_lsp_extensions')
    setLspRuntimeExtensionSnapshot(Array.isArray(snapshot) ? snapshot : [])
    return Array.isArray(snapshot) ? snapshot : []
  } catch (error) {
    logger.warn('[LSP] refresh runtime extension snapshot failed', { error: String(error?.message || error) })
    return []
  }
}

export const getLspLanguageEntry = (languageId: string) =>
  entryByLanguage.get(normalizeLanguage(languageId)) || null

export const getLspServerForLanguage = (languageId: string) =>
  String(getLspLanguageEntry(languageId)?.server || normalizeLanguage(languageId))

export const getLspDocumentSelectorForLanguage = (languageId: string) => {
  const normalizedLanguageId = normalizeLanguage(languageId)
  const entry = getLspLanguageEntry(normalizedLanguageId)
  if (!entry?.server) {
    return normalizedLanguageId ? [{ language: normalizedLanguageId }] : []
  }

  const runtimeEntry = runtimeExtensionByLanguage.get(normalizedLanguageId)
  if (runtimeEntry?.installed) {
    const runtimeLanguages = Array.isArray(runtimeEntry.languages)
      ? runtimeEntry.languages.map((item) => normalizeLanguage(item)).filter(Boolean)
      : []
    if (runtimeLanguages.length) {
      return runtimeLanguages.map((language) => ({ language }))
    }
  }

  return LSP_LANGUAGE_ENTRIES_INTERNAL
    .filter((item) => item.server === entry.server)
    .map((item) => ({ language: item.language }))
}

export const getLspRuntimeExtensionForLanguage = (languageId: string) =>
  runtimeExtensionByLanguage.get(normalizeLanguage(languageId)) || null

export const isLspSupportedLanguage = (languageId: string) =>
  LSP_SUPPORTED_LANGUAGES_INTERNAL.has(normalizeLanguage(languageId))
