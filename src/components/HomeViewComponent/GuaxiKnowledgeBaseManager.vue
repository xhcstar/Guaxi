<template>
  <n-message-provider placement="bottom">
    <div class="page">
      <div class="header">
        <div class="title">知识库管理器</div>
        <n-space>
          <n-button size="small" @click="refresh" :loading="loading">刷新</n-button>
          <n-button size="small" @click="syncAll" :loading="loading">同步全部元数据</n-button>
        </n-space>
      </div>

      <div class="path" :title="kbRootPath">
        {{ kbRootPath || '未配置 KnowledgeBases 路径' }}
      </div>

      <div class="main">
        <div class="left">
          <n-select
            v-model:value="selectedKbName"
            size="small"
            filterable
            clearable
            placeholder="选择知识库"
            :options="kbOptions"
            :disabled="loading"
            @update:value="handleKbChange"
          />

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
    </div>
  </n-message-provider>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { NButton, NCollapse, NCollapseItem, NDynamicTags, NForm, NFormItem, NInput, NSelect, NSpace, NTree, createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

const { message } = createDiscreteApi(['message'])

const loading = ref(false)
const kbRootPath = ref('')
const kbList = ref([])

const selectedKbName = ref('')
const kbMeta = ref(null)
const kbDraft = ref(null)

const treeData = ref([])
const expandedKeys = ref([])
const selectedKeys = ref([])

const currentKind = ref('')
const currentRelPath = ref('')

const fileDraft = ref(null)

const canSave = computed(() => {
  if (!selectedKbName.value) return false
  if (currentKind.value === 'kb') return !!kbDraft.value
  if (currentKind.value === 'file') return !!fileDraft.value
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

const deepClone = (v) => (v == null ? v : JSON.parse(JSON.stringify(v)))

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

const loadKbMeta = async (kbName) => {
  const meta = await invoke('kbmeta_read_kb_meta', { knowledgeBaseName: kbName })
  kbMeta.value = meta
  kbDraft.value = deepClone(meta)
  fileDraft.value = null

  const rootNode = meta?.tree
  const rootKey = '__kb_root__'
  treeData.value = rootNode ? [{
    key: rootKey,
    label: rootNode.name || kbName,
    isDir: true,
    children: buildTreeNodes(rootNode)
  }] : []

  expandedKeys.value = treeData.value.length ? [rootKey] : []
  selectedKeys.value = treeData.value.length ? [rootKey] : []

  currentKind.value = 'kb'
  currentRelPath.value = ''
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
    console.error('加载知识库元数据失败:', e)
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
    return
  }

  try {
    loading.value = true
    await loadKbMeta(kbName)
  } catch (e) {
    console.error('加载知识库配置失败:', e)
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
  } catch (e) {
    console.error('读取文件配置失败:', e)
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
    console.error('重新加载失败:', e)
    message.error('重新加载失败')
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
    console.error('保存失败:', e)
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
}

.title {
  font-size: 16px;
  font-weight: 650;
  letter-spacing: 0.2px;
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