<template>
  <div class="tree-column">
    <TreeViewBrainView />
  </div>
</template>

<script setup>
import TreeViewBrainView from "./TreeViewBrainView.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>