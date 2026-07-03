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
  min-width: 0;
  min-height: 0;
  /* 右侧不留白：让文件库面板贴紧分割线，避免与表格之间出现过宽空隙 */
  padding: 5px 0 5px 5px;
  /* 填满实际可用高度（内容区已是 100vh-顶栏），不要用 100vh 否则比插槽高出顶栏高度，
     触发本列自身滚动；滚动交给内部菜单。 */
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
}
</style>