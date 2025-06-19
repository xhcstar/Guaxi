<template>
  <div class="right-sider-content">
    <GuaxiOutline v-if="rightSiderStore.panel === 'outline'" />
    <GuaxiSearchNavigator v-else-if="rightSiderStore.panel === 'search'" :file-results="searchResults.fileResults"
      :note-results="searchResults.noteResults" :search-query="searchResults.searchQuery" />
  </div>
</template>

<script setup>
import GuaxiSearchNavigator from "./GuaxiSearchNavigator.vue"
import GuaxiOutline from './GuaxiOutline.vue'
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { computed } from 'vue'

const rightSiderStore = useRightSiderStore()
const searchResults = computed(() => rightSiderStore.searchResults)
</script>

<style scoped>
.right-sider-content {
  flex: 1 1 0;
  min-width: 0;
  max-width: 100vw;
  box-sizing: border-box;
  background: #fafbfc;
  overflow: auto;
  display: flex;
  flex-direction: column;
  height: 100%;
}
</style>