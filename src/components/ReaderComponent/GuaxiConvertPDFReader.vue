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

        <input type="number" v-model.number="jumpPage" min="1" :max="totalPages" @keyup.enter="goToPage"
          style="font-size: 12px;">

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
      <div class="pdf-render-panel" ref="pdfPanel" @wheel="handleWheelZoom" @mousedown="startDrag"
        @mousemove="handleDrag" @mouseup="endDrag" @mouseleave="endDrag"
        :style="{ cursor: isDragging ? 'grabbing' : 'grab' }">

        <div class="pdf-image-container" :style="{
          transform: `scale(${zoomLevel}) rotate(${rotation}deg)`,
          transformOrigin: 'center center',
          width: '100%',
          height: '100%'
        }">
          <!-- 图片/混合模式 -->
          <img v-if="renderContent?.content" :src="renderContent.content" @load="handleImageLoad"
            class="rendered-pdf-image" ref="pdfImage" />

          <!-- 加载状态 -->
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
      <div class="notes-content">
        <n-card v-for="(card, index) in noteCards" :key="card.id" class="note-card" size="small" :title="card.title"
          :bordered="true" :segmented="{
            content: true,
            footer: 'soft'
          }" header-style="padding: 12px 16px;" content-style="padding: 0;" @click="handleCardClick(index)">
          <template #header-extra>
            <n-button v-if="activeCardIndex === index" text @click.stop="deleteCard(index)">
              <n-icon :component="CloseOutline" size="14" />
            </n-button>
          </template>

          <!-- 根据笔记类型使用不同的渲染方式 -->
          <div v-if="card.type !== 'composite'" class="note-content" :contenteditable="activeCardIndex === index"
            @blur="saveCardEdit(index)" v-html="card.content">
          </div>

          <!-- 添加复合笔记的专用渲染 -->
          <div v-else class="composite-note-content">
            <div v-for="(segment, segIndex) in parseCompositeContent(card.html)" :key="segIndex" class="note-segment"
              :class="getSegmentClass(segment)">
              <div v-html="segment.content"></div>
            </div>
          </div>

          <template #footer>
            <n-text depth="3" style="font-size: 12px;">
              {{ formatTime(card.timestamp) }}
            </n-text>
          </template>
        </n-card>

        <div class="add-card-container">
          <n-button type="primary" dashed block @click="addNewCard">
            <template #icon>
              <n-icon :component="AddOutline" />
            </template>
            添加新卡片
          </n-button>
        </div>
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
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join, dirname } from '@tauri-apps/api/path'
import { convertFileSrc } from '@tauri-apps/api/core';
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { NCard, NButton, NIcon, NText, useMessage } from 'naive-ui'
import usePdfRenderer from './usePdfRenderer'
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

// 初始化 message
const message = useMessage()
const store = repoStore()
const { librariesRoot } = storeToRefs(store)
const markdownContainer = ref(null);
// const renderMode = ref('vector') // 默认渲染模式
// const renderContent = ref(null)  // 渲染内容

