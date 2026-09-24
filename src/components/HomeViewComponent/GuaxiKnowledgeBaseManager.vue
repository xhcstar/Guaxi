<template>
  <n-message-provider placement="bottom">
    <div class="page">
      <!-- 顶部工具栏 -->
      <div class="header">
        <div class="header-left">
          <div class="title">知识库管理器</div>
          <n-select
            v-model:value="selectedKbName"
            size="small"
            filterable
            clearable
            placeholder="选择知识库"
            :options="kbOptions"
            :disabled="loading"
            style="width: 240px"
            @update:value="handleKbChange"
          />
          <n-text v-if="graphMeta.returned" depth="3" class="meta-hint">
            {{ graphMeta.returned }} / {{ graphMeta.total }} 节点{{ graphMeta.truncated ? '(已截断)' : '' }}
          </n-text>
        </div>

        <n-space size="small" align="center">
          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="refresh">
                <n-icon :component="RefreshOutline" size="18" />
              </n-icon-wrapper>
            </template>
            刷新
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="rebuildIndex">
                <n-icon :component="HammerOutline" size="18" />
              </n-icon-wrapper>
            </template>
            重建图索引
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="embedKb">
                <n-icon :component="SparklesOutline" size="18" />
              </n-icon-wrapper>
            </template>
            生成 / 更新向量嵌入
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="detectCommunities">
                <n-icon :component="GitNetworkOutline" size="18" />
              </n-icon-wrapper>
            </template>
            社区检测
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="extractConcepts">
                <n-icon :component="BulbOutline" size="18" />
              </n-icon-wrapper>
            </template>
            LLM 概念抽取
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="openKbPluginMenu($event)">
                <n-icon :component="ExtensionPuzzleOutline" size="18" />
              </n-icon-wrapper>
            </template>
            运行知识库插件
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper
                :size="28"
                :border-radius="8"
                class="header-icon"
                :class="{ active: syncing }"
                @click="syncNow"
              >
                <n-icon :component="SyncOutline" size="18" />
              </n-icon-wrapper>
            </template>
            多机同步(.md + 人工知识层)
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-switch v-model:value="manualSyncEnabled" size="small" @update:value="toggleManualSync">
                <template #checked>精细</template>
                <template #unchecked>精细</template>
              </n-switch>
            </template>
            机制B:行级精细同步(并发编辑更友好;删除不传播)
          </n-tooltip>
        </n-space>
      </div>

      <div v-if="busyText" class="busy-bar">
        <n-spin :size="14" />
        <span class="busy-text">{{ busyText }}</span>
      </div>

      <div class="main">
        <!-- 左:节点列表 + 语义检索 -->
        <div class="left">
          <div class="left-toolbar">
            <n-input
              v-model:value="search"
              size="small"
              clearable
              placeholder="按名称过滤 / 回车语义检索"
              @keyup.enter="semanticSearch"
            >
              <template #prefix><n-icon :component="SearchOutline" /></template>
            </n-input>
            <n-select
              v-model:value="typeFilter"
              size="small"
              multiple
              :options="typeOptions"
              placeholder="类型"
              style="margin-top: 8px"
            />
          </div>

          <n-scrollbar class="node-scroll">
            <div v-if="semanticResults.length" class="section-label">语义检索结果</div>
            <div
              v-for="n in semanticResults"
              :key="'s-' + n.id"
              class="node-row"
              :class="{ selected: selectedNode && selectedNode.id === n.id }"
              @click="selectNode(n)"
            >
              <span class="node-type" :data-t="n.type">{{ typeLabel(n.type) }}</span>
              <span class="node-name">{{ n.name || n.id }}</span>
              <span class="node-score">{{ (n.score || 0).toFixed(2) }}</span>
            </div>

            <div v-if="semanticResults.length" class="section-label">全部节点</div>
            <div
              v-for="n in filteredNodes"
              :key="n.id"
              class="node-row"
              :class="{ selected: selectedNode && selectedNode.id === n.id }"
              @click="selectNode(n)"
            >
              <span class="node-type" :data-t="n.type">{{ typeLabel(n.type) }}</span>
              <span class="node-name">{{ n.name || n.id }}</span>
              <span v-if="n.tags && n.tags.length" class="node-tagcount">#{{ n.tags.length }}</span>
            </div>

            <div v-if="!filteredNodes.length && !semanticResults.length" class="empty">
              {{ selectedKbName ? '无节点,试试重建图索引' : '请选择一个知识库' }}
            </div>
          </n-scrollbar>
        </div>

        <!-- 右:选中节点编辑 -->
        <div class="right">
          <div v-if="!selectedNode" class="empty big">在左侧选择一个知识点进行编辑</div>

          <template v-else>
            <div class="editor-head">
              <span class="node-type" :data-t="selectedNode.type">{{ typeLabel(selectedNode.type) }}</span>
              <span class="editor-title">{{ selectedNode.name || selectedNode.id }}</span>
            </div>
            <div class="node-meta">
              <span v-if="selectedNode.rel_path">路径:{{ selectedNode.rel_path }}</span>
              <span>度:{{ selectedNode.degree ?? 0 }}</span>
              <span>PageRank:{{ (selectedNode.pagerank || 0).toFixed(4) }}</span>
              <span v-if="selectedNode.community_id != null">社区:{{ selectedNode.community_id }}</span>
            </div>

            <n-scrollbar class="editor-scroll">
              <!-- 标签 -->
              <div class="block">
                <div class="block-head">
                  <span class="block-title">标签</span>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="loadTagSuggestions" :loading="tagSugLoading">
                        <template #icon><n-icon :component="BulbOutline" /></template>
                      </n-button>
                    </template>
                    推荐相关标签
                  </n-tooltip>
                </div>
                <n-space size="small" :wrap="true">
                  <n-tag
                    v-for="t in selectedTags"
                    :key="t"
                    closable
                    size="medium"
                    @close="removeTag(t)"
                  >{{ t }}</n-tag>
                  <n-input
                    v-model:value="newTag"
                    size="small"
                    placeholder="新标签↵"
                    style="width: 140px"
                    @keyup.enter="addTag"
                  />
                </n-space>
                <div v-if="tagSuggestions.length" class="suggest-row">
                  <span class="suggest-label">建议:</span>
                  <n-tag
                    v-for="s in tagSuggestions"
                    :key="'ts-' + s.tag"
                    size="small"
                    type="info"
                    class="suggest-tag"
                    @click="acceptTagSuggestion(s)"
                  >+ {{ s.tag }} ({{ s.score.toFixed(2) }})</n-tag>
                </div>
              </div>

              <!-- 关系 -->
              <div class="block">
                <div class="block-head">
                  <span class="block-title">关系</span>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button text @click="loadRelationSuggestions" :loading="relSugLoading">
                        <template #icon><n-icon :component="BulbOutline" /></template>
                      </n-button>
                    </template>
                    推荐相关关系
                  </n-tooltip>
                </div>

                <div v-for="(e, i) in selectedEdges" :key="'e-' + i" class="edge-row">
                  <span class="edge-dir">{{ e.dir }}</span>
                  <span class="edge-type">{{ edgeTypeLabel(e.type) }}</span>
                  <span class="edge-target">{{ e.otherName }}</span>
                  <span class="edge-source">{{ sourceLabel(e.source) }}</span>
                  <n-tooltip v-if="e.removable" trigger="hover">
                    <template #trigger>
                      <n-button text type="error" @click="removeRelation(e)">
                        <template #icon><n-icon :component="TrashOutline" /></template>
                      </n-button>
                    </template>
                    删除该关系
                  </n-tooltip>
                </div>
                <div v-if="!selectedEdges.length" class="muted">暂无关系</div>

                <div class="add-relation">
                  <n-select
                    v-model:value="relationTarget"
                    size="small"
                    filterable
                    clearable
                    placeholder="选择目标节点新建关系"
                    :options="relationTargetOptions"
                    style="flex: 1"
                  />
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-select
                        v-model:value="relationType"
                        size="small"
                        filterable
                        tag
                        :options="relationTypeOptions"
                        placeholder="关系类型"
                        style="width: 130px"
                      />
                    </template>
                    选择内置关系类型,或直接输入自定义类型
                  </n-tooltip>
                  <n-tooltip trigger="hover">
                    <template #trigger>
                      <n-button size="small" type="primary" :disabled="!relationTarget" @click="confirmRelation">
                        <template #icon><n-icon :component="AddOutline" /></template>
                      </n-button>
                    </template>
                    新建关系
                  </n-tooltip>
                </div>

                <div v-if="relationSuggestions.length" class="suggest-row">
                  <span class="suggest-label">建议:</span>
                  <n-tag
                    v-for="s in relationSuggestions"
                    :key="'rs-' + s.to"
                    size="small"
                    type="info"
                    class="suggest-tag"
                    @click="acceptRelationSuggestion(s)"
                  >+ {{ s.to_name }} ({{ s.score.toFixed(2) }})</n-tag>
                </div>
              </div>
            </n-scrollbar>
          </template>
        </div>
      </div>
    </div>

    <n-dropdown trigger="manual" :show="kbPluginMenuVisible" placement="bottom-start"
      :options="kbPluginMenuOptions" :x="kbPluginMenuPosition.x" :y="kbPluginMenuPosition.y"
      @select="handleKbPluginMenuSelect" @clickoutside="kbPluginMenuVisible = false" />

    <n-modal v-model:show="kbReportModalVisible" preset="card" :title="kbReportTitle || '插件结果'"
      style="width: 640px; max-width: 92vw;">
      <div class="kb-plugin-report" v-html="kbReportHtml" />
    </n-modal>
  </n-message-provider>
