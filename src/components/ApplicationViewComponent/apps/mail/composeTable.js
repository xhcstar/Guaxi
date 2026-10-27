/**
 * 写信正文的表格与 Markdown 转换。
 *
 * 用户的两条要求：
 * 1. **表格必须是真表格**（`<table>` + 边框），不能是 Markdown 的 `|` 管道语法——
 *    后者在 Outlook / 企业邮箱里会原样显示成一堆竖线；
 * 2. **Markdown 要转成渲染结果**再发出去，收件端无论用什么客户端都能正常显示。
 *
 * 这里只放纯函数（便于 node --test 锁住行为），DOM 操作在 MailComposer 里。
 */

/** 表格默认列数 / 行数。 */
export const TABLE_COLS = 3
export const TABLE_ROWS = 3

/**
 * 生成一份「任何客户端都能正常显示」的表格 HTML。
 *
 * 关键点全是邮件客户端的坑：
 * - Outlook（Word 渲染引擎）对现代 CSS 支持有限，所以用**表现属性**
 *   `border="1" cellpadding cellspacing` 而不只写 style；
 * - 空 `<td>` 在部分客户端里会塌掉，每个单元格放一个 `<br>` 占位；
 * - 表头写 `bgcolor`，Outlook 才会真的画出底色。
 */
export function buildTableHtml({ rows = TABLE_ROWS, cols = TABLE_COLS } = {}) {
  const safeRows = Math.max(1, Math.min(20, Number(rows) || TABLE_ROWS))
  const safeCols = Math.max(1, Math.min(10, Number(cols) || TABLE_COLS))
  const head = Array.from({ length: safeCols }, (_, c) => `<th>列 ${c + 1}</th>`).join('')
  const body = Array.from({ length: Math.max(0, safeRows - 1) }, () =>
    `<tr>${Array.from({ length: safeCols }, () => '<td><br></td>').join('')}</tr>`,
  ).join('')
  return (
    '<table border="1" cellpadding="6" cellspacing="0" ' +
    'style="border-collapse:collapse;width:100%;font-size:13px">' +
    `<thead><tr bgcolor="#f2f2f2">${head}</tr></thead>` +
    `<tbody>${body}</tbody>` +
    '</table><p><br></p>'
  )
}

