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
Object.defineProperty(exports, "__esModule", { value: true });
const ionicons5_1 = require("@vicons/ionicons5"); // 引入服务器图标
const naive_ui_1 = require("naive-ui");
const vue_1 = require("vue");
const repoStore_1 = require("../../stores/repoStore");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
// 渲染图标
function renderIcon(icon) {
    return () => (0, vue_1.h)(naive_ui_1.NIcon, null, { default: () => (0, vue_1.h)(icon) });
}
// 菜单选项
const menuOptions = [
    {
        label: '文件数据库',
        key: 'importFileDB',
        icon: renderIcon(ionicons5_1.ServerOutline)
    },
    {
        label: '笔记数据库',
        key: 'NotesDb',
        icon: renderIcon(ionicons5_1.ServerOutline)
    },
    {
        label: '数据库 3',
        key: 'db3',
        icon: renderIcon(ionicons5_1.ServerOutline)
    }
];
const message = (0, naive_ui_1.useMessage)(); // 使用消息提示
const store = (0, repoStore_1.repoStore)(); // 使用 Pinia store
// 处理菜单项点击事件
function handleUpdateValue(key, item) {
    message.info(`切换到数据库: ${key}`);
    store.currentDatabase = key; // 更新当前数据库
    store.fetchFiles(key); // 根据 key 获取对应的数据
}
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
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NInput;
    /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ round: (true), size: ("small"), placeholder: ("搜索"), }));
    const __VLS_2 = __VLS_1({ round: (true), size: ("small"), placeholder: ("搜索"), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NMenu;
    /** @type { [typeof __VLS_components.NMenu, typeof __VLS_components.nMenu, ] } */
    // @ts-ignore
    const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6(Object.assign({ 'onUpdate:value': {} }, { options: ((__VLS_ctx.menuOptions)) })));
    const __VLS_8 = __VLS_7(Object.assign({ 'onUpdate:value': {} }, { options: ((__VLS_ctx.menuOptions)) }), ...__VLS_functionalComponentArgsRest(__VLS_7));
    let __VLS_12;
    const __VLS_13 = {
        'onUpdate:value': (__VLS_ctx.handleUpdateValue)
    };
    let __VLS_9;
    let __VLS_10;
    var __VLS_11;
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
            menuOptions: menuOptions,
            handleUpdateValue: handleUpdateValue,
        };
    },
});
exports.default = (await Promise.resolve().then(() => __importStar(require('vue')))).defineComponent({
    setup() {
        return {};
    },
});
; /* PartiallyEnd: #4569/main.vue */
