<template>
  <n-modal
    v-model:show="knowledgeBaseStore.showForm"
    preset="card"
    :bordered="false"
    :closable="false"
    :mask-closable="!submitting"
    style="width: 640px; max-width: calc(100vw - 32px);"
    size="large"
    class="knowledge-base-modal"
    @after-leave="resetForm"
  >
    <div class="kb-create-shell">
      <div class="kb-create-header">
        <div class="kb-create-icon" aria-hidden="true">
          <n-icon :component="BookOutline" size="22" />
        </div>
        <div class="kb-create-title-group">
          <div class="kb-create-eyebrow">Knowledge Base</div>
          <h2 class="kb-create-title">创建新知识库</h2>
          <p class="kb-create-subtitle">
            为当前工作区创建一个独立资料空间，后续可继续添加文档、摘录和任务素材。
          </p>
        </div>
      </div>

      <!-- 设计思路：
       * 1. 表单信息按“名称 -> 标签 -> 描述”的创建流程垂直组织，减少旧样式里大块空白和默认表单感。
       * 2. 字体栈与主编辑器保持一致，中文和代码式路径在同一面板中不会显得跳脱。
       * 3. 操作区固定在底部，提交/取消状态清晰，不改变原有 store 创建流程。
       -->
      <n-form
        ref="formRef"
        class="kb-create-form"
        :model="knowledgeBaseStore.formData"
        :rules="rules"
        label-placement="top"
        label-width="auto"
        size="medium"
        :show-require-mark="false"
        @submit.prevent="handleSubmit"
      >
        <div class="kb-field-block kb-field-block-primary">
          <n-form-item label="知识库名称" path="knowledgeBaseName" required>
            <n-input
              v-model:value="knowledgeBaseStore.formData.knowledgeBaseName"
              placeholder="例如：产品文档、学习笔记"
              clearable
            >
              <template #prefix>
                <n-icon :component="BookOutline" />
              </template>
            </n-input>
          </n-form-item>
          <p class="field-hint">名称会用于知识库列表展示，建议简短、具体，便于快速定位。</p>
        </div>

        <div class="kb-field-block">
          <n-form-item label="标签" path="knowledgeBaseTags">
            <n-dynamic-tags v-model:value="tags" placeholder="输入标签后按回车" :max="5" />
          </n-form-item>
          <p class="field-hint">最多 5 个标签，可用于后续筛选和分组。</p>
        </div>

        <div class="kb-field-block">
          <n-form-item label="描述" path="knowledgeBaseDesc">
            <n-input
              v-model:value="knowledgeBaseStore.formData.knowledgeBaseDesc"
              type="textarea"
              placeholder="简单描述这个知识库的用途和内容"
              :autosize="{ minRows: 4, maxRows: 6 }"
              show-count
              :maxlength="200"
            />
          </n-form-item>
        </div>

        <div class="form-footer">
          <n-button secondary size="medium" :disabled="submitting" @click="closeForm">
            取消
          </n-button>
          <n-button type="primary" size="medium" attr-type="submit" :loading="submitting">
            <template #icon>
              <n-icon :component="CheckmarkCircleOutline" />
            </template>
            创建知识库
          </n-button>
        </div>
      </n-form>
    </div>
  </n-modal>
</template>

<script setup>
import { createLogger } from '../../utils/logger'
import { ref, computed } from 'vue';
import { useKnowledgeBaseStore } from '../../stores/knowledgeBaseStore';
import { useMessage } from 'naive-ui';
import {
  BookOutline,
  CheckmarkCircleOutline,
} from '@vicons/ionicons5';

const logger = createLogger('CreateKnowledgeBaseForm')
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
    logger.info('开始创建知识库', {
      name: knowledgeBaseStore.formData.knowledgeBaseName,
      tagCount: tags.value.length,
      hasDescription: Boolean(knowledgeBaseStore.formData.knowledgeBaseDesc?.trim())
    });

    // 验证表单
    await formRef.value?.validate();

    // 调用 store 方法创建知识库
    await knowledgeBaseStore.createKnowledgeBase();

    // 显示成功消息
    message.success('知识库创建成功');
    logger.info('知识库创建成功');

    // 关闭表单
    closeForm(true);
  } catch (e) {
    logger.error('知识库创建失败:', e);

    // 显示错误消息（使用 store 返回的错误或默认消息）
    message.error(e.message || '知识库创建失败');
  } finally {
    submitting.value = false;
  }
};

// 重置表单（在模态框关闭后调用）
const resetForm = () => {
  logger.debug('重置新建知识库表单');
  // 重置表单验证状态
  formRef.value?.restoreValidation();

  // 调用 store 的 resetForm 方法
  knowledgeBaseStore.resetForm();
};

// 关闭表单
const closeForm = (force = false) => {
  if (submitting.value && !force) {
    logger.info('创建中，忽略关闭新建知识库表单');
    return;
  }
  logger.info('关闭新建知识库表单', { force });
  knowledgeBaseStore.toggleForm(false);
};
</script>

