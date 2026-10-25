<template>
  <button
    type="button"
    class="sider-toggle"
    :class="{ 'is-collapsed': collapsed }"
    :title="label"
    :aria-label="label"
    :aria-expanded="!collapsed"
    @click="toggle"
  >
    <Icon :icon="icon" width="18" height="18" />
  </button>
</template>

<script setup>
/**
 * 侧边栏展开 / 收起按钮（左、右各一个实例）。
 *
 * 背景：这个按钮原先在每个页面的 Header 里各写一遍——有的用 20px 的 Fluent 图标、
 * 有的手写 SVG，尺寸、内边距、hover 效果互不相同，而且图标永远是「收起」那一个，
 * 侧栏已经收起时也看不出来。这里收成唯一实现：同一种图标体系、同一种点击区域，
 * 快捷键 Alt+[ / Alt+] 也随组件一起注册，页面不用再各写一份。
 *
 * 图标改成 Lucide 的 panel-left / panel-right（Obsidian 同一套图标语言的「面板」造型）：
 * 图形本身不随状态翻转，靠透明度与 title / aria-expanded 表达当前是收起还是展开，
 * 和 Obsidian 的侧栏开关观感一致。图标是 import 进来的数据对象，会被 Vite 打进 bundle，
 * 不依赖运行时按名字下载，release 断网时也不会变成空白。
 *
 * 只负责「折叠状态」本身，侧栏容器怎么让位由宿主页面决定
 * （各页面读同一个 siderStore，按自己的布局收起）。
 */
import { computed } from 'vue'
import { Icon } from '@iconify/vue'
import panelLeftIcon from '@iconify-icons/lucide/panel-left'
import panelRightIcon from '@iconify-icons/lucide/panel-right'
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

const icon = computed(() => (isLeft.value ? panelLeftIcon : panelRightIcon))

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
  transition: background-color 0.16s ease, color 0.16s ease, opacity 0.16s ease;
}

/* 侧栏已收起时图标变淡：图形语言和 Obsidian 一样固定，用明暗补一个状态提示。 */
.sider-toggle.is-collapsed {
  opacity: 0.62;
}

.sider-toggle:hover {
  color: var(--guaxi-text-primary);
  background: rgba(127, 127, 127, 0.16);
  opacity: 1;
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
