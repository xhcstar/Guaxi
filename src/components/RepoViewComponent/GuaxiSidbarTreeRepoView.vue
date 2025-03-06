<template>
  <div class="tree-column">
    <RepoList />
  </div>
</template>

<script setup>
import RepoList from "./RepoList.vue";

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>