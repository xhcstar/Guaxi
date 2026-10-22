import { computed, h } from 'vue'
import { createRouter, createWebHistory, useRoute } from 'vue-router'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiChatPage from '../views/GuaxiChatPage.vue'
import GuaxiAgentWorkspacePage from '../views/GuaxiAgentWorkspacePage.vue'
import GuaxiBrainPage from '../views/GuaxiBrainPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import GuaxiStatisticsPage from '../views/GuaxiStatisticsPage.vue'
import ExcalidrawPage from '../views/ExcalidrawPage.vue'
import GuaxiReader from '../components/ReaderComponent/GuaxiReader.vue'
import DocReaderWindow from '../components/ReaderComponent/DocReaderWindow.vue'
import GuaxiMainLayout from '../components/GuaxiMainLayout.vue'
import GitPanel from '../components/HomeViewComponent/GitPanel.vue'
import GuaxiTemplateManager from '../components/HomeViewComponent/GuaxiTemplateManager.vue'
import GuaxiResourceRepoManager from '../components/HomeViewComponent/GuaxiResourceRepoManager.vue'
import GuaxiSlidevManager from '../components/HomeViewComponent/GuaxiSlidevManager.vue'
import GuaxiKnowledgeBaseManager from '../components/HomeViewComponent/GuaxiKnowledgeBaseManager.vue'
import GuaxiMarkdownManual from '../components/HomeViewComponent/GuaxiMarkdownManual.vue'
import GuaxiSyntaxManualWindow from '../components/HomeViewComponent/GuaxiSyntaxManualWindow.vue'
import GuaxiLanguageLearning from '../components/HomeViewComponent/GuaxiLanguageLearning.vue'
import GuaxiTaskManagerPage from '../views/GuaxiTaskManagerPage.vue'
import GuaxiDiffComparePage from '../views/GuaxiDiffComparePage.vue'
import GuaxiLifeAlbumViewerPage from '../views/GuaxiLifeAlbumViewerPage.vue'
import GuaxiLibrarySitePage from '../views/GuaxiLibrarySitePage.vue'
import FlipboardFeedManager from '../components/SearchViewComponent/FlipboardFeedManager.vue'
import GuaxiDataLabWorkbench from '../components/HomeViewComponent/GuaxiDataLabWorkbench.vue'
import GuaxiMathWorkbench from '../components/HomeViewComponent/GuaxiMathWorkbench.vue'

import LifeAlbumApp from '../components/ApplicationViewComponent/apps/LifeAlbumApp.vue'
import MusicPlayerApp from '../components/ApplicationViewComponent/apps/MusicPlayerApp.vue'
import LanguageTranslatorApp from '../components/ApplicationViewComponent/apps/LanguageTranslatorApp.vue'
import FileTransferAssistantApp from '../components/ApplicationViewComponent/apps/FileTransferAssistantApp.vue'
import PythonPlaygroundApp from '../components/ApplicationViewComponent/apps/PythonPlaygroundApp.vue'
import PlaceholderApp from '../components/ApplicationViewComponent/apps/PlaceholderApp.vue'
import TODOApp from '../components/ApplicationViewComponent/apps/TODO.vue'
import TTSBookApp from '../components/ApplicationViewComponent/apps/TTSBookApp.vue'
import OcrApp from '../components/ApplicationViewComponent/apps/OcrApp.vue'
import ResumeBuilderApp from '../components/ApplicationViewComponent/apps/ResumeBuilderApp.vue'
import AIFilmStudioApp from '../components/ApplicationViewComponent/apps/AIFilmStudioApp.vue'
import RemoteTerminalApp from '../components/ApplicationViewComponent/apps/RemoteTerminalApp.vue'
import ResourceHunterApp from '../components/ApplicationViewComponent/apps/ResourceHunterApp.vue'
import MailApp from '../components/ApplicationViewComponent/apps/MailApp.vue'