</template>

<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import {
  NButton, NIcon, NIconWrapper, NInput, NScrollbar, NSelect, NSpace, NSpin,
  NSwitch, NTag, NText, NTooltip, NMessageProvider, createDiscreteApi,
  NModal, NDropdown,
} from 'naive-ui'
import {
  RefreshOutline, SyncOutline, SearchOutline, HammerOutline, SparklesOutline,
  GitNetworkOutline, BulbOutline, AddOutline, TrashOutline, ExtensionPuzzleOutline,
} from '@vicons/ionicons5'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'
import { createLogger } from '../../utils/logger'
import { onKgIndexChanged } from '../../utils/kgIndexEvents'
import { listPluginsForSurface, runGuaxiPlugin } from '../../utils/guaxiPluginRunner'
import { renderMarkdown } from '../GuaxiMarkdownRender/GuaxiMarker'

const logger = createLogger('GuaxiKnowledgeBaseManager')
const { message } = createDiscreteApi(['message'])
const store = useKnowledgeBaseStore()

const STRUCTURAL_EDGES = ['contains', 'next']
const TYPE_LABELS = {
  'knowledge-base': '知识库', directory: '目录', file: '文件',
  section: '章节', paragraph: '段落', concept: '概念',
}
const typeLabel = (t) => TYPE_LABELS[t] || t
const typeOptions = Object.entries(TYPE_LABELS).map(([value, label]) => ({ label, value }))

