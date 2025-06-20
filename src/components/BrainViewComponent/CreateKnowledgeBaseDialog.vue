<template>
  <n-dialog
    v-model:show="showDialog"
    title="新建知识库"
    preset="dialog"
    positive-text="创建"
    negative-text="取消"
    @positive-click="handleCreate"
    @negative-click="showDialog = false"
  >
    <n-form :model="form">
      <n-form-item label="知识库名字" path="knowledgeBaseName" required>
        <n-input v-model:value="form.knowledgeBaseName" placeholder="请输入知识库名字" />
      </n-form-item>
      <n-form-item label="知识库标签" path="knowledgeBaseTags">
        <n-input v-model:value="form.knowledgeBaseTags" placeholder="多个标签用逗号分隔" />
      </n-form-item>
      <n-form-item label="知识库描述" path="knowledgeBaseDesc">
        <n-input v-model:value="form.knowledgeBaseDesc" type="textarea" placeholder="请输入知识库描述" />
      </n-form-item>
    </n-form>
  </n-dialog>
</template>

<script setup>
import { ref } from 'vue'
import { useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore'

const props = defineProps({
  show: Boolean
})
const emit = defineEmits(['update:show', 'created'])

const showDialog = ref(props.show)
const message = useMessage()
const store = useKnowledgeBaseStore() // 使用 Pinia Store

const form = ref({
  knowledgeBaseName: '',
  knowledgeBaseTags: '',
  knowledgeBaseDesc: ''
})

// 监听父组件的 show 变化
watch(() => props.show, (val) => {
  showDialog.value = val
})

// 监听内部 showDialog 变化，通知父组件
watch(showDialog, (val) => {
  emit('update:show', val)
})

const handleCreate = async () => {
  if (!form.value.knowledgeBaseName) {
    message.error('知识库名字不能为空')
    return false
  }
  try {
    await invoke('create_knowledge_base', {
      knowledge_base_name: form.value.knowledgeBaseName,
      knowledge_base_tags: form.value.knowledgeBaseTags,
      knowledge_base_description: form.value.knowledgeBaseDesc
    })
    message.success('知识库创建成功')
    showDialog.value = false
    form.value = { knowledgeBaseName: '', knowledgeBaseTags: '', knowledgeBaseDesc: '' } // 重置表单
    await store.refresh() // 刷新知识库列表
    emit('created') // 通知父组件
    return true
  } catch (e) {
    message.error('知识库创建失败: ' + e)
    return false
  }
}
</script>