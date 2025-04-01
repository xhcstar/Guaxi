<template>
  <div class="pdf-viewer-container">
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

      <div class="right-controls">
        <button class="toggle-notes" @click="toggleNotesPanel">
          <i :class="showNotes ? 'icon-notes-active' : 'icon-notes'"></i>
          {{ showNotes ? '隐藏笔记' : '显示笔记' }}
        </button>
        <button class="export-notes" @click="exportNotes">
          <i class="icon-download"></i> 导出笔记
        </button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="pdf-main-content">
      <!-- 左侧PDF内容 -->
      <div class="pdf-panel">
        <div class="pdf-content" ref="pdfContent">
          <template v-for="(element, index) in elements" :key="index">
            <!-- 文本元素 -->
            <div v-if="element.type === 'text'" class="text-element content-element" @mouseenter="hoverElement(index)"
              @mouseleave="delayUnhover(index)" @click="addToNotes(element)"
              :class="{ 'hovered': hoveredIndex === index }">
              <div class="element-content">{{ element.content }}</div>

              <div class="hover-area" @mouseenter="keepHovered(index)" @mouseleave="delayUnhover(index)">
                <transition name="bubble">
                  <div v-if="hoveredIndex === index" class="hover-bubble" @click.stop="addToNotes(element)">
                    点击添加到笔记
                  </div>
                </transition>
              </div>
            </div>

            <!-- 图像元素 -->
            <div v-else-if="element.type === 'image'" class="image-element content-element"
              @mouseenter="hoverElement(index)" @mouseleave="delayUnhover(index)" @click="addToNotes(element)"
              :class="{ 'hovered': hoveredIndex === index }">
              <img :src="'data:image/png;base64,' + element.content">

              <div class="hover-area" @mouseenter="keepHovered(index)" @mouseleave="delayUnhover(index)">
                <transition name="bubble">
                  <div v-if="hoveredIndex === index" class="hover-bubble" @click.stop="addToNotes(element)">
                    点击添加到笔记
                  </div>
                </transition>
              </div>
            </div>

            <!-- 表格元素 -->
            <div v-else-if="element.type === 'table'" class="table-element content-element"
              @mouseenter="hoverElement(index)" @mouseleave="delayUnhover(index)" @click="addToNotes(element)"
              :class="{ 'hovered': hoveredIndex === index }">
              <img :src="'data:image/png;base64,' + element.content">

              <div class="hover-area" @mouseenter="keepHovered(index)" @mouseleave="delayUnhover(index)">
                <transition name="bubble">
                  <div v-if="hoveredIndex === index" class="hover-bubble" @click.stop="addToNotes(element)">
                    点击添加到笔记
                  </div>
                </transition>
              </div>
            </div>
          </template>
        </div>
      </div>

      <!-- 右侧Markdown内容 -->
      <div class="markdown-panel">
        <div class="markdown-content">
          <div v-for="(line, index) in markdownLines" :key="index" 
               class="markdown-line" 
               @click="addMarkdownToNotes(line)">
            {{ line }}
          </div>
        </div>
      </div>
    </div>

    <!-- 笔记面板 -->
    <div class="notes-panel" :class="{ 'notes-panel-visible': showNotes }">
      <div class="notes-header">
        <h3>我的笔记</h3>
        <button @click="toggleNotesPanel" class="close-btn">
          <i class="icon-close">×</i>
        </button>
      </div>
      <div class="notes-content">
        <textarea v-model="notesContent" placeholder="点击PDF或Markdown内容添加到笔记..." class="notes-textarea"></textarea>
      </div>
    </div>

    <!-- 添加笔记的提示气泡 -->
    <transition name="fade">
      <div v-if="showNoteBubble" class="note-added-bubble">
        <div class="bubble-content">
          <span>已添加到笔记</span>
          <i class="icon-check"></i>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { repoStore } from '../../stores/repoStore'

const store = repoStore()
const librariesRoot = store.librariesRoot

const props = defineProps({
  filePath: {
    type: String,
    required: true,
  },
  fileType: {
    type: String,
    required: true,
    validator: (value) => ['pdf'].includes(value)
  },
  fileName: {
    type: String,
    required: true,
  },
  library: {
    type: String,
    required: true
  }
})

// PDF数据状态
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const elements = ref([])
// Markdown数据状态
const markdownContent = ref('')
const markdownLines = ref([])
// 笔记状态
const showNotes = ref(false)
const notesContent = ref('')
// UI状态
const hoveredIndex = ref(-1)
const showNoteBubble = ref(false)
const hoverTimeout = ref(null)

// 计算Markdown文件路径
const markdownFilePath = computed(() => {
  store.get_librariesRoot()
  const baseFileName = props.fileName.replace('.pdf', '')
  return `${librariesRoot}\\Libraries\\${props.library}\\${baseFileName}\\markdown\\${baseFileName}\\${baseFileName}.md`
})