// 内置关系类型(值为存储用英文,标签为中文);用户也可在下拉框直接输入自定义类型。
const RELATION_TYPES = [
  { value: 'related', label: '相关' },
  { value: 'prerequisite', label: '前置(先修)' },
  { value: 'elaborates', label: '阐述' },
  { value: 'contradicts', label: '矛盾' },
  { value: 'part-of', label: '部分' },
  { value: 'similar', label: '相似' },
  { value: 'mention', label: '提及' },
]
const relationTypeOptions = RELATION_TYPES.map((r) => ({ label: r.label, value: r.value }))

const EDGE_TYPE_LABELS = {
  contains: '包含', next: '后续', link: '链接', mention: '提及', tag: '标签',
  related: '相关', prerequisite: '前置', elaborates: '阐述', contradicts: '矛盾',
  'part-of': '部分', similar: '相似',
}
const edgeTypeLabel = (t) => EDGE_TYPE_LABELS[t] || t

const SOURCE_LABELS = {
  manual: '手动', llm: 'AI', semantic: '语义', structural: '结构',
  wikilink: '链接', mdlink: '链接', inline: '行内', zeroshot: '推断', related: '相关',
}
const sourceLabel = (s) => SOURCE_LABELS[s] || s

const loading = ref(false)
const busyText = ref('')
const syncing = ref(false)
const manualSyncEnabled = ref(false)

const selectedKbName = ref(null)
const kbOptions = ref([])

// —— 知识库插件(knowledge-base 作用面)——
const kbPluginRunning = ref(false)
const kbPluginMenuVisible = ref(false)
const kbPluginMenuOptions = ref([])
const kbPluginMenuPosition = ref({ x: 0, y: 0 })
let kbPluginCache = []
const kbReportModalVisible = ref(false)
const kbReportTitle = ref('')
const kbReportHtml = ref('')

