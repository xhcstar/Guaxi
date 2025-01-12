<template>
  <div class="tree-column">
    <TreeViewSearchView />
  </div>
</template>

<script setup>
import TreeViewSearchView from "./TreeViewSearchView.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>