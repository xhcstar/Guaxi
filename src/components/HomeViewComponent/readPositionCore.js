// 编辑器阅读位置保持的核心逻辑（纯函数 + 一个与 DOM 无关的缓存）。
//
// 为什么单独成模块
//
// 这段逻辑原先整块长在 GuaxiEditor.vue 里（1.1 万行），而且每一步都直接摸 DOM：
// 读 scrollTop、判 display:none、等渲染完成。结果是它只能靠人工点界面验证——
// 而「切回文件后回到开头」恰好是那种肉眼能看见、但没法在 CI 里复现的 bug。
//
// 这里只抽出**与 DOM 无关的决策部分**：文件路径怎么归一、哪些文件用哪种载体、
// 缓存怎么存取、恢复请求该排给谁。真正写 scrollTop 的部分仍留在组件里，
// 通过注入的 `read` / `write` 回调访问。于是这套决策可以在 Node 里完整测。
//
// 语义约定（与组件内保持一致，改动时两处一起改）
// - 阅读位置的载体按「模式」而非「文件后缀」选择：
//     edit      → Monaco viewState（光标/选区/滚动）
//     preview   → 预览容器 scrollTop，按文件后缀再分 markdown / pdf / org
//     split     → 两侧都要存/恢复
//     explorer  → notebook surface
//     realtime  → Vditor 内容区
// - 一个文件可能同时有多份记录（分屏下编辑器与预览各一份），所以缓存 key 是
//   `kind:path`，kind 里区分载体。

/** 缓存条目上限。与组件里的 PREVIEW_READ_POSITION_CACHE_LIMIT 保持一致。 */
export const READ_POSITION_CACHE_LIMIT = 160

/** 统一路径写法：反斜杠转正斜杠并去空白。缓存 key 必须用它，否则 Windows 路径会分裂成两份。 */
export const normalizeReadPath = (path) => String(path || '').replace(/\\/g, '/').trim()

/**
 * 文件后缀 → 预览载体类型。
 *
 * 注意这里返回的是「预览面板用哪种渲染器」，不是「文件能不能预览」：
 * 空字符串表示没有独立预览面（普通文本、代码等），它们的预览走 markdown 渲染器。
 */
export const previewReadKindForPath = (path) => {
  const p = normalizeReadPath(path).toLowerCase()
  if (/\.(tex|typ)$/.test(p)) return 'pdf'
  if (/\.(md|markdown)$/.test(p)) return 'markdown'
  if (/\.org$/.test(p)) return 'org'
  return ''
}

/**
 * 缓存的 key。
 *
 * 加 kind 前缀是必须的：同一份 .md 在「编辑模式」与「预览模式」下存的是两种位置
 * （Monaco 行号 vs 容器像素），共用一个 key 会互相覆盖。
 */
export const readPositionKey = (path, kind = previewReadKindForPath(path)) => {
  const normalized = normalizeReadPath(path)
  return normalized && kind ? `${kind}:${normalized}` : ''
}

/** 模式键归一。无法识别的值一律落到 edit——它是唯一一定可用的载体。 */
export const normalizeModeKey = (key) => {
  const k = String(key || '').trim().toLowerCase()
  return ['edit', 'preview', 'split', 'realtime', 'explorer'].includes(k) ? k : 'edit'
}

/**
 * 某个模式需要哪几个缓存 key 才有「完整」的阅读位置。
 *
 * 分屏要两个（编辑器 + 预览），其余一个。返回值用于恢复时判断"这次恢复是否完整"，
 * 也用于测试断言"切走时该存的都存了"。
 */
export const expectedKeysForMode = (modeKey, path) => {
  const mode = normalizeModeKey(modeKey)
  const p = normalizeReadPath(path)
  if (!p) return []
  const kind = previewReadKindForPath(p)
  if (mode === 'edit') return [readPositionKey(p, 'editor')]
  if (mode === 'explorer') return [readPositionKey(p, 'explorer')]
  if (mode === 'realtime') return [readPositionKey(p, 'realtime')]
  if (mode === 'split') {
    // 编辑器侧 + 预览侧。普通文本没有独立预览面，它的预览仍走 markdown 渲染器，
    // 所以这里用 `kind || 'markdown'`，与 planReadPositionRestore 保持一致。
    return [readPositionKey(p, 'editor'), readPositionKey(p, kind || 'markdown')]
  }
  // preview
  return kind
    ? [readPositionKey(p, kind)]
    // 普通文本/代码在预览模式里仍由 markdown 渲染器出图，位置记在 markdown 载体上。
    : [readPositionKey(p, 'markdown')]
}

