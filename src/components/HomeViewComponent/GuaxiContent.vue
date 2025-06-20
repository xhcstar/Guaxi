<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <!-- 左侧边栏 -->
      <n-layout-sider v-model:collapsed="isLeftSiderCollapsed" :width="leftSiderbarWidth" collapse-mode="width"
        :collapsed-width="0" bordered>
        <GuaxiSidebar />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar" />
      </n-layout-sider>

      <!-- 中间内容 -->
      <GuaxiCenter />

      <!-- 右侧边栏 -->
      <n-layout has-sider sider-placement="right">
        <n-layout-sider v-model:collapsed="isRightSiderCollapsed" :width="rightSiderbarWidth" collapse-mode="width"
          :collapsed-width="0" bordered content-style="padding: 0;" :native-scrollbar="false"
          style="height: 100%; position: relative;">
          <GuaxiRightContent />
          <div class="sidebar-dragger right" @mousedown="onMouseDownRightSidebar" />
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

// 常量定义
const MAX_RIGHT_SIDEBAR_WIDTH = 1000
const MIN_RIGHT_SIDEBAR_WIDTH = 350

// 响应式状态
const leftSiderbarWidth = ref(350)
const rightSiderbarWidth = ref(512)
const windowWidth = ref(window.innerWidth)

// Pinia 状态
const viewKey = 'home'
const siderStore = useSiderStore()
const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false, // 默认展开
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[viewKey] ?? true, // 默认隐藏
  set: (val) => siderStore.setRightSiderCollapsed(viewKey, val)
})

// 窗口大小变化处理
const updateWindowWidth = () => {
  windowWidth.value = window.innerWidth
  if (rightSiderbarWidth.value > windowWidth.value) {
    rightSiderbarWidth.value = windowWidth.value
  }
}

onMounted(() => window.addEventListener('resize', updateWindowWidth))
onBeforeUnmount(() => {
  window.removeEventListener('resize', updateWindowWidth)
  document.body.classList.remove('resizing')
})

// 优化移动事件处理
const optimizedMove = (handler) => {
  let ticking = false
  return (event) => {
    if (!ticking) {
      window.requestAnimationFrame(() => {
        handler(event)
        ticking = false
      })
      ticking = true
    }
  }
}

// 左侧边栏拖动
const onMouseDownLeftSidebar = (event) => {
  event.preventDefault()
  document.body.classList.add('resizing')
  const startX = event.clientX
  const startWidth = leftSiderbarWidth.value

  const moveHandler = optimizedMove((e) => {
    const delta = e.clientX - startX
    leftSiderbarWidth.value = Math.max(250, startWidth + delta)
  })

  const onMouseUp = () => {
    document.body.classList.remove('resizing')
    document.removeEventListener("mousemove", moveHandler)
    document.removeEventListener("mouseup", onMouseUp)
  }

  document.addEventListener("mousemove", moveHandler)
  document.addEventListener("mouseup", onMouseUp, { once: true })
}

// 右侧边栏拖动
const onMouseDownRightSidebar = (event) => {
  event.preventDefault()
  document.body.classList.add('resizing')
  const sider = event.currentTarget.parentElement
  sider.style.transition = 'none'
  sider.style.userSelect = 'none'

  const startX = event.clientX
  const startWidth = rightSiderbarWidth.value

  const onMouseMove = (e) => {
    const newWidth = startWidth + (startX - e.clientX)
    rightSiderbarWidth.value = Math.max(
      MIN_RIGHT_SIDEBAR_WIDTH,
      Math.min(MAX_RIGHT_SIDEBAR_WIDTH, newWidth)
    )
  }

  const onMouseUp = () => {
    document.removeEventListener('mousemove', onMouseMove)
    document.removeEventListener('mouseup', onMouseUp)
    document.body.classList.remove('resizing')
    sider.style.transition = ''
    sider.style.userSelect = ''
  }

  document.addEventListener('mousemove', onMouseMove)
  document.addEventListener('mouseup', onMouseUp, { once: true })
}
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

.sider-resize-handle {
  position: absolute;
  right: -3px;
  top: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
  z-index: 10;
  background-color: transparent;
  will-change: transform;
}

.sidebar-dragger.right {
  position: absolute;
  left: -3px;
  top: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
  z-index: 10;
  background-color: transparent;
}

.sider-resize-handle:hover,
.sidebar-dragger.right:hover {
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

body.resizing .n-layout-sider {
  transition: none !important;
}

body.resizing .sider-resize-handle,
body.resizing .sidebar-dragger.right {
  pointer-events: auto;
}
</style>