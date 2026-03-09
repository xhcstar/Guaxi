<template>
  <div class="html-reader-container">
    <div class="html-reader-header">
      <span>HTML 阅读器</span>
      <button class="toggle-assistant" type="button" @click="toggleAssistantPanel">
        {{ showAssistant ? '隐藏助手' : '显示助手' }}
      </button>
    </div>
    <div class="html-reader-body">
      <div class="html-reader-empty">当前 HTML 阅读器暂未实现正文渲染</div>
    </div>
    <div class="assistant-panel" :class="{ 'assistant-panel-visible': showAssistant }">
      <div class="assistant-header">
        <h3>智能助手</h3>
        <button @click="toggleAssistantPanel" class="close-btn">×</button>
      </div>
      <div class="assistant-content">
        <GuaxiAgentAssistant />
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import GuaxiAgentAssistant from '../HomeViewComponent/GuaxiAgentAssistant.vue'

const showAssistant = ref(false)

const toggleAssistantPanel = () => {
  showAssistant.value = !showAssistant.value
}
</script>

<style scoped>
.html-reader-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.html-reader-header {
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  border-bottom: 1px solid var(--guaxi-border);
  background-color: var(--guaxi-header-bg);
}

.html-reader-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--guaxi-text-secondary);
}

.toggle-assistant {
  border: 1px solid #5f3dc4;
  background-color: #7048e8;
  color: white;
  border-radius: 6px;
  padding: 4px 10px;
  cursor: pointer;
}

.assistant-panel {
  position: fixed;
  top: 0;
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: var(--guaxi-panel-bg);
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 101;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--guaxi-border);
}

.assistant-panel-visible {
  right: 0;
}

.assistant-header {
  height: 48px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background-color: #7048e8;
  color: white;
}

.assistant-header h3 {
  margin: 0;
  font-size: 16px;
}

.close-btn {
  border: none;
  background: transparent;
  color: white;
  font-size: 20px;
  cursor: pointer;
}

.assistant-content {
  flex: 1;
  min-height: 0;
}
</style>
