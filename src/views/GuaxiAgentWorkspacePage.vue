<template>
  <n-message-provider placement="bottom">
    <div class="agent-page">
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
import { createLogger } from '../utils/logger'

const logger = createLogger('GuaxiAgentWorkspacePage')

logger.info('Agent 工作台页面已挂载')
</script>

<style scoped>
.agent-page {
  height: 100vh;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: var(--guaxi-content-bg);
}
</style>
