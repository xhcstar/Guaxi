// 独立写信窗口的专用入口。
//
// 与 reader-main.js 同一个理由：写信窗口只需要 MailComposer 这一棵子树。
// 走主入口会把 Monaco、LSP、全部路由页面重新解析一遍——每开一封回复都要等几秒，
// 而这扇窗口的生命周期通常只有几分钟。
//
// 草稿内容不在这里的 URL 里，而是由后端 `mail_take_compose_request` 交接：
// 带引用的回复正文动辄几十 KB，塞进 query string 会被长度与转义拖死。
import { createApp, h } from 'vue'
import { createPinia } from 'pinia'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import './styles/guaxiBase.css'
import './components/ApplicationViewComponent/apps/mail/mail.css'
import { initFrontendLogging } from './utils/logger'
import { FALLBACK_THEMES, themeToStyle } from './components/ApplicationViewComponent/apps/mail/theme'
import MailComposeWindow from './components/ApplicationViewComponent/apps/mail/MailComposeWindow.vue'

initFrontendLogging({ scope: 'MailCompose', forwardConsole: false, forwardErrors: true })

// 当前窗口 label：由 Tauri 注入（`__TAURI_INTERNALS__.metadata.currentWebview.label`）。
const LABEL = (() => {
  try {
    return window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || ''
  } catch {
    return ''
  }
})()

const params = new URLSearchParams(window.location.search)
const themeMode = params.get('theme') === 'light' ? 'light' : 'dark'
// MailComposer 的配色完全依赖 --mail-* 变量（主题 token），这里先落一套兜底，
// 后端内置主题在组件挂载后由主题模块覆盖。
const themeTokens = themeMode === 'light' ? FALLBACK_THEMES.paper : FALLBACK_THEMES.ink

const app = createApp({
  setup() {
    return () =>
      h(
        'div',
        {
          class: ['mail-root', themeMode === 'light' ? '' : 'mail-doc-dark'],
          style: { ...themeToStyle(themeTokens), height: '100vh' },
        },
        [h(MailComposeWindow, { label: LABEL, themeMode })],
      )
  },
})

app.use(createPinia())
app.mount('#app')

// 窗口已经开着时，主窗口再发一次请求：让页面重新取草稿。
listen('mail-compose:request', async () => {
  const { reloadComposeRequest } = await import(
    './components/ApplicationViewComponent/apps/mail/composeWindowBridge'
  )
  reloadComposeRequest()
})

// 首帧就绪后告诉后端可以显示窗口了（后端有 5 秒兜底，脚本出错也不会永远隐藏）。
requestAnimationFrame(() => {
  invoke('mail_compose_window_ready', { label: LABEL }).catch(() => {})
})
