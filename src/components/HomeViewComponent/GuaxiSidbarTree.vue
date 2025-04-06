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
  overflow: hidden;
  /* 添加这行 */
  display: flex;
  /* 添加这行 */
  flex-direction: column;
  /* 添加这行 */
  height: 100%;
}
</style>