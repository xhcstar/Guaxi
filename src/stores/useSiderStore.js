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
  // 侧栏宽度也按 viewKey 记在这里。宽度放在布局组件的 ref 里的话，
  // 只要用户切到别的页面把组件卸载掉，拖出来的宽度就回到默认值了。
  const leftSiderWidth = ref({})
  const rightSiderWidth = ref({})

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

  function setLeftSiderWidth(viewKey, value) {
    leftSiderWidth.value[viewKey] = value
  }
  function setRightSiderWidth(viewKey, value) {
    rightSiderWidth.value[viewKey] = value
  }

  return {
    isLeftSiderCollapsed,
    isRightSiderCollapsed,
    leftSiderWidth,
    rightSiderWidth,
    toggleLeftSider,
    toggleRightSider,
    setLeftSiderCollapsed,
    setRightSiderCollapsed,
    setLeftSiderWidth,
    setRightSiderWidth,
  }
})