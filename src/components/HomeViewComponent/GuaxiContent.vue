<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <n-layout-sider v-model:collapsed="isLeftSiderCollapsed" :width="leftSiderbarWidth" collapse-mode="width"
        :collapsed-width="0" bordered content-style="padding: 0px;">
        <GuaxiSidebar />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar" />
      </n-layout-sider>

      <GuaxiCenter />

      <n-layout has-sider sider-placement="right">
        <n-layout-sider v-model:collapsed="isRightSiderCollapsed" :width="rightSiderbarWidth" collapse-mode="width"
          :collapsed-width="0" bordered content-style="padding: 0px;">
          <GuaxiRightContent />
          <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar" />
        </n-layout-sider>
      </n-layout>
    </n-layout>
  </div>
</template>

<script setup>
import { ref, computed, onBeforeUnmount } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebar from "./GuaxiSidebar.vue"
import GuaxiCenter from "./GuaxiCenter.vue"
import GuaxiRightContent from "./GuaxiRightContent.vue"

const MAX_RIGHT_SIDEBAR_WIDTH = 700; // 推荐最大宽度
const MIN_RIGHT_SIDEBAR_WIDTH = 300; // 推荐最小宽度

const leftSiderbarWidth = ref(350);
const leftSidebarresizing = ref(false);
const leftSidebarStartX = ref(0);
const leftSidebarStartWidth = ref(0);

const rightSiderbarWidth = ref(512);
const rightSidebarresizing = ref(false);
const rightSidebarStartX = ref(0);
const rightSidebarStartWidth = ref(0);

const siderStore = useSiderStore();
const isLeftSiderCollapsed = computed(() => siderStore.isLeftSiderCollapsed);
const isRightSiderCollapsed = computed(() => siderStore.isRightSiderCollapsed);

const toggleLeftSider = () => {
  siderStore.toggleLeftSider();
};

const toggleRightSider = () => {
  siderStore.toggleRightSider();
};

// 使用requestAnimationFrame优化拖动性能
const optimizedMove = (handler) => {
  let ticking = false;
  return (event) => {
    if (!ticking) {
      window.requestAnimationFrame(() => {
        handler(event);
        ticking = false;
      });
      ticking = true;
    }
  };
};

// 左侧边栏拖动处理（优化版）
const onMouseDownLeftSidebar = (event) => {
  event.preventDefault();
  document.body.classList.add('resizing');

  leftSidebarresizing.value = true;
  leftSidebarStartX.value = event.clientX;
  leftSidebarStartWidth.value = leftSiderbarWidth.value;

  const optimizedMoveHandler = optimizedMove((e) => {
    const delta = e.clientX - leftSidebarStartX.value;
    leftSiderbarWidth.value = Math.max(250, leftSidebarStartWidth.value + delta);
  });

  const onMouseUp = () => {
    document.body.classList.remove('resizing');
    document.removeEventListener("mousemove", optimizedMoveHandler);
    document.removeEventListener("mouseup", onMouseUp);
    leftSidebarresizing.value = false;
  };

  document.addEventListener("mousemove", optimizedMoveHandler);
  document.addEventListener("mouseup", onMouseUp, { once: true });
};

// 右侧边栏拖动处理（优化版）
const onMouseDownRightSidebar = (event) => {
  event.preventDefault();
  document.body.classList.add('resizing');

  rightSidebarresizing.value = true;

  const optimizedMoveHandler = optimizedMove((e) => {
    const windowWidth = window.innerWidth;
    const newWidth = windowWidth - e.clientX;
    rightSiderbarWidth.value = Math.max(
      MIN_RIGHT_SIDEBAR_WIDTH,
      Math.min(MAX_RIGHT_SIDEBAR_WIDTH, newWidth)
    );
  });

  const onMouseUp = () => {
    document.body.classList.remove('resizing');
    document.removeEventListener("mousemove", optimizedMoveHandler);
    document.removeEventListener("mouseup", onMouseUp);
    rightSidebarresizing.value = false;
  };

  document.addEventListener("mousemove", optimizedMoveHandler);
  document.addEventListener("mouseup", onMouseUp, { once: true });
};

// 清理事件监听器
onBeforeUnmount(() => {
  document.body.classList.remove('resizing');
});
</script>

<style scoped>
.guaxi-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: #ffffff;
}

/* 公共侧边栏样式 */
.n-layout-sider {
  position: relative;
  background: rgba(239, 243, 239, 0.3);
  /* 移除过渡效果提升性能 */
  will-change: width;
}

/* 拖动手柄样式 */
.sider-resize-handle,
.sider-resize-handle-right {
  position: absolute;
  top: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
  z-index: 10;
  background-color: transparent;
  will-change: transform;
}

.sider-resize-handle {
  right: -3px;
}

.sider-resize-handle-right {
  left: -3px;
}

.sider-resize-handle:hover,
.sider-resize-handle-right:hover {
  background-color: rgba(0, 0, 0, 0.1);
}
</style>

<style>
/* 全局拖动样式 */
body.resizing {
  cursor: ew-resize !important;
  user-select: none !important;
}

/* 提升拖动性能 */
body.resizing * {
  pointer-events: none;
}

body.resizing .n-layout-sider,
body.resizing .sider-resize-handle,
body.resizing .sider-resize-handle-right {
  pointer-events: auto;
}
</style>