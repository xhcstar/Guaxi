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

export const LSP_LANGUAGE_ENTRIES: LspLanguageEntry[] = normalizedEntries

export const LSP_SUPPORTED_LANGUAGES = new Set(
  normalizedEntries.map((entry) => entry.language)
)

export const isLspSupportedLanguage = (languageId: string) =>
  LSP_SUPPORTED_LANGUAGES.has(String(languageId || '').trim().toLowerCase())
