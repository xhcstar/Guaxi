<template>
  <div class="right-sider-content">
    <SideNavigatorSearchView />
  </div>
</template>

<script setup>
import SideNavigatorSearchView from "./SideNavigatorSearchView.vue";
</script>

<style scope>
.right-sider-content {
  flex-grow: 1;
  padding: 0px;
  margin-left: 5px;
  margin-top: 2px;
  width: 100vh;
}
</style>