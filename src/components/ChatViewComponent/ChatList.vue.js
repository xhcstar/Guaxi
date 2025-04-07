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
const chatStore_1 = require("../../stores/chatStore");
const naive_ui_1 = require("naive-ui");
const { defineProps, defineSlots, defineEmits, defineExpose, defineModel, defineOptions, withDefaults, } = await Promise.resolve().then(() => __importStar(require('vue')));
const chatStore = (0, chatStore_1.useChatStore)();
const activeChat = (0, vue_1.computed)(() => chatStore.SelectModelchatSessions.find((chatmodel) => chatmodel.id === chatStore.selectedModel));
// 当前会话的消息记录
const activeChatMessages = (0, vue_1.computed)(() => { var _a; return ((_a = activeChat.value) === null || _a === void 0 ? void 0 : _a.MessagesList.find((topic) => topic.topicid === chatStore.selectedTopic)) || []; });
const showMenu = (0, vue_1.ref)(false);
const menuTop = (0, vue_1.ref)(0);
const menuLeft = (0, vue_1.ref)(0);
const selectedChat = (0, vue_1.ref)(null);
const selectChat = (id) => {
    chatStore.setModelTopic(id);
};
// 格式化时间戳
const formatTimestamp = (timestamp) => {
    if (!timestamp)
        return "";
    const date = new Date(timestamp);
    return `${date.getHours()}:${String(date.getMinutes()).padStart(2, "0")}`;
};
// 新建聊天逻辑（可自定义实现）
const handleAddChat = () => {
    chatStore.addChatSession();
};
// 显示右键菜单
const showContextMenu = (event, chat) => {
    event.preventDefault();
    selectedChat.value = chat;
    showMenu.value = true;
    menuTop.value = event.clientY + "px";
    menuLeft.value = event.clientX + "px";
};
// 删除聊天条目
const deleteChatItem = () => {
    if (activeChatMessages.value) {
        activeChat.value.MessagesList = activeChat.value.MessagesList.filter((chat) => chat !== activeChatMessages.value);
    }
    showMenu.value = false; // 隐藏菜单
};
// 监听全局点击事件，点击其他地方关闭菜单
const closeContextMenu = (event) => {
    if (!event.target.closest(".context-menu")) {
        showMenu.value = false;
    }
};
// 组件挂载时添加监听
(0, vue_1.onMounted)(() => {
    document.addEventListener("click", closeContextMenu);
});
// 组件卸载时移除监听
(0, vue_1.onUnmounted)(() => {
    document.removeEventListener("click", closeContextMenu);
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
    __VLS_styleScopedClasses['chat-item'];
    __VLS_styleScopedClasses['chat-item'];
    __VLS_styleScopedClasses['menu-item'];
    // CSS variable injection 
    // CSS variable injection end 
    let __VLS_resolvedLocalAndGlobalComponents;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-list-container") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-list-header") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ style: ({}) }));
    const __VLS_0 = __VLS_resolvedLocalAndGlobalComponents.NInput;
    /** @type { [typeof __VLS_components.NInput, typeof __VLS_components.nInput, ] } */
    // @ts-ignore
    const __VLS_1 = __VLS_asFunctionalComponent(__VLS_0, new __VLS_0({ round: (true), size: ("small"), placeholder: ("搜索"), }));
    const __VLS_2 = __VLS_1({ round: (true), size: ("small"), placeholder: ("搜索"), }, ...__VLS_functionalComponentArgsRest(__VLS_1));
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)({});
    const __VLS_6 = __VLS_resolvedLocalAndGlobalComponents.NIcon;
    /** @type { [typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, typeof __VLS_components.NIcon, typeof __VLS_components.nIcon, ] } */
    // @ts-ignore
    const __VLS_7 = __VLS_asFunctionalComponent(__VLS_6, new __VLS_6(Object.assign(Object.assign({ 'onClick': {} }, { size: ("20") }), { style: ({}) })));
    const __VLS_8 = __VLS_7(Object.assign(Object.assign({ 'onClick': {} }, { size: ("20") }), { style: ({}) }), ...__VLS_functionalComponentArgsRest(__VLS_7));
    let __VLS_12;
    const __VLS_13 = {
        onClick: (__VLS_ctx.handleAddChat)
    };
    let __VLS_9;
    let __VLS_10;
    __VLS_elementAsFunction(__VLS_intrinsicElements.svg, __VLS_intrinsicElements.svg)(Object.assign(Object.assign({ t: ("1736598327807") }, { class: ("icon") }), { viewBox: ("0 0 1024 1024"), version: ("1.1"), xmlns: ("http://www.w3.org/2000/svg"), "p-id": ("21966"), width: ("20"), height: ("20") }));
    __VLS_elementAsFunction(__VLS_intrinsicElements.path, __VLS_intrinsicElements.path)({ d: ("M146.311835 1024a146.275266 146.275266 0 0 1-146.275266-146.275266v-731.37633a146.275266 146.275266 0 0 1 146.275266-146.275266h731.37633a146.275266 146.275266 0 0 1 146.275266 146.275266v731.37633a146.275266 146.275266 0 0 1-146.275266 146.275266z m-73.137633-877.651596v731.37633a73.137633 73.137633 0 0 0 73.137633 73.137633h731.37633a73.137633 73.137633 0 0 0 73.137633-73.137633v-731.37633a73.137633 73.137633 0 0 0-73.137633-73.137633H146.311835a73.137633 73.137633 0 0 0-73.137633 73.137633z m399.624027 600.752517V551.165202H276.935647a39.128634 39.128634 0 0 1-39.128633-39.128633 39.274909 39.274909 0 0 1 39.128633-39.274909h195.862582V276.899079a39.274909 39.274909 0 0 1 39.274909-39.128634 39.128634 39.128634 0 0 1 39.128633 39.128634v195.862581h196.008857a39.274909 39.274909 0 0 1 39.128633 39.274909 39.128634 39.128634 0 0 1-39.128633 39.128633h-196.008857v195.935719a39.128634 39.128634 0 0 1-39.128633 39.128634 39.274909 39.274909 0 0 1-39.348047-39.055496z"), fill: ("#6A6A6A"), "p-id": ("21967"), });
    __VLS_nonNullable(__VLS_11.slots).default;
    var __VLS_11;
    __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-list") }));
    for (const [curtopic] of __VLS_getVForSourceType((__VLS_ctx.activeChat.MessagesList))) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign(Object.assign(Object.assign(Object.assign({ onClick: (...[$event]) => {
                __VLS_ctx.selectChat(curtopic.topicid);
            } }, { onContextmenu: (...[$event]) => {
                __VLS_ctx.showContextMenu($event, curtopic);
            } }), { key: ((curtopic.topicid)) }), { class: ("chat-item") }), { class: (({ active: __VLS_ctx.activeChat.id === __VLS_ctx.chatStore.selectedModel })) }));
        const __VLS_14 = __VLS_resolvedLocalAndGlobalComponents.NAvatar;
        /** @type { [typeof __VLS_components.NAvatar, typeof __VLS_components.nAvatar, ] } */
        // @ts-ignore
        const __VLS_15 = __VLS_asFunctionalComponent(__VLS_14, new __VLS_14({ src: ((__VLS_ctx.activeChat.ModelAvatar)), size: ("medium"), round: (true), }));
        const __VLS_16 = __VLS_15({ src: ((__VLS_ctx.activeChat.ModelAvatar)), size: ("medium"), round: (true), }, ...__VLS_functionalComponentArgsRest(__VLS_15));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-info") }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-title") }));
        (__VLS_ctx.activeChat.name);
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("chat-last-message") }));
        (curtopic.topic.at(0).content);
    }
    if (__VLS_ctx.showMenu) {
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ class: ("context-menu") }, { style: (({ top: __VLS_ctx.menuTop, left: __VLS_ctx.menuLeft })) }));
        __VLS_elementAsFunction(__VLS_intrinsicElements.div, __VLS_intrinsicElements.div)(Object.assign({ onClick: (__VLS_ctx.deleteChatItem) }, { class: ("menu-item") }));
    }
    __VLS_styleScopedClasses['chat-list-container'];
    __VLS_styleScopedClasses['chat-list-header'];
    __VLS_styleScopedClasses['icon'];
    __VLS_styleScopedClasses['chat-list'];
    __VLS_styleScopedClasses['chat-item'];
    __VLS_styleScopedClasses['active'];
    __VLS_styleScopedClasses['chat-info'];
    __VLS_styleScopedClasses['chat-title'];
    __VLS_styleScopedClasses['chat-last-message'];
    __VLS_styleScopedClasses['context-menu'];
    __VLS_styleScopedClasses['menu-item'];
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
            NAvatar: naive_ui_1.NAvatar,
            chatStore: chatStore,
            activeChat: activeChat,
            showMenu: showMenu,
            menuTop: menuTop,
            menuLeft: menuLeft,
            selectChat: selectChat,
            handleAddChat: handleAddChat,
            showContextMenu: showContextMenu,
            deleteChatItem: deleteChatItem,
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
