import * as monaco from 'monaco-editor'
import { createModelReference } from '@codingame/monaco-vscode-api/monaco'
import {
  FileSystemProviderCapabilities,
  FileSystemProviderError,
  FileSystemProviderErrorCode,
  FileType,
  registerFileSystemOverlay,
} from '@codingame/monaco-vscode-files-service-override'
import { createLogger } from '../utils/logger'

const logger = createLogger('vscodeModelBridge')
const textEncoder = new TextEncoder()
const vscodeModelBridgeEntries = new Map()

let overlayProvider = null
let overlayDisposable = null

const normalizeOwner = (owner) => String(owner || 'global').trim() || 'global'

const normalizeResourceUri = (resource) => {
  if (!resource) return null

  try {
    if (resource instanceof monaco.Uri) {
      return resource
    }
  } catch {
    // 某些热更新场景下可能跨上下文，这里继续走字符串兜底。
  }

  const raw = String(resource?.toString?.() || resource || '').trim()
  if (!raw) return null

  try {
    return monaco.Uri.parse(raw)
  } catch (error) {
    logger.warn('[VSCodeModelBridge] normalize resource uri failed', {
      resource: raw,
      error,
    })
    return null
  }
}

const createFileNotFoundError = (uri) => new FileSystemProviderError(
  `file not found: ${String(uri?.toString?.() || uri || '')}`,
  FileSystemProviderErrorCode.FileNotFound
)

const splitPathSegments = (uri) => String(uri?.path || '/').split('/').filter(Boolean)

const joinSegmentsToPath = (segments) => `/${segments.join('/')}`

const createOverlayProvider = () => {
  const files = new Map()
  const directories = new Map()
  const noopEvent = () => ({ dispose() {} })

  const ensureDirectory = (uri) => {
    const normalized = normalizeResourceUri(uri)
    if (!normalized) return

    const key = normalized.toString()
    if (!directories.has(key)) {
      directories.set(key, {
        ctime: Date.now(),
        mtime: Date.now(),
      })
    }
  }

  const ensureAncestorDirectories = (uri) => {
    const normalized = normalizeResourceUri(uri)
    if (!normalized) return

    const segments = splitPathSegments(normalized)
    ensureDirectory(normalized.with({ path: '/' }))
    for (let i = 0; i < segments.length - 1; i += 1) {
      ensureDirectory(normalized.with({ path: joinSegmentsToPath(segments.slice(0, i + 1)) }))
    }
  }

  const provider = {
    capabilities:
      FileSystemProviderCapabilities.FileReadWrite |
      FileSystemProviderCapabilities.PathCaseSensitive,
    onDidChangeCapabilities: noopEvent,
    onDidChangeFile: noopEvent,
    watch() {
      return { dispose() {} }
    },
    async stat(resource) {
      const uri = normalizeResourceUri(resource)
      if (!uri) throw createFileNotFoundError(resource)

      const fileEntry = files.get(uri.toString())
      if (fileEntry) {
        return {
          type: FileType.File,
          ctime: fileEntry.ctime,
          mtime: fileEntry.mtime,
          size: fileEntry.content.length,
        }
      }

      const directoryEntry = directories.get(uri.toString())
      if (directoryEntry) {
        return {
          type: FileType.Directory,
          ctime: directoryEntry.ctime,
          mtime: directoryEntry.mtime,
          size: 0,
        }
      }

      throw createFileNotFoundError(uri)
    },
    async readFile(resource) {
      const uri = normalizeResourceUri(resource)
      const entry = uri ? files.get(uri.toString()) : null
      if (!entry) throw createFileNotFoundError(resource)
      return entry.content
    },
    async writeFile(resource, content) {
      const uri = normalizeResourceUri(resource)
      if (!uri) throw createFileNotFoundError(resource)

      ensureAncestorDirectories(uri)
      files.set(uri.toString(), {
        content: content instanceof Uint8Array ? content : new Uint8Array(content),
        ctime: files.get(uri.toString())?.ctime || Date.now(),
        mtime: Date.now(),
      })
    },
    async mkdir(resource) {
      const uri = normalizeResourceUri(resource)
      if (!uri) throw createFileNotFoundError(resource)
      ensureDirectory(uri)
    },
    async readdir(resource) {
      const uri = normalizeResourceUri(resource)
      if (!uri) throw createFileNotFoundError(resource)

      const targetPath = String(uri.path || '/').replace(/[\\/]+$/, '') || '/'
      const children = new Map()

      for (const key of directories.keys()) {
        const childUri = monaco.Uri.parse(key)
        const childPath = String(childUri.path || '/').replace(/[\\/]+$/, '') || '/'
        if (!childPath.startsWith(targetPath === '/' ? '/' : `${targetPath}/`)) continue
        const relative = childPath.slice(targetPath === '/' ? 1 : targetPath.length + 1)
        if (!relative || relative.includes('/')) continue
        children.set(relative, FileType.Directory)
      }

      for (const key of files.keys()) {
        const childUri = monaco.Uri.parse(key)
        const childPath = String(childUri.path || '/')
        if (!childPath.startsWith(targetPath === '/' ? '/' : `${targetPath}/`)) continue
        const relative = childPath.slice(targetPath === '/' ? 1 : targetPath.length + 1)
        if (!relative || relative.includes('/')) continue
        children.set(relative, FileType.File)
      }

      return Array.from(children.entries())
    },
    async delete(resource) {
      const uri = normalizeResourceUri(resource)
      if (!uri) throw createFileNotFoundError(resource)

      const target = uri.toString()
      files.delete(target)
      directories.delete(target)
    },
    async rename(from, to) {
      const fromUri = normalizeResourceUri(from)
      const toUri = normalizeResourceUri(to)
      if (!fromUri || !toUri) throw createFileNotFoundError(from)

      const fileEntry = files.get(fromUri.toString())
      if (!fileEntry) throw createFileNotFoundError(fromUri)

      ensureAncestorDirectories(toUri)
      files.set(toUri.toString(), {
        ...fileEntry,
        mtime: Date.now(),
      })
      files.delete(fromUri.toString())
    },
    ensureFile(resource, content = '') {
      const uri = normalizeResourceUri(resource)
      if (!uri) return null

      ensureAncestorDirectories(uri)
      const encoded = textEncoder.encode(String(content ?? ''))
      const existing = files.get(uri.toString())
      if (!existing) {
        files.set(uri.toString(), {
          content: encoded,
          ctime: Date.now(),
          mtime: Date.now(),
        })
      } else if (
        existing.content.length !== encoded.length ||
        existing.content.some((value, index) => value !== encoded[index])
      ) {
        existing.content = encoded
        existing.mtime = Date.now()
      }

      return uri
    },
    removeFile(resource) {
      const uri = normalizeResourceUri(resource)
      if (!uri) return
      files.delete(uri.toString())
    },
  }

  return provider
}

