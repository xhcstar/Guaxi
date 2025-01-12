<template>
  <div class="tree-column">
    <TreeViewStatView />
  </div>
</template>

<script setup>
import TreeViewStatView from "./TreeViewStatView.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>