const {
  renderMode,
  loadPage,
  renderContent,
  textLayer,
  isLoading,
  error,
  currentPageData
} = usePdfRenderer(props);

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
// 添加状态锁防止重复计算
let isFitting = false
// 拖拽处理
const isDragging = ref(false)
const startDragPos = ref({ x: 0, y: 0 })
const startScrollPos = ref({ left: 0, top: 0 })
const velocity = ref({ x: 0, y: 0 })
const lastDragTime = ref(0)
const animationFrameId = ref(null)
const isDecelerating = ref(false)

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
        [...pre.classList].some(c => ['hovered', 'hljs', 'code'].includes(c))) {
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

// 添加全局状态管理
const altKeyCollector = ref({
  isActive: false,
  currentCard: null,
  timer: null
})

const handleMarkdownClick = (e) => {
  // 检查是否按下了Alt键
  const isAltPressed = e.altKey;

  // 如果按下Alt键且当前没有激活收集器，则开始新的收集
  if (isAltPressed && !altKeyCollector.value.isActive) {
    startAltKeyCollection();
    return; // 开始收集后直接返回，避免重复处理
  }

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
    noteHtml = codeBlock.outerHTML;
  }
  // 处理数学公式点击
  else if (target.closest('mjx-container, mjx-math')) {
    selectedElement = target.closest('mjx-container, mjx-math');
    const mathContent = selectedElement.getAttribute('data-mjx-text') || selectedElement.textContent || '';
    message = `已选中数学公式: ${mathContent.substring(0, 30)}${mathContent.length > 30 ? '...' : ''}`;
    noteContent = `$$${mathContent}$$`;
    noteHtml = selectedElement.outerHTML;
  }
  // 处理mermaid图表点击
  else if (target.closest('.mermaid, svg')) {
    selectedElement = target.closest('.mermaid, svg');
    const mermaidContent = selectedElement.querySelector('.mermaid')?.textContent || selectedElement.textContent || '';
    message = '已选中图表';
    noteContent = `\`\`\`mermaid\n${mermaidContent}\n\`\`\``;
    noteHtml = selectedElement.outerHTML;
  }
  // 处理图片点击
  else if (target.tagName === 'IMG') {
    selectedElement = target;
    message = '已选中图片';
    noteContent = `![图片](${target.src})`;
    noteHtml = target.outerHTML;
  }
  // 处理文本选择
  else if (selectedText && selection.rangeCount > 0) {
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;
    noteContent = selectedText;
    noteHtml = `<p>${selectedText}</p>`;
  }
  // 处理段落点击
  else {
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      const textContent = paragraph.textContent || '';
      message = `已选中段落: ${textContent.substring(0, 30)}${textContent.length > 30 ? '...' : ''}`;
      noteContent = textContent;
      noteHtml = paragraph.outerHTML;
    }
  }

  // 添加选中效果和笔记
  if (selectedElement) {
    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    // 添加到笔记
    const noteType = determineNoteType(target);

    // 如果Alt键被按下且收集器处于激活状态，则追加到当前卡片
    if (altKeyCollector.value.isActive) {
      appendToCurrentNote({
        type: noteType,
        content: noteContent,
        html: noteHtml
      });

      // 更新卡片标题显示已收集内容数量
      if (altKeyCollector.value.currentCard) {
        const segments = parseCompositeContent(altKeyCollector.value.currentCard.html);
        altKeyCollector.value.currentCard.title = `复合笔记 (${segments.length}项)`;

        // 触发响应式更新
        noteCards.value = [...noteCards.value];
      }
    } else {
      // 否则创建新卡片
      addToNotes({
        type: noteType,
        content: noteContent,
        html: noteHtml
      });
    }
  }

  // 如果松开Alt键且收集器处于激活状态，则立即结束收集
  if (!isAltPressed && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

// 开始Alt键收集模式
const startAltKeyCollection = () => {
  // 清除任何现有的定时器
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  altKeyCollector.value.isActive = true;

  // 创建新卡片并立即添加到笔记列表
  const newCard = {
    id: Date.now().toString(),
    type: 'composite',
    title: '复合笔记',
    content: '',
    html: '',
    timestamp: new Date().toISOString()
  };

  noteCards.value.push(newCard);
  activeCardIndex.value = noteCards.value.length - 1;
  altKeyCollector.value.currentCard = newCard;

  // 设置超时自动结束收集（安全机制）
  altKeyCollector.value.timer = setTimeout(() => {
    if (altKeyCollector.value.isActive) {
      endAltKeyCollection();
    }
  }, 30000); // 30秒后自动结束

  // 滚动到新卡片
  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) {
      cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
    }
  });

  message.info('已进入多选模式，松开Alt键结束收集', { duration: 3000 });
};

