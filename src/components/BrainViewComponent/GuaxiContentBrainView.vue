<template>
  <div class="guaxi-content">
    <!-- 宽度同时给到 aside 的 width 和一组 CSS 变量：拖动把手是 .brain-layout 的绝对定位子元素，
         靠变量贴在两栏的交界线上。把手不放在 aside 内部，是因为 aside 有 overflow: hidden，
         放里面就会被面板自己的竖滚动条压住，用户抓到的十次有九次是滚动条。 -->
    <div
      ref="layoutRef"
      class="brain-layout"
      :style="{ '--brain-left-w': leftWidth + 'px', '--brain-right-w': rightWidth + 'px' }"
    >
      <aside class="brain-sider brain-sider-left" :style="{ width: leftWidth + 'px' }">
        <GuaxiSidebarBrainView />
      </aside>

      <main class="brain-center">
        <GuaxiCenterBrainView />
      </main>

      <aside class="brain-sider brain-sider-right" :style="{ width: rightWidth + 'px' }">
        <GuaxiRightContentBrainView />
      </aside>

      <div
        v-show="!isLeftSiderCollapsed"
        class="sider-resize-handle left"
        @mousedown="onMouseDownLeftSidebar"
      ></div>
      <div
        v-show="!isRightSiderCollapsed"
        class="sider-resize-handle right"
        @mousedown="onMouseDownRightSidebar"
      ></div>
    </div>
  </div>
</template>


<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, onMounted, onBeforeUnmount } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebarBrainView from "./GuaxiSidebarBrainView.vue"
import GuaxiCenterBrainView from "./GuaxiCenterBrainView.vue"
import GuaxiRightContentBrainView from "./GuaxiRightContentBrainView.vue"

const logger = createLogger('GuaxiContentBrainView')
const viewKey = 'brain'
const siderStore = useSiderStore()

const DEFAULT_SIDER_WIDTH = 350
const MIN_SIDER_WIDTH = 250
/** 中间至少留这么宽。侧栏再怎么拖也不能把正文挤没，否则只能靠折叠按钮救回来。 */
const MIN_CENTER_WIDTH = 320

const layoutRef = ref(null)

const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false,
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[viewKey] ?? true,
  set: (val) => siderStore.setRightSiderCollapsed(viewKey, val)
})
const leftSiderbarWidth = computed({
  get: () => siderStore.leftSiderWidth[viewKey] ?? DEFAULT_SIDER_WIDTH,
  set: (val) => siderStore.setLeftSiderWidth(viewKey, val)
})
const rightSiderbarWidth = computed({
  get: () => siderStore.rightSiderWidth[viewKey] ?? DEFAULT_SIDER_WIDTH,
  set: (val) => siderStore.setRightSiderWidth(viewKey, val)
})

const leftWidth = computed(() => (isLeftSiderCollapsed.value ? 0 : leftSiderbarWidth.value))
const rightWidth = computed(() => (isRightSiderCollapsed.value ? 0 : rightSiderbarWidth.value))

/** 拖动时每帧只算一次。mousemove 原速驱动 Vue 重渲染，认知面板和 3D 图谱都吃不消，手感就是拖不动。 */
const rafThrottle = (handler) => {
  let ticking = false
  return (event) => {
    if (ticking) return
    ticking = true
    window.requestAnimationFrame(() => {
      handler(event)
      ticking = false
    })
  }
}

/**
 * 两侧共用一套拖动逻辑。
 * - 容器宽度在按下时量一次，拖动过程中不再读 DOM，省掉每帧的强制重排。
 * - body.resizing 期间全局屏蔽 pointer-events：否则鼠标扫过 3D 画布、卡片和悬浮提示时，
 *   这些元素会抢走事件并触发大量 hover 重绘，拖起来就是一顿一顿的。
 */
