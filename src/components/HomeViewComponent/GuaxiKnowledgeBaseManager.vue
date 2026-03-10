<template>
  <n-message-provider placement="bottom">
    <div class="page">
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
        </div>
        <n-space size="small" align="center">
          <n-tooltip v-if="viewMode === 'graph'" trigger="hover">
            <template #trigger>
              <n-icon-wrapper
                :size="28"
                :border-radius="8"
                class="header-icon"
                :class="{ active: !graphLeftVisible }"
                @click="graphLeftVisible = !graphLeftVisible"
              >
                <n-icon :component="PanelLeftContract24Regular" size="18" />
              </n-icon-wrapper>
            </template>
            {{ graphLeftVisible ? '隐藏左侧面板' : '显示左侧面板' }}
          </n-tooltip>

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
              <n-icon-wrapper :size="28" :border-radius="8" class="header-icon" @click="syncAll">
                <n-icon :component="SyncOutline" size="18" />
              </n-icon-wrapper>
            </template>
            同步全部元数据
          </n-tooltip>

          <n-tooltip trigger="hover">
            <template #trigger>
              <n-icon-wrapper
                :size="28"
                :border-radius="8"
                class="header-icon"
                :class="{ active: viewMode === 'graph' }"
                @click="toggleViewMode"
              >
                <n-icon :component="viewMode === 'meta' ? LayersOutline : ListOutline" size="18" />
              </n-icon-wrapper>
            </template>
            {{ viewMode === 'meta' ? '切换到图形编辑模式' : '切换到元数据模式' }}
          </n-tooltip>

          <n-tooltip v-if="viewMode === 'graph'" trigger="hover">
            <template #trigger>
              <n-icon-wrapper
                :size="28"
                :border-radius="8"
                class="header-icon"
                :class="{ active: !graphRightVisible }"
                @click="graphRightVisible = !graphRightVisible"
              >
                <n-icon :component="PanelRightContract24Regular" size="18" />
              </n-icon-wrapper>
            </template>
            {{ graphRightVisible ? '隐藏右侧面板' : '显示右侧面板' }}
          </n-tooltip>
        </n-space>
      </div>

      <div v-if="viewMode === 'meta'" class="main">
        <div class="left">
          <div class="left-body">
            <n-tree
              v-if="treeData.length"
              class="kb-tree"
              block-line
              :data="treeData"
              :expanded-keys="expandedKeys"
              :selected-keys="selectedKeys"
              @update:expanded-keys="expandedKeys = $event"
              @update:selected-keys="handleSelectNode"
            />

            <div v-else class="empty">
              请选择一个知识库
            </div>
          </div>
        </div>

        <div class="right">
          <div class="editor-header">
            <div class="editor-title">{{ editorTitle }}</div>
            <n-space justify="end">
              <n-button size="small" @click="reloadCurrent" :disabled="loading || !selectedKbName">重新加载</n-button>
              <n-button size="small" tertiary @click="runKbMetaAgent" :loading="kbAgentProcessing" :disabled="loading || kbAgentProcessing || !canRunKbAgent">AI整理</n-button>
              <n-button type="primary" size="small" @click="saveCurrent" :disabled="loading || !canSave">保存</n-button>
            </n-space>
          </div>

          <div class="editor-body">
            <div v-if="!selectedKbName" class="empty">请选择一个知识库</div>

            <n-form v-else-if="currentKind === 'kb' && kbDraft" :model="kbDraft" label-placement="left" label-width="96">
              <n-form-item label="知识库名">
                <n-input :value="kbDraft.knowledge_base_name" disabled />
              </n-form-item>
              <n-form-item label="类型">
                <n-input v-model:value="kbDraft.knowledge_base_type" placeholder="例如：技术 / 学习 / 项目" />
              </n-form-item>
              <n-form-item label="路径">
                <n-input :value="kbDraft.knowledge_base_location" disabled />
              </n-form-item>
              <n-form-item label="创建时间">
                <n-input :value="kbDraft.created_at" disabled />
              </n-form-item>
              <n-form-item label="修改时间">
                <n-input :value="kbDraft.modified_at" disabled />
              </n-form-item>
              <n-form-item label="文件数">
                <n-input :value="String(kbDraft.stats?.file_count ?? '')" disabled />
              </n-form-item>
              <n-form-item label="目录数">
                <n-input :value="String(kbDraft.stats?.dir_count ?? '')" disabled />
              </n-form-item>
            </n-form>

            <n-form v-else-if="currentKind === 'file' && fileDraft" :model="fileDraft" label-placement="left" label-width="96">
              <n-form-item label="文件名">
                <n-input :value="fileDraft.file_name" disabled />
              </n-form-item>
              <n-form-item label="相对路径">
                <n-input :value="fileDraft.rel_path" disabled />
              </n-form-item>
              <n-form-item label="创建时间">
                <n-input :value="fileDraft.created_at" disabled />
              </n-form-item>
              <n-form-item label="修改时间">
                <n-input :value="fileDraft.modified_at" disabled />
              </n-form-item>

              <n-form-item label="文件标签">
                <n-dynamic-tags v-model:value="fileDraft.file_tags" />
              </n-form-item>
              <n-form-item label="关联节点">
                <n-dynamic-tags v-model:value="fileDraft.related_nodes" />
              </n-form-item>

              <n-form-item label="结构节点">
                <n-collapse v-if="fileDraft.structure_nodes?.length">
                  <n-collapse-item
                    v-for="(n, idx) in fileDraft.structure_nodes"
                    :key="n.stable_id || `${n.level}-${n.title}-${idx}`"
                    :title="n.display_title || n.title"
                  >
                    <n-form-item label="标签">
                      <n-dynamic-tags v-model:value="n.tags" />
                    </n-form-item>
                    <n-form-item label="关联">
                      <n-dynamic-tags v-model:value="n.related_nodes" />
                    </n-form-item>
                    <n-form-item label="属性(JSON)">
                      <n-input v-model:value="n.attrs_json" type="textarea" :autosize="{ minRows: 2, maxRows: 8 }" placeholder="{ }" />
                    </n-form-item>
                  </n-collapse-item>
                </n-collapse>
                <div v-else class="empty">该文件没有可扫描的 Markdown 结构</div>
              </n-form-item>
            </n-form>
          </div>
        </div>
      </div>

      <GuaxiKnowledgeBaseGraphEditor
        v-else
        v-model:selected-kb-name="selectedKbName"
        v-model:expanded-keys="expandedKeys"
        v-model:selected-keys="selectedKeys"
        v-model:left-visible="graphLeftVisible"
        v-model:right-visible="graphRightVisible"
        :loading="loading"
        :kb-options="kbOptions"
        :tree-data="treeData"
        :kb-location="currentKbLocation"
        :root-key="ROOT_KEY"
        @kb-change="handleKbChange"
        @patch-tree="handleGraphPatchTree"
        @reload="handleGraphReload"
      />
    </div>
  </n-message-provider>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { computed, onMounted, ref, watch } from 'vue'
