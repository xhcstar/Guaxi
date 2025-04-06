<template>
  <div class="text-aera">
    <GuaxiEditor/>
  </div>
</template>

<script setup>

import GuaxiEditor from './GuaxiEditor.vue';

</script>

<style scoped>
.text-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  padding: 0;
  margin: 0;
  height: 100%;
  overflow: hidden;
  width: 100%;
  box-sizing: border-box;
  position: relative; /* 新增 */
}
</style>