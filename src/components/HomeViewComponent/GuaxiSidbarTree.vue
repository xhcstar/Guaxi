<template>
  <div class="tree-column">
    <!-- <TreeView /> -->
    <GuaxiFileTree />
  </div>
</template>

<script setup>
// import TreeView from "./TreeView.vue";
import GuaxiFileTree from './GuaxiFileTree.vue';

</script>

<style scoped>
.tree-column {
  flex: 1;
  padding: 0;
  overflow: hidden;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
}
</style>