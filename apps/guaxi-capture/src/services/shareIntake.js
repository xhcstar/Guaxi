import { createCaptureDraft } from './captureClient.js'
import { htmlToMarkdown, looksLikeHtml } from './htmlToMarkdown.js'

export const NATIVE_CAPTURE_EVENT = 'guaxi-capture-incoming'
let androidSharePluginAvailable = true

const URL_FIELD_ALIASES = {
  title: ['title', 'name'],
  sourceUrl: ['sourceUrl', 'source_url', 'url', 'link'],
  sourceApp: ['sourceApp', 'source_app', 'source', 'app', 'from'],
  description: ['description', 'desc', 'summary'],
  knowledgeBase: ['knowledgeBase', 'knowledge_base', 'kb'],
  fileRepo: ['fileRepo', 'file_repo', 'repo', 'library'],
  tagsText: ['tagsText', 'tags', 'tag'],
  markdown: ['markdown', 'md', 'content', 'text', 'body'],
  html: ['html', 'articleHtml', 'article_html', 'selectionHtml', 'selection_html'],
}

const KNOWN_SOURCE_MAP = new Map([
  ['wechat', 'wechat'],
  ['weixin', 'wechat'],
  ['公众号', 'wechat'],
  ['zhihu', 'zhihu'],
  ['知乎', 'zhihu'],
  ['xiaohongshu', 'xiaohongshu'],
  ['小红书', 'xiaohongshu'],
  ['bilibili', 'bilibili'],
  ['b站', 'bilibili'],
  ['browser', 'browser'],
  ['camera', 'camera'],
  ['ocr', 'camera'],
  ['clipboard', 'clipboard'],
])

// 设计说明：
// Capture App 后续会接浏览器扩展、移动端 Share Intent、桌面深链和剪贴板。
// 这些入口的字段命名不一致，所以这里先收敛成统一草稿结构，页面层只关心 draft。
export function createDraftFromIncomingPayload(payload = {}) {
  const normalized = normalizeIncomingPayload(payload)
  if (!hasMeaningfulCaptureContent(normalized)) {
    console.info('[capture-intake] incoming payload skipped: empty or invalid')
    return null
  }

  return createCaptureDraft({
    ...normalized,
    title: normalized.title || buildFallbackTitle(normalized),
    markdown: buildDraftMarkdown(normalized),
    sourceApp: normalizeSourceApp(normalized.sourceApp),
    tagsText: normalizeTagsText(normalized.tagsText, normalized.sourceApp),
  })
}

export function readInitialSharePayload(location = window.location) {
  const payload = {
    ...paramsToPayload(new URLSearchParams(location.search || '')),
    ...hashToPayload(location.hash || ''),
  }

  if (hasMeaningfulCaptureContent(payload)) {
    console.info('[capture-intake] initial payload detected', {
      title: payload.title,
      sourceApp: payload.sourceApp,
      hasMarkdown: Boolean(payload.markdown),
      sourceUrl: payload.sourceUrl,
    })
    return payload
  }

  return null
}

export async function readClipboardPayload() {
  if (!navigator.clipboard?.read && !navigator.clipboard?.readText) {
    throw new Error('当前环境不支持读取剪贴板')
  }

  const clipboard = await readClipboardHtmlOrText()
  const payload = clipboard.html
    ? { html: clipboard.html, sourceApp: 'clipboard' }
    : parseSharedText(clipboard.text)

  console.info('[capture-intake] clipboard payload parsed', {
    hasHtml: Boolean(clipboard.html),
    hasText: Boolean(clipboard.text),
    sourceUrl: payload.sourceUrl,
  })
  return payload
}

export async function registerNativeCaptureListener(onPayload) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen(NATIVE_CAPTURE_EVENT, (event) => {
      console.info('[capture-intake] native payload received', {
        event: NATIVE_CAPTURE_EVENT,
      })
      onPayload(event.payload || {})
    })
    console.info('[capture-intake] native listener registered', {
      event: NATIVE_CAPTURE_EVENT,
    })
    return unlisten
  } catch (error) {
    console.info('[capture-intake] native listener skipped', error)
    return () => {}
  }
}

export async function readStartupNativePayload() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const payload = await invoke('read_startup_capture_payload')
    if (hasMeaningfulCaptureContent(payload || {})) {
      console.info('[capture-intake] startup native payload loaded')
      return payload
    }
  } catch (error) {
    console.info('[capture-intake] startup native payload skipped', error)
  }
  return null
}

export async function readAndroidSharePayload() {
  if (!window.__TAURI_INTERNALS__ || !androidSharePluginAvailable) return null

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const payload = await invoke('plugin:guaxi-share-intent|readPendingShare')
    if (hasMeaningfulCaptureContent(payload || {})) {
      console.info('[capture-intake] android share payload loaded')
      return payload
    }
  } catch (error) {
    androidSharePluginAvailable = false
    console.info('[capture-intake] android share plugin skipped', error)
  }

  return null
}