/**
 * 有界的 LRU 缓存。
 *
 * 用 Map 的插入序做 LRU：命中时先删再插，把最近用的挪到末尾；
 * 超限从头部淘汰。和组件里 `rememberPreviewReadPosition` 的写法一致。
 */
export const createReadPositionCache = (limit = READ_POSITION_CACHE_LIMIT) => {
  const store = new Map()
  return {
    /** 写入一条位置。entry 至少要有 path 与 kind。 */
    remember(entry, now = Date.now()) {
      const key = readPositionKey(entry?.path, entry?.kind)
      if (!key) return ''
      store.delete(key)
      store.set(key, { ...entry, key, savedAt: now })
      while (store.size > limit) {
        const oldest = store.keys().next().value
        if (!oldest) break
        store.delete(oldest)
      }
      return key
    },
    get(key) {
      return store.get(key) || null
    },
    has(key) {
      return store.has(key)
    },
    delete(key) {
      return store.delete(key)
    },
    clear() {
      store.clear()
    },
    get size() {
      return store.size
    },
    keys() {
      return [...store.keys()]
    },
  }
}

/**
 * 决定「切换文件时，应从哪个载体恢复位置」。
 *
 * 这是问题 2 的核心：A 在编辑模式、B 记忆为预览模式时，如果按**当前**模式
 * （edit）去恢复，会写到一个马上要被隐藏的编辑器上；等模式切到预览，容器重新显示
 * 且 scrollTop 为 0，用户看到的就是回到开头。
 *
 * 所以恢复载体必须按**目标模式**（该文件自己的记忆）算，而不是按当前模式。
 *
 * @param {object} opts
 * @param {string} opts.modeKey        目标模式（该文件记忆的模式）
 * @param {string} opts.path           文件路径
 * @param {boolean} opts.modeChanging  本次切文件是否会触发模式切换
 * @returns {{ carriers: string[], deferUntilAfterModeSwitch: boolean }}
 */
export const planReadPositionRestore = ({ modeKey, path, modeChanging }) => {
  const mode = normalizeModeKey(modeKey)
  const kind = previewReadKindForPath(path)

  const carriers = []
  if (mode === 'edit') carriers.push('editor')
  else if (mode === 'explorer') carriers.push('explorer')
  else if (mode === 'realtime') carriers.push('realtime')
  else {
    // 分屏：编辑器在前。
    // 预览面板恢复时可能要等 PDF 编译/渲染完成，而编辑器是同步落位的；
    // 先恢复编辑器能保证即使预览那边还在等，用户已经看到正确的一半。
    if (mode === 'split') carriers.push('editor')
    carriers.push(kind || 'markdown')
  }

  // 模式要变时，载体在切换过程中会被 v-show 隐藏或重新挂载：
  // display:none 的元素 scrollTop 恒为 0，此刻写进去等于没恢复。
  return { carriers, deferUntilAfterModeSwitch: Boolean(modeChanging) }
}

/**
 * 位置是否「真的」需要恢复。
 *
 * 顶部（scrollTop 0）是合法位置——用户可能主动回到开头，此时不应该被旧位置拽走。
 * 但缓存为空、或记录属于别的文件时必须拒绝，否则会把 A 的位置写给 B。
 */
export const shouldRestoreEntry = (entry, path) => {
  if (!entry) return false
  const expected = normalizeReadPath(path)
  if (!expected) return false
  return normalizeReadPath(entry.path) === expected
}

/**
 * 把缓存里的条目换算成实际要写入的 scrollTop。
 *
 * 两个来源：绝对像素与比例。优先用像素（同尺寸下最准确）；
 * 当文档高度变化（图片加载、窗口缩放）导致像素超出可滚动范围时，退回比例，
 * 避免「恢复到一个不存在的位置」。
 */
export const resolveScrollTarget = (entry, { scrollHeight, clientHeight }) => {
  const maxTop = Math.max(0, Number(scrollHeight || 0) - Number(clientHeight || 0))
  const rawTop = Number(entry?.scrollTop)
  const ratio = Number(entry?.scrollRatio ?? entry?.ratio)

  if (Number.isFinite(rawTop) && rawTop >= 0 && rawTop <= maxTop) {
    return { top: Math.round(rawTop), source: 'pixels' }
  }
  if (Number.isFinite(ratio) && ratio > 0 && maxTop > 0) {
    return { top: Math.round(Math.min(maxTop, ratio * maxTop)), source: 'ratio' }
  }
  return { top: 0, source: 'none' }
}

