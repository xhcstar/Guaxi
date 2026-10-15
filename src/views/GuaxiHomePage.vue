<template>
  <n-message-provider placement="bottom">
    <div class="app-layout">
      <div class="main-header">
        <GuaxiHeader />
      </div>
      <div class="main-content">
        <GuaxiContent />
      </div>
    </div>
  </n-message-provider>
</template>



<script>
export default { name: 'GuaxiHomePage' }
</script>

<script setup>
import { nextTick, onBeforeUnmount, onMounted } from 'vue';
import GuaxiHeader from "../components/HomeViewComponent/GuaxiHeader.vue";
import GuaxiContent from "../components/HomeViewComponent/GuaxiContent.vue";
import { NMessageProvider } from 'naive-ui';
import { createLogger } from '../utils/logger';
import { useOnboardingTour } from '../composables/useOnboardingTour';

const logger = createLogger('GuaxiHomePage');
const { start: startOnboardingTour } = useOnboardingTour();
let layoutCheckFrame = 0;

/**
 * 检查主窗口是否意外出现文档级滚动。
 *
 * 设计思路：
 * - 主界面的滚动应只发生在文件树、编辑器等内部容器；
 * - 正常布局仅在首次挂载时记录 debug，检测到根文档溢出时才记录 warn；
 * - 使用 requestAnimationFrame 等待浏览器完成本轮布局，避免读取到过渡状态。
 */
const checkDocumentOverflow = (reason, logWhenHealthy = false) => {
  cancelAnimationFrame(layoutCheckFrame);
  layoutCheckFrame = requestAnimationFrame(() => {
    const root = document.documentElement;
    const body = document.body;
    const metrics = {
      reason,
      viewportHeight: window.innerHeight,
      rootClientHeight: root.clientHeight,
      rootScrollHeight: root.scrollHeight,
      bodyClientHeight: body.clientHeight,
      bodyScrollHeight: body.scrollHeight,
    };
    const hasDocumentOverflow =
      root.scrollHeight > root.clientHeight ||
      body.scrollHeight > body.clientHeight;

    if (hasDocumentOverflow) {
      logger.warn('[HomeLayout] detected unexpected document-level overflow', metrics);
    } else if (logWhenHealthy) {
      logger.debug('[HomeLayout] viewport height chain is constrained', metrics);
    }
  });
};

const handleWindowResize = () => checkDocumentOverflow('window-resize');

onMounted(async () => {
  window.addEventListener('resize', handleWindowResize);
  await nextTick();
  checkDocumentOverflow('mounted', true);
  // 首次启动给一段新手引导；看过了就静默跳过。
  // 必须放在 nextTick 之后：引导要定位的控件这时才在 DOM 里。
  startOnboardingTour();
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleWindowResize);
  cancelAnimationFrame(layoutCheckFrame);
});
</script>


<style scoped>
/* 确保根布局占据整个窗口 */
.app-layout {
  display: flex;
  flex-direction: column;
  /*
   * 高度继承自全局 html/body/#app 高度链。
   * 不直接使用 100vh，避免 body 外边距或 WebView 视口换算造成页面级溢出。
   */
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  margin: 0;
  /* 确保没有外部间距 */
  padding: 0;
  /* 确保没有外部内边距 */
  box-sizing: border-box;
  /* 包括 padding 和 border 在内计算高度 */
}

/* 头部样式 */
.main-header {
  flex: 0 0 auto;
  /* 固定高度的 header */
  height: 35px;
  /* 设置你需要的高度 */
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
}

/* 内容区域样式 */
.main-content {
  flex: 1 1 auto;
  /* 填充剩余空间 */
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  /* overflow-y: auto; */
  overflow: hidden;
  min-height: 0;
  /* 如果内容超出，允许垂直滚动 */
  margin: 0;
  padding: 0;
}
</style>



<!-- <template>
  <div class="app-layout">
    <GuaxiHeader 
      :is-left-sider-collapsed="isLeftSiderCollapsed" 
      :is-right-sider-collapsed="isRightSiderCollapsed"
      @update-left-sider="handleLeftSiderUpdate"
      @update-right-sider="handleRightSiderUpdate"
    />
    <GuaxiContent 
      :is-left-sider-collapsed="isLeftSiderCollapsed"
      :is-right-sider-collapsed="isRightSiderCollapsed"
    />
  </div>
</template>


<script >
import { defineComponent, ref} from "vue";
import GuaxiHeader from "./GuaxiHeader.vue";
import GuaxiContent from "./GuaxiContent.vue";


export default defineComponent({
  components: {
    GuaxiHeader,
    GuaxiContent,
  },

  setup() {
    // const isLeftSiderCollapsed = ref(false);
    // const isRightSiderCollapsed = ref(true);

    // // 更新左侧边栏的状态
    // const handleLeftSiderUpdate = (collapsed) => {
    //   isLeftSiderCollapsed.value = collapsed;
    //   console.log("Main context isLeftSiderCollapsed", isLeftSiderCollapsed.value);
    // };

    // // 更新右侧边栏的状态
    // const handleRightSiderUpdate = (collapsed) => {
    //   isRightSiderCollapsed.value = collapsed;
    //   console.log("Main context isRightSiderCollapsed", isRightSiderCollapsed.value);
    // };

    return {
      // isLeftSiderCollapsed,
      // isRightSiderCollapsed,
      // handleLeftSiderUpdate,
      // handleRightSiderUpdate
    };
  }
})
</script> -->
