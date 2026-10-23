<template>
  <n-message-provider placement="bottom">
    <div class="agent-page">
      <!-- 全局导航列：工作台自己的三段布局占满右侧，导航列单独占 44px。
           用户从这里可以随时切走，不会困在 /chat 里。 -->
      <GuaxiNavRail />
      <AgentWorkspace />
    </div>
  </n-message-provider>
</template>

<script setup>
/**
 * Agent 工作台页面宿主。
 *
 * 只做两件事：撑满窗口、提供消息容器。
 * 旧版聊天页（GuaxiChatPage）仍在 `/chat-legacy`，它的头部工具栏与左右侧栏是那一版的结构；
 * 工作台自己的三段布局（会话 / 转录 / 仪表）已经把这些都包含了，所以这里不再套外层侧栏。
 */
import { NMessageProvider } from 'naive-ui'
import AgentWorkspace from '../components/AgentWorkspace/AgentWorkspace.vue'
import GuaxiNavRail from '../components/GuaxiNavRail.vue'
import { createLogger } from '../utils/logger'

const logger = createLogger('GuaxiAgentWorkspacePage')

logger.info('Agent 工作台页面已挂载')
</script>

<style scoped>
.agent-page {
  display: flex;
  height: 100vh;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: var(--guaxi-content-bg);
}

/* 导航列固定宽度，工作台吸收剩余空间。 */
.agent-page > :last-child {
  flex: 1 1 auto;
  min-width: 0;
}
</style>
