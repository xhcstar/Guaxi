<template>
  <div class="kg3d-left">
    <!-- 概览 / 统计 -->
    <n-collapse :default-expanded-names="['overview', 'community']" accordion-disabled>
      <n-collapse-item title="概览 / 统计" name="overview">
        <div v-if="stats" class="kl-stats">
          <div class="kl-row"><span>节点</span><b>{{ stats.total_nodes ?? totalNodes }}</b></div>
          <div class="kl-row"><span>边</span><b>{{ stats.total_edges ?? totalEdges }}</b></div>
          <div class="kl-sub">类型分布</div>
          <div v-for="t in (stats.node_types || [])" :key="t.key" class="kl-bar">
            <span class="kl-bar-label">{{ typeLabel(t.key) }}</span>
            <div class="kl-bar-track"><div class="kl-bar-fill" :style="barStyle(t.count)"></div></div>
            <span class="kl-bar-num">{{ t.count }}</span>
          </div>
          <div class="kl-sub">核心节点(PageRank)</div>
          <div
            v-for="n in (stats.top_nodes || []).slice(0, 8)"
            :key="n.id"
            class="kl-top"
            @click="select(n)"
          >
            <span class="kl-top-name" :title="n.name">{{ n.name }}</span>
            <span class="kl-top-pr">{{ (n.pagerank || 0).toFixed(3) }}</span>
          </div>
        </div>
        <div v-else class="kl-empty">
          <n-button size="tiny" :loading="loadingStats" @click="loadStats">加载统计</n-button>
        </div>
      </n-collapse-item>

      <!-- 社区 / 分类导航 -->
      <n-collapse-item title="社区 / 分类" name="community">
        <div v-if="communities.length" class="kl-comms">
          <div
            v-for="c in communities"
            :key="c.community_id"
            class="kl-comm"
            @click="focusCommunity(c.community_id)"
          >
            <span class="kl-dot" :style="{ background: c.color || '#888' }"></span>
            <span class="kl-comm-label" :title="c.label">{{ c.label || ('社区 ' + c.community_id) }}</span>
            <span class="kl-comm-size">{{ c.size }}</span>
          </div>
        </div>
        <div v-else class="kl-empty">
          <n-button size="tiny" :loading="loadingComm" @click="loadCommunities">加载社区</n-button>
        </div>
        <div v-if="tagCloud.length" class="kl-tagcloud">
          <div class="kl-sub">标签</div>
          <n-tag
            v-for="t in tagCloud"
            :key="t.key"
            size="small"
            class="kl-tag"
            @click="switchTagView"
          >#{{ t.key }} <i>{{ t.count }}</i></n-tag>
        </div>
      </n-collapse-item>

      <!-- 历史 / 书签 -->
      <n-collapse-item title="历史 / 书签" name="history">
        <div class="kl-sub">收藏</div>
        <div v-if="bookmarks.length" class="kl-list">
          <div v-for="b in bookmarks" :key="b.id" class="kl-item" @click="select(b)">
            <span class="kl-item-name" :title="b.name">★ {{ b.name }}</span>
          </div>
        </div>
        <div v-else class="kl-empty-text">暂无收藏</div>

        <div class="kl-sub" style="margin-top:8px;">最近访问</div>
        <div v-if="history.length" class="kl-list">
          <div v-for="h in history.slice(0, 12)" :key="h.id + h.ts" class="kl-item" @click="select(h)">
            <span class="kl-item-name" :title="h.name">{{ h.name }}</span>
          </div>
        </div>
        <div v-else class="kl-empty-text">暂无历史</div>
      </n-collapse-item>
    </n-collapse>
  </div>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { NButton, NCollapse, NCollapseItem, NTag } from 'naive-ui'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { onKgIndexChanged } from '../../utils/kgIndexEvents'

const store = useKnowledgeBaseStore()
const { kg3dHistory, kg3dBookmarks } = storeToRefs(store)

const stats = ref(null)
const communities = ref([])
const loadingStats = ref(false)
const loadingComm = ref(false)

const history = computed(() => kg3dHistory.value)
const bookmarks = computed(() => kg3dBookmarks.value)
const totalNodes = computed(() => store.graphMeta?.totalNodes || 0)
const totalEdges = computed(() => (store.graphEdges || []).length)
const tagCloud = computed(() => (stats.value?.tags || []).slice(0, 16))

