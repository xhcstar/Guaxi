// 独立阅读器窗口的专用入口。
//
// 【为什么不能复用 main.js】阅读器窗口只需要 DocReaderWindow → ReaderPane 这一棵
// 子树，而 main.js 在启动时会初始化 Monaco worker、VSCode service、LSP 运行时快照，
// 并静态引入全部路由页面（其中 GitPanel / ResumeBuilder 又会把 monaco-editor 拖进来）。
// 每开一本书都是一个新 webview，等于把十几 MB 模块重新解析一遍 —— 实测打开要十秒。
//
// 这里只挂阅读器本身；PDF / Office / 图片等阅读器、划词翻译面板（唯一用到 naive-ui
// 的地方）都由组件按需异步加载，打开一本 MOBI 不会连带下载它们。
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import './styles/guaxiBase.css'
// 阅读器共用的正文样式由多个异步阅读器共享；在这里显式引入一份，避免 Rollup 把它
// 收进主应用的共享 chunk（那样阅读器窗口会被迫在首帧加载一个几 MB 的无关 chunk）。
import './components/ReaderComponent/readers/readerPane.css'
import { initFrontendLogging } from './utils/logger'
import DocReaderWindow from './components/ReaderComponent/DocReaderWindow.vue'
initFrontendLogging({ scope: 'Reader', forwardConsole: false, forwardErrors: true })

const params = new URLSearchParams(window.location.search)

const app = createApp(DocReaderWindow, {
  path: params.get('path') || '',
  name: params.get('name') || '',
  kind: params.get('kind') || '',
  reader: params.get('reader') || '',
  theme: params.get('theme') || '',
})

app.use(createPinia())
app.mount('#app')
