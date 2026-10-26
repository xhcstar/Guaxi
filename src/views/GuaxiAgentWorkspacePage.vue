<template>
  <n-message-provider placement="bottom">
    <div class="agent-page">
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
 * 只做三件事：撑满窗口、提供消息容器、放全局导航列。
 * 旧版聊天页（GuaxiChatPage）仍在 `/chat-legacy`；工作台的顶栏、
 * 三段布局（会话 / 转录 / 仪表）与面板开关全部由 AgentWorkspace 自己负责，
 * 页面层不再叠一条应用顶栏。
 */
import { NMessageProvider } from 'naive-ui'
import AgentWorkspace from '../components/AgentWorkspace/AgentWorkspace.vue'
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

/* 顶栏由工作台自己画，页面层只负责把剩余高度交给三段布局。 */
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
