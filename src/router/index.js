import { createRouter, createWebHistory } from 'vue-router'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiChatPage from '../views/GuaxiChatPage.vue'
import GuaxiStatisticsPage from '../views/GuaxiStatisticsPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import GuaxiReader from '../components/ReaderComponent/GuaxiReader.vue'
import GuaxiMainLayout from '../components/GuaxiMainLayout.vue'
import { Library } from '@vicons/ionicons5'


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
    path: '/statistics',
    name: 'statistics',
    component: GuaxiStatisticsPage,
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
    })
  },
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;