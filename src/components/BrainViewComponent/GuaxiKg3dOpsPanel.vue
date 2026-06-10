<template>
  <div class="kg3d-ops">
    <n-card title="工业图谱 · 节点" size="small" class="ops-card">
      <template v-if="node">
        <n-space vertical size="small">
          <div class="ops-head">
            <n-tag :type="tagType(node.type)" size="small">{{ typeLabel(node.type) }}</n-tag>
            <span class="ops-name" :title="node.name">{{ node.name }}</span>
          </div>
          <n-text v-if="node.rel_path" depth="3" class="ops-path">{{ node.rel_path }}</n-text>
          <n-text depth="3" style="font-size:12px;">
            度 {{ node.degree ?? 0 }} · PR {{ (node.pagerank || 0).toFixed(4) }}
          </n-text>

          <n-button-group size="small">
            <n-button v-if="canOpen" @click="emitAction('open', { node })">打开</n-button>
            <n-button @click="emitAction('focusEgo')">聚焦邻居</n-button>
            <n-button @click="emitAction('flyTo', { id: node.id })">飞向</n-button>
          </n-button-group>
        </n-space>

        <!-- 标签 -->
        <div class="ops-section">
          <div class="ops-sub">标签</div>
          <div class="ops-tags">
            <n-tag
              v-for="t in node.tags || []"
              :key="t"
              size="small"
              closable
              @close="emitAction('removeTag', { tag: t })"
            >#{{ t }}</n-tag>
            <span v-if="!(node.tags && node.tags.length)" class="ops-empty">暂无标签</span>
          </div>
          <div class="ops-add">
            <n-input v-model:value="newTag" size="tiny" placeholder="添加标签…" @keyup.enter="addTag" />
            <n-button size="tiny" @click="addTag">＋</n-button>
            <n-button size="tiny" tertiary @click="emitAction('suggestTags')">推荐</n-button>
          </div>
        </div>

        <!-- 关系 -->
        <div class="ops-section">
          <div class="ops-sub">关系</div>
          <div class="ops-views">
            <n-button size="tiny" tertiary @click="emitAction('loadRelations')">加载已有</n-button>
            <n-button size="tiny" tertiary @click="emitAction('suggestRelations')">推荐关联</n-button>
            <n-button size="tiny" tertiary @click="emitAction('focusEgo')">高亮邻居</n-button>
          </div>

          <div v-if="relations.length" class="ops-rel-list">
            <div v-for="r in relations" :key="r.to + r.type" class="ops-rel">
              <span class="ops-rel-name" :title="r.to_name || r.to" @click="highlightPair(r.toId ?? r.to)">{{ r.to_name || r.to }}</span>
              <span class="ops-rel-type">{{ r.type }}</span>
              <n-button v-if="r.deletable" size="tiny" type="error" tertiary @click="emitAction('removeRelation', r)">×</n-button>
            </div>
          </div>

          <div v-if="suggestions.length" class="ops-rel-list">
            <div class="ops-sub">建议(点名称预览)</div>
            <div v-for="s in suggestions" :key="s.to" class="ops-rel">
              <span class="ops-rel-name" :title="s.to_name" @click="highlightPair(s.to)">{{ s.to_name }}</span>
              <span class="ops-rel-type">{{ (s.score || 0).toFixed(2) }}</span>
            </div>
          </div>
        </div>
      </template>
      <n-text v-else depth="3">点击图谱中的节点查看与编辑</n-text>
    </n-card>

    <n-card title="视角 / 操作" size="small" class="ops-card">
      <div class="ops-views">
        <n-button v-for="v in views" :key="v.key" size="tiny" @click="emitAction('switchView', { view: v.key })">{{ v.label }}</n-button>
      </div>
      <div class="ops-views" style="margin-top:6px;">
        <n-button size="tiny" tertiary @click="emitAction('community')">社区着色</n-button>
        <n-button size="tiny" tertiary @click="emitAction('embed')">生成向量</n-button>
        <n-button size="tiny" tertiary @click="emitAction('reload')">刷新</n-button>
      </div>
    </n-card>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { NButton, NButtonGroup, NCard, NInput, NSpace, NTag, NText } from 'naive-ui'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'

const store = useKnowledgeBaseStore()
const newTag = ref('')

const node = computed(() => store.kg3dSelected)
const relations = computed(() => store.kg3dRelations)
const suggestions = computed(() => store.kg3dSuggestions)

const highlightPair = (targetId) => emitAction('highlightPair', { targetId })

const views = [
  { key: 'relation', label: '关系' },
  { key: 'tag', label: '分类' },
  { key: 'time', label: '时间' },
  { key: 'importance', label: '重要度' },
]

const typeLabels = {
  'knowledge-base': '知识库',
  directory: '目录',
  file: '文件',
  section: '章节',
  paragraph: '段落',
  concept: '概念',
}
const typeLabel = (t) => typeLabels[t] || t || ''
const tagType = (t) => ({
  'knowledge-base': 'warning',
  directory: 'info',
  file: 'success',
  section: 'primary',
  paragraph: 'default',
  concept: 'error',
}[t] || 'default')

const canOpen = computed(() => {
  const n = node.value
  if (!n) return false
  const path = n.file_path || n.abs_path || n.rel_path || n.path
  return !!path && ['file', 'section', 'paragraph', 'concept'].includes(n.type)
})

const emitAction = (type, payload = null) => {
  store.requestKg3dAction(type, payload)
}

const addTag = () => {
  const t = newTag.value.trim()
  if (!t) return
  emitAction('addTag', { tag: t })
  newTag.value = ''
}
</script>

<style scoped>
.kg3d-ops {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 8px;
  height: 100%;
  overflow: auto;
  box-sizing: border-box;
}
.ops-card { flex: 0 0 auto; }
.ops-head { display: flex; align-items: center; gap: 6px; }
.ops-name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ops-path { font-size: 12px; word-break: break-all; }
.ops-section { margin-top: 10px; }
.ops-sub { font-size: 12px; opacity: 0.65; margin-bottom: 4px; }
.ops-tags { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 6px; }
.ops-empty { font-size: 12px; opacity: 0.5; }
.ops-add { display: flex; gap: 4px; align-items: center; }
.ops-views { display: flex; flex-wrap: wrap; gap: 4px; }
.ops-rel-list { margin-top: 6px; display: flex; flex-direction: column; gap: 2px; max-height: 220px; overflow: auto; }
.ops-rel { display: flex; align-items: center; gap: 6px; }
.ops-rel-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; cursor: pointer; }
.ops-rel-name:hover { color: #58a6ff; text-decoration: underline; }
.ops-rel-type { font-size: 11px; opacity: 0.6; }
</style>
