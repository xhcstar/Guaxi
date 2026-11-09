import test from 'node:test'
import assert from 'node:assert/strict'

// 邮件正文的明暗配色映射。
//
// 这些断言锁的是「用户看得见的现象」而不是实现：
//   - 白卡片 / 浅灰页脚在暗色下不能变成纯黑（真实反馈过的「一块块黑板」）；
//   - 深色正文在暗色纸面上必须提亮，浅色文字不能被压暗；
//   - 品牌按钮这类中间调不能被当成「浅底」刷掉；
//   - 同一个元素被重复应用时结果要稳定（切换主题会重跑一次）。
import {
  parseCssColor,
  luminance,
  mapBackgroundColor,
  mapTextColor,
  mapBackgroundImage,
  gradientLuminance,
  DARK_PAPER,
} from '../src/components/ApplicationViewComponent/apps/mail/colors.js'

const explore = (css) => {
  const parsed = parseCssColor(css)
  assert.ok(parsed, `无法解析 ${css}`)
  return parsed
}

const lumOf = (css) => luminance(explore(css))

test('解析常见颜色写法', () => {
  assert.deepEqual(parseCssColor('#fff'), { r: 255, g: 255, b: 255, a: 1 })
  assert.deepEqual(parseCssColor('#F5F5F7'), { r: 245, g: 245, b: 247, a: 1 })
  assert.deepEqual(parseCssColor('rgb(29, 29, 31)'), { r: 29, g: 29, b: 31, a: 1 })
  assert.deepEqual(parseCssColor('rgba(0, 0, 0, 0.5)'), { r: 0, g: 0, b: 0, a: 0.5 })
  assert.deepEqual(parseCssColor('rgb( 0, 113, 227 )'), { r: 0, g: 113, b: 227, a: 1 })
  assert.deepEqual(parseCssColor('white'), { r: 255, g: 255, b: 255, a: 1 })
  assert.deepEqual(parseCssColor('transparent'), { r: 0, g: 0, b: 0, a: 0 })
  assert.equal(parseCssColor('currentColor'), null)
  assert.equal(parseCssColor('url(#x)'), null)
})

test('暗色下浅色底不能压成纯黑，且仍有层次', () => {
  const white = mapBackgroundColor('#ffffff')
  const footer = mapBackgroundColor('rgb(245, 245, 247)')
  assert.ok(white && footer)
  const whiteLum = lumOf(white)
  // 「不是黑板」的硬指标：亮度明显高于纯黑。
  assert.ok(whiteLum > 0.02, `白卡片映射后太暗：${white}`)
  // 白卡片要比页面纸面亮一档，才有卡片浮起的感觉。
  assert.ok(lumOf(white) > luminance(DARK_PAPER), `白卡片应比纸面亮：${white}`)
  // 两个本来不同的浅色，压暗后仍需可区分。
  assert.notEqual(white, footer)
})

test('彩色浅底保留色相', () => {
  const blue = mapBackgroundColor('#e8f0fe')
  assert.ok(blue)
  const { r, g, b } = explore(blue)
  assert.ok(b > r, `浅蓝卡片映射后应仍偏蓝：${blue}`)
})

test('品牌色按钮这类中间调保持原样', () => {
  assert.equal(mapBackgroundColor('rgb(0, 113, 227)'), null)
  assert.equal(mapBackgroundColor('rgba(122, 162, 247, 0.14)'), null)
})

test('深色文字在暗色纸面上被提亮', () => {
  const heading = mapTextColor('rgb(29, 29, 31)', luminance(DARK_PAPER))
  const muted = mapTextColor('rgb(110, 110, 115)', luminance(DARK_PAPER))
  assert.ok(heading && muted)
  assert.ok(lumOf(heading) > 0.7, `标题应接近白色：${heading}`)
  // 原来的次要灰要变成中灰，仍然低于主文字——层次不能抹平。
  assert.ok(lumOf(muted) > 0.3 && lumOf(muted) < lumOf(heading))
})

test('浅色文字已经适配深色底，不再改动', () => {
  assert.equal(mapTextColor('#ffffff', luminance(DARK_PAPER)), null)
  assert.equal(mapTextColor('rgb(200, 200, 200)', luminance(DARK_PAPER)), null)
})

test('彩色文字保留色相只提亮', () => {
  const link = mapTextColor('#0066cc', luminance(DARK_PAPER))
  assert.ok(link)
  const { r, g, b } = explore(link)
  assert.ok(b > g && b > r)
  assert.ok(lumOf(link) > lumOf('#0066cc'))
})

