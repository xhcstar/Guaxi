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
  /**
   * 编辑区「上下装饰条」的显示开关（按 viewKey 记）。
   *
   * 这两条指的是标签栏下方的**格式工具栏**和**面包屑导航**，它们服务于文本编辑；
   * 打开 PDF / 电子书 / 图片这类只读阅读器时，它们既不适用也占掉宝贵的高度。
   *
   * 为什么放在这里而不是编辑器组件内部：开关按钮在主窗口 Header 上，
   * Header 与编辑器是两棵独立的组件树，共享状态只能落在 store。
   * 默认显示（与改造前一致），用户折叠后按窗口记住。
   */
  const isEditorChromeVisible = ref({})

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

  function toggleEditorChrome(viewKey) {
    const key = viewKey || 'default'
    isEditorChromeVisible.value[key] = isEditorChromeVisible.value[key] === false
  }
  function setEditorChromeVisible(viewKey, value) {
    isEditorChromeVisible.value[viewKey || 'default'] = value !== false
  }
  function isEditorChromeShown(viewKey) {
    // 未初始化 = 显示，保证老用户升级后界面与之前完全一致。
    return isEditorChromeVisible.value[viewKey || 'default'] !== false
  }

  return {
    isEditorChromeVisible,
    toggleEditorChrome,
    setEditorChromeVisible,
    isEditorChromeShown,
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