// 结束Alt键收集模式
const endAltKeyCollection = () => {
  if (!altKeyCollector.value.isActive) return;

  // 清除定时器
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  // 如果有收集内容，则添加到笔记
  if (altKeyCollector.value.currentCard &&
    (altKeyCollector.value.currentCard.content || altKeyCollector.value.currentCard.html)) {
    // 确保卡片已经存在于noteCards中（因为我们在start时已经添加）
    const existingIndex = noteCards.value.findIndex(card => card.id === altKeyCollector.value.currentCard.id);
    if (existingIndex === -1) {
      noteCards.value.push(altKeyCollector.value.currentCard);
    }
    activeCardIndex.value = noteCards.value.length - 1;
    updateBackendNotes('add', altKeyCollector.value.currentCard);

    nextTick(() => {
      const cards = document.querySelectorAll('.n-card');
      if (cards.length > 0) {
        cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
      }
    });
  }

  // 重置状态
  altKeyCollector.value.isActive = false;
  altKeyCollector.value.currentCard = null;

  message.success('多选模式已结束', { duration: 2000 });
};

// 解析复合笔记内容
const parseCompositeContent = (html) => {
  if (!html) return [];

  try {
    // 创建临时DOM来解析HTML
    const tempDiv = document.createElement('div');
    tempDiv.innerHTML = html;

    // 提取各个片段
    const segments = Array.from(tempDiv.children).map(el => {
      const classList = Array.from(el.classList);
      let type = 'text';

      if (classList.includes('code-segment')) type = 'code';
      else if (classList.includes('image-segment')) type = 'image';
      else if (classList.includes('diagram-segment')) type = 'diagram';
      else if (el.querySelector('mjx-container, mjx-math')) type = 'math';

      return {
        type,
        content: el.outerHTML
      };
    });

    return segments;
  } catch (error) {
    console.error('解析复合笔记失败:', error);
    return [{
      type: 'text',
      content: html
    }];
  }
};

// 获取片段样式类
const getSegmentClass = (segment) => {
  switch (segment.type) {
    case 'code': return 'code-segment';
    case 'image': return 'image-segment';
    case 'diagram': return 'diagram-segment';
    case 'math': return 'math-segment';
    default: return 'text-segment';
  }
};

// 修改appendToCurrentNote方法，确保复合笔记数据正确
const appendToCurrentNote = (element) => {
  if (!altKeyCollector.value.isActive || !altKeyCollector.value.currentCard) return;

  const currentCard = altKeyCollector.value.currentCard;

  // 根据类型处理内容追加
  switch (element.type) {
    case 'text':
    case 'math':
      currentCard.content += `\n\n${element.content}`;
      currentCard.html += `<div class="note-segment ${element.type}-segment">${element.html}</div>`;
      break;
    case 'code':
      currentCard.content += `\n\n${element.content}`;
      currentCard.html += `<div class="note-segment code-segment">${element.html}</div>`;
      break;
    case 'image':
      currentCard.content += `\n\n${element.content}`;
      currentCard.html += `<div class="note-segment image-segment">${element.html}</div>`;
      break;
    case 'diagram':
      currentCard.content += `\n\n${element.content}`;
      currentCard.html += `<div class="note-segment diagram-segment">${element.html}</div>`;
      break;
    default:
      currentCard.content += `\n\n${element.content}`;
      currentCard.html += `<div class="note-segment">${element.html}</div>`;
  }

  // 更新卡片类型为复合类型
  currentCard.type = 'composite';
  currentCard.timestamp = new Date().toISOString();

  // 显示追加成功的消息
  message.success('内容已添加到当前收集', { duration: 1500 });
};

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

