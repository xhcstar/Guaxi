<!-- components/CreateKnowledgeBaseForm.vue -->
<template>
  <n-modal v-model:show="knowledgeBaseStore.showForm" preset="card" :bordered="false"
    style="width: 600px; max-width: 90vw;" title="创建新知识库" size="large" class="knowledge-base-modal"
    @after-leave="resetForm">
    <n-form ref="formRef" :model="knowledgeBaseStore.formData" :rules="rules" label-placement="top" label-width="auto"
      size="large" @submit.prevent="handleSubmit">
      <n-form-item label="知识库名称" path="knowledgeBaseName" required>
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseName" placeholder="例如：产品文档、学习笔记" clearable>
          <template #prefix>
            <n-icon :component="BookOutline" />
          </template>
        </n-input>
      </n-form-item>

      <n-form-item label="标签" path="knowledgeBaseTags">
        <n-dynamic-tags v-model:value="tags" placeholder="输入标签后按回车" :max="5" />
      </n-form-item>

      <n-form-item label="描述" path="knowledgeBaseDesc">
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseDesc" type="textarea"
          placeholder="简单描述这个知识库的用途和内容" :autosize="{ minRows: 3, maxRows: 5 }" show-count :maxlength="200" />
      </n-form-item>

      <div class="form-footer">
        <n-button secondary size="large" @click="closeForm">
          取消
        </n-button>
        <n-button type="primary" size="large" attr-type="submit" :loading="submitting">
          <template #icon>
            <n-icon :component="CheckmarkCircleOutline" />
          </template>
          创建知识库
        </n-button>
      </div>
    </n-form>
  </n-modal>
</template>

<script setup>
import { ref, computed } from 'vue';
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { useMessage } from 'naive-ui';
import {
  BookOutline,
  CheckmarkCircleOutline,
} from '@vicons/ionicons5';

const knowledgeBaseStore = useKnowledgeBaseStore();
const message = useMessage();
const formRef = ref(null);
const submitting = ref(false);

// 将逗号分隔的标签字符串转换为数组
const tags = computed({
  get: () => {
    return knowledgeBaseStore.formData.knowledgeBaseTags
      ? knowledgeBaseStore.formData.knowledgeBaseTags.split(',').filter(tag => tag.trim())
      : [];
  },
  set: (newTags) => {
    knowledgeBaseStore.formData.knowledgeBaseTags = newTags.join(',');
  }
});

const rules = {
  knowledgeBaseName: [
    { required: true, message: '请输入知识库名称', trigger: 'blur' },
    { min: 2, max: 30, message: '长度在2到30个字符之间', trigger: 'blur' }
  ],
  knowledgeBaseDesc: [
    { max: 200, message: '描述不能超过200个字符', trigger: 'blur' }
  ]
};

const handleSubmit = async () => {
  submitting.value = true;
  try {
    await formRef.value?.validate();
    await knowledgeBaseStore.createKnowledgeBase();
    message.success('知识库创建成功');
    closeForm();
  } catch (e) {
    console.error('知识库创建失败:', e);
    message.error(e.message || '知识库创建失败');
  } finally {
    submitting.value = false;
  }
};

const resetForm = () => {
  formRef.value?.restoreValidation();
  knowledgeBaseStore.resetFormData();
};

const closeForm = () => {
  knowledgeBaseStore.toggleForm(false);
};
</script>

<style scoped>
.knowledge-base-modal {
  border-radius: 12px;
  overflow: hidden;
}

.knowledge-base-modal :deep(.n-card-header) {
  padding: 20px 24px;
  border-bottom: 1px solid var(--n-divider-color);
}

.knowledge-base-modal :deep(.n-card__content) {
  padding: 24px;
}

.n-form-item {
  margin-bottom: 24px;
}

.n-form-item:last-child {
  margin-bottom: 0;
}

.form-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 32px;
  padding-top: 16px;
  border-top: 1px solid var(--n-divider-color);
}
</style>