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
const naive_ui_1 = require("naive-ui");
const core_1 = require("@tauri-apps/api/core");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const props = defineProps({
    value: [String, Number], // 单元格的值
    onUpdateValue: Function, // 更新值的回调函数
    enableAutoComplete: {
        type: Boolean,
        default: false,
    },
    searchType: String, // 搜索类型，用于区分不同的后端接口
});
const isEdit = (0, vue_1.ref)(false);
const inputRef = (0, vue_1.ref)(null);
const inputValue = (0, vue_1.ref)(props.value);
const options = (0, vue_1.ref)([]); // 自动补全的选项
const isLoading = (0, vue_1.ref)(false); // 是否正在加载
// 样式
const style = {
    minHeight: '22px',
    cursor: 'pointer',
};
// 限制 input 宽度
const inputStyle = {
    width: '100%', // 宽度与单元格一致
    maxWidth: '200px', // 最大宽度，避免过大
};
// 处理点击事件
const handleOnClick = () => {
    isEdit.value = true;
    (0, vue_1.nextTick)(() => {
        var _a;
        (_a = inputRef.value) === null || _a === void 0 ? void 0 : _a.focus(); // 确保焦点设置到输入框
    });
};
// 处理值更新
const handleUpdateValue = (v) => {
    inputValue.value = v;
};
const handleChange = () => {
    console.log(`handleChange 被调用，输入值: ${inputValue.value}`);
    props.onUpdateValue(inputValue.value); // 调用父组件的回调函数
    isEdit.value = false;
};
// 处理搜索
const handleSearch = (query) => __awaiter(void 0, void 0, void 0, function* () {
    if (!props.enableAutoComplete || !props.searchType)
        return;
    console.log(`handleSearch 被调用，搜索关键字: ${query}, 搜索类型: ${props.searchType}`);
    isLoading.value = true;
    try {
        let result;
        switch (props.searchType) {
            case 'noteTags':
                result = yield (0, core_1.invoke)('search_notes_tags', { query });
                break;
            case 'importedFileTags':
                result = yield (0, core_1.invoke)('search_imported_files_tags', { query });
                break;
            case 'importedFiles':
                result = yield (0, core_1.invoke)('search_imported_files', { query });
                break;
            case 'libraries':
                result = yield (0, core_1.invoke)('search_libraries', { query });
                break;
            case 'importedFileLibraries':
                result = yield (0, core_1.invoke)('search_imported_file_libraries', { query });
                break;
            default:
                // console.error('未知的搜索类型:', props.searchType);
                return;
        }
        if (!result) {
            console.error('搜索结果为空');
            return;
        }
        else if (result.length === 0) {
            console.log('搜索结果为空');
        }
        else {
            console.log(`handleSearch 返回结果:`, result);
            options.value = result;
        }
    }
    catch (error) {
        console.error('搜索失败:', error);
    }
    finally {
        isLoading.value = false;
    }
});
// 监听输入值变化，触发搜索
(0, vue_1.watch)(inputValue, (newValue) => {
    if (isEdit.value && props.enableAutoComplete && newValue) {
        handleSearch(newValue);
    }
});
// 点击其他地方关闭编辑模式
const handleClickOutside = (event) => {
    if (isEdit.value && !event.target.closest('.editable-cell')) {
        handleChange();
    }
};
(0, vue_1.onMounted)(() => {
    document.addEventListener('click', handleClickOutside);
});
(0, vue_1.onUnmounted)(() => {
    document.removeEventListener('click', handleClickOutside);
});
; /* PartiallyEnd: #3632/scriptSetup.vue */
const __VLS_fnComponent = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    props: {
        value: [String, Number], // 单元格的值
        onUpdateValue: Function, // 更新值的回调函数
        enableAutoComplete: {
            type: Boolean,
            default: false,
        },
        searchType: String, // 搜索类型，用于区分不同的后端接口
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
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign({ onClick: (__VLS_ctx.handleOnClick) }, { style: ((__VLS_ctx.style)) }), { class: ("editable-cell") }));
    if (__VLS_ctx.isEdit && __VLS_ctx.enableAutoComplete) {
        const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NAutoComplete;
        /** @type { [typeof __VLS_components.NAutoComplete, typeof __VLS_components.nAutoComplete, ] } */
        // @ts-ignore
        const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign(Object.assign(Object.assign(Object.assign(Object.assign(Object.assign({ 'onUpdate:value': {} }, { 'onBlur': {} }), { 'onSearch': {} }), { 'onKeyup': {} }), { ref: ("inputRef"), value: ((__VLS_ctx.inputValue)), options: ((__VLS_ctx.options)), loading: ((__VLS_ctx.isLoading)) }), { style: ((__VLS_ctx.inputStyle)) }), { clearable: (true) })));
        const __VLS_2 = __VLS_1(Object.assign(Object.assign(Object.assign(Object.assign(Object.assign(Object.assign({ 'onUpdate:value': {} }, { 'onBlur': {} }), { 'onSearch': {} }), { 'onKeyup': {} }), { ref: ("inputRef"), value: ((__VLS_ctx.inputValue)), options: ((__VLS_ctx.options)), loading: ((__VLS_ctx.isLoading)) }), { style: ((__VLS_ctx.inputStyle)) }), { clearable: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_1));
        // @ts-ignore navigation for `const inputRef = ref()`
        __VLS_ctx.inputRef;
        var __VLS_6 = {};
        let __VLS_7;
        const __VLS_8 = {
            'onUpdate:value': (__VLS_ctx.handleUpdateValue)
        };
        const __VLS_9 = {
            onBlur: (__VLS_ctx.handleChange)
        };
        const __VLS_10 = {
            onSearch: (__VLS_ctx.handleSearch)
        };
        const __VLS_11 = {
            onKeyup: (__VLS_ctx.handleChange)
        };
        let __VLS_3;
        let __VLS_4;
        var __VLS_5;
    }
    else {
        __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)({});
        (__VLS_ctx.value);
    }
    __VLS_styleScopedClasses['editable-cell'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "inputRef": __VLS_6,
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
        return Object.assign(Object.assign({ $props: __VLS_makeOptional(props) }, props), { NAutoComplete: naive_ui_1.NAutoComplete, isEdit: isEdit, inputRef: inputRef, inputValue: inputValue, options: options, isLoading: isLoading, style: style, inputStyle: inputStyle, handleOnClick: handleOnClick, handleUpdateValue: handleUpdateValue, handleChange: handleChange, handleSearch: handleSearch });
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return Object.assign({ $props: __VLS_makeOptional(props) }, props);
    },
    __typeEl: {},
});
; /* PartiallyEnd: #4569/main.vue */
