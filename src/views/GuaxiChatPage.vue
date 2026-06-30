<template>
  <n-message-provider placement="bottom">
    <div class="app-layout">
      <div class="main-header">
        <GuaxiHeaderChatView />
      </div>
      <div class="main-content">
        <GuaxiContentChatView />
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { onBeforeUnmount, onMounted } from 'vue'
import { NMessageProvider } from 'naive-ui'
import GuaxiHeaderChatView from "../components/ChatViewComponent/GuaxiHeaderChatView.vue";
import GuaxiContentChatView from "../components/ChatViewComponent/GuaxiContentChatView.vue";
import { createLogger } from '../utils/logger'

const logger = createLogger('GuaxiChatPage')

onMounted(() => {
  logger.info('Chat页面已挂载，Rehoboam风格背景由中央聊天区承载')
})

onBeforeUnmount(() => {
  logger.info('Chat页面已卸载')
})
</script>

<style scoped>
/* 根布局占满窗口，避免 Chat 页面出现额外滚动条。 */
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  margin: 0;
  padding: 0;
  box-sizing: border-box;
  background: var(--guaxi-content-bg);
  overflow: hidden;
}

/* 顶部工具栏保持固定高度，聊天主体负责吸收剩余空间。 */
.main-header {
  flex: 0 0 auto;
  height: 35px;
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  position: relative;
  z-index: 3;
}

/* 内容区只控制布局，不侵入左右侧栏和中心聊天组件的内部状态。 */
.main-content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
  margin: 0;
  padding: 0;
}
</style>
