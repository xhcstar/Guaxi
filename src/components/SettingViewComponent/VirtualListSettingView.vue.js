"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const vue_1 = require("vue");
exports.default = (0, vue_1.defineComponent)({
    setup() {
        const avatars = [
            'https://07akioni.oss-cn-beijing.aliyuncs.com/07akioni.jpeg',
            'https://avatars.githubusercontent.com/u/20943608?s=60&v=4',
            'https://avatars.githubusercontent.com/u/46394163?s=60&v=4',
            'https://avatars.githubusercontent.com/u/39197136?s=60&v=4',
            'https://avatars.githubusercontent.com/u/19239641?s=60&v=4'
        ];
        const messages = [
            '生活就像一场盛大的魔术表演，我们时而扮演着魔术师，时而又成了被戏弄的观众，最后发现，最大的魔术其实是时间，它把我们的青春和钱包都变得不见了',
            '复杂度不会消失，只会转移，当你听到一些人对于精致的概念模型侃侃而谈，请保持清醒',
            '当谈到虚拟列表时，能让你感觉列表像是无限长的，但实际上它只是在偷偷隐藏那些不可见的元素，就像是个懒惰的程序员拿着一个空白纸条说：“你看不见我，我也不会加载自己！”',
            '问题有时候本身就是答案，追寻的过程就是一种答案，语言具有破坏的能力，一旦一个东西说出口它就破坏了，我说要沉默，但我一说口沉默就没了，我说要享受当下，一说享受当下，当下就溜走了',
            '弗洛伊德阅读梦，发现一条直达潜意识的秘密通道。海明威阅读海，发现生命是一条要花一辈子才会上钩的鱼。凡高阅读麦田，发现艺术躲在太阳的背后乘凉。罗丹阅读人体，发现哥伦布没有发现的美丽海岸线。加缪阅读卡夫卡，发现真理已经被讲完一半'
        ];
        const items = Array.from({ length: 10000 }, (_, i) => ({
            key: `${i}`,
            value: i,
            avatar: avatars[i % avatars.length],
            message: messages[Math.floor(Math.random() * messages.length)]
        }));
        return {
            items
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
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NVirtualList;
    /** @type { [typeof __VLS_components.NVirtualList, typeof __VLS_components.nVirtualList, typeof __VLS_components.NVirtualList, typeof __VLS_components.nVirtualList, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0(Object.assign({ style: ({}) }, { itemSize: ((42)), items: ((__VLS_ctx.items)), itemResizable: (true) })));
    const __VLS_2 = __VLS_1(Object.assign({ style: ({}) }, { itemSize: ((42)), items: ((__VLS_ctx.items)), itemResizable: (true) }), ...__VLS_functionalComponentArgsRest(__VLS_1));
    var __VLS_6 = {};
    __VLS_elementAsFunction(__VLS_intrinsicElements.template, __VLS_intrinsicElements.template)({});
    {
        const { default: __VLS_thisSlot } = __VLS_nonNullable(__VLS_5.slots);
        const [{ item, index }] = __VLS_getSlotParams(__VLS_thisSlot);
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ key: ((item.key)) }, { class: ("item") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.img, __VLS_intrinsicElements.img)(Object.assign({ class: ("avatar") }, { src: ((item.avatar)), alt: ("") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.span, __VLS_intrinsicElements.span)({});
        (index);
        (item.message);
    }
    var __VLS_5;
    __VLS_styleScopedClasses['item'];
    __VLS_styleScopedClasses['avatar'];
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
