<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <GuaxiSiderToggle view-key="brain" side="left" />
      <div class="logo-text">Guaxi</div>
    </div>

    <!-- 中间:图谱视图切换 -->
    <div class="header-center">
      <div class="kg-switch">
        <!-- <button :class="{ on: kbStore.graphViewMode === 'kg3d' }" @click="kbStore.graphViewMode = 'kg3d'">工业图谱</button> -->
        <!-- <button :class="{ on: kbStore.graphViewMode === 'classic' }" @click="kbStore.graphViewMode = 'classic'">经典视图</button> -->
      </div>
    </div>

    <!-- 右侧：功能图标。
         大脑 / 图谱的切换放在这里（原来占位的截图图标位置）：它是页面级的视图切换，
         和左右边栏折叠属于同一类操作，放在顶栏比放在内容区里更符合直觉，
         也把内容区的垂直空间全部还给正文。 -->
    <div class="header-right">
      <div class="brain-mode-switch">
        <button
          class="mode-btn"
          :class="{ on: viewMode === 'cognition' }"
          title="大脑：外部输入、待裁决的矛盾、结构缺口与今日简报"
          @click="cog.setViewMode('cognition')"
        >
          <n-icon :size="17">
            <Icon :icon="brainIcon" width="16" height="16" />
          </n-icon>
          <span class="mode-label">大脑</span>
          <i v-if="cog.openNudgeCount" class="mode-badge">{{ badgeText }}</i>
        </button>
        <button
          class="mode-btn"
          :class="{ on: viewMode === 'graph' }"
          title="图谱：知识结构的空间视图（关系 / 标签 / 时间 / 重要度）"
          @click="cog.setViewMode('graph')"
        >
          <n-icon :size="17">
            <Icon :icon="networkIcon" width="16" height="16" />
          </n-icon>
          <span class="mode-label">图谱</span>
        </button>
      </div>
      <GuaxiSiderToggle view-key="brain" side="right" />
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted } from 'vue'
import { Icon } from '@iconify/vue'
import brainIcon from '@iconify-icons/lucide/brain'
import networkIcon from '@iconify-icons/lucide/network'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useCognitionStore } from '../../stores/cognitionStore'
import GuaxiSiderToggle from '../GuaxiSiderToggle.vue'

const kbStore = useKnowledgeBaseStore();
const cog = useCognitionStore();

const viewMode = computed(() => cog.viewMode)
// 超过 99 就不再显示具体数字：徽标是"有多少待处理"的信号，不是精确计数。
const badgeText = computed(() => (cog.openNudgeCount > 99 ? '99+' : String(cog.openNudgeCount)))

onMounted(() => {
  // 顶栏徽标要在进入页面时就准确，不能等内容区加载完。
  cog.refreshStatus().then(() => {
    if (cog.enabled) cog.refreshNudges()
  })
})
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  background-color: var(--guaxi-header-bg);
  box-shadow: var(--guaxi-header-shadow);
  border-bottom: 2px solid var(--guaxi-border);
  padding: 0 8px;
  box-sizing: border-box;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
}

.logo-text {
  font-weight: bold;
  font-size: 16px;
  white-space: nowrap;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 200px;
  justify-content: flex-end;
}

.brain-mode-switch {
  display: inline-flex;
  gap: 2px;
  background: rgba(127, 127, 127, 0.14);
  border-radius: 7px;
  padding: 2px;
}

.mode-btn {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 10px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  font-size: 13px;
  line-height: 18px;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.mode-btn:hover {
  color: var(--guaxi-text-primary);
}

.mode-btn.on {
  background: var(--guaxi-panel-bg);
  color: var(--n-color-primary, #2dd4bf);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.16);
}

.mode-label {
  white-space: nowrap;
}

.mode-badge {
  position: absolute;
  top: -5px;
  right: -4px;
  min-width: 16px;
  padding: 0 4px;
  border-radius: 8px;
  background: #e04f5f;
  color: #fff;
  font-size: 10px;
  font-style: normal;
  line-height: 16px;
  text-align: center;
}

.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
}
.kg-switch {
  display: inline-flex;
  gap: 3px;
  background: rgba(127, 127, 127, 0.14);
  border-radius: 7px;
  padding: 2px;
}
.kg-switch button {
  padding: 2px 12px;
  font-size: 12px;
  line-height: 18px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--guaxi-text-primary);
  cursor: pointer;
  transition: background 0.15s ease;
}
.kg-switch button.on {
  background: #1f6feb;
  color: #fff;
}

.header-icon {
  cursor: pointer;
  transition: all 0.2s ease;
  padding: 2px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.header-icon:hover {
  opacity: 0.8;
  transform: scale(1.05);
  background-color: rgba(0, 0, 0, 0.05);
}
</style>
