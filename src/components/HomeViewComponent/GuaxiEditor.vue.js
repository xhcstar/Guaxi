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
var _a;
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
const fileTreeStore_1 = require("../../stores/fileTreeStore");
const naive_ui_1 = require("naive-ui");
const ionicons5_1 = require("@vicons/ionicons5");
const fluent_1 = require("@vicons/fluent");
const path_1 = require("@tauri-apps/api/path");
const GuaxiMarker_1 = require("../GuaxiMarkdownRender/GuaxiMarker");
const lodash_es_1 = require("lodash-es");
const monaco = __importStar(require("monaco-editor"));
const monaco_vim_1 = require("monaco-vim");
// 更新所有语言导入路径
require("monaco-editor/esm/vs/basic-languages/python/python.contribution");
require("monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution");
require("monaco-editor/esm/vs/basic-languages/rust/rust.contribution");
require("monaco-editor/esm/vs/basic-languages/shell/shell.contribution");
require("monaco-editor/esm/vs/basic-languages/html/html.contribution");
require("monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution");
require("monaco-editor/esm/vs/basic-languages/css/css.contribution");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
// 引入 Pinia store
const fileTreeStore = (0, fileTreeStore_1.useFileTreeStore)();
const mode = (0, vue_1.ref)('edit');
const renderedHtml = (0, vue_1.ref)('');
const previewContainer = (0, vue_1.ref)(null);
const editorContainer = (0, vue_1.ref)(null);
let editor = null;
const selectedFile = (0, vue_1.computed)(() => fileTreeStore.selectedFile);
// 定义 rawContent 变量
const rawContent = (0, vue_1.ref)(((_a = selectedFile.value) === null || _a === void 0 ? void 0 : _a.content) || '');
const message = (0, naive_ui_1.useMessage)(); // 使用消息提示
const vimMode = (0, vue_1.ref)(false);
const vimStatusBar = (0, vue_1.ref)(null);
let vimModeInstance = null;
// 初始化编辑器
const initEditor = () => {
    var _a, _b;
    if (!editorContainer.value) {
        console.error('编辑器容器未找到');
        return;
    }
    // 获取文件扩展名并确定语言
    const fileExt = ((_b = (_a = selectedFile.value) === null || _a === void 0 ? void 0 : _a.path) === null || _b === void 0 ? void 0 : _b.split('.').pop()) || 'markdown';
    const languageMap = {
        'js': 'javascript',
        'ts': 'typescript',
        'css': 'css',
        'html': 'html',
        'md': 'markdown',
        'py': 'python',
        'cpp': 'cpp',
        'c': 'cpp',
        'h': 'cpp',
        'hpp': 'cpp',
        'rs': 'rust',
        'sh': 'shell',
        'jsx': 'javascript',
        'tsx': 'javascript',
        'htm': 'html',
    };
    const language = languageMap[fileExt] || 'markdown';
    editor = monaco.editor.create(editorContainer.value, {
        value: rawContent.value,
        language: language,
        theme: 'vs',
        automaticLayout: true, // 自动布局
        minimap: {
            enabled: true, // 启用迷你地图
            showSlider: 'always',
            size: 'fit'
        },
        scrollbar: {
            vertical: 'auto',
            horizontal: 'auto',
            useShadows: true,
            verticalScrollbarSize: 10,
            horizontalScrollbarSize: 10,
            arrowSize: 10,
            alwaysConsumeMouseWheel: false,
            verticalHasArrows: false,
            horizontalHasArrows: false,
            handleMouseWheel: true,
            scrollByPage: false,
            theme: {
                scrollbarShadow: '#00000022',
                scrollbarSliderBackground: '#64646466',
                scrollbarSliderHoverBackground: '#64646499',
                scrollbarSliderActiveBackground: '#646464CC'
            }
        },
        vim: {
            enabled: vimMode.value,
            status: true,
            useSystemClipboard: true,
            handleKeys: {
                '<C-s>': false,
                '<C-c>': false,
                '<C-v>': false,
                '<C-a>': false
            }
        },
        contextmenu: {
            enabled: true,
            addDefaultActions: true,
            addNavigationActions: true
        },
        lineNumbers: 'on',
        lineNumbersMinChars: 3, // 最小行号宽度
        scrollBeyondLastLine: false,
        fontSize: 18, // 调整为更合适的字号
        fontFamily: "'Maple Mono NF CN','JetBrainsMono Nerd Font Mono', Consolas, monospace, 'Microsoft YaHei', 'Courier New'",
        fontLigatures: true,
        wordWrap: 'on', // 自动换行
        wrappingIndent: 'indent', // 换行缩进
        renderWhitespace: 'selection', // 选中时显示空白字符
        renderLineHighlight: 'all', // 高亮当前行
        lineHeight: 24, // 行高
        padding: {
            top: 8, // 顶部内边距
            bottom: 8 // 底部内边距
        },
        suggest: {
            snippetsPreventQuickSuggestions: true // 允许代码片段建议
        },
        quickSuggestions: {
            other: true,
            comments: false,
            strings: true
        },
        parameterHints: {
            enabled: true // 启用参数提示
        },
        bracketPairColorization: {
            enabled: true // 括号配对着色
        },
        guides: {
            bracketPairs: true, // 显示括号配对引导线
            bracketPairsHorizontal: 'active' // 水平引导线
        },
        mouseWheelZoom: true // 启用鼠标滚轮缩放
    });
    if (vimMode.value) {
        (0, vue_1.nextTick)(() => {
            if (editor && vimStatusBar.value) {
                vimModeInstance = (0, monaco_vim_1.initVimMode)(editor, vimStatusBar.value);
                editor.updateOptions({
                    vim: {
                        enabled: true,
                        status: true,
                        useSystemClipboard: true,
                        handleKeys: {
                            '<C-s>': false,
                            '<C-c>': false,
                            '<C-v>': false
                        }
                    }
                });
            }
        });
    }
    // 注册自定义命令
    const registerCommands = () => {
        // Markdown模板命令
        editor.addAction({
            id: 'insert.md.template',
            label: '插入Markdown模板',
            run: () => {
                const selection = editor.getSelection();
                editor.executeEdits('', [{
                        range: selection,
                        text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
                    }]);
            }
        });
    };
    // 注册自定义命令和菜单项
    const registerContextMenu = () => {
        // 1. 注册Markdown模板命令
        editor.addAction({
            id: 'insert.md.template',
            label: '插入Markdown模板',
            contextMenuGroupId: '2_custom',
            contextMenuOrder: 1,
            run: () => {
                const selection = editor.getSelection();
                editor.executeEdits('', [{
                        range: selection,
                        text: '# 标题\n\n## 二级标题\n\n- 列表项1\n- 列表项2\n\n```python\n# 代码块\nprint("Hello World")\n```\n\n> 引用内容\n\n**粗体** *斜体*\n\n[链接文字](https://example.com)\n\n![图片描述](图片URL)'
                    }]);
            }
        });
        // 2. 覆盖默认上下文菜单
        editor.onContextMenu((e) => {
            e.event.preventDefault();
            // 获取鼠标位置
            const { posx, posy } = e.event;
            // 3. 定义完整的菜单结构
            const menuItems = [
                // 基础编辑操作
                { id: 'editor.action.clipboardCutAction', label: '剪切' },
                { id: 'editor.action.clipboardCopyAction', label: '复制' },
                { id: 'editor.action.clipboardPasteAction', label: '粘贴' },
                { type: 'separator' },
                // 自定义功能
                { id: 'insert.md.template', label: '插入Markdown模板' },
                { type: 'separator' },
                // 其他功能
                { id: 'editor.action.quickCommand', label: '命令面板' }
            ];
            // 4. 触发菜单显示
            editor.focus();
            editor.trigger('', 'editor.action.showContextMenu', {
                anchor: { x: posx, y: posy },
                menuItems,
                domNode: editorContainer.value
            });
        });
    };
    // 初始化上下文菜单
    registerContextMenu();
    editor.onDidChangeModelContent(() => {
        rawContent.value = editor.getValue();
        // 内容变化时触发预览更新
        if (mode.value === 'preview') {
            renderDebounced(rawContent.value);
        }
    });
};
// 更新编辑器内容
const updateEditorContent = (content) => {
    if (editor) {
        editor.setValue(content);
        rawContent.value = content;
    }
    else {
        initEditor();
    }
};
// 保存内容
const saveContent = () => {
    if (selectedFile.value && editor) {
        const content = editor.getValue();
        fileTreeStore.updateFileContent(selectedFile.value.path, content);
        message.info('修改已保存');
        // 如果是vim模式，也显示保存提示
        if (vimMode.value) {
            message.info('修改已保存');
        }
    }
};
// 快捷键处理
const handleKeyDown = (e) => {
    if (e.ctrlKey && e.key === 's') {
        e.preventDefault();
        saveContent();
    }
};
// 切换模式
const toggleMode = (0, lodash_es_1.debounce)(() => __awaiter(void 0, void 0, void 0, function* () {
    mode.value = mode.value === 'edit' ? 'preview' : 'edit';
    if (mode.value === 'edit') {
        yield (0, vue_1.nextTick)();
        if (!editor) {
            initEditor();
        }
    }
    else {
        yield renderDebounced(rawContent.value);
    }
}), 150);
// 防抖渲染
const renderDebounced = (0, lodash_es_1.debounce)((content) => __awaiter(void 0, void 0, void 0, function* () {
    if (!selectedFile.value || !mode.value || !content) {
        renderedHtml.value = '';
        return;
    }
    try {
        const basePath = selectedFile.value.path ? yield (0, path_1.dirname)(selectedFile.value.path) : '';
        renderedHtml.value = yield (0, GuaxiMarker_1.renderMarkdown)(content, {
            basePath: basePath,
            mermaid: true,
            math: true,
            codeHighlight: true,
            theme: 'default'
        });
        // 确保DOM更新后添加按钮
        (0, vue_1.nextTick)(() => {
            if (previewContainer.value) {
                (0, GuaxiMarker_1.addCopyButtons)(previewContainer.value);
            }
        });
    }
    catch (error) {
        console.error('渲染错误:', error);
        renderedHtml.value = `
      <div class="render-error">
        <p>渲染错误: ${error instanceof Error ? error.message : String(error)}</p>
        <button @click="retryRender">重试</button>
      </div>
    `;
    }
}), 300);
// 监听文件选择
(0, vue_1.watch)(selectedFile, (newFile) => {
    var _a;
    if (newFile) {
        const fileExt = ((_a = newFile.path) === null || _a === void 0 ? void 0 : _a.split('.').pop()) || 'markdown';
        const languageMap = {
            'py': 'python',
            'cpp': 'cpp',
            'c': 'cpp',
            'h': 'cpp',
            'hpp': 'cpp',
            'rs': 'rust',
            'sh': 'shell',
            'js': 'javascript',
            'jsx': 'javascript',
            'ts': 'javascript',
            'tsx': 'javascript',
            'html': 'html',
            'htm': 'html',
            'css': 'css',
            'md': 'markdown'
        };
        const language = languageMap[fileExt] || 'markdown';
        if (editor) {
            monaco.editor.setModelLanguage(editor.getModel(), language);
        }
        // 更新内容
        rawContent.value = newFile.content || '';
        if (editor) {
            updateEditorContent(newFile.content || '');
        }
        else {
            (0, vue_1.nextTick)(() => {
                initEditor();
                updateEditorContent(newFile.content || '');
            });
        }
        if (!selectedFile.value || !mode.value) {
            renderedHtml.value = '';
            return;
        }
        // 如果是预览模式立即渲染
        if (mode.value === 'preview') {
            renderDebounced(newFile.content);
        }
    }
}, { immediate: true });
// 监听内容变化
(0, vue_1.watch)([rawContent, mode], () => {
    if (mode.value === 'preview') {
        renderDebounced(rawContent.value);
    }
});
// 监听内容变化
(0, vue_1.watch)(renderedHtml, () => {
    (0, vue_1.nextTick)(() => {
        if (previewContainer.value) {
            (0, GuaxiMarker_1.addCopyButtons)(previewContainer.value);
        }
    });
});
// 添加Vim模式切换函数
const toggleVimMode = () => {
    if (mode.value === 'preview') {
        message.warning('阅读模式下不能启用Vim编辑模式');
        return;
    }
    vimMode.value = !vimMode.value;
    (0, vue_1.nextTick)(() => {
        if (vimMode.value) {
            if (editor && vimStatusBar.value) {
                vimModeInstance = (0, monaco_vim_1.initVimMode)(editor, vimStatusBar.value);
                editor.updateOptions({
                    vim: {
                        enabled: true,
                        status: true,
                        useSystemClipboard: true,
                        handleKeys: {
                            '<C-s>': false,
                            '<C-c>': false,
                            '<C-v>': false
                        }
                    }
                });
            }
        }
        else {
            if (vimModeInstance) {
                vimModeInstance.dispose();
                vimModeInstance = null;
            }
            if (editor) {
                editor.updateOptions({
                    vim: {
                        enabled: false
                    }
                });
            }
        }
    });
};
// 生命周期
(0, vue_1.onMounted)(() => {
    window.addEventListener('keydown', handleKeyDown);
});
(0, vue_1.onUnmounted)(() => {
    window.removeEventListener('keydown', handleKeyDown);
    editor === null || editor === void 0 ? void 0 : editor.dispose();
    editor = null;
    if (vimModeInstance) {
        vimModeInstance.dispose();
    }
});
; /* PartiallyEnd: #3632/scriptSetup.vue */
const __VLS_fnComponent = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({});
;
let __VLS_functionalComponentProps;
function __VLS_template() {
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign(Object.assign({}, {}), {}), __VLS_ctx);
    let __VLS_components;
    const __VLS_localDirectives = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_directives;
    let __VLS_styleScopedClasses;
    __VLS_styleScopedClasses['toolbar'];
    __VLS_styleScopedClasses['mode-toggle'];
    __VLS_styleScopedClasses['mode-toggle'];
    __VLS_styleScopedClasses['preview'];
    __VLS_styleScopedClasses['render-error'];
    __VLS_styleScopedClasses['toolbar'];
    __VLS_styleScopedClasses['dark'];
    __VLS_styleScopedClasses['mode-toggle'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("editor-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("toolbar") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("toolbar-spacer") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("toolbar-actions") }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
    /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ placement: ("top"), trigger: ("hover"), }));
    const __VLS_2 = __VLS_1({ placement: ("top"), trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_5.slots);
        const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6(Object.assign({ 'onClick': {} }, { text: (true) })));
        const __VLS_8 = __VLS_7(Object.assign({ 'onClick': {} }, { text: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_7));
        let __VLS_12;
        const __VLS_13 = {
            onClick: (__VLS_ctx.toggleVimMode)
        };
        let __VLS_9;
        let __VLS_10;
        const __VLS_14 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_15 = __VLS_asFunctionalComponent(__VLS_14, new __VLS_14({ component: ((__VLS_ctx.vimMode ? __VLS_ctx.LogoVimeo : __VLS_ctx.Edit16Filled)), }));
        const __VLS_16 = __VLS_15({ component: ((__VLS_ctx.vimMode ? __VLS_ctx.LogoVimeo : __VLS_ctx.Edit16Filled)), }, ...__VLS_functionalComponentArgsRest(__VLS_15));
        __VLS_nonNullable(__VLS_11.slots).default;
        var __VLS_11;
    }
    (__VLS_ctx.vimMode ? '禁用Vim模式' : '启用Vim模式');
    var __VLS_5;
    const __VLS_20 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
    /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
    // @ts-ignore
    const __VLS_21 = __VLS_asFunctionalComponent(__VLS_20, new __VLS_20({ placement: ("top"), trigger: ("hover"), }));
    const __VLS_22 = __VLS_21({ placement: ("top"), trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_21));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_25.slots);
        const __VLS_26 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_27 = __VLS_asFunctionalComponent(__VLS_26, new __VLS_26(Object.assign({ 'onClick': {} }, { text: (true) })));
        const __VLS_28 = __VLS_27(Object.assign({ 'onClick': {} }, { text: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_27));
        let __VLS_32;
        const __VLS_33 = {
            onClick: (__VLS_ctx.toggleMode)
        };
        let __VLS_29;
        let __VLS_30;
        const __VLS_34 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_35 = __VLS_asFunctionalComponent(__VLS_34, new __VLS_34({ component: ((__VLS_ctx.mode === 'edit' ? __VLS_ctx.DocumentEdit16Regular : __VLS_ctx.EyeOutline)), }));
        const __VLS_36 = __VLS_35({ component: ((__VLS_ctx.mode === 'edit' ? __VLS_ctx.DocumentEdit16Regular : __VLS_ctx.EyeOutline)), }, ...__VLS_functionalComponentArgsRest(__VLS_35));
        __VLS_nonNullable(__VLS_31.slots).default;
        var __VLS_31;
    }
    (__VLS_ctx.mode === 'edit' ? '编辑模式' : '阅读模式');
    var __VLS_25;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ ref: ("editorContainer") }, { class: ("editor-wrapper") }));
    __VLS_asFunctionalDirective(__VLS_directives.vShow)(null, Object.assign(Object.assign({}, __VLS_directiveBindingRestFields), { modifiers: {}, value: (__VLS_ctx.selectedFile && __VLS_ctx.mode === 'edit') }), null, null);
    // @ts-ignore navigation for `const editorContainer = ref()`
    __VLS_ctx.editorContainer;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ ref: ("editorContainer") }, { class: ("editor") }));
    // @ts-ignore navigation for `const editorContainer = ref()`
    __VLS_ctx.editorContainer;
    if (__VLS_ctx.vimMode) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("vim-status-bar") }, { ref: ("vimStatusBar") }));
        // @ts-ignore navigation for `const vimStatusBar = ref()`
        __VLS_ctx.vimStatusBar;
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("preview") }, { ref: ("previewContainer") }));
    __VLS_asFunctionalDirective(__VLS_directives.vShow)(null, Object.assign(Object.assign({}, __VLS_directiveBindingRestFields), { modifiers: {}, value: (__VLS_ctx.selectedFile && __VLS_ctx.mode === 'preview') }), null, null);
    __VLS_asFunctionalDirective(__VLS_directives.vHtml)(null, Object.assign(Object.assign({}, __VLS_directiveBindingRestFields), { modifiers: {}, value: (__VLS_ctx.renderedHtml) }), null, null);
    // @ts-ignore navigation for `const previewContainer = ref()`
    __VLS_ctx.previewContainer;
    __VLS_styleScopedClasses['editor-container'];
    __VLS_styleScopedClasses['toolbar'];
    __VLS_styleScopedClasses['toolbar-spacer'];
    __VLS_styleScopedClasses['toolbar-actions'];
    __VLS_styleScopedClasses['editor-wrapper'];
    __VLS_styleScopedClasses['editor'];
    __VLS_styleScopedClasses['vim-status-bar'];
    __VLS_styleScopedClasses['preview'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "editorContainer": __VLS_nativeElements['div'],
        "vimStatusBar": __VLS_nativeElements['div'],
        "previewContainer": __VLS_nativeElements['div'],
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
            NTooltip: naive_ui_1.NTooltip,
            NIcon: naive_ui_1.NIcon,
            EyeOutline: ionicons5_1.EyeOutline,
            LogoVimeo: ionicons5_1.LogoVimeo,
            DocumentEdit16Regular: fluent_1.DocumentEdit16Regular,
            Edit16Filled: fluent_1.Edit16Filled,
            mode: mode,
            renderedHtml: renderedHtml,
            previewContainer: previewContainer,
            editorContainer: editorContainer,
            selectedFile: selectedFile,
            vimMode: vimMode,
            vimStatusBar: vimStatusBar,
            toggleMode: toggleMode,
            toggleVimMode: toggleVimMode,
        };
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {};
    },
    __typeEl: {},
});
; /* PartiallyEnd: #4569/main.vue */
