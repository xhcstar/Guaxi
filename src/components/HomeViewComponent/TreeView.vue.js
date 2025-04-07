"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const ionicons5_1 = require("@vicons/ionicons5");
const naive_ui_1 = require("naive-ui");
const seemly_1 = require("seemly");
const vue_1 = require("vue");
function createData(level = 4, baseKey = '') {
    if (!level)
        return undefined;
    return (0, seemly_1.repeat)(6 - level, undefined).map((_, index) => {
        const key = `${baseKey}${level}${index}`;
        return {
            label: createLabel(level),
            key,
            children: createData(level - 1, key)
        };
    });
}
function createLabel(level) {
    if (level === 4)
        return '道生一';
    if (level === 3)
        return '一生二';
    if (level === 2)
        return '二生三';
    if (level === 1)
        return '三生万物';
    return '';
}
exports.default = (0, vue_1.defineComponent)({
    setup() {
        return {
            data: createData(),
            defaultExpandedKeys: (0, vue_1.ref)(['40', '41']),
            renderSwitcherIcon: () => (0, vue_1.h)(naive_ui_1.NIcon, null, { default: () => (0, vue_1.h)(ionicons5_1.ChevronForward) }),
            renderSwitcherIconWithExpaned: ({ expanded }) => (0, vue_1.h)(naive_ui_1.NIcon, null, {
                default: () => (0, vue_1.h)(expanded ? ionicons5_1.SunnyOutline : ionicons5_1.PlanetOutline)
            })
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
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NInput;
    /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ round: (true), size: ("small"), placeholder: ("搜索"), }));
    const __VLS_2 = __VLS_1({ round: (true), size: ("small"), placeholder: ("搜索"), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NTree;
    /** @type { [typeof __VLS_components.NTree, typeof __VLS_components.nTree, ] } */
    // @ts-ignore
    const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6(Object.assign(Object.assign({ ref: ("treeInstRef"), blockLine: (true), data: ((__VLS_ctx.data)), defaultExpandAll: (true), defaultExpandedKeys: ((__VLS_ctx.defaultExpandedKeys)), virtualScroll: (true) }, { style: ({}) }), { renderSwitcherIcon: ((__VLS_ctx.renderSwitcherIconWithExpaned)), selectable: (true) })));
    const __VLS_8 = __VLS_7(Object.assign(Object.assign({ ref: ("treeInstRef"), blockLine: (true), data: ((__VLS_ctx.data)), defaultExpandAll: (true), defaultExpandedKeys: ((__VLS_ctx.defaultExpandedKeys)), virtualScroll: (true) }, { style: ({}) }), { renderSwitcherIcon: ((__VLS_ctx.renderSwitcherIconWithExpaned)), selectable: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_7));
    // @ts-ignore navigation for `const treeInstRef = ref()`
    __VLS_ctx.treeInstRef;
    var __VLS_12 = {};
    var __VLS_11;
    var __VLS_slots;
    var __VLS_inheritedAttrs;
    const __VLS_refs = {
        "treeInstRef": __VLS_12,
    };
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
