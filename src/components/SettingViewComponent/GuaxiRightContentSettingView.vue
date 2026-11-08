<template>
  <div class="right-sider-content" :class="{ 'is-manual': isManual }">
    <ManualSidebar v-if="isManual" />
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useSettingsStore } from '../../stores/settingsStore'
import ManualSidebar from './manual/ManualSidebar.vue'

const settingsStore = useSettingsStore()
const isManual = computed(() => settingsStore.selectedMenu === 'manual')
</script>

<style scoped>
.right-sider-content {
  height: 100%;
  padding: 0px;
  margin-left: 5px;
  margin-top: 2px;
  width: 100%;
  overflow: auto;
  box-sizing: border-box;
}

.right-sider-content.is-manual {
  margin: 0;
  overflow: hidden;
}
</style>
