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
// import { routes } from '../router/index.js'
const GuaxiHomePage_vue_1 = __importDefault(require("../views/GuaxiHomePage.vue"));
const GuaxiChatPage_vue_1 = __importDefault(require("../views/GuaxiChatPage.vue"));
const GuaxiStatisticsPage_vue_1 = __importDefault(require("../views/GuaxiStatisticsPage.vue"));
const GuaxiApplicationPage_vue_1 = __importDefault(require("../views/GuaxiApplicationPage.vue"));
const GuaxiSearchPage_vue_1 = __importDefault(require("../views/GuaxiSearchPage.vue"));
const GuaxiSettingPage_vue_1 = __importDefault(require("../views/GuaxiSettingPage.vue"));
const GuaxiRepoPage_vue_1 = __importDefault(require("../views/GuaxiRepoPage.vue"));
const NotFound_vue_1 = __importDefault(require("../views/NotFound.vue"));
const naive_ui_1 = require("naive-ui");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const routes = {
    '/': GuaxiHomePage_vue_1.default,
    '/chat': GuaxiChatPage_vue_1.default,
    '/repository': GuaxiRepoPage_vue_1.default,
    '/statistics': GuaxiStatisticsPage_vue_1.default,
    '/application': GuaxiApplicationPage_vue_1.default,
    '/search': GuaxiSearchPage_vue_1.default,
    '/setting': GuaxiSettingPage_vue_1.default,
};
const currentPath = (0, vue_1.ref)(window.location.hash);
window.addEventListener('hashchange', () => {
    currentPath.value = window.location.hash;
});
const currentView = (0, vue_1.computed)(() => {
    return routes[currentPath.value.slice(1) || '/'] || NotFound_vue_1.default;
}); /* PartiallyEnd: #3632/scriptSetup.vue */
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
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NConfigProvider;
    /** @type { [typeof __VLS_components.NConfigProvider, typeof __VLS_components.nConfigProvider, typeof __VLS_components.NConfigProvider, typeof __VLS_components.nConfigProvider, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({}));
    const __VLS_2 = __VLS_1({}, ...__VLS_functionalComponentArgsRest(__VLS_1));
    var __VLS_6 = {};
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("app-layout") }));
    const __VLS_7 = ((__VLS_ctx.currentView));
    // @ts-ignore
    const __VLS_8 = __VLS_asFunctionalComponent(__VLS_7, new __VLS_7({}));
    const __VLS_9 = __VLS_8({}, ...__VLS_functionalComponentArgsRest(__VLS_8));
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    __VLS_styleScopedClasses['app-layout'];
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
            NConfigProvider: naive_ui_1.NConfigProvider,
            currentView: currentView,
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
