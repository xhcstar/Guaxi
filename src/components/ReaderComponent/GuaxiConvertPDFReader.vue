<template>
  <div class="convert-pdf-container">
    <!-- 顶部控制栏 -->
    <div class="pdf-controls">
      <div class="page-info">
        <span>第 {{ currentPage }} 页 / 共 {{ totalPages }} 页</span>
      </div>

      <div class="page-navigation">
        <button @click="prevPage" :disabled="currentPage <= 1">
          <i class="icon-arrow-left"></i> 上一页
        </button>

        <input type="number" v-model.number="jumpPage" min="1" :max="totalPages" @keyup.enter="goToPage">

        <button @click="nextPage" :disabled="currentPage >= totalPages">
          下一页 <i class="icon-arrow-right"></i>
        </button>
      </div>
    </div>

    <!-- 双屏内容区 -->
    <div class="content-area">
      <!-- 左侧PDF视图 -->
      <div class="pdf-view">
        <iframe 
          :src="`/pdfjs/web/viewer.html?file=${encodeURIComponent(convertedPdfPath)}`"
          frameborder="0"
          style="width: 100%; height: 100%;"
        ></iframe>
      </div>

      <!-- 右侧Markdown视图 -->
      <div class="markdown-view">
        <div 
          v-for="(line, index) in markdownLines" 
          :key="index" 
          class="markdown-line"
          @click="addToNotes(line)"
        >
          {{ line }}
        </div>
      </div>
    </div>

    <!-- 笔记面板 -->
    <div class="notes-panel" :class="{ 'notes-panel-active': showNotes }">
      <div class="notes-header">
        <h3>笔记内容</h3>
        <button @click="toggleNotesPanel" class="close-btn">
          <i class="icon-close">×</i>
        </button>
      </div>
      <div class="notes-content">
        <textarea v-model="notesContent" class="notes-textarea"></textarea>
        <button @click="saveNotes" class="save-btn">保存笔记</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useStore } from '@/stores/store'
import { invoke } from '@tauri-apps/api/tauri'

const props = defineProps({
  convertedPdfPath: String,
  fileName: String,
  library: String
})

const store = useStore()
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const showNotes = ref(false)
const notesContent = ref('')
const markdownContent = ref('')
const markdownLines = ref([])
const isLoading = ref(true)

// 计算Markdown文件路径
const markdownPath = computed(() => {
  const baseFileName = props.fileName.replace(/\.[^/.]+$/, "")
  return `${store.librariesRoot}\\Libraries\\${props.library}\\${baseFileName}\\markdown\\${baseFileName}.md`
})

// 加载PDF信息
const loadPdfInfo = async () => {
  try {
    totalPages.value = await invoke('get_converted_pdf_page_count', {
      pdf_path: props.convertedPdfPath
    })
    isLoading.value = false
  } catch (error) {
    console.error('获取PDF信息失败:', error)
  }
}

// 加载Markdown内容
const loadMarkdown = async () => {
  try {
    markdownContent.value = await invoke('read_text_file', { 
      path: markdownPath.value 
    })
    markdownLines.value = markdownContent.value.split('\n').filter(line => line.trim())
  } catch (error) {
    console.error('加载Markdown失败:', error)
  }
}

// 监听页码变化
watch(currentPage, async (newPage) => {
  try {
    await invoke('get_converted_pdf_page', {
      pdf_path: props.convertedPdfPath,
      page_num: newPage
    })
  } catch (error) {
    console.error('获取页面内容失败:', error)
  }
})

// 添加到笔记
const addToNotes = (line) => {
  const timestamp = new Date().toLocaleString()
  notesContent.value += `## 摘录 (${timestamp})\n${line}\n\n`
}

// 保存笔记
const saveNotes = async () => {
  try {
    const baseFileName = props.fileName.replace(/\.[^/.]+$/, "")
    const notePath = `${store.librariesRoot}\\Libraries\\${props.library}\\${baseFileName}\\notes\\${baseFileName}_notes.md`
    
    await invoke('write_text_file', {
      path: notePath,
      contents: notesContent.value
    })
    alert('笔记保存成功')
  } catch (error) {
    console.error('保存笔记失败:', error)
  }
}

// 页面导航
const prevPage = () => {
  if (currentPage.value > 1) {
    currentPage.value--
    jumpPage.value = currentPage.value
  }
}

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    currentPage.value++
    jumpPage.value = currentPage.value
  }
}

const goToPage = () => {
  const page = Math.max(1, Math.min(jumpPage.value, totalPages.value))
  currentPage.value = page
}

const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value
}

onMounted(async () => {
  await Promise.all([loadPdfInfo(), loadMarkdown()])
})
</script>

<style scoped>
.convert-pdf-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100%;
}

.pdf-controls {
  display: flex;
  justify-content: space-between;
  padding: 10px;
  background: #f5f5f5;
  border-bottom: 1px solid #ddd;
}

.content-area {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.pdf-view, .markdown-view {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
}

.markdown-line {
  padding: 8px;
  margin: 4px 0;
  border: 1px solid #eee;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
}

.markdown-line:hover {
  background-color: #f0f7ff;
  border-color: #cce0ff;
}

.notes-panel {
  position: fixed;
  right: -400px;
  top: 0;
  width: 400px;
  height: 100%;
  background: white;
  box-shadow: -2px 0 5px rgba(0,0,0,0.1);
  transition: right 0.3s;
  z-index: 100;
}

.notes-panel-active {
  right: 0;
}

.notes-textarea {
  width: 100%;
  height: calc(100% - 60px);
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 4px;
  resize: none;
}

.save-btn {
  margin-top: 10px;
  padding: 8px 16px;
  background: #1890ff;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}
</style>