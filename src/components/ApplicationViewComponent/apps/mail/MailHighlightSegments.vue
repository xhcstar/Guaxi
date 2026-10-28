<template>
  <template v-for="(part, index) in parts" :key="index">
    <mark v-if="part.hit" class="mail-hl">{{ part.text }}</mark>
    <template v-else>{{ part.text }}</template>
  </template>
</template>

<script setup>
// 搜索结果里的一段文本：命中的部分包成 <mark>，其余原样输出。
// 抽成组件是为了让列表行（虚拟滚动时每屏十几行）不必各自展开一段 v-for。
defineProps({
  parts: { type: Array, default: () => [] },
})
</script>