test('压在仍偏亮的色块上的文字不动', () => {
  assert.equal(mapTextColor('rgb(29, 29, 31)', 0.6), null)
})

// Word / Outlook 的表格边框不写十六进制，直接写 `border: 1pt solid windowtext`。
// 这个关键字过去解析不出来，映射层整个跳过，边框留给浏览器按系统主题解释：
// 系统浅色主题下它是黑色，落在暗色纸面上就等于没有——用户看到的「表格边框缺失」。
test('Outlook 的系统色边框参与暗色映射，不再落成黑色', async () => {
  const { mapBorderColor } = await import(
    '../src/components/ApplicationViewComponent/apps/mail/colors.js'
  )
  assert.deepEqual(parseCssColor('windowtext'), { r: 0, g: 0, b: 0, a: 1 })
  assert.deepEqual(parseCssColor('WindowText'), { r: 0, g: 0, b: 0, a: 1 })
  const line = mapBorderColor('windowtext')
  assert.ok(line, 'windowtext 必须给出映射结果，否则边框会留给系统主题')
  // 结果要和「邮件里本来就写 #ccc」的那类边框落在同一档，表格才不会出现断线。
  const gray = mapBorderColor('rgb(204, 204, 204)')
  assert.ok(gray)
  assert.ok(Math.abs(lumOf(line) - lumOf(gray)) < 0.06, `${line} 与 ${gray} 亮度差太多`)
  // 比纸面亮一点才有分隔感，但又不能亮成一条白线。
  assert.ok(lumOf(line) > luminance(DARK_PAPER), `边框比纸面还暗：${line}`)
  assert.ok(lumOf(line) < 0.2, `边框太扎眼：${line}`)
  // currentColor 跟着元素自己的文字色走，不能提前定值。
  assert.equal(parseCssColor('currentcolor'), null)
})


test('亮色主题不做任何改写', () => {
  assert.equal(mapBackgroundColor('#ffffff', 'light'), null)
  assert.equal(mapTextColor('rgb(29, 29, 31)', 0.9, 'light'), null)
  assert.equal(mapBackgroundImage('linear-gradient(#fff, #f5f5f7)', 'light'), null)
})