// 方法定义
const hoverElement = (index) => {
  clearTimeout(hoverTimeout.value)
  hoveredIndex.value = index
}

const keepHovered = (index) => {
  clearTimeout(hoverTimeout.value)
  hoveredIndex.value = index
}

const delayUnhover = (index) => {
  hoverTimeout.value = setTimeout(() => {
    if (hoveredIndex.value === index) {
      hoveredIndex.value = -1
    }
  }, 300)
}

// 显示添加笔记的提示气泡
const showNoteBubbleMessage = () => {
  showNoteBubble.value = true
  setTimeout(() => {
    showNoteBubble.value = false
  }, 2000)
}

// 加载PDF页面
// 在setup()中或<script setup>顶部添加
const loadPdfPage = async (pageNum: number) => {
  try {
    const response = await invoke<string>('get_converted_pdf_page', {
      request: {
        filetype: 'pdf',
        filepath: props.filePath,
        curpage: pageNum,
        lastpage: 0
      }
    });

    console.log('Loading PDF', request)

    const data = JSON.parse(response) as {
      page_number: number;
      total_pages: number;
      elements: Array<{
        type: string;
        content: string;
      }>;
    };

    // 更新状态
    currentPage.value = data.page_number;
    totalPages.value = data.total_pages;
    
    // 处理元素内容
    elements.value = data.elements.map(el => ({
      ...el,
      // 为图片类元素添加Data URL前缀
      content: ['image', 'table'].includes(el.type) 
        ? `data:image/png;base64,${el.content}`
        : el.content,
      // 添加唯一key
      key: `${el.type}_${Date.now()}_${Math.random().toString(36).slice(2)}`
    }));

  } catch (error) {
    console.error('PDF加载失败:', error);
    // 错误处理增强
    elements.value = [];
    if (error instanceof Error) {
      alert(`PDF加载错误: ${error.message}`);
    }
  }
};

// 加载Markdown文件
const loadMarkdownFile = async () => {
  try {
    console.log('加载Markdown文件...', markdownFilePath.value);
    const response = await invoke('load_markdown_content', {
      filepath: markdownFilePath.value
    });

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    markdownContent.value = response.content;
    markdownLines.value = response.content.split('\n').filter(line => line.trim() !== '');
    return response.content;
  } catch (error) {
    console.error('加载Markdown失败:', error);
    markdownContent.value = '';
    markdownLines.value = [];
    throw error;
  }
}

// 添加内容到笔记
const addToNotes = (element) => {
  const timestamp = new Date().toLocaleString()
  let note = ''

  switch (element.type) {
    case 'text':
      note = `## 文本摘录 (${timestamp})\n${element.content}\n\n`
      break
    case 'image':
      note = `## 图片摘录 (${timestamp})\n![图片](${element.content})\n\n`
      break
    case 'table':
      note = `## 表格摘录 (${timestamp})\n![表格](${element.content})\n\n`
      break
  }

  notesContent.value += note
  showNotes.value = true
  showNoteBubbleMessage()
}

// 添加Markdown内容到笔记
const addMarkdownToNotes = (line) => {
  const timestamp = new Date().toLocaleString()
  const note = `## Markdown摘录 (${timestamp})\n${line}\n\n`
  notesContent.value += note
  showNotes.value = true
  showNoteBubbleMessage()
}

// 导出笔记
const exportNotes = () => {
  if (!notesContent.value) {
    alert('笔记内容为空')
    return
  }

  const blob = new Blob([notesContent.value], { type: 'text/markdown' })
  const url = URL.createObjectURL(blob)

  const a = document.createElement('a')
  a.href = url
  a.download = `PDF笔记_${props.filePath.split('/').pop().split('.')[0]}.md`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

// 分页控制
const prevPage = () => {
  if (currentPage.value > 1) {
    loadPdfPage(currentPage.value - 1)
  }
}

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    loadPdfPage(currentPage.value + 1)
  }
}

const goToPage = (page) => {
  const pageNum = typeof page === 'number' ? page : jumpPage.value
  const targetPage = Math.max(1, Math.min(pageNum, totalPages.value))
  loadPdfPage(targetPage)
}

// UI控制
const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value
}

// 初始化加载第一页和Markdown文件
onMounted(() => {
  store.get_librariesRoot()
  loadPdfPage(1)
  loadMarkdownFile()
})

// 监听PDF路径变化
watch(() => props.filePath, (newPath) => {
  if (newPath) {
    currentPage.value = 1
    loadPdfPage(1)
    loadMarkdownFile()
    notesContent.value = '' // 清空笔记
  }
})
</script>

<style scoped>
.pdf-viewer-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: #f5f5f5;
  overflow: hidden;
}

.right-controls {
  display: flex;
  gap: 10px;
}

/* 笔记开关按钮样式 */
.toggle-notes {
  padding: 6px 12px;
  background-color: #3498db;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
}

