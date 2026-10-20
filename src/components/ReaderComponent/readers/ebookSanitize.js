// 电子书章节 HTML 净化（零依赖纯函数，便于单测）。
//
// 背景（真实问题）：EPUB 章节里常带 <script>（排版工具导出、阅读增强脚本等）。
// 阅读器的 iframe 用 sandbox 禁掉了脚本执行，但浏览器在**拒绝执行之前**仍会尝试
// 解析每一段脚本，并往控制台打印：
//   Blocked script execution in 'blob:...' because the document's frame is sandboxed
// 一本几十个脚本的书打开就是几十条红色报错，看起来像软件坏了。
//
// 所以策略是「在送进 iframe 之前就把可执行内容彻底删掉」，沙箱保留作为第二道防线。

/** 剥离脚本、嵌套文档容器、内联事件与 javascript: 链接。 */
export function stripScripts(html) {
  return String(html || '')
    // 1) 脚本块（跨行、属性里带 > 的写法都要覆盖）
    .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '')
    .replace(/<script[^>]*\/?>/gi, '')
    // 2) 会再加载文档的容器：<iframe srcdoc> 会创建继承 sandbox 的嵌套浏览上下文，
    //    其中的脚本同样会被拦并报警。电子书正文不需要它们。
    .replace(/<iframe[^>]*>[\s\S]*?<\/iframe>/gi, '')
    .replace(/<iframe[^>]*\/?>/gi, '')
    .replace(/<object[^>]*>[\s\S]*?<\/object>/gi, '')
    .replace(/<object[^>]*\/?>/gi, '')
    .replace(/<embed[^>]*\/?>/gi, '')
    // 3) 内联事件处理器
    .replace(/\son[a-z]+\s*=\s*"[^"]*"/gi, '')
    .replace(/\son[a-z]+\s*=\s*'[^']*'/gi, '')
    // 4) javascript: 伪协议链接
    .replace(/href\s*=\s*(["'])\s*javascript:[^"']*\1/gi, 'href="#"')
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