test('渐变背景逐色标映射', () => {
  const mapped = mapBackgroundImage('linear-gradient(180deg, #ffffff 0%, #f5f5f7 100%)')
  assert.ok(mapped)
  assert.ok(!/#fff|#f5f5f7|rgb\(255/i.test(mapped), `浅色渐变应被改写：${mapped}`)
  // 色标位置保留，避免渐变方向 / 分界跑掉。
  assert.ok(mapped.includes('0%') && mapped.includes('100%'))
  // 纯品牌色渐变不动。
  assert.equal(mapBackgroundImage('linear-gradient(#0071e3, #0066cc)'), null)
})

test('渐变亮度：近白的页脚算浅色', () => {
  assert.ok(gradientLuminance('linear-gradient(#f6f6f6, #ffffff)') > 0.9)
  assert.equal(gradientLuminance('none'), null)
})

test('重复应用结果稳定（切主题时会重跑）', () => {
  const once = mapBackgroundColor('#ffffff')
  // 映射出来的已经是一个深色表面：再来一次必须返回「不动」，而不是继续压暗。
  assert.equal(mapBackgroundColor(once), null, '已映射的表面不应被再次改写')
  const textOnce = mapTextColor('rgb(29, 29, 31)', luminance(DARK_PAPER))
  assert.equal(mapTextColor(textOnce, luminance(DARK_PAPER)), null, '已提亮的文字不应再被改写')
})

// 重复应用必须幂等：MailReader 在换信、切换主题、显示图片时都会重跑一次映射，
// 如果第二次从「映射后的值」再算一遍，深色文字会一路变亮、浅色表面会一路变暗。
// 这里用一个最小 DOM 桩跑真实的 applyBodyTheme 两遍，断言两次结果一致。
test('applyBodyTheme 对同一份 DOM 重复执行结果不变', async () => {
  const { applyBodyTheme } = await import(
    '../src/components/ApplicationViewComponent/apps/mail/colors.js'
  )
  const makeStyle = () => {
    const store = {}
    return {
      get backgroundColor() {
        return store.bg || ''
      },
      set backgroundColor(v) {
        store.bg = v
      },
      get backgroundImage() {
        return store.img || ''
      },
      set backgroundImage(v) {
        store.img = v
      },
      get color() {
        return store.fg || ''
      },
      set color(v) {
        store.fg = v
      },
      removeProperty(name) {
        if (name === 'background-color') delete store.bg
        if (name === 'background-image') delete store.img
        if (name === 'color') delete store.fg
      },
    }
  }
  const makeEl = (attrs = {}, style = {}) => {
    const attributes = { ...attrs }
    const api = {
      style: makeStyle(),
      hasAttribute: (name) => name in attributes,
      getAttribute: (name) => (name in attributes ? attributes[name] : null),
      setAttribute: (name, value) => {
        attributes[name] = value
      },
      removeAttribute: (name) => {
        delete attributes[name]
      },
      classList: { contains: () => false },
      parentElement: null,
    }
    Object.assign(api.style, style)
    return api
  }

  // 卡片：白底 + 深色标题；页脚：浅灰底 + 中灰字；页脚 banner：近白渐变。
  const title = makeEl({}, { color: 'rgb(29, 29, 31)' })
  const note = makeEl({}, { color: '#ffffff' })
  const card = makeEl({}, { backgroundColor: '#ffffff' })
  const footer = makeEl({}, { backgroundColor: 'rgb(245, 245, 247)' })
  const gradient = makeEl(
    {},
    { backgroundImage: 'linear-gradient(rgb(246, 246, 246) 0%, rgb(255, 255, 255) 59%)' },
  )
  // 旧式 <font color>：颜色写在属性里，映射必须走内联 style（属性写 rgb() 会变绿）。
  const legacyFont = makeEl({ color: '#212121' })
  const body = makeEl({ class: 'mail-body' })
  const all = [body, card, title, note, footer, gradient, legacyFont]
  const root = {
    querySelectorAll: (selector) => (selector === '.mail-body' ? [body] : all),
  }
  body.querySelectorAll = () => all

  globalThis.getComputedStyle = (el) => ({
    backgroundColor: el.style.backgroundColor || 'rgba(0, 0, 0, 0)',
    backgroundImage: el.style.backgroundImage || 'none',
    color: el.style.color || '',
  })

  applyBodyTheme(root, 'dark')
  const first = {
    card: card.style.backgroundColor,
    footer: footer.style.backgroundColor,
    gradient: gradient.style.backgroundImage,
    title: title.style.color,
    note: note.style.color,
  }
  applyBodyTheme(root, 'dark')
  const second = {
    card: card.style.backgroundColor,
    footer: footer.style.backgroundColor,
    gradient: gradient.style.backgroundImage,
    title: title.style.color,
    note: note.style.color,
  }
  assert.deepEqual(second, first, '第二次应用不能改变已有结果')
  // 纯白/近白的段落底和渐变不再铺成「一层层表面」，直接透出纸面。
  assert.equal(first.card, 'transparent', '白底段落应直接透明')
  assert.equal(first.footer, 'transparent', '近白页脚也透明')
  assert.equal(first.gradient, 'none', '近白渐变不应再画出色带')
  assert.ok(lumOf(first.title) > 0.7, `标题应被提亮：${first.title}`)
  assert.equal(first.note, '#ffffff', '白字本来就适合深底，不能再改')
  assert.ok(lumOf(legacyFont.style.color) > 0.7, 'font color 属性也要提亮')
  assert.equal(legacyFont.getAttribute('color'), '#212121', 'color 属性不能写 rgb()')

  // 切回亮色：原值要能还原，不能留下暗色的痕迹。
  applyBodyTheme(root, 'light')
  assert.equal(card.style.backgroundColor, '#ffffff')
  assert.equal(gradient.style.backgroundImage, 'linear-gradient(rgb(246, 246, 246) 0%, rgb(255, 255, 255) 59%)')
  assert.equal(title.style.color, 'rgb(29, 29, 31)')
  assert.equal(legacyFont.style.color, '', 'font 的内联覆盖应被移除')
  assert.equal(legacyFont.getAttribute('color'), '#212121')
})

test('邮件里写死的浅色边框在暗色下被压成细线，切回亮色能还原', async () => {
  const { applyBodyTheme, mapBorderColor, luminance, parseCssColor } = await import(
    '../src/components/ApplicationViewComponent/apps/mail/colors.js'
  )
  // 浅色边框必须压到「纸面」和「浅色表面」之间：比纸面能看出线，又不会发白。
  const paper = luminance(parseCssColor('#23262b'))
  const mapped = mapBorderColor('#f1f1f1')
  assert.ok(mapped, '浅色边框应当被映射')
  const lum = luminance(parseCssColor(mapped))
  assert.ok(lum > paper, `边框应比纸面亮一点：${mapped}`)
  assert.ok(lum < 0.05, `边框不能亮成一块白边：${mapped}`)
  // 深色边框：在深色纸面上本来「看不见」，抬到刚好能分辨的位置，但仍贴近纸面。
  const dark = mapBorderColor('#333333')
  assert.ok(dark, '深色边框也要抬一点，才有分隔感')
  assert.ok(luminance(parseCssColor(dark)) < 0.035, `深色边框不能抬高成亮线：${dark}`)

  // 中间灰的边框是最容易漏的一类：`#888a85`（工单系统模板常见）在浅色纸面上很克制，
  // 压到深色纸面上就是一条明晃晃的亮线。它必须被压到纸面附近。
  const midGrey = mapBorderColor('#888a85')
  assert.ok(midGrey, '中间灰边框必须映射')
  assert.ok(luminance(parseCssColor(midGrey)) < 0.035, `中间灰不能保持亮线：${midGrey}`)

  // 彩色边框保留色相，只压亮度。
  const blue = mapBorderColor('#0071e3')
  assert.ok(blue, '彩色边框也要压暗')
  assert.ok(luminance(parseCssColor(blue)) < 0.04, `彩色边框不能过亮：${blue}`)

  // 端到端：<table style="border:1px solid #f1f1f1"> 在暗色下被改写，亮色下还原。
  const style = { borderTopColor: 'rgb(241, 241, 241)', borderRightColor: 'rgb(241, 241, 241)',
    borderBottomColor: 'rgb(241, 241, 241)', borderLeftColor: 'rgb(241, 241, 241)',
    borderTopStyle: 'solid', borderRightStyle: 'solid', borderBottomStyle: 'solid',
    borderLeftStyle: 'solid' }
  // 真实 DOM 里 setAttribute / getAttribute 是配对的；桩也要配对，
  // 否则「切回亮色还原」这条路径根本跑不到。
  const attrs = new Map()
  const el = {
    style,
    hasAttribute: (k) => attrs.has(k),
    getAttribute: (k) => (attrs.has(k) ? attrs.get(k) : null),
    setAttribute: (k, v) => attrs.set(k, String(v)),
    removeAttribute: (k) => attrs.delete(k),
    classList: { contains: () => false },
    parentElement: null,
  }
  const body = { classList: { contains: () => true }, style: {}, parentElement: null, querySelectorAll: () => [el] }
  const root = { querySelectorAll: () => [body] }
  globalThis.getComputedStyle = () => ({
    backgroundColor: 'rgba(0, 0, 0, 0)', backgroundImage: 'none', color: '',
  })

  applyBodyTheme(root, 'dark')
  assert.ok(style.borderBottomColor.startsWith('rgb('), '边框色应被改写')
  assert.notEqual(style.borderBottomColor, 'rgb(241, 241, 241)')
  applyBodyTheme(root, 'light')
  assert.equal(style.borderBottomColor, 'rgb(241, 241, 241)', '切回亮色要还原原边框')
})

test('近白灰底（#f5f5f7 / #fafafa）在暗色下透出纸面，不再叠成暗色块', async () => {
  const { applyBodyTheme } = await import(
    '../src/components/ApplicationViewComponent/apps/mail/colors.js'
  )
  // 真实邮件（工单系统 / Outlook）会给整封或每个段落套一层近白灰底。
  // 这类底色没有设计含义；映射成「比纸面亮一档的表面」就会逐层叠成暗色块。
  const makeEl = (bg) => {
    const attrs = new Map()
    const style = { backgroundColor: bg }
    return {
      style,
      hasAttribute: (k) => attrs.has(k),
      getAttribute: (k) => (attrs.has(k) ? attrs.get(k) : null),
      setAttribute: (k, v) => attrs.set(k, String(v)),
      removeAttribute: (k) => attrs.delete(k),
      classList: { contains: () => false },
      parentElement: null,
    }
  }
  const nearWhite = makeEl('rgb(245, 245, 247)')
  const nearWhite2 = makeEl('#fafafa')
  const white = makeEl('#ffffff')
  // 有颜色含义的浅底仍要映射成深色表面（米色 banner）。
  const beige = makeEl('#f3efe6')
  const all = [nearWhite, nearWhite2, white, beige]
  const body = { classList: { contains: () => true }, style: {}, parentElement: null, querySelectorAll: () => all }
  globalThis.getComputedStyle = (el) => ({
    backgroundColor: el.style.backgroundColor || 'rgba(0, 0, 0, 0)',
    backgroundImage: 'none',
    color: '',
  })

  applyBodyTheme({ querySelectorAll: () => [body] }, 'dark')
  assert.equal(nearWhite.style.backgroundColor, 'transparent', '#f5f5f7 应透出纸面')
  assert.equal(nearWhite2.style.backgroundColor, 'transparent', '#fafafa 应透出纸面')
  assert.equal(white.style.backgroundColor, 'transparent', '#ffffff 应透出纸面')
  assert.notEqual(beige.style.backgroundColor, 'transparent', '有色浅底仍要映射成表面')
})

test('写信区：引用卡片用同一套映射，序列化前能还原成邮件原值', async () => {
  const { applyBodyThemeToElements } = await import(
    '../src/components/ApplicationViewComponent/apps/mail/colors.js'
  )
  // 引用原文是发件人按浅色纸面写的：白卡片 + 深色字。放进深色编辑器里就是
  // 一整块白底浅字（新窗口看起来和阅读区完全不一样）。
  const makeEl = (style) => {
    const attrs = new Map()
    const store = { ...style }
    return {
      style: {
        get backgroundColor() {
          return store.backgroundColor || ''
        },
        set backgroundColor(v) {
          store.backgroundColor = v
        },
        get backgroundImage() {
          return store.backgroundImage || ''
        },
        set backgroundImage(v) {
          store.backgroundImage = v
        },
        get color() {
          return store.color || ''
        },
        set color(v) {
          store.color = v
        },
        removeProperty(name) {
          if (name === 'background-color') delete store.backgroundColor
          if (name === 'background-image') delete store.backgroundImage
          if (name === 'color') delete store.color
        },
      },
      hasAttribute: (k) => attrs.has(k),
      getAttribute: (k) => (attrs.has(k) ? attrs.get(k) : null),
      setAttribute: (k, v) => attrs.set(k, String(v)),
      removeAttribute: (k) => attrs.delete(k),
      classList: { contains: () => false },
      parentElement: null,
    }
  }
  const card = makeEl({ backgroundColor: 'rgba(127, 127, 127, 0.07)' })
  const whiteBody = makeEl({ backgroundColor: '#ffffff' })
  const darkText = makeEl({ color: '#222222' })
  const elements = [card, whiteBody, darkText]
  globalThis.getComputedStyle = (el) => ({
    backgroundColor: el.style.backgroundColor || 'rgba(0, 0, 0, 0)',
    backgroundImage: 'none',
    color: '',
  })

  applyBodyThemeToElements(elements, 'dark')
  assert.equal(whiteBody.style.backgroundColor, 'transparent', '白底要透出深色编辑区')
  assert.ok(luminance(parseCssColor(darkText.style.color)) > 0.5, '深色字要提亮')

  // 序列化草稿 / 发送前必须还原：收件端看到的还是原邮件的配色。
  applyBodyThemeToElements(elements, 'light')
  assert.equal(whiteBody.style.backgroundColor, '#ffffff')
  assert.equal(darkText.style.color, '#222222')
  assert.equal(card.style.backgroundColor, 'rgba(127, 127, 127, 0.07)')
})

// 现象（用户截图）：暗色下工单 / Word 生成的表格整张没有框线。
// 原因：浅色单元格的底色在暗色下被抬到纸面上方一档（约 #2c2d33），
// 而边框仍按「贴近纸面」压暗（≈ #2e2e2e）——线比自己的底色还暗，等于没画。
test('浅色单元格里的边框必须比映射后的单元格表面亮，否则整张表像没有框线', async () => {
  const { mapBorderColor, mapBackgroundColor } = await import(
    '../src/components/ApplicationViewComponent/apps/mail/colors.js'
  )
  // 单元格最典型的浅色底：纯白 / #fafafa / #f5f5f7，映射后落在同一档表面。
  const surface = lumOf(mapBackgroundColor('#ffffff'))
  assert.ok(surface > luminance(DARK_PAPER), '先决条件：浅色单元格映射后要比纸面亮一档')
  // Word / Outlook 的 `border: 1pt solid windowtext` 与常见浅灰边框都要看得见。
  for (const original of ['windowtext', '#dddddd', '#eeeeee', '#888a85']) {
    const line = mapBorderColor(original, 'dark', { overLightSurface: true })
    assert.ok(line, `${original} 应给出映射结果`)
    const lum = lumOf(line)
    assert.ok(
      lum > surface + 0.02,
      `${original} 映射成 ${line} 后仍比单元格表面暗：线会消失`,
    )
    // 也不能亮成一条白线：仍然要比正文文字暗。
    assert.ok(lum < 0.25, `${original} 映射成 ${line} 太扎眼`)
  }
  // 没压在浅色表面上的边框保持原来的克制映射，避免整封邮件到处都是亮边。
  assert.equal(mapBorderColor('#dddddd'), mapBorderColor('#dddddd', 'dark', { overLightSurface: false }))
  assert.ok(lumOf(mapBorderColor('#dddddd')) < surface, '纸面上的细线仍要贴近纸面')
})
