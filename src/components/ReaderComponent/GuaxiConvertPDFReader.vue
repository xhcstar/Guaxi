<template>
  <div class="pdf-viewer-container">
    <!-- 顶部控制栏 -->
    <div class="pdf-controls">
      <div class="page-info">
        <span>第 {{ currentPage }} 页 / 共 {{ totalPages }} 页</span>
      </div>

      <div class="page-navigation">
        <button @click="prevPage" :disabled="currentPage <= 1" title="上一页">
          <n-icon :component="ArrowBackOutline" />
          <span class="btn-text">上一页</span>
        </button>

        <input type="number" v-model.number="jumpPage" min="1" :max="totalPages" @keyup.enter="goToPage">

        <button @click="nextPage" :disabled="currentPage >= totalPages" title="下一页">
          <span class="btn-text">下一页</span>
          <n-icon :component="ArrowForwardOutline" />
        </button>
      </div>

      <div class="right-controls">
        <button class="rotate-left" @click="rotate(-90)" title="向左旋转90度">
          <n-icon :component="RefreshCircleOutline" :rotate="90" />
          <span class="btn-text">左旋</span>
        </button>
        <button class="rotate-right" @click="rotate(90)" title="向右旋转90度">
          <n-icon :component="RefreshCircleOutline" :rotate="-90" />
          <span class="btn-text">右旋</span>
        </button>
        <button class="fit-width" @click="fitToWidth" title="将页面调整为适合宽度">
          <n-icon :component="ExpandOutline" />
          <span class="btn-text">适合宽度</span>
        </button>
        <button class="zoom-out" @click="zoomOut" :disabled="zoomLevel <= 0.5" title="缩小">
          <n-icon :component="RemoveOutline" />
        </button>
        <span class="zoom-level">{{ Math.round(zoomLevel * 100) }}%</span>
        <button class="zoom-in" @click="zoomIn" :disabled="zoomLevel >= 2" title="放大">
          <n-icon :component="AddOutline" />
        </button>
        <!-- 显示/隐藏笔记按钮 -->
        <button class="toggle-notes" @click="toggleNotesPanel" :title="showNotes ? '隐藏笔记' : '显示笔记'">
          <n-icon :component="DocumentTextOutline" />
          <span class="btn-text">{{ showNotes ? '隐藏笔记' : '显示笔记' }}</span>
        </button>

        <!-- 导出笔记按钮 -->
        <button class="export-notes" @click="exportNotes" title="导出笔记">
          <n-icon :component="DownloadOutline" />
          <span class="btn-text">导出笔记</span>
        </button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="pdf-main-content">
      <!-- 修改PDF渲染区域 -->
      <div class="pdf-render-panel" @wheel="handleWheelZoom" ref="pdfPanel">
        <div class="pdf-image-container" :style="{
          transform: `scale(${zoomLevel}) rotate(${rotation}deg)`,
          transformOrigin: 'center center'
        }">
          <img v-if="elements.length > 0 && elements[0].type === 'image'" :src="elements[0].content"
            alt="Rendered PDF Page" class="rendered-pdf-image" ref="pdfImage" />
          <div v-else class="pdf-loading-message">
            正在加载PDF页面...
          </div>
        </div>
      </div>

      <!-- 可拖动的中间线 -->
      <div class="resize-handle" @mousedown="startResize"></div>

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
          <n-icon :component="CloseOutline" size="18" />
        </button>
      </div>
      <div class="notes-content" ref="notesEditor" contenteditable="true" @input="handleNotesInput"
        @paste="handleNotesPaste" @keydown="handleNotesKeyDown" v-html="processedNotesContent"
        placeholder="点击Markdown内容添加到笔记..."></div>
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
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join, dirname } from '@tauri-apps/api/path'
import { convertFileSrc } from '@tauri-apps/api/core';
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'


import {
  ArrowBackOutline,
  ArrowForwardOutline,
  RefreshCircleOutline,
  ExpandOutline,
  SearchOutline,
  AddOutline,
  RemoveOutline,
  DocumentTextOutline,
  DownloadOutline,
  CloseOutline
} from '@vicons/ionicons5'

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

const store = repoStore()
const { librariesRoot } = storeToRefs(store)
const markdownContainer = ref(null);
// 缩放状态
const zoomLevel = ref(1);
// 添加旋转状态
const rotation = ref(0);
const pdfPanel = ref(null);
const pdfImage = ref(null);
// 添加拖动状态
const isResizing = ref(false);
const startX = ref(0);
const startLeftWidth = ref(0);
const leftPanelWidth = ref('50%'); // 初始宽度
// PDF数据状态
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const elements = ref([])
// Markdown数据状态
const markdownContent = ref('')
// const markdownLines = ref([])
const markdownLines = ref([]);
const markdownFilePath = ref('')
// 笔记状态
const showNotes = ref(false)
const notesContent = ref('')
// UI状态
const showNoteBubble = ref(false)

