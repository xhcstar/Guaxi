import { reactive } from 'vue'

// 全局共享的确认弹窗状态（本应用不使用 naive-ui 的 dialog-provider，
// 统一用一个轻量自定义 Teleport 弹窗，通过该状态驱动）。
const state = reactive({
  open: false,
  title: '确认操作',
  description: '',
  count: null,
  countUnit: '项',
  confirmLabel: '删除',
  cancelLabel: '取消',
  danger: true,
  _resolve: null,
})

function settle(result) {
  const resolve = state._resolve
  state._resolve = null
  state.open = false
  if (typeof resolve === 'function') resolve(result)
}

/**
 * 弹出确认框，返回 Promise<boolean>。
 * opts: { title, description, count, countUnit, confirmLabel, cancelLabel, danger }
 */
export function useConfirm() {
  const confirm = (opts = {}) => {
    // 若已有未决确认，先取消它，避免状态串扰。
    if (state._resolve) settle(false)
    state.title = opts.title || '确认操作'
    state.description = opts.description || ''
    state.count = opts.count ?? null
    state.countUnit = opts.countUnit || '项'
    state.confirmLabel = opts.confirmLabel || '删除'
    state.cancelLabel = opts.cancelLabel || '取消'
    state.danger = opts.danger !== false
    state.open = true
    return new Promise((resolve) => {
      state._resolve = resolve
    })
  }

  const onConfirm = () => settle(true)
  const onCancel = () => settle(false)

  return { confirmState: state, confirm, onConfirm, onCancel }
}
