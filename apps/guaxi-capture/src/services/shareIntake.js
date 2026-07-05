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

// iOS「快捷指令 + URL Scheme」入口：Shortcuts 把分享的链接以
// guaxi-capture://capture?url=<encoded> 的形式打开本 App，这里统一解析成草稿 payload。
export async function registerDeepLinkListener(onPayload) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlisten = await listen('deep-link://new-url', (event) => {
      const urls = Array.isArray(event.payload) ? event.payload : [event.payload]
      for (const rawUrl of urls) {
        const payload = parseDeepLinkUrl(rawUrl)
        if (payload) {
          console.info('[capture-intake] deep link received', summarizePayloadForLog(payload))
          onPayload(payload)
        }
      }
    })
    console.info('[capture-intake] deep link listener registered')
    return unlisten
  } catch (error) {
    console.info('[capture-intake] deep link listener skipped', error)
    return () => {}
  }
}

export async function readInitialDeepLinkPayload() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const urls = await invoke('plugin:deep-link|get_current')
    const rawUrl = Array.isArray(urls) ? urls[0] : urls
    if (rawUrl) {
      const payload = parseDeepLinkUrl(rawUrl)
      if (payload) {
        console.info('[capture-intake] initial deep link loaded', summarizePayloadForLog(payload))
        return payload
      }
    }
  } catch (error) {
    console.info('[capture-intake] initial deep link skipped', error)
  }
  return null
}

function parseDeepLinkUrl(rawUrl) {
  const value = String(rawUrl || '').trim()
  if (!value) return null

  let params
  try {
    params = new URL(value).searchParams
  } catch {
    // 不是标准 URL：整段当作分享文本解析（兼容只丢一段文字的快捷指令）。
    return parseSharedText(value)
  }

  const payload = {}
  for (const [targetKey, aliases] of Object.entries(URL_FIELD_ALIASES)) {
    const alias = aliases.find((name) => params.has(name))
    if (alias) payload[targetKey] = params.get(alias)
  }
  if (hasMeaningfulCaptureContent(payload)) return payload

  // 没有结构化参数时，把 text/url/link 当作分享文本解析（快捷指令通常只传一个链接）。
  const fallback = params.get('text') || params.get('url') || params.get('link') || ''
  return fallback ? parseSharedText(fallback) : null
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
  // 各来源 App（知乎/公众号/小红书/B站/系统分享）通常只在 EXTRA_TEXT 里给“标题 + URL”。
  // 对于这类链接采集，移动端只保存 sourceUrl 和备注，正文抓取 + Markdown 转换交给 PC 端
  // （build_url_capture_body → flipboard 抓取 + 抽取）。否则 PC 会把这段分享文本当作已经
  // 转换好的 Markdown，收到的文档里就只剩一个 URL，无法再抓取原文内容。
  // 仅当带完整 html 正文时才保留（真正的富文本剪藏），链接类薄文本一律清空交给 PC。
  if (normalized.sourceUrl && !normalized.html && isThinLinkShareBody(normalized.markdown, normalized.sourceUrl)) {
    // 正文留空交给 PC 抓取；description 不塞“标题 + URL”样板文本，让 PC 端用主题标题
    // 作为文件名与描述，避免文件库里出现夹带 URL 的杂乱描述。
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
    text: await readClipboardText(),
  }
}

// 读取剪贴板纯文本：优先 Tauri clipboard-manager 插件（移动端 WebView 的
// navigator.clipboard 常因权限/安全上下文被拦），失败再退回 Web API。
async function readClipboardText() {
  if (window.__TAURI_INTERNALS__) {
    try {
      const { readText } = await import('@tauri-apps/plugin-clipboard-manager')
      const text = await readText()
      // 插件调用成功即以其结果为准（空剪贴板返回空串也算成功），不再回退到
      // Android WebView 会拒绝的 navigator.clipboard，避免误报 “Read permission denied”。
      return typeof text === 'string' ? text : ''
    } catch (error) {
      console.info('[capture-intake] tauri clipboard read failed, fallback to web api', error)
    }
  }

  if (navigator.clipboard?.readText) {
    return await navigator.clipboard.readText()
  }

  throw new Error('当前环境不支持读取剪贴板')
}

function buildDraftMarkdown(payload) {
  if (payload.markdown) return payload.markdown
  if (payload.html) return htmlToMarkdown(payload.html)
  // 链接类分享：正文留空，让 PC 端按 sourceUrl 抓取原文并转 Markdown，
  // 不要再塞“> 来源：URL”样板正文，否则 PC 会当作已转换好的正文而跳过抓取。
  if (payload.sourceUrl) return ''
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

// 判断这段 markdown 是否只是“链接分享的薄文本”（标题 + URL / 短备注），
// 而不是真正的文章正文。命中则清空 markdown，改由 PC 端按 sourceUrl 抓取原文。
function isThinLinkShareBody(markdown = '', sourceUrl = '') {
  const text = String(markdown || '').trim()
  if (!text) return false
  const url = String(sourceUrl || '').trim()
  // 分享面板给的 EXTRA_TEXT 通常包含该 URL 本身。
  if (url && text.includes(url)) return true
  // 没有明显换行的短文本，基本就是标题；正文一般更长且分段。
  const lineCount = text.split(/\r?\n/).filter((line) => line.trim()).length
  return text.length <= 200 && lineCount <= 3
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
