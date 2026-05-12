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
import { initialize } from '@codingame/monaco-vscode-api'
import getModelServiceOverride from '@codingame/monaco-vscode-model-service-override'
import 'vscode/localExtensionHost'
import { refreshLspRuntimeExtensionSnapshot } from './monaco/lspCapabilities'
import { configureMonacoWorkers } from './monaco/workerBootstrap'
import { ensureVsCodeVirtualFileSystemProvidersRegistered } from './monaco/vscodeModelBridge'

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
  ensureVsCodeVirtualFileSystemProvidersRegistered()
  // 设计说明：
  // 1. 根包 `@codingame/monaco-vscode-api` 默认会注入 base/files/extensions 等常用 override，
  //    但不会自动注入 model-service-override。
  // 2. 如果缺少这个 override，`createModelReference` 会回退到 StandaloneTextModelService，
  //    最终所有 VSCode 文档命令都会报 `Model not found`，补全链路也就完全断掉。
  // 3. 这里显式补上 model service，让 Monaco model 能被 VSCode 文档服务正确解析。
  await initialize({
    ...getModelServiceOverride(),
  })
  logger.info('[Main] vscode services initialized')
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

const openAssistantPanel = async (source = 'unknown') => {
  try {
    rightSiderStore.setPanel('assistant')
    rightSiderStore.open()
    siderStore.setRightSiderCollapsed('home', false)
    if (router?.currentRoute?.value?.path !== '/') {
      await router.push('/').catch(() => {})
    }
    logger.info('[Main] assistant panel opened', { source })
  } catch (e) {
    logger.error('[Main] open assistant panel failed:', e)
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

  listen(OPEN_ASSISTANT_EVENT, () => {
    openAssistantPanel('global_shortcut')
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