import { NButton, NCollapse, NCollapseItem, NDynamicTags, NForm, NFormItem, NIcon, NIconWrapper, NInput, NSelect, NSpace, NTooltip, NTree, createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { LayersOutline, ListOutline, RefreshOutline, SyncOutline } from '@vicons/ionicons5'
import { PanelLeftContract24Regular, PanelRightContract24Regular } from '@vicons/fluent'
import GuaxiKnowledgeBaseGraphEditor from './GuaxiKnowledgeBaseGraphEditor.vue'
import { useRightSiderStore } from '../../stores/rightSiderStore'

const logger = createLogger('GuaxiKnowledgeBaseManager')
// 顶部工具条使用 createDiscreteApi，避免依赖组件上下文
const { message } = createDiscreteApi(['message'])
const rightSiderStore = useRightSiderStore()

// 主界面模式：meta（当前 JSON/表单编辑） / graph（图形化目录树编辑）
const viewMode = ref('graph')

// 图形模式下左右面板显隐
const graphLeftVisible = ref(true)
const graphRightVisible = ref(false)

const toggleViewMode = () => {
  const next = viewMode.value === 'meta' ? 'graph' : 'meta'
  viewMode.value = next

  if (next === 'graph') {
    graphLeftVisible.value = true
    graphRightVisible.value = false

    // 图形模式默认不全展开，只保留根节点展开；选中节点会在图形面板内按需展开并聚焦
    expandedKeys.value = [ROOT_KEY]
    if (!Array.isArray(selectedKeys.value) || !selectedKeys.value.length) {
      selectedKeys.value = [ROOT_KEY]
    }
  }
}

const ROOT_KEY = '__kb_root__'

const loading = ref(false)
const kbRootPath = ref('')
const kbList = ref([])

const selectedKbName = ref('')
const kbMeta = ref(null)
const kbDraft = ref(null)

const treeData = ref([])
const expandedKeys = ref(['__kb_root__'])
const selectedKeys = ref(['__kb_root__'])

const currentKind = ref('')
const currentRelPath = ref('')

const fileDraft = ref(null)
const kbAgentProcessing = ref(false)
const kbAgentSessionId = ref('')

const canSave = computed(() => {
  if (!selectedKbName.value) return false
  if (currentKind.value === 'kb') return !!kbDraft.value
  if (currentKind.value === 'file') return !!fileDraft.value
  return false
})

const kbAgentModelId = computed(() => {
  const n = Number(rightSiderStore?.assistant?.modelId)
  return Number.isFinite(n) ? n : 5
})

const isMarkdownRelPath = (relPath) => {
  const p = String(relPath || '').trim().toLowerCase()
  return p.endsWith('.md') || p.endsWith('.markdown')
}

const toAbsPathByRel = (relPath) => {
  const base = String(currentKbLocation.value || '').replace(/\\/g, '/').replace(/\/+$/, '')
  const rel = String(relPath || '').replace(/\\/g, '/').replace(/^\/+/, '')
  if (!base || !rel) return ''
  return `${base}/${rel}`
}

const canRunKbAgent = computed(() => {
  if (!selectedKbName.value) return false
  if (currentKind.value === 'kb') return !!kbDraft.value
  if (currentKind.value === 'file') return !!fileDraft.value && isMarkdownRelPath(currentRelPath.value)
  return false
})

const kbOptions = computed(() => {
  return kbList.value.map(x => ({
    label: x?.knowledge_base_name || '',
    value: x?.knowledge_base_name || ''
  })).filter(x => x.value)
})

const editorTitle = computed(() => {
  if (!selectedKbName.value) return ''
  if (currentKind.value === 'file' && currentRelPath.value) {
    const name = String(currentRelPath.value).split('/').pop() || currentRelPath.value
    return `${name}_meta.json`
  }
  return `${selectedKbName.value}_meta.json`
})

const buildTreeNodes = (node) => {
  if (!node) return []
  const kids = Array.isArray(node.children) ? node.children : []
  return kids.map(ch => ({
    key: ch.rel_path,
    label: ch.name,
    isDir: !!ch.is_dir,
    children: ch.is_dir ? buildTreeNodes(ch) : undefined
  }))
}

// 段落节点：使用“文件相对路径/__para__/stable_id”作为 key，便于随文件/目录重命名同步 remap
const PARA_SEG = '__para__'
const PARA_MARK = `/${PARA_SEG}/`

const normalizeRelKey = (p) => String(p || '').replace(/\\/g, '/').replace(/^\/+/, '').replace(/\/+$/, '')

const extractFileRelPathFromTreeKey = (key) => {
  const k = String(key || '')
  if (!k || k === ROOT_KEY) return ''
  const idx = k.indexOf(PARA_MARK)
  return idx >= 0 ? k.slice(0, idx) : k
}

const findTreeNodeByKey = (root, key) => {
  const k = String(key || '')
  if (!root || !k) return null

  const dfs = (n) => {
    if (!n) return null
    if (String(n.key || '') === k) return n
    const kids = Array.isArray(n.children) ? n.children : []
    for (const ch of kids) {
      const hit = dfs(ch)
      if (hit) return hit
    }
    return null
  }

  return dfs(root)
}

const fileParagraphChildrenCache = new Map()

const makeParagraphChildrenFromFileMeta = (fileRelPath, meta) => {
  const rel = normalizeRelKey(fileRelPath)
  const nodes = Array.isArray(meta?.structure_nodes) ? meta.structure_nodes : []
  const out = []

  nodes.forEach((n, idx) => {
    const title = String(n?.title || '').trim()
    const level = Number(n?.level || 0)
    const attrs = (n?.attrs && typeof n.attrs === 'object') ? n.attrs : {}
    const kind = String(attrs?.kind || (level > 0 ? 'section' : 'paragraph'))
    if (kind !== 'paragraph') return

    const stableId = typeof attrs?.stable_id === 'string' && attrs.stable_id
      ? attrs.stable_id
      : `idx_${idx}`

    const start = attrs?.start_line
    const end = attrs?.end_line

    const display = title
      ? `P L${start ?? ''}-${end ?? ''} ${title}`
      : `P L${start ?? ''}-${end ?? ''}`

    out.push({
      key: `${rel}/${PARA_SEG}/${stableId}`,
      label: display,
      isDir: false,
      kind: 'paragraph',
      isParagraph: true,
      stableId,
      startLine: start,
      endLine: end
    })
  })

  return out
}

const applyParagraphChildrenToTree = (fileRelPath, children) => {
  const rel = normalizeRelKey(fileRelPath)
  const root = Array.isArray(treeData.value) ? treeData.value[0] : null
  if (!root || !rel) return false

  const fileNode = findTreeNodeByKey(root, rel)
  if (!fileNode || fileNode.isDir) return false

  const nextChildren = Array.isArray(children) ? children : []
  fileNode.children = nextChildren.length ? nextChildren : undefined

  // 触发树组件刷新（深层对象变更在某些情况下不会立刻触发 n-tree 重绘）
  treeData.value = [...(treeData.value || [])]
  return true
}

const ensureParagraphChildrenForFile = async (fileRelPath) => {
  const rel = normalizeRelKey(fileRelPath)
  if (!rel || !selectedKbName.value) return false

  const root = Array.isArray(treeData.value) ? treeData.value[0] : null
  if (!root) return false

  const fileNode = findTreeNodeByKey(root, rel)
  if (!fileNode || fileNode.isDir) return false

  if (Array.isArray(fileNode.children) && fileNode.children.some(ch => String(ch?.kind || '') === 'paragraph')) {
    return true
  }

  if (fileParagraphChildrenCache.has(rel)) {
    const cached = fileParagraphChildrenCache.get(rel)
    applyParagraphChildrenToTree(rel, cached)
    return true
  }

  try {
    logger.info('[KBManager] load paragraph nodes', { relPath: rel })
    const meta = await invoke('kbmeta_read_file_meta', { knowledgeBaseName: selectedKbName.value, relPath: rel })
    const children = makeParagraphChildrenFromFileMeta(rel, meta)
    fileParagraphChildrenCache.set(rel, children)
    applyParagraphChildrenToTree(rel, children)

    if (children.length) {
      expandedKeys.value = Array.from(new Set([...(expandedKeys.value || []), ROOT_KEY, rel]))
    }

    logger.info('[KBManager] paragraph nodes ready', { relPath: rel, count: children.length })
    return true
  } catch (e) {
    logger.error('[KBManager] load paragraph nodes failed:', e)
    return false
  }
}

const deepClone = (v) => (v == null ? v : JSON.parse(JSON.stringify(v)))

const uniqueStringList = (arr) => {
  const list = Array.isArray(arr) ? arr : []
  const normalized = list.map(x => String(x || '').trim()).filter(Boolean)
  return Array.from(new Set(normalized))
}

const mergeUniqueStringList = (base, patch) => {
  return uniqueStringList([...(Array.isArray(base) ? base : []), ...(Array.isArray(patch) ? patch : [])])
}

const pickResultArray = (obj, names) => {
  if (!obj || typeof obj !== 'object') return []
  for (const key of names) {
    const v = obj?.[key]
    if (Array.isArray(v)) return v
  }
  return []
}

const pickResultValue = (obj, names) => {
  if (!obj || typeof obj !== 'object') return undefined
  for (const key of names) {
    if (Object.prototype.hasOwnProperty.call(obj, key)) return obj[key]
  }
  return undefined
}

const relationHint = (toStableId, relation) => {
  const toId = String(toStableId || '').trim()
  const rel = String(relation || '').trim()
  if (!toId) return ''
  return rel ? `${toId}(${rel})` : toId
}

const makeFileDraft = (meta) => {
  const d = deepClone(meta) || {}
  d.file_tags = Array.isArray(d.file_tags) ? d.file_tags : []
  d.related_nodes = Array.isArray(d.related_nodes) ? d.related_nodes : []
  const nodes = Array.isArray(d.structure_nodes) ? d.structure_nodes : []

  d.structure_nodes = nodes.map(n => {
    const title = String(n?.title || '')
    const level = Number(n?.level || 0)
    const attrs = (n?.attrs && typeof n.attrs === 'object') ? n.attrs : {}
    const kind = String(attrs?.kind || (level > 0 ? 'section' : 'paragraph'))
    const stable_id = typeof attrs?.stable_id === 'string' ? attrs.stable_id : ''
    const start = attrs?.start_line
    const end = attrs?.end_line

    const display_title = kind === 'paragraph'
      ? `P L${start ?? ''}-${end ?? ''} ${title}`
      : `H${level} ${title}`

    return {
      title,
      level,
      kind,
      stable_id,
      tags: Array.isArray(n?.tags) ? n.tags : [],
      related_nodes: Array.isArray(n?.related_nodes) ? n.related_nodes : [],
      attrs_json: JSON.stringify(n?.attrs ?? {}, null, 2),
      display_title
    }
  }).filter(n => n.title)

  return d
}

const parseAttrsJsonObject = (raw) => {
  const text = String(raw || '').trim()
  if (!text) return {}
  try {
    const parsed = JSON.parse(text)
    return (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) ? parsed : {}
  } catch (e) {
    logger.warn('[KBManager] parse attrs_json failed, fallback empty object', e)
    return {}
  }
}

const buildFileMetaForKbAgent = (draft) => {
  const d = deepClone(draft) || {}
  const nodes = Array.isArray(d.structure_nodes) ? d.structure_nodes : []
  const structure_nodes = nodes.map(n => {
    const attrs = parseAttrsJsonObject(n?.attrs_json)
    const stable_id = String(n?.stable_id || '').trim()
    if (stable_id && !attrs.stable_id) attrs.stable_id = stable_id
    return {
      title: String(n?.title || ''),
      level: Number(n?.level || 0),
      tags: uniqueStringList(n?.tags),
      related_nodes: uniqueStringList(n?.related_nodes),
      attrs
    }
  })

  return {
    file_name: String(d.file_name || ''),
    rel_path: String(d.rel_path || currentRelPath.value || ''),
    file_tags: uniqueStringList(d.file_tags),
    related_nodes: uniqueStringList(d.related_nodes),
    structure_nodes
  }
}

const applyKbAgentResultToKbDraft = (result) => {
  if (!kbDraft.value || !result || typeof result !== 'object') return false
  let changed = false
  const next = deepClone(kbDraft.value)
  const nextType = String(pickResultValue(result, ['knowledge_base_type', 'knowledgeBaseType', 'kb_type', 'kbType']) || '').trim()
  if (nextType && next.knowledge_base_type !== nextType) {
    next.knowledge_base_type = nextType
    changed = true
  }
  if (changed) kbDraft.value = next
  return changed
}

const applyKbAgentResultToFileDraft = (result) => {
  if (!fileDraft.value || !result || typeof result !== 'object') return false
  let changed = false
  const next = deepClone(fileDraft.value)

  const fileTags = pickResultArray(result, ['file_tags', 'fileTags'])
  if (fileTags.length) {
    const merged = mergeUniqueStringList(next.file_tags, fileTags)
    if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(next.file_tags))) {
      next.file_tags = merged
      changed = true
    }
  }

  const fileRelated = pickResultArray(result, ['related_nodes', 'relatedNodes'])
  if (fileRelated.length) {
    const merged = mergeUniqueStringList(next.related_nodes, fileRelated)
    if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(next.related_nodes))) {
      next.related_nodes = merged
      changed = true
    }
  }

  const patches = pickResultArray(result, ['structure_nodes', 'structureNodes', 'nodes'])
  if (patches.length && Array.isArray(next.structure_nodes)) {
    for (const patch of patches) {
      if (!patch || typeof patch !== 'object') continue
      const stableId = String(pickResultValue(patch, ['stable_id', 'stableId', 'id', 'node_id']) || '').trim()
      const title = String(pickResultValue(patch, ['title', 'name']) || '').trim()
      const level = Number(pickResultValue(patch, ['level']) || 0)
      const hit = next.structure_nodes.find(n => {
        if (stableId && String(n?.stable_id || '').trim() === stableId) return true
        if (title && Number(n?.level || 0) === level && String(n?.title || '').trim() === title) return true
        return false
      })
      if (!hit) continue

      const patchTags = pickResultArray(patch, ['tags'])
      if (patchTags.length) {
        const merged = mergeUniqueStringList(hit.tags, patchTags)
        if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(hit.tags))) {
          hit.tags = merged
          changed = true
        }
      }

      const patchRelated = pickResultArray(patch, ['related_nodes', 'relatedNodes'])
      if (patchRelated.length) {
        const merged = mergeUniqueStringList(hit.related_nodes, patchRelated)
        if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(hit.related_nodes))) {
          hit.related_nodes = merged
          changed = true
        }
      }

      const attrsPatch = pickResultValue(patch, ['attrs_patch', 'attrsPatch', 'attrs'])
      if (attrsPatch && typeof attrsPatch === 'object' && !Array.isArray(attrsPatch)) {
        const currentAttrs = parseAttrsJsonObject(hit.attrs_json)
        const mergedAttrs = { ...currentAttrs, ...attrsPatch }
        const nextJson = JSON.stringify(mergedAttrs, null, 2)
        if (nextJson !== String(hit.attrs_json || '')) {
          hit.attrs_json = nextJson
          changed = true
        }
      }
    }
  }

  const knowledgeRelations = pickResultArray(result, ['knowledge_relations', 'knowledgeRelations', 'relations'])
  if (knowledgeRelations.length && Array.isArray(next.structure_nodes)) {
    for (const edge of knowledgeRelations) {
      if (!edge || typeof edge !== 'object') continue
      const fromId = String(pickResultValue(edge, ['from_stable_id', 'fromStableId', 'from']) || '').trim()
      const toId = String(pickResultValue(edge, ['to_stable_id', 'toStableId', 'to']) || '').trim()
      const hint = relationHint(toId, pickResultValue(edge, ['relation', 'type', 'label']))
      if (!fromId || !hint) continue
      const fromNode = next.structure_nodes.find(n => String(n?.stable_id || '').trim() === fromId)
      if (!fromNode) continue
      const merged = mergeUniqueStringList(fromNode.related_nodes, [hint])
      if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(fromNode.related_nodes))) {
        fromNode.related_nodes = merged
        changed = true
      }
    }
  }

  if (changed) fileDraft.value = next
  return changed
}

