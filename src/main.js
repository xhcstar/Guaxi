import { createApp } from "vue";
import naive from "naive-ui";
import { addIcon } from '@iconify/vue'
import mdiFolderOutline from '@iconify-icons/mdi/folder-outline'
import mdiFileOutline from '@iconify-icons/mdi/file-outline'
import mdiFileDocumentOutline from '@iconify-icons/mdi/file-document-outline'
import mdiFileDelimitedOutline from '@iconify-icons/mdi/file-delimited-outline'
import mdiBash from '@iconify-icons/mdi/bash'
import mdiGit from '@iconify-icons/mdi/git'
import vscodeFileTypeJsOfficial from '@iconify-icons/vscode-icons/file-type-js-official'
import vscodeFileTypeTypescriptOfficial from '@iconify-icons/vscode-icons/file-type-typescript-official'
import vscodeFileTypePython from '@iconify-icons/vscode-icons/file-type-python'
import vscodeFileTypeJava from '@iconify-icons/vscode-icons/file-type-java'
import vscodeFileTypeCpp from '@iconify-icons/vscode-icons/file-type-cpp'
import vscodeFileTypeCppHeader from '@iconify-icons/vscode-icons/file-type-cppheader'
import vscodeFileTypeC from '@iconify-icons/vscode-icons/file-type-c'
import vscodeFileTypeCHeader from '@iconify-icons/vscode-icons/file-type-cheader'
import vscodeFileTypeCsharp from '@iconify-icons/vscode-icons/file-type-csharp'
import vscodeFileTypeGo from '@iconify-icons/vscode-icons/file-type-go'
import vscodeFileTypeRust from '@iconify-icons/vscode-icons/file-type-rust'
import vscodeFileTypePhp from '@iconify-icons/vscode-icons/file-type-php'
import vscodeFileTypeRuby from '@iconify-icons/vscode-icons/file-type-ruby'
import vscodeFileTypeMarkdown from '@iconify-icons/vscode-icons/file-type-markdown'
import vscodeFileTypeLua from '@iconify-icons/vscode-icons/file-type-lua'
import vscodeFileTypeVue from '@iconify-icons/vscode-icons/file-type-vue'
import vscodeFileTypeWord from '@iconify-icons/vscode-icons/file-type-word'
import vscodeFileTypeExcel from '@iconify-icons/vscode-icons/file-type-excel'
import vscodeFileTypePowerpoint from '@iconify-icons/vscode-icons/file-type-powerpoint'
import vscodeFileTypePdf from '@iconify-icons/vscode-icons/file-type-pdf2'
import vscodeFileTypeJson from '@iconify-icons/vscode-icons/file-type-json'
import vscodeFileTypeHtml from '@iconify-icons/vscode-icons/file-type-html'
import vscodeFileTypeCss from '@iconify-icons/vscode-icons/file-type-css'
import vscodeFileTypeYaml from '@iconify-icons/vscode-icons/file-type-yaml'
import vscodeFileTypeToml from '@iconify-icons/vscode-icons/file-type-toml'
import vscodeFileTypeXml from '@iconify-icons/vscode-icons/file-type-xml'
import vscodeFileTypeZip from '@iconify-icons/vscode-icons/file-type-zip'
import vscodeFileTypeImage from '@iconify-icons/vscode-icons/file-type-image'
import vscodeFileTypeSvg from '@iconify-icons/vscode-icons/file-type-svg'
import vscodeFileTypeAudio from '@iconify-icons/vscode-icons/file-type-audio'
import vscodeFileTypeVideo from '@iconify-icons/vscode-icons/file-type-video'
import "pdfjs-dist/web/pdf_viewer.css";
// 六个阅读器共用的外壳样式（工具栏 + 双语行）。选择器都带 .guaxi-reader-controls 前缀，
// 不会外溢；放在全局是为了让它们只有一份，而不是每个阅读器各抄一份然后各自漂移。
import "./components/ReaderComponent/readerChrome.css";
import App from "./App.vue";
import router from "./router";
import { createPinia } from 'pinia';
import { initTauriEvents, cleanupTauriEvents } from './stores/tauriEvents';
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { useSettingsStore } from './stores/settingsStore'
import { useFileTreeStore } from './stores/fileTreeStore'
import { useEditorStore } from './stores/editorStore'
import { useChatStore } from './stores/chatStore'
import { useRightSiderStore } from './stores/rightSiderStore'
import { useSiderStore } from './stores/useSiderStore'
import { initFrontendLogging, createLogger } from './utils/logger'
import { refreshLspRuntimeExtensionSnapshot } from './monaco/lspCapabilities'
import { configureMonacoWorkers } from './monaco/workerBootstrap'
import { initializeGuaxiVscodeServices } from './monaco/vscodeApiBootstrap'

