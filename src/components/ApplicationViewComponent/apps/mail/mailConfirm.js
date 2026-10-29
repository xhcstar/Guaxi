import { reactive } from 'vue'

/**
 * 应用内确认框（Promise 版）。
 *
 * 为什么不直接用 `window.confirm`：在 Tauri 的 webview 里它可能被吞掉——返回 false
 * 且不显示任何东西。删账户就是这么坏的：`remove()` 第一行就 return，用户点「删除」
 * 毫无反应也没有提示。所有不可逆的确认都改走这里，由 `MailConfirmHost` 渲染。
 */
export const mailConfirmState = reactive({
  open: false,
  title: '',
  message: '',
  confirmText: '确定',
  danger: true,
  /** 由 host 在用户选择后调用。 */
  resolve: null,
})

export function askMailConfirm(message, options = {}) {
  // 上一个还没答复就被新的顶掉：当作取消，避免 Promise 永远挂着。
  mailConfirmState.resolve?.(false)
  return new Promise((resolve) => {
    Object.assign(mailConfirmState, {
      open: true,
      title: options.title || '',
      message,
      confirmText: options.confirmText || '确定',
      danger: options.danger !== false,
      resolve,
    })
  })
}

export function answerMailConfirm(ok) {
  const resolve = mailConfirmState.resolve
  mailConfirmState.open = false
  mailConfirmState.resolve = null
  resolve?.(ok)
}