const applyKbAgentResultToRawFileMeta = (fileMeta, result) => {
  if (!fileMeta || typeof fileMeta !== 'object') return { ok: false, changed: false, meta: fileMeta }
  if (!result || typeof result !== 'object') return { ok: false, changed: false, meta: fileMeta }
  let changed = false
  const next = deepClone(fileMeta)

  const fileTags = pickResultArray(result, ['file_tags', 'fileTags'])
  if (fileTags.length) {
    const merged = mergeUniqueStringList(next.file_tags, fileTags)
    if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(next.file_tags))) {
      next.file_tags = merged
      changed = true
    }
  }

  const fileRelated = pickResultArray(result, ['related_nodes', 'relatedNodes'])
  if (fileRelated.length) {
    const merged = mergeUniqueStringList(next.related_nodes, fileRelated)
    if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(next.related_nodes))) {
      next.related_nodes = merged
      changed = true
    }
  }

  const patches = pickResultArray(result, ['structure_nodes', 'structureNodes', 'nodes'])
  const nodes = Array.isArray(next.structure_nodes) ? next.structure_nodes : []
  if (patches.length && nodes.length) {
    for (const patch of patches) {
      if (!patch || typeof patch !== 'object') continue
      const stableId = String(pickResultValue(patch, ['stable_id', 'stableId', 'id', 'node_id']) || '').trim()
      const title = String(pickResultValue(patch, ['title', 'name']) || '').trim()
      const level = Number(pickResultValue(patch, ['level']) || 0)
      const hit = nodes.find(n => {
        const attrs = (n?.attrs && typeof n.attrs === 'object') ? n.attrs : {}
        const sid = String(attrs?.stable_id || '').trim()
        if (stableId && sid === stableId) return true
        if (title && Number(n?.level || 0) === level && String(n?.title || '').trim() === title) return true
        return false
      })
      if (!hit) continue

      const patchTags = pickResultArray(patch, ['tags'])
      if (patchTags.length) {
        const merged = mergeUniqueStringList(hit.tags, patchTags)
        if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(hit.tags))) {
          hit.tags = merged
          changed = true
        }
      }

      const patchRelated = pickResultArray(patch, ['related_nodes', 'relatedNodes'])
      if (patchRelated.length) {
        const merged = mergeUniqueStringList(hit.related_nodes, patchRelated)
        if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(hit.related_nodes))) {
          hit.related_nodes = merged
          changed = true
        }
      }

      const attrsPatch = pickResultValue(patch, ['attrs_patch', 'attrsPatch', 'attrs'])
      if (attrsPatch && typeof attrsPatch === 'object' && !Array.isArray(attrsPatch)) {
        const attrs = (hit?.attrs && typeof hit.attrs === 'object') ? hit.attrs : {}
        hit.attrs = { ...attrs, ...attrsPatch }
        changed = true
      }
    }
  }

  const knowledgeRelations = pickResultArray(result, ['knowledge_relations', 'knowledgeRelations', 'relations'])
  if (knowledgeRelations.length && nodes.length) {
    for (const edge of knowledgeRelations) {
      if (!edge || typeof edge !== 'object') continue
      const fromId = String(pickResultValue(edge, ['from_stable_id', 'fromStableId', 'from']) || '').trim()
      const toId = String(pickResultValue(edge, ['to_stable_id', 'toStableId', 'to']) || '').trim()
      const hint = relationHint(toId, pickResultValue(edge, ['relation', 'type', 'label']))
      if (!fromId || !hint) continue
      const fromNode = nodes.find(n => {
        const attrs = (n?.attrs && typeof n.attrs === 'object') ? n.attrs : {}
        return String(attrs?.stable_id || '').trim() === fromId
      })
      if (!fromNode) continue
      const merged = mergeUniqueStringList(fromNode.related_nodes, [hint])
      if (JSON.stringify(merged) !== JSON.stringify(uniqueStringList(fromNode.related_nodes))) {
        fromNode.related_nodes = merged
        changed = true
      }
    }
  }

  return { ok: true, changed, meta: next }
}

