<!-- filepath: e:\Record\source\Vue\guaxi\src\components\RepoViewComponent/components/TableViewComponents/ContextMenu.vue -->
<template>
  <div v-if="show" class="custom-context-menu-wrapper" @contextmenu.prevent>
    <n-dropdown placement="bottom-start" trigger="manual" :options="options" :show="show" :x="x" :y="y"
      @clickoutside="handleClickOutside" @select="handleSelect">
      <!-- 为 dropdown 提供一个虚拟触发元素 -->
      <template #default>
        <div :style="menuStyle" class="dropdown-trigger" @mousedown.stop />
      </template>
    </n-dropdown>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { NDropdown } from 'naive-ui'

const props = defineProps({
  show: {
    type: Boolean,
    default: false
  },
  x: {
    type: Number,
    default: 0
  },
  y: {
    type: Number,
    default: 0
  },
  options: {
    type: Array,
    default: () => []
  }
})

const emit = defineEmits(['update:show', 'select', 'clickoutside'])

// 菜单样式
const menuStyle = computed(() => ({
  position: 'absolute',
  left: `${props.x}px`,
  top: `${props.y}px`,
  width: '1px',
  height: '1px',
  pointerEvents: 'none'
}))

const handleSelect = (key) => {
  emit('select', key)
  emit('update:show', false)
}

const handleClickOutside = () => {
  emit('clickoutside')
  emit('update:show', false)
}
</script>

<style scoped>
.custom-context-menu-wrapper {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  z-index: 9999;
  pointer-events: none;
}

.dropdown-trigger {
  position: fixed;
  width: 0;
  height: 0;
  pointer-events: none;
}

:deep(.n-dropdown) {
  z-index: 10000 !important;
  pointer-events: auto;
}

:deep(.n-dropdown-menu) {
  z-index: 10001 !important;
  padding: 6px;
  border-radius: 10px;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.22);
  pointer-events: auto;
  /* 与主界面字体保持一致 */
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', 'Noto Sans CJK SC', Consolas, 'Courier New', monospace;
  /* 取消内部垂直滚动条：完整显示所有条目（不截断） */
  max-height: none !important;
}

/* 禁用 naive-ui 下拉内部的滚动容器与滚动条，改为自然高度铺开 */
:deep(.n-dropdown-menu .n-scrollbar),
:deep(.n-dropdown-menu .n-scrollbar-container) {
  max-height: none !important;
  overflow: visible !important;
}
:deep(.n-dropdown-menu .n-scrollbar-rail) {
  display: none !important;
}

:deep(.n-dropdown-option) {
  min-width: 180px;
}

:deep(.n-dropdown-option .n-dropdown-option-body) {
  border-radius: 7px;
  padding: 7px 10px;
  transition: background-color 0.15s ease, color 0.15s ease;
  font-family: inherit;
}

:deep(.n-dropdown-option .n-dropdown-option-body__prefix) {
  margin-right: 10px;
  color: var(--guaxi-text-secondary);
  transition: color 0.15s ease;
}

:deep(.n-dropdown-option .n-dropdown-option-body__label) {
  font-size: 13px;
  font-family: inherit;
}

:deep(.n-dropdown-option .n-dropdown-option-body:not(.n-dropdown-option-body--disabled):hover) {
  background-color: var(--n-color-primary, #409EFF);
  color: #fff;
}

:deep(.n-dropdown-option .n-dropdown-option-body:not(.n-dropdown-option-body--disabled):hover .n-dropdown-option-body__prefix) {
  color: #fff;
}

:deep(.n-dropdown-divider) {
  margin: 5px 6px;
  background-color: var(--guaxi-border);
}
</style>