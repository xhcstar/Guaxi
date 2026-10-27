import { onActivated, onDeactivated, onMounted, onUnmounted } from 'vue'

/*
 * 主界面顶栏图标的快捷键统一用「Alt + 字母」这种两键组合。
 *
 * 为什么从 Ctrl+Alt+字母 换成 Alt+字母：
 * 1. 旧方案是三个键，而且和编辑器自己的一族快捷键正撞——Monaco 的表格工具用的是
 *    Ctrl+Alt+T（插入表格）、Ctrl+Alt+H/J/K/L（在单元格间移动）。顶栏的热键挂在
 *    window 捕获阶段，而它又特意放行 .monaco-editor（见 isPlainEditableTarget），
 *    于是同一组按键被顶栏先吃掉，编辑器的表格功能在首页里根本按不出来。
 * 2. Alt + 单个字母打头的组合在应用里几乎是空的，按起来也少一个手指。
 *
 * 选字母时避开这些已经有主的组合（它们各自在局部生效，顶栏却是全局的，撞上就等于把它们屏蔽掉）：
 * - Alt+P / Alt+M / Alt+K / Alt+I / Alt+O / Alt+L / Alt+T —— 探索者（笔记本）工具栏
 * - Alt+1 / Alt+2 / Alt+3 —— 搜索结果面板的三个标签
 * - Alt+[ / Alt+] —— 左右侧栏开关
 * - Alt+← / Alt+→ / Alt+↑ / Alt+↓ —— 编辑器与差异窗口的定位键
 * - Ctrl+Alt+E/W/M/L/U/D/Y —— 编辑器导出（仍是三键，与这里不重叠）
 * - Alt+R / Alt+C / Alt+W —— Monaco 查找框里的正则 / 区分大小写 / 全词
 *
 * 关于裸 Alt：`matchesShortcut` 要求 ctrl / meta / alt / shift 四个修饰键<b>逐一相等</b>，
 * 所以 `alt: true` 的条目只认「Alt 且没有 Ctrl」这一种按法，不会顺带匹配 Ctrl+Alt+X。
 */
export const GUAXI_SHORTCUTS = Object.freeze({
  // 左栏文件树的三个图标不在这次改动范围内，维持原有的三键组合：
  // 它们和编辑器、阅读器都不冲突，改键只会让老用户重新学一遍。
  fileTreeNewFile: { ctrl: true, alt: true, code: 'KeyN', label: 'Ctrl+Alt+N' },
  fileTreeNewFolder: { ctrl: true, alt: true, shift: true, code: 'KeyN', label: 'Ctrl+Alt+Shift+N' },
  fileTreeImportFile: { ctrl: true, alt: true, code: 'KeyI', label: 'Ctrl+Alt+I' },

  // 导航仍保留 Ctrl+Alt+数字：Alt+1/2/3 是搜索结果面板的标签快捷键，
  // 而导航是常驻的，撞上就会把搜索面板那三个标签彻底屏蔽。
  navHome: { ctrl: true, alt: true, code: 'Digit1', label: 'Ctrl+Alt+1' },
  navChat: { ctrl: true, alt: true, code: 'Digit2', label: 'Ctrl+Alt+2' },
  navRepository: { ctrl: true, alt: true, code: 'Digit3', label: 'Ctrl+Alt+3' },
  navBrain: { ctrl: true, alt: true, code: 'Digit4', label: 'Ctrl+Alt+4' },
  navApplication: { ctrl: true, alt: true, code: 'Digit5', label: 'Ctrl+Alt+5' },
  navStatistics: { ctrl: true, alt: true, code: 'Digit6', label: 'Ctrl+Alt+6' },
  navSearch: { ctrl: true, alt: true, code: 'Digit7', label: 'Ctrl+Alt+7' },
  navExcalidraw: { ctrl: true, alt: true, code: 'Digit8', label: 'Ctrl+Alt+8' },
  navSetting: { ctrl: true, alt: true, code: 'Digit9', label: 'Ctrl+Alt+9' },

  // 知识库：Base
  createKnowledgeBase: { alt: true, code: 'KeyB', label: 'Alt+B' },
  // 助手：Assistant。注意 Ctrl+Alt+A 是系统级全局快捷键（主窗口最小化时也能唤回），
  // 那一档走 Rust 侧的 global shortcut；这里是应用内的两键版本。
  openAssistant: { alt: true, code: 'KeyA', label: 'Alt+A' },
  // 分屏：Vertical / Horizontal
  splitVertical: { alt: true, code: 'KeyV', label: 'Alt+V' },
  splitHorizontal: { alt: true, code: 'KeyH', label: 'Alt+H' },
  // 大纲：Outline 的 O 被探索者工具栏『切换输出』占了，用 Document outline 的 D。
  openOutline: { alt: true, code: 'KeyD', label: 'Alt+D' },
  // 截图：Screenshot
  captureScreenshot: { alt: true, code: 'KeyS', label: 'Alt+S' },
  // 模板：Template 的 T 被探索者工具栏『打开终端』占了，用第二个字母 E。
  openTemplateManager: { alt: true, code: 'KeyE', label: 'Alt+E' },
  // 资源库：Resource
  openResourceRepoManager: { alt: true, code: 'KeyR', label: 'Alt+R' },
  // 工具栏 / 面包屑显隐：Gutter
  toggleEditorChrome: { alt: true, code: 'KeyG', label: 'Alt+G' },

  editorExportPdf: { ctrl: true, alt: true, code: 'KeyE', label: 'Ctrl+Alt+E' },
  editorExportDocx: { ctrl: true, alt: true, code: 'KeyW', label: 'Ctrl+Alt+W' },
  editorExportHtml: { ctrl: true, alt: true, code: 'KeyM', label: 'Ctrl+Alt+M' },
  editorExportTex: { ctrl: true, alt: true, code: 'KeyL', label: 'Ctrl+Alt+L' },
  editorExportEpub: { ctrl: true, alt: true, code: 'KeyU', label: 'Ctrl+Alt+U' },
  editorExportOdt: { ctrl: true, alt: true, code: 'KeyD', label: 'Ctrl+Alt+D' },
  editorExportTxt: { ctrl: true, alt: true, code: 'KeyY', label: 'Ctrl+Alt+Y' },
})

