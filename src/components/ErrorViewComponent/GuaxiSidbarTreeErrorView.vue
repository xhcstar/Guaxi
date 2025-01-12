<template>
  <div class="tree-column">
    <TreeViewErrorView />
  </div>
</template>

<script setup>
import TreeViewErrorView from "./TreeViewErrorView.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>