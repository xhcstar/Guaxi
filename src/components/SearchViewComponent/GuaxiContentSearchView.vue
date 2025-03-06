<template>
  <div class="guaxi-content">
    <n-layout has-sider>
      <n-layout-sider v-model:collapsed="isLeftSiderCollapsed" :width="leftSiderbarWidth" collapse-mode="width"
        :collapsed-width="0" bordered content-style="padding: 0px;">
        <GuaxiSidebarSearchView />
        <div class="sider-resize-handle" @mousedown="onMouseDownLeftSidebar"></div>
      </n-layout-sider>
      <GuaxiCenterSearchView />
      <n-layout has-sider sider-placement="right">
        <n-layout-sider v-model:collapsed="isRightSiderCollapsed" :width="rightSiderbarWidth" collapse-mode="width"
          :collapsed-width="0" bordered content-style="padding: 0px;">
          <div class="sider-resize-handle-right" @mousedown="onMouseDownRightSidebar">
            <GuaxiRightContentSearchView />
          </div>
        </n-layout-sider>
      </n-layout>
    </n-layout>
  </div>
</template>


<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue"
import { useSiderStore } from '../../stores/useSiderStore'
import GuaxiSidebarSearchView from "./GuaxiSidebarSearchView.vue"
import GuaxiCenterSearchView from "./GuaxiCenterSearchView.vue"
import GuaxiRightContentSearchView from "./GuaxiRightContentSearchView.vue"


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

</script>


<style scoped>
.n-layout-sider {
  background: rgba(239, 243, 239, 0.3);
}

.n-layout-content {
  background: rgba(239, 243, 239, 0.3);
}

.guaxi-content {
  flex: 1 1 auto; /* 填充剩余空间 */
  display: flex;
  flex-direction: column;
  background-color: #ffffff; /* 示例背景色 */
  overflow-y: auto; /* 如果内容超出，允许垂直滚动 */
  margin: 0;
  padding: 0;
}

.sider-resize-handle {
  position: absolute;
  top: 0;
  right: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: #f8fdfa;
}

.sider-resize-handle-right {
  position: relative;
  top: 0;
  left: 0;
  width: 3px;
  height: 100%;
  cursor: ew-resize;
  background-color: #f8fdfa;
}
</style>


<!-- <script>
import { defineComponent, ref, computed, onMounted, onBeforeUnmount, watch } from "vue"
import { useSiderStore } from '../stores/useSiderStore'
import GuaxicoSidebar from "./GuaxicoSidebar.vue"
import GuaxiSidbarTree from "./GuaxiSidbarTree.vue"
import GuaxiRightContentSearchView from "./GuaxiRightContentSearchView.vue"

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