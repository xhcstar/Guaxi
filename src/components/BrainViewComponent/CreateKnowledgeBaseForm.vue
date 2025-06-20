<!-- components/CreateKnowledgeBaseForm.vue -->
<template>
  <div v-if="knowledgeBaseStore.showForm" class="knowledge-base-form">
    <n-form :model="knowledgeBaseStore.formData" @submit.prevent="handleSubmit">
      <n-form-item label="知识库名字" path="knowledgeBaseName" required>
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseName" placeholder="请输入知识库名字" />
      </n-form-item>
      <n-form-item label="知识库标签" path="knowledgeBaseTags">
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseTags" placeholder="多个标签用逗号分隔" />
      </n-form-item>
      <n-form-item label="知识库描述" path="knowledgeBaseDesc">
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseDesc" type="textarea" placeholder="请输入知识库描述" />
      </n-form-item>
      <div class="form-actions">
        <n-button type="primary" attr-type="submit">创建</n-button>
        <n-button @click="closeForm">取消</n-button>
      </div>
    </n-form>
  </div>
</template>

<script setup>
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { useMessage } from 'naive-ui';
import { onMounted } from 'vue';

const knowledgeBaseStore = useKnowledgeBaseStore();
const message = useMessage();

onMounted(() => {
  console.log('表单组件已挂载，初始可见性:', knowledgeBaseStore.showForm);
});

const handleSubmit = async () => {
  console.log('提交表单数据:', JSON.stringify(knowledgeBaseStore.formData));
  try {
    await knowledgeBaseStore.createKnowledgeBase();
    message.success('知识库创建成功');
    knowledgeBaseStore.toggleForm(false);
  } catch (e) {
    console.error('知识库创建失败:', e);
    message.error(e.message || '知识库创建失败');
  }
};

const closeForm = () => {
  knowledgeBaseStore.toggleForm(false);
};
</script>

<style scoped>
.knowledge-base-form {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 9999;
  padding: 24px;
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.2);
  width: 500px;
  max-width: 90vw;
  max-height: 90vh;
  overflow-y: hidden;
}

.form-actions {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  margin-top: 24px;
}
</style>