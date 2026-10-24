<template>
  <div class="sider-container">
    <GuaxiNavRail />
    <!-- 左栏内容按顶栏选的视图分发：
         图谱模式给知识库列表 + 文件树（点击可在图中定位），
         大脑模式给待处理分类、感知来源、简报历史与快捷操作。
         两者都占满这块区域，不会出现"文件树消失后留一片空白"。 -->
    <GuaxiCognitionLeftPanel v-if="cog.viewMode === 'cognition'" />
    <GuaxiSidbarBrainContent v-else />
  </div>
</template>

<script setup>
import GuaxiNavRail from '../GuaxiNavRail.vue'
import GuaxiSidbarBrainContent from "./GuaxiSidbarBrainContent.vue"
import GuaxiCognitionLeftPanel from "./GuaxiCognitionLeftPanel.vue"
import { useCognitionStore } from '../../stores/cognitionStore'

const cog = useCognitionStore()
</script>

<style scoped>
.sider-container {
  display: flex;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
</style>