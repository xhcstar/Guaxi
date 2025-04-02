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
      <div class="markdown-panel" @click="handleMarkdownClick" @mousemove="handleMarkdownHover"
        @mouseleave="handleMarkdownLeave">
        <div class="markdown-content" ref="markdownContainer" v-html="renderedMarkdown"></div>
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

const renderMarkdown = (content: string) => {
  const renderer = {
    ...new marked.Renderer(),
    image(token: any) {
      const src = token.href || '';
      console.log('图片路径:', src); // 调试日志

      return `<img src="${src}" 
                  alt="${token.text || ''}" 
                  title="${token.title || ''}" 
                  class="markdown-image"
                  onerror="console.error('图片加载失败:', this.src); this.style.display='none'">`;
    }
  };

  marked.use({ 
    renderer,
    mangle: false,
    headerIds: false
  });

  // 其余处理保持不变
  content = renderMermaid(content);
  content = renderCodeBlocks(content);
  content = renderMath(content);

  const html = marked.parse(content);
  return html.split('\n')
    .filter(line => line.trim())
    .map(line => ({
      raw: getRawContent(line),
      html: line
    }));
};

const renderedMarkdown = computed(() => {
  return markdownLines.value.map(line => line.html).join('')
})

const handleMarkdownClick = (e) => {
  // 清除之前选中的元素
  const prevSelected = document.querySelectorAll('.markdown-content .selected');
  prevSelected.forEach(el => el.classList.remove('selected'));

  // 获取点击的元素
  const target = e.target;

  // 如果是代码块、数学公式等特殊元素，则跳过
  if (target.closest('pre, .katex')) {
    return;
  }

  // 获取选中的文本
  const selection = window.getSelection();
  const selectedText = selection.toString().trim();

  let selectedElement = null;
  let message = '';

  if (selectedText && selection.rangeCount > 0) {
    // 添加文本到笔记
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;
    addToNotes({
      type: 'text',
      content: selectedText
    });
    selection.removeAllRanges();
  } else if (target.tagName === 'IMG') {
    // 添加图片到笔记
    selectedElement = target;
    message = '已选中图片';
    addToNotes({
      type: 'image',
      content: target.src
    });
  } else {
    // 获取最近的段落内容
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      message = `已选中段落: ${paragraph.textContent.substring(0, 30)}${paragraph.textContent.length > 30 ? '...' : ''}`;
      addToNotes({
        type: 'text',
        content: paragraph.textContent || ''
      });
    }
  }

  // 添加选中效果
  if (selectedElement) {
    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    // 确保元素可见
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }
}

const showSelectionMessage = (message: string) => {
  // 移除旧的消息
  const oldMessage = document.querySelector('.selection-message');
  if (oldMessage) oldMessage.remove();

  const messageEl = document.createElement('div');
  messageEl.className = 'selection-message';
  messageEl.textContent = message;
  document.body.appendChild(messageEl);

  setTimeout(() => {
    messageEl.classList.add('fade-out');
    setTimeout(() => messageEl.remove(), 500);
  }, 1500);
}
const handleMarkdownHover = (e) => {
  const target = e.target;

  // 清除之前的悬停效果
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });

  // 如果是代码块、数学公式等特殊元素，则跳过
  if (target.closest('pre, .katex')) {
    return;
  }

  // 获取最近的段落元素
  const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
  if (paragraph) {
    paragraph.classList.add('hovered');
  }
}

const handleMarkdownLeave = () => {
  // 清除所有悬停效果
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });
}

const markdownFilePath = ref('')

// 在onMounted或watch中更新路径
const updateMarkdownPath = async () => {
  const baseFileName = props.fileName.replace('.pdf', '')
  markdownFilePath.value = await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'markdown',
    baseFileName,
    `${baseFileName}.md`
  )
}

// 辅助函数：从HTML中提取原始Markdown内容
const getRawContent = (html) => {
  // 简单实现：去除HTML标签
  return html.replace(/<[^>]+>/g, '').trim();
};

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