async function processImages(content, basePath = '') {
  const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
  const replacements = [];

  for (const match of content.matchAll(imgRegex)) {
    const [full, alt, src] = match;
    if (!src.match(/^(http|\/|data:image)/i)) {
      try {
        const fullPath = await join(basePath, src);
        const safeSrc = await convertFileSrc(fullPath);
        replacements.push({ from: full, to: `![${alt}](${safeSrc})` });
      } catch (error) {
        console.warn(`路径转换失败: ${src}`, error);
      }
    }
  }
  return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
}

async function convertPdfRenderMarkdown(content, options = {}) {
  const processedContent = await processImages(content, options.libraryBasePath);
  try {
    let html = await renderMarkdown(processedContent, {
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: 'default'
    });

    // 确保DOM更新后添加按钮
    nextTick(() => {
      if (markdownContainer.value) {
        addCopyButtons(markdownContainer.value);
      }
    });

    // 创建临时DOM元素来解析HTML
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = html;

    // 递归遍历DOM节点并拆分
    const splitByNodes = (node) => {
      const result = [];

      // 处理文本节点
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent.trim();
        if (text) {
          result.push({
            raw: text,
            html: text
          });
        }
        return result;
      }

      // 处理元素节点
      if (node.nodeType === Node.ELEMENT_NODE) {
        // 特殊处理这些元素，保持其完整性
        const keepWholeTags = ['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
          'img', 'table', 'pre', 'code', 'div.katex-display',
          'span.katex-inline', 'div.mermaid', 'svg.mermaid',
          // 添加MathJax相关标签
          'mjx-container', 'mjx-math', 'mjx-assistive-mml',
          // 修改代码高亮相关标签 - 移除无效选择器
          'div.hljs', 'code.hljs',
          // 添加所有hljs开头的类名元素
          '[class^="hljs-"]', '[class*=" hljs-"]'];

        if (keepWholeTags.some(tag => node.matches(tag))) {
          const outerHTML = node.outerHTML;
          result.push({
            raw: outerHTML,
            html: outerHTML
          });
          return result;
        }

        // 递归处理子节点
        for (const child of node.childNodes) {
          result.push(...splitByNodes(child));
        }
        return result;
      }

      return result;
    };

    // 从临时div的子节点开始处理
    const lines = [];
    for (const child of tempDiv.childNodes) {
      lines.push(...splitByNodes(child));
    }

    return lines;
  } catch (error) {
    console.error('[ERROR] 渲染失败:', error);
    return [{
      raw: content,
      html: `<div class="render-error">${error.message}</div>`
    }];
  }
}

const renderedMarkdown = computed(() => {
  return markdownLines.value.map(line => line.html).join('')
})

function determineNoteType(target) {
  const timestamp = new Date().toLocaleString();
  
  // 1. 优先检查数学公式
  if (target.closest('mjx-container, mjx-math')) {
    return 'math';
  }
  
  // 2. 检查Mermaid图表
  const mermaidContainer = target.closest('.mermaid, [data-processed="true"]');
  if (mermaidContainer) {
    // 确认是Mermaid而非普通代码块
    if (mermaidContainer.querySelector('svg') || mermaidContainer.classList.contains('mermaid')) {
      return 'diagram';
    }
  }
  
  // 3. 检查代码块
  const pre = target.closest('pre');
  if (pre) {
    // 排除图表
    if (!pre.classList.contains('mermaid')) {
      // 检测标准代码结构或高亮类
      if (pre.querySelector('code') || 
          [...pre.classList].some(c => ['hovered','hljs','code'].includes(c))) {
        return 'code';
      }
    }
  }
  
  // 4. 检查图片
  if (target.tagName === 'IMG') {
    return 'image';
  }
  
  // 默认文本类型
  return 'text';
}

