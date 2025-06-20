import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useSiderStore = defineStore('sider', () => {
  const isLeftSiderCollapsed = ref({})
  const isRightSiderCollapsed = ref({})

  function toggleLeftSider(viewKey) {
    // 默认展开(false)，未初始化时先设为false
    if (typeof isLeftSiderCollapsed.value[viewKey] === 'undefined') {
      isLeftSiderCollapsed.value[viewKey] = false
    }
    isLeftSiderCollapsed.value[viewKey] = !isLeftSiderCollapsed.value[viewKey]
  }
  function toggleRightSider(viewKey) {
    // 默认隐藏(true)，未初始化时先设为true
    if (typeof isRightSiderCollapsed.value[viewKey] === 'undefined') {
      isRightSiderCollapsed.value[viewKey] = true
    }
    isRightSiderCollapsed.value[viewKey] = !isRightSiderCollapsed.value[viewKey]
  }

  function setLeftSiderCollapsed(viewKey, value) {
    isLeftSiderCollapsed.value[viewKey] = value
  }
  function setRightSiderCollapsed(viewKey, value) {
    isRightSiderCollapsed.value[viewKey] = value
  }

  return {
    isLeftSiderCollapsed,
    isRightSiderCollapsed,
    toggleLeftSider,
    toggleRightSider,
    setLeftSiderCollapsed,
    setRightSiderCollapsed,
  }
})