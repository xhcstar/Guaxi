<template>
  <div class="guaxi-header">
    <!-- 左侧：Logo 和 Icon -->
    <div class="header-left">
      <n-icon size="20" class="header-icon" :component="PanelLeftContract24Regular" @click="toggleLeftSider"></n-icon>
      <div class="logo-text">Guaxi</div>
      <ChatModelSelector class="model-selector" />
    </div>

    <!-- 中间：搜索框 (占位实现) -->
    <div class="header-center">
      <div class="search-container">
        <n-input round clearable placeholder="搜索..." @focus="showSearchPlaceholder">
          <template #prefix>
            <n-icon :component="Search20Filled" />
          </template>
        </n-input>
      </div>
    </div>

    <!-- 右侧：功能图标 -->
    <div class="header-right">
      <n-icon class="header-icon" size="20" :component="TextBulletListLtr20Filled" @click="showNotes"></n-icon>
      <n-icon class="header-icon" size="20" :component="Screenshot24Regular" @click="CaptureScreenshot"></n-icon>
      <n-icon class="header-icon" size="20" :component="PanelRightContract24Regular" @click="toggleRightSider"></n-icon>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useSiderStore } from '../../stores/useSiderStore'
import ChatModelSelector from "./ChatModelSelector.vue"
import {
  PanelLeftContract24Regular,
  PanelRightContract24Regular,
  Search20Filled,
  TextBulletListLtr20Filled,
  Screenshot24Regular
} from '@vicons/fluent'

const viewKey = 'chat'
const siderStore = useSiderStore();
const toggleLeftSider = () => {
  siderStore.toggleLeftSider(viewKey)
};

const toggleRightSider = () => {
  siderStore.toggleRightSider(viewKey)
};

// 占位函数
const showSearchPlaceholder = () => {
  console.log('搜索框获得焦点 - 占位功能');
};

const showNotes = () => {
  console.log('显示笔记点击');
};

const CaptureScreenshot = () => {
  console.log('截图功能点击');
};
</script>

<style scoped>
.guaxi-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 35px;
  background-color: #f5f5f5;
  box-shadow: 0px 2px 4px rgba(0, 0, 0, 0.1);
  border-bottom: 2px solid #aab4ae;
  padding: 0 8px;
  box-sizing: border-box;
}

/* 左侧样式 */
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

.model-selector {
  margin-left: 16px;
}

/* 中间样式 */
.header-center {
  flex: 1;
  display: flex;
  justify-content: center;
  max-width: 400px;
  margin: 0 10px;
}

.search-container {
  width: 100%;
}

/* 右侧样式 */
.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 200px;
  justify-content: flex-end;
}

/* 图标通用样式 */
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

/* 输入框样式 */
:deep(.n-input) {
  height: 25px !important;
  min-height: 25px !important;
  line-height: 25px !important;
}
</style>