.toggle-notes:hover {
  background-color: #2980b9;
}

.icon-notes::before {
  content: "📝";
  margin-right: 5px;
}

.icon-notes-active::before {
  content: "📖";
  margin-right: 5px;
}

.pdf-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 20px;
  background-color: #2c3e50;
  color: white;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
  z-index: 10;
}

.page-info {
  font-size: 14px;
  opacity: 0.8;
}

.page-navigation {
  display: flex;
  align-items: center;
  gap: 10px;
}

.page-navigation input {
  width: 60px;
  padding: 6px;
  text-align: center;
  border: 1px solid #ddd;
  border-radius: 4px;
}

button {
  padding: 6px 12px;
  background-color: #3498db;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
}

button:hover {
  background-color: #2980b9;
}

button:disabled {
  background-color: #95a5a6;
  cursor: not-allowed;
}

.export-notes {
  background-color: #27ae60;
}

.export-notes:hover {
  background-color: #219653;
}

.pdf-main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.pdf-panel {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  border-right: 1px solid #ddd;
}

.markdown-panel {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  background-color: #f9f9f9;
}

.markdown-content {
  line-height: 1.6;
}

.markdown-line {
  margin-bottom: 10px;
  padding: 8px;
  background-color: white;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.2s;
}

.markdown-line:hover {
  background-color: #e6f7ff;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.text-element {
  margin-bottom: 15px;
  padding: 10px;
  background-color: #f9f9f9;
  border-radius: 4px;
  cursor: pointer;
  position: relative;
}

.image-element,
.table-element {
  margin-bottom: 20px;
  cursor: pointer;
  position: relative;
}

.image-element img,
.table-element img {
  max-width: 100%;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
}

.hover-area {
  position: absolute;
  bottom: -40px;
  left: 0;
  width: 100%;
  height: 50px;
  pointer-events: auto;
}

.hover-bubble {
  position: absolute;
  bottom: 10px;
  left: 50%;
  transform: translateX(-50%);
  background-color: #2c3e50;
  color: white;
  padding: 5px 10px;
  border-radius: 15px;
  font-size: 12px;
  white-space: nowrap;
  z-index: 20;
  cursor: pointer;
}

.text-element.hovered {
  box-shadow: 0 0 15px rgba(100, 149, 237, 0.6);
  background-color: rgba(100, 149, 237, 0.1);
  border-radius: 4px;
}

.image-element.hovered,
.table-element.hovered {
  box-shadow: 0 0 20px rgba(100, 149, 237, 0.8);
}

/* 气泡动画 */
.bubble-enter-active,
.bubble-leave-active {
  transition: all 0.3s ease;
}

.bubble-enter-from,
.bubble-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(10px);
}

/* 添加笔记的提示气泡 */
.note-added-bubble {
  position: fixed;
  top: 70px;
  right: 20px;
  background-color: #27ae60;
  color: white;
  padding: 10px 15px;
  border-radius: 20px;
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.2);
  z-index: 1000;
}

.bubble-content {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bubble-content .icon-check {
  font-size: 16px;
}

/* 淡入淡出动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.notes-panel {
  position: fixed;
  right: -400px;
  top: 0;
  bottom: 0;
  width: 400px;
  background-color: white;
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 100;
  display: flex;
  flex-direction: column;
}

.notes-panel-visible {
  right: 0;
}


.notes-header {
	padding: 15px 20px;
	height: 22px;
	background-color: #2c3e50;
	color: white;
	display: flex;
	justify-content: space-between;
	align-items: center;
	flex-shrink: 0;
	/* 防止header被压缩 */
}

.notes-header h3 {
	margin: 0;
	font-size: 18px;
	font-weight: 500;
}

.close-btn {
	background: none;
	border: none;
	color: white;
	font-size: 20px;
	cursor: pointer;
	padding: 5px;
	transition: transform 0.2s;
}

.close-btn:hover {
	transform: scale(1.1);
}

.notes-content {
	flex: 1;
	/* 占据剩余空间 */
	padding: 0;
	/* 移除内边距 */
	display: flex;
	overflow: hidden;
	/* 防止内容溢出 */
}

.notes-textarea {
	width: 100%;
	height: 100%;
	border: none;
	resize: none;
	padding: 20px;
	font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
	font-size: 14px;
	line-height: 1.6;
	color: #333;
	background-color: #f9f9f9;
}

.notes-textarea:focus {
	outline: none;
	background-color: #fff;
}

/* 滚动条样式 */
.notes-textarea::-webkit-scrollbar {
	width: 8px;
}

.notes-textarea::-webkit-scrollbar-track {
	background: #f1f1f1;
}

.notes-textarea::-webkit-scrollbar-thumb {
	background: #c1c1c1;
	border-radius: 4px;
}

.notes-textarea::-webkit-scrollbar-thumb:hover {
	background: #a8a8a8;
}

</style>