// 加载PDF页面
const loadPdfPage = async (pageNum) => {
  try {
    const data = await loadPage(pageNum);
    currentPage.value = data.page_number;
    totalPages.value = data.total_pages;

    console.log('加载成功:', {
      page: currentPage.value,
      total: totalPages.value,
      type: renderContent.value?.type
    });

  } catch (err) {
    console.error('页面加载失败:', err);
    message.error(`加载失败: ${err.message}`);
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
    markdownContent.value = response.content;
    // console.log("markdownContent.value:", markdownContent.value);
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

// 响应式数据
const noteCards = ref([])
const activeCardIndex = ref(-1)
// 添加加载状态
const isNotesReady = ref(false)
const notesFilePath = ref('')

// 确保路径初始化的专用函数
const ensureNotesPath = async () => {
  if (!notesFilePath.value) {
    const notesDir = await getNotesDirectory()
    notesFilePath.value = await join(notesDir, `${props.fileName.replace('.pdf', '')}.html`)
  }
  return notesFilePath.value
}

// 时间格式化
const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString()
}

// 添加新卡片
const addNewCard = () => {
  const newCard = {
    id: Date.now().toString(),
    type: 'text',
    title: '新笔记',
    content: '<p>点击编辑内容...</p>',
    timestamp: new Date().toISOString()
  }

  noteCards.value.push(newCard)
  activeCardIndex.value = noteCards.value.length - 1
  updateBackendNotes('add', newCard)
}

// 处理卡片点击
const handleCardClick = (index) => {
  activeCardIndex.value = index
}

const saveCardEdit = async (index) => {
  if (index >= 0 && index < noteCards.value.length) {
    const cardElement = document.querySelector(
      `.note-card:nth-child(${index + 1}) .note-content`
    );

    if (cardElement) {
      noteCards.value[index].content = cardElement.innerHTML;
      noteCards.value[index].timestamp = new Date().toISOString();

      // 调用更新函数
      await updateBackendNotes('update', noteCards.value[index]);
    }
  }
};

const deleteCard = async (index) => {
  if (index >= 0 && index < noteCards.value.length) {
    const cardId = noteCards.value[index].id;
    noteCards.value.splice(index, 1);

    // 调用更新函数
    await updateBackendNotes('delete', { id: cardId });

    // 调整选中状态
    if (activeCardIndex.value === index) {
      activeCardIndex.value = -1;
    } else if (activeCardIndex.value > index) {
      activeCardIndex.value--;
    }
  }
};

// 从Markdown内容添加卡片
const addToNotes = async (element) => {
  // 如果处于Alt键收集模式，则追加到当前卡片
  if (altKeyCollector.value.isActive) {
    appendToCurrentNote(element)
    return
  }

  const newCard = {
    id: Date.now().toString(),
    type: element.type || 'text',
    title: getCardTitle(element.type),
    content: element.html || element.content,
    timestamp: new Date().toISOString()
  }

  noteCards.value.push(newCard)
  activeCardIndex.value = noteCards.value.length - 1
  await updateBackendNotes('add', newCard)

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card')
    if (cards.length > 0) {
      cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' })
    }
  })
}

// 获取卡片标题
const getCardTitle = (type) => {
  const titles = {
    text: '文本笔记',
    image: '图片笔记',
    code: '代码片段',
    math: '数学公式',
    diagram: '图表'
  }
  return titles[type] || '笔记'
}

// 加载现有笔记
const loadExistingNotes = async () => {
  try {
    const response = await invoke('load_notes_file', {
      filePath: notesFilePath.value
    });

    // 确保返回的是解析后的JSON数据
    const notesData = typeof response === 'string' ? JSON.parse(response) : response;

    if (notesData?.cards?.length) {
      noteCards.value = notesData.cards.map(card => ({
        ...card,
        // 确保旧数据兼容
        id: card.id || Date.now().toString(),
        timestamp: card.timestamp || new Date().toISOString()
      }));
    }
  } catch (error) {
    console.error('加载笔记失败:', error);
    // 如果文件不存在，初始化空数组
    noteCards.value = [];
  }
};

