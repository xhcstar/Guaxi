<template>
  <div>
    <n-button @click="showAnchor = !showAnchor">切换 Anchor</n-button>
    <div v-if="showAnchor" class="document-scroll-container" style="height: 100vh; overflow: auto;">
      <div style="height: 200px">
        <n-anchor
          affix
          listen-to=".document-scroll-container"
          :trigger-top="24"
          :top="88"
          style="z-index: 1"
          :bound="24"
        >
          <n-anchor-link title="演示" href="#演示">
            <n-anchor-link title="基础用法" href="#basic.vue" />
            <n-anchor-link title="忽略间隔" href="#ignore-gap.vue" />
            <n-anchor-link title="固定" href="#affix.vue" />
            <n-anchor-link title="滚动到" href="#scrollto.vue" />
          </n-anchor-link>
          <n-anchor-link title="API" href="#API" />
        </n-anchor>
      </div>
      <div id="演示" style="height: 1000px; background: #f0f0f0;">演示</div>
      <div id="basic.vue" style="height: 1000px; background: #e0e0e0;">基础用法</div>
      <div id="ignore-gap.vue" style="height: 1000px; background: #d0d0d0;">忽略间隔</div>
      <div id="affix.vue" style="height: 1000px; background: #c0c0c0;">固定</div>
      <div id="scrollto.vue" style="height: 1000px; background: #b0b0b0;">滚动到</div>
      <div id="API" style="height: 1000px; background: #a0a0a0;">API</div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'

const showAnchor = ref(false)
</script>