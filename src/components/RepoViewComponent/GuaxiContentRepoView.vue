<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <n-layout-sider 
        v-model:collapsed="isLeftSiderCollapsed" 
        :width="leftSiderbarWidth" 
        collapse-mode="width"
        :collapsed-width="0" 
        bordered 
        content-style="padding: 0px;"
      >
        <GuaxiSidebarRepoView />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </n-layout-sider>

      <n-layout-content class="main-content-area">
        <GuaxiCenterRepoView />
      </n-layout-content>
    </n-layout>
  </div>
</template>


<script setup>
import { ref, computed, onBeforeUnmount } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebarRepoView from "./GuaxiSidebarRepoView.vue"
import GuaxiCenterRepoView from "./GuaxiCenterRepoView.vue"


const leftSiderbarWidth = ref(350); // 默认宽度
const leftSidebarresizing = ref(false);
const leftSidebarStartX = ref(0);
const leftSidebarStartWidth = ref(0);
const viewKey = 'repo'
const siderStore = useSiderStore();
const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false,
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})

const onMouseDownLeftSidebar = (event) => {
  leftSidebarresizing.value = true;
  leftSidebarStartX.value = event.clientX;
  leftSidebarStartWidth.value = leftSiderbarWidth.value;
  // Attach mousemove and mouseup listeners
  document.addEventListener("mousemove", onMouseMoveLeftSidebar);
  document.addEventListener("mouseup", onMouseUpLeftSidebar);
};

const onMouseMoveLeftSidebar = (event) => {
  if (!leftSidebarresizing.value) return;
  const delta = event.clientX - leftSidebarStartX.value;
  leftSiderbarWidth.value = Math.max(250, leftSidebarStartWidth.value + delta); // 设置最小宽度为200px
};

const onMouseUpLeftSidebar = () => {
  leftSidebarresizing.value = false;
  // Remove mousemove and mouseup listeners
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
  document.removeEventListener("mouseup", onMouseUpLeftSidebar);
};

// Cleanup listeners when the component is unmounted
onBeforeUnmount(() => {
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
  document.removeEventListener("mouseup", onMouseUpLeftSidebar);
});

</script>


<style scoped>
.guaxi-content {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100%;
  overflow: hidden; /* 防止外层滚动 */
}

.n-layout {
  height: 100%;
  overflow: hidden;
}

.main-content-area {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden; /* 内容区域禁止滚动 */
}

.n-layout-sider {
  background: var(--guaxi-content-bg);
  height: 100%; /* 确保侧边栏高度填满 */
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-content-bg);
  z-index: 1;
}

</style>