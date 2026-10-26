/**
 * 邮件正文的配色适配。
 *
 * 邮件正文里的颜色是发件人写死的（内联 style / font color / bgcolor），而阅读区有
 * 明暗两套纸面。以前暗色模式的做法是给整封正文加 `filter: invert(1)`，结果：
 * - 白色卡片、#F5F5F7 页脚全部被压成纯黑，在深色纸面上显出一块块「黑板」；
 * - 未加载的远端图片（`src=""`）那个破图图标也被反色，成了黑块；
 * - 浅色背景上挑的灰字反色后对比度发灰，读起来发闷。
 *
 * 现在的做法：**不反色**，逐元素把写死的颜色映射到当前纸面上——
 * 浅色底 → 深色表面（保留色相），深色文字 → 浅色文字（按原来的强调层次给亮度），
 * 彩色文字（链接 / 品牌强调）保留色相只提亮；中间调的色块（品牌按钮）原样保留。
 * 纯白 / 近白的段落底与渐变没有层次含义，直接透出纸面，避免逐行白底铺成一层层色块。
 *
 * 这里只放纯函数与一个 DOM 应用器，便于用 node --test 锁住映射规则。
 */

/** 暗色纸面（与 mail.css 的 .mail-doc-dark .mail-body 一致）。 */
export const DARK_PAPER = { r: 0x23, g: 0x26, b: 0x2b }

/** 纸面的色相：中性色映射时借用它，让深浅层次落在同一套冷调里。 */
const PAPER_HUE = 0.62

const NAMED_COLORS = {
  transparent: { r: 0, g: 0, b: 0, a: 0 },
  white: { r: 255, g: 255, b: 255, a: 1 },
  black: { r: 0, g: 0, b: 0, a: 1 },
  silver: { r: 192, g: 192, b: 192, a: 1 },
  gray: { r: 128, g: 128, b: 128, a: 1 },
  grey: { r: 128, g: 128, b: 128, a: 1 },
  whitesmoke: { r: 245, g: 245, b: 245, a: 1 },
  gainsboro: { r: 220, g: 220, b: 220, a: 1 },
  lightgrey: { r: 211, g: 211, b: 211, a: 1 },
  lightgray: { r: 211, g: 211, b: 211, a: 1 },
  dimgrey: { r: 105, g: 105, b: 105, a: 1 },
  dimgray: { r: 105, g: 105, b: 105, a: 1 },
  darkslategray: { r: 47, g: 79, b: 79, a: 1 },
  navy: { r: 0, g: 0, b: 128, a: 1 },
}

