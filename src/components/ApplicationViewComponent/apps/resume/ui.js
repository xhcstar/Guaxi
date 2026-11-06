import { reactive } from 'vue'

// 应用内确认框：这个环境里 window.confirm 会被静默拦截，统一走这里。
export const confirmState = reactive({
  open: false,
  title: '确认操作',
  body: '',
  confirmText: '确认',
  cancelText: '取消',
  danger: false,
  resolve: null,
})

export function askConfirm(options) {
  return new Promise((resolve) => {
    confirmState.title = options.title || '确认操作'
    confirmState.body = options.body || ''
    confirmState.confirmText = options.confirmText || '确认'
    confirmState.cancelText = options.cancelText || '取消'
    confirmState.danger = Boolean(options.danger)
    confirmState.resolve = resolve
    confirmState.open = true
  })
}

export function settleConfirm(ok) {
  const resolve = confirmState.resolve
  confirmState.open = false
  confirmState.resolve = null
  if (resolve) resolve(ok)
}
