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
const fileProcessStore_1 = require("../../stores/fileProcessStore");
const plugin_dialog_1 = require("@tauri-apps/plugin-dialog");
const core_1 = require("@tauri-apps/api/core");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const fileProcessStore = (0, fileProcessStore_1.useFileProcessStore)();
const supportFileTypeList = fileProcessStore.supportFileType;
// 动态生成 options
const dropdownOptions = (0, vue_1.computed)(() => {
    const openFilelist = {
        label: "打开文件",
        key: "local",
        children: supportFileTypeList.map((filetype) => {
            return {
                label: `打开${filetype.type}`,
                key: filetype.id,
            };
        }),
    };
    console.log('openFilelist', openFilelist);
    return [openFilelist];
});
// 打开文件选择框的函数
const handleFileSelect = () => __awaiter(void 0, void 0, void 0, function* () {
    try {
        // 打开文件选择器并获取选择的文件路径
        const filePath = yield (0, plugin_dialog_1.open)({
            multiple: false, // 如果需要多选文件，可以将其设置为 true
            filters: [{
                    name: 'Text Files',
                    extensions: ['txt', 'md', 'docx', 'pdf'] // 文件过滤器，可以选择你想要的文件类型
                }],
        });
        // 打印所选择的文件路径
        if (filePath) {
            fileProcessStore.setSelectedFile(filePath);
            // 从文件路径中提取文件扩展名
            const fileExtension = filePath.split('.').pop().toLowerCase();
            // 根据文件扩展名设置 filetype
            let filetype;
            switch (fileExtension) {
                case 'pdf':
                    filetype = 'pdf';
                    break;
                case 'md':
                    filetype = 'md';
                    break;
                case 'txt':
                    filetype = 'txt';
                    break;
                case 'docx':
                    filetype = 'doc';
                    break;
                default:
                    filetype = 'unknown';
                    break;
            }
            try {
                const openfile_request = {
                    filetype: filetype,
                    filepath: fileProcessStore.selectedFile,
                    curpage: fileProcessStore.curentPage,
                    lastpage: fileProcessStore.lastPage,
                };
                console.log('openfile_request:', openfile_request);
                const response = yield (0, core_1.invoke)('openfile', { request: openfile_request });
                fileProcessStore.getCurentOpenedFileContent(response, 0);
                console.log('Response:', response);
            }
            catch (error) {
                console.error('Error calling openfile API:', error);
            }
        }
        else {
            console.log('未选择文件');
        }
    }
    catch (error) {
        console.error('打开文件选择框时发生错误:', error);
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
    let __VLS_resolvedLocalAndGlobalComponents;
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NDropdown;
    /** @type { [typeof __VLS_components.NDropdown, typeof __VLS_components.nDropdown, typeof __VLS_components.NDropdown, typeof __VLS_components.nDropdown, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign({ 'onSelect': {} }, { options: ((__VLS_ctx.dropdownOptions)), placement: ("bottom-start"), trigger: ("click") })));
    const __VLS_2 = __VLS_1(Object.assign({ 'onSelect': {} }, { options: ((__VLS_ctx.dropdownOptions)), placement: ("bottom-start"), trigger: ("click") }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    var __VLS_6 = {};
    let __VLS_7;
    const __VLS_8 = {
        onSelect: (__VLS_ctx.handleFileSelect)
    };
    let __VLS_3;
    let __VLS_4;
    const __VLS_9 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
    /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
    // @ts-ignore
    const __VLS_10 = __VLS_asFunctionalComponent(__VLS_9, new __VLS_9({ placement: ("top"), trigger: ("hover"), }));
    const __VLS_11 = __VLS_10({ placement: ("top"), trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_10));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_14.slots);
        const __VLS_15 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_16 = __VLS_asFunctionalComponent(__VLS_15, new __VLS_15(Object.assign({ size: ("18") }, { style: ({}) })));
        const __VLS_17 = __VLS_16(Object.assign({ size: ("18") }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_16));
        __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1740559605848") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("1912"), width: ("32"), height: ("32") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M859.5 193H446.939c-1.851-53.25-45.747-96-99.439-96h-183C109.635 97 65 141.635 65 196.5v632c0 54.864 44.635 99.5 99.5 99.5h695c54.864 0 99.5-44.636 99.5-99.5v-536c0-54.865-44.636-99.5-99.5-99.5zM896 828.5c0 20.126-16.374 36.5-36.5 36.5h-695c-20.126 0-36.5-16.374-36.5-36.5V384h768v444.5z m0-507.5H128V196.5c0-20.126 16.374-36.5 36.5-36.5h183c20.126 0 36.5 16.374 36.5 36.5v28c0 17.397 14.103 31.5 31.5 31.5h444c20.126 0 36.5 16.374 36.5 36.5V321z"), fill: ("#070102"), "p-id": ("1913"), });
        __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M384 640h95v96c0 17.674 14.327 32 32 32s32-14.326 32-32v-96h97c17.673 0 32-14.327 32-32s-14.327-32-32-32h-97v-96c0-17.673-14.327-32-32-32s-32 14.327-32 32v96h-95c-17.673 0-32 14.327-32 32s14.327 32 32 32z"), fill: ("#070102"), "p-id": ("1914"), });
        __VLS_nonNullable(__VLS_20.slots).default;
        var __VLS_20;
    }
    var __VLS_14;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    __VLS_styleScopedClasses['icon'];
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
        return {
            dropdownOptions: dropdownOptions,
            handleFileSelect: handleFileSelect,
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
