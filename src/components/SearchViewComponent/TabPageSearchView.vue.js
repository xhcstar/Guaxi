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
const placement = (0, vue_1.ref)('left');
const type = (0, vue_1.ref)('card'); /* PartiallyEnd: #3632/scriptSetup.vue */
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
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NSpace;
    /** @type { [typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, typeof __VLS_components.NSpace, typeof __VLS_components.nSpace, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ vertical: (true), }));
    const __VLS_2 = __VLS_1({ vertical: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    var __VLS_6 = {};
    const __VLS_7 = __VLS_resolvedLocalAndGlobalComponents.NRadioGroup;
    /** @type { [typeof __VLS_components.NRadioGroup, typeof __VLS_components.nRadioGroup, typeof __VLS_components.NRadioGroup, typeof __VLS_components.nRadioGroup, ] } */
    // @ts-ignore
    const __VLS_8 = __VLS_asFunctionalComponent(__VLS_7, new __VLS_7({ value: ((__VLS_ctx.placement)), }));
    const __VLS_9 = __VLS_8({ value: ((__VLS_ctx.placement)), }, ...__VLS_functionalComponentArgsRest(__VLS_8));
    const __VLS_13 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_14 = __VLS_asFunctionalComponent(__VLS_13, new __VLS_13({ label: ("top"), value: ("top"), }));
    const __VLS_15 = __VLS_14({ label: ("top"), value: ("top"), }, ...__VLS_functionalComponentArgsRest(__VLS_14));
    const __VLS_19 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_20 = __VLS_asFunctionalComponent(__VLS_19, new __VLS_19({ label: ("bottom"), value: ("bottom"), }));
    const __VLS_21 = __VLS_20({ label: ("bottom"), value: ("bottom"), }, ...__VLS_functionalComponentArgsRest(__VLS_20));
    const __VLS_25 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_26 = __VLS_asFunctionalComponent(__VLS_25, new __VLS_25({ label: ("left"), value: ("left"), }));
    const __VLS_27 = __VLS_26({ label: ("left"), value: ("left"), }, ...__VLS_functionalComponentArgsRest(__VLS_26));
    const __VLS_31 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_32 = __VLS_asFunctionalComponent(__VLS_31, new __VLS_31({ label: ("right"), value: ("right"), }));
    const __VLS_33 = __VLS_32({ label: ("right"), value: ("right"), }, ...__VLS_functionalComponentArgsRest(__VLS_32));
    __VLS_nonNullable(__VLS_12.slots).default;
    var __VLS_12;
    const __VLS_37 = __VLS_resolvedLocalAndGlobalComponents.NRadioGroup;
    /** @type { [typeof __VLS_components.NRadioGroup, typeof __VLS_components.nRadioGroup, typeof __VLS_components.NRadioGroup, typeof __VLS_components.nRadioGroup, ] } */
    // @ts-ignore
    const __VLS_38 = __VLS_asFunctionalComponent(__VLS_37, new __VLS_37({ value: ((__VLS_ctx.type)), }));
    const __VLS_39 = __VLS_38({ value: ((__VLS_ctx.type)), }, ...__VLS_functionalComponentArgsRest(__VLS_38));
    const __VLS_43 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_44 = __VLS_asFunctionalComponent(__VLS_43, new __VLS_43({ label: ("card"), value: ("card"), }));
    const __VLS_45 = __VLS_44({ label: ("card"), value: ("card"), }, ...__VLS_functionalComponentArgsRest(__VLS_44));
    const __VLS_49 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_50 = __VLS_asFunctionalComponent(__VLS_49, new __VLS_49({ label: ("bar"), value: ("bar"), }));
    const __VLS_51 = __VLS_50({ label: ("bar"), value: ("bar"), }, ...__VLS_functionalComponentArgsRest(__VLS_50));
    const __VLS_55 = __VLS_resolvedLocalAndGlobalComponents.NRadio;
    /** @type { [typeof __VLS_components.NRadio, typeof __VLS_components.nRadio, ] } */
    // @ts-ignore
    const __VLS_56 = __VLS_asFunctionalComponent(__VLS_55, new __VLS_55({ label: ("line"), value: ("line"), }));
    const __VLS_57 = __VLS_56({ label: ("line"), value: ("line"), }, ...__VLS_functionalComponentArgsRest(__VLS_56));
    __VLS_nonNullable(__VLS_42.slots).default;
    var __VLS_42;
    const __VLS_61 = __VLS_resolvedLocalAndGlobalComponents.NTabs;
    /** @type { [typeof __VLS_components.NTabs, typeof __VLS_components.nTabs, typeof __VLS_components.NTabs, typeof __VLS_components.nTabs, ] } */
    // @ts-ignore
    const __VLS_62 = __VLS_asFunctionalComponent(__VLS_61, new __VLS_61(Object.assign({ key: ((__VLS_ctx.type + __VLS_ctx.placement)), addable: (true), type: ((__VLS_ctx.type)), animated: (true), placement: ((__VLS_ctx.placement)) }, { style: ((__VLS_ctx.placement === 'left' || __VLS_ctx.placement === 'right'
            ? { height: '240px' }
            : undefined)) })));
    const __VLS_63 = __VLS_62(Object.assign({ key: ((__VLS_ctx.type + __VLS_ctx.placement)), addable: (true), type: ((__VLS_ctx.type)), animated: (true), placement: ((__VLS_ctx.placement)) }, { style: ((__VLS_ctx.placement === 'left' || __VLS_ctx.placement === 'right'
            ? { height: '240px' }
            : undefined)) }), ...__VLS_functionalComponentArgsRest(__VLS_62));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { prefix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_66.slots);
    }
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { suffix: __VLS_thisSlot } = __VLS_nonNullable(__VLS_66.slots);
    }
    const __VLS_67 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_68 = __VLS_asFunctionalComponent(__VLS_67, new __VLS_67({ name: ("oasis"), tab: ("Oasis"), }));
    const __VLS_69 = __VLS_68({ name: ("oasis"), tab: ("Oasis"), }, ...__VLS_functionalComponentArgsRest(__VLS_68));
    __VLS_nonNullable(__VLS_72.slots).default;
    var __VLS_72;
    const __VLS_73 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_74 = __VLS_asFunctionalComponent(__VLS_73, new __VLS_73({ name: ("the beatles"), tab: ("the Beatles"), }));
    const __VLS_75 = __VLS_74({ name: ("the beatles"), tab: ("the Beatles"), }, ...__VLS_functionalComponentArgsRest(__VLS_74));
    __VLS_nonNullable(__VLS_78.slots).default;
    var __VLS_78;
    const __VLS_79 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_80 = __VLS_asFunctionalComponent(__VLS_79, new __VLS_79({ name: ("jay chou"), tab: ("Jay Chou"), }));
    const __VLS_81 = __VLS_80({ name: ("jay chou"), tab: ("Jay Chou"), }, ...__VLS_functionalComponentArgsRest(__VLS_80));
    __VLS_nonNullable(__VLS_84.slots).default;
    var __VLS_84;
    const __VLS_85 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_86 = __VLS_asFunctionalComponent(__VLS_85, new __VLS_85({ name: ("oasis1"), tab: ("Oasis1"), }));
    const __VLS_87 = __VLS_86({ name: ("oasis1"), tab: ("Oasis1"), }, ...__VLS_functionalComponentArgsRest(__VLS_86));
    __VLS_nonNullable(__VLS_90.slots).default;
    var __VLS_90;
    const __VLS_91 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_92 = __VLS_asFunctionalComponent(__VLS_91, new __VLS_91({ name: ("the beatles1"), tab: ("the Beatles1"), }));
    const __VLS_93 = __VLS_92({ name: ("the beatles1"), tab: ("the Beatles1"), }, ...__VLS_functionalComponentArgsRest(__VLS_92));
    __VLS_nonNullable(__VLS_96.slots).default;
    var __VLS_96;
    const __VLS_97 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_98 = __VLS_asFunctionalComponent(__VLS_97, new __VLS_97({ name: ("jay chou1"), tab: ("Jay Chou1"), }));
    const __VLS_99 = __VLS_98({ name: ("jay chou1"), tab: ("Jay Chou1"), }, ...__VLS_functionalComponentArgsRest(__VLS_98));
    __VLS_nonNullable(__VLS_102.slots).default;
    var __VLS_102;
    const __VLS_103 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_104 = __VLS_asFunctionalComponent(__VLS_103, new __VLS_103({ name: ("oasis2"), tab: ("Oasis2"), }));
    const __VLS_105 = __VLS_104({ name: ("oasis2"), tab: ("Oasis2"), }, ...__VLS_functionalComponentArgsRest(__VLS_104));
    __VLS_nonNullable(__VLS_108.slots).default;
    var __VLS_108;
    const __VLS_109 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_110 = __VLS_asFunctionalComponent(__VLS_109, new __VLS_109({ name: ("the beatles2"), tab: ("the Beatles2"), }));
    const __VLS_111 = __VLS_110({ name: ("the beatles2"), tab: ("the Beatles2"), }, ...__VLS_functionalComponentArgsRest(__VLS_110));
    __VLS_nonNullable(__VLS_114.slots).default;
    var __VLS_114;
    const __VLS_115 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_116 = __VLS_asFunctionalComponent(__VLS_115, new __VLS_115({ name: ("jay chou2"), tab: ("Jay Chou2"), }));
    const __VLS_117 = __VLS_116({ name: ("jay chou2"), tab: ("Jay Chou2"), }, ...__VLS_functionalComponentArgsRest(__VLS_116));
    __VLS_nonNullable(__VLS_120.slots).default;
    var __VLS_120;
    const __VLS_121 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_122 = __VLS_asFunctionalComponent(__VLS_121, new __VLS_121({ name: ("oasis3"), tab: ("Oasis3"), }));
    const __VLS_123 = __VLS_122({ name: ("oasis3"), tab: ("Oasis3"), }, ...__VLS_functionalComponentArgsRest(__VLS_122));
    __VLS_nonNullable(__VLS_126.slots).default;
    var __VLS_126;
    const __VLS_127 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_128 = __VLS_asFunctionalComponent(__VLS_127, new __VLS_127({ name: ("the beatles3"), tab: ("the Beatles3"), }));
    const __VLS_129 = __VLS_128({ name: ("the beatles3"), tab: ("the Beatles3"), }, ...__VLS_functionalComponentArgsRest(__VLS_128));
    __VLS_nonNullable(__VLS_132.slots).default;
    var __VLS_132;
    const __VLS_133 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_134 = __VLS_asFunctionalComponent(__VLS_133, new __VLS_133({ name: ("jay chou3"), tab: ("Jay Chou3"), }));
    const __VLS_135 = __VLS_134({ name: ("jay chou3"), tab: ("Jay Chou3"), }, ...__VLS_functionalComponentArgsRest(__VLS_134));
    __VLS_nonNullable(__VLS_138.slots).default;
    var __VLS_138;
    const __VLS_139 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_140 = __VLS_asFunctionalComponent(__VLS_139, new __VLS_139({ name: ("oasis4"), tab: ("Oasis4"), }));
    const __VLS_141 = __VLS_140({ name: ("oasis4"), tab: ("Oasis4"), }, ...__VLS_functionalComponentArgsRest(__VLS_140));
    __VLS_nonNullable(__VLS_144.slots).default;
    var __VLS_144;
    const __VLS_145 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_146 = __VLS_asFunctionalComponent(__VLS_145, new __VLS_145({ name: ("the beatles4"), tab: ("the Beatles4"), }));
    const __VLS_147 = __VLS_146({ name: ("the beatles4"), tab: ("the Beatles4"), }, ...__VLS_functionalComponentArgsRest(__VLS_146));
    __VLS_nonNullable(__VLS_150.slots).default;
    var __VLS_150;
    const __VLS_151 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_152 = __VLS_asFunctionalComponent(__VLS_151, new __VLS_151({ name: ("jay chou4"), tab: ("Jay Chou4"), }));
    const __VLS_153 = __VLS_152({ name: ("jay chou4"), tab: ("Jay Chou4"), }, ...__VLS_functionalComponentArgsRest(__VLS_152));
    __VLS_nonNullable(__VLS_156.slots).default;
    var __VLS_156;
    const __VLS_157 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_158 = __VLS_asFunctionalComponent(__VLS_157, new __VLS_157({ name: ("oasis5"), tab: ("Oasis5"), }));
    const __VLS_159 = __VLS_158({ name: ("oasis5"), tab: ("Oasis5"), }, ...__VLS_functionalComponentArgsRest(__VLS_158));
    __VLS_nonNullable(__VLS_162.slots).default;
    var __VLS_162;
    const __VLS_163 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_164 = __VLS_asFunctionalComponent(__VLS_163, new __VLS_163({ name: ("the beatles5"), tab: ("the Beatles5"), }));
    const __VLS_165 = __VLS_164({ name: ("the beatles5"), tab: ("the Beatles5"), }, ...__VLS_functionalComponentArgsRest(__VLS_164));
    __VLS_nonNullable(__VLS_168.slots).default;
    var __VLS_168;
    const __VLS_169 = __VLS_resolvedLocalAndGlobalComponents.NTabPane;
    /** @type { [typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, typeof __VLS_components.NTabPane, typeof __VLS_components.nTabPane, ] } */
    // @ts-ignore
    const __VLS_170 = __VLS_asFunctionalComponent(__VLS_169, new __VLS_169({ name: ("jay chou5"), tab: ("Jay Chou5"), }));
    const __VLS_171 = __VLS_170({ name: ("jay chou5"), tab: ("Jay Chou5"), }, ...__VLS_functionalComponentArgsRest(__VLS_170));
    __VLS_nonNullable(__VLS_174.slots).default;
    var __VLS_174;
    var __VLS_66;
    __VLS_nonNullable(__VLS_5.slots).default;
    var __VLS_5;
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
            placement: placement,
            type: type,
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
