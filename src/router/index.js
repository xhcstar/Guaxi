import { computed, h } from 'vue'
import { createRouter, createWebHistory, useRoute } from 'vue-router'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiChatPage from '../views/GuaxiChatPage.vue'
import GuaxiBrainPage from '../views/GuaxiBrainPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import ExcalidrawPage from '../views/ExcalidrawPage.vue'
import GuaxiReader from '../components/ReaderComponent/GuaxiReader.vue'
import GuaxiMainLayout from '../components/GuaxiMainLayout.vue'
import GitPanel from '../components/HomeViewComponent/GitPanel.vue'
import GuaxiTemplateManager from '../components/HomeViewComponent/GuaxiTemplateManager.vue'
import GuaxiResourceRepoManager from '../components/HomeViewComponent/GuaxiResourceRepoManager.vue'
import GuaxiSlidevManager from '../components/HomeViewComponent/GuaxiSlidevManager.vue'
import GuaxiKnowledgeBaseManager from '../components/HomeViewComponent/GuaxiKnowledgeBaseManager.vue'
import GuaxiMarkdownManual from '../components/HomeViewComponent/GuaxiMarkdownManual.vue'
import GuaxiLanguageLearning from '../components/HomeViewComponent/GuaxiLanguageLearning.vue'
import GuaxiTaskManagerPage from '../views/GuaxiTaskManagerPage.vue'
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

const APP_COMPONENT_MAP = {
  'life-album': LifeAlbumApp,
  'life-habits': TODOApp,
  'music-player': MusicPlayerApp,
  'language-translator': LanguageTranslatorApp,
  'tools-file-transfer': FileTransferAssistantApp,
  'tools-python-playground': PythonPlaygroundApp,
  'tools-tts-book': TTSBookApp,
}

const GuaxiAppWindow = {
  name: 'GuaxiAppWindow',
  setup() {
    const route = useRoute()
    const appId = computed(() => String(route.params?.appId || ''))
    const Comp = computed(() => APP_COMPONENT_MAP[appId.value] || PlaceholderApp)
    const hostPadding = computed(() => (appId.value === 'tools-tts-book' ? '0px' : '16px'))

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


export const routes = [
  {
    path: '/',
    name: 'main',
    component: GuaxiMainLayout,
  },
  {
    path: '/home',
    name: 'home',
    component: GuaxiHomePage,
  },
  {
    path: '/chat',
    name: 'chat',
    component: GuaxiChatPage,
  },
  {
    path: '/repository',
    name: 'repository',
    component: GuaxiRepoPage,
  },
  {
    path: '/brain',
    name: 'brain',
    component: GuaxiBrainPage,
  },
  {
    path: '/application',
    name: 'application',
    component: GuaxiApplicationPage,
  },
  {
    path: '/search',
    name: 'search',
    component: GuaxiSearchPage,
  },
  {
    path: '/excalidraw',
    name: 'excalidraw',
    component: ExcalidrawPage,
  },
  {
    path: '/setting',
    name: 'setting',
    component: GuaxiSettingPage,
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