const updateBackendNotes = async (action, cardData) => {
  try {
    // 添加类型检查
    if (!['add', 'update', 'delete'].includes(action)) {
      throw new Error('无效的操作类型');
    }

    console.log('准备发送的数据 notesFilePath:', notesFilePath.value);
    // 准备要发送的数据
    const payload = {
      action,
      data: {
        ...cardData,
        // 确保数据完整
        timestamp: cardData.timestamp || new Date().toISOString()
      },
      filePath: notesFilePath.value
    };
    console.log('发送的数据:', payload);

    await invoke('update_notes_file', payload);

    // 成功后的反馈
    message.success('笔记已保存', {
      duration: 1500
    });
  } catch (error) {
    console.error('更新笔记失败:', error);
    message.error(`保存失败: ${error.message}`, {
      duration: 3000
    });
  }
};

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

const startDrag = (e) => {
  try {
    if (e.button !== 0 || !pdfPanel.value) return;

    // 取消任何进行中的动画
    if (animationFrameId.value) {
      cancelAnimationFrame(animationFrameId.value);
      animationFrameId.value = null;
    }

    isDragging.value = true;
    isDecelerating.value = false;
    startDragPos.value = {
      x: e.clientX,
      y: e.clientY
    };
    startScrollPos.value = {
      left: pdfPanel.value.scrollLeft,
      top: pdfPanel.value.scrollTop
    };
    velocity.value = { x: 0, y: 0 };
    lastDragTime.value = performance.now();

    document.body.style.userSelect = 'none';
    pdfPanel.value.style.cursor = 'grabbing';

    e.preventDefault();
  } catch (error) {
    console.error('Error in startDrag:', error);
    resetDragState();
  }
};

const handleDrag = (e) => {
  try {
    if (!isDragging.value || !pdfPanel.value) return;

    const now = performance.now();
    const deltaTime = now - lastDragTime.value;
    lastDragTime.value = now;

    // 计算移动距离
    const dx = e.clientX - startDragPos.value.x;
    const dy = e.clientY - startDragPos.value.y;

    // 更新滚动位置
    pdfPanel.value.scrollLeft = startScrollPos.value.left - dx;
    pdfPanel.value.scrollTop = startScrollPos.value.top - dy;

    // 计算速度（像素/毫秒）
    if (deltaTime > 0) {
      velocity.value = {
        x: dx / deltaTime,
        y: dy / deltaTime
      };
    }

    e.preventDefault();
  } catch (error) {
    console.error('Error in handleDrag:', error);
    endDrag();
  }
};

const endDrag = () => {
  try {
    if (!isDragging.value) return;

    resetDragState();

    // 如果速度足够大，启动惯性滚动
    const speedThreshold = 0.1;
    if (Math.abs(velocity.value.x) > speedThreshold ||
      Math.abs(velocity.value.y) > speedThreshold) {
      isDecelerating.value = true;
      decelerate();
    }
  } catch (error) {
    console.error('Error in endDrag:', error);
    resetDragState();
  }
};

const resetDragState = () => {
  isDragging.value = false;
  isDecelerating.value = false;
  document.body.style.userSelect = '';
  if (pdfPanel.value) {
    pdfPanel.value.style.cursor = 'grab';
  }
};

// 在onBeforeUnmount中添加清理
onBeforeUnmount(() => {
  resetDragState();
  if (animationFrameId.value) {
    cancelAnimationFrame(animationFrameId.value);
  }
});

