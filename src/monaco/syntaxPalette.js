// 设计说明：
// 1. 代码配色只有一个真相源：先从上游主题里解析出 9 个「角色色」，
//    再把 27 个语义桶按「角色 + 明度档」映射出来。编辑器 token、围栏 decoration、
//    Notebook cell、Markdown 预览的 `.hljs-*` 全部读这份表。
// 2. 以前是按 hljs 类名一对一取色，Atom One / Stack Overflow 这种只有 4~7 个色的主题里，
//    变量、参数、类成员、常量会全部拿到同一个橙色——不是取错类，是上游没给这么多颜色。
// 3. 所以同族内再分档（±9 / ±18：一点明度 + 一点色相偏移）。基准色相永远来自主题自身，
//    换主题时「关键字紫、类型黄、变量红」这层语义关系不变，只是整族跟着主题走。

const clamp = (value, min, max) => Math.min(max, Math.max(min, value))

const hexToRgb = (hex) => {
  let value = String(hex || '').trim().replace('#', '')
  if (value.length === 3) value = value.split('').map((char) => char + char).join('')
  if (!/^[0-9a-f]{6}$/i.test(value)) return null
  return [
    parseInt(value.slice(0, 2), 16),
    parseInt(value.slice(2, 4), 16),
    parseInt(value.slice(4, 6), 16),
  ]
}

const rgbToHsl = ([red, green, blue]) => {
  const r = red / 255
  const g = green / 255
  const b = blue / 255
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const lightness = (max + min) / 2
  const delta = max - min
  if (!delta) return [0, 0, lightness * 100]

  const saturation = lightness > 0.5
    ? delta / (2 - max - min)
    : delta / (max + min)
  let hue = 0
  if (max === r) hue = (g - b) / delta + (g < b ? 6 : 0)
  else if (max === g) hue = (b - r) / delta + 2
  else hue = (r - g) / delta + 4
  return [hue * 60, saturation * 100, lightness * 100]
}

const hslToHex = ([hue, saturation, lightness]) => {
  const h = ((hue % 360) + 360) % 360
  const s = clamp(saturation, 0, 100) / 100
  const l = clamp(lightness, 0, 100) / 100
  const chroma = (1 - Math.abs(2 * l - 1)) * s
  const second = chroma * (1 - Math.abs(((h / 60) % 2) - 1))
  const offset = l - chroma / 2
  let rgb
  if (h < 60) rgb = [chroma, second, 0]
  else if (h < 120) rgb = [second, chroma, 0]
  else if (h < 180) rgb = [0, chroma, second]
  else if (h < 240) rgb = [0, second, chroma]
  else if (h < 300) rgb = [second, 0, chroma]
  else rgb = [chroma, 0, second]
  return `#${rgb.map((value) => Math.round((value + offset) * 255).toString(16).padStart(2, '0')).join('')}`
}

const toHsl = (hex) => {
  const rgb = hexToRgb(hex)
  return rgb ? rgbToHsl(rgb) : null
}

// 一个语义桶 = 角色 + 档位。shade 为 0 的桶直接吃角色原色；
// 其余在角色色相附近做轻微偏移（见 SHADE_STEPS），亮色主题统一往深走。
const BUCKET_SPECS = [
  { key: 'base', role: 'base', shade: 0 },
  { key: 'comment', role: 'muted', shade: 0 },

  { key: 'keyword', role: 'purple', shade: 0 },
  { key: 'macro', role: 'purple', shade: 18 },
  { key: 'decorator', role: 'purple', shade: -18 },
  { key: 'label', role: 'purple', shade: 9 },

  { key: 'string', role: 'green', shade: 0 },
  { key: 'regexp', role: 'green', shade: -18 },

  { key: 'number', role: 'orange', shade: 0 },
  { key: 'boolean', role: 'orange', shade: 18 },
  { key: 'constant', role: 'orange', shade: -18 },
  { key: 'attribute', role: 'orange', shade: 9 },

  { key: 'function', role: 'blue', shade: 0 },
  { key: 'method', role: 'blue', shade: 18 },

  { key: 'class', role: 'yellow', shade: 0 },
  { key: 'type', role: 'yellow', shade: 18 },
  { key: 'inherit', role: 'yellow', shade: -18 },
  { key: 'namespace', role: 'yellow', shade: 9 },

  { key: 'variable', role: 'red', shade: 0 },
  { key: 'property', role: 'red', shade: 18 },
  { key: 'parameter', role: 'red', shade: -18 },
  { key: 'variable-language', role: 'red', shade: 9 },
  { key: 'tag', role: 'red', shade: -9 },

  { key: 'operator', role: 'cyan', shade: 0 },
  { key: 'builtin', role: 'cyan', shade: 18 },
  { key: 'escape', role: 'cyan', shade: -18 },

  { key: 'punctuation', role: 'base', shade: 0 },
]

