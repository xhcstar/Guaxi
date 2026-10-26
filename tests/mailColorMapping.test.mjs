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

  // 卡片：白底 + 深色标题；页脚：浅灰底 + 中灰字。
  const title = makeEl({}, { color: 'rgb(29, 29, 31)' })
  const note = makeEl({}, { color: '#ffffff' })
  const card = makeEl({}, { backgroundColor: '#ffffff' })
  const footer = makeEl({}, { backgroundColor: 'rgb(245, 245, 247)' })
  const body = makeEl({ class: 'mail-body' })
  const all = [body, card, title, note, footer]
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
    title: title.style.color,
    note: note.style.color,
  }
  applyBodyTheme(root, 'dark')
  const second = {
    card: card.style.backgroundColor,
    footer: footer.style.backgroundColor,
    title: title.style.color,
    note: note.style.color,
  }
  assert.deepEqual(second, first, '第二次应用不能改变已有结果')
  assert.ok(lumOf(first.title) > 0.7, `标题应被提亮：${first.title}`)
  assert.equal(first.note, '#ffffff', '白字本来就适合深底，不能再改')

  // 切回亮色：原值要能还原，不能留下暗色的痕迹。
  applyBodyTheme(root, 'light')
  assert.equal(card.style.backgroundColor, '#ffffff')
  assert.equal(title.style.color, 'rgb(29, 29, 31)')
})