const openKbPluginMenu = async (event) => {
  if (!selectedKbName.value) {
    message.warning('请先选择一个知识库')
    return
  }
  let plugins = []
  try {
    plugins = await listPluginsForSurface('knowledge-base')
  } catch (e) {
    message.error('读取插件失败:' + String(e?.message || e))
    return
  }
  if (!plugins.length) {
    message.info('没有已启用的知识库插件(可在设置→插件管理里新建)')
    return
  }
  kbPluginCache = plugins
  if (plugins.length === 1) {
    runKbPlugin(plugins[0])
    return
  }
  kbPluginMenuOptions.value = plugins.map((p) => ({ label: p.name, key: p.id }))
  kbPluginMenuPosition.value = { x: Number(event?.clientX || 200), y: Number(event?.clientY || 200) }
  kbPluginMenuVisible.value = true
}

const handleKbPluginMenuSelect = (id) => {
  kbPluginMenuVisible.value = false
  const p = kbPluginCache.find((x) => x.id === id)
  if (p) runKbPlugin(p)
}

const runKbPlugin = async (plugin) => {
  if (kbPluginRunning.value) return
  const kb = store.knowledgeBases.find((k) => k.knowledge_base_name === selectedKbName.value)
  const ctx = {
    kb_name: kb?.knowledge_base_name || selectedKbName.value || '',
    kb_location: kb?.location || '',
  }
  kbPluginRunning.value = true
  try {
    const rj = await runGuaxiPlugin(plugin.entry_path, ctx)
    const md =
      typeof rj?.report === 'string'
        ? rj.report
        : typeof rj?.text === 'string'
          ? rj.text
          : JSON.stringify(rj, null, 2)
    kbReportTitle.value = plugin.name
    kbReportHtml.value = await renderMarkdown(md, { buttons: false, mermaid: true, math: true })
    kbReportModalVisible.value = true
  } catch (e) {
    message.error('插件运行失败:' + String(e?.message || e))
  } finally {
    kbPluginRunning.value = false
  }
}

const nodes = ref([])
const edges = ref([])
const graphMeta = ref({ total: 0, returned: 0, truncated: false })
const nodeMap = computed(() => {
  const m = new Map()
  for (const n of nodes.value) m.set(n.id, n)
  return m
})

const search = ref('')
const typeFilter = ref([])
const semanticResults = ref([])

const selectedNode = ref(null)
const newTag = ref('')
const tagSuggestions = ref([])
const tagSugLoading = ref(false)
const relationSuggestions = ref([])
const relSugLoading = ref(false)
const relationTarget = ref(null)
const relationType = ref('related')

const filteredNodes = computed(() => {
  const q = search.value.trim().toLowerCase()
  const types = typeFilter.value
  return nodes.value.filter((n) => {
    if (types.length && !types.includes(n.type)) return false
    if (q && !(n.name || '').toLowerCase().includes(q) && !(n.id || '').toLowerCase().includes(q)) return false
    return true
  }).slice(0, 800)
})

const selectedTags = computed(() => (selectedNode.value?.tags) || [])

const selectedEdges = computed(() => {
  const node = selectedNode.value
  if (!node) return []
  const out = []
  for (const e of edges.value) {
    let other = null
    let dir = ''
    if (e.from === node.id) { other = e.to; dir = '→' }
    else if (e.to === node.id) { other = e.from; dir = '←' }
    else continue
    const on = nodeMap.value.get(other)
    const removable = !STRUCTURAL_EDGES.includes(e.type) && e.source !== 'structural'
    out.push({
      from: e.from, to: e.to, type: e.type, source: e.source, dir,
      otherName: on ? (on.name || other) : other, removable,
    })
  }
  return out
})

const relationTargetOptions = computed(() => {
  const cur = selectedNode.value?.id
  return nodes.value
    .filter((n) => n.id !== cur)
    .slice(0, 1000)
    .map((n) => ({ label: `${typeLabel(n.type)} · ${n.name || n.id}`, value: n.id }))
})

async function refresh() {
  loading.value = true
  try {
    const list = await store.fetchKnowledgeBases()
    const arr = Array.isArray(list) ? list : store.knowledgeBases
    kbOptions.value = (arr || []).map((kb) => ({
      label: kb.knowledge_base_name, value: kb.knowledge_base_name,
    }))
    if (selectedKbName.value) await loadGraph()
  } catch (e) {
    logger.error('refresh failed', e)
    message.error('刷新失败')
  } finally {
    loading.value = false
  }
}