const ensureOverlayProviderRegistered = () => {
  if (overlayProvider && overlayDisposable) {
    return overlayProvider
  }

  // 设计说明：
  // 1. model-service-override 在解析 `file://` 文档时，会真的走 file service 去读取该 URI。
  // 2. 如果直接读真实磁盘文件，Notebook cell / Markdown fence 这类虚拟文档一定会失败。
  // 3. 这里直接对原始 `file://` URI 注册一个高优先级内存 overlay，让后续所有 read/stat 都命中内存内容。
  overlayProvider = createOverlayProvider()
  overlayDisposable = registerFileSystemOverlay(1, overlayProvider)
  logger.info('[VSCodeModelBridge] registered direct file-uri overlay provider')
  return overlayProvider
}

const getEntryModel = (entry) => {
  const model = entry?.ref?.object?.textEditorModel || null
  if (!model || model.isDisposed?.()) return null
  return model
}

const getMonacoBackedModel = (uri) => {
  if (!uri) return null
  const model = monaco?.editor?.getModel?.(uri) || null
  if (!model || model.isDisposed?.()) return null
  return model
}

export const replaceModelContentPreserveUndo = (
  model,
  content,
  {
    logPrefix = '[VSCodeModelBridge] model content sync',
    uri = '',
    owner = 'global',
    reason = 'manual',
  } = {}
) => {
  if (!model || model.isDisposed?.()) return model

  const nextContent = String(content ?? '')
  const currentContent = String(model.getValue?.() || '')
  if (currentContent === nextContent) return model

  const fullRange = typeof model.getFullModelRange === 'function'
    ? model.getFullModelRange()
    : null
  if (!fullRange) {
    model.setValue(nextContent)
    logger.debug(`${logPrefix} fallback to setValue`, {
      uri,
      owner,
      reason,
      length: nextContent.length,
    })
    return model
  }

  try {
    if (typeof model.pushEditOperations === 'function') {
      model.pushEditOperations([], [{
        range: fullRange,
        text: nextContent,
        forceMoveMarkers: true,
      }], () => null)
      logger.debug(`${logPrefix} via pushEditOperations`, {
        uri,
        owner,
        reason,
        length: nextContent.length,
      })
      return model
    }
  } catch (error) {
    logger.warn(`${logPrefix} pushEditOperations failed, fallback to setValue`, {
      uri,
      owner,
      reason,
      length: nextContent.length,
      error,
    })
  }

  model.setValue(nextContent)
  logger.debug(`${logPrefix} fallback to setValue`, {
    uri,
    owner,
    reason,
    length: nextContent.length,
  })
  return model
}

