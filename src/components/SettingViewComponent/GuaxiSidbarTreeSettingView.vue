<template>
  <div class="tree-column">
    <SidebarList />
  </div>
</template>

<script setup>
import SidebarList from "./SidebarList.vue";

</script>

<style scoped>
.tree-column {
  flex: 1;
  min-width: 0;
  min-height: 0;
  padding: 5px;
  /* 填满实际可用高度（内容区已是 100vh-顶栏），不要用 100vh，
     否则整列比可视区域高出顶栏高度，底部被父级裁掉且无法滚动。 */
  height: 100%;
  box-sizing: border-box;
  overflow-y: auto;
  /* 必须是 block：仓库里 GuaxiSidbarTreeRepoView 等组件把 <style scoped> 写成了
     <style scope>，其 .tree-column{display:flex;flex-direction:column} 会全局生效，
     把菜单当成 flex 子项压缩到可视高度，菜单内部再 overflow:hidden 藏掉剩余条目，
     于是本列认为没有溢出、永远不出现滚动条。 */
  display: block;
}
</style>