<template>
  <div class="tree-column">
    <TreeViewSettingView />
  </div>
</template>

<script setup>
import TreeViewSettingView from "./TreeViewSettingView.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>