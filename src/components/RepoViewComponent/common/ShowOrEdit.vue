<template>
  <div :style="style" @dblclick="handleOnDoubleClick" class="editable-cell" :class="{ 'is-edit': isEdit }">
    <!-- 编辑状态：自动补全 -->
    <n-auto-complete v-if="isEdit && enableAutoComplete" ref="inputRef" v-model:value="inputValue" :options="options"
      :loading="isLoading" :style="inputStyle" @update:value="handleUpdateValue" @blur="handleChange"
      @search="handleSearch" @keyup.enter="handleChange" clearable />
    <!-- 编辑状态：普通输入 -->
    <n-input v-else-if="isEdit" ref="inputRef" v-model:value="inputValue" :style="inputStyle" type="textarea"
      :autosize="{ minRows: 3, maxRows: 10 }" @update:value="handleUpdateValue" @change="handleChange"
      @blur="handleChange" @keydown.ctrl.enter="handleChange" />
    <!-- 标签显示模式 - 只读显示 -->
    <div v-else-if="displayMode === 'tags' && !isEdit" class="tags-display">
      <n-tag v-for="tag in tagList" :key="tag" :type="getTagColorFn(tag)" bordered-false
        :style="{ marginRight: '6px', marginBottom: '4px' }">
        {{ tag }}
      </n-tag>
    </div>
    <!-- 展示状态：带 Tooltip 的截断文本 -->
    <n-tooltip v-else placement="top" trigger="hover" :show-arrow="true">
      <template #trigger>
        <span class="cell-content">{{ truncatedValue }}</span>
      </template>
      {{ value }}
    </n-tooltip>
  </div>
</template>

<script setup>
import { ref, nextTick, watch, onMounted, onUnmounted, computed, h } from 'vue';
import { NInput, NAutoComplete, NTooltip } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps({
  value: [String, Number], // 单元格的值
  onUpdateValue: Function, // 更新值的回调函数
  enableAutoComplete: { // 是否启用自动补全
    type: Boolean,
    default: false,
  },
  searchType: String, // 搜索类型，用于区分不同的后端接口
  readonly: { // 是否只读
    type: Boolean,
    default: false
  },
  placeholder: {
    type: String,
    default: '请输入内容'
  },
  displayMode: { //显示模式 'text' 或 'tags'
    type: String,
    default: 'text'
  },
  getTagColor: Function // 获取标签颜色的函数
});

const emit = defineEmits(['update:value']);

const isEdit = ref(false);
const inputRef = ref(null);
const inputValue = ref(props.value);
const options = ref([]); // 自动补全的选项
const isLoading = ref(false); // 是否正在加载

const tagList = computed(() => {
  if (!props.value) return [];
  return (typeof props.value === "string"
    ? props.value.split(",").map(tag => tag.trim())
    : props.value);
});

const getTagColorFn = (tag) => {
  return props.getTagColor ? props.getTagColor(tag) : 'default';
};

// 计算截断的文本（最多显示 120 个字符）
const truncatedValue = computed(() => {
  const val = String(props.value || '');
  return val.length > 120 ? val.substring(0, 120) + '...' : val;
});

// 样式
const style = {
  minHeight: '60px',
  cursor: props.readonly ? 'default' : 'pointer',
};

// 限制 input 宽度
const inputStyle = {
  width: '100%',
  height: '100%',
  maxWidth: '100%',
};

const handleOnDoubleClick = () => {
  if (props.readonly) return;
  isEdit.value = true;
  inputValue.value = props.value;
  nextTick(() => {
    inputRef.value?.focus();
  });
};

// 处理值更新
const handleUpdateValue = (v) => {
  inputValue.value = v;
};

const handleChange = () => {
  if (props.readonly) return;

  console.log(`handleChange 被调用，输入值: ${inputValue.value}`);
  if (props.onUpdateValue) {
    props.onUpdateValue(inputValue.value);
  }
  emit('update:value', inputValue.value);
  isEdit.value = false;
};

// 处理搜索
const handleSearch = async (query) => {
  if (!props.enableAutoComplete || !props.searchType) return;

  console.log(`handleSearch 被调用，搜索关键字: ${query}, 搜索类型: ${props.searchType}`);
  isLoading.value = true;
  try {
    let result;
    switch (props.searchType) {
      case 'noteTags':
        result = await invoke('search_notes_tags', { query });
        break;
      case 'importedFileTags':
        result = await invoke('search_imported_files_tags', { query });
        break;
      case 'importedFiles':
        result = await invoke('search_imported_files', { query });
        break;
      case 'libraries':
        result = await invoke('search_libraries', { query });
        break;
      case 'importedFileLibraries':
        result = await invoke('search_imported_file_libraries', { query });
        break;
      default:
        return;
    }

    if (!result) {
      console.error('搜索结果为空');
      return;
    } else if (result.length === 0) {
      console.log('搜索结果为空');
    } else {
      console.log(`handleSearch 返回结果:`, result);
      options.value = result;
    }

  } catch (error) {
    console.error('搜索失败:', error);
  } finally {
    isLoading.value = false;
  }
};

// 监听输入值变化，触发搜索
watch(inputValue, (newValue) => {
  if (isEdit.value && props.enableAutoComplete && newValue) {
    handleSearch(newValue);
  }
});

// 点击其他地方关闭编辑模式
const handleClickOutside = (event) => {
  if (isEdit.value && !event.target.closest('.editable-cell')) {
    handleChange();
  }
};

onMounted(() => {
  if (!props.readonly) {
    document.addEventListener('click', handleClickOutside);
  }
});

onUnmounted(() => {
  if (!props.readonly) {
    document.removeEventListener('click', handleClickOutside);
  }
});

// 监听外部value变化
watch(() => props.value, (newValue) => {
  inputValue.value = newValue;
});
</script>

<style scoped>
.editable-cell {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 2px, 20px;
  margin: 0px;
  min-height: 90px;
  width: 100%;
  height: 100%;
}

.editable-cell.is-edit {
  align-self: stretch;
}

.tags-display {
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
  cursor: pointer;
  min-height: 30px;
  align-items: center;
  padding: 0;
  width: 100%;
  height: 50%;
}

.cell-content {
  width: 100%;
  word-break: break-word;
  white-space: normal;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  line-height: 1.5;
  cursor: pointer;
}

:deep(.n-input) {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
}

:deep(.n-auto-complete) {
  width: 100%;
  height: 100%;
  box-sizing: border-box;
}

:deep(.n-input .n-input__input) {
  height: 100%;
  border-radius: 8px;
}

:deep(.n-input .n-input__textarea-el) {
  height: 100%;
}

:deep(.n-auto-complete .n-input__input) {
  height: 100%;
  border-radius: 8px;
}

:deep(.n-tooltip) {
  max-width: 100%;
}
</style>