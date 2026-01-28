<template>
  <n-message-provider placement="bottom">
    <div class="page">
      <div class="header">
        <div class="title">资源库管理</div>
        <n-space>
          <n-button size="small" :loading="loading" @click="refresh">刷新</n-button>
          <n-button size="small" :disabled="loading || !resourceRoot" @click="openInExplorer">打开资源库目录</n-button>
          <n-button size="small" :disabled="loading" @click="testConnection">测试连接</n-button>
          <n-button size="small" type="primary" :disabled="loading" @click="syncNow">立即同步</n-button>
        </n-space>
      </div>

      <div class="path" :title="resourceRoot">
        {{ resourceRoot || '未配置资源库根目录（将使用默认 ResourceRepo）' }}
      </div>

      <div class="main">
        <div class="left">
          <div class="left-toolbar">
            <n-space>
              <n-button size="tiny" :disabled="loading || !selectedPath" @click="openSelectedInExplorer">打开所选</n-button>
              <n-button size="tiny" type="error" :disabled="loading || !selectedPath"
                @click="deleteSelected">删除所选</n-button>
            </n-space>
          </div>

          <div class="tree">
            <n-spin :show="loadingTree">
              <n-tree :data="treeData" key-field="key" label-field="label" children-field="children"
                :default-expand-all="false" :expanded-keys="expandedKeys" :selected-keys="selectedKeys"
                :render-prefix="renderPrefix" block-line @update:expanded-keys="handleExpandedKeysUpdate"
                @update:selected-keys="handleSelectedKeysUpdate" />
            </n-spin>
          </div>
        </div>

        <div class="right">
          <n-card size="small" title="详情">
            <n-space vertical>
              <n-text depth="3">选中路径：</n-text>
              <n-text>{{ selectedPath || '（未选择）' }}</n-text>
            </n-space>
          </n-card>
        </div>
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { h, onMounted, ref } from 'vue'
import { NButton, NCard, NMessageProvider, NSpace, NSpin, NText, NTree, useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { Folder20Regular, Document20Regular } from '@vicons/fluent'
import { NIcon } from 'naive-ui'

const message = useMessage()

const formatInvokeError = (e) => {
  const msg = e?.message ?? (typeof e?.toString === 'function' ? e.toString() : String(e))
  const s = typeof msg === 'string' ? msg.trim() : ''
  return s || '操作失败'
}

const loading = ref(false)
const loadingTree = ref(false)

const resourceRoot = ref('')
const treeData = ref([])

const expandedKeys = ref([])
const selectedKeys = ref([])
const selectedPath = ref('')

const toTreeNode = (n) => {
  const isDir = Boolean(n?.is_directory)
  const key = String(n?.id || '')
  return {
    key,
    label: String(n?.name || ''),
    isDir,
    children: isDir ? [] : undefined,
  }
}

const loadRoot = async () => {
  const root = await invoke('get_image_bed_root')
  resourceRoot.value = typeof root === 'string' ? root : ''
}

const loadChildren = async (dirPath) => {
  const nodes = await invoke('get_file_tree', { path: dirPath })
  if (!Array.isArray(nodes)) return []
  return nodes.map(toTreeNode)
}

const refresh = async () => {
  try {
    loadingTree.value = true
    await loadRoot()
    if (!resourceRoot.value) {
      treeData.value = []
      return
    }
    const children = await loadChildren(resourceRoot.value)
    treeData.value = children
    expandedKeys.value = []
    selectedKeys.value = []
    selectedPath.value = ''
    console.log('[ResourceRepoManager] refreshed', {
      root: resourceRoot.value,
      count: treeData.value.length,
    })
  } catch (e) {
    console.error('[ResourceRepoManager] refresh failed', e)
    message.error('刷新失败')
  } finally {
    loadingTree.value = false
  }
}

const findNodeByKey = (nodes, key) => {
  for (const n of nodes || []) {
    if (n?.key === key) return n
    const hit = findNodeByKey(n?.children, key)
    if (hit) return hit
  }
  return null
}

const renderPrefix = ({ option }) => {
  const icon = option?.isDir ? Folder20Regular : Document20Regular
  return h(NIcon, { component: icon, size: 16 })
}

const handleSelectedKeysUpdate = (keys) => {
  selectedKeys.value = keys
  selectedPath.value = keys?.[0] ? String(keys[0]) : ''
}

const handleExpandedKeysUpdate = async (keys) => {
  expandedKeys.value = keys

  const newest = keys?.[keys.length - 1]
  if (!newest) return

  const node = findNodeByKey(treeData.value, String(newest))
  if (!node || !node.isDir) return
  if (Array.isArray(node.children) && node.children.length > 0) return

  try {
    loadingTree.value = true
    const children = await loadChildren(node.key)
    node.children = children
  } catch (e) {
    console.error('[ResourceRepoManager] load children failed', e)
    message.error('加载目录失败')
  } finally {
    loadingTree.value = false
  }
}

const openInExplorer = async () => {
  try {
    if (!resourceRoot.value) return
    await invoke('open_in_file_explorer', { path: resourceRoot.value })
  } catch (e) {
    console.error('[ResourceRepoManager] open explorer failed', e)
    message.error('打开失败')
  }
}

const openSelectedInExplorer = async () => {
  try {
    if (!selectedPath.value) return
    await invoke('open_in_file_explorer', { path: selectedPath.value })
  } catch (e) {
    console.error('[ResourceRepoManager] open selected failed', e)
    message.error('打开失败')
  }
}

const deleteSelected = async () => {
  try {
    if (!selectedPath.value) return
    loading.value = true
    await invoke('delete_path', { path: selectedPath.value })
    message.success('已删除')
    await refresh()
  } catch (e) {
    console.error('[ResourceRepoManager] delete failed', e)
    message.error('删除失败')
  } finally {
    loading.value = false
  }
}

const testConnection = async () => {
  try {
    loading.value = true
    const res = await invoke('resource_repo_test_connection')
    message.success(typeof res === 'string' ? res : '连接正常')
  } catch (e) {
    console.error('[ResourceRepoManager] test connection failed', e)
    message.error(formatInvokeError(e))
  } finally {
    loading.value = false
  }
}

const syncNow = async () => {
  try {
    loading.value = true
    const res = await invoke('resource_repo_sync_now')
    message.success(typeof res === 'string' ? res : '同步完成')
  } catch (e) {
    console.error('[ResourceRepoManager] sync failed', e)
    message.error(formatInvokeError(e))
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await refresh()
})
</script>

<style scoped>
.page {
  height: 100vh;
  padding: 16px;
  box-sizing: border-box;
  background: var(--guaxi-app-bg);
}

.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
}

.title {
  font-size: 18px;
  font-weight: 700;
}

.path {
  color: var(--guaxi-text-secondary);
  font-size: 12px;
  margin-bottom: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.main {
  display: grid;
  grid-template-columns: 1fr 360px;
  gap: 12px;
  height: calc(100vh - 110px);
}

.left {
  border: 1px solid var(--guaxi-border);
  border-radius: 8px;
  overflow: hidden;
  background: var(--guaxi-panel-bg);
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.left-toolbar {
  padding: 8px;
  border-bottom: 1px solid var(--guaxi-border);
  background: var(--guaxi-header-bg);
}

.tree {
  flex: 1;
  min-height: 0;
  padding: 8px;
  overflow: auto;
}

.right {
  min-height: 0;
}
</style>