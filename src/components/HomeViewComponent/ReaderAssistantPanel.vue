<template>
  <!-- abstract：不要包那一层 div。助手面板靠 flex 撑满高度，多一层普通块会把高度链断掉，
       表现为正文塌成 0 高或溢出。 -->
  <n-config-provider :theme="isDark ? darkTheme : null" abstract>
    <n-message-provider placement="bottom">
      <GuaxiAgentAssistant />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup>
// 独立阅读器窗口的内置助手宿主。
//
// 为什么需要这一层：`GuaxiAgentAssistant` 在 setup 里就调用 `useMessage()`，没有外层
// `<n-message-provider>` 会直接抛错。主应用由 App.vue 包了一层、/reader 由 GuaxiReader
// 包了一层，而 doc-reader.html 这条链路没有 —— 助手挂进来就会炸。
//
// 为什么单独一个文件：naive-ui 不能被静态引进阅读器入口（见 reader-main.js 的说明），
// 所以 provider 与助手一起待在这个异步 chunk 里，只有真正展开面板才加载。
import { computed } from 'vue'
import { NConfigProvider, NMessageProvider, darkTheme } from 'naive-ui'
import GuaxiAgentAssistant from './GuaxiAgentAssistant.vue'

const props = defineProps({
  theme: { type: String, default: 'light' },
})

const isDark = computed(() => props.theme === 'dark')
</script>