const APP_COMPONENT_MAP = {
  'life-album': LifeAlbumApp,
  'life-habits': TODOApp,
  'music-player': MusicPlayerApp,
  'language-translator': LanguageTranslatorApp,
  'tools-file-transfer': FileTransferAssistantApp,
  'tools-python-playground': PythonPlaygroundApp,
  'tools-tts-book': TTSBookApp,
  'tools-ocr': OcrApp,
  'work-timer': ResumeBuilderApp,
  'creation-ai-film-studio': AIFilmStudioApp,
  'tools-remote-terminal': RemoteTerminalApp,
  'tools-resource-hunter': ResourceHunterApp,
  'tools-mail': MailApp,
}

const GuaxiAppWindow = {
  name: 'GuaxiAppWindow',
  setup() {
    const route = useRoute()
    const appId = computed(() => String(route.params?.appId || ''))
    const Comp = computed(() => APP_COMPONENT_MAP[appId.value] || PlaceholderApp)
    // Workbench-style apps own their full window layout, so the route host should not add outer padding.
    const fullBleedAppIds = new Set(['life-album', 'music-player', 'tools-tts-book', 'work-timer', 'creation-ai-film-studio', 'tools-remote-terminal', 'tools-resource-hunter', 'tools-mail'])
    const hostPadding = computed(() => (fullBleedAppIds.has(appId.value) ? '0px' : '16px'))

    return () =>
      h(
        'div',
        {
          style: {
            height: '100vh',
            padding: hostPadding.value,
            boxSizing: 'border-box',
            background: 'var(--guaxi-app-bg)',
          },
        },
        [h(Comp.value)],
      )
  },
}

// 设计说明：最左侧全局导航栏在多个页面中有重复实现，导航项和快捷键 id 统一放在路由层声明。
// App 根组件会读取这份配置注册全局快捷键，并给当前页面渲染出来的 icon 自动补充 title/aria-label。
export const mainNavigationItems = Object.freeze([
  { path: '/', routeName: 'main', label: '知识编辑', shortcutId: 'navHome' },
  { path: '/chat', routeName: 'chat', label: '智能助手', shortcutId: 'navChat' },
  { path: '/repository', routeName: 'repository', label: '仓库', shortcutId: 'navRepository' },
  { path: '/brain', routeName: 'brain', label: '知识图谱', shortcutId: 'navBrain' },
  { path: '/application', routeName: 'application', label: '应用', shortcutId: 'navApplication' },
  { path: '/statistics', routeName: 'statistics', label: '统计', shortcutId: 'navStatistics' },
  { path: '/search', routeName: 'search', label: '搜索', shortcutId: 'navSearch' },
  { path: '/excalidraw', routeName: 'excalidraw', label: '白板', shortcutId: 'navExcalidraw' },
  { path: '/setting', routeName: 'setting', label: '设置', shortcutId: 'navSetting' },
])

const navMeta = (routeName) => {
  const item = mainNavigationItems.find((entry) => entry.routeName === routeName)
  return item ? { navLabel: item.label, navShortcutId: item.shortcutId } : {}
}


