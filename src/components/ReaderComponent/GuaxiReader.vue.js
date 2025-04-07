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
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
// import GuaxiPDFReader from './GuaxiPDFReader.vue'
const GuaxiConvertPDFReader_vue_1 = __importDefault(require("./GuaxiConvertPDFReader.vue"));
const GuaxiMarkdownReader_vue_1 = __importDefault(require("./GuaxiMarkdownReader.vue"));
const GuaxiHtmlReader_vue_1 = __importDefault(require("./GuaxiHtmlReader.vue"));
const GuaxiEpubReader_vue_1 = __importDefault(require("./GuaxiEpubReader.vue"));
const GauxiWordReader_vue_1 = __importDefault(require("./GauxiWordReader.vue"));
const GuaxiTextReader_vue_1 = __importDefault(require("./GuaxiTextReader.vue"));
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
// import { Base64 } from 'js-base64'
const props = defineProps({
    filePath: {
        type: String,
        required: true,
    },
    fileType: {
        type: String,
        required: true,
        validator: (value) => ['pdf', 'md', 'html', 'epub', 'doc', 'txt'].includes(value)
    },
    fileName: {
        type: String,
        required: true,
    },
    library: {
        type: String,
        required: true,
    }
});
// Base64 解码文件路径
// const decodedFilePath = computed (() => {
//   try {
//     // 先URL解码，再Base64解码
//     const urlDecoded = decodeURIComponent(props.filePath)
// 	console.log('urlDecoded', urlDecoded)
//     return Base64.decode(urlDecoded)
//   } catch (error) {
//     console.error('路径解码失败:', error)
//     return null
//   }
// })
// 更严格的组件选择逻辑
const readerComponent = (0, vue_1.computed)(() => {
    const { fileType, filePath, fileName, library } = props;
    console.log('readerComponent', fileType, filePath, fileName, library);
    if (!fileType || !fileName) {
        console.error('Missing required props');
        return null;
    }
    const typeMap = {
        pdf: GuaxiConvertPDFReader_vue_1.default,
        md: GuaxiMarkdownReader_vue_1.default,
        markdown: GuaxiMarkdownReader_vue_1.default,
        html: GuaxiHtmlReader_vue_1.default,
        htm: GuaxiHtmlReader_vue_1.default,
        epub: GuaxiEpubReader_vue_1.default,
        doc: GauxiWordReader_vue_1.default,
        docx: GauxiWordReader_vue_1.default,
        word: GauxiWordReader_vue_1.default,
        txt: GuaxiTextReader_vue_1.default
    };
    return typeMap[fileType] || null;
});
const emit = defineEmits(['close']); /* PartiallyEnd: #3632/scriptSetup.vue */
const __VLS_fnComponent = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    props: {
        filePath: {
            type: String,
            required: true,
        },
        fileType: {
            type: String,
            required: true,
            validator: (value) => ['pdf', 'md', 'html', 'epub', 'doc', 'txt'].includes(value)
        },
        fileName: {
            type: String,
            required: true,
        },
        library: {
            type: String,
            required: true,
        }
    },
    emits: {},
});
;
let __VLS_functionalComponentProps;
function __VLS_template() {
    var _a;
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign(Object.assign({}, {}), {}), __VLS_ctx);
    let __VLS_components;
    const __VLS_localDirectives = Object.assign(Object.assign({}, {}), __VLS_ctx);
    let __VLS_directives;
    let __VLS_styleScopedClasses;
    __VLS_styleScopedClasses['unknown-type'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("guaxi-reader") }));
    if (__VLS_ctx.readerComponent) {
        const __VLS_0 = ((__VLS_ctx.readerComponent));
        // @ts-ignore
        const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign({ 'onClose': {} }, { filePath: ((__VLS_ctx.filePath)), fileType: ((__VLS_ctx.fileType)), fileName: ((__VLS_ctx.fileName)), library: ((__VLS_ctx.library)) })));
        const __VLS_2 = __VLS_1(Object.assign({ 'onClose': {} }, { filePath: ((__VLS_ctx.filePath)), fileType: ((__VLS_ctx.fileType)), fileName: ((__VLS_ctx.fileName)), library: ((__VLS_ctx.library)) }), ...__VLS_functionalComponentArgsRest(__VLS_1));
        let __VLS_6;
        const __VLS_7 = {
            onClose: (...[$event]) => {
                if (!((__VLS_ctx.readerComponent)))
                    return;
                __VLS_ctx.emit('close');
            }
        };
        let __VLS_3;
        let __VLS_4;
        var __VLS_5;
    }
    else {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("unknown-type") }));
        (__VLS_ctx.fileType);
        __VLS_elementAsFunction(__VLS_intrinsicElements.p, __VLS_intrinsicElements.p)({});
        (((_a = __VLS_ctx.fileData) === null || _a === void 0 ? void 0 : _a.name) || '未知');
    }
    __VLS_styleScopedClasses['guaxi-reader'];
    __VLS_styleScopedClasses['unknown-type'];
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {};
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
        return Object.assign(Object.assign({ $props: __VLS_makeOptional(props) }, props), { $emit: emit, readerComponent: readerComponent, emit: emit });
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return Object.assign(Object.assign({ $props: __VLS_makeOptional(props) }, props), { $emit: emit });
    },
    __typeEl: {},
});
; /* PartiallyEnd: #4569/main.vue */