// ───────────────────────────── 内容锚点（解决「像素/比例只能恢复个大概」）

/**
 * 由「视口顶部所在的内容块行号 + 块内像素偏移」组成的锚点。
 *
 * 为什么需要它：像素只在文档高度完全不变时精确；比例在图片、公式、Mermaid
 * 图表加载前后会把同一比例换算到不同位置。而 Markdown 渲染结果里每个块都带
 * `data-md-start-line`，行号在任何窗口宽度/字体/图片加载状态下都会指回同一段文字，
 * 所以「行号 + 块内偏移」是唯一真正稳定的坐标。
 *
 * @param {object} opts
 * @param {number} opts.anchorLine    视口顶部所在块的起始行（<=0 表示量不到）
 * @param {number} [opts.anchorOffset] 视口顶部相对该块顶部的像素偏移
 */
export const makeContentAnchor = ({ anchorLine, anchorOffset = 0 } = {}) => {
  const line = Number(anchorLine)
  if (!Number.isFinite(line) || line <= 0) return null
  const offset = Number(anchorOffset)
  return {
    anchorLine: Math.round(line),
    anchorOffset: Number.isFinite(offset) ? Math.round(offset) : 0,
  }
}

/**
 * 恢复时用锚点换算目标 scrollTop。
 *
 * resolveBlockTop(anchorLine) 由调用方提供：它负责把行号定位到当前 DOM 里的像素位置
 * （块尚未渲染出来时返回 null）。量不到就返回 null，让调用方退回像素/比例，
 * 而不是把 0 当成目标（那正好就是「回到开头」）。
 *
 * @param {object} entry     位置条目（含 anchorLine / anchorOffset）
 * @param {(line:number)=>number|null} resolveBlockTop
 * @returns {{top:number, source:'anchor'}|null}
 */
export const resolveAnchorTarget = (entry, resolveBlockTop) => {
  const line = Number(entry?.anchorLine)
  if (!Number.isFinite(line) || line <= 0) return null
  if (typeof resolveBlockTop !== 'function') return null
  const blockTop = resolveBlockTop(line)
  if (!Number.isFinite(blockTop)) return null
  const offset = Number(entry?.anchorOffset)
  const top = blockTop + (Number.isFinite(offset) ? offset : 0)
  return { top: Math.max(0, Math.round(top)), source: 'anchor' }
}

/**
 * 决定一次「渐进渲染中的逐帧落位」何时算收敛。
 *
 * 预览/PDF/探索者都是先出骨架、再被图片和图表撑高。每帧算出的目标像素会随文档变高
 * 而变化，所以「写进去的 scrollTop 和目标相等」并不代表稳定；真正稳定只有一个判据：
 * 可滚动高度连续 stableFrames 帧不再变化。
 *
 * 抽成纯函数是为了让「到底哪一帧收工」这件事能被测试锁住——它错了的表现就是
 * 「恢复了一部分」或「和用户抢滚动条」，这两种只在真实渲染节奏下才看得见。
 */
export const isRestoreSettled = ({
  reachable,
  targetTop,
  heightStableFrames,
  stableFrames = 2,
  awaitingTarget = false,
}) => {
  if (!Number.isFinite(targetTop)) return true
  // 「记录里有位置，但这一帧还算不出来」（块未渲染 / 文档还没撑高）不能当成已收敛：
  // 那会在第一帧就把滚动条停在 0，正是「切回来回到开头」的成因之一。
  if (awaitingTarget) return false
  if (targetTop <= 0) return true
  // 目标可达与否都看「文档高度是否已经稳定」：可达但还在长高时提前收工，
  // 图片/图表撑高后就会再次错位；不可达且高度已定，说明目标确实超出文档，只能保留 clamp 后的结果。
  void reachable
  return heightStableFrames >= stableFrames
}

// ───────────────────────────── 持久化（解决「重启软件记不住」）

/** 落盘版本号。结构不兼容时整体丢弃，绝不尝试迁移脏数据。 */
export const READ_POSITION_STORE_VERSION = 1

/**
 * 生成落盘用的紧凑快照。
 *
 * 只保留恢复真正需要的字段：路径写法、像素、比例、内容锚点、文档时间戳。
 * 视口尺寸（clientHeight / scrollHeight）是用来判断「当时的像素在现在还有没有意义」的，
 * 所以要一起存——换了窗口大小后像素不再可信，必须走比例或锚点。
 *
 * 每类载体各自有上限，超出按「最久未访问」淘汰（Map 插入序已在缓存层保证）。
 */