const applyLanguageId = (entry, languageId, reason, owner) => {
  const nextLanguageId = String(languageId || '').trim()
  const model = getEntryModel(entry)
  if (!model || !nextLanguageId) return model

  if (String(model.getLanguageId?.() || '').trim() === nextLanguageId) {
    return model
  }

  try {
    if (typeof entry?.ref?.object?.setLanguageId === 'function') {
      entry.ref.object.setLanguageId(nextLanguageId)
    } else {
      monaco.editor.setModelLanguage(model, nextLanguageId)
    }
    logger.info('[VSCodeModelBridge] language id synchronized', {
      uri: entry.uri.toString(),
      owner,
      reason,
      languageId: nextLanguageId,
    })
  } catch (error) {
    logger.warn('[VSCodeModelBridge] failed to synchronize language id', {
      uri: entry.uri.toString(),
      owner,
      reason,
      languageId: nextLanguageId,
      error,
    })
  }

  return getEntryModel(entry)
}

const ensureMonacoTextModel = (uri, content, languageId, reason, owner) => {
  const nextContent = String(content ?? '')
  const nextLanguageId = String(languageId || '').trim()
  let model = monaco.editor.getModel(uri)

  if (!model) {
    model = monaco.editor.createModel(nextContent, nextLanguageId || undefined, uri)
    logger.info('[VSCodeModelBridge] created Monaco text model before retaining reference', {
      uri: uri.toString(),
      owner,
      reason,
      languageId: nextLanguageId,
      length: nextContent.length,
    })
    return model
  }

  if (nextLanguageId && model.getLanguageId?.() !== nextLanguageId) {
    monaco.editor.setModelLanguage(model, nextLanguageId)
  }
  replaceModelContentPreserveUndo(model, nextContent, {
    logPrefix: '[VSCodeModelBridge] ensure Monaco text model',
    uri: uri.toString(),
    owner,
    reason,
  })
  return model
}

const syncModelContent = (entry, content, reason, owner) => {
  const model = getEntryModel(entry)
  if (!model) return null

  const nextContent = String(content ?? '')
  if (model.getValue() !== nextContent) {
    replaceModelContentPreserveUndo(model, nextContent, {
      logPrefix: '[VSCodeModelBridge] model content synchronized',
      uri: entry.uri.toString(),
      owner,
      reason,
    })
  }

  overlayProvider?.ensureFile?.(entry.uri, nextContent)
  return model
}

