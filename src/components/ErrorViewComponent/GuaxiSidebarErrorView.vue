<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <GuaxiSidbarTreeErrorView />
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
import GuaxiSidbarTreeErrorView from "./GuaxiSidbarTreeErrorView.vue"
</script>

<style scoped>
.sider-container {
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
</style>