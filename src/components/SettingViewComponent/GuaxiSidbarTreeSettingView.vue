<template>
  <div class="tree-column">
    <SidebarList />
  </div>
</template>

<script setup>
import SidebarList from "./SidebarList.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>