<template>
  <div class="text-viewer-container">
    <!-- 顶部控制栏 -->
    <div class="text-markdown-controls">
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

        <!-- 导出笔记按钮移除，改用布局下拉菜单 -->
        <n-dropdown trigger="click" :options="viewModeOptions" @select="handleViewModeSelect">
          <n-button class="layout-switch" text title="切换布局">
            <template #icon>
              <n-icon :component="OptionsOutline" />
            </template>
          </n-button>
        </n-dropdown>

        <!-- 显示/隐藏笔记按钮 -->
        <button class="toggle-notes" @click="toggleNotesPanel" :title="showNotes ? '隐藏笔记' : '显示笔记'">
          <n-icon :component="DocumentTextOutline" />
          <span class="btn-text">{{ showNotes ? '隐藏笔记' : '显示笔记' }}</span>
        </button>

        <button class="toggle-translate" @click="toggleTranslate" :title="showTranslation ? '隐藏译文' : '显示译文'"
          :class="{ 'translating': isTranslating }">
          <n-icon :component="LanguageOutline" />
          <span class="btn-text">
            {{ showTranslation ? '隐藏译文' : '显示译文' }}
            <span v-if="isTranslating" class="translating-indicator">...</span>
          </span>
        </button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="text-main-content">
      <!-- 左侧text文档预览区域 -->
      <div class="text-source-panel" v-show="viewMode !== 'full-right'" :style="{ width: leftPanelWidth }">
        <div class="text-source-container">
          <div ref="textPreviewContainer" class="text-preview-container"></div>
          <!-- 加载状态 -->
          <div v-if="isLoading" class="text-loading-message">
            正在加载文本文件...
          </div>
        </div>
      </div>

      <!-- 可拖动的中间线 -->
      <div class="resize-handle" v-show="viewMode === 'split'" @mousedown="startResize"></div>

      <!-- 右侧Markdown内容 -->
      <div class="markdown-panel" v-show="viewMode !== 'full-left'" @click="handleMarkdownClick"
        @mousemove="handleMarkdownHover" @mouseleave="handleMarkdownLeave">
        <div class="markdown-content" ref="markdownContainer">
          <template v-for="(line, index) in markdownLines" :key="index">
            <div class="markdown-line">
              <div class="original-content" v-html="line.html"></div>
              <div v-if="showTranslation" class="translated-content" :class="{
                'skipped-translation': line.skipTranslation,
                'error-translation': line.translationError
              }">
                <template v-if="line.skipTranslation">
                  <div v-html="line.html"></div>
                  <span class="translation-skipped-marker">(内容未翻译)</span>
                </template>
                <template v-if="line.translationError">
                  <div class="translation-error-message">
                    {{ line.translationError }}
                  </div>
                  <div v-html="line.html" class="original-fallback"></div>
                </template>
                <template v-else>
                  <div v-html="line.translation"></div>
                </template>
              </div>
            </div>
          </template>
        </div>
        <div v-if="showOutline" class="outline-panel-wrapper" @click.stop>
          <GuaxiOutline />
        </div>
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
          }" header-style="padding: 12px 16px;" content-style="padding: 0;">
          <template #header-extra>
            <div class="card-actions" v-if="activeCardIndex === index">
              <n-button v-if="card.type !== 'composite'" size="tiny" type="primary" @click.stop="saveCardEdit(index)">
                <n-icon :component="SaveOutline" size="12" />
              </n-button>
              <n-button v-if="card.type !== 'composite'" size="tiny" text @click.stop="exitEditMode(index)">
                <n-icon :component="ExitOutline" size="12" />
              </n-button>
              <n-button size="tiny" text @click.stop="deleteCard(index)">
                <n-icon :component="TrashOutline" size="12" />
              </n-button>
            </div>
          </template>

          <!-- 根据笔记类型使用不同的渲染方式 -->
          <div v-if="card.type !== 'composite' && activeCardIndex !== index" class="note-content"
            :contenteditable="false" v-html="noteHtmls[index]" @click="enterEditMode(index)"></div>

          <div v-else-if="card.type !== 'composite' && activeCardIndex === index" class="note-content"
            contenteditable="true" @keydown.meta.s.prevent="saveCardEdit(index)"
            @keydown.ctrl.s.prevent="saveCardEdit(index)" :ref="el => setContentEditableRef(index, el)"></div>

          <!-- 添加复合笔记的专用渲染 -->
          <div v-else class="composite-note-content">
            <div v-for="(segment, segIndex) in compositeSegments[index]" :key="segIndex" class="note-segment"
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
import { ref, watchEffect, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { join, dirname } from '@tauri-apps/api/path'
import { convertFileSrc } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';
import { storeToRefs } from 'pinia'
import { repoStore } from '../../stores/repoStore'
import { renderMarkdown, addCopyButtons } from '../GuaxiMarkdownRender/GuaxiMarker'
import { NCard, NButton, NIcon, NText, useMessage, NDropdown } from 'naive-ui'
import MarkdownIt from 'markdown-it'
import { useOutlineStore } from '../../stores/outlineStore'
import GuaxiOutline from '../HomeViewComponent/GuaxiOutline.vue'
import {
  ArrowBackOutline,
  ArrowForwardOutline,
  RefreshCircleOutline,
  ExpandOutline,
  TrashOutline,
  AddOutline,
  RemoveOutline,
  DocumentTextOutline,
  DownloadOutline,
  CloseOutline,
  LanguageOutline,
  SaveOutline,
  ExitOutline,
  OptionsOutline
} from '@vicons/ionicons5'

// 组件属性定义
const props = defineProps({
  filePath: {
    type: String,
    required: true,
  },
  fileType: {
    type: String,
    required: true,
    validator: (value) => ['', 'txt'].includes(value)
  },
  fileName: {
    type: String,
    required: true,
  },
  library: {
    type: String,
    required: true
  },
  importFileId: {
    type: Number,
    default: null,
  }
})

// 初始化状态和工具
const message = useMessage()
const store = repoStore()
const { librariesRoot } = storeToRefs(store)
const outlineStore = useOutlineStore()
const md = new MarkdownIt()
const markdownContainer = ref(null);
const noteHtmls = ref([]);
const compositeSegments = ref([]);

// 视图状态
const zoomLevel = ref(1);
const markdownPanel = ref(null);
const isResizing = ref(false);
const startX = ref(0);
const startLeftWidth = ref(0);
const leftPanelWidth = ref('50%');
const currentPage = ref(1)
const totalPages = ref(0)
const jumpPage = ref(1)
const elements = ref([])
const viewMode = ref('split')
const previousLeftWidth = ref(leftPanelWidth.value)
const showOutline = ref(false)

const viewModeOptions = [
  { label: '左右分栏', key: 'split' },
  { label: '仅左侧', key: 'full-left' },
  { label: '仅右侧', key: 'full-right' },
  { label: '大纲', key: 'outline' }
]

const handleViewModeSelect = (key) => {
  if (key === 'outline') {
    showOutline.value = !showOutline.value
  } else {
    setViewMode(key)
  }
}


const textPreviewContainer = ref(null)
const isLoading = ref(false)

// Markdown相关
const markdownContent = ref('')
// const markdownHtml = ref('')
const markdownLines = ref([]);
const markdownFilePath = ref('')

// 笔记相关
const showNotes = ref(false)
const notesContent = ref('')
const showNoteBubble = ref(false)
const isFitting = false
const noteCards = ref([])
const activeCardIndex = ref(-1)
const isNotesReady = ref(false)
const notesFilePath = ref('')

// 拖拽和动画相关
const isDragging = ref(false)
const startDragPos = ref({ x: 0, y: 0 })
const startScrollPos = ref({ left: 0, top: 0 })
const velocity = ref({ x: 0, y: 0 })
const lastDragTime = ref(0)
const animationFrameId = ref(null)
const isDecelerating = ref(false)

// 翻译相关
const showTranslation = ref(false)
const isTranslating = ref(false)

// Alt键收集器状态
const altKeyCollector = ref({
  isActive: false,
  currentCard: null,
  timer: null
})

// 添加行号功能
const addLineNumbers = () => {
  if (!textPreviewContainer.value) return;

  const preElement = textPreviewContainer.value.querySelector('pre');
  if (!preElement) return;

  const lineCount = preElement.innerHTML.split('<br>').length;
  let lineNumbersHtml = '<div class="line-numbers">';

  for (let i = 1; i <= lineCount; i++) {
    lineNumbersHtml += `<span class="line-number">${i}</span><br>`;
  }

  lineNumbersHtml += '</div>';

  // 创建行号容器
  const lineNumbersContainer = document.createElement('div');
  lineNumbersContainer.className = 'line-numbers-container';
  lineNumbersContainer.innerHTML = lineNumbersHtml;
  lineNumbersContainer.style.cssText = `
    position: absolute;
    left: 0;
    top: 0;
    padding: 20px 10px 20px 20px;
    text-align: right;
    color: #999;
    background: #f0f0f0;
    border-right: 1px solid #ddd;
    user-select: none;
  `;

  // 更新pre元素的样式以留出行号空间
  preElement.style.paddingLeft = '60px';
  preElement.style.position = 'relative';

  // 将行号容器添加到pre元素中
  preElement.prepend(lineNumbersContainer);
};

// 高亮特殊行（如TODO、NOTE等）
const highlightSpecialLines = () => {
  if (!textPreviewContainer.value) return;

  const preElement = textPreviewContainer.value.querySelector('pre');
  if (!preElement) return;

  const lines = preElement.innerHTML.split('<br>');
  let highlightedHtml = '';

  lines.forEach(line => {
    let processedLine = line;

    // 高亮TODO注释
    if (line.includes('TODO:')) {
      processedLine = `<span style="color: #e74c3c; font-weight: bold;">${line}</span>`;
    }
    // 高亮NOTE注释
    else if (line.includes('NOTE:')) {
      processedLine = `<span style="color: #3498db; font-weight: bold;">${line}</span>`;
    }
    // 高亮FIXME注释
    else if (line.includes('FIXME:')) {
      processedLine = `<span style="color: #f39c12; font-weight: bold;">${line}</span>`;
    }

    highlightedHtml += processedLine + '<br>';
  });

  preElement.innerHTML = highlightedHtml;
};

// 计算总页数（基于滚动高度）
const calculateTotalPages = async () => {
  await nextTick();

  if (!textPreviewContainer.value) {
    totalPages.value = 1;
    return;
  }

  const container = textPreviewContainer.value;
  const contentHeight = container.scrollHeight;
  const viewportHeight = container.clientHeight;

  totalPages.value = Math.max(1, Math.ceil(contentHeight / viewportHeight));
};

const loadWithTextPreview = async () => {
  try {
    isLoading.value = true;

    // 读取文本文件内容
    const textContent = await invoke('read_text_file', {
      path: props.filePath
    });

    // 创建预处理后的HTML内容
    const processedContent = textContent
      // 替换特殊字符
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      // 将换行符转换为<br>标签
      .replace(/\n/g, '<br>')
      // 将连续空格转换为&nbsp;
      .replace(/ {2}/g, ' &nbsp;');

    // 创建完整的HTML结构
    const htmlContent = `
      <div class="text-preview-content">
        <pre style="
          font-family: 'JetBrains Mono', 'Fira Code', Consolas, monospace;
          font-size: 16px;
          line-height: 1.6;
          white-space: pre-wrap;
          word-wrap: break-word;
          padding: 0px;
        ">${processedContent}</pre>
      </div>
    `;

    // 设置容器内容
    if (textPreviewContainer.value) {
      textPreviewContainer.value.innerHTML = htmlContent;

      // 添加行号功能
      addLineNumbers();

      // 添加语法高亮（简单实现）
      highlightSpecialLines();
    }

    // 计算总页数（基于滚动高度）
    await calculateTotalPages();

  } catch (error) {
    console.error('加载text文件失败:', error);
    if (textPreviewContainer.value) {
      textPreviewContainer.value.innerHTML = `
        <div class="error-message" style="
          padding: 20px;
          color: #ff4d4f;
          text-align: center;
        ">
          加载text文件失败: ${error.message || '未知错误'}
        </div>
      `;
    }
    message.error(`加载text文件失败: ${error.message || '未知错误'}`);
  } finally {
    isLoading.value = false;
  }
};

// 修改 loadTextDocument 函数
const loadTextDocument = async () => {
  isLoading.value = true;

  try {
    console.log('开始加载text文档:', props.filePath);
    await loadWithTextPreview();
  } catch (error) {
    console.error('加载text文件失败:', error);
    message.error(`加载text文件失败: ${error.message}`);
  } finally {
    isLoading.value = false;
  }
};

const applyZoomLevel = () => {
  if (!textPreviewContainer.value) return;

  const content = textPreviewContainer.value.querySelector('.text-preview-content');
  if (!content) return;

  content.style.fontSize = `${14 * zoomLevel.value}px`;
  content.style.lineHeight = `${1.6 * zoomLevel.value}`;

  // 重新计算总页数
  calculateTotalPages();
};

// 工具函数
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

// 渲染笔记内容为HTML
const renderNoteContent = async (markdown) => {
  if (!markdown) return '';
  try {
    const imageDir = await getImagesDirectory();
    const processedContent = await processImages(markdown, imageDir);
    return await renderMarkdown(processedContent, {
      preserveOriginal: false,
      basePath: "",
      mermaid: true,
      math: true,
      codeHighlight: true,
      theme: 'default'
    });
  } catch (e) {
    return `<div class="render-error">${e.message}</div>`;
  }
};

function generateOutlineFromMarkdown(content) {
  if (!content) {
    outlineStore.setOutline([]);
    return;
  }
  const tokens = md.parse(content, {});
  const result = [];
  tokens.forEach((token, idx) => {
    if (token.type === 'heading_open') {
      const level = Number(token.tag.slice(1));
      const nextToken = tokens[idx + 1];
      if (nextToken && nextToken.type === 'inline') {
        result.push({
          level,
          text: nextToken.content,
          line: result.length
        });
      }
    }
  });
  outlineStore.setOutline(result);
}

function jumpToHeading(index) {
  if (!markdownContainer.value) return;
  const headings = markdownContainer.value.querySelectorAll('h1, h2, h3, h4, h5, h6');
  const target = headings[index];
  if (target) {
    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}

async function convertMarkdownToLines(content, options = {}) {
  const processedContent = await processImages(content, options.libraryBasePath);
  try {
    let html = await renderMarkdown(processedContent, {
      preserveOriginal: true,
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
            html: text,
            skipTranslation: false
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
          'mjx-container', 'mjx-math', 'mjx-assistive-mml',
          'div.hljs', 'code.hljs',
          '[class^="hljs-"]', '[class*=" hljs-"]'];

        if (keepWholeTags.some(tag => node.matches(tag))) {
          const outerHTML = node.outerHTML;
          const skipTranslation = shouldSkipTranslation(node);

          result.push({
            raw: outerHTML,
            html: outerHTML,
            skipTranslation
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
      html: `<div class="render-error">${error.message}</div>`,
      skipTranslation: true
    }];
  }
}

function shouldSkipTranslation(node) {
  if (!node || typeof node.matches !== 'function') {
    return false;
  }

  try {
    // 1. 图片元素
    if (node.tagName === 'IMG') {
      return true;
    }

    // 2. 数学公式相关元素
    if (node.classList && (
      node.classList.contains('katex-display') ||
      node.classList.contains('katex-inline') ||
      node.tagName === 'MJX-CONTAINER' ||
      node.tagName === 'MJX-MATH'
    )) {
      return true;
    }

    // 3. 图表相关元素
    if (node.classList && (
      node.classList.contains('mermaid') ||
      (node.tagName === 'SVG' && node.classList.contains('mermaid'))
    )) {
      return true;
    }

    // 4. 代码块
    if (['PRE', 'CODE'].includes(node.tagName) ||
      (node.classList && (
        node.classList.contains('hljs') ||
        Array.from(node.classList).some(cls => cls.startsWith('hljs-'))
      ))) {
      return true;
    }

    // 5. 表格
    if (node.tagName === 'TABLE') {
      return true;
    }

    return false;
  } catch (error) {
    console.error('Error checking node:', error);
    return false;
  }
}

function determineNoteType(target) {
  // 1. 优先检查数学公式
  if (target.closest('mjx-container, mjx-math, .math-inline, .math-block')) {
    return 'math';
  }

  // 2. 检查Mermaid图表
  const mermaidContainer = target.closest('.mermaid, [data-processed="true"]');
  if (mermaidContainer) {
    if (mermaidContainer.querySelector('svg') || mermaidContainer.classList.contains('mermaid')) {
      return 'diagram';
    }
  }

  // 3. 检查代码块
  const pre = target.closest('pre');
  if (pre) {
    if (!pre.classList.contains('mermaid')) {
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

async function processNoteImage(imgElement) {
  try {
    const rawSrc = imgElement.src;
    let filePath = '';

    if (rawSrc.startsWith('http://asset.localhost/')) {
      filePath = decodeURIComponent(rawSrc.replace('http://asset.localhost/', ''));
    } else if (rawSrc.startsWith('file://')) {
      filePath = decodeURIComponent(new URL(rawSrc).pathname);
    } else {
      return rawSrc;
    }

    const osPlatform = await platform();
    if (osPlatform === 'win32') {
      filePath = filePath.replace(/^\/([a-zA-Z]:)/, '$1');
    }

    const imageDir = await getImagesDirectory();
    const ext = filePath.split('.').pop().toLowerCase();
    const validExts = ['jpg', 'jpeg', 'png', 'gif', 'webp'];
    const finalExt = validExts.includes(ext) ? ext : 'png';
    const fileName = `note-img-${Date.now()}.${finalExt}`;

    const relativePath = await invoke('copy_image_file', {
      srcPath: filePath,
      destDir: imageDir,
      fileName: fileName
    });

    return `../images/${relativePath}`;
  } catch (error) {
    console.error('图片处理失败:', error);
    return imgElement.src;
  }
}

const handleMarkdownClick = async (e) => {
  e.preventDefault();

  const isAltPressed = e.altKey;
  const target = e.target;
  const selection = window.getSelection();
  const selectedText = selection.toString().trim();

  if (isAltPressed && !altKeyCollector.value.isActive) {
    startAltKeyCollection();
  }

  document.querySelectorAll('.markdown-content .selected').forEach(el => {
    el.classList.remove('selected');
  });

  let selectedElement = null;
  let message = '';
  let noteContent = '';
  let originalMarkdown = '';

  const translatedElement = target.closest('.translated-content');
  if (translatedElement) {
    const contentDiv = translatedElement.querySelector('div');
    if (contentDiv) {
      selectedElement = translatedElement;
      noteContent = contentDiv.innerHTML;
      message = '已选中译文';
      noteContent = contentDiv.textContent;
    }
  }
  else if (target.tagName === 'IMG') {
    selectedElement = target;
    try {
      const processedSrc = await processNoteImage(target);
      message = '已选中图片';
      noteContent = `![图片](${processedSrc})`;
      originalMarkdown = noteContent;
    } catch (error) {
      console.error('图片处理失败:', error);
      message = '图片处理失败，使用原始链接';
      noteContent = `![图片](${target.src})`;
      originalMarkdown = noteContent;
    }
  }
  else if (target.closest('pre, code, .code-block')) {
    const codeBlock = target.closest('pre, code, .code-block');
    selectedElement = codeBlock;
    const clonedBlock = codeBlock.cloneNode(true);
    const copyButton = clonedBlock.querySelector('.copy-button');
    if (copyButton) {
      copyButton.remove();
    }
    const codeContent = clonedBlock.textContent || '';
    message = `已选中代码块: ${codeContent.substring(0, 30)}${codeContent.length > 30 ? '...' : ''}`;
    originalMarkdown = codeBlock.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`\n${codeContent}\n\`\`\``;
    originalMarkdown = decodeURIComponent(originalMarkdown);
    originalMarkdown = originalMarkdown.replace(/\s*复制\s*$/, '');
    noteContent = originalMarkdown;
  }
  else if (target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered')) {
    const mathElement = target.closest('mjx-container, mjx-math, .math-inline, .math-block, svg.math-rendered');
    selectedElement = mathElement;
    let original = mathElement.getAttribute('data-md-original');
    if (!original && mathElement.closest('[data-md-original]')) {
      original = mathElement.closest('[data-md-original]').getAttribute('data-md-original');
    }
    if (!original && mathElement.querySelector('[data-md-original]')) {
      original = mathElement.querySelector('[data-md-original]').getAttribute('data-md-original');
    }
    let decodedOriginal = original;
    try {
      if (decodedOriginal) {
        decodedOriginal = decodeURIComponent(decodedOriginal);
      } else {
        const isBlock = mathElement.classList.contains('math-block') ||
          mathElement.closest('.math-block') ||
          mathElement.tagName === 'MJX-BLOCK';
        const content = mathElement.textContent || '';
        decodedOriginal = isBlock ? `$$${content}$$` : `$${content}$`;
      }
    } catch (err) {
      decodedOriginal = original || '无法获取公式内容';
    }
    noteContent = decodedOriginal;
    originalMarkdown = decodedOriginal;
    const isBlockFormula = decodedOriginal.startsWith('$$') &&
      decodedOriginal.endsWith('$$') &&
      decodedOriginal.length > 4;
    message = `已选中${isBlockFormula ? '块级' : '行内'}公式: ${decodedOriginal.substring(0, 30)}${decodedOriginal.length > 30 ? '...' : ''}`;
  }
  else if (target.closest('.mermaid, svg')) {
    selectedElement = target.closest('.mermaid, svg');
    const mermaidContent = selectedElement.querySelector('.mermaid')?.textContent || selectedElement.textContent || '';
    message = '已选中图表';
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      `\`\`\`mermaid\n${mermaidContent}\n\`\`\``;
    originalMarkdown = decodeURIComponent(originalMarkdown);
    noteContent = originalMarkdown;
  }
  else if (selectedText && selection.rangeCount > 0) {
    selectedElement = selection.getRangeAt(0).startContainer.parentElement;
    message = `已选中文本: ${selectedText.substring(0, 30)}${selectedText.length > 30 ? '...' : ''}`;
    originalMarkdown = selectedElement.closest('[data-md-original]')?.getAttribute('data-md-original') ||
      selectedText;
    originalMarkdown = decodeURIComponent(originalMarkdown);
    noteContent = originalMarkdown;
  }
  else {
    const paragraph = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote');
    if (paragraph) {
      selectedElement = paragraph;
      const textContent = paragraph.textContent || '';
      message = `已选中段落: ${textContent.substring(0, 30)}${textContent.length > 30 ? '...' : ''}`;
      originalMarkdown = paragraph.getAttribute('data-md-original') ||
        textContent;
      originalMarkdown = decodeURIComponent(originalMarkdown);
      noteContent = originalMarkdown;
    }
  }

  if (selectedElement) {
    selectedElement.classList.add('selected');
    showSelectionMessage(message);
    selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    const noteType = determineNoteType(target);
    const noteData = {
      type: noteType,
      content: noteContent,
      originalMarkdown: originalMarkdown
    };

    if (altKeyCollector.value.isActive) {
      appendToCurrentNote(noteData);
    } else {
      await addToNotes(noteData);
    }
  }

  if (!isAltPressed && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

const startAltKeyCollection = () => {
  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  altKeyCollector.value.isActive = true;

  const newCard = {
    id: Date.now().toString(),
    type: 'composite',
    title: '复合笔记 (0项)',
    content: '',
    items: [],
    timestamp: new Date().toISOString()
  };

  noteCards.value.push(newCard);
  activeCardIndex.value = noteCards.value.length - 1;
  altKeyCollector.value.currentCard = newCard;

  altKeyCollector.value.timer = setTimeout(() => {
    if (altKeyCollector.value.isActive) endAltKeyCollection();
  }, 30000);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
  });

  message.info('已进入多选模式, 松开Alt键结束收集', { duration: 3000 });
};

const endAltKeyCollection = () => {
  if (!altKeyCollector.value.isActive) return;

  if (altKeyCollector.value.timer) {
    clearTimeout(altKeyCollector.value.timer);
    altKeyCollector.value.timer = null;
  }

  if (altKeyCollector.value.currentCard &&
    (altKeyCollector.value.currentCard.content || altKeyCollector.value.currentCard.originalMarkdown)) {
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

  altKeyCollector.value.isActive = false;
  altKeyCollector.value.currentCard = null;

  message.success('多选模式已结束', { duration: 2000 });
};

async function parseCompositeContent(items) {
  if (!items || !Array.isArray(items)) {
    return [];
  }

  try {
    const segments = await Promise.all(
      items.map(async (item, index) => {
        const content = item.original_markdown || item.content || '';
        const html = await renderNoteContent(content);

        return {
          id: item.id || `${Date.now()}-${index}`,
          type: item.type || 'text',
          content: html,
          originalMarkdown: content,
          timestamp: item.timestamp || new Date().toISOString()
        };
      })
    );

    return segments;
  } catch (error) {
    return [{
      type: 'error',
      content: `<div class="parse-error">解析失败: ${error.message}</div>`,
      originalMarkdown: '解析失败',
      timestamp: new Date().toISOString()
    }];
  }
}

const getSegmentClass = (segment) => {
  switch (segment.type) {
    case 'code': return 'code-segment';
    case 'image': return 'image-segment';
    case 'diagram': return 'diagram-segment';
    case 'math': return 'math-segment';
    default: return 'text-segment';
  }
};

const appendToCurrentNote = (element) => {
  if (!altKeyCollector.value.isActive || !altKeyCollector.value.currentCard) return;

  const currentCard = altKeyCollector.value.currentCard;

  if (!currentCard.items) {
    currentCard.items = [];
  }

  const newItem = {
    id: Date.now().toString(),
    type: element.type,
    content: element.content,
    originalMarkdown: element.originalMarkdown,
    timestamp: new Date().toISOString()
  };

  currentCard.items.push(newItem);
  currentCard.content = currentCard.items.map((item, index) => {
    return `${index + 1}. ${item.originalMarkdown || item.content}`;
  }).join('\n\n');

  currentCard.type = 'composite';
  currentCard.title = `复合笔记 (${currentCard.items.length}项)`;
  currentCard.timestamp = new Date().toISOString();

  noteCards.value = [...noteCards.value];

  message.success(`已添加第 ${currentCard.items.length} 条内容`, { duration: 1500 });
};

const showSelectionMessage = (message) => {
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
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });

  const element = e.target.closest(
    'p, h1, h2, h3, h4, h5, h6, li, blockquote, ' +
    'pre, code, .code-block, ' +
    '.katex-display, .katex-inline, ' +
    '.mermaid, svg'
  );

  if (element) {
    element.classList.add('hovered');
  }
}

const handleMarkdownLeave = () => {
  document.querySelectorAll('.markdown-content .hovered').forEach(el => {
    el.classList.remove('hovered');
  });
}

const updateMarkdownPath = async () => {
  const baseFileName = props.fileName.replace('.txt', '')
  markdownFilePath.value = await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'markdown',
    `${baseFileName}.md`
  )
}

const getNotesDirectory = async () => {
  const baseFileName = props.fileName.replace('.txt', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'notes'
  );
};

const getImagesDirectory = async () => {
  const baseFileName = props.fileName.replace('.md', '');
  return await join(
    librariesRoot.value,
    'Libraries',
    props.library,
    baseFileName,
    'images'
  );
};

const loadMarkdownPage = async () => {
  try {
    if (!markdownFilePath.value) {
      await updateMarkdownPath();
    }

    console.log('Loading markdown from:', markdownFilePath.value);
    const response = await invoke('markdown_reader_load_markdown_file', {
      filepath: markdownFilePath.value,
    });

    if (!response.success) {
      throw new Error(response.error || 'Failed to load markdown');
    }

    const mdDir = await dirname(markdownFilePath.value);
    markdownContent.value = response.content;
    markdownLines.value = await convertMarkdownToLines(markdownContent.value,
      { libraryBasePath: mdDir }
    );
    generateOutlineFromMarkdown(markdownContent.value);

  } catch (error) {
    console.error('加载Markdown失败:', error);
    markdownLines.value = [{
      raw: '',
      html: `<div class="error">加载错误: ${error.message}</div>`
    }];
    outlineStore.setOutline([]);
  }
};

const ensureNotesPath = async () => {
  if (!notesFilePath.value) {
    const notesDir = await getNotesDirectory()
    notesFilePath.value = await join(notesDir, `${props.fileName.replace('.md', '')}.json`)
  }
  console.log('Notes file path:', notesFilePath.value);
  return notesFilePath.value
}

const formatTime = (timestamp) => {
  return new Date(timestamp).toLocaleString()
}

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

const deleteCard = async (index) => {
  if (index >= 0 && index < noteCards.value.length) {
    const cardId = noteCards.value[index].id;
    noteCards.value.splice(index, 1);

    await updateBackendNotes('delete', { id: cardId });

    if (activeCardIndex.value === index) {
      activeCardIndex.value = -1;
    } else if (activeCardIndex.value > index) {
      activeCardIndex.value--;
    }
  }
};

const addToNotes = async (element) => {
  if (altKeyCollector.value.isActive) {
    appendToCurrentNote(element);
    return;
  }

  const newCard = {
    id: Date.now().toString(),
    type: 'single',
    title: getCardTitle(element.type),
    timestamp: new Date().toISOString(),
    items: [{
      id: `${Date.now()}-0`,
      type: element.type,
      content: element.content,
      originalMarkdown: element.originalMarkdown,
      timestamp: new Date().toISOString()
    }]
  };

  noteCards.value.push(newCard);
  activeCardIndex.value = noteCards.value.length - 1;
  await updateBackendNotes('add', newCard);

  nextTick(() => {
    const cards = document.querySelectorAll('.n-card');
    if (cards.length > 0) {
      cards[cards.length - 1].scrollIntoView({ behavior: 'smooth' });
    }
  });
};

const getCardTitle = (type) => {
  const titles = {
    text: '文本摘录',
    image: '图片摘录',
    code: '代码摘录',
    math: '公式摘录',
    diagram: '图表摘录'
  }
  return titles[type] || '笔记'
}

const loadExistingNotes = async () => {
  try {
    const response = await invoke('load_notes_file', {
      filePath: notesFilePath.value
    });

    const notesData = typeof response === 'string' ? JSON.parse(response) : response;

    if (notesData?.cards?.length) {
      noteCards.value = notesData.cards.map(card => {
        const items = card.items || [{
          id: card.id || Date.now().toString(),
          type: card.type === 'composite' ? 'text' : card.type,
          content: card.content,
          originalMarkdown: card.originalMarkdown || card.content,
          timestamp: card.timestamp || new Date().toISOString()
        }];

        return {
          ...card,
          id: card.id || Date.now().toString(),
          type: items.length > 1 ? 'composite' : 'single',
          items,
          timestamp: card.timestamp || new Date().toISOString()
        };
      });
    }
  } catch (error) {
    console.error('加载笔记失败:', error);
    noteCards.value = [];
  }
};

const updateBackendNotes = async (action, cardData) => {
  try {
    if (!cardData?.id) {
      throw new Error('无效的笔记ID');
    }

    const items = Array.isArray(cardData.items) ? cardData.items : [];

    const payload = {
      action,
      data: {
        id: cardData.id,
        type: cardData.type || 'single',
        title: cardData.title || '',
        items: items.map(item => ({
          id: item.id || `${cardData.id}-${Date.now()}`,
          type: item.type || 'text',
          content: item.content || '',
          original_markdown: item.originalMarkdown || item.content || '',
          timestamp: item.timestamp || new Date().toISOString()
        })),
        timestamp: cardData.timestamp || new Date().toISOString()
      },
      filePath: await ensureNotesPath(),
      importFileId: Number(props.importFileId) || 0,
      importFileName: props.fileName || '',
    };

    await invoke('update_notes_file', payload);
    message.success('笔记已保存', { duration: 1500 });

  } catch (error) {
    const errorMessage = typeof error === 'object'
      ? error.message || error.toString()
      : String(error);

    console.error('更新笔记失败:', {
      error: errorMessage,
      cardData: JSON.parse(JSON.stringify(cardData || {}))
    });

    const normalizedError = errorMessage.toLowerCase();
    if (normalizedError.includes('未找到') || normalizedError.includes('not found')) {
      await handleMissingNote(action, cardData);
    } else {
      message.error(`保存失败: ${errorMessage}`, { duration: 3000 });
    }
  }
};

const handleMissingNote = async (action, cardData) => {
  try {
    await loadExistingNotes();
    const exists = noteCards.value.some(card => card.id === cardData?.id);

    if (!exists) {
      const completeCard = {
        ...cardData,
        items: Array.isArray(cardData.items) ? cardData.items : [],
        timestamp: cardData.timestamp || new Date().toISOString()
      };

      await updateBackendNotes('add', completeCard);
    }
  } catch (error) {
    console.error('处理缺失笔记失败:', error);
    message.error('自动恢复失败，请手动添加笔记');
  }
};

const exportNotes = () => {
  if (!notesContent.value) {
    alert('笔记内容为空');
    return;
  }

  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = notesContent.value;

  let exportContent = '';
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
  a.download = `Markdown笔记_${props.fileName.replace('.md', '')}_${new Date().toISOString().slice(0, 10)}.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

const startDrag = (e) => {
  try {
    if (e.button !== 0 || !markdownPanel.value) return;

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
      left: markdownPanel.value.scrollLeft,
      top: markdownPanel.value.scrollTop
    };
    velocity.value = { x: 0, y: 0 };
    lastDragTime.value = performance.now();

    document.body.style.userSelect = 'none';
    markdownPanel.value.style.cursor = 'grabbing';

    e.preventDefault();
  } catch (error) {
    console.error('Error in startDrag:', error);
    resetDragState();
  }
};

const handleDrag = (e) => {
  try {
    if (!isDragging.value || !markdownPanel.value) return;

    const now = performance.now();
    const deltaTime = now - lastDragTime.value;
    lastDragTime.value = now;

    const dx = e.clientX - startDragPos.value.x;
    const dy = e.clientY - startDragPos.value.y;

    markdownPanel.value.scrollLeft = startScrollPos.value.left - dx;
    markdownPanel.value.scrollTop = startScrollPos.value.top - dy;

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
  if (markdownPanel.value) {
    markdownPanel.value.style.cursor = 'grab';
  }
};

const decelerate = () => {
  if (!isDecelerating.value || !markdownPanel.value) return;

  const friction = 0.95;
  const minSpeed = 0.5;

  velocity.value.x *= friction;
  velocity.value.y *= friction;

  markdownPanel.value.scrollLeft += velocity.value.x * 16;
  markdownPanel.value.scrollTop += velocity.value.y * 16;

  if (Math.abs(velocity.value.x) > minSpeed || Math.abs(velocity.value.y) > minSpeed) {
    animationFrameId.value = requestAnimationFrame(decelerate);
  } else {
    isDecelerating.value = false;
    velocity.value = { x: 0, y: 0 };
  }
};

const handleWheelZoom = (e) => {
  if (e.ctrlKey) {
    e.preventDefault()
    const delta = e.deltaY < 0 ? 0.1 : -0.1
    zoomLevel.value = Math.max(0.5, Math.min(2, zoomLevel.value + delta))

    nextTick(() => {
      if (markdownPanel.value && markdownContent.value) {
        const panel = markdownPanel.value
        const content = markdownContent.value

        panel.scrollLeft = Math.max(0, (content.clientWidth * zoomLevel.value - panel.clientWidth) / 2)
        panel.scrollTop = Math.max(0, (content.clientHeight * zoomLevel.value - panel.clientHeight) / 2)
      }
    })
  }
}

// 上一页
const prevPage = () => {
  if (currentPage.value > 1) {
    goToPage(currentPage.value - 1);
  }
};

// 下一页
const nextPage = () => {
  if (currentPage.value < totalPages.value) {
    goToPage(currentPage.value + 1);
  }
};

// 跳转到指定页
const goToPage = (page) => {
  const pageNum = Math.max(1, Math.min(page, totalPages.value));
  currentPage.value = pageNum;

  if (!textPreviewContainer.value) return;

  const container = textPreviewContainer.value;
  const viewportHeight = container.clientHeight;
  const scrollTop = (pageNum - 1) * viewportHeight;

  container.scrollTo({
    top: scrollTop,
    behavior: 'smooth'
  });
};

const toggleNotesPanel = () => {
  showNotes.value = !showNotes.value;
  if (showNotes.value) {
    noteCards.value = [...noteCards.value];
  }
};

const zoomIn = () => {
  zoomLevel.value = Math.min(zoomLevel.value + 0.1, 2)
  applyZoomLevel()
}

const zoomOut = () => {
  zoomLevel.value = Math.max(zoomLevel.value - 0.1, 0.5)
  applyZoomLevel()
}

const startResize = (e) => {
  try {
    console.log('startResize triggered');
    isResizing.value = true;
    startX.value = e.clientX;

    // 修改选择器为正确的面板类名
    const leftPanel = document.querySelector('.text-source-panel');
    if (!leftPanel) {
      throw new Error('左侧面板元素未找到');
    }

    startLeftWidth.value = parseInt(window.getComputedStyle(leftPanel).width, 10);

    // 添加事件监听
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', stopResize);

    // 更新光标样式
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    e.preventDefault();
  } catch (error) {
    console.error('调整大小错误:', error);
    isResizing.value = false;
    message.error('调整大小失败: ' + error.message);
  }
};

const handleMouseMove = (e) => {
  if (!isResizing.value) return;

  try {
    const container = document.querySelector('.text-main-content');
    if (!container) return;

    const containerWidth = container.offsetWidth;
    const dx = e.clientX - startX.value;
    let newLeftWidth = ((startLeftWidth.value + dx) * 100) / containerWidth;

    newLeftWidth = Math.max(30, Math.min(70, newLeftWidth));

    leftPanelWidth.value = `${newLeftWidth}%`;
    previousLeftWidth.value = leftPanelWidth.value;
    e.preventDefault();
  } catch (error) {
    console.error('调整大小移动错误:', error);
    stopResize();
  }
};

const stopResize = () => {
  try {
    isResizing.value = false;
    document.removeEventListener('mousemove', handleMouseMove);
    document.removeEventListener('mouseup', stopResize);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  } catch (error) {
    console.error('停止调整大小错误:', error);
  }
};

// 适合宽度
const fitToWidth = () => {
  if (!textPreviewContainer.value) return;

  const container = textPreviewContainer.value;
  const content = container.querySelector('pre');
  if (!content) return;

  const containerWidth = container.clientWidth;
  const contentWidth = content.scrollWidth;

  zoomLevel.value = Math.min(1, containerWidth / (contentWidth + 60));

  applyZoomLevel();
};

const setViewMode = (mode) => {
  if (mode === viewMode.value) return;

  if (mode === 'split') {
    leftPanelWidth.value = previousLeftWidth.value || '50%';
  } else if (mode === 'full-left') {
    previousLeftWidth.value = leftPanelWidth.value;
    leftPanelWidth.value = '100%';
  } else if (mode === 'full-right') {
    if (viewMode.value === 'full-left') {
      leftPanelWidth.value = previousLeftWidth.value || '50%';
    }
  }

  viewMode.value = mode;
};

const debounce = (fn, delay) => {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn.apply(this, args), delay);
  };
};

const translateQueue = ref([]);
const lastTranslateTime = ref(0);
const translateBatchSize = 5;
const translateInterval = 200;

const translateMarkdown = async () => {
  if (isTranslating.value) return;

  const linesToTranslate = markdownLines.value
    .filter(line => !line.skipTranslation && !line.translation)
    .map(line => {
      const tempDiv = document.createElement('div');
      tempDiv.innerHTML = line.html;
      const textNodes = [];
      const extractText = (node) => {
        if (shouldSkipTranslation(node)) return;

        if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
          textNodes.push(node.textContent.trim());
        } else if (node.childNodes) {
          for (const child of node.childNodes) {
            extractText(child);
          }
        }
      };
      extractText(tempDiv);

      return {
        line,
        text: textNodes.join(' '),
        html: line.html
      };
    })
    .filter(item => item.text.length > 0);

  translateQueue.value.push(...linesToTranslate);
  processTranslateQueue();
};

const processTranslateQueue = async () => {
  if (isTranslating.value || translateQueue.value.length === 0) return;

  const now = Date.now();
  if (now - lastTranslateTime.value < translateInterval) {
    setTimeout(processTranslateQueue, translateInterval - (now - lastTranslateTime.value));
    return;
  }

  isTranslating.value = true;
  try {
    const currentBatch = translateQueue.value.splice(0, translateBatchSize);
    const texts = currentBatch.map(item => item.text);

    const response = await invoke('translate_text_free', {
      texts: texts,
    });

    if (response && response.translated_texts?.length === texts.length) {
      currentBatch.forEach((item, index) => {
        const translatedText = response.translated_texts[index];
        if (translatedText) {
          item.line.translation = wrapTranslation(item.line.html, translatedText);
          item.line.translationError = null;
        } else {
          item.line.translationError = '翻译结果为空';
        }
      });
    } else {
      const errorMsg = response?.error ||
        (response?.translated_texts ? '返回结果数量不匹配' : '无效的响应格式');
      currentBatch.forEach(item => {
        item.line.translationError = `翻译失败: ${errorMsg}`;
      });
      console.error('翻译错误:', response);
    }

    lastTranslateTime.value = Date.now();
  } catch (error) {
    console.error('翻译异常:', error);
    translateQueue.value.forEach(item => {
      item.line.translationError = `翻译异常: ${error.message || '未知错误'}`;
    });
  } finally {
    isTranslating.value = false;
    if (translateQueue.value.length > 0) {
      setTimeout(processTranslateQueue, translateInterval);
    }
  }
};

const wrapTranslation = (html, translatedText) => {
  if (!translatedText || translatedText.trim() === '') {
    return html;
  }

  const tempDiv = document.createElement('div');
  tempDiv.innerHTML = html;

  const textNodes = [];
  const findTextNodes = (node) => {
    if (shouldSkipTranslation(node)) return;

    if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
      textNodes.push(node);
    } else if (node.childNodes) {
      for (const child of node.childNodes) {
        findTextNodes(child);
      }
    }
  };
  findTextNodes(tempDiv);

  let textPos = 0;
  for (const node of textNodes) {
    const originalText = node.textContent.trim();
    if (textPos < translatedText.length) {
      const endPos = Math.min(textPos + originalText.length, translatedText.length);
      node.textContent = translatedText.substring(textPos, endPos);
      textPos = endPos + 1;
    }
  }

  return tempDiv.innerHTML;
};

const toggleTranslate = async () => {
  showTranslation.value = !showTranslation.value;

  if (showTranslation.value) {
    markdownLines.value = markdownLines.value.map(line => {
      if (!line.translation) {
        line.skipTranslation = shouldSkipTranslation(line.raw, line.html);
        if (line.skipTranslation) {
          line.translation = line.html;
        }
      }
      return line;
    });

    await translateMarkdown();
  } else {
    // 新增：隐藏译文时重置翻译状态
    isTranslating.value = false;
    translateQueue.value = [];
    lastTranslateTime.value = 0;

    // 移除所有行的翻译错误状态
    markdownLines.value = markdownLines.value.map(line => {
      line.translationError = null;
      return line;
    });
  }
};
const contentEditables = ref([]);

const setContentEditableRef = (index, el) => {
  if (el) {
    if (!contentEditables.value[index]) {
      contentEditables.value = [...contentEditables.value];
    }
    contentEditables.value[index] = el;
  }
};

const saveCardEdit = async (index) => {
  try {
    if (index < 0 || index >= noteCards.value.length) {
      throw new Error('无效的卡片索引');
    }

    const editableDiv = contentEditables.value[index];
    if (!editableDiv) {
      throw new Error('找不到可编辑元素');
    }

    const currentCard = JSON.parse(JSON.stringify(noteCards.value[index]));

    if (currentCard.type === 'composite') {
      currentCard.content = editableDiv.innerHTML;
    } else {
      if (!Array.isArray(currentCard.items)) {
        currentCard.items = [];
      }

      if (currentCard.items[0]) {
        currentCard.items[0].content = editableDiv.innerHTML;
      } else {
        currentCard.items[0] = {
          id: `${currentCard.id}-0`,
          type: 'text',
          content: editableDiv.innerHTML,
          original_markdown: '',
          timestamp: new Date().toISOString()
        };
      }
    }

    currentCard.timestamp = new Date().toISOString();
    await updateBackendNotes('update', currentCard);

  } catch (error) {
    console.error('保存编辑失败:', {
      error,
      index,
      card: noteCards.value[index]
    });
    message.error(`保存失败: ${error.message}`);
  }
};

const exitEditMode = (index) => {
  const editableDiv = contentEditables.value[index];
  if (editableDiv && editableDiv.innerHTML !== noteCards.value[index].content) {
    if (!confirm('有未保存的修改，确定要取消吗？')) return;
  }
  activeCardIndex.value = -1;
};

const enterEditMode = async (index) => {
  activeCardIndex.value = index;
  await nextTick();
  const editableDiv = contentEditables.value[index];
  if (editableDiv) {
    if (!editableDiv.innerHTML || editableDiv.innerHTML.trim() === '') {
      editableDiv.innerHTML = noteHtmls.value[index] || '';
    }
    editableDiv.focus();
    const range = document.createRange();
    range.selectNodeContents(editableDiv);
    range.collapse(false);
    const selection = window.getSelection();
    if (selection) {
      selection.removeAllRanges();
      selection.addRange(range);
    }
  }
};

const debouncedSave = debounce(saveCardEdit, 500);

const handleKeyUp = (e) => {
  if (e.key === 'Alt' && altKeyCollector.value.isActive) {
    endAltKeyCollection();
  }
};

onMounted(async () => {
  await store.get_librariesRoot()
  await updateMarkdownPath()
  outlineStore.setJumpToLine(jumpToHeading)

  await Promise.all([
    (async () => {
      await ensureNotesPath()
      await loadExistingNotes()
    })(),
    loadTextDocument(),
    loadMarkdownPage()
  ])

  window.addEventListener('keyup', handleKeyUp);

  nextTick(() => {
    window.addEventListener('resize', debounce(() => {
      if (isNotesReady.value) fitToWidth()
    }, 300))

    const checkContentLoaded = () => {
      if (markdownContent.value?.clientWidth > 0) {
        fitToWidth()
      } else {
        setTimeout(checkContentLoaded, 100)
      }
    }
    checkContentLoaded()
  })

  const autoSaveInterval = setInterval(() => {
    if (activeCardIndex.value !== -1) {
      saveCardEdit(activeCardIndex.value);
    }
  }, 30000);

  onBeforeUnmount(() => clearInterval(autoSaveInterval));
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize);
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', stopResize);
  window.removeEventListener('keyup', handleKeyUp);
  outlineStore.setOutline([]);
});

const handleResize = () => {
  if (markdownPanel.value && markdownContent.value) {
    fitToWidth();
  }
};

watch(() => props.filePath, async (newPath) => {
  if (newPath) {
    await updateMarkdownPath();
    currentPage.value = 1;
    loadMarkdownPage(1);
    notesContent.value = '';
  }
});

watchEffect(async () => {
  if (props.fileType === 'txt' || props.fileType === '') {
    loadTextDocument()
  }

  noteHtmls.value = await Promise.all(
    noteCards.value.map(async card => {
      if (card.type === 'single') {
        const item = card.items?.[0] || {};
        return await renderNoteContent(item.originalMarkdown || item.content || '');
      }
      return "";
    })
  );

  compositeSegments.value = await Promise.all(
    noteCards.value.map(card =>
      card.type === 'composite'
        ? parseCompositeContent(card.items)
        : []
    )
  );

  nextTick(() => {
    document.querySelectorAll('.composite-note-content .mermaid').forEach(el => {
      try {
        if (!el.querySelector('svg')) {
          mermaid.run({ nodes: [el] });
        }
      } catch (e) {
      }
    });
  });
});
</script>

<style scoped>
.text-main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  position: relative;
}

/* 左侧text文档区域 */
.text-source-panel {
  height: 100%;
  overflow: auto;
  position: relative;
  background-color: white;
  min-width: 30%;
}

.text-preview-container {
  width: 100%;
  height: 100%;
  overflow: auto;
  padding: 20px;
  background: white;
}

.text-preview-container :deep(.docx) {
  width: 100% !important;
  height: auto !important;
  padding: 0;
  margin: 0;
  box-shadow: none;
}

.text-preview-container :deep(.docx > *) {
  max-width: 100% !important;
}

.text-preview-container :deep(page) {
  margin-bottom: 20px;
  break-after: page;
  overflow: visible !important;
  /* 强制取消内部滚动 */
}

.text-preview-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace;
  font-size: 14px;
  line-height: 1.6;
  color: var(--n-text-color);
  background-color: var(--n-color);
  padding: 16px;
  white-space: pre-wrap;
  word-wrap: break-word;
  border-radius: 4px;
}

.text-preview-content pre {
  margin: 0;
  padding: 0;
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
  color: inherit;
  background-color: transparent;
  white-space: pre-wrap;
  word-wrap: break-word;
}

.error-message {
  color: #ff4d4f;
  padding: 20px;
  text-align: center;
}

/* 右侧渲染面板样式调整 */
.markdown-render-panel {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 20px;
  background-color: var(--guaxi-content-bg);
}

.markdown-content {
  font-family: 'Maple Mono NF CN', 'JetBrainsMono Nerd Font Mono', Consolas, monospace, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  font-size: 18px;
  line-height: 1.6;
  color: var(--guaxi-text-primary);
  background-color: var(--guaxi-content-bg);
  user-select: text;
}

.text-viewer-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--guaxi-content-bg);
  overflow: hidden;
}

.text-markdown-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 28px;
  padding: 6px 8px;
  background-color: var(--guaxi-panel-bg);
  border-bottom: 1px solid var(--guaxi-border);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

button {
  padding: 5px 8px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
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
  background-color: var(--guaxi-header-bg);
  border-color: var(--guaxi-border);
  transform: translateY(-1px);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

button:disabled {
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-secondary);
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}

.page-info {
  font-size: 12px;
  font-weight: 500;
  color: var(--guaxi-text-primary);
  padding: 5px 8px;
  background-color: var(--guaxi-panel-bg);
  border-radius: 6px;
  border: 1px solid var(--guaxi-border);
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
  border: 1px solid var(--guaxi-border);
  border-radius: 6px;
  background-color: var(--guaxi-panel-bg);
  color: var(--guaxi-text-primary);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.right-controls {
  display: flex;
  gap: 10px;
}

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
  background-color: var(--guaxi-header-bg);
  color: var(--guaxi-text-primary);
  border: 1px solid var(--guaxi-border);
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

.fit-width {
  background-color: #f8f9fa;
  color: #495057;
  border: 1px solid #e9ecef;
  padding: 5px 8px;
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

.markdown-render-panel {
  flex: 0 0 v-bind(leftPanelWidth);
  min-width: 0;
  overflow: auto;
  position: relative;
  background-color: #ffffff;
  overscroll-behavior: none;
  scroll-behavior: auto;
}

.markdown-content-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: center;
  align-items: center;
  background-color: #f8f9fa;
  transform-origin: center center !important;
  transition: transform 0.3s ease-out;
  will-change: transform;
}

.rendered-markdown {
  width: 100%;
  height: 100%;
  padding: 20px;
  overflow: auto;
}

.text-loading-message {
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
  display: flex;
}

.outline-panel-wrapper {
  width: 260px;
  border-left: 1px solid #e0e0e0;
  box-sizing: border-box;
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

.note-content {
  padding: 12px 16px;
  min-height: 60px;
  outline: none;
}

.note-content.saved-feedback {
  animation: savedPulse 1s;
}

@keyframes savedPulse {
  0% {
    box-shadow: 0 0 0 0 rgba(77, 171, 247, 0.4);
  }

  70% {
    box-shadow: 0 0 0 10px rgba(77, 171, 247, 0);
  }

  100% {
    box-shadow: 0 0 0 0 rgba(77, 171, 247, 0);
  }
}

.note-content[contenteditable="true"] {
  padding: 12px;
  border-radius: 8px;
  background-color: rgba(3, 101, 167, 0.05);
}

.composite-note-content[contenteditable="true"] {
  min-height: 100px;
  padding: 12px;
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

.card-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.n-button--tiny .n-icon {
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 按钮悬停效果 */
.n-button--tiny:hover {
  transform: scale(1.1);
  transition: transform 0.2s ease;
}

/* 删除按钮特殊样式 */
.n-button--tiny.text-type[data-v-xxxxxx]:hover {
  color: #ff6b6b;
  background-color: rgba(255, 107, 107, 0.1);
}


/* 68. 翻译按钮样式 */
.toggle-translate {
  background-color: #7950f2;
  color: white;
  border: 1px solid #6741d9;
  transition: all 0.2s;
}

.toggle-translate:hover {
  background-color: #6741d9;
  box-shadow: 0 2px 8px rgba(121, 80, 242, 0.3);
}

/* 69. Markdown行样式 */
.markdown-line {
  margin-bottom: 1em;
}

/* 70. 原文和译文样式 */
.original-content {
  padding: 8px;
  border-radius: 4px;
  background-color: rgba(255, 255, 255, 0.8);
}

.translated-content {
  padding: 8px;
  margin-top: 4px;
  border-radius: 4px;
  background-color: rgba(245, 245, 245, 0.8);
  border-left: 3px solid #7950f2;
  color: #555;
}

.error-translation {
  border-left: 3px solid #ff6b6b;
  background-color: rgba(255, 107, 107, 0.05);
}

.translation-error-message {
  color: #ff6b6b;
  font-size: 0.9em;
  margin-bottom: 8px;
  font-weight: bold;
}

.original-fallback {
  opacity: 0.7;
  border-top: 1px dashed #ccc;
  padding-top: 8px;
  margin-top: 8px;
}

.translation-error {
  color: #ff6b6b;
  font-size: 0.9em;
  font-style: italic;
}

.translation-error-marker {
  display: inline-block;
  font-size: 0.8em;
  background-color: #ffebee;
  color: #ff6b6b;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
}

.translating-indicator {
  display: inline-block;
  width: 1em;
  text-align: left;
}

.translating {
  position: relative;
  opacity: 0.7;
}

.translating::after {
  content: "";
  position: absolute;
  bottom: -2px;
  left: 0;
  right: 0;
  height: 2px;
  background-color: #4dabf7;
  animation: translating-progress 2s infinite;
}

@keyframes translating-progress {
  0% {
    transform: translateX(-100%);
  }

  100% {
    transform: translateX(100%);
  }
}

.skipped-translation {
  opacity: 0.7;
  border-left: 3px solid #ccc;
  padding-left: 8px;
  margin-left: 4px;
  font-style: italic;
  color: #666;
}

.translation-skipped-marker {
  display: inline-block;
  font-size: 0.8em;
  background-color: #f0f0f0;
  color: #999;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 8px;
}
</style>