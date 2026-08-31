/**
 * 放行 Markdown 里的 `file:` 协议链接。
 *
 * 背景：markdown-it 的默认 validateLink 把 `file:` 和 `javascript:` / `vbscript:` / `data:`
 * 一起列进了黑名单（见 markdown-it 的 BAD_PROTO_RE），所以
 *
 *   [ssl\_var\_shared.h#L1594-L1597](file:///e:/Code/.../ssl_var_shared.h#L1594)
 *
 * 这种写法压根不会被渲染成 <a>，只会留下一段纯文本 —— 看起来就像"渲染引擎不认这个语法"。
 *
 * 这里只把 `file:` 单独放行，`javascript:` / `vbscript:` / 非图片 `data:` 仍旧按原样拦截，
 * 因此不会放宽既有的安全边界。
 *
 * 点击后的行为不需要额外处理：预览的链接点击链路本来就会把非 http 链接交给
 * splitFsLinkAndAnchor + openFsLinkSmart，那条链路已经能解 `file://` 前缀和 `#L1594` 锚点。
 *
 * 另外，`linkify` 不认识 `file:` schema，所以正文里裸写的 file:// 文本不会因此被自动转成链接，
 * 对既有渲染结果零影响。
 */

const BLOCKED_PROTO_RE = /^(vbscript|javascript|data):/
const GOOD_DATA_RE = /^data:image\/(gif|png|jpeg|webp);/

/**
 * @param {import('markdown-it')} md markdown-it 实例
 * @returns {import('markdown-it')} 原实例，便于链式调用
 */
export const allowFileProtocolLinks = (md) => {
  if (!md || typeof md.validateLink !== 'function') return md

  md.validateLink = (url) => {
    const str = String(url || '').trim().toLowerCase()
    if (str.startsWith('file:')) return true
    return BLOCKED_PROTO_RE.test(str) ? GOOD_DATA_RE.test(str) : true
  }

  return md
}