const kbName = computed(() => store.selectedKnowledgeBase?.knowledge_base_name || '')

const typeLabels = {
  'knowledge-base': '知识库', directory: '目录', file: '文件',
  section: '章节', paragraph: '段落', concept: '概念',
}
const typeLabel = (t) => typeLabels[t] || t || ''

const maxTypeCount = computed(() => {
  const arr = stats.value?.node_types || []
  return arr.reduce((m, x) => Math.max(m, x.count || 0), 1)
})
const barStyle = (c) => ({ width: `${Math.round((c / maxTypeCount.value) * 100)}%` })

const loadStats = async () => {
  if (!kbName.value) return
  loadingStats.value = true
  try {
    stats.value = await store.kgStats(kbName.value, 12)
  } catch (e) {
    /* ignore */
  } finally {
    loadingStats.value = false
  }
}

const loadCommunities = async () => {
  if (!kbName.value) return
  loadingComm.value = true
  try {
    const list = await store.kgCommunities(kbName.value)
    communities.value = (list || []).sort((a, b) => (b.size || 0) - (a.size || 0))
  } catch (e) {
    /* ignore */
  } finally {
    loadingComm.value = false
  }
}

const select = (n) => store.requestKg3dAction('select', { id: n.id })
const focusCommunity = (cid) => store.requestKg3dAction('focusCommunity', { community_id: cid })
const switchTagView = () => store.requestKg3dAction('switchView', { view: 'tag' })

// 切换知识库时清空已加载的派生数据
watch(kbName, () => {
  stats.value = null
  communities.value = []
})

// 索引变更(新建/删除文件等)后概览统计会过时:已展开的重新拉一次,未展开的清掉待下次按需加载。
let unlistenIndex = null
onMounted(async () => {
  try {
    unlistenIndex = await onKgIndexChanged(({ kbName: changed }) => {
      if (!kbName.value || changed !== kbName.value) return
      const hadStats = !!stats.value
      const hadComm = communities.value.length > 0
      stats.value = null
      communities.value = []
      if (hadStats) loadStats()
      if (hadComm) loadCommunities()
    }, { debounceMs: 1200 })
  } catch (e) {
    /* 事件不可用时退化为手动刷新 */
  }
})
onUnmounted(() => {
  if (unlistenIndex) { try { unlistenIndex() } catch (e) { /* noop */ } }
})
</script>

<style scoped>
.kg3d-left {
  padding: 6px 4px;
  font-size: 13px;
}
.kl-stats { display: flex; flex-direction: column; gap: 4px; }
.kl-row { display: flex; justify-content: space-between; font-size: 12px; }
.kl-sub { font-size: 11px; opacity: 0.6; margin: 6px 0 2px; }
.kl-bar { display: flex; align-items: center; gap: 6px; }
.kl-bar-label { width: 38px; font-size: 11px; opacity: 0.8; }
.kl-bar-track { flex: 1; height: 8px; background: rgba(127,127,127,0.18); border-radius: 4px; overflow: hidden; }
.kl-bar-fill { height: 100%; background: #3b82f6; border-radius: 4px; }
.kl-bar-num { width: 28px; text-align: right; font-size: 11px; opacity: 0.7; }
.kl-top, .kl-comm, .kl-item {
  display: flex; align-items: center; gap: 6px;
  padding: 3px 4px; border-radius: 5px; cursor: pointer;
}
.kl-top:hover, .kl-comm:hover, .kl-item:hover { background: rgba(127,127,127,0.14); }
.kl-top-name, .kl-comm-label, .kl-item-name {
  flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px;
}
.kl-top-pr, .kl-comm-size { font-size: 11px; opacity: 0.6; }
.kl-dot { width: 10px; height: 10px; border-radius: 50%; flex: 0 0 auto; }
.kl-tagcloud { margin-top: 6px; }
.kl-tag { cursor: pointer; margin: 2px; }
.kl-tag i { opacity: 0.6; font-style: normal; font-size: 10px; }
.kl-empty, .kl-empty-text { font-size: 12px; opacity: 0.55; padding: 4px 0; }
</style>
