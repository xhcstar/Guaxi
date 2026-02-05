import { defineStore } from 'pinia'
import { ref } from 'vue'
import { createLogger } from '../utils/logger'

const logger = createLogger('useSiderStore')
const console = {
  debug: (...args) => logger.debug(...args),
  log: (...args) => logger.info(...args),
  info: (...args) => logger.info(...args),
  warn: (...args) => logger.warn(...args),
  error: (...args) => logger.error(...args),
}

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
    console.log('setRightSiderCollapsed', viewKey, value)
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