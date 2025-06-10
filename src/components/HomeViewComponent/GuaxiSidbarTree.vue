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

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;  /* 添加内部滚动 */
  height: 100%;
  display: flex;
  flex-direction: column;
}
</style>