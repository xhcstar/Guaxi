"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
const chatStore_1 = require("../../stores/chatStore");
exports.default = (0, vue_1.defineComponent)({
    setup() {
        // 引入 Pinia 的 ChatStore
        const chatStore = (0, chatStore_1.useChatStore)();
        // 动态生成 options
        const dropdownOptions = (0, vue_1.computed)(() => {
            const { LLMlist } = chatStore;
            return [
                {
                    type: "group",
                    label: "远程模型",
                    key: "remote",
                    children: LLMlist.remoteModel.map((model) => ({
                        label: model.name,
                        key: `remote-${model.selectedModel}`,
                    })),
                },
                {
                    type: "group",
                    label: "本地模型",
                    key: "local",
                    children: LLMlist.localModel.map((model) => ({
                        label: model.name,
                        key: `local-${model.selectedModel}`,
                    })),
                },
            ];
        });
        // 处理模型选择变化
        const handleModelChange = (key) => {
            console.log('key', key);
            const selectModelId = parseInt(key.split('-')[1], 10);
            console.log('selectModelId', selectModelId);
            chatStore.setChatModel(selectModelId);
        };
        return {
            dropdownOptions,
            handleModelChange,
        };
    }
}); /* PartiallyEnd: #3632/script.vue */
function __VLS_template() {
    const __VLS_ctx = {};
    const __VLS_localComponents = Object.assign(Object.assign({}, {}), __VLS_ctx);
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
        onSelect: (__VLS_ctx.handleModelChange)
    };
    let __VLS_3;
    let __VLS_4;
    const __VLS_9 = __VLS_resolvedLocalAndGlobalComponents.NTag;
    /** @type { [typeof __VLS_components.NTag, typeof __VLS_components.nTag, typeof __VLS_components.NTag, typeof __VLS_components.nTag, ] } */
    // @ts-ignore
    const __VLS_10 = __VLS_asFunctionalComponent(__VLS_9, new __VLS_9({ round: (true), bordered: ((false)), size: ("small"), }));
    const __VLS_11 = __VLS_10({ round: (true), bordered: ((false)), size: ("small"), }, ...__VLS_functionalComponentArgsRest(__VLS_10));
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { icon: __VLS_thisSlot } = __VLS_nonNullable(__VLS_14.slots);
        const __VLS_15 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
        /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
        // @ts-ignore
        const __VLS_16 = __VLS_asFunctionalComponent(__VLS_15, new __VLS_15(Object.assign({ size: ("16") }, { style: ({}) })));
        const __VLS_17 = __VLS_16(Object.assign({ size: ("16") }, { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_16));
        __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1736674014610") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("13379"), width: ("16"), height: ("16") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M509.976 715.584c15.872 0 30.08-7.12 39.624-18.328a46.64 46.64 0 0 0 10.4-7.912L854.24 392.4a47.296 47.296 0 0 0-0.08-66.664l-0.224-0.216a46.664 46.664 0 0 0-65.992 0.296L509.976 606.344 232 325.816l-1.456-1.408a46.664 46.664 0 0 0-64.544 1.112l-0.216 0.216-1.416 1.48a47.296 47.296 0 0 0 1.328 65.184l294.256 296.944 1.52 1.472c2.752 2.536 5.736 4.68 8.88 6.44a51.888 51.888 0 0 0 39.624 18.32z"), fill: ("#666666"), "p-id": ("13380"), });
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
let __VLS_self;
