/**
 * 引用卡片（一层「历史邮件」的折叠块）的共同处理。
 *
 * 卡片由写信侧生成（MailApp.buildQuote），阅读侧再把它升级成卡片样式
 * （MailReader.upgradeQuoteCards）。问题出在**回信套回信**：每一层的引用
 * 都把上一层整封历史包在自己里面，于是卡片一层叠一层，越往里缩进越深，
 * 读起来非常累（用户反馈：「只要按时间顺序从上到下列出即可，没必要层层叠加」）。
 *
 * 这里提供拍平（flatten）的实现：把嵌在别的卡片里的卡片提到外面，
 * 紧跟在父卡片之后 —— 顺序天然就是「最新在上、越往下越早」。
 * 写信侧与阅读侧共用，避免两处各写一份而慢慢走样。
 *
 * 卡片识别沿用阅读侧原有的两条线索：
 * 1. 我们自己生成的标记 `data-guaxi-quote-card`（`data-*` 能穿过收件清洗层，
 *    `class` 会被剥掉，所以标记必须写在 data 上）；
 * 2. 老邮件里的历史块：圆角外框 + 第一个子元素是「像信头的一行」+ 内含 blockquote。
 */

const CARD_MARK = 'data-guaxi-quote-card'
const CARD_CLASS = 'mail-quote--card'

function isElement(node) {
  return Boolean(node) && node.nodeType === 1
}

export function isQuoteCard(el) {
  if (!isElement(el) || el.tagName !== 'DIV') return false
  if (el.hasAttribute(CARD_MARK) || el.classList?.contains(CARD_CLASS)) return true
  const style = el.getAttribute('style') || ''
  if (!/border-radius\s*:\s*(10|14)px/i.test(style)) return false
  const head = el.firstElementChild
  if (!head || head.tagName !== 'DIV' || !el.querySelector('blockquote')) return false
  // 只认「像历史邮件头」的圆角块（含邮箱地址或时间），避免误伤普通内容卡片。
  const text = head.textContent || ''
  return /@|写道|wrote/i.test(text) || /\d{1,2}:\d{2}/.test(text)
}

/** 文档顺序（外层卡片在前）收集所有引用卡片。 */
export function collectQuoteCards(root) {
  if (!root?.querySelectorAll) return []
  return [...root.querySelectorAll('div')].filter(isQuoteCard)
}

function nearestCardAncestor(card) {
  let node = card.parentElement
  while (node) {
    if (isQuoteCard(node)) return node
    node = node.parentElement
  }
  return null
}

/** 卡片被掏空后留下的空引用体（只剩空白）会让列表多出几行空隙。 */
function dropEmptyBody(card) {
  const body =
    card.querySelector(`[data-guaxi-quote-body]`) || card.querySelector(':scope > blockquote')
  if (!body) return
  if (body.textContent.trim() || body.querySelector('img, table, [data-guaxi-quote-card]')) return
  body.remove()
}

/** 这个元素里还剩点能看见的东西吗？只剩空白、`<br>` 或一张描边的空表就算没有。 */
function hasVisibleContent(el) {
  if (!el) return false
  // 表格不用单独判断：单元格里有字就被 textContent 收到，只剩个空框则不该留着。
  return Boolean(
    (el.textContent || '').trim() ||
      el.querySelector('img, video, audio, iframe, object, embed, input'),
  )
}

/**
 * 收掉没有任何内容的 `<blockquote>`，就地修改传入的 DOM 子树。
 *
 * 回信套回信时，每往下一层就多一个 blockquote：mail.css 给每一层画一条 2px 左竖线、
 * 再缩进 14px。历史卡片被摘走（会话视图）或本身就是空壳（别的客户端的引用格式）之后，
 * 这些壳还留在原地——于是正文底部出现一片由深到浅、一层比一层短的竖线，
 * 嵌套几十封就是一整块「栅栏」（用户反馈的「多次回复的邮件底部有大量竖线」）。
 * 空引用块本身不承载任何信息，直接收掉；从最里层往外收，
 * 因为外层往往只是包着一串空的内层。
 */
export function pruneEmptyQuotes(root) {
  if (!root?.querySelectorAll) return
  for (const block of [...root.querySelectorAll('blockquote')].reverse()) {
    if (!hasVisibleContent(block)) block.remove()
  }
}

/**
 * 把嵌套的引用卡片提到同一层，就地修改传入的 DOM 子树。
 *
 * `A > B > C` 会变成 `A, B, C` 三个兄弟：B 紧跟在 A 后面，C 紧跟在 B 后面，
 * 也就是「最新的一封在最上、往下依次更早」。
 */
export function flattenQuoteCards(root) {
  if (!root?.querySelectorAll) return
  // 一次遍历就能把大多数嵌套提出来；多留两轮兜底（极端嵌套 / 移动后新暴露的层级）。
  for (let pass = 0; pass < 3; pass += 1) {
    let lifted = false
    for (const card of collectQuoteCards(root)) {
      const parent = nearestCardAncestor(card)
      if (!parent) continue
      parent.insertAdjacentElement('afterend', card)
      lifted = true
    }
    if (!lifted) break
  }
  for (const card of collectQuoteCards(root)) dropEmptyBody(card)
}

/** 字符串版：解析 HTML → 拍平卡片 → 序列化。写信侧与阅读侧都用它。 */
export function flattenQuoteHtml(html) {
  const source = String(html || '')
  if (!source.includes('blockquote')) return source
  try {
    const doc = new DOMParser().parseFromString(source, 'text/html')
    flattenQuoteCards(doc.body)
    return doc.body.innerHTML
  } catch {
    // 解析失败就用原文：宁可继续叠着，也不能把用户的引用弄丢。
    return source
  }
}
