<template>
  <n-message-provider placement="bottom">
    <div class="page">
      <div class="header">
        <div class="title">模板管理器</div>
        <n-space>
          <n-button size="small" @click="refresh" :loading="loading">刷新</n-button>
          <n-button size="small" @click="importFromFile" :disabled="loading">导入 Markdown 文件</n-button>
          <n-button size="small" @click="newTemplate" :disabled="loading">新建</n-button>
        </n-space>
      </div>

      <div class="path" :title="templateRepoPath">
        {{ templateRepoPath || '未配置模板库路径' }}
      </div>

      <div class="main">
        <div class="left">
          <n-input v-model:value="filter" size="small" placeholder="搜索模板..." clearable />
          <n-collapse accordion>
            <n-collapse-item
              v-for="group in groupedTemplates"
              :key="group.type"
              :title="`${group.type} (${group.items.length})`"
              :name="group.type"
            >
              <n-list hoverable clickable>
                <n-list-item
                  v-for="t in group.items"
                  :key="t.name"
                  :class="{ active: t.name === selectedName }"
                  @click="selectTemplate(t.name)"
                >
                  <div class="row">
                    <div class="row-name">{{ t.name }}</div>
                    <n-button
                      size="tiny"
                      tertiary
                      @click.stop="insertToEditor(t.name)"
                      :disabled="loading"
                    >
                      插入
                    </n-button>
                  </div>
                </n-list-item>
              </n-list>
            </n-collapse-item>
          </n-collapse>
        </div>

        <div class="right">
          <n-form label-placement="left" label-width="auto" :show-require-mark="false">
            <n-form-item label="名称">
              <n-input v-model:value="editName" :disabled="loading" placeholder="例如：会议纪要" />
            </n-form-item>
            <n-form-item label="类型">
              <n-input v-model:value="editType" :disabled="loading" placeholder="例如：会议 / 日记 / TODO" />
            </n-form-item>
            <n-form-item label="内容">
              <n-input
                v-model:value="editContent"
                type="textarea"
                :disabled="loading"
                placeholder="输入模板内容（Markdown）"
                :autosize="{ minRows: 16, maxRows: 28 }"
              />
            </n-form-item>
          </n-form>

          <n-space justify="end">
            <n-button type="primary" :disabled="loading || !editName" @click="save">保存</n-button>
            <n-button
              tertiary
              type="error"
              :disabled="loading || !selectedName"
              @click="remove"
            >
              删除
            </n-button>
          </n-space>
        </div>
      </div>
    </div>
  </n-message-provider>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue'
import { NButton, NCollapse, NCollapseItem, NForm, NFormItem, NInput, NList, NListItem, NSpace, createDiscreteApi } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const { message } = createDiscreteApi(['message'])

const loading = ref(false)
const templateRepoPath = ref('')
const templates = ref([])
const filter = ref('')

const selectedName = ref('')
const editName = ref('')
const editType = ref('未分类')
const editContent = ref('')

const refresh = async () => {
  try {
    loading.value = true
    const [path, list] = await Promise.all([
      invoke('get_template_repo_path'),
      invoke('list_templates')
    ])
    templateRepoPath.value = typeof path === 'string' ? path : ''
    templates.value = Array.isArray(list) ? list : []
  } catch (e) {
    console.error('加载模板失败:', e)
    message.error('加载模板失败')
  } finally {
    loading.value = false
  }
}

const filtered = computed(() => {
  const q = filter.value.trim().toLowerCase()
  if (!q) return templates.value
  return templates.value.filter(t => {
    const n = String(t?.name || '').toLowerCase()
    const tp = String(t?.template_type || '').toLowerCase()
    return n.includes(q) || tp.includes(q)
  })
})

const groupedTemplates = computed(() => {
  const map = new Map()
  for (const t of filtered.value) {
    const tp = String(t?.template_type || '未分类') || '未分类'
    if (!map.has(tp)) map.set(tp, [])
    map.get(tp).push(t)
  }
  return Array.from(map.entries()).map(([type, items]) => ({
    type,
    items: items.slice().sort((a, b) => String(a.name).localeCompare(String(b.name)))
  }))
})

const newTemplate = () => {
  selectedName.value = ''
  editName.value = ''
  editType.value = '未分类'
  editContent.value = ''
}

const selectTemplate = async (name) => {
  try {
    loading.value = true
    const data = await invoke('read_template', { name })
    selectedName.value = data?.name || name
    editName.value = data?.name || name
    editType.value = data?.template_type || '未分类'
    editContent.value = data?.content || ''
  } catch (e) {
    console.error('读取模板失败:', e)
    message.error('读取模板失败')
  } finally {
    loading.value = false
  }
}

const save = async () => {
  const name = editName.value.trim()
  if (!name) {
    message.warning('请输入模板名称')
    return
  }

  try {
    loading.value = true

    if (selectedName.value && selectedName.value !== name) {
      await invoke('rename_template', { oldName: selectedName.value, newName: name })
      selectedName.value = name
    }

    await invoke('save_template', {
      name,
      templateType: editType.value.trim() || '未分类',
      content: editContent.value || ''
    })

    message.success('已保存')
    await refresh()
  } catch (e) {
    console.error('保存模板失败:', e)
    message.error('保存模板失败')
  } finally {
    loading.value = false
  }
}

const remove = async () => {
  if (!selectedName.value) return
  const ok = window.confirm(`确认删除模板「${selectedName.value}」？`)
  if (!ok) return

  try {
    loading.value = true
    await invoke('delete_template', { name: selectedName.value })
    message.success('已删除')
    newTemplate()
    await refresh()
  } catch (e) {
    console.error('删除模板失败:', e)
    message.error('删除模板失败')
  } finally {
    loading.value = false
  }
}

const insertToEditor = async (name) => {
  try {
    loading.value = true
    const data = await invoke('read_template', { name })
    const content = typeof data?.content === 'string' ? data.content : ''
    if (!content) return
    await invoke('emit_template_insert', { content })
    message.success('已发送到编辑器窗口')
  } catch (e) {
    console.error('插入失败:', e)
    message.error('插入失败')
  } finally {
    loading.value = false
  }
}

const importFromFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Markdown', extensions: ['md', 'markdown'] }]
    })
    if (!selected) return

    const defaultName = String(selected).split(/[\\/]/).pop()?.replace(/\.(md|markdown)$/i, '') || '新模板'
    const name = window.prompt('模板名称', defaultName) || ''
    if (!name.trim()) return
    const type = window.prompt('模板类型（用于分类）', '未分类') || '未分类'

    loading.value = true
    await invoke('import_template_from_file', {
      sourcePath: selected,
      name: name.trim(),
      templateType: type.trim() || '未分类'
    })
    message.success('导入成功')
    await refresh()
  } catch (e) {
    console.error('导入失败:', e)
    message.error('导入失败')
  } finally {
    loading.value = false
  }
}

onMounted(refresh)
</script>

<style scoped>
.page {
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 12px;
  box-sizing: border-box;
  gap: 10px;
  overflow: hidden;
  background: var(--guaxi-content-bg);
  color: var(--guaxi-text-primary);
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

.left :deep(.n-collapse) {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.right {
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--guaxi-border);
  border-radius: 10px;
  padding: 12px;
  box-sizing: border-box;
  background: var(--guaxi-panel-bg);
  box-shadow: 0 10px 28px rgba(16, 24, 40, 0.08);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.row-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.active {
  background: rgba(45, 212, 191, 0.12);
}

:global(:root[data-guaxi-theme='dark']) .active {
  background: rgba(97, 175, 239, 0.16);
}
</style>