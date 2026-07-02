// AddFileModal.vue
<template>
  <n-modal :show="show" @update:show="$emit('update:show', $event)" title="添加文件" preset="dialog" :mask-closable="false"
    :closable="false" class="add-file-modal" style="width: 90%; max-width: 800px;">
    <n-form :model="form" :rules="rules" label-placement="left" label-width="auto" label-align="right" size="medium"
      ref="formRef" class="custom-form">
      <n-form-item v-for="column in formColumns" :key="column.key" :label="column.title" :path="column.key"
        :label-style="{ fontWeight: 'bold', color: 'var(--guaxi-text-primary)', fontSize: '15px' }" class="form-item">

        <!-- 文件路径 - 带选择文件按钮 -->
        <template v-if="column.key === 'filePath'">
          <n-input-group>
            <n-input v-model:value="form.filePath" placeholder="请输入文件路径" clearable />
            <n-button @click="$emit('open-file-browser')" type="primary">
              <template #icon>
                <n-icon :component="FolderAdd20Regular" />
              </template>
              选择文件
            </n-button>
          </n-input-group>
        </template>

        <!-- 只读字段 - 修复：使用 :value 而不是 v-model，移除 @update:value -->
        <template v-else-if="['importFileName', 'type', 'importFileSize', 'importFileCreateTime'].includes(column.key)">
          <n-input :value="getDisplayValue(form[column.key])" :placeholder="`${column.title}`" readonly clearable />
        </template>

        <!-- 转换引擎选择 -->
        <template v-else-if="column.key === 'convertEngine'">
          <div style="width:100%">
            <n-select v-model:value="form.convertEngine" :options="engineOptions"
              placeholder="选择转换引擎" />
            <div class="engine-hint">{{ engineHint }}</div>
          </div>
        </template>

        <!-- 描述 - 多行文本 -->
        <template v-else-if="column.key === 'description'">
          <n-input v-model:value="form.description" type="textarea" :placeholder="`请输入${column.title}`" :rows="3"
            :maxlength="240" show-count clearable />
        </template>

        <!-- 文件标签 - 带自动补全 -->
        <template v-else-if="column.key === 'importedFileTags'">
          <n-auto-complete v-model:value="form.importedFileTags" :options="importedFileTagOptions"
            :loading="isImportedFileTagLoading" placeholder="请输入文件标签，多个标签用逗号分隔" clearable
            @update:value="handleTagsChange" @search="handleTagsSearch" />
        </template>

        <!-- 文件库 - 带自动补全的选择器 -->
        <template v-else-if="column.key === 'library'">
          <n-auto-complete v-model:value="form.library" :options="libraryOptions" :loading="isLibraryLoading"
            placeholder="请选择或输入文件库" clearable @update:value="handleLibraryChange" @search="handleLibrarySearch" />
        </template>

        <!-- 知识库 -->
        <template v-else-if="column.key === 'knowledgeRepo'">
          <n-auto-complete v-model:value="form.knowledgeRepo" :options="knowledgeRepoOptions"
            :loading="isKnowledgeRepoLoading" placeholder="请选择或输入知识库" clearable
            @update:value="handleKnowledgeRepoChange" @search="handleKnowledgeRepoSearch" />
        </template>

        <!-- 默认输入框 -->
        <template v-else>
          <n-input v-model:value="form[column.key]" :placeholder="`请输入${column.title}`" clearable />
        </template>
      </n-form-item>
    </n-form>

    <template #action>
      <n-space justify="end">
        <n-button type="primary" @click="handleSubmit" class="submit-button" :loading="submitting">
          提交
        </n-button>
        <n-button @click="handleCancel" class="cancel-button" :disabled="submitting">
          取消
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<script setup>
import { createLogger } from '../../../../utils/logger'
import { ref, watch, computed } from 'vue'
import {
  NModal, NForm, NFormItem, NInput, NButton, NIcon, NInputGroup,
  NAutoComplete, NSpace, NSelect
} from 'naive-ui'
import { FolderAdd20Regular } from '@vicons/fluent'