export const GUAXI_SYNTAX_BUCKETS = BUCKET_SPECS.map((spec) => spec.key)

// 角色色从上游样式表里按代表性类名取，顺序即优先级：
// 先取语义最贴合的类（number 之于橙色、comment 之于灰色），取不到才退到邻近类。
const ROLE_SOURCES = [
  ['base', ['hljs']],
  ['muted', ['hljs-comment', 'hljs-quote']],
  ['purple', ['hljs-keyword', 'hljs-doctag', 'hljs-formula', 'hljs-selector-tag']],
  ['green', ['hljs-string', 'hljs-addition']],
  ['orange', ['hljs-number', 'hljs-attr', 'hljs-attribute']],
  ['cyan', ['hljs-literal', 'hljs-symbol', 'hljs-bullet', 'hljs-subst']],
  ['blue', ['hljs-title.function_', 'hljs-title', 'hljs-link']],
  ['yellow', ['hljs-title.class_', 'hljs-built_in', 'hljs-type']],
  ['red', ['hljs-name', 'hljs-deletion', 'hljs-template-tag', 'hljs-tag']],
]

const readDeclaredColor = (body) => {
  for (const declaration of String(body || '').split(';')) {
    const colon = declaration.indexOf(':')
    if (colon < 0) continue
    // 精确匹配 color，排除 background-color / border-color
    if (declaration.slice(0, colon).trim().toLowerCase() !== 'color') continue

    const value = declaration.slice(colon + 1)
    const hash = value.indexOf('#')
    if (hash < 0) continue
    let hex = ''
    for (let i = hash + 1; i < value.length && i < hash + 10; i += 1) {
      const char = value[i]
      if (/[0-9a-fA-F]/.test(char)) hex += char
      else break
    }
    if (hex) return `#${hex.toLowerCase()}`
  }
  return ''
}

