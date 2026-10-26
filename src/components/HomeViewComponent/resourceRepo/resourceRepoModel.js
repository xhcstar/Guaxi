// 资源库管理器的纯数据层。
//
// 这里只放不依赖 Vue / Tauri 的函数：路径运算、类型判定、树构建、排序、过滤、统计。
// 抽出来的理由和 readPositionCore 一样 —— 这些规则决定了界面上「选中落到哪、删除后
// 还剩什么、搜索命中的是什么」，值得在 Node 里直接断言，不必每次都去点窗口。

/** 资源库内路径一律用 `/`，且不带首尾斜杠。根目录用空串表示。 */
export const normalizeRepoPath = (value) =>
  String(value ?? '')
    .replace(/\\/g, '/')
    .replace(/\/+/g, '/')
    .replace(/^\/+|\/+$/g, '')

/** 父目录的相对路径；根目录下的条目返回空串。 */
export const parentPath = (path) => {
  const p = normalizeRepoPath(path)
  const idx = p.lastIndexOf('/')
  return idx > 0 ? p.slice(0, idx) : ''
}

/** 拼库内相对路径，跳过空段，避免出现 `//` 或尾斜杠。 */
export const joinPath = (...parts) =>
  parts
    .map((part) => String(part ?? '').replace(/\\/g, '/').replace(/^\/+|\/+$/g, ''))
    .filter(Boolean)
    .join('/')

/** 路径最后一段。 */
export const baseName = (path) => {
  const p = normalizeRepoPath(path)
  const idx = p.lastIndexOf('/')
  return idx >= 0 ? p.slice(idx + 1) : p
}

/** 小写扩展名；没有扩展名返回空串。 */
export const extOf = (name) => {
  const value = String(name ?? '')
  const idx = value.lastIndexOf('.')
  if (idx <= 0 || idx === value.length - 1) return ''
  return value.slice(idx + 1).toLowerCase()
}

/** 面包屑：`images/2026/09/01` → 各级累积路径。 */
export const breadcrumbs = (path) => {
  const p = normalizeRepoPath(path)
  if (!p) return []
  const out = []
  let acc = ''
  for (const segment of p.split('/')) {
    acc = acc ? `${acc}/${segment}` : segment
    out.push({ name: segment, path: acc })
  }
  return out
}

/**
 * 资源类型。这些分组不是装饰：图片与图表数据是 Guaxi 里两条真实的引用链路
 * （Markdown 图床 / chart 围栏的 `resource:` 引用），分开统计和筛选用得着。
 */
export const RESOURCE_TYPE_META = Object.freeze({
  dir: { label: '目录' },
  image: { label: '图片' },
  data: { label: '图表数据' },
  text: { label: '文本' },
  document: { label: '文档' },
  media: { label: '音视频' },
  archive: { label: '压缩包' },
  font: { label: '字体' },
  other: { label: '其它' },
})

const TYPE_BY_EXT = (() => {
  const map = new Map()
  const put = (type, exts) => exts.split(' ').forEach((ext) => map.set(ext, type))
  put('image', 'png jpg jpeg jfif gif webp bmp ico avif tif tiff svg')
  put('data', 'csv tsv json jsonl ndjson xml yaml yml')
  put('text', 'md markdown mdx txt text log sql py js mjs cjs ts tsx jsx vue css scss less html htm tex bib')
  put('document', 'pdf doc docx rtf odt ods xls xlsx ppt pptx odp epub mobi azw3')
  put('media', 'mp3 wav flac ogg m4a aac opus wma mp4 mkv webm avi mov m4v wmv')
  put('archive', 'zip rar 7z tar gz tgz bz2 xz')
  put('font', 'ttf otf woff woff2')
  return map
})()

/** 交给 classifyName 的类型键，与 RESOURCE_TYPE_META 的键一致。 */
export const resourceTypeOf = (name, isDir = false) => {
  if (isDir) return 'dir'
  return TYPE_BY_EXT.get(extOf(name)) || 'other'
}

