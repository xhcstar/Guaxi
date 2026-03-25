import { createApp } from "vue";
import naive from "naive-ui";
import { createDiscreteApi } from 'naive-ui'
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
import 'vscode/localExtensionHost'

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
const { message } = createDiscreteApi(['message'])

initFrontendLogging({
  scope: 'Main',
  forwardConsole: true,
  forwardErrors: true
})
const logger = createLogger('Main')

try {
  await initialize({})
  logger.info('[Main] vscode services initialized')
} catch (error) {
  logger.error('[Main] vscode services init failed:', error)
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
const BUNDLED_RESOURCE_PREWARM_EVENT = 'bundled_resource_prewarm_status'
let unlistenOpenFileInHome = null
let unlistenOpenAssistant = null
let unlistenBundledResourcePrewarm = null
let bundledResourceStartShown = false

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

  listen(BUNDLED_RESOURCE_PREWARM_EVENT, (event) => {
    const payload = event?.payload || {}
    const phase = String(payload?.phase || '')
    const tip = String(payload?.message || '').trim()
    const report = payload?.report || {}

    logger.info('[Main] bundled_resource_prewarm_status received', { phase, payload })

    if (phase === 'start') {
      if (!bundledResourceStartShown && tip) {
        bundledResourceStartShown = true
        message.info(tip, { duration: 3500 })
      }
      return
    }

    const failedCount = Array.isArray(report?.failed) ? report.failed.length : 0
    const recoveredCount = Array.isArray(report?.recovered) ? report.recovered.length : 0
    const extractedCount = Array.isArray(report?.extracted) ? report.extracted.length : 0

    if (phase === 'finish') {
      if (failedCount > 0) {
        message.warning(tip || `运行资源准备完成，但仍有 ${failedCount} 项失败`, { duration: 6000 })
      } else if (recoveredCount > 0) {
        message.success(tip || `已恢复上次未完成的资源解压（${recoveredCount} 项）`, { duration: 5000 })
      } else if (extractedCount > 0) {
        message.success(tip || `首次启动已完成运行资源准备（${extractedCount} 项）`, { duration: 4000 })
      }
      return
    }

    if (phase === 'partial') {
      message.warning(tip || '运行资源准备部分完成，请稍后重试相关功能', { duration: 6000 })
    }
  }).then((unlisten) => {
    unlistenBundledResourcePrewarm = unlisten
    logger.info('[Main] bundled_resource_prewarm_status listener registered')
  }).catch((e) => {
    logger.error('[Main] bundled_resource_prewarm_status listener register failed:', e)
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
    if (typeof unlistenBundledResourcePrewarm === 'function') {
      await unlistenBundledResourcePrewarm()
    }
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