/** 把样式表拆成 [选择器, 颜色]；只收带 color 声明的普通规则，注释与 at-rule 跳过。 */
export const parseThemeRules = (css) => {
  const rules = []
  for (const chunk of String(css || '').replace(/\/\*[\s\S]*?\*\//g, '').split('}')) {
    const brace = chunk.indexOf('{')
    if (brace < 0) continue
    const selectors = chunk.slice(0, brace).split(',').map((item) => item.trim())
      .filter((item) => item && !item.startsWith('@'))
    if (!selectors.length) continue

    const color = readDeclaredColor(chunk.slice(brace + 1))
    if (!color) continue

    for (const selector of selectors) rules.push([selector, color])
  }
  return rules
}

// 选择器是否命中某个 hljs 类名。名字带点（hljs-title.class_）表示复合选择器，要求全部命中。
const selectorMatchesHljsClass = (selector, name) => {
  const wanted = name.split('.').filter(Boolean)
  const present = selector.split('.').filter(Boolean).map((part) => part.split(/[\s:>+~[\]()]/)[0])
  return wanted.every((item) => present.includes(item))
}

const resolveRoleColors = (rules) => {
  const roles = {}
  for (const [role, names] of ROLE_SOURCES) {
    for (const name of names) {
      const hit = rules.find(([selector]) => selectorMatchesHljsClass(selector, name))
      if (hit) {
        roles[role] = hit[1]
        break
      }
    }
  }
  return roles
}

// 解析出来的上游角色色往往不够用：Tokyo Night / Stack Overflow 这类主题本来就只用
// 两三个颜色覆盖全部语法元素，直接按类名取色会让关键字、函数、类名撞成同一个色。
//
// 处理原则是「先保主题，再拆撞色」：
// 1. 上游给了这个角色的颜色就保留，主题观感不变（Atom One / Panda 这类本来就够分的主题完全不动）；
// 2. 两个角色拿到同一个颜色时，后来者在**自己的色相内**改明度，而不是转色相——
//    转动色相会让「数字」变成红色这种离谱结果；
// 3. 角色整个缺失（主题没写那个类）才用标准色相 + 主题的平均饱和度/明度合成，并避开已用色相。
const ROLE_CANONICAL_HUES = {
  purple: 285,
  green: 140,
  blue: 215,
  yellow: 48,
  orange: 28,
  red: 0,
  cyan: 190,
}

// 先认领「多数主题都分得清」的角色，撞色时让后认领的改明度。
const ROLE_KEEP_ORDER = ['purple', 'green', 'blue', 'yellow', 'orange', 'red', 'cyan']

const MIN_HUE_GAP = 22
const MIN_COLOR_DISTANCE = 36

const hueDistance = (a, b) => {
  const diff = Math.abs((((a % 360) + 360) % 360) - (((b % 360) + 360) % 360))
  return Math.min(diff, 360 - diff)
}

const rgbDistance = (left, right) => {
  const a = hexToRgb(left)
  const b = hexToRgb(right)
  if (!a || !b) return 999
  return Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]) + Math.abs(a[2] - b[2])
}

const averageAccent = (roles) => {
  const samples = ROLE_KEEP_ORDER
    .map((role) => toHsl(roles[role]))
    .filter((hsl) => hsl && hsl[1] >= 12)
  if (!samples.length) return { saturation: 65, lightness: 60 }
  return {
    saturation: samples.reduce((sum, hsl) => sum + hsl[1], 0) / samples.length,
    lightness: samples.reduce((sum, hsl) => sum + hsl[2], 0) / samples.length,
  }
}

const synthesizeRole = (role, accent, usedHues) => {
  const target = ROLE_CANONICAL_HUES[role] ?? 200
  let chosen = target
  for (const offset of [0, 22, -22, 44, -44, 66, -66, 90, -90, 120, -120, 150, -150, 180]) {
    const candidate = (target + offset + 360) % 360
    if (usedHues.every((used) => hueDistance(used, candidate) >= MIN_HUE_GAP)) {
      chosen = candidate
      break
    }
  }
  return hslToHex([chosen, Math.max(accent.saturation, 48), clamp(accent.lightness, 46, 72)])
}

const separateByLightness = (hex, usedColors, appearance) => {
  if (!usedColors.some((used) => rgbDistance(used, hex) < MIN_COLOR_DISTANCE)) return hex
  const hsl = toHsl(hex)
  if (!hsl) return hex
  const direction = appearance === 'light' ? -1 : 1
  for (const step of [10, -10, 18, -18, 26, -26, 34, -34]) {
    const candidate = hslToHex([hsl[0], hsl[1], clamp(hsl[2] + step * direction, 24, 84)])
    if (!usedColors.some((used) => rgbDistance(used, candidate) < MIN_COLOR_DISTANCE)) {
      return candidate
    }
  }
  return hex
}

const finalizeRoleColors = (roles, appearance) => {
  const accent = averageAccent(roles)
  const usedColors = []
  const usedHues = []
  const resolved = {}

  for (const role of ['base', 'muted']) {
    if (!roles[role]) continue
    resolved[role] = roles[role]
    usedColors.push(roles[role])
    const hsl = toHsl(roles[role])
    if (hsl && hsl[1] >= 12) usedHues.push(hsl[0])
  }

  for (const role of ROLE_KEEP_ORDER) {
    let hex = roles[role]
    if (!hex) {
      hex = synthesizeRole(role, accent, usedHues)
    }
    hex = separateByLightness(hex, usedColors, appearance)
    usedColors.push(hex)
    const hsl = toHsl(hex)
    if (hsl && hsl[1] >= 12) usedHues.push(hsl[0])
    resolved[role] = hex
  }

  return resolved
}

