<template>
    <n-input round size="small" placeholder="搜索" />
    <n-tree
      ref="treeInstRef"
      block-line
      :data="data"
      default-expand-all
      :default-expanded-keys="defaultExpandedKeys"
      virtual-scroll
      style="height: 95vh"
      :render-switcher-icon="renderSwitcherIconWithExpaned"
      selectable
    />
  </template>
  
  <script >
  import { ChevronForward, PlanetOutline, SunnyOutline } from '@vicons/ionicons5'
  import { NIcon } from 'naive-ui'
  import { repeat } from 'seemly'
  import { defineComponent, h, ref } from 'vue'
  
  function createData(level = 4, baseKey = ''){
    if (!level)
      return undefined
    return repeat(6 - level, undefined).map((_, index) => {
      const key = `${baseKey}${level}${index}`
      return {
        label: createLabel(level),
        key,
        children: createData(level - 1, key)
      }
    })
  }
  
  function createLabel(level) {
    if (level === 4)
      return '道生一'
    if (level === 3)
      return '一生二'
    if (level === 2)
      return '二生三'
    if (level === 1)
      return '三生万物'
    return ''
  }
  
  export default defineComponent({
    setup() {
      return {
        data: createData(),
        defaultExpandedKeys: ref(['40', '41']),
        renderSwitcherIcon: () =>
          h(NIcon, null, { default: () => h(ChevronForward) }),
        renderSwitcherIconWithExpaned: ({ expanded }) =>
          h(NIcon, null, {
            default: () => h(expanded ? SunnyOutline : PlanetOutline)
          })
      }
    }
  })
  </script>