const handleMarkdownClick = (e) => {
  // 清除之前选中的元素
  const prevSelected = document.querySelectorAll('.markdown-content .selected');
  prevSelected.forEach(el => el.classList.remove('selected'));

  // 获取点击的元素
  const target = e.target;

  // 获取选中的文本
  const selection = window.getSelection();
  const selectedText = selection.toString().trim();

  let selectedElement = null;
  let message = '';
  let noteContent = '';
  let noteHtml = '';

  // 处理代码块点击
  const codeBlock = target.closest('pre, code, .code-block');
  if (codeBlock) {
    selectedElement = codeBlock;
    const codeContent = codeBlock.textContent || '';
    message = `已选中代码块: ${codeContent.substring(0, 30)}${codeContent.length > 30 ? '...' : ''}`;
    noteContent = `\`\`\`\n${codeContent}\n\`\`\``;
    noteHtml = codeBlock.outerHTML; // 直接使用原始HTML
  }
  // 处理数学公式点击
  else if (target.closest('mjx-container, mjx-math')) {
    selectedElement = target.closest('mjx-container, mjx-math');
    const mathContent = selectedElement.getAttribute('data-mjx-text') || selectedElement.textContent || '';
    message = `已选中数学公式: ${mathContent.substring(0, 30)}${mathContent.length > 30 ? '...' : ''}`;
    noteContent = `$$${mathContent}$$`;
    noteHtml = selectedElement.outerHTML; // 直接使用原始HTML
  }
  // 处理mermaid图表点击
  else if (target.closest('.mermaid, svg')) {
    selectedElement = target.closest('.mermaid, svg');
    const mermaidContent = selectedElement.querySelector('.mermaid')?.textContent || selectedElement.textContent || '';
    message = '已选中图表';
    noteContent = `\`\`\`mermaid\n${mermaidContent}\n\`\`\``;
    noteHtml = selectedElement.outerHTML; // 直接使用原始HTML
  }
  // 处理图片点击
  else if (target.tagName === 'IMG') {
    selectedElement = target;
    message = '已选中图片';
    noteContent = `![图片](${target.src})`;
    noteHtml = target.outerHTML; // 直接使用原始HTML
  }
  // 处理文本选择
  else if (selectedText && selection.rangeCount > 0) {
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;
    noteContent = selectedText;
    noteHtml = `<p>${selectedText}</p>`; // 包装成段落
  }
  // 处理段落点击
  else {
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      const textContent = paragraph.textContent || '';
      message = `已选中段落: ${textContent.substring(0, 30)}${textContent.length > 30 ? '...' : ''}`;
      noteContent = textContent;
      noteHtml = paragraph.outerHTML; // 直接使用原始HTML
    }
  }

  // 添加选中效果和笔记
  if (selectedElement) {
    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    // 添加到笔记
    const timestamp = new Date().toLocaleString();
    const noteType = determineNoteType(target);

    addToNotes({
      type: noteType,
      content: noteContent,
      html: noteHtml // 添加HTML格式内容
    });
  }
}

const showSelectionMessage = (message) => {
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

  // 获取最近的段落元素或特殊元素
  const element = target.closest(
    'p, h1, h2, h3, h4, h5, h6, li, blockquote, ' +
    'pre, code, .code-block, ' +  // 代码块
    '.katex-display, .katex-inline, ' +  // 数学公式
    '.mermaid, svg'  // mermaid图表和生成的SVG元素
  );

  if (element) {
    element.classList.add('hovered');
  }
}

const handleMarkdownLeave = () => {
  // 清除所有悬停效果
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });
}

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

const showNoteBubbleMessage = () => {
  showNoteBubble.value = true
  setTimeout(() => {
    showNoteBubble.value = false
  }, 2000)
}

