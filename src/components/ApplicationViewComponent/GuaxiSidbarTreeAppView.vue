<template>
  <div class="tree-column">
    <TreeViewAppView />
  </div>
</template>

<script setup>
import TreeViewAppView from "./TreeViewAppView.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100%;
}
</style>