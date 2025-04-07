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
const useSiderStore_1 = require("../../stores/useSiderStore");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const siderStore = (0, useSiderStore_1.useSiderStore)();
const isLeftSiderCollapsed = (0, vue_1.computed)(() => siderStore.isLeftSiderCollapsed);
const isRightSiderCollapsed = (0, vue_1.computed)(() => siderStore.isRightSiderCollapsed);
const toggleLeftSider = () => {
    siderStore.toggleLeftSider();
};
const toggleRightSider = () => {
    siderStore.toggleRightSider();
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
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("guaxi-header") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ style: ({}) }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign(Object.assign({ 'onClick': {} }, { size: ("20") }), { style: ({}) })));
    const __VLS_2 = __VLS_1(Object.assign(Object.assign({ 'onClick': {} }, { size: ("20") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    let __VLS_6;
    const __VLS_7 = {
        onClick: (__VLS_ctx.toggleLeftSider)
    };
    let __VLS_3;
    let __VLS_4;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)({ xmlns: ("http://www.w3.org/2000/svg"), "xmlns:xlink": ("http://www.w3.org/1999/xlink"), viewBox: ("0 0 24 24"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.g, __VLS_intrinsicElements.g)({ fill: ("none"), stroke: ("currentColor"), "stroke-width": ("2"), "stroke-linecap": ("round"), "stroke-linejoin": ("round"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.rect, __VLS_intrinsicElements.rect)({ x: ("4"), y: ("4"), width: ("16"), height: ("16"), rx: ("2"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M9 4v16"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M15 10l-2 2l2 2"), });
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ style: ({}) }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ style: ({}) }));
    const __VLS_8 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_9 = __VLS_asFunctionalComponent(__VLS_8, new __VLS_8(Object.assign(Object.assign({ class: ("top-right-navigation-icon") }, { size: ("20") }), { style: ({}) })));
    const __VLS_10 = __VLS_9(Object.assign(Object.assign({ class: ("top-right-navigation-icon") }, { size: ("20") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_9));
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735470281681") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("24213"), width: ("54"), height: ("54") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M469.333333 192A277.333333 277.333333 0 1 1 192 469.333333 277.76 277.76 0 0 1 469.333333 192M469.333333 128a341.333333 341.333333 0 1 0 341.333334 341.333333 341.333333 341.333333 0 0 0-341.333334-341.333333z"), fill: ("#444444"), "p-id": ("24214"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M341.333333 640a31.573333 31.573333 0 0 1-18.773333-5.973333 213.333333 213.333333 0 0 1-87.893333-175.36 32 32 0 0 1 64 0A160.853333 160.853333 0 0 0 308.053333 512a151.893333 151.893333 0 0 0 52.906667 68.266667 32 32 0 0 1 7.253333 44.8A32.853333 32.853333 0 0 1 341.333333 640z"), fill: ("#4E78FE"), "p-id": ("24215"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M705.28 673.28A31.573333 31.573333 0 0 0 682.666667 682.666667a32 32 0 0 0 0 45.226666l158.72 158.72a32 32 0 0 0 45.226666 0 32 32 0 0 0 0-45.226666L727.893333 682.666667a31.573333 31.573333 0 0 0-22.613333-9.386667z"), fill: ("#444444"), "p-id": ("24216"), });
    __VLS_nonNullable(__VLS_13.slots).default;
    var __VLS_13;
    const __VLS_14 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_15 = __VLS_asFunctionalComponent(__VLS_14, new __VLS_14(Object.assign(Object.assign({ class: ("top-right-navigation-icon") }, { size: ("20") }), { style: ({}) })));
    const __VLS_16 = __VLS_15(Object.assign(Object.assign({ class: ("top-right-navigation-icon") }, { size: ("20") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_15));
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735469990838") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("21231"), width: ("16"), height: ("16") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M966.485333 173.624889v638.008889c1.137778 16.952889 0.739556 33.962667-1.308444 50.801778a131.413333 131.413333 0 0 1-25.941333 57.287111c-18.830222 22.755556-45.511111 38.570667-75.377778 44.600889a234.211556 234.211556 0 0 1-49.208889 2.389333H209.521778a244.280889 244.280889 0 0 1-45.340445-1.706667c-29.866667-4.949333-56.888889-19.683556-76.458666-41.642666a131.754667 131.754667 0 0 1-28.216889-56.604445 277.617778 277.617778 0 0 1-1.991111-56.32V209.578667c-1.194667-19.399111-0.113778-38.855111 3.242666-58.026667 9.898667-40.163556 40.561778-72.874667 81.464889-87.04 23.324444-6.257778 47.616-8.760889 71.850667-7.168h484.295111l150.528 0.227556c17.521778 1.877333 34.531556 6.769778 50.176 14.392888 27.704889 14.791111 48.924444 38.4 60.017778 66.673778 2.730667 7.452444 4.721778 15.189333 6.087111 22.983111l1.308444 12.003556z m-71.907555 3.811555l-0.455111-5.006222-3.299556-12.003555a56.832 56.832 0 0 0-24.007111-27.079111 110.08 110.08 0 0 0-55.011556-7.68H223.232a225.052444 225.052444 0 0 0-51.2 1.706666 56.376889 56.376889 0 0 0-40.106667 36.408889c-2.673778 13.084444-3.527111 26.453333-2.503111 39.822222v610.417778c-1.137778 13.880889-0.682667 27.875556 1.251556 41.699556 2.673778 9.500444 7.566222 18.261333 14.392889 25.656889 14.791111 17.408 39.253333 17.009778 72.419555 17.009777h594.204445c13.255111 0.910222 26.624 0.341333 39.822222-1.706666a58.595556 58.595556 0 0 0 31.516444-19.911111c13.255111-17.066667 11.605333-35.669333 11.605334-64.910223V177.436444z m-85.902222 330.581334a32.085333 32.085333 0 0 1-26.737778 35.498666c-8.760889 0.967111-17.635556 1.137778-26.396445 0.455112H415.061333a32.995556 32.995556 0 0 1-29.866666-29.240889 32.256 32.256 0 0 1 24.291555-33.564445c9.272889-1.422222 18.659556-1.706667 27.989334-0.967111h320.284444c10.638222-0.910222 21.333333 0 31.573333 2.673778 7.964444 3.470222 14.165333 9.728 17.408 17.464889l1.934223 7.68z m-3.925334 206.392889a31.971556 31.971556 0 0 1-27.704889 33.792l-45.397333 0.227555H461.368889c-39.594667 0-70.200889 5.12-76.174222-28.046222a28.444444 28.444444 0 0 1 3.811555-18.432c12.344889-22.983111 42.211556-17.294222 77.141334-17.294222h254.862222c22.357333-2.503111 44.942222-1.137778 66.787555 4.096 7.281778 4.039111 12.743111 10.410667 15.36 17.976889l1.592889 7.68z m9.841778-410.851556a32.142222 32.142222 0 0 1-16.384 31.630222c-20.48 5.575111-41.927111 7.054222-63.032889 4.323556H456.647111c-38.286222 0-65.706667 3.584-71.395555-28.273778a28.444444 28.444444 0 0 1 3.754666-18.488889c11.548444-21.105778 35.726222-17.237333 68.380445-17.237333h290.531555c15.928889-1.536 32.028444-0.568889 47.672889 2.844444a32.938667 32.938667 0 0 1 17.123556 17.521778l1.877333 7.68z m-566.613333-41.358222c26.510222 0 48.014222 20.366222 48.014222 45.511111 0 25.144889-21.504 45.511111-48.014222 45.511111-26.567111 0-48.071111-20.366222-48.071111-45.511111a44.373333 44.373333 0 0 1 14.108444-32.199111 49.379556 49.379556 0 0 1 33.962667-13.312z m0 204.8c26.510222 0 48.014222 20.48 48.014222 45.568 0 25.144889-21.504 45.511111-48.014222 45.511111-26.567111 0-48.071111-20.366222-48.071111-45.511111a44.373333 44.373333 0 0 1 14.108444-32.199111 49.379556 49.379556 0 0 1 33.962667-13.312z m0 204.913778c26.510222 0 48.014222 20.366222 48.014222 45.511111 0 25.144889-21.504 45.511111-48.014222 45.511111-26.567111 0-48.071111-20.366222-48.071111-45.511111a44.373333 44.373333 0 0 1 14.108444-32.199111 49.379556 49.379556 0 0 1 33.962667-13.312z"), fill: ("#333333"), "p-id": ("21232"), });
    __VLS_nonNullable(__VLS_19.slots).default;
    var __VLS_19;
    const __VLS_20 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_21 = __VLS_asFunctionalComponent(__VLS_20, new __VLS_20(Object.assign(Object.assign(Object.assign({ 'onClick': {} }, { class: ("top-right-navigation-icon") }), { size: ("20") }), { style: ({}) })));
    const __VLS_22 = __VLS_21(Object.assign(Object.assign(Object.assign({ 'onClick': {} }, { class: ("top-right-navigation-icon") }), { size: ("20") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_21));
    let __VLS_26;
    const __VLS_27 = {
        onClick: (__VLS_ctx.toggleRightSider)
    };
    let __VLS_23;
    let __VLS_24;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)({ xmlns: ("http://www.w3.org/2000/svg"), "xmlns:xlink": ("http://www.w3.org/1999/xlink"), viewBox: ("0 0 24 24"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.g, __VLS_intrinsicElements.g)({ fill: ("none"), stroke: ("currentColor"), "stroke-width": ("2"), "stroke-linecap": ("round"), "stroke-linejoin": ("round"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.rect, __VLS_intrinsicElements.rect)({ x: ("4"), y: ("4"), width: ("16"), height: ("16"), rx: ("2"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M15 4v16"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M9 10l2 2l-2 2"), });
    __VLS_nonNullable(__VLS_25.slots).default;
    var __VLS_25;
    __VLS_styleScopedClasses['guaxi-header'];
    __VLS_styleScopedClasses['top-right-navigation-icon'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['top-right-navigation-icon'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['top-right-navigation-icon'];
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
            toggleLeftSider: toggleLeftSider,
            toggleRightSider: toggleRightSider,
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