// 按名字用的图标（<Icon icon="mdi:xxx" />）必须在这里离线注册：没注册的名字会让
// @iconify/vue 去 api.iconify.design 现取，release 包在断网、内网、CSP 收紧的机器上
// 就是一个空白。只在一两处用到的图标更省事的做法是直接 import 图标对象传给 :icon
// （见 GuaxiHeader.vue / GuaxiSearchNavigator.vue），名字写错在构建期就报错。
// `npm run check:icons` 会扫 src/ 里的图标名字，漏注册的直接失败。
const iconRegistry = {
  'mdi:folder-outline': mdiFolderOutline,
  'mdi:file-outline': mdiFileOutline,
  'mdi:file-document-outline': mdiFileDocumentOutline,
  'mdi:file-delimited-outline': mdiFileDelimitedOutline,
  'mdi:bash': mdiBash,
  'mdi:git': mdiGit,
  'vscode-icons:file-type-js-official': vscodeFileTypeJsOfficial,
  'vscode-icons:file-type-typescript-official': vscodeFileTypeTypescriptOfficial,
  'vscode-icons:file-type-python': vscodeFileTypePython,
  'vscode-icons:file-type-java': vscodeFileTypeJava,
  'vscode-icons:file-type-cpp': vscodeFileTypeCpp,
  'vscode-icons:file-type-cpp-header': vscodeFileTypeCppHeader,
  'vscode-icons:file-type-c': vscodeFileTypeC,
  'vscode-icons:file-type-c-header': vscodeFileTypeCHeader,
  'vscode-icons:file-type-csharp': vscodeFileTypeCsharp,
  'vscode-icons:file-type-go': vscodeFileTypeGo,
  'vscode-icons:file-type-rust': vscodeFileTypeRust,
  'vscode-icons:file-type-php': vscodeFileTypePhp,
  'vscode-icons:file-type-ruby': vscodeFileTypeRuby,
  'vscode-icons:file-type-markdown': vscodeFileTypeMarkdown,
  'vscode-icons:file-type-lua': vscodeFileTypeLua,
  'vscode-icons:file-type-vue': vscodeFileTypeVue,
  'vscode-icons:file-type-word': vscodeFileTypeWord,
  'vscode-icons:file-type-excel': vscodeFileTypeExcel,
  'vscode-icons:file-type-powerpoint': vscodeFileTypePowerpoint,
  'vscode-icons:file-type-pdf': vscodeFileTypePdf,
  'vscode-icons:file-type-json': vscodeFileTypeJson,
  'vscode-icons:file-type-html': vscodeFileTypeHtml,
  'vscode-icons:file-type-css': vscodeFileTypeCss,
  'vscode-icons:file-type-yaml': vscodeFileTypeYaml,
  'vscode-icons:file-type-toml': vscodeFileTypeToml,
  'vscode-icons:file-type-xml': vscodeFileTypeXml,
  'vscode-icons:file-type-zip': vscodeFileTypeZip,
  'vscode-icons:file-type-image': vscodeFileTypeImage,
  'vscode-icons:file-type-svg': vscodeFileTypeSvg,
  'vscode-icons:file-type-audio': vscodeFileTypeAudio,
  'vscode-icons:file-type-video': vscodeFileTypeVideo
};

for (const [name, data] of Object.entries(iconRegistry)) {
  addIcon(name, data);
}

