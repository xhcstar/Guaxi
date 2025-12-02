<template>
  <div class="text-area">
    <TableViewRepoView/>
  </div>
</template>

<script setup>
import TableViewRepoView from './TableViewRepoView.vue'

</script>

<style scope>

.text-area {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
}
</style>