/** 能被预览面板内嵌显示的图片类型（SVG 当文本处理，避免脚本与样式污染预览）。 */
export const PREVIEWABLE_IMAGE_EXTS = new Set([
  'png', 'jpg', 'jpeg', 'jfif', 'gif', 'webp', 'bmp', 'ico', 'avif',
])

/**
 * 从扁平条目构建目录树。
 *
 * 扫描结果本身就包含目录项，但仍会补出缺失的父目录：只读扫描可能因为权限漏掉某一层，
 * 或者未来改成「按需返回」时只给了叶子项。节点带上 parentPath，便于删除后原地修剪。
 *
 * @param {Array<{relative_path:string,parent?:string,name:string,is_dir:boolean,size:number,mtime_ms:number,hidden?:boolean}>} entries
 */
export const buildResourceTree = (entries) => {
  const root = { path: '', name: '', isDir: true, size: 0, mtime: 0, children: [], childMap: new Map() }
  const dirNodes = new Map([['', root]])

  const ensureDir = (path) => {
    const p = normalizeRepoPath(path)
    if (dirNodes.has(p)) return dirNodes.get(p)
    const parent = ensureDir(parentPath(p))
    const node = {
      path: p,
      name: baseName(p),
      isDir: true,
      size: 0,
      mtime: 0,
      children: [],
      childMap: new Map(),
    }
    parent.children.push(node)
    parent.childMap.set(node.name, node)
    dirNodes.set(p, node)
    return node
  }

  const list = Array.isArray(entries) ? entries : []
  for (const raw of list) {
    const path = normalizeRepoPath(raw?.relative_path ?? raw?.path ?? '')
    if (!path) continue
    const isDir = Boolean(raw?.is_dir ?? raw?.isDir)
    const parent = ensureDir(parentPath(path))
    const name = String(raw?.name ?? baseName(path))
    const size = Number(raw?.size ?? 0) || 0
    const mtime = Number(raw?.mtime_ms ?? raw?.mtime ?? 0) || 0

    if (isDir) {
      const node = ensureDir(path)
      node.size = size
      node.mtime = mtime
      node.hidden = Boolean(raw?.hidden)
      if (!parent.childMap.has(name) || parent.childMap.get(name) !== node) {
        parent.childMap.set(name, node)
      }
      continue
    }

    const node = {
      path,
      name,
      isDir: false,
      size,
      mtime,
      hidden: Boolean(raw?.hidden),
      parentPath: parent.path,
    }
    parent.children.push(node)
    parent.childMap.set(name, node)
  }

  return root
}

/** 深度优先遍历，visitor 返回 false 可剪枝该子树。 */
export const walkTree = (node, visitor, depth = 0) => {
  if (!node) return
  if (visitor(node, depth) === false) return
  for (const child of node.children || []) walkTree(child, visitor, depth + 1)
}

/** 从树里按库内相对路径找节点。 */
export const findNode = (root, path) => {
  const target = normalizeRepoPath(path)
  if (!target) return root || null
  let node = root
  for (const segment of target.split('/')) {
    const next = node?.childMap?.get(segment) || node?.children?.find((c) => c.name === segment)
    if (!next) return null
    node = next
  }
  return node
}

/** 节点自身加全部后代（含目录）的路径。删除后据此一次性清掉内存中的整棵子树。 */
export const collectSubtreePaths = (node, out = []) => {
  if (!node) return out
  if (node.path) out.push(node.path)
  for (const child of node.children || []) collectSubtreePaths(child, out)
  return out
}

/** 目录的直接子项数量与总大小。 */
export const summarizeDir = (node) => {
  let files = 0
  let dirs = 0
  let size = 0
  for (const child of node?.children || []) {
    if (child.isDir) dirs += 1
    else {
      files += 1
      size += Number(child.size) || 0
    }
  }
  return { files, dirs, size, total: files + dirs }
}

const nameCollator = (() => {
  try {
    return new Intl.Collator('zh-Hans-CN', { numeric: true, sensitivity: 'base' })
  } catch {
    return null
  }
})()