const collectFileRelPathsFromTreeNode = (node, out = []) => {
  if (!node || typeof node !== 'object') return out
  const isDir = !!node.isDir
  const key = String(node.key || '')
  if (!isDir && key && key !== ROOT_KEY && !key.includes(PARA_MARK)) {
    out.push(normalizeRelKey(key))
    return out
  }
  const kids = Array.isArray(node.children) ? node.children : []
  for (const ch of kids) collectFileRelPathsFromTreeNode(ch, out)
  return out
}

const runKbMetaAgent = async () => {
  if (!canRunKbAgent.value || !selectedKbName.value) return

  const task = currentKind.value === 'file' ? 'file' : 'kb'
  const kbMetaPayload = deepClone(kbDraft.value || kbMeta.value || {})
  const fileMetaPayload = task === 'file' ? buildFileMetaForKbAgent(fileDraft.value) : null

  try {
    kbAgentProcessing.value = true
    logger.info('[KBManager] kbmeta_agent_process start', {
      task,
      kbName: selectedKbName.value,
      relPath: currentRelPath.value,
      modelid: kbAgentModelId.value
    })

    if (task === 'kb') {
      const root = Array.isArray(treeData.value) ? treeData.value[0] : null
      const relPaths = Array.from(new Set(collectFileRelPathsFromTreeNode(root, [])))
        .filter(Boolean)
        .filter(isMarkdownRelPath)
      if (!relPaths.length) {
        message.info('当前知识库没有可处理的 Markdown 文件')
        return
      }

      const fileMetaMap = new Map()
      const fileContentMap = new Map()
      for (const relPath of relPaths) {
        try {
          const currentMeta = await invoke('kbmeta_read_file_meta', {
            knowledgeBaseName: selectedKbName.value,
            relPath
          })
          fileMetaMap.set(relPath, currentMeta)
          const absPath = toAbsPathByRel(relPath)
          if (absPath) {
            const content = await invoke('read_file_content', { path: absPath })
            fileContentMap.set(relPath, String(content || ''))
          }
        } catch (e) {
          logger.warn('[KBManager] read file meta failed in batch pre-load', { relPath, error: String(e?.message || e) })
        }
      }

      const kbContextFiles = Array.from(fileMetaMap.entries()).map(([relPath, meta]) => ({
        rel_path: relPath,
        file_tags: Array.isArray(meta?.file_tags) ? meta.file_tags : [],
        related_nodes: Array.isArray(meta?.related_nodes) ? meta.related_nodes : [],
        content_excerpt: String(fileContentMap.get(relPath) || '').slice(0, 3000),
        structure_outline: Array.isArray(meta?.structure_nodes)
          ? meta.structure_nodes.slice(0, 50).map(n => ({
            title: String(n?.title || ''),
            level: Number(n?.level || 0),
            stable_id: String(n?.attrs?.stable_id || '')
          }))
          : []
      }))

      let updatedCount = 0
      for (const relPath of relPaths) {
        try {
          const currentMeta = fileMetaMap.get(relPath)
          if (!currentMeta) continue
          const response = await invoke('kbmeta_agent_process', {
            request: {
              modelid: kbAgentModelId.value,
              sessionid: kbAgentSessionId.value,
              task: 'file',
              kb_name: selectedKbName.value,
              rel_path: relPath,
              kb_meta: kbMetaPayload,
              file_meta: currentMeta,
              kb_context_files: kbContextFiles,
              file_content: String(fileContentMap.get(relPath) || '')
            }
          })

          if (response?.sessionid) kbAgentSessionId.value = String(response.sessionid)
          const result = response?.result
          logger.info('[KBManager] batch kbmeta_agent_process result keys', { relPath, keys: Object.keys(result || {}) })
          const applied = applyKbAgentResultToRawFileMeta(currentMeta, result)
          if (applied.ok && applied.changed) {
            await invoke('kbmeta_save_file_meta', {
              knowledgeBaseName: selectedKbName.value,
              relPath,
              meta: applied.meta
            })
            updatedCount += 1
          }
        } catch (e) {
          logger.warn('[KBManager] batch kbmeta_agent_process failed on file', { relPath, error: String(e?.message || e) })
        }
      }

      await loadKbMeta(selectedKbName.value)
      if (updatedCount > 0) {
        message.success(`AI 已批量更新 ${updatedCount} 个文件元数据`)
      } else {
        message.info('AI 已完成分析，但没有可应用的变更')
      }
      return
    }

    const root = Array.isArray(treeData.value) ? treeData.value[0] : null
    const allRelPaths = Array.from(new Set(collectFileRelPathsFromTreeNode(root, [])))
      .filter(Boolean)
      .filter(isMarkdownRelPath)
    const kbContextFiles = []
    const fileContentMap = new Map()
    for (const relPath of allRelPaths) {
      try {
        const meta = await invoke('kbmeta_read_file_meta', {
          knowledgeBaseName: selectedKbName.value,
          relPath
        })
        const absPath = toAbsPathByRel(relPath)
        let fileContent = ''
        if (absPath) {
          fileContent = String(await invoke('read_file_content', { path: absPath }) || '')
          fileContentMap.set(relPath, fileContent)
        }
        kbContextFiles.push({
          rel_path: relPath,
          file_tags: Array.isArray(meta?.file_tags) ? meta.file_tags : [],
          related_nodes: Array.isArray(meta?.related_nodes) ? meta.related_nodes : [],
          content_excerpt: fileContent.slice(0, 3000),
          structure_outline: Array.isArray(meta?.structure_nodes)
            ? meta.structure_nodes.slice(0, 50).map(n => ({
              title: String(n?.title || ''),
              level: Number(n?.level || 0),
              stable_id: String(n?.attrs?.stable_id || '')
            }))
            : []
        })
      } catch (e) {
        logger.warn('[KBManager] read file meta failed in single context', { relPath, error: String(e?.message || e) })
      }
    }

    const response = await invoke('kbmeta_agent_process', {
      request: {
        modelid: kbAgentModelId.value,
        sessionid: kbAgentSessionId.value,
        task,
        kb_name: selectedKbName.value,
        rel_path: currentRelPath.value,
        kb_meta: kbMetaPayload,
        file_meta: fileMetaPayload,
        kb_context_files: kbContextFiles,
        file_content: String(fileContentMap.get(currentRelPath.value) || '')
      }
    })

    if (response?.sessionid) kbAgentSessionId.value = String(response.sessionid)

    const result = response?.result
    logger.info('[KBManager] kbmeta_agent_process result keys', { keys: Object.keys(result || {}) })
    const changed = applyKbAgentResultToFileDraft(result)

    if (changed) {
      message.success('AI 建议已应用到当前草稿，请点击保存落盘')
    } else {
      message.info('AI 已完成分析，但没有可应用的变更')
    }
  } catch (e) {
    logger.error('[KBManager] kbmeta_agent_process failed:', e)
    message.error(String(e?.message || 'AI 处理失败'))
  } finally {
    kbAgentProcessing.value = false
  }
}

