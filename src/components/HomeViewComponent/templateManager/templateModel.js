// 模板管理器的纯数据层。
//
// 和 resourceRepoModel 同样的理由：合并内置/自定义、判定语言、分类归并、全文检索这几条规则
// 决定了「列表里出现什么、搜索命中的是什么、保存时哪个名字冲突」，值得在 Node 里直接断言，
// 不必每次都开窗口点一遍。

/** 与编辑器一致的判断：Python Cell 的模板正文是纯 Python，不是 Markdown。 */
export const PYTHON_TEMPLATE_RE = /python\s*cell|python\s*模板|^python$/i

/** 分类路径的分隔符。Python Cell 那批模板用 `探索者 / Python Cell / 数据读取` 这种层级。 */
export const TYPE_PATH_SEP = ' / '

export const UNCATEGORIZED = '未分类'

export const TEMPLATE_NAME_MAX = 80

export const normalizeTemplateName = (value) => String(value ?? '').trim()

export const normalizeTemplateType = (value) => {
  const t = String(value ?? '').trim()
  return t || UNCATEGORIZED
}

/**
 * 名称校验。
 *
 * 后端只挡 `/ \ ..`，但模板最终落成 `<名称>.md`，Windows 文件名里的 `< > : " | ? *`
 * 会让写入直接失败并弹一个 ENOENT 之类的系统错误。这里提前拦下，报人话。
 */
