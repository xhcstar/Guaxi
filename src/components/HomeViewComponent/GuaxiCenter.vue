<template>
  <div class="guaxi-center">
    <GuaxiEditor/>
  </div>
</template>

<script setup>

import GuaxiEditor from './GuaxiEditor.vue';

</script>

<style scoped>
.guaxi-center {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
</style>