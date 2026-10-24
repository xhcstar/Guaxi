<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <GuaxiSidbarListChatView />
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
import GuaxiSidbarListChatView from "./GuaxiSidbarListChatView.vue"
</script>

<style scoped>
.sider-container {
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
</style>