const loadKbMeta = async (kbName) => {
  const meta = await invoke('kbmeta_read_kb_meta', { knowledgeBaseName: kbName })
  kbMeta.value = meta
  kbDraft.value = deepClone(meta)
  fileDraft.value = null

  // 切换知识库时清空段落缓存，避免跨 KB 串数据
  fileParagraphChildrenCache.clear()

  const rootNode = meta?.tree
  treeData.value = rootNode ? [{
    key: ROOT_KEY,
    label: rootNode.name || kbName,
    isDir: true,
    children: buildTreeNodes(rootNode)
  }] : []

  expandedKeys.value = treeData.value.length ? [ROOT_KEY] : []
  selectedKeys.value = treeData.value.length ? [ROOT_KEY] : []

  currentKind.value = 'kb'
  currentRelPath.value = ''

  logger.info('[KBManager] kb meta loaded', { kbName, hasTree: !!rootNode })
}

const refresh = async () => {
  try {
    loading.value = true
    const [root, list] = await Promise.all([
      invoke('get_knowledge_bases_root'),
      invoke('kbmeta_sync_all')
    ])
    kbRootPath.value = typeof root === 'string' ? root : ''
    kbList.value = Array.isArray(list) ? list : []
  } catch (e) {
    logger.error('加载知识库元数据失败:', e)
    message.error('加载知识库元数据失败')
  } finally {
    loading.value = false
  }
}

