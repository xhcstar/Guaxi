<template>
  <div class="guaxi-plugin-manager">
    <div class="gpm-toolbar">
      <n-button size="small" @click="refresh">刷新</n-button>
      <n-button size="small" type="primary" @click="importPlugin">导入 .zip</n-button>
      <n-input v-model:value="newName" size="small" placeholder="新插件名称" class="gpm-new-name" />
      <n-button size="small" :disabled="!newName.trim()" @click="createNew">新建</n-button>
      <n-button size="small" text @click="openDir">打开插件目录</n-button>
    </div>

    <div v-if="loading" class="gpm-hint">加载中…</div>
    <div v-else-if="!plugins.length" class="gpm-hint">还没有插件。点“新建”或“导入 .zip”添加。</div>
    <div v-else class="gpm-list">
      <div v-for="p in plugins" :key="p.id" class="gpm-item">
        <div class="gpm-item__main">
          <div class="gpm-item__title">
            <span class="gpm-item__name">{{ p.name }}</span>
            <span v-for="s in p.surfaces" :key="s" class="gpm-tag">{{ surfaceLabel(s) }}</span>
          </div>
          <div class="gpm-item__desc">{{ p.description || p.id }}</div>
        </div>
        <div class="gpm-item__ops">
          <n-switch size="small" :value="p.enabled" @update:value="(v) => toggle(p, v)" />
          <n-button size="tiny" @click="edit(p)">编辑</n-button>
          <n-button size="tiny" @click="exportOne(p)">导出</n-button>
          <n-button size="tiny" type="error" ghost @click="remove(p)">删除</n-button>
        </div>
      </div>
    </div>
  </div>
</template>

<!-- SCRIPT_PLACEHOLDER -->

<script setup>
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit as emitTauriEvent } from '@tauri-apps/api/event'
import { open as openFileDialog, confirm as confirmDialog } from '@tauri-apps/plugin-dialog'
import { useMessage } from 'naive-ui'

const message = useMessage()
const plugins = ref([])
const loading = ref(false)
const newName = ref('')

const surfaceLabel = (s) =>
  ({ editor: '编辑器', repo: '文件库', 'knowledge-base': '知识库', agent: 'Agent' }[String(s)] || String(s))

const refresh = async () => {
  loading.value = true
  try {
    plugins.value = (await invoke('list_all_plugins')) || []
  } catch (e) {
    message.error('加载插件失败:' + String(e?.message || e))
    plugins.value = []
  } finally {
    loading.value = false
  }
}

const toggle = async (p, v) => {
  try {
    await invoke('set_plugin_enabled', { id: p.id, enabled: !!v })
    p.enabled = !!v
  } catch (e) {
    message.error('切换失败:' + String(e?.message || e))
  }
}

const edit = async (p) => {
  try {
    await emitTauriEvent('open_file_in_home', { path: p.entry_path, lineNumber: 1 })
    message.info('已在编辑器打开插件源码,改完 Ctrl+S 保存即可')
  } catch (e) {
    message.error('打开失败:' + String(e?.message || e))
  }
}

const exportOne = async (p) => {
  try {
    const path = await invoke('export_plugin_to_zip', { id: p.id })
    message.success('已导出到:' + path)
  } catch (e) {
    message.error('导出失败:' + String(e?.message || e))
  }
}

const remove = async (p) => {
  const ok = await confirmDialog(`确定删除插件“${p.name}”?此操作不可撤销。`, {
    title: '删除插件',
    kind: 'warning',
  })
  if (!ok) return
  try {
    await invoke('delete_plugin', { id: p.id })
    message.success('已删除')
    await refresh()
  } catch (e) {
    message.error('删除失败:' + String(e?.message || e))
  }
}

const importPlugin = async () => {
  try {
    const file = await openFileDialog({
      multiple: false,
      filters: [{ name: 'Plugin Package', extensions: ['zip'] }],
    })
    if (!file) return
    const info = await invoke('import_plugin_from_zip', { zipPath: String(file) })
    message.success('已导入插件:' + (info?.name || info?.id || ''))
    await refresh()
  } catch (e) {
    message.error('导入失败:' + String(e?.message || e))
  }
}

const createNew = async () => {
  const name = newName.value.trim()
  if (!name) return
  try {
    const info = await invoke('create_plugin', { name, surface: 'editor' })
    newName.value = ''
    await refresh()
    message.success('已创建插件:' + (info?.name || ''))
    if (info?.entry_path) {
      await emitTauriEvent('open_file_in_home', { path: info.entry_path, lineNumber: 1 })
    }
  } catch (e) {
    message.error('创建失败:' + String(e?.message || e))
  }
}

const openDir = async () => {
  try {
    const dir = await invoke('get_editor_plugins_dir')
    await invoke('open_in_file_explorer', { path: dir })
  } catch (e) {
    message.error('打开目录失败:' + String(e?.message || e))
  }
}

onMounted(refresh)
</script>

<!-- STYLE_PLACEHOLDER -->

<style scoped>
.guaxi-plugin-manager {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.gpm-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.gpm-new-name {
  max-width: 180px;
}
.gpm-hint {
  opacity: 0.6;
  font-size: 13px;
  padding: 8px 0;
}
.gpm-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.gpm-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border: 1px solid var(--guaxi-border-color, #e0e0e6);
  border-radius: 8px;
}
.gpm-item__main {
  min-width: 0;
  flex: 1 1 auto;
}
.gpm-item__title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-weight: 600;
}
.gpm-item__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gpm-tag {
  font-size: 11px;
  font-weight: 400;
  opacity: 0.7;
  border: 1px solid var(--guaxi-border-color, #e0e0e6);
  border-radius: 4px;
  padding: 0 5px;
}
.gpm-item__desc {
  margin-top: 2px;
  font-size: 12px;
  opacity: 0.65;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gpm-item__ops {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
}
</style>