export const compareNames = (a, b) => {
  const left = String(a ?? '')
  const right = String(b ?? '')
  if (nameCollator) return nameCollator.compare(left, right)
  return left.localeCompare(right)
}

/** 排序：目录永远在前；同类型内按 key 排，升降序只影响同类型。 */
export const createComparator = (key = 'name', direction = 'asc') => {
  const dir = direction === 'desc' ? -1 : 1
  return (a, b) => {
    if (a.isDir !== b.isDir) return a.isDir ? -1 : 1
    let delta = 0
    if (key === 'size') delta = (Number(a.size) || 0) - (Number(b.size) || 0)
    else if (key === 'mtime') delta = (Number(a.mtime) || 0) - (Number(b.mtime) || 0)
    else delta = compareNames(a.name, b.name)
    if (delta === 0) delta = compareNames(a.name, b.name)
    return delta * dir
  }
}

export const sortTree = (node, comparator) => {
  node.children.sort(comparator)
  for (const child of node.children) {
    if (child.isDir) sortTree(child, comparator)
  }
  return node
}

/** 条目是否命中搜索：文件名或完整相对路径包含关键字（大小写不敏感）。 */
export const matchKeyword = (node, keyword) => {
  const k = String(keyword || '').trim().toLowerCase()
  if (!k) return true
  return `${node.name} ${node.path}`.toLowerCase().includes(k)
}

/**
 * 在整棵子树里收集命中项。
 * 搜索按「文件」返回，目录只作为路径信息出现在结果行里。
 */
export const collectMatches = (root, { keyword = '', type = 'all' } = {}) => {
  const out = []
  walkTree(root, (node) => {
    if (!node.path) return
    if (node.isDir) {
      if (type === 'dir' && matchKeyword(node, keyword)) out.push(node)
      return
    }
    if (type !== 'all' && resourceTypeOf(node.name, false) !== type) return
    if (matchKeyword(node, keyword)) out.push(node)
  })
  return out
}

/** 统计：条目数、目录数、总大小、各类型数量。 */
export const summarizeRepo = (root) => {
  const byType = {}
  let files = 0
  let dirs = 0
  let size = 0
  walkTree(root, (node) => {
    if (!node.path) return
    if (node.isDir) {
      dirs += 1
      byType.dir = (byType.dir || 0) + 1
      return
    }
    files += 1
    size += Number(node.size) || 0
    const type = resourceTypeOf(node.name, false)
    byType[type] = (byType[type] || 0) + 1
  })
  return { files, dirs, size, total: files + dirs, byType }
}

/** 与目录内容冲突时生成 `name (2).ext` 这样的候选名。 */
export const makeUniqueName = (existingNames, desired) => {
  const wanted = String(desired || '').trim()
  if (!wanted) return wanted
  const names = existingNames instanceof Set
    ? new Set([...existingNames].map((n) => String(n).toLowerCase()))
    : new Set((existingNames || []).map((n) => String(n).toLowerCase()))
  if (!names.has(wanted.toLowerCase())) return wanted

  const dot = wanted.lastIndexOf('.')
  const base = dot > 0 ? wanted.slice(0, dot) : wanted
  const ext = dot > 0 ? wanted.slice(dot) : ''
  for (let i = 2; i < 5000; i += 1) {
    const candidate = `${base} (${i})${ext}`
    if (!names.has(candidate.toLowerCase())) return candidate
  }
  return `${base} (${Date.now()})${ext}`
}

