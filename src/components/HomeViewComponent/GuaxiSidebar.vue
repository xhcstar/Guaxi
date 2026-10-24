<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <GuaxiSidbarTree v-if="!props.iconOnly" />
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
import GuaxiSidbarTree from "./GuaxiSidbarTree.vue"

const props = defineProps({
  iconOnly: {
    type: Boolean,
    default: false
  }
})
</script>

<style scoped>
.sider-container {
  display: flex;
  flex: 1 1 auto;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
</style>