/** 注释色不能等于正文色（Rosé Pine 暗色上游就是同一个灰度），否则整段注释等于没上色。 */
const ensureMutedContrast = (roles, appearance) => {
  const muted = toHsl(roles.muted)
  const base = toHsl(roles.base)
  if (!muted || !base) return roles
  if (Math.abs(muted[2] - base[2]) >= 10) return roles
  const delta = appearance === 'light' ? -14 : 14
  return { ...roles, muted: hslToHex([muted[0], muted[1], clamp(muted[2] + delta, 20, 84)]) }
}

const relativeLuminance = (hex) => {
  const rgb = hexToRgb(hex)
  if (!rgb) return 0
  const [r, g, b] = rgb.map((value) => {
    const channel = value / 255
    return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

const contrastRatio = (left, right) => {
  const a = relativeLuminance(left)
  const b = relativeLuminance(right)
  const lighter = Math.max(a, b)
  const darker = Math.min(a, b)
  return (lighter + 0.05) / (darker + 0.05)
}

const MIN_CONTRAST = 3

// 只在明度上做分档，同族的桶会变成一串「深一点的橙、更深的橙」，肉眼很难分。
// 所以在明度之外再带一个小角度色相偏移：变量（红）→ 成员（偏橙红）→ 参数（偏品红），
// 仍然是一家人，但同屏能一眼分开。
const SHADE_STEPS = {
  18: { hue: 15, light: 10 },
  9: { hue: 8, light: 5 },
  '-9': { hue: -8, light: -5 },
  '-18': { hue: -15, light: -10 },
}

const shadeColor = (hex, shade, appearance, background) => {
  if (!shade) return hex
  const hsl = toHsl(hex)
  if (!hsl) return hex
  const step = SHADE_STEPS[shade] || SHADE_STEPS[String(shade)]
  if (!step) return hex

  // 亮色主题统一往深里压，避免黄色/青色提亮后掉进白底；色相偏移继续保留，靠它拉区分度。
  const lightDelta = appearance === 'light' ? -(4 + Math.abs(step.light) * 0.3) : step.light
  const bounds = appearance === 'light' ? [16, 70] : [32, 86]
  // 明度一变，同样的饱和度观感会差很多：高饱和色压暗会变成霓虹蓝这种不属于主题的颜色，
  // 所以档位越远，饱和度收得越多。
  const saturation = clamp(hsl[1] - Math.abs(step.light) * 0.6, 28, 92)
  const render = (extraDarken = 0) => hslToHex([
    hsl[0] + step.hue,
    saturation,
    clamp(hsl[2] + lightDelta - extraDarken, bounds[0], bounds[1]),
  ])

  const candidate = render()
  if (contrastRatio(candidate, background) >= MIN_CONTRAST) return candidate
  // 对比度不够时朝「远离背景」的方向补：暗色主题提亮，亮色主题压暗。
  // 早先一律压暗，Monokai 那种高饱和紫色会被压成霓虹蓝，完全脱离主题。
  return render(appearance === 'light' ? 14 : -18)
}

/** 上游 `.hljs` 规则的底色，用来判断某个颜色在纸上够不够醒目；读不到就按明暗给默认值。 */
const readThemeBackground = (css, appearance) => {
  const block = String(css || '').replace(/\/\*[\s\S]*?\*\//g, '')
    .split('}')
    .find((chunk) => /\.hljs\s*\{/.test(chunk))
  if (block) {
    for (const declaration of block.slice(block.indexOf('{') + 1).split(';')) {
      const colon = declaration.indexOf(':')
      if (colon < 0) continue
      const property = declaration.slice(0, colon).trim().toLowerCase()
      if (property !== 'background' && property !== 'background-color') continue
      const match = declaration.slice(colon + 1).match(/#([0-9a-f]{3,6})/i)
      if (match) return `#${match[1]}`
    }
  }
  return appearance === 'light' ? '#ffffff' : '#282c34'
}

/**
 * 明度分档之后仍可能撞色（同一角色被别人先占了、或者对比度兜底把两个桶推到一起）。
 * 这里按桶的固定顺序再散一次：后到的往「离背景更远」的方向挪，保证同屏可区分。
 */
const dedupePalette = (palette, background, appearance) => {
  // punctuation 故意跟正文同色，不参与散色；base 排在最前，保证它拿到的是主题原色。
  const order = BUCKET_SPECS.map((spec) => spec.key).filter((key) => key !== 'punctuation')
  const used = []
  const resolved = { ...palette }
  // 明度上下限会卡住单向寻找（两个桶都贴到 86% 就再也分不开），所以两个方向都试；
  // 走满一圈还分不开就保留原色——硬挪只会挪到别人的颜色上，反而制造新的撞色。
  const steps = [7, -7, 14, -14, 21, -21, 28, -28, 35, -35, 42, -42, 49, -49]
  const collides = (color) => used.some((usedColor) => rgbDistance(usedColor, color) < MIN_COLOR_DISTANCE)

  for (const key of order) {
    const hsl = toHsl(resolved[key])
    if (!hsl) continue
    const original = resolved[key]
    let candidate = original
    for (const step of steps) {
      if (!collides(candidate)) break
      candidate = hslToHex([hsl[0], hsl[1], clamp(hsl[2] + step, 12, 92)])
    }
    resolved[key] = collides(candidate) ? original : candidate
    used.push(resolved[key])
  }
  return resolved
}

const paletteCache = new Map()

/**
 * 由一张上游样式表解析出完整语义色板；按「样式表 + 明暗 + 覆盖」缓存。
 *
 * `roleOverrides` 用于主题本身就是人工挑选的场景（Monokai / Gruvbox）：
 * 这些主题的 hljs 类名和我们的角色语义对不上（Gruvbox 的标签是青色、函数是绿色），
 * 直接按类名取色会让「变量」整族变成绿色，所以允许主题显式指定 9 个角色色。
 */
export const resolveSyntaxPalette = (css, appearanceTheme, roleOverrides = null) => {
  const appearance = appearanceTheme === 'light' ? 'light' : 'dark'
  const overrideKey = roleOverrides ? JSON.stringify(roleOverrides) : ''
  const cacheKey = `${appearance}\n${overrideKey}\n${String(css || '')}`
  const cached = paletteCache.get(cacheKey)
  if (cached) return cached

  const background = readThemeBackground(css, appearance)
  const parsedRoles = { ...resolveRoleColors(parseThemeRules(css)), ...(roleOverrides || {}) }
  const roles = ensureMutedContrast(finalizeRoleColors(parsedRoles, appearance), appearance)
  const palette = {}
  for (const spec of BUCKET_SPECS) {
    palette[spec.key] = shadeColor(roles[spec.role] || roles.base, spec.shade, appearance, background)
  }
  const resolved = dedupePalette(palette, background, appearance)

  paletteCache.set(cacheKey, resolved)
  return resolved
}

// hljs 类名 → 语义桶。这份表同时供「预览 CSS」和「Rust HTML 链路」使用：
// 表里出现的类即使 hljs 自己不会产出（hljs-method / hljs-namespace / hljs-decorator），
// 前端生成的样式表也会给它上色，探索者 Python cell 就能和编辑器落在同一个桶上。
const HLJS_CLASSES_BY_BUCKET = [
  ['base', ['hljs']],
  ['comment', ['hljs-comment', 'hljs-quote']],
  ['keyword', ['hljs-keyword', 'hljs-doctag', 'hljs-formula', 'hljs-selector-tag']],
  ['string', ['hljs-string', 'hljs-code', 'hljs-addition']],
  ['escape', ['hljs-subst']],
  ['regexp', ['hljs-regexp']],
  ['number', ['hljs-number']],
  ['boolean', ['hljs-literal']],
  ['type', ['hljs-type']],
  ['builtin', ['hljs-built_in', 'hljs-builtin-name']],
  ['function', ['hljs-title', 'hljs-title.function_', 'hljs-function', 'hljs-link']],
  ['method', ['hljs-method']],
  ['class', ['hljs-title.class_', 'hljs-section']],
  ['inherit', ['hljs-title.class_.inherited__']],
  ['parameter', ['hljs-params']],
  ['variable', ['hljs-variable']],
  ['variable-language', ['hljs-variable.language_']],
  ['constant', ['hljs-variable.constant_']],
  ['property', ['hljs-property']],
  ['attribute', ['hljs-attr', 'hljs-attribute']],
  ['namespace', ['hljs-namespace']],
  ['macro', ['hljs-meta', 'hljs-meta-keyword']],
  ['decorator', ['hljs-decorator', 'hljs-annotation']],
  ['label', ['hljs-label']],
  ['tag', ['hljs-name', 'hljs-tag', 'hljs-deletion']],
  ['operator', ['hljs-operator', 'hljs-symbol', 'hljs-bullet']],
  ['punctuation', ['hljs-punctuation']],
]

/**
 * token 颜色规则。`valueOf` 决定取 CSS 变量还是写死的色值：
 * - 页面内注入用变量（换主题只换变量，样式表只注入一次）；
 * - 导出 HTML 用写死色值（导出文件是独立的，没有 :root 变量可读）。
 */
const buildTokenColorCss = (valueOf) => {
  const rules = []
  for (const [bucket, classes] of HLJS_CLASSES_BY_BUCKET) {
    rules.push(`${classes.map((name) => `.${name}`).join(',')}{color:${valueOf(bucket)}}`)
  }
  // 预览代码块改用 tree-sitter 之后走 `guaxi-tok-*`，与上面是同一批桶。
  for (const bucket of GUAXI_SYNTAX_BUCKETS) {
    rules.push(`.guaxi-tok-${bucket}{color:${valueOf(bucket)}}`)
  }
  rules.push(
    '.hljs-comment,.hljs-quote,.guaxi-tok-comment{font-style:italic}',
    '.hljs-emphasis{font-style:italic}',
    '.hljs-strong{font-weight:700}',
  )
  return rules.join('\n')
}

/**
 * 预览侧颜色规则：值全部走 `--guaxi-syntax-*` 变量，所以这份 CSS 与主题无关，
 * 只需注入一次；换主题只换变量。
 */
export const buildSyntaxTokenCss = () => buildTokenColorCss((bucket) => `var(--guaxi-syntax-${bucket})`)

/** 导出/打印用：同样的规则，但把颜色写成字面色值（导出文件不读页面变量）。 */
export const buildSyntaxTokenCssForPalette = (palette) => {
  const safe = palette || {}
  const fallback = safe.base || '#000000'
  return buildTokenColorCss((bucket) => safe[bucket] || fallback)
}

/**
 * 上游样式表只保留「非颜色」声明（背景、内边距、斜体、下划线等）。
 *
 * 保留它们是为了不改变代码块底色和排版；颜色一律由上面的 token 规则接管，
 * 否则上游 `.hljs-title.function_` 这类复合选择器会和我们的颜色规则抢同一批元素。
 */
export const stripThemeColors = (css) => {
  const source = String(css || '')
  const blocks = []

  for (const chunk of source.replace(/\/\*[\s\S]*?\*\//g, '').split('}')) {
    const brace = chunk.indexOf('{')
    if (brace < 0) continue
    const selectors = chunk.slice(0, brace).trim()
    if (!selectors || selectors.startsWith('@')) continue

    const declarations = chunk.slice(brace + 1).split(';')
      .filter((declaration) => {
        const colon = declaration.indexOf(':')
        if (colon < 0) return false
        const property = declaration.slice(0, colon).trim().toLowerCase()
        if (!property) return false
        // 只丢 color，保留 background / background-color / border-color / font-style
        return property !== 'color'
      })
      .map((declaration) => declaration.trim())
      .filter(Boolean)

    if (!declarations.length) continue
    blocks.push(`${selectors}{${declarations.join(';')}}`)
  }

  return blocks.join('\n')
}
