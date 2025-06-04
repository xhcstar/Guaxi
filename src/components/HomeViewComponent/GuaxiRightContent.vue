<template>
  <div class="right-sider-content">
    <GuaxiSearchNavigator 
      :file-results="searchResults.fileResults"
      :note-results="searchResults.noteResults"
      :search-query="searchResults.searchQuery"
    />
  </div>
</template>

<script setup>
import GuaxiSearchNavigator from "./GuaxiSearchNavigator.vue"
import { useRightSiderStore } from '../../stores/rightSiderStore'
import { computed } from 'vue'

const rightSiderStore = useRightSiderStore()
const searchResults = computed(() => rightSiderStore.searchResults)
</script>

<style scoped>
.right-sider-content {
  flex-grow: 1;
  padding: 0;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>