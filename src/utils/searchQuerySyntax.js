/**
 * 搜索框的查询语法。
 *
 * 用户在一个输入框里同时表达"找什么"和"在哪找"：
 *
 *     配置 加载 ext:md,txt after:7d path:src tag:待办
 *     >重建索引
 *
 * 为什么不做成纯 UI 筛选面板：面板要点四五次，键盘用户还得离开输入框。
 * 记号能一口气敲完，也能从历史记录里整条复用。
 *
 * 这里只做纯粹的字符串解析——不碰 Vue、不碰 invoke，方便单独验证。
 */

/** 认得的记号键。别名放在一起，用户不用记"到底是 ext 还是 type"。 */
const FILTER_KEYS = {
  ext: 'ext',
  type: 'ext',
  tag: 'tag',
  tags: 'tag',
  path: 'path',
  in: 'path',
  dir: 'path',
  after: 'after',
  since: 'after',
  before: 'before',
  until: 'before',
}

/** 命令面板前缀。放在最前面才算，`a>b` 不是命令。 */
export const COMMAND_PREFIX = '>'

const DAY = 86400000

/**
 * 把一行输入切成 token，尊重引号。
 *
 * `path:"我的 笔记"` 得当成一个 token——目录名带空格太常见了，
 * 按空白硬切会把它拆成两半，然后 `笔记"` 变成搜索词。
 */
function tokenize(raw) {
  const tokens = []
  let current = ''
  let quote = null
  let hasQuoted = false

  const flush = () => {
    if (current || hasQuoted) tokens.push(current)
    current = ''
    hasQuoted = false
  }

  for (const ch of raw) {
    if (quote) {
      if (ch === quote) {
        quote = null
        hasQuoted = true
      } else {
        current += ch
      }
      continue
    }
    if (ch === '"' || ch === "'") {
      quote = ch
      continue
    }
    if (ch === ' ' || ch === '\t' || ch === '\n') {
      flush()
      continue
    }
    current += ch
  }
  flush()

  return tokens
}

/** 归一化扩展名：去掉前导点、统一小写。用户敲 `.MD` 和 `md` 是一个意思。 */
function normalizeExt(value) {
  return value.trim().replace(/^\./, '').toLowerCase()
}

/**
 * 解析时间值，返回毫秒时间戳。
 *
 * 支持 `2026-01-31`、`2026/01/31`、`today`、`yesterday`，
 * 以及 `7d` / `2w` / `3m` / `1y` 这种相对写法——"最近一周"比算日期快得多。
 *
 * `edge` 决定日期只写到天时取当天的哪一头：after 取 00:00，before 取 23:59:59。
 */
export function parseTimeValue(value, edge, now = Date.now()) {
  const text = value.trim().toLowerCase()
  if (!text) return null

  if (text === 'today') {
    const d = new Date(now)
    d.setHours(0, 0, 0, 0)
    return edge === 'before' ? d.getTime() + DAY - 1 : d.getTime()
  }
  if (text === 'yesterday') {
    const d = new Date(now)
    d.setHours(0, 0, 0, 0)
    const start = d.getTime() - DAY
    return edge === 'before' ? start + DAY - 1 : start
  }

  const relative = /^(\d+)\s*([dwmy])$/.exec(text)
  if (relative) {
    const n = Number(relative[1])
    const unit = relative[2]
    const span = unit === 'd' ? DAY : unit === 'w' ? 7 * DAY : unit === 'm' ? 30 * DAY : 365 * DAY
    return now - n * span
  }

  const ymd = /^(\d{4})[-/.](\d{1,2})(?:[-/.](\d{1,2}))?$/.exec(text)
  if (ymd) {
    const year = Number(ymd[1])
    const month = Number(ymd[2]) - 1
    // 只写到月份时，after 取当月 1 号，before 取月末
    const hasDay = ymd[3] !== undefined
    const day = hasDay ? Number(ymd[3]) : 1
    const d = new Date(year, month, day, 0, 0, 0, 0)
    if (Number.isNaN(d.getTime())) return null
    if (edge !== 'before') return d.getTime()
    if (hasDay) return d.getTime() + DAY - 1
    return new Date(year, month + 1, 1, 0, 0, 0, 0).getTime() - 1
  }

  return null
}

/** 一个值可能用逗号带多项：`ext:md,txt`。 */
function splitValues(value) {
  return value
    .split(',')
    .map((v) => v.trim())
    .filter(Boolean)
}

function pushUnique(list, value) {
  if (value && !list.includes(value)) list.push(value)
}

/**
 * 解析整条查询。
 *
 * 返回的 `text` 是剥掉记号之后真正要搜的词——这一步很关键：
 * 不剥的话 `ext:md 配置` 会被当成字面量丢给 grep，一条结果都搜不到。
 *
 * 不认识的键（`title:`、`C:`、`http://`）原样留在 `text` 里：
 * 前者要交给 tantivy 的查询语法，后者本来就是用户想搜的字符串。
 */
export function parseSearchQuery(raw, { now = Date.now() } = {}) {
  const result = {
    text: '',
    exts: [],
    tags: [],
    paths: [],
    after: null,
    before: null,
    command: null,
    tokens: [],
    invalid: [],
  }

  const input = typeof raw === 'string' ? raw : ''
  const trimmedStart = input.trimStart()

  // 命令模式整条都归命令面板，不再解析筛选记号
  if (trimmedStart.startsWith(COMMAND_PREFIX)) {
    result.command = trimmedStart.slice(COMMAND_PREFIX.length).trim()
    return result
  }

  const words = []

  for (const token of tokenize(input)) {
    const at = token.indexOf(':')
    const key = at > 0 ? FILTER_KEYS[token.slice(0, at).toLowerCase()] : undefined
    if (!key) {
      words.push(token)
      continue
    }

    const rawValue = token.slice(at + 1)
    // 用户才敲到 `ext:`，值还没输入：记号先吞掉，别当搜索词
    if (!rawValue.trim()) {
      result.tokens.push({ key, value: '', raw: token, pending: true })
      continue
    }

    if (key === 'ext') {
      for (const v of splitValues(rawValue)) pushUnique(result.exts, normalizeExt(v))
    } else if (key === 'tag') {
      for (const v of splitValues(rawValue)) pushUnique(result.tags, v)
    } else if (key === 'path') {
      // 反斜杠统一成正斜杠：后端给的 rel_path 是正斜杠的
      pushUnique(result.paths, rawValue.trim().replace(/\\/g, '/'))
    } else {
      const ms = parseTimeValue(rawValue, key, now)
      if (ms === null) {
        // 时间写错了就当普通词——总比静默丢掉一个筛选条件好
        result.invalid.push({ key, value: rawValue })
        words.push(token)
        continue
      }
      result[key] = ms
    }

    result.tokens.push({ key, value: rawValue.trim(), raw: token })
  }

  result.text = words.join(' ')
  return result
}

/**
 * 结果是否落在 `path:` 限定的目录里。
 *
 * 后端没有路径前缀参数，这一条只能在前端筛。子串匹配而不是前缀匹配：
 * 用户敲 `path:components` 想的是"路径里带 components 的"，
 * 不是"必须从 components 开头"。
 */
export function matchesPathFilters(relPath, paths) {
  if (!paths || paths.length === 0) return true
  const hay = String(relPath || '').replace(/\\/g, '/').toLowerCase()
  return paths.every((p) => hay.includes(p.toLowerCase()))
}
