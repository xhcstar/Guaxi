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
      <!-- 修改PDF渲染区域 -->
      <div class="pdf-render-panel">
        <img v-if="elements.length > 0 && elements[0].type === 'image'" :src="elements[0].content"
          alt="Rendered PDF Page" class="rendered-pdf-image" />
        <div v-else class="pdf-loading-message">
          正在加载PDF页面...
        </div>
      </div>

      <!-- 右侧Markdown内容 -->
      <div class="markdown-panel">
        <div class="markdown-content">
          <div v-for="(line, index) in markdownLines" :key="index" class="markdown-line"
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
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join } from '@tauri-apps/api/path'
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'

import { marked } from 'marked'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import Prism from 'prismjs'
import 'prismjs/themes/prism.css'
import mermaid from 'mermaid'

const store = repoStore()
const { librariesRoot } = storeToRefs(store)

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
const pdfUrl = ref('')
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


// 渲染数学公式
function renderMath(html) {
  const latexRegex = /\$\$(.+?)\$\$|\$(.+?)\$/g;
  return html.replace(latexRegex, (match, blockMath, inlineMath) => {
    let mathContent = blockMath || inlineMath;
    try {
      if (blockMath) {
        return `<div class="katex-display">${katex.renderToString(mathContent, { throwOnError: false })}</div>`;
      } else {
        return `<span class="katex-inline">${katex.renderToString(mathContent, { throwOnError: false })}</span>`;
      }
    } catch (e) {
      console.error("KaTeX 渲染错误：", e);
      return match; // 如果渲染失败，返回原始文本
    }
  });
}

const renderCodeBlocks = (content) => {
  const codeBlocks = content.match(/```[\s\S]+?```/g) || [];

  codeBlocks.forEach((block) => {
    const language = block.match(/```(\w+)\n/);  // 获取语言类型
    if (language && language[1]) {
      const lang = language[1].toLowerCase();
      if (Prism.languages[lang]) {
        block = block.replace(/```(\w+)\n([\s\S]+?)```/g, (match, p1, p2) => {
          const highlightedCode = Prism.highlight(p2, Prism.languages[lang], lang);
          return `<pre class="code-block"><code class="language-${lang}">${highlightedCode}</code></pre>`;
        });
      }
    }
  });

  return content.replace(/```[\s\S]+?```/g, (block) => {
    return `<pre class="code-block">${block}</pre>`;
  });
};

const renderMermaid = (content) => {
  const mermaidRegex = /```mermaid([\s\S]+?)```/g
  return content.replace(mermaidRegex, (match, diagram) => {
    const mermaidId = `mermaid-${Math.random().toString(36).substring(2, 9)}`
    nextTick(() => {
      const target = document.getElementById(mermaidId)
      if (target) {
        target.style.width = '100%' // 确保容器宽度占满父容器
        target.style.height = 'auto' // 高度自适应
        mermaid.render(mermaidId, diagram, (svgCode) => {
          // 移除或调整 viewBox 和 max-width
          svgCode = svgCode.replace(/viewBox="[^"]+"/, 'viewBox="0 0 100 100"')
          svgCode = svgCode.replace(/style="[^"]+"/, 'style="width: 100%; height: auto;"')
          target.innerHTML = svgCode
        })
      }
    })
    return `<div id="${mermaidId}" class="mermaid">${diagram}</div>`
  })
}

const renderMarkdown = (content) => {
  if (!content) {
    console.warn('Markdown 内容为空或未定义')
    return ''
  }
  // console.log('Markdown 内容:', content)

  // 渲染 Mermaid 图表
  let html = renderMermaid(content);

  html = marked(html);

  // 渲染数学公式
  html = renderMath(html);

  // 渲染代码块
  html = renderCodeBlocks(html);

  // 添加表格样式
  html = html.replace(
    /<table>/g,
    '<table style="border-collapse: collapse; width: 100%;">'
  );
  html = html.replace(
    /<th>/g,
    '<th style="border: 1px solid black; padding: 8px; background: linear-gradient(to right, #7e807f, #7e806f); color: white; font-weight: bold; border-radius: 4px; box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2); transition: background-color 0.3s ease;">'
  );
  html = html.replace(
    /<td>/g,
    '<td style="border: 1px solid black; padding: 8px; background-color: #f9f9f9; color: #333;">'
  );

  nextTick(() => {
    Prism.highlightAll();
    mermaid.run();
  });

  // console.log('渲染后的 Markdown 内容:', html)

  return html;
}

// 计算Markdown文件路径
const markdownFilePath = computed(async () => {
  const baseFileName = props.fileName.replace('.pdf', '')
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'markdown',
    baseFileName,
    `${baseFileName}.md`
  )
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

const showNoteBubbleMessage = () => {
  showNoteBubble.value = true
  setTimeout(() => {
    showNoteBubble.value = false
  }, 2000)
}

// 加载PDF页面
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

    const data = JSON.parse(response);
    currentPage.value = data.page_number;
    totalPages.value = data.total_pages;

    // 显示渲染后的PDF图像
    elements.value = [{
      type: "image",
      content: `data:image/png;base64,${data.image_data}`,
      key: `rendered_page_${pageNum}`
    }];

  } catch (error) {
    console.error('PDF加载失败:', error);
    elements.value = [];
    if (error instanceof Error) {
      alert(`PDF加载错误: ${error.message}`);
    }
  }
};

// PDF加载事件处理
const onPdfLoad = () => {
  console.log('PDF加载成功')
}

const onPdfError = () => {
  console.error('PDF加载失败')
}

// 加载Markdown文件
const loadMarkdownFile = async () => {
  try {
    const filepath = await markdownFilePath.value
    const response = await invoke('load_markdown_content', {
      filepath: filepath
    })

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown')
    }

    markdownContent.value = response.content
    console.log('Markdown内容:', markdownContent.value)
    // 确保 content 是字符串
    let content = typeof response.content === 'string' ? response.content : ''
    markdownLines.value = renderMarkdown(content)
    return response.content
  } catch (error) {
    console.error('加载Markdown失败:', error)
    markdownContent.value = ''
    markdownLines.value = []
    throw error
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
  store.get_librariesRoot().then(() => {
    loadPdfPage(1)
    loadMarkdownFile()
  })
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

.pdf-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 20px;
  background-color: #2c3e50;
  color: white;
  flex-shrink: 0;
}

.page-info {
  font-size: 14px;
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

.right-controls {
  display: flex;
  gap: 10px;
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

.toggle-notes {
  background-color: #3498db;
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

.pdf-render-panel {
  flex: 1;
  overflow: hidden;
  border-right: 1px solid #ddd;
}

.pdf-object {
  width: 100%;
  height: 100%;
  border: none;
}

/* 添加渲染PDF图像的样式 */
.rendered-pdf-image {
  width: 100%;
  height: auto;
  max-height: 100%;
  object-fit: contain;
}

.pdf-loading-message {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  color: #666;
}

.markdown-panel {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  background-color: #f9f9f9;
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
  background-color: #e8f4fc;
}

.notes-panel {
  position: fixed;
  top: 0;
  right: -400px;
  width: 400px;
  height: 100%;
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
  background-color: #2c3e50;
  color: white;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.notes-content {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}

.notes-textarea {
  width: 100%;
  height: 100%;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 4px;
  resize: none;
  font-family: inherit;
  font-size: 14px;
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

.note-added-bubble {
  position: fixed;
  top: 70px;
  right: 20px;
  background-color: #27ae60;
  color: white;
  padding: 10px 20px;
  border-radius: 20px;
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.2);
  z-index: 1000;
  display: flex;
  align-items: center;
  gap: 10px;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>