export const routes = [
  {
    path: '/',
    name: 'main',
    component: GuaxiMainLayout,
    meta: navMeta('main'),
  },
  {
    path: '/home',
    name: 'home',
    component: GuaxiHomePage,
    meta: navMeta('main'),
  },
  {
    path: '/chat',
    name: 'chat',
    component: GuaxiAgentWorkspacePage,
    meta: navMeta('chat'),
  },
  {
    // 旧聊天页保留一个版本：新的工作台覆盖了「Agent 任务」，但「多模型闲聊 / 网页模型」
    // 这些场景还没搬过去。并存一段时间，确认覆盖后再删。
    path: '/chat-legacy',
    name: 'chatLegacy',
    component: GuaxiChatPage,
    meta: { navLabel: '旧版对话' },
  },
  {
    path: '/repository',
    name: 'repository',
    component: GuaxiRepoPage,
    meta: navMeta('repository'),
  },
  {
    path: '/brain',
    name: 'brain',
    component: GuaxiBrainPage,
    meta: navMeta('brain'),
  },
  {
    path: '/application',
    name: 'application',
    component: GuaxiApplicationPage,
    meta: navMeta('application'),
  },
  {
    path: '/search',
    name: 'search',
    component: GuaxiSearchPage,
    meta: navMeta('search'),
  },
  {
    path: '/excalidraw',
    name: 'excalidraw',
    component: ExcalidrawPage,
    meta: navMeta('excalidraw'),
  },
  {
    path: '/setting',
    name: 'setting',
    component: GuaxiSettingPage,
    meta: navMeta('setting'),
  },
  {
    path: '/statistics',
    name: 'statistics',
    component: GuaxiStatisticsPage,
    meta: navMeta('statistics'),
  },
  {
    path: '/reader',
    name: 'reader',
    component: GuaxiReader,
    props: (route) => ({  // 关键：显式传递查询参数作为props
      filePath: route.query.filePath,  // 直接传递Base64编码的字符串
      fileType: route.query.fileType,
      fileName: route.query.fileName,
      library: route.query.library,
      importFileId: route.query.importFileId,
    })
  },
  {
    path: '/doc-reader',
    name: 'doc-reader',
    component: DocReaderWindow,
    // 独立窗口：参数全部来自 URL query（Windows 路径含空格/中文时必须显式编码）。
    props: (route) => ({
      path: route.query.path,
      name: route.query.name,
      kind: route.query.kind,
      reader: route.query.reader,
      theme: route.query.theme,
    })
  },
  {
    path: '/git',
    name: 'git',
    component: GitPanel,
    props: (route) => ({  // 关键：显式传递查询参数作为props
      workspacePath: route.query.workspacePath,
    })
  },
  {
    path: '/templates',
    name: 'templates',
    component: GuaxiTemplateManager,
  },
  {
    path: '/resource-repo',
    name: 'resource-repo',
    component: GuaxiResourceRepoManager,
  },
  {
    path: '/slidev',
    name: 'slidev',
    component: GuaxiSlidevManager,
    props: (route) => ({
      theme: route.query.theme,
    }),
  },
  {
    path: '/markdown-manual',
    name: 'markdown-manual',
    component: GuaxiMarkdownManual,
    props: (route) => ({
      theme: route.query.theme,
    })
  },
  {
    path: '/syntax-manual',
    name: 'syntax-manual',
    component: GuaxiSyntaxManualWindow,
    props: (route) => ({
      language: route.query.language,
      sourcePath: route.query.sourcePath,
      theme: route.query.theme,
    })
  },
  {
    path: '/language-learning',
    name: 'language-learning',
    component: GuaxiLanguageLearning,
  },
  {
    path: '/task-manager',
    name: 'task-manager',
    component: GuaxiTaskManagerPage,
  },
  {
    path: '/diff-compare',
    name: 'diff-compare',
    component: GuaxiDiffComparePage,
  },
  {
    path: '/life-album-viewer',
    name: 'life-album-viewer',
    component: GuaxiLifeAlbumViewerPage,
    props: (route) => ({
      theme: route.query.theme,
    }),
  },
  {
    path: '/library-site',
    name: 'library-site',
    component: GuaxiLibrarySitePage,
  },
  {
    path: '/kb-manager',
    name: 'kb-manager',
    component: GuaxiKnowledgeBaseManager,
  },
  {
    path: '/flipboard-feeds',
    name: 'flipboard-feeds',
    component: FlipboardFeedManager,
    props: (route) => ({
      category: route.query.category,
    })
  },
  {
    path: '/app/:appId',
    name: 'app-window',
    component: GuaxiAppWindow,
  },
  {
    path: '/data-lab-workbench',
    name: 'data-lab-workbench',
    component: GuaxiDataLabWorkbench,
  },
  {
    path: '/math-workbench',
    name: 'math-workbench',
    component: GuaxiMathWorkbench,
  },
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;
