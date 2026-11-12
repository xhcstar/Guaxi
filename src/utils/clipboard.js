// 写系统剪贴板（带降级）。
//
// 为什么单独成模块：构建失败日志、复制路径等好几处都要用，原先每个组件各抄一份，
// 行为（先 navigator 再 Tauri 插件）容易漂。这里统一，并且永远不抛错——
// 复制失败只应提示「复制失败」，不该打断用户正在做的事。
import { createLogger } from './logger'

const logger = createLogger('Clipboard')

export const writeClipboardText = async (text) => {
  const value = String(text ?? '')
  try {
    await navigator.clipboard.writeText(value)
    return true
  } catch (error) {
    logger.debug('[Clipboard] navigator write failed, trying Tauri clipboard manager', {
      error: String(error?.message || error || ''),
    })
  }

  try {
    const mod = await import('@tauri-apps/plugin-clipboard-manager')
    await mod.writeText(value)
    return true
  } catch (error) {
    logger.warn('[Clipboard] write failed', {
      error: String(error?.message || error || ''),
    })
    return false
  }
}
