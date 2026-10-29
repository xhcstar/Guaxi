/**
 * 文件夹树的「服务商模板壳」识别。
 *
 * QQ / 163 会下发一棵以「其他文件夹」为根的子树，里面装着「已删除」「已发送」这类
 * 系统目录。它们不是用户整理出来的结构，但又不是标准角色，混进「我的文件夹」里
 * 既碍眼、还会被误当成用户的目录。
 *
 * 判定依据是**结构**而不是名字：一个自定义目录，如果它的子树里挂着两个以上
 * 标准角色的文件夹（收件箱 / 已发送 / 已删除…），它就是个模板壳，连同它下面的
 * 自定义子目录一起隐藏。用户自己建「工作/2026」这种，子树里不会有系统目录，
 * 照常显示；他若确实想留着壳里的某个目录，右键「保留在侧栏」可以强制显示。
 */
const DEFAULT_DELIMITER = '/'

function delimiterOf(folder) {
  return folder?.delimiter || DEFAULT_DELIMITER
}

/** 收集「模板壳」的完整路径。 */
export function providerShellPrefixes(folders) {
  const list = Array.isArray(folders) ? folders : []
  const counts = new Map()
  for (const folder of list) {
    if (folder?.role === 'custom' || folder?.role === 'outbox') continue
    const delim = delimiterOf(folder)
    const parts = String(folder?.name || '').split(delim)
    // 只统计「被包裹在别的目录里」的系统目录：顶层的收件箱 / 已发送不算。
    for (let i = 1; i < parts.length; i += 1) {
      const prefix = parts.slice(0, i).join(delim)
      counts.set(prefix, (counts.get(prefix) || 0) + 1)
    }
  }
  const shells = new Set()
  for (const [prefix, count] of counts) {
    // 两个以上系统目录才认定为模板壳，避免误伤「用户目录里放了一个系统夹」这种个例。
    if (count >= 2) shells.add(prefix)
  }
  return shells
}

/** 这个文件夹是不是模板壳本身，或它里面的子目录。 */
export function isTemplateFolder(folder, shells) {
  if (!folder || !shells?.size) return false
  const name = String(folder.name || '')
  if (!name) return false
  for (const prefix of shells) {
    if (name === prefix) return true
    if (name.startsWith(`${prefix}${delimiterOf(folder)}`)) return true
  }
  return false
}

/** 界面上真正该列出来的文件夹（标准角色由侧栏的固定区块负责，这里只管自定义）。 */
export function visibleCustomFolders(folders) {
  const list = Array.isArray(folders) ? folders : []
  const shells = providerShellPrefixes(list)
  return list.filter(
    (folder) =>
      folder?.role === 'custom' &&
      // 用户显式「保留在侧栏」的，即便在模板壳里也照常显示。
      (folder.userCreated || !isTemplateFolder(folder, shells)),
  )
}

/** 可作为「移动 / 归档」目标的文件夹：模板壳里的目录不放进来。 */
export function selectableMoveTargets(folders) {
  const list = Array.isArray(folders) ? folders : []
  const shells = providerShellPrefixes(list)
  return list.filter(
    (folder) =>
      folder?.selectable !== false &&
      folder?.role !== 'outbox' &&
      (folder.userCreated || !isTemplateFolder(folder, shells)),
  )
}
