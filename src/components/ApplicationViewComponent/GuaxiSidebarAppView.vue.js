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
const GuaxiSidbarTreeAppView_vue_1 = __importDefault(require("./GuaxiSidbarTreeAppView.vue"));
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const navigateTo = (path) => {
    window.location.hash = path; // 修改 hash，实现导航
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
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("sider-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("sider-icon-container") }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_2 = __VLS_1(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    let __VLS_6;
    const __VLS_7 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/');
        }
    };
    let __VLS_3;
    let __VLS_4;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)({ xmlns: ("http://www.w3.org/2000/svg"), "xmlns:xlink": ("http://www.w3.org/1999/xlink"), viewBox: ("0 0 24 24"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.g, __VLS_intrinsicElements.g)({ fill: ("none"), stroke: ("currentColor"), "stroke-width": ("2"), "stroke-linecap": ("round"), "stroke-linejoin": ("round"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M5 12H3l9-9l9 9h-2"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M5 12v7a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-7"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M9 21v-6a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v6"), });
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
    const __VLS_8 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_9 = __VLS_asFunctionalComponent(__VLS_8, new __VLS_8(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_10 = __VLS_9(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_9));
    let __VLS_14;
    const __VLS_15 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/chat');
        }
    };
    let __VLS_11;
    let __VLS_12;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735469078531") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("11424"), width: ("54"), height: ("54") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M919.192216 976.840649a42.620541 42.620541 0 0 1-21.919135-6.088649l-185.094919-110.675027A560.95827 560.95827 0 0 1 512 896.249081c-274.681081 0-498.162162-192.982486-498.162162-430.190703C13.837838 228.850162 237.318919 35.867676 512 35.867676S1010.162162 228.850162 1010.162162 466.058378c0 104.64173-42.952649 203.637622-121.66227 281.821406l70.379243 168.683243c7.195676 17.269622 2.601514 37.251459-11.374703 49.567135-8.025946 7.084973-18.127568 10.710486-28.312216 10.710487z m-203.277838-208.45319c7.610811 0 15.193946 2.048 21.919136 6.088649l91.108324 54.438054-31.494919-75.443892a43.699892 43.699892 0 0 1 11.623784-49.816216c74.170811-64.345946 115.020108-148.729081 115.020108-237.595676C924.090811 276.756757 739.217297 122.713946 512 122.713946S99.909189 276.756757 99.909189 466.058378c0 189.301622 184.873514 343.344432 412.090811 343.344433 65.785081 0 128.719568-12.647784 187.142919-37.583568 5.369081-2.297081 11.07027-3.431784 16.771459-3.431784zM260.953946 470.154378a56.32 56.32 0 0 1 56.347676-56.015567 56.347676 56.347676 0 0 1 55.794162 56.596757c0 31.135135-24.908108 56.430703-55.794162 56.569081A56.32 56.32 0 0 1 260.981622 471.316757v-1.134703z m186.478703 0c0 31.965405 25.710703 57.897514 57.399351 57.897514a57.648432 57.648432 0 0 0 57.371676-57.897514 57.648432 57.648432 0 0 0-57.371676-57.897513 57.648432 57.648432 0 0 0-57.399351 57.897513z m186.506378 0a56.32 56.32 0 0 1 56.347676-56.015567 56.347676 56.347676 0 0 1 55.794162 56.596757c0 31.135135-24.908108 56.430703-55.794162 56.569081a56.32 56.32 0 0 1-56.347676-56.015568v-1.134703z"), fill: ("#000000"), "p-id": ("11425"), });
    __VLS_nonNullable(__VLS_13.slots).default;
    var __VLS_13;
    const __VLS_16 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_17 = __VLS_asFunctionalComponent(__VLS_16, new __VLS_16(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_18 = __VLS_17(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_17));
    let __VLS_22;
    const __VLS_23 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/repository');
        }
    };
    let __VLS_19;
    let __VLS_20;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1741272321151") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("44336"), width: ("32"), height: ("32") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M870.4 57.6C780.8 19.2 652.8 0 512 0 371.2 0 243.2 19.2 153.6 57.6 51.2 102.4 0 153.6 0 211.2v595.2C0 864 51.2 921.6 153.6 960c89.6 44.8 217.6 64 358.4 64 140.8 0 268.8-19.2 358.4-57.6 96-38.4 153.6-96 153.6-153.6V211.2c0-57.6-51.2-108.8-153.6-153.6zM812.8 320c-83.2 32-198.4 44.8-300.8 44.8-108.8 0-217.6-12.8-300.8-44.8-96-25.6-140.8-64-140.8-108.8 0-38.4 51.2-76.8 140.8-108.8C294.4 76.8 403.2 64 512 64c102.4 0 217.6 19.2 300.8 44.8 89.6 32 140.8 70.4 140.8 108.8 0 38.4-44.8 76.8-140.8 102.4z m6.4 185.6C736 531.2 620.8 550.4 512 550.4c-108.8 0-217.6-19.2-307.2-44.8C115.2 473.6 64 435.2 64 396.8v-70.4c64 25.6 108.8 57.6 179.2 70.4 83.2 19.2 172.8 32 268.8 32s185.6-12.8 268.8-32C851.2 384 896 352 960 326.4v76.8c0 32-51.2 70.4-140.8 102.4z m0 204.8C736 736 620.8 755.2 512 755.2c-108.8 0-217.6-19.2-307.2-44.8C115.2 684.8 64 646.4 64 601.6v-96c64 32 108.8 57.6 179.2 76.8 83.2 19.2 172.8 32 268.8 32s185.6-12.8 268.8-32c70.4-19.2 115.2-44.8 179.2-76.8v96c0 44.8-51.2 83.2-140.8 108.8zM512 960c-108.8 0-217.6-19.2-307.2-44.8C115.2 889.6 64 851.2 64 812.8v-96c64 32 108.8 57.6 179.2 76.8 76.8 19.2 172.8 32 262.4 32 96 0 185.6-12.8 268.8-32 76.8-19.2 121.6-44.8 185.6-76.8v96c0 38.4-51.2 76.8-140.8 108.8C736 947.2 614.4 960 512 960z m0 0"), "p-id": ("44337"), fill: ("#000000"), });
    __VLS_nonNullable(__VLS_21.slots).default;
    var __VLS_21;
    const __VLS_24 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_25 = __VLS_asFunctionalComponent(__VLS_24, new __VLS_24(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_26 = __VLS_25(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_25));
    let __VLS_30;
    const __VLS_31 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/statistics');
        }
    };
    let __VLS_27;
    let __VLS_28;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735455553189") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("3208"), width: ("54"), height: ("54") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M93.090909 930.909091h930.909091v93.090909H0V0h93.090909v930.909091z m93.090909-442.181818h93.090909v372.363636H186.181818V488.727273z m209.454546-139.636364h93.090909v512h-93.090909V349.090909z m209.454545 139.636364h93.090909v372.363636h-93.090909V488.727273z m209.454546-186.181818h93.090909v558.545454h-93.090909V302.545455z m-555.985455 15.45309l-51.665455-77.451636L442.181818 83.688727l205.381818 136.936728 182.597819-162.327273 61.858909 69.585454-236.311273 210.036364L442.181818 195.584l-183.621818 122.414545z"), fill: ("#333333"), "p-id": ("3209"), });
    __VLS_nonNullable(__VLS_29.slots).default;
    var __VLS_29;
    const __VLS_32 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_33 = __VLS_asFunctionalComponent(__VLS_32, new __VLS_32(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_34 = __VLS_33(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_33));
    let __VLS_38;
    const __VLS_39 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/application');
        }
    };
    let __VLS_35;
    let __VLS_36;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735460858811") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("9472"), width: ("54"), height: ("54") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M51.199097 870.402709V665.606322c0-56.554538 45.843656-102.407225 102.398194-102.407225h204.803612c56.556344 0 102.398194 45.852687 102.398194 102.407225v204.796387c0 56.556344-45.843656 102.398194-102.398194 102.398194H153.597291c-56.554538-0.001806-102.398194-45.843656-102.398194-102.398194z m332.799548 0V665.606322c0-14.139086-11.45685-25.604967-25.595936-25.604967H153.599097c-14.139086 0-25.59413 11.465881-25.59413 25.604967v204.796387c0 14.139086 11.45685 25.595936 25.59413 25.595936h204.803612c14.139086 0 25.595936-11.45685 25.595936-25.595936zM870.402709 895.998645H665.597291c-14.13728 0-25.59413-11.45685-25.59413-25.595936V665.606322c0-14.139086 11.45685-25.604967 25.59413-25.604967h204.805418c14.139086 0 25.59413 11.465881 25.59413 25.604967v204.796387c0 14.139086-11.45685 25.595936-25.59413 25.595936z m-204.807225 76.800452h204.805419c56.556344 0 102.398194-45.84185 102.398194-102.398194V665.604516c0-56.554538-45.843656-102.407225-102.398194-102.407225H665.595484c-56.552731 0-102.398194 45.852687-102.398193 102.407225v204.796387c0.001806 56.556344 45.845462 102.398194 102.398193 102.398194zM358.400903 383.998645H153.597291c-14.139086 0-25.59413-11.45685-25.59413-25.595936V153.606322c0-14.139086 11.45685-25.603161 25.59413-25.603161h204.803612c14.140892 0 25.595936 11.465881 25.595936 25.603161v204.796387c0.001806 14.139086-11.455044 25.595936-25.595936 25.595936zM153.597291 460.800903h204.803612c56.556344 0 102.398194-45.845462 102.398194-102.398194V153.606322c0-56.554538-45.843656-102.407225-102.398194-102.407225H153.597291c-56.554538 0-102.398194 45.852687-102.398194 102.407225v204.796387c0 56.552731 45.843656 102.398194 102.398194 102.398194zM930.912868 274.101028a1563837.2918 1563837.2918 0 0 1-144.813646 144.822677c-9.997425 9.9902-26.20102 9.9902-36.20025 0-48.274828-48.274828-96.549655-96.549655-144.813646-144.822677-9.999231-9.9902-9.999231-26.202826 0-36.20025 48.26399-48.274828 96.538818-96.540624 144.813646-144.813646 9.999231-9.999231 26.202826-9.999231 36.20025 0 48.274828 48.274828 96.549655 96.540624 144.813646 144.813646 9.999231 9.997425 9.999231 26.210051 0 36.20025z m-90.506045 199.11944c48.274828-48.265796 96.538818-96.538818 144.813645-144.813645 39.986086-39.986086 39.986086-104.820334 0-144.813646-48.274828-48.265796-96.538818-96.540624-144.813645-144.815451-39.995118-39.98428-104.827559-39.98428-144.813646 0-48.274828 48.274828-96.538818 96.549655-144.813645 144.815451-39.993311 39.995118-39.993311 104.829365 0 144.813646 48.274828 48.274828 96.538818 96.547849 144.813645 144.813645 39.986086 39.995118 104.818528 39.995118 144.813646 0z"), fill: ("#231815"), "p-id": ("9473"), });
    __VLS_nonNullable(__VLS_37.slots).default;
    var __VLS_37;
    const __VLS_40 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_41 = __VLS_asFunctionalComponent(__VLS_40, new __VLS_40(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_42 = __VLS_41(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_41));
    let __VLS_46;
    const __VLS_47 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/search');
        }
    };
    let __VLS_43;
    let __VLS_44;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735455405203") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("10538"), width: ("54"), height: ("54") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M383.36 524.842667L332.8 387.584a42.666667 42.666667 0 0 1 54.784-54.826667l137.301333 50.602667a42.666667 42.666667 0 0 1 15.445334 70.186667l-86.741334 86.698666a42.666667 42.666667 0 0 1-70.186666-15.36z"), fill: ("#FFA26D"), "p-id": ("10539"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M483.754667 570.453333l86.698666-86.698666a42.666667 42.666667 0 0 1 70.186667 15.402666l50.602667 137.301334a42.666667 42.666667 0 0 1-54.784 54.784l-137.301334-50.602667a42.666667 42.666667 0 0 1-15.402666-70.186667z"), fill: ("#000000"), "p-id": ("10540"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M981.333333 512c0 259.2-210.133333 469.333333-469.333333 469.333333S42.666667 771.2 42.666667 512 252.8 42.666667 512 42.666667s469.333333 210.133333 469.333333 469.333333z m-85.333333 0a384 384 0 1 0-768 0 384 384 0 0 0 768 0z"), fill: ("#000000"), "p-id": ("10541"), });
    __VLS_nonNullable(__VLS_45.slots).default;
    var __VLS_45;
    const __VLS_48 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_49 = __VLS_asFunctionalComponent(__VLS_48, new __VLS_48(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) })));
    const __VLS_50 = __VLS_49(Object.assign(Object.assign({ 'onClick': {} }, { size: ("24") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_49));
    let __VLS_54;
    const __VLS_55 = {
        onClick: (...[$event]) => {
            __VLS_ctx.navigateTo('/setting');
        }
    };
    let __VLS_51;
    let __VLS_52;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1735455601102") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("5062"), width: ("54"), height: ("54") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M513.08309 726.48892c-115.618713 0-209.986634-94.367921-209.986634-210.346817s94.007738-210.346817 209.986634-210.346816c115.618713 0 209.986634 94.367921 209.986634 210.346816s-94.007738 210.346817-209.986634 210.346817z m0-351.178333c-77.439325 0-140.831516 63.392191-140.831516 140.831516 0 77.799508 63.032008 140.831516 140.831516 140.831516 77.439325 0 140.831516-63.392191 140.831516-140.831516s-63.032008-140.831516-140.831516-140.831516z"), fill: ("#040000"), "p-id": ("5063"), });
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M592.323329 76.358776V122.462188c0 33.857193 20.170243 64.47274 51.506155 77.439324 10.085121 4.322195 20.890609 6.483292 32.056279 6.483293 22.33134 0 43.221949-8.64439 59.069996-24.492438l29.534999-29.534998 112.016883 112.377066-29.534998 29.534999c-24.132255 24.132255-30.97573 59.790362-18.009145 91.126275 12.966585 31.335913 43.221949 51.506155 77.079141 51.506155h44.662681v158.840662h-44.662681c-33.857193 0-64.112557 20.170243-77.079141 51.506155-12.966585 31.335913-5.762926 67.354203 18.009145 91.126275l29.534998 29.534998-112.016883 112.377067-29.534999-29.534998c-15.848048-15.848048-36.738656-24.492438-59.069996-24.492438-11.16567 0-21.610974 2.161097-32.056279 6.483292-31.335913 12.966585-51.506155 43.221949-51.506155 77.439325v44.66268h-158.480478v-44.66268c0-33.857193-20.170243-64.47274-51.506156-77.439325-10.085121-4.322195-20.890609-6.483292-32.056279-6.483292-22.33134 0-43.221949 8.64439-59.069996 24.492438l-29.534998 29.534998-112.016884-112.377067 29.534998-29.534998c24.132255-24.132255 30.97573-59.790362 18.009146-91.126275-12.966585-31.335913-43.221949-51.506155-77.079142-51.506155H75.46086v-158.840662h44.66268c33.857193 0 64.112557-20.170243 77.079142-51.506155 12.966585-31.335913 5.762926-67.354203-18.009146-91.126275l-29.534998-29.534999 112.016884-112.377066 29.534998 29.534998c15.848048 15.848048 36.738656 24.492438 59.069996 24.492438 11.16567 0 21.610974-2.161097 32.056279-6.483293 31.335913-12.966585 51.506155-43.221949 51.506156-77.439324V76.358776h158.480478z m0-69.155118h-158.480478C396.023645 7.203658 364.687733 42.501583 364.687733 80.680971V122.462188a14.497362 14.497362 0 0 1-14.407317 14.407316c-3.601829 0-7.563841-1.440732-10.085121-4.322195L310.660297 103.012311c-13.326767-13.326767-31.335913-20.170243-48.984875-20.170243s-35.297925 6.843475-48.984875 20.170243L100.673663 215.389377c-27.013718 27.013718-27.013718 70.956032 0 97.969751l29.534998 29.534998c3.962012 3.962012 5.402744 10.445304 3.241646 15.848048-2.161097 5.402744-7.563841 9.004573-13.326767 9.004572H78.342323c-38.179388 0-72.036581 31.335913-72.036581 69.155118v158.840662c0 38.179388 33.857193 69.155118 72.036581 69.155117h41.781217c5.762926 0 11.16567 3.601829 13.326767 9.004573 2.161097 5.402744 1.080549 11.525853-3.241646 15.848048l-29.534998 29.534998c-27.013718 27.013718-27.013718 70.956032 0 97.96975l112.016884 112.377067c13.326767 13.326767 31.335913 20.170243 48.984875 20.170242s35.297925-6.843475 48.984875-20.170242l29.534998-29.534998c2.881463-2.881463 6.483292-4.322195 10.085121-4.322195a14.497362 14.497362 0 0 1 14.407317 14.407316v41.781217c0 38.179388 30.97573 72.036581 69.155118 72.036581h158.480478c38.179388 0 69.155118-34.217376 69.155118-72.036581v-41.781217a14.497362 14.497362 0 0 1 14.407316-14.407316c3.601829 0 7.563841 1.440732 10.085121 4.322195l29.534999 29.534998c13.326767 13.326767 31.335913 20.170243 48.984875 20.170242s35.297925-6.843475 48.984875-20.170242l112.016883-112.377067c27.013718-27.013718 27.013718-70.956032 0-97.96975l-29.534998-29.534998c-3.962012-3.962012-5.402744-10.445304-3.241646-15.848048 2.161097-5.402744 7.563841-9.004573 13.326767-9.004573H948.184039c38.179388 0 72.036581-31.335913 72.036581-69.155117v-158.840662c0-38.179388-33.857193-69.155118-72.036581-69.155118h-41.781217c-5.762926 0-11.16567-3.601829-13.326767-9.004572-2.161097-5.402744-1.080549-11.525853 3.241646-15.848048l29.534998-29.534998c27.013718-27.013718 27.013718-70.956032 0-97.969751l-112.016883-112.377066C800.509048 89.685543 782.499903 82.842068 764.850941 82.842068s-35.297925 6.843475-48.984876 20.170243l-29.534998 29.534998c-2.881463 2.881463-6.483292 4.322195-10.085121 4.322195a14.497362 14.497362 0 0 1-14.407316-14.407316V80.680971C661.478447 42.501583 630.502717 7.203658 592.323329 7.203658"), fill: ("#040000"), "p-id": ("5064"), });
    __VLS_nonNullable(__VLS_53.slots).default;
    var __VLS_53;
    // @ts-ignore
    [GuaxiSidbarTreeAppView_vue_1.default,];
    // @ts-ignore
    const __VLS_56 = __VLS_asFunctionalComponent(GuaxiSidbarTreeAppView_vue_1.default, new GuaxiSidbarTreeAppView_vue_1.default({}));
    const __VLS_57 = __VLS_56({}, ...__VLS_functionalComponentArgsRest(__VLS_56));
    __VLS_styleScopedClasses['sider-container'];
    __VLS_styleScopedClasses['sider-icon-container'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['icon'];
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
            GuaxiSidbarTreeAppView: GuaxiSidbarTreeAppView_vue_1.default,
            navigateTo: navigateTo,
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
