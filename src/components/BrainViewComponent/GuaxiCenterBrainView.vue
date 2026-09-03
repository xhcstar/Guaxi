<template>
  <div class="text-aera">
    <!-- 视图由顶栏切换（见 GuaxiHeaderBrainView 的 brain-mode-switch），
         这里只负责挂载。用 KeepAlive 是刻意的：图谱组件已实现 onDeactivated 时停 RAF，
         所以既能保留相机与布局状态，又不会在后台空耗 GPU。 -->
    <KeepAlive>
      <GuaxiCognitionInbox v-if="mode === 'cognition'" @open-ref="onOpenRef" />
      <GuaxiKnowledgeGraph3D v-else :kb-name="kbName" />
    </KeepAlive>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import GuaxiKnowledgeGraph3D from './GuaxiKnowledgeGraph3D.vue'
import GuaxiCognitionInbox from './GuaxiCognitionInbox.vue'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { useCognitionStore } from '../../stores/cognitionStore'

const store = useKnowledgeBaseStore()
const cog = useCognitionStore()
const kbName = computed(() => store.selectedKnowledgeBase?.knowledge_base_name || '')
const mode = computed(() => cog.viewMode)

/**
 * 提醒里的引用被点开。
 *
 * 图谱节点 → 切到图谱并请求聚焦（复用既有的 kg3d 动作总线）；
 * 其他（percept / task / 纯路径）→ 交给收件箱自己处理（打开原文或弹窗）。
 */
function onOpenRef(ref_) {
  if (!ref_ || ref_.kind !== 'kg_node') return
  cog.setViewMode('graph')
  store.requestKg3dAction('focus', { nodeId: ref_.id })
}
</script>

<style scoped>
.text-aera {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 2px;
  margin-top: 2px;
}

:deep(.knowledge-graph-container) {
  flex: 1 1 auto;
  min-height: 0;
}
</style>