// 加载PDF页面
const loadPdfPage = async (pageNum) => {
  try {
    const response = await invoke('get_converted_pdf_page', {
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
const loadMarkdownPage = async (pageNum) => {
  try {
    if (!markdownFilePath.value) {
      await updateMarkdownPath();
    }

    const response = await invoke('load_markdown_content', {
      filepath: markdownFilePath.value,
      page: pageNum - 1
    });

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    // 获取markdown文件所在目录
    const mdDir = await dirname(markdownFilePath.value);

    // // 修正后的正则表达式（匹配标准Markdown图片语法）
    // const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
    // let processedContent = response.content;
    // let match;

    // // 创建替换映射表
    // const replacements: { original: string; replacement: string }[] = [];

    // // 先收集所有需要替换的内容
    // while ((match = imgRegex.exec(response.content)) !== null) {
    //   const [fullMatch, altText, imgPath] = match;
    //   console.debug('匹配到图片:', { altText, imgPath });

    //   // 只处理本地相对路径
    //   if (!imgPath.match(/^(http|https|data:|\/)/i)) {
    //     try {
    //       // 处理Windows路径中的反斜杠
    //       const normalizedPath = imgPath.replace(/\\/g, '/');
    //       const fullFsPath = await join(mdDir, normalizedPath);
    //       const tauriPath = convertFileSrc(fullFsPath);

    //       replacements.push({
    //         original: fullMatch,
    //         replacement: `![${altText}](${tauriPath})`
    //       });

    //       console.debug('图片路径转换:', {
    //         original: imgPath,
    //         converted: tauriPath
    //       });
    //     } catch (error) {
    //       console.error(`路径转换失败: ${imgPath}`, error);
    //       replacements.push({
    //         original: fullMatch,
    //         replacement: `![${altText}](${imgPath}) <!-- 转换失败 -->`
    //       });
    //     }
    //   }
    // }

    // // 执行批量替换（避免重复处理）
    // replacements.forEach(({ original, replacement }) => {
    //   processedContent = processedContent.replace(original, replacement);
    // });

    // markdownContent.value = processedContent;
    markdownContent.value = response.content;
    // console.log("markdownContent.value:", markdownContent.value);
    // markdownLines.value = await renderMarkdown(markdownContent.value, {
    //   libraryBasePath: mdDir
    // });
    markdownLines.value = await convertPdfRenderMarkdown(markdownContent.value,
      { libraryBasePath: mdDir }
    );
    console.log("markdownLines.value:", markdownLines.value);

  } catch (error) {
    console.error('加载Markdown失败:', error);
    markdownLines.value = [{
      raw: '',
      html: `<div class="error">加载错误: ${error.message}</div>`
    }];
  }
};

const getNotesDirectory = async () => {
  const baseFileName = props.fileName.replace('.pdf', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'notes'
  );
};

const getImagesDirectory = async () => {
  const baseFileName = props.fileName.replace('.pdf', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'images'
  );
};

const processedNotesContent = ref('')
const notesEditor = ref(null)
let isComposing = false

// 添加保存笔记方法
const saveNotes = async () => {
  try {
    const notesDir = await getNotesDirectory();
    const content = notesEditor.value.innerText; // 获取纯文本内容
    // 或者使用HTML内容：const content = notesEditor.value.innerHTML;
    
    const response = await invoke('save_notes_to_file', {
      content: content,
      notesDir: notesDir,
      fileName: props.fileName.replace('.pdf', '')
    });

    console.log('笔记保存成功:', response);
    showNoteBubbleMessage();
  } catch (error) {
    console.error('保存笔记失败:', error);
    alert(`保存笔记失败: ${error.message}`);
  }
};

// 改进后的输入处理
const handleNotesInput = debounce(() => {
  if (isComposing) return
  
  // 保存选区
  const selection = window.getSelection()
  const range = selection.rangeCount > 0 ? selection.getRangeAt(0) : null
  const cursorState = range ? {
    node: range.startContainer,
    offset: range.startOffset
  } : null
  
  // 获取并处理内容
  const html = notesEditor.value.innerHTML
  notesContent.value = html
  processedNotesContent.value = renderInlineHtml(html)
  
  // 恢复选区
  if (cursorState && cursorState.node) {
    const newRange = document.createRange()
    newRange.setStart(cursorState.node, Math.min(cursorState.offset, cursorState.node.length))
    newRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newRange)
  }

  // 自动保存（可选）
  saveNotes();
}, 100)

// 改进的粘贴处理
const handleNotesPaste = (e) => {
  e.preventDefault()
  const text = e.clipboardData.getData('text/plain')
  
  // 使用现代API插入文本
  const selection = window.getSelection()
  if (!selection.rangeCount) return
  
  const range = selection.getRangeAt(0)
  range.deleteContents()
  
  // 创建文本节点保持格式
  const textNode = document.createTextNode(text)
  range.insertNode(textNode)
  
  // 移动光标到插入内容之后
  range.setStartAfter(textNode)
  range.collapse(true)
  selection.removeAllRanges()
  selection.addRange(range)
}

// 改进的按键处理
const handleNotesKeyDown = (e) => {
  // 中文输入法状态下不处理
  if (isComposing) return
  
  // Enter键处理
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    
    const selection = window.getSelection()
    if (!selection.rangeCount) return
    
    const range = selection.getRangeAt(0)
    const p = document.createElement('p')
    p.innerHTML = '<br>'
    
    range.deleteContents()
    range.insertNode(p)
    
    // 在新段落开始位置设置光标
    const newRange = document.createRange()
    newRange.setStart(p, 0)
    newRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newRange)
  }
}

// 防抖函数
function debounce(fn, delay) {
  let timer = null
  return function(...args) {
    clearTimeout(timer)
    timer = setTimeout(() => fn.apply(this, args), delay)
  }
}

// 简单的HTML渲染函数（保持基本格式）
const renderInlineHtml = (html) => {
  // 保留换行符
  let processed = html.replace(/\n/g, '<br>')

  // 自动为段落添加p标签（当没有块级元素时）
  if (!/<(p|div|h[1-6]|blockquote|pre|ul|ol|li|table)/i.test(processed)) {
    processed = processed.split('<br><br>').join('</p><p>')
    processed = `<p>${processed}</p>`
  }

  return processed
}

// 生成唯一文件名（浏览器兼容方案）
function generateUniqueSvgName(content, type = 'mermaid') {
  // 简单哈希函数（替代Node.js的crypto）
  const simpleHash = (str) => {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash = hash & hash; // Convert to 32bit integer
    }
    return Math.abs(hash).toString(16).substring(0, 8);
  };

  // 使用内容哈希和时间戳
  const contentHash = simpleHash(content);
  const timestamp = Date.now();
  
  return `${type}-${contentHash}-${timestamp}.svg`;
}

