<!-- components/TableViewComponents/ContextMenu.vue -->
<template>
  <n-dropdown v-if="show" placement="bottom-start" trigger="manual" :x="x" :y="y" :options="options" :show="show"
    :style="dropdownStyle" @clickoutside="handleClickOutside" @select="handleSelect" class="custom-context-menu" />
</template>

<script setup>
import { ref, computed } from 'vue'
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

// 下拉菜单样式
const dropdownStyle = computed(() => ({
  zIndex: 9999,
  position: 'fixed'
}))

// 处理选择
const handleSelect = (key) => {
  emit('select', key)
  emit('update:show', false)
}

// 处理点击外部
const handleClickOutside = () => {
  emit('clickoutside')
  emit('update:show', false)
}
</script>

<style scoped>
.custom-context-menu {
  z-index: 9999;
}

:deep(.n-dropdown) {
  z-index: 10000 !important;
}

:deep(.n-dropdown-menu) {
  z-index: 10001 !important;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  border: 1px solid #e0e0e0;
}

:deep(.n-dropdown-option) {
  min-width: 140px;
  padding: 8px 12px;
}

:deep(.n-dropdown-option:hover) {
  background-color: #f0f7ff;
}
</style>