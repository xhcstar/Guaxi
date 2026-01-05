import { createRouter, createWebHistory } from 'vue-router'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiChatPage from '../views/GuaxiChatPage.vue'
import GuaxiBrainPage from '../views/GuaxiBrainPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import GuaxiReader from '../components/ReaderComponent/GuaxiReader.vue'
import GuaxiMainLayout from '../components/GuaxiMainLayout.vue'
import GitPanel from '../components/HomeViewComponent/GitPanel.vue'
import GuaxiTemplateManager from '../components/HomeViewComponent/GuaxiTemplateManager.vue'
import GuaxiKnowledgeBaseManager from '../components/HomeViewComponent/GuaxiKnowledgeBaseManager.vue'


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
    path: '/kb-manager',
    name: 'kb-manager',
    component: GuaxiKnowledgeBaseManager,
  },
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;