async function processRenderedSvg(content, type = 'mermaid') {
  try {
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = content;
    
    const svgElement = tempDiv.querySelector('svg');
    if (!svgElement) return content;

    // 克隆并清理SVG
    const svgClone = svgElement.cloneNode(true);
    svgClone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    const svgContent = svgClone.outerHTML;
    
    // 生成唯一文件名（浏览器兼容）
    const fileName = generateUniqueSvgName(svgContent, type);
    const imageDir = await getImagesDirectory();
    
    // 调用Tauri后端保存
    const { relative_path } = await invoke('save_svg_file', {
      content: svgContent,
      fileName: fileName,
      imageDir: imageDir
    });
    const processedImagesPath = await processImages(`![${type}图表](../${relative_path})`, imageDir);
    console.log(`${type}图表已保存到: ${processedImagesPath}`);
    return processedImagesPath;
    
  } catch (error) {
    console.error(`处理${type} SVG失败:`, error);
    // 失败时返回原始内容，但添加错误标记
    return `<!-- SVG处理失败 -->\n${content}`;
  }
}

// 从渲染后的HTML代码块中提取原始代码
function extractCodeFromRendered(htmlContent) {
  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = htmlContent;

  // 尝试从不同结构的代码块中提取
  const codeElement = tempDiv.querySelector('pre code') ||
    tempDiv.querySelector('.hljs code') ||
    tempDiv.querySelector('code');

  if (codeElement) {
    return codeElement.textContent;
  }

  // 如果无法提取，尝试从原始内容中获取
  const match = htmlContent.match(/<code[^>]*>([\s\S]*?)<\/code>/);
  return match ? match[1] : htmlContent;
}

