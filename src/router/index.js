import { createRouter, createWebHistory } from 'vue-router'
import GuaxiHomePage from '../views/GuaxiHomePage.vue'
import GuaxiChatPage from '../views/GuaxiChatPage.vue'
import GuaxiStatisicsPage from '../views/GuaxiStatisicsPage.vue'
import GuaxiApplicationPage from '../views/GuaxiApplicationPage.vue'
import GuaxiSearchPage from '../views/GuaxiSearchPage.vue'
import GuaxiSettingPage from '../views/GuaxiSettingPage.vue'
import GuaxiRepoPage from '../views/GuaxiRepoPage.vue'
import { path } from '@tauri-apps/api'


export const routes = [
  {
    path: '/',
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
    component: GuaxiStatisicsPage,
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
];

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes,
});

export default router;