// 修改handleWheelZoom方法，支持拖拽状态
const handleWheelZoom = (e) => {
  if (e.ctrlKey) {
    e.preventDefault()
    const delta = e.deltaY < 0 ? 0.1 : -0.1
    zoomLevel.value = Math.max(0.5, Math.min(2, zoomLevel.value + delta))

    // 缩放后确保内容可见
    nextTick(() => {
      if (pdfPanel.value && pdfImage.value) {
        const panel = pdfPanel.value
        const img = pdfImage.value

        // 考虑旋转状态
        const isVertical = Math.abs(rotation.value % 180) === 90
        const effectiveWidth = isVertical ? img.naturalHeight : img.naturalWidth
        const effectiveHeight = isVertical ? img.naturalWidth : img.naturalHeight

        const scaledWidth = effectiveWidth * zoomLevel.value
        const scaledHeight = effectiveHeight * zoomLevel.value

        // 确保内容居中
        panel.scrollLeft = Math.max(0, (scaledWidth - panel.clientWidth) / 2)
        panel.scrollTop = Math.max(0, (scaledHeight - panel.clientHeight) / 2)
      }
    })
  }
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
  showNotes.value = !showNotes.value;
  if (showNotes.value) {
    // 强制重新渲染笔记卡片
    noteCards.value = [...noteCards.value];
  }
};

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
  rotation.value = (rotation.value + degrees) % 360
  // 延迟处理确保DOM更新
  nextTick(() => {
    if (pdfImage.value?.naturalWidth) {
      fitToWidth()
    } else {
      setTimeout(fitToWidth, 300)
    }
  })
}

const fitToWidth = () => {
  if (!pdfPanel.value || !pdfImage.value || isFitting) return
  isFitting = true

  try {
    const panel = pdfPanel.value
    const img = pdfImage.value

    // 确保图像已加载
    if (!img.naturalWidth) {
      console.warn('图像尺寸未加载完成')
      return
    }

    const panelWidth = panel.clientWidth
    const panelHeight = panel.clientHeight
    const imgNaturalWidth = img.naturalWidth
    const imgNaturalHeight = img.naturalHeight

    // 考虑旋转状态
    const isVertical = Math.abs(rotation.value % 180) === 90
    const effectiveWidth = isVertical ? imgNaturalHeight : imgNaturalWidth
    const effectiveHeight = isVertical ? imgNaturalWidth : imgNaturalHeight

    // 计算安全边距（动态适应）
    const horizontalMargin = Math.min(20, panelWidth * 0.05) // 不超过5%宽度
    const verticalMargin = Math.min(20, panelHeight * 0.05) // 不超过5%高度

    // 计算比例时考虑最小缩放限制
    const minScale = 0.5 // 最小缩放比例
    const widthRatio = (panelWidth - horizontalMargin * 2) / effectiveWidth
    const heightRatio = (panelHeight - verticalMargin * 2) / effectiveHeight

    // 动态选择策略：优先宽度适应，但确保内容可见
    let newZoom = Math.max(minScale, Math.min(widthRatio, heightRatio))

    // 防止过度缩小（当容器很大时）
    if (effectiveWidth * newZoom < panelWidth * 0.8 &&
      effectiveHeight * newZoom < panelHeight * 0.8) {
      newZoom = Math.max(widthRatio, heightRatio)
    }

    zoomLevel.value = newZoom

    // 智能居中（使用requestAnimationFrame确保DOM更新）
    requestAnimationFrame(() => {
      const scaledWidth = effectiveWidth * zoomLevel.value
      const scaledHeight = effectiveHeight * zoomLevel.value

      // 重置速度和拖拽状态
      velocity.value = { x: 0, y: 0 }
      isDecelerating.value = false

      // 计算居中位置，考虑边界情况
      panel.scrollLeft = Math.max(0, Math.min(
        (scaledWidth - panelWidth) / 2,
        scaledWidth - panelWidth
      ))
      panel.scrollTop = Math.max(0, Math.min(
        (scaledHeight - panelHeight) / 2,
        scaledHeight - panelHeight
      ))
    })

  } catch (error) {
    console.error('fitToWidth错误:', error)
  } finally {
    isFitting = false
  }
}

// 防抖函数防止频繁触发
const debounce = (fn, delay) => {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), delay);
  };
};

