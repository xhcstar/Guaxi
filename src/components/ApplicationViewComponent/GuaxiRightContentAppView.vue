<template>
  <div class="right-sider-content">
    <component
      v-if="activeAppComponent"
      :is="activeAppComponent"
      class="right-app-root"
    />
    <div v-else class="right-empty">
      <span>请选择一个应用卡片，在这里查看具体界面</span>
    </div>
  </div>
</template>

<script setup>
const props = defineProps({
  activeAppComponent: {
    type: [Object, Function],
    default: null
  }
})
</script>

<style scope>
.right-sider-content {
  flex-grow: 1;
  padding: 0px;
  margin-left: 5px;
  margin-top: 2px;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
}

.right-app-root {
  width: 100%;
  height: 100%;
}

.right-empty {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
  font-size: 13px;
}
</style>