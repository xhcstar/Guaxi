// 阅读器窗口的「外观」加载：只做明暗主题 + 字号。
//
// 为什么不复用 settingsStore.loadAppearanceConfig：
// 那个 action 会顺带加载代码主题（10 张 highlight.js 样式表 + Monaco 主题重注册）。
// 独立阅读器窗口里没有代码块、没有 Monaco，为了一个明暗标记把整套主题机制拉进来
// 会把首帧拖慢一大截。这里只发一个 invoke，然后给 <html> 打主题标记，
// 变量由 styles/guaxiBase.css 提供。
import { invoke } from '@tauri-apps/api/core'
import { createLogger } from '../../utils/logger'

const logger = createLogger('ReaderAppearance')

/** 读取应用外观配置；失败时回退亮色。 */
export async function loadReaderAppearance() {
  try {
    const appearance = await invoke('get_appearance_config')
    const fontSize = Number(appearance?.font_size ?? appearance?.fontSize)
    return {
      theme: appearance?.theme === 'dark' ? 'dark' : 'light',
      fontSize: Number.isFinite(fontSize) && fontSize > 0 ? fontSize : 14,
    }
  } catch (error) {
    logger.warn('[ReaderAppearance] load failed, fall back to light', {
      error: String(error?.message || error),
    })
    return { theme: 'light', fontSize: 14 }
  }
}

/** 把主题落到文档根节点（CSS 变量随之切换）。 */
export function applyReaderAppearance(theme) {
  if (!document?.documentElement) return
  const normalized = theme === 'dark' ? 'dark' : 'light'
  document.documentElement.dataset.guaxiTheme = normalized
  document.documentElement.classList.toggle('dark-mode', normalized === 'dark')
}
