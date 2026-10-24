<template>
  <button
    type="button"
    class="sider-toggle"
    :title="label"
    :aria-label="label"
    :aria-expanded="!collapsed"
    @click="toggle"
  >
    <n-icon size="18" :component="icon" />
  </button>
</template>

<script setup>
/**
 * 侧边栏展开 / 收起按钮（左、右各一个实例）。
 *
 * 背景：这个按钮原先在每个页面的 Header 里各写一遍——有的用 20px 的 Fluent 图标、
 * 有的手写 SVG，尺寸、内边距、hover 效果互不相同，而且图标永远是「收起」那一个，
 * 侧栏已经收起时也看不出来。这里收成唯一实现：同一种图标体系、同一种点击区域，
 * 图标跟着当前状态切换（展开态显示「收起」，收起态显示「展开」），
 * 快捷键 Alt+[ / Alt+] 也随组件一起注册，页面不用再各写一份。
 *
 * 只负责「折叠状态」本身，侧栏容器怎么让位由宿主页面决定
 * （各页面读同一个 siderStore，按自己的布局收起）。
 */
import { computed } from 'vue'
import { NIcon } from 'naive-ui'
import {
  PanelLeftContract20Regular,
  PanelLeftExpand20Regular,
  PanelRightContract20Regular,
  PanelRightExpand20Regular,
} from '@vicons/fluent'
import { useSiderStore } from '../stores/useSiderStore'
import { useSiderHotkeys } from '../composables/useSiderHotkeys'

const props = defineProps({
  /** 与 siderStore 的 viewKey 一致，决定折叠状态按哪个页面记忆。 */
  viewKey: { type: String, required: true },
  /** 'left' | 'right' */
  side: {
    type: String,
    default: 'left',
    validator: (v) => v === 'left' || v === 'right',
  },
})

const siderStore = useSiderStore()

const isLeft = computed(() => props.side === 'left')

/**
 * 右侧栏在各页面一直默认收起（旧版行为），左侧栏默认展开。
 * 未初始化时按这个默认值显示，点击后 siderStore 才有显式状态。
 */
const collapsed = computed(() => {
  const map = isLeft.value
    ? siderStore.isLeftSiderCollapsed
    : siderStore.isRightSiderCollapsed
  return map[props.viewKey] ?? !isLeft.value
})

const icon = computed(() => {
  if (isLeft.value) {
    return collapsed.value ? PanelLeftExpand20Regular : PanelLeftContract20Regular
  }
  return collapsed.value ? PanelRightExpand20Regular : PanelRightContract20Regular
})

const label = computed(() => {
  const target = isLeft.value ? '左侧栏' : '右侧栏'
  const action = collapsed.value ? '展开' : '收起'
  return `${action}${target}（${isLeft.value ? 'Alt+[' : 'Alt+]'}）`
})

const toggle = () => {
  if (isLeft.value) {
    siderStore.toggleLeftSider(props.viewKey)
  } else {
    siderStore.toggleRightSider(props.viewKey)
  }
}

useSiderHotkeys(
  isLeft.value ? { toggleLeft: toggle } : { toggleRight: toggle },
)
</script>

<style scoped>
.sider-toggle {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--guaxi-text-secondary);
  cursor: pointer;
  transition: background-color 0.16s ease, color 0.16s ease;
}

.sider-toggle:hover {
  color: var(--guaxi-text-primary);
  background: rgba(127, 127, 127, 0.16);
}

.sider-toggle:active {
  background: rgba(127, 127, 127, 0.24);
}

.sider-toggle:focus-visible {
  outline: 2px solid var(--n-color-primary);
  outline-offset: 1px;
}

@media (prefers-reduced-motion: reduce) {
  .sider-toggle {
    transition: none;
  }
}
</style>