async function handleKbChange(name) {
  selectedKbName.value = name
  selectedNode.value = null
  semanticResults.value = []
  if (name) await loadGraph()
  else { nodes.value = []; edges.value = [] }
}

// 设计说明:必须显式给全深度/大上限。后端 kg_query_graph 的默认是 max_depth=2、limit=3000,
// 传 null 会让二级目录以下新建的文件永远进不了节点列表(即使点刷新)。
const FULL_GRAPH_OPTS = { max_depth: 99, limit: 100000 }

async function loadGraph() {
  if (!selectedKbName.value) return
  loading.value = true
  busyText.value = '加载知识图谱…'
  try {
    const data = await store.kgQueryGraph(selectedKbName.value, FULL_GRAPH_OPTS)
    nodes.value = data?.nodes || []
    edges.value = data?.edges || []
    graphMeta.value = {
      total: data?.total_nodes || 0,
      returned: data?.returned_nodes || nodes.value.length,
      truncated: !!data?.truncated,
    }
    if (selectedNode.value) {
      selectedNode.value = nodeMap.value.get(selectedNode.value.id) || null
    }
  } catch (e) {
    logger.error('loadGraph failed', e)
    message.error('加载图谱失败:' + (e?.message || e))
  } finally {
    loading.value = false
    busyText.value = ''
  }
}

function selectNode(n) {
  selectedNode.value = nodeMap.value.get(n.id) || n
  tagSuggestions.value = []
  relationSuggestions.value = []
  relationTarget.value = null
  relationType.value = 'related'
}

async function withBusy(text, fn) {
  busyText.value = text
  try {
    return await fn()
  } catch (e) {
    logger.error(text + ' failed', e)
    message.error(text + '失败:' + (e?.message || e))
  } finally {
    busyText.value = ''
  }
}

// ───── 标签编辑 ─────
async function addTag() {
  const tag = newTag.value.trim()
  const node = selectedNode.value
  if (!tag || !node) return
  await withBusy('添加标签', async () => {
    await store.kgAddTag(selectedKbName.value, node.id, tag, 'manual', 1.0)
    if (!node.tags) node.tags = []
    if (!node.tags.includes(tag)) node.tags.push(tag)
    newTag.value = ''
  })
}

async function removeTag(tag) {
  const node = selectedNode.value
  if (!node) return
  await withBusy('删除标签', async () => {
    await store.kgRemoveTag(selectedKbName.value, node.id, tag)
    node.tags = (node.tags || []).filter((t) => t !== tag)
  })
}

async function loadTagSuggestions() {
  const node = selectedNode.value
  if (!node) return
  tagSugLoading.value = true
  try {
    const list = await store.kgSuggestTags(selectedKbName.value, node.id)
    const have = new Set(node.tags || [])
    tagSuggestions.value = (list || []).filter((s) => !have.has(s.tag))
  } catch (e) {
    logger.error('suggest tags failed', e)
    message.error('标签建议失败(需先生成向量嵌入)')
  } finally {
    tagSugLoading.value = false
  }
}

async function acceptTagSuggestion(s) {
  newTag.value = s.tag
  await addTag()
  tagSuggestions.value = tagSuggestions.value.filter((x) => x.tag !== s.tag)
}

// ───── 关系编辑 ─────
async function confirmRelation() {
  const node = selectedNode.value
  const to = relationTarget.value
  if (!node || !to) return
  const rel = String(relationType.value || 'related').trim() || 'related'
  await withBusy('新建关系', async () => {
    await store.kgConfirmRelation(selectedKbName.value, node.id, to, rel, 0.5)
    edges.value.push({ from: node.id, to, type: rel, strength: 0.5, source: 'manual' })
    relationTarget.value = null
  })
}

async function removeRelation(e) {
  await withBusy('删除关系', async () => {
    await store.kgRemoveRelation(selectedKbName.value, e.from, e.to, e.type)
    edges.value = edges.value.filter(
      (x) => !(x.from === e.from && x.to === e.to && x.type === e.type),
    )
  })
}

