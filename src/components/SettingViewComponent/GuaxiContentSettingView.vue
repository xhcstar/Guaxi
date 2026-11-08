<template>
  <div class="guaxi-content">
    <n-layout has-sider class="setting-layout">
      <n-layout-sider v-model:collapsed="isLeftSiderCollapsed" :width="leftSiderbarWidth" collapse-mode="width"
        :collapsed-width="0" bordered content-style="padding: 0px;">
        <GuaxiSidebarSettingView />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </n-layout-sider>

      <n-layout has-sider sider-placement="right" class="setting-layout">
        <n-layout-content class="setting-main">
          <GuaxiSettingContent />
        </n-layout-content>

        <n-layout-sider v-model:collapsed="isRightSiderCollapsed" :width="rightSiderbarWidth" collapse-mode="width"
          :collapsed-width="0" bordered content-style="padding: 0px;">
          <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar"></div>
          <GuaxiRightContentSettingView />
        </n-layout-sider>
      </n-layout>
    </n-layout>
  </div>
</template>


<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import { useSettingsStore } from '../../stores/settingsStore'
import GuaxiSidebarSettingView from "./GuaxiSidebarSettingView.vue"
import GuaxiSettingContent from "./GuaxiSettingContent.vue"
import GuaxiRightContentSettingView from "./GuaxiRightContentSettingView.vue"

const logger = createLogger('GuaxiContentSettingView')

const leftSiderbarWidth = ref(350); // 默认宽度
const leftSidebarresizing = ref(false);
const leftSidebarStartX = ref(0);
const leftSidebarStartWidth = ref(0);
const rightSiderbarWidth = ref(650); // 默认宽度
const rightSidebarresizing = ref(false);
const rightSidebarStartX = ref(0);
const rightSidebarStartWidth = ref(0);
const viewKey = 'setting'
const siderStore = useSiderStore();
const isLeftSiderCollapsed = computed({
  get: () => siderStore.isLeftSiderCollapsed[viewKey] ?? false,
  set: (val) => siderStore.setLeftSiderCollapsed(viewKey, val)
})
const isRightSiderCollapsed = computed({
  get: () => siderStore.isRightSiderCollapsed[viewKey] ?? true,
  set: (val) => siderStore.setRightSiderCollapsed(viewKey, val)
})
const settingsStore = useSettingsStore()

// 手册把右侧栏当作自己的章节目录：进入时自动展开并收窄到适合阅读的宽度，
// 离开时恢复原来的宽度与折叠状态。窗口不够宽时保持收起，改用手册内的吸顶章节条。
const MANUAL_RIGHT_SIDER_WIDTH = 320
const MANUAL_RIGHT_SIDER_MIN_WINDOW = 1360
let rightWidthBeforeManual = 0
let rightCollapsedBeforeManual = null
let manualAutoOpenedRight = false

watch(
  () => settingsStore.selectedMenu,
  (menu, previous) => {
    if (menu === 'manual') {
      rightWidthBeforeManual = rightSiderbarWidth.value
      rightCollapsedBeforeManual = isRightSiderCollapsed.value
      rightSiderbarWidth.value = MANUAL_RIGHT_SIDER_WIDTH
      if (window.innerWidth >= MANUAL_RIGHT_SIDER_MIN_WINDOW && isRightSiderCollapsed.value) {
        siderStore.setRightSiderCollapsed(viewKey, false)
        manualAutoOpenedRight = true
      }
      return
    }

    if (previous === 'manual') {
      rightSiderbarWidth.value = rightWidthBeforeManual || 650
      if (rightCollapsedBeforeManual !== null) {
        siderStore.setRightSiderCollapsed(viewKey, rightCollapsedBeforeManual)
      }
      manualAutoOpenedRight = false
      rightCollapsedBeforeManual = null
    }
  },
  { immediate: true },
)

const onWindowResizeForManual = () => {
  if (settingsStore.selectedMenu !== 'manual') return
  if (window.innerWidth < MANUAL_RIGHT_SIDER_MIN_WINDOW && manualAutoOpenedRight) {
    siderStore.setRightSiderCollapsed(viewKey, true)
    manualAutoOpenedRight = false
  }
}

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

onMounted(() => {
  window.addEventListener('resize', onWindowResizeForManual)
})

// Cleanup listeners when the component is unmounted
onBeforeUnmount(() => {
  window.removeEventListener('resize', onWindowResizeForManual)
  document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
  document.removeEventListener("mouseup", onMouseUpLeftSidebar);
  document.removeEventListener("mousemove", onMouseMoveRightSidebar);
  document.removeEventListener("mouseup", onMouseUpRightSidebar);
});

</script>


<style scoped>
.n-layout-sider {
  background: var(--guaxi-panel-bg);
  position: relative;
}

.n-layout-content {
  background: var(--guaxi-panel-bg);
}

.guaxi-content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  min-height: 0;
  height: 100%;
  overflow: hidden;
  margin: 0;
  padding: 0;
}

.guaxi-content :deep(.n-layout) {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.guaxi-content :deep(.n-layout-content) {
  min-height: 0;
}

.setting-layout {
  flex: 1 1 auto;
  min-height: 0;
  height: 100%;
}

.setting-main {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-border);
}

.sider-resize-handle-right {
  position: absolute;
  top: 0;
  left: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: var(--guaxi-border);
}
</style>


<!-- <script>
import { defineComponent, ref, computed, onMounted, onBeforeUnmount, watch } from "vue"
import { useSiderStore } from '../stores/useSiderStore'
import GuaxicoSidebar from "./GuaxicoSidebar.vue"
import GuaxiSidbarTree from "./GuaxiSidbarTree.vue"
import GuaxiRightContentSettingView from "./GuaxiRightContentSettingView.vue"

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