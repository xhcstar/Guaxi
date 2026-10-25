// 电子书章节 HTML 净化（零依赖纯函数，便于单测）。
//
// 背景（真实问题）：EPUB 章节里常带 <script>（排版工具导出、阅读增强脚本等）。
// 阅读器的 iframe 不允许执行任何书籍脚本；而浏览器在**拒绝执行之前**仍会尝试解析
// 每一段脚本，并往控制台打印红色报错，一本几十个脚本的书打开就像软件坏了。
//
// 两层防线：
//   1. 这里在送进 iframe 之前把可执行内容彻底删掉（脚本、内联事件、javascript:、
//      会再加载文档的容器、meta refresh）；
//   2. 章节文档带一条 CSP（script-src 'none'），即便有畸形标签漏网也无法执行。
// 注意：iframe 本身**不加 sandbox** —— Tauri 会往每个新文档注入它的 IPC 初始化脚本，
// sandbox 一开，这份注入脚本就会被 Chromium 拦住并逐帧打印
// "Blocked script execution …"；关掉 sandbox、用 CSP 收口则两者都不再报错。

/** 剥离脚本、嵌套文档容器、内联事件与 javascript: 链接。 */
export function stripScripts(html) {
  return String(html || '')
    // 1) 脚本块（跨行、属性里带 > 的写法都要覆盖）
    .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '')
    .replace(/<script[^>]*\/?>/gi, '')
    // 2) 会再加载文档的容器：<iframe srcdoc> 会在章节里再开一个浏览上下文，
    //    其中的脚本同样不受控。电子书正文不需要它们。
    .replace(/<iframe[^>]*>[\s\S]*?<\/iframe>/gi, '')
    .replace(/<iframe[^>]*\/?>/gi, '')
    .replace(/<object[^>]*>[\s\S]*?<\/object>/gi, '')
    .replace(/<object[^>]*\/?>/gi, '')
    .replace(/<embed[^>]*\/?>/gi, '')
    // 3) 自动跳转：meta refresh 不依赖脚本，必须显式摘掉；<base> 会改写所有相对链接。
    .replace(/<meta[^>]+http-equiv\s*=\s*["']?refresh["']?[^>]*>/gi, '')
    .replace(/<base[^>]*\/?>/gi, '')
    // 4) 内联事件处理器（带引号与不带引号两种写法）
    .replace(/\son[a-z]+\s*=\s*"[^"]*"/gi, '')
    .replace(/\son[a-z]+\s*=\s*'[^']*'/gi, '')
    .replace(/\son[a-z]+\s*=\s*[^\s>"']+/gi, '')
    // 5) javascript: 伪协议（href/src/xlink:href/action，带引号与不带引号）
    .replace(/\b(href|src|xlink:href|action)\s*=\s*(["'])\s*javascript:[\s\S]*?\2/gi, '$1=$2#$2')
    .replace(/\b(href|src|xlink:href|action)\s*=\s*javascript:[^\s>]*/gi, '$1="#"')
}

/**
 * 净化并自检。剥离后若仍有 <script> 残留（畸形标签等边界情况），
 * 整章退化为纯文本 —— 宁可少一点排版，也绝不让控制台刷屏。
 *
 * @param {string} html
 * @returns {{ html: string, degraded: boolean }}
 */
export function sanitizeSectionHtml(html) {
  const stripped = stripScripts(html)
  if (/<script/i.test(stripped)) {
    const text = stripped
      .replace(/<[^>]*>/g, ' ')
      .replace(/[<>]/g, (c) => (c === '<' ? '&lt;' : '&gt;'))
      .replace(/\s+/g, ' ')
      .trim()
    return { html: `<pre style="white-space:pre-wrap">${text}</pre>`, degraded: true }
  }
  return { html: stripped, degraded: false }
}
