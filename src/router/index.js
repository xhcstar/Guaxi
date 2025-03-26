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
    props: (route) => ({
      fileType: route.query.fileType,
      fileName: route.query.fileName
    })
  },
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;