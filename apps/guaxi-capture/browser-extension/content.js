const SOURCE_RULES = [
  [/mp\.weixin\.qq\.com|weixin\.qq\.com/i, 'wechat'],
  [/zhihu\.com/i, 'zhihu'],
  [/xiaohongshu\.com|xhslink\.com/i, 'xiaohongshu'],
  [/bilibili\.com|b23\.tv/i, 'bilibili'],
]

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== 'GUAXI_CAPTURE_EXTRACT') return false

  try {
    const payload = extractGuaxiCapturePayload(message.mode || 'article')
    console.info('[guaxi-extension] extracted payload', {
      title: payload.title,
      sourceApp: payload.sourceApp,
      htmlLength: payload.html.length,
    })
    sendResponse({ success: true, payload })
  } catch (error) {
    console.warn('[guaxi-extension] extract failed', error)
    sendResponse({ success: false, message: error.message })
  }

  return false
})

// 设计说明：
// 扩展只负责尽量提取结构化 HTML，不在插件里做 Markdown 转换。
// Capture App 内已有统一 HTML -> Markdown 管线，插件、剪贴板、移动分享都复用那一套。
function extractGuaxiCapturePayload(mode) {
  const selectionHtml = mode === 'selection' ? getSelectionHtml() : ''
  const articleNode = pickArticleNode()
  const html = selectionHtml || cleanHtml(articleNode?.innerHTML || document.body.innerHTML)
  const title = getTitle(articleNode)
  const sourceUrl = location.href

  return {
    title,
    sourceUrl,
    sourceApp: inferSourceApp(sourceUrl),
    description: getDescription(),
    html,
    tags: ['capture', 'browser', inferSourceApp(sourceUrl)].filter(Boolean).join(','),
  }
}

function pickArticleNode() {
  return document.querySelector('article')
    || document.querySelector('main')
    || document.querySelector('[role="main"]')
    || document.querySelector('.article')
    || document.querySelector('.content')
    || document.body
}

function getSelectionHtml() {
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return ''

  const container = document.createElement('div')
  for (let index = 0; index < selection.rangeCount; index += 1) {
    container.appendChild(selection.getRangeAt(index).cloneContents())
  }
  return cleanHtml(container.innerHTML)
}

function cleanHtml(html) {
  const template = document.createElement('template')
  template.innerHTML = html
  template.content
    .querySelectorAll('script,style,noscript,template,iframe,canvas,svg')
    .forEach((node) => node.remove())

  template.content.querySelectorAll('*').forEach((node) => {
    ;[...node.attributes].forEach((attribute) => {
      const name = attribute.name.toLowerCase()
      if (name.startsWith('on') || ['style', 'class', 'id'].includes(name)) {
        node.removeAttribute(attribute.name)
      }
    })
  })

  return template.innerHTML.trim()
}

function getTitle(articleNode) {
  return document.querySelector('meta[property="og:title"]')?.content
    || articleNode?.querySelector('h1')?.textContent?.trim()
    || document.title
    || '网页采集'
}

function getDescription() {
  return document.querySelector('meta[name="description"]')?.content
    || document.querySelector('meta[property="og:description"]')?.content
    || ''
}

function inferSourceApp(url) {
  const match = SOURCE_RULES.find(([pattern]) => pattern.test(url))
  return match?.[1] || 'browser'
}