const bootstrap = async () => {
const app = createApp(App);
initFrontendLogging({
  scope: 'Main',
  forwardConsole: true,
  forwardErrors: true
})
const logger = createLogger('Main')

try {
  // 设计说明：
  // 1. 先在应用启动早期显式注册 `MonacoEnvironment.getWorker`，确保后续 Monaco、
  //    TextMate 背景分词、以及 VSCode service override 在第一次取 Worker 时就能拿到正确入口。
  // 2. 之前仅依赖 HTML 注入的 `getWorkerUrl`，在 Tauri + Vite 场景下路径一旦解析失败，
  //    浏览器就会把返回的 `index.html` 当成模块脚本去加载，最终出现 `text/html` MIME 报错。
  configureMonacoWorkers()
  logger.info('[Main] Monaco worker bootstrap configured')
} catch (error) {
  logger.error('[Main] Monaco worker bootstrap failed:', error)
}

try {
  await initializeGuaxiVscodeServices()
  logger.info('[Main] vscode services initialized for language client')
} catch (error) {
  logger.error('[Main] vscode services init failed:', error)
}

try {
  await refreshLspRuntimeExtensionSnapshot()
  logger.info('[Main] LSP runtime extension snapshot initialized')
} catch (error) {
  logger.warn('[Main] LSP runtime extension snapshot init failed:', error)
}

const pinia = createPinia()

app.use(naive);
app.use(router);
app.use(pinia);

const settingsStore = useSettingsStore(pinia)
const fileTreeStore = useFileTreeStore(pinia)
const editorStore = useEditorStore(pinia)
const chatStore = useChatStore(pinia)
const rightSiderStore = useRightSiderStore(pinia)
const siderStore = useSiderStore(pinia)

const OPEN_FILE_IN_HOME_EVENT = 'open_file_in_home'
const OPEN_ASSISTANT_EVENT = 'assistant_global_shortcut'
let unlistenOpenFileInHome = null
let unlistenOpenAssistant = null

/**
 * 唤醒 / 收起智能助手侧栏。
 *
 * Ctrl+Alt+A 在 Rust 端注册成了**系统级全局快捷键**（见 main.rs 的 global shortcut 插件），
 * 按下后按键被操作系统截走，不会再送进 webview —— 也就是说 GuaxiHeader 里那份会 toggle
 * 的热键处理根本收不到这个键。真正响应的是这里的 `assistant_global_shortcut` 事件，
 * 所以"再按一次收起"必须由这里自己判断，否则快捷键只能展开、永远收不回来。
 *
 * 判断与 Header 的 openAssistantPanel 保持同一套条件：回到首页、右侧栏展开着、且当前面板就是助手。
 *
 * @param {{ mode?: 'toggle' | 'reveal' }} options
 *   `reveal` 只展开不收起 —— 后端在主窗口本来最小化/隐藏、被这个快捷键唤回时用这一档。
 */
const toggleAssistantPanelFromShortcut = async (source = 'unknown', options = {}) => {
  const mode = options.mode === 'reveal' ? 'reveal' : 'toggle'
  try {
    const isOnHome = router?.currentRoute?.value?.path === '/'
    const isVisible = siderStore.isRightSiderCollapsed?.['home'] === false

    const isAssistantOpen =
      isOnHome && isVisible && rightSiderStore.isOpen && rightSiderStore.panel === 'assistant'

    if (mode === 'toggle' && isAssistantOpen) {
      rightSiderStore.close()
      siderStore.setRightSiderCollapsed('home', true)
      logger.info('[Main] assistant panel closed', { source })
      return
    }

    rightSiderStore.setPanel('assistant')
    rightSiderStore.open()
    siderStore.setRightSiderCollapsed('home', false)
    if (!isOnHome) {
      await router.push('/').catch(() => {})
    }
    logger.info('[Main] assistant panel opened', { source })
  } catch (e) {
    logger.error('[Main] toggle assistant panel failed:', e)
  }
}

const windowLabel = (() => {
  try {
    return String(getCurrentWebviewWindow()?.label || '')
  } catch (e) {
    logger.warn('[Main] getCurrentWebviewWindow failed:', e)
    return ''
  }
})()

if (windowLabel === 'main') {
  listen(OPEN_FILE_IN_HOME_EVENT, async (event) => {
    const payload = event?.payload || {}
    const path = String(payload?.path || '')
    const kind = String(payload?.kind || '')
    const lineNumberRaw = payload?.lineNumber ?? payload?.startLine ?? payload?.line ?? 1
    const lineNumber = Math.max(1, Number(lineNumberRaw) || 1)

    if (!path) {
      logger.warn('[Main] open_file_in_home ignored (empty path)', { payload })
      return
    }

    logger.info('[Main] open_file_in_home received', { path, kind, lineNumber })

    try {
      const res = await fileTreeStore.fetchFileContent(path)
      if (!res?.ok) {
        logger.error('[Main] open_file_in_home failed (read file failed)', { path, res })
        return
      }

      await fileTreeStore.openFileInTab(path, res.content, { unsupported: !!res.unsupported })

      editorStore.setFile({
        path,
        content: String(res.content || ''),
        highlightInfo: {
          lineNumber,
          matches: [{ start: 0, end: 1000000 }],
          text: ''
        }
      })

      if (router?.currentRoute?.value?.path !== '/') {
        router.push('/').catch(() => {})
      }
    } catch (e) {
      logger.error('[Main] open_file_in_home failed:', e)
    }
  }).then((unlisten) => {
    unlistenOpenFileInHome = unlisten
    logger.info('[Main] open_file_in_home listener registered')
  }).catch((e) => {
    logger.error('[Main] open_file_in_home listener register failed:', e)
  })

  listen(OPEN_ASSISTANT_EVENT, (event) => {
    // 后端会区分「窗口本就在前台 → toggle」和「窗口刚被唤回 → 只展开」：
    // 最小化状态下按快捷键，用户要的是看到助手，不该因为侧栏上次是开着的就把它关掉。
    const payload = String(event?.payload || '')
    const mode = payload === 'reveal_assistant' ? 'reveal' : 'toggle'
    toggleAssistantPanelFromShortcut('global_shortcut', { mode })
  }).then((unlisten) => {
    unlistenOpenAssistant = unlisten
    logger.info('[Main] assistant_global_shortcut listener registered')
  }).catch((e) => {
    logger.error('[Main] assistant_global_shortcut listener register failed:', e)
  })
} else {
  logger.info('[Main] skip open_file_in_home listener (not main window)', { windowLabel })
}

const getThemeFromUrl = () => {
  try {
    let qs = window.location.search || ''
    if (!qs) {
      const hash = window.location.hash || ''
      const qIdx = hash.indexOf('?')
      if (qIdx >= 0) qs = hash.slice(qIdx)
    }
    const theme = new URLSearchParams(qs).get('theme')
    return theme === 'dark' || theme === 'light' ? theme : null
  } catch {
    return null
  }
}

const theme = getThemeFromUrl()
if (theme) {
  settingsStore.appearanceTheme = theme
  settingsStore.applyAppearanceTheme(theme)
}

// 初始化Tauri事件系统
initTauriEvents().catch(error => {
  logger.error('全局事件初始化失败:', error);
});

chatStore.initChatEventListeners().catch((error) => {
  logger.error('聊天事件初始化失败:', error);
});

invoke('init_backend_services').catch(error => {
  logger.error('后端服务初始化失败:', error);
});

app.mount("#app");

// 应用卸载时清理
window.addEventListener('beforeunload', async () => {
  try {
    if (typeof unlistenOpenFileInHome === 'function') {
      await unlistenOpenFileInHome()
    }
  } catch (e) {
    logger.error('[Main] unlisten open_file_in_home failed:', e)
  }

  try {
    if (typeof unlistenOpenAssistant === 'function') {
      await unlistenOpenAssistant()
    }
  } catch (e) {
    logger.error('[Main] unlisten assistant_global_shortcut failed:', e)
  }

  await cleanupTauriEvents();
});
}

bootstrap().catch((error) => {
  const logger = createLogger('Main')
  logger.error('[Main] bootstrap failed:', error)
})
