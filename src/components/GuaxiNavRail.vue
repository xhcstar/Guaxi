<template>
  <div class="sider-icon-container">
    <div class="sider-icon-top">
      <n-icon
        v-for="item in topItems"
        :key="item.path"
        size="24"
        class="nav-icon"
        :class="{ active: activePath === item.path }"
        :data-nav-path="item.path"
        :title="item.label"
        :aria-label="item.label"
        :aria-current="activePath === item.path ? 'page' : undefined"
        @click="navigateTo(item.path)"
      >
        <span class="nav-glyph" v-html="item.svg" />
      </n-icon>
    </div>

    <div class="sider-icon-bottom">
      <n-icon
        v-if="settingItem"
        size="24"
        class="nav-icon"
        :class="{ active: activePath === settingItem.path }"
        :data-nav-path="settingItem.path"
        :title="settingItem.label"
        :aria-label="settingItem.label"
        :aria-current="activePath === settingItem.path ? 'page' : undefined"
        @click="navigateTo(settingItem.path)"
      >
        <span class="nav-glyph" v-html="settingItem.svg" />
      </n-icon>
    </div>
  </div>
</template>

<script setup>
/**
 * 全局左侧导航图标列（44px 宽）。
 *
 * 背景：这条列此前在 Home / Chat / Brain / Repo / Search / Setting / Statistics /
 * Application / Excalidraw 等页面的 `GuaxiSidebar*View.vue` 里各复制了一份。
 * 复制件之间慢慢走样：有的把 SVG 的所有 fill / stroke 强制刷成 currentColor，
 * 多色图标被压成实心色块，和这里的观感对不上；新增页面时如果忘了复制，
 * 界面还会少一列导航、并且再也回不去。
 *
 * 现在各页面的侧栏都直接用这个组件，图标、间距、悬停与高亮只有一处实现。
 * 各页面自己的 `GuaxiSidebar*View.vue` 只负责图标列右侧的内容面板。
 *
 * 图标沿用原来那份 SVG 字符串：这些路径是沿用的图形资产，
 * 换成图标库会直接改变观感，不属于本次统一的范围。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { NIcon } from 'naive-ui'
import { mainNavigationItems } from '../router/navigationItems.js'

const props = defineProps({
  /** 覆盖当前高亮项；省略时读 window.location.hash。 */
  active: { type: String, default: '' },
})

const activePath = ref(props.active || (window.location.hash || '#/').slice(1) || '/')

const syncActivePath = () => {
  if (props.active) return
  activePath.value = (window.location.hash || '#/').slice(1) || '/'
}

onMounted(() => {
  window.addEventListener('hashchange', syncActivePath)
})

onBeforeUnmount(() => {
  window.removeEventListener('hashchange', syncActivePath)
})

/**
 * 导航图标。key 与 `mainNavigationItems[].path` 一一对应，
 * 顺序必须与顶部图标列一致（Home / Chat / Repo / Brain / App / Statistics / Search / Excalidraw）。
 */