const syncAll = async () => {
  await refresh()
  message.success('已同步')
}

const handleKbChange = async (kbName) => {
  if (!kbName) {
    selectedKbName.value = ''
    kbMeta.value = null
    kbDraft.value = null
    fileDraft.value = null
    treeData.value = []
    expandedKeys.value = []
    selectedKeys.value = []
    currentKind.value = ''
    currentRelPath.value = ''

    fileParagraphChildrenCache.clear()
    return
  }

  try {
    loading.value = true
    await loadKbMeta(kbName)
  } catch (e) {
    logger.error('加载知识库配置失败:', e)
    message.error('加载知识库配置失败')
  } finally {
    loading.value = false
  }
}

const handleSelectNode = async (keys, options) => {
  selectedKeys.value = Array.isArray(keys) ? keys : []
  const opt = Array.isArray(options) ? options[0] : null
  if (!opt || !selectedKbName.value) return

  if (opt.isDir) {
    currentKind.value = 'kb'
    currentRelPath.value = ''
    fileDraft.value = null
    kbDraft.value = deepClone(kbMeta.value)
    return
  }

  try {
    loading.value = true
    currentKind.value = 'file'
    currentRelPath.value = String(opt.key || '')
    const meta = await invoke('kbmeta_read_file_meta', {
      knowledgeBaseName: selectedKbName.value,
      relPath: currentRelPath.value
    })
    kbDraft.value = null
    fileDraft.value = makeFileDraft(meta)

    // 元数据模式下：选中文件时直接把段落节点挂到树上（避免再发一次请求）
    const paras = makeParagraphChildrenFromFileMeta(currentRelPath.value, meta)
    fileParagraphChildrenCache.set(normalizeRelKey(currentRelPath.value), paras)
    applyParagraphChildrenToTree(currentRelPath.value, paras)

    if (paras.length) {
      expandedKeys.value = Array.from(new Set([...(expandedKeys.value || []), ROOT_KEY, normalizeRelKey(currentRelPath.value)]))
    }
  } catch (e) {
    logger.error('读取文件配置失败:', e)
    message.error('读取文件配置失败')
  } finally {
    loading.value = false
  }
}

