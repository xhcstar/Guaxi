<template>
  <div class="setting-page">
    <n-card class="setting-page-card" title="插件管理">
      <p class="setting-page-desc">
        用户可编写的软件级插件(基于内置 Python 引擎)。可启用/禁用、导入/导出、新建、编辑源码;
        带参数的插件可点「参数」配置。编辑器里选中文本后右键“运行插件…”即可调用,
        运行时会先弹出参数面板,文件/目录可直接选择;插件也能通过 ui 协议自己请求输入。
        开发契约见用户手册的「插件」章节或 docs/plugin-api.md。
      </p>
      <GuaxiPluginManager />
    </n-card>
  </div>
</template>

<script setup>
import { NCard } from 'naive-ui'
import GuaxiPluginManager from './GuaxiPluginManager.vue'
</script>
