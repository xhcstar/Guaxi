// 外部资料全文阅读器的专用入口。
//
// 与 reader-main.js 同一个理由：这扇窗口只需要「Markdown 阅读器」这一棵子树。
// 走主入口会把 Monaco、LSP、全部路由页面重新解析一遍，而它只是从收件箱点开、
// 读完就关的一扇小窗口。
//
// 正文不在这里的 URL 里，而是由窗口挂载后按感知 id 向 sqlite 取一次
// （cognition_percept_get）：一篇清洗后的全文动辄几十 KB，塞进 query string
// 会被长度与转义拖死。
import { createApp } from 'vue'
import './styles/guaxiBase.css'
import './components/ReaderComponent/articleReader.css'
import { initFrontendLogging } from './utils/logger'
import { applyReaderAppearance, loadReaderAppearance } from './components/ReaderComponent/readerAppearance'
import MarkdownArticleReader from './components/ReaderComponent/MarkdownArticleReader.vue'

initFrontendLogging({ scope: 'ArticleReader', forwardConsole: false, forwardErrors: true })

const params = new URLSearchParams(window.location.search)
const themeParam = String(params.get('theme') || '').trim().toLowerCase()

async function bootstrap() {
  // 主题优先用窗口参数（主窗口打开时已经知道自己在亮还是暗）；没有就按后端配置，
  // 与 DocReaderWindow 同一套来源，避免阅读窗口与主界面配色脱节。
  const appearance = await loadReaderAppearance()
  const theme = themeParam === 'dark' || themeParam === 'light' ? themeParam : appearance.theme
  applyReaderAppearance(theme)

  // 代码块的配色由代码主题注入。GuaxiMarker 在模块求值时先按「当时能读到的主题」注入过
  // 一次，而那时 <html> 还没打上明暗标记（就是上面这行才打的）。这里按真实主题再注入一次，
  // 否则暗色窗口里的代码块会停在亮色底上。
  try {
    const [{ invoke }, { applyCodeThemeToDocument, DEFAULT_CODE_THEME_ID, normalizeCodeThemeId }] = await Promise.all([
      import('@tauri-apps/api/core'),
      import('./monaco/codeThemes'),
    ])
    let codeThemeId = DEFAULT_CODE_THEME_ID
    try {
      codeThemeId = normalizeCodeThemeId(await invoke('get_code_theme'))
    } catch {
      /* 读不到就沿用默认代码主题 */
    }
    applyCodeThemeToDocument(codeThemeId, theme)
  } catch {
    /* 代码主题注入失败不影响正文 */
  }

  const app = createApp(MarkdownArticleReader, {
    perceptId: params.get('id') || '',
    initialTitle: params.get('title') || '',
    theme,
  })
  app.mount('#app')
}

bootstrap()