/** 转义 HTML 文本（Markdown 渲染时先转义再套标签）。 */
export function escapeHtml(text) {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

/** 是否是管道表格的一行。 */
const isTableRow = (line) => /^\s*\|.*\|\s*$/.test(line)
/** 是否是表格的对齐分隔行（`|---|:--:|`）。 */
const isTableDivider = (line) => /^\s*\|?[\s:|-]+\|?\s*$/.test(line) && /-/.test(line)

/** 把管道表格渲染成真 `<table>`（带边框，Outlook 可显示）。 */
export function renderPipeTable(lines) {
  const rows = lines.map((line) =>
    line
      .trim()
      .replace(/^\|/, '')
      .replace(/\|$/, '')
      .split('|')
      .map((cell) => cell.trim()),
  )
  const body = rows.filter((cells, index) => !(index === 1 && cells.every((c) => /^:?-{2,}:?$/.test(c))))
  const head = body.length > 1 ? body.shift() : []
  const headHtml = head.length
    ? `<thead><tr bgcolor="#f2f2f2">${head
        .map((cell) => `<th style="border:1px solid #ccc;padding:6px 8px;text-align:left">${inline(cell)}</th>`)
        .join('')}</tr></thead>`
    : ''
  const bodyHtml = body
    .map(
      (cells) =>
        `<tr>${cells
          .map((cell) => `<td style="border:1px solid #ccc;padding:6px 8px">${inline(cell)}</td>`)
          .join('')}</tr>`,
    )
    .join('')
  return (
    '<table border="1" cellpadding="6" cellspacing="0" ' +
    'style="border-collapse:collapse;width:100%;margin:0 0 12px;font-size:13px">' +
    `${headHtml}<tbody>${bodyHtml}</tbody></table>`
  )
}

/**
 * 一份「够用就好」的 Markdown → HTML。
 *
 * 不引第三方库的理由：写完信立刻要出结果，而完整 Markdown 库是几十 KB 的额外依赖，
 * 还要额外处理「邮件里不能出现 <script>」这一层。这里只覆盖写信场景实际会用到的语法，
 * 输出的是**内联样式**——邮件客户端会剥掉 `<style>` 块，样式必须写在标签上。
 *
 * 支持：标题、粗体、斜体、删除线、行内代码、代码块、链接、图片、无序/有序列表、
 * 引用、分隔线、**管道表格**（转成真 `<table>`）、段落与换行。
 */
export function markdownToEmailHtml(markdown) {
  const lines = String(markdown ?? '').replace(/\r\n?/g, '\n').split('\n')
  const out = []
  let paragraph = []
  let listKind = ''
  let inCode = false
  let codeLang = ''
  let codeLines = []
  let inQuote = false

  const flushParagraph = () => {
    if (!paragraph.length) return
    out.push(`<p style="margin:0 0 10px">${inline(paragraph.join(' '))}</p>`)
    paragraph = []
  }
  const closeList = () => {
    if (!listKind) return
    out.push(`</${listKind}>`)
    listKind = ''
  }
  const closeQuote = () => {
    if (!inQuote) return
    out.push('</blockquote>')
    inQuote = false
  }
  const flushBlocks = () => {
    flushParagraph()
    closeList()
    closeQuote()
  }

  for (let index = 0; index < lines.length; index += 1) {
    const raw = lines[index]
    const line = raw.replace(/\s+$/, '')

    // 代码块围栏
    const fence = line.match(/^```\s*([\w+#.-]*)\s*$/)
    if (fence) {
      flushBlocks()
      if (inCode) {
        out.push(codeBlockHtml(codeLines, codeLang))
        codeLines = []
        codeLang = ''
        inCode = false
      } else {
        inCode = true
        codeLang = fence[1] || ''
      }
      continue
    }
    if (inCode) {
      codeLines.push(raw)
      continue
    }

    if (!line.trim()) {
      flushBlocks()
      continue
    }

    // 管道表格：连续收集，一次渲染
    if (isTableRow(line) && index + 1 < lines.length && isTableRow(lines[index + 1])) {
      flushBlocks()
      const tableLines = []
      while (index < lines.length && isTableRow(lines[index])) {
        tableLines.push(lines[index])
        index += 1
      }
      index -= 1
      // 只有「表头 + 分隔行」的写法才当表格（避免把两行普通文本误判成表格）
      const hasDivider = tableLines.length > 1 && isTableDivider(tableLines[1])
      out.push(
        hasDivider
          ? renderPipeTable(tableLines)
          : `<p style="margin:0 0 10px">${inline(tableLines.join(' '))}</p>`,
      )
      continue
    }

    // 分隔线
    if (/^\s*(---|\*\*\*|___)\s*$/.test(line)) {
      flushBlocks()
      out.push('<hr style="border:0;border-top:1px solid #ddd;margin:14px 0" />')
      continue
    }

    // 标题
    const heading = line.match(/^(#{1,6})\s+(.*)$/)
    if (heading) {
      flushBlocks()
      const level = Math.min(6, heading[1].length)
      const sizes = { 1: 22, 2: 19, 3: 17, 4: 15.5, 5: 14.5, 6: 14 }
      out.push(
        `<h${level} style="margin:16px 0 8px;font-size:${sizes[level]}px;line-height:1.35">` +
          `${inline(heading[2])}</h${level}>`,
      )
      continue
    }

    // 引用
    const quote = line.match(/^>\s?(.*)$/)
    if (quote) {
      flushParagraph()
      closeList()
      if (!inQuote) {
        out.push(
          '<blockquote style="margin:0 0 10px;padding:2px 0 2px 12px;' +
            'border-left:3px solid #ddd;color:#666">',
        )
        inQuote = true
      }
      out.push(`<p style="margin:0 0 6px">${inline(quote[1])}</p>`)
      continue
    }
    closeQuote()

    // 列表
    const bullet = line.match(/^\s*[-*+]\s+(.*)$/)
    const ordered = line.match(/^\s*\d+[.)]\s+(.*)$/)
    if (bullet || ordered) {
      flushParagraph()
      const kind = bullet ? 'ul' : 'ol'
      if (listKind !== kind) {
        closeList()
        const style = kind === 'ul' ? 'disc' : 'decimal'
        out.push(`<${kind} style="margin:0 0 10px;padding-left:24px;list-style:${style}">`)
        listKind = kind
      }
      out.push(`<li style="margin:0 0 4px">${inline((bullet || ordered)[1])}</li>`)
      continue
    }
    closeList()

    paragraph.push(line.trim())
  }

  if (inCode && codeLines.length) out.push(codeBlockHtml(codeLines, codeLang))
  flushBlocks()
  return out.join('\n')
}

function codeBlockHtml(lines, lang) {
  const label = lang
    ? `<div style="font-size:11px;color:#888;margin-bottom:4px">${escapeHtml(lang)}</div>`
    : ''
  return (
    '<div style="margin:0 0 10px;padding:10px 12px;border-radius:6px;background:#f5f5f5">' +
    label +
    "<pre style=\"margin:0;overflow-x:auto\"><code style=\"font-family:Consolas,'Courier New',monospace;font-size:12.5px\">" +
    `${escapeHtml(lines.join('\n'))}</code></pre></div>`
  )
}

/** 行内语法：粗体 / 斜体 / 行内代码 / 链接 / 图片 / 删除线。 */
function inline(text) {
  let out = escapeHtml(text)
  // 行内代码先处理：里面的内容不再参与其它替换。
  const codes = []
  out = out.replace(/`([^`]+)`/g, (_, code) => {
    codes.push(code)
    return `\u0000CODE${codes.length - 1}\u0000`
  })
  out = out
    .replace(/!\[([^\]]*)\]\(([^)\s]+)\)/g, (_, alt, src) => `<img src="${src}" alt="${alt}" />`)
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, label, href) => {
      const safe = /^(https?:|mailto:|tel:)/i.test(href) ? href : '#'
      return `<a href="${safe}" style="color:#1a5fd0">${label}</a>`
    })
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/__([^_]+)__/g, '<strong>$1</strong>')
    .replace(/(^|[\s(])\*([^*\s][^*]*)\*/g, '$1<em>$2</em>')
    .replace(/(^|[\s(])_([^_\s][^_]*)_/g, '$1<em>$2</em>')
    .replace(/~~([^~]+)~~/g, '<del>$1</del>')
  return out.replace(/\u0000CODE(\d+)\u0000/g, (_, index) => {
    const code = codes[Number(index)]
    return (
      '<code style="background:#f2f2f2;border-radius:3px;padding:1px 4px;' +
      `font-family:Consolas,'Courier New',monospace;font-size:0.92em">${code}</code>`
    )
  })
}