/** 解析 CSS 颜色；认不出的形态返回 null（宁可不动，也不要写坏一处颜色）。 */
export function parseCssColor(value) {
  const text = String(value || '').trim().toLowerCase()
  if (!text) return null
  if (NAMED_COLORS[text]) return { ...NAMED_COLORS[text] }

  const rgb = text.match(
    /^rgba?\(\s*([\d.]+%?)[,\s]+([\d.]+%?)[,\s]+([\d.]+%?)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/,
  )
  if (rgb) {
    const channel = (raw) =>
      raw.endsWith('%')
        ? Math.round((Number.parseFloat(raw) / 100) * 255)
        : Math.round(Number.parseFloat(raw))
    const alpha = rgb[4] === undefined
      ? 1
      : rgb[4].endsWith('%')
        ? Number.parseFloat(rgb[4]) / 100
        : Number.parseFloat(rgb[4])
    const color = { r: channel(rgb[1]), g: channel(rgb[2]), b: channel(rgb[3]), a: alpha }
    return validRgb(color) ? color : null
  }

  const hex = text.match(/^#([0-9a-f]{3,8})$/)
  if (hex) {
    const digits = hex[1]
    if (digits.length === 3 || digits.length === 4) {
      const [r, g, b, a] = [...digits].map((d) => Number.parseInt(d + d, 16))
      return { r, g, b, a: digits.length === 4 ? a / 255 : 1 }
    }
    if (digits.length === 6 || digits.length === 8) {
      const v = (i) => Number.parseInt(digits.slice(i, i + 2), 16)
      return { r: v(0), g: v(2), b: v(4), a: digits.length === 8 ? v(6) / 255 : 1 }
    }
  }
  return null
}

function validRgb({ r, g, b, a }) {
  return [r, g, b].every((n) => Number.isFinite(n) && n >= 0 && n <= 255) &&
    Number.isFinite(a) && a >= 0 && a <= 1
}

/** 相对亮度（WCAG 定义，0 = 黑，1 = 白）。 */
export function luminance({ r, g, b }) {
  const channel = (v) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

export function rgbToHsl({ r, g, b }) {
  const rn = r / 255
  const gn = g / 255
  const bn = b / 255
  const max = Math.max(rn, gn, bn)
  const min = Math.min(rn, gn, bn)
  const l = (max + min) / 2
  if (max === min) return { h: 0, s: 0, l }
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  let h
  if (max === rn) h = ((gn - bn) / d + (gn < bn ? 6 : 0)) / 6
  else if (max === gn) h = ((bn - rn) / d + 2) / 6
  else h = ((rn - gn) / d + 4) / 6
  return { h, s, l }
}

function hslToRgb(h, s, l) {
  if (s === 0) {
    const v = Math.round(l * 255)
    return { r: v, g: v, b: v }
  }
  const hue = (t) => {
    let x = t
    if (x < 0) x += 1
    if (x > 1) x -= 1
    if (x < 1 / 6) return p + (q - p) * 6 * x
    if (x < 1 / 2) return q
    if (x < 2 / 3) return p + (q - p) * (2 / 3 - x) * 6
    return p
  }
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s
  const p = 2 * l - q
  return {
    r: Math.round(hue(h + 1 / 3) * 255),
    g: Math.round(hue(h) * 255),
    b: Math.round(hue(h - 1 / 3) * 255),
  }
}

export function formatColor({ r, g, b }, a = 1) {
  const clamp = (v) => Math.max(0, Math.min(255, Math.round(v)))
  if (a >= 0.999) return `rgb(${clamp(r)}, ${clamp(g)}, ${clamp(b)})`
  return `rgba(${clamp(r)}, ${clamp(g)}, ${clamp(b)}, ${Number(a.toFixed(3))})`
}

/** 按目标相对亮度给一个中性色（同色相的极低饱和度，避免大片纯灰）。 */
function neutralAt(targetLum, hue, alpha) {
  // 相对亮度对通道值近似线性（同色相差一截可忽略），二分求解即可。
  let low = 0
  let high = 255
  for (let i = 0; i < 18; i += 1) {
    const mid = (low + high) / 2
    const gray = { r: mid, g: mid, b: mid }
    if (luminance(gray) < targetLum) low = mid
    else high = mid
  }
  return formatColor({ ...hslToRgb(hue, 0.06, (low + high) / 2 / 255), a: alpha }, alpha)
}

/**
 * 背景色映射（暗色模式）。
 *
 * 返回 `null` 表示「保持原样」：
 * - 近乎透明（a < 0.12）的叠层不动；
 * - 中间调（品牌色按钮、深色标题条）不动——它们是设计意图，改成纸面色会丢掉品牌识别；
 * - 已经是深色的表面（「显示图片」或切换主题后重跑）也不动，否则会被反复压暗。
 *
 * 亮底的映射不是「一律刷成同一个灰」：#FFFFFF 的卡片与 #F5F5F7 的外带宽在亮色模式里
 * 本来就是两个层次，压到暗色后必须仍然分得出来，所以按原亮度落在纸面上方一段很窄的
 * 区间（相对亮度 0.022~0.033，约 #2C2D33~#36373D）。中性色统一带上纸面的冷色相，
 * 避免在偏蓝的深色界面里冒出一片纯灰。
 */
export function mapBackgroundColor(value, mode = 'dark') {
  if (mode !== 'dark') return null
  const color = parseCssColor(value)
  if (!color) return null
  if (color.a < 0.12) return null
  const lum = luminance(color)
  const { h, s } = rgbToHsl(color)

  // 浅底（白卡片 / 浅灰页脚 / 浅色 banner）：换成纸面上方一档的深色表面。
  // 彩色块保留自己的色相（只压饱和度）。
  if (lum > 0.62) {
    const target = 0.022 + ((lum - 0.62) / 0.38) * 0.011
    if (s >= 0.14) {
      // 彩色：按色相找一个接近目标亮度的深色。
      let best = null
      for (let l = 0.06; l <= 0.4; l += 0.005) {
        const candidate = hslToRgb(h, Math.min(s, 0.5) * 0.7, l)
        if (!best || Math.abs(luminance(candidate) - target) < Math.abs(luminance(best) - target)) {
          best = candidate
        }
      }
      return formatColor(best, color.a)
    }
    return neutralAt(target, PAPER_HUE, color.a)
  }
  // 纯黑块：提亮到纸面略深的位置，避免深色纸面上出现一个「洞」。
  // 阈值压得很低，是因为映射出来的表面本身就在 0.022 上下——重跑时必须原样返回。
  if (lum < 0.008) {
    return neutralAt(0.012, PAPER_HUE, Math.min(1, color.a + 0.5))
  }
  return null
}

/**
 * 文字色映射（暗色模式）。
 *
 * 判断标准是**对比度**而不是「颜色深浅」：同一个 #1D1D1F 原本压在白色卡片上是正常的，
 * 移到深色纸面上就几乎看不见；而 #C8C8C8 这种中间灰本来就能在深色底上读。
 * 已经达到正文可读标准（WCAG AA 4.5:1）的颜色一律不动。
 *
 * @param {string} value 邮件里写死的颜色
 * @param {number} backdropLum 这个字实际压着的底色亮度（已按映射后的结果计算）
 */
export function mapTextColor(value, backdropLum, mode = 'dark') {
  if (mode !== 'dark') return null
  const color = parseCssColor(value)
  if (!color) return null
  if (color.a < 0.12) return null
  const lum = luminance(color)
  // 底子本身还偏亮（映射后留下的中间调色块）：原色仍是可读的，不动。
  if (backdropLum >= 0.45) return null
  if (contrastRatio(lum, backdropLum) >= 4.5) return null
  const { h, s } = rgbToHsl(color)

  // 彩色文字（链接、强调）：保留色相，只把亮度提上来。
  if (s > 0.22) {
    let best = null
    for (let l = 0.45; l <= 0.92; l += 0.005) {
      const candidate = hslToRgb(h, Math.min(s, 0.9), l)
      if (contrastRatio(luminance(candidate), backdropLum) >= 6) {
        best = candidate
        break
      }
    }
    const picked = best || hslToRgb(h, Math.min(s, 0.9), 0.8)
    return formatColor(picked, color.a)
  }

  // 中性文字：按原来的强调层次给亮度——原来的深黑变近白，次要灰变成中灰。
  // 用二分求「相对亮度」，因为 HSL 的 l 与感知亮度不是线性关系。
  const target = Math.min(0.86, Math.max(0.16, 0.82 - lum * 0.7))
  return neutralAt(target, h, color.a)
}

/** WCAG 对比度（1~21）。 */
export function contrastRatio(a, b) {
  const light = Math.max(a, b)
  const dark = Math.min(a, b)
  return (light + 0.05) / (dark + 0.05)
}

/** 从 `linear-gradient(...)` / `radial-gradient(...)` 里取出所有颜色（按出现顺序）。 */
export function gradientColors(value) {
  const text = String(value || '')
  if (!/gradient/i.test(text)) return []
  const found = text.match(/rgba?\([^)]*\)|#[0-9a-fA-F]{3,8}\b|[a-zA-Z]+(?=\s*(?:,|\)|$))/g) || []
  return found.filter((token) => parseCssColor(token) && !/gradient/i.test(token))
}

/** 渐变里的平均亮度（用来看它算不算浅色底）。 */
export function gradientLuminance(value) {
  const colors = gradientColors(value)
    .map(parseCssColor)
    .filter(Boolean)
    .filter((c) => c.a >= 0.12)
  if (!colors.length) return null
  return colors.reduce((sum, c) => sum + luminance(c), 0) / colors.length
}

/**
 * 渐变映射：逐色标按背景规则改写。
 * 只要有一个色标需要改，就整条重写（保持色标位置不变）；否则返回 null。
 */
export function mapBackgroundImage(value, mode = 'dark') {
  if (mode !== 'dark') return null
  const text = String(value || '')
  if (!/gradient/i.test(text)) return null
  let changed = false
  const mapped = text.replace(
    /rgba?\([^)]*\)|#[0-9a-fA-F]{3,8}\b/g,
    (token) => {
      const next = mapBackgroundColor(token, mode)
      if (!next) return token
      changed = true
      return next
    },
  )
  return changed ? mapped : null
}

/** 元素背后实际压着的亮度（已映射）；走到 `.mail-body` 就是纸面本身。 */
function backdropLuminance(el, paperLum) {
  let node = el
  while (node) {
    const style = getComputedStyle(node)
    const image = style.backgroundImage
    if (image && image !== 'none') {
      const lum = gradientLuminance(image)
      if (lum !== null) return lum
      // 背景图（照片）亮度不可知：按「深色」处理，浅色文字留在上面可读。
      if (/url\(/i.test(image)) return 0.12
    }
    const bg = parseCssColor(style.backgroundColor)
    if (bg && bg.a > 0.12) {
      const own = luminance(bg)
      // 半透明叠层：按与纸面混合后的结果算。
      if (bg.a >= 0.999) return own
      return own * bg.a + paperLum * (1 - bg.a)
    }
    if (node.classList?.contains('mail-body')) break
    node = node.parentElement
  }
  return paperLum
}

/**
 * 近乎无彩色的近白色。
 *
 * 邮箱客户端（Outlook / Seznam）常给**每个段落**套一层 `background-color:#fff`。
 * 这类底色没有设计含义，暗色下如果映射成「比纸面亮一档的表面」，就会变成一条条
 * 分层色块——正是用户说的「没必要包裹多层区域」。所以这类白底在暗色下直接透明，
 * 让正文回到同一张纸面；真正有颜色的浅底（米色 banner、浅蓝卡片）仍按色相映射。
 */
function isNeutralNearWhite(value) {
  const color = typeof value === 'string' ? parseCssColor(value) : value
  if (!color || color.a < 0.12) return false
  const max = Math.max(color.r, color.g, color.b)
  const min = Math.min(color.r, color.g, color.b)
  return luminance(color) > 0.9 && (max - min) / 255 <= 0.04
}

/** 整条渐变是否都是这类近白色。 */
function isNeutralNearWhiteGradient(value) {
  const colors = gradientColors(value)
    .map(parseCssColor)
    .filter(Boolean)
    .filter((color) => color.a >= 0.12)
  return colors.length > 0 && colors.every((color) => isNeutralNearWhite(color))
}

/** 元素自己声明的颜色：内联 style 优先，其次 `font color` / `bgcolor` 这类表现属性。 */
function declared(el) {
  const style = el.style || {}
  return {
    color: style.color || el.getAttribute?.('color') || '',
    styleColor: style.color || '',
    background: style.backgroundColor || '',
    backgroundImage: style.backgroundImage || '',
    bgcolorAttr: el.getAttribute?.('bgcolor') || '',
    colorAttr: el.getAttribute?.('color') || '',
  }
}

// 原值记录：同一个 DOM 会被反复映射（换信、切换主题、显示图片都会重跑），
// 直接把映射结果再映射一次会一路变暗。把邮件里的原值记在 data-* 上，
// 每次从原值出发，同时让「切回亮色」能原样还原。
const ORIG = {
  bg: 'data-guaxi-orig-bg',
  bgAttr: 'data-guaxi-orig-bg-attr',
  fg: 'data-guaxi-orig-fg',
  fgAttr: 'data-guaxi-orig-fg-attr',
  image: 'data-guaxi-orig-image',
}

/**
 * 还原一个被改写过的属性。`apply` 与 `remove` 分别对应「有原值」与「原来没有」。
 * 没有记录过就什么也不做。
 */
function restore(el, key, apply, remove) {
  if (!el.hasAttribute(key)) return
  const original = el.getAttribute(key)
  el.removeAttribute(key)
  if (original) apply(original)
  else remove()
}

/**
 * 把阅读区里的每一封正文按当前主题重上色。
 *
 * 两趟：先背景、后文字。文字要判断「压着什么底色」，必须在背景改完之后再看。
 * 只处理 `style` / `color` / `bgcolor` 这类**写死**的颜色；继承来的颜色由纸面自己管。
 */
export function applyBodyTheme(root, mode = 'dark') {
  if (!root) return
  const bodies = [...root.querySelectorAll('.mail-body')]
  if (!bodies.length) return
  const dark = mode === 'dark'
  const paperLum = luminance(DARK_PAPER)

  const targets = []
  for (const body of bodies) targets.push(body, ...body.querySelectorAll('*'))

  // 先把所有元素还原成原值：既保证重复应用稳定，也让切回亮色时不留痕。
  for (const el of targets) {
    if (!el.hasAttribute) continue
    restore(
      el,
      ORIG.bg,
      (v) => (el.style.backgroundColor = v),
      () => el.style.removeProperty('background-color'),
    )
    restore(
      el,
      ORIG.bgAttr,
      (v) => el.setAttribute('bgcolor', v),
      () => el.removeAttribute('bgcolor'),
    )
    restore(
      el,
      ORIG.image,
      (v) => (el.style.backgroundImage = v),
      () => el.style.removeProperty('background-image'),
    )
    restore(
      el,
      ORIG.fg,
      (v) => (el.style.color = v),
      () => el.style.removeProperty('color'),
    )
    restore(
      el,
      ORIG.fgAttr,
      (v) => el.setAttribute('color', v),
      () => el.removeAttribute('color'),
    )
  }
  if (!dark) return

  for (const el of targets) {
    if (!el.style || !el.hasAttribute) continue
    // 原值优先：还原后 el.style 可能已经是空串，但原值仍是「邮件里写的那个」。
    const own = declared(el)
    const bgOriginal = el.getAttribute(ORIG.bgAttr) || own.bgcolorAttr
    const bgSource = bgOriginal || own.background
    // 纯白段落底不保留「表面」这一层：直接透明，避免正文变成条带状的分层色块。
    const nextBg = isNeutralNearWhite(bgSource) ? 'transparent' : mapBackgroundColor(bgSource, mode)
    if (nextBg) {
      if (!bgOriginal && own.background) {
        el.setAttribute(ORIG.bg, own.background)
        el.style.backgroundColor = nextBg
      } else {
        el.setAttribute(ORIG.bgAttr, bgOriginal)
        el.setAttribute('bgcolor', nextBg)
      }
    }
    const imageOriginal = el.getAttribute(ORIG.image) || own.backgroundImage
    const nextImage = isNeutralNearWhiteGradient(imageOriginal)
      ? 'none'
      : mapBackgroundImage(imageOriginal, mode)
    if (nextImage) {
      el.setAttribute(ORIG.image, imageOriginal)
      el.style.backgroundImage = nextImage
    }
  }

  for (const el of targets) {
    if (!el.style || !el.hasAttribute) continue
    const own = declared(el)
    const colorOriginal = el.getAttribute(ORIG.fg) || el.getAttribute(ORIG.fgAttr) || own.color
    if (!colorOriginal) continue
    const next = mapTextColor(colorOriginal, backdropLuminance(el, paperLum), mode)
    if (!next) continue
    if (el.hasAttribute(ORIG.fgAttr) || (!own.styleColor && own.colorAttr)) {
      const original = el.getAttribute(ORIG.fgAttr) || own.colorAttr
      el.setAttribute(ORIG.fgAttr, original)
      // 旧式 `color` 属性只认 #hex / 颜色名；把 rgb() 写进去会被按「无效颜色」处理
      // （Chromium 会落成绿色）。所以用内联 style 覆盖，属性保持原值供切回亮色时还原。
      if (!el.hasAttribute(ORIG.fg)) el.setAttribute(ORIG.fg, own.styleColor || '')
      el.style.color = next
    } else {
      const original = el.getAttribute(ORIG.fg) || own.styleColor
      el.setAttribute(ORIG.fg, original)
      el.style.color = next
    }
  }
}
