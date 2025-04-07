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
const useSiderStore_1 = require("../../stores/useSiderStore");
const GuaxiSidebarErrorView_vue_1 = __importDefault(require("./GuaxiSidebarErrorView.vue"));
const GuaxiCenterErrorView_vue_1 = __importDefault(require("./GuaxiCenterErrorView.vue"));
const GuaxiRightContentErrorView_vue_1 = __importDefault(require("./GuaxiRightContentErrorView.vue"));
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const leftSiderbarWidth = (0, vue_1.ref)(350); // 默认宽度
const leftSidebarresizing = (0, vue_1.ref)(false);
const leftSidebarStartX = (0, vue_1.ref)(0);
const leftSidebarStartWidth = (0, vue_1.ref)(0);
const rightSiderbarWidth = (0, vue_1.ref)(650); // 默认宽度
const rightSidebarresizing = (0, vue_1.ref)(false);
const rightSidebarStartX = (0, vue_1.ref)(0);
const rightSidebarStartWidth = (0, vue_1.ref)(0);
const siderStore = (0, useSiderStore_1.useSiderStore)();
const isLeftSiderCollapsed = (0, vue_1.computed)(() => siderStore.isLeftSiderCollapsed);
const isRightSiderCollapsed = (0, vue_1.computed)(() => siderStore.isRightSiderCollapsed);
const toggleLeftSider = () => {
    siderStore.toggleLeftSider();
};
const toggleRightSider = () => {
    siderStore.toggleRightSider();
};
const onMouseDownLeftSidebar = (event) => {
    leftSidebarresizing.value = true;
    leftSidebarStartX.value = event.clientX;
    leftSidebarStartWidth.value = leftSiderbarWidth.value;
    // Attach mousemove and mouseup listeners
    document.addEventListener("mousemove", onMouseMoveLeftSidebar);
    document.addEventListener("mouseup", onMouseUpLeftSidebar);
};
const onMouseMoveLeftSidebar = (event) => {
    if (!leftSidebarresizing.value)
        return;
    const delta = event.clientX - leftSidebarStartX.value;
    leftSiderbarWidth.value = Math.max(250, leftSidebarStartWidth.value + delta); // 设置最小宽度为200px
};
const onMouseUpLeftSidebar = () => {
    leftSidebarresizing.value = false;
    // Remove mousemove and mouseup listeners
    document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
    document.removeEventListener("mouseup", onMouseUpLeftSidebar);
};
const onMouseDownRightSidebar = (event) => {
    console.log("onMouseDownRightSidebar");
    rightSidebarresizing.value = true;
    rightSidebarStartX.value = event.clientX;
    rightSidebarStartWidth.value = rightSiderbarWidth.value;
    // Attach mousemove and mouseup listeners
    document.addEventListener("mousemove", onMouseMoveRightSidebar);
    document.addEventListener("mouseup", onMouseUpRightSidebar);
};
const onMouseMoveRightSidebar = (event) => {
    if (!rightSidebarresizing.value)
        return;
    const delta = rightSidebarStartX.value - event.clientX;
    rightSiderbarWidth.value = Math.max(250, rightSidebarStartWidth.value + delta); // 设置最小宽度为200px
};
const onMouseUpRightSidebar = () => {
    rightSidebarresizing.value = false;
    // Remove mousemove and mouseup listeners
    document.removeEventListener("mousemove", onMouseMoveRightSidebar);
    document.removeEventListener("mouseup", onMouseUpRightSidebar);
};
// Cleanup listeners when the component is unmounted
(0, vue_1.onBeforeUnmount)(() => {
    document.removeEventListener("mousemove", onMouseMoveLeftSidebar);
    document.removeEventListener("mouseup", onMouseUpLeftSidebar);
    document.removeEventListener("mousemove", onMouseMoveRightSidebar);
    document.removeEventListener("mouseup", onMouseUpRightSidebar);
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
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("guaxi-content") }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ vertical: (true), size: ("large"), }));
    const __VLS_2 = __VLS_1({ vertical: (true), size: ("large"), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NLayout;
    /** @type { [typeof __VLS_components.NLayout, typeof __VLS_components.nLayout, typeof __VLS_components.NLayout, typeof __VLS_components.nLayout, ] } */
    // @ts-ignore
    const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6({ hasSider: (true), }));
    const __VLS_8 = __VLS_7({ hasSider: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_7));
    const __VLS_12 = __VLS_resolvedLocalAndGlobalComponents.NLayoutSider;
    /** @type { [typeof __VLS_components.NLayoutSider, typeof __VLS_components.nLayoutSider, typeof __VLS_components.NLayoutSider, typeof __VLS_components.nLayoutSider, ] } */
    // @ts-ignore
    const __VLS_13 = __VLS_asFunctionalComponent(__VLS_12, new __VLS_12({ collapsed: ((__VLS_ctx.isLeftSiderCollapsed)), width: ((__VLS_ctx.leftSiderbarWidth)), collapseMode: ("width"), collapsedWidth: ((0)), bordered: (true), contentStyle: ("padding: 0px;"), }));
    const __VLS_14 = __VLS_13({ collapsed: ((__VLS_ctx.isLeftSiderCollapsed)), width: ((__VLS_ctx.leftSiderbarWidth)), collapseMode: ("width"), collapsedWidth: ((0)), bordered: (true), contentStyle: ("padding: 0px;"), }, ...__VLS_functionalComponentArgsRest(__VLS_13));
    // @ts-ignore
    [GuaxiSidebarErrorView_vue_1.default,];
    // @ts-ignore
    const __VLS_18 = __VLS_asFunctionalComponent(GuaxiSidebarErrorView_vue_1.default, new GuaxiSidebarErrorView_vue_1.default({}));
    const __VLS_19 = __VLS_18({}, ...__VLS_functionalComponentArgsRest(__VLS_18));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onMousedown: (__VLS_ctx.onMouseDownLeftSidebar) }, { class: ("sider-resize-handle") }));
    __VLS_nonNullable(__VLS_17.slots).default;
    var __VLS_17;
    // @ts-ignore
    [GuaxiCenterErrorView_vue_1.default,];
    // @ts-ignore
    const __VLS_23 = __VLS_asFunctionalComponent(GuaxiCenterErrorView_vue_1.default, new GuaxiCenterErrorView_vue_1.default({}));
    const __VLS_24 = __VLS_23({}, ...__VLS_functionalComponentArgsRest(__VLS_23));
    const __VLS_28 = __VLS_resolvedLocalAndGlobalComponents.NLayout;
    /** @type { [typeof __VLS_components.NLayout, typeof __VLS_components.nLayout, typeof __VLS_components.NLayout, typeof __VLS_components.nLayout, ] } */
    // @ts-ignore
    const __VLS_29 = __VLS_asFunctionalComponent(__VLS_28, new __VLS_28({ hasSider: (true), siderPlacement: ("right"), }));
    const __VLS_30 = __VLS_29({ hasSider: (true), siderPlacement: ("right"), }, ...__VLS_functionalComponentArgsRest(__VLS_29));
    const __VLS_34 = __VLS_resolvedLocalAndGlobalComponents.NLayoutSider;
    /** @type { [typeof __VLS_components.NLayoutSider, typeof __VLS_components.nLayoutSider, typeof __VLS_components.NLayoutSider, typeof __VLS_components.nLayoutSider, ] } */
    // @ts-ignore
    const __VLS_35 = __VLS_asFunctionalComponent(__VLS_34, new __VLS_34({ collapsed: ((__VLS_ctx.isRightSiderCollapsed)), width: ((__VLS_ctx.rightSiderbarWidth)), collapseMode: ("width"), collapsedWidth: ((0)), bordered: (true), contentStyle: ("padding: 0px;"), }));
    const __VLS_36 = __VLS_35({ collapsed: ((__VLS_ctx.isRightSiderCollapsed)), width: ((__VLS_ctx.rightSiderbarWidth)), collapseMode: ("width"), collapsedWidth: ((0)), bordered: (true), contentStyle: ("padding: 0px;"), }, ...__VLS_functionalComponentArgsRest(__VLS_35));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onMousedown: (__VLS_ctx.onMouseDownRightSidebar) }, { class: ("sider-resize-handle-right") }));
    // @ts-ignore
    [GuaxiRightContentErrorView_vue_1.default,];
    // @ts-ignore
    const __VLS_40 = __VLS_asFunctionalComponent(GuaxiRightContentErrorView_vue_1.default, new GuaxiRightContentErrorView_vue_1.default({}));
    const __VLS_41 = __VLS_40({}, ...__VLS_functionalComponentArgsRest(__VLS_40));
    __VLS_nonNullable(__VLS_39.slots).default;
    var __VLS_39;
    __VLS_nonNullable(__VLS_33.slots).default;
    var __VLS_33;
    __VLS_nonNullable(__VLS_11.slots).default;
    var __VLS_11;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    __VLS_styleScopedClasses['guaxi-content'];
    __VLS_styleScopedClasses['sider-resize-handle'];
    __VLS_styleScopedClasses['sider-resize-handle-right'];
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
            GuaxiSidebarErrorView: GuaxiSidebarErrorView_vue_1.default,
            GuaxiCenterErrorView: GuaxiCenterErrorView_vue_1.default,
            GuaxiRightContentErrorView: GuaxiRightContentErrorView_vue_1.default,
            leftSiderbarWidth: leftSiderbarWidth,
            rightSiderbarWidth: rightSiderbarWidth,
            isLeftSiderCollapsed: isLeftSiderCollapsed,
            isRightSiderCollapsed: isRightSiderCollapsed,
            onMouseDownLeftSidebar: onMouseDownLeftSidebar,
            onMouseDownRightSidebar: onMouseDownRightSidebar,
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
