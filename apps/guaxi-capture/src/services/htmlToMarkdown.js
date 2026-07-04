const BLOCK_TAGS = new Set([
  'ARTICLE',
  'ASIDE',
  'BLOCKQUOTE',
  'DIV',
  'FIGURE',
  'FOOTER',
  'HEADER',
  'MAIN',
  'NAV',
  'P',
  'SECTION',
])

// 设计说明：
// 这里先实现一个轻量 HTML -> Markdown 转换器，避免为了采集入口引入新的依赖。
// 浏览器插件或移动端分享只要传入正文 HTML，Capture App 就能先得到可编辑 Markdown；
// 后续如果需要更高保真，再把这个模块替换为 Readability/Turndown 一类成熟管线。
export function htmlToMarkdown(html = '') {
  const trimmed = String(html || '').trim()
  if (!trimmed) return ''

  console.info('[capture-markdown] converting html', {
    length: trimmed.length,
  })

  if (typeof DOMParser === 'undefined') {
    console.warn('[capture-markdown] DOMParser unavailable, using plain-text fallback')
    return stripHtmlTags(trimmed)
  }

  const document = new DOMParser().parseFromString(trimmed, 'text/html')
  const root = pickArticleRoot(document)
  const markdown = normalizeMarkdown(renderChildren(root))

  console.info('[capture-markdown] html converted', {
    outputLength: markdown.length,
  })

  return markdown
}

export function looksLikeHtml(value = '') {
  return /<\/?[a-z][\s\S]*>/i.test(String(value || ''))
}

function pickArticleRoot(document) {
  return document.querySelector('article')
    || document.querySelector('main')
    || document.body
}

function renderChildren(node, context = {}) {
  return Array.from(node.childNodes)
    .map((child) => renderNode(child, context))
    .join('')
}

function renderNode(node, context = {}) {
  if (node.nodeType === Node.TEXT_NODE) {
    return normalizeTextNode(node.textContent || '', context)
  }

  if (node.nodeType !== Node.ELEMENT_NODE) {
    return ''
  }

  const element = node
  const tag = element.tagName
  const children = () => renderChildren(element, context)

  if (isHiddenElement(element) || ['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE'].includes(tag)) {
    return ''
  }

  if (/^H[1-6]$/.test(tag)) {
    const level = Number(tag.slice(1))
    return block(`${'#'.repeat(level)} ${collapseWhitespace(children())}`)
  }

  if (tag === 'P') return block(children())
  if (tag === 'BR') return '\n'
  if (tag === 'HR') return '\n\n---\n\n'
  if (tag === 'BLOCKQUOTE') return quoteBlock(children())
  if (tag === 'PRE') return fencedCode(element.textContent || '')
  if (tag === 'CODE') return inlineCode(element.textContent || '')
  if (tag === 'STRONG' || tag === 'B') return wrapInline(children(), '**')
  if (tag === 'EM' || tag === 'I') return wrapInline(children(), '*')
  if (tag === 'DEL' || tag === 'S') return wrapInline(children(), '~~')
  if (tag === 'A') return renderLink(element, children())
  if (tag === 'IMG') return renderImage(element)
  if (tag === 'UL') return renderList(element, false, context)
  if (tag === 'OL') return renderList(element, true, context)
  if (tag === 'LI') return children()
  if (tag === 'TABLE') return renderTable(element)

  if (BLOCK_TAGS.has(tag)) {
    return block(children())
  }

  return children()
}

function renderList(element, ordered, context) {
  const depth = context.depth || 0
  const items = Array.from(element.children).filter((child) => child.tagName === 'LI')

  const lines = items.map((item, index) => {
    const marker = ordered ? `${index + 1}. ` : '- '
    const content = renderChildren(item, { ...context, depth: depth + 1 })
      .trim()
      .replace(/\n{3,}/g, '\n\n')
      .replace(/\n/g, `\n${'  '.repeat(depth + 1)}`)

    return `${'  '.repeat(depth)}${marker}${content}`
  })

  return block(lines.join('\n'))
}

function renderLink(element, text) {
  const label = collapseWhitespace(text)
  const href = element.getAttribute('href') || ''
  if (!href) return label
  if (!label || label === href) return href
  return `[${escapeMarkdownLinkText(label)}](${href})`
}

function renderImage(element) {
  const src = element.getAttribute('src') || element.getAttribute('data-src') || ''
  if (!src) return ''
  const alt = element.getAttribute('alt') || 'image'
  return `![${escapeMarkdownLinkText(alt)}](${src})`
}

function renderTable(table) {
  const rows = Array.from(table.querySelectorAll('tr'))
    .map((row) => Array.from(row.children).map((cell) => collapseWhitespace(renderChildren(cell))))
    .filter((cells) => cells.length)

  if (!rows.length) return ''

  const maxColumns = Math.max(...rows.map((cells) => cells.length))
  const normalizedRows = rows.map((cells) => padCells(cells, maxColumns))
  const header = normalizedRows[0]
  const separator = header.map(() => '---')
  const body = normalizedRows.slice(1)

  return block([header, separator, ...body].map(renderTableRow).join('\n'))
}

function block(value) {
  const content = normalizeMarkdown(value)
  return content ? `\n\n${content}\n\n` : ''
}

function quoteBlock(value) {
  const content = normalizeMarkdown(value)
  if (!content) return ''
  return block(content.split('\n').map((line) => `> ${line}`).join('\n'))
}

function fencedCode(value) {
  const content = String(value || '').replace(/\n+$/g, '')
  return content ? `\n\n\`\`\`\n${content}\n\`\`\`\n\n` : ''
}

function inlineCode(value) {
  const content = String(value || '').replace(/\s+/g, ' ').trim()
  return content ? `\`${content.replaceAll('`', '\\`')}\`` : ''
}

function wrapInline(value, wrapper) {
  const content = collapseWhitespace(value)
  return content ? `${wrapper}${content}${wrapper}` : ''
}

function normalizeTextNode(value, context) {
  if (context.preserveWhitespace) return value
  return value.replace(/\s+/g, ' ')
}

function normalizeMarkdown(value) {
  return String(value || '')
    .replace(/\u00a0/g, ' ')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n[ \t]+/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

function collapseWhitespace(value) {
  return normalizeMarkdown(value).replace(/\s+/g, ' ').trim()
}

function stripHtmlTags(value) {
  return normalizeMarkdown(
    value
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/p>/gi, '\n\n')
      .replace(/<[^>]+>/g, ''),
  )
}

function isHiddenElement(element) {
  return element.hidden
    || element.getAttribute('aria-hidden') === 'true'
    || /display\s*:\s*none/i.test(element.getAttribute('style') || '')
}

function escapeMarkdownLinkText(value) {
  return String(value || '').replaceAll('[', '\\[').replaceAll(']', '\\]')
}

function padCells(cells, maxColumns) {
  return [...cells, ...Array(Math.max(maxColumns - cells.length, 0)).fill('')]
}

function renderTableRow(cells) {
  return `| ${cells.map((cell) => cell.replaceAll('|', '\\|')).join(' | ')} |`
}
