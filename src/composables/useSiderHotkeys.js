import { onActivated, onDeactivated, onMounted, onUnmounted } from 'vue'

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
  let isListening = false

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