const logger = createLogger('AddFileModal')
const props = defineProps({
  show: {
    type: Boolean,
    default: false
  },
  form: {
    type: Object,
    required: true
  },
  rules: {
    type: Object,
    default: () => ({})
  },
  formColumns: {
    type: Array,
    default: () => []
  },
  libraryOptions: {
    type: Array,
    default: () => []
  },
  importedFileTagOptions: {
    type: Array,
    default: () => []
  },
  knowledgeRepoOptions: {
    type: Array,
    default: () => []
  },
  isLibraryLoading: {
    type: Boolean,
    default: false
  },
  isImportedFileTagLoading: {
    type: Boolean,
    default: false
  },
  isKnowledgeRepoLoading: {
    type: Boolean,
    default: false
  },
  engineStatus: {
    type: Object,
    default: () => ({ fastReady: false, fastProvisioning: false, markerReady: false, mineruReady: false })
  }
})

const emit = defineEmits([
  'update:show',
  'update:form',
  'open-file-browser',
  'submit',
  'cancel',
  'library-search-complete',
  'tag-search-complete',
  'knowledge-repo-search-complete'
])

const formRef = ref(null)
const submitting = ref(false)

// 获取显示值的方法
const getDisplayValue = (value) => {
  if (value === null || value === undefined) return ''
  return String(value)
}

// 处理表单值更新
const handleValueUpdate = (key, value) => {
  const newForm = { ...props.form, [key]: value }
  emit('update:form', newForm)
}

// 处理文件库变化
const handleLibraryChange = (value) => {
  logger.info('文件库变化信号发出:', value)
  handleValueUpdate('library', value)
}

// 处理文件库搜索
const handleLibrarySearch = (value) => {
  logger.info('文件库搜索信号发出:', value)
  emit('library-search-complete', value)
}

// 处理标签变化
const handleTagsChange = (value) => {
  handleValueUpdate('importedFileTags', value)
}

// 处理标签搜索
const handleTagsSearch = (value) => {
  emit('tag-search-complete', value)
}

// 处理知识库变化
const handleKnowledgeRepoChange = (value) => {
  handleValueUpdate('knowledgeRepo', value)
}

// 处理知识库搜索
const handleKnowledgeRepoSearch = (value) => {
  logger.info('知识库搜索信号发出:', value)
  emit('knowledge-repo-search-complete', value)
}

// 处理提交
const handleSubmit = async () => {
  try {
    submitting.value = true

    emit('submit')
  } catch (errors) {
    logger.error('表单验证失败:', errors)
  } finally {
    submitting.value = false
  }
}

// 处理取消
const handleCancel = () => {
  logger.info('取消按钮被点击')
  // 发出 cancel 事件
  emit('cancel')
  // 同时关闭模态框
  emit('update:show', false)
}

// 监听外部form变化，同步到本地
watch(() => props.form, (newForm) => {
  logger.info('表单数据更新:', newForm)
}, { deep: true, immediate: true })

// 转换引擎选项（label 后缀反映装配状态）
const engineOptions = computed(() => {
  const s = props.engineStatus || {}
  const tag = (ready, provisioning) =>
    ready ? '（就绪）' : provisioning ? '（装配中…）' : '（首次使用将自动装配）'
  const mineruTag = s.mineruReady
    ? (s.mineruCuda ? '（就绪·GPU 加速）' : '（就绪·CPU）')
    : '（首次使用将自动装配）'
  return [
    { label: '自动检测（推荐）', value: 'auto' },
    { label: `快速引擎（纯文本/简单排版）${tag(s.fastReady, s.fastProvisioning)}`, value: 'fast' },
    { label: `Marker（复杂版面/图片）${tag(s.markerReady, s.markerProvisioning)}`, value: 'marker' },
    { label: `高质量强引擎（理工科/公式/表格）${mineruTag}`, value: 'mineru' },
  ]
})

// 引擎状态提示
const engineHint = computed(() => {
  const s = props.engineStatus || {}
  if (s.fastProvisioning) return '正在后台自动装配快速引擎（下载运行时/依赖），稍后即可使用…'
  if (s.fastReady) return '快速引擎已就绪，简单文档开箱即用；复杂文档首次会自动装配强引擎。'
  return '首次使用会自动检查并安装所需依赖，无需手动配置。'
})

// 计算表单列配置
const formColumns = computed(() => {
  return [
    { key: 'filePath', title: '文件路径' },
    { key: 'importFileName', title: '文件名' },
    { key: 'description', title: '文件描述' },
    { key: 'knowledgeRepo', title: '知识库' },
    { key: 'library', title: '文件库' },
    { key: 'importedFileTags', title: '文件标签' },
    { key: 'convertEngine', title: '转换引擎' },
    { key: 'type', title: '文件类型' },
    { key: 'importFileCreateTime', title: '入库时间' },
    { key: 'importFileSize', title: '文件大小' },
  ]
})
</script>

