<template>
  <div class="guaxi-content">
    <div class="brain-layout">
      <aside class="brain-sider brain-sider-left"
        :style="{ width: isLeftSiderCollapsed ? '0px' : leftSiderbarWidth + 'px' }">
        <GuaxiSidebarBrainView />
        <div v-show="!isLeftSiderCollapsed" class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </aside>

      <main class="brain-center">
        <GuaxiCenterBrainView />
      </main>

      <aside class="brain-sider brain-sider-right"
        :style="{ width: isRightSiderCollapsed ? '0px' : rightSiderbarWidth + 'px' }">
        <div v-show="!isRightSiderCollapsed" class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar">
        </div>
        <GuaxiRightContentBrainView />
      </aside>
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
const leftSiderbarWidth = ref(350); // 默认宽度
const leftSidebarresizing = ref(false);
const leftSidebarStartX = ref(0);
const leftSidebarStartWidth = ref(0);
const rightSiderbarWidth = ref(350); // 默认宽度
const rightSidebarresizing = ref(false);
const rightSidebarStartX = ref(0);
const rightSidebarStartWidth = ref(0);
const viewKey = 'brain'
const siderStore = useSiderStore();
const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false,
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[viewKey] ?? true,
  set: (val) => siderStore.setRightSiderCollapsed(viewKey, val)
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

const onMouseDownRightSidebar = (event) => {
  logger.info("onMouseDownRightSidebar");
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

.brain-sider {
  height: 100%;
  min-height: 0;
  overflow: hidden;
  position: relative;
  background: var(--guaxi-panel-bg);
}

.sider-resize-handle {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
}

:deep(.n-layout-content__content) {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
}


.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-panel-bg);
}

.sider-resize-handle-right {
  position: absolute;
  top: 0;
  left: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-panel-bg);
  z-index: 2;
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