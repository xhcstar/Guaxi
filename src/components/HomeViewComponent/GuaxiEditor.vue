<template>
    <div class="editor-container">
      <n-input
        v-if="selectedFile"
        v-model:value="content"
        type="textarea"
        :autosize="{ minRows: 10 }"
        placeholder="编辑文件内容"
        class="editor"
      />
      <n-alert v-else type="info" class="info-message">请从左侧选择一个文件进行编辑。</n-alert>
    </div>
  </template>
  
  <script setup>
  import { NInput, NAlert } from 'naive-ui'
  import { computed, ref, watch, onMounted, onUnmounted } from 'vue'
  import { useFileTreeStore } from '../../stores/fileTreeStore'
  
  // 引入 Pinia store
  const fileTreeStore = useFileTreeStore()
  console.log('GuaxiEditor 组件初始化完成')
  
  // 当前选中的文件
  const selectedFile = computed(() => {
    console.log('获取当前选中的文件:', fileTreeStore.selectedFile)
    return fileTreeStore.selectedFile
  })
  
  // 文件内容
  const content = ref(selectedFile.value?.content || '')
  console.log('初始化文件内容:', content.value)
  
  // 监听选中的文件变化
  watch(selectedFile, (newFile) => {
    console.log('选中的文件发生变化，新文件:', newFile)
    content.value = newFile?.content || ''
    console.log('更新文件内容:', content.value)
  })
  
  // 保存文件内容
  const saveContent = () => {
    console.log('开始保存文件内容')
    if (selectedFile.value) {
      console.log('保存文件路径:', selectedFile.value.path)
      console.log('保存文件内容:', content.value)
      fileTreeStore.updateFileContent(selectedFile.value.path, content.value)
      console.log('保存文件内容成功')
    } else {
      console.warn('未选中任何文件，无法保存')
    }
  }
  
  // 监听快捷键 Ctrl + S
  const handleKeyDown = (event) => {
    if (event.ctrlKey && event.key === 's') {
      event.preventDefault() // 阻止默认保存行为
      saveContent()
    }
  }
  
  // 组件挂载时添加事件监听
  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown)
  })
  
  // 组件卸载时移除事件监听
  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown)
  })
  </script>
  
  <style scoped>
  .editor-container {
    display: flex;
    flex-direction: column;
    height: 98%; /* 占满父组件高度 */
    padding: 5px; /* 添加内边距 */
  }
  
  .editor {
    flex: 1; /* 占满剩余空间 */
    width: 100%; /* 占满父组件宽度 */
    font-family: 'Courier New', Courier, monospace; /* 使用等宽字体 */
    font-size: 18px; /* 调大默认字体大小 */
    line-height: 1.6; /* 设置行高 */
    border: 1px solid #ddd; /* 添加边框 */
    border-radius: 4px; /* 添加圆角 */
    padding: 5px; /* 添加内边距 */
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1); /* 添加阴影 */
    overflow-y: auto; /* 超过高度时触发滚动 */
  }
  
  .info-message {
    margin-top: 16px; /* 添加外边距 */
  }
  </style>