// 处理代码块内容
function processCodeContent(content, htmlContent) {
  // 提取原始代码
  const rawCode = extractCodeFromRendered(htmlContent || content);

  // 检测语言类型
  let language = '';
  const langMatch = content.match(/^```(\w+)/) ||
    (htmlContent || '').match(/language-(\w+)/);
  if (langMatch) language = langMatch[1];

  // 生成标准Markdown代码块
  return `\`\`\`${language}
${rawCode.trim()}
\`\`\``;
}

// 添加内容到笔记
const addToNotes = async (element) => {
  const timestamp = new Date().toLocaleString();
  let markdownNote = '';
  let htmlNote = '';
  let content = '';

  // 优先使用HTML内容，如果没有则使用Markdown内容
  const markdownContent = element.content;
  const htmlContent = element.html;

  // console.log("addToNotes raw content:", element.content);

  // 根据类型生成不同的视觉样式
  switch (element.type) {
    case 'text':
      markdownNote = `### ${markdownContent}\n\n`;
      htmlNote = `
        <div class="note-section note-text">
          <div class="note-header">
            <span class="note-icon">📝</span>
          </div>
          <div class="note-content">${htmlContent || `<p>${markdownContent}</p>`}</div>
        </div>
      `;
      break;
    case 'image':
      markdownNote = `${markdownContent}\n\n`;
      htmlNote = `
        <div class="note-section note-image">
          <div class="note-header">
            <span class="note-icon">🖼️</span>
          </div>
          <div class="note-content">${htmlContent || `<img src="${markdownContent.match(/!$$.*$$$(.*)$/)?.[1]}" alt="${timestamp}">`}</div>
        </div>
      `;
      break;
    case 'code':
      markdownNote = `${processCodeContent(element.content, element.html)}\n\n`;
      htmlNote = `
        <div class="note-section note-code">
          <div class="note-header">
            <span class="note-icon">💻</span>
          </div>
          <div class="note-content">${htmlContent || `<pre><code>${markdownContent.replace(/^```[^\n]*\n|\n```$/g, '')}</code></pre>`}</div>
        </div>
      `;
      break;
    case 'math':
      markdownNote = `${await processRenderedSvg(element.html, 'math')}\n\n`;
      htmlNote = `
        <div class="note-section note-math">
          <div class="note-header">
            <span class="note-icon">🧮</span>
          </div>
          <div class="note-content">${htmlContent || `<div class="math-expression">${markdownContent}</div>`}</div>
        </div>
      `;
      break;
    case 'diagram':
      markdownNote = `${await processRenderedSvg(element.html, 'mermaid')}\n\n`;
      htmlNote = `
        <div class="note-section note-diagram">
          <div class="note-header">
            <span class="note-icon">📊</span>
          </div>
          <div class="note-content">${htmlContent || `<div class="diagram-container">${markdownContent}</div>`}</div>
        </div>
      `;
      break;
    default:
      markdownNote = `${markdownContent}\n\n`;
      htmlNote = `
        <div class="note-section note-default">
          <div class="note-header">
            <span class="note-icon">📌</span>
          </div>
          <div class="note-content">${htmlContent || `<p>${markdownContent}</p>`}</div>
        </div>
      `;
  }

  // 更新笔记内容
  notesContent.value += markdownNote;
  processedNotesContent.value += htmlNote;

  // 滚动到底部
  nextTick(() => {
    notesEditor.value.scrollTop = notesEditor.value.scrollHeight;
  });

  const notesDir = await getNotesDirectory();

  try {
    content = markdownNote
    console.log('笔记类型:', element.type);
    console.log('markdown笔记内容:', htmlNote);
    console.log('笔记内容:', content);

    // 调用Tauri后端写入笔记
    const response = await invoke('save_notes_to_file', {
      content: content,
      notesDir: notesDir,
      fileName: props.fileName.replace('.pdf', '')
    });

    console.log('笔记保存成功:', response);
    showNoteBubbleMessage();
  } catch (error) {
    console.error('保存笔记失败:', error);
    alert(`保存笔记失败: ${error.message}`);
  }
}

// 导出笔记
const exportNotes = () => {
  if (!notesContent.value) {
    alert('笔记内容为空');
    return;
  }

  // 创建临时div来提取文本内容
  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = notesContent.value;

  // 转换内容为Markdown格式
  let exportContent = '';

  // 处理每个笔记条目
  const items = tempDiv.querySelectorAll('.note-item');
  items.forEach(item => {
    const type = item.querySelector('.note-type')?.textContent || '笔记';
    const time = item.querySelector('.note-time')?.textContent || '';
    const content = item.querySelector('.note-content')?.textContent || '';

    exportContent += `### ${type} (${time})\n\n`;
    exportContent += `${content}\n\n---\n\n`;
  });

  const blob = new Blob([exportContent], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);

  const a = document.createElement('a');
  a.href = url;
  a.download = `PDF笔记_${props.fileName.replace('.pdf', '')}_${new Date().toISOString().slice(0, 10)}.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

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

// 缩放功能方法
const zoomIn = () => {
  zoomLevel.value = Math.min(zoomLevel.value + 0.1, 2);
};

const zoomOut = () => {
  zoomLevel.value = Math.max(zoomLevel.value - 0.1, 0.5);
};

// 开始拖动
const startResize = (e) => {
  isResizing.value = true;
  startX.value = e.clientX;
  startLeftWidth.value = parseInt(document.defaultView.getComputedStyle(document.querySelector('.pdf-render-panel')).width, 10);
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', stopResize);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
};

// 处理鼠标移动
const handleMouseMove = (e) => {
  if (!isResizing.value) return;

  const containerWidth = document.querySelector('.pdf-main-content').offsetWidth;
  const dx = e.clientX - startX.value;
  let newLeftWidth = ((startLeftWidth.value + dx) * 100) / containerWidth;

  // 限制最小宽度
  newLeftWidth = Math.max(30, Math.min(70, newLeftWidth));

  leftPanelWidth.value = `${newLeftWidth}%`;
};

// 停止拖动
const stopResize = () => {
  isResizing.value = false;
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
};
// 旋转功能
const rotate = (degrees) => {
  rotation.value = (rotation.value + degrees) % 360;
};

const fitToWidth = () => {
  if (!pdfPanel.value || !pdfImage.value) return;

  const panelWidth = pdfPanel.value.clientWidth;
  const imgWidth = pdfImage.value.naturalWidth;

  // 考虑旋转状态
  const isVertical = Math.abs(rotation.value % 180) === 90;
  const effectiveWidth = isVertical ? pdfImage.value.naturalHeight : imgWidth;

  // 计算适合宽度，留出10px边距
  zoomLevel.value = (panelWidth - 10) / effectiveWidth;

  // 确保图片居中
  nextTick(() => {
    if (pdfPanel.value) {
      pdfPanel.value.scrollLeft = (pdfImage.value.clientWidth - panelWidth) / 2;
    }
  });
};

// 修改handleWheelZoom方法，考虑旋转状态
const handleWheelZoom = (e) => {
  if (e.ctrlKey) {
    e.preventDefault();
    const delta = e.deltaY < 0 ? 0.1 : -0.1;
    zoomLevel.value = Math.max(0.5, Math.min(2, zoomLevel.value + delta));
  }
};

onMounted(() => {
  const editor = notesEditor.value
  editor.addEventListener('compositionstart', () => { isComposing = true })
  editor.addEventListener('compositionend', () => { isComposing = false })
  store.get_librariesRoot().then(async () => {
    await updateMarkdownPath();
    loadPdfPage(1);
    loadMarkdownPage(1);

    // 加载现有笔记
    try {
      const notesDir = await getNotesDirectory();
      const response = await invoke('load_existing_notes', {
        notesDir: notesDir,
        fileName: props.fileName.replace('.pdf', '')
      });

      if (response.content) {
        // 保存原始Markdown内容
        notesContent.value = response.content;

        // 渲染Markdown为HTML
        const renderedHtml = await renderMarkdown(response.content, {
          basePath: "",
          mermaid: true,
          math: true,
          codeHighlight: true,
          theme: 'default'
        });

        // 处理渲染后的HTML
        processedNotesContent.value = renderedHtml
          .replace(/<p><\/p>/g, '') // 移除空段落
          .replace(/\n/g, '<br>'); // 保留换行
        console.log('processedNotesContent笔记内容:', processedNotesContent.value);
      }
    } catch (error) {
      console.error('加载笔记失败:', error);
      processedNotesContent.value = `<div class="error">加载笔记失败: ${error.message}</div>`;
    }

    // 添加窗口大小变化监听
    window.addEventListener('resize', handleResize);
    // 添加延迟确保DOM完全渲染
    setTimeout(() => {
      fitToWidth(); // 初始加载时自动适合宽度
    }, 100);
  });
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize);
  // 清理拖动事件监听
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
});

const handleResize = () => {
  if (pdfPanel.value && pdfImage.value) {
    fitToWidth();
  }
};

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
  padding: 8px 16px;
  background-color: #f8f9fa;
  border-bottom: 1px solid #e0e0e0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

button {
  padding: 8px 12px;
  background-color: #ffffff;
  color: #495057;
  border: 1px solid #e9ecef;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

button:hover {
  background-color: #f8f9fa;
  border-color: #e9ecef;
  transform: translateY(-1px);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

button:disabled {
  background-color: #f8f9fa;
  color: #adb5bd;
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}

.n-icon {
  font-size: 16px;
}

.page-info {
  font-size: 14px;
  font-weight: 500;
  color: #495057;
  padding: 6px 12px;
  background-color: #ffffff;
  border-radius: 6px;
  border: 1px solid #e9ecef;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.page-navigation {
  display: flex;
  align-items: center;
  gap: 10px;
}

.page-navigation input {
  width: 50px;
  padding: 6px;
  text-align: center;
  border: 1px solid #e9ecef;
  border-radius: 6px;
  background-color: #ffffff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.right-controls {
  display: flex;
  gap: 10px;
}

/* 添加缩放控制按钮样式 */
.zoom-level {
  min-width: 40px;
  text-align: center;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.zoom-in,
.zoom-out {
  background-color: #f8f9fa;
  color: #495057;
  border: 1px solid #e9ecef;
}

.zoom-in:hover,
.zoom-out:hover {
  background-color: #f1f3f5;
}

.zoom-in:disabled,
.zoom-out:disabled {
  background-color: #95a5a6;
  cursor: not-allowed;
}

.rotate-left,
.rotate-right,
.fit-width {
  background-color: #f8f9fa;
  color: #495057;
  border: 1px solid #e9ecef;
}

.rotate-left:hover,
.rotate-right:hover,
.fit-width:hover {
  background-color: #f1f3f5;
}

.toggle-notes {
  background-color: #4dabf7;
  color: white;
  border: 1px solid #339af0;
  transition: all 0.2s;
}

.toggle-notes:hover {
  background-color: #339af0;
  box-shadow: 0 2px 8px rgba(77, 171, 247, 0.3);
}

.export-notes {
  background-color: #20c997;
  color: white;
  border: 1px solid #12b886;
  transition: all 0.2s;
}

.export-notes:hover {
  background-color: #12b886;
  box-shadow: 0 2px 8px rgba(32, 201, 151, 0.3);
}

/* 保持按钮统一间距和形状 */
.toggle-notes,
.export-notes {
  padding: 8px 12px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.pdf-main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  gap: 0;
}

.pdf-render-panel {
  flex: 0 0 v-bind(leftPanelWidth);
  min-width: 0;
  overflow: auto;
  position: relative;
  background-color: #ffffff;
}

.pdf-image-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
  background-color: #f8f9fa;
}

.rendered-pdf-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  transition: transform 0.2s ease;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.pdf-loading-message {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 100%;
  color: #868e96;
  font-size: 14px;
}

.resize-handle {
  width: 3px;
  height: 100%;
  background-color: #e9ecef;
  cursor: col-resize;
  position: relative;
  z-index: 10;
  transition: background-color 0.2s;
}

.resize-handle:hover {
  background-color: #4acadb;
}

.resize-handle:active {
  background-color: #55ebd7;
}

.markdown-panel {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 5px;
  background-color: #ffffff;
}

.markdown-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 18px;
  line-height: 1.6;
  color: #000000;
  background-color: #ffffff;
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
  background-color: rgba(72, 215, 240, 0.2);
  box-shadow: 0 0 0 2px rgba(65, 210, 230, 0.5);
  transform: scale(1.005);
  outline: none;
}

.markdown-content :deep(.selected) {
  background-color: rgba(33, 213, 236, 0.3);
  box-shadow: 0 0 0 3px rgba(85, 231, 207, 0.7);
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
  right: -600px;
  width: 600px;
  height: 100%;
  background-color: #ffffff;
  box-shadow: -2px 0 10px rgba(0, 0, 0, 0.1);
  transition: right 0.3s ease;
  z-index: 100;
  display: flex;
  flex-direction: column;
  border-left: 1px solid #e9ecef;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 14px;
}

.notes-panel-visible {
  right: 0;
}

.notes-header {
  padding: 12px 16px;
  background-color: #4dabf7;
  color: white;
  display: flex;
  justify-content: space-between;
  align-items: center;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.notes-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
}

/* .notes-content {
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
} */

.close-btn {
  background: none;
  border: none;
  color: white;
  cursor: pointer;
  padding: 4px;
  border-radius: 50%;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background-color: rgba(255, 255, 255, 0.2);
  transform: rotate(90deg);
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

/* 笔记条目基础样式 */
.note-section {
  margin-bottom: 16px;
  padding: 12px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  transition: all 0.2s ease;
}

.note-section:hover {
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.1);
}

/* 笔记头部样式 - 修改为单行布局 */
.note-header {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  gap: 8px;
  flex-wrap: wrap;
}

.note-icon {
  font-size: 1.2em;
}

.note-type {
  font-weight: 500;
  color: #333;
  font-size: 0.95em;
}

.note-time {
  font-size: 0.85em;
  color: #888;
  margin-left: auto;
}

/* 笔记内容样式 */
.note-content {
  padding: 0 4px;
  line-height: 1.6;
}

/* 不同类型笔记的特殊样式 */
.note-text {
  border-left: 3px solid #4dabf7;
}

.note-code {
  border-left: 3px solid #20c997;
}

.note-code .note-content pre {
  background: #f8f8f8;
  padding: 8px;
  border-radius: 4px;
  overflow-x: auto;
  font-size: 0.9em;
}

.note-math {
  border-left: 3px solid #f76707;
}

.note-diagram {
  border-left: 3px solid #9775fa;
}

.note-image {
  border-left: 3px solid #f783ac;
}

.note-image .note-content img {
  max-width: 100%;
  border-radius: 4px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

/* 笔记容器样式 */
.notes-content {
  flex: 1;
  padding: 12px;
  overflow-y: auto;
  background-color: #f9f9f9;
}

.notes-content[contenteditable="true"] {
  min-height: 100px;
  background: white;
  border-radius: 6px;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.05);
}

/* 空状态提示 */
.notes-content:empty:before {
  content: attr(placeholder);
  color: #aaa;
  display: block;
  padding: 16px;
  font-style: italic;
}

/* 淡入动画 */
@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>

<style>
/* 数学公式样式 */
.math-inline {
  display: inline;
  vertical-align: middle;
}

.math-block {
  display: block;
  text-align: center;
  margin: 1rem 0;
}

/* Mermaid图表样式 */
.mermaid {
  background-color: white;
  border-radius: 8px;
  padding: 16px;
  margin: 1em 0;
  text-align: center;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}

.dark .mermaid {
  background-color: #2d2d2d;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}

/* 代码块样式 */
.hljs {
  position: relative;
  border-radius: 6px;
  padding: 1.5em 1em !important;
  font-size: 17px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  background: #f6f8fa !important;
  border: 1px solid #e1e4e8;
  margin: 1em 0;
}

.hljs code {
  font-family: inherit !important;
}

code:not(.hljs code) {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
  font-size: 0.9em;
}

.hljs:hover::before {
  opacity: 1;
}

/* 暗色模式代码块 */
.dark .hljs {
  background: #2d2d2d !important;
  border-color: #444;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace !important;
}

.dark .hljs code {
  font-family: inherit !important;
}

.dark .hljs::before {
  background: #444;
  color: #fff;
}

.copy-button {
  opacity: 1 !important;
  background: rgba(255, 255, 255, 0.8);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .copy-button {
  background: rgba(60, 60, 60, 0.8) !important;
  border-color: rgba(255, 255, 255, 0.1) !important;
}


.text-note {
  white-space: pre-wrap;
}

.code-note pre {
  background-color: #f6f8fa;
  padding: 12px;
  border-radius: 4px;
  overflow-x: auto;
}

.math-note {
  overflow-x: auto;
}

.diagram-note {
  text-align: center;
}
</style>