export const buildPersistPayload = ({
  positions = new Map(),
  modes = new Map(),
  extra = {},
  limit = READ_POSITION_CACHE_LIMIT,
} = {}) => {
  const entries = []
  for (const [key, entry] of positions) {
    if (!entry) continue
    entries.push({
      key,
      path: normalizeReadPath(entry.path),
      scrollTop: Number.isFinite(Number(entry.scrollTop)) ? Math.round(Number(entry.scrollTop)) : null,
      scrollRatio: Number.isFinite(Number(entry.scrollRatio)) ? +Number(entry.scrollRatio).toFixed(4) : null,
      anchorLine: Number(entry.anchorLine) > 0 ? Math.round(Number(entry.anchorLine)) : null,
      anchorOffset: Number(entry.anchorOffset) > 0 ? Math.round(Number(entry.anchorOffset)) : null,
      page: Number(entry.page) > 0 ? Math.round(Number(entry.page)) : null,
      pageYPt: Number(entry.pageYPt) > 0 ? +Number(entry.pageYPt).toFixed(2) : null,
      lineNumber: Number(entry.lineNumber) > 0 ? Math.round(Number(entry.lineNumber)) : null,
      cellKey: entry.cellKey ? String(entry.cellKey) : null,
      scrollHeight: Number(entry.scrollHeight) > 0 ? Math.round(Number(entry.scrollHeight)) : null,
      clientHeight: Number(entry.clientHeight) > 0 ? Math.round(Number(entry.clientHeight)) : null,
      savedAt: Number(entry.savedAt) || 0,
    })
    if (entries.length >= limit) break
  }
  const modeEntries = {}
  for (const [path, modeKey] of modes) {
    const p = normalizeReadPath(path)
    if (p) modeEntries[p] = normalizeModeKey(modeKey)
  }
  return {
    version: READ_POSITION_STORE_VERSION,
    savedAt: Date.now(),
    positions: entries,
    modes: modeEntries,
    ...extra,
  }
}

/**
 * 解析落盘数据，任何不符预期的输入都返回空结果（当作没存过），绝不抛错。
 *
 * 位置条目要重建缓存 key：key 由 path + kind 推导，落盘里的 key 只能作为校验，
 * 以免旧版本写出的 key 与现在的规则不一致时把位置写到错误的载体上。
 */
export const parsePersistPayload = (raw) => {
  const empty = { positions: [], modes: {} }
  if (!raw || typeof raw !== 'object') return empty
  if (Number(raw.version) !== READ_POSITION_STORE_VERSION) return empty
  const positions = []
  if (Array.isArray(raw.positions)) {
    for (const item of raw.positions) {
      if (!item || typeof item !== 'object') continue
      const path = normalizeReadPath(item.path)
      if (!path) continue
      const kind = String(item.key || '').includes(':')
        ? String(item.key).slice(0, String(item.key).indexOf(':')).trim()
        : ''
      if (!kind) continue
      const key = readPositionKey(path, kind)
      if (!key) continue
      positions.push({ ...item, kind, path, key })
    }
  }
  const modes = {}
  if (raw.modes && typeof raw.modes === 'object' && !Array.isArray(raw.modes)) {
    for (const [path, modeKey] of Object.entries(raw.modes)) {
      const p = normalizeReadPath(path)
      if (p) modes[p] = normalizeModeKey(modeKey)
    }
  }
  return { positions, modes }
}

/** 每个载体「当前窗口尺寸下，存下的像素是否还可信」。不可信时改用比例/锚点。 */
export const isPixelTrustworthy = (entry, { scrollHeight, clientHeight }) => {
  const savedScrollHeight = Number(entry?.scrollHeight)
  const savedClientHeight = Number(entry?.clientHeight)
  // 没存尺寸（旧记录）时只按当前可滚动范围判断，交给 resolveScrollTarget 兜底。
  if (!(savedScrollHeight > 0) || !(savedClientHeight > 0)) return true
  const heightDrift = Math.abs(savedScrollHeight - Number(scrollHeight || 0))
  const viewportDrift = Math.abs(savedClientHeight - Number(clientHeight || 0))
  // 文档换了内容/窗口缩放会让这两个值明显变化；留一点余量给滚动条与取整误差。
  return heightDrift <= Math.max(24, savedScrollHeight * 0.01)
    && viewportDrift <= Math.max(8, savedClientHeight * 0.02)
}