/** 文件名合法性：不能带路径分隔符，不能是 . / ..，不能为空。 */
export const validateEntryName = (name) => {
  const value = String(name || '').trim()
  if (!value) return { ok: false, reason: '名称不能为空' }
  if (/[\\/]/.test(value)) return { ok: false, reason: '名称不能包含路径分隔符' }
  if (value === '.' || value === '..') return { ok: false, reason: '名称不能是 . 或 ..' }
  if (/[<>:"|?*\u0000-\u001f]/.test(value)) return { ok: false, reason: '名称不能包含 < > : " | ? * 等字符' }
  return { ok: true, value }
}

export const humanBytes = (bytes) => {
  const n = Number(bytes) || 0
  if (n <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = n
  let idx = 0
  while (size >= 1024 && idx < units.length - 1) {
    size /= 1024
    idx += 1
  }
  const digits = idx === 0 ? 0 : size >= 100 ? 0 : 1
  return `${size.toFixed(digits)} ${units[idx]}`
}

export const humanCount = (value) => {
  const n = Number(value) || 0
  try {
    return n.toLocaleString('zh-CN')
  } catch {
    return String(n)
  }
}

/** 相对时间用中文短句，够用就好；精确时间放在 title 里。 */
export const humanAge = (mtimeMs, now = Date.now()) => {
  const t = Number(mtimeMs) || 0
  if (!t) return '—'
  const diff = Math.max(0, now - t)
  const minute = 60_000
  const hour = 60 * minute
  const day = 24 * hour
  if (diff < minute) return '刚刚'
  if (diff < hour) return `${Math.floor(diff / minute)} 分钟前`
  if (diff < day) return `${Math.floor(diff / hour)} 小时前`
  if (diff < 30 * day) return `${Math.floor(diff / day)} 天前`
  const d = new Date(t)
  const sameYear = d.getFullYear() === new Date(now).getFullYear()
  const pad = (v) => String(v).padStart(2, '0')
  return sameYear
    ? `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
    : `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

export const formatDateTime = (mtimeMs) => {
  const t = Number(mtimeMs) || 0
  if (!t) return '—'
  try {
    return new Date(t).toLocaleString('zh-CN', { hour12: false })
  } catch {
    return '—'
  }
}

/**
 * 与编辑器插入图片时使用的地址保持一致：
 * `http://localhost:8888/markdown/api/bed/<urlencoded relative path>`。
 * 复制出来可以直接粘进任意 Markdown 文档。
 */
export const bedUrlOf = (relativePath, origin = 'http://localhost:8888') =>
  `${origin}/markdown/api/bed/${encodeURIComponent(normalizeRepoPath(relativePath))}`

export const markdownImageOf = (node) => {
  const alt = baseName(node?.name || '').replace(/\.[^.]+$/, '') || 'image'
  return `![${alt}](${bedUrlOf(node?.path)})`
}

/** 图表围栏引用：与 guaxiEditingCommands 里 `resource:charts/...` 的写法一致。 */
export const chartRefOf = (node) => `resource:${normalizeRepoPath(node?.path)}`

/** 简易 CSV / TSV 解析：支持引号包裹、引号内换行与 `""` 转义。 */
export const parseDelimited = (text, delimiter = ',', maxRows = 200) => {
  const rows = []
  const source = String(text ?? '')
  let row = []
  let field = ''
  let quoted = false
  let i = 0
  while (i < source.length && rows.length < maxRows) {
    const ch = source[i]
    if (quoted) {
      if (ch === '"') {
        if (source[i + 1] === '"') {
          field += '"'
          i += 2
          continue
        }
        quoted = false
        i += 1
        continue
      }
      field += ch
      i += 1
      continue
    }
    if (ch === '"') {
      quoted = true
      i += 1
      continue
    }
    if (ch === delimiter) {
      row.push(field)
      field = ''
      i += 1
      continue
    }
    if (ch === '\n' || ch === '\r') {
      row.push(field)
      field = ''
      rows.push(row)
      row = []
      if (ch === '\r' && source[i + 1] === '\n') i += 1
      i += 1
      continue
    }
    field += ch
    i += 1
  }
  if (field.length > 0 || row.length > 0) {
    row.push(field)
    rows.push(row)
  }
  // 去掉末尾因换行产生的空行
  while (rows.length && rows[rows.length - 1].every((cell) => cell === '')) rows.pop()
  return rows
}

/** 从一段文本截断预览，避免把几十 MB 的日志整段塞进 DOM。 */
export const clipText = (text, limit = 60_000) => {
  const value = String(text ?? '')
  if (value.length <= limit) return { text: value, clipped: false }
  return { text: value.slice(0, limit), clipped: true }
}
