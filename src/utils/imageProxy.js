/**
 * 图片 URL 代理工具。
 *
 * 把外部图片 URL 转成本地代理地址 `http://localhost:8888/markdown/api/remote_image?url=...`，
 * 让浏览器请求同源后端而非直接访问第三方 CDN。
 *
 * 主要用途：避免浏览器的 Tracking Prevention（跟踪防护）拦截第三方图片 CDN
 * （如 wp.com、cloudinary.com）对 cookie/storage 的访问，消除 console 里大量警告。
 *
 * 后端代理实现见 `src-tauri/src/guaxiservices/markdown_image_process.rs` 的
 * `fetch_remote_image` —— 已接入系统代理、SSRF 防护、本地磁盘缓存。
 */

const LOCAL_HOST_RE = /^https?:\/\/(localhost|127\.0\.0\.1|asset\.localhost)(:\d+)?\//i
const INLINE_SCHEME_RE = /^(data:|blob:|file:|asset:)/i
const HTTP_SCHEME_RE = /^https?:\/\//i

/**
 * 把外部图片 URL 转成本地代理地址。
 *
 * - 已是本地代理地址（localhost:8888 / 127.0.0.1:8888 / asset.localhost）直接返回。
 * - data: / blob: / file: / asset: 等内联或自定义协议直接返回。
 * - 非 http/https 协议直接返回。
 * - 其余 http/https 外部地址包装成 `http://localhost:8888/markdown/api/remote_image?url=...`。
 *
 * @param {string} url 原始图片 URL
 * @returns {string} 代理后的 URL（或原样返回）
 */
export function proxiedImageUrl(url) {
  if (!url) return ''
  const trimmed = url.trim()
  if (!trimmed) return ''
  if (INLINE_SCHEME_RE.test(trimmed)) return trimmed
  if (LOCAL_HOST_RE.test(trimmed)) return trimmed
  if (!HTTP_SCHEME_RE.test(trimmed)) return trimmed
  return `http://localhost:8888/markdown/api/remote_image?url=${encodeURIComponent(trimmed)}`
}
