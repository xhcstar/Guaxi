<template>
  <n-modal v-model:show="knowledgeBaseStore.showForm" preset="card" :bordered="false"
    style="width: 600px; max-width: 90vw;" title="创建新知识库" size="large" class="knowledge-base-modal"
    @after-leave="resetForm">
    <!-- 使用 Naive UI 的 Form 组件构建表单 -->
    <n-form ref="formRef" :model="knowledgeBaseStore.formData" :rules="rules" label-placement="top" label-width="auto"
      size="large" @submit.prevent="handleSubmit">
      <!-- 知识库名称输入框 -->
      <n-form-item label="知识库名称" path="knowledgeBaseName" required>
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseName" placeholder="例如：产品文档、学习笔记" clearable>
          <template #prefix>
            <n-icon :component="BookOutline" />
          </template>
        </n-input>
      </n-form-item>

      <!-- 标签输入区域 -->
      <n-form-item label="标签" path="knowledgeBaseTags">
        <n-dynamic-tags v-model:value="tags" placeholder="输入标签后按回车" :max="5" />
      </n-form-item>

      <!-- 描述文本区域 -->
      <n-form-item label="描述" path="knowledgeBaseDesc">
        <n-input v-model:value="knowledgeBaseStore.formData.knowledgeBaseDesc" type="textarea"
          placeholder="简单描述这个知识库的用途和内容" :autosize="{ minRows: 3, maxRows: 5 }" show-count :maxlength="200" />
      </n-form-item>

      <!-- 表单操作按钮 -->
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

// 使用知识库 store 和消息组件
const knowledgeBaseStore = useKnowledgeBaseStore();
const message = useMessage();

// 表单引用和提交状态
const formRef = ref(null);
const submitting = ref(false);

// 将标签字符串转换为数组（双向计算属性）
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

// 表单验证规则
const rules = {
  knowledgeBaseName: [
    { required: true, message: '请输入知识库名称', trigger: 'blur' },
    { min: 2, max: 30, message: '长度在2到30个字符之间', trigger: 'blur' }
  ],
  knowledgeBaseDesc: [
    { max: 200, message: '描述不能超过200个字符', trigger: 'blur' }
  ]
};

// 提交表单处理函数
const handleSubmit = async () => {
  submitting.value = true;
  try {
    // 验证表单
    await formRef.value?.validate();

    // 调用 store 方法创建知识库
    await knowledgeBaseStore.createKnowledgeBase();

    // 显示成功消息
    message.success('知识库创建成功');

    // 关闭表单
    closeForm();
  } catch (e) {
    console.error('知识库创建失败:', e);

    // 显示错误消息（使用 store 返回的错误或默认消息）
    message.error(e.message || '知识库创建失败');
  } finally {
    submitting.value = false;
  }
};

// 重置表单（在模态框关闭后调用）
const resetForm = () => {
  // 重置表单验证状态
  formRef.value?.restoreValidation();

  // 调用 store 的 resetForm 方法
  knowledgeBaseStore.resetForm();
};

// 关闭表单
const closeForm = () => {
  knowledgeBaseStore.toggleForm(false);
};
</script>

<style scoped>
/* 模态框样式 */
.knowledge-base-modal {
  border-radius: 12px;
  overflow: hidden;
}

/* 模态框头部样式 */
.knowledge-base-modal :deep(.n-card-header) {
  padding: 20px 24px;
  border-bottom: 1px solid var(--n-divider-color);
}

/* 模态框内容区域样式 */
.knowledge-base-modal :deep(.n-card__content) {
  padding: 24px;
}

/* 表单项间距 */
.n-form-item {
  margin-bottom: 24px;
}

/* 最后一个表单项不需要下边距 */
.n-form-item:last-child {
  margin-bottom: 0;
}

/* 表单底部操作按钮区域 */
.form-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 32px;
  padding-top: 16px;
  border-top: 1px solid var(--n-divider-color);
}
</style>