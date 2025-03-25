<template>
  <div :style="style" @click="handleOnClick">
    <n-input
      v-if="isEdit"
      ref="inputRef"
      v-model:value="inputValue"
      :style="inputStyle"
      @update:value="handleUpdateValue"
      @change="handleChange"
      @blur="handleChange"
    />
    <span v-else>{{ value }}</span>
  </div>
</template>

<script setup>
import { ref, nextTick } from 'vue';
import { NInput } from 'naive-ui';

const props = defineProps({
  value: [String, Number], // 单元格的值
  onUpdateValue: Function, // 更新值的回调函数
});

const isEdit = ref(false);
const inputRef = ref(null);
const inputValue = ref(props.value);

// 样式
const style = {
  minHeight: '22px',
  cursor: 'pointer',
};

// 限制 input 宽度
const inputStyle = {
  width: '100%', // 宽度与单元格一致
  maxWidth: '200px', // 最大宽度，避免过大
};

const handleOnClick = () => {
  isEdit.value = true;
  nextTick(() => {
    inputRef.value?.focus();
  });
};

const handleChange = () => {
  props.onUpdateValue(inputValue.value); // 调用父组件的回调函数
  isEdit.value = false;
};

const handleUpdateValue = (v) => {
  inputValue.value = v;
};
</script>