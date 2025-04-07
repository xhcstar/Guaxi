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
const vue_1 = require("vue");
const naive_ui_1 = require("naive-ui");
const ionicons5_1 = require("@vicons/ionicons5");
const fluent_1 = require("@vicons/fluent");
const settingsStore_1 = require("../../stores/settingsStore");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const settingsStore = (0, settingsStore_1.useSettingsStore)();
const selectedMenu = (0, vue_1.ref)(settingsStore.selectedMenu);
// 侧边栏菜单选项
const menuOptions = [
    {
        label: '笔记库',
        key: 'noterepo',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.FolderAdd20Regular, size: 24 }),
    },
    {
        label: '外观',
        key: 'preference',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.Image28Regular, size: 24 }),
    },
    {
        label: '服务器',
        key: 'server',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: fluent_1.Server24Regular, size: 24 }),
    },
    {
        label: '关于',
        key: 'about',
        icon: () => (0, vue_1.h)(naive_ui_1.NIcon, { component: ionicons5_1.InformationCircleOutline, size: 24 }),
    },
];
// 处理菜单选择
const handleMenuSelect = (key) => {
    settingsStore.setSelectedMenu(key);
};
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
    __VLS_styleScopedClasses['custom-menu'];
    __VLS_styleScopedClasses['custom-menu'];
    __VLS_styleScopedClasses['n-menu-item'];
    __VLS_styleScopedClasses['custom-menu'];
    __VLS_styleScopedClasses['n-menu-item'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NMenu;
    /** @type { [typeof __VLS_components.NMenu, typeof __VLS_components.nMenu, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign(Object.assign({ 'onUpdate:value': {} }, { collapsedWidth: ((64)), collapsedIconSize: ((24)), options: ((__VLS_ctx.menuOptions)), value: ((__VLS_ctx.selectedMenu)), itemSpacing: ((12)) }), { class: ("custom-menu") })));
    const __VLS_2 = __VLS_1(Object.assign(Object.assign({ 'onUpdate:value': {} }, { collapsedWidth: ((64)), collapsedIconSize: ((24)), options: ((__VLS_ctx.menuOptions)), value: ((__VLS_ctx.selectedMenu)), itemSpacing: ((12)) }), { class: ("custom-menu") }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    var __VLS_6 = {};
    let __VLS_7;
    const __VLS_8 = {
        'onUpdate:value': (__VLS_ctx.handleMenuSelect)
    };
    let __VLS_3;
    let __VLS_4;
    var __VLS_5;
    __VLS_styleScopedClasses['custom-menu'];
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
            NMenu: naive_ui_1.NMenu,
            selectedMenu: selectedMenu,
            menuOptions: menuOptions,
            handleMenuSelect: handleMenuSelect,
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
