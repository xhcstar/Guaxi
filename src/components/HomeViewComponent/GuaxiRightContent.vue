<template>
  <div class="right-sider-content">
    <GuaxiOutline v-if="rightSiderStore.panel === 'outline'" />
    <GuaxiSearchNavigator v-else-if="rightSiderStore.panel === 'search'"
      :file-results="rightSiderStore.searchResults.fileResults"
      :note-results="rightSiderStore.searchResults.noteResults"
      :search-query="rightSiderStore.searchResults.searchQuery"
      :excerpt-results="rightSiderStore.searchResults.excerptResults"
      :last-search-type="rightSiderStore.searchResults.lastSearchType" />
  </div>
</template>

<script setup>
import GuaxiSearchNavigator from "./GuaxiSearchNavigator.vue"
import GuaxiOutline from './GuaxiOutline.vue'
import { useRightSiderStore } from '../../stores/rightSiderStore'

const rightSiderStore = useRightSiderStore()
</script>

<style scoped>
.right-sider-content {
  flex: 1 1 0;
  min-width: 0;
  max-width: 100vw;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  overflow: auto;
  display: flex;
  flex-direction: column;
  height: 100%;
}
</style>