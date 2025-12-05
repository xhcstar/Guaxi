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
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  border: 1px solid #e0e0e0;
  border-radius: 4px;
  pointer-events: auto;
}

:deep(.n-dropdown-option) {
  min-width: 140px;
  padding: 8px 12px;
}

:deep(.n-dropdown-option:hover) {
  background-color: #f0f7ff;
}
</style>