import { onMounted, onUnmounted } from 'vue'

const isEditableElement = (target) => {
  if (!(target instanceof Element)) {
    return false
  }

  if (target.closest('input, textarea, [contenteditable="true"], [contenteditable=""]')) {
    return true
  }

  if (target.closest('.monaco-editor')) {
    return true
  }

  return false
}

export function useSiderHotkeys({ toggleLeft, toggleRight }) {
  const handleKeydown = (event) => {
    if (!toggleLeft && !toggleRight) return
    if (event.defaultPrevented || event.isComposing) return
    if (event.ctrlKey || event.metaKey || event.shiftKey || !event.altKey) return
    if (isEditableElement(event.target)) return

    if (event.code === 'BracketLeft' && typeof toggleLeft === 'function') {
      event.preventDefault()
      event.stopPropagation()
      toggleLeft()
      return
    }

    if (event.code === 'BracketRight' && typeof toggleRight === 'function') {
      event.preventDefault()
      event.stopPropagation()
      toggleRight()
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeydown, true)
  })

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeydown, true)
  })
}
