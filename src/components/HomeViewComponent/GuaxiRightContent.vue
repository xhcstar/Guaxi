<template>
  <div class="right-sider-content">
    <GuaxiOutline v-show="rightSiderStore.panel === 'outline'" />
    <GuaxiSearchNavigator v-show="rightSiderStore.panel === 'search'"
      :file-results="rightSiderStore.searchResults.fileResults"
      :note-results="rightSiderStore.searchResults.noteResults"
      :search-query="rightSiderStore.searchResults.searchQuery"
      :excerpt-results="rightSiderStore.searchResults.excerptResults"
      :last-search-type="rightSiderStore.searchResults.lastSearchType" />
    <KeepAlive>
      <GuaxiAgentAssistant v-if="rightSiderStore.panel === 'assistant'" />
    </KeepAlive>
  </div>
</template>

<script setup>
import GuaxiSearchNavigator from "./GuaxiSearchNavigator.vue"
import GuaxiAgentAssistant from "./GuaxiAgentAssistant.vue"
import GuaxiOutline from './GuaxiOutline.vue'
import { useRightSiderStore } from '../../stores/rightSiderStore'

const rightSiderStore = useRightSiderStore()
</script>

<style scoped>
.right-sider-content {
  flex: 1 1 0;
  height: 100%;
  min-width: 0;
  min-height: 0;
  max-width: 100vw;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
