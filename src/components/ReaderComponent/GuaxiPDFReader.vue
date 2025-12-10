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
      <!-- 左侧缩略图导航 -->
      <div class="thumbnail-panel">
        <div class="thumbnail-container">
          <img :src="'data:image/png;base64,' + thumbnail" @click="toggleThumbnailView"
            :class="{ 'thumbnail-active': !showLargeThumbnail }">
          <div v-if="showLargeThumbnail" class="large-thumbnail">
            <img :src="'data:image/png;base64,' + thumbnail">
            <div class="thumbnail-page-number">{{ currentPage }}</div>
          </div>
        </div>

        <div class="thumbnail-scroll" @scroll="handleThumbnailScroll">
          <div v-for="page in totalPages" :key="page" class="thumbnail-item"
            :class="{ 'thumbnail-current': page === currentPage }" @click="goToPage(page)">
            <span class="thumbnail-page-indicator">{{ page }}</span>
          </div>
        </div>
      </div>

      <!-- 右侧内容区 -->
      <div class="content-panel">
        <div class="pdf-content" ref="pdfContent">
          <template v-for="(element, index) in elements" :key="index">
            <!-- 文本元素 -->
            <div v-if="element.type === 'text'" class="text-element content-element" @mouseenter="hoverElement(index)"
              @mouseleave="delayUnhover(index)" @click="addToNotes(element)"
              :class="{ 'hovered': hoveredIndex === index }">
              <div class="element-content">{{ element.content }}</div>

              <!-- 将提示气泡包含在hover区域内 -->
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
        <textarea v-model="notesContent" placeholder="点击PDF内容添加到笔记..." class="notes-textarea"></textarea>
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

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

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
  }
})

// PDF数据状态
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const thumbnail = ref('')
const elements = ref([])
// 笔记状态
const showNotes = ref(false)
const notesContent = ref('')
// UI状态
const showLargeThumbnail = ref(false)
const hoveredIndex = ref(-1)
const showNoteBubble = ref(false)
const hoverTimeout = ref(null)

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
  }, 2000) // 2秒后自动消失
}

// 加载PDF页面
const loadPdfPage = async (pageNum) => {
  try {
    const response = await invoke('reader_open_file', {
      request: {
        filetype: 'pdf',
        filepath: props.filePath,
        curpage: pageNum,
        lastpage: 0
      }
    });

    console.log('Raw response:', response);

    if (!response.success) {
      throw new Error(response.error || 'Unknown error from backend');
    }

    let data;
    try {
      data = JSON.parse(response.content);
    } catch (e) {
      throw new Error('Failed to parse response content');
    }

    if (data.error) {
      throw new Error(data.error);
    }

    currentPage.value = data.page_number;
    totalPages.value = data.total_pages;
    thumbnail.value = data.thumbnail || '';
    elements.value = data.elements || [];

  } catch (error) {
    console.error('加载PDF失败:', error);
    // 显示用户友好的错误信息
    alert(`加载PDF失败: ${error.message}`);
    // 重置状态
    currentPage.value = 1;
    totalPages.value = 0;
    thumbnail.value = '';
    elements.value = [];
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
  showNoteBubble.value = true
  setTimeout(() => {
    showNoteBubble.value = false
  }, 2000)
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
const toggleThumbnailView = () => {
  showLargeThumbnail.value = !showLargeThumbnail.value
}

const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value
}

const handleThumbnailScroll = (e) => {
  // 实现缩略图滚动逻辑
}

// 初始化加载第一页
onMounted(() => {
  store.get_librariesRoot()
  loadPdfPage(1)
})

// 监听PDF路径变化
watch(() => props.filePath, (newPath) => {
  if (newPath) {
    currentPage.value = 1
    loadPdfPage(1)
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
  /* height: 60px; 固定控制栏高度 */
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

.thumbnail-panel {
  width: 200px;
  background-color: #ecf0f1;
  border-right: 1px solid #ddd;
  display: flex;
  flex-direction: column;
}

.thumbnail-container {
  padding: 15px;
  text-align: center;
  border-bottom: 1px solid #ddd;
}

.thumbnail-container img {
  max-width: 180px;
  /* 增大默认缩略图大小 */
  cursor: pointer;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
  transform: scale(1.05);
}

.large-thumbnail {
  position: fixed;
  left: 220px;
  top: 50%;
  transform: translateY(-50%);
  z-index: 100;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  max-width: 60vw;
  /* 增大最大宽度 */
  max-height: 80vh;
}

.large-thumbnail img {
  max-width: 100%;
  max-height: 70vh;
  /* 增大高度 */
}

.thumbnail-page-number {
  position: absolute;
  bottom: -25px;
  left: 0;
  right: 0;
  text-align: center;
  font-weight: bold;
}

.thumbnail-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
}

.thumbnail-item {
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 5px;
  cursor: pointer;
  border-radius: 4px;
  transition: all 0.2s;
}

.thumbnail-item:hover {
  background-color: #d6eaf8;
}

.thumbnail-current {
  background-color: #3498db;
  color: white;
}

.thumbnail-current:hover {
  background-color: #2980b9;
}

.content-panel {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
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

.element-hover-indicator {
  position: absolute;
  bottom: -20px;
  left: 0;
  font-size: 12px;
  color: #7f8c8d;
  opacity: 0;
  transition: opacity 0.2s;
}

.text-element:hover .element-hover-indicator,
.image-element:hover .element-hover-indicator,
.table-element:hover .element-hover-indicator {
  opacity: 1;
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

.hover-area {
  position: absolute;
  bottom: -40px;
  left: 0;
  width: 100%;
  height: 50px;
  /* 扩大可hover区域 */
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

/* 扩大内容元素的hover区域 */
.content-element {
  position: relative;
  padding-bottom: 20px;
  /* 为提示区域留出空间 */
}

.content-element.hovered {
  transform: scale(1.02);
  z-index: 10;
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
  top: 0px;
  /* 与控制栏高度一致 */
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