onMounted(async () => {
  await store.get_librariesRoot()
  await updateMarkdownPath()

  // 并行加载但不包括fitToWidth
  await Promise.all([
    (async () => {
      await ensureNotesPath()
      await loadExistingNotes()
    })(),
    loadPdfPage(1),
    loadMarkdownPage(1)
  ])

  // 添加全局键盘事件监听
  const handleKeyUp = (e) => {
    if (e.key === 'Alt' && altKeyCollector.value.isActive) {
      endAltKeyCollection();
    }
  };

  window.addEventListener('keyup', handleKeyUp);

  // 单独处理尺寸适应
  nextTick(() => {
    // 只保留一个触发源
    window.addEventListener('resize', debounce(() => {
      if (isNotesReady.value) fitToWidth()
    }, 300))

    // 确保图像加载完成
    const checkImageLoaded = () => {
      if (pdfImage.value?.complete && pdfImage.value.naturalWidth > 0) {
        fitToWidth()
      } else {
        setTimeout(checkImageLoaded, 100)
      }
    }
    checkImageLoaded()
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize);
  // 清理拖动事件监听
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  window.removeEventListener('keyup', handleKeyUp);
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
  height: 28px;
  padding: 6px 8px;
  /* 原padding 8px 16px，按比例缩小 */
  background-color: #f8f9fa;
  border-bottom: 1px solid #e0e0e0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

button {
  padding: 5px 8px;
  background-color: #ffffff;
  color: #495057;
  border: 1px solid #e9ecef;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  gap: 20px;
  font-size: 12px;
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
  font-size: 12px;
}

.page-info {
  font-size: 12px;
  font-weight: 500;
  color: #495057;
  padding: 5px 8px;
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
  min-width: 20px;
  text-align: center;
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 5px 8px;
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
  padding: 5px 8px;
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
  overscroll-behavior: none;
  /* 防止浏览器默认的滚动行为 */
  scroll-behavior: auto;
  /* 禁用默认的平滑滚动 */
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
  /* 确保transform-origin始终有效 */
  transform-origin: center center !important;
  /* 添加过渡效果 */
  transition: transform 0.3s ease-out;
  will-change: transform;
}

.rendered-pdf-image {
  max-width: none !important;
  max-height: none !important;
  width: auto;
  height: auto;
  object-fit: contain;
  transition: transform 0.2s ease;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  pointer-events: none;
  /* 拖拽时禁用图片事件 */
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

.notes-content {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}

.note-card {
  transition: all 0.2s var(--n-bezier);
  cursor: pointer;
}

.note-card:hover {
  box-shadow: 0 1px 2px -2px rgba(0, 0, 0, 0.08),
    0 3px 6px 0 rgba(0, 0, 0, 0.06),
    0 5px 12px 4px rgba(0, 0, 0, 0.04);
}

.note-content {
  padding: 12px 16px;
  min-height: 60px;
  outline: none;
}

.note-content[contenteditable="true"] {
  background-color: var(--n-color);
  border-radius: var(--n-border-radius);
}

.add-card-container {
  margin-top: 8px;
  margin-bottom: 16px;
}

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

/* 复合笔记样式 */
.composite-note-content {
  padding: 12px;
}

.note-segment {
  margin-bottom: 16px;
  padding-bottom: 16px;
  border-bottom: 1px dashed #e0e0e0;
}

.text-segment {
  font-size: 14px;
  line-height: 1.6;
}

.code-segment {
  background: #f6f8fa;
  padding: 12px;
  border-radius: 6px;
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  overflow-x: auto;
}

.image-segment {
  text-align: center;
}

.image-segment img {
  max-width: 100%;
  border-radius: 4px;
}

.diagram-segment {
  background: white;
  padding: 16px;
  border-radius: 8px;
  overflow-x: auto;
}

.math-segment {
  padding: 8px;
  overflow-x: auto;
}

/* 确保内容可编辑时的样式 */
.note-content[contenteditable="true"],
.composite-note-content[contenteditable="true"] {
  outline: 1px dashed #4dabf7;
  min-height: 60px;
  padding: 8px;
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