const NAV_ICONS = {
  '/': `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12H3l9-9l9 9h-2"></path><path d="M5 12v7a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-7"></path><path d="M9 21v-6a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v6"></path></g></svg>`,
  '/chat': `<svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg"><path d="M919.192216 976.840649a42.620541 42.620541 0 0 1-21.919135-6.088649l-185.094919-110.675027A560.95827 560.95827 0 0 1 512 896.249081c-274.681081 0-498.162162-192.982486-498.162162-430.190703C13.837838 228.850162 237.318919 35.867676 512 35.867676S1010.162162 228.850162 1010.162162 466.058378c0 104.64173-42.952649 203.637622-121.66227 281.821406l70.379243 168.683243c7.195676 17.269622 2.601514 37.251459-11.374703 49.567135-8.025946 7.084973-18.127568 10.710486-28.312216 10.710487z m-203.277838-208.45319c7.610811 0 15.193946 2.048 21.919136 6.088649l91.108324 54.438054-31.494919-75.443892a43.699892 43.699892 0 0 1 11.623784-49.816216c74.170811-64.345946 115.020108-148.729081 115.020108-237.595676C924.090811 276.756757 739.217297 122.713946 512 122.713946S99.909189 276.756757 99.909189 466.058378c0 189.301622 184.873514 343.344432 412.090811 343.344433 65.785081 0 128.719568-12.647784 187.142919-37.583568 5.369081-2.297081 11.07027-3.431784 16.771459-3.431784zM260.953946 470.154378a56.32 56.32 0 0 1 56.347676-56.015567 56.347676 56.347676 0 0 1 55.794162 56.596757c0 31.135135-24.908108 56.430703-55.794162 56.569081A56.32 56.32 0 0 1 260.981622 471.316757v-1.134703z m186.478703 0c0 31.965405 25.710703 57.897514 57.399351 57.897514a57.648432 57.648432 0 0 0 57.371676-57.897514 57.648432 57.648432 0 0 0-57.371676-57.897513 57.648432 57.648432 0 0 0-57.399351 57.897513z m186.506378 0a56.32 56.32 0 0 1 56.347676-56.015567 56.347676 56.347676 0 0 1 55.794162 56.596757c0 31.135135-24.908108 56.430703-55.794162 56.569081a56.32 56.32 0 0 1-56.347676-56.015568v-1.134703z" fill="currentColor"></path></svg>`,
  '/repository': `<svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg"><path d="M870.4 57.6C780.8 19.2 652.8 0 512 0 371.2 0 243.2 19.2 153.6 57.6 51.2 102.4 0 153.6 0 211.2v595.2C0 864 51.2 921.6 153.6 960c89.6 44.8 217.6 64 358.4 64 140.8 0 268.8-19.2 358.4-57.6 96-38.4 153.6-96 153.6-153.6V211.2c0-57.6-51.2-108.8-153.6-153.6z m6.4 262.4c-83.2 32-198.4 44.8-300.8 44.8-108.8 0-217.6-12.8-300.8-44.8-96-25.6-140.8-64-140.8-108.8 0-38.4 51.2-76.8 140.8-108.8C294.4 76.8 403.2 64 512 64c102.4 0 217.6 19.2 300.8 44.8 89.6 32 140.8 70.4 140.8 108.8 0 38.4-44.8 76.8-140.8 102.4z m6.4 185.6C736 531.2 620.8 550.4 512 550.4c-108.8 0-217.6-19.2-307.2-44.8C115.2 473.6 64 435.2 64 396.8v-70.4c64 25.6 108.8 57.6 179.2 70.4 83.2 19.2 172.8 32 268.8 32s185.6-12.8 268.8-32C851.2 384 896 352 960 326.4v76.8c0 32-51.2 70.4-140.8 102.4z m0 204.8C736 736 620.8 755.2 512 755.2c-108.8 0-217.6-19.2-307.2-44.8C115.2 684.8 64 646.4 64 601.6v-96c64 32 108.8 57.6 179.2 76.8 83.2 19.2 172.8 32 268.8 32s185.6-12.8 268.8-32c70.4-19.2 115.2-44.8 179.2-76.8v96c0 44.8-51.2 83.2-140.8 108.8zM512 960c-108.8 0-217.6-19.2-307.2-44.8C115.2 889.6 64 851.2 64 812.8v-96c64 32 108.8 57.6 179.2 76.8 76.8 19.2 172.8 32 262.4 32 96 0 185.6-12.8 268.8-32 76.8-19.2 121.6-44.8 185.6-76.8v96c0 38.4-51.2 76.8-140.8 108.8C736 947.2 614.4 960 512 960z" fill="currentColor"></path></svg>`,
  '/brain': `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5a3 3 0 0 0-6 0 3 3 0 0 0-1 5.8A3 3 0 0 0 8 18a3 3 0 0 0 4-1"></path><path d="M12 5a3 3 0 0 1 6 0 3 3 0 0 1 1 5.8A3 3 0 0 1 16 18a3 3 0 0 1-4-1"></path><path d="M12 5v13"></path></g></svg>`,
  '/application': `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="1.8"><rect x="3" y="3" width="7.5" height="7.5" rx="1.6"></rect><rect x="13.5" y="3" width="7.5" height="7.5" rx="1.6"></rect><rect x="3" y="13.5" width="7.5" height="7.5" rx="1.6"></rect><path d="M17.25 13.5v7.5M13.5 17.25h7.5" stroke-linecap="round"></path></g></svg>`,
  '/statistics': `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"></path><path d="M7 14v4"></path><path d="M12 10v8"></path><path d="M17 6v12"></path></g></svg>`,
  '/search': `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"></circle><path d="m16.2 16.2 4.3 4.3"></path></g></svg>`,
  '/excalidraw': `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"></path><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L8 18l-4 1l1-4Z"></path></g></svg>`,
  '/setting': `<svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><g fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3.2"></circle><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.55-1.1 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9a1.7 1.7 0 0 0 1.55 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"></path></g></svg>`,
}

const items = computed(() =>
  mainNavigationItems
    .filter((item) => NAV_ICONS[item.path])
    .map((item) => ({ ...item, svg: NAV_ICONS[item.path] })),
)

const settingItem = computed(() => items.value.find((i) => i.path === '/setting') || null)
const topItems = computed(() => items.value.filter((i) => i.path !== '/setting'))

const navigateTo = (path) => {
  window.location.hash = path
}
</script>

<style scoped>
.sider-icon-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: var(--sider-width, 44px);
  min-width: var(--sider-width, 44px);
  height: 100%;
  min-height: 0;
  box-sizing: border-box;
  border-right: 1px solid var(--guaxi-border);
  background: var(--guaxi-panel-bg);
}

.sider-icon-top {
  display: flex;
  flex-direction: column;
  gap: 18px;
  align-items: center;
  justify-content: flex-start;
  width: 100%;
  padding: 14px 0;
  flex: 1 1 auto;
  min-height: 0;
}

.sider-icon-bottom {
  width: 100%;
  padding: 0 0 14px;
  display: flex;
  justify-content: center;
}

.nav-icon {
  cursor: pointer;
  color: var(--guaxi-text-secondary);
  padding: 8px;
  border-radius: 12px;
  transition: background-color 0.2s ease, color 0.2s ease, transform 0.12s ease, box-shadow 0.2s ease;
}

.nav-glyph {
  display: flex;
  width: 24px;
  height: 24px;
}

.nav-glyph :deep(svg) {
  width: 24px;
  height: 24px;
  fill: currentColor;
  stroke: currentColor;
}

/* 线性图标（stroke 为主）不能被 fill 填成实心块，这里按原样式还原。 */
.nav-glyph :deep(svg g[fill='none']) {
  fill: none;
}

.nav-icon:hover,
.nav-icon.active {
  color: var(--n-color-primary);
  background: rgba(45, 212, 191, 0.12);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.08);
  transform: translateY(-1px);
}

:global(:root[data-guaxi-theme='dark']) .nav-icon {
  color: rgba(255, 255, 255, 0.92);
}

:global(:root[data-guaxi-theme='dark']) .nav-icon:hover,
:global(:root[data-guaxi-theme='dark']) .nav-icon.active {
  background: rgba(97, 175, 239, 0.16);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.32);
}

@media (prefers-reduced-motion: reduce) {
  .nav-icon,
  .nav-icon:hover,
  .nav-icon.active {
    transition: none;
    transform: none;
  }
}
</style>