const reloadCurrent = async () => {
  if (!selectedKbName.value) return
  try {
    loading.value = true
    if (currentKind.value === 'file' && currentRelPath.value) {
      const meta = await invoke('kbmeta_read_file_meta', {
        knowledgeBaseName: selectedKbName.value,
        relPath: currentRelPath.value
      })
      kbDraft.value = null
      fileDraft.value = makeFileDraft(meta)
      return
    }

    await loadKbMeta(selectedKbName.value)
  } catch (e) {
    logger.error('重新加载失败:', e)
    message.error('重新加载失败')
  } finally {
    loading.value = false
  }
}

const currentKbLocation = computed(() => {
  const kbName = selectedKbName.value
  if (!kbName) return ''

  const hit = Array.isArray(kbList.value)
    ? kbList.value.find(x => x?.knowledge_base_name === kbName)
    : null

  return String(hit?.knowledge_base_location || kbMeta.value?.knowledge_base_location || '')
})

const expandToRelPath = (relPath) => {
  const p = String(relPath || '').replace(/\\/g, '/').replace(/^\/+/, '')
  if (!p) return
  const parts = p.split('/').filter(Boolean)
  const keys = [ROOT_KEY]
  for (let i = 0; i < Math.max(parts.length - 1, 0); i++) {
    keys.push(parts.slice(0, i + 1).join('/'))
  }
  expandedKeys.value = Array.from(new Set([...(expandedKeys.value || []), ...keys]))
}

const basenameOfRel = (relPath) => {
  const s = String(relPath || '').replace(/\\/g, '/').replace(/^\/+/, '').replace(/\/+$/, '')
  return s.split('/').filter(Boolean).pop() || ''
}

const remapKey = (key, oldKey, newKey) => {
  const k = String(key || '')
  const oldK = String(oldKey || '')
  const newK = String(newKey || '')

  if (!k || k === ROOT_KEY) return k
  if (!oldK || !newK) return k

  if (k === oldK) return newK
  if (k.startsWith(`${oldK}/`)) return `${newK}${k.slice(oldK.length)}`
  return k
}

const patchTreeAfterRename = (treeRoot, oldKey, newKey) => {
  const oldK = String(oldKey || '')
  const newK = String(newKey || '')
  if (!treeRoot || !oldK || !newK) return false

  const patchDescendants = (node) => {
    const kids = Array.isArray(node?.children) ? node.children : []
    for (const ch of kids) {
      ch.key = remapKey(ch.key, oldK, newK)

      // 段落节点 label 是展示文本（例如行号+标题），不应该被 basename 覆盖
      if (String(ch?.kind || '') !== 'paragraph') {
        const base = basenameOfRel(ch.key)
        if (base) ch.label = base
      }

      patchDescendants(ch)
    }
  }

  const dfs = (node) => {
    if (!node) return false
    if (String(node.key || '') === oldK) {
      node.key = newK
      const base = basenameOfRel(newK)
      if (base) node.label = base
      patchDescendants(node)
      return true
    }

    const kids = Array.isArray(node.children) ? node.children : []
    for (const ch of kids) {
      if (dfs(ch)) return true
    }
    return false
  }

  return dfs(treeRoot)
}

const handleGraphPatchTree = (payload) => {
  const oldKey = String(payload?.oldKey || '')
  const newKey = String(payload?.newKey || '')

  if (!oldKey || !newKey) {
    logger.warn('[KBManager] patch-tree invalid payload', payload)
    return
  }

  const root = Array.isArray(treeData.value) ? treeData.value[0] : null
  if (!root) {
    logger.warn('[KBManager] patch-tree missed: empty tree', payload)
    return
  }

  const ok = patchTreeAfterRename(root, oldKey, newKey)
  if (!ok) {
    logger.warn('[KBManager] patch-tree missed: node not found', payload)
    return
  }

  selectedKeys.value = (Array.isArray(selectedKeys.value) ? selectedKeys.value : []).map(k => remapKey(k, oldKey, newKey))

  const remappedExpanded = (Array.isArray(expandedKeys.value) ? expandedKeys.value : []).map(k => remapKey(k, oldKey, newKey))
  expandedKeys.value = Array.from(new Set([ROOT_KEY, ...remappedExpanded]))

  if (currentRelPath.value) currentRelPath.value = remapKey(currentRelPath.value, oldKey, newKey)

  if (!selectedKeys.value.length) selectedKeys.value = [newKey]
  expandToRelPath(newKey)

  logger.info('[KBManager] patch-tree ok', { oldKey, newKey })
}

