import lspLanguageEntries from '../../shared/lsp_languages.json'

export type LspLanguageEntry = {
  language: string
  server: string
  bin_name: string
  args: string[]
  auto_download: boolean
}

const normalizedEntries = (Array.isArray(lspLanguageEntries) ? lspLanguageEntries : [])
  .map((entry) => ({
    language: String(entry?.language || '').trim().toLowerCase(),
    server: String(entry?.server || '').trim(),
    bin_name: String(entry?.bin_name || '').trim(),
    args: Array.isArray(entry?.args) ? entry.args.map((item) => String(item || '')) : [],
    auto_download: !!entry?.auto_download,
  }))
  .filter((entry) => !!entry.language)

const entryByLanguage = new Map(
  normalizedEntries.map((entry) => [entry.language, entry] as const)
)

export const LSP_LANGUAGE_ENTRIES: LspLanguageEntry[] = normalizedEntries

export const LSP_SUPPORTED_LANGUAGES = new Set(
  normalizedEntries.map((entry) => entry.language)
)

export const getLspLanguageEntry = (languageId: string) =>
  entryByLanguage.get(String(languageId || '').trim().toLowerCase()) || null

export const getLspServerForLanguage = (languageId: string) =>
  String(getLspLanguageEntry(languageId)?.server || String(languageId || '').trim().toLowerCase())

export const getLspDocumentSelectorForLanguage = (languageId: string) => {
  const entry = getLspLanguageEntry(languageId)
  if (!entry?.server) {
    const lang = String(languageId || '').trim().toLowerCase()
    return lang ? [{ language: lang }] : []
  }

  return normalizedEntries
    .filter((item) => item.server === entry.server)
    .map((item) => ({ language: item.language }))
}

export const isLspSupportedLanguage = (languageId: string) =>
  LSP_SUPPORTED_LANGUAGES.has(String(languageId || '').trim().toLowerCase())