export const validateTemplateName = (name) => {
  const value = normalizeTemplateName(name)
  if (!value) return { ok: false, reason: '模板名称不能为空' }
  if (value.length > TEMPLATE_NAME_MAX) {
    return { ok: false, reason: `模板名称不能超过 ${TEMPLATE_NAME_MAX} 个字符` }
  }
  if (/[\\/]/.test(value)) return { ok: false, reason: '模板名称不能包含 / 或 \\' }
  if (value.includes('..')) return { ok: false, reason: '模板名称不能包含 ..' }
  if (/[<>:"|?*\u0000-\u001f]/.test(value)) {
    return { ok: false, reason: '模板名称不能包含 < > : " | ? * 等字符' }
  }
  return { ok: true, value }
}

/** 正文语言：类型名或模板名里带 python 的，按 Python 高亮与 LSP 处理。 */
export const templateKindOf = ({ name = '', type = '' } = {}) =>
  PYTHON_TEMPLATE_RE.test(String(type)) || PYTHON_TEMPLATE_RE.test(String(name)) ? 'python' : 'markdown'

/** 一级分类。工具栏的筛选 chip 按它分组，避免 Python Cell 的 20 个子类占满一行。 */
export const templateFamilyOf = (type) => {
  const t = normalizeTemplateType(type)
  // 已知的内置模板都会写成「分组 / 子类」（两级），所以一级分类取前两段；
  // 用户自定义的 `工作 / 周` 这类单层路径按第一段算。
  const parts = t.split(TYPE_PATH_SEP).map((segment) => segment.trim()).filter(Boolean)
  if (parts.length <= 1) return t
  return isKnownTwoLevelFamily(parts) ? parts.slice(0, 2).join(TYPE_PATH_SEP) : parts[0]
}

/** 内置 Python Cell 模板的分类形如「探索者 / Python Cell / 数据读取」。 */
const isKnownTwoLevelFamily = (parts) => parts.length >= 3 && parts[0] === '探索者'

/** 二级分类（没有就返回空串）。 */
export const templateVariantOf = (type) => {
  const t = normalizeTemplateType(type)
  const family = templateFamilyOf(t)
  if (family === t) return ''
  return t.slice(family.length + TYPE_PATH_SEP.length).trim()
}

export const templateFileOf = (name) => {
  const n = normalizeTemplateName(name)
  if (!n) return ''
  return /\.(md|markdown)$/i.test(n) ? n : `${n}.md`
}

const sortByName = (a, b) => {
  const left = a.name || ''
  const right = b.name || ''
  try {
    return left.localeCompare(right, 'zh-Hans-CN', { numeric: true, sensitivity: 'base' })
  } catch {
    return left.localeCompare(right)
  }
}

/**
 * 合并内置与自定义模板。
 *
 * 同名时自定义覆盖内置，但保留 `overriddenBuiltin` 标记——界面上要能提示
 * 「这份内置模板已被你的版本替换」，并给一个「恢复内置版本」的入口。
 *
 * @param {Array<{name:string, templateType:string, content:string}>} builtin
 * @param {Array<{name:string, template_type:string}>} user
 * @param {Map<string,string>} [contents] 已读取的自定义模板正文
 */
export const mergeTemplates = (builtin = [], user = [], contents = new Map()) => {
  const byName = new Map()
  const builtinNames = new Set()

  for (const raw of builtin || []) {
    const name = normalizeTemplateName(raw?.name)
    if (!name) continue
    builtinNames.add(name)
    byName.set(name, {
      name,
      type: normalizeTemplateType(raw?.templateType ?? raw?.template_type),
      content: String(raw?.content ?? ''),
      builtin: true,
      overriddenBuiltin: false,
      loaded: true,
    })
  }

  for (const raw of user || []) {
    const name = normalizeTemplateName(raw?.name)
    if (!name) continue
    const previous = byName.get(name)
    const content = contents?.get(name)
    const loaded = typeof content === 'string'
    byName.set(name, {
      name,
      type: normalizeTemplateType(raw?.template_type ?? raw?.templateType),
      // 覆盖内置时不能拿内置正文顶包：那不是用户手里的那份。
      content: loaded ? content : '',
      builtin: false,
      overriddenBuiltin: Boolean(previous?.builtin) || builtinNames.has(name),
      loaded,
    })
  }

  const list = [...byName.values()].map((item) => ({ ...item, kind: templateKindOf(item) }))
  return list.sort(sortByName)
}

/** 分类排序：先按调用方给的顺序（内置模板的既有顺序），未知分类按名称排在后面。 */
export const typeRankOf = (type, typeOrder = []) => {
  const t = normalizeTemplateType(type)
  const index = typeOrder.indexOf(t)
  return index >= 0 ? index : typeOrder.length
}

/** 把模板按完整分类分组，组内按名称排。 */
export const groupTemplates = (list = [], typeOrder = []) => {
  const map = new Map()
  for (const item of list) {
    const type = normalizeTemplateType(item.type)
    if (!map.has(type)) map.set(type, [])
    map.get(type).push(item)
  }
  const groups = [...map.entries()].map(([type, items]) => ({
    type,
    family: templateFamilyOf(type),
    variant: templateVariantOf(type),
    items: items.slice().sort(sortByName),
  }))
  groups.sort((a, b) => {
    const diff = typeRankOf(a.type, typeOrder) - typeRankOf(b.type, typeOrder)
    if (diff !== 0) return diff
    return a.type.localeCompare(b.type, 'zh-Hans-CN')
  })
  return groups
}

/** 一级分类的计数，工具栏 chip 用。 */
export const summarizeFamilies = (list = []) => {
  const map = new Map()
  for (const item of list) {
    const family = templateFamilyOf(item.type)
    map.set(family, (map.get(family) || 0) + 1)
  }
  return [...map.entries()]
    .map(([family, count]) => ({ family, count }))
    .sort((a, b) => b.count - a.count || a.family.localeCompare(b.family, 'zh-Hans-CN'))
}

/**
 * 按一级分类分组。
 *
 * 不按完整分类分组：Python Cell 有二十来个子类，逐类展开会把列表撑成一面墙；
 * 收成「探索者 · Python Cell」一组、子类作为每行的尾注，才浏览得下去。
 */
export const groupTemplatesByFamily = (list = [], typeOrder = []) => {
  const map = new Map()
  for (const item of list) {
    const family = templateFamilyOf(item.type)
    if (!map.has(family)) map.set(family, [])
    map.get(family).push(item)
  }
  const groups = [...map.entries()].map(([family, items]) => ({
    family,
    // 组内有没有子类，决定行尾要不要显示子类名。
    variantized: items.some((item) => Boolean(templateVariantOf(item.type))),
    items: items.slice().sort(sortByName),
    rank: Math.min(...items.map((item) => typeRankOf(item.type, typeOrder))),
  }))
  groups.sort((a, b) => a.rank - b.rank || a.family.localeCompare(b.family, 'zh-Hans-CN'))
  return groups
}

export const summarizeTemplates = (list = []) => {
  let builtin = 0
  let user = 0
  let chars = 0
  const categories = new Set()
  for (const item of list) {
    if (item.builtin) builtin += 1
    else user += 1
    chars += String(item.content || '').length
    categories.add(normalizeTemplateType(item.type))
  }
  return {
    total: list.length,
    builtin,
    user,
    categories: categories.size,
    chars,
  }
}

/** 正文里的 Markdown 控制符压成纯文本，摘要才读得懂。 */
const plainOf = (text) =>
  String(text ?? '')
    .replace(/\r\n?/g, '\n')
    .replace(/^[\s>]*(?:#{1,6}|[-*+]|\d+\.)\s+/gm, '')
    .replace(/```[\s\S]*?```/g, ' ')
    // 只删 * ` ~，保留下划线：Python 代码里 read_csv、base_url 全靠它，删了反而读不懂。
    .replace(/[*`~]/g, '')
    .replace(/\s+/g, ' ')
    .trim()

/** 卡片视图的一行摘要：去掉 Markdown 控制符，截断到几行字。 */
export const excerptOf = (content, limit = 120) => {
  const plain = plainOf(content)
  return plain.length > limit ? `${plain.slice(0, limit)}…` : plain
}

/**
 * 命中片段：返回前后文与所在行号，列表第二行直接渲染（命中词单独着色）。
 *
 * 先在折叠空白后的文本里定位，再回原始正文数行号——折叠后的下标对不上原始内容。
 */
export const contentSnippetOf = (content, query, radius = 34) => {
  const raw = String(content ?? '')
  const q = String(query ?? '').trim()
  if (!q) return null
  const plain = plainOf(raw)
  const lowerPlain = plain.toLowerCase()
  const idx = lowerPlain.indexOf(q.toLowerCase())
  if (idx < 0) return null
  const start = Math.max(0, idx - radius)
  const end = Math.min(plain.length, idx + q.length + radius)
  const rawIdx = raw.toLowerCase().indexOf(q.toLowerCase())
  const line = rawIdx < 0 ? 0 : raw.slice(0, rawIdx).split('\n').length
  return {
    prefix: (start > 0 ? '…' : '') + plain.slice(start, idx),
    match: plain.slice(idx, idx + q.length),
    suffix: plain.slice(idx + q.length, end) + (end < plain.length ? '…' : ''),
    line,
  }
}

const HIT_RANK = { name: 0, type: 1, content: 2 }

/**
 * 全文检索。
 *
 * 模板是「拿来用的文字」，只搜名称等于让人背名字；正文命中也算，并给出片段。
 * 名称命中排在类型命中前，类型命中排在正文命中前。
 */
export const searchTemplates = (list = [], query = '') => {
  const q = String(query ?? '').trim().toLowerCase()
  if (!q) return list.map((item) => ({ item, hit: null }))

  const results = []
  for (const item of list) {
    const name = String(item.name || '').toLowerCase()
    const type = String(item.type || '').toLowerCase()
    if (name.includes(q)) {
      results.push({ item, hit: { field: 'name', snippet: null } })
      continue
    }
    if (type.includes(q)) {
      results.push({ item, hit: { field: 'type', snippet: null } })
      continue
    }
    const snippet = contentSnippetOf(item.content, q)
    if (snippet) results.push({ item, hit: { field: 'content', snippet } })
  }

  return results.sort((a, b) => {
    const rank = HIT_RANK[a.hit.field] - HIT_RANK[b.hit.field]
    if (rank !== 0) return rank
    return sortByName(a.item, b.item)
  })
}

export const sortTemplates = (list = [], key = 'category', direction = 'asc') => {
  const dir = direction === 'desc' ? -1 : 1
  if (key !== 'name') return list.slice()
  return list.slice().sort((a, b) => sortByName(a, b) * dir)
}

/** 编辑器里的字符数与行数，用在文档头的元信息。 */
export const textStatsOf = (text) => {
  const value = String(text ?? '')
  const lines = value ? value.split('\n').length : 0
  return { chars: value.length, lines }
}

export const formatCount = (value) => {
  try {
    return Number(value || 0).toLocaleString('zh-CN')
  } catch {
    return String(value || 0)
  }
}

export const formatChars = (value) => {
  const n = Number(value) || 0
  if (n < 10_000) return `${formatCount(n)} 字符`
  return `${(n / 10_000).toFixed(1)} 万字符`
}
