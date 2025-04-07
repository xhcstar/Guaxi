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
const fluent_1 = require("@vicons/fluent");
const fileTreeStore_1 = require("../../stores/fileTreeStore");
const repoStore_1 = require("../../stores/repoStore");
const plugin_dialog_1 = require("@tauri-apps/plugin-dialog");
const core_1 = require("@tauri-apps/api/core");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const fileTreeStore = (0, fileTreeStore_1.useFileTreeStore)();
const store = (0, repoStore_1.repoStore)();
const newRepoPath = (0, vue_1.ref)('');
const showAddPathModal = (0, vue_1.ref)(false);
const activeRepoPath = (0, vue_1.ref)(null);
// 文件库根目录
const librariesRoot = (0, vue_1.ref)('');
// 打开文件浏览器选择文件库根目录
const handleOpenFileBrowserForLibraryRoot = () => __awaiter(void 0, void 0, void 0, function* () {
    try {
        // 打开文件浏览器选择文件夹
        const selectedPath = yield (0, plugin_dialog_1.open)({
            directory: true, // 选择文件夹
            multiple: false, // 单选
        });
        if (selectedPath) {
            // 将路径统一转换为 Linux 系统的正斜杠模式
            // const normalizedPath = selectedPath.replace(/\\/g, '/');
            yield (0, core_1.invoke)('set_libraries_root', { path: selectedPath }); // 设置文件库根目录
            librariesRoot.value = selectedPath; // 更新页面显示
            store.set_librariesRoot(selectedPath);
            console.log('文件库根目录已更新为:', store.librariesRoot);
        }
    }
    catch (error) {
        console.error('打开文件浏览器失败:', error);
    }
});
// 获取笔记库路径列表
const repopathlist = (0, vue_1.computed)(() => fileTreeStore.repopathlist);
// 加载激活的笔记库路径
(0, vue_1.onMounted)(() => __awaiter(void 0, void 0, void 0, function* () {
    activeRepoPath.value = yield (0, core_1.invoke)('get_active_repo_path');
    const paths = yield (0, core_1.invoke)('get_repopathlist');
    fileTreeStore.setRepopathlist(paths); // 更新 store 中的数据
    console.log('加载激活的笔记库路径：', fileTreeStore.repopathlist);
    librariesRoot.value = yield (0, core_1.invoke)('get_libraries_root'); // 加载文件库根目录
}));
// 激活笔记库路径
const handleActivate = (path) => __awaiter(void 0, void 0, void 0, function* () {
    yield (0, core_1.invoke)('set_active_repo_path', { path });
    activeRepoPath.value = path;
    fileTreeStore.setActiveRepoPath(path);
});
// 移除笔记库路径
const handleRemove = (path) => {
    fileTreeStore.removeRepoPath(path);
};
// 添加新的笔记库路径
const handleAdd = () => __awaiter(void 0, void 0, void 0, function* () {
    if (newRepoPath.value) {
        // 将路径统一转换为 Linux 系统的正斜杠模式
        // const normalizedPath = newRepoPath.value.replace(/\\/g, '/');
        fileTreeStore.addRepoPath(newRepoPath.value);
        newRepoPath.value = '';
        showAddPathModal.value = false;
    }
});
// 打开文件浏览器选择路径
const handleOpenFileBrowser = () => __awaiter(void 0, void 0, void 0, function* () {
    try {
        // 打开文件浏览器选择文件夹
        const selectedPath = yield (0, plugin_dialog_1.open)({
            directory: true, // 选择文件夹
            multiple: false, // 单选
        });
        if (selectedPath) {
            // 将路径统一转换为 Linux 系统的正斜杠模式
            // const normalizedPath = selectedPath.replace(/\\/g, '/');
            newRepoPath.value = selectedPath;
            showAddPathModal.value = true; // 显示模态框
        }
    }
    catch (error) {
        console.error('打开文件浏览器失败:', error);
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
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NCard;
    /** @type { [typeof __VLS_components.NCard, typeof __VLS_components.nCard, typeof __VLS_components.NCard, typeof __VLS_components.nCard, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({}));
    const __VLS_2 = __VLS_1({}, ...__VLS_functionalComponentArgsRest(__VLS_1));
    if (__VLS_ctx.activeRepoPath) {
        const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NAlert;
        /** @type { [typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, ] } */
        // @ts-ignore
        const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6(Object.assign({ type: ("info") }, { style: ({}) })));
        const __VLS_8 = __VLS_7(Object.assign({ type: ("info") }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_7));
        (__VLS_ctx.activeRepoPath);
        __VLS_nonNullable(__VLS_11.slots).default;
        var __VLS_11;
    }
    else {
        const __VLS_12 = __VLS_resolvedLocalAndGlobalComponents.NAlert;
        /** @type { [typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, ] } */
        // @ts-ignore
        const __VLS_13 = __VLS_asFunctionalComponent(__VLS_12, new __VLS_12(Object.assign({ type: ("warning") }, { style: ({}) })));
        const __VLS_14 = __VLS_13(Object.assign({ type: ("warning") }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_13));
        __VLS_nonNullable(__VLS_17.slots).default;
        var __VLS_17;
    }
    const __VLS_18 = __VLS_resolvedLocalAndGlobalComponents.NList;
    /** @type { [typeof __VLS_components.NList, typeof __VLS_components.nList, typeof __VLS_components.NList, typeof __VLS_components.nList, ] } */
    // @ts-ignore
    const __VLS_19 = __VLS_asFunctionalComponent(__VLS_18, new __VLS_18(Object.assign({ hoverable: (true) }, { style: ({}) })));
    const __VLS_20 = __VLS_19(Object.assign({ hoverable: (true) }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_19));
    for (const [path] of __VLS_getVForSourceType((__VLS_ctx.repopathlist))) {
        const __VLS_24 = __VLS_resolvedLocalAndGlobalComponents.NListItem;
        /** @type { [typeof __VLS_components.NListItem, typeof __VLS_components.nListItem, typeof __VLS_components.NListItem, typeof __VLS_components.nListItem, ] } */
        // @ts-ignore
        const __VLS_25 = __VLS_asFunctionalComponent(__VLS_24, new __VLS_24({ key: ((path)), }));
        const __VLS_26 = __VLS_25({ key: ((path)), }, ...__VLS_functionalComponentArgsRest(__VLS_25));
        __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
        {
            const { prefix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_29.slots);
            const __VLS_30 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
            /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
            // @ts-ignore
            const __VLS_31 = __VLS_asFunctionalComponent(__VLS_30, new __VLS_30({ component: ((__VLS_ctx.Folder20Regular)), size: ((24)), }));
            const __VLS_32 = __VLS_31({ component: ((__VLS_ctx.Folder20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_31));
        }
        const __VLS_36 = __VLS_resolvedLocalAndGlobalComponents.NText;
        /** @type { [typeof __VLS_components.NText, typeof __VLS_components.nText, typeof __VLS_components.NText, typeof __VLS_components.nText, ] } */
        // @ts-ignore
        const __VLS_37 = __VLS_asFunctionalComponent(__VLS_36, new __VLS_36({ strong: (true), }));
        const __VLS_38 = __VLS_37({ strong: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_37));
        (path);
        __VLS_nonNullable(__VLS_41.slots).default;
        var __VLS_41;
        __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
        {
            const { suffix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_29.slots);
            const __VLS_42 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
            /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
            // @ts-ignore
            const __VLS_43 = __VLS_asFunctionalComponent(__VLS_42, new __VLS_42({ wrap: ((false)), }));
            const __VLS_44 = __VLS_43({ wrap: ((false)), }, ...__VLS_functionalComponentArgsRest(__VLS_43));
            const __VLS_48 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
            /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
            // @ts-ignore
            const __VLS_49 = __VLS_asFunctionalComponent(__VLS_48, new __VLS_48({ trigger: ("hover"), }));
            const __VLS_50 = __VLS_49({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_49));
            __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
            {
                const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_53.slots);
                const __VLS_54 = __VLS_resolvedLocalAndGlobalComponents.NButton;
                /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
                // @ts-ignore
                const __VLS_55 = __VLS_asFunctionalComponent(__VLS_54, new __VLS_54(Object.assign({ 'onClick': {} }, { text: (true), disabled: ((path === __VLS_ctx.activeRepoPath)) })));
                const __VLS_56 = __VLS_55(Object.assign({ 'onClick': {} }, { text: (true), disabled: ((path === __VLS_ctx.activeRepoPath)) }), ...__VLS_functionalComponentArgsRest(__VLS_55));
                let __VLS_60;
                const __VLS_61 = {
                    onClick: (...[$event]) => {
                        __VLS_ctx.handleActivate(path);
                    }
                };
                let __VLS_57;
                let __VLS_58;
                if (path === __VLS_ctx.activeRepoPath) {
                    const __VLS_62 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
                    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
                    // @ts-ignore
                    const __VLS_63 = __VLS_asFunctionalComponent(__VLS_62, new __VLS_62(Object.assign({ component: ((__VLS_ctx.CheckmarkCircle20Regular)), size: ((24)) }, { style: ({}) })));
                    const __VLS_64 = __VLS_63(Object.assign({ component: ((__VLS_ctx.CheckmarkCircle20Regular)), size: ((24)) }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_63));
                }
                else {
                    const __VLS_68 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
                    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
                    // @ts-ignore
                    const __VLS_69 = __VLS_asFunctionalComponent(__VLS_68, new __VLS_68(Object.assign({ component: ((__VLS_ctx.CheckmarkCircle32Regular)), size: ((24)) }, { style: ({}) })));
                    const __VLS_70 = __VLS_69(Object.assign({ component: ((__VLS_ctx.CheckmarkCircle32Regular)), size: ((24)) }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_69));
                }
                __VLS_nonNullable(__VLS_59.slots).default;
                var __VLS_59;
            }
            (path === __VLS_ctx.activeRepoPath ? '已激活' : '激活');
            var __VLS_53;
            const __VLS_74 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
            /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
            // @ts-ignore
            const __VLS_75 = __VLS_asFunctionalComponent(__VLS_74, new __VLS_74({ trigger: ("hover"), }));
            const __VLS_76 = __VLS_75({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_75));
            __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
            {
                const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_79.slots);
                const __VLS_80 = __VLS_resolvedLocalAndGlobalComponents.NButton;
                /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
                // @ts-ignore
                const __VLS_81 = __VLS_asFunctionalComponent(__VLS_80, new __VLS_80(Object.assign({ 'onClick': {} }, { text: (true) })));
                const __VLS_82 = __VLS_81(Object.assign({ 'onClick': {} }, { text: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_81));
                let __VLS_86;
                const __VLS_87 = {
                    onClick: (...[$event]) => {
                        __VLS_ctx.handleRemove(path);
                    }
                };
                let __VLS_83;
                let __VLS_84;
                const __VLS_88 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
                /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
                // @ts-ignore
                const __VLS_89 = __VLS_asFunctionalComponent(__VLS_88, new __VLS_88({ component: ((__VLS_ctx.Delete20Regular)), size: ((24)), }));
                const __VLS_90 = __VLS_89({ component: ((__VLS_ctx.Delete20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_89));
                __VLS_nonNullable(__VLS_85.slots).default;
                var __VLS_85;
            }
            var __VLS_79;
            __VLS_nonNullable(__VLS_47.slots).default;
            var __VLS_47;
        }
        var __VLS_29;
    }
    __VLS_nonNullable(__VLS_23.slots).default;
    var __VLS_23;
    const __VLS_94 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
    /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
    // @ts-ignore
    const __VLS_95 = __VLS_asFunctionalComponent(__VLS_94, new __VLS_94({ trigger: ("hover"), }));
    const __VLS_96 = __VLS_95({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_95));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_99.slots);
        const __VLS_100 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_101 = __VLS_asFunctionalComponent(__VLS_100, new __VLS_100(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("add-path-button") })));
        const __VLS_102 = __VLS_101(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("add-path-button") }), ...__VLS_functionalComponentArgsRest(__VLS_101));
        let __VLS_106;
        const __VLS_107 = {
            onClick: (__VLS_ctx.handleOpenFileBrowser)
        };
        let __VLS_103;
        let __VLS_104;
        const __VLS_108 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_109 = __VLS_asFunctionalComponent(__VLS_108, new __VLS_108({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }));
        const __VLS_110 = __VLS_109({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_109));
        __VLS_nonNullable(__VLS_105.slots).default;
        var __VLS_105;
    }
    var __VLS_99;
    const __VLS_114 = __VLS_resolvedLocalAndGlobalComponents.NModal;
    /** @type { [typeof __VLS_components.NModal, typeof __VLS_components.nModal, typeof __VLS_components.NModal, typeof __VLS_components.nModal, ] } */
    // @ts-ignore
    const __VLS_115 = __VLS_asFunctionalComponent(__VLS_114, new __VLS_114({ show: ((__VLS_ctx.showAddPathModal)), }));
    const __VLS_116 = __VLS_115({ show: ((__VLS_ctx.showAddPathModal)), }, ...__VLS_functionalComponentArgsRest(__VLS_115));
    const __VLS_120 = __VLS_resolvedLocalAndGlobalComponents.NCard;
    /** @type { [typeof __VLS_components.NCard, typeof __VLS_components.nCard, typeof __VLS_components.NCard, typeof __VLS_components.nCard, ] } */
    // @ts-ignore
    const __VLS_121 = __VLS_asFunctionalComponent(__VLS_120, new __VLS_120(Object.assign({ style: ({}) }, { title: ("添加笔记库路径") })));
    const __VLS_122 = __VLS_121(Object.assign({ style: ({}) }, { title: ("添加笔记库路径") }), ...__VLS_functionalComponentArgsRest(__VLS_121));
    const __VLS_126 = __VLS_resolvedLocalAndGlobalComponents.NInput;
    /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
    // @ts-ignore
    const __VLS_127 = __VLS_asFunctionalComponent(__VLS_126, new __VLS_126({ value: ((__VLS_ctx.newRepoPath)), placeholder: ("输入新的笔记库路径"), clearable: (true), }));
    const __VLS_128 = __VLS_127({ value: ((__VLS_ctx.newRepoPath)), placeholder: ("输入新的笔记库路径"), clearable: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_127));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { footer: __VLS_thisSlot } = __VLS_nonNullable(__VLS_125.slots);
        const __VLS_132 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
        /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
        // @ts-ignore
        const __VLS_133 = __VLS_asFunctionalComponent(__VLS_132, new __VLS_132({ justify: ("end"), }));
        const __VLS_134 = __VLS_133({ justify: ("end"), }, ...__VLS_functionalComponentArgsRest(__VLS_133));
        const __VLS_138 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_139 = __VLS_asFunctionalComponent(__VLS_138, new __VLS_138(Object.assign({ 'onClick': {} })));
        const __VLS_140 = __VLS_139(Object.assign({ 'onClick': {} }), ...__VLS_functionalComponentArgsRest(__VLS_139));
        let __VLS_144;
        const __VLS_145 = {
            onClick: (...[$event]) => {
                __VLS_ctx.showAddPathModal = false;
            }
        };
        let __VLS_141;
        let __VLS_142;
        __VLS_nonNullable(__VLS_143.slots).default;
        var __VLS_143;
        const __VLS_146 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_147 = __VLS_asFunctionalComponent(__VLS_146, new __VLS_146(Object.assign({ 'onClick': {} }, { type: ("primary") })));
        const __VLS_148 = __VLS_147(Object.assign({ 'onClick': {} }, { type: ("primary") }), ...__VLS_functionalComponentArgsRest(__VLS_147));
        let __VLS_152;
        const __VLS_153 = {
            onClick: (__VLS_ctx.handleAdd)
        };
        let __VLS_149;
        let __VLS_150;
        __VLS_nonNullable(__VLS_151.slots).default;
        var __VLS_151;
        __VLS_nonNullable(__VLS_137.slots).default;
        var __VLS_137;
    }
    var __VLS_125;
    __VLS_nonNullable(__VLS_119.slots).default;
    var __VLS_119;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    const __VLS_154 = __VLS_resolvedLocalAndGlobalComponents.NCard;
    /** @type { [typeof __VLS_components.NCard, typeof __VLS_components.nCard, typeof __VLS_components.NCard, typeof __VLS_components.nCard, ] } */
    // @ts-ignore
    const __VLS_155 = __VLS_asFunctionalComponent(__VLS_154, new __VLS_154({}));
    const __VLS_156 = __VLS_155({}, ...__VLS_functionalComponentArgsRest(__VLS_155));
    if (__VLS_ctx.librariesRoot) {
        const __VLS_160 = __VLS_resolvedLocalAndGlobalComponents.NAlert;
        /** @type { [typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, ] } */
        // @ts-ignore
        const __VLS_161 = __VLS_asFunctionalComponent(__VLS_160, new __VLS_160(Object.assign({ type: ("info") }, { style: ({}) })));
        const __VLS_162 = __VLS_161(Object.assign({ type: ("info") }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_161));
        (__VLS_ctx.librariesRoot);
        __VLS_nonNullable(__VLS_165.slots).default;
        var __VLS_165;
    }
    else {
        const __VLS_166 = __VLS_resolvedLocalAndGlobalComponents.NAlert;
        /** @type { [typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, typeof __VLS_components.NAlert, typeof __VLS_components.nAlert, ] } */
        // @ts-ignore
        const __VLS_167 = __VLS_asFunctionalComponent(__VLS_166, new __VLS_166(Object.assign({ type: ("warning") }, { style: ({}) })));
        const __VLS_168 = __VLS_167(Object.assign({ type: ("warning") }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_167));
        __VLS_nonNullable(__VLS_171.slots).default;
        var __VLS_171;
    }
    const __VLS_172 = __VLS_resolvedLocalAndGlobalComponents.NList;
    /** @type { [typeof __VLS_components.NList, typeof __VLS_components.nList, typeof __VLS_components.NList, typeof __VLS_components.nList, ] } */
    // @ts-ignore
    const __VLS_173 = __VLS_asFunctionalComponent(__VLS_172, new __VLS_172(Object.assign({ hoverable: (true) }, { style: ({}) })));
    const __VLS_174 = __VLS_173(Object.assign({ hoverable: (true) }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_173));
    if (__VLS_ctx.librariesRoot) {
        const __VLS_178 = __VLS_resolvedLocalAndGlobalComponents.NListItem;
        /** @type { [typeof __VLS_components.NListItem, typeof __VLS_components.nListItem, typeof __VLS_components.NListItem, typeof __VLS_components.nListItem, ] } */
        // @ts-ignore
        const __VLS_179 = __VLS_asFunctionalComponent(__VLS_178, new __VLS_178({}));
        const __VLS_180 = __VLS_179({}, ...__VLS_functionalComponentArgsRest(__VLS_179));
        __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
        {
            const { prefix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_183.slots);
            const __VLS_184 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
            /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
            // @ts-ignore
            const __VLS_185 = __VLS_asFunctionalComponent(__VLS_184, new __VLS_184({ component: ((__VLS_ctx.Folder20Regular)), size: ((24)), }));
            const __VLS_186 = __VLS_185({ component: ((__VLS_ctx.Folder20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_185));
        }
        const __VLS_190 = __VLS_resolvedLocalAndGlobalComponents.NText;
        /** @type { [typeof __VLS_components.NText, typeof __VLS_components.nText, typeof __VLS_components.NText, typeof __VLS_components.nText, ] } */
        // @ts-ignore
        const __VLS_191 = __VLS_asFunctionalComponent(__VLS_190, new __VLS_190({ strong: (true), }));
        const __VLS_192 = __VLS_191({ strong: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_191));
        (__VLS_ctx.librariesRoot);
        __VLS_nonNullable(__VLS_195.slots).default;
        var __VLS_195;
        __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
        {
            const { suffix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_183.slots);
            const __VLS_196 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
            /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
            // @ts-ignore
            const __VLS_197 = __VLS_asFunctionalComponent(__VLS_196, new __VLS_196({ wrap: ((false)), }));
            const __VLS_198 = __VLS_197({ wrap: ((false)), }, ...__VLS_functionalComponentArgsRest(__VLS_197));
            const __VLS_202 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
            /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
            // @ts-ignore
            const __VLS_203 = __VLS_asFunctionalComponent(__VLS_202, new __VLS_202({ trigger: ("hover"), }));
            const __VLS_204 = __VLS_203({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_203));
            __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
            {
                const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_207.slots);
                const __VLS_208 = __VLS_resolvedLocalAndGlobalComponents.NButton;
                /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
                // @ts-ignore
                const __VLS_209 = __VLS_asFunctionalComponent(__VLS_208, new __VLS_208(Object.assign({ 'onClick': {} }, { text: (true) })));
                const __VLS_210 = __VLS_209(Object.assign({ 'onClick': {} }, { text: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_209));
                let __VLS_214;
                const __VLS_215 = {
                    onClick: (__VLS_ctx.handleOpenFileBrowserForLibraryRoot)
                };
                let __VLS_211;
                let __VLS_212;
                const __VLS_216 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
                /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
                // @ts-ignore
                const __VLS_217 = __VLS_asFunctionalComponent(__VLS_216, new __VLS_216({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }));
                const __VLS_218 = __VLS_217({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_217));
                __VLS_nonNullable(__VLS_213.slots).default;
                var __VLS_213;
            }
            var __VLS_207;
            __VLS_nonNullable(__VLS_201.slots).default;
            var __VLS_201;
        }
        var __VLS_183;
    }
    __VLS_nonNullable(__VLS_177.slots).default;
    var __VLS_177;
    const __VLS_222 = __VLS_resolvedLocalAndGlobalComponents.NTooltip;
    /** @type { [typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, typeof __VLS_components.NTooltip, typeof __VLS_components.nTooltip, ] } */
    // @ts-ignore
    const __VLS_223 = __VLS_asFunctionalComponent(__VLS_222, new __VLS_222({ trigger: ("hover"), }));
    const __VLS_224 = __VLS_223({ trigger: ("hover"), }, ...__VLS_functionalComponentArgsRest(__VLS_223));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { trigger: __VLS_thisSlot } = __VLS_nonNullable(__VLS_227.slots);
        const __VLS_228 = __VLS_resolvedLocalAndGlobalComponents.NButton;
        /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
        // @ts-ignore
        const __VLS_229 = __VLS_asFunctionalComponent(__VLS_228, new __VLS_228(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("add-path-button") })));
        const __VLS_230 = __VLS_229(Object.assign(Object.assign({ 'onClick': {} }, { text: (true) }), { class: ("add-path-button") }), ...__VLS_functionalComponentArgsRest(__VLS_229));
        let __VLS_234;
        const __VLS_235 = {
            onClick: (__VLS_ctx.handleOpenFileBrowserForLibraryRoot)
        };
        let __VLS_231;
        let __VLS_232;
        const __VLS_236 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_237 = __VLS_asFunctionalComponent(__VLS_236, new __VLS_236({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }));
        const __VLS_238 = __VLS_237({ component: ((__VLS_ctx.FolderAdd20Regular)), size: ((24)), }, ...__VLS_functionalComponentArgsRest(__VLS_237));
        __VLS_nonNullable(__VLS_233.slots).default;
        var __VLS_233;
    }
    (__VLS_ctx.librariesRoot ? '更改文件库根目录' : '添加文件库根目录');
    var __VLS_227;
    __VLS_nonNullable(__VLS_159.slots).default;
    var __VLS_159;
    __VLS_styleScopedClasses['add-path-button'];
    __VLS_styleScopedClasses['add-path-button'];
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
            NCard: naive_ui_1.NCard,
            NList: naive_ui_1.NList,
            NListItem: naive_ui_1.NListItem,
            NText: naive_ui_1.NText,
            NButton: naive_ui_1.NButton,
            NIcon: naive_ui_1.NIcon,
            NInput: naive_ui_1.NInput,
            NSpace: naive_ui_1.NSpace,
            NAlert: naive_ui_1.NAlert,
            NModal: naive_ui_1.NModal,
            NTooltip: naive_ui_1.NTooltip,
            FolderAdd20Regular: fluent_1.FolderAdd20Regular,
            CheckmarkCircle20Regular: fluent_1.CheckmarkCircle20Regular,
            CheckmarkCircle32Regular: fluent_1.CheckmarkCircle32Regular,
            Delete20Regular: fluent_1.Delete20Regular,
            Folder20Regular: fluent_1.Folder20Regular,
            newRepoPath: newRepoPath,
            showAddPathModal: showAddPathModal,
            activeRepoPath: activeRepoPath,
            librariesRoot: librariesRoot,
            handleOpenFileBrowserForLibraryRoot: handleOpenFileBrowserForLibraryRoot,
            repopathlist: repopathlist,
            handleActivate: handleActivate,
            handleRemove: handleRemove,
            handleAdd: handleAdd,
            handleOpenFileBrowser: handleOpenFileBrowser,
        };
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {};
    },
});
; /* PartiallyEnd: #4569/main.vue */