const beginResize = (event, side) => {
  event.preventDefault()
  const container = layoutRef.value?.clientWidth || window.innerWidth
  const startX = event.clientX
  const isLeft = side === 'left'
  const otherWidth = isLeft ? rightWidth.value : leftWidth.value
  const maxWidth = Math.max(MIN_SIDER_WIDTH, container - otherWidth - MIN_CENTER_WIDTH)
  const startWidth = isLeft ? leftSiderbarWidth.value : rightSiderbarWidth.value

  document.body.classList.add('resizing')
  logger.info('开始拖动大脑页侧栏', { side, startWidth, maxWidth, container })

  const onMove = rafThrottle((e) => {
    const delta = isLeft ? e.clientX - startX : startX - e.clientX
    const next = Math.min(maxWidth, Math.max(MIN_SIDER_WIDTH, Math.round(startWidth + delta)))
    if (isLeft) leftSiderbarWidth.value = next
    else rightSiderbarWidth.value = next
  })

  const onUp = () => {
    document.body.classList.remove('resizing')
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    stopResize = null
    logger.info('结束拖动大脑页侧栏', {
      side,
      width: isLeft ? leftSiderbarWidth.value : rightSiderbarWidth.value,
    })
  }

  stopResize = onUp
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp, { once: true })
}

/** 卸载时兜底：拖动中被路由切走，body 上的 resizing 类和监听不能留着。 */
let stopResize = null

const onMouseDownLeftSidebar = (event) => beginResize(event, 'left')
const onMouseDownRightSidebar = (event) => beginResize(event, 'right')

/** 窗口变窄后，之前拖出来的宽度可能已经把正文挤没了，这里收回到合法范围。 */
const clampToWindow = () => {
  const container = layoutRef.value?.clientWidth || window.innerWidth
  const left = leftWidth.value
  const right = rightWidth.value
  const overflow = left + right + MIN_CENTER_WIDTH - container
  if (overflow <= 0) return
  if (!isLeftSiderCollapsed.value) {
    leftSiderbarWidth.value = Math.max(MIN_SIDER_WIDTH, left - overflow)
  }
  const stillOver = leftWidth.value + right + MIN_CENTER_WIDTH - container
  if (stillOver > 0 && !isRightSiderCollapsed.value) {
    rightSiderbarWidth.value = Math.max(MIN_SIDER_WIDTH, right - stillOver)
  }
}

