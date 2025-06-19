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
        <GuaxiSidebar />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar" />
      </n-layout-sider>

      <GuaxiCenter />

      <n-layout has-sider sider-placement="right">
        <n-layout-sider
          v-model:collapsed="isRightSiderCollapsed"
          :width="rightSiderbarWidth"
          :style="{ maxWidth: windowWidth + 'px' }"
          collapse-mode="width"
          :collapsed-width="0"
          bordered
          content-style="padding: 0px;"
        >
          <GuaxiRightContent />
          <div
            class="sidebar-dragger right"
            @mousedown="onMouseDownRightSidebar"
            style="cursor: ew-resize; width: 6px; position: absolute; left: 0; top: 0; bottom: 0; z-index: 10;"
          ></div>
        </n-layout-sider>
      </n-layout>
    </n-layout>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebar from "./GuaxiSidebar.vue"
import GuaxiCenter from "./GuaxiCenter.vue"
import GuaxiRightContent from "./GuaxiRightContent.vue"

const MAX_RIGHT_SIDEBAR_WIDTH = 1000;
const MIN_RIGHT_SIDEBAR_WIDTH = 350;

const leftSiderbarWidth = ref(350);
const leftSidebarresizing = ref(false);
const leftSidebarStartX = ref(0);
const leftSidebarStartWidth = ref(0);

const rightSiderbarWidth = ref(512);
const rightSidebarresizing = ref(false);

const siderStore = useSiderStore();
const isLeftSiderCollapsed = computed(() => siderStore.isLeftSiderCollapsed);
const isRightSiderCollapsed = computed(() => siderStore.isRightSiderCollapsed);

const windowWidth = ref(window.innerWidth);

const updateWindowWidth = () => {
  windowWidth.value = window.innerWidth;
  // 保证右侧边栏宽度不会超出窗口
  if (rightSiderbarWidth.value > windowWidth.value) {
    rightSiderbarWidth.value = windowWidth.value;
  }
};

onMounted(() => {
  window.addEventListener('resize', updateWindowWidth);
});
onBeforeUnmount(() => {
  window.removeEventListener('resize', updateWindowWidth);
  document.body.classList.remove('resizing');
});

// 左侧边栏拖动
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

// 右侧边栏拖动
const onMouseDownRightSidebar = (event) => {
  event.preventDefault();
  document.body.classList.add('resizing');
  rightSidebarresizing.value = true;

  const sider = document.querySelector('.n-layout-sider[sider-placement="right"]');
  if (sider) {
    sider.style.transition = 'none';
    sider.style.userSelect = 'none';
  }

  const onMouseMove = (e) => {
    const winWidth = window.innerWidth;
    let newWidth = winWidth - e.clientX;
    // 限制最大宽度为窗口宽度
    newWidth = Math.max(MIN_RIGHT_SIDEBAR_WIDTH, Math.min(MAX_RIGHT_SIDEBAR_WIDTH, newWidth, winWidth));
    rightSiderbarWidth.value = newWidth;
  };

  const onMouseUp = () => {
    document.body.classList.remove('resizing');
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', onMouseUp);
    rightSidebarresizing.value = false;
    if (sider) {
      sider.style.transition = '';
      sider.style.userSelect = '';
    }
    // 拖动结束时再次保证宽度不会超出
    if (rightSiderbarWidth.value > window.innerWidth) {
      rightSiderbarWidth.value = window.innerWidth;
    }
  };

  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('mouseup', onMouseUp, { once: true });
};
</script>

<style scoped>
.guaxi-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100vh;
  background-color: #ffffff;
}

.n-layout-sider {
  position: relative;
  background: rgba(239, 243, 239, 0.3);
  will-change: width;
}

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
body.resizing {
  cursor: ew-resize !important;
  user-select: none !important;
}
body.resizing * {
  pointer-events: none;
}
.n-layout-sider[sider-placement="right"] {
  transition: width 0.2s;
  will-change: width;
  max-width: 100vw;
  box-sizing: border-box;
}
body.resizing .n-layout-sider,
body.resizing .search-result-item {
  transition: none !important;
  box-shadow: none !important;
}
body.resizing .sider-resize-handle,
body.resizing .sider-resize-handle-right {
  pointer-events: auto;
}
</style>