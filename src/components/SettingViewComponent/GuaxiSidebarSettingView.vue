<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <GuaxiSidbarTreeSettingView />
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
import GuaxiSidbarTreeSettingView from "./GuaxiSidbarTreeSettingView.vue"
</script>

<style scoped>
.sider-container {
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
</style>