export function normalizeIncomingPayload(payload = {}) {
  if (payload == null) {
    console.info('[capture-intake] normalize skipped null payload')
    return {}
  }

  if (typeof payload === 'string') {
    return parseSharedText(payload)
  }

  if (typeof payload !== 'object') {
    console.info('[capture-intake] normalize skipped non-object payload', {
      payloadType: typeof payload,
    })
    return {}
  }

  const normalized = {}
  for (const [targetKey, aliases] of Object.entries(URL_FIELD_ALIASES)) {
    const value = firstPresent(payload, aliases)
    if (value !== undefined && value !== null) {
      normalized[targetKey] = String(value).trim()
    }
  }

  if (!normalized.markdown && typeof payload.sharedText === 'string') {
    normalized.markdown = payload.sharedText.trim()
  }

  if (!normalized.html && typeof payload.sharedHtml === 'string') {
    normalized.html = payload.sharedHtml.trim()
  }

  if (Array.isArray(payload.assets)) {
    normalized.assets = payload.assets
  }

  if (typeof payload.createdAt === 'string') {
    normalized.createdAt = payload.createdAt
  }

  if (normalized.html) {
    normalized.markdown = htmlToMarkdown(normalized.html)
  } else if (normalized.markdown && looksLikeHtml(normalized.markdown)) {
    normalized.html = normalized.markdown
    normalized.markdown = htmlToMarkdown(normalized.html)
  }

  return normalized
}

export function hasMeaningfulCaptureContent(payload = {}) {
  return Boolean(
    String(payload.title || '').trim()
      || String(payload.sourceUrl || '').trim()
      || String(payload.markdown || payload.text || payload.content || payload.html || '').trim(),
  )
}

function paramsToPayload(params) {
  const payload = {}
  for (const [targetKey, aliases] of Object.entries(URL_FIELD_ALIASES)) {
    const alias = aliases.find((name) => params.has(name))
    if (alias) {
      payload[targetKey] = params.get(alias)
    }
  }
  return payload
}

function hashToPayload(hash) {
  if (!hash) return {}
  const value = hash.startsWith('#') ? hash.slice(1) : hash
  const query = value.startsWith('?') ? value.slice(1) : value
  if (!query.includes('=')) return {}
  return paramsToPayload(new URLSearchParams(query))
}

function parseSharedText(text = '') {
  const trimmed = String(text).trim()
  if (!trimmed) return {}

  const jsonPayload = tryParseJson(trimmed)
  if (jsonPayload) return normalizeIncomingPayload(jsonPayload)

  if (looksLikeHtml(trimmed)) {
    return {
      sourceApp: 'clipboard',
      html: trimmed,
      markdown: htmlToMarkdown(trimmed),
    }
  }

  const firstUrl = trimmed.match(/https?:\/\/\S+/i)?.[0] || ''
  const title = trimmed
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line && line !== firstUrl) || ''

  return {
    title,
    sourceUrl: firstUrl,
    sourceApp: inferSourceApp(firstUrl),
    markdown: trimmed,
  }
}

async function readClipboardHtmlOrText() {
  if (navigator.clipboard?.read) {
    try {
      const items = await navigator.clipboard.read()
      for (const item of items) {
        if (item.types.includes('text/html')) {
          const blob = await item.getType('text/html')
          return {
            html: await blob.text(),
            text: '',
          }
        }
      }

      for (const item of items) {
        if (item.types.includes('text/plain')) {
          const blob = await item.getType('text/plain')
          return {
            html: '',
            text: await blob.text(),
          }
        }
      }
    } catch (error) {
      console.info('[capture-intake] rich clipboard skipped', error)
    }
  }

  return {
    html: '',
    text: await navigator.clipboard.readText(),
  }
}

function buildDraftMarkdown(payload) {
  if (payload.markdown) return payload.markdown
  if (payload.html) return htmlToMarkdown(payload.html)
  return buildMarkdownFromSharedText(payload)
}

function firstPresent(payload, aliases) {
  for (const alias of aliases) {
    if (Object.prototype.hasOwnProperty.call(payload, alias)) {
      return payload[alias]
    }
  }
  return undefined
}

function normalizeSourceApp(sourceApp = '') {
  const key = String(sourceApp || '').trim().toLowerCase()
  return KNOWN_SOURCE_MAP.get(key) || inferSourceApp(key) || key || 'manual'
}

function inferSourceApp(value = '') {
  const lower = String(value).toLowerCase()
  if (lower.includes('weixin.qq.com') || lower.includes('mp.weixin.qq.com')) return 'wechat'
  if (lower.includes('zhihu.com')) return 'zhihu'
  if (lower.includes('xiaohongshu.com') || lower.includes('xhslink.com')) return 'xiaohongshu'
  if (lower.includes('bilibili.com') || lower.includes('b23.tv')) return 'bilibili'
  return ''
}

function normalizeTagsText(tagsText = '', sourceApp = '') {
  const source = normalizeSourceApp(sourceApp)
  const tags = String(tagsText || '')
    .split(',')
    .map((tag) => tag.trim())
    .filter(Boolean)

  if (!tags.includes('capture')) tags.unshift('capture')
  if (source && source !== 'manual' && !tags.includes(source)) tags.push(source)
  return [...new Set(tags)].join(',')
}

function buildFallbackTitle(payload) {
  if (payload.sourceUrl) {
    try {
      const url = new URL(payload.sourceUrl)
      return url.hostname.replace(/^www\./, '') || '外部分享'
    } catch {
      return '外部分享'
    }
  }
  return '外部分享'
}

function buildMarkdownFromSharedText(payload) {
  const lines = []
  if (payload.sourceUrl) {
    lines.push(`> 来源：${payload.sourceUrl}`)
    lines.push('')
  }
  if (payload.description) {
    lines.push(payload.description)
    lines.push('')
  }
  return lines.join('\n')
}

function tryParseJson(value) {
  try {
    const parsed = JSON.parse(value)
    return parsed && typeof parsed === 'object' ? parsed : null
  } catch {
    return null
  }
}