<style scoped>
/* 模态框样式 */
.knowledge-base-modal {
  --kb-create-font: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New';
  --kb-create-hairline: rgba(148, 163, 184, 0.18);
  --kb-create-hairline-soft: rgba(148, 163, 184, 0.1);
  --kb-create-surface: color-mix(in srgb, var(--guaxi-panel-bg, #1f252d) 94%, transparent);
  border-radius: 10px;
  overflow: hidden;
  font-family: var(--kb-create-font);
}

.knowledge-base-modal :deep(.n-card),
.knowledge-base-modal :deep(.n-card-header),
.knowledge-base-modal :deep(.n-card__content),
.knowledge-base-modal :deep(.n-form),
.knowledge-base-modal :deep(.n-form-item-label),
.knowledge-base-modal :deep(.n-input__input-el),
.knowledge-base-modal :deep(.n-input__textarea-el),
.knowledge-base-modal :deep(.n-base-selection),
.knowledge-base-modal :deep(.n-dynamic-tags),
.knowledge-base-modal :deep(.n-button),
.knowledge-base-modal :deep(.n-tag) {
  font-family: var(--kb-create-font);
}

.knowledge-base-modal :deep(.n-card-header) {
  display: none;
}

.knowledge-base-modal :deep(.n-card__content) {
  padding: 0;
}

.knowledge-base-modal :deep(.n-card) {
  background: var(--guaxi-panel-bg);
  border: 1px solid var(--guaxi-border);
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.28);
}

.kb-create-shell {
  background:
    radial-gradient(circle at top right, rgba(33, 189, 236, 0.12), transparent 34%),
    var(--kb-create-surface);
}

.kb-create-header {
  display: flex;
  gap: 14px;
  padding: 24px 26px 20px;
  border-bottom: 1px solid var(--kb-create-hairline);
  background: linear-gradient(180deg, rgba(148, 163, 184, 0.08), transparent);
}

.kb-create-icon {
  flex: 0 0 42px;
  width: 42px;
  height: 42px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 10px;
  color: var(--n-color-primary, #1890ff);
  background: rgba(33, 189, 236, 0.12);
  box-shadow: inset 0 0 0 1px rgba(33, 189, 236, 0.18);
}

.kb-create-title-group {
  min-width: 0;
}

.kb-create-eyebrow {
  margin-bottom: 4px;
  font-size: 11px;
  line-height: 1;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--guaxi-text-secondary);
}

.kb-create-title {
  margin: 0;
  font-size: 20px;
  line-height: 1.25;
  color: var(--guaxi-text-primary);
}

.kb-create-subtitle {
  margin: 8px 0 0;
  max-width: 500px;
  font-size: 13px;
  line-height: 1.65;
  color: var(--guaxi-text-secondary);
}

.kb-create-form {
  padding: 20px 26px 22px;
}

.kb-field-block {
  padding: 14px 14px 12px;
  margin-bottom: 12px;
  border: 1px solid var(--kb-create-hairline-soft);
  border-radius: 8px;
  background: rgba(148, 163, 184, 0.04);
}

.kb-field-block-primary {
  border-color: rgba(33, 189, 236, 0.22);
  background: rgba(33, 189, 236, 0.055);
}

.field-hint {
  margin: -8px 0 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--guaxi-text-secondary);
}

.knowledge-base-modal :deep(.n-form-item) {
  margin-bottom: 0;
}

.knowledge-base-modal :deep(.n-form-item-label) {
  padding-bottom: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--guaxi-text-primary);
}

.knowledge-base-modal :deep(.n-input),
.knowledge-base-modal :deep(.n-base-selection) {
  --n-border-radius: 7px;
}

.knowledge-base-modal :deep(.n-input-wrapper),
.knowledge-base-modal :deep(.n-base-selection) {
  background-color: var(--guaxi-content-bg) !important;
}

.knowledge-base-modal :deep(.n-input__border),
.knowledge-base-modal :deep(.n-input__state-border),
.knowledge-base-modal :deep(.n-base-selection__border),
.knowledge-base-modal :deep(.n-base-selection__state-border) {
  border-color: var(--kb-create-hairline) !important;
}

.knowledge-base-modal :deep(.n-dynamic-tags) {
  min-height: 32px;
}

.knowledge-base-modal :deep(.n-dynamic-tags .n-tag) {
  border-radius: 999px;
  background: rgba(33, 189, 236, 0.12);
  color: var(--n-color-primary, #1890ff);
}

.knowledge-base-modal :deep(.n-input__input-el),
.knowledge-base-modal :deep(.n-input__textarea-el) {
  color: var(--guaxi-text-primary);
}

.knowledge-base-modal :deep(.n-input__placeholder),
.knowledge-base-modal :deep(.n-input__textarea-el::placeholder) {
  color: var(--guaxi-text-secondary);
  opacity: 0.75;
}

/* 表单底部操作按钮区域 */
.form-footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--kb-create-hairline);
}

.form-footer :deep(.n-button) {
  min-width: 104px;
  border-radius: 7px;
}

@media (max-width: 560px) {
  .kb-create-header {
    padding: 20px;
  }

  .kb-create-form {
    padding: 16px 18px 18px;
  }

  .form-footer {
    flex-direction: column-reverse;
  }

  .form-footer :deep(.n-button) {
    width: 100%;
  }
}
</style>
