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
    }
});
// PDF数据状态
const currentPage = (0, vue_1.ref)(1);
const totalPages = (0, vue_1.ref)(0);
const jumpPage = (0, vue_1.ref)(1);
const thumbnail = (0, vue_1.ref)('');
const elements = (0, vue_1.ref)([]);
// 笔记状态
const showNotes = (0, vue_1.ref)(false);
const notesContent = (0, vue_1.ref)('');
// UI状态
const showLargeThumbnail = (0, vue_1.ref)(false);
const hoveredIndex = (0, vue_1.ref)(-1);
const showNoteBubble = (0, vue_1.ref)(false);
const hoverTimeout = (0, vue_1.ref)(null);
// 方法定义
const hoverElement = (index) => {
    clearTimeout(hoverTimeout.value);
    hoveredIndex.value = index;
};
const keepHovered = (index) => {
    clearTimeout(hoverTimeout.value);
    hoveredIndex.value = index;
};
const delayUnhover = (index) => {
    hoverTimeout.value = setTimeout(() => {
        if (hoveredIndex.value === index) {
            hoveredIndex.value = -1;
        }
    }, 300);
};
// 显示添加笔记的提示气泡
const showNoteBubbleMessage = () => {
    showNoteBubble.value = true;
    setTimeout(() => {
        showNoteBubble.value = false;
    }, 2000); // 2秒后自动消失
};
// 加载PDF页面
const loadPdfPage = (pageNum) => __awaiter(void 0, void 0, void 0, function* () {
    try {
        const response = yield (0, core_1.invoke)('reader_open_file', {
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
        }
        catch (e) {
            throw new Error('Failed to parse response content');
        }
        if (data.error) {
            throw new Error(data.error);
        }
        currentPage.value = data.page_number;
        totalPages.value = data.total_pages;
        thumbnail.value = data.thumbnail || '';
        elements.value = data.elements || [];
    }
    catch (error) {
        console.error('加载PDF失败:', error);
        // 显示用户友好的错误信息
        alert(`加载PDF失败: ${error.message}`);
        // 重置状态
        currentPage.value = 1;
        totalPages.value = 0;
        thumbnail.value = '';
        elements.value = [];
    }
});
// 添加内容到笔记
const addToNotes = (element) => {
    const timestamp = new Date().toLocaleString();
    let note = '';
    switch (element.type) {
        case 'text':
            note = `## 文本摘录 (${timestamp})\n${element.content}\n\n`;
            break;
        case 'image':
            note = `## 图片摘录 (${timestamp})\n![图片](${element.content})\n\n`;
            break;
        case 'table':
            note = `## 表格摘录 (${timestamp})\n![表格](${element.content})\n\n`;
            break;
    }
    notesContent.value += note;
    showNotes.value = true;
    showNoteBubbleMessage();
    showNoteBubble.value = true;
    setTimeout(() => {
        showNoteBubble.value = false;
    }, 2000);
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
// 分页控制
const prevPage = () => {
    if (currentPage.value > 1) {
        loadPdfPage(currentPage.value - 1);
    }
};
const nextPage = () => {
    if (currentPage.value < totalPages.value) {
        loadPdfPage(currentPage.value + 1);
    }
};
const goToPage = (page) => {
    const pageNum = typeof page === 'number' ? page : jumpPage.value;
    const targetPage = Math.max(1, Math.min(pageNum, totalPages.value));
    loadPdfPage(targetPage);
};
// UI控制
const toggleThumbnailView = () => {
    showLargeThumbnail.value = !showLargeThumbnail.value;
};
const toggleNotesPanel = () => {
    showNotes.value = !showNotes.value;
};
const handleThumbnailScroll = (e) => {
    // 实现缩略图滚动逻辑
};
// 初始化加载第一页
(0, vue_1.onMounted)(() => {
    store.get_librariesRoot();
    loadPdfPage(1);
});
// 监听PDF路径变化
(0, vue_1.watch)(() => props.filePath, (newPath) => {
    if (newPath) {
        currentPage.value = 1;
        loadPdfPage(1);
        notesContent.value = ''; // 清空笔记
    }
}); /* PartiallyEnd: #3632/scriptSetup.vue */
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
    __VLS_styleScopedClasses['toggle-notes'];
    __VLS_styleScopedClasses['page-navigation'];
    __VLS_styleScopedClasses['export-notes'];
    __VLS_styleScopedClasses['thumbnail-container'];
    __VLS_styleScopedClasses['large-thumbnail'];
    __VLS_styleScopedClasses['thumbnail-item'];
    __VLS_styleScopedClasses['thumbnail-current'];
    __VLS_styleScopedClasses['image-element'];
    __VLS_styleScopedClasses['table-element'];
    __VLS_styleScopedClasses['text-element'];
    __VLS_styleScopedClasses['element-hover-indicator'];
    __VLS_styleScopedClasses['image-element'];
    __VLS_styleScopedClasses['element-hover-indicator'];
    __VLS_styleScopedClasses['table-element'];
    __VLS_styleScopedClasses['element-hover-indicator'];
    __VLS_styleScopedClasses['text-element'];
    __VLS_styleScopedClasses['image-element'];
    __VLS_styleScopedClasses['hovered'];
    __VLS_styleScopedClasses['table-element'];
    __VLS_styleScopedClasses['hovered'];
    __VLS_styleScopedClasses['content-element'];
    __VLS_styleScopedClasses['hovered'];
    __VLS_styleScopedClasses['bubble-content'];
    __VLS_styleScopedClasses['notes-header'];
    __VLS_styleScopedClasses['close-btn'];
    __VLS_styleScopedClasses['notes-textarea'];
    __VLS_styleScopedClasses['notes-textarea'];
    __VLS_styleScopedClasses['notes-textarea'];
    __VLS_styleScopedClasses['notes-textarea'];
    __VLS_styleScopedClasses['notes-textarea'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-viewer-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-controls") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("page-info") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)({});
    (__VLS_ctx.currentPage);
    (__VLS_ctx.totalPages);
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("page-navigation") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.prevPage) }, { disabled: ((__VLS_ctx.currentPage <= 1)) }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ("icon-arrow-left") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.input, __VLS_intrinsicElements.input)(Object.assign({ onKeyup: (__VLS_ctx.goToPage) }, { type: ("number"), min: ("1"), max: ((__VLS_ctx.totalPages)) }));
    (__VLS_ctx.jumpPage);
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.nextPage) }, { disabled: ((__VLS_ctx.currentPage >= __VLS_ctx.totalPages)) }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ("icon-arrow-right") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("right-controls") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.toggleNotesPanel) }, { class: ("toggle-notes") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ((__VLS_ctx.showNotes ? 'icon-notes-active' : 'icon-notes')) }));
    (__VLS_ctx.showNotes ? '隐藏笔记' : '显示笔记');
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.exportNotes) }, { class: ("export-notes") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ("icon-download") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-main-content") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("thumbnail-panel") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("thumbnail-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.img, __VLS_intrinsicElements.img)(Object.assign(Object.assign({ onClick: (__VLS_ctx.toggleThumbnailView) }, { src: (('data:image/png;base64,' + __VLS_ctx.thumbnail)) }), { class: (({ 'thumbnail-active': !__VLS_ctx.showLargeThumbnail })) }));
    if (__VLS_ctx.showLargeThumbnail) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("large-thumbnail") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.img, __VLS_intrinsicElements.img)({ src: (('data:image/png;base64,' + __VLS_ctx.thumbnail)), });
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("thumbnail-page-number") }));
        (__VLS_ctx.currentPage);
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onScroll: (__VLS_ctx.handleThumbnailScroll) }, { class: ("thumbnail-scroll") }));
    for (const [page] of __VLS_getVForSourceType((__VLS_ctx.totalPages))) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign(Object.assign({ onClick: (...[$event]) => {
                __VLS_ctx.goToPage(page);
            } }, { key: ((page)) }), { class: ("thumbnail-item") }), { class: (({ 'thumbnail-current': page === __VLS_ctx.currentPage })) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)(Object.assign({ class: ("thumbnail-page-indicator") }));
        (page);
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("content-panel") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("pdf-content") }, { ref: ("pdfContent") }));
    // @ts-ignore navigation for `const pdfContent = ref()`
    __VLS_ctx.pdfContent;
    for (const [element, index] of __VLS_getVForSourceType((__VLS_ctx.elements))) {
        (index);
        if (element.type === 'text') {
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign(Object.assign(Object.assign({ onMouseenter: (...[$event]) => {
                    if (!((element.type === 'text')))
                        return;
                    __VLS_ctx.hoverElement(index);
                } }, { onMouseleave: (...[$event]) => {
                    if (!((element.type === 'text')))
                        return;
                    __VLS_ctx.delayUnhover(index);
                } }), { onClick: (...[$event]) => {
                    if (!((element.type === 'text')))
                        return;
                    __VLS_ctx.addToNotes(element);
                } }), { class: ("text-element content-element") }), { class: (({ 'hovered': __VLS_ctx.hoveredIndex === index })) }));
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("element-content") }));
            (element.content);
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign({ onMouseenter: (...[$event]) => {
                    if (!((element.type === 'text')))
                        return;
                    __VLS_ctx.keepHovered(index);
                } }, { onMouseleave: (...[$event]) => {
                    if (!((element.type === 'text')))
                        return;
                    __VLS_ctx.delayUnhover(index);
                } }), { class: ("hover-area") }));
            const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.transition;
            /** @type { [typeof __VLS_components.Transition, typeof __VLS_components.transition, typeof __VLS_components.Transition, typeof __VLS_components.transition, ] } */
            // @ts-ignore
            const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ name: ("bubble"), }));
            const __VLS_2 = __VLS_1({ name: ("bubble"), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
            if (__VLS_ctx.hoveredIndex === index) {
                __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onClick: (...[$event]) => {
                        if (!((element.type === 'text')))
                            return;
                        if (!((__VLS_ctx.hoveredIndex === index)))
                            return;
                        __VLS_ctx.addToNotes(element);
                    } }, { class: ("hover-bubble") }));
            }
            __VLS_nonNullable(__VLS_5.slots).default;
            var __VLS_5;
        }
        else if (element.type === 'image') {
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign(Object.assign(Object.assign({ onMouseenter: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!((element.type === 'image')))
                        return;
                    __VLS_ctx.hoverElement(index);
                } }, { onMouseleave: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!((element.type === 'image')))
                        return;
                    __VLS_ctx.delayUnhover(index);
                } }), { onClick: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!((element.type === 'image')))
                        return;
                    __VLS_ctx.addToNotes(element);
                } }), { class: ("image-element content-element") }), { class: (({ 'hovered': __VLS_ctx.hoveredIndex === index })) }));
            __VLS_elementAsFunction(__VLS_intrinsicElements.img, __VLS_intrinsicElements.img)({ src: (('data:image/png;base64,' + element.content)), });
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign({ onMouseenter: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!((element.type === 'image')))
                        return;
                    __VLS_ctx.keepHovered(index);
                } }, { onMouseleave: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!((element.type === 'image')))
                        return;
                    __VLS_ctx.delayUnhover(index);
                } }), { class: ("hover-area") }));
            const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.transition;
            /** @type { [typeof __VLS_components.Transition, typeof __VLS_components.transition, typeof __VLS_components.Transition, typeof __VLS_components.transition, ] } */
            // @ts-ignore
            const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6({ name: ("bubble"), }));
            const __VLS_8 = __VLS_7({ name: ("bubble"), }, ...__VLS_functionalComponentArgsRest(__VLS_7));
            if (__VLS_ctx.hoveredIndex === index) {
                __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onClick: (...[$event]) => {
                        if (!(!((element.type === 'text'))))
                            return;
                        if (!((element.type === 'image')))
                            return;
                        if (!((__VLS_ctx.hoveredIndex === index)))
                            return;
                        __VLS_ctx.addToNotes(element);
                    } }, { class: ("hover-bubble") }));
            }
            __VLS_nonNullable(__VLS_11.slots).default;
            var __VLS_11;
        }
        else if (element.type === 'table') {
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign(Object.assign(Object.assign({ onMouseenter: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!(!((element.type === 'image'))))
                        return;
                    if (!((element.type === 'table')))
                        return;
                    __VLS_ctx.hoverElement(index);
                } }, { onMouseleave: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!(!((element.type === 'image'))))
                        return;
                    if (!((element.type === 'table')))
                        return;
                    __VLS_ctx.delayUnhover(index);
                } }), { onClick: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!(!((element.type === 'image'))))
                        return;
                    if (!((element.type === 'table')))
                        return;
                    __VLS_ctx.addToNotes(element);
                } }), { class: ("table-element content-element") }), { class: (({ 'hovered': __VLS_ctx.hoveredIndex === index })) }));
            __VLS_elementAsFunction(__VLS_intrinsicElements.img, __VLS_intrinsicElements.img)({ src: (('data:image/png;base64,' + element.content)), });
            __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign({ onMouseenter: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!(!((element.type === 'image'))))
                        return;
                    if (!((element.type === 'table')))
                        return;
                    __VLS_ctx.keepHovered(index);
                } }, { onMouseleave: (...[$event]) => {
                    if (!(!((element.type === 'text'))))
                        return;
                    if (!(!((element.type === 'image'))))
                        return;
                    if (!((element.type === 'table')))
                        return;
                    __VLS_ctx.delayUnhover(index);
                } }), { class: ("hover-area") }));
            const __VLS_12 = __VLS_resolvedLocalAndGlobalComponents.transition;
            /** @type { [typeof __VLS_components.Transition, typeof __VLS_components.transition, typeof __VLS_components.Transition, typeof __VLS_components.transition, ] } */
            // @ts-ignore
            const __VLS_13 = __VLS_asFunctionalComponent(__VLS_12, new __VLS_12({ name: ("bubble"), }));
            const __VLS_14 = __VLS_13({ name: ("bubble"), }, ...__VLS_functionalComponentArgsRest(__VLS_13));
            if (__VLS_ctx.hoveredIndex === index) {
                __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onClick: (...[$event]) => {
                        if (!(!((element.type === 'text'))))
                            return;
                        if (!(!((element.type === 'image'))))
                            return;
                        if (!((element.type === 'table')))
                            return;
                        if (!((__VLS_ctx.hoveredIndex === index)))
                            return;
                        __VLS_ctx.addToNotes(element);
                    } }, { class: ("hover-bubble") }));
            }
            __VLS_nonNullable(__VLS_17.slots).default;
            var __VLS_17;
        }
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("notes-panel") }, { class: (({ 'notes-panel-visible': __VLS_ctx.showNotes })) }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("notes-header") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.h3, __VLS_intrinsicElements.h3)({});
    __VLS_elementAsFunction(__VLS_intrinsicElements.button, __VLS_intrinsicElements.button)(Object.assign({ onClick: (__VLS_ctx.toggleNotesPanel) }, { class: ("close-btn") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ("icon-close") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("notes-content") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.textarea, __VLS_intrinsicElements.textarea)(Object.assign({ value: ((__VLS_ctx.notesContent)), placeholder: ("点击PDF内容添加到笔记...") }, { class: ("notes-textarea") }));
    const __VLS_18 = __VLS_resolvedLocalAndGlobalComponents.transition;
    /** @type { [typeof __VLS_components.Transition, typeof __VLS_components.transition, typeof __VLS_components.Transition, typeof __VLS_components.transition, ] } */
    // @ts-ignore
    const __VLS_19 = __VLS_asFunctionalComponent(__VLS_18, new __VLS_18({ name: ("fade"), }));
    const __VLS_20 = __VLS_19({ name: ("fade"), }, ...__VLS_functionalComponentArgsRest(__VLS_19));
    if (__VLS_ctx.showNoteBubble) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("note-added-bubble") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("bubble-content") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)({});
        __VLS_elementAsFunction(__VLS_intrinsicElements.i, __VLS_intrinsicElements.i)(Object.assign({ class: ("icon-check") }));
    }
    __VLS_nonNullable(__VLS_23.slots).default;
    var __VLS_23;
    __VLS_styleScopedClasses['pdf-viewer-container'];
    __VLS_styleScopedClasses['pdf-controls'];
    __VLS_styleScopedClasses['page-info'];
    __VLS_styleScopedClasses['page-navigation'];
    __VLS_styleScopedClasses['icon-arrow-left'];
    __VLS_styleScopedClasses['icon-arrow-right'];
    __VLS_styleScopedClasses['right-controls'];
    __VLS_styleScopedClasses['toggle-notes'];
    __VLS_styleScopedClasses['export-notes'];
    __VLS_styleScopedClasses['icon-download'];
    __VLS_styleScopedClasses['pdf-main-content'];
    __VLS_styleScopedClasses['thumbnail-panel'];
    __VLS_styleScopedClasses['thumbnail-container'];
    __VLS_styleScopedClasses['thumbnail-active'];
    __VLS_styleScopedClasses['large-thumbnail'];
    __VLS_styleScopedClasses['thumbnail-page-number'];
    __VLS_styleScopedClasses['thumbnail-scroll'];
    __VLS_styleScopedClasses['thumbnail-item'];
    __VLS_styleScopedClasses['thumbnail-current'];
    __VLS_styleScopedClasses['thumbnail-page-indicator'];
    __VLS_styleScopedClasses['content-panel'];
    __VLS_styleScopedClasses['pdf-content'];
    __VLS_styleScopedClasses['text-element'];
    __VLS_styleScopedClasses['content-element'];
    __VLS_styleScopedClasses['hovered'];
    __VLS_styleScopedClasses['element-content'];
    __VLS_styleScopedClasses['hover-area'];
    __VLS_styleScopedClasses['hover-bubble'];
    __VLS_styleScopedClasses['image-element'];
    __VLS_styleScopedClasses['content-element'];
    __VLS_styleScopedClasses['hovered'];
    __VLS_styleScopedClasses['hover-area'];
    __VLS_styleScopedClasses['hover-bubble'];
    __VLS_styleScopedClasses['table-element'];
    __VLS_styleScopedClasses['content-element'];
    __VLS_styleScopedClasses['hovered'];
    __VLS_styleScopedClasses['hover-area'];
    __VLS_styleScopedClasses['hover-bubble'];
    __VLS_styleScopedClasses['notes-panel'];
    __VLS_styleScopedClasses['notes-panel-visible'];
    __VLS_styleScopedClasses['notes-header'];
    __VLS_styleScopedClasses['close-btn'];
    __VLS_styleScopedClasses['icon-close'];
    __VLS_styleScopedClasses['notes-content'];
    __VLS_styleScopedClasses['notes-textarea'];
    __VLS_styleScopedClasses['note-added-bubble'];
    __VLS_styleScopedClasses['bubble-content'];
    __VLS_styleScopedClasses['icon-check'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "pdfContent": __VLS_nativeElements['div'],
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
        return Object.assign(Object.assign({ $props: __VLS_makeOptional(props) }, props), { currentPage: currentPage, totalPages: totalPages, jumpPage: jumpPage, thumbnail: thumbnail, elements: elements, showNotes: showNotes, notesContent: notesContent, showLargeThumbnail: showLargeThumbnail, hoveredIndex: hoveredIndex, showNoteBubble: showNoteBubble, hoverElement: hoverElement, keepHovered: keepHovered, delayUnhover: delayUnhover, addToNotes: addToNotes, exportNotes: exportNotes, prevPage: prevPage, nextPage: nextPage, goToPage: goToPage, toggleThumbnailView: toggleThumbnailView, toggleNotesPanel: toggleNotesPanel, handleThumbnailScroll: handleThumbnailScroll });
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return Object.assign({ $props: __VLS_makeOptional(props) }, props);
    },
    __typeEl: {},
});
; /* PartiallyEnd: #4569/main.vue */
