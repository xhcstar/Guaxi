import { onActivated, onDeactivated, onMounted, onUnmounted } from 'vue'

// 设计说明：主界面图标快捷键统一采用 Ctrl+Alt+字母，避开已有的 Ctrl+P 搜索、
// Ctrl+S 保存、Alt+[ / Alt+] 侧栏切换，也便于在 tooltip 中形成稳定的记忆规则。
export const GUAXI_SHORTCUTS = Object.freeze({
  fileTreeNewFile: { ctrl: true, alt: true, code: 'KeyN', label: 'Ctrl+Alt+N' },
  fileTreeNewFolder: { ctrl: true, alt: true, shift: true, code: 'KeyN', label: 'Ctrl+Alt+Shift+N' },
  fileTreeImportFile: { ctrl: true, alt: true, code: 'KeyI', label: 'Ctrl+Alt+I' },

  createKnowledgeBase: { ctrl: true, alt: true, code: 'KeyK', label: 'Ctrl+Alt+K' },
  openAssistant: { ctrl: true, alt: true, code: 'KeyA', label: 'Ctrl+Alt+A' },
  splitVertical: { ctrl: true, alt: true, code: 'KeyV', label: 'Ctrl+Alt+V' },
  splitHorizontal: { ctrl: true, alt: true, code: 'KeyH', label: 'Ctrl+Alt+H' },
  openOutline: { ctrl: true, alt: true, code: 'KeyO', label: 'Ctrl+Alt+O' },
  captureScreenshot: { ctrl: true, alt: true, code: 'KeyX', label: 'Ctrl+Alt+X' },
  openTemplateManager: { ctrl: true, alt: true, code: 'KeyT', label: 'Ctrl+Alt+T' },
  openResourceRepoManager: { ctrl: true, alt: true, code: 'KeyR', label: 'Ctrl+Alt+R' },

  editorExport: { ctrl: true, alt: true, code: 'KeyE', label: 'Ctrl+Alt+E' },
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