async function loadRelationSuggestions() {
  const node = selectedNode.value
  if (!node) return
  relSugLoading.value = true
  try {
    const list = await store.kgSuggestRelations(selectedKbName.value, node.id)
    relationSuggestions.value = list || []
  } catch (e) {
    logger.error('suggest relations failed', e)
    message.error('关系建议失败(需先生成向量嵌入)')
  } finally {
    relSugLoading.value = false
  }
}

async function acceptRelationSuggestion(s) {
  const node = selectedNode.value
  if (!node) return
  await withBusy('采纳建议关系', async () => {
    await store.kgConfirmRelation(selectedKbName.value, node.id, s.to, 'related', s.score)
    edges.value.push({ from: node.id, to: s.to, type: 'related', strength: s.score, source: 'manual' })
    relationSuggestions.value = relationSuggestions.value.filter((x) => x.to !== s.to)
  })
}

// ───── 语义检索 ─────
async function semanticSearch() {
  const q = search.value.trim()
  if (!q || !selectedKbName.value) { semanticResults.value = []; return }
  await withBusy('语义检索', async () => {
    const list = await store.kgSemanticSearch(selectedKbName.value, q, 20)
    semanticResults.value = list || []
  })
}

// ───── 处理动作 ─────
async function rebuildIndex() {
  if (!selectedKbName.value) return
  await withBusy('重建图索引', async () => {
    await store.kgEnsureIndex(selectedKbName.value)
    await loadGraph()
    message.success('图索引已重建')
  })
}

async function embedKb() {
  if (!selectedKbName.value) return
  await withBusy('生成向量嵌入(首次较慢)', async () => {
    const r = await store.kgEmbedKb(selectedKbName.value)
    message.success('嵌入完成:' + (r?.embedded_count ?? '') + ' 节点')
  })
}

async function detectCommunities() {
  if (!selectedKbName.value) return
  await withBusy('社区检测', async () => {
    await store.kgDetectCommunities(selectedKbName.value)
    await loadGraph()
    message.success('社区检测完成')
  })
}

async function extractConcepts() {
  if (!selectedKbName.value) return
  await withBusy('LLM 概念抽取(较慢)', async () => {
    const r = await store.kgExtractConcepts(selectedKbName.value)
    await loadGraph()
    message.success(`概念抽取:${r?.concepts ?? 0} 概念 / ${r?.relations ?? 0} 关系`)
  })
}

// ───── 多机同步(机制A:文件同步 + 人工层导入)─────
let unlistenSync = null
async function syncNow() {
  if (syncing.value) return
  syncing.value = true
  busyText.value = '正在与其它设备同步…'
  try {
    await invoke('p2p_sync_knowledge_base_now')
    // 文件落地后,把各知识库的人工层快照导入本机 DB(整层替换,天然携带删除)
    await importAllManual()
    if (selectedKbName.value) await loadGraph()
    message.success('同步完成')
  } catch (e) {
    logger.error('sync failed', e)
    message.error('同步失败:' + (e?.message || e))
  } finally {
    syncing.value = false
    busyText.value = ''
  }
}

async function toggleManualSync(val) {
  try {
    await store.kgSetManualSync(val)
    message.success(val ? '已开启行级精细同步' : '已关闭行级精细同步')
  } catch (e) {
    manualSyncEnabled.value = !val
    message.error('切换失败:' + (e?.message || e))
  }
}

async function importAllManual() {
  const names = kbOptions.value.map((o) => o.value)
  for (const name of names) {
    try {
      await store.kgImportManual(name, null)
    } catch (e) {
      logger.warn('import manual failed for ' + name, e)
    }
  }
}

// ───── 索引变更自动跟随 ─────
// 主界面在知识库里新建/删除/移动文件后,后端把索引标脏并广播 kg_index_changed;
// 本窗口据此自动重查,无需用户手动点刷新。
let unlistenIndex = null
async function handleIndexChanged({ kbName, reason }) {
  const known = kbOptions.value.some((o) => o.value === kbName)
  if (reason === 'removed' || !known) {
    logger.info('知识库索引变更,刷新知识库列表', { kbName, reason })
    // 被删的正是当前选中库:先清空,免得 refresh 里对已消失的库查图报错。
    if (reason === 'removed' && kbName === selectedKbName.value) {
      selectedKbName.value = null
      selectedNode.value = null
      nodes.value = []
      edges.value = []
      semanticResults.value = []
      graphMeta.value = { total: 0, returned: 0, truncated: false }
    }
    await refresh()
    return
  }
  if (kbName !== selectedKbName.value) return
  logger.info('知识库索引变更,重新加载图谱', { kbName, reason })
  await loadGraph()
}

