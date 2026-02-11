<template>
  <div class="sider-icon-container">
    <div class="sider-icon-top">
      <n-icon size="24" class="nav-icon" :class="{ active: activePath === '/' }" @click="navigateTo('/')" title="首页">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
          <g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M5 12H3l9-9l9 9h-2"></path>
            <path d="M5 12v7a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-7"></path>
            <path d="M9 21v-6a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v6"></path>
          </g>
        </svg>
      </n-icon>

      <n-icon size="24" class="nav-icon" :class="{ active: activePath === '/excalidraw' }"
        @click="navigateTo('/excalidraw')" title="白板">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
          <g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 20h9" />
            <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L8 18l-4 1l1-4Z" />
          </g>
        </svg>
      </n-icon>
    </div>

    <div class="sider-icon-bottom">
      <n-icon size="24" class="nav-icon" :class="{ active: activePath === '/setting' }" @click="navigateTo('/setting')"
        title="设置">
        <svg t="1735455601102" class="icon" viewBox="0 0 1024 1024" version="1.1" xmlns="http://www.w3.org/2000/svg"
          p-id="5062" width="54" height="54">
          <path
            d="M513.08309 726.48892c-115.618713 0-209.986634-94.367921-209.986634-210.346817s94.007738-210.346817 209.986634-210.346816c115.618713 0 209.986634 94.367921 209.986634 210.346816s-94.007738 210.346817-209.986634 210.346817z m0-351.178333c-77.439325 0-140.831516 63.392191-140.831516 140.831516 0 77.799508 63.032008 140.831516 140.831516 140.831516 77.439325 0 140.831516-63.392191 140.831516-140.831516s-63.032008-140.831516-140.831516-140.831516z"
            fill="currentColor" p-id="5063"></path>
          <path
            d="M592.323329 76.358776V122.462188c0 33.857193 20.170243 64.47274 51.506155 77.439324 10.085121 4.322195 20.890609 6.483292 32.056279 6.483293 22.33134 0 43.221949-8.64439 59.069996-24.492438l29.534999-29.534998 112.016883 112.377066-29.534998 29.534999c-24.132255 24.132255-30.97573 59.790362-18.009145 91.126275 12.966585 31.335913 43.221949 51.506155 77.079141 51.506155h44.662681v158.840662h-44.662681c-33.857193 0-64.112557 20.170243-77.079141 51.506155-12.966585 31.335913-5.762926 67.354203 18.009145 91.126275l29.534998 29.534998-112.016883 112.377067-29.534999-29.534998c-15.848048-15.848048-36.738656-24.492438-59.069996-24.492438-11.16567 0-21.610974 2.161097-32.056279 6.483292-31.335913 12.966585-51.506155 43.221949-51.506155 77.439325v44.66268h-158.480478v-44.66268c0-33.857193-20.170243-64.47274-51.506156-77.439325-10.085121-4.322195-20.890609-6.483292-32.056279-6.483292-22.33134 0-43.221949 8.64439-59.069996 24.492438l-29.534998 29.534998-112.016884-112.377067 29.534998-29.534998c24.132255-24.132255 30.97573-59.790362 18.009146-91.126275-12.966585-31.335913-43.221949-51.506155-77.079142-51.506155H75.46086v-158.840662h44.66268c33.857193 0 64.112557-20.170243 77.079142-51.506155 12.966585-31.335913 5.762926-67.354203-18.009146-91.126275l-29.534998-29.534999 112.016884-112.377066 29.534998 29.534998c15.848048 15.848048 36.738656 24.492438 59.069996 24.492438 11.16567 0 21.610974-2.161097 32.056279-6.483293 31.335913-12.966585 51.506156-43.221949 51.506156-77.439324V76.358776h158.480478z"
            fill="currentColor" p-id="5064"></path>
        </svg>
      </n-icon>
    </div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { NIcon } from 'naive-ui'
import { createLogger } from '../../utils/logger'

const logger = createLogger('GuaxiExcalidrawSidebar')

const activePath = ref((window.location.hash || '#/').slice(1) || '/')

const syncActivePath = () => {
  activePath.value = (window.location.hash || '#/').slice(1) || '/'
}

onMounted(() => {
  window.addEventListener('hashchange', syncActivePath)
  syncActivePath()
  logger.info('[sidebar] mounted', { activePath: activePath.value })
})

onBeforeUnmount(() => {
  window.removeEventListener('hashchange', syncActivePath)
})

const navigateTo = (path) => {
  logger.info('[sidebar] navigate', { from: activePath.value, to: path })
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

.nav-icon svg,
.nav-icon svg * {
  fill: currentColor !important;
  stroke: currentColor !important;
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
</style>