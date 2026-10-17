// 文件内容归属判定。
//
// 背景：编辑器内容写回 tab / 磁盘时，最容易出错的是“这份内容属于哪个文件”。
// 切换标签、重命名、删除的瞬间，store 里的 activeTab 可能已经指向新文件，
// 而编辑器里的 model / 防抖队列里的内容仍属于旧文件。若这时按 activeTab 取路径，
// 旧内容就会被写进新文件（新建空文件“自带内容”的根因）。
//
// 这里把判定逻辑抽成纯函数，便于单测覆盖；调用方只需提供
// “编辑器模型绑定的路径”和“store 当前激活路径”。

/** 路径归一化：统一分隔符、去掉末尾斜杠，仅用于比较。 */
export const normalizePathKey = (path) => String(path || '').trim().replace(/\\/g, '/').replace(/\/+$/g, '')

/** 两个路径是否指向同一个文件。 */
export const isSameFilePath = (left, right) => {
  const a = normalizePathKey(left)
  const b = normalizePathKey(right)
  return !!a && a === b
}

/** path 是否等于 root 或位于 root 目录之下（用于重命名/删除目录时匹配子文件）。 */
export const isPathWithin = (path, root) => {
  const p = normalizePathKey(path)
  const r = normalizePathKey(root)
  if (!p || !r) return false
  return p === r || p.startsWith(`${r}/`)
}

/**
 * 解析编辑器当前 model 真正绑定的文件路径。
 *
 * `modelUriToPath` 是 GuaxiEditor 在 ensureEditorModelBinding 时登记的 URI → 路径映射。
 * 这份映射不会随着 store 的 activeTab 切换而改变，正好用来判断“编辑器里现在是谁的内容”。
 */
export const resolveModelBoundPath = ({ modelUriToPath, modelUri, fallbackPath = '' } = {}) => {
  const uri = String(modelUri || '').trim()
  let bound = ''
  if (uri && modelUriToPath && typeof modelUriToPath.get === 'function') {
    bound = modelUriToPath.get(uri) || ''
  }
  return String(bound || fallbackPath || '').trim()
}

/**
 * 内容是否可以写回“当前激活视图”（rawContent / 预览 / 源码编辑器）。
 * 内容属于旧文件时不能写，否则切换过程中会把旧内容顶到新文件上。
 */
export const shouldApplyContentToActiveView = (contentPath, activePath) => {
  if (!contentPath) return true
  return isSameFilePath(contentPath, activePath)
}

/**
 * 判断一条延迟写盘任务是否还应该执行。
 *
 * - 目标路径不再属于任何打开标签（文件被删除、或被重命名走了）：跳过，
 *   否则会在旧路径重新创建文件 / .cellmeta。
 * - 路径属于刚刚被删除的目录（delete/rename 目录）：同样跳过。
 */
export const shouldPersistContentForPath = ({ targetPath, openPaths = [], removedRoot = '' } = {}) => {
  const target = normalizePathKey(targetPath)
  if (!target) return false
  if (removedRoot && isPathWithin(target, removedRoot)) return false
  const open = Array.isArray(openPaths) ? openPaths : []
  return open.some((path) => isSameFilePath(path, target))
}