onMounted(async () => {
  await refresh()
  try {
    manualSyncEnabled.value = await store.kgGetManualSync()
  } catch (e) {
    logger.warn('get manual sync flag failed', e)
  }
  try {
    unlistenIndex = await onKgIndexChanged(handleIndexChanged)
  } catch (e) {
    logger.warn('listen kg index changed failed', e)
  }
  try {
    unlistenSync = await listen('resource_repo_sync_progress', (ev) => {
      const p = ev.payload || {}
      if (p.target && String(p.target).includes('knowledge') && p.message) {
        busyText.value = `同步:${p.message}`
      }
    })
  } catch (e) {
    logger.warn('listen sync progress failed', e)
  }
})

onUnmounted(() => {
  if (unlistenSync) { try { unlistenSync() } catch (_) { /* noop */ } }
  if (unlistenIndex) { try { unlistenIndex() } catch (_) { /* noop */ } }
})
</script>

<style scoped>
.page {
  display: flex;
  flex-direction: column;
  height: 100vh;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei';
  background: var(--n-color, #1e1e1e);
}
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid var(--n-divider-color, #333);
  gap: 12px;
}
.header-left { display: flex; align-items: center; gap: 12px; }
.title { font-size: 15px; font-weight: 600; }
.meta-hint { font-size: 12px; }
.header-icon { cursor: pointer; display: flex; align-items: center; justify-content: center; }
.header-icon.active { outline: 2px solid var(--n-primary-color, #63e2b7); }

.busy-bar { display: flex; align-items: center; gap: 8px; padding: 4px 16px; font-size: 12px; opacity: 0.85; }
.busy-text { opacity: 0.8; }

.main { flex: 1; display: flex; min-height: 0; }
.left {
  width: 320px;
  border-right: 1px solid var(--n-divider-color, #333);
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.left-toolbar { padding: 10px; }
.node-scroll { flex: 1; min-height: 0; }
.right { flex: 1; display: flex; flex-direction: column; min-height: 0; padding: 12px 16px; }

.section-label { padding: 9px 10px 5px; font-size: 13px; opacity: 0.55; }
.node-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  cursor: pointer;
  font-size: 15px;
  border-radius: 6px;
}
.node-row:hover { background: var(--n-color-hover, rgba(255,255,255,0.06)); }
.node-row.selected { background: var(--n-primary-color-hover, rgba(99,226,183,0.18)); }
.node-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.node-type {
  font-size: 12px; padding: 2px 6px; border-radius: 4px;
  background: rgba(125,125,125,0.25); white-space: nowrap;
}
.node-type[data-t='concept'] { background: rgba(240,180,80,0.3); }
.node-type[data-t='file'] { background: rgba(99,160,226,0.3); }
.node-type[data-t='section'] { background: rgba(150,99,226,0.3); }
.node-score, .node-tagcount { font-size: 13px; opacity: 0.6; }

.editor-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.editor-title { font-size: 18px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.node-meta { display: flex; flex-wrap: wrap; gap: 14px; font-size: 14px; opacity: 0.65; margin-bottom: 12px; }
.editor-scroll { flex: 1; min-height: 0; }

.block { border: 1px solid var(--n-divider-color, #333); border-radius: 8px; padding: 14px; margin-bottom: 14px; }
.block-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.block-title { font-size: 16px; font-weight: 600; }

.edge-row { display: flex; align-items: center; gap: 8px; padding: 6px 0; font-size: 15px; }
.edge-dir { opacity: 0.6; }
.edge-type { color: var(--n-primary-color, #63e2b7); }
.edge-target { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.edge-source { font-size: 12px; opacity: 0.5; }
.add-relation { display: flex; gap: 6px; align-items: center; margin-top: 12px; }

.suggest-row { margin-top: 10px; display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.suggest-label { font-size: 13px; opacity: 0.55; }
.suggest-tag { cursor: pointer; font-size: 13px; }

.muted { font-size: 14px; opacity: 0.5; }
.empty { padding: 20px; text-align: center; opacity: 0.55; font-size: 15px; }
.empty.big { margin: auto; }
</style>
