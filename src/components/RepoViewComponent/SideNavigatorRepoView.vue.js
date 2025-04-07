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
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const showAnchor = (0, vue_1.ref)(false); /* PartiallyEnd: #3632/scriptSetup.vue */
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
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)({});
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NButton;
    /** @type { [typeof __VLS_components.NButton, typeof __VLS_components.nButton, typeof __VLS_components.NButton, typeof __VLS_components.nButton, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign({ 'onClick': {} })));
    const __VLS_2 = __VLS_1(Object.assign({ 'onClick': {} }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    let __VLS_6;
    const __VLS_7 = {
        onClick: (...[$event]) => {
            __VLS_ctx.showAnchor = !__VLS_ctx.showAnchor;
        }
    };
    let __VLS_3;
    let __VLS_4;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    if (__VLS_ctx.showAnchor) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("document-scroll-container") }, { style: ({}) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ style: ({}) }));
        const __VLS_8 = __VLS_resolvedLocalAndGlobalComponents.NAnchor;
        /** @type { [typeof __VLS_components.NAnchor, typeof __VLS_components.nAnchor, typeof __VLS_components.NAnchor, typeof __VLS_components.nAnchor, ] } */
        // @ts-ignore
        const __VLS_9 = __VLS_asFunctionalComponent(__VLS_8, new __VLS_8(Object.assign(Object.assign({ affix: (true), listenTo: (".document-scroll-container"), triggerTop: ((24)), top: ((88)) }, { style: ({}) }), { bound: ((24)) })));
        const __VLS_10 = __VLS_9(Object.assign(Object.assign({ affix: (true), listenTo: (".document-scroll-container"), triggerTop: ((24)), top: ((88)) }, { style: ({}) }), { bound: ((24)) }), ...__VLS_functionalComponentArgsRest(__VLS_9));
        const __VLS_14 = __VLS_resolvedLocalAndGlobalComponents.NAnchorLink;
        /** @type { [typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, ] } */
        // @ts-ignore
        const __VLS_15 = __VLS_asFunctionalComponent(__VLS_14, new __VLS_14({ title: ("演示"), href: ("#演示"), }));
        const __VLS_16 = __VLS_15({ title: ("演示"), href: ("#演示"), }, ...__VLS_functionalComponentArgsRest(__VLS_15));
        const __VLS_20 = __VLS_resolvedLocalAndGlobalComponents.NAnchorLink;
        /** @type { [typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, ] } */
        // @ts-ignore
        const __VLS_21 = __VLS_asFunctionalComponent(__VLS_20, new __VLS_20({ title: ("基础用法"), href: ("#basic.vue"), }));
        const __VLS_22 = __VLS_21({ title: ("基础用法"), href: ("#basic.vue"), }, ...__VLS_functionalComponentArgsRest(__VLS_21));
        const __VLS_26 = __VLS_resolvedLocalAndGlobalComponents.NAnchorLink;
        /** @type { [typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, ] } */
        // @ts-ignore
        const __VLS_27 = __VLS_asFunctionalComponent(__VLS_26, new __VLS_26({ title: ("忽略间隔"), href: ("#ignore-gap.vue"), }));
        const __VLS_28 = __VLS_27({ title: ("忽略间隔"), href: ("#ignore-gap.vue"), }, ...__VLS_functionalComponentArgsRest(__VLS_27));
        const __VLS_32 = __VLS_resolvedLocalAndGlobalComponents.NAnchorLink;
        /** @type { [typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, ] } */
        // @ts-ignore
        const __VLS_33 = __VLS_asFunctionalComponent(__VLS_32, new __VLS_32({ title: ("固定"), href: ("#affix.vue"), }));
        const __VLS_34 = __VLS_33({ title: ("固定"), href: ("#affix.vue"), }, ...__VLS_functionalComponentArgsRest(__VLS_33));
        const __VLS_38 = __VLS_resolvedLocalAndGlobalComponents.NAnchorLink;
        /** @type { [typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, ] } */
        // @ts-ignore
        const __VLS_39 = __VLS_asFunctionalComponent(__VLS_38, new __VLS_38({ title: ("滚动到"), href: ("#scrollto.vue"), }));
        const __VLS_40 = __VLS_39({ title: ("滚动到"), href: ("#scrollto.vue"), }, ...__VLS_functionalComponentArgsRest(__VLS_39));
        __VLS_nonNullable(__VLS_19.slots).default;
        var __VLS_19;
        const __VLS_44 = __VLS_resolvedLocalAndGlobalComponents.NAnchorLink;
        /** @type { [typeof __VLS_components.NAnchorLink, typeof __VLS_components.nAnchorLink, ] } */
        // @ts-ignore
        const __VLS_45 = __VLS_asFunctionalComponent(__VLS_44, new __VLS_44({ title: ("API"), href: ("#API"), }));
        const __VLS_46 = __VLS_45({ title: ("API"), href: ("#API"), }, ...__VLS_functionalComponentArgsRest(__VLS_45));
        __VLS_nonNullable(__VLS_13.slots).default;
        var __VLS_13;
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ id: ("演示") }, { style: ({}) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ id: ("basic.vue") }, { style: ({}) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ id: ("ignore-gap.vue") }, { style: ({}) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ id: ("affix.vue") }, { style: ({}) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ id: ("scrollto.vue") }, { style: ({}) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ id: ("API") }, { style: ({}) }));
    }
    __VLS_styleScopedClasses['document-scroll-container'];
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
            showAnchor: showAnchor,
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
