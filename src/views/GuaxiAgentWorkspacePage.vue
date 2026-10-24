<template>
  <n-message-provider placement="bottom">
    <div class="agent-page">
      <div class="main-header">
        <GuaxiHeaderAgentWorkspace />
      </div>
      <!-- 全局导航列：工作台自己的三段布局占满右侧，导航列单独占 44px。
           用户从这里可以随时切走，不会困在 /chat 里。 -->
      <div class="agent-body">
        <GuaxiNavRail />
        <AgentWorkspace />
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
/**
 * Agent 工作台页面宿主。
 *
 * 只做三件事：撑满窗口、提供消息容器、给出与其他页面一致的顶栏。
 * 旧版聊天页（GuaxiChatPage）仍在 `/chat-legacy`，它的头部工具栏与左右侧栏是那一版的结构；
 * 工作台自己的三段布局（会话 / 转录 / 仪表）由 AgentWorkspace 负责，
 * 顶栏只放左右侧栏的展开 / 收起按钮。
 */
import { NMessageProvider } from 'naive-ui'
import AgentWorkspace from '../components/AgentWorkspace/AgentWorkspace.vue'
import GuaxiHeaderAgentWorkspace from '../components/AgentWorkspace/GuaxiHeaderAgentWorkspace.vue'
import GuaxiNavRail from '../components/GuaxiNavRail.vue'
import { useSiderStore } from '../stores/useSiderStore'
import { createLogger } from '../utils/logger'

const logger = createLogger('GuaxiAgentWorkspacePage')

// 会话栏与仪表默认都展开。siderStore 里右侧栏的缺省语义是「收起」（其余页面如此），
// 所以这里在子组件挂载前显式落一次初始状态，折叠按钮的图标才不会一开始就画反。
const siderStore = useSiderStore()
if (typeof siderStore.isLeftSiderCollapsed.chat === 'undefined') {
  siderStore.setLeftSiderCollapsed('chat', false)
}
if (typeof siderStore.isRightSiderCollapsed.chat === 'undefined') {
  siderStore.setRightSiderCollapsed('chat', false)
}

logger.info('Agent 工作台页面已挂载')
</script>

<style scoped>
.agent-page {
  display: flex;
  flex-direction: column;
  height: 100vh;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: var(--guaxi-content-bg);
}

/* 顶栏保持固定高度，下方三段布局吸收剩余空间。 */
.main-header {
  flex: 0 0 auto;
  height: 35px;
  position: relative;
  z-index: 3;
}

.agent-body {
  flex: 1 1 auto;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

/* 导航列固定宽度，工作台吸收剩余空间。 */
.agent-body > :last-child {
  flex: 1 1 auto;
  min-width: 0;
}
</style>