export function shortcutText(shortcut) {
  return shortcut?.label || ''
}

export function shortcutTooltip(label, shortcut) {
  const text = shortcutText(shortcut)
  return text ? `${label} (${text})` : label
}

function isPlainEditableTarget(target) {
  if (!(target instanceof Element)) return false

  /*
   * Monaco 的「查找」小窗虽然长在 .monaco-editor 里面，但它自己占用了 Alt+R / Alt+C / Alt+W
   * （正则 / 区分大小写 / 全词）。这几个键里 Alt+R 就是顶栏「知识库资源管理」，
   * 不把它排除掉的话，用户在查找框里切正则会被顶栏截走。
   * 只排除查找框本身，编辑器正文仍然放行——那正是顶栏快捷键要在编辑器聚焦时也能用的原因。
   */
  if (target.closest('.monaco-editor .find-widget')) return true

  // Monaco 自己会处理文本编辑，但主界面图标快捷键需要在编辑器聚焦时仍可用。
  if (target.closest('.monaco-editor')) return false

  return Boolean(target.closest('input, textarea, select, [contenteditable="true"], [contenteditable=""]'))
}

export function matchesShortcut(event, shortcut) {
  if (!shortcut) return false
  return Boolean(event.ctrlKey) === Boolean(shortcut.ctrl) &&
    Boolean(event.metaKey) === Boolean(shortcut.meta) &&
    Boolean(event.altKey) === Boolean(shortcut.alt) &&
    Boolean(event.shiftKey) === Boolean(shortcut.shift) &&
    String(event.code || '') === String(shortcut.code || '')
}

export function useGuaxiHotkeys(bindings, options = {}) {
  const list = Array.isArray(bindings) ? bindings : []
  const {
    logger = null,
    scope = 'GuaxiHotkeys',
    ignorePlainEditable = true,
  } = options
  let isListening = false

  const handleKeydown = (event) => {
    if (event.defaultPrevented || event.isComposing || event.repeat) return
    if (ignorePlainEditable && isPlainEditableTarget(event.target)) return

    const binding = list.find((item) => matchesShortcut(event, item.shortcut))
    if (!binding || typeof binding.handler !== 'function') return

    event.preventDefault()
    event.stopPropagation()

    logger?.info?.(`[${scope}] shortcut triggered`, {
      id: binding.id,
      label: binding.label,
      shortcut: shortcutText(binding.shortcut),
    })

    Promise.resolve(binding.handler(event)).catch((error) => {
      logger?.error?.(`[${scope}] shortcut handler failed`, {
        id: binding.id,
        label: binding.label,
        error,
      })
    })
  }

  const startListening = () => {
    if (isListening) return
    isListening = true
    window.addEventListener('keydown', handleKeydown, true)
  }

  const stopListening = () => {
    if (!isListening) return
    isListening = false
    window.removeEventListener('keydown', handleKeydown, true)
  }

  onMounted(startListening)
  onActivated(startListening)
  onDeactivated(stopListening)
  onUnmounted(stopListening)
}
