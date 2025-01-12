<template>
  <div class="right-sider-content">
    <SideNavigatorAppView />
  </div>
</template>

<script setup>
import SideNavigatorAppView from "./SideNavigatorAppView.vue";
</script>

<style scope>
.right-sider-content {
  flex-grow: 1;
  padding: 0px;
  margin-left: 5px;
  margin-top: 2px;
  width: 100%;
}
</style>