onMounted(() => {
  window.addEventListener('resize', clampToWindow)
  clampToWindow()
  logger.info('brain layout mounted', {
    leftSidebarWidth: leftSiderbarWidth.value,
    rightSidebarWidth: rightSiderbarWidth.value,
    isRightSiderCollapsed: isRightSiderCollapsed.value
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', clampToWindow)
  if (stopResize) stopResize()
  document.body.classList.remove('resizing')
})

</script>


<style scoped>
.guaxi-content {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
  margin: 0;
  padding: 0;
  height: 100%;
}

.brain-layout {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  display: flex;
  overflow: hidden;
}

.brain-center {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--guaxi-content-bg);
}

/* flex: 0 0 auto 是关键：宽度由内联 style 决定，侧栏不能参与收缩。
   少了这一句，中间那块很宽的内容（收件箱的长标题、成排的来源标签）一超出视口，
   flex 默认的 shrink 就会把侧栏一路压到 min-width: 0，看起来就是"边栏被挤没了"。 */
.brain-sider {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  align-self: stretch;
  min-width: 0;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  position: relative;
  background: var(--guaxi-panel-bg);
}

:deep(.n-layout-content__content) {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* 把手骑在交界线上（各占两侧 3px），并且盖在所有内容之上。
   原来它是 aside 里 3px 宽的绝对定位元素，正好被面板的竖滚动条压住，所以"很不灵敏"。 */
.sider-resize-handle {
  position: absolute;
  top: 0;
  width: 6px;
  height: 100%;
  cursor: ew-resize;
  z-index: 20;
  background-color: transparent;
}

.sider-resize-handle.left {
  left: calc(var(--brain-left-w, 350px) - 3px);
}

.sider-resize-handle.right {
  right: calc(var(--brain-right-w, 350px) - 3px);
}

.sider-resize-handle:hover {
  background-color: rgba(127, 127, 127, 0.28);
}
</style>

<!-- 非 scoped：body 上的类要全局生效。首页 GuaxiContent.vue 里有同样一份，
     两个页面的拖动手感因此保持一致。 -->
<style>
body.resizing {
  cursor: ew-resize !important;
  user-select: none !important;
}

body.resizing * {
  pointer-events: none;
}

body.resizing .sider-resize-handle {
  pointer-events: auto;
}
</style>


<!-- <script>
import { defineComponent, ref, computed, onMounted, onBeforeUnmount, watch } from "vue"
import { useSiderStore } from '../stores/useSiderStore'
import GuaxicoSidebar from "./GuaxicoSidebar.vue"
import GuaxiSidbarTree from "./GuaxiSidbarTree.vue"
import GuaxiRightContentBrainView from "./GuaxiRightContentBrainView.vue"

export default defineComponent({
  components: {

  },

  props: {
    isLeftSiderCollapsed: {
      type: Boolean,
      required: true,
    },
    isRightSiderCollapsed: {
      type: Boolean,
      required: true,
    },
  },

  setup() {
    const leftSiderbarWidth = ref(350); // 默认宽度
    const leftSidebarresizing = ref(false);
    const leftSidebarStartX = ref(0);
    const leftSidebarStartWidth = ref(0);
    const rightSiderbarWidth = ref(650); // 默认宽度
    const rightSidebarresizing = ref(false);
    const rightSidebarStartX = ref(0);
    const rightSidebarStartWidth = ref(0);

    const siderStore = useSiderStore();
    const isLeftSiderCollapsed = computed(() => siderStore.isLeftSiderCollapsed);
    const isRightSiderCollapsed = computed(() => siderStore.isRightSiderCollapsed);

    
  // 监听props的变化
  watch(
    () => props.isLeftSiderCollapsed,
    (newVal) => {
      console.log('左侧边栏折叠状态变化:', newVal);
    }
  );
  
  watch(
    () => props.isRightSiderCollapsed,
    (newVal) => {
      console.log('右侧边栏折叠状态变化:', newVal);
    }
  );
  
    const toggleLeftSider = () => {
      siderStore.toggleLeftSider()
    };

    const toggleRightSider = () => {
      siderStore.toggleRightSider()
    };

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

    const onMouseDownRightSidebar = (event) => {
      console.log("onMouseDownRightSidebar");
      rightSidebarresizing.value = true;
      rightSidebarStartX.value = event.clientX;
      rightSidebarStartWidth.value = rightSiderbarWidth.value;

      // Attach mousemove and mouseup listeners
      document.addEventListener("mousemove", onMouseMoveRightSidebar);
      document.addEventListener("mouseup", onMouseUpRightSidebar);
    };

    const onMouseMoveRightSidebar = (event) => {
      if (!rightSidebarresizing.value) return;
      const delta = rightSidebarStartX.value - event.clientX;
      rightSiderbarWidth.value = Math.max(250, rightSidebarStartWidth.value + delta); // 设置最小宽度为200px
    };

    const onMouseUpRightSidebar = () => {
      rightSidebarresizing.value = false;
      // Remove mousemove and mouseup listeners
      document.removeEventListener("mousemove", onMouseMoveRightSidebar);
      document.removeEventListener("mouseup", onMouseUpRightSidebar);
    };

    // Cleanup listeners when the component is unmounted
    onBeforeUnmount(() => {
      document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
      document.removeEventListener("mouseup", onMouseUpLeftSidebar);
      document.removeEventListener("mousemove", onMouseMoveRightSidebar);
      document.removeEventListener("mouseup", onMouseUpRightSidebar);
    });

    return {
      isLeftSiderCollapsed,
      isRightSiderCollapsed,
      leftSiderbarWidth,
      rightSiderbarWidth,
      toggleLeftSider,
      toggleRightSider,
      onMouseDownLeftSidebar,
      onMouseDownRightSidebar,
    };
  },
});
</script> -->
