<template>
  <div :style="style" @click="handleOnClick" class="editable-cell">
    <!-- 使用 n-auto-complete 替换 n-input，当需要自动补全时 -->
    <n-auto-complete
      v-if="isEdit && enableAutoComplete"
      ref="inputRef"
      v-model:value="inputValue"
      :options="options"
      :loading="isLoading"
      :style="inputStyle"
      @update:value="handleUpdateValue"
      @blur="handleChange"
      @search="handleSearch"
      @keyup.enter="handleChange"
      clearable
    />
    <!-- 否则使用 n-input -->
    <!-- <n-input
      v-else-if="isEdit"
      ref="inputRef"
      v-model:value="inputValue"
      :style="inputStyle"
      @update:value="handleUpdateValue"
      @change="handleChange"
      @blur="handleChange"
      @keyup.enter="handleChange"
    /> -->
    <span v-else>{{ value }}</span>
  </div>
</template>

<script setup>
import { ref, nextTick, watch, onMounted, onUnmounted, h } from 'vue';
import { NInput, NAutoComplete } from 'naive-ui';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps({
  value: [String, Number], // 单元格的值
  onUpdateValue: Function, // 更新值的回调函数
  enableAutoComplete: { // 是否启用自动补全
    type: Boolean,
    default: false,
  },
  searchType: String, // 搜索类型，用于区分不同的后端接口
});

const isEdit = ref(false);
const inputRef = ref(null);
const inputValue = ref(props.value);
const options = ref([]); // 自动补全的选项
const isLoading = ref(false); // 是否正在加载

// 样式
const style = {
  minHeight: '50px',
  cursor: 'pointer',
  margin: '0 0px',
  padding: '0 0px',
};

// 限制 input 宽度
const inputStyle = {
  width: '100%', // 宽度与单元格一致
  height: '100%',
  maxWidth: '20px', // 最大宽度，避免过大
  margin: '0 0px',
  padding: '0 0px',
};

// 处理点击事件
const handleOnClick = () => {
  isEdit.value = true;
  nextTick(() => {
    inputRef.value?.focus(); // 确保焦点设置到输入框
  });
};

// 处理值更新
const handleUpdateValue = (v) => {
  inputValue.value = v;
};

const handleChange = () => {
  console.log(`handleChange 被调用，输入值: ${inputValue.value}`);
  props.onUpdateValue(inputValue.value); // 调用父组件的回调函数
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
        // console.error('未知的搜索类型:', props.searchType);
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
  document.addEventListener('click', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});
</script>