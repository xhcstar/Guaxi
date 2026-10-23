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