// 图形模式：选中节点时按需补齐段落节点（避免启动时扫描全库）
watch(() => selectedKeys.value, async (keys) => {
  if (!selectedKbName.value) return

  const rawKey = Array.isArray(keys) ? String(keys[0] || '') : ''
  const fileRel = extractFileRelPathFromTreeKey(rawKey)
  if (!fileRel || fileRel === ROOT_KEY) return

  const root = Array.isArray(treeData.value) ? treeData.value[0] : null
  const node = root ? findTreeNodeByKey(root, fileRel) : null
  if (!node || node.isDir) return

  await ensureParagraphChildrenForFile(fileRel)
}, { deep: false })

const handleGraphReload = async (payload) => {
  if (!selectedKbName.value) return

  try {
    loading.value = true
    await loadKbMeta(selectedKbName.value)

    const nextKey = payload?.selectKey
    if (nextKey) {
      selectedKeys.value = [nextKey]
      expandToRelPath(nextKey)
    }
  } catch (e) {
    logger.error('图形模式刷新树失败:', e)
    message.error('图形模式刷新树失败')
  } finally {
    loading.value = false
  }
}

const saveCurrent = async () => {
  if (!selectedKbName.value) return

  try {
    loading.value = true

    if (currentKind.value === 'file' && currentRelPath.value && fileDraft.value) {
      const payload = deepClone(fileDraft.value)
      payload.file_tags = Array.isArray(payload.file_tags) ? payload.file_tags : []
      payload.related_nodes = Array.isArray(payload.related_nodes) ? payload.related_nodes : []

      const nodes = Array.isArray(payload.structure_nodes) ? payload.structure_nodes : []
      payload.structure_nodes = nodes.map(n => {
        let attrs = {}
        const raw = String(n?.attrs_json || '').trim()
        if (raw) {
          try {
            attrs = JSON.parse(raw)
          } catch (e) {
            const prefix = n?.kind === 'paragraph' ? 'P' : `H${n.level}`
            throw new Error(`结构节点属性 JSON 不正确: ${prefix} ${n.title}`)
          }
        }
        return {
          title: String(n?.title || ''),
          level: Number(n?.level || 0),
          tags: Array.isArray(n?.tags) ? n.tags : [],
          attrs,
          related_nodes: Array.isArray(n?.related_nodes) ? n.related_nodes : []
        }
      })

      await invoke('kbmeta_save_file_meta', {
        knowledgeBaseName: selectedKbName.value,
        relPath: currentRelPath.value,
        meta: payload
      })
      message.success('已保存')
      await reloadCurrent()
      return
    }

    if (currentKind.value === 'kb' && kbDraft.value) {
      const payload = deepClone(kbDraft.value)
      await invoke('kbmeta_save_kb_meta', { meta: payload })
      message.success('已保存')
      await loadKbMeta(selectedKbName.value)
      return
    }
  } catch (e) {
    logger.error('保存失败:', e)
    message.error(String(e?.message || '保存失败'))
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await refresh()
})
</script>

<style scoped>
:global(html),
:global(body),
:global(#app) {
  height: 100%;
  overflow: hidden;
}

:global(body) {
  margin: 0;
}

.page {
  height: 100vh;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 12px;
  box-sizing: border-box;
  gap: 10px;
  overflow: hidden;
  background: var(--guaxi-content-bg);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Microsoft YaHei', system-ui, sans-serif;
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
  color: var(--guaxi-text-primary);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.title {
  font-size: 16px;
  font-weight: 650;
  letter-spacing: 0.2px;
}

.header-icon {
  cursor: pointer;
  border: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
  transition: transform 0.18s ease, box-shadow 0.18s ease, background 0.18s ease;
  color: var(--guaxi-text-primary);
}

.header-icon:hover {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.16), 0 2px 12px rgba(7, 118, 161, 0.08);
  transform: scale(1.02);
}

.header-icon.active {
  background: rgba(24, 160, 88, 0.10);
  box-shadow: 0 0 0 3px rgba(24, 160, 88, 0.18), 0 2px 12px rgba(16, 24, 40, 0.08);
  color: rgba(24, 160, 88, 0.95);
}

.path {
  font-size: 12px;
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding: 6px 10px;
  border-radius: 10px;
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
}

.main {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 360px 1fr;
  gap: 12px;
  overflow: hidden;
}

.left {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 10px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 8px 24px rgba(16, 24, 40, 0.06);
}

.left-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 2px 0;
}

.right {
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 12px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 10px 28px rgba(16, 24, 40, 0.08);
}

.editor-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--guaxi-border);
}

.editor-title {
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty {
  font-size: 12px;
  opacity: 0.75;
  padding: 10px 10px;
  border-radius: 10px;
  border: 1px dashed var(--guaxi-border);
  background: var(--guaxi-header-bg);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.row-name {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub {
  font-size: 12px;
  opacity: 0.7;
  margin-top: 2px;
}

.active {
  background: rgba(24, 160, 88, 0.08);
}

.detail {
  display: flex;
  flex-direction: column;
}

:deep(.kb-tree .n-tree-node) {
  padding: 0;
}

:deep(.kb-tree .n-tree-node-content) {
  margin: 2px 6px;
  padding: 4px 8px;
  border-radius: 8px;
  transition: transform 0.18s ease, box-shadow 0.18s ease, background 0.18s ease;
}

:deep(.kb-tree .n-tree-node-content__text) {
  font-size: 13px;
  line-height: 1.35;
  font-family: 'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', system-ui;
}

:deep(.kb-tree .n-tree-node-content:hover) {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.16), 0 2px 12px rgba(7, 118, 161, 0.08);
  transform: scale(1.01);
}

:deep(.kb-tree .n-tree-node--selected .n-tree-node-content) {
  background: var(--guaxi-header-bg);
  box-shadow: 0 0 0 3px rgba(33, 189, 236, 0.24), 0 2px 12px rgba(7, 118, 161, 0.12);
  animation: kb-tree-select-pop 160ms ease-out;
}

@keyframes kb-tree-select-pop {
  from {
    transform: scale(1);
  }
  60% {
    transform: scale(1.02);
  }
  to {
    transform: scale(1.01);
  }
}
</style>
