import { createCaptureDraft } from './captureClient.js'
import { htmlToMarkdown, looksLikeHtml } from './htmlToMarkdown.js'

export const NATIVE_CAPTURE_EVENT = 'guaxi-capture-incoming'
export const ANDROID_SHARE_DOM_EVENT = 'guaxi-android-share-intent'
export const ANDROID_SHARE_WINDOW_PAYLOAD = '__GUAXI_CAPTURE_LAST_ANDROID_SHARE__'
let androidSharePluginDisabled = false
let lastAndroidSharePluginLogAt = 0

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

  const sourceApp = normalizeIncomingSourceApp(normalized)
  return createCaptureDraft({
    ...normalized,
    title: normalized.title || buildFallbackTitle(normalized),
    markdown: buildDraftMarkdown(normalized),
    sourceApp,
    tagsText: normalizeTagsText(normalized.tagsText, sourceApp),
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
  if (!window.__TAURI_INTERNALS__ || androidSharePluginDisabled) return null

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const payload = await invoke('plugin:guaxi-share-intent|readPendingShare')
    if (hasMeaningfulCaptureContent(payload || {})) {
      console.info('[capture-intake] android share payload loaded', summarizePayloadForLog(payload))
      return payload
    }
  } catch (error) {
    // 设计说明：
    // Android WebView 首屏初始化时，前端可能早于原生插件完成注册。不能因为第一次 invoke
    // 失败就永久停掉轮询，否则系统分享已经唤起 App，但 payload 永远不会被读到。
    if (!isProbablyAndroidRuntime()) {
      androidSharePluginDisabled = true
    }
    logAndroidSharePluginIssue(error)
  }

  return null
}

export function registerAndroidShareDomListener(onPayload) {
  const listener = (event) => {
    const payload = event.detail || {}
    console.info('[capture-intake] android share dom event received', summarizePayloadForLog(payload))
    delete window[ANDROID_SHARE_WINDOW_PAYLOAD]
    onPayload(payload)
  }
  window.addEventListener(ANDROID_SHARE_DOM_EVENT, listener)
  console.info('[capture-intake] android share dom listener registered', {
    event: ANDROID_SHARE_DOM_EVENT,
  })
  return () => window.removeEventListener(ANDROID_SHARE_DOM_EVENT, listener)
}

export function readLastAndroidShareDomPayload() {
  const payload = window[ANDROID_SHARE_WINDOW_PAYLOAD]
  if (hasMeaningfulCaptureContent(payload || {})) {
    delete window[ANDROID_SHARE_WINDOW_PAYLOAD]
    console.info('[capture-intake] android share window payload loaded', summarizePayloadForLog(payload))
    return payload
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

  const textForUrl = [
    normalized.sourceUrl,
    normalized.markdown,
    normalized.description,
    payload.sharedText,
    payload.text,
    payload.url,
    payload.link,
  ].filter(Boolean).join('\n')
  if (!normalized.sourceUrl) {
    normalized.sourceUrl = extractFirstUrl(textForUrl)
  }
  if (!normalized.title) {
    normalized.title = extractTitleFromSharedText(textForUrl, normalized.sourceUrl)
  }

  // 设计说明：
  // Android 的系统分享经常把“标题 + URL”放在 EXTRA_TEXT。对于文章链接采集，
  // 移动端应只保存 sourceUrl 和备注，正文转换交给 PC 端完成；否则 PC 会把这段
  // 分享文本当作已经转换好的 Markdown，无法再抓取知乎/公众号等原文内容。
  if (isAndroidSharePayload(payload) && normalized.sourceUrl && normalized.markdown) {
    normalized.description = normalized.description || normalized.markdown
    normalized.markdown = ''
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
      || String(payload.sourceUrl || payload.source_url || payload.url || payload.link || '').trim()
      || String(payload.markdown || payload.text || payload.sharedText || payload.content || payload.html || '').trim()
      || (Array.isArray(payload.assets) && payload.assets.length > 0),
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

  const firstUrl = extractFirstUrl(trimmed)
  const title = extractTitleFromSharedText(trimmed, firstUrl)

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

function normalizeIncomingSourceApp(payload) {
  const explicit = normalizeSourceApp(payload.sourceApp)
  const inferred = inferSourceApp([
    payload.sourceUrl,
    payload.markdown,
    payload.description,
  ].filter(Boolean).join('\n'))

  if ((!payload.sourceApp || explicit === 'manual' || explicit === 'android-share') && inferred) {
    return inferred
  }
  return explicit
}

function isAndroidSharePayload(payload = {}) {
  return String(payload.sourceApp || payload.source_app || payload.source || payload.app || payload.from || '')
    .trim()
    .toLowerCase() === 'android-share'
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

function extractFirstUrl(text = '') {
  return String(text || '').match(/https?:\/\/\S+/i)?.[0]?.replace(/[),，。；;]+$/, '') || ''
}

function extractTitleFromSharedText(text = '', sourceUrl = '') {
  return String(text || '')
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line && line !== sourceUrl && !/^https?:\/\//i.test(line)) || ''
}

function isProbablyAndroidRuntime() {
  return /Android/i.test(navigator.userAgent || '')
}

function logAndroidSharePluginIssue(error) {
  const now = Date.now()
  if (now - lastAndroidSharePluginLogAt < 30000) return
  lastAndroidSharePluginLogAt = now
  console.info('[capture-intake] android share plugin pending/unavailable', {
    android: isProbablyAndroidRuntime(),
    error: stringifyError(error),
  })
}

function summarizePayloadForLog(payload = {}) {
  return {
    title: payload.title || payload.name || '',
    sourceUrl: payload.sourceUrl || payload.source_url || payload.url || payload.link || '',
    hasText: Boolean(payload.text || payload.sharedText || payload.markdown || payload.content),
    hasHtml: Boolean(payload.html || payload.sharedHtml),
    assetCount: Array.isArray(payload.assets) ? payload.assets.length : 0,
  }
}

function stringifyError(error) {
  if (!error) return ''
  if (typeof error === 'string') return error
  if (error.message) return error.message
  try {
    return JSON.stringify(error)
  } catch {
    return String(error)
  }
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
