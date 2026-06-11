<template>
  <div class="guaxi-content">
    <div class="layout-row">
      <!-- 左侧边栏（原生实现，不依赖 naive-ui，保证高度链可控） -->
      <div
        class="app-sider left"
        :class="{ collapsed: isLeftSiderCollapsed }"
        :style="{ width: isLeftSiderCollapsed ? '0px' : leftSiderbarWidth + 'px' }"
      >
        <div class="app-sider-content">
          <GuaxiSidebar />
        </div>
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar" />
      </div>

      <!-- 中间内容 -->
      <GuaxiCenter />

      <!-- 右侧边栏（原生实现） -->
      <div
        class="app-sider right"
        :class="{ collapsed: isRightSiderCollapsed }"
        :style="{ width: isRightSiderCollapsed ? '0px' : rightSiderbarWidth + 'px' }"
      >
        <div class="app-sider-content">
          <GuaxiRightContent />
        </div>
        <div class="sidebar-dragger right" @mousedown="onMouseDownRightSidebar" />
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebar from "./GuaxiSidebar.vue"
import GuaxiCenter from "./GuaxiCenter.vue"
import GuaxiRightContent from "./GuaxiRightContent.vue"

// 常量定义
const MAX_RIGHT_SIDEBAR_WIDTH = 490
const MIN_RIGHT_SIDEBAR_WIDTH = 350

// 响应式状态
const leftSiderbarWidth = ref(350)
const rightSiderbarWidth = ref(490)
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
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  width: 100%;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
}

/* 横向布局：左栏 + 中间内容 + 右栏 */
.layout-row {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  align-items: stretch;
  width: 100%;
  overflow: hidden;
  background-color: var(--guaxi-content-bg);
}

/* 侧边栏：固定宽度、整列高度，内部用纵向 flex 把高度交给内容 */
.app-sider {
  position: relative;
  flex: 0 0 auto;
  align-self: stretch;
  min-height: 0;
  height: 100%;
  box-sizing: border-box;
  background: var(--guaxi-content-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: width 0.2s ease;
  will-change: width;
}

.app-sider.left {
  border-right: 1px solid var(--guaxi-border);
}

.app-sider.right {
  border-left: 1px solid var(--guaxi-border);
  max-width: 100vw;
}

.app-sider.collapsed {
  border-width: 0;
}

/* 内容包裹层：用绝对定位铺满侧边栏，按父元素的"实际渲染尺寸"撑开，
   不依赖百分比高度在 flex 链上的解析，从根本上杜绝塌成内容高度的问题。 */
.app-sider-content {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* 拖动时禁用过渡，避免跟手延迟 */
body.resizing .app-sider {
  transition: none !important;
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

/* 折叠时隐藏拖动条 */
.app-sider.collapsed .sider-resize-handle,
.app-sider.collapsed .sidebar-dragger.right {
  display: none;
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

body.resizing .sider-resize-handle,
body.resizing .sidebar-dragger.right {
  pointer-events: auto;
}
</style>