<style scoped>
/* 样式保持不变 */
.add-file-modal {
  width: 90%;
  max-width: 800px;
}

.custom-form {
  padding: 20px 0;
  max-height: 70vh;
  overflow-y: auto;
}

.form-item {
  margin-bottom: 24px;
}

.form-item:last-child {
  margin-bottom: 0;
}

.input-tips {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-top: 4px;
}

.engine-hint {
  font-size: 12px;
  color: var(--guaxi-text-secondary);
  margin-top: 6px;
  line-height: 1.4;
}

.submit-button {
  background-color: #409EFF;
  border-color: #409EFF;
  color: white;
  border-radius: 6px;
  transition: all 0.2s ease;
  min-width: 80px;
}

.submit-button:hover {
  background-color: #3385d9;
  border-color: #3385d9;
}

.cancel-button {
  background-color: var(--guaxi-header-bg);
  border-color: var(--guaxi-border);
  color: var(--guaxi-text-primary);
  border-radius: 6px;
  transition: all 0.2s ease;
  min-width: 80px;
}

.cancel-button:hover {
  background-color: var(--guaxi-panel-bg);
  border-color: var(--guaxi-border);
}
</style>

<style scoped>
/* 修复样式冲突 - 使用更具体的选择器 */
.add-file-modal :deep(.n-form-item-label) {
  font-weight: 600;
  color: var(--guaxi-text-primary);
  min-width: 80px;
}

.add-file-modal :deep(.n-input),
.add-file-modal :deep(.n-auto-complete) {
  width: 100%;
}

/* 修复输入框样式 */
.add-file-modal :deep(.n-input .n-input__input-el) {
  border-radius: 8px;
  border: 1px solid #e0e0e0;
  transition: border-color 0.2s ease;
  padding: 0 12px;
  height: 34px;
  line-height: 34px;
}

.add-file-modal :deep(.n-input .n-input__input-el:focus) {
  border-color: #409EFF;
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.2);
}

.add-file-modal :deep(.n-input .n-input__input-el:hover) {
  border-color: #c0c4cc;
}

/* 修复文本域样式 */
.add-file-modal :deep(.n-input.n-textarea .n-input__input-el) {
  border-radius: 8px;
  min-height: 80px;
  resize: vertical;
  padding: 8px 12px;
  line-height: 1.5;
}

/* 修复自动补全样式 */
.add-file-modal :deep(.n-auto-complete .n-input__input-el) {
  border-radius: 8px;
  border: 1px solid #e0e0e0;
  transition: border-color 0.2s ease;
  padding: 0 12px;
  height: 34px;
  line-height: 34px;
}

.add-file-modal :deep(.n-auto-complete .n-input__input-el:focus) {
  border-color: #409EFF;
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.2);
}

.add-file-modal :deep(.n-auto-complete .n-input__input-el:hover) {
  border-color: #c0c4cc;
}

/* 修复输入框组样式 */
.add-file-modal :deep(.n-input-group) {
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--guaxi-border);
  transition: border-color 0.2s ease;
}

.add-file-modal :deep(.n-input-group:focus-within) {
  border-color: var(--n-color-primary, #409EFF);
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.2);
}

.add-file-modal :deep(.n-input-group .n-input) {
  border-radius: 0;
  border: none;
}

.add-file-modal :deep(.n-input-group .n-input .n-input__input-el) {
  border-radius: 0;
  border: none;
  box-shadow: none;
}

.add-file-modal :deep(.n-input-group .n-button) {
  border-radius: 0;
  border-left: 1px solid var(--guaxi-border);
  height: 34px;
}

.add-file-modal :deep(.n-input-group .n-button:hover) {
  border-left-color: var(--n-color-primary, #409EFF);
}

.add-file-modal :deep(.n-input-group:focus-within .n-button) {
  border-left-color: var(--n-color-primary, #409EFF);
}

/* 修复只读输入框样式 */
.add-file-modal :deep(.n-input[readonly] .n-input__input-el) {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  cursor: not-allowed;
}

/* 确保下拉菜单样式正确 */
.add-file-modal :deep(.n-base-select-menu) {
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.add-file-modal :deep(.n-base-select-option) {
  padding: 8px 12px;
  border-radius: 4px;
}

.add-file-modal :deep(.n-base-select-option:hover) {
  background-color: var(--guaxi-header-bg);
}
</style>