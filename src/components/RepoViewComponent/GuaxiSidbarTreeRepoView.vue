<template>
    <n-message-provider>
      <div class="tree-column">
        <RepoList />
      </div>
    </n-message-provider>
</template>

<script setup>
import RepoList from "./RepoList.vue";
import { NMessageProvider } from 'naive-ui'

</script>

<style scope>
.tree-column {
  flex: 1;
  padding: 5px;
  overflow-y: auto;
  height: 100vh;
}
</style>