export const retainVsCodeTextModel = async ({
  resource,
  content = '',
  languageId = '',
  owner = 'global',
  reason = 'manual',
} = {}) => {
  const uri = normalizeResourceUri(resource)
  if (!uri) return null

  const ownerKey = normalizeOwner(owner)
  const uriKey = uri.toString()
  let entry = vscodeModelBridgeEntries.get(uriKey)

  if (entry && !getEntryModel(entry)) {
    try {
      entry.ref?.dispose?.()
    } catch {
      // ignore
    }
    vscodeModelBridgeEntries.delete(uriKey)
    entry = null
  }

  try {
    ensureOverlayProviderRegistered()
    overlayProvider.ensureFile(uri, content)
    ensureMonacoTextModel(uri, content, languageId, reason, ownerKey)

    if (!entry) {
      const ref = await createModelReference(uri)
      entry = {
        uri,
        ref,
        owners: new Set(),
      }
      vscodeModelBridgeEntries.set(uriKey, entry)
      logger.info('[VSCodeModelBridge] retained new model reference', {
        uri: uriKey,
        owner: ownerKey,
        reason,
        languageId: String(languageId || ''),
      })
    }
  } catch (error) {
    logger.warn('[VSCodeModelBridge] retain model reference failed', {
      uri: uriKey,
      owner: ownerKey,
      reason,
      languageId: String(languageId || ''),
      storageUri: uriKey,
      error,
    })
    return null
  }

  entry.owners.add(ownerKey)
  applyLanguageId(entry, languageId, reason, ownerKey)
  const bridgeModel = syncModelContent(entry, content, reason, ownerKey) || getEntryModel(entry)
  const monacoBackedModel = getMonacoBackedModel(uri)

  // 设计说明：
  // 1. `createModelReference(...).object.textEditorModel` 在 monaco-vscode 体系里可能是一个 projected model，
  //    它适合给 VSCode 文档服务/LSP 使用，但直接挂到 standalone Monaco editor 上会触发视图层异常。
  // 2. Notebook/Explorer 场景里的编辑器本体，应该优先绑定 `monaco.editor.getModel(uri)` 返回的真实 Monaco text model；
  //    同时继续保留 VSCode reference，确保补全、hover、命令调用仍然命中同一个 URI。
  // 3. 这里保底再把 Monaco text model 的内容和语言同步一次，避免 bridge model / editor model 在边界场景下短暂漂移。
  if (monacoBackedModel) {
    const nextLanguageId = String(languageId || '').trim()
    if (nextLanguageId && monacoBackedModel.getLanguageId?.() !== nextLanguageId) {
      monaco.editor.setModelLanguage(monacoBackedModel, nextLanguageId)
      logger.info('[VSCodeModelBridge] synchronized Monaco-backed model language', {
        uri: uriKey,
        owner: ownerKey,
        reason,
        languageId: nextLanguageId,
      })
    }
    replaceModelContentPreserveUndo(monacoBackedModel, content, {
      logPrefix: '[VSCodeModelBridge] keep Monaco-backed model synchronized',
      uri: uriKey,
      owner: ownerKey,
      reason,
    })
    if (bridgeModel && bridgeModel !== monacoBackedModel) {
      logger.info('[VSCodeModelBridge] preferred Monaco-backed model for editor attachment', {
        uri: uriKey,
        owner: ownerKey,
        reason,
        bridgeModelType: String(bridgeModel?.constructor?.name || 'unknown'),
        monacoModelType: String(monacoBackedModel?.constructor?.name || 'unknown'),
      })
    }
    return monacoBackedModel
  }

  logger.warn('[VSCodeModelBridge] Monaco-backed model missing after retain, fallback to bridge model', {
    uri: uriKey,
    owner: ownerKey,
    reason,
    bridgeModelType: String(bridgeModel?.constructor?.name || 'unknown'),
  })
  return bridgeModel
}

export const releaseVsCodeTextModel = (resource, owner = 'global', reason = 'manual') => {
  const uri = normalizeResourceUri(resource)
  if (!uri) return

  const ownerKey = normalizeOwner(owner)
  const uriKey = uri.toString()
  const entry = vscodeModelBridgeEntries.get(uriKey)
  if (!entry) return

  entry.owners.delete(ownerKey)
  if (entry.owners.size > 0) {
    logger.debug('[VSCodeModelBridge] owner released but model reference kept alive', {
      uri: uriKey,
      owner: ownerKey,
      reason,
      remainingOwners: Array.from(entry.owners),
    })
    return
  }

  try {
    entry.ref?.dispose?.()
  } catch (error) {
    logger.warn('[VSCodeModelBridge] dispose model reference failed', {
      uri: uriKey,
      owner: ownerKey,
      reason,
      error,
    })
  } finally {
    overlayProvider?.removeFile?.(uri)
    vscodeModelBridgeEntries.delete(uriKey)
  }

  logger.info('[VSCodeModelBridge] disposed model reference', {
    uri: uriKey,
    owner: ownerKey,
    reason,
  })
}

export const disposeAllVsCodeTextModelsForOwner = (owner = 'global', reason = 'manual') => {
  const ownerKey = normalizeOwner(owner)
  Array.from(vscodeModelBridgeEntries.values()).forEach((entry) => {
    if (!entry?.owners?.has(ownerKey)) return
    releaseVsCodeTextModel(entry.uri, ownerKey, reason)
  })
}
