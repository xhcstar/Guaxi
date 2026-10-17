// 文件路径变更（重命名 / 移动 / 删除）前的统一落地钩子。
//
// 问题：这些操作由文件树发起，但“防抖中的编辑”掌握在编辑器/探索者组件手里。
// 如果直接调后端移动/删除，编辑器可能在操作完成后才把旧内容写回旧路径，
// 轻则丢编辑，重则把已删除/已改名的文件重新创建出来。
//
// 约定：文件树在执行后端命令前 `await runFilePathMutationGuards(...)`，
// 编辑器在挂载时注册守卫，守卫里负责 flush 防抖、把编辑器内容回写到 tab、取消延迟写盘。
const guards = new Set()

/** 注册一个路径变更守卫，返回反注册函数。 */
export const registerFilePathMutationGuard = (guard) => {
  if (typeof guard !== 'function') return () => {}
  guards.add(guard)
  return () => guards.delete(guard)
}

/**
 * 依次执行所有守卫。
 * 单个守卫失败只记录日志，不能阻止真正的文件操作（否则用户会卡在无法重命名）。
 */
export const runFilePathMutationGuards = async (mutation = {}) => {
  for (const guard of Array.from(guards)) {
    try {
      await guard(mutation)
    } catch (error) {
      console.warn?.('[FilePathMutation] guard failed', {
        type: mutation?.type,
        oldPath: mutation?.oldPath,
        error,
      })
    }
  }
}
