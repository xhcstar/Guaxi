"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
const core_1 = require("@tauri-apps/api/core");
const path_1 = require("@tauri-apps/api/path");
const core_2 = require("@tauri-apps/api/core");
const pinia_1 = require("pinia");
const repoStore_1 = require("../../stores/repoStore");
const GuaxiMarker_1 = require("../GuaxiMarkdownRender/GuaxiMarker");
const ionicons5_1 = require("@vicons/ionicons5");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
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
});
const store = (0, repoStore_1.repoStore)();
const { librariesRoot } = (0, pinia_1.storeToRefs)(store);
const markdownContainer = (0, vue_1.ref)(null);
// 缩放状态
const zoomLevel = (0, vue_1.ref)(1);
// 添加旋转状态
const rotation = (0, vue_1.ref)(0);
const pdfPanel = (0, vue_1.ref)(null);
const pdfImage = (0, vue_1.ref)(null);
// 添加拖动状态
const isResizing = (0, vue_1.ref)(false);
const startX = (0, vue_1.ref)(0);
const startLeftWidth = (0, vue_1.ref)(0);
const leftPanelWidth = (0, vue_1.ref)('50%'); // 初始宽度
// PDF数据状态
const currentPage = (0, vue_1.ref)(1);
const totalPages = (0, vue_1.ref)(0);
const jumpPage = (0, vue_1.ref)(1);
const elements = (0, vue_1.ref)([]);
// Markdown数据状态
const markdownContent = (0, vue_1.ref)('');
// const markdownLines = ref([])
const markdownLines = (0, vue_1.ref)([]);
const markdownFilePath = (0, vue_1.ref)('');
// 笔记状态
const showNotes = (0, vue_1.ref)(false);
const notesContent = (0, vue_1.ref)('');
// UI状态
const showNoteBubble = (0, vue_1.ref)(false);
function processImages(content_1) {
    return __awaiter(this, arguments, void 0, function* (content, basePath = '') {
        const imgRegex = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"([^"]+)")?\)/g;
        const replacements = [];
        for (const match of content.matchAll(imgRegex)) {
            const [full, alt, src] = match;
            if (!src.match(/^(http|\/|data:image)/i)) {
                try {
                    const fullPath = yield (0, path_1.join)(basePath, src);
                    const safeSrc = yield (0, core_2.convertFileSrc)(fullPath);
                    replacements.push({ from: full, to: `![${alt}](${safeSrc})` });
                }
                catch (error) {
                    console.warn(`路径转换失败: ${src}`, error);
                }
            }
        }
        return replacements.reduce((result, { from, to }) => result.replace(from, to), content);
    });
}
function convertPdfRenderMarkdown(content_1) {
    return __awaiter(this, arguments, void 0, function* (content, options = {}) {
        const processedContent = yield processImages(content, options.libraryBasePath);
        try {
            let html = yield (0, GuaxiMarker_1.renderMarkdown)(processedContent, {
                basePath: "",
                mermaid: true,
                math: true,
                codeHighlight: true,
                theme: 'default'
            });
            // 确保DOM更新后添加按钮
            (0, vue_1.nextTick)(() => {
                if (markdownContainer.value) {
                    (0, GuaxiMarker_1.addCopyButtons)(markdownContainer.value);
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
        }
        catch (error) {
            console.error('[ERROR] 渲染失败:', error);
            return [{
                    raw: content,
                    html: `<div class="render-error">${error.message}</div>`
                }];
        }
    });
}
const renderedMarkdown = (0, vue_1.computed)(() => {
    return markdownLines.value.map(line => line.html).join('');
});
const handleMarkdownClick = (e) => {
    var _a;
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
        const mermaidContent = ((_a = selectedElement.querySelector('.mermaid')) === null || _a === void 0 ? void 0 : _a.textContent) || selectedElement.textContent || '';
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
        let noteType = 'text';
        if (target.tagName === 'IMG')
            noteType = 'image';
        else if (codeBlock)
            noteType = 'code';
        else if (target.closest('mjx-container, mjx-math'))
            noteType = 'math';
        else if (target.closest('.mermaid, svg'))
            noteType = 'diagram';
        addToNotes({
            type: noteType,
            content: noteContent,
            html: noteHtml // 添加HTML格式内容
        });
    }
};
const showSelectionMessage = (message) => {
    // 移除旧的消息
    const oldMessage = document.querySelector('.selection-message');
    if (oldMessage)
        oldMessage.remove();
    const messageEl = document.createElement('div');
    messageEl.className = 'selection-message';
    messageEl.textContent = message;
    document.body.appendChild(messageEl);
    setTimeout(() => {
        messageEl.classList.add('fade-out');
        setTimeout(() => messageEl.remove(), 500);
    }, 1500);
};
const handleMarkdownHover = (e) => {
    const target = e.target;
    // 清除之前的悬停效果
    document.querySelectorAll('.markdown-content .hovered').forEach(el => {
        el.classList.remove('hovered');
    });
    // 获取最近的段落元素或特殊元素
    const element = target.closest('p, h1, h2, h3, h4, h5, h6, li, blockquote, ' +
        'pre, code, .code-block, ' + // 代码块
        '.katex-display, .katex-inline, ' + // 数学公式
        '.mermaid, svg' // mermaid图表和生成的SVG元素
    );
    if (element) {
        element.classList.add('hovered');
    }
};
const handleMarkdownLeave = () => {
    // 清除所有悬停效果
    document.querySelectorAll('.markdown-content .hovered').forEach(el => {
        el.classList.remove('hovered');
    });
};
// 在onMounted或watch中更新路径
const updateMarkdownPath = () => __awaiter(void 0, void 0, void 0, function* () {
    const baseFileName = props.fileName.replace('.pdf', '');
    markdownFilePath.value = yield (0, path_1.join)(librariesRoot.value, 'Libraries', props.library, baseFileName, 'markdown', baseFileName, `${baseFileName}.md`);
});
const showNoteBubbleMessage = () => {
    showNoteBubble.value = true;
    setTimeout(() => {
        showNoteBubble.value = false;
    }, 2000);
};
// 加载PDF页面
const loadPdfPage = (pageNum) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        const response = yield (0, core_1.invoke)('get_converted_pdf_page', {
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
    }
    catch (error) {
        console.error('PDF加载失败:', error);
        elements.value = [];
        if (error instanceof Error) {
            alert(`PDF加载错误: ${error.message}`);
        }
    }
});
// 加载Markdown页面
const loadMarkdownPage = (pageNum) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        if (!markdownFilePath.value) {
            yield updateMarkdownPath();
        }
        const response = yield (0, core_1.invoke)('load_markdown_content', {
            filepath: markdownFilePath.value,
            page: pageNum - 1
        });
        if (!response.success) {
            throw new Error(response.error || 'Failed to load markdown');
        }
        // 获取markdown文件所在目录
        const mdDir = yield (0, path_1.dirname)(markdownFilePath.value);
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
        markdownLines.value = yield convertPdfRenderMarkdown(markdownContent.value, { libraryBasePath: mdDir });
        console.log("markdownLines.value:", markdownLines.value);
    }
    catch (error) {
        console.error('加载Markdown失败:', error);
        markdownLines.value = [{
                raw: '',
                html: `<div class="error">加载错误: ${error.message}</div>`
            }];
    }
});
// 添加内容到笔记
const addToNotes = (element) => {
    const timestamp = new Date().toLocaleString();
    let note = '';
    // 优先使用HTML内容，如果没有则使用Markdown内容
    const content = element.html || element.content;
    switch (element.type) {
        case 'text':
            note = `## 文本摘录 (${timestamp})\n${content}\n\n`;
            break;
        case 'image':
            note = `## 图片摘录 (${timestamp})\n${content}\n\n`;
            break;
        case 'code':
            note = `## 代码摘录 (${timestamp})\n${content}\n\n`;
            break;
        case 'math':
            note = `## 公式摘录 (${timestamp})\n${content}\n\n`;
            break;
        case 'diagram':
            note = `## 图表摘录 (${timestamp})\n${content}\n\n`;
            break;
    }
    notesContent.value += note;
    // showNotes.value = true;
    showNoteBubbleMessage();
};
// 导出笔记
const exportNotes = () => {
    if (!notesContent.value) {
        alert('笔记内容为空');
        return;
    }
    const blob = new Blob([notesContent.value], { type: 'text/markdown' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `PDF笔记_${props.filePath.split('/').pop().split('.')[0]}.md`;
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
    showNotes.value = !showNotes.value;
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
    if (!isResizing.value)
        return;
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
    if (!pdfPanel.value || !pdfImage.value)
        return;
    const panelWidth = pdfPanel.value.clientWidth;
    const imgWidth = pdfImage.value.naturalWidth;
    // 考虑旋转状态
    const isVertical = Math.abs(rotation.value % 180) === 90;
    const effectiveWidth = isVertical ? pdfImage.value.naturalHeight : imgWidth;
    // 计算适合宽度，留出10px边距
    zoomLevel.value = (panelWidth - 10) / effectiveWidth;
    // 确保图片居中
    (0, vue_1.nextTick)(() => {
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
(0, vue_1.onMounted)(() => {
    store.get_librariesRoot().then(() => __awaiter(void 0, void 0, void 0, function* () {
        yield updateMarkdownPath();
        loadPdfPage(1);
        loadMarkdownPage(1);
        // 添加窗口大小变化监听
        window.addEventListener('resize', handleResize);
        // 添加延迟确保DOM完全渲染
        setTimeout(() => {
            fitToWidth(); // 初始加载时自动适合宽度
        }, 100);
    }));
});
(0, vue_1.onBeforeUnmount)(() => {
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
(0, vue_1.watch)(() => props.filePath, (newPath) => __awaiter(void 0, void 0, void 0, function* () {
    if (newPath) {
        yield updateMarkdownPath();
        currentPage.value = 1;
        loadPdfPage(1);
        loadMarkdownPage(1);
        notesContent.value = '';
    }
}));
; /* PartiallyEnd: #3632/scriptSetup.vue */
const __VLS_fnComponent = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    props: {
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
    },
});
;
let __VLS_functionalComponentProps;
function __VLS_template() {
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign(Object.assign({}, {}), {}), __VLS_ctx);
    let __VLS_components;
    const __VLS_localDirectives = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_directives;
    let __VLS_styleScopedClasses;
    __VLS_styleScopedClasses['page-navigation'];
    __VLS_styleScopedClasses['zoom-in'];
    __VLS_styleScopedClasses['zoom-out'];
    __VLS_styleScopedClasses['zoom-in'];
    __VLS_styleScopedClasses['zoom-out'];
    __VLS_styleScopedClasses['rotate-left'];
    __VLS_styleScopedClasses['rotate-right'];
    __VLS_styleScopedClasses['fit-width'];
    __VLS_styleScopedClasses['toggle-notes'];
    __VLS_styleScopedClasses['export-notes'];
    __VLS_styleScopedClasses['toggle-notes'];
    __VLS_styleScopedClasses['export-notes'];
    __VLS_styleScopedClasses['resize-handle'];
    __VLS_styleScopedClasses['resize-handle'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-image'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['selected'];
    __VLS_styleScopedClasses['selection-message'];
    __VLS_styleScopedClasses['notes-header'];
    __VLS_styleScopedClasses['close-btn'];
    // CSS variable injection 
    __VLS_ctx.leftPanelWidth;
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-viewer-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-controls") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("page-info") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)({});
    (__VLS_ctx.currentPage);
    (__VLS_ctx.totalPages);
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("page-navigation") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.prevPage) }, { disabled: ((__VLS_ctx.currentPage <= 1)), title: ("上一页") }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ component: ((__VLS_ctx.ArrowBackOutline)), }));
    const __VLS_2 = __VLS_1({ component: ((__VLS_ctx.ArrowBackOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.input, __VLS_intrinsicElements.input)(Object.assign({ onKeyup: (__VLS_ctx.goToPage) }, { type: ("number"), min: ("1"), max: ((__VLS_ctx.totalPages)) }));
    (__VLS_ctx.jumpPage);
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.nextPage) }, { disabled: ((__VLS_ctx.currentPage >= __VLS_ctx.totalPages)), title: ("下一页") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6({ component: ((__VLS_ctx.ArrowForwardOutline)), }));
    const __VLS_8 = __VLS_7({ component: ((__VLS_ctx.ArrowForwardOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_7));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("right-controls") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (...[$event]) => {
            __VLS_ctx.rotate(-90);
        } }, { class: ("rotate-left") }), { title: ("向左旋转90度") }));
    const __VLS_12 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_13 = __VLS_asFunctionalComponent(__VLS_12, new __VLS_12({ component: ((__VLS_ctx.RefreshCircleOutline)), rotate: ((90)), }));
    const __VLS_14 = __VLS_13({ component: ((__VLS_ctx.RefreshCircleOutline)), rotate: ((90)), }, ...__VLS_functionalComponentArgsRest(__VLS_13));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (...[$event]) => {
            __VLS_ctx.rotate(90);
        } }, { class: ("rotate-right") }), { title: ("向右旋转90度") }));
    const __VLS_18 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_19 = __VLS_asFunctionalComponent(__VLS_18, new __VLS_18({ component: ((__VLS_ctx.RefreshCircleOutline)), rotate: ((-90)), }));
    const __VLS_20 = __VLS_19({ component: ((__VLS_ctx.RefreshCircleOutline)), rotate: ((-90)), }, ...__VLS_functionalComponentArgsRest(__VLS_19));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (__VLS_ctx.fitToWidth) }, { class: ("fit-width") }), { title: ("将页面调整为适合宽度") }));
    const __VLS_24 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_25 = __VLS_asFunctionalComponent(__VLS_24, new __VLS_24({ component: ((__VLS_ctx.ExpandOutline)), }));
    const __VLS_26 = __VLS_25({ component: ((__VLS_ctx.ExpandOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_25));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (__VLS_ctx.zoomOut) }, { class: ("zoom-out") }), { disabled: ((__VLS_ctx.zoomLevel <= 0.5)), title: ("缩小") }));
    const __VLS_30 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_31 = __VLS_asFunctionalComponent(__VLS_30, new __VLS_30({ component: ((__VLS_ctx.RemoveOutline)), }));
    const __VLS_32 = __VLS_31({ component: ((__VLS_ctx.RemoveOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_31));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("zoom-level") }));
    (Math.round(__VLS_ctx.zoomLevel * 100));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (__VLS_ctx.zoomIn) }, { class: ("zoom-in") }), { disabled: ((__VLS_ctx.zoomLevel >= 2)), title: ("放大") }));
    const __VLS_36 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_37 = __VLS_asFunctionalComponent(__VLS_36, new __VLS_36({ component: ((__VLS_ctx.AddOutline)), }));
    const __VLS_38 = __VLS_37({ component: ((__VLS_ctx.AddOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_37));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (__VLS_ctx.toggleNotesPanel) }, { class: ("toggle-notes") }), { title: ((__VLS_ctx.showNotes ? '隐藏笔记' : '显示笔记')) }));
    const __VLS_42 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_43 = __VLS_asFunctionalComponent(__VLS_42, new __VLS_42({ component: ((__VLS_ctx.DocumentTextOutline)), }));
    const __VLS_44 = __VLS_43({ component: ((__VLS_ctx.DocumentTextOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_43));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    (__VLS_ctx.showNotes ? '隐藏笔记' : '显示笔记');
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign(Object.assign({ onClick: (__VLS_ctx.exportNotes) }, { class: ("export-notes") }), { title: ("导出笔记") }));
    const __VLS_48 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_49 = __VLS_asFunctionalComponent(__VLS_48, new __VLS_48({ component: ((__VLS_ctx.DownloadOutline)), }));
    const __VLS_50 = __VLS_49({ component: ((__VLS_ctx.DownloadOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_49));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("btn-text") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-main-content") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign({ onWheel: (__VLS_ctx.handleWheelZoom) }, { class: ("pdf-render-panel") }), { ref: ("pdfPanel") }));
    // @ts-ignore navigation for `const pdfPanel = ref()`
    __VLS_ctx.pdfPanel;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-image-container") }, { style: (({
            transform: `scale(${__VLS_ctx.zoomLevel}) rotate(${__VLS_ctx.rotation}deg)`,
            transformOrigin: 'center center'
        })) }));
    if (__VLS_ctx.elements.length > 0 && __VLS_ctx.elements[0].type === 'image') {
        __VLS_elementAsFunction(__VLS_intrinsicElements.img)(Object.assign(Object.assign({ src: ((__VLS_ctx.elements[0].content)), alt: ("Rendered PDF Page") }, { class: ("rendered-pdf-image") }), { ref: ("pdfImage") }));
        // @ts-ignore navigation for `const pdfImage = ref()`
        __VLS_ctx.pdfImage;
    }
    else {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-loading-message") }));
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onMousedown: (__VLS_ctx.startResize) }, { class: ("resize-handle") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign(Object.assign({ onClick: (__VLS_ctx.handleMarkdownClick) }, { onMousemove: (__VLS_ctx.handleMarkdownHover) }), { onMouseleave: (__VLS_ctx.handleMarkdownLeave) }), { class: ("markdown-panel") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("markdown-content") }, { ref: ("markdownContainer") }));
    __VLS_asFunctionalDirective(__VLS_directives.vHtml)(null, Object.assign(Object.assign({}, __VLS_directiveBindingRestFields), { modifiers: {}, value: (__VLS_ctx.renderedMarkdown) }), null, null);
    // @ts-ignore navigation for `const markdownContainer = ref()`
    __VLS_ctx.markdownContainer;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("notes-panel") }, { class: (({ 'notes-panel-visible': __VLS_ctx.showNotes })) }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("notes-header") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.h3, __VLS_intrinsicElements.h3)({});
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.toggleNotesPanel) }, { class: ("close-btn") }));
    const __VLS_54 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_55 = __VLS_asFunctionalComponent(__VLS_54, new __VLS_54({ component: ((__VLS_ctx.CloseOutline)), size: ("18"), }));
    const __VLS_56 = __VLS_55({ component: ((__VLS_ctx.CloseOutline)), size: ("18"), }, ...__VLS_functionalComponentArgsRest(__VLS_55));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("notes-content") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.textarea, __VLS_intrinsicElements.textarea)(Object.assign({ value: ((__VLS_ctx.notesContent)), placeholder: ("点击Markdown内容添加到笔记...") }, { class: ("notes-textarea") }));
    const __VLS_60 = __VLS_resolvedLocalAndGlobalComponents.transition;
    /** @type { [typeof __VLS_components.Transition, typeof __VLS_components.transition, typeof __VLS_components.Transition, typeof __VLS_components.transition, ] } */
    // @ts-ignore
    const __VLS_61 = __VLS_asFunctionalComponent(__VLS_60, new __VLS_60({ name: ("fade"), }));
    const __VLS_62 = __VLS_61({ name: ("fade"), }, ...__VLS_functionalComponentArgsRest(__VLS_61));
    if (__VLS_ctx.showNoteBubble) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("note-added-bubble") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("bubble-content") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)({});
        __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ("icon-check") }));
    }
    __VLS_nonNullable(__VLS_65.slots).default;
    var __VLS_65;
    __VLS_styleScopedClasses['pdf-viewer-container'];
    __VLS_styleScopedClasses['pdf-controls'];
    __VLS_styleScopedClasses['page-info'];
    __VLS_styleScopedClasses['page-navigation'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['right-controls'];
    __VLS_styleScopedClasses['rotate-left'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['rotate-right'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['fit-width'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['zoom-out'];
    __VLS_styleScopedClasses['zoom-level'];
    __VLS_styleScopedClasses['zoom-in'];
    __VLS_styleScopedClasses['toggle-notes'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['export-notes'];
    __VLS_styleScopedClasses['btn-text'];
    __VLS_styleScopedClasses['pdf-main-content'];
    __VLS_styleScopedClasses['pdf-render-panel'];
    __VLS_styleScopedClasses['pdf-image-container'];
    __VLS_styleScopedClasses['rendered-pdf-image'];
    __VLS_styleScopedClasses['pdf-loading-message'];
    __VLS_styleScopedClasses['resize-handle'];
    __VLS_styleScopedClasses['markdown-panel'];
    __VLS_styleScopedClasses['markdown-content'];
    __VLS_styleScopedClasses['notes-panel'];
    __VLS_styleScopedClasses['notes-panel-visible'];
    __VLS_styleScopedClasses['notes-header'];
    __VLS_styleScopedClasses['close-btn'];
    __VLS_styleScopedClasses['notes-content'];
    __VLS_styleScopedClasses['notes-textarea'];
    __VLS_styleScopedClasses['note-added-bubble'];
    __VLS_styleScopedClasses['bubble-content'];
    __VLS_styleScopedClasses['icon-check'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "pdfPanel": __VLS_nativeElements['div'],
        "pdfImage": __VLS_nativeElements['img'],
        "markdownContainer": __VLS_nativeElements['div'],
    };
    var $refs;
    var $el;
    return {
        attrs: {},
        slots: __VLS_slots,
        refs: $refs,
        rootEl: $el,
    };
}
;
const __VLS_self = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {
            ArrowBackOutline: ionicons5_1.ArrowBackOutline,
            ArrowForwardOutline: ionicons5_1.ArrowForwardOutline,
            RefreshCircleOutline: ionicons5_1.RefreshCircleOutline,
            ExpandOutline: ionicons5_1.ExpandOutline,
            AddOutline: ionicons5_1.AddOutline,
            RemoveOutline: ionicons5_1.RemoveOutline,
            DocumentTextOutline: ionicons5_1.DocumentTextOutline,
            DownloadOutline: ionicons5_1.DownloadOutline,
            CloseOutline: ionicons5_1.CloseOutline,
            markdownContainer: markdownContainer,
            zoomLevel: zoomLevel,
            rotation: rotation,
            pdfPanel: pdfPanel,
            pdfImage: pdfImage,
            leftPanelWidth: leftPanelWidth,
            currentPage: currentPage,
            totalPages: totalPages,
            jumpPage: jumpPage,
            elements: elements,
            showNotes: showNotes,
            notesContent: notesContent,
            showNoteBubble: showNoteBubble,
            renderedMarkdown: renderedMarkdown,
            handleMarkdownClick: handleMarkdownClick,
            handleMarkdownHover: handleMarkdownHover,
            handleMarkdownLeave: handleMarkdownLeave,
            exportNotes: exportNotes,
            prevPage: prevPage,
            nextPage: nextPage,
            goToPage: goToPage,
            toggleNotesPanel: toggleNotesPanel,
            zoomIn: zoomIn,
            zoomOut: zoomOut,
            startResize: startResize,
            rotate: rotate,
            fitToWidth: fitToWidth,
            handleWheelZoom: handleWheelZoom,
        };
    },
    props: {
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
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {};
    },
    props: {
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
    },
    __typeEl: {},
});
; /* PartiallyEnd: #4569/main.vue */