// 加载Markdown页面
const loadMarkdownPage = async (pageNum: number) => {
  try {
    if (!markdownFilePath.value) {
      await updateMarkdownPath()
    }

    const response = await invoke('load_markdown_content', {
      filepath: markdownFilePath.value,
      page: pageNum - 1
    });

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    // // 获取markdown文件所在目录
    // const mdDir = await join(markdownFilePath.value, '..');
    
    // // 处理图片路径 - 确保返回绝对路径
    // const imgMatches = [...response.content.matchAll(/!\[(.*?)\]\((.*?)\)/g)];
    // const processedParts = await Promise.all(
    //   imgMatches.map(async (match) => {
    //     const [fullMatch, alt, src] = match;
    //     if (!src.startsWith('http') && !src.startsWith('/') && !src.startsWith('data:')) {
    //       // 构建绝对路径
    //       const imgPath = await join(mdDir, src);
    //       // 将Windows路径转换为正斜杠
    //       const normalizedPath = imgPath.replace(/\\/g, '/');
    //       return { match: fullMatch, replacement: `![${alt}](${normalizedPath})` };
    //     }
    //     return { match: fullMatch, replacement: fullMatch };
    //   })
    // );

    // 构建最终内容
    // let content = response.content;
    // processedParts.forEach(({ match, replacement }) => {
    //   content = content.replace(match, replacement);
    // });

    markdownContent.value = response.content;
    console.log('渲染前的 Markdown 内容:', markdownContent.value);
    markdownLines.value = renderMarkdown(markdownContent.value);
    console.log('渲染后的 Markdown 内容:', markdownLines.value);
  } catch (error) {
    console.error('加载Markdown失败:', error);
    markdownContent.value = '';
    markdownLines.value = [];
  }
};

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

// 分页控制 - 统一处理PDF和Markdown的页面切换
const prevPage = () => {
  if (currentPage.value > 1) {
    const newPage = currentPage.value - 1;
    currentPage.value = newPage;
    jumpPage.value = newPage;
    loadPdfPage(newPage);
    loadMarkdownPage(newPage);
  }
};

const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    const newPage = currentPage.value + 1;
    currentPage.value = newPage;
    jumpPage.value = newPage;
    loadPdfPage(newPage);
    loadMarkdownPage(newPage);
  }
};

const goToPage = (page) => {
  const pageNum = typeof page === 'number' ? page : jumpPage.value;
  const targetPage = Math.max(1, Math.min(pageNum, totalPages.value));
  currentPage.value = targetPage;
  jumpPage.value = targetPage;
  loadPdfPage(targetPage);
  loadMarkdownPage(targetPage);
};

// UI控制
const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value
}

onMounted(() => {
  store.get_librariesRoot().then(async () => {
    await updateMarkdownPath();
    loadPdfPage(1);
    loadMarkdownPage(1);
  });
});

watch(() => props.filePath, async (newPath) => {
  if (newPath) {
    await updateMarkdownPath();
    currentPage.value = 1;
    loadPdfPage(1);
    loadMarkdownPage(1);
    notesContent.value = '';
  }
});
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

.markdown-content {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  line-height: 1.6;
  color: #333;
  user-select: text;
  padding: 10px;
  cursor: text;
}

.markdown-content :deep(p),
.markdown-content :deep(h1),
.markdown-content :deep(h2),
.markdown-content :deep(h3),
.markdown-content :deep(h4),
.markdown-content :deep(h5),
.markdown-content :deep(h6),
.markdown-content :deep(li),
.markdown-content :deep(blockquote) {
  position: relative;
  transition: all 0.2s ease;
  padding: 4px 8px;
  margin: 4px 0;
  border-radius: 4px;
}

.markdown-content :deep(.markdown-image) {
  max-width: 100%;
  height: auto;
  margin: 10px 0;
  border-radius: 4px;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
  cursor: pointer;
  transition: transform 0.2s;
}

.markdown-content :deep(.markdown-image:hover) {
  transform: scale(1.02);
}

.markdown-content :deep(.selected),
.markdown-content :deep(.hovered) {
  background-color: rgba(52, 152, 219, 0.2);
  box-shadow: 0 0 0 2px rgba(52, 152, 219, 0.5);
  transform: scale(1.005);
  outline: none;
}

.markdown-content :deep(.selected) {
  background-color: rgba(52, 152, 219, 0.3);
  box-shadow: 0 0 0 3px rgba(52, 152, 219, 0.7);
}

.selection-message {
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  background-color: rgba(0, 0, 0, 0.8);
  color: white;
  padding: 10px 20px;
  border-radius: 20px;
  z-index: 1001;
  opacity: 1;
  transition: opacity 0.5s ease;
  font-size: 14px;
  pointer-events: none;
}

.selection-message.fade-out {
  opacity: 0;
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
  height: 19px;
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